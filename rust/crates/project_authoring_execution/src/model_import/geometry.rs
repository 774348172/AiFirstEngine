use super::*;
use engine_runtime::render_contract::*;
use gltf::accessor::{DataType, Dimensions};
use gltf::Semantic;
use std::sync::Arc;

pub(super) fn validate_accessor<'a>(
    accessor: &gltf::Accessor<'a>,
    budget: &Budget<'_>,
) -> Result<gltf::buffer::View<'a>> {
    let path = format!("/accessors/{}", accessor.index());
    if accessor.sparse().is_some() {
        return Err(fail(
            budget.source,
            &format!("{path}/sparse"),
            "import_sparse_unsupported",
            "Sparse accessors are unsupported.",
        ));
    }
    let view = accessor.view().ok_or_else(|| {
        fail(
            budget.source,
            &path,
            "import_accessor_invalid",
            "A non-sparse accessor requires a bufferView.",
        )
    })?;
    let size = accessor.size();
    let stride = view.stride().unwrap_or(size);
    let component = accessor.data_type().size();
    let end = accessor
        .count()
        .checked_sub(1)
        .and_then(|n| n.checked_mul(stride))
        .and_then(|n| n.checked_add(size))
        .and_then(|n| n.checked_add(accessor.offset()));
    let view_end = view.offset().checked_add(view.length());
    let absolute = view.offset().checked_add(accessor.offset());
    if stride < size
        || view
            .stride()
            .is_some_and(|s| !(4..=252).contains(&s) || s % 4 != 0)
        || accessor.offset() % component != 0
        || absolute.is_none_or(|n| n % component != 0)
        || end.is_none_or(|n| n > view.length())
        || view_end.is_none_or(|n| n > view.buffer().length())
    {
        return Err(fail(
            budget.source,
            &path,
            "import_accessor_range_invalid",
            "Accessor offset/count/stride/alignment exceeds its bufferView or declared buffer.",
        ));
    }
    Ok(view)
}

pub(super) fn validate_primitive(
    primitive: &gltf::Primitive<'_>,
    path: &str,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if primitive.mode() != gltf::mesh::Mode::Triangles {
        return Err(fail(
            budget.source,
            &format!("{path}/mode"),
            "import_topology_unsupported",
            "Only triangle-list primitives are supported.",
        ));
    }
    let position = primitive.get(&Semantic::Positions).ok_or_else(|| {
        fail(
            budget.source,
            &format!("{path}/attributes/POSITION"),
            "import_position_missing",
            "POSITION is required; no placeholder geometry is generated.",
        )
    })?;
    let count = position.count();
    charge(
        &mut budget.vertices,
        count as u64,
        budget.limits.vertices,
        budget.source,
        path,
        "import_vertex_budget_exceeded",
    )?;
    let index_count = primitive.indices().map_or(count, |a| a.count());
    charge(
        &mut budget.indices,
        index_count as u64,
        budget.limits.indices,
        budget.source,
        path,
        "import_index_budget_exceeded",
    )?;
    if count == 0 || index_count == 0 || index_count % 3 != 0 {
        return Err(fail(
            budget.source,
            path,
            "import_triangle_invalid",
            "Triangle-list requires nonempty vertices and a multiple of three indices.",
        ));
    }
    let mut stride = 0u64;
    for (semantic, accessor) in primitive.attributes() {
        let valid = match semantic {
            Semantic::Positions | Semantic::Normals => {
                stride += 12;
                accessor.data_type() == DataType::F32
                    && accessor.dimensions() == Dimensions::Vec3
                    && !accessor.normalized()
            }
            Semantic::TexCoords(0) => {
                stride += 8;
                accessor.dimensions() == Dimensions::Vec2 && normalized_float(&accessor)
            }
            Semantic::Colors(0) => {
                stride += 16;
                matches!(accessor.dimensions(), Dimensions::Vec3 | Dimensions::Vec4)
                    && normalized_float(&accessor)
            }
            Semantic::Joints(0) => {
                accessor.dimensions() == Dimensions::Vec4
                    && matches!(accessor.data_type(), DataType::U8 | DataType::U16)
                    && !accessor.normalized()
            }
            Semantic::Weights(0) => {
                accessor.dimensions() == Dimensions::Vec4 && normalized_float(&accessor)
            }
            _ => false,
        };
        if !valid || accessor.count() != count {
            return Err(fail(budget.source, &format!("{path}/attributes/{}", semantic_name(&semantic)), "import_attribute_unsupported", "Supported vertex streams are matching-count FLOAT POSITION/NORMAL, FLOAT or normalized U8/U16 UV0/COLOR_0."));
        }
    }
    if let Some(indices) = primitive.indices() {
        if !matches!(
            indices.data_type(),
            DataType::U8 | DataType::U16 | DataType::U32
        ) || indices.dimensions() != Dimensions::Scalar
            || indices.normalized()
            || indices.view().is_some_and(|v| v.stride().is_some())
        {
            return Err(fail(
                budget.source,
                &format!("{path}/indices"),
                "import_index_invalid",
                "Indices must be tightly packed non-normalized unsigned scalar values.",
            ));
        }
    }
    if let Some(texture) = primitive
        .material()
        .pbr_metallic_roughness()
        .base_color_texture()
    {
        if texture.tex_coord() != 0 || primitive.get(&Semantic::TexCoords(0)).is_none() {
            return Err(fail(
                budget.source,
                &format!("{path}/attributes/TEXCOORD_0"),
                "import_uv_missing",
                "Base-color texture requires supported UV0.",
            ));
        }
    }
    // Charge the worst-case normalized index width before any vertex/index allocation.
    let size = (count as u64)
        .checked_mul(stride)
        .and_then(|n| {
            (index_count as u64)
                .checked_mul(4)
                .and_then(|i| n.checked_add(i))
        })
        .ok_or_else(|| {
            fail(
                budget.source,
                path,
                "import_cooked_budget_exceeded",
                "Normalized geometry length overflow.",
            )
        })?;
    charge(
        &mut budget.cooked_bytes,
        size,
        budget.limits.cooked_bytes,
        budget.source,
        path,
        "import_cooked_budget_exceeded",
    )?;
    Ok(())
}

fn normalized_float(accessor: &gltf::Accessor<'_>) -> bool {
    (accessor.data_type() == DataType::F32 && !accessor.normalized())
        || (matches!(accessor.data_type(), DataType::U8 | DataType::U16) && accessor.normalized())
}

fn semantic_name(semantic: &Semantic) -> String {
    match semantic {
        Semantic::Positions => "POSITION".into(),
        Semantic::Normals => "NORMAL".into(),
        Semantic::Tangents => "TANGENT".into(),
        Semantic::Colors(n) => format!("COLOR_{n}"),
        Semantic::TexCoords(n) => format!("TEXCOORD_{n}"),
        Semantic::Joints(n) => format!("JOINTS_{n}"),
        Semantic::Weights(n) => format!("WEIGHTS_{n}"),
        Semantic::Extras(name) => name.clone(),
    }
}

pub(super) fn decode_primitive(
    primitive: &gltf::Primitive<'_>,
    buffers: &BTreeMap<usize, Arc<[u8]>>,
    source: &str,
    path: &str,
) -> Result<CookedGeometry> {
    for (_, accessor) in primitive
        .attributes()
        .chain(primitive.indices().map(|a| (Semantic::Positions, a)))
    {
        super::source::checked_view(&accessor.view().unwrap(), buffers, source)?;
    }
    let invalid = || {
        fail(
            source,
            &path,
            "import_geometry_invalid",
            "Geometry stream is incomplete or contains non-finite/out-of-range values.",
        )
    };
    let reader = primitive.reader(|buffer| buffers.get(&buffer.index()).map(|b| b.as_ref()));
    let positions: Vec<_> = reader.read_positions().ok_or_else(invalid)?.collect();
    let normals: Option<Vec<_>> = reader.read_normals().map(Iterator::collect);
    let uv: Option<Vec<_>> = reader.read_tex_coords(0).map(|v| v.into_f32().collect());
    let colors: Option<Vec<_>> = reader.read_colors(0).map(|v| v.into_rgba_f32().collect());
    let indices: Vec<u32> = match primitive.indices() {
        Some(_) => reader
            .read_indices()
            .ok_or_else(invalid)?
            .into_u32()
            .collect(),
        None => (0..positions.len() as u32).collect(),
    };
    let expected = primitive.get(&Semantic::Positions).unwrap().count();
    if positions.len() != expected
        || normals.as_ref().is_some_and(|v| v.len() != expected)
        || uv.as_ref().is_some_and(|v| v.len() != expected)
        || colors.as_ref().is_some_and(|v| v.len() != expected)
        || indices.iter().any(|i| *i as usize >= expected)
    {
        return Err(invalid());
    }
    let mut attributes = vec![VertexAttribute {
        semantic: VertexSemantic::Position,
        format: VertexFormat::Float32x3,
        offset: 0,
    }];
    let mut stride = 12;
    for (exists, semantic, format, size) in [
        (
            normals.is_some(),
            VertexSemantic::Normal,
            VertexFormat::Float32x3,
            12,
        ),
        (
            uv.is_some(),
            VertexSemantic::TexCoord0,
            VertexFormat::Float32x2,
            8,
        ),
        (
            colors.is_some(),
            VertexSemantic::Color0,
            VertexFormat::Float32x4,
            16,
        ),
    ] {
        if exists {
            attributes.push(VertexAttribute {
                semantic,
                format,
                offset: stride,
            });
            stride += size;
        }
    }
    let mut vertex_bytes = Vec::with_capacity(expected * stride as usize);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for (index, position) in positions.iter().enumerate() {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
        for value in position
            .iter()
            .copied()
            .chain(normals.as_ref().into_iter().flat_map(|v| v[index]))
            .chain(uv.as_ref().into_iter().flat_map(|v| v[index]))
            .chain(colors.as_ref().into_iter().flat_map(|v| v[index]))
        {
            if !value.is_finite() {
                return Err(invalid());
            }
            vertex_bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    if colors
        .as_ref()
        .is_some_and(|c| c.iter().flatten().any(|v| !(0.0..=1.0).contains(v)))
    {
        return Err(invalid());
    }
    let sphere_center =
        std::array::from_fn(|i| ((f64::from(min[i]) + f64::from(max[i])) * 0.5) as f32);
    let sphere_radius = positions
        .iter()
        .map(|p| {
            (0..3)
                .map(|i| (f64::from(p[i]) - f64::from(sphere_center[i])).powi(2))
                .sum::<f64>()
        })
        .fold(0.0f64, f64::max)
        .sqrt() as f32;
    if !sphere_radius.is_finite() {
        return Err(invalid());
    }
    let index_format = if indices.iter().copied().max().unwrap_or(0) <= u16::MAX as u32 {
        GeometryIndexFormat::Uint16
    } else {
        GeometryIndexFormat::Uint32
    };
    let mut index_bytes = Vec::with_capacity(
        indices.len()
            * if index_format == GeometryIndexFormat::Uint16 {
                2
            } else {
                4
            },
    );
    for index in &indices {
        match index_format {
            GeometryIndexFormat::Uint16 => {
                index_bytes.extend_from_slice(&(*index as u16).to_le_bytes())
            }
            GeometryIndexFormat::Uint32 => index_bytes.extend_from_slice(&index.to_le_bytes()),
        }
    }
    let geometry = CookedGeometry {
        vertex_bytes,
        index_bytes,
        vertex_stride: stride,
        vertex_count: expected as u32,
        attributes,
        index_format,
        index_count: indices.len() as u32,
        topology: GeometryTopology::TriangleList,
        submeshes: vec![SubmeshRange {
            first_index: 0,
            index_count: indices.len() as u32,
            material_slot: format!("primitive-{}", primitive.index()),
        }],
        bounds: GeometryBounds {
            min,
            max,
            sphere_center,
            sphere_radius,
        },
        coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
        material_slots: vec![format!("primitive-{}", primitive.index())],
    };
    geometry
        .validate()
        .map_err(|e| fail(source, &path, "import_geometry_invalid", e.to_string()))?;
    Ok(geometry)
}
