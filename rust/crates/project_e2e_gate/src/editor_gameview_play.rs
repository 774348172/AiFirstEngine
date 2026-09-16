use editor_core::{
    command_for_test, ApplyRuntimeChangeCandidateStatus, CommandStatus, GameViewPresentStatus,
};
use editor_ui_model::UiCommandPayload;
use engine_runtime::input_action::PointerPosition;
use engine_runtime::input_mapping::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-editor-gameview-play-runner-productization-report.v1";
pub const COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_SCENARIO_ID: &str =
    "complex-shooter-editor-gameview-play-runner-productization-v1";
pub const COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-editor-gameview-gpu-texture-present-productization-report.v1";
pub const COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_SCENARIO_ID: &str =
    "complex-shooter-editor-gameview-gpu-texture-present-productization-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterEditorGameViewPlayRunnerStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterEditorGameViewGpuTexturePresentStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorGameViewPlayRunnerMetrics {
    pub preview_package_report_present: bool,
    pub play_session_report_present: bool,
    pub game_view_present_report_present: bool,
    pub frame_count: u64,
    pub renderable_count: usize,
    pub ui_draw_item_count: usize,
    pub has_frame_hash: bool,
    pub texture_descriptor_status: String,
    pub input_bridge_status: String,
    pub runtime_input_event_count: usize,
    pub filtered_runtime_input_event_count: usize,
    pub aui_consumed_event_count: usize,
    pub gameplay_action_count: usize,
    pub gameplay_action_ids: Vec<String>,
    pub report_panel_provider_present: bool,
    pub viewport_descriptor_present: bool,
    pub game_view_report_path_exists: bool,
    pub maximize_on_play_enabled: bool,
    pub game_view_maximized_during_play: bool,
    pub game_view_maximize_restored_after_stop: bool,
    pub pause_kept_instance: bool,
    pub pause_did_not_advance_frame: bool,
    pub paused_tick_reused_last_frame: bool,
    pub step_advanced_exactly_one_frame: bool,
    pub resume_kept_instance: bool,
    pub stop_cleared_instance: bool,
    pub runtime_selection_pick_committed: bool,
    pub runtime_selection_selected_entity_id: Option<String>,
    pub runtime_selection_source: String,
    pub runtime_hierarchy_source_domain: String,
    pub runtime_inspector_readonly: bool,
    pub runtime_inspector_temporary_play_session: bool,
    pub runtime_inspector_transform_present: bool,
    pub runtime_inspector_transform_editable: bool,
    pub runtime_temporary_edit_committed: bool,
    pub runtime_apply_preview_ready: bool,
    pub runtime_apply_committed: bool,
    pub runtime_apply_authoring_scene_updated: bool,
    pub runtime_apply_pending_summary_cleared: bool,
    pub runtime_temporary_edit_discarded_on_stop: bool,
    pub runtime_pick_blocked_by_aui_kept_selection: bool,
    pub runtime_pick_miss_diagnostic_present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorGameViewPlayRunnerReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterEditorGameViewPlayRunnerStatus,
    pub project_root: String,
    pub output_root: String,
    pub metrics: ComplexShooterEditorGameViewPlayRunnerMetrics,
    pub preview_package_report_path: Option<String>,
    pub play_runner_kind: Option<String>,
    pub game_view_present_report_path: Option<String>,
    pub last_frame_hash: Option<String>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterEditorGameViewPlayRunnerRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
}

impl ComplexShooterEditorGameViewPlayRunnerRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorGameViewGpuTexturePresentMetrics {
    pub preview_package_report_present: bool,
    pub play_session_report_present: bool,
    pub game_view_present_report_present: bool,
    pub viewport_descriptor_present: bool,
    pub has_frame_hash: bool,
    pub frame_count: u64,
    pub texture_descriptor_status: String,
    pub gpu_present_status: String,
    pub shared_gpu_context_status: String,
    pub descriptor_only_not_presented: bool,
    pub rhi_command_count: usize,
    pub render_graph_pass_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterEditorGameViewGpuTexturePresentReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterEditorGameViewGpuTexturePresentStatus,
    pub project_root: String,
    pub output_root: String,
    pub metrics: ComplexShooterEditorGameViewGpuTexturePresentMetrics,
    pub game_view_present_report_path: Option<String>,
    pub last_frame_hash: Option<String>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

pub fn run_complex_shooter_editor_gameview_play_runner_report(
    request: ComplexShooterEditorGameViewPlayRunnerRequest,
) -> ComplexShooterEditorGameViewPlayRunnerReport {
    let mut report = ComplexShooterEditorGameViewPlayRunnerReport {
        schema_version: COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_REPORT_SCHEMA_VERSION
            .to_string(),
        scenario_id: COMPLEX_SHOOTER_EDITOR_GAMEVIEW_PLAY_RUNNER_SCENARIO_ID.to_string(),
        status: ComplexShooterEditorGameViewPlayRunnerStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        metrics: ComplexShooterEditorGameViewPlayRunnerMetrics::default(),
        preview_package_report_path: None,
        play_runner_kind: None,
        game_view_present_report_path: None,
        last_frame_hash: None,
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };

    let mut session = crate::complex_shooter_editor_session();
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_root.display().to_string(),
    }));
    if open.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:sample_project_open_failed".to_string());
    }

    let maximize = session.execute_command(command_for_test(
        UiCommandPayload::ToggleGameViewMaximizeOnPlay,
    ));
    if maximize.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:maximize_on_play_toggle_failed".to_string());
    }

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    if play.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:editor_gameview_play_command_failed".to_string());
    }
    let playing_model = session.build_ui_model();
    report.metrics.maximize_on_play_enabled =
        playing_model.toolbar.game_view_layout.maximize_on_play;
    report.metrics.game_view_maximized_during_play = playing_model
        .toolbar
        .game_view_layout
        .is_game_view_maximized;
    if !report.metrics.maximize_on_play_enabled {
        report
            .diagnostics
            .push("fail:maximize_on_play_not_enabled".to_string());
    }
    if !report.metrics.game_view_maximized_during_play {
        report
            .diagnostics
            .push("fail:game_view_not_maximized_during_play".to_string());
    }
    let input_tick = session.tick_active_game_view_runtime_descriptor_frame_with_input(
        complex_shooter_gameplay_pointer_frame(),
    );
    if input_tick.is_none() {
        report
            .diagnostics
            .push("fail:editor_gameview_input_tick_missing_active_instance".to_string());
    }

    if let Some(preview) = session.last_editor_preview_package_report() {
        report.metrics.preview_package_report_present = true;
        report.preview_package_report_path = preview.report_path.clone();
        if !matches!(
            preview.status,
            editor_core::EditorPreviewPackageStatus::Success
        ) {
            report
                .diagnostics
                .push("fail:preview_package_not_success".to_string());
        }
    } else {
        report
            .diagnostics
            .push("fail:preview_package_report_missing".to_string());
    }

    if let Some(play_report) = session.last_play_session_report() {
        report.metrics.play_session_report_present = true;
        report.play_runner_kind = play_report.runner_kind.clone();
        if play_report.runner_kind.as_deref() != Some("editor_in_process_gameview") {
            report
                .diagnostics
                .push("fail:play_runner_kind_not_editor_gameview".to_string());
        }
        if play_report.game_view_frame_count.unwrap_or_default() == 0 {
            report
                .diagnostics
                .push("fail:play_report_game_view_frame_count_missing".to_string());
        }
    } else {
        report
            .diagnostics
            .push("fail:play_session_report_missing".to_string());
    }

    if let Some(game_view) = session.last_game_view_present_report() {
        report.metrics.game_view_present_report_present = true;
        report.metrics.frame_count = game_view.frame_count;
        report.metrics.texture_descriptor_status = game_view.texture_descriptor_status.clone();
        report.metrics.input_bridge_status = game_view.input_bridge_status.clone();
        report.metrics.runtime_input_event_count = game_view.runtime_input_event_count;
        report.metrics.filtered_runtime_input_event_count =
            game_view.filtered_runtime_input_event_count;
        report.metrics.aui_consumed_event_count = game_view.aui_consumed_event_count;
        report.metrics.gameplay_action_count = game_view.gameplay_action_count;
        report.metrics.gameplay_action_ids = game_view.gameplay_action_ids.clone();
        report.game_view_present_report_path = game_view.report_path.clone();
        report.last_frame_hash = game_view.last_frame_hash.clone();
        if game_view.status != GameViewPresentStatus::Success {
            report
                .diagnostics
                .push("fail:game_view_present_status_not_success".to_string());
        }
        if game_view.frame_count == 0 {
            report
                .diagnostics
                .push("fail:game_view_frame_count_zero".to_string());
        }
        if game_view.texture_descriptor_status != "descriptor_only" {
            report
                .diagnostics
                .push("fail:texture_descriptor_status_not_descriptor_only".to_string());
        }
        if matches!(
            game_view.input_bridge_status.as_str(),
            "deferred" | "not_requested"
        ) {
            report
                .diagnostics
                .push("fail:input_bridge_status_not_runtime_input_frame".to_string());
        }
        if game_view.runtime_input_event_count == 0 {
            report
                .diagnostics
                .push("fail:runtime_input_event_count_zero".to_string());
        }
        if !game_view
            .gameplay_action_ids
            .iter()
            .any(|action_id| action_id == "action.fire")
        {
            report
                .diagnostics
                .push("fail:gameplay_fire_action_missing_after_gameview_input".to_string());
        }
        if let Some(path) = &game_view.report_path {
            report.metrics.game_view_report_path_exists = Path::new(path).exists();
            if !report.metrics.game_view_report_path_exists {
                report
                    .diagnostics
                    .push("fail:game_view_report_path_missing_on_disk".to_string());
            }
            report.artifacts.push(path.clone());
        }
    } else {
        report
            .diagnostics
            .push("fail:game_view_present_report_missing".to_string());
    }

    if let Some(frame) = session.last_game_view_runtime_frame() {
        report.metrics.renderable_count = frame.renderable_count;
        report.metrics.ui_draw_item_count = frame.ui_draw_item_count;
        report.metrics.runtime_input_event_count = frame.runtime_input_event_count;
        report.metrics.filtered_runtime_input_event_count =
            frame.filtered_runtime_input_event_count;
        report.metrics.aui_consumed_event_count = frame.aui_consumed_event_count;
        report.metrics.gameplay_action_count = frame.gameplay_action_count;
        report.metrics.gameplay_action_ids = frame.gameplay_action_ids.clone();
        report.metrics.has_frame_hash = !frame.frame_hash.is_empty();
        if frame.renderable_count == 0 {
            report
                .diagnostics
                .push("fail:game_view_renderable_count_zero".to_string());
        }
        if frame.ui_draw_item_count == 0 {
            report
                .diagnostics
                .push("fail:game_view_ui_draw_item_count_zero".to_string());
        }
        if frame.texture_descriptor_status != "descriptor_only" {
            report
                .diagnostics
                .push("fail:frame_texture_descriptor_not_descriptor_only".to_string());
        }
    } else {
        report
            .diagnostics
            .push("fail:game_view_runtime_frame_missing".to_string());
    }

    let model = session.build_ui_model();
    report.metrics.report_panel_provider_present = model
        .report_panel
        .reports
        .iter()
        .any(|entry| entry.provider_id == "play.game_view_present");
    report.metrics.viewport_descriptor_present =
        model.viewport.texture_id.is_some() && model.viewport.target_id.is_some();
    if !report.metrics.report_panel_provider_present {
        report
            .diagnostics
            .push("fail:report_panel_game_view_provider_missing".to_string());
    }
    if !report.metrics.viewport_descriptor_present {
        report
            .diagnostics
            .push("fail:viewport_descriptor_missing".to_string());
    }

    let frame_before_pause = session
        .last_game_view_present_report()
        .map(|game_view| game_view.frame_count)
        .unwrap_or_default();
    let pause = session.execute_command(command_for_test(UiCommandPayload::Pause));
    report.metrics.pause_kept_instance = pause.status == CommandStatus::Committed
        && session.has_active_editor_runtime_play_instance();
    let paused_frame = session
        .last_game_view_present_report()
        .map(|game_view| game_view.frame_count)
        .unwrap_or_default();
    report.metrics.pause_did_not_advance_frame = paused_frame == frame_before_pause;
    if !report.metrics.pause_kept_instance {
        report
            .diagnostics
            .push("fail:pause_did_not_keep_editor_runtime_play_instance".to_string());
    }
    if !report.metrics.pause_did_not_advance_frame {
        report
            .diagnostics
            .push("fail:pause_advanced_frame".to_string());
    }

    let paused_tick = session.tick_active_game_view_runtime_descriptor_frame();
    report.metrics.paused_tick_reused_last_frame = paused_tick.is_some_and(|game_view| {
        game_view.frame_count == paused_frame
            && game_view.paused_last_frame_reused
            && !game_view.runtime_advanced
    });
    if !report.metrics.paused_tick_reused_last_frame {
        report
            .diagnostics
            .push("fail:paused_tick_did_not_reuse_last_frame".to_string());
    }

    let runtime_pick =
        session.execute_command(command_for_test(UiCommandPayload::PickRuntimeEntityAt {
            x: 400.0,
            y: 480.0,
            viewport_width: Some(800.0),
            viewport_height: Some(600.0),
            aui_consumed: false,
        }));
    report.metrics.runtime_selection_pick_committed =
        runtime_pick.status == CommandStatus::Committed;
    let runtime_model = session.build_ui_model();
    report.metrics.runtime_hierarchy_source_domain =
        format!("{:?}", runtime_model.hierarchy.source_domain);
    let runtime_inspector = runtime_model.inspector;
    report.metrics.runtime_selection_selected_entity_id =
        runtime_inspector.selected_entity_id.clone();
    report.metrics.runtime_inspector_readonly = runtime_inspector.readonly;
    report.metrics.runtime_inspector_temporary_play_session = runtime_inspector.persistence
        == editor_ui_model::InspectorPersistence::TemporaryPlaySession;
    report.metrics.runtime_inspector_transform_present = runtime_inspector
        .sections
        .iter()
        .any(|section| section.section_id == "transform");
    report.metrics.runtime_inspector_transform_editable = runtime_inspector
        .sections
        .iter()
        .flat_map(|section| section.fields.iter())
        .any(|field| field.field_id == "transform.localPosition" && field.editable);
    report.metrics.runtime_selection_source = runtime_inspector
        .sections
        .iter()
        .find(|section| section.section_id == "metadata")
        .and_then(|section| {
            section
                .fields
                .iter()
                .find(|field| field.field_id == "metadata.selectionSource")
        })
        .and_then(|field| match &field.value {
            editor_ui_model::InspectorValue::String(value) => Some(value.clone()),
            _ => None,
        })
        .unwrap_or_default();
    if !report.metrics.runtime_selection_pick_committed {
        report
            .diagnostics
            .push("fail:runtime_selection_pick_not_committed".to_string());
    }
    if report
        .metrics
        .runtime_selection_selected_entity_id
        .as_deref()
        != Some("entity-player")
    {
        report
            .diagnostics
            .push("fail:runtime_selection_did_not_pick_player".to_string());
    }
    if report.metrics.runtime_selection_source != "active_game_view_runtime" {
        report
            .diagnostics
            .push("fail:runtime_selection_source_not_active_gameview".to_string());
    }
    if report.metrics.runtime_hierarchy_source_domain != "ActiveGameViewRuntime" {
        report
            .diagnostics
            .push("fail:runtime_hierarchy_source_not_active_runtime".to_string());
    }
    if report.metrics.runtime_inspector_readonly {
        report
            .diagnostics
            .push("fail:runtime_inspector_still_readonly".to_string());
    }
    if !report.metrics.runtime_inspector_temporary_play_session {
        report
            .diagnostics
            .push("fail:runtime_inspector_not_temporary_play_session".to_string());
    }
    if !report.metrics.runtime_inspector_transform_present {
        report
            .diagnostics
            .push("fail:runtime_inspector_transform_missing".to_string());
    }
    if !report.metrics.runtime_inspector_transform_editable {
        report
            .diagnostics
            .push("fail:runtime_inspector_transform_not_editable".to_string());
    }

    let temporary_edit = session.execute_command(command_for_test(
        UiCommandPayload::SetRuntimeComponentFieldTemporary {
            entity_id: "entity-player".to_string(),
            component_type: "Transform".to_string(),
            field_path: "local_position.x".to_string(),
            value: serde_json::json!(2.0),
        },
    ));
    report.metrics.runtime_temporary_edit_committed =
        temporary_edit.status == CommandStatus::Committed;
    if !report.metrics.runtime_temporary_edit_committed {
        report
            .diagnostics
            .push("fail:runtime_temporary_edit_not_committed".to_string());
    }
    let apply_preview = session.execute_command(command_for_test(
        UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring,
    ));
    let apply_candidate = session
        .last_runtime_apply_report()
        .and_then(|runtime_apply| runtime_apply.candidates.first())
        .cloned();
    report.metrics.runtime_apply_preview_ready = apply_preview.status == CommandStatus::Committed
        && apply_candidate
            .as_ref()
            .is_some_and(|candidate| candidate.status == ApplyRuntimeChangeCandidateStatus::Ready);
    if !report.metrics.runtime_apply_preview_ready {
        report
            .diagnostics
            .push("fail:runtime_apply_preview_not_ready".to_string());
    }
    if let Some(candidate) = apply_candidate {
        let apply = session.execute_command(command_for_test(
            UiCommandPayload::ApplyRuntimeChangeToAuthoring {
                edit_id: candidate.edit_id.clone(),
                candidate_hash: candidate.candidate_hash.clone(),
            },
        ));
        report.metrics.runtime_apply_committed = apply.status == CommandStatus::Committed;
        report.metrics.runtime_apply_authoring_scene_updated = session
            .editor_scene_document()
            .and_then(|document| document.entity("entity-player"))
            .and_then(|entity| entity.transform)
            .is_some_and(|transform| (transform.local_position.x - 2.0).abs() < f32::EPSILON);
        let post_apply_preview = session.execute_command(command_for_test(
            UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring,
        ));
        report.metrics.runtime_apply_pending_summary_cleared = post_apply_preview.status
            == CommandStatus::Committed
            && session
                .last_runtime_apply_report()
                .is_some_and(|runtime_apply| runtime_apply.candidate_count == 0);
    }
    if !report.metrics.runtime_apply_committed {
        report
            .diagnostics
            .push("fail:runtime_apply_not_committed".to_string());
    }
    if !report.metrics.runtime_apply_authoring_scene_updated {
        report
            .diagnostics
            .push("fail:runtime_apply_authoring_scene_not_updated".to_string());
    }
    if !report.metrics.runtime_apply_pending_summary_cleared {
        report
            .diagnostics
            .push("fail:runtime_apply_pending_summary_not_cleared".to_string());
    }
    let discard_probe = session.execute_command(command_for_test(
        UiCommandPayload::SetRuntimeComponentFieldTemporary {
            entity_id: "entity-player".to_string(),
            component_type: "Transform".to_string(),
            field_path: "local_position.y".to_string(),
            value: serde_json::json!(3.0),
        },
    ));
    if discard_probe.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:runtime_temporary_edit_discard_probe_not_committed".to_string());
    }

    let blocked =
        session.execute_command(command_for_test(UiCommandPayload::PickRuntimeEntityAt {
            x: 400.0,
            y: 480.0,
            viewport_width: Some(800.0),
            viewport_height: Some(600.0),
            aui_consumed: true,
        }));
    report.metrics.runtime_pick_blocked_by_aui_kept_selection = blocked.status
        == CommandStatus::Committed
        && session
            .build_ui_model()
            .inspector
            .selected_entity_id
            .as_deref()
            == Some("entity-player")
        && blocked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("blocked_by_aui"));
    if !report.metrics.runtime_pick_blocked_by_aui_kept_selection {
        report
            .diagnostics
            .push("fail:runtime_pick_blocked_by_aui_not_preserved".to_string());
    }

    let miss = session.execute_command(command_for_test(UiCommandPayload::PickRuntimeEntityAt {
        x: 760.0,
        y: 540.0,
        viewport_width: Some(800.0),
        viewport_height: Some(600.0),
        aui_consumed: false,
    }));
    report.metrics.runtime_pick_miss_diagnostic_present = miss.status == CommandStatus::Committed
        && miss
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("miss"));
    if !report.metrics.runtime_pick_miss_diagnostic_present {
        report
            .diagnostics
            .push("fail:runtime_pick_miss_diagnostic_missing".to_string());
    }

    let step = session.execute_command(command_for_test(UiCommandPayload::StepFrame));
    let stepped_frame = session
        .last_game_view_present_report()
        .map(|game_view| game_view.frame_count)
        .unwrap_or_default();
    report.metrics.step_advanced_exactly_one_frame =
        step.status == CommandStatus::Committed && stepped_frame == paused_frame + 1;
    if !report.metrics.step_advanced_exactly_one_frame {
        report
            .diagnostics
            .push("fail:step_frame_did_not_advance_exactly_one_frame".to_string());
    }

    let resume = session.execute_command(command_for_test(UiCommandPayload::Play));
    report.metrics.resume_kept_instance = resume.status == CommandStatus::Committed
        && session.has_active_editor_runtime_play_instance()
        && session
            .last_game_view_present_report()
            .is_some_and(|game_view| {
                !game_view.runtime_advanced && game_view.control_command == "resume"
            });
    if !report.metrics.resume_kept_instance {
        report
            .diagnostics
            .push("fail:resume_did_not_keep_editor_runtime_play_instance".to_string());
    }

    let stop = session.execute_command(command_for_test(UiCommandPayload::StopPlaySession));
    report.metrics.stop_cleared_instance = stop.status == CommandStatus::Committed
        && !session.has_active_editor_runtime_play_instance();
    report.metrics.runtime_temporary_edit_discarded_on_stop = stop
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "runtime_temporary_edits_discarded");
    if !report.metrics.stop_cleared_instance {
        report
            .diagnostics
            .push("fail:stop_did_not_clear_editor_runtime_play_instance".to_string());
    }
    if !report.metrics.runtime_temporary_edit_discarded_on_stop {
        report
            .diagnostics
            .push("fail:runtime_temporary_edit_discard_missing".to_string());
    }
    if session
        .last_game_view_present_report()
        .is_some_and(|game_view| game_view.stop_status != "stopped")
    {
        report
            .diagnostics
            .push("fail:game_view_stop_status_not_stopped".to_string());
    }
    let stopped_model = session.build_ui_model();
    report.metrics.game_view_maximize_restored_after_stop = !stopped_model
        .toolbar
        .game_view_layout
        .is_game_view_maximized;
    if !report.metrics.game_view_maximize_restored_after_stop {
        report
            .diagnostics
            .push("fail:game_view_maximize_not_restored_after_stop".to_string());
    }

    report.status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("fail:"))
    {
        report
            .next_actions
            .push("inspect_editor_gameview_play_runner_report".to_string());
        ComplexShooterEditorGameViewPlayRunnerStatus::Failed
    } else {
        ComplexShooterEditorGameViewPlayRunnerStatus::Passed
    };

    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-editor-gameview-play-runner-productization-report.json");
    report.artifacts.push(artifact_path.display().to_string());
    if let Err(error) = write_json(&artifact_path, &report) {
        report
            .diagnostics
            .push(format!("fail:editor_gameview_report_write_failed:{error}"));
        report.status = ComplexShooterEditorGameViewPlayRunnerStatus::Failed;
    }

    report
}

pub fn run_complex_shooter_editor_gameview_gpu_texture_present_report(
    request: ComplexShooterEditorGameViewPlayRunnerRequest,
) -> ComplexShooterEditorGameViewGpuTexturePresentReport {
    let mut report = ComplexShooterEditorGameViewGpuTexturePresentReport {
        schema_version: COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_REPORT_SCHEMA_VERSION
            .to_string(),
        scenario_id: COMPLEX_SHOOTER_EDITOR_GAMEVIEW_GPU_TEXTURE_PRESENT_SCENARIO_ID.to_string(),
        status: ComplexShooterEditorGameViewGpuTexturePresentStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        metrics: ComplexShooterEditorGameViewGpuTexturePresentMetrics::default(),
        game_view_present_report_path: None,
        last_frame_hash: None,
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };

    let mut session = crate::complex_shooter_editor_session();
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: request.project_root.display().to_string(),
    }));
    if open.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:sample_project_open_failed".to_string());
    }

    let play = session.execute_command(command_for_test(UiCommandPayload::Play));
    if play.status != CommandStatus::Committed {
        report
            .diagnostics
            .push("fail:editor_gameview_play_command_failed".to_string());
    }

    report.metrics.preview_package_report_present =
        session.last_editor_preview_package_report().is_some();
    report.metrics.play_session_report_present = session.last_play_session_report().is_some();

    if let Some(game_view) = session.last_game_view_present_report() {
        report.metrics.game_view_present_report_present = true;
        report.metrics.frame_count = game_view.frame_count;
        report.metrics.texture_descriptor_status = game_view.texture_descriptor_status.clone();
        report.metrics.gpu_present_status = game_view.gpu_present_status.clone();
        report.metrics.shared_gpu_context_status = game_view.shared_gpu_context_status.clone();
        report.metrics.descriptor_only_not_presented = game_view.texture_descriptor_status
            == "descriptor_only"
            && game_view.gpu_present_status != "presented";
        report.game_view_present_report_path = game_view.report_path.clone();
        report.last_frame_hash = game_view.last_frame_hash.clone();
        if game_view.frame_count == 0 {
            report
                .diagnostics
                .push("fail:game_view_frame_count_zero".to_string());
        }
        if game_view.texture_descriptor_status == "descriptor_only"
            && game_view.gpu_present_status == "presented"
        {
            report
                .diagnostics
                .push("fail:descriptor_only_claimed_as_presented".to_string());
        }
        if !matches!(
            game_view.gpu_present_status.as_str(),
            "presented" | "fallback_placeholder" | "gpu_unavailable" | "failed"
        ) {
            report
                .diagnostics
                .push("fail:unknown_gpu_present_status".to_string());
        }
        if let Some(path) = &game_view.report_path {
            report.artifacts.push(path.clone());
        }
    } else {
        report
            .diagnostics
            .push("fail:game_view_present_report_missing".to_string());
    }

    if let Some(frame) = session.last_game_view_runtime_frame() {
        report.metrics.viewport_descriptor_present =
            !frame.texture_id.is_empty() && !frame.target_id.is_empty();
        report.metrics.has_frame_hash = !frame.frame_hash.is_empty();
        report.metrics.rhi_command_count = frame.rhi_command_count;
        report.metrics.render_graph_pass_count = frame.render_graph_pass_count;
        if frame.rhi_command_count == 0 {
            report
                .diagnostics
                .push("fail:rhi_command_count_zero".to_string());
        }
        if frame.render_graph_pass_count == 0 {
            report
                .diagnostics
                .push("fail:render_graph_pass_count_zero".to_string());
        }
    } else {
        report
            .diagnostics
            .push("fail:game_view_runtime_frame_missing".to_string());
    }

    if !report.metrics.preview_package_report_present {
        report
            .diagnostics
            .push("fail:preview_package_report_missing".to_string());
    }
    if !report.metrics.play_session_report_present {
        report
            .diagnostics
            .push("fail:play_session_report_missing".to_string());
    }
    if !report.metrics.viewport_descriptor_present {
        report
            .diagnostics
            .push("fail:viewport_descriptor_missing".to_string());
    }

    report.status = if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("fail:"))
    {
        report
            .next_actions
            .push("inspect_editor_gameview_gpu_texture_present_report".to_string());
        ComplexShooterEditorGameViewGpuTexturePresentStatus::Failed
    } else {
        ComplexShooterEditorGameViewGpuTexturePresentStatus::Passed
    };

    let artifact_path = request
        .output_root
        .join("reports")
        .join("complex-shooter-editor-gameview-gpu-texture-present-productization-report.json");
    report.artifacts.push(artifact_path.display().to_string());
    if let Err(error) = write_json(&artifact_path, &report) {
        report.diagnostics.push(format!(
            "fail:editor_gameview_gpu_texture_report_write_failed:{error}"
        ));
        report.status = ComplexShooterEditorGameViewGpuTexturePresentStatus::Failed;
    }

    report
}

fn complex_shooter_gameplay_pointer_frame() -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(220, "game-view");
    frame.pointer_position = Some(PointerPosition { x: 700.0, y: 520.0 });
    frame.events.push(RuntimeInputEvent::PointerDown {
        x: 700.0,
        y: 520.0,
        button: RuntimePointerButton::Primary,
    });
    frame
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
