pub mod animator2d;
pub mod archetype;
pub mod atomic_directory_publish;
pub mod atomic_file_replace;
pub mod audio;
pub mod aui;
pub mod aui_control_feedback;
pub mod canonical_digest;
pub mod component_value;
pub mod components;
pub mod default_game_run;
pub mod diagnostics;
pub mod domain;
pub mod engine_host_loop;
pub mod engine_rhi;
pub mod field_path;
pub mod font_bundle;
pub mod font_generation;
pub mod frame_hash;
pub mod frame_loop;
pub mod game_view_presentation;
pub mod gameplay_command;
pub mod gameplay_rule_report;
pub mod gameplay_trace;
pub mod golden;
#[cfg(feature = "real-wgpu")]
pub mod gpu_frame_measurement;
pub mod gpu_texture_lifetime;
pub mod headless_rhi_backend;
pub mod ids;
pub mod input_action;
pub mod input_mapping;
pub mod logic_executor;
pub mod m2_rule_demo;
pub mod math;
pub mod minimal_renderer;
pub mod particle_effect;
#[cfg(feature = "real-wgpu")]
pub mod particle_gpu;
#[cfg(feature = "real-wgpu")]
pub mod particle_render;
pub mod particle_render_contract;
pub mod physics2d;
pub mod project_logic;
pub mod project_observation;
pub mod project_rule_asset;
pub mod project_runtime_module;
pub mod project_runtime_native_adapter;
pub mod project_runtime_session;
pub mod projection;
pub mod query;
pub mod release_package_manifest;
pub mod render_asset_bridge;
pub mod render_asset_production;
pub mod render_command;
pub mod render_extract;
pub mod render_graph;
pub mod render_graph_report;
pub mod render_resource;
pub mod render_snapshot;
pub mod render_state;
pub mod render_thread;
pub mod render_thread_worker;
pub mod renderer_feature_builder;
pub mod rhi_command_plan;
pub mod rule_artifact;
pub mod rule_compiler;
pub mod rule_ir;
pub mod rule_registry;
pub mod runtime_asset;
pub mod runtime_asset_diagnostics;
pub mod runtime_asset_loader;
pub mod runtime_audio;
mod runtime_entity_hydration;
pub mod runtime_instance;
pub mod runtime_instance_diagnostics;
pub mod runtime_instance_loader;
pub mod runtime_package;
pub mod runtime_package_builder;
pub mod runtime_package_path;
pub mod runtime_particles;
pub mod runtime_renderer;
pub mod runtime_run;
pub mod runtime_scene_hydration;
pub mod runtime_texture;
pub mod runtime_time;
pub mod runtime_trace;
pub mod scene_loader;
pub mod sprite2d_render_pipeline;
pub mod text_shaping;
pub mod visual_issue;
pub mod wgpu_backend;
pub mod windowed_continuous_runtime;
pub mod windowed_player;
pub mod world;
pub mod world_api;

pub const ENGINE_RUNTIME_NAME: &str = "engine_runtime";

pub fn runtime_smoke_value() -> u32 {
    7
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EngineRuntimeApiInfo {
    pub struct_size: u32,
    pub api_major: u32,
    pub api_minor: u32,
    pub capabilities: u64,
}

pub fn engine_runtime_api_info_v1() -> EngineRuntimeApiInfo {
    EngineRuntimeApiInfo {
        struct_size: std::mem::size_of::<EngineRuntimeApiInfo>() as u32,
        api_major: 1,
        api_minor: 0,
        capabilities: 0b1111,
    }
}

pub fn engine_runtime_start_v1() -> i32 {
    0
}

pub fn engine_runtime_stop_v1() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_smoke_test_passes() {
        assert_eq!(ENGINE_RUNTIME_NAME, "engine_runtime");
        assert_eq!(runtime_smoke_value(), 7);
    }
}
