use super::*;

#[test]
fn reachability_report_serializes_versioned_snapshot_and_provenance() {
    let scenario = EditorReachabilityScenario::new("schema", 1280, 720, 1.0);
    let report =
        run_deterministic_reachability_scenario(&scenario, EditorReachabilityReportLevel::Trace);

    assert_eq!(report.status, EditorReachabilityStatus::Passed);
    let snapshot = report.snapshot.as_ref().expect("widget snapshot");
    assert_eq!(
        snapshot.schema_version,
        EDITOR_WIDGET_TREE_SNAPSHOT_SCHEMA_VERSION
    );
    assert!(snapshot.widget_count > 0);
    assert!(snapshot.visible_widget_count > 0);
    assert!(snapshot.widgets.iter().all(|widget| {
        widget.physical_rect.x == widget.logical_rect.x
            && widget.physical_rect.y == widget.logical_rect.y
    }));
    let serialized = serde_json::to_string_pretty(&report).expect("serialize report");
    assert!(serialized.contains(EDITOR_UI_REACHABILITY_REPORT_SCHEMA_VERSION));
    assert!(!serialized.contains("G:\\\\"));
}

#[test]
fn reachability_matrix_covers_three_sizes_and_fractional_scale() {
    let scenarios = deterministic_reachability_scenarios();
    assert_eq!(scenarios.len(), 9);
    assert!(scenarios
        .iter()
        .any(|scenario| scenario.scale_factor == 1.5));
    for scenario in scenarios {
        let report = run_deterministic_reachability_scenario(
            &scenario,
            EditorReachabilityReportLevel::Summary,
        );
        assert_eq!(
            report.status,
            EditorReachabilityStatus::Passed,
            "{}: {:?}",
            scenario.scenario_id,
            report.diagnostics
        );
        let snapshot = report.snapshot.expect("snapshot");
        assert_eq!(snapshot.viewport.physical_width, scenario.physical_width);
        assert_eq!(snapshot.viewport.scale_factor, scenario.scale_factor);
        assert!(snapshot.focusable_widget_count > 0);
        assert!(snapshot.reachable_widget_count > 0);
    }
}

#[test]
fn reachability_report_off_avoids_snapshot_cost() {
    let scenario = EditorReachabilityScenario::new("off", 1280, 720, 1.0);
    let report =
        run_deterministic_reachability_scenario(&scenario, EditorReachabilityReportLevel::Off);

    assert_eq!(report.status, EditorReachabilityStatus::NotEvaluated);
    assert!(report.snapshot.is_none());
    assert!(report.diagnostics.is_empty());
}

#[test]
fn human_workflow_panel_matrix_is_reachable_through_retained_tabs() {
    let source_root = complex_shooter_project_fixture_root();
    let source_journal_path = source_root.join("Library/ProjectIntent/journal.json");
    let source_journal_before = std::fs::read(&source_journal_path).ok();
    let project_root = copy_complex_shooter_project_fixture();
    let mut session = session_with_linked_project_runtime("sample.complex-shooter.runtime");
    let payload = UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    };
    let opened = session.execute_command(UiCommand {
        command_id: editor_ui_model::ui_command_id_for_payload(&payload).to_string(),
        source: UiCommandSource::Test,
        request_id: "reachability-open-project".to_string(),
        payload,
    });
    assert_eq!(
        opened.status,
        CommandStatus::Committed,
        "OpenProject diagnostics: {:?}",
        opened.diagnostics
    );
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    app.frame(1280.0, 720.0);

    for (panel_id, root_widget_id) in [
        ("asset_browser", "editor/panel/asset-browser"),
        ("authoring_workflow", "editor/panel/authoring-workflow"),
        ("input_mapping", "editor/panel/input-mapping"),
        ("build_export", "editor/panel/build-export"),
        ("ai_panel", "editor/panel/ai"),
        ("console", "editor/panel/console"),
        ("runtime_trace", "editor/panel/runtime-trace"),
        ("report", "editor/panel/report"),
    ] {
        let tab_id =
            editor_ui_renderer::WidgetId::semantic(format!("editor/dock/bottom-tabs/{panel_id}"))
                .unwrap();
        let tab_rect = app
            .retained_ui_renderer()
            .tree()
            .and_then(|tree| tree.node(&tab_id))
            .map(|node| node.logical_rect)
            .expect("retained dock tab");
        let x = tab_rect.x + tab_rect.width * 0.5;
        let y = tab_rect.y + tab_rect.height * 0.5;
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
        let app_report = app.frame(1280.0, 720.0);
        assert_eq!(
            app.workspace_docking()
                .active_panel_id("workspace/bottom")
                .map(|active| active.as_str()),
            Some(panel_id),
            "{panel_id} tab did not activate"
        );
        let tree = app.retained_ui_renderer().tree().expect("retained tree");
        let root_id = editor_ui_renderer::WidgetId::semantic(root_widget_id).unwrap();
        let root = tree.node(&root_id).expect("manifest panel root");
        assert_eq!(
            root.visibility,
            editor_ui_renderer::WidgetVisibility::Visible
        );
        let viewport = EditorReachabilityViewport::from_physical(1280, 720, 1.0);
        let (_snapshot, diagnostics) = snapshot_widget_tree(
            tree,
            EditorWidgetSnapshotContext {
                frame_index: app_report.frame_index,
                model_revision: app_report.model_revision,
                viewport,
                keyboard_focus: app.focus_input().keyboard_focus.as_ref(),
                pointer_capture: app.focus_input().pointer_capture.as_ref(),
                level: EditorReachabilityReportLevel::Summary,
            },
        );
        assert!(
            diagnostics.iter().all(
                |diagnostic| diagnostic.severity != EditorReachabilityDiagnosticSeverity::Error
            ),
            "{panel_id}: {diagnostics:?}"
        );
    }

    assert!(app
        .latest_model()
        .report_panel
        .registry
        .descriptors
        .iter()
        .any(|descriptor| descriptor.provider_id == "editor.ui_reachability"));
    assert_eq!(
        std::fs::read(source_journal_path).ok(),
        source_journal_before
    );
}
