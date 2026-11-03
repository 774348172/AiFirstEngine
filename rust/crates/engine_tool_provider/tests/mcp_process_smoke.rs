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

#[cfg(windows)]
#[test]
#[ignore = "A4.2 real stdio Provider, independent Windows delivery and installed-reader negative"]
fn three_d_a4_stdio_roundtrip() {
    let evidence =
        PathBuf::from(std::env::var_os("AIFE_A42_EVIDENCE_ROOT").expect("evidence root"));
    let delivery =
        PathBuf::from(std::env::var_os("AIFE_A42_DELIVERY_ROOT").expect("fresh external delivery"));
    assert!(!delivery.exists(), "delivery must be fresh");
    fs::create_dir_all(&evidence).unwrap();
    let root = unique_temp_dir("three-d-a42");
    let _guard = TestDirectoryGuard(root.clone());
    let project = root.join("project");
    copy_source_tree(
        &workspace_root().join("samples/three_d_lab_project"),
        &project,
    );
    let project = fs::canonicalize(project).unwrap();
    let save = |name: &str, value: &Value| {
        fs::write(
            evidence.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap()
    };
    save(
        "ownership",
        &json!({"temporaryRoot":root,"project":project,"delivery":delivery}),
    );
    let mut provider = McpProcess::spawn(&project);
    let initialize =
        provider.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    save("initialize", &initialize);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "ai-first-game-engine"
    );
    let listed =
        provider.request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}));
    save("tools", &listed);
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 9);
    let inspect = provider.tool_call(3, "engine_project_inspect", json!({}));
    save("inspect", &inspect);
    assert_completed("3D inspect", &inspect);
    assert_eq!(
        canonical(&inspect)["output"]["spatialInspection"]["scenes"][0]["mainCamera"],
        "camera"
    );
    let check = provider.tool_call(4, "engine_project_check", json!({}));
    save("check", &check);
    assert_completed("3D check", &check);
    let built = provider.tool_call(
        5,
        "engine_project_build",
        json!({"targetProfile":"windows-dev","frameLimit":120}),
    );
    save("build", &built);
    assert_completed("3D build", &built);
    let output = &canonical(&built)["output"];
    let package = PathBuf::from(output["packageDir"].as_str().unwrap());
    let played = provider.tool_call(
        6,
        "engine_runtime_playtest",
        json!({"deliveryRef":output["deliveryRef"]}),
    );
    save("playtest", &played);
    assert_completed("3D playtest", &played);
    let semantic = &canonical(&played)["output"]["semantic"];
    assert_eq!(semantic["overall"], "passed");
    assert_eq!(semantic["assertions"].as_array().unwrap().len(), 13);
    assert_eq!(semantic["captureEvidence"].as_array().unwrap().len(), 7);
    let worker = PathBuf::from(semantic["captureEvidence"][0]["path"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_owned();
    a42_copy(&worker, &evidence.join("original-worker"));
    let observed = provider.tool_call(
        7,
        "engine_runtime_observe",
        json!({"runRef":canonical(&played)["output"]["runRef"]}),
    );
    save("observe", &observed);
    assert_completed("3D observe", &observed);
    assert_eq!(canonical(&observed)["output"], canonical(&played)["output"]);
    let verified=provider.tool_call(8,"engine_delivery_verify",json!({"deliveryRef":output["deliveryRef"],"mode":"windowed","frameLimit":120,"timeoutMs":30000,"screenshot":true}));
    save("original-verify", &verified);
    assert_completed("3D verify", &verified);
    assert_eq!(canonical(&verified)["output"]["status"], "passed");
    for key in ["deliveryIdentity", "artifactIdentity"] {
        assert_eq!(output[key], canonical(&played)["output"][key]);
        assert_eq!(output[key], canonical(&verified)["output"][key]);
    }
    save(
        "provider",
        &json!({"path":env!("CARGO_BIN_EXE_ai_engine_tool_provider_mcp"),"sha256":a42_hash(Path::new(env!("CARGO_BIN_EXE_ai_engine_tool_provider_mcp"))),"pid":provider.child.as_ref().unwrap().id()}),
    );
    save("transcript", &json!(provider.transcript));
    provider.shutdown();
    a42_copy(&package, &delivery);
    let inventory = a42_inventory(&package);
    assert_eq!(inventory, a42_inventory(&delivery));
    save("copied-inventory", &json!(inventory));
    let manifest: Value =
        serde_json::from_slice(&fs::read(delivery.join("package-manifest.json")).unwrap()).unwrap();
    for (file, key) in [
        ("Game.exe", "playerArtifactHash"),
        ("engine_runtime.dll", "engineRuntimeHash"),
        (
            "data/bin/sample_three_d_lab_runtime.dll",
            "projectRuntimeModuleHash",
        ),
    ] {
        assert_eq!(
            a42_hash(&delivery.join(file)),
            manifest[key].as_str().unwrap()
        );
    }
    save("package-manifest", &manifest);
    // The older installed consumer only sees this owned source copy, never the live host session.
    let old = PathBuf::from(std::env::var_os("AIFE_A42_OLD_PROVIDER").expect("357 provider"));
    let old_hash = a42_hash(&old);
    let mut old_command = Command::new(&old);
    old_command
        .args(["--workspace-root"])
        .arg(&project)
        .arg("--project-root")
        .arg(&project)
        .arg("--session-id")
        .arg("a42-old-reader");
    let old_input=b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"engine_project_check\",\"arguments\":{}}}\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"shutdown\",\"params\":{}}\n";
    let old_status = a42_run(&mut old_command, &evidence, "old-provider", Some(old_input));
    let old_text = fs::read_to_string(evidence.join("old-provider.stdout")).unwrap()
        + &fs::read_to_string(evidence.join("old-provider.stderr")).unwrap();
    save(
        "old-reader",
        &json!({"path":old,"sha256Before":old_hash,"sha256After":a42_hash(&old),"exitCode":old_status.code(),"output":old_text}),
    );
    assert_eq!(old_hash, a42_hash(&old));
    assert!(
        old_text.contains("aife-project.v3")
            && (old_text.contains("unsupported") || old_text.contains("Unsupported")),
        "{old_text}"
    );
    // Make the complete source+Library+build-cache tree inaccessible at its original path.
    fs::rename(&project, root.join("source-isolated")).unwrap();
    assert!(!project.exists() && !package.exists());
    let reports = delivery.join("reports/a42-independent");
    fs::create_dir_all(&reports).unwrap();
    let report_path = reports.join("verify.json");
    let mut command = Command::new(delivery.join("Game.exe"));
    command
        .current_dir(&delivery)
        .arg("verify-exported-player")
        .arg("--package")
        .arg(&delivery)
        .args(["--mode", "windowed", "--frames", "120", "--report"])
        .arg(&report_path)
        .arg("--screenshot")
        .arg("--screenshot-path")
        .arg(reports.join("window.png"));
    let status = a42_run(&mut command, &evidence, "independent-verify", None);
    let verification: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    save("independent-verify", &verification);
    assert!(status.success());
    assert_eq!(verification["status"], "passed");
    assert_eq!(verification["childFramesCompleted"], 120);
    assert_eq!(verification["childPresentStatus"], "presented");
    let mut request: Value =
        serde_json::from_slice(&fs::read(evidence.join("original-worker/request.json")).unwrap())
            .unwrap();
    request["runtimePackage"] = json!(delivery.join("data/runtime_package"));
    request["runId"] = json!("a42-independent-semantic");
    let request_path = reports.join("request.json");
    fs::write(&request_path, serde_json::to_vec_pretty(&request).unwrap()).unwrap();
    let player_path = reports.join("player.json");
    let mut command = Command::new(delivery.join("Game.exe"));
    command
        .current_dir(&delivery)
        .arg("--semantic-playtest-worker")
        .arg(&request_path)
        .arg(&player_path);
    let status = a42_run(&mut command, &evidence, "independent-semantic", None);
    let player: Value = serde_json::from_slice(&fs::read(&player_path).unwrap()).unwrap();
    save("independent-player", &player);
    assert!(status.success());
    assert_eq!(player["presentStatus"], "presented");
    let isolated = &player["semanticPlaytest"];
    assert_eq!(isolated["overall"], "passed");
    assert_eq!(isolated["injectedTransitionCount"], 17);
    assert_eq!(isolated["assertions"].as_array().unwrap().len(), 13);
    assert!(isolated["assertions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["status"] == "passed"));
    assert_eq!(isolated["captureEvidence"].as_array().unwrap().len(), 7);
    for capture in isolated["captureEvidence"].as_array().unwrap() {
        let path = PathBuf::from(capture["path"].as_str().unwrap());
        assert!(path.is_file());
        assert_eq!(a42_hash(&path), capture["sha256"].as_str().unwrap());
    }
    assert!(player["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |d| d["code"] == "engine_runtime.execution_owner" && d["layer"] == "engine_runtime.dll"
        ));
    assert!(player["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "graphics.backend_selected" && d["message"] == "Vulkan"));
    // Runtime reports are mutable; all originally copied runtime inputs stay byte-identical.
    for (relative, digest) in &inventory {
        if relative.starts_with("reports/") {
            continue;
        }
        assert_eq!(
            &a42_hash(&delivery.join(relative)),
            digest,
            "changed {relative}"
        );
    }
    save(
        "qualification",
        &json!({"status":"passed","sourcePathUnavailable":!project.exists(),"originalPackageUnavailable":!package.exists(),"delivery":delivery,"fileCount":inventory.len(),"build":output,"verify":verification,"assertions":13,"captures":7,"inputTransitions":17,"installedUnchanged":old_hash==a42_hash(&old)}),
    );
}

#[cfg(windows)]
fn a42_hash(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(fs::read(path).unwrap()))
}
#[cfg(windows)]
fn a42_copy(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        let kind = entry.file_type().unwrap();
        assert!(!kind.is_symlink());
        if kind.is_dir() {
            a42_copy(&entry.path(), &target)
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
#[cfg(windows)]
fn a42_inventory(root: &Path) -> std::collections::BTreeMap<String, String> {
    fn walk(root: &Path, current: &Path, out: &mut std::collections::BTreeMap<String, String>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &entry.path(), out)
            } else {
                out.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    a42_hash(&entry.path()),
                );
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}
#[cfg(windows)]
fn a42_run(
    command: &mut Command,
    evidence: &Path,
    label: &str,
    input: Option<&[u8]>,
) -> std::process::ExitStatus {
    command
        .stdout(Stdio::from(
            fs::File::create(evidence.join(format!("{label}.stdout"))).unwrap(),
        ))
        .stderr(Stdio::from(
            fs::File::create(evidence.join(format!("{label}.stderr"))).unwrap(),
        ));
    command.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        let mut stdin = child.stdin.take().unwrap();
        let _ = stdin.write_all(input);
    }
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() > std::time::Duration::from_secs(90) {
            let _ = Command::new("taskkill")
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .output();
            let _ = child.kill();
            let _ = child.wait();
            panic!("{label} exceeded 90s; owned process tree terminated");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
