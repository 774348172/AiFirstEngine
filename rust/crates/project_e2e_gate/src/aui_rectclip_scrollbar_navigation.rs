use engine_input::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
use engine_runtime::aui::{
    AuiCanvas, AuiDocument, AuiInteractionConfig, AuiInteractionResult, AuiInteractionState,
    AuiInteractionSystem, AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect,
    AuiRectClipScrollbarNavigationProductizationReport,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-aui-rectclip-scrollbar-navigation-productization-report.v1";
pub const COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_SCENARIO_ID: &str =
    "complex-shooter-aui-rectclip-scrollbar-navigation-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterAuiRectClipScrollbarNavigationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiRectClipScrollbarNavigationMetrics {
    pub clip_root_count: usize,
    pub effective_clip_item_count: usize,
    pub culled_draw_item_count: usize,
    pub hit_test_clip_rejected_count: usize,
    pub scrollbar_visible_count: usize,
    pub scrollbar_thumb_drag_count: usize,
    pub scrollbar_offset_change_count: usize,
    pub keyboard_navigation_event_count: usize,
    pub focus_move_count: usize,
    pub focus_visible_scroll_count: usize,
    pub deferred_flag_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterAuiRectClipScrollbarNavigationReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterAuiRectClipScrollbarNavigationStatus,
    pub project_root: String,
    pub output_root: String,
    pub metrics: ComplexShooterAuiRectClipScrollbarNavigationMetrics,
    pub core_report: AuiRectClipScrollbarNavigationProductizationReport,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComplexShooterAuiRectClipScrollbarNavigationRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterAuiRectClipScrollbarNavigationRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

pub fn run_complex_shooter_aui_rectclip_scrollbar_navigation_report(
    request: ComplexShooterAuiRectClipScrollbarNavigationRequest,
) -> ComplexShooterAuiRectClipScrollbarNavigationReport {
    let document = equipment_list_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (_, render_report) = AuiLayoutEngine::extract_draw_list(&document, &layout);

    let clipped_hit = AuiInteractionSystem::hit_test(&document, &layout, 24.0, 190.0);

    let metrics = layout
        .scrollbar_metrics
        .iter()
        .find(|metrics| metrics.scroll_node_id == "equipment_list" && metrics.visible)
        .expect("equipment list should produce a visible vertical scrollbar");
    let thumb_x = metrics.thumb_rect.x + metrics.thumb_rect.width * 0.5;
    let thumb_y = metrics.thumb_rect.y + metrics.thumb_rect.height * 0.5;
    let thumb_frame = runtime_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: thumb_x,
            y: thumb_y,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove {
            x: thumb_x,
            y: thumb_y + 22.0,
        },
        RuntimeInputEvent::PointerUp {
            x: thumb_x,
            y: thumb_y + 22.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut thumb_state = AuiInteractionState::default();
    let thumb_result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &thumb_frame,
        &mut thumb_state,
        AuiInteractionConfig::default(),
    );

    let navigation_frame = runtime_frame(vec![RuntimeInputEvent::KeyDown {
        key: "ArrowDown".to_string(),
    }]);
    let mut navigation_state = AuiInteractionState::default();
    navigation_state.focus.focused_node = Some("equipment_slot_1".to_string());
    let navigation_result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &navigation_frame,
        &mut navigation_state,
        AuiInteractionConfig::default(),
    );

    let mut combined = combine_results(thumb_result, navigation_result);
    combined.hit_test_clip_rejected_count += clipped_hit.clip_rejected_count;
    let core_report = AuiRectClipScrollbarNavigationProductizationReport::from_parts(
        &layout.report,
        &render_report,
        &combined,
        Some("equipment_slot_1".to_string()),
        navigation_state.focus.focused_node.clone(),
    );
    let mut report = ComplexShooterAuiRectClipScrollbarNavigationReport {
        schema_version: COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION
            .to_string(),
        scenario_id: COMPLEX_SHOOTER_AUI_RECTCLIP_SCROLLBAR_NAVIGATION_SCENARIO_ID.to_string(),
        status: ComplexShooterAuiRectClipScrollbarNavigationStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        metrics: ComplexShooterAuiRectClipScrollbarNavigationMetrics {
            clip_root_count: core_report.clip_root_count,
            effective_clip_item_count: core_report.effective_clip_item_count,
            culled_draw_item_count: core_report.culled_draw_item_count,
            hit_test_clip_rejected_count: core_report.hit_test_clip_rejected_count,
            scrollbar_visible_count: core_report.scrollbar_visible_count,
            scrollbar_thumb_drag_count: core_report.scrollbar_thumb_drag_count,
            scrollbar_offset_change_count: core_report.scrollbar_offset_change_count,
            keyboard_navigation_event_count: core_report.keyboard_navigation_event_count,
            focus_move_count: core_report.focus_move_count,
            focus_visible_scroll_count: core_report.focus_visible_scroll_count,
            deferred_flag_count: [
                core_report.stencil_mask_deferred,
                core_report.nested_scroll_deferred,
                core_report.inertia_elastic_deferred,
                core_report.virtualized_list_deferred,
                core_report.input_field_ime_deferred,
                core_report.full_gamepad_navigation_deferred,
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

fn combine_results(
    mut primary: AuiInteractionResult,
    secondary: AuiInteractionResult,
) -> AuiInteractionResult {
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
    primary
}

fn recompute(report: &mut ComplexShooterAuiRectClipScrollbarNavigationReport) {
    report.diagnostics.clear();
    report.next_actions.clear();
    if report.metrics.clip_root_count == 0 {
        report
            .diagnostics
            .push("error:aui_clip.no_clip_root".to_string());
    }
    if report.metrics.effective_clip_item_count == 0 {
        report
            .diagnostics
            .push("error:aui_clip.no_effective_clip_items".to_string());
    }
    if report.metrics.culled_draw_item_count == 0 {
        report
            .diagnostics
            .push("error:aui_clip.no_culled_draw_items".to_string());
    }
    if report.metrics.hit_test_clip_rejected_count == 0 {
        report
            .diagnostics
            .push("error:aui_hit_test.clip_not_rejected".to_string());
    }
    if report.metrics.scrollbar_visible_count == 0 {
        report
            .diagnostics
            .push("error:aui_scrollbar.not_visible".to_string());
    }
    if report.metrics.scrollbar_thumb_drag_count == 0
        || report.metrics.scrollbar_offset_change_count == 0
    {
        report
            .diagnostics
            .push("error:aui_scrollbar.thumb_drag_not_working".to_string());
    }
    if report.metrics.keyboard_navigation_event_count == 0 || report.metrics.focus_move_count == 0 {
        report
            .diagnostics
            .push("error:aui_navigation.focus_not_moved".to_string());
    }
    if report.metrics.focus_visible_scroll_count == 0 {
        report
            .diagnostics
            .push("error:aui_navigation.focus_not_scrolled_visible".to_string());
    }
    if report.metrics.deferred_flag_count != 6 {
        report
            .next_actions
            .push("verify_215_deferred_flags".to_string());
    }
    report.status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        ComplexShooterAuiRectClipScrollbarNavigationStatus::Failed
    } else {
        ComplexShooterAuiRectClipScrollbarNavigationStatus::Passed
    };
}

fn write_report(
    output_root: PathBuf,
    mut report: ComplexShooterAuiRectClipScrollbarNavigationReport,
) -> ComplexShooterAuiRectClipScrollbarNavigationReport {
    let report_path = output_root
        .join("reports")
        .join("complex-shooter-aui-rectclip-scrollbar-navigation-productization-report.json");
    if let Some(parent) = report_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            report.diagnostics.push(format!(
                "error:aui_rectclip_scrollbar_navigation_report_dir_create_failed:{error}"
            ));
            report.status = ComplexShooterAuiRectClipScrollbarNavigationStatus::Failed;
            return report;
        }
    }
    report.artifacts.push(report_path.display().to_string());
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(error) = fs::write(&report_path, json) {
                report.diagnostics.push(format!(
                    "error:aui_rectclip_scrollbar_navigation_report_write_failed:{error}"
                ));
                report.status = ComplexShooterAuiRectClipScrollbarNavigationStatus::Failed;
            }
        }
        Err(error) => report.diagnostics.push(format!(
            "error:aui_rectclip_scrollbar_navigation_report_serialize_failed:{error}"
        )),
    }
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        report.status = ComplexShooterAuiRectClipScrollbarNavigationStatus::Failed;
    }
    report
}

fn runtime_frame(events: Vec<RuntimeInputEvent>) -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(215, "game-view");
    frame.events = events;
    frame
}

fn equipment_list_document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["equipment_list"]);
    let list = AuiNode::new(
        "equipment_list",
        AuiNodeKind::ScrollView,
        AuiRect::fixed_position(16.0, 16.0, 180.0, 110.0),
    )
    .with_parent("root")
    .with_children([
        "equipment_slot_0",
        "equipment_slot_1",
        "equipment_slot_2",
        "equipment_slot_3",
    ]);
    let slot_0 = AuiNode::new(
        "equipment_slot_0",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 156.0, 38.0),
    )
    .with_parent("equipment_list")
    .with_interactable(true);
    let slot_1 = AuiNode::new(
        "equipment_slot_1",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 46.0, 156.0, 38.0),
    )
    .with_parent("equipment_list")
    .with_interactable(true);
    let slot_2 = AuiNode::new(
        "equipment_slot_2",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 138.0, 156.0, 38.0),
    )
    .with_parent("equipment_list")
    .with_interactable(true);
    let slot_3 = AuiNode::new(
        "equipment_slot_3",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 184.0, 156.0, 38.0),
    )
    .with_parent("equipment_list")
    .with_interactable(true);
    AuiDocument::new(
        "complex-shooter-equipment-ui",
        vec![AuiCanvas::screen_overlay("main", 360.0, 240.0, "root")],
        vec![root, list, slot_0, slot_1, slot_2, slot_3],
    )
}
