use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

use super::llm_source::generate_mock_project_patch_json;
use super::{
    LlmPatchSourceConfig, LlmPatchSourceKind, LlmPatchSourceResult, LlmPatchSourceStatus,
    LlmTransport, LlmTransportConfig, ReqwestAsyncTransport,
};

pub const LLM_CANCEL_JOIN_DEADLINE: Duration = Duration::from_secs(2);
pub const LLM_SESSION_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(5);
pub const LLM_DROP_JOIN_BUDGET: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LlmRequestId(pub String);

impl LlmRequestId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for LlmRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmLifecycleState {
    Idle,
    Starting,
    RunningGenerate,
    WaitingForMainThreadDecision,
    RunningRepair,
    Cancelling,
    CompletedJoined,
    FailedJoined,
    CancelledJoined,
    ShutdownJoinTimedOut,
}

impl LlmLifecycleState {
    pub fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Starting
                | Self::RunningGenerate
                | Self::WaitingForMainThreadDecision
                | Self::RunningRepair
                | Self::Cancelling
        )
    }

    pub fn is_joined_terminal(self) -> bool {
        matches!(
            self,
            Self::CompletedJoined | Self::FailedJoined | Self::CancelledJoined
        )
    }

    pub fn accepts_new_request(self) -> bool {
        self == Self::Idle || self.is_joined_terminal()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LlmAttemptDecision {
    Complete,
    Fail { diagnostic_summary: String },
    ContinueRepair { repair_spec: LlmRepairSpec },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmRepairSpec {
    pub candidate_json: String,
    pub import: super::ProjectPatchImportResult,
    pub maximum_candidate_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelSource {
    User,
    SessionShutdown,
    ControllerDrop,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTerminalStatus {
    Completed,
    Failed,
    Cancelled,
    ShutdownJoinTimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskJoinStatus {
    Joined,
    Panicked,
    JoinTimedOut,
    NotStarted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialOwnerStatus {
    Held,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmLocalExecutionStatus {
    NotStarted,
    Running,
    Stopped,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmRemoteExecutionStatus {
    NotStarted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTransportCancelCapability {
    AsyncAbort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmLifecycleDiagnostic {
    pub code: String,
    pub message: String,
}

impl LlmLifecycleDiagnostic {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Default, PartialEq, Eq)]
pub struct LlmCredentialLease(Option<Zeroizing<String>>);

impl LlmCredentialLease {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self((!value.is_empty()).then(|| Zeroizing::new(value)))
    }

    pub fn is_present(&self) -> bool {
        self.0.is_some()
    }

    pub(crate) fn expose(&self) -> Option<&str> {
        self.0.as_deref().map(String::as_str)
    }
}

impl fmt::Debug for LlmCredentialLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(if self.is_present() {
            "LlmCredentialLease([REDACTED])"
        } else {
            "LlmCredentialLease(None)"
        })
    }
}

#[derive(Debug)]
pub(crate) struct LlmLifecycleStateMachine {
    state: LlmLifecycleState,
    terminal_committed: bool,
}

impl Default for LlmLifecycleStateMachine {
    fn default() -> Self {
        Self {
            state: LlmLifecycleState::Idle,
            terminal_committed: false,
        }
    }
}

impl LlmLifecycleStateMachine {
    pub(crate) fn state(&self) -> LlmLifecycleState {
        self.state
    }

    pub(crate) fn start(&mut self) -> Result<(), LlmLifecycleDiagnostic> {
        if !self.state.accepts_new_request()
            || self.state == LlmLifecycleState::ShutdownJoinTimedOut
        {
            return Err(LlmLifecycleDiagnostic::new(
                "llm_request_controller.busy",
                "The LLM request controller is not idle.",
            ));
        }
        self.state = LlmLifecycleState::Starting;
        self.terminal_committed = false;
        Ok(())
    }

    pub(crate) fn running_generate(&mut self) -> Result<(), LlmLifecycleDiagnostic> {
        self.transition_from(
            &[LlmLifecycleState::Starting],
            LlmLifecycleState::RunningGenerate,
        )
    }

    pub(crate) fn waiting_for_decision(&mut self) -> Result<(), LlmLifecycleDiagnostic> {
        self.transition_from(
            &[
                LlmLifecycleState::RunningGenerate,
                LlmLifecycleState::RunningRepair,
            ],
            LlmLifecycleState::WaitingForMainThreadDecision,
        )
    }

    pub(crate) fn resolve_attempt(
        &mut self,
        decision: &LlmAttemptDecision,
    ) -> Result<(), LlmLifecycleDiagnostic> {
        if self.state != LlmLifecycleState::WaitingForMainThreadDecision {
            return Err(self.invalid_transition());
        }
        match decision {
            LlmAttemptDecision::Complete => {
                self.commit_terminal(LlmLifecycleState::CompletedJoined)
            }
            LlmAttemptDecision::Fail { .. } => {
                self.commit_terminal(LlmLifecycleState::FailedJoined)
            }
            LlmAttemptDecision::ContinueRepair { .. } => {
                self.state = LlmLifecycleState::RunningRepair;
                Ok(())
            }
        }
    }

    pub(crate) fn request_cancel(&mut self) -> Result<(), LlmLifecycleDiagnostic> {
        if self.terminal_committed || self.state.is_joined_terminal() {
            return Err(LlmLifecycleDiagnostic::new(
                "llm_request_controller.request_not_found",
                "The request already reached a joined terminal state.",
            ));
        }
        if !self.state.is_busy() {
            return Err(self.invalid_transition());
        }
        self.state = LlmLifecycleState::Cancelling;
        Ok(())
    }

    pub(crate) fn cancelled_joined(&mut self) -> Result<(), LlmLifecycleDiagnostic> {
        if self.state != LlmLifecycleState::Cancelling {
            return Err(self.invalid_transition());
        }
        self.commit_terminal(LlmLifecycleState::CancelledJoined)
    }

    pub(crate) fn failed_joined(&mut self) -> Result<(), LlmLifecycleDiagnostic> {
        if !matches!(
            self.state,
            LlmLifecycleState::Starting
                | LlmLifecycleState::RunningGenerate
                | LlmLifecycleState::WaitingForMainThreadDecision
                | LlmLifecycleState::RunningRepair
                | LlmLifecycleState::Cancelling
        ) {
            return Err(self.invalid_transition());
        }
        self.commit_terminal(LlmLifecycleState::FailedJoined)
    }

    pub(crate) fn shutdown_join_timed_out(&mut self) {
        self.state = LlmLifecycleState::ShutdownJoinTimedOut;
        self.terminal_committed = true;
    }

    fn commit_terminal(
        &mut self,
        terminal: LlmLifecycleState,
    ) -> Result<(), LlmLifecycleDiagnostic> {
        if self.terminal_committed {
            return Err(self.invalid_transition());
        }
        self.state = terminal;
        self.terminal_committed = true;
        Ok(())
    }

    fn transition_from(
        &mut self,
        allowed: &[LlmLifecycleState],
        next: LlmLifecycleState,
    ) -> Result<(), LlmLifecycleDiagnostic> {
        if !allowed.contains(&self.state) || self.terminal_committed {
            return Err(self.invalid_transition());
        }
        self.state = next;
        Ok(())
    }

    fn invalid_transition(&self) -> LlmLifecycleDiagnostic {
        LlmLifecycleDiagnostic::new(
            "llm_request_controller.invalid_transition",
            format!("Invalid LLM lifecycle transition from {:?}.", self.state),
        )
    }
}

pub struct LlmRequestSpec {
    pub request_id: LlmRequestId,
    pub prompt: String,
    pub context_json: String,
    pub config: LlmPatchSourceConfig,
}

#[derive(Debug)]
pub enum LlmRequestEvent {
    AttemptJoined {
        request_id: LlmRequestId,
        attempt_index: u8,
        result: LlmPatchSourceResult,
    },
    CancelledJoined {
        receipt: LlmCancelReceipt,
    },
    FailedJoined {
        request_id: LlmRequestId,
        diagnostic: LlmLifecycleDiagnostic,
        task_join_status: LlmTaskJoinStatus,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmCancelReceipt {
    pub request_id: LlmRequestId,
    pub accepted: bool,
    pub state: LlmLifecycleState,
    pub cancel_source: CancelSource,
    pub transport_abort_requested: bool,
    pub transport_abort_observed: bool,
    pub task_join_status: LlmTaskJoinStatus,
    pub credential_owner_status: CredentialOwnerStatus,
    pub local_execution_status: LlmLocalExecutionStatus,
    pub remote_execution_status: LlmRemoteExecutionStatus,
    pub cancel_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmShutdownReceipt {
    pub state: LlmLifecycleState,
    pub task_join_status: LlmTaskJoinStatus,
    pub active_task_count: usize,
    pub reaper_count: usize,
    pub shutdown_latency_ms: u64,
    pub diagnostic: Option<LlmLifecycleDiagnostic>,
}

struct LlmRequestOwnedData {
    original_prompt: String,
    context_json: String,
    transport_config: LlmTransportConfig,
    credential: LlmCredentialLease,
}

struct WorkerOutput {
    owned: LlmRequestOwnedData,
    result: LlmPatchSourceResult,
}

struct ActiveLlmRequest {
    request_id: LlmRequestId,
    attempt_index: u8,
    owned: Option<LlmRequestOwnedData>,
    cancellation: CancellationToken,
    receiver: Receiver<WorkerOutput>,
    task: Option<JoinHandle<()>>,
    pending_output: Option<WorkerOutput>,
    cancel_source: CancelSource,
    cancel_started: Option<Instant>,
    request_may_have_started_remotely: bool,
}

pub struct LlmAsyncExecutor {
    runtime: Runtime,
    active_tasks: Arc<AtomicUsize>,
    reapers: Mutex<Vec<JoinHandle<()>>>,
}

impl fmt::Debug for LlmAsyncExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LlmAsyncExecutor")
            .field("active_task_count", &self.active_task_count())
            .field("reaper_count", &self.reaper_count())
            .finish()
    }
}

impl LlmAsyncExecutor {
    fn new(worker_threads: usize, thread_name: &'static str) -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(worker_threads)
                .thread_name(thread_name)
                .enable_all()
                .build()
                .expect("editor LLM async executor must initialize"),
            active_tasks: Arc::new(AtomicUsize::new(0)),
            reapers: Mutex::new(Vec::new()),
        }
    }

    pub fn process_owned() -> Arc<Self> {
        static EXECUTOR: OnceLock<Arc<LlmAsyncExecutor>> = OnceLock::new();
        EXECUTOR
            .get_or_init(|| Arc::new(Self::new(2, "aife-llm")))
            .clone()
    }

    #[cfg(test)]
    fn isolated_for_test() -> Arc<Self> {
        Arc::new(Self::new(1, "aife-llm-test"))
    }

    fn spawn_request<F>(&self, future: F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.active_tasks.fetch_add(1, Ordering::SeqCst);
        let active_tasks = self.active_tasks.clone();
        struct ActiveGuard(Arc<AtomicUsize>);
        impl Drop for ActiveGuard {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let guard = ActiveGuard(active_tasks);
        self.runtime.spawn(async move {
            let _guard = guard;
            future.await;
        })
    }

    fn join_finished(&self, handle: JoinHandle<()>) -> Result<(), tokio::task::JoinError> {
        self.runtime.block_on(handle)
    }

    fn join_with_deadline(
        &self,
        handle: &mut JoinHandle<()>,
        deadline: Duration,
    ) -> Result<Result<(), tokio::task::JoinError>, tokio::time::error::Elapsed> {
        self.runtime
            .block_on(async { tokio::time::timeout(deadline, handle).await })
    }

    fn transfer_to_reaper(&self, request_handle: JoinHandle<()>) {
        let reaper = self.runtime.spawn(async move {
            let _ = request_handle.await;
        });
        self.reapers
            .lock()
            .expect("LLM reaper inventory poisoned")
            .push(reaper);
    }

    pub fn active_task_count(&self) -> usize {
        self.active_tasks.load(Ordering::SeqCst)
    }

    pub fn reaper_count(&self) -> usize {
        self.reapers
            .lock()
            .expect("LLM reaper inventory poisoned")
            .len()
    }

    pub fn drain_reapers(&self, deadline: Duration) -> bool {
        let started = Instant::now();
        let mut handles = {
            let mut inventory = self.reapers.lock().expect("LLM reaper inventory poisoned");
            std::mem::take(&mut *inventory)
        };
        let mut pending = Vec::new();
        for mut handle in handles.drain(..) {
            let remaining = deadline.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                pending.push(handle);
                continue;
            }
            if self.join_with_deadline(&mut handle, remaining).is_err() {
                pending.push(handle);
            }
        }
        let drained = pending.is_empty();
        self.reapers
            .lock()
            .expect("LLM reaper inventory poisoned")
            .extend(pending);
        drained
    }
}

pub struct LlmRequestController {
    executor: Arc<LlmAsyncExecutor>,
    transport: Arc<dyn LlmTransport>,
    lifecycle: LlmLifecycleStateMachine,
    active: Option<ActiveLlmRequest>,
    pending_events: Vec<LlmRequestEvent>,
}

impl fmt::Debug for LlmRequestController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LlmRequestController")
            .field("state", &self.lifecycle.state())
            .field(
                "request_id",
                &self.active.as_ref().map(|active| &active.request_id),
            )
            .finish()
    }
}

impl Default for LlmRequestController {
    fn default() -> Self {
        Self::new_with_transport(
            LlmAsyncExecutor::process_owned(),
            Arc::new(ReqwestAsyncTransport),
        )
    }
}

impl LlmRequestController {
    pub(crate) fn new_with_transport(
        executor: Arc<LlmAsyncExecutor>,
        transport: Arc<dyn LlmTransport>,
    ) -> Self {
        Self {
            executor,
            transport,
            lifecycle: LlmLifecycleStateMachine::default(),
            active: None,
            pending_events: Vec::new(),
        }
    }

    pub fn state(&self) -> LlmLifecycleState {
        self.lifecycle.state()
    }

    pub fn is_busy(&self) -> bool {
        self.lifecycle.state().is_busy()
    }

    pub fn request_id(&self) -> Option<&LlmRequestId> {
        self.active.as_ref().map(|active| &active.request_id)
    }

    pub fn start(&mut self, spec: LlmRequestSpec) -> Result<LlmRequestId, LlmLifecycleDiagnostic> {
        if self.active.is_some() {
            return Err(LlmLifecycleDiagnostic::new(
                "llm_request_controller.busy",
                "An LLM request is already owned by this controller.",
            ));
        }
        self.lifecycle.start()?;
        self.lifecycle.running_generate()?;
        let request_id = spec.request_id.clone();
        let (transport_config, credential) = spec.config.into_transport_parts();
        let owned = LlmRequestOwnedData {
            original_prompt: spec.prompt,
            context_json: spec.context_json,
            transport_config,
            credential,
        };
        self.active = Some(ActiveLlmRequest {
            request_id: request_id.clone(),
            attempt_index: 0,
            owned: Some(owned),
            cancellation: CancellationToken::new(),
            receiver: mpsc::channel().1,
            task: None,
            pending_output: None,
            cancel_source: CancelSource::None,
            cancel_started: None,
            request_may_have_started_remotely: false,
        });
        self.spawn_current_attempt(None)?;
        Ok(request_id)
    }

    pub fn poll(&mut self) -> Vec<LlmRequestEvent> {
        let mut events = std::mem::take(&mut self.pending_events);
        let Some(active) = self.active.as_mut() else {
            return events;
        };

        if self.lifecycle.state() == LlmLifecycleState::Cancelling
            && active
                .cancel_started
                .is_some_and(|started| started.elapsed() >= LLM_CANCEL_JOIN_DEADLINE)
            && active.task.as_ref().is_some_and(|task| !task.is_finished())
        {
            if let Some(task) = active.task.as_ref() {
                task.abort();
            }
        }

        if active.pending_output.is_none() {
            match active.receiver.try_recv() {
                Ok(output) => active.pending_output = Some(output),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    if active.task.as_ref().is_some_and(JoinHandle::is_finished) {
                        let request_id = active.request_id.clone();
                        let task = active.task.take().expect("finished task must exist");
                        let join = self.executor.join_finished(task);
                        if self.lifecycle.state() == LlmLifecycleState::Cancelling
                            && join.as_ref().is_err_and(|error| error.is_cancelled())
                        {
                            let mut receipt = Self::cancelled_receipt(active, false);
                            receipt.state = LlmLifecycleState::CancelledJoined;
                            receipt.task_join_status = LlmTaskJoinStatus::Joined;
                            receipt.credential_owner_status = CredentialOwnerStatus::Released;
                            receipt.local_execution_status = LlmLocalExecutionStatus::Stopped;
                            self.lifecycle.cancelled_joined().ok();
                            self.active = None;
                            events.push(LlmRequestEvent::CancelledJoined { receipt });
                            return events;
                        }
                        let (code, join_status) = if join.is_err_and(|error| error.is_panic()) {
                            (
                                "llm_request_controller.task_panicked",
                                LlmTaskJoinStatus::Panicked,
                            )
                        } else {
                            (
                                "llm_request_controller.task_join_failed",
                                LlmTaskJoinStatus::Joined,
                            )
                        };
                        self.lifecycle.failed_joined().ok();
                        self.active = None;
                        events.push(LlmRequestEvent::FailedJoined {
                            request_id,
                            diagnostic: LlmLifecycleDiagnostic::new(
                                code,
                                "The LLM request task ended without a result.",
                            ),
                            task_join_status: join_status,
                        });
                        return events;
                    }
                }
            }
        }

        let ready_to_join = active.pending_output.is_some()
            && active.task.as_ref().is_some_and(JoinHandle::is_finished);
        if !ready_to_join {
            return events;
        }

        let mut active = self.active.take().expect("active request must exist");
        let output = active
            .pending_output
            .take()
            .expect("ready worker output must exist");
        let task = active.task.take().expect("ready worker task must exist");
        let join = self.executor.join_finished(task);
        if let Err(error) = join {
            let join_status = if error.is_panic() {
                LlmTaskJoinStatus::Panicked
            } else {
                LlmTaskJoinStatus::Joined
            };
            self.lifecycle.failed_joined().ok();
            events.push(LlmRequestEvent::FailedJoined {
                request_id: active.request_id,
                diagnostic: LlmLifecycleDiagnostic::new(
                    "llm_request_controller.task_join_failed",
                    "The LLM request task could not be joined.",
                ),
                task_join_status: join_status,
            });
            return events;
        }

        active.owned = Some(output.owned);
        if self.lifecycle.state() == LlmLifecycleState::Cancelling {
            let transport_abort_observed = output.result.status == LlmPatchSourceStatus::Cancelled;
            let receipt = Self::cancelled_receipt(&active, transport_abort_observed);
            active.owned.take();
            self.lifecycle.cancelled_joined().ok();
            let mut receipt = receipt;
            receipt.state = LlmLifecycleState::CancelledJoined;
            receipt.task_join_status = LlmTaskJoinStatus::Joined;
            receipt.credential_owner_status = CredentialOwnerStatus::Released;
            receipt.local_execution_status = LlmLocalExecutionStatus::Stopped;
            self.pending_events
                .push(LlmRequestEvent::CancelledJoined { receipt });
            events.append(&mut self.pending_events);
            return events;
        }

        self.lifecycle.waiting_for_decision().ok();
        let request_id = active.request_id.clone();
        let attempt_index = active.attempt_index;
        self.active = Some(active);
        events.push(LlmRequestEvent::AttemptJoined {
            request_id,
            attempt_index,
            result: output.result,
        });
        events
    }

    pub fn resolve_attempt(
        &mut self,
        request_id: &LlmRequestId,
        decision: LlmAttemptDecision,
    ) -> Result<(), LlmLifecycleDiagnostic> {
        let active = self.active.as_ref().ok_or_else(|| {
            LlmLifecycleDiagnostic::new(
                "llm_request_controller.request_not_found",
                "The LLM request is no longer active.",
            )
        })?;
        if &active.request_id != request_id {
            return Err(LlmLifecycleDiagnostic::new(
                "llm_request_controller.request_not_found",
                "The LLM request id does not match the active request.",
            ));
        }
        match &decision {
            LlmAttemptDecision::ContinueRepair { repair_spec } => {
                let original_prompt = self
                    .active
                    .as_ref()
                    .and_then(|active| active.owned.as_ref())
                    .map(|owned| owned.original_prompt.as_str())
                    .ok_or_else(|| {
                        LlmLifecycleDiagnostic::new(
                            "llm_request_controller.invalid_transition",
                            "The request-owned prompt is unavailable for repair.",
                        )
                    })?;
                let prompt = super::build_project_patch_repair_prompt(
                    original_prompt,
                    &repair_spec.candidate_json,
                    &repair_spec.import,
                    repair_spec.maximum_candidate_bytes,
                );
                self.lifecycle.resolve_attempt(&decision)?;
                if let Some(active) = self.active.as_mut() {
                    active.attempt_index = active.attempt_index.saturating_add(1);
                }
                self.spawn_current_attempt(Some(prompt))
            }
            LlmAttemptDecision::Complete | LlmAttemptDecision::Fail { .. } => {
                self.lifecycle.resolve_attempt(&decision)?;
                self.active.take();
                Ok(())
            }
        }
    }

    pub fn cancel(&mut self, request_id: &LlmRequestId, source: CancelSource) -> LlmCancelReceipt {
        let Some(active) = self.active.as_mut() else {
            return LlmCancelReceipt {
                request_id: request_id.clone(),
                accepted: false,
                state: self.lifecycle.state(),
                cancel_source: source,
                transport_abort_requested: false,
                transport_abort_observed: false,
                task_join_status: LlmTaskJoinStatus::NotStarted,
                credential_owner_status: CredentialOwnerStatus::Released,
                local_execution_status: LlmLocalExecutionStatus::Stopped,
                remote_execution_status: LlmRemoteExecutionStatus::NotStarted,
                cancel_latency_ms: None,
            };
        };
        if &active.request_id != request_id {
            return LlmCancelReceipt {
                request_id: request_id.clone(),
                accepted: false,
                state: self.lifecycle.state(),
                cancel_source: source,
                transport_abort_requested: false,
                transport_abort_observed: false,
                task_join_status: LlmTaskJoinStatus::NotStarted,
                credential_owner_status: CredentialOwnerStatus::Held,
                local_execution_status: LlmLocalExecutionStatus::Running,
                remote_execution_status: LlmRemoteExecutionStatus::Unknown,
                cancel_latency_ms: None,
            };
        }
        if self.lifecycle.state().is_joined_terminal() {
            return Self::cancelled_receipt(active, false);
        }
        let _ = self.lifecycle.request_cancel();
        active.cancel_source = source;
        active.cancel_started.get_or_insert_with(Instant::now);
        active.cancellation.cancel();
        if active.task.is_none() {
            let mut active = self.active.take().expect("active request must exist");
            let mut receipt = Self::cancelled_receipt(&active, false);
            active.owned.take();
            self.lifecycle.cancelled_joined().ok();
            receipt.state = LlmLifecycleState::CancelledJoined;
            receipt.task_join_status = LlmTaskJoinStatus::Joined;
            receipt.credential_owner_status = CredentialOwnerStatus::Released;
            self.pending_events.push(LlmRequestEvent::CancelledJoined {
                receipt: receipt.clone(),
            });
            return receipt;
        }
        Self::cancelled_receipt(active, false)
    }

    pub fn shutdown(&mut self, deadline: Duration) -> LlmShutdownReceipt {
        let started = Instant::now();
        if let Some(request_id) = self.request_id().cloned() {
            self.cancel(&request_id, CancelSource::SessionShutdown);
        }
        let mut join_status = LlmTaskJoinStatus::NotStarted;
        let mut diagnostic = None;
        if let Some(mut active) = self.active.take() {
            if let Some(mut handle) = active.task.take() {
                match self.executor.join_with_deadline(&mut handle, deadline) {
                    Ok(Ok(())) => join_status = LlmTaskJoinStatus::Joined,
                    Ok(Err(error)) => {
                        join_status = if error.is_panic() {
                            LlmTaskJoinStatus::Panicked
                        } else {
                            LlmTaskJoinStatus::Joined
                        };
                        diagnostic = Some(LlmLifecycleDiagnostic::new(
                            "llm_request_controller.task_join_failed",
                            "The LLM request task failed during shutdown join.",
                        ));
                    }
                    Err(_) => {
                        handle.abort();
                        self.executor.transfer_to_reaper(handle);
                        join_status = LlmTaskJoinStatus::JoinTimedOut;
                        self.lifecycle.shutdown_join_timed_out();
                        diagnostic = Some(LlmLifecycleDiagnostic::new(
                            "llm_request_controller.shutdown_join_timed_out",
                            "The LLM request task did not join before the shutdown deadline.",
                        ));
                    }
                }
            }
            active.owned.take();
        }
        let remaining = deadline.saturating_sub(started.elapsed());
        let reapers_drained = self.executor.drain_reapers(remaining);
        if !reapers_drained && diagnostic.is_none() {
            self.lifecycle.shutdown_join_timed_out();
            join_status = LlmTaskJoinStatus::JoinTimedOut;
            diagnostic = Some(LlmLifecycleDiagnostic::new(
                "llm_request_controller.shutdown_join_timed_out",
                "The LLM task reaper did not drain before the shutdown deadline.",
            ));
        }
        let state = if diagnostic.is_some() {
            LlmLifecycleState::ShutdownJoinTimedOut
        } else if join_status == LlmTaskJoinStatus::NotStarted {
            self.lifecycle.state()
        } else {
            self.lifecycle.cancelled_joined().ok();
            LlmLifecycleState::CancelledJoined
        };
        LlmShutdownReceipt {
            state,
            task_join_status: join_status,
            active_task_count: usize::from(self.active.is_some()),
            reaper_count: usize::from(!reapers_drained),
            shutdown_latency_ms: started.elapsed().as_millis() as u64,
            diagnostic,
        }
    }

    pub fn executor(&self) -> &Arc<LlmAsyncExecutor> {
        &self.executor
    }

    fn spawn_current_attempt(
        &mut self,
        attempt_prompt: Option<String>,
    ) -> Result<(), LlmLifecycleDiagnostic> {
        let active = self.active.as_mut().ok_or_else(|| {
            LlmLifecycleDiagnostic::new(
                "llm_request_controller.request_not_found",
                "The LLM request is no longer active.",
            )
        })?;
        let owned = active.owned.take().ok_or_else(|| {
            LlmLifecycleDiagnostic::new(
                "llm_request_controller.invalid_transition",
                "The LLM request-owned data is already in flight.",
            )
        })?;
        let prompt = attempt_prompt.unwrap_or_else(|| owned.original_prompt.clone());
        let cancellation = CancellationToken::new();
        active.cancellation = cancellation.clone();
        active.request_may_have_started_remotely =
            owned.transport_config.source_kind == LlmPatchSourceKind::OpenAiCompatible;
        let (sender, receiver) = mpsc::channel();
        active.receiver = receiver;
        let transport = self.transport.clone();
        active.task = Some(self.executor.spawn_request(async move {
            let result = match owned.transport_config.source_kind {
                LlmPatchSourceKind::Mock => {
                    generate_mock_project_patch_json(&owned.transport_config, &prompt)
                }
                LlmPatchSourceKind::OpenAiCompatible => {
                    transport
                        .execute(
                            &owned.transport_config,
                            &owned.credential,
                            &prompt,
                            &owned.context_json,
                            cancellation,
                        )
                        .await
                }
            };
            let _ = sender.send(WorkerOutput { owned, result });
        }));
        Ok(())
    }

    fn cancelled_receipt(
        active: &ActiveLlmRequest,
        transport_abort_observed: bool,
    ) -> LlmCancelReceipt {
        LlmCancelReceipt {
            request_id: active.request_id.clone(),
            accepted: true,
            state: LlmLifecycleState::Cancelling,
            cancel_source: active.cancel_source,
            transport_abort_requested: true,
            transport_abort_observed,
            task_join_status: LlmTaskJoinStatus::NotStarted,
            credential_owner_status: CredentialOwnerStatus::Held,
            local_execution_status: LlmLocalExecutionStatus::Running,
            remote_execution_status: if active.request_may_have_started_remotely {
                LlmRemoteExecutionStatus::Unknown
            } else {
                LlmRemoteExecutionStatus::NotStarted
            },
            cancel_latency_ms: active
                .cancel_started
                .map(|started| started.elapsed().as_millis() as u64),
        }
    }
}

impl Drop for LlmRequestController {
    fn drop(&mut self) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        active.cancel_source = CancelSource::ControllerDrop;
        active.cancellation.cancel();
        active.owned.take();
        let Some(mut handle) = active.task.take() else {
            return;
        };
        handle.abort();
        if self
            .executor
            .join_with_deadline(&mut handle, LLM_DROP_JOIN_BUDGET)
            .is_err()
        {
            self.executor.transfer_to_reaper(handle);
        }
    }
}

pub fn validate_llm_join_timeout_fail_closed() -> bool {
    struct NeverCompletesTransport;
    impl LlmTransport for NeverCompletesTransport {
        fn execute<'a>(
            &'a self,
            _config: &'a LlmTransportConfig,
            _credential: &'a LlmCredentialLease,
            _prompt: &'a str,
            _context_json: &'a str,
            _cancellation: CancellationToken,
        ) -> std::pin::Pin<Box<dyn Future<Output = LlmPatchSourceResult> + Send + 'a>> {
            Box::pin(std::future::pending())
        }
    }

    let executor = Arc::new(LlmAsyncExecutor::new(1, "aife-llm-timeout-validation"));
    let mut controller = LlmRequestController::new_with_transport(
        executor.clone(),
        Arc::new(NeverCompletesTransport),
    );
    let mut config = LlmPatchSourceConfig::deterministic_mock();
    config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
    let started = controller.start(LlmRequestSpec {
        request_id: LlmRequestId::new("join-timeout-validation"),
        prompt: "validation".to_string(),
        context_json: "{}".to_string(),
        config,
    });
    if started.is_err() {
        return false;
    }
    let receipt = controller.shutdown(Duration::from_millis(1));
    let failed_closed = receipt.state == LlmLifecycleState::ShutdownJoinTimedOut
        && receipt.task_join_status == LlmTaskJoinStatus::JoinTimedOut
        && receipt.diagnostic.as_ref().is_some_and(|diagnostic| {
            diagnostic.code == "llm_request_controller.shutdown_join_timed_out"
        });
    let drained = executor.drain_reapers(Duration::from_secs(1));
    failed_closed && drained && executor.active_task_count() == 0 && executor.reaper_count() == 0
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::thread;

    use super::*;
    use crate::project_patch::llm_transport::test_support::ControllableLoopbackTransport;

    struct PanickingTransport {
        started: mpsc::Sender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }

    impl LlmTransport for PanickingTransport {
        fn execute<'a>(
            &'a self,
            _config: &'a LlmTransportConfig,
            _credential: &'a LlmCredentialLease,
            _prompt: &'a str,
            _context_json: &'a str,
            _cancellation: CancellationToken,
        ) -> std::pin::Pin<Box<dyn Future<Output = LlmPatchSourceResult> + Send + 'a>> {
            Box::pin(async {
                let _ = self.started.send(());
                self.release
                    .lock()
                    .expect("panic transport release gate poisoned")
                    .recv_timeout(Duration::from_secs(5))
                    .expect("panic transport was not released");
                std::panic::resume_unwind(Box::new("intentional controller panic fixture"))
            })
        }
    }

    struct IgnoresCancellationTransport;

    impl LlmTransport for IgnoresCancellationTransport {
        fn execute<'a>(
            &'a self,
            _config: &'a LlmTransportConfig,
            _credential: &'a LlmCredentialLease,
            _prompt: &'a str,
            _context_json: &'a str,
            _cancellation: CancellationToken,
        ) -> std::pin::Pin<Box<dyn Future<Output = LlmPatchSourceResult> + Send + 'a>> {
            Box::pin(std::future::pending())
        }
    }

    fn poll_until_event(controller: &mut LlmRequestController) -> LlmRequestEvent {
        for _ in 0..500 {
            if let Some(event) = controller.poll().into_iter().next() {
                return event;
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("LLM request controller did not produce an event");
    }

    #[test]
    fn llm_request_lifecycle_generate_complete_is_joined_terminal() {
        let mut lifecycle = LlmLifecycleStateMachine::default();
        lifecycle.start().unwrap();
        lifecycle.running_generate().unwrap();
        lifecycle.waiting_for_decision().unwrap();
        lifecycle
            .resolve_attempt(&LlmAttemptDecision::Complete)
            .unwrap();
        assert_eq!(lifecycle.state(), LlmLifecycleState::CompletedJoined);
        assert!(lifecycle.state().accepts_new_request());
    }

    #[test]
    fn llm_request_lifecycle_repair_uses_same_decision_state() {
        let mut lifecycle = LlmLifecycleStateMachine::default();
        lifecycle.start().unwrap();
        lifecycle.running_generate().unwrap();
        lifecycle.waiting_for_decision().unwrap();
        lifecycle
            .resolve_attempt(&LlmAttemptDecision::ContinueRepair {
                repair_spec: LlmRepairSpec {
                    candidate_json: "{bad".to_string(),
                    import: super::super::ProjectPatchImportResult {
                        schema_version: "project-patch-import-result.v1".to_string(),
                        source_kind: super::super::ProjectPatchImportSourceKind::AiStructuredOutput,
                        source_label: "test".to_string(),
                        parse_status: super::super::ProjectPatchImportParseStatus::Rejected,
                        parsed_patch: None,
                        schema_diagnostics: Vec::new(),
                        capability_diagnostics: Vec::new(),
                        validation: None,
                        review: None,
                        proposal_id: None,
                        next_actions: Vec::new(),
                    },
                    maximum_candidate_bytes: 1024,
                },
            })
            .unwrap();
        assert_eq!(lifecycle.state(), LlmLifecycleState::RunningRepair);
        lifecycle.waiting_for_decision().unwrap();
        lifecycle
            .resolve_attempt(&LlmAttemptDecision::Complete)
            .unwrap();
        assert_eq!(lifecycle.state(), LlmLifecycleState::CompletedJoined);
    }

    #[test]
    fn llm_request_lifecycle_cancel_is_busy_until_joined() {
        let mut lifecycle = LlmLifecycleStateMachine::default();
        lifecycle.start().unwrap();
        lifecycle.running_generate().unwrap();
        lifecycle.request_cancel().unwrap();
        assert!(lifecycle.state().is_busy());
        assert!(lifecycle.start().is_err());
        lifecycle.cancelled_joined().unwrap();
        assert_eq!(lifecycle.state(), LlmLifecycleState::CancelledJoined);
    }

    #[test]
    fn llm_request_lifecycle_single_terminal_rejects_cancel_after_completion() {
        let mut lifecycle = LlmLifecycleStateMachine::default();
        lifecycle.start().unwrap();
        lifecycle.running_generate().unwrap();
        lifecycle.waiting_for_decision().unwrap();
        lifecycle
            .resolve_attempt(&LlmAttemptDecision::Complete)
            .unwrap();
        let error = lifecycle.request_cancel().unwrap_err();
        assert_eq!(error.code, "llm_request_controller.request_not_found");
    }

    #[test]
    fn llm_request_lifecycle_join_timeout_is_not_reusable() {
        let mut lifecycle = LlmLifecycleStateMachine::default();
        lifecycle.start().unwrap();
        lifecycle.running_generate().unwrap();
        lifecycle.shutdown_join_timed_out();
        assert_eq!(lifecycle.state(), LlmLifecycleState::ShutdownJoinTimedOut);
        assert!(!lifecycle.state().accepts_new_request());
        assert!(lifecycle.start().is_err());
    }

    #[test]
    fn llm_credential_debug_never_contains_secret() {
        let credential = LlmCredentialLease::new("gate-a-secret");
        let debug = format!("{credential:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("gate-a-secret"));
    }

    #[test]
    fn llm_request_controller_attempt_joins_before_event() {
        let executor = LlmAsyncExecutor::isolated_for_test();
        let transport = Arc::new(ControllableLoopbackTransport::default());
        let entered = transport.entered_probe();
        let mut controller =
            LlmRequestController::new_with_transport(executor.clone(), transport.clone());
        let request_id = controller
            .start(LlmRequestSpec {
                request_id: LlmRequestId::new("controller-joined"),
                prompt: "prompt".to_string(),
                context_json: "{}".to_string(),
                config: {
                    let mut config = LlmPatchSourceConfig::deterministic_mock();
                    config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
                    config
                },
            })
            .unwrap();
        while !entered.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        transport.release();
        let event = poll_until_event(&mut controller);
        assert!(matches!(
            event,
            LlmRequestEvent::AttemptJoined { request_id: id, attempt_index: 0, .. }
                if id == request_id
        ));
        assert_eq!(executor.active_task_count(), 0);
        assert_eq!(
            controller.state(),
            LlmLifecycleState::WaitingForMainThreadDecision
        );
        controller
            .resolve_attempt(&request_id, LlmAttemptDecision::Complete)
            .unwrap();
        assert_eq!(controller.state(), LlmLifecycleState::CompletedJoined);
    }

    #[test]
    fn llm_request_controller_cancel_stays_busy_until_joined() {
        let executor = LlmAsyncExecutor::isolated_for_test();
        let transport = Arc::new(ControllableLoopbackTransport::default());
        let entered = transport.entered_probe();
        let mut controller = LlmRequestController::new_with_transport(executor.clone(), transport);
        let request_id = controller
            .start(LlmRequestSpec {
                request_id: LlmRequestId::new("controller-cancel"),
                prompt: "prompt".to_string(),
                context_json: "{}".to_string(),
                config: {
                    let mut config = LlmPatchSourceConfig::deterministic_mock();
                    config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
                    config
                },
            })
            .unwrap();
        while !entered.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        let receipt = controller.cancel(&request_id, CancelSource::User);
        assert!(receipt.accepted);
        assert_eq!(receipt.state, LlmLifecycleState::Cancelling);
        assert!(controller.is_busy());
        assert!(controller
            .start(LlmRequestSpec {
                request_id: LlmRequestId::new("overlap"),
                prompt: String::new(),
                context_json: String::new(),
                config: LlmPatchSourceConfig::deterministic_mock(),
            })
            .is_err());
        let event = poll_until_event(&mut controller);
        let LlmRequestEvent::CancelledJoined { receipt } = event else {
            panic!("expected cancelled joined event");
        };
        assert_eq!(receipt.task_join_status, LlmTaskJoinStatus::Joined);
        assert_eq!(
            receipt.credential_owner_status,
            CredentialOwnerStatus::Released
        );
        assert!(receipt.transport_abort_observed);
        assert_eq!(controller.state(), LlmLifecycleState::CancelledJoined);
        assert_eq!(executor.active_task_count(), 0);
    }

    #[test]
    fn llm_request_controller_repair_reuses_request_id() {
        let executor = LlmAsyncExecutor::isolated_for_test();
        let transport = Arc::new(ControllableLoopbackTransport::default());
        let mut controller =
            LlmRequestController::new_with_transport(executor.clone(), transport.clone());
        let request_id = controller
            .start(LlmRequestSpec {
                request_id: LlmRequestId::new("controller-repair"),
                prompt: "prompt".to_string(),
                context_json: "{}".to_string(),
                config: {
                    let mut config = LlmPatchSourceConfig::deterministic_mock();
                    config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
                    config
                },
            })
            .unwrap();
        transport.release();
        let _ = poll_until_event(&mut controller);
        controller
            .resolve_attempt(
                &request_id,
                LlmAttemptDecision::ContinueRepair {
                    repair_spec: LlmRepairSpec {
                        candidate_json: "{bad".to_string(),
                        import: crate::ProjectPatchImportResult {
                            schema_version: "project-patch-import-result.v1".to_string(),
                            source_kind: crate::ProjectPatchImportSourceKind::AiStructuredOutput,
                            source_label: "test".to_string(),
                            parse_status: crate::ProjectPatchImportParseStatus::Rejected,
                            parsed_patch: None,
                            schema_diagnostics: Vec::new(),
                            capability_diagnostics: Vec::new(),
                            validation: None,
                            review: None,
                            proposal_id: None,
                            next_actions: Vec::new(),
                        },
                        maximum_candidate_bytes: 1024,
                    },
                },
            )
            .unwrap();
        transport.release();
        let event = poll_until_event(&mut controller);
        assert!(matches!(
            event,
            LlmRequestEvent::AttemptJoined { request_id: id, attempt_index: 1, .. }
                if id == request_id
        ));
        controller
            .resolve_attempt(&request_id, LlmAttemptDecision::Complete)
            .unwrap();
        assert_eq!(executor.active_task_count(), 0);
    }

    #[test]
    fn llm_executor_reaper_controller_drop_drains_without_detached_request() {
        let executor = LlmAsyncExecutor::isolated_for_test();
        let transport = Arc::new(ControllableLoopbackTransport::default());
        {
            let mut controller =
                LlmRequestController::new_with_transport(executor.clone(), transport);
            controller
                .start(LlmRequestSpec {
                    request_id: LlmRequestId::new("controller-drop"),
                    prompt: "prompt".to_string(),
                    context_json: "{}".to_string(),
                    config: {
                        let mut config = LlmPatchSourceConfig::deterministic_mock();
                        config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
                        config
                    },
                })
                .unwrap();
        }
        assert!(executor.drain_reapers(Duration::from_secs(1)));
        assert_eq!(executor.active_task_count(), 0);
        assert_eq!(executor.reaper_count(), 0);
    }

    #[test]
    fn llm_request_controller_task_panic_becomes_failed_joined() {
        let executor = LlmAsyncExecutor::isolated_for_test();
        let (started_sender, started_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let mut controller = LlmRequestController::new_with_transport(
            executor.clone(),
            Arc::new(PanickingTransport {
                started: started_sender,
                release: Mutex::new(release_receiver),
            }),
        );
        let mut config = LlmPatchSourceConfig::deterministic_mock();
        config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
        controller
            .start(LlmRequestSpec {
                request_id: LlmRequestId::new("controller-panic"),
                prompt: "panic".to_string(),
                context_json: "{}".to_string(),
                config,
            })
            .unwrap();
        started_receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("panic transport must start before terminal polling");
        release_sender
            .send(())
            .expect("panic transport release receiver must remain active");

        let event = poll_until_event(&mut controller);

        assert!(matches!(
            event,
            LlmRequestEvent::FailedJoined {
                task_join_status: LlmTaskJoinStatus::Panicked,
                ..
            }
        ));
        assert_eq!(controller.state(), LlmLifecycleState::FailedJoined);
        assert_eq!(executor.active_task_count(), 0);
    }

    #[test]
    fn llm_request_controller_cancel_deadline_aborts_and_joins_uncooperative_task() {
        let executor = LlmAsyncExecutor::isolated_for_test();
        let mut controller = LlmRequestController::new_with_transport(
            executor.clone(),
            Arc::new(IgnoresCancellationTransport),
        );
        let mut config = LlmPatchSourceConfig::deterministic_mock();
        config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
        let request_id = controller
            .start(LlmRequestSpec {
                request_id: LlmRequestId::new("controller-cancel-deadline"),
                prompt: "cancel".to_string(),
                context_json: "{}".to_string(),
                config,
            })
            .unwrap();
        controller.cancel(&request_id, CancelSource::User);
        controller
            .active
            .as_mut()
            .expect("active request")
            .cancel_started = Some(Instant::now() - LLM_CANCEL_JOIN_DEADLINE);

        let event = poll_until_event(&mut controller);

        let LlmRequestEvent::CancelledJoined { receipt } = event else {
            panic!("cancel deadline must produce cancelled joined");
        };
        assert_eq!(receipt.task_join_status, LlmTaskJoinStatus::Joined);
        assert!(!receipt.transport_abort_observed);
        assert_eq!(controller.state(), LlmLifecycleState::CancelledJoined);
        assert_eq!(executor.active_task_count(), 0);
    }
}
