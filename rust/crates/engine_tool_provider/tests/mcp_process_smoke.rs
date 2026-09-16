use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn mcp_process_smoke_runs_without_editor_or_gateway() {
    let source = workspace_root().join("samples/complex_shooter_project");
    let root = unique_temp_dir("mcp-process");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(&source, &project);
    let project = fs::canonicalize(project).expect("canonical copied project root");

    let mut process = McpProcess::spawn(&project);
    let initialize = process.request(json!({
        "jsonrpc":"2.0","id":1,"method":"initialize","params":{}
    }));
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "ai-first-game-engine"
    );

    let listed = process.request(json!({
        "jsonrpc":"2.0","id":2,"method":"tools/list","params":{}
    }));
    let tools = listed["result"]["tools"]
        .as_array()
        .expect("tools/list returns tools");
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "engine_project_inspect",
            "engine_project_check",
            "engine_project_mutate",
            "engine_project_rollback",
            "engine_runtime_run",
            "engine_project_build",
            "engine_runtime_playtest",
            "engine_runtime_observe",
            "engine_delivery_verify"
        ]
    );
    assert!(tools.iter().all(|tool| {
        tool["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("engine_") && !name.contains("gateway"))
    }));
    for name in [
        "engine_project_check",
        "engine_runtime_run",
        "engine_project_build",
        "engine_runtime_playtest",
        "engine_runtime_observe",
        "engine_delivery_verify",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("missing MCP tool {name}"));
        assert_eq!(tool["_meta"]["engine/maturity"], "ready");
    }

    let inspected = process.tool_call(3, "engine_project_inspect", json!({}));
    assert_completed("project inspect", &inspected);
    let initial_revision = canonical(&inspected)["projectRevision"].clone();

    let scene_path = project.join("Scenes/Main.scene.json");
    let mut changed_scene = fs::read(&scene_path).expect("read copied scene");
    changed_scene.extend_from_slice(b"\n");
    fs::write(&scene_path, &changed_scene).expect("externally rewrite copied scene");

    let refreshed = process.tool_call(4, "engine_project_inspect", json!({}));
    assert_completed("project refresh", &refreshed);
    assert_ne!(canonical(&refreshed)["projectRevision"], initial_revision);

    let direct_run = process.tool_call(
        5,
        "engine_runtime_run",
        json!({"mode":"headless","frameLimit":1,"timeoutMs":30000}),
    );
    assert_completed("run without explicit check", &direct_run);

    let source_path = project.join("RuntimeModule/src/lib.rs");
    let original_source = fs::read_to_string(&source_path).expect("read copied Rust source");
    let broken_line = original_source.lines().count() + 2;
    fs::write(
        &source_path,
        format!("{original_source}\nconst MCP_CHECK_TYPE_ERROR: u32 = \"invalid\";\n"),
    )
    .expect("introduce real Rust type error");
    let broken = process.tool_call(6, "engine_project_check", json!({}));
    assert_eq!(broken["result"]["isError"], true, "{broken:#}");
    assert_eq!(
        canonical(&broken)["status"],
        "rejected_by_engine",
        "{broken:#}"
    );
    let location = &canonical(&broken)["diagnostics"][0]["sourceLocation"];
    assert_eq!(
        location["sourcePath"], "RuntimeModule/src/lib.rs",
        "{broken:#}"
    );
    assert_eq!(location["line"], broken_line, "{broken:#}");
    assert!(location["column"].as_u64().is_some_and(|column| column > 0));
    assert_eq!(canonical(&broken)["retryability"], "retry_after_correction");
    assert!(canonical(&broken)["diagnostics"][0]["compilerDiagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|detail| detail["code"] == "E0308"));
    assert_eq!(
        canonical(&broken)["recommendedLocalTransitions"][0]["toolName"],
        "engine_project_check"
    );
    assert_ne!(
        canonical(&broken)["projectRevision"],
        canonical(&direct_run)["projectRevision"]
    );

    fs::write(&source_path, &original_source).expect("Host repairs copied Rust source");
    let check = process.tool_call(7, "engine_project_check", json!({}));
    assert_completed("project check", &check);
    assert_eq!(
        canonical(&check)["output"]["checkReport"]["rustCheck"],
        "passed",
        "{check:#}"
    );
    assert_ne!(
        canonical(&broken)["projectRevision"],
        canonical(&check)["projectRevision"]
    );

    let run = process.tool_call(
        8,
        "engine_runtime_run",
        json!({"mode":"headless","frameLimit":1,"timeoutMs":30000}),
    );
    assert_completed("run", &run);

    let build = process.tool_call(
        9,
        "engine_project_build",
        json!({"targetProfile":"windows-dev","frameLimit":1}),
    );
    assert_completed("build", &build);
    let delivery_ref = canonical(&build)["output"]["deliveryRef"]
        .as_str()
        .expect("build returns deliveryRef")
        .to_string();

    let verify = process.tool_call(
        10,
        "engine_delivery_verify",
        json!({
            "deliveryRef":delivery_ref,
            "mode":"headless",
            "frameLimit":1,
            "timeoutMs":30000,
            "screenshot":false
        }),
    );
    assert_completed("delivery verification", &verify);

    let run_result = canonical(&run);
    let build_result = canonical(&build);
    let verify_result = canonical(&verify);
    assert_eq!(
        canonical(&check)["projectRevision"],
        run_result["projectRevision"]
    );
    assert_eq!(
        canonical(&check)["output"]["checkReport"]["projectRevision"],
        run_result["projectRevision"]
    );
    assert_eq!(
        run_result["projectRevision"],
        build_result["projectRevision"]
    );
    assert_eq!(
        build_result["projectRevision"],
        verify_result["projectRevision"]
    );
    assert_eq!(
        run_result["output"]["artifactIdentity"],
        build_result["output"]["artifactIdentity"]
    );
    assert_eq!(
        build_result["output"]["artifactIdentity"],
        verify_result["output"]["artifactIdentity"]
    );
    assert_eq!(
        build_result["output"]["deliveryIdentity"],
        verify_result["output"]["deliveryIdentity"]
    );
    assert_eq!(verify_result["output"]["status"], "passed");
    assert_eq!(verify_result["output"]["processExitCode"], 0);
    assert_eq!(verify_result["output"]["childPlayerExitCode"], 0);
    for result in [run_result, build_result, verify_result] {
        let evidence = result["evidenceRefs"]
            .as_array()
            .expect("canonical result carries evidenceRefs");
        assert!(!evidence.is_empty());
        assert!(evidence
            .iter()
            .all(|path| { path.as_str().is_some_and(|path| Path::new(path).is_file()) }));
    }

    add_semantic_fixture(&project);
    let failed = process.tool_call(20, "engine_runtime_playtest", json!({}));
    assert_eq!(canonical(&failed)["status"], "failed", "{failed:#}");
    assert_eq!(failed["result"]["isError"], true);
    assert_eq!(
        canonical(&failed)["output"]["outcome"]["technical"],
        "passed",
        "{failed:#}"
    );
    assert_eq!(
        canonical(&failed)["output"]["outcome"]["gameplay"],
        "failed"
    );
    assert_eq!(
        canonical(&failed)["output"]["outcome"]["visual"],
        "not-checked"
    );
    assert_eq!(
        canonical(&failed)["output"]["semantic"]["assertions"][0]["actual"],
        1
    );
    assert_eq!(canonical(&failed)["retryability"], "retry_after_correction");
    let failed_ref = canonical(&failed)["output"]["runRef"].clone();
    let failed_observe =
        process.tool_call(21, "engine_runtime_observe", json!({"runRef":failed_ref}));
    assert_completed("read failed outcome", &failed_observe);
    assert_eq!(
        canonical(&failed_observe)["output"],
        canonical(&failed)["output"]
    );

    let scenario_path = project.join("Tests/default.json");
    let mut scenario: Value = serde_json::from_slice(&fs::read(&scenario_path).unwrap()).unwrap();
    scenario["assertions"][0]["equals"] = 1.into();
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();
    let passed = process.tool_call(22, "engine_runtime_playtest", json!({}));
    assert_completed("Host repaired scenario", &passed);
    assert_eq!(canonical(&passed)["output"]["overall"], "passed");
    assert_ne!(
        canonical(&passed)["output"]["scenarioDigest"],
        canonical(&failed)["output"]["scenarioDigest"]
    );
    let replayed = process.tool_call(22, "engine_runtime_playtest", json!({}));
    assert_eq!(canonical(&replayed)["replayed"], true);
    assert_eq!(canonical(&replayed)["output"], canonical(&passed)["output"]);
    let run_ref = canonical(&passed)["output"]["runRef"].clone();
    let observed = process.tool_call(23, "engine_runtime_observe", json!({"runRef":run_ref}));
    assert_completed("observe same run", &observed);
    assert_eq!(canonical(&observed)["output"], canonical(&passed)["output"]);

    let semantic_build = process.tool_call(24, "engine_project_build", json!({"frameLimit":1}));
    assert_completed("build with frozen scenario", &semantic_build);
    fs::write(&source_path, "not valid Rust").unwrap();
    fs::write(&scenario_path, "not valid JSON").unwrap();
    let retest = process.tool_call(
        25,
        "engine_runtime_playtest",
        json!({"deliveryRef":canonical(&semantic_build)["output"]["deliveryRef"]}),
    );
    assert_completed("specified delivery despite broken live input", &retest);
    assert_eq!(
        canonical(&retest)["output"]["deliveryIdentity"],
        canonical(&semantic_build)["output"]["deliveryIdentity"]
    );
    assert_eq!(
        canonical(&retest)["projectRevision"],
        canonical(&semantic_build)["projectRevision"]
    );
    assert_eq!(
        canonical(&retest)["output"]["outcome"]["delivery"],
        "passed"
    );
    let still_observed = process.tool_call(26, "engine_runtime_observe", json!({"runRef":run_ref}));
    assert_completed("observe without live refresh", &still_observed);
    assert_eq!(
        canonical(&still_observed)["output"],
        canonical(&passed)["output"]
    );

    let fresh_provider_rejection = {
        let mut other = McpProcess::spawn(&project);
        let rejected = other.tool_call(1, "engine_runtime_observe", json!({"runRef":run_ref}));
        other.shutdown();
        rejected
    };
    assert_eq!(
        canonical(&fresh_provider_rejection)["diagnostics"][0]["code"],
        "engine_provider.run_ref_unknown"
    );
    let report_path = project.join(format!(
        "Library/EngineTools/Deliveries/{}/SemanticEvidence/Windows/result.json",
        canonical(&passed)["operationId"].as_str().unwrap()
    ));
    let retained_bytes = fs::read(&report_path).unwrap();
    if let Some(evidence) = std::env::var_os("AIFE_312_EVIDENCE") {
        fs::write(
            PathBuf::from(evidence).join("mcp-semantic-result.json"),
            &retained_bytes,
        )
        .unwrap();
    }
    fs::write(&report_path, b"{}").unwrap();
    let stale = process.tool_call(27, "engine_runtime_observe", json!({"runRef":run_ref}));
    assert_eq!(canonical(&stale)["status"], "rejected_by_engine");
    assert_eq!(
        canonical(&stale)["diagnostics"][0]["code"],
        "game_project_compiler.playtest_evidence_invalid"
    );
    fs::remove_file(&report_path).unwrap();
    let missing = process.tool_call(28, "engine_runtime_observe", json!({"runRef":run_ref}));
    assert_eq!(canonical(&missing)["status"], "rejected_by_engine");
    process.shutdown();
}

fn add_semantic_fixture(project: &Path) {
    let manifest_path = project.join("project.aife.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["playtestScenario"] = "Tests/default.json".into();
    manifest["observationContract"] = "Tests/observations.json".into();
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    fs::create_dir_all(project.join("Tests")).unwrap();
    fs::write(project.join("Tests/observations.json"), serde_json::to_vec_pretty(&json!({
        "schemaVersion":"project-observation-contract.v1", "contractId":"test.controls",
        "observations":[{"path":"controls.fireCount","type":"integer","description":"Input-triggered project rule count"}]
    })).unwrap()).unwrap();
    fs::write(project.join("Tests/default.json"), serde_json::to_vec_pretty(&json!({
        "schemaVersion":"playtest-scenario.v1", "scenarioId":"test.controls", "initialSceneId":"scene-main",
        "target":"windows-headless", "maxSimulationTicks":3, "maxPresentationFrames":6, "timeoutMs":10000,
        "inputs":[{"simulationTick":1,"keyDown":["Space"]},{"simulationTick":2,"keyUp":["Space"]}],
        "assertions":[{"assertionId":"fire", "fromSimulationTick":3,"throughSimulationTick":3,"path":"controls.fireCount","equals":99}]
    })).unwrap()).unwrap();
    let path = project.join("RuntimeModule/src/lib.rs");
    let source = fs::read_to_string(&path).unwrap();
    // Fail during fixture setup if the evolving sample no longer has these seams.
    // Silent replacements otherwise reach Player with a mismatched observation contract.
    let replace_once = |source: String, from: &str, to: &str| {
        assert_eq!(
            source.matches(from).count(),
            1,
            "semantic fixture anchor drift: {from}"
        );
        source.replacen(from, to, 1)
    };
    let source = replace_once(
        source,
        "    session_id: String,",
        "    session_id: String,\n    fire_count: i64,",
    );
    let source = replace_once(source, "        session_id: \"complex-shooter.runtime-session\".to_string(),", "        session_id: \"complex-shooter.runtime-session\".to_string(),\n        fire_count: 0,");
    let source = replace_once(source, "    let mut output = run_rule(request, world);", "    if request.rule_id == \"rule.fire-bullet\"\n        && request.input_actions.iter().any(|action| {\n            action.action_id == \"action.fire\" && action.phase.as_deref() == Some(\"pressed\")\n        })\n    {\n        session.fire_count += 1;\n    }\n    let mut output = run_rule(request, world);");
    let source = replace_once(source, "    Ok(ProjectRuntimeObservationOutput { values })", "    Ok(ProjectRuntimeObservationOutput {\n        values: BTreeMap::from([(\n            \"controls.fireCount\".to_string(),\n            ProjectRuntimeValue::Integer(session.fire_count),\n        )]),\n    })");
    fs::write(path, source).unwrap();
}

fn canonical(response: &Value) -> &Value {
    &response["result"]["structuredContent"]
}

fn assert_completed(label: &str, response: &Value) {
    assert_eq!(
        response["result"]["isError"], false,
        "{label} MCP call failed: {response:#}"
    );
    assert_eq!(canonical(response)["status"], "completed");
}

struct McpProcess {
    child: Option<Child>,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    transcript: Vec<Value>,
}

impl McpProcess {
    fn spawn(project: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ai_engine_tool_provider_mcp"))
            .arg("--workspace-root")
            .arg(project)
            .arg("--project-root")
            .arg(project)
            .arg("--session-id")
            .arg("process-smoke")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn standalone MCP provider");
        let stdin = child.stdin.take().expect("MCP stdin");
        let stdout = BufReader::new(child.stdout.take().expect("MCP stdout"));
        Self {
            child: Some(child),
            stdin,
            stdout,
            transcript: Vec::new(),
        }
    }

    fn request(&mut self, request: Value) -> Value {
        writeln!(
            self.stdin,
            "{}",
            serde_json::to_string(&request).expect("encode MCP request")
        )
        .expect("write MCP request");
        self.stdin.flush().expect("flush MCP request");
        let mut response = String::new();
        self.stdout
            .read_line(&mut response)
            .expect("read MCP response");
        assert!(!response.is_empty(), "MCP process closed before responding");
        let response = serde_json::from_str(&response).expect("decode MCP response");
        self.transcript
            .push(json!({"request":request,"response":response}));
        response
    }

    fn tool_call(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        self.request(json!({
            "jsonrpc":"2.0",
            "id":id,
            "method":"tools/call",
            "params":{"name":name,"arguments":arguments}
        }))
    }

    fn shutdown(mut self) {
        let response = self.request(json!({
            "jsonrpc":"2.0","id":11,"method":"shutdown","params":{}
        }));
        assert_eq!(response["result"], json!({}));
        let output = self
            .child
            .take()
            .expect("MCP child")
            .wait_with_output()
            .expect("wait for MCP child");
        assert!(
            output.status.success(),
            "MCP stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        if self.transcript.len() > 5 {
            if let Some(root) = std::env::var_os("AIFE_312_EVIDENCE") {
                fs::write(
                    PathBuf::from(root).join("mcp-transcript.json"),
                    serde_json::to_vec_pretty(&self.transcript).unwrap(),
                )
                .unwrap();
            }
        }
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn copy_source_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied project directory");
    for entry in fs::read_dir(source).expect("read source project directory") {
        let entry = entry.expect("read source project entry");
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some("Build" | "Library" | "target" | ".aife" | ".git")
        ) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(name);
        if entry.file_type().expect("inspect source entry").is_dir() {
            copy_source_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).expect("copy source project file");
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("aife-{label}-{}-{nonce}", std::process::id()))
}

struct TestDirectoryGuard(PathBuf);

impl Drop for TestDirectoryGuard {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained failed MCP root: {}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
