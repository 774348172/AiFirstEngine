use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderFenceStatus {
    Pending,
    Completed,
    Timeout,
    WorkerLost,
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
        if fence.sync_depth != RenderFenceSyncDepth::RenderThread {
            diagnostics.push(RenderThreadDiagnostic::info(
                "simulated_sync_depth",
                "RhiSubmit and Present fence depths are mapped to RenderThread depth in D-min",
                "render_fence",
            ));
        }
        Self {
            fence_id: fence.fence_id,
            sync_depth: fence.sync_depth,
            status: RenderFenceStatus::Completed,
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
    command_sender: Sender<RenderThreadCommand>,
    event_receiver: Receiver<RenderWorkerEvent>,
    thread_handle: Option<JoinHandle<()>>,
    cached_submissions: VecDeque<CachedSubmission>,
    cached_fences: VecDeque<RenderFenceResult>,
    latest_report: RenderWorkerReport,
}

impl RenderThreadWorker {
    pub fn spawn(worker_id: impl Into<String>, config: RenderThreadConfig) -> Self {
        let worker_id = worker_id.into();
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        let thread_worker_id = worker_id.clone();
        let thread_handle = thread::Builder::new()
            .name(thread_worker_id.clone())
            .spawn(move || {
                run_render_worker(thread_worker_id, config, command_receiver, event_sender);
            })
            .expect("spawn render thread worker");
        let mut latest_report =
            RenderWorkerReport::new(worker_id.clone(), RenderWorkerMode::DedicatedWorker);
        latest_report.state = RenderWorkerState::Running;
        Self {
            command_sender,
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
        let frame_index = packet.frame_index;
        self.command_sender
            .send(RenderThreadCommand::SubmitFrame { packet, ticket })
            .map_err(|_| {
                self.latest_report.state = RenderWorkerState::WorkerLost;
                self.latest_report.worker_lost = true;
                RenderThreadDiagnostic::error(
                    "worker_lost",
                    "RenderThread worker command channel is closed",
                    "render_worker",
                )
            })?;
        self.latest_report.last_submitted_frame = frame_index;
        self.latest_report.refresh_lag();
        Ok(())
    }

    pub fn insert_fence(&mut self, fence: RenderFence) -> Result<(), RenderThreadDiagnostic> {
        self.command_sender
            .send(RenderThreadCommand::InsertFence { fence })
            .map_err(|_| {
                self.latest_report.state = RenderWorkerState::WorkerLost;
                self.latest_report.worker_lost = true;
                RenderThreadDiagnostic::error(
                    "worker_lost",
                    "RenderThread worker command channel is closed",
                    "render_worker",
                )
            })
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
        if self
            .command_sender
            .send(RenderThreadCommand::Shutdown {
                fence: fence.clone(),
            })
            .is_err()
        {
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
        while let Ok(event) = self.event_receiver.try_recv() {
            self.handle_event(event);
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
            RenderWorkerEvent::WorkerReport(report) => {
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
    mode: RenderWorkerMode,
    inline_render_thread: Option<RenderThread>,
    worker: Option<RenderThreadWorker>,
    next_submit_sequence: u64,
    completed_frame_index: u64,
    inline_reports: VecDeque<CachedSubmission>,
}

impl RenderCommandDispatcher {
    pub fn inline(config: RenderThreadConfig) -> Self {
        Self {
            mode: RenderWorkerMode::InlineDeterministic,
            inline_render_thread: Some(RenderThread::new(config)),
            worker: None,
            next_submit_sequence: 0,
            completed_frame_index: 0,
            inline_reports: VecDeque::new(),
        }
    }

    pub fn dedicated_worker(config: RenderThreadConfig) -> Self {
        Self {
            mode: RenderWorkerMode::DedicatedWorker,
            inline_render_thread: None,
            worker: Some(RenderThreadWorker::spawn("render-thread-worker-1", config)),
            next_submit_sequence: 0,
            completed_frame_index: 0,
            inline_reports: VecDeque::new(),
        }
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
                        self.inline_reports.push_back(CachedSubmission {
                            output: output.clone(),
                            report: report.clone(),
                        });
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
    command_receiver: Receiver<RenderThreadCommand>,
    event_sender: Sender<RenderWorkerEvent>,
) {
    let report_thread_mode = config.thread_mode;
    let mut render_thread = RenderThread::new(RenderThreadConfig {
        thread_mode: RenderThreadMode::InlineDeterministic,
        backend_kind: config.backend_kind,
    });
    let mut report = RenderWorkerReport::new(worker_id, RenderWorkerMode::DedicatedWorker);
    report.state = RenderWorkerState::Running;
    let _ = event_sender.send(RenderWorkerEvent::WorkerReport(report.clone()));
    while let Ok(command) = command_receiver.recv() {
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
    fn rhi_submit_fence_reports_simulated_depth_in_d_min() {
        let fence = RenderFence::new(RenderFenceSyncDepth::RhiSubmit, 1, Some(1));
        let result = RenderFenceResult::completed(&fence, 0, 1);

        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "simulated_sync_depth"));
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
