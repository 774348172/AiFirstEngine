//! One camera's immutable inputs, shared by its color and shadow builds.
use super::*;

#[cfg(test)]
thread_local! {
    pub(super) static MATERIAL_INTERPRETATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static INSTANCE_DATA_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static COMMAND_WRITES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static COMMAND_LOCKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Default)]
pub(super) struct InstanceReuse {
    entries: std::collections::HashMap<u64, ([u64; 3], std::sync::Weak<DrawInstance>)>,
    batch_identities: batch::IdentityMemo,
    compact: Option<CompactReuse>,
    commands: Arc<std::sync::RwLock<retained_commands::CommandCache>>,
}
struct CompactReuse {
    table: std::sync::Weak<Vec<Arc<DrawInstance>>>,
    indices: Arc<std::collections::HashMap<u64, u32>>,
}
impl InstanceReuse {
    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        self.batch_identities.observe_memory(sample);
        sample.cpu_arc(&self.commands, "snapshots", &["snapshot"]);
        self.commands
            .read()
            .expect("geometry commands poisoned")
            .observe_memory(sample);
        if let Some(compact) = &self.compact {
            sample.cpu_arc(&compact.indices, "snapshots", &["snapshot"]);
            sample.partial(
                "cpu.owner.snapshots",
                "compactIndexHashBucketsNotInstrumented",
            );
        }
    }
}
impl std::fmt::Debug for InstanceReuse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstanceReuse")
            .field("entries", &self.entries.len())
            .finish()
    }
}
const MAX_REUSED_INSTANCES: usize = 65_536;

// Borrow only for one immutable camera build group. Published draws retain
// their existing owned Arc data; this is not a cross-frame metadata cache.
pub(super) struct BorrowedGeometryInstance<'a> {
    pub(super) deformation: Option<&'a Arc<crate::character_animation::SkinDeformation3D>>,
    pub(super) dynamic_bounds: Option<&'a GeometryBounds>,
    pub(super) instance_id: u64,
    pub(super) source: &'a PreparedGeometry,
    pub(super) world_transform: [f32; 16],
    pub(super) visibility: Visibility,
    pub(super) layers: &'a [String],
    pub(super) provenance: &'a Provenance,
}

#[derive(Clone, Copy)]
pub(super) enum InstanceLayers<'a> {
    Owned(&'a LayerSelection),
    Named(&'a [String]),
}
impl InstanceLayers<'_> {
    fn validate(self) -> Result<(), ContractError> {
        match self {
            Self::Owned(layers) => layers.validate(),
            Self::Named(names) => crate::render_contract::validate_named_layers(names),
        }
    }
    pub(super) fn matches(self, view: &LayerSelection) -> Result<bool, ContractError> {
        match self {
            Self::Owned(layers) => layers_match(view, layers),
            Self::Named(names) => match view {
                LayerSelection::Named { names: selected } => {
                    Ok(selected.is_empty() || selected.iter().any(|n| names.contains(n)))
                }
                _ => Err(invalid("layers.representation")),
            },
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct GeometryInstanceRef<'a> {
    pub(super) deformation: Option<&'a Arc<crate::character_animation::SkinDeformation3D>>,
    pub(super) dynamic_bounds: Option<&'a GeometryBounds>,
    pub(super) instance_id: u64,
    pub(super) source: &'a PreparedGeometry,
    pub(super) world_transform: [f32; 16],
    pub(super) visibility: &'a Visibility,
    pub(super) layers: InstanceLayers<'a>,
    pub(super) provenance: &'a Provenance,
}

pub(super) struct GeometryCandidate<'a> {
    pub(super) instance_id: u64,
    pub(super) world_transform: [f32; 16],
    pub(super) visibility: &'a Visibility,
    pub(super) layers: InstanceLayers<'a>,
    pub(super) convention: CoordinateConvention,
    pub(super) bounds: &'a GeometryBounds,
    pub(super) blend: MaterialBlend,
    pub(super) depth: MaterialDepth,
}

// One evaluated view/range borrows an exact frozen workset. Layer membership
// is pure group data; visibility, representation errors and spatial selection
// are still evaluated in the original row order. Never survives this call.
pub(super) struct LayerMatcher<'prepared, 'view> {
    prepared: Option<&'prepared PreparedInputs>,
    view: &'view LayerSelection,
    groups: Vec<Option<bool>>,
}

impl LayerMatcher<'_, '_> {
    pub(super) fn matches(
        &mut self,
        index: usize,
        candidate: &GeometryCandidate<'_>,
    ) -> Result<bool, ContractError> {
        let Some(prepared) = self.prepared else {
            return candidate.layers.matches(self.view);
        };
        let group = prepared.candidates[index].layer_group;
        if let Some(included) = self.groups[group] {
            return Ok(included);
        }
        // Lazy evaluation preserves hidden/domain-excluded representation
        // mismatches: an unused group must not produce a new error.
        let included = layers_match(self.view, &prepared.layer_groups[group])?;
        self.groups[group] = Some(included);
        Ok(included)
    }
}

#[derive(Debug)]
struct PreparedCandidate {
    instance_id: u64,
    visibility: Visibility,
    convention: CoordinateConvention,
    bounds: GeometryBounds,
    blend: MaterialBlend,
    depth: MaterialDepth,
    layer_group: usize,
}

#[derive(Clone, Copy)]
pub(super) enum InstanceInputs<'a, 'resources> {
    Owned(&'a [GeometryInstance<'resources>]),
    Borrowed(&'a [BorrowedGeometryInstance<'resources>]),
    Retained {
        instances: &'a [Arc<production::RetainedInstance>],
        world: &'a [[f32; 16]],
    },
}
#[cfg(test)]
thread_local! {
    pub(super) static INSTANCE_INPUT_ROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
impl<'a, 'resources> InstanceInputs<'a, 'resources> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
            Self::Retained { instances, .. } => instances.len(),
        }
    }
    pub(super) fn iter(
        self,
    ) -> impl ExactSizeIterator<Item = GeometryInstanceRef<'a>> + use<'a, 'resources> {
        (0..self.len()).map(move |i| self.get(i).expect("prepared input index"))
    }

    // Random access must address the backing slices directly. Iterator::nth
    // may project every skipped row, turning repeated ID lookups quadratic.
    pub(super) fn get(self, i: usize) -> Option<GeometryInstanceRef<'a>> {
        #[cfg(test)]
        INSTANCE_INPUT_ROWS.with(|n| n.set(n.get() + 1));
        Some(match self {
            Self::Owned(v) => {
                let v = v.get(i)?;
                GeometryInstanceRef {
                    deformation: None, dynamic_bounds: None,
                    instance_id: v.instance_id,
                    source: v.source,
                    world_transform: v.world_transform,
                    visibility: &v.visibility,
                    layers: InstanceLayers::Owned(&v.layers),
                    provenance: &v.provenance,
                }
            }
            Self::Borrowed(v) => {
                let v = v.get(i)?;
                GeometryInstanceRef {
                    deformation: v.deformation, dynamic_bounds: v.dynamic_bounds,
                    instance_id: v.instance_id,
                    source: v.source,
                    world_transform: v.world_transform,
                    visibility: &v.visibility,
                    layers: InstanceLayers::Named(v.layers),
                    provenance: v.provenance,
                }
            }
            Self::Retained { instances, world } => {
                let v = instances.get(i)?;
                GeometryInstanceRef {
                    deformation: v.projection.deformation(), dynamic_bounds: v.dynamic_bounds.as_ref(),
                    instance_id: v.instance_id,
                    source: &v.prepared.geometry[v.submesh],
                    world_transform: *world.get(i)?,
                    visibility: &v.visibility,
                    layers: InstanceLayers::Named(v.projection.layers()),
                    provenance: v.projection.provenance(),
                }
            }
        })
    }
}

pub(super) struct GeometryBuildInputs<'a, 'resources> {
    pub(super) instances: InstanceInputs<'a, 'resources>,
    pub(super) manager: &'a RenderResourceManager,
    pub(super) native_lease: Option<&'a Arc<()>>,
    pub(super) batch_identities: batch::IdentityMemo,
    resources: std::collections::HashMap<*const PreparedGeometry, Arc<data::DrawResources>>,
    instance_data: std::collections::HashMap<u64, Arc<DrawInstance>>,
    reuse: Option<std::sync::MutexGuard<'a, InstanceReuse>>,
    reuse_origin: [u64; 3],
    pub(super) compact_table: Option<Arc<Vec<Arc<DrawInstance>>>>,
    pub(super) compact_indices: Arc<std::collections::HashMap<u64, u32>>,
    pub(super) prepared: Option<&'a PreparedInputs>,
}

#[derive(Clone, Copy)]
pub(super) struct FrozenBuildInputs<'a, 'resources> {
    instances: InstanceInputs<'a, 'resources>,
    manager: &'a RenderResourceManager,
    native_lease: Option<&'a Arc<()>>,
    prepared: &'a PreparedInputs,
}
impl<'a, 'resources> FrozenBuildInputs<'a, 'resources> {
    pub(super) fn fork(self) -> GeometryBuildInputs<'a, 'resources> {
        // The caller already admitted the exact frozen table and lease. All
        // children borrow the same immutable manager through the joined scope;
        // no global reuse guard, mutable registry or GPU state is transferred.
        GeometryBuildInputs {
            instances: self.instances,
            manager: self.manager,
            native_lease: self.native_lease,
            batch_identities: Default::default(),
            resources: Default::default(),
            instance_data: Default::default(),
            reuse: None,
            reuse_origin: [0; 3],
            compact_table: Some(self.prepared.table.clone()),
            compact_indices: self.prepared.indices.clone(),
            prepared: Some(self.prepared),
        }
    }
}

/// Private validated facts owned by a snapshot, never by the global reuse memo.
#[derive(Debug)]
pub(super) struct PreparedInputs {
    pub(super) table: Arc<Vec<Arc<DrawInstance>>>,
    indices: Arc<std::collections::HashMap<u64, u32>>,
    pub(super) world: Vec<[f32; 16]>,
    spatial: Vec<workset::SpatialBounds>,
    spatial_envelope: workset::SpatialEnvelope,
    pub(super) depths: Vec<MaterialDepth>,
    handles: Vec<crate::render_resource::RenderResourceHandle>,
    pub(super) phases: std::sync::Mutex<commands::PhaseCache>,
    commands: Arc<std::sync::RwLock<retained_commands::CommandCache>>,
    candidates: Vec<PreparedCandidate>,
    layer_groups: Vec<LayerSelection>,
}

impl PreparedInputs {
    #[cfg(test)]
    pub(super) fn owns_receiver_indices(
        &self,
        table: &Arc<Vec<Arc<DrawInstance>>>,
        indices: &std::collections::HashMap<u64, u32>,
    ) -> bool {
        Arc::ptr_eq(&self.table, table) && std::ptr::eq(self.indices.as_ref(), indices)
    }
    pub(super) fn receiver_bounds(
        &self,
        index: usize,
        selected: &Arc<DrawInstance>,
    ) -> Option<&workset::ReceiverBounds> {
        // IDs and matrix values alone are insufficient: a modified COW row
        // must not inherit static corners from a different immutable backing.
        self.table
            .get(index)
            .filter(|row| Arc::ptr_eq(row, selected))?;
        self.spatial.get(index)?.receiver()
    }

    pub(super) fn shadow_corners(&self, index: usize) -> Option<[[f32; 3]; 8]> {
        self.spatial
            .get(index)?
            .receiver()
            .map(workset::ReceiverBounds::shadow_corners)
    }

    /// Consume the frozen candidate columns directly. Source descriptors and
    /// matrices are needed only for an uncertain spatial-bound fallback.
    pub(super) fn shadow_caster_corners(
        &self,
        instances: InstanceInputs<'_, '_>,
        view: &RenderView,
    ) -> Result<Vec<[[f32; 3]; 8]>, ContractError> {
        let mut layers = vec![None; self.layer_groups.len()];
        let mut corners = Vec::new();
        for (index, candidate) in self.candidates.iter().enumerate() {
            if !candidate.visibility.authored
                || candidate.visibility.inherited != Evaluation::Known(true)
                || candidate.blend.is_transparent()
            {
                continue;
            }
            let group = candidate.layer_group;
            let included = match layers[group] {
                Some(value) => value,
                None => {
                    let value = layers_match(&view.layers, &self.layer_groups[group])?;
                    layers[group] = Some(value);
                    value
                }
            };
            if included {
                corners.push(self.shadow_corners(index).unwrap_or_else(|| {
                    let instance = instances.get(index).expect("frozen caster source");
                    let bounds = instance.dynamic_bounds.unwrap_or(&instance.source.bounds);
                    std::array::from_fn(|i| {
                        let p = std::array::from_fn::<_, 3, _>(|axis| {
                            if i & (1 << axis) == 0 {
                                bounds.min[axis]
                            } else {
                                bounds.max[axis]
                            }
                        });
                        let p =
                            workset::world_corner(instance.world_transform, [p[0], p[1], p[2], 1.]);
                        [p[0], p[1], p[2]]
                    })
                }));
            }
        }
        Ok(corners)
    }

    #[cfg(test)]
    pub(super) fn candidate_is_hot(&self, index: usize, candidate: &GeometryCandidate<'_>) -> bool {
        std::ptr::eq(candidate.bounds, &self.candidates[index].bounds)
            && std::ptr::eq(candidate.visibility, &self.candidates[index].visibility)
    }

    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_arc(&self.table, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.table, "snapshots", &["snapshot"]);
        sample.cpu_arc(&self.indices, "snapshots", &["snapshot"]);
        for row in self.table.iter() {
            memory::instance_backing(sample, row, "snapshot");
        }
        sample.cpu_vec(&self.world, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.spatial, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.depths, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.handles, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.candidates, "snapshots", &["snapshot"]);
        sample.cpu_vec(&self.layer_groups, "snapshots", &["snapshot"]);
        for group in &self.layer_groups {
            match group {
                LayerSelection::Named { names } => {
                    sample.cpu_vec(names, "snapshots", &["snapshot"]);
                    for name in names {
                        sample.cpu_string(name, "snapshots", &["snapshot"]);
                    }
                }
                LayerSelection::Legacy { mask: Some(mask) } => {
                    sample.cpu_string(mask, "snapshots", &["snapshot"]);
                }
                _ => {}
            }
        }
        self.phases
            .lock()
            .expect("geometry phase cache poisoned")
            .observe_memory(sample);
        self.commands
            .read()
            .expect("geometry commands poisoned")
            .observe_memory(sample);
        sample.partial(
            "cpu.owner.snapshots",
            "workset.compactIndexHashBuckets+ArcControl+PendingSelectionAndTaskScratchNotInstrumented",
        );
    }
}

impl<'a, 'resources> GeometryBuildInputs<'a, 'resources> {
    pub(super) fn for_tasks(&self) -> Option<FrozenBuildInputs<'a, 'resources>> {
        let prepared = self.prepared?;
        if !matches!(self.instances, InstanceInputs::Retained { .. })
            || !self
                .compact_table
                .as_ref()
                .is_some_and(|table| Arc::ptr_eq(table, &prepared.table))
        {
            return None;
        }
        Some(FrozenBuildInputs {
            instances: self.instances,
            manager: self.manager,
            native_lease: self.native_lease,
            prepared,
        })
    }
    pub(super) fn finalize_batch_commands(
        &self,
        batches: &mut [GeometryBatch],
        state: &FinalViewState,
        phase: Option<&commands::PreparedPhase>,
    ) {
        self.finalize_commands(commands::CommandHeads::Batches(batches), state, phase)
    }
    pub(super) fn finalize_commands(
        &self,
        mut heads: commands::CommandHeads<'_>,
        state: &FinalViewState,
        phase: Option<&commands::PreparedPhase>,
    ) {
        let static_proven = phase.is_some();
        for index in 0..heads.len() {
            state.apply(heads.draw_mut(index));
        }
        let Some(prepared) = self.prepared else {
            return;
        };
        // The private complete ID->row map was built from exactly this table;
        // its cardinality proves uniqueness without another full-table scan.
        if prepared.indices.len() == prepared.table.len() {
            heads.mark_unique(&prepared.table);
        }
        let mut missing = Vec::new();
        for index in 0..heads.len() {
            let command = phase
                .and_then(|p| heads.command_slot(index, p))
                .and_then(std::sync::OnceLock::get);
            let draw = heads.draw_mut(index);
            if !command.is_some_and(|command| command.attach_to(draw, static_proven)) {
                missing.push(index);
            }
        }
        let remember = !missing.is_empty();
        if !missing.is_empty() {
            #[cfg(test)]
            COMMAND_LOCKS.with(|n| n.set(n.get() + 1));
            let commands = prepared
                .commands
                .read()
                .expect("geometry commands poisoned");
            missing.retain(|&index| {
                let draw = heads.draw_mut(index);
                !commands.attach_existing(draw, static_proven)
            });
        }
        // Drop read ownership before a cold insertion. Other tasks can have
        // filled a miss meanwhile; attach rechecks the full key under the write.
        // Warm views allocate no missing backing and acquire no exclusive guard.
        if !missing.is_empty() {
            #[cfg(test)]
            COMMAND_WRITES.with(|n| n.set(n.get() + 1));
            let mut commands = prepared
                .commands
                .write()
                .expect("geometry commands poisoned");
            for index in missing {
                let draw = heads.draw_mut(index);
                commands.attach(draw, static_proven);
            }
        }
        if let Some(phase) = phase.filter(|_| remember) {
            for index in 0..heads.len() {
                if let Some(slot) = heads
                    .command_slot(index, phase)
                    .filter(|slot| slot.get().is_none())
                {
                    if let Some(command) = &heads.draw(index).command {
                        let _ = slot.set(command.clone());
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn refresh_commands(&self, view: &mut GeometryViewFrame) {
        if let Some(prepared) = self.prepared {
            #[cfg(test)]
            COMMAND_LOCKS.with(|n| n.set(n.get() + 1));
            let mut commands = prepared
                .commands
                .write()
                .expect("geometry commands poisoned");
            for draw in &mut view.draws {
                let proven = draw.static_instance_proven();
                commands.attach(Arc::make_mut(draw), proven);
            }
        }
        view.refresh_batch_draws();
    }
    pub(super) fn compact_batch_key(&mut self, draw: &DrawGeometry) -> (Arc<str>, Arc<str>) {
        let memo = match self.reuse.as_mut() {
            Some(reuse) => &mut reuse.batch_identities,
            None => &mut self.batch_identities,
        };
        memo.resolve(draw)
    }
    pub(super) fn sort_opaque_segment(&mut self, draws: &mut [Arc<DrawGeometry>]) {
        let memo = match self.reuse.as_mut() {
            Some(reuse) => &mut reuse.batch_identities,
            None => &mut self.batch_identities,
        };
        batch::sort_opaque_segment(draws, memo);
    }

    pub(super) fn with_instance_reuse(
        mut self,
        reuse: &'a std::sync::Mutex<InstanceReuse>,
        origin: [u64; 3],
    ) -> Self {
        self.reuse = Some(reuse.lock().expect("geometry instance reuse poisoned"));
        self.reuse.as_mut().unwrap().batch_identities.prune();
        self.reuse_origin = origin;
        self
    }

    pub(super) fn new(
        instances: &'a [GeometryInstance<'resources>],
        manager: &'a RenderResourceManager,
        native_lease: Option<&'a Arc<()>>,
    ) -> Result<Self, GeometryBuildError> {
        Self::from_instances(InstanceInputs::Owned(instances), manager, native_lease)
    }

    pub(super) fn borrowed(
        instances: &'a [BorrowedGeometryInstance<'resources>],
        manager: &'a RenderResourceManager,
        native_lease: Option<&'a Arc<()>>,
    ) -> Result<Self, GeometryBuildError> {
        Self::from_instances(InstanceInputs::Borrowed(instances), manager, native_lease)
    }

    pub(super) fn retained(
        instances: &'a [Arc<production::RetainedInstance>],
        prepared: &'a PreparedInputs,
        manager: &'a RenderResourceManager,
        native_lease: &'a Arc<()>,
    ) -> Result<Self, GeometryBuildError> {
        // The private workset owns the exact immutable sources/world table. A
        // successful preparation never substitutes for current resource admission.
        if !manager.native_leases_valid(&prepared.handles, native_lease) {
            return Err(invalid("nativeLease.stale").into());
        }
        Ok(Self {
            instances: InstanceInputs::Retained {
                instances,
                world: &prepared.world,
            },
            manager,
            native_lease: Some(native_lease),
            batch_identities: Default::default(),
            resources: Default::default(),
            instance_data: Default::default(),
            reuse: None,
            reuse_origin: [0; 3],
            compact_table: Some(prepared.table.clone()),
            compact_indices: prepared.indices.clone(),
            prepared: Some(prepared),
        })
    }

    pub(super) fn freeze(&self) -> PreparedInputs {
        let mut handles: Vec<_> = self
            .instances
            .iter()
            .flat_map(|i| i.source.resource_handles())
            .collect();
        handles.sort_unstable();
        handles.dedup();
        let mut groups = std::collections::HashMap::new();
        let mut layer_groups = Vec::new();
        let candidates = self
            .instances
            .iter()
            .map(|i| {
                let layer = match i.layers {
                    InstanceLayers::Owned(l) => l.clone(),
                    InstanceLayers::Named(names) => LayerSelection::Named {
                        names: names.to_vec(),
                    },
                };
                let key = match &layer {
                    LayerSelection::Named { names } => (false, None, names.clone()),
                    LayerSelection::Legacy { mask } => (true, mask.clone(), Vec::new()),
                };
                let layer_group = *groups.entry(key).or_insert_with(|| {
                    let index = layer_groups.len();
                    layer_groups.push(layer);
                    index
                });
                PreparedCandidate {
                    instance_id: i.instance_id,
                    visibility: i.visibility.clone(),
                    convention: i.source.convention,
                    bounds: i.dynamic_bounds.unwrap_or(&i.source.bounds).clone(),
                    blend: i.source.material.blend,
                    depth: i.source.material.depth,
                    layer_group,
                }
            })
            .collect();
        let spatial: Vec<_> = self
            .instances
            .iter()
            .map(|i| workset::SpatialBounds::new(i.world_transform, i.dynamic_bounds.unwrap_or(&i.source.bounds)))
            .collect();
        let spatial_envelope = workset::SpatialEnvelope::new(&spatial);
        let commands = self
            .reuse
            .as_ref()
            .map_or_else(Default::default, |reuse| reuse.commands.clone());
        commands
            .write()
            .expect("geometry commands poisoned")
            .prune();
        PreparedInputs {
            table: self
                .compact_table
                .as_ref()
                .expect("prepared compact table")
                .clone(),
            indices: self.compact_indices.clone(),
            world: self.instances.iter().map(|i| i.world_transform).collect(),
            spatial,
            spatial_envelope,
            depths: self
                .instances
                .iter()
                .map(|i| i.source.material.depth)
                .collect(),
            handles,
            phases: Default::default(),
            commands,
            candidates,
            layer_groups,
        }
    }

    pub(super) fn spatial_view(&self, matrix: [f32; 16]) -> workset::SpatialView {
        self.prepared.map_or_else(
            || workset::SpatialView::new(matrix),
            |p| workset::SpatialView::with_envelope(matrix, &p.spatial_envelope),
        )
    }

    pub(super) fn candidate(&self, index: usize) -> GeometryCandidate<'a> {
        if let Some(prepared) = self.prepared {
            let row = &prepared.candidates[index];
            return GeometryCandidate {
                instance_id: row.instance_id,
                world_transform: prepared.world[index],
                visibility: &row.visibility,
                layers: InstanceLayers::Owned(&prepared.layer_groups[row.layer_group]),
                convention: row.convention,
                bounds: &row.bounds,
                blend: row.blend,
                depth: row.depth,
            };
        }
        let instance = self.instances.get(index).expect("prepared row index");
        GeometryCandidate {
            instance_id: instance.instance_id,
            world_transform: instance.world_transform,
            visibility: instance.visibility,
            layers: instance.layers,
            convention: instance.source.convention,
            bounds: instance.dynamic_bounds.unwrap_or(&instance.source.bounds),
            blend: instance.source.material.blend,
            depth: instance.source.material.depth,
        }
    }

    pub(super) fn layer_matcher<'view>(
        &self,
        view: &'view LayerSelection,
    ) -> LayerMatcher<'a, 'view> {
        LayerMatcher {
            prepared: self.prepared,
            view,
            groups: vec![None; self.prepared.map_or(0, |p| p.layer_groups.len())],
        }
    }

    pub(super) fn outside_frustum(
        &self,
        index: usize,
        view: &workset::SpatialView,
        instance: &GeometryCandidate<'_>,
    ) -> Result<bool, ContractError> {
        if let Some(decision) = self.prepared.and_then(|p| p.spatial[index].classify(view)) {
            return Ok(decision);
        }
        super::outside_frustum(view.matrix, instance.world_transform, instance.bounds)
    }

    fn from_instances(
        instances: InstanceInputs<'a, 'resources>,
        manager: &'a RenderResourceManager,
        native_lease: Option<&'a Arc<()>>,
    ) -> Result<Self, GeometryBuildError> {
        let mut ids = std::collections::BTreeSet::new();
        // Instances, sources and manager/lease stay immutably borrowed for this
        // camera build group. This scope never survives into a later frame.
        let mut checked_sources = std::collections::HashSet::new();
        for instance in instances.iter() {
            if !ids.insert(instance.instance_id) {
                return Err(build_error(
                    invalid("instanceId"),
                    None,
                    Some(instance.instance_id),
                ));
            }
            validate_world(instance.world_transform)
                .map_err(|e| build_error(e, None, Some(instance.instance_id)))?;
            instance
                .layers
                .validate()
                .map_err(|e| build_error(e, None, Some(instance.instance_id)))?;
            if !checked_sources.insert(instance.source as *const PreparedGeometry) {
                continue;
            }
            #[cfg(test)]
            BUILD_SOURCE_VALIDATIONS.with(|n| n.set(n.get() + 1));
            for binding in [
                &instance.source.geometry_binding,
                &instance.source.material_binding,
            ] {
                binding
                    .validate()
                    .map_err(|e| build_error(e, None, Some(instance.instance_id)))?;
                if let Some(lease) = native_lease {
                    if !binding
                        .resources()
                        .all(|h| manager.native_lease_valid(h, lease))
                    {
                        return Err(build_error(
                            invalid("nativeLease.stale"),
                            None,
                            Some(instance.instance_id),
                        ));
                    }
                } else {
                    binding
                        .validate_residency(manager)
                        .map_err(|e| build_error(e, None, Some(instance.instance_id)))?;
                }
            }
        }
        Ok(Self {
            instances,
            manager,
            native_lease,
            batch_identities: batch::IdentityMemo::default(),
            resources: Default::default(),
            instance_data: Default::default(),
            reuse: None,
            reuse_origin: [0; 3],
            compact_table: None,
            compact_indices: Default::default(),
            prepared: None,
        })
    }

    /// The immutable input slice guarantees a unique instance ID in this scope.
    /// Source addresses are borrowed for the whole scope, never reused across calls.
    pub(super) fn instance_data(
        &mut self,
        instance: &GeometryInstanceRef<'_>,
    ) -> Result<Arc<DrawInstance>, ContractError> {
        #[cfg(test)]
        INSTANCE_DATA_LOOKUPS.with(|n| n.set(n.get() + 1));
        if let Some(data) = self.instance_data.get(&instance.instance_id) {
            return Ok(data.clone());
        }
        let source = instance.source;
        let material = source.material.values()?;
        let world_transform = instance.world_transform.map(Into::into);
        let base_color = material.base_color;
        let alpha_cutoff = material.alpha_cutoff;
        let standard_lit = material.standard_lit.clone();
        let previous = self
            .reuse
            .as_ref()
            .and_then(|reuse| reuse.entries.get(&instance.instance_id))
            .filter(|(origin, _)| *origin == self.reuse_origin)
            .and_then(|(_, weak)| weak.upgrade())
            .filter(|old| {
                self.native_lease
                    .is_some_and(|lease| Arc::ptr_eq(&old.resource_lease, lease))
                    && Arc::ptr_eq(&old.geometry, &source.buffers)
                    && Arc::ptr_eq(&old.geometry_binding, &source.geometry_binding)
                    && Arc::ptr_eq(&old.material_binding, &source.material_binding)
                    && Arc::ptr_eq(&old.geometry_source, &source.geometry_source)
                    && Arc::ptr_eq(&old.material_source, &source.material_source)
                    && old.first_index == source.submesh.first_index
                    && old.index_count == source.submesh.index_count
                    && old.deformation.as_ref() == instance.deformation
                    && old.world_transform == world_transform
                    && old.base_color == base_color
                    && old.alpha_cutoff == alpha_cutoff
                    && old.standard_lit == standard_lit
                    && old.decal_bias == source.material.decal_bias
                    && old.opaque_alpha == source.material.opaque_alpha
                    && old.ignore_vertex_color == source.material.ignore_vertex_color
                    && old.blend == source.material.blend
                    && old.cull == source.material.cull
                    && old.provenance.as_ref() == instance.provenance
            });
        if let Some(previous) = previous {
            self.resources
                .entry(source as *const PreparedGeometry)
                .or_insert_with(|| previous.resources.clone());
            self.instance_data
                .insert(instance.instance_id, previous.clone());
            return Ok(previous);
        }
        let resources =
            self.resources
                .entry(source as *const PreparedGeometry)
                .or_insert_with(|| {
                    Arc::new(data::DrawResources::new(DrawResourceData {
                        resource_lease: self.native_lease.cloned().unwrap_or_else(|| {
                            self.manager.lease_resources(source.resource_handles())
                        }),
                        geometry: source.buffers.clone(),
                        geometry_binding: source.geometry_binding.clone(),
                        material_binding: source.material_binding.clone(),
                        geometry_source: source.geometry_source.clone(),
                        material_source: source.material_source.clone(),
                        first_index: source.submesh.first_index,
                        index_count: source.submesh.index_count,
                    }))
                })
                .clone();
        let data = Arc::new(DrawInstance {
            deformation: instance.deformation.cloned(),
            resources,
            decal_bias: source.material.decal_bias,
            world_transform,
            base_color,
            alpha_cutoff,
            standard_lit,
            opaque_alpha: source.material.opaque_alpha,
            ignore_vertex_color: source.material.ignore_vertex_color,
            blend: source.material.blend,
            cull: source.material.cull,
            instance_id: instance.instance_id,
            provenance: Arc::new(instance.provenance.clone()),
        });
        self.instance_data
            .insert(instance.instance_id, data.clone());
        if let Some(reuse) = &mut self.reuse {
            // This is only a reuse budget: overflow never rejects a valid draw.
            // Weak values cannot pin an old packet, source closure or native lease.
            if reuse.entries.len() < MAX_REUSED_INSTANCES
                || reuse.entries.contains_key(&instance.instance_id)
            {
                reuse.entries.insert(
                    instance.instance_id,
                    (self.reuse_origin, Arc::downgrade(&data)),
                );
            }
        }
        Ok(data)
    }

    pub(super) fn enable_compact(&mut self) -> Result<(), GeometryBuildError> {
        if self.instances.len() > u32::MAX as usize {
            return Err(invalid("geometry.batch.instance_capacity").into());
        }
        let previous = self
            .reuse
            .as_ref()
            .and_then(|r| r.compact.as_ref())
            .and_then(|compact| {
                compact
                    .table
                    .upgrade()
                    .map(|table| (table, compact.indices.clone()))
            })
            .filter(|(table, _)| table.len() == self.instances.len());
        let mut table = previous
            .is_none()
            .then(|| Vec::with_capacity(self.instances.len()));
        let mut same_ids = previous.is_some();
        for (index, instance) in self.instances.iter().enumerate() {
            let data = self
                .instance_data(&instance)
                .map_err(|e| build_error(e, None, Some(instance.instance_id)))?;
            if let Some((old, _)) = &previous {
                same_ids &= old[index].instance_id == data.instance_id;
                if table.is_none() && !Arc::ptr_eq(&old[index], &data) {
                    let mut changed = Vec::with_capacity(self.instances.len());
                    changed.extend(old[..index].iter().cloned());
                    table = Some(changed);
                }
            }
            if let Some(table) = &mut table {
                table.push(data);
            }
        }
        let table = table
            .map(Arc::new)
            .unwrap_or_else(|| previous.as_ref().unwrap().0.clone());
        self.compact_indices = if same_ids {
            previous.as_ref().unwrap().1.clone()
        } else {
            Arc::new(
                table
                    .iter()
                    .enumerate()
                    .map(|(i, data)| (data.instance_id, i as u32))
                    .collect(),
            )
        };
        if table.len() <= MAX_REUSED_INSTANCES {
            if let Some(reuse) = &mut self.reuse {
                reuse.compact = Some(CompactReuse {
                    table: Arc::downgrade(&table),
                    indices: self.compact_indices.clone(),
                });
            }
        }
        self.compact_table = Some(table);
        Ok(())
    }

    pub(super) fn instances(&self) -> InstanceInputs<'a, 'resources> {
        self.instances
    }

    pub(super) fn build(
        &mut self,
        target: &str,
        extent: [u32; 2],
        views: &[GeometryViewInput<'_>],
        diagnostics: DiagnosticLevel,
    ) -> Result<GeometryFrame, GeometryBuildError> {
        GeometryFrame::build_validated(target, extent, views, self, diagnostics)
    }
}

#[cfg(test)]
mod selection_work_tests {
    use super::*;

    fn rows(source: &PreparedGeometry, count: usize) -> Vec<GeometryInstance<'_>> {
        (0..count)
            .map(|i| GeometryInstance {
                instance_id: i as u64 + 1,
                source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[14] = -100.;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named {
                    names: vec!["main".into()],
                },
                provenance: source.geometry_source.as_ref().clone(),
            })
            .collect()
    }

    #[test]
    fn frozen_selection_matches_each_consumed_layer_group_once() {
        let (manager, source, mut view, _) =
            super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        view.layers = LayerSelection::Named {
            names: vec!["main".into()],
        };
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let instances = rows(&source, count);
            let prepared;
            let mut inputs = GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            inputs.enable_compact().unwrap();
            prepared = inputs.freeze();
            inputs.prepared = Some(&prepared);
            LAYER_MATCH_EVALUATIONS.with(|n| n.set(0));
            let result = GeometryFrame::select_view(
                "main",
                [64, 64],
                &GeometryViewInput {
                    view: &view,
                    pose: Default::default(),
                },
                &mut inputs,
                DiagnosticLevel::Trace,
            )
            .unwrap();
            assert_eq!(result.frame.summary.unwrap().included, count);
            observed.push((count, LAYER_MATCH_EVALUATIONS.with(|n| n.get())));
        }
        eprintln!("frozen selection real layer comparisons: {observed:?}");
        assert!(
            observed.iter().all(|(_, calls)| *calls == 1),
            "layer groups reinterpreted per row: {observed:?}"
        );
    }

    #[test]
    fn legacy_candidate_and_visible_source_access_are_linear() {
        let (manager, source, mut view, _) =
            super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        view.layers = LayerSelection::Named {
            names: vec!["main".into()],
        };
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let instances = rows(&source, count);
            let mut inputs = GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            INSTANCE_INPUT_ROWS.with(|n| n.set(0));
            for i in 0..count {
                assert_eq!(inputs.candidate(i).instance_id, i as u64 + 1);
            }
            let candidates = INSTANCE_INPUT_ROWS.with(|n| n.get());
            INSTANCE_INPUT_ROWS.with(|n| n.set(0));
            let result = GeometryFrame::select_view(
                "main",
                [64, 64],
                &GeometryViewInput {
                    view: &view,
                    pose: Default::default(),
                },
                &mut inputs,
                DiagnosticLevel::Off,
            )
            .unwrap();
            assert_eq!(result.frame.draws.len(), count);
            observed.push((count, candidates, INSTANCE_INPUT_ROWS.with(|n| n.get())));
        }
        eprintln!("legacy actual backing projections: {observed:?}");
        assert!(
            observed
                .iter()
                .all(|(n, candidates, selected)| candidates == n && *selected == n * 2),
            "random access traverses skipped rows: {observed:?}"
        );
    }

    fn frame(
        instances: &[GeometryInstance<'_>],
        manager: &RenderResourceManager,
        view: &RenderView,
        frozen: bool,
    ) -> Result<GeometryFrame, GeometryBuildError> {
        let prepared;
        let mut inputs = GeometryBuildInputs::new(instances, manager, None)?;
        if frozen {
            inputs.enable_compact()?;
            prepared = inputs.freeze();
            inputs.prepared = Some(&prepared);
        }
        inputs.build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view,
                pose: Default::default(),
            }],
            DiagnosticLevel::Trace,
        )
    }

    #[test]
    fn frozen_layer_matching_keeps_current_view_sparse_groups_and_old_frames() {
        let (manager, source, mut view, _) =
            super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let mut instances = rows(&source, 32);
        for (i, row) in instances.iter_mut().enumerate() {
            row.layers = LayerSelection::Named {
                names: vec![if i % 3 == 0 { "side" } else { "main" }.into()],
            };
            row.visibility.authored = i % 5 != 0;
            row.visibility.inherited = Evaluation::Known(i % 7 != 0);
            if i % 11 == 0 {
                row.world_transform[12] = 10000.;
            }
        }
        let mut saved = Vec::new();
        for names in [
            vec!["main".into()],
            vec!["side".into()],
            vec![],
            vec!["main".into()],
        ] {
            view.layers = LayerSelection::Named { names };
            let expected = frame(&instances, &manager, &view, false).unwrap();
            let actual = frame(&instances, &manager, &view, true).unwrap();
            assert_eq!(actual.views[0].trace, expected.views[0].trace);
            assert_eq!(actual.views[0].summary, expected.views[0].summary);
            assert_eq!(
                actual.views[0].selected_draws().collect::<Vec<_>>(),
                expected.views[0].selected_draws().collect::<Vec<_>>()
            );
            saved.push(actual);
        }
        assert_eq!(saved[0].views[0].trace, saved[3].views[0].trace);
        assert_eq!(
            saved[0].views[0].selected_draws().collect::<Vec<_>>(),
            saved[3].views[0].selected_draws().collect::<Vec<_>>()
        );
        assert_ne!(saved[0].views[0].trace, saved[1].views[0].trace);
    }

    #[test]
    fn frozen_layer_matching_preserves_lazy_representation_and_visibility_errors() {
        let (manager, source, mut view, _) =
            super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        view.layers = LayerSelection::Named {
            names: vec!["main".into()],
        };
        let mut instances = rows(&source, 4);
        instances[2].layers = LayerSelection::Legacy { mask: None };
        instances[2].visibility.authored = false;
        for authored in [false, true] {
            instances[2].visibility.authored = authored;
            let old = frame(&instances, &manager, &view, false);
            let new = frame(&instances, &manager, &view, true);
            if authored {
                let old = old.unwrap_err();
                let new = new.unwrap_err();
                assert_eq!(new, old);
                assert_eq!(new.error.field, "layers.representation");
            } else {
                assert_eq!(new.unwrap().views[0].trace, old.unwrap().views[0].trace);
            }
        }
        instances[2].visibility.authored = true;
        instances[2].visibility.inherited = Evaluation::NotEvaluated;
        let old = frame(&instances, &manager, &view, false).unwrap_err();
        let new = frame(&instances, &manager, &view, true).unwrap_err();
        assert_eq!(new, old);
        assert_eq!(new.error.field, "visibility.inherited");
    }
}

#[cfg(test)]
mod borrowed_tests {
    use super::*;
    fn borrowed<'a>(instances: &'a [GeometryInstance<'a>]) -> Vec<BorrowedGeometryInstance<'a>> {
        instances
            .iter()
            .map(|i| BorrowedGeometryInstance {
                deformation: None,dynamic_bounds: None,
                instance_id: i.instance_id,
                source: i.source,
                world_transform: i.world_transform,
                visibility: i.visibility.clone(),
                layers: match &i.layers {
                    LayerSelection::Named { names } => names,
                    _ => panic!("named fixture"),
                },
                provenance: &i.provenance,
            })
            .collect()
    }

    #[test]
    fn borrowed_metadata_matches_legacy_inputs_after_sparse_selection_and_reordering() {
        let (manager, source, mut view, _) =
            super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        view.layers = LayerSelection::Named {
            names: vec!["main".into()],
        };
        let mut instances: Vec<_> = (0..32)
            .map(|i| GeometryInstance {
                instance_id: (i * 197 % 64) as u64,
                source: &source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[14] = -20. - i as f32;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(i % 7 != 0),
                },
                layers: LayerSelection::Named {
                    names: vec![if i % 3 == 0 { "other" } else { "main" }.into()],
                },
                provenance: source.geometry_source.as_ref().clone(),
            })
            .collect();
        let lease = manager.lease_resources(source.resource_handles());
        let reuse = std::sync::Mutex::new(InstanceReuse::default());
        let mut retained = Vec::new();
        for reversed in [false, true] {
            if reversed {
                instances.reverse();
            }
            let borrowed = borrowed(&instances);
            let views = [GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }];
            let reference = GeometryFrame::build(
                "main",
                [64, 64],
                &views,
                &instances,
                &manager,
                DiagnosticLevel::Trace,
            )
            .unwrap();
            let mut inputs = GeometryBuildInputs::borrowed(&borrowed, &manager, Some(&lease))
                .unwrap()
                .with_instance_reuse(&reuse, [0; 3]);
            inputs.enable_compact().unwrap();
            let actual = inputs
                .build("main", [64, 64], &views, DiagnosticLevel::Trace)
                .unwrap();
            assert_eq!(
                actual.views[0].selected_draws().collect::<Vec<_>>(),
                reference.views[0].selected_draws().collect::<Vec<_>>()
            );
            assert_eq!(actual.views[0].trace, reference.views[0].trace);
            assert!(actual.views[0].selected_instances().count() < instances.len());
            retained.push(actual);
        }
        assert_eq!(retained.len(), 2);
        // Published Arc rows remain valid after the borrowed adapters are gone.
        for frame in retained {
            for draw in frame.views[0].selected_draws() {
                draw.validate().unwrap();
            }
        }
    }

    #[test]
    fn borrowed_metadata_preserves_layer_world_duplicate_and_representation_failures() {
        let (manager, source, view, _) =
            super::super::shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let valid_names = vec!["main".to_owned()];
        let provenance = source.geometry_source.as_ref();
        let mut m = IDENTITY_MATRIX;
        m[14] = -100.;
        fn make<'a>(
            source: &'a PreparedGeometry,
            m: [f32; 16],
            layers: &'a [String],
        ) -> BorrowedGeometryInstance<'a> {
            BorrowedGeometryInstance {
                deformation: None,dynamic_bounds: None,                instance_id: 7,
                source,
                world_transform: m,
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers,
                provenance: source.geometry_source.as_ref(),
            }
        }
        for names in [
            vec!["".to_owned()],
            vec!["main".to_owned(), "main".to_owned()],
        ] {
            let borrowed = [make(&source, m, &names)];
            let error = GeometryBuildInputs::borrowed(&borrowed, &manager, None)
                .err()
                .unwrap()
                .error;
            let legacy = [GeometryInstance {
                instance_id: 7,
                source: &source,
                world_transform: m,
                visibility: borrowed[0].visibility.clone(),
                layers: LayerSelection::Named {
                    names: names.clone(),
                },
                provenance: provenance.clone(),
            }];
            let expected = GeometryBuildInputs::new(&legacy, &manager, None)
                .err()
                .unwrap()
                .error;
            assert_eq!(error, expected);
            assert_eq!(error.code, ContractErrorCode::InvalidLayers);
        }
        let duplicate = [
            make(&source, m, &valid_names),
            make(&source, m, &valid_names),
        ];
        assert_eq!(
            GeometryBuildInputs::borrowed(&duplicate, &manager, None)
                .err()
                .unwrap()
                .error
                .field,
            "instanceId"
        );
        let mut invalid = [make(&source, m, &valid_names)];
        invalid[0].world_transform[0] = f32::NAN;
        assert_eq!(
            GeometryBuildInputs::borrowed(&invalid, &manager, None)
                .err()
                .unwrap()
                .error
                .code,
            ContractErrorCode::NonFinite
        );
        invalid[0].world_transform = m;
        invalid[0].world_transform[3] = 1.;
        assert_eq!(
            GeometryBuildInputs::borrowed(&invalid, &manager, None)
                .err()
                .unwrap()
                .error
                .field,
            "worldTransform.affine"
        );
        invalid[0].world_transform = m;
        let mut inputs = GeometryBuildInputs::borrowed(&invalid, &manager, None).unwrap();
        let mut wrong_view = view.clone();
        wrong_view.layers = LayerSelection::Legacy { mask: None };
        let views = [GeometryViewInput {
            view: &wrong_view,
            pose: CameraPose::default(),
        }];
        assert_eq!(
            inputs
                .build("main", [64, 64], &views, DiagnosticLevel::Off)
                .unwrap_err()
                .error
                .field,
            "layers.representation"
        );
        assert!(InstanceLayers::Named(&valid_names)
            .matches(&LayerSelection::Named { names: vec![] })
            .unwrap());
    }
}
