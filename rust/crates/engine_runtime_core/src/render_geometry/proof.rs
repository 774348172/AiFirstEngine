//! Bounded success facts for immutable geometry inputs, never native admission.
use super::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::sync::{Arc, Weak};

/// A private success bound to the entire immutable allocation. Weak prevents
/// get_mut/address recycling; COW clones reset it. Moving a unique allocation
/// dissociates its Weak and can only fall back to strict validation.
#[derive(Debug)]
pub(super) struct BoundSuccess<T> {
    source: OnceLock<Weak<T>>,
    success: OnceLock<()>,
}
impl<T> Default for BoundSuccess<T> {
    fn default() -> Self {
        Self {
            source: OnceLock::new(),
            success: OnceLock::new(),
        }
    }
}
impl<T> Clone for BoundSuccess<T> {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl<T> BoundSuccess<T> {
    fn contains(&self, value: &Arc<T>) -> bool {
        self.success.get().is_some()
            && self
                .source
                .get()
                .is_some_and(|source| source.as_ptr() == Arc::as_ptr(value))
    }
    fn mark(&self, value: &Arc<T>) -> bool {
        let source = self.source.get_or_init(|| Arc::downgrade(value));
        if source.as_ptr() != Arc::as_ptr(value) {
            return false;
        }
        let _ = self.success.set(());
        true
    }
}

fn validate_bound<T: 'static>(
    bound: &BoundSuccess<T>,
    fallback: &'static std::thread::LocalKey<RefCell<Successes<T>>>,
    value: &Arc<T>,
    check: impl FnOnce() -> Result<(), ContractError>,
) -> Result<(), ContractError> {
    if bound.contains(value) {
        return Ok(());
    }
    if !fallback.with(|cache| cache.borrow().contains(value)) {
        check()?;
    }
    // Normal packets carry the fact themselves. The old bounded owner remains
    // only for a moved, already-bound allocation; no duplicate hot-path registry.
    if !bound.mark(value) {
        fallback.with(|cache| cache.borrow_mut().insert(value));
    }
    Ok(())
}

const CAPACITY: usize = 65_536;

struct Successes<T> {
    entries: HashMap<usize, Weak<T>>,
    order: VecDeque<usize>,
    capacity: usize,
    pending_insertions: usize,
}

impl<T> Default for Successes<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity: CAPACITY,
            pending_insertions: 0,
        }
    }
}

impl<T> Successes<T> {
    fn prune(&mut self) {
        if self.pending_insertions < 128 {
            return;
        }
        self.entries.retain(|_, source| source.strong_count() != 0);
        self.order.retain(|key| self.entries.contains_key(key));
        self.pending_insertions = 0;
    }

    fn contains(&self, value: &Arc<T>) -> bool {
        self.entries.contains_key(&(Arc::as_ptr(value) as usize))
    }

    fn insert(&mut self, value: &Arc<T>) {
        let key = Arc::as_ptr(value) as usize;
        if self.capacity == 0 || self.entries.contains_key(&key) {
            return;
        }
        // Metadata only: evict one oldest fact, preserving current successful
        // batches through downstream consumers. No full-map overflow clear or
        // expiry sweep, and no strong payload lease. Weak retains the address
        // until eviction; hits do not append duplicate order entries.
        if self.entries.len() == self.capacity {
            let oldest = self
                .order
                .pop_front()
                .expect("success order matches entries");
            self.entries.remove(&oldest);
        }
        self.entries.insert(key, Arc::downgrade(value));
        self.order.push_back(key);
        self.pending_insertions = self.pending_insertions.saturating_add(1);
    }

    #[cfg(any(feature = "real-wgpu", test))]
    fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_deque(&self.order, "renderer", &["decodedCache"]);
    }
}

thread_local! {
    static DRAWS: RefCell<Successes<DrawGeometry>> = RefCell::default();
    static SELECTIONS: RefCell<Successes<GeometryBatch>> = RefCell::default();
    static INSTANCES: RefCell<Successes<DrawInstance>> = RefCell::default();
    static STATIC_INSTANCES: RefCell<Successes<DrawInstance>> = RefCell::default();
}

#[cfg(feature = "real-wgpu")]
pub(super) fn observe_memory(sample: &mut crate::resource_memory::ResourceMemorySample) {
    DRAWS.with(|c| c.borrow().observe_memory(sample));
    SELECTIONS.with(|c| c.borrow().observe_memory(sample));
    INSTANCES.with(|c| c.borrow().observe_memory(sample));
    STATIC_INSTANCES.with(|c| c.borrow().observe_memory(sample));
    sample.partial(
        "cpu.owner.renderer",
        "currentThreadGeometryProofOrderBacking;hashBucketsWeakControlBlocksOtherThreadsNotInstrumented",
    );
}

fn validate<T: 'static>(
    cache: &'static std::thread::LocalKey<RefCell<Successes<T>>>,
    value: &Arc<T>,
    check: impl FnOnce() -> Result<(), ContractError>,
) -> Result<(), ContractError> {
    // A retained Weak prevents allocation address recycling, blocks get_mut,
    // and make_mut dissociates the identity. The callback runs without a borrow:
    // nested validation is safe and errors never become successful facts.
    if cache.with(|cache| cache.borrow().contains(value)) {
        return Ok(());
    }
    check()?;
    cache.with(|cache| cache.borrow_mut().insert(value));
    Ok(())
}

pub(super) fn draw<'a>(
    draw: &'a Arc<DrawGeometry>,
    context: &mut DrawViewValidation<'a>,
) -> Result<(), ContractError> {
    validate_bound(&draw.success, &DRAWS, draw, || {
        let static_proven = draw.static_instance_proven()
            || STATIC_INSTANCES.with(|cache| cache.borrow().contains(&draw.instance));
        draw.validate_with_static_facts(context, static_proven)
    })
}

// The caller owns a still-unpublished CompactView: its full selection succeeded
// before FinalViewState changed only illumination. This is not a reusable flag
// on a mutable descriptor, nor permission for any public/wire caller to skip math.
pub(super) fn draw_after_selection<'a>(
    draw: &'a Arc<DrawGeometry>,
    context: &mut DrawViewValidation<'a>,
) -> Result<(), ContractError> {
    validate_bound(&draw.success, &DRAWS, draw, || {
        let static_proven = draw.static_instance_proven()
            || STATIC_INSTANCES.with(|cache| cache.borrow().contains(&draw.instance));
        draw.validate_with_selection_facts(context, static_proven, true)
    })
}

// Called only after every row of the immutable prepared table passes the full
// static contract. This metadata lives with the original bounded proof owner.
pub(super) fn prepared_instances(table: &Arc<Vec<Arc<DrawInstance>>>) {
    // Reclaim expired allocation headers at the existing table boundary. Never
    // sweep per draw, and stable live tables add no further collection work.
    DRAWS.with(|cache| cache.borrow_mut().prune());
    SELECTIONS.with(|cache| cache.borrow_mut().prune());
    INSTANCES.with(|cache| cache.borrow_mut().prune());
    STATIC_INSTANCES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.prune();
        for row in table.iter() {
            cache.insert(row);
        }
    });
}

pub(super) fn selection_with_view<'a>(
    batch: &'a Arc<GeometryBatch>,
    context: &mut DrawViewValidation<'a>,
) -> Result<(), ContractError> {
    validate_bound(&batch.success, &SELECTIONS, batch, || {
        batch.validate_selection_with_view(context)
    })
}

#[cfg(test)]
pub(super) fn prepared_selection<'a>(
    batch: &'a Arc<GeometryBatch>,
    context: &mut DrawViewValidation<'a>,
    prepared: &commands::PreparedPhase,
) -> Result<(), ContractError> {
    validate_bound(&batch.success, &SELECTIONS, batch, || {
        batch.validate_prepared_selection(context, prepared)
    })
}

#[cfg(test)]
pub(super) fn selection(batch: &Arc<GeometryBatch>) -> Result<(), ContractError> {
    selection_with_view(batch, &mut DrawViewValidation::default())
}

// Private CompactView publication only: all rows passed selection validation,
// then FinalViewState changed only environment/lights/shadows before final draw
// validation. No mutable batch or partial selection is ever certified.
pub(super) fn publish_complete_selection(batch: &Arc<GeometryBatch>) {
    if !batch.success.mark(batch) {
        SELECTIONS.with(|cache| cache.borrow_mut().insert(batch));
    }
}

pub(super) fn refresh_selection_draw(batch: &mut Arc<GeometryBatch>, draw: &Arc<DrawGeometry>) {
    if Arc::ptr_eq(&batch.draw, draw) {
        return;
    }
    // This operation cannot modify the schema, table or visible sequence. The
    // selection validator consumes only these head dependencies, not lighting,
    // environment, target or shadows. A changed head still needs its own full
    // draw validation; only an already successful complete selection transfers.
    #[cfg(test)]
    HEAD_REWRITES.with(|n| n.set(n.get() + 1));
    let unchanged = Arc::ptr_eq(&batch.draw.instance, &draw.instance)
        && Arc::ptr_eq(&batch.draw.view, &draw.view)
        && batch.draw.depth == draw.depth
        && batch.draw.camera_depth == draw.camera_depth;
    let successful = unchanged
        && (batch.success.contains(batch)
            || SELECTIONS.with(|cache| cache.borrow().contains(batch)));
    let updated = Arc::make_mut(batch);
    updated.draw = draw.clone();
    updated.success = BoundSuccess::default();
    if successful && !batch.success.mark(batch) {
        SELECTIONS.with(|cache| cache.borrow_mut().insert(batch));
    }
}

pub(super) fn instance(instance: &Arc<DrawInstance>) -> Result<(), ContractError> {
    #[cfg(test)]
    INSTANCE_PROOF_LOOKUPS.with(|n| n.set(n.get() + 1));
    validate(&INSTANCES, instance, || instance.validate_values())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "spine")]
    #[ignore = "bounded original single package proof-owner memory diagnosis"]
    fn spine374_f_expired_proofs_at_the_original_host_loop() {
        use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
        use crate::frame_loop::RuntimeFrameContext;
        let path = std::path::PathBuf::from(std::env::var_os("SPINE374_MEMORY_PACKAGE").unwrap());
        let package = crate::runtime_package::load_runtime_package(&path)
            .value
            .unwrap();
        let mut world = crate::scene_loader::load_scene_into_world(&package.active_scene)
            .value
            .unwrap();
        let mut loader =
            crate::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
        let mut host = EngineHostLoop::new("main");
        host.set_external_rhi_enabled(true);
        let mut samples = Vec::new();
        let started = crate::platform_time::Instant::now();
        for tick in 1..=1200 {
            let output = host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_unscaled_delta_time(1. / 60.)
                    .with_fixed_step_count(1)
                    .with_deferred_render_submission(),
                &mut world,
                RuntimeFrameContext::new(&package, &mut loader),
            );
            assert!(output.runtime_advanced && output.host_control.is_none());
            let render = host.render_thread_for_target(
                crate::runtime_renderer::RenderTarget::surface("proof-owner", 1280, 720),
            );
            assert_eq!(render.report.rhi_status, "ok");
            assert_eq!(
                host.retained_geometry_snapshot()
                    .unwrap()
                    .prepared_geometries()
                    .count(),
                1
            );
            drop((output, render));
            if tick % 200 == 0 {
                let count = |cache: &Successes<DrawInstance>| {
                    (
                        cache.entries.len(),
                        cache
                            .entries
                            .values()
                            .filter(|v| v.strong_count() != 0)
                            .count(),
                    )
                };
                samples.push(serde_json::json!({"tick":tick,
                    "instances":INSTANCES.with(|c| count(&c.borrow())),
                    "staticInstances":STATIC_INSTANCES.with(|c| count(&c.borrow()))}));
            }
            assert!(started.elapsed().as_secs() < 30);
        }
        let root = std::path::PathBuf::from(std::env::var_os("SPINE374_EVIDENCE_ROOT").unwrap());
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("proof-owner-counts.json"),
            serde_json::to_vec_pretty(&samples).unwrap(),
        )
        .unwrap();
        eprintln!("proof owner counts: {samples:#?}");
        for cache in [&INSTANCES, &STATIC_INSTANCES] {
            cache.with(|c| {
                let c = c.borrow();
                let dead = c.entries.values().filter(|v| v.strong_count() == 0).count();
                assert!(
                    dead <= 128,
                    "expired proof backing grew during the original HostLoop: {dead}"
                );
            });
        }
    }

    #[test]
    fn proof_capacity_keeps_current_batch_successes_across_boundary_at_multiple_scales() {
        thread_local! {static CACHE: RefCell<Successes<GeometryBatch>> = RefCell::new(Successes {capacity:64,..Successes::default()});}
        let mut repeated = Vec::new();
        for rows in [4, 32, 256] {
            CACHE.with(|c| {
                *c.borrow_mut() = Successes {
                    capacity: 64,
                    ..Successes::default()
                }
            });
            let frame = commands::tests::batch_test_frame(true, MaterialBlend::Opaque, rows, 1);
            let source = &frame.views[0].batches.as_ref().unwrap()[0];
            let check = |b: &Arc<GeometryBatch>| {
                validate(&CACHE, b, || {
                    b.validate_selection_with_view(&mut DrawViewValidation::default())
                })
            };
            for _ in 0..63 {
                let expired = Arc::new(source.as_ref().clone());
                check(&expired).unwrap();
            }
            let current: [Arc<GeometryBatch>; 2] =
                std::array::from_fn(|_| Arc::new(source.as_ref().clone()));
            for b in &current {
                check(b).unwrap();
            }
            CACHE.with(|c| {
                for _ in 0..64 {
                    c.borrow_mut().insert(&current[0]);
                }
            });
            commands::STATIC_COMPATIBILITY_CHECKS.with(|n| n.set(0));
            for b in &current {
                check(b).unwrap();
            }
            repeated.push((
                rows,
                commands::STATIC_COMPATIBILITY_CHECKS.with(|n| n.get()),
            ));
            assert!(CACHE.with(|c| c.borrow().entries.len()) <= 64);
            assert!(CACHE.with(|c| {
                let c = c.borrow();
                c.order.len() == c.entries.len()
            }));
            let weak = Arc::downgrade(&current[0]);
            assert_eq!(Arc::strong_count(&current[0]), 1);
            drop(current);
            assert!(weak.upgrade().is_none(), "success metadata pinned a batch");
        }
        eprintln!("capacity boundary repeated actual compatibility rows {repeated:?}");
        assert!(
            repeated.iter().all(|(_, visits)| *visits == 0),
            "full current batch work repeated: {repeated:?}"
        );
    }

    #[test]
    fn proof_order_observes_actual_storage_and_zero_budget_retains_nothing() {
        let value = Arc::new(vec![1u8]);
        let mut disabled = Successes {
            capacity: 0,
            ..Successes::default()
        };
        disabled.insert(&value);
        assert!(disabled.entries.is_empty() && disabled.order.is_empty());
        let mut cache = Successes {
            capacity: 2,
            ..Successes::default()
        };
        for _ in 0..128 {
            cache.insert(&value);
        }
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.order.len(), 1);
        assert_eq!(Arc::strong_count(&value), 1);
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        cache.observe_memory(&mut sample);
        assert_eq!(sample.allocations.len(), 1);
        let allocation = &sample.allocations[0];
        assert_eq!(allocation.owner, "renderer");
        assert!(allocation.roles.contains("decodedCache"));
        assert_eq!(
            allocation.bytes,
            (cache.order.capacity() * std::mem::size_of::<usize>()) as u64
        );
        let weak = Arc::downgrade(&value);
        drop(value);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn selection_refresh_misses_changed_dependencies_and_never_transfers_failure() {
        let frame = commands::tests::batch_test_frame(true, MaterialBlend::Opaque, 2, 1);
        let original = frame.views[0].batches.as_ref().unwrap()[0].clone();
        selection(&original).unwrap();
        let mut unchanged = original.clone();
        let old_identity = Arc::as_ptr(&unchanged);
        refresh_selection_draw(&mut unchanged, &original.draw);
        assert_eq!(Arc::as_ptr(&unchanged), old_identity);
        for changed in 0..4 {
            let mut batch = original.clone();
            let mut draw = original.draw.as_ref().clone();
            match changed {
                0 => draw.view = Arc::new(draw.view.as_ref().clone()),
                1 => Arc::make_mut(&mut draw.instance).world_transform[12] = 0.25.into(),
                2 => draw.depth = MaterialDepth::TestOnly,
                _ => draw.camera_depth = 99f32.into(),
            }
            refresh_selection_draw(&mut batch, &Arc::new(draw));
            assert!(
                !SELECTIONS.with(|c| c.borrow().contains(&batch)),
                "changed dependency {changed} inherited a selection success"
            );
        }
        let mut failed = original.clone();
        Arc::make_mut(&mut failed)
            .visible
            .last_mut()
            .unwrap()
            .instance_index = u32::MAX;
        assert!(selection(&failed).is_err());
        let mut draw = failed.draw.as_ref().clone();
        draw.target = format!("{}-new", draw.target).into();
        refresh_selection_draw(&mut failed, &Arc::new(draw));
        assert!(!SELECTIONS.with(|c| c.borrow().contains(&failed)));
        assert!(selection(&failed).is_err());
        drop(unchanged);
        assert_eq!(
            Arc::strong_count(&original),
            2,
            "proof retained batch payload"
        );
    }

    #[test]
    fn weak_success_facts_recheck_cow_expiry_overflow_errors_and_reentry_without_pinning() {
        thread_local! { static CACHE: RefCell<Successes<Vec<u8>>> = RefCell::new(Successes { capacity: 2, ..Successes::default() }); }
        let mut value = Arc::new(vec![1]);
        let mut checks = 0;
        validate(&CACHE, &value, || {
            checks += 1;
            Ok(())
        })
        .unwrap();
        validate(&CACHE, &value, || panic!("success rescanned")).unwrap();
        assert_eq!(Arc::strong_count(&value), 1, "proof pins payload");
        assert!(Arc::get_mut(&mut value).is_none());
        let old = Arc::downgrade(&value);
        Arc::make_mut(&mut value)[0] = 2;
        assert!(old.upgrade().is_none(), "COW did not dissociate proof");
        validate(&CACHE, &value, || {
            checks += 1;
            Ok(())
        })
        .unwrap();
        let other = Arc::new(vec![3]);
        validate(&CACHE, &other, || {
            validate(&CACHE, &value, || panic!("nested miss"))
        })
        .unwrap();
        assert!(CACHE.with(|c| c.borrow().entries.len()) <= 2);
        validate(&CACHE, &value, || {
            checks += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(checks, 2, "capacity discarded a current successful fact");
        let third = Arc::new(vec![4]);
        validate(&CACHE, &third, || Ok(())).unwrap();
        validate(&CACHE, &value, || {
            checks += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            checks, 3,
            "an actually evicted fact must miss rather than skip validation"
        );
        let bad = Arc::new(vec![0]);
        for _ in 0..2 {
            assert!(validate(&CACHE, &bad, || Err(invalid("test.failed"))).is_err());
        }
        assert!(!CACHE.with(|c| c.borrow().contains(&bad)));
        let weak = Arc::downgrade(&value);
        drop(value);
        assert!(weak.upgrade().is_none());
        for _ in 0..128 {
            let new = Arc::new(vec![4]);
            let mut visited = false;
            validate(&CACHE, &new, || {
                visited = true;
                Ok(())
            })
            .unwrap();
            assert!(visited, "expired input admitted a replacement allocation");
            assert!(CACHE.with(|c| {
                let c = c.borrow();
                c.entries.len() <= 2 && c.order.len() == c.entries.len()
            }));
        }
    }
}

#[cfg(test)]
thread_local! {pub(super) static HEAD_REWRITES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}

#[cfg(test)]
thread_local! {pub(super) static INSTANCE_PROOF_LOOKUPS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}

/// Complete immutable CPU content, shared across consumers/tasks. This is not
/// a view pairing, current resource, upload or completion fact. The single
/// initializer serializes a cold content check; an immutable failure remains an
/// error. Clone/serde reset metadata and a Weak-dissociated move is checked anew.
#[derive(Debug)]
pub(crate) struct ContentValidation<T> {
    source: OnceLock<Weak<T>>,
    result: OnceLock<Result<(), ContractError>>,
}
impl<T> Default for ContentValidation<T> {
    fn default() -> Self {
        Self {
            source: OnceLock::new(),
            result: OnceLock::new(),
        }
    }
}
impl<T> Clone for ContentValidation<T> {
    fn clone(&self) -> Self {
        Self::default()
    }
}
// Validation metadata has no part in semantic value equality, hash or wire.
impl<T> PartialEq for ContentValidation<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl<T> Eq for ContentValidation<T> {}
impl<T> std::hash::Hash for ContentValidation<T> {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}
impl<T> ContentValidation<T> {
    pub(crate) fn validate(
        &self,
        value: &Arc<T>,
        check: impl FnOnce() -> Result<(), ContractError>,
    ) -> Result<(), ContractError> {
        let source = self.source.get_or_init(|| Arc::downgrade(value));
        if source.as_ptr() != Arc::as_ptr(value) {
            return check();
        }
        *self.result.get_or_init(check)
    }
}
