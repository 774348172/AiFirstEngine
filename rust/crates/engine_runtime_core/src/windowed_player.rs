use crate::default_game_run::{DefaultGameRunMode, DefaultGameRunRequest};
use crate::diagnostics::{DiagnosticSeverity, RuntimeDiagnostic};
use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use crate::render_asset_production::{
    RuntimeRenderAssetKind, RuntimeRenderAssetProducer, RuntimeRenderAssetRequest,
    RuntimeRenderAssetStatus, RuntimeRenderAssetUsage,
};
use crate::render_resource::RenderResourceManager;
use crate::rhi_command_plan::RhiCommand;
use crate::runtime_asset::RuntimeAssetHandle;
use crate::runtime_asset::RuntimeAssetLoadState;
use crate::runtime_asset_diagnostics::AssetLoadState;
use crate::runtime_asset_loader::RuntimeAssetLoader;
use crate::runtime_package::{load_runtime_package, RuntimePackage};
use crate::runtime_scene_hydration::hydrate_active_scene_into_world;
use crate::runtime_trace::RuntimeTrace;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const WINDOWED_PLAYER_RUN_REPORT_SCHEMA_VERSION: &str = "windowed-player-run-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowedPlayerMode {
    HeadlessGate,
    Windowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerRunRequest {
    pub project_path: PathBuf,
    pub runtime_package_path: PathBuf,
    pub mode: WindowedPlayerMode,
    pub frame_limit: u64,
    pub scenario_id: String,
}

impl WindowedPlayerRunRequest {
    pub fn headless_gate(runtime_package_path: impl Into<PathBuf>) -> Self {
        let runtime_package_path = runtime_package_path.into();
        Self {
            project_path: runtime_package_path.clone(),
            runtime_package_path,
            mode: WindowedPlayerMode::HeadlessGate,
            frame_limit: 3,
            scenario_id: "windowed_player_runtime_v1_gate".to_string(),
        }
    }

    pub fn windowed(runtime_package_path: impl Into<PathBuf>) -> Self {
        let runtime_package_path = runtime_package_path.into();
        Self {
            project_path: runtime_package_path.clone(),
            runtime_package_path,
            mode: WindowedPlayerMode::Windowed,
            frame_limit: 3,
            scenario_id: "windowed_player_runtime_v1".to_string(),
        }
    }

    pub fn default_game_run_request(&self) -> DefaultGameRunRequest {
        DefaultGameRunRequest {
            project_path: self.project_path.clone(),
            runtime_package_path: self.runtime_package_path.clone(),
            mode: match self.mode {
                WindowedPlayerMode::HeadlessGate => DefaultGameRunMode::Headless,
                WindowedPlayerMode::Windowed => DefaultGameRunMode::Windowed,
            },
            scenario_id: self.scenario_id.clone(),
            frame_limit: self.frame_limit,
            report_path: None,
            launch_runtime_process: matches!(self.mode, WindowedPlayerMode::Windowed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowedPlayerDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerDiagnostic {
    pub severity: WindowedPlayerDiagnosticSeverity,
    pub code: String,
    pub layer: String,
    pub message: String,
    pub path: Option<String>,
}

impl WindowedPlayerDiagnostic {
    pub fn error(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: WindowedPlayerDiagnosticSeverity::Error,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerLayerStatus {
    pub request: String,
    pub package: String,
    pub asset: String,
    pub scene: String,
    pub world: String,
    pub logic: String,
    pub input: String,
    pub physics: String,
    pub projection: String,
    pub render: String,
    pub rdg: String,
    pub rhi: String,
    pub surface: String,
    pub present: String,
}

impl Default for WindowedPlayerLayerStatus {
    fn default() -> Self {
        Self {
            request: "not_started".to_string(),
            package: "not_started".to_string(),
            asset: "not_started".to_string(),
            scene: "not_started".to_string(),
            world: "not_started".to_string(),
            logic: "not_started".to_string(),
            input: "not_started".to_string(),
            physics: "not_started".to_string(),
            projection: "not_started".to_string(),
            render: "not_started".to_string(),
            rdg: "not_started".to_string(),
            rhi: "not_started".to_string(),
            surface: "not_started".to_string(),
            present: "not_started".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerCounters {
    pub frames_requested: u64,
    pub frames_completed: u64,
    pub entity_count: usize,
    pub archetype_count: usize,
    pub hydrated_entity_count: usize,
    pub hydration_loaded_asset_count: usize,
    pub hydration_dirty_record_count: usize,
    pub render_proxy_count: usize,
    pub runtime_trace_event_count: usize,
}

impl WindowedPlayerCounters {
    fn new(frames_requested: u64) -> Self {
        Self {
            frames_requested,
            frames_completed: 0,
            entity_count: 0,
            archetype_count: 0,
            hydrated_entity_count: 0,
            hydration_loaded_asset_count: 0,
            hydration_dirty_record_count: 0,
            render_proxy_count: 0,
            runtime_trace_event_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerPackageSummary {
    pub project_name: String,
    pub project_version: String,
    pub active_scene_id: String,
    pub scene_count: usize,
    pub declared_asset_count: usize,
    pub runtime_asset_record_count: usize,
    pub cooked_asset_count: usize,
    pub rule_mode: String,
    pub content_hash: Option<String>,
}

impl WindowedPlayerPackageSummary {
    fn from_package(package: &RuntimePackage) -> Self {
        Self {
            project_name: package.manifest.project.name.clone(),
            project_version: package.manifest.project.version.clone(),
            active_scene_id: package.manifest.active_scene_id.clone(),
            scene_count: package.manifest.scenes.len(),
            declared_asset_count: package.assets.assets.len(),
            runtime_asset_record_count: package.assets.runtime_asset_index.len(),
            cooked_asset_count: package.assets.cooked_asset_table.len(),
            rule_mode: package.rules.mode.clone(),
            content_hash: package.manifest.content_hash.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerAssetLoadSummary {
    pub requested_asset_count: usize,
    pub ready_handle_count: usize,
    pub decoded_cache_count: usize,
    pub diagnostic_count: usize,
}

impl WindowedPlayerAssetLoadSummary {
    fn empty() -> Self {
        Self {
            requested_asset_count: 0,
            ready_handle_count: 0,
            decoded_cache_count: 0,
            diagnostic_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerGpuBindingSummary {
    pub requested_handle_count: usize,
    pub prepare_request_count: usize,
    pub prepared_asset_count: usize,
    pub render_resource_request_count: usize,
    pub resident_resource_count: usize,
    pub diagnostic_count: usize,
}

impl WindowedPlayerGpuBindingSummary {
    fn empty() -> Self {
        Self {
            requested_handle_count: 0,
            prepare_request_count: 0,
            prepared_asset_count: 0,
            render_resource_request_count: 0,
            resident_resource_count: 0,
            diagnostic_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerRendererSummary {
    pub draw_item_count: usize,
    pub rhi_command_count: usize,
    pub rhi_draw_command_count: usize,
    pub graph_pass_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerScreenshotSummary {
    pub requested: bool,
    pub status: String,
    pub path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub byte_size: Option<u64>,
}

impl WindowedPlayerScreenshotSummary {
    pub fn not_requested() -> Self {
        Self {
            requested: false,
            status: "not_requested".to_string(),
            path: None,
            width: 0,
            height: 0,
            byte_size: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowedPlayerRuntimeReportLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerFramePhasePerformanceSummary {
    pub observed_sample_frames: u64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerFramePerformanceSummary {
    /// Real monotonic interval from first sampled frame start to last frame end.
    /// Missing on legacy/headless consumers; never inferred from CPU totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_window: Option<WindowedPlayerSamplingWindow>,
    /// Per-frame app CPU interval minus measured acquire/Present waits. It keeps
    /// dependency waits, excludes external frame pacing, and is not OS display time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_excluding_present_wait: Option<WindowedPlayerFramePhasePerformanceSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_cpu_frames: Vec<WindowedPlayerCpuFrameSample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<WindowedPlayerGpuPerformance>,
    pub warmup_frames: u64,
    pub requested_sample_frames: u64,
    pub observed_sample_frames: u64,
    pub mean_cpu_frame_ms: f64,
    pub p50_cpu_frame_ms: f64,
    pub p95_cpu_frame_ms: f64,
    pub p99_cpu_frame_ms: f64,
    pub max_cpu_frame_ms: f64,
    pub aui_cpu: WindowedPlayerFramePhasePerformanceSummary,
    pub update: WindowedPlayerFramePhasePerformanceSummary,
    pub render_submit: WindowedPlayerFramePhasePerformanceSummary,
    pub present_wait: WindowedPlayerFramePhasePerformanceSummary,
    pub aui_work: WindowedPlayerAuiWorkSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerSamplingWindow {
    pub wall_ns: u64,
    pub first_frame: u64,
    pub last_frame: u64,
    pub first_tick: Option<u64>,
    pub last_tick: Option<u64>,
    pub complete: bool,
    pub streaming_resources_start: Option<WindowedPlayerStreamingResourceSample>,
    pub streaming_resources_end: Option<WindowedPlayerStreamingResourceSample>,
    pub streaming_resources_unchanged: Option<bool>,
    pub peak_private_commit_bytes: Option<u64>,
    pub peak_working_set_bytes: Option<u64>,
    /// Private commit peak over all requested frames, including warmup.
    /// The existing peak_private_commit_bytes remains sampled-window only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_peak_private_commit_bytes: Option<u64>,
    /// All requested frames, including warmup. Missing observations stay unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_system_available_bytes: Option<u64>,
    /// Least-squares private-commit slope over the final 1200 consecutive frames.
    /// Computed after sampling, including for the explicit Off CPU probe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail1200_private_bytes_per_frame: Option<f64>,
    /// Sampled-window private commit [P95, P99], retaining the original budgets
    /// without exporting per-frame diagnostic histories from Off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_commit_percentile_bytes: Option<[u64; 2]>,
}

/// Ordered per-frame observations. Each total keeps the pre-363 timing boundary.
/// AUI subintervals exclude renderer projection and GPU submission, which occur later.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerCpuFrameSample {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spine: Option<crate::spine_animation::measurement::SpineCpuFrameSample>,
    /// Trace-only, bounded receipts from the existing cell owner. State is
    /// observed at sampling_wall_ns; transition latency has frame resolution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cell_receipts: Vec<WindowedPlayerCellReceipt>,
    /// Actual backend draw totals and this frame's segmented upload charge.
    /// Draws/instances are cumulative, not visibility declarations or OS presents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_gpu: Option<WindowedPlayerSpatialGpuSample>,
    /// Monotonic offset from first sampled frame start; never simulation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_wall_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_identity: Option<std::sync::Arc<crate::render_thread::RenderFrameIdentity>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_memory: Option<WindowedPlayerProcessMemorySample>,
    /// Allocation inventory is exported only on explicitly filtered Trace frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_memory: Option<crate::resource_memory::ResourceMemorySample>,
    /// Optional owner-tracked streaming domains, not process or driver totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streaming_resources: Option<WindowedPlayerStreamingResourceSample>,
    pub frame_index: u64,
    pub warmup: bool,
    pub sampled: bool,
    pub cpu_frame_ns: u64,
    pub aui_cpu_ns: u64,
    pub update_ns: u64,
    pub render_submit_ns: u64,
    pub present_wait_ns: u64,
    pub aui_stages: crate::aui::AuiUpdateTimings,
    pub preparation_layout_geometry: crate::aui::AuiUpdateWork,
    pub feedback_cpu_ns: u64,
    /// Pointer mapping, input dispatch/hit testing, and platform IME when applicable.
    pub input_dispatch_ime_ns: u64,
    pub input_ime_schedule_other_ns: u64,
    pub aui_intervals_consistent: bool,
    pub drawing: crate::aui::AuiDrawingStats,
    pub feedback_drawing: crate::aui::AuiDrawingStats,
    pub feedback_source_nodes_visited: u64,
    /// None means the render-dispatch composition has no supported local telemetry.
    pub projection: Option<crate::runtime_renderer::UiProjectionStats>,
    /// All UI and world buffer preparation; a subinterval of render_submit_ns.
    pub cpu_draw_buffer_prepare_ns: u64,
    pub ui_uploaded_bytes: u64,
    #[serde(default)]
    pub retained_geometry_uploaded_bytes: u64,
    #[serde(default)]
    pub retained_parameter_uploaded_bytes: u64,
    #[serde(default)]
    pub retained_cache_hits: u64,
    #[serde(default)]
    pub retained_draw_calls: u64,
    #[serde(default)]
    pub retained_instances: u64,

    pub ui_partial_uploads: u64,
    pub ui_full_uploads: u64,
    pub ui_full_upload_cold_or_evicted: u64,
    pub ui_full_upload_untrusted_lineage: u64,
}

pub use project_runtime_sdk::ProjectRuntimeCellReceipt as WindowedPlayerCellReceipt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerSpatialGpuSample {
    pub cumulative_draws: u64,
    pub cumulative_instances: u64,
    pub frame_uploaded_bytes: u64,
    /// Native geometry instance/view/index/clear writes, not UI counters.
    #[serde(default)]
    pub cumulative_parameter_uploaded_bytes: u64,
    /// Conservative bound for this feature: the backend's existing admission
    /// census includes all owned requested buffers/textures and retirement.
    /// Excludes measurement queries, external surfaces and driver-private heaps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_gpu_backing_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerStreamingResourceSample {
    pub active_cells: usize,
    pub live_cells: usize,
    pub cpu_resident_bytes: u64,
    pub cpu_reserved_bytes: u64,
    pub pending_asset_operations: usize,
    pub pending_asset_tickets: usize,
    pub gpu_mesh_bytes: u64,
    /// Physical mesh pages include alignment, spare ranges and pending retirement.
    #[serde(default)]
    pub gpu_mesh_physical_bytes: u64,
    pub gpu_texture_bytes: u64,
    pub gpu_staging_bytes: u64,
    pub gpu_retiring_bytes: u64,
    pub gpu_uploaded_bytes: u64,
    pub pending_gpu_tickets: usize,
    pub gpu_meshes: usize,
    pub gpu_textures: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerProcessMemorySample {
    pub working_set_bytes: u64,
    pub private_commit_bytes: u64,
    /// Same opt-in capture, using the existing platform memory owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_available_bytes: Option<u64>,
    pub sampling_cpu_ns: u64,
}
/// Opt-in platform observation; absence is unsupported/failure, never a zero reading.
pub fn sample_player_process_memory() -> Option<WindowedPlayerProcessMemorySample> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::{
            ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Threading::GetCurrentProcess,
        };
        let start = crate::platform_time::Instant::now();
        // SAFETY: the zeroed counters contain only integer fields. The pointer is
        // writable for the EX structure's complete byte size for this synchronous API.
        let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of_val(&counters) as u32;
        // SAFETY: the current-process pseudo handle is valid and the native API
        // accepts the EX layout through its base-structure pointer and explicit size.
        if unsafe {
            K32GetProcessMemoryInfo(
                GetCurrentProcess(),
                (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                counters.cb,
            )
        } == 0
        {
            return None;
        }
        Some(WindowedPlayerProcessMemorySample {
            working_set_bytes: counters.WorkingSetSize as u64,
            private_commit_bytes: counters.PrivateUsage as u64,
            system_available_bytes: crate::resource_memory::system_available_bytes(),
            sampling_cpu_ns: start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
        })
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerAuiWorkSummary {
    #[serde(default)]
    pub graphics_tessellation_builds: u64,
    #[serde(default)]
    pub graphics_cache_hits: u64,
    #[serde(default)]
    pub graphics_cache_high_water_bytes: u64,
    pub observed_sample_frames: u64,
    pub present_rebuilds: u64,
    pub present_hits: u64,
    pub layout_nodes_visited: u64,
    pub layout_measure_calls: u64,
    pub layout_geometry_nodes: u64,
    pub geometry_nodes_updated: u64,
    pub geometry_source_nodes_compared: u64,
    pub geometry_snapshot_nodes_copied: u64,
    pub drawing_nodes_compared: u64,
    pub drawing_nodes_built: u64,
    pub drawing_nodes_reused: u64,
    pub drawing_payloads_built: u64,
    pub drawing_payloads_reused: u64,
    pub paragraph_cache_hits: u64,
    pub paragraph_cache_misses: u64,
    pub paragraph_reshapes: u64,
    pub paragraph_evictions: u64,
    pub tracked_cache_allocation_events: u64,
    pub paragraph_cache_high_water_bytes: u64,
    pub extension_calls: u64,
    pub extension_request_bytes: u64,
    pub extension_response_bytes: u64,
    pub ui_uploads: u64,
    pub ui_full_uploads: u64,
    pub ui_partial_uploads: u64,
    pub ui_uploaded_bytes: u64,
    #[serde(default)]
    pub retained_geometry_uploaded_bytes: u64,
    #[serde(default)]
    pub retained_parameter_uploaded_bytes: u64,
    #[serde(default)]
    pub retained_cache_hits: u64,
    #[serde(default)]
    pub retained_draw_calls: u64,
    #[serde(default)]
    pub retained_instances: u64,

    pub ui_upload_hits: u64,
    pub ui_resident_high_water_bytes: u64,
    pub virtual_list_materialized_instances: u64,
    pub virtual_list_recycled_instances: u64,
    pub virtual_list_disposed_instances: u64,
    pub theme_change_events: u64,
    pub dpi_change_events: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerGameplayTraceSummary {
    pub report_level: WindowedPlayerRuntimeReportLevel,
    pub input_script_id: Option<String>,
    pub record_count: usize,
    pub write_count: usize,
    pub command_enqueue_count: usize,
    pub command_apply_count: usize,
    pub prefab_instantiate_apply_count: usize,
    pub failed_record_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_details: Vec<WindowedPlayerRuntimeFailure>,
    #[serde(default)]
    pub omitted_failure_count: usize,
}

/// Explicit performance capture only; counters describe the last simulation substep
/// of each sampled frame, not capacity or a CPU particle mirror.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerGpuPerformance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_present: Option<NativePresentReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submissions: Vec<crate::render_thread::GpuSubmissionObservation>,
    pub status: String,
    pub adapter: String,
    pub driver: String,
    pub backend: String,
    pub timestamp_period_ns: f32,
    pub measurement_buffer_bytes: u64,
    #[serde(default)]
    pub measurement_used_bytes: u64,
    #[serde(default)]
    pub submission_observations_omitted: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<WindowedPlayerFramePhasePerformanceSummary>,
    pub samples: Vec<WindowedPlayerGpuFrameSample>,
}

/// Native presentation timings are separate from surface.present calls and GPU fences.
/// Absence of an event does not prove a discarded frame or a repeated scan-out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePresentFrameObservation {
    /// Application present-call boundaries in Windows QPC units, never display times.
    #[serde(default)]
    pub application_present_qpc: Option<[i64; 2]>,
    #[serde(default)]
    pub application_present_thread_id: Option<u32>,
    pub submission: crate::render_thread::GpuSubmissionToken,
    pub frame_identity: Option<std::sync::Arc<crate::render_thread::RenderFrameIdentity>>,
    pub application_present_index: u64,
    pub present_id: Option<u32>,
    pub swapchain_epoch: u64,
    pub image_index: u32,
    pub actual_present_time_ns: Option<u64>,
    pub earliest_present_time_ns: Option<u64>,
    pub present_margin_ns: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePresentReport {
    #[serde(default)]
    pub windows_etw: Option<WindowsEtwCaptureReport>,
    pub status: String,
    pub source: String,
    pub frames: Vec<NativePresentFrameObservation>,
    pub unobserved_frames: u64,
    pub unmatched_events: u64,
    pub duplicate_events: u64,
    /// This API provides successful presentation timing, not discard/repeat telemetry.
    pub discarded_frames: Option<u64>,
    pub repeated_displays: Option<u64>,
    pub sampling_cpu_ns: u64,
    pub errors: Vec<String>,
}

/// Raw ETW acquisition is separate from decoding and attributing OS display events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwCaptureReport {
    #[serde(default)]
    pub rundown_requested: bool,
    #[serde(default)]
    pub rundown_error: Option<u32>,
    #[serde(default)]
    pub enabled_providers: Vec<WindowsEtwProvider>,
    #[serde(default)]
    pub failed_provider: Option<WindowsEtwProvider>,
    pub status: String,
    pub process_id: u32,
    pub session_name: Option<String>,
    pub etl_path: Option<String>,
    pub scope: String,
    pub decode_status: String,
    pub qpc_frequency: Option<i64>,
    pub start_error: Option<u32>,
    pub enable_error: Option<u32>,
    pub stop_error: Option<u32>,
    pub cleanup_status: String,
    pub stop_attempts: u32,
    pub events_lost: Option<u32>,
    pub log_buffers_lost: Option<u32>,
    pub buffers_written: Option<u32>,
    pub maximum_file_bytes: u64,
    pub maximum_buffer_bytes: u64,
    pub etl_bytes: Option<u64>,
    pub lifecycle_cpu_ns: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoded: Option<WindowsEtwDecodeReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlated: Option<WindowsEtwCorrelationReport>,
}

/// Conservative event chains referencing the frozen parent report. A candidate
/// is neither a qualified display timestamp nor discard/repeat telemetry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwCorrelationReport {
    #[serde(default)]
    pub independent_flip_candidates: u64,
    #[serde(default)]
    pub composition_handoff_candidates: u64,
    #[serde(default)]
    pub explicit_discard_candidates: u64,
    pub status: String,
    pub processing_elapsed_ns: u64,
    pub frames_examined: u64,
    pub rejected_frames: u64,
    pub unmatched_frames: u64,
    pub ambiguous_frames: u64,
    pub history_ready_candidates: u64,
    pub legacy_flip_candidates: u64,
    pub frames: Vec<WindowsEtwFrameCorrelation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwFrameCorrelation {
    /// Lifetime origins for a legacy flip candidate, from the same decoded trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_device_event: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_context_event: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub independent_flip: Option<WindowsEtwIndependentFlipEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<WindowsEtwCompositionEvidence>,
    pub frame_record_index: u32,
    pub submission: crate::render_thread::GpuSubmissionToken,
    pub state: String,
    /// Indices into windowsEtw.decoded.events, never indices into a new World.
    pub root_event: u32,
    pub history_info_event: Option<u32>,
    pub queue_submit_event: Option<u32>,
    pub queue_complete_event: Option<u32>,
    pub mmio_flip_event: Option<u32>,
    pub vsync_event: Option<u32>,
}

/// Explicit surface-token provenance, not a scan-out timestamp. All indices refer
/// to the parent's decoded events and preserve its frozen frame identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwCompositionEvidence {
    pub surface_luid: u64,
    pub present_count: u64,
    pub bind_id: u64,
    pub token_event: u32,
    pub in_frame_event: Option<u32>,
    pub discard_event: Option<u32>,
    pub surface_update_event: Option<u32>,
    /// DWM 467/468/476 content-consumption events. Neither count nor timestamp
    /// establishes scan-out, and uniqueId is not assumed to be a PresentId.
    pub consumed_events: Vec<u32>,
}

/// Explicit independent-flip provenance. Event indices refer to the same decoded
/// trace. A candidate is not qualified native display timing or a repeat count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwIndependentFlipEvidence {
    pub token_event: u32,
    pub in_frame_event: u32,
    pub independent_event: u32,
    pub device_event: u32,
    pub context_event: u32,
    pub queue_submit_event: u32,
    pub queue_complete_event: u32,
    pub mmio_flip_event: u32,
    pub scanout_event: u32,
    pub multi_plane_events: Vec<u32>,
    /// Native Dxg adapter pointer identity, not a display adapter LUID.
    pub adapter: u64,
    pub vid_pn_source_id: u32,
    pub vid_pn_target_id: u32,
    pub plane_index: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsEtwProvider {
    #[default]
    DxgKrnl,
    Win32k,
    DwmCore,
}

/// Bounded raw ETL inputs. These records do not establish application-frame display.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwDecodeReport {
    pub status: String,
    pub open_error: Option<u32>,
    pub process_error: Option<u32>,
    pub close_error: Option<u32>,
    pub trace_qpc_frequency: Option<i64>,
    pub clock_type: u32,
    pub buffers_lost: u32,
    pub events_seen: u64,
    pub unsupported_events: u64,
    /// Exact known DMA/metadata tuples excluded only from candidate attribution.
    #[serde(default)]
    pub ignored_irrelevant_events: u64,
    /// First 16 distinct unsupported DxgKrnl descriptors, with repeat counts.
    #[serde(default)]
    pub unsupported_event_kinds: Vec<WindowsEtwUnsupportedEventKind>,
    /// Occurrences whose descriptor was not retained in the bounded kind list.
    #[serde(default)]
    pub unlisted_unsupported_events: u64,
    pub property_errors: u64,
    pub first_property_error: Option<u32>,
    pub omitted_events: u64,
    pub event_budget_reached: bool,
    pub events: Vec<WindowsEtwDecodedEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwUnsupportedEventKind {
    #[serde(default)]
    pub provider: WindowsEtwProvider,
    pub event_id: u16,
    pub version: u8,
    pub opcode: u8,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsEtwDecodedEvent {
    #[serde(default)]
    pub provider: WindowsEtwProvider,
    pub event_id: u16,
    pub version: u8,
    pub opcode: u8,
    pub process_id: u32,
    pub thread_id: u32,
    /// Raw trace-clock timestamp; conversion requires clock_type, not merely frequency.
    pub timestamp: i64,
    /// Manifest property names and exact unsigned scalar bits, including pointer keys.
    pub fields: Vec<(String, u64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerGpuFrameSample {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_identity: Option<std::sync::Arc<crate::render_thread::RenderFrameIdentity>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission: Option<crate::render_thread::GpuSubmissionToken>,
    pub frame_index: u64,
    pub frame_ms: f64,
    pub particle_simulation_ms: f64,
    pub particle_prepare_ms: f64,
    pub particle_draw_ms: f64,
    /// Disjoint actual timestamp ranges for SpineAnimator geometry only.
    #[serde(default)]
    pub spine_draw_ms: f64,
    #[serde(default)]
    pub spine_draw_count: u64,
    #[serde(default)]
    pub spine_instances: u64,
    /// An instanced draw with both Spine and ordinary sources cannot have an
    /// exclusive Spine GPU duration. Keep rendering, but reject qualification.
    #[serde(default)]
    pub spine_mixed_source_batches: u64,
    pub effect_count: usize,
    pub emitter_count: usize,
    pub particle_buffer_bytes: u64,
    pub live: u64,
    pub spawned_last_substep: u64,
    pub dropped_last_substep: u64,
    pub collisions_last_substep: u64,
    pub invalid_last_substep: u64,
    pub indirect_instances: u64,
    pub dispatch_count: u64,
    pub particle_draw_count: u64,
}

/// Bounded failure evidence retained even when successful trace records are omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerRuntimeFailure {
    pub frame_index: u64,
    pub phase: String,
    pub rule_id: String,
    pub operation: String,
    pub entity_id: Option<String>,
    pub component_type: Option<String>,
    pub field_path: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerGameplayTraceRecord {
    pub frame_index: u64,
    pub phase: String,
    pub rule_id: String,
    pub operation: String,
    pub entity_id: Option<String>,
    pub component_type: Option<String>,
    pub field_path: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub source: Option<String>,
    pub result: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedPlayerRunReport {
    pub schema_version: String,
    pub run_id: String,
    pub mode: WindowedPlayerMode,
    pub project_path: String,
    pub runtime_package_path: String,
    pub scenario_id: String,
    pub status: WindowedPlayerLayerStatus,
    pub counters: WindowedPlayerCounters,
    pub package_summary: Option<WindowedPlayerPackageSummary>,
    pub asset_load_summary: Option<WindowedPlayerAssetLoadSummary>,
    pub gpu_binding_summary: Option<WindowedPlayerGpuBindingSummary>,
    pub renderer_summary: Option<WindowedPlayerRendererSummary>,
    pub screenshot_summary: Option<WindowedPlayerScreenshotSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_performance_summary: Option<WindowedPlayerFramePerformanceSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gameplay_trace_summary: Option<WindowedPlayerGameplayTraceSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_summary: Option<crate::runtime_audio::RuntimeAudioReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gameplay_trace_records: Vec<WindowedPlayerGameplayTraceRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_bind_receipt:
        Option<crate::project_runtime_module::ProjectRuntimeBindReceipt>,
    pub exit_code: Option<i32>,
    pub exit_reason: String,
    pub diagnostics: Vec<WindowedPlayerDiagnostic>,
}

impl WindowedPlayerRunReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == WindowedPlayerDiagnosticSeverity::Error)
    }

    /// Empty report for adapters that already executed the runtime elsewhere.
    /// Constructing a report must not load or execute a package a second time.
    pub fn base(request: &WindowedPlayerRunRequest) -> Self {
        Self {
            schema_version: WINDOWED_PLAYER_RUN_REPORT_SCHEMA_VERSION.to_string(),
            run_id: format!("windowed-player-{}", request.scenario_id),
            mode: request.mode,
            project_path: request.project_path.display().to_string(),
            runtime_package_path: request.runtime_package_path.display().to_string(),
            scenario_id: request.scenario_id.clone(),
            status: WindowedPlayerLayerStatus::default(),
            counters: WindowedPlayerCounters::new(request.frame_limit),
            package_summary: None,
            asset_load_summary: None,
            gpu_binding_summary: None,
            renderer_summary: None,
            screenshot_summary: Some(WindowedPlayerScreenshotSummary::not_requested()),
            frame_performance_summary: None,
            gameplay_trace_summary: None,
            audio_summary: None,
            gameplay_trace_records: Vec::new(),
            project_runtime_bind_receipt: None,
            exit_code: None,
            exit_reason: "not_started".to_string(),
            diagnostics: Vec::new(),
        }
    }
}

pub struct WindowedPlayerHost;

impl WindowedPlayerHost {
    pub fn run_headless_gate(request: WindowedPlayerRunRequest) -> WindowedPlayerRunReport {
        let mut report = WindowedPlayerRunReport::base(&request);
        report.status.request = "ok".to_string();

        if request.frame_limit == 0 {
            report.exit_code = Some(1);
            report.exit_reason = "failed".to_string();
            report.diagnostics.push(WindowedPlayerDiagnostic::error(
                "invalid_frame_limit",
                "request",
                "frame_limit must be greater than zero",
            ));
            return report;
        }

        if request.mode == WindowedPlayerMode::Windowed {
            report.status.surface = "native_host_required".to_string();
            report.status.present = "native_host_required".to_string();
            report.exit_code = Some(1);
            report.exit_reason = "native_host_required".to_string();
            report.diagnostics.push(WindowedPlayerDiagnostic::error(
                "native_window_host_required",
                "surface",
                "Windowed mode must be executed by the native window host; engine_runtime does not create OS windows",
            ));
            return report;
        }

        report.status.surface = "headless_gate".to_string();

        let load = load_runtime_package(&request.runtime_package_path);
        report.diagnostics.extend(convert_runtime_diagnostics(
            "package",
            &load.diagnostics.issues,
        ));
        let Some(package) = load.value else {
            report.status.package = "error".to_string();
            report.exit_code = Some(1);
            report.exit_reason = "failed".to_string();
            return report;
        };
        report.status.package = "ok".to_string();
        report.package_summary = Some(WindowedPlayerPackageSummary::from_package(&package));

        let (asset_summary, ready_handles) =
            load_runtime_assets_for_player(&package, &mut report.diagnostics);
        if asset_summary.ready_handle_count == asset_summary.requested_asset_count {
            report.status.asset = "ok".to_string();
        } else {
            report.status.asset = "error".to_string();
        }
        report.asset_load_summary = Some(asset_summary);
        if report.status.asset == "error" {
            report.exit_code = Some(1);
            report.exit_reason = "failed".to_string();
            return report;
        }
        let gpu_summary = build_gpu_resource_bindings_for_player(&ready_handles, &mut report);
        report.gpu_binding_summary = Some(gpu_summary);

        let world_result = hydrate_active_scene_into_world(&package);
        report.diagnostics.extend(convert_runtime_diagnostics(
            "scene",
            &world_result.diagnostics.issues,
        ));
        let Some((mut world, hydration_report)) = world_result.value else {
            report.status.scene = "error".to_string();
            report.status.world = "error".to_string();
            report.exit_code = Some(1);
            report.exit_reason = "failed".to_string();
            return report;
        };
        report.status.scene = "ok".to_string();
        report.status.world = "ok".to_string();
        report.counters.entity_count = world.entity_count();
        report.counters.archetype_count = world.archetype_count();
        report.counters.hydrated_entity_count = hydration_report.created_entity_count();
        report.counters.hydration_loaded_asset_count = hydration_report.loaded_asset_count();
        report.counters.hydration_dirty_record_count = hydration_report.initial_dirty_records.len();

        let mut host = EngineHostLoop::new(package.active_scene.id.clone());
        let mut trace = RuntimeTrace::new();
        for _ in 0..request.frame_limit {
            let output = host.tick(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                &mut world,
            );
            report.counters.frames_completed += 1;
            trace.events.extend(output.runtime_trace.events);
            if output.runtime_advanced {
                report.status.logic = "ok".to_string();
                report.status.input = "ok".to_string();
                report.status.physics = "ok".to_string();
            }
            if output.render_frame_report.is_some() {
                report.status.projection = "ok".to_string();
                report.status.render = "ok".to_string();
            }
            if let Some(render_thread_frame) = output.render_thread_frame {
                report.renderer_summary = Some(renderer_summary_from_render_thread_frame(
                    &render_thread_frame,
                ));
                report.status.rdg = render_thread_frame.report.rdg_status;
                report.status.rhi = render_thread_frame.report.rhi_status;
                report.status.present = render_thread_frame.report.present_status;
            }
        }
        report.counters.render_proxy_count = host.render_scene().proxies_len();
        report.counters.runtime_trace_event_count = trace.events.len();

        mark_missing_success_statuses(&mut report);
        report.exit_code = Some(if report.has_errors() { 1 } else { 0 });
        report.exit_reason = if report.has_errors() {
            "failed".to_string()
        } else {
            "completed".to_string()
        };
        report
    }
}

fn renderer_summary_from_render_thread_frame(
    frame: &crate::render_thread::RenderThreadFrameOutput,
) -> WindowedPlayerRendererSummary {
    WindowedPlayerRendererSummary {
        draw_item_count: frame.report.render_frame_report.draw_item_count,
        rhi_command_count: frame.renderer_output.rhi_command_plan.commands.len(),
        rhi_draw_command_count: frame
            .renderer_output
            .rhi_command_plan
            .commands
            .iter()
            .filter(|command| matches!(command, RhiCommand::Draw { .. }))
            .count(),
        graph_pass_count: frame.renderer_output.render_graph.passes.len(),
    }
}

fn load_runtime_assets_for_player(
    package: &RuntimePackage,
    diagnostics: &mut Vec<WindowedPlayerDiagnostic>,
) -> (WindowedPlayerAssetLoadSummary, Vec<RuntimeAssetHandle>) {
    let mut summary = WindowedPlayerAssetLoadSummary::empty();
    let mut ready_handles = Vec::new();
    let mut loader = RuntimeAssetLoader::new(
        package.package_dir.clone(),
        package.runtime_asset_index.clone(),
        package.runtime_asset_mount_table.clone(),
    );
    loader.mount_bundle("startup");
    for record in &package.assets.runtime_asset_index {
        summary.requested_asset_count += 1;
        let result = loader.load_by_id(record.asset_id.clone(), record.asset_type.clone());
        if let Ok(handle) = result {
            if loader.get_handle_state(&handle) == Some(RuntimeAssetLoadState::Ready) {
                summary.ready_handle_count += 1;
                ready_handles.push(handle);
            }
        }
    }
    summary.decoded_cache_count = loader.decoded_cache_len();
    summary.diagnostic_count = loader.diagnostics().entries().len();
    diagnostics.extend(
        loader
            .diagnostics()
            .entries()
            .iter()
            .filter(|diagnostic| diagnostic.state != AssetLoadState::Ok)
            .map(|diagnostic| WindowedPlayerDiagnostic {
                severity: WindowedPlayerDiagnosticSeverity::Error,
                code: diagnostic
                    .error_code
                    .as_ref()
                    .map(|code| format!("asset_{code:?}"))
                    .unwrap_or_else(|| "asset_load_error".to_string()),
                layer: "asset".to_string(),
                message: format!(
                    "asset load failed at {:?}: {}",
                    diagnostic.stage, diagnostic.asset_ref_id
                ),
                path: diagnostic.cooked_asset_id.clone(),
            }),
    );
    (summary, ready_handles)
}

fn build_gpu_resource_bindings_for_player(
    ready_handles: &[RuntimeAssetHandle],
    report: &mut WindowedPlayerRunReport,
) -> WindowedPlayerGpuBindingSummary {
    let mut summary = WindowedPlayerGpuBindingSummary::empty();
    let producer = RuntimeRenderAssetProducer::new();
    let mut resource_manager = RenderResourceManager::new();
    for handle in ready_handles {
        let (kind, usage) = render_asset_kind_and_usage_from_handle(handle);
        let prepare_request =
            RuntimeRenderAssetRequest::from_asset(0, kind, usage, handle.asset_id.clone());
        let record = RuntimeAssetRecordLike::from_handle(handle);
        let output = producer.produce_from_record(
            &prepare_request,
            &record.to_runtime_record(),
            &mut resource_manager,
        );
        if output.handle.status == RuntimeRenderAssetStatus::Ready {
            summary.requested_handle_count += 1;
            summary.prepare_request_count += 1;
            summary.prepared_asset_count += usize::from(output.typed_asset.is_some());
            summary.render_resource_request_count += usize::from(output.resource_request.is_some());
            summary.resident_resource_count += usize::from(output.handle.resource_handle.is_some());
        } else {
            summary.diagnostic_count += output.report.failed_count;
        }
    }
    if summary.render_resource_request_count > 0 {
        report.status.rhi = "resource_bindings_ready".to_string();
    }
    summary
}

fn render_asset_kind_and_usage_from_handle(
    handle: &RuntimeAssetHandle,
) -> (RuntimeRenderAssetKind, RuntimeRenderAssetUsage) {
    match (handle.asset_type.as_str(), handle.loader_kind.as_str()) {
        ("mesh", _) | ("model", _) | (_, "mesh") | (_, "model") => (
            RuntimeRenderAssetKind::Mesh,
            RuntimeRenderAssetUsage::MeshGeometry,
        ),
        ("material", _) | (_, "material") => (
            RuntimeRenderAssetKind::Material,
            RuntimeRenderAssetUsage::MaterialBinding,
        ),
        ("texture", _) | (_, "texture") => (
            RuntimeRenderAssetKind::Texture,
            RuntimeRenderAssetUsage::Sprite2DTexture,
        ),
        _ => (
            RuntimeRenderAssetKind::Texture,
            RuntimeRenderAssetUsage::Sprite2DTexture,
        ),
    }
}

struct RuntimeAssetRecordLike<'a> {
    handle: &'a RuntimeAssetHandle,
}

impl<'a> RuntimeAssetRecordLike<'a> {
    fn from_handle(handle: &'a RuntimeAssetHandle) -> Self {
        Self { handle }
    }

    fn to_runtime_record(&self) -> crate::runtime_asset::RuntimeAssetRecord {
        crate::runtime_asset::RuntimeAssetRecord {
            asset_guid: self.handle.asset_guid.clone(),
            asset_id: self.handle.asset_id.clone(),
            asset_type: self.handle.asset_type.clone(),
            sub_asset_id: self.handle.sub_asset_id.clone(),
            version: self.handle.version.clone(),
            cooked_asset_id: self.handle.cooked_asset_id.clone(),
            bundle_id: self.handle.bundle_id.clone(),
            loader_kind: self.handle.loader_kind.clone(),
            dependencies: Vec::new(),
            hash: None,
            size: None,
            flags: Vec::new(),
            source_map_debug: None,
        }
    }
}

fn mark_missing_success_statuses(report: &mut WindowedPlayerRunReport) {
    if report.status.logic == "not_started" {
        report.status.logic = "error".to_string();
        report.diagnostics.push(WindowedPlayerDiagnostic::error(
            "logic_tick_missing",
            "logic",
            "EngineHostLoop did not advance runtime logic",
        ));
    }
    if report.status.render == "not_started" {
        report.status.render = "error".to_string();
        report.diagnostics.push(WindowedPlayerDiagnostic::error(
            "render_missing",
            "render",
            "EngineHostLoop did not produce a render frame",
        ));
    }
    if report.status.rdg == "not_started" {
        report.status.rdg = "error".to_string();
        report.diagnostics.push(WindowedPlayerDiagnostic::error(
            "rdg_missing",
            "rdg",
            "Render graph status was not produced",
        ));
    }
    if report.status.rhi == "not_started" {
        report.status.rhi = "error".to_string();
        report.diagnostics.push(WindowedPlayerDiagnostic::error(
            "rhi_missing",
            "rhi",
            "RHI status was not produced",
        ));
    }
    if report.status.present == "not_started" {
        report.status.present = "error".to_string();
        report.diagnostics.push(WindowedPlayerDiagnostic::error(
            "present_missing",
            "present",
            "Present status was not produced",
        ));
    }
}

fn convert_runtime_diagnostics(
    layer: &str,
    diagnostics: &[RuntimeDiagnostic],
) -> Vec<WindowedPlayerDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| WindowedPlayerDiagnostic {
            severity: match diagnostic.severity {
                DiagnosticSeverity::Error => WindowedPlayerDiagnosticSeverity::Error,
                DiagnosticSeverity::Warning => WindowedPlayerDiagnosticSeverity::Warning,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_run::tests_support::write_minimal_runtime_package;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn windowed_player_request_serializes() {
        let request = WindowedPlayerRunRequest::headless_gate("runtime-package");
        let json = serde_json::to_string(&request).expect("request should serialize");

        assert!(json.contains("windowed_player_runtime_v1_gate"));
        assert!(json.contains("headless-gate"));
    }

    #[test]
    fn windowed_player_request_maps_to_default_game_run_request() {
        let request = WindowedPlayerRunRequest::windowed("runtime-package");
        let default_request = request.default_game_run_request();

        assert_eq!(default_request.mode, DefaultGameRunMode::Windowed);
        assert!(default_request.launch_runtime_process);
        assert_eq!(default_request.frame_limit, request.frame_limit);
    }

    #[test]
    fn windowed_player_headless_gate_runs_runtime_package() {
        let root = temp_root("headless-gate");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(package_dir),
        );

        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.exit_reason, "completed");
        assert_eq!(report.status.package, "ok");
        assert_eq!(report.status.scene, "ok");
        assert_eq!(report.status.world, "ok");
        assert_eq!(report.status.logic, "ok");
        assert_eq!(report.status.render, "ok");
        assert_eq!(report.status.surface, "headless_gate");
        assert_eq!(report.counters.frames_completed, 3);
        assert_eq!(report.counters.entity_count, 1);
    }

    #[test]
    fn windowed_player_package_summary_records_loaded_runtime_package() {
        let root = temp_root("package-summary");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(package_dir),
        );

        let summary = report
            .package_summary
            .expect("loaded package should produce summary");
        assert_eq!(summary.project_name, "Runtime CLI Test");
        assert_eq!(summary.active_scene_id, "scene-main");
        assert_eq!(summary.scene_count, 1);
        assert_eq!(summary.declared_asset_count, 1);
        assert_eq!(summary.rule_mode, "none");
        assert_eq!(summary.content_hash.as_deref(), Some("testhash"));
    }

    #[test]
    fn windowed_player_package_failure_has_no_package_summary() {
        let root = temp_root("package-summary-failure");
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(root.join("missing-package")),
        );

        assert_eq!(report.status.package, "error");
        assert!(report.package_summary.is_none());
    }

    #[test]
    fn windowed_player_world_hydration_records_counts() {
        let root = temp_root("world-hydration");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(package_dir),
        );

        assert_eq!(report.status.scene, "ok");
        assert_eq!(report.status.world, "ok");
        assert_eq!(report.counters.hydrated_entity_count, 1);
        assert_eq!(report.counters.entity_count, 1);
        assert!(report.counters.archetype_count > 0);
        assert!(report.counters.hydration_dirty_record_count > 0);
    }

    #[test]
    fn windowed_player_asset_loads_runtime_asset_records() {
        let root = temp_root("asset-load");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(package_dir),
        );

        let summary = report
            .asset_load_summary
            .expect("loaded package should produce asset summary");
        assert_eq!(report.status.asset, "ok");
        assert_eq!(summary.requested_asset_count, 1);
        assert_eq!(summary.ready_handle_count, 1);
        assert_eq!(summary.decoded_cache_count, 1);
    }

    #[test]
    fn windowed_player_gpu_binding_builds_render_resource_requests() {
        let root = temp_root("gpu-binding");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        add_texture_runtime_asset(&package_dir);
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(package_dir),
        );

        let gpu = report
            .gpu_binding_summary
            .expect("loaded assets should produce gpu binding summary");
        assert_eq!(report.status.asset, "ok");
        assert_eq!(gpu.requested_handle_count, 1);
        assert_eq!(gpu.prepare_request_count, 1);
        assert_eq!(gpu.prepared_asset_count, 1);
        assert_eq!(gpu.render_resource_request_count, 1);
        assert_eq!(gpu.resident_resource_count, 1);
    }

    #[test]
    fn windowed_player_projection_creates_render_proxy_for_renderable_scene() {
        let root = temp_root("projection");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        add_texture_runtime_asset(&package_dir);
        add_mesh_renderer_to_scene(&package_dir);
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(package_dir),
        );

        assert_eq!(report.status.projection, "ok");
        assert_eq!(report.status.render, "ok");
        assert!(report.counters.render_proxy_count > 0);
    }

    #[test]
    fn windowed_player_renderer_generates_rhi_draw_commands() {
        let root = temp_root("renderer");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        add_texture_runtime_asset(&package_dir);
        add_mesh_renderer_to_scene(&package_dir);
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(package_dir),
        );

        let renderer = report
            .renderer_summary
            .expect("renderable scene should produce renderer summary");
        assert!(renderer.draw_item_count > 0);
        assert!(renderer.rhi_command_count > 0);
        assert!(renderer.rhi_draw_command_count > 0);
        assert!(renderer.graph_pass_count > 0);
    }

    #[test]
    fn windowed_player_frameloop_runs_multiple_frames_stably() {
        let root = temp_root("frameloop");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        add_texture_runtime_asset(&package_dir);
        add_mesh_renderer_to_scene(&package_dir);
        let mut request = WindowedPlayerRunRequest::headless_gate(package_dir);
        request.frame_limit = 120;
        let report = WindowedPlayerHost::run_headless_gate(request);

        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.counters.frames_completed, 120);
        assert_eq!(report.status.logic, "ok");
        assert_eq!(report.status.input, "ok");
        assert_eq!(report.status.physics, "ok");
        assert_eq!(report.status.render, "ok");
        assert!(report.counters.runtime_trace_event_count >= 120);
    }

    #[test]
    fn windowed_player_windowed_mode_requires_native_host() {
        let report = WindowedPlayerHost::run_headless_gate(WindowedPlayerRunRequest::windowed(
            "runtime-package",
        ));

        assert_eq!(report.exit_code, Some(1));
        assert_eq!(report.exit_reason, "native_host_required");
        assert_eq!(report.status.surface, "native_host_required");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "native_window_host_required"));
    }

    #[test]
    fn windowed_player_report_locates_package_failure() {
        let root = temp_root("missing-package");
        let report = WindowedPlayerHost::run_headless_gate(
            WindowedPlayerRunRequest::headless_gate(root.join("missing-package")),
        );

        assert_eq!(report.exit_code, Some(1));
        assert_eq!(report.status.package, "error");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.layer == "package"));
    }

    #[test]
    fn windowed_player_report_serializes() {
        let mut report =
            WindowedPlayerRunReport::base(&WindowedPlayerRunRequest::headless_gate("runtime"));
        report.diagnostics.push(WindowedPlayerDiagnostic::error(
            "present_missing",
            "present",
            "missing present",
        ));

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains(WINDOWED_PLAYER_RUN_REPORT_SCHEMA_VERSION));
        assert!(json.contains("present_missing"));
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("windowed-player-{name}-{stamp}"))
    }

    fn add_texture_runtime_asset(package_dir: &std::path::Path) {
        std::fs::write(
            package_dir.join("assets").join("texture-main.bin"),
            [255_u8, 255, 255, 255],
        )
        .unwrap();
        std::fs::write(
            package_dir.join("assets").join("asset-manifest.json"),
            r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [
    {
      "id": "scene-main",
      "name": "Main",
      "type": "scene",
      "source": "scenes/scene-main.json",
      "state": "available",
      "bundleId": "startup"
    },
    {
      "id": "texture-main",
      "name": "Texture Main",
      "type": "texture",
      "source": "assets/texture-main.bin",
      "state": "available",
      "bundleId": "startup"
    }
  ],
  "runtimeAssetIndex": [
    {
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
    },
    {
      "assetGuid": "texture-main",
      "assetId": "texture-main",
      "assetType": "texture",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-texture-main",
      "bundleId": "startup",
      "loaderKind": "texture",
      "dependencies": [],
      "hash": null,
      "size": 4,
      "flags": ["test"]
    }
  ],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [
    {
      "cookedAssetId": "cooked-scene-main",
      "bundleId": "startup",
      "path": "scenes/scene-main.json",
      "offset": null,
      "size": null,
      "compression": "none",
      "hash": null
    },
    {
      "cookedAssetId": "cooked-texture-main",
      "bundleId": "startup",
      "path": "assets/texture-main.bin",
      "offset": null,
      "size": 4,
      "compression": "none",
      "hash": null
    }
  ],
  "dependencyTable": []
}"#,
        )
        .unwrap();
    }

    fn add_mesh_renderer_to_scene(package_dir: &std::path::Path) {
        std::fs::write(
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
    "kind": "player",
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
      "label": "PlayerQuad",
      "assetRef": null,
      "materialRef": null,
      "textureRef": { "id": "texture-main", "type": "texture", "guid": null, "subAsset": null },
      "visible": true,
      "layer": "Default",
      "metalness": 0,
      "roughness": 1
    }
  }]
}"##,
        )
        .unwrap();
    }
}
