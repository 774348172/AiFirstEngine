use super::*;
use editor_ui_renderer::{native_editor_panel_manifest, WidgetId};

#[test]
fn workspace_dpi_matrix_preserves_production_dock_geometry() {
    for (physical_width, physical_height, scale_factor) in [
        (1280_u32, 720_u32, 1.0_f64),
        (1600, 900, 1.5),
        (1920, 1080, 2.0),
    ] {
        let project_root = write_editor_project_fixture_for_shell();
        let session = opened_editor_project_session(&project_root);
        let mut app =
            NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
        let viewport = EditorReachabilityViewport::from_physical(
            physical_width,
            physical_height,
            scale_factor,
        );
        app.frame(
            viewport.logical_width as f32,
            viewport.logical_height as f32,
        );
        let snapshot = app
            .workspace_docking()
            .snapshot(editor_ui_renderer::editor_workspace_rect(
                viewport.logical_width as f32,
                viewport.logical_height as f32,
            ));

        assert_eq!(snapshot.splitters.len(), 3);
        assert!(snapshot.diagnostics.is_empty());
        for splitter in &snapshot.splitters {
            assert!(splitter.hit_rect.width > 0.0 && splitter.hit_rect.height > 0.0);
            assert!(splitter.visual_rect.width > 0.0 && splitter.visual_rect.height > 0.0);
            let physical_right =
                f64::from(splitter.hit_rect.x + splitter.hit_rect.width) * scale_factor;
            let physical_bottom =
                f64::from(splitter.hit_rect.y + splitter.hit_rect.height) * scale_factor;
            assert!(physical_right <= f64::from(physical_width) + 1.0);
            assert!(physical_bottom <= f64::from(physical_height) + 1.0);
        }

        let dockable_count = native_editor_panel_manifest()
            .iter()
            .filter(|panel| panel.dockable)
            .count();
        assert_eq!(snapshot.panel_rects.len(), dockable_count);
        for panel in native_editor_panel_manifest()
            .iter()
            .filter(|panel| panel.dockable)
        {
            assert!(
                snapshot.panel_rects.contains_key(panel.panel_id),
                "missing production dockable panel {}",
                panel.panel_id
            );
        }
        let _ = std::fs::remove_dir_all(project_root);
    }
}

#[test]
fn workspace_dpi_matrix_reports_narrow_minimum_pressure_without_invalid_geometry() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(640.0, 360.0);
    let tree = app.retained_ui_renderer().tree().expect("narrow tree");
    let snapshot = app
        .workspace_docking()
        .snapshot(editor_ui_renderer::editor_workspace_rect(640.0, 360.0));

    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "minimum_size_unsatisfied"));
    for node in tree.nodes.values() {
        assert!(node.logical_rect.x.is_finite());
        assert!(node.logical_rect.y.is_finite());
        assert!(node.logical_rect.width.is_finite() && node.logical_rect.width >= 0.0);
        assert!(node.logical_rect.height.is_finite() && node.logical_rect.height >= 0.0);
    }
    assert!(tree
        .node(&WidgetId::semantic("editor/workspace/splitter/workspace/root").unwrap())
        .is_some());
    let _ = std::fs::remove_dir_all(project_root);
}
