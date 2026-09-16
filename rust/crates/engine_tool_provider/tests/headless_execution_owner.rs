use engine_tool_provider::{
    CanonicalToolStatus, EngineToolProvider, HostSessionContext, HostToolCall,
    NativeEngineToolProvider,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn project_game_sdk_generated_glue_headless_run_build_delivery_verify() {
    let source = workspace_root().join("samples/complex_shooter_project");
    let root = unique_temp_dir("provider-headless-owner");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(&source, &project);
    let project = fs::canonicalize(project).expect("canonical copied project root");

    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "headless-owner".to_string(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .expect("attach provider without Editor");

    let run = provider.invoke(approved_call(
        "headless-run",
        "engine_runtime_run",
        json!({"mode":"headless","frameLimit":1,"timeoutMs":30000}),
    ));
    assert_completed("run", &run);
    assert_project_delivery(&project, &run.output);

    let build = provider.invoke(approved_call(
        "headless-build",
        "engine_project_build",
        json!({"targetProfile":"windows-dev","frameLimit":1}),
    ));
    assert_completed("build", &build);
    assert_project_delivery(&project, &build.output);
    let delivery_ref = build.output["deliveryRef"]
        .as_str()
        .expect("build returns an opaque deliveryRef")
        .to_string();

    let verify = provider.invoke(approved_call(
        "delivery-verify",
        "engine_delivery_verify",
        json!({
            "deliveryRef":delivery_ref,
            "mode":"headless",
            "frameLimit":1,
            "timeoutMs":30000,
            "screenshot":false
        }),
    ));
    assert_completed("delivery verification", &verify);
    assert_eq!(verify.output["status"], "passed");
    assert_eq!(verify.output["processExitCode"], 0);
    assert_eq!(verify.output["childPlayerExitCode"], 0);
    let absent = provider.invoke(approved_call(
        "no-scenario",
        "engine_runtime_playtest",
        json!({"deliveryRef":delivery_ref}),
    ));
    // The packaged delivery carries the project's default playtest scenario;
    // omitting an explicit scenario therefore executes that declared default.
    assert_completed("default delivery playtest", &absent);
}

#[test]
fn switch_puzzle_semantic_outcome_uses_its_own_bool_contract() {
    let root = unique_temp_dir("provider-puzzle");
    let project = root.join("project");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(
        &workspace_root().join("samples/switch_puzzle_project"),
        &project,
    );
    let manifest_path = project.join("project.aife.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["playtestScenario"] = "Tests/default.json".into();
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    fs::create_dir_all(project.join("Tests")).unwrap();
    fs::write(project.join("Tests/default.json"), serde_json::to_vec_pretty(&json!({
        "schemaVersion":"playtest-scenario.v1", "scenarioId":"puzzle.keyboard", "initialSceneId":"scene-main",
        "target":"windows-headless", "maxSimulationTicks":3, "maxPresentationFrames":6,"timeoutMs":10000,
        "inputs":[{"simulationTick":1,"keyDown":["Enter"]},{"simulationTick":2,"keyUp":["Enter"]}],
        "assertions":[{"assertionId":"solved","fromSimulationTick":3,"throughSimulationTick":3,"path":"puzzle.solved","equals":true}]
    })).unwrap()).unwrap();
    let project = fs::canonicalize(project).unwrap();
    let mut provider = EngineToolProvider::attach(HostSessionContext {
        session_id: "puzzle-owner".into(),
        workspace_root: project.clone(),
        project_root: Some(project.clone()),
    })
    .unwrap();
    let tested = provider.invoke(approved_call(
        "puzzle-playtest",
        "engine_runtime_playtest",
        json!({}),
    ));
    assert_completed("puzzle playtest", &tested);
    assert_eq!(tested.output["semantic"]["assertions"][0]["actual"], true);
    assert_eq!(tested.output["outcome"]["gameplay"], "passed");
    let delivery = project
        .join("Library/EngineTools/Deliveries")
        .join(&tested.operation_id)
        .join("Playtest/Windows/dev");
    assert!(delivery.join("Game.exe").is_file());
    assert!(
        !delivery
            .join("reports/exported-player-process-verification-report.json")
            .exists(),
        "playtest must execute the declared semantic scenario, not an additional headless preflight"
    );
    assert_eq!(tested.output["processExitCode"], 0);
    let read = provider.invoke(HostToolCall {
        call_id: "puzzle-observe".into(),
        tool_name: "engine_runtime_observe".into(),
        arguments: json!({"runRef":tested.output["runRef"]}),
        approved: false,
    });
    assert_completed("read with no process approval", &read);
    assert_eq!(read.output, tested.output);
    fs::create_dir(project.join("second")).unwrap();
    fs::write(
        project.join("second/project.aife.json"),
        br#"{"schemaVersion":"aife-project.v2","projectId":"second"}"#,
    )
    .unwrap();
    let rebound = provider.invoke(approved_call(
        "rebind",
        "engine_project_open",
        json!({"projectRoot":"second"}),
    ));
    assert_completed("project rebind", &rebound);
    let expired = provider.invoke(approved_call(
        "old-run",
        "engine_runtime_observe",
        json!({"runRef":tested.output["runRef"]}),
    ));
    assert_eq!(
        expired.diagnostics[0].code,
        "engine_provider.run_ref_unknown"
    );
    if let Some(root) = std::env::var_os("AIFE_312_EVIDENCE") {
        fs::write(
            PathBuf::from(root).join("native-puzzle.json"),
            serde_json::to_vec_pretty(&json!({"playtest":tested,"observe":read})).unwrap(),
        )
        .unwrap();
    }
}

fn approved_call(call_id: &str, tool_name: &str, arguments: Value) -> HostToolCall {
    HostToolCall {
        call_id: call_id.to_string(),
        tool_name: tool_name.to_string(),
        arguments,
        approved: true,
    }
}

fn assert_completed(label: &str, result: &engine_tool_provider::CanonicalToolResult) {
    assert_eq!(
        result.status,
        CanonicalToolStatus::Completed,
        "{label} failed: {result:#?}"
    );
}

fn assert_project_delivery(project: &Path, output: &Value) {
    let package = PathBuf::from(
        output["packageDir"]
            .as_str()
            .expect("execution returns packageDir"),
    );
    assert!(package.starts_with(project));
    assert!(package.join("Game.exe").is_file());
    assert!(package.join("package-manifest.json").is_file());
    assert!(package.join("data/runtime_package/manifest.json").is_file());
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
            eprintln!("retained failed headless owner root: {}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
