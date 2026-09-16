use editor_core::{command_for_test, CommandResult, CommandStatus, EditorSession};
use editor_ui_model::{ManualWalkthroughCoverageSummary, UiCommandPayload};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const COMPLEX_SHOOTER_AUI_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-aui-authoring-productization-report.v1";
pub const COMPLEX_SHOOTER_AUI_AUTHORING_SCENARIO_ID: &str =
    "complex-shooter-aui-authoring-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterAuiAuthoringStatus {
    Passed,
    Partial,
    Failed,
}

impl ComplexShooterAuiAuthoringStatus {
    fn from_command_status(status: CommandStatus) -> Self {
        match status {
            CommandStatus::Committed | CommandStatus::Validated => Self::Passed,
            CommandStatus::Rejected => Self::Partial,
            CommandStatus::Pending | CommandStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiAuthoringCommandEvidence {
    pub command_id: String,
    pub status: ComplexShooterAuiAuthoringStatus,
    pub diagnostic_codes: Vec<String>,
    pub state_change_kinds: Vec<String>,
}

impl AuiAuthoringCommandEvidence {
    fn from_result(result: &CommandResult) -> Self {
        Self {
            command_id: result.command_id.clone(),
            status: ComplexShooterAuiAuthoringStatus::from_command_status(result.status),
            diagnostic_codes: result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
            state_change_kinds: result
                .state_changes
                .iter()
                .map(|change| change.kind.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiAuthoringDocumentEvidence {
    pub source_path: String,
    pub source_shape: String,
    pub document_id: Option<String>,
    pub node_count: usize,
    pub binding_count: usize,
    pub action_count: usize,
    pub validation_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiAuthoringMetrics {
    pub sample_document_count: usize,
    pub sample_canonical_document_count: usize,
    pub sample_legacy_document_count: usize,
    pub smoke_command_count: usize,
    pub smoke_committed_command_count: usize,
    pub preview_partial_count: usize,
    pub preview_failed_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiAuthoringReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAuiAuthoringStatus,
    pub project_root: String,
    pub output_root: String,
    pub sample_documents: Vec<AuiAuthoringDocumentEvidence>,
    pub smoke_project_root: Option<String>,
    pub smoke_commands: Vec<AuiAuthoringCommandEvidence>,
    pub manual_walkthrough_summary: Option<ManualWalkthroughCoverageSummary>,
    pub metrics: ComplexShooterAuiAuthoringMetrics,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl ComplexShooterAuiAuthoringReport {
    fn new(project_root: impl Into<String>, output_root: impl Into<String>) -> Self {
        Self {
            schema_version: COMPLEX_SHOOTER_AUI_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: COMPLEX_SHOOTER_AUI_AUTHORING_SCENARIO_ID.to_string(),
            status: ComplexShooterAuiAuthoringStatus::Failed,
            project_root: project_root.into(),
            output_root: output_root.into(),
            sample_documents: Vec::new(),
            smoke_project_root: None,
            smoke_commands: Vec::new(),
            manual_walkthrough_summary: None,
            metrics: ComplexShooterAuiAuthoringMetrics {
                sample_document_count: 0,
                sample_canonical_document_count: 0,
                sample_legacy_document_count: 0,
                smoke_command_count: 0,
                smoke_committed_command_count: 0,
                preview_partial_count: 0,
                preview_failed_count: 0,
            },
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn recompute(&mut self) {
        self.metrics.sample_document_count = self.sample_documents.len();
        self.metrics.sample_canonical_document_count = self
            .sample_documents
            .iter()
            .filter(|document| document.source_shape == "runtime_aui_document")
            .count();
        self.metrics.sample_legacy_document_count = self
            .sample_documents
            .iter()
            .filter(|document| document.source_shape == "legacy_authoring_tree")
            .count();
        self.metrics.smoke_command_count = self.smoke_commands.len();
        self.metrics.smoke_committed_command_count = self
            .smoke_commands
            .iter()
            .filter(|command| command.status == ComplexShooterAuiAuthoringStatus::Passed)
            .count();
        self.metrics.preview_partial_count = self
            .smoke_commands
            .iter()
            .filter(|command| {
                command.command_id == "preview_aui_overlay"
                    && command
                        .diagnostic_codes
                        .iter()
                        .any(|code| code.contains("glyph_not_proven"))
            })
            .count();
        self.metrics.preview_failed_count = self
            .smoke_commands
            .iter()
            .filter(|command| {
                command.command_id == "preview_aui_overlay"
                    && command.status == ComplexShooterAuiAuthoringStatus::Failed
            })
            .count();

        if self.sample_documents.is_empty() {
            self.next_actions
                .push("create_sample_aui_document".to_string());
        }
        if self.metrics.sample_legacy_document_count > 0 {
            self.next_actions
                .push("save_sample_aui_documents_in_canonical_shape".to_string());
        }
        if self.metrics.preview_partial_count > 0 {
            self.next_actions
                .push("runtime_text_glyph_present".to_string());
        }
        self.next_actions.sort();
        self.next_actions.dedup();

        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("error:"))
            || self
                .smoke_commands
                .iter()
                .any(|command| command.status == ComplexShooterAuiAuthoringStatus::Failed)
        {
            self.status = ComplexShooterAuiAuthoringStatus::Failed;
        } else if !self.next_actions.is_empty() {
            self.status = ComplexShooterAuiAuthoringStatus::Partial;
        } else {
            self.status = ComplexShooterAuiAuthoringStatus::Passed;
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAuiAuthoringRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub manual_walkthrough_summary: Option<ManualWalkthroughCoverageSummary>,
}

impl ComplexShooterAuiAuthoringRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
            manual_walkthrough_summary: None,
        }
    }
}

pub fn run_complex_shooter_aui_authoring_report(
    request: ComplexShooterAuiAuthoringRequest,
) -> ComplexShooterAuiAuthoringReport {
    let mut report = ComplexShooterAuiAuthoringReport::new(
        request.project_root.display().to_string(),
        request.output_root.display().to_string(),
    );
    report.manual_walkthrough_summary = request.manual_walkthrough_summary;

    collect_sample_aui_documents(&request.project_root, &mut report);
    run_aui_authoring_smoke(&mut report);

    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-aui-authoring-productization-report.json");
    report.recompute();
    if write_json(&artifact_path, &report).is_ok() {
        report.artifacts.push(artifact_path.display().to_string());
    } else {
        report.diagnostics.push(format!(
            "error:aui_authoring_report_write_failed:{}",
            artifact_path.display()
        ));
    }
    report.recompute();
    if write_json(&artifact_path, &report).is_err() {
        report.diagnostics.push(format!(
            "error:aui_authoring_report_rewrite_failed:{}",
            artifact_path.display()
        ));
        report.recompute();
    }
    report
}

fn collect_sample_aui_documents(
    project_root: &Path,
    report: &mut ComplexShooterAuiAuthoringReport,
) {
    let aui_root = project_root.join("AUI");
    let Ok(entries) = fs::read_dir(&aui_root) else {
        report.diagnostics.push(format!(
            "error:aui_directory_missing:{}",
            aui_root.display()
        ));
        return;
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".aui.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let relative_path = project_relative_path(project_root, &path);
        let Ok(text) = fs::read_to_string(&path) else {
            report
                .diagnostics
                .push(format!("error:aui_document_read_failed:{relative_path}"));
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            report
                .diagnostics
                .push(format!("error:aui_document_parse_failed:{relative_path}"));
            continue;
        };
        let source_shape = if value.get("canvases").is_some() && value.get("nodes").is_some() {
            "runtime_aui_document"
        } else {
            "legacy_authoring_tree"
        };
        let mut session = EditorSession::new();
        let smoke_root = std::env::temp_dir().join(format!(
            "project-e2e-gate-aui-sample-open-{}",
            unique_stamp()
        ));
        let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: smoke_root.display().to_string(),
            name: "AUI Sample Open".to_string(),
        }));
        if create.status != CommandStatus::Committed {
            report.diagnostics.push(format!(
                "error:aui_sample_project_create_failed:{relative_path}"
            ));
            continue;
        }
        let target = smoke_root.join(&relative_path);
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::copy(&path, &target).is_err() {
            report
                .diagnostics
                .push(format!("error:aui_sample_copy_failed:{relative_path}"));
            continue;
        }
        let open = session.execute_command(command_for_test(UiCommandPayload::OpenAuiDocument {
            path: relative_path.clone(),
        }));
        let preview =
            session.execute_command(command_for_test(UiCommandPayload::PreviewAuiOverlay {
                path: relative_path.clone(),
            }));
        if open.status != CommandStatus::Committed {
            report
                .diagnostics
                .push(format!("error:aui_sample_open_failed:{relative_path}"));
        }
        if preview.status == CommandStatus::Failed {
            report
                .next_actions
                .push(format!("fix_aui_sample_preview:{relative_path}"));
        }
        report.sample_documents.push(AuiAuthoringDocumentEvidence {
            source_path: relative_path,
            source_shape: source_shape.to_string(),
            document_id: value
                .get("documentId")
                .or_else(|| value.get("document_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            node_count: count_nodes(&value),
            binding_count: count_field_recursive(&value, "binding_refs")
                + count_field_recursive(&value, "bindingRefs"),
            action_count: count_field_recursive(&value, "action_refs")
                + count_field_recursive(&value, "actionRefs"),
            validation_ok: open.status == CommandStatus::Committed,
        });
        let _ = fs::remove_dir_all(smoke_root);
    }
}

fn run_aui_authoring_smoke(report: &mut ComplexShooterAuiAuthoringReport) {
    let smoke_root = std::env::temp_dir().join(format!(
        "project-e2e-gate-aui-authoring-smoke-{}",
        unique_stamp()
    ));
    report.smoke_project_root = Some(smoke_root.display().to_string());
    let mut session = EditorSession::new();
    let commands = vec![
        UiCommandPayload::CreateProject {
            path: smoke_root.display().to_string(),
            name: "AUI Authoring Smoke".to_string(),
        },
        UiCommandPayload::CreateAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
            document_id: "hud".to_string(),
            width: 1280.0,
            height: 720.0,
        },
        UiCommandPayload::OpenAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
        },
        UiCommandPayload::AddAuiNode {
            path: "AUI/hud.aui.json".to_string(),
            parent_node_id: "root".to_string(),
            node_id: "score_text".to_string(),
            kind: "text".to_string(),
            name: "Score Text".to_string(),
            rect: serde_json::json!({
                "x": 16.0,
                "y": 16.0,
                "width": 240.0,
                "height": 40.0
            }),
        },
        UiCommandPayload::SetAuiNodeField {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "score_text".to_string(),
            schema_path: "text".to_string(),
            value: serde_json::json!("Score: 0"),
        },
        UiCommandPayload::SetAuiBindingPath {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "score_text".to_string(),
            target_field: "text.text".to_string(),
            binding_id: "bind.score".to_string(),
            binding_path: "game.score_text".to_string(),
            fallback: Some(serde_json::json!("Score: 0")),
        },
        UiCommandPayload::AddAuiNode {
            path: "AUI/hud.aui.json".to_string(),
            parent_node_id: "root".to_string(),
            node_id: "pause_button".to_string(),
            kind: "button".to_string(),
            name: "Pause Button".to_string(),
            rect: serde_json::json!({
                "x": 1100.0,
                "y": 16.0,
                "width": 120.0,
                "height": 48.0
            }),
        },
        UiCommandPayload::SetAuiActionRef {
            path: "AUI/hud.aui.json".to_string(),
            node_id: "pause_button".to_string(),
            event: "click".to_string(),
            action_id: "ui.pause".to_string(),
            payload: None,
        },
        UiCommandPayload::ValidateAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
        },
        UiCommandPayload::SaveAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
        },
        UiCommandPayload::PreviewAuiOverlay {
            path: "AUI/hud.aui.json".to_string(),
        },
    ];

    for payload in commands {
        let result = session.execute_command(command_for_test(payload));
        report
            .smoke_commands
            .push(AuiAuthoringCommandEvidence::from_result(&result));
    }
}

fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn count_nodes(value: &serde_json::Value) -> usize {
    if let Some(nodes) = value.get("nodes").and_then(serde_json::Value::as_array) {
        return nodes.len();
    }
    value.get("root").map_or(0, count_legacy_nodes)
}

fn count_legacy_nodes(value: &serde_json::Value) -> usize {
    1 + value
        .get("children")
        .and_then(serde_json::Value::as_array)
        .map(|children| children.iter().map(count_legacy_nodes).sum::<usize>())
        .unwrap_or(0)
}

fn count_field_recursive(value: &serde_json::Value, field: &str) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            let own = map
                .get(field)
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            own + map
                .values()
                .map(|value| count_field_recursive(value, field))
                .sum::<usize>()
        }
        serde_json::Value::Array(values) => values
            .iter()
            .map(|value| count_field_recursive(value, field))
            .sum(),
        _ => 0,
    }
}

fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
