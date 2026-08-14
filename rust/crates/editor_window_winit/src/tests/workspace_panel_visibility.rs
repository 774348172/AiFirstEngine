use super::*;

#[test]
fn workspace_window_menu_closes_shows_and_resets_without_project_mutation() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let project_revision = app.latest_model().revision;

    click_target(&mut app, |target| {
        matches!(target, HitTarget::WorkspaceWindowMenu)
    });
    app.frame(1280.0, 720.0);
    click_target(&mut app, |target| {
        matches!(
            target,
            HitTarget::WorkspacePanelVisibility { panel_id, visible }
                if panel_id == "ai_panel" && *visible
        )
    });
    assert!(app
        .workspace_docking()
        .layout()
        .closed_panels
        .iter()
        .any(|panel_id| panel_id.as_str() == "ai_panel"));
    assert_eq!(app.latest_model().revision, project_revision);

    app.frame(1280.0, 720.0);
    click_target(&mut app, |target| {
        matches!(target, HitTarget::WorkspaceWindowMenu)
    });
    app.frame(1280.0, 720.0);
    click_target(&mut app, |target| {
        matches!(
            target,
            HitTarget::WorkspacePanelVisibility { panel_id, visible }
                if panel_id == "ai_panel" && !*visible
        )
    });
    assert!(!app
        .workspace_docking()
        .layout()
        .closed_panels
        .iter()
        .any(|panel_id| panel_id.as_str() == "ai_panel"));

    assert!(app.close_workspace_panel("ai_panel").changed);
    app.frame(1280.0, 720.0);
    click_target(&mut app, |target| {
        matches!(target, HitTarget::WorkspaceWindowMenu)
    });
    app.frame(1280.0, 720.0);
    click_target(&mut app, |target| {
        matches!(target, HitTarget::WorkspaceResetLayout)
    });
    assert!(app.workspace_docking().layout().closed_panels.is_empty());
    assert_eq!(app.latest_model().revision, project_revision);
    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
}

fn click_target(app: &mut NativeEditorApplication, predicate: impl Fn(&HitTarget) -> bool) {
    let region = app
        .latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| predicate(&region.target))
        .expect("workspace menu target")
        .clone();
    app.handle_input_event(EditorInputEvent::PointerDown {
        x: region.rect.x + region.rect.width * 0.5,
        y: region.rect.y + region.rect.height * 0.5,
        button: PointerButton::Primary,
    });
}
