use serde::{Deserialize, Serialize};

use crate::diagnostics::{
    RealUiPresentDiagnostic, RealUiPresentDiagnosticSeverity, UI_RENDER_GRAPH_SCHEMA_VERSION,
    UI_RHI_COMMAND_PLAN_SCHEMA_VERSION,
};
use crate::draw_plan::{UiGpuDrawPlan, UiGpuPaintBatchKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiRenderPassKind {
    Clear,
    DrawRects,
    DrawText,
    DrawImageTextures,
    DrawViewportTextures,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiRenderResourceKind {
    SurfaceBackbuffer,
    VertexBuffer,
    GlyphAtlasTexture,
    ImageTexture,
    ViewportTexture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiRenderResource {
    pub id: String,
    pub kind: UiRenderResourceKind,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiRenderPass {
    pub id: String,
    pub kind: UiRenderPassKind,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub command_count: usize,
    pub first_item: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiRenderGraph {
    pub schema_version: String,
    pub surface_width: u32,
    pub surface_height: u32,
    pub resources: Vec<UiRenderResource>,
    pub passes: Vec<UiRenderPass>,
    pub diagnostics: Vec<RealUiPresentDiagnostic>,
}

impl UiRenderGraph {
    pub fn from_draw_plan(plan: &UiGpuDrawPlan) -> Self {
        let mut resources = vec![UiRenderResource {
            id: "surface-backbuffer".to_string(),
            kind: UiRenderResourceKind::SurfaceBackbuffer,
            label: "Editor UI surface backbuffer".to_string(),
        }];
        let mut passes = vec![UiRenderPass {
            id: "clear-ui-surface".to_string(),
            kind: UiRenderPassKind::Clear,
            reads: Vec::new(),
            writes: vec!["surface-backbuffer".to_string()],
            command_count: 1,
            first_item: 0,
        }];

        if !plan.drawable_rects.is_empty() {
            resources.push(UiRenderResource {
                id: "ui-rect-vertex-buffer".to_string(),
                kind: UiRenderResourceKind::VertexBuffer,
                label: "Editor UI rect vertices".to_string(),
            });
        }

        if !plan.text_glyphs.is_empty() {
            resources.push(UiRenderResource {
                id: "ui-text-vertex-buffer".to_string(),
                kind: UiRenderResourceKind::VertexBuffer,
                label: "Editor UI text vertices".to_string(),
            });
            resources.push(UiRenderResource {
                id: "ui-glyph-atlas".to_string(),
                kind: UiRenderResourceKind::GlyphAtlasTexture,
                label: "Editor UI glyph atlas".to_string(),
            });
        }

        if !plan.viewport_texture_quads.is_empty() {
            resources.push(UiRenderResource {
                id: "ui-viewport-texture-vertex-buffer".to_string(),
                kind: UiRenderResourceKind::VertexBuffer,
                label: "Editor UI viewport texture vertices".to_string(),
            });
            for quad in &plan.viewport_texture_quads {
                resources.push(UiRenderResource {
                    id: format!("viewport-texture::{}", quad.texture_id),
                    kind: UiRenderResourceKind::ViewportTexture,
                    label: format!("Viewport texture {}", quad.texture_id),
                });
            }
        }

        if !plan.image_texture_quads.is_empty() {
            resources.push(UiRenderResource {
                id: "ui-image-texture-vertex-buffer".to_string(),
                kind: UiRenderResourceKind::VertexBuffer,
                label: "Editor UI image texture vertices".to_string(),
            });
            for quad in &plan.image_texture_quads {
                resources.push(UiRenderResource {
                    id: format!("image-texture::{}", quad.texture_id),
                    kind: UiRenderResourceKind::ImageTexture,
                    label: format!("Editor image texture {}", quad.texture_id),
                });
            }
        }

        for (batch_index, batch) in plan.paint_batches.iter().enumerate() {
            let (kind, label, reads) = match batch.kind {
                UiGpuPaintBatchKind::Rects => (
                    UiRenderPassKind::DrawRects,
                    "rects",
                    vec!["ui-rect-vertex-buffer".to_string()],
                ),
                UiGpuPaintBatchKind::Text => (
                    UiRenderPassKind::DrawText,
                    "text",
                    vec![
                        "ui-text-vertex-buffer".to_string(),
                        "ui-glyph-atlas".to_string(),
                    ],
                ),
                UiGpuPaintBatchKind::ViewportTextures => (
                    UiRenderPassKind::DrawViewportTextures,
                    "viewport-textures",
                    plan.viewport_texture_quads
                        [batch.first_item..batch.first_item + batch.item_count]
                        .iter()
                        .map(|quad| format!("viewport-texture::{}", quad.texture_id))
                        .collect(),
                ),
                UiGpuPaintBatchKind::ImageTextures => (
                    UiRenderPassKind::DrawImageTextures,
                    "image-textures",
                    plan.image_texture_quads[batch.first_item..batch.first_item + batch.item_count]
                        .iter()
                        .map(|quad| format!("image-texture::{}", quad.texture_id))
                        .collect(),
                ),
            };
            passes.push(UiRenderPass {
                id: format!("draw-ui-{label}-{batch_index}"),
                kind,
                reads,
                writes: vec!["surface-backbuffer".to_string()],
                command_count: batch.item_count,
                first_item: batch.first_item,
            });
        }

        passes.push(UiRenderPass {
            id: "present-ui-surface".to_string(),
            kind: UiRenderPassKind::Present,
            reads: vec!["surface-backbuffer".to_string()],
            writes: Vec::new(),
            command_count: 1,
            first_item: 0,
        });

        let mut diagnostics = Vec::new();
        if plan.drawable_rects.is_empty()
            && plan.text_glyphs.is_empty()
            && plan.viewport_texture_quads.is_empty()
            && plan.image_texture_quads.is_empty()
            && plan.draw_command_count > 0
        {
            diagnostics.push(RealUiPresentDiagnostic {
                severity: RealUiPresentDiagnosticSeverity::Warning,
                code: "ui_render_graph.no_draw_commands".to_string(),
                message: "DrawList produced no drawable UI render graph draw passes.".to_string(),
                source_stage: "editor_wgpu_renderer.ui_render_graph".to_string(),
            });
        }

        Self {
            schema_version: UI_RENDER_GRAPH_SCHEMA_VERSION.to_string(),
            surface_width: plan.surface_width,
            surface_height: plan.surface_height,
            resources,
            passes,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiRhiCommandKind {
    ClearSurface,
    DrawRectBatch,
    DrawTextBatch,
    DrawImageTextureBatch,
    DrawViewportTextureBatch,
    PresentSurface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiRhiCommand {
    pub kind: UiRhiCommandKind,
    pub pass_id: String,
    pub item_count: usize,
    pub first_item: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiRhiCommandPlan {
    pub schema_version: String,
    pub surface_width: u32,
    pub surface_height: u32,
    pub commands: Vec<UiRhiCommand>,
    pub diagnostics: Vec<RealUiPresentDiagnostic>,
}

impl UiRhiCommandPlan {
    pub fn from_render_graph(graph: &UiRenderGraph) -> Self {
        let mut commands = Vec::new();
        let mut diagnostics = graph.diagnostics.clone();
        for pass in &graph.passes {
            let kind = match pass.kind {
                UiRenderPassKind::Clear => UiRhiCommandKind::ClearSurface,
                UiRenderPassKind::DrawRects => UiRhiCommandKind::DrawRectBatch,
                UiRenderPassKind::DrawText => UiRhiCommandKind::DrawTextBatch,
                UiRenderPassKind::DrawImageTextures => UiRhiCommandKind::DrawImageTextureBatch,
                UiRenderPassKind::DrawViewportTextures => {
                    UiRhiCommandKind::DrawViewportTextureBatch
                }
                UiRenderPassKind::Present => UiRhiCommandKind::PresentSurface,
            };
            commands.push(UiRhiCommand {
                kind,
                pass_id: pass.id.clone(),
                item_count: pass.command_count,
                first_item: pass.first_item,
            });
        }
        if !commands
            .iter()
            .any(|command| command.kind == UiRhiCommandKind::PresentSurface)
        {
            diagnostics.push(RealUiPresentDiagnostic {
                severity: RealUiPresentDiagnosticSeverity::Error,
                code: "ui_rhi_command_plan.missing_present".to_string(),
                message: "UI RHI command plan has no present command.".to_string(),
                source_stage: "editor_wgpu_renderer.ui_rhi_command_plan".to_string(),
            });
        }
        Self {
            schema_version: UI_RHI_COMMAND_PLAN_SCHEMA_VERSION.to_string(),
            surface_width: graph.surface_width,
            surface_height: graph.surface_height,
            commands,
            diagnostics,
        }
    }
}
