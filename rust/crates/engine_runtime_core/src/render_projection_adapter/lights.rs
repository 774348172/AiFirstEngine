//! Lights use the existing typed projection adapter and camera packet.
use super::*;
use crate::spatial_contract::{CoordinateDomain, Light3D, LightKind, WorldPosition};
use crate::world::SpatialWorldState;
use std::sync::Arc;
#[derive(Debug, Clone, PartialEq)]
pub struct LightProjection {
    pub component: Light3D,
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub origin: [f64; 3],
}
pub struct Light3DAdapter<'a> {
    pub world: &'a SpatialWorldState,
    pub origin: [f64; 3],
}
impl RenderProjectionAdapter<Light3D> for Light3DAdapter<'_> {
    type Projection = LightProjection;
    fn project(&self, _source: &SourceEntityId, component: &Light3D) -> LightProjection {
        let [x, y, z, w] = self.world.pose.rotation;
        LightProjection {
            component: component.clone(),
            position: self.world.pose.position,
            direction: [
                -2. * (x * z + w * y),
                -2. * (y * z - w * x),
                -(1. - 2. * (x * x + y * y)),
            ],
            origin: self.origin,
        }
    }
}
pub(crate) fn project_lights(
    world: &crate::world::World,
    view: &CameraProjection,
) -> Result<Arc<Vec<LightProjection>>, ContractError> {
    let mut result = Vec::new();
    // Query only light archetypes, never scan every mesh or create a second registry.
    for id in world.query_entities(&crate::query::QuerySpec::all([
        crate::components::ComponentTypeId::light3d(),
    ])) {
        let Some(light) = world.light3d(&id).filter(|l| l.enabled) else {
            continue;
        };
        let Some(state) = world
            .spatial_presentation(&id)
            .filter(|s| s.enabled && s.authored_visible && s.inherited_visible)
        else {
            continue;
        };
        if let LayerSelection::Named { names } = &view.view.layers {
            if !light.layers.iter().any(|l| names.contains(l)) {
                continue;
            }
        }
        light
            .validate()
            .map_err(|_| ContractError::new(ContractErrorCode::InvalidBinding, "light.value"))?;
        if result.len() == 256 {
            return Err(ContractError::new(
                ContractErrorCode::InvalidBinding,
                "light.view_capacity",
            ));
        }
        let mut projected = Light3DAdapter {
            world: state,
            origin: world.spatial_presentation_origin(&id).ok_or_else(|| {
                ContractError::new(ContractErrorCode::InvalidProjection, "light.origin")
            })?,
        }
        .project(&id, light);
        if !matches!(light.kind, LightKind::Directional { .. }) {
            projected.position = WorldPosition::new(projected.origin, projected.position)
                .and_then(|p| {
                    p.relative_to(
                        WorldPosition::new(view.origin, [0.; 3])?,
                        CoordinateDomain::new(1e12, 32768., 32768., 0.005)?,
                    )
                })
                .map_err(|_| {
                    ContractError::new(
                        ContractErrorCode::InvalidProjection,
                        "light.relative_position",
                    )
                })?;
        } else {
            projected.position = [0.; 3];
        }
        projected.origin = view.origin;
        result.push(projected);
    }
    Ok(Arc::new(result))
}
