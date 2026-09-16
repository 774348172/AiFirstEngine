use editor_core::{scan_rule_asset_paths, RuleAuthoringService};
use editor_ui_model::{
    ManualWalkthroughCoverageSummary, RuleAuthoringReport, RuleAuthoringStatus,
    RULE_AUTHORING_REPORT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_RULE_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-rule-authoring-productization-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexShooterRuleAuthoringStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterRuleAuthoringReport {
    pub schema_version: String,
    pub status: ComplexShooterRuleAuthoringStatus,
    pub project_root: String,
    pub output_root: String,
    pub rule_asset_count: usize,
    pub runtime_manifest_count: usize,
    pub rule_reports: Vec<RuleAuthoringReport>,
    pub manual_walkthrough_summary: Option<ManualWalkthroughCoverageSummary>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterRuleAuthoringRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub manual_walkthrough_summary: Option<ManualWalkthroughCoverageSummary>,
}

impl ComplexShooterRuleAuthoringRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
            manual_walkthrough_summary: None,
        }
    }
}

pub fn run_complex_shooter_rule_authoring_report(
    request: ComplexShooterRuleAuthoringRequest,
) -> ComplexShooterRuleAuthoringReport {
    let mut diagnostics = Vec::new();
    let mut artifacts = Vec::new();
    let mut next_actions = Vec::new();
    let rule_paths = scan_rule_asset_paths(&request.project_root);
    let runtime_manifest_count = usize::from(
        request
            .project_root
            .join("Rules")
            .join("rule-manifest.json")
            .exists(),
    );
    let mut rule_reports = Vec::new();

    for path in &rule_paths {
        match RuleAuthoringService::build(&request.project_root, path) {
            Ok(report) => rule_reports.push(report),
            Err(message) => {
                diagnostics.push(format!("rule_authoring_build_failed:{path}:{message}"))
            }
        }
    }

    if rule_paths.is_empty() {
        diagnostics.push("rule_authoring_assets_missing".to_string());
        next_actions.push("create_rule_asset".to_string());
        if runtime_manifest_count > 0 {
            next_actions.push("migrate_runtime_manifest_to_rule_authoring_assets".to_string());
        }
    }
    if rule_reports.iter().any(|report| {
        matches!(
            report.status,
            RuleAuthoringStatus::Invalid | RuleAuthoringStatus::Failed
        )
    }) {
        next_actions.push("fix_rule_diagnostics".to_string());
    }
    if rule_reports
        .iter()
        .any(|report| report.schema_version != RULE_AUTHORING_REPORT_SCHEMA_VERSION)
    {
        diagnostics.push("rule_authoring_report_schema_mismatch".to_string());
    }

    let status = if !diagnostics.is_empty() && rule_paths.is_empty() {
        ComplexShooterRuleAuthoringStatus::Partial
    } else if !diagnostics.is_empty() {
        ComplexShooterRuleAuthoringStatus::Failed
    } else {
        ComplexShooterRuleAuthoringStatus::Passed
    };

    let report = ComplexShooterRuleAuthoringReport {
        schema_version: COMPLEX_SHOOTER_RULE_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
        status,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        rule_asset_count: rule_paths.len(),
        runtime_manifest_count,
        rule_reports,
        manual_walkthrough_summary: request.manual_walkthrough_summary,
        artifacts: Vec::new(),
        diagnostics,
        next_actions,
    };

    let report_path = request
        .output_root
        .join("reports")
        .join("rule-authoring-productization-report.json");
    if write_report(&report_path, &report).is_ok() {
        artifacts.push(report_path.display().to_string());
    }

    ComplexShooterRuleAuthoringReport {
        artifacts,
        ..report
    }
}

fn write_report(path: &Path, report: &ComplexShooterRuleAuthoringReport) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create report directory: {error}"))?;
    }
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("Failed to serialize rule authoring report: {error}"))?;
    fs::write(path, json).map_err(|error| format!("Failed to write rule authoring report: {error}"))
}
