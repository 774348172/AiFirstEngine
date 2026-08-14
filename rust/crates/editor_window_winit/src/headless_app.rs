use crate::config::NativeEditorWindowConfig;
use crate::headless_window::HeadlessWindowBackend;
use crate::input_route::CminInputRoute;
use crate::surface::HeadlessSurfaceBackend;
use crate::viewport::ViewportHost;
use editor_input::{EditorInputEvent, EditorInputRouter, PointerButton};
use editor_ui_model::EditorUiModel;
use editor_ui_renderer::{HitTarget, SelfUiRenderer, UiDrawList, UiRendererConfig};
use editor_wgpu_renderer::{
    EditorSharedGpuContextSummary, GameViewPublicationReceipt, HeadlessUiGpuRenderer,
    RealUiPresentReport,
};
use serde::{Deserialize, Serialize};

pub const REAL_NATIVE_EDITOR_WINDOW_REPORT_SCHEMA_VERSION: &str =
    "real-native-editor-window-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealNativeEditorWindowDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealNativeEditorWindowDiagnostic {
    pub severity: RealNativeEditorWindowDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealNativeEditorWindowReport {
    pub schema_version: String,
    pub backend: String,
    pub window_created: bool,
    pub surface_created: bool,
    pub surface_configured: bool,
    pub device_created: bool,
    pub frame_index: u64,
    pub draw_command_count: usize,
    pub hit_region_count: usize,
    pub input_event_count: usize,
    pub ui_command_count: usize,
    pub present_status: String,
    pub shared_gpu_context_status: String,
    pub shared_gpu_backend: String,
    pub viewport_texture_registry_count: usize,
    pub viewport_texture_lifecycle_event_count: u64,
    pub game_view_publication_receipt: Option<GameViewPublicationReceipt>,
    pub resize_count: u64,
    pub close_requested: bool,
    pub diagnostics: Vec<RealNativeEditorWindowDiagnostic>,
}

impl RealNativeEditorWindowReport {
    pub fn new(backend: impl Into<String>) -> Self {
        Self {
            schema_version: REAL_NATIVE_EDITOR_WINDOW_REPORT_SCHEMA_VERSION.to_string(),
            backend: backend.into(),
            window_created: false,
            surface_created: false,
            surface_configured: false,
            device_created: false,
            frame_index: 0,
            draw_command_count: 0,
            hit_region_count: 0,
            input_event_count: 0,
            ui_command_count: 0,
            present_status: "not_presented".to_string(),
            shared_gpu_context_status: "headless_mock".to_string(),
            shared_gpu_backend: "headless".to_string(),
            viewport_texture_registry_count: 0,
            viewport_texture_lifecycle_event_count: 0,
            game_view_publication_receipt: None,
            resize_count: 0,
            close_requested: false,
            diagnostics: Vec::new(),
        }
    }

    pub fn environment_blocked(backend: impl Into<String>, message: impl Into<String>) -> Self {
        let mut report = Self::new(backend);
        report.present_status = "environment_blocked".to_string();
        report.diagnostics.push(RealNativeEditorWindowDiagnostic {
            severity: RealNativeEditorWindowDiagnosticSeverity::Error,
            code: "environment_blocked".to_string(),
            message: message.into(),
            source_stage: "real_window_feature_gate".to_string(),
        });
        report
    }

    pub fn feature_not_enabled() -> Self {
        let mut report = Self::new("headless");
        report.present_status = "real_window_feature_not_enabled".to_string();
        report.diagnostics.push(RealNativeEditorWindowDiagnostic {
            severity: RealNativeEditorWindowDiagnosticSeverity::Warning,
            code: "real_window_feature_not_enabled".to_string(),
            message: "Re-run with the real-window feature to open a native editor window."
                .to_string(),
            source_stage: "editor_host".to_string(),
        });
        report
    }

    pub fn apply_ui_present_report(&mut self, report: &RealUiPresentReport) {
        self.draw_command_count = report.draw_command_count;
        self.present_status = report.present_status.clone();
        for diagnostic in &report.diagnostics {
            self.diagnostics.push(RealNativeEditorWindowDiagnostic {
                severity: match diagnostic.severity {
                    editor_wgpu_renderer::RealUiPresentDiagnosticSeverity::Info => {
                        RealNativeEditorWindowDiagnosticSeverity::Info
                    }
                    editor_wgpu_renderer::RealUiPresentDiagnosticSeverity::Warning => {
                        RealNativeEditorWindowDiagnosticSeverity::Warning
                    }
                    editor_wgpu_renderer::RealUiPresentDiagnosticSeverity::Error => {
                        RealNativeEditorWindowDiagnosticSeverity::Error
                    }
                },
                code: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                source_stage: diagnostic.source_stage.clone(),
            });
        }
    }

    pub fn apply_shared_gpu_context_summary(&mut self, summary: &EditorSharedGpuContextSummary) {
        self.shared_gpu_context_status = format!("{:?}", summary.status);
        self.shared_gpu_backend = summary.backend_name.clone();
        if !summary.real_wgpu_available && self.present_status == "not_presented" {
            self.present_status = "gpu_unavailable".to_string();
        }
    }

    pub fn apply_viewport_texture_registry_state(
        &mut self,
        texture_count: usize,
        lifecycle_event_count: u64,
    ) {
        self.viewport_texture_registry_count = texture_count;
        self.viewport_texture_lifecycle_event_count = lifecycle_event_count;
    }

    pub fn apply_game_view_publication_receipt(&mut self, receipt: GameViewPublicationReceipt) {
        self.game_view_publication_receipt = Some(receipt);
    }
}

pub struct HeadlessNativeEditorWindowApp {
    window: HeadlessWindowBackend,
    surface: HeadlessSurfaceBackend,
    ui_renderer: HeadlessUiGpuRenderer,
    router: EditorInputRouter,
    viewport_host: ViewportHost,
    report: RealNativeEditorWindowReport,
    latest_draw_list: Option<UiDrawList>,
}

impl HeadlessNativeEditorWindowApp {
    pub fn new(config: NativeEditorWindowConfig) -> Self {
        let window = HeadlessWindowBackend::create_window(&config);
        let mut surface = HeadlessSurfaceBackend::create_surface();
        surface.configure(config.width, config.height, "Bgra8UnormSrgb", "Fifo");
        let mut report = RealNativeEditorWindowReport::new("headless");
        report.window_created = true;
        report.surface_created = true;
        report.surface_configured = true;
        report.device_created = true;

        Self {
            window,
            surface,
            ui_renderer: HeadlessUiGpuRenderer::new(),
            router: EditorInputRouter::new(),
            viewport_host: ViewportHost::new(),
            report,
            latest_draw_list: None,
        }
    }

    pub fn frame(&mut self, model: &EditorUiModel) -> RealNativeEditorWindowReport {
        let window = self.window.snapshot();
        let draw_list = SelfUiRenderer::build_draw_list(
            model,
            UiRendererConfig::new(window.size.width as f32, window.size.height as f32),
        );
        let ui_present_report = self.ui_renderer.present(&draw_list);
        if self.viewport_host.scene_viewport().is_none() {
            if let Some(region) = draw_list
                .hit_regions
                .iter()
                .find(|region| region.target == HitTarget::Viewport)
            {
                let _ = self
                    .viewport_host
                    .register_scene_viewport("scene-view", region.rect);
            }
        }

        self.surface.acquire();
        self.surface.present();
        self.report.frame_index += 1;
        self.report.apply_ui_present_report(&ui_present_report);
        self.report.hit_region_count = draw_list.hit_regions.len();
        if let Some(error) = self.surface.snapshot().last_error {
            self.report.present_status = error;
        }
        self.latest_draw_list = Some(draw_list);
        self.report.clone()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.window.resize(width, height, 1.0);
        self.surface.resize(width, height, "Bgra8UnormSrgb", "Fifo");
        self.report.surface_configured = true;
        self.report.resize_count += 1;
    }

    pub fn click(
        &mut self,
        x: f32,
        y: f32,
        draw_list: &UiDrawList,
    ) -> RealNativeEditorWindowReport {
        self.report.input_event_count += 1;
        let event = EditorInputEvent::PointerDown {
            x,
            y,
            button: PointerButton::Primary,
        };
        let command = self.router.route(event.clone(), draw_list).command;
        let route = self.viewport_host.route_input(&event, draw_list);
        if command.is_some() || route == CminInputRoute::Ui {
            self.report.ui_command_count += 1;
        }
        self.report.clone()
    }

    pub fn close(&mut self) {
        self.window.close();
        self.report.close_requested = true;
    }

    pub fn report(&self) -> RealNativeEditorWindowReport {
        self.report.clone()
    }

    pub fn latest_draw_list(&self) -> Option<&UiDrawList> {
        self.latest_draw_list.as_ref()
    }
}
