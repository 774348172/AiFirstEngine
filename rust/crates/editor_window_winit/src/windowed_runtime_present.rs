use serde::{Deserialize, Serialize};

use crate::{
    HeadlessSurfaceBackend, HeadlessWindowBackend, NativeEditorWindowConfig, SurfaceState,
    WindowState,
};
use engine_runtime::components::{Hierarchy, Renderable, Transform};
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::ids::EntityId;
use engine_runtime::render_state::{
    RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
};
use engine_runtime::runtime_renderer::RenderTarget;
use engine_runtime::world::World;
#[cfg(feature = "real-wgpu-surface")]
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedRuntimeConfig {
    pub window_id: String,
    pub target_id: String,
    pub width: u32,
    pub height: u32,
    pub surface_format: String,
    pub present_mode: String,
}

impl Default for WindowedRuntimeConfig {
    fn default() -> Self {
        Self {
            window_id: "main-window".to_string(),
            target_id: "main-surface".to_string(),
            width: 1280,
            height: 720,
            surface_format: "Bgra8UnormSrgb".to_string(),
            present_mode: "Fifo".to_string(),
        }
    }
}

impl WindowedRuntimeConfig {
    pub fn window_config(&self) -> NativeEditorWindowConfig {
        NativeEditorWindowConfig {
            title: "AI First Engine Runtime".to_string(),
            width: self.width,
            height: self.height,
            resizable: true,
            scale_factor: 1.0,
        }
    }

    pub fn surface_target(&self) -> RenderTarget {
        RenderTarget::surface(self.target_id.clone(), self.width, self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedRuntimeFrameInput {
    pub frame_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealSurfaceTarget {
    pub target_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub present_mode: String,
}

impl RealSurfaceTarget {
    fn from_config(config: &WindowedRuntimeConfig) -> Self {
        Self {
            target_id: config.target_id.clone(),
            width: config.width,
            height: config.height,
            format: config.surface_format.clone(),
            present_mode: config.present_mode.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RealSurfacePresentStatus {
    Presented,
    NotPresented,
    AcquireFailed,
    SubmitFailed,
    PresentFailed,
    SurfaceLost,
}

impl RealSurfacePresentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Presented => "presented",
            Self::NotPresented => "not_presented",
            Self::AcquireFailed => "acquire_failed",
            Self::SubmitFailed => "submit_failed",
            Self::PresentFailed => "present_failed",
            Self::SurfaceLost => "surface_lost",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedRuntimeDiagnostic {
    pub severity: WindowedRuntimeDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub layer: String,
}

impl WindowedRuntimeDiagnostic {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        layer: impl Into<String>,
    ) -> Self {
        Self {
            severity: WindowedRuntimeDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            layer: layer.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowedRuntimeDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedRuntimePresentReport {
    pub schema_version: String,
    pub frame_index: u64,
    pub window_id: String,
    pub target: RealSurfaceTarget,
    pub window: WindowState,
    pub surface: SurfaceState,
    pub runtime_tick_status: String,
    pub render_thread_status: String,
    pub rdg_status: String,
    pub rhi_status: String,
    pub acquire_status: String,
    pub submit_status: String,
    pub present_status: RealSurfacePresentStatus,
    pub render_thread_report_schema: Option<String>,
    pub texture_lifetime_event_count: usize,
    pub diagnostics: Vec<WindowedRuntimeDiagnostic>,
}

impl WindowedRuntimePresentReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == WindowedRuntimeDiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowedRuntimeFrameOutput {
    pub report: WindowedRuntimePresentReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlessWindowedRuntimeFailure {
    SurfaceLost,
    AcquireBeforeConfigure,
    PresentWithoutAcquire,
}

pub struct HeadlessWindowedRuntimePresentBackend {
    config: WindowedRuntimeConfig,
    window: HeadlessWindowBackend,
    surface: HeadlessSurfaceBackend,
    engine_host: EngineHostLoop,
    world: World,
}

impl HeadlessWindowedRuntimePresentBackend {
    pub fn new(config: WindowedRuntimeConfig) -> Self {
        let window = HeadlessWindowBackend::create_window(&config.window_config());
        let mut surface = HeadlessSurfaceBackend::create_surface();
        surface.configure(
            config.width,
            config.height,
            config.surface_format.clone(),
            config.present_mode.clone(),
        );
        let mut engine_host = EngineHostLoop::new("scene-main");
        engine_host
            .render_scene_mut()
            .register_view(RenderViewState::new(
                RenderViewId(1),
                RenderViewKind::Game,
                RenderTargetKind::Window,
            ));
        Self {
            config,
            window,
            surface,
            engine_host,
            world: minimal_world(),
        }
    }

    pub fn present_one_frame(&mut self) -> WindowedRuntimeFrameOutput {
        self.present_one_frame_with_failure(None)
    }

    pub fn present_one_frame_with_failure(
        &mut self,
        failure: Option<HeadlessWindowedRuntimeFailure>,
    ) -> WindowedRuntimeFrameOutput {
        if matches!(
            failure,
            Some(HeadlessWindowedRuntimeFailure::AcquireBeforeConfigure)
        ) {
            self.surface = HeadlessSurfaceBackend::create_surface();
        }
        if matches!(failure, Some(HeadlessWindowedRuntimeFailure::SurfaceLost)) {
            self.surface.lose_surface("surface_lost");
        }

        let engine_output = self.engine_host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut self.world,
        );
        let render_thread_frame = self
            .engine_host
            .render_thread_for_target(self.config.surface_target());

        if matches!(failure, Some(HeadlessWindowedRuntimeFailure::SurfaceLost)) {
            // Preserve the surface_lost diagnostic. A real backend cannot acquire
            // from a lost surface until it is reconfigured.
        } else if matches!(
            failure,
            Some(HeadlessWindowedRuntimeFailure::PresentWithoutAcquire)
        ) {
            let mut reset_surface = HeadlessSurfaceBackend::create_surface();
            reset_surface.configure(
                self.config.width,
                self.config.height,
                self.config.surface_format.clone(),
                self.config.present_mode.clone(),
            );
            self.surface = reset_surface;
            self.surface.present();
        } else {
            self.surface.acquire();
            self.surface.present();
        }

        WindowedRuntimeFrameOutput {
            report: build_windowed_runtime_present_report(
                &self.config,
                self.window.snapshot(),
                self.surface.snapshot(),
                engine_output.runtime_advanced,
                render_thread_frame,
            ),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> WindowedRuntimeFrameOutput {
        self.config.width = width;
        self.config.height = height;
        self.window.resize(width, height, 1.0);
        self.surface.resize(
            width,
            height,
            self.config.surface_format.clone(),
            self.config.present_mode.clone(),
        );
        self.present_one_frame()
    }
}

pub fn build_windowed_runtime_present_report(
    config: &WindowedRuntimeConfig,
    window: WindowState,
    surface: SurfaceState,
    runtime_advanced: bool,
    render_thread_frame: engine_runtime::render_thread::RenderThreadFrameOutput,
) -> WindowedRuntimePresentReport {
    let mut diagnostics = Vec::new();
    if !surface.configured {
        diagnostics.push(WindowedRuntimeDiagnostic::error(
            "surface_not_configured",
            "surface is not configured before present",
            "surface",
        ));
    }
    if let Some(error) = &surface.last_error {
        diagnostics.push(WindowedRuntimeDiagnostic::error(
            error.clone(),
            format!("surface reported {error}"),
            "surface",
        ));
    }
    for diagnostic in &render_thread_frame.report.diagnostics {
        if matches!(
            diagnostic.severity,
            engine_runtime::render_thread::RenderThreadDiagnosticSeverity::Error
        ) {
            diagnostics.push(WindowedRuntimeDiagnostic::error(
                diagnostic.code.clone(),
                diagnostic.message.clone(),
                "render_thread",
            ));
        }
    }

    let acquire_status = if surface.acquired_frame > 0 && surface.configured {
        "ok"
    } else {
        "error"
    }
    .to_string();
    let submit_status = if render_thread_frame.report.rhi_status == "ok" {
        "ok"
    } else {
        "error"
    }
    .to_string();
    let present_status = if surface.last_error.as_deref() == Some("surface_frame_not_acquired") {
        RealSurfacePresentStatus::PresentFailed
    } else if surface.last_error.as_deref() == Some("surface_lost") || !surface.configured {
        RealSurfacePresentStatus::SurfaceLost
    } else if acquire_status == "error" {
        RealSurfacePresentStatus::AcquireFailed
    } else if submit_status == "error" {
        RealSurfacePresentStatus::SubmitFailed
    } else if surface.presented_frame > 0 {
        RealSurfacePresentStatus::Presented
    } else {
        RealSurfacePresentStatus::PresentFailed
    };

    WindowedRuntimePresentReport {
        schema_version: "windowed-runtime-present-report.v1".to_string(),
        frame_index: render_thread_frame.report.frame_index,
        window_id: config.window_id.clone(),
        target: RealSurfaceTarget::from_config(config),
        window,
        surface,
        runtime_tick_status: if runtime_advanced {
            "advanced"
        } else {
            "skipped"
        }
        .to_string(),
        render_thread_status: if render_thread_frame.report.present_status == "presented" {
            "rendered"
        } else {
            "not_rendered"
        }
        .to_string(),
        rdg_status: render_thread_frame.report.rdg_status.clone(),
        rhi_status: render_thread_frame.report.rhi_status.clone(),
        acquire_status,
        submit_status,
        present_status,
        render_thread_report_schema: Some(render_thread_frame.report.schema_version.clone()),
        texture_lifetime_event_count: render_thread_frame
            .report
            .texture_lifetime_report
            .events
            .len(),
        diagnostics,
    }
}

#[cfg(feature = "real-wgpu-surface")]
pub struct RealWindowedRuntimeSurfaceHost {
    config: WindowedRuntimeConfig,
    window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    backend: engine_runtime::wgpu_backend::real::RealWgpuBackend,
    engine_host: EngineHostLoop,
    world: World,
    surface_state: SurfaceState,
}

#[cfg(feature = "real-wgpu-surface")]
impl RealWindowedRuntimeSurfaceHost {
    pub fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        config: WindowedRuntimeConfig,
    ) -> Result<Self, String> {
        let window_attributes = crate::winit_window_attributes(&config.window_config());
        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .map_err(|error| format!("window.create_failed:{error}"))?,
        );
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Err("window.zero_sized_surface".to_string());
        }

        let instance = wgpu::Instance::new(&crate::wgpu_instance_descriptor());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| format!("surface.create_failed:{error}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|error| format!("surface.request_adapter_failed:{error}"))?;
        let backend_name = format!("{:?}", adapter.get_info().backend);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("runtime-windowed-wgpu-device"),
            required_features: wgpu::Features::empty(),
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| format!("surface.request_device_failed:{error}"))?;
        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or_else(|| "surface.default_config_unavailable".to_string())?;
        surface.configure(&device, &surface_config);
        let backend = engine_runtime::wgpu_backend::real::RealWgpuBackend::from_device_queue(
            device,
            queue,
            surface_config.format,
            size.width,
            size.height,
            backend_name,
        );
        let surface_format = format!("{:?}", surface_config.format);
        let surface_present_mode = format!("{:?}", surface_config.present_mode);
        let mut engine_host = EngineHostLoop::new("scene-main");
        engine_host
            .render_scene_mut()
            .register_view(RenderViewState::new(
                RenderViewId(1),
                RenderViewKind::Game,
                RenderTargetKind::Window,
            ));

        Ok(Self {
            config,
            window,
            surface,
            backend,
            engine_host,
            world: minimal_world(),
            surface_state: SurfaceState {
                created: true,
                configured: true,
                format: surface_format,
                present_mode: surface_present_mode,
                width: size.width,
                height: size.height,
                acquired_frame: 0,
                presented_frame: 0,
                last_error: None,
            },
        })
    }

    pub fn present_one_frame(&mut self) -> WindowedRuntimeFrameOutput {
        let engine_output = self.engine_host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut self.world,
        );
        let renderer_output = self
            .engine_host
            .render_thread_for_target(self.config.surface_target());

        let surface_texture = match self.surface.get_current_texture() {
            Ok(surface_texture) => {
                self.surface_state.acquired_frame += 1;
                self.surface_state.last_error = None;
                surface_texture
            }
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface_state.configured = false;
                self.surface_state.last_error = Some("surface_lost".to_string());
                return WindowedRuntimeFrameOutput {
                    report: build_windowed_runtime_present_report(
                        &self.config,
                        self.window_state(),
                        self.surface_state.clone(),
                        engine_output.runtime_advanced,
                        renderer_output,
                    ),
                };
            }
            Err(error) => {
                self.surface_state.last_error = Some(format!("surface_acquire_failed:{error}"));
                return WindowedRuntimeFrameOutput {
                    report: build_windowed_runtime_present_report(
                        &self.config,
                        self.window_state(),
                        self.surface_state.clone(),
                        engine_output.runtime_advanced,
                        renderer_output,
                    ),
                };
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let backend_report = self
            .backend
            .execute_plan_to_surface_view(&renderer_output.renderer_output.rhi_command_plan, &view);
        surface_texture.present();
        self.surface_state.presented_frame = self.surface_state.acquired_frame;
        self.surface_state.configured = true;
        self.surface_state.last_error = if backend_report.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.severity,
                engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity::Error
            )
        }) {
            Some("rhi_backend_error".to_string())
        } else {
            None
        };

        WindowedRuntimeFrameOutput {
            report: build_windowed_runtime_present_report(
                &self.config,
                self.window_state(),
                self.surface_state.clone(),
                engine_output.runtime_advanced,
                renderer_output,
            ),
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    fn window_state(&self) -> WindowState {
        let size = self.window.inner_size();
        WindowState {
            created: true,
            size: crate::WindowSize {
                width: size.width,
                height: size.height,
            },
            scale_factor: self.window.scale_factor(),
            close_requested: false,
            redraw_requested: false,
        }
    }
}

#[cfg(feature = "real-wgpu-surface")]
pub fn real_windowed_runtime_present_smoke_plan() -> WindowedRuntimePresentReport {
    real_windowed_runtime_surface_smoke::run(WindowedRuntimeConfig::default())
}

#[cfg(feature = "real-wgpu-surface")]
pub fn windowed_end_to_end_game_smoke_plan() -> WindowedRuntimePresentReport {
    real_windowed_runtime_present_smoke_plan()
}

#[cfg(feature = "real-wgpu-surface")]
mod real_windowed_runtime_surface_smoke {
    use super::*;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    #[cfg(target_os = "windows")]
    use winit::platform::windows::EventLoopBuilderExtWindows;

    struct SmokeApp {
        config: WindowedRuntimeConfig,
        host: Option<RealWindowedRuntimeSurfaceHost>,
        report: Option<WindowedRuntimePresentReport>,
    }

    impl SmokeApp {
        fn new(config: WindowedRuntimeConfig) -> Self {
            Self {
                config,
                host: None,
                report: None,
            }
        }
    }

    impl ApplicationHandler for SmokeApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            match RealWindowedRuntimeSurfaceHost::new(event_loop, self.config.clone()) {
                Ok(host) => {
                    host.request_redraw();
                    self.host = Some(host);
                }
                Err(error) => {
                    self.report = Some(environment_blocked_report(&self.config, error));
                    event_loop.exit();
                }
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::RedrawRequested => {
                    if let Some(host) = &mut self.host {
                        let output = host.present_one_frame();
                        self.report = Some(output.report);
                    }
                    event_loop.exit();
                }
                WindowEvent::CloseRequested => event_loop.exit(),
                _ => {}
            }
        }
    }

    pub fn run(config: WindowedRuntimeConfig) -> WindowedRuntimePresentReport {
        #[cfg(target_os = "windows")]
        let event_loop_result = EventLoop::builder().with_any_thread(true).build();
        #[cfg(not(target_os = "windows"))]
        let event_loop_result = EventLoop::new();

        let event_loop = match event_loop_result {
            Ok(event_loop) => event_loop,
            Err(error) => return environment_blocked_report(&config, error.to_string()),
        };
        let mut app = SmokeApp::new(config.clone());
        match event_loop.run_app(&mut app) {
            Ok(()) => app
                .report
                .unwrap_or_else(|| environment_blocked_report(&config, "smoke report missing")),
            Err(error) => environment_blocked_report(&config, error.to_string()),
        }
    }

    fn environment_blocked_report(
        config: &WindowedRuntimeConfig,
        error: impl Into<String>,
    ) -> WindowedRuntimePresentReport {
        let mut surface = HeadlessSurfaceBackend::create_surface();
        surface.lose_surface("environment_blocked");
        let mut backend = HeadlessWindowedRuntimePresentBackend::new(config.clone());
        let mut report = build_windowed_runtime_present_report(
            config,
            HeadlessWindowBackend::create_window(&config.window_config()).snapshot(),
            surface.snapshot(),
            false,
            backend
                .engine_host
                .render_thread_for_target(config.surface_target()),
        );
        report.diagnostics.push(WindowedRuntimeDiagnostic::error(
            "real_window_environment_blocked",
            error.into(),
            "window",
        ));
        report
    }
}

fn minimal_world() -> World {
    let mut world = World::new();
    world
        .try_spawn_with_components(
            EntityId::from("entity-windowed-present"),
            "Windowed Present Test",
            "actor",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
            Some(Transform::identity()),
            Some(Renderable {
                mesh_ref: None,
                material_ref: Some("material-test".to_string()),
                visible: true,
                layer: "default".to_string(),
            }),
        )
        .expect("static windowed present fixture must be valid");
    world.take_dirty_records();
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windowed_runtime_present_report_is_json_serializable() {
        let mut backend =
            HeadlessWindowedRuntimePresentBackend::new(WindowedRuntimeConfig::default());

        let output = backend.present_one_frame();
        let json = serde_json::to_string(&output.report).expect("report should serialize");

        assert_eq!(
            output.report.schema_version,
            "windowed-runtime-present-report.v1"
        );
        assert!(json.contains("windowed-runtime-present-report.v1"));
        assert_eq!(
            output.report.present_status,
            RealSurfacePresentStatus::Presented
        );
    }

    #[test]
    fn headless_windowed_runtime_presents_one_frame() {
        let mut backend =
            HeadlessWindowedRuntimePresentBackend::new(WindowedRuntimeConfig::default());

        let output = backend.present_one_frame();

        assert_eq!(output.report.runtime_tick_status, "advanced");
        assert_eq!(output.report.render_thread_status, "rendered");
        assert_eq!(output.report.acquire_status, "ok");
        assert_eq!(output.report.submit_status, "ok");
        assert_eq!(
            output.report.present_status,
            RealSurfacePresentStatus::Presented
        );
        assert_eq!(
            output.report.render_thread_report_schema.as_deref(),
            Some("render-thread-report.v1")
        );
        assert!(!output.report.has_errors());
    }

    #[test]
    fn headless_windowed_runtime_reports_resize() {
        let mut backend =
            HeadlessWindowedRuntimePresentBackend::new(WindowedRuntimeConfig::default());

        let output = backend.resize(800, 450);

        assert_eq!(output.report.target.width, 800);
        assert_eq!(output.report.target.height, 450);
        assert_eq!(output.report.surface.width, 800);
        assert_eq!(output.report.surface.height, 450);
        assert_eq!(
            output.report.present_status,
            RealSurfacePresentStatus::Presented
        );
    }

    #[test]
    fn headless_windowed_runtime_reports_surface_lost() {
        let mut backend =
            HeadlessWindowedRuntimePresentBackend::new(WindowedRuntimeConfig::default());

        let output = backend
            .present_one_frame_with_failure(Some(HeadlessWindowedRuntimeFailure::SurfaceLost));

        assert_eq!(
            output.report.present_status,
            RealSurfacePresentStatus::SurfaceLost
        );
        assert!(output
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "surface_lost"));
    }

    #[test]
    fn windowed_runtime_present_report_locates_surface_failure() {
        let mut backend =
            HeadlessWindowedRuntimePresentBackend::new(WindowedRuntimeConfig::default());

        let output = backend.present_one_frame_with_failure(Some(
            HeadlessWindowedRuntimeFailure::AcquireBeforeConfigure,
        ));

        assert_eq!(
            output.report.present_status,
            RealSurfacePresentStatus::PresentFailed
        );
        assert!(output
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.layer == "surface"));
    }

    #[test]
    fn windowed_runtime_present_failure_generates_diagnostic() {
        let mut backend =
            HeadlessWindowedRuntimePresentBackend::new(WindowedRuntimeConfig::default());

        let output = backend.present_one_frame_with_failure(Some(
            HeadlessWindowedRuntimeFailure::PresentWithoutAcquire,
        ));

        assert_eq!(
            output.report.present_status,
            RealSurfacePresentStatus::PresentFailed
        );
        assert!(output
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "surface_frame_not_acquired"));
    }

    #[cfg(feature = "real-wgpu-surface")]
    #[test]
    #[ignore = "real window / gpu smoke gate is local-only"]
    fn real_windowed_runtime_present_smoke() {
        let report = real_windowed_runtime_present_smoke_plan();

        assert_eq!(report.schema_version, "windowed-runtime-present-report.v1");
        assert_eq!(report.present_status, RealSurfacePresentStatus::Presented);
    }

    #[cfg(feature = "real-wgpu-surface")]
    #[test]
    #[ignore = "real window / gpu smoke gate is local-only"]
    fn windowed_end_to_end_game_smoke() {
        let report = windowed_end_to_end_game_smoke_plan();

        assert_eq!(report.schema_version, "windowed-runtime-present-report.v1");
        assert_eq!(report.runtime_tick_status, "advanced");
        assert_eq!(report.present_status, RealSurfacePresentStatus::Presented);
    }
}
