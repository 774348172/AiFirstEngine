use super::*;
use crate::spatial_contract::{CanonicalTransform3D, Rotation3D, SpatialAssetRef, Transform3D};
use project_runtime_sdk::ProjectRuntimeSpatialIntent;

pub(super) fn prepare_spatial(
    world: &World,
    overlay: &BTreeMap<(EntityId, ComponentTypeId), ComponentValue>,
    id: &EntityId,
    intent: ProjectRuntimeSpatialIntent,
) -> Result<Option<ComponentValue>, WorldApiError> {
    let invalid = || {
        WorldApiError::invalid_component_value(
            "spatial_intent",
            id.clone(),
            ComponentTypeId::transform3d(),
        )
    };
    if world.transform3d(id).is_none() {
        return Err(invalid());
    }
    if matches!(
        &intent,
        ProjectRuntimeSpatialIntent::SetLocalTransform { .. }
    ) {
        if let Some(crate::archetype::ComponentValue::Dynamic { value, .. }) =
            world.component_value(id, &ComponentTypeId::new("engine.body3d"))
        {
            let body: crate::physics_3d::Body3D =
                serde_json::from_value(crate::character_animation::dynamic_json(&value))
                    .map_err(|_| invalid())?;
            if body.enabled && body.motion == crate::physics_3d::BodyMotion3D::Dynamic {
                return Err(invalid());
            }
        }
    }
    Ok(Some(match intent {
        ProjectRuntimeSpatialIntent::WorldLogicalCapture { request_id } => {
            crate::world_persistence::prepare_request(world, overlay, id, request_id, None)
                .map_err(|_| invalid())?
        }
        ProjectRuntimeSpatialIntent::WorldLogicalRestore {
            request_id,
            snapshot,
        } => crate::world_persistence::prepare_request(
            world,
            overlay,
            id,
            request_id,
            Some(snapshot),
        )
        .map_err(|_| invalid())?,
        ProjectRuntimeSpatialIntent::WorldStreamingSources { sources } => {
            crate::world_streaming::prepare_sources(world, overlay, id, sources)
                .map_err(|_| invalid())?
        }
        control @ (ProjectRuntimeSpatialIntent::NavigationTarget { .. }
        | ProjectRuntimeSpatialIntent::NavigationCancel { .. }) => {
            crate::navigation_3d::prepare_control(world, overlay, id, control)
                .map_err(|_| invalid())?
        }
        control @ (ProjectRuntimeSpatialIntent::BodyVelocity { .. }
        | ProjectRuntimeSpatialIntent::BodyImpulse { .. }
        | ProjectRuntimeSpatialIntent::MotorMove { .. }
        | ProjectRuntimeSpatialIntent::MotorJump { .. }) => {
            crate::physics_3d::prepare_control(world, overlay, id, control)
                .map_err(|_| invalid())?
        }
        control @ (ProjectRuntimeSpatialIntent::CharacterParameter { .. }
        | ProjectRuntimeSpatialIntent::CharacterPlay { .. }
        | ProjectRuntimeSpatialIntent::CharacterSeek { .. }
        | ProjectRuntimeSpatialIntent::CharacterPaused { .. }
        | ProjectRuntimeSpatialIntent::CharacterMode { .. }) => {
            crate::character_animation::prepare_control(world, overlay, id, control)
                .map_err(|_| invalid())?
        }
        ProjectRuntimeSpatialIntent::SetLocalTransform { transform }
        | ProjectRuntimeSpatialIntent::Teleport { transform } => {
            let transform: CanonicalTransform3D = Transform3D {
                schema_version: "transform3d.v1".into(),
                translation: transform.translation,
                rotation: Rotation3D::Quaternion {
                    xyzw: transform.rotation_xyzw,
                },
                scale: transform.scale,
            }
            .canonicalize()
            .map_err(|_| invalid())?;
            ComponentValue::Transform3D(transform)
        }
        light_intent @ (ProjectRuntimeSpatialIntent::SetLightEnabled { .. }
        | ProjectRuntimeSpatialIntent::SetLightColor { .. }
        | ProjectRuntimeSpatialIntent::SetLightIntensity { .. }) => {
            let light_invalid = || {
                WorldApiError::invalid_component_value(
                    "spatial_light_intent",
                    id.clone(),
                    ComponentTypeId::light3d(),
                )
            };
            let Some(ComponentValue::Light3D(mut light)) =
                current_component(world, overlay, id, &ComponentTypeId::light3d())
            else {
                return Err(WorldApiError::missing_component(
                    "spatial_light_intent",
                    id.clone(),
                    ComponentTypeId::light3d(),
                ));
            };
            match light_intent {
                ProjectRuntimeSpatialIntent::SetLightEnabled { enabled } => light.enabled = enabled,
                ProjectRuntimeSpatialIntent::SetLightColor { color } => light.color = color,
                ProjectRuntimeSpatialIntent::SetLightIntensity { intensity } => {
                    match &mut light.kind {
                        crate::spatial_contract::LightKind::Directional { illuminance_lux } => {
                            *illuminance_lux = intensity
                        }
                        crate::spatial_contract::LightKind::Point {
                            intensity_candela, ..
                        }
                        | crate::spatial_contract::LightKind::Spot {
                            intensity_candela, ..
                        } => *intensity_candela = intensity,
                    }
                }
                _ => unreachable!(),
            }
            light.validate().map_err(|_| light_invalid())?;
            ComponentValue::Light3D(light)
        }
        ProjectRuntimeSpatialIntent::SetMeshVisible { visible } => {
            let Some(ComponentValue::MeshRenderer3D(mut mesh)) =
                current_component(world, overlay, id, &ComponentTypeId::mesh_renderer3d())
            else {
                return Err(invalid());
            };
            mesh.visible = visible;
            ComponentValue::MeshRenderer3D(mesh)
        }
        ProjectRuntimeSpatialIntent::SetMaterial { slot, material } => {
            let Some(ComponentValue::MeshRenderer3D(mut mesh)) =
                current_component(world, overlay, id, &ComponentTypeId::mesh_renderer3d())
            else {
                return Err(invalid());
            };
            if material.asset_type != "material"
                || material.id.trim().is_empty()
                || material.guid.as_ref().is_none_or(|v| v.trim().is_empty())
            {
                return Err(invalid());
            }
            let destination = mesh.materials.get_mut(&slot).ok_or_else(invalid)?;
            *destination = SpatialAssetRef(crate::runtime_package::RuntimeAssetRef {
                id: material.id,
                asset_type: material.asset_type,
                guid: material.guid,
                sub_asset: material.sub_asset,
            });
            ComponentValue::MeshRenderer3D(mesh)
        }
        ProjectRuntimeSpatialIntent::SetInterpolation { .. }
        | ProjectRuntimeSpatialIntent::Reparent { .. }
        | ProjectRuntimeSpatialIntent::SetWorldAnchor { .. } => return Ok(None),
        ProjectRuntimeSpatialIntent::SelectMainCamera {} => {
            return Ok(None);
        }
    }))
}
