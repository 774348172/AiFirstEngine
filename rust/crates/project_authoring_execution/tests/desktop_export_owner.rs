use project_authoring_execution::{
    DesktopExportPipeline, DesktopExportRequest, DesktopExportStatus, ProjectRelativePath,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn desktop_export_owner_builds_windows_layout_and_verifies_player() {
    let evidence_root = configured_evidence_root();
    let source = workspace_root().join("samples/complex_shooter_project");
    let root = unique_temp_dir("dx");
    let project = root.join("project");
    let player_build = root.join("player-build");
    let _guard = TestDirectoryGuard(root);
    copy_source_tree(&source, &project);
    // Keep the fixture compatible with both canonical snake_case files and
    // the assembler's externally accepted documentId spelling.
    let hud_path = project.join("AUI/hud.aui.json");
    let mut hud: serde_json::Value = serde_json::from_slice(&fs::read(&hud_path).unwrap()).unwrap();
    hud["documentId"] = serde_json::Value::String("hud-main".to_string());
    fs::write(&hud_path, serde_json::to_vec(&hud).unwrap()).unwrap();

    let manifest_path = project.join("project.aife.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["playtestScenario"] = "Tests/default.json".into();
    manifest["observationContract"] = "Tests/observations.json".into();
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::create_dir_all(project.join("Tests")).unwrap();
    fs::write(project.join("Tests/observations.json"), br#"{"schemaVersion":"project-observation-contract.v1","contractId":"test.observations","observations":[{"path":"test.frame","type":"integer","description":"Committed project frame"}]}"#).unwrap();
    fs::write(project.join("Tests/default.json"), br#"{"schemaVersion":"playtest-scenario.v1","scenarioId":"delivery.test","initialSceneId":"scene-main","target":"windows-headless","maxSimulationTicks":3,"maxPresentationFrames":6,"timeoutMs":10000,"assertions":[{"assertionId":"frame","fromSimulationTick":3,"throughSimulationTick":3,"path":"test.frame","equals":3}]}"#).unwrap();
    let module_path = project.join("RuntimeModule/src/lib.rs");
    let module = fs::read_to_string(&module_path).unwrap();
    let original = "Ok(ProjectRuntimeObservationOutput { values })";
    assert_eq!(module.matches(original).count(), 1);
    let observation = r#"Ok(ProjectRuntimeObservationOutput {
        values: BTreeMap::from([(
            "test.frame".to_string(),
            ProjectRuntimeValue::Integer(_request.frame_index as i64),
        )]),
    })"#;
    fs::write(&module_path, module.replace(original, observation)).unwrap();

    let mut request = DesktopExportRequest::windows_dev(&project)
        .with_player_artifact_build_root(player_build.clone())
        .with_player_verification(true);
    request.frame_limit = 1;
    let mut session = project_authoring_execution::ProjectAuthoringSession::open(&project).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("desktop-export-snapshot", paths)
        .unwrap();
    let compiler = project_authoring_execution::GameProjectCompiler::bind(&lease).unwrap();
    let prepared = compiler
        .prepare(
            &lease,
            project_authoring_execution::TargetProfile::WindowsDev,
        )
        .unwrap();
    let original_rust = fs::read(project.join("RuntimeModule/src/lib.rs")).unwrap();
    let built = compiler
        .build(
            &prepared,
            project_authoring_execution::BuildRequest::for_project(
                project_authoring_execution::TargetProfile::WindowsDev,
                project.clone(),
                ProjectRelativePath::parse("Build/Windows").unwrap(),
                player_build.clone(),
            )
            .with_player_verification(false),
        )
        .unwrap();
    let report = built.desktop_export();
    assert_eq!(report.status, DesktopExportStatus::Success, "{report:#?}");
    let package = Path::new(&report.package_dir);
    assert!(package.join("Game.exe").is_file());
    assert!(package.join("engine_runtime.dll").is_file());
    assert!(package
        .join("data/bin/sample_complex_shooter_runtime.dll")
        .is_file());
    assert!(package.join("package-manifest.json").is_file());
    assert!(package.join("data/runtime_package/manifest.json").is_file());
    assert!(!package
        .join("reports/exported-player-process-verification-report.json")
        .is_file());
    assert_eq!(report.player_exit_code, None);
    assert_eq!(report.player_exit_reason, "not_started");
    assert!(!Path::new(&report.player_report_path).exists());
    assert!(!built
        .delivery()
        .evidence_refs()
        .contains(&report.player_report_path));
    assert!(built
        .delivery()
        .evidence_refs()
        .iter()
        .all(|path| Path::new(path).is_file()));
    let dev_manifest = runtime_cli::validate_desktop_dev_package(package).unwrap();
    assert_eq!(
        dev_manifest.player_artifact_hash,
        report.player_artifact_hash
    );
    assert_eq!(
        dev_manifest.engine_runtime_hash,
        Some(runtime_cli::semantic_file_digest(&package.join("engine_runtime.dll")).unwrap())
    );
    assert_eq!(
        dev_manifest.project_runtime_module_hash,
        Some(
            runtime_cli::semantic_file_digest(
                &package.join("data/bin/sample_complex_shooter_runtime.dll")
            )
            .unwrap()
        )
    );
    assert_eq!(
        dev_manifest.runtime_package_digest,
        Some(runtime_cli::runtime_package_digest(&package.join("data/runtime_package")).unwrap())
    );
    if let Some(evidence_root) = &evidence_root {
        retain_successful_delivery(package, &project, &source, evidence_root);
    }
    let first_build: project_authoring_execution::ProjectRuntimePlayerArtifactBuildReport =
        serde_json::from_slice(
            &fs::read(report.player_artifact_build_report_path.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
    assert!(first_build.cargo_rebuilt_artifacts > 0);
    assert_project_module_build_excludes_engine(&first_build);
    let original_artifact = first_build.artifact.as_ref().unwrap();
    assert_eq!(
        original_artifact.module_descriptor.module_id,
        first_build.expected_module.module_id
    );
    assert_eq!(
        original_artifact.module_descriptor.aot_content_digest,
        first_build.expected_module.aot_content_digest
    );
    assert!(first_build.engine_player_identity.is_some());

    // A fresh project has a distinct module identity but consumes the same
    // precompiled Host and Engine DLL without compiling either in its target.
    let second_project = project.parent().unwrap().join("second-project");
    copy_source_tree(&project, &second_project);
    let second_manifest_path = second_project.join("project.aife.json");
    let mut second_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&second_manifest_path).unwrap()).unwrap();
    second_manifest["projectId"] = "project-second-shooter-fixture".into();
    second_manifest["runtimeModule"]["moduleId"] = "sample.second-shooter.runtime".into();
    fs::write(
        &second_manifest_path,
        serde_json::to_vec(&second_manifest).unwrap(),
    )
    .unwrap();
    let mut second_session =
        project_authoring_execution::ProjectAuthoringSession::open(&second_project).unwrap();
    let second_paths = second_session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();
    let second_lease = second_session
        .acquire_snapshot_lease("desktop-export-second-project", second_paths)
        .unwrap();
    let second_compiler =
        project_authoring_execution::GameProjectCompiler::bind(&second_lease).unwrap();
    let second_prepared = second_compiler
        .prepare(
            &second_lease,
            project_authoring_execution::TargetProfile::WindowsDev,
        )
        .unwrap();
    let mut second_request = DesktopExportRequest::windows_dev(&second_project)
        .with_player_artifact_build_root(player_build.clone())
        .with_player_verification(true);
    second_request.frame_limit = 1;
    let second = DesktopExportPipeline::export_prepared(second_request, &second_prepared);
    assert_eq!(second.status, DesktopExportStatus::Success, "{second:#?}");
    assert_eq!(second.player_exit_code, Some(0));
    let second_build: project_authoring_execution::ProjectRuntimePlayerArtifactBuildReport =
        serde_json::from_slice(
            &fs::read(second.player_artifact_build_report_path.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
    assert_project_module_build_excludes_engine(&second_build);
    assert!(second_build.cargo_rebuilt_artifacts > 0);
    assert_ne!(
        first_build.compile_workspace,
        second_build.compile_workspace
    );
    assert_eq!(
        first_build.engine_player_identity,
        second_build.engine_player_identity
    );
    assert_eq!(
        second_build.engine_player_cache_status.as_deref(),
        Some("hit")
    );
    assert!(!second_build
        .steps
        .iter()
        .any(|step| step.stage == "build_engine_player"));
    let second_artifact = second_build.artifact.as_ref().unwrap();
    assert_eq!(
        second_artifact.module_descriptor.module_id,
        "sample.second-shooter.runtime"
    );
    assert_eq!(
        second_artifact.module_descriptor.aot_content_digest,
        second_build.expected_module.aot_content_digest
    );
    assert_ne!(
        original_artifact.module_descriptor,
        second_artifact.module_descriptor
    );
    let second_package = Path::new(&second.package_dir);
    let second_dev_manifest = runtime_cli::validate_desktop_dev_package(second_package).unwrap();
    assert_ne!(
        dev_manifest.project_runtime_module_hash,
        second_dev_manifest.project_runtime_module_hash
    );
    for binary in ["Game.exe", "engine_runtime.dll"] {
        assert_eq!(
            runtime_cli::semantic_file_digest(&package.join(binary)).unwrap(),
            runtime_cli::semantic_file_digest(&second_package.join(binary)).unwrap(),
            "Both projects must reuse the same {binary} bytes"
        );
    }
    // The retained package and reports must precede every intentional failure below.
    fs::write(
        project.join("RuntimeModule/src/lib.rs"),
        "this live source must not compile",
    )
    .unwrap();
    compiler
        .verify(
            built.delivery(),
            project_authoring_execution::VerifyRequest::new(
                project_authoring_execution::ExecutionMode::Headless,
                1,
                30_000,
                false,
            ),
        )
        .unwrap();
    assert!(Path::new(&report.player_report_path).is_file());
    let scenario = prepared.load_playtest_scenario().unwrap();
    fs::write(project.join("Tests/default.json"), "invalid live scenario").unwrap();
    let semantic = compiler
        .playtest_delivery(
            built.delivery(),
            &scenario,
            project.parent().unwrap().join("semantic-run"),
        )
        .unwrap();
    assert_eq!(
        semantic.process().overall,
        runtime_player_winit::semantic_outcome::OutcomeStatus::Passed,
        "{semantic:#?}"
    );
    assert_eq!(
        semantic.process().outcome.delivery,
        runtime_player_winit::semantic_outcome::OutcomeStatus::Passed
    );
    let observed = compiler.observe_playtest(&semantic).unwrap();
    assert_eq!(observed.run_id, semantic.process().run_id);
    assert_eq!(
        observed
            .player
            .as_ref()
            .unwrap()
            .semantic_playtest
            .as_ref()
            .unwrap()
            .assertions[0]
            .actual,
        Some(engine_runtime::project_observation::ProjectObservationValue::Integer(3))
    );
    // A replacement DLL with a matching new manifest is internally consistent,
    // but it is not the delivery that this Compiler originally returned.
    let engine_dll = package.join("engine_runtime.dll");
    let original_engine_dll = project
        .parent()
        .unwrap()
        .join("original-engine-runtime.dll");
    let package_manifest_path = package.join("package-manifest.json");
    let original_manifest = fs::read(&package_manifest_path).unwrap();
    fs::rename(&engine_dll, &original_engine_dll).unwrap();
    fs::write(&engine_dll, b"different Engine DLL; must never be loaded").unwrap();
    let mut replacement_manifest = dev_manifest;
    replacement_manifest.engine_runtime_hash =
        Some(runtime_cli::semantic_file_digest(&engine_dll).unwrap());
    fs::write(
        &package_manifest_path,
        serde_json::to_vec_pretty(&replacement_manifest).unwrap(),
    )
    .unwrap();
    runtime_cli::validate_desktop_dev_package(package).unwrap();
    assert!(compiler
        .verify(
            built.delivery(),
            project_authoring_execution::VerifyRequest::new(
                project_authoring_execution::ExecutionMode::Headless,
                1,
                30_000,
                false,
            ),
        )
        .unwrap_err()
        .code()
        .contains("delivery_changed"));
    assert!(compiler
        .playtest_delivery(built.delivery(), &scenario, project.join("must-not-run"))
        .unwrap_err()
        .code()
        .contains("delivery_changed"));
    assert!(!project.join("must-not-run").exists());
    fs::remove_file(&engine_dll).unwrap();
    fs::rename(&original_engine_dll, &engine_dll).unwrap();
    fs::write(&package_manifest_path, original_manifest).unwrap();
    fs::write(semantic.evidence().report_path.clone(), b"{}").unwrap();
    assert!(compiler
        .observe_playtest(&semantic)
        .unwrap_err()
        .message()
        .contains("digest_mismatch"));
    let payload_file = package.join("data/runtime_package/scenes/scene-main.json");
    let original_payload = fs::read(&payload_file).unwrap();
    fs::write(&payload_file, b"{}").unwrap();
    assert!(compiler
        .playtest_delivery(built.delivery(), &scenario, project.join("must-not-run"))
        .unwrap_err()
        .code()
        .contains("delivery_changed"));
    assert!(!project.join("must-not-run").exists());
    fs::write(&payload_file, original_payload).unwrap();
    let player_file = package.join("Game.exe");
    let original_player = fs::read(&player_file).unwrap();
    fs::write(&player_file, b"not the specified player").unwrap();
    assert!(compiler
        .playtest_delivery(built.delivery(), &scenario, project.join("must-not-run"))
        .unwrap_err()
        .code()
        .contains("delivery_changed"));
    fs::write(&player_file, original_player).unwrap();
    fs::write(
        project.join("Tests/default.json"),
        serde_json::to_vec(scenario.scenario()).unwrap(),
    )
    .unwrap();
    let repeated = DesktopExportPipeline::export_prepared(request.clone(), &prepared);
    assert_eq!(
        repeated.status,
        DesktopExportStatus::Success,
        "{repeated:#?}"
    );
    let repeated_build: project_authoring_execution::ProjectRuntimePlayerArtifactBuildReport =
        serde_json::from_slice(
            &fs::read(repeated.player_artifact_build_report_path.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
    assert_eq!(repeated_build.cache_status, "hit");
    assert_eq!(repeated_build.cargo_rebuilt_artifacts, 0);
    let mut changed_rust = original_rust;
    changed_rust.extend_from_slice(b"\n// Incremental source revision.\n");
    fs::write(project.join("RuntimeModule/src/lib.rs"), changed_rust).unwrap();
    session.refresh().unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();
    let new_lease = session
        .acquire_snapshot_lease("desktop-export-changed", paths)
        .unwrap();
    let new_compiler = project_authoring_execution::GameProjectCompiler::bind(&new_lease).unwrap();
    let changed_prepared = new_compiler
        .prepare(
            &new_lease,
            project_authoring_execution::TargetProfile::WindowsDev,
        )
        .unwrap();
    assert!(new_compiler
        .playtest_delivery(built.delivery(), &scenario, project.join("must-not-run"))
        .unwrap_err()
        .code()
        .contains("operation_binding_mismatch"));
    assert!(compiler
        .playtest_delivery(
            built.delivery(),
            &changed_prepared.load_playtest_scenario().unwrap(),
            project.join("must-not-run")
        )
        .unwrap_err()
        .code()
        .contains("scenario_delivery_mismatch"));
    let changed = DesktopExportPipeline::export_prepared(request.clone(), &changed_prepared);
    assert_eq!(changed.status, DesktopExportStatus::Success, "{changed:#?}");
    let changed_build: project_authoring_execution::ProjectRuntimePlayerArtifactBuildReport =
        serde_json::from_slice(
            &fs::read(changed.player_artifact_build_report_path.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
    assert_eq!(
        first_build.compile_workspace,
        changed_build.compile_workspace
    );
    assert_ne!(first_build.artifact_root, changed_build.artifact_root);
    assert!(changed_build.cargo_fresh_artifacts > 0);
    assert!(changed_build.cargo_rebuilt_artifacts > 0);
    assert!(changed_build.cargo_rebuilt_artifacts < first_build.cargo_rebuilt_artifacts);
    assert_eq!(
        first_build.engine_player_identity,
        changed_build.engine_player_identity
    );
    assert_eq!(
        changed_build.engine_player_cache_status.as_deref(),
        Some("hit")
    );
    assert_project_module_build_excludes_engine(&changed_build);
    assert_eq!(
        engine_runtime::canonical_digest::sha256_prefixed(
            &fs::read(&original_artifact.executable_path).unwrap()
        ),
        original_artifact.source_executable_hash
    );

    // A directory at the executable destination deterministically prevents copy.
    // Disabling process verification must not turn that failure into success.
    let destination = Path::new(&changed.package_dir).join("Game.exe");
    fs::remove_file(&destination).unwrap();
    fs::create_dir(&destination).unwrap();
    let failed = DesktopExportPipeline::export_prepared(
        request.with_player_verification(false),
        &changed_prepared,
    );
    assert_eq!(failed.status, DesktopExportStatus::Failed, "{failed:#?}");
    assert_eq!(failed.player_exit_reason, "not_started");
    assert!(failed
        .diagnostics
        .iter()
        .any(|d| d.code == "PlayerExecutableCopyFailed"));
}

fn assert_project_module_build_excludes_engine(
    report: &project_authoring_execution::ProjectRuntimePlayerArtifactBuildReport,
) {
    let step = report
        .steps
        .iter()
        .find(|step| step.stage == "build_project_runtime_module")
        .expect("Project Cargo compilation must have its own reported step");
    let artifacts: Vec<serde_json::Value> = step
        .process
        .stdout_summary
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|message: &serde_json::Value| message["reason"] == "compiler-artifact")
        .collect();
    for expected in [
        "complex_shooter_project_runtime",
        "aife_generated_runtime_glue",
    ] {
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact["target"]["name"] == expected),
            "Project build did not report {expected}: {artifacts:#?}"
        );
    }
    for artifact in &artifacts {
        let target = artifact["target"]["name"].as_str().unwrap();
        assert!(
            !matches!(
                target,
                "ai_project_runtime_player"
                    | "runtime_cli"
                    | "engine_runtime"
                    | "engine_runtime_host"
                    | "runtime_player_winit"
                    | "winit"
            ) && !target.starts_with("wgpu")
                && !target.contains("renderer"),
            "Project Cargo graph still contains engine/Host target {target}: {artifact:#?}"
        );
    }
}

#[test]
fn desktop_export_project_relative_output_keeps_project_authority() {
    let project = unique_temp_dir("desktop-export-relative");
    let relative =
        ProjectRelativePath::parse("Library/AiCapability/Deliveries/operation-1/Windows").unwrap();
    let request =
        DesktopExportRequest::windows_dev(&project).with_project_relative_output(relative.clone());

    assert_eq!(
        request.package_dir(),
        project.join(relative.as_path()).join("dev")
    );
}

#[cfg(windows)]
#[test]
fn desktop_export_project_relative_output_rejects_delivery_junction() {
    let project = unique_temp_dir("desktop-export-junction");
    let outside = unique_temp_dir("desktop-export-junction-outside");
    fs::create_dir_all(project.join("Library/AiCapability")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let delivery_root = project.join("Library/AiCapability/Deliveries");
    create_directory_junction(&outside, &delivery_root);
    let relative =
        ProjectRelativePath::parse("Library/AiCapability/Deliveries/operation-1/Windows").unwrap();

    let report = DesktopExportPipeline::export(
        DesktopExportRequest::windows_dev(&project).with_project_relative_output(relative),
    );
    let escaped = outside.join("operation-1").exists();

    fs::remove_dir(&delivery_root).unwrap();
    let _ = fs::remove_dir_all(&project);
    let _ = fs::remove_dir_all(&outside);
    assert_eq!(report.status, DesktopExportStatus::Failed);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.starts_with("project_write.")));
    assert!(!escaped);
}

fn copy_source_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some("Build" | "Library" | "target" | ".aife" | ".git")
        ) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(name);
        if entry.file_type().unwrap().is_dir() {
            copy_source_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).unwrap();
        }
    }
}

fn configured_evidence_root() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("AIFE_DESKTOP_EXPORT_EVIDENCE_ROOT")?);
    assert!(
        root.is_absolute(),
        "Evidence root must be absolute: {root:?}"
    );
    assert!(!root.exists(), "Evidence root must be fresh: {root:?}");
    Some(root)
}

fn retain_successful_delivery(
    package: &Path,
    fixture_project: &Path,
    source_project: &Path,
    evidence_root: &Path,
) {
    fs::create_dir_all(evidence_root.parent().unwrap()).unwrap();
    fs::create_dir(evidence_root).expect("create fresh evidence root without merging prior output");
    let retained_package = evidence_root.join("package");
    fs::create_dir(&retained_package).unwrap();
    let original_inventory = file_inventory(package);
    for relative in original_inventory.keys() {
        let destination = retained_package.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(package.join(relative), &destination).unwrap();
    }
    assert_eq!(file_inventory(&retained_package), original_inventory);
    runtime_cli::validate_desktop_dev_package(&retained_package).unwrap();

    let fixture_files = [
        "AUI/hud.aui.json",
        "project.aife.json",
        "Tests/observations.json",
        "Tests/default.json",
        "RuntimeModule/src/lib.rs",
    ];
    let mut fixture_inputs = BTreeMap::new();
    for relative in fixture_files {
        let source = fixture_project.join(relative);
        let destination = evidence_root.join("fixture-inputs").join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(&source, &destination).unwrap();
        fixture_inputs.insert(
            relative,
            runtime_cli::semantic_file_digest(&source).unwrap(),
        );
    }
    let verification_dir = evidence_root.join("verification");
    fs::create_dir(&verification_dir).unwrap();
    let verification = runtime_cli::verify_exported_player_process(
        runtime_cli::ExportedPlayerProcessVerificationRequest {
            exported_package_dir: retained_package.clone(),
            mode: "headless-gate".to_string(),
            frame_limit: 1,
            report_path: Some(
                verification_dir.join("exported-player-process-verification-report.json"),
            ),
            timeout_ms: 30_000,
            screenshot: false,
            screenshot_path: None,
        },
    );
    let verified_inventory = file_inventory(&retained_package);
    let execution_owner = fs::read(&verification.child_report_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|report| {
            report
                .get("diagnostics")
                .and_then(|value| value.as_array())
                .cloned()
        })
        .is_some_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                diagnostic.get("code").and_then(|value| value.as_str())
                    == Some("native_host.engine_runtime.execution_owner")
            })
        });
    let test_source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/desktop_export_owner.rs");
    let evidence = serde_json::json!({
        "status": if verification.status == runtime_cli::ExportedPlayerProcessVerificationStatus::Passed
            && verified_inventory == original_inventory && execution_owner { "passed" } else { "failed" },
        "claim": "single successful Compiler/DesktopExport fixture delivery; complete byte-preserving copy",
        "sourceProject": source_project,
        "fixtureProject": fixture_project,
        "fixtureScope": "complex_shooter_project test fixture, not unchanged project gameplay qualification",
        "fixtureEdits": [
            "AUI/hud.aui.json documentId normalized for owner fixture",
            "project.aife.json points at fixture playtest and observation contracts",
            "Tests/default.json and observations.json check test.frame=3",
            "RuntimeModule observation producer returns test.frame"
        ],
        "fixtureInputDigests": fixture_inputs,
        "testSource": test_source,
        "testSourceDigest": runtime_cli::semantic_file_digest(&test_source).unwrap(),
        "originalPackage": package,
        "retainedPackage": retained_package,
        "capturedBeforeIntentionalMutations": true,
        "originalReportsPreservedWithoutRewriting": true,
        "copyMethod": "fresh directory; every original relative file copied once; no source exclusions",
        "originalFileInventory": original_inventory,
        "verifiedCopyFileInventory": verified_inventory,
        "runtimeExecutionOwnerIsEngineDll": execution_owner,
        "verification": verification
    });
    fs::write(
        evidence_root.join("delivery-consistency-evidence.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    assert_eq!(evidence["status"], "passed", "{evidence:#}");
    assert_eq!(verification.child_frames_completed, Some(1));
    assert!(verification.process_id.is_some());
}

fn file_inventory(root: &Path) -> BTreeMap<String, String> {
    let mut inventory = BTreeMap::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "Unexpected link: {path:?}"
            );
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                assert_eq!(
                    metadata.file_attributes() & 0x400,
                    0,
                    "Reparse point: {path:?}"
                );
            }
            if metadata.is_dir() {
                directories.push(path);
            } else {
                assert!(metadata.is_file(), "Unexpected file type: {path:?}");
                inventory.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    runtime_cli::semantic_file_digest(&path).unwrap(),
                );
            }
        }
    }
    inventory
}

#[cfg(windows)]
fn create_directory_junction(target: &Path, link: &Path) {
    let link = link.to_string_lossy().replace('/', "\\");
    let target = target.to_string_lossy().replace('/', "\\");
    let output = std::process::Command::new("cmd")
        .args(["/D", "/C", "mklink", "/J", &link, &target])
        .output()
        .expect("launch mklink /J");
    assert!(
        output.status.success(),
        "mklink /J failed: link={link}; target={target}; stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
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
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("aife-{label}-{}-{nonce}", std::process::id()))
}

struct TestDirectoryGuard(PathBuf);

impl Drop for TestDirectoryGuard {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "failed desktop export fixture retained: {}",
                self.0.display()
            );
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
