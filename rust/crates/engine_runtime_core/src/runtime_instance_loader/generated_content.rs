//! Admission and correlations on the existing instance owner. No GPU readiness is inferred here.
use super::*;
use crate::ids::EntityId;
use crate::render_geometry::generated::GeneratedGeometry;
use project_runtime_sdk::generated_content::*;
use project_runtime_sdk::ProjectRuntimeCellEntity;
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "generated_content_cpu.rs"]
mod cpu;
#[cfg(feature = "real-wgpu")]
#[path = "generated_content_native.rs"]
pub(super) mod native;

#[derive(Debug)]
pub(super) struct GeneratedContentState {
    generation: u64,
    world_epoch: Option<u64>,
    accepting: bool,
    last_request: u64,
    next_revision: u64,
    records: BTreeMap<u64, Record>,
    bindings: BTreeMap<(String, u64), Binding>,
    cpu_backings: BTreeMap<usize, cpu::BackingCharge>,
    worker: Option<cpu::Worker>,
    #[cfg(feature = "real-wgpu")]
    commit_epoch: u64,
    #[cfg(test)]
    worker_gate: Option<cpu::TestGate>,
}

impl Default for GeneratedContentState {
    fn default() -> Self {
        Self {
            generation: next_content_session_generation(),
            accepting: true,
            world_epoch: None,
            last_request: 0,
            next_revision: 1,
            records: BTreeMap::new(),
            bindings: BTreeMap::new(),
            cpu_backings: BTreeMap::new(),
            worker: None,
            #[cfg(feature = "real-wgpu")]
            commit_epoch: 0,
            #[cfg(test)]
            worker_gate: None,
        }
    }
}

pub(super) fn next_content_session_generation() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("content session identity exhausted")
}

#[derive(Debug)]
struct Record {
    receipt: ContentReceipt,
    request: Option<GeneratedContentRequest>,
    expected_revision: u64,
    binding_before: Option<crate::spatial_contract::MeshRenderer3D>,
    collision_before: Option<Option<crate::archetype::ComponentValue>>,
    // Retained after cancellation/supersession until the actual worker returns.
    worker_active: bool,
    cpu_charge: usize,
    backing_slots: usize,
    candidate: Option<cpu::Candidate>,
    #[cfg(feature = "real-wgpu")]
    upload: Option<crate::geometry_upload::GeometryUploadTicket>,
    #[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
    prepared_collision: Option<crate::physics_3d::PreparedStaticCollision>,
}

impl Record {
    fn native_in_flight(&self) -> bool {
        #[cfg(feature = "real-wgpu")]
        {
            self.upload.is_some()
        }
        #[cfg(not(feature = "real-wgpu"))]
        {
            false
        }
    }
}

#[derive(Debug, Clone)]
struct Layout {
    vertex_count: u32,
    vertex_stride: u32,
    attributes: Vec<AttributeLayout3D>,
}
#[derive(Debug, Default)]
struct Binding {
    applied: u64,
    pending: Option<u64>,
    collision_revision: u64,
    layout: Option<Layout>,
    canonical: Option<GeneratedGeometry>,
    #[cfg(feature = "real-wgpu")]
    published: Option<native::Publication>,
    #[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
    collision: Option<crate::physics_3d::GeneratedStaticBinding>,
    #[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
    collision_cpu_charge: usize,
}

/// Host-owned input, moved out exactly once. It never crosses the module ABI.
#[derive(Debug)]
pub(crate) struct GeneratedContentWork {
    pub ticket: ContentTicket,
    pub candidate_revision: u64,
    pub request: GeneratedContentRequest,
}

fn error(code: ContentErrorCode, request_id: u64, field: &str) -> ContentError {
    ContentError {
        code,
        request_id,
        field_path: field.into(),
        expected: None,
        actual: None,
        next_action: match code {
            ContentErrorCode::RevisionConflict | ContentErrorCode::PendingUpdate => {
                "Query the applied and pending revisions before submitting a new request."
            }
            ContentErrorCode::BudgetExceeded => {
                "Wait for outstanding work to retire or submit a smaller request."
            }
            ContentErrorCode::StaleTarget => "Resolve the target's current entity generation.",
            ContentErrorCode::StaleSession => "Use a ticket from the current session.",
            ContentErrorCode::ReceiptExpired => {
                "The terminal receipt has left the bounded receipt window."
            }
            ContentErrorCode::AlreadyApplied => {
                "Submit a new revision to change the applied content."
            }
            _ => "Query the current request state.",
        }
        .into(),
    }
}
fn target_key(target: &ProjectRuntimeCellEntity) -> (String, u64) {
    (target.entity_id.clone(), target.generation)
}
fn live(target: &ProjectRuntimeCellEntity, world: &World) -> bool {
    let id: EntityId = target.entity_id.as_str().into();
    world.entity_generation(&id) == Some(target.generation) && world.transform3d(&id).is_some()
}

impl GeneratedContentState {
    fn check_world(&self, world: &World, request_id: u64) -> Result<(), ContentError> {
        if self.world_epoch.is_some_and(|epoch| epoch != world.epoch()) {
            return Err(error(
                ContentErrorCode::StaleSession,
                request_id,
                "session.worldEpoch",
            ));
        }
        Ok(())
    }
    fn check_ticket(&self, ticket: ContentTicket) -> Result<&Record, ContentError> {
        if ticket.session_generation != self.generation {
            return Err(error(
                ContentErrorCode::StaleSession,
                ticket.request_id,
                "ticket.sessionGeneration",
            ));
        }
        self.records.get(&ticket.request_id).ok_or_else(|| {
            error(
                if ticket.request_id != 0 && ticket.request_id <= self.last_request {
                    ContentErrorCode::ReceiptExpired
                } else {
                    ContentErrorCode::NotFound
                },
                ticket.request_id,
                "ticket.requestId",
            )
        })
    }

    fn reclaim_terminal(&mut self, id: u64, assets: &mut RuntimeAssetLoader) {
        let record = self
            .records
            .get_mut(&id)
            .expect("existing generated request");
        if record.receipt.status.terminal() && !record.worker_active {
            record.request = None;
            record.candidate = None;
            #[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
            {
                record.prepared_collision = None;
            }
            record.backing_slots = 0;
            assets.release_prepared_cpu(std::mem::take(&mut record.cpu_charge));
        }
    }

    fn update_pending_receipts(&mut self, target: &ProjectRuntimeCellEntity) {
        let binding = self.bindings.get(&target_key(target));
        for record in self
            .records
            .values_mut()
            .filter(|r| r.receipt.target == *target)
        {
            record.receipt.applied_revision = binding.map_or(0, |b| b.applied);
            record.receipt.pending_revision = binding.and_then(|b| b.pending);
            record.receipt.collision_revision = binding.map_or(0, |b| b.collision_revision);
        }
    }

    fn admit(
        &mut self,
        request: GeneratedContentRequest,
        world: &World,
        assets: &mut RuntimeAssetLoader,
    ) -> Result<ContentTicket, ContentError> {
        let id = request.request_id;
        self.reap_cpu_backings(assets);
        if !self.accepting {
            return Err(error(ContentErrorCode::StaleSession, id, "session"));
        }
        request.validate()?;
        self.check_world(world, id)?;
        if id <= self.last_request {
            return Err(error(ContentErrorCode::InvalidArgument, id, "requestId"));
        }
        if !live(&request.target, world) {
            return Err(error(ContentErrorCode::StaleTarget, id, "target"));
        }
        let resolve = |source: &project_runtime_sdk::ProjectRuntimeSpatialAssetRef| {
            let reference = RuntimeAssetRef {
                id: source.id.clone(),
                asset_type: source.asset_type.clone(),
                guid: source.guid.clone(),
                sub_asset: source.sub_asset.clone(),
            };
            let record = assets
                .asset_index()
                .resolve(&reference)
                .map_err(|_| error(ContentErrorCode::InvalidArgument, id, "assetRef"))?;
            if record.asset_id != reference.id || record.sub_asset_id != reference.sub_asset {
                return Err(error(
                    ContentErrorCode::InvalidArgument,
                    id,
                    "assetRef.identity",
                ));
            }
            Ok(())
        };
        let collision = match &request.operation {
            GeneratedOperation3D::Replace {
                geometry,
                collision,
            } => {
                for section in &geometry.sections {
                    resolve(&section.material)?;
                }
                Some(collision)
            }
            GeneratedOperation3D::ReplaceCollision { collision } => Some(collision),
            _ => None,
        };
        if let Some(CollisionUpdate3D::Replace {
            body:
                StaticBody3D {
                    shape: StaticShape3D::Asset { source },
                    ..
                },
        }) = collision
        {
            resolve(source)?;
        }
        let key = target_key(&request.target);
        let old = self.bindings.get(&key);
        let applied = old.map_or(0, |b| b.applied);
        if applied != request.expected_applied_revision {
            let mut e = error(
                ContentErrorCode::RevisionConflict,
                id,
                "expectedAppliedRevision",
            );
            e.expected = Some(applied);
            e.actual = Some(request.expected_applied_revision);
            return Err(e);
        }
        if let GeneratedOperation3D::UpdateAttributes { patch } = &request.operation {
            #[cfg(feature = "real-wgpu")]
            if old
                .and_then(|b| b.published.as_ref())
                .is_some_and(|p| !native::owns_renderer(p, world, &key))
            {
                return Err(error(
                    ContentErrorCode::RevisionConflict,
                    id,
                    "patch.baseBinding",
                ));
            }
            if old.is_some_and(|b| b.pending.is_some()) {
                return Err(error(
                    ContentErrorCode::PendingUpdate,
                    id,
                    "target.pendingRevision",
                ));
            }
            let layout = old
                .and_then(|b| b.layout.as_ref())
                .ok_or_else(|| error(ContentErrorCode::RevisionConflict, id, "patch.base"))?;
            if patch
                .first_vertex
                .checked_add(patch.vertex_count)
                .is_none_or(|end| end > layout.vertex_count)
                || !layout
                    .attributes
                    .iter()
                    .any(|a| a.semantic == patch.semantic)
            {
                return Err(error(
                    ContentErrorCode::InvalidArgument,
                    id,
                    "patch.rangeOrAttribute",
                ));
            }
        }
        let active = self
            .records
            .values()
            .filter(|r| !r.receipt.status.terminal() || r.worker_active || r.native_in_flight())
            .count();
        let budget = assets.streaming_budget();
        let backing_slots = match request.operation {
            GeneratedOperation3D::Replace { .. } => 2,
            GeneratedOperation3D::UpdateAttributes { .. } => 1,
            _ => 0,
        };
        let reserved_slots: usize = self.records.values().map(|r| r.backing_slots).sum();
        if self
            .cpu_backings
            .len()
            .saturating_add(reserved_slots)
            .saturating_add(backing_slots)
            > budget.max_handles
        {
            return Err(error(
                ContentErrorCode::BudgetExceeded,
                id,
                "cpu.backingHandles",
            ));
        }
        if active >= GENERATED_RECEIPT_LIMIT
            || active.saturating_add(assets.streaming_snapshot().pending_operations)
                >= budget.max_operations
        {
            return Err(error(ContentErrorCode::BudgetExceeded, id, "queue"));
        }
        let eviction = if self.records.len() >= GENERATED_RECEIPT_LIMIT {
            Some(
                self.records
                    .iter()
                    .find(|(_, r)| {
                        r.receipt.status.terminal()
                            && !r.worker_active
                            && !r.native_in_flight()
                            && r.cpu_charge == 0
                    })
                    .map(|(id, _)| *id)
                    .ok_or_else(|| error(ContentErrorCode::BudgetExceeded, id, "receipts"))?,
            )
        } else {
            None
        };
        let payload = request.payload_bytes();
        if payload > project_runtime_sdk::spatial_bulk::MAX_SESSION_BYTES {
            return Err(error(ContentErrorCode::BudgetExceeded, id, "request.bytes"));
        }
        if payload > budget.max_asset_bytes {
            return Err(error(
                ContentErrorCode::BudgetExceeded,
                id,
                "request.assetBytes",
            ));
        }
        // A small attribute attachment still constructs a complete new vertex backing.
        // Reserve it before changing pending/superseded state or dispatching any work.
        let candidate_bytes = if matches!(
            request.operation,
            GeneratedOperation3D::UpdateAttributes { .. }
        ) {
            let layout = old
                .and_then(|b| b.layout.as_ref())
                .expect("validated patch base");
            (layout.vertex_count as usize)
                .checked_mul(layout.vertex_stride as usize)
                .ok_or_else(|| error(ContentErrorCode::BudgetExceeded, id, "cpu.candidateBytes"))?
        } else {
            0
        };
        let charge = cpu::request_backing_bytes(&request)
            .checked_add(candidate_bytes)
            .and_then(|bytes| bytes.checked_add(cpu::collision_decode_bytes(&request)))
            .ok_or_else(|| error(ContentErrorCode::BudgetExceeded, id, "cpu.candidateBytes"))?;
        let prepared_bytes = self.pending_cpu_bytes();
        if prepared_bytes
            .saturating_add(assets.streaming_snapshot().reserved_bytes)
            .saturating_add(charge)
            > budget.max_in_flight_bytes
        {
            return Err(error(
                ContentErrorCode::BudgetExceeded,
                id,
                "cpu.inFlightBytes",
            ));
        }
        let revision = self.next_revision;
        let next_revision = revision
            .checked_add(1)
            .ok_or_else(|| error(ContentErrorCode::BudgetExceeded, id, "candidateRevision"))?;
        // Reservation precedes every mutation, including old-candidate supersession and eviction.
        assets
            .reserve_prepared_cpu(charge)
            .map_err(|_| error(ContentErrorCode::BudgetExceeded, id, "cpu.preparedBytes"))?;
        self.world_epoch = Some(world.epoch());
        if let Some(eviction) = eviction {
            self.records.remove(&eviction);
        }
        if let Some(previous) = self.bindings.get(&key).and_then(|b| b.pending) {
            let previous_id = self
                .records
                .iter()
                .find(|(_, r)| {
                    r.receipt.target == request.target && r.receipt.candidate_revision == previous
                })
                .map(|(id, _)| *id)
                .expect("pending has a live receipt");
            self.records.get_mut(&previous_id).unwrap().receipt.status = ContentStatus::Superseded;
            self.reclaim_terminal(previous_id, assets);
        }
        let binding = self.bindings.entry(key).or_default();
        binding.pending = Some(revision);
        let ticket = ContentTicket {
            session_generation: self.generation,
            request_id: id,
        };
        let target = request.target.clone();
        self.records.insert(
            id,
            Record {
                receipt: ContentReceipt {
                    ticket,
                    target: target.clone(),
                    status: ContentStatus::Accepted,
                    candidate_revision: revision,
                    applied_revision: binding.applied,
                    pending_revision: binding.pending,
                    collision_revision: binding.collision_revision,
                    commit_epoch: None,
                    error: None,
                },
                binding_before: world
                    .mesh_renderer3d(&EntityId::from(target.entity_id.as_str()))
                    .cloned(),
                collision_before: collision
                    .filter(|c| !matches!(c, CollisionUpdate3D::Keep))
                    .map(|_| {
                        world.component_value(
                            &EntityId::from(target.entity_id.as_str()),
                            &crate::components::ComponentTypeId::new("engine.body3d"),
                        )
                    }),
                expected_revision: request.expected_applied_revision,
                request: Some(request),
                worker_active: false,
                cpu_charge: charge,
                backing_slots,
                candidate: None,
                #[cfg(feature = "real-wgpu")]
                upload: None,
                #[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
                prepared_collision: None,
            },
        );
        self.next_revision = next_revision;
        self.last_request = id;
        self.update_pending_receipts(&target);
        Ok(ticket)
    }

    fn clear_pending(&mut self, ticket: ContentTicket) {
        let record = &self.records[&ticket.request_id];
        let key = target_key(&record.receipt.target);
        if let Some(binding) = self.bindings.get_mut(&key) {
            if binding.pending == Some(record.receipt.candidate_revision) {
                binding.pending = None;
            }
        }
        let target = record.receipt.target.clone();
        if self
            .bindings
            .get(&key)
            .is_some_and(|b| b.applied == 0 && b.pending.is_none() && b.layout.is_none())
        {
            self.bindings.remove(&key);
        }
        self.update_pending_receipts(&target);
    }

    fn cancel(
        &mut self,
        ticket: ContentTicket,
        assets: &mut RuntimeAssetLoader,
    ) -> Result<(), ContentError> {
        let status = self.check_ticket(ticket)?.receipt.status;
        if status == ContentStatus::Applied {
            return Err(error(
                ContentErrorCode::AlreadyApplied,
                ticket.request_id,
                "ticket",
            ));
        }
        if status.terminal() {
            return Ok(());
        }
        self.records
            .get_mut(&ticket.request_id)
            .unwrap()
            .receipt
            .status = ContentStatus::Cancelled;
        self.clear_pending(ticket);
        self.reclaim_terminal(ticket.request_id, assets);
        self.reap_cpu_backings(assets);
        Ok(())
    }

    fn begin(
        &mut self,
        ticket: ContentTicket,
        world: &World,
        assets: &mut RuntimeAssetLoader,
    ) -> Result<GeneratedContentWork, ContentError> {
        let record = self.check_ticket(ticket)?;
        self.check_world(world, ticket.request_id)?;
        if record.receipt.status != ContentStatus::Accepted {
            return Err(error(
                ContentErrorCode::InvalidArgument,
                ticket.request_id,
                "status",
            ));
        }
        if !live(&record.receipt.target, world) {
            let e = error(ContentErrorCode::StaleTarget, ticket.request_id, "target");
            self.fail(ticket, e.clone(), assets)?;
            return Err(e);
        }
        let record = self.records.get_mut(&ticket.request_id).unwrap();
        record.worker_active = true;
        record.receipt.status = ContentStatus::Preparing;
        Ok(GeneratedContentWork {
            ticket,
            candidate_revision: record.receipt.candidate_revision,
            request: record
                .request
                .take()
                .expect("one dispatch per admitted request"),
        })
    }

    fn fail(
        &mut self,
        ticket: ContentTicket,
        failure: ContentError,
        assets: &mut RuntimeAssetLoader,
    ) -> Result<(), ContentError> {
        let record = self.check_ticket(ticket)?;
        if record.receipt.status.terminal() {
            return Ok(());
        }
        let record = self.records.get_mut(&ticket.request_id).unwrap();
        record.receipt.status = ContentStatus::Failed;
        record.receipt.error = Some(failure);
        self.clear_pending(ticket);
        self.reclaim_terminal(ticket.request_id, assets);
        Ok(())
    }

    /// Call only after the actual worker has released its input/candidate backing.
    fn worker_finished(
        &mut self,
        ticket: ContentTicket,
        assets: &mut RuntimeAssetLoader,
    ) -> Result<bool, ContentError> {
        self.check_ticket(ticket)?;
        let record = self.records.get_mut(&ticket.request_id).unwrap();
        record.worker_active = false;
        record.backing_slots = 0;
        let eligible = !record.receipt.status.terminal();
        self.reclaim_terminal(ticket.request_id, assets);
        self.reap_cpu_backings(assets);
        Ok(eligible)
    }

    /// The caller must additionally prove original GPU Ready/native Prepared before publication.
    /// This guard deliberately never changes status to Applied or mutates World.
    fn validate_commit(&self, ticket: ContentTicket, world: &World) -> Result<(), ContentError> {
        let record = self.check_ticket(ticket)?;
        self.check_world(world, ticket.request_id)?;
        if !self.accepting {
            return Err(error(
                ContentErrorCode::StaleSession,
                ticket.request_id,
                "session",
            ));
        }
        if !live(&record.receipt.target, world) {
            return Err(error(
                ContentErrorCode::StaleTarget,
                ticket.request_id,
                "target",
            ));
        }
        if record.receipt.status != ContentStatus::Preparing {
            return Err(error(
                ContentErrorCode::RevisionConflict,
                ticket.request_id,
                "candidateRevision",
            ));
        }
        let binding = self
            .bindings
            .get(&target_key(&record.receipt.target))
            .filter(|b| b.pending == Some(record.receipt.candidate_revision))
            .ok_or_else(|| {
                error(
                    ContentErrorCode::RevisionConflict,
                    ticket.request_id,
                    "candidateRevision",
                )
            })?;
        if binding.applied != record.expected_revision {
            return Err(error(
                ContentErrorCode::RevisionConflict,
                ticket.request_id,
                "expectedAppliedRevision",
            ));
        }
        let now = world.mesh_renderer3d(&EntityId::from(record.receipt.target.entity_id.as_str()));
        if record.collision_before.as_ref().is_some_and(|before| {
            &world.component_value(
                &EntityId::from(record.receipt.target.entity_id.as_str()),
                &crate::components::ComponentTypeId::new("engine.body3d"),
            ) != before
        }) {
            return Err(error(
                ContentErrorCode::RevisionConflict,
                ticket.request_id,
                "target.collisionBinding",
            ));
        }
        if now.map(|r| (&r.mesh, &r.materials))
            != record
                .binding_before
                .as_ref()
                .map(|r| (&r.mesh, &r.materials))
        {
            return Err(error(
                ContentErrorCode::RevisionConflict,
                ticket.request_id,
                "target.binding",
            ));
        }
        Ok(())
    }
}

impl RuntimeInstanceLoader {
    pub(crate) fn reject_generated_content_without_native(&mut self, world: &World) {
        if self.generated_content.check_world(world, 0).is_ok() {
            let active: Vec<_> = self
                .generated_content
                .records
                .values()
                .filter(|r| !r.receipt.status.terminal())
                .map(|r| r.receipt.ticket)
                .collect();
            for ticket in active {
                let failure = error(
                    ContentErrorCode::CapabilityGap,
                    ticket.request_id,
                    "render.nativeUpload",
                );
                self.generated_content
                    .fail(ticket, failure, &mut self.asset_loader)
                    .expect("current ticket");
            }
        }
        self.advance_generated_content_cpu(world);
    }

    // Borrowed counters on the existing opt-in memory path, never a heap census.
    pub(super) fn observe_generated_content_usage(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        #[cfg(test)]
        crate::project_runtime_session::report_work::CONTENT_USAGE_CALLS
            .with(|n| n.set(n.get() + 1));
        let state = &self.generated_content;
        let request_charge: usize = state.records.values().map(|r| r.cpu_charge).sum();
        let backing_charge: usize = state.cpu_backings.values().map(|b| b.bytes).sum();
        #[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
        let collision_charge: usize = state
            .bindings
            .values()
            .map(|b| b.collision_cpu_charge)
            .sum();
        #[cfg(not(all(feature = "real-wgpu", not(target_arch = "wasm32"))))]
        let collision_charge = 0;
        for (name, value) in [
            ("receipts", state.records.len()),
            (
                "pendingTargets",
                state
                    .bindings
                    .values()
                    .filter(|b| b.pending.is_some())
                    .count(),
            ),
            ("cpuWorkerActive", usize::from(state.worker.is_some())),
            (
                "nativeUploadPins",
                state
                    .records
                    .values()
                    .filter(|r| r.native_in_flight())
                    .count(),
            ),
            ("cpuRequestChargedBytes", request_charge),
            ("cpuBackingChargedBytes", backing_charge),
            ("cpuCollisionChargedBytes", collision_charge),
            (
                "cpuChargedBytes",
                request_charge
                    .saturating_add(backing_charge)
                    .saturating_add(collision_charge),
            ),
        ] {
            sample
                .capacities
                .insert(format!("generatedContent.{name}"), value as u64);
        }
        sample.observed_scopes.insert("generatedContent.fixedUsageCounters;reservationBytesNotHeapOrDriverCensus;requestIdentityViaContentReceipt".into());
    }

    pub(crate) fn query_generated_content(
        &self,
        query: ContentQuery,
        world: &World,
    ) -> ContentQueryReply {
        query.validate()?;
        self.generated_content.check_world(world, 0)?;
        match query {
            ContentQuery::Request { request_id } => self
                .generated_content_receipt(ContentTicket {
                    session_generation: self.generated_content.generation,
                    request_id,
                })
                .map(|receipt| ContentQueryValue::Receipt { receipt }),
            ContentQuery::Ticket { ticket } => self
                .generated_content_receipt(ticket)
                .map(|receipt| ContentQueryValue::Receipt { receipt }),
            ContentQuery::Target { target } => {
                if !live(&target, world) {
                    return Err(error(ContentErrorCode::StaleTarget, 0, "target"));
                }
                let binding = self.generated_content.bindings.get(&target_key(&target));
                Ok(ContentQueryValue::Target {
                    state: ContentTargetState {
                        session_generation: self.generated_content.generation,
                        target,
                        applied_revision: binding.map_or(0, |b| b.applied),
                        pending_revision: binding.and_then(|b| b.pending),
                        collision_revision: binding.map_or(0, |b| b.collision_revision),
                    },
                })
            }
        }
    }
    pub(crate) fn accept_generated_content(
        &mut self,
        request: GeneratedContentRequest,
        world: &World,
    ) -> Result<ContentTicket, ContentError> {
        self.generated_content
            .admit(request, world, &mut self.asset_loader)
    }
    pub(crate) fn generated_content_receipt(
        &self,
        ticket: ContentTicket,
    ) -> Result<ContentReceipt, ContentError> {
        let record = self.generated_content.check_ticket(ticket)?;
        Ok(record.receipt.clone())
    }
    pub(crate) fn cancel_generated_content(
        &mut self,
        ticket: ContentTicket,
    ) -> Result<(), ContentError> {
        self.generated_content
            .cancel(ticket, &mut self.asset_loader)
    }
    pub(crate) fn begin_generated_content(
        &mut self,
        ticket: ContentTicket,
        world: &World,
    ) -> Result<GeneratedContentWork, ContentError> {
        self.generated_content
            .begin(ticket, world, &mut self.asset_loader)
    }
    pub(crate) fn generated_content_worker_finished(
        &mut self,
        ticket: ContentTicket,
    ) -> Result<bool, ContentError> {
        self.generated_content
            .worker_finished(ticket, &mut self.asset_loader)
    }
    pub(crate) fn validate_generated_content_commit(
        &self,
        ticket: ContentTicket,
        world: &World,
    ) -> Result<(), ContentError> {
        self.generated_content.validate_commit(ticket, world)
    }
    pub(crate) fn close_generated_content(&mut self) {
        self.generated_content.accepting = false;
        let active: Vec<_> = self
            .generated_content
            .records
            .values()
            .filter(|r| !r.receipt.status.terminal())
            .map(|r| r.receipt.ticket)
            .collect();
        for ticket in active {
            self.cancel_generated_content(ticket)
                .expect("current session ticket");
        }
        // This worker only processes bounded engine-owned CPU data, never filesystem/native
        // calls or project callbacks. Join before releasing its reservations or this owner.
        if let Some(worker) = self.generated_content.worker.take() {
            let ticket = worker.ticket;
            drop(worker.join());
            self.generated_content_worker_finished(ticket)
                .expect("current worker ticket");
        }
        // Closing this instance relinquishes its canonical readers as well. Upload/frame
        // readers may still pin their backings, so only the weak reaper returns the charge.
        self.generated_content.bindings.retain(|_, binding| {
            binding.canonical = None;
            binding.layout = None;
            #[cfg(feature = "real-wgpu")]
            {
                let retained = binding.published.is_some();
                #[cfg(not(target_arch = "wasm32"))]
                let retained = retained || binding.collision.is_some();
                retained
            }
            #[cfg(not(feature = "real-wgpu"))]
            {
                false
            }
        });
        self.generated_content
            .reap_cpu_backings(&mut self.asset_loader);
    }
}

#[cfg(test)]
#[path = "generated_content_tests.rs"]
mod tests;
