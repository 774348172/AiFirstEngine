use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::aui::{
    AuiCompositionFrame, AuiCompositionStage, AuiCompositionStageFrame, AuiOverlayFrame,
    AuiOverlayItemKind,
};
use crate::engine_rhi::{EngineRhiBackend, RhiBackendReport};
use crate::font_bundle::FontBundleRenderMode;
use crate::game_view_presentation::{
    CanvasReferenceFact, GameViewExtent, GameViewPresentationModule, GameViewPresentationSpec,
    GameViewRect, GameViewScalePolicy, ResolvedGameViewPresentation,
};
use crate::gpu_texture_lifetime::GpuTextureLifetimeReport;
use crate::headless_rhi_backend::HeadlessRhiBackend;
use crate::render_graph::{
    color, RenderDrawVertex, RenderGraph, RenderGraphDiagnostic, RenderGraphView, RenderPass,
    RenderPassCommand, RenderPassKind, RenderResource,
};
use crate::render_graph_report::RenderGraphReport;
use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
use crate::render_state::{RenderPayloadKind, RenderSceneState, RenderTargetKind, RenderViewState};
use crate::renderer_feature_builder::RendererFeatureBuilder;
use crate::rhi_command_plan::{compile_render_graph_to_rhi_plan, RhiCommandPlan};
use crate::runtime_texture::RuntimeTextureBindingContext;
use crate::sprite2d_render_pipeline::{
    Sprite2DDrawPlan, Sprite2DRenderDiagnostic, Sprite2DRenderPipeline, Sprite2DRenderSeverity,
    Sprite2DTextureBindingContext,
};

pub const DEFAULT_2D_ORTHOGRAPHIC_HALF_HEIGHT: f32 = 7.5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityProfile {
    pub profile_id: String,
}

impl Default for QualityProfile {
    fn default() -> Self {
        Self {
            profile_id: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTarget {
    pub target_id: String,
    pub target_kind: RuntimeRenderTargetKind,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    #[serde(default)]
    pub presentation_scale_policy: GameViewScalePolicy,
}

impl RenderTarget {
    pub fn headless_texture(target_id: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            target_id: target_id.into(),
            target_kind: RuntimeRenderTargetKind::HeadlessTexture,
            width,
            height,
            format: "Rgba8Unorm".to_string(),
            color_space: "srgb".to_string(),
            presentation_scale_policy: GameViewScalePolicy::Stretch,
        }
    }

    pub fn viewport_texture(target_id: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            target_id: target_id.into(),
            target_kind: RuntimeRenderTargetKind::ViewportTexture,
            width,
            height,
            format: "Rgba8Unorm".to_string(),
            color_space: "srgb".to_string(),
            presentation_scale_policy: GameViewScalePolicy::Stretch,
        }
    }

    pub fn surface(target_id: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            target_id: target_id.into(),
            target_kind: RuntimeRenderTargetKind::Surface,
            width,
            height,
            format: "Bgra8UnormSrgb".to_string(),
            color_space: "srgb".to_string(),
            presentation_scale_policy: GameViewScalePolicy::Stretch,
        }
    }

    pub fn with_presentation_scale_policy(mut self, policy: GameViewScalePolicy) -> Self {
        self.presentation_scale_policy = policy;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRenderTargetKind {
    HeadlessTexture,
    Surface,
    ViewportTexture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportTextureDescriptor {
    pub texture_id: String,
    pub target_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub frame_index: u64,
    pub producer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderTargetSummary {
    pub frame_index: u64,
    pub target_id: String,
    pub target_kind: RuntimeRenderTargetKind,
    pub width: u32,
    pub height: u32,
    pub texture_descriptor: Option<ViewportTextureDescriptor>,
}

pub struct RuntimeRendererInput<'a> {
    pub frame_index: u64,
    pub render_scene_state: &'a RenderSceneState,
    pub render_view_state: Option<&'a RenderViewState>,
    pub aui_overlay: Option<&'a AuiOverlayFrame>,
    pub aui_composition: Option<&'a AuiCompositionFrame>,
    pub sprite_texture_bindings: Option<&'a Sprite2DTextureBindingContext>,
    pub runtime_texture_bindings: Option<&'a RuntimeTextureBindingContext>,
    pub quality_profile: QualityProfile,
    pub render_target: RenderTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderFrameReport {
    pub frame_index: u64,
    pub graph_id: String,
    pub target_id: String,
    pub pass_count: usize,
    pub draw_item_count: usize,
    pub fallback_count: usize,
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
    pub ui_image_request_count: usize,
    pub ui_image_resolved_count: usize,
    pub ui_image_missing_count: usize,
    pub ui_textured_batch_count: usize,
    pub diagnostics: Vec<RuntimeRendererDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRendererDiagnostic {
    pub severity: RuntimeRendererDiagnosticSeverity,
    pub code: String,
    pub message: String,
}

impl RuntimeRendererDiagnostic {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RuntimeRendererDiagnosticSeverity::Info,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRendererDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRendererOutput {
    pub render_graph: RenderGraph,
    pub rhi_command_plan: RhiCommandPlan,
    pub target_summary: RuntimeRenderTargetSummary,
    pub texture_descriptor: Option<ViewportTextureDescriptor>,
    pub texture_lifetime_report: Option<GpuTextureLifetimeReport>,
    pub render_frame_report: RuntimeRenderFrameReport,
    pub render_graph_report: RenderGraphReport,
    pub rhi_backend_report: Option<RhiBackendReport>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRenderer {
    feature_builder: RendererFeatureBuilder,
    sprite2d_pipeline: Sprite2DRenderPipeline,
}

impl RuntimeRenderer {
    pub fn new() -> Self {
        Self {
            feature_builder: RendererFeatureBuilder::new(),
            sprite2d_pipeline: Sprite2DRenderPipeline::new(),
        }
    }

    pub fn build(&self, input: RuntimeRendererInput<'_>) -> RuntimeRendererOutput {
        let render_graph = self.build_graph(&input);
        let render_graph_report = RenderGraphReport::from_graph(&render_graph);
        let rhi_command_plan = compile_render_graph_to_rhi_plan(&render_graph);
        let texture_descriptor = build_texture_descriptor(&input);
        let target_summary = RuntimeRenderTargetSummary {
            frame_index: input.frame_index,
            target_id: input.render_target.target_id.clone(),
            target_kind: input.render_target.target_kind,
            width: input.render_target.width,
            height: input.render_target.height,
            texture_descriptor: texture_descriptor.clone(),
        };
        let ui_stage_metrics = RuntimeUiCompositionStageMetrics::from_graph(&render_graph);
        let ui_image_request_count = input
            .aui_composition
            .map(|composition| {
                composition
                    .stages
                    .iter()
                    .map(|stage| stage.image_count)
                    .sum()
            })
            .or_else(|| input.aui_overlay.map(|overlay| overlay.report.image_count))
            .unwrap_or(0);
        let ui_image_missing_count = render_graph
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.code.as_str(),
                    "aui_image.asset_id_missing" | "aui_image.texture_not_resolved"
                )
            })
            .count();
        let ui_textured_batch_count = render_graph
            .passes
            .iter()
            .flat_map(|pass| pass.commands.iter())
            .filter(|command| {
                matches!(
                    command,
                    RenderPassCommand::DrawUiComposition {
                        texture: Some(_),
                        font_render_mode: None,
                        ..
                    }
                )
            })
            .count();
        let render_frame_report = RuntimeRenderFrameReport {
            frame_index: input.frame_index,
            graph_id: render_graph.graph_id.clone(),
            target_id: input.render_target.target_id.clone(),
            pass_count: render_graph.passes.len(),
            draw_item_count: render_graph
                .passes
                .iter()
                .flat_map(|pass| pass.commands.iter())
                .map(|command| match command {
                    RenderPassCommand::DrawMeshBasic { .. }
                    | RenderPassCommand::DrawSpriteBasic { .. }
                    | RenderPassCommand::DrawSpriteTextured { .. }
                    | RenderPassCommand::DrawTestGeometry { .. } => 1,
                    RenderPassCommand::DrawUiOverlay { item_count, .. } => *item_count,
                    RenderPassCommand::DrawUiComposition { item_count, .. } => *item_count,
                    _ => 0,
                })
                .sum(),
            fallback_count: render_graph
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "fallback_to_test_geometry")
                .count(),
            ui_composition_stage_count: ui_stage_metrics.stage_count,
            ui_before_world_item_count: ui_stage_metrics.before_world_item_count,
            ui_screen_overlay_item_count: ui_stage_metrics.screen_overlay_item_count,
            ui_modal_item_count: ui_stage_metrics.modal_item_count,
            ui_before_world_pass_present: ui_stage_metrics.before_world_pass_present,
            ui_screen_overlay_pass_present: ui_stage_metrics.screen_overlay_pass_present,
            ui_modal_pass_present: ui_stage_metrics.modal_pass_present,
            ui_before_world_skipped: !ui_stage_metrics.before_world_pass_present,
            ui_screen_overlay_skipped: !ui_stage_metrics.screen_overlay_pass_present,
            ui_modal_skipped: !ui_stage_metrics.modal_pass_present,
            ui_image_request_count,
            ui_image_resolved_count: ui_image_request_count.saturating_sub(ui_image_missing_count),
            ui_image_missing_count,
            ui_textured_batch_count,
            diagnostics: render_graph
                .diagnostics
                .iter()
                .map(|diagnostic| RuntimeRendererDiagnostic {
                    severity: match diagnostic.severity {
                        crate::render_graph::RenderGraphDiagnosticSeverity::Info => {
                            RuntimeRendererDiagnosticSeverity::Info
                        }
                        crate::render_graph::RenderGraphDiagnosticSeverity::Warning => {
                            RuntimeRendererDiagnosticSeverity::Warning
                        }
                        crate::render_graph::RenderGraphDiagnosticSeverity::Error => {
                            RuntimeRendererDiagnosticSeverity::Error
                        }
                    },
                    code: diagnostic.code.clone(),
                    message: diagnostic.message.clone(),
                })
                .collect(),
        };

        RuntimeRendererOutput {
            render_graph,
            rhi_command_plan,
            target_summary,
            texture_descriptor,
            texture_lifetime_report: None,
            render_frame_report,
            render_graph_report,
            rhi_backend_report: None,
        }
    }

    pub fn render_headless(&self, input: RuntimeRendererInput<'_>) -> RuntimeRendererOutput {
        let mut output = self.build(input);
        let mut backend = HeadlessRhiBackend::new();
        output.rhi_backend_report = Some(self.execute_with_rhi_backend(&mut backend, &output));
        output
    }

    pub fn render_with_rhi_backend(
        &self,
        input: RuntimeRendererInput<'_>,
        backend: &mut dyn EngineRhiBackend,
    ) -> RuntimeRendererOutput {
        let mut output = self.build(input);
        output.rhi_backend_report = Some(self.execute_with_rhi_backend(backend, &output));
        output
    }

    fn execute_with_rhi_backend(
        &self,
        backend: &mut dyn EngineRhiBackend,
        output: &RuntimeRendererOutput,
    ) -> RhiBackendReport {
        backend.execute_plan(&output.rhi_command_plan)
    }

    fn build_graph(&self, input: &RuntimeRendererInput<'_>) -> RenderGraph {
        let target_id = input.render_target.target_id.clone();
        let mut graph = RenderGraph::new(
            format!("runtime-graph-{}", input.frame_index),
            input.frame_index,
        );
        graph.output_target = Some(target_id.clone());
        graph.resources.push(RenderResource::surface_backbuffer(
            target_id.clone(),
            input.render_target.width,
            input.render_target.height,
        ));

        let view = input
            .render_view_state
            .or_else(|| input.render_scene_state.views().next());
        let (view_id, view_kind, clear_color) = view
            .map(|view| {
                (
                    view.view_id.to_string(),
                    render_view_kind_name(&view.view_kind).to_string(),
                    view.clear_color,
                )
            })
            .unwrap_or_else(|| {
                (
                    "view-runtime-default".to_string(),
                    "game".to_string(),
                    [0.0, 0.0, 0.0, 1.0],
                )
            });
        graph.views.push(RenderGraphView {
            view_id: view_id.clone(),
            view_kind,
            width: input.render_target.width,
            height: input.render_target.height,
            clear_color: color(clear_color),
        });

        graph.passes.push(RenderPass {
            pass_id: "clear-main".to_string(),
            pass_name: "Clear Main".to_string(),
            pass_kind: RenderPassKind::Clear,
            view_id: view_id.clone(),
            reads: Vec::new(),
            writes: vec![target_id.clone()],
            color_targets: vec![target_id.clone()],
            depth_target: None,
            commands: vec![RenderPassCommand::Clear {
                target: target_id.clone(),
                color: color(clear_color),
            }],
            debug_source: Some("RuntimeRenderer.clear".to_string()),
        });

        let feature_frame = self
            .feature_builder
            .build(input.frame_index, input.render_scene_state);
        let sprite_frame = self.sprite2d_pipeline.build_with_texture_bindings(
            input.frame_index,
            &feature_frame,
            view,
            input.sprite_texture_bindings,
        );

        for diagnostic in &sprite_frame.diagnostics {
            graph
                .diagnostics
                .push(sprite_diagnostic_to_graph_diagnostic(diagnostic));
        }

        let overlay_composition = input
            .aui_overlay
            .map(AuiCompositionFrame::from_overlay_frame);
        let aui_composition = input.aui_composition.or(overlay_composition.as_ref());
        self.push_ui_composition_pass(
            &mut graph,
            aui_composition,
            AuiCompositionStage::BeforeWorld,
            &target_id,
            &view_id,
            input.render_target.presentation_scale_policy,
            input.runtime_texture_bindings,
        );

        for (index, draw_item) in feature_frame
            .draw_items
            .iter()
            .filter(|draw_item| draw_item.payload_kind != RenderPayloadKind::Sprite)
            .enumerate()
        {
            let (pass_kind, command) = if let Some(mesh_ref) = &draw_item.mesh_ref {
                (
                    RenderPassKind::DrawMeshBasic,
                    RenderPassCommand::DrawMeshBasic {
                        target: target_id.clone(),
                        mesh_ref: mesh_ref.clone(),
                        material_ref: draw_item.material_ref.clone(),
                    },
                )
            } else {
                graph.diagnostics.push(RenderGraphDiagnostic::info(
                    "fallback_to_test_geometry",
                    format!(
                        "proxy '{}' has no mesh_ref or sprite_ref; draw test geometry instead",
                        draw_item.proxy_id
                    ),
                ));
                (
                    RenderPassKind::DrawTestGeometry,
                    RenderPassCommand::DrawTestGeometry {
                        target: target_id.clone(),
                        vertex_count: 3,
                    },
                )
            };

            graph.passes.push(RenderPass {
                pass_id: format!("draw-{}", index + 1),
                pass_name: format!("Draw {}", index + 1),
                pass_kind,
                view_id: view_id.clone(),
                reads: Vec::new(),
                writes: vec![target_id.clone()],
                color_targets: vec![target_id.clone()],
                depth_target: None,
                commands: vec![command],
                debug_source: Some(format!("RuntimeRenderer.proxy.{}", draw_item.proxy_id)),
            });
        }

        for (index, draw_plan) in sprite_frame.draw_plans.iter().enumerate() {
            graph.passes.push(RenderPass {
                pass_id: format!("draw-sprite2d-{}", index + 1),
                pass_name: format!("Draw Sprite2D {}", index + 1),
                pass_kind: RenderPassKind::DrawSpriteTextured,
                view_id: view_id.clone(),
                reads: Vec::new(),
                writes: vec![target_id.clone()],
                color_targets: vec![target_id.clone()],
                depth_target: None,
                commands: vec![RenderPassCommand::DrawSpriteTextured {
                    target: target_id.clone(),
                    sprite_ref: draw_plan.sprite_ref.clone(),
                    material_ref: draw_plan.material_ref.clone(),
                    sort_key: format!("{:?}", draw_plan.sort_key),
                    texture: draw_plan
                        .binding
                        .as_ref()
                        .and_then(|binding| binding.resources.first().copied()),
                    binding: draw_plan.binding.clone(),
                    fallback_used: draw_plan.fallback_used,
                    vertices: sprite_draw_vertices(
                        draw_plan,
                        view,
                        input.render_target.width,
                        input.render_target.height,
                    ),
                }],
                debug_source: Some(format!("Sprite2DRenderPipeline.{}", draw_plan.proxy_id)),
            });
        }

        self.push_ui_composition_pass(
            &mut graph,
            aui_composition,
            AuiCompositionStage::ScreenOverlay,
            &target_id,
            &view_id,
            input.render_target.presentation_scale_policy,
            input.runtime_texture_bindings,
        );
        self.push_ui_composition_pass(
            &mut graph,
            aui_composition,
            AuiCompositionStage::Modal,
            &target_id,
            &view_id,
            input.render_target.presentation_scale_policy,
            input.runtime_texture_bindings,
        );

        graph.passes.push(RenderPass {
            pass_id: "present-main".to_string(),
            pass_name: "Present Main".to_string(),
            pass_kind: RenderPassKind::Present,
            view_id,
            reads: vec![target_id.clone()],
            writes: Vec::new(),
            color_targets: Vec::new(),
            depth_target: None,
            commands: vec![RenderPassCommand::Present { target: target_id }],
            debug_source: Some("RuntimeRenderer.present".to_string()),
        });

        graph
    }

    fn push_ui_composition_pass(
        &self,
        graph: &mut RenderGraph,
        composition: Option<&AuiCompositionFrame>,
        stage: AuiCompositionStage,
        target_id: &str,
        view_id: &str,
        scale_policy: GameViewScalePolicy,
        texture_bindings: Option<&RuntimeTextureBindingContext>,
    ) {
        let Some(composition) = composition else {
            return;
        };
        let Some(stage_frame) = composition.stage(stage) else {
            return;
        };
        if stage_frame.is_empty() {
            return;
        }
        let presentation = match resolve_ui_presentation(
            composition,
            stage_frame,
            input_extent_from_graph(graph),
            target_id,
            scale_policy,
        ) {
            Ok(presentation) => presentation,
            Err(code) => {
                graph.diagnostics.push(RenderGraphDiagnostic::error(
                    code,
                    "AUI composition presentation could not be resolved.",
                ));
                return;
            }
        };
        let projected = ui_projection_ordered_batches(
            composition,
            stage_frame,
            &presentation,
            texture_bindings,
        );
        for diagnostic in projected.diagnostics {
            graph.diagnostics.push(diagnostic);
        }
        let mut commands = Vec::new();
        for (index, batch) in projected.batches.into_iter().enumerate() {
            let owns_stage_metrics = index == 0;
            commands.push(RenderPassCommand::DrawUiComposition {
                target: target_id.to_string(),
                stage: stage.as_str().to_string(),
                item_count: if owns_stage_metrics {
                    stage_frame.item_count
                } else {
                    0
                },
                text_count: if owns_stage_metrics {
                    stage_frame.text_count
                } else {
                    0
                },
                image_count: if owns_stage_metrics {
                    stage_frame.image_count
                } else {
                    0
                },
                glyph_count: batch.glyph_count,
                font_atlas_id: composition
                    .glyph_plan
                    .as_ref()
                    .map(|plan| plan.font_atlas_id.clone()),
                text_pass_inserted: batch.font_render_mode.is_some(),
                debug_label: format!("{} {}", stage.debug_label(), batch.debug_label),
                texture: batch.texture,
                font_render_mode: batch.font_render_mode,
                font_page_index: batch.font_page_index,
                vertices: batch.vertices,
            });
        }
        if commands.is_empty() && stage_frame.image_count == 0 {
            commands.push(RenderPassCommand::DrawUiComposition {
                target: target_id.to_string(),
                stage: stage.as_str().to_string(),
                item_count: stage_frame.item_count,
                text_count: stage_frame.text_count,
                image_count: 0,
                glyph_count: 0,
                font_atlas_id: composition
                    .glyph_plan
                    .as_ref()
                    .map(|plan| plan.font_atlas_id.clone()),
                text_pass_inserted: false,
                debug_label: stage.debug_label().to_string(),
                texture: None,
                font_render_mode: None,
                font_page_index: None,
                vertices: Vec::new(),
            });
        }
        graph.passes.push(RenderPass {
            pass_id: format!("draw-aui-{}", stage.pass_id_suffix()),
            pass_name: format!("Draw {}", stage.debug_label()),
            pass_kind: RenderPassKind::DrawUiComposition,
            view_id: view_id.to_string(),
            reads: Vec::new(),
            writes: vec![target_id.to_string()],
            color_targets: vec![target_id.to_string()],
            depth_target: None,
            commands,
            debug_source: Some(format!(
                "RuntimeRenderer.auiComposition.{}",
                stage.pass_id_suffix()
            )),
        });
    }
}

pub fn font_atlas_render_handle(font_atlas_id: &str) -> RenderResourceHandle {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in font_atlas_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    RenderResourceHandle {
        kind: RenderResourceKind::Texture,
        index: hash | (1u64 << 63),
        generation: 1,
    }
}

pub fn font_bundle_page_render_handle(
    font_bundle_id: &str,
    render_mode: FontBundleRenderMode,
    page_index: u32,
) -> RenderResourceHandle {
    font_bundle_page_generation_render_handle(font_bundle_id, render_mode, page_index, 1)
}

pub fn font_bundle_page_generation_render_handle(
    font_bundle_id: &str,
    render_mode: FontBundleRenderMode,
    page_index: u32,
    generation: u64,
) -> RenderResourceHandle {
    let mut handle = font_atlas_render_handle(&format!(
        "{font_bundle_id}:{render_mode:?}:page:{page_index}"
    ));
    handle.generation = generation;
    handle
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiProjectedFontBatch {
    pub render_mode: FontBundleRenderMode,
    pub page_index: u32,
    pub glyph_count: usize,
    pub texture: RenderResourceHandle,
    pub font_render_mode: Option<FontBundleRenderMode>,
    pub vertices: Vec<RenderDrawVertex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum UiProjectedBatchKey {
    Solid,
    Texture(RenderResourceHandle),
    Font(RenderResourceHandle, FontBundleRenderMode, u32),
}

#[derive(Debug, Clone)]
struct UiProjectedOrderedBatch {
    key: UiProjectedBatchKey,
    texture: Option<RenderResourceHandle>,
    font_render_mode: Option<FontBundleRenderMode>,
    font_page_index: Option<u32>,
    glyph_count: usize,
    debug_label: String,
    vertices: Vec<RenderDrawVertex>,
}

#[derive(Debug, Default)]
struct UiProjectedOrderedBatches {
    batches: Vec<UiProjectedOrderedBatch>,
    diagnostics: Vec<RenderGraphDiagnostic>,
}

fn ui_projection_ordered_batches(
    composition: &AuiCompositionFrame,
    stage: &AuiCompositionStageFrame,
    presentation: &ResolvedGameViewPresentation,
    texture_bindings: Option<&RuntimeTextureBindingContext>,
) -> UiProjectedOrderedBatches {
    let mut output = UiProjectedOrderedBatches::default();
    let mut glyphs_by_item = BTreeMap::<&str, Vec<_>>::new();
    if let Some(plan) = composition.glyph_plan.as_ref() {
        for glyph in &plan.quads {
            glyphs_by_item
                .entry(glyph.item_id.as_str())
                .or_default()
                .push(glyph);
        }
    }

    for item in &stage.draw_items {
        match item.item_kind {
            AuiOverlayItemKind::Rect
            | AuiOverlayItemKind::ScrollbarTrack
            | AuiOverlayItemKind::ScrollbarThumb => {
                let Some(color) = parse_aui_color(item.color.as_deref()) else {
                    continue;
                };
                let Some(rect) = clipped_ui_rect(item.rect, item.effective_clip_rect) else {
                    continue;
                };
                let Some(positions) =
                    projected_ui_rect_positions(presentation, item.canvas_id.as_str(), rect)
                else {
                    continue;
                };
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Solid,
                    None,
                    None,
                    None,
                    0,
                    "solid".to_string(),
                    quad_vertices(
                        positions,
                        color,
                        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                    ),
                );
            }
            AuiOverlayItemKind::Image => {
                let Some(asset_id) = item.asset_id.as_deref() else {
                    output.diagnostics.push(RenderGraphDiagnostic::error(
                        "aui_image.asset_id_missing",
                        format!("AUI Image item '{}' has no asset_id.", item.item_id),
                    ));
                    continue;
                };
                let Some(binding) = texture_bindings.and_then(|bindings| bindings.get(asset_id))
                else {
                    output.diagnostics.push(RenderGraphDiagnostic::error(
                        "aui_image.texture_not_resolved",
                        format!(
                            "AUI Image item '{}' could not resolve RuntimePackage texture asset '{}'.",
                            item.item_id, asset_id
                        ),
                    ));
                    continue;
                };
                let Some(vertices) = ui_projection_image_vertices(item, presentation) else {
                    continue;
                };
                append_ui_projected_batch(
                    &mut output.batches,
                    UiProjectedBatchKey::Texture(binding.handle),
                    Some(binding.handle),
                    None,
                    None,
                    0,
                    format!("image:{asset_id}"),
                    vertices,
                );
            }
            AuiOverlayItemKind::Text => {
                let Some(glyphs) = glyphs_by_item.get(item.item_id.as_str()) else {
                    continue;
                };
                let color = parse_aui_color(item.color.as_deref()).unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let Some(plan) = composition.glyph_plan.as_ref() else {
                    continue;
                };
                for glyph in glyphs {
                    let formal_v2 = plan.font_source_kind == "project_font_bundle_v2";
                    let handle = if formal_v2 {
                        font_bundle_page_generation_render_handle(
                            &plan.font_atlas_id,
                            glyph.render_mode,
                            glyph.page_index,
                            plan.atlas_generation,
                        )
                    } else {
                        font_atlas_render_handle(&plan.font_atlas_id)
                    };
                    let Some(positions) = projected_ui_rect_positions(
                        presentation,
                        item.canvas_id.as_str(),
                        glyph.rect,
                    ) else {
                        continue;
                    };
                    let [u0, v0, u1, v1] = glyph.uv_rect;
                    append_ui_projected_batch(
                        &mut output.batches,
                        UiProjectedBatchKey::Font(handle, glyph.render_mode, glyph.page_index),
                        Some(handle),
                        formal_v2.then_some(glyph.render_mode),
                        Some(glyph.page_index),
                        1,
                        format!("font:{:?}:{}", glyph.render_mode, glyph.page_index),
                        quad_vertices(positions, color, [[u0, v1], [u1, v1], [u1, v0], [u0, v0]]),
                    );
                }
            }
        }
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn append_ui_projected_batch(
    batches: &mut Vec<UiProjectedOrderedBatch>,
    key: UiProjectedBatchKey,
    texture: Option<RenderResourceHandle>,
    font_render_mode: Option<FontBundleRenderMode>,
    font_page_index: Option<u32>,
    glyph_count: usize,
    debug_label: String,
    vertices: Vec<RenderDrawVertex>,
) {
    if let Some(last) = batches.last_mut().filter(|batch| batch.key == key) {
        last.glyph_count += glyph_count;
        last.vertices.extend(vertices);
        return;
    }
    batches.push(UiProjectedOrderedBatch {
        key,
        texture,
        font_render_mode,
        font_page_index,
        glyph_count,
        debug_label,
        vertices,
    });
}

fn ui_projection_image_vertices(
    item: &crate::aui::AuiOverlayDrawItem,
    presentation: &ResolvedGameViewPresentation,
) -> Option<Vec<RenderDrawVertex>> {
    let (rect, [u0, v0, u1, v1]) = clipped_ui_rect_with_uv(item.rect, item.effective_clip_rect)?;
    let positions = projected_ui_rect_positions(presentation, item.canvas_id.as_str(), rect)?;
    let color = item
        .color
        .as_deref()
        .and_then(|value| parse_aui_color(Some(value)))
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
    Some(quad_vertices(
        positions,
        color,
        [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
    ))
}

fn sprite_draw_vertices(
    plan: &Sprite2DDrawPlan,
    view: Option<&RenderViewState>,
    width: u32,
    height: u32,
) -> Vec<RenderDrawVertex> {
    let aspect = if height == 0 {
        1.0
    } else {
        width.max(1) as f32 / height as f32
    };
    let fallback_projection = orthographic_2d_projection(
        DEFAULT_2D_ORTHOGRAPHIC_HALF_HEIGHT * aspect,
        DEFAULT_2D_ORTHOGRAPHIC_HALF_HEIGHT,
    );
    let view_matrix = view.map(|view| view.view_matrix).unwrap_or(IDENTITY_MATRIX);
    let projection_matrix = view
        .filter(|view| view.source_entity_id.is_some() || view.projection_matrix != IDENTITY_MATRIX)
        .map(|view| view.projection_matrix)
        .unwrap_or(fallback_projection);
    let position = plan.transform.local_position;
    let scale = plan.transform.local_scale;
    let angle = plan.transform.local_rotation.z.to_radians();
    let (sin, cos) = angle.sin_cos();
    let corners = [
        (-scale.x, -scale.y),
        (scale.x, -scale.y),
        (scale.x, scale.y),
        (-scale.x, scale.y),
    ];
    let positions = corners.map(|(x, y)| {
        let world_x = position.x + x * cos - y * sin;
        let world_y = position.y + x * sin + y * cos;
        let view_position = transform_point_3d(view_matrix, [world_x, world_y, position.z]);
        project_point(projection_matrix, view_position)
    });
    let (u0, u1) = if plan.flip_x { (1.0, 0.0) } else { (0.0, 1.0) };
    let (v0, v1) = if plan.flip_y { (1.0, 0.0) } else { (0.0, 1.0) };
    quad_vertices(
        positions,
        plan.color,
        [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
    )
}

const IDENTITY_MATRIX: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

fn orthographic_2d_projection(half_width: f32, half_height: f32) -> [f32; 16] {
    [
        1.0 / half_width,
        0.0,
        0.0,
        0.0, //
        0.0,
        1.0 / half_height,
        0.0,
        0.0, //
        0.0,
        0.0,
        1.0,
        0.0, //
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

fn transform_point_3d(matrix: [f32; 16], point: [f32; 3]) -> [f32; 3] {
    let x = matrix[0] * point[0] + matrix[4] * point[1] + matrix[8] * point[2] + matrix[12];
    let y = matrix[1] * point[0] + matrix[5] * point[1] + matrix[9] * point[2] + matrix[13];
    let z = matrix[2] * point[0] + matrix[6] * point[1] + matrix[10] * point[2] + matrix[14];
    [x, y, z]
}

fn project_point(matrix: [f32; 16], point: [f32; 3]) -> [f32; 2] {
    let projected = transform_point_3d(matrix, point);
    let w = matrix[3] * point[0] + matrix[7] * point[1] + matrix[11] * point[2] + matrix[15];
    let reciprocal_w = if w.abs() > f32::EPSILON { 1.0 / w } else { 1.0 };
    [projected[0] * reciprocal_w, projected[1] * reciprocal_w]
}

fn input_extent_from_graph(graph: &RenderGraph) -> [f32; 2] {
    graph
        .views
        .first()
        .map(|view| [view.width.max(1) as f32, view.height.max(1) as f32])
        .unwrap_or([1.0, 1.0])
}

fn resolve_ui_presentation(
    composition: &AuiCompositionFrame,
    stage: &AuiCompositionStageFrame,
    output_extent: [f32; 2],
    target_id: &str,
    scale_policy: GameViewScalePolicy,
) -> Result<ResolvedGameViewPresentation, &'static str> {
    let target_extent = GameViewExtent::new(
        output_extent[0].round().max(1.0) as u32,
        output_extent[1].round().max(1.0) as u32,
    );
    let stage_canvas_ids = stage
        .draw_items
        .iter()
        .map(|item| item.canvas_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut canvas_references = composition
        .canvas_references
        .iter()
        .filter(|fact| stage_canvas_ids.contains(fact.canvas_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let known = canvas_references
        .iter()
        .map(|fact| fact.canvas_id.clone())
        .collect::<BTreeSet<_>>();
    for canvas_id in stage_canvas_ids {
        if !known.contains(canvas_id) {
            canvas_references.push(CanvasReferenceFact {
                canvas_id: canvas_id.to_string(),
                reference_extent: target_extent,
            });
        }
    }
    GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: format!("runtime-frame-{}", composition.frame_index),
        target_id: target_id.to_string(),
        target_extent,
        display_rect: GameViewRect::from_extent(target_extent),
        scale_policy,
        surface_generation: 1,
        presentation_revision: composition.frame_index,
        canvas_references,
    })
    .map_err(|error| error.code)
}

pub fn ui_projection_font_batches(
    composition: &AuiCompositionFrame,
    stage: &AuiCompositionStageFrame,
    presentation: &ResolvedGameViewPresentation,
) -> Vec<UiProjectedFontBatch> {
    let Some(plan) = composition.glyph_plan.as_ref() else {
        return Vec::new();
    };
    let item_ids = stage
        .draw_items
        .iter()
        .map(|item| item.item_id.as_str())
        .collect::<HashSet<_>>();
    let item_facts = stage
        .draw_items
        .iter()
        .map(|item| {
            (
                item.item_id.as_str(),
                (
                    item.canvas_id.as_str(),
                    parse_aui_color(item.color.as_deref()).unwrap_or([1.0, 1.0, 1.0, 1.0]),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<(FontBundleRenderMode, u32), Vec<_>>::new();
    for glyph in plan
        .quads
        .iter()
        .filter(|glyph| item_ids.contains(glyph.item_id.as_str()))
    {
        grouped
            .entry((glyph.render_mode, glyph.page_index))
            .or_default()
            .push(glyph);
    }
    grouped
        .into_iter()
        .map(|((render_mode, page_index), glyphs)| {
            let mut vertices = Vec::new();
            for glyph in &glyphs {
                let Some((canvas_id, color)) = item_facts.get(glyph.item_id.as_str()) else {
                    continue;
                };
                let Some(positions) =
                    projected_ui_rect_positions(presentation, canvas_id, glyph.rect)
                else {
                    continue;
                };
                let [u0, v0, u1, v1] = glyph.uv_rect;
                vertices.extend(quad_vertices(
                    positions,
                    *color,
                    [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                ));
            }
            let formal_v2 = plan.font_source_kind == "project_font_bundle_v2";
            UiProjectedFontBatch {
                render_mode,
                page_index,
                glyph_count: glyphs.len(),
                texture: if formal_v2 {
                    font_bundle_page_generation_render_handle(
                        &plan.font_atlas_id,
                        render_mode,
                        page_index,
                        plan.atlas_generation,
                    )
                } else {
                    font_atlas_render_handle(&plan.font_atlas_id)
                },
                font_render_mode: formal_v2.then_some(render_mode),
                vertices,
            }
        })
        .collect()
}

pub fn ui_projection_geometry_vertices(
    stage: &AuiCompositionStageFrame,
    presentation: &ResolvedGameViewPresentation,
) -> Vec<RenderDrawVertex> {
    let mut vertices = Vec::new();
    for item in &stage.draw_items {
        if !matches!(
            item.item_kind,
            AuiOverlayItemKind::Rect
                | AuiOverlayItemKind::ScrollbarTrack
                | AuiOverlayItemKind::ScrollbarThumb
        ) {
            continue;
        }
        let Some(color) = parse_aui_color(item.color.as_deref()) else {
            continue;
        };
        let Some(rect) = clipped_ui_rect(item.rect, item.effective_clip_rect) else {
            continue;
        };
        let Some(positions) =
            projected_ui_rect_positions(presentation, item.canvas_id.as_str(), rect)
        else {
            continue;
        };
        vertices.extend(quad_vertices(
            positions,
            color,
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        ));
    }
    vertices
}

fn projected_ui_rect_positions(
    presentation: &ResolvedGameViewPresentation,
    canvas_id: &str,
    rect: crate::aui::AuiComputedRect,
) -> Option<[[f32; 2]; 4]> {
    let target = presentation
        .reference_rect_to_target(
            canvas_id,
            GameViewRect::new(rect.x, rect.y, rect.width, rect.height),
        )
        .ok()?;
    let width = presentation.target_extent.width.max(1) as f32;
    let height = presentation.target_extent.height.max(1) as f32;
    let x0 = target.x / width * 2.0 - 1.0;
    let x1 = (target.x + target.width) / width * 2.0 - 1.0;
    let y0 = 1.0 - target.y / height * 2.0;
    let y1 = 1.0 - (target.y + target.height) / height * 2.0;
    Some([[x0, y1], [x1, y1], [x1, y0], [x0, y0]])
}

fn clipped_ui_rect(
    rect: crate::aui::AuiComputedRect,
    clip: Option<crate::aui::AuiComputedRect>,
) -> Option<crate::aui::AuiComputedRect> {
    let Some(clip) = clip else {
        return (rect.width > 0.0 && rect.height > 0.0).then_some(rect);
    };
    let x0 = rect.x.max(clip.x);
    let y0 = rect.y.max(clip.y);
    let x1 = (rect.x + rect.width).min(clip.x + clip.width);
    let y1 = (rect.y + rect.height).min(clip.y + clip.height);
    (x1 > x0 && y1 > y0).then_some(crate::aui::AuiComputedRect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

fn clipped_ui_rect_with_uv(
    rect: crate::aui::AuiComputedRect,
    clip: Option<crate::aui::AuiComputedRect>,
) -> Option<(crate::aui::AuiComputedRect, [f32; 4])> {
    let clipped = clipped_ui_rect(rect, clip)?;
    let u0 = (clipped.x - rect.x) / rect.width;
    let v0 = (clipped.y - rect.y) / rect.height;
    let u1 = (clipped.x + clipped.width - rect.x) / rect.width;
    let v1 = (clipped.y + clipped.height - rect.y) / rect.height;
    Some((clipped, [u0, v0, u1, v1]))
}

fn parse_aui_color(value: Option<&str>) -> Option<[f32; 4]> {
    let value = value?.strip_prefix('#')?;
    if value.len() != 6 && value.len() != 8 {
        return None;
    }
    let channel = |offset| u8::from_str_radix(&value[offset..offset + 2], 16).ok();
    Some([
        f32::from(channel(0)?) / 255.0,
        f32::from(channel(2)?) / 255.0,
        f32::from(channel(4)?) / 255.0,
        value
            .get(6..8)
            .map(|_| channel(6).map(|alpha| f32::from(alpha) / 255.0))
            .unwrap_or(Some(1.0))?,
    ])
}

fn quad_vertices(
    positions: [[f32; 2]; 4],
    color: [f32; 4],
    uvs: [[f32; 2]; 4],
) -> Vec<RenderDrawVertex> {
    [0usize, 1, 2, 0, 2, 3]
        .into_iter()
        .map(|index| RenderDrawVertex::new(positions[index], color, uvs[index]))
        .collect()
}

fn sprite_diagnostic_to_graph_diagnostic(
    diagnostic: &Sprite2DRenderDiagnostic,
) -> RenderGraphDiagnostic {
    match diagnostic.severity {
        Sprite2DRenderSeverity::Info => {
            RenderGraphDiagnostic::info(diagnostic.code, diagnostic.message.clone())
        }
        Sprite2DRenderSeverity::Warning => {
            RenderGraphDiagnostic::warning(diagnostic.code, diagnostic.message.clone())
        }
        Sprite2DRenderSeverity::Error => {
            RenderGraphDiagnostic::error(diagnostic.code, diagnostic.message.clone())
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct RuntimeUiCompositionStageMetrics {
    stage_count: usize,
    before_world_item_count: usize,
    screen_overlay_item_count: usize,
    modal_item_count: usize,
    before_world_pass_present: bool,
    screen_overlay_pass_present: bool,
    modal_pass_present: bool,
}

impl RuntimeUiCompositionStageMetrics {
    fn from_graph(graph: &RenderGraph) -> Self {
        let mut metrics = Self::default();
        for command in graph.passes.iter().flat_map(|pass| pass.commands.iter()) {
            let RenderPassCommand::DrawUiComposition {
                stage, item_count, ..
            } = command
            else {
                continue;
            };
            metrics.stage_count += 1;
            match stage.as_str() {
                "BeforeWorld" => {
                    metrics.before_world_item_count += *item_count;
                    metrics.before_world_pass_present = true;
                }
                "ScreenOverlay" => {
                    metrics.screen_overlay_item_count += *item_count;
                    metrics.screen_overlay_pass_present = true;
                }
                "Modal" => {
                    metrics.modal_item_count += *item_count;
                    metrics.modal_pass_present = true;
                }
                _ => {}
            }
        }
        metrics
    }
}

fn build_texture_descriptor(input: &RuntimeRendererInput<'_>) -> Option<ViewportTextureDescriptor> {
    match input.render_target.target_kind {
        RuntimeRenderTargetKind::HeadlessTexture | RuntimeRenderTargetKind::ViewportTexture => {
            Some(ViewportTextureDescriptor {
                texture_id: input.render_target.target_id.clone(),
                target_id: input.render_target.target_id.clone(),
                width: input.render_target.width,
                height: input.render_target.height,
                format: input.render_target.format.clone(),
                color_space: input.render_target.color_space.clone(),
                frame_index: input.frame_index,
                producer: "RuntimeRenderer".to_string(),
            })
        }
        RuntimeRenderTargetKind::Surface => None,
    }
}

fn render_view_kind_name(kind: &crate::render_state::RenderViewKind) -> &'static str {
    match kind {
        crate::render_state::RenderViewKind::Game => "game",
        crate::render_state::RenderViewKind::SceneView => "sceneView",
        crate::render_state::RenderViewKind::Preview => "preview",
        crate::render_state::RenderViewKind::Shadow => "shadow",
        crate::render_state::RenderViewKind::Reflection => "reflection",
    }
}

#[allow(dead_code)]
fn render_target_kind_name(kind: &RenderTargetKind) -> &'static str {
    match kind {
        RenderTargetKind::Window => "window",
        RenderTargetKind::ViewportTexture => "viewportTexture",
        RenderTargetKind::RenderTexture => "renderTexture",
        RenderTargetKind::ShadowMap => "shadowMap",
        RenderTargetKind::PreviewTexture => "previewTexture",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui::{
        AuiCanvas, AuiCompositionStage, AuiCompositionStageFrame, AuiComputedRect, AuiDocument,
        AuiDrawCommand, AuiDrawList, AuiLayoutEngine, AuiNode, AuiNodeKind, AuiOverlayDrawItem,
        AuiOverlayFrame, AuiOverlayItemKind, AuiOverlaySortKey, AuiRect, AuiRenderReport,
        AuiRendererBridge, AuiStyle, AuiTextGlyphPlan, AuiTextGlyphQuad,
    };
    use crate::components::{Renderable, Transform};
    use crate::font_bundle::FontBundleRenderMode;
    use crate::headless_rhi_backend::HeadlessRhiBackend;
    use crate::ids::{RuntimeEntityId, SourceEntityId};
    use crate::math::Vec3;
    use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
    use crate::render_state::{
        RenderProxy, RenderProxyId, RenderProxyPayload, RenderTargetKind, RenderViewId,
        RenderViewKind, SpritePayload,
    };
    use crate::rhi_command_plan::{RhiCommand, RhiDrawKind};

    fn target() -> RenderTarget {
        RenderTarget::headless_texture("surface-main", 640, 480)
    }

    fn viewport_target() -> RenderTarget {
        RenderTarget::viewport_texture("viewport-scene", 800, 450)
    }

    fn surface_target() -> RenderTarget {
        RenderTarget::surface("surface-main", 1024, 576)
    }

    fn input<'a>(frame_index: u64, scene: &'a RenderSceneState) -> RuntimeRendererInput<'a> {
        RuntimeRendererInput {
            frame_index,
            render_scene_state: scene,
            render_view_state: None,
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            quality_profile: QualityProfile::default(),
            render_target: target(),
        }
    }

    fn viewport_input<'a>(
        frame_index: u64,
        scene: &'a RenderSceneState,
    ) -> RuntimeRendererInput<'a> {
        RuntimeRendererInput {
            frame_index,
            render_scene_state: scene,
            render_view_state: None,
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            quality_profile: QualityProfile::default(),
            render_target: viewport_target(),
        }
    }

    fn surface_input<'a>(
        frame_index: u64,
        scene: &'a RenderSceneState,
    ) -> RuntimeRendererInput<'a> {
        RuntimeRendererInput {
            frame_index,
            render_scene_state: scene,
            render_view_state: None,
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            quality_profile: QualityProfile::default(),
            render_target: surface_target(),
        }
    }

    fn aui_overlay(frame_index: u64) -> crate::aui::AuiOverlayFrame {
        let draw_list = AuiDrawList {
            commands: vec![
                AuiDrawCommand::DrawRect {
                    node_id: "panel".to_string(),
                    rect: AuiComputedRect {
                        x: 10.0,
                        y: 20.0,
                        width: 200.0,
                        height: 100.0,
                    },
                    effective_clip_rect: None,
                    color: AuiStyle::color("#223344").color,
                },
                AuiDrawCommand::DrawText {
                    node_id: "label".to_string(),
                    rect: AuiComputedRect {
                        x: 20.0,
                        y: 30.0,
                        width: 160.0,
                        height: 32.0,
                    },
                    effective_clip_rect: None,
                    text: "Hello".to_string(),
                    color: Some("#ffffff".to_string()),
                    font_size: Some(18.0),
                    font: None,
                },
            ],
        };
        AuiRendererBridge::build_overlay_frame(frame_index, &draw_list)
    }

    fn aui_composition(frame_index: u64) -> crate::aui::AuiCompositionFrame {
        let mut before = AuiCanvas::screen_overlay("before", 800.0, 600.0, "before-root");
        before.composition_stage = AuiCompositionStage::BeforeWorld;
        let overlay = AuiCanvas::screen_overlay("overlay", 800.0, 600.0, "overlay-root");
        let mut modal = AuiCanvas::screen_overlay("modal", 800.0, 600.0, "modal-root");
        modal.composition_stage = AuiCompositionStage::Modal;
        let document = AuiDocument::new(
            "runtime-renderer-composition",
            vec![before, overlay, modal],
            vec![
                AuiNode::new("before-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
                AuiNode::new("overlay-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
                AuiNode::new("modal-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            ],
        );
        let layout = AuiLayoutEngine::layout(&document, frame_index);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        AuiRendererBridge::build_composition_frame(frame_index, &document, &layout, &draw_list)
    }

    fn image_overlay(frame_index: u64, asset_ids: &[&str]) -> AuiOverlayFrame {
        let draw_items = asset_ids
            .iter()
            .enumerate()
            .map(|(index, asset_id)| AuiOverlayDrawItem {
                item_id: format!("image:{index}"),
                canvas_id: "legacy-overlay".to_string(),
                composition_stage: AuiCompositionStage::ScreenOverlay,
                node_id: format!("image-{index}"),
                item_kind: AuiOverlayItemKind::Image,
                rect: AuiComputedRect {
                    x: index as f32 * 40.0,
                    y: 0.0,
                    width: 40.0,
                    height: 40.0,
                },
                effective_clip_rect: None,
                color: None,
                asset_id: Some((*asset_id).to_string()),
                text: None,
                font_size: None,
                font: None,
                sort_key: AuiOverlaySortKey {
                    canvas_layer: 0,
                    canvas_sorting_order: 0,
                    tree_order: index,
                },
            })
            .collect::<Vec<_>>();
        AuiOverlayFrame {
            frame_index,
            report: AuiRenderReport {
                draw_command_count: draw_items.len(),
                text_count: 0,
                image_count: draw_items.len(),
                effective_clip_item_count: 0,
                culled_draw_item_count: 0,
                scrollbar_visible_count: 0,
                batch_hint_count: 0,
            },
            draw_items,
            glyph_plan: None,
        }
    }

    #[test]
    fn runtime_renderer_batches_only_adjacent_images_with_same_texture() {
        let scene = scene_with_view();
        let overlay = image_overlay(30, &["texture-a", "texture-a", "texture-b", "texture-a"]);
        let mut bindings = RuntimeTextureBindingContext::default();
        for asset_id in ["texture-a", "texture-b"] {
            bindings.insert(
                asset_id,
                crate::runtime_texture::runtime_texture_render_handle(asset_id),
                "linearClamp",
            );
        }
        let mut renderer_input = input(30, &scene);
        renderer_input.aui_overlay = Some(&overlay);
        renderer_input.runtime_texture_bindings = Some(&bindings);

        let output = RuntimeRenderer::new().build(renderer_input);
        let commands = output
            .render_graph
            .passes
            .iter()
            .find(|pass| pass.pass_kind == RenderPassKind::DrawUiComposition)
            .expect("UI pass")
            .commands
            .iter()
            .filter(|command| matches!(command, RenderPassCommand::DrawUiComposition { .. }))
            .collect::<Vec<_>>();

        assert_eq!(commands.len(), 3);
        assert_eq!(output.render_frame_report.ui_image_request_count, 4);
        assert_eq!(output.render_frame_report.ui_image_resolved_count, 4);
        assert_eq!(output.render_frame_report.ui_image_missing_count, 0);
        assert_eq!(output.render_frame_report.ui_textured_batch_count, 3);
        assert!(matches!(
            commands[0],
            RenderPassCommand::DrawUiComposition { vertices, .. } if vertices.len() == 12
        ));
    }

    #[test]
    fn runtime_renderer_reports_missing_aui_image_texture_without_solid_fallback() {
        let scene = scene_with_view();
        let overlay = image_overlay(31, &["texture-missing"]);
        let mut renderer_input = input(31, &scene);
        renderer_input.aui_overlay = Some(&overlay);

        let output = RuntimeRenderer::new().build(renderer_input);

        assert_eq!(output.render_frame_report.ui_image_request_count, 1);
        assert_eq!(output.render_frame_report.ui_image_resolved_count, 0);
        assert_eq!(output.render_frame_report.ui_image_missing_count, 1);
        assert!(output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code == "aui_image.texture_not_resolved"
                    && diagnostic.message.contains("texture-missing")
            }));
        assert!(output.render_graph.passes.iter().all(|pass| {
            pass.commands.iter().all(|command| {
                !matches!(
                    command,
                    RenderPassCommand::DrawUiComposition { texture: None, .. }
                )
            })
        }));
    }

    #[test]
    fn ui_projection_image_requires_textured_quad_with_clip_adjusted_uv() {
        let mut stage = AuiCompositionStageFrame::empty(AuiCompositionStage::ScreenOverlay);
        stage.draw_items.push(AuiOverlayDrawItem {
            item_id: "image:item".to_string(),
            canvas_id: "canvas".to_string(),
            composition_stage: AuiCompositionStage::ScreenOverlay,
            node_id: "image".to_string(),
            item_kind: AuiOverlayItemKind::Image,
            rect: AuiComputedRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            effective_clip_rect: Some(AuiComputedRect {
                x: 25.0,
                y: 20.0,
                width: 50.0,
                height: 60.0,
            }),
            color: None,
            asset_id: Some("texture-ui".to_string()),
            text: None,
            font_size: None,
            font: None,
            sort_key: AuiOverlaySortKey {
                canvas_layer: 0,
                canvas_sorting_order: 0,
                tree_order: 0,
            },
        });
        let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
            session_id: "image-test".to_string(),
            target_id: "target".to_string(),
            target_extent: GameViewExtent::new(100, 100),
            display_rect: GameViewRect::new(0.0, 0.0, 100.0, 100.0),
            scale_policy: GameViewScalePolicy::Stretch,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![CanvasReferenceFact {
                canvas_id: "canvas".to_string(),
                reference_extent: GameViewExtent::new(100, 100),
            }],
        })
        .unwrap();

        let vertices = ui_projection_image_vertices(&stage.draw_items[0], &presentation)
            .expect("projected image vertices");

        assert_eq!(vertices.len(), 6, "AUI Image must not be silently skipped");
        assert_eq!(vertices[0].uv.map(|value| value.to_f32()), [0.25, 0.8]);
        assert_eq!(vertices[2].uv.map(|value| value.to_f32()), [0.75, 0.2]);
    }

    #[test]
    fn ui_projection_font_batches_v2_glyphs_by_stable_mode_and_page() {
        let mut overlay = aui_overlay(21);
        let text_item = overlay
            .draw_items
            .iter()
            .find(|item| item.text.is_some())
            .expect("text draw item")
            .clone();
        let glyph = |render_mode, page_index, x| AuiTextGlyphQuad {
            item_id: text_item.item_id.clone(),
            node_id: text_item.node_id.clone(),
            codepoint: 0x4e2d + page_index,
            glyph_id: format!("glyph-{page_index}"),
            rect: AuiComputedRect {
                x,
                y: 30.0,
                width: 12.0,
                height: 18.0,
            },
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            page_index,
            render_mode,
            clipped: false,
        };
        overlay.glyph_plan = Some(AuiTextGlyphPlan {
            font_atlas_id: "tower-defense-ui".to_string(),
            font_source_kind: "project_font_bundle_v2".to_string(),
            font_asset_id: "tower-defense-ui".to_string(),
            font_asset_status: "ready".to_string(),
            fallback_used: false,
            requested_glyph_count: 3,
            rendered_glyph_count: 3,
            unsupported_glyph_count: 0,
            clipped_glyph_count: 0,
            atlas_width: 256,
            atlas_height: 256,
            atlas_generation: 7,
            glyph_plan_hash: "sha256:test".to_string(),
            quads: vec![
                glyph(FontBundleRenderMode::MsdfRgba8, 2, 50.0),
                glyph(FontBundleRenderMode::BitmapR8, 1, 35.0),
                glyph(FontBundleRenderMode::BitmapR8, 0, 20.0),
            ],
        });
        let composition = crate::aui::AuiCompositionFrame::from_overlay_frame(&overlay);
        let stage = composition
            .stage(AuiCompositionStage::ScreenOverlay)
            .expect("screen overlay");

        let presentation = resolve_ui_presentation(
            &composition,
            stage,
            [800.0, 600.0],
            "font-test",
            GameViewScalePolicy::Stretch,
        )
        .unwrap();
        let batches = ui_projection_font_batches(&composition, stage, &presentation);

        assert_eq!(batches.len(), 3);
        assert_eq!(
            batches
                .iter()
                .map(|batch| (batch.render_mode, batch.page_index, batch.glyph_count))
                .collect::<Vec<_>>(),
            vec![
                (FontBundleRenderMode::BitmapR8, 0, 1),
                (FontBundleRenderMode::BitmapR8, 1, 1),
                (FontBundleRenderMode::MsdfRgba8, 2, 1),
            ]
        );
        assert!(batches.iter().all(|batch| batch.font_render_mode.is_some()));
        assert!(batches.iter().all(|batch| batch.vertices.len() == 6));
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.texture)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            3
        );
    }

    fn scene_with_view() -> RenderSceneState {
        let mut scene = RenderSceneState::new();
        scene.register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::ViewportTexture,
        ));
        scene
    }

    fn add_proxy(scene: &mut RenderSceneState, mesh_ref: Option<&str>) {
        let proxy = RenderProxy::new(
            RenderProxyId(0),
            RuntimeEntityId::new(1, 0),
            SourceEntityId::from("entity-a"),
            Transform {
                local_position: Vec3::ZERO,
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
            Renderable {
                mesh_ref: mesh_ref.map(str::to_string),
                material_ref: Some("material-a".to_string()),
                visible: true,
                layer: "default".to_string(),
            },
        );
        scene.insert_proxy(proxy);
    }

    fn add_sprite_proxy(scene: &mut RenderSceneState, sprite_ref: &str) {
        let mut proxy = RenderProxy::new(
            RenderProxyId(0),
            RuntimeEntityId::new(2, 0),
            SourceEntityId::from("entity-sprite"),
            Transform {
                local_position: Vec3::ZERO,
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
            Renderable {
                mesh_ref: None,
                material_ref: Some("material-sprite".to_string()),
                visible: true,
                layer: "default".to_string(),
            },
        );
        proxy.payload = RenderProxyPayload::Sprite(SpritePayload {
            sprite_ref: Some(sprite_ref.to_string()),
            material_ref: Some("material-sprite".to_string()),
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            sorting_layer: 0,
            order_in_layer: 0,
            sort_z: 0.0,
        });
        scene.insert_proxy(proxy);
    }

    #[test]
    fn empty_scene_generates_clear_and_present_graph() {
        let scene = scene_with_view();

        let output = RuntimeRenderer::new().build(input(1, &scene));

        assert_eq!(output.render_graph.passes.len(), 2);
        assert_eq!(output.render_frame_report.draw_item_count, 0);
        assert!(!output.rhi_command_plan.has_errors());
    }

    #[test]
    fn scene_with_mesh_generates_draw_pass() {
        let mut scene = scene_with_view();
        add_proxy(&mut scene, Some("mesh-a"));

        let output = RuntimeRenderer::new().build(input(1, &scene));

        assert_eq!(output.render_frame_report.draw_item_count, 1);
        assert!(output
            .render_graph
            .passes
            .iter()
            .any(|pass| pass.pass_kind == RenderPassKind::DrawMeshBasic));
        assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::MeshBasic,
                    payload: crate::rhi_command_plan::RhiDrawPayload::MeshBasic { mesh_ref, .. },
                    ..
                } if mesh_ref == "mesh-a"
            )
        }));
    }

    #[test]
    fn missing_mesh_falls_back_to_test_geometry() {
        let mut scene = scene_with_view();
        add_proxy(&mut scene, None);

        let output = RuntimeRenderer::new().build(input(1, &scene));

        assert_eq!(output.render_frame_report.fallback_count, 1);
        assert!(output
            .render_graph
            .passes
            .iter()
            .any(|pass| pass.pass_kind == RenderPassKind::DrawTestGeometry));
    }

    #[test]
    fn scene_with_sprite_generates_sprite_draw_pass() {
        let mut scene = scene_with_view();
        add_sprite_proxy(&mut scene, "sprite-a");

        let output = RuntimeRenderer::new().build(input(1, &scene));

        assert_eq!(output.render_frame_report.draw_item_count, 1);
        assert!(output
            .render_graph
            .passes
            .iter()
            .any(|pass| pass.pass_kind == RenderPassKind::DrawSpriteTextured));
        assert!(output.render_graph.passes.iter().any(|pass| {
            pass.commands.iter().any(|command| {
                matches!(
                    command,
                    RenderPassCommand::DrawSpriteTextured { sprite_ref, .. }
                        if sprite_ref == "sprite-a"
                )
            })
        }));
        assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::SpriteTextured,
                    payload: crate::rhi_command_plan::RhiDrawPayload::SpriteTextured { sprite_ref, binding: Some(_), .. },
                    ..
                } if sprite_ref == "sprite-a"
            )
        }));
        assert!(output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_ready"));
        assert!(output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_binding_fallback"));
    }

    #[test]
    fn sprite_vertices_use_scene_view_orthographic_projection() {
        let mut scene = RenderSceneState::new();
        let mut view = RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::ViewportTexture,
        );
        view.projection_matrix = orthographic_2d_projection(5.4, 9.6);
        scene.register_view(view);
        add_sprite_proxy(&mut scene, "sprite-a");

        let output = RuntimeRenderer::new().build(input(1, &scene));
        let vertices = output
            .render_graph
            .passes
            .iter()
            .flat_map(|pass| pass.commands.iter())
            .find_map(|command| match command {
                RenderPassCommand::DrawSpriteTextured { vertices, .. } => Some(vertices),
                _ => None,
            })
            .expect("Sprite2D vertices");

        assert!((vertices[0].position[0].to_f32() + 1.0 / 5.4).abs() < 0.0001);
        assert!((vertices[0].position[1].to_f32() + 1.0 / 9.6).abs() < 0.0001);
    }

    #[test]
    fn sprite_vertices_keep_valid_identity_projection_from_scene_camera() {
        let mut scene = RenderSceneState::new();
        let mut view = RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::ViewportTexture,
        );
        view.source_entity_id = Some(SourceEntityId::from("camera-unit-ortho"));
        scene.register_view(view);
        add_sprite_proxy(&mut scene, "sprite-a");

        let output = RuntimeRenderer::new().build(input(1, &scene));
        let first = output
            .render_graph
            .passes
            .iter()
            .flat_map(|pass| pass.commands.iter())
            .find_map(|command| match command {
                RenderPassCommand::DrawSpriteTextured { vertices, .. } => vertices.first(),
                _ => None,
            })
            .expect("Sprite2D first vertex");

        assert_eq!(first.position.map(|value| value.to_f32()), [-1.0, -1.0]);
    }

    #[test]
    fn scene_with_sprite_uses_prepared_texture_binding_when_supplied() {
        let mut scene = scene_with_view();
        add_sprite_proxy(&mut scene, "sprite-a");
        let mut bindings = Sprite2DTextureBindingContext::new();
        bindings.insert_texture_handle(
            "sprite-a",
            RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 9,
                generation: 2,
            },
            "linearClamp",
        );
        let mut input = input(1, &scene);
        input.sprite_texture_bindings = Some(&bindings);

        let output = RuntimeRenderer::new().build(input);

        assert!(output.render_graph.passes.iter().any(|pass| {
            pass.commands.iter().any(|command| {
                matches!(
                    command,
                    RenderPassCommand::DrawSpriteTextured {
                        texture: Some(RenderResourceHandle {
                            index: 9,
                            generation: 2,
                            ..
                        }),
                        fallback_used: false,
                        ..
                    }
                )
            })
        }));
        assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::SpriteTextured,
                    payload: crate::rhi_command_plan::RhiDrawPayload::SpriteTextured {
                        texture: Some(RenderResourceHandle {
                            index: 9,
                            generation: 2,
                            ..
                        }),
                        binding: Some(_),
                        ..
                    },
                    ..
                }
            )
        }));
        assert!(output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_texture_binding_ready"));
        assert!(!output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_binding_fallback"));
    }

    #[test]
    fn runtime_renderer_executes_mesh_through_engine_rhi_backend() {
        let mut scene = scene_with_view();
        add_proxy(&mut scene, Some("mesh-a"));
        let mut backend = HeadlessRhiBackend::new();

        let output = RuntimeRenderer::new().render_with_rhi_backend(input(1, &scene), &mut backend);
        let backend_report = output.rhi_backend_report.expect("backend report");

        assert_eq!(backend_report.backend_kind, "headless-rhi");
        assert_eq!(backend_report.draw_count, 1);
        assert_eq!(backend_report.present_count, 1);
        assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::MeshBasic,
                    payload: crate::rhi_command_plan::RhiDrawPayload::MeshBasic { mesh_ref, .. },
                    ..
                } if mesh_ref == "mesh-a"
            )
        }));
    }

    #[test]
    fn runtime_renderer_executes_sprite_through_engine_rhi_backend() {
        let mut scene = scene_with_view();
        add_sprite_proxy(&mut scene, "sprite-a");
        let mut backend = HeadlessRhiBackend::new();

        let output = RuntimeRenderer::new().render_with_rhi_backend(input(1, &scene), &mut backend);
        let backend_report = output.rhi_backend_report.expect("backend report");

        assert_eq!(backend_report.backend_kind, "headless-rhi");
        assert_eq!(backend_report.draw_count, 1);
        assert_eq!(backend_report.present_count, 1);
        assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::SpriteTextured,
                    vertex_count: 6,
                    payload: crate::rhi_command_plan::RhiDrawPayload::SpriteTextured { sprite_ref, binding: Some(_), .. },
                    ..
                } if sprite_ref == "sprite-a"
            )
        }));
    }

    #[test]
    fn runtime_renderer_does_not_panic_with_sprite_draw_item() {
        let mut scene = scene_with_view();
        add_sprite_proxy(&mut scene, "sprite-a");

        let output = RuntimeRenderer::new().render_headless(input(1, &scene));

        assert_eq!(output.render_frame_report.fallback_count, 0);
        assert_eq!(
            output
                .rhi_backend_report
                .as_ref()
                .expect("backend report")
                .draw_count,
            1
        );
    }

    #[test]
    fn headless_end_to_end_outputs_backend_report() {
        let mut scene = scene_with_view();
        add_proxy(&mut scene, Some("mesh-a"));

        let output = RuntimeRenderer::new().render_headless(input(1, &scene));
        let backend_report = output.rhi_backend_report.expect("backend report");

        assert_eq!(backend_report.draw_count, 1);
        assert_eq!(backend_report.present_count, 1);
        assert_eq!(output.render_graph_report.error_count, 0);
    }

    #[test]
    fn viewport_texture_target_outputs_descriptor() {
        let scene = scene_with_view();

        let output = RuntimeRenderer::new().build(viewport_input(7, &scene));
        let descriptor = output
            .texture_descriptor
            .as_ref()
            .expect("viewport texture descriptor");

        assert_eq!(descriptor.target_id, "viewport-scene");
        assert_eq!(descriptor.texture_id, "viewport-scene");
        assert_eq!(descriptor.frame_index, 7);
        assert_eq!(descriptor.width, 800);
        assert_eq!(descriptor.height, 450);
        assert_eq!(
            output.target_summary.texture_descriptor.as_ref(),
            Some(descriptor)
        );
    }

    #[test]
    fn surface_target_does_not_emit_viewport_texture_descriptor() {
        let scene = scene_with_view();

        let output = RuntimeRenderer::new().render_headless(surface_input(9, &scene));

        assert_eq!(
            output.target_summary.target_kind,
            RuntimeRenderTargetKind::Surface
        );
        assert_eq!(output.target_summary.target_id, "surface-main");
        assert!(output.texture_descriptor.is_none());
        assert_eq!(
            output
                .rhi_backend_report
                .as_ref()
                .expect("backend report")
                .present_count,
            1
        );
    }

    #[test]
    fn runtime_renderer_inserts_aui_overlay_pass_before_present() {
        let scene = scene_with_view();
        let overlay = aui_overlay(12);
        let input = RuntimeRendererInput {
            frame_index: 12,
            render_scene_state: &scene,
            render_view_state: None,
            aui_overlay: Some(&overlay),
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            quality_profile: QualityProfile::default(),
            render_target: target(),
        };

        let output = RuntimeRenderer::new().build(input);

        let ui_pass_index = output
            .render_graph
            .passes
            .iter()
            .position(|pass| pass.pass_kind == RenderPassKind::DrawUiComposition)
            .expect("ui composition pass");
        let present_index = output
            .render_graph
            .passes
            .iter()
            .position(|pass| pass.pass_kind == RenderPassKind::Present)
            .expect("present pass");
        assert!(ui_pass_index < present_index);
        assert_eq!(output.render_frame_report.draw_item_count, 2);
        assert_eq!(output.render_frame_report.ui_composition_stage_count, 1);
        assert_eq!(output.render_frame_report.ui_screen_overlay_item_count, 2);
        assert!(output.render_frame_report.ui_screen_overlay_pass_present);
        assert!(output.render_frame_report.ui_before_world_skipped);
        assert!(output.render_frame_report.ui_modal_skipped);
        assert!(matches!(
            output.render_graph.passes[ui_pass_index].commands[0],
            RenderPassCommand::DrawUiComposition {
                ref stage,
                item_count: 2,
                text_count: 1,
                image_count: 0,
                ..
            } if stage == "ScreenOverlay"
        ));
        let RenderPassCommand::DrawUiComposition { vertices, .. } =
            &output.render_graph.passes[ui_pass_index].commands[0]
        else {
            panic!("expected UI composition geometry command");
        };
        assert_eq!(vertices.len(), 6);
        assert_eq!(
            vertices[0].color.map(|channel| channel.to_f32()),
            [
                0x22 as f32 / 255.0,
                0x33 as f32 / 255.0,
                0x44 as f32 / 255.0,
                1.0
            ]
        );
        assert!(vertices
            .iter()
            .any(|vertex| vertex.position[0].to_f32() < 0.0));
        assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::UiComposition,
                    payload: crate::rhi_command_plan::RhiDrawPayload::UiComposition {
                        stage,
                        item_count: 2,
                        text_count: 1,
                        image_count: 0,
                        ..
                    },
                    ..
                } if stage == "ScreenOverlay"
            )
        }));
    }

    #[test]
    fn runtime_renderer_orders_multi_stage_aui_composition_around_world() {
        let mut scene = scene_with_view();
        add_sprite_proxy(&mut scene, "sprite-a");
        let composition = aui_composition(13);
        let input = RuntimeRendererInput {
            frame_index: 13,
            render_scene_state: &scene,
            render_view_state: None,
            aui_overlay: None,
            aui_composition: Some(&composition),
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            quality_profile: QualityProfile::default(),
            render_target: target(),
        };

        let output = RuntimeRenderer::new().build(input);
        let pass_ids = output
            .render_graph
            .passes
            .iter()
            .map(|pass| pass.pass_id.as_str())
            .collect::<Vec<_>>();

        let before_index = pass_ids
            .iter()
            .position(|pass_id| *pass_id == "draw-aui-before-world")
            .expect("before world pass");
        let sprite_index = pass_ids
            .iter()
            .position(|pass_id| pass_id.starts_with("draw-sprite2d-"))
            .expect("sprite pass");
        let overlay_index = pass_ids
            .iter()
            .position(|pass_id| *pass_id == "draw-aui-screen-overlay")
            .expect("screen overlay pass");
        let modal_index = pass_ids
            .iter()
            .position(|pass_id| *pass_id == "draw-aui-modal")
            .expect("modal pass");
        let present_index = pass_ids
            .iter()
            .position(|pass_id| *pass_id == "present-main")
            .expect("present pass");

        assert!(before_index < sprite_index);
        assert!(sprite_index < overlay_index);
        assert!(overlay_index < modal_index);
        assert!(modal_index < present_index);
        assert_eq!(output.render_frame_report.ui_composition_stage_count, 3);
        assert_eq!(output.render_frame_report.ui_before_world_item_count, 1);
        assert_eq!(output.render_frame_report.ui_screen_overlay_item_count, 1);
        assert_eq!(output.render_frame_report.ui_modal_item_count, 1);
        assert!(output.render_frame_report.ui_before_world_pass_present);
        assert!(output.render_frame_report.ui_screen_overlay_pass_present);
        assert!(output.render_frame_report.ui_modal_pass_present);
    }

    #[test]
    fn runtime_renderer_without_aui_overlay_keeps_existing_passes() {
        let scene = scene_with_view();

        let output = RuntimeRenderer::new().build(input(1, &scene));

        assert_eq!(output.render_graph.passes.len(), 2);
        assert!(!output
            .render_graph
            .passes
            .iter()
            .any(|pass| pass.pass_kind == RenderPassKind::DrawUiComposition));
        assert_eq!(output.render_frame_report.ui_composition_stage_count, 0);
        assert!(output.render_frame_report.ui_before_world_skipped);
        assert!(output.render_frame_report.ui_screen_overlay_skipped);
        assert!(output.render_frame_report.ui_modal_skipped);
    }

    #[test]
    fn sprite_missing_ref_is_reported_without_test_geometry_fallback() {
        let mut scene = scene_with_view();
        let mut proxy = RenderProxy::new(
            RenderProxyId(0),
            RuntimeEntityId::new(3, 0),
            SourceEntityId::from("entity-sprite-missing"),
            Transform {
                local_position: Vec3::ZERO,
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
            Renderable {
                mesh_ref: None,
                material_ref: Some("material-sprite".to_string()),
                visible: true,
                layer: "sprite2d".to_string(),
            },
        );
        proxy.payload = RenderProxyPayload::Sprite(SpritePayload::default());
        scene.insert_proxy(proxy);

        let output = RuntimeRenderer::new().build(input(1, &scene));

        assert_eq!(output.render_frame_report.draw_item_count, 0);
        assert_eq!(output.render_frame_report.fallback_count, 0);
        assert!(!output
            .render_graph
            .passes
            .iter()
            .any(|pass| pass.pass_kind == RenderPassKind::DrawTestGeometry));
        assert!(output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code == "sprite_missing_ref"
                    && diagnostic.severity == RuntimeRendererDiagnosticSeverity::Warning
            }));
    }

    #[test]
    fn sprite_layer_mismatch_is_reported() {
        let mut scene = scene_with_view();
        let mut view = RenderViewState::new(
            RenderViewId(2),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        );
        view.layer_mask = Some("gameplay".to_string());
        scene.register_view(view.clone());
        add_sprite_proxy(&mut scene, "sprite-a");

        let mut input = input(1, &scene);
        input.render_view_state = Some(&view);
        let output = RuntimeRenderer::new().build(input);

        assert_eq!(output.render_frame_report.draw_item_count, 0);
        assert!(output
            .render_frame_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "sprite_layer_mismatch"));
    }
}
