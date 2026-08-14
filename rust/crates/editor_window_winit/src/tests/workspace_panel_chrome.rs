use super::*;

#[test]
fn workspace_panel_chrome_popup_close_and_inspector_lock_are_retained() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);

    click_target(
        &mut app,
        |target| matches!(target, HitTarget::HierarchyEntity { entity_id } if entity_id == "entity-player"),
    );
    app.frame(1280.0, 720.0);
    let lock = target_region(&app, |target| {
        matches!(
            target,
            HitTarget::WorkspacePanelLock { panel_id, locked, .. }
                if panel_id == "inspector" && !*locked
        )
    });
    assert!(lock.enabled, "{:?}", lock.reason_disabled);
    click_region(&mut app, lock);
    app.frame(1280.0, 720.0);
    assert!(app
        .latest_draw_list()
        .hit_regions
        .iter()
        .any(|region| matches!(
            &region.target,
            HitTarget::WorkspacePanelLock { panel_id, locked, .. }
                if panel_id == "inspector" && *locked
        )));

    let more = target_region(&app, |target| {
        matches!(
            target,
            HitTarget::WorkspacePanelMore { panel_id, .. } if panel_id == "inspector"
        )
    });
    click_region(&mut app, more);
    app.frame(1280.0, 720.0);
    assert!(app
        .latest_draw_list()
        .hit_regions
        .iter()
        .any(|region| matches!(
            &region.target,
            HitTarget::WorkspacePanelClose { panel_id, .. } if panel_id == "inspector"
        ) && !region.enabled));
    app.handle_input_event(EditorInputEvent::KeyDown {
        key: "Escape".to_string(),
    });
    app.frame(1280.0, 720.0);
    assert!(!app
        .latest_draw_list()
        .hit_regions
        .iter()
        .any(|region| matches!(region.target, HitTarget::WorkspacePanelClose { .. })));

    click_target(
        &mut app,
        |target| matches!(target, HitTarget::DockTab { panel_id } if panel_id == "ai_panel"),
    );
    app.frame(1280.0, 720.0);
    click_target(&mut app, |target| {
        matches!(
            target,
            HitTarget::WorkspacePanelMore { panel_id, .. } if panel_id == "ai_panel"
        )
    });
    app.frame(1280.0, 720.0);
    let close = target_region(&app, |target| {
        matches!(
            target,
            HitTarget::WorkspacePanelClose { panel_id, .. } if panel_id == "ai_panel"
        )
    });
    assert!(close.enabled);
    click_region(&mut app, close);
    assert!(app
        .workspace_docking()
        .layout()
        .closed_panels
        .iter()
        .any(|panel_id| panel_id.as_str() == "ai_panel"));

    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn workspace_panel_chrome_popup_closes_on_outside_click_and_focus_loss() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);

    for event in [
        EditorInputEvent::PointerDown {
            x: 640.0,
            y: 360.0,
            button: PointerButton::Primary,
        },
        EditorInputEvent::FocusLost,
    ] {
        click_target(&mut app, |target| {
            matches!(target, HitTarget::WorkspacePanelMore { .. })
        });
        app.frame(1280.0, 720.0);
        assert!(app
            .latest_draw_list()
            .hit_regions
            .iter()
            .any(|region| matches!(region.target, HitTarget::WorkspacePanelClose { .. })));
        app.handle_input_event(event);
        app.frame(1280.0, 720.0);
        assert!(!app
            .latest_draw_list()
            .hit_regions
            .iter()
            .any(|region| matches!(region.target, HitTarget::WorkspacePanelClose { .. })));
    }

    drop(app);
    std::fs::remove_dir_all(project_root).unwrap();
}

fn target_region(
    app: &NativeEditorApplication,
    predicate: impl Fn(&HitTarget) -> bool,
) -> editor_ui_renderer::HitRegion {
    app.latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| predicate(&region.target))
        .expect("workspace panel chrome target")
        .clone()
}

fn click_target(app: &mut NativeEditorApplication, predicate: impl Fn(&HitTarget) -> bool) {
    let region = target_region(app, predicate);
    click_region(app, region);
}

fn click_region(app: &mut NativeEditorApplication, region: editor_ui_renderer::HitRegion) {
    let x = region.rect.x + region.rect.width * 0.5;
    let y = region.rect.y + region.rect.height * 0.5;
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
}
