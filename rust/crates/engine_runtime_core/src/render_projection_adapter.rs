#[cfg_attr(test, path = "render_projection_adapter/lights.rs")]
mod lights;
#[cfg(feature = "spine")]
#[cfg_attr(test, path = "render_projection_adapter/spine.rs")]
pub mod spine;
pub(crate) use lights::project_lights;
pub use lights::{Light3DAdapter, LightProjection};
#[cfg_attr(test, path = "render_projection_adapter/spatial.rs")]
mod spatial;
pub use spatial::*;
// Sprite's adapter inside the existing World -> RenderProjection owner.
// No extra dirty scanner, proxy registry or renderer state is maintained here.

use crate::components::SpriteRenderer2D;
use crate::ids::SourceEntityId;
use crate::render_contract::*;
use crate::render_state::{RenderProxyDescriptor, RenderProxyPayload, SpritePayload};
use serde::{Deserialize, Serialize};

pub trait RenderProjectionAdapter<T> {
    type Projection;
    fn project(&self, source: &SourceEntityId, component: &T) -> Self::Projection;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SpriteGeometry {
    /// These are unresolved keys from the legacy component, NOT fabricated typed assets.
    LegacyGeneratedQuad {
        sprite_ref: Option<String>,
        color: [f32; 4],
        flip_x: bool,
        flip_y: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteProjection {
    pub domain: RenderDomain,
    pub geometry: SpriteGeometry,
    pub legacy_material_ref: Option<String>,
    pub visibility: Visibility,
    pub layers: LayerSelection,
    pub policy: RenderPolicy,
    pub provenance: Provenance,
}

impl SpriteProjection {
    pub fn from_legacy(
        descriptor: &RenderProxyDescriptor,
        provenance: Provenance,
    ) -> Result<Self, ContractError> {
        let RenderProxyPayload::Sprite(payload) = &descriptor.payload else {
            return Err(ContractError::new(
                ContractErrorCode::UnsupportedLegacyConversion,
                "payload",
            )
            .at_stage(ProducerStage::RenderProjection));
        };
        Ok(Self {
            domain: RenderDomain::World2d,
            geometry: SpriteGeometry::LegacyGeneratedQuad {
                sprite_ref: payload.sprite_ref.clone(),
                color: payload.color,
                flip_x: payload.flip_x,
                flip_y: payload.flip_y,
            },
            legacy_material_ref: payload.material_ref.clone(),
            visibility: Visibility {
                authored: descriptor.visible,
                inherited: Evaluation::NotEvaluated,
            },
            layers: LayerSelection::Legacy {
                mask: Some(descriptor.layer.clone()),
            },
            policy: RenderPolicy::LegacySprite {
                sorting_layer: payload.sorting_layer,
                order_in_layer: payload.order_in_layer,
                sort_z: payload.sort_z,
            },
            provenance,
        })
    }

    /// Checks losslessness, not new-input numeric validity. Old values (including
    /// missing/empty asset keys) must continue to reach their existing consumers.
    pub fn to_legacy(&self) -> Result<LegacyConversion<'_, RenderProxyDescriptor>, ContractError> {
        let fail = |field| {
            ContractError::new(ContractErrorCode::UnsupportedLegacyConversion, field)
                .at_stage(ProducerStage::RenderProjection)
        };
        if self.domain != RenderDomain::World2d {
            return Err(fail("domain"));
        }
        if !matches!(self.visibility.inherited, Evaluation::NotEvaluated) {
            return Err(fail("visibility.inherited"));
        }
        let LayerSelection::Legacy { mask: Some(layer) } = &self.layers else {
            return Err(fail("layers"));
        };
        let RenderPolicy::LegacySprite {
            sorting_layer,
            order_in_layer,
            sort_z,
        } = self.policy
        else {
            return Err(fail("policy"));
        };
        let SpriteGeometry::LegacyGeneratedQuad {
            sprite_ref,
            color,
            flip_x,
            flip_y,
        } = &self.geometry;
        Ok(LegacyConversion {
            payload: RenderProxyDescriptor {
                visible: self.visibility.authored,
                layer: layer.clone(),
                payload: RenderProxyPayload::Sprite(SpritePayload {
                    sprite_ref: sprite_ref.clone(),
                    material_ref: self.legacy_material_ref.clone(),
                    color: *color,
                    flip_x: *flip_x,
                    flip_y: *flip_y,
                    sorting_layer,
                    order_in_layer,
                    sort_z,
                }),
            },
            provenance: &self.provenance,
        })
    }
}

impl ValidateRenderContract for SpriteProjection {
    fn validate(&self) -> Result<(), ContractError> {
        if self.domain != RenderDomain::World2d
            || !matches!(self.policy, RenderPolicy::LegacySprite { .. })
        {
            return Err(ContractError::new(
                ContractErrorCode::InvalidDomain,
                "domain/policy",
            ));
        }
        let SpriteGeometry::LegacyGeneratedQuad { color, .. } = &self.geometry;
        if color.iter().any(|v| !v.is_finite()) {
            return Err(ContractError::new(
                ContractErrorCode::NonFinite,
                "geometry.color",
            ));
        }
        self.layers.validate()?;
        self.policy.validate()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpriteRenderer2DAdapter;

impl RenderProjectionAdapter<SpriteRenderer2D> for SpriteRenderer2DAdapter {
    type Projection = SpriteProjection;

    fn project(&self, source: &SourceEntityId, component: &SpriteRenderer2D) -> Self::Projection {
        SpriteProjection {
            domain: RenderDomain::World2d,
            geometry: SpriteGeometry::LegacyGeneratedQuad {
                sprite_ref: component.sprite_ref.clone(),
                color: component.color,
                flip_x: component.flip_x,
                flip_y: component.flip_y,
            },
            legacy_material_ref: component.material_ref.clone(),
            visibility: Visibility {
                authored: component.visible,
                inherited: Evaluation::NotEvaluated,
            },
            layers: LayerSelection::Legacy {
                mask: Some("sprite2d".into()),
            },
            policy: RenderPolicy::LegacySprite {
                sorting_layer: component.sorting_layer,
                order_in_layer: component.order_in_layer,
                sort_z: component.sort_z,
            },
            provenance: Provenance::legacy_sprite(source.as_str()),
        }
    }
}
