use engine_input::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
use engine_runtime::aui::{
    AuiActionRef, AuiCanvas, AuiCompositionStage, AuiDocument, AuiInteractionConfig,
    AuiInteractionResult, AuiInteractionState, AuiInteractionSystem, AuiLayoutEngine, AuiNode,
    AuiNodeKind, AuiRect, AuiRuntimeNavigationScreenFlowTextEntryProductizationReport,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-aui-runtime-navigation-screenflow-textentry-productization-report.v1";
pub const COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_SCENARIO_ID: &str =
    "complex-shooter-aui-runtime-navigation-screenflow-textentry-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryMetrics {
    pub submit_count: usize,
    pub cancel_count: usize,
    pub screen_stack_push_count: usize,
    pub screen_stack_pop_count: usize,
    pub default_focus_applied_count: usize,
    pub focus_restore_count: usize,
    pub gamepad_intent_count: usize,
    pub keyboard_navigation_event_count: usize,
    pub text_edit_session_count: usize,
    pub text_changed_count: usize,
    pub text_submitted_count: usize,
    pub ime_preedit_count: usize,
    pub ime_commit_count: usize,
    pub ime_cancel_count: usize,
    pub gameplay_input_filtered_count: usize,
    pub deferred_flag_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus,
    pub project_root: String,
    pub output_root: String,
    pub metrics: ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryMetrics,
    pub core_report: AuiRuntimeNavigationScreenFlowTextEntryProductizationReport,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_aui_runtime_navigation_screenflow_textentry_report(
    request: ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryRequest,
) -> ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryReport {
    let document = menu_text_entry_document();
    let mut aggregate = AuiInteractionResult::default();

    let mut screen_state = AuiInteractionState::default();
    screen_state.focus.focused_node = Some("play_button".to_string());
    AuiInteractionSystem::push_screen(&document, &mut screen_state, "pause_screen")
        .expect("pause screen should push in 216 gate fixture");
    let pushed_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &screen_state);
    let screen_result = AuiInteractionSystem::process_with_state(
        &document,
        &pushed_layout,
        &runtime_frame(vec![RuntimeInputEvent::KeyDown {
            key: "Escape".to_string(),
        }]),
        &mut screen_state,
        AuiInteractionConfig::default(),
    );
    combine_result(&mut aggregate, screen_result);

    let mut gamepad_state = AuiInteractionState::default();
    gamepad_state.focus.focused_node = Some("play_button".to_string());
    let gamepad_layout = AuiLayoutEngine::layout(&document, 2);
    let gamepad_result = AuiInteractionSystem::process_with_state(
        &document,
        &gamepad_layout,
        &runtime_frame(vec![
            RuntimeInputEvent::GamepadButtonDown {
                gamepad_id: 0,
                button: "DPadDown".to_string(),
            },
            RuntimeInputEvent::GamepadButtonDown {
                gamepad_id: 0,
                button: "South".to_string(),
            },
        ]),
        &mut gamepad_state,
        AuiInteractionConfig::default(),
    );
    combine_result(&mut aggregate, gamepad_result);

    let mut text_state = AuiInteractionState::default();
    AuiInteractionSystem::push_screen(&document, &mut text_state, "pause_screen")
        .expect("pause screen should push before text entry");
    let text_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 3, &text_state);
    let text_frame = runtime_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 250.0,
            y: 70.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::TextInput {
            text: "B".to_string(),
        },
        RuntimeInputEvent::ImePreedit {
            text: "ni".to_string(),
            cursor_start: 0,
            cursor_end: 2,
        },
        RuntimeInputEvent::ImeCancel,
        RuntimeInputEvent::ImeCommit {
            text: "hao".to_string(),
        },
        RuntimeInputEvent::KeyDown {
            key: "Enter".to_string(),
        },
    ]);
    let text_result = AuiInteractionSystem::process_with_state(
        &document,
        &text_layout,
        &text_frame,
        &mut text_state,
        AuiInteractionConfig::default(),
    );
    let total_input_count = 1 + 2 + text_frame.events.len();
    combine_result(&mut aggregate, text_result);
    let consumed_input_count = aggregate
        .consumed_event_count_by_kind
        .values()
        .copied()
        .sum::<usize>();
    let filtered_input_count = total_input_count.saturating_sub(consumed_input_count);

    let core_report = AuiRuntimeNavigationScreenFlowTextEntryProductizationReport::from_result(
        &document,
        total_input_count,
        filtered_input_count,
        &aggregate,
    );
    let mut report = ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryReport {
        schema_version:
            COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION
                .to_string(),
        scenario_id: COMPLEX_SHOOTER_AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_SCENARIO_ID
            .to_string(),
        status: ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        metrics: ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryMetrics {
            submit_count: core_report.submit_count,
            cancel_count: core_report.cancel_count,
            screen_stack_push_count: core_report.screen_stack_push_count,
            screen_stack_pop_count: core_report.screen_stack_pop_count,
            default_focus_applied_count: core_report.default_focus_applied_count,
            focus_restore_count: core_report.focus_restore_count,
            gamepad_intent_count: core_report.gamepad_intent_count,
            keyboard_navigation_event_count: aggregate.keyboard_navigation_event_count,
            text_edit_session_count: core_report.text_edit_session_count,
            text_changed_count: core_report.text_changed_count,
            text_submitted_count: core_report.text_submitted_count,
            ime_preedit_count: core_report.ime_preedit_count,
            ime_commit_count: core_report.ime_commit_count,
            ime_cancel_count: core_report.ime_cancel_count,
            gameplay_input_filtered_count: core_report.gameplay_input_filtered_count,
            deferred_flag_count: [
                core_report.rich_text_deferred,
                core_report.ime_candidate_window_deferred,
                core_report.accessibility_deferred,
                core_report.screen_transition_animation_deferred,
                core_report.clipboard_full_deferred,
                core_report.multi_line_text_edit_deferred,
                core_report.common_ui_action_bar_deferred,
                core_report.dirty_cache_batch_deferred,
                core_report.touch_virtual_keyboard_deferred,
                core_report.multi_user_input_deferred,
            ]
            .into_iter()
            .filter(|flag| *flag)
            .count(),
        },
        core_report,
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };
    recompute(&mut report);
    write_report(request.output_root, report)
}

fn combine_result(primary: &mut AuiInteractionResult, secondary: AuiInteractionResult) {
    primary.consumed |= secondary.consumed;
    primary
        .consumed_event_indices
        .extend(secondary.consumed_event_indices);
    for (kind, count) in secondary.consumed_event_count_by_kind {
        *primary
            .consumed_event_count_by_kind
            .entry(kind)
            .or_insert(0) += count;
    }
    primary.commands.extend(secondary.commands);
    primary.actions.extend(secondary.actions);
    primary.traces.extend(secondary.traces);
    primary.focus_change_count += secondary.focus_change_count;
    primary.scroll_offset_change_count += secondary.scroll_offset_change_count;
    primary.hit_test_clip_rejected_count += secondary.hit_test_clip_rejected_count;
    primary.keyboard_navigation_event_count += secondary.keyboard_navigation_event_count;
    primary.focus_visible_scroll_count += secondary.focus_visible_scroll_count;
    primary.normalized_ui_intent_count += secondary.normalized_ui_intent_count;
    primary.keyboard_intent_count += secondary.keyboard_intent_count;
    primary.gamepad_intent_count += secondary.gamepad_intent_count;
    primary.submit_count += secondary.submit_count;
    primary.cancel_count += secondary.cancel_count;
    primary.screen_stack_push_count += secondary.screen_stack_push_count;
    primary.screen_stack_pop_count += secondary.screen_stack_pop_count;
    primary.default_focus_applied_count += secondary.default_focus_applied_count;
    primary.focus_restore_count += secondary.focus_restore_count;
    primary.text_edit_session_count += secondary.text_edit_session_count;
    primary.text_changed_count += secondary.text_changed_count;
    primary.text_submitted_count += secondary.text_submitted_count;
    primary.text_cancelled_count += secondary.text_cancelled_count;
    primary.caret_move_count += secondary.caret_move_count;
    primary.selection_change_count += secondary.selection_change_count;
    primary.ime_preedit_count += secondary.ime_preedit_count;
    primary.ime_commit_count += secondary.ime_commit_count;
    primary.ime_cancel_count += secondary.ime_cancel_count;
    primary.action_prompt_reported |= secondary.action_prompt_reported;
    primary.focusable_derived_from_interactable |= secondary.focusable_derived_from_interactable;
    if !secondary.ime_platform_coverage.is_empty() {
        primary.ime_platform_coverage = secondary.ime_platform_coverage;
    }
}

fn recompute(report: &mut ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryReport) {
    report.diagnostics.clear();
    report.next_actions.clear();
    if report.metrics.screen_stack_push_count == 0 || report.metrics.screen_stack_pop_count == 0 {
        report
            .diagnostics
            .push("error:aui_screen_flow.stack_push_pop_missing".to_string());
    }
    if report.metrics.default_focus_applied_count == 0 || report.metrics.focus_restore_count == 0 {
        report
            .diagnostics
            .push("error:aui_screen_flow.focus_restore_missing".to_string());
    }
    if report.metrics.gamepad_intent_count < 2
        || report.metrics.keyboard_navigation_event_count == 0
        || report.metrics.submit_count == 0
    {
        report
            .diagnostics
            .push("error:aui_navigation.gamepad_submit_missing".to_string());
    }
    if report.metrics.text_edit_session_count == 0
        || report.metrics.text_changed_count == 0
        || report.metrics.text_submitted_count == 0
    {
        report
            .diagnostics
            .push("error:aui_text_entry.lifecycle_missing".to_string());
    }
    if report.metrics.ime_preedit_count == 0
        || report.metrics.ime_commit_count == 0
        || report.metrics.ime_cancel_count == 0
    {
        report
            .diagnostics
            .push("error:aui_text_entry.ime_missing".to_string());
    }
    if report.core_report.ime_platform_coverage != "schema_headless_and_winit_cmin" {
        report
            .diagnostics
            .push("error:aui_text_entry.ime_platform_coverage_missing".to_string());
    }
    if report.metrics.deferred_flag_count != 10 {
        report
            .next_actions
            .push("verify_216_deferred_flags".to_string());
    }
    report.status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus::Failed
    } else {
        ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus::Passed
    };
}

fn write_report(
    output_root: PathBuf,
    mut report: ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryReport,
) -> ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryReport {
    let report_path = output_root.join("reports").join(
        "complex-shooter-aui-runtime-navigation-screenflow-textentry-productization-report.json",
    );
    if let Some(parent) = report_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            report.diagnostics.push(format!(
                "error:aui_runtime_navigation_screenflow_textentry_report_dir_create_failed:{error}"
            ));
            report.status = ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus::Failed;
            return report;
        }
    }
    report.artifacts.push(report_path.display().to_string());
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(error) = fs::write(&report_path, json) {
                report.diagnostics.push(format!(
                    "error:aui_runtime_navigation_screenflow_textentry_report_write_failed:{error}"
                ));
                report.status = ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus::Failed;
            }
        }
        Err(error) => report.diagnostics.push(format!(
            "error:aui_runtime_navigation_screenflow_textentry_report_serialize_failed:{error}"
        )),
    }
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        report.status = ComplexShooterAuiRuntimeNavigationScreenFlowTextEntryStatus::Failed;
    }
    report
}

fn runtime_frame(events: Vec<RuntimeInputEvent>) -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(216, "game-view");
    frame.events = events;
    frame
}

fn menu_text_entry_document() -> AuiDocument {
    let main_root = AuiNode::new("main_root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["play_button", "settings_button"]);
    let play_button = AuiNode::new(
        "play_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 20.0, 160.0, 40.0),
    )
    .with_parent("main_root")
    .with_interactable(true)
    .with_action(AuiActionRef::submit("ui.play"));
    let settings_button = AuiNode::new(
        "settings_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 80.0, 160.0, 40.0),
    )
    .with_parent("main_root")
    .with_interactable(true)
    .with_action(AuiActionRef::submit("ui.settings"));
    let pause_root = AuiNode::new(
        "pause_root",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(220.0, 40.0, 260.0, 180.0),
    )
    .with_children(["name_input", "resume_button"]);
    let name_input = AuiNode::new(
        "name_input",
        AuiNodeKind::InputField,
        AuiRect::fixed_position(20.0, 20.0, 180.0, 36.0),
    )
    .with_parent("pause_root")
    .with_interactable(true)
    .with_text("A")
    .with_action(AuiActionRef::text_changed("ui.name_changed"))
    .with_action(AuiActionRef::text_submitted("ui.name_submitted"))
    .with_action(AuiActionRef::text_cancelled("ui.name_cancelled"));
    let resume_button = AuiNode::new(
        "resume_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 76.0, 180.0, 36.0),
    )
    .with_parent("pause_root")
    .with_interactable(true)
    .with_action(AuiActionRef::submit("ui.resume"));
    let mut main_canvas = AuiCanvas::screen_overlay("main", 640.0, 360.0, "main_root");
    main_canvas.default_focus_node_id = Some("play_button".to_string());
    let mut pause_canvas = AuiCanvas::screen_overlay("pause", 640.0, 360.0, "pause_root");
    pause_canvas.composition_stage = AuiCompositionStage::Modal;
    pause_canvas.layer = 10;
    pause_canvas.visible = false;
    pause_canvas.screen_id = Some("pause_screen".to_string());
    pause_canvas.default_focus_node_id = Some("name_input".to_string());
    pause_canvas.cancel_action_id = Some("ui.pause_cancel".to_string());
    AuiDocument::new(
        "complex-shooter-runtime-menu-text-entry",
        vec![main_canvas, pause_canvas],
        vec![
            main_root,
            play_button,
            settings_button,
            pause_root,
            name_input,
            resume_button,
        ],
    )
}
