use editor_core::{
    command_for_test, CommandStatus, EditorBuildAndRunMode, EditorBuildAndRunReport,
    EditorBuildAndRunStatus,
};
use editor_ui_model::UiCommandPayload;
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationRequest,
    ExportedPlayerProcessVerificationStatus,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-editor-build-and-run-productization-report.v1";
pub const COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_SCENARIO_ID: &str =
    "complex-shooter-editor-build-and-run-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterEditorBuildAndRunStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorBuildAndRunMetrics {
    pub open_project_committed: bool,
    pub command_committed: bool,
    pub editor_report_present: bool,
    pub editor_report_status: String,
    pub export_status: String,
    pub launch_attempted: bool,
    pub launch_started: bool,
    pub editor_headless_verification_status: String,
    pub exported_process_contract_used: bool,
    pub verifier_status: String,
    pub process_exit_code: Option<i32>,
    pub child_player_exit_code: Option<i32>,
    pub child_frames_completed: Option<u64>,
    pub report_panel_provider_present: bool,
}

impl ComplexShooterEditorBuildAndRunMetrics {
    fn empty() -> Self {
        Self {
            open_project_committed: false,
            command_committed: false,
            editor_report_present: false,
            editor_report_status: "none".to_string(),
            export_status: "not_started".to_string(),
            launch_attempted: false,
            launch_started: false,
            editor_headless_verification_status: "not_started".to_string(),
            exported_process_contract_used: false,
            verifier_status: "not_started".to_string(),
            process_exit_code: None,
            child_player_exit_code: None,
            child_frames_completed: None,
            report_panel_provider_present: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorBuildAndRunArtifact {
    pub artifact_id: String,
    pub path: String,
    pub source_domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorBuildAndRunDiagnostic {
    pub severity: String,
    pub code: String,
    pub domain: String,
    pub stage: String,
    pub source_path: Option<String>,
    pub message: String,
    pub next_action: Option<String>,
}

impl ComplexShooterEditorBuildAndRunDiagnostic {
    fn error(
        code: impl Into<String>,
        stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            domain: "editor_build_and_run".to_string(),
            stage: stage.into(),
            source_path: None,
            message: message.into(),
            next_action: None,
        }
    }

    fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    fn with_next_action(mut self, next_action: impl Into<String>) -> Self {
        self.next_action = Some(next_action.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorBuildAndRunReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterEditorBuildAndRunStatus,
    pub project_root: String,
    pub output_root: String,
    pub metrics: ComplexShooterEditorBuildAndRunMetrics,
    pub editor_report: Option<EditorBuildAndRunReport>,
    pub artifacts: Vec<ComplexShooterEditorBuildAndRunArtifact>,
    pub diagnostics: Vec<ComplexShooterEditorBuildAndRunDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterEditorBuildAndRunRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub frames: u64,
}

impl ComplexShooterEditorBuildAndRunRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
            frames: 3,
        }
    }
}

pub fn run_complex_shooter_editor_build_and_run_report(
    request: ComplexShooterEditorBuildAndRunRequest,
) -> ComplexShooterEditorBuildAndRunReport {
    let _ = fs::create_dir_all(request.output_root.join("reports"));
    let mut session = crate::complex_shooter_editor_session();
    let open_project = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_root.display().to_string(),
    }));
    let build_and_run = session.execute_build_and_run_desktop_package_for_test(
        Some("windows-dev".to_string()),
        EditorBuildAndRunMode::HeadlessVerification,
        30_000,
        request.frames.max(1),
    );
    let editor_report = session.last_build_and_run_report().cloned();
    let report_panel = session.build_ui_model().report_panel;
    let mut report = ComplexShooterEditorBuildAndRunReport {
        schema_version: COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_EDITOR_BUILD_AND_RUN_SCENARIO_ID.to_string(),
        status: ComplexShooterEditorBuildAndRunStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        metrics: ComplexShooterEditorBuildAndRunMetrics::empty(),
        editor_report: editor_report.clone(),
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };
    report.metrics.open_project_committed = open_project.status == CommandStatus::Committed;
    report.metrics.command_committed = build_and_run.status == CommandStatus::Committed;
    report.metrics.report_panel_provider_present = report_panel
        .registry
        .descriptors
        .iter()
        .any(|descriptor| descriptor.provider_id == "build.and_run");

    if let Some(editor_report) = &editor_report {
        collect_editor_report_metrics(&mut report, editor_report);
        collect_editor_report_artifacts(&mut report, editor_report);
        for diagnostic in &editor_report.diagnostics {
            report.diagnostics.push(
                ComplexShooterEditorBuildAndRunDiagnostic::error(
                    diagnostic.code.clone(),
                    diagnostic.stage.clone(),
                    diagnostic.message.clone(),
                )
                .with_source_path(diagnostic.path.clone().unwrap_or_default())
                .with_next_action(diagnostic.next_action.clone().unwrap_or_default()),
            );
        }
        if let Some(package_dir) = editor_report.desktop_export.package_dir.as_ref() {
            run_outer_exported_process_verification(&request, &mut report, package_dir);
        }
    } else {
        report.diagnostics.push(
            ComplexShooterEditorBuildAndRunDiagnostic::error(
                "editor_build_and_run_report_missing",
                "editor_service",
                "EditorSession did not produce an EditorBuildAndRunReport.",
            )
            .with_next_action("Inspect editor_core build service command dispatch."),
        );
    }

    if report.metrics.open_project_committed
        && report.metrics.command_committed
        && report.metrics.editor_report_present
        && report.metrics.editor_report_status == "verification_passed"
        && report.metrics.exported_process_contract_used
        && report.metrics.verifier_status == "passed"
        && report.metrics.report_panel_provider_present
    {
        report.status = ComplexShooterEditorBuildAndRunStatus::Passed;
    }
    if report.status != ComplexShooterEditorBuildAndRunStatus::Passed {
        report
            .next_actions
            .push("inspect_editor_build_and_run_report".to_string());
    }

    let report_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-editor-build-and-run-productization-report.json");
    push_artifact(
        &mut report,
        "complex-shooter-editor-build-and-run-report",
        &report_path,
        "project_e2e.editor_build_and_run",
    );
    let _ = write_json(&report_path, &report);
    report
}

fn collect_editor_report_metrics(
    report: &mut ComplexShooterEditorBuildAndRunReport,
    editor_report: &EditorBuildAndRunReport,
) {
    report.metrics.editor_report_present = true;
    report.metrics.editor_report_status = editor_status_name(editor_report.status).to_string();
    report.metrics.export_status = editor_report.desktop_export.status.clone();
    report.metrics.launch_attempted = editor_report.launch.attempted;
    report.metrics.launch_started = editor_report.launch.started;
    report.metrics.editor_headless_verification_status = editor_report.verification.status.clone();
    report.metrics.process_exit_code = editor_report.verification.process_exit_code;
    report.metrics.child_player_exit_code = editor_report.verification.child_player_exit_code;
    report.metrics.child_frames_completed = editor_report.verification.child_frames_completed;
}

fn collect_editor_report_artifacts(
    report: &mut ComplexShooterEditorBuildAndRunReport,
    editor_report: &EditorBuildAndRunReport,
) {
    for artifact_ref in &editor_report.artifacts {
        push_artifact(
            report,
            &artifact_ref.artifact_id,
            Path::new(&artifact_ref.path),
            "editor_core.build_and_run",
        );
    }
    if let Some(path) = &editor_report.verification.child_report_path {
        push_artifact(
            report,
            "editor-build-and-run-child-windowed-player-report",
            Path::new(path),
            "editor_core.build_and_run",
        );
    }
    if let Some(path) = &editor_report.verification.verification_report_path {
        push_artifact(
            report,
            "editor-build-and-run-process-verification-summary",
            Path::new(path),
            "editor_core.build_and_run",
        );
    }
}

fn run_outer_exported_process_verification(
    request: &ComplexShooterEditorBuildAndRunRequest,
    report: &mut ComplexShooterEditorBuildAndRunReport,
    package_dir: &str,
) {
    let verifier_report_path = request
        .output_root
        .join("reports")
        .join("editor-build-and-run-exported-player-process-verification-report.json");
    let verification = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
        exported_package_dir: PathBuf::from(package_dir),
        mode: "headless-gate".to_string(),
        frame_limit: request.frames.max(1),
        report_path: Some(verifier_report_path.clone()),
        timeout_ms: 30_000,
        screenshot: false,
        screenshot_path: None,
    });
    report.metrics.exported_process_contract_used = true;
    report.metrics.verifier_status = match verification.status {
        ExportedPlayerProcessVerificationStatus::Passed => "passed".to_string(),
        ExportedPlayerProcessVerificationStatus::Failed => "failed".to_string(),
        ExportedPlayerProcessVerificationStatus::EnvironmentBlocked => {
            "environment_blocked".to_string()
        }
    };
    report.metrics.process_exit_code = verification.process_exit_code;
    report.metrics.child_player_exit_code = verification.child_player_exit_code;
    report.metrics.child_frames_completed = verification.child_frames_completed;
    push_artifact(
        report,
        "exported-player-process-verification-report",
        &verifier_report_path,
        "runtime_cli.exported_player_verification",
    );
    push_artifact(
        report,
        "outer-child-windowed-player-run-report",
        Path::new(&verification.child_report_path),
        "runtime_cli.exported_player_verification",
    );
    for diagnostic in verification.diagnostics {
        report.diagnostics.push(
            ComplexShooterEditorBuildAndRunDiagnostic::error(
                format!("runtime_cli.{}", diagnostic.code),
                "outer_verification",
                diagnostic.message,
            )
            .with_source_path(diagnostic.path.unwrap_or_default())
            .with_next_action("Inspect exported-player-process-verification-report.json."),
        );
    }
}

fn editor_status_name(status: EditorBuildAndRunStatus) -> &'static str {
    match status {
        EditorBuildAndRunStatus::NotStarted => "not_started",
        EditorBuildAndRunStatus::ExportFailed => "export_failed",
        EditorBuildAndRunStatus::LaunchFailed => "launch_failed",
        EditorBuildAndRunStatus::Launched => "launched",
        EditorBuildAndRunStatus::VerificationPassed => "verification_passed",
        EditorBuildAndRunStatus::VerificationFailed => "verification_failed",
        EditorBuildAndRunStatus::EnvironmentBlocked => "environment_blocked",
    }
}

fn push_artifact(
    report: &mut ComplexShooterEditorBuildAndRunReport,
    artifact_id: &str,
    path: &Path,
    source_domain: &str,
) {
    report
        .artifacts
        .push(ComplexShooterEditorBuildAndRunArtifact {
            artifact_id: artifact_id.to_string(),
            path: path.display().to_string(),
            source_domain: source_domain.to_string(),
        });
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
