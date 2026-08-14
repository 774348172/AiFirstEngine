use super::*;
use editor_core::{ProjectRuntimeTrustInspection, ProjectRuntimeTrustStatus};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempRoot(std::path::PathBuf);
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_root() -> TempRoot {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("aife-262-trust-ui-{stamp}"));
    fs::create_dir_all(&root).unwrap();
    TempRoot(root)
}

fn resolved_identity(
    identity: &editor_core::ProjectEditorCompositionIdentity,
) -> editor_core::ProjectEditorCompositionResolvedIdentity {
    editor_core::ProjectEditorCompositionResolvedIdentity::new(
        identity.digest().unwrap(),
        &editor_core::GeneratedCompositionLockLineage {
            schema_version: editor_core::GENERATED_COMPOSITION_LOCK_LINEAGE_SCHEMA_VERSION
                .to_string(),
            lock_input_digest: format!("sha256:{}", "1".repeat(64)),
            raw_lock_digest: format!("sha256:{}", "2".repeat(64)),
            resolved_graph_digest: format!("sha256:{}", "3".repeat(64)),
        },
    )
    .unwrap()
}

fn project_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/tower_defense_project")
}

fn engine_sdk_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn open_command(path: &std::path::Path) -> UiCommand {
    UiCommand {
        command_id: "open_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "trust-open".to_string(),
        payload: UiCommandPayload::OpenProject {
            path: path.display().to_string(),
        },
    }
}

struct StartupRuntime {
    descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor,
}

impl engine_runtime::project_runtime_module::ProjectRuntimeModule for StartupRuntime {
    fn descriptor(
        &self,
    ) -> &engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        _: &mut engine_runtime::project_runtime_module::ProjectRuntimeRegistration,
    ) -> Result<(), engine_runtime::project_runtime_module::ProjectRuntimeError> {
        Ok(())
    }
}

fn startup_identity() -> editor_core::ProjectEditorCompositionIdentity {
    editor_core::ProjectEditorCompositionIdentity {
        schema_version: editor_core::PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
        project_id: "fixture.startup".to_string(),
        module_id: "fixture.startup.runtime".to_string(),
        interface_version: "project-runtime-module.v2".to_string(),
        aot_content_digest: format!("sha256:{}", "a".repeat(64)),
        editor_build_identity: format!("sha256:{}", "b".repeat(64)),
        engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
        toolchain_identity: "rustc-test".to_string(),
        target_triple: "x86_64-pc-windows-msvc".to_string(),
        profile: "release".to_string(),
        normalized_manifest_digest: format!("sha256:{}", "d".repeat(64)),
        normalized_dependency_digest: format!("sha256:{}", "e".repeat(64)),
        dependency_lock_digest: format!("sha256:{}", "f".repeat(64)),
    }
}

#[test]
fn editor_composition_startup_injects_singleton_set_and_exact_identity_into_session() {
    let identity = startup_identity();
    let linked = engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(
        Arc::new(StartupRuntime {
            descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::new(
                identity.module_id.clone(),
                identity.aot_content_digest.clone(),
            ),
        }),
    )
    .unwrap();
    let session =
        EditorSession::with_project_editor_composition(Arc::new(linked), identity.clone()).unwrap();
    let app = NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);

    assert_eq!(
        app.session()
            .project_editor_composition_identity()
            .map(|value| value.digest().unwrap()),
        Some(identity.digest().unwrap())
    );
}

#[test]
fn verified_handoff_open_requires_running_composition_identity_and_skips_second_trust_prompt() {
    let mut identity = startup_identity();
    identity.project_id = "project-4966952341520437268".to_string();
    identity.module_id = "sample.tower-defense.runtime".to_string();
    let linked = engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(
        Arc::new(StartupRuntime {
            descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::new(
                identity.module_id.clone(),
                identity.aot_content_digest.clone(),
            ),
        }),
    )
    .unwrap();
    let session =
        EditorSession::with_project_editor_composition(Arc::new(linked), identity).unwrap();
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);

    assert!(app
        .dispatch_verified_composition_handoff_project_open(
            &project_root(),
            "verified-handoff-open".to_string(),
        )
        .is_none());
    assert!(app.latest_model().project_launcher.activity.is_some());
    assert!(app.latest_model().project_runtime_trust_prompt.is_none());
}

struct HandoffClock;
impl editor_core::EditorCompositionClock for HandoffClock {
    fn now_epoch_ms(&self) -> u64 {
        10
    }
}

struct HandoffWorkspace;
impl editor_core::EditorCompositionWorkspaceAdapter for HandoffWorkspace {
    fn save_recoverable_state(&self, _: &std::path::Path) -> Result<String, String> {
        Ok("workspace-state.json".to_string())
    }
}

struct HandoffProcess {
    ticket_path: Mutex<Option<std::path::PathBuf>>,
}
impl editor_core::EditorCompositionProcessAdapter for HandoffProcess {
    fn launch_candidate(
        &self,
        _: &std::path::Path,
        ticket_path: &std::path::Path,
    ) -> Result<u32, String> {
        *self.ticket_path.lock().unwrap() = Some(ticket_path.to_path_buf());
        Ok(77)
    }

    fn candidate_state(
        &self,
        _: u32,
    ) -> Result<editor_core::EditorCompositionCandidateProcessState, String> {
        Ok(editor_core::EditorCompositionCandidateProcessState::Running)
    }

    fn terminate_owned_candidate(&self, _: u32) -> Result<(), String> {
        Ok(())
    }
}

struct HandoffExit(AtomicUsize);
impl editor_core::EditorCompositionExitAdapter for HandoffExit {
    fn request_graceful_exit(&self) -> Result<(), String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct ApprovedRequestPreparer {
    artifact_root: std::path::PathBuf,
}

struct BlockingCompositionPreparer {
    started: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

impl ProjectEditorCompositionPreparationAdapter for BlockingCompositionPreparer {
    fn prepare(
        &self,
        _approved: ApprovedProjectRuntimeTrustRequest,
        control: editor_core::ProjectEditorCompositionPreparationControl,
        progress: &mut dyn FnMut(editor_core::ProjectEditorCompositionPreparationPhase),
    ) -> Result<
        editor_core::ProjectEditorCompositionArtifact,
        editor_core::ProjectEditorCompositionDiagnostic,
    > {
        self.started.store(true, Ordering::Release);
        progress(editor_core::ProjectEditorCompositionPreparationPhase::Compiling);
        while !control.is_cancelled() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        self.finished.store(true, Ordering::Release);
        Err(editor_core::ProjectEditorCompositionDiagnostic {
            code: "project_editor_composition.cancelled".to_string(),
            stage: "prepare".to_string(),
            message: "cancelled".to_string(),
            path: None,
            expected_identity: None,
            actual_identity: None,
            next_action: "Close the Editor.".to_string(),
        })
    }
}

impl ProjectEditorCompositionPreparationAdapter for ApprovedRequestPreparer {
    fn prepare(
        &self,
        approved: ApprovedProjectRuntimeTrustRequest,
        _control: editor_core::ProjectEditorCompositionPreparationControl,
        _progress: &mut dyn FnMut(editor_core::ProjectEditorCompositionPreparationPhase),
    ) -> Result<
        editor_core::ProjectEditorCompositionArtifact,
        editor_core::ProjectEditorCompositionDiagnostic,
    > {
        fs::create_dir_all(&self.artifact_root).unwrap();
        let executable = self.artifact_root.join("editor.exe");
        fs::write(&executable, b"approved-editor").unwrap();
        let manifest: editor_core::ProjectManifest = serde_json::from_slice(
            &fs::read(approved.project_root.join("project.aife.json")).unwrap(),
        )
        .unwrap();
        let identity = editor_core::ProjectEditorCompositionIdentity {
            schema_version: editor_core::PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION
                .to_string(),
            project_id: approved.trust_request.project_id,
            module_id: manifest.runtime_module.module_id,
            interface_version: manifest.runtime_module.interface_version,
            aot_content_digest: format!("sha256:{}", "a".repeat(64)),
            editor_build_identity: approved.trust_request.editor_build_identity,
            engine_sdk_digest: format!("sha256:{}", "b".repeat(64)),
            toolchain_identity: "rustc-test".to_string(),
            target_triple: "x86_64-pc-windows-msvc".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: approved.trust_request.normalized_manifest_digest,
            normalized_dependency_digest: approved.trust_request.normalized_dependency_digest,
            dependency_lock_digest: format!("sha256:{}", "c".repeat(64)),
        };
        Ok(editor_core::ProjectEditorCompositionArtifact {
            schema_version: editor_core::PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION
                .to_string(),
            executable_path: executable,
            descriptor_path: self.artifact_root.join("composition-descriptor.json"),
            build_report_path: self.artifact_root.join("build-report.json"),
            descriptor: editor_core::ProjectEditorCompositionDescriptor {
                schema_version: editor_core::PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION
                    .to_string(),
                identity_digest: identity.digest().unwrap(),
                resolved_identity: resolved_identity(&identity),
                identity,
                executable_hash: engine_runtime::canonical_digest::sha256_prefixed(
                    b"approved-editor",
                ),
                created_at: 1,
            },
        })
    }
}

#[test]
fn approved_project_runtime_is_prepared_and_handed_off_from_application_stable_point() {
    let state = temp_root();
    let trust = editor_core::ProjectRuntimeTrustModule::open(state.0.join("trust")).unwrap();
    let process = Arc::new(HandoffProcess {
        ticket_path: Mutex::new(None),
    });
    let exit = Arc::new(HandoffExit(AtomicUsize::new(0)));
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
            trust_module: trust,
            engine_sdk_root: engine_sdk_root(),
            editor_build_identity: format!("sha256:{}", "d".repeat(64)),
        });
    app.install_project_editor_composition_preparer(
        Arc::new(ApprovedRequestPreparer {
            artifact_root: state.0.join("artifact"),
        }),
        state.0.join("tickets"),
    );
    fs::create_dir_all(state.0.join("tickets")).unwrap();
    app.install_project_editor_composition_launcher(
        editor_core::EditorProjectCompositionLauncher::new(
            Arc::new(HandoffClock),
            Arc::new(HandoffWorkspace),
            process.clone(),
            exit,
        ),
    );

    assert!(app
        .dispatch_project_launcher_command_or_dispatch(open_command(&project_root()))
        .is_none());
    let request_id = app
        .latest_model()
        .project_runtime_trust_prompt
        .as_ref()
        .unwrap()
        .request_id
        .clone();
    let result = app.dispatch_command(UiCommand {
        command_id: "approve_project_runtime_trust".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "approve-production".to_string(),
        payload: UiCommandPayload::ApproveProjectRuntimeTrust { request_id },
    });
    assert_eq!(result.status, CommandStatus::Committed);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        app.frame(1280.0, 720.0);
        if process.ticket_path.lock().unwrap().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        process.ticket_path.lock().unwrap().is_some(),
        "receipt: {:#?}",
        app.last_project_editor_composition_launch_receipt()
    );
    assert_eq!(
        app.last_project_editor_composition_launch_receipt()
            .unwrap()
            .status,
        editor_core::ProjectEditorCompositionLaunchStatus::Pending
    );
    app.cancel_project_editor_composition_handoff();
}

#[test]
fn dropping_application_cancels_and_joins_project_editor_composition_worker() {
    let state = temp_root();
    let started = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
            trust_module: editor_core::ProjectRuntimeTrustModule::open(state.0.join("trust"))
                .unwrap(),
            engine_sdk_root: engine_sdk_root(),
            editor_build_identity: format!("sha256:{}", "d".repeat(64)),
        });
    app.install_project_editor_composition_preparer(
        Arc::new(BlockingCompositionPreparer {
            started: started.clone(),
            finished: finished.clone(),
        }),
        state.0.join("tickets"),
    );
    fs::create_dir_all(state.0.join("tickets")).unwrap();
    assert!(app
        .dispatch_project_launcher_command_or_dispatch(open_command(&project_root()))
        .is_none());
    let request_id = app
        .latest_model()
        .project_runtime_trust_prompt
        .as_ref()
        .unwrap()
        .request_id
        .clone();
    app.dispatch_command(UiCommand {
        command_id: "approve_project_runtime_trust".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "approve-cancellable-composition".to_string(),
        payload: UiCommandPayload::ApproveProjectRuntimeTrust { request_id },
    });
    app.frame(1280.0, 720.0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !started.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(started.load(Ordering::Acquire));

    drop(app);

    assert!(finished.load(Ordering::Acquire));
}

#[test]
fn project_editor_handoff_worker_completion_is_consumed_only_at_application_stable_point() {
    let temp = temp_root();
    let project = temp.0.join("project");
    let tickets = temp.0.join("tickets");
    let artifact_root = temp.0.join("artifact");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&tickets).unwrap();
    fs::create_dir_all(&artifact_root).unwrap();
    let executable = artifact_root.join("editor.exe");
    fs::write(&executable, b"sealed-editor").unwrap();
    let identity = startup_identity();
    let descriptor = editor_core::ProjectEditorCompositionDescriptor {
        schema_version: editor_core::PROJECT_EDITOR_COMPOSITION_DESCRIPTOR_SCHEMA_VERSION
            .to_string(),
        identity: identity.clone(),
        identity_digest: identity.digest().unwrap(),
        resolved_identity: resolved_identity(&identity),
        executable_hash: engine_runtime::canonical_digest::sha256_prefixed(b"sealed-editor"),
        created_at: 1,
    };
    let request = editor_core::EditorCompositionHandoffRequest {
        old_editor_instance_id: "old-editor".to_string(),
        running_identity_digest: None,
        artifact: editor_core::ProjectEditorCompositionArtifact {
            schema_version: editor_core::PROJECT_EDITOR_COMPOSITION_ARTIFACT_SCHEMA_VERSION
                .to_string(),
            executable_path: executable.clone(),
            descriptor_path: artifact_root.join("composition-descriptor.json"),
            build_report_path: artifact_root.join("build-report.json"),
            descriptor,
        },
        project_root: project.clone(),
        project_id: identity.project_id.clone(),
        ticket_root: tickets,
        timeout_ms: 100,
    };
    let process = Arc::new(HandoffProcess {
        ticket_path: Mutex::new(None),
    });
    let exit = Arc::new(HandoffExit(AtomicUsize::new(0)));
    let launcher = editor_core::EditorProjectCompositionLauncher::new(
        Arc::new(HandoffClock),
        Arc::new(HandoffWorkspace),
        process.clone(),
        exit.clone(),
    );
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    app.install_project_editor_composition_launcher(launcher);
    assert_eq!(
        app.begin_project_editor_composition_handoff(request.clone())
            .status,
        editor_core::ProjectEditorCompositionLaunchStatus::Pending
    );
    app.frame(1280.0, 720.0);
    assert_eq!(exit.0.load(Ordering::SeqCst), 0);

    let ticket_path = process.ticket_path.lock().unwrap().clone().unwrap();
    let mut readiness = Some(
        editor_core::prepare_editor_composition_candidate_readiness(
            ticket_path,
            executable,
            identity,
            "new-editor".to_string(),
            77,
            11,
        )
        .unwrap(),
    );
    assert!(
        crate::acknowledge_project_editor_candidate_after_present(false, &mut readiness)
            .unwrap()
            .is_none()
    );
    assert!(readiness.is_some());
    assert!(
        crate::acknowledge_project_editor_candidate_after_present(true, &mut readiness)
            .unwrap()
            .is_some()
    );
    assert!(readiness.is_none());
    assert_eq!(exit.0.load(Ordering::SeqCst), 0);
    app.frame(1280.0, 720.0);
    assert_eq!(exit.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.last_project_editor_composition_launch_receipt()
            .unwrap()
            .status,
        editor_core::ProjectEditorCompositionLaunchStatus::Ready
    );
}

#[test]
fn project_runtime_trust_prompt_requires_explicit_approve_and_cancel_writes_nothing() {
    let state = temp_root();
    let trust = editor_core::ProjectRuntimeTrustModule::open(&state.0).unwrap();
    let environment = ProjectRuntimeTrustEnvironment {
        trust_module: trust.clone(),
        engine_sdk_root: engine_sdk_root(),
        editor_build_identity: "sha256:editor-one".to_string(),
    };
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_project_runtime_trust_environment(environment.clone());

    assert!(app
        .dispatch_project_launcher_command_or_dispatch(open_command(&project_root()))
        .is_none());
    app.frame(1280.0, 720.0);
    let prompt = app
        .latest_model()
        .project_runtime_trust_prompt
        .clone()
        .expect("ProjectRust trust prompt");
    assert_eq!(prompt.module_id, "sample.tower-defense.runtime");
    assert!(prompt
        .dependency_summary
        .iter()
        .any(|dependency| dependency.starts_with("serde ")));
    assert_eq!(
        app.latest_draw_list()
            .hit_regions
            .iter()
            .filter(|region| matches!(region.target, HitTarget::ProjectRuntimeTrustDecision { .. }))
            .count(),
        3
    );
    let cancelled = app.dispatch_command(UiCommand {
        command_id: "cancel_project_runtime_trust".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "trust-cancel".to_string(),
        payload: UiCommandPayload::CancelProjectRuntimeTrust {
            request_id: prompt.request_id,
        },
    });
    assert_eq!(cancelled.status, CommandStatus::Committed);
    assert!(app.take_approved_project_runtime_trust_request().is_none());

    let inspection = ProjectRuntimeTrustInspection::inspect(
        project_root(),
        engine_sdk_root(),
        "sha256:editor-one",
    )
    .unwrap();
    assert_eq!(
        trust.evaluate(&inspection.request, None).unwrap().status,
        ProjectRuntimeTrustStatus::Required
    );

    assert!(app
        .dispatch_project_launcher_command_or_dispatch(open_command(&project_root()))
        .is_none());
    let prompt = app
        .latest_model()
        .project_runtime_trust_prompt
        .clone()
        .unwrap();
    let approved = app.dispatch_command(UiCommand {
        command_id: "approve_project_runtime_trust".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "trust-approve".to_string(),
        payload: UiCommandPayload::ApproveProjectRuntimeTrust {
            request_id: prompt.request_id,
        },
    });
    assert_eq!(approved.status, CommandStatus::Committed);
    assert!(app.take_approved_project_runtime_trust_request().is_some());
    assert_eq!(
        trust.evaluate(&inspection.request, None).unwrap().status,
        ProjectRuntimeTrustStatus::Trusted
    );

    let mut stale_app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
            editor_build_identity: "sha256:editor-two".to_string(),
            ..environment
        });
    assert!(stale_app
        .dispatch_project_launcher_command_or_dispatch(open_command(&project_root()))
        .is_none());
    assert!(stale_app
        .latest_model()
        .project_runtime_trust_prompt
        .as_ref()
        .is_some_and(|prompt| prompt.identity_changed));
}

#[test]
fn recent_project_selection_reuses_project_runtime_trust_review() {
    let state = temp_root();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
            trust_module: editor_core::ProjectRuntimeTrustModule::open(&state.0).unwrap(),
            engine_sdk_root: engine_sdk_root(),
            editor_build_identity: "sha256:editor-recent".to_string(),
        });
    let path = project_root().display().to_string();

    let result = app.dispatch_project_launcher_command_or_dispatch(UiCommand {
        command_id: "select_recent_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "recent-project-trust".to_string(),
        payload: UiCommandPayload::SelectRecentProject { path },
    });

    assert!(
        result.is_none(),
        "recent ProjectRust open must pause for review"
    );
    let prompt = app
        .latest_model()
        .project_runtime_trust_prompt
        .as_ref()
        .expect("recent ProjectRust selection must use the shared trust prompt");
    assert_eq!(prompt.module_id, "sample.tower-defense.runtime");
    assert_eq!(
        app.latest_model().mode,
        editor_ui_model::EditorUiMode::ProjectLauncher
    );
}

#[test]
fn recent_project_trust_rejection_preserves_source_command_identity() {
    let state = temp_root();
    let trust = editor_core::ProjectRuntimeTrustModule::open(&state.0).unwrap();
    let inspection = ProjectRuntimeTrustInspection::inspect(
        project_root(),
        engine_sdk_root(),
        "sha256:editor-recent-denied",
    )
    .unwrap();
    trust
        .record_explicit(
            &inspection.request,
            editor_core::ProjectRuntimeTrustDecisionKind::Denied,
            1,
        )
        .unwrap();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
            trust_module: trust,
            engine_sdk_root: engine_sdk_root(),
            editor_build_identity: "sha256:editor-recent-denied".to_string(),
        });

    let result = app
        .dispatch_project_launcher_command_or_dispatch(UiCommand {
            command_id: "select_recent_project".to_string(),
            source: UiCommandSource::ProjectLauncher,
            request_id: "recent-project-denied".to_string(),
            payload: UiCommandPayload::SelectRecentProject {
                path: project_root().display().to_string(),
            },
        })
        .expect("denied recent project must return a failure result");

    assert_eq!(result.command_id, "select_recent_project");
    assert_eq!(result.request_id, "recent-project-denied");
    assert_eq!(result.status, CommandStatus::Rejected);
}
