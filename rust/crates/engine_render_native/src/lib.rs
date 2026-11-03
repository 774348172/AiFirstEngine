//! Native device, queue, uploads, particles and completion remain one owner.
use engine_runtime_core::*;
#[cfg(feature = "real-wgpu")]
pub mod gpu_frame_measurement;
#[cfg(feature = "real-wgpu")]
pub mod particle_gpu;
#[cfg(feature = "real-wgpu")]
pub mod particle_render;
pub mod wgpu_backend;

pub use engine_runtime_core::platform_time;
