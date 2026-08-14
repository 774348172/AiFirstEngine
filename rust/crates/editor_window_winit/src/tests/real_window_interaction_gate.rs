use super::*;

#[test]
fn real_window_interaction_report_serializes() {
    let scenario = RealWindowInteractionSmokeScenario::new(NativeEditorInteractionScenario::new(
        "empty",
        "Empty real-window smoke",
    ));

    let report = run_headless_real_window_interaction_smoke(scenario);
    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(REAL_WINDOW_INTERACTION_SMOKE_REPORT_SCHEMA_VERSION));
    assert!(json.contains("headless-compatible-window-event-bridge"));
}

#[test]
fn real_window_interaction_gate_create_project_presents_and_runs_scenario() {
    let fixture_root = unique_project_launcher_temp_dir();
    std::fs::create_dir_all(&fixture_root).expect("fixture owner root");
    let project_root = fixture_root.join("RealWindowScenarioCreated");
    let mut app = NativeEditorApplication::with_project_manager(
        NativeEditorWindowConfig::default(),
        EditorSession::new(),
        ProjectManagerController::default(),
        Box::new(HeadlessFolderDialogBackend::with_create_project_path(
            project_root.display().to_string(),
        )),
    );
    let scenario = RealWindowInteractionSmokeScenario::new(
        NativeEditorInteractionScenario::new(
            "real-window-create-project",
            "Create project through real-window smoke gate",
        )
        .with_step(
            NativeEditorInteractionStep::click_hit_region(
                "click-create-project",
                "hit.project_launcher.create_project",
            )
            .expect_command("create_project", CommandStatus::Committed)
            .expect_mode(EditorUiMode::AuthoringWorkspace)
            .expect_revision_increase(),
        ),
    );

    let report = RealWindowInteractionSmokeRunner::default().run(&mut app, scenario);

    assert_eq!(report.status, RealWindowInteractionSmokeStatus::Passed);
    assert!(report.window_created);
    assert!(report.surface_created);
    assert!(report.surface_configured);
    assert_eq!(report.present_status, "presented");
    assert!(report.frame_count > 0);
    assert!(report.draw_command_count > 0);
    assert!(report.hit_region_count > 0);
    let interaction = report.interaction_report.expect("interaction report");
    assert_eq!(interaction.status, NativeEditorInteractionStatus::Passed);
    assert_eq!(interaction.final_mode, EditorUiMode::AuthoringWorkspace);
    assert!(project_root.join("project.aife.json").exists());
}

#[test]
fn real_window_interaction_gate_missing_hit_region_fails_with_diagnostic() {
    let scenario = RealWindowInteractionSmokeScenario::new(
        NativeEditorInteractionScenario::new(
            "real-window-missing-hit",
            "Missing hit region through real-window smoke gate",
        )
        .with_step(NativeEditorInteractionStep::click_hit_region(
            "click-missing",
            "hit.missing",
        )),
    );

    let report = run_headless_real_window_interaction_smoke(scenario);

    assert_eq!(report.status, RealWindowInteractionSmokeStatus::Failed);
    let interaction = report.interaction_report.expect("interaction report");
    assert!(interaction
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "interaction.hit_region_missing"));
}

#[test]
fn real_window_interaction_screenshot_evidence_is_recorded() {
    let scenario = RealWindowInteractionSmokeScenario::new(NativeEditorInteractionScenario::new(
        "screenshot-evidence",
        "Screenshot evidence metadata",
    ));

    let report = run_headless_real_window_interaction_smoke(scenario);
    let screenshot = report.screenshot.expect("screenshot evidence");

    assert_eq!(screenshot.kind, "metadata-only");
    assert_eq!(screenshot.width, 1280);
    assert_eq!(screenshot.height, 720);
    assert!(screenshot.rgba_hash.is_some());
}

#[cfg(not(feature = "real-window"))]
#[test]
fn real_window_interaction_local_only_skips_when_feature_disabled() {
    let scenario = RealWindowInteractionSmokeScenario::new(NativeEditorInteractionScenario::new(
        "local-only",
        "Local-only real window smoke",
    ));

    let report = run_real_window_interaction_smoke_local_only(scenario);

    assert_eq!(report.status, RealWindowInteractionSmokeStatus::Skipped);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "real_window_feature_not_enabled"));
}

#[cfg(feature = "real-window")]
#[test]
#[ignore]
fn real_window_interaction_feature_builds_local_runner() {
    let scenario = RealWindowInteractionSmokeScenario::new(NativeEditorInteractionScenario::new(
        "local-only-build",
        "Local-only real window smoke build",
    ));

    let report = run_real_window_interaction_smoke_local_only(scenario);

    assert!(
        report.status == RealWindowInteractionSmokeStatus::Passed
            || report.status == RealWindowInteractionSmokeStatus::Failed
    );
    assert_eq!(
        report
            .screenshot
            .as_ref()
            .map(|evidence| evidence.kind.as_str()),
        Some("actual_window_rgba")
    );
}
