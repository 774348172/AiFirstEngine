use crate::headless_window::WindowState;
use crate::input_route::CminInputRoute;
use crate::runtime_render::RuntimeFrameDescriptor;
use crate::surface::SurfaceState;
use crate::viewport::{RuntimeViewportFrameSummary, ViewportState};
use editor_ui_renderer::UiDrawList;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuState {
    pub adapter_name: String,
    pub backend: String,
    pub device_created: bool,
    pub queue_created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameState {
    pub frame_index: u64,
    pub redraw_requested: bool,
    pub acquired_surface_texture: bool,
    pub presented: bool,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealWindowGateReport {
    pub schema_version: String,
    pub window: WindowState,
    pub surface: SurfaceState,
    pub gpu: GpuState,
    pub frame: FrameState,
    pub viewport: Option<ViewportState>,
    pub runtime_viewport_frame: Option<RuntimeViewportFrameSummary>,
    pub input: Vec<CminInputRoute>,
    pub draw_list: UiDrawListSummary,
    pub runtime_frame: Option<RuntimeFrameDescriptor>,
    pub diagnostics: Vec<GateDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiDrawListSummary {
    pub revision: u64,
    pub frame: u64,
    pub command_count: usize,
    pub hit_region_count: usize,
    pub has_viewport_slot: bool,
}

pub fn summarize_draw_list(draw_list: &UiDrawList) -> UiDrawListSummary {
    UiDrawListSummary {
        revision: draw_list.revision,
        frame: draw_list.frame,
        command_count: draw_list.commands.len(),
        hit_region_count: draw_list.hit_regions.len(),
        has_viewport_slot: draw_list.commands.iter().any(|command| {
            matches!(
                command,
                editor_ui_renderer::DrawCommand::ViewportTextureSlot { .. }
            )
        }),
    }
}

pub fn build_real_window_gate_report(
    window: WindowState,
    surface: SurfaceState,
    viewport: Option<ViewportState>,
    runtime_viewport_frame: Option<RuntimeViewportFrameSummary>,
    draw_list: &UiDrawList,
    runtime_frame: Option<RuntimeFrameDescriptor>,
    input: Vec<CminInputRoute>,
) -> RealWindowGateReport {
    let mut diagnostics = Vec::new();
    if !surface.configured {
        diagnostics.push(GateDiagnostic {
            severity: DiagnosticSeverity::Error,
            code: "surface_not_configured".to_string(),
            message: "Surface is not configured.".to_string(),
            source_stage: "surface".to_string(),
        });
    }
    if viewport.is_none() {
        diagnostics.push(GateDiagnostic {
            severity: DiagnosticSeverity::Error,
            code: "viewport_missing".to_string(),
            message: "Scene viewport is missing.".to_string(),
            source_stage: "viewport".to_string(),
        });
    }
    if let Some(error) = &surface.last_error {
        diagnostics.push(GateDiagnostic {
            severity: DiagnosticSeverity::Error,
            code: error.clone(),
            message: format!("Surface reported error: {error}"),
            source_stage: "surface".to_string(),
        });
    }

    let frame_index = runtime_frame
        .as_ref()
        .map(|frame| frame.frame_id)
        .unwrap_or(surface.presented_frame);
    RealWindowGateReport {
        schema_version: "real-window-gate-report.v1".to_string(),
        window,
        surface: surface.clone(),
        gpu: GpuState {
            adapter_name: "headless-adapter".to_string(),
            backend: "headless".to_string(),
            device_created: true,
            queue_created: true,
        },
        frame: FrameState {
            frame_index,
            redraw_requested: surface.presented_frame > 0,
            acquired_surface_texture: surface.acquired_frame > 0,
            presented: surface.presented_frame > 0,
            error_code: surface.last_error,
        },
        viewport,
        runtime_viewport_frame,
        input,
        draw_list: summarize_draw_list(draw_list),
        runtime_frame,
        diagnostics,
    }
}
