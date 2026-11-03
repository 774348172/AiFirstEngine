use crate::platform_time::Instant;
use crate::render_resource::RenderResourceManager;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::render_thread::{
    RenderFramePacket, RenderSubmissionReport, RenderSubmissionTicket, RenderThread,
    RenderThreadConfig, RenderThreadDiagnostic, RenderThreadDiagnosticSeverity,
    RenderThreadFrameOutput, RenderThreadMode,
};

static NEXT_FENCE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFenceId(pub u64);

impl RenderFenceId {
    fn next() -> Self {
        Self(NEXT_FENCE_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderFenceSyncDepth {
    RenderThread,
    RhiSubmit,
    Present,
    GpuCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderFenceStatus {
    Pending,
    Completed,
    Timeout,
    WorkerLost,
    NotObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFence {
    pub fence_id: RenderFenceId,
    pub sync_depth: RenderFenceSyncDepth,
    pub created_frame_index: u64,
    pub target_frame_index: Option<u64>,
}

impl RenderFence {
    pub fn new(
        sync_depth: RenderFenceSyncDepth,
        created_frame_index: u64,
        target_frame_index: Option<u64>,
    ) -> Self {
        Self {
            fence_id: RenderFenceId::next(),
            sync_depth,
            created_frame_index,
            target_frame_index,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderFenceResult {
    pub fence_id: RenderFenceId,
    pub sync_depth: RenderFenceSyncDepth,
    pub status: RenderFenceStatus,
    pub wait_ms: u64,
    pub completed_frame_index: u64,
    pub diagnostics: Vec<RenderThreadDiagnostic>,
}

impl RenderFenceResult {
    fn completed(fence: &RenderFence, wait_ms: u64, completed_frame_index: u64) -> Self {
        let mut diagnostics = Vec::new();
        let target_reached = fence
            .target_frame_index
            .is_none_or(|target| completed_frame_index >= target);
        if !target_reached {
            diagnostics.push(RenderThreadDiagnostic::info(
                "fence_target_not_reached",
                "Requested CPU frame has not completed",
                "render_fence",
            ));
        }
        if fence.sync_depth != RenderFenceSyncDepth::RenderThread {
            diagnostics.push(RenderThreadDiagnostic::info(
                "gpu_sync_not_observed",
                "CPU plan completion cannot prove RHI submission, GPU completion or surface Present; consume the backend submission token",
                "render_fence",
            ));
        }
        Self {
            fence_id: fence.fence_id,
            sync_depth: fence.sync_depth,
            status: if fence.sync_depth == RenderFenceSyncDepth::RenderThread && target_reached {
                RenderFenceStatus::Completed
            } else {
                RenderFenceStatus::NotObserved
            },
            wait_ms,
            completed_frame_index,
            diagnostics,
        }
    }

    fn worker_lost(fence: &RenderFence, wait_ms: u64, completed_frame_index: u64) -> Self {
        Self {
            fence_id: fence.fence_id,
            sync_depth: fence.sync_depth,
            status: RenderFenceStatus::WorkerLost,
            wait_ms,
            completed_frame_index,
            diagnostics: vec![RenderThreadDiagnostic::error(
                "worker_lost",
                "RenderThread worker channel closed before fence completed",
                "render_fence",
            )],
        }
    }

    fn timeout(fence: &RenderFence, wait_ms: u64, completed_frame_index: u64) -> Self {
        Self {
            fence_id: fence.fence_id,
            sync_depth: fence.sync_depth,
            status: RenderFenceStatus::Timeout,
            wait_ms,
            completed_frame_index,
            diagnostics: vec![RenderThreadDiagnostic {
                severity: RenderThreadDiagnosticSeverity::Warning,
                code: "fence_timeout".to_string(),
                message: "Render fence wait timed out".to_string(),
                layer: "render_fence".to_string(),
            }],
        }
    }
}

#[derive(Debug, Clone)]
pub enum RenderThreadCommand {
    SubmitFrame {
        packet: RenderFramePacket,
        ticket: RenderSubmissionTicket,
    },
    InsertFence {
        fence: RenderFence,
    },
    Shutdown {
        fence: RenderFence,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderWorkerMode {
    InlineDeterministic,
    DedicatedWorker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderWorkerState {
    NotStarted,
    Running,
    ShuttingDown,
    Stopped,
    WorkerLost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderWorkerReport {
    pub schema_version: String,
    pub worker_id: String,
    pub mode: RenderWorkerMode,
    pub state: RenderWorkerState,
    pub last_submitted_frame: u64,
    pub last_completed_frame: u64,
    pub in_flight_frames: u64,
    pub frame_lag: u64,
    pub fence_wait_count: u64,
    pub timeout_count: u64,
    pub worker_lost: bool,
    pub diagnostics: Vec<RenderThreadDiagnostic>,
}

impl RenderWorkerReport {
    fn new(worker_id: impl Into<String>, mode: RenderWorkerMode) -> Self {
        Self {
            schema_version: "render-worker-report.v1".to_string(),
            worker_id: worker_id.into(),
            mode,
            state: RenderWorkerState::NotStarted,
            last_submitted_frame: 0,
            last_completed_frame: 0,
            in_flight_frames: 0,
            frame_lag: 0,
            fence_wait_count: 0,
            timeout_count: 0,
            worker_lost: false,
            diagnostics: Vec::new(),
        }
    }

    fn refresh_lag(&mut self) {
        self.in_flight_frames = self
            .last_submitted_frame
            .saturating_sub(self.last_completed_frame);
        self.frame_lag = self.in_flight_frames;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameLagController {
    pub max_frames_in_flight: u64,
    pub current_game_frame: u64,
    pub completed_render_frame: u64,
}

impl Default for FrameLagController {
    fn default() -> Self {
        Self {
            max_frames_in_flight: 2,
            current_game_frame: 0,
            completed_render_frame: 0,
        }
    }
}

impl FrameLagController {
    pub fn new(max_frames_in_flight: u64) -> Self {
        Self {
            max_frames_in_flight: max_frames_in_flight.max(1),
            ..Self::default()
        }
    }

    pub fn update(&mut self, current_game_frame: u64, completed_render_frame: u64) {
        self.current_game_frame = current_game_frame;
        self.completed_render_frame = completed_render_frame;
    }

    pub fn frame_lag(&self) -> u64 {
        self.current_game_frame
            .saturating_sub(self.completed_render_frame)
    }

    pub fn should_wait(&self) -> bool {
        self.frame_lag() > self.max_frames_in_flight
    }

    pub fn wait_target_frame(&self) -> Option<u64> {
        self.should_wait()
            .then(|| self.current_game_frame - self.max_frames_in_flight)
    }
}

#[derive(Debug)]
enum RenderWorkerEvent {
    Submission(RenderThreadFrameOutput, RenderSubmissionReport),
    Fence(RenderFenceResult),
    WorkerReport(RenderWorkerReport),
}

#[derive(Debug, Clone)]
struct CachedSubmission {
    output: RenderThreadFrameOutput,
    report: RenderSubmissionReport,
}

#[derive(Debug)]
pub struct RenderThreadWorker {
    command_sender: SyncSender<RenderThreadCommand>,
    stop_requested: Arc<AtomicBool>,
    outstanding_frames: usize,
    outstanding_fences: usize,
    event_receiver: Receiver<RenderWorkerEvent>,
    thread_handle: Option<JoinHandle<()>>,
    cached_submissions: VecDeque<CachedSubmission>,
    cached_fences: VecDeque<RenderFenceResult>,
    latest_report: RenderWorkerReport,
}

impl RenderThreadWorker {
    pub fn spawn(worker_id: impl Into<String>, config: RenderThreadConfig) -> Self {
        Self::spawn_with_resources(
            worker_id,
            config,
            Arc::new(Mutex::new(RenderResourceManager::new())),
        )
    }

    fn spawn_with_resources(
        worker_id: impl Into<String>,
        config: RenderThreadConfig,
        resources: Arc<Mutex<RenderResourceManager>>,
    ) -> Self {
        let worker_id = worker_id.into();
        let (command_sender, command_receiver) = mpsc::sync_channel(4);
        let stop_requested = Arc::new(AtomicBool::new(false));
        let thread_stop = stop_requested.clone();
        let (event_sender, event_receiver) = mpsc::channel();
        let thread_worker_id = worker_id.clone();
        let thread_handle = thread::Builder::new()
            .name(thread_worker_id.clone())
            .spawn(move || {
                run_render_worker(
                    thread_worker_id,
                    config,
                    resources,
                    command_receiver,
                    event_sender,
                    thread_stop,
                );
            })
            .expect("spawn render thread worker");
        let mut latest_report =
            RenderWorkerReport::new(worker_id.clone(), RenderWorkerMode::DedicatedWorker);
        latest_report.state = RenderWorkerState::Running;
        Self {
            command_sender,
            stop_requested,
            outstanding_frames: 0,
            outstanding_fences: 0,
            event_receiver,
            thread_handle: Some(thread_handle),
            cached_submissions: VecDeque::new(),
            cached_fences: VecDeque::new(),
            latest_report,
        }
    }

    pub fn submit_frame(
        &mut self,
        packet: RenderFramePacket,
        ticket: RenderSubmissionTicket,
    ) -> Result<(), RenderThreadDiagnostic> {
        if self.stop_requested.load(Ordering::Acquire)
            || self.latest_report.state != RenderWorkerState::Running
        {
            return Err(RenderThreadDiagnostic::error(
                "worker_stopped",
                "Worker admission is closed",
                "render_worker",
            ));
        }
        if self.outstanding_frames >= 8 {
            return Err(RenderThreadDiagnostic::error(
                "queue_full",
                "Eight reliable frame results are outstanding; consume results before retry",
                "render_worker",
            ));
        }
        let frame_index = packet.frame_index;
        self.command_sender
            .try_send(RenderThreadCommand::SubmitFrame { packet, ticket })
            .map_err(|error| {
                if matches!(error,mpsc::TrySendError::Full(_)) {return RenderThreadDiagnostic::error("queue_full","Bounded render queue rejected admission; caller must retry the immutable packet","render_worker");}
                self.latest_report.state = RenderWorkerState::WorkerLost;
                self.latest_report.worker_lost = true;
                RenderThreadDiagnostic::error(
                    "worker_lost",
                    "RenderThread worker command channel is closed",
                    "render_worker",
                )
            })?;
        self.latest_report.last_submitted_frame = frame_index;
        self.outstanding_frames += 1;
        self.latest_report.refresh_lag();
        Ok(())
    }

    pub fn insert_fence(&mut self, fence: RenderFence) -> Result<(), RenderThreadDiagnostic> {
        if self.stop_requested.load(Ordering::Acquire)
            || self.latest_report.state != RenderWorkerState::Running
        {
            return Err(RenderThreadDiagnostic::error(
                "worker_stopped",
                "Worker admission is closed",
                "render_worker",
            ));
        }
        if self.outstanding_fences >= 16 {
            return Err(RenderThreadDiagnostic::error(
                "queue_full",
                "Consume outstanding fence results before retry",
                "render_worker",
            ));
        }
        self.command_sender
            .try_send(RenderThreadCommand::InsertFence { fence })
            .map_err(|error| {
                if matches!(error, mpsc::TrySendError::Full(_)) {
                    return RenderThreadDiagnostic::error(
                        "queue_full",
                        "Fence admission rejected by full render queue",
                        "render_worker",
                    );
                }
                self.latest_report.state = RenderWorkerState::WorkerLost;
                self.latest_report.worker_lost = true;
                RenderThreadDiagnostic::error(
                    "worker_lost",
                    "RenderThread worker command channel is closed",
                    "render_worker",
                )
            })?;
        self.outstanding_fences += 1;
        Ok(())
    }

    pub fn poll_submission(
        &mut self,
        ticket: RenderSubmissionTicket,
    ) -> Option<RenderSubmissionReport> {
        self.poll_submission_output(ticket)
            .map(|(_, report)| report)
    }

    pub fn poll_submission_output(
        &mut self,
        ticket: RenderSubmissionTicket,
    ) -> Option<(RenderThreadFrameOutput, RenderSubmissionReport)> {
        self.drain_events();
        let index = self.cached_submissions.iter().position(|cached| {
            cached.report.frame_index == ticket.frame_index
                && cached.report.submit_sequence == ticket.submit_sequence
        })?;
        let cached = self.cached_submissions.remove(index)?;
        self.outstanding_frames = self.outstanding_frames.saturating_sub(1);
        Some((cached.output, cached.report))
    }

    pub fn wait_fence(&mut self, fence: &RenderFence, timeout: Duration) -> RenderFenceResult {
        self.latest_report.fence_wait_count += 1;
        let start = Instant::now();
        loop {
            self.drain_events();
            if let Some(index) = self
                .cached_fences
                .iter()
                .position(|result| result.fence_id == fence.fence_id)
            {
                self.outstanding_fences = self.outstanding_fences.saturating_sub(1);
                return self
                    .cached_fences
                    .remove(index)
                    .expect("fence result should exist");
            }
            if start.elapsed() >= timeout {
                self.latest_report.timeout_count += 1;
                return RenderFenceResult::timeout(
                    fence,
                    elapsed_ms(start),
                    self.latest_report.last_completed_frame,
                );
            }
            match self.event_receiver.recv_timeout(Duration::from_millis(1)) {
                Ok(event) => self.handle_event(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.latest_report.state = RenderWorkerState::WorkerLost;
                    self.latest_report.worker_lost = true;
                    return RenderFenceResult::worker_lost(
                        fence,
                        elapsed_ms(start),
                        self.latest_report.last_completed_frame,
                    );
                }
            }
        }
    }

    pub fn shutdown(&mut self, timeout: Duration) -> RenderFenceResult {
        let fence = RenderFence::new(
            RenderFenceSyncDepth::RenderThread,
            self.latest_report.last_submitted_frame,
            None,
        );
        if let Err(error) = self.command_sender.try_send(RenderThreadCommand::Shutdown {
            fence: fence.clone(),
        }) {
            self.stop_requested.store(true, Ordering::Release);
            if matches!(error, mpsc::TrySendError::Full(_)) {
                self.latest_report.state = RenderWorkerState::ShuttingDown;
                return RenderFenceResult::timeout(
                    &fence,
                    0,
                    self.latest_report.last_completed_frame,
                );
            }
            self.latest_report.state = RenderWorkerState::WorkerLost;
            self.latest_report.worker_lost = true;
            return RenderFenceResult::worker_lost(
                &fence,
                0,
                self.latest_report.last_completed_frame,
            );
        }
        self.latest_report.state = RenderWorkerState::ShuttingDown;
        let result = self.wait_fence(&fence, timeout);
        if result.status != RenderFenceStatus::Completed {
            self.stop_requested.store(true, Ordering::Release);
        }
        if self
            .thread_handle
            .as_ref()
            .is_some_and(|handle| handle.is_finished())
        {
            if let Some(handle) = self.thread_handle.take() {
                if handle.join().is_err() {
                    self.latest_report.state = RenderWorkerState::WorkerLost;
                    self.latest_report.worker_lost = true;
                    return RenderFenceResult::worker_lost(
                        &fence,
                        result.wait_ms,
                        result.completed_frame_index,
                    );
                }
            }
        }
        if result.status == RenderFenceStatus::Completed {
            self.latest_report.state = RenderWorkerState::Stopped;
        }
        result
    }

    pub fn latest_report(&mut self) -> RenderWorkerReport {
        self.drain_events();
        self.latest_report.clone()
    }

    fn drain_events(&mut self) {
        loop {
            match self.event_receiver.try_recv() {
                Ok(event) => self.handle_event(event),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.latest_report.state != RenderWorkerState::Stopped {
                        self.latest_report.state = RenderWorkerState::WorkerLost;
                        self.latest_report.worker_lost = true;
                        self.stop_requested.store(true, Ordering::Release);
                    }
                    break;
                }
            }
        }
    }

    fn handle_event(&mut self, event: RenderWorkerEvent) {
        match event {
            RenderWorkerEvent::Submission(output, report) => {
                self.latest_report.last_completed_frame = report.completed_frame_index;
                self.latest_report.refresh_lag();
                self.cached_submissions
                    .push_back(CachedSubmission { output, report });
            }
            RenderWorkerEvent::Fence(result) => {
                self.latest_report.last_completed_frame = result.completed_frame_index;
                self.latest_report.refresh_lag();
                self.cached_fences.push_back(result);
            }
            RenderWorkerEvent::WorkerReport(mut report) => {
                // The worker's queued startup/progress report may predate frames
                // already accepted by this producer. Do not erase admission or
                // local wait accounting while consuming that older observation.
                report.last_submitted_frame = report
                    .last_submitted_frame
                    .max(self.latest_report.last_submitted_frame);
                report.fence_wait_count = self.latest_report.fence_wait_count;
                report.timeout_count = self.latest_report.timeout_count;
                if self.stop_requested.load(Ordering::Acquire)
                    && !self.latest_report.diagnostics.is_empty()
                {
                    report.diagnostics = self.latest_report.diagnostics.clone();
                }
                report.refresh_lag();
                self.latest_report = report;
            }
        }
    }
}

impl Drop for RenderThreadWorker {
    fn drop(&mut self) {
        if self.thread_handle.is_some() {
            let _ = self.shutdown(Duration::from_millis(100));
        }
    }
}

#[derive(Debug)]
pub struct RenderCommandDispatcher {
    resources: Arc<Mutex<RenderResourceManager>>,
    mode: RenderWorkerMode,
    inline_render_thread: Option<RenderThread>,
    worker: Option<RenderThreadWorker>,
    next_submit_sequence: u64,
    completed_frame_index: u64,
    inline_reports: VecDeque<CachedSubmission>,
}

impl RenderCommandDispatcher {
    /// Native callers consume the checked plan in their own GPU backend.
    /// Dedicated workers retain their configured backend; this setter never silently changes them.
    pub(crate) fn set_external_rhi_enabled(&mut self, enabled: bool) -> bool {
        if let Some(thread) = &mut self.inline_render_thread {
            thread.set_external_rhi_enabled(enabled);
            true
        } else {
            false
        }
    }
    /// Explicit local measurement; dedicated-worker telemetry is unavailable here.
    pub fn set_ui_measurement_enabled(&self, enabled: bool) -> bool {
        if let Some(thread) = &self.inline_render_thread {
            thread.set_ui_measurement_enabled(enabled);
            true
        } else {
            false
        }
    }

    pub fn ui_projection_stats(&self) -> Option<crate::runtime_renderer::UiProjectionStats> {
        self.inline_render_thread
            .as_ref()
            .map(RenderThread::ui_projection_stats)
    }
    pub fn inline(config: RenderThreadConfig) -> Self {
        let resources = Arc::new(Mutex::new(RenderResourceManager::new()));
        Self {
            mode: RenderWorkerMode::InlineDeterministic,
            inline_render_thread: Some(RenderThread::with_resource_manager(
                config,
                Arc::clone(&resources),
            )),
            resources,
            worker: None,
            next_submit_sequence: 0,
            completed_frame_index: 0,
            inline_reports: VecDeque::new(),
        }
    }

    pub fn dedicated_worker(config: RenderThreadConfig) -> Self {
        let resources = Arc::new(Mutex::new(RenderResourceManager::new()));
        Self {
            mode: RenderWorkerMode::DedicatedWorker,
            inline_render_thread: None,
            worker: Some(RenderThreadWorker::spawn_with_resources(
                "render-thread-worker-1",
                config,
                Arc::clone(&resources),
            )),
            resources,
            next_submit_sequence: 0,
            completed_frame_index: 0,
            inline_reports: VecDeque::new(),
        }
    }

    pub(crate) fn with_resources<T>(
        &self,
        prepare: impl FnOnce(&mut RenderResourceManager) -> T,
    ) -> Result<T, RenderThreadDiagnostic> {
        let mut resources = self
            .resources
            .lock()
            .map_err(|_| Self::resource_owner_error())?;
        Ok(prepare(&mut resources))
    }

    pub(crate) fn preparation_error(&mut self) -> Option<RenderThreadDiagnostic> {
        if self.resources.is_poisoned() {
            return Some(Self::resource_owner_error());
        }
        let worker = self.worker.as_mut()?;
        worker.drain_events();
        if !worker.stop_requested.load(Ordering::Acquire)
            && worker.latest_report.state == RenderWorkerState::Running
        {
            return None;
        }
        // A timed-out task may still hold the original resource lock. Check its
        // terminal admission state before Host attempts any resource preparation.
        // This does not release the task or turn its CPU result into GPU completion.
        Some(
            worker
                .latest_report
                .diagnostics
                .iter()
                .rev()
                .find(|d| d.severity == RenderThreadDiagnosticSeverity::Error)
                .cloned()
                .unwrap_or_else(|| {
                    RenderThreadDiagnostic::error(
                        "worker_stopped",
                        "Worker admission is closed before Host resource preparation",
                        "render_worker",
                    )
                }),
        )
    }

    #[cfg(test)]
    pub(crate) fn resource_owner_for_fault_test(&self) -> Arc<Mutex<RenderResourceManager>> {
        Arc::clone(&self.resources)
    }

    pub(crate) fn resource_owner_error() -> RenderThreadDiagnostic {
        RenderThreadDiagnostic::error(
            "render_resource_owner_failed",
            "Original render resource owner is poisoned; reuse and automatic recovery are forbidden",
            "render_resource",
        )
    }

    pub(crate) fn reject_frame_output(
        &mut self,
        packet: RenderFramePacket,
        diagnostic: RenderThreadDiagnostic,
    ) -> (RenderThreadFrameOutput, RenderSubmissionReport) {
        self.next_submit_sequence += 1;
        let ticket = RenderSubmissionTicket {
            frame_index: packet.frame_index,
            submit_sequence: self.next_submit_sequence,
        };
        if let Some(worker) = self.worker.as_mut() {
            worker.stop_requested.store(true, Ordering::Release);
            if !worker
                .latest_report
                .diagnostics
                .iter()
                .any(|d| d.code == diagnostic.code)
            {
                worker.latest_report.diagnostics.push(diagnostic.clone());
            }
            // Teardown may still be pending. No join, poisoned-state recovery,
            // resource release, or GPU completion is inferred from this request.
            let _ = worker.shutdown(Duration::ZERO);
        }
        let (mut output, mut report) = synthetic_submission_output(ticket, diagnostic, packet);
        let mode = match self.mode {
            RenderWorkerMode::InlineDeterministic => RenderThreadMode::InlineDeterministic,
            RenderWorkerMode::DedicatedWorker => RenderThreadMode::DedicatedThread,
        };
        output.report.thread_mode = mode;
        report.thread_mode = mode;
        report.render_thread_report.thread_mode = mode;
        (output, report)
    }

    pub fn from_thread_mode(mode: RenderThreadMode) -> Self {
        match mode {
            RenderThreadMode::InlineDeterministic => Self::inline(RenderThreadConfig::default()),
            RenderThreadMode::DedicatedThread => Self::dedicated_worker(RenderThreadConfig {
                thread_mode: RenderThreadMode::DedicatedThread,
                backend_kind: "headless-rhi".to_string(),
            }),
        }
    }

    pub fn submit_frame(
        &mut self,
        packet: RenderFramePacket,
    ) -> (RenderSubmissionTicket, Option<RenderSubmissionReport>) {
        let (ticket, immediate) = self.submit_frame_output(packet);
        (ticket, immediate.map(|(_, report)| report))
    }

    pub fn submit_frame_output(
        &mut self,
        packet: RenderFramePacket,
    ) -> (
        RenderSubmissionTicket,
        Option<(RenderThreadFrameOutput, RenderSubmissionReport)>,
    ) {
        self.next_submit_sequence += 1;
        let ticket = RenderSubmissionTicket {
            frame_index: packet.frame_index,
            submit_sequence: self.next_submit_sequence,
        };
        match self.mode {
            RenderWorkerMode::InlineDeterministic => {
                let render_thread = self
                    .inline_render_thread
                    .as_mut()
                    .expect("inline render thread");
                let (output, mut report) = render_thread.submit_frame_output(packet);
                report.submit_sequence = ticket.submit_sequence;
                self.completed_frame_index = report.completed_frame_index;
                (ticket, Some((output, report)))
            }
            RenderWorkerMode::DedicatedWorker => {
                if let Some(worker) = self.worker.as_mut() {
                    let fallback_packet = packet.clone();
                    if let Err(diagnostic) = worker.submit_frame(packet, ticket) {
                        let (output, report) =
                            synthetic_submission_output(ticket, diagnostic, fallback_packet);
                        // Same immediate-result ownership as inline mode: the
                        // caller already consumes this result. Retaining a second
                        // copy here makes repeated rejected frames unbounded.
                        return (ticket, Some((output, report)));
                    }
                }
                (ticket, None)
            }
        }
    }

    pub fn poll_submission(
        &mut self,
        ticket: RenderSubmissionTicket,
    ) -> Option<RenderSubmissionReport> {
        self.poll_submission_output(ticket)
            .map(|(_, report)| report)
    }

    pub fn poll_submission_output(
        &mut self,
        ticket: RenderSubmissionTicket,
    ) -> Option<(RenderThreadFrameOutput, RenderSubmissionReport)> {
        if let Some(index) = self.inline_reports.iter().position(|cached| {
            cached.report.frame_index == ticket.frame_index
                && cached.report.submit_sequence == ticket.submit_sequence
        }) {
            let cached = self.inline_reports.remove(index)?;
            self.completed_frame_index = cached.report.completed_frame_index;
            return Some((cached.output, cached.report));
        }
        let (output, report) = self.worker.as_mut()?.poll_submission_output(ticket)?;
        self.completed_frame_index = report.completed_frame_index;
        Some((output, report))
    }

    /// The Host's existing synchronous consumer owns the fallback packet only
    /// for this bounded wait. No pending-packet store or fabricated GPU result.
    pub(crate) fn submit_frame_output_and_wait(
        &mut self,
        packet: RenderFramePacket,
    ) -> (RenderThreadFrameOutput, RenderSubmissionReport) {
        let fallback = (self.mode == RenderWorkerMode::DedicatedWorker).then(|| packet.clone());
        let (ticket, immediate) = self.submit_frame_output(packet);
        if let Some(output) = immediate {
            return output;
        }
        let fence = self.flush(RenderFenceSyncDepth::RenderThread);
        if let Some(output) = self.poll_submission_output(ticket) {
            return output;
        }
        let diagnostic = RenderThreadDiagnostic::error(
            match fence.status {
                RenderFenceStatus::Timeout => "render_worker_timeout",
                RenderFenceStatus::WorkerLost => "worker_lost",
                _ => "render_worker_missing_result",
            },
            "Accepted frame has no CPU result after the bounded render-worker wait",
            "render_worker",
        );
        if let Some(worker) = self.worker.as_mut() {
            // A timed-out accepted task is not complete. Close admission and
            // request bounded teardown; the current task still owns all of its
            // resources until it really leaves the owner. Never join it here.
            worker.stop_requested.store(true, Ordering::Release);
            if fence.status == RenderFenceStatus::Timeout {
                worker.latest_report.state = RenderWorkerState::ShuttingDown;
            }
            worker.latest_report.diagnostics.push(diagnostic.clone());
        }
        synthetic_submission_output(
            ticket,
            diagnostic,
            fallback.expect("dedicated worker packet"),
        )
    }

    pub fn insert_fence(&mut self, sync_depth: RenderFenceSyncDepth) -> RenderFence {
        let fence = RenderFence::new(sync_depth, self.completed_frame_index, None);
        match self.mode {
            RenderWorkerMode::InlineDeterministic => {
                let result = RenderFenceResult::completed(&fence, 0, self.completed_frame_index);
                self.inline_reports.make_contiguous();
                if let Some(worker) = self.worker.as_mut() {
                    let _ = worker.insert_fence(fence.clone());
                }
                let _ = result;
            }
            RenderWorkerMode::DedicatedWorker => {
                if let Some(worker) = self.worker.as_mut() {
                    let _ = worker.insert_fence(fence.clone());
                }
            }
        }
        fence
    }

    pub fn flush(&mut self, sync_depth: RenderFenceSyncDepth) -> RenderFenceResult {
        let fence = RenderFence::new(sync_depth, self.completed_frame_index, None);
        match self.mode {
            RenderWorkerMode::InlineDeterministic => {
                RenderFenceResult::completed(&fence, 0, self.completed_frame_index)
            }
            RenderWorkerMode::DedicatedWorker => {
                if let Some(worker) = self.worker.as_mut() {
                    let _ = worker.insert_fence(fence.clone());
                    let result = worker.wait_fence(&fence, Duration::from_secs(2));
                    self.completed_frame_index = result.completed_frame_index;
                    result
                } else {
                    RenderFenceResult::worker_lost(&fence, 0, self.completed_frame_index)
                }
            }
        }
    }

    pub fn shutdown(&mut self) -> RenderFenceResult {
        match self.mode {
            RenderWorkerMode::InlineDeterministic => {
                let fence = RenderFence::new(
                    RenderFenceSyncDepth::RenderThread,
                    self.completed_frame_index,
                    None,
                );
                RenderFenceResult::completed(&fence, 0, self.completed_frame_index)
            }
            RenderWorkerMode::DedicatedWorker => {
                if let Some(worker) = self.worker.as_mut() {
                    let result = worker.shutdown(Duration::from_secs(2));
                    self.completed_frame_index = result.completed_frame_index;
                    result
                } else {
                    let fence = RenderFence::new(
                        RenderFenceSyncDepth::RenderThread,
                        self.completed_frame_index,
                        None,
                    );
                    RenderFenceResult::worker_lost(&fence, 0, self.completed_frame_index)
                }
            }
        }
    }

    pub fn completed_frame_index(&mut self) -> u64 {
        if let Some(worker) = self.worker.as_mut() {
            self.completed_frame_index = worker.latest_report().last_completed_frame;
        }
        self.completed_frame_index
    }

    pub fn worker_report(&mut self) -> RenderWorkerReport {
        match self.mode {
            RenderWorkerMode::InlineDeterministic => {
                let mut report = RenderWorkerReport::new(
                    "inline-render-thread",
                    RenderWorkerMode::InlineDeterministic,
                );
                report.state = RenderWorkerState::Running;
                report.last_completed_frame = self.completed_frame_index;
                report.last_submitted_frame = self.completed_frame_index;
                report.refresh_lag();
                report
            }
            RenderWorkerMode::DedicatedWorker => self
                .worker
                .as_mut()
                .map(RenderThreadWorker::latest_report)
                .unwrap_or_else(|| {
                    let mut report = RenderWorkerReport::new(
                        "render-thread-worker-1",
                        RenderWorkerMode::DedicatedWorker,
                    );
                    report.state = RenderWorkerState::WorkerLost;
                    report.worker_lost = true;
                    report
                }),
        }
    }
}

impl Drop for RenderCommandDispatcher {
    fn drop(&mut self) {
        if self.mode == RenderWorkerMode::DedicatedWorker {
            let _ = self.shutdown();
        }
    }
}

fn run_render_worker(
    worker_id: String,
    config: RenderThreadConfig,
    resources: Arc<Mutex<RenderResourceManager>>,
    command_receiver: Receiver<RenderThreadCommand>,
    event_sender: Sender<RenderWorkerEvent>,
    stop_requested: Arc<AtomicBool>,
) {
    let report_thread_mode = config.thread_mode;
    let mut render_thread = RenderThread::with_resource_manager(
        RenderThreadConfig {
            thread_mode: RenderThreadMode::InlineDeterministic,
            backend_kind: config.backend_kind,
        },
        resources,
    );
    let mut report = RenderWorkerReport::new(worker_id, RenderWorkerMode::DedicatedWorker);
    report.state = RenderWorkerState::Running;
    let _ = event_sender.send(RenderWorkerEvent::WorkerReport(report.clone()));
    while let Ok(command) = command_receiver.recv() {
        if stop_requested.load(Ordering::Acquire) {
            break;
        }
        match command {
            RenderThreadCommand::SubmitFrame { packet, ticket } => {
                report.last_submitted_frame = packet.frame_index;
                let (mut output, mut submission) = render_thread.submit_frame_output(packet);
                output.report.thread_mode = report_thread_mode;
                submission.submit_sequence = ticket.submit_sequence;
                submission.thread_mode = report_thread_mode;
                submission.render_thread_report = output.report.clone();
                report.last_completed_frame = submission.completed_frame_index;
                report.refresh_lag();
                let _ = event_sender.send(RenderWorkerEvent::Submission(output, submission));
                let _ = event_sender.send(RenderWorkerEvent::WorkerReport(report.clone()));
            }
            RenderThreadCommand::InsertFence { fence } => {
                let result = RenderFenceResult::completed(&fence, 0, report.last_completed_frame);
                let _ = event_sender.send(RenderWorkerEvent::Fence(result));
            }
            RenderThreadCommand::Shutdown { fence } => {
                report.state = RenderWorkerState::ShuttingDown;
                let _ = event_sender.send(RenderWorkerEvent::WorkerReport(report.clone()));
                let result = RenderFenceResult::completed(&fence, 0, report.last_completed_frame);
                let _ = event_sender.send(RenderWorkerEvent::Fence(result));
                report.state = RenderWorkerState::Stopped;
                let _ = event_sender.send(RenderWorkerEvent::WorkerReport(report));
                break;
            }
        }
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn synthetic_submission_output(
    ticket: RenderSubmissionTicket,
    diagnostic: RenderThreadDiagnostic,
    packet: RenderFramePacket,
) -> (RenderThreadFrameOutput, RenderSubmissionReport) {
    let mut render_thread = RenderThread::new(RenderThreadConfig {
        thread_mode: RenderThreadMode::InlineDeterministic,
        backend_kind: "headless-rhi".to_string(),
    });
    let (mut output, _) = render_thread.submit_frame_output(packet);
    output.report.thread_mode = RenderThreadMode::DedicatedThread;
    output.report.rdg_status = "error".to_string();
    output.report.rhi_status = "error".to_string();
    output.report.present_status = "not_presented".to_string();
    output.report.diagnostics.push(diagnostic.clone());
    let report = RenderSubmissionReport {
        schema_version: "render-submission-report.v1".to_string(),
        frame_index: ticket.frame_index,
        submit_sequence: ticket.submit_sequence,
        accepted: false,
        submitted: false,
        presented: false,
        completed_frame_index: 0,
        queue_depth_after_submit: 0,
        queue_wait_frames: 0,
        thread_mode: RenderThreadMode::DedicatedThread,
        diagnostics: vec![diagnostic.clone()],
        render_thread_report: output.report.clone(),
    };
    (output, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_state::{
        RenderSceneState, RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
    };
    use crate::runtime_renderer::{QualityProfile, RenderTarget};

    fn scene() -> RenderSceneState {
        let mut scene = RenderSceneState::new();
        scene.register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::ViewportTexture,
        ));
        scene
    }

    fn packet(frame_index: u64) -> RenderFramePacket {
        RenderFramePacket {
            frame_identity: None,
            geometry: None,
            frame_index,
            render_scene_state: scene(),
            render_frame_report: None,
            resource_requests: Vec::new(),
            resource_release_requests: Vec::new(),
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            game_view_presentation: None,
            view_id: Some(RenderViewId(1)),
            quality_profile: QualityProfile::default(),
            render_target: RenderTarget::viewport_texture("viewport-main", 640, 360),
        }
    }

    #[test]
    fn render_fence_result_serializes() {
        let fence = RenderFence::new(RenderFenceSyncDepth::RenderThread, 1, Some(1));
        let result = RenderFenceResult::completed(&fence, 0, 1);
        let json = serde_json::to_string(&result).expect("serialize fence result");

        assert_eq!(result.status, RenderFenceStatus::Completed);
        assert!(json.contains("completedFrameIndex"));
    }

    #[test]
    fn rhi_submit_fence_does_not_fake_gpu_observation() {
        let fence = RenderFence::new(RenderFenceSyncDepth::RhiSubmit, 1, Some(1));
        let result = RenderFenceResult::completed(&fence, 0, 1);

        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "gpu_sync_not_observed"));
        assert_eq!(result.status, RenderFenceStatus::NotObserved);
    }
    #[test]
    fn graph_lifetime_worker_loss_is_observable_without_waiting_a_new_fence() {
        let resources = Arc::new(Mutex::new(RenderResourceManager::new()));
        let mut worker = RenderThreadWorker::spawn_with_resources(
            "failed-worker",
            RenderThreadConfig::default(),
            resources.clone(),
        );
        // Poison the real resource owner; the actual worker panics while consuming
        // an accepted immutable packet, without pretending a CPU fence is a GPU ACK.
        assert!(std::panic::catch_unwind(|| {
            let _guard = resources.lock().unwrap();
            panic!("injected render resource owner failure");
        })
        .is_err());
        worker
            .submit_frame(
                packet(1),
                RenderSubmissionTicket {
                    frame_index: 1,
                    submit_sequence: 1,
                },
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !worker.thread_handle.as_ref().unwrap().is_finished() {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        let report = worker.latest_report();
        assert_eq!(report.state, RenderWorkerState::WorkerLost);
        assert!(report.worker_lost);
        assert_eq!(report.last_completed_frame, 0);
        assert_eq!(report.last_submitted_frame, 1);
        assert_eq!(report.in_flight_frames, 1);
        assert!(worker
            .poll_submission_output(RenderSubmissionTicket {
                frame_index: 1,
                submit_sequence: 1,
            })
            .is_none());
        assert_eq!(
            worker.shutdown(Duration::from_secs(1)).status,
            RenderFenceStatus::WorkerLost
        );
    }

    #[test]
    fn graph_lifetime_worker_loss_returns_terminal_host_output_without_gpu_success() {
        let mut dispatcher =
            RenderCommandDispatcher::dedicated_worker(RenderThreadConfig::default());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = dispatcher.with_resources(|_| panic!("injected original owner failure"));
        }))
        .is_err());
        let start = Instant::now();
        let (output, report) = dispatcher.submit_frame_output_and_wait(packet(7));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(report.frame_index, 7);
        assert!(!report.accepted && !report.submitted && !report.presented);
        assert_eq!(report.completed_frame_index, 0);
        assert!(report.diagnostics.iter().any(|d| d.code == "worker_lost"));
        assert_eq!(output.report.rdg_status, "error");
        assert_eq!(output.report.present_status, "not_presented");
        assert!(dispatcher.worker_report().worker_lost);
        assert_eq!(dispatcher.completed_frame_index(), 0);
        for index in 8..=40 {
            let (_, report) = dispatcher.submit_frame_output_and_wait(packet(index));
            assert!(!report.accepted && !report.submitted && !report.presented);
            assert!(
                dispatcher.inline_reports.is_empty(),
                "immediate failure must not retain a second packet/output"
            );
        }
    }

    #[test]
    fn graph_lifetime_worker_loss_timeout_preserves_packet_and_real_completion_boundary() {
        let mut dispatcher =
            RenderCommandDispatcher::dedicated_worker(RenderThreadConfig::default());
        let resources = dispatcher.resources.clone();
        let guard = resources.lock().unwrap();
        let start = Instant::now();
        let (output, report) = dispatcher.submit_frame_output_and_wait(packet(9));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(report.frame_index, 9);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == "render_worker_timeout"));
        assert!(!report.accepted && !report.submitted && !report.presented);
        assert_eq!(report.completed_frame_index, 0);
        assert_eq!(output.report.present_status, "not_presented");
        let (_, next) = dispatcher.submit_frame_output(packet(10));
        let (_, next) = next.expect("timeout must close admission, not enqueue another frame");
        assert!(!next.accepted && !next.submitted && !next.presented);
        drop(guard);
        let start = Instant::now();
        while !dispatcher.worker_report().worker_lost {
            assert!(start.elapsed() < Duration::from_secs(3));
            thread::yield_now();
        }
        // Closing admission does not fabricate cancellation of a CPU task that
        // is actually still inside the resource owner. Its late result is real.
        assert_eq!(dispatcher.completed_frame_index(), 9);
        assert_eq!(dispatcher.shutdown().status, RenderFenceStatus::WorkerLost);
    }

    #[test]
    fn graph_lifetime_worker_full_queue_shutdown_never_joins_while_resource_lock_is_held() {
        let resources = Arc::new(Mutex::new(RenderResourceManager::new()));
        let guard = resources.lock().unwrap();
        let mut worker = RenderThreadWorker::spawn_with_resources(
            "bounded-worker",
            RenderThreadConfig::default(),
            resources.clone(),
        );
        // Block the actual owner, not a fake delay/scheduler. Admission must remain bounded.
        let mut accepted = 0;
        for i in 1..20 {
            match worker.submit_frame(
                packet(i),
                RenderSubmissionTicket {
                    frame_index: i,
                    submit_sequence: i,
                },
            ) {
                Ok(()) => accepted += 1,
                Err(d) => {
                    assert_eq!(d.code, "queue_full");
                    break;
                }
            }
        }
        assert!((4..=5).contains(&accepted));
        let start = Instant::now();
        let result = worker.shutdown(Duration::from_millis(5));
        assert!(matches!(result.status, RenderFenceStatus::Timeout));
        assert!(start.elapsed() < Duration::from_millis(100));
        drop(guard);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !worker.thread_handle.as_ref().unwrap().is_finished() {
            assert!(
                Instant::now() < deadline,
                "owned worker must terminate after the held lock is released"
            );
            thread::yield_now();
        }
        worker.thread_handle.take().unwrap().join().unwrap();
    }
    #[test]
    fn graph_lifetime_cpu_fence_and_gpu_depths_are_distinct() {
        let mut dispatcher =
            RenderCommandDispatcher::dedicated_worker(RenderThreadConfig::default());
        let (ticket, _) = dispatcher.submit_frame_output(packet(1));
        assert_eq!(
            dispatcher.flush(RenderFenceSyncDepth::RenderThread).status,
            RenderFenceStatus::Completed
        );
        assert!(dispatcher.poll_submission_output(ticket).is_some());
        for depth in [
            RenderFenceSyncDepth::RhiSubmit,
            RenderFenceSyncDepth::GpuCompletion,
            RenderFenceSyncDepth::Present,
        ] {
            assert_eq!(
                dispatcher.flush(depth).status,
                RenderFenceStatus::NotObserved
            );
        }
        assert_eq!(dispatcher.shutdown().status, RenderFenceStatus::Completed);
        assert_eq!(
            RenderFenceResult::completed(
                &RenderFence::new(RenderFenceSyncDepth::RenderThread, 1, Some(2)),
                0,
                1
            )
            .status,
            RenderFenceStatus::NotObserved
        );
    }
    #[test]
    fn graph_lifetime_unconsumed_fences_and_shutdown_admission_are_bounded() {
        let mut worker = RenderThreadWorker::spawn("bounded-fences", RenderThreadConfig::default());
        let mut fences = Vec::new();
        for _ in 0..16 {
            let fence = RenderFence::new(RenderFenceSyncDepth::RenderThread, 0, None);
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match worker.insert_fence(fence.clone()) {
                    Ok(()) => break,
                    Err(e) => {
                        assert_eq!(e.code, "queue_full");
                        assert!(Instant::now() < deadline);
                        thread::yield_now();
                    }
                }
            }
            fences.push(fence);
        }
        assert_eq!(
            worker
                .insert_fence(RenderFence::new(
                    RenderFenceSyncDepth::RenderThread,
                    0,
                    None
                ))
                .unwrap_err()
                .code,
            "queue_full"
        );
        for fence in fences {
            assert_eq!(
                worker.wait_fence(&fence, Duration::from_secs(2)).status,
                RenderFenceStatus::Completed
            );
        }
        assert_eq!(
            worker.shutdown(Duration::from_secs(2)).status,
            RenderFenceStatus::Completed
        );
        assert_eq!(
            worker
                .submit_frame(
                    packet(1),
                    RenderSubmissionTicket {
                        frame_index: 1,
                        submit_sequence: 1
                    }
                )
                .unwrap_err()
                .code,
            "worker_stopped"
        );
    }

    #[test]
    fn frame_lag_controller_waits_only_when_limit_exceeded() {
        let mut controller = FrameLagController::new(2);

        controller.update(3, 1);
        assert!(!controller.should_wait());
        assert_eq!(controller.wait_target_frame(), None);

        controller.update(4, 1);
        assert!(controller.should_wait());
        assert_eq!(controller.wait_target_frame(), Some(2));
    }

    #[test]
    fn dedicated_worker_submits_polls_flushes_and_shutdowns() {
        let mut dispatcher = RenderCommandDispatcher::dedicated_worker(RenderThreadConfig {
            thread_mode: RenderThreadMode::DedicatedThread,
            backend_kind: "headless-rhi".to_string(),
        });

        let (ticket, immediate) = dispatcher.submit_frame(packet(1));
        assert!(immediate.is_none());
        let fence = dispatcher.flush(RenderFenceSyncDepth::RenderThread);
        assert_eq!(fence.status, RenderFenceStatus::Completed);
        let report = dispatcher
            .poll_submission(ticket)
            .expect("submission report should be available after flush");

        assert_eq!(report.frame_index, 1);
        assert_eq!(report.submit_sequence, ticket.submit_sequence);
        assert_eq!(report.thread_mode, RenderThreadMode::DedicatedThread);
        assert!(report.presented);

        let worker_report = dispatcher.worker_report();
        assert_eq!(worker_report.last_completed_frame, 1);
        assert_eq!(worker_report.state, RenderWorkerState::Running);

        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.status, RenderFenceStatus::Completed);
    }

    #[test]
    fn inline_dispatcher_returns_submission_immediately() {
        let mut dispatcher = RenderCommandDispatcher::inline(RenderThreadConfig::default());

        let (ticket, immediate) = dispatcher.submit_frame(packet(7));
        let report = immediate.expect("inline report");

        assert_eq!(report.frame_index, 7);
        assert_eq!(report.submit_sequence, ticket.submit_sequence);
        assert_eq!(dispatcher.completed_frame_index(), 7);
    }

    #[test]
    fn inline_dispatcher_does_not_retain_immediate_submissions() {
        let mut dispatcher = RenderCommandDispatcher::inline(RenderThreadConfig::default());

        for frame_index in 1..=256 {
            let (_, immediate) = dispatcher.submit_frame_output(packet(frame_index));

            assert!(
                immediate.is_some(),
                "inline frame {frame_index} must be immediate"
            );
            assert!(
                dispatcher.inline_reports.is_empty(),
                "inline frame {frame_index} retained a duplicate completed submission"
            );
        }
    }
}

#[cfg(all(test, feature = "real-wgpu"))]
#[path = "render_thread_worker/f04_tests.rs"]
mod f04_tests;
