use super::*;

fn visual_app() -> NativeEditorApplication {
    NativeEditorApplication::new(NativeEditorWindowConfig::default())
}

fn produce_evidence(
    scenario_id: &str,
) -> (
    NativeEditorApplication,
    EditorVisualRegressionEvidence,
    EditorVisualRegressionBaseline,
) {
    let mut app = visual_app();
    let app_report = app.frame(1280.0, 720.0);
    let draw_plan =
        UiGpuDrawPlan::from_draw_list(app.latest_draw_list()).expect("draw plan should build");
    let evidence = EditorVisualRegressionEvidence::from_app_report_and_draw_plan(
        scenario_id,
        &app_report,
        &draw_plan,
    );
    let baseline = EditorVisualRegressionBaseline::from_evidence(&evidence);
    (app, evidence, baseline)
}

#[test]
fn visual_regression_report_serializes() {
    let (_app, evidence, baseline) = produce_evidence("project-launcher-default");
    let report = EditorVisualRegressionReport {
        schema_version: EDITOR_VISUAL_REGRESSION_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: "project-launcher-default".to_string(),
        title: "Project launcher default".to_string(),
        status: EditorVisualRegressionStatus::Passed,
        backend: "headless-deterministic-visual".to_string(),
        evidence: Some(evidence),
        baseline: Some(baseline),
        diagnostics: Vec::new(),
    };

    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(EDITOR_VISUAL_REGRESSION_REPORT_SCHEMA_VERSION));
    assert!(json.contains("project-launcher-default"));
}

#[test]
fn visual_regression_matching_baseline_passes() {
    let (mut app, _evidence, baseline) = produce_evidence("project-launcher-default");
    let scenario =
        EditorVisualRegressionScenario::new("project-launcher-default", "Project launcher default")
            .with_baseline(baseline);

    let report = EditorVisualRegressionRunner::default().run(&mut app, scenario);

    assert_eq!(report.status, EditorVisualRegressionStatus::Passed);
    assert!(report.diagnostics.is_empty());
    assert!(report.evidence.is_some());
}

#[test]
fn visual_regression_mismatched_baseline_fails() {
    let (mut app, _evidence, mut baseline) = produce_evidence("project-launcher-default");
    baseline.draw_command_count += 1;
    let scenario =
        EditorVisualRegressionScenario::new("project-launcher-default", "Project launcher default")
            .with_baseline(baseline);

    let report = EditorVisualRegressionRunner::default().run(&mut app, scenario);

    assert_eq!(report.status, EditorVisualRegressionStatus::Failed);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.field.as_deref() == Some("draw_command_count")));
}

#[test]
fn visual_regression_missing_baseline_is_reported() {
    let mut app = visual_app();
    let scenario =
        EditorVisualRegressionScenario::new("project-launcher-default", "Project launcher default");

    let report = EditorVisualRegressionRunner::default().run(&mut app, scenario);

    assert_eq!(report.status, EditorVisualRegressionStatus::BaselineMissing);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "visual_regression.baseline_missing"));
}

#[test]
fn visual_regression_app_scenario_produces_evidence() {
    let mut app = visual_app();
    let scenario =
        EditorVisualRegressionScenario::new("project-launcher-default", "Project launcher default")
            .with_surface_size(1024.0, 640.0);

    let report = EditorVisualRegressionRunner::default().run(&mut app, scenario);
    let evidence = report.evidence.expect("evidence should exist");

    assert_eq!(evidence.scenario_id, "project-launcher-default");
    assert_eq!(evidence.surface_width, 1024);
    assert_eq!(evidence.surface_height, 640);
    assert!(evidence.draw_command_count > 0);
    assert!(evidence.hit_region_count > 0);
    assert!(!evidence.structural_hash.is_empty());
}
