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
    /// CPU cell transactions consume the existing native upload owner through RHI.
    fn begin_geometry_upload(
        &mut self,
        _geometry: &[crate::render_geometry::PreparedGeometry],
        _textures: &[crate::render_asset_production::PreparedTextureAsset],
    ) -> Result<crate::geometry_upload::GeometryUploadTicket, String> {
        Err("geometry.upload.unsupported_backend".into())
    }
    fn geometry_upload_state(
        &mut self,
        _ticket: crate::geometry_upload::GeometryUploadTicket,
    ) -> Result<crate::geometry_upload::GeometryUploadState, String> {
        Err("geometry.upload.unsupported_backend".into())
    }
    fn release_geometry_upload(
        &mut self,
        _ticket: crate::geometry_upload::GeometryUploadTicket,
    ) -> Result<(), String> {
        Err("geometry.upload.unsupported_backend".into())
    }

    /// Legacy consumers must opt in; a new typed payload cannot disappear silently.
    fn supports_geometry_batches(&self) -> bool {
        false
    }
    fn backend_kind(&self) -> &'static str;
    fn begin_frame(&mut self, frame: EngineRhiFrame);
    fn resolve_color_3d(&mut self, resolve: &crate::color_pipeline::ColorResolve3D);
    fn begin_view(
        &mut self,
        target: &str,
        view: &crate::render_contract::ExecutableRenderView,
        depth_target: Option<&str>,
    );
    fn clear(&mut self, target_id: &str, color: [OrderedF32; 4]);
    fn draw(&mut self, draw_call: EngineRhiDrawCall);
    fn reconcile_particles(
        &mut self,
        sources: &[crate::particle_render_contract::ParticleSourceFrame],
    );
    fn particle_step(
        &mut self,
        instance: u64,
        step: &crate::particle_render_contract::ParticleRenderStep,
    );
    fn submit(&mut self);
    fn present(&mut self, target_id: &str);
    fn finish_report(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport;

    fn execute_plan(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport {
        let mut validation = crate::render_geometry::DrawViewValidation::default();
        for command in &plan.commands {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::GeometryBatch { batch },
                ..
            } = command
            {
                let error = if !self.supports_geometry_batches() {
                    Some("geometry.batch.unsupported_backend".to_owned())
                } else {
                    batch
                        .validate_shared_with_view(&mut validation)
                        .err()
                        .map(|e| e.to_string())
                };
                if let Some(code) = error {
                    let mut report =
                        RhiBackendReport::unavailable(self.backend_kind(), plan.frame_index, code);
                    report.failed_resource_count = 1;
                    return report;
                }
            }
        }
        for command in &plan.commands {
            match command {
                RhiCommand::ResolveColor3d { resolve } => self.resolve_color_3d(resolve),
                RhiCommand::BeginView {
                    target,
                    view,
                    depth_target,
                } => self.begin_view(target, view, depth_target.as_deref()),
                RhiCommand::ReconcileParticles { sources } => self.reconcile_particles(sources),
                RhiCommand::SimulateParticles { instance, step } => {
                    self.particle_step(*instance, step)
                }
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
                    target_id: target.as_ref().to_owned(),
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

#[cfg(test)]
mod batch_reader_tests {
    use super::*;
    struct OldBackend;
    impl EngineRhiBackend for OldBackend {
        fn backend_kind(&self) -> &'static str {
            "old-reader"
        }
        fn begin_frame(&mut self, _: EngineRhiFrame) {
            panic!("unsupported plan performed side effects")
        }
        fn resolve_color_3d(&mut self, _: &crate::color_pipeline::ColorResolve3D) {
            panic!()
        }
        fn begin_view(
            &mut self,
            _: &str,
            _: &crate::render_contract::ExecutableRenderView,
            _: Option<&str>,
        ) {
            panic!()
        }
        fn clear(&mut self, _: &str, _: [OrderedF32; 4]) {
            panic!()
        }
        fn draw(&mut self, _: EngineRhiDrawCall) {
            panic!()
        }
        fn reconcile_particles(
            &mut self,
            _: &[crate::particle_render_contract::ParticleSourceFrame],
        ) {
            panic!()
        }
        fn particle_step(
            &mut self,
            _: u64,
            _: &crate::particle_render_contract::ParticleRenderStep,
        ) {
            panic!()
        }
        fn submit(&mut self) {
            panic!()
        }
        fn present(&mut self, _: &str) {
            panic!()
        }
        fn finish_report(&mut self, _: &RhiCommandPlan) -> RhiBackendReport {
            panic!()
        }
    }
    #[test]
    fn batch_reader_capability_rejects_before_legacy_backend_side_effects() {
        let frame = crate::render_geometry::batch_test_frame(
            true,
            crate::render_contract::MaterialBlend::Opaque,
            128,
            1,
        );
        let graph = crate::render_geometry::batch_test_graph(&frame);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let report = OldBackend.execute_plan(&plan);
        assert_eq!(report.failed_resource_count, 1);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == "geometry.batch.unsupported_backend"));
        let report = crate::headless_rhi_backend::HeadlessRhiBackend::new().execute_plan(&plan);
        assert_eq!(report.failed_resource_count, 0);
        assert!(report.draw_count > 0);
        #[derive(serde::Deserialize)]
        #[serde(tag = "payloadKind", rename_all = "camelCase")]
        enum LegacyPayload {
            Geometry {
                #[serde(rename = "draw")]
                _draw: std::sync::Arc<crate::render_geometry::DrawGeometry>,
            },
        }
        let payload = plan
            .commands
            .iter()
            .find_map(RhiCommand::geometry_payload)
            .unwrap();
        let wire = serde_json::to_value(payload).unwrap();
        assert!(serde_json::from_value::<LegacyPayload>(wire).is_err());
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
