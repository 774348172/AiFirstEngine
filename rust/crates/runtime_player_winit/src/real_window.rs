use super::*;
use engine_runtime::runtime_renderer::font_atlas_render_handle;
use engine_runtime::runtime_texture::{RuntimeTextureBindingContext, RuntimeTextureUploadRegistry};
use engine_runtime::sprite2d_render_pipeline::Sprite2DTextureBindingContext;
use std::sync::Arc;
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

#[cfg(all(test, target_os = "windows"))]
#[path = "tests/real_window_terminal.rs"]
mod terminal_tests;

#[cfg(not(target_os = "android"))]
pub fn run_windowed(
    request: NativePlayerWindowRunRequest,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
) -> NativeWindowHostReport {
    run_windowed_controlled(request, linked_modules, None)
}

pub(super) fn run_windowed_controlled(
    request: NativePlayerWindowRunRequest,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
    semantic: Option<(semantic_outcome::PlaytestScenario, PathBuf)>,
) -> NativeWindowHostReport {
    #[cfg(target_os = "windows")]
    let event_loop_result = EventLoop::builder().with_any_thread(true).build();
    #[cfg(not(target_os = "windows"))]
    let event_loop_result = EventLoop::new();
    run_event_loop(event_loop_result, request, linked_modules, semantic)
}

#[cfg(target_os = "android")]
pub fn run_android(
    android_app: AndroidApp,
    request: NativePlayerWindowRunRequest,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
) -> NativeWindowHostReport {
    let event_loop_result = EventLoop::builder().with_android_app(android_app).build();
    run_event_loop(event_loop_result, request, linked_modules, None)
}

fn run_event_loop(
    event_loop_result: Result<EventLoop<()>, winit::error::EventLoopError>,
    request: NativePlayerWindowRunRequest,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
    semantic: Option<(semantic_outcome::PlaytestScenario, PathBuf)>,
) -> NativeWindowHostReport {
    let event_loop = match event_loop_result {
        Ok(event_loop) => event_loop,
        Err(error) => return environment_blocked_report(request, error.to_string()),
    };
    let mut app = RealWindowApp::new(request, linked_modules, semantic);
    match event_loop.run_app(&mut app) {
        Ok(()) => app
            .report
            .unwrap_or_else(|| environment_blocked_report(app.request, "window report missing")),
        Err(error) => environment_blocked_report(app.request, error.to_string()),
    }
}

struct RealWindowApp {
    semantic: Option<(semantic_outcome::PlaytestScenario, PathBuf)>,
    request: NativePlayerWindowRunRequest,
    host: Option<RealWindowHost>,
    report: Option<NativeWindowHostReport>,
    pending_raw_input: Vec<RawInputEvent>,
    last_cursor_position: Option<(f32, f32)>,
    frame_id: u64,
    next_redraw_at: Instant,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
    lifecycle: NativePlayerLifecycleState,
}

impl RealWindowApp {
    fn new(
        request: NativePlayerWindowRunRequest,
        linked_modules: Arc<LinkedProjectRuntimeSet>,
        semantic: Option<(semantic_outcome::PlaytestScenario, PathBuf)>,
    ) -> Self {
        Self {
            semantic,
            request,
            host: None,
            report: None,
            pending_raw_input: Vec::new(),
            last_cursor_position: None,
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
        if let Some(host) = self.host.as_mut() {
            if let Some(session) = host.session.as_mut() {
                session.host.set_audio_suspended(false);
            }
            self.request_immediate_redraw();
            return;
        }
        match RealWindowHost::new(
            event_loop,
            self.request.clone(),
            Arc::clone(&self.linked_modules),
            self.semantic.clone(),
        ) {
            Ok(host) => {
                host.window.request_redraw();
                self.next_redraw_at = Instant::now();
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
        self.last_cursor_position = None;
        if let Some(host) = self.host.as_mut() {
            if let Some(session) = host.session.as_mut() {
                session.host.set_audio_suspended(true);
            }
            host.window.set_ime_allowed(false);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                if self.request.config.target_fps != 60 && Instant::now() < self.next_redraw_at {
                    return;
                }
                if let Some(host) = &mut self.host {
                    let frame_started = Instant::now();
                    let raw_input =
                        if host.pending_frame.is_some() || host.pending_geometry_frame.is_some() {
                            Vec::new()
                        } else {
                            std::mem::take(&mut self.pending_raw_input)
                        };
                    match host.present_next_frame(raw_input) {
                        RealWindowFrameAdvance::Continue => {
                            self.next_redraw_at = next_redraw_deadline(
                                self.next_redraw_at,
                                frame_started,
                                Instant::now(),
                                self.request.config.target_fps,
                                host.session
                                    .as_ref()
                                    .is_some_and(|s| s.world.has_spatial_entities()),
                            );
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
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(host) = &mut self.host {
                    let size = host.window.inner_size();
                    host.resize_surface(size.width, size.height);
                    host.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.frame_id += 1;
                if let Some(raw_event) = raw_keyboard_event(self.frame_id, window_id, &event) {
                    self.pending_raw_input.push(raw_event);
                }
                if let Some(raw_event) = raw_text_input_event(self.frame_id, window_id, &event) {
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
                let position = (position.x as f32, position.y as f32);
                self.last_cursor_position = Some(position);
                self.pending_raw_input.push(RawInputEvent::mouse_move(
                    self.frame_id,
                    format!("{window_id:?}"),
                    position.0,
                    position.1,
                ));
                self.request_immediate_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.frame_id += 1;
                self.pending_raw_input.push(native_mouse_leave_raw_event(
                    self.frame_id,
                    format!("{window_id:?}"),
                    self.last_cursor_position,
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
            WindowEvent::Focused(focused) => {
                if !focused {
                    if let Some(host) = self.host.as_mut() {
                        host.fixed_actions = FixedActionLatch::default();
                        host.window.set_ime_allowed(false);
                        if let Some(input) = host.aui_interaction_state.input_field.as_mut() {
                            input.cancel_composition();
                        }
                    }
                }
                if let Some(host) = self.host.as_mut() {
                    if host.ensure_session() {
                        let session = host.session.as_mut().expect("initialized session");
                        session
                            .host
                            .dispatch_focus_changed(focused, &mut session.world);
                        session.host.flush_control_audio(
                            &session.world,
                            session.hydrator.instance_loader_mut().asset_loader_mut(),
                        );
                        if session.host.host_control().is_some() {
                            host.report.session_exit_requested = true;
                            self.report = Some(host.finish(false));
                            event_loop.exit();
                            return;
                        }
                    }
                }
                if !focused {
                    self.frame_id += 1;
                    self.pending_raw_input.push(RawInputEvent::focus_lost(
                        self.frame_id,
                        format!("{window_id:?}"),
                    ));
                }
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
    aui_feedback_state: AuiControlFeedbackState,
    aui_performance_load: Option<AuiThemeDpiLoadState>,
}

const GATE_E_ALTERNATE_THEME_ID: &str = "gate-e.alternate-theme";

struct AuiThemeDpiLoadState {
    original_document_themes: std::collections::BTreeMap<
        String,
        Option<engine_runtime::runtime_package::RuntimeAssetRef>,
    >,
}

impl AuiThemeDpiLoadState {
    fn install(package: &mut RuntimePackage) -> Result<Self, String> {
        let original_document_themes = package
            .aui_documents
            .documents_by_id
            .iter()
            .map(|(id, document)| (id.clone(), document.theme.clone()))
            .collect();
        let mut alternate = package
            .aui_themes
            .get(engine_runtime::aui_theme::BUILTIN_AUI_THEME_ID)
            .cloned()
            .ok_or_else(|| "aui.performance_load.builtin_theme_missing".to_string())?;
        alternate.asset_id = GATE_E_ALTERNATE_THEME_ID.to_string();
        if let Some(style) = alternate.styles.values_mut().next() {
            style.paint.opacity =
                Some(engine_runtime::aui_theme::AuiThemeValue::Value { value: 0.875 });
        }
        let mut assets = package
            .aui_themes
            .assets()
            .filter(|asset| {
                asset.asset_id != engine_runtime::aui_theme::BUILTIN_AUI_THEME_ID
                    && asset.asset_id != GATE_E_ALTERNATE_THEME_ID
            })
            .cloned()
            .collect::<Vec<_>>();
        assets.push(alternate);
        package.aui_themes = engine_runtime::aui_theme::AuiThemeRegistry::from_assets(assets)
            .map_err(|diagnostics| format!("aui.performance_load.theme_invalid:{diagnostics:?}"))?;
        Ok(Self {
            original_document_themes,
        })
    }

    fn apply(&self, package: &mut RuntimePackage, frame_index: u64, native_scale: f64) -> f64 {
        let alternate = frame_index.is_multiple_of(2);
        for (document_id, document) in &mut package.aui_documents.documents_by_id {
            document.theme = if alternate {
                Some(engine_runtime::runtime_package::RuntimeAssetRef {
                    id: GATE_E_ALTERNATE_THEME_ID.to_string(),
                    asset_type: "aui-theme".to_string(),
                    guid: None,
                    sub_asset: None,
                })
            } else {
                self.original_document_themes
                    .get(document_id)
                    .cloned()
                    .flatten()
            };
        }
        if alternate {
            native_scale * 1.25
        } else {
            native_scale
        }
    }
}

pub(super) fn player_wgpu_backends(is_android: bool, is_x86_64: bool) -> wgpu::Backends {
    if is_android && is_x86_64 {
        wgpu::Backends::GL
    } else {
        wgpu::Backends::PRIMARY
    }
}

// One frozen game frame spans any number of loading presentations. Its
// runtime work, script input and immutable render packet are produced once.
struct PreparedWindowFrame {
    render_thread: engine_runtime::render_thread::RenderThreadFrameOutput,
    presentation: Arc<ResolvedGameViewPresentation>,
    frame_started: Instant,
    render_started: Instant,
    input_dispatch_ime_ns: u64,
    size: winit::dpi::PhysicalSize<u32>,
    loading_started: Option<Instant>,
}

struct CommittedWindowFrame {
    output: engine_runtime::engine_host_loop::EngineFrameOutput,
    aui_present: Option<engine_runtime::aui::AuiRuntimePresentOutput>,
    presentation: Arc<ResolvedGameViewPresentation>,
    frame_started: Instant,
    input_dispatch_ime_ns: u64,
    size: winit::dpi::PhysicalSize<u32>,
}

struct RealWindowHost {
    pending_geometry_frame: Option<CommittedWindowFrame>,
    pending_frame: Option<PreparedWindowFrame>,
    pending_startup_package: Option<RuntimePackage>,
    startup_started: Option<Instant>,
    startup_input: Vec<RawInputEvent>,
    geometry_texture_upload: crate::geometry_loading::GeometryTextureUpload,
    geometry_resource_wait_started: Option<Instant>,
    sampling_clock: crate::measurement::SamplingClock,
    native_present: Option<crate::native_present::NativePresentObserver>,
    previous_presentation_started: Option<Instant>,
    fixed_clock: PresentationFixedClock,
    fixed_actions: FixedActionLatch,
    semantic: Option<(semantic_outcome::PlaytestScenario, PathBuf)>,
    playtest: Option<semantic_playtest::SemanticPlaytest>,
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
    frame_aui_cpu_ns: Vec<u64>,
    frame_aui_work: Vec<AuiFrameWorkCounters>,
    frame_update_ns: Vec<u64>,
    frame_render_submit_ns: Vec<u64>,
    frame_present_wait_ns: Vec<u64>,
}

impl RealWindowHost {
    fn new(
        event_loop: &ActiveEventLoop,
        request: NativePlayerWindowRunRequest,
        linked_modules: Arc<LinkedProjectRuntimeSet>,
        semantic: Option<(semantic_outcome::PlaytestScenario, PathBuf)>,
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
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|error| format!("surface.request_adapter_failed:{error}"))?;
        let backend_name = format!("{:?}", adapter.get_info().backend);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("runtime-player-wgpu-device"),
            required_features: (adapter.features()
                & (wgpu::Features::TEXTURE_COMPRESSION_BC
                    | wgpu::Features::INDIRECT_FIRST_INSTANCE))
                | engine_runtime::gpu_frame_measurement::features(
                    adapter.features(),
                    request.measurement_enabled(),
                )
                | crate::native_present::features(
                    adapter.features(),
                    request.measurement_enabled(),
                ),
            required_limits: engine_runtime::particle_gpu::renderer_device_limits(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| format!("surface.request_device_failed:{error}"))?;
        let mut surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or_else(|| "surface.default_config_unavailable".to_string())?;
        let requested_mode = match request.config.present_mode.as_str() {
            "Fifo" => wgpu::PresentMode::Fifo,
            "Immediate" => wgpu::PresentMode::Immediate,
            "Mailbox" => wgpu::PresentMode::Mailbox,
            _ => return Err("surface.invalid_present_mode".into()),
        };
        if !surface
            .get_capabilities(&adapter)
            .present_modes
            .contains(&requested_mode)
        {
            return Err(format!(
                "surface.present_mode_unsupported:{}",
                request.config.present_mode
            ));
        }
        surface_config.present_mode = requested_mode;
        surface.configure(&device, &surface_config);
        let mut backend = engine_runtime::wgpu_backend::real::RealWgpuBackend::from_device_queue(
            device.clone(),
            queue,
            surface_config.format,
            size.width,
            size.height,
            backend_name.clone(),
        );
        if request.measurement_enabled() {
            backend.enable_frame_measurement(
                &adapter.get_info(),
                request.performance_warmup_frames,
                gpu_performance_sample_frames(request.performance_sample_frames),
            )?;
        }
        let mut report = NativeWindowHostReport::base(&request);
        report.diagnostics.push(NativeWindowHostDiagnostic {
            severity: NativeWindowHostDiagnosticSeverity::Info,
            code: "graphics.backend_selected".to_string(),
            layer: "surface".to_string(),
            message: backend_name,
            path: None,
        });
        Ok(Self {
            native_present: crate::native_present::NativePresentObserver::new(
                &device,
                if request.measurement_enabled() {
                    request.measurement_policy()
                } else {
                    Default::default()
                },
                request.frame_limit,
            ),
            pending_frame: None,
            pending_geometry_frame: None,
            pending_startup_package: None,
            startup_started: None,
            startup_input: Vec::new(),
            geometry_texture_upload: Default::default(),
            geometry_resource_wait_started: None,
            sampling_clock: crate::measurement::SamplingClock::default(),
            previous_presentation_started: None,
            fixed_clock: PresentationFixedClock::default(),
            fixed_actions: FixedActionLatch::default(),
            semantic,
            playtest: None,
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
            frame_cpu_ns: cpu_frame_history(&request),
            frame_aui_cpu_ns: cpu_frame_history(&request),
            frame_aui_work: cpu_frame_history(&request),
            frame_update_ns: cpu_frame_history(&request),
            frame_render_submit_ns: cpu_frame_history(&request),
            frame_present_wait_ns: cpu_frame_history(&request),
            request,
        })
    }

    fn resize_surface(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some(observer) = &mut self.native_present {
            observer.reconfigure();
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.report.surface.configured = true;
        self.report.surface.width = width;
        self.report.surface.height = height;
        self.report.window.width = width;
        self.report.window.height = height;
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
        let retrying = self.pending_startup_package.is_some();
        if !retrying {
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
        }
        let mut package = if let Some(package) = self.pending_startup_package.take() {
            package
        } else {
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
            package
        };
        if !retrying {
            #[cfg(target_os = "windows")]
            if let Some(budget) = windows_3d_geometry_upload_budget(&package) {
                if let Err(error) = self.backend.set_geometry_upload_budget(budget) {
                    self.report.rhi_status = "error".into();
                    self.report.present_status = NativeWindowPresentStatus::RhiFailed;
                    self.report
                        .diagnostics
                        .push(NativeWindowHostDiagnostic::error(
                            "resource_profile_rejected",
                            "gpu",
                            error,
                        ));
                    return false;
                }
            }
        }
        #[cfg(target_os = "windows")]
        if windows_3d_geometry_upload_budget(&package).is_some() {
            if let Err(error) = self.backend.bind_geometry_upload_presentation(
                self.report.surface.presented_frame_count.saturating_add(1),
            ) {
                self.geometry_loading_failed(error);
                return false;
            }
        }
        let started = *self.startup_started.get_or_insert_with(Instant::now);
        let preparation = self.backend.poll_gpu_completion().and_then(|stats| {
            if stats.in_flight >= 2 {
                Err("gpu.in_flight_backpressure".into())
            } else {
                // Sprites, AUI images and fonts share the same per-Present
                // upload owner. Retain the package and retry the whole
                // idempotent preparation when any of them exhausts it.
                prepare_real_gpu_resources(&mut self.backend, &package)
            }
        });
        let (sprite_texture_bindings, runtime_texture_bindings) = match preparation {
            Ok(bindings) => {
                if retrying {
                    let summary = self
                        .report
                        .geometry_loading
                        .get_or_insert_with(Default::default);
                    summary.wait_wall_ns = summary.wait_wall_ns.saturating_add(
                        started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                    );
                }
                self.startup_started = None;
                bindings
            }
            Err(error)
                if matches!(
                    error.as_str(),
                    "geometry.visual_upload_capacity" | "gpu.in_flight_backpressure"
                ) =>
            {
                if started.elapsed() > std::time::Duration::from_secs(120) {
                    self.geometry_loading_failed("geometry.upload_timeout".into());
                    return false;
                }
                let upload = self.backend.geometry_upload_snapshot();
                let summary = self
                    .report
                    .geometry_loading
                    .get_or_insert_with(Default::default);
                summary.uploaded_bytes = upload.uploaded_bytes;
                summary.max_frame_uploaded_bytes = summary
                    .max_frame_uploaded_bytes
                    .max(upload.frame_uploaded_bytes as u64);
                summary.peak_staging_bytes =
                    summary.peak_staging_bytes.max(upload.staging_bytes as u64);
                self.pending_startup_package = Some(package);
                if matches!(
                    self.present_geometry_loading(),
                    RealWindowFrameAdvance::Complete
                ) {
                    self.pending_startup_package = None;
                }
                return false;
            }
            Err(error) => {
                self.geometry_loading_failed(error);
                return false;
            }
        };
        self.report.aui.package_document_count = package.aui_manifest.documents.len();
        self.report.aui.loaded_document_count = package.aui_documents.len();
        let aui_performance_load = match self.request.aui_performance_load {
            Some(NativeAuiPerformanceLoad::ThemeDpi) => {
                match AuiThemeDpiLoadState::install(&mut package) {
                    Ok(state) => Some(state),
                    Err(error) => {
                        self.report
                            .diagnostics
                            .push(NativeWindowHostDiagnostic::error(
                                "aui_performance_load_prepare_failed",
                                "aui",
                                error,
                            ));
                        return false;
                    }
                }
            }
            Some(
                NativeAuiPerformanceLoad::VirtualListLifecycle
                | NativeAuiPerformanceLoad::Dpi100
                | NativeAuiPerformanceLoad::Dpi150
                | NativeAuiPerformanceLoad::Dpi200,
            ) => None,
            None => None,
        };
        if let Some((scenario, _)) = &self.semantic {
            if let Err(error) = semantic_playtest::SemanticPlaytest::validate(scenario, &package) {
                self.report.diagnostics.push(error);
                return false;
            }
        }
        let bound_runtime = match ProjectRuntimeBootstrap::bind(&package, &self.linked_modules) {
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
        configure_runtime_audio(&mut host, &self.request);
        if let Some((scenario, _)) = &self.semantic {
            match semantic_playtest::SemanticPlaytest::new(
                scenario.clone(),
                &package,
                &receipt.session_id,
                Instant::now(),
            ) {
                Ok(playtest) => self.playtest = Some(playtest),
                Err(error) => {
                    self.report.diagnostics.push(error);
                    return false;
                }
            }
        }
        self.input_mapping = Some(input_mapping);
        self.report.project_runtime_bind_receipt = Some(receipt);
        let Some((world, mut hydrator)) =
            hydrate_active_scene_for_player(&package, &mut self.report)
        else {
            self.report.scene_status = "error".to_string();
            self.report.world_status = "error".to_string();
            self.report.present_status = NativeWindowPresentStatus::SceneFailed;
            return false;
        };
        self.report.scene_status = "ok".to_string();
        self.report.world_status = "ok".to_string();
        if !project_storage::bind(
            &self.request,
            &package,
            &self.linked_modules,
            &mut hydrator,
            &mut self.report,
        ) {
            return false;
        }
        let mut world = world;
        host.dispatch_focus_changed(self.window.has_focus(), &mut world);
        self.session = Some(RealWindowSession {
            aui_present_cache: NativeAuiPresentCache::for_package(&package),
            aui_feedback_state: AuiControlFeedbackState::default(),
            package,
            host,
            ui_state_producer,
            world,
            hydrator,
            sprite_texture_bindings,
            runtime_texture_bindings,
            aui_performance_load,
        });
        true
    }

    fn present_next_frame(&mut self, raw_input: Vec<RawInputEvent>) -> RealWindowFrameAdvance {
        if !self.ensure_session() {
            return if self.pending_startup_package.is_some() {
                self.startup_input.extend(raw_input);
                RealWindowFrameAdvance::Continue
            } else {
                RealWindowFrameAdvance::Complete
            };
        }
        let mut held_input = std::mem::take(&mut self.startup_input);
        held_input.extend(raw_input);
        let raw_input = held_input;
        // Retries and loading clears use the same allowance until an actual
        // surface Present increments the physical counter. Cell pumps inside
        // host.tick share this bound even though they use game-frame IDs.
        if let Err(error) = self.backend.bind_geometry_upload_presentation(
            self.report.surface.presented_frame_count.saturating_add(1),
        ) {
            return self.geometry_loading_failed(error);
        }
        if let Some(pending) = self.pending_frame.take() {
            return self.present_prepared_frame(pending);
        }
        if let Some(pending) = self.pending_geometry_frame.take() {
            if let Err(error) = self
                .backend
                .pump_geometry_uploads(self.report.frames_completed.saturating_add(1))
            {
                if error == "gpu.in_flight_backpressure" {
                    self.startup_input = raw_input;
                    self.pending_geometry_frame = Some(pending);
                    return RealWindowFrameAdvance::Continue;
                }
                return self.geometry_loading_failed(error);
            }
            let session = self
                .session
                .as_mut()
                .expect("committed frame has a session");
            session.host.retry_pending_geometry_resources(
                session.hydrator.instance_loader_mut().asset_loader_mut(),
            );
            return self.prepare_committed_frame(pending);
        }
        if self
            .playtest
            .as_mut()
            .is_some_and(|p| p.should_stop(&mut self.report))
        {
            return RealWindowFrameAdvance::Complete;
        }
        let frame_index = self.report.frames_completed.saturating_add(1);
        if frame_index > self.request.frame_limit {
            return RealWindowFrameAdvance::Complete;
        }
        let attempt_started = Instant::now();
        // Keep dependency waits in the eventual frame's CPU/wall interval.
        let frame_started = self
            .geometry_resource_wait_started
            .unwrap_or(attempt_started);
        if self.request.cpu_sampling_enabled() {
            self.sampling_clock.begin(
                frame_index,
                self.request.performance_warmup_frames,
                self.request.performance_sample_frames,
                frame_started,
            );
        }
        let presentation_elapsed = self
            .previous_presentation_started
            .replace(attempt_started)
            .map_or(
                std::time::Duration::from_secs_f64(
                    1. / f64::from(if self.request.config.target_fps == 0 {
                        240
                    } else {
                        self.request.config.target_fps
                    }),
                ),
                |previous| {
                    attempt_started
                        .saturating_duration_since(previous)
                        .min(std::time::Duration::from_millis(250))
                },
            );
        let presentation_delta_us = presentation_elapsed.as_micros() as u64;
        if let Err(error) = self.backend.pump_geometry_uploads(frame_index) {
            if error == "gpu.in_flight_backpressure" {
                // No World tick or scripted input has run. Wait for real completion.
                self.startup_input = raw_input;
                return RealWindowFrameAdvance::Continue;
            }
            self.report.rhi_status = "error".into();
            self.report
                .diagnostics
                .push(NativeWindowHostDiagnostic::error(
                    "gpu.device_epoch_failed",
                    "gpu",
                    error,
                ));
            return RealWindowFrameAdvance::Complete;
        }
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return RealWindowFrameAdvance::Continue;
        }

        let session = self
            .session
            .as_mut()
            .expect("successful session initialization stores runtime state");
        let native_scale = self
            .request
            .aui_performance_load
            .and_then(NativeAuiPerformanceLoad::fixed_dpi_scale)
            .unwrap_or_else(|| self.window.scale_factor());
        let viewport_scale = session
            .aui_performance_load
            .as_ref()
            .map_or(native_scale, |load| {
                load.apply(&mut session.package, frame_index, native_scale)
            });
        if self.request.aui_performance_load == Some(NativeAuiPerformanceLoad::VirtualListLifecycle)
            && frame_index > self.request.performance_warmup_frames
            && frame_index.is_multiple_of(60)
        {
            let forward = (frame_index / 60).is_multiple_of(2);
            for scroll in self.aui_interaction_state.scroll_offsets.values_mut() {
                scroll.offset_y = if forward {
                    scroll.max_offset_y * 0.75
                } else {
                    0.0
                };
                scroll.last_delta_y = if forward { 1.0 } else { -1.0 };
            }
        }
        let mut input_events = if self.geometry_resource_wait_started.is_some() {
            // A retry is not another scripted presented frame. Real OS input
            // below remains live while the loader continues its normal work.
            Vec::new()
        } else if let Some(playtest) = &mut self.playtest {
            playtest
                .next_input_tick()
                .map_or_else(Vec::new, |tick| scripted_input_events(&self.request, tick))
        } else {
            scripted_input_events(&self.request, frame_index)
        };
        let canvas_references = session
            .package
            .aui_manifest
            .documents
            .first()
            .and_then(|entry| session.package.aui_documents.get(&entry.document_id))
            .map(|document| {
                let viewport =
                    native_window_viewport_input(size.width, size.height, viewport_scale);
                let resolved = AuiViewportResolver::resolve_document(document, &viewport)
                    .map(|resolution| resolution.document)
                    .unwrap_or_else(|_| document.clone());
                resolved
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
        let measuring = self.request.measurement_enabled();
        let cpu_sampling = self.request.cpu_sampling_enabled();
        session.aui_present_cache.set_measurement_enabled(measuring);
        session
            .aui_feedback_state
            .set_measurement_enabled(measuring);
        session.host.set_ui_measurement_enabled(measuring);
        session.host.begin_spine_measurement_frame(measuring);
        self.backend.set_ui_measurement_enabled(measuring);
        let aui_started = Instant::now();
        let mut aui_present = session.aui_present_cache.take_or_rebuild_stabilized(
            &session.package,
            &session.world,
            frame_index,
            session.ui_state_producer.as_mut(),
            &mut session.host,
            NativeAuiPresentContext::new(
                Some(presentation.as_ref()),
                Some(&native_window_viewport_input(
                    size.width,
                    size.height,
                    viewport_scale,
                )),
                &mut self.aui_interaction_state,
            )
            .with_feedback_cache(&mut session.aui_feedback_state),
        );
        let input_dispatch_started = measuring.then(Instant::now);
        let mut raw_input = raw_input;
        map_display_pointer_events_to_target(&mut raw_input, presentation.as_ref());
        if self.playtest.is_none()
            && self.request.input_script.as_ref().is_none_or(|script| {
                script.schema_version != NATIVE_PLAYER_INPUT_SCRIPT_V2_SCHEMA_VERSION
            })
        {
            input_events.extend(raw_input);
        }
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
        let ime_request =
            aui_present
                .as_ref()
                .map_or_else(NativeImePlatformRequest::default, |present| {
                    native_ime_platform_request(
                        &AuiInteractionSystem::ime_platform_request(
                            &present.resolved_document,
                            &present.layout,
                            &present.paragraph_layouts,
                            &self.aui_interaction_state,
                        ),
                        presentation.as_ref(),
                    )
                });
        self.window.set_ime_allowed(ime_request.enabled);
        if let Some(area) = ime_request.cursor_area {
            self.window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(area.x.round() as i32, area.y.round() as i32),
                winit::dpi::PhysicalSize::new(
                    area.width.ceil().max(1.0) as u32,
                    area.height.ceil().max(1.0) as u32,
                ),
            );
        }
        if let Some(playtest) = &mut self.playtest {
            playtest.record_injected_events(input_events.len());
        }
        let input_dispatch_ime_ns = input_dispatch_started.map_or(0, |start| {
            start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });
        aui_present = session.aui_present_cache.advance_animations_for_present(
            aui_present,
            &session.package,
            &session.world,
            session.ui_state_producer.as_mut(),
            &mut session.host,
            engine_runtime::aui_present_cache::AuiAnimationPresentContext {
                frame_index,
                delta_us: presentation_delta_us,
                interaction_result: aui_interaction.as_ref().map(|i| &i.result),
                presentation: NativeAuiPresentContext::new(
                    Some(presentation.as_ref()),
                    Some(&native_window_viewport_input(
                        size.width,
                        size.height,
                        viewport_scale,
                    )),
                    &mut self.aui_interaction_state,
                )
                .with_feedback_cache(&mut session.aui_feedback_state),
            },
        );
        if let (Some(present), Some(interaction)) = (aui_present.as_mut(), aui_interaction.as_ref())
        {
            session
                .aui_present_cache
                .apply_virtual_list_actions(present, &interaction.result.actions);
            AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
                present,
                &interaction.result,
                &mut session.aui_feedback_state,
                presentation_delta_us,
                &session.package.font_atlases,
                &session.package.font_bundles,
                &session.package.aui_themes,
                Some(presentation.as_ref()),
            );
            session.host.publish_ui_geometry(present);
        }
        record_cpu_frame_sample(cpu_sampling, &mut self.frame_aui_cpu_ns, || {
            aui_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });
        self.report.input = input_summary;
        // Scripted input does not prove that the OS window has focus.
        self.report.input.focused = self.window.has_focus();
        self.report.input_status = "ok".to_string();
        let mut frame_input = EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_action_snapshot(action_snapshot)
            .with_input_trace_summary(input_trace_summary)
            .with_deferred_render_submission();
        if uses_presentation_elapsed(
            self.request.config.target_fps,
            session.world.has_spatial_entities(),
            self.playtest.is_some(),
        ) {
            let elapsed = presentation_elapsed;
            let ticks = self.fixed_clock.advance(elapsed);
            let snapshot = frame_input.action_snapshot.take().expect("resolved input");
            frame_input = frame_input
                .with_action_snapshot(self.fixed_actions.resolve(snapshot, ticks > 0))
                .with_unscaled_delta_time(elapsed.as_secs_f32());
            if !session.world.has_spatial_entities() {
                frame_input = frame_input.with_fixed_step_count(ticks);
            }
        }
        if self.playtest.is_some() {
            frame_input = frame_input.with_fixed_step_count(1);
        }
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
                cell_resources: None,
                #[cfg(feature = "real-window")]
                cell_backend: Some(&mut self.backend),
                package: &session.package,
                instance_loader: session.hydrator.instance_loader_mut(),
            },
        );
        if output.host_control.is_some() {
            record_runtime_audio(
                &mut self.report,
                &session.host,
                self.request.runtime_report_level,
            );
            self.report
                .record_project_runtime_session(output.project_runtime_session_report.clone());
            self.report.session_exit_requested = true;
            record_runtime_trace(
                &mut self.report,
                &output.runtime_trace,
                self.request.runtime_report_level,
            );
            return RealWindowFrameAdvance::Complete;
        }
        self.report
            .record_project_runtime_session(output.project_runtime_session_report.clone());
        record_runtime_audio(
            &mut self.report,
            &session.host,
            self.request.runtime_report_level,
        );
        // Observation belongs to this committed game frame, not a later
        // resource retry or focus/control callback during the loading wait.
        if let Some(playtest) = &mut self.playtest {
            if !playtest.spatial_ids().is_empty() {
                let target = RenderTarget::surface(
                    self.request.config.target_id.clone(),
                    size.width,
                    size.height,
                )
                .with_presentation_scale_policy(self.request.game_view_target.scale_policy);
                playtest.set_spatial_sample(session.host.observe_spatial_for_target(
                    &session.world,
                    playtest.spatial_ids(),
                    playtest.spatial_level(),
                    &target,
                ));
            }
            playtest.sample(
                &output,
                &mut self.report,
                aui_present.as_ref(),
                Some(&self.aui_interaction_state),
            );
        }
        self.prepare_committed_frame(CommittedWindowFrame {
            output,
            aui_present,
            presentation,
            frame_started,
            input_dispatch_ime_ns,
            size,
        })
    }

    fn prepare_committed_frame(&mut self, pending: CommittedWindowFrame) -> RealWindowFrameAdvance {
        let CommittedWindowFrame {
            output,
            aui_present,
            presentation,
            frame_started,
            input_dispatch_ime_ns,
            size,
        } = pending;
        // A terminal tick has no committed render identity. Keep its original
        // session/trace failure instead of preparing a packet from older state.
        if output
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|session| session.terminal_fault)
        {
            record_runtime_trace(
                &mut self.report,
                &output.runtime_trace,
                self.request.runtime_report_level,
            );
            return RealWindowFrameAdvance::Complete;
        }
        let attempt_started = Instant::now();
        let cpu_sampling = self.request.cpu_sampling_enabled();
        let session = self
            .session
            .as_mut()
            .expect("committed frame has a session");
        let prepare_error = session
            .host
            .retained_geometry_snapshot()
            .and_then(|s| s.preparation_error());
        let was_waiting = self.geometry_resource_wait_started.is_some();
        match defer_unready_geometry(
            prepare_error,
            &mut self.geometry_resource_wait_started,
            attempt_started,
        ) {
            Ok(true) => {
                if !was_waiting {
                    self.report.diagnostics.push(NativeWindowHostDiagnostic {
                        severity: NativeWindowHostDiagnosticSeverity::Info,
                        code: "three_d.resource_pending".into(),
                        layer: "render".into(),
                        message:
                            "3D resource preparation pending; no GPU submission or present counted"
                                .into(),
                        path: None,
                    });
                }
                self.pending_geometry_frame = Some(CommittedWindowFrame {
                    output,
                    aui_present,
                    presentation,
                    frame_started,
                    input_dispatch_ime_ns,
                    size,
                });
                return RealWindowFrameAdvance::Continue;
            }
            Err(code) => {
                self.report.rhi_status = "error".into();
                self.report.present_status = NativeWindowPresentStatus::RhiFailed;
                self.report
                    .diagnostics
                    .push(NativeWindowHostDiagnostic::error(
                        code,
                        "render",
                        "3D resources did not become ready within 120 seconds",
                    ));
                return RealWindowFrameAdvance::Complete;
            }
            Ok(false) => {
                if was_waiting && prepare_error.is_none() {
                    self.report.diagnostics.push(NativeWindowHostDiagnostic {
                            severity: NativeWindowHostDiagnosticSeverity::Info,
                            code: "three_d.resource_ready".into(),
                            layer: "render".into(),
                            message: format!("3D resource wait completed after {:.3} ms; retained in frame CPU/wall interval", frame_started.elapsed().as_secs_f64()*1000.),
                            path: None,
                        });
                }
            }
        }
        record_cpu_frame_sample(cpu_sampling, &mut self.frame_update_ns, || {
            frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });
        let render_started = Instant::now();
        record_runtime_trace(
            &mut self.report,
            &output.runtime_trace,
            self.request.runtime_report_level,
        );
        if output.runtime_advanced {
            self.report.mark_runtime_advanced();
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
        if !session.host.set_external_rhi_enabled(true) {
            self.report.rhi_status = "error".into();
            self.report.present_status = NativeWindowPresentStatus::RhiFailed;
            self.report
                .diagnostics
                .push(NativeWindowHostDiagnostic::error(
                    "three_d.production_frame_failed",
                    "render",
                    "external RHI plan production requires the inline dispatcher",
                ));
            return RealWindowFrameAdvance::Complete;
        }
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
        if render_thread.report.rhi_status != "ok" {
            self.report.rhi_status = "error".into();
            self.report.present_status = NativeWindowPresentStatus::RhiFailed;
            self.report
                .diagnostics
                .push(NativeWindowHostDiagnostic::error(
                    "three_d.production_frame_failed",
                    "render",
                    format!("{:?}", render_thread.report.diagnostics),
                ));
            return RealWindowFrameAdvance::Complete;
        }
        session.aui_present_cache.store(aui_present);
        self.present_prepared_frame(PreparedWindowFrame {
            render_thread,
            presentation,
            frame_started,
            render_started,
            input_dispatch_ime_ns,
            size,
            loading_started: None,
        })
    }

    fn geometry_loading_failed(&mut self, error: String) -> RealWindowFrameAdvance {
        self.report.rhi_status = "error".into();
        self.report.present_status = NativeWindowPresentStatus::RhiFailed;
        self.report
            .diagnostics
            .push(NativeWindowHostDiagnostic::error(
                "geometry.loading_failed",
                "render",
                error,
            ));
        RealWindowFrameAdvance::Complete
    }

    fn present_prepared_frame(
        &mut self,
        mut pending: PreparedWindowFrame,
    ) -> RealWindowFrameAdvance {
        if pending
            .loading_started
            .is_some_and(|start| start.elapsed() >= std::time::Duration::from_secs(120))
        {
            return self.geometry_loading_failed("geometry.upload_timeout".into());
        }
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            self.pending_frame = Some(pending);
            return RealWindowFrameAdvance::Continue;
        }
        // The OS can expose the new inner size before its queued Resized event
        // reaches us. Configure the surface before consuming a plan at that size.
        if self.surface_config.width != size.width || self.surface_config.height != size.height {
            self.resize_surface(size.width, size.height);
        }
        if size != pending.size {
            // Rebuild only the presentation consumer from the same committed
            // world/snapshot. No tick, input injection or playtest sample repeats.
            let session = self.session.as_mut().expect("prepared frame has a session");
            let present = session.aui_present_cache.current();
            let canvas_references = present.map_or_else(Vec::new, |present| {
                present
                    .resolved_document
                    .canvases
                    .iter()
                    .map(|canvas| {
                        CanvasReferenceFact::new(
                            canvas.canvas_id.clone(),
                            canvas.reference_resolution.x.round().max(1.) as u32,
                            canvas.reference_resolution.y.round().max(1.) as u32,
                        )
                    })
                    .collect()
            });
            let presentation = match GameViewPresentationModule::resolve(GameViewPresentationSpec {
                session_id: "native-player-window".into(),
                target_id: self.request.config.target_id.clone(),
                target_extent: pending.presentation.target_extent,
                display_rect: GameViewRect::from_extent(GameViewExtent::new(
                    size.width,
                    size.height,
                )),
                scale_policy: self.request.game_view_target.scale_policy,
                surface_generation: 1,
                presentation_revision: pending
                    .render_thread
                    .renderer_output
                    .rhi_command_plan
                    .frame_index,
                canvas_references,
            }) {
                Ok(presentation) => Arc::new(presentation),
                Err(error) => return self.geometry_loading_failed(error.code.into()),
            };
            let rebuilt = session
                .host
                .render_thread_for_target_with_runtime_resources_and_presentation(
                    RenderTarget::surface(
                        self.request.config.target_id.clone(),
                        size.width,
                        size.height,
                    )
                    .with_presentation_scale_policy(self.request.game_view_target.scale_policy),
                    present.map(|p| &p.overlay),
                    present.map(|p| &p.composition),
                    Some(&session.sprite_texture_bindings),
                    Some(&session.runtime_texture_bindings),
                    Some(Arc::clone(&presentation)),
                );
            if rebuilt.report.rhi_status != "ok"
                || rebuilt.renderer_output.rhi_command_plan.frame_index
                    != pending
                        .render_thread
                        .renderer_output
                        .rhi_command_plan
                        .frame_index
            {
                return self.geometry_loading_failed("geometry.loading_rebuild_failed".into());
            }
            pending.render_thread = rebuilt;
            pending.presentation = presentation;
            pending.size = size;
        }
        let frame_index = self.report.frames_completed.saturating_add(1);
        let mut ready = match self.geometry_texture_upload.prepare(
            &mut self.backend,
            &pending.render_thread.geometry_textures,
            frame_index,
        ) {
            Ok(ready) => ready,
            Err(error) => return self.geometry_loading_failed(error),
        };
        if ready {
            ready = match self.backend.prepare_surface_resources(
                &pending.render_thread.renderer_output.rhi_command_plan,
                [size.width, size.height],
                Some(pending.presentation.display_content_rect),
            ) {
                Ok(ready) => ready,
                Err(error) => return self.geometry_loading_failed(error),
            };
        }
        let upload = self.backend.geometry_upload_snapshot();
        if !ready || pending.loading_started.is_some() {
            let summary = self
                .report
                .geometry_loading
                .get_or_insert_with(Default::default);
            summary.uploaded_bytes = upload.uploaded_bytes;
            summary.max_frame_uploaded_bytes = summary
                .max_frame_uploaded_bytes
                .max(upload.frame_uploaded_bytes as u64);
            summary.peak_staging_bytes =
                summary.peak_staging_bytes.max(upload.staging_bytes as u64);
            summary.peak_in_flight = summary
                .peak_in_flight
                .max(self.backend.gpu_lifecycle_stats().in_flight);
        }
        if !ready {
            pending.loading_started.get_or_insert_with(Instant::now);
            let outcome = self.present_geometry_loading();
            self.pending_frame = Some(pending);
            return outcome;
        }
        if let Some(started) = pending.loading_started {
            let summary = self.report.geometry_loading.as_mut().unwrap();
            summary.wait_wall_ns = summary
                .wait_wall_ns
                .saturating_add(started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        }
        let PreparedWindowFrame {
            render_thread,
            presentation,
            frame_started,
            render_started,
            input_dispatch_ime_ns,
            size,
            ..
        } = pending;
        let cpu_sampling = self.request.cpu_sampling_enabled();
        let measuring = self.request.measurement_enabled();
        let session = self.session.as_mut().expect("prepared frame has a session");
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
                Some(presentation.display_content_rect),
            );
        }
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
                self.frame_aui_cpu_ns.pop();
                if let Some(playtest) = &mut self.playtest {
                    playtest.presentation_failed(&mut self.report);
                    return RealWindowFrameAdvance::Complete;
                }
                let size = self.window.inner_size();
                self.resize_surface(size.width, size.height);
                return RealWindowFrameAdvance::Continue;
            }
            Err(wgpu::SurfaceError::Timeout) => {
                self.frame_aui_cpu_ns.pop();
                if let Some(playtest) = &mut self.playtest {
                    playtest.presentation_failed(&mut self.report);
                    return RealWindowFrameAdvance::Complete;
                }
                self.report.surface.last_error = Some("surface.acquire_timeout".to_string());
                return RealWindowFrameAdvance::Continue;
            }
            Err(error @ (wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other)) => {
                self.frame_aui_cpu_ns.pop();
                self.report.surface.last_error = Some(format!("surface.acquire_failed:{error}"));
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
        // Target-space vertices must use the same content rect as pointer mapping and capture.
        let previous_submission = self.backend.last_gpu_submission();
        let backend_report = self.backend.execute_plan_to_surface_view_in_rect(
            &render_thread.renderer_output.rhi_command_plan,
            &view,
            presentation.display_content_rect,
        );
        let present_submission = match crate::native_present::submission_for_present(
            previous_submission,
            self.backend.last_gpu_submission(),
            render_thread.renderer_output.rhi_command_plan.frame_index,
            &backend_report,
        ) {
            Ok(token) => Some(token),
            Err(error) => {
                self.report.rhi_status = "error".into();
                self.report.present_status = NativeWindowPresentStatus::RhiFailed;
                self.report
                    .diagnostics
                    .push(NativeWindowHostDiagnostic::error(
                        "gpu.frame_rejected",
                        "gpu",
                        error,
                    ));
                return RealWindowFrameAdvance::Complete;
            }
        };
        let submit_ns = submit_started
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        let present_started = Instant::now();
        if let Some((observer, token)) = self.native_present.as_mut().zip(present_submission) {
            observer.before_present(
                &self.surface,
                &surface_texture.texture,
                token,
                render_thread
                    .renderer_output
                    .rhi_command_plan
                    .frame_identity
                    .clone(),
            );
        }
        surface_texture.present();
        if let Some(observer) = &mut self.native_present {
            observer.after_present();
        }
        if let Some(token) = present_submission {
            self.backend.record_surface_present(token);
        }
        let present_ns = present_started
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        self.report.surface.presented_frame_count += 1;
        if let Some(identity) = &render_thread
            .renderer_output
            .rhi_command_plan
            .frame_identity
        {
            if self.report.run_id != identity.source.run_id {
                self.report.run_id.clone_from(&identity.source.run_id);
            }
        }
        if self.report.surface.presented_frame_count.is_multiple_of(16) {
            if let Some(observer) = &mut self.native_present {
                observer.collect();
            }
        }
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
        // The history is unchanged until this closure returns and pushes its result.
        // Capture its backing before record_cpu_frame_sample borrows the Vec mutably.
        let frame_work_backing = (
            self.frame_aui_work.as_ptr() as usize,
            self.frame_aui_work.capacity() * std::mem::size_of::<AuiFrameWorkCounters>(),
        );
        record_cpu_frame_sample(cpu_sampling, &mut self.frame_aui_work, || {
            let mut aui_work = if measuring {
                capture_aui_work_with_input(
                    &session.aui_present_cache,
                    &session.host,
                    Some(aui_gpu_upload_counters(
                        self.backend.ui_vertex_upload_stats(),
                        self.backend.retained_ui_stats(),
                    )),
                    &session.aui_feedback_state,
                    frame_index,
                    input_dispatch_ime_ns,
                )
            } else {
                AuiFrameWorkCounters {
                    frame_index,
                    ..Default::default()
                }
            };
            crate::measurement::capture_render_identity(
                &mut aui_work,
                self.request.measurement_policy(),
                render_thread
                    .renderer_output
                    .rhi_command_plan
                    .frame_identity
                    .as_ref(),
            );
            // Summary needs bounded resource/memory aggregates to verify a
            // steady sampling window; raw export remains Trace-filtered.
            if cpu_sampling {
                let census = self
                    .request
                    .measurement_policy()
                    .allocation_frame(frame_index);
                let memory_summary = self.request.measurement_policy().allocation_summary;
                let memory_capture_started = (census || memory_summary)
                    .then(|| self.sampling_clock.offset_ns(Instant::now()))
                    .flatten();
                aui_work.process_memory =
                    engine_runtime::windowed_player::sample_player_process_memory();
                let loader = session.hydrator.instance_loader();
                if self.request.measurement_policy().trace_frame(frame_index) {
                    aui_work.cell_receipts = loader
                        .project_cell_snapshot(&session.world)
                        .receipts
                        .into_values()
                        .collect();
                }
                if census || memory_summary {
                    if let Some(started) = memory_capture_started {
                        let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(
                            frame_index,
                            self.backend.gpu_lifecycle_stats().device_epoch,
                            started,
                        );
                        sample.frame_identity = aui_work.frame_identity.clone();
                        session.world.observe_memory(&mut sample);
                        session.host.observe_project_memory(&mut sample);
                        if let Some(snapshot) = session.host.retained_geometry_snapshot() {
                            snapshot.observe_memory(&mut sample);
                        }
                        loader.observe_memory(&mut sample);
                        self.backend.observe_memory(&mut sample);
                        render_thread.observe_report_memory(&mut sample);
                        backend_report.observe_report_memory(&mut sample);
                        self.report.observe_memory(&mut sample);
                        crate::memory_observation::observe_measurement_history(
                            &mut sample,
                            [
                                &self.frame_cpu_ns,
                                &self.frame_aui_cpu_ns,
                                &self.frame_update_ns,
                                &self.frame_render_submit_ns,
                                &self.frame_present_wait_ns,
                            ],
                            frame_work_backing,
                        );
                        sample.gpu_memory_budget = crate::gpu_memory::sample(&self.device);
                        sample.gpu_available_bytes = sample.gpu_memory_budget.as_ref().and_then(
                            engine_runtime::resource_memory::GpuMemoryBudgetSample::available_bytes,
                        );
                        sample.process_private_bytes =
                            aui_work.process_memory.map(|m| m.private_commit_bytes);
                        sample.system_available_bytes =
                            engine_runtime::resource_memory::system_available_bytes();
                        sample.capture_finished_ns = self
                            .sampling_clock
                            .offset_ns(Instant::now())
                            .unwrap_or(started);
                        if memory_summary {
                            sample.capacities.insert(
                                "memory.censusRecords".into(),
                                sample.allocations.len() as u64,
                            );
                            let reduction_started = Instant::now();
                            let summary = self
                                .report
                                .resource_memory_summary
                                .get_or_insert_with(Default::default);
                            summary.observe(&sample);
                            let elapsed = reduction_started
                                .elapsed()
                                .as_nanos()
                                .min(u128::from(u64::MAX))
                                as u64;
                            let peak = summary
                                .capacity_peaks
                                .entry("memory.reductionCpuNs".into())
                                .or_default();
                            *peak = (*peak).max(elapsed);
                        }
                        if census {
                            aui_work.resource_memory = Some(sample);
                        }
                    }
                }
                let cpu = loader.asset_loader().streaming_snapshot();
                let (active_cells, live_cells) = loader.cell_lifetime_counts();
                let residency = self.backend.geometry_residency();
                let upload = self.backend.geometry_upload_snapshot();
                let geometry = self.backend.retained_geometry_stats();
                aui_work.spatial_gpu = Some(
                    engine_runtime::windowed_player::WindowedPlayerSpatialGpuSample {
                        cumulative_draws: geometry.draws,
                        cumulative_instances: geometry.instances,
                        frame_uploaded_bytes: upload.frame_uploaded_bytes as u64,
                        cumulative_parameter_uploaded_bytes: geometry.parameter_uploaded_bytes
                            + geometry.index_uploaded_bytes
                            + geometry.clear_uploaded_bytes,
                        requested_gpu_backing_bytes: measuring
                            .then(|| self.backend.requested_gpu_backing_bytes_for_measurement()),
                    },
                );
                let lifecycle = self.backend.gpu_lifecycle_stats();
                aui_work.sampling_wall_ns = self.sampling_clock.offset_ns(Instant::now());
                aui_work.streaming_resources = Some(
                    engine_runtime::windowed_player::WindowedPlayerStreamingResourceSample {
                        active_cells,
                        live_cells,
                        cpu_resident_bytes: cpu.cpu_resident_bytes as u64,
                        cpu_reserved_bytes: cpu.reserved_bytes as u64,
                        pending_asset_operations: cpu.pending_operations,
                        pending_asset_tickets: cpu.pending_tickets,
                        gpu_mesh_bytes: residency.mesh_bytes,
                        gpu_mesh_physical_bytes: residency.physical_mesh_bytes,
                        gpu_texture_bytes: residency.texture_bytes,
                        gpu_staging_bytes: upload.staging_bytes as u64,
                        gpu_retiring_bytes: lifecycle.retiring_bytes,
                        gpu_uploaded_bytes: upload.uploaded_bytes,
                        pending_gpu_tickets: upload.tickets,
                        gpu_meshes: residency.meshes,
                        gpu_textures: residency.textures,
                    },
                );
            }
            if session.aui_performance_load.is_some() {
                aui_work.theme_change_events = 1;
                aui_work.dpi_change_events = 1;
            }
            aui_work
        });
        self.report.render_thread_report_schema = Some(render_thread.report.schema_version);
        if let Some(playtest) = &mut self.playtest {
            if self.report.rhi_status != "ok" {
                playtest.presentation_failed(&mut self.report);
            }
            if self.report.rhi_status == "ok" {
                playtest.presented(frame_index, &render_thread.renderer_output.rhi_command_plan);
            }
            if let Some(capture_id) = playtest.capture_at(frame_index) {
                let directory = &self.semantic.as_ref().unwrap().1;
                let path = directory.join(format!("capture-{frame_index}.png"));
                let capture_request = self.request.clone().with_screenshot(path);
                if u64::from(size.width) * u64::from(size.height) > 4 * 1024 * 1024 {
                    self.report
                        .screenshot
                        .mark_failed(NativeWindowScreenshotStatus::ReadbackFailed);
                } else {
                    self.report.screenshot = NativeWindowScreenshotReport::from_request(
                        &capture_request.screenshot,
                        &capture_request.config,
                    );
                    capture_screenshot(
                        &mut self.backend,
                        &render_thread.renderer_output.rhi_command_plan,
                        &capture_request,
                        &mut self.report,
                        size.width,
                        size.height,
                        Some(presentation.display_content_rect),
                    );
                }
                playtest.record_capture(capture_id, frame_index, &self.report);
            }
        }
        record_cpu_frame_sample(cpu_sampling, &mut self.frame_render_submit_ns, || {
            render_prepare_ns.saturating_add(submit_ns)
        });
        record_cpu_frame_sample(cpu_sampling, &mut self.frame_present_wait_ns, || {
            acquire_wait_ns.saturating_add(present_ns)
        });
        record_cpu_frame_sample(cpu_sampling, &mut self.frame_cpu_ns, || {
            frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        });

        if cpu_sampling {
            self.sampling_clock.end(
                frame_index,
                self.request.performance_warmup_frames,
                self.request.performance_sample_frames,
                Instant::now(),
            );
        }

        if windowed_session_has_more_frames(self.report.frames_completed, self.request.frame_limit)
        {
            RealWindowFrameAdvance::Continue
        } else {
            RealWindowFrameAdvance::Complete
        }
    }

    fn present_geometry_loading(&mut self) -> RealWindowFrameAdvance {
        use engine_runtime::rhi_command_plan::{RhiCommand, RhiCommandPlan};
        // Loading uploads and clear submissions share the original completion
        // owner. Backpressure is a retry, never a fake presentation or ACK.
        match self.backend.poll_gpu_completion() {
            Ok(stats) if stats.in_flight >= 2 => return RealWindowFrameAdvance::Continue,
            Ok(_) => {}
            Err(error) => return self.geometry_loading_failed(error),
        }
        let image = match self.surface.get_current_texture() {
            Ok(image) => image,
            Err(wgpu::SurfaceError::Timeout) => return RealWindowFrameAdvance::Continue,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                let size = self.window.inner_size();
                self.resize_surface(size.width, size.height);
                return RealWindowFrameAdvance::Continue;
            }
            Err(error) => {
                return self.geometry_loading_failed(format!("surface.acquire_failed:{error}"))
            }
        };
        self.report.surface.acquired_frame_count += 1;
        let target = self.request.config.target_id.clone();
        // Frame zero deliberately has no game identity or sample. The real
        // queue token still proves this distinct loading clear before Present.
        let plan = RhiCommandPlan {
            frame_identity: None,
            graph_schedule: None,
            packet_content_version: None,
            frame_index: 0,
            graph_id: "native-geometry-loading".into(),
            target_kind: "surface".into(),
            commands: vec![
                RhiCommand::BeginFrame {
                    target: target.clone(),
                },
                RhiCommand::Clear {
                    target: target.clone(),
                    color: engine_runtime::render_graph::color([0.025, 0.025, 0.035, 1.]),
                },
                RhiCommand::Submit,
                RhiCommand::Present { target },
            ]
            .into(),
            diagnostics: vec![],
        };
        let view = image.texture.create_view(&Default::default());
        let previous = self.backend.last_gpu_submission();
        let report = match self
            .backend
            .execute_loading_clear_to_surface_view(&plan, &view)
        {
            Ok(report) => report,
            Err(error) => return self.geometry_loading_failed(error),
        };
        let token = match crate::native_present::submission_for_present(
            previous,
            self.backend.last_gpu_submission(),
            0,
            &report,
        ) {
            Ok(token) => token,
            Err(error) => return self.geometry_loading_failed(error),
        };
        image.present();
        self.backend.record_surface_present(token);
        self.report.surface.presented_frame_count += 1;
        self.report
            .geometry_loading
            .get_or_insert_with(Default::default)
            .presented_frames += 1;
        let in_flight = self.backend.gpu_lifecycle_stats().in_flight;
        let summary = self.report.geometry_loading.as_mut().unwrap();
        summary.peak_in_flight = summary.peak_in_flight.max(in_flight);
        RealWindowFrameAdvance::Continue
    }

    fn finish(&mut self, close_requested: bool) -> NativeWindowHostReport {
        if let Some(started) = self.startup_started.take() {
            let summary = self
                .report
                .geometry_loading
                .get_or_insert_with(Default::default);
            summary.wait_wall_ns = summary
                .wait_wall_ns
                .saturating_add(started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        }
        if let Some(started) = self.pending_frame.as_ref().and_then(|p| p.loading_started) {
            let summary = self
                .report
                .geometry_loading
                .get_or_insert_with(Default::default);
            summary.wait_wall_ns = summary
                .wait_wall_ns
                .saturating_add(started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        }
        self.report.window.close_requested = close_requested;
        if self.request.measurement_policy().level == crate::measurement::MeasurementLevel::Trace {
            self.report.final_project_observation = self
                .session
                .as_ref()
                .and_then(|s| s.host.project_observation_state())
                .cloned();
        }
        finalize_frame_performance(
            &mut self.report,
            &self.request,
            FramePerformanceSamples {
                cpu_ns: &self.frame_cpu_ns,
                aui_cpu_ns: &self.frame_aui_cpu_ns,
                aui_work: &self.frame_aui_work,
                update_ns: &self.frame_update_ns,
                render_submit_ns: &self.frame_render_submit_ns,
                present_wait_ns: &self.frame_present_wait_ns,
            },
        );
        if let Some(summary) = &mut self.report.frame_performance_summary {
            summary.sampling_window = self.sampling_clock.report(
                self.request.performance_warmup_frames,
                self.request.performance_sample_frames,
            );
            if let Some(window) = &mut summary.sampling_window {
                let (available, growth) = crate::measurement::memory_window(
                    &self.frame_aui_work,
                    self.request.frame_limit,
                );
                window.minimum_system_available_bytes = available;
                window.tail1200_private_bytes_per_frame = growth;
                window.requested_peak_private_commit_bytes =
                    crate::measurement::requested_memory_peak(
                        &self.frame_aui_work,
                        self.request.frame_limit,
                    );
                let sample_start = (self.request.performance_warmup_frames as usize)
                    .min(self.frame_aui_work.len());
                let sample_end = sample_start
                    .saturating_add(self.request.performance_sample_frames as usize)
                    .min(self.frame_aui_work.len());
                window.private_commit_percentile_bytes = crate::measurement::memory_percentiles(
                    &self.frame_aui_work[sample_start..sample_end],
                );
                let first = self
                    .frame_aui_work
                    .get(self.request.performance_warmup_frames as usize);
                let last = self
                    .frame_aui_work
                    .get(window.last_frame.saturating_sub(1) as usize);
                window.streaming_resources_start = first.and_then(|w| w.streaming_resources);
                window.streaming_resources_end = last.and_then(|w| w.streaming_resources);
                let sample = self
                    .frame_aui_work
                    .iter()
                    .skip(self.request.performance_warmup_frames as usize)
                    .take(self.request.performance_sample_frames as usize);
                window.streaming_resources_unchanged =
                    window.streaming_resources_start.map(|baseline| {
                        sample
                            .clone()
                            .all(|w| w.streaming_resources == Some(baseline))
                    });
                window.peak_private_commit_bytes = sample
                    .clone()
                    .filter_map(|w| w.process_memory.map(|m| m.private_commit_bytes))
                    .max();
                window.peak_working_set_bytes = sample
                    .filter_map(|w| w.process_memory.map(|m| m.working_set_bytes))
                    .max();
                window.first_tick = first.and_then(|w| w.tick_pair).map(|ticks| ticks[0]);
                window.last_tick = last.and_then(|w| w.tick_pair).map(|ticks| ticks[1]);
            }
        }
        if let Err(error) = self.backend.shutdown_gpu(std::time::Duration::from_secs(2)) {
            self.report.rhi_status = "error".into();
            self.report
                .diagnostics
                .push(NativeWindowHostDiagnostic::error(
                    "gpu.shutdown_failed",
                    "gpu",
                    error,
                ));
        }
        if let Err(error) = self.geometry_texture_upload.release(&mut self.backend) {
            self.report.rhi_status = "error".into();
            self.report
                .diagnostics
                .push(NativeWindowHostDiagnostic::error(
                    "geometry.loading_release_failed",
                    "render",
                    error,
                ));
        }
        let mut gpu = self.backend.finish_frame_measurement();
        if let Some(mut observer) = self.native_present.take() {
            if self.backend.gpu_lifecycle_stats().abandoned {
                observer.abandon_device();
            }
            if let Some(gpu) = &mut gpu {
                gpu.native_present = Some(observer.finish());
            }
        }
        if let Some(gpu) = &mut gpu {
            self.request.measurement_policy().filter_gpu(gpu);
        }
        if let Some(summary) = &mut self.report.frame_performance_summary {
            summary.gpu = gpu;
        }
        if !self.report.session_exit_requested
            && self.report.present_status == NativeWindowPresentStatus::NotPresented
        {
            self.report.present_status =
                if self.report.frames_completed > 0 && self.report.rhi_status == "ok" {
                    NativeWindowPresentStatus::Presented
                } else {
                    NativeWindowPresentStatus::RhiFailed
                };
        }
        self.report.exit_code = self.report.completion_exit_code();
        if let Some(mut playtest) = self.playtest.take() {
            if close_requested {
                playtest.presentation_failed(&mut self.report);
            }
            playtest.should_stop(&mut self.report);
            playtest.finish(&mut self.report);
        }
        self.report.clone()
    }

    #[allow(dead_code)]
    fn present_limited_frames(&mut self, raw_input: Vec<RawInputEvent>) -> NativeWindowHostReport {
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
        let bound_runtime = match ProjectRuntimeBootstrap::bind(&package, &self.linked_modules) {
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
        configure_runtime_audio(&mut host, &self.request);
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
        if !project_storage::bind(
            &self.request,
            &package,
            &self.linked_modules,
            &mut hydrator,
            &mut report,
        ) {
            return report;
        }
        let mut frame_cpu_ns = cpu_frame_history(&self.request);
        let mut frame_aui_cpu_ns = cpu_frame_history(&self.request);
        let mut frame_aui_work = cpu_frame_history(&self.request);
        let mut frame_update_ns = cpu_frame_history(&self.request);
        let mut frame_render_submit_ns = cpu_frame_history(&self.request);
        let mut frame_present_wait_ns = cpu_frame_history(&self.request);
        let mut aui_present_cache = NativeAuiPresentCache::for_package(&package);
        let mut aui_feedback_state = AuiControlFeedbackState::default();
        for frame_index in 0..self.request.frame_limit {
            let frame_started = Instant::now();
            if let Err(error) = self.backend.pump_geometry_uploads(frame_index) {
                report.rhi_status = "error".into();
                report.diagnostics.push(NativeWindowHostDiagnostic::error(
                    "cell.gpu_advance_failed",
                    "gpu",
                    error,
                ));
                report.present_status = NativeWindowPresentStatus::RhiFailed;
                return report;
            }
            let mut input_events = scripted_input_events(&self.request, frame_index + 1);
            if frame_index == 0
                && self.request.input_script.as_ref().is_none_or(|script| {
                    script.schema_version != NATIVE_PLAYER_INPUT_SCRIPT_V2_SCHEMA_VERSION
                })
            {
                input_events.extend(raw_input.iter().cloned());
            }
            let measuring = self.request.measurement_enabled();
            let cpu_sampling = self.request.cpu_sampling_enabled();
            aui_present_cache.set_measurement_enabled(measuring);
            aui_feedback_state.set_measurement_enabled(measuring);
            host.set_ui_measurement_enabled(measuring);
            host.begin_spine_measurement_frame(measuring);
            self.backend.set_ui_measurement_enabled(measuring);
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
                        self.request.game_view_target.extent.width,
                        self.request.game_view_target.extent.height,
                        1.0,
                    )),
                    &mut self.aui_interaction_state,
                )
                .with_feedback_cache(&mut aui_feedback_state),
            );
            let input_dispatch_started = measuring.then(Instant::now);
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
            let input_dispatch_ime_ns = input_dispatch_started.map_or(0, |start| {
                start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
            });
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
                            self.request.game_view_target.extent.width,
                            self.request.game_view_target.extent.height,
                            1.0,
                        )),
                        &mut self.aui_interaction_state,
                    )
                    .with_feedback_cache(&mut aui_feedback_state),
                },
            );
            if let (Some(present), Some(interaction)) =
                (aui_present.as_mut(), aui_interaction.as_ref())
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
                .with_deferred_render_submission();
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
                    cell_resources: None,
                    #[cfg(feature = "real-window")]
                    cell_backend: Some(&mut self.backend),
                    package: &package,
                    instance_loader: hydrator.instance_loader_mut(),
                },
            );
            report.record_project_runtime_session(output.project_runtime_session_report.clone());
            record_runtime_audio(&mut report, &host, self.request.runtime_report_level);
            record_cpu_frame_sample(cpu_sampling, &mut frame_update_ns, || {
                frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
            });
            let render_started = Instant::now();
            record_runtime_trace(
                &mut report,
                &output.runtime_trace,
                self.request.runtime_report_level,
            );
            if output.runtime_advanced {
                report.mark_runtime_advanced();
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
            let prepared = if render_thread.report.rhi_status != "ok" {
                Err(format!("{:?}", render_thread.report.diagnostics))
            } else {
                self.backend
                    .prepare_geometry_textures(&render_thread.geometry_textures)
            };
            if let Err(error) = prepared {
                report.rhi_status = "error".into();
                report.present_status = NativeWindowPresentStatus::RhiFailed;
                report.diagnostics.push(NativeWindowHostDiagnostic::error(
                    "geometry_frame_failed",
                    "render",
                    error,
                ));
                return report;
            }
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
                    None,
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
            if let Some(token) = self.backend.last_gpu_submission() {
                self.backend.record_surface_present(token);
            }
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
                    Some(aui_gpu_upload_counters(
                        self.backend.ui_vertex_upload_stats(),
                        self.backend.retained_ui_stats(),
                    )),
                    &aui_feedback_state,
                    frame_index + 1,
                    input_dispatch_ime_ns,
                )
            });
            record_cpu_frame_sample(cpu_sampling, &mut frame_render_submit_ns, || {
                render_prepare_ns.saturating_add(submit_ns)
            });
            record_cpu_frame_sample(cpu_sampling, &mut frame_present_wait_ns, || {
                acquire_wait_ns.saturating_add(present_ns)
            });
            record_cpu_frame_sample(cpu_sampling, &mut frame_cpu_ns, || {
                frame_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
            });
        }
        finalize_frame_performance(
            &mut report,
            &self.request,
            FramePerformanceSamples {
                cpu_ns: &frame_cpu_ns,
                aui_cpu_ns: &frame_aui_cpu_ns,
                aui_work: &frame_aui_work,
                update_ns: &frame_update_ns,
                render_submit_ns: &frame_render_submit_ns,
                present_wait_ns: &frame_present_wait_ns,
            },
        );
        report.present_status = if report.rhi_status == "ok" {
            NativeWindowPresentStatus::Presented
        } else {
            NativeWindowPresentStatus::RhiFailed
        };
        report.exit_code = report.completion_exit_code();
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
        let _ =
            window.request_inner_size(winit::dpi::PhysicalSize::new(fitted_width, fitted_height));
    }

    let fitted_outer = window.outer_size();
    let x = monitor_info.rcWork.left + (work_width.saturating_sub(fitted_outer.width) / 2) as i32;
    let y = monitor_info.rcWork.top + (work_height.saturating_sub(fitted_outer.height) / 2) as i32;
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
        .map(|image| image.asset_id.clone())
        .chain(package.aui_themes.texture_asset_ids());
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
        backend.register_cooked_texture(upload.handle, &upload.payload)?;
    }
    let runtime_bindings = registry.binding_context();
    let mut bindings = Sprite2DTextureBindingContext::new();
    for asset_id in sprite_refs {
        let binding = runtime_bindings
            .get(&asset_id)
            .ok_or_else(|| format!("runtime_texture.binding_missing:{asset_id}"))?;
        bindings.insert_texture_handle(asset_id, binding.handle, binding.sampler.clone());
    }
    prepare_real_gpu_fonts(backend, package)?;
    Ok((bindings, runtime_bindings))
}

fn prepare_real_gpu_fonts(
    backend: &mut engine_runtime::wgpu_backend::real::RealWgpuBackend,
    package: &RuntimePackage,
) -> Result<(), String> {
    if !package.font_bundles.bundles_by_id.is_empty() {
        for bundle in package.font_bundles.bundles_by_id.values() {
            backend.register_font_texture_arrays(bundle)?;
        }
    } else if let Some(atlas) = package.font_atlases.default_atlas() {
        backend.register_alpha8_texture(
            font_atlas_render_handle(&atlas.metadata.font_atlas_id),
            atlas.metadata.atlas_width,
            atlas.metadata.atlas_height,
            &atlas.atlas_alpha,
            "nearestClamp",
        )?;
    }
    Ok(())
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

#[cfg(test)]
#[test]
fn contain_screenshot_keeps_square_pixels_and_matches_pointer_mapping() {
    use engine_runtime::render_graph::{
        color, RenderDrawVertex, RenderGraph, RenderPass, RenderPassCommand, RenderPassKind,
        RenderResource,
    };
    use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;
    use std::io::BufReader;

    let directory = std::env::temp_dir().join(format!(
        "player-contain-capture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    for [width, height] in [[60, 40], [75, 50], [48, 52], [60, 50]] {
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "contain-capture-test".into(),
            target_id: "surface".into(),
            target_extent: GameViewExtent::new(48, 40),
            display_rect: GameViewRect::new(0., 0., width as f32, height as f32),
            scale_policy: engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: Vec::new(),
        })
        .unwrap();
        let mut backend =
            engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(width, height)
                .expect("contain regression requires a GPU adapter");
        let quad = |bounds: [f32; 4], rgba| {
            let [left, top, right, bottom] = bounds;
            let corners = [[left, bottom], [right, bottom], [right, top], [left, top]];
            [0usize, 1, 2, 0, 2, 3]
                .map(|index| RenderDrawVertex::new(corners[index], rgba, [0., 0.]))
        };
        let mut vertices = quad([-1., 1., 1., -1.], [0., 1., 0., 1.]).to_vec();
        // A centered 16x16 square in the fixed 48x40 target, in target-space NDC.
        vertices.extend(quad([-1. / 3., 0.4, 1. / 3., -0.4], [1., 0., 0., 1.]));
        let mut graph = RenderGraph::new("contain-capture", 1);
        graph.output_target = Some("surface".into());
        graph
            .resources
            .push(RenderResource::surface_backbuffer("surface", width, height));
        graph.passes.push(RenderPass {
            pass_id: "ui".into(),
            pass_name: "Target-space UI".into(),
            pass_kind: RenderPassKind::DrawUiComposition,
            view_id: "view".into(),
            reads: Vec::new(),
            writes: vec!["surface".into()],
            color_targets: vec!["surface".into()],
            depth_target: None,
            commands: vec![
                RenderPassCommand::Clear {
                    target: "surface".into(),
                    color: color([0., 0., 0., 1.]),
                },
                RenderPassCommand::DrawUiComposition {
                    target: "surface".into(),
                    stage: "screen_overlay".into(),
                    item_count: 2,
                    text_count: 0,
                    image_count: 0,
                    glyph_count: 0,
                    font_atlas_id: None,
                    text_pass_inserted: false,
                    debug_label: "background and square".into(),
                    texture: None,
                    font_render_mode: None,
                    font_page_index: None,
                    vertices: vertices.into(),
                },
            ]
            .into(),
            debug_source: None,
        });
        let path = directory.join(format!("{width}x{height}.png"));
        let request =
            NativePlayerWindowRunRequest::windowed("unused-package").with_screenshot(&path);
        let mut report = NativeWindowHostReport::base(&request);
        capture_screenshot(
            &mut backend,
            &compile_render_graph_to_rhi_plan(&graph),
            &request,
            &mut report,
            width,
            height,
            Some(presentation.display_content_rect),
        );
        assert_eq!(
            report.screenshot.status,
            NativeWindowScreenshotStatus::Captured
        );
        let mut reader = png::Decoder::new(BufReader::new(std::fs::File::open(&path).unwrap()))
            .read_info()
            .unwrap();
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut bytes).unwrap();
        assert_eq!([info.width, info.height], [width, height]);
        let pixel = |x: u32, y: u32| {
            let index = ((y * width + x) * 4) as usize;
            &bytes[index..index + 4]
        };
        let is_red = |rgba: &[u8]| rgba[0] > 200 && rgba[1] < 8 && rgba[2] < 8;
        let red_width = (0..width).filter(|&x| is_red(pixel(x, height / 2))).count();
        let red_height = (0..height).filter(|&y| is_red(pixel(width / 2, y))).count();
        let expected = 16. * (width as f32 / 48.).min(height as f32 / 40.);
        assert!(
            (red_width as f32 - expected).abs() <= 1.,
            "{width}x{height}: red width {red_width}, expected {expected}"
        );
        assert!(
            (red_height as f32 - expected).abs() <= 1.,
            "{width}x{height}: red height {red_height}, expected {expected}"
        );
        if width as f32 / 48. > height as f32 / 40. {
            assert_eq!(pixel(0, height / 2), &[0, 0, 0, 255]);
            assert_eq!(pixel(width - 1, height / 2), &[0, 0, 0, 255]);
        } else if width as f32 / 48. < height as f32 / 40. {
            assert_eq!(pixel(width / 2, 0), &[0, 0, 0, 255]);
            assert_eq!(pixel(width / 2, height - 1), &[0, 0, 0, 255]);
        }
        let display = presentation
            .target_to_display(GameViewPoint::new(24., 20.))
            .unwrap();
        assert!(is_red(pixel(
            display.x.floor() as u32,
            display.y.floor() as u32
        )));
        let mut events = vec![RawInputEvent::mouse_move(1, "window", display.x, display.y)];
        map_display_pointer_events_to_target(&mut events, &presentation);
        assert_eq!(events[0].value, RawInputValue::Pointer { x: 24., y: 20. });
    }
}
fn capture_screenshot(
    backend: &mut engine_runtime::wgpu_backend::real::RealWgpuBackend,
    plan: &engine_runtime::rhi_command_plan::RhiCommandPlan,
    request: &NativePlayerWindowRunRequest,
    report: &mut NativeWindowHostReport,
    width: u32,
    height: u32,
    display_content_rect: Option<GameViewRect>,
) {
    let path = request.screenshot.path.clone().unwrap_or_else(|| {
        request
            .runtime_package_path
            .parent()
            .unwrap_or(&request.runtime_package_path)
            .join("reports")
            .join("windowed-player-screenshot.png")
    });
    let (rgba, token) = match backend.render_plan_to_rgba_bytes_with_receipt(
        plan,
        width,
        height,
        display_content_rect,
    ) {
        Ok(capture) => capture,
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
    backend.record_submission_capture(token);
    report.screenshot.submission = Some(token);
    report.screenshot.frame_identity = plan.frame_identity.clone();
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
