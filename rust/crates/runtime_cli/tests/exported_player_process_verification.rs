use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::release_package_manifest::{
    release_payload_hash, ReleasePackageApplication, ReleasePackageFile, ReleasePackageFileRole,
    ReleasePackageLaunch, ReleasePackageManifest, ReleasePackageTarget,
    RELEASE_PACKAGE_MANIFEST_FILE_NAME, RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION,
};
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationRequest,
    ExportedPlayerProcessVerificationStatus,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn exported_player_process_verifier_runs_staged_game_exe_and_reads_child_report() {
    let root = temp_root("runs-staged-game-exe");
    let exported = root.join("Build").join("Windows").join("dev");
    stage_exported_package(&exported);
    let game_exe = exported.join(if cfg!(windows) { "Game.exe" } else { "Game" });
    fs::copy(env!("CARGO_BIN_EXE_ai_engine_runtime_cli"), &game_exe).unwrap();
    // This fixture tests the generic empty-runtime process contract. Current
    // dev DLL composition is covered by desktop_dev_consistency and the owner.
    write_release_package_manifest(&exported, &game_exe, None);

    let report = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
        exported_package_dir: exported.clone(),
        mode: "headless-gate".to_string(),
        frame_limit: 3,
        report_path: Some(
            exported.join("reports/exported-player-process-verification-report.json"),
        ),
        timeout_ms: 30_000,
        screenshot: false,
        screenshot_path: None,
    });

    assert_eq!(
        report.status,
        ExportedPlayerProcessVerificationStatus::Passed
    );
    assert_eq!(report.process_exit_code, Some(0));
    assert_eq!(report.child_player_exit_code, Some(0));
    assert_eq!(report.child_frames_completed, Some(3));
    assert!(exported
        .join("reports")
        .join("windowed-player-run-report.json")
        .exists());
    assert!(exported
        .join("reports")
        .join("exported-player-process-verification-report.json")
        .exists());
}

#[test]
fn exported_game_exe_verify_entry_spawns_child_player_and_writes_parent_report() {
    let root = temp_root("game-exe-verify-entry");
    let exported = root.join("Build").join("Windows").join("dev");
    stage_exported_package(&exported);
    let game_exe = exported.join(if cfg!(windows) { "Game.exe" } else { "Game" });
    fs::copy(env!("CARGO_BIN_EXE_ai_engine_runtime_cli"), &game_exe).unwrap();
    write_release_package_manifest(&exported, &game_exe, None);
    let parent_report = exported
        .join("reports")
        .join("exported-player-process-verification-report.json");

    let status = Command::new(&game_exe)
        .arg("verify-exported-player")
        .arg("--package")
        .arg(&exported)
        .arg("--mode")
        .arg("headless-gate")
        .arg("--frames")
        .arg("2")
        .arg("--report")
        .arg(&parent_report)
        .current_dir(&exported)
        .status()
        .unwrap();

    assert!(status.success());
    let report: runtime_cli::ExportedPlayerProcessVerificationReport =
        serde_json::from_str(&fs::read_to_string(parent_report).unwrap()).unwrap();
    assert_eq!(
        report.status,
        ExportedPlayerProcessVerificationStatus::Passed
    );
    assert_eq!(report.process_exit_code, Some(0));
    assert_eq!(report.child_player_exit_code, Some(0));
    assert_eq!(report.child_frames_completed, Some(2));
}

#[cfg(not(feature = "real-window"))]
#[test]
fn desktop_dev_game_exe_zero_arg_entrypoint_rejects_missing_identity() {
    let root = temp_root("desktop-dev-zero-arg-entrypoint");
    let exported = root.join("Build").join("Windows").join("dev");
    stage_exported_package(&exported);
    let game_exe = exported.join(if cfg!(windows) { "Game.exe" } else { "Game" });
    fs::copy(env!("CARGO_BIN_EXE_ai_engine_runtime_cli"), &game_exe).unwrap();

    let output = Command::new(&game_exe).current_dir(&root).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("desktop_dev_identity_missing"), "{stderr}");
}

#[cfg(not(feature = "real-window"))]
#[test]
fn exported_game_exe_windowed_screenshot_reports_feature_disabled_without_faking_pass() {
    let root = temp_root("game-exe-windowed-screenshot-feature-disabled");
    let exported = root.join("Build").join("Windows").join("dev");
    stage_exported_package(&exported);
    let game_exe = exported.join(if cfg!(windows) { "Game.exe" } else { "Game" });
    fs::copy(env!("CARGO_BIN_EXE_ai_engine_runtime_cli"), &game_exe).unwrap();
    write_release_package_manifest(&exported, &game_exe, None);
    let parent_report = exported
        .join("reports")
        .join("exported-player-process-verification-report.json");
    let screenshot = exported
        .join("reports")
        .join("windowed-player-screenshot.png");

    let status = Command::new(&game_exe)
        .arg("verify-exported-player")
        .arg("--package")
        .arg(&exported)
        .arg("--mode")
        .arg("windowed")
        .arg("--frames")
        .arg("1")
        .arg("--screenshot")
        .arg("--screenshot-path")
        .arg(&screenshot)
        .arg("--report")
        .arg(&parent_report)
        .current_dir(&exported)
        .status()
        .unwrap();

    assert!(!status.success());
    let report: runtime_cli::ExportedPlayerProcessVerificationReport =
        serde_json::from_str(&fs::read_to_string(parent_report).unwrap()).unwrap();
    assert_eq!(
        report.status,
        ExportedPlayerProcessVerificationStatus::Failed
    );
    assert_eq!(
        report.child_present_status.as_deref(),
        Some("native_host_required")
    );
    assert!(!screenshot.exists());
}

#[test]
fn release_package_verification_runs_zero_arg_entrypoint_with_timeout_and_report_off() {
    let root = temp_root("release-zero-arg-entrypoint");
    let exported = root.join("ComplexShooter");
    stage_exported_package(&exported);
    let entrypoint = exported.join("ComplexShooter.exe");
    fs::copy(env!("CARGO_BIN_EXE_ai_engine_runtime_cli"), &entrypoint).unwrap();
    write_release_package_manifest(&exported, &entrypoint, Some(1));

    let mut child = Command::new(&entrypoint)
        .current_dir(&root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let status = wait_with_timeout(&mut child, Duration::from_secs(30));

    #[cfg(feature = "real-window")]
    assert!(status.success());
    #[cfg(not(feature = "real-window"))]
    assert_eq!(status.code(), Some(1));
    assert!(!exported.join("reports").exists());
    assert!(!exported.join("data/runtime_package/reports").exists());
}

fn stage_exported_package(exported: &Path) {
    let package = exported.join("data").join("runtime_package");
    fs::create_dir_all(package.join("scenes")).unwrap();
    fs::create_dir_all(package.join("assets")).unwrap();
    fs::create_dir_all(package.join("input")).unwrap();
    fs::create_dir_all(package.join("rules")).unwrap();
    fs::create_dir_all(exported.join("reports")).unwrap();
    fs::write(
        exported.join("package-manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": "desktop-package-manifest.v1",
            "target": "windows",
            "profile": "dev",
            "packageDir": exported.display().to_string(),
            "runtimePackageDir": package.display().to_string(),
            "reportsDir": exported.join("reports").display().to_string(),
            "playerExecutable": null,
            "playerExecutableStatus": "test-staged"
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        package.join("manifest.json"),
        r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "project-exported-player-process-test",
    "name": "Exported Player Process Test",
    "version": "0.1.0",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 1 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "none" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.none", "mappingCount": 1 },
  "contentHash": "testhash"
}"#,
    )
    .unwrap();
    fs::write(
        package.join("scenes").join("scene-main.json"),
        r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#101010",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
    "id": "entity-main",
    "name": "Main Entity",
    "kind": "actor",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    }
  }]
}"##,
    )
    .unwrap();
    fs::write(
        package.join("assets").join("asset-manifest.json"),
        r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [{
    "id": "scene-main",
    "name": "Main",
    "type": "scene",
    "source": "scenes/scene-main.json",
    "state": "available",
    "bundleId": "startup"
  }],
  "runtimeAssetIndex": [{
    "assetGuid": "scene-main",
    "assetId": "scene-main",
    "assetType": "scene",
    "subAssetId": null,
    "version": "1",
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "loaderKind": "scene",
    "dependencies": [],
    "hash": null,
    "size": null,
    "flags": ["test"]
  }],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [{
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "path": "scenes/scene-main.json",
    "offset": null,
    "size": null,
    "compression": "none",
    "hash": null
  }],
  "dependencyTable": []
}"#,
    )
    .unwrap();
    fs::write(
        package.join("rules").join("rule-manifest.json"),
        r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "none",
  "rules": [],
  "modules": []
}"#,
    )
    .unwrap();
    fs::write(
        package.join("input").join("input-manifest.json"),
        r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.none",
  "mappings": [{ "id": "input.none", "path": "input/input.none.json", "enabled": true }]
}"#,
    )
    .unwrap();
    fs::write(
        package.join("input").join("input.none.json"),
        r#"{
  "schema_version": "input-mapping.v2",
  "asset_id": "input.none",
  "actions": [],
  "contexts": [],
  "bindings": [],
  "platform_overrides": []
}"#,
    )
    .unwrap();
}

#[cfg(windows)]
fn semantic_scenario(
    id: &str,
    timeout: u64,
) -> runtime_player_winit::semantic_outcome::PlaytestScenario {
    serde_json::from_value(serde_json::json!({
        "schemaVersion":"playtest-scenario.v1", "scenarioId":id, "initialSceneId":"scene-main",
        "target":"windows-headless", "maxSimulationTicks":3, "maxPresentationFrames":8, "timeoutMs":timeout
    })).unwrap()
}

#[cfg(windows)]
#[test]
fn semantic_playtest_real_worker_and_invalid_scene_preserve_outcome() {
    use runtime_player_winit::semantic_outcome::OutcomeStatus;
    let root = temp_root("semantic-worker");
    stage_exported_package(&root);
    let request = runtime_cli::SemanticPlaytestProcessRequest {
        player_executable: env!("CARGO_BIN_EXE_ai_engine_runtime_cli").into(),
        runtime_package: root.join("data/runtime_package"),
        scenario: semantic_scenario("empty", 5000),
        output_dir: root.join("valid"),
    };
    let report = runtime_cli::run_bounded_semantic_playtest(request.clone()).unwrap();
    assert_eq!(report.overall, OutcomeStatus::Passed, "{report:#?}");
    assert_eq!(report.outcome.gameplay, OutcomeStatus::NotChecked);
    assert_eq!(report.outcome.visual, OutcomeStatus::NotChecked);
    assert!(report.process.owned_process_cleanup_confirmed());
    assert_eq!(
        report
            .player
            .unwrap()
            .semantic_playtest
            .unwrap()
            .simulation_ticks,
        3
    );
    assert!(runtime_cli::run_bounded_semantic_playtest(request.clone())
        .unwrap_err()
        .contains("output_not_fresh"));
    let mut invalid = request;
    invalid.output_dir = root.join("invalid");
    invalid.scenario.initial_scene_id = "wrong-scene".into();
    let report = runtime_cli::run_bounded_semantic_playtest(invalid).unwrap();
    assert_eq!(report.overall, OutcomeStatus::Failed);
    assert!(report.process.owned_process_cleanup_confirmed());
    assert!(report
        .player
        .unwrap()
        .diagnostics
        .iter()
        .any(|d| d.code == "playtest.initial_scene_mismatch"));
    let scenario_path = root.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec(&semantic_scenario("cli", 5000)).unwrap(),
    )
    .unwrap();
    let cli = runtime_cli::run_bounded_child_process(runtime_cli::BoundedChildProcessRequest {
        executable: env!("CARGO_BIN_EXE_ai_engine_runtime_cli").into(),
        args: vec![
            "playtest".into(),
            "--package".into(),
            root.join("data/runtime_package").into_os_string(),
            "--scenario".into(),
            scenario_path.into_os_string(),
            "--output-dir".into(),
            root.join("cli").into_os_string(),
        ],
        current_dir: root.clone(),
        environment: Vec::new(),
        timeout: Duration::from_secs(10),
        stdout_capture_limit_bytes: 32 * 1024,
        stderr_capture_limit_bytes: 4096,
        priority: runtime_cli::BoundedChildProcessPriority::Normal,
    });
    assert_eq!(cli.exit_code, Some(0), "{cli:#?}");
    assert!(cli.owned_process_cleanup_confirmed());
    let report: runtime_cli::SemanticPlaytestProcessReport =
        serde_json::from_slice(&fs::read(root.join("cli/result.json")).unwrap()).unwrap();
    assert_eq!(report.overall, OutcomeStatus::Passed);
    let reference =
        runtime_cli::retain_semantic_playtest_evidence(&root.join("cli/result.json")).unwrap();
    for _ in 0..2 {
        let observed = runtime_cli::read_semantic_playtest_evidence(&reference).unwrap();
        assert_eq!(observed.run_id, report.run_id);
        assert_eq!(observed.process.process_id, report.process.process_id);
    }
    let mut stale = reference.clone();
    stale.run_id = "stale-run".into();
    assert!(runtime_cli::read_semantic_playtest_evidence(&stale)
        .unwrap_err()
        .contains("identity_mismatch"));
    let input_path = root.join("cli/request.json");
    let mut input: serde_json::Value =
        serde_json::from_slice(&fs::read(&input_path).unwrap()).unwrap();
    input["scenario"]["scenarioId"] = "other-scenario".into();
    fs::write(&input_path, serde_json::to_vec(&input).unwrap()).unwrap();
    assert!(runtime_cli::read_semantic_playtest_evidence(&reference)
        .unwrap_err()
        .contains("identity_mismatch"));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn semantic_playtest_missing_report_nonzero_and_timeout_fail_closed() {
    use runtime_player_winit::semantic_outcome::OutcomeStatus;
    let root = temp_root("semantic-faults");
    fs::create_dir_all(&root).unwrap();
    for (id, expected) in [
        (
            "fixture.no-report",
            runtime_cli::BoundedChildProcessExitReason::Completed,
        ),
        (
            "fixture.nonzero",
            runtime_cli::BoundedChildProcessExitReason::Failed,
        ),
        (
            "fixture.timeout",
            runtime_cli::BoundedChildProcessExitReason::Timeout,
        ),
    ] {
        let output = root.join(id);
        let started = Instant::now();
        let report = runtime_cli::run_bounded_semantic_playtest(
            runtime_cli::SemanticPlaytestProcessRequest {
                player_executable: env!("CARGO_BIN_EXE_bounded_output_fixture").into(),
                runtime_package: root.clone(),
                scenario: semantic_scenario(id, if id == "fixture.timeout" { 2000 } else { 5000 }),
                output_dir: output.clone(),
            },
        )
        .unwrap();
        assert_eq!(report.process.exit_reason, expected, "{report:#?}");
        assert_eq!(report.overall, OutcomeStatus::Failed);
        assert!(report.process.owned_process_cleanup_confirmed());
        assert!(started.elapsed() < Duration::from_secs(8));
        if id == "fixture.timeout" {
            assert!(report
                .process
                .stderr_summary
                .contains("timeout first cause"));
            assert!(output.join("player.ready").exists());
            assert_semantic_processes_exited(&output.join("player.ready"));
        }
        assert!(output.join("result.json").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn semantic_playtest_owner_loss_closes_job_and_kills_descendants() {
    let root = temp_root("semantic-owner-loss");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("scenario.json"),
        serde_json::to_vec(&semantic_scenario("fixture.timeout", 5000)).unwrap(),
    )
    .unwrap();
    let mut owner = Command::new(env!("CARGO_BIN_EXE_bounded_output_fixture"))
        .arg("semantic-owner")
        .arg(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let ready = root.join("owned-run/player.ready");
    while !ready.exists() && started.elapsed() < Duration::from_secs(3) {
        thread::sleep(Duration::from_millis(10));
    }
    let observed_ready = ready.exists();
    let _ = owner.kill();
    owner.wait().unwrap();
    assert!(observed_ready, "owned worker never started");
    assert_semantic_processes_exited(&ready);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
fn assert_semantic_processes_exited(ready: &Path) {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let pids: Vec<u32> = serde_json::from_slice(&fs::read(ready).unwrap()).unwrap();
    for pid in pids {
        let started = Instant::now();
        loop {
            // Query only the fixture-owned process IDs, never terminate an unrelated process.
            let exited = unsafe {
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if handle.is_null() {
                    assert_eq!(GetLastError(), ERROR_INVALID_PARAMETER);
                    true
                } else {
                    let mut code = 0;
                    let queried = GetExitCodeProcess(handle, &mut code);
                    CloseHandle(handle);
                    assert_ne!(queried, 0);
                    code != 259
                }
            };
            if exited {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "owned PID {pid} still running"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn write_release_package_manifest(
    exported: &Path,
    entrypoint: &Path,
    user_frame_limit: Option<u64>,
) {
    fs::remove_dir_all(exported.join("reports")).unwrap();
    let runtime_manifest = exported.join("data/runtime_package/manifest.json");
    let entrypoint_bytes = fs::read(entrypoint).unwrap();
    let runtime_manifest_bytes = fs::read(&runtime_manifest).unwrap();
    let files = vec![
        ReleasePackageFile {
            path: entrypoint
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            size: entrypoint_bytes.len() as u64,
            sha256: sha256_prefixed(&entrypoint_bytes),
            roles: vec![
                ReleasePackageFileRole::Entrypoint,
                ReleasePackageFileRole::Runtime,
            ],
        },
        ReleasePackageFile {
            path: "data/runtime_package/manifest.json".to_string(),
            size: runtime_manifest_bytes.len() as u64,
            sha256: sha256_prefixed(&runtime_manifest_bytes),
            roles: vec![ReleasePackageFileRole::RuntimePayload],
        },
    ];
    let manifest = ReleasePackageManifest {
        schema_version: RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION.to_string(),
        application: ReleasePackageApplication {
            display_name: "Complex Shooter".to_string(),
            executable_name: entrypoint
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            company_name: "AI First Engine Studio".to_string(),
            file_description: "Complex Shooter".to_string(),
            display_version: "1.0.0".to_string(),
            windows_file_version: [1, 0, 0, 0],
            windows_product_version: [1, 0, 0, 0],
            copyright: "Copyright AI First Engine Studio".to_string(),
        },
        target: ReleasePackageTarget {
            platform: "windows".to_string(),
            architecture: "x86_64".to_string(),
            profile: "release".to_string(),
        },
        launch: ReleasePackageLaunch { user_frame_limit },
        entrypoint: entrypoint
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        runtime_package: "data/runtime_package".to_string(),
        runtime_content_hash: format!("sha256:{}", "a".repeat(64)),
        release_payload_hash: release_payload_hash(&files),
        files,
    };
    fs::write(
        exported.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> ExitStatus {
    let started = Instant::now();
    loop {
        match child.try_wait().unwrap() {
            Some(status) => return status,
            None if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("zero-argument release entrypoint exceeded {timeout:?}");
            }
        }
    }
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("runtime-cli-{name}-{stamp}"))
}
