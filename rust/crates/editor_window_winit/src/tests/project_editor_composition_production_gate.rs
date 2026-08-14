use super::*;
use editor_core::{
    EditorCompositionProcessAdapter, GeneratedCompositionLockLineage,
    ProjectEditorCompositionArtifact, ProjectEditorCompositionDescriptor,
    ProjectEditorCompositionLaunchStatus, ProjectEditorCompositionResolvedIdentity,
    ProjectRuntimeTrustDecisionKind, ProjectRuntimeTrustInspection, ProjectRuntimeTrustModule,
    ProjectRuntimeTrustStatus, GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::{
    LinkedProjectRuntimeSet, ProjectRuntimeError, ProjectRuntimeModule,
    ProjectRuntimeModuleDescriptor, ProjectRuntimeRegistration,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RUN_ROOT_ENV: &str = "AIFE_262_WINDOW4_RUN_ROOT";
const HANDOFF_EVIDENCE_ROOT_ENV: &str = "AIFE_262_HANDOFF_EVIDENCE_ROOT";

fn resolved_identity(
    identity: &editor_core::ProjectEditorCompositionIdentity,
) -> ProjectEditorCompositionResolvedIdentity {
    ProjectEditorCompositionResolvedIdentity::new(
        identity.digest().unwrap(),
        &GeneratedCompositionLockLineage {
            schema_version: GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION.to_string(),
            lock_input_digest: format!("sha256:{}", "1".repeat(64)),
            raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
            resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
        },
    )
    .unwrap()
}

struct IdentityOnlyRuntime {
    descriptor: ProjectRuntimeModuleDescriptor,
}

impl ProjectRuntimeModule for IdentityOnlyRuntime {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(&self, _: &mut ProjectRuntimeRegistration) -> Result<(), ProjectRuntimeError> {
        Ok(())
    }
}

#[test]
#[ignore = "launches a real generated Editor candidate using run-owned Window 4 state"]
fn project_editor_composition_production_handoff_real_process_gate() {
    let run_root = absolute_env_path(RUN_ROOT_ENV);
    let evidence_root = absolute_env_path(HANDOFF_EVIDENCE_ROOT_ENV);
    assert!(evidence_root.starts_with(&run_root) && evidence_root != run_root);
    fs::create_dir_all(&evidence_root).unwrap();
    let launch_root = absolute_env_path(crate::PROJECT_EDITOR_HANDOFF_ISOLATED_LAUNCH_ROOT_ENV);
    assert!(launch_root.starts_with(&evidence_root) && launch_root != evidence_root);
    fs::create_dir_all(launch_root.join("picker-start")).unwrap();
    assert!(!launch_root.join("state").exists());
    let production_state_root = crate::project_editor_composition_state_root().unwrap();
    assert!(production_state_root.starts_with(&evidence_root));

    let source_project = run_root.join("projects/tower-defense-candidate");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let project_root = run_root
        .join("projects")
        .join(format!("tower-defense-handoff-{stamp}"));
    copy_project_tree(&source_project, &project_root);
    let sdk_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let editor_build_identity = crate::current_editor_build_identity().unwrap();
    let trust =
        ProjectRuntimeTrustModule::open(production_state_root.join("project-runtime-trust"))
            .unwrap();

    let baseline = ProjectRuntimeTrustInspection::inspect(
        &project_root,
        &sdk_root,
        editor_build_identity.clone(),
    )
    .unwrap();
    trust
        .record_explicit(
            &baseline.request,
            ProjectRuntimeTrustDecisionKind::Trusted,
            stamp as u64,
        )
        .unwrap();
    let baseline_identity = crate::project_editor_composition_production::composition_identity(
        &project_root,
        &sdk_root,
        &baseline,
        &editor_build_identity,
    )
    .unwrap();

    let data_path = project_root.join("AUI/battle-hud.aui.json");
    let mut data_bytes = fs::read(&data_path).unwrap();
    data_bytes.extend_from_slice(b"\n");
    fs::write(&data_path, data_bytes).unwrap();
    let data_changed = ProjectRuntimeTrustInspection::inspect(
        &project_root,
        &sdk_root,
        editor_build_identity.clone(),
    )
    .unwrap();
    assert_eq!(data_changed.request, baseline.request);
    assert_eq!(
        trust.evaluate(&data_changed.request, None).unwrap().status,
        ProjectRuntimeTrustStatus::Trusted
    );
    let data_identity = crate::project_editor_composition_production::composition_identity(
        &project_root,
        &sdk_root,
        &data_changed,
        &editor_build_identity,
    )
    .unwrap();
    assert_eq!(
        data_identity.digest().unwrap(),
        baseline_identity.digest().unwrap()
    );
    assert_data_only_open_does_not_handoff(
        &project_root,
        &sdk_root,
        &editor_build_identity,
        trust.clone(),
        baseline_identity.clone(),
    );

    let runtime_source = project_root.join("RuntimeModule/src/lib.rs");
    let mut runtime_bytes = fs::read(&runtime_source).unwrap();
    runtime_bytes.extend_from_slice(b"\n// 262 Window 4 production handoff qualification\n");
    fs::write(&runtime_source, runtime_bytes).unwrap();
    let changed = ProjectRuntimeTrustInspection::inspect(
        &project_root,
        &sdk_root,
        editor_build_identity.clone(),
    )
    .unwrap();
    assert_ne!(
        changed.request.runtime_module_source_digest,
        baseline.request.runtime_module_source_digest
    );
    assert_eq!(
        trust.evaluate(&changed.request, None).unwrap().status,
        ProjectRuntimeTrustStatus::Stale
    );

    let process = Arc::new(crate::NativeEditorCompositionProcessAdapter::default());
    let graceful_exit = Arc::new(AtomicBool::new(false));
    let build_root = production_state_root.clone();
    let ticket_root = production_state_root.join("project-editor-handoff");
    fs::create_dir_all(&build_root).unwrap();
    fs::create_dir_all(&ticket_root).unwrap();
    let session = EditorSession::with_linked_project_runtimes(
        crate::default_editor_linked_project_runtimes(),
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session)
            .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
                trust_module: trust.clone(),
                engine_sdk_root: sdk_root.clone(),
                editor_build_identity: editor_build_identity.clone(),
            });
    app.install_project_editor_composition_preparer(
        Arc::new(crate::NativeProjectEditorCompositionPreparer::new(
            sdk_root.clone(),
            build_root,
            editor_build_identity.clone(),
        )),
        ticket_root.clone(),
    );
    app.install_project_editor_composition_launcher(
        editor_core::EditorProjectCompositionLauncher::new(
            Arc::new(crate::NativeEditorCompositionClock),
            Arc::new(crate::NativeEditorCompositionWorkspaceAdapter::new(
                ticket_root.join("workspace-state.json"),
            )),
            process.clone(),
            Arc::new(crate::NativeEditorCompositionExitAdapter::new(
                graceful_exit.clone(),
            )),
        ),
    );

    assert!(app
        .dispatch_project_launcher_command_or_dispatch(open_command(&project_root))
        .is_none());
    let request_id = app
        .latest_model()
        .project_runtime_trust_prompt
        .as_ref()
        .expect("AOT change must prompt stale trust")
        .request_id
        .clone();
    let approval = app.dispatch_command(UiCommand {
        command_id: "approve_project_runtime_trust".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "262-production-handoff-approve".to_string(),
        payload: UiCommandPayload::ApproveProjectRuntimeTrust { request_id },
    });
    assert_eq!(approval.status, CommandStatus::Committed);

    let ready = wait_for_ready_receipt(&mut app, Duration::from_secs(720));
    assert!(graceful_exit.load(Ordering::Acquire));
    let candidate_pid = ready
        .candidate_process_id
        .expect("ready receipt candidate pid");
    process.terminate_owned_candidate(candidate_pid).unwrap();

    let failed_exit = Arc::new(AtomicBool::new(false));
    let failed_receipt = qualify_failed_candidate_keeps_old_editor(
        &project_root,
        &ticket_root,
        baseline_identity,
        Arc::new(crate::NativeEditorCompositionProcessAdapter::default()),
        failed_exit.clone(),
    );
    assert!(!failed_exit.load(Ordering::Acquire));

    let report = json!({
        "schemaVersion": "project-editor-composition-production-handoff-report.v1",
        "status": "passed",
        "projectRoot": project_root,
        "editorBuildIdentity": editor_build_identity,
        "dataOnlyTrustStatus": "trusted",
        "dataOnlyHandoff": false,
        "aotTrustStatusBeforeApproval": "stale",
        "readyCompositionIdentity": ready.composition_identity_digest,
        "candidateProcessId": candidate_pid,
        "oldEditorGracefulExitRequested": true,
        "candidateTerminatedAndJoined": true,
        "failedCandidateStatus": format!("{:?}", failed_receipt.status).to_lowercase(),
        "failedCandidateKeptOldEditor": true,
    });
    fs::write(
        evidence_root.join("production-handoff-report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string(&report).unwrap());
}

#[test]
#[ignore = "runs the 282 C1 run-owned cold composition build and real candidate handoff"]
fn project_editor_composition_282_c1_real_process_gate() {
    let run_root = absolute_env_path(RUN_ROOT_ENV);
    let evidence_root = absolute_env_path(HANDOFF_EVIDENCE_ROOT_ENV);
    assert!(evidence_root.starts_with(&run_root) && evidence_root != run_root);
    fs::create_dir_all(&evidence_root).unwrap();
    let launch_root = absolute_env_path(crate::PROJECT_EDITOR_HANDOFF_ISOLATED_LAUNCH_ROOT_ENV);
    assert!(launch_root.starts_with(&evidence_root) && launch_root != evidence_root);
    fs::create_dir_all(launch_root.join("picker-start")).unwrap();
    assert!(!launch_root.join("state").exists());
    let production_state_root = crate::project_editor_composition_state_root().unwrap();
    assert!(production_state_root.starts_with(&evidence_root));

    let source_project = run_root.join("projects/tower-defense-candidate");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let project_root = run_root
        .join("projects")
        .join(format!("tower-defense-282-c1-{stamp}"));
    copy_project_tree(&source_project, &project_root);
    let sdk_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let editor_build_identity = crate::current_editor_build_identity().unwrap();
    let trust =
        ProjectRuntimeTrustModule::open(production_state_root.join("project-runtime-trust"))
            .unwrap();
    let baseline = ProjectRuntimeTrustInspection::inspect(
        &project_root,
        &sdk_root,
        editor_build_identity.clone(),
    )
    .unwrap();
    trust
        .record_explicit(
            &baseline.request,
            ProjectRuntimeTrustDecisionKind::Trusted,
            stamp as u64,
        )
        .unwrap();

    let runtime_source = project_root.join("RuntimeModule/src/lib.rs");
    let mut runtime_bytes = fs::read(&runtime_source).unwrap();
    runtime_bytes.extend_from_slice(b"\n// 282 C1 cold-build qualification\n");
    fs::write(&runtime_source, runtime_bytes).unwrap();
    let changed = ProjectRuntimeTrustInspection::inspect(
        &project_root,
        &sdk_root,
        editor_build_identity.clone(),
    )
    .unwrap();
    assert_eq!(
        trust.evaluate(&changed.request, None).unwrap().status,
        ProjectRuntimeTrustStatus::Stale
    );

    let process = Arc::new(crate::NativeEditorCompositionProcessAdapter::default());
    let graceful_exit = Arc::new(AtomicBool::new(false));
    let ticket_root = production_state_root.join("project-editor-handoff");
    fs::create_dir_all(&production_state_root).unwrap();
    fs::create_dir_all(&ticket_root).unwrap();
    let session = EditorSession::with_linked_project_runtimes(
        crate::default_editor_linked_project_runtimes(),
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session)
            .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
                trust_module: trust,
                engine_sdk_root: sdk_root.clone(),
                editor_build_identity: editor_build_identity.clone(),
            });
    app.install_project_editor_composition_preparer(
        Arc::new(crate::NativeProjectEditorCompositionPreparer::new(
            sdk_root,
            production_state_root,
            editor_build_identity.clone(),
        )),
        ticket_root.clone(),
    );
    app.install_project_editor_composition_launcher(
        editor_core::EditorProjectCompositionLauncher::new(
            Arc::new(crate::NativeEditorCompositionClock),
            Arc::new(crate::NativeEditorCompositionWorkspaceAdapter::new(
                ticket_root.join("workspace-state.json"),
            )),
            process.clone(),
            Arc::new(crate::NativeEditorCompositionExitAdapter::new(
                graceful_exit.clone(),
            )),
        ),
    );

    assert!(app
        .dispatch_project_launcher_command_or_dispatch(open_command(&project_root))
        .is_none());
    let request_id = app
        .latest_model()
        .project_runtime_trust_prompt
        .as_ref()
        .expect("changed AOT must prompt for trust")
        .request_id
        .clone();
    let approval = app.dispatch_command(UiCommand {
        command_id: "approve_project_runtime_trust".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "282-c1-approve".to_string(),
        payload: UiCommandPayload::ApproveProjectRuntimeTrust { request_id },
    });
    assert_eq!(approval.status, CommandStatus::Committed);

    if std::env::var_os("AIFE_282_CANCEL_AFTER_COMPILING").is_some() {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            app.frame(1280.0, 720.0);
            let compiling = app
                .latest_model()
                .project_launcher
                .activity
                .as_ref()
                .is_some_and(|activity| {
                    activity.phase == editor_ui_model::ProjectOpenActivityPhase::Compiling
                });
            if compiling {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "282 cancellation qualification did not reach Compiling"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let cancel_started = Instant::now();
        drop(app);
        let cancel_join_elapsed_ms = cancel_started.elapsed().as_millis() as u64;
        assert!(cancel_join_elapsed_ms < 30_000);
        let report = json!({
            "schemaVersion": "project-editor-composition-282-c1-cancel-report.v1",
            "status": "passed",
            "projectRoot": project_root,
            "editorBuildIdentity": editor_build_identity,
            "cancelIssuedAtPhase": "compiling",
            "applicationWorkerDroppedAndJoined": true,
            "cancelJoinElapsedMs": cancel_join_elapsed_ms
        });
        fs::write(
            evidence_root.join("282-c1-cancel-report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("{}", serde_json::to_string(&report).unwrap());
        return;
    }

    let ready = wait_for_ready_receipt(&mut app, Duration::from_secs(720));
    assert!(graceful_exit.load(Ordering::Acquire));
    let candidate_pid = ready
        .candidate_process_id
        .expect("ready receipt candidate pid");
    process.terminate_owned_candidate(candidate_pid).unwrap();
    drop(app);

    let report = json!({
        "schemaVersion": "project-editor-composition-282-c1-real-process-report.v1",
        "status": "passed",
        "projectRoot": project_root,
        "editorBuildIdentity": editor_build_identity,
        "readyCompositionIdentity": ready.composition_identity_digest,
        "candidateProcessId": candidate_pid,
        "oldEditorGracefulExitRequested": true,
        "candidateTerminatedAndJoined": true,
        "applicationWorkerDroppedAndJoined": true
    });
    fs::write(
        evidence_root.join("282-c1-real-process-report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string(&report).unwrap());
}

fn assert_data_only_open_does_not_handoff(
    project_root: &Path,
    sdk_root: &Path,
    editor_build_identity: &str,
    trust: ProjectRuntimeTrustModule,
    identity: editor_core::ProjectEditorCompositionIdentity,
) {
    let linked = LinkedProjectRuntimeSet::singleton(Arc::new(IdentityOnlyRuntime {
        descriptor: ProjectRuntimeModuleDescriptor::new(
            identity.module_id.clone(),
            identity.aot_content_digest.clone(),
        ),
    }))
    .unwrap();
    let session =
        EditorSession::with_project_editor_composition(Arc::new(linked), identity).unwrap();
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session)
            .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
                trust_module: trust,
                engine_sdk_root: sdk_root.to_path_buf(),
                editor_build_identity: editor_build_identity.to_string(),
            });
    let result = app
        .dispatch_project_launcher_command_or_dispatch(open_command(project_root))
        .expect("trusted data-only open must continue");
    assert_eq!(result.status, CommandStatus::Committed);
    app.frame(1280.0, 720.0);
    assert!(app.latest_model().project_runtime_trust_prompt.is_none());
    assert!(app
        .last_project_editor_composition_launch_receipt()
        .is_none());
}

fn wait_for_ready_receipt(
    app: &mut NativeEditorApplication,
    timeout: Duration,
) -> editor_core::ProjectEditorCompositionLaunchReceipt {
    let deadline = Instant::now() + timeout;
    loop {
        app.frame(1280.0, 720.0);
        if let Some(receipt) = app
            .last_project_editor_composition_launch_receipt()
            .cloned()
        {
            match receipt.status {
                ProjectEditorCompositionLaunchStatus::Ready => return receipt,
                ProjectEditorCompositionLaunchStatus::Failed
                | ProjectEditorCompositionLaunchStatus::TimedOut
                | ProjectEditorCompositionLaunchStatus::Cancelled => {
                    panic!("production handoff failed: {receipt:#?}")
                }
                ProjectEditorCompositionLaunchStatus::Pending => {}
            }
        }
        assert!(
            Instant::now() < deadline,
            "production handoff qualification timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn qualify_failed_candidate_keeps_old_editor(
    project_root: &Path,
    ticket_root: &Path,
    identity: editor_core::ProjectEditorCompositionIdentity,
    process: Arc<crate::NativeEditorCompositionProcessAdapter>,
    exit: Arc<AtomicBool>,
) -> editor_core::ProjectEditorCompositionLaunchReceipt {
    let executable = PathBuf::from(r"C:\Windows\System32\where.exe");
    let executable_hash = sha256_prefixed(&fs::read(&executable).unwrap());
    let identity_digest = identity.digest().unwrap();
    let resolved_identity = resolved_identity(&identity);
    let artifact = ProjectEditorCompositionArtifact {
        schema_version: PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION.to_string(),
        executable_path: executable,
        descriptor_path: ticket_root.join("failed-candidate-descriptor.json"),
        build_report_path: ticket_root.join("failed-candidate-build-report.json"),
        descriptor: ProjectEditorCompositionDescriptor {
            schema_version: PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity,
            identity_digest,
            resolved_identity,
            executable_hash,
            created_at: 1,
        },
    };
    let failed_ticket_root = ticket_root.join("failed-candidate");
    fs::create_dir_all(&failed_ticket_root).unwrap();
    let mut launcher = editor_core::EditorProjectCompositionLauncher::new(
        Arc::new(crate::NativeEditorCompositionClock),
        Arc::new(crate::NativeEditorCompositionWorkspaceAdapter::new(
            failed_ticket_root.join("workspace-state.json"),
        )),
        process,
        Arc::new(crate::NativeEditorCompositionExitAdapter::new(exit)),
    );
    let pending = launcher.handoff(editor_core::EditorCompositionHandoffRequest {
        old_editor_instance_id: "262-failed-candidate-old-editor".to_string(),
        running_identity_digest: None,
        artifact,
        project_root: project_root.to_path_buf(),
        project_id: "project-4966952341520437268".to_string(),
        ticket_root: failed_ticket_root,
        timeout_ms: 10_000,
    });
    assert_eq!(
        pending.status,
        ProjectEditorCompositionLaunchStatus::Pending
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(receipt) = launcher.poll() {
            assert_eq!(receipt.status, ProjectEditorCompositionLaunchStatus::Failed);
            return receipt;
        }
        assert!(Instant::now() < deadline, "failed candidate did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn open_command(path: &Path) -> UiCommand {
    UiCommand {
        command_id: "open_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "262-production-handoff-open".to_string(),
        payload: UiCommandPayload::OpenProject {
            path: path.display().to_string(),
        },
    }
}

fn absolute_env_path(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| panic!("{name} must be set"));
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{name} must be absolute");
    path
}

fn copy_project_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    let mut entries = fs::read_dir(source)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        if metadata.is_dir() && matches!(name.as_str(), ".aife" | "target" | "tests") {
            continue;
        }
        let target = destination.join(path.file_name().unwrap());
        if metadata.is_dir() {
            copy_project_tree(&path, &target);
        } else if metadata.is_file() {
            fs::copy(path, target).unwrap();
        }
    }
}
