use super::*;
use editor_ui_renderer::{DockNode, LayoutNodeId, WidgetId};

#[test]
fn workspace_splitter_production_tree_exposes_stable_resize_widgets() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);

    app.frame(1280.0, 720.0);

    let splitter_count = app
        .retained_ui_renderer()
        .tree()
        .expect("retained production tree")
        .nodes
        .keys()
        .filter(|id| id.as_str().starts_with("editor/workspace/splitter/"))
        .count();
    assert_eq!(
        splitter_count, 3,
        "default workspace must expose one stable Widget per Split node"
    );

    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn workspace_splitter_horizontal_and_vertical_drag_use_pointer_capture_and_commit() {
    for (node_id, delta) in [
        ("workspace/root", (0.0, -48.0)),
        ("workspace/top", (64.0, 0.0)),
    ] {
        let project_root = write_editor_project_fixture_for_shell();
        let session = opened_editor_project_session(&project_root);
        let mut app =
            NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
        app.frame(1280.0, 720.0);
        let before_model_revision = app.latest_model().revision;
        let before_ratio = split_ratio(app.workspace_docking().layout(), node_id);
        let hit = splitter_hit(&app, node_id);
        let pointer = (
            hit.rect.x + hit.rect.width * 0.5,
            hit.rect.y + hit.rect.height * 0.5,
        );
        let picked = editor_ui_renderer::pick_widget(
            app.retained_ui_renderer().tree().unwrap(),
            editor_ui_renderer::UiPoint {
                x: pointer.0,
                y: pointer.1,
            },
            None,
        )
        .expect("splitter WidgetTree pick");
        assert_eq!(
            picked.target.as_str(),
            format!("editor/workspace/splitter/{node_id}")
        );

        app.handle_input_event(EditorInputEvent::PointerDown {
            x: pointer.0,
            y: pointer.1,
            button: PointerButton::Primary,
        });
        assert_eq!(
            app.workspace_docking()
                .active_resize_node_id()
                .map(LayoutNodeId::as_str),
            Some(node_id)
        );
        assert_eq!(
            app.focus_input()
                .pointer_capture
                .as_ref()
                .map(WidgetId::as_str),
            Some(format!("editor/workspace/splitter/{node_id}").as_str())
        );
        app.handle_input_event(EditorInputEvent::PointerMove {
            x: pointer.0 + delta.0,
            y: pointer.1 + delta.1,
        });
        assert_eq!(app.latest_model().revision, before_model_revision);
        assert_ne!(
            split_ratio(app.workspace_docking().layout(), node_id),
            before_ratio
        );
        app.handle_input_event(EditorInputEvent::PointerUp {
            x: pointer.0 + delta.0,
            y: pointer.1 + delta.1,
            button: PointerButton::Primary,
        });
        assert!(app.workspace_docking().active_resize_node_id().is_none());
        assert!(app.focus_input().pointer_capture.is_none());

        let committed = split_ratio(app.workspace_docking().layout(), node_id);
        app.resize(1600, 900);
        app.frame(1600.0, 900.0);
        assert_eq!(
            split_ratio(app.workspace_docking().layout(), node_id),
            committed
        );
        let _ = std::fs::remove_dir_all(project_root);
    }
}

#[test]
fn workspace_splitter_hover_and_active_drag_expose_resize_cursor_contract() {
    for (node_id, expected_cursor) in [
        ("workspace/top", WorkspacePointerCursor::ColumnResize),
        ("workspace/root", WorkspacePointerCursor::RowResize),
    ] {
        let project_root = write_editor_project_fixture_for_shell();
        let session = opened_editor_project_session(&project_root);
        let mut app =
            NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
        app.frame(1280.0, 720.0);
        let hit = splitter_hit(&app, node_id);
        let x = hit.rect.x + hit.rect.width * 0.5;
        let y = hit.rect.y + hit.rect.height * 0.5;

        app.handle_input_event(EditorInputEvent::PointerMove { x, y });
        assert_eq!(app.workspace_pointer_cursor(), expected_cursor);

        app.handle_input_event(EditorInputEvent::PointerDown {
            x,
            y,
            button: PointerButton::Primary,
        });
        app.handle_input_event(EditorInputEvent::PointerMove {
            x: x + 96.0,
            y: y + 96.0,
        });
        assert_eq!(
            app.workspace_pointer_cursor(),
            expected_cursor,
            "active resize must keep its cursor after leaving the splitter hit rect"
        );

        app.handle_input_event(EditorInputEvent::PointerUp {
            x: x + 96.0,
            y: y + 96.0,
            button: PointerButton::Primary,
        });
        app.handle_input_event(EditorInputEvent::PointerMove { x: 1.0, y: 1.0 });
        assert_eq!(
            app.workspace_pointer_cursor(),
            WorkspacePointerCursor::Default
        );
        let _ = std::fs::remove_dir_all(project_root);
    }
}

#[test]
fn workspace_splitter_escape_and_focus_lost_restore_starting_ratio() {
    for cancel_event in [
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
        let node_id = "workspace/top-main";
        let before_ratio = split_ratio(app.workspace_docking().layout(), node_id);
        let hit = splitter_hit(&app, node_id);
        let x = hit.rect.x + hit.rect.width * 0.5;
        let y = hit.rect.y + hit.rect.height * 0.5;
        app.handle_input_event(EditorInputEvent::PointerDown {
            x,
            y,
            button: PointerButton::Primary,
        });
        app.handle_input_event(EditorInputEvent::PointerMove { x: x - 48.0, y });
        assert_ne!(
            split_ratio(app.workspace_docking().layout(), node_id),
            before_ratio
        );

        app.handle_input_event(cancel_event);

        assert_eq!(
            split_ratio(app.workspace_docking().layout(), node_id),
            before_ratio
        );
        assert!(app.workspace_docking().active_resize_node_id().is_none());
        assert!(app.focus_input().pointer_capture.is_none());
        assert!(app.focus_input().pressed_hit_id.is_none());
        let _ = std::fs::remove_dir_all(project_root);
    }
}

#[test]
fn workspace_splitter_production_panel_geometry_matches_workspace_snapshot() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);
    let snapshot = app
        .workspace_docking()
        .snapshot(editor_ui_renderer::editor_workspace_rect(1280.0, 720.0));
    let tree = app.retained_ui_renderer().tree().expect("retained tree");

    for (panel_id, widget_id) in [
        ("hierarchy", "editor/panel/hierarchy"),
        ("viewport", "editor/panel/viewport"),
        ("inspector", "editor/panel/inspector"),
        ("asset_browser", "editor/panel/asset-browser"),
    ] {
        let actual = tree
            .node(&WidgetId::semantic(widget_id).unwrap())
            .expect("visible production panel")
            .logical_rect;
        let expected = snapshot.panel_rects[panel_id];
        for (actual, expected) in [
            (actual.x, expected.x),
            (actual.y, expected.y),
            (actual.width, expected.width),
            (actual.height, expected.height),
        ] {
            assert!(
                (actual - expected).abs() <= 0.5,
                "{panel_id} must consume WorkspaceSnapshot geometry: {actual} != {expected}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(project_root);
}

fn splitter_hit(app: &NativeEditorApplication, node_id: &str) -> editor_ui_renderer::HitRegion {
    app.latest_draw_list()
        .hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.target,
                HitTarget::WorkspaceSplitter { node_id: candidate } if candidate == node_id
            )
        })
        .expect("workspace splitter hit")
        .clone()
}

fn split_ratio(layout: &editor_ui_renderer::EditorWorkspaceLayout, node_id: &str) -> f32 {
    fn find(node: &DockNode, node_id: &str) -> Option<f32> {
        match node {
            DockNode::Split {
                node_id: candidate,
                ratio,
                first,
                second,
                ..
            } => {
                if candidate.as_str() == node_id {
                    Some(*ratio)
                } else {
                    find(first, node_id).or_else(|| find(second, node_id))
                }
            }
            DockNode::Stack { .. } => None,
        }
    }
    find(&layout.root, node_id).expect("split ratio")
}
