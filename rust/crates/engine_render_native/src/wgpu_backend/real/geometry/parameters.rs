//! Derived GPU parameter backing. Resource identity and visibility stay with their owners.
use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

pub(super) const INSTANCE_FLOATS: usize = 32;
pub(super) const VIEW_FLOATS: usize = 44;
pub(super) const INSTANCE_BYTES: u64 = INSTANCE_FLOATS as u64 * 4;
pub(super) const VIEW_BYTES: u64 = VIEW_FLOATS as u64 * 4;
pub(super) const INSTANCE_ARENA_BYTES: u64 = MAX_GEOMETRY_INSTANCES as u64 * INSTANCE_BYTES;
// Existing native plan limit: 16 color + 192 shadow views. Each can select
// scene illumination or explicit preview, so at most two parameter variants.
pub(super) const MAX_COLOR_VIEWS: usize = 16;
pub(super) const MAX_SHADOW_VIEWS: usize = 192;
pub(super) const MAX_VIEW_RECORDS: usize = 2 * (MAX_COLOR_VIEWS + MAX_SHADOW_VIEWS);
pub(super) const VIEW_ARENA_BYTES: u64 = MAX_VIEW_RECORDS as u64 * VIEW_BYTES;
pub(super) const INDEX_BYTES: u64 = 12;

// Upload only changed contiguous rows. A shorter selection does not require
// clearing unselected GPU rows; no command may address them.
pub(super) fn index_updates(old: &[[u32; 3]], next: &[[u32; 3]]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, row) in next.iter().enumerate() {
        if old.get(index) != Some(row) {
            start.get_or_insert(index);
        } else if let Some(first) = start.take() {
            ranges.push(first..index);
        }
    }
    if let Some(first) = start {
        ranges.push(first..next.len());
    }
    ranges
}

struct RecordKey<const N: usize>(u64, [u32; N], u64, AtomicU32);

impl<const N: usize> Clone for RecordKey<N> {
    fn clone(&self) -> Self {
        Self(self.0, self.1, self.2, AtomicU32::new(u32::MAX))
    }
}
impl<const N: usize> PartialEq for RecordKey<N> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1 && self.2 == other.2
    }
}
impl<const N: usize> Eq for RecordKey<N> {}

impl<const N: usize> RecordKey<N> {
    fn new(id: u64, data: [f32; N]) -> Self {
        #[cfg(test)]
        if N == INSTANCE_FLOATS {
            INSTANCE_RECORD_KEYS_BUILT.with(|n| n.set(n.get() + 1));
        }
        let bits = data.map(f32::to_bits);
        let mut fingerprint = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&(id, bits), &mut fingerprint);
        Self(
            id,
            bits,
            std::hash::Hasher::finish(&fingerprint),
            AtomicU32::new(u32::MAX),
        )
    }
}

impl<const N: usize> std::hash::Hash for RecordKey<N> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        #[cfg(test)]
        RECORD_KEY_HASHES.with(|n| n.set(n.get() + 1));
        // A cached hash selects a bucket only. Eq still compares id and every
        // float bit; a hash collision cannot merge different records.
        std::hash::Hash::hash(&self.2, state);
    }
}

pub(super) struct Records<const N: usize> {
    slots: HashMap<Arc<RecordKey<N>>, u32>,
    // Exact live immutable ownership validates a hint against this arena and
    // its current occupant. The atomic is only a hint, never upload success.
    owners: Vec<Option<Arc<RecordKey<N>>>>,
    free: BTreeSet<u32>,
    pub(super) content: Vec<Option<[f32; N]>>,
}

impl<const N: usize> Default for Records<N> {
    fn default() -> Self {
        Self::with_capacity(MAX_GEOMETRY_INSTANCES)
    }
}

impl<const N: usize> Records<N> {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: HashMap::new(),
            owners: vec![None; capacity],
            free: (0..capacity as u32).collect(),
            content: vec![None; capacity],
        }
    }
    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_vec(&self.content, "renderer", &["resident"]);
        sample.cpu_vec(&self.owners, "renderer", &["resident"]);
        let mut seen = std::collections::HashSet::new();
        for key in self.slots.keys().chain(self.owners.iter().flatten()) {
            if seen.insert(Arc::as_ptr(key)) {
                sample.cpu_arc(key, "renderer", &["resident"]);
            }
        }
        sample.partial(
            "cpu.owner.renderer",
            "parameterSlotHashBucketsAndFreeTree+ParallelPreparationUpdateScratchNotInstrumented",
        );
    }
    #[cfg(test)]
    fn stage(&mut self, values: &[(u64, [f32; N])]) -> (Vec<u32>, BTreeMap<u32, [f32; N]>) {
        let keyed: Vec<_> = values
            .iter()
            .map(|(id, data)| (RecordKey::new(*id, *data), *data))
            .collect();
        self.stage_keys(&keyed)
    }

    #[cfg(test)]
    fn stage_keys(
        &mut self,
        values: &[(RecordKey<N>, [f32; N])],
    ) -> (Vec<u32>, BTreeMap<u32, [f32; N]>) {
        self.stage_records(
            &values
                .iter()
                .map(|(key, _)| Arc::new(key.clone()))
                .collect::<Vec<_>>(),
        )
    }

    fn stage_records(
        &mut self,
        values: &[Arc<RecordKey<N>>],
    ) -> (Vec<u32>, BTreeMap<u32, [f32; N]>) {
        // Resolve surviving complete keys once. Membership is then a slot fact,
        // not a second hash table containing all parameter bytes. This bitmap
        // is call-local and bounded by this arena's existing record capacity.
        let mut active = vec![false; self.content.len()];
        let mut active_count = 0;
        let keys: Vec<_> = values
            .iter()
            .map(|key| {
                let hint = key.3.load(Ordering::Relaxed);
                let previous = self
                    .owners
                    .get(hint as usize)
                    .and_then(Option::as_ref)
                    .filter(|owner| Arc::ptr_eq(owner, key))
                    .map(|_| hint)
                    .or_else(|| self.slots.get(key).copied());
                if let Some(slot) = previous {
                    if !active[slot as usize] {
                        active[slot as usize] = true;
                        active_count += 1;
                    }
                }
                previous
            })
            .collect();
        // Preserve pure CPU parameter positions through visibility changes.
        // These bounded keys own no draw/resource/lease. Reclaim inactive keys
        // before assigning new records only when this fixed arena needs room;
        // full-capacity replacement still frees obsolete slots first.
        let needed = keys.iter().filter(|key| key.is_none()).count();
        // Camera records intentionally recycle immediately: moving the same
        // camera must not change its index in every instance selection row.
        if active_count != self.slots.len() && (N != INSTANCE_FLOATS || needed > self.free.len()) {
            self.slots.retain(|_, slot| {
                #[cfg(test)]
                RECORD_RETAIN_VISITS.with(|n| n.set(n.get() + 1));
                if active[*slot as usize] {
                    true
                } else {
                    self.free.insert(*slot);
                    self.owners[*slot as usize] = None;
                    false
                }
            });
        }
        let mut indices = Vec::with_capacity(values.len());
        let mut dirty = BTreeMap::new();
        for (previous, key) in keys.into_iter().zip(values) {
            let slot = previous.unwrap_or_else(|| {
                *self
                    .slots
                    .entry(key.clone())
                    .or_insert_with(|| self.free.pop_first().expect("validated draw capacity"))
            });
            self.owners[slot as usize] = Some(key.clone());
            key.3.store(slot, Ordering::Relaxed);
            if self.content[slot as usize].map(|old| old.map(f32::to_bits)) != Some(key.1) {
                dirty.insert(slot, key.1.map(f32::from_bits));
            }
            indices.push(slot);
        }
        (indices, dirty)
    }
}

pub(super) struct Update {
    pub(super) instances: BTreeMap<u32, [f32; INSTANCE_FLOATS]>,
    pub(super) views: BTreeMap<u32, [f32; VIEW_FLOATS]>,
    pub(super) indices: Vec<[u32; 3]>,
}

struct InstanceValue {
    // Weak prevents address recycling and dissociates COW without retaining
    // an instance, its resources, or its manager lease.
    source: std::sync::Weak<crate::render_geometry::DrawInstance>,
    // Pure parameter bytes shared with Records and this call's selection.
    // This Arc has no instance/resource/lease, nor any uploaded-content fact.
    key: Arc<RecordKey<INSTANCE_FLOATS>>,
    // An index into this one stage's local values, never a retained visible
    // result, GPU slot or uploaded-content fact.
    selected: Option<(u64, usize)>,
}

pub(super) struct InstanceValues {
    indices: HashMap<usize, usize>,
    entries: Vec<InstanceValue>,
    capacity: usize,
    selection_serial: u64,
    next_prune: usize,
}
impl Default for InstanceValues {
    fn default() -> Self {
        Self {
            indices: HashMap::new(),
            entries: Vec::new(),
            capacity: MAX_GEOMETRY_INSTANCES,
            selection_serial: 0,
            next_prune: 128,
        }
    }
}
impl InstanceValues {
    fn resolve(
        &mut self,
        instance: &Arc<crate::render_geometry::DrawInstance>,
    ) -> [f32; INSTANCE_FLOATS] {
        let key = Arc::as_ptr(instance) as usize;
        if let Some(&index) = self.indices.get(&key) {
            let entry = &self.entries[index];
            debug_assert_eq!(entry.source.as_ptr(), Arc::as_ptr(instance));
            return entry.key.1.map(f32::from_bits);
        }
        #[cfg(test)]
        INSTANCE_VALUES_BUILT.with(|n| n.set(n.get() + 1));
        let mut data = [0.; INSTANCE_FLOATS];
        data[..16].copy_from_slice(&instance.world_transform.map(OrderedF32::to_f32));
        data[16..20].copy_from_slice(&instance.base_color.map(OrderedF32::to_f32));
        if let Some(lit) = &instance.standard_lit {
            data[20..24].copy_from_slice(&lit.surface.map(OrderedF32::to_f32));
            data[24..28].copy_from_slice(&lit.emissive.map(OrderedF32::to_f32));
        }
        data[28] = instance.alpha_cutoff.map_or(0., OrderedF32::to_f32);
        if self.entries.len() == self.capacity {
            self.entries.clear();
            self.indices.clear();
            self.next_prune = 128;
        }
        if self.capacity != 0 {
            self.indices.insert(key, self.entries.len());
            self.entries.push(InstanceValue {
                source: Arc::downgrade(instance),
                key: Arc::new(RecordKey::new(instance.instance_id, data)),
                selected: None,
            });
        }
        data
    }

    fn begin_selection(&mut self) {
        // Prune only at a selection boundary so local selection indices remain
        // valid throughout staging. Live immutable instances keep their keys.
        if self.entries.len() >= self.next_prune {
            self.entries
                .retain(|entry| entry.source.strong_count() != 0);
            self.indices.clear();
            self.indices.extend(
                self.entries
                    .iter()
                    .enumerate()
                    .map(|(i, entry)| (entry.source.as_ptr() as usize, i)),
            );
            self.next_prune = self.entries.len().saturating_add(128);
        }
        self.selection_serial = self.selection_serial.wrapping_add(1);
        if self.selection_serial == 0 {
            for entry in &mut self.entries {
                entry.selected = None;
            }
            self.selection_serial = 1;
        }
    }

    fn select(
        &mut self,
        instance: &Arc<crate::render_geometry::DrawInstance>,
        values: &mut Vec<Arc<RecordKey<INSTANCE_FLOATS>>>,
    ) -> usize {
        #[cfg(test)]
        INSTANCE_SELECTION_LOOKUPS.with(|n| n.set(n.get() + 1));
        let identity = Arc::as_ptr(instance) as usize;
        let entry_index = match self.indices.get(&identity).copied() {
            Some(index) => index,
            None => {
                let data = self.resolve(instance);
                if self.capacity == 0 {
                    let index = values.len();
                    values.push(Arc::new(RecordKey::new(instance.instance_id, data)));
                    return index;
                }
                self.entries.len() - 1
            }
        };
        let entry = &mut self.entries[entry_index];
        if let Some((serial, index)) = entry.selected {
            if serial == self.selection_serial {
                return index;
            }
        }
        let index = values.len();
        values.push(entry.key.clone());
        entry.selected = Some((self.selection_serial, index));
        index
    }

    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_vec(&self.entries, "renderer", &["resident"]);
        for entry in &self.entries {
            sample.cpu_arc(&entry.key, "renderer", &["resident"]);
        }
        sample.partial(
            "cpu.owner.renderer",
            "instanceParameterIndexHashBucketsAndWeakAllocationBackingNotInstrumented",
        );
    }
}

#[cfg(test)]
pub(super) fn stage(
    draws: &[&DrawGeometry],
    preview: [[f32; 4]; 3],
    derived: &mut InstanceValues,
    instances: &mut Records<INSTANCE_FLOATS>,
    views: &mut Records<VIEW_FLOATS>,
) -> Update {
    stage_selected(
        draws
            .iter()
            .map(|draw| (*draw, draw.native_instance(), draw.camera_depth)),
        preview,
        derived,
        instances,
        views,
    )
}

// One call's ordinal map only. The plan owns every borrowed immutable table
// throughout this function; no ordinal or native-slot success survives the call.
pub(super) fn stage_payloads<'a>(
    payloads: impl IntoIterator<Item = &'a RhiDrawPayload>,
    preview: [[f32; 4]; 3],
    derived: &mut InstanceValues,
    instances: &mut Records<INSTANCE_FLOATS>,
    views: &mut Records<VIEW_FLOATS>,
) -> Update {
    derived.begin_selection();
    let mut instance_values = Vec::new();
    let mut indices = Vec::new();
    let mut view_values = Vec::new();
    let mut view_memo = HashMap::new();
    let mut previous_view = None;
    let mut tables = HashMap::new();
    let mut ordinals: Vec<Vec<u32>> = Vec::new();
    let mut reserved_rows = 0;
    for payload in payloads {
        if payload.geometry_instance_count() == 0 {
            continue;
        }
        let draw = payload.geometry_draw().expect("selected geometry head");
        let view_key = (Arc::as_ptr(&draw.view), draw.lighting.is_some());
        let view_index = if let Some((_, index)) = previous_view.filter(|(old, _)| *old == view_key)
        {
            index
        } else {
            let index = *view_memo.entry(view_key).or_insert_with(|| {
                let index = view_values.len();
                view_values.push(view_record(draw, preview));
                index
            });
            previous_view = Some((view_key, index));
            index
        };
        let intensity = draw
            .environment
            .as_ref()
            .map_or(0., |e| e.intensity.to_f32());
        if let RhiDrawPayload::GeometryBatch { batch } = payload {
            let table_key = Arc::as_ptr(&batch.instances);
            let group = tables.get(&table_key).copied().or_else(|| {
                if batch.instances.len() > MAX_GEOMETRY_INSTANCES - reserved_rows {
                    return None;
                }
                let index = ordinals.len();
                reserved_rows += batch.instances.len();
                ordinals.push(vec![u32::MAX; batch.instances.len()]);
                tables.insert(table_key, index);
                Some(index)
            });
            for entry in &batch.visible {
                #[cfg(test)]
                PARAMETER_SELECTION_VISITS.with(|n| n.set(n.get() + 1));
                let row = entry.instance_index as usize;
                let previous = group.map_or(u32::MAX, |group| ordinals[group][row]);
                let index = if previous != u32::MAX {
                    previous
                } else {
                    let index = derived.select(&batch.instances[row], &mut instance_values) as u32;
                    if let Some(group) = group {
                        ordinals[group][row] = index;
                    }
                    index
                };
                indices.push([index, view_index as u32, intensity.to_bits()]);
            }
        } else {
            for (_, instance, _) in payload.geometry_instances() {
                #[cfg(test)]
                PARAMETER_SELECTION_VISITS.with(|n| n.set(n.get() + 1));
                let index = derived.select(instance, &mut instance_values);
                indices.push([index as u32, view_index as u32, intensity.to_bits()]);
            }
        }
    }
    #[cfg(test)]
    PARAMETER_SELECTED_BYTES.with(|n| n.set(std::mem::size_of_val(instance_values.as_slice())));
    let (instance_slots, instances) = instances.stage_records(&instance_values);
    let (view_slots, views) = views.stage_records(&view_values);
    for pair in &mut indices {
        pair[0] = instance_slots[pair[0] as usize];
        pair[1] = view_slots[pair[1] as usize];
    }
    Update {
        instances,
        views,
        indices,
    }
}

fn view_record(draw: &DrawGeometry, preview: [[f32; 4]; 3]) -> Arc<RecordKey<VIEW_FLOATS>> {
    let view = &draw.view;
    let mut data = [0.; VIEW_FLOATS];
    data[..16].copy_from_slice(&view.view_projection());
    let v = view.view_matrix.map(OrderedF32::to_f32);
    data[16] = -(v[0] * v[12] + v[1] * v[13] + v[2] * v[14]);
    data[17] = -(v[4] * v[12] + v[5] * v[13] + v[6] * v[14]);
    data[18] = -(v[8] * v[12] + v[9] * v[13] + v[10] * v[14]);
    data[19] =
        if view.projection_matrix[15].to_f32() == 1. && view.projection_matrix[11].to_f32() == 0. {
            1.
        } else {
            0.
        };
    data[20..23].copy_from_slice(&[v[2], v[6], v[10]]);
    if draw.lighting.is_none() {
        data[24..28].copy_from_slice(&preview[0]);
        data[28..32].copy_from_slice(&preview[1]);
        data[32..36].copy_from_slice(&preview[2]);
    }
    if let Some(pipeline) = &view.color_pipeline {
        data[32..35].copy_from_slice(&pipeline.ambient_diffuse.map(OrderedF32::to_f32));
        if let Some(fog) = &pipeline.fog {
            data[36..39].copy_from_slice(&fog.color.map(OrderedF32::to_f32));
            data[39] = fog.density_per_m.to_f32();
            data[40] = fog.start_distance_m.to_f32();
        }
    }
    Arc::new(RecordKey::new(view.view_id, data))
}

// Independent former flattened consumer retained only as a test oracle.
#[cfg(test)]
pub(super) fn stage_selected<'a>(
    draws: impl IntoIterator<
        Item = (
            &'a DrawGeometry,
            &'a Arc<crate::render_geometry::DrawInstance>,
            OrderedF32,
        ),
    >,
    preview: [[f32; 4]; 3],
    derived: &mut InstanceValues,
    instances: &mut Records<INSTANCE_FLOATS>,
    views: &mut Records<VIEW_FLOATS>,
) -> Update {
    // Borrowed backing is immutable for this call. A key is shared pure CPU
    // data; Records still resolves ownership and actual uploaded bits below.
    derived.begin_selection();
    let mut instance_values = Vec::new();
    let mut indices = Vec::new();
    // Only borrowed for this immutable prepare; same-ID/detached views still
    // resolve through full record equality, never pointer identity across calls.
    let mut memo = HashMap::new();
    let mut view_values = Vec::new();
    let mut previous_view = None;
    for (draw, instance, _) in draws {
        let intensity = draw
            .environment
            .as_ref()
            .map_or(0., |e| e.intensity.to_f32());
        let instance_index = derived.select(instance, &mut instance_values);
        #[cfg(test)]
        PARAMETER_SELECTION_VISITS.with(|n| n.set(n.get() + 1));
        let key = (Arc::as_ptr(&draw.view), draw.lighting.is_some());
        if let Some((old, index)) = previous_view {
            if old == key {
                indices.push([instance_index as u32, index as u32, intensity.to_bits()]);
                continue;
            }
        }
        let index = *memo.entry(key).or_insert_with(|| {
            let view = &draw.view;
            let mut data = [0.; VIEW_FLOATS];
            data[..16].copy_from_slice(&view.view_projection());
            let v = view.view_matrix.map(OrderedF32::to_f32);
            data[16] = -(v[0] * v[12] + v[1] * v[13] + v[2] * v[14]);
            data[17] = -(v[4] * v[12] + v[5] * v[13] + v[6] * v[14]);
            data[18] = -(v[8] * v[12] + v[9] * v[13] + v[10] * v[14]);
            data[19] = if view.projection_matrix[15].to_f32() == 1.
                && view.projection_matrix[11].to_f32() == 0.
            {
                1.
            } else {
                0.
            };
            data[20..23].copy_from_slice(&[v[2], v[6], v[10]]);
            if draw.lighting.is_none() {
                data[24..28].copy_from_slice(&preview[0]);
                data[28..32].copy_from_slice(&preview[1]);
                data[32..36].copy_from_slice(&preview[2]);
            }
            if let Some(pipeline) = &view.color_pipeline {
                data[32..35].copy_from_slice(&pipeline.ambient_diffuse.map(OrderedF32::to_f32));
                if let Some(fog) = &pipeline.fog {
                    data[36..39].copy_from_slice(&fog.color.map(OrderedF32::to_f32));
                    data[39] = fog.density_per_m.to_f32();
                    data[40] = fog.start_distance_m.to_f32();
                }
            }
            let index = view_values.len();
            view_values.push(Arc::new(RecordKey::new(view.view_id, data)));
            index
        });
        indices.push([instance_index as u32, index as u32, intensity.to_bits()]);
        previous_view = Some((key, index));
    }
    #[cfg(test)]
    PARAMETER_SELECTED_BYTES.with(|n| n.set(std::mem::size_of_val(instance_values.as_slice())));
    let (instance_slots, instances) = instances.stage_records(&instance_values);
    let (view_slots, views) = views.stage_records(&view_values);
    // Resolve the one compact mapping in place; no second full selected Vec
    // or zip/collect allocation. Slot assignment remains call-local and retryable.
    for pair in &mut indices {
        pair[0] = instance_slots[pair[0] as usize];
        pair[1] = view_slots[pair[1] as usize];
    }
    Update {
        instances,
        views,
        indices,
    }
}

pub(super) fn write<const N: usize>(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    records: &mut Records<N>,
    mut dirty: BTreeMap<u32, [f32; N]>,
) -> (u64, u64) {
    let mut writes = 0;
    let mut bytes = 0;
    while let Some((first, data)) = dirty.pop_first() {
        let mut values = Vec::from(data);
        records.content[first as usize] = Some(data);
        let mut end = first + 1;
        while let Some(data) = dirty.remove(&end) {
            records.content[end as usize] = Some(data);
            values.extend_from_slice(&data);
            end += 1;
        }
        queue.write_buffer(
            buffer,
            first as u64 * N as u64 * 4,
            bytemuck::cast_slice(&values),
        );
        writes += 1;
        bytes += values.len() as u64 * 4;
    }
    (writes, bytes)
}

#[cfg(test)]
thread_local! {
    static PARAMETER_SELECTED_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PARAMETER_SELECTION_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RECORD_RETAIN_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static INSTANCE_VALUES_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RECORD_KEY_HASHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static INSTANCE_RECORD_KEYS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static INSTANCE_SELECTION_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_parameter_slots_survive_visibility_and_reclaim_only_inactive_keys_at_capacity() {
        let keys: Vec<_> = (0..12)
            .map(|id| Arc::new(RecordKey::new(id, [id as f32; INSTANCE_FLOATS])))
            .collect();
        let mut records = Records::with_capacity(8);
        assert_eq!(records.stage_records(&keys[..4]).0, [0, 1, 2, 3]);
        assert_eq!(
            records.stage_records(&[keys[0].clone(), keys[2].clone()]).0,
            [0, 2]
        );
        assert_eq!(
            records
                .stage_records(&[
                    keys[3].clone(),
                    keys[0].clone(),
                    keys[1].clone(),
                    keys[2].clone()
                ])
                .0,
            [3, 0, 1, 2],
            "invisible records lost their native position despite free arena capacity"
        );
        records.stage_records(&keys[4..8]);
        assert_eq!(records.slots.len(), 8);
        let (slots, dirty) =
            records.stage_records(&[keys[0].clone(), keys[8].clone(), keys[9].clone()]);
        assert_eq!(
            slots[0], 0,
            "capacity eviction displaced a current exact key"
        );
        assert_eq!(records.slots.len(), 3);
        assert_eq!(dirty[&slots[1]], [8.; INSTANCE_FLOATS]);
        assert_eq!(dirty[&slots[2]], [9.; INSTANCE_FLOATS]);
        assert!(records.owners.iter().flatten().count() <= 8);
    }
    #[test]
    fn retained_slot_reuses_immutable_records_without_content_key_queries() {
        for count in [4, 32, 256] {
            let keys: Vec<_> = (0..count)
                .map(|i| Arc::new(RecordKey::new(i as u64, [i as f32; 4])))
                .collect();
            let mut records = Records::<4>::with_capacity(count);
            let (first, pending) = records.stage_records(&keys);
            RECORD_KEY_HASHES.with(|n| n.set(0));
            let (second, retry) = records.stage_records(&keys);
            let queries = RECORD_KEY_HASHES.with(|n| n.get());
            assert_eq!(second, first);
            assert_eq!(retry, pending, "slot reuse cannot hide an unwritten record");
            assert_eq!(queries, 0, "{count} unchanged records queried full keys");
        }
    }

    #[test]
    fn retained_slot_hints_check_live_arena_owner_after_recycling_and_cross_arena_use() {
        let a = Arc::new(RecordKey::new(1, [1.; 2]));
        let b = Arc::new(RecordKey::new(2, [2.; 2]));
        let mut first = Records::with_capacity(2);
        let mut second = Records::with_capacity(2);
        let first_slots = first.stage_records(&[a.clone(), b.clone()]).0;
        let other_slots = second.stage_records(&[b.clone(), a.clone()]).0;
        assert_eq!(first_slots, [0, 1]);
        assert_eq!(other_slots, [0, 1]);
        // Both hints now describe the other arena. Exact live ownership must
        // reject them, rather than associating the wrong parameter values.
        assert_eq!(first.stage_records(&[a.clone(), b.clone()]).0, first_slots);
        let c = Arc::new(RecordKey::new(3, [3.; 2]));
        first.stage_records(std::slice::from_ref(&c));
        let back = first.stage_records(std::slice::from_ref(&a));
        assert_eq!(back.1.len(), 1, "a recycled hint hid pending content");
        assert_eq!(back.1[&back.0[0]], [1.; 2]);
        let detached = Arc::new(a.as_ref().clone());
        assert_eq!(first.stage_records(&[detached]).0, back.0);
        assert_eq!(second.stage_records(&[a.clone(), b.clone()]).0, [1, 0]);
    }

    #[test]
    fn retained_environment_context_does_not_duplicate_instance_records() {
        use crate::environment_lighting::{EnvironmentLighting, EnvironmentMap, ENVIRONMENT_MAGIC};
        let mut words = vec![0u32; 32 + 6 * 5 * 3 + 32 * 32 * 2];
        words[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, 2, 2, 32]);
        let map = EnvironmentMap::from_bytes(
            &words
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        for count in [4, 32, 256] {
            let frame =
                crate::render_geometry::batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let batches = frame.views[0].batches.as_ref().unwrap();
            let mut payloads = Vec::new();
            for intensity in [0., 2.] {
                for source in batches {
                    let mut batch = source.as_ref().clone();
                    Arc::make_mut(&mut batch.draw).environment =
                        Some(Arc::new(EnvironmentLighting {
                            map: map.clone(),
                            intensity: intensity.into(),
                        }));
                    payloads.push(RhiDrawPayload::GeometryBatch {
                        batch: Arc::new(batch),
                    });
                }
            }
            let update = stage_payloads(
                &payloads,
                [[0.; 4]; 3],
                &mut InstanceValues::default(),
                &mut Records::with_capacity(count * 2),
                &mut Records::with_capacity(MAX_VIEW_RECORDS),
            );
            assert_eq!(update.indices.len(), count * 2);
            assert_eq!(
                update.instances.len(),
                count,
                "environment duplicated {count} instance records"
            );
            for row in 0..count {
                assert_eq!(update.indices[row][0], update.indices[row + count][0]);
                assert_eq!(update.indices[row][2], 0f32.to_bits());
                assert_eq!(update.indices[row + count][2], 2f32.to_bits());
            }
        }
    }

    #[test]
    fn flow_batched_parameters_match_flat_oracle_across_cow_retry_and_cache_limits() {
        use crate::environment_lighting::{EnvironmentLighting, EnvironmentMap, ENVIRONMENT_MAGIC};
        let frame = crate::render_geometry::batch_test_frame(true, MaterialBlend::Opaque, 32, 1);
        let batches = frame.views[0].batches.as_ref().unwrap();
        let original = batches[0].clone();
        let mut words = vec![0u32; 32 + 6 * 5 * 3 + 32 * 32 * 2];
        words[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, 2, 2, 32]);
        let map = EnvironmentMap::from_bytes(
            &words
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let mut payloads: Vec<_> = batches
            .iter()
            .map(|batch| RhiDrawPayload::GeometryBatch {
                batch: batch.clone(),
            })
            .collect();
        // Detached backing, sparse reversed order, same IDs with different bytes,
        // view and intensity: none of these may reuse another table's ordinal.
        let mut detached = original.as_ref().clone();
        detached.instances = Arc::new(original.instances.as_ref().clone());
        Arc::make_mut(&mut Arc::make_mut(&mut detached.instances)[0]).world_transform[12] =
            17f32.into();
        detached.visible.reverse();
        detached.visible.retain(|v| v.instance_index % 2 == 0);
        Arc::make_mut(&mut Arc::make_mut(&mut detached.draw).view).view_matrix[12] = 3f32.into();
        Arc::make_mut(&mut detached.draw).environment = Some(Arc::new(EnvironmentLighting {
            map: map.clone(),
            intensity: 2f32.into(),
        }));
        payloads.push(RhiDrawPayload::GeometryBatch {
            batch: Arc::new(detached),
        });
        let mut variant = original.as_ref().clone();
        Arc::make_mut(&mut variant.draw).environment = Some(Arc::new(EnvironmentLighting {
            map,
            intensity: (-0f32).into(),
        }));
        payloads.push(RhiDrawPayload::GeometryBatch {
            batch: Arc::new(variant),
        });
        payloads.push(RhiDrawPayload::Geometry {
            draw: original.draw.clone(),
        });
        payloads.push(RhiDrawPayload::GeometryBatch { batch: original });
        for capacity in [0, 3, MAX_GEOMETRY_INSTANCES] {
            let mut current = InstanceValues::default();
            let mut reference = InstanceValues::default();
            current.capacity = capacity;
            reference.capacity = capacity;
            current.selection_serial = u64::MAX;
            reference.selection_serial = u64::MAX;
            let mut instances = Records::with_capacity(128);
            let mut old_instances = Records::with_capacity(128);
            let mut views = Records::with_capacity(MAX_VIEW_RECORDS);
            let mut old_views = Records::with_capacity(MAX_VIEW_RECORDS);
            for attempt in 0..4 {
                let a = stage_payloads(
                    &payloads,
                    [[0.5; 4]; 3],
                    &mut current,
                    &mut instances,
                    &mut views,
                );
                let b = stage_selected(
                    payloads.iter().flat_map(RhiDrawPayload::geometry_instances),
                    [[0.5; 4]; 3],
                    &mut reference,
                    &mut old_instances,
                    &mut old_views,
                );
                assert_eq!(
                    a.indices, b.indices,
                    "capacity={capacity} attempt={attempt}"
                );
                assert_eq!(a.instances, b.instances);
                assert_eq!(a.views, b.views);
                // Attempt 0 leaves all writes pending. Attempt 1 commits only
                // half the records; retry must preserve every outstanding write.
                if attempt > 0 {
                    for (slot, data) in &a.instances {
                        if attempt > 1 || slot % 2 == 0 {
                            instances.content[*slot as usize] = Some(*data);
                            old_instances.content[*slot as usize] = Some(*data);
                        }
                    }
                    for (slot, data) in &a.views {
                        views.content[*slot as usize] = Some(*data);
                        old_views.content[*slot as usize] = Some(*data);
                    }
                }
            }
        }
    }

    #[test]
    fn flow_ordinal_scratch_budget_falls_back_without_changing_parameters() {
        let frame = crate::render_geometry::batch_test_frame(true, MaterialBlend::Opaque, 4, 1);
        let mut batch = frame.views[0].batches.as_ref().unwrap()[0].as_ref().clone();
        let row = batch.instances[0].clone();
        // Sparse table larger than the scratch budget: only selected rows are
        // parameter records; ordinal storage must not become an admission limit.
        let mut table = batch.instances.as_ref().clone();
        table.resize(MAX_GEOMETRY_INSTANCES + 1, row);
        batch.instances = Arc::new(table);
        let payloads = [RhiDrawPayload::GeometryBatch {
            batch: Arc::new(batch),
        }];
        let a = stage_payloads(
            &payloads,
            [[0.; 4]; 3],
            &mut InstanceValues::default(),
            &mut Records::with_capacity(4),
            &mut Records::with_capacity(MAX_VIEW_RECORDS),
        );
        let b = stage_selected(
            payloads.iter().flat_map(RhiDrawPayload::geometry_instances),
            [[0.; 4]; 3],
            &mut InstanceValues::default(),
            &mut Records::with_capacity(4),
            &mut Records::with_capacity(MAX_VIEW_RECORDS),
        );
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.instances, b.instances);
        assert_eq!(a.views, b.views);
    }
    #[test]
    fn flow_parameter_batches_resolve_each_table_row_and_intensity_once() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let frame =
                crate::render_geometry::batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let batches = frame.views[0].batches.as_ref().unwrap();
            let payloads: Vec<_> = batches
                .iter()
                .chain(batches.iter().rev())
                .map(|batch| RhiDrawPayload::GeometryBatch {
                    batch: batch.clone(),
                })
                .collect();
            let mut derived = InstanceValues::default();
            let mut records = Records::with_capacity(count);
            let mut views = Records::with_capacity(MAX_VIEW_RECORDS);
            INSTANCE_SELECTION_LOOKUPS.with(|n| n.set(0));
            let result = stage_payloads(
                &payloads,
                [[0.; 4]; 3],
                &mut derived,
                &mut records,
                &mut views,
            );
            actual.push((count, INSTANCE_SELECTION_LOOKUPS.with(|n| n.get())));
            assert_eq!(result.indices.len(), count * 2);
            assert_eq!(result.instances.len(), count);
        }
        eprintln!("actual instance-identity lookup calls: {actual:?}");
        assert!(actual.iter().all(|(rows, calls)| *rows == *calls));
    }
    #[test]
    fn consumption_stable_parameters_share_values_and_visit_selection_once_at_multiple_scales() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let source = plan.commands.iter().find_map(geometry_draw).unwrap();
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let draws: Vec<_> = (0..count)
                .map(|i| {
                    let mut d = source.clone();
                    d.instance_id = i as u64;
                    d.world_transform[12] = (i as f32).into();
                    d
                })
                .collect();
            let selected: Vec<_> = draws.iter().chain(draws.iter().rev()).collect();
            let mut derived = InstanceValues::default();
            let mut records = Records::with_capacity(count);
            let mut views = Records::with_capacity(MAX_VIEW_RECORDS);
            let first = stage(
                &selected,
                [[0.; 4]; 3],
                &mut derived,
                &mut records,
                &mut views,
            );
            PARAMETER_SELECTED_BYTES.with(|n| n.set(0));
            PARAMETER_SELECTION_VISITS.with(|n| n.set(0));
            let second = stage(
                &selected,
                [[0.; 4]; 3],
                &mut derived,
                &mut records,
                &mut views,
            );
            assert_eq!(second.indices, first.indices);
            assert_eq!(
                second.instances, first.instances,
                "CPU reuse hid pending upload"
            );
            actual.push((
                count,
                PARAMETER_SELECTED_BYTES.with(|n| n.get()),
                PARAMETER_SELECTION_VISITS.with(|n| n.get()),
            ));
        }
        println!("(unique, selected value storage bytes, actual selection rows): {actual:?}");
        assert!(
            actual.iter().all(|(n, bytes, visits)| *bytes
                == n * std::mem::size_of::<Arc<RecordKey<INSTANCE_FLOATS>>>()
                && *visits == n * 2),
            "shared values expanded or selection rescanned: {actual:?}"
        );
    }
    #[test]
    fn consumption_all_active_slots_skip_retain_scan_and_replacements_still_release() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let values: Vec<_> = (0..count).map(|i| (i as u64, [i as f32; 2])).collect();
            let mut records = Records::<2>::with_capacity(count);
            let first = records.stage(&values);
            let mut duplicates = values.clone();
            duplicates.reverse();
            duplicates.extend_from_slice(&values);
            RECORD_RETAIN_VISITS.with(|n| n.set(0));
            let second = records.stage(&duplicates);
            actual.push((count, RECORD_RETAIN_VISITS.with(|n| n.get())));
            assert_eq!(&second.0[count..], first.0.as_slice());
            let replacement: Vec<_> = values
                .iter()
                .map(|(id, data)| (id + count as u64, *data))
                .collect();
            let replaced = records.stage(&replacement);
            assert_eq!(replaced.0.len(), count);
            assert_eq!(records.slots.len(), count);
            assert_eq!(replaced.1.len(), count);
        }
        assert!(
            actual.iter().all(|(_, visits)| *visits == 0),
            "actual retained slot scan visits: {actual:?}"
        );
    }

    #[test]
    fn cached_record_hash_collisions_keep_full_keys_and_pending_content_distinct() {
        let a = (RecordKey::new(7, [0., 1.]), [0., 1.]);
        let mut b = (RecordKey::new(7, [-0., 1.]), [-0., 1.]);
        b.0 .2 = a.0 .2; // Force a real HashMap bucket collision.
        let mut records = Records::<2>::with_capacity(2);
        let (slots, dirty) = records.stage_keys(&[a.clone(), b.clone()]);
        assert_ne!(slots[0], slots[1]);
        assert_eq!(dirty.len(), 2);
        assert_eq!(dirty[&slots[0]][0].to_bits(), 0f32.to_bits());
        assert_eq!(dirty[&slots[1]][0].to_bits(), (-0f32).to_bits());
        let (reordered, retry) = records.stage_keys(&[b.clone(), a.clone()]);
        assert_eq!(reordered, [slots[1], slots[0]]);
        assert_eq!(
            retry.len(),
            2,
            "CPU key reuse cannot mark pending content clean"
        );
        for (slot, value) in retry {
            records.content[slot as usize] = Some(value);
        }
        assert!(records.stage_keys(&[a, b]).1.is_empty());
    }

    #[test]
    fn instance_selection_is_call_local_through_overflow_serial_wrap_and_zero_capacity() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let original = plan.commands.iter().find_map(geometry_draw).unwrap();
        let mut a = original.clone();
        a.world_transform[12] = 1f32.into();
        let mut b = original.clone();
        b.world_transform[12] = 2f32.into();
        let mut c = original.clone();
        c.world_transform[12] = 3f32.into();
        let mut derived = InstanceValues {
            capacity: 2,
            ..Default::default()
        };
        let mut records = Records::with_capacity(3);
        let mut views = Records::with_capacity(MAX_VIEW_RECORDS);
        let first = stage(
            &[&a, &b, &a, &c, &a],
            [[0.; 4]; 3],
            &mut derived,
            &mut records,
            &mut views,
        );
        assert_eq!(first.instances.len(), 3);
        assert_eq!(first.indices[0], first.indices[2]);
        assert_eq!(first.indices[0], first.indices[4]);
        assert_ne!(first.indices[0][0], first.indices[1][0]);
        assert_ne!(first.indices[1][0], first.indices[3][0]);
        for (slot, data) in &first.instances {
            records.content[*slot as usize] = Some(*data);
        }
        for (slot, data) in &first.views {
            views.content[*slot as usize] = Some(*data);
        }
        // Overflow may forget CPU derivations but must not retain a selection
        // index into a previous call's differently ordered local values.
        derived.selection_serial = u64::MAX;
        let second = stage(
            &[&c, &b, &a, &c],
            [[0.; 4]; 3],
            &mut derived,
            &mut records,
            &mut views,
        );
        assert!(second.instances.is_empty());
        for (entry, expected) in second.indices.iter().zip([3., 2., 1., 3.]) {
            assert_eq!(records.content[entry[0] as usize].unwrap()[12], expected);
        }
        let mut uncached = InstanceValues {
            capacity: 0,
            ..Default::default()
        };
        let strict = stage(
            &[&c, &b, &a, &c],
            [[0.; 4]; 3],
            &mut uncached,
            &mut records,
            &mut views,
        );
        assert_eq!(strict.indices, second.indices);
        assert!(strict.instances.is_empty());
        assert!(uncached.entries.is_empty());
    }

    #[test]
    fn records_preserve_bit_distinct_replacements_and_pending_gpu_dirty() {
        let mut records = Records::<2>::with_capacity(1);
        let original = [(7, [0., 1.])];
        let replacement = [(7, [-0., 1.])];
        let (slots, dirty) = records.stage(&original);
        let slot = slots[0];
        records.content[slot as usize] = Some(dirty[&slot]);
        let (next, dirty) = records.stage(&replacement);
        assert_eq!(next, slots, "full capacity must reuse the inactive slot");
        assert_eq!(
            dirty.len(),
            1,
            "bit-distinct replacement incorrectly inherited uploaded GPU content"
        );
        assert_eq!(
            dirty[&slot].map(f32::to_bits),
            replacement[0].1.map(f32::to_bits)
        );
        let (_, retry) = records.stage(&replacement);
        assert_eq!(
            retry[&slot].map(f32::to_bits),
            replacement[0].1.map(f32::to_bits)
        );
        records.content[slot as usize] = Some(dirty[&slot]);
        assert!(records.stage(&replacement).1.is_empty());
        let (_, back) = records.stage(&original);
        assert_eq!(
            back[&slot].map(f32::to_bits),
            original[0].1.map(f32::to_bits)
        );
    }

    #[test]
    fn cpu_parameter_derivation_cow_overflow_and_expiry_never_pin_resources() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let draw = plan.commands.iter().find_map(geometry_draw).unwrap();
        let mut source = Arc::new(draw.instance.as_ref().clone());
        let mut cache = InstanceValues {
            capacity: 2,
            ..Default::default()
        };
        let before = cache.resolve(&source);
        assert_eq!(Arc::strong_count(&source), 1);
        assert!(Arc::get_mut(&mut source).is_none());
        let weak = Arc::downgrade(&source);
        Arc::make_mut(&mut source).world_transform[12] = 3f32.into();
        assert!(
            weak.upgrade().is_none(),
            "weak CPU derivation retained old instance"
        );
        let changed = cache.resolve(&source);
        assert_ne!(before, changed);
        let mut third = Arc::new(source.as_ref().clone());
        Arc::make_mut(&mut third).world_transform[12] = 4f32.into();
        let third_values = cache.resolve(&third);
        assert_eq!(
            cache.entries.len(),
            1,
            "overflow must clear only derived CPU facts"
        );
        assert_eq!(third_values[12], 4.);
        assert_eq!(changed[29], 0., "environment is selection context");
        let changed_weak = Arc::downgrade(&source);
        drop(source);
        assert!(changed_weak.upgrade().is_none());
        for i in 0..64 {
            let mut row = draw.instance.as_ref().clone();
            row.world_transform[12] = (i as f32).into();
            let row = Arc::new(row);
            let packed = cache.resolve(&row);
            assert_eq!(packed[12], i as f32, "recycled input borrowed old values");
            assert!(cache.entries.len() <= 2);
        }
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        cache.observe_memory(&mut sample);
        assert!(sample.allocations.iter().any(|a| a.owner == "renderer"
            && a.bytes
                == (cache.entries.capacity() * std::mem::size_of::<InstanceValue>()) as u64));
    }

    #[test]
    fn spine374_f_instance_values_reclaims_dead_entries_before_capacity() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let draw = plan.commands.iter().find_map(geometry_draw).unwrap();
        let live = Arc::new(draw.instance.as_ref().clone());
        let mut cache = InstanceValues::default();
        let expected = cache.resolve(&live);
        let live_key = cache.entries[0].key.clone();
        let mut dead_key = None;
        for frame in 1..=4200 {
            cache.begin_selection();
            let mut transient = Arc::new(live.as_ref().clone());
            Arc::make_mut(&mut transient).world_transform[12] = (frame as f32).into();
            assert_eq!(cache.resolve(&transient)[12], frame as f32);
            if frame == 1 {
                dead_key = Some(Arc::downgrade(&cache.entries.last().unwrap().key));
            }
            assert_eq!(cache.resolve(&live), expected);
            let index = cache.indices[&(Arc::as_ptr(&live) as usize)];
            assert!(Arc::ptr_eq(&cache.entries[index].key, &live_key));
            assert!(
                cache.entries.len() <= 129,
                "dead parameter derivations accumulated at frame {frame}"
            );
            assert_eq!(cache.entries.len(), cache.indices.len());
        }
        assert!(dead_key.unwrap().upgrade().is_none());
    }

    #[test]
    fn resident_records_resolve_full_keys_once_and_keep_slots_when_reordered() {
        let values: Vec<_> = (0..128).map(|i| (i, [i as f32; INSTANCE_FLOATS])).collect();
        let mut records = Records::with_capacity(values.len());
        let (slots, dirty) = records.stage(&values);
        for (slot, data) in dirty {
            records.content[slot as usize] = Some(data);
        }
        let mut reversed = values.clone();
        reversed.reverse();
        RECORD_KEY_HASHES.with(|n| n.set(0));
        let (actual, dirty) = records.stage(&reversed);
        assert_eq!(actual, slots.into_iter().rev().collect::<Vec<_>>());
        assert!(dirty.is_empty());
        assert!(
            RECORD_KEY_HASHES.with(|n| n.get()) <= values.len(),
            "resident record content hashed {} times for {} records",
            RECORD_KEY_HASHES.with(|n| n.get()),
            values.len()
        );
        // All old records can be replaced at the full capacity; no spare slot
        // may be needed before inactive records are released.
        let replacement: Vec<_> = values.iter().map(|(id, data)| (id + 128, *data)).collect();
        let (new, _) = records.stage(&replacement);
        assert_eq!(new.len(), values.len());
        assert_eq!(records.slots.len(), values.len());
    }

    #[test]
    fn shared_instances_build_once_but_world_and_environment_context_stay_correct() {
        use crate::environment_lighting::{EnvironmentLighting, EnvironmentMap, ENVIRONMENT_MAGIC};
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let original = plan
            .commands
            .iter()
            .find_map(geometry_draw)
            .unwrap()
            .clone();
        let mut other_view = original.clone();
        Arc::make_mut(&mut other_view.view).view_matrix[12] = 1f32.into();
        let mut other_world = original.clone();
        other_world.world_transform[12] = 2f32.into();
        let mut words = vec![0u32; 32 + 6 * (4 + 1) * 3 + 32 * 32 * 2];
        words[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, 2, 2, 32]);
        let map = EnvironmentMap::from_bytes(
            &words
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let mut environment_a = original.clone();
        environment_a.environment = Some(Arc::new(EnvironmentLighting {
            map: map.clone(),
            intensity: 1f32.into(),
        }));
        let mut environment_b = original.clone();
        environment_b.environment = Some(Arc::new(EnvironmentLighting {
            map,
            intensity: 2f32.into(),
        }));
        assert!(Arc::ptr_eq(
            &environment_a.instance,
            &environment_b.instance
        ));
        INSTANCE_VALUES_BUILT.with(|n| n.set(0));
        let mut instances = Records::default();
        let mut views = Records::with_capacity(MAX_VIEW_RECORDS);
        let mut derived = InstanceValues::default();
        let update = stage(
            &[
                &original,
                &other_view,
                &other_world,
                &environment_a,
                &environment_b,
            ],
            [[0.; 4]; 3],
            &mut derived,
            &mut instances,
            &mut views,
        );
        assert_eq!(INSTANCE_VALUES_BUILT.with(|n| n.get()), 2);
        assert_eq!(update.instances.len(), 2);
        assert_eq!(update.indices[0][0], update.indices[1][0]);
        assert_ne!(update.indices[0][1], update.indices[1][1]);
        assert_ne!(update.indices[0][0], update.indices[2][0]);
        assert_eq!(update.indices[0][0], update.indices[3][0]);
        assert_eq!(update.indices[3][0], update.indices[4][0]);
        assert_eq!(update.indices[3][2], 1f32.to_bits());
        assert_eq!(update.indices[4][2], 2f32.to_bits());
        INSTANCE_VALUES_BUILT.with(|n| n.set(0));
        INSTANCE_RECORD_KEYS_BUILT.with(|n| n.set(0));
        let retry = stage(
            &[
                &original,
                &other_view,
                &other_world,
                &environment_a,
                &environment_b,
            ],
            [[0.; 4]; 3],
            &mut derived,
            &mut instances,
            &mut views,
        );
        assert_eq!(
            INSTANCE_VALUES_BUILT.with(|n| n.get()),
            0,
            "unchanged immutable instance parameters rebuilt across frames"
        );
        assert_eq!(
            INSTANCE_RECORD_KEYS_BUILT.with(|n| n.get()),
            0,
            "stable native instance parameters rebuilt their complete record key"
        );
        assert_eq!(
            retry.instances, update.instances,
            "pure CPU reuse hid pending GPU uploads"
        );
    }

    #[test]
    fn records_keep_equal_content_stable_but_separate_same_id_origins_and_material_values() {
        let mut records = Records::<4>::default();
        let (a, dirty) = records.stage(&[(7, [1., 2., 3., 4.]), (7, [1., 2., 3., 4.])]);
        assert_eq!(a[0], a[1]);
        assert_eq!(dirty.len(), 1);
        records.content[a[0] as usize] = Some([1., 2., 3., 4.]);
        let (b, dirty) = records.stage(&[(7, [5., 2., 3., 4.]), (7, [1., 2., 3., 4.])]);
        assert_ne!(b[0], b[1]);
        assert_eq!(
            b[1], a[0],
            "ordering changes cannot reassign a surviving record"
        );
        assert_eq!(dirty.len(), 1);
        // Staging isn't a successful GPU upload. A retry must remain dirty.
        let (_, retry) = records.stage(&[(7, [5., 2., 3., 4.]), (7, [1., 2., 3., 4.])]);
        assert_eq!(retry, dirty);
        let (c, _) = records.stage(&[(7, [1., 2., 3., 4.]), (8, [1., 2., 3., 4.])]);
        assert_ne!(
            c[0], c[1],
            "separate instance IDs must retain separate identity"
        );
    }
}

// One declaration for every production geometry shader, including opaque depth.
pub(super) const WGSL: &str = "
struct InstanceParameters { world:mat4x4<f32>, color:vec4<f32>, surface:vec4<f32>, emissive:vec4<f32>, extra:vec4<f32> };
struct ViewParameters { matrix:mat4x4<f32>, camera:vec4<f32>, forward:vec4<f32>, light:vec4<f32>, radiance:vec4<f32>, ambient:vec4<f32>, fog:vec4<f32>, fog_settings:vec4<f32> };
@group(0) @binding(0) var<storage,read> instances:array<InstanceParameters>;
@group(0) @binding(1) var<storage,read> views:array<ViewParameters>;
struct Uniforms { matrix:mat4x4<f32>, color:vec4<f32>, extra:vec4<f32>, world:mat4x4<f32>, surface:vec4<f32>, emissive:vec4<f32>, camera:vec4<f32>, light:vec4<f32>, radiance:vec4<f32>, ambient:vec4<f32>, fog:vec4<f32>, fog_settings:vec4<f32> };
fn load_parameters(slot:vec3<u32>)->Uniforms {
    let i=instances[slot.x]; let v=views[slot.y]; var u:Uniforms;
    u.matrix=v.matrix; u.world=i.world; u.color=i.color; u.surface=i.surface; u.emissive=i.emissive;
    u.extra=vec4<f32>(i.extra.x,v.forward.xyz); u.camera=v.camera; u.light=v.light; u.radiance=v.radiance;
    u.ambient=vec4<f32>(v.ambient.xyz,bitcast<f32>(slot.z)); u.fog=v.fog; u.fog_settings=v.fog_settings; return u;
}
";
