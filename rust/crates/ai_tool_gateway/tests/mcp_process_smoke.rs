#![cfg(windows)]

use ai_tool_gateway::{
    gateway_owner_thread_channel, GatewayAccessDecision, GatewayCore, GatewayDiscoveryPublication,
    GatewayDiscoveryRecord, GatewayNamedPipeServer, GatewayOwnerThreadDispatcher,
};
use editor_core::{
    command_for_test, CommandStatus, EditorSession, InputPatchOperation, PatchOperation,
    PatchSource, ProjectCandidateEntry, ProjectPatchDocument,
};
use editor_ui_model::{InputActionValueKind, UiCommandPayload};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn mcp_process_smoke_typed_surface_is_strict_and_session_bound() {
    let root = std::env::temp_dir().join(format!(
        "ai-tool-gateway-mcp-process-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut session = EditorSession::new();
    let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "MCP Process Smoke".to_string(),
    }));
    assert_eq!(created.status, CommandStatus::Committed);
    let discovery = GatewayDiscoveryRecord::new("mcp-process-smoke");
    let local_app_data = root.join("local-app-data");
    let discovery_root = local_app_data
        .join("AiFirstGameEngine")
        .join("Gateway")
        .join("discovery");
    let publication = GatewayDiscoveryPublication::publish(&discovery_root, &discovery).unwrap();
    let (owner_client, mut dispatcher) = gateway_owner_thread_channel();
    let mut core = GatewayCore::new_for_editor_instance(discovery.editor_instance_id.clone());
    let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ai_engine_gateway_mcp"))
        .env_remove("AI_ENGINE_GATEWAY_DISCOVERY")
        .env_remove("AI_ENGINE_GATEWAY_EDITOR_INSTANCE_ID")
        .env("LOCALAPPDATA", &local_app_data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        let search_arguments = serde_json::json!({
            "query": "project",
            "kinds": [],
            "continuationToken": null,
            "pageSize": 25
        });
        let mut requests = vec![
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"aife_status","arguments":{}}}),
            serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"aife_catalog","arguments":{}}}),
            serde_json::json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"aife_project_inspect","arguments":{"kind":"project"}}}),
            serde_json::json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"aife_project_search","arguments":search_arguments}}),
        ];
        for (id, field) in [
            (7, "schemaVersion"),
            (8, "projectIdentity"),
            (9, "toolVersion"),
            (10, "payloadKind"),
            (11, "grantRef"),
        ] {
            let mut arguments = search_arguments.clone();
            arguments
                .as_object_mut()
                .unwrap()
                .insert(field.to_string(), serde_json::json!("forged"));
            requests.push(serde_json::json!({
                "jsonrpc":"2.0",
                "id":id,
                "method":"tools/call",
                "params":{"name":"aife_project_search","arguments":arguments}
            }));
        }
        requests.extend([
            serde_json::json!({
                "jsonrpc":"2.0",
                "id":12,
                "method":"tools/call",
                "params":{"name":"aife_project_inspect","arguments":{
                    "kind":{"grant_lineage":{"grant_digest":"sha256:test","unknown":"forged"}}
                }}
            }),
            serde_json::json!({"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"aife_cancel","arguments":{"operationId":"operation-1","grantRef":"$active"}}}),
            serde_json::json!({"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"aife_cancel","arguments":{"operationId":""}}}),
            serde_json::json!({"jsonrpc":"2.0","id":15,"method":"tools/call","params":{"name":"aife_observe","arguments":{"operationId":"x".repeat(129)}}}),
            serde_json::json!({"jsonrpc":"2.0","id":16,"method":"shutdown","params":{}}),
        ]);
        for request in requests {
            writeln!(stdin, "{}", serde_json::to_string(&request).unwrap()).unwrap();
        }
    }
    let mut child_stdout = child.stdout.take().unwrap();
    let stdout_thread = std::thread::spawn(move || {
        let mut stdout = String::new();
        child_stdout.read_to_string(&mut stdout).unwrap();
        stdout
    });
    let mut child_stderr = child.stderr.take().unwrap();
    let stderr_thread = std::thread::spawn(move || {
        let mut stderr = String::new();
        child_stderr.read_to_string(&mut stderr).unwrap();
        stderr
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        core.pump_operations(&mut session, 1);
        dispatcher.pump(&mut core, &mut session);
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = stderr_thread.join().unwrap();
            panic!("MCP child process timed out; stderr: {stderr}");
        }
        std::thread::yield_now();
    }
    let status = child.wait().unwrap();
    let stdout = stdout_thread.join().unwrap();
    let stderr = stderr_thread.join().unwrap();
    assert!(status.success(), "MCP stderr: {stderr}");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 16, "MCP stderr: {stderr}");
    let response = |id: i64| {
        responses
            .iter()
            .find(|response| response["id"] == id)
            .unwrap_or_else(|| panic!("missing MCP response {id}"))
    };
    let tools = response(2)["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 21);
    assert!(tools.iter().all(|tool| tool["name"] != "aife_execute"));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "aife_project_search"));
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "aife_project_mutate"));
    for tool in tools.iter().filter(|tool| {
        tool["name"].as_str().is_some_and(|name| {
            name.starts_with("aife_project_")
                || name.starts_with("aife_ui_")
                || name.starts_with("aife_runtime_")
                || name == "aife_evidence_read"
        })
    }) {
        if tool["name"] == "aife_project_rollback" {
            assert_eq!(
                tool["inputSchema"]["properties"]["schemaVersion"]["const"],
                editor_core::EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION
            );
        } else {
            assert!(tool["inputSchema"]["properties"]
                .get("schemaVersion")
                .is_none());
        }
    }
    for id in 3..=6 {
        assert_eq!(response(id)["result"]["isError"], false);
    }
    assert_eq!(
        response(3)["result"]["structuredContent"]["payload"]["replyKind"],
        "session_status"
    );
    assert_eq!(
        response(3)["result"]["structuredContent"]["payload"]["reply"]["access"]["read"]["state"],
        "active"
    );
    assert_eq!(
        response(3)["result"]["structuredContent"]["payload"]["reply"]["access"]["mutation"]
            ["state"],
        "not_requested"
    );
    assert_eq!(
        response(5)["result"]["structuredContent"]["payload"]["replyKind"],
        "inspection"
    );
    for id in 7..=15 {
        assert_eq!(response(id)["error"]["code"], -32602, "response {id}");
    }
    assert!(
        response(4)["result"]["structuredContent"]["sessionBinding"]["catalogDigest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    server.join().unwrap();
    drop(publication);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn mcp_process_typed_project_create_transitions_launcher_session_in_place() {
    let root = std::env::temp_dir().join(format!(
        "ai-tool-gateway-mcp-project-create-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let project_root = root.join("typed-created-project");
    assert!(!project_root.exists());

    let mut session = EditorSession::new();
    let discovery = GatewayDiscoveryRecord::new("mcp-process-project-create");
    let local_app_data = root.join("local-app-data");
    let discovery_root = local_app_data
        .join("AiFirstGameEngine")
        .join("Gateway")
        .join("discovery");
    let publication = GatewayDiscoveryPublication::publish(&discovery_root, &discovery).unwrap();
    let discovery_path = publication.path().to_path_buf();
    let (owner_client, mut dispatcher) = gateway_owner_thread_channel();
    let mut core = GatewayCore::new_for_editor_instance(discovery.editor_instance_id.clone());
    let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ai_engine_gateway_mcp"))
        .env_remove("AI_ENGINE_GATEWAY_DISCOVERY")
        .env_remove("AI_ENGINE_GATEWAY_EDITOR_INSTANCE_ID")
        .env("LOCALAPPDATA", &local_app_data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        let requests = [
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"aife_status","arguments":{}}}),
            serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"aife_catalog","arguments":{}}}),
            serde_json::json!({
                "jsonrpc":"2.0",
                "id":5,
                "method":"tools/call",
                "params":{"name":"aife_project_create","arguments":{
                    "requestedProjectRoot": project_root.display().to_string(),
                    "projectName": "MCP Typed Create"
                }}
            }),
            serde_json::json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"aife_status","arguments":{}}}),
            serde_json::json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"aife_catalog","arguments":{}}}),
            serde_json::json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"aife_project_inspect","arguments":{"kind":"project"}}}),
            serde_json::json!({"jsonrpc":"2.0","id":9,"method":"shutdown","params":{}}),
        ];
        for request in requests {
            writeln!(stdin, "{}", serde_json::to_string(&request).unwrap()).unwrap();
        }
    }
    let mut child_stdout = child.stdout.take().unwrap();
    let stdout_thread = std::thread::spawn(move || {
        let mut stdout = String::new();
        child_stdout.read_to_string(&mut stdout).unwrap();
        stdout
    });
    let mut child_stderr = child.stderr.take().unwrap();
    let stderr_thread = std::thread::spawn(move || {
        let mut stderr = String::new();
        child_stderr.read_to_string(&mut stderr).unwrap();
        stderr
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        core.pump_operations(&mut session, 1);
        dispatcher.pump(&mut core, &mut session);
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = stderr_thread.join().unwrap();
            panic!("typed project.create MCP child timed out; stderr: {stderr}");
        }
        std::thread::yield_now();
    }
    let status = child.wait().unwrap();
    let stdout = stdout_thread.join().unwrap();
    let stderr = stderr_thread.join().unwrap();
    assert!(status.success(), "MCP stderr: {stderr}");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 9, "MCP stderr: {stderr}");
    let response = |id: i64| {
        responses
            .iter()
            .find(|response| response["id"] == id)
            .unwrap_or_else(|| panic!("missing MCP response {id}"))
    };
    let structured = |id: i64| &response(id)["result"]["structuredContent"];

    let create_tool = response(2)["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "aife_project_create")
        .expect("typed project.create tool");
    assert_eq!(
        create_tool["inputSchema"]["required"],
        serde_json::json!(["requestedProjectRoot", "projectName"])
    );
    let create_properties = create_tool["inputSchema"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        create_properties,
        BTreeSet::from(["projectName", "requestedProjectRoot"])
    );
    assert_eq!(create_tool["inputSchema"]["additionalProperties"], false);

    for id in 3..=8 {
        assert_eq!(response(id)["result"]["isError"], false, "response {id}");
    }
    let launcher_status = &structured(3)["payload"]["reply"];
    assert!(launcher_status["project"].is_null());
    assert_eq!(launcher_status["access"]["read"]["state"], "unavailable");
    let launcher_catalog = &structured(4)["payload"]["reply"];
    assert_eq!(
        catalog_tool_state(launcher_catalog, "project.create"),
        "ready"
    );
    assert_eq!(
        catalog_tool_state(launcher_catalog, "project.inspect"),
        "blocked"
    );

    assert_eq!(structured(5)["payload"]["replyKind"], "tool_result");
    let create_result = &structured(5)["payload"]["reply"];
    assert_eq!(create_result["status"], "completed");
    assert_eq!(create_result["toolId"], "project.create");
    let facts = create_result["facts"].as_object().unwrap();
    assert_eq!(
        facts.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "canonicalProjectRoot",
            "openedInEditor",
            "projectDigest",
            "projectIdentity",
            "projectName",
            "readGeneration",
            "receiptId",
            "replayed",
            "requestedProjectRoot",
            "status",
        ])
    );
    assert_eq!(facts["status"], "created");
    assert_eq!(
        facts["requestedProjectRoot"],
        project_root.display().to_string()
    );
    assert_eq!(facts["projectName"], "MCP Typed Create");
    assert_eq!(facts["openedInEditor"], "true");
    assert_eq!(facts["replayed"], "false");
    assert!(facts["receiptId"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(facts["canonicalProjectRoot"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(facts["projectIdentity"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(facts["projectDigest"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert!(facts["readGeneration"]
        .as_str()
        .is_some_and(|value| value.parse::<u64>().is_ok_and(|value| value > 0)));

    let launcher_binding = &structured(3)["sessionBinding"];
    for id in 4..=8 {
        assert_eq!(
            structured(id)["sessionBinding"]["clientSessionId"],
            launcher_binding["clientSessionId"],
            "request {id} changed the MCP client session"
        );
        assert_eq!(
            structured(id)["sessionBinding"]["editorInstanceId"],
            discovery.editor_instance_id,
            "request {id} changed the Editor instance"
        );
    }
    let project_status = &structured(6)["payload"]["reply"];
    assert_eq!(
        project_status["session"]["id"],
        launcher_binding["clientSessionId"]
    );
    assert_eq!(
        project_status["project"]["identity"],
        facts["projectIdentity"]
    );
    assert_eq!(
        project_status["project"]["observedDigest"],
        facts["projectDigest"]
    );

    let project_catalog = &structured(7)["payload"]["reply"];
    assert_eq!(
        launcher_catalog["catalogDigest"],
        project_catalog["catalogDigest"]
    );
    assert_ne!(
        launcher_catalog["availabilityDigest"],
        project_catalog["availabilityDigest"]
    );
    assert_eq!(
        project_catalog["basis"]["readGeneration"].to_string(),
        facts["readGeneration"].as_str().unwrap()
    );
    assert_eq!(
        catalog_tool_state(project_catalog, "project.create"),
        "blocked"
    );
    assert_eq!(
        catalog_tool_state(project_catalog, "project.inspect"),
        "ready"
    );
    assert_eq!(structured(8)["payload"]["replyKind"], "inspection");
    let inspected_project = &structured(8)["payload"]["reply"]["payload"]["result"];
    assert_eq!(inspected_project["projectId"], facts["projectIdentity"]);
    assert_eq!(inspected_project["projectDigest"], facts["projectDigest"]);
    assert!(project_root.is_dir());
    assert!(discovery_path.is_file());

    server.join().unwrap();
    drop(publication);
    drop(core);
    drop(dispatcher);
    drop(session);
    assert!(!discovery_path.exists());
    std::fs::remove_dir_all(&root).unwrap();
    assert!(!root.exists());
}

fn catalog_tool_state<'a>(catalog: &'a serde_json::Value, tool_id: &str) -> &'a str {
    catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["descriptor"]["toolId"] == tool_id)
        .and_then(|entry| entry["availability"]["state"].as_str())
        .unwrap_or_else(|| panic!("missing Catalog availability for {tool_id}"))
}

#[test]
fn external_codex_authoring_process_smoke() {
    let root = unique_temp_root("external-codex-authoring");
    std::fs::create_dir_all(&root).unwrap();
    let project_root = root.join("authoring-project");
    let local_app_data = root.join("local-app-data");
    let discovery_root = local_app_data
        .join("AiFirstGameEngine")
        .join("Gateway")
        .join("discovery");

    let mut session = EditorSession::new();
    let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "External Codex Authoring".to_string(),
    }));
    assert_eq!(created.status, CommandStatus::Committed);
    let initial_project = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let initial_digest = initial_project.project_digest.clone();

    let mut stale_discovery = GatewayDiscoveryRecord::new("external-codex-authoring-stale");
    stale_discovery.editor_process_id = u32::MAX;
    let stale_publication =
        GatewayDiscoveryPublication::publish(&discovery_root, &stale_discovery).unwrap();
    let discovery = GatewayDiscoveryRecord::new("external-codex-authoring-active");
    let publication = GatewayDiscoveryPublication::publish(&discovery_root, &discovery).unwrap();
    assert!(stale_publication.path().is_file());
    assert!(publication.path().is_file());

    let (owner_client, mut dispatcher) = gateway_owner_thread_channel();
    let mut core = GatewayCore::new_for_editor_instance(discovery.editor_instance_id.clone());
    let server = GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ai_engine_gateway_mcp"))
        .env_remove("AI_ENGINE_GATEWAY_DISCOVERY")
        .env_remove("AI_ENGINE_GATEWAY_EDITOR_INSTANCE_ID")
        .env("LOCALAPPDATA", &local_app_data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (stdout_rx, stdout_thread) = spawn_mcp_stdout_reader(child.stdout.take().unwrap());
    let mut child_stderr = child.stderr.take().unwrap();
    let stderr_thread = std::thread::spawn(move || {
        let mut stderr = String::new();
        child_stderr.read_to_string(&mut stderr).unwrap();
        stderr
    });
    let deadline = Instant::now() + Duration::from_secs(20);

    write_mcp_request(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    let initialized = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        1,
        false,
        deadline,
    );
    assert_eq!(
        initialized["result"]["serverInfo"]["name"],
        "ai-first-game-engine"
    );

    write_mcp_request(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );
    let listed = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        2,
        false,
        deadline,
    );
    let tool_names = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(tool_names.contains("aife_project_mutate"));
    assert!(tool_names.contains("aife_project_rollback"));
    assert!(!tool_names.contains("aife_project_mutate_candidate"));
    assert!(!tool_names.contains("aife_project_rollback_candidate"));

    write_mcp_request(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"aife_status","arguments":{}}}),
    );
    let status = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        3,
        false,
        deadline,
    );
    let session_binding = &status["result"]["structuredContent"]["sessionBinding"];
    let client_session_id = session_binding["clientSessionId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        session_binding["editorInstanceId"],
        discovery.editor_instance_id
    );
    let status_payload = &status["result"]["structuredContent"]["payload"]["reply"];
    assert_eq!(status_payload["access"]["read"]["state"], "active");
    assert_eq!(
        status_payload["access"]["mutation"]["state"],
        "not_requested"
    );
    assert_eq!(status_payload["project"]["observedDigest"], initial_digest);

    write_mcp_request(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"aife_catalog","arguments":{}}}),
    );
    let catalog = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        4,
        false,
        deadline,
    );
    let catalog_payload = &catalog["result"]["structuredContent"]["payload"]["reply"];
    assert_eq!(
        catalog_tool_state(catalog_payload, "project.mutate"),
        "authorization_required"
    );

    write_mcp_request(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"aife_project_inspect","arguments":{"kind":"project"}}}),
    );
    let inspected = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        5,
        false,
        deadline,
    );
    assert_eq!(
        inspected["result"]["structuredContent"]["payload"]["reply"]["payload"]["result"]
            ["projectDigest"],
        initial_digest
    );

    let goal = "Add bounded external Codex authoring input actions.";
    write_mcp_request(
        &mut stdin,
        tool_call(
            6,
            "aife_project_mutate",
            mutation_direct_input("first", goal, "first"),
        ),
    );
    let first_mutate = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        6,
        false,
        deadline,
    );
    assert_eq!(first_mutate["result"]["isError"], false);
    let first_accepted = &first_mutate["result"]["structuredContent"]["payload"]["reply"];
    assert_eq!(first_accepted["state"], "awaiting_user");
    let first_operation_id = first_accepted["operationId"].as_str().unwrap().to_string();
    let approval = core
        .approval_inbox(now_epoch_ms())
        .into_iter()
        .find(|request| request.operation_id.as_deref() == Some(first_operation_id.as_str()))
        .expect("process smoke mutation approval request");
    let decision = core
        .decide_access(
            &session,
            &approval.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();
    assert!(decision.grant_ref.is_some());
    assert!(core.client_has_active_grant(&client_session_id));
    assert_eq!(
        core.session_status(&session, &client_session_id, now_epoch_ms())
            .unwrap()
            .access
            .mutation
            .state,
        ai_tool_gateway::GatewayMutationAccessState::Active
    );

    write_mcp_request(
        &mut stdin,
        tool_call(
            7,
            "aife_observe",
            serde_json::json!({"operationId": first_operation_id}),
        ),
    );
    let first_observed = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        7,
        true,
        deadline,
    );
    let first_operation = &first_observed["result"]["structuredContent"]["payload"]["reply"];
    assert_eq!(first_operation["state"], "completed");
    let first_result = &first_operation["result"];
    assert_eq!(first_result["toolId"], "project.mutate");
    assert_eq!(first_result["output"]["outputKind"], "candidate_applied");
    let receipt = &first_result["output"]["output"];
    assert_eq!(receipt["beforeProjectDigest"], initial_digest);
    assert!(receipt["afterProjectDigest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:") && digest != initial_digest));
    assert!(receipt["receiptDigest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    let rollback_ref = first_result["rollbackRef"]
        .as_str()
        .expect("completed mutation rollbackRef")
        .to_string();
    assert!(rollback_ref.starts_with("rbk_"));

    write_mcp_request(
        &mut stdin,
        tool_call(
            8,
            "aife_project_mutate",
            mutation_direct_input("second", goal, "second"),
        ),
    );
    let second_mutate = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        8,
        false,
        deadline,
    );
    let second_accepted = &second_mutate["result"]["structuredContent"]["payload"]["reply"];
    assert_ne!(second_accepted["state"], "awaiting_user");
    assert!(core.approval_inbox(now_epoch_ms()).is_empty());
    let second_operation_id = second_accepted["operationId"].as_str().unwrap().to_string();
    write_mcp_request(
        &mut stdin,
        tool_call(
            9,
            "aife_cancel",
            serde_json::json!({"operationId": second_operation_id}),
        ),
    );
    let second_cancelled = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        9,
        false,
        deadline,
    );
    assert_eq!(
        second_cancelled["result"]["structuredContent"]["payload"]["replyKind"],
        "cancellation"
    );

    write_mcp_request(
        &mut stdin,
        tool_call(
            10,
            "aife_project_rollback",
            serde_json::json!({
                "schemaVersion": editor_core::EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
                "rollbackRef": rollback_ref
            }),
        ),
    );
    let rollback_started = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        10,
        false,
        deadline,
    );
    let rollback_operation_id = rollback_started["result"]["structuredContent"]["payload"]["reply"]
        ["operationId"]
        .as_str()
        .unwrap()
        .to_string();
    write_mcp_request(
        &mut stdin,
        tool_call(
            11,
            "aife_observe",
            serde_json::json!({"operationId": rollback_operation_id}),
        ),
    );
    let rollback_observed = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        11,
        true,
        deadline,
    );
    let rollback_operation = &rollback_observed["result"]["structuredContent"]["payload"]["reply"];
    assert_eq!(rollback_operation["state"], "completed");
    let rollback_result = &rollback_operation["result"];
    assert_eq!(rollback_result["toolId"], "project.rollback");
    assert_eq!(
        rollback_result["output"]["outputKind"],
        "candidate_rolled_back"
    );
    assert_eq!(
        rollback_result["output"]["output"]["restoredProjectDigest"],
        initial_digest
    );
    assert_eq!(
        ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_digest,
        initial_digest
    );

    write_mcp_request(
        &mut stdin,
        tool_call(
            12,
            "aife_project_mutate",
            mutation_direct_input("disconnect", goal, "disconnect"),
        ),
    );
    let disconnect_mutate = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        12,
        false,
        deadline,
    );
    let disconnect_operation_id = disconnect_mutate["result"]["structuredContent"]["payload"]
        ["reply"]["operationId"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!disconnect_operation_id.is_empty());
    write_mcp_request(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":13,"method":"shutdown","params":{}}),
    );
    let shutdown = wait_for_mcp_response(
        &stdout_rx,
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        13,
        false,
        deadline,
    );
    assert!(shutdown["result"].is_object());
    drop(stdin);
    wait_for_mcp_exit(
        &mut child,
        &mut core,
        &mut session,
        &mut dispatcher,
        deadline,
    );
    let status = child.wait().unwrap();
    let stdout_result = stdout_thread.join().unwrap();
    assert!(
        stdout_result.is_ok(),
        "MCP stdout reader failed: {stdout_result:?}"
    );
    let stderr = stderr_thread.join().unwrap();
    assert!(status.success(), "MCP stderr: {stderr}");
    for _ in 0..4 {
        core.pump_operations(&mut session, 8);
    }
    assert_eq!(
        ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_digest,
        initial_digest,
        "MCP close must not allow the pending mutation to apply"
    );

    server.join().unwrap();
    drop(publication);
    drop(stale_publication);
    drop(core);
    drop(dispatcher);
    drop(session);
    std::fs::remove_dir_all(&root).unwrap();
    assert!(!root.exists());
}

#[test]
fn mcp_process_keeps_session_across_project_switch_and_drops_old_authority() {
    let root = std::env::temp_dir().join(format!(
        "ai-tool-gateway-mcp-reconnect-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let first_root = root.join("first-project");
    let second_root = root.join("second-project");
    let mut session = EditorSession::new();
    let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: first_root.display().to_string(),
        name: "MCP Reconnect First".to_string(),
    }));
    assert_eq!(created.status, CommandStatus::Committed);
    let mut second_session = EditorSession::new();
    let created =
        second_session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: second_root.display().to_string(),
            name: "MCP Reconnect Second".to_string(),
        }));
    assert_eq!(created.status, CommandStatus::Committed);
    let second_project =
        editor_core::ProjectCandidateEntry::inspect_project_binding(&second_session).unwrap();
    drop(second_session);

    let discovery = GatewayDiscoveryRecord::new("mcp-process-reconnect");
    let local_app_data = root.join("local-app-data");
    let discovery_root = local_app_data
        .join("AiFirstGameEngine")
        .join("Gateway")
        .join("discovery");
    let publication = GatewayDiscoveryPublication::publish(&discovery_root, &discovery).unwrap();
    let (owner_client, mut dispatcher) = gateway_owner_thread_channel();
    let mut core = GatewayCore::new_for_editor_instance(discovery.editor_instance_id.clone());
    let server =
        GatewayNamedPipeServer::spawn(&discovery.pipe_locator, owner_client.clone()).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ai_engine_gateway_mcp"))
        .env_remove("AI_ENGINE_GATEWAY_DISCOVERY")
        .env_remove("AI_ENGINE_GATEWAY_EDITOR_INSTANCE_ID")
        .env("LOCALAPPDATA", &local_app_data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut child_stdout = child.stdout.take().unwrap();
    let stdout_thread = std::thread::spawn(move || {
        let mut stdout = String::new();
        child_stdout.read_to_string(&mut stdout).unwrap();
        stdout
    });
    let mut child_stderr = child.stderr.take().unwrap();
    let stderr_thread = std::thread::spawn(move || {
        let mut stderr = String::new();
        child_stderr.read_to_string(&mut stderr).unwrap();
        stderr
    });
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "jsonrpc":"2.0",
            "id":0,
            "method":"tools/call",
            "params":{"name":"aife_status","arguments":{}}
        })
    )
    .unwrap();
    stdin.flush().unwrap();

    let connect_deadline = Instant::now() + Duration::from_secs(10);
    while core.active_client_bindings().is_empty() {
        dispatcher.pump(&mut core, &mut session);
        if let Some(status) = child.try_wait().unwrap() {
            drop(stdin);
            let stdout = stdout_thread.join().unwrap();
            let stderr = stderr_thread.join().unwrap();
            panic!(
                "MCP process exited before its first fixed session connected ({status}); stdout: {stdout}; stderr: {stderr}"
            );
        }
        assert!(
            Instant::now() < connect_deadline,
            "MCP process did not establish its first fixed session"
        );
        std::thread::yield_now();
    }
    let old_session_id = core.active_client_bindings()[0].client_session_id.clone();
    let old_binding = core.active_client_bindings()[0].clone();
    let old_project = old_binding
        .project_context
        .as_ref()
        .expect("first MCP project context");
    let goal = editor_core::AiGoalBinding::new(
        "mcp-reconnect-process-goal",
        "Validate bounded mutation authority is invalidated by a project switch.",
        old_project.project_identity.clone(),
        old_project.project_digest.clone(),
        editor_core::AiGoalCompletionPolicy::CommitVerified,
    )
    .unwrap();
    core.request_goal_mutation_access(
        &session,
        &old_session_id,
        goal,
        editor_core::AiRiskEnvelope::default_project_owned_low_risk().unwrap(),
    )
    .unwrap();
    let old_access_request = core
        .approval_inbox(now_epoch_ms())
        .into_iter()
        .find(|request| request.client_session_id == old_session_id)
        .expect("old MCP session mutation approval request");
    let old_approval = core
        .decide_access(
            &session,
            &old_access_request.request_id,
            GatewayAccessDecision::Approve,
            "mcp-reconnect-process-test",
            now_epoch_ms(),
        )
        .expect("approve old MCP mutation grant");
    assert!(old_approval.grant_ref.is_some());
    assert!(core.client_has_active_grant(&old_session_id));

    let switched = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: second_root.display().to_string(),
    }));
    assert_eq!(switched.status, CommandStatus::Committed);
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"tools/call",
            "params":{"name":"aife_status","arguments":{}}
        })
    )
    .unwrap();
    writeln!(
        stdin,
        "{}",
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}})
    )
    .unwrap();
    drop(stdin);

    let exit_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        dispatcher.pump(&mut core, &mut session);
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= exit_deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = stderr_thread.join().unwrap();
            panic!("stable MCP process timed out after shutdown; stderr: {stderr}");
        }
        std::thread::yield_now();
    }

    let status = child.wait().unwrap();
    let stdout = stdout_thread.join().unwrap();
    let stderr = stderr_thread.join().unwrap();
    assert!(status.success(), "MCP stderr: {stderr}");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 3, "MCP stderr: {stderr}");
    let initial_status_reply = responses
        .iter()
        .find(|response| response["id"] == 0)
        .expect("initial same-process status response");
    assert_eq!(
        initial_status_reply["result"]["structuredContent"]["sessionBinding"]["clientSessionId"],
        old_session_id
    );
    let status_reply = responses
        .iter()
        .find(|response| response["id"] == 1)
        .expect("same-process status response");
    assert_eq!(status_reply["result"]["isError"], false);
    assert_eq!(
        status_reply["result"]["structuredContent"]["sessionBinding"]["clientSessionId"],
        old_session_id
    );
    let status_payload = &status_reply["result"]["structuredContent"]["payload"]["reply"];
    assert_eq!(status_payload["session"]["id"], old_session_id);
    assert_eq!(
        status_payload["project"]["identity"],
        second_project.project_id
    );
    assert_eq!(status_payload["access"]["read"]["state"], "active");
    assert_eq!(
        status_payload["access"]["mutation"]["state"],
        "not_requested"
    );
    assert!(status_payload["access"]["mutation"]["grantDigest"].is_null());
    assert!(!core.client_has_active_grant(&old_session_id));

    server.join().unwrap();
    drop(publication);
    let _ = std::fs::remove_dir_all(root);
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ai-tool-gateway-mcp-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn tool_call(id: i64, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    })
}

fn mutation_direct_input(label: &str, outcome: &str, action_id: &str) -> serde_json::Value {
    let path = format!("Input/{action_id}.input.json");
    let patch = ProjectPatchDocument::new(
        format!("process-smoke-{label}"),
        format!("Process smoke {label}"),
        PatchSource::AiAssistant,
        vec![
            PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping {
                operation_id: format!("create-{action_id}"),
                depends_on: Vec::new(),
                path: path.clone(),
            }),
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: format!("action-{action_id}"),
                depends_on: vec![format!("create-{action_id}")],
                path,
                action_id: action_id.to_string(),
                value_type: InputActionValueKind::Button,
            }),
        ],
    );
    serde_json::json!({
        "goal": {"outcome": outcome},
        "change": {
            "kind": "project_patch",
            "payload": serde_json::to_value(patch).unwrap()
        }
    })
}

fn spawn_mcp_stdout_reader(
    stdout: impl Read + Send + 'static,
) -> (
    Receiver<Result<serde_json::Value, String>>,
    std::thread::JoinHandle<Result<(), String>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let line = line.map_err(|error| error.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            let value = serde_json::from_str::<serde_json::Value>(&line)
                .map_err(|error| format!("{error}; line: {line}"))?;
            tx.send(Ok(value))
                .map_err(|error| format!("stdout receiver dropped: {error}"))?;
        }
        Ok(())
    });
    (rx, handle)
}

fn write_mcp_request(stdin: &mut ChildStdin, request: serde_json::Value) {
    writeln!(stdin, "{}", serde_json::to_string(&request).unwrap()).unwrap();
    stdin.flush().unwrap();
}

#[allow(clippy::too_many_arguments)]
fn wait_for_mcp_response(
    stdout_rx: &Receiver<Result<serde_json::Value, String>>,
    child: &mut Child,
    core: &mut GatewayCore,
    session: &mut EditorSession,
    dispatcher: &mut GatewayOwnerThreadDispatcher,
    expected_id: i64,
    pump_operations: bool,
    deadline: Instant,
) -> serde_json::Value {
    loop {
        match stdout_rx.try_recv() {
            Ok(Ok(response)) => {
                assert_eq!(
                    response["id"], expected_id,
                    "unexpected MCP response order: {response}"
                );
                return response;
            }
            Ok(Err(error)) => panic!("MCP stdout reader failed: {error}"),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                panic!("MCP stdout closed before response {expected_id}")
            }
        }
        if pump_operations {
            core.pump_operations(session, 8);
        }
        dispatcher.pump(core, session);
        if let Some(status) = child.try_wait().unwrap() {
            panic!("MCP child exited before response {expected_id}: {status}");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("MCP child timed out waiting for response {expected_id}");
        }
        std::thread::yield_now();
    }
}

fn wait_for_mcp_exit(
    child: &mut Child,
    core: &mut GatewayCore,
    session: &mut EditorSession,
    dispatcher: &mut GatewayOwnerThreadDispatcher,
    deadline: Instant,
) {
    loop {
        dispatcher.pump(core, session);
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("MCP child timed out during shutdown");
        }
        std::thread::yield_now();
    }
}
