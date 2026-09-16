use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use editor_core::{
    CommandStatus, CredentialOwnerStatus, EditorSession, LlmAsyncExecutor, LlmLifecycleState,
    LlmPatchSourceConfig, LlmPatchSourceKind, LlmTaskJoinStatus, RedactedSecret,
};
use editor_ui_model::{AiPanelStage, UiCommand, UiCommandPayload, UiCommandSource};
use serde::{Deserialize, Serialize};

pub const LLM_WORKER_LIFECYCLE_REPORT_SCHEMA_VERSION: &str = "llm-worker-lifecycle-report.v1";

const TEST_SECRET: &str = "llm-lifecycle-secret-must-not-leak";
const TEST_PROMPT: &str = "llm lifecycle private prompt must not leak";
const PHASE_READY_TIMEOUT: Duration = Duration::from_secs(5);
const SERVER_DISCONNECT_HARD_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmWorkerLifecycleStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmWorkerLifecycleScenarioEvidence {
    pub phase: String,
    pub status: LlmWorkerLifecycleStatus,
    pub lifecycle_state: LlmLifecycleState,
    pub task_join_status: LlmTaskJoinStatus,
    pub credential_owner_status: CredentialOwnerStatus,
    pub transport_abort_observed: bool,
    pub connection_close_observed: bool,
    pub request_count: usize,
    pub cancel_latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmWorkerLifecycleReport {
    pub schema_version: String,
    pub status: LlmWorkerLifecycleStatus,
    pub scenarios: Vec<LlmWorkerLifecycleScenarioEvidence>,
    pub resubmit_after_join_passed: bool,
    pub session_shutdown_joined: bool,
    pub drop_reaper_drained: bool,
    pub join_timeout_failed_closed: bool,
    pub privacy_scan_passed: bool,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum ServerMode {
    HeaderStall,
    BodyStall,
    RetryWait,
    RepairSecondRequestStall,
}

struct ServerObservation {
    request_count: usize,
    connection_close_observed: bool,
    cancellation_release_observed: bool,
}

pub fn run_llm_worker_lifecycle_report() -> LlmWorkerLifecycleReport {
    let mut scenarios = Vec::new();
    let mut diagnostic_codes = Vec::new();
    let mut resubmit_after_join_passed = false;

    for mode in [
        ServerMode::HeaderStall,
        ServerMode::BodyStall,
        ServerMode::RetryWait,
        ServerMode::RepairSecondRequestStall,
    ] {
        let (mut session, project_root) = lifecycle_project_session(phase_name(mode));
        let (base_url, phase_ready, cancellation_release, server) = spawn_server(mode);
        session.set_llm_patch_source_config_for_test(openai_config(base_url));
        let started = session.execute_command(command(TEST_PROMPT));
        if started.status != CommandStatus::Committed {
            diagnostic_codes.push("llm_worker_lifecycle.start_failed".to_string());
            let _ = server.join();
            drop(session);
            let _ = std::fs::remove_dir_all(project_root);
            continue;
        }

        if matches!(mode, ServerMode::RepairSecondRequestStall) {
            let deadline = Instant::now() + PHASE_READY_TIMEOUT;
            while Instant::now() < deadline
                && session.build_ui_model().ai_panel.stage != AiPanelStage::Repairing
            {
                let _ = session.pump_llm_patch_request();
                thread::sleep(Duration::from_millis(2));
            }
        }
        let phase_ready_observed = phase_ready.recv_timeout(PHASE_READY_TIMEOUT).is_ok();
        if !phase_ready_observed {
            diagnostic_codes.push(format!(
                "llm_worker_lifecycle.{}_phase_not_ready",
                phase_name(mode)
            ));
        }
        let cancelling_started = Instant::now();
        let cancel = session.execute_command(UiCommand {
            command_id: "cancel-llm-worker-lifecycle".to_string(),
            source: UiCommandSource::Test,
            request_id: "cancel-llm-worker-lifecycle".to_string(),
            payload: UiCommandPayload::CancelLlmPatchRequest,
        });
        let busy_while_cancelling = session.has_active_llm_patch_request()
            && session.build_ui_model().ai_panel.stage == AiPanelStage::Cancelling;
        let stage_after_cancel = session.build_ui_model().ai_panel.stage;
        let cancellation_release_sent = if matches!(mode, ServerMode::RepairSecondRequestStall) {
            cancellation_release.send(()).is_ok()
        } else {
            true
        };
        pump_until_settled(&mut session, Duration::from_secs(2));
        let elapsed = cancelling_started.elapsed();
        let observation = server.join().expect("LLM lifecycle server must join");
        let llm_report = session
            .last_llm_patch_report()
            .expect("LLM lifecycle report must exist");
        let passed = phase_ready_observed
            && cancel.status == CommandStatus::Committed
            && busy_while_cancelling
            && llm_report.lifecycle_state == LlmLifecycleState::CancelledJoined
            && llm_report.task_join_status == LlmTaskJoinStatus::Joined
            && llm_report.credential_owner_status == CredentialOwnerStatus::Released
            && llm_report.transport_abort_observed
            && (!matches!(mode, ServerMode::RepairSecondRequestStall)
                || server_release_observed(
                    observation.connection_close_observed,
                    observation.cancellation_release_observed,
                ))
            && elapsed < Duration::from_secs(2)
            && observation.request_count
                == if matches!(mode, ServerMode::RepairSecondRequestStall) {
                    2
                } else {
                    1
                };
        if !passed {
            diagnostic_codes.push(format!(
                "llm_worker_lifecycle.{}_failed.cancel={:?}.stage={:?}.busy_while_cancelling={}.cancellation_release_sent={}.cancellation_release_observed={}.connection_close_observed={}.cancel_diagnostics={:?}",
                phase_name(mode),
                cancel.status,
                stage_after_cancel,
                busy_while_cancelling,
                cancellation_release_sent,
                observation.cancellation_release_observed,
                observation.connection_close_observed,
                cancel
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.as_str())
                    .collect::<Vec<_>>()
            ));
        }
        scenarios.push(LlmWorkerLifecycleScenarioEvidence {
            phase: phase_name(mode).to_string(),
            status: if passed {
                LlmWorkerLifecycleStatus::Passed
            } else {
                LlmWorkerLifecycleStatus::Failed
            },
            lifecycle_state: llm_report.lifecycle_state,
            task_join_status: llm_report.task_join_status,
            credential_owner_status: llm_report.credential_owner_status,
            transport_abort_observed: llm_report.transport_abort_observed,
            connection_close_observed: observation.connection_close_observed,
            request_count: observation.request_count,
            cancel_latency_ms: elapsed.as_millis() as u64,
        });

        if matches!(mode, ServerMode::HeaderStall) {
            session
                .set_llm_patch_source_config_for_test(LlmPatchSourceConfig::deterministic_mock());
            let resubmit = session.execute_command(command("create resubmit after joined"));
            pump_until_settled(&mut session, Duration::from_secs(2));
            let resubmit_report = session.last_llm_patch_report();
            resubmit_after_join_passed = resubmit.status == CommandStatus::Committed
                && !session.has_active_llm_patch_request()
                && resubmit_report.is_some_and(|report| {
                    report.lifecycle_state.is_joined_terminal()
                        && !report.cancelled
                        && !report.attempts.is_empty()
                });
            if !resubmit_after_join_passed {
                diagnostic_codes.push(format!(
                    "llm_worker_lifecycle.resubmit_failed.status={:?}.stage={:?}.busy={}",
                    resubmit.status,
                    session.build_ui_model().ai_panel.stage,
                    session.has_active_llm_patch_request()
                ));
            }
        }
        drop(session);
        let _ = std::fs::remove_dir_all(project_root);
    }

    let session_shutdown_joined = session_shutdown_scenario();
    let drop_reaper_drained = drop_reaper_scenario();
    let join_timeout_failed_closed = join_timeout_scenario();

    let mut report = LlmWorkerLifecycleReport {
        schema_version: LLM_WORKER_LIFECYCLE_REPORT_SCHEMA_VERSION.to_string(),
        status: LlmWorkerLifecycleStatus::Failed,
        scenarios,
        resubmit_after_join_passed,
        session_shutdown_joined,
        drop_reaper_drained,
        join_timeout_failed_closed,
        privacy_scan_passed: false,
        diagnostic_codes,
    };
    let encoded = serde_json::to_string(&report).expect("lifecycle report must serialize");
    report.privacy_scan_passed = !encoded.contains(TEST_SECRET)
        && !encoded.contains(TEST_PROMPT)
        && !encoded.contains("Authorization")
        && !encoded.contains("G:\\");
    let passed = report
        .scenarios
        .iter()
        .all(|scenario| scenario.status == LlmWorkerLifecycleStatus::Passed)
        && report.resubmit_after_join_passed
        && report.session_shutdown_joined
        && report.drop_reaper_drained
        && report.join_timeout_failed_closed
        && report.privacy_scan_passed
        && report.diagnostic_codes.is_empty();
    report.status = if passed {
        LlmWorkerLifecycleStatus::Passed
    } else {
        LlmWorkerLifecycleStatus::Failed
    };
    report
}

fn session_shutdown_scenario() -> bool {
    let (mut session, project_root) = lifecycle_project_session("session-shutdown");
    let (base_url, phase_ready, _cancellation_release, server) =
        spawn_server(ServerMode::HeaderStall);
    session.set_llm_patch_source_config_for_test(openai_config(base_url));
    session.execute_command(command("session shutdown prompt"));
    let phase_ready_observed = phase_ready.recv_timeout(PHASE_READY_TIMEOUT).is_ok();
    let receipt = session.shutdown_llm(Duration::from_secs(2));
    let _ = server.join();
    let passed = phase_ready_observed
        && receipt.task_join_status == LlmTaskJoinStatus::Joined
        && receipt.active_task_count == 0
        && receipt.reaper_count == 0
        && receipt.diagnostic.is_none();
    drop(session);
    let _ = std::fs::remove_dir_all(project_root);
    passed
}

fn drop_reaper_scenario() -> bool {
    let executor = LlmAsyncExecutor::process_owned();
    let (mut session, project_root) = lifecycle_project_session("drop-reaper");
    let (base_url, phase_ready, _cancellation_release, server) =
        spawn_server(ServerMode::HeaderStall);
    session.set_llm_patch_source_config_for_test(openai_config(base_url));
    session.execute_command(command("drop fallback prompt"));
    let phase_ready_observed = phase_ready.recv_timeout(PHASE_READY_TIMEOUT).is_ok();
    drop(session);
    let drained = executor.drain_reapers(Duration::from_secs(2));
    let _ = server.join();
    let passed = phase_ready_observed
        && drained
        && executor.active_task_count() == 0
        && executor.reaper_count() == 0;
    let _ = std::fs::remove_dir_all(project_root);
    passed
}

fn join_timeout_scenario() -> bool {
    editor_core::validate_llm_join_timeout_fail_closed()
}

fn command(prompt: &str) -> UiCommand {
    UiCommand {
        command_id: "llm-worker-lifecycle".to_string(),
        source: UiCommandSource::Test,
        request_id: "llm-worker-lifecycle".to_string(),
        payload: UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: prompt.to_string(),
        },
    }
}

fn lifecycle_project_session(label: &str) -> (EditorSession, PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let project_root = std::env::temp_dir().join(format!(
        "aife-llm-worker-lifecycle-{label}-{}-{stamp}",
        std::process::id()
    ));
    let mut session = EditorSession::new();
    let result = session.execute_command(UiCommand {
        command_id: format!("llm-worker-lifecycle-create-{label}"),
        source: UiCommandSource::Test,
        request_id: format!("llm-worker-lifecycle-create-{label}"),
        payload: UiCommandPayload::CreateProject {
            path: project_root.display().to_string(),
            name: format!("LLM Lifecycle {label}"),
        },
    });
    assert_eq!(
        result.status,
        CommandStatus::Committed,
        "lifecycle project creation failed: {result:#?}"
    );
    (session, project_root)
}

fn openai_config(base_url: String) -> LlmPatchSourceConfig {
    let mut config = LlmPatchSourceConfig::deterministic_mock();
    config.source_kind = LlmPatchSourceKind::OpenAiCompatible;
    config.provider_id = "loopback-lifecycle-provider".to_string();
    config.model = "loopback-lifecycle-model".to_string();
    config.base_url = base_url;
    config.timeout_ms = 5_000;
    config.maximum_retry_after_ms = 2_000;
    config.api_key = RedactedSecret::new(TEST_SECRET);
    config
}

fn pump_until_settled(session: &mut EditorSession, deadline: Duration) {
    let started = Instant::now();
    while started.elapsed() < deadline && session.has_active_llm_patch_request() {
        let _ = session.pump_llm_patch_request();
        thread::sleep(Duration::from_millis(2));
    }
}

fn phase_name(mode: ServerMode) -> &'static str {
    match mode {
        ServerMode::HeaderStall => "header_wait",
        ServerMode::BodyStall => "body_chunk_wait",
        ServerMode::RetryWait => "retry_wait",
        ServerMode::RepairSecondRequestStall => "repair_attempt",
    }
}

fn spawn_server(
    mode: ServerMode,
) -> (
    String,
    Receiver<()>,
    Sender<()>,
    thread::JoinHandle<ServerObservation>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener must bind");
    listener
        .set_nonblocking(true)
        .expect("loopback listener must support bounded accept");
    let address = listener.local_addr().expect("loopback address");
    let (phase_sender, phase_receiver) = mpsc::channel();
    let (cancellation_sender, cancellation_receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let expected = if matches!(mode, ServerMode::RepairSecondRequestStall) {
            2
        } else {
            1
        };
        let mut request_count = 0;
        let mut connection_close_observed = false;
        let mut cancellation_release_observed = false;
        for index in 0..expected {
            let deadline = Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error)
                        if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!(
                        "loopback request failed to connect for {mode:?} request {index}: {error}"
                    ),
                }
            };
            stream
                .set_nonblocking(false)
                .expect("accepted loopback stream must restore blocking reads");
            request_count += 1;
            read_request(&mut stream);
            if matches!(mode, ServerMode::RepairSecondRequestStall) && index == 0 {
                let envelope = serde_json::json!({
                    "choices": [{
                        "message": { "content": "{not-json", "refusal": null },
                        "finish_reason": "stop"
                    }]
                })
                .to_string();
                write_response(&mut stream, 200, &[], envelope.as_bytes());
                continue;
            }
            match mode {
                ServerMode::HeaderStall => {
                    phase_sender
                        .send(())
                        .expect("header stall phase receiver must remain active");
                    thread::sleep(Duration::from_millis(150));
                    connection_close_observed |= write_response(&mut stream, 200, &[], b"{}");
                }
                ServerMode::RepairSecondRequestStall => {
                    phase_sender
                        .send(())
                        .expect("repair phase receiver must remain active");
                    let (connection_closed, cancellation_released) =
                        wait_for_disconnect_or_cancellation(
                            &mut stream,
                            &cancellation_receiver,
                            SERVER_DISCONNECT_HARD_DEADLINE,
                        );
                    connection_close_observed |= connection_closed;
                    cancellation_release_observed |= cancellation_released;
                }
                ServerMode::BodyStall => {
                    let head = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n{";
                    let _ = stream.write_all(head);
                    phase_sender
                        .send(())
                        .expect("body stall phase receiver must remain active");
                    thread::sleep(Duration::from_millis(150));
                    connection_close_observed |= stream.write_all(&vec![b'x'; 4095]).is_err();
                    connection_close_observed |= observe_disconnect(&mut stream);
                }
                ServerMode::RetryWait => {
                    write_response(&mut stream, 503, &[("Retry-After", "1")], b"{}");
                    phase_sender
                        .send(())
                        .expect("retry wait phase receiver must remain active");
                }
            }
        }
        ServerObservation {
            request_count,
            connection_close_observed,
            cancellation_release_observed,
        }
    });
    (
        format!("http://{address}/v1"),
        phase_receiver,
        cancellation_sender,
        handle,
    )
}

fn read_request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout");
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap_or(0);
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
) -> bool {
    let reason = if status == 200 {
        "OK"
    } else {
        "Service Unavailable"
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let failed = stream.write_all(head.as_bytes()).is_err() || stream.write_all(body).is_err();
    failed || observe_disconnect(stream)
}

fn observe_disconnect(stream: &mut TcpStream) -> bool {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    let mut byte = [0_u8; 1];
    match stream.read(&mut byte) {
        Ok(0) => true,
        Err(error) => !matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
        _ => false,
    }
}

fn wait_for_disconnect_or_cancellation(
    stream: &mut TcpStream,
    cancellation: &Receiver<()>,
    timeout: Duration,
) -> (bool, bool) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
    let started = Instant::now();
    let mut byte = [0_u8; 1];
    while started.elapsed() < timeout {
        if cancellation.try_recv().is_ok() {
            return (false, true);
        }
        match stream.read(&mut byte) {
            Ok(0) => return (true, false),
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return (true, false),
        }
    }
    (false, false)
}

fn server_release_observed(
    connection_close_observed: bool,
    cancellation_release_observed: bool,
) -> bool {
    connection_close_observed || cancellation_release_observed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_second_request_stays_blocked_until_client_disconnects() {
        let (base_url, phase_ready, _cancellation_release, server) =
            spawn_server(ServerMode::RepairSecondRequestStall);
        let address = base_url
            .strip_prefix("http://")
            .and_then(|value| value.strip_suffix("/v1"))
            .expect("loopback base URL must contain a socket address");

        let mut first = TcpStream::connect(address).expect("first request must connect");
        first
            .write_all(b"POST /v1 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .expect("first request must write");
        first
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("first response timeout must configure");
        let mut response = [0_u8; 1024];
        assert!(first.read(&mut response).expect("first response must read") > 0);
        drop(first);

        let mut second = TcpStream::connect(address).expect("repair request must connect");
        second
            .write_all(b"POST /v1 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .expect("repair request must write");
        phase_ready
            .recv_timeout(PHASE_READY_TIMEOUT)
            .expect("repair phase must become ready");
        second
            .set_read_timeout(Some(Duration::from_millis(1_200)))
            .expect("repair response timeout must configure");
        let read = second.read(&mut response);
        let stayed_blocked = matches!(
            read,
            Err(ref error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
        );
        drop(second);
        let _ = server.join();

        assert!(
            stayed_blocked,
            "repair server returned before the client cancelled or disconnected: {read:?}"
        );
    }

    #[test]
    fn repair_second_request_outlives_the_old_phase_deadline() {
        let (base_url, phase_ready, _cancellation_release, server) =
            spawn_server(ServerMode::RepairSecondRequestStall);
        let address = base_url
            .strip_prefix("http://")
            .and_then(|value| value.strip_suffix("/v1"))
            .expect("loopback base URL must contain a socket address");

        let mut first = TcpStream::connect(address).expect("first request must connect");
        first
            .write_all(b"POST /v1 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .expect("first request must write");
        first
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("first response timeout must configure");
        let mut response = [0_u8; 1024];
        assert!(first.read(&mut response).expect("first response must read") > 0);
        drop(first);

        let mut second = TcpStream::connect(address).expect("repair request must connect");
        second
            .write_all(b"POST /v1 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .expect("repair request must write");
        phase_ready
            .recv_timeout(PHASE_READY_TIMEOUT)
            .expect("repair phase must become ready");

        thread::sleep(Duration::from_millis(2_200));
        second
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("repair response timeout must configure");
        let read = second.read(&mut response);
        let stayed_blocked = matches!(
            read,
            Err(ref error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
        );
        drop(second);
        let _ = server.join();

        assert!(
            stayed_blocked,
            "repair server reused the client phase deadline as its lifetime: {read:?}"
        );
    }

    #[test]
    fn repair_server_release_accepts_barrier_first() {
        let (mut server_stream, _client_stream) = connected_stream_pair();
        let (release_sender, release_receiver) = mpsc::channel();
        release_sender
            .send(())
            .expect("barrier receiver must remain active");

        let (connection_closed, cancellation_released) = wait_for_disconnect_or_cancellation(
            &mut server_stream,
            &release_receiver,
            Duration::from_millis(100),
        );

        assert!(server_release_observed(
            connection_closed,
            cancellation_released
        ));
        assert!(!connection_closed);
        assert!(cancellation_released);
    }

    #[test]
    fn repair_server_release_accepts_disconnect_first() {
        let (mut server_stream, client_stream) = connected_stream_pair();
        let (_release_sender, release_receiver) = mpsc::channel();
        drop(client_stream);

        let (connection_closed, cancellation_released) = wait_for_disconnect_or_cancellation(
            &mut server_stream,
            &release_receiver,
            Duration::from_millis(100),
        );

        assert!(server_release_observed(
            connection_closed,
            cancellation_released
        ));
        assert!(connection_closed);
        assert!(!cancellation_released);
    }

    #[test]
    fn repair_server_release_fails_closed_when_neither_signal_arrives() {
        let (mut server_stream, _client_stream) = connected_stream_pair();
        let (_release_sender, release_receiver) = mpsc::channel();

        let (connection_closed, cancellation_released) = wait_for_disconnect_or_cancellation(
            &mut server_stream,
            &release_receiver,
            Duration::from_millis(100),
        );

        assert!(!server_release_observed(
            connection_closed,
            cancellation_released
        ));
        assert!(!connection_closed);
        assert!(!cancellation_released);
    }

    fn connected_stream_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener must bind");
        let client = TcpStream::connect(listener.local_addr().expect("loopback address"))
            .expect("loopback client must connect");
        let (server, _) = listener.accept().expect("loopback server must accept");
        (server, client)
    }
}
