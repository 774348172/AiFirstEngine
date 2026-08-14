use editor_ui_model::Animator2DPlayObservationModel;
use engine_runtime::archetype::ComponentValue;
use engine_runtime::aui::{
    AuiActionEvent, AuiComputedRect, AuiInteractionConfig, AuiInteractionState,
    AuiInteractionSystem, AuiRuntimePresentOutput, AuiRuntimePresentStatus, AuiRuntimePresenter,
    ProjectUiStateProducerContext, ProjectUiStateSnapshotProducer,
};
use engine_runtime::aui_control_feedback::{
    presentation_delta_us_from_seconds, AuiControlFeedbackState,
};
use engine_runtime::components::ComponentTypeId;
use engine_runtime::diagnostics::{
    DiagnosticSeverity as RuntimeDiagnosticSeverity, RuntimeDiagnostic,
};
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::game_view_presentation::{
    GameViewPresentationModule, GameViewPresentationSpec, GameViewRect, GameViewScalePolicy,
    GameViewTargetSpec,
};
use engine_runtime::input_action::InputTraceSummary;
use engine_runtime::input_mapping::{
    InputMappingAsset, InputResolver, RuntimeInputEvent, RuntimeInputFrame,
};
use engine_runtime::math::Vec3;
use engine_runtime::project_observation::ProjectRuntimeObservationState;
use engine_runtime::project_runtime_module::{
    LinkedProjectRuntimeSet, ProjectRuntimeBindReceipt, ProjectRuntimeBootstrap,
};
use engine_runtime::project_runtime_session::{
    ProjectRuntimeSessionFrameReport, ProjectRuntimeSessionReportLevel,
};
use engine_runtime::rhi_command_plan::RhiCommandPlan;
use engine_runtime::runtime_package::{load_runtime_package, RuntimePackage};
use engine_runtime::runtime_renderer::RenderTarget;
use engine_runtime::runtime_scene_hydration::{
    hydrate_active_scene_into_world, RuntimeSceneHydrationReport,
};
use engine_runtime::runtime_texture::{RuntimeTextureBindingContext, RuntimeTextureUploadRegistry};
use engine_runtime::world::World;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{
    PlayRunner, PlaySessionDiagnostic, PlaySessionDiagnosticSeverity, PlaySessionMode,
    PlaySessionReport, PlaySessionRequest,
};

pub const EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION: &str = "editor-runtime-play-request.v1";
pub const GAME_VIEW_RUNTIME_FRAME_SCHEMA_VERSION: &str = "game-view-runtime-frame.v1";
pub const GAME_VIEW_PRESENT_REPORT_SCHEMA_VERSION: &str = "editor-gameview-present-report.v1";
pub const APPLY_RUNTIME_CHANGE_REPORT_SCHEMA_VERSION: &str = "apply-runtime-change-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorRuntimePlayRequest {
    pub schema_version: String,
    pub session_id: String,
    pub project_root: PathBuf,
    pub runtime_package_path: PathBuf,
    pub scene_ref: Option<String>,
    pub run_profile: Option<String>,
    pub frame_limit: u64,
    pub requested_by: String,
    pub preview_package_report_path: Option<String>,
}

impl EditorRuntimePlayRequest {
    pub fn from_play_session_request(request: &PlaySessionRequest) -> Self {
        Self {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: request.session_id.clone(),
            project_root: request.project_root.clone(),
            runtime_package_path: request.runtime_package_path.clone(),
            scene_ref: request.scene_ref.clone(),
            run_profile: request.run_profile.clone(),
            frame_limit: request.frame_limit,
            requested_by: format!("{:?}", request.requested_by),
            preview_package_report_path: request.preview_package_report_path.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorRuntimePlayState {
    Loading,
    Running,
    Paused,
    Stepping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameViewPresentStatus {
    Success,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameViewPresentDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewPresentDiagnostic {
    pub severity: GameViewPresentDiagnosticSeverity,
    pub code: String,
    pub layer: String,
    pub message: String,
    pub path: Option<String>,
}

impl GameViewPresentDiagnostic {
    pub fn info(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: GameViewPresentDiagnosticSeverity::Info,
            code: code.into(),
            layer: layer.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn warning(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: GameViewPresentDiagnosticSeverity::Warning,
            code: code.into(),
            layer: layer.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn error(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: GameViewPresentDiagnosticSeverity::Error,
            code: code.into(),
            layer: layer.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

impl From<&GameViewPresentDiagnostic> for PlaySessionDiagnostic {
    fn from(diagnostic: &GameViewPresentDiagnostic) -> Self {
        Self {
            severity: match diagnostic.severity {
                GameViewPresentDiagnosticSeverity::Info => PlaySessionDiagnosticSeverity::Info,
                GameViewPresentDiagnosticSeverity::Warning => {
                    PlaySessionDiagnosticSeverity::Warning
                }
                GameViewPresentDiagnosticSeverity::Error => PlaySessionDiagnosticSeverity::Error,
            },
            code: diagnostic.code.clone(),
            layer: diagnostic.layer.clone(),
            message: diagnostic.message.clone(),
            path: diagnostic.path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewRuntimeFrame {
    pub schema_version: String,
    pub session_id: String,
    pub scene_id: String,
    pub frame_index: u64,
    pub frame_hash: String,
    pub target_id: String,
    pub texture_id: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub aui_presentation_identity: String,
    pub presentation_scale_policy: GameViewScalePolicy,
    pub renderable_count: usize,
    pub ui_draw_item_count: usize,
    pub aui_present_status: String,
    pub input_bridge_status: String,
    pub runtime_input_event_count: usize,
    pub filtered_runtime_input_event_count: usize,
    pub aui_consumed_event_count: usize,
    pub aui_feedback_override_count: usize,
    pub aui_feedback_profile_ids: Vec<String>,
    pub gameplay_action_count: usize,
    pub gameplay_action_ids: Vec<String>,
    pub texture_descriptor_status: String,
    pub gpu_present_status: String,
    pub rhi_command_count: usize,
    pub render_graph_pass_count: usize,
    pub runtime_target_kind: String,
    #[serde(default)]
    pub animator2d_play_observations: Vec<Animator2DPlayObservationModel>,
    pub diagnostics: Vec<String>,
}

pub fn stable_game_view_surface_id(session_id: &str, target_id: &str) -> String {
    format!("gameview-surface::{session_id}::{target_id}")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewAuiActionTarget {
    pub canvas_id: String,
    pub node_id: String,
    pub action_id: String,
    pub visible: bool,
    pub interactable: bool,
    pub computed_rect: AuiComputedRect,
    pub effective_clip_rect: Option<AuiComputedRect>,
    pub reference_width: u32,
    pub reference_height: u32,
}

impl GameViewAuiActionTarget {
    pub fn actionable_rect(&self) -> Option<AuiComputedRect> {
        if !self.visible || !self.interactable {
            return None;
        }
        self.effective_clip_rect
            .map_or(Some(self.computed_rect), |clip| {
                self.computed_rect.intersection(clip)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewPresentReport {
    pub schema_version: String,
    pub session_id: String,
    pub status: GameViewPresentStatus,
    pub runtime_package_path: String,
    pub preview_package_report_path: Option<String>,
    pub scene_id: Option<String>,
    pub frame_count: u64,
    pub last_frame_hash: Option<String>,
    pub game_view_output_kind: String,
    pub texture_descriptor_status: String,
    pub gpu_present_status: String,
    pub gpu_present_report_path: Option<String>,
    pub shared_gpu_context_status: String,
    pub input_bridge_status: String,
    pub runtime_input_event_count: usize,
    pub filtered_runtime_input_event_count: usize,
    pub aui_consumed_event_count: usize,
    pub gameplay_action_count: usize,
    pub gameplay_action_ids: Vec<String>,
    pub aui_present_status: String,
    pub stop_status: String,
    pub control_state: EditorRuntimePlayState,
    pub control_command: String,
    pub runtime_advanced: bool,
    pub paused_last_frame_reused: bool,
    pub step_count: u64,
    pub target_runtime_domain: String,
    pub report_path: Option<String>,
    pub diagnostics: Vec<GameViewPresentDiagnostic>,
    pub next_actions: Vec<String>,
    pub deferred_flags: Vec<String>,
    pub last_frame: Option<GameViewRuntimeFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_bind_receipt: Option<ProjectRuntimeBindReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_observation_state: Option<ProjectRuntimeObservationState>,
}

impl GameViewPresentReport {
    fn base(request: &EditorRuntimePlayRequest) -> Self {
        Self {
            schema_version: GAME_VIEW_PRESENT_REPORT_SCHEMA_VERSION.to_string(),
            session_id: request.session_id.clone(),
            status: GameViewPresentStatus::Failed,
            runtime_package_path: request.runtime_package_path.display().to_string(),
            preview_package_report_path: request.preview_package_report_path.clone(),
            scene_id: request.scene_ref.clone(),
            frame_count: 0,
            last_frame_hash: None,
            game_view_output_kind: "viewport_texture_descriptor".to_string(),
            texture_descriptor_status: "not_started".to_string(),
            gpu_present_status: "gpu_unavailable".to_string(),
            gpu_present_report_path: None,
            shared_gpu_context_status: "not_connected".to_string(),
            input_bridge_status: "not_requested".to_string(),
            runtime_input_event_count: 0,
            filtered_runtime_input_event_count: 0,
            aui_consumed_event_count: 0,
            gameplay_action_count: 0,
            gameplay_action_ids: Vec::new(),
            aui_present_status: "not_started".to_string(),
            stop_status: "running".to_string(),
            control_state: EditorRuntimePlayState::Loading,
            control_command: "start".to_string(),
            runtime_advanced: false,
            paused_last_frame_reused: false,
            step_count: 0,
            target_runtime_domain: "active_gameview_runtime".to_string(),
            report_path: None,
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
            deferred_flags: deferred_flags(),
            last_frame: None,
            project_runtime_bind_receipt: None,
            project_runtime_session_report: None,
            project_observation_state: None,
        }
    }

    fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == GameViewPresentDiagnosticSeverity::Error)
    }

    fn recompute_status(&mut self) {
        if self.status == GameViewPresentStatus::Stopped {
            return;
        }
        self.status = if self.frame_count > 0 && !self.has_error_diagnostics() {
            GameViewPresentStatus::Success
        } else {
            GameViewPresentStatus::Failed
        };
        if self.status == GameViewPresentStatus::Failed && self.next_actions.is_empty() {
            self.next_actions
                .push("inspect_game_view_present_report".to_string());
        }
    }
}

pub struct EditorRuntimePlayInstance {
    session_id: String,
    runtime_package_path: PathBuf,
    preview_package_report_path: Option<String>,
    scene_id: String,
    package: RuntimePackage,
    world: World,
    host: EngineHostLoop,
    game_view_target: GameViewTargetSpec,
    ui_state_producer: Box<dyn ProjectUiStateSnapshotProducer>,
    input_mapping: InputMappingAsset,
    project_runtime_bind_receipt: ProjectRuntimeBindReceipt,
    project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    project_observation_state: Option<ProjectRuntimeObservationState>,
    aui_interaction_state: AuiInteractionState,
    aui_feedback_state: AuiControlFeedbackState,
    last_aui_present: Option<AuiRuntimePresentOutput>,
    pending_gameplay_input: Option<RuntimeInputFrame>,
    state: EditorRuntimePlayState,
    frame_count: u64,
    last_frame: Option<GameViewRuntimeFrame>,
    last_aui_action_targets: Vec<GameViewAuiActionTarget>,
    last_rhi_command_plan: Option<RhiCommandPlan>,
    runtime_texture_uploads: RuntimeTextureUploadRegistry,
    runtime_texture_bindings: RuntimeTextureBindingContext,
    report_path: Option<String>,
    hydration_report: RuntimeSceneHydrationReport,
    runtime_authoring_origins: BTreeMap<String, RuntimeAuthoringOrigin>,
    temporary_edit_records: Vec<RuntimeTemporaryEditRecord>,
    temporary_edit_sequence: u64,
    temporary_edit_summary: RuntimeTemporaryEditSummary,
}

impl std::fmt::Debug for EditorRuntimePlayInstance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EditorRuntimePlayInstance")
            .field("session_id", &self.session_id)
            .field("runtime_package_path", &self.runtime_package_path)
            .field("scene_id", &self.scene_id)
            .field("state", &self.state)
            .field("frame_count", &self.frame_count)
            .field(
                "project_runtime_bind_receipt",
                &self.project_runtime_bind_receipt,
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAuthoringOriginKind {
    SceneEntity,
    PrefabInstanceEntity,
    RuntimeSpawned,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuthoringOrigin {
    pub kind: RuntimeAuthoringOriginKind,
    pub scene_id: Option<String>,
    pub entity_id: Option<String>,
    pub runtime_entity_id: Option<String>,
    pub prefab_asset_guid: Option<String>,
    pub prefab_instance_entity_id: Option<String>,
    pub diagnostic: Option<String>,
}

impl RuntimeAuthoringOrigin {
    fn scene_entity(scene_id: &str, entity_id: &str, runtime_entity_id: &str) -> Self {
        Self {
            kind: RuntimeAuthoringOriginKind::SceneEntity,
            scene_id: Some(scene_id.to_string()),
            entity_id: Some(entity_id.to_string()),
            runtime_entity_id: Some(runtime_entity_id.to_string()),
            prefab_asset_guid: None,
            prefab_instance_entity_id: None,
            diagnostic: None,
        }
    }

    fn blocked(
        kind: RuntimeAuthoringOriginKind,
        entity_id: &str,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            scene_id: None,
            entity_id: Some(entity_id.to_string()),
            runtime_entity_id: None,
            prefab_asset_guid: None,
            prefab_instance_entity_id: None,
            diagnostic: Some(diagnostic.into()),
        }
    }

    pub fn is_ready_scene_entity(&self) -> bool {
        self.kind == RuntimeAuthoringOriginKind::SceneEntity
            && self.scene_id.is_some()
            && self.entity_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTemporaryEditSummary {
    pub edited_entity_ids: Vec<String>,
    pub edited_field_paths: Vec<String>,
    pub edited_field_count: usize,
    pub last_edited_entity_id: Option<String>,
    pub last_edited_field_path: Option<String>,
    pub discard_policy: String,
}

impl Default for RuntimeTemporaryEditSummary {
    fn default() -> Self {
        Self {
            edited_entity_ids: Vec::new(),
            edited_field_paths: Vec::new(),
            edited_field_count: 0,
            last_edited_entity_id: None,
            last_edited_field_path: None,
            discard_policy: "discard_on_stop_play".to_string(),
        }
    }
}

impl RuntimeTemporaryEditSummary {
    pub fn edited_entity_count(&self) -> usize {
        self.edited_entity_ids.len()
    }

    pub fn record(&mut self, entity_id: &str, component_type: &str, field_path: &str) {
        insert_sorted_unique(&mut self.edited_entity_ids, entity_id.to_string());
        let full_path = format!("{component_type}.{field_path}");
        insert_sorted_unique(&mut self.edited_field_paths, full_path.clone());
        self.edited_field_count += 1;
        self.last_edited_entity_id = Some(entity_id.to_string());
        self.last_edited_field_path = Some(full_path);
    }

    fn from_pending_records(records: &[RuntimeTemporaryEditRecord]) -> Self {
        let mut summary = Self::default();
        for record in records.iter().filter(|record| !record.applied) {
            insert_sorted_unique(&mut summary.edited_entity_ids, record.entity_id.clone());
            insert_sorted_unique(
                &mut summary.edited_field_paths,
                format!("{}.{}", record.component_type, record.field_path),
            );
            summary.edited_field_count += 1;
            summary.last_edited_entity_id = Some(record.entity_id.clone());
            summary.last_edited_field_path =
                Some(format!("{}.{}", record.component_type, record.field_path));
        }
        summary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTemporaryEditApplyPolicy {
    ApplyToAuthoringScene,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTemporaryEditRecord {
    pub edit_id: String,
    pub sequence: u64,
    pub entity_id: String,
    pub runtime_entity_id: Option<String>,
    pub component_type: String,
    pub field_path: String,
    pub value_after: serde_json::Value,
    pub authoring_origin: RuntimeAuthoringOrigin,
    pub apply_policy: RuntimeTemporaryEditApplyPolicy,
    pub applied: bool,
    pub before_summary: Option<String>,
    pub after_summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyRuntimeChangeCandidateStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRuntimeChangeCandidate {
    pub edit_id: String,
    pub runtime_entity_id: String,
    pub runtime_slot_id: Option<String>,
    pub component_type: String,
    pub field_path: String,
    pub runtime_value: serde_json::Value,
    pub target_authoring_scene_id: Option<String>,
    pub target_authoring_entity_id: Option<String>,
    pub target_authoring_path: Option<String>,
    pub authoring_origin: RuntimeAuthoringOrigin,
    pub candidate_hash: String,
    pub status: ApplyRuntimeChangeCandidateStatus,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRuntimeChangeReport {
    pub schema_version: String,
    pub status: String,
    pub scene_id: String,
    pub command: String,
    pub candidate_count: usize,
    pub ready_count: usize,
    pub blocked_count: usize,
    pub applied_edit_id: Option<String>,
    pub diagnostics: Vec<String>,
    pub candidates: Vec<ApplyRuntimeChangeCandidate>,
}

impl ApplyRuntimeChangeReport {
    pub fn from_candidates(
        scene_id: &str,
        command: &str,
        candidates: Vec<ApplyRuntimeChangeCandidate>,
        diagnostics: Vec<String>,
        applied_edit_id: Option<String>,
    ) -> Self {
        apply_report_for_candidates(scene_id, command, candidates, diagnostics, applied_edit_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTemporaryEditError {
    pub code: String,
    pub message: String,
}

impl RuntimeTemporaryEditError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

fn insert_sorted_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
        values.sort();
    }
}

fn normalize_component_type(component_type: &str) -> String {
    match component_type
        .trim()
        .replace('-', "_")
        .replace(' ', "_")
        .to_ascii_lowercase()
        .as_str()
    {
        "sprite_renderer_2d" | "spriterenderer2d" | "sprite_renderer2d" => {
            "sprite_renderer2d".to_string()
        }
        "collider_2d" | "collider2d" => "collider2d".to_string(),
        "transform" => "transform".to_string(),
        "renderable" => "renderable".to_string(),
        other => other.to_string(),
    }
}

fn normalize_runtime_field_path(component_type: &str, field_path: &str) -> String {
    let mut path = field_path.trim().replace('\\', "/").replace('/', ".");
    for prefix in [
        "transform.",
        "renderable.",
        "spriteRenderer2D.",
        "sprite_renderer2d.",
        "collider2D.",
        "collider2d.",
    ] {
        if let Some(rest) = path.strip_prefix(prefix) {
            path = rest.to_string();
            break;
        }
    }
    path = path
        .replace("localPosition", "local_position")
        .replace("localRotation", "local_rotation")
        .replace("localScale", "local_scale")
        .replace("sortingLayer", "sorting_layer")
        .replace("orderInLayer", "order_in_layer")
        .replace("sortZ", "sort_z");
    if component_type == "sprite_renderer2d" || component_type == "collider2d" {
        path = path.to_ascii_lowercase();
    }
    path
}

fn apply_transform_temporary_edit(
    transform: &mut engine_runtime::components::Transform,
    field_path: &str,
    value: serde_json::Value,
) -> Result<(), RuntimeTemporaryEditError> {
    match field_path {
        "local_position" => transform.local_position = json_vec3(value)?,
        "local_rotation" => transform.local_rotation = json_vec3(value)?,
        "local_scale" => transform.local_scale = json_vec3(value)?,
        "local_position.x" => transform.local_position.x = json_f64(value)? as f32,
        "local_position.y" => transform.local_position.y = json_f64(value)? as f32,
        "local_position.z" => transform.local_position.z = json_f64(value)? as f32,
        "local_rotation.x" => transform.local_rotation.x = json_f64(value)? as f32,
        "local_rotation.y" => transform.local_rotation.y = json_f64(value)? as f32,
        "local_rotation.z" => transform.local_rotation.z = json_f64(value)? as f32,
        "local_scale.x" => transform.local_scale.x = json_f64(value)? as f32,
        "local_scale.y" => transform.local_scale.y = json_f64(value)? as f32,
        "local_scale.z" => transform.local_scale.z = json_f64(value)? as f32,
        _ => return Err(field_not_runtime_temporary_editable(field_path)),
    }
    Ok(())
}

fn json_bool(value: serde_json::Value) -> Result<bool, RuntimeTemporaryEditError> {
    value.as_bool().ok_or_else(|| {
        RuntimeTemporaryEditError::new(
            "unsupported_value_type",
            "Expected a boolean value for this runtime temporary edit.",
        )
    })
}

fn json_i64(value: serde_json::Value) -> Result<i64, RuntimeTemporaryEditError> {
    value.as_i64().ok_or_else(|| {
        RuntimeTemporaryEditError::new(
            "unsupported_value_type",
            "Expected an integer value for this runtime temporary edit.",
        )
    })
}

fn json_f64(value: serde_json::Value) -> Result<f64, RuntimeTemporaryEditError> {
    value.as_f64().ok_or_else(|| {
        RuntimeTemporaryEditError::new(
            "unsupported_value_type",
            "Expected a number value for this runtime temporary edit.",
        )
    })
}

fn json_vec3(value: serde_json::Value) -> Result<Vec3, RuntimeTemporaryEditError> {
    if let Some(array) = value.as_array() {
        if array.len() == 3 {
            return Ok(Vec3 {
                x: array[0].as_f64().ok_or_else(unsupported_vec3)? as f32,
                y: array[1].as_f64().ok_or_else(unsupported_vec3)? as f32,
                z: array[2].as_f64().ok_or_else(unsupported_vec3)? as f32,
            });
        }
    }
    let object = value.as_object().ok_or_else(unsupported_vec3)?;
    Ok(Vec3 {
        x: object
            .get("x")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(unsupported_vec3)? as f32,
        y: object
            .get("y")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(unsupported_vec3)? as f32,
        z: object
            .get("z")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(unsupported_vec3)? as f32,
    })
}

fn json_color(value: serde_json::Value) -> Result<[f32; 4], RuntimeTemporaryEditError> {
    if let Some(array) = value.as_array() {
        if array.len() == 4 {
            return Ok([
                array[0].as_f64().ok_or_else(unsupported_color)? as f32,
                array[1].as_f64().ok_or_else(unsupported_color)? as f32,
                array[2].as_f64().ok_or_else(unsupported_color)? as f32,
                array[3].as_f64().ok_or_else(unsupported_color)? as f32,
            ]);
        }
    }
    let object = value.as_object().ok_or_else(unsupported_color)?;
    Ok([
        object
            .get("r")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(unsupported_color)? as f32,
        object
            .get("g")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(unsupported_color)? as f32,
        object
            .get("b")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(unsupported_color)? as f32,
        object
            .get("a")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(unsupported_color)? as f32,
    ])
}

fn unsupported_vec3() -> RuntimeTemporaryEditError {
    RuntimeTemporaryEditError::new(
        "unsupported_value_type",
        "Expected a Vec3 object or [x, y, z] array for this runtime temporary edit.",
    )
}

fn unsupported_color() -> RuntimeTemporaryEditError {
    RuntimeTemporaryEditError::new(
        "unsupported_value_type",
        "Expected a color object or [r, g, b, a] array for this runtime temporary edit.",
    )
}

fn field_not_runtime_temporary_editable(field_path: &str) -> RuntimeTemporaryEditError {
    RuntimeTemporaryEditError::new(
        "field_not_runtime_temporary_editable",
        format!("Field path {field_path} is not in the runtime temporary edit allowlist."),
    )
}

fn build_runtime_authoring_origin_index(
    report: &RuntimeSceneHydrationReport,
) -> BTreeMap<String, RuntimeAuthoringOrigin> {
    let mut origins = BTreeMap::new();
    if let Some(instance) = &report.instance {
        for (source_id, runtime_id) in &instance.source_to_runtime_entity {
            origins.insert(
                runtime_id.to_string(),
                RuntimeAuthoringOrigin::scene_entity(
                    &report.scene_id,
                    source_id.as_str(),
                    &runtime_id.to_string(),
                ),
            );
        }
    }
    origins
}

fn runtime_field_value(
    world: &World,
    entity_id: &engine_runtime::ids::EntityId,
    component_type: &str,
    field_path: &str,
) -> Result<serde_json::Value, RuntimeTemporaryEditError> {
    match component_type {
        "transform" => {
            let transform = world.transform(entity_id).ok_or_else(|| {
                RuntimeTemporaryEditError::new(
                    "component_missing",
                    "Transform component is missing on the runtime entity.",
                )
            })?;
            let value = match field_path {
                "local_position" => vec3_json(transform.local_position),
                "local_rotation" => vec3_json(transform.local_rotation),
                "local_scale" => vec3_json(transform.local_scale),
                "local_position.x" => serde_json::json!(transform.local_position.x),
                "local_position.y" => serde_json::json!(transform.local_position.y),
                "local_position.z" => serde_json::json!(transform.local_position.z),
                "local_rotation.x" => serde_json::json!(transform.local_rotation.x),
                "local_rotation.y" => serde_json::json!(transform.local_rotation.y),
                "local_rotation.z" => serde_json::json!(transform.local_rotation.z),
                "local_scale.x" => serde_json::json!(transform.local_scale.x),
                "local_scale.y" => serde_json::json!(transform.local_scale.y),
                "local_scale.z" => serde_json::json!(transform.local_scale.z),
                _ => return Err(field_not_runtime_temporary_editable(field_path)),
            };
            Ok(value)
        }
        "renderable" => {
            let renderable = world.renderable(entity_id).ok_or_else(|| {
                RuntimeTemporaryEditError::new(
                    "component_missing",
                    "Renderable component is missing on the runtime entity.",
                )
            })?;
            match field_path {
                "visible" => Ok(serde_json::json!(renderable.visible)),
                _ => Err(field_not_runtime_temporary_editable(field_path)),
            }
        }
        "sprite_renderer2d" => {
            let sprite = world.sprite_renderer2d(entity_id).ok_or_else(|| {
                RuntimeTemporaryEditError::new(
                    "component_missing",
                    "SpriteRenderer2D component is missing on the runtime entity.",
                )
            })?;
            match field_path {
                "visible" => Ok(serde_json::json!(sprite.visible)),
                "color" => Ok(serde_json::json!(sprite.color)),
                "sorting_layer" => Ok(serde_json::json!(sprite.sorting_layer)),
                "order_in_layer" => Ok(serde_json::json!(sprite.order_in_layer)),
                "sort_z" => Ok(serde_json::json!(sprite.sort_z)),
                _ => Err(field_not_runtime_temporary_editable(field_path)),
            }
        }
        "collider2d" => {
            let collider = world.collider2d(entity_id).ok_or_else(|| {
                RuntimeTemporaryEditError::new(
                    "component_missing",
                    "Collider2D component is missing on the runtime entity.",
                )
            })?;
            match field_path {
                "enabled" => Ok(serde_json::json!(collider.enabled)),
                _ => Err(field_not_runtime_temporary_editable(field_path)),
            }
        }
        _ => Err(RuntimeTemporaryEditError::new(
            "field_not_runtime_temporary_editable",
            format!(
                "Component {} is not editable through Play Mode temporary inspector edits.",
                component_type
            ),
        )),
    }
}

fn vec3_json(value: Vec3) -> serde_json::Value {
    serde_json::json!({
        "x": value.x,
        "y": value.y,
        "z": value.z,
    })
}

fn candidate_hash_input(
    runtime_entity_id: &str,
    component_type: &str,
    field_path: &str,
    runtime_value: &serde_json::Value,
    target_authoring_path: Option<&str>,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        runtime_entity_id,
        component_type,
        field_path,
        serde_json::to_string(runtime_value).unwrap_or_else(|_| "null".to_string()),
        target_authoring_path.unwrap_or("blocked")
    )
}

fn candidate_hash_for_input(input: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn scene_component_type_for_runtime(component_type: &str) -> &'static str {
    match component_type {
        "transform" => "Transform",
        "renderable" => "Renderable",
        "sprite_renderer2d" => "SpriteRenderer2D",
        "collider2d" => "engine.collider2d",
        _ => "Dynamic",
    }
}

pub fn scene_field_path_for_runtime(field_path: &str) -> String {
    field_path
        .replace("local_position", "localPosition")
        .replace("local_rotation", "localRotation")
        .replace("local_scale", "localScale")
        .replace("sorting_layer", "sortingLayer")
        .replace("order_in_layer", "orderInLayer")
        .replace("sort_z", "sortZ")
}

fn apply_report_for_candidates(
    scene_id: &str,
    command: &str,
    candidates: Vec<ApplyRuntimeChangeCandidate>,
    diagnostics: Vec<String>,
    applied_edit_id: Option<String>,
) -> ApplyRuntimeChangeReport {
    let ready_count = candidates
        .iter()
        .filter(|candidate| candidate.status == ApplyRuntimeChangeCandidateStatus::Ready)
        .count();
    let blocked_count = candidates
        .iter()
        .filter(|candidate| candidate.status == ApplyRuntimeChangeCandidateStatus::Blocked)
        .count();
    let status = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("error:"))
    {
        "failed"
    } else if blocked_count > 0 && ready_count == 0 {
        "blocked"
    } else {
        "success"
    };
    ApplyRuntimeChangeReport {
        schema_version: APPLY_RUNTIME_CHANGE_REPORT_SCHEMA_VERSION.to_string(),
        status: status.to_string(),
        scene_id: scene_id.to_string(),
        command: command.to_string(),
        candidate_count: candidates.len(),
        ready_count,
        blocked_count,
        applied_edit_id,
        diagnostics,
        candidates,
    }
}

#[derive(Debug)]
pub struct EditorGameViewPlayOutput {
    pub instance: Option<EditorRuntimePlayInstance>,
    pub frame: Option<GameViewRuntimeFrame>,
    pub present_report: GameViewPresentReport,
}

pub struct EditorGameViewPlayRunner {
    last_output: RefCell<Option<EditorGameViewPlayOutput>>,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
}

impl EditorGameViewPlayRunner {
    pub fn new() -> Self {
        Self {
            last_output: RefCell::new(None),
            linked_modules: Arc::new(LinkedProjectRuntimeSet::explicit_empty()),
        }
    }

    pub fn with_linked_modules(linked_modules: Arc<LinkedProjectRuntimeSet>) -> Self {
        Self {
            last_output: RefCell::new(None),
            linked_modules,
        }
    }

    pub fn take_last_output(&self) -> Option<EditorGameViewPlayOutput> {
        self.last_output.borrow_mut().take()
    }
}

impl Default for EditorGameViewPlayRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EditorGameViewPlayRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EditorGameViewPlayRunner")
            .field("linked_module_count", &self.linked_modules.len())
            .finish_non_exhaustive()
    }
}

impl PlayRunner for EditorGameViewPlayRunner {
    fn run_play_session(&self, request: PlaySessionRequest) -> PlaySessionReport {
        if request.mode != PlaySessionMode::WindowedUserRun {
            return PlaySessionReport::failed_before_runtime(
                &request,
                "editor_gameview_runner_requires_windowed_mode",
                "request",
                "EditorGameViewPlayRunner only handles WindowedUserRun / EditorGameView sessions.",
            );
        }

        let target = request.game_view_target;
        let runtime_request = EditorRuntimePlayRequest::from_play_session_request(&request);
        let output = EditorRuntimePlayInstance::start_with_linked_modules_and_target(
            runtime_request,
            &self.linked_modules,
            target,
        );
        let success = output.present_report.status == GameViewPresentStatus::Success;
        let play_report = PlaySessionReport::from_game_view_present_report(
            &request,
            success,
            output.present_report.report_path.clone(),
            output.present_report.frame_count,
            output.present_report.last_frame_hash.clone(),
            output
                .present_report
                .diagnostics
                .iter()
                .map(PlaySessionDiagnostic::from)
                .collect(),
        );
        *self.last_output.borrow_mut() = Some(output);
        play_report
    }
}

impl EditorRuntimePlayInstance {
    pub fn runtime_world(&self) -> &World {
        &self.world
    }

    pub fn scene_id(&self) -> &str {
        &self.scene_id
    }

    pub fn temporary_edit_summary(&self) -> &RuntimeTemporaryEditSummary {
        &self.temporary_edit_summary
    }

    pub fn hydration_report(&self) -> &RuntimeSceneHydrationReport {
        &self.hydration_report
    }

    pub fn temporary_edit_records(&self) -> &[RuntimeTemporaryEditRecord] {
        &self.temporary_edit_records
    }

    pub fn preview_apply_runtime_change_to_authoring(
        &self,
        authoring_scene_id: Option<&str>,
    ) -> ApplyRuntimeChangeReport {
        let mut candidates = Vec::new();
        let mut diagnostics = Vec::new();
        for record in self
            .temporary_edit_records
            .iter()
            .filter(|record| !record.applied)
        {
            candidates.push(self.candidate_for_record(record, authoring_scene_id));
        }
        if candidates.is_empty() {
            diagnostics.push("info:no_pending_runtime_temporary_edits".to_string());
        }
        apply_report_for_candidates(
            &self.scene_id,
            "preview_apply_runtime_change_to_authoring",
            candidates,
            diagnostics,
            None,
        )
    }

    pub fn confirm_apply_runtime_change_candidate(
        &self,
        edit_id: &str,
        candidate_hash: &str,
        authoring_scene_id: Option<&str>,
    ) -> Result<ApplyRuntimeChangeCandidate, ApplyRuntimeChangeReport> {
        let Some(record) = self
            .temporary_edit_records
            .iter()
            .find(|record| record.edit_id == edit_id && !record.applied)
        else {
            return Err(apply_report_for_candidates(
                &self.scene_id,
                "apply_runtime_change_to_authoring",
                Vec::new(),
                vec![format!("error:pending_edit_not_found:{edit_id}")],
                None,
            ));
        };

        let candidate = self.candidate_for_record(record, authoring_scene_id);
        if candidate.status != ApplyRuntimeChangeCandidateStatus::Ready {
            return Err(apply_report_for_candidates(
                &self.scene_id,
                "apply_runtime_change_to_authoring",
                vec![candidate],
                vec!["error:candidate_not_ready".to_string()],
                None,
            ));
        }
        if candidate.candidate_hash != candidate_hash {
            return Err(apply_report_for_candidates(
                &self.scene_id,
                "apply_runtime_change_to_authoring",
                vec![candidate],
                vec!["error:stale_candidate_hash".to_string()],
                None,
            ));
        }
        Ok(candidate)
    }

    pub fn mark_runtime_temporary_edit_applied(&mut self, edit_id: &str) -> bool {
        let mut marked = false;
        for record in &mut self.temporary_edit_records {
            if record.edit_id == edit_id {
                record.applied = true;
                marked = true;
            }
        }
        if marked {
            self.refresh_temporary_edit_summary();
        }
        marked
    }

    fn record_temporary_edit(&mut self, record: RuntimeTemporaryEditRecord) {
        self.temporary_edit_records.retain(|existing| {
            existing.entity_id != record.entity_id
                || existing.component_type != record.component_type
                || existing.field_path != record.field_path
        });
        self.temporary_edit_records.push(record);
        self.temporary_edit_records
            .sort_by(|left, right| left.sequence.cmp(&right.sequence));
        self.refresh_temporary_edit_summary();
    }

    fn refresh_temporary_edit_summary(&mut self) {
        self.temporary_edit_summary =
            RuntimeTemporaryEditSummary::from_pending_records(&self.temporary_edit_records);
    }

    fn authoring_origin_for_entity(
        &self,
        entity_id: &str,
        runtime_slot_id: Option<&str>,
    ) -> RuntimeAuthoringOrigin {
        let Some(runtime_slot_id) = runtime_slot_id else {
            return RuntimeAuthoringOrigin::blocked(
                RuntimeAuthoringOriginKind::Unknown,
                entity_id,
                "Runtime entity has no slot id in the active World.",
            );
        };
        self.runtime_authoring_origins
            .get(runtime_slot_id)
            .cloned()
            .unwrap_or_else(|| {
                RuntimeAuthoringOrigin::blocked(
                    RuntimeAuthoringOriginKind::RuntimeSpawned,
                    entity_id,
                    "Runtime entity was not created from the hydrated authoring scene.",
                )
            })
    }

    fn candidate_for_record(
        &self,
        record: &RuntimeTemporaryEditRecord,
        authoring_scene_id: Option<&str>,
    ) -> ApplyRuntimeChangeCandidate {
        let entity_id = engine_runtime::ids::EntityId::new(record.entity_id.clone());
        let mut diagnostics = Vec::new();
        let live_value = match runtime_field_value(
            &self.world,
            &entity_id,
            &record.component_type,
            &record.field_path,
        ) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(format!("error:runtime_live_read_failed:{}", error.code));
                record.value_after.clone()
            }
        };
        let mut origin = record.authoring_origin.clone();
        if let Some(runtime_slot_id) = record.runtime_entity_id.as_deref() {
            origin = self.authoring_origin_for_entity(&record.entity_id, Some(runtime_slot_id));
        }

        let mut target_scene_id = origin.scene_id.clone();
        let target_entity_id = origin.entity_id.clone();
        let mut target_path = target_entity_id.as_ref().map(|entity_id| {
            format!(
                "scene.{}/entities/{}/{}.{}",
                target_scene_id.as_deref().unwrap_or("unknown"),
                entity_id,
                scene_component_type_for_runtime(&record.component_type),
                scene_field_path_for_runtime(&record.field_path)
            )
        });

        if let Some(authoring_scene_id) = authoring_scene_id {
            if self.scene_id != authoring_scene_id {
                diagnostics.push(format!(
                    "error:scene_id_mismatch:runtime={} authoring={}",
                    self.scene_id, authoring_scene_id
                ));
                target_scene_id = Some(authoring_scene_id.to_string());
                target_path = None;
            }
        } else {
            diagnostics.push("error:authoring_scene_not_loaded".to_string());
            target_path = None;
        }

        if !origin.is_ready_scene_entity() {
            diagnostics.push(format!("error:origin_blocked:{:?}", origin.kind));
            target_path = None;
        }

        let hash_input = candidate_hash_input(
            &record.entity_id,
            &record.component_type,
            &record.field_path,
            &live_value,
            target_path.as_deref(),
        );
        let status = if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("error:"))
        {
            ApplyRuntimeChangeCandidateStatus::Blocked
        } else {
            ApplyRuntimeChangeCandidateStatus::Ready
        };

        ApplyRuntimeChangeCandidate {
            edit_id: record.edit_id.clone(),
            runtime_entity_id: record.entity_id.clone(),
            runtime_slot_id: record.runtime_entity_id.clone(),
            component_type: record.component_type.clone(),
            field_path: record.field_path.clone(),
            runtime_value: live_value,
            target_authoring_scene_id: target_scene_id,
            target_authoring_entity_id: target_entity_id,
            target_authoring_path: target_path,
            authoring_origin: origin,
            candidate_hash: candidate_hash_for_input(&hash_input),
            status,
            diagnostics,
        }
    }

    pub fn apply_temporary_component_edit(
        &mut self,
        entity_id: &str,
        component_type: &str,
        field_path: &str,
        value: serde_json::Value,
    ) -> Result<RuntimeTemporaryEditRecord, RuntimeTemporaryEditError> {
        let entity_id = engine_runtime::ids::EntityId::new(entity_id.to_string());
        if self.world.entity(&entity_id).is_none() {
            return Err(RuntimeTemporaryEditError::new(
                "entity_missing",
                format!("Runtime entity {} does not exist.", entity_id.as_str()),
            ));
        }

        let normalized_component = normalize_component_type(component_type);
        let normalized_field = normalize_runtime_field_path(&normalized_component, field_path);
        let value_after_input = value.clone();
        let before_summary;
        let after_summary;

        match normalized_component.as_str() {
            "transform" => {
                let mut transform = self.world.transform(&entity_id).cloned().ok_or_else(|| {
                    RuntimeTemporaryEditError::new(
                        "component_missing",
                        "Transform component is missing on the runtime entity.",
                    )
                })?;
                before_summary = Some(format!("{:?}", transform));
                apply_transform_temporary_edit(&mut transform, &normalized_field, value)?;
                after_summary = format!("{:?}", transform);
                self.world
                    .try_insert_transform(entity_id.clone(), transform)
                    .map_err(runtime_temporary_world_mutation_error)?;
            }
            "renderable" => {
                let mut renderable =
                    self.world.renderable(&entity_id).cloned().ok_or_else(|| {
                        RuntimeTemporaryEditError::new(
                            "component_missing",
                            "Renderable component is missing on the runtime entity.",
                        )
                    })?;
                before_summary = Some(format!("{:?}", renderable));
                match normalized_field.as_str() {
                    "visible" => renderable.visible = json_bool(value)?,
                    _ => return Err(field_not_runtime_temporary_editable(&normalized_field)),
                }
                after_summary = format!("{:?}", renderable);
                self.world
                    .try_insert_component_value(
                        entity_id.clone(),
                        ComponentTypeId::renderable(),
                        ComponentValue::Renderable(renderable),
                    )
                    .map_err(runtime_temporary_world_mutation_error)?;
            }
            "sprite_renderer2d" => {
                let mut sprite = self
                    .world
                    .sprite_renderer2d(&entity_id)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimeTemporaryEditError::new(
                            "component_missing",
                            "SpriteRenderer2D component is missing on the runtime entity.",
                        )
                    })?;
                before_summary = Some(format!("{:?}", sprite));
                match normalized_field.as_str() {
                    "visible" => sprite.visible = json_bool(value)?,
                    "color" => sprite.color = json_color(value)?,
                    "sorting_layer" => sprite.sorting_layer = json_i64(value)? as i16,
                    "order_in_layer" => sprite.order_in_layer = json_i64(value)? as i32,
                    "sort_z" => sprite.sort_z = json_f64(value)? as f32,
                    _ => return Err(field_not_runtime_temporary_editable(&normalized_field)),
                }
                after_summary = format!("{:?}", sprite);
                self.world
                    .try_insert_component_value(
                        entity_id.clone(),
                        ComponentTypeId::sprite_renderer2d(),
                        ComponentValue::SpriteRenderer2D(sprite),
                    )
                    .map_err(runtime_temporary_world_mutation_error)?;
            }
            "collider2d" => {
                let mut component = self
                    .world
                    .component_value(&entity_id, &ComponentTypeId::collider2d())
                    .ok_or_else(|| {
                        RuntimeTemporaryEditError::new(
                            "component_missing",
                            "Collider2D component is missing on the runtime entity.",
                        )
                    })?;
                before_summary = Some(format!("{:?}", component));
                let ComponentValue::Collider2D(collider) = &mut component else {
                    return Err(RuntimeTemporaryEditError::new(
                        "component_missing",
                        "Collider2D component data is not available.",
                    ));
                };
                match normalized_field.as_str() {
                    "enabled" => collider.enabled = json_bool(value)?,
                    _ => return Err(field_not_runtime_temporary_editable(&normalized_field)),
                }
                after_summary = format!("{:?}", component);
                self.world
                    .try_insert_component_value(
                        entity_id.clone(),
                        ComponentTypeId::collider2d(),
                        component,
                    )
                    .map_err(runtime_temporary_world_mutation_error)?;
            }
            _ => {
                return Err(RuntimeTemporaryEditError::new(
                    "field_not_runtime_temporary_editable",
                    format!(
                        "Component {} is not editable through Play Mode temporary inspector edits.",
                        component_type
                    ),
                ));
            }
        }

        let runtime_entity_id = self
            .world
            .runtime_id_for_source(&entity_id)
            .map(|runtime_id| runtime_id.to_string());
        let value_after = runtime_field_value(
            &self.world,
            &entity_id,
            &normalized_component,
            &normalized_field,
        )
        .unwrap_or(value_after_input);
        let authoring_origin =
            self.authoring_origin_for_entity(entity_id.as_str(), runtime_entity_id.as_deref());
        let apply_policy = if authoring_origin.is_ready_scene_entity() {
            RuntimeTemporaryEditApplyPolicy::ApplyToAuthoringScene
        } else {
            RuntimeTemporaryEditApplyPolicy::Blocked
        };
        self.temporary_edit_sequence = self.temporary_edit_sequence.saturating_add(1);
        let record = RuntimeTemporaryEditRecord {
            edit_id: format!("runtime-edit-{}", self.temporary_edit_sequence),
            sequence: self.temporary_edit_sequence,
            entity_id: entity_id.as_str().to_string(),
            runtime_entity_id,
            component_type: normalized_component,
            field_path: normalized_field,
            value_after,
            authoring_origin,
            apply_policy,
            applied: false,
            before_summary,
            after_summary,
        };
        self.record_temporary_edit(record.clone());
        Ok(record)
    }

    pub fn start(request: EditorRuntimePlayRequest) -> EditorGameViewPlayOutput {
        let linked_modules = LinkedProjectRuntimeSet::explicit_empty();
        Self::start_with_linked_modules_and_target(
            request,
            &linked_modules,
            GameViewTargetSpec::default(),
        )
    }

    pub fn start_with_linked_modules(
        request: EditorRuntimePlayRequest,
        linked_modules: &LinkedProjectRuntimeSet,
    ) -> EditorGameViewPlayOutput {
        Self::start_with_linked_modules_and_target(
            request,
            linked_modules,
            GameViewTargetSpec::default(),
        )
    }

    pub fn start_with_linked_modules_and_target(
        request: EditorRuntimePlayRequest,
        linked_modules: &LinkedProjectRuntimeSet,
        game_view_target: GameViewTargetSpec,
    ) -> EditorGameViewPlayOutput {
        let mut report = GameViewPresentReport::base(&request);
        if request.frame_limit == 0 {
            report.diagnostics.push(GameViewPresentDiagnostic::error(
                "invalid_frame_limit",
                "request",
                "frame_limit must be greater than zero.",
            ));
            finalize_report(&mut report, &request.runtime_package_path);
            return EditorGameViewPlayOutput {
                instance: None,
                frame: None,
                present_report: report,
            };
        }

        let package_load = load_runtime_package(&request.runtime_package_path);
        report.diagnostics.extend(convert_runtime_diagnostics(
            "package",
            &package_load.diagnostics.issues,
        ));
        let Some(package) = package_load.value else {
            report.texture_descriptor_status = "not_started".to_string();
            finalize_report(&mut report, &request.runtime_package_path);
            return EditorGameViewPlayOutput {
                instance: None,
                frame: None,
                present_report: report,
            };
        };

        let bound_runtime = match ProjectRuntimeBootstrap::bind(&package, linked_modules) {
            Ok(bound_runtime) => bound_runtime,
            Err(error) => {
                report.diagnostics.push(GameViewPresentDiagnostic::error(
                    error.code,
                    "project_runtime",
                    error.message,
                ));
                report.next_actions.push(error.next_action);
                finalize_report(&mut report, &request.runtime_package_path);
                return EditorGameViewPlayOutput {
                    instance: None,
                    frame: None,
                    present_report: report,
                };
            }
        };
        let parts = bound_runtime.into_parts();
        report.project_runtime_bind_receipt = Some(parts.receipt.clone());

        let world_load = hydrate_active_scene_into_world(&package);
        report.diagnostics.extend(convert_runtime_diagnostics(
            "scene",
            &world_load.diagnostics.issues,
        ));
        let Some((world, hydration_report)) = world_load.value else {
            report.scene_id = Some(package.active_scene.id.clone());
            finalize_report(&mut report, &request.runtime_package_path);
            return EditorGameViewPlayOutput {
                instance: None,
                frame: None,
                present_report: report,
            };
        };

        let aui_texture_asset_ids = package
            .aui_documents
            .documents_by_id
            .values()
            .flat_map(|document| document.nodes.iter())
            .filter_map(|node| node.image.as_ref())
            .map(|image| image.asset_id.clone())
            .collect::<BTreeSet<_>>();
        let sprite_texture_asset_ids = package
            .active_scene
            .entities
            .iter()
            .filter_map(|entity| entity.sprite_renderer2d.as_ref())
            .filter_map(|renderer| renderer.sprite_ref.as_ref())
            .map(|asset| asset.id.clone())
            .chain(
                package
                    .animator2d_registry
                    .clips
                    .iter()
                    .flat_map(|clip| clip.frames.iter())
                    .map(|frame| frame.sprite_asset_id.clone()),
            )
            .collect::<BTreeSet<_>>();
        let runtime_texture_uploads = RuntimeTextureUploadRegistry::load(
            &package.package_dir,
            &package.runtime_asset_index,
            aui_texture_asset_ids
                .iter()
                .chain(sprite_texture_asset_ids.iter())
                .cloned(),
        );
        for diagnostic in runtime_texture_uploads.diagnostics() {
            if aui_texture_asset_ids.contains(&diagnostic.asset_ref_id) {
                report.diagnostics.push(GameViewPresentDiagnostic::error(
                    "aui_image.texture_not_resolved",
                    "runtime_texture_resolve",
                    format!(
                        "AUI texture '{}' failed at {:?}: {}",
                        diagnostic.asset_ref_id, diagnostic.code, diagnostic.message
                    ),
                ));
            }
            if sprite_texture_asset_ids.contains(&diagnostic.asset_ref_id) {
                report.diagnostics.push(GameViewPresentDiagnostic::error(
                    "sprite2d.texture_not_resolved",
                    "runtime_texture_resolve",
                    format!(
                        "Sprite2D texture '{}' failed at {:?}: {}",
                        diagnostic.asset_ref_id, diagnostic.code, diagnostic.message
                    ),
                ));
            }
        }
        let runtime_texture_bindings = runtime_texture_uploads.binding_context();
        let scene_id = package.active_scene.id.clone();
        report.scene_id = Some(scene_id.clone());
        let runtime_authoring_origins = build_runtime_authoring_origin_index(&hydration_report);
        let mut host = EngineHostLoop::with_project_runtime_session(
            scene_id.clone(),
            parts.project_logic,
            parts.project_runtime_session,
        );
        host.set_game_view_target(game_view_target);
        if let Err(diagnostics) = host.set_animator2d_registry(package.animator2d_registry.clone())
        {
            report
                .diagnostics
                .extend(diagnostics.into_iter().map(|diagnostic| {
                    GameViewPresentDiagnostic::error(
                        diagnostic.code,
                        "animator2d_registry",
                        diagnostic.message,
                    )
                    .with_path(diagnostic.path)
                }));
            finalize_report(&mut report, &request.runtime_package_path);
            return EditorGameViewPlayOutput {
                instance: None,
                frame: None,
                present_report: report,
            };
        }
        host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
        host.set_project_observation_contract(package.manifest.observation_contract.clone());
        let mut instance = Self {
            session_id: request.session_id.clone(),
            runtime_package_path: request.runtime_package_path.clone(),
            preview_package_report_path: request.preview_package_report_path.clone(),
            scene_id: scene_id.clone(),
            package,
            world,
            host,
            game_view_target,
            ui_state_producer: parts.ui_state_producer,
            input_mapping: parts.default_input_mapping,
            project_runtime_bind_receipt: parts.receipt,
            project_runtime_session_report: None,
            project_observation_state: None,
            aui_interaction_state: AuiInteractionState::default(),
            aui_feedback_state: AuiControlFeedbackState::default(),
            last_aui_present: None,
            pending_gameplay_input: None,
            state: EditorRuntimePlayState::Loading,
            frame_count: 0,
            last_frame: None,
            last_aui_action_targets: Vec::new(),
            last_rhi_command_plan: None,
            runtime_texture_uploads,
            runtime_texture_bindings,
            report_path: None,
            hydration_report,
            runtime_authoring_origins,
            temporary_edit_records: Vec::new(),
            temporary_edit_sequence: 0,
            temporary_edit_summary: RuntimeTemporaryEditSummary::default(),
        };
        instance.state = EditorRuntimePlayState::Running;

        for _ in 0..request.frame_limit {
            let frame = instance.tick_descriptor_frame(&mut report, None);
            instance.last_frame = Some(frame);
            instance.frame_count += 1;
        }

        report.frame_count = instance.frame_count;
        report.control_state = instance.state;
        report.runtime_advanced = instance.frame_count > 0;
        report.last_frame = instance.last_frame.clone();
        report.last_frame_hash = instance
            .last_frame
            .as_ref()
            .map(|frame| frame.frame_hash.clone());
        report.recompute_status();
        finalize_report(&mut report, &request.runtime_package_path);
        instance.report_path = report.report_path.clone();
        let frame = instance.last_frame.clone();
        let instance = (report.status == GameViewPresentStatus::Success).then_some(instance);
        EditorGameViewPlayOutput {
            instance,
            frame,
            present_report: report,
        }
    }

    pub fn stop(mut self) -> GameViewPresentReport {
        self.state = EditorRuntimePlayState::Stopped;
        self.aui_feedback_state.reset();
        self.host.clear_project_observation_state();
        self.project_observation_state = None;
        let temporary_edit_summary = self.temporary_edit_summary.clone();
        let mut report = GameViewPresentReport {
            schema_version: GAME_VIEW_PRESENT_REPORT_SCHEMA_VERSION.to_string(),
            session_id: self.session_id.clone(),
            status: GameViewPresentStatus::Stopped,
            runtime_package_path: self.runtime_package_path.display().to_string(),
            preview_package_report_path: self.preview_package_report_path.clone(),
            scene_id: Some(self.scene_id.clone()),
            frame_count: self.frame_count,
            last_frame_hash: self
                .last_frame
                .as_ref()
                .map(|frame| frame.frame_hash.clone()),
            game_view_output_kind: "viewport_texture_descriptor".to_string(),
            texture_descriptor_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.texture_descriptor_status.clone())
                .unwrap_or_else(|| "not_started".to_string()),
            gpu_present_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.gpu_present_status.clone())
                .unwrap_or_else(|| "gpu_unavailable".to_string()),
            gpu_present_report_path: None,
            shared_gpu_context_status: "not_connected".to_string(),
            input_bridge_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.input_bridge_status.clone())
                .unwrap_or_else(|| "not_requested".to_string()),
            runtime_input_event_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.runtime_input_event_count)
                .unwrap_or(0),
            filtered_runtime_input_event_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.filtered_runtime_input_event_count)
                .unwrap_or(0),
            aui_consumed_event_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.aui_consumed_event_count)
                .unwrap_or(0),
            gameplay_action_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.gameplay_action_count)
                .unwrap_or(0),
            gameplay_action_ids: self
                .last_frame
                .as_ref()
                .map(|frame| frame.gameplay_action_ids.clone())
                .unwrap_or_default(),
            aui_present_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.aui_present_status.clone())
                .unwrap_or_else(|| "not_started".to_string()),
            stop_status: "stopped".to_string(),
            control_state: EditorRuntimePlayState::Stopped,
            control_command: "stop".to_string(),
            runtime_advanced: false,
            paused_last_frame_reused: true,
            step_count: 0,
            target_runtime_domain: "active_gameview_runtime".to_string(),
            report_path: self.report_path.clone(),
            diagnostics: vec![GameViewPresentDiagnostic::info(
                "editor_gameview_play_instance_stopped",
                "stop",
                "EditorRuntimePlayInstance was dropped by Stop.",
            )],
            next_actions: Vec::new(),
            deferred_flags: deferred_flags(),
            last_frame: self.last_frame.clone(),
            project_runtime_bind_receipt: Some(self.project_runtime_bind_receipt.clone()),
            project_runtime_session_report: self.project_runtime_session_report.clone(),
            project_observation_state: None,
        };
        if temporary_edit_summary.edited_field_count > 0 {
            report.diagnostics.push(GameViewPresentDiagnostic::info(
                "runtime_temporary_edits_discarded",
                "play_control",
                format!(
                    "Discarded Play Mode temporary edits: edited_entity_count={} edited_field_count={} last_field={}.",
                    temporary_edit_summary.edited_entity_count(),
                    temporary_edit_summary.edited_field_count,
                    temporary_edit_summary
                        .last_edited_field_path
                        .as_deref()
                        .unwrap_or("none")
                ),
            ));
        }
        write_game_view_report(&mut report, self.report_path.as_deref().map(Path::new));
        report
    }

    pub fn tick_next_descriptor_frame(&mut self) -> GameViewPresentReport {
        let pending_input = self.pending_gameplay_input.take();
        self.tick_next_descriptor_frame_internal(pending_input)
    }

    pub fn tick_next_descriptor_frame_with_runtime_input(
        &mut self,
        runtime_input_frame: RuntimeInputFrame,
    ) -> GameViewPresentReport {
        self.tick_next_descriptor_frame_internal(Some(runtime_input_frame))
    }

    pub fn route_aui_input_immediately(
        &mut self,
        runtime_input_frame: RuntimeInputFrame,
    ) -> GameViewPresentReport {
        let mut report = self.running_report_base();
        report.control_command = "ordinary_aui_input".to_string();
        report.runtime_advanced = false;
        report.runtime_input_event_count = runtime_input_frame.events.len();

        let Some(mut aui_present) = self.last_aui_present.clone() else {
            report.diagnostics.push(GameViewPresentDiagnostic::warning(
                "aui_present_cache_missing",
                "aui_interaction",
                "Ordinary AUI input requires a successful current GameView AUI presentation.",
            ));
            report.recompute_status();
            return report;
        };
        let Some(last_frame) = self.last_frame.clone() else {
            report.diagnostics.push(GameViewPresentDiagnostic::warning(
                "game_view_frame_missing",
                "aui_interaction",
                "Ordinary AUI input requires a current GameView frame.",
            ));
            report.recompute_status();
            return report;
        };
        if runtime_input_frame.viewport_id.is_empty()
            || last_frame.width != self.game_view_target.extent.width
            || last_frame.height != self.game_view_target.extent.height
        {
            self.aui_interaction_state = AuiInteractionState::default();
            self.aui_feedback_state.reset();
            report.diagnostics.push(GameViewPresentDiagnostic::warning(
                "game_view_presentation_stale",
                "aui_interaction",
                "Ordinary AUI input rejected a stale GameView presentation.",
            ));
            report.recompute_status();
            return report;
        }

        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: self.session_id.clone(),
            target_id: runtime_input_frame.viewport_id.clone(),
            target_extent: self.game_view_target.extent,
            display_rect: GameViewRect::from_extent(self.game_view_target.extent),
            scale_policy: self.game_view_target.scale_policy,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: aui_present.composition.canvas_references.clone(),
        });
        let Ok(presentation) = presentation else {
            self.aui_interaction_state = AuiInteractionState::default();
            self.aui_feedback_state.reset();
            report.diagnostics.push(GameViewPresentDiagnostic::error(
                "game_view_presentation_invalid",
                "aui_interaction",
                "GameView presentation facts could not be resolved for ordinary AUI input.",
            ));
            report.recompute_status();
            return report;
        };
        let feedback_session_id = format!(
            "{}:{}:{}x{}:{:?}",
            self.session_id,
            aui_present.report.document_id,
            self.game_view_target.extent.width,
            self.game_view_target.extent.height,
            self.game_view_target.scale_policy
        );
        let interaction_result = AuiInteractionSystem::process_target_space_session_with_state(
            &aui_present.resolved_document,
            &aui_present.layout,
            &runtime_input_frame,
            &mut self.aui_interaction_state,
            AuiInteractionConfig::default(),
            &presentation,
            &feedback_session_id,
        );
        let filtered_frame =
            runtime_input_frame.filter_consumed_events(&interaction_result.consumed_event_indices);
        report.aui_consumed_event_count = interaction_result.consumed_event_indices.len();
        report.filtered_runtime_input_event_count = filtered_frame.events.len();
        report.input_bridge_status = if report.aui_consumed_event_count > 0 {
            "runtime_input_frame_filtered_by_aui".to_string()
        } else {
            "runtime_input_frame_pending_gameplay".to_string()
        };

        let dispatch = self
            .host
            .dispatch_aui_actions_immediately(&interaction_result.actions, &mut self.world);
        self.project_runtime_session_report = dispatch.project_runtime_session_report.clone();
        self.project_observation_state = dispatch.project_observation_state.clone();
        report.project_runtime_session_report = self.project_runtime_session_report.clone();
        report.project_observation_state = self.project_observation_state.clone();

        if !interaction_result.actions.is_empty() {
            if let Some(refreshed) = build_aui_present_output(
                &self.package,
                &self.world,
                last_frame.frame_index,
                self.ui_state_producer.as_mut(),
            ) {
                aui_present = refreshed;
            }
        }
        let feedback = AuiRuntimePresenter::apply_control_feedback_with_fonts(
            &mut aui_present,
            &interaction_result,
            &mut self.aui_feedback_state,
            presentation_delta_us_from_seconds(
                engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME,
            ),
            &self.package.font_atlases,
            &self.package.font_bundles,
        );
        let render_thread_frame = self.host.render_thread_for_target_with_runtime_resources(
            RenderTarget::viewport_texture(
                last_frame.target_id.clone(),
                last_frame.width,
                last_frame.height,
            )
            .with_presentation_scale_policy(self.game_view_target.scale_policy),
            Some(&aui_present.overlay),
            Some(&aui_present.composition),
            None,
            Some(&self.runtime_texture_bindings),
        );
        self.last_rhi_command_plan = Some(render_thread_frame.renderer_output.rhi_command_plan);
        self.last_aui_present = Some(aui_present.clone());
        self.enqueue_pending_gameplay_input(filtered_frame);

        let mut updated_frame = last_frame.clone();
        updated_frame.ui_draw_item_count = aui_present.report.draw_item_count;
        updated_frame.aui_present_status =
            aui_present_status_name(aui_present.report.status).to_string();
        updated_frame.input_bridge_status = report.input_bridge_status.clone();
        updated_frame.runtime_input_event_count = report.runtime_input_event_count;
        updated_frame.filtered_runtime_input_event_count =
            report.filtered_runtime_input_event_count;
        updated_frame.aui_consumed_event_count = report.aui_consumed_event_count;
        updated_frame.aui_feedback_override_count = feedback.overrides.len();
        updated_frame.aui_feedback_profile_ids = feedback
            .report
            .resolved_profile_ids
            .iter()
            .cloned()
            .collect();
        updated_frame.aui_presentation_identity = aui_presentation_identity(Some(&aui_present));
        updated_frame.gameplay_action_count = 0;
        updated_frame.gameplay_action_ids.clear();
        self.last_aui_action_targets =
            build_aui_action_targets(&aui_present, updated_frame.width, updated_frame.height);
        self.last_frame = Some(updated_frame.clone());
        report.last_frame = Some(updated_frame);
        report.last_frame_hash = Some(last_frame.frame_hash);
        report.frame_count = self.frame_count;
        report.recompute_status();
        report
    }

    pub fn cancel_pending_game_view_input(&mut self) {
        self.pending_gameplay_input = None;
        self.aui_interaction_state = AuiInteractionState::default();
        self.aui_feedback_state.reset();
    }

    fn enqueue_pending_gameplay_input(&mut self, frame: RuntimeInputFrame) {
        if frame.events.is_empty() {
            return;
        }
        let pending = self.pending_gameplay_input.get_or_insert_with(|| {
            RuntimeInputFrame::new(frame.frame_id, frame.viewport_id.clone())
        });
        pending.frame_id = frame.frame_id;
        pending.viewport_id = frame.viewport_id;
        pending.modifiers = frame.modifiers;
        pending.pointer_position = frame.pointer_position;
        for event in frame.events {
            if matches!(event, RuntimeInputEvent::PointerMove { .. }) {
                pending
                    .events
                    .retain(|queued| !matches!(queued, RuntimeInputEvent::PointerMove { .. }));
            }
            pending.events.push(event);
        }
        const MAX_PENDING_GAMEPLAY_EVENTS: usize = 64;
        if pending.events.len() > MAX_PENDING_GAMEPLAY_EVENTS {
            let excess = pending.events.len() - MAX_PENDING_GAMEPLAY_EVENTS;
            pending.events.drain(0..excess);
        }
    }

    fn tick_next_descriptor_frame_internal(
        &mut self,
        runtime_input_frame: Option<RuntimeInputFrame>,
    ) -> GameViewPresentReport {
        if self.state == EditorRuntimePlayState::Paused {
            return self.paused_report("auto_tick");
        }
        self.state = EditorRuntimePlayState::Running;
        let mut report = self.running_report_base();
        report.control_command = "tick".to_string();
        let frame = self.tick_descriptor_frame(&mut report, runtime_input_frame);
        self.last_frame = Some(frame);
        self.frame_count += 1;
        report.frame_count = self.frame_count;
        report.control_state = self.state;
        report.runtime_advanced = true;
        report.last_frame = self.last_frame.clone();
        report.last_frame_hash = self
            .last_frame
            .as_ref()
            .map(|frame| frame.frame_hash.clone());
        report.recompute_status();
        report.report_path = self.report_path.clone();
        report
    }

    pub fn pause(&mut self) -> GameViewPresentReport {
        self.state = EditorRuntimePlayState::Paused;
        let mut report = self.paused_report("pause");
        report.diagnostics.push(GameViewPresentDiagnostic::info(
            "editor_gameview_play_paused",
            "play_control",
            "EditorRuntimePlayInstance is paused and will reuse the last frame until resumed or stepped.",
        ));
        write_game_view_report(&mut report, self.report_path.as_deref().map(Path::new));
        report
    }

    pub fn resume(&mut self) -> GameViewPresentReport {
        self.state = EditorRuntimePlayState::Running;
        let mut report = self.running_report_base();
        report.control_state = self.state;
        report.control_command = "resume".to_string();
        report.runtime_advanced = false;
        report.paused_last_frame_reused = true;
        report.diagnostics.push(GameViewPresentDiagnostic::info(
            "editor_gameview_play_resumed",
            "play_control",
            "EditorRuntimePlayInstance resumed without recreating the runtime instance.",
        ));
        report.recompute_status();
        write_game_view_report(&mut report, self.report_path.as_deref().map(Path::new));
        report
    }

    pub fn step_next_frame(&mut self) -> GameViewPresentReport {
        self.state = EditorRuntimePlayState::Stepping;
        let mut report = self.running_report_base();
        report.control_state = self.state;
        report.control_command = "step_frame".to_string();
        report.step_count = 1;
        let frame = self.tick_descriptor_frame(&mut report, None);
        self.last_frame = Some(frame);
        self.frame_count += 1;
        self.state = EditorRuntimePlayState::Paused;
        report.frame_count = self.frame_count;
        report.control_state = self.state;
        report.runtime_advanced = true;
        report.last_frame = self.last_frame.clone();
        report.last_frame_hash = self
            .last_frame
            .as_ref()
            .map(|frame| frame.frame_hash.clone());
        report.recompute_status();
        finalize_report(&mut report, &self.runtime_package_path);
        self.report_path = report.report_path.clone();
        report
    }

    pub fn control_state(&self) -> EditorRuntimePlayState {
        self.state
    }

    pub fn apply_gpu_present_result(
        &mut self,
        gpu_present_status: impl Into<String>,
        shared_gpu_context_status: impl Into<String>,
        diagnostics: Vec<GameViewPresentDiagnostic>,
    ) -> GameViewPresentReport {
        let gpu_present_status = gpu_present_status.into();
        let shared_gpu_context_status = shared_gpu_context_status.into();
        if let Some(frame) = &mut self.last_frame {
            frame.gpu_present_status = gpu_present_status.clone();
        }
        let mut report = self.running_report_base();
        report.control_command = "gpu_present".to_string();
        report.frame_count = self.frame_count;
        report.last_frame = self.last_frame.clone();
        report.last_frame_hash = self
            .last_frame
            .as_ref()
            .map(|frame| frame.frame_hash.clone());
        report.texture_descriptor_status = self
            .last_frame
            .as_ref()
            .map(|frame| frame.texture_descriptor_status.clone())
            .unwrap_or_else(|| "not_started".to_string());
        report.gpu_present_status = gpu_present_status;
        report.shared_gpu_context_status = shared_gpu_context_status;
        report.diagnostics.extend(diagnostics);
        report.recompute_status();
        report
    }

    pub fn last_rhi_command_plan(&self) -> Option<&RhiCommandPlan> {
        self.last_rhi_command_plan.as_ref()
    }

    pub fn last_aui_action_targets(&self) -> &[GameViewAuiActionTarget] {
        &self.last_aui_action_targets
    }

    pub fn set_project_runtime_session_report_level(
        &mut self,
        level: ProjectRuntimeSessionReportLevel,
    ) {
        self.host.set_project_runtime_session_report_level(level);
    }

    pub fn font_bundles(&self) -> &engine_runtime::font_bundle::RuntimeFontBundleRegistry {
        &self.package.font_bundles
    }

    pub fn runtime_texture_uploads(&self) -> &RuntimeTextureUploadRegistry {
        &self.runtime_texture_uploads
    }

    fn running_report_base(&self) -> GameViewPresentReport {
        GameViewPresentReport {
            schema_version: GAME_VIEW_PRESENT_REPORT_SCHEMA_VERSION.to_string(),
            session_id: self.session_id.clone(),
            status: GameViewPresentStatus::Failed,
            runtime_package_path: self.runtime_package_path.display().to_string(),
            preview_package_report_path: self.preview_package_report_path.clone(),
            scene_id: Some(self.scene_id.clone()),
            frame_count: self.frame_count,
            last_frame_hash: self
                .last_frame
                .as_ref()
                .map(|frame| frame.frame_hash.clone()),
            game_view_output_kind: "viewport_texture_descriptor".to_string(),
            texture_descriptor_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.texture_descriptor_status.clone())
                .unwrap_or_else(|| "not_started".to_string()),
            gpu_present_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.gpu_present_status.clone())
                .unwrap_or_else(|| "gpu_unavailable".to_string()),
            gpu_present_report_path: None,
            shared_gpu_context_status: "not_connected".to_string(),
            input_bridge_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.input_bridge_status.clone())
                .unwrap_or_else(|| "not_requested".to_string()),
            runtime_input_event_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.runtime_input_event_count)
                .unwrap_or(0),
            filtered_runtime_input_event_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.filtered_runtime_input_event_count)
                .unwrap_or(0),
            aui_consumed_event_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.aui_consumed_event_count)
                .unwrap_or(0),
            gameplay_action_count: self
                .last_frame
                .as_ref()
                .map(|frame| frame.gameplay_action_count)
                .unwrap_or(0),
            gameplay_action_ids: self
                .last_frame
                .as_ref()
                .map(|frame| frame.gameplay_action_ids.clone())
                .unwrap_or_default(),
            aui_present_status: self
                .last_frame
                .as_ref()
                .map(|frame| frame.aui_present_status.clone())
                .unwrap_or_else(|| "not_started".to_string()),
            stop_status: "running".to_string(),
            control_state: self.state,
            control_command: "status".to_string(),
            runtime_advanced: false,
            paused_last_frame_reused: self.state == EditorRuntimePlayState::Paused,
            step_count: 0,
            target_runtime_domain: "active_gameview_runtime".to_string(),
            report_path: self.report_path.clone(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
            deferred_flags: deferred_flags(),
            last_frame: self.last_frame.clone(),
            project_runtime_bind_receipt: Some(self.project_runtime_bind_receipt.clone()),
            project_runtime_session_report: self.project_runtime_session_report.clone(),
            project_observation_state: self.project_observation_state.clone(),
        }
    }

    fn paused_report(&self, control_command: &str) -> GameViewPresentReport {
        let mut report = self.running_report_base();
        report.control_state = EditorRuntimePlayState::Paused;
        report.control_command = control_command.to_string();
        report.runtime_advanced = false;
        report.paused_last_frame_reused = self.last_frame.is_some();
        report.recompute_status();
        report
    }

    fn tick_descriptor_frame(
        &mut self,
        report: &mut GameViewPresentReport,
        runtime_input_frame: Option<RuntimeInputFrame>,
    ) -> GameViewRuntimeFrame {
        let expected_frame_index = self.frame_count + 1;
        let mut aui_present = build_aui_present_output(
            &self.package,
            &self.world,
            expected_frame_index,
            self.ui_state_producer.as_mut(),
        );
        let mut frame_input = EngineFrameInput::new(EngineHostMode::EditorPlay)
            .with_runtime_texture_bindings(self.runtime_texture_bindings.clone());
        let mut ui_draw_item_count = 0;
        let aui_present_status = if let Some(aui_present) = aui_present.as_ref() {
            ui_draw_item_count = aui_present.report.draw_item_count;
            let status = aui_present_status_name(aui_present.report.status).to_string();
            if aui_present.report.status == AuiRuntimePresentStatus::Failed {
                report.diagnostics.push(GameViewPresentDiagnostic::error(
                    "aui_present_failed",
                    "aui_present",
                    "AUI runtime present produced a failed report.",
                ));
            }
            frame_input = frame_input
                .with_aui_overlay(aui_present.overlay.clone())
                .with_aui_composition(aui_present.composition.clone());
            status
        } else if self.package.aui_manifest.documents.is_empty() {
            "no_documents".to_string()
        } else {
            report.diagnostics.push(GameViewPresentDiagnostic::warning(
                "aui_document_missing",
                "aui_present",
                "RuntimePackage declares AUI documents, but no document could be presented.",
            ));
            "missing_document".to_string()
        };
        report.aui_present_status = aui_present_status.clone();

        let mut input_bridge_status = if runtime_input_frame.is_some() {
            "runtime_input_frame".to_string()
        } else {
            "not_requested".to_string()
        };
        let mut runtime_input_event_count = 0;
        let mut filtered_runtime_input_event_count = 0;
        let mut aui_consumed_event_count = 0;
        let mut aui_feedback_override_count = 0;
        let mut aui_feedback_profile_ids = Vec::new();
        let mut gameplay_action_count = 0;
        let mut gameplay_action_ids = Vec::new();

        if let Some(runtime_frame) = runtime_input_frame.as_ref() {
            runtime_input_event_count = runtime_frame.events.len();
            let mut filtered_frame = runtime_frame.clone();
            if let Some(aui_present_view) = aui_present.as_ref() {
                let config = AuiInteractionConfig::default();
                let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
                    session_id: self.session_id.clone(),
                    target_id: runtime_frame.viewport_id.clone(),
                    target_extent: self.game_view_target.extent,
                    display_rect: GameViewRect::from_extent(self.game_view_target.extent),
                    scale_policy: self.game_view_target.scale_policy,
                    surface_generation: 1,
                    presentation_revision: 1,
                    canvas_references: aui_present_view.composition.canvas_references.clone(),
                });
                if let Ok(presentation) = presentation {
                    let feedback_session_id = format!(
                        "{}:{}:{}x{}:{:?}",
                        self.session_id,
                        aui_present_view.report.document_id,
                        self.game_view_target.extent.width,
                        self.game_view_target.extent.height,
                        self.game_view_target.scale_policy
                    );
                    let interaction_result =
                        AuiInteractionSystem::process_target_space_session_with_state(
                            &aui_present_view.resolved_document,
                            &aui_present_view.layout,
                            runtime_frame,
                            &mut self.aui_interaction_state,
                            config,
                            &presentation,
                            &feedback_session_id,
                        );
                    aui_consumed_event_count = interaction_result.consumed_event_indices.len();
                    filtered_frame = runtime_frame
                        .filter_consumed_events(&interaction_result.consumed_event_indices);
                    if aui_consumed_event_count > 0 {
                        input_bridge_status = "runtime_input_frame_filtered_by_aui".to_string();
                    }
                    if let Some(present) = aui_present.as_mut() {
                        let feedback = AuiRuntimePresenter::apply_control_feedback_with_fonts(
                            present,
                            &interaction_result,
                            &mut self.aui_feedback_state,
                            presentation_delta_us_from_seconds(
                                engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME,
                            ),
                            &self.package.font_atlases,
                            &self.package.font_bundles,
                        );
                        aui_feedback_override_count = feedback.overrides.len();
                        aui_feedback_profile_ids = feedback
                            .report
                            .resolved_profile_ids
                            .iter()
                            .cloned()
                            .collect();
                        frame_input = frame_input
                            .with_aui_overlay(present.overlay.clone())
                            .with_aui_composition(present.composition.clone());
                    }
                    frame_input = frame_input.with_aui_interaction(interaction_result);
                } else {
                    report.diagnostics.push(GameViewPresentDiagnostic::error(
                        "game_view_presentation_invalid",
                        "aui_interaction",
                        "GameView presentation facts could not be resolved for AUI input.",
                    ));
                    input_bridge_status = "aui_presentation_rejected".to_string();
                }
            }
            filtered_runtime_input_event_count = filtered_frame.events.len();

            let input_result = InputResolver::resolve(&filtered_frame, &self.input_mapping);
            let snapshot = input_result.action_snapshot;
            gameplay_action_count = snapshot.action_count();
            gameplay_action_ids = snapshot.action_ids();
            let input_trace_summary = InputTraceSummary::from_snapshot(Some(&snapshot)).with_route(
                Some(runtime_frame.viewport_id.clone()),
                Some("EditorGameView".to_string()),
                Some(if aui_consumed_event_count > 0 {
                    "RuntimeInputFrameFilteredByAui".to_string()
                } else {
                    "RuntimeInputFrame".to_string()
                }),
                Some("editor_gameview_play".to_string()),
            );
            frame_input = frame_input
                .with_action_snapshot(snapshot)
                .with_input_trace_summary(input_trace_summary);
        }
        report.input_bridge_status = input_bridge_status.clone();
        report.runtime_input_event_count = runtime_input_event_count;
        report.filtered_runtime_input_event_count = filtered_runtime_input_event_count;
        report.aui_consumed_event_count = aui_consumed_event_count;
        report.gameplay_action_count = gameplay_action_count;
        report.gameplay_action_ids = gameplay_action_ids.clone();

        let output = self.host.tick(frame_input, &mut self.world);
        let animator2d_play_observations = self
            .host
            .animator2d_module()
            .map(|module| {
                self.world
                    .entity_ids()
                    .into_iter()
                    .filter(|entity_id| self.world.animator2d(entity_id).is_some())
                    .filter_map(|entity_id| {
                        crate::Animator2DAuthoringService::play_observation(
                            entity_id,
                            module,
                            &output.animator2d_frame_result,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.project_runtime_session_report = output.project_runtime_session_report.clone();
        report.project_runtime_session_report = self.project_runtime_session_report.clone();
        self.project_observation_state = output.project_observation_state.clone();
        report.project_observation_state = self.project_observation_state.clone();
        let mut diagnostics = Vec::new();
        let frame_hash = output.frame_hash.unwrap_or_else(|| {
            report.diagnostics.push(GameViewPresentDiagnostic::error(
                "runtime_frame_hash_missing",
                "tick",
                "EngineHostLoop did not produce a frame_hash.",
            ));
            diagnostics.push("runtime_frame_hash_missing".to_string());
            "missing-frame-hash".to_string()
        });
        let renderable_count = runtime_visual_item_count(&self.world);
        let render_thread_frame = output.render_thread_frame.as_ref();
        self.last_rhi_command_plan =
            render_thread_frame.map(|frame| frame.renderer_output.rhi_command_plan.clone());
        let rhi_command_count = render_thread_frame
            .map(|frame| frame.renderer_output.rhi_command_plan.commands.len())
            .unwrap_or(0);
        let render_graph_pass_count = render_thread_frame
            .map(|frame| frame.renderer_output.render_graph.passes.len())
            .unwrap_or(0);
        let runtime_target_kind = render_thread_frame
            .map(|frame| format!("{:?}", frame.renderer_output.target_summary.target_kind))
            .unwrap_or_else(|| "missing".to_string());
        let descriptor =
            render_thread_frame.and_then(|frame| frame.renderer_output.texture_descriptor.clone());
        let (target_id, _descriptor_texture_id, width, height, descriptor_status) =
            if let Some(descriptor) = descriptor {
                (
                    descriptor.target_id,
                    descriptor.texture_id,
                    descriptor.width,
                    descriptor.height,
                    "descriptor_only".to_string(),
                )
            } else {
                report.diagnostics.push(GameViewPresentDiagnostic::error(
                    "viewport_texture_descriptor_missing",
                    "present",
                    "RuntimeRenderer did not produce a ViewportTextureDescriptor for GameView.",
                ));
                diagnostics.push("viewport_texture_descriptor_missing".to_string());
                (
                    "viewport-main".to_string(),
                    "viewport-main".to_string(),
                    1280,
                    720,
                    "missing".to_string(),
                )
            };
        let texture_id = stable_game_view_surface_id(&self.session_id, &target_id);
        let aui_presentation_identity = aui_presentation_identity(aui_present.as_ref());
        self.last_aui_action_targets = aui_present
            .as_ref()
            .map(|present| build_aui_action_targets(present, width, height))
            .unwrap_or_default();
        self.last_aui_present = aui_present.clone();
        report.texture_descriptor_status = descriptor_status.clone();
        report.gpu_present_status = "gpu_unavailable".to_string();
        GameViewRuntimeFrame {
            schema_version: GAME_VIEW_RUNTIME_FRAME_SCHEMA_VERSION.to_string(),
            session_id: self.session_id.clone(),
            scene_id: self.scene_id.clone(),
            frame_index: output.frame_index,
            frame_hash,
            target_id,
            texture_id,
            width,
            height,
            aui_presentation_identity,
            presentation_scale_policy: self.game_view_target.scale_policy,
            renderable_count,
            ui_draw_item_count,
            aui_present_status,
            input_bridge_status,
            runtime_input_event_count,
            filtered_runtime_input_event_count,
            aui_consumed_event_count,
            aui_feedback_override_count,
            aui_feedback_profile_ids,
            gameplay_action_count,
            gameplay_action_ids,
            texture_descriptor_status: descriptor_status,
            gpu_present_status: "gpu_unavailable".to_string(),
            rhi_command_count,
            render_graph_pass_count,
            runtime_target_kind,
            animator2d_play_observations,
            diagnostics,
        }
    }
}

fn aui_presentation_identity(present: Option<&AuiRuntimePresentOutput>) -> String {
    let Some(present) = present else {
        return "aui:none".to_string();
    };
    let visible_content = (
        &present.composition.stages,
        &present.composition.glyph_plan,
        &present.composition.canvas_references,
    );
    match serde_json::to_vec(&visible_content) {
        Ok(bytes) => format!("aui:sha256:{:x}", Sha256::digest(bytes)),
        Err(_) => "aui:serialization-failed".to_string(),
    }
}

fn build_aui_action_targets(
    present: &AuiRuntimePresentOutput,
    reference_width: u32,
    reference_height: u32,
) -> Vec<GameViewAuiActionTarget> {
    let nodes = present
        .resolved_document
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let canvases = present
        .resolved_document
        .canvases
        .iter()
        .map(|canvas| (canvas.canvas_id.as_str(), canvas.reference_resolution))
        .collect::<BTreeMap<_, _>>();
    let mut targets = Vec::new();
    for computed in &present.layout.computed_nodes {
        let Some(node) = nodes.get(computed.node_id.as_str()) else {
            continue;
        };
        let reference = canvases.get(computed.canvas_id.as_str()).copied();
        for action in &node.action_refs {
            if action.event != AuiActionEvent::Click {
                continue;
            }
            targets.push(GameViewAuiActionTarget {
                canvas_id: computed.canvas_id.clone(),
                node_id: node.node_id.clone(),
                action_id: action.action_id.clone(),
                visible: computed.effective_visible,
                interactable: node.interactable,
                computed_rect: computed.rect,
                effective_clip_rect: computed.effective_clip_rect,
                reference_width: reference
                    .filter(|value| value.x > 0.0)
                    .map_or(reference_width, |value| value.x.round() as u32),
                reference_height: reference
                    .filter(|value| value.y > 0.0)
                    .map_or(reference_height, |value| value.y.round() as u32),
            });
        }
    }
    targets
}

fn runtime_temporary_world_mutation_error(
    error: engine_runtime::world::WorldMutationError,
) -> RuntimeTemporaryEditError {
    RuntimeTemporaryEditError::new(error.code, error.message)
}

fn build_aui_present_output(
    package: &RuntimePackage,
    world: &World,
    frame_index: u64,
    producer: &mut dyn ProjectUiStateSnapshotProducer,
) -> Option<AuiRuntimePresentOutput> {
    let document_id = package
        .aui_manifest
        .documents
        .first()
        .map(|entry| entry.document_id.as_str())?;
    let document = package.aui_documents.get(document_id)?;
    let snapshot_output = producer.produce(ProjectUiStateProducerContext::new(
        frame_index,
        package,
        world,
    ));
    Some(AuiRuntimePresenter::present_project_snapshot_with_fonts(
        document,
        snapshot_output,
        &package.font_atlases,
        &package.font_bundles,
    ))
}

fn aui_present_status_name(status: AuiRuntimePresentStatus) -> &'static str {
    match status {
        AuiRuntimePresentStatus::Success => "success",
        AuiRuntimePresentStatus::Partial => "partial",
        AuiRuntimePresentStatus::Failed => "failed",
    }
}

fn convert_runtime_diagnostics(
    layer: &str,
    diagnostics: &[RuntimeDiagnostic],
) -> Vec<GameViewPresentDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let severity = match diagnostic.severity {
                RuntimeDiagnosticSeverity::Error => GameViewPresentDiagnosticSeverity::Error,
                RuntimeDiagnosticSeverity::Warning => GameViewPresentDiagnosticSeverity::Warning,
            };
            GameViewPresentDiagnostic {
                severity,
                code: match diagnostic.severity {
                    RuntimeDiagnosticSeverity::Error => format!("{layer}_load_error"),
                    RuntimeDiagnosticSeverity::Warning => format!("{layer}_load_warning"),
                },
                layer: layer.to_string(),
                message: diagnostic.message.clone(),
                path: Some(diagnostic.path.clone()),
            }
        })
        .collect()
}

fn runtime_visual_item_count(world: &World) -> usize {
    let mesh_renderable_count = world.alive_renderables().len();
    let sprite_renderer_count = world
        .entity_ids()
        .into_iter()
        .filter(|entity_id| world.sprite_renderer2d(entity_id).is_some())
        .count();
    mesh_renderable_count + sprite_renderer_count
}

fn deferred_flags() -> Vec<String> {
    vec![
        "world_pickcollector_deferred".to_string(),
        "editor_overlay_runtime_input_arbitration_deferred".to_string(),
        "gameview_wheel_text_ime_gamepad_bridge_deferred".to_string(),
        "pause_step_deferred".to_string(),
        "maximize_on_play_deferred".to_string(),
        "embedded_native_surface_deferred".to_string(),
        "multi_instance_play_deferred".to_string(),
    ]
}

fn finalize_report(report: &mut GameViewPresentReport, runtime_package_path: &Path) {
    report.recompute_status();
    let path = game_view_report_path(runtime_package_path);
    write_game_view_report(report, Some(&path));
}

fn write_game_view_report(report: &mut GameViewPresentReport, path: Option<&Path>) {
    let Some(path) = path else {
        return;
    };
    report.report_path = Some(path.display().to_string());
    match serde_json::to_string_pretty(report) {
        Ok(text) => {
            let write_result = path
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| "GameView report path has no owned root".to_string())
                .and_then(|root| {
                    let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
                    crate::ProjectWriteScope::open(root)
                        .and_then(|scope| scope.write_atomic(relative, text.as_bytes()))
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = write_result {
                report.diagnostics.push(GameViewPresentDiagnostic::error(
                    "gameview_present_report_write_failed",
                    "report",
                    format!("Could not write GameView present report: {error}"),
                ));
                report.status = GameViewPresentStatus::Failed;
            }
        }
        Err(error) => {
            report.diagnostics.push(GameViewPresentDiagnostic::error(
                "gameview_present_report_serialize_failed",
                "report",
                format!("Could not serialize GameView present report: {error}"),
            ));
            report.status = GameViewPresentStatus::Failed;
        }
    }
}

fn game_view_report_path(runtime_package_path: &Path) -> PathBuf {
    runtime_package_path
        .parent()
        .unwrap_or(runtime_package_path)
        .join("reports")
        .join("editor-gameview-present-report.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::aui::{
        AuiActionRef, AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect,
    };
    use engine_runtime::game_view_presentation::GameViewTargetSpec;
    use engine_runtime::input_action::PointerPosition;
    use engine_runtime::input_mapping::{RuntimeInputEvent, RuntimePointerButton};
    use engine_runtime::project_observation::{
        ProjectObservationContract, ProjectObservationEntry, ProjectObservationType,
        PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION,
    };
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    use engine_runtime::runtime_package::{
        RuntimeAuiManifest, RuntimeAuiManifestEntry, RUNTIME_AUI_MANIFEST_SCHEMA_VERSION,
    };
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn editor_gameview_play_schema_serializes() {
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-test".to_string(),
            project_root: PathBuf::from("project"),
            runtime_package_path: PathBuf::from("runtime-package"),
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 3,
            requested_by: "Toolbar".to_string(),
            preview_package_report_path: Some("preview.json".to_string()),
        };
        let json = serde_json::to_string(&request).expect("schema should serialize");

        assert!(json.contains(EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION));
        assert!(json.contains("scene-main"));
    }

    #[test]
    fn editor_runtime_play_instance_runs_descriptor_frames() {
        let root = temp_root("runtime-instance");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-runtime-instance".to_string(),
            project_root: root.clone(),
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 2,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };

        let output = EditorRuntimePlayInstance::start(request);

        assert_eq!(
            output.present_report.status,
            GameViewPresentStatus::Success,
            "{:?}",
            output.present_report.diagnostics
        );
        assert_eq!(output.present_report.frame_count, 2);
        let frame = output.frame.expect("last frame");
        assert_eq!(frame.texture_descriptor_status, "descriptor_only");
        assert_eq!(frame.gpu_present_status, "gpu_unavailable");
        assert!(frame.rhi_command_count > 0);
        assert!(frame.render_graph_pass_count > 0);
        assert_eq!(frame.runtime_target_kind, "ViewportTexture");
        assert!(!frame.frame_hash.is_empty());
        assert_eq!(frame.input_bridge_status, "not_requested");
        let instance = output.instance.expect("runtime instance");
        assert!(instance.last_rhi_command_plan().is_some());
        assert!(output
            .present_report
            .report_path
            .as_ref()
            .is_some_and(|path| Path::new(path).exists()));
    }

    #[test]
    fn editor_gameview_sprite_texture_is_uploaded_and_bound() {
        let root = temp_root("sprite-texture-bound");
        let package_dir = write_runtime_package_with_sprite_texture(
            &root,
            "runtime-package",
            SpriteTextureFixture::Ready,
        );
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-sprite-texture-bound".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };

        let output = EditorRuntimePlayInstance::start(request);

        assert_eq!(
            output.present_report.status,
            GameViewPresentStatus::Success,
            "{:?}",
            output.present_report.diagnostics
        );
        let instance = output.instance.expect("runtime instance");
        assert!(instance
            .runtime_texture_uploads()
            .uploads()
            .any(|upload| upload.asset_id == "texture-sprite"));
        let sprite_draw = instance
            .last_rhi_command_plan()
            .expect("RHI command plan")
            .commands
            .iter()
            .find_map(|command| match command {
                RhiCommand::Draw {
                    payload:
                        RhiDrawPayload::SpriteTextured {
                            sprite_ref,
                            texture,
                            fallback_used,
                            ..
                        },
                    ..
                } if sprite_ref == "texture-sprite" => Some((texture, fallback_used)),
                _ => None,
            })
            .expect("Sprite2D textured draw");
        assert!(
            sprite_draw.0.is_some(),
            "Sprite2D texture handle must resolve"
        );
        assert!(
            !sprite_draw.1,
            "Sprite2D draw must not use fallback binding"
        );
    }

    #[test]
    fn editor_gameview_sprite_texture_missing_reports_owner_diagnostic() {
        let root = temp_root("sprite-texture-missing");
        let package_dir = write_runtime_package_with_sprite_texture(
            &root,
            "runtime-package",
            SpriteTextureFixture::MissingPixelPayload,
        );
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-sprite-texture-missing".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };

        let output = EditorRuntimePlayInstance::start(request);

        assert!(output
            .present_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite2d.texture_not_resolved"));
    }

    #[test]
    fn editor_gameview_continuous_tick_keeps_report_in_memory() {
        let root = temp_root("continuous-tick-report");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let output = EditorRuntimePlayInstance::start(EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-continuous-tick-report".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        });
        let report_path = PathBuf::from(
            output
                .present_report
                .report_path
                .as_deref()
                .expect("start report path"),
        );
        let report_on_disk = fs::read(&report_path).expect("start report on disk");
        let mut instance = output.instance.expect("runtime instance");

        let tick_report = instance.tick_next_descriptor_frame();

        assert_eq!(tick_report.report_path.as_deref(), report_path.to_str());
        assert_eq!(
            fs::read(&report_path).expect("report remains readable"),
            report_on_disk.clone(),
            "continuous tick must not rewrite the disk report"
        );

        instance.apply_gpu_present_result("presented", "Available", Vec::new());
        assert_eq!(
            fs::read(&report_path).expect("report remains readable after present"),
            report_on_disk,
            "per-frame GPU present must not rewrite the disk report"
        );

        instance.pause();
        let paused_report_on_disk = fs::read(&report_path).expect("pause report on disk");
        assert_ne!(paused_report_on_disk, report_on_disk);

        instance.tick_next_descriptor_frame();
        assert_eq!(
            fs::read(&report_path).expect("paused report remains readable"),
            paused_report_on_disk,
            "paused auto tick must not rewrite the disk report"
        );
    }

    #[test]
    fn runtime_play_instance_can_tick_and_mark_gpu_presented() {
        let root = temp_root("runtime-gpu-present");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-runtime-gpu-present".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };
        let output = EditorRuntimePlayInstance::start(request);
        let first_frame = output.frame.clone().expect("first frame");
        let mut instance = output.instance.expect("runtime instance");

        let tick_report = instance.tick_next_descriptor_frame();
        assert_eq!(tick_report.frame_count, 2);
        let second_frame = tick_report.last_frame.as_ref().expect("second frame");
        assert_eq!(first_frame.texture_id, second_frame.texture_id);
        assert_eq!(
            second_frame.texture_id,
            stable_game_view_surface_id(&second_frame.session_id, &second_frame.target_id)
        );
        assert_eq!(
            tick_report
                .last_frame
                .as_ref()
                .expect("last frame")
                .gpu_present_status,
            "gpu_unavailable"
        );

        let present_report = instance.apply_gpu_present_result(
            "presented",
            "Available",
            vec![GameViewPresentDiagnostic::info(
                "gpu_present_test",
                "gpu_present",
                "test present succeeded",
            )],
        );

        assert_eq!(present_report.gpu_present_status, "presented");
        assert_eq!(present_report.shared_gpu_context_status, "Available");
        assert_eq!(
            present_report
                .last_frame
                .as_ref()
                .expect("last frame")
                .gpu_present_status,
            "presented"
        );
        assert_eq!(present_report.status, GameViewPresentStatus::Success);
    }

    #[test]
    fn runtime_play_instance_pause_reuses_last_frame_and_step_advances_once() {
        let root = temp_root("runtime-pause-step");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-runtime-pause-step".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };
        let output = EditorRuntimePlayInstance::start(request);
        let mut instance = output.instance.expect("runtime instance");
        let first_frame_count = output.present_report.frame_count;

        let pause_report = instance.pause();
        assert_eq!(pause_report.control_state, EditorRuntimePlayState::Paused);
        assert_eq!(pause_report.control_command, "pause");
        assert_eq!(pause_report.frame_count, first_frame_count);
        assert!(!pause_report.runtime_advanced);
        assert!(pause_report.paused_last_frame_reused);

        let paused_tick_report = instance.tick_next_descriptor_frame();
        assert_eq!(paused_tick_report.frame_count, first_frame_count);
        assert_eq!(paused_tick_report.control_command, "auto_tick");
        assert!(!paused_tick_report.runtime_advanced);
        assert!(paused_tick_report.paused_last_frame_reused);

        let step_report = instance.step_next_frame();
        assert_eq!(step_report.control_state, EditorRuntimePlayState::Paused);
        assert_eq!(step_report.control_command, "step_frame");
        assert_eq!(step_report.frame_count, first_frame_count + 1);
        assert!(step_report.runtime_advanced);
        assert_eq!(step_report.step_count, 1);
        assert_eq!(step_report.target_runtime_domain, "active_gameview_runtime");
    }

    #[test]
    fn editor_gameview_project_runtime_session_pause_step_stop_and_replay_are_isolated() {
        let root = temp_root("project-runtime-session-lifecycle");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "editor-session-first".to_string(),
            project_root: root.clone(),
            runtime_package_path: package_dir.clone(),
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };

        let first = EditorRuntimePlayInstance::start(request);
        let first_surface_id = first
            .frame
            .as_ref()
            .expect("first Runtime frame")
            .texture_id
            .clone();
        let first_summary = first
            .present_report
            .project_runtime_session_report
            .clone()
            .expect("initial session summary");
        let first_receipt = first
            .present_report
            .project_runtime_bind_receipt
            .as_ref()
            .expect("initial bind receipt");
        assert_eq!(first_summary.session_id, first_receipt.session_id);
        assert_eq!(first_summary.frame_index, 1);
        assert_eq!(first_summary.stages.len(), 1);
        assert_eq!(
            first_summary.stages[0].stage,
            engine_runtime::project_runtime_session::ProjectRuntimeSessionStage::FixedUpdate
        );

        let mut instance = first.instance.expect("first runtime instance");
        let pause = instance.pause();
        let paused_tick = instance.tick_next_descriptor_frame();
        assert!(!pause.runtime_advanced);
        assert!(!paused_tick.runtime_advanced);
        assert_eq!(
            pause.project_runtime_session_report,
            Some(first_summary.clone())
        );
        assert_eq!(
            paused_tick.project_runtime_session_report,
            Some(first_summary.clone())
        );

        let step = instance.step_next_frame();
        let step_summary = step
            .project_runtime_session_report
            .as_ref()
            .expect("step session summary");
        assert!(step.runtime_advanced);
        assert_eq!(step_summary.frame_index, 2);
        assert_eq!(step_summary.stages.len(), 1);
        assert_eq!(
            step_summary.stages[0].stage,
            engine_runtime::project_runtime_session::ProjectRuntimeSessionStage::FixedUpdate
        );
        assert_eq!(step_summary.stages[0].action_count, 0);

        let stopped = instance.stop();
        assert_eq!(stopped.control_state, EditorRuntimePlayState::Stopped);
        assert_eq!(
            stopped
                .project_runtime_session_report
                .as_ref()
                .expect("stopped session summary")
                .frame_index,
            2
        );

        let second = EditorRuntimePlayInstance::start(EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "editor-session-second".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        });
        let second_surface_id = second
            .frame
            .as_ref()
            .expect("second Runtime frame")
            .texture_id
            .clone();
        let second_summary = second
            .present_report
            .project_runtime_session_report
            .as_ref()
            .expect("second session summary");
        assert_eq!(second.present_report.session_id, "editor-session-second");
        assert_ne!(first_surface_id, second_surface_id);
        assert_eq!(second_summary.frame_index, 1);
        assert_eq!(second_summary.session_id, first_summary.session_id);
    }

    #[test]
    fn editor_gameview_project_observation_projection_is_latest_and_clears_on_stop() {
        let root = temp_root("project-observation-projection");
        let package_dir = write_runtime_package_with_observation_contract(&root, "runtime-package");
        let output = EditorRuntimePlayInstance::start(EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "editor-observation-session".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        });

        assert!(matches!(
            output.present_report.project_observation_state,
            Some(ProjectRuntimeObservationState::ContractViolated {
                runtime_frame: 1,
                ..
            })
        ));
        let mut instance = output.instance.expect("runtime instance");
        let paused = instance.pause();
        assert_eq!(
            paused
                .project_observation_state
                .as_ref()
                .and_then(ProjectRuntimeObservationState::runtime_frame),
            Some(1)
        );
        let stopped = instance.stop();
        assert!(stopped.project_observation_state.is_none());
    }

    #[test]
    fn editor_gameview_aui_feedback_pointer_down_is_same_frame_and_filters_gameplay() {
        let root = temp_root("input-aui-filter");
        let package_dir =
            write_runtime_package_with_aui(&root, "runtime-package", aui_button_document());
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-input-aui-filter".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };
        let output = EditorRuntimePlayInstance::start(request);
        let mut instance = output.instance.expect("runtime instance");
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "session-input-aui-filter".to_string(),
            target_id: "game-view".to_string(),
            target_extent: GameViewTargetSpec::default().extent,
            display_rect: GameViewRect::from_extent(GameViewTargetSpec::default().extent),
            scale_policy: GameViewTargetSpec::default().scale_policy,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![
                engine_runtime::game_view_presentation::CanvasReferenceFact::new("main", 800, 600),
            ],
        })
        .expect("default GameView presentation");
        let point = presentation
            .reference_to_target(
                "main",
                engine_runtime::game_view_presentation::GameViewPoint::new(150.0, 130.0),
            )
            .expect("button point maps to target");
        let runtime_frame = pointer_down_frame(2, point.x, point.y);

        let report = instance.tick_next_descriptor_frame_with_runtime_input(runtime_frame);

        let frame = report.last_frame.as_ref().expect("last frame");
        assert_eq!(
            frame.input_bridge_status,
            "runtime_input_frame_filtered_by_aui"
        );
        assert_eq!(frame.runtime_input_event_count, 1);
        assert_eq!(frame.filtered_runtime_input_event_count, 0);
        assert_eq!(frame.aui_consumed_event_count, 1);
        assert_eq!(frame.aui_feedback_override_count, 1);
        assert!(frame
            .aui_feedback_profile_ids
            .iter()
            .any(|profile| profile == engine_runtime::aui::AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID));
        assert!(!frame
            .gameplay_action_ids
            .iter()
            .any(|action_id| action_id == "action.fire"));
        assert_eq!(report.input_bridge_status, frame.input_bridge_status);
        assert_eq!(report.aui_consumed_event_count, 1);
    }

    #[test]
    fn editor_gameview_immediate_aui_input_does_not_tick_runtime() {
        let root = temp_root("immediate-input-aui");
        let package_dir =
            write_runtime_package_with_aui(&root, "runtime-package", aui_button_document());
        let output = EditorRuntimePlayInstance::start(EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-immediate-input-aui".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        });
        let mut instance = output.instance.expect("runtime instance");
        let point = button_target_point(&instance, "session-immediate-input-aui");
        let initial_frame = instance.last_frame.clone().expect("initial frame");

        let pressed = instance.route_aui_input_immediately(pointer_down_frame(2, point.x, point.y));
        let activated = instance.route_aui_input_immediately(pointer_up_frame(3, point.x, point.y));

        assert_eq!(pressed.frame_count, 1);
        assert!(!pressed.runtime_advanced);
        assert_eq!(
            pressed
                .last_frame
                .as_ref()
                .expect("pressed frame")
                .aui_feedback_override_count,
            1
        );
        let pressed_frame = pressed.last_frame.as_ref().expect("pressed frame");
        assert_eq!(pressed_frame.frame_index, initial_frame.frame_index);
        assert_eq!(pressed_frame.frame_hash, initial_frame.frame_hash);
        assert_ne!(
            pressed_frame.aui_presentation_identity,
            initial_frame.aui_presentation_identity
        );
        assert_eq!(activated.frame_count, 1);
        assert!(!activated.runtime_advanced);
        let stage = activated
            .project_runtime_session_report
            .as_ref()
            .and_then(|report| report.stages.first())
            .expect("immediate action dispatch stage");
        assert_eq!(stage.action_count, 1);
        assert_eq!(instance.frame_count, 1);
    }

    #[test]
    fn editor_gameview_immediate_input_preserves_unconsumed_gameplay() {
        let root = temp_root("immediate-input-gameplay");
        let package_dir =
            write_runtime_package_with_aui(&root, "runtime-package", aui_button_document());
        let output = EditorRuntimePlayInstance::start(EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-immediate-input-gameplay".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        });
        let mut instance = output.instance.expect("runtime instance");

        let immediate = instance.route_aui_input_immediately(pointer_down_frame(2, 700.0, 520.0));
        assert_eq!(immediate.frame_count, 1);
        assert!(!immediate.runtime_advanced);
        assert_eq!(immediate.gameplay_action_count, 0);

        let next = instance.tick_next_descriptor_frame();
        assert_eq!(next.frame_count, 2);
        assert!(next.runtime_advanced);
        assert_eq!(next.filtered_runtime_input_event_count, 1);
        assert!(next
            .gameplay_action_ids
            .iter()
            .any(|action_id| action_id == "action.fire"));
        let following = instance.tick_next_descriptor_frame();
        assert!(!following
            .gameplay_action_ids
            .iter()
            .any(|action_id| action_id == "action.fire"));
    }

    #[test]
    fn editor_gameview_cancel_discards_pending_gameplay_input() {
        let root = temp_root("immediate-input-cancel");
        let package_dir =
            write_runtime_package_with_aui(&root, "runtime-package", aui_button_document());
        let output = EditorRuntimePlayInstance::start(EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-immediate-input-cancel".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        });
        let mut instance = output.instance.expect("runtime instance");

        let immediate = instance.route_aui_input_immediately(pointer_down_frame(2, 700.0, 520.0));
        assert_eq!(immediate.filtered_runtime_input_event_count, 1);
        instance.cancel_pending_game_view_input();

        let next = instance.tick_next_descriptor_frame();
        assert!(!next
            .gameplay_action_ids
            .iter()
            .any(|action_id| action_id == "action.fire"));
    }

    #[test]
    fn editor_gameview_portrait_target_maps_aui_reference_input_before_gameplay() {
        let root = temp_root("input-aui-portrait-filter");
        let package_dir =
            write_runtime_package_with_aui(&root, "runtime-package", aui_button_document());
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-input-aui-portrait-filter".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };
        let target = GameViewTargetSpec::portrait_720x1280();
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: request.session_id.clone(),
            target_id: "game-view".to_string(),
            target_extent: target.extent,
            display_rect: GameViewRect::from_extent(target.extent),
            scale_policy: target.scale_policy,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![
                engine_runtime::game_view_presentation::CanvasReferenceFact::new("main", 800, 600),
            ],
        })
        .expect("portrait AUI presentation");
        let point = presentation
            .reference_to_target(
                "main",
                engine_runtime::game_view_presentation::GameViewPoint::new(150.0, 130.0),
            )
            .expect("button center maps to target");
        let linked_modules = LinkedProjectRuntimeSet::explicit_empty();
        let output = EditorRuntimePlayInstance::start_with_linked_modules_and_target(
            request,
            &linked_modules,
            target,
        );
        let mut instance = output.instance.expect("runtime instance");

        let report = instance
            .tick_next_descriptor_frame_with_runtime_input(pointer_down_frame(2, point.x, point.y));

        let frame = report.last_frame.as_ref().expect("last frame");
        assert_eq!((frame.width, frame.height), (720, 1280));
        assert_eq!(frame.aui_consumed_event_count, 1);
        assert_eq!(frame.filtered_runtime_input_event_count, 0);
        assert!(!frame
            .gameplay_action_ids
            .iter()
            .any(|action_id| action_id == "action.fire"));
    }

    #[test]
    fn editor_gameview_keeps_bounded_action_target_snapshot_for_authority() {
        let root = temp_root("aui-action-targets");
        let package_dir =
            write_runtime_package_with_aui(&root, "runtime-package", aui_button_document());
        let output = EditorRuntimePlayInstance::start(EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-aui-action-targets".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        });
        let instance = output.instance.expect("runtime instance");

        let targets = instance.last_aui_action_targets();
        assert_eq!(targets.len(), 1);
        let target = &targets[0];
        assert_eq!(target.node_id, "fire_button");
        assert_eq!(target.action_id, "ui.fire_button");
        assert!(target.visible);
        assert!(target.interactable);
        assert_eq!(target.computed_rect.x, 100.0);
        assert_eq!(target.computed_rect.y, 100.0);
        assert_eq!(target.reference_width, 800);
        assert_eq!(target.reference_height, 600);
        assert!(target.actionable_rect().is_some());
    }

    #[test]
    fn editor_gameview_input_tick_allows_gameplay_when_aui_misses() {
        let root = temp_root("input-aui-miss");
        let package_dir =
            write_runtime_package_with_aui(&root, "runtime-package", aui_button_document());
        let request = EditorRuntimePlayRequest {
            schema_version: EDITOR_RUNTIME_PLAY_REQUEST_SCHEMA_VERSION.to_string(),
            session_id: "session-input-aui-miss".to_string(),
            project_root: root,
            runtime_package_path: package_dir,
            scene_ref: Some("scene-main".to_string()),
            run_profile: Some("editor-gameview".to_string()),
            frame_limit: 1,
            requested_by: "Automation".to_string(),
            preview_package_report_path: None,
        };
        let output = EditorRuntimePlayInstance::start(request);
        let mut instance = output.instance.expect("runtime instance");
        let runtime_frame = pointer_down_frame(2, 700.0, 520.0);

        let report = instance.tick_next_descriptor_frame_with_runtime_input(runtime_frame);

        let frame = report.last_frame.as_ref().expect("last frame");
        assert_eq!(frame.input_bridge_status, "runtime_input_frame");
        assert_eq!(frame.runtime_input_event_count, 1);
        assert_eq!(frame.filtered_runtime_input_event_count, 1);
        assert_eq!(frame.aui_consumed_event_count, 0);
        assert!(frame
            .gameplay_action_ids
            .iter()
            .any(|action_id| action_id == "action.fire"));
        assert_eq!(report.input_bridge_status, frame.input_bridge_status);
        assert_eq!(report.gameplay_action_count, frame.gameplay_action_count);
    }

    #[test]
    fn editor_gameview_play_runner_reports_running_session() {
        let root = temp_root("runner");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let mut request = PlaySessionRequest::windowed_user_run(package_dir);
        request.project_root = root;
        request.session_id = "session-runner".to_string();
        request.game_view_target = GameViewTargetSpec::portrait_720x1280();
        let runner = EditorGameViewPlayRunner::new();

        let report = runner.run_play_session(request);

        assert_eq!(
            report.runner_kind.as_deref(),
            Some("editor_in_process_gameview")
        );
        assert_eq!(report.game_view_frame_count, Some(3));
        assert!(report.game_view_last_frame_hash.is_some());
        let output = runner.take_last_output().expect("runner output");
        let frame = output.frame.expect("portrait GameView frame");
        assert_eq!((frame.width, frame.height), (720, 1280));
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("editor-gameview-play-{name}-{stamp}"))
    }

    fn write_minimal_runtime_package(root: &Path, name: &str) -> PathBuf {
        let package_dir = root.join(name);
        fs::create_dir_all(package_dir.join("scenes")).unwrap();
        fs::create_dir_all(package_dir.join("assets")).unwrap();
        fs::create_dir_all(package_dir.join("rules")).unwrap();
        fs::create_dir_all(package_dir.join("input")).unwrap();
        fs::write(
            package_dir.join("manifest.json"),
            r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "project-editor-gameview-test",
    "name": "Editor GameView Test",
    "version": "0.0.1",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 1 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "none" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.default", "mappingCount": 1 },
  "contentHash": "testhash"
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("scenes").join("scene-main.json"),
            r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#101010",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
    "id": "entity-player",
    "name": "Player",
    "kind": "actor",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },
    "mesh": {
      "primitive": "quad",
      "color": "#ffffff",
      "visible": true,
      "layer": "default"
    }
  }]
}"##,
        )
        .unwrap();
        fs::write(
            package_dir.join("assets").join("asset-manifest.json"),
            r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [{
    "id": "scene-main",
    "name": "Main",
    "type": "scene",
    "source": "scenes/scene-main.json",
    "state": "available",
    "bundleId": "startup"
  }],
  "runtimeAssetIndex": [{
    "assetGuid": "scene-main",
    "assetId": "scene-main",
    "assetType": "scene",
    "subAssetId": null,
    "version": "1",
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "loaderKind": "scene",
    "dependencies": [],
    "hash": null,
    "size": null,
    "flags": ["test"]
  }],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [{
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "path": "scenes/scene-main.json",
    "offset": null,
    "size": null,
    "compression": "none",
    "hash": null
  }],
  "dependencyTable": []
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("rules").join("rule-manifest.json"),
            r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "none",
  "rules": [],
  "modules": []
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input-manifest.json"),
            r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.default",
  "mappings": [{ "id": "input.default", "path": "input/input.default.json", "enabled": true }]
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input.default.json"),
            serde_json::to_string_pretty(&InputMappingAsset::gameplay_default()).unwrap(),
        )
        .unwrap();
        package_dir
    }

    #[derive(Debug, Clone, Copy)]
    enum SpriteTextureFixture {
        Ready,
        MissingPixelPayload,
    }

    fn write_runtime_package_with_sprite_texture(
        root: &Path,
        name: &str,
        fixture: SpriteTextureFixture,
    ) -> PathBuf {
        let package_dir = write_minimal_runtime_package(root, name);
        let scene_path = package_dir.join("scenes").join("scene-main.json");
        let mut scene: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&scene_path).unwrap()).unwrap();
        scene["entities"][0]["mesh"] = serde_json::Value::Null;
        scene["entities"][0]["spriteRenderer2D"] = json!({
            "spriteRef": { "id": "texture-sprite", "type": "texture" },
            "materialRef": null,
            "color": [1.0, 1.0, 1.0, 1.0],
            "flipX": false,
            "flipY": false,
            "sortingLayer": 0,
            "orderInLayer": 0,
            "sortZ": 0,
            "visible": true
        });
        fs::write(&scene_path, serde_json::to_vec_pretty(&scene).unwrap()).unwrap();

        let cooked_dir = package_dir.join("cooked").join("textures");
        fs::create_dir_all(&cooked_dir).unwrap();
        fs::write(
            cooked_dir.join("texture-sprite.texture.json"),
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": "cooked-texture.v1",
                "assetId": "texture-sprite",
                "cookedAssetId": "cooked-texture-sprite",
                "sourceHash": "sha256:test-texture-sprite",
                "width": 1,
                "height": 1,
                "format": "rgba8UnormSrgb",
                "colorSpace": "srgb",
                "mipCount": 1,
                "byteLength": 4,
                "pixelDataPath": "cooked/textures/texture-sprite.rgba8",
                "sampler": "linearClamp"
            }))
            .unwrap(),
        )
        .unwrap();
        if matches!(fixture, SpriteTextureFixture::Ready) {
            fs::write(cooked_dir.join("texture-sprite.rgba8"), [255, 64, 32, 255]).unwrap();
        }

        let asset_manifest_path = package_dir.join("assets").join("asset-manifest.json");
        let mut asset_manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&asset_manifest_path).unwrap()).unwrap();
        asset_manifest["assets"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "texture-sprite",
                "name": "Sprite Texture",
                "type": "texture",
                "source": "cooked/textures/texture-sprite.texture.json",
                "state": "available",
                "bundleId": "startup"
            }));
        asset_manifest["runtimeAssetIndex"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "assetGuid": "texture-sprite",
                "assetId": "texture-sprite",
                "assetType": "texture",
                "subAssetId": null,
                "version": "1",
                "cookedAssetId": "cooked-texture-sprite",
                "bundleId": "startup",
                "loaderKind": "texture",
                "dependencies": [],
                "hash": null,
                "size": 4,
                "flags": ["test"]
            }));
        asset_manifest["cookedAssetTable"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "cookedAssetId": "cooked-texture-sprite",
                "bundleId": "startup",
                "path": "cooked/textures/texture-sprite.texture.json",
                "offset": null,
                "size": 4,
                "compression": "none",
                "hash": null
            }));
        fs::write(
            &asset_manifest_path,
            serde_json::to_vec_pretty(&asset_manifest).unwrap(),
        )
        .unwrap();

        let manifest_path = package_dir.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["assets"]["assetCount"] = json!(2);
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        package_dir
    }

    fn write_runtime_package_with_aui(root: &Path, name: &str, document: AuiDocument) -> PathBuf {
        let package_dir = write_minimal_runtime_package(root, name);
        fs::create_dir_all(package_dir.join("aui").join("documents")).unwrap();
        fs::write(
            package_dir
                .join("aui")
                .join("documents")
                .join("hud.aui.json"),
            serde_json::to_string_pretty(&document).unwrap(),
        )
        .unwrap();
        fs::write(
            package_dir.join("aui").join("aui-manifest.json"),
            serde_json::to_string_pretty(&RuntimeAuiManifest {
                schema_version: RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.to_string(),
                documents: vec![RuntimeAuiManifestEntry {
                    document_id: document.document_id.clone(),
                    path: "aui/documents/hud.aui.json".to_string(),
                    canvas_count: document.canvases.len(),
                    node_count: document.nodes.len(),
                    binding_count: document
                        .nodes
                        .iter()
                        .map(|node| node.binding_refs.len())
                        .sum(),
                    action_count: document
                        .nodes
                        .iter()
                        .map(|node| node.action_refs.len())
                        .sum(),
                    asset_refs: Vec::new(),
                }],
            })
            .unwrap(),
        )
        .unwrap();

        let manifest_path = package_dir.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["aui"] = json!({
            "path": "aui/aui-manifest.json",
            "documentCount": 1
        });
        fs::write(
            manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        package_dir
    }

    fn write_runtime_package_with_observation_contract(root: &Path, name: &str) -> PathBuf {
        let package_dir = write_minimal_runtime_package(root, name);
        let contract = ProjectObservationContract {
            schema_version: PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION.to_string(),
            contract_id: "test.runtime-observations".to_string(),
            observations: vec![ProjectObservationEntry {
                path: "test.round".to_string(),
                value_type: ProjectObservationType::Integer,
                description: "Current test round".to_string(),
                allowed_values: None,
            }],
        }
        .cook()
        .unwrap();
        let manifest_path = package_dir.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["observationContract"] = serde_json::to_value(contract).unwrap();
        fs::write(
            manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        package_dir
    }

    fn aui_button_document() -> AuiDocument {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["fire_button"]);
        let button = AuiNode::new(
            "fire_button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(100.0, 100.0, 240.0, 80.0),
        )
        .with_parent("root")
        .with_interactable(true)
        .with_action(AuiActionRef::click("ui.fire_button"));
        AuiDocument::new(
            "hud",
            vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
            vec![root, button],
        )
    }

    fn pointer_down_frame(frame_id: u64, x: f32, y: f32) -> RuntimeInputFrame {
        let mut frame = RuntimeInputFrame::new(frame_id, "game-view");
        frame.pointer_position = Some(PointerPosition { x, y });
        frame.events.push(RuntimeInputEvent::PointerDown {
            x,
            y,
            button: RuntimePointerButton::Primary,
        });
        frame
    }

    fn pointer_up_frame(frame_id: u64, x: f32, y: f32) -> RuntimeInputFrame {
        let mut frame = RuntimeInputFrame::new(frame_id, "game-view");
        frame.pointer_position = Some(PointerPosition { x, y });
        frame.events.push(RuntimeInputEvent::PointerUp {
            x,
            y,
            button: RuntimePointerButton::Primary,
        });
        frame
    }

    fn button_target_point(
        instance: &EditorRuntimePlayInstance,
        session_id: &str,
    ) -> engine_runtime::game_view_presentation::GameViewPoint {
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: session_id.to_string(),
            target_id: "game-view".to_string(),
            target_extent: instance.game_view_target.extent,
            display_rect: GameViewRect::from_extent(instance.game_view_target.extent),
            scale_policy: instance.game_view_target.scale_policy,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![
                engine_runtime::game_view_presentation::CanvasReferenceFact::new("main", 800, 600),
            ],
        })
        .expect("GameView presentation");
        presentation
            .reference_to_target(
                "main",
                engine_runtime::game_view_presentation::GameViewPoint::new(150.0, 130.0),
            )
            .expect("button point maps to target")
    }
}
