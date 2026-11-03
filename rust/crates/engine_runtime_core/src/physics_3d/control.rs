use super::*;
use crate::character_animation::{dynamic_json, runtime_json};
use crate::spatial_contract::ContractResult;
use crate::{archetype::ComponentValue, components::ComponentTypeId, ids::EntityId, world::World};
use project_runtime_sdk::ProjectRuntimeSpatialIntent as Intent;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PhysicsControl3D {
    pub revision: u64,
    pub velocity: Option<[f32; 3]>,
    pub impulse: Option<(u64, [f32; 3])>,
    pub movement: Option<(u64, [f32; 3])>,
    pub jump: Option<(u64, f32)>,
}
pub(super) fn component(world: &World, id: &EntityId, ty: &str) -> Option<serde_json::Value> {
    match world.component_value(id, &ComponentTypeId::new(ty)) {
        Some(ComponentValue::Dynamic { value, .. }) => Some(dynamic_json(&value)),
        _ => None,
    }
}
pub(crate) fn permits_control_bootstrap(
    world: &World,
    id: &EntityId,
    value: &ComponentValue,
) -> bool {
    matches!(value,ComponentValue::Dynamic{component_type,value} if component_type==&ComponentTypeId::new("engine.physics3d_control") && serde_json::from_value::<PhysicsControl3D>(dynamic_json(value)).is_ok())
        && (component(world, id, "engine.body3d").is_some()
            || component(world, id, "engine.motor3d").is_some())
}
pub(crate) fn prepare_control(
    world: &World,
    overlay: &BTreeMap<(EntityId, ComponentTypeId), ComponentValue>,
    id: &EntityId,
    intent: Intent,
) -> ContractResult<ComponentValue> {
    let read = |ty: &str| match overlay.get(&(id.clone(), ComponentTypeId::new(ty))) {
        Some(ComponentValue::Dynamic { value, .. }) => Some(dynamic_json(value)),
        _ => component(world, id, ty),
    };
    let body = read("engine.body3d")
        .map(serde_json::from_value::<Body3D>)
        .transpose()
        .map_err(|e| invalid("/body", e.to_string()))?;
    let motor = read("engine.motor3d")
        .map(serde_json::from_value::<CharacterMotor3D>)
        .transpose()
        .map_err(|e| invalid("/motor", e.to_string()))?;
    if let Some(body) = &body {
        body.validate()?;
    }
    if let Some(motor) = &motor {
        motor.validate()?;
    }
    let mut control = read("engine.physics3d_control")
        .map(serde_json::from_value::<PhysicsControl3D>)
        .transpose()
        .map_err(|e| invalid("/control", e.to_string()))?
        .unwrap_or_default();
    control.revision = control
        .revision
        .checked_add(1)
        .ok_or_else(|| invalid("/revision", "Physics command sequence exhausted."))?;
    let vector_ok = |v: &[f32; 3]| {
        v.iter().all(|v| v.is_finite()) && v.iter().map(|v| v * v).sum::<f32>() <= 1_000_000.
    };
    match intent {
        Intent::BodyVelocity { velocity } => {
            if body
                .as_ref()
                .is_none_or(|b| b.motion == BodyMotion3D::Static)
                || !vector_ok(&velocity)
            {
                return Err(invalid(
                    "/velocity",
                    "Moving body and finite velocity required.",
                ));
            }
            control.velocity = Some(velocity);
        }
        Intent::BodyImpulse { impulse } => {
            if body
                .as_ref()
                .is_none_or(|b| b.motion != BodyMotion3D::Dynamic)
                || !vector_ok(&impulse)
            {
                return Err(invalid(
                    "/impulse",
                    "Dynamic body and finite impulse required.",
                ));
            }
            control.impulse = Some((control.revision, impulse));
        }
        Intent::MotorMove { displacement } => {
            if motor.is_none() || !vector_ok(&displacement) {
                return Err(invalid(
                    "/movement",
                    "Character motor and finite displacement required.",
                ));
            }
            control.movement = Some((control.revision, displacement));
        }
        Intent::MotorJump { speed } => {
            if motor.is_none() || !speed.is_finite() || !(0.0..=100.).contains(&speed) {
                return Err(invalid(
                    "/jump",
                    "Character motor and finite jump speed required.",
                ));
            }
            control.jump = Some((control.revision, speed));
        }
        _ => return Err(invalid("/intent", "Expected Physics3D command.")),
    }
    Ok(ComponentValue::Dynamic {
        component_type: ComponentTypeId::new("engine.physics3d_control"),
        value: runtime_json(serde_json::to_value(control).unwrap()),
    })
}

pub(crate) fn project_transform_owned(world: &World, id: &EntityId) -> bool {
    component(world, id, "engine.body3d")
        .is_some_and(|v| v["motion"] == "dynamic" && v["enabled"] != false)
}
