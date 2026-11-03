//! Bounded CPU-only preparation within one frozen frame. No GPU owner, queue
//! across frames, detached work, or mutable World crosses this scope.
use super::*;

pub(super) struct ViewJob {
    pub(super) target: String,
    pub(super) extent: [u32; 2],
    pub(super) view: RenderView,
    pub(super) pose: CameraPose,
    pub(super) empty: bool,
}

pub(super) struct PreparedBatch<T> {
    pub(super) values: Vec<T>,
    #[cfg(test)]
    pub(super) threads: Vec<std::thread::ThreadId>,
}

// Four cascades and eight spots are the existing per-camera contract. Fixed
// limits bound scratch and fan-out independently of hardware or input size.
const MAX_JOBS: usize = 16; // twelve shadow views plus four ordered color partitions
const MAX_WORKERS: usize = 4;

#[cfg(test)]
mod work_claim_tests {
    use super::*;
    #[test]
    fn free_worker_claims_later_job_while_first_lane_is_busy_and_keeps_index_order() {
        use std::sync::{Condvar, Mutex};
        let gate = (Mutex::new(false), Condvar::new());
        let completed = std::sync::atomic::AtomicUsize::new(0);
        let output = run_bounded(8, 4, &|index| {
            let ready = if index == 0 {
                let (lock, changed) = &gate;
                let state = lock.lock().unwrap();
                let (state, _) = changed
                    .wait_timeout_while(state, std::time::Duration::from_secs(2), |ready| !*ready)
                    .unwrap();
                *state
            } else {
                if index == 4 {
                    *gate.0.lock().unwrap() = true;
                    gate.1.notify_all();
                }
                true
            };
            completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok((index, ready))
        })
        .unwrap();
        assert_eq!(completed.load(std::sync::atomic::Ordering::Relaxed), 8);
        assert_eq!(
            output.values.iter().map(|v| v.0).collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        assert!(
            output.values[0].1,
            "free preparation workers could not claim job 4 while fixed lane 0 was busy"
        );
    }
}

#[cfg(test)]
fn task_thread() -> std::thread::ThreadId {
    std::thread::current().id()
}
#[cfg(all(not(test), not(target_arch = "wasm32")))]
fn task_thread() {}
#[cfg(test)]
thread_local! { static LAST_THREADS: std::cell::RefCell<Vec<std::thread::ThreadId>> = const { std::cell::RefCell::new(Vec::new()) }; }
#[cfg(test)]
pub(super) fn take_test_threads() -> Vec<std::thread::ThreadId> {
    LAST_THREADS.with(|threads| std::mem::take(&mut *threads.borrow_mut()))
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run_bounded<T: Send>(
    count: usize,
    workers: usize,
    task: &(impl Fn(usize) -> Result<T, ContractError> + Sync),
) -> Result<PreparedBatch<T>, ContractError> {
    if count > MAX_JOBS || workers == 0 || workers > MAX_WORKERS {
        return Err(invalid("geometry.prepare_task_capacity"));
    }
    let workers = workers.min(count.max(1));
    // Each lane keeps its first item, including a failed spawn's inline item.
    // The remaining independent jobs can be claimed by any free worker. This
    // cursor is frame-local and bounded by MAX_JOBS; it is not a task queue
    // across frames. Relaxed ordering assigns disjoint indices; the result
    // mutex and the completed joins publish all values before consumption.
    let next = std::sync::atomic::AtomicUsize::new(workers);
    let ordered = std::sync::Mutex::new((0..count).map(|_| None).collect::<Vec<_>>());
    let lane = |first: usize| {
        let mut index = first;
        while index < count {
            let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task(index)))
                .unwrap_or_else(|_| Err(invalid("geometry.prepare_task_panicked")));
            ordered
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)[index] =
                Some((value, task_thread()));
            index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    };
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        let mut inline = vec![0];
        for worker in 1..workers {
            match std::thread::Builder::new()
                .name("geometry-prepare".into())
                .spawn_scoped(scope, {
                    let lane = &lane;
                    move || lane(worker)
                }) {
                Ok(handle) => handles.push(handle),
                // Resource exhaustion changes execution speed, never frame
                // semantics or the original output order.
                Err(_) => inline.push(worker),
            }
        }
        for worker in inline {
            lane(worker);
        }
        // Join every child before propagating any error. No failed or obsolete
        // packet can leave a child preparing a later frame in the background.
        for handle in handles {
            // Task panics already became indexed errors. An unexpected worker
            // unwind can leave a missing slot; handle it only after every join,
            // without overwriting completed work claimed by another lane.
            let _ = handle.join();
        }
    });
    let mut values = Vec::with_capacity(count);
    #[cfg(test)]
    let mut threads = Vec::with_capacity(count);
    for result in ordered
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        let (value, _thread) = result.unwrap_or_else(|| {
            (
                Err(invalid("geometry.prepare_task_panicked")),
                task_thread(),
            )
        });
        values.push(value?);
        #[cfg(test)]
        threads.push(_thread);
    }
    Ok(PreparedBatch {
        values,
        #[cfg(test)]
        threads,
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn run_bounded<T>(
    count: usize,
    workers: usize,
    task: &impl Fn(usize) -> Result<T, ContractError>,
) -> Result<PreparedBatch<T>, ContractError> {
    if count > MAX_JOBS || workers == 0 || workers > MAX_WORKERS {
        return Err(invalid("geometry.prepare_task_capacity"));
    }
    Ok(PreparedBatch {
        values: (0..count).map(task).collect::<Result<_, _>>()?,
        #[cfg(test)]
        threads: Vec::new(),
    })
}

/// Reuse the frozen-frame preparation group for immutable native registry
/// reads. Indexed results are joined before the native owner may commit.
#[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub fn read_native_partitions<T: Send>(
    count: usize,
    task: &(impl Fn(usize) -> Result<T, ContractError> + Sync),
) -> Result<Vec<T>, ContractError> {
    run_bounded(count, MAX_WORKERS, task).map(|batch| batch.values)
}

/// Browser GPU handles remain on the browser thread; preserve result ordering.
#[cfg(all(feature = "real-wgpu", target_arch = "wasm32"))]
#[doc(hidden)]
pub fn read_native_partitions<T>(
    count: usize,
    task: &impl Fn(usize) -> Result<T, ContractError>,
) -> Result<Vec<T>, ContractError> {
    run_bounded(count, MAX_WORKERS, task).map(|batch| batch.values)
}

/// Two bounded preparation branches. The inline branch may borrow the native
/// owner exclusively; only the immutable validation inputs cross threads.
/// Both finish before errors are returned (validation retains precedence).
/// No GPU mutation is allowed in either branch; callers commit after success.
#[cfg(all(feature = "real-wgpu", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub fn join_validation<T: Send, U>(
    enabled: bool,
    validation: impl FnOnce() -> Result<T, String> + Send,
    preparation: impl FnOnce() -> Result<U, String>,
) -> Result<(T, U), String> {
    if !enabled {
        let validated = validation()?;
        return preparation().map(|prepared| (validated, prepared));
    }
    // Keep closure ownership outside spawn_scoped, so thread exhaustion can
    // execute the same validation inline instead of dropping required work.
    let pending = std::sync::Mutex::new(Some(validation));
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("geometry-validation".into())
            .spawn_scoped(scope, || {
                pending
                    .lock()
                    .expect("validation closure poisoned")
                    .take()
                    .unwrap()()
            });
        match worker {
            Ok(worker) => {
                let prepared = preparation();
                let validated = worker
                    .join()
                    .unwrap_or_else(|_| Err("geometry.prepare_task_panicked".into()));
                Ok((validated?, prepared?))
            }
            Err(_) => {
                let validated = pending
                    .lock()
                    .expect("validation closure poisoned")
                    .take()
                    .unwrap()()?;
                preparation().map(|prepared| (validated, prepared))
            }
        }
    })
}

#[cfg(all(feature = "real-wgpu", target_arch = "wasm32"))]
#[doc(hidden)]
pub fn join_validation<T, U>(
    _enabled: bool,
    validation: impl FnOnce() -> Result<T, String>,
    preparation: impl FnOnce() -> Result<U, String>,
) -> Result<(T, U), String> {
    let validated = validation()?;
    preparation().map(|prepared| (validated, prepared))
}

#[cfg(test)]
thread_local! {static COLOR_THREADS: std::cell::RefCell<Vec<std::thread::ThreadId>> = const {std::cell::RefCell::new(Vec::new())};}
#[cfg(test)]
thread_local! {pub(super) static FINISH_THREADS: std::cell::RefCell<Vec<std::thread::ThreadId>> = const {std::cell::RefCell::new(Vec::new())};}
#[cfg(test)]
pub(super) fn take_finish_threads() -> Vec<std::thread::ThreadId> {
    FINISH_THREADS.with(|v| std::mem::take(&mut *v.borrow_mut()))
}
#[cfg(test)]
pub(super) fn take_color_threads() -> Vec<std::thread::ThreadId> {
    COLOR_THREADS.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

// Use the existing frozen inputs and bounded group for the final color work
// even when no shadow jobs exist. Partitions end at original batch boundaries.
pub(super) fn finish_color(
    compact: commands::CompactView,
    inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
    state: &FinalViewState,
) -> Result<commands::ColorOutput, ContractError> {
    #[cfg(test)]
    FINISH_THREADS.with(|v| *v.borrow_mut() = vec![std::thread::current().id()]);
    let Some(ordered) = compact.order(inputs)? else {
        return Ok(commands::ColorOutput::empty(false));
    };
    let direct = inputs.prepared.is_some() && ordered.all_singletons();
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_WORKERS);
    if inputs.instances.len() >= 4096 && workers > 1 {
        if let Some(frozen) = inputs.for_tasks() {
            let parts = ordered.partitions(workers);
            if parts.len() > 1 {
                let result = run_bounded(parts.len(), workers, &|index| {
                    parts[index].finish_color_unpublished(&mut frozen.fork(), state, direct)
                })?;
                #[cfg(test)]
                FINISH_THREADS.with(|v| *v.borrow_mut() = result.threads);
                let mut output = commands::ColorOutput::empty(direct);
                for part in result.values {
                    output.append(part)?;
                }
                return Ok(output);
            }
            return parts
                .into_iter()
                .next()
                .expect("one color partition")
                .finish_color_unpublished(inputs, state, direct);
        }
    }
    ordered.finish_color_unpublished(inputs, state, direct)
}

pub(super) fn select_color(
    target: &str,
    extent: [u32; 2],
    input: &GeometryViewInput<'_>,
    inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
) -> Result<PendingGeometryView, ContractError> {
    let count = inputs.instances.len();
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_WORKERS);
    if count >= 4096 && workers > 1 {
        if let Some(frozen) = inputs.for_tasks() {
            let parts = run_bounded(workers, workers, &|index| {
                GeometryFrame::select_view_range(
                    target,
                    extent,
                    input,
                    &mut frozen.fork(),
                    DiagnosticLevel::Off,
                    (count * index / workers)..(count * (index + 1) / workers),
                )
                .map_err(|e| e.error)
            })?;
            #[cfg(test)]
            COLOR_THREADS.with(|v| *v.borrow_mut() = parts.threads);
            let mut parts = parts.values.into_iter();
            let mut merged = parts.next().expect("nonempty color tasks");
            for mut part in parts {
                merged
                    .compact
                    .as_mut()
                    .expect("frozen compact selection")
                    .append_selection(part.compact.take().expect("frozen compact selection"))?;
            }
            return Ok(merged);
        }
    }
    #[cfg(test)]
    COLOR_THREADS.with(|v| *v.borrow_mut() = vec![std::thread::current().id()]);
    GeometryFrame::select_view(target, extent, input, inputs, DiagnosticLevel::Off)
        .map_err(|e| e.error)
}

fn build_job(
    inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
    job: &ViewJob,
) -> Result<GeometryFrame, ContractError> {
    let views = [GeometryViewInput {
        view: &job.view,
        pose: job.pose,
    }];
    if job.empty {
        GeometryFrame::build_retained(
            &job.target,
            job.extent,
            &views,
            &[],
            inputs.manager,
            DiagnosticLevel::Off,
            inputs.native_lease,
        )
    } else {
        inputs.build(&job.target, job.extent, &views, DiagnosticLevel::Off)
    }
    .map_err(|e| e.error)
}

pub(super) fn build_jobs(
    inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
    jobs: &[ViewJob],
) -> Result<Vec<GeometryFrame>, ContractError> {
    if jobs.len() > 12 {
        return Err(invalid("geometry.prepare_task_capacity"));
    }
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_WORKERS);
    if jobs.len() > 1 && inputs.instances.len() >= 512 && workers > 1 {
        if let Some(frozen) = inputs.for_tasks() {
            let batch = build_frozen(frozen, jobs, workers)?;
            #[cfg(test)]
            LAST_THREADS.with(|threads| *threads.borrow_mut() = batch.threads);
            return Ok(batch.values);
        }
    }
    #[cfg(test)]
    LAST_THREADS
        .with(|threads| *threads.borrow_mut() = vec![std::thread::current().id(); jobs.len()]);
    jobs.iter().map(|job| build_job(inputs, job)).collect()
}

/// The final color phase and independent shadow preparations share one
/// bounded group. Main error precedes shadows; output order remains shadows
/// then color. No child is detached and the prerequisite group has joined.
pub(super) fn build_lit_jobs(
    inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
    jobs: &[ViewJob],
    pending: PendingGeometryView,
    state: FinalViewState,
) -> Result<Vec<GeometryViewFrame>, ContractError> {
    if jobs.len() > 12 {
        return Err(invalid("geometry.prepare_task_capacity"));
    }
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_WORKERS);
    let (main, shadows) = if jobs.len() > 1 && inputs.instances.len() >= 512 && workers > 1 {
        if let Some(frozen) = inputs.for_tasks() {
            let PendingGeometryView { mut frame, compact } = pending;
            let ordered = compact
                .expect("frozen compact color selection")
                .order(&mut frozen.fork())?;
            let direct = ordered
                .as_ref()
                .is_some_and(|ordered| ordered.all_singletons());
            let parts = ordered
                .map(|ordered| ordered.partitions(MAX_WORKERS))
                .unwrap_or_default();
            let color_count = parts.len();
            enum LitPart {
                Color(commands::ColorOutput),
                Shadows(Vec<GeometryViewFrame>),
            }
            let batch = run_bounded(jobs.len() + color_count, workers, &|index| {
                let mut fork = frozen.fork();
                if index < color_count {
                    parts[index]
                        .finish_color_unpublished(&mut fork, &state, direct)
                        .map(LitPart::Color)
                } else {
                    build_job(&mut fork, &jobs[index - color_count])
                        .map(|v| LitPart::Shadows(v.views))
                }
            })?;
            #[cfg(test)]
            LAST_THREADS.with(|threads| *threads.borrow_mut() = batch.threads);
            let mut batches = commands::ColorOutput::empty(direct);
            let mut shadows = Vec::new();
            for part in batch.values {
                match part {
                    LitPart::Color(values) => batches.append(values)?,
                    LitPart::Shadows(values) => shadows.extend(values),
                }
            }
            commands::publish_color(&mut frame, batches, true);
            (frame, shadows)
        } else {
            let main = pending.finish(inputs, &state).map_err(|e| e.error)?;
            (
                main,
                jobs.iter()
                    .map(|job| build_job(inputs, job))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flat_map(|v| v.views)
                    .collect(),
            )
        }
    } else {
        #[cfg(test)]
        LAST_THREADS.with(|threads| {
            *threads.borrow_mut() = vec![std::thread::current().id(); jobs.len() + 1]
        });
        let main = pending.finish(inputs, &state).map_err(|e| e.error)?;
        (
            main,
            jobs.iter()
                .map(|job| build_job(inputs, job))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flat_map(|v| v.views)
                .collect(),
        )
    };
    let mut views = shadows;
    views.push(main);
    Ok(views)
}

pub(super) fn build_frozen(
    frozen: build_inputs::FrozenBuildInputs<'_, '_>,
    jobs: &[ViewJob],
    workers: usize,
) -> Result<PreparedBatch<GeometryFrame>, ContractError> {
    run_bounded(jobs.len(), workers, &|index| {
        build_job(&mut frozen.fork(), &jobs[index])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(feature = "real-wgpu")]
    fn dynamic_native_validation_overlaps_preparation_joins_and_preserves_first_error() {
        let ended = std::sync::atomic::AtomicUsize::new(0);
        let (worker, inline) = join_validation(
            true,
            || {
                ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(std::thread::current().id())
            },
            || {
                ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(std::thread::current().id())
            },
        )
        .unwrap();
        assert_ne!(worker, inline);
        assert_eq!(ended.load(std::sync::atomic::Ordering::SeqCst), 2);
        let failed = join_validation(
            true,
            || {
                ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err::<(), _>("first-validation".to_owned())
            },
            || {
                ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err::<(), _>("later-preparation".to_owned())
            },
        );
        assert_eq!(failed.unwrap_err(), "first-validation");
        assert_eq!(ended.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert_eq!(
            join_validation(
                true,
                || -> Result<(), String> { panic!("worker panic") },
                || Ok(())
            )
            .unwrap_err(),
            "geometry.prepare_task_panicked"
        );
        let (first, second) = join_validation(
            false,
            || Ok(std::thread::current().id()),
            || Ok(std::thread::current().id()),
        )
        .unwrap();
        assert_eq!(first, second);
    }
    #[test]
    fn bounded_tasks_execute_on_real_threads_merge_in_order_and_join_before_first_error() {
        for rows in [4, 32, 256] {
            let visited = std::sync::atomic::AtomicUsize::new(0);
            let batch = run_bounded(12, 4, &|job| {
                for _ in 0..rows {
                    visited.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Ok(job)
            })
            .unwrap();
            assert_eq!(batch.values, (0..12).collect::<Vec<_>>());
            assert_eq!(
                visited.load(std::sync::atomic::Ordering::Relaxed),
                rows * 12
            );
            assert_eq!(
                batch
                    .threads
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                4
            );
        }
        let ended = std::sync::atomic::AtomicUsize::new(0);
        let failure = run_bounded(12, 4, &|job| {
            ended.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if job == 5 {
                panic!("deterministic preparation failure");
            }
            if job == 2 {
                return Err(invalid("first_ordered_failure"));
            }
            Ok(job)
        });
        assert_eq!(failure.err().unwrap().field, "first_ordered_failure");
        assert_eq!(ended.load(std::sync::atomic::Ordering::Relaxed), 12);
        assert!(run_bounded(17, 4, &Ok::<_, ContractError>).is_err());
        assert!(run_bounded(1, 5, &Ok::<_, ContractError>).is_err());
    }
}
