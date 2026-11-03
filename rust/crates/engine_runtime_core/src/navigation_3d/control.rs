use super::*;
use crate::{
    archetype::ComponentValue,
    character_animation::{dynamic_json, runtime_json},
    components::ComponentTypeId,
    ids::EntityId,
    spatial_contract::ContractResult,
    world::World,
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NavigationControl3D {
    pub revision: u64,
    pub target: Option<[f64; 3]>,
    pub deadline_ticks: u32,
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
    matches!(value,ComponentValue::Dynamic{component_type,value} if component_type==&ComponentTypeId::new("engine.navigation3d_control")&&serde_json::from_value::<NavigationControl3D>(dynamic_json(value)).is_ok())
        && component(world, id, "engine.navigation_agent3d").is_some()
}
pub(crate) fn prepare_control(
    world: &World,
    overlay: &BTreeMap<(EntityId, ComponentTypeId), ComponentValue>,
    id: &EntityId,
    intent: project_runtime_sdk::ProjectRuntimeSpatialIntent,
) -> ContractResult<ComponentValue> {
    let read = |ty: &str| match overlay.get(&(id.clone(), ComponentTypeId::new(ty))) {
        Some(ComponentValue::Dynamic { value, .. }) => Some(dynamic_json(value)),
        _ => component(world, id, ty),
    };
    let agent: NavigationAgent3D = serde_json::from_value(
        read("engine.navigation_agent3d")
            .ok_or_else(|| contract::invalid("/agent", "Navigation agent required."))?,
    )
    .map_err(|e| contract::invalid("/agent", e.to_string()))?;
    agent.validate()?;
    if component(world, id, "engine.motor3d").is_none() {
        return Err(contract::invalid(
            "/motor",
            "Navigation agent requires the accepted-displacement motor.",
        ));
    }
    let mut control = read("engine.navigation3d_control")
        .map(serde_json::from_value::<NavigationControl3D>)
        .transpose()
        .map_err(|e| contract::invalid("/control", e.to_string()))?
        .unwrap_or_default();
    control.revision = control
        .revision
        .checked_add(1)
        .ok_or_else(|| contract::invalid("/revision", "Navigation revision exhausted."))?;
    match intent {
        project_runtime_sdk::ProjectRuntimeSpatialIntent::NavigationTarget {
            target,
            deadline_ticks,
        } => {
            if target.iter().any(|v| !v.is_finite() || v.abs() > 1e12)
                || !(1..=600).contains(&deadline_ticks)
            {
                return Err(contract::invalid(
                    "/goal",
                    "Finite world target and deadline 1..600 ticks required.",
                ));
            }
            control.target = Some(target);
            control.deadline_ticks = deadline_ticks;
        }
        project_runtime_sdk::ProjectRuntimeSpatialIntent::NavigationCancel {} => {
            control.target = None
        }
        _ => return Err(contract::invalid("/intent", "Expected navigation intent.")),
    }
    Ok(ComponentValue::Dynamic {
        component_type: ComponentTypeId::new("engine.navigation3d_control"),
        value: runtime_json(serde_json::to_value(control).unwrap()),
    })
}
