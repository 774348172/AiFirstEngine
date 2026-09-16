use crate::gate::{run_complex_project_e2e_gate, ComplexProjectE2eGateRequest};
use crate::report::{ComplexProjectE2eGateReport, ComplexProjectE2eStatus};
use editor_core::{command_for_test, CommandStatus};
use editor_ui_model::{
    AuthoringStepCompletion, AuthoringStepId, AuthoringStepStatus, EditorUiMode, UiCommandPayload,
    WorkspaceDomainKind, WorkspaceDomainStatus,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const AUTHORING_TO_PLAYABLE_VERTICAL_SLICE_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-authoring-to-playable-vertical-slice-report.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringToPlayableVerticalSliceRequest {
    pub source_project_path: PathBuf,
    pub output_root: PathBuf,
    pub frame_limit: u64,
    pub copy_project_to_output: bool,
    pub include_optional_real_window_step: bool,
}

impl AuthoringToPlayableVerticalSliceRequest {
    pub fn new(source_project_path: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            source_project_path: source_project_path.into(),
            output_root: output_root.into(),
            frame_limit: 6,
            copy_project_to_output: true,
            include_optional_real_window_step: true,
        }
    }

    pub fn sample_from_workspace(workspace_root: impl AsRef<Path>) -> Self {
        let workspace_root = workspace_root.as_ref();
        Self::new(
            workspace_root
                .join("samples")
                .join("complex_shooter_project"),
            workspace_root
                .join("target")
                .join("project_e2e_gate")
                .join("authoring_to_playable"),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthoringToPlayableVerticalSliceStatus {
    Passed,
    Failed,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringToPlayableStep {
    pub step_id: String,
    pub status: AuthoringToPlayableVerticalSliceStatus,
    pub summary: String,
}

impl AuthoringToPlayableStep {
    fn new(
        step_id: impl Into<String>,
        status: AuthoringToPlayableVerticalSliceStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            step_id: step_id.into(),
            status,
            summary: summary.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringWorkspaceDomainEvidence {
    pub domain: String,
    pub status: String,
    pub item_count: usize,
    pub dirty: bool,
    pub active_document_path: Option<String>,
    pub summary: String,
    pub error_count: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringWorkflowStepEvidence {
    pub step_id: String,
    pub status: String,
    pub completion: String,
    pub item_count: usize,
    pub required_for_play: bool,
    pub required_for_build: bool,
    pub issue_count: usize,
    pub next_hint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringToPlayableMetrics {
    pub workspace_domain_count: usize,
    pub ready_or_dirty_domain_count: usize,
    pub workflow_step_count: usize,
    pub workflow_blocking_issue_count: usize,
    pub scene_entity_count: usize,
    pub asset_count: usize,
    pub prefab_count: usize,
    pub rule_count: usize,
    pub input_action_count: usize,
    pub aui_document_count: usize,
    pub runtime_package_entity_count: usize,
    pub frames_run: u64,
    pub present_count: u64,
    pub draw_item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringToPlayableDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl AuthoringToPlayableDiagnostic {
    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoringToPlayableVerticalSliceReport {
    pub schema_version: String,
    pub gate_id: String,
    pub status: AuthoringToPlayableVerticalSliceStatus,
    pub source_project_path: String,
    pub working_project_path: String,
    pub output_root: String,
    pub editor_mode: Option<EditorUiMode>,
    pub project_id: Option<String>,
    pub active_scene_id: Option<String>,
    pub can_play: bool,
    pub can_build: bool,
    pub steps: Vec<AuthoringToPlayableStep>,
    pub workspace_domains: Vec<AuthoringWorkspaceDomainEvidence>,
    pub workflow_steps: Vec<AuthoringWorkflowStepEvidence>,
    pub metrics: AuthoringToPlayableMetrics,
    pub e2e_report: Option<ComplexProjectE2eGateReport>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<AuthoringToPlayableDiagnostic>,
    pub next_actions: Vec<String>,
}

impl AuthoringToPlayableVerticalSliceReport {
    fn new(
        source_project_path: impl Into<String>,
        working_project_path: impl Into<String>,
        output_root: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: AUTHORING_TO_PLAYABLE_VERTICAL_SLICE_REPORT_SCHEMA_VERSION.to_string(),
            gate_id: "complex-shooter-authoring-to-playable-vertical-slice-v1".to_string(),
            status: AuthoringToPlayableVerticalSliceStatus::Failed,
            source_project_path: source_project_path.into(),
            working_project_path: working_project_path.into(),
            output_root: output_root.into(),
            editor_mode: None,
            project_id: None,
            active_scene_id: None,
            can_play: false,
            can_build: false,
            steps: Vec::new(),
            workspace_domains: Vec::new(),
            workflow_steps: Vec::new(),
            metrics: AuthoringToPlayableMetrics::default(),
            e2e_report: None,
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn recompute_status(&mut self) {
        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
            || self
                .steps
                .iter()
                .any(|step| step.status == AuthoringToPlayableVerticalSliceStatus::Failed)
        {
            self.status = AuthoringToPlayableVerticalSliceStatus::Failed;
        } else if self
            .steps
            .iter()
            .any(|step| step.status == AuthoringToPlayableVerticalSliceStatus::Partial)
        {
            self.status = AuthoringToPlayableVerticalSliceStatus::Partial;
        } else {
            self.status = AuthoringToPlayableVerticalSliceStatus::Passed;
        }
    }
}

pub fn run_authoring_to_playable_vertical_slice(
    request: AuthoringToPlayableVerticalSliceRequest,
) -> AuthoringToPlayableVerticalSliceReport {
    let working_project_path = if request.copy_project_to_output {
        request.output_root.join("WorkingProject")
    } else {
        request.source_project_path.clone()
    };
    let mut report = AuthoringToPlayableVerticalSliceReport::new(
        request.source_project_path.display().to_string(),
        working_project_path.display().to_string(),
        request.output_root.display().to_string(),
    );

    if let Err(error) = fs::create_dir_all(&request.output_root) {
        report.diagnostics.push(
            AuthoringToPlayableDiagnostic::error(
                "VerticalSliceOutputCreateFailed",
                format!("failed to create output directory: {error}"),
            )
            .with_path(request.output_root.display().to_string()),
        );
        finalize_vertical_slice_report(&mut report);
        return report;
    }

    if request.copy_project_to_output {
        if let Err(error) = copy_project_dir(&request.source_project_path, &working_project_path) {
            report.diagnostics.push(
                AuthoringToPlayableDiagnostic::error(
                    "VerticalSliceProjectCopyFailed",
                    format!("failed to copy project for vertical slice: {error}"),
                )
                .with_path(working_project_path.display().to_string()),
            );
            finalize_vertical_slice_report(&mut report);
            return report;
        }
        report.steps.push(AuthoringToPlayableStep::new(
            "copy-project-working-set",
            AuthoringToPlayableVerticalSliceStatus::Passed,
            "Copied sample project into vertical-slice working directory.",
        ));
    }

    collect_editor_authoring_evidence(&working_project_path, &mut report);
    if report.steps.iter().any(|step| {
        step.step_id == "editor-open-project"
            && step.status == AuthoringToPlayableVerticalSliceStatus::Failed
    }) {
        finalize_vertical_slice_report(&mut report);
        return report;
    }

    let mut e2e_request = ComplexProjectE2eGateRequest::new(
        &working_project_path,
        request.output_root.join("PlayableExport"),
    );
    e2e_request.frame_limit = request.frame_limit.max(1);
    e2e_request.include_optional_real_window_step = request.include_optional_real_window_step;
    e2e_request.strict_assembly = true;
    let e2e_report = run_complex_project_e2e_gate(e2e_request);
    merge_e2e_report(&mut report, e2e_report);

    finalize_vertical_slice_report(&mut report);
    report
}

fn collect_editor_authoring_evidence(
    project_path: &Path,
    report: &mut AuthoringToPlayableVerticalSliceReport,
) {
    let mut session = crate::complex_shooter_editor_session();
    let open_result = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: project_path.display().to_string(),
    }));
    if open_result.status != CommandStatus::Committed {
        report.steps.push(AuthoringToPlayableStep::new(
            "editor-open-project",
            AuthoringToPlayableVerticalSliceStatus::Failed,
            format!(
                "EditorSession failed to open project: {:?}",
                open_result.status
            ),
        ));
        for diagnostic in open_result.diagnostics {
            report.diagnostics.push(AuthoringToPlayableDiagnostic {
                severity: format!("{:?}", diagnostic.severity).to_ascii_lowercase(),
                code: diagnostic.code,
                message: diagnostic.message,
                path: diagnostic.path,
            });
        }
        return;
    }

    report.steps.push(AuthoringToPlayableStep::new(
        "editor-open-project",
        AuthoringToPlayableVerticalSliceStatus::Passed,
        "EditorSession opened project and loaded the default scene.",
    ));

    let model = session.build_ui_model();
    report.editor_mode = Some(model.mode.clone());
    report.project_id = model.project_authoring_workspace.project_id.clone();
    report.active_scene_id = model.project_authoring_workspace.active_scene_id.clone();
    report.can_play = model.authoring_workflow.can_play;
    report.can_build = model.authoring_workflow.can_build;

    report.workspace_domains = model
        .project_authoring_workspace
        .domains
        .iter()
        .map(|domain| AuthoringWorkspaceDomainEvidence {
            domain: domain.kind.as_str().to_string(),
            status: workspace_status_name(domain.status).to_string(),
            item_count: domain.item_count,
            dirty: domain.dirty,
            active_document_path: domain.active_document_path.clone(),
            summary: domain.summary.clone(),
            error_count: domain.diagnostics.error_count,
            warning_count: domain.diagnostics.warning_count,
        })
        .collect();
    report.workflow_steps = model
        .authoring_workflow
        .steps
        .iter()
        .map(|step| AuthoringWorkflowStepEvidence {
            step_id: step.id.as_str().to_string(),
            status: workflow_status_name(step.status).to_string(),
            completion: workflow_completion_name(step.completion).to_string(),
            item_count: step.item_count,
            required_for_play: step.is_required_for_play,
            required_for_build: step.is_required_for_build,
            issue_count: step.issues.len(),
            next_hint: step.next_hint.clone(),
        })
        .collect();

    report.metrics.workspace_domain_count = report.workspace_domains.len();
    report.metrics.ready_or_dirty_domain_count = model
        .project_authoring_workspace
        .domains
        .iter()
        .filter(|domain| {
            matches!(
                domain.status,
                WorkspaceDomainStatus::Ready
                    | WorkspaceDomainStatus::Dirty
                    | WorkspaceDomainStatus::Warning
            )
        })
        .count();
    report.metrics.workflow_step_count = report.workflow_steps.len();
    report.metrics.workflow_blocking_issue_count = model.authoring_workflow.blocking_issues.len();
    report.metrics.scene_entity_count = workspace_item_count(&model, WorkspaceDomainKind::Scene);
    report.metrics.asset_count = workspace_item_count(&model, WorkspaceDomainKind::Asset);
    report.metrics.prefab_count = workspace_item_count(&model, WorkspaceDomainKind::Prefab);
    report.metrics.rule_count = workspace_item_count(&model, WorkspaceDomainKind::Rule);
    report.metrics.input_action_count = workspace_item_count(&model, WorkspaceDomainKind::Input);
    report.metrics.aui_document_count = workspace_item_count(&model, WorkspaceDomainKind::Aui);

    validate_editor_readiness(&model, report);
}

fn validate_editor_readiness(
    model: &editor_ui_model::EditorUiModel,
    report: &mut AuthoringToPlayableVerticalSliceReport,
) {
    if model.mode != EditorUiMode::AuthoringWorkspace {
        report
            .diagnostics
            .push(AuthoringToPlayableDiagnostic::error(
                "EditorDidNotEnterAuthoringWorkspace",
                format!("expected AuthoringWorkspace, got {:?}", model.mode),
            ));
    }

    for required in [
        WorkspaceDomainKind::Project,
        WorkspaceDomainKind::Asset,
        WorkspaceDomainKind::Scene,
        WorkspaceDomainKind::Prefab,
        WorkspaceDomainKind::Rule,
        WorkspaceDomainKind::Input,
        WorkspaceDomainKind::Aui,
        WorkspaceDomainKind::Build,
        WorkspaceDomainKind::Report,
    ] {
        let Some(domain) = model
            .project_authoring_workspace
            .domains
            .iter()
            .find(|domain| domain.kind == required)
        else {
            report
                .diagnostics
                .push(AuthoringToPlayableDiagnostic::error(
                    "AuthoringDomainMissing",
                    format!("missing workspace domain {}", required.as_str()),
                ));
            continue;
        };
        if domain.status == WorkspaceDomainStatus::NotConfigured {
            report
                .diagnostics
                .push(AuthoringToPlayableDiagnostic::error(
                    "AuthoringDomainNotConfigured",
                    format!("workspace domain {} is not configured", required.as_str()),
                ));
        }
    }

    for required in [
        AuthoringStepId::Project,
        AuthoringStepId::Assets,
        AuthoringStepId::Scene,
        AuthoringStepId::Rules,
        AuthoringStepId::Input,
        AuthoringStepId::Aui,
    ] {
        if model.authoring_workflow.step(required).is_none() {
            report
                .diagnostics
                .push(AuthoringToPlayableDiagnostic::error(
                    "AuthoringWorkflowStepMissing",
                    format!("missing authoring workflow step {}", required.as_str()),
                ));
        }
    }

    let status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.starts_with("Authoring"))
    {
        AuthoringToPlayableVerticalSliceStatus::Failed
    } else if !model.authoring_workflow.can_build {
        report.next_actions.push(
            "Resolve authoring workflow build blockers before exporting the project.".to_string(),
        );
        AuthoringToPlayableVerticalSliceStatus::Failed
    } else {
        if !model.authoring_workflow.can_play {
            report.next_actions.push(
                "Editor preview Play is not ready until a runtime package is loaded or built; the vertical slice verifies playability through the export/player chain."
                    .to_string(),
            );
        }
        AuthoringToPlayableVerticalSliceStatus::Passed
    };

    report.steps.push(AuthoringToPlayableStep::new(
        "editor-authoring-readiness",
        status,
        format!(
            "domains={} workflow_steps={} can_play={} can_build={}",
            report.workspace_domains.len(),
            report.workflow_steps.len(),
            model.authoring_workflow.can_play,
            model.authoring_workflow.can_build
        ),
    ));
}

fn merge_e2e_report(
    report: &mut AuthoringToPlayableVerticalSliceReport,
    e2e_report: ComplexProjectE2eGateReport,
) {
    report.metrics.runtime_package_entity_count = e2e_report.metrics.runtime_package_entity_count;
    report.metrics.frames_run = e2e_report.metrics.frames_run;
    report.metrics.present_count = e2e_report.metrics.present_count;
    report.metrics.draw_item_count = e2e_report.metrics.draw_item_count;
    report.artifacts.extend(
        e2e_report
            .artifacts
            .iter()
            .map(|artifact| artifact.path.clone()),
    );
    report
        .diagnostics
        .extend(
            e2e_report
                .diagnostics
                .iter()
                .map(|diagnostic| AuthoringToPlayableDiagnostic {
                    severity: diagnostic.severity.clone(),
                    code: diagnostic.code.clone(),
                    message: diagnostic.message.clone(),
                    path: diagnostic.path.clone(),
                }),
        );
    for gap in &e2e_report.gaps {
        report.next_actions.push(format!(
            "{}: {} -> {}",
            gap.gap_id, gap.summary, gap.next_action
        ));
    }
    let status = match e2e_report.status {
        ComplexProjectE2eStatus::Passed => AuthoringToPlayableVerticalSliceStatus::Passed,
        ComplexProjectE2eStatus::Failed => AuthoringToPlayableVerticalSliceStatus::Failed,
        ComplexProjectE2eStatus::Partial | ComplexProjectE2eStatus::Skipped => {
            AuthoringToPlayableVerticalSliceStatus::Partial
        }
    };
    report.steps.push(AuthoringToPlayableStep::new(
        "existing-e2e-export-player-chain",
        status,
        format!(
            "e2e status={:?} runtime_entities={} frames={} present={} draw_items={}",
            e2e_report.status,
            e2e_report.metrics.runtime_package_entity_count,
            e2e_report.metrics.frames_run,
            e2e_report.metrics.present_count,
            e2e_report.metrics.draw_item_count
        ),
    ));
    report.e2e_report = Some(e2e_report);
}

fn finalize_vertical_slice_report(report: &mut AuthoringToPlayableVerticalSliceReport) {
    let report_path = PathBuf::from(&report.output_root)
        .join("reports")
        .join("authoring-to-playable-vertical-slice-report.json");
    report.recompute_status();
    if let Err(error) = write_json(&report_path, report) {
        report.diagnostics.push(
            AuthoringToPlayableDiagnostic::error(
                "VerticalSliceReportWriteFailed",
                format!("failed to write vertical slice report: {error}"),
            )
            .with_path(report_path.display().to_string()),
        );
        report.recompute_status();
    } else {
        report.artifacts.push(report_path.display().to_string());
    }
}

fn workspace_item_count(
    model: &editor_ui_model::EditorUiModel,
    kind: WorkspaceDomainKind,
) -> usize {
    model
        .project_authoring_workspace
        .domains
        .iter()
        .find(|domain| domain.kind == kind)
        .map_or(0, |domain| domain.item_count)
}

fn workspace_status_name(status: WorkspaceDomainStatus) -> &'static str {
    match status {
        WorkspaceDomainStatus::NotConfigured => "not_configured",
        WorkspaceDomainStatus::Empty => "empty",
        WorkspaceDomainStatus::Ready => "ready",
        WorkspaceDomainStatus::Dirty => "dirty",
        WorkspaceDomainStatus::Warning => "warning",
        WorkspaceDomainStatus::Error => "error",
    }
}

fn workflow_status_name(status: AuthoringStepStatus) -> &'static str {
    match status {
        AuthoringStepStatus::NotAvailable => "not_available",
        AuthoringStepStatus::Empty => "empty",
        AuthoringStepStatus::NeedsAttention => "needs_attention",
        AuthoringStepStatus::Ready => "ready",
        AuthoringStepStatus::Dirty => "dirty",
        AuthoringStepStatus::Running => "running",
        AuthoringStepStatus::Blocked => "blocked",
        AuthoringStepStatus::Failed => "failed",
        AuthoringStepStatus::Complete => "complete",
    }
}

fn workflow_completion_name(completion: AuthoringStepCompletion) -> &'static str {
    match completion {
        AuthoringStepCompletion::Missing => "missing",
        AuthoringStepCompletion::Partial => "partial",
        AuthoringStepCompletion::Ready => "ready",
        AuthoringStepCompletion::Blocked => "blocked",
        AuthoringStepCompletion::Complete => "complete",
    }
}

fn copy_project_dir(source: &Path, destination: &Path) -> std::io::Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    copy_dir_recursive(source, destination)
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
