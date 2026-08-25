use super::*;
use editor_core::{ProjectRuntimeTrustInspection, ProjectRuntimeTrustStatus};
use std::fs;
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
    let root = std::env::temp_dir().join(format!("aife-runtime-trust-ui-{stamp}"));
    fs::create_dir_all(&root).unwrap();
    TempRoot(root)
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

fn pump_until_trust_prompt(
    app: &mut NativeEditorApplication,
) -> editor_ui_model::ProjectRuntimeTrustPromptModel {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.frame(1280.0, 720.0);
        if let Some(prompt) = app.latest_model().project_runtime_trust_prompt.clone() {
            return prompt;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "ProjectRust trust prompt did not appear: activity={:?} preparation={:?}",
            app.latest_model().project_launcher.activity,
            app.session().project_runtime_preparation_state(),
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
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
    let prompt = pump_until_trust_prompt(&mut app);
    assert_eq!(prompt.module_id, "sample.tower-defense.runtime");
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
    let prompt = pump_until_trust_prompt(&mut app);
    let approved = app.dispatch_command(UiCommand {
        command_id: "approve_project_runtime_trust".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "trust-approve".to_string(),
        payload: UiCommandPayload::ApproveProjectRuntimeTrust {
            request_id: prompt.request_id,
        },
    });
    assert_eq!(approved.status, CommandStatus::Committed);
    assert!(app.take_approved_project_runtime_trust_request().is_none());
    let editor_core::ProjectRuntimePreparationState::Failed { diagnostic, .. } =
        app.session().project_runtime_preparation_state()
    else {
        panic!("approved trust must flow directly into runtime preparation");
    };
    assert_eq!(diagnostic.code, "project_runtime.preparer_unavailable");
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
    assert!(pump_until_trust_prompt(&mut stale_app).identity_changed);
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
    let result = app.dispatch_project_launcher_command_or_dispatch(UiCommand {
        command_id: "select_recent_project".to_string(),
        source: UiCommandSource::ProjectLauncher,
        request_id: "recent-project-trust".to_string(),
        payload: UiCommandPayload::SelectRecentProject {
            path: project_root().display().to_string(),
        },
    });
    assert!(result.is_none());
    assert_eq!(
        pump_until_trust_prompt(&mut app).module_id,
        "sample.tower-defense.runtime"
    );
}

#[test]
fn recent_project_trust_rejection_preserves_authoring_and_blocks_runtime() {
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
    assert!(app
        .dispatch_project_launcher_command_or_dispatch(UiCommand {
            command_id: "select_recent_project".to_string(),
            source: UiCommandSource::ProjectLauncher,
            request_id: "recent-project-denied".to_string(),
            payload: UiCommandPayload::SelectRecentProject {
                path: project_root().display().to_string(),
            },
        })
        .is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.frame(1280.0, 720.0);
        if matches!(
            app.session().project_runtime_preparation_state(),
            editor_core::ProjectRuntimePreparationState::Failed { .. }
        ) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "denied ProjectRust runtime did not reach Failed: activity={:?} preparation={:?}",
            app.latest_model().project_launcher.activity,
            app.session().project_runtime_preparation_state(),
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(app.latest_model().mode, EditorUiMode::AuthoringWorkspace);
    let editor_core::ProjectRuntimePreparationState::Failed { diagnostic, .. } =
        app.session().project_runtime_preparation_state()
    else {
        unreachable!("runtime failure was observed above");
    };
    assert_eq!(diagnostic.code, "project_editor_composition.trust_denied");
}
