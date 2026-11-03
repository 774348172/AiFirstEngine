//! Explicit current-frame report accounting; no sampling or retained counters.
use super::{RenderThreadFrameOutput, RenderThreadReport};
use crate::engine_rhi::RhiBackendReport;
use crate::gpu_texture_lifetime::GpuTextureLifetimeReport;
use crate::render_graph_report::RenderGraphReport;
use crate::resource_memory::ResourceMemorySample;
use crate::runtime_renderer::RuntimeRenderFrameReport;

const OWNER: &str = "instrumentation";
const ROLES: &[&str] = &["instrumentation"];

fn strings(sample: &mut ResourceMemorySample, values: &[&String]) {
    for value in values {
        sample.cpu_string(value, OWNER, ROLES);
    }
}

fn frame_report(sample: &mut ResourceMemorySample, report: &RuntimeRenderFrameReport) {
    strings(sample, &[&report.graph_id, &report.target_id]);
    sample.cpu_vec(&report.diagnostics, OWNER, ROLES);
    for diagnostic in &report.diagnostics {
        strings(sample, &[&diagnostic.code, &diagnostic.message]);
    }
}

fn texture_report(sample: &mut ResourceMemorySample, report: &GpuTextureLifetimeReport) {
    strings(sample, &[&report.schema_version, &report.target_id]);
    if let Some(id) = &report.texture_id {
        strings(sample, &[&id.0]);
    }
    if let Some(error) = &report.last_error {
        strings(sample, &[error]);
    }
    sample.cpu_vec(&report.events, OWNER, ROLES);
    for event in &report.events {
        strings(
            sample,
            &[&event.texture_id.0, &event.target_id, &event.message],
        );
    }
}

fn graph_report(sample: &mut ResourceMemorySample, report: &RenderGraphReport) {
    strings(sample, &[&report.graph_id]);
    if let Some(target) = &report.output_target {
        strings(sample, &[target]);
    }
    sample.cpu_vec(&report.diagnostics, OWNER, ROLES);
    for diagnostic in &report.diagnostics {
        strings(sample, &[&diagnostic.code, &diagnostic.message]);
    }
}

fn thread_report(sample: &mut ResourceMemorySample, report: &RenderThreadReport) {
    strings(
        sample,
        &[
            &report.schema_version,
            &report.backend_kind,
            &report.target_id,
            &report.target_kind,
            &report.rdg_status,
            &report.rhi_status,
            &report.present_status,
        ],
    );
    sample.cpu_vec(&report.diagnostics, OWNER, ROLES);
    for diagnostic in &report.diagnostics {
        strings(
            sample,
            &[&diagnostic.code, &diagnostic.message, &diagnostic.layer],
        );
    }
    frame_report(sample, &report.render_frame_report);
    texture_report(sample, &report.texture_lifetime_report);
    let resources = &report.resource_lifetime_report;
    strings(sample, &[&resources.schema_version]);
    sample.cpu_vec(&resources.events, OWNER, ROLES);
    for event in &resources.events {
        strings(sample, &[&event.asset_id, &event.reason]);
        if let Some(diagnostic) = &event.diagnostic {
            strings(sample, &[&diagnostic.code, &diagnostic.message]);
        }
    }
}

impl RenderThreadFrameOutput {
    #[doc(hidden)]
    pub fn observe_report_memory(&self, sample: &mut ResourceMemorySample) {
        thread_report(sample, &self.report);
        // Equal report text can occupy different allocations. The original
        // census reducer deduplicates only physical aliases within this capture.
        frame_report(sample, &self.renderer_output.render_frame_report);
        if let Some(report) = &self.renderer_output.texture_lifetime_report {
            texture_report(sample, report);
        }
        graph_report(sample, &self.renderer_output.render_graph_report);
        if let Some(report) = &self.renderer_output.rhi_backend_report {
            report.observe_report_memory(sample);
        }
        sample.partial(
            "cpu.owner.instrumentation",
            "render.currentFrame.thread+frame+textureLifetime+resourceEvents+graph+rhi.actualCapacity;workerAndStdNodesNotIntegrated",
        );
    }
}

impl RhiBackendReport {
    #[doc(hidden)]
    pub fn observe_report_memory(&self, sample: &mut ResourceMemorySample) {
        strings(
            sample,
            &[&self.backend_kind, &self.target_kind, &self.target_hash],
        );
        sample.cpu_vec(&self.diagnostics, OWNER, ROLES);
        for diagnostic in &self.diagnostics {
            strings(sample, &[&diagnostic.code, &diagnostic.message]);
        }
        sample.partial(
            "cpu.owner.instrumentation",
            "rhi.currentReport.actualCapacity;otherReportsAndStdNodesNotIntegrated",
        );
    }
}
