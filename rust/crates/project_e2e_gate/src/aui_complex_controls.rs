use engine_input::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
use engine_runtime::aui::{
    AuiActionRef, AuiCanvas, AuiCompositionStage, AuiDocument, AuiInteractionConfig,
    AuiInteractionProductizationReport, AuiInteractionState, AuiInteractionSystem, AuiLayoutEngine,
    AuiNode, AuiNodeKind, AuiRect,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::aui_scene_authoring::{
    run_complex_shooter_aui_scene_authoring_report, ComplexShooterAuiSceneAuthoringReport,
    ComplexShooterAuiSceneAuthoringRequest,
};

pub const COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-aui-complex-controls-productization-report.v1";
pub const COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_SCENARIO_ID: &str =
    "complex-shooter-aui-complex-controls-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterAuiComplexControlsStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiComplexControlsMetrics {
    pub modal_consumed_pointer_count: usize,
    pub modal_consumed_wheel_count: usize,
    pub modal_consumed_keyboard_count: usize,
    pub focus_change_count: usize,
    pub cancel_action_count: usize,
    pub wheel_scroll_offset_change_count: usize,
    pub drag_scroll_offset_change_count: usize,
    pub scroll_offset_applied_count: usize,
    pub scroll_applied_node_count: usize,
    pub clipped_node_count: usize,
    pub scene_selectable_proxy_count: usize,
    pub deferred_flag_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiComplexControlsReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAuiComplexControlsStatus,
    pub project_root: String,
    pub output_root: String,
    pub metrics: ComplexShooterAuiComplexControlsMetrics,
    pub modal_report: AuiInteractionProductizationReport,
    pub focus_report: AuiInteractionProductizationReport,
    pub wheel_scroll_report: AuiInteractionProductizationReport,
    pub drag_scroll_report: AuiInteractionProductizationReport,
    pub scene_authoring_report: ComplexShooterAuiSceneAuthoringReport,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAuiComplexControlsRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterAuiComplexControlsRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_aui_complex_controls_report(
    request: ComplexShooterAuiComplexControlsRequest,
) -> ComplexShooterAuiComplexControlsReport {
    let modal_document = modal_document();
    let modal_layout = AuiLayoutEngine::layout(&modal_document, 1);
    let mut modal_frame = runtime_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 40.0,
            y: 40.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::MouseWheel { delta: -1.0 },
        RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        },
    ]);
    modal_frame.pointer_position = Some(engine_input::PointerPosition { x: 40.0, y: 40.0 });
    let mut modal_state = AuiInteractionState::default();
    let modal_report = run_interaction_report(
        &modal_document,
        &modal_layout,
        &modal_frame,
        &mut modal_state,
    );

    let focus_frame = runtime_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 230.0,
            y: 150.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::KeyDown {
            key: "Tab".to_string(),
        },
        RuntimeInputEvent::KeyDown {
            key: "Escape".to_string(),
        },
    ]);
    let mut focus_state = AuiInteractionState::default();
    let focus_report = run_interaction_report(
        &modal_document,
        &modal_layout,
        &focus_frame,
        &mut focus_state,
    );

    let scroll_document = scroll_document();
    let scroll_layout = AuiLayoutEngine::layout(&scroll_document, 1);
    let mut wheel_frame = runtime_frame(vec![RuntimeInputEvent::MouseWheel { delta: -1.0 }]);
    wheel_frame.pointer_position = Some(engine_input::PointerPosition { x: 20.0, y: 20.0 });
    let mut wheel_state = AuiInteractionState::default();
    let wheel_scroll_report = run_interaction_report(
        &scroll_document,
        &scroll_layout,
        &wheel_frame,
        &mut wheel_state,
    );

    let drag_frame = runtime_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 20.0,
            y: 80.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove { x: 20.0, y: 20.0 },
        RuntimeInputEvent::PointerUp {
            x: 20.0,
            y: 20.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut drag_state = AuiInteractionState::default();
    let drag_scroll_report = run_interaction_report(
        &scroll_document,
        &scroll_layout,
        &drag_frame,
        &mut drag_state,
    );

    let scene_authoring_report = run_complex_shooter_aui_scene_authoring_report(
        ComplexShooterAuiSceneAuthoringRequest::new(&request.project_root, &request.output_root),
    );

    let mut report = ComplexShooterAuiComplexControlsReport {
        schema_version: COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_AUI_COMPLEX_CONTROLS_SCENARIO_ID.to_string(),
        status: ComplexShooterAuiComplexControlsStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        metrics: ComplexShooterAuiComplexControlsMetrics {
            modal_consumed_pointer_count: modal_report.consumed_pointer_event_count,
            modal_consumed_wheel_count: modal_report.consumed_wheel_event_count,
            modal_consumed_keyboard_count: modal_report.consumed_keyboard_event_count,
            focus_change_count: focus_report.focus_change_count,
            cancel_action_count: focus_report.cancel_action_count,
            wheel_scroll_offset_change_count: wheel_scroll_report.scroll_offset_change_count,
            drag_scroll_offset_change_count: drag_scroll_report.scroll_offset_change_count,
            scroll_offset_applied_count: [
                wheel_scroll_report.scroll_offset_applied,
                drag_scroll_report.scroll_offset_applied,
            ]
            .into_iter()
            .filter(|applied| *applied)
            .count(),
            scroll_applied_node_count: wheel_scroll_report.scroll_applied_node_count
                + drag_scroll_report.scroll_applied_node_count,
            clipped_node_count: wheel_scroll_report.clipped_node_count
                + drag_scroll_report.clipped_node_count,
            scene_selectable_proxy_count: scene_authoring_report.metrics.selectable_proxy_count,
            deferred_flag_count: [
                modal_report.authoring_action_payload_deferred,
                modal_report.control_style_deferred,
                modal_report.slider_toggle_binding_target_deferred,
            ]
            .into_iter()
            .filter(|flag| *flag)
            .count(),
        },
        modal_report,
        focus_report,
        wheel_scroll_report,
        drag_scroll_report,
        scene_authoring_report,
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };
    recompute(&mut report);
    write_report(request.output_root, report)
}

fn run_interaction_report(
    document: &AuiDocument,
    layout: &engine_runtime::aui::AuiLayoutResult,
    frame: &RuntimeInputFrame,
    state: &mut AuiInteractionState,
) -> AuiInteractionProductizationReport {
    let config = AuiInteractionConfig::default();
    let result = AuiInteractionSystem::process_with_state(document, layout, frame, state, config);
    let filtered = frame.filter_consumed_events(&result.consumed_event_indices);
    let layout_after = AuiLayoutEngine::layout_with_scroll_offsets(
        document,
        frame.frame_id,
        &state.scroll_offsets,
    );
    AuiInteractionProductizationReport::from_result(
        document,
        frame.events.len(),
        filtered.events.len(),
        &result,
        config,
        state.active_drag_source().map(ToOwned::to_owned),
    )
    .with_focus_state(&state.focus)
    .with_layout_report(&layout_after.report)
}

fn recompute(report: &mut ComplexShooterAuiComplexControlsReport) {
    report.diagnostics.clear();
    report.next_actions.clear();
    if report.metrics.modal_consumed_pointer_count == 0 {
        report
            .diagnostics
            .push("error:aui_modal.pointer_not_consumed".to_string());
    }
    if report.metrics.modal_consumed_wheel_count == 0 {
        report
            .diagnostics
            .push("error:aui_modal.wheel_not_consumed".to_string());
    }
    if report.metrics.modal_consumed_keyboard_count == 0 {
        report
            .diagnostics
            .push("error:aui_modal.keyboard_not_consumed".to_string());
    }
    if report.metrics.focus_change_count == 0 || report.metrics.cancel_action_count == 0 {
        report
            .diagnostics
            .push("error:aui_focus_trap.not_closed".to_string());
    }
    if report.metrics.wheel_scroll_offset_change_count == 0 {
        report
            .diagnostics
            .push("error:aui_scroll.wheel_offset_not_changed".to_string());
    }
    if report.metrics.drag_scroll_offset_change_count == 0 {
        report
            .diagnostics
            .push("error:aui_scroll.drag_offset_not_changed".to_string());
    }
    if report.metrics.scroll_offset_applied_count < 2 {
        report
            .diagnostics
            .push("error:aui_scroll.layout_offset_not_applied".to_string());
    }
    if report.metrics.scene_selectable_proxy_count == 0 {
        report
            .diagnostics
            .push("error:aui_scene_hit_test.no_selectable_proxy".to_string());
    }
    if report.metrics.deferred_flag_count != 2 {
        report
            .next_actions
            .push("verify_214_deferred_flags".to_string());
    }
    report.status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        ComplexShooterAuiComplexControlsStatus::Failed
    } else {
        ComplexShooterAuiComplexControlsStatus::Passed
    };
}

fn write_report(
    output_root: PathBuf,
    mut report: ComplexShooterAuiComplexControlsReport,
) -> ComplexShooterAuiComplexControlsReport {
    let report_path = output_root
        .join("reports")
        .join("complex-shooter-aui-complex-controls-productization-report.json");
    if let Some(parent) = report_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            report.diagnostics.push(format!(
                "error:aui_complex_controls_report_dir_create_failed:{error}"
            ));
            report.status = ComplexShooterAuiComplexControlsStatus::Failed;
            return report;
        }
    }
    report.artifacts.push(report_path.display().to_string());
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(error) = fs::write(&report_path, json) {
                report.diagnostics.push(format!(
                    "error:aui_complex_controls_report_write_failed:{error}"
                ));
                report.status = ComplexShooterAuiComplexControlsStatus::Failed;
            }
        }
        Err(error) => report.diagnostics.push(format!(
            "error:aui_complex_controls_report_serialize_failed:{error}"
        )),
    }
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        report.status = ComplexShooterAuiComplexControlsStatus::Failed;
    }
    report
}

fn modal_document() -> AuiDocument {
    let background_root = AuiNode::new(
        "background_root",
        AuiNodeKind::Panel,
        AuiRect::stretch_full(),
    )
    .with_children(["background_button"]);
    let background_button = AuiNode::new(
        "background_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 20.0, 160.0, 60.0),
    )
    .with_parent("background_root")
    .with_interactable(true);
    let modal_root = AuiNode::new(
        "modal_root",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(200.0, 120.0, 300.0, 240.0),
    )
    .with_children(["modal_button_a", "modal_button_b"]);
    let modal_button_a = AuiNode::new(
        "modal_button_a",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 20.0, 120.0, 50.0),
    )
    .with_parent("modal_root")
    .with_interactable(true);
    let modal_button_b = AuiNode::new(
        "modal_button_b",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 90.0, 120.0, 50.0),
    )
    .with_parent("modal_root")
    .with_interactable(true)
    .with_action(AuiActionRef::cancel("ui.cancel"));
    let overlay = AuiCanvas::screen_overlay("overlay", 800.0, 600.0, "background_root");
    let mut modal = AuiCanvas::screen_overlay("modal", 800.0, 600.0, "modal_root");
    modal.composition_stage = AuiCompositionStage::Modal;
    modal.layer = 10;
    AuiDocument::new(
        "complex-controls-modal",
        vec![overlay, modal],
        vec![
            background_root,
            background_button,
            modal_root,
            modal_button_a,
            modal_button_b,
        ],
    )
}

fn scroll_document() -> AuiDocument {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["list"]);
    let list = AuiNode::new(
        "list",
        AuiNodeKind::ScrollView,
        AuiRect::fixed_position(10.0, 10.0, 120.0, 100.0),
    )
    .with_parent("root")
    .with_children(["item_0", "item_1", "item_2"])
    .with_action(AuiActionRef::scroll("ui.scroll"));
    let item_0 = AuiNode::new(
        "item_0",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 120.0, 80.0),
    )
    .with_parent("list");
    let item_1 = AuiNode::new(
        "item_1",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 90.0, 120.0, 80.0),
    )
    .with_parent("list");
    let item_2 = AuiNode::new(
        "item_2",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 180.0, 120.0, 80.0),
    )
    .with_parent("list");
    AuiDocument::new(
        "complex-controls-scroll",
        vec![AuiCanvas::screen_overlay("main", 320.0, 240.0, "root")],
        vec![root, list, item_0, item_1, item_2],
    )
}

fn runtime_frame(events: Vec<RuntimeInputEvent>) -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(1, "game-view");
    frame.events = events;
    frame
}
