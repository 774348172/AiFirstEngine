use super::*;

#[test]
fn report_serializes_and_reports_surface_loss() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let window =
        HeadlessWindowBackend::create_window(&NativeEditorWindowConfig::default()).snapshot();
    let mut surface = HeadlessSurfaceBackend::create_surface();
    surface.lose_surface("surface_lost");
    let report = build_real_window_gate_report(
        window,
        surface.snapshot(),
        None,
        None,
        &draw_list,
        None,
        Vec::new(),
    );
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "surface_lost"));
    let json = serde_json::to_string(&report).expect("report should serialize");
    assert!(json.contains("real-window-gate-report.v1"));
}

#[test]
fn real_native_editor_window_report_serializes() {
    let mut report = RealNativeEditorWindowReport::new("headless");
    report.window_created = true;
    report.surface_created = true;
    report.surface_configured = true;
    report.device_created = true;
    report.present_status = "presented".to_string();

    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(REAL_NATIVE_EDITOR_WINDOW_REPORT_SCHEMA_VERSION));
    assert!(json.contains("presented"));
}

#[test]
fn real_native_editor_window_report_marks_environment_blocked() {
    let report =
        RealNativeEditorWindowReport::environment_blocked("winit-wgpu", "os policy blocked");

    assert_eq!(report.present_status, "environment_blocked");
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "environment_blocked"));
}

#[test]
fn ui_gpu_draw_plan_counts_panel_and_viewport_rects() {
    let model = fixture_model();
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");

    assert!(plan.rect_count >= 5);
    assert_eq!(plan.viewport_slot_count, 1);
    assert_eq!(plan.draw_command_count, draw_list.commands.len());
}

#[test]
fn ui_gpu_draw_plan_is_stable_for_same_ui_model() {
    let model = fixture_model();
    let draw_list_a = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let draw_list_b = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));

    let plan_a = UiGpuDrawPlan::from_draw_list(&draw_list_a).expect("draw plan a");
    let plan_b = UiGpuDrawPlan::from_draw_list(&draw_list_b).expect("draw plan b");

    assert_eq!(plan_a, plan_b);
}

#[test]
fn ui_gpu_draw_plan_rejects_empty_surface() {
    let draw_list = UiDrawList {
        revision: 1,
        frame: 1,
        surface_width: 0.0,
        surface_height: 720.0,
        commands: Vec::new(),
        hit_regions: Vec::new(),
    };

    assert_eq!(
        UiGpuDrawPlan::from_draw_list(&draw_list),
        Err("ui_gpu_draw_plan.empty_surface".to_string())
    );
}
