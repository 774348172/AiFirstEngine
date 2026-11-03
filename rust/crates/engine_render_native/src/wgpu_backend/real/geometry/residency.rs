//! Live mesh-source comparison inside GeometryGpuState; no retained residency proof.
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// Equality scratch for one immutable preparation call. All inputs outlive the
/// scratch; it owns no source, native handle, or cross-frame residency proof.
pub(super) struct SourceComparisons<'a, V> {
    equal: std::collections::HashSet<(*const V, *const V)>,
    borrowed: std::marker::PhantomData<&'a V>,
}

impl<V> Default for SourceComparisons<'_, V> {
    fn default() -> Self {
        Self {
            equal: std::collections::HashSet::new(),
            borrowed: std::marker::PhantomData,
        }
    }
}

impl<'a, V: PartialEq> SourceComparisons<'a, V> {
    pub(super) fn matches(&mut self, a: &'a Arc<V>, b: &'a Arc<V>) -> bool {
        if Arc::ptr_eq(a, b) {
            return true;
        }
        let pair = (Arc::as_ptr(a), Arc::as_ptr(b));
        if self.equal.contains(&pair) {
            return true;
        }
        if a != b {
            return false;
        }
        self.equal.insert(pair);
        true
    }
}

/// Sources admitted during one immutable native prepare. Residency and source
/// conflicts are checked before any uploads; nothing is retained across calls.
pub(super) struct PlanSources<'a, K, V> {
    sources: HashMap<K, &'a Arc<V>>,
    comparisons: SourceComparisons<'a, V>,
    last: Option<(K, *const V)>,
    dense: Vec<Option<(K, &'a Arc<V>)>>,
    ordinal: Option<fn(&K) -> usize>,
}

impl<K, V> Default for PlanSources<'_, K, V> {
    fn default() -> Self {
        Self {
            sources: HashMap::new(),
            comparisons: SourceComparisons::default(),
            last: None,
            dense: Vec::new(),
            ordinal: None,
        }
    }
}

impl<'a, K: Copy + Ord + std::hash::Hash, V: PartialEq> PlanSources<'a, K, V> {
    pub(super) fn with_dense_capacity(draws: usize, ordinal: fn(&K) -> usize) -> Self {
        Self {
            dense: vec![None; draws.saturating_mul(2).min(131_072)],
            ordinal: Some(ordinal),
            ..Self::default()
        }
    }
    pub(super) fn include(
        &mut self,
        key: K,
        source: &'a Arc<V>,
        lookup: impl FnOnce(&K) -> Option<&'a Arc<V>>,
    ) -> bool {
        let identity = (key, Arc::as_ptr(source));
        if self.last == Some(identity) {
            self.last = Some(identity);
            return true;
        }
        if let Some(slot) = self
            .ordinal
            .and_then(|ordinal| self.dense.get_mut(ordinal(&key)))
        {
            // Compare the complete key: generations/kinds sharing an ordinal
            // remain distinct and use the bounded sparse path below.
            if slot.as_ref().is_none_or(|(old, _)| *old == key) {
                if slot
                    .as_ref()
                    .is_some_and(|(_, old)| Arc::ptr_eq(old, source))
                {
                    self.last = Some(identity);
                    return true;
                }
                if slot
                    .as_ref()
                    .is_some_and(|(_, old)| !self.comparisons.matches(old, source))
                    || lookup(&key).is_some_and(|old| !self.comparisons.matches(old, source))
                {
                    return false;
                }
                *slot = Some((key, source));
                self.last = Some(identity);
                return true;
            }
        }
        // Admission preserves command order, but does not consume sorted keys.
        // Search one unordered entry; order the unique closure only at handoff.
        let entry = self.sources.entry(key);
        if matches!(&entry, std::collections::hash_map::Entry::Occupied(old)
            if Arc::ptr_eq(old.get(), source))
        {
            // This exact source already passed today's immutable registry.
            self.last = Some(identity);
            return true;
        }
        if matches!(&entry, std::collections::hash_map::Entry::Occupied(old)
            if !self.comparisons.matches(old.get(), source))
            || lookup(&key).is_some_and(|old| !self.comparisons.matches(old, source))
        {
            return false;
        }
        // The resident table remains immutably borrowed until this scratch is
        // consumed. Failed conflicts never enter either successful identity set.
        *entry.or_insert(source) = source;
        self.last = Some(identity);
        true
    }

    pub(super) fn into_sources(self) -> BTreeMap<K, Arc<V>> {
        self.dense
            .into_iter()
            .flatten()
            .chain(self.sources)
            .map(|(key, source)| {
                #[cfg(test)]
                SOURCE_CLOSURE_INSERTIONS.with(|n| n.set(n.get() + 1));
                (key, source.clone())
            })
            .collect()
    }

    pub(super) fn into_ordered_sources(self) -> Vec<(K, Arc<V>)> {
        let mut sources: Vec<_> = self
            .dense
            .into_iter()
            .flatten()
            .chain(self.sources)
            .map(|(key, source)| (key, source.clone()))
            .collect();
        sources.sort_unstable_by_key(|a| a.0);
        sources
    }
}

/// Borrow today's registry for one preparation. Dense indices address scratch
/// directly, while the complete key (including generation/kind) still decides
/// membership. Sparse indices and collisions retain the strict ordered lookup.
pub(super) struct RegistryLookup<'a, K, V> {
    registry: Registry<'a, K, V>,
    dense: Option<Vec<Option<(K, &'a V)>>>,
    ordinal: fn(&K) -> usize,
}
enum Registry<'a, K, V> {
    Tree(&'a BTreeMap<K, V>),
    Ordered(&'a [(K, V)]),
}
#[cfg(test)]
thread_local! {pub(super) static REGISTRY_BUILD_VISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
thread_local! {pub(super) static SOURCE_CLOSURE_INSERTIONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
thread_local! {pub(super) static SOURCE_PUBLICATION_ALLOCATION: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
thread_local! {pub(super) static SOURCE_PUBLICATION_REVISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
impl<'a, K: Copy + Ord + 'a, V: 'a> RegistryLookup<'a, K, V> {
    pub(super) fn new(
        registry: &'a BTreeMap<K, V>,
        draws: usize,
        ordinal: fn(&K) -> usize,
    ) -> Self {
        Self::from_entries(Registry::Tree(registry), registry.iter(), draws, ordinal)
    }
    // The producer's admitted closure is already ordered and unique. Sparse or
    // colliding ordinals use exact binary lookup in this same immutable slice.
    pub(super) fn new_ordered(
        registry: &'a [(K, V)],
        draws: usize,
        ordinal: fn(&K) -> usize,
    ) -> Self {
        Self::from_entries(
            Registry::Ordered(registry),
            registry.iter().map(|(k, v)| (k, v)),
            draws,
            ordinal,
        )
    }
    fn from_entries(
        registry: Registry<'a, K, V>,
        mut entries: impl ExactSizeIterator<Item = (&'a K, &'a V)>,
        draws: usize,
        ordinal: fn(&K) -> usize,
    ) -> Self {
        let capacity = draws.saturating_mul(2).min(131_072);
        let dense = if entries.len() > 1 && entries.len() <= capacity {
            let mut slots = vec![None; capacity];
            let complete = entries.all(|(key, value)| {
                #[cfg(test)]
                REGISTRY_BUILD_VISITS.with(|n| n.set(n.get() + 1));
                let Some(slot) = slots.get_mut(ordinal(key)) else {
                    return false;
                };
                if slot.is_some() {
                    return false;
                }
                *slot = Some((*key, value));
                true
            });
            complete.then_some(slots)
        } else {
            None
        };
        Self {
            registry,
            dense,
            ordinal,
        }
    }
    pub(super) fn get(&self, key: &K) -> Option<&'a V> {
        if let Some(slots) = &self.dense {
            return slots
                .get((self.ordinal)(key))
                .and_then(Option::as_ref)
                .filter(|(candidate, _)| candidate == key)
                .map(|(_, value)| *value);
        }
        match &self.registry {
            Registry::Tree(registry) => registry.get(key),
            Registry::Ordered(registry) => registry
                .binary_search_by(|(candidate, _)| candidate.cmp(key))
                .ok()
                .map(|index| &registry[index].1),
        }
    }
    pub(super) fn is_direct(&self) -> bool {
        self.dense.is_some()
    }
    #[cfg(test)]
    pub(super) fn dense_len(&self) -> usize {
        if self.dense.is_some() {
            match &self.registry {
                Registry::Tree(r) => r.len(),
                Registry::Ordered(r) => r.len(),
            }
        } else {
            0
        }
    }
}

/// `required` is unique and ordered by the complete key from successful preparation.
/// `resident` walks the current table in key order and `resident_count` is its length.
/// Sparse tables use point lookups so unrelated resources cannot make a small closure costly.
pub(crate) fn sources_match<'a, K: Ord + 'a, V: PartialEq + 'a>(
    required: &[(K, Arc<V>)],
    resident_count: usize,
    resident: impl Iterator<Item = (&'a K, &'a Arc<V>)>,
    mut lookup: impl FnMut(&K) -> Option<&'a Arc<V>>,
) -> bool {
    if resident_count <= required.len().saturating_mul(2) {
        let mut resident = resident.peekable();
        return required.iter().all(|(key, source)| {
            while resident
                .peek()
                .is_some_and(|(candidate, _)| *candidate < key)
            {
                resident.next();
            }
            resident
                .peek()
                .is_some_and(|(candidate, value)| *candidate == key && *value == source)
        });
    }
    required
        .iter()
        .all(|(key, source)| lookup(key).is_some_and(|value| value == source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn ordered_selection_lookup_preserves_sparse_and_colliding_full_keys() {
        let dense = [((1u32, 7u32), 10), ((3, 7), 30)];
        let lookup = RegistryLookup::new_ordered(&dense, 4, |key| key.0 as usize);
        assert!(lookup.is_direct());
        assert_eq!(lookup.get(&(1, 7)), Some(&10));
        assert_eq!(lookup.get(&(1, 8)), None);
        let sparse = [((1u32, 7u32), 10), ((1, 8), 20), ((u32::MAX, 7), 30)];
        let lookup = RegistryLookup::new_ordered(&sparse, 4, |key| key.0 as usize);
        assert!(!lookup.is_direct());
        for (key, value) in &sparse {
            assert_eq!(lookup.get(key), Some(value));
        }
        assert_eq!(lookup.get(&(1, 9)), None);
        assert_eq!(lookup.get(&(u32::MAX, 8)), None);
    }

    #[test]
    fn bounded_dense_source_admission_avoids_hash_work_and_keeps_collisions() {
        #[derive(Clone, Copy)]
        struct Key<'a>(u32, u32, &'a Cell<usize>);
        impl PartialEq for Key<'_> {
            fn eq(&self, b: &Self) -> bool {
                (self.0, self.1) == (b.0, b.1)
            }
        }
        impl Eq for Key<'_> {}
        impl PartialOrd for Key<'_> {
            fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(b))
            }
        }
        impl Ord for Key<'_> {
            fn cmp(&self, b: &Self) -> std::cmp::Ordering {
                (self.0, self.1).cmp(&(b.0, b.1))
            }
        }
        impl std::hash::Hash for Key<'_> {
            fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
                self.2.set(self.2.get() + 1);
                (self.0, self.1).hash(h)
            }
        }
        for count in [4, 32, 256, 4096] {
            let hashes = Cell::new(0);
            let source = Arc::new(7);
            let bad = Arc::new(9);
            let mut admitted = PlanSources::with_dense_capacity(count, |k: &Key<'_>| k.0 as usize);
            for index in (0..count as u32).rev() {
                assert!(admitted.include(Key(index, 1, &hashes), &source, |_| Some(&source)));
                assert!(
                    admitted.include(Key(index, 1, &hashes), &source, |_| panic!(
                        "same source rechecked"
                    ))
                );
            }
            assert_eq!(hashes.get(), 0, "dense admission still hashes each source");
            assert!(!admitted.include(Key(1, 1, &hashes), &bad, |_| Some(&source)));
            assert!(admitted.include(Key(1, 2, &hashes), &source, |_| Some(&source)));
            assert!(!admitted.include(Key(1, 2, &hashes), &bad, |_| Some(&source)));
            assert!(admitted.include(Key(u32::MAX, 1, &hashes), &source, |_| Some(&source)));
            let ordered = admitted.into_sources();
            assert_eq!(ordered.len(), count + 2);
            assert!(ordered.keys().map(|k| (k.0, k.1)).is_sorted());
        }
    }

    #[test]
    fn unique_plan_admission_searches_its_ordered_table_once_per_key() {
        #[derive(Clone, Copy)]
        struct Key<'a>(u32, &'a Cell<usize>);
        impl PartialEq for Key<'_> {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl Eq for Key<'_> {}
        impl PartialOrd for Key<'_> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Key<'_> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.1.set(self.1.get() + 1);
                self.0.cmp(&other.0)
            }
        }
        impl std::hash::Hash for Key<'_> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state)
            }
        }
        let source = Arc::new(7u32);
        let mut rows = Vec::new();
        for count in [4, 32, 256] {
            let actual = Cell::new(0);
            let reference = Cell::new(0);
            let mut admitted = PlanSources::default();
            let mut single_search = BTreeMap::new();
            for i in 0..count {
                assert!(admitted.include(Key(i, &actual), &source, |_| None));
                single_search.entry(Key(i, &reference)).or_insert(&source);
            }
            rows.push((count, actual.get(), reference.get()));
            assert_eq!(admitted.into_sources().len(), count as usize);
        }
        eprintln!("actual/source-table single-search Ord comparisons {rows:?}");
        assert!(
            rows.iter()
                .all(|(_, actual, reference)| actual <= reference),
            "excess ordered-table search remains {rows:?}"
        );
    }

    #[test]
    fn repeated_plan_source_admission_checks_residency_once_and_keeps_late_conflicts() {
        let sources = [Arc::new(7u32), Arc::new(7u32), Arc::new(9u32)];
        let lookups = Cell::new(0);
        let lookup = |_: &u32| {
            lookups.set(lookups.get() + 1);
            Some(&sources[0])
        };
        let mut admitted = PlanSources::default();
        for _ in 0..4096 {
            assert!(admitted.include(1, &sources[1], lookup));
        }
        assert_eq!(
            lookups.get(),
            1,
            "immutable resident table rechecked per draw"
        );
        assert!(!admitted.include(1, &sources[2], lookup));
        assert!(!admitted.include(1, &sources[2], lookup));
        assert!(admitted.include(1, &sources[1], lookup));
        // A new generation/key must not inherit admission of the same pointer.
        assert!(!admitted.include(2, &sources[1], |_| Some(&sources[2])));
        assert!(admitted.include(2, &sources[1], lookup));
        assert_eq!(lookups.get(), 2, "new generation skipped current residency");
        let retained = admitted.into_sources();
        assert_eq!(retained.len(), 2);
        drop(retained);
        let mut fresh = PlanSources::default();
        assert!(
            !fresh.include(1, &sources[1], |_| Some(&sources[2])),
            "proof escaped its prepare"
        );
        assert_eq!(Arc::strong_count(&sources[1]), 1);
    }

    #[test]
    fn repeated_detached_sources_compare_once_and_conflicts_still_fail() {
        struct Source<'a> {
            value: u32,
            comparisons: &'a Cell<usize>,
        }
        impl PartialEq for Source<'_> {
            fn eq(&self, other: &Self) -> bool {
                self.comparisons.set(self.comparisons.get() + 1);
                self.value == other.value
            }
        }
        let comparisons = Cell::new(0);
        let make = |value| {
            Arc::new(Source {
                value,
                comparisons: &comparisons,
            })
        };
        let sources = [make(1), make(1), make(1), make(2)];
        let mut scratch = SourceComparisons::default();
        for i in 0..4096 {
            assert!(scratch.matches(&sources[0], &sources[1 + i % 2]));
        }
        assert_eq!(
            comparisons.get(),
            2,
            "same immutable source bytes compared per instance"
        );
        assert!(!scratch.matches(&sources[0], &sources[3]));
        assert!(
            !scratch.matches(&sources[0], &sources[3]),
            "failed comparisons must not become proofs"
        );
        assert_eq!(comparisons.get(), 4);
        drop(scratch);
        let changed = make(2);
        assert!(
            !SourceComparisons::default().matches(&sources[0], &changed),
            "new call hid changed source"
        );
        assert_eq!(
            Arc::strong_count(&sources[0]),
            1,
            "scratch retained source ownership"
        );
    }

    fn check(
        required: &BTreeMap<u32, Arc<u32>>,
        resident: &BTreeMap<u32, Arc<u32>>,
        lookups: &Cell<usize>,
    ) -> bool {
        let ordered: Vec<_> = required.iter().map(|(k, v)| (*k, v.clone())).collect();
        sources_match(&ordered, resident.len(), resident.iter(), |key| {
            lookups.set(lookups.get() + 1);
            resident.get(key)
        })
    }

    #[test]
    fn dense_live_sources_avoid_per_mesh_searches_and_recheck_late_changes() {
        let required: BTreeMap<_, _> = (0..10000).map(|i| (i, Arc::new(i))).collect();
        let mut resident = required.clone();
        let lookups = Cell::new(0);
        assert!(check(&required, &resident, &lookups));
        assert_eq!(
            lookups.get(),
            0,
            "dense current mesh table searched once per resource"
        );
        resident.remove(&9997);
        assert!(!check(&required, &resident, &lookups));
        resident.insert(9997, Arc::new(9997));
        assert!(
            check(&required, &resident, &lookups),
            "equal detached source must remain legal"
        );
        resident.insert(9997, Arc::new(0));
        assert!(
            !check(&required, &resident, &lookups),
            "late source change was hidden"
        );
        resident.insert(9997, Arc::new(9997));
        resident.insert(20000, Arc::new(20000));
        assert!(check(&required, &resident, &lookups));
    }

    #[test]
    fn sparse_and_empty_closures_keep_bounded_search_and_missing_keys_fail() {
        let resident: BTreeMap<_, _> = (0..10000).map(|i| (i, Arc::new(i))).collect();
        let required = BTreeMap::from([(9999, Arc::new(9999))]);
        let lookups = Cell::new(0);
        assert!(check(&required, &resident, &lookups));
        assert_eq!(lookups.get(), 1);
        assert!(check(&BTreeMap::new(), &resident, &lookups));
        assert_eq!(lookups.get(), 1);
        assert!(!check(
            &BTreeMap::from([(10000, Arc::new(10000))]),
            &resident,
            &lookups
        ));
        assert!(!check(&required, &BTreeMap::new(), &lookups));
    }

    #[test]
    fn dense_walk_skips_unrelated_keys_without_accepting_a_hole() {
        let required: BTreeMap<_, _> = (0..128).map(|i| (i * 2, Arc::new(i * 2))).collect();
        let mut resident: BTreeMap<_, _> = (0..256).map(|i| (i, Arc::new(i))).collect();
        let lookups = Cell::new(0);
        assert!(check(&required, &resident, &lookups));
        assert_eq!(lookups.get(), 0);
        resident.remove(&126);
        assert!(!check(&required, &resident, &lookups));
    }
}
