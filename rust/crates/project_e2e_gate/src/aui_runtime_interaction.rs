use engine_input::{
    InputActionDefinition, InputActionValueType, InputBindingDefinition, InputContextDefinition,
    InputMappingAsset, InputResolver, RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton,
};
use engine_runtime::aui::{
    AuiActionRef, AuiCanvas, AuiDocument, AuiInteractionConfig, AuiInteractionProductizationReport,
    AuiInteractionState, AuiInteractionSystem, AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-aui-runtime-interaction-productization-report.v1";
pub const COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_SCENARIO_ID: &str =
    "complex-shooter-aui-runtime-interaction-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterAuiRuntimeInteractionStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiRuntimeInteractionMetrics {
    pub click_action_count: usize,
    pub click_consumed_pointer_event_count: usize,
    pub click_filtered_input_event_count: usize,
    pub gameplay_fire_triggered_after_ui_click: bool,
    pub drag_start_count: usize,
    pub drop_count: usize,
    pub drag_cancel_count: usize,
    pub payload_project_semantics_detected: bool,
    pub deferred_flag_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiRuntimeInteractionReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAuiRuntimeInteractionStatus,
    pub project_root: String,
    pub output_root: String,
    pub metrics: ComplexShooterAuiRuntimeInteractionMetrics,
    pub click_report: AuiInteractionProductizationReport,
    pub drag_drop_report: AuiInteractionProductizationReport,
    pub drag_cancel_report: AuiInteractionProductizationReport,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAuiRuntimeInteractionRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterAuiRuntimeInteractionRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_aui_runtime_interaction_report(
    request: ComplexShooterAuiRuntimeInteractionRequest,
) -> ComplexShooterAuiRuntimeInteractionReport {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mapping = mouse_fire_mapping();

    let click_frame = pointer_frame(vec![
        RuntimeInputEvent::PointerMove { x: 96.0, y: 96.0 },
        RuntimeInputEvent::PointerDown {
            x: 96.0,
            y: 96.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerUp {
            x: 96.0,
            y: 96.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut click_state = AuiInteractionState::default();
    let click_result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &click_frame,
        &mut click_state,
        AuiInteractionConfig::default(),
    );
    let click_filtered = click_frame.filter_consumed_events(&click_result.consumed_event_indices);
    let click_input = InputResolver::resolve(&click_filtered, &mapping);
    let click_report = AuiInteractionProductizationReport::from_result(
        &document,
        click_frame.events.len(),
        click_filtered.events.len(),
        &click_result,
        AuiInteractionConfig::default(),
        click_state.active_drag_source().map(ToOwned::to_owned),
    );

    let drag_drop_frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 80.0,
            y: 190.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove { x: 160.0, y: 190.0 },
        RuntimeInputEvent::PointerUp {
            x: 260.0,
            y: 190.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut drag_state = AuiInteractionState::default();
    let drag_drop_result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &drag_drop_frame,
        &mut drag_state,
        AuiInteractionConfig::default(),
    );
    let drag_drop_filtered =
        drag_drop_frame.filter_consumed_events(&drag_drop_result.consumed_event_indices);
    let drag_drop_report = AuiInteractionProductizationReport::from_result(
        &document,
        drag_drop_frame.events.len(),
        drag_drop_filtered.events.len(),
        &drag_drop_result,
        AuiInteractionConfig::default(),
        drag_state.active_drag_source().map(ToOwned::to_owned),
    );

    let drag_cancel_frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 80.0,
            y: 190.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove { x: 160.0, y: 190.0 },
        RuntimeInputEvent::PointerUp {
            x: 520.0,
            y: 420.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut cancel_state = AuiInteractionState::default();
    let drag_cancel_result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &drag_cancel_frame,
        &mut cancel_state,
        AuiInteractionConfig::default(),
    );
    let drag_cancel_filtered =
        drag_cancel_frame.filter_consumed_events(&drag_cancel_result.consumed_event_indices);
    let drag_cancel_report = AuiInteractionProductizationReport::from_result(
        &document,
        drag_cancel_frame.events.len(),
        drag_cancel_filtered.events.len(),
        &drag_cancel_result,
        AuiInteractionConfig::default(),
        cancel_state.active_drag_source().map(ToOwned::to_owned),
    );

    let payload_text = drag_drop_result
        .commands
        .iter()
        .filter_map(|command| command.payload.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    let payload_project_semantics_detected = contains_project_semantics(&payload_text);

    let mut report = ComplexShooterAuiRuntimeInteractionReport {
        schema_version: COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_AUI_RUNTIME_INTERACTION_SCENARIO_ID.to_string(),
        status: ComplexShooterAuiRuntimeInteractionStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        metrics: ComplexShooterAuiRuntimeInteractionMetrics {
            click_action_count: click_report.click_action_count,
            click_consumed_pointer_event_count: click_report.consumed_pointer_event_count,
            click_filtered_input_event_count: click_report.filtered_input_event_count,
            gameplay_fire_triggered_after_ui_click: click_input
                .action_snapshot
                .button_pressed("action.fire"),
            drag_start_count: drag_drop_report.drag_start_count,
            drop_count: drag_drop_report.drop_count,
            drag_cancel_count: drag_cancel_report.drag_cancel_count,
            payload_project_semantics_detected,
            deferred_flag_count: [
                click_report.authoring_action_payload_deferred,
                click_report.modal_input_blocking_deferred,
                click_report.editor_hit_test_deferred_to_209,
            ]
            .into_iter()
            .filter(|flag| *flag)
            .count(),
        },
        click_report,
        drag_drop_report,
        drag_cancel_report,
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };
    recompute(&mut report);
    write_report(request.output_root, report)
}

fn recompute(report: &mut ComplexShooterAuiRuntimeInteractionReport) {
    report.diagnostics.clear();
    report.next_actions.clear();
    if report.metrics.click_action_count == 0 {
        report
            .diagnostics
            .push("error:aui_click_action_not_dispatched".to_string());
    }
    if report.metrics.gameplay_fire_triggered_after_ui_click {
        report
            .diagnostics
            .push("error:aui_input.consumed_pointer_leaked_to_gameplay".to_string());
    }
    if report.metrics.drop_count == 0 {
        report
            .diagnostics
            .push("error:aui_drag.drop_without_target".to_string());
    }
    if report.metrics.drag_cancel_count == 0 {
        report
            .diagnostics
            .push("error:aui_drag.cancel_not_reported".to_string());
    }
    if report.metrics.payload_project_semantics_detected {
        report
            .diagnostics
            .push("error:aui_drag.payload_contains_project_semantics".to_string());
    }
    if report.metrics.deferred_flag_count != 1 {
        report
            .next_actions
            .push("verify_aui_interaction_deferred_flags".to_string());
    }
    report.status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        ComplexShooterAuiRuntimeInteractionStatus::Failed
    } else {
        ComplexShooterAuiRuntimeInteractionStatus::Passed
    };
}

fn write_report(
    output_root: PathBuf,
    mut report: ComplexShooterAuiRuntimeInteractionReport,
) -> ComplexShooterAuiRuntimeInteractionReport {
    let report_path = output_root
        .join("reports")
        .join("complex-shooter-aui-runtime-interaction-productization-report.json");
    if let Some(parent) = report_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            report.diagnostics.push(format!(
                "error:aui_interaction_report_dir_create_failed:{error}"
            ));
            report.status = ComplexShooterAuiRuntimeInteractionStatus::Failed;
            return report;
        }
    }
    report.artifacts.push(report_path.display().to_string());
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(error) = fs::write(&report_path, json) {
                report
                    .diagnostics
                    .push(format!("error:aui_interaction_report_write_failed:{error}"));
                report.status = ComplexShooterAuiRuntimeInteractionStatus::Failed;
            }
        }
        Err(error) => report.diagnostics.push(format!(
            "error:aui_interaction_report_serialize_failed:{error}"
        )),
    }
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        report.status = ComplexShooterAuiRuntimeInteractionStatus::Failed;
    }
    report
}

fn interaction_document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children([
        "pause_button",
        "drag_source",
        "drop_target",
    ]);
    let pause_button = AuiNode::new(
        "pause_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(40.0, 40.0, 160.0, 80.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_action(AuiActionRef::click("ui.pause"));
    let drag_source = AuiNode::new(
        "drag_source",
        AuiNodeKind::Button,
        AuiRect::fixed_position(40.0, 160.0, 120.0, 80.0),
    )
    .with_parent("root")
    .with_draggable()
    .with_action(AuiActionRef::drag_start("ui.drag_start"))
    .with_action(AuiActionRef::drag_move("ui.drag_move"))
    .with_action(AuiActionRef::drop("ui.drop"));
    let drop_target = AuiNode::new(
        "drop_target",
        AuiNodeKind::Button,
        AuiRect::fixed_position(220.0, 160.0, 120.0, 80.0),
    )
    .with_parent("root")
    .with_drop_target();
    AuiDocument::new(
        "complex-shooter-aui-runtime-interaction",
        vec![AuiCanvas::screen_overlay("main", 640.0, 480.0, "root")],
        vec![root, pause_button, drag_source, drop_target],
    )
}

fn pointer_frame(events: Vec<RuntimeInputEvent>) -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(1, "game-view");
    frame.events = events;
    frame
}

fn mouse_fire_mapping() -> InputMappingAsset {
    InputMappingAsset::new(
        "input.mouse-fire",
        vec![InputActionDefinition::new(
            "action.fire",
            InputActionValueType::Button,
        )],
        vec![InputContextDefinition::new("gameplay", 0)],
        vec![InputBindingDefinition::new(
            "gameplay",
            "action.fire",
            "mouse/Left",
        )],
    )
}

fn contains_project_semantics(payload: &str) -> bool {
    [
        "equipment_id",
        "chess_piece_id",
        "inventory_index",
        "entity",
        "renderer_handle",
    ]
    .iter()
    .any(|needle| payload.contains(needle))
}
