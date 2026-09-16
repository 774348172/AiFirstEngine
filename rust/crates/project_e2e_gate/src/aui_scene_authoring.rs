use editor_core::{AuiDocumentCookRequest, AuiDocumentCooker, AuiSceneAuthoringService};
use editor_ui_model::{
    AuiSceneUnifiedAuthoringReport, AuiSceneViewProjection, WorkspaceSelectionTarget,
};
use engine_runtime::aui::AuiDocument;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-aui-scene-unified-authoring-report.v1";
pub const COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_SCENARIO_ID: &str =
    "complex-shooter-aui-scene-unified-authoring-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterAuiSceneAuthoringStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuiSceneAuthoringDocumentEvidence {
    pub source_path: String,
    pub document_id: String,
    pub proxy_count: usize,
    pub selectable_proxy_count: usize,
    pub visual_order_entry_count: usize,
    pub runtime_gap_count: usize,
    pub selected_node_id: Option<String>,
    pub report: AuiSceneUnifiedAuthoringReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiSceneAuthoringMetrics {
    pub document_count: usize,
    pub proxy_count: usize,
    pub selectable_proxy_count: usize,
    pub visual_order_entry_count: usize,
    pub runtime_composition_gap_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiSceneAuthoringReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAuiSceneAuthoringStatus,
    pub project_root: String,
    pub output_root: String,
    pub documents: Vec<AuiSceneAuthoringDocumentEvidence>,
    pub metrics: ComplexShooterAuiSceneAuthoringMetrics,
    pub visual_order_runtime_supported: bool,
    pub visual_order_runtime_support_reason: String,
    pub next_required_runtime_gate: Option<String>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl ComplexShooterAuiSceneAuthoringReport {
    fn new(project_root: impl Into<String>, output_root: impl Into<String>) -> Self {
        Self {
            schema_version: COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: COMPLEX_SHOOTER_AUI_SCENE_AUTHORING_SCENARIO_ID.to_string(),
            status: ComplexShooterAuiSceneAuthoringStatus::Failed,
            project_root: project_root.into(),
            output_root: output_root.into(),
            documents: Vec::new(),
            metrics: ComplexShooterAuiSceneAuthoringMetrics {
                document_count: 0,
                proxy_count: 0,
                selectable_proxy_count: 0,
                visual_order_entry_count: 0,
                runtime_composition_gap_count: 0,
            },
            visual_order_runtime_supported: true,
            visual_order_runtime_support_reason:
                "before_world_screen_overlay_modal_runtime_pass_supported; world_space_deferred"
                    .to_string(),
            next_required_runtime_gate: None,
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: vec!["AUI Prefab / Template Reuse Productization v1".to_string()],
        }
    }

    fn recompute(&mut self) {
        self.metrics.document_count = self.documents.len();
        self.metrics.proxy_count = self
            .documents
            .iter()
            .map(|document| document.proxy_count)
            .sum();
        self.metrics.selectable_proxy_count = self
            .documents
            .iter()
            .map(|document| document.selectable_proxy_count)
            .sum();
        self.metrics.visual_order_entry_count = self
            .documents
            .iter()
            .map(|document| document.visual_order_entry_count)
            .sum();
        let document_runtime_gaps = self
            .documents
            .iter()
            .map(|document| document.runtime_gap_count)
            .sum::<usize>();
        self.metrics.runtime_composition_gap_count = document_runtime_gaps;
        self.visual_order_runtime_supported = document_runtime_gaps == 0;
        self.visual_order_runtime_support_reason = if self.visual_order_runtime_supported {
            "before_world_screen_overlay_modal_runtime_pass_supported; world_space_deferred"
                .to_string()
        } else {
            "some_aui_documents_still_use_deferred_render_space".to_string()
        };
        self.next_required_runtime_gate = if self.visual_order_runtime_supported {
            None
        } else {
            Some("RuntimeRenderer Multi-stage UI Composition Pass".to_string())
        };
        if !self.visual_order_runtime_supported {
            self.next_actions
                .push("RuntimeRenderer Multi-stage UI Composition Pass".to_string());
        }

        if self.documents.is_empty() {
            self.next_actions
                .push("create_sample_aui_document".to_string());
        }
        self.next_actions.sort();
        self.next_actions.dedup();

        self.status = if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("error:"))
        {
            ComplexShooterAuiSceneAuthoringStatus::Failed
        } else if self.visual_order_runtime_supported || self.documents.is_empty() {
            ComplexShooterAuiSceneAuthoringStatus::Partial
        } else {
            ComplexShooterAuiSceneAuthoringStatus::Partial
        };
    }
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAuiSceneAuthoringRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterAuiSceneAuthoringRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_aui_scene_authoring_report(
    request: ComplexShooterAuiSceneAuthoringRequest,
) -> ComplexShooterAuiSceneAuthoringReport {
    let mut report = ComplexShooterAuiSceneAuthoringReport::new(
        request.project_root.display().to_string(),
        request.output_root.display().to_string(),
    );
    collect_aui_scene_authoring_documents(&request.project_root, &mut report);

    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-aui-scene-unified-authoring-report.json");
    report.recompute();
    if write_json(&artifact_path, &report).is_ok() {
        report.artifacts.push(artifact_path.display().to_string());
    } else {
        report.diagnostics.push(format!(
            "error:aui_scene_authoring_report_write_failed:{}",
            artifact_path.display()
        ));
    }
    report.recompute();
    let _ = write_json(&artifact_path, &report);
    report
}

fn collect_aui_scene_authoring_documents(
    project_root: &Path,
    report: &mut ComplexShooterAuiSceneAuthoringReport,
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
        let Some(document) = load_or_cook_aui_document(&path, report) else {
            continue;
        };
        let selected_node_id = document
            .nodes
            .iter()
            .find(|node| node.node_id != "root")
            .map(|node| node.node_id.clone())
            .or_else(|| document.nodes.first().map(|node| node.node_id.clone()));
        let selected = selected_node_id
            .as_ref()
            .map(|node_id| WorkspaceSelectionTarget::AuiNode {
                document_path: relative_path.clone(),
                document_id: document.document_id.clone(),
                node_id: node_id.clone(),
            });
        let output = AuiSceneAuthoringService::build_document_overlay(
            Some("Scenes/Main.scene.json".to_string()),
            relative_path.clone(),
            &document,
            AuiSceneViewProjection::Orthographic2D,
            selected.as_ref(),
        );
        report.documents.push(AuiSceneAuthoringDocumentEvidence {
            source_path: relative_path,
            document_id: document.document_id.clone(),
            proxy_count: output.report.proxy_count,
            selectable_proxy_count: output.report.selectable_proxy_count,
            visual_order_entry_count: output.report.visual_order_entry_count,
            runtime_gap_count: output.visual_order.runtime_gap_count(),
            selected_node_id,
            report: output.report,
        });
    }
}

fn load_or_cook_aui_document(
    path: &Path,
    report: &mut ComplexShooterAuiSceneAuthoringReport,
) -> Option<AuiDocument> {
    let text = fs::read_to_string(path).ok()?;
    if let Ok(document) = serde_json::from_str::<AuiDocument>(&text) {
        return Some(document);
    }
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let output = AuiDocumentCooker::cook(AuiDocumentCookRequest {
        source_path: path.to_path_buf(),
        document: value,
    })
    .ok()?;
    if !output.report.diagnostics.is_empty() {
        report.diagnostics.push(format!(
            "warning:aui_document_cooked_with_diagnostics:{}",
            path.display()
        ));
    }
    Some(output.document)
}

fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
