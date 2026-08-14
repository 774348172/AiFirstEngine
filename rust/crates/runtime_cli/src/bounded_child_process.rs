use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::{io::AsRawHandle, process::CommandExt};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME},
    },
};

const READ_BUFFER_BYTES: usize = 16 * 1024;
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedChildProcessRequest {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub timeout: Duration,
    pub stdout_capture_limit_bytes: usize,
    pub stderr_capture_limit_bytes: usize,
    pub priority: BoundedChildProcessPriority,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundedChildProcessPriority {
    #[default]
    Normal,
    BelowNormal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedChildProcessPriorityEvidence {
    pub requested: BoundedChildProcessPriority,
    pub effective: Option<BoundedChildProcessPriority>,
    pub applied: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundedChildProcessExitReason {
    Completed,
    Failed,
    Cancelled,
    Timeout,
    WaitFailed,
    SpawnFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundedProcessOwnershipKind {
    WindowsJobObject,
    DirectChild,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundedProcessOwnershipEvidence {
    pub ownership_kind: BoundedProcessOwnershipKind,
    pub process_group_created: bool,
    pub root_process_bound: bool,
    pub termination_requested: bool,
    pub root_process_wait_completed: bool,
    pub process_group_release_completed: bool,
    pub output_readers_joined: bool,
}

impl Default for BoundedProcessOwnershipEvidence {
    fn default() -> Self {
        Self {
            ownership_kind: if cfg!(windows) {
                BoundedProcessOwnershipKind::WindowsJobObject
            } else {
                BoundedProcessOwnershipKind::DirectChild
            },
            process_group_created: false,
            root_process_bound: false,
            termination_requested: false,
            root_process_wait_completed: false,
            process_group_release_completed: false,
            output_readers_joined: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BoundedChildProcessCancellation {
    requested: Arc<AtomicBool>,
}

impl PartialEq for BoundedChildProcessCancellation {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.requested, &other.requested)
    }
}

impl Eq for BoundedChildProcessCancellation {}

impl BoundedChildProcessCancellation {
    pub fn request_cancel(&self) {
        self.requested.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundedChildProcessResult {
    pub process_id: Option<u32>,
    pub exit_reason: BoundedChildProcessExitReason,
    pub exit_code: Option<i32>,
    pub elapsed_ms: u128,
    pub stdout_summary: String,
    pub stderr_summary: String,
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub spawn_error: Option<String>,
    pub kill_error: Option<String>,
    pub wait_error: Option<String>,
    pub reader_join_error: Option<String>,
    #[serde(default)]
    pub ownership: BoundedProcessOwnershipEvidence,
    #[serde(default)]
    pub priority: BoundedChildProcessPriorityEvidence,
}

impl BoundedChildProcessResult {
    pub fn owned_process_cleanup_confirmed(&self) -> bool {
        if self.exit_reason == BoundedChildProcessExitReason::SpawnFailed
            && self.process_id.is_none()
        {
            return self.process_id.is_none() && self.spawn_error.is_some();
        }
        let ownership_closed = match self.ownership.ownership_kind {
            BoundedProcessOwnershipKind::WindowsJobObject => {
                self.ownership.process_group_created
                    && self.ownership.process_group_release_completed
            }
            BoundedProcessOwnershipKind::DirectChild => true,
        };
        self.process_id.is_some()
            && self.ownership.root_process_bound
            && self.ownership.root_process_wait_completed
            && ownership_closed
            && self.ownership.output_readers_joined
            && self.reader_join_error.is_none()
    }
}

#[derive(Debug)]
struct StreamCapture {
    retained: Vec<u8>,
    total_bytes: u64,
    read_error: Option<String>,
}

impl StreamCapture {
    fn empty() -> Self {
        Self {
            retained: Vec::new(),
            total_bytes: 0,
            read_error: None,
        }
    }
}

pub fn run_bounded_child_process(request: BoundedChildProcessRequest) -> BoundedChildProcessResult {
    run_bounded_child_process_impl(request, WaitFault::None, None)
}

pub fn run_bounded_child_process_cancellable(
    request: BoundedChildProcessRequest,
    cancellation: BoundedChildProcessCancellation,
) -> BoundedChildProcessResult {
    run_bounded_child_process_impl(request, WaitFault::None, Some(cancellation))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitFault {
    None,
    #[cfg(test)]
    InjectOnce,
}

fn run_bounded_child_process_impl(
    request: BoundedChildProcessRequest,
    wait_fault: WaitFault,
    cancellation: Option<BoundedChildProcessCancellation>,
) -> BoundedChildProcessResult {
    let started = Instant::now();
    let requested_priority = request.priority;
    let process_group = match ProcessGroup::create() {
        Ok(group) => group,
        Err(error) => return spawn_failed_result(started, error, requested_priority),
    };
    let mut ownership = BoundedProcessOwnershipEvidence {
        process_group_created: true,
        ..BoundedProcessOwnershipEvidence::default()
    };
    let mut command = Command::new(&request.executable);
    command
        .args(&request.args)
        .current_dir(&request.current_dir)
        .envs(request.environment)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_SUSPENDED);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return spawn_failed_result(started, error, requested_priority),
    };

    let process_id = child.id();
    let mut priority = BoundedChildProcessPriorityEvidence {
        requested: requested_priority,
        ..BoundedChildProcessPriorityEvidence::default()
    };
    if let Err(error) = process_group.bind_and_start(&child, &mut priority) {
        ownership.termination_requested = true;
        let mut exit_code = None;
        let mut kill_error = None;
        let mut wait_error = None;
        kill_and_reap_group(
            process_group,
            &mut child,
            &mut exit_code,
            &mut kill_error,
            &mut wait_error,
            &mut ownership,
        );
        ownership.output_readers_joined = true;
        return BoundedChildProcessResult {
            process_id: Some(process_id),
            exit_reason: BoundedChildProcessExitReason::SpawnFailed,
            exit_code,
            elapsed_ms: started.elapsed().as_millis(),
            stdout_summary: String::new(),
            stderr_summary: String::new(),
            stdout_total_bytes: 0,
            stderr_total_bytes: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            spawn_error: Some(format!("process ownership bind/start failed: {error}")),
            kill_error,
            wait_error,
            reader_join_error: None,
            ownership,
            priority,
        };
    }
    ownership.root_process_bound = true;
    let stdout_reader = spawn_reader(child.stdout.take(), request.stdout_capture_limit_bytes);
    let stderr_reader = spawn_reader(child.stderr.take(), request.stderr_capture_limit_bytes);
    let timeout = request.timeout.max(Duration::from_millis(1));
    let mut exit_code = None;
    let mut kill_error = None;
    let mut wait_error = None;
    let exit_reason;
    #[cfg(test)]
    let mut inject_wait_fault = matches!(wait_fault, WaitFault::InjectOnce);
    #[cfg(not(test))]
    let _ = wait_fault;

    loop {
        if cancellation
            .as_ref()
            .is_some_and(BoundedChildProcessCancellation::is_cancelled)
        {
            exit_reason = BoundedChildProcessExitReason::Cancelled;
            ownership.termination_requested = true;
            kill_and_reap_group(
                process_group,
                &mut child,
                &mut exit_code,
                &mut kill_error,
                &mut wait_error,
                &mut ownership,
            );
            break;
        }
        #[cfg(test)]
        let wait_result = if inject_wait_fault {
            inject_wait_fault = false;
            Err(std::io::Error::other(
                "injected bounded child process wait failure",
            ))
        } else {
            child.try_wait()
        };
        #[cfg(not(test))]
        let wait_result = child.try_wait();

        match wait_result {
            Ok(Some(status)) => {
                exit_code = status.code();
                exit_reason = if status.success() {
                    BoundedChildProcessExitReason::Completed
                } else {
                    BoundedChildProcessExitReason::Failed
                };
                ownership.root_process_wait_completed = true;
                release_process_group(process_group, &mut ownership, &mut kill_error);
                break;
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(WAIT_POLL_INTERVAL),
            Ok(None) => {
                exit_reason = BoundedChildProcessExitReason::Timeout;
                ownership.termination_requested = true;
                kill_and_reap_group(
                    process_group,
                    &mut child,
                    &mut exit_code,
                    &mut kill_error,
                    &mut wait_error,
                    &mut ownership,
                );
                break;
            }
            Err(error) => {
                exit_reason = BoundedChildProcessExitReason::WaitFailed;
                wait_error = Some(error.to_string());
                ownership.termination_requested = true;
                kill_and_reap_group(
                    process_group,
                    &mut child,
                    &mut exit_code,
                    &mut kill_error,
                    &mut wait_error,
                    &mut ownership,
                );
                break;
            }
        }
    }

    let (stdout, stdout_join_error) = join_reader(stdout_reader, "stdout");
    let (stderr, stderr_join_error) = join_reader(stderr_reader, "stderr");
    let reader_errors = [
        stdout_join_error,
        stderr_join_error,
        stdout
            .read_error
            .clone()
            .map(|error| format!("stdout read failed: {error}")),
        stderr
            .read_error
            .clone()
            .map(|error| format!("stderr read failed: {error}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    ownership.output_readers_joined = reader_errors.is_empty();

    BoundedChildProcessResult {
        process_id: Some(process_id),
        exit_reason,
        exit_code,
        elapsed_ms: started.elapsed().as_millis(),
        stdout_summary: String::from_utf8_lossy(&stdout.retained).into_owned(),
        stderr_summary: String::from_utf8_lossy(&stderr.retained).into_owned(),
        stdout_total_bytes: stdout.total_bytes,
        stderr_total_bytes: stderr.total_bytes,
        stdout_truncated: stdout.total_bytes > stdout.retained.len() as u64,
        stderr_truncated: stderr.total_bytes > stderr.retained.len() as u64,
        spawn_error: None,
        kill_error,
        wait_error,
        reader_join_error: (!reader_errors.is_empty()).then(|| reader_errors.join("; ")),
        ownership,
        priority,
    }
}

fn spawn_failed_result(
    started: Instant,
    error: impl ToString,
    requested_priority: BoundedChildProcessPriority,
) -> BoundedChildProcessResult {
    BoundedChildProcessResult {
        process_id: None,
        exit_reason: BoundedChildProcessExitReason::SpawnFailed,
        exit_code: None,
        elapsed_ms: started.elapsed().as_millis(),
        stdout_summary: String::new(),
        stderr_summary: String::new(),
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        spawn_error: Some(error.to_string()),
        kill_error: None,
        wait_error: None,
        reader_join_error: None,
        ownership: BoundedProcessOwnershipEvidence::default(),
        priority: BoundedChildProcessPriorityEvidence {
            requested: requested_priority,
            ..BoundedChildProcessPriorityEvidence::default()
        },
    }
}

fn spawn_reader(
    stream: Option<impl Read + Send + 'static>,
    capture_limit_bytes: usize,
) -> Option<JoinHandle<StreamCapture>> {
    stream.map(|mut stream| {
        thread::spawn(move || {
            let mut capture = StreamCapture {
                retained: Vec::with_capacity(capture_limit_bytes.min(READ_BUFFER_BYTES)),
                total_bytes: 0,
                read_error: None,
            };
            let mut buffer = [0_u8; READ_BUFFER_BYTES];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        capture.total_bytes = capture.total_bytes.saturating_add(count as u64);
                        let remaining = capture_limit_bytes.saturating_sub(capture.retained.len());
                        capture
                            .retained
                            .extend_from_slice(&buffer[..count.min(remaining)]);
                    }
                    Err(error) => {
                        capture.read_error = Some(error.to_string());
                        break;
                    }
                }
            }
            capture
        })
    })
}

fn kill_and_reap_group(
    process_group: ProcessGroup,
    child: &mut Child,
    exit_code: &mut Option<i32>,
    kill_error: &mut Option<String>,
    wait_error: &mut Option<String>,
    ownership: &mut BoundedProcessOwnershipEvidence,
) {
    release_process_group(process_group, ownership, kill_error);
    #[cfg(not(windows))]
    if let Err(error) = child.kill() {
        *kill_error = Some(error.to_string());
    }
    match child.wait() {
        Ok(status) => {
            *exit_code = status.code();
            ownership.root_process_wait_completed = true;
        }
        Err(error) => {
            append_error(wait_error, error.to_string());
            if let Err(error) = child.kill() {
                *kill_error = Some(error.to_string());
            }
            if let Err(error) = child.wait() {
                append_error(wait_error, error.to_string());
            } else {
                ownership.root_process_wait_completed = true;
            }
        }
    }
}

fn release_process_group(
    process_group: ProcessGroup,
    ownership: &mut BoundedProcessOwnershipEvidence,
    release_error: &mut Option<String>,
) {
    match process_group.release() {
        Ok(()) => ownership.process_group_release_completed = true,
        Err(error) => append_error(
            release_error,
            format!("process group release failed: {error}"),
        ),
    }
}

#[cfg(not(windows))]
#[derive(Debug)]
struct ProcessGroup;

#[cfg(not(windows))]
impl ProcessGroup {
    fn create() -> std::io::Result<Self> {
        Ok(Self)
    }

    fn bind_and_start(
        &self,
        _child: &Child,
        priority: &mut BoundedChildProcessPriorityEvidence,
    ) -> std::io::Result<()> {
        match priority.requested {
            BoundedChildProcessPriority::Normal => {
                priority.effective = Some(BoundedChildProcessPriority::Normal);
                priority.applied = true;
            }
            BoundedChildProcessPriority::BelowNormal => {
                priority.error = Some(
                    "below-normal process priority is unsupported on this platform".to_string(),
                );
            }
        }
        Ok(())
    }

    fn release(self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct ProcessGroup {
    handle: HANDLE,
}

#[cfg(windows)]
impl ProcessGroup {
    fn create() -> std::io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&information as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            let error = std::io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            return Err(error);
        }
        Ok(Self { handle })
    }

    fn bind_and_start(
        &self,
        child: &Child,
        priority: &mut BoundedChildProcessPriorityEvidence,
    ) -> std::io::Result<()> {
        let assigned =
            unsafe { AssignProcessToJobObject(self.handle, child.as_raw_handle() as HANDLE) };
        if assigned == 0 {
            return Err(std::io::Error::last_os_error());
        }
        apply_and_query_process_priority(child, priority);
        resume_process_threads(child.id())
    }

    fn release(mut self) -> std::io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        let closed = unsafe { CloseHandle(self.handle) };
        self.handle = std::ptr::null_mut();
        if closed == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
fn apply_and_query_process_priority(
    child: &Child,
    evidence: &mut BoundedChildProcessPriorityEvidence,
) {
    use windows_sys::Win32::System::Threading::{
        GetPriorityClass, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    };

    let requested_class = match evidence.requested {
        BoundedChildProcessPriority::Normal => NORMAL_PRIORITY_CLASS,
        BoundedChildProcessPriority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
    };
    let handle = child.as_raw_handle() as HANDLE;
    if unsafe { SetPriorityClass(handle, requested_class) } == 0 {
        evidence.error = Some(std::io::Error::last_os_error().to_string());
    }
    let effective_class = unsafe { GetPriorityClass(handle) };
    evidence.effective = match effective_class {
        NORMAL_PRIORITY_CLASS => Some(BoundedChildProcessPriority::Normal),
        BELOW_NORMAL_PRIORITY_CLASS => Some(BoundedChildProcessPriority::BelowNormal),
        0 => {
            append_error(
                &mut evidence.error,
                format!(
                    "process priority query failed: {}",
                    std::io::Error::last_os_error()
                ),
            );
            None
        }
        other => {
            append_error(
                &mut evidence.error,
                format!("unexpected effective process priority class: {other}"),
            );
            None
        }
    };
    evidence.applied = evidence.effective == Some(evidence.requested);
}

#[cfg(windows)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

#[cfg(windows)]
fn resume_process_threads(process_id: u32) -> std::io::Result<()> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
    let mut found = false;
    let mut has_entry = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    while has_entry {
        if entry.th32OwnerProcessID == process_id {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if thread.is_null() {
                let error = std::io::Error::last_os_error();
                unsafe { CloseHandle(snapshot) };
                return Err(error);
            }
            let resume_result = unsafe { ResumeThread(thread) };
            unsafe { CloseHandle(thread) };
            if resume_result == u32::MAX {
                let error = std::io::Error::last_os_error();
                unsafe { CloseHandle(snapshot) };
                return Err(error);
            }
            found = true;
        }
        has_entry = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    if found {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "suspended child process had no resumable thread",
        ))
    }
}

fn append_error(target: &mut Option<String>, error: String) {
    match target {
        Some(existing) => {
            existing.push_str("; ");
            existing.push_str(&error);
        }
        None => *target = Some(error),
    }
}

fn join_reader(
    reader: Option<JoinHandle<StreamCapture>>,
    stream_name: &str,
) -> (StreamCapture, Option<String>) {
    let Some(reader) = reader else {
        return (
            StreamCapture::empty(),
            Some(format!("{stream_name} pipe was unavailable")),
        );
    };
    match reader.join() {
        Ok(capture) => (capture, None),
        Err(_) => (
            StreamCapture::empty(),
            Some(format!("{stream_name} reader thread panicked")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_wait_failure_still_kills_reaps_and_joins() {
        let result = run_bounded_child_process_impl(
            BoundedChildProcessRequest {
                executable: std::env::current_exe().unwrap(),
                args: vec!["--ignored".into()],
                current_dir: std::env::current_dir().unwrap(),
                environment: Vec::new(),
                timeout: Duration::from_secs(5),
                stdout_capture_limit_bytes: 128,
                stderr_capture_limit_bytes: 128,
                priority: BoundedChildProcessPriority::Normal,
            },
            WaitFault::InjectOnce,
            None,
        );

        assert_eq!(
            result.exit_reason,
            BoundedChildProcessExitReason::WaitFailed
        );
        assert!(result
            .wait_error
            .as_deref()
            .is_some_and(|error| error.contains("injected")));
        assert!(result.process_id.is_some());
        assert!(result.reader_join_error.is_none());
    }
}
