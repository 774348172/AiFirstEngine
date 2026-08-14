use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::engine_rhi::{
    EngineRhiBackend, EngineRhiDrawCall, EngineRhiFrame, RhiBackendDiagnostic, RhiBackendReport,
};
use crate::render_graph::OrderedF32;
use crate::rhi_command_plan::{RhiCommandPlan, RhiDrawPayload};

#[derive(Debug, Clone)]
pub struct HeadlessRhiBackend {
    target_kind: String,
    clear_count: usize,
    draw_count: usize,
    submit_count: usize,
    present_count: usize,
    binding_count: usize,
    uploaded_resource_count: usize,
    hasher: DefaultHasher,
}

impl Default for HeadlessRhiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessRhiBackend {
    pub fn new() -> Self {
        Self {
            target_kind: "headlessTexture".to_string(),
            clear_count: 0,
            draw_count: 0,
            submit_count: 0,
            present_count: 0,
            binding_count: 0,
            uploaded_resource_count: 0,
            hasher: DefaultHasher::new(),
        }
    }

    fn record_payload_resources(&mut self, payload: &RhiDrawPayload) {
        if let RhiDrawPayload::SpriteTextured {
            binding: Some(binding),
            ..
        } = payload
        {
            self.binding_count += 1;
            self.uploaded_resource_count += binding.resources.len();
            binding.binding_id.hash(&mut self.hasher);
            binding.resources.hash(&mut self.hasher);
        }
    }
}

impl EngineRhiBackend for HeadlessRhiBackend {
    fn backend_kind(&self) -> &'static str {
        "headless-rhi"
    }

    fn begin_frame(&mut self, frame: EngineRhiFrame) {
        frame.frame_index.hash(&mut self.hasher);
        "begin".hash(&mut self.hasher);
        frame.target_id.hash(&mut self.hasher);
    }

    fn clear(&mut self, target_id: &str, color: [OrderedF32; 4]) {
        self.clear_count += 1;
        "clear".hash(&mut self.hasher);
        target_id.hash(&mut self.hasher);
        color.hash(&mut self.hasher);
    }

    fn draw(&mut self, draw_call: EngineRhiDrawCall) {
        self.draw_count += 1;
        "draw".hash(&mut self.hasher);
        draw_call.target_id.hash(&mut self.hasher);
        draw_call.draw_kind.hash(&mut self.hasher);
        draw_call.vertex_count.hash(&mut self.hasher);
        self.record_payload_resources(&draw_call.payload);
        draw_call.payload.hash(&mut self.hasher);
    }

    fn submit(&mut self) {
        self.submit_count += 1;
        "submit".hash(&mut self.hasher);
    }

    fn present(&mut self, target_id: &str) {
        self.present_count += 1;
        "present".hash(&mut self.hasher);
        target_id.hash(&mut self.hasher);
    }

    fn finish_report(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport {
        let mut diagnostics = Vec::new();
        plan.graph_id.hash(&mut self.hasher);
        if plan.has_errors() {
            diagnostics.push(RhiBackendDiagnostic::error(
                "rhi_plan_has_errors",
                "headless backend skipped invalid graph side effects",
            ));
        }

        RhiBackendReport {
            backend_kind: self.backend_kind().to_string(),
            frame_index: plan.frame_index,
            target_kind: self.target_kind.clone(),
            clear_count: self.clear_count,
            draw_count: self.draw_count,
            submit_count: self.submit_count,
            present_count: self.present_count,
            binding_count: self.binding_count,
            uploaded_resource_count: self.uploaded_resource_count,
            reused_resource_count: 0,
            failed_resource_count: if plan.has_errors() { 1 } else { 0 },
            target_hash: format!("{:016x}", self.hasher.finish()),
            diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_graph::{
        color, RenderGraph, RenderPass, RenderPassCommand, RenderPassKind, RenderResource,
    };
    use crate::rhi_command_plan::compile_render_graph_to_rhi_plan;

    fn plan() -> RhiCommandPlan {
        let target = "surface-main".to_string();
        let mut graph = RenderGraph::new("graph-1", 1);
        graph.output_target = Some(target.clone());
        graph
            .resources
            .push(RenderResource::surface_backbuffer(target.clone(), 640, 480));
        graph.passes.push(RenderPass {
            pass_id: "pass-main".to_string(),
            pass_name: "Pass Main".to_string(),
            pass_kind: RenderPassKind::DrawTestGeometry,
            view_id: "view-1".to_string(),
            reads: Vec::new(),
            writes: vec![target.clone()],
            color_targets: vec![target.clone()],
            depth_target: None,
            commands: vec![
                RenderPassCommand::Clear {
                    target: target.clone(),
                    color: color([0.1, 0.2, 0.3, 1.0]),
                },
                RenderPassCommand::DrawTestGeometry {
                    target,
                    vertex_count: 3,
                },
            ],
            debug_source: None,
        });
        compile_render_graph_to_rhi_plan(&graph)
    }

    #[test]
    fn headless_backend_counts_clear_draw_submit_present() {
        let mut backend = HeadlessRhiBackend::new();

        let report = backend.execute_plan(&plan());

        assert_eq!(report.clear_count, 1);
        assert_eq!(report.draw_count, 1);
        assert_eq!(report.submit_count, 1);
        assert_eq!(report.present_count, 1);
    }

    #[test]
    fn same_input_produces_same_target_hash() {
        let mut first_backend = HeadlessRhiBackend::new();
        let mut second_backend = HeadlessRhiBackend::new();

        let first = first_backend.execute_plan(&plan());
        let second = second_backend.execute_plan(&plan());

        assert_eq!(first.target_hash, second.target_hash);
    }

    #[test]
    fn headless_backend_counts_sprite_texture_binding() {
        use crate::render_asset_production::{RenderBindingKind, RenderBindingSet};
        use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
        use crate::rhi_command_plan::{RhiCommand, RhiDrawKind};

        let handle = RenderResourceHandle {
            kind: RenderResourceKind::Texture,
            index: 1,
            generation: 0,
        };
        let plan = RhiCommandPlan {
            frame_index: 1,
            graph_id: "graph-binding".to_string(),
            target_kind: "headlessTexture".to_string(),
            commands: vec![
                RhiCommand::BeginFrame {
                    target: "surface-main".to_string(),
                },
                RhiCommand::Draw {
                    target: "surface-main".to_string(),
                    draw_kind: RhiDrawKind::SpriteTextured,
                    vertex_count: 6,
                    payload: RhiDrawPayload::SpriteTextured {
                        sprite_ref: "sprite-a".to_string(),
                        material_ref: None,
                        sort_key: "sort".to_string(),
                        texture: Some(handle),
                        binding: Some(RenderBindingSet {
                            binding_id: "binding:sprite-a".to_string(),
                            binding_kind: RenderBindingKind::Texture,
                            resources: vec![handle],
                            material_handle: None,
                            sampler: "linearClamp".to_string(),
                            fallback_used: false,
                            debug_label: "test".to_string(),
                        }),
                        fallback_used: false,
                        pipeline_key: "sprite-basic.default".to_string(),
                        vertices: Vec::new(),
                    },
                },
                RhiCommand::Submit,
                RhiCommand::Present {
                    target: "surface-main".to_string(),
                },
            ],
            diagnostics: Vec::new(),
        };
        let mut backend = HeadlessRhiBackend::new();

        let report = backend.execute_plan(&plan);

        assert_eq!(report.binding_count, 1);
        assert_eq!(report.uploaded_resource_count, 1);
        assert_eq!(report.failed_resource_count, 0);
    }
}
