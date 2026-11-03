#[path = "runtime_renderer/ui_projection.rs"]
mod ui_projection;
use ui_projection::*;
pub use ui_projection::{
    ui_projection_font_batches, ui_projection_geometry_vertices, UiProjectedFontBatch,
    UiProjectionStats,
};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, Mutex};

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
    RenderPassCommand, RenderPassKind, RenderResource, UiVertexStream,
};
use crate::render_graph_report::RenderGraphReport;
use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
use crate::render_state::{RenderPayloadKind, RenderSceneState, RenderTargetKind, RenderViewState};
use crate::renderer_feature_builder::RendererFeatureBuilder;
use crate::rhi_command_plan::RhiCommandPlan;
use crate::runtime_texture::RuntimeTextureBindingContext;
use crate::sprite2d_render_pipeline::{
    Sprite2DDrawPlan, Sprite2DRenderDiagnostic, Sprite2DRenderPipeline, Sprite2DRenderSeverity,
    Sprite2DTextureBindingContext,
};

pub const DEFAULT_2D_ORTHOGRAPHIC_HALF_HEIGHT: f32 = 7.5;

// Disposal remains in this call; this is not a retirement queue or GPU proof.
fn dispose_graph_while_building<T: Send, R>(
    retired: T,
    parallel: bool,
    build: impl FnOnce() -> R,
) -> R {
    if !parallel {
        drop(retired);
        return build();
    }
    std::thread::scope(|scope| {
        // On spawn failure std drops the captured old value on this caller.
        let worker = std::thread::Builder::new()
            .name("render-graph-cpu-dispose".into())
            .spawn_scoped(scope, move || drop(retired));
        let result = build();
        if let Ok(worker) = worker {
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
        }
        result
    })
}

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
    pub game_view_presentation: Option<&'a ResolvedGameViewPresentation>,
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
    ui_projection: Arc<Mutex<UiProjectionCache>>,
    graph_schedule: Arc<Mutex<crate::render_graph::GraphScheduleCache>>,
    retained_geometry_plan: Arc<Mutex<Option<RetainedGeometryPlan>>>,
}

/// A single successful immutable compilation, never a resource-residency proof.
#[derive(Debug, Clone)]
struct RetainedGeometryPlan {
    geometry: Arc<crate::render_geometry::GeometryFrame>,
    base_graph: RenderGraph,
    target: RenderTarget,
    quality: QualityProfile,
    graph: RenderGraph,
    plan: RhiCommandPlan,
}

impl RuntimeRenderer {
    /// Diagnostic reference mode, also useful for exact GPU/CPU pixel comparisons.
    pub fn set_retained_graphics_enabled(&self, enabled: bool) {
        let mut cache = self.ui_projection.lock().unwrap_or_else(|p| p.into_inner());
        if cache.force_cpu_graphics == enabled {
            cache.force_cpu_graphics = !enabled;
            cache.stages.clear();
            cache.stage_variants.clear();
        }
    }

    pub fn set_ui_measurement_enabled(&self, enabled: bool) {
        self.ui_projection
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .measurement_enabled = enabled;
    }
    pub fn new() -> Self {
        let renderer = Self {
            feature_builder: RendererFeatureBuilder::new(),
            sprite2d_pipeline: Sprite2DRenderPipeline::new(),
            ui_projection: Default::default(),
            graph_schedule: Default::default(),
            retained_geometry_plan: Default::default(),
        };
        // Explicit process-local diagnostic reference; normal Player uses retained Graphics.
        if std::env::var_os("AIFE_AUI_CPU_GRAPHICS_REFERENCE").is_some_and(|v| v == "1") {
            renderer.set_retained_graphics_enabled(false);
        }
        renderer
    }

    pub fn build(&self, input: RuntimeRendererInput<'_>) -> RuntimeRendererOutput {
        self.build_with_geometry(input, None)
    }

    pub fn ui_projection_stats(&self) -> UiProjectionStats {
        self.ui_projection
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .stats
    }

    /// Consumes prepared Projection products through the existing graph/report path.
    pub(crate) fn build_with_retained_geometry(
        &self,
        input: RuntimeRendererInput<'_>,
        geometry: Option<&Arc<crate::render_geometry::GeometryFrame>>,
    ) -> RuntimeRendererOutput {
        self.build_geometry(input, geometry.map(Arc::as_ref), geometry)
    }

    /// Borrowed geometry is always rebuilt and validated.
    pub fn build_with_geometry(
        &self,
        input: RuntimeRendererInput<'_>,
        geometry: Option<&crate::render_geometry::GeometryFrame>,
    ) -> RuntimeRendererOutput {
        self.build_geometry(input, geometry, None)
    }

    fn build_geometry(
        &self,
        input: RuntimeRendererInput<'_>,
        geometry: Option<&crate::render_geometry::GeometryFrame>,
        retained: Option<&Arc<crate::render_geometry::GeometryFrame>>,
    ) -> RuntimeRendererOutput {
        let mut render_graph = self.build_graph(&input);
        let mut cached = self
            .retained_geometry_plan
            .lock()
            .expect("retained graph poisoned");
        let base_graph = retained.map(|_| {
            let mut base = render_graph.clone();
            base.frame_index = 0;
            base.graph_id.clear();
            base
        });
        let hit = cached.as_ref().filter(|old| {
            retained.is_some_and(|frame| Arc::ptr_eq(frame, &old.geometry))
                && base_graph.as_ref() == Some(&old.base_graph)
                && old.target == input.render_target
                && old.quality == input.quality_profile
        });
        let rhi_command_plan = if let Some(old) = hit {
            let graph_id = render_graph.graph_id.clone();
            render_graph = old.graph.clone();
            render_graph.frame_index = input.frame_index;
            render_graph.graph_id = graph_id.clone();
            let mut plan = old.plan.clone();
            plan.frame_index = input.frame_index;
            plan.graph_id = graph_id;
            plan
        } else {
            // Release the previous entry even on failure or an unretained call.
            let retired = cached.take();
            let parallel = retired.as_ref().is_some_and(|old| {
                old.geometry
                    .views
                    .iter()
                    .map(|view| view.draws.len())
                    .sum::<usize>()
                    >= 1024
            }) && std::thread::available_parallelism().is_ok_and(|n| n.get() > 1);
            let plan = dispose_graph_while_building(retired, parallel, || {
                if let Some(geometry) = geometry {
                    geometry.append_to_graph(&mut render_graph);
                }
                crate::rhi_command_plan::compile_render_graph_with_cache(
                    &render_graph,
                    &mut self.graph_schedule.lock().expect("graph compiler poisoned"),
                )
            });
            // Join before cache publication, including destructor failure.
            if !plan.has_errors() {
                if let (Some(geometry), Some(base_graph)) = (retained, base_graph) {
                    *cached = Some(RetainedGeometryPlan {
                        geometry: Arc::clone(geometry),
                        base_graph,
                        target: input.render_target.clone(),
                        quality: input.quality_profile.clone(),
                        graph: render_graph.clone(),
                        plan: plan.clone(),
                    });
                }
            }
            plan
        };
        drop(cached);

        let render_graph_report = RenderGraphReport::from_diagnostics(
            &render_graph,
            rhi_command_plan.diagnostics.clone(),
        );
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
                    RenderPassCommand::DrawGeometryBatch { batch } => batch.instance_count(),
                    RenderPassCommand::DrawGeometry { .. }
                    | RenderPassCommand::DrawMeshBasic { .. }
                    | RenderPassCommand::DrawSpriteBasic { .. }
                    | RenderPassCommand::DrawSpriteTextured { .. }
                    | RenderPassCommand::DrawTestGeometry { .. } => 1,
                    RenderPassCommand::DrawUiOverlay { item_count, .. } => *item_count,
                    RenderPassCommand::DrawUiComposition { item_count, .. }
                    | RenderPassCommand::DrawUiRetained { item_count, .. } => *item_count,
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
        self.render_headless_with_geometry(input, None)
    }

    pub fn render_headless_with_geometry(
        &self,
        input: RuntimeRendererInput<'_>,
        geometry: Option<&crate::render_geometry::GeometryFrame>,
    ) -> RuntimeRendererOutput {
        let mut output = self.build_with_geometry(input, geometry);
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
        // Retain only stages present in this frame, including when UI is detached.
        {
            let mut cache = self.ui_projection.lock().unwrap_or_else(|p| p.into_inner());
            let active = |stage: &AuiCompositionStage| {
                input
                    .aui_composition
                    .and_then(|c| c.stage(*stage))
                    .is_some_and(|s| !s.is_empty())
            };
            cache.stages.retain(|stage, _| active(stage));
            cache.stage_variants.retain(|stage, _| active(stage));
            cache
                .fallback_presentations
                .retain(|stage, _| active(stage));
        }
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
            }]
            .into(),
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
            input.game_view_presentation,
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
                commands: vec![command].into(),
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
                    texture: draw_plan.binding.as_ref().and_then(|binding| {
                        binding
                            .texture("baseColorTexture")
                            .map(|handle| handle.raw())
                    }),
                    binding: draw_plan.binding.clone(),
                    fallback_used: draw_plan.fallback_used,
                    vertices: sprite_draw_vertices(
                        draw_plan,
                        view,
                        input.render_target.width,
                        input.render_target.height,
                    ),
                }]
                .into(),
                debug_source: Some(format!("Sprite2DRenderPipeline.{}", draw_plan.proxy_id)),
            });
        }

        for message in &input.render_scene_state.particle_diagnostics {
            graph.diagnostics.push(RenderGraphDiagnostic::error(
                "particle_effect.projection_failed",
                message.clone(),
            ));
        }
        let sources = &input.render_scene_state.particle_sources;
        if input.render_scene_state.particle_projection_active || !sources.is_empty() {
            let mut particle_commands = vec![RenderPassCommand::ReconcileParticles {
                target: target_id.clone(),
                sources: sources.clone(),
            }];
            for source in sources {
                particle_commands.push(RenderPassCommand::SimulateParticles {
                    target: target_id.clone(),
                    instance: source.instance,
                    step: source.step.clone(),
                });
                let mut camera =
                    crate::particle_render_contract::ParticleRenderView::from_scene_view(
                        view,
                        input.render_target.width,
                        input.render_target.height,
                        source.step.origin,
                    );
                camera.emitter_affine = source.step.emitter_affine;
                for emitter in 0..source.textures.len() {
                    particle_commands.push(RenderPassCommand::DrawParticles {
                        target: target_id.clone(),
                        instance: source.instance,
                        emitter: emitter as u32,
                        view: camera.clone(),
                        texture: source.textures[emitter],
                        mesh: source.meshes[emitter],
                    });
                }
            }
            graph.passes.push(RenderPass {
                pass_id: "particle-effects".into(),
                pass_name: "Particle Effects".into(),
                pass_kind: RenderPassKind::DrawParticles,
                view_id: view_id.clone(),
                reads: Vec::new(),
                writes: vec![target_id.clone()],
                color_targets: vec![target_id.clone()],
                depth_target: None,
                commands: particle_commands.into(),
                debug_source: Some("RenderProjectionAdapter<ParticleEffect>".into()),
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
            input.game_view_presentation,
        );
        self.push_ui_composition_pass(
            &mut graph,
            aui_composition,
            AuiCompositionStage::Modal,
            &target_id,
            &view_id,
            input.render_target.presentation_scale_policy,
            input.runtime_texture_bindings,
            input.game_view_presentation,
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
            commands: vec![RenderPassCommand::Present { target: target_id }].into(),
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
        shared_presentation: Option<&ResolvedGameViewPresentation>,
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
        let resolved_presentation;
        let presentation = if let Some(presentation) = shared_presentation {
            presentation
        } else {
            resolved_presentation = match self
                .ui_projection
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .resolve_presentation(
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
            &resolved_presentation
        };
        let projected = self
            .ui_projection
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .project(composition, stage_frame, presentation, texture_bindings);
        for diagnostic in projected.diagnostics {
            graph.diagnostics.push(diagnostic);
        }
        let mut commands = Vec::new();
        for (index, batch) in projected.batches.into_iter().enumerate() {
            let owns_stage_metrics = index == 0;
            if !batch.retained.is_empty() {
                commands.push(RenderPassCommand::DrawUiRetained {
                    target: target_id.to_string(),
                    stage: stage.as_str().to_string(),
                    item_count: if owns_stage_metrics {
                        stage_frame.item_count
                    } else {
                        0
                    },
                    draws: batch.retained,
                });
                continue;
            }
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
                vertices: Default::default(),
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
            commands: commands.into(),
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

fn uses_font_bundle_pages(font_source_kind: &str) -> bool {
    matches!(
        font_source_kind,
        "project_font_bundle_v2" | "project_font_bundle_v2_paragraph"
    )
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
#[path = "runtime_renderer/tests.rs"]
mod tests;
