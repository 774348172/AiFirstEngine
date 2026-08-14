use runtime_cli::{
    run_bounded_child_process, run_bounded_child_process_cancellable,
    BoundedChildProcessCancellation, BoundedChildProcessExitReason, BoundedChildProcessPriority,
    BoundedChildProcessRequest,
};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FLOOD_BYTES: u64 = 1024 * 1024 + 257;

fn run(
    mode: &str,
    timeout: Duration,
    capture_limit: usize,
) -> runtime_cli::BoundedChildProcessResult {
    run_bounded_child_process(BoundedChildProcessRequest {
        executable: env!("CARGO_BIN_EXE_bounded_output_fixture").into(),
        args: vec![OsString::from(mode)],
        current_dir: std::env::current_dir().unwrap(),
        environment: Vec::new(),
        timeout,
        stdout_capture_limit_bytes: capture_limit,
        stderr_capture_limit_bytes: capture_limit,
        priority: BoundedChildProcessPriority::Normal,
    })
}

#[test]
fn drains_megabyte_floods_while_retaining_bounded_summaries() {
    let result = run("flood-success", Duration::from_secs(10), 4096);

    assert_eq!(result.exit_reason, BoundedChildProcessExitReason::Completed);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout_total_bytes, FLOOD_BYTES);
    assert_eq!(result.stderr_total_bytes, FLOOD_BYTES);
    assert_eq!(result.stdout_summary.len(), 4096);
    assert_eq!(result.stderr_summary.len(), 4096);
    assert!(result.stdout_truncated);
    assert!(result.stderr_truncated);
    assert!(result.reader_join_error.is_none());
    assert!(result.owned_process_cleanup_confirmed());
}

#[test]
fn preserves_nonzero_exit_and_both_streams() {
    let result = run("nonzero", Duration::from_secs(10), 4096);

    assert_eq!(result.exit_reason, BoundedChildProcessExitReason::Failed);
    assert_eq!(result.exit_code, Some(23));
    assert!(result.stdout_summary.contains("stdout nonzero"));
    assert!(result.stderr_summary.contains("stderr nonzero"));
    assert!(!result.stdout_truncated);
    assert!(!result.stderr_truncated);
}

#[test]
fn timeout_is_hard_bounded_and_reaps_the_child() {
    let started = Instant::now();
    let result = run("timeout", Duration::from_millis(500), 4096);

    assert_eq!(result.exit_reason, BoundedChildProcessExitReason::Timeout);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(result.stdout_summary.contains("waiting for timeout"));
    assert!(result.stderr_summary.contains("timeout stderr"));
    assert!(result.reader_join_error.is_none());
    assert!(result.ownership.termination_requested);
    assert!(result.owned_process_cleanup_confirmed());
}

#[cfg(windows)]
#[test]
fn timeout_kills_the_entire_descendant_process_group() {
    let root = temp_root("process-group-timeout");
    fs::create_dir_all(&root).unwrap();
    let sentinel = root.join("grandchild-survived.txt");
    let result = run_bounded_child_process(BoundedChildProcessRequest {
        executable: env!("CARGO_BIN_EXE_bounded_output_fixture").into(),
        args: vec![
            OsString::from("spawn-grandchild"),
            sentinel.clone().into_os_string(),
        ],
        current_dir: std::env::current_dir().unwrap(),
        environment: Vec::new(),
        timeout: Duration::from_millis(500),
        stdout_capture_limit_bytes: 4096,
        stderr_capture_limit_bytes: 4096,
        priority: BoundedChildProcessPriority::Normal,
    });

    assert_eq!(result.exit_reason, BoundedChildProcessExitReason::Timeout);
    assert!(result.stdout_summary.contains("spawned grandchild"));
    assert_eq!(
        result.ownership.ownership_kind,
        runtime_cli::BoundedProcessOwnershipKind::WindowsJobObject
    );
    assert!(result.owned_process_cleanup_confirmed());
    thread::sleep(Duration::from_millis(1400));
    assert!(
        !sentinel.exists(),
        "grandchild survived after the bounded process timed out"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bounded_child_process_cancellation_kills_reaps_and_joins_the_child() {
    let cancellation = BoundedChildProcessCancellation::default();
    let signal = cancellation.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        signal.request_cancel();
    });
    let started = Instant::now();
    let result = run_bounded_child_process_cancellable(
        BoundedChildProcessRequest {
            executable: env!("CARGO_BIN_EXE_bounded_output_fixture").into(),
            args: vec![OsString::from("timeout")],
            current_dir: std::env::current_dir().unwrap(),
            environment: Vec::new(),
            timeout: Duration::from_secs(30),
            stdout_capture_limit_bytes: 4096,
            stderr_capture_limit_bytes: 4096,
            priority: BoundedChildProcessPriority::Normal,
        },
        cancellation,
    );
    cancel_thread.join().unwrap();

    assert_eq!(result.exit_reason, BoundedChildProcessExitReason::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(result.process_id.is_some());
    assert!(result.reader_join_error.is_none());
    assert!(result.ownership.termination_requested);
    assert!(result.owned_process_cleanup_confirmed());
}

#[cfg(windows)]
#[test]
fn bounded_child_process_priority_below_normal_is_observed() {
    let result = run_bounded_child_process(BoundedChildProcessRequest {
        executable: env!("CARGO_BIN_EXE_bounded_output_fixture").into(),
        args: vec![OsString::from("nonzero")],
        current_dir: std::env::current_dir().unwrap(),
        environment: Vec::new(),
        timeout: Duration::from_secs(10),
        stdout_capture_limit_bytes: 4096,
        stderr_capture_limit_bytes: 4096,
        priority: BoundedChildProcessPriority::BelowNormal,
    });

    assert_eq!(
        result.priority.requested,
        BoundedChildProcessPriority::BelowNormal
    );
    assert_eq!(
        result.priority.effective,
        Some(BoundedChildProcessPriority::BelowNormal)
    );
    assert!(
        result.priority.applied,
        "priority evidence: {:#?}",
        result.priority
    );
    assert!(result.priority.error.is_none());
}

#[test]
fn spawn_failure_is_structured_without_stream_evidence() {
    let result = run_bounded_child_process(BoundedChildProcessRequest {
        executable: std::env::temp_dir().join("bounded-child-process-does-not-exist"),
        args: Vec::new(),
        current_dir: std::env::current_dir().unwrap(),
        environment: Vec::new(),
        timeout: Duration::from_secs(1),
        stdout_capture_limit_bytes: 32,
        stderr_capture_limit_bytes: 32,
        priority: BoundedChildProcessPriority::Normal,
    });

    assert_eq!(
        result.exit_reason,
        BoundedChildProcessExitReason::SpawnFailed
    );
    assert!(result.process_id.is_none());
    assert!(result.spawn_error.is_some());
    assert_eq!(result.stdout_total_bytes, 0);
    assert_eq!(result.stderr_total_bytes, 0);
    assert!(result.owned_process_cleanup_confirmed());
}

#[test]
fn injects_explicit_environment_into_the_child() {
    let result = run_bounded_child_process(BoundedChildProcessRequest {
        executable: env!("CARGO_BIN_EXE_bounded_output_fixture").into(),
        args: vec![OsString::from("print-environment")],
        current_dir: std::env::current_dir().unwrap(),
        environment: vec![(
            OsString::from("AIFE_BOUNDED_CHILD_TEST"),
            OsString::from("isolated-value"),
        )],
        timeout: Duration::from_secs(10),
        stdout_capture_limit_bytes: 4096,
        stderr_capture_limit_bytes: 4096,
        priority: BoundedChildProcessPriority::Normal,
    });

    assert_eq!(result.exit_reason, BoundedChildProcessExitReason::Completed);
    assert_eq!(result.stdout_summary.trim(), "isolated-value");
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("runtime-cli-{name}-{stamp}"))
}
