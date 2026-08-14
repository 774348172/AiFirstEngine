mod diagnostics;
mod draw_plan;
mod font_system;
mod headless;
mod image_texture;
#[cfg(feature = "real-wgpu")]
mod real_wgpu;
mod render_graph;
mod shared_gpu;
mod surface;
mod texture_atlas;
mod viewport_texture;

pub use diagnostics::{
    RealUiPresentDiagnostic, RealUiPresentDiagnosticSeverity, RealUiPresentReport,
    REAL_UI_PRESENT_REPORT_SCHEMA_VERSION, UI_GPU_DRAW_PLAN_SCHEMA_VERSION,
    UI_RENDER_GRAPH_SCHEMA_VERSION, UI_RHI_COMMAND_PLAN_SCHEMA_VERSION,
};
pub use draw_plan::{
    UiGpuDrawPlan, UiGpuDrawableRect, UiGpuDrawableRectSource, UiGpuImageTextureQuad,
    UiGpuPaintBatch, UiGpuPaintBatchKind, UiGpuTextGlyph, UiGpuViewportTextureQuad,
};
pub use editor_ui_renderer::UiUvRect;
pub use headless::HeadlessUiGpuRenderer;
pub use image_texture::{
    EditorImageTextureRegistry, EditorImageTextureSummary, EditorImageTextureUploadStatus,
    EDITOR_IMAGE_TEXTURE_MAX_BYTES, EDITOR_IMAGE_TEXTURE_MAX_ITEMS,
};
#[cfg(feature = "real-wgpu")]
pub use real_wgpu::{RealWgpuUiRenderer, UiRgbaCapture};
pub use render_graph::{
    UiRenderGraph, UiRenderPass, UiRenderPassKind, UiRenderResource, UiRenderResourceKind,
    UiRhiCommand, UiRhiCommandKind, UiRhiCommandPlan,
};
#[cfg(feature = "real-wgpu")]
pub use shared_gpu::EditorSharedGpuContext;
pub use shared_gpu::{EditorSharedGpuContextStatus, EditorSharedGpuContextSummary};
#[cfg(feature = "real-wgpu")]
pub use viewport_texture::EditorViewportTextureReadback;
pub use viewport_texture::{
    EditorViewportTexturePresentStatus, EditorViewportTextureRegistry,
    EditorViewportTextureSummary, GameViewPublicationIdentity, GameViewPublicationReceipt,
    GameViewPublicationStatus, RuntimeContentIdentity,
};

#[cfg(test)]
mod tests;
