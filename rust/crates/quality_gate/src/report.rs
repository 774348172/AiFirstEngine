use serde::{Deserialize, Serialize};

use crate::architecture_policy::ArchitectureDiagnostic;

pub const QUALITY_GATE_REPORT_V1_SCHEMA_VERSION: &str = "quality-gate-report.v1";
pub const QUALITY_GATE_REPORT_SCHEMA_VERSION: &str = "quality-gate-report.v2";
pub const LOCAL_CI_RUN_REPORT_SCHEMA_VERSION: &str = "local-ci-run-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportMode {
    Summary,
    Trace,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityProfile {
    #[default]
    Fast,
    EngineStrict,
    ProjectAdvisory,
    ProjectStrict,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureGateEvidence {
    pub profile: QualityProfile,
    pub policy_digest: String,
    pub inventory_digest: String,
    pub debt_ledger_digest: String,
    pub coverage_ledger_digest: String,
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub changed_subjects: usize,
    pub active_debt: usize,
    pub coverage_pending: usize,
    pub artifact_status: String,
    pub finding_count: usize,
    pub disposition_count: usize,
    pub final_decision: String,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Passed,
    Failed,
    TimedOut,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityStageReport {
    pub id: String,
    pub command_id: Option<String>,
    pub status: StageStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub output_truncated: bool,
    pub next_action: Option<String>,
    pub trace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityDiagnostic {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintGateSummary {
    pub observed_occurrences: usize,
    pub known_entries: usize,
    pub new_entries: usize,
    pub resolved_entries: usize,
    pub stale_entries: usize,
    pub suppression_entries: usize,
    pub new_suppressions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintClassification {
    Known,
    New,
    Resolved,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintItemEvidence {
    pub fingerprint: String,
    pub lint_code: String,
    pub relative_path: String,
    pub ledger_id: Option<String>,
    pub classification: LintClassification,
    pub occurrences: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainEvidence {
    pub expected_release: String,
    pub rustc_version: String,
    pub cargo_version: String,
    pub clippy_version: String,
    pub rustfmt_version: String,
    pub host: String,
    pub matched: bool,
}

impl Default for ToolchainEvidence {
    fn default() -> Self {
        Self {
            expected_release: "1.96.0".to_string(),
            rustc_version: String::new(),
            cargo_version: String::new(),
            clippy_version: String::new(),
            rustfmt_version: String::new(),
            host: String::new(),
            matched: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestMatrixEvidence {
    pub default_workspace_passed: bool,
    pub all_features_workspace_passed: bool,
    pub hygiene_evidence_generated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiEvidence {
    pub adapter: String,
    pub adapter_configured: bool,
    pub execution_scope: String,
    pub run_id: Option<String>,
    pub commit_sha: Option<String>,
    pub execution_status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalCiRunReport {
    pub schema_version: String,
    pub started_at_epoch_seconds: u64,
    pub duration_ms: u64,
    pub run_id: String,
    pub requested_revision: String,
    pub commit_sha: Option<String>,
    pub source_workspace_state: String,
    pub isolated_workspace_state: String,
    pub quality_gate_report_path: Option<String>,
    pub quality_gate_report_digest: Option<String>,
    pub cleanup_status: String,
    pub diagnostics: Vec<QualityDiagnostic>,
    pub passed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneEvidence {
    pub schema_version: String,
    pub files: usize,
    pub total_lines: usize,
    pub recommendation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityGateReport {
    pub schema_version: String,
    pub report_mode: ReportMode,
    #[serde(default)]
    pub fail_fast: bool,
    pub started_at_epoch_seconds: u64,
    pub duration_ms: u64,
    pub workspace_state: String,
    pub workspace_commit_sha: Option<String>,
    pub workspace_digest: String,
    pub lockfile_digest: String,
    pub toolchain_file_digest: String,
    pub manifest_set_digest: String,
    pub ledger_digest: String,
    pub toolchain: ToolchainEvidence,
    pub stages: Vec<QualityStageReport>,
    pub lint: LintGateSummary,
    pub lint_items: Vec<LintItemEvidence>,
    pub test_matrix: TestMatrixEvidence,
    pub hygiene_summary: HygieneEvidence,
    pub ci: CiEvidence,
    pub diagnostics: Vec<QualityDiagnostic>,
    #[serde(default)]
    pub architecture: ArchitectureGateEvidence,
    #[serde(default)]
    pub architecture_diagnostics: Vec<ArchitectureDiagnostic>,
    pub passed: bool,
}

impl Default for QualityGateReport {
    fn default() -> Self {
        Self {
            schema_version: QUALITY_GATE_REPORT_SCHEMA_VERSION.to_string(),
            report_mode: ReportMode::Summary,
            fail_fast: false,
            started_at_epoch_seconds: 0,
            duration_ms: 0,
            workspace_state: "unknown".to_string(),
            workspace_commit_sha: None,
            workspace_digest: String::new(),
            lockfile_digest: String::new(),
            toolchain_file_digest: String::new(),
            manifest_set_digest: String::new(),
            ledger_digest: String::new(),
            toolchain: ToolchainEvidence::default(),
            stages: Vec::new(),
            lint: LintGateSummary::default(),
            lint_items: Vec::new(),
            test_matrix: TestMatrixEvidence::default(),
            hygiene_summary: HygieneEvidence::default(),
            ci: CiEvidence::default(),
            diagnostics: Vec::new(),
            architecture: ArchitectureGateEvidence::default(),
            architecture_diagnostics: Vec::new(),
            passed: false,
        }
    }
}

pub fn read_quality_gate_report(source: &[u8]) -> Result<QualityGateReport, String> {
    let value: serde_json::Value =
        serde_json::from_slice(source).map_err(|error| error.to_string())?;
    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "quality gate report is missing schema_version".to_string())?;
    if !matches!(
        schema,
        QUALITY_GATE_REPORT_V1_SCHEMA_VERSION | QUALITY_GATE_REPORT_SCHEMA_VERSION
    ) {
        return Err(format!("unsupported quality gate report schema {schema:?}"));
    }
    serde_json::from_value(value).map_err(|error| error.to_string())
}

impl QualityGateReport {
    pub fn push_diagnostic(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) {
        self.diagnostics.push(QualityDiagnostic {
            code: code.into(),
            message: message.into(),
            next_action: next_action.into(),
        });
    }
}
