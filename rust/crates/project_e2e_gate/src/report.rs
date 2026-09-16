use serde::{Deserialize, Serialize};

pub const COMPLEX_PROJECT_E2E_GATE_REPORT_SCHEMA_VERSION: &str =
    "complex-project-e2e-gate-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexProjectE2eStatus {
    Passed,
    Failed,
    Partial,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexProjectE2eStep {
    pub step_id: String,
    pub status: ComplexProjectE2eStatus,
    pub summary: String,
    pub artifact_path: Option<String>,
}

impl ComplexProjectE2eStep {
    pub fn new(
        step_id: impl Into<String>,
        status: ComplexProjectE2eStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            step_id: step_id.into(),
            status,
            summary: summary.into(),
            artifact_path: None,
        }
    }

    pub fn with_artifact_path(mut self, path: impl Into<String>) -> Self {
        self.artifact_path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexProjectE2eGap {
    pub gap_id: String,
    pub severity: String,
    pub summary: String,
    pub next_action: String,
}

impl ComplexProjectE2eGap {
    pub fn new(
        gap_id: impl Into<String>,
        severity: impl Into<String>,
        summary: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            gap_id: gap_id.into(),
            severity: severity.into(),
            summary: summary.into(),
            next_action: next_action.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexProjectE2eArtifact {
    pub artifact_id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexProjectE2eDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl ComplexProjectE2eDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "warning".to_string(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexProjectE2eMetrics {
    pub scene_count: usize,
    pub entity_count: usize,
    pub prefab_count: usize,
    pub asset_count: usize,
    pub rule_count: usize,
    pub input_action_count: usize,
    pub aui_document_count: usize,
    pub aui_package_document_count: usize,
    pub aui_loaded_document_count: usize,
    pub aui_draw_item_count: usize,
    pub aui_text_command_count: usize,
    pub aui_ui_pass_inserted: bool,
    pub aui_composition_stage_count: usize,
    pub aui_before_world_item_count: usize,
    pub aui_screen_overlay_item_count: usize,
    pub aui_modal_item_count: usize,
    pub aui_before_world_pass_present: bool,
    pub aui_screen_overlay_pass_present: bool,
    pub aui_modal_pass_present: bool,
    pub aui_before_world_skipped: bool,
    pub aui_screen_overlay_skipped: bool,
    pub aui_modal_skipped: bool,
    pub aui_modal_rendering_only: bool,
    pub aui_glyph_present: bool,
    pub aui_font_atlas_present: bool,
    pub aui_font_atlas_id: Option<String>,
    pub aui_font_source_kind: Option<String>,
    pub aui_font_asset_id: Option<String>,
    pub aui_font_asset_status: Option<String>,
    pub aui_font_fallback_used: bool,
    pub aui_requested_glyph_count: usize,
    pub aui_rendered_glyph_count: usize,
    pub aui_unsupported_glyph_count: usize,
    pub aui_clipped_glyph_count: usize,
    pub aui_glyph_plan_hash: Option<String>,
    pub aui_snapshot_source: String,
    pub aui_producer_id: Option<String>,
    pub aui_snapshot_value_count: usize,
    pub aui_produced_path_count: usize,
    pub aui_declared_binding_path_count: usize,
    pub aui_missing_path_count: usize,
    pub aui_type_mismatch_path_count: usize,
    pub aui_status: String,
    pub aui_next_actions: Vec<String>,
    pub runtime_package_entity_count: usize,
    pub frames_run: u64,
    pub draw_item_count: usize,
    pub present_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexProjectE2eGateReport {
    pub schema_version: String,
    pub gate_id: String,
    pub status: ComplexProjectE2eStatus,
    pub project_path: String,
    pub build_output_path: String,
    pub exported_package_path: Option<String>,
    pub steps: Vec<ComplexProjectE2eStep>,
    pub gaps: Vec<ComplexProjectE2eGap>,
    pub artifacts: Vec<ComplexProjectE2eArtifact>,
    pub metrics: ComplexProjectE2eMetrics,
    pub diagnostics: Vec<ComplexProjectE2eDiagnostic>,
}

impl ComplexProjectE2eGateReport {
    pub fn new(project_path: impl Into<String>, build_output_path: impl Into<String>) -> Self {
        Self {
            schema_version: COMPLEX_PROJECT_E2E_GATE_REPORT_SCHEMA_VERSION.to_string(),
            gate_id: "complex-shooter-real-project-end-to-end-gate-v1".to_string(),
            status: ComplexProjectE2eStatus::Failed,
            project_path: project_path.into(),
            build_output_path: build_output_path.into(),
            exported_package_path: None,
            steps: Vec::new(),
            gaps: Vec::new(),
            artifacts: Vec::new(),
            metrics: ComplexProjectE2eMetrics::default(),
            diagnostics: Vec::new(),
        }
    }

    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
    }

    pub fn recompute_status(&mut self) {
        if self.has_error_diagnostics()
            || self
                .steps
                .iter()
                .any(|step| step.status == ComplexProjectE2eStatus::Failed)
        {
            self.status = ComplexProjectE2eStatus::Failed;
        } else if self
            .steps
            .iter()
            .any(|step| step.status == ComplexProjectE2eStatus::Partial)
        {
            self.status = ComplexProjectE2eStatus::Partial;
        } else {
            self.status = ComplexProjectE2eStatus::Passed;
        }
    }
}
