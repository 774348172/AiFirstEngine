use super::fixtures::*;
use super::*;

#[test]
fn project_open_builds_project_browser_entries() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();

    let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    let model = session.build_ui_model();

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(model.mode, EditorUiMode::AuthoringWorkspace);
    assert_eq!(
        model.project_browser.selected_path.as_deref(),
        Some("Scenes/Main.scene.json")
    );
    assert!(model
        .project_browser
        .entries
        .iter()
        .any(|entry| entry.path == "Assets" && entry.exists));
    assert!(model
        .project_browser
        .entries
        .iter()
        .any(|entry| { entry.path == "Scenes/Main.scene.json" && entry.openable && entry.exists }));
}

#[test]
fn project_browser_open_scene_document_refreshes_hierarchy() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));

    let result = session.execute_command(command_for_test(
        UiCommandPayload::OpenProjectBrowserEntry {
            path: "Scenes/Main.scene.json".to_string(),
        },
    ));
    let model = session.build_ui_model();

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(
        model.project_browser.selected_path.as_deref(),
        Some("Scenes/Main.scene.json")
    );
    assert_eq!(model.hierarchy.scene_id.as_deref(), Some("scene-main"));
    assert_eq!(model.viewport.scene_id.as_deref(), Some("scene-main"));
}

#[test]
fn project_browser_model_remains_asset_browser_compatible_projection() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    fs::write(root.join("Assets").join("icon.png"), b"png").unwrap();
    session.refresh_asset_browser_now("test_external_asset_write");

    let model = session.build_ui_model().project_browser;

    assert!(model
        .entries
        .iter()
        .any(|entry| entry.path == "Assets/icon.png"
            && entry.kind == editor_ui_model::ProjectBrowserEntryKind::Asset));
    assert_eq!(
        model.selected_path.as_deref(),
        Some("Scenes/Main.scene.json")
    );
}

#[test]
fn project_browser_native_asset_commands_use_stable_key_without_rescan() {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: "PlaneGame".to_string(),
    }));
    let browser = session.build_asset_browser_model(editor_ui_model::AssetQuery::default());
    let scene_key = browser
        .entries
        .iter()
        .find(|entry| entry.path == "Scenes/Main.scene.json")
        .expect("default scene asset")
        .entry_key
        .clone();

    session.execute_command(command_for_test(
        UiCommandPayload::SelectAssetBrowserEntry {
            entry_key: scene_key.clone(),
            additive: false,
            range: false,
        },
    ));
    session.execute_command(command_for_test(UiCommandPayload::SetAssetBrowserSearch {
        search_text: "Main".to_string(),
    }));
    session.execute_command(command_for_test(UiCommandPayload::AssetBrowserToolbar {
        action: editor_ui_model::AssetBrowserToolbarAction::ToggleView,
    }));

    let browser = session.build_ui_model().asset_browser;
    assert_eq!(browser.selection.primary_entry_key, Some(scene_key));
    assert_eq!(browser.scan_generation, 1);
    assert_eq!(browser.query.search_text, "Main");
    assert_eq!(
        browser.view_mode,
        editor_ui_model::AssetBrowserViewMode::Grid
    );
}
