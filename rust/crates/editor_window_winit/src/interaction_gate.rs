use crate::application::{NativeEditorApplication, NativeEditorApplicationReport};
use editor_core::CommandStatus;
use editor_input::{EditorInputEvent, PointerButton};
use editor_ui_model::EditorUiMode;
use editor_ui_renderer::{HitTarget, UiRect};
use serde::{Deserialize, Serialize};

pub const NATIVE_EDITOR_INTERACTION_REPORT_SCHEMA_VERSION: &str =
    "native-editor-interaction-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeEditorInteractionStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeEditorInteractionDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeEditorInteractionDiagnostic {
    pub severity: NativeEditorInteractionDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub step_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeEditorInteractionScenario {
    pub scenario_id: String,
    pub title: String,
    pub steps: Vec<NativeEditorInteractionStep>,
}

impl NativeEditorInteractionScenario {
    pub fn new(scenario_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            title: title.into(),
            steps: Vec::new(),
        }
    }

    pub fn with_step(mut self, step: NativeEditorInteractionStep) -> Self {
        self.steps.push(step);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeEditorInteractionStep {
    pub step_id: String,
    pub action: NativeEditorInteractionAction,
    pub expect_command_id: Option<String>,
    pub expect_command_status: Option<CommandStatus>,
    pub expect_mode: Option<EditorUiMode>,
    pub expect_selected_entity_id: Option<String>,
    pub expect_feedback_status: Option<editor_ui_model::EditorCommandFeedbackStatus>,
    pub expect_model_revision_increase: bool,
}

impl NativeEditorInteractionStep {
    pub fn click_hit_region(step_id: impl Into<String>, hit_region_id: impl Into<String>) -> Self {
        Self {
            step_id: step_id.into(),
            action: NativeEditorInteractionAction::ClickHitRegion {
                hit_region_id: hit_region_id.into(),
            },
            expect_command_id: None,
            expect_command_status: None,
            expect_mode: None,
            expect_selected_entity_id: None,
            expect_feedback_status: None,
            expect_model_revision_increase: false,
        }
    }

    pub fn replace_focused_property_text(
        step_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            step_id: step_id.into(),
            action: NativeEditorInteractionAction::ReplaceFocusedPropertyText { text: text.into() },
            expect_command_id: None,
            expect_command_status: None,
            expect_mode: None,
            expect_selected_entity_id: None,
            expect_feedback_status: None,
            expect_model_revision_increase: false,
        }
    }

    pub fn commit_focused_property_edit(step_id: impl Into<String>) -> Self {
        Self {
            step_id: step_id.into(),
            action: NativeEditorInteractionAction::CommitFocusedPropertyEdit,
            expect_command_id: None,
            expect_command_status: None,
            expect_mode: None,
            expect_selected_entity_id: None,
            expect_feedback_status: None,
            expect_model_revision_increase: false,
        }
    }

    pub fn frame(step_id: impl Into<String>) -> Self {
        Self {
            step_id: step_id.into(),
            action: NativeEditorInteractionAction::Frame,
            expect_command_id: None,
            expect_command_status: None,
            expect_mode: None,
            expect_selected_entity_id: None,
            expect_feedback_status: None,
            expect_model_revision_increase: false,
        }
    }

    pub fn expect_command(mut self, command_id: impl Into<String>, status: CommandStatus) -> Self {
        self.expect_command_id = Some(command_id.into());
        self.expect_command_status = Some(status);
        self
    }

    pub fn expect_mode(mut self, mode: EditorUiMode) -> Self {
        self.expect_mode = Some(mode);
        self
    }

    pub fn expect_selected_entity(mut self, entity_id: impl Into<String>) -> Self {
        self.expect_selected_entity_id = Some(entity_id.into());
        self
    }

    pub fn expect_feedback_status(
        mut self,
        status: editor_ui_model::EditorCommandFeedbackStatus,
    ) -> Self {
        self.expect_feedback_status = Some(status);
        self
    }

    pub fn expect_revision_increase(mut self) -> Self {
        self.expect_model_revision_increase = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NativeEditorInteractionAction {
    ClickHitRegion { hit_region_id: String },
    ReplaceFocusedPropertyText { text: String },
    CommitFocusedPropertyEdit,
    Frame,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeEditorInteractionStepReport {
    pub step_id: String,
    pub action_kind: String,
    pub status: NativeEditorInteractionStatus,
    pub input_event_kind: Option<String>,
    pub hit_region_id: Option<String>,
    pub hit_target: Option<HitTarget>,
    pub hit_rect: Option<UiRect>,
    pub command_id: Option<String>,
    pub command_status: Option<CommandStatus>,
    pub feedback_status: Option<editor_ui_model::EditorCommandFeedbackStatus>,
    pub mode: EditorUiMode,
    pub selected_entity_id: Option<String>,
    pub model_revision_before: u64,
    pub model_revision_after: u64,
    pub draw_command_count: usize,
    pub hit_region_count: usize,
    pub diagnostics: Vec<NativeEditorInteractionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeEditorInteractionReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub title: String,
    pub status: NativeEditorInteractionStatus,
    pub backend: String,
    pub step_count: usize,
    pub passed_step_count: usize,
    pub failed_step_count: usize,
    pub final_mode: EditorUiMode,
    pub final_selected_entity_id: Option<String>,
    pub final_model_revision: u64,
    pub steps: Vec<NativeEditorInteractionStepReport>,
    pub diagnostics: Vec<NativeEditorInteractionDiagnostic>,
}

pub struct NativeEditorInteractionRunner {
    backend: String,
    width: f32,
    height: f32,
}

impl Default for NativeEditorInteractionRunner {
    fn default() -> Self {
        Self::headless(1280.0, 720.0)
    }
}

impl NativeEditorInteractionRunner {
    pub fn headless(width: f32, height: f32) -> Self {
        Self {
            backend: "headless-deterministic".to_string(),
            width,
            height,
        }
    }

    pub fn run(
        &self,
        app: &mut NativeEditorApplication,
        scenario: NativeEditorInteractionScenario,
    ) -> NativeEditorInteractionReport {
        app.frame(self.width, self.height);
        let mut steps = Vec::new();
        let mut diagnostics = Vec::new();

        for step in &scenario.steps {
            let report = self.run_step(app, step);
            diagnostics.extend(report.diagnostics.iter().cloned());
            steps.push(report);
        }

        let final_report = app.report();
        let failed_step_count = steps
            .iter()
            .filter(|step| step.status == NativeEditorInteractionStatus::Failed)
            .count();
        let passed_step_count = steps.len().saturating_sub(failed_step_count);
        NativeEditorInteractionReport {
            schema_version: NATIVE_EDITOR_INTERACTION_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: scenario.scenario_id,
            title: scenario.title,
            status: if failed_step_count == 0 {
                NativeEditorInteractionStatus::Passed
            } else {
                NativeEditorInteractionStatus::Failed
            },
            backend: self.backend.clone(),
            step_count: steps.len(),
            passed_step_count,
            failed_step_count,
            final_mode: final_report.mode,
            final_selected_entity_id: final_report.workspace.primary_entity_id,
            final_model_revision: final_report.model_revision,
            steps,
            diagnostics,
        }
    }

    fn run_step(
        &self,
        app: &mut NativeEditorApplication,
        step: &NativeEditorInteractionStep,
    ) -> NativeEditorInteractionStepReport {
        let before = app.report();
        let mut diagnostics = Vec::new();
        let mut hit_region_id = None;
        let mut hit_target = None;
        let mut hit_rect = None;
        let mut input_event_kind = None;

        let after = match &step.action {
            NativeEditorInteractionAction::ClickHitRegion { hit_region_id: id } => {
                hit_region_id = Some(id.clone());
                let region = app
                    .latest_draw_list()
                    .hit_regions
                    .iter()
                    .find(|region| region.id == *id)
                    .cloned();
                match region {
                    Some(region) => {
                        hit_target = Some(region.target.clone());
                        hit_rect = Some(region.rect);
                        let x = region.rect.x + (region.rect.width * 0.5).max(1.0);
                        let y = region.rect.y + (region.rect.height * 0.5).max(1.0);
                        app.handle_input_event(EditorInputEvent::PointerDown {
                            x,
                            y,
                            button: PointerButton::Primary,
                        });
                        input_event_kind = Some("PointerDown+PointerUp".to_string());
                        app.handle_input_event(EditorInputEvent::PointerUp {
                            x,
                            y,
                            button: PointerButton::Primary,
                        })
                    }
                    None => {
                        diagnostics.push(step_error(
                            &step.step_id,
                            "interaction.hit_region_missing",
                            format!("Hit region `{id}` was not found."),
                        ));
                        before.clone()
                    }
                }
            }
            NativeEditorInteractionAction::ReplaceFocusedPropertyText { text } => {
                let ok = app.replace_focused_property_text(text.clone());
                if !ok {
                    diagnostics.push(step_error(
                        &step.step_id,
                        "interaction.no_focused_property",
                        "No focused property was available for text replacement.",
                    ));
                }
                app.report()
            }
            NativeEditorInteractionAction::CommitFocusedPropertyEdit => {
                match app.commit_focused_property_edit() {
                    Some(_) => app.report(),
                    None => {
                        diagnostics.push(step_error(
                            &step.step_id,
                            "interaction.property_commit_failed",
                            "Focused property edit did not produce a command.",
                        ));
                        app.report()
                    }
                }
            }
            NativeEditorInteractionAction::Frame => app.frame(self.width, self.height),
        };

        validate_step_expectations(step, &before, &after, &mut diagnostics);
        let status = if diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == NativeEditorInteractionDiagnosticSeverity::Error
        }) {
            NativeEditorInteractionStatus::Failed
        } else {
            NativeEditorInteractionStatus::Passed
        };

        NativeEditorInteractionStepReport {
            step_id: step.step_id.clone(),
            action_kind: action_kind(&step.action).to_string(),
            status,
            input_event_kind,
            hit_region_id,
            hit_target,
            hit_rect,
            command_id: after.last_command_id.clone(),
            command_status: after.last_command_status,
            feedback_status: after.last_feedback.as_ref().map(|feedback| feedback.status),
            mode: after.mode,
            selected_entity_id: after.workspace.primary_entity_id,
            model_revision_before: before.model_revision,
            model_revision_after: after.model_revision,
            draw_command_count: after.draw_command_count,
            hit_region_count: after.hit_region_count,
            diagnostics,
        }
    }
}

fn validate_step_expectations(
    step: &NativeEditorInteractionStep,
    before: &NativeEditorApplicationReport,
    after: &NativeEditorApplicationReport,
    diagnostics: &mut Vec<NativeEditorInteractionDiagnostic>,
) {
    if let Some(expected) = &step.expect_command_id {
        if after.last_command_id.as_deref() != Some(expected.as_str()) {
            diagnostics.push(step_error(
                &step.step_id,
                "interaction.command_id_mismatch",
                format!(
                    "Expected command `{expected}`, got `{:?}`.",
                    after.last_command_id
                ),
            ));
        }
    }

    if let Some(expected) = step.expect_command_status {
        if after.last_command_status != Some(expected) {
            diagnostics.push(step_error(
                &step.step_id,
                "interaction.command_status_mismatch",
                format!(
                    "Expected command status `{:?}`, got `{:?}`.",
                    expected, after.last_command_status
                ),
            ));
        }
    }

    if let Some(expected) = &step.expect_mode {
        if &after.mode != expected {
            diagnostics.push(step_error(
                &step.step_id,
                "interaction.mode_mismatch",
                format!("Expected mode `{:?}`, got `{:?}`.", expected, after.mode),
            ));
        }
    }

    if let Some(expected) = &step.expect_selected_entity_id {
        if after.workspace.primary_entity_id.as_deref() != Some(expected.as_str()) {
            diagnostics.push(step_error(
                &step.step_id,
                "interaction.selection_mismatch",
                format!(
                    "Expected selected entity `{expected}`, got `{:?}`.",
                    after.workspace.primary_entity_id
                ),
            ));
        }
    }

    if let Some(expected) = step.expect_feedback_status {
        let actual = after.last_feedback.as_ref().map(|feedback| feedback.status);
        if actual != Some(expected) {
            diagnostics.push(step_error(
                &step.step_id,
                "interaction.feedback_status_mismatch",
                format!(
                    "Expected feedback status `{:?}`, got `{:?}`.",
                    expected, actual
                ),
            ));
        }
    }

    if step.expect_model_revision_increase && after.model_revision <= before.model_revision {
        diagnostics.push(step_error(
            &step.step_id,
            "interaction.revision_not_increased",
            format!(
                "Expected model revision to increase from {}, got {}.",
                before.model_revision, after.model_revision
            ),
        ));
    }
}

fn action_kind(action: &NativeEditorInteractionAction) -> &'static str {
    match action {
        NativeEditorInteractionAction::ClickHitRegion { .. } => "click_hit_region",
        NativeEditorInteractionAction::ReplaceFocusedPropertyText { .. } => {
            "replace_focused_property_text"
        }
        NativeEditorInteractionAction::CommitFocusedPropertyEdit => "commit_focused_property_edit",
        NativeEditorInteractionAction::Frame => "frame",
    }
}

fn step_error(
    step_id: &str,
    code: impl Into<String>,
    message: impl Into<String>,
) -> NativeEditorInteractionDiagnostic {
    NativeEditorInteractionDiagnostic {
        severity: NativeEditorInteractionDiagnosticSeverity::Error,
        code: code.into(),
        message: message.into(),
        step_id: Some(step_id.to_string()),
    }
}
