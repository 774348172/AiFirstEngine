use serde::{Deserialize, Serialize};

use crate::render_graph::OrderedF32;
use crate::rhi_command_plan::{RhiCommand, RhiCommandPlan, RhiDrawKind, RhiDrawPayload};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiDevice {
    pub device_id: String,
    pub backend_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiQueue {
    pub queue_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiSurface {
    pub surface_id: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiTexture {
    pub texture_id: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiBuffer {
    pub buffer_id: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiCommandEncoder {
    pub encoder_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiRenderPass {
    pub pass_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiFrame {
    pub frame_index: u64,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRhiDrawCall {
    pub target_id: String,
    pub draw_kind: RhiDrawKind,
    pub vertex_count: u32,
    pub payload: RhiDrawPayload,
}

pub trait EngineRhiBackend {
    fn backend_kind(&self) -> &'static str;
    fn begin_frame(&mut self, frame: EngineRhiFrame);
    fn clear(&mut self, target_id: &str, color: [OrderedF32; 4]);
    fn draw(&mut self, draw_call: EngineRhiDrawCall);
    fn submit(&mut self);
    fn present(&mut self, target_id: &str);
    fn finish_report(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport;

    fn execute_plan(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport {
        for command in &plan.commands {
            match command {
                RhiCommand::BeginFrame { target } => self.begin_frame(EngineRhiFrame {
                    frame_index: plan.frame_index,
                    target_id: target.clone(),
                }),
                RhiCommand::Clear { target, color } => self.clear(target, *color),
                RhiCommand::Draw {
                    target,
                    draw_kind,
                    vertex_count,
                    payload,
                } => self.draw(EngineRhiDrawCall {
                    target_id: target.clone(),
                    draw_kind: *draw_kind,
                    vertex_count: *vertex_count,
                    payload: payload.clone(),
                }),
                RhiCommand::Submit => self.submit(),
                RhiCommand::Present { target } => self.present(target),
            }
        }
        self.finish_report(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RhiBackendReport {
    pub backend_kind: String,
    pub frame_index: u64,
    pub target_kind: String,
    pub clear_count: usize,
    pub draw_count: usize,
    pub submit_count: usize,
    pub present_count: usize,
    pub binding_count: usize,
    pub uploaded_resource_count: usize,
    pub reused_resource_count: usize,
    pub failed_resource_count: usize,
    pub target_hash: String,
    pub diagnostics: Vec<RhiBackendDiagnostic>,
}

impl RhiBackendReport {
    pub fn unavailable(
        backend_kind: impl Into<String>,
        frame_index: u64,
        code: impl Into<String>,
    ) -> Self {
        Self {
            backend_kind: backend_kind.into(),
            frame_index,
            target_kind: "unavailable".to_string(),
            clear_count: 0,
            draw_count: 0,
            submit_count: 0,
            present_count: 0,
            binding_count: 0,
            uploaded_resource_count: 0,
            reused_resource_count: 0,
            failed_resource_count: 0,
            target_hash: "unavailable".to_string(),
            diagnostics: vec![RhiBackendDiagnostic::error(
                code,
                "backend is not available in this build",
            )],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RhiBackendDiagnostic {
    pub severity: RhiBackendDiagnosticSeverity,
    pub code: String,
    pub message: String,
}

impl RhiBackendDiagnostic {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RhiBackendDiagnosticSeverity::Info,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RhiBackendDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: RhiBackendDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RhiBackendDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_report_is_json_serializable() {
        let report = RhiBackendReport::unavailable("wgpu", 1, "backend_unavailable");
        let json = serde_json::to_string(&report).expect("serialize report");

        assert!(json.contains("backendKind"));
        assert!(json.contains("backend_unavailable"));
        assert!(json.contains("bindingCount"));
    }
}
