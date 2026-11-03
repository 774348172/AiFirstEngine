//! Compatibility assembly; no second production implementation.
#[cfg(not(test))]
pub use engine_render_native::wgpu_backend;
#[cfg(all(not(test), feature = "real-wgpu"))]
pub use engine_render_native::{gpu_frame_measurement, particle_gpu, particle_render};
#[cfg(not(test))]
pub use engine_runtime_core::*;
#[cfg(not(test))]
pub mod domain {
    pub use engine_runtime_core::domain::*;
    pub mod render {
        pub use engine_render_native::wgpu_backend;
        pub use engine_runtime_core::domain::render::*;
    }
}
#[cfg(test)]
include!("unit_composition.rs");
