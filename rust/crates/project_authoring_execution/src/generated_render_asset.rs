//! Build-side generated geometry and Unlit lowering into the existing U1 contract.
use crate::three_d_contract::{GeneratedPrimitive, MaterialAssetV2, MeshAssetV2, MeshSource};
use engine_runtime::render_contract::*;
use engine_runtime::runtime_texture::TextureSampler;
use engine_runtime::spatial_contract::{decode, ContractResult, SpatialContractError};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn lower(document: &Value) -> ContractResult<RenderAssetData> {
    let result = match document["schemaVersion"].as_str() {
        Some("mesh-asset.v2") => {
            let source: MeshAssetV2 = decode(document.clone(), "")?;
            source.validate()?;
            let MeshSource::Generated { geometry } = source.source;
            RenderAssetData::Mesh(generate(geometry))
        }
        Some("material-asset.v2") => {
            let source: MaterialAssetV2 = decode(document.clone(), "")?;
            source.validate()?;
            let mut textures = BTreeMap::new();
            let sampling = source.base_color_texture.map(|t| {
                textures.insert(
                    "baseColorTexture".into(),
                    RenderAssetReference {
                        expected_kind: RenderAssetKind::Texture,
                        reference: t.texture.0,
                    },
                );
                TextureSampler {
                    min_filter: t.min_filter,
                    mag_filter: t.mag_filter,
                    wrap_s: t.wrap_s,
                    wrap_t: t.wrap_t,
                }
            });
            RenderAssetData::Material(CookedMaterial {
                decal_bias: source.decal_bias,
                preset: source.preset,
                texture_sampling: Default::default(),
                parameters: BTreeMap::from([(
                    "baseColor".into(),
                    MaterialParameter::Vector4(source.base_color),
                )]),
                textures,
                base_color_texture_sampling: sampling,
                opaque_alpha: false,
                ignore_vertex_color: source.ignore_vertex_color,
                blend: source.blend,
                cull: source.cull,
                depth: source.depth,
                shader_source: UNLIT_SHADER_SOURCE.into(),
            })
        }
        _ => {
            return Err(SpatialContractError::new(
                "schema_unsupported",
                "/schemaVersion",
                "Expected generated Mesh or Unlit Material v2.",
            ))
        }
    };
    result
        .validate()
        .map_err(|e| SpatialContractError::new("asset_invalid", "/source", e.to_string()))?;
    Ok(result)
}

fn generate(primitive: GeneratedPrimitive) -> CookedGeometry {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    // u cross v is the outward normal. UV v=0 is the +v edge of each face.
    let mut face = |center: [f32; 3], u: [f32; 3], v: [f32; 3], normal: [f32; 3]| {
        let start = (vertices.len() / 32) as u16;
        for (su, sv, uv) in [
            (-1., -1., [0., 1.]),
            (1., -1., [1., 1.]),
            (1., 1., [1., 0.]),
            (-1., 1., [0., 0.]),
        ] {
            let p: [f32; 3] = std::array::from_fn(|i| center[i] + su * u[i] + sv * v[i]);
            for value in p.into_iter().chain(normal).chain(uv) {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
        }
        for i in [0, 1, 2, 0, 2, 3] {
            indices.extend_from_slice(&(start + i).to_le_bytes());
        }
    };
    let half = match primitive {
        GeneratedPrimitive::Cube { size } => {
            let [x, y, z] = size.map(|v| v * 0.5);
            face([0., 0., z], [x, 0., 0.], [0., y, 0.], [0., 0., 1.]);
            face([0., 0., -z], [-x, 0., 0.], [0., y, 0.], [0., 0., -1.]);
            face([x, 0., 0.], [0., 0., -z], [0., y, 0.], [1., 0., 0.]);
            face([-x, 0., 0.], [0., 0., z], [0., y, 0.], [-1., 0., 0.]);
            face([0., y, 0.], [x, 0., 0.], [0., 0., -z], [0., 1., 0.]);
            face([0., -y, 0.], [x, 0., 0.], [0., 0., z], [0., -1., 0.]);
            [x, y, z]
        }
        GeneratedPrimitive::Plane { size } => {
            let [x, z] = size.map(|v| v * 0.5);
            face([0.; 3], [x, 0., 0.], [0., 0., -z], [0., 1., 0.]);
            [x, 0., z]
        }
    };
    let index_count = (indices.len() / 2) as u32;
    CookedGeometry {
        vertex_count: (vertices.len() / 32) as u32,
        index_count,
        vertex_bytes: vertices,
        index_bytes: indices,
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
        index_format: GeometryIndexFormat::Uint16,
        topology: GeometryTopology::TriangleList,
        submeshes: vec![SubmeshRange {
            first_index: 0,
            index_count,
            material_slot: "surface".into(),
        }],
        bounds: GeometryBounds {
            min: half.map(|v| -v),
            max: half,
            sphere_center: [0.; 3],
            sphere_radius: half
                .iter()
                .map(|v| f64::from(*v).powi(2))
                .sum::<f64>()
                .sqrt() as f32,
        },
        coordinate_convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
        material_slots: vec!["surface".into()],
    }
}
