use editor_core::{
    command_for_test, CommandStatus, ManualWalkthroughCoverageAnalyzer,
    ManualWalkthroughCoverageInput,
};
use editor_ui_model::{ManualWalkthroughCoverageReport, UiCommandPayload};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_MANUAL_WALKTHROUGH_SCENARIO_ID: &str =
    "complex-shooter-manual-walkthrough-coverage-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterManualWalkthroughCoverageRequest {
    pub project_path: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterManualWalkthroughCoverageRequest {
    pub fn new(project_path: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_manual_walkthrough_coverage(
    request: ComplexShooterManualWalkthroughCoverageRequest,
) -> ManualWalkthroughCoverageReport {
    let mut session = crate::complex_shooter_editor_session();
    let open_result = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_path.display().to_string(),
    }));

    let model = session.build_ui_model();
    let mut report = ManualWalkthroughCoverageAnalyzer::analyze(ManualWalkthroughCoverageInput {
        workspace: &model.project_authoring_workspace,
        workflow: &model.authoring_workflow,
        scenario_id: COMPLEX_SHOOTER_MANUAL_WALKTHROUGH_SCENARIO_ID,
    });

    if open_result.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:sample_project_open_failed".to_string());
        report = ManualWalkthroughCoverageReport::from_operations(
            report.project_id.clone(),
            report.scenario_id.clone(),
            report.operations,
            report.diagnostics,
        );
    }

    let artifact_path = request
        .output_root
        .join("reports")
        .join("manual-walkthrough-coverage-report.json");
    if let Err(error) = write_json(&artifact_path, &report) {
        report.diagnostics.push(format!(
            "fail:manual_walkthrough_report_write_failed:{error}"
        ));
        report = ManualWalkthroughCoverageReport::from_operations(
            report.project_id.clone(),
            report.scenario_id.clone(),
            report.operations,
            report.diagnostics,
        );
    }

    report
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
