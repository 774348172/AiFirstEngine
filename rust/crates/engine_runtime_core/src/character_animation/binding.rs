use super::*;
use crate::spatial_contract::SpatialAssetRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterJointTarget {
    pub entity_ref: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBinding3D {
    pub schema_version: String,
    pub character: SpatialAssetRef,
    pub joints: Vec<CharacterJointTarget>,
    #[serde(default)]
    pub graph: Option<AnimationGraph3D>,
    #[serde(default)]
    pub play_on_awake: bool,
    #[serde(default)]
    pub retarget: Option<CharacterRetargetBinding3D>,
    #[serde(default)]
    pub simulation_ik: Vec<PoseIk3D>,
    #[serde(default)]
    pub visual_ik: Vec<PoseIk3D>,
    #[serde(default = "default_visual_interval")]
    pub visual_interval: u32,
    #[serde(default)]
    pub visual_max_distance: Option<f64>,
    #[serde(default)]
    pub root_motion: Option<RootMotion3D>,
    #[serde(default)]
    pub ragdoll: Option<Ragdoll3D>,
    #[serde(default)]
    pub initial_mode: CharacterMode3D,
}
fn default_visual_interval() -> u32 {
    1
}
impl CharacterBinding3D {
    pub fn validate(&self) -> ContractResult<()> {
        if self.visual_interval == 0
            || self.visual_interval > 120
            || self.simulation_ik.len() > 32
            || self.visual_ik.len() > 32
            || self
                .visual_max_distance
                .is_some_and(|d| !d.is_finite() || d <= 0.)
        {
            return Err(invalid(
                "/binding/lod",
                "Invalid visual interval/distance or IK budget.",
            ));
        }
        if self.schema_version != "character3d-binding.v1"
            || self.character.0.asset_type != "character3d"
            || self.character.0.id.is_empty()
            || self.joints.is_empty()
            || self.joints.len() > MAX_JOINTS
            || self.joints.iter().any(|j| j.entity_ref.trim().is_empty())
            || self
                .joints
                .iter()
                .map(|j| &j.entity_ref)
                .collect::<BTreeSet<_>>()
                .len()
                != self.joints.len()
        {
            return Err(invalid("/binding","Character binding requires a character AssetRef and unique ordered entity targets."));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CharacterControl3D {
    pub revision: u64,
    pub parameters: std::collections::BTreeMap<String, f32>,
    pub paused: Option<bool>,
    pub seek: Option<(u64, f64)>,
    pub play: Option<(u64, String, bool)>,
    #[serde(default)]
    pub mode: Option<CharacterMode3D>,
}

// The typed deferred owner creates this private mailbox on the first character intent.
// Ordinary replacement still requires an existing component.
pub(crate) fn permits_control_bootstrap(
    world: &crate::world::World,
    id: &crate::ids::EntityId,
    value: &crate::archetype::ComponentValue,
) -> bool {
    use crate::{archetype::ComponentValue as C, components::ComponentTypeId as T};
    matches!(value,C::Dynamic{component_type,..} if component_type==&T::new("engine.character3d_control"))
        && world
            .component_value(id, &T::new("engine.character3d"))
            .is_some()
        && match value {
            C::Dynamic { value, .. } => {
                serde_json::from_value::<CharacterControl3D>(dynamic_json(value)).is_ok()
            }
            _ => false,
        }
}

pub(crate) fn dynamic_json(value: &crate::component_value::RuntimeValue) -> serde_json::Value {
    use crate::component_value::RuntimeValue as V;
    match value {
        V::Null => serde_json::Value::Null,
        V::Bool(v) => (*v).into(),
        V::I64(v) => (*v).into(),
        V::F64(v) => serde_json::json!(v),
        V::String(v) | V::AssetRef(v) => v.clone().into(),
        V::EntityRef(v) => v.to_string().into(),
        V::Object(v) => v
            .iter()
            .map(|(k, v)| (k.clone(), dynamic_json(v)))
            .collect(),
        V::Array(v) => v.iter().map(dynamic_json).collect(),
        V::Vec2 { x, y } => serde_json::json!([x, y]),
        V::Vec3(v) => serde_json::json!([v.x, v.y, v.z]),
        V::Color { r, g, b, a } => serde_json::json!([r, g, b, a]),
    }
}
pub(crate) fn runtime_json(value: serde_json::Value) -> crate::component_value::RuntimeValue {
    use crate::component_value::RuntimeValue as V;
    match value {
        serde_json::Value::Null => V::Null,
        serde_json::Value::Bool(v) => V::Bool(v),
        serde_json::Value::String(v) => V::String(v),
        serde_json::Value::Number(v) => v
            .as_i64()
            .map(V::I64)
            .unwrap_or_else(|| V::F64(v.as_f64().unwrap())),
        serde_json::Value::Object(v) => {
            V::Object(v.into_iter().map(|(k, v)| (k, runtime_json(v))).collect())
        }
        serde_json::Value::Array(v) => V::Array(v.into_iter().map(runtime_json).collect()),
    }
}

pub(crate) fn prepare_control(
    world: &crate::world::World,
    overlay: &std::collections::BTreeMap<
        (crate::ids::EntityId, crate::components::ComponentTypeId),
        crate::archetype::ComponentValue,
    >,
    id: &crate::ids::EntityId,
    intent: project_runtime_sdk::ProjectRuntimeSpatialIntent,
) -> ContractResult<crate::archetype::ComponentValue> {
    use crate::{archetype::ComponentValue as C, components::ComponentTypeId as T};
    use project_runtime_sdk::ProjectRuntimeSpatialIntent as I;
    let read = |ty: &str| {
        overlay
            .get(&(id.clone(), T::new(ty)))
            .cloned()
            .or_else(|| world.component_value(id, &T::new(ty)))
    };
    let Some(C::Dynamic { value, .. }) = read("engine.character3d") else {
        return Err(invalid("/binding", "Target has no character binding."));
    };
    let binding: CharacterBinding3D = serde_json::from_value(dynamic_json(&value))
        .map_err(|e| invalid("/binding", e.to_string()))?;
    binding.validate()?;
    let mut control: CharacterControl3D = match read("engine.character3d_control") {
        Some(C::Dynamic { value, .. }) => serde_json::from_value(dynamic_json(&value))
            .map_err(|e| invalid("/control", e.to_string()))?,
        _ => Default::default(),
    };
    control.revision = control
        .revision
        .checked_add(1)
        .ok_or_else(|| invalid("/revision", "Control sequence exhausted."))?;
    match intent {
        I::CharacterParameter { name, value } => {
            if !value.is_finite()
                || !binding
                    .graph
                    .as_ref()
                    .is_some_and(|g| g.parameters.contains_key(&name))
            {
                return Err(invalid("/parameter", "Unknown or invalid graph parameter."));
            }
            control.parameters.insert(name, value);
        }
        I::CharacterPlay { state, reset } => {
            if state.is_empty()
                || binding
                    .graph
                    .as_ref()
                    .is_some_and(|g| !g.states.iter().any(|s| s.key == state))
            {
                return Err(invalid("/state", "Undeclared state."));
            }
            control.play = Some((control.revision, state, reset));
            control.paused = Some(false);
        }
        I::CharacterSeek { seconds } => {
            if !seconds.is_finite() || seconds < 0. {
                return Err(invalid("/seek", "Invalid seek time."));
            }
            control.seek = Some((control.revision, seconds));
        }
        I::CharacterPaused { paused } => control.paused = Some(paused),
        I::CharacterMode { mode } => {
            if mode == CharacterMode3D::RootMotion && binding.root_motion.is_none()
                || mode == CharacterMode3D::Ragdoll && binding.ragdoll.is_none()
            {
                return Err(invalid(
                    "/mode",
                    "Character mode was not declared in its binding.",
                ));
            }
            control.mode = Some(mode);
        }
        _ => return Err(invalid("/operation", "Expected character intent.")),
    }
    Ok(C::Dynamic {
        component_type: T::new("engine.character3d_control"),
        value: runtime_json(serde_json::to_value(control).unwrap()),
    })
}
