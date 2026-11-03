use super::*;
use crate::render_contract::{
    CookedGeometry, CoordinateConvention, GeometryBounds, GeometryIndexFormat, GeometryTopology,
    SubmeshRange, ValidateRenderContract, VertexAttribute, VertexFormat, VertexSemantic,
};

/// The full-density collision and navigation use precisely the same surface as
/// render LOD zero. Coarser visual LODs never replace collision implicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainCollisionMesh3D {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

impl HeightfieldAsset3D {
    pub fn build_tile_mesh(&self, key: &str, step: u16) -> ContractResult<CookedGeometry> {
        self.validate()?;
        if step == 0 || !step.is_power_of_two() || step > self.quads {
            return Err(invalid("/lod/step", "Invalid terrain LOD stride."));
        }
        let tile = self.tile(key)?;
        let lattice = self.sample_lattice()?;
        let n = usize::from(self.quads);
        let stride = usize::from(step);
        let mut vertices = Vec::<([f32; 3], [f32; 3], [f32; 2])>::new();
        let mut layers = vec![Vec::<u32>::new(); self.material_slots.len()];
        for z in (0..n).step_by(stride) {
            for x in (0..n).step_by(stride) {
                // Mixed-layer cells stay full density, so a visual LOD cannot
                // silently remove a road, shoreline or material boundary.
                let layer = tile.cell_layers.get(z * n + x).copied().unwrap_or(0);
                let mixed = (z..z + stride).any(|cz| {
                    (x..x + stride)
                        .any(|cx| tile.cell_layers.get(cz * n + cx).copied().unwrap_or(0) != layer)
                });
                let cell_stride = if mixed { 1 } else { stride };
                for cz in (z..z + stride).step_by(cell_stride) {
                    for cx in (x..x + stride).step_by(cell_stride) {
                        emit_cell(
                            self,
                            tile,
                            &lattice,
                            cx,
                            cz,
                            cell_stride,
                            &mut vertices,
                            &mut layers,
                        )?;
                    }
                }
            }
        }
        let mut vertex_bytes = Vec::with_capacity(vertices.len() * 32);
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for (position, normal, uv) in &vertices {
            for i in 0..3 {
                min[i] = min[i].min(position[i]);
                max[i] = max[i].max(position[i]);
            }
            for value in position.iter().chain(normal).chain(uv) {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let center = std::array::from_fn(|i| (min[i] + max[i]) * 0.5);
        let radius = vertices
            .iter()
            .map(|(position, _, _)| {
                (0..3)
                    .map(|i| f64::from(position[i] - center[i]).powi(2))
                    .sum::<f64>()
                    .sqrt() as f32
            })
            .fold(0f32, f32::max);
        let mut index_bytes = Vec::new();
        let mut submeshes = Vec::new();
        for (layer, indices) in layers.iter().enumerate() {
            if indices.is_empty() {
                continue;
            }
            submeshes.push(SubmeshRange {
                first_index: (index_bytes.len() / 4) as u32,
                index_count: indices.len() as u32,
                material_slot: self.material_slots[layer].clone(),
            });
            for index in indices {
                index_bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
        let output = CookedGeometry {
            vertex_count: vertices.len() as u32,
            index_count: (index_bytes.len() / 4) as u32,
            vertex_bytes,
            index_bytes,
            vertex_stride: 32,
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
            ],
            index_format: GeometryIndexFormat::Uint32,
            topology: GeometryTopology::TriangleList,
            submeshes,
            bounds: GeometryBounds {
                min,
                max,
                sphere_center: center,
                sphere_radius: radius + 1e-4,
            },
            coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
            material_slots: self.material_slots.clone(),
        };
        output
            .validate()
            .map_err(|e| invalid("/mesh", e.to_string()))?;
        Ok(output)
    }

    pub fn build_tile_collision(&self, key: &str) -> ContractResult<TerrainCollisionMesh3D> {
        let mesh = self.build_tile_mesh(key, 1)?;
        Ok(TerrainCollisionMesh3D {
            vertices: mesh
                .vertex_bytes
                .chunks_exact(32)
                .map(|vertex| {
                    std::array::from_fn(|i| {
                        f32::from_le_bytes(vertex[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect(),
            triangles: mesh
                .index_bytes
                .chunks_exact(12)
                .map(|triangle| {
                    std::array::from_fn(|i| {
                        u32::from_le_bytes(triangle[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect(),
        })
    }
}

fn emit_cell(
    source: &HeightfieldAsset3D,
    tile: &HeightfieldTile3D,
    lattice: &BTreeMap<[i64; 2], f32>,
    x: usize,
    z: usize,
    step: usize,
    vertices: &mut Vec<([f32; 3], [f32; 3], [f32; 2])>,
    layers: &mut [Vec<u32>],
) -> ContractResult<()> {
    let n = usize::from(source.quads);
    let layer = usize::from(tile.cell_layers.get(z * n + x).copied().unwrap_or(0));
    let side = n + 1;
    let height = |x: usize, z: usize| tile.heights[z * side + x];
    let origin = [
        i64::from(tile.coordinate[0]) * n as i64,
        i64::from(tile.coordinate[1]) * n as i64,
    ];
    let normal = |x: usize, z: usize| {
        let gx = origin[0] + x as i64;
        let gz = origin[1] + z as i64;
        let h = height(x, z);
        let left = lattice.get(&[gx - 1, gz]).copied();
        let right = lattice.get(&[gx + 1, gz]).copied();
        let back = lattice.get(&[gx, gz - 1]).copied();
        let front = lattice.get(&[gx, gz + 1]).copied();
        let derivative = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(a), Some(b)) => (b - a) / (2. * source.sample_spacing),
            (Some(a), None) => (h - a) / source.sample_spacing,
            (None, Some(b)) => (b - h) / source.sample_spacing,
            (None, None) => 0.,
        };
        normalized([-derivative(left, right), 1., -derivative(back, front)])
    };
    let mut perimeter = Vec::new();
    // Clockwise viewed from above, producing +Y-facing triangles with the
    // center-first winding below. Every cell edge keeps its lattice samples,
    // including material-refined interior edges, so a coarse neighbour cannot
    // leave a T-junction on a curved heightfield. LOD reduces cell interiors.
    for offset in 0..step {
        perimeter.push((x + offset, z));
    }
    for offset in 0..step {
        perimeter.push((x + step, z + offset));
    }
    for offset in 0..step {
        perimeter.push((x + step - offset, z + step));
    }
    for offset in 0..step {
        perimeter.push((x, z + step - offset));
    }
    let center_x = x as f32 + step as f32 * 0.5;
    let center_z = z as f32 + step as f32 * 0.5;
    let center_height = if step == 1 {
        (height(x, z) + height(x + 1, z) + height(x, z + 1) + height(x + 1, z + 1)) * 0.25
    } else {
        height(x + step / 2, z + step / 2)
    };
    let center_normal = normalized(perimeter.iter().fold([0.; 3], |mut sum, &(px, pz)| {
        let n = normal(px, pz);
        for i in 0..3 {
            sum[i] += n[i];
        }
        sum
    }));
    let base =
        u32::try_from(vertices.len()).map_err(|_| invalid("/mesh", "Vertex index overflow."))?;
    vertices.push((
        [
            center_x * source.sample_spacing,
            center_height,
            center_z * source.sample_spacing,
        ],
        center_normal,
        [center_x / n as f32, center_z / n as f32],
    ));
    for &(px, pz) in &perimeter {
        vertices.push((
            [
                px as f32 * source.sample_spacing,
                height(px, pz),
                pz as f32 * source.sample_spacing,
            ],
            normal(px, pz),
            [px as f32 / n as f32, pz as f32 / n as f32],
        ));
    }
    for i in 0..perimeter.len() {
        layers[layer].extend_from_slice(&[
            base,
            base + 1 + ((i + 1) % perimeter.len()) as u32,
            base + 1 + i as u32,
        ]);
    }
    Ok(())
}

fn normalized(value: [f32; 3]) -> [f32; 3] {
    let length = value.iter().map(|v| v * v).sum::<f32>().sqrt();
    value.map(|v| v / length)
}
