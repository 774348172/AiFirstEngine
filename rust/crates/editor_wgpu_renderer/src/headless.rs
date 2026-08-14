use editor_ui_renderer::UiDrawList;

use crate::diagnostics::RealUiPresentReport;
use crate::draw_plan::UiGpuDrawPlan;
use crate::render_graph::{UiRenderGraph, UiRhiCommandPlan};

pub struct HeadlessUiGpuRenderer {
    backend: String,
}

impl Default for HeadlessUiGpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessUiGpuRenderer {
    pub fn new() -> Self {
        Self {
            backend: "headless-ui-gpu".to_string(),
        }
    }

    pub fn present(&self, draw_list: &UiDrawList) -> RealUiPresentReport {
        match UiGpuDrawPlan::from_draw_list(draw_list) {
            Ok(plan) => {
                let graph = UiRenderGraph::from_draw_plan(&plan);
                let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);
                RealUiPresentReport::from_compiled_plan(
                    self.backend.clone(),
                    &plan,
                    &rhi_plan,
                    true,
                )
            }
            Err(error) => RealUiPresentReport::failed(
                self.backend.clone(),
                error.clone(),
                error,
                "editor_wgpu_renderer.headless",
            ),
        }
    }
}
