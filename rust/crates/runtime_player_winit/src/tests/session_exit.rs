use super::*;
#[test]
fn orderly_exit_without_presenting_is_success_but_does_not_hide_errors() {
    let request = NativePlayerWindowRunRequest::headless_surface_gate("package");
    let mut report = NativeWindowHostReport::base(&request);
    report.session_exit_requested = true;
    assert_eq!(report.completion_exit_code(), 0);
    assert_eq!(
        report.present_status,
        NativeWindowPresentStatus::NotPresented
    );
    report.logic_status = "error".into();
    assert_eq!(report.completion_exit_code(), 1);
}
#[test]
fn project_session_terminal_fault_cannot_complete_successfully() {
    let request = NativePlayerWindowRunRequest::headless_surface_gate("package");
    let mut report = NativeWindowHostReport::base(&request);
    report.present_status = NativeWindowPresentStatus::Presented;
    report.mark_runtime_advanced();
    report.project_runtime_session_report = Some(
        ProjectRuntimeSessionFrameReport::reentry_after_fault(8, "dll-consumer"),
    );
    assert!(
        report.has_errors(),
        "terminal project fault must reach Player completion"
    );
    assert_eq!(report.completion_exit_code(), 1);
}

#[test]
fn project_session_fault_diagnostic_survives_exit_and_later_advance() {
    let request = NativePlayerWindowRunRequest::headless_surface_gate("package");
    let mut report = NativeWindowHostReport::base(&request);
    report.present_status = NativeWindowPresentStatus::Presented;
    report.session_exit_requested = true;
    report
        .record_project_runtime_session(Some(ProjectRuntimeSessionFrameReport::new(1, "consumer")));
    assert!(!report.has_errors());
    let fault = ProjectRuntimeSessionFrameReport::reentry_after_fault(8, "consumer");
    report.record_project_runtime_session(Some(fault.clone()));
    report.record_project_runtime_session(Some(fault));
    report.mark_runtime_advanced();
    assert_eq!(report.logic_status, "error");
    assert_eq!(report.completion_exit_code(), 1);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        "project_runtime.session_reentry_after_fault"
    );
    assert_eq!(report.diagnostics[0].layer, "project_runtime_session");
}
