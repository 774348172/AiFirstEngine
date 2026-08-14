#![cfg(windows)]

use ai_tool_gateway::{
    resolve_gateway_discovery_path_in_root, ClientKind, GatewayRemoteAdapter, GatewayReplyPayload,
    GatewayRequestPayload,
};
use editor_core::{
    AiToolCatalogRequest, AiToolInvocation, AiToolInvocationPayload, AiToolOperationSnapshot,
    AiToolOperationState, AI_TOOL_INVOCATION_SCHEMA_VERSION, TOOL_ID_PROJECT_PREVIEW,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn retired_production_lifecycle_cli_is_unavailable() {
    for argument in [
        "--candidate-freeze-preflight",
        "--production-candidate-request",
        "--production-candidate-result",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_editor_host"))
            .arg(argument)
            .output()
            .expect("run retired lifecycle argument");
        assert_diagnostic(&output, "editor_host.unknown_argument");
    }
}

fn assert_diagnostic(output: &std::process::Output, expected: &str) {
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "expected diagnostic {expected}, got: {stderr}"
    );
}

#[test]
fn real_editor_gateway_process_preflight_catalog_and_cleanup() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "editor-host-gateway-process-{}-{}-{stamp}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let discovery_root = root.join("discovery");
    let source_project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/complex_shooter_project")
        .canonicalize()
        .expect("canonical sample project");
    let project_root = root.join("project");
    copy_project_tree(&source_project, &project_root);
    std::fs::create_dir_all(&discovery_root).expect("create isolated discovery root");

    let mut child = Command::new(env!("CARGO_BIN_EXE_editor_host"))
        .args([
            "--gateway-process-preflight",
            "--project-root",
            project_root.to_str().expect("UTF-8 project root"),
            "--gateway-discovery-root",
            discovery_root.to_str().expect("UTF-8 discovery root"),
            "--gateway-preflight-timeout-ms",
            "60000",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real editor_host process");

    let discovery_path = wait_for_discovery(&mut child, &discovery_root);
    let mut adapter = GatewayRemoteAdapter::connect_from_discovery(
        &discovery_path,
        ClientKind::Test,
        "real-editor-process-preflight.v1",
    )
    .expect("connect real editor process Gateway");
    let catalog = adapter
        .dispatch(GatewayRequestPayload::Catalog(
            AiToolCatalogRequest::default(),
        ))
        .expect("dispatch Catalog through real editor process");
    assert!(matches!(catalog.payload, GatewayReplyPayload::Catalog(_)));

    let project_digest = adapter
        .binding()
        .project_context
        .as_ref()
        .expect("real editor process project context")
        .project_digest
        .clone();
    let preview = adapter
        .dispatch(GatewayRequestPayload::ExecuteSessionBound {
            invocation: AiToolInvocation {
                schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                invocation_id: "real-editor-process-production-preview".to_string(),
                tool_id: TOOL_ID_PROJECT_PREVIEW.to_string(),
                expected_project_digest: project_digest,
                payload: AiToolInvocationPayload::Preview,
            },
        })
        .expect("dispatch production Preview through real editor process");
    let accepted = match preview.payload {
        GatewayReplyPayload::Accepted(accepted) => accepted,
        other => panic!("production Preview must be accepted: {other:?}"),
    };
    let operation = wait_for_awaiting_frame_evidence(&mut adapter, &accepted.operation_id);
    assert_eq!(operation.state, AiToolOperationState::Running);
    assert_eq!(operation.stage, "awaiting_frame_evidence");
    assert!(operation.result.is_none());
    adapter.close().expect("close Gateway client binding");

    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().expect("poll editor_host").is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if child.try_wait().expect("final poll editor_host").is_none() {
        let _ = child.kill();
        let output = child
            .wait_with_output()
            .expect("collect timed out editor_host");
        panic!(
            "editor_host preflight did not exit; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = child
        .wait_with_output()
        .expect("collect editor_host output");
    assert!(
        output.status.success(),
        "editor_host stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("editor_host gateway-process-preflight: passed"));
    assert!(!discovery_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

fn wait_for_awaiting_frame_evidence(
    adapter: &mut GatewayRemoteAdapter,
    operation_id: &str,
) -> AiToolOperationSnapshot {
    let deadline = Instant::now() + Duration::from_secs(50);
    loop {
        let reply = adapter
            .dispatch(GatewayRequestPayload::Observe {
                operation_id: operation_id.to_string(),
            })
            .expect("observe production Preview operation");
        let operation = match reply.payload {
            GatewayReplyPayload::Operation(operation) => operation,
            other => panic!("operation snapshot expected: {other:?}"),
        };
        if operation.state == AiToolOperationState::Running
            && operation.stage == "awaiting_frame_evidence"
        {
            return operation;
        }
        assert!(
            !matches!(
                operation.state,
                AiToolOperationState::Completed
                    | AiToolOperationState::Failed
                    | AiToolOperationState::Cancelled
                    | AiToolOperationState::Interrupted
            ),
            "headless production Preview reached a terminal state before real-window frame evidence: {operation:?}"
        );
        assert!(
            Instant::now() < deadline,
            "production Preview operation did not reach awaiting_frame_evidence"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_discovery(child: &mut Child, discovery_root: &std::path::Path) -> std::path::PathBuf {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match resolve_gateway_discovery_path_in_root(discovery_root, None) {
            Ok(path) => return path,
            Err(_) if Instant::now() < deadline => {
                if let Some(status) = child.try_wait().expect("poll editor_host startup") {
                    panic!("editor_host exited before discovery publication: {status}");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("Gateway discovery did not appear: {}", error.code),
        }
    }
}

fn copy_project_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create copied project directory");
    for entry in std::fs::read_dir(source)
        .expect("read sample project directory")
        .flatten()
    {
        let name = entry.file_name();
        if entry.path().is_dir()
            && matches!(
                name.to_string_lossy().as_ref(),
                "Build" | "Library" | ".aife" | "target"
            )
        {
            continue;
        }
        let destination_path = destination.join(&name);
        if entry.path().is_dir() {
            copy_project_tree(&entry.path(), &destination_path);
        } else {
            std::fs::copy(entry.path(), destination_path).expect("copy sample project file");
        }
    }
}
