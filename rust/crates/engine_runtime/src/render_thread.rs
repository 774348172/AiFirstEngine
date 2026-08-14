use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::aui::{AuiCompositionFrame, AuiOverlayFrame};
use crate::gpu_texture_lifetime::{
    GpuTextureDescriptor, GpuTextureLifetimeReport, RuntimeGpuTextureRegistry,
};
use crate::render_command::RenderFrameReport;
use crate::render_resource::{
    RenderResourceLifetimeReport, RenderResourceManager, RenderResourceReleaseRequest,
    RenderResourceRequest,
};
use crate::render_state::{RenderSceneState, RenderViewId};
use crate::runtime_renderer::{
    QualityProfile, RenderTarget, RuntimeRenderFrameReport, RuntimeRenderer, RuntimeRendererInput,
    RuntimeRendererOutput,
};
use crate::runtime_texture::RuntimeTextureBindingContext;
use crate::sprite2d_render_pipeline::Sprite2DTextureBindingContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderThreadConfig {
    pub thread_mode: RenderThreadMode,
    pub backend_kind: String,
}

impl Default for RenderThreadConfig {
    fn default() -> Self {
        Self {
            thread_mode: RenderThreadMode::InlineDeterministic,
            backend_kind: "headless-rhi".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderThreadMode {
    InlineDeterministic,
    DedicatedThread,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderThreadCommandSummary {
    pub raw_command_count: usize,
    pub applied_command_count: usize,
    pub diagnostic_count: usize,
}

impl RenderThreadCommandSummary {
    pub fn from_render_frame_report(report: &RenderFrameReport) -> Self {
        Self {
            raw_command_count: report.counters.raw_command_count,
            applied_command_count: report.counters.applied_command_count,
            diagnostic_count: report.render_events.len(),
        }
    }
}

pub struct RenderThreadFrameInput<'a> {
    pub frame_index: u64,
    pub render_scene_state: &'a RenderSceneState,
    pub render_frame_report: Option<&'a RenderFrameReport>,
    pub resource_requests: Vec<RenderResourceRequest>,
    pub resource_release_requests: Vec<RenderResourceReleaseRequest>,
    pub aui_overlay: Option<&'a AuiOverlayFrame>,
    pub aui_composition: Option<&'a AuiCompositionFrame>,
    pub sprite_texture_bindings: Option<&'a Sprite2DTextureBindingContext>,
    pub runtime_texture_bindings: Option<&'a RuntimeTextureBindingContext>,
    pub view_id: Option<RenderViewId>,
    pub quality_profile: QualityProfile,
    pub render_target: RenderTarget,
}

#[derive(Debug, Clone)]
pub struct RenderFramePacket {
    pub frame_index: u64,
    pub render_scene_state: RenderSceneState,
    pub render_frame_report: Option<RenderFrameReport>,
    pub resource_requests: Vec<RenderResourceRequest>,
    pub resource_release_requests: Vec<RenderResourceReleaseRequest>,
    pub aui_overlay: Option<AuiOverlayFrame>,
    pub aui_composition: Option<AuiCompositionFrame>,
    pub sprite_texture_bindings: Option<Sprite2DTextureBindingContext>,
    pub runtime_texture_bindings: Option<RuntimeTextureBindingContext>,
    pub view_id: Option<RenderViewId>,
    pub quality_profile: QualityProfile,
    pub render_target: RenderTarget,
}

impl RenderFramePacket {
    pub fn from_input(input: RenderThreadFrameInput<'_>) -> Self {
        Self {
            frame_index: input.frame_index,
            render_scene_state: input.render_scene_state.clone(),
            render_frame_report: input.render_frame_report.cloned(),
            resource_requests: input.resource_requests,
            resource_release_requests: input.resource_release_requests,
            aui_overlay: input.aui_overlay.cloned(),
            aui_composition: input.aui_composition.cloned(),
            sprite_texture_bindings: input.sprite_texture_bindings.cloned(),
            runtime_texture_bindings: input.runtime_texture_bindings.cloned(),
            view_id: input.view_id,
            quality_profile: input.quality_profile,
            render_target: input.render_target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderThreadDiagnostic {
    pub severity: RenderThreadDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub layer: String,
}

impl RenderThreadDiagnostic {
    pub fn info(
        code: impl Into<String>,
        message: impl Into<String>,
        layer: impl Into<String>,
    ) -> Self {
        Self {
            severity: RenderThreadDiagnosticSeverity::Info,
            code: code.into(),
            message: message.into(),
            layer: layer.into(),
        }
    }

    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        layer: impl Into<String>,
    ) -> Self {
        Self {
            severity: RenderThreadDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            layer: layer.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderThreadDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderThreadReport {
    pub schema_version: String,
    pub frame_index: u64,
    pub thread_mode: RenderThreadMode,
    pub backend_kind: String,
    pub target_id: String,
    pub target_kind: String,
    pub scene_proxy_count: usize,
    pub command_summary: Option<RenderThreadCommandSummary>,
    pub render_frame_report: RuntimeRenderFrameReport,
    pub texture_lifetime_report: GpuTextureLifetimeReport,
    pub resource_lifetime_report: RenderResourceLifetimeReport,
    pub rdg_status: String,
    pub rhi_status: String,
    pub present_status: String,
    pub diagnostics: Vec<RenderThreadDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderThreadFrameOutput {
    pub renderer_output: RuntimeRendererOutput,
    pub report: RenderThreadReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderSubmissionTicket {
    pub frame_index: u64,
    pub submit_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderSubmissionReport {
    pub schema_version: String,
    pub frame_index: u64,
    pub submit_sequence: u64,
    pub accepted: bool,
    pub submitted: bool,
    pub presented: bool,
    pub completed_frame_index: u64,
    pub queue_depth_after_submit: usize,
    pub queue_wait_frames: u64,
    pub thread_mode: RenderThreadMode,
    pub diagnostics: Vec<RenderThreadDiagnostic>,
    pub render_thread_report: RenderThreadReport,
}

#[derive(Debug, Clone)]
struct QueuedRenderFrame {
    packet: RenderFramePacket,
    ticket: RenderSubmissionTicket,
    queue_depth_after_submit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RenderThreadQueue {
    next_submit_sequence: u64,
    completed_frame_index: u64,
    pending: VecDeque<QueuedRenderFrame>,
    reports: Vec<RenderSubmissionReport>,
}

impl RenderThreadQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submit(&mut self, packet: RenderFramePacket) -> RenderSubmissionTicket {
        self.next_submit_sequence += 1;
        let ticket = RenderSubmissionTicket {
            frame_index: packet.frame_index,
            submit_sequence: self.next_submit_sequence,
        };
        self.pending.push_back(QueuedRenderFrame {
            packet,
            ticket,
            queue_depth_after_submit: self.pending.len() + 1,
        });
        ticket
    }

    pub fn process_next(
        &mut self,
        render_thread: &mut RenderThread,
    ) -> Option<RenderSubmissionReport> {
        self.process_next_output(render_thread)
            .map(|(_, report)| report)
    }

    pub fn process_next_output(
        &mut self,
        render_thread: &mut RenderThread,
    ) -> Option<(RenderThreadFrameOutput, RenderSubmissionReport)> {
        let queued = self.pending.pop_front()?;
        let output = render_thread.execute_packet(queued.packet);
        self.completed_frame_index = output.report.frame_index;
        let presented = output.report.present_status == "presented";
        let mut diagnostics = output.report.diagnostics.clone();
        if render_thread.config.thread_mode == RenderThreadMode::DedicatedThread {
            diagnostics.push(RenderThreadDiagnostic::info(
                "dedicated_thread_not_spawned_c_min",
                "DedicatedThread mode keeps the queue boundary in C-min, but does not spawn an OS render thread yet",
                "render_submission",
            ));
        }
        let report = RenderSubmissionReport {
            schema_version: "render-submission-report.v1".to_string(),
            frame_index: output.report.frame_index,
            submit_sequence: queued.ticket.submit_sequence,
            accepted: true,
            submitted: output.report.rhi_status == "ok",
            presented,
            completed_frame_index: self.completed_frame_index,
            queue_depth_after_submit: queued.queue_depth_after_submit,
            queue_wait_frames: output
                .report
                .frame_index
                .saturating_sub(queued.ticket.frame_index),
            thread_mode: render_thread.config.thread_mode,
            diagnostics,
            render_thread_report: output.report.clone(),
        };
        self.reports.push(report.clone());
        Some((output, report))
    }

    pub fn poll_report(&self, ticket: RenderSubmissionTicket) -> Option<&RenderSubmissionReport> {
        self.reports.iter().find(|report| {
            report.frame_index == ticket.frame_index
                && report.submit_sequence == ticket.submit_sequence
        })
    }

    pub fn completed_frame_index(&self) -> u64 {
        self.completed_frame_index
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn drain_shutdown(
        &mut self,
        render_thread: &mut RenderThread,
    ) -> Vec<RenderSubmissionReport> {
        let mut drained = Vec::new();
        while let Some(report) = self.process_next(render_thread) {
            drained.push(report);
        }
        drained
    }
}

#[derive(Debug, Clone)]
pub struct RenderThread {
    config: RenderThreadConfig,
    renderer: RuntimeRenderer,
    texture_registry: RuntimeGpuTextureRegistry,
    resource_manager: RenderResourceManager,
}

impl Default for RenderThread {
    fn default() -> Self {
        Self::new(RenderThreadConfig::default())
    }
}

impl RenderThread {
    pub fn new(config: RenderThreadConfig) -> Self {
        Self {
            config,
            renderer: RuntimeRenderer::new(),
            texture_registry: RuntimeGpuTextureRegistry::new(),
            resource_manager: RenderResourceManager::new(),
        }
    }

    pub fn render_frame(&mut self, input: RenderThreadFrameInput<'_>) -> RenderThreadFrameOutput {
        let packet = RenderFramePacket::from_input(input);
        self.submit_frame_output(packet).0
    }

    pub fn submit_frame(&mut self, packet: RenderFramePacket) -> RenderSubmissionReport {
        self.submit_frame_output(packet).1
    }

    pub fn submit_frame_output(
        &mut self,
        packet: RenderFramePacket,
    ) -> (RenderThreadFrameOutput, RenderSubmissionReport) {
        let mut queue = RenderThreadQueue::new();
        let ticket = queue.submit(packet);
        let (output, report) = queue
            .process_next_output(self)
            .expect("submitted packet should produce a render submission report");
        debug_assert_eq!(queue.poll_report(ticket), Some(&report));
        (output, report)
    }

    fn execute_packet(&mut self, packet: RenderFramePacket) -> RenderThreadFrameOutput {
        for request in packet.resource_requests {
            self.resource_manager
                .request_resource(packet.frame_index, request);
        }
        for release_request in packet.resource_release_requests {
            if let Err(error) = self.resource_manager.release_resource(release_request) {
                let _ = error;
            }
        }
        let view = packet
            .view_id
            .and_then(|view_id| packet.render_scene_state.view(view_id));
        let mut renderer_output = self.renderer.render_headless(RuntimeRendererInput {
            frame_index: packet.frame_index,
            render_scene_state: &packet.render_scene_state,
            render_view_state: view,
            aui_overlay: packet.aui_overlay.as_ref(),
            aui_composition: packet.aui_composition.as_ref(),
            sprite_texture_bindings: packet.sprite_texture_bindings.as_ref(),
            runtime_texture_bindings: packet.runtime_texture_bindings.as_ref(),
            quality_profile: packet.quality_profile,
            render_target: packet.render_target.clone(),
        });
        let texture_lifetime_report = if let Some(descriptor) = &renderer_output.texture_descriptor
        {
            self.texture_registry.record_successful_frame(
                GpuTextureDescriptor::from_viewport_descriptor(
                    descriptor,
                    packet.render_target.target_kind,
                ),
            )
        } else {
            self.texture_registry.record_successful_frame(
                GpuTextureDescriptor::from_surface_target(
                    &packet.render_target,
                    packet.frame_index,
                ),
            )
        };
        renderer_output.texture_lifetime_report = Some(texture_lifetime_report.clone());
        let resource_lifetime_report = self.resource_manager.end_frame(packet.frame_index);
        let mut diagnostics = Vec::new();
        for diagnostic in &renderer_output.render_graph_report.diagnostics {
            diagnostics.push(RenderThreadDiagnostic::info(
                diagnostic.code.clone(),
                diagnostic.message.clone(),
                "rdg",
            ));
        }
        if let Some(rhi_report) = &renderer_output.rhi_backend_report {
            for diagnostic in &rhi_report.diagnostics {
                diagnostics.push(RenderThreadDiagnostic {
                    severity: match diagnostic.severity {
                        crate::engine_rhi::RhiBackendDiagnosticSeverity::Info => {
                            RenderThreadDiagnosticSeverity::Info
                        }
                        crate::engine_rhi::RhiBackendDiagnosticSeverity::Warning => {
                            RenderThreadDiagnosticSeverity::Warning
                        }
                        crate::engine_rhi::RhiBackendDiagnosticSeverity::Error => {
                            RenderThreadDiagnosticSeverity::Error
                        }
                    },
                    code: diagnostic.code.clone(),
                    message: diagnostic.message.clone(),
                    layer: "rhi_backend".to_string(),
                });
            }
        }
        let rdg_status = if renderer_output.render_graph_report.error_count == 0 {
            "ok"
        } else {
            "error"
        }
        .to_string();
        let rhi_status = if renderer_output
            .rhi_backend_report
            .as_ref()
            .is_some_and(|report| {
                report.diagnostics.iter().any(|diagnostic| {
                    diagnostic.severity == crate::engine_rhi::RhiBackendDiagnosticSeverity::Error
                })
            }) {
            "error"
        } else {
            "ok"
        }
        .to_string();
        let present_status = renderer_output
            .rhi_backend_report
            .as_ref()
            .map(|report| {
                if report.present_count > 0 {
                    "presented"
                } else {
                    "not_presented"
                }
            })
            .unwrap_or("not_submitted")
            .to_string();
        let report = RenderThreadReport {
            schema_version: "render-thread-report.v1".to_string(),
            frame_index: packet.frame_index,
            thread_mode: self.config.thread_mode,
            backend_kind: self.config.backend_kind.clone(),
            target_id: packet.render_target.target_id.clone(),
            target_kind: format!("{:?}", packet.render_target.target_kind),
            scene_proxy_count: packet.render_scene_state.proxies_len(),
            command_summary: packet
                .render_frame_report
                .as_ref()
                .map(RenderThreadCommandSummary::from_render_frame_report),
            render_frame_report: renderer_output.render_frame_report.clone(),
            texture_lifetime_report,
            resource_lifetime_report,
            rdg_status,
            rhi_status,
            present_status,
            diagnostics,
        };
        RenderThreadFrameOutput {
            renderer_output,
            report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui::{
        AuiComputedRect, AuiOverlayDrawItem, AuiOverlayFrame, AuiOverlayItemKind,
        AuiOverlaySortKey, AuiRenderReport,
    };
    use crate::components::{Renderable, Transform};
    use crate::ids::{RuntimeEntityId, SourceEntityId};
    use crate::math::Vec3;
    use crate::render_resource::{
        RenderResourceKind, RenderResourceRequest, RenderResourceSource, RenderResourceState,
    };
    use crate::render_state::{
        RenderProxy, RenderProxyId, RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
    };

    fn scene() -> RenderSceneState {
        let mut scene = RenderSceneState::new();
        scene.register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::ViewportTexture,
        ));
        scene
    }

    fn scene_with_proxy() -> RenderSceneState {
        let mut scene = scene();
        scene.insert_proxy(RenderProxy::new(
            RenderProxyId(0),
            RuntimeEntityId::new(1, 0),
            SourceEntityId::from("entity-player"),
            Transform {
                local_position: Vec3::ZERO,
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            },
            Renderable {
                mesh_ref: Some("mesh-player".to_string()),
                material_ref: Some("material-player".to_string()),
                visible: true,
                layer: "default".to_string(),
            },
        ));
        scene
    }

    fn input(scene: &RenderSceneState) -> RenderThreadFrameInput<'_> {
        RenderThreadFrameInput {
            frame_index: 1,
            render_scene_state: scene,
            render_frame_report: None,
            resource_requests: Vec::new(),
            resource_release_requests: Vec::new(),
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            view_id: Some(RenderViewId(1)),
            quality_profile: QualityProfile::default(),
            render_target: RenderTarget::viewport_texture("viewport-main", 640, 360),
        }
    }

    fn packet(scene: &RenderSceneState, frame_index: u64) -> RenderFramePacket {
        RenderFramePacket::from_input(RenderThreadFrameInput {
            frame_index,
            render_scene_state: scene,
            render_frame_report: None,
            resource_requests: Vec::new(),
            resource_release_requests: Vec::new(),
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            view_id: Some(RenderViewId(1)),
            quality_profile: QualityProfile::default(),
            render_target: RenderTarget::viewport_texture("viewport-main", 640, 360),
        })
    }

    fn overlay(frame_index: u64) -> AuiOverlayFrame {
        AuiOverlayFrame {
            frame_index,
            draw_items: vec![AuiOverlayDrawItem {
                item_id: "aui-item-1".to_string(),
                canvas_id: "main".to_string(),
                composition_stage: crate::aui::AuiCompositionStage::ScreenOverlay,
                node_id: "score-label".to_string(),
                item_kind: AuiOverlayItemKind::Text,
                rect: AuiComputedRect {
                    x: 24.0,
                    y: 24.0,
                    width: 220.0,
                    height: 36.0,
                },
                effective_clip_rect: None,
                color: Some("#fff".to_string()),
                asset_id: None,
                text: Some("SCORE 000000".to_string()),
                font_size: Some(24.0),
                font: None,
                sort_key: AuiOverlaySortKey {
                    canvas_layer: 0,
                    canvas_sorting_order: 0,
                    tree_order: 0,
                },
            }],
            report: AuiRenderReport {
                draw_command_count: 1,
                text_count: 1,
                image_count: 0,
                effective_clip_item_count: 0,
                culled_draw_item_count: 0,
                scrollbar_visible_count: 0,
                batch_hint_count: 0,
            },
            glyph_plan: None,
        }
    }

    #[test]
    fn render_thread_consumes_render_commands_without_reading_world() {
        let scene = scene_with_proxy();
        let mut render_thread = RenderThread::default();

        let output = render_thread.render_frame(input(&scene));

        assert_eq!(output.report.scene_proxy_count, 1);
        assert_eq!(output.report.render_frame_report.draw_item_count, 1);
        assert_eq!(output.report.present_status, "presented");
    }

    #[test]
    fn render_thread_outputs_frame_report() {
        let scene = scene();
        let mut render_thread = RenderThread::default();

        let output = render_thread.render_frame(input(&scene));
        let json = serde_json::to_string(&output.report).expect("serialize report");

        assert_eq!(output.report.schema_version, "render-thread-report.v1");
        assert_eq!(output.report.rdg_status, "ok");
        assert_eq!(output.report.rhi_status, "ok");
        assert!(json.contains("textureLifetimeReport"));
        assert!(json.contains("resourceLifetimeReport"));
        assert!(output.renderer_output.texture_lifetime_report.is_some());
    }

    #[test]
    fn render_thread_keeps_scene_state_owner_boundary() {
        let scene = scene_with_proxy();
        let mut render_thread = RenderThread::default();

        let before = scene.proxies_len();
        let output = render_thread.render_frame(input(&scene));

        assert_eq!(before, scene.proxies_len());
        assert_eq!(output.report.scene_proxy_count, before);
    }

    #[test]
    fn render_thread_reports_render_resource_lifetime() {
        let scene = scene();
        let mut render_thread = RenderThread::default();
        let mut input = input(&scene);
        input
            .resource_requests
            .push(RenderResourceRequest::texture("texture-player", "v1", 16));

        let output = render_thread.render_frame(input);

        assert_eq!(output.report.resource_lifetime_report.created_count, 1);
        assert_eq!(output.report.resource_lifetime_report.uploaded_bytes, 16);
        assert!(output
            .report
            .resource_lifetime_report
            .events
            .iter()
            .any(|event| event.state_after == RenderResourceState::Resident));
    }

    #[test]
    fn render_thread_does_not_expose_backend_resource_to_runtime_logic() {
        let scene = scene();
        let mut render_thread = RenderThread::default();
        let mut input = input(&scene);
        input.resource_requests.push(RenderResourceRequest {
            key: crate::render_resource::RenderAssetKey::new(
                "mesh-player",
                "v1",
                RenderResourceKind::MeshBuffer,
            ),
            source: RenderResourceSource::MeshBufferDescriptor {
                vertex_count: 3,
                index_count: 3,
            },
            byte_len: 96,
            reason: "mesh upload".to_string(),
        });

        let output = render_thread.render_frame(input);
        let json = serde_json::to_string(&output.report).expect("serialize report");

        assert!(json.contains("resourceLifetimeReport"));
        assert!(!json.contains("backendResource"));
        assert!(!json.contains("wgpu::"));
    }

    #[test]
    fn render_frame_packet_is_owned_and_cloneable() {
        let scene = scene_with_proxy();
        let packet = packet(&scene, 9);
        let cloned = packet.clone();

        assert_eq!(cloned.frame_index, 9);
        assert_eq!(cloned.render_scene_state.proxies_len(), 1);
        assert_eq!(cloned.render_target.target_id, "viewport-main");
    }

    #[test]
    fn render_thread_forwards_packet_aui_overlay_to_runtime_renderer() {
        let scene = scene();
        let mut render_thread = RenderThread::default();
        let mut packet = packet(&scene, 21);
        packet.aui_overlay = Some(overlay(21));

        let output = render_thread.submit_frame_output(packet).0;

        assert!(output
            .renderer_output
            .render_graph
            .passes
            .iter()
            .any(|pass| pass.pass_kind == crate::render_graph::RenderPassKind::DrawUiComposition));
        assert!(
            output
                .renderer_output
                .render_frame_report
                .ui_screen_overlay_pass_present
        );
        assert_eq!(
            output.renderer_output.render_frame_report.draw_item_count,
            1
        );
    }

    #[test]
    fn render_thread_queue_submits_processes_and_polls_report() {
        let scene = scene_with_proxy();
        let mut queue = RenderThreadQueue::new();
        let mut render_thread = RenderThread::default();

        let ticket = queue.submit(packet(&scene, 11));

        assert_eq!(queue.pending_len(), 1);
        let report = queue
            .process_next(&mut render_thread)
            .expect("submission report");

        assert_eq!(report.schema_version, "render-submission-report.v1");
        assert_eq!(report.frame_index, 11);
        assert_eq!(report.submit_sequence, ticket.submit_sequence);
        assert!(report.accepted);
        assert!(report.submitted);
        assert!(report.presented);
        assert_eq!(report.completed_frame_index, 11);
        assert_eq!(queue.completed_frame_index(), 11);
        assert_eq!(queue.pending_len(), 0);
        assert_eq!(queue.poll_report(ticket), Some(&report));
    }

    #[test]
    fn dedicated_thread_mode_reports_c_min_queue_boundary() {
        let scene = scene();
        let mut render_thread = RenderThread::new(RenderThreadConfig {
            thread_mode: RenderThreadMode::DedicatedThread,
            backend_kind: "headless-rhi".to_string(),
        });

        let report = render_thread.submit_frame(packet(&scene, 13));

        assert_eq!(report.thread_mode, RenderThreadMode::DedicatedThread);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "dedicated_thread_not_spawned_c_min" }));
    }
}
