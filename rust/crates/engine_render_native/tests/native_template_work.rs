#![cfg(feature = "real-wgpu")]
extern crate engine_runtime_core as engine_runtime;

#[path = "../../engine_runtime/tests/support/geometry_fixture.rs"]
mod geometry_fixture;

use engine_render_native::wgpu_backend::real::RealWgpuBackend;
use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_resource::*;

#[test]
#[ignore = "requires a real GPU; shared instance records across views"]
fn shared_instance_parameters_allow_more_view_references_than_parameter_rows() {
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    use std::sync::Arc;

    let mut manager = RenderResourceManager::new();
    let source = geometry_fixture::source(&mut manager);
    let mut frame = geometry_fixture::frame(&source, &manager);
    let head = frame.views[0].draws[0].clone();
    let count = 32_769;
    let table = Arc::new(
        (0..count)
            .map(|index| {
                let mut instance = head.native_instance().as_ref().clone();
                instance.instance_id += index as u64;
                Arc::new(instance)
            })
            .collect::<Vec<_>>(),
    );
    let base = frame.views[0].clone();
    frame.views.clear();
    for index in 0..2 {
        let mut view = base.clone();
        view.view.view_id = index + 1;
        view.depth_target = Some(format!("shared-parameter-depth-{index}"));
        let mut draw = head.as_ref().clone();
        draw.view = Arc::new(view.view.clone());
        draw.depth_target = view.depth_target.clone().map(Into::into);
        let batch = GeometryBatch::new(
            Arc::new(draw),
            table.clone(),
            (0..count)
                .map(|instance_index| GeometryVisibleInstance {
                    instance_index,
                    camera_depth: head.camera_depth,
                })
                .collect(),
        );
        batch.validate().unwrap();
        view.draws = vec![batch.draw.clone()];
        view.batches = Some(vec![Arc::new(batch)]);
        frame.views.push(view);
    }
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &geometry_fixture::graph(&frame),
    );
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert_eq!(
        backend.retained_geometry_stats().instances,
        u64::from(count) * 2
    );
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] > 200));

    let mut duplicate = plan.clone();
    duplicate.commands.push(
        plan.commands
            .iter()
            .find(|command| command.geometry_payload().is_some())
            .unwrap()
            .clone(),
    );
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&duplicate, 64, 64)
            .unwrap_err(),
        "geometry.duplicate_view_instance"
    );

    let mut distinct = plan.clone();
    let last = distinct
        .commands
        .iter_mut()
        .rev()
        .find_map(|command| match command {
            RhiCommand::Draw {
                payload: RhiDrawPayload::GeometryBatch { batch },
                ..
            } => Some(batch),
            _ => None,
        })
        .unwrap();
    let batch = Arc::make_mut(last);
    for instance in Arc::make_mut(&mut batch.instances) {
        Arc::make_mut(instance).instance_id += u64::from(count);
    }
    Arc::make_mut(&mut batch.draw).instance_id += u64::from(count);
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&distinct, 64, 64)
            .unwrap_err(),
        "geometry.instance_capacity"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU; mixed-frame upload admission"]
fn surface_preparation_keeps_particle_steps_pending_when_geometry_uses_the_allowance() {
    use engine_render_native::wgpu_backend::real::GeometryUploadBudget;
    use engine_runtime::particle_effect::{program, *};
    use engine_runtime::particle_render_contract::ParticleRenderStep;
    use engine_runtime::rhi_command_plan::RhiCommand;
    use std::time::Duration;

    let mut description: ParticleEffectDescription =
        serde_json::from_str(include_str!("fixtures/particle-effect.json")).unwrap();
    description.emitters[0].capacity = 4;
    description.emitters[0].bursts[0].count = 1;
    description.validate().unwrap();
    let programs = description
        .emitters
        .iter()
        .map(|emitter| CookedParticleProgram {
            emitter: emitter.name.clone(),
            wgsl: program::compute_source(
                &description,
                emitter,
                &(program::prefix(&description, emitter)
                    + program::DEFAULT_BEHAVIOR
                    + program::CONTRACT_CALLS),
            ),
            author_path: None,
            author_first_line: 0,
            author_line_count: 0,
            particle_stride: program::particle_layout(emitter).2,
        })
        .collect();
    let effect = CookedParticleEffect {
        description,
        program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
        programs,
        source_digests: Default::default(),
    };
    let mut manager = RenderResourceManager::new();
    let source = geometry_fixture::source(&mut manager);
    let frame = geometry_fixture::frame(&source, &manager);
    let mut plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &geometry_fixture::graph(&frame),
    );
    let mut commands = plan.commands.to_vec();
    commands.insert(
        1,
        RhiCommand::SimulateParticles {
            instance: 7,
            step: ParticleRenderStep {
                emitter_affine: Default::default(),
                step_id: 1,
                delta_seconds: 0.015625.into(),
                origin: [0f32.into(); 3],
                emitting: true,
                paused: false,
            },
        },
    );
    plan.commands = commands.into();
    let budget = |bytes_per_frame| GeometryUploadBudget {
        resident_bytes: 128 * 1024 * 1024,
        staging_bytes: 1024 * 1024,
        bytes_per_frame,
        max_tickets: 8,
    };
    // Measure only the actual geometry upload owner, then leave 143 bytes for
    // the real 144-byte particle step. No product limit is changed by this test.
    let mut measure = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    measure.install_particle_effect(7, &effect).unwrap();
    measure
        .set_geometry_upload_budget(budget(1024 * 1024))
        .unwrap();
    measure.bind_geometry_upload_presentation(1).unwrap();
    assert!(measure
        .prepare_surface_resources(&plan, [64, 64], None)
        .unwrap());
    let geometry_bytes = measure.geometry_upload_snapshot().frame_uploaded_bytes;
    measure.shutdown_gpu(Duration::from_secs(5)).unwrap();

    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    backend.install_particle_effect(7, &effect).unwrap();
    backend
        .set_geometry_upload_budget(budget(geometry_bytes + 143))
        .unwrap();
    backend.bind_geometry_upload_presentation(1).unwrap();
    let before = backend.capture_particle_state_for_validation(7).unwrap();
    let ready = backend
        .prepare_surface_resources(&plan, [64, 64], None)
        .unwrap();
    assert!(!ready, "geometry preparation incorrectly certified the mixed frame with only 143 bytes left for a 144-byte particle step");
    assert_eq!(
        format!("{before:?}"),
        format!(
            "{:?}",
            backend.capture_particle_state_for_validation(7).unwrap()
        ),
        "loading must not advance particle simulation"
    );
    let clear = engine_runtime::rhi_command_plan::RhiCommandPlan {
        frame_index: 0,
        commands: vec![RhiCommand::Clear {
            target: "main".into(),
            color: [0f32.into(); 4],
        }]
        .into(),
        ..plan.clone()
    };
    backend.render_plan_to_rgba_bytes(&clear, 64, 64).unwrap();
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
    backend.bind_geometry_upload_presentation(2).unwrap();
    assert!(backend
        .prepare_surface_resources(&plan, [64, 64], None)
        .unwrap());
    let (_, draw) = backend
        .render_plan_to_rgba_bytes_with_receipt(&plan, 64, 64, None)
        .unwrap();
    assert_eq!(draw.frame_index, plan.frame_index);
    let after = backend.capture_particle_state_for_validation(7).unwrap();
    assert_ne!(
        format!("{before:?}"),
        format!("{after:?}"),
        "the resumed frame must execute the real particle step"
    );
    let uploaded = backend.geometry_upload_snapshot().uploaded_bytes;
    backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert_eq!(
        backend.geometry_upload_snapshot().uploaded_bytes,
        uploaded,
        "replaying the same step must not charge or execute it twice"
    );
    assert_eq!(
        format!("{after:?}"),
        format!(
            "{:?}",
            backend.capture_particle_state_for_validation(7).unwrap()
        )
    );
    assert!(backend.geometry_upload_snapshot().frame_uploaded_bytes <= geometry_bytes + 143);
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert_eq!(backend.geometry_upload_snapshot().staging_bytes, 0);
}

#[test]
#[ignore = "requires a real GPU"]
fn retained_instance_count_counts_batch_members_on_direct_and_indirect_paths() {
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    use std::sync::Arc;
    let mut observations = Vec::new();
    for (groups, gpu_scale) in [(1, false), (32, true)] {
        let mut manager = RenderResourceManager::new();
        let mesh = geometry_fixture::prepare(
            "batch-count-mesh",
            RenderAssetData::Mesh(geometry_fixture::triangle()),
            &mut manager,
        );
        let materials: Vec<_> = (0..groups)
            .map(|i| {
                geometry_fixture::prepare(
                    &format!("batch-count-material-{i}"),
                    RenderAssetData::Material(geometry_fixture::unlit([0.2, 0.4, 0.6, 1.])),
                    &mut manager,
                )
            })
            .collect();
        let sources: Vec<_> = materials
            .iter()
            .map(|m| PreparedGeometry::from_assets(&mesh, m, 0, &manager).unwrap())
            .collect();
        let instances: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, s)| geometry_fixture::instance(s, i as u64 * 3 + 1, [0., 0., -2.]))
            .collect();
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        let frame = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: Default::default(),
            }],
            &instances,
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        let mut plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &geometry_fixture::graph(&frame),
        );
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        plan.commands = plan
            .commands
            .iter()
            .map(|command| {
                let Some(head) = command.geometry_draw() else {
                    return command.clone();
                };
                let mut table = vec![head.native_instance().clone()];
                for offset in 1..3 {
                    let mut member = head.native_instance().as_ref().clone();
                    member.instance_id += offset;
                    table.push(Arc::new(member));
                }
                let batch = GeometryBatch::new(
                    head.clone(),
                    Arc::new(table),
                    (0..3)
                        .map(|instance_index| GeometryVisibleInstance {
                            instance_index,
                            camera_depth: head.camera_depth,
                        })
                        .collect(),
                );
                batch.validate().unwrap();
                RhiCommand::Draw {
                    target: head.target.clone(),
                    draw_kind: engine_runtime::rhi_command_plan::RhiDrawKind::Geometry,
                    vertex_count: head.index_count,
                    payload: RhiDrawPayload::GeometryBatch {
                        batch: Arc::new(batch),
                    },
                }
            })
            .collect::<Vec<_>>()
            .into();
        assert_eq!(
            plan.commands
                .iter()
                .map(RhiCommand::geometry_instance_count)
                .sum::<usize>(),
            groups * 3
        );
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend.set_geometry_gpu_scale_enabled(gpu_scale);
        for _ in 0..2 {
            let before = backend.retained_geometry_stats().instances;
            backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
            observations.push((groups, backend.retained_geometry_stats().instances - before));
        }
        let paths = backend.geometry_gpu_scale_counts();
        if gpu_scale {
            assert!(
                paths[0] > 0 && paths[1] > 0,
                "missing query/indirect path: {paths:?}"
            );
        } else {
            assert_eq!((paths[0], paths[1]), (0, 0));
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    assert!(
        observations
            .iter()
            .all(|(groups, count)| *count == (*groups * 3) as u64),
        "submitted batch members were counted as commands: {observations:?}"
    );
}

#[test]
#[ignore = "requires a real GPU"]
fn moving_view_keeps_warm_template_memo_unchanged() {
    use engine_runtime::render_asset_production::*;
    let mut observed = Vec::new();
    for count in [4, 32, 256] {
        let mut manager = RenderResourceManager::new();
        let mesh = geometry_fixture::prepare(
            "shared-work-mesh",
            RenderAssetData::Mesh(geometry_fixture::triangle()),
            &mut manager,
        );
        let materials: Vec<_> = (0..count)
            .map(|i| {
                geometry_fixture::prepare(
                    &format!("work-material-{i}"),
                    RenderAssetData::Material(geometry_fixture::unlit([
                        i as f32 / count as f32,
                        0.25,
                        0.5,
                        1.,
                    ])),
                    &mut manager,
                )
            })
            .collect();
        let sources: Vec<_> = materials
            .iter()
            .map(|material| PreparedGeometry::from_assets(&mesh, material, 0, &manager).unwrap())
            .collect();
        let instances: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, source)| geometry_fixture::instance(source, i as u64 + 1, [0., 0., -2.]))
            .collect();
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        let mut frame = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: Default::default(),
            }],
            &instances,
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        assert_eq!(frame.views[0].draws.len(), count);
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
            &geometry_fixture::graph(&frame),
        );
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        let before = backend.retained_geometry_stats();
        let mut moved = frame.views[0].view.clone();
        moved.view_matrix[12] = 0.01f32.into();
        let moved = std::sync::Arc::new(moved);
        frame.views[0].view = moved.as_ref().clone();
        for draw in &mut frame.views[0].draws {
            std::sync::Arc::make_mut(draw).view = moved.clone();
        }
        assert!(
            frame.views[0].batches.is_none(),
            "fixture must keep independent material heads"
        );
        let graph = geometry_fixture::graph(&frame);
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        let after = backend.retained_geometry_stats();
        eprintln!(
            "warm native scale={count} memo_writes={}",
            after.template_memo_writes - before.template_memo_writes
        );
        observed.push((
            count,
            after.template_memo_writes - before.template_memo_writes,
        ));
        assert_eq!(after.instances - before.instances, count as u64);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    assert!(
        observed.iter().all(|(_, writes)| *writes == 0),
        "unchanged weak entries were rewritten: {observed:?}"
    );
}
