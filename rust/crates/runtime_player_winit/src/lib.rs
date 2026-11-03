#[cfg(feature = "real-window")]
mod geometry_loading;
pub mod measurement;
mod project_storage;
pub use project_storage::NativePlayerProjectStorage;
#[cfg(any(test, feature = "real-window"))]
mod memory_observation;
#[cfg(feature = "real-window")]
use engine_input::RawInputValue;
use engine_input::{
    ActionSnapshot, InputDeviceState, InputDeviceStateReport, InputMappingAsset, InputResolver,
    InputTraceSummary, RawInputEvent,
};
use engine_runtime::aui::{
    AuiImePlatformRequest, AuiInteractionConfig, AuiInteractionProductizationReport,
    AuiInteractionResult, AuiInteractionState, AuiInteractionSystem, AuiLayoutEngine,
    AuiRuntimeNavigationScreenFlowTextEntryProductizationReport, AuiRuntimePresentOutput,
    AuiRuntimePresentStatus, AuiRuntimePresenter, AuiSnapshotSource,
    ProjectUiStateSnapshotProducer,
};
use engine_runtime::aui_control_feedback::{
    presentation_delta_us_from_seconds, AuiControlFeedbackState,
};
use engine_runtime::aui_present_cache::{
    AuiPresentCache as NativeAuiPresentCache, AuiPresentContext as NativeAuiPresentContext,
};
use engine_runtime::aui_viewport::AuiViewportInput;
#[cfg(feature = "real-window")]
use engine_runtime::aui_viewport::AuiViewportResolver;
use engine_runtime::diagnostics::{DiagnosticSeverity, RuntimeDiagnostic};
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::frame_loop::RuntimeFrameContext;
#[cfg(feature = "real-window")]
use engine_runtime::game_view_presentation::{
    CanvasReferenceFact, GameViewExtent, GameViewPresentationModule, GameViewPresentationSpec,
    GameViewRect,
};
use engine_runtime::game_view_presentation::{
    GameViewPoint, GameViewTargetSpec, ResolvedGameViewPresentation,
};
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

#[cfg(all(any(windows, target_os = "android"), feature = "real-window"))]
mod device_audio;
pub mod engine_dll_execution;
pub mod semantic_outcome;
mod semantic_playtest;
pub use semantic_playtest::{
    PlaytestAssertionResult, PlaytestCaptureEvidence, SemanticPlaytestReport,
};

pub const NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION: &str = "native-window-host-report.v1";

#[cfg(any(test, feature = "real-window"))]
const REAL_WINDOW_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_micros(16_667);

#[cfg(any(test, feature = "real-window"))]
fn frame_deadline(frame_started: Instant, frame_finished: Instant) -> Instant {
    // Rendering (including any present wait) consumes this frame's budget.
    // An overrun is ready immediately, without accumulating catch-up frames.
    (frame_started + REAL_WINDOW_FRAME_INTERVAL).max(frame_finished)
}

#[cfg(any(test, feature = "real-window"))]
fn presentation_deadline(scheduled: Instant, finished: Instant, fps: u32) -> Instant {
    if fps == 0 {
        return finished;
    }
    (scheduled + std::time::Duration::from_secs_f64(1. / f64::from(fps))).max(finished)
}

#[cfg(any(test, feature = "real-window"))]
fn next_redraw_deadline(
    scheduled: Instant,
    started: Instant,
    finished: Instant,
    fps: u32,
    spatial: bool,
) -> Instant {
    if fps == 60 && !spatial {
        frame_deadline(started, finished)
    } else {
        presentation_deadline(scheduled, finished, fps)
    }
}

fn default_target_fps() -> u32 {
    60
}

#[cfg(any(test, feature = "real-window"))]
#[derive(Default)]
struct PresentationFixedClock {
    remainder: u128,
}
#[cfg(any(test, feature = "real-window"))]
impl PresentationFixedClock {
    fn advance(&mut self, elapsed: std::time::Duration) -> usize {
        self.remainder += elapsed
            .min(std::time::Duration::from_millis(250))
            .as_nanos()
            * 60;
        let ticks = self.remainder / 1_000_000_000;
        self.remainder %= 1_000_000_000;
        ticks as usize
    }
}

#[cfg(any(test, feature = "real-window"))]
#[derive(Default)]
struct FixedActionLatch {
    pressed: std::collections::BTreeMap<String, engine_input::InputActionState>,
}
#[cfg(any(test, feature = "real-window"))]
impl FixedActionLatch {
    fn resolve(&mut self, mut snapshot: ActionSnapshot, consume: bool) -> ActionSnapshot {
        for action in &snapshot.actions {
            if matches!(
                action.value,
                engine_input::ActionValue::Button {
                    phase: engine_input::ActionPhase::Pressed
                }
            ) {
                self.pressed
                    .insert(action.action_id.clone(), action.clone());
            }
        }
        if consume {
            for (id, action) in std::mem::take(&mut self.pressed) {
                snapshot.actions.retain(|current| current.action_id != id);
                snapshot.actions.push(action);
            }
        }
        snapshot
    }
}

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

#[cfg(any(test, feature = "real-window"))]
fn native_mouse_leave_raw_event(
    frame_id: u64,
    window_id: impl Into<String>,
    last_cursor_position: Option<(f32, f32)>,
) -> RawInputEvent {
    let (x, y) = last_cursor_position.unwrap_or((0.0, 0.0));
    RawInputEvent::mouse_leave(frame_id, window_id, x, y)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeImeCursorArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NativeImePlatformRequest {
    pub enabled: bool,
    pub node_id: Option<String>,
    pub cursor_area: Option<NativeImeCursorArea>,
}

pub fn native_ime_platform_request(
    request: &AuiImePlatformRequest,
    presentation: &ResolvedGameViewPresentation,
) -> NativeImePlatformRequest {
    let (Some(canvas_id), Some(rect)) = (request.canvas_id.as_deref(), request.cursor_rect) else {
        return NativeImePlatformRequest::default();
    };
    if !request.enabled {
        return NativeImePlatformRequest::default();
    }
    let Ok(top_left_target) =
        presentation.reference_to_target(canvas_id, GameViewPoint::new(rect.x, rect.y))
    else {
        return NativeImePlatformRequest::default();
    };
    let Ok(bottom_right_target) = presentation.reference_to_target(
        canvas_id,
        GameViewPoint::new(rect.x + rect.width, rect.y + rect.height),
    ) else {
        return NativeImePlatformRequest::default();
    };
    let Ok(top_left) = presentation.target_to_display(top_left_target) else {
        return NativeImePlatformRequest::default();
    };
    let Ok(bottom_right) = presentation.target_to_display(bottom_right_target) else {
        return NativeImePlatformRequest::default();
    };
    NativeImePlatformRequest {
        enabled: true,
        node_id: request.node_id.clone(),
        cursor_area: Some(NativeImeCursorArea {
            x: f64::from(top_left.x),
            y: f64::from(top_left.y),
            width: f64::from((bottom_right.x - top_left.x).max(1.0)),
            height: f64::from((bottom_right.y - top_left.y).max(1.0)),
        }),
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
    #[serde(default = "default_target_fps")]
    pub target_fps: u32,
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
            target_fps: 60,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeAuiPerformanceLoad {
    ThemeDpi,
    VirtualListLifecycle,
    Dpi100,
    Dpi150,
    Dpi200,
}

impl NativeAuiPerformanceLoad {
    /// Explicit qualification input; does not modify the OS/window scale setting.
    fn fixed_dpi_scale(self) -> Option<f64> {
        match self {
            Self::Dpi100 => Some(1.0),
            Self::Dpi150 => Some(1.5),
            Self::Dpi200 => Some(2.0),
            _ => None,
        }
    }
}

#[cfg(any(test, feature = "real-window"))]
fn gpu_performance_sample_frames(cpu_samples: u64) -> u64 {
    cpu_samples
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePlayerWindowRunRequest {
    /// Trusted launcher override. Identity always comes from the verified package project ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_storage: Option<NativePlayerProjectStorage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<measurement::MeasurementPolicy>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aui_performance_load: Option<NativeAuiPerformanceLoad>,
}

impl NativePlayerWindowRunRequest {
    pub fn measurement_policy(&self) -> measurement::MeasurementPolicy {
        self.measurement.unwrap_or(measurement::MeasurementPolicy {
            level: if self.performance_sample_frames == 0 {
                measurement::MeasurementLevel::Off
            } else {
                measurement::MeasurementLevel::Trace
            },
            first_frame: None,
            end_frame: None,
            native_events: self.performance_sample_frames > 0,
            allocation_frame: None,
            allocation_end_frame: None,
            allocation_summary: false,
            cpu_probe: false,
        })
    }

    pub fn validate_measurement(&self) -> Result<(), &'static str> {
        let policy = self.measurement_policy();
        policy.validate()?;
        if policy.allocation_summary
            && (self.mode != NativePlayerWindowRunMode::Windowed
                || self.performance_sample_frames == 0)
        {
            return Err("measurement_allocation_summary_requires_sampled_window");
        }
        for frame in policy.allocation_frames() {
            if self.mode != NativePlayerWindowRunMode::Windowed
                || frame <= self.performance_warmup_frames
                || frame
                    > self
                        .performance_warmup_frames
                        .saturating_add(self.performance_sample_frames)
                || frame > self.frame_limit
            {
                return Err("measurement_allocation_frame_requires_sampled_window");
            }
        }
        Ok(())
    }

    pub fn measurement_enabled(&self) -> bool {
        self.performance_sample_frames > 0 && self.measurement_policy().enabled()
    }

    pub fn cpu_sampling_enabled(&self) -> bool {
        self.performance_sample_frames > 0
            && (self.measurement_policy().enabled() || self.measurement_policy().cpu_probe)
    }

    pub fn with_measurement_policy(mut self, policy: measurement::MeasurementPolicy) -> Self {
        self.measurement = Some(policy);
        self
    }

    pub fn headless_surface_gate(runtime_package_path: impl Into<PathBuf>) -> Self {
        Self {
            project_storage: None,
            measurement: None,
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
            aui_performance_load: None,
        }
    }

    pub fn windowed(runtime_package_path: impl Into<PathBuf>) -> Self {
        Self {
            project_storage: None,
            measurement: None,
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
            aui_performance_load: None,
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

fn cpu_frame_history<T>(request: &NativePlayerWindowRunRequest) -> Vec<T> {
    // Request validation bounds enabled histories before any session is made.
    // Reserve before warmup so Vec growth is not measured as Player retention.
    let frames = if request.cpu_sampling_enabled() {
        request.frame_limit.min(measurement::MAX_CAPTURE_FRAMES)
    } else {
        0
    };
    Vec::with_capacity(frames as usize)
}

fn record_cpu_frame_sample<T>(enabled: bool, samples: &mut Vec<T>, sample: impl FnOnce() -> T) {
    if enabled {
        samples.push(sample());
    }
}

#[cfg(any(test, feature = "real-window"))]
fn windowed_session_has_more_frames(frames_completed: u64, frame_limit: u64) -> bool {
    frames_completed < frame_limit
}

#[cfg(any(test, feature = "real-window"))]
fn uses_presentation_elapsed(target_fps: u32, spatial: bool, semantic: bool) -> bool {
    (target_fps != 60 || spatial) && !semantic
}

pub const NATIVE_PLAYER_INPUT_SCRIPT_SCHEMA_VERSION: &str = "native-player-input-script.v1";
pub const NATIVE_PLAYER_INPUT_SCRIPT_V2_SCHEMA_VERSION: &str = "native-player-input-script.v2";
pub mod replay_input;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePlayerInputScript {
    pub schema_version: String,
    pub script_id: String,
    #[serde(deserialize_with = "deserialize_script_frames")]
    pub frames: Vec<NativePlayerInputScriptFrame>,
}

fn deserialize_script_frames<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<NativePlayerInputScriptFrame>, D::Error> {
    replay_input::deserialize_points(deserializer, "frames")
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
        if self.schema_version != NATIVE_PLAYER_INPUT_SCRIPT_SCHEMA_VERSION
            && self.schema_version != NATIVE_PLAYER_INPUT_SCRIPT_V2_SCHEMA_VERSION
        {
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
        let ordered = self.schema_version == NATIVE_PLAYER_INPUT_SCRIPT_V2_SCHEMA_VERSION;
        let mut state = replay_input::ReplayInputState::default();
        if ordered && self.frames.len() > semantic_outcome::MAX_PLAYTEST_INPUTS {
            return Err("native-player-input-script.v2: /frames exceeds 4096 input points".into());
        }
        for (i, frame) in self.frames.iter().enumerate() {
            if ordered {
                if frame.frame_index > semantic_outcome::MAX_PLAYTEST_FRAMES {
                    return Err(format!(
                        "/frames/{i}/frameIndex exceeds {}",
                        semantic_outcome::MAX_PLAYTEST_FRAMES
                    ));
                }
                if !frame.key_down.is_empty()
                    || !frame.key_up.is_empty()
                    || frame.pointer_position.is_some()
                {
                    return Err(format!("/frames/{i}: v2 cannot contain legacy fields"));
                }
                let events = frame.events.as_ref().ok_or_else(|| {
                    format!("/frames/{i}/events: native-player-input-script.v2 requires events")
                })?;
                state
                    .validate(events, &format!("/frames/{i}/events"))
                    .map_err(|(path, message)| {
                        format!("native-player-input-script.v2: {path}: {message}")
                    })?;
            } else if frame.events.is_some() {
                return Err(format!(
                    "/frames/{i}/events: native-player-input-script.v1 does not accept events"
                ));
            }
        }
        if !state.released() {
            return Err("native-player-input-script.v2: /frames: release all keys and mouse buttons before the timeline ends".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePlayerInputScriptFrame {
    pub frame_index: u64,
    /// Target-canvas pixels; replay enters after the display/DPI coordinate mapping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_position: Option<[i32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_down: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_up: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<replay_input::ReplayInputEvent>>,
}

impl NativePlayerInputScriptFrame {
    pub fn keys(
        frame_index: u64,
        key_down: impl IntoIterator<Item = impl Into<String>>,
        key_up: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            frame_index,
            pointer_position: None,
            events: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_identity: Option<std::sync::Arc<engine_runtime::render_thread::RenderFrameIdentity>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission: Option<engine_runtime::render_thread::GpuSubmissionToken>,
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
            frame_identity: None,
            submission: None,
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

/// Explicit loading presentations are not game frames or performance samples.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGeometryLoadingSummary {
    pub presented_frames: u64,
    pub wait_wall_ns: u64,
    pub uploaded_bytes: u64,
    pub max_frame_uploaded_bytes: u64,
    pub peak_staging_bytes: u64,
    pub peak_in_flight: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowHostReport {
    /// Absent in historical reports; absence must not relabel an old Trace as Off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement_policy: Option<measurement::MeasurementPolicy>,
    #[serde(default)]
    pub session_exit_requested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_loading: Option<NativeGeometryLoadingSummary>,
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
    pub resource_memory_summary: Option<engine_runtime::resource_memory::ResourceMemoryRunSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gameplay_trace_summary: Option<WindowedPlayerGameplayTraceSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gameplay_trace_records: Vec<WindowedPlayerGameplayTraceRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_bind_receipt: Option<ProjectRuntimeBindReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_session_report: Option<ProjectRuntimeSessionFrameReport>,
    /// Existing project observation owner frozen once at Trace shutdown. Values
    /// retain their original runtime frame/contract; absence is not a zero reading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_project_observation:
        Option<engine_runtime::project_observation::ProjectRuntimeObservationState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_playtest: Option<SemanticPlaytestReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<engine_runtime::runtime_audio::RuntimeAudioReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spine: Option<engine_runtime::spine_animation::session::SpineObservation>,
    pub render_thread_report_schema: Option<String>,
    pub rhi_command_count: usize,
    pub exit_code: i32,
    pub diagnostics: Vec<NativeWindowHostDiagnostic>,
}

impl NativeWindowHostReport {
    pub fn base(request: &NativePlayerWindowRunRequest) -> Self {
        Self {
            session_exit_requested: false,
            geometry_loading: None,
            measurement_policy: Some(request.measurement_policy()),
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
            resource_memory_summary: None,
            audio: None,
            spine: None,
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
            final_project_observation: None,
            semantic_playtest: None,
            render_thread_report_schema: None,
            rhi_command_count: 0,
            exit_code: 1,
            diagnostics: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        self.logic_status == "error"
            || self
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|session| session.terminal_fault)
            || self
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == NativeWindowHostDiagnosticSeverity::Error)
    }

    fn record_project_runtime_session(
        &mut self,
        session: Option<ProjectRuntimeSessionFrameReport>,
    ) {
        if let Some(fault) = session.as_ref().filter(|session| session.terminal_fault) {
            self.logic_status = "error".into();
            for stage in &fault.stages {
                for code in &stage.diagnostics {
                    if !self
                        .diagnostics
                        .iter()
                        .any(|item| item.code == *code && item.layer == "project_runtime_session")
                    {
                        self.diagnostics.push(NativeWindowHostDiagnostic::error(
                            code,
                            "project_runtime_session",
                            format!(
                                "Project session {} failed at {:?} in frame {}.",
                                fault.session_id, stage.stage, fault.frame_index
                            ),
                        ));
                    }
                }
            }
            if !self
                .diagnostics
                .iter()
                .any(|item| item.layer == "project_runtime_session")
            {
                self.diagnostics.push(NativeWindowHostDiagnostic::error(
                    "project_runtime.session_faulted",
                    "project_runtime_session",
                    "Project session reached a terminal fault.",
                ));
            }
        }
        self.project_runtime_session_report = session;
    }

    fn mark_runtime_advanced(&mut self) {
        if self.logic_status != "error" {
            self.logic_status = "ok".into();
        }
    }

    fn completion_exit_code(&self) -> i32 {
        i32::from(
            self.has_errors()
                || (!self.session_exit_requested
                    && self.present_status != NativeWindowPresentStatus::Presented),
        )
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
        // C2 geometry (including retained translation) belongs to the presenter. A legacy
        // layout pass here both discards that geometry and adds an unnecessary full traversal.
        let legacy_layout = (aui_present.resolved_document.schema_version
            != engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION)
            .then(|| {
                AuiLayoutEngine::layout_with_interaction_state(
                    &aui_present.resolved_document,
                    frame_id,
                    aui_state,
                )
            });
        let layout_report = legacy_layout
            .as_ref()
            .map(|layout| &layout.report)
            .unwrap_or(&aui_present.layout.report);
        let report = AuiInteractionProductizationReport::from_result(
            &aui_present.resolved_document,
            runtime_frame.events.len(),
            filtered_frame.events.len(),
            &result,
            config,
            aui_state.active_drag_source().map(ToOwned::to_owned),
        )
        .with_focus_state(&aui_state.focus)
        .with_layout_report(layout_report);
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
    #[cfg(target_os = "windows")]
    if package.active_scene.entities.iter().any(|entity| {
        matches!(
            entity.spatial,
            Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D { .. })
        )
    }) {
        // The desktop 3D profile is selected before hydration creates consumer
        // leases. Existing full terrain meshes have ~42MiB JSON source and
        // <32MiB typed backing. Keep decode, aggregate in-flight and GPU upload
        // caps; admit these files through the bounded geometry decoder.
        let budget = engine_runtime::runtime_asset_loader::StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        };
        if let Err(error) = hydrator
            .instance_loader_mut()
            .asset_loader_mut()
            .set_streaming_budget(budget)
        {
            report.diagnostics.push(NativeWindowHostDiagnostic::error(
                "resource_profile_rejected",
                "scene",
                format!("Windows 3D resource profile rejected: {error:?}"),
            ));
            return None;
        }
    }
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

#[cfg(any(test, feature = "real-window"))]
fn defer_unready_geometry(
    error: Option<engine_runtime::render_contract::ContractError>,
    started: &mut Option<Instant>,
    now: Instant,
) -> Result<bool, &'static str> {
    if error.is_some_and(|e| {
        e.code == engine_runtime::render_contract::ContractErrorCode::ResourceNotReady
    }) {
        let first = started.get_or_insert(now);
        if now.saturating_duration_since(*first) >= std::time::Duration::from_secs(120) {
            return Err("three_d.resource_timeout");
        }
        Ok(true)
    } else {
        *started = None;
        Ok(false)
    }
}

#[cfg(all(target_os = "windows", feature = "real-window"))]
fn windows_3d_geometry_upload_budget(
    package: &RuntimePackage,
) -> Option<engine_runtime::wgpu_backend::real::GeometryUploadBudget> {
    package
        .active_scene
        .entities
        .iter()
        .any(|e| {
            matches!(
                e.spatial,
                Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D { .. })
            )
        })
        .then_some(engine_runtime::wgpu_backend::real::GeometryUploadBudget {
            resident_bytes: 4 * 1024 * 1024 * 1024,
            staging_bytes: 128 * 1024 * 1024,
            bytes_per_frame: 16 * 1024 * 1024,
            max_tickets: 256,
        })
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
    if let Err(code) = request.validate_measurement() {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            code,
            "request",
            "invalid measurement policy",
        ));
        return false;
    }
    if request
        .measurement
        .is_some_and(|p| p.enabled() || p.cpu_probe)
        && request.performance_sample_frames == 0
    {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "measurement_requires_samples",
            "request",
            "enabled measurement requires a nonzero sample count",
        ));
        return false;
    }
    if request.cpu_sampling_enabled() && request.frame_limit > measurement::MAX_CAPTURE_FRAMES {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "measurement_frame_limit_exceeded",
            "request",
            "measurement histories are limited to 262144 frames per request",
        ));
        return false;
    }
    let required_frames = request
        .performance_warmup_frames
        .saturating_add(request.performance_sample_frames);
    if request.cpu_sampling_enabled() && request.frame_limit < required_frames {
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
    if request.aui_performance_load.is_some()
        && (request.mode != NativePlayerWindowRunMode::Windowed
            || !request.measurement_enabled()
            || (request
                .aui_performance_load
                .and_then(NativeAuiPerformanceLoad::fixed_dpi_scale)
                .is_none()
                && request.runtime_report_level != WindowedPlayerRuntimeReportLevel::Trace))
    {
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "invalid_aui_performance_load",
            "request",
            "AUI performance load requires windowed mode and a non-zero performance sample; mutation loads also require Trace reporting.",
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
    let events = frame.events.clone().unwrap_or_else(|| {
        replay_input::legacy_events(&frame.key_down, &frame.key_up, frame.pointer_position)
    });
    events
        .iter()
        .map(|event| event.raw(frame_index, &request.config.window_id))
        .collect()
}

fn configure_runtime_audio(host: &mut EngineHostLoop, request: &NativePlayerWindowRunRequest) {
    let trace = request.runtime_report_level == WindowedPlayerRuntimeReportLevel::Trace;
    #[cfg(all(any(windows, target_os = "android"), feature = "real-window"))]
    if request.mode == NativePlayerWindowRunMode::Windowed {
        host.set_audio_output(Box::<device_audio::DeviceAudioOutput>::default(), trace);
        return;
    }
    host.set_audio_output(
        Box::<engine_runtime::runtime_audio::HeadlessAudioOutput>::default(),
        trace,
    );
}

fn record_runtime_audio(
    report: &mut NativeWindowHostReport,
    host: &EngineHostLoop,
    level: WindowedPlayerRuntimeReportLevel,
) {
    report.spine = host.observe_spine(match level {
        WindowedPlayerRuntimeReportLevel::Off => {
            engine_runtime::render_contract::DiagnosticLevel::Off
        }
        WindowedPlayerRuntimeReportLevel::Summary => {
            engine_runtime::render_contract::DiagnosticLevel::Summary
        }
        WindowedPlayerRuntimeReportLevel::Trace => {
            engine_runtime::render_contract::DiagnosticLevel::Trace
        }
    });
    let audio = host.audio_report();
    for diagnostic in &audio.diagnostics {
        if !report
            .diagnostics
            .iter()
            .any(|old| old.code == diagnostic.code && old.path == diagnostic.entity_id)
        {
            let mut error = NativeWindowHostDiagnostic::error(
                &diagnostic.code,
                "runtime_audio",
                &diagnostic.message,
            );
            error.path = diagnostic.entity_id.clone();
            report.diagnostics.push(error);
        }
    }
    if level != WindowedPlayerRuntimeReportLevel::Off
        && (audio.source_count > 0 || audio.play_count > 0 || !audio.diagnostics.is_empty())
    {
        report.audio = Some(audio);
    }
}

fn record_runtime_trace(
    report: &mut NativeWindowHostReport,
    trace: &engine_runtime::runtime_trace::RuntimeTrace,
    level: WindowedPlayerRuntimeReportLevel,
) {
    if trace
        .gameplay_records
        .iter()
        .any(|record| record.result != "ok")
    {
        report.logic_status = "error".into();
    }
    let Some(summary) = &mut report.gameplay_trace_summary else {
        return;
    };
    for record in &trace.gameplay_records {
        summary.record_count += 1;
        summary.write_count += usize::from(record.operation == "write" && record.result == "ok");
        summary.command_enqueue_count += usize::from(record.operation == "command_enqueue");
        summary.command_apply_count += usize::from(record.operation == "command_apply");
        summary.prefab_instantiate_apply_count += usize::from(
            record.operation == "command_apply" && record.source.is_some() && record.result == "ok",
        );
        summary.failed_record_count += usize::from(record.result != "ok");
        if record.result != "ok" {
            if summary.failure_details.len() < 16 {
                use engine_runtime::windowed_player::WindowedPlayerRuntimeFailure;
                let short = |value: &str| bounded_failure_text(value, 256);
                summary.failure_details.push(WindowedPlayerRuntimeFailure {
                    frame_index: record.frame_index,
                    phase: short(&record.phase),
                    rule_id: short(&record.rule_id),
                    operation: short(&record.operation),
                    entity_id: record.entity_id.as_ref().map(|id| short(id.as_str())),
                    component_type: record.component_type.as_ref().map(|ty| short(ty.as_str())),
                    field_path: record.field_path.as_deref().map(short),
                    error_code: record.error_code.as_deref().map(short),
                    message: record
                        .after
                        .as_deref()
                        .map(|value| bounded_failure_text(value, 1024)),
                });
            } else {
                summary.omitted_failure_count += 1;
            }
        }
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

fn bounded_failure_text(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut end = limit.saturating_sub(3);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &value[..end])
}

#[derive(Clone, Default)]
struct AuiFrameWorkCounters {
    #[cfg(any(test, feature = "real-window"))]
    tick_pair: Option<[u64; 2]>,
    spine: Option<engine_runtime::spine_animation::measurement::SpineCpuFrameSample>,
    cell_receipts: Vec<engine_runtime::windowed_player::WindowedPlayerCellReceipt>,
    spatial_gpu: Option<engine_runtime::windowed_player::WindowedPlayerSpatialGpuSample>,
    frame_identity: Option<std::sync::Arc<engine_runtime::render_thread::RenderFrameIdentity>>,
    process_memory: Option<engine_runtime::windowed_player::WindowedPlayerProcessMemorySample>,
    resource_memory: Option<engine_runtime::resource_memory::ResourceMemorySample>,
    sampling_wall_ns: Option<u64>,
    streaming_resources:
        Option<engine_runtime::windowed_player::WindowedPlayerStreamingResourceSample>,
    graphics_tessellation_builds: u64,
    graphics_cache_hits: u64,
    graphics_cache_resident_bytes: u64,
    input_dispatch_ime_ns: u64,
    work: engine_runtime::aui::AuiUpdateWork,
    frame_index: u64,
    timings: engine_runtime::aui::AuiUpdateTimings,
    drawing: engine_runtime::aui::AuiDrawingStats,
    feedback_drawing: engine_runtime::aui::AuiDrawingStats,
    feedback_cpu_ns: u64,
    feedback_source_nodes_visited: u64,
    projection: Option<engine_runtime::runtime_renderer::UiProjectionStats>,
    cpu_draw_buffer_prepare_ns: u64,
    ui_full_upload_cold_or_evicted: u64,
    ui_full_upload_untrusted_lineage: u64,
    present_rebuilds: u64,
    present_hits: u64,
    layout_nodes_visited: u64,
    layout_measure_calls: u64,
    layout_geometry_nodes: u64,
    geometry_nodes_updated: u64,
    geometry_source_nodes_compared: u64,
    geometry_snapshot_nodes_copied: u64,
    drawing_nodes_compared: u64,
    drawing_nodes_built: u64,
    drawing_nodes_reused: u64,
    drawing_payloads_built: u64,
    drawing_payloads_reused: u64,
    paragraph_cache_hits: u64,
    paragraph_cache_misses: u64,
    paragraph_reshapes: u64,
    paragraph_evictions: u64,
    paragraph_cache_resident_bytes: u64,
    extension_calls: u64,
    extension_request_bytes: u64,
    extension_response_bytes: u64,
    ui_uploads: u64,
    ui_full_uploads: u64,
    ui_partial_uploads: u64,
    ui_uploaded_bytes: u64,
    retained_geometry_uploaded_bytes: u64,
    retained_parameter_uploaded_bytes: u64,
    retained_cache_hits: u64,
    retained_draw_calls: u64,
    retained_instances: u64,

    ui_upload_hits: u64,
    ui_resident_bytes: u64,
    virtual_list_materialized_instances: u64,
    virtual_list_recycled_instances: u64,
    virtual_list_disposed_instances: u64,
    theme_change_events: u64,
    dpi_change_events: u64,
}

#[derive(Clone, Copy, Default)]
struct AuiGpuUploadCounters {
    cpu_draw_buffer_prepare_ns: u64,
    full_upload_cold_or_evicted: u64,
    full_upload_untrusted_lineage: u64,
    uploads: u64,
    full_uploads: u64,
    partial_uploads: u64,
    uploaded_bytes: u64,
    retained_geometry_uploaded_bytes: u64,
    retained_parameter_uploaded_bytes: u64,
    retained_cache_hits: u64,
    retained_draw_calls: u64,
    retained_instances: u64,
    hits: u64,
    resident_bytes: u64,
}

#[cfg(feature = "real-window")]
fn aui_gpu_upload_counters(
    stats: engine_runtime::wgpu_backend::real::UiVertexUploadStats,
    retained: engine_runtime::wgpu_backend::real::RetainedUiStats,
) -> AuiGpuUploadCounters {
    AuiGpuUploadCounters {
        cpu_draw_buffer_prepare_ns: stats.cpu_draw_buffer_prepare_ns,
        full_upload_cold_or_evicted: stats.full_upload_cold_or_evicted,
        full_upload_untrusted_lineage: stats.full_upload_untrusted_lineage,
        uploads: stats.uploads,
        full_uploads: stats.full_uploads,
        partial_uploads: stats.partial_uploads,
        uploaded_bytes: stats.uploaded_bytes,
        retained_geometry_uploaded_bytes: retained.geometry_uploaded_bytes,
        retained_parameter_uploaded_bytes: retained.parameter_uploaded_bytes,
        retained_cache_hits: retained.cache_hits,
        retained_draw_calls: retained.draws,
        retained_instances: retained.instances,

        hits: stats.hits,
        resident_bytes: stats.resident_bytes as u64,
    }
}

fn capture_aui_work_with_input(
    cache: &NativeAuiPresentCache,
    host: &EngineHostLoop,
    upload: Option<AuiGpuUploadCounters>,
    feedback: &AuiControlFeedbackState,
    frame_index: u64,
    input_dispatch_ime_ns: u64,
) -> AuiFrameWorkCounters {
    let runtime = cache.c2_runtime();
    let drawing = runtime.drawing_stats();
    let measured_work = runtime.update_work();
    let paragraph = runtime.paragraph_cache_stats();
    let graphics = runtime.graphics_cache_measurement();
    let extension = host.aui_extension_stats();
    let virtual_list = cache.virtual_list_lifecycle_stats();
    let upload = upload.unwrap_or_default();
    AuiFrameWorkCounters {
        #[cfg(any(test, feature = "real-window"))]
        tick_pair: None,
        spine: host.spine_cpu_measurement(),
        cell_receipts: Vec::new(),
        spatial_gpu: None,
        frame_identity: None,
        process_memory: None,
        resource_memory: None,
        sampling_wall_ns: None,
        streaming_resources: None,
        graphics_tessellation_builds: graphics.0,
        graphics_cache_hits: graphics.1,
        graphics_cache_resident_bytes: graphics.2 as u64,
        input_dispatch_ime_ns,
        work: runtime.update_work(),
        frame_index,
        timings: cache.update_timings(),
        drawing,
        feedback_drawing: feedback.drawing_stats(),
        feedback_cpu_ns: feedback.measurement().0,
        feedback_source_nodes_visited: feedback.measurement().1,
        projection: host.ui_projection_stats(),
        cpu_draw_buffer_prepare_ns: upload.cpu_draw_buffer_prepare_ns,
        ui_full_upload_cold_or_evicted: upload.full_upload_cold_or_evicted,
        ui_full_upload_untrusted_lineage: upload.full_upload_untrusted_lineage,
        present_rebuilds: cache.rebuild_count,
        present_hits: cache.hit_count,
        layout_nodes_visited: measured_work.layout_nodes_visited,
        layout_measure_calls: measured_work.layout_measure_calls,
        layout_geometry_nodes: measured_work.layout_geometry_nodes,
        geometry_nodes_updated: measured_work.geometry_nodes_updated,
        geometry_source_nodes_compared: measured_work.geometry_source_nodes_compared,
        geometry_snapshot_nodes_copied: measured_work.geometry_snapshot_nodes_copied,
        drawing_nodes_compared: drawing.nodes_compared,
        drawing_nodes_built: drawing.nodes_built,
        drawing_nodes_reused: drawing.nodes_reused,
        drawing_payloads_built: drawing
            .command_payloads_built
            .saturating_add(drawing.item_payloads_built)
            .saturating_add(drawing.text_items_built)
            .saturating_add(drawing.quad_payloads_built)
            .saturating_add(drawing.glyph_hash_leaves_built),
        drawing_payloads_reused: drawing
            .command_payloads_reused
            .saturating_add(drawing.item_payloads_reused)
            .saturating_add(drawing.text_items_reused)
            .saturating_add(drawing.quad_payloads_reused)
            .saturating_add(drawing.glyph_hash_leaves_reused),
        paragraph_cache_hits: paragraph.hits as u64,
        paragraph_cache_misses: paragraph.misses as u64,
        paragraph_reshapes: paragraph.reshapes as u64,
        paragraph_evictions: paragraph.evictions as u64,
        paragraph_cache_resident_bytes: runtime.paragraph_cache_resident_bytes() as u64,
        extension_calls: extension.calls,
        extension_request_bytes: extension.request_bytes,
        extension_response_bytes: extension.response_bytes,
        ui_uploads: upload.uploads,
        ui_full_uploads: upload.full_uploads,
        ui_partial_uploads: upload.partial_uploads,
        ui_uploaded_bytes: upload.uploaded_bytes,
        retained_geometry_uploaded_bytes: upload.retained_geometry_uploaded_bytes,
        retained_parameter_uploaded_bytes: upload.retained_parameter_uploaded_bytes,
        retained_cache_hits: upload.retained_cache_hits,
        retained_draw_calls: upload.retained_draw_calls,
        retained_instances: upload.retained_instances,

        ui_upload_hits: upload.hits,
        ui_resident_bytes: upload.resident_bytes,
        virtual_list_materialized_instances: virtual_list.materialized_instance_count,
        virtual_list_recycled_instances: virtual_list.recycled_instance_count,
        virtual_list_disposed_instances: virtual_list.disposed_instance_count,
        theme_change_events: 0,
        dpi_change_events: 0,
    }
}

fn summarize_aui_work(
    values: &[AuiFrameWorkCounters],
    request: &NativePlayerWindowRunRequest,
) -> engine_runtime::windowed_player::WindowedPlayerAuiWorkSummary {
    use engine_runtime::windowed_player::WindowedPlayerAuiWorkSummary;
    let start = request.performance_warmup_frames as usize;
    let sample = values
        .iter()
        .skip(start)
        .take(request.performance_sample_frames as usize)
        .cloned()
        .collect::<Vec<_>>();
    let Some(last) = sample.last().cloned() else {
        return WindowedPlayerAuiWorkSummary::default();
    };
    let baseline = start
        .checked_sub(1)
        .and_then(|slot| values.get(slot))
        .cloned()
        .unwrap_or_default();
    let delta = |end: u64, begin: u64| end.saturating_sub(begin);
    let sum = |read: fn(&AuiFrameWorkCounters) -> u64| {
        sample.iter().map(read).fold(0_u64, u64::saturating_add)
    };
    let high_water =
        |read: fn(&AuiFrameWorkCounters) -> u64| sample.iter().map(read).max().unwrap_or_default();
    let drawing_nodes_built = delta(last.drawing_nodes_built, baseline.drawing_nodes_built);
    let drawing_payloads_built =
        delta(last.drawing_payloads_built, baseline.drawing_payloads_built);
    let paragraph_cache_misses =
        delta(last.paragraph_cache_misses, baseline.paragraph_cache_misses);
    WindowedPlayerAuiWorkSummary {
        graphics_tessellation_builds: delta(
            last.graphics_tessellation_builds,
            baseline.graphics_tessellation_builds,
        ),
        graphics_cache_hits: delta(last.graphics_cache_hits, baseline.graphics_cache_hits),
        graphics_cache_high_water_bytes: high_water(|value| value.graphics_cache_resident_bytes),
        observed_sample_frames: sample.len() as u64,
        present_rebuilds: delta(last.present_rebuilds, baseline.present_rebuilds),
        present_hits: delta(last.present_hits, baseline.present_hits),
        layout_nodes_visited: delta(last.layout_nodes_visited, baseline.layout_nodes_visited),
        layout_measure_calls: delta(last.layout_measure_calls, baseline.layout_measure_calls),
        layout_geometry_nodes: delta(last.layout_geometry_nodes, baseline.layout_geometry_nodes),
        geometry_nodes_updated: delta(last.geometry_nodes_updated, baseline.geometry_nodes_updated),
        geometry_source_nodes_compared: delta(
            last.geometry_source_nodes_compared,
            baseline.geometry_source_nodes_compared,
        ),
        geometry_snapshot_nodes_copied: delta(
            last.geometry_snapshot_nodes_copied,
            baseline.geometry_snapshot_nodes_copied,
        ),
        drawing_nodes_compared: delta(last.drawing_nodes_compared, baseline.drawing_nodes_compared),
        drawing_nodes_built,
        drawing_nodes_reused: delta(last.drawing_nodes_reused, baseline.drawing_nodes_reused),
        drawing_payloads_built,
        drawing_payloads_reused: delta(
            last.drawing_payloads_reused,
            baseline.drawing_payloads_reused,
        ),
        paragraph_cache_hits: delta(last.paragraph_cache_hits, baseline.paragraph_cache_hits),
        paragraph_cache_misses,
        paragraph_reshapes: delta(last.paragraph_reshapes, baseline.paragraph_reshapes),
        paragraph_evictions: delta(last.paragraph_evictions, baseline.paragraph_evictions),
        tracked_cache_allocation_events: drawing_nodes_built
            .saturating_add(drawing_payloads_built)
            .saturating_add(paragraph_cache_misses),
        paragraph_cache_high_water_bytes: high_water(|value| value.paragraph_cache_resident_bytes),
        extension_calls: delta(last.extension_calls, baseline.extension_calls),
        extension_request_bytes: delta(
            last.extension_request_bytes,
            baseline.extension_request_bytes,
        ),
        extension_response_bytes: delta(
            last.extension_response_bytes,
            baseline.extension_response_bytes,
        ),
        ui_uploads: delta(last.ui_uploads, baseline.ui_uploads),
        ui_full_uploads: delta(last.ui_full_uploads, baseline.ui_full_uploads),
        ui_partial_uploads: delta(last.ui_partial_uploads, baseline.ui_partial_uploads),
        ui_uploaded_bytes: delta(last.ui_uploaded_bytes, baseline.ui_uploaded_bytes),
        retained_geometry_uploaded_bytes: delta(
            last.retained_geometry_uploaded_bytes,
            baseline.retained_geometry_uploaded_bytes,
        ),
        retained_parameter_uploaded_bytes: delta(
            last.retained_parameter_uploaded_bytes,
            baseline.retained_parameter_uploaded_bytes,
        ),
        retained_cache_hits: delta(last.retained_cache_hits, baseline.retained_cache_hits),
        retained_draw_calls: delta(last.retained_draw_calls, baseline.retained_draw_calls),
        retained_instances: delta(last.retained_instances, baseline.retained_instances),

        ui_upload_hits: delta(last.ui_upload_hits, baseline.ui_upload_hits),
        ui_resident_high_water_bytes: high_water(|value| value.ui_resident_bytes),
        virtual_list_materialized_instances: delta(
            last.virtual_list_materialized_instances,
            baseline.virtual_list_materialized_instances,
        ),
        virtual_list_recycled_instances: delta(
            last.virtual_list_recycled_instances,
            baseline.virtual_list_recycled_instances,
        ),
        virtual_list_disposed_instances: delta(
            last.virtual_list_disposed_instances,
            baseline.virtual_list_disposed_instances,
        ),
        theme_change_events: sum(|value| value.theme_change_events),
        dpi_change_events: sum(|value| value.dpi_change_events),
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
            p50_ms: percentile_ms(0.50),
            p95_ms: percentile_ms(0.95),
            p99_ms: percentile_ms(0.99),
            max_ms: *sample.last().unwrap() as f64 / 1_000_000.0,
        },
    )
}

struct FramePerformanceSamples<'a> {
    cpu_ns: &'a [u64],
    aui_cpu_ns: &'a [u64],
    aui_work: &'a [AuiFrameWorkCounters],
    update_ns: &'a [u64],
    render_submit_ns: &'a [u64],
    present_wait_ns: &'a [u64],
}

fn finalize_frame_performance(
    report: &mut NativeWindowHostReport,
    request: &NativePlayerWindowRunRequest,
    samples: FramePerformanceSamples<'_>,
) {
    if !request.cpu_sampling_enabled() {
        return;
    }
    let mut sample = samples
        .cpu_ns
        .iter()
        .skip(request.performance_warmup_frames as usize)
        .take(request.performance_sample_frames as usize)
        .copied()
        .collect::<Vec<_>>();
    sample.sort_unstable();
    if sample.is_empty() {
        return;
    }
    let Some(aui_cpu) = summarize_frame_phase(samples.aui_cpu_ns, request) else {
        return;
    };
    let Some(update) = summarize_frame_phase(samples.update_ns, request) else {
        return;
    };
    let Some(render_submit) = summarize_frame_phase(samples.render_submit_ns, request) else {
        return;
    };
    let Some(present_wait) = summarize_frame_phase(samples.present_wait_ns, request) else {
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
        sampling_window: None,
        cpu_excluding_present_wait: summarize_frame_phase(
            &samples
                .cpu_ns
                .iter()
                .zip(samples.present_wait_ns)
                .map(|(cpu, wait)| cpu.saturating_sub(*wait))
                .collect::<Vec<_>>(),
            request,
        ),
        raw_cpu_frames: samples
            .aui_work
            .iter()
            .enumerate()
            .filter(|(index, work)| {
                request
                    .measurement_policy()
                    .trace_frame(if work.frame_index == 0 {
                        *index as u64 + 1
                    } else {
                        work.frame_index
                    })
            })
            .filter_map(|(index, work)| {
                let previous = index
                    .checked_sub(1)
                    .and_then(|i| samples.aui_work.get(i))
                    .cloned()
                    .unwrap_or_default();
                let delta = |now: u64, old: u64| now.saturating_sub(old);
                let stages = work.timings.since(previous.timings);
                let feedback_cpu_ns = delta(work.feedback_cpu_ns, previous.feedback_cpu_ns);
                let aui_cpu_ns = *samples.aui_cpu_ns.get(index)?;
                let accounted = stages
                    .total_ns()
                    .saturating_add(feedback_cpu_ns)
                    .saturating_add(work.input_dispatch_ime_ns);
                Some(
                    engine_runtime::windowed_player::WindowedPlayerCpuFrameSample {
                        spine: work.spine,
                        cell_receipts: work.cell_receipts.clone(),
                        spatial_gpu: work.spatial_gpu,
                        sampling_wall_ns: work.sampling_wall_ns,
                        frame_identity: work.frame_identity.clone(),
                        process_memory: work.process_memory,
                        resource_memory: work.resource_memory.clone(),
                        streaming_resources: work.streaming_resources,
                        frame_index: if work.frame_index == 0 {
                            index as u64 + 1
                        } else {
                            work.frame_index
                        },
                        warmup: (index as u64) < request.performance_warmup_frames,
                        sampled: (index as u64) >= request.performance_warmup_frames
                            && (index as u64)
                                < request.performance_warmup_frames
                                    + request.performance_sample_frames,
                        cpu_frame_ns: *samples.cpu_ns.get(index)?,
                        aui_cpu_ns,
                        update_ns: *samples.update_ns.get(index)?,
                        render_submit_ns: *samples.render_submit_ns.get(index)?,
                        present_wait_ns: *samples.present_wait_ns.get(index)?,
                        aui_stages: stages,
                        preparation_layout_geometry: work.work.since(previous.work),
                        feedback_cpu_ns,
                        input_dispatch_ime_ns: work.input_dispatch_ime_ns,
                        input_ime_schedule_other_ns: aui_cpu_ns.saturating_sub(accounted),
                        aui_intervals_consistent: accounted <= aui_cpu_ns,
                        drawing: work.drawing.since(previous.drawing),
                        feedback_drawing: work.feedback_drawing.since(previous.feedback_drawing),
                        feedback_source_nodes_visited: delta(
                            work.feedback_source_nodes_visited,
                            previous.feedback_source_nodes_visited,
                        ),
                        projection: work
                            .projection
                            .map(|now| now.since(previous.projection.unwrap_or_default())),
                        cpu_draw_buffer_prepare_ns: delta(
                            work.cpu_draw_buffer_prepare_ns,
                            previous.cpu_draw_buffer_prepare_ns,
                        ),
                        ui_uploaded_bytes: delta(
                            work.ui_uploaded_bytes,
                            previous.ui_uploaded_bytes,
                        ),
                        retained_geometry_uploaded_bytes: delta(
                            work.retained_geometry_uploaded_bytes,
                            previous.retained_geometry_uploaded_bytes,
                        ),
                        retained_parameter_uploaded_bytes: delta(
                            work.retained_parameter_uploaded_bytes,
                            previous.retained_parameter_uploaded_bytes,
                        ),
                        retained_cache_hits: delta(
                            work.retained_cache_hits,
                            previous.retained_cache_hits,
                        ),
                        retained_draw_calls: delta(
                            work.retained_draw_calls,
                            previous.retained_draw_calls,
                        ),
                        retained_instances: delta(
                            work.retained_instances,
                            previous.retained_instances,
                        ),
                        ui_full_uploads: delta(work.ui_full_uploads, previous.ui_full_uploads),
                        ui_partial_uploads: delta(
                            work.ui_partial_uploads,
                            previous.ui_partial_uploads,
                        ),
                        ui_full_upload_cold_or_evicted: delta(
                            work.ui_full_upload_cold_or_evicted,
                            previous.ui_full_upload_cold_or_evicted,
                        ),
                        ui_full_upload_untrusted_lineage: delta(
                            work.ui_full_upload_untrusted_lineage,
                            previous.ui_full_upload_untrusted_lineage,
                        ),
                    },
                )
            })
            .collect(),
        gpu: None,
        warmup_frames: request.performance_warmup_frames,
        requested_sample_frames: request.performance_sample_frames,
        observed_sample_frames: sample.len() as u64,
        mean_cpu_frame_ms: mean_ns / 1_000_000.0,
        p50_cpu_frame_ms: percentile_ms(0.50),
        p95_cpu_frame_ms: percentile_ms(0.95),
        p99_cpu_frame_ms: percentile_ms(0.99),
        max_cpu_frame_ms: *sample.last().unwrap() as f64 / 1_000_000.0,
        aui_cpu,
        update,
        render_submit,
        present_wait,
        aui_work: summarize_aui_work(samples.aui_work, request),
    });
    if let Some(summary) = &mut report.frame_performance_summary {
        request.measurement_policy().filter_cpu(summary);
    }
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
    run_headless_native_player(request, linked_modules, None)
}

/// Cooperative in-process execution. Use runtime_cli's bounded process owner for a hard deadline.
pub fn run_windowed_semantic_playtest_with_linked_modules(
    mut request: NativePlayerWindowRunRequest,
    linked_modules: std::sync::Arc<LinkedProjectRuntimeSet>,
    scenario: semantic_outcome::PlaytestScenario,
    capture_directory: PathBuf,
) -> NativeWindowHostReport {
    if scenario.target != semantic_outcome::PlaytestTarget::WindowsWindowed
        || request.input_script.is_some()
        || request.screenshot.enabled
        || request.mode != NativePlayerWindowRunMode::Windowed
    {
        let mut report = NativeWindowHostReport::base(&request);
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "playtest.request_conflict",
            "playtest",
            "Windowed scenario owns input and capture requests.",
        ));
        return report;
    }
    request.frame_limit = scenario
        .max_presentation_frames
        .min(semantic_outcome::MAX_PLAYTEST_FRAMES);
    request.input_script = Some(semantic_playtest::SemanticPlaytest::input_script(&scenario));
    #[cfg(feature = "real-window")]
    {
        real_window::run_windowed_controlled(
            request,
            linked_modules,
            Some((scenario, capture_directory)),
        )
    }
    #[cfg(not(feature = "real-window"))]
    {
        let _ = (linked_modules, capture_directory);
        let mut report = NativeWindowHostReport::base(&request);
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "playtest.target_unsupported",
            "playtest",
            "Player was built without real-window.",
        ));
        report
    }
}

/// Cooperative in-process execution. Use runtime_cli's bounded process owner for a hard deadline.
pub fn run_headless_semantic_playtest_with_linked_modules(
    mut request: NativePlayerWindowRunRequest,
    linked_modules: &LinkedProjectRuntimeSet,
    scenario: semantic_outcome::PlaytestScenario,
) -> NativeWindowHostReport {
    if scenario.target != semantic_outcome::PlaytestTarget::WindowsHeadless
        || request.input_script.is_some()
        || request.screenshot.enabled
        || request.mode != NativePlayerWindowRunMode::HeadlessSurfaceGate
    {
        let mut report = NativeWindowHostReport::base(&request);
        report.diagnostics.push(NativeWindowHostDiagnostic::error(
            "playtest.request_conflict",
            "playtest",
            "Scenario owns input/capture requests and requires a headless request.",
        ));
        return report;
    }
    request.frame_limit = scenario
        .max_presentation_frames
        .min(semantic_outcome::MAX_PLAYTEST_FRAMES);
    request.input_script = Some(semantic_playtest::SemanticPlaytest::input_script(&scenario));
    run_headless_native_player(request, linked_modules, Some(scenario))
}

fn run_headless_native_player(
    request: NativePlayerWindowRunRequest,
    linked_modules: &LinkedProjectRuntimeSet,
    scenario: Option<semantic_outcome::PlaytestScenario>,
) -> NativeWindowHostReport {
    let playtest_started = Instant::now();
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

    if let Some(scenario) = scenario.as_ref() {
        if let Err(error) = semantic_playtest::SemanticPlaytest::validate(scenario, &package) {
            report.diagnostics.push(error);
            return report;
        }
    }
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

    configure_runtime_audio(&mut host, &request);
    let mut playtest = match scenario
        .map(|scenario| {
            semantic_playtest::SemanticPlaytest::new(
                scenario,
                &package,
                &report
                    .project_runtime_bind_receipt
                    .as_ref()
                    .unwrap()
                    .session_id,
                playtest_started,
            )
        })
        .transpose()
    {
        Ok(playtest) => playtest,
        Err(error) => {
            report.diagnostics.push(error);
            return report;
        }
    };

    let Some((mut world, mut hydrator)) = hydrate_active_scene_for_player(&package, &mut report)
    else {
        report.scene_status = "error".to_string();
        report.world_status = "error".to_string();
        report.present_status = NativeWindowPresentStatus::SceneFailed;
        return report;
    };
    report.scene_status = "ok".to_string();
    report.world_status = "ok".to_string();
    if !project_storage::bind(
        &request,
        &package,
        linked_modules,
        &mut hydrator,
        &mut report,
    ) {
        return report;
    }
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
    let mut frame_cpu_ns = cpu_frame_history(&request);
    let mut frame_aui_cpu_ns = cpu_frame_history(&request);
    let mut frame_aui_work = cpu_frame_history(&request);
    let mut frame_update_ns = cpu_frame_history(&request);
    let mut frame_render_submit_ns = cpu_frame_history(&request);
    let mut frame_present_wait_ns = cpu_frame_history(&request);
    for frame_index in 0..request.frame_limit {
        if playtest
            .as_mut()
            .is_some_and(|playtest| playtest.should_stop(&mut report))
        {
            break;
        }
        let frame_started = Instant::now();
        let scripted_events = if let Some(playtest) = &mut playtest {
            playtest
                .next_input_tick()
                .map_or_else(Vec::new, |tick| scripted_input_events(&request, tick))
        } else {
            scripted_input_events(&request, frame_index + 1)
        };
        let measuring = request.measurement_enabled();
        let cpu_sampling = request.cpu_sampling_enabled();
        aui_present_cache.set_measurement_enabled(measuring);
        aui_feedback_state.set_measurement_enabled(measuring);
        host.set_ui_measurement_enabled(measuring);
        host.begin_spine_measurement_frame(measuring);
        let aui_started = Instant::now();
        let mut aui_present = aui_present_cache.take_or_rebuild_stabilized(
            &package,
            &world,
            frame_index + 1,
            ui_state_producer.as_mut(),
            &mut host,
            NativeAuiPresentContext::new(
                None,
                Some(&AuiViewportInput::new(
                    request.game_view_target.extent.width,
                    request.game_view_target.extent.height,
                    1.0,
                )),
                &mut aui_interaction_state,
            )
            .with_feedback_cache(&mut aui_feedback_state),
        );
        let input_dispatch_started = measuring.then(Instant::now);
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
        let input_dispatch_ime_ns = input_dispatch_started.map_or(0, |start| {
            start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });
        if let Some(playtest) = &mut playtest {
            playtest.record_injected_events(scripted_events.len());
        }
        aui_present = aui_present_cache.advance_animations_for_present(
            aui_present,
            &package,
            &world,
            ui_state_producer.as_mut(),
            &mut host,
            engine_runtime::aui_present_cache::AuiAnimationPresentContext {
                frame_index: frame_index + 1,
                delta_us: presentation_delta_us_from_seconds(
                    engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME,
                ),
                interaction_result: aui_interaction.as_ref().map(|i| &i.result),
                presentation: NativeAuiPresentContext::new(
                    None,
                    Some(&AuiViewportInput::new(
                        request.game_view_target.extent.width,
                        request.game_view_target.extent.height,
                        1.0,
                    )),
                    &mut aui_interaction_state,
                )
                .with_feedback_cache(&mut aui_feedback_state),
            },
        );
        if let (Some(present), Some(interaction)) = (aui_present.as_mut(), aui_interaction.as_ref())
        {
            aui_present_cache.apply_virtual_list_actions(present, &interaction.result.actions);
            AuiRuntimePresenter::apply_control_feedback_with_fonts_and_themes(
                present,
                &interaction.result,
                &mut aui_feedback_state,
                presentation_delta_us_from_seconds(
                    engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME,
                ),
                &package.font_atlases,
                &package.font_bundles,
                &package.aui_themes,
            );
            host.publish_ui_geometry(present);
        }
        record_cpu_frame_sample(cpu_sampling, &mut frame_aui_cpu_ns, || {
            aui_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });
        report.input = input_summary;
        report.input_status = "ok".to_string();
        let mut frame_input = EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_action_snapshot(action_snapshot)
            .with_input_trace_summary(input_trace_summary)
            .with_runtime_texture_bindings(runtime_texture_bindings.clone())
            .with_deferred_render_submission();
        if playtest.is_some() {
            frame_input = frame_input.with_fixed_step_count(1);
        }
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
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        );
        if output.host_control.is_some() {
            record_runtime_audio(&mut report, &host, request.runtime_report_level);
            report.record_project_runtime_session(output.project_runtime_session_report.clone());
            report.session_exit_requested = true;
            record_runtime_trace(
                &mut report,
                &output.runtime_trace,
                request.runtime_report_level,
            );
            break;
        }
        report.record_project_runtime_session(output.project_runtime_session_report.clone());
        record_runtime_audio(&mut report, &host, request.runtime_report_level);
        if let Some(playtest) = &mut playtest {
            if !playtest.spatial_ids().is_empty() {
                playtest.set_spatial_sample(host.observe_spatial(
                    &world,
                    playtest.spatial_ids(),
                    playtest.spatial_level(),
                ));
            }
            playtest.sample(
                &output,
                &mut report,
                aui_present.as_ref(),
                Some(&aui_interaction_state),
            );
        }
        record_cpu_frame_sample(cpu_sampling, &mut frame_update_ns, || {
            frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });
        let render_started = Instant::now();
        record_runtime_trace(
            &mut report,
            &output.runtime_trace,
            request.runtime_report_level,
        );
        if output.runtime_advanced {
            report.mark_runtime_advanced();
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
        record_cpu_frame_sample(cpu_sampling, &mut frame_aui_work, || {
            if !measuring {
                return AuiFrameWorkCounters {
                    frame_index: frame_index + 1,
                    ..Default::default()
                };
            }
            capture_aui_work_with_input(
                &aui_present_cache,
                &host,
                None,
                &aui_feedback_state,
                frame_index + 1,
                input_dispatch_ime_ns,
            )
        });
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
        record_cpu_frame_sample(cpu_sampling, &mut frame_render_submit_ns, || {
            render_started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64
        });
        record_cpu_frame_sample(cpu_sampling, &mut frame_present_wait_ns, || 0);
        record_cpu_frame_sample(cpu_sampling, &mut frame_cpu_ns, || {
            frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });
    }

    finalize_frame_performance(
        &mut report,
        &request,
        FramePerformanceSamples {
            cpu_ns: &frame_cpu_ns,
            aui_cpu_ns: &frame_aui_cpu_ns,
            aui_work: &frame_aui_work,
            update_ns: &frame_update_ns,
            render_submit_ns: &frame_render_submit_ns,
            present_wait_ns: &frame_present_wait_ns,
        },
    );

    report.render_thread_report_schema = last_render_schema;
    report.rhi_command_count = last_rhi_command_count;
    report.surface =
        NativeSurfaceState::headless_presented(&request.config, report.frames_completed);
    if report.render_status == "ok" && report.rhi_status == "ok" {
        report.present_status = NativeWindowPresentStatus::Presented;
    }
    report.exit_code = report.completion_exit_code();
    if let Some(mut playtest) = playtest {
        playtest.should_stop(&mut report);
        playtest.finish(&mut report);
    }
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
                .assets
                .assets
                .iter()
                .filter(|asset| asset.asset_type == "prefab")
                .filter_map(|asset| asset.data.as_ref())
                .filter_map(|data| {
                    serde_json::from_value::<engine_runtime::runtime_package::RuntimePrefabData>(
                        data.get("prefab").unwrap_or(data).clone(),
                    )
                    .ok()
                })
                .flat_map(|prefab| prefab.entities)
                .filter_map(|entity| entity.sprite_renderer2d)
                .filter_map(|renderer| renderer.sprite_ref)
                .map(|asset| asset.id),
        )
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
        .map(|image| image.asset_id.clone())
        .chain(package.aui_themes.texture_asset_ids());
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
        let render_report = output.base_draw_report();
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

#[cfg(any(feature = "real-window", test))]
fn native_window_viewport_input(
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
) -> AuiViewportInput {
    AuiViewportInput::new(physical_width, physical_height, scale_factor)
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
mod gpu_memory;
#[cfg(feature = "real-window")]
mod native_present;

#[cfg(feature = "real-window")]
mod real_window;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/session_exit.rs"]
mod session_exit_tests;

#[cfg(test)]
#[path = "tests/foundation_cpu_probe.rs"]
mod foundation_cpu_probe_tests;
