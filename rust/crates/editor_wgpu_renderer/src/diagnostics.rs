use serde::{Deserialize, Serialize};

use crate::draw_plan::UiGpuDrawPlan;
use crate::render_graph::{UiRenderGraph, UiRhiCommandKind, UiRhiCommandPlan};

pub const UI_GPU_DRAW_PLAN_SCHEMA_VERSION: &str = "ui-gpu-draw-plan.v2";
pub const UI_RENDER_GRAPH_SCHEMA_VERSION: &str = "ui-render-graph.v2";
pub const UI_RHI_COMMAND_PLAN_SCHEMA_VERSION: &str = "ui-rhi-command-plan.v2";
pub const REAL_UI_PRESENT_REPORT_SCHEMA_VERSION: &str = "real-ui-present-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealUiPresentDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealUiPresentDiagnostic {
    pub severity: RealUiPresentDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealUiPresentReport {
    pub schema_version: String,
    pub backend: String,
    pub surface_width: u32,
    pub surface_height: u32,
    pub draw_command_count: usize,
    pub rect_count: usize,
    pub viewport_slot_count: usize,
    pub image_texture_slot_count: usize,
    pub skipped_text_count: usize,
    pub text_command_count: usize,
    pub rendered_glyph_count: usize,
    pub unsupported_glyph_count: usize,
    pub font_backend: String,
    pub font_loaded: bool,
    pub font_source: Option<String>,
    pub glyph_atlas_width: u32,
    pub glyph_atlas_height: u32,
    pub glyph_cache_count: usize,
    pub missing_glyph_count: usize,
    pub submitted_batch_count: usize,
    pub presented: bool,
    pub present_status: String,
    pub diagnostics: Vec<RealUiPresentDiagnostic>,
}

impl RealUiPresentReport {
    pub fn from_plan(backend: impl Into<String>, plan: &UiGpuDrawPlan, presented: bool) -> Self {
        let graph = UiRenderGraph::from_draw_plan(plan);
        let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);
        Self::from_compiled_plan(backend, plan, &rhi_plan, presented)
    }

    pub fn from_compiled_plan(
        backend: impl Into<String>,
        plan: &UiGpuDrawPlan,
        rhi_plan: &UiRhiCommandPlan,
        presented: bool,
    ) -> Self {
        let mut diagnostics = Vec::new();
        if plan.drawable_rects.is_empty() && plan.draw_command_count > 0 {
            diagnostics.push(RealUiPresentDiagnostic {
                severity: RealUiPresentDiagnosticSeverity::Warning,
                code: "ui_present.no_drawable_rects".to_string(),
                message: "DrawList contains commands but no C-min drawable rects.".to_string(),
                source_stage: "editor_wgpu_renderer.plan".to_string(),
            });
        }
        diagnostics.extend(rhi_plan.diagnostics.clone());

        Self {
            schema_version: REAL_UI_PRESENT_REPORT_SCHEMA_VERSION.to_string(),
            backend: backend.into(),
            surface_width: plan.surface_width,
            surface_height: plan.surface_height,
            draw_command_count: plan.draw_command_count,
            rect_count: plan.rect_count,
            viewport_slot_count: plan.viewport_slot_count,
            image_texture_slot_count: plan.image_texture_slot_count,
            skipped_text_count: plan.skipped_text_count,
            text_command_count: plan.text_command_count,
            rendered_glyph_count: plan.rendered_glyph_count,
            unsupported_glyph_count: plan.unsupported_glyph_count,
            font_backend: plan.font_backend.clone(),
            font_loaded: plan.font_loaded,
            font_source: plan.font_source.clone(),
            glyph_atlas_width: plan.glyph_atlas_width,
            glyph_atlas_height: plan.glyph_atlas_height,
            glyph_cache_count: plan.glyph_cache_count,
            missing_glyph_count: plan.missing_glyph_count,
            submitted_batch_count: rhi_plan
                .commands
                .iter()
                .filter(|command| {
                    matches!(
                        command.kind,
                        UiRhiCommandKind::DrawRectBatch
                            | UiRhiCommandKind::DrawTextBatch
                            | UiRhiCommandKind::DrawViewportTextureBatch
                            | UiRhiCommandKind::DrawImageTextureBatch
                    )
                })
                .count(),
            presented,
            present_status: if presented {
                "presented".to_string()
            } else {
                "not_presented".to_string()
            },
            diagnostics,
        }
    }

    pub fn failed(
        backend: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        source_stage: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: REAL_UI_PRESENT_REPORT_SCHEMA_VERSION.to_string(),
            backend: backend.into(),
            surface_width: 0,
            surface_height: 0,
            draw_command_count: 0,
            rect_count: 0,
            viewport_slot_count: 0,
            image_texture_slot_count: 0,
            skipped_text_count: 0,
            text_command_count: 0,
            rendered_glyph_count: 0,
            unsupported_glyph_count: 0,
            font_backend: "none".to_string(),
            font_loaded: false,
            font_source: None,
            glyph_atlas_width: 0,
            glyph_atlas_height: 0,
            glyph_cache_count: 0,
            missing_glyph_count: 0,
            submitted_batch_count: 0,
            presented: false,
            present_status: "failed".to_string(),
            diagnostics: vec![RealUiPresentDiagnostic {
                severity: RealUiPresentDiagnosticSeverity::Error,
                code: code.into(),
                message: message.into(),
                source_stage: source_stage.into(),
            }],
        }
    }
}
