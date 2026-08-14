use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSharedGpuContextSummary {
    pub schema_version: String,
    pub status: EditorSharedGpuContextStatus,
    pub backend_name: String,
    pub surface_format: Option<String>,
    pub device_label: String,
    pub queue_label: String,
    pub real_wgpu_available: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorSharedGpuContextStatus {
    Available,
    HeadlessMock,
    GpuUnavailable,
}

impl EditorSharedGpuContextSummary {
    pub fn headless_mock() -> Self {
        Self {
            schema_version: "editor-shared-gpu-context.v1".to_string(),
            status: EditorSharedGpuContextStatus::HeadlessMock,
            backend_name: "headless".to_string(),
            surface_format: None,
            device_label: "none".to_string(),
            queue_label: "none".to_string(),
            real_wgpu_available: false,
            diagnostics: vec!["real_wgpu_context_not_created_in_headless_mode".to_string()],
        }
    }

    pub fn gpu_unavailable(message: impl Into<String>) -> Self {
        Self {
            schema_version: "editor-shared-gpu-context.v1".to_string(),
            status: EditorSharedGpuContextStatus::GpuUnavailable,
            backend_name: "unavailable".to_string(),
            surface_format: None,
            device_label: "none".to_string(),
            queue_label: "none".to_string(),
            real_wgpu_available: false,
            diagnostics: vec![message.into()],
        }
    }
}

#[cfg(feature = "real-wgpu")]
pub struct EditorSharedGpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    summary: EditorSharedGpuContextSummary,
}

#[cfg(feature = "real-wgpu")]
impl EditorSharedGpuContext {
    pub(crate) fn from_device_queue(
        device: wgpu::Device,
        queue: wgpu::Queue,
        backend_name: impl Into<String>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            device,
            queue,
            summary: EditorSharedGpuContextSummary {
                schema_version: "editor-shared-gpu-context.v1".to_string(),
                status: EditorSharedGpuContextStatus::Available,
                backend_name: backend_name.into(),
                surface_format: Some(format!("{surface_format:?}")),
                device_label: "editor-ui-device".to_string(),
                queue_label: "editor-ui-queue".to_string(),
                real_wgpu_available: true,
                diagnostics: Vec::new(),
            },
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn summary(&self) -> &EditorSharedGpuContextSummary {
        &self.summary
    }
}
