use super::*;

pub(super) fn merge(
    settings: &ModelImportV1,
    output: &ImportOutput,
    primitives: &[&DecodedPrimitive],
    conversion: &coordinates::Conversion,
) -> Result<CookedGeometry> {
    let path = &output.source_locator;
    let invalid = |message| fail(&settings.source, path, "import_geometry_invalid", message);
    let vertices: usize = primitives
        .iter()
        .map(|p| p.geometry.vertex_count as usize)
        .sum();
    let indices: usize = primitives
        .iter()
        .map(|p| p.geometry.index_count as usize)
        .sum();
    if vertices as u64 * 48 + indices as u64 * 4 > settings.budgets.cooked_bytes {
        return Err(fail(
            &settings.source,
            path,
            "import_cooked_budget_exceeded",
            "Normalized vertex layout exceeds cookedBytes before allocation.",
        ));
    }
    let mut slots = BTreeMap::new();
    for slot in &output.material_slots {
        if slots.insert(slot.source_locator.as_str(), slot).is_some() {
            return Err(invalid("Duplicate primitive slot mapping."));
        }
    }
    if slots.len() != primitives.len() {
        return Err(invalid(
            "Every primitive requires exactly one stable named slot mapping.",
        ));
    }
    let mut vertex_bytes = Vec::with_capacity(vertices * 48);
    let mut index_values = Vec::with_capacity(indices);
    let mut submeshes = Vec::new();
    let mut material_slots = Vec::new();
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut vertex_offset = 0u32;
    for p in primitives {
        let g = &p.geometry;
        let locator = format!("{path}/primitives/{}", p.primitive_index);
        let slot = slots.get(locator.as_str()).ok_or_else(|| {
            invalid("Material slot locator does not identify a selected primitive.")
        })?;
        material_slots.push(slot.name.clone());
        submeshes.push(SubmeshRange {
            first_index: index_values.len() as u32,
            index_count: g.index_count,
            material_slot: slot.name.clone(),
        });
        for vertex in g.vertex_bytes.chunks_exact(g.vertex_stride as usize) {
            let position = conversion.position(read(g, vertex, VertexSemantic::Position, [0.; 3]));
            let normal = conversion.normal(
                read(g, vertex, VertexSemantic::Normal, [0., 0., 1.]),
                &settings.source,
                &locator,
            )?;
            for i in 0..3 {
                min[i] = min[i].min(position[i]);
                max[i] = max[i].max(position[i]);
            }
            for value in position
                .into_iter()
                .chain(normal)
                .chain(read(g, vertex, VertexSemantic::TexCoord0, [0.; 2]))
                .chain(read(g, vertex, VertexSemantic::Color0, [1.; 4]))
            {
                if !value.is_finite() {
                    return Err(invalid("Converted vertex is not representable."));
                }
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let width = if g.index_format == GeometryIndexFormat::Uint16 {
            2
        } else {
            4
        };
        for triangle in g.index_bytes.chunks_exact(width * 3) {
            let mut face: [u32; 3] = std::array::from_fn(|i| {
                let bytes = &triangle[i * width..(i + 1) * width];
                vertex_offset
                    + if width == 2 {
                        u16::from_le_bytes(bytes.try_into().unwrap()) as u32
                    } else {
                        u32::from_le_bytes(bytes.try_into().unwrap())
                    }
            });
            if conversion.reflected() {
                face.swap(1, 2);
            }
            index_values.extend(face);
        }
        vertex_offset += g.vertex_count;
    }
    let sphere_center: [f32; 3] =
        std::array::from_fn(|i| ((f64::from(min[i]) + f64::from(max[i])) * 0.5) as f32);
    let sphere_radius = vertex_bytes
        .chunks_exact(48)
        .map(|v| {
            (0..3)
                .map(|i| {
                    let p = f32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap());
                    (f64::from(p) - f64::from(sphere_center[i])).powi(2)
                })
                .sum::<f64>()
        })
        .fold(0., f64::max)
        .sqrt() as f32;
    let index_format = if index_values.iter().copied().max().unwrap_or(0) <= u16::MAX as u32 {
        GeometryIndexFormat::Uint16
    } else {
        GeometryIndexFormat::Uint32
    };
    let mut index_bytes = Vec::with_capacity(
        indices
            * if index_format == GeometryIndexFormat::Uint16 {
                2
            } else {
                4
            },
    );
    for value in index_values {
        if index_format == GeometryIndexFormat::Uint16 {
            index_bytes.extend_from_slice(&(value as u16).to_le_bytes());
        } else {
            index_bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    let result = CookedGeometry {
        vertex_bytes,
        index_bytes,
        vertex_stride: 48,
        attributes: vec![
            VertexAttribute {
                semantic: VertexSemantic::Position,
                format: VertexFormat::Float32x3,
                offset: 0,
            },
            VertexAttribute {
                semantic: VertexSemantic::Normal,
                format: VertexFormat::Float32x3,
                offset: 12,
            },
            VertexAttribute {
                semantic: VertexSemantic::TexCoord0,
                format: VertexFormat::Float32x2,
                offset: 24,
            },
            VertexAttribute {
                semantic: VertexSemantic::Color0,
                format: VertexFormat::Float32x4,
                offset: 32,
            },
        ],
        vertex_count: vertices as u32,
        index_count: indices as u32,
        index_format,
        topology: GeometryTopology::TriangleList,
        submeshes,
        bounds: GeometryBounds {
            min,
            max,
            sphere_center,
            sphere_radius,
        },
        coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
        material_slots,
    };
    result.validate().map_err(|e| {
        fail(
            &settings.source,
            path,
            "import_geometry_invalid",
            e.to_string(),
        )
    })?;
    Ok(result)
}

fn read<const N: usize>(
    geometry: &CookedGeometry,
    vertex: &[u8],
    semantic: VertexSemantic,
    defaults: [f32; N],
) -> [f32; N] {
    match geometry.attributes.iter().find(|a| a.semantic == semantic) {
        Some(a) => std::array::from_fn(|i| {
            let o = a.offset as usize + i * 4;
            f32::from_le_bytes(vertex[o..o + 4].try_into().unwrap())
        }),
        None => defaults,
    }
}
