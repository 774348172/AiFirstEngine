//! Declared character ownership. This is data projected into the existing
//! body/motor owners, not a second skeleton or physics registry.
use super::*;
use crate::{components::ComponentTypeId, ids::EntityId, world::World};
pub use project_runtime_sdk::ProjectRuntimeCharacterMode3D as CharacterMode3D;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RootMotion3D {
    pub joint: String,
    pub axes: [bool; 3],
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ragdoll3D {
    pub joints: Vec<String>,
}
pub(super) fn validate_modes(
    world: &World,
    id: &EntityId,
    binding: &CharacterBinding3D,
    asset: &CharacterAsset,
) -> ContractResult<()> {
    if let Some(root) = &binding.root_motion {
        let index = asset
            .joints
            .iter()
            .position(|j| j.key == root.joint)
            .ok_or_else(|| invalid("/rootMotion/joint", "Unknown root motion joint."))?;
        if asset.joints[index].parent.is_some()
            || !root.axes.iter().any(|v| *v)
            || world
                .component_value(id, &ComponentTypeId::new("engine.motor3d"))
                .is_none()
            || world
                .hierarchy(&EntityId::from(binding.joints[index].entity_ref.clone()))
                .and_then(|h| h.parent_id)
                .as_ref()
                != Some(id)
        {
            return Err(invalid(
                "/rootMotion",
                "Root motion requires a skeleton root directly under its motor owner.",
            ));
        }
    }
    if let Some(ragdoll) = &binding.ragdoll {
        if ragdoll.joints.len() < 2
            || ragdoll.joints.len() > 64
            || ragdoll
                .joints
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != ragdoll.joints.len()
        {
            return Err(invalid(
                "/ragdoll",
                "Ragdoll requires 2..64 unique connected declared joints.",
            ));
        }
        let mut roots = 0;
        for key in &ragdoll.joints {
            let i = asset
                .joints
                .iter()
                .position(|j| &j.key == key)
                .ok_or_else(|| invalid("/ragdoll/joint", "Unknown ragdoll joint."))?;
            let target = EntityId::from(binding.joints[i].entity_ref.clone());
            let body = body_data(world, &target, "engine.body3d")?;
            if body["motion"] != "dynamic" || body["enabled"] != false {
                return Err(invalid(
                    "/ragdoll/body",
                    "Ragdoll bones must declare initially disabled dynamic bodies.",
                ));
            }
            if let Some(parent) = asset.joints[i]
                .parent
                .filter(|p| ragdoll.joints.contains(&asset.joints[*p as usize].key))
            {
                let joint = body_data(world, &target, "engine.joint3d")?;
                if joint["other"]["entityRef"] != binding.joints[parent as usize].entity_ref {
                    return Err(invalid(
                        "/ragdoll/joint",
                        "Each ragdoll child needs a constraint to its declared skeleton parent.",
                    ));
                }
            } else {
                roots += 1;
            }
        }
        if roots != 1 {
            return Err(invalid(
                "/ragdoll/hierarchy",
                "Ragdoll subset must be connected.",
            ));
        }
    }
    Ok(())
}
fn body_data(world: &World, id: &EntityId, ty: &str) -> ContractResult<serde_json::Value> {
    match world.component_value(id, &ComponentTypeId::new(ty)) {
        Some(crate::archetype::ComponentValue::Dynamic { value, .. }) => Ok(dynamic_json(&value)),
        _ => Err(invalid("/ownership", format!("Missing {ty} on {id}."))),
    }
}
pub(super) fn apply_mode(
    world: &mut World,
    id: &EntityId,
    binding: &CharacterBinding3D,
    asset: &CharacterAsset,
    mode: CharacterMode3D,
) -> ContractResult<()> {
    if mode == CharacterMode3D::RootMotion && binding.root_motion.is_none() {
        return Err(invalid("/mode", "Root motion was not declared."));
    }
    if mode == CharacterMode3D::Ragdoll && binding.ragdoll.is_none() {
        return Err(invalid("/mode", "Ragdoll was not declared."));
    }
    let mut updates = Vec::new();
    if world
        .component_value(id, &ComponentTypeId::new("engine.motor3d"))
        .is_some()
    {
        let mut v = body_data(world, id, "engine.motor3d")?;
        v["enabled"] = (mode != CharacterMode3D::Ragdoll).into();
        updates.push((id.clone(), "engine.motor3d", v));
    }
    if let Some(ragdoll) = &binding.ragdoll {
        for key in &ragdoll.joints {
            let i = asset.joints.iter().position(|j| &j.key == key).unwrap();
            let target = EntityId::from(binding.joints[i].entity_ref.clone());
            let mut v = body_data(world, &target, "engine.body3d")?;
            v["enabled"] = (mode == CharacterMode3D::Ragdoll).into();
            updates.push((target, "engine.body3d", v));
        }
    }
    for (target, ty, v) in updates {
        world
            .try_insert_dynamic_component(target, ComponentTypeId::new(ty), runtime_json(v))
            .map_err(|e| invalid("/mode", e.message))?;
    }
    Ok(())
}
