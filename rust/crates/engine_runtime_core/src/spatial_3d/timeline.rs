//! Tick-owned immutable spatial history and a separate presentation cache.
use super::*;
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(test)]
std::thread_local! { static PRESENTATION_LOCAL_REBUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
std::thread_local! { static PRESENTATION_MATRIX_REBUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
std::thread_local! { static PRESENTATION_CACHE_INSERTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
std::thread_local! { static DELTA_MAP_INSERTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
std::thread_local! { static DELTA_MATERIALIZATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
std::thread_local! {static INTERPOLATION_LINEAR_BUILDS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
struct InterpolatedLinear {
    key: ([u32; 4], [u32; 3], [u32; 4], [u32; 3], u32),
    rotation: [f32; 4],
    scale: [f32; 3],
}
fn interpolated_linear(
    previous: &CanonicalTransform3D,
    current: &CanonicalTransform3D,
    alpha: f32,
    cache: &mut Option<InterpolatedLinear>,
) -> ([f32; 4], [f32; 3]) {
    let key = (
        previous.rotation_xyzw.map(f32::to_bits),
        previous.scale.map(f32::to_bits),
        current.rotation_xyzw.map(f32::to_bits),
        current.scale.map(f32::to_bits),
        alpha.to_bits(),
    );
    if let Some(value) = cache.as_ref().filter(|value| value.key == key) {
        return (value.rotation, value.scale);
    }
    #[cfg(test)]
    INTERPOLATION_LINEAR_BUILDS.with(|n| n.set(n.get() + 1));
    let scale =
        std::array::from_fn(|i| previous.scale[i] + (current.scale[i] - previous.scale[i]) * alpha);
    let a = previous.rotation_xyzw;
    let b = current.rotation_xyzw;
    let sign = if a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>() < 0. {
        -1.
    } else {
        1.
    };
    let q = std::array::from_fn::<_, 4, _>(|i| a[i] + (sign * b[i] - a[i]) * alpha);
    let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    let rotation = q.map(|v| v / norm);
    *cache = Some(InterpolatedLinear {
        key,
        rotation,
        scale,
    });
    (rotation, scale)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpatialFieldMask(pub u16);
impl SpatialFieldMask {
    pub const TRANSLATION: Self = Self(1);
    pub const ROTATION: Self = Self(2);
    pub const SCALE: Self = Self(4);
    pub const TRANSFORM: Self = Self(7);
    pub const VISIBILITY: Self = Self(8);
    pub const BINDING: Self = Self(16);
    pub const HIERARCHY: Self = Self(32);
    pub const HISTORY: Self = Self(64);
    pub const ALL: Self = Self(127);
    pub fn merge(&mut self, other: Self) {
        self.0 |= other.0;
    }
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
pub(crate) fn field_mask(
    before: Option<&ComponentValue>,
    after: &ComponentValue,
) -> SpatialFieldMask {
    let mut mask = SpatialFieldMask::default();
    match (before, after) {
        (Some(ComponentValue::Transform3D(a)), ComponentValue::Transform3D(b)) => {
            mask = transform_field_mask(a, b);
        }
        (Some(ComponentValue::MeshRenderer3D(a)), ComponentValue::MeshRenderer3D(b)) => {
            if a.visible != b.visible || a.layers != b.layers {
                mask.merge(SpatialFieldMask::VISIBILITY);
            }
            if a.mesh != b.mesh || a.materials != b.materials {
                mask.merge(SpatialFieldMask::BINDING);
            }
        }
        (_, ComponentValue::Visibility3D(_) | ComponentValue::EntityMeta(_)) => {
            mask = SpatialFieldMask::VISIBILITY
        }
        _ => mask = SpatialFieldMask::ALL,
    }
    mask
}
pub(crate) fn transform_field_mask(
    a: &CanonicalTransform3D,
    b: &CanonicalTransform3D,
) -> SpatialFieldMask {
    let mut mask = SpatialFieldMask::default();
    if a.translation != b.translation {
        mask.merge(SpatialFieldMask::TRANSLATION);
    }
    if a.rotation_xyzw != b.rotation_xyzw {
        mask.merge(SpatialFieldMask::ROTATION);
    }
    if a.scale != b.scale {
        mask.merge(SpatialFieldMask::SCALE);
    }
    mask
}

#[cfg(test)]
std::thread_local! { pub(crate) static POSE_BODY_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
std::thread_local! { pub(crate) static TICK_COW_BODIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

fn allocate_pose(value: CommittedSpatialPose) -> Arc<CommittedSpatialPose> {
    #[cfg(test)]
    POSE_BODY_ALLOCATIONS.with(|n| n.set(n.get() + 1));
    Arc::new(value)
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommittedSpatialPose {
    pub runtime_id: RuntimeEntityId,
    pub tick: u64,
    pub local: CanonicalTransform3D,
    pub parent: Option<EntityId>,
    pub state: SpatialWorldState,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialChange {
    pub runtime_id: RuntimeEntityId,
    pub entity_id: EntityId,
    pub field_mask: SpatialFieldMask,
    pub pose: Option<Arc<CommittedSpatialPose>>,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpatialDelta {
    pub tick: u64,
    pub dirty_roots: usize,
    pub affected_count: usize,
    pub changes: Arc<[SpatialChange]>,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresentationDelta {
    pub frame: u64,
    pub tick_pair: [u64; 2],
    pub alpha: f32,
    pub changed: Arc<[EntityId]>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialProfile {
    pub max_value_intents: usize,
    pub max_structure_intents: usize,
}
impl Default for SpatialProfile {
    fn default() -> Self {
        Self {
            max_value_intents: 16384,
            max_structure_intents: 1024,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub(crate) struct SpatialTimeline {
    pub tick: u64,
    // These indices address private, packed history rows, never entity handles.
    // Only source lookup crosses this map; worklists consume the row directly.
    indices: HashMap<EntityId, usize>,
    rows: Vec<TimelineRow>,
    active: Vec<usize>,
    pending: Vec<usize>,
    pub delta: Arc<SpatialDelta>,
    changes: HashMap<RuntimeEntityId, SpatialChange>,
    defer_delta: bool,
    deferred_roots: usize,
    deferred_changes: bool,
    pub presentation_delta: Arc<PresentationDelta>,
    pub profile: SpatialProfile,
    pub used_values: usize,
    pub used_structure: usize,
    origin: [f64; 3],
    pub(super) simple_order: Option<(Vec<RuntimeEntityId>, Vec<usize>)>,
}

#[derive(Debug, Clone)]
struct TimelineRow {
    entity_id: EntityId,
    world: SpatialWorldState,
    previous: Arc<CommittedSpatialPose>,
    current: Arc<CommittedSpatialPose>,
    // At most one spare body per live row. It is writable only while unique.
    spare: Option<Arc<CommittedSpatialPose>>,
    disabled: bool,
    presentation: Option<SpatialWorldState>,
    active_index: Option<usize>,
    pending_index: Option<usize>,
}

impl SpatialTimeline {
    pub(crate) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        fn pose(
            sample: &mut crate::resource_memory::ResourceMemorySample,
            value: &Arc<CommittedSpatialPose>,
        ) {
            sample.cpu_arc(value, "world", &["resident", "snapshot"]);
            if let Some(parent) = &value.parent {
                sample.cpu_arc(parent.storage(), "world", &["resident", "snapshot"]);
            }
        }
        fn change(
            sample: &mut crate::resource_memory::ResourceMemorySample,
            value: &SpatialChange,
        ) {
            sample.cpu_arc(
                value.entity_id.storage(),
                "world",
                &["resident", "snapshot"],
            );
            if let Some(value) = &value.pose {
                pose(sample, value);
            }
        }
        sample.cpu_vec(&self.rows, "world", &["resident", "snapshot"]);
        sample.cpu_vec(&self.active, "world", &["resident"]);
        sample.cpu_vec(&self.pending, "world", &["resident"]);
        if let Some((ids, order)) = &self.simple_order {
            sample.cpu_vec(ids, "world", &["resident"]);
            sample.cpu_vec(order, "world", &["resident"]);
        }
        for row in &self.rows {
            sample.cpu_arc(row.entity_id.storage(), "world", &["resident"]);
            pose(sample, &row.previous);
            pose(sample, &row.current);
            if let Some(value) = &row.spare {
                pose(sample, value);
            }
        }
        for id in self
            .indices
            .keys()
            .chain(self.presentation_delta.changed.iter())
        {
            sample.cpu_arc(id.storage(), "world", &["resident", "snapshot"]);
        }
        sample.cpu_arc(&self.delta, "world", &["resident", "snapshot"]);
        sample.cpu_arc(&self.delta.changes, "world", &["resident", "snapshot"]);
        for value in self.changes.values().chain(self.delta.changes.iter()) {
            change(sample, value);
        }
        sample.cpu_arc(&self.presentation_delta, "world", &["resident", "snapshot"]);
        sample.cpu_arc(
            &self.presentation_delta.changed,
            "world",
            &["resident", "snapshot"],
        );
        sample.partial("cpu.owner.world", "world.timeline.packedRows+worklists+pose+delta+presentationDelta.actualData;mapBacking+ArcControlsOpaque;externallyRetainedArcsNeedConsumerObservation");
    }

    pub(super) fn world_state(&self, id: &EntityId) -> Option<&SpatialWorldState> {
        self.row(id).map(|row| &row.world)
    }
    pub(super) fn world_state_mut(&mut self, id: &EntityId) -> Option<&mut SpatialWorldState> {
        self.indices
            .get(id)
            .copied()
            .map(|index| &mut self.rows[index].world)
    }

    fn row(&self, id: &EntityId) -> Option<&TimelineRow> {
        self.indices.get(id).map(|&index| &self.rows[index])
    }

    fn set_active(&mut self, index: usize, active: bool) {
        match (self.rows[index].active_index, active) {
            (None, true) => {
                self.rows[index].active_index = Some(self.active.len());
                self.active.push(index);
            }
            (Some(position), false) => {
                self.active.swap_remove(position);
                self.rows[index].active_index = None;
                if let Some(&moved) = self.active.get(position) {
                    self.rows[moved].active_index = Some(position);
                }
            }
            _ => {}
        }
    }

    fn set_pending(&mut self, index: usize) {
        if self.rows[index].pending_index.is_none() {
            self.rows[index].pending_index = Some(self.pending.len());
            self.pending.push(index);
        }
    }

    pub fn remove(&mut self, id: &EntityId) {
        if let Some(index) = self.indices.remove(id) {
            self.set_active(index, false);
            if let Some(position) = self.rows[index].pending_index {
                self.pending.swap_remove(position);
                if let Some(&moved) = self.pending.get(position) {
                    self.rows[moved].pending_index = Some(position);
                }
            }
            let row = self.rows.swap_remove(index);
            if let Some(moved) = self.rows.get(index) {
                *self
                    .indices
                    .get_mut(&moved.entity_id)
                    .expect("moved history row") = index;
                if let Some(position) = moved.active_index {
                    self.active[position] = index;
                }
                if let Some(position) = moved.pending_index {
                    self.pending[position] = index;
                }
            }
            self.merge_delta(
                0,
                vec![SpatialChange {
                    runtime_id: row.current.runtime_id,
                    entity_id: id.clone(),
                    field_mask: SpatialFieldMask::ALL,
                    pose: None,
                }],
            );
        }
    }
    pub fn merge_delta(&mut self, roots: usize, mut changes: Vec<SpatialChange>) {
        if !self.defer_delta && self.delta.changes.is_empty() && self.changes.is_empty() {
            // First publication in a tick already owns its rows. Move those rows
            // into the immutable result; build a merge map only if required.
            // Stable sorting preserves last-write semantics for duplicate IDs.
            changes.sort_by_key(|change| change.runtime_id);
            if changes
                .windows(2)
                .all(|p| p[0].runtime_id != p[1].runtime_id)
            {
                #[cfg(test)]
                DELTA_MATERIALIZATIONS.with(|n| n.set(n.get() + 1));
                self.delta = Arc::new(SpatialDelta {
                    tick: self.tick,
                    dirty_roots: self.delta.dirty_roots + roots,
                    affected_count: changes.len(),
                    changes: changes.into(),
                });
                return;
            }
        }
        if self.changes.is_empty() {
            for previous in self.delta.changes.iter() {
                self.changes.insert(previous.runtime_id, previous.clone());
                #[cfg(test)]
                DELTA_MAP_INSERTS.with(|n| n.set(n.get() + 1));
            }
        }
        for mut change in changes {
            if let Some(previous) = self.changes.get(&change.runtime_id) {
                change.field_mask.merge(previous.field_mask);
            }
            self.changes.insert(change.runtime_id, change);
            #[cfg(test)]
            DELTA_MAP_INSERTS.with(|n| n.set(n.get() + 1));
        }
        if self.defer_delta {
            self.deferred_roots += roots;
            self.deferred_changes = true;
            return;
        }
        self.materialize_delta(roots);
    }
    fn materialize_delta(&mut self, roots: usize) {
        #[cfg(test)]
        DELTA_MATERIALIZATIONS.with(|n| n.set(n.get() + 1));
        let mut ordered = self.changes.values().cloned().collect::<Vec<_>>();
        ordered.sort_unstable_by_key(|change| change.runtime_id);
        self.delta = Arc::new(SpatialDelta {
            tick: self.tick,
            dirty_roots: self.delta.dirty_roots + roots,
            affected_count: self.changes.len(),
            changes: ordered.into(),
        });
    }
    #[cfg(test)]
    pub fn publish(
        &mut self,
        id: EntityId,
        value: CommittedSpatialPose,
    ) -> Arc<CommittedSpatialPose> {
        SPATIAL_PUBLICATION_LOOKUPS.with(|n| n.set(n.get() + 1));
        let existing = self.indices.get(&id).copied();
        let (index, pose) = self.publish_at(existing, &id, value);
        self.rows[index].world = pose.state.clone();
        pose
    }
    fn publish_at(
        &mut self,
        existing: Option<usize>,
        id: &EntityId,
        mut value: CommittedSpatialPose,
    ) -> (usize, Arc<CommittedSpatialPose>) {
        let (index, pose) = if let Some(index) = existing {
            let row = &mut self.rows[index];
            let reset = row.current.runtime_id != value.runtime_id
                || row.disabled
                || row.current.parent != value.parent
                || row.current.state.anchor != value.state.anchor
                || row
                    .current
                    .local
                    .scale
                    .iter()
                    .zip(value.local.scale)
                    .any(|(a, b)| a.signum() != b.signum());
            if reset {
                value.state.previous_world_matrix = value.state.world_matrix;
            }
            let pose = if let Some(mut spare) = row.spare.take() {
                if let Some(body) = Arc::get_mut(&mut spare) {
                    *body = value;
                    spare
                } else {
                    allocate_pose(value)
                }
            } else {
                allocate_pose(value)
            };
            if reset {
                row.previous = pose.clone();
            }
            row.current = pose.clone();
            let active = row.previous.state.world_matrix != row.current.state.world_matrix;
            self.set_active(index, active);
            (index, pose)
        } else {
            let pose = allocate_pose(value);
            let index = self.rows.len();
            self.indices.insert(id.clone(), index);
            self.rows.push(TimelineRow {
                entity_id: id.clone(),
                world: pose.state.clone(),
                previous: pose.clone(),
                current: pose.clone(),
                spare: None,
                disabled: false,
                presentation: None,
                active_index: None,
                pending_index: None,
            });
            (index, pose)
        };
        self.set_pending(index);
        (index, pose)
    }
    fn evaluate(
        &self,
        index: usize,
        alpha: f32,
        cache: &mut HashMap<EntityId, SpatialWorldState>,
        store_in_ancestor_cache: bool,
    ) -> Option<SpatialWorldState> {
        self.evaluate_with_linear(index, alpha, cache, store_in_ancestor_cache, &mut None)
    }
    fn evaluate_with_linear(
        &self,
        index: usize,
        alpha: f32,
        cache: &mut HashMap<EntityId, SpatialWorldState>,
        store_in_ancestor_cache: bool,
        linear: &mut Option<InterpolatedLinear>,
    ) -> Option<SpatialWorldState> {
        let row = &self.rows[index];
        let id = &row.entity_id;
        if store_in_ancestor_cache {
            if let Some(v) = cache.get(id) {
                return Some(v.clone());
            }
        }
        let (previous, current) = (&row.previous, &row.current);
        // Independent root endpoints already have a committed immutable matrix.
        // Keep current metadata and leave the caller's coordinate checks intact.
        // Trees/cache evaluation and mismatched history bases still rebuild.
        let endpoint = if !store_in_ancestor_cache
            && current.parent.is_none()
            && previous.parent.is_none()
            && current.runtime_id == previous.runtime_id
        {
            if row.disabled || alpha == 1. {
                Some(current)
            } else if alpha == 0. {
                Some(previous)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(endpoint) = endpoint.filter(|pose| pose.state.anchor == current.state.anchor) {
            let mut state = current.state.clone();
            state.world_matrix = endpoint.state.world_matrix;
            state.pose = endpoint.state.pose;
            return Some(state);
        }
        #[cfg(test)]
        PRESENTATION_LOCAL_REBUILDS.with(|n| n.set(n.get() + 1));
        let mut local = current.local.clone();
        if !row.disabled {
            local.translation = std::array::from_fn(|i| {
                previous.local.translation[i]
                    + (current.local.translation[i] - previous.local.translation[i]) * alpha
            });
            let mut no_reuse = None;
            let reuse = if !store_in_ancestor_cache
                && current.parent.is_none()
                && previous.parent.is_none()
            {
                &mut *linear
            } else {
                &mut no_reuse
            };
            (local.rotation_xyzw, local.scale) =
                interpolated_linear(&previous.local, &current.local, alpha, reuse);
        }
        let parent = current
            .parent
            .as_ref()
            .and_then(|p| self.indices.get(p))
            .and_then(|&index| self.evaluate(index, alpha, cache, true));
        let mut state = current.state.clone();
        // Check the *computed* local, after all interpolation above. Equality
        // of authored endpoints alone cannot authorize skipping normalization.
        // Bit equality preserves signed zero. This borrows this row's current
        // committed matrix, never a cached proof from another entity or tick.
        let same_root_linear = !store_in_ancestor_cache
            && current.parent.is_none()
            && previous.parent.is_none()
            && current.runtime_id == previous.runtime_id
            && current.state.anchor == previous.state.anchor
            && local.rotation_xyzw.map(f32::to_bits)
                == current.local.rotation_xyzw.map(f32::to_bits)
            && local.scale.map(f32::to_bits) == current.local.scale.map(f32::to_bits);
        if same_root_linear {
            state.world_matrix[12..15].copy_from_slice(&local.translation);
            state.pose.position = local.translation;
        } else {
            #[cfg(test)]
            PRESENTATION_MATRIX_REBUILDS.with(|n| n.set(n.get() + 1));
            state.world_matrix = compose_spatial_matrix(parent.as_ref(), matrix(&local));
            state.pose = CameraPose {
                position: [
                    state.world_matrix[12],
                    state.world_matrix[13],
                    state.world_matrix[14],
                ],
                rotation: rotation(
                    parent
                        .as_ref()
                        .map_or([0., 0., 0., 1.], |p| p.pose.rotation),
                    local.rotation_xyzw,
                ),
            };
        }
        if store_in_ancestor_cache {
            cache.insert(id.clone(), state.clone());
            #[cfg(test)]
            PRESENTATION_CACHE_INSERTS.with(|n| n.set(n.get() + 1));
        }
        Some(state)
    }
}
impl World {
    /// Only immutable delta flattening is deferred. All mutations and World
    /// reads retain their original order; the callback must not consume deltas.
    /// Both ordinary errors and unwinding publish already committed changes.
    pub(crate) fn with_deferred_spatial_delta<T>(
        &mut self,
        callback: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let outer = !self.spatial_timeline.defer_delta;
        self.spatial_timeline.defer_delta = true;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(self)));
        if outer {
            let timeline = &mut self.spatial_timeline;
            timeline.defer_delta = false;
            let roots = std::mem::take(&mut timeline.deferred_roots);
            if std::mem::take(&mut timeline.deferred_changes) {
                timeline.materialize_delta(roots);
            }
        }
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(super) fn publish_spatial_state(
        &mut self,
        input: SpatialPublishInput<'_>,
        already_dirty: &BTreeSet<EntityId>,
        changes: &mut Vec<SpatialChange>,
    ) {
        let SpatialPublishInput {
            entity_id: id,
            runtime_id,
            local,
            parent,
            mut state,
            mut mask,
            explicit,
        } = input;
        #[cfg(test)]
        SPATIAL_PUBLICATION_LOOKUPS.with(|n| n.set(n.get() + 1));
        let existing = self.spatial_timeline.indices.get(id).copied();
        let old = existing.map(|index| &self.spatial_timeline.rows[index].world);
        let mut published_index = existing;
        if let Some(old) = old {
            state.previous_world_matrix = if self.spatial_timeline.tick > 0 {
                // begin_tick and history reset keep the World state in the
                // same history pair. A second name lookup into timeline cannot
                // add information; later writes in this tick retain this value.
                old.previous_world_matrix
            } else {
                old.world_matrix
            };
            state.version = old.version;
        }
        if old != Some(&state) {
            state.version = state.version.saturating_add(1);
        }
        if let Some(old) = old {
            if !explicit && old.world_matrix != state.world_matrix {
                mask.merge(SpatialFieldMask::TRANSFORM);
            }
            if old.anchor != state.anchor {
                mask.merge(SpatialFieldMask::HIERARCHY);
            }
            if old.enabled != state.enabled
                || old.authored_visible != state.authored_visible
                || old.inherited_visible != state.inherited_visible
            {
                mask.merge(SpatialFieldMask::VISIBILITY);
            }
        } else {
            mask = SpatialFieldMask::ALL;
        }
        if old.is_none_or(|old| {
            old.world_matrix != state.world_matrix
                || old.anchor != state.anchor
                || old.enabled != state.enabled
                || old.authored_visible != state.authored_visible
                || old.inherited_visible != state.inherited_visible
        }) || mask != SpatialFieldMask::default()
        {
            let current = CommittedSpatialPose {
                runtime_id,
                tick: self.spatial_timeline.tick,
                local,
                parent,
                state: state.clone(),
            };
            let (index, current) = self.spatial_timeline.publish_at(existing, id, current);
            published_index = Some(index);
            state = current.state.clone();
            changes.push(SpatialChange {
                runtime_id: current.runtime_id,
                entity_id: id.clone(),
                field_mask: mask,
                pose: Some(current),
            });
            // Both callers publish a unique source domain (affected set or final
            // row overlay). Only pre-existing notifications need membership;
            // inserting every newly published ID builds an unused second set.
            if !already_dirty.contains(id) {
                self.dirty_records.push(DirtyRecord {
                    entity_id: id.clone(),
                    dirty_type: DirtyType::RenderState,
                });
            }
        }
        // The tick-owned timeline settles history itself. The legacy set is
        // consumed only in tick 0 and cannot contribute in a monotonic tick run.
        if self.spatial_timeline.tick == 0 && state.previous_world_matrix != state.world_matrix {
            self.spatial_previous_pending.insert(id.clone());
        }
        self.spatial_timeline.rows[published_index.expect("new state publishes a pose")].world =
            state;
    }

    #[cfg(test)]
    pub(crate) fn presentation_cache_work(reset: bool) -> usize {
        PRESENTATION_CACHE_INSERTS.with(|n| if reset { n.replace(0) } else { n.get() })
    }
    pub fn has_spatial_entities(&self) -> bool {
        !self.spatial_timeline.rows.is_empty() || !self.spatial_dirty.is_empty()
    }
    pub fn spatial_profile(&self) -> SpatialProfile {
        self.spatial_timeline.profile
    }
    pub fn set_spatial_profile(
        &mut self,
        profile: SpatialProfile,
    ) -> Result<(), WorldMutationError> {
        if profile.max_value_intents == 0
            || profile.max_structure_intents == 0
            || self.spatial_timeline.used_values > profile.max_value_intents
            || self.spatial_timeline.used_structure > profile.max_structure_intents
        {
            return Err(failure(
                &EntityId::from("world"),
                "three_d.spatial_budget_invalid",
                "Profile must admit current tick usage and have positive limits",
            ));
        }
        self.spatial_timeline.profile = profile;
        Ok(())
    }
    pub(crate) fn check_spatial_budget(
        &self,
        values: usize,
        structure: usize,
    ) -> Result<(), WorldMutationError> {
        let t = &self.spatial_timeline;
        if values > t.profile.max_value_intents.saturating_sub(t.used_values)
            || structure
                > t.profile
                    .max_structure_intents
                    .saturating_sub(t.used_structure)
        {
            return Err(failure(
                &EntityId::from("world"),
                "three_d.spatial_budget_exceeded",
                "Whole batch exceeds remaining tick budget",
            ));
        }
        Ok(())
    }
    pub(crate) fn charge_spatial_budget(&mut self, values: usize, structure: usize) {
        self.spatial_timeline.used_values += values;
        self.spatial_timeline.used_structure += structure;
    }
    pub fn begin_spatial_tick(&mut self, tick: u64) {
        assert!(
            tick > self.spatial_timeline.tick,
            "spatial tick must advance monotonically"
        );
        self.propagate_spatial();
        let t = &mut self.spatial_timeline;
        t.tick = tick;
        t.used_values = 0;
        t.used_structure = 0;
        // Release only this owner's old delta references before COW. External
        // snapshots/deltas still retain their Arc and force a private body.
        t.changes.clear();
        t.delta = Arc::new(SpatialDelta {
            tick,
            ..Default::default()
        });
        let mut active = std::mem::take(&mut t.active);
        for index in active.drain(..) {
            let row = &mut t.rows[index];
            row.active_index = None;
            #[cfg(test)]
            if Arc::strong_count(&row.current) > 1 || Arc::weak_count(&row.current) > 0 {
                TICK_COW_BODIES.with(|n| n.set(n.get() + 1));
            }
            let settled = Arc::make_mut(&mut row.current);
            settled.state.previous_world_matrix = settled.state.world_matrix;
            let old_previous = std::mem::replace(&mut row.previous, row.current.clone());
            // Never keep a second reference to the body being settled next tick.
            // Shared strong/weak readers are respected again at reuse time.
            row.spare = (!Arc::ptr_eq(&old_previous, &row.current)).then_some(old_previous);
            row.world.previous_world_matrix = row.world.world_matrix;
            t.set_pending(index);
        }
        t.active = active;
    }
    pub fn finish_spatial_tick(&mut self) -> Arc<SpatialDelta> {
        self.propagate_spatial();
        self.spatial_timeline.delta.clone()
    }
    /// Shared immutable delta: reading it never consumes another domain's updates.
    pub fn spatial_delta(&self) -> Arc<SpatialDelta> {
        self.spatial_timeline.delta.clone()
    }
    pub fn spatial_presentation_delta(&self) -> Arc<PresentationDelta> {
        self.spatial_timeline.presentation_delta.clone()
    }
    pub fn committed_spatial_pose(&self, id: &EntityId) -> Option<Arc<CommittedSpatialPose>> {
        self.spatial_timeline.row(id).map(|row| row.current.clone())
    }
    /// Coordinate basis of the immutable evaluated presentation cache.
    pub fn spatial_presentation_origin(&self, id: &EntityId) -> Option<[f64; 3]> {
        self.spatial_presentation(id).map(|state| {
            if self
                .spatial_timeline
                .row(id)
                .is_some_and(|row| row.presentation.is_some())
            {
                self.spatial_timeline.origin
            } else {
                state.anchor
            }
        })
    }
    pub fn spatial_presentation(&self, id: &EntityId) -> Option<&SpatialWorldState> {
        self.spatial_timeline
            .row(id)
            .and_then(|row| row.presentation.as_ref())
            .or_else(|| self.spatial_world(id))
    }
    pub fn prepare_spatial_presentation(
        &mut self,
        frame: u64,
        alpha: f32,
    ) -> Result<Arc<PresentationDelta>, WorldMutationError> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(failure(
                &"world".into(),
                "three_d.presentation_alpha_invalid",
                "Alpha must be finite and in [0,1]",
            ));
        }
        self.propagate_spatial();
        let origin = self
            .main_camera()
            .and_then(|id| self.spatial_world(id))
            .map_or([0.; 3], |s| s.anchor);
        let origin_changed = origin != self.spatial_timeline.origin;
        // Dense worklist membership is unique. Intersect by row membership, not
        // by sorting/hashing all source IDs again. Sort only published updates.
        let candidates = if origin_changed {
            (0..self.spatial_timeline.rows.len()).collect::<Vec<_>>()
        } else {
            let mut indices = self.spatial_timeline.pending.clone();
            indices.extend(
                self.spatial_timeline
                    .active
                    .iter()
                    .copied()
                    .filter(|&index| self.spatial_timeline.rows[index].pending_index.is_none()),
            );
            indices
        };

        let origin_position = crate::spatial_contract::WorldPosition::new(origin, [0.; 3]);
        let domain = crate::spatial_contract::CoordinateDomain::new(1e12, 32768., 32768., 0.005)
            .expect("fixed presentation coordinate envelope is valid");
        let mut cache = HashMap::new();
        let mut updates = Vec::new();
        let mut linear = None;
        for index in candidates {
            let row = &self.spatial_timeline.rows[index];
            let id = &row.entity_id;
            let needs_ancestor_cache = self
                .hierarchy_children
                .get(id)
                .is_some_and(|children| !children.is_empty())
                || row.current.parent.is_some();
            if let Some(mut state) = self.spatial_timeline.evaluate_with_linear(
                index,
                alpha,
                &mut cache,
                needs_ancestor_cache,
                &mut linear,
            ) {
                let relative = crate::spatial_contract::WorldPosition::new(
                    state.anchor,
                    [
                        state.world_matrix[12],
                        state.world_matrix[13],
                        state.world_matrix[14],
                    ],
                )
                .and_then(|position| position.relative_to(origin_position.clone()?, domain))
                .map_err(|e| {
                    failure(id, "three_d.presentation_coordinate_invalid", e.to_string())
                })?;
                state.world_matrix[12..15].copy_from_slice(&relative);
                state.pose.position = relative;
                if row.presentation.as_ref() != Some(&state) {
                    updates.push((index, Some(state)));
                }
            }
        }
        for &index in &self.spatial_timeline.pending {
            self.spatial_timeline.rows[index].pending_index = None;
        }
        self.spatial_timeline.pending.clear();
        self.spatial_timeline.origin = origin;
        let mut order = (0..updates.len()).collect::<Vec<_>>();
        order.sort_unstable_by(|&a, &b| {
            self.spatial_timeline.rows[updates[a].0]
                .entity_id
                .cmp(&self.spatial_timeline.rows[updates[b].0].entity_id)
        });
        let mut changed = Vec::with_capacity(updates.len());
        for index in order {
            let (index, state) = &mut updates[index];
            let row = &mut self.spatial_timeline.rows[*index];
            row.presentation = state.take();
            let id = row.entity_id.clone();
            changed.push(id.clone());
            self.dirty_records.push(DirtyRecord {
                entity_id: id,
                dirty_type: DirtyType::RenderState,
            });
        }
        changed.extend(self.prepare_character_presentation(alpha).map_err(|e| {
            failure(
                &"world".into(),
                "three_d.character_presentation",
                e.to_string(),
            )
        })?);
        changed.sort_unstable();
        changed.dedup();
        let tick = self.spatial_timeline.tick;
        let delta = Arc::new(PresentationDelta {
            frame,
            tick_pair: [tick.saturating_sub(1), tick],
            alpha,
            changed: changed.into(),
        });
        self.spatial_timeline.presentation_delta = delta.clone();
        Ok(delta)
    }
    pub(crate) fn reset_spatial_history(&mut self, id: &EntityId, interpolate: Option<bool>) {
        let ids = self.descendants(id);
        let mut changes = Vec::new();
        for id in ids {
            if let Some(&index) = self.spatial_timeline.indices.get(&id) {
                let row = &mut self.spatial_timeline.rows[index];
                if let Some(interpolate) = interpolate {
                    row.disabled = !interpolate;
                }
                let mut reset = (*row.current).clone();
                reset.state.previous_world_matrix = reset.state.world_matrix;
                row.current = Arc::new(reset);
                row.previous = row.current.clone();
                changes.push(SpatialChange {
                    runtime_id: row.current.runtime_id,
                    entity_id: id.clone(),
                    field_mask: SpatialFieldMask::HISTORY,
                    pose: Some(row.current.clone()),
                });
                row.world.previous_world_matrix = row.world.world_matrix;
                self.spatial_timeline.set_active(index, false);
                self.spatial_timeline.set_pending(index);
            }
        }
        if !changes.is_empty() {
            self.spatial_timeline.merge_delta(0, changes);
        }
    }
}

#[cfg(test)]
mod packed_history_tests {
    use super::*;

    #[test]
    fn deferred_delta_matches_ordered_merges_and_preserves_retained_snapshots() {
        let change = |i: u32, x: Option<f32>, mask| {
            let id: EntityId = format!("node-{i}").into();
            SpatialChange {
                runtime_id: RuntimeEntityId::new(i, 0),
                entity_id: id.clone(),
                field_mask: mask,
                pose: x.map(|x| Arc::new(pose(&id, i, 0, 1, x))),
            }
        };
        let mut eager = World::new();
        eager.spatial_timeline.tick = 1;
        eager
            .spatial_timeline
            .merge_delta(1, vec![change(0, Some(0.), SpatialFieldMask::TRANSLATION)]);
        let old = eager.spatial_delta();
        let mut deferred = eager.clone();
        let groups = (0..100)
            .map(|n| {
                let i = n % 17;
                vec![
                    change(i, Some(n as f32), SpatialFieldMask::ROTATION),
                    change(
                        i,
                        (n % 5 != 0).then_some(n as f32 + 1.),
                        SpatialFieldMask::SCALE,
                    ),
                ]
            })
            .collect::<Vec<_>>();
        DELTA_MATERIALIZATIONS.with(|n| n.set(0));
        for group in &groups {
            eager.spatial_timeline.merge_delta(2, group.clone());
        }
        assert_eq!(DELTA_MATERIALIZATIONS.with(|n| n.get()), 100);
        DELTA_MATERIALIZATIONS.with(|n| n.set(0));
        deferred.with_deferred_spatial_delta(|world| {
            for group in groups {
                world.spatial_timeline.merge_delta(2, group);
            }
        });
        assert_eq!(DELTA_MATERIALIZATIONS.with(|n| n.get()), 1);
        assert_eq!(deferred.spatial_delta(), eager.spatial_delta());
        assert_eq!(old.changes.len(), 1);
        assert_eq!(old.dirty_roots, 1);
        assert_eq!(old.changes[0].field_mask, SpatialFieldMask::TRANSLATION);
        let before_noop = deferred.spatial_delta();
        deferred.with_deferred_spatial_delta(|_| ());
        assert!(Arc::ptr_eq(&before_noop, &deferred.spatial_delta()));
    }

    #[test]
    fn deferred_delta_flushes_partial_errors_and_unwinds_without_poisoning_later_commits() {
        let change = |i: u32| SpatialChange {
            runtime_id: RuntimeEntityId::new(i, 0),
            entity_id: format!("node-{i}").into(),
            field_mask: SpatialFieldMask::TRANSFORM,
            pose: None,
        };
        let mut expected = World::new();
        expected.spatial_timeline.merge_delta(1, vec![change(1)]);
        expected.spatial_timeline.merge_delta(1, vec![change(2)]);
        let mut error = World::new();
        let result: Result<(), &str> = error.with_deferred_spatial_delta(|world| {
            world.spatial_timeline.merge_delta(1, vec![change(1)]);
            world.with_deferred_spatial_delta(|world| {
                world.spatial_timeline.merge_delta(1, vec![change(2)]);
            });
            Err("original publication failure")
        });
        assert_eq!(result, Err("original publication failure"));
        assert_eq!(error.spatial_delta(), expected.spatial_delta());
        let mut unwound = World::new();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unwound.with_deferred_spatial_delta(|world| {
                world.spatial_timeline.merge_delta(1, vec![change(1)]);
                world.spatial_timeline.merge_delta(1, vec![change(2)]);
                panic!("publication panic");
            });
        }));
        assert!(panic.is_err());
        assert_eq!(unwound.spatial_delta(), expected.spatial_delta());
        assert!(!unwound.spatial_timeline.defer_delta);
        expected.spatial_timeline.merge_delta(1, vec![change(3)]);
        unwound.spatial_timeline.merge_delta(1, vec![change(3)]);
        assert_eq!(unwound.spatial_delta(), expected.spatial_delta());
    }

    #[test]
    fn first_delta_moves_unique_rows_and_later_merge_preserves_masks_and_old_readers() {
        for count in [1usize, 100, 1000, 10000] {
            let mut t = SpatialTimeline::default();
            let changes = (0..count)
                .rev()
                .map(|i| SpatialChange {
                    runtime_id: RuntimeEntityId::new(i as u32, 0),
                    entity_id: format!("node-{i}").into(),
                    field_mask: SpatialFieldMask::TRANSLATION,
                    pose: None,
                })
                .collect();
            DELTA_MAP_INSERTS.with(|n| n.set(0));
            t.merge_delta(count, changes);
            assert_eq!(
                DELTA_MAP_INSERTS.with(|n| n.get()),
                0,
                "initial unique publication must not create a redundant full merge map at {count}"
            );
            assert_eq!(t.delta.affected_count, count);
            assert!(t
                .delta
                .changes
                .windows(2)
                .all(|p| p[0].runtime_id < p[1].runtime_id));
            let retained = t.delta.clone();
            let last = (count - 1) as u32;
            let update = |mask| SpatialChange {
                runtime_id: RuntimeEntityId::new(last, 0),
                entity_id: format!("node-{last}").into(),
                field_mask: mask,
                pose: None,
            };
            t.merge_delta(
                2,
                vec![
                    update(SpatialFieldMask::ROTATION),
                    update(SpatialFieldMask::SCALE),
                ],
            );
            assert_eq!(t.delta.affected_count, count);
            assert_eq!(t.delta.dirty_roots, count + 2);
            assert_eq!(
                t.delta.changes.last().unwrap().field_mask,
                SpatialFieldMask::TRANSFORM
            );
            assert_eq!(
                retained.changes.last().unwrap().field_mask,
                SpatialFieldMask::TRANSLATION
            );
        }
        let mut t = SpatialTimeline::default();
        let id = EntityId::from("same");
        let make = |x, mask| SpatialChange {
            runtime_id: RuntimeEntityId::new(7, 0),
            entity_id: id.clone(),
            field_mask: mask,
            pose: Some(Arc::new(pose(&id, 7, 0, 1, x))),
        };
        t.merge_delta(
            2,
            vec![
                make(1., SpatialFieldMask::ROTATION),
                make(2., SpatialFieldMask::SCALE),
            ],
        );
        assert_eq!(t.delta.changes.len(), 1);
        assert_eq!(
            t.delta.changes[0].pose.as_ref().unwrap().local.translation[0],
            2.
        );
        assert_eq!(t.delta.changes[0].field_mask.0, 6);
    }

    #[test]
    fn packed_spare_is_observed_and_weak_reader_prevents_body_reuse() {
        let mut t = SpatialTimeline::default();
        let id = EntityId::from("spare");
        t.publish(id.clone(), pose(&id, 0, 0, 1, 0.));
        let spare = Arc::new(pose(&id, 0, 0, 1, 1.));
        let weak = Arc::downgrade(&spare);
        let address = Arc::as_ptr(&spare) as usize;
        t.rows[0].spare = Some(spare);
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        t.observe_memory(&mut sample);
        assert!(sample
            .allocations
            .iter()
            .any(|a| a.allocation_id == format!("cpu-storage:{address:x}")
                && a.bytes == std::mem::size_of::<CommittedSpatialPose>() as u64));
        let next = t.publish(id.clone(), pose(&id, 0, 0, 2, 2.));
        assert_ne!(Arc::as_ptr(&next) as usize, address);
        assert!(weak.upgrade().is_none());
        assert!(t.rows[0].spare.is_none());
        t.remove(&id);
        assert!(t.rows.is_empty());
    }

    #[test]
    fn interpolation_linear_work_is_shared_only_for_identical_inputs() {
        let mut previous = CanonicalTransform3D {
            translation: [0.; 3],
            rotation_xyzw: [0., 0., 0., 1.],
            scale: [1.; 3],
        };
        let mut current = previous.clone();
        for count in [1usize, 16, 128, 1024] {
            let mut cache = None;
            INTERPOLATION_LINEAR_BUILDS.with(|n| n.set(0));
            for i in 0..count {
                previous.translation = [i as f32, 0., 0.];
                current.translation = [i as f32 + 2., 0., 0.];
                let got = interpolated_linear(&previous, &current, 0.25, &mut cache);
                assert_eq!(got, ([0., 0., 0., 1.], [1.; 3]));
            }
            assert_eq!(
                INTERPOLATION_LINEAR_BUILDS.with(|n| n.get()),
                1,
                "actual nlerp/scale work at N={count}"
            );
        }
    }

    #[test]
    fn root_presentation_shares_linear_math_in_the_production_pass() {
        for count in [1usize, 16, 128, 1024] {
            let mut world = World::new();
            for i in 0..count {
                let id = EntityId::from(format!("pass-{i}"));
                world
                    .spatial_timeline
                    .publish(id.clone(), pose(&id, i as u32, 0, 1, 2.));
                world
                    .spatial_timeline
                    .publish(id.clone(), pose(&id, i as u32, 0, 2, 10.));
            }
            INTERPOLATION_LINEAR_BUILDS.with(|n| n.set(0));
            let delta = world.prepare_spatial_presentation(3, 0.25).unwrap();
            assert_eq!(delta.changed.len(), count);
            assert_eq!(INTERPOLATION_LINEAR_BUILDS.with(|n| n.get()), 1);
            assert!(world.spatial_timeline.rows.iter().all(|r| r
                .presentation
                .as_ref()
                .unwrap()
                .world_matrix[12]
                == 4.));
        }
    }

    #[test]
    fn interpolated_linear_reuse_matches_scalar_math_and_invalidates_each_input() {
        let mut cache = None;
        for i in 0..256 {
            let q = [0.1, (i as f32 * 0.02).sin(), 0.2, 1.];
            let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            let previous = CanonicalTransform3D {
                translation: [0.; 3],
                rotation_xyzw: q.map(|v| v / n),
                scale: [1., -2., if i % 2 == 0 { -0. } else { 0. }],
            };
            let current = CanonicalTransform3D {
                translation: [0.; 3],
                rotation_xyzw: q.map(|v| -v / n),
                scale: [3., -1., 0.],
            };
            for alpha in [0., 0.25, 0.5, 0.75, 1.] {
                let a = previous.rotation_xyzw;
                let b = current.rotation_xyzw;
                let sign = if a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>() < 0. {
                    -1.
                } else {
                    1.
                };
                let q = std::array::from_fn::<_, 4, _>(|i| a[i] + (sign * b[i] - a[i]) * alpha);
                let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
                let rotation = q.map(|v| v / norm);
                let scale = std::array::from_fn::<_, 3, _>(|i| {
                    previous.scale[i] + (current.scale[i] - previous.scale[i]) * alpha
                });
                for _ in 0..2 {
                    let actual = interpolated_linear(&previous, &current, alpha, &mut cache);
                    assert_eq!(actual.0.map(f32::to_bits), rotation.map(f32::to_bits));
                    assert_eq!(actual.1.map(f32::to_bits), scale.map(f32::to_bits));
                }
            }
        }
    }

    #[test]
    fn root_presentation_endpoints_use_committed_pose_and_keep_dynamic_middle() {
        for count in [1usize, 100, 1000, 10000] {
            let mut t = SpatialTimeline::default();
            for i in 0..count {
                let id = EntityId::from(format!("endpoint-{i}"));
                t.publish(id.clone(), pose(&id, i as u32, 0, 1, 0.));
                t.publish(id.clone(), pose(&id, i as u32, 0, 2, 10.));
            }
            let old = t.rows[0].previous.clone();
            let mut cache = HashMap::new();
            PRESENTATION_LOCAL_REBUILDS.with(|n| n.set(0));
            for i in 0..count {
                assert_eq!(
                    t.evaluate(i, 0., &mut cache, false).unwrap().world_matrix[12],
                    0.
                );
                assert_eq!(
                    t.evaluate(i, 1., &mut cache, false).unwrap().world_matrix[12],
                    10.
                );
            }
            assert_eq!(
                PRESENTATION_LOCAL_REBUILDS.with(|n| n.get()),
                0,
                "committed root endpoints must not rebuild interpolation/matrices at {count}"
            );
            for i in 0..count {
                assert_eq!(
                    t.evaluate(i, 0.5, &mut cache, false).unwrap().world_matrix[12],
                    5.
                );
            }
            assert_eq!(PRESENTATION_LOCAL_REBUILDS.with(|n| n.get()), count);
            assert_eq!(old.state.world_matrix[12], 0.);
            // Ancestor-cache evaluation retains its original tree path.
            PRESENTATION_LOCAL_REBUILDS.with(|n| n.set(0));
            t.evaluate(0, 1., &mut cache, true).unwrap();
            assert_eq!(PRESENTATION_LOCAL_REBUILDS.with(|n| n.get()), 1);
            assert_eq!(cache.len(), 1);
            t.rows[0].disabled = true;
            PRESENTATION_LOCAL_REBUILDS.with(|n| n.set(0));
            assert_eq!(
                t.evaluate(0, 0.5, &mut HashMap::new(), false)
                    .unwrap()
                    .world_matrix[12],
                10.
            );
            assert_eq!(PRESENTATION_LOCAL_REBUILDS.with(|n| n.get()), 0);
            t.rows[0].disabled = false;
            Arc::make_mut(&mut t.rows[0].current).state.authored_visible = false;
            assert!(
                !t.evaluate(0, 0., &mut HashMap::new(), false)
                    .unwrap()
                    .authored_visible,
                "endpoint geometry must retain current non-interpolated metadata"
            );
            Arc::make_mut(&mut t.rows[0].previous).state.anchor = [64., 0., 0.];
            PRESENTATION_LOCAL_REBUILDS.with(|n| n.set(0));
            t.evaluate(0, 0., &mut HashMap::new(), false).unwrap();
            assert_eq!(
                PRESENTATION_LOCAL_REBUILDS.with(|n| n.get()),
                1,
                "a mismatched history basis cannot authorize endpoint reuse"
            );
        }
        // A child cannot reuse its root-local matrix as the final world matrix.
        let mut t = SpatialTimeline::default();
        let parent = EntityId::from("parent");
        let child = EntityId::from("child");
        t.publish(parent.clone(), pose(&parent, 0, 0, 1, 10.));
        let mut value = pose(&child, 1, 0, 1, 5.);
        value.parent = Some(parent);
        t.publish(child, value);
        assert_eq!(
            t.evaluate(1, 1., &mut HashMap::new(), false)
                .unwrap()
                .world_matrix[12],
            15.
        );
    }

    #[test]
    fn translating_root_middle_reuses_only_the_identical_computed_linear_matrix() {
        for count in [1usize, 16, 128, 1024] {
            let mut t = SpatialTimeline::default();
            for i in 0..count {
                let id = EntityId::from(format!("translated-{i}"));
                t.publish(id.clone(), pose(&id, i as u32, 0, 1, 2.));
                t.publish(id.clone(), pose(&id, i as u32, 0, 2, 10.));
            }
            let frozen = t.rows[0].previous.clone();
            PRESENTATION_MATRIX_REBUILDS.with(|n| n.set(0));
            for i in 0..count {
                let mut scalar = t.rows[i].current.local.clone();
                scalar.translation[0] = 4.;
                let actual = t.evaluate(i, 0.25, &mut HashMap::new(), false).unwrap();
                assert_eq!(
                    actual.world_matrix.map(f32::to_bits),
                    matrix(&scalar).map(f32::to_bits)
                );
                assert_eq!(actual.pose.position, scalar.translation);
                assert_eq!(actual.version, t.rows[i].current.state.version);
            }
            assert_eq!(
                PRESENTATION_MATRIX_REBUILDS.with(|n| n.get()),
                0,
                "unchanged computed linear matrices must not be rebuilt at N={count}"
            );
            assert_eq!(frozen.local.translation[0], 2.);
            // Rotation, scale, parent/basis and generation changes are not the
            // same proof, even if a cached numeric matrix happens to match.
            for case in ["rotation", "scale", "parent", "anchor", "generation"] {
                let current = t.rows[0].current.clone();
                let previous = t.rows[0].previous.clone();
                match case {
                    "rotation" => {
                        Arc::make_mut(&mut t.rows[0].previous).local.rotation_xyzw =
                            [0., 0., 1., 0.]
                    }
                    "scale" => Arc::make_mut(&mut t.rows[0].previous).local.scale = [2.; 3],
                    "parent" => {
                        Arc::make_mut(&mut t.rows[0].current).parent =
                            Some(EntityId::from("missing-parent"))
                    }
                    "anchor" => Arc::make_mut(&mut t.rows[0].previous).state.anchor = [64., 0., 0.],
                    "generation" => {
                        Arc::make_mut(&mut t.rows[0].previous).runtime_id =
                            RuntimeEntityId::new(0, 7)
                    }
                    _ => unreachable!(),
                }
                PRESENTATION_MATRIX_REBUILDS.with(|n| n.set(0));
                t.evaluate(0, 0.25, &mut HashMap::new(), false).unwrap();
                assert_eq!(PRESENTATION_MATRIX_REBUILDS.with(|n| n.get()), 1, "{case}");
                t.rows[0].current = current;
                t.rows[0].previous = previous;
            }
        }
    }

    fn pose(id: &EntityId, index: u32, generation: u32, tick: u64, x: f32) -> CommittedSpatialPose {
        let local = CanonicalTransform3D {
            translation: [x, 0., 0.],
            rotation_xyzw: [0., 0., 0., 1.],
            scale: [1.; 3],
        };
        let state = evaluate_node(
            id,
            Node {
                anchor: [0.; 3],
                parent: None,
                transform: Some(local.clone()),
                enabled: true,
                visible: true,
                camera: false,
            },
            None,
        )
        .unwrap();
        CommittedSpatialPose {
            runtime_id: RuntimeEntityId::new(index, generation),
            tick,
            local,
            parent: None,
            state,
        }
    }

    fn assert_indices(t: &SpatialTimeline) {
        assert_eq!(t.indices.len(), t.rows.len());
        for (index, row) in t.rows.iter().enumerate() {
            assert_eq!(t.indices.get(&row.entity_id), Some(&index));
            if let Some(position) = row.active_index {
                assert_eq!(t.active[position], index);
            }
            if let Some(position) = row.pending_index {
                assert_eq!(t.pending[position], index);
            }
        }
        for (position, &index) in t.active.iter().enumerate() {
            assert_eq!(t.rows[index].active_index, Some(position));
        }
        for (position, &index) in t.pending.iter().enumerate() {
            assert_eq!(t.rows[index].pending_index, Some(position));
        }
    }

    #[test]
    fn packed_history_repairs_both_worklists_when_rows_move_at_scale() {
        for count in [1usize, 100, 1000, 10000] {
            let mut t = SpatialTimeline::default();
            for i in 0..count {
                let id = EntityId::from(format!("node-{i:05}"));
                t.publish(id.clone(), pose(&id, i as u32, 0, 1, 0.));
                t.publish(id.clone(), pose(&id, i as u32, 0, 1, 1.));
                t.publish(id.clone(), pose(&id, i as u32, 0, 1, 2.));
            }
            assert_eq!(
                (t.rows.len(), t.active.len(), t.pending.len()),
                (count, count, count)
            );
            assert_indices(&t);
            let mut removed = [0, count / 2, count - 1]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            for i in &removed {
                t.remove(&EntityId::from(format!("node-{i:05}")));
                assert_indices(&t);
            }
            let remaining = count - removed.len();
            assert_eq!(
                (t.rows.len(), t.active.len(), t.pending.len()),
                (remaining, remaining, remaining)
            );
            for i in std::mem::take(&mut removed) {
                let id = EntityId::from(format!("node-{i:05}"));
                t.publish(id.clone(), pose(&id, i as u32, 1, 2, 5.));
                let row = t.row(&id).unwrap();
                assert_eq!(row.previous.runtime_id.generation, 1);
                assert_eq!(
                    row.previous.state.world_matrix,
                    row.current.state.world_matrix
                );
            }
            assert_indices(&t);
            assert_eq!(t.rows.len(), count);
            assert!(t.rows.capacity() < 2 * count.max(4));
            eprintln!(
                "actual packed history: rows={}, active={}, pending={}, rowCapacity={}",
                t.rows.len(),
                t.active.len(),
                t.pending.len(),
                t.rows.capacity()
            );
        }
    }

    #[test]
    fn packed_delta_orders_runtime_handles_and_retains_independent_old_readers() {
        let mut t = SpatialTimeline::default();
        let mut changes = Vec::new();
        for index in [9, 2, 7] {
            let id = EntityId::from(format!("id-{index}"));
            let p = t.publish(id.clone(), pose(&id, index, 0, 1, 1.));
            changes.push(SpatialChange {
                runtime_id: p.runtime_id,
                entity_id: id,
                field_mask: SpatialFieldMask::TRANSLATION,
                pose: Some(p),
            });
        }
        t.merge_delta(3, changes);
        let retained = t.delta.clone();
        assert_eq!(
            retained
                .changes
                .iter()
                .map(|c| c.runtime_id.index)
                .collect::<Vec<_>>(),
            vec![2, 7, 9]
        );
        t.remove(&"id-7".into());
        assert!(t
            .delta
            .changes
            .iter()
            .find(|c| c.runtime_id.index == 7)
            .unwrap()
            .pose
            .is_none());
        assert_eq!(
            retained
                .changes
                .iter()
                .find(|c| c.runtime_id.index == 7)
                .unwrap()
                .pose
                .as_ref()
                .unwrap()
                .local
                .translation,
            [1., 0., 0.]
        );
        assert_indices(&t);
    }
}
