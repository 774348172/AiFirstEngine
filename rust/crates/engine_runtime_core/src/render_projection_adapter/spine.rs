//! Spine's adapter in the existing Projection domain; output is ordinary neutral geometry.
use super::*;
use engine_spine_runtime::{BlendMode, Geometry};

pub struct SpineGeometrySource<'a> {
    pub geometry: &'a Geometry,
    /// Package AssetRefs in the atlas's declared order, already linear/PMA cooked.
    pub pages: &'a [crate::spine_animation::SpinePage],
}
#[derive(Debug, Clone)]
pub struct SpineProjectedBatch {
    pub draw_order: u32,
    pub texture_asset_id: String,
    pub geometry: CookedGeometry,
    pub material: CookedMaterial,
    pub provenance: Provenance,
}
pub struct SpineRendererAdapter;
impl RenderProjectionAdapter<SpineGeometrySource<'_>> for SpineRendererAdapter {
    type Projection = Result<Vec<SpineProjectedBatch>, ContractError>;
    fn project(
        &self,
        source: &SourceEntityId,
        input: &SpineGeometrySource<'_>,
    ) -> Self::Projection {
        let g = input.geometry;
        let invalid = || {
            ContractError::new(ContractErrorCode::InvalidBinding, "spine.geometry")
                .at_stage(ProducerStage::RenderProjection)
        };
        if g.vertices.len() > 32768
            || g.indices.len() > 98304
            || g.batches.len() > 256
            || g.indices.iter().any(|&i| i as usize >= g.vertices.len())
        {
            return Err(invalid());
        }
        if g.batches.is_empty() {
            return if g.indices.is_empty() {
                Ok(Vec::new())
            } else {
                Err(invalid())
            };
        }
        let bounds = g.bounds.ok_or_else(invalid)?;
        if bounds.iter().any(|v| !v.is_finite()) || bounds[0] > bounds[2] || bounds[1] > bounds[3] {
            return Err(invalid());
        }
        let mut vertex_bytes = Vec::with_capacity(g.vertices.len() * 36);
        for v in &g.vertices {
            let values = [v.x, v.y, 0.0, v.u, v.v, v.r, v.g, v.b, v.a];
            if values.iter().any(|v| !v.is_finite()) || !(0.0..=1.0).contains(&v.a) {
                return Err(invalid());
            }
            for value in values {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let center = [
            (bounds[0] + bounds[2]) * 0.5,
            (bounds[1] + bounds[3]) * 0.5,
            0.0,
        ];
        let radius = ((bounds[2] - center[0]).powi(2) + (bounds[3] - center[1]).powi(2)).sqrt();
        let mut consumed = 0u32;
        let mut result = Vec::with_capacity(g.batches.len());
        for (order, b) in g.batches.iter().enumerate() {
            if b.first_index != consumed || b.index_count == 0 || b.index_count % 3 != 0 {
                return Err(invalid());
            }
            consumed = consumed.checked_add(b.index_count).ok_or_else(invalid)?;
            let indices = g
                .indices
                .get(b.first_index as usize..consumed as usize)
                .ok_or_else(invalid)?;
            let page = input.pages.get(b.page as usize).ok_or_else(invalid)?;
            let geometry = CookedGeometry {
                vertex_bytes: vertex_bytes.clone(),
                index_bytes: indices.iter().flat_map(|i| i.to_le_bytes()).collect(),
                vertex_stride: 36,
                vertex_count: g.vertices.len() as u32,
                index_count: b.index_count,
                attributes: vec![
                    VertexAttribute {
                        semantic: VertexSemantic::Position,
                        format: VertexFormat::Float32x3,
                        offset: 0,
                    },
                    VertexAttribute {
                        semantic: VertexSemantic::TexCoord0,
                        format: VertexFormat::Float32x2,
                        offset: 12,
                    },
                    VertexAttribute {
                        semantic: VertexSemantic::Color0,
                        format: VertexFormat::Float32x4,
                        offset: 20,
                    },
                ],
                index_format: GeometryIndexFormat::Uint32,
                topology: GeometryTopology::TriangleList,
                submeshes: vec![SubmeshRange {
                    first_index: 0,
                    index_count: b.index_count,
                    material_slot: "surface".into(),
                }],
                bounds: GeometryBounds {
                    min: [bounds[0], bounds[1], 0.0],
                    max: [bounds[2], bounds[3], 0.0],
                    sphere_center: center,
                    sphere_radius: radius,
                },
                coordinate_convention: CoordinateConvention::Legacy2d,
                material_slots: vec!["surface".into()],
            };
            let material = CookedMaterial {
                decal_bias: None,
                preset: MaterialPreset::Unlit,
                texture_sampling: Default::default(),
                parameters: std::collections::BTreeMap::from([(
                    "baseColor".into(),
                    MaterialParameter::Vector4([1.0; 4]),
                )]),
                textures: std::collections::BTreeMap::from([(
                    "baseColorTexture".into(),
                    RenderAssetReference {
                        reference: crate::runtime_package::RuntimeAssetRef {
                            id: page.texture_asset_id.clone(),
                            asset_type: "texture".into(),
                            guid: Some(page.texture_asset_id.clone()),
                            sub_asset: None,
                        },
                        expected_kind: RenderAssetKind::Texture,
                    },
                )]),
                base_color_texture_sampling: None,
                opaque_alpha: false,
                ignore_vertex_color: false,
                blend: match b.blend {
                    BlendMode::Normal => MaterialBlend::PremultipliedAlpha,
                    BlendMode::Additive => MaterialBlend::PremultipliedAdditive,
                    BlendMode::Multiply => MaterialBlend::PremultipliedMultiply,
                    BlendMode::Screen => MaterialBlend::PremultipliedScreen,
                },
                cull: MaterialCull::None,
                depth: MaterialDepth::Disabled,
                shader_source: UNLIT_SHADER_SOURCE.into(),
            };
            geometry.validate()?;
            material.validate()?;
            result.push(SpineProjectedBatch {
                draw_order: order as u32,
                texture_asset_id: page.texture_asset_id.clone(),
                geometry,
                material,
                provenance: Provenance {
                    source_entity_id: Some(source.as_str().into()),
                    source_component: Some("SpineAnimator".into()),
                    ..Provenance::unspecified(ProducerStage::RenderProjection)
                },
            });
        }
        if consumed as usize != g.indices.len() {
            return Err(invalid());
        }
        Ok(result)
    }
}
