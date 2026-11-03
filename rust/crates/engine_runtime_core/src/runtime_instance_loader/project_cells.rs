//! Session correlations and bounded deferred demands on the original cell owner.
use super::*;
use crate::render_resource::RenderResourceManager;
use project_runtime_sdk::{
    ProjectRuntimeCellEntity as Entity, ProjectRuntimeCellIntent as Intent,
    ProjectRuntimeCellReceipt as Receipt, ProjectRuntimeCellStatus as Status,
};
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, Clone, Default)]
pub struct ProjectCellSnapshot {
    pub receipts: BTreeMap<u64, Receipt>,
    pub entities: BTreeMap<(String, String), (SourceEntityId, u64)>,
}
#[derive(Debug, Default)]
pub(super) struct ProjectCells {
    last_request: u64,
    pending: VecDeque<Intent>,
    tickets: BTreeMap<u64, CellTicket>,
    receipts: BTreeMap<u64, Receipt>,
    promotion_assets: BTreeMap<u64, RuntimeAssetHandle>,
}
fn request_id(intent: &Intent) -> u64 {
    match intent {
        Intent::Load { request_id, .. }
        | Intent::Release { request_id }
        | Intent::PromoteVegetation { request_id, .. }
        | Intent::Migrate { request_id, .. } => *request_id,
    }
}
fn parent_valid(parent: &Option<Entity>, world: &World) -> bool {
    parent.as_ref().is_none_or(|p| {
        p.generation != 0
            && world.entity_generation(&p.entity_id.clone().into()) == Some(p.generation)
    })
}
fn placement_valid(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};
    use project_runtime_sdk::ProjectRuntimeSpatialAssetRef;

    #[test]
    fn pending_cell_memory_observes_nested_load_and_migration_payloads_until_consumed() {
        let mut loader = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
            "unused",
            RuntimeAssetIndex::new(vec![], vec![]),
            RuntimePackageMountTable::new(vec![]),
        ));
        let mut expected = Vec::new();
        let mut text = |v: &str| {
            let mut s = String::with_capacity(4096);
            s.push_str(v);
            expected.push((
                format!("cpu-storage:{:x}", s.as_ptr() as usize),
                s.capacity() as u64,
            ));
            s
        };
        loader.project_cells.pending.push_back(Intent::Load {
            request_id: 1,
            placement_id: text("load-placement"),
            prefab: ProjectRuntimeSpatialAssetRef {
                id: text("prefab"),
                asset_type: text("prefab-type"),
                guid: Some(text("prefab-guid")),
                sub_asset: Some(text("sub-asset")),
            },
            parent: Some(Entity {
                entity_id: text("load-parent"),
                generation: 1,
            }),
            render_required: true,
        });
        loader.project_cells.pending.push_back(Intent::Migrate {
            request_id: 1,
            persistent_request_id: 2,
            source: Entity {
                entity_id: text("migration-source"),
                generation: 1,
            },
            persistent_placement_id: text("migration-placement"),
            parent: Some(Entity {
                entity_id: text("migration-parent"),
                generation: 1,
            }),
        });
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        loader.observe_memory(&mut sample);
        for (id, bytes) in &expected {
            assert!(
                sample
                    .allocations
                    .iter()
                    .any(|a| a.allocation_id == *id && a.bytes == *bytes),
                "missing pending backing {id}: {bytes} bytes"
            );
        }
        loader.project_cells.pending.clear();
        let mut after = crate::resource_memory::ResourceMemorySample::new(2, 1, 0);
        loader.observe_memory(&mut after);
        for (id, _) in expected {
            assert!(!after.allocations.iter().any(|a| a.allocation_id == id));
        }
    }
}

impl RuntimeInstanceLoader {
    pub(super) fn observe_project_cell_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        sample.cpu_deque(&self.project_cells.pending, "queues", &["queuePayload"]);
        for intent in &self.project_cells.pending {
            match intent {
                Intent::Load {
                    placement_id,
                    prefab,
                    parent,
                    ..
                } => {
                    for value in [placement_id, &prefab.id, &prefab.asset_type]
                        .into_iter()
                        .chain(prefab.guid.iter())
                        .chain(prefab.sub_asset.iter())
                        .chain(parent.iter().map(|p| &p.entity_id))
                    {
                        sample.cpu_string(value, "queues", &["queuePayload"]);
                    }
                }
                Intent::Migrate {
                    source,
                    persistent_placement_id,
                    parent,
                    ..
                } => {
                    for value in [&source.entity_id, persistent_placement_id]
                        .into_iter()
                        .chain(parent.iter().map(|p| &p.entity_id))
                    {
                        sample.cpu_string(value, "queues", &["queuePayload"]);
                    }
                }
                Intent::Release { .. } => (),
                Intent::PromoteVegetation {
                    placement_id,
                    owner,
                    source_instance_id,
                    target,
                    fields,
                    ..
                } => {
                    for value in [
                        placement_id,
                        &owner.entity_id,
                        source_instance_id,
                        &target.entity_id,
                        &fields.component_type,
                        &fields.source_id,
                        &fields.cluster_key,
                        &fields.cluster_asset_id,
                    ] {
                        sample.cpu_string(value, "queues", &["queuePayload"]);
                    }
                }
            }
        }
        for receipt in self.project_cells.receipts.values() {
            sample.cpu_string(&receipt.placement_id, "queues", &["queuePayload"]);
            for value in [&receipt.error_code, &receipt.error_message]
                .into_iter()
                .flatten()
            {
                sample.cpu_string(value, "queues", &["queuePayload"]);
            }
        }
    }
    /// Validate the complete callback before accepting any demand. Cell callbacks are separate
    /// from synchronous World mutation batches; completion is asynchronous and queryable.
    pub(crate) fn accept_project_cells(
        &mut self,
        intents: Vec<Intent>,
        world: &World,
    ) -> Result<(), &'static str> {
        if intents.len() > 64 || self.project_cells.pending.len() + intents.len() > 64 {
            return Err("cell.demand_budget_exceeded");
        }
        let mut last = self.project_cells.last_request;
        let mut placements: BTreeSet<String> = self
            .project_cells
            .receipts
            .values()
            .filter(|r| !matches!(r.status, Status::Released | Status::Failed))
            .map(|r| r.placement_id.clone())
            .collect();
        let mut known: BTreeSet<u64> = self.project_cells.receipts.keys().copied().collect();
        let mut releasing = BTreeSet::new();
        let mut admissions = Vec::new();
        for intent in &intents {
            let (new_id, placement) = match intent {
                Intent::Load {
                    request_id,
                    placement_id,
                    prefab,
                    parent,
                    ..
                } => {
                    if prefab.asset_type != "prefab"
                        || prefab.id.trim().is_empty()
                        || prefab.guid.as_ref().is_none_or(|v| v.trim().is_empty())
                        || !parent_valid(parent, world)
                    {
                        return Err("cell.invalid_load");
                    }
                    (*request_id, placement_id)
                }
                Intent::Release { request_id } => {
                    if !known.contains(request_id) || !releasing.insert(*request_id) {
                        return Err("cell.unknown_or_duplicate_release");
                    }
                    continue;
                }
                Intent::PromoteVegetation {
                    request_id,
                    placement_id,
                    owner,
                    source_instance_id,
                    target,
                    fields,
                } => {
                    if !parent_valid(&Some(owner.clone()), world)
                        || !parent_valid(&Some(target.clone()), world)
                        || source_instance_id.is_empty()
                        || source_instance_id.len() > 256
                        || !fields.component_type.starts_with("project.")
                        || [
                            &fields.source_id,
                            &fields.cluster_key,
                            &fields.cluster_asset_id,
                        ]
                        .iter()
                        .any(|p| p.is_empty() || p.len() > 256)
                    {
                        return Err("cell.invalid_vegetation_promotion");
                    }
                    (*request_id, placement_id)
                }
                Intent::Migrate {
                    request_id,
                    persistent_request_id,
                    source,
                    persistent_placement_id,
                    parent,
                } => {
                    if !known.contains(request_id)
                        || releasing.contains(request_id)
                        || source.generation == 0
                        || world.entity_generation(&source.entity_id.clone().into())
                            != Some(source.generation)
                        || !parent_valid(parent, world)
                    {
                        return Err("cell.invalid_migration");
                    }
                    (*persistent_request_id, persistent_placement_id)
                }
            };
            if new_id <= last
                || !placement_valid(placement)
                || !placements.insert(placement.clone())
            {
                return Err("cell.request_identity_invalid");
            }
            last = new_id;
            known.insert(new_id);
            admissions.push(Receipt {
                request_id: new_id,
                placement_id: placement.clone(),
                status: Status::Accepted,
                world_epoch: world.epoch(),
                asset_revision_epoch: self.asset_loader.asset_revision_epoch(),
                activation_epoch: None,
                error_code: None,
                error_message: None,
            });
        }
        let live = self
            .project_cells
            .receipts
            .values()
            .filter(|r| !matches!(r.status, Status::Released | Status::Failed))
            .count();
        if live + admissions.len() > 128 {
            return Err("cell.receipt_budget_exceeded");
        }
        if self.project_cells.receipts.len() + admissions.len() > 128 {
            let expired: Vec<_> = self
                .project_cells
                .receipts
                .iter()
                .filter(|(_, r)| matches!(r.status, Status::Released | Status::Failed))
                .map(|(k, _)| *k)
                .collect();
            for key in expired {
                self.project_cells.receipts.remove(&key);
                self.project_cells.tickets.remove(&key);
            }
        }
        for receipt in admissions {
            self.project_cells
                .receipts
                .insert(receipt.request_id, receipt);
        }
        self.project_cells.last_request = last;
        self.project_cells.pending.extend(intents);
        Ok(())
    }
    pub(crate) fn project_cell_receipt(&self, key: u64) -> Option<&Receipt> {
        self.project_cells.receipts.get(&key)
    }
    pub(crate) fn project_cell_entity(
        &self,
        placement: &str,
        source: &str,
        world: &World,
    ) -> Option<(SourceEntityId, u64)> {
        let runtime = self.resolve_soft_cell_entity(
            &SoftCellEntityRef {
                placement_id: placement.to_owned(),
                source_entity_id: source.to_owned(),
            },
            world,
        )?;
        let id = world
            .try_resolve_runtime_entity(runtime)
            .ok()?
            .source_id
            .clone();
        Some((id.clone(), world.entity_generation(&id)?))
    }
    pub fn project_cell_snapshot(&self, world: &World) -> ProjectCellSnapshot {
        let receipts = self.project_cells.receipts.clone();
        let mut entities = BTreeMap::new();
        for (key, ticket) in &self.project_cells.tickets {
            let Some(r) = receipts
                .get(key)
                .filter(|r| r.status == Status::Active && r.world_epoch == world.epoch())
            else {
                continue;
            };
            if let Some(instance) = self.prefab_instance(ticket.instance_id()) {
                for (source, id) in &instance.source_to_world_entity {
                    if world.runtime_id_for_source(id)
                        != instance.source_to_runtime_entity.get(source).copied()
                    {
                        continue;
                    }
                    if let Some(generation) = world.entity_generation(id) {
                        entities.insert(
                            (r.placement_id.clone(), source.to_string()),
                            (id.clone(), generation),
                        );
                    }
                }
            }
        }
        ProjectCellSnapshot { receipts, entities }
    }
    fn project_cell_error(&mut self, key: u64, error: &CellError) {
        if let Some(r) = self.project_cells.receipts.get_mut(&key) {
            r.status = Status::Failed;
            r.error_code = Some(
                match error {
                    CellError::WrongWorld => "cell.world_changed",
                    CellError::RevisionChanged => "cell.revision_changed",
                    CellError::ParentChanged => "cell.parent_changed",
                    CellError::BudgetExceeded => "cell.budget_exceeded",
                    CellError::Render(_) => "cell.render_failed",
                    CellError::Asset(_) => "cell.asset_failed",
                    _ => "cell.transaction_failed",
                }
                .into(),
            );
            r.error_message = Some(format!("{error:?}"));
        }
    }
    /// Runs only after begin_spatial_tick, before project callbacks. No blocking waits.
    pub(crate) fn advance_project_cells(
        &mut self,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
        physics: &mut crate::physics_3d::PhysicsModule,
        navigation: &mut crate::navigation_3d::NavigationModule,
        #[cfg(feature = "real-wgpu")] mut backend: Option<
            &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        >,
    ) {
        self.asset_loader.poll_async();
        let pending_count = self.project_cells.pending.len();
        for _ in 0..pending_count {
            let intent = self.project_cells.pending.pop_front().unwrap();
            let retry = intent.clone();
            let key = request_id(&intent);
            match intent {
                Intent::Load {
                    placement_id,
                    prefab,
                    parent,
                    render_required,
                    ..
                } => {
                    let receipt = self.project_cells.receipts.get(&key).unwrap();
                    if matches!(receipt.status, Status::Released | Status::Failed) {
                        continue;
                    }
                    let result = if receipt.world_epoch != world.epoch() {
                        Err(CellError::WrongWorld)
                    } else if !parent_valid(&parent, world) {
                        Err(CellError::ParentChanged)
                    } else {
                        self.request_cell(
                            CellActivationRequest {
                                schema_version: "cell-activation.v1".into(),
                                placement_id,
                                prefab: RuntimeAssetRef {
                                    id: prefab.id,
                                    asset_type: prefab.asset_type,
                                    guid: prefab.guid,
                                    sub_asset: prefab.sub_asset,
                                },
                                asset_revision_epoch: receipt.asset_revision_epoch,
                                required_domains: if render_required {
                                    BTreeSet::from([
                                        CellDomain::World,
                                        CellDomain::Asset,
                                        CellDomain::Physics,
                                        CellDomain::Navigation,
                                        CellDomain::Render,
                                    ])
                                } else {
                                    BTreeSet::from([
                                        CellDomain::World,
                                        CellDomain::Asset,
                                        CellDomain::Physics,
                                        CellDomain::Navigation,
                                    ])
                                },
                                parent: parent.map(|p| p.entity_id),
                            },
                            world,
                        )
                    };
                    match result {
                        Ok(ticket) => {
                            self.project_cells.tickets.insert(key, ticket);
                        }
                        Err(CellError::Asset(
                            crate::runtime_asset_loader::StreamingError::Busy,
                        )) => self.project_cells.pending.push_back(retry),
                        Err(e) => self.project_cell_error(key, &e),
                    }
                }
                Intent::Release { .. } => {
                    if let Some(handle) = self.project_cells.promotion_assets.remove(&key) {
                        let _ = self.asset_loader.release(&handle);
                    }
                    if let Some(ticket) = self.project_cells.tickets.get(&key).copied() {
                        #[cfg(feature = "real-wgpu")]
                        let result = if let Some(b) = backend.as_deref_mut() {
                            self.release_cell_gpu_with_simulation(
                                ticket, world, manager, frame, b, physics, navigation,
                            )
                        } else {
                            self.release_cell_with_simulation(
                                ticket, world, manager, frame, physics, navigation,
                            )
                        };
                        #[cfg(not(feature = "real-wgpu"))]
                        let result = self.release_cell_with_simulation(
                            ticket, world, manager, frame, physics, navigation,
                        );
                        match result {
                            Ok(()) => {
                                self.project_cells.receipts.get_mut(&key).unwrap().status =
                                    Status::Released;
                            }
                            Err(e) => {
                                // A failed unload preserves the live cell; its error remains queryable.
                                let r = self.project_cells.receipts.get_mut(&key).unwrap();
                                r.error_code = Some("cell.release_failed".into());
                                r.error_message = Some(format!("{e:?}"));
                            }
                        }
                    } else if let Some(r) = self.project_cells.receipts.get_mut(&key) {
                        r.status = Status::Released;
                    }
                }
                Intent::PromoteVegetation {
                    owner,
                    source_instance_id,
                    target,
                    fields,
                    ..
                } => {
                    let receipt = self.project_cells.receipts[&key].clone();
                    if matches!(receipt.status, Status::Released | Status::Failed) {
                        if let Some(handle) = self.project_cells.promotion_assets.remove(&key) {
                            let _ = self.asset_loader.release(&handle);
                        }
                        continue;
                    }
                    let result = (|| -> Result<(), CellError> {
                        if receipt.world_epoch != world.epoch() {
                            return Err(CellError::WrongWorld);
                        }
                        if receipt.asset_revision_epoch != self.asset_loader.asset_revision_epoch()
                        {
                            return Err(CellError::RevisionChanged);
                        }
                        if !self.project_cells.promotion_assets.contains_key(&key) {
                            let Some(crate::archetype::ComponentValue::Dynamic { value, .. }) =
                                world.component_value(
                                    &owner.entity_id.clone().into(),
                                    &crate::components::ComponentTypeId::new("engine.vegetation3d"),
                                )
                            else {
                                return Err(CellError::InvalidRequest);
                            };
                            let binding: crate::terrain3d::VegetationBinding3D =
                                serde_json::from_value(crate::character_animation::dynamic_json(
                                    &value,
                                ))
                                .map_err(|_| CellError::InvalidRequest)?;
                            let handle = self
                                .asset_loader
                                .request_async(&RuntimeAssetRef {
                                    id: binding.cluster_asset.asset_id,
                                    asset_type: "vegetation3d".into(),
                                    guid: Some(binding.cluster_asset.asset_guid),
                                    sub_asset: None,
                                })
                                .map_err(CellError::Asset)?;
                            self.project_cells.promotion_assets.insert(key, handle);
                        }
                        match self
                            .asset_loader
                            .get_handle_state(&self.project_cells.promotion_assets[&key])
                        {
                            Some(crate::runtime_asset::RuntimeAssetLoadState::Ready) => {}
                            Some(crate::runtime_asset::RuntimeAssetLoadState::Loading) => {
                                return Err(CellError::NotReady)
                            }
                            _ => {
                                return Err(CellError::Hydration(
                                    "vegetation.promotion: source request failed or was released"
                                        .into(),
                                ))
                            }
                        }
                        self.promote_vegetation_to_entity(
                            &owner.entity_id.clone().into(),
                            owner.generation,
                            &source_instance_id,
                            &target.entity_id.clone().into(),
                            target.generation,
                            &VegetationPromotionFields3D {
                                component_type: fields.component_type,
                                source_id: fields.source_id,
                                cluster_key: fields.cluster_key,
                                cluster_asset_id: fields.cluster_asset_id,
                            },
                            world,
                        )
                    })();
                    match result {
                        Err(
                            CellError::NotReady
                            | CellError::Asset(crate::runtime_asset_loader::StreamingError::Busy),
                        ) => {
                            self.project_cells.pending.push_back(retry);
                            continue;
                        }
                        Ok(()) => {
                            self.project_cells.receipts.get_mut(&key).unwrap().status =
                                Status::Active;
                        }
                        Err(e) => self.project_cell_error(key, &e),
                    }
                    if let Some(handle) = self.project_cells.promotion_assets.remove(&key) {
                        let _ = self.asset_loader.release(&handle);
                    }
                }
                Intent::Migrate {
                    persistent_request_id,
                    source,
                    persistent_placement_id,
                    parent,
                    ..
                } => {
                    let result = if let Some(ticket) = self.project_cells.tickets.get(&key).copied()
                    {
                        if !parent_valid(&parent, world) {
                            Err(CellError::ParentChanged)
                        } else {
                            let source_id =
                                self.prefab_instance(ticket.instance_id())
                                    .and_then(|instance| {
                                        instance
                                            .source_to_world_entity
                                            .iter()
                                            .find(|(_, id)| id.as_str() == source.entity_id)
                                            .map(|(authored, _)| authored.to_string())
                                    });
                            let Some(source_id) = source_id else {
                                self.project_cell_error(
                                    persistent_request_id,
                                    &CellError::StaleTicket,
                                );
                                continue;
                            };
                            let request = CellMigrationRequest {
                                source_entity_id: source_id,
                                source_generation: source.generation,
                                persistent_placement_id,
                                parent: parent.map(|p| p.entity_id),
                                preserve_world: true,
                            };
                            #[cfg(feature = "real-wgpu")]
                            let result = if self.project_cell_has_upload(ticket) {
                                if let Some(b) = backend.as_deref_mut() {
                                    self.migrate_cell_subtree_gpu_with_simulation(
                                        ticket, request, world, manager, b, physics, navigation,
                                    )
                                } else {
                                    Err(CellError::Render("render owner unavailable".into()))
                                }
                            } else {
                                self.migrate_cell_subtree_with_simulation(
                                    ticket, request, world, manager, physics, navigation,
                                )
                            };
                            #[cfg(not(feature = "real-wgpu"))]
                            let result = self.migrate_cell_subtree_with_simulation(
                                ticket, request, world, manager, physics, navigation,
                            );
                            result
                        }
                    } else {
                        Err(CellError::StaleTicket)
                    };
                    match result {
                        Ok(ticket) => {
                            self.project_cells
                                .tickets
                                .insert(persistent_request_id, ticket);
                        }
                        Err(e) => self.project_cell_error(persistent_request_id, &e),
                    }
                }
            }
        }
        let tickets: Vec<_> = self
            .project_cells
            .tickets
            .iter()
            .map(|(k, t)| (*k, *t))
            .collect();
        for (key, ticket) in tickets {
            if self
                .project_cells
                .receipts
                .get(&key)
                .is_some_and(|r| matches!(r.status, Status::Released | Status::Failed))
            {
                continue;
            }
            let result = (|| {
                self.cell_context_for_project(ticket, world)?;
                match self.cell_status(ticket)? {
                    CellStatus::Loading => {
                        self.prepare_cell_with_simulation(
                            ticket, world, frame, manager, physics, navigation,
                        )?;
                    }
                    CellStatus::ReadyToCommit => {
                        self.commit_cell_with_simulation(ticket, world, physics, navigation)?;
                    }
                    #[cfg(feature = "real-wgpu")]
                    CellStatus::Prepared => {
                        if let Some(b) = backend.as_deref_mut() {
                            self.begin_cell_upload(ticket, world, b)?;
                            let receipt = self.project_cells.receipts.get_mut(&key).unwrap();
                            receipt.error_code = None;
                            receipt.error_message = None;
                        } else {
                            let receipt = self.project_cells.receipts.get_mut(&key).unwrap();
                            receipt.error_code = Some("cell.render_owner_unavailable".into());
                            receipt.error_message=Some("Waiting for the native render owner; no GPU completion has been acknowledged.".into());
                        }
                    }
                    #[cfg(not(feature = "real-wgpu"))]
                    CellStatus::Prepared => {
                        return Err(CellError::Render("real GPU backend is unavailable".into()))
                    }
                    #[cfg(feature = "real-wgpu")]
                    CellStatus::Uploading => {
                        if let Some(b) = backend.as_deref_mut() {
                            self.commit_cell_gpu_with_simulation(
                                ticket, world, b, physics, navigation,
                            )?;
                        }
                    }
                    _ => {}
                }
                Ok::<_, CellError>(())
            })();
            if let Err(e) = result {
                if e == CellError::NotReady
                    || (e == CellError::BudgetExceeded
                        && self.cell_status(ticket).is_ok_and(|s| {
                            matches!(s, CellStatus::ReadyToCommit | CellStatus::Uploading)
                        }))
                {
                    continue;
                }
                if e == CellError::WrongWorld {
                    let cleanup = self.discard_obsolete_project_cell(
                        ticket,
                        world,
                        manager,
                        frame,
                        #[cfg(feature = "real-wgpu")]
                        backend.as_deref_mut(),
                    );
                    if let Err(cleanup_error) = cleanup {
                        // Keep the owner tracked so cleanup can retry once backpressure clears.
                        if let Some(r) = self.project_cells.receipts.get_mut(&key) {
                            r.error_code = Some("cell.cleanup_pending".into());
                            r.error_message = Some(format!("{cleanup_error:?}"));
                        }
                        continue;
                    }
                    self.project_cell_error(key, &e);
                    continue;
                }
                // Discard the old request through its original owner; never resurrect on late completion.
                #[cfg(feature = "real-wgpu")]
                let cleanup = if let Some(b) = backend.as_deref_mut() {
                    self.release_cell_gpu_with_simulation(
                        ticket, world, manager, frame, b, physics, navigation,
                    )
                } else {
                    self.release_cell_with_simulation(
                        ticket, world, manager, frame, physics, navigation,
                    )
                };
                #[cfg(not(feature = "real-wgpu"))]
                let cleanup = self.release_cell_with_simulation(
                    ticket, world, manager, frame, physics, navigation,
                );
                if let Err(cleanup_error) = cleanup {
                    let r = self.project_cells.receipts.get_mut(&key).unwrap();
                    r.error_code = Some("cell.cleanup_pending".into());
                    r.error_message = Some(format!("{e:?}; cleanup: {cleanup_error:?}"));
                    continue;
                }
                self.project_cell_error(key, &e);
                continue;
            }
            if let Ok(status) = self.cell_status(ticket) {
                let activation_epoch = self.cell_activation_epoch(ticket).ok().flatten();
                let r = self.project_cells.receipts.get_mut(&key).unwrap();
                r.status = match status {
                    CellStatus::Loading => Status::Loading,
                    CellStatus::Prepared | CellStatus::ReadyToCommit => Status::Prepared,
                    CellStatus::Uploading => Status::Uploading,
                    CellStatus::Active => Status::Active,
                    CellStatus::Released => Status::Released,
                    CellStatus::Failed => Status::Failed,
                };
                r.activation_epoch = activation_epoch;
            }
        }
    }
}
