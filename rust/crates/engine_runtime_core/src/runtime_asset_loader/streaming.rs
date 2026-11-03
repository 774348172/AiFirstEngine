//! Bounded immutable package work. The loader remains the only cache/reference owner.
use super::*;
use crate::platform_time::Instant;
use crate::runtime_asset::{CookedAssetRecord, RuntimeAssetRecord, RuntimeDecodedPayload};
use crate::runtime_package_path::safe_join_runtime_package;
use std::fs;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

#[path = "streaming_geometry.rs"]
mod geometry_decode;
#[path = "streaming_prefab.rs"]
mod prefab_decode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamingBudget {
    pub max_operations: usize,
    pub max_handles: usize,
    pub max_dependency_edges: usize,
    pub max_asset_bytes: usize,
    pub max_in_flight_bytes: usize,
    pub max_cpu_resident_bytes: usize,
    /// Maximum retained decoded payload per operation, in addition to source bytes.
    pub max_decode_bytes: usize,
    pub max_completions_per_poll: usize,
}

impl Default for StreamingBudget {
    fn default() -> Self {
        Self {
            max_operations: 64,
            max_handles: 4096,
            max_dependency_edges: 16384,
            max_asset_bytes: 16 * 1024 * 1024,
            max_in_flight_bytes: 256 * 1024 * 1024,
            max_cpu_resident_bytes: 512 * 1024 * 1024,
            max_decode_bytes: 32 * 1024 * 1024,
            max_completions_per_poll: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamingError {
    InvalidBudget,
    Busy,
    BudgetExceeded,
    MissingAsset,
    DependencyCycle,
    BundleNotMounted,
    UnsupportedCookedLayout,
    UnsafePath,
    ReadFailed(String),
    IntegrityMismatch,
    DecodeFailed(String),
    Cancelled,
    RevisionChanged,
    WorkerStopped,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamingSnapshot {
    pub pending_operations: usize,
    pub pending_tickets: usize,
    pub reserved_bytes: usize,
    pub cpu_resident_bytes: usize,
    pub dispatched_operations: u64,
    pub applied_operations: u64,
    pub cancelled_results: u64,
}

#[derive(Debug, Clone)]
struct Spec {
    key: String,
    record: RuntimeAssetRecord,
    cooked: CookedAssetRecord,
    reservation: usize,
}

#[derive(Debug)]
struct Operation {
    id: u64,
    consumers: HashSet<u64>,
    cancel: Arc<AtomicBool>,
    reservation: usize,
}

#[derive(Debug)]
struct Work {
    // Drop the observation before any backing it names is freed, including
    // queued cancellation, failed dispatch and worker/channel destruction.
    observation: Option<PayloadLease>,
    queued_at: Instant,
    id: u64,
    priority: i32,
    spec: Spec,
    root: PathBuf,
    budget: StreamingBudget,
    cancel: Arc<AtomicBool>,
    #[cfg(test)]
    gate: Option<Arc<TestWorkerGate>>,
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct TestWorkerGate {
    pub blocked: AtomicBool,
    pub entered: AtomicBool,
    pub phase: AtomicUsize,
    pub operation: AtomicU64,
    pub thread: std::sync::Mutex<Option<std::thread::ThreadId>>,
    pub completed: std::sync::Mutex<Vec<String>>,
    pub completion_blocked: AtomicBool,
    pub panic_on_resume: AtomicBool,
    pub decoded_backing_address: AtomicUsize,
    pub parser_backing_bytes: AtomicUsize,
}

#[cfg(test)]
impl TestWorkerGate {
    fn pause(&self, phase: usize, operation: u64, cancelled: &impl Fn() -> bool) {
        if self.phase.load(Ordering::Acquire) != phase {
            return;
        }
        self.operation.store(operation, Ordering::Release);
        self.entered.store(true, Ordering::Release);
        while self.blocked.load(Ordering::Acquire) && !cancelled() {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            !self.panic_on_resume.load(Ordering::Acquire),
            "injected package worker failure at phase {phase}"
        );
    }
}

#[derive(Debug)]
struct Completion {
    _observation: Option<PayloadLease>,
    finished_at: Instant,
    id: u64,
    key: String,
    result: Result<DecodedAsset, StreamingError>,
}

#[derive(Debug)]
struct Worker {
    sender: SyncSender<Work>,
    receiver: Receiver<Completion>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    activity: Arc<WorkerActivity>,
}

#[derive(Debug, Default)]
struct WorkerActivity {
    memory: Arc<PayloadMemory>,
    decoding: AtomicUsize,
    /// Completed payloads owned by the sender or completion mailbox, not reservations.
    ready: AtomicUsize,
    // Cumulative service/wall times, not additive critical-path spans.
    timing: [AtomicU64; 5],
}

/// Per-loader opt-in diagnostic metadata. It never owns payloads, dereferences
/// recorded addresses, charges a reservation, or changes the bounded channels.
#[derive(Debug, Default)]
struct PayloadMemory {
    entries: Mutex<Option<HashMap<u64, Vec<crate::resource_memory::AllocationSample>>>>,
}
#[derive(Debug)]
struct PayloadLease {
    id: u64,
    memory: Arc<PayloadMemory>,
}
// A borrowed census of the finished typed payload while decode_work still owns
// it. The observation must disappear before that backing can move or unwind;
// no Arc or other resource lifetime is extended for instrumentation.
struct DecodedObservation<'a> {
    asset: &'a DecodedAsset,
    memory: &'a PayloadMemory,
    operation: u64,
}
struct JsonObservation<'a> {
    value: &'a serde_json::Value,
    memory: &'a PayloadMemory,
    operation: u64,
}
impl Drop for JsonObservation<'_> {
    fn drop(&mut self) {
        let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
        crate::runtime_asset::observe_json_backing(&mut sample, self.value, "decodeWork");
        let mut ids: Vec<_> = sample
            .allocations
            .into_iter()
            .map(|a| a.allocation_id)
            .collect();
        ids.sort_unstable();
        if let Some(entries) = self.memory.entries.lock().unwrap().as_mut() {
            if let Some(allocations) = entries.get_mut(&self.operation) {
                allocations.retain(|a| ids.binary_search(&a.allocation_id).is_err());
            }
        }
    }
}
impl Drop for DecodedObservation<'_> {
    fn drop(&mut self) {
        let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
        self.asset.observe_memory(&mut sample, "decodeWork");
        let mut ids: Vec<_> = sample
            .allocations
            .into_iter()
            .map(|a| a.allocation_id)
            .collect();
        ids.sort_unstable();
        if let Some(entries) = self.memory.entries.lock().unwrap().as_mut() {
            if let Some(allocations) = entries.get_mut(&self.operation) {
                allocations.retain(|a| ids.binary_search(&a.allocation_id).is_err());
            }
        }
    }
}
impl Drop for PayloadLease {
    fn drop(&mut self) {
        if let Some(entries) = self.memory.entries.lock().unwrap().as_mut() {
            entries.remove(&self.id);
        }
    }
}
impl PayloadMemory {
    fn observe_json<'a>(
        &'a self,
        value: &'a serde_json::Value,
        operation: u64,
    ) -> Option<JsonObservation<'a>> {
        let mut guard = self.entries.lock().unwrap();
        let entries = guard.as_mut()?;
        let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
        crate::runtime_asset::observe_json_backing(&mut sample, value, "decodeWork");
        entries
            .entry(operation)
            .or_default()
            .extend(sample.allocations);
        Some(JsonObservation {
            value,
            memory: self,
            operation,
        })
    }
    fn observe_decoded<'a>(
        &'a self,
        asset: &'a DecodedAsset,
        operation: u64,
    ) -> Option<DecodedObservation<'a>> {
        let mut guard = self.entries.lock().unwrap();
        let entries = guard.as_mut()?;
        let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
        asset.observe_memory(&mut sample, "decodeWork");
        entries
            .entry(operation)
            .or_default()
            .extend(sample.allocations);
        Some(DecodedObservation {
            asset,
            memory: self,
            operation,
        })
    }
    fn publish_queue(&self, queued: &Vec<Work>) {
        let mut guard = self.entries.lock().unwrap();
        if let Some(entries) = guard.as_mut() {
            let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            sample.cpu_vec(queued, "queues", &["queuePayload"]);
            entries.insert(0, sample.allocations);
        }
    }
    fn publish_work(&self, work: &Work, role: &str) {
        let mut guard = self.entries.lock().unwrap();
        if let Some(entries) = guard.as_mut() {
            let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            work.observe_memory(&mut sample, role);
            entries.insert(work.id, sample.allocations);
        }
    }
    fn capture(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        let mut guard = self.entries.lock().unwrap();
        let entries = guard.get_or_insert_with(HashMap::new);
        sample.capacities.insert(
            "loader.observedPayloads".into(),
            entries.keys().filter(|id| **id != 0).count() as u64,
        );
        for allocations in entries.values() {
            sample.cpu_vec(allocations, "instrumentation", &["instrumentation"]);
            for a in allocations {
                sample.cpu_string(&a.allocation_id, "instrumentation", &["instrumentation"]);
                sample.cpu_string(&a.owner, "instrumentation", &["instrumentation"]);
                for role in &a.roles {
                    sample.cpu_string(role, "instrumentation", &["instrumentation"]);
                }
            }
            sample.allocations.extend(allocations.iter().cloned());
        }
        sample.partial(
            "cpu.owner.instrumentation",
            "loader.payloadObservation.vec+strings.capacity;stdNodesOpaque",
        );
    }
}
impl Work {
    fn observe_memory(&self, s: &mut crate::resource_memory::ResourceMemorySample, role: &str) {
        s.cpu_string(&self.spec.key, "queues", &[role]);
        self.spec.record.observe_memory(s, "queues", role);
        self.spec.cooked.observe_memory(s, "queues", role);
        s.cpu_storage(
            self.root.as_os_str().as_encoded_bytes().as_ptr() as usize,
            self.root.capacity(),
            "queues",
            &[role],
        );
        s.cpu_arc(&self.cancel, "queues", &[role]);
    }
}
impl Completion {
    fn observe_memory(&self, s: &mut crate::resource_memory::ResourceMemorySample) {
        s.cpu_string(&self.key, "queues", &["queuePayload"]);
        match &self.result {
            Ok(asset) => asset.observe_memory(s, "queuePayload"),
            Err(StreamingError::ReadFailed(message) | StreamingError::DecodeFailed(message)) => {
                s.cpu_string(message, "queues", &["queuePayload"]);
            }
            _ => {}
        }
    }
}

// Source/pixel backing lives across read and decode. The buffer never grows past
// its preallocated capacity; drop its diagnostic record before freeing/moving it.
struct ReadBuffer {
    observation: Option<ReadObservation>,
    bytes: Vec<u8>,
}
struct ReadObservation {
    operation: u64,
    address: usize,
    memory: Arc<PayloadMemory>,
}
impl Drop for ReadObservation {
    fn drop(&mut self) {
        if let Some(entries) = self.memory.entries.lock().unwrap().as_mut() {
            if let Some(allocations) = entries.get_mut(&self.operation) {
                allocations.retain(|a| {
                    a.allocation_id
                        .strip_prefix("cpu-storage:")
                        .and_then(|v| usize::from_str_radix(v, 16).ok())
                        != Some(self.address)
                });
                if allocations.is_empty() {
                    entries.remove(&self.operation);
                }
            }
        }
    }
}
impl ReadBuffer {
    fn new(capacity: usize, operation: u64, memory: &Arc<PayloadMemory>) -> Self {
        let bytes = Vec::with_capacity(capacity);
        let mut guard = memory.entries.lock().unwrap();
        let observation = guard.as_mut().map(|entries| {
            let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
            sample.cpu_vec(&bytes, "assets", &["decodeWork"]);
            entries
                .entry(operation)
                .or_default()
                .extend(sample.allocations);
            ReadObservation {
                operation,
                address: bytes.as_ptr() as usize,
                memory: memory.clone(),
            }
        });
        Self { observation, bytes }
    }
    fn into_vec(mut self) -> Vec<u8> {
        drop(self.observation.take());
        std::mem::take(&mut self.bytes)
    }
}
impl std::ops::Deref for ReadBuffer {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}
impl AsRef<[u8]> for ReadBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

struct DecodeActivity<'a>(&'a WorkerActivity);
impl Drop for DecodeActivity<'_> {
    fn drop(&mut self) {
        self.0.decoding.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Worker {
    fn new(capacity: usize, memory: Arc<PayloadMemory>) -> Result<Self, StreamingError> {
        let (sender, jobs) = mpsc::sync_channel::<Work>(capacity);
        // PC ready bound includes the worker's completed payload while send is
        // blocked. One fewer mailbox slot leaves that ownership within 128.
        // Operations retain their original admission charges until host poll.
        let (completed, receiver) = mpsc::sync_channel(capacity.min(128).saturating_sub(1));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let activity = Arc::new(WorkerActivity {
            memory,
            decoding: AtomicUsize::new(0),
            ready: AtomicUsize::new(0),
            timing: Default::default(),
        });
        let worker_activity = activity.clone();
        let thread = std::thread::Builder::new()
            .name("runtime-package-loader".into())
            .spawn(move || {
                let mut queued = Vec::with_capacity(capacity);
                // Operation IDs start at one. Zero names only this worker-owned
                // bounded Vec backing, whose lease drops before the Vec itself.
                let _queued_observation = PayloadLease {
                    id: 0,
                    memory: worker_activity.memory.clone(),
                };
                while !worker_stop.load(Ordering::Acquire) {
                    if queued.is_empty() {
                        match jobs.recv_timeout(Duration::from_millis(20)) {
                            Ok(work) => queued.push(work),
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    while queued.len() < capacity {
                        match jobs.try_recv() {
                            Ok(work) => queued.push(work),
                            Err(_) => break,
                        }
                    }
                    let selected = queued
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, w)| (w.priority, std::cmp::Reverse(w.id)))
                        .unwrap()
                        .0;
                    let mut work = queued.swap_remove(selected);
                    worker_activity.memory.publish_queue(&queued);
                    worker_activity.memory.publish_work(&work, "decodeWork");
                    worker_activity.timing[0]
                        .fetch_add(nanos(work.queued_at.elapsed()), Ordering::AcqRel);
                    let decode_started = Instant::now();
                    let result = {
                        worker_activity.decoding.fetch_add(1, Ordering::AcqRel);
                        let _activity = DecodeActivity(&worker_activity);
                        decode_work(&work, &worker_stop, &worker_activity)
                    };
                    worker_activity.timing[1]
                        .fetch_add(nanos(decode_started.elapsed()), Ordering::AcqRel);
                    worker_activity.timing[2].fetch_add(1, Ordering::AcqRel);
                    #[cfg(test)]
                    if let Some(gate) = &work.gate {
                        gate.completed
                            .lock()
                            .unwrap()
                            .push(work.spec.record.asset_id.clone());
                    }
                    // Remove the old Work fields and publish Completion under the
                    // same lock used by capture. No freed address can outlive its
                    // diagnostic lease during this ownership transition.
                    let mut memory = worker_activity.memory.entries.lock().unwrap();
                    let completion = Completion {
                        _observation: work.observation.take(),
                        finished_at: Instant::now(),
                        id: work.id,
                        key: std::mem::take(&mut work.spec.key),
                        result,
                    };
                    #[cfg(test)]
                    let completion_gate = work.gate.clone();
                    drop(work);
                    if let Some(entries) = memory.as_mut() {
                        let mut sample = crate::resource_memory::ResourceMemorySample::new(0, 0, 0);
                        completion.observe_memory(&mut sample);
                        entries.insert(completion.id, sample.allocations);
                    }
                    drop(memory);
                    worker_activity.ready.fetch_add(1, Ordering::AcqRel);
                    #[cfg(test)]
                    let sent = match completed.try_send(completion) {
                        Ok(()) => Ok(()),
                        Err(mpsc::TrySendError::Full(completion)) => {
                            if let Some(gate) = completion_gate {
                                gate.completion_blocked.store(true, Ordering::Release);
                            }
                            completed.send(completion)
                        }
                        Err(mpsc::TrySendError::Disconnected(completion)) => {
                            Err(mpsc::SendError(completion))
                        }
                    };
                    #[cfg(not(test))]
                    let sent = completed.send(completion);
                    if sent.is_err() {
                        worker_activity.ready.fetch_sub(1, Ordering::AcqRel);
                        break;
                    }
                }
            })
            .map_err(|_| StreamingError::WorkerStopped)?;
        Ok(Self {
            sender,
            receiver,
            stop,
            thread: Some(thread),
            activity,
        })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Never unconditionally join a worker blocked by the OS. Dropping the
        // receiver also unblocks completion delivery; all work owns immutable data.
        if self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            let _ = self.thread.take().unwrap().join();
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct StreamingState {
    memory: Arc<PayloadMemory>,
    budget: StreamingBudget,
    worker: Option<Worker>,
    operations: HashMap<String, Operation>,
    // Cancelled work no longer owns the current lookup key, but keeps its
    // admission charge until its own completion (or worker shutdown) arrives.
    retired_operations: HashMap<u64, Operation>,
    tickets: HashMap<u64, Vec<u64>>,
    errors: HashMap<u64, StreamingError>,
    // Pending consumers have not acquired a decoded-cache reference yet. A
    // synchronous consumer may populate the same key while their worker runs.
    pending_cache_handles: HashSet<u64>,
    next_operation: u64,
    dispatched: u64,
    applied: u64,
    cancelled: u64,
    stopped: bool,
    #[cfg(test)]
    pub gate: Option<Arc<TestWorkerGate>>,
}

impl RuntimeAssetLoader {
    pub(super) fn observe_streaming_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        self.streaming.memory.capture(sample);
        sample.cpu_arc(
            &self.streaming.memory,
            "instrumentation",
            &["instrumentation"],
        );
        let (decoding, ready) = self.streaming.worker.as_ref().map_or((0, 0), |w| {
            (
                w.activity.decoding.load(Ordering::Acquire),
                w.activity.ready.load(Ordering::Acquire),
            )
        });
        sample.capacities.insert(
            "activeRequests".into(),
            self.handles
                .values()
                .filter(|h| h.state == RuntimeAssetLoadState::Loading)
                .count() as u64,
        );
        sample.capacities.insert("readyQueue".into(), ready as u64);
        sample
            .capacities
            .insert("failureCache".into(), self.streaming.errors.len() as u64);
        sample
            .capacities
            .insert("concurrentDecodes".into(), decoding as u64);
        sample.observed_scopes.insert(
            "loader.capacities=loadingHandles+pendingCompletions+errors+activeDecode".into(),
        );
        if let Some(worker) = &self.streaming.worker {
            for (i, name) in [
                "queueNs",
                "serviceNs",
                "operations",
                "completionPollNs",
                "readNs",
            ]
            .iter()
            .enumerate()
            {
                sample.capacities.insert(
                    format!("loader.stage.{name}"),
                    worker.activity.timing[i].load(Ordering::Acquire),
                );
            }
        }
        for key in self.streaming.operations.keys() {
            sample.cpu_string(key, "queues", &["queuePayload"]);
        }
        for operation in self.streaming.retired_operations.values() {
            sample.cpu_arc(&operation.cancel, "queues", &["inFlight", "retiring"]);
        }
        sample.capacities.insert(
            "retiredAssetOperations".into(),
            self.streaming.retired_operations.len() as u64,
        );
        for error in self.streaming.errors.values() {
            if let StreamingError::ReadFailed(message) | StreamingError::DecodeFailed(message) =
                error
            {
                sample.cpu_string(message, "queues", &["queuePayload"]);
            }
        }
        for handles in self.streaming.tickets.values() {
            sample.cpu_vec(handles, "queues", &["queuePayload"]);
        }
        sample.partial("cpu.role.queuePayload", "loader.ticketVectors+operationKeys+workSpec+completionPayload.capacity;captureEnablesFutureTransitions;stdChannelSlotsOpaque");
        sample.partial("cpu.role.decodeWork", "loader.workSpec+sourceReadBuffer+pixelReadBuffer+parsedScenePrefabValue+finishedTypedPayload.capacity;captureEnablesFutureTransitions;parserScratchNotInstrumented;stdChannelSlotsOpaque");
        // A reserved decode envelope is an admission charge, never allocated bytes.
        sample.partial(
            "cpu.owner.queues",
            "loader.workSpec+completionPayload.capacity;captureEnablesFutureTransitions;stdChannelSlots+stdNodes+arcControlBlocksOpaque",
        );
    }

    // Keep recent Released lookups bounded for synchronous consumers too.
    // Only Released records are eligible; live failure reports stay queryable.
    pub(super) fn prune_released_handles(&mut self, retain: usize) {
        while self.released_handles.len() > retain {
            let id = self.released_handles.pop_front().unwrap();
            if self
                .handles
                .get(&id)
                .is_some_and(|h| h.state == RuntimeAssetLoadState::Released)
            {
                self.handles.remove(&id);
                self.streaming.errors.remove(&id);
            }
        }
    }

    pub fn streaming_budget(&self) -> StreamingBudget {
        self.streaming.budget
    }
    pub fn set_streaming_budget(&mut self, budget: StreamingBudget) -> Result<(), StreamingError> {
        if !self.streaming.operations.is_empty()
            || !self.streaming.retired_operations.is_empty()
            || !self.streaming.tickets.is_empty()
            || self.external_io_operations != 0
        {
            return Err(StreamingError::Busy);
        }
        if budget.max_operations == 0
            || budget.max_operations > 4096
            || budget.max_handles == 0
            || budget.max_handles > 1_048_576
            || budget.max_dependency_edges == 0
            || budget.max_dependency_edges > 1_048_576
            || budget.max_asset_bytes == 0
            || budget.max_decode_bytes == 0
            || budget.max_completions_per_poll == 0
            || budget.max_in_flight_bytes < budget.max_asset_bytes
            || budget.max_cpu_resident_bytes < budget.max_asset_bytes
        {
            return Err(StreamingError::InvalidBudget);
        }
        if self.streaming_snapshot().cpu_resident_bytes > budget.max_cpu_resident_bytes
            || self
                .handles
                .values()
                .filter(|h| h.state != RuntimeAssetLoadState::Released)
                .count()
                > budget.max_handles
        {
            return Err(StreamingError::BudgetExceeded);
        }
        self.streaming.worker = None;
        self.streaming.budget = budget;
        self.prune_released_handles(budget.max_handles);
        self.released_handles.shrink_to(budget.max_handles);
        Ok(())
    }

    pub fn streaming_snapshot(&self) -> StreamingSnapshot {
        StreamingSnapshot {
            pending_operations: self.streaming.operations.len()
                + self.streaming.retired_operations.len()
                + self.external_io_operations,
            pending_tickets: self.streaming.tickets.len(),
            reserved_bytes: self
                .streaming
                .operations
                .values()
                .chain(self.streaming.retired_operations.values())
                .map(|op| op.reservation)
                .sum::<usize>()
                .saturating_add(self.external_io_bytes),
            cpu_resident_bytes: self
                .decoded_cache
                .values()
                .map(|a| a.bytes_len)
                .sum::<usize>()
                .saturating_add(self.external_cpu_reservation),
            dispatched_operations: self.streaming.dispatched,
            applied_operations: self.streaming.applied,
            cancelled_results: self.streaming.cancelled,
        }
    }

    pub fn streaming_error(&self, handle: &RuntimeAssetHandle) -> Option<&StreamingError> {
        self.handles
            .get(&handle.handle_id)
            .filter(|stored| stored.generation == handle.generation)?;
        self.streaming.errors.get(&handle.handle_id)
    }

    pub fn current_handle(&self, handle: &RuntimeAssetHandle) -> Option<RuntimeAssetHandle> {
        self.handles
            .get(&handle.handle_id)
            .filter(|stored| stored.generation == handle.generation)
            .cloned()
    }

    pub fn decoded_payload(
        &self,
        handle: &RuntimeAssetHandle,
    ) -> Option<Arc<RuntimeDecodedPayload>> {
        if self.get_handle_state(handle)? != RuntimeAssetLoadState::Ready {
            return None;
        }
        self.decoded_cache
            .get(self.cache_keys_by_handle.get(&handle.handle_id)?)?
            .payload
            .clone()
    }

    pub(crate) fn closure_contains_ready(
        &self,
        root: &RuntimeAssetHandle,
        reference: &RuntimeAssetRef,
    ) -> bool {
        let Ok(record) = self.index.resolve(reference) else {
            return false;
        };
        let key = self.cache_identity(record);
        std::iter::once(root)
            .chain(
                self.dependency_handles_by_owner
                    .get(&root.handle_id)
                    .into_iter()
                    .flatten(),
            )
            .any(|h| {
                self.get_handle_state(h) == Some(RuntimeAssetLoadState::Ready)
                    && self.cache_keys_by_handle.get(&h.handle_id) == Some(&key)
            })
    }

    pub(crate) fn ready_reference_charge(&self, reference: &RuntimeAssetRef) -> Option<usize> {
        fn charge(
            loader: &RuntimeAssetLoader,
            record: &RuntimeAssetRecord,
            seen: &mut HashSet<String>,
        ) -> Option<usize> {
            if !seen.insert(record.asset_guid.clone()) {
                return Some(0);
            }
            let mut bytes = loader
                .decoded_cache
                .get(&loader.cache_identity(record))?
                .bytes_len;
            for guid in &record.dependencies {
                bytes =
                    bytes.checked_add(charge(loader, loader.index.record_by_guid(guid)?, seen)?)?;
            }
            Some(bytes)
        }
        charge(
            self,
            self.index.resolve(reference).ok()?,
            &mut HashSet::new(),
        )
    }

    pub(crate) fn ready_prefab_copy_charge(&self, root: &RuntimeAssetHandle) -> Option<usize> {
        let cached = self
            .decoded_cache
            .get(self.cache_keys_by_handle.get(&root.handle_id)?)?;
        if !matches!(
            cached.payload.as_deref(),
            Some(RuntimeDecodedPayload::Prefab(_))
        ) {
            return None;
        }
        // Only the root's typed payload is cloned by hydration. Its dependency
        // cache is already charged and each actual consumer copy is added below.
        cached
            .bytes_len
            .checked_sub(cached.bytes.as_ref().map_or(0, |b| b.len()))?
            .checked_mul(4)
    }

    /// Preparation peak and backing retained after preparation. Source Arc bytes
    /// and decoder growth buffers remain charged in the cache, not as cell copies.
    pub(crate) fn prepared_reference_charges(
        &self,
        reference: &RuntimeAssetRef,
    ) -> Option<(usize, usize)> {
        let cached_charge = self.ready_reference_charge(reference)?;
        if matches!(reference.asset_type.as_str(), "character3d" | "audio") {
            // Cell Prepare copies their component references in the root, but
            // invokes only physics/navigation and renderer owners. It creates
            // no character/audio payload copy. The complete dependency cache
            // stays charged and pinned by the root until actual release.
            return Some((0, 0));
        }
        if reference.asset_type == "vegetation3d" {
            // Clusters are consumed into renderers; their mesh references are
            // charged separately by the same cell preparation traversal. The
            // cache holds source bytes, but preparation parses them again.
            let record = self.index.resolve(reference).ok()?;
            let source_bytes = usize::try_from(record.size?).ok()?;
            let parse_peak = decode_envelope(source_bytes, &record.asset_type).ok()?;
            return Some((cached_charge.checked_add(parse_peak)?.checked_mul(4)?, 0));
        }
        if reference.asset_type == "material" {
            let record = self.index.resolve(reference).ok()?;
            let cached = self.decoded_cache.get(&self.cache_identity(record))?;
            let RuntimeDecodedPayload::Render(asset) = cached.payload.as_deref()? else {
                return None;
            };
            let retained = geometry_decode::material_metadata_copy_bound(asset)?;
            let source = usize::try_from(record.size?).ok()?;
            return Some((retained.checked_add(source)?, retained));
        }
        let peak = cached_charge.checked_mul(4)?;
        if reference.asset_type != "mesh" {
            return Some((peak, peak));
        }
        let record = self.index.resolve(reference).ok()?;
        let cached = self.decoded_cache.get(&self.cache_identity(record))?;
        let RuntimeDecodedPayload::Render(asset) = cached.payload.as_deref()? else {
            return None;
        };
        let crate::render_contract::RenderAssetData::Mesh(mesh) = &asset.asset else {
            return None;
        };
        let binary = mesh
            .vertex_bytes
            .len()
            .checked_add(mesh.index_bytes.len())?;
        let metadata = geometry_decode::mesh_metadata_copy_bound(asset)?;
        // from_assets creates one real buffer copy for every submesh and
        // renderer. Keep those copies; do not deduplicate authored references.
        let retained = binary
            .checked_add(metadata)?
            .checked_mul(mesh.submeshes.len())?;
        // A temporary CookedRenderAsset clone and a verified source reread can
        // coexist with the final buffers, even after the cache shed raw bytes.
        let source_bytes = usize::try_from(record.size?).ok()?;
        Some((
            retained
                .checked_add(binary)?
                .checked_add(metadata)?
                .checked_add(source_bytes)?,
            retained,
        ))
    }

    pub(crate) fn closure_reference_bytes(
        &self,
        root: &RuntimeAssetHandle,
        reference: &RuntimeAssetRef,
    ) -> Option<Arc<[u8]>> {
        let record = self.index.resolve(reference).ok()?;
        let key = self.cache_identity(record);
        let handle = std::iter::once(root)
            .chain(
                self.dependency_handles_by_owner
                    .get(&root.handle_id)
                    .into_iter()
                    .flatten(),
            )
            .find(|h| {
                self.get_handle_state(h) == Some(RuntimeAssetLoadState::Ready)
                    && self.cache_keys_by_handle.get(&h.handle_id) == Some(&key)
            })?;
        self.get_asset_bytes(handle)
    }

    /// External I/O uses the same pending-operation, in-flight and CPU reservation limits.
    /// Completed reads remain reserved until their actual payload is claimed or abandoned.
    pub(crate) fn reserve_external_io(&mut self, bytes: usize) -> Result<(), StreamingError> {
        let snapshot = self.streaming_snapshot();
        let budget = self.streaming.budget;
        if snapshot.pending_operations >= budget.max_operations
            || snapshot.reserved_bytes.saturating_add(bytes) > budget.max_in_flight_bytes
            || snapshot
                .cpu_resident_bytes
                .saturating_add(snapshot.reserved_bytes)
                .saturating_add(bytes)
                > budget.max_cpu_resident_bytes
        {
            return Err(StreamingError::BudgetExceeded);
        }
        self.external_io_bytes += bytes;
        self.external_io_operations += 1;
        Ok(())
    }
    pub(crate) fn release_external_io(&mut self, bytes: usize) {
        debug_assert!(self.external_io_bytes >= bytes && self.external_io_operations > 0);
        self.external_io_bytes -= bytes;
        self.external_io_operations -= 1;
    }
    pub(crate) fn reserve_prepared_cpu(&mut self, bytes: usize) -> Result<(), StreamingError> {
        let snapshot = self.streaming_snapshot();
        if snapshot
            .cpu_resident_bytes
            .saturating_add(snapshot.reserved_bytes)
            .saturating_add(bytes)
            > self.streaming.budget.max_cpu_resident_bytes
        {
            return Err(StreamingError::BudgetExceeded);
        }
        self.external_cpu_reservation = self.external_cpu_reservation.saturating_add(bytes);
        Ok(())
    }
    pub(crate) fn release_prepared_cpu(&mut self, bytes: usize) {
        debug_assert!(self.external_cpu_reservation >= bytes);
        self.external_cpu_reservation = self.external_cpu_reservation.saturating_sub(bytes);
    }

    /// Borrow the exact ready request and its original dependency closure. The
    /// caller still owns the asynchronous request; this never reads or polls.
    pub(crate) fn retain_ready_reference(
        &mut self,
        reference: &RuntimeAssetRef,
    ) -> Result<Option<RuntimeAssetHandle>, StreamingError> {
        let record = self
            .index
            .resolve(reference)
            .map_err(|_| StreamingError::MissingAsset)?;
        let key = self.cache_identity(record);
        let ready = self
            .handles
            .values()
            .find(|handle| {
                handle.state == RuntimeAssetLoadState::Ready
                    && self.cache_keys_by_handle.get(&handle.handle_id) == Some(&key)
                    && self
                        .dependency_handles_by_owner
                        .contains_key(&handle.handle_id)
            })
            .cloned();
        ready.map(|handle| self.retain_handle(&handle)).transpose()
    }

    /// Acquire an independent lease of the exact resident generation, including
    /// its original dependency closure. Does not resolve a newer index or read disk.
    pub fn retain_handle(
        &mut self,
        handle: &RuntimeAssetHandle,
    ) -> Result<RuntimeAssetHandle, StreamingError> {
        let mut pending = vec![handle.clone()];
        let mut originals = Vec::new();
        let mut seen = HashSet::new();
        while let Some(current) = pending.pop() {
            if !seen.insert(current.handle_id) {
                continue;
            }
            if self.get_handle_state(&current) != Some(RuntimeAssetLoadState::Ready) {
                return Err(StreamingError::MissingAsset);
            }
            if !self
                .cache_keys_by_handle
                .get(&current.handle_id)
                .is_some_and(|key| self.decoded_cache.contains_key(key))
            {
                return Err(StreamingError::MissingAsset);
            }
            if originals.len() >= self.streaming.budget.max_handles {
                return Err(StreamingError::BudgetExceeded);
            }
            if let Some(dependencies) = self.dependency_handles_by_owner.get(&current.handle_id) {
                pending.extend(dependencies.iter().cloned());
            }
            originals.push(self.handles[&current.handle_id].clone());
        }
        if self.handles.len().saturating_add(originals.len()) > self.streaming.budget.max_handles {
            self.prune_released_handles(0);
        }
        if self.handles.len().saturating_add(originals.len()) > self.streaming.budget.max_handles {
            return Err(StreamingError::BudgetExceeded);
        }
        let mut retained = HashMap::new();
        for old in &originals {
            let key = self.cache_keys_by_handle[&old.handle_id].clone();
            let mut new = old.clone();
            new.handle_id = self.next_handle_id();
            new.generation = self.next_generation();
            self.decoded_cache.get_mut(&key).unwrap().ref_count += 1;
            self.cache_keys_by_handle.insert(new.handle_id, key);
            self.handles.insert(new.handle_id, new.clone());
            retained.insert(old.handle_id, new);
        }
        for old in originals {
            if let Some(dependencies) = self.dependency_handles_by_owner.get(&old.handle_id) {
                let dependencies = dependencies
                    .iter()
                    .map(|dep| retained[&dep.handle_id].clone())
                    .collect();
                self.dependency_handles_by_owner
                    .insert(retained[&old.handle_id].handle_id, dependencies);
            }
        }
        Ok(retained.remove(&handle.handle_id).unwrap())
    }

    pub(super) fn cache_identity(&self, record: &RuntimeAssetRecord) -> String {
        // Length-delimited canonical serde identity includes version, subasset,
        // both digests, layout and dependency revision. No cooked-id-only alias.
        serde_json::to_string(&(record, self.index.cooked_asset(&record.cooked_asset_id))).unwrap()
    }

    fn streaming_specs(&self, reference: &RuntimeAssetRef) -> Result<Vec<Spec>, StreamingError> {
        fn visit(
            loader: &RuntimeAssetLoader,
            record: &RuntimeAssetRecord,
            visiting: &mut HashSet<String>,
            seen: &mut HashSet<String>,
            out: &mut Vec<Spec>,
            edges: &mut usize,
        ) -> Result<(), StreamingError> {
            if visiting.contains(&record.asset_guid) {
                return Err(StreamingError::DependencyCycle);
            }
            if visiting.len() >= 128 {
                return Err(StreamingError::BudgetExceeded);
            }
            if !seen.insert(record.asset_guid.clone()) {
                return Ok(());
            }
            *edges = edges
                .checked_add(record.dependencies.len())
                .ok_or(StreamingError::BudgetExceeded)?;
            if *edges > loader.streaming.budget.max_dependency_edges {
                return Err(StreamingError::BudgetExceeded);
            }
            if out.len() >= loader.streaming.budget.max_handles {
                return Err(StreamingError::BudgetExceeded);
            }
            if !loader.mount_table.is_bundle_mounted(&record.bundle_id) {
                return Err(StreamingError::BundleNotMounted);
            }
            let cooked = loader
                .index
                .cooked_asset(&record.cooked_asset_id)
                .ok_or(StreamingError::MissingAsset)?
                .clone();
            if cooked.bundle_id != record.bundle_id
                || cooked.path.is_none()
                || cooked.offset.unwrap_or(0) != 0
                || !matches!(cooked.compression.as_deref(), None | Some("none"))
            {
                return Err(StreamingError::UnsupportedCookedLayout);
            }
            crate::runtime_package_path::RuntimePackagePath::parse(
                cooked.path.as_ref().unwrap().clone(),
            )
            .map_err(|_| StreamingError::UnsafePath)?;
            let size = usize::try_from(cooked.size.ok_or(StreamingError::IntegrityMismatch)?)
                .map_err(|_| StreamingError::BudgetExceeded)?;
            if record.size != cooked.size || record.hash.is_none() || cooked.hash != record.hash {
                return Err(StreamingError::IntegrityMismatch);
            }
            if size > loader.streaming.budget.max_asset_bytes {
                return Err(StreamingError::BudgetExceeded);
            }
            // Texture dimensions are read by the worker, so retain their full
            // ceiling. Other assets use the same input-proportional bound as decode.
            let decode_capacity = if record.asset_type == "texture" {
                loader.streaming.budget.max_decode_bytes
            } else {
                decode_envelope(size, &record.asset_type)?
                    .min(loader.streaming.budget.max_decode_bytes)
            };
            let reservation = size
                .checked_add(decode_capacity)
                .ok_or(StreamingError::BudgetExceeded)?;
            out.push(Spec {
                key: loader.cache_identity(record),
                record: record.clone(),
                cooked,
                reservation,
            });
            visiting.insert(record.asset_guid.clone());
            for guid in &record.dependencies {
                let dep = loader
                    .index
                    .record_by_guid(guid)
                    .ok_or(StreamingError::MissingAsset)?;
                visit(loader, dep, visiting, seen, out, edges)?;
            }
            visiting.remove(&record.asset_guid);
            Ok(())
        }
        let record = self
            .index
            .resolve(reference)
            .map_err(|_| StreamingError::MissingAsset)?;
        let mut specs = Vec::new();
        visit(
            self,
            record,
            &mut HashSet::new(),
            &mut HashSet::new(),
            &mut specs,
            &mut 0,
        )?;
        Ok(specs)
    }

    /// Returns a consumer ticket; even cache hits become visible only at poll.
    /// No disk read, decode, World mutation or wait occurs on this path.
    pub fn request_async(
        &mut self,
        reference: &RuntimeAssetRef,
    ) -> Result<RuntimeAssetHandle, StreamingError> {
        self.request_async_priority(reference, 0)
    }

    /// Higher values run first among queued operations. An immutable read already
    /// in progress is not preempted; joining consumers preserve its original priority.
    pub fn request_async_priority(
        &mut self,
        reference: &RuntimeAssetRef,
        priority: i32,
    ) -> Result<RuntimeAssetHandle, StreamingError> {
        if self.source.is_memory() {
            return Err(StreamingError::UnsupportedCookedLayout);
        }
        if self.streaming.stopped {
            return Err(StreamingError::WorkerStopped);
        }
        let specs = self.streaming_specs(reference)?;
        let budget = self.streaming.budget;
        if self.handles.len() + specs.len() > budget.max_handles {
            self.prune_released_handles(0);
        }
        if self.handles.len() + specs.len() > budget.max_handles {
            return Err(StreamingError::BudgetExceeded);
        }
        // Synchronous hydration can retain a mesh identity without typed
        // geometry. An async cell still needs the worker payload for bounded
        // preparation; keep old readers pinned while one upgrade is in flight.
        let cache_ready = |decoded: &DecodedAsset| {
            !matches!(decoded.asset_type.as_str(), "mesh" | "material")
                || matches!(
                    decoded.payload.as_deref(),
                    Some(RuntimeDecodedPayload::Render(_))
                )
        };
        let new_specs: Vec<_> = specs
            .iter()
            .filter(|s| {
                !self.decoded_cache.get(&s.key).is_some_and(cache_ready)
                    && !self.streaming.operations.contains_key(&s.key)
            })
            .collect();
        let additional = new_specs
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.reservation))
            .ok_or(StreamingError::BudgetExceeded)?;
        let snapshot = self.streaming_snapshot();
        if snapshot.pending_operations + new_specs.len() > budget.max_operations
            || snapshot.reserved_bytes.saturating_add(additional) > budget.max_in_flight_bytes
            || snapshot
                .cpu_resident_bytes
                .saturating_add(snapshot.reserved_bytes)
                .saturating_add(additional)
                > budget.max_cpu_resident_bytes
        {
            return Err(StreamingError::BudgetExceeded);
        }
        if self.streaming.worker.is_none() {
            self.streaming.worker = Some(Worker::new(
                budget.max_operations,
                self.streaming.memory.clone(),
            )?);
        }
        let mut handles = Vec::with_capacity(specs.len());
        for spec in specs {
            let record = &spec.record;
            let mut handle = RuntimeAssetHandle {
                handle_id: self.next_handle_id(),
                asset_guid: record.asset_guid.clone(),
                asset_id: record.asset_id.clone(),
                asset_type: record.asset_type.clone(),
                sub_asset_id: record.sub_asset_id.clone(),
                cooked_asset_id: record.cooked_asset_id.clone(),
                bundle_id: record.bundle_id.clone(),
                runtime_resource_id: None,
                state: RuntimeAssetLoadState::Loading,
                generation: self.next_generation(),
                ref_count: 1,
                loader_kind: record.loader_kind.clone(),
                version: record.version.clone(),
            };
            if let Some(decoded) = self
                .decoded_cache
                .get_mut(&spec.key)
                .filter(|value| cache_ready(value))
            {
                decoded.ref_count += 1;
                handle.state = RuntimeAssetLoadState::Ready;
            } else if let Some(operation) = self.streaming.operations.get_mut(&spec.key) {
                operation.consumers.insert(handle.handle_id);
                self.streaming
                    .pending_cache_handles
                    .insert(handle.handle_id);
            } else {
                self.streaming.next_operation += 1;
                let id = self.streaming.next_operation;
                let cancel = Arc::new(AtomicBool::new(false));
                let operation = Operation {
                    id,
                    consumers: HashSet::from([handle.handle_id]),
                    cancel: cancel.clone(),
                    reservation: spec.reservation,
                };
                let work = Work {
                    observation: Some(PayloadLease {
                        id,
                        memory: self.streaming.memory.clone(),
                    }),
                    queued_at: Instant::now(),
                    id,
                    priority,
                    spec: spec.clone(),
                    root: self.package_dir.clone(),
                    budget,
                    cancel,
                    #[cfg(test)]
                    gate: self.streaming.gate.clone(),
                };
                self.streaming.memory.publish_work(&work, "queuePayload");
                if self
                    .streaming
                    .worker
                    .as_ref()
                    .unwrap()
                    .sender
                    .try_send(work)
                    .is_err()
                {
                    self.release_loaded_dependencies(handles);
                    return Err(StreamingError::WorkerStopped);
                }
                self.streaming.dispatched += 1;
                self.streaming
                    .operations
                    .insert(spec.key.clone(), operation);
                self.streaming
                    .pending_cache_handles
                    .insert(handle.handle_id);
            }
            self.cache_keys_by_handle.insert(handle.handle_id, spec.key);
            self.handles.insert(handle.handle_id, handle.clone());
            handles.push(handle);
        }
        let mut root = handles.remove(0);
        self.streaming.tickets.insert(
            root.handle_id,
            handles.iter().map(|h| h.handle_id).collect(),
        );
        self.dependency_handles_by_owner
            .insert(root.handle_id, handles);
        // Root CPU payload may be ready before its dependency closure.
        self.handles.get_mut(&root.handle_id).unwrap().state = RuntimeAssetLoadState::Loading;
        root.state = RuntimeAssetLoadState::Loading;
        Ok(root)
    }

    /// Applies a bounded number of worker results. Ready means CPUReady only.
    pub fn poll_async(&mut self) -> usize {
        let mut count = 0;
        for _ in 0..self.streaming.budget.max_completions_per_poll {
            let completion = match self
                .streaming
                .worker
                .as_ref()
                .map(|w| w.receiver.try_recv())
            {
                Some(Ok(completion)) => {
                    // poll has exclusive loader access; remove the mailbox census
                    // before moving the same allocation into the cache or dropping it.
                    if let Some(entries) = self.streaming.memory.entries.lock().unwrap().as_mut() {
                        entries.remove(&completion.id);
                    }
                    self.streaming
                        .worker
                        .as_ref()
                        .unwrap()
                        .activity
                        .ready
                        .fetch_sub(1, Ordering::AcqRel);
                    let activity = &self.streaming.worker.as_ref().unwrap().activity;
                    activity.timing[3]
                        .fetch_add(nanos(completion.finished_at.elapsed()), Ordering::AcqRel);
                    completion
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    // A disconnected producer cannot accept or finish new work.
                    // Keep admission charges until the actual thread has exited:
                    // channel loss alone must not stand in for payload retirement.
                    self.streaming.stopped = true;
                    self.fail_pending(StreamingError::WorkerStopped);
                    if self.streaming.worker.as_ref().is_some_and(|worker| {
                        worker.thread.as_ref().is_some_and(JoinHandle::is_finished)
                    }) {
                        self.streaming.worker = None;
                        self.streaming.operations.clear();
                        self.streaming.retired_operations.clear();
                    }
                    break;
                }
                _ => break,
            };
            count += 1;
            if self
                .streaming
                .retired_operations
                .remove(&completion.id)
                .is_some()
            {
                self.streaming.cancelled += 1;
                continue;
            }
            let Some(operation) = self.streaming.operations.remove(&completion.key) else {
                continue;
            };
            if operation.id != completion.id {
                self.streaming.operations.insert(completion.key, operation);
                continue;
            }
            if operation.cancel.load(Ordering::Acquire) || operation.consumers.is_empty() {
                self.streaming.cancelled += 1;
                continue;
            }
            match completion.result {
                Ok(mut decoded) => {
                    decoded.ref_count = operation.consumers.len() as u32;
                    for id in operation.consumers {
                        self.streaming.pending_cache_handles.remove(&id);
                        if let Some(h) = self.handles.get_mut(&id) {
                            h.state = RuntimeAssetLoadState::Ready;
                        }
                    }
                    if let Some(resident) = self.decoded_cache.get(&completion.key) {
                        decoded.ref_count = decoded.ref_count.saturating_add(resident.ref_count);
                    }
                    self.decoded_cache.insert(completion.key, decoded);
                    self.streaming.applied += 1;
                }
                Err(error) => {
                    for id in operation.consumers {
                        if let Some(h) = self.handles.get_mut(&id) {
                            h.state = RuntimeAssetLoadState::Failed;
                        }
                        self.streaming.errors.insert(id, error.clone());
                    }
                }
            }
        }
        let roots: Vec<_> = self.streaming.tickets.keys().copied().collect();
        for root in roots {
            let members = self.streaming.tickets.get(&root).unwrap();
            let failure = std::iter::once(&root)
                .chain(members.iter())
                .find_map(|id| self.streaming.errors.get(id))
                .cloned();
            if let Some(error) = failure {
                let handle = self.handles[&root].clone();
                let _ = self.release(&handle);
                self.handles.get_mut(&root).unwrap().state = RuntimeAssetLoadState::Failed;
                self.streaming.errors.insert(root, error);
                continue;
            }
            let ready = std::iter::once(&root).chain(members.iter()).all(|id| {
                !self.streaming.pending_cache_handles.contains(id)
                    && self
                        .cache_keys_by_handle
                        .get(id)
                        .is_some_and(|key| self.decoded_cache.contains_key(key))
            });
            let h = self.handles.get_mut(&root).unwrap();
            h.state = if ready {
                RuntimeAssetLoadState::Ready
            } else {
                RuntimeAssetLoadState::Loading
            };
            if ready {
                h.runtime_resource_id =
                    Some(format!("prepared:{}:{}", h.loader_kind, h.cooked_asset_id));
                self.streaming.tickets.remove(&root);
            }
        }
        count
    }

    pub(super) fn detach_stream_ticket(&mut self, id: u64) {
        self.streaming.tickets.remove(&id);
        self.streaming.errors.remove(&id);
        if let Some(key) = self.cache_keys_by_handle.get(&id) {
            let mut retired = false;
            if let Some(operation) = self.streaming.operations.get_mut(key) {
                operation.consumers.remove(&id);
                if operation.consumers.is_empty() {
                    operation.cancel.store(true, Ordering::Release);
                    retired = true;
                }
            }
            if retired {
                let operation = self.streaming.operations.remove(key).unwrap();
                self.streaming
                    .retired_operations
                    .insert(operation.id, operation);
            }
        }
    }

    pub(super) fn owns_decoded_reference(&mut self, id: u64) -> bool {
        !self.streaming.pending_cache_handles.remove(&id)
    }

    fn fail_pending(&mut self, error: StreamingError) {
        let roots: Vec<_> = self
            .streaming
            .tickets
            .keys()
            .filter_map(|id| self.handles.get(id).cloned())
            .collect();
        for root in roots {
            let _ = self.release(&root);
            // release appends this root after its dependency closure. It is
            // becoming Failed, so it must not leave a stale history entry that
            // could expire a later explicit release of the same handle.
            let released_id = self.released_handles.pop_back();
            debug_assert_eq!(released_id, Some(root.handle_id));
            self.handles.get_mut(&root.handle_id).unwrap().state = RuntimeAssetLoadState::Failed;
            self.streaming.errors.insert(root.handle_id, error.clone());
        }
    }

    pub(super) fn invalidate_pending_streams(&mut self) {
        self.fail_pending(StreamingError::RevisionChanged);
    }

    /// Bounded shutdown: false means an OS read is still outstanding. No World,
    /// cache or callback can be accessed after this loader is dropped.
    pub fn shutdown_streaming(&mut self, timeout: Duration) -> bool {
        self.fail_pending(StreamingError::Cancelled);
        self.streaming.stopped = true;
        let Some(worker) = &self.streaming.worker else {
            return true;
        };
        worker.stop.store(true, Ordering::Release);
        let started = Instant::now();
        while self.streaming.worker.as_ref().is_some_and(|worker| {
            worker
                .thread
                .as_ref()
                .is_some_and(|thread| !thread.is_finished())
        }) {
            self.poll_async();
            if started.elapsed() >= timeout {
                return false;
            }
            std::thread::yield_now();
        }
        self.streaming.worker = None;
        self.streaming.operations.clear();
        self.streaming.retired_operations.clear();
        true
    }
}

fn observe_parsed_value(
    value: &serde_json::Value,
    work: &Work,
    activity: &WorkerActivity,
    _cancelled: impl Fn() -> bool,
) {
    let _observation = activity.memory.observe_json(value, work.id);
    #[cfg(test)]
    if let Some(gate) = &work.gate {
        if let Some(serde_json::Value::String(name)) = value.get("name") {
            gate.decoded_backing_address
                .store(name.as_ptr() as usize, Ordering::Release);
            gate.parser_backing_bytes
                .store(name.capacity(), Ordering::Release);
        }
        gate.pause(5, work.id, &_cancelled);
    }
}

fn read_bounded(
    path: &std::path::Path,
    expected: usize,
    limit: usize,
    cancelled: impl Fn() -> bool,
    activity: &WorkerActivity,
    operation: u64,
    #[cfg(test)] gate: Option<&TestWorkerGate>,
) -> Result<ReadBuffer, StreamingError> {
    if expected > limit {
        return Err(StreamingError::BudgetExceeded);
    }
    let mut file = fs::File::open(path).map_err(|e| StreamingError::ReadFailed(e.to_string()))?;
    if file
        .metadata()
        .map_err(|e| StreamingError::ReadFailed(e.to_string()))?
        .len()
        != expected as u64
    {
        return Err(StreamingError::IntegrityMismatch);
    }
    let mut bytes = ReadBuffer::new(expected, operation, &activity.memory);
    #[cfg(test)]
    if let Some(gate) = gate {
        gate.pause(1, operation, &cancelled);
    }
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err(StreamingError::Cancelled);
        }
        let n = file
            .read(&mut chunk)
            .map_err(|e| StreamingError::ReadFailed(e.to_string()))?;
        if n == 0 {
            break;
        }
        if bytes.len().saturating_add(n) > expected {
            return Err(StreamingError::IntegrityMismatch);
        }
        bytes.bytes.extend_from_slice(&chunk[..n]);
    }
    if bytes.len() != expected {
        return Err(StreamingError::IntegrityMismatch);
    }
    Ok(bytes)
}

fn decode_envelope(size: usize, asset_type: &str) -> Result<usize, StreamingError> {
    size.checked_mul(if asset_type == "environment" {
        2
    } else if asset_type == "audio" {
        4
    } else {
        64
    })
    .ok_or(StreamingError::BudgetExceeded)
}

fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn decode_work(
    work: &Work,
    stop: &AtomicBool,
    activity: &WorkerActivity,
) -> Result<DecodedAsset, StreamingError> {
    let cancelled = || work.cancel.load(Ordering::Acquire) || stop.load(Ordering::Acquire);
    #[cfg(test)]
    if let Some(gate) = &work.gate {
        *gate.thread.lock().unwrap() = Some(std::thread::current().id());
        gate.pause(0, work.id, &cancelled);
    }
    if cancelled() {
        return Err(StreamingError::Cancelled);
    }
    let spec = &work.spec;
    let path = safe_join_runtime_package(&work.root, spec.cooked.path.as_ref().unwrap())
        .map_err(|_| StreamingError::UnsafePath)?;
    let read_started = Instant::now();
    let bytes = read_bounded(
        &path,
        spec.cooked.size.unwrap() as usize,
        work.budget.max_asset_bytes,
        cancelled,
        activity,
        work.id,
        #[cfg(test)]
        work.gate.as_deref(),
    )?;
    activity.timing[4].fetch_add(nanos(read_started.elapsed()), Ordering::AcqRel);
    // Reserve an input-proportional envelope before serde/audio allocation. This
    // deliberately rejects large JSON assets instead of measuring only after decode.
    let mut decode_envelope = decode_envelope(bytes.len(), &spec.record.asset_type)?;
    let bounded_numeric = decode_envelope > work.budget.max_decode_bytes
        && matches!(
            spec.record.asset_type.as_str(),
            "mesh" | "collision3d" | "character3d" | "prefab"
        );
    if decode_envelope > work.budget.max_decode_bytes && !bounded_numeric {
        return Err(StreamingError::BudgetExceeded);
    }
    let digest = crate::canonical_digest::sha256_prefixed(&bytes);
    if spec.record.asset_type != "texture"
        && (spec.cooked.hash.as_deref() != Some(&digest)
            || spec.record.hash.as_deref() != Some(&digest))
    {
        return Err(StreamingError::IntegrityMismatch);
    }
    let decode_error = |e: serde_json::Error| StreamingError::DecodeFailed(e.to_string());
    #[cfg(test)]
    if let Some(gate) = &work.gate {
        gate.pause(2, work.id, &cancelled);
    }
    let mut extra = 0usize;
    let mut audio_clip = None;
    let payload = match spec.record.asset_type.as_str() {
        "environment" => {
            let map = crate::environment_lighting::EnvironmentMap::from_bytes(&bytes)
                .map_err(StreamingError::DecodeFailed)?;
            extra = map.words().len() * 4;
            Some(RuntimeDecodedPayload::Environment(map))
        }
        "mesh" | "material" => {
            if bounded_numeric {
                let (asset, charge) =
                    geometry_decode::decode_mesh(&bytes, work.budget.max_decode_bytes)?;
                decode_envelope = charge;
                extra = charge;
                Some(RuntimeDecodedPayload::Render(asset))
            } else {
                let text = std::str::from_utf8(&bytes)
                    .map_err(|e| StreamingError::DecodeFailed(e.to_string()))?;
                let asset = crate::render_contract::RenderContract::<
                    crate::render_contract::CookedRenderAsset,
                >::decode(text)
                .map_err(|e| StreamingError::DecodeFailed(format!("{e:?}")))?
                .data;
                extra = serde_json::to_vec(&asset).map_err(decode_error)?.len();
                Some(RuntimeDecodedPayload::Render(asset))
            }
        }
        "prefab" => {
            if bounded_numeric {
                let (prefab, charge) =
                    prefab_decode::decode_prefab(&bytes, work.budget.max_decode_bytes)?;
                decode_envelope = charge;
                extra = charge;
                Some(RuntimeDecodedPayload::Prefab(prefab))
            } else {
                let value: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(decode_error)?;
                observe_parsed_value(&value, work, activity, cancelled);
                let document =
                    crate::spatial_contract::decode_runtime_prefab(value).map_err(|e| {
                        StreamingError::DecodeFailed(format!("{}: {}", e.code, e.message))
                    })?;
                let prefab = match document {
                    crate::spatial_contract::RuntimePrefabDocument::Legacy(prefab) => prefab,
                    crate::spatial_contract::RuntimePrefabDocument::Spatial(prefab) => {
                        prefab.into()
                    }
                };
                extra = bytes.len();
                Some(RuntimeDecodedPayload::Prefab(prefab))
            }
        }
        "scene" => {
            let document =
                crate::spatial_contract::decode_runtime_scene_observed(&bytes, |value| {
                    observe_parsed_value(value, work, activity, cancelled)
                })
                .map_err(|e| StreamingError::DecodeFailed(format!("{}: {}", e.code, e.message)))?;
            let scene = match document {
                crate::spatial_contract::RuntimeSceneDocument::Legacy(scene) => scene,
                crate::spatial_contract::RuntimeSceneDocument::Spatial(scene) => scene.into(),
            };
            extra = bytes.len();
            Some(RuntimeDecodedPayload::Scene(scene))
        }
        "texture" => {
            let meta: crate::runtime_package::CookedTextureAsset =
                serde_json::from_slice(&bytes).map_err(decode_error)?;
            crate::runtime_texture::validate_metadata(
                &meta,
                &spec.record.asset_id,
                &spec.record.cooked_asset_id,
            )
            .map_err(|e| StreamingError::DecodeFailed(format!("{e:?}")))?;
            let expected = crate::runtime_texture::texture_mip_byte_len(
                &meta.format,
                meta.width,
                meta.height,
                meta.mip_count,
            )
            .ok_or(StreamingError::BudgetExceeded)?;
            if expected != meta.byte_length
                || expected > work.budget.max_decode_bytes.saturating_sub(decode_envelope)
            {
                return Err(StreamingError::BudgetExceeded);
            }
            let pixels = safe_join_runtime_package(&work.root, &meta.pixel_data_path)
                .map_err(|_| StreamingError::UnsafePath)?;
            let read_started = Instant::now();
            let rgba8 = read_bounded(
                &pixels,
                expected,
                work.budget.max_decode_bytes,
                cancelled,
                activity,
                work.id,
                #[cfg(test)]
                work.gate.as_deref(),
            )?;
            activity.timing[4].fetch_add(nanos(read_started.elapsed()), Ordering::AcqRel);
            use sha2::{Digest, Sha256};
            let mut texture_digest = Sha256::new();
            texture_digest.update(&bytes);
            texture_digest.update(&rgba8);
            let texture_digest = format!("sha256:{:x}", texture_digest.finalize());
            if spec.cooked.hash.as_deref() != Some(&texture_digest)
                || spec.record.hash.as_deref() != Some(&texture_digest)
            {
                return Err(StreamingError::IntegrityMismatch);
            }
            extra = rgba8.len();
            Some(RuntimeDecodedPayload::Texture(
                crate::runtime_texture::VerifiedTexturePayload::new(
                    crate::runtime_texture::RuntimeTexturePayload {
                        asset_id: meta.asset_id,
                        cooked_asset_id: meta.cooked_asset_id,
                        width: meta.width,
                        height: meta.height,
                        format: meta.format,
                        color_space: meta.color_space,
                        mip_count: meta.mip_count,
                        rgba8: rgba8.into_vec().into(),
                        sampler: meta.sampler,
                        source_hash: meta.source_hash,
                    },
                ),
            ))
        }
        "audio" => {
            let clip =
                crate::audio::decode_audio_wav(&bytes).map_err(StreamingError::DecodeFailed)?;
            extra = clip
                .samples
                .len()
                .checked_mul(4)
                .ok_or(StreamingError::BudgetExceeded)?;
            audio_clip = Some(Arc::new(clip));
            None
        }
        "particle-effect" => None,
        "vegetation3d" => {
            let value = crate::spatial_contract::strict_json(&bytes)
                .map_err(|e| StreamingError::DecodeFailed(e.to_string()))?;
            let asset: crate::terrain3d::VegetationAsset3D =
                serde_json::from_value(value).map_err(decode_error)?;
            if asset.asset_id != spec.record.asset_id || asset.asset_guid != spec.record.asset_guid
            {
                return Err(StreamingError::IntegrityMismatch);
            }
            asset
                .validate()
                .map_err(|e| StreamingError::DecodeFailed(e.to_string()))?;
            None
        }
        "character3d" => {
            if bounded_numeric {
                let (_, charge) =
                    geometry_decode::decode_character(&bytes, work.budget.max_decode_bytes)?;
                decode_envelope = charge;
            } else {
                let asset: crate::character_animation::CharacterAsset =
                    serde_json::from_slice(&bytes).map_err(decode_error)?;
                asset
                    .validate()
                    .map_err(|e| StreamingError::DecodeFailed(e.to_string()))?;
            }
            None
        }
        "collision3d" => {
            if bounded_numeric {
                let (_, charge) =
                    geometry_decode::decode_collision(&bytes, work.budget.max_decode_bytes)?;
                decode_envelope = charge;
            } else {
                let asset: crate::physics_3d::CookedCollision3D =
                    serde_json::from_slice(&bytes).map_err(decode_error)?;
                asset
                    .validate()
                    .map_err(|e| StreamingError::DecodeFailed(e.to_string()))?;
            }
            None
        }
        "navigation3d" => {
            let asset: crate::navigation_3d::CookedNavigation3D =
                serde_json::from_slice(&bytes).map_err(decode_error)?;
            asset
                .validate()
                .map_err(|e| StreamingError::DecodeFailed(e.to_string()))?;
            None
        }
        _ => {
            return Err(StreamingError::DecodeFailed(
                "unsupported async asset type".into(),
            ))
        }
    };
    let decoded_charge = if spec.record.asset_type == "texture" {
        extra.saturating_add(decode_envelope)
    } else {
        extra.max(decode_envelope)
    };
    if decoded_charge > work.budget.max_decode_bytes
        || decoded_charge > spec.reservation.saturating_sub(bytes.len())
    {
        return Err(StreamingError::BudgetExceeded);
    }
    if cancelled() {
        return Err(StreamingError::Cancelled);
    }
    // Vegetation validation discards its temporary JSON/typed values before
    // publication. Keep the full decode reservation above, then retain only
    // backing that actually survives in the cache.
    let retained_decode_bytes = if spec.record.asset_type == "vegetation3d" && payload.is_none() {
        0
    } else {
        decoded_charge
    };
    let decoded = DecodedAsset {
        cooked_asset_id: spec.record.cooked_asset_id.clone(),
        asset_type: spec.record.asset_type.clone(),
        bytes_len: bytes.len().saturating_add(retained_decode_bytes),
        source_debug: path.display().to_string(),
        ref_count: 0,
        audio_clip,
        bytes: Some(Arc::from(bytes.into_vec())),
        payload: payload.map(Arc::new),
    };
    let observation = activity.memory.observe_decoded(&decoded, work.id);
    #[cfg(test)]
    if let Some(gate) = &work.gate {
        if let Some(clip) = &decoded.audio_clip {
            gate.decoded_backing_address
                .store(clip.samples.as_ptr() as usize, Ordering::Release);
        }
        // Phase three is already reserved by the completion-mailbox fixture.
        gate.pause(4, work.id, &cancelled);
    }
    drop(observation);
    Ok(decoded)
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;

    fn reference(i: usize) -> RuntimeAssetRef {
        RuntimeAssetRef {
            id: format!("sound-{i}"),
            asset_type: "audio".into(),
            guid: None,
            sub_asset: None,
        }
    }
    fn wait(mut ready: impl FnMut() -> bool) {
        let start = Instant::now();
        while !ready() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "real worker checkpoint timed out"
            );
            std::thread::yield_now();
        }
    }
    fn fixture() -> (RuntimeAssetLoader, PathBuf) {
        let (mut loader, root) = super::super::tests::real_streaming_loader();
        let record = loader.index.record_by_guid("guid-audio").unwrap().clone();
        let cooked = loader
            .index
            .cooked_asset(&record.cooked_asset_id)
            .unwrap()
            .clone();
        let records = (0..10)
            .map(|i| {
                let mut r = record.clone();
                r.asset_id = format!("sound-{i}");
                r.asset_guid = format!("guid-audio-{i}");
                r
            })
            .collect();
        loader.index = RuntimeAssetIndex::new(records, vec![cooked]);
        (loader, root)
    }

    #[test]
    fn pc_ready_queue_includes_blocked_sender_and_stays_within_128_real_completions() {
        let (mut loader, root) = super::super::tests::real_streaming_loader();
        let record = loader.index.record_by_guid("guid-audio").unwrap().clone();
        let cooked = loader
            .index
            .cooked_asset(&record.cooked_asset_id)
            .unwrap()
            .clone();
        let records = (0..130)
            .map(|i| {
                let mut r = record.clone();
                r.asset_id = format!("sound-{i}");
                r.asset_guid = format!("guid-audio-{i}");
                r
            })
            .collect();
        loader.index = RuntimeAssetIndex::new(records, vec![cooked]);
        let mut budget = loader.streaming_budget();
        budget.max_operations = 256;
        loader.set_streaming_budget(budget).unwrap();
        let gate = Arc::new(TestWorkerGate::default());
        loader.streaming.gate = Some(gate.clone());
        let handles = (0..130)
            .map(|i| loader.request_async(&reference(i)).unwrap())
            .collect::<Vec<_>>();
        wait(|| {
            gate.completion_blocked.load(Ordering::Acquire)
                || loader
                    .streaming
                    .worker
                    .as_ref()
                    .unwrap()
                    .activity
                    .ready
                    .load(Ordering::Acquire)
                    >= 130
        });
        let held = loader
            .streaming
            .worker
            .as_ref()
            .unwrap()
            .activity
            .ready
            .load(Ordering::Acquire);
        let before = loader.streaming_snapshot();
        assert_eq!(before.pending_operations, 130);
        assert_eq!(before.applied_operations, 0);
        let mut peak = held;
        wait(|| {
            peak = peak.max(
                loader
                    .streaming
                    .worker
                    .as_ref()
                    .unwrap()
                    .activity
                    .ready
                    .load(Ordering::Acquire),
            );
            loader.poll_async();
            handles
                .iter()
                .all(|h| loader.get_handle_state(h) == Some(RuntimeAssetLoadState::Ready))
        });
        assert_eq!(loader.streaming_snapshot().applied_operations, 130);
        assert_eq!(loader.streaming_snapshot().pending_operations, 0);
        for h in handles {
            loader.release(&h).unwrap();
        }
        assert!(loader.shutdown_streaming(Duration::from_secs(2)));
        assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
        assert_eq!(loader.streaming_snapshot().reserved_bytes, 0);
        fs::remove_dir_all(root).unwrap();
        assert!(held <= 128 && peak <= 128,
            "PC completed payloads exceed frozen ready bound, including blocked sender: {held}/{peak}");
    }

    #[test]
    fn f02_ten_reading_decoding_and_completed_cancellations_preserve_new_generations_in_three_orders(
    ) {
        // Reorder real worker completions at the mailbox boundary. No result,
        // generation, payload or GPU completion is fabricated by this test.
        for phase in [1, 2, 3] {
            for order in 0..3 {
                let (mut loader, root) = fixture();
                let gate = Arc::new(TestWorkerGate::default());
                gate.phase.store(phase, Ordering::Release);
                gate.blocked.store(true, Ordering::Release);
                let mut olds = Vec::new();
                let mut currents = Vec::new();
                for i in 0..10 {
                    loader.streaming.gate = Some(gate.clone());
                    let old = loader.request_async(&reference(i)).unwrap();
                    let operation = loader.streaming.next_operation;
                    if phase < 3 {
                        wait(|| gate.operation.load(Ordering::Acquire) == operation);
                    } else {
                        wait(|| {
                            loader
                                .streaming
                                .worker
                                .as_ref()
                                .unwrap()
                                .activity
                                .ready
                                .load(Ordering::Acquire)
                                == 2 * i + 1
                        });
                    }
                    loader.release(&old).unwrap();
                    loader.streaming.gate = None;
                    let current = loader.request_async(&reference(i)).unwrap();
                    assert_ne!(old.generation, current.generation);
                    olds.push(old);
                    currents.push(current);
                }
                wait(|| {
                    loader
                        .streaming
                        .worker
                        .as_ref()
                        .unwrap()
                        .activity
                        .ready
                        .load(Ordering::Acquire)
                        == 20
                });
                assert_eq!(loader.streaming_snapshot().pending_operations, 20);
                assert_eq!(loader.streaming.retired_operations.len(), 10);
                let charged = loader.streaming_snapshot().reserved_bytes;
                assert!(charged > 0);
                let worker = loader.streaming.worker.as_mut().unwrap();
                let mut completed = (0..20)
                    .map(|_| worker.receiver.try_recv().unwrap())
                    .collect::<Vec<_>>();
                if phase == 3 {
                    assert!(completed.iter().all(|c| c.result.is_ok()));
                }
                match order {
                    1 => completed.reverse(),
                    2 => completed.sort_by_key(|c| (c.id % 2, std::cmp::Reverse(c.id))),
                    _ => (),
                }
                let (sender, receiver) = mpsc::sync_channel(20);
                for c in completed {
                    sender.send(c).unwrap();
                }
                worker.receiver = receiver;
                // Keep this sender alive until all real results are consumed.
                while loader.streaming_snapshot().pending_operations != 0 {
                    assert!(loader.poll_async() > 0);
                }
                assert_eq!(loader.streaming_snapshot().reserved_bytes, 0);
                assert_eq!(loader.streaming_snapshot().applied_operations, 10);
                assert_eq!(loader.streaming_snapshot().cancelled_results, 10);
                for (old, current) in olds.iter().zip(&currents) {
                    assert_eq!(
                        loader.get_handle_state(old),
                        Some(RuntimeAssetLoadState::Released)
                    );
                    assert_eq!(
                        loader.get_handle_state(current),
                        Some(RuntimeAssetLoadState::Ready)
                    );
                    assert!(loader.get_audio_clip(current).is_some());
                    assert!(loader.get_audio_clip(old).is_none());
                }
                for current in currents {
                    loader.release(&current).unwrap();
                }
                assert_eq!(loader.streaming_snapshot().cpu_resident_bytes, 0);
                assert!(loader.shutdown_streaming(Duration::from_secs(2)));
                drop(sender);
                fs::remove_dir_all(root).unwrap();
            }
        }
    }

    #[test]
    fn cancelled_work_keeps_operation_byte_and_reconfiguration_limits_until_completion() {
        for limit in ["operations", "inFlight", "resident"] {
            let (mut loader, root) = fixture();
            let reservation = loader.streaming_specs(&reference(0)).unwrap()[0].reservation;
            let mut budget = loader.streaming_budget();
            match limit {
                "operations" => budget.max_operations = 1,
                "inFlight" => {
                    budget.max_asset_bytes = reservation;
                    budget.max_in_flight_bytes = reservation;
                }
                "resident" => {
                    budget.max_asset_bytes = reservation;
                    budget.max_cpu_resident_bytes = reservation;
                }
                _ => unreachable!(),
            }
            loader.set_streaming_budget(budget).unwrap();
            let gate = Arc::new(TestWorkerGate::default());
            gate.blocked.store(true, Ordering::Release);
            loader.streaming.gate = Some(gate.clone());
            let old = loader.request_async(&reference(0)).unwrap();
            wait(|| gate.entered.load(Ordering::Acquire));
            loader.release(&old).unwrap();
            let before = loader.streaming_snapshot();
            assert_eq!(before.pending_operations, 1);
            assert_eq!(before.reserved_bytes, reservation);
            assert_eq!(
                loader.request_async(&reference(0)),
                Err(StreamingError::BudgetExceeded)
            );
            assert_eq!(
                loader.streaming_snapshot().dispatched_operations,
                before.dispatched_operations
            );
            assert_eq!(
                loader.streaming_snapshot().reserved_bytes,
                before.reserved_bytes
            );
            assert_eq!(
                loader.set_streaming_budget(budget),
                Err(StreamingError::Busy)
            );
            gate.blocked.store(false, Ordering::Release);
            wait(|| {
                loader.poll_async();
                loader.streaming_snapshot().pending_operations == 0
            });
            assert_eq!(loader.streaming_snapshot().reserved_bytes, 0);
            let new = loader.request_async(&reference(0)).unwrap();
            wait(|| {
                loader.poll_async();
                loader.get_handle_state(&new) == Some(RuntimeAssetLoadState::Ready)
            });
            loader.release(&new).unwrap();
            assert!(loader.shutdown_streaming(Duration::from_secs(2)));
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::resource_memory::ResourceMemorySample;
    fn capture(memory: &PayloadMemory) -> ResourceMemorySample {
        let mut s = ResourceMemorySample::new(1, 1, 0);
        memory.capture(&mut s);
        s
    }
    #[test]
    fn read_buffer_tracks_capacity_and_removes_record_before_arc_conversion_or_drop() {
        let memory = Arc::new(PayloadMemory::default());
        capture(&memory);
        for size in [1, 8193, 65537] {
            let mut source = ReadBuffer::new(size, 7, &memory);
            source.bytes.resize(size, 23);
            let identity = format!("cpu-storage:{:x}", source.as_ptr() as usize);
            let sample = capture(&memory);
            let a = sample
                .allocations
                .iter()
                .find(|a| a.allocation_id == identity)
                .unwrap();
            assert_eq!(a.bytes, source.capacity() as u64);
            assert_eq!(a.owner, "assets");
            assert!(a.roles.contains("decodeWork"));
            let bytes = source.into_vec();
            assert!(!capture(&memory)
                .allocations
                .iter()
                .any(|a| a.allocation_id == identity));
            let immutable: Arc<[u8]> = bytes.into();
            assert_eq!(immutable.len(), size);
            let other = ReadBuffer::new(size, 8, &memory);
            assert_eq!(capture(&memory).capacities["loader.observedPayloads"], 1);
            drop(other);
            assert_eq!(capture(&memory).capacities["loader.observedPayloads"], 0);
        }
    }
    #[test]
    fn disabled_read_observation_never_allocates_metadata_or_retroactively_scans() {
        let memory = Arc::new(PayloadMemory::default());
        let source = ReadBuffer::new(8193, 1, &memory);
        assert!(memory.entries.lock().unwrap().is_none());
        assert!(source.observation.is_none());
        assert_eq!(capture(&memory).capacities["loader.observedPayloads"], 0);
        drop(source);
        assert_eq!(capture(&memory).capacities["loader.observedPayloads"], 0);
    }
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package; F10 shared tickets and real worker only"]
fn bm11_original_bm10_three_shared_consumers_cancel_without_reviving_old_tickets() {
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

    let original = loader
        .asset_loader()
        .asset_index()
        .record_by_guid("guid-static.prefab.0")
        .unwrap()
        .clone();
    let cooked = loader
        .asset_loader()
        .asset_index()
        .cooked_asset(&original.cooked_asset_id)
        .unwrap()
        .clone();
    let mut records = Vec::new();
    let mut cooked_records = Vec::new();
    for id in ["bm11-f10-shared", "bm11-f10-all-cancelled"] {
        let mut record = original.clone();
        record.asset_id = id.into();
        record.asset_guid = id.into();
        record.cooked_asset_id = format!("cooked-{id}");
        let mut cooked = cooked.clone();
        cooked.cooked_asset_id = record.cooked_asset_id.clone();
        records.push(record);
        cooked_records.push(cooked);
    }
    loader
        .asset_loader_mut()
        .mount_patch_index(RuntimeAssetIndex::new(records, cooked_records));

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

    fn wait(mut ready: impl FnMut() -> bool) {
        let started = Instant::now();
        while !ready() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "real worker did not reach the bounded gate/mailbox"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let mut manager = crate::render_resource::RenderResourceManager::new();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f10",
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
    let entities = world.entity_count();
    eprintln!("BM11 F10 original population ready: frame={frame} entities={entities} retainedDrawRecords={retained_draw_records} setup={:?}",started.elapsed());
    let fault_started = Instant::now();
    {
        let assets = loader.asset_loader_mut();
        let before = assets.streaming_snapshot();
        let cache_count = assets.decoded_cache_len();
        assert_eq!((before.pending_operations, before.reserved_bytes), (0, 0));
        let reference = RuntimeAssetRef {
            id: "bm11-f10-shared".into(),
            asset_type: "prefab".into(),
            guid: Some("bm11-f10-shared".into()),
            sub_asset: None,
        };
        let gate = Arc::new(TestWorkerGate::default());
        gate.phase.store(2, Ordering::Release);
        gate.blocked.store(true, Ordering::Release);
        assets.streaming.gate = Some(gate.clone());
        let tickets: Vec<_> = (0..3)
            .map(|_| assets.request_async(&reference).unwrap())
            .collect();
        wait(|| gate.entered.load(Ordering::Acquire));
        let joined = assets.streaming_snapshot();
        assert_eq!(joined.pending_operations, 1);
        assert_eq!(joined.pending_tickets, 3);
        assert_eq!(
            joined.dispatched_operations - before.dispatched_operations,
            1
        );
        let operation = assets.streaming.operations.values().next().unwrap();
        assert_eq!(operation.consumers.len(), 3);
        assert!(!operation.cancel.load(Ordering::Acquire));
        assert!(tickets
            .windows(2)
            .all(|p| p[0].handle_id != p[1].handle_id && p[0].generation != p[1].generation));
        assets.release(&tickets[0]).unwrap();
        let operation = assets.streaming.operations.values().next().unwrap();
        assert_eq!(operation.consumers.len(), 2);
        assert!(!operation.cancel.load(Ordering::Acquire));
        gate.blocked.store(false, Ordering::Release);
        wait(|| {
            assets
                .streaming
                .worker
                .as_ref()
                .unwrap()
                .activity
                .ready
                .load(Ordering::Acquire)
                > 0
        });
        assets.poll_async();
        assert_eq!(
            assets.get_handle_state(&tickets[0]),
            Some(RuntimeAssetLoadState::Released)
        );
        assert!(assets.get_asset_bytes(&tickets[0]).is_none());
        for t in &tickets[1..] {
            assert_eq!(
                assets.get_handle_state(t),
                Some(RuntimeAssetLoadState::Ready)
            );
        }
        let bytes_a = assets.get_asset_bytes(&tickets[1]).unwrap();
        let bytes_b = assets.get_asset_bytes(&tickets[2]).unwrap();
        assert!(!bytes_a.is_empty());
        assert!(Arc::ptr_eq(&bytes_a, &bytes_b));
        drop(bytes_a);
        drop(bytes_b);
        assets.release(&tickets[1]).unwrap();
        assert_eq!(
            assets.get_handle_state(&tickets[2]),
            Some(RuntimeAssetLoadState::Ready)
        );
        assert!(assets.get_asset_bytes(&tickets[2]).is_some());
        assets.release(&tickets[2]).unwrap();
        assets.poll_async();
        let after = assets.streaming_snapshot();
        assert_eq!(
            (
                after.pending_operations,
                after.pending_tickets,
                after.reserved_bytes,
                after.cpu_resident_bytes
            ),
            (0, 0, 0, before.cpu_resident_bytes)
        );
        assert_eq!(assets.decoded_cache_len(), cache_count);
        assert_eq!(after.applied_operations - before.applied_operations, 1);
        eprintln!("BM11 F10 shared: three tickets one real operation; cancelled first never Ready, two Ready share one Arc, second cancellation preserves last; applied=1 cache/pending/reservation baseline");
        let reference = RuntimeAssetRef {
            id: "bm11-f10-all-cancelled".into(),
            asset_type: "prefab".into(),
            guid: Some("bm11-f10-all-cancelled".into()),
            sub_asset: None,
        };
        let gate = Arc::new(TestWorkerGate::default());
        gate.phase.store(2, Ordering::Release);
        gate.blocked.store(true, Ordering::Release);
        assets.streaming.gate = Some(gate.clone());
        let before = assets.streaming_snapshot();
        let tickets: Vec<_> = (0..3)
            .map(|_| assets.request_async(&reference).unwrap())
            .collect();
        wait(|| gate.entered.load(Ordering::Acquire));
        assert_eq!(assets.streaming_snapshot().pending_operations, 1);
        assert_eq!(
            assets
                .streaming
                .operations
                .values()
                .next()
                .unwrap()
                .consumers
                .len(),
            3
        );
        gate.blocked.store(false, Ordering::Release);
        wait(|| {
            assets
                .streaming
                .worker
                .as_ref()
                .unwrap()
                .activity
                .ready
                .load(Ordering::Acquire)
                > 0
        });
        // A real, successful decoder result exists, but none has been published.
        assert!(tickets
            .iter()
            .all(|t| assets.get_handle_state(t) == Some(RuntimeAssetLoadState::Loading)));
        for t in &tickets {
            assets.release(t).unwrap();
        }
        assert_eq!(assets.streaming.retired_operations.len(), 1);
        assets.poll_async();
        for t in &tickets {
            assert_eq!(
                assets.get_handle_state(t),
                Some(RuntimeAssetLoadState::Released)
            );
            assert!(assets.get_asset_bytes(t).is_none());
        }
        let after = assets.streaming_snapshot();
        assert_eq!(after.cancelled_results - before.cancelled_results, 1);
        assert_eq!(after.applied_operations, before.applied_operations);
        assert_eq!(
            (
                after.pending_operations,
                after.pending_tickets,
                after.reserved_bytes,
                after.cpu_resident_bytes
            ),
            (0, 0, 0, before.cpu_resident_bytes)
        );
        assert_eq!(assets.decoded_cache_len(), cache_count);
        assets.streaming.gate = None;
        eprintln!("BM11 F10 all cancelled after actual decode/completion but before publication: cancelledResult=1 appliedDelta=0 old tickets Released/no bytes; cache/pending/reservation baseline");
    }
    assert_eq!(world.entity_count(), entities);
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
    assert_eq!(world.entity_count(), entities);
    eprintln!("BM11 original BM10 F10 terminal: frame={frame} entities={entities} retainedDrawRecords={retained_draw_records} nineActive=9 populationExact/allDomainsUpdated=true; two specified completion orders, elapsed={:?}",fault_started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package; actual decode worker failure only"]
fn bm11_original_bm10_decode_worker_loss_terminates_pending_without_destroying_active_world() {
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

    let original = loader
        .asset_loader()
        .asset_index()
        .record_by_guid("guid-static.prefab.0")
        .unwrap()
        .clone();
    let cooked = loader
        .asset_loader()
        .asset_index()
        .cooked_asset(&original.cooked_asset_id)
        .unwrap()
        .clone();
    let mut records = Vec::new();
    let mut cooked_records = Vec::new();
    for id in ["bm11-f06-decoder", "bm11-f06-queued"] {
        let mut record = original.clone();
        record.asset_id = id.into();
        record.asset_guid = id.into();
        record.cooked_asset_id = format!("cooked-{id}");
        let mut cooked = cooked.clone();
        cooked.cooked_asset_id = record.cooked_asset_id.clone();
        records.push(record);
        cooked_records.push(cooked);
    }
    loader
        .asset_loader_mut()
        .mount_patch_index(RuntimeAssetIndex::new(records, cooked_records));

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

    let mut manager = crate::render_resource::RenderResourceManager::new();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f06-worker",
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
    let entities = world.entity_count();
    let witness_id = world.entity_ids().into_iter().next().unwrap().clone();
    let witness = world.runtime_id_for_source(&witness_id).unwrap();
    eprintln!("BM11 F06 original population ready: frame={frame} entities={entities} retainedDrawRecords={retained_draw_records} setup={:?}",started.elapsed());
    let fault_started = Instant::now();
    {
        let assets = loader.asset_loader_mut();
        let before = assets.streaming_snapshot();
        let cache_count = assets.decoded_cache_len();
        assert_eq!((before.pending_operations, before.reserved_bytes), (0, 0));
        let gate = Arc::new(TestWorkerGate::default());
        gate.phase.store(2, Ordering::Release);
        gate.blocked.store(true, Ordering::Release);
        assets.streaming.gate = Some(gate.clone());
        let reference = RuntimeAssetRef {
            id: "bm11-f06-decoder".into(),
            asset_type: "prefab".into(),
            guid: Some("bm11-f06-decoder".into()),
            sub_asset: None,
        };
        let first = assets.request_async(&reference).unwrap();
        while !gate.entered.load(Ordering::Acquire) {
            assert!(fault_started.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(1));
        }
        let queued_reference = RuntimeAssetRef {
            id: "bm11-f06-queued".into(),
            asset_type: "prefab".into(),
            guid: Some("bm11-f06-queued".into()),
            sub_asset: None,
        };
        let second = assets.request_async(&queued_reference).unwrap();
        assert_eq!(assets.streaming_snapshot().pending_operations, 2);
        assert_eq!(assets.streaming_snapshot().pending_tickets, 2);
        assert!(assets.streaming_snapshot().reserved_bytes > 0);
        assert!(!assets
            .streaming
            .worker
            .as_ref()
            .unwrap()
            .thread
            .as_ref()
            .unwrap()
            .is_finished());
        gate.panic_on_resume.store(true, Ordering::Release);
        gate.blocked.store(false, Ordering::Release);
        while assets.streaming_error(&first).is_none()
            || assets.streaming_snapshot().pending_operations != 0
        {
            assets.poll_async();
            assert!(fault_started.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(1));
        }
        for t in [&first, &second] {
            assert_eq!(
                assets.streaming_error(t),
                Some(&StreamingError::WorkerStopped)
            );
            assert_eq!(
                assets.get_handle_state(t),
                Some(RuntimeAssetLoadState::Failed)
            );
            assert!(assets.get_asset_bytes(t).is_none());
        }
        assert_eq!(
            assets.request_async(&reference),
            Err(StreamingError::WorkerStopped)
        );
        assert!(
            assets.streaming.worker.is_none(),
            "retirement must follow the real worker exit"
        );
        let after = assets.streaming_snapshot();
        assert_eq!(
            (
                after.pending_operations,
                after.pending_tickets,
                after.reserved_bytes,
                after.cpu_resident_bytes
            ),
            (0, 0, 0, before.cpu_resident_bytes)
        );
        assert_eq!(assets.decoded_cache_len(), cache_count);
        assert!(assets.shutdown_streaming(Duration::from_secs(2)));
        for t in [&first, &second] {
            assert_eq!(
                assets.streaming_error(t),
                Some(&StreamingError::WorkerStopped)
            );
            assets.release(t).unwrap();
        }
        assets.streaming.gate = None;
        eprintln!("BM11 F06 actual decode worker panic at phase2 with one task decoding and one queued: two Failed/WorkerStopped, new admission closed, actual thread exited and owner retired, cache/pending/reservation baseline; bounded={:?}",fault_started.elapsed());
    }
    assert_eq!(world.entity_count(), entities);
    assert_eq!(world.runtime_id_for_source(&witness_id), Some(witness));
    assert!(world.try_resolve_runtime_entity(witness).is_ok());
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
    assert_eq!(world.entity_count(), entities);
    eprintln!("BM11 original BM10 F06 decoder worker terminal: frame={frame} entities={entities} retainedDrawRecords={retained_draw_records} nineActive=9 populationExact/allDomainsUpdated=true oldHandleValid; no decoder recovery or render/device claim; elapsed={:?}",fault_started.elapsed());
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package; F02 CPU phases only, no GPU UploadQueued claim"]
fn bm11_original_bm10_ten_cpu_cancellations_keep_new_generations_in_three_orders() {
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

    let original = loader
        .asset_loader()
        .asset_index()
        .record_by_guid("guid-static.prefab.0")
        .unwrap()
        .clone();
    let cooked = loader
        .asset_loader()
        .asset_index()
        .cooked_asset(&original.cooked_asset_id)
        .unwrap()
        .clone();
    let mut records = Vec::new();
    let mut cooked_records = Vec::new();
    for i in 0..10 {
        let id = format!("bm11-f02-original-prefab-{i}");
        let mut record = original.clone();
        record.asset_id = id.clone();
        record.asset_guid = id;
        record.cooked_asset_id = format!("cooked-{}", record.asset_id);
        let mut cooked = cooked.clone();
        cooked.cooked_asset_id = record.cooked_asset_id.clone();
        records.push(record);
        cooked_records.push(cooked);
    }
    loader
        .asset_loader_mut()
        .mount_patch_index(RuntimeAssetIndex::new(records, cooked_records));

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

    fn wait(mut ready: impl FnMut() -> bool) {
        let started = Instant::now();
        while !ready() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "real worker boundary timed out"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn reference(i: usize) -> RuntimeAssetRef {
        let id = format!("bm11-f02-original-prefab-{i}");
        RuntimeAssetRef {
            id: id.clone(),
            asset_type: "prefab".into(),
            guid: Some(id),
            sub_asset: None,
        }
    }
    let mut manager = crate::render_resource::RenderResourceManager::new();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let geometry = cache.snapshot(
        frame,
        host.render_scene(),
        "bm11-original-bm10-f02-cpu",
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
    let entities = world.entity_count();
    let witness_id = world.entity_ids().into_iter().next().unwrap().clone();
    let witness = world.runtime_id_for_source(&witness_id).unwrap();
    eprintln!("BM11 F02 CPU original population ready: frame={frame} entities={entities} retainedDrawRecords={retained_draw_records} setup={:?}",started.elapsed());
    let fault_started = Instant::now();
    {
        let assets = loader.asset_loader_mut();
        let original = assets.streaming_snapshot();
        let cache_count = assets.decoded_cache_len();
        assert_eq!(
            (
                original.pending_operations,
                original.pending_tickets,
                original.reserved_bytes
            ),
            (0, 0, 0)
        );
        for phase in [1, 2, 3] {
            for order in 0..3 {
                let before = assets.streaming_snapshot();
                let gate = Arc::new(TestWorkerGate::default());
                gate.phase.store(phase, Ordering::Release);
                gate.blocked.store(true, Ordering::Release);
                let mut olds = Vec::new();
                let mut currents = Vec::new();
                assert_eq!(
                    assets
                        .streaming
                        .worker
                        .as_ref()
                        .unwrap()
                        .activity
                        .ready
                        .load(Ordering::Acquire),
                    0
                );
                for i in 0..10 {
                    assets.streaming.gate = Some(gate.clone());
                    let old = assets.request_async(&reference(i)).unwrap();
                    let operation = assets.streaming.next_operation;
                    if phase < 3 {
                        wait(|| gate.operation.load(Ordering::Acquire) == operation);
                    } else {
                        wait(|| {
                            assets
                                .streaming
                                .worker
                                .as_ref()
                                .unwrap()
                                .activity
                                .ready
                                .load(Ordering::Acquire)
                                == 2 * i + 1
                        });
                    }
                    assets.release(&old).unwrap();
                    assets.streaming.gate = None;
                    let current = assets.request_async(&reference(i)).unwrap();
                    assert_ne!(old.generation, current.generation);
                    olds.push(old);
                    currents.push(current);
                }
                wait(|| {
                    assets
                        .streaming
                        .worker
                        .as_ref()
                        .unwrap()
                        .activity
                        .ready
                        .load(Ordering::Acquire)
                        == 20
                });
                assert_eq!(assets.streaming_snapshot().pending_operations, 20);
                assert_eq!(assets.streaming.retired_operations.len(), 10);
                assert!(assets.streaming_snapshot().reserved_bytes > 0);
                let worker = assets.streaming.worker.as_mut().unwrap();
                let mut completed = (0..20)
                    .map(|_| worker.receiver.try_recv().unwrap())
                    .collect::<Vec<_>>();
                if phase == 3 {
                    assert!(
                        completed.iter().all(|c| c.result.is_ok()),
                        "CpuReady mailbox must contain actual successful decoder results"
                    );
                }
                match order {
                    1 => completed.reverse(),
                    2 => completed.sort_by_key(|c| (c.id % 2, std::cmp::Reverse(c.id))),
                    _ => (),
                }
                let actual_order: Vec<_> = completed.iter().map(|c| c.id).collect();
                let (sender, receiver) = mpsc::sync_channel(20);
                for c in completed {
                    sender.send(c).unwrap();
                }
                // Preserve the real mailbox so this same live worker can service the next subcase.
                let real_receiver = std::mem::replace(&mut worker.receiver, receiver);
                while assets.streaming_snapshot().pending_operations != 0 {
                    assert!(assets.poll_async() > 0);
                }
                assets.streaming.worker.as_mut().unwrap().receiver = real_receiver;
                drop(sender);
                let after = assets.streaming_snapshot();
                assert_eq!(after.applied_operations - before.applied_operations, 10);
                assert_eq!(after.cancelled_results - before.cancelled_results, 10);
                assert_eq!(
                    (
                        after.pending_operations,
                        after.pending_tickets,
                        after.reserved_bytes
                    ),
                    (0, 0, 0)
                );
                for (old, current) in olds.iter().zip(&currents) {
                    assert_eq!(
                        assets.get_handle_state(old),
                        Some(RuntimeAssetLoadState::Released)
                    );
                    assert!(assets.get_asset_bytes(old).is_none());
                    assert_eq!(
                        assets.get_handle_state(current),
                        Some(RuntimeAssetLoadState::Ready)
                    );
                    assert!(assets.get_asset_bytes(current).is_some());
                }
                for current in currents {
                    assets.release(&current).unwrap();
                }
                let released = assets.streaming_snapshot();
                assert_eq!(released.cpu_resident_bytes, original.cpu_resident_bytes);
                assert_eq!(assets.decoded_cache_len(), cache_count);
                eprintln!("BM11 F02 CPU phase={phase} order={order} actualOperationOrder={actual_order:?}: ten old Released/no resurrection, ten new generations Ready, appliedDelta=10 cancelledDelta=10 cache/pending/reservation baseline");
            }
        }
        assets.streaming.gate = None;
    }
    assert_eq!(world.entity_count(), entities);
    assert_eq!(world.runtime_id_for_source(&witness_id), Some(witness));
    assert!(world.try_resolve_runtime_entity(witness).is_ok());
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
    assert_eq!(world.entity_count(), entities);
    eprintln!("BM11 original BM10 F02 CPU terminal: Reading/Decoding/CpuReady-mailbox x3 actual completion orders passed; frame={frame} entities={entities} retainedDrawRecords={retained_draw_records} nineActive=9 populationExact/allDomainsUpdated=true; GPU UploadQueued remains untested; elapsed={:?}",fault_started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[cfg(all(test, feature = "real-wgpu"))]
#[test]
#[ignore = "explicit original BM10 package; first-cell required Physics capability failure only"]
fn bm11_original_bm10_first_cell_rejects_missing_physics_owner_then_initializes_normally() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::render_resource::RenderResourceManager;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::{
        CellActivationRequest, CellDomain, CellError, CellStatus, RuntimeInstanceLoader,
    };
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
    let capture = |assets: &RuntimeAssetLoader| {
        let mut entries = std::collections::BTreeMap::new();
        for (key, cached) in &assets.decoded_cache {
            entries.insert(
                key.clone(),
                (
                    cached.asset_type.clone(),
                    cached.ref_count,
                    cached.bytes_len,
                    cached.payload.is_some(),
                    cached.bytes.clone(),
                ),
            );
        }
        let mut owners = std::collections::BTreeMap::new();
        for (id, key) in &assets.cache_keys_by_handle {
            if assets
                .handles
                .get(id)
                .is_some_and(|h| h.state == RuntimeAssetLoadState::Ready)
            {
                owners.insert(*id, key.clone());
            }
        }
        (entries, owners)
    };
    let before_cache = capture(loader.asset_loader());
    let baseline = loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (baseline.pending_operations, baseline.reserved_bytes),
        (0, 0)
    );
    let mut manager = RenderResourceManager::new();
    let ticket = loader
        .request_cell(
            CellActivationRequest {
                schema_version: "cell-activation.v1".into(),
                placement_id: "bm11-required-owner-before-first-activation".into(),
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
                parent: None,
            },
            &world,
        )
        .unwrap();
    let root = loader
        .asset_loader()
        .handles
        .values()
        .find(|h| h.asset_id == "bm10-k2-0" && h.state != RuntimeAssetLoadState::Released)
        .unwrap()
        .clone();
    while loader.asset_loader().get_handle_state(&root) == Some(RuntimeAssetLoadState::Loading) {
        assert!(
            fault_started.elapsed() < Duration::from_secs(30),
            "original first-cell read must be bounded"
        );
        loader.asset_loader_mut().poll_async();
        std::thread::sleep(Duration::from_millis(1));
    }
    let payload = loader.asset_loader().decoded_payload(&root).unwrap();
    let crate::runtime_asset::RuntimeDecodedPayload::Prefab(prefab) = payload.as_ref() else {
        panic!("original full prefab expected");
    };
    let original_cell_entities = prefab.entities.len();
    assert!(original_cell_entities >= 6000);
    drop(payload);
    let error = loop {
        match loader.prepare_cell(ticket, &world, 1, &mut manager) {
            Err(CellError::NotReady) => {
                assert!(
                    fault_started.elapsed() < Duration::from_secs(30),
                    "original first-cell prepare must be bounded"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => break error,
            Ok(status) => panic!("missing required Physics owner cannot prepare: {status:?}"),
        }
    };
    assert_eq!(error, CellError::UnsupportedDomain(CellDomain::Physics));
    assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Failed));
    assert_eq!(
        loader.cell_error(ticket),
        Some(&CellError::UnsupportedDomain(CellDomain::Physics))
    );
    assert_eq!(loader.cell_activation_epoch(ticket), Ok(None));
    assert!(loader
        .project_cell_snapshot(&world)
        .receipts
        .values()
        .all(|r| r.activation_epoch.is_none()));
    assert_eq!(world.entity_count(), baseline_entities);
    assert!(world.try_resolve_runtime_entity(original_handle).is_ok());
    loader
        .release_cell(ticket, &mut world, &mut manager, 1)
        .unwrap();
    assert_eq!(loader.cell_status(ticket), Ok(CellStatus::Released));
    loader.asset_loader_mut().poll_async();
    let after = loader.asset_loader().streaming_snapshot();
    let after_cache = capture(loader.asset_loader());
    // Startup's synchronous cache lacks the typed Render payload. The genuine
    // async decode may upgrade that same still-owned key; releasing the failed
    // cell must restore owners/references, not discard the scene's richer cache.
    assert_eq!(
        before_cache.1, after_cache.1,
        "failed cell retained a live owner"
    );
    assert_eq!(
        before_cache.0.keys().collect::<Vec<_>>(),
        after_cache.0.keys().collect::<Vec<_>>()
    );
    let mut upgraded_bytes = 0;
    let mut upgraded_count = 0;
    for (key, before) in &before_cache.0 {
        let after_entry = &after_cache.0[key];
        assert_eq!(
            (&before.0, before.1, &before.4),
            (&after_entry.0, after_entry.1, &after_entry.4)
        );
        let actual = &loader.asset_loader().decoded_cache[key];
        assert_eq!(
            actual.ref_count as usize,
            after_cache
                .1
                .values()
                .filter(|owned_key| *owned_key == key)
                .count()
        );
        if before != after_entry {
            assert!(!before.3 && after_entry.3);
            assert!(matches!(before.0.as_str(), "mesh" | "material"));
            assert!(matches!(
                actual.payload.as_deref(),
                Some(RuntimeDecodedPayload::Render(_))
            ));
            let raw_bytes = before.4.as_ref().unwrap().len();
            assert_eq!(before.2, raw_bytes);
            let charge = decode_envelope(raw_bytes, &before.0).unwrap();
            assert_eq!(after_entry.2, raw_bytes + charge);
            upgraded_bytes += charge;
            upgraded_count += 1;
        }
    }
    assert_eq!(upgraded_count, 2);
    assert_eq!(
        after.cpu_resident_bytes,
        baseline.cpu_resident_bytes + upgraded_bytes
    );
    assert_eq!(loader.asset_loader().external_cpu_reservation, 0);
    assert_eq!((after.pending_operations, after.reserved_bytes), (0, 0));
    eprintln!("BM11 F03 first-cell required Physics owner: entities={original_cell_entities} Failed then Released, World unchanged/old handle valid/no activation epoch; original cache keys/live owners/refcounts/raw bytes restored, two live Render typed upgrades charged={upgraded_bytes}; fault={:?}",fault_started.elapsed());
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
        "bm11-original-bm10-first-cell-capability",
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
    eprintln!("BM11 original BM10 first-cell capability terminal: frame={frame} entities=56701 retainedDrawRecords={retained} nineActive=9 populationExact/allDomainsUpdated=true after actual failed first-cell request; setup={:?}; not full-active-background capability failure or Navigation-only failure",started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[cfg(test)]
#[test]
#[ignore = "explicit original BM10 package; CPU-only bounded decoder shutdown, not automatic task timeout"]
fn bm11_original_bm10_decode_shutdown_timeout_retains_inflight_until_actual_exit() {
    let package_path = PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal452 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let world = crate::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let entities = world.entity_count();
    let witness_id = world.entity_ids().into_iter().next().unwrap().clone();
    let witness = world.runtime_id_for_source(&witness_id).unwrap();
    let mut loader = RuntimeAssetLoader::new(
        package.package_dir.clone(),
        package.runtime_asset_index.clone(),
        package.runtime_asset_mount_table.clone(),
    )
    .with_package_source(package.source.clone());
    loader
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let gate = Arc::new(TestWorkerGate::default());
    // The existing test gate's mutex holds the real worker before its first
    // read. It remains blocked after cancellation, representing an outstanding
    // non-interruptible task without adding a production hook or touching GPU.
    let held = gate.thread.lock().unwrap();
    loader.streaming.gate = Some(gate.clone());
    let reference = RuntimeAssetRef {
        id: "bm10-k2-0".into(),
        asset_type: "prefab".into(),
        guid: Some("bm10-k2-0".into()),
        sub_asset: None,
    };
    let ticket = loader.request_async(&reference).unwrap();
    let started = Instant::now();
    let activity = loader.streaming.worker.as_ref().unwrap().activity.clone();
    while activity.decoding.load(Ordering::Acquire) != 1 {
        assert!(started.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(1));
    }
    let before = loader.streaming_snapshot();
    assert_eq!(before.dispatched_operations, 13);
    assert_eq!(before.pending_operations, 13);
    assert_eq!(before.pending_tickets, 1);
    assert!(before.reserved_bytes > 0);
    assert_eq!(before.cpu_resident_bytes, 0);
    let timeout_started = Instant::now();
    assert!(!loader.shutdown_streaming(Duration::from_millis(20)));
    let timeout_elapsed = timeout_started.elapsed();
    assert!(timeout_elapsed >= Duration::from_millis(20));
    assert!(timeout_elapsed < Duration::from_secs(2));
    assert_eq!(
        loader.streaming_error(&ticket),
        Some(&StreamingError::Cancelled)
    );
    assert_eq!(
        loader.get_handle_state(&ticket),
        Some(RuntimeAssetLoadState::Failed)
    );
    assert_eq!(
        loader.request_async(&reference),
        Err(StreamingError::WorkerStopped)
    );
    let outstanding = loader.streaming_snapshot();
    assert_eq!(outstanding.pending_tickets, 0);
    assert_eq!(outstanding.pending_operations, before.pending_operations);
    assert_eq!(outstanding.reserved_bytes, before.reserved_bytes);
    assert_eq!(activity.decoding.load(Ordering::Acquire), 1);
    assert!(!loader
        .streaming
        .worker
        .as_ref()
        .unwrap()
        .thread
        .as_ref()
        .unwrap()
        .is_finished());
    assert_eq!(loader.decoded_cache_len(), 0);
    assert_eq!(world.entity_count(), entities);
    assert_eq!(world.runtime_id_for_source(&witness_id), Some(witness));
    assert!(world.try_resolve_runtime_entity(witness).is_ok());

    drop(held);
    assert!(loader.shutdown_streaming(Duration::from_secs(2)));
    assert!(loader.streaming.worker.is_none());
    assert_eq!(activity.decoding.load(Ordering::Acquire), 0);
    let after = loader.streaming_snapshot();
    assert_eq!(
        (
            after.pending_operations,
            after.pending_tickets,
            after.reserved_bytes
        ),
        (0, 0, 0)
    );
    assert_eq!(loader.decoded_cache_len(), 0);
    assert_eq!(
        loader.get_handle_state(&ticket),
        Some(RuntimeAssetLoadState::Failed)
    );
    assert_eq!(
        loader.streaming_error(&ticket),
        Some(&StreamingError::Cancelled)
    );
    assert_eq!(
        loader.request_async(&reference),
        Err(StreamingError::WorkerStopped)
    );
    assert_eq!(world.entity_count(), entities);
    assert_eq!(world.runtime_id_for_source(&witness_id), Some(witness));
    eprintln!("BM11 F06 original full cell closure: 13 dispatched/one decoding, bounded shutdown false after {timeout_elapsed:?}; admission closed/ticket Cancelled while all {} reservation bytes retained; release test gate -> actual worker exit, no late cache publication, pending/tickets/reserved0; existing scene witness remains valid. CPU initialization shutdown subitem only, not automatic execution deadline/full-active BM10/complete F06.", before.reserved_bytes);
}
