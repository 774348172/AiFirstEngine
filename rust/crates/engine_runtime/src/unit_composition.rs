extern crate self as engine_runtime;
#[path = "../../engine_runtime_core/src/character_animation.rs"]
pub mod character_animation;
#[path = "../../engine_runtime_core/src/physics_3d/mod.rs"]
pub mod physics_3d;
#[path = "../../engine_runtime_core/src/navigation_3d/mod.rs"]
pub mod navigation_3d;
#[path = "../../engine_runtime_core/src/animator2d/mod.rs"]
pub mod animator2d;
#[path = "../../engine_runtime_core/src/archetype.rs"]
pub mod archetype;
#[path = "../../engine_runtime_core/src/geometry_upload.rs"]
pub mod geometry_upload;
#[path = "../../engine_runtime_core/src/spine_animation.rs"]
pub mod spine_animation;
pub use engine_content::atomic_directory_publish;
pub use engine_content::atomic_file_replace;
#[path = "../../engine_runtime_core/src/audio.rs"]
pub mod audio;
#[path = "../../engine_runtime_core/src/aui.rs"]
pub mod aui;
#[path = "../../engine_runtime_core/src/aui_animation.rs"]
pub mod aui_animation;
#[path = "../../engine_runtime_core/src/aui_control_feedback.rs"]
pub mod aui_control_feedback;
#[path = "../../engine_runtime_core/src/aui_extension.rs"]
pub mod aui_extension;
#[path = "../../engine_runtime_core/src/aui_feedback_cache.rs"]
mod aui_feedback_cache;
#[path = "../../engine_runtime_core/src/aui_geometry_cache.rs"]
mod aui_geometry_cache;
#[path = "../../engine_runtime_core/src/aui_graphics.rs"]
pub mod aui_graphics;
#[path = "../../engine_runtime_core/src/aui_graphics_asset.rs"]
pub mod aui_graphics_asset;
#[path = "../../engine_runtime_core/src/aui_hit_test_index.rs"]
mod aui_hit_test_index;
#[path = "../../engine_runtime_core/src/aui_instance.rs"]
pub mod aui_instance;
#[path = "../../engine_runtime_core/src/aui_layout_runtime.rs"]
pub mod aui_layout_runtime;
#[path = "../../engine_runtime_core/src/aui_layout_schema.rs"]
pub mod aui_layout_schema;
#[path = "../../engine_runtime_core/src/aui_paragraph.rs"]
pub mod aui_paragraph;
#[path = "../../engine_runtime_core/src/aui_present_cache.rs"]
pub mod aui_present_cache;
#[path = "../../engine_runtime_core/src/aui_retained.rs"]
pub mod aui_retained;
#[path = "../../engine_runtime_core/src/aui_session.rs"]
pub mod aui_session;
#[path = "../../engine_runtime_core/src/aui_theme.rs"]
pub mod aui_theme;
#[path = "../../engine_runtime_core/src/aui_viewport.rs"]
pub mod aui_viewport;
#[path = "../../engine_runtime_core/src/aui_virtual_list.rs"]
pub mod aui_virtual_list;
pub use engine_content::canonical_digest;
#[path = "../../engine_runtime_core/src/color_pipeline.rs"]
pub mod color_pipeline;
#[path = "../../engine_runtime_core/src/compiled_rule_slot.rs"]
pub mod compiled_rule_slot;
#[path = "../../engine_runtime_core/src/component_value.rs"]
pub mod component_value;
#[path = "../../engine_runtime_core/src/components.rs"]
pub mod components;
#[path = "../../engine_runtime_core/src/default_game_run.rs"]
pub mod default_game_run;
#[path = "../../engine_runtime_core/src/diagnostics.rs"]
pub mod diagnostics;
#[path = "../../engine_runtime_core/src/domain/mod.rs"]
pub mod domain;
#[path = "../../engine_runtime_core/src/engine_host_loop.rs"]
pub mod engine_host_loop;
#[path = "../../engine_runtime_core/src/engine_rhi.rs"]
pub mod engine_rhi;
#[path = "../../engine_runtime_core/src/environment_lighting.rs"]
pub mod environment_lighting;
#[path = "../../engine_runtime_core/src/field_path.rs"]
pub mod field_path;
pub use engine_text::font_bundle;
pub use engine_text::font_generation;
#[path = "../../engine_runtime_core/src/frame_hash.rs"]
pub mod frame_hash;
#[path = "../../engine_runtime_core/src/frame_loop.rs"]
pub mod frame_loop;
#[path = "../../engine_runtime_core/src/game_view_presentation.rs"]
pub mod game_view_presentation;
#[path = "../../engine_runtime_core/src/gameplay_command.rs"]
pub mod gameplay_command;
#[path = "../../engine_runtime_core/src/gameplay_rule_report.rs"]
pub mod gameplay_rule_report;
#[path = "../../engine_runtime_core/src/gameplay_trace.rs"]
pub mod gameplay_trace;
#[path = "../../engine_runtime_core/src/golden.rs"]
pub mod golden;
#[cfg(feature = "real-wgpu")]
#[path = "../../engine_render_native/src/gpu_frame_measurement.rs"]
pub mod gpu_frame_measurement;
#[path = "../../engine_runtime_core/src/gpu_texture_lifetime.rs"]
pub mod gpu_texture_lifetime;
#[path = "../../engine_runtime_core/src/headless_rhi_backend.rs"]
pub mod headless_rhi_backend;
#[path = "../../engine_runtime_core/src/ids.rs"]
pub mod ids;
#[path = "../../engine_runtime_core/src/input_action.rs"]
pub mod input_action;
#[path = "../../engine_runtime_core/src/input_mapping.rs"]
pub mod input_mapping;
#[path = "../../engine_runtime_core/src/logic_executor.rs"]
pub mod logic_executor;
#[path = "../../engine_runtime_core/src/m2_rule_demo.rs"]
pub mod m2_rule_demo;
#[path = "../../engine_runtime_core/src/math.rs"]
pub mod math;
#[path = "../../engine_runtime_core/src/minimal_renderer.rs"]
pub mod minimal_renderer;
#[path = "../../engine_runtime_core/src/particle_effect.rs"]
pub mod particle_effect;
#[cfg(feature = "real-wgpu")]
#[path = "../../engine_render_native/src/particle_gpu.rs"]
pub mod particle_gpu;
#[cfg(feature = "real-wgpu")]
#[path = "../../engine_render_native/src/particle_render.rs"]
pub mod particle_render;
#[path = "../../engine_runtime_core/src/particle_render_contract.rs"]
pub mod particle_render_contract;
#[path = "../../engine_runtime_core/src/physics2d.rs"]
pub mod physics2d;
#[path = "../../engine_runtime_core/src/project_logic.rs"]
pub mod project_logic;
#[path = "../../engine_runtime_core/src/project_observation.rs"]
pub mod project_observation;
#[path = "../../engine_runtime_core/src/project_rule_asset.rs"]
pub mod project_rule_asset;
#[path = "../../engine_runtime_core/src/project_runtime_module.rs"]
pub mod project_runtime_module;
#[path = "../../engine_runtime_core/src/project_runtime_native_adapter.rs"]
pub mod project_runtime_native_adapter;
#[path = "../../engine_runtime_core/src/project_runtime_session.rs"]
pub mod project_runtime_session;
#[path = "../../engine_runtime_core/src/projection.rs"]
pub mod projection;
#[path = "../../engine_runtime_core/src/query.rs"]
pub mod query;
pub use engine_content::release_package_manifest;
#[path = "../../engine_runtime_core/src/render_asset_bridge.rs"]
pub mod render_asset_bridge;
#[path = "../../engine_runtime_core/src/render_asset_production.rs"]
pub mod render_asset_production;
#[path = "../../engine_runtime_core/src/render_command.rs"]
pub mod render_command;
#[path = "../../engine_runtime_core/src/render_contract.rs"]
pub mod render_contract;
#[path = "../../engine_runtime_core/src/render_extract.rs"]
pub mod render_extract;
#[path = "../../engine_runtime_core/src/render_geometry.rs"]
pub mod render_geometry;
#[path = "../../engine_runtime_core/src/render_graph.rs"]
pub mod render_graph;
#[path = "../../engine_runtime_core/src/render_graph_report.rs"]
pub mod render_graph_report;
#[path = "../../engine_runtime_core/src/render_projection_adapter.rs"]
pub mod render_projection_adapter;
pub use engine_render_resources::render_resource;
#[path = "../../engine_runtime_core/src/render_snapshot.rs"]
pub mod render_snapshot;
#[path = "../../engine_runtime_core/src/render_state.rs"]
pub mod render_state;
#[path = "../../engine_runtime_core/src/render_thread.rs"]
pub mod render_thread;
#[path = "../../engine_runtime_core/src/render_thread_worker.rs"]
pub mod render_thread_worker;
#[path = "../../engine_runtime_core/src/renderer_feature_builder.rs"]
pub mod renderer_feature_builder;
#[path = "../../engine_runtime_core/src/resource_memory.rs"]
pub mod resource_memory;
#[path = "../../engine_runtime_core/src/rhi_command_plan.rs"]
pub mod rhi_command_plan;
#[path = "../../engine_runtime_core/src/rule_artifact.rs"]
pub mod rule_artifact;
#[path = "../../engine_runtime_core/src/rule_compiler.rs"]
pub mod rule_compiler;
#[path = "../../engine_runtime_core/src/rule_ir.rs"]
pub mod rule_ir;
#[path = "../../engine_runtime_core/src/rule_registry.rs"]
pub mod rule_registry;
#[path = "../../engine_runtime_core/src/runtime_asset.rs"]
pub mod runtime_asset;
#[path = "../../engine_runtime_core/src/runtime_asset_diagnostics.rs"]
pub mod runtime_asset_diagnostics;
#[path = "../../engine_runtime_core/src/runtime_asset_loader.rs"]
pub mod runtime_asset_loader;
#[path = "../../engine_runtime_core/src/runtime_audio.rs"]
pub mod runtime_audio;
#[path = "../../engine_runtime_core/src/runtime_entity_hydration.rs"]
mod runtime_entity_hydration;
#[path = "../../engine_runtime_core/src/runtime_instance.rs"]
pub mod runtime_instance;
#[path = "../../engine_runtime_core/src/runtime_instance_diagnostics.rs"]
pub mod runtime_instance_diagnostics;
#[path = "../../engine_runtime_core/src/runtime_instance_loader.rs"]
pub mod runtime_instance_loader;
#[path = "../../engine_runtime_core/src/runtime_package.rs"]
pub mod runtime_package;
#[path = "../../engine_runtime_core/src/runtime_package_builder.rs"]
pub mod runtime_package_builder;
pub use engine_content::runtime_package_path;
#[path = "../../engine_runtime_core/src/runtime_particles.rs"]
pub mod runtime_particles;
#[path = "../../engine_runtime_core/src/runtime_renderer.rs"]
pub mod runtime_renderer;
#[path = "../../engine_runtime_core/src/runtime_run.rs"]
pub mod runtime_run;
#[path = "../../engine_runtime_core/src/runtime_scene_hydration.rs"]
pub mod runtime_scene_hydration;
#[path = "../../engine_runtime_core/src/runtime_texture.rs"]
pub mod runtime_texture;
#[path = "../../engine_runtime_core/src/runtime_time.rs"]
pub mod runtime_time;
#[path = "../../engine_runtime_core/src/runtime_trace.rs"]
pub mod runtime_trace;
#[path = "../../engine_runtime_core/src/scene_loader.rs"]
pub mod scene_loader;
#[path = "../../engine_runtime_core/src/spatial_contract.rs"]
pub mod spatial_contract;
#[path = "../../engine_runtime_core/src/sprite2d_render_pipeline.rs"]
pub mod sprite2d_render_pipeline;
pub use engine_text::text_shaping;
#[path = "../../engine_runtime_core/src/visual_issue.rs"]
pub mod visual_issue;
#[path = "../../engine_render_native/src/wgpu_backend.rs"]
pub mod wgpu_backend;
#[path = "../../engine_runtime_core/src/windowed_continuous_runtime.rs"]
pub mod windowed_continuous_runtime;
#[path = "../../engine_runtime_core/src/windowed_player.rs"]
pub mod windowed_player;
#[path = "../../engine_runtime_core/src/world.rs"]
pub mod world;
#[path = "../../engine_runtime_core/src/world_api.rs"]
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

pub use engine_content::runtime_package_source;
pub use engine_runtime_core::platform_time;

#[path = "../../engine_runtime_core/src/world_streaming.rs"]
pub mod world_streaming;

#[path = "../../engine_runtime_core/src/world_persistence.rs"]
pub mod world_persistence;
#[path = "../../engine_runtime_core/src/terrain3d.rs"]
pub mod terrain3d;

#[path = "../../engine_runtime_core/src/model_lod3d.rs"]
pub mod model_lod3d;
