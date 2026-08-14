use super::fixtures::*;
use super::*;

#[test]
fn project_authoring_workspace_model_summarizes_open_project_domains() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();

    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    let model = session.build_ui_model();
    let workspace = model.project_authoring_workspace;

    assert!(workspace
        .project_id
        .as_deref()
        .is_some_and(|id| id.starts_with("project-")));
    assert_eq!(workspace.active_scene_id.as_deref(), Some("scene-main"));
    assert_eq!(workspace.domains.len(), 10);
    assert!(workspace.domains.iter().any(|domain| {
        domain.kind == WorkspaceDomainKind::Project
            && domain.status == WorkspaceDomainStatus::Ready
            && domain.summary.contains("PlaneGame")
    }));
    assert!(workspace.domains.iter().any(|domain| {
        domain.kind == WorkspaceDomainKind::Scene
            && domain.status == WorkspaceDomainStatus::Ready
            && domain.summary.contains("scene-main")
    }));
    assert!(workspace.domains.iter().any(|domain| {
        domain.kind == WorkspaceDomainKind::Build
            && domain.selected_id.as_deref() == Some("windows-dev")
    }));
}

#[test]
fn workspace_report_domain_uses_unified_report_panel_counts() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();

    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    let model = session.build_ui_model();
    let report_domain = model
        .project_authoring_workspace
        .domains
        .iter()
        .find(|domain| domain.kind == WorkspaceDomainKind::Report)
        .expect("report domain should be present");

    assert_eq!(
        model.project_authoring_workspace.report.report_count,
        model.report_panel.summary.report_count
    );
    assert_eq!(
        report_domain.item_count,
        model.report_panel.summary.report_count
    );
    assert!(report_domain.summary.contains("providers="));
    assert!(model
        .report_panel
        .reports
        .iter()
        .any(|report| report.provider_id == "authoring.manual_walkthrough"));
}

#[test]
fn workspace_domain_summary_counts_prefab_rule_aui_and_input_files() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    fs::create_dir_all(root.join("Prefabs")).unwrap();
    fs::create_dir_all(root.join("Rules")).unwrap();
    fs::create_dir_all(root.join("AUI")).unwrap();
    fs::create_dir_all(root.join("Input")).unwrap();
    fs::write(root.join("Prefabs").join("ship.prefab.json"), "{}").unwrap();
    fs::write(root.join("Rules").join("frame.rule.json"), "{}").unwrap();
    fs::write(root.join("AUI").join("hud.aui.json"), "{}").unwrap();
    fs::write(
        root.join("Input").join("game.input-mapping.json"),
        serde_json::to_string_pretty(&engine_input::InputMappingAsset::gameplay_default()).unwrap(),
    )
    .unwrap();

    let workspace = session.build_ui_model().project_authoring_workspace;

    for (kind, expected_count) in [
        (WorkspaceDomainKind::Prefab, 1),
        (WorkspaceDomainKind::Rule, 1),
        (WorkspaceDomainKind::Aui, 1),
        (WorkspaceDomainKind::Input, 2),
    ] {
        assert!(workspace.domains.iter().any(|domain| {
            domain.kind == kind
                && domain.status == WorkspaceDomainStatus::Ready
                && domain.item_count == expected_count
        }));
    }
}

#[test]
fn editor_session_asset_browser_builds_after_project_create() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    fs::write(root.join("Assets").join("icon.png"), b"png").unwrap();
    session.refresh_asset_browser_now("test_external_asset_write");

    let browser = session.build_asset_browser_model(AssetQuery::default());

    assert_eq!(
        browser.project_root.as_deref(),
        Some(root.to_string_lossy().as_ref())
    );
    assert!(browser
        .entries
        .iter()
        .any(|entry| entry.path == "Assets/icon.png"
            && entry.kind == editor_ui_model::AssetKind::Texture));
    assert!(browser
        .entries
        .iter()
        .any(|entry| entry.path == "Scenes/Main.scene.json" && entry.openable));
    assert!(browser.report.asset_count >= 2);
}

#[test]
fn editor_session_asset_browser_selection_tracks_project_browser_selection() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    session.execute_command(command_for_test(
        UiCommandPayload::SelectProjectBrowserEntry {
            path: "Scenes/Main.scene.json".to_string(),
        },
    ));

    let browser = session.build_asset_browser_model(AssetQuery::default());

    assert_eq!(
        browser.selection.primary_path.as_deref(),
        Some("Scenes/Main.scene.json")
    );
    assert!(browser
        .entries
        .iter()
        .any(|entry| entry.path == "Scenes/Main.scene.json" && entry.selected));
}

#[test]
fn editor_session_input_mapping_creates_edits_validates_and_updates_workspace() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let create = session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: "Input/input.default.json".to_string(),
        },
    ));
    let add_action = session.execute_command(command_for_test(UiCommandPayload::AddInputAction {
        path: "Input/input.default.json".to_string(),
        action_id: "action.test".to_string(),
        value_type: editor_ui_model::InputActionValueKind::Button,
    }));
    let add_binding =
        session.execute_command(command_for_test(UiCommandPayload::AddInputBinding {
            path: "Input/input.default.json".to_string(),
            context_id: "gameplay".to_string(),
            action_id: "action.test".to_string(),
            device_path: "keyboard/T".to_string(),
        }));
    let validate =
        session.execute_command(command_for_test(UiCommandPayload::ValidateInputMapping {
            path: "Input/input.default.json".to_string(),
        }));
    let model = session.build_ui_model();
    let input_model = model.input_mapping_authoring;
    let input_domain = model
        .project_authoring_workspace
        .domains
        .iter()
        .find(|domain| domain.kind == WorkspaceDomainKind::Input)
        .expect("input workspace domain should exist");

    assert_eq!(create.status, CommandStatus::Committed);
    assert_eq!(add_action.status, CommandStatus::Committed);
    assert_eq!(add_binding.status, CommandStatus::Committed);
    assert_eq!(validate.status, CommandStatus::Committed);
    assert_eq!(input_model.mapping_id.as_deref(), Some("input.default"));
    assert!(input_model
        .actions
        .iter()
        .any(|action| action.action_id == "action.test"));
    assert!(input_model
        .bindings
        .iter()
        .any(|binding| binding.device_path == "keyboard/T"));
    assert_eq!(input_domain.status, WorkspaceDomainStatus::Ready);
    assert!(input_domain.summary.contains("validation=Ok"));
}

#[test]
fn editor_session_input_mapping_asset_browser_recognizes_created_mapping() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: "Input/input.default.json".to_string(),
        },
    ));

    let browser = session.build_asset_browser_model(AssetQuery::default());

    assert!(browser.entries.iter().any(|entry| {
        entry.path == "Input/input.default.json"
            && entry.kind == editor_ui_model::AssetKind::InputMapping
    }));
}

#[test]
fn editor_session_input_mapping_discard_restores_saved_asset() {
    let root = unique_editor_project_temp_dir();
    let path = "Input/input.default.json";
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: path.to_string(),
        },
    ));
    session.execute_command(command_for_test(UiCommandPayload::AddInputAction {
        path: path.to_string(),
        action_id: "action.unsaved".to_string(),
        value_type: editor_ui_model::InputActionValueKind::Button,
    }));

    assert!(session.build_input_mapping_authoring_model().dirty);
    let discard = session.execute_command(command_for_test(
        UiCommandPayload::DiscardInputMappingDraft {
            path: path.to_string(),
        },
    ));
    let model = session.build_input_mapping_authoring_model();

    assert_eq!(discard.status, CommandStatus::Committed);
    assert!(!model.dirty);
    assert!(!model
        .actions
        .iter()
        .any(|action| action.action_id == "action.unsaved"));
}

#[test]
fn project_authoring_workspace_end_to_end_gate_tracks_edit_save_play_and_export() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();

    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    session.execute_command(command_for_test(
        UiCommandPayload::OpenProjectBrowserEntry {
            path: "Scenes/Main.scene.json".to_string(),
        },
    ));
    session.execute_command(command_for_test(UiCommandPayload::CreateSceneEntity {
        parent_id: None,
        name: "WorkspaceEntity".to_string(),
    }));
    let entity_id = session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .find(|node| node.label == "WorkspaceEntity")
        .map(|node| node.entity_id.clone())
        .expect("created entity should be visible in hierarchy");
    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: entity_id.clone(),
    }));
    session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
        entity_id,
        local_position: Some(Vec3 {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    }));
    session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
        path: None,
    }));
    let play_result = session.execute_command(command_for_test(UiCommandPayload::Play));
    let export_result =
        session.execute_command(command_for_test(UiCommandPayload::ExportDesktopPackage {
            profile_id: Some("windows-dev".to_string()),
        }));

    let model = session.build_ui_model();
    let workspace = model.project_authoring_workspace;

    assert_eq!(workspace.report.project_status, "open");
    assert!(workspace.domains.iter().any(|domain| {
        domain.kind == WorkspaceDomainKind::Scene && domain.status == WorkspaceDomainStatus::Ready
    }));
    assert!(workspace.domains.iter().any(|domain| {
        domain.kind == WorkspaceDomainKind::Build
            && matches!(
                domain.status,
                WorkspaceDomainStatus::Ready | WorkspaceDomainStatus::Error
            )
    }));
    assert_eq!(workspace.report.play_status.as_deref(), Some("running"));
    assert!(workspace.report.build_status.is_some());
    assert!(matches!(
        play_result.status,
        CommandStatus::Committed | CommandStatus::Rejected | CommandStatus::Failed
    ));
    assert!(matches!(
        export_result.status,
        CommandStatus::Committed | CommandStatus::Failed
    ));

    let mut authoring_workspace = EditorAuthoringWorkspace::new();
    authoring_workspace.refresh_from_model(&session.build_ui_model());
    assert!(authoring_workspace
        .context()
        .domain_summaries
        .iter()
        .any(|summary| summary.contains("build")));
    assert!(authoring_workspace.context().build_summary.is_some());
}
