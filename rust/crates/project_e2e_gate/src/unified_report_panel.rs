use editor_core::{command_for_test, CommandStatus};
use editor_ui_model::{ReportPanelModel, UiCommandPayload, WorkspaceDomainKind};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-unified-report-panel-productization-report.v1";
pub const COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_SCENARIO_ID: &str =
    "complex-shooter-unified-report-panel-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterUnifiedReportPanelStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterUnifiedReportPanelMetrics {
    pub provider_count: usize,
    pub active_provider_count: usize,
    pub report_count: usize,
    pub evidence_count: usize,
    pub diagnostic_count: usize,
    pub next_action_count: usize,
    pub artifact_count: usize,
    pub ai_context_count: usize,
    pub covered_domain_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedReportPanelProviderEvidence {
    pub provider_id: String,
    pub label: String,
    pub domain: String,
    pub kind: String,
    pub enabled: bool,
    pub report_present: bool,
    pub report_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterUnifiedReportPanelReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterUnifiedReportPanelStatus,
    pub project_root: String,
    pub output_root: String,
    pub selected_report_id: Option<String>,
    pub metrics: ComplexShooterUnifiedReportPanelMetrics,
    pub domain_coverage: Vec<String>,
    pub provider_evidence: Vec<UnifiedReportPanelProviderEvidence>,
    pub report_ids: Vec<String>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl ComplexShooterUnifiedReportPanelReport {
    fn new(project_root: impl Into<String>, output_root: impl Into<String>) -> Self {
        Self {
            schema_version: COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: COMPLEX_SHOOTER_UNIFIED_REPORT_PANEL_SCENARIO_ID.to_string(),
            status: ComplexShooterUnifiedReportPanelStatus::Failed,
            project_root: project_root.into(),
            output_root: output_root.into(),
            selected_report_id: None,
            metrics: ComplexShooterUnifiedReportPanelMetrics::default(),
            domain_coverage: Vec::new(),
            provider_evidence: Vec::new(),
            report_ids: Vec::new(),
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn recompute(&mut self, panel: &ReportPanelModel) {
        self.selected_report_id = panel.selected_report_id.clone();
        self.metrics.provider_count = panel.summary.provider_count;
        self.metrics.active_provider_count = panel.summary.active_provider_count;
        self.metrics.report_count = panel.summary.report_count;
        self.metrics.evidence_count = panel.summary.evidence_count;
        self.metrics.diagnostic_count = panel.summary.diagnostic_count;
        self.metrics.next_action_count = panel.summary.next_action_count;
        self.metrics.artifact_count = panel.summary.artifact_count;
        self.metrics.ai_context_count = panel
            .reports
            .iter()
            .filter(|report| !report.ai_context.report_id.is_empty())
            .count();
        self.domain_coverage = covered_domains(panel);
        self.metrics.covered_domain_count = self.domain_coverage.len();
        self.provider_evidence = panel
            .registry
            .descriptors
            .iter()
            .map(|descriptor| {
                let report = panel
                    .reports
                    .iter()
                    .find(|report| report.provider_id == descriptor.provider_id);
                UnifiedReportPanelProviderEvidence {
                    provider_id: descriptor.provider_id.clone(),
                    label: descriptor.label.clone(),
                    domain: descriptor.domain.as_str().to_string(),
                    kind: descriptor.kind.clone(),
                    enabled: descriptor.enabled,
                    report_present: report.is_some(),
                    report_status: report.map(|report| format!("{:?}", report.status)),
                }
            })
            .collect();
        self.report_ids = panel
            .reports
            .iter()
            .map(|report| report.report_id.clone())
            .collect();
        self.next_actions = panel
            .reports
            .iter()
            .flat_map(|report| report.next_actions.clone())
            .collect();
        self.next_actions.sort();
        self.next_actions.dedup();

        let required_domains = ["build", "play", "asset", "aui", "rule", "prefab", "report"];
        for domain in required_domains {
            if !self.domain_coverage.iter().any(|covered| covered == domain) {
                self.diagnostics
                    .push(format!("missing_domain_coverage:{domain}"));
            }
        }
        let required_providers = [
            "build.export",
            "play.runtime",
            "authoring.asset_browser",
            "authoring.aui",
            "authoring.rule",
            "authoring.prefab",
            "project.patch",
            "authoring.manual_walkthrough",
            "editor.diagnostics",
            "project_e2e.complex_shooter",
        ];
        for provider in required_providers {
            if !self
                .provider_evidence
                .iter()
                .any(|evidence| evidence.provider_id == provider && evidence.report_present)
            {
                self.diagnostics
                    .push(format!("missing_provider_report:{provider}"));
            }
        }
        if self.metrics.ai_context_count != self.metrics.report_count {
            self.diagnostics
                .push("ai_context_count_does_not_match_report_count".to_string());
        }

        self.status = if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("fail:"))
            || self.diagnostics.iter().any(|diagnostic| {
                diagnostic.starts_with("missing_domain_coverage")
                    || diagnostic.starts_with("missing_provider_report")
            }) {
            ComplexShooterUnifiedReportPanelStatus::Failed
        } else if self.metrics.report_count == 0 {
            ComplexShooterUnifiedReportPanelStatus::Partial
        } else {
            ComplexShooterUnifiedReportPanelStatus::Passed
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterUnifiedReportPanelRequest {
    pub project_path: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterUnifiedReportPanelRequest {
    pub fn new(project_path: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_unified_report_panel(
    request: ComplexShooterUnifiedReportPanelRequest,
) -> ComplexShooterUnifiedReportPanelReport {
    let mut report = ComplexShooterUnifiedReportPanelReport::new(
        request.project_path.display().to_string(),
        request.output_root.display().to_string(),
    );
    let mut session = crate::complex_shooter_editor_session();
    let open_result = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_path.display().to_string(),
    }));
    if open_result.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:sample_project_open_failed".to_string());
    }

    let model = session.build_ui_model();
    report.recompute(&model.report_panel);
    if model.project_authoring_workspace.report.report_count
        != model.report_panel.summary.report_count
    {
        report
            .diagnostics
            .push("workspace_report_count_mismatch".to_string());
        report.status = ComplexShooterUnifiedReportPanelStatus::Failed;
    }

    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-unified-report-panel-productization-report.json");
    report.artifacts.push(artifact_path.display().to_string());
    if let Err(error) = write_json(&artifact_path, &report) {
        report.diagnostics.push(format!(
            "fail:unified_report_panel_report_write_failed:{error}"
        ));
        report.status = ComplexShooterUnifiedReportPanelStatus::Failed;
    }

    report
}

fn covered_domains(panel: &ReportPanelModel) -> Vec<String> {
    let mut domains = panel
        .reports
        .iter()
        .map(|report| report.domain.as_str().to_string())
        .collect::<Vec<_>>();
    if panel
        .reports
        .iter()
        .any(|report| report.provider_id == "project.patch")
    {
        domains.push(WorkspaceDomainKind::Report.as_str().to_string());
    }
    domains.sort();
    domains.dedup();
    domains
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
