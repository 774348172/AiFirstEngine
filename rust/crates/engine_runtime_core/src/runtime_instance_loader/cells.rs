//! Cell transactions reuse the instance loader, package cache and hydration owner.
use super::*;
#[path = "cell_persistence.rs"]
mod persistence;
#[path = "cell_simulation.rs"]
mod simulation;
use crate::{navigation_3d::NavigationModule, physics_3d::PhysicsModule};
type SimulationOwners<'a> = Option<(&'a mut PhysicsModule, &'a mut NavigationModule)>;
use crate::render_asset_production::{PreparedMeshRenderer, RuntimeRenderAssetProducer};
use crate::render_resource::{
    RenderResourceHandle, RenderResourceManager, RenderResourceReleaseRequest,
};
use crate::runtime_asset::RuntimeDecodedPayload;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CellDomain {
    World,
    Asset,
    Render,
    Physics,
    Navigation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CellActivationRequest {
    pub schema_version: String,
    pub placement_id: String,
    pub prefab: RuntimeAssetRef,
    pub asset_revision_epoch: u64,
    pub required_domains: BTreeSet<CellDomain>,
    pub parent: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellTicket {
    owner: u64,
    instance: RuntimeInstanceId,
    generation: u64,
}
impl CellTicket {
    pub fn instance_id(self) -> RuntimeInstanceId {
        self.instance
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStatus {
    Loading,
    Prepared,
    Uploading,
    ReadyToCommit,
    Active,
    Released,
    Failed,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellError {
    InvalidRequest,
    UnsupportedDomain(CellDomain),
    UnsupportedEntityDomain,
    BudgetExceeded,
    PlacementInUse,
    StaleTicket,
    WrongWorld,
    RevisionChanged,
    ParentChanged,
    NotReady,
    AlreadyCommitted,
    ForeignChildren,
    MissingDependency(String),
    Asset(crate::runtime_asset_loader::StreamingError),
    Hydration(String),
    Render(String),
    Simulation(String),
}
/// Non-owning authored placement identity; resolving never reloads or pins a cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftCellEntityRef {
    pub placement_id: String,
    pub source_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CellMigrationRequest {
    pub source_entity_id: String,
    pub source_generation: u64,
    pub persistent_placement_id: String,
    pub parent: Option<String>,
    pub preserve_world: bool,
}

#[derive(Debug)]
struct Entry {
    ticket: CellTicket,
    phase_started: crate::platform_time::Instant,
    upload_submitted: bool,
    request: CellActivationRequest,
    world_epoch: u64,
    parent_generation: Option<u64>,
    root: RuntimeAssetHandle,
    status: CellStatus,
    error: Option<CellError>,
    prepared: Option<PreparedPrefab>,
    simulation: Option<simulation::PreparedSimulation>,
    render: Vec<PreparedMeshRenderer>,
    logical_handles: BTreeSet<RenderResourceHandle>,
    logical_lease: Option<Arc<()>>,
    activation_epoch: Option<u64>,
    prepared_bytes: usize,
    active_bytes: usize,
    #[cfg(feature = "real-wgpu")]
    upload: Option<crate::geometry_upload::GeometryUploadTicket>,
}
#[derive(Debug)]
pub(super) struct CellState {
    owner: u64,
    stages: BTreeMap<&'static str, (u64, u64, u64)>,
    generation: u64,
    activation_epoch: u64,
    entries: BTreeMap<RuntimeInstanceId, Entry>,
    logical_owners: BTreeMap<RenderResourceHandle, usize>,
}
impl Default for CellState {
    fn default() -> Self {
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
        Self {
            owner: NEXT_OWNER.fetch_add(1, Ordering::Relaxed),
            stages: BTreeMap::new(),
            generation: 0,
            activation_epoch: 0,
            entries: BTreeMap::new(),
            logical_owners: BTreeMap::new(),
        }
    }
}
impl CellState {
    pub(super) fn owns_active(&self, instance: RuntimeInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|e| e.status == CellStatus::Active)
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::archetype::ComponentValue;
    use crate::component_value::RuntimeValue;
    use crate::components::{ComponentTypeId, Hierarchy};
    use crate::runtime_asset::{
        RuntimeAssetIndex, RuntimeAssetLoadState, RuntimePackageMountTable,
    };
    use crate::runtime_entity_hydration::PreparedRuntimeEntity;

    #[test]
    fn prepared_cell_payload_is_observed_until_the_actual_prepared_owner_releases_it() {
        let mut loader = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
            "unused",
            RuntimeAssetIndex::new(vec![], vec![]),
            RuntimePackageMountTable::new(vec![]),
        ));
        let mut text = String::with_capacity(8192);
        text.push_str("prepared-cell-payload");
        let allocation = format!("cpu-storage:{:x}", text.as_ptr() as usize);
        let bytes = text.capacity() as u64;
        let id = SourceEntityId::from("prepared-cell-root");
        let mut prefab_id = String::with_capacity(4096);
        prefab_id.push_str("prefab");
        let reference_allocation = format!("cpu-storage:{:x}", prefab_id.as_ptr() as usize);
        let reference_bytes = prefab_id.capacity() as u64;
        let reference = RuntimeAssetRef {
            id: prefab_id,
            asset_type: "prefab".into(),
            guid: None,
            sub_asset: None,
        };
        let root = RuntimeAssetHandle {
            handle_id: 1,
            asset_guid: "prefab-guid".into(),
            asset_id: "prefab".into(),
            asset_type: "prefab".into(),
            sub_asset_id: None,
            cooked_asset_id: "cooked-prefab".into(),
            bundle_id: "startup".into(),
            runtime_resource_id: None,
            state: RuntimeAssetLoadState::Ready,
            generation: 1,
            ref_count: 1,
            loader_kind: "prefab".into(),
            version: "1".into(),
        };
        let mut prepared_handle = root.clone();
        prepared_handle.asset_guid = String::with_capacity(6144);
        prepared_handle.asset_guid.push_str("prepared-handle-guid");
        let handle_allocation = format!(
            "cpu-storage:{:x}",
            prepared_handle.asset_guid.as_ptr() as usize
        );
        let handle_bytes = prepared_handle.asset_guid.capacity() as u64;
        let mut handles = Vec::with_capacity(32);
        handles.push(prepared_handle);
        let handles_allocation = format!("cpu-storage:{:x}", handles.as_ptr() as usize);
        let handles_bytes = (handles.capacity() * std::mem::size_of::<RuntimeAssetHandle>()) as u64;
        let expected = [
            (allocation.clone(), bytes),
            (reference_allocation, reference_bytes),
            (handle_allocation, handle_bytes),
            (handles_allocation, handles_bytes),
        ];
        let instance = RuntimeInstanceId(1);
        loader.cells.entries.insert(
            instance,
            Entry {
                simulation: None,
                ticket: CellTicket {
                    owner: loader.cells.owner,
                    instance,
                    generation: 1,
                },
                phase_started: crate::platform_time::Instant::now(),
                upload_submitted: false,
                request: CellActivationRequest {
                    schema_version: "cell-activation.v1".into(),
                    placement_id: "cell".into(),
                    prefab: reference.clone(),
                    asset_revision_epoch: 1,
                    required_domains: BTreeSet::new(),
                    parent: None,
                },
                world_epoch: 1,
                parent_generation: None,
                root: root.clone(),
                status: CellStatus::Prepared,
                error: None,
                prepared: Some(PreparedPrefab {
                    prepared: PreparedRuntimeEntities {
                        entities: vec![PreparedRuntimeEntity {
                            source_id: id.clone(),
                            world_id: id.clone(),
                            name: "prepared".into(),
                            kind: "actor".into(),
                            enabled: true,
                            hierarchy: Hierarchy {
                                parent_id: None,
                                sibling_order: 0,
                            },
                            components: vec![ComponentValue::Dynamic {
                                component_type: ComponentTypeId::from("game.payload"),
                                value: RuntimeValue::String(text),
                            }],
                        }],
                        source_to_world: [(id.clone(), id.clone())].into(),
                        root_sources: vec![id.clone()],
                        remapped_reference_count: 0,
                    },
                    prefab_ref: reference,
                    parent_entity: None,
                    target_scene_instance: None,
                    root_source: Some(id),
                    instance_id: instance,
                    handles,
                }),
                render: vec![],
                logical_handles: BTreeSet::new(),
                logical_lease: None,
                activation_epoch: None,
                prepared_bytes: 0,
                active_bytes: 0,
                #[cfg(feature = "real-wgpu")]
                upload: None,
            },
        );
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        loader.observe_memory(&mut sample);
        crate::resource_memory::memory_totals(&sample.allocations).unwrap();
        for (id, bytes) in &expected {
            assert!(
                sample.allocations.iter().any(|a| a.allocation_id == *id
                    && a.bytes == *bytes
                    && a.owner == "queues"
                    && a.roles.contains("queuePayload")),
                "missing prepared backing {id}: {bytes} bytes"
            );
        }
        loader.cells.entries.get_mut(&instance).unwrap().prepared = None;
        let mut after = crate::resource_memory::ResourceMemorySample::new(2, 1, 0);
        loader.observe_memory(&mut after);
        for (id, _) in &expected {
            assert!(
                !after.allocations.iter().any(|a| a.allocation_id == *id),
                "sample must follow actual owner lifetime: {id}"
            );
        }
    }
}
impl RuntimeInstanceLoader {
    fn record_cell_stage(&mut self, ticket: CellTicket, name: &'static str) {
        let e = self.cells.entries.get_mut(&ticket.instance).unwrap();
        let ns = e
            .phase_started
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        e.phase_started = crate::platform_time::Instant::now();
        let value = self.cells.stages.entry(name).or_default();
        value.0 += 1;
        value.1 = value.1.saturating_add(ns);
        value.2 = value.2.max(ns);
    }
    /// Current directly accessible storage; std nodes and worker-local payloads
    /// remain explicitly partial. Stage times include polling/safe-point delay.
    pub fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        self.asset_loader.observe_memory(sample);
        self.observe_project_cell_memory(sample);
        self.observe_instance_memory(sample);
        self.observe_generated_content_usage(sample);
        self.observe_project_storage_usage(sample);
        for (name, &(count, total, max)) in &self.cells.stages {
            for (suffix, value) in [("count", count), ("totalNs", total), ("maxNs", max)] {
                sample
                    .capacities
                    .insert(format!("cell.stage.{name}.{suffix}"), value);
            }
        }
        for e in self.cells.entries.values() {
            for text in [&e.request.schema_version, &e.request.placement_id]
                .into_iter()
                .chain(e.request.parent.iter())
            {
                sample.cpu_string(text, "queues", &["queuePayload"]);
            }
            e.request
                .prefab
                .observe_memory(sample, "queues", "queuePayload");
            e.root.observe_memory(sample, "queues", "queuePayload");
            match &e.error {
                Some(
                    CellError::MissingDependency(text)
                    | CellError::Hydration(text)
                    | CellError::Render(text)
                    | CellError::Simulation(text),
                ) => sample.cpu_string(text, "queues", &["queuePayload"]),
                Some(CellError::Asset(
                    crate::runtime_asset_loader::StreamingError::ReadFailed(text)
                    | crate::runtime_asset_loader::StreamingError::DecodeFailed(text),
                )) => sample.cpu_string(text, "queues", &["queuePayload"]),
                _ => (),
            }
            if let Some(prepared) = &e.prepared {
                prepared.observe_memory(sample);
            }
            if let Some(simulation) = &e.simulation {
                simulation.observe_memory(sample);
            }
            sample.cpu_vec(&e.render, "queues", &["queuePayload"]);
            for prepared in &e.render {
                for geometry in &prepared.geometry {
                    geometry.observe_memory(sample, "queuePayload");
                }
                sample.cpu_vec(&prepared.geometry, "queues", &["queuePayload"]);
                sample.cpu_vec(&prepared.textures, "queues", &["queuePayload"]);
                for texture in &prepared.textures {
                    sample.cpu_vec(
                        &texture.payload.rgba8,
                        "assets",
                        &["decodedCache", "queuePayload"],
                    );
                }
            }
        }
        sample.partial("cpu.owner.queues", "cell.request+root+error+preparedWrapperHandles+preparedRenderVectors+preparedEntityBacking;stdNodesOpaque;otherComponentNestedNotIntegrated");
    }
    fn cell_entry(&self, ticket: CellTicket) -> Result<&Entry, CellError> {
        self.cells
            .entries
            .get(&ticket.instance)
            .filter(|e| e.ticket == ticket)
            .ok_or(CellError::StaleTicket)
    }
    pub(super) fn cell_context_for_project(
        &self,
        ticket: CellTicket,
        world: &World,
    ) -> Result<(), CellError> {
        self.cell_context(ticket, world)
    }
    fn cell_context(&self, ticket: CellTicket, world: &World) -> Result<(), CellError> {
        let e = self.cell_entry(ticket)?;
        if e.world_epoch != world.epoch() {
            return Err(CellError::WrongWorld);
        }
        if e.request.asset_revision_epoch != self.asset_loader.asset_revision_epoch() {
            return Err(CellError::RevisionChanged);
        }
        if matches!(
            e.status,
            CellStatus::Prepared | CellStatus::Uploading | CellStatus::ReadyToCommit
        ) && self.asset_loader.get_handle_state(&e.root) != Some(RuntimeAssetLoadState::Ready)
        {
            return Err(CellError::NotReady);
        }
        if let Some(parent) = &e.request.parent {
            if world.entity_generation(&SourceEntityId::from(parent.clone())) != e.parent_generation
            {
                return Err(CellError::ParentChanged);
            }
        }
        Ok(())
    }
    pub fn cell_status(&self, ticket: CellTicket) -> Result<CellStatus, CellError> {
        Ok(self.cell_entry(ticket)?.status)
    }
    /// Compact on-demand diagnostic; no receipt/entity snapshot or new ownership.
    pub fn cell_lifetime_counts(&self) -> (usize, usize) {
        let active = self
            .cells
            .entries
            .values()
            .filter(|e| e.status == CellStatus::Active)
            .count();
        let live = self
            .cells
            .entries
            .values()
            .filter(|e| !matches!(e.status, CellStatus::Released | CellStatus::Failed))
            .count();
        (active, live)
    }
    pub fn cell_activation_epoch(&self, ticket: CellTicket) -> Result<Option<u64>, CellError> {
        Ok(self.cell_entry(ticket)?.activation_epoch)
    }
    pub fn cell_error(&self, ticket: CellTicket) -> Option<&CellError> {
        self.cell_entry(ticket).ok()?.error.as_ref()
    }

    pub fn request_cell(
        &mut self,
        request: CellActivationRequest,
        world: &World,
    ) -> Result<CellTicket, CellError> {
        if request.schema_version != "cell-activation.v1"
            || request.placement_id.trim().is_empty()
            || request.placement_id.len() > 256
            || request.prefab.asset_type != "prefab"
            || !request.required_domains.contains(&CellDomain::World)
            || !request.required_domains.contains(&CellDomain::Asset)
        {
            return Err(CellError::InvalidRequest);
        }
        if request.asset_revision_epoch != self.asset_loader.asset_revision_epoch() {
            return Err(CellError::RevisionChanged);
        }
        if self.cells.entries.values().any(|e| {
            e.request.placement_id == request.placement_id
                && e.status != CellStatus::Released
                && e.status != CellStatus::Failed
        }) {
            return Err(CellError::PlacementInUse);
        }
        // Retain at most 128 recent receipts; eviction never drops an active cell.
        if self.cells.entries.len() >= 128 {
            self.cells
                .entries
                .retain(|_, e| !matches!(e.status, CellStatus::Released | CellStatus::Failed));
        }
        if self.cells.entries.len() >= 128 {
            return Err(CellError::BudgetExceeded);
        }
        let parent_generation = request
            .parent
            .as_ref()
            .map(|p| {
                world
                    .entity_generation(&SourceEntityId::from(p.clone()))
                    .ok_or(CellError::ParentChanged)
            })
            .transpose()?;
        let root = self
            .asset_loader
            .request_async(&request.prefab)
            .map_err(CellError::Asset)?;
        let instance = self.next_instance_id();
        self.cells.generation += 1;
        let ticket = CellTicket {
            owner: self.cells.owner,
            instance,
            generation: self.cells.generation,
        };
        self.cells.entries.insert(
            instance,
            Entry {
                phase_started: crate::platform_time::Instant::now(),
                upload_submitted: false,
                ticket,
                request,
                world_epoch: world.epoch(),
                parent_generation,
                root,
                status: CellStatus::Loading,
                error: None,
                prepared: None,
                simulation: None,
                render: vec![],
                logical_handles: BTreeSet::new(),
                logical_lease: None,
                activation_epoch: None,
                prepared_bytes: 0,
                active_bytes: 0,
                #[cfg(feature = "real-wgpu")]
                upload: None,
            },
        );
        Ok(ticket)
    }

    /// CPU preparation changes neither World nor gameplay enumeration. Render
    /// requests commit to the same logical manager only after all validation passes.
    pub fn prepare_cell(
        &mut self,
        ticket: CellTicket,
        world: &World,
        frame: u64,
        manager: &mut RenderResourceManager,
    ) -> Result<CellStatus, CellError> {
        self.prepare_cell_context(ticket, world, frame, manager, None)
    }
    pub fn prepare_cell_with_simulation(
        &mut self,
        ticket: CellTicket,
        world: &World,
        frame: u64,
        manager: &mut RenderResourceManager,
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
    ) -> Result<CellStatus, CellError> {
        self.prepare_cell_context(ticket, world, frame, manager, Some((physics, navigation)))
    }
    fn prepare_cell_context(
        &mut self,
        ticket: CellTicket,
        world: &World,
        frame: u64,
        manager: &mut RenderResourceManager,
        simulation: SimulationOwners<'_>,
    ) -> Result<CellStatus, CellError> {
        self.asset_loader.poll_async();
        let result = self.prepare_cell_inner(ticket, world, frame, manager, simulation);
        if let Err(error) = &result {
            if !matches!(
                error,
                CellError::NotReady | CellError::StaleTicket | CellError::AlreadyCommitted
            ) && self
                .cell_entry(ticket)
                .is_ok_and(|e| e.status == CellStatus::Loading)
            {
                let root = self.cells.entries[&ticket.instance].root.clone();
                let _ = self.asset_loader.release(&root);
                let charge = self.cells.entries[&ticket.instance].prepared_bytes;
                self.asset_loader.release_prepared_cpu(charge);
                let e = self.cells.entries.get_mut(&ticket.instance).unwrap();
                e.status = CellStatus::Failed;
                e.error = Some(error.clone());
                e.prepared_bytes = 0;
            }
        }
        result
    }
    fn prepare_cell_inner(
        &mut self,
        ticket: CellTicket,
        world: &World,
        frame: u64,
        manager: &mut RenderResourceManager,
        owners: SimulationOwners<'_>,
    ) -> Result<CellStatus, CellError> {
        self.cell_context(ticket, world)?;
        let entry = self.cell_entry(ticket)?;
        if entry.status != CellStatus::Loading {
            return Err(if entry.status == CellStatus::Active {
                CellError::AlreadyCommitted
            } else {
                CellError::NotReady
            });
        }
        let root = entry.root.clone();
        let request = entry.request.clone();
        match self.asset_loader.get_handle_state(&root) {
            Some(RuntimeAssetLoadState::Loading) => return Err(CellError::NotReady),
            Some(RuntimeAssetLoadState::Ready) => (),
            _ => {
                return Err(CellError::Asset(
                    self.asset_loader
                        .streaming_error(&root)
                        .cloned()
                        .unwrap_or(crate::runtime_asset_loader::StreamingError::Cancelled),
                ))
            }
        }
        let payload = self
            .asset_loader
            .decoded_payload(&root)
            .ok_or(CellError::InvalidRequest)?;
        let RuntimeDecodedPayload::Prefab(prefab) = payload.as_ref() else {
            return Err(CellError::InvalidRequest);
        };
        if prefab.id != root.asset_id || prefab.entities.is_empty() {
            return Err(CellError::InvalidRequest);
        }
        if prefab.entities.len() > 16_384 {
            return Err(CellError::BudgetExceeded);
        }
        let mut refs = Vec::new();
        let mut renderers = Vec::new();
        let mut vegetation_clusters = Vec::new();
        for entity in &prefab.entities {
            let Some(crate::spatial_contract::RuntimeSpatial::World3D { mesh_renderer, .. }) =
                &entity.spatial
            else {
                return Err(CellError::UnsupportedEntityDomain);
            };
            if let Some(renderer) = mesh_renderer {
                if !request.required_domains.contains(&CellDomain::Render) {
                    return Err(CellError::InvalidRequest);
                }
                refs.extend(
                    std::iter::once(&renderer.mesh)
                        .chain(renderer.materials.values())
                        .map(|r| r.0.clone()),
                );
                renderers.push(renderer.clone());
            }
            for component in &entity.components {
                let ty = component.component_type.as_str();
                if ty == "engine.model_lod3d" {
                    if !request.required_domains.contains(&CellDomain::Render) {
                        return Err(CellError::InvalidRequest);
                    }
                    let lod: crate::model_lod3d::ModelLod3D =
                        serde_json::from_value(component.data.clone())
                            .map_err(|_| CellError::InvalidRequest)?;
                    lod.validate().map_err(|_| CellError::InvalidRequest)?;
                    for level in lod.levels {
                        refs.extend(
                            std::iter::once(&level.renderer.mesh)
                                .chain(level.renderer.materials.values())
                                .map(|r| r.0.clone()),
                        );
                        renderers.push(level.renderer);
                    }
                }
                if ty == "engine.vegetation3d" {
                    if !request.required_domains.contains(&CellDomain::Render) {
                        return Err(CellError::InvalidRequest);
                    }
                    let binding: crate::terrain3d::VegetationBinding3D =
                        serde_json::from_value(component.data.clone())
                            .map_err(|_| CellError::InvalidRequest)?;
                    let reference = RuntimeAssetRef {
                        id: binding.cluster_asset.asset_id.clone(),
                        asset_type: "vegetation3d".into(),
                        guid: Some(binding.cluster_asset.asset_guid.clone()),
                        sub_asset: None,
                    };
                    let bytes = self
                        .asset_loader
                        .closure_reference_bytes(&root, &reference)
                        .ok_or_else(|| CellError::MissingDependency(reference.id.clone()))?;
                    let asset: crate::terrain3d::VegetationAsset3D = serde_json::from_value(
                        crate::spatial_contract::strict_json(&bytes)
                            .map_err(|_| CellError::InvalidRequest)?,
                    )
                    .map_err(|_| CellError::InvalidRequest)?;
                    asset
                        .validate_binding(&binding)
                        .map_err(|_| CellError::InvalidRequest)?;
                    refs.push(reference);
                    for cluster in asset.clusters {
                        refs.extend(
                            cluster
                                .mesh_levels()
                                .map(|(_, mesh)| mesh.reference.clone()),
                        );
                        refs.push(cluster.material.reference.clone());
                        vegetation_clusters.push(cluster);
                    }
                }
                if matches!(ty, "engine.body3d" | "engine.motor3d" | "engine.joint3d")
                    && !request.required_domains.contains(&CellDomain::Physics)
                    || matches!(
                        ty,
                        "engine.navigation3d"
                            | "engine.navigation_agent3d"
                            | "engine.navigation_obstacle3d"
                    ) && !request.required_domains.contains(&CellDomain::Navigation)
                {
                    return Err(CellError::InvalidRequest);
                }
                if matches!(
                    ty,
                    "engine.character3d"
                        | "engine.body3d"
                        | "engine.navigation3d"
                        | "engine.audio_source"
                ) {
                    collect_simulation_refs(&component.data, &mut refs)?;
                }
            }
        }
        let root_bytes = self
            .asset_loader
            .ready_prefab_copy_charge(&root)
            .ok_or(CellError::NotReady)?;
        let mut prepared_bytes = root_bytes;
        let mut retained_bytes = root_bytes;
        let mut active_bytes = root_bytes;
        let mut mesh_scratch = 0;
        for reference in refs {
            if !self.asset_loader.closure_contains_ready(&root, &reference) {
                return Err(CellError::MissingDependency(reference.id));
            }
            let (peak, retained) = self
                .asset_loader
                .prepared_reference_charges(&reference)
                .ok_or(CellError::NotReady)?;
            // prepare_mesh_renderer consumes one mesh at a time. Every final
            // geometry copy survives, but its typed clone/source reread dies
            // before the next renderer. Materials may coexist with that mesh;
            // keep their existing summed allowance.
            let concurrent = if reference.asset_type == "mesh" {
                mesh_scratch = mesh_scratch.max(
                    peak.checked_sub(retained)
                        .ok_or(CellError::BudgetExceeded)?,
                );
                retained
            } else {
                peak
            };
            prepared_bytes = prepared_bytes
                .checked_add(concurrent)
                .ok_or(CellError::BudgetExceeded)?;
            retained_bytes = retained_bytes
                .checked_add(retained)
                .ok_or(CellError::BudgetExceeded)?;
            if matches!(
                reference.asset_type.as_str(),
                "mesh" | "material" | "texture"
            ) {
                active_bytes = active_bytes
                    .checked_add(retained)
                    .ok_or(CellError::BudgetExceeded)?;
            }
        }
        prepared_bytes = prepared_bytes
            .checked_add(mesh_scratch)
            .ok_or(CellError::BudgetExceeded)?;
        self.asset_loader
            .reserve_prepared_cpu(prepared_bytes)
            .map_err(CellError::Asset)?;
        self.cells
            .entries
            .get_mut(&ticket.instance)
            .unwrap()
            .prepared_bytes = prepared_bytes;
        let prepared = PreparedRuntimeEntities::prepare_prefab_instance(
            &prefab.entities,
            ticket.instance,
            prefab.root_entity_id.as_deref(),
            request.parent.clone().map(SourceEntityId::from),
            world,
        )
        .map_err(|e| CellError::Hydration(format!("{e:?}")))?;
        let simulation = simulation::prepare(
            world,
            &prepared,
            &mut self.asset_loader,
            &request.required_domains,
            owners,
        )?;
        let mut staged = manager.clone();
        let mut render = Vec::new();
        for cluster in vegetation_clusters {
            for (_, mesh_reference) in cluster.mesh_levels() {
                let mesh = RuntimeRenderAssetProducer
                    .prepare_cooked_from_package(
                        frame,
                        mesh_reference,
                        &mut self.asset_loader,
                        &mut staged,
                    )
                    .map_err(|e| CellError::Render(e.to_string()))?;
                let crate::render_contract::RenderAssetData::Mesh(data) = &mesh.asset.asset else {
                    return Err(CellError::InvalidRequest);
                };
                if (0..3).any(|i| {
                    data.bounds.min[i] < cluster.mesh_bounds[0][i] - 0.0001
                        || data.bounds.max[i] > cluster.mesh_bounds[1][i] + 0.0001
                }) {
                    return Err(CellError::InvalidRequest);
                }
                renderers.push(crate::spatial_contract::MeshRenderer3D {
                    schema_version: "mesh-renderer3d.v1".into(),
                    mesh: crate::spatial_contract::SpatialAssetRef(
                        mesh_reference.reference.clone(),
                    ),
                    materials: data
                        .material_slots
                        .iter()
                        .map(|slot| {
                            (
                                slot.clone(),
                                crate::spatial_contract::SpatialAssetRef(
                                    cluster.material.reference.clone(),
                                ),
                            )
                        })
                        .collect(),
                    visible: true,
                    layers: Vec::new(),
                });
            }
        }
        for renderer in renderers {
            render.push(
                RuntimeRenderAssetProducer
                    .prepare_mesh_renderer(frame, &renderer, &mut self.asset_loader, &mut staged)
                    .map_err(|e| CellError::Render(e.to_string()))?,
            );
        }
        let logical_handles: BTreeSet<_> = render
            .iter()
            .flat_map(|r| r.geometry.iter().flat_map(|g| g.resource_handles()))
            .collect();
        let lease = staged.lease_resources(logical_handles.iter().copied());
        for handle in &logical_handles {
            if !self.cells.logical_owners.contains_key(handle) {
                staged.retain_resource_ownership(*handle)
                    .map_err(|e| CellError::Render(format!("{e:?}")))?;
            }
        }
        self.asset_loader
            .release_prepared_cpu(prepared_bytes - retained_bytes);
        *manager = staged;
        for handle in &logical_handles {
            *self.cells.logical_owners.entry(*handle).or_default() += 1;
        }
        self.record_cell_stage(ticket, "loadingAndPrepare");
        let entry = self.cells.entries.get_mut(&ticket.instance).unwrap();
        entry.prepared = Some(PreparedPrefab {
            prepared,
            prefab_ref: request.prefab,
            parent_entity: request.parent.map(SourceEntityId::from),
            target_scene_instance: None,
            root_source: prefab_root_source(prefab),
            instance_id: ticket.instance,
            handles: vec![root],
        });
        entry.simulation = simulation;
        entry.render = render;
        entry.logical_handles = logical_handles;
        entry.logical_lease = Some(lease);
        entry.prepared_bytes = retained_bytes;
        entry.active_bytes = active_bytes;
        entry.status = if request.required_domains.contains(&CellDomain::Render) {
            CellStatus::Prepared
        } else {
            CellStatus::ReadyToCommit
        };
        Ok(entry.status)
    }

    fn commit_cell_inner(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        mut owners: SimulationOwners<'_>,
    ) -> Result<RuntimePrefabInstance, CellError> {
        self.cell_context(ticket, world)?;
        let entry = self.cell_entry(ticket)?;
        if entry.status != CellStatus::ReadyToCommit {
            return Err(CellError::NotReady);
        }
        let prepared = entry.prepared.as_ref().ok_or(CellError::NotReady)?;
        if prepared
            .prepared
            .entities
            .iter()
            .any(|e| world.runtime_id_for_source(&e.world_id).is_some())
        {
            return Err(CellError::Hydration(
                "prepared entity identity is already occupied".into(),
            ));
        }
        world
            .validate_prepared_spatial(&prepared.prepared.entities)
            .map_err(|e| CellError::Hydration(e.message))?;
        world
            .check_spatial_budget(0, 1)
            .map_err(|_| CellError::BudgetExceeded)?;
        let activated = entry
            .simulation
            .as_ref()
            .map(|ack| {
                simulation::activate(
                    world,
                    &prepared.prepared,
                    ack,
                    owners.as_mut().map(|(p, n)| (&mut **p, &mut **n)),
                )
            })
            .transpose()?;
        let prepared = self
            .cells
            .entries
            .get_mut(&ticket.instance)
            .unwrap()
            .prepared
            .take()
            .unwrap();
        let report =
            PrefabInstantiateReport::new(self.next_request_id(), prepared.prefab_ref.id.clone())
                .with_report_level(self.report_level);
        world.charge_spatial_budget(0, 1);
        let (instance, _) = self.commit_prepared_prefab(prepared, report, world);
        world.propagate_spatial();
        if let Some(active) = activated {
            let ack = self
                .cells
                .entries
                .get_mut(&ticket.instance)
                .unwrap()
                .simulation
                .take()
                .unwrap();
            simulation::finish(world, ack, active, owners);
        }

        self.record_cell_stage(ticket, "commit");
        self.cells.activation_epoch += 1;
        let e = self.cells.entries.get_mut(&ticket.instance).unwrap();
        self.asset_loader
            .release_prepared_cpu(e.prepared_bytes - e.active_bytes);
        e.prepared_bytes = e.active_bytes;
        e.status = CellStatus::Active;
        e.activation_epoch = Some(self.cells.activation_epoch);
        Ok(instance)
    }
    pub fn commit_cell(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
    ) -> Result<RuntimePrefabInstance, CellError> {
        if self
            .cell_entry(ticket)?
            .request
            .required_domains
            .contains(&CellDomain::Render)
        {
            return Err(CellError::NotReady);
        }
        self.commit_cell_inner(ticket, world, None)
    }
    pub fn commit_cell_with_simulation(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
    ) -> Result<RuntimePrefabInstance, CellError> {
        if self
            .cell_entry(ticket)?
            .request
            .required_domains
            .contains(&CellDomain::Render)
        {
            return Err(CellError::NotReady);
        }
        self.commit_cell_inner(ticket, world, Some((physics, navigation)))
    }
    pub fn release_cell_with_simulation(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
    ) -> Result<(), CellError> {
        #[cfg(feature = "real-wgpu")]
        if self.cell_entry(ticket)?.upload.is_some() {
            return Err(CellError::Render(
                "release through the owning GPU backend".into(),
            ));
        }
        self.validate_cell_release(ticket, world, manager)?;
        let ids = self
            .prefab_instances
            .get(&ticket.instance)
            .map(|i| i.source_to_world_entity.values().cloned().collect())
            .unwrap_or_default();
        self.release_cell_inner(ticket, world, manager, frame)?;
        simulation::retire(world, &ids, Some((physics, navigation)))
    }

    fn require_simulation_release_owner(&self, ticket: CellTicket) -> Result<(), CellError> {
        let entry = self.cell_entry(ticket)?;
        if entry.status == CellStatus::Active {
            for domain in [CellDomain::Physics, CellDomain::Navigation] {
                if entry.request.required_domains.contains(&domain) {
                    return Err(CellError::UnsupportedDomain(domain));
                }
            }
        }
        Ok(())
    }
    pub fn resolve_soft_cell_entity(
        &self,
        reference: &SoftCellEntityRef,
        world: &World,
    ) -> Option<RuntimeEntityId> {
        let entry = self.cells.entries.values().find(|e| {
            e.request.placement_id == reference.placement_id
                && e.status == CellStatus::Active
                && e.world_epoch == world.epoch()
        })?;
        let instance = self.prefab_instances.get(&entry.ticket.instance)?;
        let runtime = *instance
            .source_to_runtime_entity
            .get(&SourceEntityId::from(reference.source_entity_id.clone()))?;
        let source = instance
            .source_to_world_entity
            .get(&SourceEntityId::from(reference.source_entity_id.clone()))?;
        (world.runtime_id_for_source(source) == Some(runtime)).then_some(runtime)
    }

    pub(super) fn cell_placement(&self, ticket: CellTicket, world: &World) -> Option<&str> {
        let entry = self.cell_entry(ticket).ok()?;
        (entry.world_epoch == world.epoch()).then_some(entry.request.placement_id.as_str())
    }

    fn validate_cell_logical_resources(
        &self,
        ticket: CellTicket,
        manager: &RenderResourceManager,
    ) -> Result<(), CellError> {
        let entry = self.cell_entry(ticket)?;
        for handle in &entry.logical_handles {
            if !self.cells.logical_owners.contains_key(handle)
                || entry
                    .logical_lease
                    .as_ref()
                    .is_none_or(|lease| !manager.native_lease_valid(*handle, lease))
            {
                return Err(CellError::Render(
                    "logical resource owner changed before release".into(),
                ));
            }
        }
        Ok(())
    }

    fn validate_cell_release(
        &self,
        ticket: CellTicket,
        world: &World,
        manager: &RenderResourceManager,
    ) -> Result<(), CellError> {
        let entry = self.cell_entry(ticket)?;
        if entry.world_epoch != world.epoch() {
            return Err(CellError::WrongWorld);
        }
        if entry.status == CellStatus::Released {
            return Err(CellError::StaleTicket);
        }
        self.validate_cell_logical_resources(ticket, manager)?;
        if entry.status == CellStatus::Active {
            let instance = self
                .prefab_instances
                .get(&ticket.instance)
                .ok_or(CellError::StaleTicket)?;
            if instance.source_to_world_entity.iter().any(|(source, id)| {
                world.runtime_id_for_source(id).is_some_and(|current| {
                    instance.source_to_runtime_entity.get(source) != Some(&current)
                })
            }) {
                return Err(CellError::StaleTicket);
            }
            world
                .check_spatial_budget(0, 1)
                .map_err(|_| CellError::BudgetExceeded)?;
            let owned: BTreeSet<_> = instance.source_to_world_entity.values().cloned().collect();
            if world.entity_ids().into_iter().any(|id| {
                !owned.contains(id)
                    && world
                        .hierarchy(id)
                        .and_then(|h| h.parent_id)
                        .is_some_and(|p| owned.contains(&p))
            }) {
                return Err(CellError::ForeignChildren);
            }
        }
        Ok(())
    }
    fn release_cell_inner(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
    ) -> Result<(), CellError> {
        self.validate_cell_release(ticket, world, manager)?;
        let entry = self.cell_entry(ticket)?;
        if entry.status == CellStatus::Active {
            world.charge_spatial_budget(0, 1);
            self.despawn_prefab_instance_unchecked(ticket.instance, world);
        } else {
            let _ = self.asset_loader.release(&entry.root.clone());
        }
        self.release_cell_resources(ticket, manager, frame)
    }
    fn release_cell_resources(
        &mut self,
        ticket: CellTicket,
        manager: &mut RenderResourceManager,
        frame: u64,
    ) -> Result<(), CellError> {
        let entry = self.cells.entries.get_mut(&ticket.instance).unwrap();
        self.asset_loader.release_prepared_cpu(entry.prepared_bytes);
        entry.prepared_bytes = 0;
        entry.logical_lease = None;
        for handle in &entry.logical_handles {
            let owners = self.cells.logical_owners.get_mut(handle).unwrap();
            *owners -= 1;
            if *owners == 0 {
                self.cells.logical_owners.remove(handle);
                manager
                    .release_resource_ownership(RenderResourceReleaseRequest {
                        handle: *handle,
                        frame_index: frame,
                    })
                    .map_err(|e| CellError::Render(format!("{e:?}")))?;
            }
        }
        entry.prepared = None;
        entry.simulation = None;
        entry.render.clear();
        entry.logical_handles.clear();
        entry.status = CellStatus::Released;
        Ok(())
    }
    /// A World replacement invalidates the old entity map. Retire only its resources;
    /// never run despawn by authoring identity against the replacement World.
    pub(super) fn discard_obsolete_project_cell(
        &mut self,
        ticket: CellTicket,
        world: &World,
        manager: &mut RenderResourceManager,
        frame: u64,
        #[cfg(feature = "real-wgpu")] backend: Option<
            &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        >,
    ) -> Result<(), CellError> {
        let entry = self.cell_entry(ticket)?;
        if entry.world_epoch == world.epoch() {
            return Err(CellError::InvalidRequest);
        }
        self.validate_cell_logical_resources(ticket, manager)?;
        #[cfg(feature = "real-wgpu")]
        if let Some(upload) = self.cell_entry(ticket)?.upload {
            backend
                .ok_or_else(|| CellError::Render("render owner unavailable".into()))?
                .release_geometry_upload(upload)
                .map_err(CellError::Render)?;
            self.cells.entries.get_mut(&ticket.instance).unwrap().upload = None;
        }
        if let Some(instance) = self.prefab_instances.remove(&ticket.instance) {
            self.release_handles(instance.owned_asset_handles);
        } else {
            let _ = self
                .asset_loader
                .release(&self.cell_entry(ticket)?.root.clone());
        }
        self.release_cell_resources(ticket, manager, frame)
    }
    #[cfg(feature = "real-wgpu")]
    pub(super) fn project_cell_has_upload(&self, ticket: CellTicket) -> bool {
        self.cell_entry(ticket).is_ok_and(|e| e.upload.is_some())
    }
    pub fn release_cell(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
    ) -> Result<(), CellError> {
        self.require_simulation_release_owner(ticket)?;
        #[cfg(feature = "real-wgpu")]
        if self.cell_entry(ticket)?.upload.is_some() {
            return Err(CellError::Render(
                "release through the owning GPU backend".into(),
            ));
        }
        self.release_cell_inner(ticket, world, manager, frame)
    }

    pub fn migrate_cell_subtree(
        &mut self,
        ticket: CellTicket,
        request: CellMigrationRequest,
        world: &mut World,
        manager: &mut RenderResourceManager,
    ) -> Result<CellTicket, CellError> {
        #[cfg(feature = "real-wgpu")]
        if self.cell_entry(ticket)?.upload.is_some() {
            return Err(CellError::Render(
                "migrate through the owning GPU backend".into(),
            ));
        }
        self.migrate_cell_inner(
            ticket,
            request,
            world,
            manager,
            None,
            #[cfg(feature = "real-wgpu")]
            None,
        )
    }

    pub fn migrate_cell_subtree_with_simulation(
        &mut self,
        ticket: CellTicket,
        request: CellMigrationRequest,
        world: &mut World,
        manager: &mut RenderResourceManager,
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
    ) -> Result<CellTicket, CellError> {
        #[cfg(feature = "real-wgpu")]
        if self.cell_entry(ticket)?.upload.is_some() {
            return Err(CellError::Render(
                "migrate through owning GPU backend".into(),
            ));
        }
        self.migrate_cell_inner(
            ticket,
            request,
            world,
            manager,
            Some((physics, navigation)),
            #[cfg(feature = "real-wgpu")]
            None,
        )
    }

    fn migrate_cell_inner(
        &mut self,
        ticket: CellTicket,
        request: CellMigrationRequest,
        world: &mut World,
        manager: &mut RenderResourceManager,
        simulation_owners: SimulationOwners<'_>,
        #[cfg(feature = "real-wgpu")] upload: Option<crate::geometry_upload::GeometryUploadTicket>,
    ) -> Result<CellTicket, CellError> {
        if simulation_owners.is_none() {
            self.require_simulation_release_owner(ticket)?;
        }
        self.cell_context(ticket, world)?;
        self.validate_cell_logical_resources(ticket, manager)?;
        let entry = self.cell_entry(ticket)?;
        if entry.status != CellStatus::Active {
            return Err(CellError::NotReady);
        }
        if request.persistent_placement_id.trim().is_empty()
            || request.persistent_placement_id.len() > 256
        {
            return Err(CellError::InvalidRequest);
        }
        if self.cells.entries.len() >= 128 {
            return Err(CellError::BudgetExceeded);
        }
        if self.cells.entries.values().any(|e| {
            e.request.placement_id == request.persistent_placement_id
                && !matches!(e.status, CellStatus::Released | CellStatus::Failed)
        }) {
            return Err(CellError::PlacementInUse);
        }
        let source = SourceEntityId::from(request.source_entity_id);
        let original = self
            .prefab_instances
            .get(&ticket.instance)
            .ok_or(CellError::StaleTicket)?;
        if original.source_to_world_entity.iter().any(|(source, id)| {
            world.runtime_id_for_source(id)
                != original.source_to_runtime_entity.get(source).copied()
        }) {
            return Err(CellError::StaleTicket);
        }
        let root_world = original
            .source_to_world_entity
            .get(&source)
            .ok_or(CellError::StaleTicket)?
            .clone();
        if world.entity_generation(&root_world) != Some(request.source_generation) {
            return Err(CellError::StaleTicket);
        }
        let root_runtime = original.source_to_runtime_entity[&source];
        let mut moving = BTreeSet::from([root_world.clone()]);
        loop {
            let before = moving.len();
            for id in original.source_to_world_entity.values() {
                if world
                    .hierarchy(id)
                    .and_then(|h| h.parent_id)
                    .is_some_and(|p| moving.contains(&p))
                {
                    moving.insert(id.clone());
                }
            }
            if moving.len() == before {
                break;
            }
        }
        if world.entity_ids().into_iter().any(|id| {
            !moving.contains(id)
                && world
                    .hierarchy(id)
                    .and_then(|h| h.parent_id)
                    .is_some_and(|p| moving.contains(&p))
        }) {
            return Err(CellError::ForeignChildren);
        }
        if let Some((physics, navigation)) = simulation_owners {
            if !request.preserve_world {
                return Err(CellError::InvalidRequest);
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                physics
                    .validate_cell_migration(world, &moving)
                    .map_err(|e| CellError::Simulation(e.to_string()))?;
                navigation
                    .validate_cell_migration(world, &moving)
                    .map_err(|e| CellError::Simulation(e.to_string()))?;
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = (physics, navigation);
                return Err(CellError::UnsupportedDomain(CellDomain::Physics));
            }
        }
        let parent = request.parent.map(SourceEntityId::from);
        let parent_generation = parent
            .as_ref()
            .map(|p| world.entity_generation(p).ok_or(CellError::ParentChanged))
            .transpose()?;
        let mut batch = crate::project_runtime_session::ProjectRuntimeMutationBuffer::new();
        batch.spatial_command(
            root_world,
            request.source_generation,
            project_runtime_sdk::ProjectRuntimeSpatialIntent::Reparent {
                parent_entity_id: parent.as_ref().map(|p| p.as_str().to_string()),
                parent_generation: parent_generation.unwrap_or(0),
                preserve_world: request.preserve_world,
            },
        );
        let batch = batch
            .prepare(world)
            .map_err(|e| CellError::Hydration(e.message))?;
        let root = entry.root.clone();
        let prepared_bytes = entry.prepared_bytes;
        let active_bytes = entry.active_bytes;
        let mut activation = entry.request.clone();
        activation.placement_id = request.persistent_placement_id;
        activation.parent = parent.as_ref().map(|p| p.as_str().to_string());
        self.asset_loader
            .reserve_prepared_cpu(prepared_bytes)
            .map_err(CellError::Asset)?;
        let retained = match self.asset_loader.retain_handle(&root) {
            Ok(h) => h,
            Err(e) => {
                self.asset_loader.release_prepared_cpu(prepared_bytes);
                return Err(CellError::Asset(e));
            }
        };
        let entry = self.cell_entry(ticket).unwrap();
        let render = entry.render.clone();
        let logical_handles = entry.logical_handles.clone();
        if let Err(e) = batch.commit(world) {
            let _ = self.asset_loader.release(&retained);
            self.asset_loader.release_prepared_cpu(prepared_bytes);
            return Err(CellError::Hydration(e.message));
        }
        // No fallible work follows World commit. Identity is preserved; ownership
        // moves to an ordinary persistent instance with its exact asset closure.
        let instance = self.next_instance_id();
        self.cells.generation += 1;
        self.cells.activation_epoch += 1;
        let persistent = CellTicket {
            owner: self.cells.owner,
            instance,
            generation: self.cells.generation,
        };
        let original = self.prefab_instances.get_mut(&ticket.instance).unwrap();
        let source_to_world_entity: BTreeMap<_, _> = original
            .source_to_world_entity
            .iter()
            .filter(|(_, id)| moving.contains(*id))
            .map(|(s, id)| (s.clone(), id.clone()))
            .collect();
        let mut source_to_runtime_entity = BTreeMap::new();
        for source in source_to_world_entity.keys() {
            original.source_to_world_entity.remove(source);
            source_to_runtime_entity.insert(
                source.clone(),
                original.source_to_runtime_entity.remove(source).unwrap(),
            );
        }
        if original.root_entity == Some(root_runtime) {
            original.root_entity = None;
        }
        self.prefab_instances.insert(
            instance,
            RuntimePrefabInstance {
                instance_id: instance,
                prefab_asset_guid: retained.asset_guid.clone(),
                root_entity: Some(root_runtime),
                parent_entity: parent,
                target_scene_instance: None,
                source_to_world_entity,
                source_to_runtime_entity,
                owned_asset_handles: vec![retained.clone()],
                state: RuntimeInstanceState::Active,
            },
        );
        let logical_lease = Some(manager.lease_resources(logical_handles.iter().copied()));
        for handle in &logical_handles {
            // Migration retains a subset already held by the source cell; it does
            // not introduce another manager-level owner for this cells collection.
            debug_assert!(self.cells.logical_owners.contains_key(handle));
            *self.cells.logical_owners.entry(*handle).or_default() += 1;
        }
        self.cells.entries.insert(
            instance,
            Entry {
                phase_started: crate::platform_time::Instant::now(),
                upload_submitted: false,
                ticket: persistent,
                request: activation,
                world_epoch: world.epoch(),
                parent_generation,
                root: retained,
                status: CellStatus::Active,
                error: None,
                prepared: None,
                simulation: None,
                render,
                logical_handles,
                logical_lease,
                activation_epoch: Some(self.cells.activation_epoch),
                prepared_bytes,
                active_bytes,
                #[cfg(feature = "real-wgpu")]
                upload,
            },
        );
        Ok(persistent)
    }
}

#[cfg(feature = "real-wgpu")]
impl RuntimeInstanceLoader {
    pub fn migrate_cell_subtree_gpu(
        &mut self,
        ticket: CellTicket,
        request: CellMigrationRequest,
        world: &mut World,
        manager: &mut RenderResourceManager,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
    ) -> Result<CellTicket, CellError> {
        let entry = self.cell_entry(ticket)?;
        if entry.status != CellStatus::Active {
            return Err(CellError::NotReady);
        }
        let geometry: Vec<_> = entry
            .render
            .iter()
            .flat_map(|r| r.geometry.iter().cloned())
            .collect();
        let textures: Vec<_> = entry
            .render
            .iter()
            .flat_map(|r| r.textures.iter().cloned())
            .collect();
        let upload = backend
            .begin_geometry_upload(&geometry, &textures)
            .map_err(CellError::Render)?;
        if backend
            .geometry_upload_state(upload)
            .map_err(CellError::Render)?
            != crate::geometry_upload::GeometryUploadState::Ready
        {
            let _ = backend.release_geometry_upload(upload);
            return Err(CellError::NotReady);
        }
        let result = self.migrate_cell_inner(ticket, request, world, manager, None, Some(upload));
        if result.is_err() {
            let _ = backend.release_geometry_upload(upload);
        }
        result
    }
    pub fn migrate_cell_subtree_gpu_with_simulation(
        &mut self,
        ticket: CellTicket,
        request: CellMigrationRequest,
        world: &mut World,
        manager: &mut RenderResourceManager,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
    ) -> Result<CellTicket, CellError> {
        let entry = self.cell_entry(ticket)?;
        if entry.status != CellStatus::Active {
            return Err(CellError::NotReady);
        }
        let geometry: Vec<_> = entry
            .render
            .iter()
            .flat_map(|r| r.geometry.iter().cloned())
            .collect();
        let textures: Vec<_> = entry
            .render
            .iter()
            .flat_map(|r| r.textures.iter().cloned())
            .collect();
        let upload = backend
            .begin_geometry_upload(&geometry, &textures)
            .map_err(CellError::Render)?;
        let state = backend
            .geometry_upload_state(upload)
            .map_err(CellError::Render);
        if !matches!(
            state,
            Ok(crate::geometry_upload::GeometryUploadState::Ready)
        ) {
            let _ = backend.release_geometry_upload(upload);
            return state.map(|_| ()).and(Err(CellError::NotReady));
        }
        let result = self.migrate_cell_inner(
            ticket,
            request,
            world,
            manager,
            Some((physics, navigation)),
            Some(upload),
        );
        if result.is_err() {
            let _ = backend.release_geometry_upload(upload);
        }
        result
    }
    pub fn begin_cell_upload(
        &mut self,
        ticket: CellTicket,
        world: &World,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
    ) -> Result<(), CellError> {
        self.cell_context(ticket, world)?;
        let entry = self.cell_entry(ticket)?;
        if entry.status != CellStatus::Prepared || entry.upload.is_some() {
            return Err(CellError::NotReady);
        }
        let geometry: Vec<_> = entry
            .render
            .iter()
            .flat_map(|r| r.geometry.iter().cloned())
            .collect();
        let textures: Vec<_> = entry
            .render
            .iter()
            .flat_map(|r| r.textures.iter().cloned())
            .collect();
        self.record_cell_stage(ticket, "preparedQueue");
        let upload = backend
            .begin_geometry_upload(&geometry, &textures)
            .map_err(CellError::Render)?;
        self.record_cell_stage(ticket, "beginUploadCpu");
        let e = self.cells.entries.get_mut(&ticket.instance).unwrap();
        e.upload = Some(upload);
        e.status = CellStatus::Uploading;
        Ok(())
    }
    pub fn commit_cell_gpu(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
    ) -> Result<RuntimePrefabInstance, CellError> {
        self.commit_cell_gpu_context(ticket, world, backend, None)
    }
    pub fn commit_cell_gpu_with_simulation(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
    ) -> Result<RuntimePrefabInstance, CellError> {
        self.commit_cell_gpu_context(ticket, world, backend, Some((physics, navigation)))
    }
    fn commit_cell_gpu_context(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        owners: SimulationOwners<'_>,
    ) -> Result<RuntimePrefabInstance, CellError> {
        self.cell_context(ticket, world)?;
        let e = self.cell_entry(ticket)?;
        if e.status != CellStatus::Uploading {
            return Err(CellError::NotReady);
        }
        let upload = e.upload.ok_or(CellError::NotReady)?;
        match backend
            .geometry_upload_state(upload)
            .map_err(CellError::Render)?
        {
            crate::geometry_upload::GeometryUploadState::Ready => {
                let stage = if self.cell_entry(ticket)?.upload_submitted {
                    "gpuCompletionAndPoll"
                } else {
                    "uploadSlicesAndCompletion"
                };
                self.record_cell_stage(ticket, stage);
            }
            crate::geometry_upload::GeometryUploadState::Submitted => {
                if !self.cell_entry(ticket)?.upload_submitted {
                    self.record_cell_stage(ticket, "uploadSlices");
                    self.cells
                        .entries
                        .get_mut(&ticket.instance)
                        .unwrap()
                        .upload_submitted = true;
                }
                return Err(CellError::NotReady);
            }
            crate::geometry_upload::GeometryUploadState::Abandoned => {
                return Err(CellError::Render("device abandoned".into()))
            }
            _ => return Err(CellError::NotReady),
        }
        self.cells.entries.get_mut(&ticket.instance).unwrap().status = CellStatus::ReadyToCommit;
        let result = self.commit_cell_inner(ticket, world, owners);
        if result.is_err() {
            self.cells.entries.get_mut(&ticket.instance).unwrap().status = CellStatus::Uploading;
        }
        result
    }
    pub fn release_cell_gpu_with_simulation(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
    ) -> Result<(), CellError> {
        self.validate_cell_release(ticket, world, manager)?;
        let ids = self
            .prefab_instances
            .get(&ticket.instance)
            .map(|i| i.source_to_world_entity.values().cloned().collect())
            .unwrap_or_default();
        self.release_cell_gpu_context(ticket, world, manager, frame, backend)?;
        simulation::retire(world, &ids, Some((physics, navigation)))
    }
    pub fn release_cell_gpu(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
    ) -> Result<(), CellError> {
        self.require_simulation_release_owner(ticket)?;
        self.release_cell_gpu_context(ticket, world, manager, frame, backend)
    }
    fn release_cell_gpu_context(
        &mut self,
        ticket: CellTicket,
        world: &mut World,
        manager: &mut RenderResourceManager,
        frame: u64,
        backend: &mut (dyn crate::engine_rhi::EngineRhiBackend + 'static),
    ) -> Result<(), CellError> {
        // Validate every owner before touching physical resources or World.
        self.validate_cell_release(ticket, world, manager)?;
        let e = self.cell_entry(ticket)?;
        if let Some(upload) = e.upload {
            backend
                .release_geometry_upload(upload)
                .map_err(CellError::Render)?;
        }
        self.cells.entries.get_mut(&ticket.instance).unwrap().upload = None;
        self.release_cell_inner(ticket, world, manager, frame)
    }
}

fn collect_simulation_refs(
    value: &serde_json::Value,
    refs: &mut Vec<RuntimeAssetRef>,
) -> Result<(), CellError> {
    match value {
        serde_json::Value::Object(v) => {
            if v.contains_key("id") && v.contains_key("type") {
                let reference: RuntimeAssetRef =
                    serde_json::from_value(value.clone()).map_err(|_| CellError::InvalidRequest)?;
                refs.push(reference);
            } else {
                for value in v.values() {
                    collect_simulation_refs(value, refs)?;
                }
            }
        }
        serde_json::Value::Array(v) => {
            for value in v {
                collect_simulation_refs(value, refs)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package copy and GPU; four F03 load-error subcases"]
fn bm11_original_bm10_load_failures_preserve_the_active_population() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    use std::time::{Duration, Instant};
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal451 package"),
    );
    eprintln!("BM11 F03 phase=load-package path={package_path:?}");
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    eprintln!("BM11 F03 phase=link-module");
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    eprintln!("BM11 F03 phase=bootstrap");
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    // F03 injects failed load requests, not a live global asset-index revision.
    // Install the fault aliases before activating the unchanged original world.
    let original = loader
        .asset_loader()
        .asset_index()
        .record_by_guid("bm10-k2-0")
        .unwrap()
        .clone();
    let original_cooked = loader
        .asset_loader()
        .asset_index()
        .cooked_asset(&original.cooked_asset_id)
        .unwrap()
        .clone();
    let original_bytes =
        std::fs::read(package_path.join(original_cooked.path.as_ref().unwrap())).unwrap();
    let fault_dir = package_path.join("bm11-f03-owned");
    std::fs::create_dir(&fault_dir).expect("fresh run-owned package copy");
    let truncated = &original_bytes[..original_bytes.len() / 2];
    std::fs::write(fault_dir.join("truncated-prefab.json"), truncated).unwrap();
    let cases = [
        "missing-file",
        "hash-mismatch",
        "decode-failure",
        "missing-dependency",
    ];
    let mut records: Vec<RuntimeAssetRecord> = vec![];
    let mut cooked_records: Vec<CookedAssetRecord> = vec![];
    for case in cases {
        let mut record = original.clone();
        record.asset_guid = format!("bm11-f03-{case}");
        record.asset_id = record.asset_guid.clone();
        record.cooked_asset_id = format!("cooked-bm11-f03-{case}");
        let mut cooked = original_cooked.clone();
        cooked.cooked_asset_id = record.cooked_asset_id.clone();
        match case {
            "missing-file" => cooked.path = Some("bm11-f03-owned/missing-prefab.json".into()),
            "hash-mismatch" => {
                record.hash = Some(format!("sha256:{}", "0".repeat(64)));
                cooked.hash = record.hash.clone();
            }
            "decode-failure" => {
                cooked.path = Some("bm11-f03-owned/truncated-prefab.json".into());
                record.size = Some(truncated.len() as u64);
                record.hash = Some(crate::canonical_digest::sha256_prefixed(truncated));
                cooked.size = record.size;
                cooked.hash = record.hash.clone();
            }
            "missing-dependency" => record
                .dependencies
                .push("bm11-f03-absent-dependency".into()),
            _ => unreachable!(),
        }
        records.push(record);
        cooked_records.push(cooked);
    }
    loader
        .asset_loader_mut()
        .mount_patch_index(RuntimeAssetIndex::new(records, cooked_records));
    eprintln!("BM11 F03 phase=load-active-scene");
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    eprintln!(
        "BM11 F03 phase=create-backend entities={}",
        world.entity_count()
    );
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    loop {
        frame += 1;
        if frame == 1 || frame % 25 == 0 {
            eprintln!(
                "BM11 F03 phase=population frame={frame} elapsed={:?} entities={}",
                started.elapsed(),
                world.entity_count()
            );
        }
        assert!(
            frame <= 3000 && started.elapsed() < Duration::from_secs(180),
            "original BM10 setup did not reach its population before the bounded fault test"
        );
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "original BM10 setup fault: {:?}",
            output.project_runtime_session_report
        );
        let receipts = loader.project_cell_snapshot(&world).receipts;
        let all_active = receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|state| {
            state.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && state.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("original project must publish its actual population");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }

    use crate::runtime_asset::{CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord};
    use crate::runtime_asset_loader::StreamingError;

    eprintln!(
        "BM11 F03 phase=prepare-original-geometry frame={frame} elapsed={:?}",
        started.elapsed()
    );
    let mut manager = RenderResourceManager::new();
    let mut geometry_cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = geometry_cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f03",
        Some(loader.asset_loader_mut()),
        &mut manager,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    let retained_draw_records = geometry.prepared_geometries().count();
    assert!(retained_draw_records >= 50_000);
    drop(geometry);

    let before = loader.asset_loader().streaming_snapshot();
    assert_eq!(before.pending_operations, 0);
    assert_eq!(before.reserved_bytes, 0);
    let entities = world.entity_count();
    let witness_id = world.entity_ids().into_iter().next().unwrap().clone();
    let witness = world.runtime_id_for_source(&witness_id).unwrap();
    for case in cases {
        eprintln!(
            "BM11 F03 phase=inject case={case} elapsed={:?}",
            started.elapsed()
        );
        let fault_started = Instant::now();
        let id = format!("bm11-f03-{case}");
        let request = CellActivationRequest {
            schema_version: "cell-activation.v1".into(),
            placement_id: format!("fault/{case}"),
            prefab: RuntimeAssetRef {
                id: id.clone(),
                asset_type: "prefab".into(),
                guid: Some(id.clone()),
                sub_asset: None,
            },
            asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
            required_domains: [CellDomain::World, CellDomain::Asset].into_iter().collect(),
            parent: None,
        };
        eprintln!("BM11 F03 phase=request case={case}");
        let error = match loader.request_cell(request, &world) {
            Err(error) => {
                assert_eq!(case, "missing-dependency", "{case}: {error:?}");
                error
            }
            Ok(ticket) => {
                eprintln!("BM11 F03 phase=prepare case={case}");
                assert_ne!(case, "missing-dependency");
                let error = loop {
                    match loader.prepare_cell(ticket, &world, frame, &mut manager) {
                        Err(CellError::NotReady) => {
                            assert!(fault_started.elapsed() < Duration::from_secs(10), "{case}");
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        Err(error) => break error,
                        Ok(status) => panic!("{case} unexpectedly prepared: {status:?}"),
                    }
                };
                eprintln!("BM11 F03 phase=failed case={case} error={error:?}");
                assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Failed));
                assert_eq!(loader.cell_activation_epoch(ticket), Ok(None));
                assert_eq!(loader.cell_error(ticket), Some(&error));
                loader
                    .release_cell(ticket, &mut world, &mut manager, frame)
                    .unwrap();
                eprintln!("BM11 F03 phase=released case={case}");
                error
            }
        };
        match (case, &error) {
            ("missing-file", CellError::Asset(StreamingError::ReadFailed(message))) => {
                assert!(!message.is_empty())
            }
            ("hash-mismatch", CellError::Asset(StreamingError::IntegrityMismatch)) => (),
            ("decode-failure", CellError::Asset(StreamingError::DecodeFailed(message))) => {
                assert!(!message.is_empty())
            }
            ("missing-dependency", CellError::Asset(StreamingError::MissingAsset)) => (),
            _ => panic!("wrong first boundary for {case}: {error:?}"),
        }
        let drain_started = Instant::now();
        loop {
            loader.asset_loader_mut().poll_async();
            let snapshot = loader.asset_loader().streaming_snapshot();
            if snapshot.pending_operations == 0 && snapshot.reserved_bytes == 0 {
                break;
            }
            assert!(
                drain_started.elapsed() < Duration::from_secs(10),
                "{case}: {snapshot:?}"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            loader
                .asset_loader()
                .streaming_snapshot()
                .cpu_resident_bytes,
            before.cpu_resident_bytes,
            "{case}"
        );
        assert_eq!(
            world.entity_count(),
            entities,
            "{case} must not partially activate"
        );
        assert_eq!(world.runtime_id_for_source(&witness_id), Some(witness));
        assert!(world.try_resolve_runtime_entity(witness).is_ok());
        frame += 1;
        backend.pump_geometry_uploads(frame).unwrap();
        eprintln!("BM11 F03 phase=continue-original-host case={case}");
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "{case}: {:?}",
            output.project_runtime_session_report
        );
        let active = loader
            .project_cell_snapshot(&world)
            .receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count();
        assert_eq!(active, 9, "{case}");
        let state = host.project_observation_state().unwrap();
        assert_eq!(
            state.actual_value("k2.populationExact"),
            Some(&ProjectObservationValue::Bool(true)),
            "{case}"
        );
        assert_eq!(
            state.actual_value("k2.allDomainsUpdated"),
            Some(&ProjectObservationValue::Bool(true)),
            "{case}"
        );
        eprintln!("BM11 original BM10 F03 source={id} stage=request/prepare error={error:?} elapsed={:?}; frame={frame} entities={} retainedDrawRecords={retained_draw_records} nineActive={active} populationExact/allDomainsUpdated=true pending/reserved=0 originalCPUCachePreserved", fault_started.elapsed(), world.entity_count());
    }
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package and owned disk saves; F12 refs/persistence only"]
fn bm11_original_bm10_duplicate_prefab_reload_migration_and_disk_save_keep_identity() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    use std::time::{Duration, Instant};
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal451 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    loop {
        frame += 1;
        assert!(
            frame <= 3000 && started.elapsed() < Duration::from_secs(180),
            "original BM10 setup did not reach its population before the bounded fault test"
        );
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "original BM10 setup fault: {:?}",
            output.project_runtime_session_report
        );
        let receipts = loader.project_cell_snapshot(&world).receipts;
        let all_active = receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|state| {
            state.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && state.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("original project must publish its actual population");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }

    use crate::archetype::ComponentValue;
    use crate::component_value::RuntimeValue;
    use crate::components::ComponentTypeId;
    use crate::world_persistence::{WorldLogicalDeclaration3D, WorldLogicalField3D};

    fn load_static(
        loader: &mut RuntimeInstanceLoader,
        world: &mut World,
        manager: &mut RenderResourceManager,
        backend: &mut RealWgpuBackend,
        placement: &str,
        frame: u64,
    ) -> (
        CellTicket,
        SourceEntityId,
        SourceEntityId,
        crate::ids::RuntimeEntityId,
    ) {
        let request = CellActivationRequest {
            schema_version: "cell-activation.v1".into(),
            placement_id: placement.into(),
            prefab: RuntimeAssetRef {
                id: "static.prefab.0".into(),
                asset_type: "prefab".into(),
                guid: Some("guid-static.prefab.0".into()),
                sub_asset: None,
            },
            asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
            required_domains: [CellDomain::World, CellDomain::Asset, CellDomain::Render]
                .into_iter()
                .collect(),
            parent: None,
        };
        let ticket = loader.request_cell(request, world).unwrap();
        let started = Instant::now();
        loop {
            match loader.prepare_cell(ticket, world, frame, manager) {
                Ok(status) => {
                    assert_eq!(status, CellStatus::Prepared);
                    break;
                }
                Err(CellError::NotReady) => {
                    assert!(
                        started.elapsed() < Duration::from_secs(10),
                        "prepare {placement}"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("{placement}: {e:?}"),
            }
        }
        loader.begin_cell_upload(ticket, world, backend).unwrap();
        let instance = loop {
            backend.pump_geometry_uploads(frame).unwrap();
            if let Some(token) = backend.last_gpu_submission() {
                backend
                    .wait_gpu_submission(token, Duration::from_secs(2))
                    .unwrap();
            }
            match loader.commit_cell_gpu(ticket, world, backend) {
                Ok(instance) => break instance,
                Err(CellError::NotReady) => {
                    assert!(
                        started.elapsed() < Duration::from_secs(10),
                        "commit {placement}"
                    );
                    std::thread::yield_now();
                }
                Err(e) => panic!("{placement}: {e:?}"),
            }
        };
        assert_eq!(instance.prefab_asset_guid, "guid-static.prefab.0");
        let runtime = instance.root_entity.unwrap();
        let world_id = world
            .try_resolve_runtime_entity(runtime)
            .unwrap()
            .source_id
            .clone();
        let source = instance
            .source_to_runtime_entity
            .iter()
            .find(|(_, id)| **id == runtime)
            .unwrap()
            .0
            .clone();
        assert_eq!(
            loader.resolve_soft_cell_entity(
                &SoftCellEntityRef {
                    placement_id: placement.into(),
                    source_entity_id: source.to_string()
                },
                world
            ),
            Some(runtime)
        );
        (ticket, source, world_id, runtime)
    }
    fn write(world: &mut World, id: &SourceEntityId, value: i64) {
        let ty = ComponentTypeId::new("project.bm11_f12_state");
        world
            .try_insert_component_value(
                id.clone(),
                ty.clone(),
                ComponentValue::Dynamic {
                    component_type: ty,
                    value: RuntimeValue::Object(
                        [("value".into(), RuntimeValue::I64(value))]
                            .into_iter()
                            .collect(),
                    ),
                },
            )
            .unwrap();
    }
    fn read(world: &World, id: &SourceEntityId) -> RuntimeValue {
        let ComponentValue::Dynamic {
            value: RuntimeValue::Object(values),
            ..
        } = world
            .component_value(id, &ComponentTypeId::new("project.bm11_f12_state"))
            .unwrap()
        else {
            panic!()
        };
        values["value"].clone()
    }
    let mut manager = RenderResourceManager::new();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f12-refs",
        Some(loader.asset_loader_mut()),
        &mut manager,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    let retained_draw_records = geometry.prepared_geometries().count();
    assert!(retained_draw_records >= 50_000);
    drop(geometry);
    let baseline_entities = world.entity_count();
    let baseline = loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (baseline.pending_operations, baseline.reserved_bytes),
        (0, 0)
    );
    eprintln!("BM11 F12 refs original population ready: frame={frame} entities={baseline_entities} retainedDrawRecords={retained_draw_records} setup={:?}",started.elapsed());
    let (left, source, left_id, old_left) = load_static(
        &mut loader,
        &mut world,
        &mut manager,
        &mut backend,
        "bm11-left",
        frame,
    );
    let (right, right_source, right_id, right_runtime) = load_static(
        &mut loader,
        &mut world,
        &mut manager,
        &mut backend,
        "bm11-right",
        frame,
    );
    assert_eq!(source, right_source);
    assert_ne!(old_left, right_runtime);
    assert_ne!(left_id, right_id);
    write(&mut world, &left_id, 31);
    write(&mut world, &right_id, 79);
    let declaration = WorldLogicalDeclaration3D {
        world_id: "bm11-original-bm10".into(),
        revision: 1,
        placements: vec!["bm11-left".into(), "bm11-right".into()],
        fields: vec![WorldLogicalField3D {
            component: "project.bm11_f12_state".into(),
            field: "value".into(),
        }],
    };
    let save = loader
        .capture_partition_logical(&declaration, &world)
        .unwrap();
    assert_eq!(save.records.len(), 2);
    assert!(save
        .records
        .iter()
        .all(|r| r.prefab_guid == "guid-static.prefab.0"));
    let save_file = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM11_F12_SAVE_FILE").expect("run-owned disk save output"),
    );
    let bytes = serde_json::to_vec(&save).unwrap();
    std::fs::write(&save_file, &bytes).unwrap();
    assert_eq!(std::fs::read(&save_file).unwrap(), bytes);
    loader
        .release_cell_gpu(left, &mut world, &mut manager, frame, &mut backend)
        .unwrap();
    assert!(world.try_resolve_runtime_entity(old_left).is_err());
    let left_ref = SoftCellEntityRef {
        placement_id: "bm11-left".into(),
        source_entity_id: source.to_string(),
    };
    let right_ref = SoftCellEntityRef {
        placement_id: "bm11-right".into(),
        source_entity_id: source.to_string(),
    };
    assert!(loader.resolve_soft_cell_entity(&left_ref, &world).is_none());
    assert_eq!(
        loader.resolve_soft_cell_entity(&right_ref, &world),
        Some(right_runtime)
    );
    assert_eq!(read(&world, &right_id), RuntimeValue::I64(79));
    let (left, new_source, left_id, new_left) = load_static(
        &mut loader,
        &mut world,
        &mut manager,
        &mut backend,
        "bm11-left",
        frame,
    );
    assert_eq!(source, new_source);
    assert_ne!(old_left, new_left);
    assert!(world.try_resolve_runtime_entity(old_left).is_err());
    write(&mut world, &left_id, -1);
    write(&mut world, &right_id, -2);
    loader
        .restore_partition_logical(
            &declaration,
            &std::fs::read(&save_file).unwrap(),
            &mut world,
        )
        .unwrap();
    assert_eq!(read(&world, &left_id), RuntimeValue::I64(31));
    assert_eq!(read(&world, &right_id), RuntimeValue::I64(79));
    assert_eq!(
        loader.resolve_soft_cell_entity(&left_ref, &world),
        Some(new_left)
    );
    assert_eq!(
        loader.resolve_soft_cell_entity(&right_ref, &world),
        Some(right_runtime)
    );
    let generation = world.entity_generation(&left_id).unwrap();
    let pose = world.spatial_world(&left_id).unwrap().world_matrix;
    let persistent = loader
        .migrate_cell_subtree_gpu(
            left,
            CellMigrationRequest {
                source_entity_id: source.to_string(),
                source_generation: generation,
                persistent_placement_id: "bm11-persistent".into(),
                parent: None,
                preserve_world: true,
            },
            &mut world,
            &mut manager,
            &mut backend,
        )
        .unwrap();
    assert_eq!(world.entity_generation(&left_id), Some(generation));
    assert_eq!(world.spatial_world(&left_id).unwrap().world_matrix, pose);
    let persistent_ref = SoftCellEntityRef {
        placement_id: "bm11-persistent".into(),
        source_entity_id: source.to_string(),
    };
    assert!(loader.resolve_soft_cell_entity(&left_ref, &world).is_none());
    assert_eq!(
        loader.resolve_soft_cell_entity(&persistent_ref, &world),
        Some(new_left)
    );
    loader
        .release_cell_gpu(left, &mut world, &mut manager, frame, &mut backend)
        .unwrap();
    assert!(world.try_resolve_runtime_entity(new_left).is_ok());
    assert_eq!(
        loader.resolve_soft_cell_entity(&right_ref, &world),
        Some(right_runtime)
    );
    assert!(loader
        .restore_partition_logical(&declaration, &bytes, &mut world)
        .is_err());
    assert_eq!(read(&world, &left_id), RuntimeValue::I64(31));
    assert_eq!(read(&world, &right_id), RuntimeValue::I64(79));
    let mut migrated_declaration = declaration.clone();
    migrated_declaration.placements[0] = "bm11-persistent".into();
    let migrated = loader
        .capture_partition_logical(&migrated_declaration, &world)
        .unwrap();
    assert_eq!(migrated.records.len(), 2);
    let migrated_file = save_file.with_extension("migrated.json");
    let migrated_bytes = serde_json::to_vec(&migrated).unwrap();
    std::fs::write(&migrated_file, &migrated_bytes).unwrap();
    write(&mut world, &left_id, -3);
    write(&mut world, &right_id, -4);
    loader
        .restore_partition_logical(
            &migrated_declaration,
            &std::fs::read(&migrated_file).unwrap(),
            &mut world,
        )
        .unwrap();
    assert_eq!(read(&world, &left_id), RuntimeValue::I64(31));
    assert_eq!(read(&world, &right_id), RuntimeValue::I64(79));
    let mut bad = migrated;
    bad.records[0].value = project_runtime_sdk::ProjectRuntimeValue::Integer(999);
    bad.records[1].source_entity = "absent-source".into();
    assert!(loader
        .restore_partition_logical(
            &migrated_declaration,
            &serde_json::to_vec(&bad).unwrap(),
            &mut world
        )
        .is_err());
    assert_eq!(read(&world, &left_id), RuntimeValue::I64(31));
    assert_eq!(read(&world, &right_id), RuntimeValue::I64(79));
    loader
        .release_cell_gpu(right, &mut world, &mut manager, frame, &mut backend)
        .unwrap();
    loader
        .release_cell_gpu(persistent, &mut world, &mut manager, frame, &mut backend)
        .unwrap();
    assert!(world.try_resolve_runtime_entity(right_runtime).is_err());
    assert!(world.try_resolve_runtime_entity(new_left).is_err());
    assert!(loader
        .resolve_soft_cell_entity(&right_ref, &world)
        .is_none());
    assert!(loader
        .resolve_soft_cell_entity(&persistent_ref, &world)
        .is_none());
    assert_eq!(world.entity_count(), baseline_entities);
    loader.asset_loader_mut().poll_async();
    let after = loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (
            after.pending_operations,
            after.reserved_bytes,
            after.cpu_resident_bytes
        ),
        (0, 0, baseline.cpu_resident_bytes)
    );
    frame += 1;
    backend.pump_geometry_uploads(frame).unwrap();
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_fixed_step_count(1)
            .with_deferred_render_submission(),
        &mut world,
        RuntimeFrameContext {
            package: &package,
            instance_loader: &mut loader,
            cell_resources: None,
            cell_backend: Some(&mut backend),
        },
    );
    assert!(
        !output
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|r| r.terminal_fault),
        "{:?}",
        output.project_runtime_session_report
    );
    assert_eq!(
        loader
            .project_cell_snapshot(&world)
            .receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count(),
        9
    );
    let state = host.project_observation_state().unwrap();
    assert_eq!(
        state.actual_value("k2.populationExact"),
        Some(&ProjectObservationValue::Bool(true))
    );
    assert_eq!(
        state.actual_value("k2.allDomainsUpdated"),
        Some(&ProjectObservationValue::Bool(true))
    );
    eprintln!("BM11 original BM10 F12 refs/save: oldLeft={old_left:?} newLeft={new_left:?} right={right_runtime:?}; duplicate/reload/soft-resolve/migrate/disk-restore/atomic-reject/release passed; originalEntities={baseline_entities} retainedDrawRecords={retained_draw_records} nineActive=9 populationExact/allDomainsUpdated=true cache/pending/reservation returned to original baseline; elapsed={:?}",started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package; required Render owner wait only"]
fn bm11_original_bm10_prepared_cell_waits_for_render_owner_before_one_epoch_publish() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    use std::time::{Duration, Instant};
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal451 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    let mut held: Option<(CellTicket, u64, Vec<SourceEntityId>)> = None;
    let mut delayed_ticks = 0;
    let mut published_epoch = None;
    loop {
        frame += 1;
        assert!(
            frame <= 3000 && started.elapsed() < Duration::from_secs(180),
            "original BM10 did not reach its full population after the bounded Render owner delay"
        );
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let delay_this_tick = held.is_some() && delayed_ticks < 3;
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: if delay_this_tick {
                    None
                } else {
                    Some(&mut backend)
                },
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "original BM10 fault: {:?}",
            output.project_runtime_session_report
        );
        if let Some((ticket, before_epoch, ids)) = &held {
            let entry = loader.cell_entry(*ticket).unwrap();
            if delay_this_tick {
                assert_eq!(entry.status, CellStatus::Prepared);
                assert!(entry.upload.is_none());
                assert_eq!(entry.activation_epoch, None);
                assert_eq!(loader.cells.activation_epoch, *before_epoch);
                assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
                let snapshot = loader.project_cell_snapshot(&world);
                let receipt = snapshot
                    .receipts
                    .values()
                    .find(|r| r.placement_id == entry.request.placement_id)
                    .unwrap();
                assert_eq!(
                    receipt.status,
                    project_runtime_sdk::ProjectRuntimeCellStatus::Prepared
                );
                assert_eq!(receipt.activation_epoch, None);
                assert_eq!(
                    receipt.error_code.as_deref(),
                    Some("cell.render_owner_unavailable")
                );
                delayed_ticks += 1;
                eprintln!("BM11 F11 original prepared cell held: tick={frame} delay={delayed_ticks}/3 entities={} World absent, no upload/activation, epoch={before_epoch}, receipt=cell.render_owner_unavailable", ids.len());
            } else if entry.status == CellStatus::Active {
                let epoch = entry
                    .activation_epoch
                    .expect("real GPU ACK must precede epoch publication");
                assert!(epoch > *before_epoch);
                assert!(ids.iter().all(|id| world.entity_generation(id).is_some()));
                if let Some(prior) = published_epoch {
                    assert_eq!(epoch, prior, "the same cell must not publish twice");
                } else {
                    eprintln!("BM11 F11 original cell published atomically after real GPU owner restored: tick={frame} entities={} epoch={epoch}", ids.len());
                    published_epoch = Some(epoch);
                }
            } else {
                assert!(matches!(
                    entry.status,
                    CellStatus::Prepared | CellStatus::Uploading | CellStatus::ReadyToCommit
                ));
                assert_eq!(entry.activation_epoch, None);
                assert!(
                    ids.iter().all(|id| world.entity_generation(id).is_none()),
                    "no partial World publication before the required render ACK"
                );
            }
        } else if let Some(entry) = loader
            .cells
            .entries
            .values()
            .find(|e| e.status == CellStatus::Prepared)
        {
            let prepared = &entry.prepared.as_ref().unwrap().prepared;
            assert!(
                prepared.entities.len() >= 6000,
                "must consume an original whole BM10 cell, not a small prefab"
            );
            assert!([
                CellDomain::World,
                CellDomain::Asset,
                CellDomain::Render,
                CellDomain::Physics,
                CellDomain::Navigation
            ]
            .iter()
            .all(|d| entry.request.required_domains.contains(d)));
            assert!(entry.simulation.is_some());
            assert!(entry.upload.is_none() && entry.activation_epoch.is_none());
            let ids: Vec<_> = prepared
                .entities
                .iter()
                .map(|e| e.world_id.clone())
                .collect();
            assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
            eprintln!("BM11 F11 captured original Prepared: tick={frame} placement={} entities={} all five required domains, native simulation prepared, World absent, epoch={}",entry.request.placement_id,ids.len(),loader.cells.activation_epoch);
            held = Some((entry.ticket, loader.cells.activation_epoch, ids));
        }
        let all_active = loader
            .project_cell_snapshot(&world)
            .receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|s| {
            s.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && s.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(delayed_ticks, 3);
    assert!(published_epoch.is_some());
    assert_eq!(world.entity_count(), 56_701);
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("population must be published");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }
    let mut manager = RenderResourceManager::new();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f11-render-wait",
        Some(loader.asset_loader_mut()),
        &mut manager,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    let retained = geometry.prepared_geometries().count();
    assert!(retained >= 50_000);
    drop(geometry);
    eprintln!("BM11 original BM10 F11 Render owner delay terminal: frame={frame} heldTicks=3 nineActive=9 entities=56701 retainedDrawRecords={retained} populationExact/allDomainsUpdated=true one target activation epoch={published_epoch:?}; no Physics/Nav reference trajectory or full F11 claim; elapsed={:?}",started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}
#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package; foreign required Render owner receipt refusal only"]
fn bm11_original_bm10_foreign_render_receipt_cannot_commit_before_real_owner_ack() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    use std::time::{Duration, Instant};
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal451 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let mut foreign = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    let mut held: Option<(CellTicket, u64, Vec<SourceEntityId>)> = None;
    let mut foreign_rejected = false;
    let mut published_epoch = None;
    loop {
        frame += 1;
        assert!(frame <= 3000 && started.elapsed() < Duration::from_secs(180));
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        if let Some((ticket, before_epoch, ids)) = &held {
            let entry = loader.cell_entry(*ticket).unwrap();
            if !foreign_rejected && entry.status == CellStatus::Uploading {
                let upload = entry.upload.expect("original real upload ticket");
                if backend.geometry_upload_state(upload).unwrap()
                    == crate::geometry_upload::GeometryUploadState::Ready
                {
                    assert!(entry.simulation.is_some());
                    assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
                    let before_entities = world.entity_count();
                    let original_id = (*world.entity_ids().first().unwrap()).clone();
                    let original_handle = world.runtime_id_for_source(&original_id).unwrap();
                    let failure = loader
                        .commit_cell_gpu(*ticket, &mut world, &mut foreign)
                        .unwrap_err();
                    assert_eq!(
                        failure,
                        CellError::Render("geometry.stale_upload_ticket".into())
                    );
                    assert_eq!(loader.cell_status(*ticket), Ok(CellStatus::Uploading));
                    assert_eq!(loader.cell_activation_epoch(*ticket), Ok(None));
                    assert_eq!(loader.cells.activation_epoch, *before_epoch);
                    assert_eq!(world.entity_count(), before_entities);
                    assert!(world.try_resolve_runtime_entity(original_handle).is_ok());
                    assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
                    assert_eq!(
                        backend.geometry_upload_state(upload).unwrap(),
                        crate::geometry_upload::GeometryUploadState::Ready
                    );
                    foreign_rejected = true;
                    eprintln!("BM11 F11 original complete cell foreign empty backend Render ACK refused: tick={frame} entities={} all five required/native Prepared, actual original GPU Ready preserved, Uploading/no World/epoch commit, old handle valid",ids.len());
                }
            }
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "{:?}",
            output.project_runtime_session_report
        );
        if let Some((ticket, before_epoch, ids)) = &held {
            let entry = loader.cell_entry(*ticket).unwrap();
            if entry.status == CellStatus::Active {
                assert!(foreign_rejected, "original owner cannot publish before the selected rejection boundary was actually tested");
                let epoch = entry.activation_epoch.unwrap();
                assert_eq!(epoch, before_epoch + 1);
                assert!(ids.iter().all(|id| world.entity_generation(id).is_some()));
                if let Some(prior) = published_epoch {
                    assert_eq!(epoch, prior);
                } else {
                    published_epoch = Some(epoch);
                }
            } else {
                assert!(matches!(
                    entry.status,
                    CellStatus::Prepared | CellStatus::Uploading | CellStatus::ReadyToCommit
                ));
                assert_eq!(entry.activation_epoch, None);
                assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
            }
        } else if let Some(entry) = loader
            .cells
            .entries
            .values()
            .find(|e| e.status == CellStatus::Prepared)
        {
            let prepared = &entry.prepared.as_ref().unwrap().prepared;
            assert!(prepared.entities.len() >= 6000);
            assert!([
                CellDomain::World,
                CellDomain::Asset,
                CellDomain::Render,
                CellDomain::Physics,
                CellDomain::Navigation
            ]
            .iter()
            .all(|d| entry.request.required_domains.contains(d)));
            assert!(entry.simulation.is_some());
            assert!(entry.upload.is_none() && entry.activation_epoch.is_none());
            let ids: Vec<_> = prepared
                .entities
                .iter()
                .map(|e| e.world_id.clone())
                .collect();
            assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
            held = Some((entry.ticket, loader.cells.activation_epoch, ids));
        }
        let all_active = loader
            .project_cell_snapshot(&world)
            .receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|s| {
            s.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && s.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(foreign_rejected && published_epoch.is_some());
    assert_eq!(world.entity_count(), 56_701);
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("population must be published");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }
    let mut manager = RenderResourceManager::new();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f11-foreign-receipt",
        Some(loader.asset_loader_mut()),
        &mut manager,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    let retained = geometry.prepared_geometries().count();
    assert!(retained >= 50_000);
    drop(geometry);
    eprintln!("BM11 original BM10 F11 foreign empty Render owner receipt terminal: frame={frame} actual rejection before original owner ack nineActive=9 entities=56701 retainedDrawRecords={retained} populationExact/allDomainsUpdated=true one target activation epoch={published_epoch:?}; no Physics/Nav reference trajectory or full F11 claim; elapsed={:?}",started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    foreign.shutdown_gpu(Duration::from_secs(5)).unwrap();
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}
#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package; first-cell required closure at 75 percent measured CPU budget only"]
fn bm11_original_bm10_required_cell_rejects_seventy_five_percent_measured_cpu_closure() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    use std::time::{Duration, Instant};
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal452 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let fault_started = Instant::now();
    let baseline_entities = world.entity_count();
    let original_id = (*world.entity_ids().first().unwrap()).clone();
    let original_handle = world.runtime_id_for_source(&original_id).unwrap();
    let before_epoch = loader.cells.activation_epoch;
    let original_budget = loader.asset_loader().streaming_budget();
    let mut manager = RenderResourceManager::new();
    let mut physics = crate::physics_3d::PhysicsModule::default();
    let mut navigation = crate::navigation_3d::NavigationModule::default();
    let request = CellActivationRequest {
        schema_version: "cell-activation.v1".into(),
        placement_id: "bm11-f07-normal-first-cell".into(),
        prefab: RuntimeAssetRef {
            id: "bm10-k2-0".into(),
            asset_type: "prefab".into(),
            guid: Some("bm10-k2-0".into()),
            sub_asset: None,
        },
        asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
        required_domains: [
            CellDomain::World,
            CellDomain::Asset,
            CellDomain::Render,
            CellDomain::Physics,
            CellDomain::Navigation,
        ]
        .into_iter()
        .collect(),
        parent: Some("k2-parent-0-0".into()),
    };
    let normal = loader.request_cell(request.clone(), &world).unwrap();
    loop {
        match loader.prepare_cell_with_simulation(
            normal,
            &world,
            1,
            &mut manager,
            &mut physics,
            &mut navigation,
        ) {
            Err(CellError::NotReady) => {
                assert!(fault_started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(CellStatus::Prepared) => break,
            result => panic!("original complete normal closure must prepare: {result:?}"),
        }
    }
    let entry = loader.cell_entry(normal).unwrap();
    let original_cell_entities = entry.prepared.as_ref().unwrap().prepared.entities.len();
    assert!(original_cell_entities >= 6000 && entry.simulation.is_some());
    assert_eq!(world.entity_count(), baseline_entities);
    assert_eq!(loader.cells.activation_epoch, before_epoch);
    let normal_charge = loader
        .asset_loader()
        .streaming_snapshot()
        .cpu_resident_bytes;
    let limited_charge = normal_charge.checked_mul(3).unwrap() / 4;
    assert!(limited_charge >= original_budget.max_asset_bytes && limited_charge < normal_charge);
    loader
        .release_cell_with_simulation(
            normal,
            &mut world,
            &mut manager,
            2,
            &mut physics,
            &mut navigation,
        )
        .unwrap();
    drop(physics);
    drop(navigation);
    let warm_baseline = loader.asset_loader().streaming_snapshot();
    let warm_cache_count = loader.asset_loader().decoded_cache_len();
    assert_eq!(
        (
            warm_baseline.pending_operations,
            warm_baseline.reserved_bytes
        ),
        (0, 0)
    );
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_cpu_resident_bytes: limited_charge,
            ..original_budget
        })
        .unwrap();
    let mut limited_request = request;
    limited_request.placement_id = "bm11-f07-limited-first-cell".into();
    let (error, rejection_stage) = match loader.request_cell(limited_request, &world) {
        Err(error) => (error, "request"),
        Ok(ticket) => {
            let error = loop {
                match loader.prepare_cell(ticket, &world, 3, &mut manager) {
                    Err(CellError::NotReady) => {
                        assert!(fault_started.elapsed() < Duration::from_secs(60));
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => break error,
                    Ok(status) => panic!("75% required closure cannot prepare: {status:?}"),
                }
            };
            assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Failed));
            assert_eq!(loader.cell_activation_epoch(ticket), Ok(None));
            loader
                .release_cell(ticket, &mut world, &mut manager, 4)
                .unwrap();
            assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Released));
            (error, "prepare")
        }
    };
    assert_eq!(
        error,
        CellError::Asset(crate::runtime_asset_loader::StreamingError::BudgetExceeded)
    );
    assert_eq!(world.entity_count(), baseline_entities);
    assert_eq!(loader.cells.activation_epoch, before_epoch);
    assert!(world.try_resolve_runtime_entity(original_handle).is_ok());
    loader.asset_loader_mut().poll_async();
    let after = loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (
            after.pending_operations,
            after.reserved_bytes,
            after.cpu_resident_bytes
        ),
        (0, 0, warm_baseline.cpu_resident_bytes)
    );
    assert_eq!(loader.asset_loader().decoded_cache_len(), warm_cache_count);
    loader
        .asset_loader_mut()
        .set_streaming_budget(original_budget)
        .unwrap();
    eprintln!("BM11 F07 original first-cell normal entities={original_cell_entities} five required/native Prepared charge={normal_charge}; exact floor75% CPU limit={limited_charge} stage={rejection_stage} Asset(BudgetExceeded), no World/epoch publish, old handle valid, warm cache/pending/reservation baseline restored; original2GiB restored; fault={:?}",fault_started.elapsed());
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    loop {
        frame += 1;
        assert!(
            frame <= 3000 && started.elapsed() < Duration::from_secs(180),
            "original BM10 setup did not reach its population before the bounded fault test"
        );
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "original BM10 setup fault: {:?}",
            output.project_runtime_session_report
        );
        let receipts = loader.project_cell_snapshot(&world).receipts;
        let all_active = receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|state| {
            state.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && state.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("original project must publish its actual population");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }

    assert_eq!(world.entity_count(), 56_701);
    let mut geometry_cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = geometry_cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f07-required-budget",
        Some(loader.asset_loader_mut()),
        &mut manager,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    let retained = geometry.prepared_geometries().count();
    assert!(retained >= 50_000);
    drop(geometry);
    eprintln!("BM11 original BM10 75 percent measured required CPU closure terminal: frame={frame} entities=56701 retainedDrawRecords={retained} nineActive=9 populationExact/allDomainsUpdated=true after actual 75 percent CPU refusal; setup={:?}; not queue capacity or optional visual degradation or full F07/BM11",started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[test]
#[ignore = "explicit original BM10 package; CPU native preparation/reference only, no GPU"]
fn bm11_original_bm10_prepared_native_cell_preserves_live_body_and_path_reference() {
    use crate::ids::EntityId;
    use crate::navigation_3d::{NavigationModule, NavigationStatus3D};
    use crate::physics_3d::{PhysicsModule, PhysicsRay3D};
    use crate::project_runtime_session::ProjectRuntimeMutationBuffer;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::spatial_contract::{decode_runtime_scene, RuntimeSceneDocument};
    use std::time::{Duration, Instant};

    fn advance(
        world: &mut World,
        loader: &mut RuntimeInstanceLoader,
        physics: &mut PhysicsModule,
        navigation: &mut NavigationModule,
        tick: u64,
    ) {
        world.begin_spatial_tick(tick);
        navigation
            .advance(world, loader.asset_loader_mut(), tick, 1. / 60.)
            .unwrap();
        physics.advance(world, tick, 1. / 60.).unwrap();
        world.finish_spatial_tick();
    }

    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal452 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let prefab: serde_json::Value = serde_json::from_slice(
        &std::fs::read(package_path.join("prefabs/bm10-k2-0.json")).unwrap(),
    )
    .unwrap();
    let mut scene: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package_path.join("scenes/main.json")).unwrap())
            .unwrap();
    // Both worlds receive identical live probes. The target cell below is the
    // unchanged original complete prefab with all five required domains.
    let mut resident_nav = prefab["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == prefab["rootEntityId"])
        .unwrap()
        .clone();
    resident_nav["id"] = "bm11-reference-nav".into();
    resident_nav["parentId"] = serde_json::Value::Null;
    let mut live_body =
        prefab["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| {
                e["components"].as_array().unwrap().iter().any(|c| {
                    c["componentType"] == "engine.body3d" && c["data"]["motion"] == "dynamic"
                })
            })
            .unwrap()
            .clone();
    live_body["id"] = "bm11-reference-body".into();
    live_body["parentId"] = serde_json::Value::Null;
    live_body["components"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c["componentType"] == "engine.body3d");
    live_body["spatial"]
        .as_object_mut()
        .unwrap()
        .remove("meshRenderer");
    live_body["spatial"]["transform"]["translation"] = serde_json::json!([64.8, 0.6, 8.]);
    scene["entities"]
        .as_array_mut()
        .unwrap()
        .extend([resident_nav, live_body]);
    // The live actor follows the resident tile next to the prepared tile edge.
    let actor = scene["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == "agent-0000")
        .unwrap();
    actor["spatial"]["transform"]["translation"] = serde_json::json!([61., 0.85, 8.]);
    let RuntimeSceneDocument::Spatial(scene) =
        decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()
    else {
        unreachable!()
    };
    let scene = scene.into();
    let mut world = crate::scene_loader::load_scene_into_world(&scene)
        .value
        .unwrap();
    let mut control = crate::scene_loader::load_scene_into_world(&scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    let mut reference_loader = RuntimeInstanceLoader::from_package(&package);
    for loader in [&mut loader, &mut reference_loader] {
        loader
            .asset_loader_mut()
            .set_streaming_budget(StreamingBudget {
                max_handles: 65_536,
                max_operations: 256,
                max_asset_bytes: 64 * 1024 * 1024,
                max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
                ..Default::default()
            })
            .unwrap();
    }
    let mut physics = PhysicsModule::default();
    let mut reference_physics = PhysicsModule::default();
    let mut navigation = NavigationModule::default();
    let mut reference_navigation = NavigationModule::default();
    let agent = EntityId::from("agent-0000");
    let body = EntityId::from("bm11-reference-body");
    for world in [&mut world, &mut control] {
        let mut commands = ProjectRuntimeMutationBuffer::default();
        commands.spatial_command(
            agent.clone(),
            world.entity_generation(&agent).unwrap(),
            project_runtime_sdk::ProjectRuntimeSpatialIntent::NavigationTarget {
                target: [63., 0., 8.],
                deadline_ticks: 600,
            },
        );
        commands.prepare(world).unwrap().commit(world).unwrap();
    }
    let started = Instant::now();
    let mut tick = 0;
    loop {
        tick += 1;
        advance(&mut world, &mut loader, &mut physics, &mut navigation, tick);
        advance(
            &mut control,
            &mut reference_loader,
            &mut reference_physics,
            &mut reference_navigation,
            tick,
        );
        let state = world.navigation3d_state(&agent).unwrap();
        let reference = control.navigation3d_state(&agent).unwrap();
        if state.status != NavigationStatus3D::Pending
            && reference.status != NavigationStatus3D::Pending
        {
            assert_eq!(state.status, NavigationStatus3D::Following);
            assert_eq!(state.points, reference.points);
            break;
        }
        assert!(tick < 80 && started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    let before_entities = world.entity_count();
    let before_epoch = loader.cells.activation_epoch;
    let before_bodies = physics.body_count();
    let before_motors = physics.motor_count();
    let before_surfaces = navigation.resident_surface_count();
    assert!(before_bodies >= 2 && before_motors >= 3 && before_surfaces == 1);
    let mut manager = RenderResourceManager::new();
    let ticket = loader
        .request_cell(
            CellActivationRequest {
                schema_version: "cell-activation.v1".into(),
                placement_id: "bm11-f11-original-native-prepared".into(),
                prefab: RuntimeAssetRef {
                    id: "bm10-k2-0".into(),
                    asset_type: "prefab".into(),
                    guid: Some("bm10-k2-0".into()),
                    sub_asset: None,
                },
                asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
                required_domains: [
                    CellDomain::World,
                    CellDomain::Asset,
                    CellDomain::Render,
                    CellDomain::Physics,
                    CellDomain::Navigation,
                ]
                .into(),
                // Keep the live reference surface in the original cell at x=0;
                // prepare the unchanged complete prefab at its adjacent original placement.
                // Overlapping identical Detour tiles is an invalid preparation fixture.
                parent: Some("k2-parent-1-0".into()),
            },
            &world,
        )
        .unwrap();
    let prepare_started = Instant::now();
    loop {
        match loader.prepare_cell_with_simulation(
            ticket,
            &world,
            tick,
            &mut manager,
            &mut physics,
            &mut navigation,
        ) {
            Ok(CellStatus::Prepared) => break,
            Err(CellError::NotReady) => {
                assert!(prepare_started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(1));
            }
            result => panic!("original complete native preparation failed: {result:?}"),
        }
    }
    let entry = loader.cell_entry(ticket).unwrap();
    let prepared = &entry.prepared.as_ref().unwrap().prepared.entities;
    let prepared_count = prepared.len();
    assert!(prepared_count >= 6000 && entry.simulation.is_some());
    assert!(prepared
        .iter()
        .all(|e| world.entity_generation(&e.world_id).is_none()));
    let mut native_sample = crate::resource_memory::ResourceMemorySample::new(tick, 0, 0);
    loader.observe_memory(&mut native_sample);
    let native_totals = crate::resource_memory::memory_totals(&native_sample.allocations).unwrap();
    for owner in ["cpu.physics", "cpu.navigation"] {
        assert!(
            native_totals
                .by_owner
                .get(owner)
                .is_some_and(|bytes| *bytes > 0),
            "real original Prepared native Rust backing is missing from {owner}"
        );
    }
    // Consume real prepared native receipts through the same activation boundary
    // used by commit, while keeping the public Render gate untouched.
    let mut foreign_physics = PhysicsModule::default();
    let mut foreign_navigation = NavigationModule::default();
    for foreign_domain in [CellDomain::Physics, CellDomain::Navigation] {
        let entry = loader.cell_entry(ticket).unwrap();
        let prepared = &entry.prepared.as_ref().unwrap().prepared;
        let ack = entry.simulation.as_ref().unwrap();
        let owners = if foreign_domain == CellDomain::Physics {
            Some((&mut foreign_physics, &mut navigation))
        } else {
            // Physics activates before Navigation, so this case also exercises
            // removal of the real provisional Jolt bodies on native Nav rejection.
            Some((&mut physics, &mut foreign_navigation))
        };
        let error = match simulation::activate(&world, prepared, ack, owners) {
            Err(error) => error,
            Ok(_) => panic!("foreign {foreign_domain:?} ACK must reject before World publication"),
        };
        let CellError::Simulation(message) = error else {
            panic!("native owner receipt must fail at its own boundary: {error:?}");
        };
        let domain = if foreign_domain == CellDomain::Physics {
            "Physics"
        } else {
            "Navigation"
        };
        assert!(
            message.contains("/prepare/owner")
                && message.ends_with(&format!(
                    "{domain} ACK belongs to a different World or owner."
                )),
            "{message}"
        );
        assert_eq!(physics.body_count(), before_bodies);
        assert_eq!(physics.motor_count(), before_motors);
        assert_eq!(navigation.resident_surface_count(), before_surfaces);
        assert_eq!(foreign_physics.body_count(), 0);
        assert_eq!(foreign_navigation.resident_surface_count(), 0);
        assert_eq!(world.entity_count(), before_entities);
        assert_eq!(loader.cells.activation_epoch, before_epoch);
        assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Prepared));
        assert_eq!(loader.cell_activation_epoch(ticket), Ok(None));
    }
    for _ in 0..20 {
        tick += 1;
        advance(&mut world, &mut loader, &mut physics, &mut navigation, tick);
        advance(
            &mut control,
            &mut reference_loader,
            &mut reference_physics,
            &mut reference_navigation,
            tick,
        );
        assert_eq!(
            world.physics3d_frame().bodies[body.as_str()],
            control.physics3d_frame().bodies[body.as_str()]
        );
        let actual = world.navigation3d_state(&agent).unwrap();
        let reference = control.navigation3d_state(&agent).unwrap();
        assert_eq!(actual.status, reference.status);
        assert_eq!(actual.points, reference.points);
        let ray = PhysicsRay3D {
            origin: [1., 10., 1.],
            displacement: [0., -20., 0.],
        };
        assert_eq!(
            physics.raycast(&ray).unwrap().unwrap().entity,
            reference_physics.raycast(&ray).unwrap().unwrap().entity
        );
        // The original adjacent cell has a floor at x=128; it must remain invisible.
        let prepared_floor_ray = PhysicsRay3D {
            origin: [128., 10., 8.],
            displacement: [0., -20., 0.],
        };
        assert!(physics.raycast(&prepared_floor_ray).unwrap().is_none());
        assert!(reference_physics
            .raycast(&prepared_floor_ray)
            .unwrap()
            .is_none());
        assert_eq!(physics.body_count(), before_bodies);
        assert_eq!(physics.motor_count(), before_motors);
        assert_eq!(navigation.resident_surface_count(), before_surfaces);
        assert_eq!(world.entity_count(), before_entities);
        assert_eq!(loader.cells.activation_epoch, before_epoch);
        assert_eq!(loader.cell_status(ticket).unwrap(), CellStatus::Prepared);
    }
    // At x=64.8 the live body is beyond the old floor but above the unpublished one.
    // Its fall below zero would be prevented if the prepared collider entered production.
    assert!(world.physics3d_frame().bodies[body.as_str()].position[1] < 0.0);
    loader
        .release_cell_with_simulation(
            ticket,
            &mut world,
            &mut manager,
            tick,
            &mut physics,
            &mut navigation,
        )
        .unwrap();
    assert_eq!(physics.body_count(), before_bodies);
    assert_eq!(navigation.resident_surface_count(), before_surfaces);
    assert_eq!(world.entity_count(), before_entities);
    assert_eq!(loader.cells.activation_epoch, before_epoch);
    let snapshot = loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (snapshot.pending_operations, snapshot.reserved_bytes),
        (0, 0)
    );
    let mut after_abort = crate::resource_memory::ResourceMemorySample::new(tick, 0, 0);
    loader.observe_memory(&mut after_abort);
    let after_totals = crate::resource_memory::memory_totals(&after_abort.allocations).unwrap();
    for owner in ["cpu.physics", "cpu.navigation"] {
        assert_eq!(
            after_totals.by_owner.get(owner).copied().unwrap_or(0),
            0,
            "released Prepared storage must leave the loader observation"
        );
    }
    eprintln!("Prepared native Rust requested backing: physics={} bytes navigation={} bytes; after abort=0 in loader observation; native library heap remains Partial", native_totals.by_owner["cpu.physics"], native_totals.by_owner["cpu.navigation"]);

    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    assert!(reference_loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    eprintln!("BM11 F11 original complete native Prepared isolation: {prepared_count} unpublished entities, 20 exact nearby body/path reference ticks and 20 unpublished-floor ray misses; old bodies={before_bodies}, motors={before_motors}, nav surfaces={before_surfaces}; foreign Physics/Nav ACKs rejected and provisional Jolt bodies rolled back; abort preserved World/epoch. CPU-only native boundary; no public GPU commit or full BM11 claim.");
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[test]
#[ignore = "explicit original BM10 package; 75 percent closure queue refusal/recovery, CPU-only"]
fn bm11_original_bm10_required_queue75_refuses_before_dispatch_then_prepares_normally() {
    use crate::runtime_asset_loader::{StreamingBudget, StreamingError};
    use std::time::{Duration, Instant};
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal452 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let mut pending = vec!["bm10-k2-0".to_owned()];
    let mut closure = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if closure.insert(id.clone()) {
            let record = package
                .runtime_asset_index
                .record_by_guid(&id)
                .unwrap_or_else(|| panic!("original closure GUID missing: {id}"));
            pending.extend(record.dependencies.iter().cloned());
        }
    }
    let limited = closure.len() * 3 / 4;
    assert!(limited > 0 && limited < closure.len());
    let mut world = crate::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    let normal_budget = StreamingBudget {
        max_handles: 65_536,
        max_operations: 256,
        max_asset_bytes: 64 * 1024 * 1024,
        max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
        ..Default::default()
    };
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_operations: limited,
            ..normal_budget
        })
        .unwrap();
    let before_entities = world.entity_count();
    let before_epoch = loader.cells.activation_epoch;
    let request = CellActivationRequest {
        schema_version: "cell-activation.v1".into(),
        placement_id: "bm11-f07-queue75-original".into(),
        prefab: RuntimeAssetRef {
            id: "bm10-k2-0".into(),
            asset_type: "prefab".into(),
            guid: Some("bm10-k2-0".into()),
            sub_asset: None,
        },
        asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
        required_domains: [
            CellDomain::World,
            CellDomain::Asset,
            CellDomain::Render,
            CellDomain::Physics,
            CellDomain::Navigation,
        ]
        .into(),
        parent: Some("k2-parent-0-0".into()),
    };
    assert_eq!(
        loader.request_cell(request.clone(), &world),
        Err(CellError::Asset(StreamingError::BudgetExceeded))
    );
    let refused = loader.asset_loader().streaming_snapshot();
    assert_eq!(refused.dispatched_operations, 0);
    assert_eq!((refused.pending_operations, refused.reserved_bytes), (0, 0));
    assert_eq!(loader.cell_lifetime_counts(), (0, 0));
    assert_eq!(world.entity_count(), before_entities);
    assert_eq!(loader.cells.activation_epoch, before_epoch);
    loader
        .asset_loader_mut()
        .set_streaming_budget(normal_budget)
        .unwrap();
    let ticket = loader.request_cell(request, &world).unwrap();
    let mut manager = RenderResourceManager::new();
    let mut physics = crate::physics_3d::PhysicsModule::default();
    let mut navigation = crate::navigation_3d::NavigationModule::default();
    let started = Instant::now();
    loop {
        match loader.prepare_cell_with_simulation(
            ticket,
            &world,
            1,
            &mut manager,
            &mut physics,
            &mut navigation,
        ) {
            Ok(CellStatus::Prepared) => break,
            Err(CellError::NotReady) => {
                assert!(started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(1));
            }
            other => panic!("normal queue must prepare original complete cell: {other:?}"),
        }
    }
    let entry = loader.cell_entry(ticket).unwrap();
    let prepared_entities = entry.prepared.as_ref().unwrap().prepared.entities.len();
    assert!(prepared_entities >= 6000 && entry.simulation.is_some());
    assert_eq!(world.entity_count(), before_entities);
    assert_eq!(loader.cells.activation_epoch, before_epoch);
    assert_eq!(loader.cell_activation_epoch(ticket), Ok(None));
    loader
        .release_cell_with_simulation(
            ticket,
            &mut world,
            &mut manager,
            2,
            &mut physics,
            &mut navigation,
        )
        .unwrap();
    let released = loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (released.pending_operations, released.reserved_bytes),
        (0, 0)
    );
    assert_eq!(world.entity_count(), before_entities);
    assert_eq!(loader.cells.activation_epoch, before_epoch);
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    eprintln!("BM11 F07 original complete required closure queue: {} operations -> floor75%={limited}; exact pre-dispatch BudgetExceeded with no World/epoch publication; restored256 prepares {prepared_entities} entities with all five required/native owners, release pending/reserved0. CPU initialization queue subitem only; not optional visual/GPU/full F07/BM11.", closure.len());
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "original BM10 package and exclusive GPU; complete first-cell commit/immediate release only"]
fn bm11_original_bm10_complete_cell_commits_one_epoch_then_retires_native_and_gpu() {
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend};
    use std::time::{Duration, Instant};
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal452 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let mut world = crate::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let before_entities = world.entity_count();
    let before_epoch = loader.cells.activation_epoch;
    let witness_id = world.entity_ids().into_iter().next().unwrap().clone();
    let witness = world.runtime_id_for_source(&witness_id).unwrap();
    let mut physics = PhysicsModule::default();
    let mut navigation = NavigationModule::default();
    let mut manager = RenderResourceManager::new();
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    // Same fixed Windows profile as runtime_player_winit, not raised to fit a test.
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 4 * 1024 * 1024 * 1024,
            staging_bytes: 128 * 1024 * 1024,
            bytes_per_frame: 16 * 1024 * 1024,
            max_tickets: 256,
        })
        .unwrap();
    let ticket = loader
        .request_cell(
            CellActivationRequest {
                schema_version: "cell-activation.v1".into(),
                placement_id: "bm11-f11-original-immediate-retire".into(),
                prefab: RuntimeAssetRef {
                    id: "bm10-k2-0".into(),
                    asset_type: "prefab".into(),
                    guid: Some("bm10-k2-0".into()),
                    sub_asset: None,
                },
                asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
                required_domains: [
                    CellDomain::World,
                    CellDomain::Asset,
                    CellDomain::Render,
                    CellDomain::Physics,
                    CellDomain::Navigation,
                ]
                .into(),
                parent: Some("k2-parent-0-0".into()),
            },
            &world,
        )
        .unwrap();
    let started = Instant::now();
    loop {
        match loader.prepare_cell_with_simulation(
            ticket,
            &world,
            1,
            &mut manager,
            &mut physics,
            &mut navigation,
        ) {
            Ok(CellStatus::Prepared) => break,
            Err(CellError::NotReady) => {
                assert!(started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(1));
            }
            other => panic!("original complete cell prepare failed: {other:?}"),
        }
    }
    let ids: Vec<_> = loader
        .cell_entry(ticket)
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .prepared
        .entities
        .iter()
        .map(|e| e.world_id.clone())
        .collect();
    assert_eq!(ids.len(), 6863);
    assert_eq!(physics.body_count(), 0);
    assert_eq!(navigation.resident_surface_count(), 0);
    assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
    loader
        .begin_cell_upload(ticket, &world, &mut backend)
        .unwrap();
    let mut frame = 1;
    loop {
        assert!(frame <= 256 && started.elapsed() < Duration::from_secs(120));
        let uploaded = backend.pump_geometry_uploads(frame).unwrap();
        assert!(uploaded <= 16 * 1024 * 1024);
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(5))
                .unwrap();
        }
        match loader.commit_cell_gpu_with_simulation(
            ticket,
            &mut world,
            &mut backend,
            &mut physics,
            &mut navigation,
        ) {
            Ok(_) => break,
            Err(CellError::NotReady) => {
                assert_eq!(loader.cells.activation_epoch, before_epoch);
                assert_eq!(world.entity_count(), before_entities);
                assert_eq!(physics.body_count(), 0);
                assert_eq!(navigation.resident_surface_count(), 0);
                frame += 1;
            }
            other => panic!("original native/GPU commit failed: {other:?}"),
        }
    }
    assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Active));
    assert_eq!(
        loader.cell_activation_epoch(ticket),
        Ok(Some(before_epoch + 1))
    );
    assert_eq!(loader.cells.activation_epoch, before_epoch + 1);
    assert_eq!(world.entity_count(), before_entities + ids.len());
    assert!(ids.iter().all(|id| world.entity_generation(id).is_some()));
    assert!(physics.body_count() > 0 && navigation.resident_surface_count() > 0);
    let handles: Vec<_> = ids
        .iter()
        .map(|id| world.runtime_id_for_source(id).unwrap())
        .collect();
    let resident = backend.geometry_residency();
    assert!(resident.meshes > 0 && resident.physical_mesh_bytes > 0);
    let submitted = backend.gpu_lifecycle_stats().submitted;
    assert!(submitted > 0);

    // No simulation/presentation tick occurs between this single commit and release.
    loader
        .release_cell_gpu_with_simulation(
            ticket,
            &mut world,
            &mut manager,
            frame,
            &mut backend,
            &mut physics,
            &mut navigation,
        )
        .unwrap();
    assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Released));
    assert_eq!(loader.cells.activation_epoch, before_epoch + 1);
    assert_eq!(world.entity_count(), before_entities);
    assert!(ids.iter().all(|id| world.entity_generation(id).is_none()));
    assert!(handles
        .iter()
        .all(|h| world.try_resolve_runtime_entity(*h).is_err()));
    assert_eq!(world.runtime_id_for_source(&witness_id), Some(witness));
    assert!(world.try_resolve_runtime_entity(witness).is_ok());
    assert_eq!(physics.body_count(), 0);
    assert_eq!(navigation.resident_surface_count(), 0);
    let drained = backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert!(!drained.abandoned);
    assert_eq!(drained.completed, drained.submitted);
    assert_eq!(
        (
            drained.in_flight,
            drained.retiring_objects,
            drained.retiring_bytes
        ),
        (0, 0, 0)
    );
    let uploads = backend.geometry_upload_snapshot();
    assert_eq!(
        (
            uploads.tickets,
            uploads.pending_resources,
            uploads.staging_bytes
        ),
        (0, 0, 0)
    );
    let after = backend.geometry_residency();
    assert_eq!(
        (
            after.meshes,
            after.physical_mesh_bytes,
            after.textures,
            after.texture_bytes
        ),
        (0, 0, 0, 0)
    );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    let assets = loader.asset_loader().streaming_snapshot();
    assert_eq!((assets.pending_operations, assets.reserved_bytes), (0, 0));
    eprintln!("BM11 F11 original full cell: {} entities, all five required/native and actual {submitted} GPU submissions; one activation epoch then immediate same-frame release; native/World/old handles correct, actual completion with tracked geometry/upload/staging/inflight/retirement0. No full-active BM10/draw/driver/OS or complete F11 qualification. elapsed={:?}",ids.len(),started.elapsed());
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package, nine cells and actual offscreen GPU draw/retirement"]
fn bm11_original_nine_cells_draw_then_retire_tracked_gpu_resources() {
    use crate::engine_rhi::EngineRhiBackend;
    use crate::render_thread::{RenderFramePacket, RenderThread, RenderThreadConfig};
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_renderer::{QualityProfile, RenderTarget};
    use crate::wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    let started = Instant::now();
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal454 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let mut world = crate::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let baseline_entities = world.entity_count();
    assert_eq!(baseline_entities, 227);
    let baseline_ids: Vec<_> = world
        .entity_ids()
        .into_iter()
        .map(|id| {
            let handle = world.runtime_id_for_source(&id).unwrap();
            (id.clone(), handle)
        })
        .collect();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let mut physics = PhysicsModule::default();
    let mut navigation = NavigationModule::default();
    let resource_owner = Arc::new(Mutex::new(RenderResourceManager::new()));
    let mut manager = resource_owner.lock().unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 4 * 1024 * 1024 * 1024,
            staging_bytes: 128 * 1024 * 1024,
            bytes_per_frame: 16 * 1024 * 1024,
            max_tickets: 256,
        })
        .unwrap();
    let mut tickets = Vec::new();
    let mut frame = 1;
    // The original starting neighbourhood and its unmodified modulo-3 recipe.
    for z in -1_i32..=1 {
        for x in -1_i32..=1 {
            let variant = z.rem_euclid(3) * 3 + x.rem_euclid(3);
            let asset = format!("bm10-k2-{variant}");
            let ticket = loader
                .request_cell(
                    CellActivationRequest {
                        schema_version: "cell-activation.v1".into(),
                        placement_id: format!("bm11-original-draw-{x}-{z}"),
                        prefab: RuntimeAssetRef {
                            id: asset.clone(),
                            asset_type: "prefab".into(),
                            guid: Some(asset),
                            sub_asset: None,
                        },
                        asset_revision_epoch: loader.asset_loader().asset_revision_epoch(),
                        required_domains: [
                            CellDomain::World,
                            CellDomain::Asset,
                            CellDomain::Render,
                            CellDomain::Physics,
                            CellDomain::Navigation,
                        ]
                        .into(),
                        parent: Some(format!("k2-parent-{x}-{z}")),
                    },
                    &world,
                )
                .unwrap();
            loop {
                assert!(started.elapsed() < Duration::from_secs(180));
                match loader.prepare_cell_with_simulation(
                    ticket,
                    &world,
                    frame,
                    &mut manager,
                    &mut physics,
                    &mut navigation,
                ) {
                    Ok(CellStatus::Prepared) => break,
                    Err(CellError::NotReady) => std::thread::sleep(Duration::from_millis(1)),
                    other => panic!("original cell {variant} prepare: {other:?}"),
                }
            }
            loader
                .begin_cell_upload(ticket, &world, &mut backend)
                .unwrap();
            loop {
                assert!(frame <= 2304 && started.elapsed() < Duration::from_secs(180));
                let bytes = backend.pump_geometry_uploads(frame).unwrap();
                assert!(bytes <= 16 * 1024 * 1024);
                if let Some(token) = backend.last_gpu_submission() {
                    backend
                        .wait_gpu_submission(token, Duration::from_secs(5))
                        .unwrap();
                }
                match loader.commit_cell_gpu_with_simulation(
                    ticket,
                    &mut world,
                    &mut backend,
                    &mut physics,
                    &mut navigation,
                ) {
                    Ok(_) => break,
                    Err(CellError::NotReady) => frame += 1,
                    other => panic!("original cell {variant} commit: {other:?}"),
                }
            }
            assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Active));
            tickets.push(ticket);
            frame += 1;
        }
    }
    assert_eq!(tickets.len(), 9);
    assert_eq!(world.entity_count(), 56_701);
    assert!(physics.body_count() > 0 && navigation.resident_surface_count() > 0);
    eprintln!(
        "BM11 nine-cell draw phase=full-world entities={} frame={frame} elapsed={:?}",
        world.entity_count(),
        started.elapsed()
    );
    world.prepare_spatial_presentation(frame, 1.).unwrap();
    let mut extract = crate::render_extract::RenderExtractContext::new();
    let mut scene = crate::render_state::RenderSceneState::new();
    let commands = extract
        .extract_world_dirty(frame, &mut world, &scene)
        .normalize_merge(&scene);
    assert!(crate::render_command::apply_batch(&mut scene, &commands).is_empty());
    let view = extract
        .project_main_camera(&world, &mut scene, [128, 128])
        .unwrap()
        .unwrap();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = cache.snapshot(
        frame,
        &scene,
        "bm11-original-nine-cell-draw",
        Some(loader.asset_loader_mut()),
        &mut manager,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    let retained = geometry.prepared_geometries().count();
    assert!(retained >= 50_000, "original snapshot retained {retained}");
    let weak_snapshot = Arc::downgrade(&geometry);
    drop(manager);
    let mut render =
        RenderThread::with_resource_manager(RenderThreadConfig::default(), resource_owner.clone());
    render.set_external_rhi_enabled(true);
    let (output, receipt) = render.submit_frame_output(RenderFramePacket {
        frame_identity: None,
        geometry: Some(geometry),
        frame_index: frame,
        render_scene_state: scene,
        render_frame_report: None,
        resource_requests: vec![],
        resource_release_requests: vec![],
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        view_id: Some(view),
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::headless_texture("bm11-original-nine-cell-draw", 128, 128),
    });
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(output.report.rdg_status, "ok", "{:?}", output.report);
    backend
        .prepare_geometry_textures(&output.geometry_textures)
        .unwrap();
    let before_draw = backend.gpu_lifecycle_stats().submitted;
    let drawn = backend.execute_plan(&output.renderer_output.rhi_command_plan);
    assert!(drawn.diagnostics.is_empty(), "{drawn:?}");
    assert!(drawn.draw_count > 0, "{drawn:?}");
    assert!(backend.gpu_lifecycle_stats().submitted > before_draw);
    let leases: Vec<_> = output
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .filter_map(|c| c.geometry_payload())
        .filter_map(|p| p.geometry_draw())
        .map(|d| Arc::downgrade(d.native_resource_lease()))
        .collect();
    assert!(!leases.is_empty());
    eprintln!(
        "BM11 nine-cell draw phase=actual-draw retained={retained} draws={} elapsed={:?}",
        drawn.draw_count,
        started.elapsed()
    );
    // Use the original owner releases; no fabricated ticket IDs or device abandon.
    let mut manager = resource_owner.lock().unwrap();
    for (index, ticket) in tickets.into_iter().enumerate() {
        eprintln!(
            "BM11 nine-cell draw release={index} begin elapsed={:?}",
            started.elapsed()
        );
        loader
            .release_cell_gpu_with_simulation(
                ticket,
                &mut world,
                &mut manager,
                frame + 1,
                &mut backend,
                &mut physics,
                &mut navigation,
            )
            .unwrap();
        assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Released));
        eprintln!(
            "BM11 nine-cell draw release={index} end entities={} elapsed={:?}",
            world.entity_count(),
            started.elapsed()
        );
    }
    eprintln!(
        "BM11 nine-cell draw phase=cache-reset begin elapsed={:?}",
        started.elapsed()
    );
    cache.reset_world(frame + 1, &mut manager);
    eprintln!(
        "BM11 nine-cell draw phase=cache-reset end elapsed={:?}",
        started.elapsed()
    );
    drop(manager);
    drop(output);
    drop(cache);
    drop(render);
    eprintln!(
        "BM11 nine-cell draw phase=CPU-drop end elapsed={:?}",
        started.elapsed()
    );
    backend.prepare_geometry_textures(&[]).unwrap();
    eprintln!(
        "BM11 nine-cell draw phase=texture-retire end elapsed={:?}",
        started.elapsed()
    );
    let drained = backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    eprintln!(
        "BM11 nine-cell draw phase=GPU-drain end elapsed={:?}",
        started.elapsed()
    );
    assert!(!drained.abandoned);
    assert_eq!(drained.completed, drained.submitted);
    assert_eq!(
        (
            drained.in_flight,
            drained.retiring_objects,
            drained.retiring_bytes
        ),
        (0, 0, 0)
    );
    assert!(weak_snapshot.upgrade().is_none());
    assert!(leases.iter().all(|l| l.upgrade().is_none()));
    let uploads = backend.geometry_upload_snapshot();
    assert_eq!(
        (
            uploads.tickets,
            uploads.pending_resources,
            uploads.staging_bytes
        ),
        (0, 0, 0)
    );
    let resident = backend.geometry_residency();
    assert_eq!(
        (
            resident.meshes,
            resident.physical_mesh_bytes,
            resident.textures,
            resident.texture_bytes
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(world.entity_count(), baseline_entities);
    assert!(baseline_ids
        .iter()
        .all(|(id, handle)| world.runtime_id_for_source(id) == Some(*handle)));
    assert_eq!(physics.body_count(), 0);
    assert_eq!(navigation.resident_surface_count(), 0);
    let resources = resource_owner.lock().unwrap().end_frame(frame + 20);
    assert_eq!(
        (resources.resident_bytes, resources.pending_release_count),
        (0, 0)
    );
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(5)));
    let assets = loader.asset_loader().streaming_snapshot();
    assert_eq!((assets.pending_operations, assets.reserved_bytes), (0, 0));
    eprintln!("BM11 nine-cell draw terminal: original56701 entities/9cells/5required, retained={retained}, actual draws={}, native/World baseline restored, tracked mesh/texture/upload/staging/inflight/retirement0 and real completed=submitted. No project population-update/long-route/performance/driver/OS/full371 qualification. elapsed={:?}", drawn.draw_count, started.elapsed());
}
