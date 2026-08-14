use super::*;
use editor_ui_renderer::{LayoutNodeId, PanelId, UiPoint, WidgetId};

#[test]
fn workspace_tab_drag_uses_retained_capture_and_commits_snapshot_target() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let model_revision = app.latest_model().revision;
    let tab = dock_tab_hit(&app, "ai_panel");
    let start = UiPoint {
        x: tab.rect.x + tab.rect.width * 0.5,
        y: tab.rect.y + tab.rect.height * 0.5,
    };
    let target_rect = app
        .workspace_docking()
        .snapshot(editor_ui_renderer::editor_workspace_rect(1280.0, 720.0))
        .node_rects["workspace/left"];
    let target = UiPoint {
        x: target_rect.x + target_rect.width * 0.5,
        y: target_rect.y + target_rect.height * 0.5,
    };

    app.handle_input_event(EditorInputEvent::PointerDown {
        x: start.x,
        y: start.y,
        button: PointerButton::Primary,
    });
    assert_eq!(
        app.workspace_docking()
            .active_panel_drag_id()
            .map(PanelId::as_str),
        Some("ai_panel")
    );
    assert_eq!(
        app.widget_interaction_snapshot()
            .captured_widget_id
            .as_ref()
            .map(WidgetId::as_str),
        Some("editor/dock/bottom-tabs/ai_panel")
    );

    app.handle_input_event(EditorInputEvent::PointerMove {
        x: target.x,
        y: target.y,
    });
    assert!(app.workspace_docking().panel_drag_is_active());
    assert_eq!(
        app.workspace_docking()
            .snapshot(editor_ui_renderer::editor_workspace_rect(1280.0, 720.0))
            .drag_preview
            .expect("production drag preview")
            .target_node_id,
        LayoutNodeId::new("workspace/left").unwrap()
    );
    app.handle_input_event(EditorInputEvent::PointerUp {
        x: target.x,
        y: target.y,
        button: PointerButton::Primary,
    });
    assert!(app.workspace_docking().active_panel_drag_id().is_none());
    assert!(app
        .widget_interaction_snapshot()
        .captured_widget_id
        .is_none());
    assert_eq!(
        app.workspace_docking()
            .active_panel_id("workspace/left")
            .map(PanelId::as_str),
        Some("ai_panel")
    );
    assert_eq!(app.latest_model().revision, model_revision);
    app.frame(1280.0, 720.0);
    let actual = app
        .retained_ui_renderer()
        .tree()
        .unwrap()
        .node(&WidgetId::semantic("editor/panel/ai").unwrap())
        .expect("moved AI panel")
        .logical_rect;
    let expected = app
        .workspace_docking()
        .snapshot(editor_ui_renderer::editor_workspace_rect(1280.0, 720.0))
        .panel_rects["ai_panel"];
    for (actual, expected) in [
        (actual.x, expected.x),
        (actual.y, expected.y),
        (actual.width, expected.width),
        (actual.height, expected.height),
    ] {
        assert!((actual - expected).abs() <= 0.5);
    }
    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn workspace_tab_click_stays_below_threshold_and_cancel_restores_layout() {
    for cancel in [
        EditorInputEvent::KeyDown {
            key: "Escape".to_string(),
        },
        EditorInputEvent::FocusLost,
    ] {
        let project_root = write_editor_project_fixture_for_shell();
        let session = opened_editor_project_session(&project_root);
        let mut app =
            NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
        app.frame(1280.0, 720.0);
        let original = app.workspace_docking().layout().clone();
        let tab = dock_tab_hit(&app, "ai_panel");
        let start = UiPoint {
            x: tab.rect.x + tab.rect.width * 0.5,
            y: tab.rect.y + tab.rect.height * 0.5,
        };
        app.handle_input_event(EditorInputEvent::PointerDown {
            x: start.x,
            y: start.y,
            button: PointerButton::Primary,
        });
        app.handle_input_event(EditorInputEvent::PointerMove {
            x: start.x + 20.0,
            y: start.y + 20.0,
        });
        assert!(app.workspace_docking().panel_drag_is_active());
        app.handle_input_event(cancel);
        assert_eq!(app.workspace_docking().layout(), &original);
        assert!(app.workspace_docking().active_panel_drag_id().is_none());
        assert!(app
            .widget_interaction_snapshot()
            .captured_widget_id
            .is_none());
        assert!(app.widget_interaction_snapshot().active_widget_id.is_none());
        let _ = std::fs::remove_dir_all(project_root);
    }

    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let tab = dock_tab_hit(&app, "ai_panel");
    let x = tab.rect.x + tab.rect.width * 0.5;
    let y = tab.rect.y + tab.rect.height * 0.5;
    app.handle_input_event(EditorInputEvent::PointerDown {
        x,
        y,
        button: PointerButton::Primary,
    });
    assert!(!app.workspace_docking().panel_drag_is_active());
    app.handle_input_event(EditorInputEvent::PointerUp {
        x,
        y,
        button: PointerButton::Primary,
    });
    assert_eq!(
        app.workspace_docking()
            .active_panel_id("workspace/bottom")
            .map(PanelId::as_str),
        Some("ai_panel")
    );
    let _ = std::fs::remove_dir_all(project_root);
}

fn dock_tab_hit(app: &NativeEditorApplication, panel_id: &str) -> editor_ui_renderer::HitRegion {
    app.latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::DockTab { panel_id: candidate } if candidate == panel_id
            )
        })
        .expect("workspace dock tab hit")
        .clone()
}
