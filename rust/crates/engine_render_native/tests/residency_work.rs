// Exercise the production CPU dependency owner directly, including its scalar
// conflict and sparse/dense equivalence tests. No device or Player is required.
#[path = "../src/wgpu_backend/real/geometry/residency.rs"]
mod residency;

#[test]
fn dense_registry_checks_complete_keys_and_falls_back_for_sparse_or_colliding_indices() {
    use std::collections::BTreeMap;
    // Same index with a different generation must never inherit membership.
    let registry: BTreeMap<_, _> = (0..4096u32).map(|i| ((i, 1u32), i + 10)).collect();
    let dense = residency::RegistryLookup::new(&registry, 4096, |key| key.0 as usize);
    assert_eq!(dense.dense_len(), 4096);
    for i in 0..4096 {
        assert_eq!(dense.get(&(i, 1)), registry.get(&(i, 1)));
        assert_eq!(dense.get(&(i, 2)), None);
    }
    assert_eq!(dense.get(&(u32::MAX, 1)), None);
    let sparse = residency::RegistryLookup::new(&registry, 1, |key| key.0 as usize);
    assert_eq!(sparse.dense_len(), 0);
    assert_eq!(sparse.get(&(4000, 1)), Some(&4010));
    let high = BTreeMap::from([((0, 1), 1), ((u32::MAX, 1), 2)]);
    let high = residency::RegistryLookup::new(&high, 2, |key| key.0 as usize);
    assert_eq!(high.dense_len(), 0);
    assert_eq!(high.get(&(u32::MAX, 1)), Some(&2));
    let collision = BTreeMap::from([((0, 1), 1), ((0, 2), 2)]);
    let collision = residency::RegistryLookup::new(&collision, 2, |key| key.0 as usize);
    assert_eq!(collision.dense_len(), 0);
    assert_eq!(collision.get(&(0, 1)), Some(&1));
    assert_eq!(collision.get(&(0, 2)), Some(&2));
}

#[test]
fn distinct_plan_sources_do_not_search_an_ordered_scratch_tree_per_draw() {
    use std::{cell::Cell, sync::Arc};
    #[derive(Clone, Copy)]
    struct Key<'a>(u32, &'a Cell<usize>);
    impl PartialEq for Key<'_> {
        fn eq(&self, b: &Self) -> bool {
            self.0 == b.0
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
            self.1.set(self.1.get() + 1);
            self.0.cmp(&b.0)
        }
    }
    impl std::hash::Hash for Key<'_> {
        fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
            self.0.hash(h);
        }
    }
    for count in [4, 32, 256, 4096] {
        let comparisons = Cell::new(0);
        let source = Arc::new(7u32);
        let mut current = residency::PlanSources::default();
        for i in 0..count {
            assert!(current.include(Key(i, &comparisons), &source, |_| Some(&source)));
        }
        let admission_comparisons = comparisons.get();
        let ordered = current.into_sources();
        assert_eq!(
            ordered.keys().map(|k| k.0).collect::<Vec<_>>(),
            (0..count).collect::<Vec<_>>()
        );
        assert_eq!(
            admission_comparisons, 0,
            "{count} distinct sources searched an ordered scratch tree"
        );
    }
}
