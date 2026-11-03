//! Persistent Physics3D projection. Solver identities stay private to this owner.
mod contract;
use contract::invalid;
pub use contract::*;
#[cfg(not(target_arch = "wasm32"))]
mod session;
#[cfg(not(target_arch = "wasm32"))]
pub use session::PhysicsModule;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use session::{ActivatedPhysics3D, PreparedPhysics3D};
#[cfg(all(not(target_arch = "wasm32"), feature = "real-wgpu"))]
pub(crate) use session::{GeneratedStaticBinding, PreparedStaticCollision};
mod control;
mod query;
pub(crate) use control::{permits_control_bootstrap, prepare_control, project_transform_owned};

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsBodyState3D {
    pub generation: u64,
    pub position: [f64; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub sleeping: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsContact3D {
    pub entity1: String,
    pub generation1: u64,
    pub entity2: String,
    pub generation2: u64,
    pub phase: String,
    pub sensor: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsFrame3D {
    pub tick: u64,
    pub bodies: BTreeMap<String, PhysicsBodyState3D>,
    pub motors: BTreeMap<String, MotorResult3D>,
    pub contacts: Vec<PhysicsContact3D>,
}
impl crate::world::World {
    pub fn physics3d_frame(&self) -> Arc<PhysicsFrame3D> {
        self.physics3d_frame.clone()
    }
}

/// Read-only project projection, generated only when requested. Native solver
/// handles and the full shared frame never cross the project ABI.
pub(crate) fn read_state(
    world: &crate::world::World,
    id: &crate::ids::EntityId,
) -> Option<crate::archetype::ComponentValue> {
    let generation = world.entity_generation(id)?;
    let frame = &world.physics3d_frame;
    let body = frame
        .bodies
        .get(id.as_str())
        .filter(|b| b.generation == generation);
    let motor = frame
        .motors
        .get(id.as_str())
        .filter(|m| m.generation == generation);
    if body.is_none() && motor.is_none() {
        return None;
    }
    let contacts=frame.contacts.iter().filter_map(|c|{
        if c.entity1==id.as_str()&&c.generation1==generation{Some(serde_json::json!({"other":c.entity2,"generation":c.generation2,"phase":c.phase,"sensor":c.sensor}))}
        else if c.entity2==id.as_str()&&c.generation2==generation{Some(serde_json::json!({"other":c.entity1,"generation":c.generation1,"phase":c.phase,"sensor":c.sensor}))}else{None}
    }).collect::<Vec<_>>();
    Some(crate::archetype::ComponentValue::Dynamic {
        component_type: crate::components::ComponentTypeId::new("engine.physics3d_state"),
        value: crate::character_animation::runtime_json(
            serde_json::json!({"schemaVersion":"physics3d-state.v1","tick":frame.tick,"generation":generation,"body":body,"motor":motor,"contacts":contacts}),
        ),
    })
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Default)]
pub struct PhysicsModule;
#[cfg(target_arch = "wasm32")]
impl PhysicsModule {
    pub fn advance_with_assets(
        &mut self,
        world: &mut crate::world::World,
        _loader: &mut crate::runtime_asset_loader::RuntimeAssetLoader,
        tick: u64,
        dt: f32,
    ) -> crate::spatial_contract::ContractResult<()> {
        self.advance(world, tick, dt)
    }
    pub fn advance(
        &mut self,
        world: &mut crate::world::World,
        _tick: u64,
        _dt: f32,
    ) -> crate::spatial_contract::ContractResult<()> {
        use crate::{components::ComponentTypeId, query::QuerySpec};
        if !world
            .query_entities(&QuerySpec::all([ComponentTypeId::new("engine.body3d")]))
            .is_empty()
            || !world
                .query_entities(&QuerySpec::all([ComponentTypeId::new("engine.motor3d")]))
                .is_empty()
        {
            return Err(invalid(
                "/backend",
                "Jolt Physics3D is unavailable on wasm32.",
            ));
        }
        Ok(())
    }
}
