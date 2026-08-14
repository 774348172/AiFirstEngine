use crate::config::NativeEditorWindowConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuSurfacePlan {
    pub backend: &'static str,
    pub requires_native_window: bool,
    pub creates_surface: bool,
    pub creates_device_queue: bool,
    pub configures_swapchain: bool,
}

pub fn wgpu_surface_plan() -> WgpuSurfacePlan {
    WgpuSurfacePlan {
        backend: "wgpu",
        requires_native_window: true,
        creates_surface: true,
        creates_device_queue: true,
        configures_swapchain: true,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeWindowAttributePlan {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub scale_factor: f64,
}

pub fn native_window_attribute_plan(
    config: &NativeEditorWindowConfig,
) -> NativeWindowAttributePlan {
    NativeWindowAttributePlan {
        title: config.title.clone(),
        width: config.width,
        height: config.height,
        resizable: config.resizable,
        scale_factor: config.scale_factor,
    }
}

#[cfg(feature = "real-window")]
pub fn winit_window_attributes(
    config: &NativeEditorWindowConfig,
) -> winit::window::WindowAttributes {
    winit::window::Window::default_attributes()
        .with_title(config.title.clone())
        .with_inner_size(winit::dpi::LogicalSize::new(
            config.width as f64,
            config.height as f64,
        ))
        .with_resizable(config.resizable)
}

#[cfg(feature = "real-window")]
pub fn wgpu_instance_descriptor() -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        flags: wgpu::InstanceFlags::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeEditorWindowReadiness {
    pub window_attributes_ready: bool,
    pub wgpu_instance_ready: bool,
    pub headless_test_only: bool,
}

pub fn validate_window_skeleton(config: &NativeEditorWindowConfig) -> NativeEditorWindowReadiness {
    let _attributes = native_window_attribute_plan(config);
    let _surface_plan = wgpu_surface_plan();
    NativeEditorWindowReadiness {
        window_attributes_ready: true,
        wgpu_instance_ready: true,
        headless_test_only: true,
    }
}
