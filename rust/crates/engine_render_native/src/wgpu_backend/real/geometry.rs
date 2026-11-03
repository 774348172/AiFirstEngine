//! Real GPU consumption of the neutral geometry contract, inside the existing backend.
use super::*;
use crate::render_contract::*;
use crate::render_geometry::{DrawGeometry, GeometryBuffers};
#[cfg(test)]
#[cfg_attr(test, path = "geometry/batch_tests.rs")]
mod batch_tests;
#[cfg_attr(test, path = "geometry/encoding.rs")]
mod encoding;
#[cfg_attr(test, path = "geometry/environment.rs")]
mod environment;
#[cfg_attr(test, path = "geometry/ibl.rs")]
mod ibl;
#[cfg_attr(test, path = "geometry/lights.rs")]
mod lights;
#[cfg(test)]
#[cfg_attr(test, path = "geometry/lights_tests.rs")]
mod lights_tests;
#[cfg_attr(test, path = "geometry/lit.rs")]
mod lit;
mod moving_views;
#[cfg_attr(test, path = "geometry/native_template.rs")]
mod native_template;
#[cfg_attr(test, path = "geometry/pages.rs")]
mod pages;
#[cfg_attr(test, path = "geometry/parameters.rs")]
mod parameters;
#[cfg_attr(test, path = "geometry/preparation.rs")]
mod preparation;
#[cfg(test)]
mod selection_tests;
mod template_backing;
use parameters::{
    INSTANCE_ARENA_BYTES, INSTANCE_BYTES, INSTANCE_FLOATS, VIEW_ARENA_BYTES, VIEW_BYTES,
    VIEW_FLOATS,
};
mod gpu_scale;
#[cfg_attr(test, path = "geometry/post.rs")]
pub(super) mod post;
#[cfg_attr(test, path = "geometry/residency.rs")]
mod residency;
#[cfg_attr(test, path = "geometry/shadows.rs")]
mod shadows;
mod skin;
#[cfg_attr(test, path = "geometry/streaming.rs")]
mod streaming;
#[cfg_attr(test, path = "geometry/targets.rs")]
pub(super) mod targets;
pub use streaming::{
    GeometryUploadBudget, GeometryUploadSnapshot, GeometryUploadState, GeometryUploadTicket,
};

const MAX_VISUAL_RESIDENT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

struct ResidentGeometry {
    source: Arc<GeometryBuffers>,
    vertices: pages::PageRange,
    indices: pages::PageRange,
}

#[cfg(test)]
thread_local! {
    static GEOMETRY_VIEWPORT_CHECKS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static DRAW_GROUP_VISITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static COMMAND_PROOF_VISITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static COMMAND_VALUE_COMPARISONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static NATIVE_DRAW_RECORDINGS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static NATIVE_PIPELINE_BINDINGS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static NATIVE_VIEW_COMPARISONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static PIPELINE_STAGE_OBSERVATIONS: std::cell::RefCell<Vec<(bool, Vec<u32>, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
    static PARAMETER_PREPARATION_THREAD: std::cell::Cell<Option<std::thread::ThreadId>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
mod retained_plan_identity_tests {
    use super::*;

    #[test]
    #[ignore = "requires a real GPU"]
    fn moving_view_reuse_accepts_scaled_logical_viewport_and_rejects_invalid_view() {
        let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(4),
        );
        let extent = [320, 240];
        let rect = Some(GameViewRect {
            x: 0.,
            y: 0.,
            width: 320.,
            height: 240.,
        });
        let mut backend = RealWgpuBackend::new_offscreen(320, 240).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, extent, rect)
            .unwrap();
        let visits = backend.retained_geometry_stats().validation_draw_visits;
        for command in &mut plan.commands {
            match command {
                RhiCommand::BeginView { view, .. } => {
                    let view = Arc::make_mut(view);
                    view.view_matrix[12] = 0.25f32.into();
                    view.version += 1;
                }
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => {
                    let view = Arc::make_mut(&mut Arc::make_mut(draw).view);
                    view.view_matrix[12] = 0.25f32.into();
                    view.version += 1;
                }
                _ => {}
            }
        }
        backend
            .prepare_geometry_plan_for_extent(&plan, extent, rect)
            .unwrap();
        assert_eq!(
            backend.retained_geometry_stats().validation_draw_visits,
            visits,
            "a valid logical viewport scaled into the surface must reuse admission"
        );
        for command in &mut plan.commands {
            match command {
                RhiCommand::BeginView { view, .. } => {
                    Arc::make_mut(view).projection_matrix[0] = 1.1f32.into()
                }
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => {
                    Arc::make_mut(&mut Arc::make_mut(draw).view).projection_matrix[0] =
                        1.1f32.into();
                }
                _ => {}
            }
        }
        backend
            .prepare_geometry_plan_for_extent(&plan, extent, rect)
            .unwrap();
        assert!(
            backend.retained_geometry_stats().validation_draw_visits > visits,
            "projection changes must invalidate the moving-view skeleton"
        );
        for command in &mut plan.commands {
            match command {
                RhiCommand::BeginView { view, .. } => Arc::make_mut(view).viewport[2] = 641,
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => {
                    Arc::make_mut(&mut Arc::make_mut(draw).view).viewport[2] = 641;
                }
                _ => {}
            }
        }
        assert!(backend
            .prepare_geometry_plan_for_extent(&plan, extent, rect)
            .is_err());
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn resident_intent_preserves_registry_and_counters_across_moving_views_and_binding_changes() {
        use crate::wgpu_backend::GEOMETRY_INTENT_VALUE_VISITS;
        let mut costs = Vec::new();
        for count in [4, 32, 256] {
            let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
                &crate::rhi_command_plan::tests::retained_geometry_graph(count),
            );
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend.state.target_context.format = "Rgba8Unorm".into();
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            for (index, command) in plan.commands.iter().enumerate() {
                if let RhiCommand::Draw { payload, .. } = command {
                    backend.record_current_geometry_intent(&plan.commands, index, payload);
                }
            }
            for command in &mut plan.commands {
                match command {
                    RhiCommand::BeginView { view, .. } => {
                        Arc::make_mut(view).view_matrix[12] = 0.25f32.into();
                    }
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => {
                        Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] =
                            0.25f32.into();
                    }
                    _ => (),
                }
            }
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            let mut reference = backend.state.clone();
            GEOMETRY_INTENT_VALUE_VISITS.with(|n| n.set(0));
            for (index, command) in plan.commands.iter().enumerate() {
                if let RhiCommand::Draw { payload, .. } = command {
                    backend.record_current_geometry_intent(&plan.commands, index, payload);
                }
            }
            let visits = GEOMETRY_INTENT_VALUE_VISITS.with(|n| n.get());
            for command in &plan.commands {
                if let RhiCommand::Draw { payload, .. } = command {
                    reference.record_draw(payload);
                }
            }
            assert_eq!(backend.state, reference);
            costs.push((count, visits));
            assert_eq!(visits, 0, "camera-only change revisited stable CPU intent");
            let last = plan
                .commands
                .iter_mut()
                .rev()
                .find_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .unwrap();
            let draw = Arc::make_mut(last);
            Arc::make_mut(&mut draw.geometry_binding).binding_id = "renamed-mesh".into();
            Arc::make_mut(&mut draw.material_binding).binding_id = "renamed-material".into();
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            let mut reference = backend.state.clone();
            GEOMETRY_INTENT_VALUE_VISITS.with(|n| n.set(0));
            for (index, command) in plan.commands.iter().enumerate() {
                if let RhiCommand::Draw { payload, .. } = command {
                    backend.record_current_geometry_intent(&plan.commands, index, payload);
                }
            }
            assert!(GEOMETRY_INTENT_VALUE_VISITS.with(|n| n.get()) > 0);
            for command in &plan.commands {
                if let RhiCommand::Draw { payload, .. } = command {
                    reference.record_draw(payload);
                }
            }
            assert_eq!(
                backend.state, reference,
                "binding names disappeared from the CPU registry"
            );
            backend.state.target_context.format = "Rgba16Float".into();
            let mut reference = backend.state.clone();
            for (index, command) in plan.commands.iter().enumerate() {
                if let RhiCommand::Draw { payload, .. } = command {
                    backend.record_current_geometry_intent(&plan.commands, index, payload);
                    reference.record_draw(payload);
                }
            }
            assert_eq!(
                backend.state, reference,
                "format changes borrowed another intent proof"
            );
        }
        eprintln!("resident CPU intent actual value visits {costs:?}");
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn combined_parameter_preparation_joins_restores_and_retries_without_gpu_commit_on_failure() {
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(600),
        );
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        let mut changed = plan.clone();
        let changed_draw = changed
            .commands
            .iter_mut()
            .rev()
            .find_map(|c| match c {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some(draw),
                _ => None,
            })
            .unwrap();
        Arc::make_mut(changed_draw).base_color[0] = 0.25f32.into();
        let mut invalid = changed.clone();
        let invalid_draw = invalid
            .commands
            .iter_mut()
            .rev()
            .find_map(|c| match c {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some(draw),
                _ => None,
            })
            .unwrap();
        Arc::make_mut(invalid_draw).camera_depth = f32::NAN.into();
        let content = backend
            .geometry
            .parameters
            .as_ref()
            .unwrap()
            .instances
            .content
            .clone();
        let before = backend.retained_geometry_stats();
        PARAMETER_PREPARATION_THREAD.with(|n| n.set(None));
        assert!(backend
            .prepare_geometry_plan_for_extent(&invalid, [640, 480], None)
            .is_err());
        assert_eq!(
            backend
                .geometry
                .parameters
                .as_ref()
                .unwrap()
                .instances
                .content,
            content
        );
        assert_eq!(
            backend.retained_geometry_stats().parameter_uploaded_bytes,
            before.parameter_uploaded_bytes
        );
        assert_ne!(
            PARAMETER_PREPARATION_THREAD.with(|n| n.get()).unwrap(),
            std::thread::current().id()
        );
        // Unbound or divergent draw views must keep the original native error,
        // rather than exhausting CPU record capacity on the parallel branch.
        let mut invalid_views = changed.clone();
        for (index, command) in invalid_views.commands.iter_mut().enumerate() {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } = command
            {
                Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] =
                    ((index + 1) as f32 * 0.0001f32).into();
            }
        }
        let mut cold = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let original_error = cold
            .prepare_geometry_plan_for_extent(&invalid_views, [640, 480], None)
            .unwrap_err();
        PARAMETER_PREPARATION_THREAD.with(|n| n.set(None));
        assert_eq!(
            backend
                .prepare_geometry_plan_for_extent(&invalid_views, [640, 480], None)
                .unwrap_err(),
            original_error
        );
        assert_eq!(PARAMETER_PREPARATION_THREAD.with(|n| n.get()), None);
        assert_eq!(
            backend
                .geometry
                .parameters
                .as_ref()
                .unwrap()
                .instances
                .content,
            content
        );
        backend
            .prepare_geometry_plan_for_extent(&changed, [640, 480], None)
            .unwrap();
        assert_eq!(
            backend.retained_geometry_stats().parameter_uploaded_bytes
                - before.parameter_uploaded_bytes,
            INSTANCE_BYTES
        );
        let actual = backend
            .render_plan_to_rgba_bytes(&changed, 640, 480)
            .unwrap();
        let mut reference = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let expected = reference
            .render_plan_to_rgba_bytes(&changed, 640, 480)
            .unwrap();
        assert_eq!(
            actual, expected,
            "failed preparation changed the subsequent real GPU result"
        );
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_parameter_layers_share_instances_and_update_only_changed_view_data() {
        let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(1),
        );
        let mut second = plan
            .commands
            .iter()
            .find(|c| geometry_draw(c).is_some())
            .unwrap()
            .clone();
        if let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut second
        {
            let view = Arc::make_mut(&mut Arc::make_mut(draw).view);
            view.view_id += 1;
            view.view_matrix[12] = 0.25f32.into();
        }
        plan.commands.push(second);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend.prepare_geometry_parameters(&plan).unwrap();
        let before = backend.retained_geometry_stats();
        assert_eq!(
            before.parameter_uploaded_bytes,
            128 + 2 * 176,
            "same instance parameters uploaded once per view instead of shared"
        );
        for command in &mut plan.commands {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } = command
            {
                Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[13] = 0.5f32.into();
            }
        }
        backend.prepare_geometry_parameters(&plan).unwrap();
        assert_eq!(
            backend.retained_geometry_stats().parameter_uploaded_bytes
                - before.parameter_uploaded_bytes,
            2 * 176,
            "camera-only update rewrote shared instance parameters"
        );
        let before = backend.retained_geometry_stats();
        let second = plan
            .commands
            .iter_mut()
            .rev()
            .find_map(|command| match command {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some(draw),
                _ => None,
            })
            .unwrap();
        Arc::make_mut(second).world_transform[12] = 2f32.into();
        Arc::make_mut(second).base_color[0] = 0.25f32.into();
        backend.prepare_geometry_parameters(&plan).unwrap();
        assert_eq!(
            backend.retained_geometry_stats().parameter_uploaded_bytes
                - before.parameter_uploaded_bytes,
            128,
            "one changed instance rewrote unchanged view data"
        );
        let indices = &backend.geometry.parameters.as_ref().unwrap().indices;
        assert_ne!(
            indices[0][0], indices[1][0],
            "same ID with different origin/material values was merged"
        );
        assert_ne!(
            indices[0][1], indices[1][1],
            "different view matrices were merged"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_opaque_shadow_pipeline_skips_fragment_and_nonposition_inputs() {
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(1),
        );
        let mut draw = plan
            .commands
            .iter()
            .find_map(geometry_draw)
            .unwrap()
            .clone();
        draw.geometry = Arc::new(
            serde_json::from_value(serde_json::json!({
                "vertexBytes": vec![0u8; 108], "indexBytes": [0,0,1,0,2,0],
                "vertexStride": 36, "vertexCount": 3, "indexCount": 3, "indexFormat": "uint16",
                "attributes": [
                    {"semantic":"position","format":"float32x3","offset":0},
                    {"semantic":"color0","format":"float32x4","offset":12},
                    {"semantic":"texCoord0","format":"float32x2","offset":28}
                ]
            }))
            .unwrap(),
        );
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        // Observe the actual descriptor passed to wgpu, not a parallel recipe.
        PIPELINE_STAGE_OBSERVATIONS.with(|v| v.borrow_mut().clear());
        for shadow in [false, true] {
            let view = Arc::make_mut(&mut draw.view);
            view.view_kind = if shadow {
                ViewKind::Shadow
            } else {
                ViewKind::Game
            };
            view.target_kind = if shadow {
                TargetKind::ShadowMap
            } else {
                TargetKind::Window
            };
            draw.depth = MaterialDepth::TestAndWrite;
            draw.depth_target = Some("depth".into());
            for blend in [MaterialBlend::Opaque, MaterialBlend::AlphaMask] {
                draw.blend = blend;
                // StandardLit used to add an unused texture group even when
                // the actual shader was the position-only opaque writer.
                draw.standard_lit = (shadow && blend == MaterialBlend::Opaque).then(|| {
                    crate::render_contract::StandardLitParameters {
                        surface: [0f32, 0.5, 1., 1.].map(Into::into),
                        emissive: [0f32; 4].map(Into::into),
                    }
                });
                drop(geometry_pipeline(
                    &backend.device,
                    wgpu::TextureFormat::Rgba8Unorm,
                    &backend.geometry.parameter_layout,
                    &backend.texture_bind_group_layout,
                    &backend.geometry.light_layout,
                    None,
                    &draw,
                ));
            }
        }
        PIPELINE_STAGE_OBSERVATIONS.with(|v| {
            let observations = v.borrow();
            assert_eq!(observations.len(), 4);
            assert!(
                observations[0].0 && observations[1].0,
                "color requires fragment output"
            );
            assert_eq!(observations[0].1, [0, 1, 2]);
            assert!(
                !observations[2].0,
                "opaque shadow still creates a fragment stage"
            );
            assert_eq!(
                observations[2].1,
                [0],
                "opaque shadow still fetches nonposition inputs"
            );
            assert_eq!(
                observations[2].2, 1,
                "opaque depth must bind only parameters"
            );
            assert!(observations[3].0, "mask must retain fragment alpha discard");
            assert_eq!(observations[3].1, [0, 1, 2]);
        });
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    fn retained_capture_borrows_live_commands_without_cloning_each_draw_descriptor() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(128);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let first = plan.commands.iter().find_map(geometry_draw).unwrap();
        let before = Arc::strong_count(&first.geometry);
        let cached =
            ValidatedGeometryPlan::capture(&plan, [64, 64], None, wgpu::TextureFormat::Rgba8Unorm)
                .unwrap();
        assert_eq!(
            Arc::strong_count(&first.geometry),
            before + 1,
            "capture cloned every draw descriptor in addition to its unique source closure"
        );
        let replay: crate::render_graph::SharedRenderList<RhiCommand> =
            serde_json::from_value(serde_json::to_value(&plan.commands).unwrap()).unwrap();
        assert!(cached.matches_commands(&replay), "live value fallback lost");
        let lease = Arc::downgrade(&first.resource_lease);
        drop(plan);
        drop(graph);
        assert!(lease.upgrade().is_none(), "capture retained native lease");
        assert!(
            !cached.matches_commands(&replay),
            "expired command backing must be a cache miss, never an unproven hit"
        );
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn repeated_detached_mesh_sources_keep_current_plan_and_resident_conflicts_visible() {
        let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(128),
        );
        let first = plan.commands.iter().find_map(geometry_draw).unwrap();
        let wire = serde_json::to_value(first.geometry.as_ref()).unwrap();
        let detached = Arc::new(serde_json::from_value::<GeometryBuffers>(wire.clone()).unwrap());
        for (index, command) in plan.commands.iter_mut().enumerate() {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } = command
            {
                if index % 2 == 0 {
                    Arc::make_mut(draw).geometry = detached.clone();
                }
            }
        }
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        let uploads = backend.geometry_upload_count();
        let mut changed = plan.clone();
        let mut conflict = wire;
        for (i, byte) in 0.125f32.to_le_bytes().iter().enumerate() {
            conflict["vertexBytes"][i] = serde_json::json!(*byte);
        }
        let conflict = Arc::new(serde_json::from_value::<GeometryBuffers>(conflict).unwrap());
        for command in changed.commands.iter_mut().rev() {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } = command
            {
                Arc::make_mut(draw).geometry = conflict;
                break;
            }
        }
        assert_eq!(
            backend.prepare_geometry_plan_for_extent(&changed, [640, 480], None),
            Err("geometry.resource_identity_conflict".into())
        );
        assert_eq!(
            backend.geometry_upload_count(),
            uploads,
            "conflict changed residency before rejection"
        );
        let mut fresh = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        assert_eq!(
            fresh.prepare_geometry_plan_for_extent(&changed, [640, 480], None),
            Err("geometry.resource_identity_conflict".into())
        );
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        fresh
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn repeated_draws_consume_begin_view_viewport_proof_and_reject_changed_views() {
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(128),
        );
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        GEOMETRY_VIEWPORT_CHECKS.with(|n| n.set(0));
        NATIVE_VIEW_COMPARISONS.with(|n| n.set(0));
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        assert_eq!(
            GEOMETRY_VIEWPORT_CHECKS.with(|n| n.get()),
            1,
            "unchanged draw view revalidated its viewport per instance"
        );
        assert_eq!(
            NATIVE_VIEW_COMPARISONS.with(|n| n.get()),
            1,
            "unchanged draw view compared all values per instance"
        );
        let mut changed = plan.clone();
        let draw = changed
            .commands
            .iter_mut()
            .rev()
            .find_map(|command| match command {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some(draw),
                _ => None,
            })
            .unwrap();
        Arc::make_mut(&mut Arc::make_mut(draw).view).viewport[2] -= 1;
        assert!(backend
            .prepare_geometry_plan_for_extent(&changed, [640, 480], None)
            .unwrap_err()
            .contains("invalid_draw_view"));
        let mut other_target = plan.clone();
        let (target, draw) = other_target
            .commands
            .iter_mut()
            .rev()
            .find_map(|command| match command {
                RhiCommand::Draw {
                    target,
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some((target, draw)),
                _ => None,
            })
            .unwrap();
        *target = "other-target".into();
        Arc::make_mut(draw).target = "other-target".into();
        let error = backend
            .prepare_geometry_plan_for_extent(&other_target, [640, 480], None)
            .unwrap_err();
        assert_eq!(
            error, "geometry.output_resource_missing",
            "undeclared target must fail in output admission before draw matching"
        );
        let draw = plan.commands.iter().find_map(geometry_draw).unwrap();
        let other_target = "other-target".to_string();
        assert!(
            !matching_draw_attachments(
                (
                    draw.target.as_ref(),
                    draw.view.as_ref(),
                    draw.depth_target.as_deref()
                ),
                (
                    &other_target,
                    draw.view.as_ref(),
                    draw.depth_target.as_deref()
                ),
            ),
            "same view hid a changed target attachment"
        );
        let other_depth = Some("other-depth".to_string());
        assert!(
            !matching_draw_attachments(
                (
                    draw.target.as_ref(),
                    draw.view.as_ref(),
                    draw.depth_target.as_deref()
                ),
                (&draw.target, draw.view.as_ref(), other_depth.as_deref()),
            ),
            "same view hid a changed depth attachment"
        );
        let mut invalid = plan.clone();
        let begin = invalid
            .commands
            .iter_mut()
            .find_map(|command| match command {
                RhiCommand::BeginView { view, .. } => Some(view),
                _ => None,
            })
            .unwrap();
        Arc::make_mut(begin).viewport[2] = 0;
        assert!(backend
            .prepare_geometry_plan_for_extent(&invalid, [640, 480], None)
            .is_err());
        backend
            .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
            .unwrap();
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    fn shadow_batch_ignores_only_opaque_depth_color_binding() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut a = plan
            .commands
            .iter()
            .find_map(|command| match command {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some(draw.as_ref().clone()),
                _ => None,
            })
            .unwrap();
        let mut b = a.clone();
        Arc::make_mut(&mut b.material_binding).binding_id = "different-color".into();
        assert!(!batch_compatible(
            &a,
            &b,
            &a.pipeline_identity("neutral"),
            "neutral"
        ));
        for draw in [&mut a, &mut b] {
            let view = Arc::make_mut(&mut draw.view);
            view.view_kind = ViewKind::Shadow;
            view.target_kind = TargetKind::ShadowMap;
            draw.depth = MaterialDepth::TestAndWrite;
        }
        assert!(batch_compatible(
            &a,
            &b,
            &a.pipeline_identity("neutral"),
            "neutral"
        ));
        b.depth = MaterialDepth::TestOnly;
        assert!(!batch_compatible(
            &a,
            &b,
            &a.pipeline_identity("neutral"),
            "neutral"
        ));
        b.depth = MaterialDepth::TestAndWrite;
        for draw in [&mut a, &mut b] {
            draw.blend = MaterialBlend::AlphaMask;
            draw.alpha_cutoff = Some(0.5f32.into());
        }
        assert!(!batch_compatible(
            &a,
            &b,
            &a.pipeline_identity("neutral"),
            "neutral"
        ));
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_opaque_shadow_prepare_skips_unused_fragment_bindings_but_mask_keeps_them() {
        for count in [4, 32, 256] {
            let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
                &crate::rhi_command_plan::tests::retained_geometry_graph(count),
            );
            let mut view = plan
                .commands
                .iter()
                .find_map(geometry_draw)
                .unwrap()
                .view
                .as_ref()
                .clone();
            view.view_kind = ViewKind::Shadow;
            view.target_kind = TargetKind::ShadowMap;
            view.depth_policy = DepthPolicy::TestAndWrite;
            view.clear_color = None;
            view.depth_clear = Some(1f32.into());
            let view = Arc::new(view);
            let geometry: Arc<GeometryBuffers> = Arc::new(
                serde_json::from_value(serde_json::json!({
                    "vertexBytes":vec![0u8;72],"indexBytes":[0,0,1,0,2,0],
                    "vertexStride":24,"vertexCount":3,"indexCount":3,"indexFormat":"uint16",
                    "attributes":[{"semantic":"position","format":"float32x3","offset":0},
                        {"semantic":"normal","format":"float32x3","offset":12}]
                }))
                .unwrap(),
            );
            for command in plan.commands.iter_mut() {
                match command {
                    RhiCommand::BeginView {
                        target,
                        view: declared,
                        depth_target,
                    } => {
                        *target = "opaque-depth".into();
                        *declared = view.clone();
                        *depth_target = Some(target.clone());
                    }
                    RhiCommand::Draw {
                        target,
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => {
                        *target = "opaque-depth".into();
                        let draw = Arc::make_mut(draw);
                        draw.view = view.clone();
                        draw.target = "opaque-depth".into();
                        draw.depth_target = Some(draw.target.clone());
                        draw.depth = MaterialDepth::TestAndWrite;
                        draw.geometry = geometry.clone();
                        draw.standard_lit = Some(StandardLitParameters {
                            surface: [0f32, 0.5, 1., 1.].map(Into::into),
                            emissive: [0f32; 4].map(Into::into),
                        });
                        draw.validate().unwrap();
                    }
                    _ => (),
                }
            }
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            // This checks actual preparation/allocation, not visible pixels. Existing
            // independent depth/alpha pixels cover the directly affected draw consumer.
            for command in plan.commands.iter_mut() {
                if let RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } = command
                {
                    let draw = Arc::make_mut(draw);
                    draw.blend = MaterialBlend::AlphaMask;
                    draw.alpha_cutoff = Some(0.5f32.into());
                }
            }
            lit::TEXTURE_KEY_COMPUTATIONS.with(|n| n.set(0));
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            assert_eq!(
                backend.geometry.lit_bindings.len(),
                1,
                "mask lost its fragment bindings"
            );
            assert_eq!(
                lit::TEXTURE_KEY_COMPUTATIONS.with(|n| n.get()),
                0,
                "native material consumer reinterpreted an already resolved requirement"
            );
            for command in plan.commands.iter_mut() {
                if let RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } = command
                {
                    let draw = Arc::make_mut(draw);
                    draw.blend = MaterialBlend::Opaque;
                    draw.alpha_cutoff = None;
                }
            }
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            assert!(
                backend.geometry.lit_bindings.is_empty(),
                "opaque depth retained fragment bindings never consumed by its shader"
            );
            for command in plan.commands.iter_mut() {
                if let RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } = command
                {
                    let draw = Arc::make_mut(draw);
                    draw.blend = MaterialBlend::AlphaMask;
                    draw.alpha_cutoff = Some(0.5f32.into());
                }
            }
            backend
                .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
                .unwrap();
            assert_eq!(
                backend.geometry.lit_bindings.len(),
                1,
                "mask did not recreate its actual fragment binding"
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
    }

    #[test]
    fn s2d_transfer_parameter_quota_failure_keeps_dirty_content_retryable() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let bytes = PARAMETER_BYTES as usize + parameters::INDEX_BYTES as usize;
        backend
            .set_geometry_upload_budget(GeometryUploadBudget {
                resident_bytes: 128 * 1024 * 1024,
                staging_bytes: 4096,
                bytes_per_frame: bytes,
                max_tickets: 8,
            })
            .unwrap();
        backend.bind_geometry_upload_presentation(1).unwrap();
        backend.geometry.admit_visual_upload(1, 4).unwrap();
        assert!(backend.prepare_geometry_parameters(&plan).is_err());
        assert_eq!(backend.geometry.stats.parameter_uploaded_bytes, 0);
        assert_eq!(backend.geometry.stats.index_uploaded_bytes, 0);
        backend.bind_geometry_upload_presentation(2).unwrap();
        backend.prepare_geometry_parameters(&plan).unwrap();
        assert_eq!(
            backend.geometry.stats.parameter_uploaded_bytes,
            PARAMETER_BYTES
        );
        assert_eq!(
            backend.geometry.stats.index_uploaded_bytes,
            parameters::INDEX_BYTES
        );
        assert_eq!(
            backend.geometry_upload_snapshot().frame_uploaded_bytes,
            bytes
        );
        backend.prepare_geometry_parameters(&plan).unwrap();
        assert_eq!(
            backend.geometry_upload_snapshot().frame_uploaded_bytes,
            bytes,
            "clean replay must not charge a second upload"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(backend.geometry_upload_snapshot().staging_bytes, 0);
    }

    #[test]
    fn s2d_transfer_large_parameter_page_advances_without_reuploading_clean_slots() {
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(4),
        );
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend
            .set_geometry_upload_budget(GeometryUploadBudget {
                resident_bytes: 128 * 1024 * 1024,
                staging_bytes: 4096,
                bytes_per_frame: PARAMETER_BYTES as usize + 8,
                max_tickets: 8,
            })
            .unwrap();
        for present in 1..=3 {
            backend.bind_geometry_upload_presentation(present).unwrap();
            let result = backend.prepare_geometry_parameters(&plan);
            if present < 3 {
                assert_eq!(result, Err("geometry.visual_upload_capacity".into()));
                assert_eq!(
                    backend.geometry.stats.parameter_uploaded_bytes,
                    VIEW_BYTES + (if present == 1 { 1 } else { 3 }) * INSTANCE_BYTES
                );
                assert_eq!(backend.geometry.stats.index_uploaded_bytes, 0);
            } else {
                result.unwrap();
                assert_eq!(
                    backend.geometry.stats.parameter_uploaded_bytes,
                    VIEW_BYTES + 4 * INSTANCE_BYTES
                );
                assert_eq!(
                    backend.geometry.stats.index_uploaded_bytes,
                    4 * parameters::INDEX_BYTES
                );
            }
            assert!(
                backend.geometry_upload_snapshot().frame_uploaded_bytes
                    <= PARAMETER_BYTES as usize + 8
            );
        }
        assert_eq!(
            backend.geometry_upload_snapshot().uploaded_bytes,
            VIEW_BYTES + 4 * INSTANCE_BYTES + 4 * parameters::INDEX_BYTES
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(backend.geometry_upload_snapshot().staging_bytes, 0);
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn static_mesh_loading_makes_bounded_progress_across_presentations() {
        for count in [16, 64, 256] {
            // Match the product case: the index table fits one allowance,
            // while the aggregate independent mesh bytes do not.
            let allowance = count * 32;
            let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                    let draw = Arc::make_mut(draw);
                    let instance = draw.instance_id;
                    let binding = Arc::make_mut(&mut draw.geometry_binding);
                    let BindingResource::Geometry(handle) = &mut binding.slots[0].resource else {
                        panic!()
                    };
                    *handle = GeometryHandle::try_from(RenderResourceHandle {
                        index: instance + 1,
                        ..handle.raw()
                    })
                    .unwrap();
                }
            }
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend
                .set_geometry_upload_budget(GeometryUploadBudget {
                    resident_bytes: 128 * 1024 * 1024,
                    staging_bytes: allowance * 2,
                    bytes_per_frame: allowance,
                    max_tickets: 8,
                })
                .unwrap();
            let target = backend.create_output_texture(640, 480).unwrap();
            let clear = RhiCommandPlan {
                frame_identity: None,
                graph_schedule: None,
                packet_content_version: None,
                frame_index: 0,
                graph_id: "static-loading".into(),
                target_kind: "surface".into(),
                commands: vec![RhiCommand::Clear {
                    target: "surface-main".into(),
                    color: [0f32.into(); 4],
                }]
                .into(),
                diagnostics: vec![],
            };
            let mut ready = false;
            let mut previous_uploads = 0;
            for presentation in 1..=count as u64 + 32 {
                backend
                    .bind_geometry_upload_presentation(presentation)
                    .unwrap();
                ready = backend
                    .prepare_surface_resources(&plan, [640, 480], None)
                    .unwrap();
                let uploaded = backend.geometry_upload_count();
                assert!(uploaded >= previous_uploads && uploaded <= count);
                if presentation == 1 {
                    assert_eq!(
                        uploaded,
                        allowance / 44,
                        "aggregate mesh quota rejection made no prefix progress at N={count}"
                    );
                }
                previous_uploads = uploaded;
                assert!(backend.geometry_upload_snapshot().frame_uploaded_bytes <= allowance);
                // Same-presentation retries never manufacture a new allowance.
                let before = backend.geometry_upload_snapshot().uploaded_bytes;
                let _ = backend
                    .prepare_surface_resources(&plan, [640, 480], None)
                    .unwrap();
                assert_eq!(backend.geometry_upload_snapshot().uploaded_bytes, before);
                backend
                    .execute_loading_clear_to_surface_view(
                        &clear,
                        &target.create_view(&Default::default()),
                    )
                    .unwrap();
                backend
                    .shutdown_gpu(std::time::Duration::from_secs(5))
                    .unwrap();
                assert_eq!(backend.geometry_upload_snapshot().staging_bytes, 0);
                if ready {
                    break;
                }
            }
            assert!(ready, "static mesh loading never completed at N={count}");
            assert_eq!(backend.geometry_upload_count(), count);
            assert_eq!(backend.geometry_residency().meshes, count);
            backend.release_output_texture(target);
        }
    }

    #[test]
    fn s2d_transfer_native_loading_clear_preserves_prepared_meshes_until_next_present() {
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(1),
        );
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .set_geometry_upload_budget(GeometryUploadBudget {
                resident_bytes: 128 * 1024 * 1024,
                staging_bytes: 4096,
                bytes_per_frame: PARAMETER_BYTES as usize + parameters::INDEX_BYTES as usize,
                max_tickets: 8,
            })
            .unwrap();
        backend.bind_geometry_upload_presentation(1).unwrap();
        assert!(!backend
            .prepare_surface_resources(&plan, [640, 480], None)
            .unwrap());
        assert_eq!(backend.geometry_residency().meshes, 1);
        let uploads = backend.geometry_upload_count();
        let target = backend.create_output_texture(640, 480).unwrap();
        let clear = RhiCommandPlan {
            frame_identity: None,
            graph_schedule: None,
            packet_content_version: None,
            frame_index: 0,
            graph_id: "loading".into(),
            target_kind: "surface".into(),
            commands: vec![RhiCommand::Clear {
                target: "main".into(),
                color: [OrderedF32::from(0.); 4],
            }]
            .into(),
            diagnostics: vec![],
        };
        let report = backend
            .execute_loading_clear_to_surface_view(&clear, &target.create_view(&Default::default()))
            .unwrap();
        assert!(report.diagnostics.is_empty(), "{report:?}");
        assert_eq!(backend.geometry_residency().meshes, 1);
        backend.bind_geometry_upload_presentation(2).unwrap();
        assert!(backend
            .prepare_surface_resources(&plan, [640, 480], None)
            .unwrap());
        assert_eq!(backend.geometry_upload_count(), uploads);
        assert_eq!(
            backend.geometry_upload_snapshot().frame_uploaded_bytes,
            PARAMETER_BYTES as usize + parameters::INDEX_BYTES as usize
        );
        backend.release_output_texture(target);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_unique_meshes_share_physical_pages_without_sharing_mesh_identity() {
        let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(128);
        for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
            if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                let draw = Arc::make_mut(draw);
                let instance_id = draw.instance_id;
                let binding = Arc::make_mut(&mut draw.geometry_binding);
                let BindingResource::Geometry(handle) = &mut binding.slots[0].resource else {
                    panic!()
                };
                *handle = GeometryHandle::try_from(RenderResourceHandle {
                    index: instance_id + 1,
                    ..handle.raw()
                })
                .unwrap();
                let mut wire = serde_json::to_value(draw.geometry.as_ref()).unwrap();
                for (i, byte) in (draw.instance_id as f32 * 0.00001)
                    .to_le_bytes()
                    .iter()
                    .enumerate()
                {
                    wire["vertexBytes"][i] = serde_json::json!(*byte);
                }
                draw.geometry = Arc::new(serde_json::from_value(wire).unwrap());
            }
        }
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let first = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        assert_eq!(backend.geometry_residency().meshes, 128);
        let mut buffers = Vec::new();
        for mesh in backend.geometry.meshes.values() {
            for buffer in [mesh.vertices.buffer(), mesh.indices.buffer()] {
                if !buffers.contains(buffer) {
                    buffers.push(buffer.clone());
                }
            }
        }
        assert_eq!(
            buffers.len(),
            2,
            "independent mesh handles still allocate independent native buffers"
        );
        let second = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        assert_eq!(first, second);
        assert_eq!(backend.retained_geometry_stats().draws, 256);
        assert_eq!(backend.geometry_upload_count(), 128);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_textured_moving_camera_keeps_recording_and_texture_replacement_invalidates() {
        let count = 128;
        let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
        let visible: Arc<GeometryBuffers> = Arc::new(serde_json::from_value(serde_json::json!({
            "vertexBytes": ([-0.8f32,-0.8,0.,0.,0., 0.8,-0.8,0.,1.,0., -0.8,0.8,0.,0.,1., 0.8,0.8,0.,1.,1.].into_iter().flat_map(f32::to_le_bytes).collect::<Vec<_>>()),
            "indexBytes": [0,0,1,0,2,0,2,0,1,0,3,0],
            "vertexStride": 20, "vertexCount": 4, "indexCount": 6, "indexFormat": "uint16",
            "attributes": [{"semantic":"position","format":"float32x3","offset":0}, {"semantic":"texCoord0","format":"float32x2","offset":12}]
        })).unwrap());
        let texture = RenderResourceHandle {
            kind: crate::render_resource::RenderResourceKind::Texture,
            index: 404,
            generation: 7,
        };
        for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
            if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                let draw = Arc::make_mut(draw);
                draw.geometry = visible.clone();
                draw.world_transform[14] = (-2f32).into();
                Arc::make_mut(&mut draw.material_binding).slots.push(
                    crate::render_contract::NamedRenderBinding::new("baseColorTexture", texture),
                );
                let instance_id = draw.instance_id;
                let binding = Arc::make_mut(&mut draw.geometry_binding);
                let BindingResource::Geometry(handle) = &mut binding.slots[0].resource else {
                    panic!()
                };
                *handle = GeometryHandle::try_from(RenderResourceHandle {
                    index: instance_id + 1,
                    ..handle.raw()
                })
                .unwrap();
            }
        }
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend
            .register_rgba8_texture(texture, 1, 1, &[255, 32, 32, 255], "nearestClamp")
            .unwrap();
        NATIVE_DRAW_RECORDINGS.with(|n| n.set(0));
        let first = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        assert!(
            first.chunks_exact(4).any(|p| p[0] > 50 && p[0] > p[1]),
            "fixture must visibly consume the red texture"
        );
        assert_eq!(NATIVE_DRAW_RECORDINGS.with(|n| n.get()), count as u64);
        let full_visits = backend.retained_geometry_stats().validation_draw_visits;
        let mut moved = plan.clone();
        for command in &mut moved.commands {
            match command {
                RhiCommand::BeginView { view, .. } => {
                    Arc::make_mut(view).view_matrix[12] = 0.25f32.into()
                }
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] = 0.25f32.into(),
                _ => {}
            }
        }
        encoding::SURVIVAL_MESH_SEARCHES.with(|n| n.set(0));
        let camera = backend.render_plan_to_rgba_bytes(&moved, 640, 480).unwrap();
        assert_eq!(
            backend.retained_geometry_stats().validation_draw_visits,
            full_visits,
            "moving only the camera rebuilt complete native admission"
        );
        assert!(camera != first, "moving camera must change actual pixels");
        assert!(
            encoding::SURVIVAL_MESH_SEARCHES.with(|n| n.get()) <= 2,
            "survival repeated ordered native mesh lookups per draw"
        );
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            count as u64,
            "camera-only change re-recorded textured draws"
        );
        backend
            .register_rgba8_texture(texture, 1, 1, &[32, 255, 32, 255], "nearestClamp")
            .unwrap();
        let replaced = backend.render_plan_to_rgba_bytes(&moved, 640, 480).unwrap();
        assert!(
            replaced != camera,
            "same-handle replacement must change actual pixels"
        );
        assert!(replaced.chunks_exact(4).any(|p| p[1] > 50 && p[1] > p[0]));
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            (count * 2) as u64,
            "replacement reused stale recording"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_immutable_unique_mesh_plan_records_draws_only_once() {
        let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(128);
        let visible: Arc<GeometryBuffers> = Arc::new(serde_json::from_value(serde_json::json!({
            "vertexBytes": ([-0.8f32, -0.8, 0., 0.8, -0.8, 0., -0.8, 0.8, 0., 0.8, 0.8, 0.].into_iter().flat_map(f32::to_le_bytes).collect::<Vec<_>>()),
            "indexBytes": [0,0,1,0,2,0,2,0,1,0,3,0],
            "vertexStride": 12, "vertexCount": 4, "indexCount": 6, "indexFormat": "uint16",
            "attributes": [{"semantic":"position","format":"float32x3","offset":0}]
        })).unwrap());
        for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
            if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                let draw = Arc::make_mut(draw);
                draw.geometry = visible.clone();
                draw.world_transform[14] = (-2f32).into();
                let instance_id = draw.instance_id;
                let binding = Arc::make_mut(&mut draw.geometry_binding);
                let BindingResource::Geometry(handle) = &mut binding.slots[0].resource else {
                    panic!()
                };
                *handle = GeometryHandle::try_from(RenderResourceHandle {
                    index: instance_id + 1,
                    ..handle.raw()
                })
                .unwrap();
            }
        }
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        NATIVE_DRAW_RECORDINGS.with(|n| n.set(0));
        NATIVE_PIPELINE_BINDINGS.with(|n| n.set(0));
        let first = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        assert!(
            first
                .chunks_exact(4)
                .any(|p| p[..3].iter().any(|v| *v != 0)),
            "fixture must be visibly drawn"
        );
        assert_eq!(backend.geometry.meshes.len(), 128);
        assert_eq!(NATIVE_DRAW_RECORDINGS.with(|n| n.get()), 128);
        let second = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        assert!(first == second, "unchanged plan pixels differ");
        assert_eq!(backend.retained_geometry_stats().draws, 256);
        assert_eq!(backend.retained_geometry_stats().instances, 256);
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            128,
            "unchanged native plan recorded all unique mesh draws again"
        );
        assert_eq!(
            NATIVE_PIPELINE_BINDINGS.with(|n| n.get()),
            1,
            "each unique mesh rebound the same native pipeline"
        );
        let index = plan
            .commands
            .iter()
            .position(|c| geometry_draw(c).is_some())
            .unwrap();
        let lease = Arc::downgrade(&geometry_draw(&plan.commands[index]).unwrap().resource_lease);
        let mut changed = plan.clone();
        let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut changed.commands[index]
        else {
            panic!()
        };
        Arc::make_mut(draw).world_transform[12] = 0.25.into();
        let moved = backend
            .render_plan_to_rgba_bytes(&changed, 640, 480)
            .unwrap();
        assert!(moved != first, "world change must alter actual pixels");
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            128,
            "matrix-only change re-recorded an identical native encoding recipe"
        );
        assert_eq!(backend.geometry_upload_count(), 128);
        let mut camera_changed = changed.clone();
        let mut view = geometry_draw(&changed.commands[index])
            .unwrap()
            .view
            .as_ref()
            .clone();
        view.view_matrix[12] = (-0.2f32).into();
        let view = Arc::new(view);
        for command in &mut camera_changed.commands {
            match command {
                RhiCommand::BeginView { view: current, .. } => *current = view.clone(),
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Arc::make_mut(draw).view = view.clone(),
                _ => (),
            }
        }
        let camera_pixels = backend
            .render_plan_to_rgba_bytes(&camera_changed, 640, 480)
            .unwrap();
        assert!(
            camera_pixels != moved,
            "camera matrix must alter actual pixels"
        );
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            128,
            "camera-only change re-recorded native draws"
        );
        let mut range_changed = changed.clone();
        let RhiCommand::Draw {
            vertex_count,
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut range_changed.commands[index]
        else {
            panic!()
        };
        *vertex_count = 6;
        Arc::make_mut(draw).index_count = 6;
        let range_pixels = backend
            .render_plan_to_rgba_bytes(&range_changed, 640, 480)
            .unwrap();
        assert!(
            range_pixels != moved,
            "valid index range change must alter actual pixels"
        );
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            256,
            "changed index range reused stale recording"
        );
        let mut invalid = changed.clone();
        let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut invalid.commands[index]
        else {
            panic!()
        };
        Arc::make_mut(draw).index_count = 1;
        assert!(backend
            .render_plan_to_rgba_bytes(&invalid, 640, 480)
            .is_err());
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            256,
            "invalid draw reached native recording"
        );
        assert!(
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap() == first,
            "old packet pixels were changed"
        );
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            384,
            "restored index range reused another packet's recording"
        );
        assert!(backend
            .render_plan_to_rgba_bytes(&plan, 800, 600)
            .unwrap_err()
            .contains("target_extent_mismatch"));
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            384,
            "mismatched attachment reached native recording"
        );
        assert!(
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap() == first,
            "old packet pixels were changed"
        );
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            384,
            "failed extent must not invalidate equivalent recording"
        );
        let mut reordered = plan.clone();
        reordered.commands.swap(index, index + 1);
        let reordered_pixels = backend
            .render_plan_to_rgba_bytes(&reordered, 640, 480)
            .unwrap();
        assert!(
            reordered_pixels == first,
            "opaque reorder fixture output changed"
        );
        assert_eq!(
            NATIVE_DRAW_RECORDINGS.with(|n| n.get()),
            512,
            "changed native mesh/range order reused stale recording"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        drop(reordered);
        drop(range_changed);
        drop(camera_changed);
        drop(invalid);
        drop(changed);
        drop(plan);
        drop(graph);
        assert!(
            lease.upgrade().is_none(),
            "bundle kept the CPU/native ownership lease alive"
        );
        backend.abandon_device_epoch();
        assert!(backend.geometry.validated_plan.is_none());
    }

    #[test]
    #[ignore = "requires a real GPU"]
    fn gpu_geometry_intent_replay_is_consumed_only_after_full_recording() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
        assert!(!backend.reuse_geometry_intent(&plan));
        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("geometry intent replay"),
            size: wgpu::Extent3d {
                width: 640,
                height: 480,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let visits = || crate::wgpu_backend::GEOMETRY_INTENT_VALUE_VISITS.with(|c| c.get());
        crate::wgpu_backend::GEOMETRY_INTENT_VALUE_VISITS.with(|c| c.set(0));
        let cold = backend.execute_plan_to_surface_view(&plan, &view);
        assert!(cold.diagnostics.is_empty(), "{cold:?}");
        assert_eq!(visits(), 1);
        assert!(backend.reuse_geometry_intent(&plan));
        let replay = backend.execute_plan_to_surface_view(&plan, &view);
        assert!(replay.diagnostics.is_empty(), "{replay:?}");
        assert_eq!(visits(), 1, "production replay revisited geometry metadata");
        assert_eq!(replay.draw_count, 2);
        assert_eq!(replay.binding_count, 4);
        backend.state.target_context.format = "Bgra8Unorm".into();
        assert!(!backend.reuse_geometry_intent(&plan));
        let changed_format = backend.execute_plan_to_surface_view(&plan, &view);
        assert!(changed_format.diagnostics.is_empty(), "{changed_format:?}");
        assert_eq!(visits(), 2, "changed format must use full recording");
    }

    #[test]
    fn retained_draw_ranges_avoid_warm_grouping_and_changed_commands_split() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(48688);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let cached =
            ValidatedGeometryPlan::capture(&plan, [64, 64], None, wgpu::TextureFormat::Rgba8Unorm)
                .unwrap();
        let start = plan
            .commands
            .iter()
            .position(|c| {
                matches!(
                    c,
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { .. },
                        ..
                    }
                )
            })
            .unwrap();
        let end = geometry_draw_end(&plan.commands, start);
        DRAW_GROUP_VISITS.with(|c| c.set(0));
        let clock = crate::platform_time::Instant::now();
        for _ in 0..20 {
            assert_eq!(
                cached.draw_end(std::hint::black_box(&plan.commands), start),
                end
            );
        }
        let elapsed = clock.elapsed().as_nanos() / 20;
        let visits = DRAW_GROUP_VISITS.with(|c| c.get());
        eprintln!(
            "draw-grouping Debug count={} mean_ns={elapsed} visits={visits}",
            end - start
        );
        let mut changed = plan.commands.clone();
        let split = start + 200;
        let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut changed[split]
        else {
            panic!()
        };
        let changed_target = format!("{}-changed", draw.target);
        Arc::make_mut(draw).target = changed_target.into();
        assert_eq!(cached.draw_end(&changed, start), split);
        assert_eq!(cached.draw_end(&changed, split), split + 1);
        let mut barrier = plan.commands.clone();
        barrier[split] = RhiCommand::Clear {
            target: "barrier".into(),
            color: [0.0.into(); 4],
        };
        assert_eq!(cached.draw_end(&barrier, start), split);
        assert_eq!(
            visits, 0,
            "warm native bundle still regrouped all immutable draws"
        );
    }

    #[test]
    fn retained_command_backing_does_not_rescan_or_pin_original_resources() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(48688);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let cached =
            ValidatedGeometryPlan::capture(&plan, [64, 64], None, wgpu::TextureFormat::Rgba8Unorm)
                .unwrap();
        COMMAND_PROOF_VISITS.with(|c| c.set(0));
        let start = crate::platform_time::Instant::now();
        for _ in 0..20 {
            assert!(cached.matches_commands(std::hint::black_box(&plan.commands)));
        }
        eprintln!(
            "native-command-proof Debug commands={} mean_ns={} visits={}",
            plan.commands.len(),
            start.elapsed().as_nanos() / 20,
            COMMAND_PROOF_VISITS.with(|c| c.get())
        );
        assert_eq!(
            COMMAND_PROOF_VISITS.with(|c| c.get()),
            0,
            "same immutable command backing was rescanned"
        );
        let lease = plan
            .commands
            .iter()
            .find_map(|command| match command {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some(Arc::downgrade(draw.native_resource_lease())),
                _ => None,
            })
            .unwrap();
        let mut changed = plan.commands.clone();
        let RhiCommand::BeginFrame { target } = &mut changed[0] else {
            panic!()
        };
        target.push_str("-invalid");
        assert!(
            !cached.matches_commands(&changed),
            "changed root command escaped fallback"
        );
        let mut unique = plan.commands.clone();
        drop(plan);
        drop(graph);
        assert!(cached.matches_commands(&unique));
        // Only one strong list owner remains; Weak must still invalidate on COW.
        unique.pop();
        assert!(!cached.matches_commands(&unique));
        drop(unique);
        drop(changed);
        assert!(
            lease.upgrade().is_none(),
            "weak command proof pinned native lease"
        );
    }

    #[test]
    fn geometry_intent_requires_consumption_format_and_unchanged_commands() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut cached =
            ValidatedGeometryPlan::capture(&plan, [64, 64], None, wgpu::TextureFormat::Rgba8Unorm)
                .unwrap();
        assert!(!cached.intent_matches(&plan.commands, "Rgba8Unorm"));
        cached.intent_format = Some("Rgba8Unorm".into());
        assert!(cached.intent_matches(&plan.commands, "Rgba8Unorm"));
        assert!(!cached.intent_matches(&plan.commands, "Bgra8Unorm"));
        let mut changed = plan.commands.clone();
        changed.pop();
        assert!(!cached.intent_matches(&changed, "Rgba8Unorm"));
        let readback =
            ValidatedGeometryPlan::capture(&plan, [64, 64], None, wgpu::TextureFormat::Rgba8Unorm)
                .unwrap();
        assert!(!readback.intent_matches(&plan.commands, "Rgba8Unorm"));
    }

    #[test]
    fn native_plan_same_draw_identity_avoids_values_and_changed_commands_still_compare() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(128);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let cached =
            ValidatedGeometryPlan::capture(&plan, [64, 64], None, wgpu::TextureFormat::Rgba8Unorm)
                .unwrap();
        COMMAND_VALUE_COMPARISONS.with(|count| count.set(0));
        assert!(cached.matches_commands(&plan.commands));
        assert_eq!(COMMAND_VALUE_COMPARISONS.with(|count| count.get()), 0);
        let index = plan
            .commands
            .iter()
            .position(|c| {
                matches!(
                    c,
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { .. },
                        ..
                    }
                )
            })
            .unwrap();
        let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &plan.commands[index]
        else {
            panic!()
        };
        let lease = Arc::downgrade(draw.native_resource_lease());
        let mut commands = plan.commands.clone();
        let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut commands[index]
        else {
            panic!()
        };
        // Equal values at a different allocation retain the original fallback.
        let _ = Arc::make_mut(draw);
        assert!(cached.matches_commands(&commands));
        assert_eq!(COMMAND_VALUE_COMPARISONS.with(|count| count.get()), 1);
        let RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } = &mut commands[index]
        else {
            panic!()
        };
        Arc::make_mut(draw).world_transform[12] = 3.0.into();
        assert!(!cached.matches_commands(&commands));
        let mut malformed = plan.commands.clone();
        let RhiCommand::Draw { vertex_count, .. } = &mut malformed[index] else {
            panic!()
        };
        *vertex_count += 3;
        assert!(!cached.matches_commands(&malformed));
        malformed.pop();
        assert!(!cached.matches_commands(&malformed));
        drop(malformed);
        drop(commands);
        drop(plan);
        drop(graph);
        assert!(
            lease.upgrade().is_none(),
            "native command proof retained resource lease"
        );
    }
}

// One successful geometry-only plan. Borrow live commands only during comparison;
// never clone descriptors merely to detach their native resource leases.
struct ValidatedGeometryPlan {
    moving_views: Option<moving_views::MovingViews>,
    command_backing: std::sync::Weak<Vec<RhiCommand>>,
    // Command-index ranges, detached from all original resource leases.
    draw_ends: BTreeMap<usize, usize>,
    requirements: Vec<Option<Arc<preparation::Requirements>>>,
    native_templates: template_backing::TemplateCollection<Option<Arc<native_template::Template>>>,
    schedule: Option<Arc<crate::render_graph::CompiledGraphSchedule>>,
    extent: [u32; 2],
    rect: Option<GameViewRect>,
    meshes: Vec<(RenderResourceHandle, Arc<GeometryBuffers>)>,
    textures: BTreeSet<RenderResourceHandle>,
    intent_format: Option<String>,
    output_format: wgpu::TextureFormat,
    outputs_prepared: bool,
    capabilities_validated: std::cell::Cell<bool>,
    // Part of this one bounded successful plan, never a second plan registry.
    bundles: BTreeMap<u32, encoding::Recorded>,
    // Exact ranges computed by the existing preparation owner. Actual
    // measurement may split provenance ranges differently during recording.
    recording_keys: Vec<u32>,
}

impl ValidatedGeometryPlan {
    // Exact live borrowed subrange only. These are CPU descriptions; the recipe
    // still reads today's native handles and compares the complete result.
    fn requirements_for(
        &self,
        commands: &[RhiCommand],
        format: wgpu::TextureFormat,
    ) -> Option<&[Option<Arc<preparation::Requirements>>]> {
        if self.output_format != format {
            return None;
        }
        let live = self.command_backing.upgrade()?;
        let bytes = (commands.as_ptr() as usize).checked_sub(live.as_ptr() as usize)?;
        let stride = std::mem::size_of::<RhiCommand>();
        if !bytes.is_multiple_of(stride) {
            return None;
        }
        let start = bytes / stride;
        let end = start.checked_add(commands.len())?;
        if end > live.len() {
            return None;
        }
        self.requirements.get(start..end)
    }

    fn native_for(
        &self,
        commands: &[RhiCommand],
        format: wgpu::TextureFormat,
    ) -> Option<&[Option<Arc<native_template::Template>>]> {
        let requirements = self.requirements_for(commands, format)?;
        let offset = (requirements.as_ptr() as usize)
            .checked_sub(self.requirements.as_ptr() as usize)?
            / std::mem::size_of::<Option<Arc<preparation::Requirements>>>();
        self.native_templates.get(offset..offset + commands.len())
    }

    fn draw_end(
        &self,
        commands: &crate::render_graph::SharedRenderList<RhiCommand>,
        start: usize,
    ) -> usize {
        if commands.shares_backing(&self.command_backing) {
            if let Some(end) = self.draw_ends.get(&start) {
                return *end;
            }
        }
        geometry_draw_end(commands, start)
    }

    fn intent_matches(
        &self,
        commands: &crate::render_graph::SharedRenderList<RhiCommand>,
        format: &str,
    ) -> bool {
        self.intent_format.as_deref() == Some(format) && self.matches_commands(commands)
    }

    fn matches_commands(
        &self,
        commands: &crate::render_graph::SharedRenderList<RhiCommand>,
    ) -> bool {
        if commands.shares_backing(&self.command_backing) {
            return true;
        }
        // Expiry is a cache miss, not a claim about new commands. Upgrading here
        // borrows a still-owned original list only for this comparison call.
        let Some(previous) = self.command_backing.upgrade() else {
            return false;
        };
        previous.len() == commands.len()
            && previous.iter().zip(commands).all(|(old, next)| {
                #[cfg(test)]
                COMMAND_PROOF_VISITS.with(|count| count.set(count.get() + 1));
                if let (
                    RhiCommand::Draw {
                        target: old_target,
                        draw_kind: old_kind,
                        vertex_count: old_count,
                        payload: RhiDrawPayload::Geometry { draw: old_draw },
                    },
                    RhiCommand::Draw {
                        target,
                        draw_kind,
                        vertex_count,
                        payload: RhiDrawPayload::Geometry { draw },
                    },
                ) = (old, next)
                {
                    if Arc::ptr_eq(old_draw, draw) {
                        return old_target == target
                            && old_kind == draw_kind
                            && old_count == vertex_count;
                    }
                }
                #[cfg(test)]
                if matches!(
                    next,
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { .. }
                            | RhiDrawPayload::GeometryBatch { .. },
                        ..
                    }
                ) {
                    COMMAND_VALUE_COMPARISONS.with(|count| count.set(count.get() + 1));
                }
                old == next
            })
    }

    #[cfg(test)]
    fn capture(
        plan: &RhiCommandPlan,
        extent: [u32; 2],
        rect: Option<GameViewRect>,
        output_format: wgpu::TextureFormat,
    ) -> Option<Self> {
        let mut meshes = BTreeMap::new();
        let mut textures = BTreeSet::new();
        let mut bindings = std::collections::HashSet::new();
        for command in &plan.commands {
            match command {
                command if command.geometry_draw().is_some() => {
                    let draw = command.geometry_draw().unwrap();
                    meshes
                        .entry(draw.geometry_handle().raw())
                        .or_insert_with(|| draw.geometry.clone());
                    if bindings.insert(Arc::as_ptr(&draw.material_binding)) {
                        for handle in draw.material_textures().into_iter().flatten() {
                            textures.insert(handle.raw());
                        }
                    }
                }
                RhiCommand::ResolveColor3d { .. }
                | RhiCommand::BeginView { .. }
                | RhiCommand::BeginFrame { .. }
                | RhiCommand::Clear { .. }
                | RhiCommand::Submit
                | RhiCommand::Present { .. } => {}
                _ => return None,
            }
        }
        Self::capture_prepared(
            plan,
            (extent, rect),
            output_format,
            meshes.into_iter().collect(),
            textures,
            vec![None; plan.commands.len()],
            None,
        )
    }

    // The current successful preparation already admitted this dependency
    // closure. Consume it without interpreting every descriptor a second time.
    fn capture_prepared(
        plan: &RhiCommandPlan,
        (extent, rect): ([u32; 2], Option<GameViewRect>),
        output_format: wgpu::TextureFormat,
        meshes: Vec<(RenderResourceHandle, Arc<GeometryBuffers>)>,
        textures: BTreeSet<RenderResourceHandle>,
        requirements: Vec<Option<Arc<preparation::Requirements>>>,
        moving_views: Option<moving_views::MovingViews>,
    ) -> Option<Self> {
        if plan.commands.len() > MAX_GEOMETRY_INSTANCES + 64
            || requirements.len() != plan.commands.len()
        {
            return None;
        }
        let mut draw_ends = BTreeMap::new();
        let mut index = 0;
        while index < plan.commands.len() {
            let end = geometry_draw_end(&plan.commands, index);
            if end > index {
                draw_ends.insert(index, end);
                index = end;
            } else {
                index += 1;
            }
        }
        Some(Self {
            moving_views: moving_views.or_else(|| moving_views::MovingViews::capture(plan)),
            command_backing: plan.commands.weak_backing(),
            draw_ends,
            schedule: plan.graph_schedule.clone(),
            extent,
            rect,
            meshes,
            textures,
            native_templates: vec![None; requirements.len()].into(),
            requirements,
            intent_format: None,
            output_format,
            outputs_prepared: false,
            capabilities_validated: std::cell::Cell::new(false),
            bundles: BTreeMap::new(),
            recording_keys: Vec::new(),
        })
    }
}
// Measurement splits only at existing component provenance boundaries, preserving
// order. Off retains the original grouping and does not walk this path.
pub(super) fn measurement_draw_end(commands: &[RhiCommand], start: usize, end: usize) -> usize {
    let source = measurement_draw_source(&commands[start]);
    if source.is_none() {
        return start + 1;
    }
    (start + 1..end)
        .find(|&i| measurement_draw_source(&commands[i]) != source)
        .unwrap_or(end)
}
// None is a legal, mixed-source instanced draw, which has no separable GPU span.
// Inspect only selected rows; an invisible ordinary row does not contaminate it.
#[inline]
pub(super) fn measurement_draw_source(command: &RhiCommand) -> Option<bool> {
    // This hot measurement-only scan already receives a geometry command.
    // Dispatch once instead of repeatedly resolving the same execution head.
    match command {
        RhiCommand::Draw {
            payload: RhiDrawPayload::Geometry { draw },
            ..
        } => Some(draw.provenance.source_component.as_deref() == Some("SpineAnimator")),
        RhiCommand::Draw {
            payload: RhiDrawPayload::GeometryBatch { batch },
            ..
        } => {
            let spine = batch.draw.provenance.source_component.as_deref() == Some("SpineAnimator");
            if batch.visible.iter().any(|row| {
                (batch.instances[row.instance_index as usize]
                    .provenance
                    .source_component
                    .as_deref()
                    == Some("SpineAnimator"))
                    != spine
            }) {
                None
            } else {
                Some(spine)
            }
        }
        _ => unreachable!("measurement source requires a geometry command"),
    }
}

fn geometry_draw_end(commands: &[RhiCommand], start: usize) -> usize {
    let Some(draw) = commands.get(start).and_then(geometry_draw) else {
        return start;
    };
    let mut end = start;
    for command in &commands[start..] {
        let Some(next) = geometry_draw(command) else {
            break;
        };
        #[cfg(test)]
        DRAW_GROUP_VISITS.with(|c| c.set(c.get() + 1));
        if next.view != draw.view
            || next.target != draw.target
            || next.depth_target != draw.depth_target
        {
            break;
        }
        end += 1;
    }
    end
}
fn depth_lifetimes_disjoint(plan: &RhiCommandPlan, a: &str, b: &str) -> bool {
    let range = |id: &str| {
        let uses = plan
            .commands
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c {
                RhiCommand::BeginView {
                    depth_target: Some(depth),
                    ..
                } if depth == id => Some(i),
                c if c.geometry_draw().is_some_and(|draw| {
                    draw.depth_target.as_deref() == Some(id)
                        || draw
                            .shadows
                            .as_ref()
                            .is_some_and(|s| s.atlases().any(|atlas| atlas == id))
                }) =>
                {
                    Some(i)
                }
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Particles { view, .. },
                    ..
                } if view.depth_target.as_deref() == Some(id) => Some(i),
                _ => None,
            })
            .collect::<Vec<_>>();
        uses.first().copied().zip(uses.last().copied())
    };
    match (range(a), range(b)) {
        (Some((af, al)), Some((bf, bl))) => al < bf || bl < af,
        _ => false,
    }
}

/// On-demand counts owned by this backend; excludes shared textures and driver memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct GeometryResidency {
    pub meshes: usize,
    pub mesh_bytes: u64,
    /// Allocated physical mesh pages, including padding and in-flight ranges.
    pub physical_mesh_bytes: u64,
    pub textures: usize,
    pub texture_bytes: u64,
    pub depth_targets: usize,
    pub pipelines: usize,
}

/// Cumulative actual backend operations, requested by diagnostics/tests.
/// Draws count indexed geometry draws; passes include view/depth/color clears.
/// The fixed arena stays allocated while idle, avoiding in-flight reallocation peaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct RetainedGeometryStats {
    /// Actual native-template registry reads, including failed current checks.
    /// Actual weak memo writes; successful unchanged entries need no rewrite.
    pub template_memo_writes: u64,
    /// Actual draw descriptors visited by full plan validation, including retries.
    pub validation_draw_visits: u64,
    /// Actual retained parameter descriptors visited, including unchanged values.
    pub parameter_draw_visits: u64,
    pub depth_allocations: u64,
    pub draws: u64,
    pub passes: u64,
    pub bind_groups_created: u64,
    pub parameter_uploads: u64,
    pub parameter_uploaded_bytes: u64,
    pub index_uploaded_bytes: u64,
    pub clear_uploaded_bytes: u64,
    pub installed_content_version: u64,
    pub instances: u64,
    pub resident_parameter_bytes: u64,
    pub peak_parameter_bytes: u64,
}

pub(super) struct GeometryGpuState {
    skins: skin::SkinGpuState,
    gpu_scale: gpu_scale::GpuScaleState,
    shadow_layouts: BTreeMap<u8, wgpu::BindGroupLayout>,
    shadow_bindings: BTreeMap<u64, shadows::ShadowBinding>,
    light_layout: wgpu::BindGroupLayout,
    light_buffers: BTreeMap<String, lights::LightBuffer>,
    environment_buffers: BTreeMap<String, ibl::EnvironmentBuffer>,
    lit_layouts: BTreeMap<u8, wgpu::BindGroupLayout>,
    lit_bindings: BTreeMap<lit::TextureKey, wgpu::BindGroup>,
    preview: [[f32; 4]; 3],
    pages: pages::GeometryPages,
    streaming: streaming::GeometryStreaming,
    visual_resident_limit: u64,
    meshes: BTreeMap<RenderResourceHandle, ResidentGeometry>,
    pipelines: BTreeMap<String, wgpu::RenderPipeline>,
    pub(super) colors: BTreeMap<String, (u32, u32, wgpu::TextureView)>,
    pub(super) depths: BTreeMap<String, (u32, u32, wgpu::TextureView)>,
    depth_layout: Vec<(String, String)>,
    uniform_layout: wgpu::BindGroupLayout,
    clear_pipeline: wgpu::RenderPipeline,
    hdr_clear_pipeline: Option<wgpu::RenderPipeline>,
    post: Option<post::PostColorState>,
    uploaded_meshes: usize,
    stats: RetainedGeometryStats,
    parameters: Option<RetainedParameters>,
    validated_plan: Option<ValidatedGeometryPlan>,
    preparation: preparation::Preparation,
    native_templates: native_template::Templates,
    // Current native resource workset, with no command/whole-plan proof. Mixed
    // UI/particle frames retain these same prepared templates too. The weak
    // lookup does not own their lifetime; original invalidation/retirement does.
    current_templates: template_backing::TemplateCollection<Arc<native_template::Template>>,
    parameter_layout: wgpu::BindGroupLayout,
    clears: BTreeMap<u64, ([f32; 36], wgpu::Buffer, wgpu::BindGroup)>,
}

impl super::memory::RetiredGpuResource for Vec<ResidentGeometry> {
    fn observe<'a>(
        &'a self,
        gpu: &mut super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        if !gpu.is_backing_only() {
            sample.cpu_vec(self, "renderer", &["retiring"]);
        }
        for mesh in self {
            gpu.buffer(mesh.vertices.buffer(), "renderer", &["retiring"]);
            gpu.buffer(mesh.indices.buffer(), "renderer", &["retiring"]);
            if !gpu.is_backing_only() {
                mesh.source.observe_memory(sample, "retiring");
            }
        }
    }
}
impl GeometryGpuState {
    pub(super) fn visual_resident_limit(&self) -> u64 {
        self.visual_resident_limit
    }
    pub(super) fn observe_memory<'a>(
        &'a self,
        gpu: &mut super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        if let Some(post) = &self.post {
            post.observe_memory(gpu, sample);
        }
        if !gpu.is_backing_only() {
            self.streaming.observe_memory(sample);
            for mesh in self.meshes.values() {
                mesh.source.observe_memory(sample, "resident");
            }
        }
        self.pages.observe_memory(gpu);
        self.skins.observe(gpu);
        self.gpu_scale.observe(gpu, sample);
        for (_, _, view) in self.colors.values() {
            gpu.texture(view.texture(), &["resident"]);
        }
        for (_, _, view) in self.depths.values() {
            gpu.texture(view.texture(), &["resident"]);
        }
        for binding in self.shadow_bindings.values() {
            if !gpu.is_backing_only() {
                sample.cpu_vec(&binding.depths, "renderer", &["resident"]);
            }
            gpu.buffer(&binding.buffer, "renderer", &["resident"]);
            for view in &binding.depths {
                gpu.texture(view.texture(), &["resident"]);
            }
        }
        for e in self.environment_buffers.values() {
            gpu.buffer(&e.buffer, "renderer", &["resident"]);
        }
        for light in self.light_buffers.values() {
            gpu.buffer(&light.buffer, "renderer", &["resident"]);
        }
        for (_, buffer, _) in self.clears.values() {
            gpu.buffer(buffer, "renderer", &["resident"]);
        }
        if let Some(parameters) = &self.parameters {
            gpu.buffer(&parameters.buffer, "renderer", &["resident"]);
            gpu.buffer(&parameters.view_buffer, "renderer", &["resident"]);
            gpu.buffer(&parameters.index_buffer, "renderer", &["resident"]);
        }
        // Ready recipes were captured from, or compared with, every current
        // command range. Once the exact pruned key set is complete, those real
        // recipes own all dependencies of both immutable template inputs.
        // Partial preparation/recording and detached collections keep the full
        // census. Trace still observes every original role below.
        let recorded_templates = gpu.is_backing_only()
            && self.current_templates.is_recording_input()
            && self.validated_plan.as_ref().is_some_and(|plan| {
                plan.native_templates.is_recording_input()
                    && plan.bundles.len() == plan.recording_keys.len()
                    && plan
                        .bundles
                        .iter()
                        .zip(&plan.recording_keys)
                        .all(|((key, recorded), expected)| key == expected && recorded.ready)
            });
        if !recorded_templates {
            self.current_templates.observe_gpu(gpu, sample, "resident");
        }
        if !gpu.is_backing_only() {
            for template in &self.current_templates {
                sample.cpu_arc(template, "renderer", &["resident"]);
            }
        }
        if let Some(plan) = &self.validated_plan {
            if !recorded_templates {
                plan.native_templates.observe_gpu(gpu, sample, "resident");
            }
            if !gpu.is_backing_only() {
                for template in plan.native_templates.iter().flatten() {
                    sample.cpu_arc(template, "renderer", &["resident"]);
                }
            }
            for recorded in plan.bundles.values() {
                recorded.observe(gpu, sample, "resident");
            }
        }
        if gpu.is_backing_only() {
            return;
        }
        sample.cpu_vec(&self.depth_layout, "renderer", &["resident"]);
        sample.cpu_vec(&self.current_templates, "renderer", &["resident"]);
        self.native_templates.observe(sample);
        self.preparation.observe(
            sample,
            self.validated_plan
                .as_ref()
                .map_or(&[], |plan| plan.requirements.as_slice()),
        );
        if let Some(plan) = &self.validated_plan {
            sample.cpu_vec(&plan.recording_keys, "renderer", &["resident"]);
            if let Some(moving) = &plan.moving_views {
                moving.observe(sample);
            }
            sample.cpu_vec(&plan.requirements, "renderer", &["resident"]);
            sample.cpu_vec(&plan.native_templates, "renderer", &["resident"]);
            if let Some(format) = &plan.intent_format {
                sample.cpu_string(format, "renderer", &["resident"]);
            }
            sample.cpu_vec(&plan.meshes, "renderer", &["resident"]);
            for (_, source) in &plan.meshes {
                sample.cpu_arc(source, "assets", &["resident"]);
                source.observe_memory(sample, "resident");
            }
        }
        if let Some(parameters) = &self.parameters {
            parameters.instances.observe_memory(sample);
            parameters.views.observe_memory(sample);
            sample.cpu_vec(&parameters.indices, "renderer", &["resident"]);
            if let Some(selection) = &parameters.selection {
                sample.cpu_vec(selection, "renderer", &["resident"]);
            }
            parameters.derived_instances.observe_memory(sample);
        }
    }

    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("geometry-view-material-uniform"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(environment::CLEAR_BYTES),
                },
                count: None,
            }],
        });
        let hdr_clear_pipeline = None;
        let clear_pipeline = clear_pipeline(device, format, &uniform_layout);
        let parameter_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("geometry-retained-parameters"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(INSTANCE_BYTES),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(VIEW_BYTES),
                    },
                    count: None,
                },
            ],
        });
        Self {
            shadow_layouts: BTreeMap::new(),
            shadow_bindings: BTreeMap::new(),
            light_layout: lights::layout(device),
            light_buffers: BTreeMap::new(),
            environment_buffers: BTreeMap::new(),
            lit_layouts: BTreeMap::new(),
            lit_bindings: BTreeMap::new(),
            preview: [[0.; 4]; 3],
            skins: skin::SkinGpuState::default(),
            gpu_scale: gpu_scale::GpuScaleState::default(),
            pages: pages::GeometryPages::default(),
            streaming: streaming::GeometryStreaming::default(),
            visual_resident_limit: MAX_VISUAL_RESIDENT_BYTES,
            meshes: BTreeMap::new(),
            pipelines: BTreeMap::new(),
            colors: BTreeMap::new(),
            depths: BTreeMap::new(),
            depth_layout: Vec::new(),
            uniform_layout,
            hdr_clear_pipeline,
            clear_pipeline,
            post: None,
            uploaded_meshes: 0,
            stats: RetainedGeometryStats::default(),
            parameters: None,
            validated_plan: None,
            preparation: preparation::Preparation::default(),
            native_templates: native_template::Templates::default(),
            current_templates: Default::default(),
            parameter_layout,
            clears: BTreeMap::new(),
        }
    }
}

impl RealWgpuBackend {
    pub(super) fn collect_geometry_pages(&mut self) {
        self.geometry.pages.collect_empty();
    }
    pub(super) fn invalidate_geometry_bundles(&mut self) -> Result<(), String> {
        let count = self
            .geometry
            .validated_plan
            .as_ref()
            .map_or(0, |p| p.bundles.len());
        self.admit_gpu_retirement(0, count)?;
        self.geometry.current_templates.clear();
        if let Some(plan) = self.geometry.validated_plan.as_mut() {
            plan.native_templates.clear();
            let old = std::mem::take(&mut plan.bundles);
            for (_, recorded) in old {
                self.retire_gpu(recorded, 0);
            }
        }
        Ok(())
    }

    pub(super) fn invalidate_geometry_texture_bundles(
        &mut self,
        handle: &RenderResourceHandle,
    ) -> Result<(), String> {
        self.geometry.current_templates.clear();
        if let Some(plan) = self.geometry.validated_plan.as_mut() {
            plan.native_templates.clear();
        }
        if self
            .geometry
            .validated_plan
            .as_ref()
            .is_some_and(|p| p.textures.contains(handle))
        {
            self.invalidate_geometry_bundles()?;
        }
        self.invalidate_lit_texture(handle)?;
        Ok(())
    }
    // Structural classification only; never value validation or resource admission.
    pub(super) fn geometry_command_shape(plan: &RhiCommandPlan) -> bool {
        plan.commands.len() <= MAX_GEOMETRY_INSTANCES + 64
            && plan.commands.iter().all(|command| {
                matches!(
                    command,
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { .. }
                            | RhiDrawPayload::GeometryBatch { .. },
                        ..
                    } | RhiCommand::ResolveColor3d { .. }
                        | RhiCommand::BeginView { .. }
                        | RhiCommand::BeginFrame { .. }
                        | RhiCommand::Clear { .. }
                        | RhiCommand::Submit
                        | RhiCommand::Present { .. }
                )
            })
    }
    // Caller has the current complete command proof; no native object is admitted.
    #[cfg(test)]
    pub(super) fn prepared_geometry_intent_pipeline(
        &self,
        commands: &crate::render_graph::SharedRenderList<RhiCommand>,
        command_index: usize,
        draw: &DrawGeometry,
    ) -> Arc<str> {
        if let Some(requirements) = self
            .geometry
            .validated_plan
            .as_ref()
            .filter(|p| {
                p.output_format == self.format && commands.shares_backing(&p.command_backing)
            })
            .filter(|_| {
                commands
                    .get(command_index)
                    .and_then(geometry_draw)
                    .is_some_and(|current| std::ptr::eq(current, draw))
            })
            .and_then(|p| p.requirements.get(command_index))
            .and_then(Option::as_ref)
        {
            return requirements.intent_pipeline().clone();
        }
        self.geometry
            .preparation
            .resolve(draw, wgpu::TextureFormat::Rgba8Unorm)
            .pipeline
            .clone()
    }
    // Called from the ordered CPU-intent loop after current plan preparation.
    // Keep required counters even when metadata was already recorded. Actual
    // rendering/admission still executes through the original native owner.
    pub(super) fn record_current_geometry_intent(
        &mut self,
        commands: &crate::render_graph::SharedRenderList<RhiCommand>,
        index: usize,
        payload: &RhiDrawPayload,
    ) {
        let Some(draw) = payload.geometry_draw() else {
            self.state.record_draw(payload);
            return;
        };
        let cached = self.geometry.validated_plan.as_ref().filter(|p| {
            p.output_format == self.format
                && commands.shares_backing(&p.command_backing)
                && commands
                    .get(index)
                    .and_then(geometry_draw)
                    .is_some_and(|current| std::ptr::eq(current, draw.as_ref()))
                && self.state.target_context.format == "Rgba8Unorm"
        });
        if let Some((facts, native)) = cached.and_then(|p| {
            p.requirements
                .get(index)
                .and_then(Option::as_ref)
                .map(|facts| {
                    (
                        facts,
                        p.native_templates.get(index).and_then(Option::as_ref),
                    )
                })
        }) {
            if native.is_some_and(|native| native.intent_recorded(draw, facts.intent_pipeline())) {
                self.state.record_reused_geometry_intent(payload);
            } else {
                self.state
                    .record_prepared_geometry_intent(draw, facts.intent_pipeline());
                if let Some(native) = native {
                    native.mark_intent(draw, facts.intent_pipeline());
                }
            }
        } else {
            self.state.record_draw(payload);
        }
    }

    pub(super) fn validated_geometry_only_commands(&self, plan: &RhiCommandPlan) -> bool {
        self.geometry
            .validated_plan
            .as_ref()
            .is_some_and(|cached| cached.matches_commands(&plan.commands))
    }

    pub(super) fn reuse_geometry_capabilities(&self, plan: &RhiCommandPlan) -> bool {
        self.geometry.validated_plan.as_ref().is_some_and(|cached| {
            cached.capabilities_validated.get()
                && cached.output_format == self.format
                && cached.schedule == plan.graph_schedule
                && cached.matches_commands(&plan.commands)
        })
    }

    pub(super) fn record_geometry_capabilities(&self, plan: &RhiCommandPlan) {
        if let Some(cached) = self.geometry.validated_plan.as_ref().filter(|cached| {
            cached.output_format == self.format
                && cached.schedule == plan.graph_schedule
                && cached.matches_commands(&plan.commands)
        }) {
            cached.capabilities_validated.set(true);
        }
    }

    /// This is the existing successful geometry-only plan's requirement set,
    /// not a residency proof. The caller must query the live registry each time.
    pub(super) fn validated_geometry_texture_handles(
        &self,
        plan: &RhiCommandPlan,
    ) -> Option<&BTreeSet<RenderResourceHandle>> {
        self.geometry
            .validated_plan
            .as_ref()
            .filter(|cached| cached.matches_commands(&plan.commands))
            .map(|cached| &cached.textures)
    }

    pub(super) fn reuse_geometry_intent(&self, plan: &RhiCommandPlan) -> bool {
        // begin_frame normalizes the intent format before recording any draw.
        self.state.target_context.format == "Rgba8Unorm"
            && self.geometry.validated_plan.as_ref().is_some_and(|cached| {
                cached.intent_matches(&plan.commands, &self.state.target_context.format)
            })
    }

    pub(super) fn mark_geometry_intent(&mut self, plan: &RhiCommandPlan) {
        if let Some(cached) = self.geometry.validated_plan.as_mut() {
            if cached.intent_format.as_deref() != Some(&self.state.target_context.format)
                && cached.matches_commands(&plan.commands)
            {
                cached.intent_format = Some(self.state.target_context.format.clone());
            }
        }
    }

    pub(super) fn release_geometry_after_device_destroy(&mut self) {
        self.geometry.current_templates.clear();
        self.geometry
            .streaming
            .release_mesh_sources_after_device_destroy();
        self.geometry.streaming = streaming::GeometryStreaming::default();
        self.geometry.meshes.clear();
        self.geometry.pages = pages::GeometryPages::default();
        self.geometry.pipelines.clear();
        self.geometry.shadow_bindings.clear();
        self.geometry.shadow_layouts.clear();
        self.geometry.light_buffers.clear();
        self.geometry.environment_buffers.clear();
        self.geometry.lit_bindings.clear();
        self.geometry.lit_layouts.clear();
        self.geometry.post = None;
        self.geometry.colors.clear();
        self.geometry.depths.clear();
        self.geometry.clears.clear();
        self.geometry.parameters = None;
        self.geometry.gpu_scale = Default::default();
        self.geometry.validated_plan = None;
    }
    /// Stricter visual subprofiles cannot raise the Windows 4GiB ceiling.
    /// Independent of the existing upload pool budget. No eviction or waits.
    pub fn set_geometry_visual_resident_limit(&mut self, bytes: u64) -> Result<(), String> {
        if bytes == 0 || bytes > MAX_VISUAL_RESIDENT_BYTES {
            return Err("geometry.invalid_visual_resident_limit".into());
        }
        if self.geometry_backing_bytes() > bytes {
            return Err("geometry.visual_resident_budget".into());
        }
        self.geometry.visual_resident_limit = bytes;
        Ok(())
    }

    pub fn retained_geometry_stats(&self) -> RetainedGeometryStats {
        RetainedGeometryStats {
            template_memo_writes: self.geometry.native_templates.memo_writes.get(),
            ..self.geometry.stats
        }
    }

    /// Explicit verification only. Reuses the physical admission owner without
    /// walking CPU payloads or constructing the allocation-report JSON/history.
    pub fn requested_gpu_backing_bytes_for_measurement(&self) -> u64 {
        self.geometry_backing_bytes()
    }

    pub fn geometry_residency(&self) -> GeometryResidency {
        GeometryResidency {
            meshes: self.geometry.meshes.len(),
            mesh_bytes: self
                .geometry
                .meshes
                .values()
                .map(|m| (m.source.vertex_byte_len() + m.source.index_byte_len()) as u64)
                .sum(),
            physical_mesh_bytes: self.geometry.pages.physical_bytes(),
            textures: self.owned_geometry_textures.len(),
            texture_bytes: self
                .owned_geometry_textures
                .iter()
                .filter_map(|handle| self.textures.get(handle))
                .map(|t| t.byte_len())
                .sum(),
            depth_targets: self.geometry.depths.len(),
            pipelines: self.geometry.pipelines.len(),
        }
    }

    /// Actual mesh buffer uploads, independent of logical Prepared/intent counts.
    pub fn geometry_upload_count(&self) -> usize {
        self.geometry.uploaded_meshes
    }

    pub(super) fn prepare_geometry_plan(
        &mut self,
        plan: &RhiCommandPlan,
        target: &wgpu::TextureView,
        rect: Option<GameViewRect>,
    ) -> Result<(), String> {
        self.prepare_geometry_frame_resources(
            plan,
            [target.texture().width(), target.texture().height()],
            rect,
        )
    }

    /// Reuse outputs only after the original complete plan and today's native
    /// resources passed preparation. A failed/partial output preparation is retried.
    pub(in crate::wgpu_backend) fn prepare_geometry_frame_resources(
        &mut self,
        plan: &RhiCommandPlan,
        extent: [u32; 2],
        rect: Option<GameViewRect>,
    ) -> Result<(), String> {
        self.prepare_geometry_plan_for_extent(plan, extent, rect)?;
        if self
            .geometry
            .validated_plan
            .as_ref()
            .is_some_and(|p| p.outputs_prepared)
        {
            return Ok(());
        }
        self.prepare_geometry_color_targets(plan)?;
        self.prepare_color_resolves(plan)?;
        if let Some(cached) = self.geometry.validated_plan.as_mut() {
            cached.outputs_prepared = true;
        }
        Ok(())
    }

    pub(in crate::wgpu_backend) fn prepare_geometry_plan_for_extent(
        &mut self,
        plan: &RhiCommandPlan,
        extent: [u32; 2],
        rect: Option<GameViewRect>,
    ) -> Result<(), String> {
        let skins_current = self.geometry_skins_current(plan);
        let reusable = skins_current
            && self.geometry.validated_plan.as_ref().is_some_and(|cached| {
                cached.extent == extent
                    && cached.rect == rect
                    && cached.output_format == self.format
                    && cached.schedule == plan.graph_schedule
                    && cached.matches_commands(&plan.commands)
                    && cached
                        .meshes
                        .iter()
                        .all(|(handle, _)| !self.geometry.streaming.pending(handle))
                    && residency::sources_match(
                        &cached.meshes,
                        self.geometry.meshes.len(),
                        self.geometry
                            .meshes
                            .iter()
                            .map(|(handle, mesh)| (handle, &mesh.source)),
                        |handle| self.geometry.meshes.get(handle).map(|mesh| &mesh.source),
                    )
                    && cached
                        .textures
                        .iter()
                        .all(|handle| self.textures.contains_key(handle))
            });
        if reusable {
            self.admit_gpu_retirement(0, 0)?;
            // Capture occurs only after parameters were installed successfully.
            // Full command equality includes transforms, color, order and view;
            // resource admission above remains mandatory on every reuse.
            return Ok(());
        }
        // Any changed preparation invalidates occlusion even if its logical
        // commands happen to share storage; native resources and contents matter.
        self.invalidate_gpu_scale()?;
        // Hold old templates only through this attempt, so the weak native memo
        // can compare current objects. A failed attempt cannot leave obsolete
        // native handles pinned by an invalid plan after retirement completes.
        let mut previous_current_templates = std::mem::take(&mut self.geometry.current_templates);
        let previous_outputs_prepared = self
            .geometry
            .validated_plan
            .as_ref()
            .is_some_and(|p| p.outputs_prepared);
        let previous_capabilities_validated = self
            .geometry
            .validated_plan
            .as_ref()
            .is_some_and(|plan| plan.capabilities_validated.get());
        let mut previous_native_templates = self
            .geometry
            .validated_plan
            .as_mut()
            .map(|previous| std::mem::take(&mut previous.native_templates));
        if let Some(previous) = self.geometry.validated_plan.as_mut() {
            previous.command_backing = std::sync::Weak::new();
            previous.outputs_prepared = false;
            previous.capabilities_validated.set(false);
            for recorded in previous.bundles.values_mut() {
                recorded.ready = false;
            }
        }
        self.geometry.native_templates.prune();
        let validation_device = self.device.clone();
        let preview = self.geometry.preview;
        let staging = std::sync::Mutex::new(
            if plan.commands.len() >= 512
                && plan
                    .commands
                    .iter()
                    .filter_map(RhiCommand::geometry_payload)
                    .any(|p| p.geometry_instance_count() != 0)
            {
                self.geometry
                    .parameters
                    .as_mut()
                    .map(ParameterStaging::take)
            } else {
                None
            },
        );
        let mut selection = None;
        let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::render_geometry::join_validation(
                plan.commands.len() >= 512,
                || {
                    validate_parameter_plan(plan, &validation_device)?;
                    if let Some(state) = staging
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .as_mut()
                    {
                        state.stage(plan, preview);
                    }
                    Ok(())
                },
                || {
                    if skins_current
                        && self.geometry.validated_plan.as_ref().is_some_and(|cached| {
                            cached.moving_views.as_ref().is_some_and(|moving| {
                                previous_native_templates.as_ref().is_some_and(|templates| {
                                    moving.matches(self, plan, cached, templates, extent, rect)
                                })
                            })
                        })
                    {
                        return Ok(None);
                    }
                    selection = self
                        .geometry
                        .validated_plan
                        .as_ref()
                        .filter(|_| skins_current)
                        .and_then(|cached| {
                            cached
                                .moving_views
                                .as_ref()?
                                .select(self, plan, cached, extent, rect)
                        });
                    if selection.is_some() {
                        return Ok(None);
                    }
                    let view_count = plan
                        .commands
                        .iter()
                        .filter(
                            |c| matches!(c,RhiCommand::BeginView{view,..} if !view.is_shadow_map()),
                        )
                        .count();
                    let shadow_count = plan
                        .commands
                        .iter()
                        .filter(
                            |c| matches!(c,RhiCommand::BeginView{view,..} if view.is_shadow_map()),
                        )
                        .count();
                    if view_count > parameters::MAX_COLOR_VIEWS
                        || shadow_count > parameters::MAX_SHADOW_VIEWS
                    {
                        return Err("geometry.view_capacity".into());
                    }
                    let [width, height] = extent;
                    let color_layout = self.geometry_color_layout(plan)?;
                    let mut views = BTreeMap::new();
                    let mut used_depths = BTreeSet::new();
                    let mut depth_extents = BTreeMap::new();
                    let mut depth_validation =
                        crate::render_geometry::DepthAttachmentValidation::default();
                    let mut used_pipelines = BTreeSet::new();
                    let mut pipeline_draws = Vec::new();
                    let mut texture_bindings = std::collections::HashSet::new();
                    let mut required_textures = BTreeSet::new();
                    let mut source_admission = residency::PlanSources::with_dense_capacity(
                        plan.commands.len(),
                        |handle: &RenderResourceHandle| handle.index as usize,
                    );
                    let current_meshes = residency::RegistryLookup::new(
                        &self.geometry.meshes,
                        plan.commands.len(),
                        |handle| handle.index as usize,
                    );
                    let mut view_validation = crate::render_geometry::DrawViewValidation::default();
                    let mut checked_begun: Option<DrawAttachments<'_>> = None;
                    let device_limits = self.device.limits();
                    // Validate the entire geometry plan before uploads or any particle simulation.
                    let geometry_only = Self::geometry_command_shape(plan);
                    let mut requirements_by_command = vec![None; plan.commands.len()];
                    let mut bindings = preparation::Bindings::default();
                    for (command_index, command) in plan.commands.iter().enumerate() {
                        match command {
                            RhiCommand::BeginView {
                                target,
                                view,
                                depth_target,
                            } => {
                                checked_begun = None;
                                if views
                                    .insert(
                                        view.view_id,
                                        (target.as_str(), view.as_ref(), depth_target.as_deref()),
                                    )
                                    .is_some()
                                    || depth_target.is_some()
                                        != (view.depth_policy != DepthPolicy::Disabled)
                                {
                                    return Err("geometry.invalid_view_begin".into());
                                }
                                let output = color_layout.get(target);
                                let [w, h] = output.copied().unwrap_or([width, height]);
                                geometry_viewport(
                                    view,
                                    w,
                                    h,
                                    if output.is_some() { None } else { rect },
                                )?;
                                if let Some(depth) = depth_target {
                                    depth_validation
                                        .insert(depth, view)
                                        .map_err(|e| e.to_string())?;
                                    if view.is_shadow_map() && target != depth {
                                        return Err("shadow.target_mismatch".into());
                                    }
                                    if view
                                        .target_extent
                                        .iter()
                                        .any(|v| *v > self.device.limits().max_texture_dimension_2d)
                                    {
                                        return Err("shadow.device_limit".into());
                                    }
                                    used_depths.insert(depth.clone());
                                    depth_extents.insert(depth.clone(), view.target_extent);
                                }
                            }
                            RhiCommand::ResolveColor3d { resolve } => {
                                let output = color_layout.get(&resolve.target);
                                let [w, h] = output.copied().unwrap_or([width, height]);
                                checked_output_region(
                                    resolve.region(),
                                    resolve.extent,
                                    w,
                                    h,
                                    if output.is_some() { None } else { rect },
                                )?;
                            }
                            RhiCommand::Draw {
                                target,
                                draw_kind,
                                vertex_count,
                                payload,
                            } if payload.geometry_draw().is_some() => {
                                let draw = payload.geometry_draw().unwrap();
                                self.geometry.stats.validation_draw_visits += 1;
                                draw.validate_shared_with_view(&mut view_validation)
                                    .map_err(|e| e.to_string())?;
                                let handle = draw.geometry_handle().raw();
                                if self.geometry.streaming.pending(&handle) {
                                    return Err("geometry.upload_not_completed".into());
                                }
                                let attachments = (
                                    draw.target.as_ref(),
                                    draw.view.as_ref(),
                                    draw.depth_target.as_deref(),
                                );
                                let begun = checked_begun
                                    .is_some_and(|old| matching_draw_attachments(old, attachments))
                                    || views.get(&draw.view.view_id).is_some_and(|old| {
                                        matching_draw_attachments(*old, attachments)
                                    });
                                if draw_kind != &RhiDrawKind::Geometry
                                    || *vertex_count != draw.index_count
                                    || target.as_ref() != draw.target.as_ref()
                                    || !begun
                                {
                                    return Err("geometry.invalid_draw_view".into());
                                }
                                checked_begun = Some(attachments);
                                // The matching BeginView above validated exactly this full
                                // view, target extent and output rect. Keep all per-draw
                                // value/range/binding checks, without revalidating its viewport.
                                if !source_admission.include(handle, &draw.geometry, |handle| {
                                    current_meshes.get(handle).map(|old| &old.source)
                                }) {
                                    return Err("geometry.resource_identity_conflict".into());
                                }
                                let requirements = self
                                    .geometry
                                    .preparation
                                    .resolve(draw, draw_color_format(draw, self.format));
                                if bindings.include_pipeline(&requirements, &mut used_pipelines) {
                                    pipeline_draws.push((requirements.pipeline().clone(), draw));
                                }
                                if texture_bindings.insert(Arc::as_ptr(&draw.material_binding)) {
                                    required_textures.extend(
                                        requirements
                                            .textures()
                                            .into_iter()
                                            .flatten()
                                            .map(|h| h.raw()),
                                    );
                                }
                                if draw.geometry.vertex_stride()
                                    > device_limits.max_vertex_buffer_array_stride
                                    || draw.geometry.vertex_byte_len() as u64
                                        > device_limits.max_buffer_size
                                    || draw.geometry.index_byte_len() as u64
                                        > device_limits.max_buffer_size
                                {
                                    return Err("geometry.device_limit".into());
                                }
                                bindings.include(draw, &requirements);
                                requirements_by_command[command_index] = Some(requirements);
                            }
                            _ => (),
                        }
                    }
                    // Detach the unique closure before any resident owner can mutate. Only
                    // unique source Arcs are retained, never draw descriptors or leases.
                    let geometry_sources = source_admission.into_sources();

                    Ok(Some((
                        views,
                        used_depths,
                        depth_extents,
                        used_pipelines,
                        pipeline_draws,
                        required_textures,
                        geometry_sources,
                        geometry_only,
                        requirements_by_command,
                        bindings,
                    )))
                },
            )
        }));
        // Restore CPU ownership even when a worker or the inline preparation
        // fails or unwinds. No GPU content was installed by the worker.
        let staged = staging
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map(|state| {
                state.restore(
                    self.geometry
                        .parameters
                        .as_mut()
                        .expect("same parameter arena"),
                )
            });
        let prepared = match prepared {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        };
        let (_, prepared) = prepared?;
        let Some((
            views,
            used_depths,
            depth_extents,
            used_pipelines,
            pipeline_draws,
            required_textures,
            geometry_sources,
            geometry_only,
            requirements_by_command,
            bindings,
        )) = prepared
        else {
            if let Some(selection) = selection {
                return selection.install(
                    self,
                    plan,
                    staged.flatten(),
                    &mut previous_current_templates,
                    &mut previous_native_templates,
                    (previous_outputs_prepared, previous_capabilities_validated),
                );
            }
            self.admit_gpu_retirement(0, 0)?;
            self.install_geometry_parameters(plan, staged.flatten())?;
            self.geometry.current_templates = previous_current_templates;
            let cached = self
                .geometry
                .validated_plan
                .as_mut()
                .expect("matched current plan");
            cached.native_templates = previous_native_templates
                .take()
                .expect("matched native objects");
            cached.command_backing = plan.commands.weak_backing();
            cached.outputs_prepared = previous_outputs_prepared;
            cached
                .capabilities_validated
                .set(previous_capabilities_validated);
            cached.intent_format = None;
            return Ok(());
        };
        let used_meshes: BTreeSet<_> = geometry_sources.keys().copied().collect();
        shadows::validate_plan(plan)?;
        // The ordered geometry scan above already resolved this complete
        // requirement set. Positive admission reads every distinct handle from
        // today's GPU registry. Mixed commands or any missing handle retain the
        // original ordered scan, including its exact role/first-error behavior.
        if !geometry_only
            || !required_textures.iter().all(|handle| {
                #[cfg(test)]
                super::CURRENT_TEXTURE_CLOSURE_QUERIES.with(|n| n.set(n.get() + 1));
                self.textures.contains_key(handle)
            })
        {
            self.validate_plan_texture_residency(plan)?;
        }
        // A previous frame's alias assignment cannot survive a topology change.
        let mut depth_layout: Vec<(String, String)> = Vec::new();
        for id in &used_depths {
            let slot = plan.graph_schedule.as_ref().and_then(|s| {
                s.lifetimes
                    .iter()
                    .find(|l| l.resource.0 == *id)
                    .and_then(|l| l.transient_slot)
            });
            let owner = depth_layout
                .iter()
                .find(|(other, owner)| {
                    slot.is_some()
                        && plan.graph_schedule.as_ref().is_some_and(|s| {
                            s.lifetimes
                                .iter()
                                .any(|l| l.resource.0 == *other && l.transient_slot == slot)
                        })
                        && depth_layout
                            .iter()
                            .filter(|(_, group)| group == owner)
                            .all(|(member, _)| depth_lifetimes_disjoint(plan, id, member))
                })
                .map(|(_, owner)| owner.clone())
                .unwrap_or_else(|| id.clone());
            depth_layout.push((id.clone(), owner));
        }
        let layout_changed = depth_layout != self.geometry.depth_layout;
        let mut retiring_bytes = 0u64;
        let mut retiring_objects = 0usize;
        // Mesh range guards are one retirement batch. Their physical pages
        // remain charged to the page pool until completion releases the ranges.
        let retired_meshes: Vec<_> = self
            .geometry
            .meshes
            .keys()
            .filter(|id| !used_meshes.contains(id) && !self.geometry.streaming.pinned(id))
            .copied()
            .collect();
        retiring_objects += usize::from(!retired_meshes.is_empty());
        for (id, (w, h, _)) in &self.geometry.depths {
            if layout_changed
                || !used_depths.contains(id)
                || depth_extents.get(id) != Some(&[*w, *h])
            {
                retiring_bytes = retiring_bytes.saturating_add(u64::from(*w) * u64::from(*h) * 4);
                retiring_objects += 1;
            }
        }
        retiring_objects += self
            .geometry
            .pipelines
            .keys()
            .filter(|id| !used_pipelines.contains(id.as_str()))
            .count();
        let clears = self
            .geometry
            .clears
            .iter()
            .filter(|(id, (old, _, _))| {
                views
                    .get(id)
                    .is_none_or(|(_, view, _)| *old != environment::clear_data(view))
            })
            .count();
        retiring_objects += clears;
        retiring_bytes = retiring_bytes.saturating_add(clears as u64 * environment::CLEAR_BYTES);
        let sources: Vec<_> = geometry_sources
            .iter()
            .filter(|(h, _)| !self.geometry.meshes.contains_key(h))
            .map(|(h, b)| (*h, (*b).clone()))
            .collect();
        // Before any native owner can retire an old object, discard recording
        // dependencies that the new preparation cannot guarantee to preserve.
        let selected =
            residency::RegistryLookup::new(&geometry_sources, plan.commands.len(), |h| {
                h.index as usize
            });
        let mut current = native_template::Current::for_plan(self, plan.commands.len());
        let discard: Vec<_> = self
            .geometry
            .validated_plan
            .as_ref()
            .into_iter()
            .flat_map(|previous| &previous.bundles)
            .filter(|(_, recorded)| {
                !geometry_only
                    || !recorded.recipe.survives_prepare_with(
                        self,
                        &selected,
                        &used_pipelines,
                        &mut current,
                    )
            })
            .map(|(start, _)| *start)
            .collect();
        self.admit_gpu_retirement(0, discard.len())?;
        for start in discard {
            let recorded = self
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .bundles
                .remove(&start)
                .unwrap();
            self.retire_gpu(recorded, 0);
        }
        self.admit_gpu_retirement(retiring_bytes, retiring_objects)?;
        let reserved = self.reserve_geometry_meshes(&sources)?;
        if layout_changed {
            for (_, old) in std::mem::take(&mut self.geometry.depths) {
                let bytes = u64::from(old.0) * u64::from(old.1) * 4;
                self.retire_gpu(old, bytes);
            }
            self.geometry.depth_layout = depth_layout;
        }
        let retired: Vec<_> = retired_meshes
            .into_iter()
            .map(|id| self.geometry.meshes.remove(&id).unwrap())
            .collect();
        if !retired.is_empty() {
            self.retire_gpu(retired, 0);
        }
        self.upload_geometry_meshes(plan.frame_index, reserved)?;
        self.prepare_geometry_skins(plan)?;
        for id in self
            .geometry
            .depths
            .keys()
            .filter(|id| !used_depths.contains(*id))
            .cloned()
            .collect::<Vec<_>>()
        {
            let old = self.geometry.depths.remove(&id).unwrap();
            let bytes = u64::from(old.0) * u64::from(old.1) * 4;
            self.retire_gpu(old, bytes);
        }
        for id in self
            .geometry
            .pipelines
            .keys()
            .filter(|id| !used_pipelines.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>()
        {
            let old = self.geometry.pipelines.remove(&id).unwrap();
            self.retire_gpu(old, 0);
        }
        for id in used_depths {
            let [width, height] = depth_extents[&id];
            if self
                .geometry
                .depths
                .get(&id)
                .is_none_or(|(w, h, _)| *w != width || *h != height)
            {
                if let Some(old) = self.geometry.depths.remove(&id) {
                    let bytes = u64::from(old.0) * u64::from(old.1) * 4;
                    self.retire_gpu(old, bytes);
                }
                let owner = self
                    .geometry
                    .depth_layout
                    .iter()
                    .find(|(resource, _)| resource == &id)
                    .map(|(_, owner)| owner);
                if let Some(owner) = owner.filter(|owner| *owner != &id) {
                    let previous = self.geometry.depths.get(owner).cloned();
                    if let Some((w, h, view)) =
                        previous.filter(|(w, h, _)| *w == width && *h == height)
                    {
                        self.geometry.depths.insert(id, (w, h, view));
                        continue;
                    }
                }
                self.admit_geometry_backing(u64::from(width) * u64::from(height) * 4)?;
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("geometry-view-depth"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                self.geometry.stats.depth_allocations += 1;
                self.geometry.depths.insert(
                    id,
                    (width, height, texture.create_view(&Default::default())),
                );
            }
        }
        if self.geometry.hdr_clear_pipeline.is_none()
            && plan.commands.iter().any(
                |c| matches!(c,RhiCommand::BeginView{view,..} if view.color_pipeline.is_some()),
            )
        {
            self.geometry.hdr_clear_pipeline = Some(clear_pipeline(
                &self.device,
                wgpu::TextureFormat::Rgba16Float,
                &self.geometry.uniform_layout,
            ));
        }
        self.prepare_shadow_bindings(plan.frame_index, bindings.shadows)?;
        self.prepare_scene_lights(plan.frame_index, bindings.lights)?;
        self.prepare_environments(plan.frame_index, bindings.environments)?;
        self.prepare_lit_bindings(bindings.materials)?;
        for (key, draw) in pipeline_draws {
            self.geometry
                .pipelines
                .entry(key.as_ref().to_owned())
                .or_insert_with(|| {
                    geometry_pipeline(
                        &self.device,
                        draw_color_format(draw, self.format),
                        &self.geometry.parameter_layout,
                        if draw.standard_lit.is_some() && !draw.is_opaque_shadow_depth_writer() {
                            &self.geometry.lit_layouts[&draw.lit_binding_mask()]
                        } else {
                            &self.texture_bind_group_layout
                        },
                        &self.geometry.light_layout,
                        draw.shadows
                            .as_ref()
                            .map(|maps| &self.geometry.shadow_layouts[&maps.texture_mask()]),
                        draw,
                    )
                });
        }
        let removed: Vec<_> = self
            .geometry
            .clears
            .iter()
            .filter(|(id, (old, _, _))| {
                views
                    .get(id)
                    .is_none_or(|(_, view, _)| *old != environment::clear_data(view))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in removed {
            let old = self.geometry.clears.remove(&id).unwrap();
            self.retire_gpu(old, environment::CLEAR_BYTES);
        }
        for (id, (_, view, _)) in &views {
            if view.clear_color.is_some() && !self.geometry.clears.contains_key(id) {
                self.admit_geometry_backing(environment::CLEAR_BYTES)?;
                let data = environment::clear_data(view);
                self.geometry.admit_visual_upload(plan.frame_index, 144)?;
                let (buffer, binding) =
                    uniform_binding(&self.device, &self.geometry.uniform_layout, &data);
                self.geometry.clears.insert(*id, (data, buffer, binding));
                self.geometry.stats.bind_groups_created += 1;
                self.geometry.stats.clear_uploaded_bytes += environment::CLEAR_BYTES;
            }
        }
        self.install_geometry_parameters(plan, staged.flatten())?;
        let (native_templates, current_templates) =
            self.geometry
                .native_templates
                .prepare(self, plan, &requirements_by_command);
        self.geometry.current_templates = template_backing::TemplateCollection::reuse(
            current_templates,
            &mut previous_current_templates,
        );
        let mut next = geometry_only
            .then(|| {
                ValidatedGeometryPlan::capture_prepared(
                    plan,
                    (extent, rect),
                    self.format,
                    geometry_sources.into_iter().collect(),
                    required_textures,
                    requirements_by_command,
                    None,
                )
            })
            .flatten();
        if let Some(next) = &mut next {
            next.native_templates = if let Some(previous) = &mut previous_native_templates {
                template_backing::TemplateCollection::reuse(native_templates, previous)
            } else {
                native_templates.into()
            };
        }
        self.finish_geometry_preparation(plan, next)
    }

    fn reserve_geometry_meshes(
        &mut self,
        sources: &[(RenderResourceHandle, Arc<GeometryBuffers>)],
    ) -> Result<Vec<(RenderResourceHandle, ResidentGeometry)>, String> {
        let maximum_mesh_bytes = if sources.is_empty() {
            u64::MAX
        } else {
            let other_visual = self
                .geometry_backing_bytes()
                .saturating_sub(self.geometry.pages.physical_bytes());
            let asset_other = self
                .textures
                .values()
                .map(|t| t.byte_len())
                .sum::<u64>()
                .saturating_add(self.gpu_lifecycle_stats().retiring_bytes)
                .saturating_add(
                    self.geometry
                        .environment_buffers
                        .values()
                        .map(|e| e.buffer.size())
                        .sum::<u64>(),
                );
            self.geometry
                .visual_resident_limit
                .saturating_sub(other_visual)
                .min(
                    self.geometry
                        .streaming
                        .plan_resident_budget()
                        .map_or(u64::MAX, |b| b.saturating_sub(asset_other)),
                )
        };
        self.geometry
            .pages
            .reserve(&self.device, sources, maximum_mesh_bytes)
    }

    fn upload_geometry_meshes(
        &mut self,
        frame_index: u64,
        reserved: Vec<(RenderResourceHandle, ResidentGeometry)>,
    ) -> Result<(), String> {
        // A complete validated plan may need more than one presentation's
        // transfer allowance. Retain admitted whole meshes in this same owner;
        // the loading clear submits their writes before the next allowance.
        let profiled = self.geometry.streaming.visual_uploads_profiled();
        let available = if profiled {
            self.geometry
                .streaming
                .visual_upload_allowance(frame_index)
                .1
        } else {
            usize::MAX
        };
        let mut mesh_upload_bytes = 0usize;
        let mut mesh_upload_count = 0;
        for (_, mesh) in &reserved {
            let bytes = mesh.source.vertex_byte_len() + (mesh.source.index_byte_len() + 3 & !3);
            if bytes > available.saturating_sub(mesh_upload_bytes) {
                break;
            }
            mesh_upload_bytes += bytes;
            mesh_upload_count += 1;
        }
        let partial_meshes = mesh_upload_count < reserved.len();
        self.geometry
            .admit_visual_upload(frame_index, mesh_upload_bytes)?;
        // Only immutable CPU sources cross the bounded preparation group.
        // The admitted prefix already bounds retained payload bytes; GPU page
        // owners, writes, publication and error order stay on this thread.
        let mut prepared = if profiled && mesh_upload_count > 1 {
            let sources: Vec<_> = reserved
                .iter()
                .take(mesh_upload_count)
                .map(|(_, mesh)| mesh.source.clone())
                .collect();
            Some(
                streaming::prepare_mesh_sources(mesh_upload_count, true, &|index| {
                    streaming::MeshUploadSource::acquire(sources[index].clone())
                })?
                .into_iter(),
            )
        } else {
            None
        };
        for (handle, mesh) in reserved.into_iter().take(mesh_upload_count) {
            let source = if let Some(prepared) = &mut prepared {
                prepared.next().expect("one result per admitted mesh")?
            } else {
                streaming::MeshUploadSource::acquire(mesh.source.clone())?
            };
            self.queue.write_buffer(
                mesh.vertices.buffer(),
                mesh.vertices.start,
                source.payload.vertex_bytes(),
            );
            let indices = source.payload.index_bytes();
            if indices.len() % 4 == 0 {
                self.queue
                    .write_buffer(mesh.indices.buffer(), mesh.indices.start, indices);
            } else {
                let mut padded = indices.to_vec();
                padded.resize((padded.len() + 3) & !3, 0);
                self.queue
                    .write_buffer(mesh.indices.buffer(), mesh.indices.start, &padded);
            }
            self.geometry.streaming.queued_mesh_sources.push(source);
            self.geometry.meshes.insert(handle, mesh);
            self.geometry.uploaded_meshes += 1;
            self.state.uploaded_resource_count += 1;
        }
        if partial_meshes {
            // Unwritten range guards dropped above never reached the queue.
            // Do not certify the plan or draw until all remaining owners finish.
            return Err("geometry.visual_upload_capacity".into());
        }
        Ok(())
    }

    fn finish_geometry_preparation(
        &mut self,
        plan: &RhiCommandPlan,
        mut next: Option<ValidatedGeometryPlan>,
    ) -> Result<(), String> {
        let mut starts = BTreeSet::new();
        if let Some(next) = &next {
            let mut first = 0u32;
            for (start, end) in &next.draw_ends {
                starts.extend(
                    self.geometry_recording_ranges(&plan.commands[*start..*end], first)
                        .into_iter()
                        .map(|(_, _, key)| key),
                );
                first += plan.commands[*start..*end]
                    .iter()
                    .map(|command| command.geometry_instance_count() as u32)
                    .sum::<u32>();
            }
        }
        let obsolete: Vec<_> = self
            .geometry
            .validated_plan
            .as_ref()
            .into_iter()
            .flat_map(|previous| previous.bundles.keys())
            .filter(|key| !starts.contains(*key))
            .copied()
            .collect();
        self.admit_gpu_retirement(0, obsolete.len())?;
        for start in obsolete {
            let recorded = self
                .geometry
                .validated_plan
                .as_mut()
                .unwrap()
                .bundles
                .remove(&start)
                .unwrap();
            self.retire_gpu(recorded, 0);
        }
        if let (Some(next), Some(previous)) = (&mut next, &mut self.geometry.validated_plan) {
            next.bundles = std::mem::take(&mut previous.bundles);
        }
        if let Some(next) = &mut next {
            next.recording_keys = starts.into_iter().collect();
            next.native_templates.mark_recording_input();
            self.geometry.current_templates.mark_recording_input();
        }
        self.geometry.validated_plan = next;
        Ok(())
    }

    pub(super) fn encode_geometry_begin(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        view: &ExecutableRenderView,
        depth: Option<&str>,
        rect: Option<GameViewRect>,
    ) -> Result<(), String> {
        let viewport = geometry_viewport(
            view,
            target.texture().width(),
            target.texture().height(),
            rect,
        )?;
        let depth = depth
            .and_then(|id| self.geometry.depths.get::<str>(id.as_ref()))
            .map(|(_, _, v)| v);
        self.geometry.stats.passes += 1;
        let color_attachments = if view.is_shadow_map() {
            vec![]
        } else {
            vec![Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })]
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("geometry-view-begin"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: depth.map(|depth_view| {
                wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: if view.is_shadow_map() && view.depth_clear.is_none() {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(1.)
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // A separate color-only pass permits the same clear pipeline with/without depth.
        drop(pass);
        if view.clear_color.is_some() {
            self.geometry.stats.passes += 1;
            pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("geometry-view-color-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            set_viewport(&mut pass, viewport);
            pass.set_pipeline(
                if target.texture().format() == wgpu::TextureFormat::Rgba16Float {
                    self.geometry.hdr_clear_pipeline.as_ref().unwrap()
                } else {
                    &self.geometry.clear_pipeline
                },
            );
            pass.set_bind_group(0, &self.geometry.clears[&view.view_id].2, &[]);
            pass.draw(0..3, 0..1);
        }
        Ok(())
    }

    #[cfg(test)]
    fn prepare_geometry_parameters(&mut self, plan: &RhiCommandPlan) -> Result<(), String> {
        self.install_geometry_parameters(plan, None)
    }

    fn install_geometry_parameters(
        &mut self,
        plan: &RhiCommandPlan,
        staged: Option<parameters::Update>,
    ) -> Result<(), String> {
        let mut draws = plan
            .commands
            .iter()
            .filter_map(RhiCommand::geometry_payload)
            .flat_map(geometry_instances)
            .peekable();
        if draws.peek().is_none() {
            return Ok(());
        }
        if self.geometry.parameters.is_none() {
            self.admit_geometry_backing(PARAMETER_ARENA_BYTES + INDEX_ARENA_BYTES)?;
            self.geometry.parameters = Some(RetainedParameters::new(
                &self.device,
                &self.geometry.parameter_layout,
            ));
            self.geometry.stats.bind_groups_created += 1;
            self.geometry.stats.resident_parameter_bytes =
                PARAMETER_ARENA_BYTES + INDEX_ARENA_BYTES;
            self.geometry.stats.peak_parameter_bytes = self
                .geometry
                .stats
                .peak_parameter_bytes
                .max(self.geometry.stats.resident_parameter_bytes);
        }
        let arena = self.geometry.parameters.as_mut().unwrap();
        let parameters::Update {
            mut instances,
            mut views,
            mut indices,
        } = staged.unwrap_or_else(|| {
            parameters::stage_payloads(
                plan.commands
                    .iter()
                    .filter_map(RhiCommand::geometry_payload),
                self.geometry.preview,
                &mut arena.derived_instances,
                &mut arena.instances,
                &mut arena.views,
            )
        });
        self.geometry.stats.parameter_draw_visits += indices.len() as u64;
        // Unique singleton draws may use their existing instance slots as
        // native selection positions. Visibility changes then leave every
        // surviving draw's encoder range intact. Duplicate/multiview/batched
        // selections keep the original compact layout and ordering.
        let mut occupied = vec![false; MAX_GEOMETRY_INSTANCES];
        let singleton = indices.len() >= 256
            && plan
                .commands
                .iter()
                .filter_map(RhiCommand::geometry_payload)
                .all(|p| p.geometry_instance_count() == 1)
            && indices.iter().all(|row| {
                let used = &mut occupied[row[0] as usize];
                let unique = !*used;
                *used = true;
                unique
            });
        let selection = singleton.then(|| indices.iter().map(|row| row[0]).collect::<Vec<_>>());
        if let Some(selection) = &selection {
            let length = selection.iter().copied().max().unwrap() as usize + 1;
            let mut stable = if arena.selection.is_some() {
                arena.indices.clone()
            } else {
                Vec::new()
            };
            stable.resize(stable.len().max(length), [0; 3]);
            for (&slot, row) in selection.iter().zip(&indices) {
                stable[slot as usize] = *row;
            }
            indices = stable;
        }
        let index_updates = parameters::index_updates(&arena.indices, &indices);
        let index_bytes = index_updates.iter().map(|range| range.len()).sum::<usize>()
            * parameters::INDEX_BYTES as usize;
        let mut upload_bytes = instances.len() * INSTANCE_BYTES as usize
            + views.len() * VIEW_BYTES as usize
            + index_bytes;
        let mut partial = false;
        // CPU slot assignment is retryable. Only writes admitted by the original
        // per-Present owner may commit content; no unuploaded clean records.
        if self.geometry.streaming.visual_uploads_profiled() {
            let (per_frame, remaining) = self
                .geometry
                .streaming
                .visual_upload_allowance(plan.frame_index);
            if upload_bytes > per_frame {
                let view_count = remaining / VIEW_BYTES as usize;
                let pending_views =
                    views.split_off(&views.keys().nth(view_count).copied().unwrap_or(u32::MAX));
                let remaining = remaining - views.len() * VIEW_BYTES as usize;
                let instance_count = remaining / INSTANCE_BYTES as usize;
                let pending_instances = instances.split_off(
                    &instances
                        .keys()
                        .nth(instance_count)
                        .copied()
                        .unwrap_or(u32::MAX),
                );
                partial =
                    !pending_views.is_empty() || !pending_instances.is_empty() || index_bytes != 0;
                upload_bytes =
                    instances.len() * INSTANCE_BYTES as usize + views.len() * VIEW_BYTES as usize;
                if upload_bytes == 0 {
                    return Err("geometry.visual_upload_capacity".into());
                }
            }
            self.geometry
                .streaming
                .admit_visual_upload(plan.frame_index, upload_bytes)?;
        }
        let content_changed =
            !instances.is_empty() || !views.is_empty() || arena.indices != indices;
        let (instance_writes, instance_bytes) =
            parameters::write(&self.queue, &arena.buffer, &mut arena.instances, instances);
        let (view_writes, view_bytes) =
            parameters::write(&self.queue, &arena.view_buffer, &mut arena.views, views);
        self.geometry.stats.parameter_uploads += instance_writes + view_writes;
        self.geometry.stats.parameter_uploaded_bytes += instance_bytes + view_bytes;
        if !partial {
            for range in index_updates {
                self.queue.write_buffer(
                    &arena.index_buffer,
                    range.start as u64 * parameters::INDEX_BYTES,
                    bytemuck::cast_slice(&indices[range]),
                );
            }
            self.geometry.stats.index_uploaded_bytes += index_bytes as u64;
            arena.indices = indices;
            arena.selection = selection;
        }
        self.geometry.stats.installed_content_version += u64::from(content_changed);
        if partial {
            return Err("geometry.visual_upload_capacity".into());
        }
        Ok(())
    }

    pub(super) fn geometry_draw_end(&self, plan: &RhiCommandPlan, start: usize) -> usize {
        self.geometry.validated_plan.as_ref().map_or_else(
            || geometry_draw_end(&plan.commands, start),
            |cached| cached.draw_end(&plan.commands, start),
        )
    }

    /// Consume consecutive same-view draws in a single real pass; preserve command order.
    fn geometry_recording_ranges(
        &self,
        commands: &[RhiCommand],
        first: u32,
    ) -> Vec<(std::ops::Range<usize>, u32, u32)> {
        let Some(selection) = self
            .geometry
            .parameters
            .as_ref()
            .and_then(|arena| arena.selection.as_ref())
        else {
            return vec![(0..commands.len(), first, first)];
        };
        let mut ranges = Vec::new();
        let mut start = 0;
        while start < commands.len() {
            let slot = selection[first as usize + start];
            let bucket = slot / 128;
            let mut end = start + 1;
            while end < commands.len()
                && end - start < 128
                && selection[first as usize + end] / 128 == bucket
            {
                end += 1;
            }
            // A singleton slot occurs once in this entire selection, including
            // separate passes. The high bit separates compact ordinal keys.
            ranges.push((start..end, first + start as u32, 0x8000_0000 | slot));
            start = end;
        }
        ranges
    }

    pub(super) fn encode_geometry_draws(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        commands: &[RhiCommand],
        first_instance: u32,
        rect: Option<GameViewRect>,
    ) -> Result<(), String> {
        let Some(draw) = commands.first().and_then(geometry_draw) else {
            return Err("geometry.invalid_draw_range".into());
        };
        let viewport = geometry_viewport(
            &draw.view,
            target.texture().width(),
            target.texture().height(),
            rect,
        )?;
        if self.try_encode_gpu_scale(encoder, target, commands, first_instance, viewport)? {
            return Ok(());
        }
        // prepare_geometry_plan has already checked the entire command plan,
        // attachment extent/rect, mesh sources, residency and retirement admission.
        // Only that one successful bounded plan may retain recorded native draws.
        let ranges = if self.geometry.validated_plan.is_some() {
            self.geometry_recording_ranges(commands, first_instance)
        } else {
            vec![(0..commands.len(), first_instance, first_instance)]
        };
        let mut retained_bundles = Vec::with_capacity(ranges.len());
        let mut fallback_recipe = None;
        for (range, first_instance, recording_key) in ranges {
            let commands = &commands[range];
            let mut retained = self
                .geometry
                .validated_plan
                .as_ref()
                .and_then(|plan| plan.bundles.get(&recording_key))
                .filter(|recorded| {
                    recorded.command_count == commands.len()
                        && (recorded.ready
                            || recorded.recipe.matches_prepared_commands(
                                self,
                                commands,
                                first_instance,
                            ))
                })
                .map(|recorded| (recorded.bundle.clone(), recorded.draws));
            if retained.is_some() {
                self.geometry
                    .validated_plan
                    .as_mut()
                    .unwrap()
                    .bundles
                    .get_mut(&recording_key)
                    .unwrap()
                    .ready = true;
            }
            if retained.is_none() {
                let recipe = encoding::Recipe::capture_commands(self, commands, first_instance)?;
                let matches = self
                    .geometry
                    .validated_plan
                    .as_ref()
                    .and_then(|plan| plan.bundles.get(&recording_key))
                    .is_some_and(|recorded| recorded.recipe == recipe);
                if matches {
                    let recorded = self
                        .geometry
                        .validated_plan
                        .as_mut()
                        .unwrap()
                        .bundles
                        .get_mut(&recording_key)
                        .unwrap();
                    recorded.ready = true;
                    retained = Some((recorded.bundle.clone(), recorded.draws));
                } else {
                    let has_old = self
                        .geometry
                        .validated_plan
                        .as_ref()
                        .is_some_and(|plan| plan.bundles.contains_key(&recording_key));
                    self.admit_gpu_retirement(0, usize::from(has_old))?;
                    if let Some(old) = self
                        .geometry
                        .validated_plan
                        .as_mut()
                        .and_then(|plan| plan.bundles.remove(&recording_key))
                    {
                        self.retire_gpu(old, 0);
                    }
                    if let Some(plan) = &mut self.geometry.validated_plan {
                        let color_formats = if draw.view.is_shadow_map() {
                            vec![]
                        } else {
                            vec![Some(draw_color_format(draw, self.format))]
                        };
                        let mut bundle = self.device.create_render_bundle_encoder(
                            &wgpu::RenderBundleEncoderDescriptor {
                                label: Some("geometry-retained-plan-draws"),
                                color_formats: &color_formats,
                                depth_stencil: draw.depth_target.as_ref().map(|_| {
                                    wgpu::RenderBundleDepthStencil {
                                        format: wgpu::TextureFormat::Depth32Float,
                                        depth_read_only: false,
                                        stencil_read_only: true,
                                    }
                                }),
                                sample_count: 1,
                                multiview: None,
                            },
                        );
                        let recorded = recipe.record(&mut bundle);
                        let bundle = bundle.finish(&wgpu::RenderBundleDescriptor {
                            label: Some("geometry-retained-plan-draws"),
                        });
                        plan.bundles.insert(
                            recording_key,
                            encoding::Recorded {
                                recipe,
                                bundle: bundle.clone(),
                                draws: recorded,
                                command_count: commands.len(),
                                ready: true,
                            },
                        );
                        retained = Some((bundle, recorded));
                    } else {
                        // Mixed plans have no reusable native plan; use one scalar
                        // recipe below rather than retaining partial recordings.
                        fallback_recipe = Some(recipe);
                    }
                }
            }
            if let Some(retained) = retained {
                retained_bundles.push(retained);
            }
        }
        let depth = draw
            .depth_target
            .as_ref()
            .and_then(|id| self.geometry.depths.get::<str>(id.as_ref()))
            .map(|(_, _, v)| v);
        let color_attachments = if draw.view.is_shadow_map() {
            vec![]
        } else {
            vec![Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })]
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("geometry-retained-view-draws"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: depth.map(|view| wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        set_viewport(&mut pass, viewport);
        let recorded = if !retained_bundles.is_empty() {
            pass.execute_bundles(retained_bundles.iter().map(|(bundle, _)| bundle));
            retained_bundles.iter().map(|(_, recorded)| recorded).sum()
        } else {
            fallback_recipe.as_ref().unwrap().record(&mut pass)
        };
        drop(pass);
        self.geometry.stats.passes += 1;
        self.geometry.stats.draws += recorded;
        self.geometry.stats.instances += commands
            .iter()
            .map(RhiCommand::geometry_instance_count)
            .sum::<usize>() as u64;
        Ok(())
    }
}

fn geometry_viewport(
    view: &ExecutableRenderView,
    width: u32,
    height: u32,
    rect: Option<GameViewRect>,
) -> Result<GameViewRect, String> {
    #[cfg(test)]
    GEOMETRY_VIEWPORT_CHECKS.with(|n| n.set(n.get() + 1));
    if view.is_shadow_map() {
        viewport(view, view.target_extent[0], view.target_extent[1], None)
    } else {
        viewport(view, width, height, rect)
    }
}

fn viewport(
    view: &ExecutableRenderView,
    width: u32,
    height: u32,
    rect: Option<GameViewRect>,
) -> Result<GameViewRect, String> {
    view.validate_extent(view.target_extent[0], view.target_extent[1])
        .map_err(|e| e.to_string())?;
    checked_output_region(view.viewport, view.target_extent, width, height, rect)
}

fn checked_output_region(
    region: [u32; 4],
    extent: [u32; 2],
    width: u32,
    height: u32,
    rect: Option<GameViewRect>,
) -> Result<GameViewRect, String> {
    let result = output_region(region, extent, rect);
    if rect.is_none() && extent != [width, height] {
        return Err("geometry.target_extent_mismatch".into());
    }
    let [sx, sy, sw, sh] = surface_content_scissor(result)?;
    if sx.checked_add(sw).is_none_or(|n| n > width) || sy.checked_add(sh).is_none_or(|n| n > height)
    {
        return Err("geometry.viewport_out_of_bounds".into());
    }
    Ok(result)
}

pub(in crate::wgpu_backend) fn output_region(
    [x, y, w, h]: [u32; 4],
    extent: [u32; 2],
    rect: Option<GameViewRect>,
) -> GameViewRect {
    if let Some(rect) = rect {
        GameViewRect {
            x: rect.x + x as f32 / extent[0] as f32 * rect.width,
            y: rect.y + y as f32 / extent[1] as f32 * rect.height,
            width: w as f32 / extent[0] as f32 * rect.width,
            height: h as f32 / extent[1] as f32 * rect.height,
        }
    } else {
        GameViewRect {
            x: x as f32,
            y: y as f32,
            width: w as f32,
            height: h as f32,
        }
    }
}

fn set_viewport(pass: &mut wgpu::RenderPass<'_>, rect: GameViewRect) {
    pass.set_viewport(rect.x, rect.y, rect.width, rect.height, 0., 1.);
    let [x, y, w, h] = surface_content_scissor(rect).unwrap();
    pass.set_scissor_rect(x, y, w, h);
}

fn uniform_binding(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    data: &[f32; 36],
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("geometry-draw-uniform"),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("geometry-draw-binding"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, binding)
}

const GEOMETRY_PARAMETERS_WGSL: &str = parameters::WGSL;

fn geometry_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform: &wgpu::BindGroupLayout,
    texture: &wgpu::BindGroupLayout,
    light_layout: &wgpu::BindGroupLayout,
    shadow_layout: Option<&wgpu::BindGroupLayout>,
    draw: &DrawGeometry,
) -> wgpu::RenderPipeline {
    let has_color = !draw.ignore_vertex_color
        && draw
            .geometry
            .attributes()
            .iter()
            .any(|a| a.semantic == VertexSemantic::Color0);
    let has_uv = draw
        .geometry
        .attributes()
        .iter()
        .any(|a| a.semantic == VertexSemantic::TexCoord0);
    let textured = draw.material_textures()[0].is_some();
    let mut source = String::from(GEOMETRY_PARAMETERS_WGSL);
    source.push_str("\nstruct Out { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32>, @location(1) uv: vec2<f32>, @location(2) @interpolate(flat) cutoff: f32, @location(3) world:vec3<f32>, @location(4) @interpolate(flat) slot:vec3<u32> };\n");
    if textured {
        source.push_str("@group(1) @binding(0) var tex: texture_2d<f32>; @group(1) @binding(1) var samp: sampler;\n");
    }
    source.push_str(
        "@vertex fn vs_main(@location(0) position: vec3<f32>, @location(3) slot: vec3<u32>",
    );
    if has_color {
        source.push_str(", @location(1) color: vec4<f32>");
    }
    if has_uv {
        source.push_str(", @location(2) uv: vec2<f32>");
    }
    source.push_str(
        ") -> Out { let u=load_parameters(slot); let world=u.world*vec4<f32>(position,1.0); var o: Out; o.position=u.matrix*world; o.color=u.color",
    );
    if has_color {
        source.push_str("*color");
    }
    source.push_str("; o.uv=");
    source.push_str(if has_uv { "uv" } else { "vec2<f32>(0.0,0.0)" });
    source.push_str("; o.cutoff=u.extra.x; o.world=world.xyz; o.slot=slot; return o; }\n@fragment fn fs_main(i:Out)->@location(0) vec4<f32> { let result = ");
    if draw.opaque_alpha {
        source.push_str("vec4<f32>((");
    }
    source.push_str("i.color");
    if textured {
        source.push_str("*textureSample(tex,samp,i.uv)");
    }
    if draw.opaque_alpha {
        source.push_str(").rgb,1.0)");
    }
    source.push_str("; ");
    if draw.blend == MaterialBlend::AlphaMask {
        source.push_str("if (result.a < i.cutoff) { discard; } ");
    }
    if draw.blend.is_premultiplied() {
        // Textures already contain linear premultiplied RGB. Tint alpha belongs
        // in RGB exactly once, independently of sampled texture alpha.
        source.push_str("return vec4<f32>(result.rgb*i.color.a,result.a); }");
    } else {
        source.push_str("return result; }");
    }
    if draw.view.color_pipeline.is_some() {
        source.push_str(environment::FOG_SHADER);
        source = source.replace("return result;", "let u=load_parameters(i.slot); return vec4<f32>(distance_fog(result.rgb,i.world,u.camera.xyz,u.fog,u.fog_settings),result.a);");
    }
    let shadow = draw.view.is_shadow_map();
    let position_only = draw.is_opaque_shadow_depth_writer();
    if position_only {
        // Opaque depth never samples alpha or writes color. Keep the original
        // parameter stride and per-instance slot; only fetch its matrix.
        source = format!(
            "{GEOMETRY_PARAMETERS_WGSL}\n@vertex fn vs_main(@location(0) position: vec3<f32>, @location(3) slot: vec3<u32>) -> @builtin(position) vec4<f32> {{ return views[slot.y].matrix * (instances[slot.x].world * vec4<f32>(position, 1.0)); }}"
        );
    } else if shadow {
        source = source
            .replace("->@location(0) vec4<f32>", "")
            .replace("return result;", "return;");
    } else if draw.standard_lit.is_some() {
        source = lit::shader(draw, has_color, has_uv);
    }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("geometry-unlit"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let mut layouts = if position_only {
        vec![uniform]
    } else if draw.lighting.is_some() {
        vec![uniform, texture, light_layout]
    } else if textured || draw.standard_lit.is_some() {
        vec![uniform, texture]
    } else {
        vec![uniform]
    };
    if let Some(shadows) = shadow_layout.filter(|_| !position_only) {
        layouts.push(shadows);
    }
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("geometry-layout"),
        bind_group_layouts: &layouts,
        push_constant_ranges: &[],
    });
    let attributes: Vec<_> = draw
        .geometry
        .attributes()
        .iter()
        .filter_map(|a| {
            if position_only && a.semantic != VertexSemantic::Position {
                return None;
            }
            let shader_location = match a.semantic {
                VertexSemantic::Position => 0,
                VertexSemantic::Color0 => 1,
                VertexSemantic::TexCoord0 => 2,
                VertexSemantic::Normal => {
                    if draw.standard_lit.is_some() {
                        4
                    } else {
                        return None;
                    }
                }
            };
            Some(wgpu::VertexAttribute {
                format: match a.format {
                    VertexFormat::Float32x2 => wgpu::VertexFormat::Float32x2,
                    VertexFormat::Float32x3 => wgpu::VertexFormat::Float32x3,
                    VertexFormat::Float32x4 => wgpu::VertexFormat::Float32x4,
                },
                offset: u64::from(a.offset),
                shader_location,
            })
        })
        .collect();
    let color_targets = if shadow {
        vec![]
    } else {
        vec![Some(wgpu::ColorTargetState {
            format,
            blend: match draw.blend {
                MaterialBlend::Opaque | MaterialBlend::AlphaMask => None,
                MaterialBlend::AlphaBlend => Some(wgpu::BlendState::ALPHA_BLENDING),
                mode => Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: if mode == MaterialBlend::PremultipliedMultiply {
                            wgpu::BlendFactor::Dst
                        } else {
                            wgpu::BlendFactor::One
                        },
                        dst_factor: match mode {
                            MaterialBlend::PremultipliedAdditive => wgpu::BlendFactor::One,
                            MaterialBlend::PremultipliedScreen => wgpu::BlendFactor::OneMinusSrc,
                            _ => wgpu::BlendFactor::OneMinusSrcAlpha,
                        },
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
            },
            write_mask: wgpu::ColorWrites::ALL,
        })]
    };
    let fragment = (!position_only).then_some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        compilation_options: Default::default(),
        targets: &color_targets,
    });
    #[cfg(test)]
    PIPELINE_STAGE_OBSERVATIONS.with(|v| {
        v.borrow_mut().push((
            fragment.is_some(),
            attributes.iter().map(|a| a.shader_location).collect(),
            layouts.len(),
        ))
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("geometry-unlit-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: u64::from(draw.geometry.vertex_stride()),
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                },
                wgpu::VertexBufferLayout {
                    array_stride: parameters::INDEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Uint32x3,
                        offset: 0,
                        shader_location: 3,
                    }],
                },
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: if draw.mirrored() {
                wgpu::FrontFace::Cw
            } else {
                wgpu::FrontFace::Ccw
            },
            cull_mode: match draw.cull {
                MaterialCull::None => None,
                MaterialCull::Back => Some(wgpu::Face::Back),
                MaterialCull::Front => Some(wgpu::Face::Front),
            },
            ..Default::default()
        },
        depth_stencil: draw.depth_target.as_ref().map(|_| wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: draw.depth == MaterialDepth::TestAndWrite,
            depth_compare: if draw.depth == MaterialDepth::Disabled {
                wgpu::CompareFunction::Always
            } else {
                wgpu::CompareFunction::LessEqual
            },
            stencil: Default::default(),
            bias: wgpu::DepthBiasState {
                constant: draw.decal_bias.unwrap_or(0),
                ..Default::default()
            },
        }),
        multisample: Default::default(),
        fragment,
        multiview: None,
        cache: None,
    })
}

fn clear_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("geometry-clear-sky"),
        source: wgpu::ShaderSource::Wgsl(environment::CLEAR_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("geometry-clear-layout"),
        bind_group_layouts: &[uniform],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("geometry-clear-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

const MAX_GEOMETRY_INSTANCES: usize = 65536;
// Shared instance parameters and ordered per-view index references have
// different units. Keep the existing parameter-row bound; bound references
// separately so color and shadow passes may reuse those rows.
const MAX_GEOMETRY_REFERENCES: usize = MAX_GEOMETRY_INSTANCES * 16;
#[cfg(test)]
const PARAMETER_BYTES: u64 = INSTANCE_BYTES + VIEW_BYTES;
const PARAMETER_ARENA_BYTES: u64 = INSTANCE_ARENA_BYTES + VIEW_ARENA_BYTES;
const INDEX_ARENA_BYTES: u64 = MAX_GEOMETRY_REFERENCES as u64 * parameters::INDEX_BYTES;
struct RetainedParameters {
    buffer: wgpu::Buffer,
    view_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    binding: wgpu::BindGroup,
    instances: parameters::Records<INSTANCE_FLOATS>,
    derived_instances: parameters::InstanceValues,
    views: parameters::Records<VIEW_FLOATS>,
    indices: Vec<[u32; 3]>,
    // Current logical draw ordinal -> existing GPU instance slot. No resource
    // ownership or cross-frame admission is stored here.
    selection: Option<Vec<u32>>,
}
// Temporary CPU-only preparation state, not another parameter/resource owner.
// Zero-capacity placeholders allocate no arena-sized free tree. The native
// buffers and uploaded content remain in RetainedParameters throughout.
struct ParameterStaging {
    instances: parameters::Records<INSTANCE_FLOATS>,
    views: parameters::Records<VIEW_FLOATS>,
    derived: parameters::InstanceValues,
    update: Option<parameters::Update>,
    #[cfg(test)]
    thread: Option<std::thread::ThreadId>,
}
impl ParameterStaging {
    fn take(arena: &mut RetainedParameters) -> Self {
        Self {
            instances: std::mem::replace(
                &mut arena.instances,
                parameters::Records::with_capacity(0),
            ),
            views: std::mem::replace(&mut arena.views, parameters::Records::with_capacity(0)),
            derived: std::mem::take(&mut arena.derived_instances),
            update: None,
            #[cfg(test)]
            thread: None,
        }
    }
    fn stage(&mut self, plan: &RhiCommandPlan, preview: [[f32; 4]; 3]) {
        // Parameter identity validation does not prove begun-view attachment
        // equality. Bound this speculative CPU branch by the exact ordinal key
        // used by stage_payloads; malformed/divergent views take the original
        // ordered main validation and installation path instead of panicking.
        let mut variants = std::collections::HashSet::new();
        for payload in plan
            .commands
            .iter()
            .filter_map(RhiCommand::geometry_payload)
        {
            if payload.geometry_instance_count() == 0 {
                continue;
            }
            let draw = payload.geometry_draw().expect("selected geometry");
            variants.insert((Arc::as_ptr(&draw.view), draw.lighting.is_some()));
            if variants.len() > parameters::MAX_VIEW_RECORDS {
                return;
            }
        }
        #[cfg(test)]
        {
            self.thread = Some(std::thread::current().id());
        }
        self.update = Some(parameters::stage_payloads(
            plan.commands
                .iter()
                .filter_map(RhiCommand::geometry_payload),
            preview,
            &mut self.derived,
            &mut self.instances,
            &mut self.views,
        ));
    }
    fn restore(self, arena: &mut RetainedParameters) -> Option<parameters::Update> {
        #[cfg(test)]
        PARAMETER_PREPARATION_THREAD.with(|n| n.set(self.thread));
        arena.instances = self.instances;
        arena.views = self.views;
        arena.derived_instances = self.derived;
        self.update
    }
}
impl RetainedParameters {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("geometry-retained-parameter-arena"),
            size: INSTANCE_ARENA_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("geometry-retained-view-arena"),
            size: VIEW_ARENA_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("geometry-retained-instance-indices"),
            size: INDEX_ARENA_BYTES,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("geometry-retained-parameter-binding"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: view_buffer.as_entire_binding(),
                },
            ],
        });
        Self {
            buffer,
            view_buffer,
            index_buffer,
            binding,
            instances: parameters::Records::default(),
            derived_instances: parameters::InstanceValues::default(),
            views: parameters::Records::with_capacity(parameters::MAX_VIEW_RECORDS),
            indices: Vec::new(),
            selection: None,
        }
    }
}
fn geometry_draw(command: &RhiCommand) -> Option<&DrawGeometry> {
    command.geometry_draw().map(Arc::as_ref)
}
fn geometry_instances(
    payload: &RhiDrawPayload,
) -> impl Iterator<
    Item = (
        &DrawGeometry,
        &Arc<crate::render_geometry::DrawInstance>,
        OrderedF32,
    ),
> {
    payload.geometry_instances()
}
type DrawAttachments<'a> = (&'a str, &'a ExecutableRenderView, Option<&'a str>);

fn matching_draw_attachments(a: DrawAttachments<'_>, b: DrawAttachments<'_>) -> bool {
    if a.0 != b.0 || a.2 != b.2 {
        return false;
    }
    if std::ptr::eq(a.1, b.1) {
        return true;
    }
    #[cfg(test)]
    NATIVE_VIEW_COMPARISONS.with(|n| n.set(n.get() + 1));
    a.1 == b.1
}

fn validate_parameter_plan(plan: &RhiCommandPlan, device: &wgpu::Device) -> Result<(), String> {
    let count = validate_parameter_identities(plan)?;
    if count != 0
        && (INSTANCE_ARENA_BYTES > u64::from(device.limits().max_storage_buffer_binding_size)
            || VIEW_ARENA_BYTES > u64::from(device.limits().max_storage_buffer_binding_size)
            || INSTANCE_ARENA_BYTES > device.limits().max_buffer_size
            || VIEW_ARENA_BYTES > device.limits().max_buffer_size
            || INDEX_ARENA_BYTES > device.limits().max_buffer_size)
    {
        return Err("geometry.parameter_device_limit".into());
    }
    Ok(())
}

fn parameter_view_identity(draw: &DrawGeometry) -> u64 {
    #[cfg(test)]
    PARAMETER_VIEW_IDENTITIES.with(|n| n.set(n.get() + 1));
    draw.view.view_id
}

// One immutable native preparation only. Dense table scratch has a shared
// 65536-row cap, and dense membership is restricted to at most sixteen views.
// Sparse/oversized/detached tables hash selected IDs; no hidden row is read.
struct ParameterRowOrdinals {
    ids: std::collections::HashMap<u64, u32>,
    tables: std::collections::HashMap<usize, Vec<u32>>,
    reserved_rows: usize,
}
impl ParameterRowOrdinals {
    fn for_payload(
        &mut self,
        payload: &RhiDrawPayload,
    ) -> (Option<&mut [u32]>, &mut std::collections::HashMap<u64, u32>) {
        let rows = if let RhiDrawPayload::GeometryBatch { batch } = payload {
            let key = Arc::as_ptr(&batch.instances) as usize;
            if !self.tables.contains_key(&key)
                && batch.instances.len() <= MAX_GEOMETRY_INSTANCES - self.reserved_rows
            {
                self.reserved_rows += batch.instances.len();
                self.tables
                    .insert(key, vec![u32::MAX; batch.instances.len()]);
            }
            self.tables.get_mut(&key).map(Vec::as_mut_slice)
        } else {
            None
        };
        (rows, &mut self.ids)
    }
}
enum ParameterViewIds {
    Ordinals(Vec<u64>),
    Values(std::collections::HashSet<u64>),
}
#[cfg(test)]
impl ParameterViewIds {
    fn capacity(&self) -> usize {
        match self {
            Self::Ordinals(_) => 0,
            Self::Values(values) => values.capacity(),
        }
    }
}

fn validate_parameter_identities(plan: &RhiCommandPlan) -> Result<usize, String> {
    let mut validation = crate::render_geometry::DrawViewValidation::default();
    // This call-local set proves membership only; no consumer observes key
    // order. Keep the full per-view identity and capacity check on every row
    // without constructing an ordered tree for the dynamic selection.
    // Metadata is a capacity hint, never admission: preserve the ordered full
    // batch/tail checks and the original per-row cross-batch identity checks.
    let mut capacities = BTreeMap::<u64, usize>::new();
    let mut references = 0usize;
    let mut unique_table: Option<&Arc<Vec<Arc<crate::render_geometry::DrawInstance>>>> = None;
    let mut single_unique_table = true;
    for payload in plan
        .commands
        .iter()
        .filter_map(RhiCommand::geometry_payload)
    {
        match payload {
            RhiDrawPayload::GeometryBatch { batch }
                if batch.unique_instances_proven()
                    && batch.instances.len() <= MAX_GEOMETRY_INSTANCES =>
            {
                if let Some(table) = unique_table {
                    single_unique_table &= Arc::ptr_eq(table, &batch.instances);
                } else {
                    unique_table = Some(&batch.instances);
                }
            }
            _ => single_unique_table = false,
        }
        references = references.saturating_add(payload.geometry_instance_count());
        let view = parameter_view_identity(payload.geometry_draw().expect("geometry payload"));
        let count = capacities.entry(view).or_default();
        *count = count
            .saturating_add(payload.geometry_instance_count())
            .min(MAX_GEOMETRY_INSTANCES);
    }
    // Capacity hints do not admit rows. Bound the total reservation even for
    // malformed plans; the ordered row checks below keep the original errors.
    let dense = capacities.len() <= 16;
    let direct_rows = dense && single_unique_table && unique_table.is_some();
    let reserve = capacities
        .values()
        .fold(0usize, |total, count| total.saturating_add(*count))
        .min(MAX_GEOMETRY_INSTANCES);
    let mut ordinals = (dense && !direct_rows).then(|| ParameterRowOrdinals {
        ids: std::collections::HashMap::with_capacity(reserve),
        tables: Default::default(),
        reserved_rows: 0,
    });
    let mut remaining = MAX_GEOMETRY_INSTANCES;
    let mut ids: BTreeMap<_, _> = capacities
        .into_iter()
        .map(|(view, count)| {
            let reserve = count.min(remaining);
            remaining -= reserve;
            (
                view,
                if dense {
                    ParameterViewIds::Ordinals(Vec::new())
                } else {
                    ParameterViewIds::Values(std::collections::HashSet::with_capacity(reserve))
                },
            )
        })
        .collect();
    // Larger multiview selections still fit the unchanged parameter arena.
    // Pointer membership is conservative: detached equal backings may refuse,
    // but distinct parameter content can never bypass the original row bound.
    let mut instance_backings = (references > MAX_GEOMETRY_INSTANCES)
        .then(|| std::collections::HashSet::with_capacity(MAX_GEOMETRY_INSTANCES));
    let mut selected_count = 0;
    for command in &plan.commands {
        let Some(payload) = command.geometry_payload() else {
            continue;
        };
        if let RhiDrawPayload::GeometryBatch { batch } = payload {
            batch
                .validate_shared_with_view(&mut validation)
                .map_err(|e| e.to_string())?;
        }
        let view = parameter_view_identity(payload.geometry_draw().expect("geometry payload"));
        let view_ids = ids
            .get_mut(&view)
            .expect("metadata includes every geometry view");
        let mut workspace = ordinals.as_mut().map(|o| o.for_payload(payload));
        for (row, (_, instance, _)) in geometry_instances(payload).enumerate() {
            if selected_count == MAX_GEOMETRY_REFERENCES {
                return Err("geometry.instance_capacity".into());
            }
            if let Some(backings) = &mut instance_backings {
                backings.insert(Arc::as_ptr(instance));
                if backings.len() > MAX_GEOMETRY_INSTANCES {
                    return Err("geometry.instance_capacity".into());
                }
            }
            #[cfg(test)]
            let old_capacity = view_ids.capacity();
            let unique = match view_ids {
                ParameterViewIds::Ordinals(bits) => {
                    let ordinal = if direct_rows {
                        let RhiDrawPayload::GeometryBatch { batch } = payload else {
                            unreachable!("qualified table domain")
                        };
                        // The ordered complete batch validation above admitted
                        // the selected index; full-table uniqueness does not.
                        batch.visible[row].instance_index as usize
                    } else {
                        let (slots, values) =
                            workspace.as_mut().expect("dense call ordinal domain");
                        let mut slot = match (slots.as_mut(), payload) {
                            (Some(slots), RhiDrawPayload::GeometryBatch { batch }) => {
                                Some(&mut slots[batch.visible[row].instance_index as usize])
                            }
                            _ => None,
                        };
                        (match slot.as_deref() {
                            Some(&value) if value != u32::MAX => value,
                            _ => {
                                #[cfg(test)]
                                DYNAMIC_PARAMETER_ID_HASHES.with(|n| n.set(n.get() + 1));
                                let next = values.len() as u32;
                                let ordinal = *values.entry(instance.instance_id).or_insert(next);
                                if let Some(slot) = slot.as_mut() {
                                    **slot = ordinal;
                                }
                                ordinal
                            }
                        }) as usize
                    };
                    let word = ordinal / 64;
                    if bits.len() <= word {
                        bits.resize(word + 1, 0);
                    }
                    let mask = 1u64 << (ordinal % 64);
                    let unique = bits[word] & mask == 0;
                    bits[word] |= mask;
                    unique
                }
                ParameterViewIds::Values(values) => {
                    #[cfg(test)]
                    DYNAMIC_PARAMETER_ID_HASHES.with(|n| n.set(n.get() + 1));
                    values.insert(instance.instance_id)
                }
            };
            #[cfg(test)]
            if view_ids.capacity() != old_capacity {
                DYNAMIC_PARAMETER_ID_GROWS.with(|n| n.set(n.get() + 1));
            }
            if !unique {
                return Err("geometry.duplicate_view_instance".into());
            }
            selected_count += 1;
        }
    }
    Ok(selected_count)
}
#[cfg(test)]
fn validate_parameter_identities_oracle(plan: &RhiCommandPlan) -> Result<usize, String> {
    let mut validation = crate::render_geometry::DrawViewValidation::default();
    // This call-local set proves membership only; no consumer observes key
    // order. Keep the full per-view identity and capacity check on every row
    // without constructing an ordered tree for the dynamic selection.
    // Metadata is a capacity hint, never admission: preserve the ordered full
    // batch/tail checks and the original per-row cross-batch identity checks.
    let mut capacities = BTreeMap::<u64, usize>::new();
    let mut references = 0usize;
    for payload in plan
        .commands
        .iter()
        .filter_map(RhiCommand::geometry_payload)
    {
        references = references.saturating_add(payload.geometry_instance_count());
        let view = parameter_view_identity(payload.geometry_draw().expect("geometry payload"));
        let count = capacities.entry(view).or_default();
        *count = count
            .saturating_add(payload.geometry_instance_count())
            .min(MAX_GEOMETRY_INSTANCES);
    }
    // Capacity hints do not admit rows. Bound the total reservation even for
    // malformed plans; the ordered row checks below keep the original errors.
    let mut remaining = MAX_GEOMETRY_INSTANCES;
    let mut ids: BTreeMap<_, _> = capacities
        .into_iter()
        .map(|(view, count)| {
            let reserve = count.min(remaining);
            remaining -= reserve;
            (view, std::collections::HashSet::with_capacity(reserve))
        })
        .collect();
    // Larger multiview selections still fit the unchanged parameter arena.
    // Pointer membership is conservative: detached equal backings may refuse,
    // but distinct parameter content can never bypass the original row bound.
    let mut instance_backings = (references > MAX_GEOMETRY_INSTANCES)
        .then(|| std::collections::HashSet::with_capacity(MAX_GEOMETRY_INSTANCES));
    let mut selected_count = 0;
    for command in &plan.commands {
        let Some(payload) = command.geometry_payload() else {
            continue;
        };
        if let RhiDrawPayload::GeometryBatch { batch } = payload {
            batch
                .validate_shared_with_view(&mut validation)
                .map_err(|e| e.to_string())?;
        }
        let view = parameter_view_identity(payload.geometry_draw().expect("geometry payload"));
        let view_ids = ids
            .get_mut(&view)
            .expect("metadata includes every geometry view");
        for (_, instance, _) in geometry_instances(payload) {
            if selected_count == MAX_GEOMETRY_REFERENCES {
                return Err("geometry.instance_capacity".into());
            }
            if let Some(backings) = &mut instance_backings {
                backings.insert(Arc::as_ptr(instance));
                if backings.len() > MAX_GEOMETRY_INSTANCES {
                    return Err("geometry.instance_capacity".into());
                }
            }
            #[cfg(test)]
            let old_capacity = view_ids.capacity();
            #[cfg(test)]
            DYNAMIC_PARAMETER_ID_HASHES.with(|n| n.set(n.get() + 1));
            let unique = view_ids.insert(instance.instance_id);
            #[cfg(test)]
            if view_ids.capacity() != old_capacity {
                DYNAMIC_PARAMETER_ID_GROWS.with(|n| n.set(n.get() + 1));
            }
            if !unique {
                return Err("geometry.duplicate_view_instance".into());
            }
            selected_count += 1;
        }
    }
    Ok(selected_count)
}
#[cfg(test)]
fn batch_compatible(a: &DrawGeometry, b: &DrawGeometry, pipeline: &str, format: &str) -> bool {
    batch_state_compatible(a, b) && b.pipeline_key(format).as_ref() == pipeline
}

fn batch_state_compatible(a: &DrawGeometry, b: &DrawGeometry) -> bool {
    if a.deformation.as_ref().map(Arc::as_ptr) != b.deformation.as_ref().map(Arc::as_ptr) {
        return false;
    }
    a.geometry_handle() == b.geometry_handle()
        && a.first_index == b.first_index
        && a.index_count == b.index_count
        && (a.material_binding == b.material_binding
            || (a.is_opaque_shadow_depth_writer() && b.is_opaque_shadow_depth_writer()))
        && a.view == b.view
        && a.lighting == b.lighting
        && a.environment == b.environment
        && a.target == b.target
        && a.depth_target == b.depth_target
}

fn draw_color_format(draw: &DrawGeometry, fallback: wgpu::TextureFormat) -> wgpu::TextureFormat {
    if draw.view.color_pipeline.is_some() && !draw.view.is_shadow_map() {
        wgpu::TextureFormat::Rgba16Float
    } else {
        fallback
    }
}

#[cfg(test)]
thread_local! {
    static DYNAMIC_PARAMETER_ID_GROWS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};
    static PARAMETER_VIEW_IDENTITIES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};
    static DYNAMIC_PARAMETER_ID_HASHES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};
}
#[cfg(test)]
mod dynamic_consumption_tests {
    use super::*;
    #[test]
    fn call_ordinals_preserve_strict_checker_for_dense_many_view_and_oversized_tables() {
        use crate::render_geometry::{batch_test_frame, batch_test_graph};
        for views in [2, 17] {
            for count in [4, 32, 256] {
                let frame = batch_test_frame(
                    true,
                    crate::render_contract::MaterialBlend::Opaque,
                    count,
                    views,
                );
                let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
                    &batch_test_graph(&frame),
                );
                let expected = validate_parameter_identities_oracle(&plan);
                assert_eq!(expected, Ok(count * views));
                assert_eq!(validate_parameter_identities(&plan), expected);
                let mut detached = plan.clone();
                for command in &mut detached.commands {
                    if let RhiCommand::Draw {
                        payload: RhiDrawPayload::GeometryBatch { batch },
                        ..
                    } = command
                    {
                        let batch = Arc::make_mut(batch);
                        let fill = batch.instances[0].clone();
                        Arc::make_mut(&mut batch.instances)
                            .resize(MAX_GEOMETRY_INSTANCES + 1, fill);
                        break;
                    }
                }
                assert_eq!(
                    validate_parameter_identities(&detached),
                    validate_parameter_identities_oracle(&detached),
                    "unselected oversized table changed selected-only admission"
                );
                if let Some(command) = detached
                    .commands
                    .iter()
                    .find(|c| c.geometry_payload().is_some())
                    .cloned()
                {
                    detached.commands.push(command);
                }
                assert_eq!(
                    validate_parameter_identities(&detached),
                    validate_parameter_identities_oracle(&detached),
                    "cross-table/detached duplicate escaped"
                );
            }
        }
    }
    #[test]
    #[ignore = "requires a real GPU"]
    fn dynamic_native_parallel_validation_rejects_late_duplicate_before_gpu_mutation() {
        let graph = crate::render_geometry::retained_command_test_graph();
        let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let draw = plan
            .commands
            .iter()
            .find(|c| c.geometry_payload().is_some())
            .unwrap()
            .clone();
        // Legal immutable heads, but one instance selected repeatedly in the
        // same view. The tail is invalid even though each batch is proven.
        for _ in 0..512 {
            plan.commands.push(draw.clone());
        }
        assert_eq!(
            validate_parameter_identities(&plan).unwrap_err(),
            "geometry.duplicate_view_instance"
        );
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let before = backend.retained_geometry_stats();
        let uploaded = backend.geometry.uploaded_meshes;
        assert_eq!(
            backend
                .prepare_geometry_plan_for_extent(&plan, [64, 64], None)
                .unwrap_err(),
            "geometry.duplicate_view_instance"
        );
        let after = backend.retained_geometry_stats();
        assert_eq!(uploaded, backend.geometry.uploaded_meshes);
        assert_eq!(before.bind_groups_created, after.bind_groups_created);
        assert_eq!(
            before.parameter_uploaded_bytes,
            after.parameter_uploaded_bytes
        );
        assert!(backend.geometry.validated_plan.is_none());
        assert!(backend.geometry.current_templates.is_empty());
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    #[test]
    fn execution_chain_native_identity_rejects_late_cross_batch_and_detached_table_conflicts() {
        use crate::render_geometry::{batch_test_frame, batch_test_graph};
        let frame = batch_test_frame(true, crate::render_contract::MaterialBlend::Opaque, 32, 2);
        let graph = batch_test_graph(&frame);
        let original = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert_eq!(
            validate_parameter_identities(&original).unwrap(),
            64,
            "different views must allow the same IDs"
        );
        for detached in [false, true] {
            let mut plan = original.clone();
            let mut extra = plan
                .commands
                .iter()
                .find(|c| c.geometry_payload().is_some())
                .unwrap()
                .clone();
            if detached {
                let RhiCommand::Draw {
                    payload: RhiDrawPayload::GeometryBatch { batch },
                    ..
                } = &mut extra
                else {
                    panic!("compact fixture")
                };
                let batch = Arc::make_mut(batch);
                batch.instances = Arc::new(batch.instances.as_ref().clone());
            }
            plan.commands.push(extra);
            assert_eq!(
                validate_parameter_identities(&plan).unwrap_err(),
                "geometry.duplicate_view_instance",
                "detached={detached}"
            );
        }
        let mut plan = original.clone();
        let last = plan
            .commands
            .iter_mut()
            .rev()
            .find_map(|c| match c {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::GeometryBatch { batch },
                    ..
                } => Some(batch),
                _ => None,
            })
            .unwrap();
        let batch = Arc::make_mut(last);
        let first = batch.visible[0].instance_index as usize;
        let tail = batch.visible.last().unwrap().instance_index as usize;
        let id = batch.instances[first].instance_id;
        Arc::make_mut(&mut Arc::make_mut(&mut batch.instances)[tail]).instance_id = id;
        assert_eq!(
            validate_parameter_identities(&plan).unwrap_err(),
            "geometry.duplicate_view_instance",
            "different indices with one ID escaped"
        );
    }
    #[test]
    fn execution_chain_native_identity_reads_view_once_per_batch_at_multiple_scales() {
        use crate::render_geometry::{batch_test_frame, batch_test_graph};
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let frame = batch_test_frame(
                true,
                crate::render_contract::MaterialBlend::Opaque,
                count,
                2,
            );
            let graph = batch_test_graph(&frame);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let batches = plan
                .commands
                .iter()
                .filter_map(RhiCommand::geometry_payload)
                .count();
            assert_eq!(
                validate_parameter_identities_oracle(&plan).unwrap(),
                count * 2
            );
            PARAMETER_VIEW_IDENTITIES.with(|n| n.set(0));
            DYNAMIC_PARAMETER_ID_HASHES.with(|n| n.set(0));
            assert_eq!(validate_parameter_identities(&plan).unwrap(), count * 2);
            assert_eq!(
                DYNAMIC_PARAMETER_ID_HASHES.with(|n| n.get()),
                count,
                "the same immutable table row repeated its ID hash across views at {count}"
            );
            actual.push((count, batches, PARAMETER_VIEW_IDENTITIES.with(|n| n.get())));
        }
        assert!(
            actual.iter().all(|&(_, batches, n)| n == 2 * batches),
            "row view identity work {actual:?}"
        );
    }
    #[test]
    #[ignore = "requires a real GPU"]
    fn dynamic_parameter_selection_reserves_without_rehash_at_multiple_scales() {
        let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
        let mut rows = Vec::new();
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            DYNAMIC_PARAMETER_ID_GROWS.with(|n| n.set(0));
            validate_parameter_plan(&plan, &backend.device).unwrap();
            rows.push((count, DYNAMIC_PARAMETER_ID_GROWS.with(|n| n.get())));
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        eprintln!("dynamic native actual ID HashSet capacity growth {rows:?}");
        assert!(
            rows.iter().all(|&(_, n)| n == 0),
            "native repeated rehash {rows:?}"
        );
    }
}

#[cfg(test)]
mod current_texture_consumption_tests {
    use super::*;
    use crate::wgpu_backend::real::{
        CURRENT_TEXTURE_CLOSURE_QUERIES, CURRENT_TEXTURE_DESCRIPTOR_VISITS,
    };
    #[test]
    #[ignore = "requires a real GPU"]
    fn current_texture_closure_real_prepare_consumes_current_handles_without_descriptor_rescan() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let base = graph
                .passes
                .iter()
                .flat_map(|p| &p.commands)
                .find_map(|c| c.geometry_draw())
                .unwrap()
                .material_binding
                .clone();
            let handle = RenderResourceHandle {
                kind: crate::render_resource::RenderResourceKind::Texture,
                index: 404,
                generation: 7,
            };
            let mut material = base.as_ref().clone();
            material
                .slots
                .push(crate::render_contract::NamedRenderBinding::new(
                    "baseColorTexture",
                    handle,
                ));
            let material = Arc::new(material);
            for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                    Arc::make_mut(draw).material_binding = material.clone();
                }
            }
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            let uploaded = backend.geometry.uploaded_meshes;
            let mut current = plan.clone();
            for command in &mut current.commands {
                match command {
                    RhiCommand::BeginView { view, .. } => {
                        Arc::make_mut(view).view_matrix[12] = 0.25f32.into();
                    }
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => {
                        Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] =
                            0.25f32.into();
                    }
                    _ => {}
                }
            }
            drop(plan);
            drop(graph);
            CURRENT_TEXTURE_DESCRIPTOR_VISITS.with(|n| n.set(0));
            CURRENT_TEXTURE_CLOSURE_QUERIES.with(|n| n.set(0));
            backend
                .prepare_geometry_plan_for_extent(&current, [640, 480], None)
                .unwrap();
            observed.push((
                count,
                CURRENT_TEXTURE_DESCRIPTOR_VISITS.with(|n| n.get()),
                CURRENT_TEXTURE_CLOSURE_QUERIES.with(|n| n.get()),
            ));
            assert_eq!(backend.geometry.uploaded_meshes, uploaded);
            // Completed readback above; removing a resident object is an explicit
            // fault seam. A positive CPU requirement can never admit a missing GPU row.
            let texture = backend.textures.remove(&handle).unwrap();
            let expected = backend
                .validate_plan_texture_residency(&current)
                .unwrap_err();
            assert_eq!(
                backend
                    .prepare_geometry_plan_for_extent(&current, [640, 480], None)
                    .unwrap_err(),
                expected
            );
            assert_eq!(backend.geometry.uploaded_meshes, uploaded);
            backend.textures.insert(handle, texture);
            backend
                .prepare_geometry_plan_for_extent(&current, [640, 480], None)
                .unwrap();
            let mut generation = current.clone();
            let mut material = base.as_ref().clone();
            material
                .slots
                .push(crate::render_contract::NamedRenderBinding::new(
                    "baseColorTexture",
                    RenderResourceHandle {
                        generation: 8,
                        ..handle
                    },
                ));
            let material = Arc::new(material);
            for command in &mut generation.commands {
                if let RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } = command
                {
                    Arc::make_mut(draw).material_binding = material.clone();
                }
            }
            let expected = backend
                .validate_plan_texture_residency(&generation)
                .unwrap_err();
            assert_eq!(
                backend
                    .prepare_geometry_plan_for_extent(&generation, [640, 480], None)
                    .unwrap_err(),
                expected
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!("actual native descriptor scans/current registry queries {observed:?}");
        assert!(
            observed
                .iter()
                .all(|(_, visits, queries)| *visits == 0 && *queries == 1),
            "current closure reinterpreted: {observed:?}"
        );
    }
}

#[cfg(test)]
mod prepared_intent_consumption_tests {
    use super::*;
    #[test]
    #[ignore = "requires a real GPU"]
    fn prepared_intent_surface_readiness_consumes_owner_closure_and_keeps_texture_first_errors() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let mut binding = graph
                .passes
                .iter()
                .flat_map(|p| &p.commands)
                .find_map(|c| c.geometry_draw())
                .unwrap()
                .material_binding
                .as_ref()
                .clone();
            let handle = RenderResourceHandle {
                kind: crate::render_resource::RenderResourceKind::Texture,
                index: 406,
                generation: 7,
            };
            binding
                .slots
                .push(crate::render_contract::NamedRenderBinding::new(
                    "baseColorTexture",
                    handle,
                ));
            let binding = Arc::new(binding);
            for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = command {
                    Arc::make_mut(draw).material_binding = binding.clone();
                }
            }
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            let mut current = plan.clone();
            for command in &mut current.commands {
                match command {
                    RhiCommand::BeginView { view, .. } => {
                        Arc::make_mut(view).view_matrix[12] = 0.25f32.into()
                    }
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => {
                        Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] =
                            0.25f32.into()
                    }
                    _ => {}
                }
            }
            super::super::CURRENT_TEXTURE_DESCRIPTOR_VISITS.with(|n| n.set(0));
            assert!(backend
                .prepare_surface_resources(&current, [640, 480], None)
                .unwrap());
            observed.push((
                count,
                super::super::CURRENT_TEXTURE_DESCRIPTOR_VISITS.with(|n| n.get()),
            ));
            let texture = backend.textures.remove(&handle).unwrap();
            let mut invalid = current.clone();
            let draw = invalid
                .commands
                .iter_mut()
                .find_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .unwrap();
            Arc::make_mut(draw).world_transform[0] = f32::NAN.into();
            let expected = backend
                .validate_plan_texture_residency(&invalid)
                .unwrap_err();
            assert_eq!(
                backend
                    .prepare_surface_resources(&invalid, [640, 480], None)
                    .unwrap_err(),
                expected,
                "texture admission precedes the invalid geometry error"
            );
            backend.textures.insert(handle, texture);
            assert!(backend
                .prepare_surface_resources(&current, [640, 480], None)
                .unwrap());
            let mut mixed = current.clone();
            mixed.commands.push(RhiCommand::Draw {
                target: current
                    .commands
                    .iter()
                    .find_map(geometry_draw)
                    .unwrap()
                    .target
                    .as_ref()
                    .into(),
                draw_kind: RhiDrawKind::SpriteTextured,
                vertex_count: 0,
                payload: RhiDrawPayload::SpriteTextured {
                    sprite_ref: "mixed".into(),
                    material_ref: None,
                    sort_key: "0".into(),
                    texture: Some(RenderResourceHandle {
                        index: 407,
                        ..handle
                    }),
                    binding: None,
                    fallback_used: false,
                    pipeline_key: "sprite".into(),
                    vertices: vec![],
                },
            });
            let expected = backend.validate_plan_texture_residency(&mixed).unwrap_err();
            assert_eq!(
                backend
                    .prepare_surface_resources(&mixed, [640, 480], None)
                    .unwrap_err(),
                expected,
                "mixed commands retain their original texture consumer"
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!("actual surface readiness descriptor visits {observed:?}");
        assert!(
            observed.iter().all(|(_, n)| *n == 0),
            "surface repeats owner requirements: {observed:?}"
        );
    }
    #[test]
    #[ignore = "requires a real GPU"]
    fn prepared_intent_real_consumer_preserves_recording_without_rebuilding_pipeline_strings() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            assert!(
                !backend.reuse_geometry_intent(&plan),
                "preparation is not successful recording"
            );
            let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("prepared CPU intent consumer"),
                size: wgpu::Extent3d {
                    width: 640,
                    height: 480,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let mut reference = crate::wgpu_backend::WgpuBackend::new_unavailable();
            reference.device_context.real_wgpu_enabled = true;
            reference.target_context.format = "Rgba8Unorm".into();
            for draw in plan.commands.iter().filter_map(geometry_draw) {
                reference.record_draw(&RhiDrawPayload::Geometry {
                    draw: Arc::new(draw.clone()),
                });
            }
            crate::wgpu_backend::GEOMETRY_INTENT_PIPELINE_BUILDS.with(|n| n.set(0));
            let first = backend.execute_plan_to_surface_view(&plan, &view);
            assert!(first.diagnostics.is_empty(), "{first:?}");
            assert_eq!(first.draw_count, count);
            assert_eq!(first.binding_count, 2 * count);
            assert_eq!(
                backend.state.pipeline_cache_report(),
                reference.pipeline_cache_report()
            );
            assert_eq!(
                backend.state.resource_registry_report(),
                reference.resource_registry_report()
            );
            let first_builds =
                crate::wgpu_backend::GEOMETRY_INTENT_PIPELINE_BUILDS.with(|n| n.get());
            assert!(backend.reuse_geometry_intent(&plan));
            let mut current = plan.clone();
            for command in &mut current.commands {
                match command {
                    RhiCommand::BeginView { view, .. } => {
                        Arc::make_mut(view).view_matrix[12] = 0.25f32.into()
                    }
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => {
                        Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] =
                            0.25f32.into()
                    }
                    _ => {}
                }
            }
            backend
                .prepare_geometry_plan_for_extent(&current, [640, 480], None)
                .unwrap();
            assert!(!backend.reuse_geometry_intent(&current));
            crate::wgpu_backend::GEOMETRY_INTENT_PIPELINE_BUILDS.with(|n| n.set(0));
            let second = backend.execute_plan_to_surface_view(&current, &view);
            assert!(second.diagnostics.is_empty(), "{second:?}");
            assert_eq!(second.draw_count, 2 * count);
            assert_eq!(second.binding_count, 4 * count);
            assert_eq!(
                backend.state.pipeline_cache_report(),
                reference.pipeline_cache_report()
            );
            assert_eq!(
                backend.state.resource_registry_report(),
                reference.resource_registry_report()
            );
            let moving_builds =
                crate::wgpu_backend::GEOMETRY_INTENT_PIPELINE_BUILDS.with(|n| n.get());
            // New source allocation must resolve the complete changed pipeline context.
            let mut changed = current.clone();
            let first_draw = changed
                .commands
                .iter_mut()
                .find_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .unwrap();
            Arc::make_mut(first_draw).cull = crate::render_contract::MaterialCull::Front;
            let expected = first_draw.pipeline_key("Rgba8Unorm");
            backend
                .prepare_geometry_plan_for_extent(&changed, [640, 480], None)
                .unwrap();
            let third = backend.execute_plan_to_surface_view(&changed, &view);
            assert!(third.diagnostics.is_empty(), "{third:?}");
            assert!(backend.state.pipeline_keys.contains(expected.as_ref()));
            // Failed current admission cannot publish pure facts through this consumer.
            let mut invalid = changed.clone();
            let first_draw = invalid
                .commands
                .iter_mut()
                .find_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .unwrap();
            Arc::make_mut(first_draw).world_transform[0] = f32::NAN.into();
            assert!(backend
                .prepare_geometry_plan_for_extent(&invalid, [640, 480], None)
                .is_err());
            crate::wgpu_backend::GEOMETRY_INTENT_PIPELINE_BUILDS.with(|n| n.set(0));
            let rejected = backend.execute_plan_to_surface_view(&invalid, &view);
            assert!(!rejected.diagnostics.is_empty());
            assert_eq!(
                crate::wgpu_backend::GEOMETRY_INTENT_PIPELINE_BUILDS.with(|n| n.get()),
                count as u64
            );
            observed.push((count, first_builds, moving_builds));
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!("actual prepared/camera-only intent pipeline constructions {observed:?}");
        assert!(
            observed.iter().all(|(_, a, b)| *a == 0 && *b == 0),
            "prepared CPU fact reinterpreted: {observed:?}"
        );
    }
}

#[cfg(test)]
mod ordered_fact_tests {
    use super::*;
    #[test]
    #[ignore = "requires a real GPU"]
    fn flow_ordered_cpu_facts_are_consumed_without_downstream_resolve() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            // Different live command backing and view still require one current prepare.
            for command in &mut plan.commands {
                match command {
                    RhiCommand::BeginView { view, .. } => {
                        Arc::make_mut(view).view_matrix[12] = 0.5f32.into()
                    }
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => {
                        Arc::make_mut(&mut Arc::make_mut(draw).view).view_matrix[12] = 0.5f32.into()
                    }
                    _ => {}
                }
            }
            preparation::QUERIES.with(|n| n.set(0));
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            assert!(backend.validated_geometry_only_commands(&plan));
            for (index, command) in plan.commands.iter().enumerate() {
                if let Some(draw) = geometry_draw(command) {
                    assert_eq!(
                        backend.prepared_geometry_intent_pipeline(&plan.commands, index, draw),
                        draw.pipeline_key("Rgba8Unorm")
                    );
                }
            }
            let calls = preparation::QUERIES.with(|n| n.get());
            actual.push((count, calls));
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }
        eprintln!("flow ordered facts actual resolve calls {actual:?}");
        assert!(
            actual.iter().all(|(count, calls)| calls <= count),
            "{actual:?}"
        );
    }
}

#[cfg(test)]
mod ordered_fact_range_tests {
    use super::*;
    #[test]
    fn flow_ordered_fact_ranges_reject_detached_cow_expired_and_format_mismatch() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(4);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let mut cached = ValidatedGeometryPlan::capture(
            &plan,
            [640, 480],
            None,
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .unwrap();
        let memo = preparation::Preparation::default();
        cached.requirements = plan
            .commands
            .iter()
            .map(|c| geometry_draw(c).map(|d| memo.resolve(d, wgpu::TextureFormat::Rgba8Unorm)))
            .collect();
        for start in 0..plan.commands.len() {
            for end in start + 1..=plan.commands.len() {
                let facts = cached
                    .requirements_for(&plan.commands[start..end], wgpu::TextureFormat::Rgba8Unorm)
                    .unwrap();
                assert_eq!(facts.len(), end - start);
                for (fact, command) in facts.iter().zip(&plan.commands[start..end]) {
                    assert_eq!(fact.is_some(), geometry_draw(command).is_some());
                    if let (Some(fact), Some(draw)) = (fact, geometry_draw(command)) {
                        assert_eq!(fact.pipeline, draw.pipeline_key("Rgba8Unorm"));
                    }
                }
            }
        }
        assert!(cached
            .requirements_for(&plan.commands, wgpu::TextureFormat::Rgba16Float)
            .is_none());
        let detached: Vec<_> = plan.commands.iter().cloned().collect();
        assert!(cached
            .requirements_for(&detached, wgpu::TextureFormat::Rgba8Unorm)
            .is_none());
        let mut cow = plan.clone();
        cow.commands[0] = detached[0].clone();
        assert!(cached
            .requirements_for(&cow.commands, wgpu::TextureFormat::Rgba8Unorm)
            .is_none());
        drop(cow);
        let lease = Arc::downgrade(
            &geometry_draw(
                plan.commands
                    .iter()
                    .find(|c| geometry_draw(c).is_some())
                    .unwrap(),
            )
            .unwrap()
            .resource_lease,
        );
        drop(plan);
        drop(graph);
        drop(detached);
        assert!(lease.upgrade().is_none());
        assert!(cached.command_backing.upgrade().is_none());
    }
}

#[cfg(test)]
mod fact_observation_tests {
    use super::*;
    #[test]
    #[ignore = "requires a real GPU"]
    fn flow_fact_observation_emits_shared_backing_once_and_keeps_plan_only_facts() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            for hdr in [false, true] {
                let graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
                let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
                let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
                // Observation-only fixture: no GPU admission or encoding is claimed.
                let mut cached = ValidatedGeometryPlan::capture(
                    &plan,
                    [640, 480],
                    None,
                    wgpu::TextureFormat::Rgba8Unorm,
                )
                .unwrap();
                cached.requirements = plan
                    .commands
                    .iter()
                    .map(|c| {
                        geometry_draw(c).map(|d| {
                            backend.geometry.preparation.resolve(
                                d,
                                if hdr {
                                    wgpu::TextureFormat::Rgba16Float
                                } else {
                                    wgpu::TextureFormat::Rgba8Unorm
                                },
                            )
                        })
                    })
                    .collect();
                backend.geometry.validated_plan = Some(cached);
                for mode in [0, 1] {
                    if mode == 1 {
                        backend.geometry.preparation = preparation::Preparation::default();
                    }
                    let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
                    let mut gpu = crate::wgpu_backend::real::memory::GpuMemoryCensus::default();
                    backend.geometry.observe_memory(&mut gpu, &mut sample);
                    drop(gpu);
                    let renderer: Vec<_> = sample
                        .allocations
                        .into_iter()
                        .filter(|a| {
                            a.domain == crate::resource_memory::MemoryDomain::Cpu
                                && a.owner == "renderer"
                        })
                        .collect();
                    let total = crate::resource_memory::memory_totals(&renderer).unwrap();
                    actual.push((
                        count,
                        hdr,
                        mode,
                        renderer.len(),
                        total.unique_allocations,
                        // All draws share an interned pipeline string; HDR has
                        // one additional distinct intent string. Both ordered
                        // facts, native-template references, source closure and
                        // moving-view skeleton have real backing, even with
                        // native None rows.
                        count + if hdr { 6 } else { 5 },
                    ));
                }
                backend
                    .shutdown_gpu(std::time::Duration::from_secs(5))
                    .unwrap();
            }
        }
        eprintln!("flow actual emitted/unique CPU fact allocations {actual:?}");
        assert!(actual.iter().all(|(_,_,_,emitted,unique,expected)|emitted==unique && unique==expected),"{actual:?}");
    }
}

#[cfg(test)]
mod single_table_identity_tests {
    use super::*;
    #[test]
    fn shared_native_table_consumption_reuses_full_unique_domain_without_rehashing_ids() {
        for count in [4, 32, 256] {
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
                &crate::render_geometry::unique_table_test_graph(count),
            );
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let expected = validate_parameter_identities_oracle(&plan).unwrap();
            DYNAMIC_PARAMETER_ID_HASHES.with(|n| n.set(0));
            assert_eq!(validate_parameter_identities(&plan).unwrap(), expected);
            let hashes = DYNAMIC_PARAMETER_ID_HASHES.with(|n| n.get());
            eprintln!(
                "shared native domain scale={count} selected={expected} actual ID hashes={hashes}"
            );
            assert!(expected >= count);
            assert_eq!(
                hashes, 0,
                "native rebuilt an already complete producer ID domain"
            );
        }
    }
    #[test]
    fn shared_native_domain_keeps_cross_batch_cow_mixed_and_wire_first_errors() {
        let original = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::render_geometry::unique_table_test_graph(32),
        );
        let index = original
            .commands
            .iter()
            .position(|c| {
                matches!(
                    c,
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::GeometryBatch { .. },
                        ..
                    }
                )
            })
            .unwrap();
        for change in 0..6 {
            let mut plan = original.clone();
            if change == 0 {
                let duplicate = plan.commands[index].clone();
                plan.commands.insert(index + 1, duplicate);
            } else {
                let RhiCommand::Draw {
                    payload: RhiDrawPayload::GeometryBatch { batch },
                    ..
                } = &mut plan.commands[index]
                else {
                    unreachable!()
                };
                let batch = Arc::make_mut(batch);
                match change {
                    1 => {
                        let rows = Arc::make_mut(&mut batch.instances);
                        let first_id = rows[0].instance_id;
                        Arc::make_mut(&mut rows[31]).instance_id = first_id;
                    }
                    2 => {
                        batch.visible.last_mut().unwrap().instance_index = u32::MAX;
                    }
                    3 => {
                        batch.instances = Arc::new(batch.instances.as_ref().clone());
                    }
                    4 => {
                        *batch =
                            serde_json::from_value(serde_json::to_value(&*batch).unwrap()).unwrap();
                    }
                    _ => (),
                }
            }
            if change == 5 {
                let RhiCommand::Draw {
                    payload: RhiDrawPayload::GeometryBatch { batch },
                    target,
                    draw_kind,
                    vertex_count,
                } = &plan.commands[index]
                else {
                    unreachable!()
                };
                let detached = RhiCommand::Draw {
                    target: target.clone(),
                    draw_kind: *draw_kind,
                    vertex_count: *vertex_count,
                    payload: RhiDrawPayload::Geometry {
                        draw: batch.draw.clone(),
                    },
                };
                plan.commands.insert(index + 1, detached);
            }
            assert_eq!(
                validate_parameter_identities(&plan),
                validate_parameter_identities_oracle(&plan),
                "first error changed for variant {change}"
            );
        }
        assert!(
            validate_parameter_identities(&original).is_ok(),
            "COW changed the old packet"
        );
    }
}

#[cfg(test)]
mod measurement_source_tests {
    use super::*;

    #[test]
    fn direct_measurement_sources_preserve_selected_rows_and_every_split_boundary() {
        // Independent original accessor path is the semantic oracle. This is
        // equivalence coverage; the measured Release scan is the cost red.
        let oracle = |command: &RhiCommand| {
            let draw = command.geometry_draw().unwrap();
            let spine = draw.provenance.source_component.as_deref() == Some("SpineAnimator");
            if let Some(RhiDrawPayload::GeometryBatch { batch }) = command.geometry_payload() {
                if batch.visible.iter().any(|row| {
                    (batch.instances[row.instance_index as usize]
                        .provenance
                        .source_component
                        .as_deref()
                        == Some("SpineAnimator"))
                        != spine
                }) {
                    return None;
                }
            }
            Some(spine)
        };
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &crate::rhi_command_plan::tests::retained_geometry_graph(4),
        );
        let source = plan
            .commands
            .iter()
            .find_map(RhiCommand::geometry_draw)
            .unwrap();
        let mut commands = Vec::new();
        for component in [
            None,
            Some("SpineAnimator"),
            Some("MeshRenderer"),
            Some("SpineAnimatorExtra"),
        ] {
            let mut draw = source.as_ref().clone();
            Arc::make_mut(&mut draw.provenance).source_component = component.map(str::to_owned);
            let draw = Arc::new(draw);
            commands.push(RhiCommand::Draw {
                target: draw.target.clone(),
                draw_kind: crate::rhi_command_plan::RhiDrawKind::Geometry,
                vertex_count: draw.index_count,
                payload: RhiDrawPayload::Geometry { draw: draw.clone() },
            });
            let instances: Vec<_> = [None, Some("SpineAnimator"), Some("MeshRenderer")]
                .into_iter()
                .enumerate()
                .map(|(i, c)| {
                    let mut row = draw.native_instance().as_ref().clone();
                    row.instance_id += i as u64 + 1;
                    Arc::make_mut(&mut row.provenance).source_component = c.map(str::to_owned);
                    Arc::new(row)
                })
                .collect();
            for selected in [
                vec![],
                vec![0],
                vec![1],
                vec![2],
                vec![0, 1],
                vec![1, 2],
                vec![2, 0, 1],
            ] {
                let mut batch_head = draw.as_ref().clone();
                if let Some(&first) = selected.first() {
                    *batch_head = instances[first as usize].as_ref().clone();
                }
                let visible = selected
                    .into_iter()
                    .map(|index| crate::render_geometry::GeometryVisibleInstance {
                        instance_index: index,
                        camera_depth: draw.camera_depth,
                    })
                    .collect();
                let batch = crate::render_geometry::GeometryBatch::new(
                    Arc::new(batch_head),
                    Arc::new(instances.clone()),
                    visible,
                );
                if batch.visible.is_empty() {
                    assert!(batch.validate().is_err(), "empty selection stays invalid");
                } else {
                    batch.validate().unwrap();
                }
                commands.push(RhiCommand::Draw {
                    target: draw.target.clone(),
                    draw_kind: crate::rhi_command_plan::RhiDrawKind::Geometry,
                    vertex_count: draw.index_count,
                    payload: RhiDrawPayload::GeometryBatch {
                        batch: Arc::new(batch),
                    },
                });
            }
        }
        for (i, c) in commands.iter().enumerate() {
            assert_eq!(measurement_draw_source(c), oracle(c), "source {i}");
        }
        for start in 0..commands.len() {
            for end in start + 1..=commands.len() {
                let source = oracle(&commands[start]);
                let expected = if source.is_none() {
                    start + 1
                } else {
                    (start + 1..end)
                        .find(|&i| oracle(&commands[i]) != source)
                        .unwrap_or(end)
                };
                assert_eq!(
                    measurement_draw_end(&commands, start, end),
                    expected,
                    "range {start}..{end}"
                );
            }
        }
    }
}
