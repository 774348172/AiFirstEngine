#[cfg(feature = "real-window")]
use engine_input::RawInputValue;
use engine_input::{
    ActionSnapshot, InputDeviceState, InputDeviceStateReport, InputMappingAsset, InputResolver,
    InputTraceSummary, RawInputEvent,
};
use engine_runtime::aui::{
    AuiInteractionConfig, AuiInteractionProductizationReport, AuiInteractionResult,
    AuiInteractionState, AuiInteractionSystem, AuiLayoutEngine,
    AuiRuntimeNavigationScreenFlowTextEntryProductizationReport, AuiRuntimePresentOutput,
    AuiRuntimePresentStatus, AuiRuntimePresenter, AuiSnapshotSource, ProjectUiStateReportMode,
    ProjectUiStateSnapshotCache, ProjectUiStateSnapshotCacheResult, ProjectUiStateSnapshotProducer,
};
use engine_runtime::aui_control_feedback::{
    presentation_delta_us_from_seconds, AuiControlFeedbackState,
};
use engine_runtime::diagnostics::{DiagnosticSeverity, RuntimeDiagnostic};
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::frame_loop::RuntimeFrameContext;
#[cfg(feature = "real-window")]
use engine_runtime::game_view_presentation::{
    CanvasReferenceFact, GameViewExtent, GameViewPoint, GameViewPresentationModule,
    GameViewPresentationSpec, GameViewRect,
};
use engine_runtime::game_view_presentation::{GameViewTargetSpec, ResolvedGameViewPresentation};
use engine_runtime::project_runtime_module::{
    BoundProjectRuntimeParts, LinkedProjectRuntimeSet, ProjectRuntimeBindReceipt,
    ProjectRuntimeBootstrap, ProjectRuntimeError,
};
use engine_runtime::project_runtime_session::{
    ProjectRuntimeSessionFrameReport, ProjectRuntimeSessionReportLevel,
};
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package::RuntimePackage;
use engine_runtime::runtime_renderer::RenderTarget;
use engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator;
use engine_runtime::runtime_texture::{RuntimeTextureBindingContext, RuntimeTextureUploadRegistry};
use engine_runtime::sprite2d_render_pipeline::Sprite2DTextureBindingContext;
use engine_runtime::windowed_player::{
    WindowedPlayerFramePerformanceSummary, WindowedPlayerGameplayTraceRecord,
    WindowedPlayerGameplayTraceSummary, WindowedPlayerRuntimeReportLevel,
};
use engine_runtime::world::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[cfg(any(test, feature = "real-window"))]
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

pub const NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION: &str = "native-window-host-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativePrimaryTouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

pub fn primary_touch_raw_event(
    frame_id: u64,
    window_id: impl Into<String>,
    touch_id: u64,
    phase: NativePrimaryTouchPhase,
    x: f32,
    y: f32,
) -> RawInputEvent {
    let window_id = window_id.into();
    match phase {
        NativePrimaryTouchPhase::Started => {
            RawInputEvent::touch_start(frame_id, window_id, touch_id, x, y)
        }
        NativePrimaryTouchPhase::Moved => {
            RawInputEvent::touch_move(frame_id, window_id, touch_id, x, y)
        }
        NativePrimaryTouchPhase::Ended => {
            RawInputEvent::touch_end(frame_id, window_id, touch_id, x, y)
        }
        NativePrimaryTouchPhase::Cancelled => {
            RawInputEvent::touch_cancel(frame_id, window_id, touch_id, x, y)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePlayerLifecycleState {
    pub resumed: bool,
    pub surface_generation: u64,
    pub gameplay_session_generation: u64,
}

impl Default for NativePlayerLifecycleState {
    fn default() -> Self {
        Self {
            resumed: false,
            surface_generation: 0,
            gameplay_session_generation: 1,
        }
    }
}

impl NativePlayerLifecycleState {
    pub fn resume(&mut self) {
        if !self.resumed {
            self.resumed = true;
            self.surface_generation += 1;
        }
    }

    pub fn suspend(&mut self) {
        self.resumed = false;
    }

    pub fn should_present(&self) -> bool {
        self.resumed
    }
}

struct NativePlayerRuntimeComposition {
    host: EngineHostLoop,
    ui_state_producer: Box<dyn ProjectUiStateSnapshotProducer>,
    input_mapping: InputMappingAsset,
    receipt: ProjectRuntimeBindReceipt,
}

impl NativePlayerRuntimeComposition {
    fn from_bound(scene_id: impl Into<String>, parts: BoundProjectRuntimeParts) -> Self {
        let mut host = EngineHostLoop::with_project_runtime_session(
            scene_id,
            parts.project_logic,
            parts.project_runtime_session,
        );
        host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
        Self {
            host,
            ui_state_producer: parts.ui_state_producer,
            input_mapping: parts.default_input_mapping,
            receipt: parts.receipt,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePlayerWindowConfig {
    pub window_id: String,
    pub target_id: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub surface_format: String,
    pub present_mode: String,
}

impl Default for NativePlayerWindowConfig {
    fn default() -> Self {
        Self {
            window_id: "main-window".to_string(),
            target_id: "main-surface".to_string(),
            title: "AI First Engine Player".to_string(),
            width: 1280,
            height: 720,
            resizable: true,
            surface_format: "Bgra8UnormSrgb".to_string(),
            present_mode: "Fifo".to_string(),
        }
    }
}

impl NativePlayerWindowConfig {
    pub fn surface_target(&self) -> RenderTarget {
        RenderTarget::surface(self.target_id.clone(), self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativePlayerWindowRunMode {
    HeadlessSurfaceGate,
    Windowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePlayerWindowRunRequest {
    pub runtime_package_path: PathBuf,
    pub frame_limit: u64,
    pub mode: NativePlayerWindowRunMode,
    pub config: NativePlayerWindowConfig,
    pub game_view_target: GameViewTargetSpec,
    pub screenshot: NativeWindowScreenshotRequest,
    pub input_script: Option<NativePlayerInputScript>,
    pub runtime_report_level: WindowedPlayerRuntimeReportLevel,
    pub performance_warmup_frames: u64,
    pub performance_sample_frames: u64,
}

impl NativePlayerWindowRunRequest {
    pub fn headless_surface_gate(runtime_package_path: impl Into<PathBuf>) -> Self {
        Self {
            runtime_package_path: runtime_package_path.into(),
            frame_limit: 1,
            mode: NativePlayerWindowRunMode::HeadlessSurfaceGate,
            config: NativePlayerWindowConfig::default(),
            game_view_target: GameViewTargetSpec::default(),
            screenshot: NativeWindowScreenshotRequest::disabled(),
            input_script: None,
            runtime_report_level: WindowedPlayerRuntimeReportLevel::Off,
            performance_warmup_frames: 0,
            performance_sample_frames: 0,
        }
    }

    pub fn windowed(runtime_package_path: impl Into<PathBuf>) -> Self {
        Self {
            runtime_package_path: runtime_package_path.into(),
            frame_limit: 1,
            mode: NativePlayerWindowRunMode::Windowed,
            config: NativePlayerWindowConfig::default(),
            game_view_target: GameViewTargetSpec::default(),
            screenshot: NativeWindowScreenshotRequest::disabled(),
            input_script: None,
            runtime_report_level: WindowedPlayerRuntimeReportLevel::Off,
            performance_warmup_frames: 0,
            performance_sample_frames: 0,
        }
    }

    pub fn with_screenshot(mut self, path: impl Into<PathBuf>) -> Self {
        self.screenshot = NativeWindowScreenshotRequest::enabled(path);
        self
    }

    pub fn with_game_view_target(mut self, target: GameViewTargetSpec) -> Self {
        self.config.width = target.extent.width;
        self.config.height = target.extent.height;
        self.game_view_target = target;
        self
    }

    pub fn with_input_script(mut self, input_script: NativePlayerInputScript) -> Self {
        self.input_script = Some(input_script);
        self
    }

    pub fn with_runtime_report_level(
        mut self,
        report_level: WindowedPlayerRuntimeReportLevel,
    ) -> Self {
        self.runtime_report_level = report_level;
        self
    }

    pub fn with_frame_performance_sample(mut self, warmup_frames: u64, sample_frames: u64) -> Self {
        self.performance_warmup_frames = warmup_frames;
        self.performance_sample_frames = sample_frames;
        self
    }
}

#[cfg(any(test, feature = "real-window"))]
fn windowed_session_has_more_frames(frames_completed: u64, frame_limit: u64) -> bool {
    frames_completed < frame_limit
}

pub const NATIVE_PLAYER_INPUT_SCRIPT_SCHEMA_VERSION: &str = "native-player-input-script.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePlayerInputScript {
    pub schema_version: String,
    pub script_id: String,
    pub frames: Vec<NativePlayerInputScriptFrame>,
}

impl NativePlayerInputScript {
    pub fn new(script_id: impl Into<String>, frames: Vec<NativePlayerInputScriptFrame>) -> Self {
        Self {
            schema_version: NATIVE_PLAYER_INPUT_SCRIPT_SCHEMA_VERSION.to_string(),
            script_id: script_id.into(),
            frames,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != NATIVE_PLAYER_INPUT_SCRIPT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported input script schema: {}",
                self.schema_version
            ));
        }
        if self.script_id.trim().is_empty() {
            return Err("input script id must not be empty".to_string());
        }
        if self
            .frames
            .windows(2)
            .any(|pair| pair[0].frame_index >= pair[1].frame_index)
        {
            return Err("input script frames must be strictly increasing".to_string());
        }
        if self.frames.iter().any(|frame| frame.frame_index == 0) {
            return Err("input script frame indexes are one-based".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePlayerInputScriptFrame {
    pub frame_index: u64,
    #[serde(default)]
    pub key_down: Vec<String>,
    #[serde(default)]
    pub key_up: Vec<String>,
}

impl NativePlayerInputScriptFrame {
    pub fn keys(
        frame_index: u64,
        key_down: impl IntoIterator<Item = impl Into<String>>,
        key_up: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            frame_index,
            key_down: key_down.into_iter().map(Into::into).collect(),
            key_up: key_up.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeWindowHostDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowHostDiagnostic {
    pub severity: NativeWindowHostDiagnosticSeverity,
    pub code: String,
    pub layer: String,
    pub message: String,
    pub path: Option<String>,
}

impl NativeWindowHostDiagnostic {
    pub fn error(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: NativeWindowHostDiagnosticSeverity::Error,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeWindowPresentStatus {
    Presented,
    NotPresented,
    FeatureNotEnabled,
    EnvironmentBlocked,
    PackageFailed,
    SceneFailed,
    SurfaceFailed,
    RhiFailed,
}

impl NativeWindowPresentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Presented => "presented",
            Self::NotPresented => "not_presented",
            Self::FeatureNotEnabled => "feature_not_enabled",
            Self::EnvironmentBlocked => "environment_blocked",
            Self::PackageFailed => "package_failed",
            Self::SceneFailed => "scene_failed",
            Self::SurfaceFailed => "surface_failed",
            Self::RhiFailed => "rhi_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowState {
    pub created: bool,
    pub width: u32,
    pub height: u32,
    pub close_requested: bool,
}

impl NativeWindowState {
    fn absent(config: &NativePlayerWindowConfig) -> Self {
        Self {
            created: false,
            width: config.width,
            height: config.height,
            close_requested: false,
        }
    }

    fn created(config: &NativePlayerWindowConfig) -> Self {
        Self {
            created: true,
            width: config.width,
            height: config.height,
            close_requested: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSurfaceState {
    pub created: bool,
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub present_mode: String,
    pub acquired_frame_count: u64,
    pub presented_frame_count: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowScreenshotRequest {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub frame_index: Option<u64>,
}

impl NativeWindowScreenshotRequest {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            path: None,
            frame_index: None,
        }
    }

    pub fn enabled(path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            path: Some(path.into()),
            frame_index: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeWindowScreenshotStatus {
    NotRequested,
    Captured,
    Unsupported,
    ReadbackFailed,
    WriteFailed,
}

impl NativeWindowScreenshotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::Captured => "captured",
            Self::Unsupported => "unsupported",
            Self::ReadbackFailed => "readback_failed",
            Self::WriteFailed => "write_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowScreenshotReport {
    pub requested: bool,
    pub status: NativeWindowScreenshotStatus,
    pub path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub byte_size: Option<u64>,
}

impl NativeWindowScreenshotReport {
    fn from_request(
        request: &NativeWindowScreenshotRequest,
        config: &NativePlayerWindowConfig,
    ) -> Self {
        Self {
            requested: request.enabled,
            status: if request.enabled {
                NativeWindowScreenshotStatus::Unsupported
            } else {
                NativeWindowScreenshotStatus::NotRequested
            },
            path: request.path.as_ref().map(|path| path.display().to_string()),
            width: config.width,
            height: config.height,
            format: "png".to_string(),
            byte_size: None,
        }
    }

    #[cfg(feature = "real-window")]
    fn mark_captured(&mut self, path: &std::path::Path, width: u32, height: u32, byte_size: u64) {
        self.status = NativeWindowScreenshotStatus::Captured;
        self.path = Some(path.display().to_string());
        self.width = width;
        self.height = height;
        self.byte_size = Some(byte_size);
    }

    #[cfg(feature = "real-window")]
    fn mark_failed(&mut self, status: NativeWindowScreenshotStatus) {
        self.status = status;
        self.byte_size = None;
    }
}

impl NativeSurfaceState {
    fn absent(config: &NativePlayerWindowConfig) -> Self {
        Self {
            created: false,
            configured: false,
            width: config.width,
            height: config.height,
            format: config.surface_format.clone(),
            present_mode: config.present_mode.clone(),
            acquired_frame_count: 0,
            presented_frame_count: 0,
            last_error: None,
        }
    }

    fn headless_presented(config: &NativePlayerWindowConfig, frames: u64) -> Self {
        Self {
            created: true,
            configured: true,
            width: config.width,
            height: config.height,
            format: config.surface_format.clone(),
            present_mode: config.present_mode.clone(),
            acquired_frame_count: frames,
            presented_frame_count: frames,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInputSummary {
    pub backend: String,
    pub platform: String,
    pub mapping_source: String,
    pub mapping_id: Option<String>,
    pub mapping_status: String,
    pub mapping_path: Option<String>,
    pub focused: bool,
    pub raw_event_count: usize,
    pub runtime_event_count: usize,
    pub resolved_action_count: usize,
    pub last_action_ids: Vec<String>,
    pub pressed_key_count: usize,
    pub pressed_mouse_button_count: usize,
    pub pointer_position: Option<engine_input::PointerPosition>,
}

impl NativeInputSummary {
    fn empty() -> Self {
        Self {
            backend: "none".to_string(),
            platform: "unknown".to_string(),
            mapping_source: "none".to_string(),
            mapping_id: None,
            mapping_status: "not_loaded".to_string(),
            mapping_path: None,
            focused: true,
            raw_event_count: 0,
            runtime_event_count: 0,
            resolved_action_count: 0,
            last_action_ids: Vec::new(),
            pressed_key_count: 0,
            pressed_mouse_button_count: 0,
            pointer_position: None,
        }
    }

    fn from_snapshot(
        backend: impl Into<String>,
        platform: impl Into<String>,
        raw_event_count: usize,
        runtime_event_count: usize,
        device_report: InputDeviceStateReport,
        snapshot: &ActionSnapshot,
    ) -> Self {
        Self {
            backend: backend.into(),
            platform: platform.into(),
            mapping_source: "unknown".to_string(),
            mapping_id: None,
            mapping_status: "unknown".to_string(),
            mapping_path: None,
            focused: device_report.focused,
            raw_event_count,
            runtime_event_count,
            resolved_action_count: snapshot.action_count(),
            last_action_ids: snapshot.action_ids(),
            pressed_key_count: device_report.pressed_key_count,
            pressed_mouse_button_count: device_report.pressed_mouse_button_count,
            pointer_position: device_report.pointer_position,
        }
    }

    fn with_mapping(
        mut self,
        source: impl Into<String>,
        mapping_id: impl Into<String>,
        status: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        self.mapping_source = source.into();
        self.mapping_id = Some(mapping_id.into());
        self.mapping_status = status.into();
        self.mapping_path = path;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowHostReport {
    pub schema_version: String,
    pub run_id: String,
    pub mode: NativePlayerWindowRunMode,
    pub runtime_package_path: String,
    pub frame_limit: u64,
    pub frames_completed: u64,
    pub package_status: String,
    pub scene_status: String,
    pub world_status: String,
    pub logic_status: String,
    pub render_status: String,
    pub rhi_status: String,
    pub input_status: String,
    pub window_status: String,
    pub surface_status: String,
    pub present_status: NativeWindowPresentStatus,
    pub window: NativeWindowState,
    pub surface: NativeSurfaceState,
    pub screenshot: NativeWindowScreenshotReport,
    pub input: NativeInputSummary,
    pub aui: NativeAuiPresentSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_performance_summary: Option<WindowedPlayerFramePerformanceSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gameplay_trace_summary: Option<WindowedPlayerGameplayTraceSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gameplay_trace_records: Vec<WindowedPlayerGameplayTraceRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_bind_receipt: Option<ProjectRuntimeBindReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    pub render_thread_report_schema: Option<String>,
    pub rhi_command_count: usize,
    pub exit_code: i32,
    pub diagnostics: Vec<NativeWindowHostDiagnostic>,
}

impl NativeWindowHostReport {
    fn base(request: &NativePlayerWindowRunRequest) -> Self {
        Self {
            schema_version: NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION.to_string(),
            run_id: "native-player-window-host".to_string(),
            mode: request.mode,
            runtime_package_path: request.runtime_package_path.display().to_string(),
            frame_limit: request.frame_limit,
            frames_completed: 0,
            package_status: "not_started".to_string(),
            scene_status: "not_started".to_string(),
            world_status: "not_started".to_string(),
            logic_status: "not_started".to_string(),
            render_status: "not_started".to_string(),
            rhi_status: "not_started".to_string(),
            input_status: "not_started".to_string(),
            window_status: "not_started".to_string(),
            surface_status: "not_started".to_string(),
            present_status: NativeWindowPresentStatus::NotPresented,
            window: NativeWindowState::absent(&request.config),
            surface: NativeSurfaceState::absent(&request.config),
            screenshot: NativeWindowScreenshotReport::from_request(
                &request.screenshot,
                &request.config,
            ),
            input: NativeInputSummary::empty(),
            aui: NativeAuiPresentSummary::empty(),
            frame_performance_summary: None,
            gameplay_trace_summary: (request.runtime_report_level
                != WindowedPlayerRuntimeReportLevel::Off)
                .then(|| WindowedPlayerGameplayTraceSummary {
                    report_level: request.runtime_report_level,
                    input_script_id: request
                        .input_script
                        .as_ref()
                        .map(|script| script.script_id.clone()),
                    ..WindowedPlayerGameplayTraceSummary::default()
                }),
            gameplay_trace_records: Vec::new(),
            project_runtime_bind_receipt: None,
            project_runtime_session_report: None,
            render_thread_report_schema: None,
            rhi_command_count: 0,
            exit_code: 1,
            diagnostics: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == NativeWindowHostDiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAuiPresentSummary {
    pub package_document_count: usize,
    pub loaded_document_count: usize,
    pub draw_item_count: usize,
    pub text_command_count: usize,
    pub aui_interaction_status: String,
    pub aui_input_consumed: bool,
    pub aui_action_count: usize,
    pub aui_drop_count: usize,
    pub aui_modal_blocking_status: String,
    pub aui_focus_trap_status: String,
    pub aui_scroll_status: String,
    pub aui_consumed_wheel_count: usize,
    pub aui_consumed_keyboard_count: usize,
    pub aui_consumed_event_count_by_kind: std::collections::BTreeMap<String, usize>,
    pub aui_scroll_offset_count: usize,
    pub aui_scroll_offset_applied: bool,
    pub aui_scroll_applied_node_count: usize,
    pub aui_clipped_node_count: usize,
    pub aui_clip_root_count: usize,
    pub aui_effective_clip_item_count: usize,
    pub aui_culled_draw_item_count: usize,
    pub aui_hit_test_clip_rejected_count: usize,
    pub aui_scrollbar_visible_count: usize,
    pub aui_scrollbar_thumb_drag_count: usize,
    pub aui_keyboard_navigation_event_count: usize,
    pub aui_focus_visible_scroll_count: usize,
    pub aui_submit_count: usize,
    pub aui_cancel_count: usize,
    pub aui_screen_stack_push_count: usize,
    pub aui_screen_stack_pop_count: usize,
    pub aui_active_screen_id: Option<String>,
    pub aui_default_focus_applied_count: usize,
    pub aui_focus_restore_count: usize,
    pub aui_text_edit_session_count: usize,
    pub aui_text_changed_count: usize,
    pub aui_text_submitted_count: usize,
    pub aui_text_cancelled_count: usize,
    pub aui_ime_preedit_count: usize,
    pub aui_ime_commit_count: usize,
    pub aui_ime_cancel_count: usize,
    pub aui_action_prompt_reported: bool,
    pub aui_ime_platform_coverage: String,
    pub aui_focusable_derived_from_interactable: bool,
    pub gameplay_input_filtered: bool,
    pub snapshot_frame_lag: u64,
    pub authoring_action_payload_deferred: bool,
    pub modal_input_blocking_deferred: bool,
    pub editor_hit_test_deferred_to_209: bool,
    pub control_style_deferred: bool,
    pub slider_toggle_binding_target_deferred: bool,
    pub ui_pass_inserted: bool,
    pub ui_composition_stage_count: usize,
    pub ui_before_world_item_count: usize,
    pub ui_screen_overlay_item_count: usize,
    pub ui_modal_item_count: usize,
    pub ui_before_world_pass_present: bool,
    pub ui_screen_overlay_pass_present: bool,
    pub ui_modal_pass_present: bool,
    pub ui_before_world_skipped: bool,
    pub ui_screen_overlay_skipped: bool,
    pub ui_modal_skipped: bool,
    pub modal_rendering_only: bool,
    pub glyph_present: bool,
    pub font_atlas_present: bool,
    pub font_atlas_id: Option<String>,
    pub font_source_kind: Option<String>,
    pub font_asset_id: Option<String>,
    pub font_asset_status: Option<String>,
    pub font_fallback_used: bool,
    pub requested_glyph_count: usize,
    pub rendered_glyph_count: usize,
    pub unsupported_glyph_count: usize,
    pub clipped_glyph_count: usize,
    pub glyph_plan_hash: Option<String>,
    pub snapshot_source: String,
    pub producer_id: Option<String>,
    pub snapshot_value_count: usize,
    pub active_binding_paths: Vec<String>,
    pub produced_paths: Vec<String>,
    pub declared_binding_paths: Vec<String>,
    pub missing_paths: Vec<String>,
    pub type_mismatch_paths: Vec<String>,
    pub dirty_domains: Vec<String>,
    pub cache_status: String,
    pub cache_hit_paths: Vec<String>,
    pub cache_miss_paths: Vec<String>,
    pub source_paths: Vec<String>,
    pub status: String,
    pub next_actions: Vec<String>,
}

impl NativeAuiPresentSummary {
    fn empty() -> Self {
        Self {
            package_document_count: 0,
            loaded_document_count: 0,
            draw_item_count: 0,
            text_command_count: 0,
            aui_interaction_status: "not_run".to_string(),
            aui_input_consumed: false,
            aui_action_count: 0,
            aui_drop_count: 0,
            aui_modal_blocking_status: "not_run".to_string(),
            aui_focus_trap_status: "not_run".to_string(),
            aui_scroll_status: "not_run".to_string(),
            aui_consumed_wheel_count: 0,
            aui_consumed_keyboard_count: 0,
            aui_consumed_event_count_by_kind: std::collections::BTreeMap::new(),
            aui_scroll_offset_count: 0,
            aui_scroll_offset_applied: false,
            aui_scroll_applied_node_count: 0,
            aui_clipped_node_count: 0,
            aui_clip_root_count: 0,
            aui_effective_clip_item_count: 0,
            aui_culled_draw_item_count: 0,
            aui_hit_test_clip_rejected_count: 0,
            aui_scrollbar_visible_count: 0,
            aui_scrollbar_thumb_drag_count: 0,
            aui_keyboard_navigation_event_count: 0,
            aui_focus_visible_scroll_count: 0,
            aui_submit_count: 0,
            aui_cancel_count: 0,
            aui_screen_stack_push_count: 0,
            aui_screen_stack_pop_count: 0,
            aui_active_screen_id: None,
            aui_default_focus_applied_count: 0,
            aui_focus_restore_count: 0,
            aui_text_edit_session_count: 0,
            aui_text_changed_count: 0,
            aui_text_submitted_count: 0,
            aui_text_cancelled_count: 0,
            aui_ime_preedit_count: 0,
            aui_ime_commit_count: 0,
            aui_ime_cancel_count: 0,
            aui_action_prompt_reported: false,
            aui_ime_platform_coverage: "not_run".to_string(),
            aui_focusable_derived_from_interactable: false,
            gameplay_input_filtered: false,
            snapshot_frame_lag: 0,
            authoring_action_payload_deferred: false,
            modal_input_blocking_deferred: false,
            editor_hit_test_deferred_to_209: false,
            control_style_deferred: false,
            slider_toggle_binding_target_deferred: false,
            ui_pass_inserted: false,
            ui_composition_stage_count: 0,
            ui_before_world_item_count: 0,
            ui_screen_overlay_item_count: 0,
            ui_modal_item_count: 0,
            ui_before_world_pass_present: false,
            ui_screen_overlay_pass_present: false,
            ui_modal_pass_present: false,
            ui_before_world_skipped: true,
            ui_screen_overlay_skipped: true,
            ui_modal_skipped: true,
            modal_rendering_only: false,
            glyph_present: false,
            font_atlas_present: false,
            font_atlas_id: None,
            font_source_kind: None,
            font_asset_id: None,
            font_asset_status: None,
            font_fallback_used: false,
            requested_glyph_count: 0,
            rendered_glyph_count: 0,
            unsupported_glyph_count: 0,
            clipped_glyph_count: 0,
            glyph_plan_hash: None,
            snapshot_source: "none".to_string(),
            producer_id: None,
            snapshot_value_count: 0,
            active_binding_paths: Vec::new(),
            produced_paths: Vec::new(),
            declared_binding_paths: Vec::new(),
            missing_paths: Vec::new(),
            type_mismatch_paths: Vec::new(),
            dirty_domains: Vec::new(),
            cache_status: "not_reported".to_string(),
            cache_hit_paths: Vec::new(),
            cache_miss_paths: Vec::new(),
            source_paths: Vec::new(),
            status: "not_presented".to_string(),
            next_actions: Vec::new(),
        }
    }

    fn no_documents(package_document_count: usize) -> Self {
        Self {
            package_document_count,
            loaded_document_count: 0,
            draw_item_count: 0,
            text_command_count: 0,
            aui_interaction_status: "not_run".to_string(),
            aui_input_consumed: false,
            aui_action_count: 0,
            aui_drop_count: 0,
            aui_modal_blocking_status: "not_run".to_string(),
            aui_focus_trap_status: "not_run".to_string(),
            aui_scroll_status: "not_run".to_string(),
            aui_consumed_wheel_count: 0,
            aui_consumed_keyboard_count: 0,
            aui_consumed_event_count_by_kind: std::collections::BTreeMap::new(),
            aui_scroll_offset_count: 0,
            aui_scroll_offset_applied: false,
            aui_scroll_applied_node_count: 0,
            aui_clipped_node_count: 0,
            aui_clip_root_count: 0,
            aui_effective_clip_item_count: 0,
            aui_culled_draw_item_count: 0,
            aui_hit_test_clip_rejected_count: 0,
            aui_scrollbar_visible_count: 0,
            aui_scrollbar_thumb_drag_count: 0,
            aui_keyboard_navigation_event_count: 0,
            aui_focus_visible_scroll_count: 0,
            aui_submit_count: 0,
            aui_cancel_count: 0,
            aui_screen_stack_push_count: 0,
            aui_screen_stack_pop_count: 0,
            aui_active_screen_id: None,
            aui_default_focus_applied_count: 0,
            aui_focus_restore_count: 0,
            aui_text_edit_session_count: 0,
            aui_text_changed_count: 0,
            aui_text_submitted_count: 0,
            aui_text_cancelled_count: 0,
            aui_ime_preedit_count: 0,
            aui_ime_commit_count: 0,
            aui_ime_cancel_count: 0,
            aui_action_prompt_reported: false,
            aui_ime_platform_coverage: "not_run".to_string(),
            aui_focusable_derived_from_interactable: false,
            gameplay_input_filtered: false,
            snapshot_frame_lag: 0,
            authoring_action_payload_deferred: false,
            modal_input_blocking_deferred: false,
            editor_hit_test_deferred_to_209: false,
            control_style_deferred: false,
            slider_toggle_binding_target_deferred: false,
            ui_pass_inserted: false,
            ui_composition_stage_count: 0,
            ui_before_world_item_count: 0,
            ui_screen_overlay_item_count: 0,
            ui_modal_item_count: 0,
            ui_before_world_pass_present: false,
            ui_screen_overlay_pass_present: false,
            ui_modal_pass_present: false,
            ui_before_world_skipped: true,
            ui_screen_overlay_skipped: true,
            ui_modal_skipped: true,
            modal_rendering_only: false,
            glyph_present: false,
            font_atlas_present: false,
            font_atlas_id: None,
            font_source_kind: None,
            font_asset_id: None,
            font_asset_status: None,
            font_fallback_used: false,
            requested_glyph_count: 0,
            rendered_glyph_count: 0,
            unsupported_glyph_count: 0,
            clipped_glyph_count: 0,
            glyph_plan_hash: None,
            snapshot_source: "none".to_string(),
            producer_id: None,
            snapshot_value_count: 0,
            active_binding_paths: Vec::new(),
            produced_paths: Vec::new(),
            declared_binding_paths: Vec::new(),
            missing_paths: Vec::new(),
            type_mismatch_paths: Vec::new(),
            dirty_domains: Vec::new(),
            cache_status: "not_reported".to_string(),
            cache_hit_paths: Vec::new(),
            cache_miss_paths: Vec::new(),
            source_paths: Vec::new(),
            status: "no_aui_documents".to_string(),
            next_actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct NativeAuiInteractionFrame {
    result: AuiInteractionResult,
    report: AuiInteractionProductizationReport,
    navigation_screenflow_textentry_report:
        AuiRuntimeNavigationScreenFlowTextEntryProductizationReport,
}

#[cfg(test)]
fn resolve_native_input_frame(
    device_state: &mut InputDeviceState,
    mapping: &InputMappingAsset,
    mapping_source: &str,
    mapping_path: Option<&str>,
    raw_events: &[RawInputEvent],
    frame_id: u64,
    viewport_id: &str,
    backend: &str,
    platform: &str,
) -> (ActionSnapshot, InputTraceSummary, NativeInputSummary) {
    let mut aui_state = AuiInteractionState::default();
    let (snapshot, trace_summary, summary, _) = resolve_native_input_frame_with_aui(
        device_state,
        mapping,
        mapping_source,
        mapping_path,
        raw_events,
        frame_id,
        viewport_id,
        backend,
        platform,
        None,
        None,
        &mut aui_state,
    );
    (snapshot, trace_summary, summary)
}

fn resolve_native_input_frame_with_aui(
    device_state: &mut InputDeviceState,
    mapping: &InputMappingAsset,
    mapping_source: &str,
    mapping_path: Option<&str>,
    raw_events: &[RawInputEvent],
    frame_id: u64,
    viewport_id: &str,
    backend: &str,
    platform: &str,
    aui_present: Option<&AuiRuntimePresentOutput>,
    game_view_presentation: Option<&ResolvedGameViewPresentation>,
    aui_state: &mut AuiInteractionState,
) -> (
    ActionSnapshot,
    InputTraceSummary,
    NativeInputSummary,
    Option<NativeAuiInteractionFrame>,
) {
    device_state.begin_frame();
    device_state.apply_raw_events(raw_events);
    let runtime_frame = device_state.to_runtime_input_frame(frame_id, viewport_id);
    let mut filtered_frame = runtime_frame.clone();
    let mut aui_interaction = None;
    if let Some(aui_present) = aui_present {
        let config = AuiInteractionConfig::default();
        let result = if let Some(presentation) = game_view_presentation {
            AuiInteractionSystem::process_target_space_session_with_state(
                &aui_present.resolved_document,
                &aui_present.layout,
                &runtime_frame,
                aui_state,
                config,
                presentation,
                viewport_id,
            )
        } else {
            AuiInteractionSystem::process_session_with_state(
                &aui_present.resolved_document,
                &aui_present.layout,
                &runtime_frame,
                aui_state,
                config,
                viewport_id,
            )
        };
        filtered_frame = runtime_frame.filter_consumed_events(&result.consumed_event_indices);
        let layout_after_interaction = AuiLayoutEngine::layout_with_interaction_state(
            &aui_present.resolved_document,
            frame_id,
            aui_state,
        );
        let report = AuiInteractionProductizationReport::from_result(
            &aui_present.resolved_document,
            runtime_frame.events.len(),
            filtered_frame.events.len(),
            &result,
            config,
            aui_state.active_drag_source().map(ToOwned::to_owned),
        )
        .with_focus_state(&aui_state.focus)
        .with_layout_report(&layout_after_interaction.report);
        let navigation_screenflow_textentry_report =
            AuiRuntimeNavigationScreenFlowTextEntryProductizationReport::from_result(
                &aui_present.resolved_document,
                runtime_frame.events.len(),
                filtered_frame.events.len(),
                &result,
            );
        aui_interaction = Some(NativeAuiInteractionFrame {
            result,
            report,
            navigation_screenflow_textentry_report,
        });
    }
    let input_result = InputResolver::resolve(&filtered_frame, mapping);
    let snapshot = input_result.action_snapshot;
    let trace_summary = InputTraceSummary::from_snapshot(Some(&snapshot)).with_route(
        Some(viewport_id.to_string()),
        Some("NativePlayerWindow".to_string()),
        Some(if aui_interaction.is_some() {
            "RuntimeInputFrameFilteredByAui".to_string()
        } else {
            "RuntimeInputFrame".to_string()
        }),
        Some(format!("{backend}:{platform}")),
    );
    let summary = NativeInputSummary::from_snapshot(
        backend,
        platform,
        raw_events.len(),
        filtered_frame.events.len(),
        device_state.report(),
        &snapshot,
    )
    .with_mapping(
        mapping_source,
        mapping.asset_id.clone(),
        "ok",
        mapping_path.map(|path| path.to_string()),
    );
    (snapshot, trace_summary, summary, aui_interaction)
}

fn hydrate_active_scene_for_player(
    package: &RuntimePackage,
    report: &mut NativeWindowHostReport,
) -> Option<(World, RuntimeSceneHydrator)> {
    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(package);
    let hydration = hydrator.hydrate_active_scene(package, &mut world);
    for diagnostic in &hydration.instantiate_report.diagnostics {
        let severity = match diagnostic.severity {
            engine_runtime::runtime_instance_diagnostics::InstanceDiagnosticSeverity::Error => {
                NativeWindowHostDiagnosticSeverity::Error
            }
            engine_runtime::runtime_instance_diagnostics::InstanceDiagnosticSeverity::Warning => {
                NativeWindowHostDiagnosticSeverity::Warning
            }
        };
        report.diagnostics.push(NativeWindowHostDiagnostic {
            severity,
            code: diagnostic.kind.clone(),
            layer: "scene".to_string(),
            message: diagnostic.message.clone(),
            path: diagnostic
                .source_entity_id
                .as_ref()
                .map(|entity_id| format!("scene.entities.{entity_id}")),
        });
    }
    if hydration.has_errors() {
        return None;
    }
    Some((world, hydrator))
}

fn push_project_runtime_error(report: &mut NativeWindowHostReport, error: ProjectRuntimeError) {
    let mut diagnostic =
        NativeWindowHostDiagnostic::error(error.code, "project_runtime", error.message);
    diagnostic.path = error
        .rule_id
        .as_ref()
        .map(|rule_id| format!("rules.{rule_id}"));
    report.diagnostics.push(diagnostic);
}

fn validate_run_evidence_request(
    request: &NativePlayerWindowRunRequest,
    report: &mut NativeWindowHostReport,
) -> bool {
    if request.frame_limit == 0 {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "invalid_frame_limit",
            "request",
            "frame_limit must be greater than zero",
        ));
        return false;
    }
    if let Some(script) = &request.input_script {
        if let Err(message) = script.validate() {
            report.diagnostics.push(NativeWindowHostDiagnostic::error(
                "invalid_input_script",
                "request",
                message,
            ));
            return false;
        }
    }
    if let Err(error) = request.game_view_target.validate() {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            error.code,
            "request",
            "game_view_target is invalid",
        ));
        return false;
    }
    let required_frames = request
        .performance_warmup_frames
        .saturating_add(request.performance_sample_frames);
    if request.performance_sample_frames > 0 && request.frame_limit < required_frames {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "insufficient_performance_frames",
            "request",
            format!(
                "frame_limit {} is smaller than warm-up + sample frames {}",
                request.frame_limit, required_frames
            ),
        ));
        return false;
    }
    true
}

fn scripted_input_events(
    request: &NativePlayerWindowRunRequest,
    frame_index: u64,
) -> Vec<RawInputEvent> {
    let Some(script) = &request.input_script else {
        return Vec::new();
    };
    let Some(frame) = script
        .frames
        .iter()
        .find(|frame| frame.frame_index == frame_index)
    else {
        return Vec::new();
    };
    frame
        .key_down
        .iter()
        .map(|key| RawInputEvent::keyboard_down(frame_index, request.config.window_id.clone(), key))
        .chain(frame.key_up.iter().map(|key| {
            RawInputEvent::keyboard_up(frame_index, request.config.window_id.clone(), key)
        }))
        .collect()
}

fn record_runtime_trace(
    report: &mut NativeWindowHostReport,
    trace: &engine_runtime::runtime_trace::RuntimeTrace,
    level: WindowedPlayerRuntimeReportLevel,
) {
    let Some(summary) = &mut report.gameplay_trace_summary else {
        return;
    };
    for record in &trace.gameplay_records {
        summary.record_count += 1;
        summary.write_count += usize::from(record.operation == "write");
        summary.command_enqueue_count += usize::from(record.operation == "command_enqueue");
        summary.command_apply_count += usize::from(record.operation == "command_apply");
        summary.prefab_instantiate_apply_count += usize::from(
            record.operation == "command_apply" && record.source.is_some() && record.result == "ok",
        );
        summary.failed_record_count += usize::from(record.result != "ok");
        if level == WindowedPlayerRuntimeReportLevel::Trace {
            report
                .gameplay_trace_records
                .push(WindowedPlayerGameplayTraceRecord {
                    frame_index: record.frame_index,
                    phase: record.phase.clone(),
                    rule_id: record.rule_id.clone(),
                    operation: record.operation.clone(),
                    entity_id: record.entity_id.as_ref().map(ToString::to_string),
                    component_type: record.component_type.as_ref().map(ToString::to_string),
                    field_path: record.field_path.clone(),
                    before: record.before.clone(),
                    after: record.after.clone(),
                    source: record.source.clone(),
                    result: record.result.clone(),
                    error_code: record.error_code.clone(),
                });
        }
    }
}

fn summarize_frame_phase(
    values: &[u64],
    request: &NativePlayerWindowRunRequest,
) -> Option<engine_runtime::windowed_player::WindowedPlayerFramePhasePerformanceSummary> {
    let mut sample = values
        .iter()
        .skip(request.performance_warmup_frames as usize)
        .take(request.performance_sample_frames as usize)
        .copied()
        .collect::<Vec<_>>();
    sample.sort_unstable();
    if sample.is_empty() {
        return None;
    }
    let mean_ns = sample.iter().map(|value| *value as f64).sum::<f64>() / sample.len() as f64;
    let percentile_ms = |percentile: f64| {
        let rank = ((sample.len() as f64 * percentile).ceil() as usize)
            .saturating_sub(1)
            .min(sample.len() - 1);
        sample[rank] as f64 / 1_000_000.0
    };
    Some(
        engine_runtime::windowed_player::WindowedPlayerFramePhasePerformanceSummary {
            observed_sample_frames: sample.len() as u64,
            mean_ms: mean_ns / 1_000_000.0,
            p95_ms: percentile_ms(0.95),
            p99_ms: percentile_ms(0.99),
        },
    )
}

fn finalize_frame_performance(
    report: &mut NativeWindowHostReport,
    request: &NativePlayerWindowRunRequest,
    frame_cpu_ns: &[u64],
    frame_update_ns: &[u64],
    frame_render_submit_ns: &[u64],
    frame_present_wait_ns: &[u64],
) {
    if request.performance_sample_frames == 0 {
        return;
    }
    let mut sample = frame_cpu_ns
        .iter()
        .skip(request.performance_warmup_frames as usize)
        .take(request.performance_sample_frames as usize)
        .copied()
        .collect::<Vec<_>>();
    sample.sort_unstable();
    if sample.is_empty() {
        return;
    }
    let Some(update) = summarize_frame_phase(frame_update_ns, request) else {
        return;
    };
    let Some(render_submit) = summarize_frame_phase(frame_render_submit_ns, request) else {
        return;
    };
    let Some(present_wait) = summarize_frame_phase(frame_present_wait_ns, request) else {
        return;
    };
    let mean_ns = sample.iter().map(|value| *value as f64).sum::<f64>() / sample.len() as f64;
    let percentile_ms = |percentile: f64| {
        let rank = ((sample.len() as f64 * percentile).ceil() as usize)
            .saturating_sub(1)
            .min(sample.len() - 1);
        sample[rank] as f64 / 1_000_000.0
    };
    report.frame_performance_summary = Some(WindowedPlayerFramePerformanceSummary {
        warmup_frames: request.performance_warmup_frames,
        requested_sample_frames: request.performance_sample_frames,
        observed_sample_frames: sample.len() as u64,
        mean_cpu_frame_ms: mean_ns / 1_000_000.0,
        p95_cpu_frame_ms: percentile_ms(0.95),
        p99_cpu_frame_ms: percentile_ms(0.99),
        update,
        render_submit,
        present_wait,
    });
}

pub fn run_headless_native_player_from_package(
    request: NativePlayerWindowRunRequest,
) -> NativeWindowHostReport {
    let linked_modules = LinkedProjectRuntimeSet::explicit_empty();
    run_headless_native_player_from_package_with_linked_modules(request, &linked_modules)
}

pub fn run_headless_native_player_from_package_with_linked_modules(
    request: NativePlayerWindowRunRequest,
    linked_modules: &LinkedProjectRuntimeSet,
) -> NativeWindowHostReport {
    let mut report = NativeWindowHostReport::base(&request);
    report.window_status = "headless".to_string();
    report.surface_status = "headless_surface".to_string();
    report.window = NativeWindowState::created(&request.config);

    if request.frame_limit == 0 {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "invalid_frame_limit",
            "request",
            "frame_limit must be greater than zero",
        ));
        return report;
    }
    if !validate_run_evidence_request(&request, &mut report) {
        return report;
    }

    let load = load_runtime_package(&request.runtime_package_path);
    report.diagnostics.extend(convert_runtime_diagnostics(
        "package",
        &load.diagnostics.issues,
    ));
    let Some(package) = load.value else {
        report.package_status = "error".to_string();
        report.present_status = NativeWindowPresentStatus::PackageFailed;
        return report;
    };
    report.package_status = "ok".to_string();
    report.aui.package_document_count = package.aui_manifest.documents.len();
    report.aui.loaded_document_count = package.aui_documents.len();

    let bound_runtime = match ProjectRuntimeBootstrap::bind(&package, linked_modules) {
        Ok(bound_runtime) => bound_runtime,
        Err(error) => {
            report.logic_status = "error".to_string();
            push_project_runtime_error(&mut report, error);
            report.present_status = NativeWindowPresentStatus::PackageFailed;
            return report;
        }
    };
    let NativePlayerRuntimeComposition {
        mut host,
        mut ui_state_producer,
        input_mapping,
        receipt,
    } = NativePlayerRuntimeComposition::from_bound(
        package.active_scene.id.clone(),
        bound_runtime.into_parts(),
    );
    report.project_runtime_bind_receipt = Some(receipt);

    let Some((mut world, mut hydrator)) = hydrate_active_scene_for_player(&package, &mut report)
    else {
        report.scene_status = "error".to_string();
        report.world_status = "error".to_string();
        report.present_status = NativeWindowPresentStatus::SceneFailed;
        return report;
    };
    report.scene_status = "ok".to_string();
    report.world_status = "ok".to_string();
    let (sprite_texture_bindings, runtime_texture_bindings) =
        match prepare_headless_texture_bindings(&package) {
            Ok(bindings) => bindings,
            Err(diagnostics) => {
                report.rhi_status = "error".to_string();
                report.present_status = NativeWindowPresentStatus::RhiFailed;
                report.diagnostics.extend(diagnostics);
                return report;
            }
        };

    let mut input_device_state = InputDeviceState::new();
    let mut aui_interaction_state = AuiInteractionState::default();
    let mut aui_feedback_state = AuiControlFeedbackState::default();
    let mut last_render_schema = None;
    let mut last_rhi_command_count = 0;
    let mut aui_present_cache = NativeAuiPresentCache::for_package(&package);
    let mut frame_cpu_ns = Vec::with_capacity(request.frame_limit as usize);
    let mut frame_update_ns = Vec::with_capacity(request.frame_limit as usize);
    let mut frame_render_submit_ns = Vec::with_capacity(request.frame_limit as usize);
    let mut frame_present_wait_ns = Vec::with_capacity(request.frame_limit as usize);
    for frame_index in 0..request.frame_limit {
        let frame_started = Instant::now();
        let scripted_events = scripted_input_events(&request, frame_index + 1);
        let mut aui_present = aui_present_cache.take_or_rebuild(
            &package,
            &world,
            frame_index + 1,
            ui_state_producer.as_mut(),
            None,
        );
        let (action_snapshot, input_trace_summary, input_summary, aui_interaction) =
            resolve_native_input_frame_with_aui(
                &mut input_device_state,
                &input_mapping,
                "runtime-package",
                Some("input/input-manifest.json"),
                &scripted_events,
                frame_index + 1,
                &request.config.window_id,
                "headless-script",
                "headless",
                aui_present.as_ref(),
                None,
                &mut aui_interaction_state,
            );
        if let (Some(present), Some(interaction)) = (aui_present.as_mut(), aui_interaction.as_ref())
        {
            AuiRuntimePresenter::apply_control_feedback_with_fonts(
                present,
                &interaction.result,
                &mut aui_feedback_state,
                presentation_delta_us_from_seconds(
                    engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME,
                ),
                &package.font_atlases,
                &package.font_bundles,
            );
        }
        report.input = input_summary;
        report.input_status = "ok".to_string();
        let mut frame_input = EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_action_snapshot(action_snapshot)
            .with_input_trace_summary(input_trace_summary)
            .with_runtime_texture_bindings(runtime_texture_bindings.clone());
        if let Some(aui_present) = aui_present.as_ref() {
            report.aui = NativeAuiPresentSummary::from_present_output(
                &package,
                aui_present,
                request.config.surface_target().target_id.as_str(),
            );
            if let Some(aui_interaction) = aui_interaction {
                report.aui.apply_interaction_report(&aui_interaction.report);
                report.aui.apply_navigation_screenflow_textentry_report(
                    &aui_interaction.navigation_screenflow_textentry_report,
                );
                frame_input = frame_input.with_aui_interaction(aui_interaction.result);
            }
            frame_input = frame_input
                .with_aui_overlay(aui_present.overlay.clone())
                .with_aui_composition(aui_present.composition.clone());
        } else if package.aui_manifest.documents.is_empty() {
            report.aui = NativeAuiPresentSummary::no_documents(0);
        } else {
            report.aui.status = "load_failed".to_string();
        }
        let output = host.tick_with_runtime_context(
            frame_input,
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: hydrator.instance_loader_mut(),
            },
        );
        report.project_runtime_session_report = output.project_runtime_session_report.clone();
        frame_update_ns.push(frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        let render_started = Instant::now();
        record_runtime_trace(
            &mut report,
            &output.runtime_trace,
            request.runtime_report_level,
        );
        if output.runtime_advanced {
            report.logic_status = "ok".to_string();
        }
        if output.render_frame_report.is_some() {
            report.render_status = "ok".to_string();
        }
        let render_thread_frame = host.render_thread_for_target_with_runtime_resources(
            request.config.surface_target(),
            aui_present.as_ref().map(|present| &present.overlay),
            aui_present.as_ref().map(|present| &present.composition),
            Some(&sprite_texture_bindings),
            Some(&runtime_texture_bindings),
        );
        aui_present_cache.store(aui_present);
        report
            .aui
            .apply_render_frame_report(&render_thread_frame.renderer_output.render_frame_report);
        last_render_schema = Some(render_thread_frame.report.schema_version.clone());
        last_rhi_command_count = render_thread_frame
            .renderer_output
            .rhi_command_plan
            .commands
            .len();
        report.rhi_status = render_thread_frame.report.rhi_status;
        if report.rhi_status != "ok" {
            report.present_status = NativeWindowPresentStatus::RhiFailed;
        }
        report.frames_completed += 1;
        frame_render_submit_ns.push(
            render_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        );
        frame_present_wait_ns.push(0);
        frame_cpu_ns.push(frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
    }

    finalize_frame_performance(
        &mut report,
        &request,
        &frame_cpu_ns,
        &frame_update_ns,
        &frame_render_submit_ns,
        &frame_present_wait_ns,
    );

    report.render_thread_report_schema = last_render_schema;
    report.rhi_command_count = last_rhi_command_count;
    report.surface =
        NativeSurfaceState::headless_presented(&request.config, report.frames_completed);
    if report.logic_status == "ok" && report.render_status == "ok" && report.rhi_status == "ok" {
        report.present_status = NativeWindowPresentStatus::Presented;
    }
    report.exit_code =
        if report.has_errors() || report.present_status != NativeWindowPresentStatus::Presented {
            1
        } else {
            0
        };
    report
}

fn runtime_sprite_texture_asset_ids(package: &RuntimePackage) -> BTreeSet<String> {
    package
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
        .collect()
}

fn prepare_headless_texture_bindings(
    package: &RuntimePackage,
) -> Result<
    (Sprite2DTextureBindingContext, RuntimeTextureBindingContext),
    Vec<NativeWindowHostDiagnostic>,
> {
    let sprite_refs = runtime_sprite_texture_asset_ids(package);
    let aui_refs = package
        .aui_documents
        .documents_by_id
        .values()
        .flat_map(|document| document.nodes.iter())
        .filter_map(|node| node.image.as_ref())
        .map(|image| image.asset_id.clone());
    let registry = RuntimeTextureUploadRegistry::load(
        &package.package_dir,
        &package.runtime_asset_index,
        sprite_refs.iter().cloned().chain(aui_refs),
    );
    if !registry.diagnostics().is_empty() {
        return Err(registry
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                NativeWindowHostDiagnostic::error(
                    "runtime_texture_resolve_failed",
                    "render",
                    format!(
                        "Texture '{}' could not be resolved: {}",
                        diagnostic.asset_ref_id, diagnostic.message
                    ),
                )
            })
            .collect());
    }
    let runtime_bindings = registry.binding_context();
    let mut sprite_bindings = Sprite2DTextureBindingContext::new();
    for sprite_ref in sprite_refs {
        let Some(binding) = runtime_bindings.get(&sprite_ref) else {
            return Err(vec![NativeWindowHostDiagnostic::error(
                "runtime_sprite_texture_binding_missing",
                "render",
                format!("Sprite texture '{sprite_ref}' has no runtime binding."),
            )]);
        };
        sprite_bindings.insert_texture_handle(sprite_ref, binding.handle, binding.sampler.clone());
    }
    Ok((sprite_bindings, runtime_bindings))
}

impl NativeAuiPresentSummary {
    fn from_present_output(
        package: &engine_runtime::runtime_package::RuntimePackage,
        output: &AuiRuntimePresentOutput,
        _target_id: &str,
    ) -> Self {
        let mut next_actions = Vec::new();
        if !output.report.glyph_present && output.report.text_command_count > 0 {
            next_actions.push("runtime_text_glyph_present".to_string());
        }
        if output.report.snapshot_source != AuiSnapshotSource::ProjectProducer {
            next_actions.push("project_ui_state_snapshot_producer".to_string());
        }
        let snapshot_report = output.report.ui_state_snapshot_report.as_ref();
        if let Some(report) = snapshot_report {
            for path in &report.missing_paths {
                next_actions.push(format!("resolve_ui_state_path:{path}"));
            }
        }
        let (_, render_report) =
            AuiLayoutEngine::extract_draw_list(&output.resolved_document, &output.layout);
        Self {
            package_document_count: package.aui_manifest.documents.len(),
            loaded_document_count: package.aui_documents.len(),
            draw_item_count: output.report.draw_item_count,
            text_command_count: output.report.text_command_count,
            aui_interaction_status: "not_run".to_string(),
            aui_input_consumed: false,
            aui_action_count: 0,
            aui_drop_count: 0,
            aui_modal_blocking_status: "not_run".to_string(),
            aui_focus_trap_status: "not_run".to_string(),
            aui_scroll_status: "not_run".to_string(),
            aui_consumed_wheel_count: 0,
            aui_consumed_keyboard_count: 0,
            aui_consumed_event_count_by_kind: std::collections::BTreeMap::new(),
            aui_scroll_offset_count: 0,
            aui_scroll_offset_applied: false,
            aui_scroll_applied_node_count: 0,
            aui_clipped_node_count: 0,
            aui_clip_root_count: output.layout.report.clip_root_count,
            aui_effective_clip_item_count: render_report.effective_clip_item_count,
            aui_culled_draw_item_count: render_report.culled_draw_item_count,
            aui_hit_test_clip_rejected_count: 0,
            aui_scrollbar_visible_count: render_report.scrollbar_visible_count,
            aui_scrollbar_thumb_drag_count: 0,
            aui_keyboard_navigation_event_count: 0,
            aui_focus_visible_scroll_count: 0,
            aui_submit_count: 0,
            aui_cancel_count: 0,
            aui_screen_stack_push_count: 0,
            aui_screen_stack_pop_count: 0,
            aui_active_screen_id: None,
            aui_default_focus_applied_count: 0,
            aui_focus_restore_count: 0,
            aui_text_edit_session_count: 0,
            aui_text_changed_count: 0,
            aui_text_submitted_count: 0,
            aui_text_cancelled_count: 0,
            aui_ime_preedit_count: 0,
            aui_ime_commit_count: 0,
            aui_ime_cancel_count: 0,
            aui_action_prompt_reported: false,
            aui_ime_platform_coverage: "not_run".to_string(),
            aui_focusable_derived_from_interactable: false,
            gameplay_input_filtered: false,
            snapshot_frame_lag: 0,
            authoring_action_payload_deferred: false,
            modal_input_blocking_deferred: false,
            editor_hit_test_deferred_to_209: false,
            control_style_deferred: false,
            slider_toggle_binding_target_deferred: false,
            ui_pass_inserted: output.report.ui_pass_inserted,
            ui_composition_stage_count: output.report.ui_composition_stage_count,
            ui_before_world_item_count: output.report.ui_before_world_item_count,
            ui_screen_overlay_item_count: output.report.ui_screen_overlay_item_count,
            ui_modal_item_count: output.report.ui_modal_item_count,
            ui_before_world_pass_present: false,
            ui_screen_overlay_pass_present: false,
            ui_modal_pass_present: false,
            ui_before_world_skipped: true,
            ui_screen_overlay_skipped: true,
            ui_modal_skipped: true,
            modal_rendering_only: output.report.modal_rendering_only,
            glyph_present: output.report.glyph_present,
            font_atlas_present: output.report.font_atlas_present,
            font_atlas_id: output.report.font_atlas_id.clone(),
            font_source_kind: output.report.font_source_kind.clone(),
            font_asset_id: output.report.font_asset_id.clone(),
            font_asset_status: output.report.font_asset_status.clone(),
            font_fallback_used: output.report.font_fallback_used,
            requested_glyph_count: output.report.requested_glyph_count,
            rendered_glyph_count: output.report.rendered_glyph_count,
            unsupported_glyph_count: output.report.unsupported_glyph_count,
            clipped_glyph_count: output.report.clipped_glyph_count,
            glyph_plan_hash: output.report.glyph_plan_hash.clone(),
            snapshot_source: snapshot_source_id(output.report.snapshot_source).to_string(),
            producer_id: snapshot_report.map(|report| report.producer_id.clone()),
            snapshot_value_count: output.report.snapshot_value_count,
            active_binding_paths: snapshot_report
                .map(|report| report.active_binding_paths.clone())
                .unwrap_or_default(),
            produced_paths: snapshot_report
                .map(|report| report.produced_paths.clone())
                .unwrap_or_default(),
            declared_binding_paths: snapshot_report
                .map(|report| report.declared_binding_paths.clone())
                .unwrap_or_default(),
            missing_paths: snapshot_report
                .map(|report| report.missing_paths.clone())
                .unwrap_or_default(),
            type_mismatch_paths: snapshot_report
                .map(|report| report.type_mismatch_paths.clone())
                .unwrap_or_default(),
            dirty_domains: snapshot_report
                .map(|report| report.dirty_domains.clone())
                .unwrap_or_default(),
            cache_status: snapshot_report
                .map(|report| report.cache_status.clone())
                .unwrap_or_else(|| "not_reported".to_string()),
            cache_hit_paths: snapshot_report
                .map(|report| report.cache_hit_paths.clone())
                .unwrap_or_default(),
            cache_miss_paths: snapshot_report
                .map(|report| report.cache_miss_paths.clone())
                .unwrap_or_default(),
            source_paths: snapshot_report
                .map(|report| report.source_paths.clone())
                .unwrap_or_default(),
            status: match output.report.status {
                AuiRuntimePresentStatus::Success => "success".to_string(),
                AuiRuntimePresentStatus::Partial => "partial".to_string(),
                AuiRuntimePresentStatus::Failed => "failed".to_string(),
            },
            next_actions,
        }
    }

    fn apply_render_frame_report(
        &mut self,
        report: &engine_runtime::runtime_renderer::RuntimeRenderFrameReport,
    ) {
        self.ui_before_world_pass_present = report.ui_before_world_pass_present;
        self.ui_screen_overlay_pass_present = report.ui_screen_overlay_pass_present;
        self.ui_modal_pass_present = report.ui_modal_pass_present;
        self.ui_before_world_skipped = report.ui_before_world_skipped;
        self.ui_screen_overlay_skipped = report.ui_screen_overlay_skipped;
        self.ui_modal_skipped = report.ui_modal_skipped;
        self.ui_composition_stage_count = report.ui_composition_stage_count;
        self.ui_before_world_item_count = report.ui_before_world_item_count;
        self.ui_screen_overlay_item_count = report.ui_screen_overlay_item_count;
        self.ui_modal_item_count = report.ui_modal_item_count;
        self.ui_pass_inserted = self.ui_before_world_pass_present
            || self.ui_screen_overlay_pass_present
            || self.ui_modal_pass_present;
    }

    fn apply_interaction_report(&mut self, report: &AuiInteractionProductizationReport) {
        self.aui_interaction_status = "success".to_string();
        self.aui_input_consumed = !report.consumed_event_count_by_kind.is_empty();
        self.aui_action_count = report.action_count;
        self.aui_drop_count = report.drop_count;
        self.aui_modal_blocking_status = report.modal_blocking_status.clone();
        self.aui_focus_trap_status = report.focus_trap_status.clone();
        self.aui_scroll_status = report.scroll_status.clone();
        self.aui_consumed_wheel_count = report.consumed_wheel_event_count;
        self.aui_consumed_keyboard_count = report.consumed_keyboard_event_count;
        self.aui_consumed_event_count_by_kind = report.consumed_event_count_by_kind.clone();
        self.aui_scroll_offset_count = report.scroll_offset_change_count;
        self.aui_scroll_offset_applied = report.scroll_offset_applied;
        self.aui_scroll_applied_node_count = report.scroll_applied_node_count;
        self.aui_clipped_node_count = report.clipped_node_count;
        self.aui_hit_test_clip_rejected_count = report.hit_test_clip_rejected_count;
        self.aui_scrollbar_thumb_drag_count = report
            .traces
            .iter()
            .filter(|trace| {
                trace
                    .captured_node
                    .as_deref()
                    .is_some_and(|node| node.ends_with(":scrollbar-thumb"))
            })
            .count();
        self.aui_keyboard_navigation_event_count = report.keyboard_navigation_event_count;
        self.aui_focus_visible_scroll_count = report.focus_visible_scroll_count;
        self.gameplay_input_filtered = report.input_event_count > report.filtered_input_event_count;
        self.snapshot_frame_lag = report.snapshot_frame_lag;
        self.authoring_action_payload_deferred = report.authoring_action_payload_deferred;
        self.modal_input_blocking_deferred = report.modal_input_blocking_deferred;
        self.editor_hit_test_deferred_to_209 = report.editor_hit_test_deferred_to_209;
        self.control_style_deferred = report.control_style_deferred;
        self.slider_toggle_binding_target_deferred = report.slider_toggle_binding_target_deferred;
        if self.gameplay_input_filtered {
            self.next_actions
                .retain(|action| action != "aui_runtime_interaction_productization");
        }
    }

    fn apply_navigation_screenflow_textentry_report(
        &mut self,
        report: &AuiRuntimeNavigationScreenFlowTextEntryProductizationReport,
    ) {
        self.aui_submit_count = report.submit_count;
        self.aui_cancel_count = report.cancel_count;
        self.aui_screen_stack_push_count = report.screen_stack_push_count;
        self.aui_screen_stack_pop_count = report.screen_stack_pop_count;
        self.aui_active_screen_id = report.active_screen_id.clone();
        self.aui_default_focus_applied_count = report.default_focus_applied_count;
        self.aui_focus_restore_count = report.focus_restore_count;
        self.aui_text_edit_session_count = report.text_edit_session_count;
        self.aui_text_changed_count = report.text_changed_count;
        self.aui_text_submitted_count = report.text_submitted_count;
        self.aui_text_cancelled_count = report.text_cancelled_count;
        self.aui_ime_preedit_count = report.ime_preedit_count;
        self.aui_ime_commit_count = report.ime_commit_count;
        self.aui_ime_cancel_count = report.ime_cancel_count;
        self.aui_action_prompt_reported = report.action_prompt_reported;
        self.aui_ime_platform_coverage = report.ime_platform_coverage.clone();
        self.aui_focusable_derived_from_interactable = report.focusable_derived_from_interactable;
        self.gameplay_input_filtered |= report.gameplay_input_filtered_count > 0;
        if report.status == "passed" {
            self.next_actions
                .retain(|action| action != "aui_runtime_navigation_screenflow_textentry");
        } else if !self
            .next_actions
            .iter()
            .any(|action| action == "aui_runtime_navigation_screenflow_textentry")
        {
            self.next_actions
                .push("aui_runtime_navigation_screenflow_textentry".to_string());
        }
    }
}

fn snapshot_source_id(source: AuiSnapshotSource) -> &'static str {
    match source {
        AuiSnapshotSource::EmptyDefaultSnapshot => "empty_default_snapshot",
        AuiSnapshotSource::PackageSmokeSnapshot => "package_smoke_snapshot",
        AuiSnapshotSource::ProjectProducer => "project_producer",
        AuiSnapshotSource::TestSnapshot => "test_snapshot",
        AuiSnapshotSource::ProjectRuleSnapshot => "project_rule_snapshot",
    }
}

struct NativeAuiPresentCache {
    snapshot_cache: ProjectUiStateSnapshotCache,
    last_present: Option<AuiRuntimePresentOutput>,
    rebuild_count: u64,
    hit_count: u64,
    presentation_revision: u64,
}

impl NativeAuiPresentCache {
    fn for_package(package: &RuntimePackage) -> Self {
        let active_binding_paths = package
            .aui_manifest
            .documents
            .first()
            .and_then(|entry| package.aui_documents.get(&entry.document_id))
            .map(active_aui_binding_paths)
            .unwrap_or_default();
        Self {
            snapshot_cache: ProjectUiStateSnapshotCache::new(active_binding_paths),
            last_present: None,
            rebuild_count: 0,
            hit_count: 0,
            presentation_revision: 0,
        }
    }

    fn take_or_rebuild(
        &mut self,
        package: &RuntimePackage,
        world: &World,
        frame_index: u64,
        producer: &mut dyn ProjectUiStateSnapshotProducer,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> Option<AuiRuntimePresentOutput> {
        let document_id = package
            .aui_manifest
            .documents
            .first()
            .map(|entry| entry.document_id.as_str())?;
        let document = package.aui_documents.get(document_id)?;
        let snapshot_output = match self.snapshot_cache.resolve(
            producer,
            frame_index,
            package,
            world,
            ProjectUiStateReportMode::Summary,
        ) {
            Ok(ProjectUiStateSnapshotCacheResult::Reuse) | Err(_) => {
                if let Some(present) = self.last_present.take() {
                    self.hit_count = self.hit_count.saturating_add(1);
                    return Some(present);
                }
                return None;
            }
            Ok(ProjectUiStateSnapshotCacheResult::Replace(output)) => output,
        };
        self.rebuild_count = self.rebuild_count.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        Some(match presentation {
            Some(presentation) => {
                AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation(
                    document,
                    snapshot_output,
                    &package.font_atlases,
                    &package.font_bundles,
                    presentation,
                )
            }
            None => AuiRuntimePresenter::present_project_snapshot_with_fonts(
                document,
                snapshot_output,
                &package.font_atlases,
                &package.font_bundles,
            ),
        })
    }

    fn store(&mut self, present: Option<AuiRuntimePresentOutput>) {
        self.last_present = present;
    }
}

fn active_aui_binding_paths(document: &engine_runtime::aui::AuiDocument) -> Vec<String> {
    document
        .nodes
        .iter()
        .flat_map(|node| node.binding_refs.iter().map(|binding| binding.path.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(not(feature = "real-window"))]
pub fn run_windowed_native_player_from_package(
    request: NativePlayerWindowRunRequest,
) -> NativeWindowHostReport {
    run_windowed_native_player_from_package_with_linked_modules(
        request,
        std::sync::Arc::new(LinkedProjectRuntimeSet::explicit_empty()),
    )
}

#[cfg(not(feature = "real-window"))]
pub fn run_windowed_native_player_from_package_with_linked_modules(
    request: NativePlayerWindowRunRequest,
    _linked_modules: std::sync::Arc<LinkedProjectRuntimeSet>,
) -> NativeWindowHostReport {
    let mut report = NativeWindowHostReport::base(&request);
    report.window_status = "feature_not_enabled".to_string();
    report.surface_status = "feature_not_enabled".to_string();
    report.present_status = NativeWindowPresentStatus::FeatureNotEnabled;
    report.diagnostics.push(NativeWindowHostDiagnostic::error(
        "native_window_feature_not_enabled",
        "window",
        "runtime_player_winit real-window feature is not enabled",
    ));
    report
}

#[cfg(all(feature = "real-window", not(target_os = "android")))]
pub fn run_windowed_native_player_from_package(
    request: NativePlayerWindowRunRequest,
) -> NativeWindowHostReport {
    run_windowed_native_player_from_package_with_linked_modules(
        request,
        std::sync::Arc::new(LinkedProjectRuntimeSet::explicit_empty()),
    )
}

#[cfg(all(feature = "real-window", not(target_os = "android")))]
pub fn run_windowed_native_player_from_package_with_linked_modules(
    request: NativePlayerWindowRunRequest,
    linked_modules: std::sync::Arc<LinkedProjectRuntimeSet>,
) -> NativeWindowHostReport {
    real_window::run_windowed(request, linked_modules)
}

#[cfg(all(feature = "real-window", target_os = "android"))]
pub fn run_android_native_player_from_package_with_linked_modules(
    android_app: winit::platform::android::activity::AndroidApp,
    request: NativePlayerWindowRunRequest,
    linked_modules: std::sync::Arc<LinkedProjectRuntimeSet>,
) -> NativeWindowHostReport {
    real_window::run_android(android_app, request, linked_modules)
}

fn convert_runtime_diagnostics(
    layer: &str,
    diagnostics: &[RuntimeDiagnostic],
) -> Vec<NativeWindowHostDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| NativeWindowHostDiagnostic {
            severity: match diagnostic.severity {
                DiagnosticSeverity::Error => NativeWindowHostDiagnosticSeverity::Error,
                DiagnosticSeverity::Warning => NativeWindowHostDiagnosticSeverity::Warning,
            },
            code: match diagnostic.severity {
                DiagnosticSeverity::Error => format!("{layer}_load_error"),
                DiagnosticSeverity::Warning => format!("{layer}_load_warning"),
            },
            layer: layer.to_string(),
            message: diagnostic.message.clone(),
            path: Some(diagnostic.path.clone()),
        })
        .collect()
}

#[cfg(any(test, feature = "real-window"))]
fn write_rgba_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<u64, String> {
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err("png.invalid_rgba_size".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("png.create_dir_failed:{error}"))?;
    }
    let png = encode_rgba_png(width, height, rgba);
    fs::write(path, &png).map_err(|error| format!("png.write_failed:{error}"))?;
    Ok(png.len() as u64)
}

#[cfg(any(test, feature = "real-window"))]
fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((height as usize) * (1 + width as usize * 4));
    for y in 0..height as usize {
        raw.push(0);
        let start = y * width as usize * 4;
        let end = start + width as usize * 4;
        raw.extend_from_slice(&rgba[start..end]);
    }
    let compressed = zlib_store_blocks(&raw);
    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &compressed);
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(any(test, feature = "real-window"))]
fn zlib_store_blocks(data: &[u8]) -> Vec<u8> {
    let mut zlib = vec![0x78, 0x01];
    let mut offset = 0;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_len = remaining.min(u16::MAX as usize);
        let is_final = offset + block_len == data.len();
        zlib.push(if is_final { 0x01 } else { 0x00 });
        let len = block_len as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(&data[offset..offset + block_len]);
        offset += block_len;
    }
    zlib.extend_from_slice(&adler32(data).to_be_bytes());
    zlib
}

#[cfg(any(test, feature = "real-window"))]
fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(kind.len() + data.len());
    crc_data.extend_from_slice(kind);
    crc_data.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_data).to_be_bytes());
}

#[cfg(any(test, feature = "real-window"))]
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(any(test, feature = "real-window"))]
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

#[cfg(any(test, feature = "real-window"))]
fn fit_player_window_inner_extent(
    requested_width: u32,
    requested_height: u32,
    work_area_width: u32,
    work_area_height: u32,
    non_client_width: u32,
    non_client_height: u32,
) -> (u32, u32) {
    let requested_width = requested_width.max(1);
    let requested_height = requested_height.max(1);
    let available_width = work_area_width.saturating_sub(non_client_width).max(1);
    let available_height = work_area_height.saturating_sub(non_client_height).max(1);
    if requested_width <= available_width && requested_height <= available_height {
        return (requested_width, requested_height);
    }

    if u64::from(requested_width) * u64::from(available_height)
        > u64::from(available_width) * u64::from(requested_height)
    {
        (
            available_width,
            ((u64::from(requested_height) * u64::from(available_width))
                / u64::from(requested_width))
            .max(1) as u32,
        )
    } else {
        (
            ((u64::from(requested_width) * u64::from(available_height))
                / u64::from(requested_height))
            .max(1) as u32,
            available_height,
        )
    }
}

#[cfg(feature = "real-window")]
mod real_window {
    use super::*;
    use engine_runtime::runtime_renderer::font_atlas_render_handle;
    use engine_runtime::runtime_texture::{
        RuntimeTextureBindingContext, RuntimeTextureUploadRegistry,
    };
    use engine_runtime::sprite2d_render_pipeline::Sprite2DTextureBindingContext;
    use std::sync::Arc;
    use std::time::Duration;
    use winit::application::ApplicationHandler;
    use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::keyboard::{Key, ModifiersState, NamedKey};
    #[cfg(target_os = "android")]
    use winit::platform::android::{activity::AndroidApp, EventLoopBuilderExtAndroid};
    #[cfg(target_os = "windows")]
    use winit::platform::windows::EventLoopBuilderExtWindows;
    #[cfg(target_os = "windows")]
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    const REAL_WINDOW_FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

    #[cfg(not(target_os = "android"))]
    pub fn run_windowed(
        request: NativePlayerWindowRunRequest,
        linked_modules: Arc<LinkedProjectRuntimeSet>,
    ) -> NativeWindowHostReport {
        #[cfg(target_os = "windows")]
        let event_loop_result = EventLoop::builder().with_any_thread(true).build();
        #[cfg(not(target_os = "windows"))]
        let event_loop_result = EventLoop::new();
        run_event_loop(event_loop_result, request, linked_modules)
    }

    #[cfg(target_os = "android")]
    pub fn run_android(
        android_app: AndroidApp,
        request: NativePlayerWindowRunRequest,
        linked_modules: Arc<LinkedProjectRuntimeSet>,
    ) -> NativeWindowHostReport {
        let event_loop_result = EventLoop::builder().with_android_app(android_app).build();
        run_event_loop(event_loop_result, request, linked_modules)
    }

    fn run_event_loop(
        event_loop_result: Result<EventLoop<()>, winit::error::EventLoopError>,
        request: NativePlayerWindowRunRequest,
        linked_modules: Arc<LinkedProjectRuntimeSet>,
    ) -> NativeWindowHostReport {
        let event_loop = match event_loop_result {
            Ok(event_loop) => event_loop,
            Err(error) => return environment_blocked_report(request, error.to_string()),
        };
        let mut app = RealWindowApp::new(request, linked_modules);
        match event_loop.run_app(&mut app) {
            Ok(()) => app.report.unwrap_or_else(|| {
                environment_blocked_report(app.request, "window report missing")
            }),
            Err(error) => environment_blocked_report(app.request, error.to_string()),
        }
    }

    struct RealWindowApp {
        request: NativePlayerWindowRunRequest,
        host: Option<RealWindowHost>,
        report: Option<NativeWindowHostReport>,
        pending_raw_input: Vec<RawInputEvent>,
        frame_id: u64,
        next_redraw_at: Instant,
        linked_modules: Arc<LinkedProjectRuntimeSet>,
        lifecycle: NativePlayerLifecycleState,
    }

    impl RealWindowApp {
        fn new(
            request: NativePlayerWindowRunRequest,
            linked_modules: Arc<LinkedProjectRuntimeSet>,
        ) -> Self {
            Self {
                request,
                host: None,
                report: None,
                pending_raw_input: Vec::new(),
                frame_id: 0,
                next_redraw_at: Instant::now(),
                linked_modules,
                lifecycle: NativePlayerLifecycleState::default(),
            }
        }

        fn request_immediate_redraw(&self) {
            if let Some(host) = &self.host {
                host.window.request_redraw();
            }
        }
    }

    impl ApplicationHandler for RealWindowApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.lifecycle.resume();
            if self.host.is_some() {
                self.request_immediate_redraw();
                return;
            }
            match RealWindowHost::new(
                event_loop,
                self.request.clone(),
                Arc::clone(&self.linked_modules),
            ) {
                Ok(host) => {
                    host.window.request_redraw();
                    self.next_redraw_at = Instant::now() + REAL_WINDOW_FRAME_INTERVAL;
                    self.host = Some(host);
                }
                Err(error) => {
                    self.report = Some(environment_blocked_report(self.request.clone(), error));
                    event_loop.exit();
                }
            }
        }

        fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
            self.lifecycle.suspend();
            self.pending_raw_input.clear();
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::RedrawRequested => {
                    if let Some(host) = &mut self.host {
                        let raw_input = std::mem::take(&mut self.pending_raw_input);
                        match host.present_next_frame(raw_input) {
                            RealWindowFrameAdvance::Continue => {
                                self.next_redraw_at = Instant::now() + REAL_WINDOW_FRAME_INTERVAL;
                            }
                            RealWindowFrameAdvance::Complete => {
                                self.report = Some(host.finish(false));
                                event_loop.exit();
                            }
                        }
                    }
                }
                WindowEvent::CloseRequested => {
                    if let Some(host) = &mut self.host {
                        self.report = Some(host.finish(true));
                    }
                    event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    if let Some(host) = &mut self.host {
                        host.resize_surface(size.width, size.height);
                        host.window.request_redraw();
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    self.frame_id += 1;
                    if let Some(raw_event) = raw_keyboard_event(self.frame_id, window_id, &event) {
                        self.pending_raw_input.push(raw_event);
                    }
                    if let Some(raw_event) = raw_text_input_event(self.frame_id, window_id, &event)
                    {
                        self.pending_raw_input.push(raw_event);
                    }
                    self.request_immediate_redraw();
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    self.frame_id += 1;
                    self.pending_raw_input
                        .push(RawInputEvent::modifiers_changed(
                            self.frame_id,
                            format!("{window_id:?}"),
                            active_modifier_names(modifiers.state()),
                        ));
                    self.request_immediate_redraw();
                }
                WindowEvent::Ime(ime) => {
                    self.frame_id += 1;
                    if let Some(raw_event) = raw_ime_event(self.frame_id, window_id, ime) {
                        self.pending_raw_input.push(raw_event);
                    }
                    self.request_immediate_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.frame_id += 1;
                    self.pending_raw_input.push(RawInputEvent::mouse_move(
                        self.frame_id,
                        format!("{window_id:?}"),
                        position.x as f32,
                        position.y as f32,
                    ));
                    self.request_immediate_redraw();
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if let Some(button) = runtime_pointer_button(button) {
                        self.frame_id += 1;
                        let raw_event = match state {
                            ElementState::Pressed => RawInputEvent::mouse_button_down(
                                self.frame_id,
                                format!("{window_id:?}"),
                                button,
                            ),
                            ElementState::Released => RawInputEvent::mouse_button_up(
                                self.frame_id,
                                format!("{window_id:?}"),
                                button,
                            ),
                        };
                        self.pending_raw_input.push(raw_event);
                        self.request_immediate_redraw();
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    self.frame_id += 1;
                    self.pending_raw_input.push(RawInputEvent::mouse_wheel(
                        self.frame_id,
                        format!("{window_id:?}"),
                        mouse_wheel_delta(delta),
                    ));
                    self.request_immediate_redraw();
                }
                WindowEvent::Touch(touch) => {
                    self.frame_id += 1;
                    let phase = match touch.phase {
                        TouchPhase::Started => NativePrimaryTouchPhase::Started,
                        TouchPhase::Moved => NativePrimaryTouchPhase::Moved,
                        TouchPhase::Ended => NativePrimaryTouchPhase::Ended,
                        TouchPhase::Cancelled => NativePrimaryTouchPhase::Cancelled,
                    };
                    self.pending_raw_input.push(primary_touch_raw_event(
                        self.frame_id,
                        format!("{window_id:?}"),
                        touch.id,
                        phase,
                        touch.location.x as f32,
                        touch.location.y as f32,
                    ));
                    self.request_immediate_redraw();
                }
                WindowEvent::Focused(false) => {
                    self.frame_id += 1;
                    self.pending_raw_input.push(RawInputEvent::focus_lost(
                        self.frame_id,
                        format!("{window_id:?}"),
                    ));
                    self.request_immediate_redraw();
                }
                _ => {}
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if !self.lifecycle.should_present() {
                event_loop.set_control_flow(ControlFlow::Wait);
                return;
            }
            let now = Instant::now();
            if now >= self.next_redraw_at {
                if let Some(host) = &self.host {
                    host.window.request_redraw();
                }
                self.next_redraw_at = now + REAL_WINDOW_FRAME_INTERVAL;
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_redraw_at));
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RealWindowFrameAdvance {
        Continue,
        Complete,
    }

    struct RealWindowSession {
        package: RuntimePackage,
        host: EngineHostLoop,
        ui_state_producer: Box<dyn ProjectUiStateSnapshotProducer>,
        world: World,
        hydrator: RuntimeSceneHydrator,
        sprite_texture_bindings: Sprite2DTextureBindingContext,
        runtime_texture_bindings: RuntimeTextureBindingContext,
        aui_present_cache: NativeAuiPresentCache,
    }

    pub(super) fn player_wgpu_backends(is_android: bool, is_x86_64: bool) -> wgpu::Backends {
        if is_android && is_x86_64 {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::PRIMARY
        }
    }

    struct RealWindowHost {
        request: NativePlayerWindowRunRequest,
        window: Arc<winit::window::Window>,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        surface_config: wgpu::SurfaceConfiguration,
        backend: engine_runtime::wgpu_backend::real::RealWgpuBackend,
        input_device_state: InputDeviceState,
        input_mapping: Option<InputMappingAsset>,
        aui_interaction_state: AuiInteractionState,
        linked_modules: Arc<LinkedProjectRuntimeSet>,
        session: Option<RealWindowSession>,
        report: NativeWindowHostReport,
        frame_cpu_ns: Vec<u64>,
        frame_update_ns: Vec<u64>,
        frame_render_submit_ns: Vec<u64>,
        frame_present_wait_ns: Vec<u64>,
    }

    impl RealWindowHost {
        fn new(
            event_loop: &ActiveEventLoop,
            request: NativePlayerWindowRunRequest,
            linked_modules: Arc<LinkedProjectRuntimeSet>,
        ) -> Result<Self, String> {
            let attributes = winit::window::Window::default_attributes()
                .with_title(request.config.title.clone())
                .with_resizable(request.config.resizable)
                .with_inner_size(winit::dpi::PhysicalSize::new(
                    request.config.width,
                    request.config.height,
                ));
            let window = Arc::new(
                event_loop
                    .create_window(attributes)
                    .map_err(|error| format!("window.create_failed:{error}"))?,
            );
            fit_window_to_monitor_work_area(
                window.as_ref(),
                request.config.width,
                request.config.height,
            );
            let size = window.inner_size();
            if size.width == 0 || size.height == 0 {
                return Err("window.zero_sized_surface".to_string());
            }
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: player_wgpu_backends(
                    cfg!(target_os = "android"),
                    cfg!(target_arch = "x86_64"),
                ),
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                backend_options: wgpu::BackendOptions::default(),
            });
            let surface = instance
                .create_surface(window.clone())
                .map_err(|error| format!("surface.create_failed:{error}"))?;
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                }))
                .map_err(|error| format!("surface.request_adapter_failed:{error}"))?;
            let backend_name = format!("{:?}", adapter.get_info().backend);
            let (device, queue) = pollster::block_on(
                adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("runtime-player-wgpu-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                }),
            )
            .map_err(|error| format!("surface.request_device_failed:{error}"))?;
            let surface_config = surface
                .get_default_config(&adapter, size.width, size.height)
                .ok_or_else(|| "surface.default_config_unavailable".to_string())?;
            surface.configure(&device, &surface_config);
            let backend = engine_runtime::wgpu_backend::real::RealWgpuBackend::from_device_queue(
                device.clone(),
                queue,
                surface_config.format,
                size.width,
                size.height,
                backend_name.clone(),
            );
            let mut report = NativeWindowHostReport::base(&request);
            report.diagnostics.push(NativeWindowHostDiagnostic {
                severity: NativeWindowHostDiagnosticSeverity::Info,
                code: "graphics.backend_selected".to_string(),
                layer: "surface".to_string(),
                message: backend_name,
                path: None,
            });
            Ok(Self {
                request,
                window,
                surface,
                device,
                surface_config,
                backend,
                input_device_state: InputDeviceState::new(),
                input_mapping: None,
                aui_interaction_state: AuiInteractionState::default(),
                linked_modules,
                session: None,
                report,
                frame_cpu_ns: Vec::new(),
                frame_update_ns: Vec::new(),
                frame_render_submit_ns: Vec::new(),
                frame_present_wait_ns: Vec::new(),
            })
        }

        fn resize_surface(&mut self, width: u32, height: u32) {
            if width == 0 || height == 0 {
                return;
            }
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.report.surface.configured = true;
            self.report.surface.width = width;
            self.report.surface.height = height;
            self.report.surface.last_error = None;
        }

        fn ensure_session(&mut self) -> bool {
            if self.session.is_some() {
                return true;
            }
            self.report.window_status = "ok".to_string();
            self.report.surface_status = "ok".to_string();
            if !validate_run_evidence_request(&self.request, &mut self.report) {
                return false;
            }
            let size = self.window.inner_size();
            self.report.window = NativeWindowState {
                created: true,
                width: size.width,
                height: size.height,
                close_requested: false,
            };
            self.report.surface = NativeSurfaceState {
                created: true,
                configured: true,
                width: size.width,
                height: size.height,
                format: self.request.config.surface_format.clone(),
                present_mode: self.request.config.present_mode.clone(),
                acquired_frame_count: 0,
                presented_frame_count: 0,
                last_error: None,
            };

            let load = load_runtime_package(&self.request.runtime_package_path);
            self.report.diagnostics.extend(convert_runtime_diagnostics(
                "package",
                &load.diagnostics.issues,
            ));
            let Some(package) = load.value else {
                self.report.package_status = "error".to_string();
                self.report.present_status = NativeWindowPresentStatus::PackageFailed;
                return false;
            };
            self.report.package_status = "ok".to_string();
            self.report.aui.package_document_count = package.aui_manifest.documents.len();
            self.report.aui.loaded_document_count = package.aui_documents.len();
            let (sprite_texture_bindings, runtime_texture_bindings) =
                match prepare_real_gpu_resources(&mut self.backend, &package) {
                    Ok(bindings) => bindings,
                    Err(error) => {
                        self.report.render_status = "error".to_string();
                        self.report.rhi_status = "error".to_string();
                        self.report.present_status = NativeWindowPresentStatus::RhiFailed;
                        self.report
                            .diagnostics
                            .push(NativeWindowHostDiagnostic::error(
                                "runtime_gpu_resource_prepare_failed",
                                "render",
                                error,
                            ));
                        return false;
                    }
                };
            let bound_runtime = match ProjectRuntimeBootstrap::bind(&package, &self.linked_modules)
            {
                Ok(bound_runtime) => bound_runtime,
                Err(error) => {
                    self.report.logic_status = "error".to_string();
                    push_project_runtime_error(&mut self.report, error);
                    self.report.present_status = NativeWindowPresentStatus::PackageFailed;
                    return false;
                }
            };
            let NativePlayerRuntimeComposition {
                mut host,
                ui_state_producer,
                input_mapping,
                receipt,
            } = NativePlayerRuntimeComposition::from_bound(
                package.active_scene.id.clone(),
                bound_runtime.into_parts(),
            );
            host.set_game_view_target(self.request.game_view_target);
            self.input_mapping = Some(input_mapping);
            self.report.project_runtime_bind_receipt = Some(receipt);
            let Some((world, hydrator)) =
                hydrate_active_scene_for_player(&package, &mut self.report)
            else {
                self.report.scene_status = "error".to_string();
                self.report.world_status = "error".to_string();
                self.report.present_status = NativeWindowPresentStatus::SceneFailed;
                return false;
            };
            self.report.scene_status = "ok".to_string();
            self.report.world_status = "ok".to_string();
            self.session = Some(RealWindowSession {
                aui_present_cache: NativeAuiPresentCache::for_package(&package),
                package,
                host,
                ui_state_producer,
                world,
                hydrator,
                sprite_texture_bindings,
                runtime_texture_bindings,
            });
            true
        }

        fn present_next_frame(&mut self, raw_input: Vec<RawInputEvent>) -> RealWindowFrameAdvance {
            if !self.ensure_session() {
                return RealWindowFrameAdvance::Complete;
            }
            let frame_index = self.report.frames_completed.saturating_add(1);
            if frame_index > self.request.frame_limit {
                return RealWindowFrameAdvance::Complete;
            }
            let frame_started = Instant::now();
            let size = self.window.inner_size();
            if size.width == 0 || size.height == 0 {
                return RealWindowFrameAdvance::Continue;
            }

            let session = self
                .session
                .as_mut()
                .expect("successful session initialization stores runtime state");
            let mut input_events = scripted_input_events(&self.request, frame_index);
            let canvas_references = session
                .package
                .aui_manifest
                .documents
                .first()
                .and_then(|entry| session.package.aui_documents.get(&entry.document_id))
                .map(|document| {
                    document
                        .canvases
                        .iter()
                        .map(|canvas| {
                            CanvasReferenceFact::new(
                                canvas.canvas_id.clone(),
                                canvas.reference_resolution.x.round().max(1.0) as u32,
                                canvas.reference_resolution.y.round().max(1.0) as u32,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let target_extent = self.request.game_view_target.extent;
            let surface_extent = GameViewExtent::new(size.width, size.height);
            let presentation = match GameViewPresentationModule::resolve(GameViewPresentationSpec {
                session_id: "native-player-window".to_string(),
                target_id: self.request.config.target_id.clone(),
                target_extent,
                display_rect: GameViewRect::from_extent(surface_extent),
                scale_policy: self.request.game_view_target.scale_policy,
                surface_generation: 1,
                presentation_revision: frame_index,
                canvas_references,
            }) {
                Ok(presentation) => Arc::new(presentation),
                Err(error) => {
                    self.report.render_status = "error".to_string();
                    self.report.input_status = "error".to_string();
                    self.report
                        .diagnostics
                        .push(NativeWindowHostDiagnostic::error(
                            error.code,
                            "presentation",
                            "Windows Player GameView presentation could not be resolved.",
                        ));
                    return RealWindowFrameAdvance::Complete;
                }
            };
            let aui_present = session.aui_present_cache.take_or_rebuild(
                &session.package,
                &session.world,
                frame_index,
                session.ui_state_producer.as_mut(),
                Some(presentation.as_ref()),
            );
            let mut raw_input = raw_input;
            map_display_pointer_events_to_target(&mut raw_input, presentation.as_ref());
            input_events.extend(raw_input);
            let (action_snapshot, input_trace_summary, input_summary, aui_interaction) =
                resolve_native_input_frame_with_aui(
                    &mut self.input_device_state,
                    self.input_mapping
                        .as_ref()
                        .expect("project runtime binding supplies input mapping"),
                    "runtime-package",
                    Some("input/input-manifest.json"),
                    &input_events,
                    frame_index,
                    &self.request.config.window_id,
                    "winit",
                    "windows",
                    aui_present.as_ref(),
                    Some(presentation.as_ref()),
                    &mut self.aui_interaction_state,
                );
            self.report.input = input_summary;
            self.report.input_status = "ok".to_string();
            let mut frame_input = EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_action_snapshot(action_snapshot)
                .with_input_trace_summary(input_trace_summary);
            if let Some(aui_present) = aui_present.as_ref() {
                self.report.aui = NativeAuiPresentSummary::from_present_output(
                    &session.package,
                    aui_present,
                    self.request.config.target_id.as_str(),
                );
                if let Some(aui_interaction) = aui_interaction {
                    self.report
                        .aui
                        .apply_interaction_report(&aui_interaction.report);
                    self.report
                        .aui
                        .apply_navigation_screenflow_textentry_report(
                            &aui_interaction.navigation_screenflow_textentry_report,
                        );
                    frame_input = frame_input.with_aui_interaction(aui_interaction.result);
                }
                frame_input = frame_input
                    .with_aui_overlay(aui_present.overlay.clone())
                    .with_aui_composition(aui_present.composition.clone());
            } else if session.package.aui_manifest.documents.is_empty() {
                self.report.aui = NativeAuiPresentSummary::no_documents(0);
            } else {
                self.report.aui.status = "load_failed".to_string();
            }
            let output = session.host.tick_with_runtime_context(
                frame_input,
                &mut session.world,
                RuntimeFrameContext {
                    package: &session.package,
                    instance_loader: session.hydrator.instance_loader_mut(),
                },
            );
            self.report.project_runtime_session_report =
                output.project_runtime_session_report.clone();
            self.frame_update_ns
                .push(frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
            let render_started = Instant::now();
            record_runtime_trace(
                &mut self.report,
                &output.runtime_trace,
                self.request.runtime_report_level,
            );
            if output.runtime_advanced {
                self.report.logic_status = "ok".to_string();
            }
            if output.render_frame_report.is_some() {
                self.report.render_status = "ok".to_string();
            }
            let render_target = RenderTarget::surface(
                self.request.config.target_id.clone(),
                size.width,
                size.height,
            )
            .with_presentation_scale_policy(self.request.game_view_target.scale_policy);
            let render_thread = session
                .host
                .render_thread_for_target_with_runtime_resources_and_presentation(
                    render_target,
                    aui_present.as_ref().map(|present| &present.overlay),
                    aui_present.as_ref().map(|present| &present.composition),
                    Some(&session.sprite_texture_bindings),
                    Some(&session.runtime_texture_bindings),
                    Some(Arc::clone(&presentation)),
                );
            self.report
                .aui
                .apply_render_frame_report(&render_thread.renderer_output.render_frame_report);
            if should_capture_screenshot(&self.request, frame_index) {
                capture_screenshot(
                    &mut self.backend,
                    &render_thread.renderer_output.rhi_command_plan,
                    &self.request,
                    &mut self.report,
                    size.width,
                    size.height,
                );
            }
            session.aui_present_cache.store(aui_present);
            let render_prepare_ns = render_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            let present_wait_started = Instant::now();
            let surface_texture = match self.surface.get_current_texture() {
                Ok(surface_texture) => {
                    self.report.surface.last_error = None;
                    surface_texture
                }
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    let size = self.window.inner_size();
                    self.resize_surface(size.width, size.height);
                    return RealWindowFrameAdvance::Continue;
                }
                Err(wgpu::SurfaceError::Timeout) => {
                    self.report.surface.last_error = Some("surface.acquire_timeout".to_string());
                    return RealWindowFrameAdvance::Continue;
                }
                Err(error @ (wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other)) => {
                    self.report.surface.last_error =
                        Some(format!("surface.acquire_failed:{error}"));
                    self.report.present_status = NativeWindowPresentStatus::SurfaceFailed;
                    self.report
                        .diagnostics
                        .push(NativeWindowHostDiagnostic::error(
                            "surface_acquire_failed",
                            "surface",
                            format!("surface acquire failed: {error}"),
                        ));
                    return RealWindowFrameAdvance::Complete;
                }
            };
            let acquire_wait_ns = present_wait_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            self.report.surface.acquired_frame_count += 1;
            let view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let submit_started = Instant::now();
            #[cfg(target_os = "android")]
            let backend_report = self.backend.execute_plan_to_surface_view_in_rect(
                &render_thread.renderer_output.rhi_command_plan,
                &view,
                presentation.display_content_rect,
            );
            #[cfg(not(target_os = "android"))]
            let backend_report = self.backend.execute_plan_to_surface_view(
                &render_thread.renderer_output.rhi_command_plan,
                &view,
            );
            let submit_ns = submit_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            let present_started = Instant::now();
            surface_texture.present();
            let present_ns = present_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            self.report.surface.presented_frame_count += 1;
            self.report.render_thread_report_schema = Some(render_thread.report.schema_version);
            self.report.rhi_command_count = render_thread
                .renderer_output
                .rhi_command_plan
                .commands
                .len();
            self.report.rhi_status = if backend_report.diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.severity,
                    engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity::Error
                )
            }) {
                "error".to_string()
            } else {
                "ok".to_string()
            };
            self.report.frames_completed += 1;
            self.frame_render_submit_ns
                .push(render_prepare_ns.saturating_add(submit_ns));
            self.frame_present_wait_ns
                .push(acquire_wait_ns.saturating_add(present_ns));
            self.frame_cpu_ns
                .push(frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);

            if windowed_session_has_more_frames(
                self.report.frames_completed,
                self.request.frame_limit,
            ) {
                RealWindowFrameAdvance::Continue
            } else {
                RealWindowFrameAdvance::Complete
            }
        }

        fn finish(&mut self, close_requested: bool) -> NativeWindowHostReport {
            self.report.window.close_requested = close_requested;
            finalize_frame_performance(
                &mut self.report,
                &self.request,
                &self.frame_cpu_ns,
                &self.frame_update_ns,
                &self.frame_render_submit_ns,
                &self.frame_present_wait_ns,
            );
            if self.report.present_status == NativeWindowPresentStatus::NotPresented {
                self.report.present_status =
                    if self.report.frames_completed > 0 && self.report.rhi_status == "ok" {
                        NativeWindowPresentStatus::Presented
                    } else {
                        NativeWindowPresentStatus::RhiFailed
                    };
            }
            self.report.exit_code =
                if self.report.present_status == NativeWindowPresentStatus::Presented {
                    0
                } else {
                    1
                };
            self.report.clone()
        }

        #[allow(dead_code)]
        fn present_limited_frames(
            &mut self,
            raw_input: Vec<RawInputEvent>,
        ) -> NativeWindowHostReport {
            let mut report = NativeWindowHostReport::base(&self.request);
            report.window_status = "ok".to_string();
            report.surface_status = "ok".to_string();
            if !validate_run_evidence_request(&self.request, &mut report) {
                return report;
            }
            let size = self.window.inner_size();
            report.window = NativeWindowState {
                created: true,
                width: size.width,
                height: size.height,
                close_requested: false,
            };
            report.surface = NativeSurfaceState {
                created: true,
                configured: true,
                width: size.width,
                height: size.height,
                format: self.request.config.surface_format.clone(),
                present_mode: self.request.config.present_mode.clone(),
                acquired_frame_count: 0,
                presented_frame_count: 0,
                last_error: None,
            };

            let load = load_runtime_package(&self.request.runtime_package_path);
            report.diagnostics.extend(convert_runtime_diagnostics(
                "package",
                &load.diagnostics.issues,
            ));
            let Some(package) = load.value else {
                report.package_status = "error".to_string();
                report.present_status = NativeWindowPresentStatus::PackageFailed;
                return report;
            };
            report.package_status = "ok".to_string();
            report.aui.package_document_count = package.aui_manifest.documents.len();
            report.aui.loaded_document_count = package.aui_documents.len();
            let (sprite_texture_bindings, runtime_texture_bindings) =
                match prepare_real_gpu_resources(&mut self.backend, &package) {
                    Ok(bindings) => bindings,
                    Err(error) => {
                        report.render_status = "error".to_string();
                        report.rhi_status = "error".to_string();
                        report.present_status = NativeWindowPresentStatus::RhiFailed;
                        report.diagnostics.push(NativeWindowHostDiagnostic::error(
                            "runtime_gpu_resource_prepare_failed",
                            "render",
                            error,
                        ));
                        return report;
                    }
                };
            let bound_runtime = match ProjectRuntimeBootstrap::bind(&package, &self.linked_modules)
            {
                Ok(bound_runtime) => bound_runtime,
                Err(error) => {
                    report.logic_status = "error".to_string();
                    push_project_runtime_error(&mut report, error);
                    report.present_status = NativeWindowPresentStatus::PackageFailed;
                    return report;
                }
            };
            let NativePlayerRuntimeComposition {
                mut host,
                mut ui_state_producer,
                input_mapping,
                receipt,
            } = NativePlayerRuntimeComposition::from_bound(
                package.active_scene.id.clone(),
                bound_runtime.into_parts(),
            );
            self.input_mapping = Some(input_mapping);
            report.project_runtime_bind_receipt = Some(receipt);
            let Some((mut world, mut hydrator)) =
                hydrate_active_scene_for_player(&package, &mut report)
            else {
                report.scene_status = "error".to_string();
                report.world_status = "error".to_string();
                report.present_status = NativeWindowPresentStatus::SceneFailed;
                return report;
            };
            report.scene_status = "ok".to_string();
            report.world_status = "ok".to_string();
            let mut frame_cpu_ns = Vec::with_capacity(self.request.frame_limit as usize);
            let mut frame_update_ns = Vec::with_capacity(self.request.frame_limit as usize);
            let mut frame_render_submit_ns = Vec::with_capacity(self.request.frame_limit as usize);
            let mut frame_present_wait_ns = Vec::with_capacity(self.request.frame_limit as usize);
            let mut aui_present_cache = NativeAuiPresentCache::for_package(&package);
            for frame_index in 0..self.request.frame_limit {
                let frame_started = Instant::now();
                let mut input_events = scripted_input_events(&self.request, frame_index + 1);
                if frame_index == 0 {
                    input_events.extend(raw_input.iter().cloned());
                }
                let aui_present = aui_present_cache.take_or_rebuild(
                    &package,
                    &world,
                    frame_index + 1,
                    ui_state_producer.as_mut(),
                    None,
                );
                let (action_snapshot, input_trace_summary, input_summary, aui_interaction) =
                    resolve_native_input_frame_with_aui(
                        &mut self.input_device_state,
                        self.input_mapping
                            .as_ref()
                            .expect("project runtime binding supplies input mapping"),
                        "runtime-package",
                        Some("input/input-manifest.json"),
                        &input_events,
                        frame_index + 1,
                        &self.request.config.window_id,
                        "winit",
                        "windows",
                        aui_present.as_ref(),
                        None,
                        &mut self.aui_interaction_state,
                    );
                report.input = input_summary;
                report.input_status = "ok".to_string();
                let mut frame_input = EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_action_snapshot(action_snapshot)
                    .with_input_trace_summary(input_trace_summary);
                if let Some(aui_present) = aui_present.as_ref() {
                    report.aui = NativeAuiPresentSummary::from_present_output(
                        &package,
                        aui_present,
                        self.request.config.target_id.as_str(),
                    );
                    if let Some(aui_interaction) = aui_interaction {
                        report.aui.apply_interaction_report(&aui_interaction.report);
                        report.aui.apply_navigation_screenflow_textentry_report(
                            &aui_interaction.navigation_screenflow_textentry_report,
                        );
                        frame_input = frame_input.with_aui_interaction(aui_interaction.result);
                    }
                    frame_input = frame_input
                        .with_aui_overlay(aui_present.overlay.clone())
                        .with_aui_composition(aui_present.composition.clone());
                } else if package.aui_manifest.documents.is_empty() {
                    report.aui = NativeAuiPresentSummary::no_documents(0);
                } else {
                    report.aui.status = "load_failed".to_string();
                }
                let output = host.tick_with_runtime_context(
                    frame_input,
                    &mut world,
                    RuntimeFrameContext {
                        package: &package,
                        instance_loader: hydrator.instance_loader_mut(),
                    },
                );
                report.project_runtime_session_report =
                    output.project_runtime_session_report.clone();
                frame_update_ns
                    .push(frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
                let render_started = Instant::now();
                record_runtime_trace(
                    &mut report,
                    &output.runtime_trace,
                    self.request.runtime_report_level,
                );
                if output.runtime_advanced {
                    report.logic_status = "ok".to_string();
                }
                if output.render_frame_report.is_some() {
                    report.render_status = "ok".to_string();
                }
                let render_thread = host.render_thread_for_target_with_runtime_resources(
                    RenderTarget::surface(
                        self.request.config.target_id.clone(),
                        size.width,
                        size.height,
                    ),
                    aui_present.as_ref().map(|present| &present.overlay),
                    aui_present.as_ref().map(|present| &present.composition),
                    Some(&sprite_texture_bindings),
                    Some(&runtime_texture_bindings),
                );
                report
                    .aui
                    .apply_render_frame_report(&render_thread.renderer_output.render_frame_report);
                if should_capture_screenshot(&self.request, frame_index + 1) {
                    capture_screenshot(
                        &mut self.backend,
                        &render_thread.renderer_output.rhi_command_plan,
                        &self.request,
                        &mut report,
                        size.width,
                        size.height,
                    );
                }
                aui_present_cache.store(aui_present);
                let render_prepare_ns = render_started
                    .elapsed()
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                let present_wait_started = Instant::now();
                let surface_texture = match self.surface.get_current_texture() {
                    Ok(surface_texture) => surface_texture,
                    Err(error) => {
                        report.surface.last_error = Some(format!("surface.acquire_failed:{error}"));
                        report.present_status = NativeWindowPresentStatus::SurfaceFailed;
                        report.diagnostics.push(NativeWindowHostDiagnostic::error(
                            "surface_acquire_failed",
                            "surface",
                            format!("surface acquire failed: {error}"),
                        ));
                        return report;
                    }
                };
                let acquire_wait_ns = present_wait_started
                    .elapsed()
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                report.surface.acquired_frame_count += 1;
                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let submit_started = Instant::now();
                let backend_report = self.backend.execute_plan_to_surface_view(
                    &render_thread.renderer_output.rhi_command_plan,
                    &view,
                );
                let submit_ns = submit_started
                    .elapsed()
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                let present_started = Instant::now();
                surface_texture.present();
                let present_ns = present_started
                    .elapsed()
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                report.surface.presented_frame_count += 1;
                report.render_thread_report_schema = Some(render_thread.report.schema_version);
                report.rhi_command_count = render_thread
                    .renderer_output
                    .rhi_command_plan
                    .commands
                    .len();
                report.rhi_status = if backend_report.diagnostics.iter().any(|diagnostic| {
                    matches!(
                        diagnostic.severity,
                        engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity::Error
                    )
                }) {
                    "error".to_string()
                } else {
                    "ok".to_string()
                };
                report.frames_completed += 1;
                frame_render_submit_ns.push(render_prepare_ns.saturating_add(submit_ns));
                frame_present_wait_ns.push(acquire_wait_ns.saturating_add(present_ns));
                frame_cpu_ns
                    .push(frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
            }
            finalize_frame_performance(
                &mut report,
                &self.request,
                &frame_cpu_ns,
                &frame_update_ns,
                &frame_render_submit_ns,
                &frame_present_wait_ns,
            );
            report.present_status = if report.rhi_status == "ok" {
                NativeWindowPresentStatus::Presented
            } else {
                NativeWindowPresentStatus::RhiFailed
            };
            report.exit_code = if report.present_status == NativeWindowPresentStatus::Presented {
                0
            } else {
                1
            };
            report
        }
    }

    #[cfg(target_os = "windows")]
    fn fit_window_to_monitor_work_area(
        window: &winit::window::Window,
        requested_width: u32,
        requested_height: u32,
    ) {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };

        let Ok(window_handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        if monitor.is_null() {
            return;
        }
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dwFlags: 0,
        };
        if unsafe { GetMonitorInfoW(monitor, &mut monitor_info) } == 0 {
            return;
        }

        let work_width = monitor_info
            .rcWork
            .right
            .saturating_sub(monitor_info.rcWork.left)
            .max(1) as u32;
        let work_height = monitor_info
            .rcWork
            .bottom
            .saturating_sub(monitor_info.rcWork.top)
            .max(1) as u32;
        let initial_inner = window.inner_size();
        let initial_outer = window.outer_size();
        let non_client_width = initial_outer.width.saturating_sub(initial_inner.width);
        let non_client_height = initial_outer.height.saturating_sub(initial_inner.height);
        let (fitted_width, fitted_height) = fit_player_window_inner_extent(
            requested_width,
            requested_height,
            work_width,
            work_height,
            non_client_width,
            non_client_height,
        );
        if initial_inner.width != fitted_width || initial_inner.height != fitted_height {
            let _ = window
                .request_inner_size(winit::dpi::PhysicalSize::new(fitted_width, fitted_height));
        }

        let fitted_outer = window.outer_size();
        let x =
            monitor_info.rcWork.left + (work_width.saturating_sub(fitted_outer.width) / 2) as i32;
        let y =
            monitor_info.rcWork.top + (work_height.saturating_sub(fitted_outer.height) / 2) as i32;
        window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    }

    #[cfg(not(target_os = "windows"))]
    fn fit_window_to_monitor_work_area(
        _window: &winit::window::Window,
        _requested_width: u32,
        _requested_height: u32,
    ) {
    }

    fn map_display_pointer_events_to_target(
        events: &mut Vec<RawInputEvent>,
        presentation: &ResolvedGameViewPresentation,
    ) {
        events.retain_mut(|event| {
            let (point, is_touch) = match &event.value {
                RawInputValue::Pointer { x, y } => (GameViewPoint::new(*x, *y), false),
                RawInputValue::Touch { x, y, .. } => (GameViewPoint::new(*x, *y), true),
                _ => return true,
            };
            let target = match presentation.display_to_target(point) {
                Ok(target) => target,
                Err(_) => return !is_touch,
            };
            match &mut event.value {
                RawInputValue::Pointer { x, y } | RawInputValue::Touch { x, y, .. } => {
                    *x = target.x;
                    *y = target.y;
                }
                _ => {}
            }
            true
        });
    }

    #[cfg(test)]
    #[test]
    fn dpi_surface_pointer_is_mapped_back_to_portrait_target() {
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "player-dpi-input".to_string(),
            target_id: "main-surface".to_string(),
            target_extent: GameViewExtent::new(720, 1280),
            display_rect: GameViewRect::new(0.0, 0.0, 900.0, 1600.0),
            scale_policy: engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: Vec::new(),
        })
        .unwrap();
        let mut events = vec![RawInputEvent::mouse_move(1, "main-window", 225.0, 1500.0)];

        map_display_pointer_events_to_target(&mut events, &presentation);

        assert_eq!(
            events[0].value,
            RawInputValue::Pointer {
                x: 180.0,
                y: 1200.0
            }
        );
    }

    #[cfg(test)]
    #[test]
    fn portrait_contain_drops_gutter_touch_and_maps_content_touch() {
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "player-android-gutter-input".to_string(),
            target_id: "main-surface".to_string(),
            target_extent: GameViewExtent::new(720, 1280),
            display_rect: GameViewRect::new(0.0, 0.0, 1080.0, 2400.0),
            scale_policy: engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: Vec::new(),
        })
        .unwrap();
        assert_eq!(
            presentation.display_content_rect,
            GameViewRect::new(0.0, 240.0, 1080.0, 1920.0)
        );
        let mut events = vec![
            RawInputEvent::touch_start(1, "main-window", 1, 540.0, 120.0),
            RawInputEvent::touch_start(1, "main-window", 2, 540.0, 1200.0),
            RawInputEvent::mouse_move(1, "main-window", 540.0, 120.0),
        ];

        map_display_pointer_events_to_target(&mut events, &presentation);

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].value,
            RawInputValue::Touch {
                touch_id: 2,
                x: 360.0,
                y: 640.0,
            }
        );
        assert_eq!(
            events[1].value,
            RawInputValue::Pointer { x: 540.0, y: 120.0 }
        );
    }

    fn prepare_real_gpu_resources(
        backend: &mut engine_runtime::wgpu_backend::real::RealWgpuBackend,
        package: &RuntimePackage,
    ) -> Result<(Sprite2DTextureBindingContext, RuntimeTextureBindingContext), String> {
        let sprite_refs = runtime_sprite_texture_asset_ids(package);
        let aui_refs = package
            .aui_documents
            .documents_by_id
            .values()
            .flat_map(|document| document.nodes.iter())
            .filter_map(|node| node.image.as_ref())
            .map(|image| image.asset_id.clone());
        let registry = RuntimeTextureUploadRegistry::load(
            &package.package_dir,
            &package.runtime_asset_index,
            sprite_refs.iter().cloned().chain(aui_refs),
        );
        if let Some(error) = registry.diagnostics().first() {
            return Err(format!(
                "aui_image.texture_not_resolved:{}:{:?}:{}",
                error.asset_ref_id, error.code, error.message
            ));
        }
        for upload in registry.uploads() {
            backend.register_rgba8_texture(
                upload.handle,
                upload.payload.width,
                upload.payload.height,
                &upload.payload.rgba8,
                &upload.payload.sampler,
            )?;
        }
        let runtime_bindings = registry.binding_context();
        let mut bindings = Sprite2DTextureBindingContext::new();
        for asset_id in sprite_refs {
            let binding = runtime_bindings
                .get(&asset_id)
                .ok_or_else(|| format!("runtime_texture.binding_missing:{asset_id}"))?;
            bindings.insert_texture_handle(asset_id, binding.handle, binding.sampler.clone());
        }
        if let Some(bundle) = package.font_bundles.default_bundle() {
            backend.register_font_texture_arrays(bundle)?;
        } else if let Some(atlas) = package.font_atlases.default_atlas() {
            backend.register_alpha8_texture(
                font_atlas_render_handle(&atlas.metadata.font_atlas_id),
                atlas.metadata.atlas_width,
                atlas.metadata.atlas_height,
                &atlas.atlas_alpha,
                "nearestClamp",
            )?;
        }
        Ok((bindings, runtime_bindings))
    }

    fn should_capture_screenshot(request: &NativePlayerWindowRunRequest, frame_index: u64) -> bool {
        if !request.screenshot.enabled {
            return false;
        }
        let target_frame = request
            .screenshot
            .frame_index
            .unwrap_or(request.frame_limit.max(1));
        frame_index == target_frame
    }

    fn capture_screenshot(
        backend: &mut engine_runtime::wgpu_backend::real::RealWgpuBackend,
        plan: &engine_runtime::rhi_command_plan::RhiCommandPlan,
        request: &NativePlayerWindowRunRequest,
        report: &mut NativeWindowHostReport,
        width: u32,
        height: u32,
    ) {
        let path = request.screenshot.path.clone().unwrap_or_else(|| {
            request
                .runtime_package_path
                .parent()
                .unwrap_or(&request.runtime_package_path)
                .join("reports")
                .join("windowed-player-screenshot.png")
        });
        let rgba = match backend.render_plan_to_rgba_bytes(plan, width, height) {
            Ok(rgba) => rgba,
            Err(error) => {
                report
                    .screenshot
                    .mark_failed(NativeWindowScreenshotStatus::ReadbackFailed);
                report.diagnostics.push(NativeWindowHostDiagnostic::error(
                    "screenshot_readback_failed",
                    "screenshot",
                    error,
                ));
                return;
            }
        };
        match write_rgba_png(&path, width, height, &rgba) {
            Ok(byte_size) => report
                .screenshot
                .mark_captured(&path, width, height, byte_size),
            Err(error) => {
                report
                    .screenshot
                    .mark_failed(NativeWindowScreenshotStatus::WriteFailed);
                report.diagnostics.push(NativeWindowHostDiagnostic::error(
                    "screenshot_write_failed",
                    "screenshot",
                    error,
                ));
            }
        }
    }

    fn raw_keyboard_event(
        frame_id: u64,
        window_id: winit::window::WindowId,
        event: &winit::event::KeyEvent,
    ) -> Option<RawInputEvent> {
        let key = key_name(&event.logical_key);
        if key == "Unidentified" {
            return None;
        }
        let mut raw_event = match event.state {
            ElementState::Pressed => {
                RawInputEvent::keyboard_down(frame_id, format!("{window_id:?}"), key)
            }
            ElementState::Released => {
                RawInputEvent::keyboard_up(frame_id, format!("{window_id:?}"), key)
            }
        };
        raw_event.is_repeat = event.repeat;
        Some(raw_event)
    }

    fn raw_text_input_event(
        frame_id: u64,
        window_id: winit::window::WindowId,
        event: &winit::event::KeyEvent,
    ) -> Option<RawInputEvent> {
        if event.state != ElementState::Pressed || event.repeat {
            return None;
        }
        let text = event.text.as_ref()?.as_str();
        if text.is_empty() || text.chars().any(char::is_control) {
            return None;
        }
        Some(RawInputEvent::text_input(
            frame_id,
            format!("{window_id:?}"),
            text,
        ))
    }

    fn raw_ime_event(
        frame_id: u64,
        window_id: winit::window::WindowId,
        ime: Ime,
    ) -> Option<RawInputEvent> {
        match ime {
            Ime::Enabled => None,
            Ime::Disabled => Some(RawInputEvent::ime_cancel(
                frame_id,
                format!("{window_id:?}"),
            )),
            Ime::Preedit(text, cursor) => {
                let (cursor_start, cursor_end) = cursor.unwrap_or((0, 0));
                Some(RawInputEvent::ime_preedit(
                    frame_id,
                    format!("{window_id:?}"),
                    text,
                    cursor_start,
                    cursor_end,
                ))
            }
            Ime::Commit(text) => Some(RawInputEvent::ime_commit(
                frame_id,
                format!("{window_id:?}"),
                text,
            )),
        }
    }

    fn active_modifier_names(modifiers: ModifiersState) -> Vec<&'static str> {
        let mut active = Vec::new();
        if modifiers.shift_key() {
            active.push("Shift");
        }
        if modifiers.control_key() {
            active.push("Control");
        }
        if modifiers.alt_key() {
            active.push("Alt");
        }
        if modifiers.super_key() {
            active.push("Logo");
        }
        active
    }

    fn key_name(key: &Key) -> String {
        match key {
            Key::Named(NamedKey::Space) => "Space".to_string(),
            Key::Named(named) => format!("{named:?}"),
            Key::Character(text) => text.to_ascii_uppercase(),
            Key::Unidentified(_) => "Unidentified".to_string(),
            Key::Dead(dead) => format!("Dead({dead:?})"),
        }
    }

    fn runtime_pointer_button(button: MouseButton) -> Option<engine_input::RuntimePointerButton> {
        match button {
            MouseButton::Left => Some(engine_input::RuntimePointerButton::Primary),
            MouseButton::Right => Some(engine_input::RuntimePointerButton::Secondary),
            MouseButton::Middle => Some(engine_input::RuntimePointerButton::Middle),
            _ => None,
        }
    }

    fn mouse_wheel_delta(delta: MouseScrollDelta) -> f32 {
        match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32,
        }
    }

    fn environment_blocked_report(
        request: NativePlayerWindowRunRequest,
        error: impl Into<String>,
    ) -> NativeWindowHostReport {
        let mut report = NativeWindowHostReport::base(&request);
        report.window_status = "environment_blocked".to_string();
        report.surface_status = "environment_blocked".to_string();
        report.present_status = NativeWindowPresentStatus::EnvironmentBlocked;
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "real_window_environment_blocked",
            "window",
            error.into(),
        ));
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_wgpu_backends_uses_gl_only_for_android_x86_64() {
        assert_eq!(
            real_window::player_wgpu_backends(true, true),
            wgpu::Backends::GL
        );
        assert_eq!(
            real_window::player_wgpu_backends(true, false),
            wgpu::Backends::PRIMARY
        );
        assert_eq!(
            real_window::player_wgpu_backends(false, true),
            wgpu::Backends::PRIMARY
        );
    }

    #[test]
    fn primary_touch_mapping_preserves_physical_pixels_and_cancel_phase() {
        let started = primary_touch_raw_event(
            7,
            "android-main",
            42,
            NativePrimaryTouchPhase::Started,
            123.5,
            456.25,
        );
        let cancelled = primary_touch_raw_event(
            8,
            "android-main",
            42,
            NativePrimaryTouchPhase::Cancelled,
            124.0,
            458.0,
        );

        assert_eq!(
            started.event_kind,
            engine_input::RawInputEventKind::TouchStart
        );
        assert_eq!(
            cancelled.event_kind,
            engine_input::RawInputEventKind::TouchCancel
        );
        assert_eq!(
            started.value,
            engine_input::RawInputValue::Touch {
                touch_id: 42,
                x: 123.5,
                y: 456.25,
            }
        );
    }

    #[test]
    fn suspend_resume_recreates_surface_without_recreating_gameplay_session() {
        let mut lifecycle = NativePlayerLifecycleState::default();
        lifecycle.resume();
        let session_generation = lifecycle.gameplay_session_generation;
        assert!(lifecycle.should_present());

        lifecycle.suspend();
        assert!(!lifecycle.should_present());
        lifecycle.resume();

        assert_eq!(lifecycle.surface_generation, 2);
        assert_eq!(lifecycle.gameplay_session_generation, session_generation);
    }
    use engine_runtime::aui::{
        AuiActionRef, AuiBindingRef, AuiBindingTarget, AuiBindingValue, AuiCanvas, AuiDocument,
        AuiNode, AuiNodeKind, AuiRect, ProjectUiStateIdentity, ProjectUiStateProducerContext,
        ProjectUiStateResolve, ProjectUiStateResolveError, ProjectUiStateSnapshot,
        ProjectUiStateSnapshotOutput,
    };
    use engine_runtime::font_bundle::RuntimeFontBundleRegistry;
    use engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn native_window_host_report_serializes() {
        let report = NativeWindowHostReport::base(
            &NativePlayerWindowRunRequest::headless_surface_gate("runtime-package"),
        );

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains(NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION));
        assert!(json.contains("native-player-window-host"));
    }

    #[test]
    fn player_ui_producer_receives_declared_binding_paths() {
        let document = AuiDocument::new(
            "player-bindings",
            Vec::new(),
            vec![
                AuiNode::new(
                    "grain",
                    AuiNodeKind::Text,
                    AuiRect::fixed_position(0.0, 0.0, 1.0, 1.0),
                )
                .with_binding(AuiBindingRef::new(
                    "grain-binding",
                    AuiBindingTarget::TextText,
                    "tower.military_grain_text",
                    None,
                )),
                AuiNode::new(
                    "reserve",
                    AuiNodeKind::Text,
                    AuiRect::fixed_position(0.0, 0.0, 1.0, 1.0),
                )
                .with_binding(AuiBindingRef::new(
                    "reserve-binding",
                    AuiBindingTarget::TextText,
                    "tower.reserve.0.text",
                    None,
                ))
                .with_binding(AuiBindingRef::new(
                    "grain-binding-duplicate",
                    AuiBindingTarget::TextText,
                    "tower.military_grain_text",
                    None,
                )),
            ],
        );

        assert_eq!(
            active_aui_binding_paths(&document),
            vec![
                "tower.military_grain_text".to_string(),
                "tower.reserve.0.text".to_string(),
            ]
        );
    }

    struct SequencedUiProducer {
        values: Vec<String>,
        call_count: usize,
    }

    impl ProjectUiStateSnapshotProducer for SequencedUiProducer {
        fn producer_id(&self) -> &str {
            "sequenced-test-ui"
        }

        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            let value_index = self.call_count.min(self.values.len().saturating_sub(1));
            self.call_count = self.call_count.saturating_add(1);
            ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::TestSnapshot,
                ProjectUiStateSnapshot::new(context.frame_index).with_value(
                    "game.score_text",
                    AuiBindingValue::String(self.values[value_index].clone()),
                ),
            )
        }

        fn resolve(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
            let identity = ProjectUiStateIdentity {
                producer_epoch: 1,
                visible_revision: if context.frame_index < 3 { 1 } else { 2 },
                binding_set: context.binding_set.identity().clone(),
            };
            if context.previous_identity.as_ref() == Some(&identity) {
                return Ok(ProjectUiStateResolve::Reuse { identity });
            }
            let output = self.produce(context);
            Ok(ProjectUiStateResolve::Replace { identity, output })
        }
    }

    #[test]
    fn aui_present_cache_reuses_clean_frame_and_rebuilds_changed_values() {
        let root = temp_root("aui-present-cache");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        add_minimal_aui_document(&package_dir);
        let load = load_runtime_package(package_dir);
        assert!(!load.diagnostics.has_errors(), "{:#?}", load.diagnostics);
        let package = load.value.expect("minimal AUI runtime package");
        let world = World::new();
        let mut producer = SequencedUiProducer {
            values: vec!["FPS: 60".to_string(), "FPS: 59".to_string()],
            call_count: 0,
        };
        let mut cache = NativeAuiPresentCache::for_package(&package);

        let first = cache
            .take_or_rebuild(&package, &world, 1, &mut producer, None)
            .expect("first present");
        let first_composition = first.composition.clone();
        cache.store(Some(first));
        let second = cache
            .take_or_rebuild(&package, &world, 2, &mut producer, None)
            .expect("cached present");
        assert_eq!(first_composition, second.composition);
        assert_eq!(cache.rebuild_count, 1);
        assert_eq!(cache.hit_count, 1);
        assert_eq!(cache.presentation_revision, 1);

        let second_document = second.resolved_document.clone();
        cache.store(Some(second));
        let third = cache
            .take_or_rebuild(&package, &world, 3, &mut producer, None)
            .expect("changed present");
        assert_ne!(second_document, third.resolved_document);
        assert_eq!(producer.call_count, 2);
        assert_eq!(cache.rebuild_count, 2);
        assert_eq!(cache.hit_count, 1);
        assert_eq!(cache.presentation_revision, 2);
    }

    #[test]
    fn runtime_sprite_texture_collection_includes_scene_and_all_animator_frames() {
        use engine_runtime::animator2d::{
            Animator2DPlayback, CookedAnimator2DRegistry, CookedSpriteAnimationClip2D,
            CookedSpriteAnimationFrame2D,
        };
        use engine_runtime::runtime_package::{RuntimeAssetRef, RuntimeSpriteRenderer2D};

        let root = temp_root("sprite-texture-collection");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let load = load_runtime_package(package_dir);
        assert!(
            !load.diagnostics.has_errors(),
            "diagnostics={:#?}",
            load.diagnostics
        );
        let mut package = load.value.expect("minimal runtime package");
        package.active_scene.entities[0].sprite_renderer2d = Some(RuntimeSpriteRenderer2D {
            sprite_ref: Some(RuntimeAssetRef {
                id: "enemy-idle".to_string(),
                asset_type: "texture".to_string(),
                guid: None,
                sub_asset: None,
            }),
            material_ref: None,
            color: None,
            flip_x: None,
            flip_y: None,
            sorting_layer: None,
            order_in_layer: None,
            sort_z: None,
            visible: None,
        });
        package.animator2d_registry = CookedAnimator2DRegistry::from_parts(
            vec![CookedSpriteAnimationClip2D {
                id: "enemy-move".to_string(),
                playback: Animator2DPlayback::Loop,
                frames: vec![
                    CookedSpriteAnimationFrame2D {
                        sprite_asset_id: "enemy-move-0".to_string(),
                        duration_ticks: 4,
                    },
                    CookedSpriteAnimationFrame2D {
                        sprite_asset_id: "enemy-move-1".to_string(),
                        duration_ticks: 4,
                    },
                ],
            }],
            Vec::new(),
        )
        .expect("Animator2D fixture registry");

        let asset_ids = runtime_sprite_texture_asset_ids(&package);

        assert_eq!(asset_ids.len(), 3);
        assert!(asset_ids.contains("enemy-idle"));
        assert!(asset_ids.contains("enemy-move-0"));
        assert!(asset_ids.contains("enemy-move-1"));
    }

    #[test]
    fn frame_performance_reports_update_render_submit_and_present_wait() {
        let mut request = NativePlayerWindowRunRequest::headless_surface_gate("runtime-package");
        request.performance_warmup_frames = 1;
        request.performance_sample_frames = 3;
        let mut report = NativeWindowHostReport::base(&request);
        finalize_frame_performance(
            &mut report,
            &request,
            &[9_000_000, 10_000_000, 20_000_000, 30_000_000],
            &[1_000_000, 2_000_000, 4_000_000, 6_000_000],
            &[3_000_000, 4_000_000, 8_000_000, 12_000_000],
            &[0, 1_000_000, 2_000_000, 3_000_000],
        );

        let performance = report.frame_performance_summary.unwrap();
        assert_eq!(performance.observed_sample_frames, 3);
        assert_eq!(performance.update.observed_sample_frames, 3);
        assert_eq!(performance.render_submit.observed_sample_frames, 3);
        assert_eq!(performance.present_wait.observed_sample_frames, 3);
        assert!((performance.update.mean_ms - 4.0).abs() < f64::EPSILON);
        assert!((performance.render_submit.mean_ms - 8.0).abs() < f64::EPSILON);
        assert!((performance.present_wait.mean_ms - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn screenshot_request_is_reported_without_running_real_window() {
        let request = NativePlayerWindowRunRequest::windowed("runtime-package")
            .with_screenshot("reports/screenshot.png");
        let report = NativeWindowHostReport::base(&request);

        assert!(report.screenshot.requested);
        assert_eq!(
            report.screenshot.status,
            NativeWindowScreenshotStatus::Unsupported
        );
        assert_eq!(
            report.screenshot.path.as_deref(),
            Some("reports/screenshot.png")
        );
    }

    #[test]
    fn windowed_session_advances_past_first_frame_until_limit() {
        assert!(windowed_session_has_more_frames(1, 3));
        assert!(windowed_session_has_more_frames(2, 3));
        assert!(!windowed_session_has_more_frames(3, 3));
        assert!(windowed_session_has_more_frames(1, u64::MAX));
    }

    #[test]
    fn player_target_updates_initial_window_extent() {
        let request = NativePlayerWindowRunRequest::windowed("runtime-package")
            .with_game_view_target(GameViewTargetSpec::portrait_720x1280());

        assert_eq!(request.config.width, 720);
        assert_eq!(request.config.height, 1280);
        assert_eq!(
            request.game_view_target,
            GameViewTargetSpec::portrait_720x1280()
        );
    }

    #[test]
    fn portrait_player_window_fits_inside_work_area_without_changing_aspect() {
        let fitted = fit_player_window_inner_extent(720, 1280, 2048, 1104, 16, 39);

        assert_eq!(fitted, (599, 1065));
        assert!((u64::from(fitted.0) * 1280).abs_diff(u64::from(fitted.1) * 720) <= 720);
    }

    #[test]
    fn player_window_keeps_requested_extent_when_work_area_can_contain_it() {
        assert_eq!(
            fit_player_window_inner_extent(1280, 720, 2048, 1104, 16, 39),
            (1280, 720)
        );
    }

    #[test]
    fn rgba_png_writer_outputs_png_file() {
        let root = temp_root("png-writer");
        let path = root.join("reports").join("screenshot.png");
        let rgba = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];

        let size = write_rgba_png(&path, 2, 2, &rgba).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len() as u64, size);
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn headless_native_player_runs_package_one_frame() {
        let root = temp_root("headless-one-frame");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let mut request = NativePlayerWindowRunRequest::headless_surface_gate(package);
        request.frame_limit = 1;

        let report = run_headless_native_player_from_package(request);

        assert_eq!(report.exit_code, 0, "diagnostics={:#?}", report.diagnostics);
        assert_eq!(report.package_status, "ok");
        assert_eq!(report.scene_status, "ok");
        assert_eq!(report.world_status, "ok");
        assert_eq!(report.logic_status, "ok");
        assert_eq!(report.render_status, "ok");
        assert_eq!(report.rhi_status, "ok");
        assert_eq!(report.present_status, NativeWindowPresentStatus::Presented);
        assert_eq!(report.frames_completed, 1);
        assert!(report.rhi_command_count > 0);
    }

    #[test]
    fn runtime_player_project_runtime_session_headless_uses_shared_composition_and_reports_summary()
    {
        let root = temp_root("project-runtime-session");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let mut request = NativePlayerWindowRunRequest::headless_surface_gate(package);
        request.frame_limit = 1;

        let report = run_headless_native_player_from_package(request);

        let receipt = report
            .project_runtime_bind_receipt
            .as_ref()
            .expect("project runtime bind receipt");
        let session = report
            .project_runtime_session_report
            .as_ref()
            .expect("project runtime session summary");
        assert_eq!(receipt.session_id, session.session_id);
        assert_eq!(receipt.session_status, "ready");
        assert_eq!(session.status, "advanced");
        assert_eq!(session.stages.len(), 1);
        assert_eq!(
            session.stages[0].stage,
            engine_runtime::project_runtime_session::ProjectRuntimeSessionStage::FixedUpdate
        );
        assert_eq!(session.stages[0].action_count, 0);
        assert_eq!(session.stages[0].staged_mutation_count, 0);
        assert_eq!(session.stages[0].committed_mutation_count, 0);
        assert_eq!(receipt.producer_id, "engine_empty_project_ui_state");
        assert_ne!(receipt.producer_id, receipt.session_id);
    }

    // Requires the complex-shooter project module, which is excluded from engine-only releases.
    #[cfg(any())]
    #[test]
    fn headless_player_builds_linked_static_rule_runner_from_package_rules() {
        let root = temp_root("headless-linked-rules");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        write_sample_rule_manifest(&package);
        let mut request = NativePlayerWindowRunRequest::headless_surface_gate(package);
        request.frame_limit = 1;

        let linked_modules = use_complex_shooter_module(&request.runtime_package_path);
        let report =
            run_headless_native_player_from_package_with_linked_modules(request, &linked_modules);

        assert_eq!(report.exit_code, 0);
        assert_eq!(report.logic_status, "ok");
        assert!(!report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_registered_rule"));
    }

    #[cfg(any())]
    #[test]
    fn headless_player_reports_missing_registered_rule_from_package_manifest() {
        let root = temp_root("headless-missing-rule");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        fs::write(
            package.join("rules").join("rule-manifest.json"),
            r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "rust-aot",
  "rules": [{
    "ruleId": "rule.not-linked",
    "phase": "Update",
    "enabled": true,
    "executor": "rustAot",
    "irSource": "Rules/not_linked.ir.json",
    "irHash": "sample-not-linked",
    "artifactId": "rule-artifact:rule.not-linked:sample-not-linked"
  }],
  "modules": [{
    "artifactId": "rule-artifact:rule.not-linked:sample-not-linked",
    "moduleKind": "staticRegistry",
    "path": "Rules/generated/sample_project_rules.rs"
  }]
}"#,
        )
        .unwrap();

        let linked_modules = use_complex_shooter_module(&package);
        let report = run_headless_native_player_from_package_with_linked_modules(
            NativePlayerWindowRunRequest::headless_surface_gate(package),
            &linked_modules,
        );

        assert_eq!(report.exit_code, 1);
        assert_eq!(
            report.logic_status, "error",
            "diagnostics={:#?}",
            report.diagnostics
        );
        assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "project_runtime.missing_linked_rule"
            && diagnostic.layer == "project_runtime"));
    }

    #[cfg(any())]
    #[test]
    fn headless_native_player_reports_aui_present_evidence() {
        let root = temp_root("headless-aui");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        add_minimal_aui_document(&package);

        let linked_modules = use_complex_shooter_module(&package);
        let report = run_headless_native_player_from_package_with_linked_modules(
            NativePlayerWindowRunRequest::headless_surface_gate(package),
            &linked_modules,
        );
        assert_eq!(report.exit_code, 0, "diagnostics={:#?}", report.diagnostics);
        assert_eq!(report.aui.package_document_count, 1);
        assert_eq!(report.aui.loaded_document_count, 1);
        assert!(report.aui.draw_item_count > 0);
        assert!(report.aui.text_command_count > 0);
        assert!(report.aui.ui_pass_inserted);
        assert!(!report.aui.glyph_present);
        assert_eq!(report.aui.snapshot_source, "project_producer");
        assert_eq!(
            report.aui.producer_id.as_deref(),
            Some("complex_shooter_runtime_ui_state")
        );
        assert!(report.aui.snapshot_value_count >= 1);
        assert!(report
            .aui
            .active_binding_paths
            .contains(&"game.score_text".to_string()));
        assert!(report
            .aui
            .produced_paths
            .contains(&"game.score_text".to_string()));
        assert_eq!(report.aui.cache_status, "miss");
        assert!(report
            .aui
            .cache_miss_paths
            .contains(&"game.score_text".to_string()));
        assert!(report
            .aui
            .source_paths
            .iter()
            .any(|path| path.contains("project.sessionState.score")));
        assert!(report
            .aui
            .declared_binding_paths
            .contains(&"game.score_text".to_string()));
        assert!(report.aui.missing_paths.is_empty());
        assert_eq!(report.aui.status, "partial");
        assert!(report
            .aui
            .next_actions
            .contains(&"runtime_text_glyph_present".to_string()));
    }

    #[test]
    fn headless_native_player_reports_missing_package() {
        let root = temp_root("missing-package");
        let report = run_headless_native_player_from_package(
            NativePlayerWindowRunRequest::headless_surface_gate(root.join("missing-package")),
        );

        assert_eq!(report.exit_code, 1);
        assert_eq!(report.package_status, "error");
        assert_eq!(
            report.present_status,
            NativeWindowPresentStatus::PackageFailed
        );
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.layer == "package"));
    }

    #[test]
    #[cfg(not(feature = "real-window"))]
    fn feature_disabled_windowed_reports_native_host_required() {
        let root = temp_root("feature-disabled");
        let package = write_minimal_runtime_package(&root, "runtime-package");

        let report = run_windowed_native_player_from_package(
            NativePlayerWindowRunRequest::windowed(package),
        );

        assert_eq!(report.exit_code, 1);
        assert_eq!(
            report.present_status,
            NativeWindowPresentStatus::FeatureNotEnabled
        );
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "native_window_feature_not_enabled"));
    }

    #[test]
    #[cfg(not(feature = "real-window"))]
    fn feature_disabled_windowed_screenshot_reports_unsupported() {
        let root = temp_root("feature-disabled-screenshot");
        let package = write_minimal_runtime_package(&root, "runtime-package");

        let report = run_windowed_native_player_from_package(
            NativePlayerWindowRunRequest::windowed(package)
                .with_screenshot(root.join("reports").join("window.png")),
        );

        assert_eq!(report.exit_code, 1);
        assert!(report.screenshot.requested);
        assert_eq!(
            report.screenshot.status,
            NativeWindowScreenshotStatus::Unsupported
        );
        assert!(report.screenshot.path.is_some());
    }

    #[test]
    fn native_input_space_key_generates_fire_action() {
        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::gameplay_default();
        let raw_events = vec![RawInputEvent::keyboard_down(1, "main-window", "Space")];

        let (snapshot, trace, summary) = resolve_native_input_frame(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
        );

        assert!(snapshot.button_pressed("action.fire"));
        assert_eq!(trace.action_count, snapshot.action_count());
        assert_eq!(summary.raw_event_count, 1);
        assert!(summary.last_action_ids.contains(&"action.fire".to_string()));
    }

    #[test]
    fn native_input_held_key_drives_axis_across_frames() {
        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::gameplay_default();
        let raw_events = vec![RawInputEvent::keyboard_down(1, "main-window", "D")];

        let (first_snapshot, _, _) = resolve_native_input_frame(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
        );
        let (second_snapshot, _, summary) = resolve_native_input_frame(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &[],
            2,
            "main-window",
            "headless-script",
            "test",
        );

        assert_eq!(
            first_snapshot.axis2("action.move"),
            Some(engine_input::Axis2 { x: 1.0, y: 0.0 })
        );
        assert_eq!(
            second_snapshot.axis2("action.move"),
            Some(engine_input::Axis2 { x: 1.0, y: 0.0 })
        );
        assert_eq!(summary.pressed_key_count, 1);
    }

    #[test]
    fn native_input_mouse_pointer_button_and_wheel_are_resolved() {
        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::new(
            "input.mouse-test",
            vec![
                engine_input::InputActionDefinition::new(
                    "action.fire",
                    engine_input::InputActionValueType::Button,
                ),
                engine_input::InputActionDefinition::new(
                    "action.pointer",
                    engine_input::InputActionValueType::Pointer,
                ),
                engine_input::InputActionDefinition::new(
                    "action.scroll",
                    engine_input::InputActionValueType::Axis1,
                ),
            ],
            vec![engine_input::InputContextDefinition::new("gameplay", 0)],
            vec![
                engine_input::InputBindingDefinition::new("gameplay", "action.fire", "mouse/Left"),
                engine_input::InputBindingDefinition::pointer("action.pointer"),
                engine_input::InputBindingDefinition::mouse_wheel("action.scroll"),
            ],
        );
        let raw_events = vec![
            RawInputEvent::mouse_move(1, "main-window", 32.0, 64.0),
            RawInputEvent::mouse_button_down(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
            RawInputEvent::mouse_wheel(1, "main-window", -1.0),
        ];

        let (snapshot, _, summary) = resolve_native_input_frame(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
        );

        assert_eq!(
            snapshot.pointer("action.pointer"),
            Some(engine_input::PointerPosition { x: 32.0, y: 64.0 })
        );
        assert!(snapshot.button_pressed("action.fire"));
        assert_eq!(
            snapshot.axis1("action.scroll"),
            Some(engine_input::Axis1 { value: -1.0 })
        );
        assert_eq!(
            summary.pointer_position,
            Some(engine_input::PointerPosition { x: 32.0, y: 64.0 })
        );
        assert_eq!(summary.pressed_mouse_button_count, 1);
    }

    #[test]
    fn aui_interaction_filters_mouse_fire_and_dispatches_click_action() {
        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::new(
            "input.mouse-fire",
            vec![engine_input::InputActionDefinition::new(
                "action.fire",
                engine_input::InputActionValueType::Button,
            )],
            vec![engine_input::InputContextDefinition::new("gameplay", 0)],
            vec![engine_input::InputBindingDefinition::new(
                "gameplay",
                "action.fire",
                "mouse/Left",
            )],
        );
        let aui_present = test_aui_present_output();
        let mut aui_state = AuiInteractionState::default();
        let raw_events = vec![
            RawInputEvent::mouse_move(1, "main-window", 120.0, 120.0),
            RawInputEvent::mouse_button_down(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
            RawInputEvent::mouse_button_up(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
        ];

        let (snapshot, trace, summary, interaction) = resolve_native_input_frame_with_aui(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
            Some(&aui_present),
            None,
            &mut aui_state,
        );

        let interaction = interaction.expect("aui interaction should run");
        assert!(!snapshot.button_pressed("action.fire"));
        assert_eq!(summary.raw_event_count, 3);
        assert_eq!(summary.runtime_event_count, 0);
        assert_eq!(
            trace.route_kind.as_deref(),
            Some("RuntimeInputFrameFilteredByAui")
        );
        assert_eq!(interaction.report.consumed_pointer_event_count, 3);
        assert!(
            interaction.report.input_event_count > interaction.report.filtered_input_event_count
        );
        assert!(interaction
            .result
            .actions
            .iter()
            .any(|action| action.action_id == "ui.pause"));
    }

    #[test]
    fn portrait_target_space_input_uses_shared_presentation_inverse() {
        use engine_runtime::game_view_presentation::{
            CanvasReferenceFact, GameViewExtent, GameViewPoint, GameViewPresentationModule,
            GameViewPresentationSpec, GameViewRect, GameViewScalePolicy,
        };

        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::gameplay_default();
        let aui_present = test_aui_present_output();
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "player-portrait-test".to_string(),
            target_id: "main-surface".to_string(),
            target_extent: GameViewExtent::new(720, 1280),
            display_rect: GameViewRect::new(0.0, 0.0, 720.0, 1280.0),
            scale_policy: GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![CanvasReferenceFact::new("main", 1280, 720)],
        })
        .expect("portrait presentation");
        let target = presentation
            .reference_to_target("main", GameViewPoint::new(170.0, 120.0))
            .expect("button center maps into target space");
        let raw_events = vec![
            RawInputEvent::mouse_move(1, "main-window", target.x, target.y),
            RawInputEvent::mouse_button_down(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
            RawInputEvent::mouse_button_up(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
        ];
        let mut aui_state = AuiInteractionState::default();

        let (_, _, _, interaction) = resolve_native_input_frame_with_aui(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
            Some(&aui_present),
            Some(&presentation),
            &mut aui_state,
        );

        let interaction = interaction.expect("AUI interaction should run");
        assert!(interaction
            .result
            .actions
            .iter()
            .any(|action| action.action_id == "ui.pause"));
    }

    #[test]
    fn aui_feedback_player_pointer_down_updates_same_frame_overlay() {
        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::gameplay_default();
        let mut present = test_aui_present_output();
        let original_button_rect = present
            .overlay
            .draw_items
            .iter()
            .find(|item| item.node_id == "pause_button")
            .unwrap()
            .rect;
        let mut interaction_state = AuiInteractionState::default();
        let raw_events = vec![
            RawInputEvent::mouse_move(1, "main-window", 120.0, 120.0),
            RawInputEvent::mouse_button_down(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
        ];
        let (_, _, _, interaction) = resolve_native_input_frame_with_aui(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
            Some(&present),
            None,
            &mut interaction_state,
        );
        let mut feedback_state = AuiControlFeedbackState::default();
        let feedback = AuiRuntimePresenter::apply_control_feedback_with_fonts(
            &mut present,
            &interaction.unwrap().result,
            &mut feedback_state,
            presentation_delta_us_from_seconds(
                engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME,
            ),
            &RuntimeAuiFontAtlasRegistry::empty("test"),
            &RuntimeFontBundleRegistry::default(),
        );
        let animated_button_rect = present
            .overlay
            .draw_items
            .iter()
            .find(|item| item.node_id == "pause_button")
            .unwrap()
            .rect;
        assert_eq!(feedback.overrides.len(), 1);
        assert!(animated_button_rect.width < original_button_rect.width);
        assert!(feedback
            .report
            .resolved_profile_ids
            .contains(engine_runtime::aui::AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID));
    }

    #[test]
    fn aui_text_ime_events_are_consumed_and_reported_by_native_input_chain() {
        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::gameplay_default();
        let aui_present = test_aui_text_entry_present_output();
        let mut aui_state = AuiInteractionState::default();
        let raw_events = vec![
            RawInputEvent::mouse_move(1, "main-window", 96.0, 96.0),
            RawInputEvent::mouse_button_down(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
            RawInputEvent::text_input(1, "main-window", "B"),
            RawInputEvent::ime_preedit(1, "main-window", "ni", 0, 2),
            RawInputEvent::ime_cancel(1, "main-window"),
            RawInputEvent::ime_commit(1, "main-window", "hao"),
            RawInputEvent::keyboard_down(1, "main-window", "Enter"),
            RawInputEvent::keyboard_up(1, "main-window", "Enter"),
        ];

        let (_snapshot, _trace, summary, interaction) = resolve_native_input_frame_with_aui(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
            Some(&aui_present),
            None,
            &mut aui_state,
        );

        let interaction = interaction.expect("aui interaction should run");
        let mut present_summary = NativeAuiPresentSummary::empty();
        present_summary.apply_interaction_report(&interaction.report);
        present_summary.apply_navigation_screenflow_textentry_report(
            &interaction.navigation_screenflow_textentry_report,
        );

        assert!(summary.runtime_event_count < raw_events.len());
        assert_eq!(
            interaction
                .navigation_screenflow_textentry_report
                .ime_platform_coverage,
            "schema_headless_and_winit_cmin"
        );
        assert_eq!(present_summary.aui_text_edit_session_count, 1);
        assert_eq!(present_summary.aui_text_changed_count, 2);
        assert_eq!(present_summary.aui_text_submitted_count, 1);
        assert_eq!(present_summary.aui_ime_preedit_count, 1);
        assert_eq!(present_summary.aui_ime_cancel_count, 1);
        assert_eq!(present_summary.aui_ime_commit_count, 1);
        assert!(present_summary.aui_action_prompt_reported);
        assert!(present_summary.gameplay_input_filtered);
    }

    #[test]
    fn native_input_focus_lost_releases_pressed_inputs() {
        let mut device_state = InputDeviceState::new();
        let mapping = InputMappingAsset::gameplay_default();
        let raw_events = vec![
            RawInputEvent::keyboard_down(1, "main-window", "Space"),
            RawInputEvent::mouse_button_down(
                1,
                "main-window",
                engine_input::RuntimePointerButton::Primary,
            ),
        ];
        let _ = resolve_native_input_frame(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &raw_events,
            1,
            "main-window",
            "headless-script",
            "test",
        );

        let (_snapshot, _trace, summary) = resolve_native_input_frame(
            &mut device_state,
            &mapping,
            "test-fixture",
            None,
            &[RawInputEvent::focus_lost(2, "main-window")],
            2,
            "main-window",
            "headless-script",
            "test",
        );

        assert!(!summary.focused);
        assert_eq!(summary.pressed_key_count, 0);
        assert_eq!(summary.pressed_mouse_button_count, 0);
    }

    #[test]
    fn headless_player_reports_runtime_package_input_mapping() {
        let root = temp_root("runtime-package-input");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let custom_mapping = InputMappingAsset::new(
            "input.project",
            vec![engine_input::InputActionDefinition::new(
                "action.launch",
                engine_input::InputActionValueType::Button,
            )],
            vec![engine_input::InputContextDefinition::new("gameplay", 0)],
            vec![engine_input::InputBindingDefinition::button(
                "action.launch",
                "KeyL",
            )],
        );
        fs::write(
            package.join("input").join("input-manifest.json"),
            r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.project",
  "mappings": [{ "id": "input.project", "path": "input/input.project.json", "enabled": true }]
}"#,
        )
        .unwrap();
        fs::write(
            package.join("input").join("input.project.json"),
            serde_json::to_string_pretty(&custom_mapping).unwrap(),
        )
        .unwrap();
        let manifest_path = package.join("manifest.json");
        let manifest_text = fs::read_to_string(&manifest_path).unwrap().replace(
            r#""input": { "path": "input/input-manifest.json", "defaultMappingId": "input.default", "mappingCount": 1 }"#,
            r#""input": { "path": "input/input-manifest.json", "defaultMappingId": "input.project", "mappingCount": 1 }"#,
        );
        fs::write(manifest_path, manifest_text).unwrap();

        let report = run_headless_native_player_from_package(
            NativePlayerWindowRunRequest::headless_surface_gate(package),
        );

        assert_eq!(report.package_status, "ok");
        assert_eq!(report.input.mapping_source, "runtime-package");
        assert_eq!(report.input.mapping_id.as_deref(), Some("input.project"));
        assert_eq!(report.input.mapping_status, "ok");
    }

    #[cfg(feature = "real-window")]
    #[test]
    #[ignore = "real OS window / GPU smoke gate is local-only"]
    fn real_windowed_player_smoke() {
        let root = temp_root("real-windowed-smoke");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let report = run_windowed_native_player_from_package(
            NativePlayerWindowRunRequest::windowed(package),
        );

        assert_eq!(
            report.schema_version,
            NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.present_status, NativeWindowPresentStatus::Presented);
    }

    #[cfg(feature = "real-window")]
    #[test]
    #[ignore = "real OS window / GPU screenshot smoke gate is local-only"]
    fn real_windowed_player_screenshot_smoke() {
        let root = temp_root("real-windowed-screenshot-smoke");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let screenshot = root.join("reports").join("windowed-player-screenshot.png");
        let request = NativePlayerWindowRunRequest::windowed(package).with_screenshot(&screenshot);

        let report = run_windowed_native_player_from_package(request);

        assert_eq!(
            report.schema_version,
            NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.present_status, NativeWindowPresentStatus::Presented);
        assert_eq!(
            report.screenshot.status,
            NativeWindowScreenshotStatus::Captured
        );
        assert!(screenshot.exists());
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("runtime-player-winit-{name}-{stamp}"))
    }

    fn test_aui_present_output() -> AuiRuntimePresentOutput {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["pause_button"]);
        let pause_button = AuiNode::new(
            "pause_button",
            AuiNodeKind::Button,
            AuiRect::fixed_position(80.0, 80.0, 180.0, 80.0),
        )
        .with_parent("root")
        .with_interactable(true)
        .with_action(AuiActionRef::click("ui.pause"));
        let document = AuiDocument::new(
            "runtime-interaction-test",
            vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
            vec![root, pause_button],
        );
        AuiRuntimePresenter::present_package_smoke(&document, 1)
    }

    fn test_aui_text_entry_present_output() -> AuiRuntimePresentOutput {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
            .with_children(["name_input"]);
        let input = AuiNode::new(
            "name_input",
            AuiNodeKind::InputField,
            AuiRect::fixed_position(80.0, 80.0, 220.0, 44.0),
        )
        .with_parent("root")
        .with_interactable(true)
        .with_text("A")
        .with_action(AuiActionRef::text_changed("ui.name_changed"))
        .with_action(AuiActionRef::text_submitted("ui.name_submitted"))
        .with_action(AuiActionRef::text_cancelled("ui.name_cancelled"));
        let mut canvas = AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root");
        canvas.default_focus_node_id = Some("name_input".to_string());
        let document = AuiDocument::new("runtime-text-entry-test", vec![canvas], vec![root, input]);
        AuiRuntimePresenter::present_package_smoke(&document, 1)
    }

    fn write_minimal_runtime_package(root: &Path, name: &str) -> PathBuf {
        let package_dir = root.join(name);
        fs::create_dir_all(package_dir.join("scenes")).unwrap();
        fs::create_dir_all(package_dir.join("assets")).unwrap();
        fs::create_dir_all(package_dir.join("input")).unwrap();
        fs::create_dir_all(package_dir.join("rules")).unwrap();
        fs::write(
            package_dir.join("manifest.json"),
            r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "project-runtime-player-test",
    "name": "Runtime Player Test",
    "version": "0.0.3",
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
    "id": "entity-main",
    "name": "Main Entity",
    "kind": "actor",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
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

    #[cfg(any())]
    fn use_complex_shooter_module(package_dir: &Path) -> LinkedProjectRuntimeSet {
        let manifest_path = package_dir.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["project"]["runtimeModule"] =
            serde_json::to_value(complex_shooter_project_runtime::project_runtime_descriptor())
                .unwrap();
        fs::write(
            manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        // SAFETY: the statically linked project exports a process-static API table.
        let api = unsafe { *complex_shooter_project_runtime::aife_project_runtime_entry_v1() };
        engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_api(api)
            .unwrap()
    }

    #[cfg(any())]
    fn write_sample_rule_manifest(package_dir: &Path) {
        fs::write(
            package_dir.join("rules").join("rule-manifest.json"),
            r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "rust-aot",
  "rules": [{
    "ruleId": "rule.player-move",
    "phase": "Update",
    "enabled": true,
    "executor": "rustAot",
    "irSource": "Rules/player_move.ir.json",
    "irHash": "sample-player-move",
    "artifactId": "rule-artifact:rule.player-move:sample-player-move"
  }, {
    "ruleId": "rule.fire-bullet",
    "phase": "Update",
    "enabled": true,
    "executor": "rustAot",
    "irSource": "Rules/fire_bullet.ir.json",
    "irHash": "sample-fire-bullet",
    "artifactId": "rule-artifact:rule.fire-bullet:sample-fire-bullet"
  }, {
    "ruleId": "rule.linear-motion",
    "phase": "Update",
    "enabled": true,
    "executor": "rustAot",
    "irSource": "Rules/linear_motion.ir.json",
    "irHash": "sample-linear-motion",
    "artifactId": "rule-artifact:rule.linear-motion:sample-linear-motion"
  }],
  "modules": [{
    "artifactId": "rule-artifact:rule.player-move:sample-player-move",
    "moduleKind": "staticRegistry",
    "path": "Rules/generated/sample_project_rules.rs"
  }, {
    "artifactId": "rule-artifact:rule.fire-bullet:sample-fire-bullet",
    "moduleKind": "staticRegistry",
    "path": "Rules/generated/sample_project_rules.rs"
  }, {
    "artifactId": "rule-artifact:rule.linear-motion:sample-linear-motion",
    "moduleKind": "staticRegistry",
    "path": "Rules/generated/sample_project_rules.rs"
  }]
}"#,
        )
        .unwrap();
    }

    fn add_minimal_aui_document(package_dir: &Path) {
        fs::create_dir_all(package_dir.join("aui").join("documents")).unwrap();
        let manifest_path = package_dir.join("manifest.json");
        let manifest_text = fs::read_to_string(&manifest_path).unwrap().replace(
            "\"input\": { \"path\": \"input/input-manifest.json\", \"defaultMappingId\": \"input.default\", \"mappingCount\": 1 },",
            "\"input\": { \"path\": \"input/input-manifest.json\", \"defaultMappingId\": \"input.default\", \"mappingCount\": 1 },\n  \"aui\": { \"path\": \"aui/aui-manifest.json\", \"documentCount\": 1 },",
        );
        fs::write(manifest_path, manifest_text).unwrap();
        fs::write(
            package_dir.join("aui").join("aui-manifest.json"),
            r#"{
  "schemaVersion": "runtime-aui-manifest.v1",
  "documents": [{
    "documentId": "hud",
    "path": "aui/documents/hud.aui.json",
    "canvasCount": 1,
    "nodeCount": 2,
    "bindingCount": 1,
    "actionCount": 0,
    "assetRefs": []
  }]
}"#,
        )
        .unwrap();
        fs::write(
            package_dir
                .join("aui")
                .join("documents")
                .join("hud.aui.json"),
            r##"{
  "schema_version": "aui-document.v2",
  "document_id": "hud",
  "canvases": [{
    "canvas_id": "main",
    "mode": "ScreenOverlay",
    "layer": 0,
    "sorting_order": 0,
    "reference_resolution": { "x": 1280.0, "y": 720.0 },
    "scale_mode": "ConstantPixelSize",
    "root_node": "root"
  }],
  "nodes": [{
    "node_id": "root",
    "name": "root",
    "kind": "Panel",
    "parent": null,
    "children": ["score"],
    "rect": {
      "anchor_min": { "x": 0.0, "y": 0.0 },
      "anchor_max": { "x": 1.0, "y": 1.0 },
      "offset_min": { "x": 0.0, "y": 0.0 },
      "offset_max": { "x": 0.0, "y": 0.0 },
      "pivot": { "x": 0.5, "y": 0.5 },
      "size": { "x": 0.0, "y": 0.0 }
    },
    "visible": true,
    "interactable": false,
    "consume_input": true,
    "style": { "color": "#101820", "text_color": null, "font_size": null },
    "text": null,
    "image": null,
    "progress_value": null,
    "binding_refs": [],
    "action_refs": []
  }, {
    "node_id": "score",
    "name": "score",
    "kind": "Text",
    "parent": "root",
    "children": [],
    "rect": {
      "anchor_min": { "x": 0.0, "y": 0.0 },
      "anchor_max": { "x": 0.0, "y": 0.0 },
      "offset_min": { "x": 24.0, "y": 24.0 },
      "offset_max": { "x": 0.0, "y": 0.0 },
      "pivot": { "x": 0.0, "y": 0.0 },
      "size": { "x": 260.0, "y": 36.0 }
    },
    "visible": true,
    "interactable": false,
    "consume_input": true,
    "style": { "color": null, "text_color": "#ffffff", "font_size": 24.0 },
    "text": "SCORE",
    "image": null,
    "progress_value": null,
    "binding_refs": [{
      "binding_id": "score.text",
      "target_field": "TextText",
      "path": "game.score_text",
      "fallback": "SCORE 000000"
    }],
    "action_refs": []
  }]
}"##,
        )
        .unwrap();
    }
}
