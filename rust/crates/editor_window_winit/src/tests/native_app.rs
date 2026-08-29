use super::*;
use editor_ui_model::EditorLocaleId;
use editor_ui_renderer::DrawCommand;

struct BlockingProjectOpenPreparationAdapter {
    started: std::sync::Arc<std::sync::atomic::AtomicBool>,
    release: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ProjectOpenPreparationAdapter for BlockingProjectOpenPreparationAdapter {
    fn prepare(
        &self,
        project_root: &std::path::Path,
        progress: &mut dyn FnMut(editor_core::ProjectOpenPreparationPhase),
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<editor_core::PreparedProjectOpen, editor_core::ProjectOpenPreparationError> {
        use std::sync::atomic::Ordering;
        self.started.store(true, Ordering::Release);
        while !self.release.load(Ordering::Acquire) {
            if cancelled.load(Ordering::Acquire) {
                return Err(editor_core::ProjectOpenPreparationError {
                    code: "project_open.cancelled".to_string(),
                    message: "Project open preparation was cancelled.".to_string(),
                    path: None,
                    next_action: "Retry opening the project.".to_string(),
                });
            }
            std::thread::yield_now();
        }
        editor_core::ProjectOpenPreparation::prepare(project_root, progress)
    }
}

struct BlockingEditorPlayPreparationAdapter {
    started: std::sync::Arc<std::sync::atomic::AtomicBool>,
    release: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl EditorPlayPreparationAdapter for BlockingEditorPlayPreparationAdapter {
    fn prepare(
        &self,
        ticket: &editor_core::EditorPlayPreparationTicket,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<editor_core::EditorPlayPreviewPackageReport, editor_core::EditorPlayPreparationError>
    {
        use std::sync::atomic::Ordering;
        self.started.store(true, Ordering::Release);
        while !self.release.load(Ordering::Acquire) {
            if cancelled.load(Ordering::Acquire) {
                return Err(editor_core::EditorPlayPreparationError {
                    code: "editor.play_preparation.cancelled".to_string(),
                    message: "Editor Play preparation was cancelled.".to_string(),
                });
            }
            std::thread::yield_now();
        }
        Ok(editor_core::EditorPreviewPackageService::prepare(
            ticket.request.clone(),
        ))
    }
}

struct BlockingProjectRuntimePreparationAdapter {
    started: std::sync::Arc<std::sync::atomic::AtomicBool>,
    release: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

struct FixtureLoadedProjectRuntime {
    descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor,
}

impl engine_runtime::project_runtime_module::ProjectRuntimeModule for FixtureLoadedProjectRuntime {
    fn descriptor(
        &self,
    ) -> &engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        _registration: &mut engine_runtime::project_runtime_module::ProjectRuntimeRegistration,
    ) -> Result<(), engine_runtime::project_runtime_module::ProjectRuntimeError> {
        Ok(())
    }
}

impl ProjectRuntimePreparationAdapter for BlockingProjectRuntimePreparationAdapter {
    fn prepare(
        &self,
        approved: ApprovedProjectRuntimeTrustRequest,
        control: editor_core::ProjectRuntimeNativeModuleBuildControl,
        progress: &mut dyn FnMut(ProjectRuntimePreparationPhase),
    ) -> Result<PreparedProjectRuntime, editor_core::ProjectRuntimeNativeModuleDiagnostic> {
        use std::sync::atomic::Ordering;
        progress(ProjectRuntimePreparationPhase::PreparingArtifact);
        self.started.store(true, Ordering::Release);
        while !self.release.load(Ordering::Acquire) {
            if control.is_cancelled() {
                return Err(editor_core::ProjectRuntimeNativeModuleDiagnostic {
                    code: "project_runtime.cancelled".to_string(),
                    stage: "prepare".to_string(),
                    message: "Fixture preparation was cancelled.".to_string(),
                    path: None,
                    next_action: "Open the current project again.".to_string(),
                });
            }
            std::thread::yield_now();
        }
        let manifest: editor_core::ProjectManifest = serde_json::from_slice(
            &std::fs::read(approved.project_root.join("project.aife.json")).unwrap(),
        )
        .unwrap();
        let digest = |value: char| format!("sha256:{}", value.to_string().repeat(64));
        let identity = editor_core::ProjectNativeModuleIdentity {
            schema_version: editor_core::PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION
                .to_string(),
            project_runtime_abi_digest: digest('1'),
            project_runtime_sdk_digest: digest('2'),
            project_id: approved.trust_request.project_id,
            module_id: manifest.runtime_module.module_id.clone(),
            logical_interface_version: manifest.runtime_module.interface_version.clone(),
            aot_content_digest: digest('3'),
            normalized_manifest_digest: approved.trust_request.normalized_manifest_digest,
            normalized_dependency_digest: approved.trust_request.normalized_dependency_digest,
            dependency_lock_digest: digest('4'),
            toolchain_identity: "rustc-test".to_string(),
            target_triple: "host".to_string(),
            profile: "release".to_string(),
            features: Vec::new(),
            builder_schema_version:
                editor_core::PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION.to_string(),
        };
        let linked = engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(
            std::sync::Arc::new(FixtureLoadedProjectRuntime {
                descriptor:
                    engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::new(
                        manifest.runtime_module.module_id,
                        identity.aot_content_digest.clone(),
                    ),
            }),
        )
        .unwrap();
        progress(ProjectRuntimePreparationPhase::LoadingModule);
        Ok(PreparedProjectRuntime {
            identity,
            linked_project_runtimes: std::sync::Arc::new(linked),
        })
    }
}

fn trusted_runtime_preparation_app(
    project_root: &std::path::Path,
    started: std::sync::Arc<std::sync::atomic::AtomicBool>,
    release: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> NativeEditorApplication {
    let rust_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf();
    let trust_identity = format!(
        "sha256:{}",
        project_runtime_abi::project_runtime_abi_digest_hex()
    );
    let inspection = editor_core::ProjectRuntimeTrustInspection::inspect(
        project_root,
        &rust_root,
        trust_identity.clone(),
    )
    .unwrap();
    let trust_root = project_root.parent().unwrap().join(format!(
        "{}-trust",
        project_root.file_name().unwrap().to_string_lossy()
    ));
    let trust = editor_core::ProjectRuntimeTrustModule::open(&trust_root).unwrap();
    trust
        .record_explicit(
            &inspection.request,
            editor_core::ProjectRuntimeTrustDecisionKind::Trusted,
            1,
        )
        .unwrap();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    app.install_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
        trust_module: trust,
        engine_sdk_root: rust_root,
        editor_build_identity: trust_identity,
    });
    app.install_project_runtime_preparer(std::sync::Arc::new(
        BlockingProjectRuntimePreparationAdapter { started, release },
    ));
    app
}

fn write_project_rust_fixture_for_preparation() -> std::path::PathBuf {
    let root = write_editor_project_fixture_for_shell();
    let rust_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap();
    let runtime_root = root.join("RuntimeModule");
    std::fs::create_dir_all(runtime_root.join("src")).unwrap();
    let abi = rust_root
        .join("crates/project_runtime_abi")
        .display()
        .to_string()
        .replace('\\', "/");
    let sdk = rust_root
        .join("crates/project_runtime_sdk")
        .display()
        .to_string()
        .replace('\\', "/");
    std::fs::write(
        runtime_root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "fixture_project_runtime"
version = "0.0.3"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
project_runtime_abi = {{ path = "{abi}" }}
project_runtime_sdk = {{ path = "{sdk}" }}
serde = {{ version = "1", features = ["derive"] }}
"#
        ),
    )
    .unwrap();
    let fixture_root = rust_root.join("fixtures/project_runtime_native_module_minimal");
    let lock = std::fs::read_to_string(fixture_root.join("Cargo.lock"))
        .unwrap()
        .replace(
            "project_runtime_native_module_minimal",
            "fixture_project_runtime",
        );
    std::fs::write(runtime_root.join("Cargo.lock"), lock).unwrap();
    std::fs::copy(
        fixture_root.join("src/lib.rs"),
        runtime_root.join("src/lib.rs"),
    )
    .unwrap();
    let manifest_path = root.join("project.aife.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["runtimeModule"] = serde_json::json!({
        "sourceKind": "projectRust",
        "moduleId": "fixture.native.runtime",
        "interfaceVersion": "project-runtime-module.v2",
        "cargoManifest": "RuntimeModule/Cargo.toml",
        "cargoPackage": "fixture_project_runtime",
        "playerBinary": "fixture_project_player"
    });
    std::fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    root
}

fn specialized_composition_session_for_project(project_root: &std::path::Path) -> EditorSession {
    let manifest: editor_core::ProjectManifest =
        serde_json::from_slice(&std::fs::read(project_root.join("project.aife.json")).unwrap())
            .unwrap();
    let digest = |value: char| format!("sha256:{}", value.to_string().repeat(64));
    let identity = editor_core::ProjectEditorCompositionIdentity {
        schema_version: editor_core::PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
        project_id: manifest.project_id,
        module_id: manifest.runtime_module.module_id.clone(),
        interface_version: manifest.runtime_module.interface_version.clone(),
        aot_content_digest: digest('3'),
        editor_build_identity: digest('4'),
        engine_sdk_digest: digest('5'),
        toolchain_identity: "rustc-test".to_string(),
        target_triple: "host".to_string(),
        profile: "release".to_string(),
        normalized_manifest_digest: digest('6'),
        normalized_dependency_digest: digest('7'),
        dependency_lock_digest: digest('8'),
    };
    let linked = engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(
        std::sync::Arc::new(FixtureLoadedProjectRuntime {
            descriptor: engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor::new(
                manifest.runtime_module.module_id,
                identity.aot_content_digest.clone(),
            ),
        }),
    )
    .unwrap();
    EditorSession::with_project_editor_composition(std::sync::Arc::new(linked), identity).unwrap()
}

#[test]
fn headless_native_editor_window_app_builds_frame_report() {
    let model = fixture_model();
    let mut app = HeadlessNativeEditorWindowApp::new(NativeEditorWindowConfig::default());

    let report = app.frame(&model);

    assert!(report.window_created);
    assert!(report.surface_configured);
    assert_eq!(report.present_status, "presented");
    assert!(report.draw_command_count > 0);
}

#[test]
fn headless_native_editor_window_app_resize_updates_surface_state() {
    let mut app = HeadlessNativeEditorWindowApp::new(NativeEditorWindowConfig::default());

    app.resize(800, 600);
    let report = app.report();

    assert_eq!(report.resize_count, 1);
    assert!(report.surface_configured);
}

#[test]
fn headless_native_editor_window_app_click_routes_to_ui_command() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let region = draw_list
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::ToolbarCommand { .. }))
        .expect("toolbar hit region");
    let mut app = HeadlessNativeEditorWindowApp::new(NativeEditorWindowConfig::default());

    let report = app.click(region.rect.x + 1.0, region.rect.y + 1.0, &draw_list);

    assert_eq!(report.input_event_count, 1);
    assert_eq!(report.ui_command_count, 1);
}

#[test]
fn headless_native_editor_window_app_close_sets_report_state() {
    let mut app = HeadlessNativeEditorWindowApp::new(NativeEditorWindowConfig::default());

    app.close();

    assert!(app.report().close_requested);
}

#[test]
fn native_editor_application_has_complete_shell_registry() {
    let app = NativeEditorApplication::new(NativeEditorWindowConfig::default());

    assert_eq!(
        app.main_frame().layout_version,
        "native-editor-main-frame.v1"
    );
    for panel_id in editor_ui_renderer::native_editor_panel_manifest()
        .iter()
        .filter(|entry| entry.dockable)
        .map(|entry| entry.panel_id)
    {
        assert!(
            app.workspace_docking().registry().contains(panel_id),
            "missing panel {panel_id}"
        );
    }
    assert!(app.command_system().contains("select_scene_entity"));
    assert!(app.command_system().contains("open_project"));
    assert!(app.command_system().contains("create_project"));
    assert!(app.command_system().contains("undo_scene_edit"));
}

#[test]
fn launcher_project_open_prepares_off_thread_rejects_duplicate_and_commits_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let project_root = write_editor_project_fixture_for_shell();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    let started = std::sync::Arc::new(AtomicBool::new(false));
    let release = std::sync::Arc::new(AtomicBool::new(false));
    app.install_project_open_preparation_adapter(std::sync::Arc::new(
        BlockingProjectOpenPreparationAdapter {
            started: started.clone(),
            release: release.clone(),
        },
    ));
    let payload = UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    };
    let command = UiCommand {
        command_id: editor_ui_model::ui_command_id_for_payload(&payload).to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "async-project-open-first".to_string(),
        payload: payload.clone(),
    };

    assert!(app
        .dispatch_project_launcher_command_or_dispatch(command)
        .is_none());
    assert!(app.latest_model().project_launcher.activity.is_some());
    let duplicate = app
        .dispatch_project_launcher_command_or_dispatch(UiCommand {
            command_id: editor_ui_model::ui_command_id_for_payload(&payload).to_string(),
            source: UiCommandSource::ProjectLauncher,
            request_id: "async-project-open-duplicate".to_string(),
            payload,
        })
        .expect("duplicate project open must be rejected synchronously");
    assert_eq!(duplicate.status, CommandStatus::Rejected);
    assert_eq!(duplicate.diagnostics[0].code, "editor.project_open.busy");

    let initial_frame = app.report().frame_index;
    while !started.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    for _ in 0..5 {
        app.frame(1280.0, 720.0);
    }
    assert!(app.report().frame_index >= initial_frame + 5);
    assert!(app.latest_model().project_launcher.activity.is_some());
    release.store(true, Ordering::Release);
    for _ in 0..200 {
        app.frame(1280.0, 720.0);
        if app.latest_model().project_launcher.activity.is_none()
            && app.latest_model().mode == EditorUiMode::AuthoringWorkspace
        {
            break;
        }
        std::thread::yield_now();
    }
    assert!(app.report().frame_index > initial_frame);
    assert_eq!(app.latest_model().mode, EditorUiMode::AuthoringWorkspace);
    assert!(app.latest_model().project_launcher.activity.is_none());

    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn stable_editor_project_runtime_cutover_project_open_authoring_remains_responsive_while_native_module_builds(
) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let project_root = write_project_rust_fixture_for_preparation();
    let trust_root = project_root.parent().unwrap().join(format!(
        "{}-trust",
        project_root.file_name().unwrap().to_string_lossy()
    ));
    let started = std::sync::Arc::new(AtomicBool::new(false));
    let release = std::sync::Arc::new(AtomicBool::new(false));
    let mut app = trusted_runtime_preparation_app(&project_root, started.clone(), release.clone());
    let payload = UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    };
    assert!(app
        .dispatch_project_launcher_command_or_dispatch(UiCommand {
            command_id: editor_ui_model::ui_command_id_for_payload(&payload).to_string(),
            source: UiCommandSource::ProjectLauncher,
            request_id: "authoring-first-runtime".to_string(),
            payload,
        })
        .is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        app.frame(1280.0, 720.0);
        if started.load(Ordering::Acquire)
            && app.latest_model().mode == EditorUiMode::AuthoringWorkspace
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        started.load(Ordering::Acquire),
        "mode={:?} preparation={:?} trust_prompt={:?} activity={:?} feedback={:?}",
        app.latest_model().mode,
        app.session().project_runtime_preparation_state(),
        app.latest_model().project_runtime_trust_prompt,
        app.latest_model().project_launcher.activity,
        app.report().last_feedback,
    );
    assert_eq!(app.latest_model().mode, EditorUiMode::AuthoringWorkspace);
    let frame_before = app.report().frame_index;
    for _ in 0..8 {
        app.frame(1280.0, 720.0);
    }
    assert_eq!(app.report().frame_index, frame_before + 8);
    let play = app
        .latest_model()
        .toolbar
        .commands
        .iter()
        .find(|command| command.command_id == "play")
        .unwrap();
    assert!(!play.enabled);
    assert_eq!(
        play.reason_disabled.as_deref(),
        Some("project_runtime.preparation_pending")
    );
    let rejected = app.dispatch_command(editor_core::command_for_test(UiCommandPayload::Play));
    assert_eq!(rejected.status, CommandStatus::Rejected);
    assert_eq!(
        rejected.diagnostics[0].code,
        "project_runtime.preparation_pending"
    );

    release.store(true, Ordering::Release);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        app.frame(1280.0, 720.0);
        if matches!(
            app.session().project_runtime_preparation_state(),
            editor_core::ProjectRuntimePreparationState::Ready { .. }
        ) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(matches!(
        app.session().project_runtime_preparation_state(),
        editor_core::ProjectRuntimePreparationState::Ready { .. }
    ));
    app.frame(1280.0, 720.0);
    assert!(
        app.latest_model()
            .toolbar
            .commands
            .iter()
            .find(|command| command.command_id == "play")
            .unwrap()
            .enabled
    );
    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
    std::fs::remove_dir_all(trust_root).unwrap();
}

#[test]
fn project_runtime_preparation_cancel_joins_owned_worker() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let project_root = write_project_rust_fixture_for_preparation();
    let trust_root = project_root.parent().unwrap().join(format!(
        "{}-trust",
        project_root.file_name().unwrap().to_string_lossy()
    ));
    let started = std::sync::Arc::new(AtomicBool::new(false));
    let release = std::sync::Arc::new(AtomicBool::new(false));
    let mut app = trusted_runtime_preparation_app(&project_root, started.clone(), release);
    let payload = UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    };
    assert!(app
        .dispatch_project_launcher_command_or_dispatch(UiCommand {
            command_id: editor_ui_model::ui_command_id_for_payload(&payload).to_string(),
            source: UiCommandSource::ProjectLauncher,
            request_id: "cancel-runtime-preparation".to_string(),
            payload,
        })
        .is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        app.frame(1280.0, 720.0);
        if started.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        started.load(Ordering::Acquire),
        "mode={:?} preparation={:?} trust_prompt={:?} activity={:?} feedback={:?}",
        app.latest_model().mode,
        app.session().project_runtime_preparation_state(),
        app.latest_model().project_runtime_trust_prompt,
        app.latest_model().project_launcher.activity,
        app.report().last_feedback,
    );
    app.cancel_project_runtime_preparation();
    assert!(matches!(
        app.session().project_runtime_preparation_state(),
        editor_core::ProjectRuntimePreparationState::Inactive
    ));
    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
    std::fs::remove_dir_all(trust_root).unwrap();
}

#[test]
fn dropping_application_cancels_and_joins_project_open_worker() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let project_root = write_editor_project_fixture_for_shell();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    let started = std::sync::Arc::new(AtomicBool::new(false));
    app.install_project_open_preparation_adapter(std::sync::Arc::new(
        BlockingProjectOpenPreparationAdapter {
            started: started.clone(),
            release: std::sync::Arc::new(AtomicBool::new(false)),
        },
    ));
    let payload = UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    };
    assert!(app
        .dispatch_project_launcher_command_or_dispatch(UiCommand {
            command_id: editor_ui_model::ui_command_id_for_payload(&payload).to_string(),
            source: UiCommandSource::ProjectLauncher,
            request_id: "async-project-open-drop".to_string(),
            payload,
        })
        .is_none());
    while !started.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn editor_play_prepares_off_thread_rejects_duplicate_and_commits_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let started = std::sync::Arc::new(AtomicBool::new(false));
    let release = std::sync::Arc::new(AtomicBool::new(false));
    app.install_editor_play_preparation_adapter(std::sync::Arc::new(
        BlockingEditorPlayPreparationAdapter {
            started: started.clone(),
            release: release.clone(),
        },
    ));
    let play = UiCommand {
        command_id: "play".to_string(),
        source: UiCommandSource::Toolbar,
        request_id: "async-play-first".to_string(),
        payload: UiCommandPayload::Play,
    };

    let pending = app.dispatch_command(play.clone());
    assert_eq!(pending.status, CommandStatus::Pending);
    while !started.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    let duplicate = app.dispatch_command(UiCommand {
        request_id: "async-play-duplicate".to_string(),
        ..play
    });
    assert_eq!(duplicate.status, CommandStatus::Rejected);
    assert_eq!(
        duplicate.diagnostics[0].code,
        "editor.play_preparation.busy"
    );

    let initial_frame = app.report().frame_index;
    for _ in 0..5 {
        app.frame(1280.0, 720.0);
    }
    assert!(app.report().frame_index >= initial_frame + 5);
    let play_command = app
        .latest_model()
        .toolbar
        .commands
        .iter()
        .find(|command| command.command_id == "play")
        .expect("Play toolbar command");
    assert!(!play_command.enabled);
    assert_eq!(
        play_command.reason_disabled.as_deref(),
        Some("editor.play_preparation.busy")
    );
    assert!(app
        .report()
        .last_feedback
        .as_ref()
        .is_some_and(|feedback| feedback.message.contains("正在准备运行")));
    release.store(true, Ordering::Release);
    for _ in 0..500 {
        app.frame(1280.0, 720.0);
        if app.session().last_editor_preview_package_report().is_some() {
            break;
        }
        std::thread::yield_now();
    }
    let preview = app
        .session()
        .last_editor_preview_package_report()
        .expect("background preparation must commit exactly once");
    assert_eq!(preview.player_artifact_status, "not_required_in_process");

    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn dropping_application_cancels_and_joins_editor_play_worker() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let started = std::sync::Arc::new(AtomicBool::new(false));
    app.install_editor_play_preparation_adapter(std::sync::Arc::new(
        BlockingEditorPlayPreparationAdapter {
            started: started.clone(),
            release: std::sync::Arc::new(AtomicBool::new(false)),
        },
    ));
    let result = app.dispatch_command(UiCommand {
        command_id: "play".to_string(),
        source: UiCommandSource::Toolbar,
        request_id: "async-play-drop".to_string(),
        payload: UiCommandPayload::Play,
    });
    assert_eq!(result.status, CommandStatus::Pending);
    while !started.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn gateway_owner_thread_dispatch_gateway_native_editor_adapter_is_pumped_by_frame() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let hello = ai_tool_gateway::ClientHello {
        schema_version: ai_tool_gateway::GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
        gateway_protocol_version: ai_tool_gateway::GATEWAY_PROTOCOL_VERSION.to_string(),
        client_kind: ai_tool_gateway::ClientKind::Test,
        client_version: "native-editor-test.v1".to_string(),
        supported_schema_versions: vec![editor_core::AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
        expected_editor_instance_id: app.editor_instance_id().to_string(),
        requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
    };
    let client = app.gateway_client();
    let connect = client.submit_connect(hello).unwrap();
    app.frame(1280.0, 720.0);
    assert_eq!(app.last_gateway_requests_processed(), 1);
    let binding = connect.recv().unwrap().unwrap();
    let catalog = client
        .submit_dispatch(ai_tool_gateway::GatewayRequest {
            schema_version: ai_tool_gateway::GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: ai_tool_gateway::GATEWAY_PROTOCOL_VERSION.to_string(),
            request_id: "native-editor-catalog".to_string(),
            client_session_id: binding.client_session_id,
            deadline_epoch_ms: None,
            response_limit_bytes: 1024 * 1024,
            payload: ai_tool_gateway::GatewayRequestPayload::Catalog(
                editor_core::AiToolCatalogRequest::default(),
            ),
        })
        .unwrap();
    app.frame(1280.0, 720.0);
    assert_eq!(app.last_gateway_requests_processed(), 1);
    assert!(matches!(
        catalog.recv().unwrap().payload,
        ai_tool_gateway::GatewayReplyPayload::Catalog(_)
    ));
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn gateway_owner_thread_submission_wakes_idle_native_editor_host() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let (wake_sender, wake_receiver) = std::sync::mpsc::channel();
    let gateway_wake: ai_tool_gateway::GatewayOwnerThreadWake = std::sync::Arc::new(move || {
        let _ = wake_sender.send(());
    });
    let mut app =
        NativeEditorApplication::with_project_manager_and_dialog_initial_directory_and_gateway(
            NativeEditorWindowConfig::default(),
            session,
            ProjectManagerController::default(),
            Box::<HeadlessFolderDialogBackend>::default(),
            default_project_dialog_initial_directory(),
            Some(gateway_wake),
            None,
        );
    let connect =
        app.gateway_client()
            .submit_connect(ai_tool_gateway::ClientHello {
                schema_version: ai_tool_gateway::GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: ai_tool_gateway::GATEWAY_PROTOCOL_VERSION.to_string(),
                client_kind: ai_tool_gateway::ClientKind::Mcp,
                client_version: "idle-native-editor-wake-test.v1".to_string(),
                supported_schema_versions: vec![
                    editor_core::AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()
                ],
                expected_editor_instance_id: app.editor_instance_id().to_string(),
                requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
            })
            .unwrap();

    wake_receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("queued Gateway command must wake an idle Native Editor host");
    app.frame(1280.0, 720.0);
    assert!(connect.recv().unwrap().is_ok());
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn gateway_access_native_editor_approval_is_user_driven_and_session_bound() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let client = app.gateway_client();
    let connect =
        client
            .submit_connect(ai_tool_gateway::ClientHello {
                schema_version: ai_tool_gateway::GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
                gateway_protocol_version: ai_tool_gateway::GATEWAY_PROTOCOL_VERSION.to_string(),
                client_kind: ai_tool_gateway::ClientKind::Mcp,
                client_version: "codex-grant-test.v1".to_string(),
                supported_schema_versions: vec![
                    editor_core::AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()
                ],
                expected_editor_instance_id: app.editor_instance_id().to_string(),
                requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
            })
            .unwrap();
    app.frame(1280.0, 720.0);
    let binding = connect.recv().unwrap().unwrap();
    let project_context = binding
        .project_context
        .as_ref()
        .expect("opened Native Editor project context");
    app.request_gateway_goal_mutation_access(
        &binding.client_session_id,
        editor_core::AiGoalBinding::new(
            "native-editor-approval-test",
            "Apply the bounded test project change.",
            project_context.project_identity.clone(),
            project_context.project_digest.clone(),
            editor_core::AiGoalCompletionPolicy::CommitVerified,
        )
        .unwrap(),
        editor_core::AiRiskEnvelope::default_project_owned_low_risk().unwrap(),
    )
    .unwrap();
    let request = app
        .latest_model()
        .ai_panel
        .gateway_access
        .requests
        .iter()
        .find(|request| request.client_session_id == binding.client_session_id)
        .expect("connected Codex access request")
        .request_id
        .clone();

    let result = app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::ApproveGatewayAccessRequest {
            request_id: request,
        },
    ));

    assert_eq!(result.status, CommandStatus::Committed);
    let receipt = app
        .last_gateway_access_decision_receipt()
        .expect("Native Editor access decision receipt");
    assert_eq!(receipt.client_session_id, binding.client_session_id);
    assert_eq!(
        receipt.mutation_state,
        ai_tool_gateway::GatewayMutationAccessState::Active
    );
    assert!(receipt.grant_ref.is_some());
    assert!(app
        .latest_model()
        .ai_panel
        .gateway_access
        .requests
        .is_empty());
    let _ = std::fs::remove_dir_all(project_root);
}

#[cfg(windows)]
#[test]
fn gateway_host_lifecycle_stays_stable_across_launcher_and_project_switch() {
    let first_project_root = write_editor_project_fixture_for_shell();
    let second_project_root = write_editor_project_fixture_for_shell();
    let discovery_root = unique_project_launcher_temp_dir().join("gateway-discovery");
    let session = editor_core::EditorSession::new();
    let mut app = NativeEditorApplication::with_session_and_gateway_discovery_root(
        NativeEditorWindowConfig::default(),
        session,
        discovery_root.clone(),
    );

    app.frame(1280.0, 720.0);
    assert!(app.gateway_host_error().is_none());
    let first_discovery = app
        .gateway_discovery_path()
        .expect("launcher Gateway discovery")
        .to_path_buf();
    let editor_instance_id = app
        .gateway_host_binding()
        .expect("launcher Gateway binding")
        .editor_instance_id
        .clone();
    assert!(first_discovery.exists());

    let opened = app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: first_project_root.display().to_string(),
        },
    ));
    assert_eq!(opened.status, CommandStatus::Committed);
    app.frame(1280.0, 720.0);
    assert_eq!(
        app.gateway_discovery_path(),
        Some(first_discovery.as_path())
    );
    assert_eq!(
        app.gateway_host_binding()
            .expect("first project Gateway binding")
            .editor_instance_id,
        editor_instance_id
    );

    let switched = app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: second_project_root.display().to_string(),
        },
    ));
    assert_eq!(switched.status, CommandStatus::Committed);
    app.frame(1280.0, 720.0);
    assert!(app.gateway_host_error().is_none());
    let second_discovery = app
        .gateway_discovery_path()
        .expect("second project Gateway discovery")
        .to_path_buf();
    assert_eq!(first_discovery, second_discovery);
    assert_eq!(
        app.gateway_host_binding()
            .expect("second project Gateway binding")
            .editor_instance_id,
        editor_instance_id
    );
    assert!(first_discovery.exists());
    assert!(second_discovery.exists());

    drop(app);
    assert!(!second_discovery.exists());
    let _ = std::fs::remove_dir_all(discovery_root);
    let _ = std::fs::remove_dir_all(first_project_root);
    let _ = std::fs::remove_dir_all(second_project_root);
}

#[test]
fn panel_registry_covers_retained_panel_manifest() {
    let registry = editor_ui_renderer::EditorWorkspaceDockingModule::standard_editor();
    let missing: Vec<_> = editor_ui_renderer::native_editor_panel_manifest()
        .iter()
        .filter(|entry| entry.dockable && !registry.registry().contains(entry.panel_id))
        .map(|entry| entry.panel_id)
        .collect();
    assert!(
        missing.is_empty(),
        "panel registry missing retained roots: {missing:?}"
    );
}

#[test]
fn ai_panel_prompt_field_accepts_keyboard_input_and_structured_submit() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    activate_bottom_panel(&mut app, "ai_panel", 1280.0, 720.0);
    let prompt = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::AiPromptField))
        .expect("AI prompt field")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: prompt.x + 1.0,
        y: prompt.y + 1.0,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "A".to_string(),
    });
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "Space".to_string(),
    });
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "B".to_string(),
    });

    assert_eq!(app.latest_model().ai_panel.prompt_draft, "A B");
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "Enter".to_string(),
    });
    assert!(app.latest_model().ai_panel.busy);
    assert_eq!(
        app.latest_model().ai_panel.stage,
        editor_ui_model::AiPanelStage::Generating
    );
}

#[test]
fn native_editor_command_system_registers_build_export_commands() {
    let app = NativeEditorApplication::new(NativeEditorWindowConfig::default());

    for command_id in [
        "export_desktop_package",
        "build_and_run_desktop_package",
        "build_release_package",
        "save_release_profile",
        "set_release_profile_icon",
        "open_build_output",
        "open_build_report",
    ] {
        assert!(
            app.command_system().contains(command_id),
            "missing command {command_id}"
        );
    }
    assert_eq!(
        command_id_for_shell_payload(&UiCommandPayload::BuildAndRunDesktopPackage {
            profile_id: None
        }),
        "build_and_run_desktop_package"
    );
    assert_eq!(
        command_id_for_shell_payload(&UiCommandPayload::BuildReleasePackage {
            profile_id: Some("windows-release".to_string())
        }),
        "build_release_package"
    );
}

#[test]
fn native_build_export_routes_build_release_package_payload() {
    let mut model = fixture_model();
    model.build_export.commands = vec![editor_ui_model::BuildExportCommand::new(
        "build_release_package",
        "Build Release",
        true,
        None,
    )];
    let draw_list = SelfUiRenderer::build_draw_list(
        &model,
        UiRendererConfig::new(1280.0, 720.0)
            .with_active_bottom_panel(Some("build_export".to_string())),
    );
    let region = draw_list
        .hit_regions
        .iter()
        .find(|region| region.command_id.as_deref() == Some("build_release_package"))
        .expect("Build Release hit region");
    let point = (region.rect.x + 1.0, region.rect.y + 1.0);
    let mut isolated_draw_list = draw_list.clone();
    isolated_draw_list
        .hit_regions
        .retain(|candidate| candidate.id == region.id);
    let mut router = editor_input::EditorInputRouter::new();
    let routed = router.route(
        EditorInputEvent::PointerDown {
            x: point.0,
            y: point.1,
            button: PointerButton::Primary,
        },
        &isolated_draw_list,
    );

    assert_eq!(
        routed.command.unwrap().payload,
        UiCommandPayload::BuildReleasePackage {
            profile_id: Some("windows-release".to_string())
        }
    );
}

#[test]
fn native_editor_application_starts_in_project_launcher_mode() {
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());

    let report = app.frame(1280.0, 720.0);

    assert_eq!(report.mode, EditorUiMode::ProjectLauncher);
    assert!(app
        .latest_draw_list()
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.project_launcher.open_project"));
    assert!(!app
        .latest_draw_list()
        .hit_regions
        .iter()
        .any(|region| region.id == "hit.toolbar.tick_one_frame"));
}

#[test]
fn native_editor_language_menu_persists_and_publishes_english_from_launcher() {
    let root = unique_project_launcher_temp_dir();
    std::fs::create_dir_all(&root).expect("create preference root");
    let preference_path = root.join("editor_preferences.json");
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default())
        .with_editor_preference_store(EditorPreferenceStore::new(preference_path.clone()));

    app.frame(1280.0, 720.0);
    assert_eq!(app.localization_snapshot().locale, EditorLocaleId::zh_cn());
    assert!(app
        .latest_draw_list()
        .commands
        .iter()
        .any(|command| matches!(
            command.unclipped(),
            DrawCommand::Text { text, .. } if text == "打开项目"
        )));

    let language_button = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.editor.language_menu")
        .expect("launcher language menu")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: language_button.x + 1.0,
        y: language_button.y + 1.0,
        button: PointerButton::Primary,
    });
    app.frame(1280.0, 720.0);

    let english_choice = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.editor.locale.en-US")
        .expect("English locale choice")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: english_choice.x + 1.0,
        y: english_choice.y + 1.0,
        button: PointerButton::Primary,
    });
    app.frame(1280.0, 720.0);

    assert_eq!(app.localization_snapshot().locale, EditorLocaleId::en_us());
    assert!(app
        .latest_draw_list()
        .commands
        .iter()
        .any(|command| matches!(
            command.unclipped(),
            DrawCommand::Text { text, .. } if text == "Open Project"
        )));
    let persisted = std::fs::read_to_string(preference_path).expect("persisted preferences");
    assert!(persisted.contains("\"locale\": \"en-US\""));
}

#[test]
fn native_editor_launcher_create_with_ai_captures_local_draft_without_provider() {
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    app.frame(1280.0, 720.0);
    let create_with_ai = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.project_launcher.create_with_ai")
        .expect("Create with AI launcher action")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: create_with_ai.x + 1.0,
        y: create_with_ai.y + 1.0,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::PointerUp {
        x: create_with_ai.x + 1.0,
        y: create_with_ai.y + 1.0,
        button: PointerButton::Primary,
    });
    app.frame(1280.0, 720.0);
    let prompt = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.project_launcher.intent_prompt")
        .expect("pre-project intent prompt")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: prompt.x + 1.0,
        y: prompt.y + 1.0,
        button: PointerButton::Primary,
    });
    for key in ["A", "Space", "B", "Enter"] {
        app.handle_input_event(EditorInputEvent::KeyDown {
            key: key.to_string(),
        });
    }

    assert_eq!(app.latest_model().mode, EditorUiMode::ProjectLauncher);
    assert_eq!(
        app.latest_model()
            .project_intent
            .intent
            .latest_summary
            .as_deref(),
        Some("A B")
    );
    assert_eq!(app.latest_model().project_intent.intent.active_count, 1);
    assert!(!app.latest_model().ai_panel.busy);
}

#[test]
fn native_editor_application_create_project_command_enters_workspace() {
    let root = unique_project_launcher_temp_dir();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());

    let result = app.dispatch_command(UiCommand {
        command_id: "create_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "request-create-project".to_string(),
        payload: UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "LauncherGame".to_string(),
        },
    });

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(app.latest_model().mode, EditorUiMode::AuthoringWorkspace);
    assert!(root.join("project.aife.json").exists());
    assert_eq!(
        app.latest_model().hierarchy.scene_id.as_deref(),
        Some("scene-main")
    );
}

#[test]
fn native_editor_asset_browser_reuses_cached_snapshot_for_300_frames() {
    let root = unique_project_launcher_temp_dir();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    let result = app.dispatch_command(UiCommand {
        command_id: "create_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "asset-browser-cache-project".to_string(),
        payload: UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "AssetBrowserCacheGame".to_string(),
        },
    });
    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(app.latest_model().asset_browser.scan_generation, 1);

    for _ in 0..300 {
        app.frame(1280.0, 720.0);
    }

    assert_eq!(app.latest_model().asset_browser.scan_generation, 1);
    assert_eq!(
        app.latest_model().asset_browser.index_status,
        editor_ui_model::AssetBrowserIndexStatus::Ready
    );
}

#[test]
fn native_editor_asset_browser_search_keyboard_navigation_and_refresh_are_productized() {
    let root = unique_project_launcher_temp_dir();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    app.dispatch_command(UiCommand {
        command_id: "create_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "asset-browser-input-project".to_string(),
        payload: UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "AssetBrowserInputGame".to_string(),
        },
    });
    app.frame(1280.0, 720.0);
    activate_bottom_panel(&mut app, "asset_browser", 1280.0, 720.0);

    let search = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::AssetBrowserSearch))
        .expect("asset browser search field")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: search.x + 2.0,
        y: search.y + 2.0,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "M".to_string(),
    });
    assert_eq!(app.latest_model().asset_browser.query.search_text, "M");
    assert_eq!(app.latest_model().asset_browser.scan_generation, 1);

    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "Escape".to_string(),
    });
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "ArrowDown".to_string(),
    });
    assert!(app
        .latest_model()
        .asset_browser
        .selection
        .primary_entry_key
        .is_some());
    let selected_path = app
        .latest_model()
        .asset_browser
        .selection
        .primary_path
        .clone();
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "Enter".to_string(),
    });
    assert_eq!(
        app.latest_model().project_browser.selected_path,
        selected_path
    );

    let refresh = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                region.target,
                HitTarget::AssetBrowserAction {
                    action: editor_ui_model::AssetBrowserToolbarAction::Refresh
                }
            )
        })
        .expect("asset browser refresh button")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: refresh.x + 2.0,
        y: refresh.y + 2.0,
        button: PointerButton::Primary,
    });
    assert_eq!(
        app.latest_model().asset_browser.index_status,
        editor_ui_model::AssetBrowserIndexStatus::Scanning
    );
    for _ in 0..100 {
        app.frame(1280.0, 720.0);
        if app.latest_model().asset_browser.scan_generation == 2 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(app.latest_model().asset_browser.scan_generation, 2);
}

#[test]
fn native_editor_asset_browser_picker_routes_inspector_and_confirm() {
    let root = unique_project_launcher_temp_dir();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    let create = app.dispatch_command(UiCommand {
        command_id: "create_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "asset-browser-picker-project".to_string(),
        payload: UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "AssetBrowserPickerGame".to_string(),
        },
    });
    assert_eq!(create.status, CommandStatus::Committed);
    std::fs::write(root.join("Assets/new.png"), b"new-png").unwrap();
    std::fs::write(
        root.join("Assets/new.asset"),
        r#"{
  "schemaVersion": "texture-asset.v1",
  "assetId": "texture-new",
  "assetGuid": "guid-texture-new",
  "sourceImage": "Assets/new.png"
}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("Scenes/Main.scene.json"),
        r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{
    "schemaVersion": "editor-scene-entity.v1",
    "id": "entity-player",
    "name": "Player",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },
    "mesh": null,
    "components": [{
      "componentType": "SpriteRenderer2D",
      "fields": {
        "spriteRef": { "id": "texture-old", "type": "texture" }
      }
    }]
  }]
}"##,
    )
    .unwrap();
    assert_eq!(
        app.dispatch_command(editor_core::command_for_test(
            UiCommandPayload::OpenSceneDocument {
                path: root.join("Scenes/Main.scene.json").display().to_string(),
            },
        ))
        .status,
        CommandStatus::Committed
    );
    assert_eq!(
        app.dispatch_command(editor_core::command_for_test(
            UiCommandPayload::SelectSceneEntity {
                entity_id: "entity-player".to_string(),
            },
        ))
        .status,
        CommandStatus::Committed
    );
    app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::AssetBrowserToolbar {
            action: editor_ui_model::AssetBrowserToolbarAction::Refresh,
        },
    ));
    for _ in 0..100 {
        app.frame(1280.0, 720.0);
        if app.latest_model().asset_browser.scan_generation == 2 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(app.latest_model().asset_browser.scan_generation, 2);

    let picker_rect = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                region.target,
                HitTarget::InspectorAssetPicker { ref field_id }
                    if field_id == "components.SpriteRenderer2D.spriteRef"
            )
        })
        .expect("Inspector Asset Picker button")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: picker_rect.x + 1.0,
        y: picker_rect.y + 1.0,
        button: PointerButton::Primary,
    });
    assert!(app.latest_model().asset_browser.picker.is_some());
    activate_bottom_panel(&mut app, "asset_browser", 1280.0, 720.0);

    let texture_key = app
        .latest_model()
        .asset_browser
        .entries
        .iter()
        .find(|entry| entry.path == "Assets/new.asset")
        .expect("new texture in Picker query")
        .entry_key
        .clone();
    assert_eq!(
        app.dispatch_command(editor_core::command_for_test(
            UiCommandPayload::SelectAssetBrowserEntry {
                entry_key: texture_key,
                additive: false,
                range: false,
            },
        ))
        .status,
        CommandStatus::Committed
    );
    app.frame(1280.0, 720.0);
    let confirm_rect = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::AssetPickerConfirm) && region.enabled)
        .expect("enabled Asset Picker confirm button")
        .rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: confirm_rect.x + 1.0,
        y: confirm_rect.y + 1.0,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::PointerUp {
        x: confirm_rect.x + 1.0,
        y: confirm_rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert!(app.latest_model().asset_browser.picker.is_none());
    let sprite_field = app
        .latest_model()
        .inspector
        .sections
        .iter()
        .flat_map(|section| section.fields.iter())
        .find(|field| field.field_id == "components.SpriteRenderer2D.spriteRef")
        .expect("SpriteRenderer2D spriteRef Inspector field");
    let editor_ui_model::InspectorValue::AssetRef(reference) = &sprite_field.value else {
        panic!("expected structured AssetRef, got {:?}", sprite_field.value);
    };
    assert_eq!(reference.asset_id, "texture-new");
    assert_eq!(reference.asset_type_id, "texture");
    assert_eq!(reference.guid.as_deref(), Some("guid-texture-new"));
}

#[test]
fn native_editor_asset_thumbnail_requests_only_visible_slots_and_exposes_ready_cpu_payload() {
    let root = unique_project_launcher_temp_dir();
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    app.dispatch_command(UiCommand {
        command_id: "create_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "asset-thumbnail-create-project".to_string(),
        payload: UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "AssetThumbnailGame".to_string(),
        },
    });
    let image_dir = root.join("Assets").join("Images");
    std::fs::create_dir_all(&image_dir).expect("create image directory");
    let fixture_png = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/complex_shooter_project/Assets/Images/tex-player-ship.png");
    std::fs::copy(fixture_png, image_dir.join("player.png")).expect("copy PNG fixture");
    app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::AssetBrowserToolbar {
            action: editor_ui_model::AssetBrowserToolbarAction::Refresh,
        },
    ));
    activate_bottom_panel(&mut app, "asset_browser", 1280.0, 720.0);

    for _ in 0..200 {
        app.frame(1280.0, 720.0);
        let summary = app.asset_thumbnail_summary();
        if summary.ready_count > 0 && summary.pending_count == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let summary = app.asset_thumbnail_summary();
    assert_eq!(summary.ready_count, 1);
    assert_eq!(summary.decode_count, 1);
    assert!(summary.record_count <= editor_core::ASSET_THUMBNAIL_MAX_ITEMS);
    let visible_ids = app.visible_asset_thumbnail_ids();
    assert_eq!(visible_ids.len(), 1);
    let payloads = app.asset_thumbnail_payloads_for_ids(&visible_ids);
    assert_eq!(payloads.len(), 1);
    assert!(payloads[0]
        .rgba8
        .chunks_exact(4)
        .any(|pixel| pixel[3] > 0 && pixel[..3] != [0, 0, 0]));
    assert!(app
        .latest_draw_list()
        .commands
        .iter()
        .any(|command| matches!(
            command.unclipped(),
            editor_ui_renderer::DrawCommand::ImageTextureSlot {
                texture_id: Some(_),
                ..
            }
        )));
}

#[test]
fn native_editor_application_input_mapping_capture_consumes_key_before_shortcuts() {
    let root = unique_project_launcher_temp_dir();
    let path = "Input/input.default.json";
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    app.dispatch_command(UiCommand {
        command_id: "create_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "input-mapping-create-project".to_string(),
        payload: UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "InputCaptureGame".to_string(),
        },
    });
    app.dispatch_command(UiCommand {
        command_id: "create_default_input_mapping".to_string(),
        source: UiCommandSource::Unknown,
        request_id: "input-mapping-create".to_string(),
        payload: UiCommandPayload::CreateDefaultInputMapping {
            path: path.to_string(),
        },
    });
    let binding_id = add_input_capture_fixture(&mut app, path, "capture");
    app.dispatch_command(UiCommand {
        command_id: "begin_input_binding_capture".to_string(),
        source: UiCommandSource::Unknown,
        request_id: "input-mapping-capture".to_string(),
        payload: UiCommandPayload::BeginInputBindingCapture {
            path: path.to_string(),
            binding_id: binding_id.clone(),
        },
    });

    let report = app.handle_input_event(EditorInputEvent::KeyDown {
        key: "K".to_string(),
    });
    let input = &app.latest_model().input_mapping_authoring;

    assert_eq!(
        report.last_command_id.as_deref(),
        Some("commit_captured_input_binding")
    );
    assert!(input.dirty);
    assert!(input.capture_binding_id.is_none());
    assert_eq!(
        input
            .bindings
            .iter()
            .find(|binding| binding.binding_id == binding_id)
            .unwrap()
            .device_path,
        "keyboard/K"
    );
}

#[test]
fn native_editor_application_input_mapping_capture_cancels_on_escape() {
    let root = unique_project_launcher_temp_dir();
    let path = "Input/input.default.json";
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    app.dispatch_command(UiCommand {
        command_id: "create_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "input-mapping-cancel-project".to_string(),
        payload: UiCommandPayload::CreateProject {
            path: root.display().to_string(),
            name: "InputCancelGame".to_string(),
        },
    });
    app.dispatch_command(UiCommand {
        command_id: "create_default_input_mapping".to_string(),
        source: UiCommandSource::Unknown,
        request_id: "input-mapping-cancel-create".to_string(),
        payload: UiCommandPayload::CreateDefaultInputMapping {
            path: path.to_string(),
        },
    });
    let binding_id = add_input_capture_fixture(&mut app, path, "cancel");
    let original_path = app
        .latest_model()
        .input_mapping_authoring
        .bindings
        .iter()
        .find(|binding| binding.binding_id == binding_id)
        .expect("capture binding")
        .device_path
        .clone();
    app.dispatch_command(UiCommand {
        command_id: "begin_input_binding_capture".to_string(),
        source: UiCommandSource::Unknown,
        request_id: "input-mapping-cancel-capture".to_string(),
        payload: UiCommandPayload::BeginInputBindingCapture {
            path: path.to_string(),
            binding_id: binding_id.clone(),
        },
    });

    let report = app.handle_input_event(EditorInputEvent::KeyDown {
        key: "Escape".to_string(),
    });
    let input = &app.latest_model().input_mapping_authoring;

    assert_eq!(
        report.last_command_id.as_deref(),
        Some("cancel_input_binding_capture")
    );
    assert!(input.capture_binding_id.is_none());
    assert_eq!(
        input
            .bindings
            .iter()
            .find(|binding| binding.binding_id == binding_id)
            .unwrap()
            .device_path,
        original_path
    );
}

fn add_input_capture_fixture(
    app: &mut NativeEditorApplication,
    path: &str,
    request_prefix: &str,
) -> String {
    app.dispatch_command(UiCommand {
        command_id: "add_input_context".to_string(),
        source: UiCommandSource::Unknown,
        request_id: format!("{request_prefix}-context"),
        payload: UiCommandPayload::AddInputContext {
            path: path.to_string(),
            context_id: "gameplay".to_string(),
            priority: 0,
        },
    });
    app.dispatch_command(UiCommand {
        command_id: "add_input_action".to_string(),
        source: UiCommandSource::Unknown,
        request_id: format!("{request_prefix}-action"),
        payload: UiCommandPayload::AddInputAction {
            path: path.to_string(),
            action_id: "action.capture".to_string(),
            value_type: editor_ui_model::InputActionValueKind::Button,
        },
    });
    app.dispatch_command(UiCommand {
        command_id: "add_input_binding".to_string(),
        source: UiCommandSource::Unknown,
        request_id: format!("{request_prefix}-binding"),
        payload: UiCommandPayload::AddInputBinding {
            path: path.to_string(),
            context_id: "gameplay".to_string(),
            action_id: "action.capture".to_string(),
            device_path: "keyboard/T".to_string(),
        },
    });
    app.latest_model().input_mapping_authoring.bindings[0]
        .binding_id
        .clone()
}

#[test]
fn native_editor_application_loads_recent_projects_from_store() {
    let project_root = write_editor_project_fixture_for_shell();
    let store_path = unique_project_launcher_temp_dir().join("recent.json");
    let document = editor_core::ProjectRecentProjectsDocument::new(vec![
        editor_ui_model::RecentProjectEntry {
            name: "StoredProject".to_string(),
            path: project_root.display().to_string(),
            engine_version: "0.0.3".to_string(),
            last_opened_at: Some("1".to_string()),
            last_modified_at: Some("1".to_string()),
            valid: true,
            status: "ready".to_string(),
        },
    ]);
    editor_core::ProjectRecentStore::save(&store_path, &document).expect("save recent");

    let app = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        EditorSession::new(),
        ProjectManagerController::with_recent_store_path(store_path),
        Box::<HeadlessFolderDialogBackend>::default(),
    );

    assert_eq!(app.latest_model().mode, EditorUiMode::ProjectLauncher);
    assert_eq!(app.latest_model().project_launcher.recent_projects.len(), 1);
    assert_eq!(
        app.latest_model().project_launcher.recent_projects[0].status,
        "ready"
    );
}

#[cfg(windows)]
#[test]
fn native_editor_migrates_duplicate_windows_recent_project_paths_once() {
    let project_root = write_editor_project_fixture_for_shell();
    let display_path = project_root.display().to_string();
    let verbatim_path = std::fs::canonicalize(&project_root)
        .expect("canonicalize project fixture")
        .display()
        .to_string();
    assert!(verbatim_path.starts_with(r"\\?\"));
    let store_path = unique_project_launcher_temp_dir().join("recent.json");
    let entry = |path: String, last_opened_at: &str| editor_ui_model::RecentProjectEntry {
        name: "StoredProject".to_string(),
        path,
        engine_version: "0.0.3".to_string(),
        last_opened_at: Some(last_opened_at.to_string()),
        last_modified_at: Some("1".to_string()),
        valid: true,
        status: "ready".to_string(),
    };
    let document = editor_core::ProjectRecentProjectsDocument::new(vec![
        entry(display_path, "10"),
        entry(verbatim_path, "20"),
    ]);
    editor_core::ProjectRecentStore::save(&store_path, &document).expect("save duplicate recent");

    let app = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        EditorSession::new(),
        ProjectManagerController::with_recent_store_path(store_path.clone()),
        Box::<HeadlessFolderDialogBackend>::default(),
    );

    assert_eq!(app.latest_model().project_launcher.recent_projects.len(), 1);
    let migrated = editor_core::ProjectRecentStore::load(&store_path).expect("load migration");
    assert_eq!(migrated.recent_projects.len(), 1);
    assert!(!migrated.recent_projects[0].path.starts_with(r"\\?\"));
    assert_eq!(
        migrated.recent_projects[0].last_opened_at.as_deref(),
        Some("20")
    );
}

#[test]
fn default_native_editor_recent_store_path_points_to_recent_json() {
    let store_path = default_native_editor_recent_store_path();

    assert_eq!(
        store_path.file_name().and_then(|name| name.to_str()),
        Some("editor_recent_projects.json")
    );
    assert!(store_path.components().any(|component| component
        .as_os_str()
        .to_string_lossy()
        .contains("AI First Engine")
        || component
            .as_os_str()
            .to_string_lossy()
            .contains("ai-first-engine")));
}

#[test]
fn project_dialog_request_roundtrips_explicit_initial_directory() {
    let initial_directory = unique_project_launcher_temp_dir();
    let value = serde_json::json!({
        "purpose": "CreateProject",
        "title": "Create Project",
        "initial_directory": initial_directory,
    });

    let request: ProjectFolderDialogRequest =
        serde_json::from_value(value.clone()).expect("deserialize dialog request");
    let roundtrip = serde_json::to_value(request).expect("serialize dialog request");

    assert_eq!(roundtrip, value);
}

#[test]
fn configured_initial_directory_reaches_all_project_folder_requests() {
    use std::cell::RefCell;
    use std::rc::Rc;

    struct RecordingDialogBackend {
        requests: Rc<RefCell<Vec<ProjectFolderDialogRequest>>>,
    }

    impl ProjectLocationDialogService for RecordingDialogBackend {
        fn pick_folder(
            &mut self,
            request: ProjectFolderDialogRequest,
        ) -> ProjectFolderDialogResponse {
            self.requests.borrow_mut().push(request);
            ProjectFolderDialogResponse::Cancelled
        }
    }

    fn click_hit_region(app: &mut NativeEditorApplication, region_id: &str) {
        app.frame(1280.0, 720.0);
        let region = app
            .latest_draw_list()
            .hit_regions
            .iter()
            .find(|region| region.id == region_id)
            .unwrap_or_else(|| panic!("missing hit region {region_id}"))
            .clone();
        app.handle_input_event(EditorInputEvent::PointerDown {
            x: region.rect.x + 1.0,
            y: region.rect.y + 1.0,
            button: PointerButton::Primary,
        });
        app.handle_input_event(EditorInputEvent::PointerUp {
            x: region.rect.x + 1.0,
            y: region.rect.y + 1.0,
            button: PointerButton::Primary,
        });
    }

    let initial_directory = unique_project_launcher_temp_dir();
    std::fs::create_dir_all(&initial_directory).expect("create dialog initial directory");
    let requests = Rc::new(RefCell::new(Vec::new()));
    let mut launcher_app =
        NativeEditorApplication::with_project_manager_and_dialog_initial_directory(
            NativeEditorWindowConfig::default(),
            EditorSession::new(),
            ProjectManagerController::default(),
            Box::new(RecordingDialogBackend {
                requests: requests.clone(),
            }),
            initial_directory.clone(),
        );
    click_hit_region(&mut launcher_app, "hit.project_launcher.open_project");
    click_hit_region(&mut launcher_app, "hit.project_launcher.create_project");

    let project_root = write_editor_project_fixture_for_shell();
    let mut workspace_app =
        NativeEditorApplication::with_project_manager_and_dialog_initial_directory(
            NativeEditorWindowConfig::default(),
            opened_editor_project_session(&project_root),
            ProjectManagerController::default(),
            Box::new(RecordingDialogBackend {
                requests: requests.clone(),
            }),
            initial_directory.clone(),
        );
    click_hit_region(&mut workspace_app, "hit.toolbar.open_runtime_package");

    let requests = requests.borrow();
    assert_eq!(requests.len(), 3);
    let initial_directories = requests
        .iter()
        .map(|request| {
            serde_json::to_value(request)
                .expect("dialog request should serialize")
                .get("initial_directory")
                .and_then(serde_json::Value::as_str)
                .map(std::path::PathBuf::from)
                .expect("every project folder request must declare an initial directory")
        })
        .collect::<Vec<_>>();
    assert!(initial_directories
        .iter()
        .all(|path| path == &initial_directory));
}

#[test]
fn native_editor_application_distinguishes_cancelled_and_unavailable_project_dialog() {
    struct FixedDialogBackend(ProjectFolderDialogResponse);

    impl ProjectLocationDialogService for FixedDialogBackend {
        fn pick_folder(
            &mut self,
            _request: ProjectFolderDialogRequest,
        ) -> ProjectFolderDialogResponse {
            self.0.clone()
        }
    }

    fn click_hit_region(
        app: &mut NativeEditorApplication,
        region_id: &str,
    ) -> NativeEditorApplicationReport {
        app.frame(1280.0, 720.0);
        let region = app
            .latest_draw_list()
            .hit_regions
            .iter()
            .find(|region| region.id == region_id)
            .unwrap_or_else(|| panic!("missing hit region {region_id}"))
            .clone();
        app.handle_input_event(EditorInputEvent::PointerDown {
            x: region.rect.x + 1.0,
            y: region.rect.y + 1.0,
            button: PointerButton::Primary,
        });
        app.handle_input_event(EditorInputEvent::PointerUp {
            x: region.rect.x + 1.0,
            y: region.rect.y + 1.0,
            button: PointerButton::Primary,
        })
    }

    fn assert_unavailable(
        mut app: NativeEditorApplication,
        region_id: &str,
        command_id: &str,
        source: UiCommandSource,
    ) {
        let diagnostic = "project.dialog.windows_set_initial_folder_failed: 0x80070005";
        let report = click_hit_region(&mut app, region_id);

        assert_eq!(report.last_command_id.as_deref(), Some(command_id));
        assert_eq!(report.last_command_status, Some(CommandStatus::Rejected));
        let feedback = report.last_feedback.expect("unavailable feedback");
        assert_eq!(feedback.command_id, command_id);
        assert_eq!(
            feedback.status,
            editor_ui_model::EditorCommandFeedbackStatus::Rejected
        );
        assert_eq!(feedback.message, diagnostic);
        assert_eq!(feedback.reason.as_deref(), Some(diagnostic));
        assert_eq!(feedback.source, source);
        assert_eq!(
            app.latest_model().interaction_feedback,
            Some(feedback.clone())
        );
        assert_eq!(
            app.project_manager().last_dialog_response,
            Some(ProjectFolderDialogResponse::Unavailable {
                diagnostic: diagnostic.to_string(),
            })
        );
    }

    let mut cancelled = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        EditorSession::new(),
        ProjectManagerController::default(),
        Box::new(FixedDialogBackend(ProjectFolderDialogResponse::Cancelled)),
    );
    let cancelled_report = click_hit_region(&mut cancelled, "hit.project_launcher.open_project");
    assert_eq!(cancelled_report.last_command_status, None);
    assert_eq!(cancelled_report.last_feedback, None);
    assert_eq!(
        cancelled.project_manager().last_dialog_response,
        Some(ProjectFolderDialogResponse::Cancelled)
    );

    let diagnostic = "project.dialog.windows_set_initial_folder_failed: 0x80070005";
    for (region_id, command_id) in [
        ("hit.project_launcher.open_project", "open_project"),
        ("hit.project_launcher.create_project", "create_project"),
    ] {
        assert_unavailable(
            NativeEditorApplication::with_project_manager(
                NativeEditorWindowConfig::default(),
                EditorSession::new(),
                ProjectManagerController::default(),
                Box::new(FixedDialogBackend(
                    ProjectFolderDialogResponse::Unavailable {
                        diagnostic: diagnostic.to_string(),
                    },
                )),
            ),
            region_id,
            command_id,
            UiCommandSource::ProjectLauncher,
        );
    }

    let project_root = write_editor_project_fixture_for_shell();
    assert_unavailable(
        NativeEditorApplication::with_project_manager(
            NativeEditorWindowConfig::default(),
            opened_editor_project_session(&project_root),
            ProjectManagerController::default(),
            Box::new(FixedDialogBackend(
                ProjectFolderDialogResponse::Unavailable {
                    diagnostic: diagnostic.to_string(),
                },
            )),
        ),
        "hit.toolbar.open_runtime_package",
        "open_runtime_package",
        UiCommandSource::Toolbar,
    );
}

#[test]
fn native_editor_application_click_create_uses_headless_dialog_and_persists_recent() {
    let fixture_root = unique_project_launcher_temp_dir();
    std::fs::create_dir_all(&fixture_root).expect("fixture owner root");
    let project_root = fixture_root.join("DialogCreated");
    let store_path = fixture_root.join("recent.json");
    let mut app = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        EditorSession::new(),
        ProjectManagerController::with_recent_store_path(store_path.clone()),
        Box::new(HeadlessFolderDialogBackend::with_create_project_path(
            project_root.display().to_string(),
        )),
    );
    app.frame(1280.0, 720.0);
    let region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.project_launcher.create_project")
        .expect("create project hit")
        .clone();

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    let report = app.handle_input_event(EditorInputEvent::PointerUp {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert_eq!(report.last_command_status, Some(CommandStatus::Committed));
    assert_eq!(app.latest_model().mode, EditorUiMode::AuthoringWorkspace);
    assert!(project_root.join("project.aife.json").exists());
    let loaded = editor_core::ProjectRecentStore::load(&store_path).expect("recent saved");
    assert_eq!(loaded.recent_projects.len(), 1);
    assert_eq!(loaded.recent_projects[0].name, "DialogCreated");
}

#[test]
fn native_editor_application_click_open_uses_headless_dialog() {
    let project_root = write_editor_project_fixture_for_shell();
    let mut app = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        EditorSession::new(),
        ProjectManagerController::default(),
        Box::new(HeadlessFolderDialogBackend::with_open_project_path(
            project_root.display().to_string(),
        )),
    );
    app.frame(1280.0, 720.0);
    let region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.project_launcher.open_project")
        .expect("open project hit")
        .clone();

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    let report = app.handle_input_event(EditorInputEvent::PointerUp {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert_eq!(report.last_command_status, None);
    assert!(app.latest_model().project_launcher.activity.is_some());
    for _ in 0..200 {
        let report = app.frame(1280.0, 720.0);
        if report.last_command_status == Some(CommandStatus::Committed) {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(
        app.report().last_command_status,
        Some(CommandStatus::Committed)
    );
    assert_eq!(app.latest_model().mode, EditorUiMode::AuthoringWorkspace);
    assert_eq!(
        app.project_manager().last_dialog_response,
        Some(ProjectFolderDialogResponse::Selected {
            path: project_root.display().to_string()
        })
    );
}

#[test]
fn native_editor_application_click_open_runtime_package_uses_headless_dialog() {
    let root = unique_project_launcher_temp_dir();
    let package_dir = write_runtime_package_fixture_for_shell(&root, "runtime-package");
    let project_root = write_editor_project_fixture_for_shell();
    let mut app = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        opened_editor_project_session(&project_root),
        ProjectManagerController::default(),
        Box::new(HeadlessFolderDialogBackend::with_open_runtime_package_path(
            package_dir.display().to_string(),
        )),
    );
    app.frame(1280.0, 720.0);
    let region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.toolbar.open_runtime_package")
        .expect("open runtime package hit")
        .clone();

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    let report = app.handle_input_event(EditorInputEvent::PointerUp {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert_eq!(report.last_command_status, Some(CommandStatus::Committed));
    assert_eq!(
        report.last_command_id.as_deref(),
        Some("open_runtime_package")
    );
    assert!(app.latest_model().active_runtime_package.is_some());
    assert_eq!(
        app.project_manager().last_dialog_response,
        Some(ProjectFolderDialogResponse::Selected {
            path: package_dir.display().to_string()
        })
    );
}

#[test]
fn native_editor_application_layout_uses_workspace_snapshot_panel_rects() {
    let app = NativeEditorApplication::new(NativeEditorWindowConfig::default());

    let snapshot = app
        .workspace_docking()
        .snapshot(editor_ui_renderer::editor_workspace_rect(1280.0, 720.0));

    assert!(snapshot.panel_rects.contains_key("viewport"));
    assert!(snapshot.panel_rects.contains_key("ai_panel"));
    assert!(snapshot
        .panel_rects
        .values()
        .all(|rect| rect.width >= 0.0 && rect.height >= 0.0));
}

#[test]
fn dock_tab_click_changes_only_editor_dock_state_and_rebuilds_visible_panel() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    let open = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: project_root.display().to_string(),
        },
    ));
    assert_eq!(open.status, CommandStatus::Committed);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let tab = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| matches!(&region.target, HitTarget::DockTab { panel_id } if panel_id == "ai_panel"))
        .expect("AI dock tab")
        .clone();
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: tab.rect.x + tab.rect.width * 0.5,
        y: tab.rect.y + tab.rect.height * 0.5,
        button: PointerButton::Primary,
    });
    assert_eq!(
        app.workspace_docking()
            .active_panel_id("workspace/bottom")
            .map(|panel_id| panel_id.as_str()),
        Some("ai_panel")
    );
    app.frame(1280.0, 720.0);
    assert!(app
        .latest_draw_list()
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::AiPromptField)));
}

#[test]
fn native_application_reconciles_retained_tree_across_frames() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    app.frame(1280.0, 720.0);
    let report = app.retained_ui_renderer().last_reconcile();
    assert!(report.reused > 0);
    assert_eq!(report.created, 0);
    assert!(app.retained_ui_renderer().tree().is_some());
}

#[test]
fn toolbar_overflow_click_opens_editor_local_command_popup() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    let open = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: project_root.display().to_string(),
        },
    ));
    assert_eq!(open.status, CommandStatus::Committed);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(320.0, 480.0);
    let overflow = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::ToolbarOverflow))
        .expect("toolbar overflow")
        .clone();

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: overflow.rect.x + 1.0,
        y: overflow.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::PointerUp {
        x: overflow.rect.x + 1.0,
        y: overflow.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    assert!(app.toolbar_overflow_open());
    app.frame(320.0, 480.0);
    assert!(app.latest_draw_list().hit_regions.iter().any(|region| {
        region.id.starts_with("hit.toolbar.overflow.")
            && matches!(region.target, HitTarget::ToolbarCommand { .. })
    }));
    let barrier = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.toolbar.overflow.barrier")
        .expect("popup barrier")
        .clone();
    app.configure_game_view_input_viewport_for_test(barrier.rect, 320, 480);
    let last_command = app.report().last_command_id;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: barrier.rect.x + 2.0,
        y: barrier.rect.y + barrier.rect.height - 2.0,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::PointerUp {
        x: barrier.rect.x + 2.0,
        y: barrier.rect.y + barrier.rect.height - 2.0,
        button: PointerButton::Primary,
    });
    assert_eq!(
        app.last_viewport_input_route()
            .expect("overlay route evidence")
            .route_kind,
        ViewportInputRouteKind::UiConsumed
    );
    assert!(!app.toolbar_overflow_open());
    assert_eq!(app.report().last_command_id, last_command);
}

#[test]
fn recent_open_keeps_specialized_composition_play_actionable_in_720_toolbar_overflow() {
    let project_root = write_project_rust_fixture_for_preparation();
    let state_root = unique_project_launcher_temp_dir();
    let rust_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf();
    let session = specialized_composition_session_for_project(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session)
            .with_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
                trust_module: editor_core::ProjectRuntimeTrustModule::open(&state_root).unwrap(),
                engine_sdk_root: rust_root,
                editor_build_identity: format!(
                    "sha256:{}",
                    project_runtime_abi::project_runtime_abi_digest_hex()
                ),
            });

    assert!(app
        .dispatch_project_launcher_command_or_dispatch(UiCommand {
            command_id: "select_recent_project".to_string(),
            source: UiCommandSource::ProjectLauncher,
            request_id: "recent-specialized-composition".to_string(),
            payload: UiCommandPayload::SelectRecentProject {
                path: project_root.display().to_string(),
            },
        })
        .is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.frame(360.0, 640.0);
        if app.latest_model().mode == EditorUiMode::AuthoringWorkspace
            && app.latest_model().project_launcher.activity.is_none()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "recent-open did not reach authoring: activity={:?}",
            app.latest_model().project_launcher.activity
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert!(app
        .session()
        .project_editor_composition_identity()
        .is_some());
    assert!(app.latest_model().project_runtime_trust_prompt.is_none());
    let play = app
        .latest_model()
        .toolbar
        .commands
        .iter()
        .find(|command| command.command_id == "play")
        .expect("Play command");
    assert!(play.enabled, "Play disabled: {:?}", play.reason_disabled);

    let overflow = app
        .retained_ui_renderer()
        .tree()
        .and_then(|tree| {
            tree.node(
                &editor_ui_renderer::WidgetId::semantic("editor/shell/toolbar/overflow").unwrap(),
            )
        })
        .expect("720 portrait toolbar overflow")
        .logical_rect;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: overflow.x + overflow.width * 0.5,
        y: overflow.y + overflow.height * 0.5,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::PointerUp {
        x: overflow.x + overflow.width * 0.5,
        y: overflow.y + overflow.height * 0.5,
        button: PointerButton::Primary,
    });
    app.frame(360.0, 640.0);
    let overflow_play = app
        .retained_ui_renderer()
        .tree()
        .and_then(|tree| {
            tree.node(
                &editor_ui_renderer::WidgetId::semantic("editor/shell/toolbar/overflow/play")
                    .unwrap(),
            )
        })
        .expect("overflow Play command");
    assert_eq!(
        overflow_play.visibility,
        editor_ui_renderer::WidgetVisibility::Visible
    );
    assert!(
        overflow_play.enabled,
        "overflow Play disabled: {:?}",
        overflow_play
            .binding
            .as_ref()
            .and_then(|binding| binding.reason_disabled.as_deref())
    );
    let play_rect = overflow_play.logical_rect;
    let play_center = editor_ui_renderer::UiPoint {
        x: play_rect.x + play_rect.width * 0.5,
        y: play_rect.y + play_rect.height * 0.5,
    };
    let picked = editor_ui_renderer::pick_widget(
        app.retained_ui_renderer().tree().unwrap(),
        play_center,
        None,
    )
    .expect("overflow Play center pick");
    assert_eq!(picked.target.as_str(), "editor/shell/toolbar/overflow/play");
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: play_center.x,
        y: play_center.y,
        button: PointerButton::Primary,
    });
    assert_eq!(
        app.widget_interaction_snapshot()
            .captured_widget_id
            .as_ref()
            .map(editor_ui_renderer::WidgetId::as_str),
        Some("editor/shell/toolbar/overflow/play")
    );
    let activated = app.handle_input_event(EditorInputEvent::PointerUp {
        x: play_center.x,
        y: play_center.y,
        button: PointerButton::Primary,
    });
    assert_eq!(activated.last_command_id.as_deref(), Some("play"));

    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_dir_all(state_root);
}

#[test]
fn native_editor_application_click_hierarchy_routes_through_session() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    let open = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: project_root.display().to_string(),
        },
    ));
    assert_eq!(open.status, CommandStatus::Committed);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let before_revision = app.latest_model().revision;
    let region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.hierarchy.entity-player")
        .expect("hierarchy hit region")
        .clone();

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    let report = app.handle_input_event(EditorInputEvent::PointerUp {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert_eq!(report.last_command_status, Some(CommandStatus::Committed));
    assert_eq!(
        report.last_command_id.as_deref(),
        Some("select_scene_entity")
    );
    assert_eq!(
        report.workspace.last_command_id.as_deref(),
        Some("select_scene_entity")
    );
    assert_eq!(
        report.workspace.primary_entity_id.as_deref(),
        Some("entity-player")
    );
    assert_eq!(
        app.latest_model().inspector.selected_entity_id.as_deref(),
        Some("entity-player")
    );
    assert!(app.latest_model().revision > before_revision);
    assert_eq!(
        app.transaction_service().last_status,
        Some(CommandStatus::Committed)
    );
}

#[test]
fn native_editor_application_toolbar_play_runs_for_open_project() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.toolbar.play")
        .expect("play hit region")
        .clone();

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    let report = app.handle_input_event(EditorInputEvent::PointerUp {
        x: region.rect.x + 1.0,
        y: region.rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert_eq!(report.last_command_id.as_deref(), Some("play"));
    assert_eq!(report.last_command_status, Some(CommandStatus::Pending));
    if let Some(feedback) = report.last_feedback {
        assert_ne!(feedback.status, EditorCommandFeedbackStatus::Disabled);
    }
    let completed = pump_editor_play_until_terminal(&mut app);
    assert_eq!(completed.last_command_id.as_deref(), Some("play"));
    assert_eq!(
        completed.last_command_status,
        Some(CommandStatus::Committed)
    );
    assert_eq!(
        app.session()
            .last_editor_preview_package_report()
            .expect("Play preparation report")
            .player_artifact_status,
        "not_required_in_process"
    );
}

#[test]
fn release_inside_toolbar_waits_for_up_and_cancels_outside_without_dispatch() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| region.id == "hit.toolbar.play")
        .expect("play toolbar control")
        .clone();
    let x = region.rect.x + region.rect.width * 0.5;
    let y = region.rect.y + region.rect.height * 0.5;
    let before = app.report().last_command_id;

    let down = app.handle_input_event(EditorInputEvent::PointerDown {
        x,
        y,
        button: PointerButton::Primary,
    });
    assert_eq!(down.last_command_id, before);
    assert_eq!(down.pressed_hit_id.as_deref(), Some(region.id.as_str()));
    app.handle_input_event(EditorInputEvent::PointerMove {
        x: region.rect.x + region.rect.width + 40.0,
        y,
    });
    let cancelled = app.handle_input_event(EditorInputEvent::PointerUp {
        x: region.rect.x + region.rect.width + 40.0,
        y,
        button: PointerButton::Primary,
    });
    assert_eq!(cancelled.last_command_id, before);

    app.handle_input_event(EditorInputEvent::PointerDown {
        x,
        y,
        button: PointerButton::Primary,
    });
    let activated = app.handle_input_event(EditorInputEvent::PointerUp {
        x,
        y,
        button: PointerButton::Primary,
    });
    assert_eq!(activated.last_command_id.as_deref(), Some("play"));
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn native_editor_application_shortcut_routes_to_transaction_service() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::SetSceneTransform {
                    entity_id: "entity-player".to_string(),
                    local_position: Some(editor_ui_model::Vec3 {
                        x: 4.0,
                        y: 0.0,
                        z: 0.0,
                    }),
                    local_rotation: None,
                    local_scale: None,
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);

    let report = app.handle_shortcut("Ctrl+Z");

    assert_eq!(report.last_command_id.as_deref(), Some("undo_scene_edit"));
    assert_eq!(report.last_command_status, Some(CommandStatus::Committed));
    assert_eq!(app.transaction_service().committed_count, 1);
}

#[test]
fn native_editor_application_text_input_is_owned_by_focus_system() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::SelectSceneEntity {
                    entity_id: "entity-player".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let field = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::InspectorField { .. }))
        .expect("inspector field")
        .clone();
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: field.rect.x + 1.0,
        y: field.rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert!(app.handle_text_input("A"));
    assert!(app.report().redraw_requested);
}

#[test]
fn native_editor_application_inspector_field_focuses_property_buffer() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::SelectSceneEntity {
                    entity_id: "entity-player".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let field_region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| matches!(region.target, HitTarget::InspectorField { .. }))
        .expect("inspector field hit region")
        .clone();

    let report = app.handle_input_event(EditorInputEvent::PointerDown {
        x: field_region.rect.x + 1.0,
        y: field_region.rect.y + 1.0,
        button: PointerButton::Primary,
    });

    assert_eq!(
        report
            .workspace
            .property_editing
            .focused_property_path
            .as_deref(),
        Some("transform.localPosition")
    );
    assert!(report.workspace.property_editing.editing);
}

#[test]
fn native_editor_application_commits_focused_property_edit_through_transaction() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::SelectSceneEntity {
                    entity_id: "entity-player".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let field_region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::InspectorField { field_id }
                    if field_id == "transform.localPosition"
            )
        })
        .expect("localPosition field")
        .clone();
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: field_region.rect.x + 1.0,
        y: field_region.rect.y + 1.0,
        button: PointerButton::Primary,
    });
    assert!(app.replace_focused_property_text("7,8,9"));

    let result = app
        .commit_focused_property_edit()
        .expect("property commit should produce command");

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(result.command_id, "set_scene_transform");
    let position = app.latest_model().viewport.renderables[0].local_position;
    assert_eq!(
        position,
        editor_ui_model::Vec3 {
            x: 7.0,
            y: 8.0,
            z: 9.0,
        }
    );
}

#[test]
fn native_editor_application_ai_proposal_accept_uses_command_system() {
    let mut session = EditorSession::new();
    let project_root = write_editor_project_fixture_for_shell();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::SelectSceneEntity {
                    entity_id: "entity-player".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::AiSubmitPrompt {
                    prompt: "rename selected to hero".to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let proposal_id = session.build_ui_model().ai_panel.proposed_commands[0]
        .proposal_id
        .clone();
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);

    let result = app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::AiAcceptProposedCommand { proposal_id },
    ));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(app.latest_model().hierarchy.roots[0].label, "hero");
    assert_eq!(app.transaction_service().committed_count, 1);
    assert_eq!(
        app.authoring_workspace()
            .report()
            .last_command_id
            .as_deref(),
        Some("rename_scene_entity")
    );
}

#[test]
fn native_editor_window_headless_frame_draws_ui_model() {
    let model = fixture_model();
    let mut app = HeadlessNativeEditorWindowApp::new(NativeEditorWindowConfig::default());

    let report = app.frame(&model);
    let draw_list = app.latest_draw_list().expect("latest draw list");

    assert_eq!(report.shared_gpu_context_status, "headless_mock");
    assert_eq!(report.shared_gpu_backend, "headless");
    assert_eq!(report.viewport_texture_registry_count, 0);
    assert!(draw_list.commands.iter().any(|command| matches!(
        command,
        editor_ui_renderer::DrawCommand::ViewportTextureSlot { .. }
    )));
}

#[test]
fn native_editor_window_headless_click_toolbar_routes_command() {
    headless_native_editor_window_app_click_routes_to_ui_command();
}

#[test]
fn native_editor_window_headless_resize_then_frame_presents_report() {
    let model = fixture_model();
    let mut app = HeadlessNativeEditorWindowApp::new(NativeEditorWindowConfig::default());

    app.resize(900, 700);
    let report = app.frame(&model);

    assert_eq!(report.resize_count, 1);
    assert_eq!(report.present_status, "presented");
}

#[test]
fn native_editor_window_headless_close_exits_cleanly() {
    let mut app = HeadlessNativeEditorWindowApp::new(NativeEditorWindowConfig::default());

    app.close();

    assert!(app.report().close_requested);
}

#[test]
fn llm_shutdown_native_application_joins_active_request() {
    let mut session = EditorSession::new();
    session.execute_command(UiCommand {
        command_id: "start-llm-for-shutdown".to_string(),
        source: UiCommandSource::Test,
        request_id: "native-llm-shutdown".to_string(),
        payload: UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create shutdown entity".to_string(),
        },
    });
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);

    let receipt = app.shutdown_llm();

    assert_ne!(
        receipt.state,
        editor_core::LlmLifecycleState::ShutdownJoinTimedOut
    );
    assert_eq!(receipt.active_task_count, 0);
    assert_eq!(receipt.reaper_count, 0);
    assert!(receipt.diagnostic.is_none());
}

fn activate_bottom_panel(
    app: &mut NativeEditorApplication,
    panel_id: &str,
    width: f32,
    height: f32,
) {
    app.frame(width, height);
    let tab = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| {
            matches!(&region.target, HitTarget::DockTab { panel_id: target } if target == panel_id)
        })
        .unwrap_or_else(|| panic!("missing dock tab for {panel_id}"))
        .clone();
    let x = tab.rect.x + tab.rect.width * 0.5;
    let y = tab.rect.y + tab.rect.height * 0.5;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x,
        y,
        button: PointerButton::Primary,
    });
    app.handle_input_event(EditorInputEvent::PointerUp {
        x,
        y,
        button: PointerButton::Primary,
    });
    app.frame(width, height);
}

#[test]
fn native_editor_application_routes_game_view_content_input_without_ui_double_dispatch() {
    let project_root = write_editor_project_fixture_for_shell();
    let mut session = opened_editor_project_session(&project_root);
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(UiCommandPayload::Play))
            .status,
        CommandStatus::Committed
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let rect = app
        .latest_draw_list()
        .commands
        .iter()
        .find_map(|command| match command.unclipped() {
            editor_ui_renderer::DrawCommand::ViewportTextureSlot { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("workspace viewport texture content rect");
    app.configure_game_view_input_viewport_for_test(rect, 1280, 720);
    let before_command = app.report().last_command_id;
    let before_runtime_frame = app
        .session()
        .last_game_view_runtime_frame()
        .expect("Play must retain a runtime frame before ordinary input")
        .frame_index;

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: rect.x + rect.width * 0.5,
        y: rect.y + rect.height * 0.5,
        button: PointerButton::Primary,
    });

    let route = app
        .last_viewport_input_route()
        .expect("production app must preserve route evidence");
    assert_eq!(route.route_kind, ViewportInputRouteKind::RuntimeInputFrame);
    assert_eq!(
        route
            .runtime_input_frame
            .as_ref()
            .and_then(|frame| frame.pointer_position),
        Some(engine_runtime::input_action::PointerPosition { x: 640.0, y: 360.0 })
    );
    assert_eq!(app.report().last_command_id, before_command);
    assert_eq!(
        app.session()
            .last_game_view_runtime_frame()
            .expect("ordinary input must retain the current runtime frame")
            .frame_index,
        before_runtime_frame,
        "ordinary GameView input must not advance the runtime frame"
    );
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn native_editor_application_routes_r7_narrow_portrait_game_view_input() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(720.0, 1280.0);
    let display_rect = rect(116.99999, 224.83333, 237.00002, 421.33334);
    app.configure_game_view_input_presentation_for_test(
        display_rect,
        720,
        1280,
        vec![
            engine_runtime::game_view_presentation::CanvasReferenceFact::new(
                "battle-canvas",
                1080,
                1920,
            ),
        ],
    );

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: 215.96945,
        y: 615.225,
        button: PointerButton::Primary,
    });

    app.frame(720.0, 1280.0);
    app.handle_input_event(EditorInputEvent::PointerUp {
        x: 215.96945,
        y: 615.225,
        button: PointerButton::Primary,
    });

    let route = app
        .last_viewport_input_route()
        .expect("R7 GameView pointer release must preserve route evidence");
    assert_eq!(route.route_kind, ViewportInputRouteKind::RuntimeInputFrame);
    let pointer = route
        .runtime_input_frame
        .as_ref()
        .and_then(|frame| frame.pointer_position)
        .expect("R7 GameView pointer must reach TargetSpace");
    assert!((pointer.x - 300.66666).abs() < 0.001, "x={}", pointer.x);
    assert!((pointer.y - 1186.0).abs() < 0.001, "y={}", pointer.y);
    let _ = std::fs::remove_dir_all(project_root);
}
