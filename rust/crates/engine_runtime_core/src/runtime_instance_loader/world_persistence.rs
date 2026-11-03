use super::*;
use crate::{
    archetype::ComponentValue,
    character_animation::{dynamic_json, runtime_json},
    components::ComponentTypeId,
    query::QuerySpec,
    world_persistence::*,
};
#[derive(Debug)]
struct ResultState {
    epoch: u64,
    generation: u64,
    request: u64,
    snapshot: Option<WorldLogicalSave3D>,
    error: Option<String>,
}
#[derive(Debug, Default)]
pub(super) struct PersistenceState {
    results: BTreeMap<SourceEntityId, ResultState>,
}
impl RuntimeInstanceLoader {
    pub(crate) fn persistence_state(
        &self,
        id: &SourceEntityId,
        world: &World,
    ) -> Option<ComponentValue> {
        let s = self.persistence.results.get(id)?;
        if s.epoch != world.epoch() || world.entity_generation(id) != Some(s.generation) {
            return None;
        }
        Some(ComponentValue::Dynamic {
            component_type: ComponentTypeId::new("engine.world_persistence_state"),
            value: runtime_json(
                serde_json::json!({"requestId":s.request,"snapshot":s.snapshot,"error":s.error}),
            ),
        })
    }
    pub fn advance_world_persistence(&mut self, world: &mut World) {
        self.persistence.results.retain(|id, s| {
            s.epoch == world.epoch()
                && world.entity_generation(id) == Some(s.generation)
                && world
                    .component_value(id, &ComponentTypeId::new("engine.world_persistence3d"))
                    .is_some()
        });
        let ids = world.query_entities(
            &QuerySpec::all([ComponentTypeId::new("engine.world_persistence3d")]).limit(9),
        );
        if ids.len() > 8 {
            return;
        }
        for id in ids {
            let Some(ComponentValue::Dynamic { value, .. }) =
                world.component_value(&id, &ComponentTypeId::new("engine.world_persistence3d"))
            else {
                continue;
            };
            let Ok(b) = serde_json::from_value::<WorldPersistence3D>(dynamic_json(&value)) else {
                continue;
            };
            if b.request_id == 0
                || self
                    .persistence
                    .results
                    .get(&id)
                    .is_some_and(|s| s.request >= b.request_id)
            {
                continue;
            }
            let result = validate_binding(&b).and_then(|_| {
                if let Some(save) = b.restore {
                    self.restore_partition_logical(
                        &b.declaration,
                        &serde_json::to_vec(&save).map_err(|e| invalid(e.to_string()))?,
                        world,
                    )?;
                }
                self.capture_partition_logical(&b.declaration, world)
            });
            let (snapshot, error) = match result {
                Ok(save) => (Some(save), None),
                Err(e) => (None, Some(e.to_string())),
            };
            self.persistence.results.insert(
                id.clone(),
                ResultState {
                    epoch: world.epoch(),
                    generation: world.entity_generation(&id).unwrap(),
                    request: b.request_id,
                    snapshot,
                    error,
                },
            );
        }
    }
}
