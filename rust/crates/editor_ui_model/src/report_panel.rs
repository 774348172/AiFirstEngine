use serde::{Deserialize, Serialize};

use super::{DiagnosticSeverity, WorkspaceDomainKind};

pub const REPORT_PANEL_SCHEMA_VERSION: &str = "report-panel.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPanelModel {
    pub schema_version: String,
    pub registry: ReportRegistrySummary,
    pub filters: ReportPanelFilters,
    pub selected_report_id: Option<String>,
    pub selected_report: Option<UnifiedReportEntry>,
    pub reports: Vec<UnifiedReportEntry>,
    pub summary: ReportPanelSummary,
    pub empty_message: String,
}

impl ReportPanelModel {
    pub fn empty() -> Self {
        Self {
            schema_version: REPORT_PANEL_SCHEMA_VERSION.to_string(),
            registry: ReportRegistrySummary::default(),
            filters: ReportPanelFilters::default(),
            selected_report_id: None,
            selected_report: None,
            reports: Vec::new(),
            summary: ReportPanelSummary::default(),
            empty_message: "No reports are available yet.".to_string(),
        }
    }

    pub fn from_reports(
        registry: ReportRegistrySummary,
        mut reports: Vec<UnifiedReportEntry>,
        selected_report_id: Option<String>,
    ) -> Self {
        reports.sort_by(|left, right| {
            severity_rank(right.max_severity)
                .cmp(&severity_rank(left.max_severity))
                .then_with(|| left.domain.as_str().cmp(right.domain.as_str()))
                .then_with(|| left.title.cmp(&right.title))
        });
        let selected_report_id =
            selected_report_id.or_else(|| reports.first().map(|report| report.report_id.clone()));
        let selected_report = selected_report_id.as_ref().and_then(|id| {
            reports
                .iter()
                .find(|report| &report.report_id == id)
                .cloned()
        });
        let summary = ReportPanelSummary::from_reports(&registry, &reports);
        let empty_message = if reports.is_empty() {
            "No registered report provider produced a report.".to_string()
        } else {
            String::new()
        };
        Self {
            schema_version: REPORT_PANEL_SCHEMA_VERSION.to_string(),
            registry,
            filters: ReportPanelFilters::default(),
            selected_report_id,
            selected_report,
            reports,
            summary,
            empty_message,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRegistrySummary {
    pub provider_count: usize,
    pub active_provider_count: usize,
    pub descriptors: Vec<ReportDescriptor>,
}

impl ReportRegistrySummary {
    pub fn from_descriptors(descriptors: Vec<ReportDescriptor>) -> Self {
        let active_provider_count = descriptors
            .iter()
            .filter(|descriptor| descriptor.enabled)
            .count();
        Self {
            provider_count: descriptors.len(),
            active_provider_count,
            descriptors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportDescriptor {
    pub provider_id: String,
    pub label: String,
    pub domain: WorkspaceDomainKind,
    pub kind: String,
    pub source_kind: ReportSourceKind,
    pub supported_schema_versions: Vec<String>,
    pub capabilities: Vec<ReportCapability>,
    pub enabled: bool,
}

impl ReportDescriptor {
    pub fn new(
        provider_id: impl Into<String>,
        label: impl Into<String>,
        domain: WorkspaceDomainKind,
        kind: impl Into<String>,
        source_kind: ReportSourceKind,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            label: label.into(),
            domain,
            kind: kind.into(),
            source_kind,
            supported_schema_versions: Vec::new(),
            capabilities: vec![
                ReportCapability::OpenRawReport,
                ReportCapability::RevealPath,
                ReportCapability::CopyAiContext,
                ReportCapability::OpenRelatedArtifact,
                ReportCapability::FilterBySeverity,
            ],
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnifiedReportEntry {
    pub report_id: String,
    pub provider_id: String,
    pub title: String,
    pub domain: WorkspaceDomainKind,
    pub kind: String,
    pub status: ReportStatus,
    pub max_severity: DiagnosticSeverity,
    pub source_kind: ReportSourceKind,
    pub source_path: Option<String>,
    pub report_path: Option<String>,
    pub schema_version: Option<String>,
    pub summary: String,
    pub updated_at_label: String,
    pub evidence: Vec<EvidenceEntry>,
    pub diagnostics: Vec<EvidenceEntry>,
    pub next_actions: Vec<String>,
    pub artifacts: Vec<ReportArtifactRef>,
    pub ai_context: ReportAiContext,
}

impl UnifiedReportEntry {
    pub fn new(
        report_id: impl Into<String>,
        provider_id: impl Into<String>,
        title: impl Into<String>,
        domain: WorkspaceDomainKind,
        kind: impl Into<String>,
    ) -> Self {
        let report_id = report_id.into();
        let provider_id = provider_id.into();
        Self {
            report_id: report_id.clone(),
            provider_id: provider_id.clone(),
            title: title.into(),
            domain,
            kind: kind.into(),
            status: ReportStatus::Unknown,
            max_severity: DiagnosticSeverity::Info,
            source_kind: ReportSourceKind::Derived,
            source_path: None,
            report_path: None,
            schema_version: None,
            summary: String::new(),
            updated_at_label: "current".to_string(),
            evidence: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
            artifacts: Vec::new(),
            ai_context: ReportAiContext {
                report_id,
                provider_id,
                domain,
                status: ReportStatus::Unknown,
                max_severity: DiagnosticSeverity::Info,
                summary: String::new(),
                top_diagnostics: Vec::new(),
                next_actions: Vec::new(),
                source_paths: Vec::new(),
                artifact_paths: Vec::new(),
                suggested_patch_scope: None,
            },
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self.ai_context.summary = self.summary.clone();
        self
    }

    pub fn finalize_counts(mut self) -> Self {
        self.max_severity = max_severity_for_entries(&self.evidence, &self.diagnostics);
        self.ai_context.status = self.status;
        self.ai_context.max_severity = self.max_severity;
        self.ai_context.next_actions = self.next_actions.clone();
        self.ai_context.top_diagnostics = self
            .diagnostics
            .iter()
            .take(5)
            .map(|diagnostic| {
                format!(
                    "{}:{}:{}",
                    diagnostic.severity.label(),
                    diagnostic.code.as_deref().unwrap_or("diagnostic"),
                    diagnostic.message
                )
            })
            .collect();
        self.ai_context.source_paths = self
            .evidence
            .iter()
            .chain(self.diagnostics.iter())
            .filter_map(|entry| entry.source_path.clone())
            .collect();
        self.ai_context.source_paths.sort();
        self.ai_context.source_paths.dedup();
        self.ai_context.artifact_paths = self
            .artifacts
            .iter()
            .map(|artifact| artifact.path.clone())
            .collect();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceEntry {
    pub evidence_id: String,
    pub title: String,
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub message: String,
    pub domain: WorkspaceDomainKind,
    pub stage: Option<String>,
    pub source_path: Option<String>,
    pub entity_id: Option<String>,
    pub node_id: Option<String>,
    pub command_id: Option<String>,
    pub request_id: Option<String>,
    pub trace_entry_id: Option<String>,
    pub suggested_action: Option<String>,
    pub next_actions: Vec<String>,
    pub related_artifacts: Vec<String>,
    pub raw_payload_summary: Option<String>,
}

impl EvidenceEntry {
    pub fn diagnostic(
        evidence_id: impl Into<String>,
        domain: WorkspaceDomainKind,
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let code = code.into();
        Self {
            evidence_id: evidence_id.into(),
            title: code.clone(),
            severity,
            code: Some(code),
            message: message.into(),
            domain,
            stage: None,
            source_path: None,
            entity_id: None,
            node_id: None,
            command_id: None,
            request_id: None,
            trace_entry_id: None,
            suggested_action: None,
            next_actions: Vec::new(),
            related_artifacts: Vec::new(),
            raw_payload_summary: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportAiContext {
    pub report_id: String,
    pub provider_id: String,
    pub domain: WorkspaceDomainKind,
    pub status: ReportStatus,
    pub max_severity: DiagnosticSeverity,
    pub summary: String,
    pub top_diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
    pub source_paths: Vec<String>,
    pub artifact_paths: Vec<String>,
    pub suggested_patch_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportArtifactRef {
    pub artifact_id: String,
    pub label: String,
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPanelFilters {
    pub domains: Vec<WorkspaceDomainKind>,
    pub severities: Vec<DiagnosticSeverity>,
    pub statuses: Vec<ReportStatus>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPanelSummary {
    pub provider_count: usize,
    pub active_provider_count: usize,
    pub report_count: usize,
    pub evidence_count: usize,
    pub diagnostic_count: usize,
    pub next_action_count: usize,
    pub artifact_count: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
}

impl ReportPanelSummary {
    pub fn from_reports(registry: &ReportRegistrySummary, reports: &[UnifiedReportEntry]) -> Self {
        let mut summary = Self {
            provider_count: registry.provider_count,
            active_provider_count: registry.active_provider_count,
            report_count: reports.len(),
            ..Self::default()
        };
        for report in reports {
            summary.evidence_count += report.evidence.len();
            summary.diagnostic_count += report.diagnostics.len();
            summary.next_action_count += report.next_actions.len();
            summary.artifact_count += report.artifacts.len();
            for entry in report.evidence.iter().chain(report.diagnostics.iter()) {
                match entry.severity {
                    DiagnosticSeverity::Info => summary.info_count += 1,
                    DiagnosticSeverity::Warning => summary.warning_count += 1,
                    DiagnosticSeverity::Error => summary.error_count += 1,
                }
            }
        }
        summary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportStatus {
    Passed,
    Partial,
    Failed,
    Skipped,
    Empty,
    Unknown,
}

impl Default for ReportStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportSourceKind {
    InMemory,
    Artifact,
    Derived,
    Placeholder,
}

impl Default for ReportSourceKind {
    fn default() -> Self {
        Self::Derived
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportCapability {
    OpenRawReport,
    RevealPath,
    CopyAiContext,
    OpenRelatedArtifact,
    FilterBySeverity,
    CreatePatchFromEvidence,
    RunRepairLoop,
    CompareReportHistory,
}

fn max_severity_for_entries(
    evidence: &[EvidenceEntry],
    diagnostics: &[EvidenceEntry],
) -> DiagnosticSeverity {
    evidence
        .iter()
        .chain(diagnostics.iter())
        .map(|entry| entry.severity)
        .max_by_key(|severity| severity_rank(*severity))
        .unwrap_or(DiagnosticSeverity::Info)
}

fn severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Info => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Error => 2,
    }
}

trait SeverityLabel {
    fn label(self) -> &'static str;
}

impl SeverityLabel for DiagnosticSeverity {
    fn label(self) -> &'static str {
        match self {
            DiagnosticSeverity::Info => "info",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Error => "error",
        }
    }
}
