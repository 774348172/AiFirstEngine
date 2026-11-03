//! Bounded snapshot planner feeding the original loading/prepare/upload/commit owner.
#[cfg(test)]
#[path = "world_streaming_failure_tests.rs"]
mod tests;
use super::*;
use crate::world_streaming::{StreamingPlan3D, WorldStreaming3D};
use crate::{archetype::ComponentValue, components::ComponentTypeId, query::QuerySpec};
use crate::{
    navigation_3d::NavigationModule, physics_3d::PhysicsModule,
    render_resource::RenderResourceManager,
};
use std::{collections::BTreeSet, sync::mpsc, thread};

struct Work {
    owner: SourceEntityId,
    generation: u64,
    epoch: u64,
    stamp: String,
    binding: WorldStreaming3D,
    resident: BTreeSet<String>,
}
struct ResultPlan {
    owner: SourceEntityId,
    generation: u64,
    epoch: u64,
    stamp: String,
    result: Result<StreamingPlan3D, String>,
}
struct Worker {
    tx: Option<mpsc::SyncSender<Work>>,
    rx: Option<mpsc::Receiver<ResultPlan>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    fn new() -> std::io::Result<Self> {
        let (tx, input) = mpsc::sync_channel::<Work>(1);
        let (output, rx) = mpsc::sync_channel(1);
        let handle = thread::Builder::new()
            .name("world-demand-planner".into())
            .spawn(move || {
                while let Ok(w) = input.recv() {
                    let result = w.binding.plan(&w.resident).map_err(|e| e.to_string());
                    if output
                        .send(ResultPlan {
                            owner: w.owner,
                            generation: w.generation,
                            epoch: w.epoch,
                            stamp: w.stamp,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            tx: Some(tx),
            rx: Some(rx),
            thread: Some(handle),
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.tx.take();
        self.rx.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
#[derive(Default)]
struct Owner {
    epoch: u64,
    generation: u64,
    definition_stamp: String,
    stamp: String,
    plan_stamp: String,
    inflight: bool,
    plan: Option<StreamingPlan3D>,
    tickets: BTreeMap<String, CellTicket>,
    failed: BTreeMap<String, String>,
    error: Option<String>,
}
#[derive(Default)]
pub(super) struct WorldStreamingState {
    owners: BTreeMap<SourceEntityId, Owner>,
    worker: Option<Worker>,
}
impl std::fmt::Debug for WorldStreamingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldStreamingState")
            .field("owners", &self.owners.len())
            .finish()
    }
}

impl RuntimeInstanceLoader {
    /// Query-only compact projection; JSON is created only for an explicit reader.
    pub(crate) fn streaming_state(
        &self,
        id: &SourceEntityId,
        world: &World,
    ) -> Option<ComponentValue> {
        let owner = self.world_streaming.owners.get(id)?;
        if owner.epoch != world.epoch() || world.entity_generation(id) != Some(owner.generation) {
            return None;
        }
        let cells: serde_json::Map<_, _> = owner
            .tickets
            .iter()
            .map(|(id, ticket)| {
                let status = self
                    .cell_status(*ticket)
                    .map(|s| format!("{s:?}"))
                    .unwrap_or_else(|_| "Stale".into());
                (id.clone(), serde_json::Value::String(status))
            })
            .collect();
        let placements: BTreeMap<_, _> = owner
            .tickets
            .iter()
            .filter_map(|(id, ticket)| {
                self.cell_placement(*ticket, world)
                    .map(|p| (id.as_str(), p))
            })
            .collect();
        Some(ComponentValue::Dynamic {
            component_type: ComponentTypeId::new("engine.world_streaming_state"),
            value: crate::character_animation::runtime_json(
                serde_json::json!({"generation":owner.generation,"planning":owner.inflight,"ready":owner.plan_stamp==owner.stamp,"cells":cells,"placements":placements,"failed":owner.failed,"error":owner.error}),
            ),
        })
    }
    pub fn streaming_cell_ticket(
        &self,
        owner: &SourceEntityId,
        cell: &str,
        world: &World,
    ) -> Option<CellTicket> {
        let state = self.world_streaming.owners.get(owner)?;
        (state.epoch == world.epoch() && world.entity_generation(owner) == Some(state.generation))
            .then(|| state.tickets.get(cell).copied())
            .flatten()
    }
    /// Strong dependencies are checked in addition to the destination's active state.
    pub fn streaming_can_enter(
        &self,
        owner: &SourceEntityId,
        position: [f64; 3],
        world: &World,
    ) -> bool {
        let Some(binding) = binding(world, owner).ok().flatten() else {
            return false;
        };
        let Some(state) = self.world_streaming.owners.get(owner).filter(|s| {
            s.epoch == world.epoch() && world.entity_generation(owner) == Some(s.generation)
        }) else {
            return false;
        };
        fn ready(
            id: &str,
            b: &WorldStreaming3D,
            s: &Owner,
            loader: &RuntimeInstanceLoader,
        ) -> bool {
            s.tickets
                .get(id)
                .is_some_and(|t| loader.cell_status(*t) == Ok(CellStatus::Active))
                && b.cells
                    .iter()
                    .find(|c| c.id == id)
                    .is_some_and(|c| c.dependencies.iter().all(|d| ready(d, b, s, loader)))
        }
        binding
            .cells
            .iter()
            .any(|c| c.contains(position) && ready(&c.id, &binding, state, self))
    }
    pub fn advance_world_streaming(
        &mut self,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
        #[cfg(feature = "real-wgpu")] mut backend: Option<
            &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        >,
    ) {
        self.asset_loader.poll_async();
        let candidates = world.query_entities(
            &QuerySpec::all([ComponentTypeId::new("engine.world_streaming3d")]).limit(9),
        );
        let live: BTreeSet<_> = candidates.iter().cloned().collect();
        if let Some(worker) = &self.world_streaming.worker {
            while let Ok(result) = worker.rx.as_ref().unwrap().try_recv() {
                if let Some(owner) = self.world_streaming.owners.get_mut(&result.owner) {
                    if owner.epoch == result.epoch && owner.generation == result.generation {
                        owner.inflight = false;
                        if result.stamp == owner.stamp {
                            match result.result {
                                Ok(plan) => {
                                    owner.plan = Some(plan);
                                    owner.plan_stamp = result.stamp;
                                    owner.error = None;
                                }
                                Err(e) => owner.error = Some(e),
                            }
                        }
                    }
                }
            }
        }
        let old: Vec<_> = self.world_streaming.owners.keys().cloned().collect();
        for id in old {
            let mut owner = self.world_streaming.owners.remove(&id).unwrap();
            if !live.contains(&id)
                || owner.epoch != world.epoch()
                || world.entity_generation(&id) != Some(owner.generation)
            {
                for (cell, ticket) in owner.tickets.clone() {
                    let result = if owner.epoch != world.epoch() {
                        self.discard_obsolete_project_cell(
                            ticket,
                            world,
                            manager,
                            frame,
                            #[cfg(feature = "real-wgpu")]
                            backend.as_deref_mut(),
                        )
                    } else {
                        self.release_streaming_cell(
                            ticket,
                            world,
                            manager,
                            frame,
                            physics,
                            navigation,
                            #[cfg(feature = "real-wgpu")]
                            backend.as_deref_mut(),
                        )
                    };
                    match result {
                        Ok(()) => {
                            owner.tickets.remove(&cell);
                        }
                        Err(e) => owner.error = Some(format!("cleanup: {e:?}")),
                    }
                }
                if !owner.tickets.is_empty() {
                    self.world_streaming.owners.insert(id, owner);
                }
            } else {
                self.world_streaming.owners.insert(id, owner);
            }
        }
        world.streaming_ready.retain(|id, _| live.contains(id));
        if candidates.len() > 8 {
            world.streaming_ready.clear();
            return;
        }
        for id in candidates {
            let definition = match binding(world, &id) {
                Ok(Some(v)) => v,
                Ok(None) => continue,
                Err(error) => {
                    world.streaming_ready.remove(&id);
                    if let Some(o) = self.world_streaming.owners.get_mut(&id) {
                        o.error = Some(error);
                    }
                    continue;
                }
            };
            let generation = world.entity_generation(&id).unwrap();
            let mut owner = self
                .world_streaming
                .owners
                .remove(&id)
                .unwrap_or_else(|| Owner {
                    epoch: world.epoch(),
                    generation,
                    ..Default::default()
                });
            if owner.epoch != world.epoch() || owner.generation != generation {
                self.world_streaming.owners.insert(id, owner);
                continue;
            }
            let mut identity = definition.clone();
            identity.sources.clear();
            let definition_stamp =
                crate::canonical_digest::sha256_prefixed(&serde_json::to_vec(&identity).unwrap());
            if owner.definition_stamp != definition_stamp {
                for (cell, ticket) in owner.tickets.clone() {
                    match self.release_streaming_cell(
                        ticket,
                        world,
                        manager,
                        frame,
                        physics,
                        navigation,
                        #[cfg(feature = "real-wgpu")]
                        backend.as_deref_mut(),
                    ) {
                        Ok(()) => {
                            owner.tickets.remove(&cell);
                        }
                        Err(e) => owner.error = Some(format!("revision cleanup: {e:?}")),
                    }
                }
                if !owner.tickets.is_empty() {
                    world.streaming_ready.remove(&id);
                    self.world_streaming.owners.insert(id, owner);
                    continue;
                }
                owner.definition_stamp = definition_stamp;
                owner.plan_stamp.clear();
                owner.plan = None;
                owner.failed.clear();
                owner.error = None;
            }
            let resident: BTreeSet<_> = owner.tickets.keys().cloned().collect();
            let stamp = crate::canonical_digest::sha256_prefixed(
                &serde_json::to_vec(&(&definition, &resident)).unwrap(),
            );
            owner.stamp = stamp.clone();
            if owner.plan_stamp != stamp && !owner.inflight {
                if self.world_streaming.worker.is_none() {
                    match Worker::new() {
                        Ok(worker) => self.world_streaming.worker = Some(worker),
                        Err(e) => {
                            owner.error = Some(format!("planner unavailable: {e}"));
                            self.world_streaming.owners.insert(id, owner);
                            continue;
                        }
                    }
                }
                let worker = self.world_streaming.worker.as_ref().unwrap();
                if worker
                    .tx
                    .as_ref()
                    .unwrap()
                    .try_send(Work {
                        owner: id.clone(),
                        generation,
                        epoch: world.epoch(),
                        stamp: stamp.clone(),
                        binding: definition.clone(),
                        resident,
                    })
                    .is_ok()
                {
                    owner.inflight = true;
                }
            }
            if owner.plan_stamp == stamp {
                let plan = owner.plan.as_ref().unwrap();
                for (cell, ticket) in owner.tickets.clone() {
                    if !plan.desired.contains_key(&cell) {
                        match self.release_streaming_cell(
                            ticket,
                            world,
                            manager,
                            frame,
                            physics,
                            navigation,
                            #[cfg(feature = "real-wgpu")]
                            backend.as_deref_mut(),
                        ) {
                            Ok(()) => {
                                owner.tickets.remove(&cell);
                            }
                            Err(e) => owner.error = Some(format!("release: {e:?}")),
                        }
                    }
                }
                let mut loading = owner
                    .tickets
                    .values()
                    .filter(|t| self.cell_status(**t).is_ok_and(|s| s != CellStatus::Active))
                    .count();
                for cell in &plan.order {
                    if owner.failed.contains_key(cell) {
                        continue;
                    }
                    let descriptor = definition.cells.iter().find(|c| c.id == *cell).unwrap();
                    if !owner.tickets.contains_key(cell) {
                        if loading >= definition.max_loading_cells
                            || owner.tickets.len() >= definition.max_resident_cells
                        {
                            continue;
                        }
                        let domains = if descriptor.render_required {
                            BTreeSet::from([
                                CellDomain::World,
                                CellDomain::Asset,
                                CellDomain::Render,
                                CellDomain::Physics,
                                CellDomain::Navigation,
                            ])
                        } else {
                            BTreeSet::from([
                                CellDomain::World,
                                CellDomain::Asset,
                                CellDomain::Physics,
                                CellDomain::Navigation,
                            ])
                        };
                        match self.request_cell(
                            CellActivationRequest {
                                schema_version: "cell-activation.v1".into(),
                                placement_id: format!(
                                    "world/{}/{}",
                                    crate::canonical_digest::sha256_prefixed(
                                        &serde_json::to_vec(&(
                                            &definition.world_id,
                                            id.as_str(),
                                            &owner.definition_stamp
                                        ))
                                        .unwrap()
                                    ),
                                    cell
                                ),
                                prefab: descriptor.prefab.0.clone(),
                                asset_revision_epoch: self.asset_loader.asset_revision_epoch(),
                                required_domains: domains,
                                parent: None,
                            },
                            world,
                        ) {
                            Ok(ticket) => {
                                owner.tickets.insert(cell.clone(), ticket);
                                loading += 1;
                            }
                            Err(e) => {
                                let error = format!("load: {e:?}");
                                owner.error = Some(error.clone());
                                if !matches!(
                                    e,
                                    CellError::BudgetExceeded
                                        | CellError::Asset(
                                            crate::runtime_asset_loader::StreamingError::Busy
                                        )
                                ) {
                                    owner.failed.insert(cell.clone(), error);
                                }
                                continue;
                            }
                        }
                    }
                    let ticket = owner.tickets[cell];
                    let activate = plan.desired[cell].activate
                        && descriptor.dependencies.iter().all(|dep| {
                            owner
                                .tickets
                                .get(dep)
                                .is_some_and(|t| self.cell_status(*t) == Ok(CellStatus::Active))
                        });
                    let result = (|| -> Result<(), CellError> {
                        match self.cell_status(ticket)? {
                            CellStatus::Loading => {
                                self.prepare_cell_with_simulation(
                                    ticket, world, frame, manager, physics, navigation,
                                )?;
                            }
                            CellStatus::Failed => {
                                return Err(self
                                    .cell_error(ticket)
                                    .cloned()
                                    .unwrap_or(CellError::InvalidRequest));
                            }
                            CellStatus::ReadyToCommit if activate => {
                                self.commit_cell_with_simulation(
                                    ticket, world, physics, navigation,
                                )?;
                            }
                            #[cfg(feature = "real-wgpu")]
                            CellStatus::Prepared => {
                                if let Some(b) = backend.as_deref_mut() {
                                    self.begin_cell_upload(ticket, world, b)?;
                                }
                            }
                            #[cfg(feature = "real-wgpu")]
                            CellStatus::Uploading if activate => {
                                if let Some(b) = backend.as_deref_mut() {
                                    self.commit_cell_gpu_with_simulation(
                                        ticket, world, b, physics, navigation,
                                    )?;
                                }
                            }
                            _ => {}
                        }
                        Ok(())
                    })();
                    if let Err(e) = result {
                        if !matches!(e, CellError::NotReady | CellError::BudgetExceeded)
                            || self.cell_status(ticket) == Ok(CellStatus::Failed)
                        {
                            let error = format!("cell {cell}: {e:?}");
                            owner.error = Some(error.clone());
                            if self
                                .release_streaming_cell(
                                    ticket,
                                    world,
                                    manager,
                                    frame,
                                    physics,
                                    navigation,
                                    #[cfg(feature = "real-wgpu")]
                                    backend.as_deref_mut(),
                                )
                                .is_ok()
                            {
                                owner.tickets.remove(cell);
                                owner.failed.insert(cell.clone(), error);
                                loading = loading.saturating_sub(1);
                            }
                        }
                    }
                }
            }
            let mut regions = Vec::new();
            let mut ready = BTreeMap::new();
            fn active(
                id: &str,
                d: &WorldStreaming3D,
                o: &Owner,
                l: &RuntimeInstanceLoader,
                memo: &mut BTreeMap<String, bool>,
            ) -> bool {
                if let Some(v) = memo.get(id) {
                    return *v;
                }
                let result =
                    o.tickets
                        .get(id)
                        .is_some_and(|t| l.cell_status(*t) == Ok(CellStatus::Active))
                        && d.cells.iter().find(|c| c.id == id).is_some_and(|c| {
                            c.dependencies.iter().all(|id| active(id, d, o, l, memo))
                        });
                memo.insert(id.to_owned(), result);
                result
            }
            for cell in &definition.cells {
                if active(&cell.id, &definition, &owner, self, &mut ready) {
                    regions.push(crate::world_streaming::ReadyRegion {
                        min: cell.min,
                        max: cell.max,
                    });
                }
            }
            world
                .streaming_ready
                .insert(id.clone(), (generation, regions));
            self.world_streaming.owners.insert(id, owner);
        }
    }
    fn release_streaming_cell(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
        #[cfg(feature = "real-wgpu")] backend: Option<
            &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        >,
    ) -> Result<(), CellError> {
        #[cfg(feature = "real-wgpu")]
        if self.project_cell_has_upload(ticket) {
            return self.release_cell_gpu_with_simulation(
                ticket,
                world,
                manager,
                frame,
                backend.ok_or_else(|| CellError::Render("GPU owner unavailable".into()))?,
                physics,
                navigation,
            );
        }
        self.release_cell_with_simulation(ticket, world, manager, frame, physics, navigation)
    }
}
fn binding(world: &World, id: &SourceEntityId) -> Result<Option<WorldStreaming3D>, String> {
    let Some(ComponentValue::Dynamic { value, .. }) =
        world.component_value(id, &ComponentTypeId::new("engine.world_streaming3d"))
    else {
        return Ok(None);
    };
    let value: WorldStreaming3D =
        serde_json::from_value(crate::character_animation::dynamic_json(&value))
            .map_err(|e| e.to_string())?;
    value.validate().map_err(|e| e.to_string())?;
    Ok(Some(value))
}
