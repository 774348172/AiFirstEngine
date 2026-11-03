use super::*;
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::frame_loop::RuntimeFrameContext;
use std::sync::Arc;

fn tick(
    host: &mut EngineHostLoop,
    world: &mut World,
    hydrator: &mut RuntimeSceneHydrator,
    fixture: &Fixture,
) -> engine_runtime::render_thread::RenderThreadFrameOutput {
    let output = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            world,
            RuntimeFrameContext::new(&fixture.package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    assert_eq!(output.report.rhi_status, "ok", "{:?}", output.report);
    output
}
fn many(count: usize) -> Value {
    let mut scene = base();
    scene["entities"][1]["spatial"]["transform"]["translation"] = json!([0, 0, 5]);
    let mesh = scene["entities"][0].clone();
    for i in 1..count {
        let mut copy = mesh.clone();
        copy["id"] = json!(format!("cube-{i}"));
        scene["entities"].as_array_mut().unwrap().push(copy);
    }
    scene
}
#[test]
fn retained_frame_cache_respects_target_extent_and_foreign_resource_owner() {
    use engine_runtime::render_thread::{
        RenderFramePacket, RenderThread, RenderThreadConfig, RenderThreadFrameInput,
    };
    use engine_runtime::runtime_renderer::{QualityProfile, RenderTarget};
    let fixture = Fixture::new(many(2), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let target = RenderTarget::surface("cache-target", 64, 64);
    let first = host.render_thread_for_target(target.clone());
    let same = host.render_thread_for_target(target.clone());
    assert!(Arc::ptr_eq(
        &production_draws(&first)[0].view,
        &production_draws(&same)[0].view
    ));
    let resized_target = RenderTarget::surface("cache-target", 96, 64);
    let resized = host.render_thread_for_target(resized_target.clone());
    assert_eq!(production_draws(&resized)[0].view.target_extent, [96, 64]);
    assert!(!Arc::ptr_eq(
        &production_draws(&first)[0].view,
        &production_draws(&resized)[0].view
    ));
    let suspended = host.render_thread_for_target(RenderTarget::surface("cache-target", 0, 0));
    assert!(production_draws(&suspended).is_empty());
    let restored = host.render_thread_for_target(resized_target.clone());
    assert!(!Arc::ptr_eq(
        &production_draws(&resized)[0].view,
        &production_draws(&restored)[0].view
    ));
    // The cache is now populated for exactly this target. It cannot turn the
    // old snapshot into valid geometry in a different resource manager.
    let mut packet = RenderFramePacket::from_input(RenderThreadFrameInput {
        frame_index: 1,
        render_scene_state: &RenderSceneState::new(),
        render_frame_report: None,
        resource_requests: vec![],
        resource_release_requests: vec![],
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        view_id: None,
        quality_profile: QualityProfile::default(),
        render_target: resized_target,
    });
    packet.geometry = host.retained_geometry_snapshot().cloned();
    let output = RenderThread::new(RenderThreadConfig::default())
        .submit_frame_output(packet)
        .0;
    assert_eq!(output.report.rhi_status, "error");
    assert!(output
        .report
        .diagnostics
        .iter()
        .any(|d| d.code == "three_d.production_frame_failed"
            && d.message.contains("nativeLease.stale")));
    assert!(output.renderer_output.rhi_command_plan.commands.is_empty());
}

#[test]
fn retained_proxy_mutation_and_same_size_scene_replacement_cannot_reuse_stale_meshes() {
    let fixture = Fixture::new(many(1), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    let old = tick(&mut host, &mut world, &mut hydrator, &fixture);
    let pristine = host.render_scene().clone();
    let proxy_id = pristine.proxy_for_source(&"cube".into()).unwrap();
    let mut changed = pristine.clone();
    let RenderProxyPayload::Mesh3D(projection) = &mut changed.proxy_mut(proxy_id).unwrap().payload
    else {
        panic!("expected native geometry");
    };
    projection.world.world_matrix[12] = 3.;
    *host.render_scene_mut() = changed;
    let moved = tick(&mut host, &mut world, &mut hydrator, &fixture);
    near(production_draws(&moved)[0].world_transform[12].to_f32(), 3.);
    near(production_draws(&old)[0].world_transform[12].to_f32(), 0.);
    *host.render_scene_mut() = pristine.clone();
    let restored = tick(&mut host, &mut world, &mut hydrator, &fixture);
    near(
        production_draws(&restored)[0].world_transform[12].to_f32(),
        0.,
    );
    let mut replacement = RenderSceneState::new();
    for view in pristine.views() {
        replacement.register_view(view.clone());
    }
    let mut proxy = pristine.proxy(proxy_id).unwrap().clone();
    let RenderProxyPayload::Mesh3D(projection) = &mut proxy.payload else {
        panic!("expected native geometry");
    };
    projection.world.world_matrix[12] = 4.;
    replacement.insert_proxy(proxy);
    assert_eq!(replacement.proxies_len(), pristine.proxies_len());
    *host.render_scene_mut() = replacement;
    let replaced = tick(&mut host, &mut world, &mut hydrator, &fixture);
    near(
        production_draws(&replaced)[0].world_transform[12].to_f32(),
        4.,
    );
}

#[test]
fn retained_production_static_sparse_full_and_old_snapshot() {
    let fixture = Fixture::new(many(64), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    let first = tick(&mut host, &mut world, &mut hydrator, &fixture);
    let old = host.retained_geometry_snapshot().unwrap().clone();
    assert_eq!(old.updated_instances, 64);
    let second = tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert!(
        Arc::ptr_eq(&old, host.retained_geometry_snapshot().unwrap()),
        "static content must reuse the immutable snapshot"
    );
    assert!(
        Arc::ptr_eq(
            &production_draws(&first)[0].view,
            &production_draws(&second)[0].view
        ),
        "unchanged immutable snapshot/target rebuilt the executable view and draw descriptors"
    );
    let old_draw = production_draws(&first)[0].clone();
    let id = production_draws(&first)[0].instance_id;
    set(
        &mut world,
        "cube-7",
        ComponentValue::Transform3D(local([0.5, 0., 0.], [0.; 3], [1.; 3])),
    );
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let sparse = host.retained_geometry_snapshot().unwrap().clone();
    assert_eq!(sparse.updated_instances, 1);
    assert_eq!(sparse.content_version, old.content_version + 1);
    for i in 0..64 {
        let name = if i == 0 {
            "cube".into()
        } else {
            format!("cube-{i}")
        };
        set(
            &mut world,
            &name,
            ComponentValue::Transform3D(local([0.7, 0., 0.], [0.; 3], [1.; 3])),
        );
    }
    let full = tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert_eq!(
        host.retained_geometry_snapshot().unwrap().updated_instances,
        64
    );
    assert_eq!(production_draws(&first)[0], &old_draw);
    assert_eq!(production_draws(&second)[0].instance_id, id);
    assert!(production_draws(&full).iter().any(|d| d.instance_id == id));
    assert_eq!(old.content_version, 1);
}
#[test]
fn retained_generation_and_camera_change_do_not_alias_old_content() {
    let fixture = Fixture::new(many(1), None);
    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&fixture.package);
    let instance = hydrator
        .hydrate_active_scene(&fixture.package, &mut world)
        .instance
        .unwrap();
    let mut host = EngineHostLoop::new("main");
    let first = tick(&mut host, &mut world, &mut hydrator, &fixture);
    let old = host.retained_geometry_snapshot().unwrap().clone();
    set(
        &mut world,
        "camera",
        ComponentValue::Transform3D(local([0., 0., 6.], [0.; 3], [1.; 3])),
    );
    let moved = tick(&mut host, &mut world, &mut hydrator, &fixture);
    let new = host.retained_geometry_snapshot().unwrap();
    assert_eq!(new.updated_instances, 0);
    assert_eq!(new.views[0].previous_camera, old.views[0].camera);
    assert_ne!(
        production_draws(&first)[0].view.view_matrix,
        production_draws(&moved)[0].view.view_matrix
    );
    let instance_id = production_draws(&first)[0].instance_id;
    // Unload/hydrate is the real generation lifecycle, rather than replacing a fake ID.
    hydrator
        .instance_loader_mut()
        .unload_scene_instance(instance.instance_id, &mut world);
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let report = hydrator.hydrate_active_scene(&fixture.package, &mut world);
    assert!(!report.has_errors());
    let reborn = tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert_ne!(production_draws(&reborn)[0].instance_id, instance_id);
    assert_eq!(old.views[0].camera.pose.position, [0., 0., 5.]);
}

#[test]
fn retained_two_production_views_have_independent_origin_history_and_resize() {
    use engine_runtime::render_state::{RenderTargetKind, RenderViewKind, Viewport};
    use engine_runtime::runtime_renderer::RenderTarget;
    let mut scene = many(1);
    let mut camera = scene["entities"][1].clone();
    camera["id"] = json!("second-camera");
    camera["spatial"]["transform"]["translation"] = json!([20, 0, 5]);
    scene["entities"].as_array_mut().unwrap().push(camera);
    let fixture = Fixture::new(scene, None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut anchors = engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::new();
    for name in ["cube", "camera", "second-camera"] {
        anchors.spatial_command(
            name.into(),
            world.entity_generation(&name.into()).unwrap(),
            project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
                anchor: [1_000_000., 0., 0.],
                preserve_world: false,
            },
        );
    }
    anchors.prepare(&world).unwrap().commit(&mut world).unwrap();
    let mut host = EngineHostLoop::new("main");
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let view = host
        .project_auxiliary_camera_view(
            &world,
            &"second-camera".into(),
            Viewport {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            RenderViewKind::Preview,
            RenderTargetKind::ViewportTexture,
        )
        .unwrap();
    let output = tick(&mut host, &mut world, &mut hydrator, &fixture);
    let old = host.retained_geometry_snapshot().unwrap().clone();
    assert_eq!(old.views.len(), 2);
    let old_identity = output
        .renderer_output
        .rhi_command_plan
        .frame_identity
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(old_identity.views.len(), 2);
    let observation = host
        .observe_spatial(&world, &["cube".into()], DiagnosticLevel::Summary)
        .unwrap();
    assert!(
        observation.matches_projection,
        "auxiliary views must not steal primary camera observation"
    );
    assert_eq!(
        observation.view.unwrap().source_camera_entity_id.as_deref(),
        Some("camera")
    );
    // The secondary camera looks away from the cube: independent visible sets.
    let begins = output
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .filter(|c| {
            matches!(
                c,
                engine_runtime::rhi_command_plan::RhiCommand::BeginView { .. }
            )
        })
        .count();
    assert_eq!(begins, 2);
    assert_eq!(production_draws(&output).len(), 1);
    let secondary = old
        .views
        .iter()
        .find(|v| v.camera.view.view_id == view.0)
        .unwrap();
    assert_eq!(
        secondary.camera.view.view_kind,
        engine_runtime::render_contract::ViewKind::Preview
    );
    assert_eq!(
        secondary.camera.view.target,
        engine_runtime::render_contract::TargetKind::ViewportTexture
    );
    let primary = old
        .views
        .iter()
        .find(|v| v.camera.view.view_id != view.0)
        .unwrap();
    let mut batch = engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::new();
    batch.spatial_command(
        "second-camera".into(),
        world.entity_generation(&"second-camera".into()).unwrap(),
        project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
            anchor: [1_000_100., 0., 0.],
            preserve_world: false,
        },
    );
    batch.prepare(&world).unwrap().commit(&mut world).unwrap();
    world.prepare_spatial_presentation(99, 1.).unwrap();
    host.project_auxiliary_camera_view(
        &world,
        &"second-camera".into(),
        Viewport {
            x: 0,
            y: 0,
            width: 640,
            height: 360,
        },
        RenderViewKind::Preview,
        RenderTargetKind::ViewportTexture,
    )
    .unwrap();
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let new = host.retained_geometry_snapshot().unwrap().clone();
    let changed = new
        .views
        .iter()
        .find(|v| v.camera.view.view_id == view.0)
        .unwrap();
    assert_eq!(changed.camera.origin, [1_000_100., 0., 0.]);
    assert_eq!(
        changed.previous_camera, changed.camera,
        "origin/extent changes invalidate only this view history"
    );
    assert_eq!(changed.history_version, secondary.history_version + 1);
    assert_eq!(
        new.views
            .iter()
            .find(|v| v.camera.view.view_id != view.0)
            .unwrap(),
        primary
    );
    let resized = host.render_thread_for_target(RenderTarget::surface("main", 800, 450));
    let identity = resized
        .renderer_output
        .rhi_command_plan
        .frame_identity
        .as_ref()
        .unwrap();
    let secondary_identity = identity.views.iter().find(|v| v.view_id == view.0).unwrap();
    assert_eq!(
        secondary_identity.origin_bits,
        changed.camera.origin.map(f64::to_bits)
    );
    assert_eq!(
        secondary_identity.history_version,
        secondary.history_version + 1
    );
    let old_secondary_identity = old_identity
        .views
        .iter()
        .find(|v| v.view_id == view.0)
        .unwrap();
    assert_eq!(
        old_secondary_identity.origin_bits,
        secondary.camera.origin.map(f64::to_bits)
    );
    assert_eq!(
        old_secondary_identity.history_version,
        secondary.history_version
    );
    let viewports: Vec<_> = resized
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .filter_map(|c| match c {
            engine_runtime::rhi_command_plan::RhiCommand::BeginView { view, .. } => {
                Some(view.viewport)
            }
            _ => None,
        })
        .collect();
    assert!(viewports.contains(&[0, 0, 800, 450]));
    assert!(viewports.contains(&[0, 0, 400, 225]));
    assert_eq!(secondary.camera.origin, [1_000_000., 0., 0.]);
}

#[test]
fn retained_auxiliary_camera_updates_and_destruction_remove_stale_views() {
    use engine_runtime::render_state::{RenderTargetKind, RenderViewKind, Viewport};
    let mut scene = many(1);
    let mut camera = scene["entities"][1].clone();
    camera["id"] = json!("aux");
    scene["entities"].as_array_mut().unwrap().push(camera);
    let fixture = Fixture::new(scene, None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert!(host
        .project_auxiliary_camera_view(
            &world,
            &"camera".into(),
            Viewport {
                x: 0,
                y: 0,
                width: 640,
                height: 360
            },
            RenderViewKind::Preview,
            RenderTargetKind::ViewportTexture
        )
        .is_err());
    let view = host
        .project_auxiliary_camera_view(
            &world,
            &"aux".into(),
            Viewport {
                x: 0,
                y: 0,
                width: 640,
                height: 360,
            },
            RenderViewKind::Preview,
            RenderTargetKind::ViewportTexture,
        )
        .unwrap();
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    set(
        &mut world,
        "aux",
        ComponentValue::Transform3D(local([1., 0., 5.], [0.; 3], [1.; 3])),
    );
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert_eq!(
        host.retained_geometry_snapshot()
            .unwrap()
            .views
            .iter()
            .find(|v| v.camera.view.view_id == view.0)
            .unwrap()
            .camera
            .pose
            .position,
        [1., 0., 5.]
    );
    world.try_despawn_entity(&"aux".into()).unwrap();
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert!(host.render_scene().view(view).is_none());
    assert_eq!(host.retained_geometry_snapshot().unwrap().views.len(), 1);
}

#[test]
fn retained_binding_failure_never_reuses_old_projection_and_can_recover() {
    let fixture = Fixture::new(many(1), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    let old = tick(&mut host, &mut world, &mut hydrator, &fixture);
    let original = world.mesh_renderer3d(&"cube".into()).unwrap().clone();
    let mut broken = original.clone();
    broken.materials.get_mut("surface").unwrap().0.id = "missing-material".into();
    set(&mut world, "cube", ComponentValue::MeshRenderer3D(broken));
    let failed = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&fixture.package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    assert_ne!(failed.report.rhi_status, "ok");
    assert!(production_draws(&failed).is_empty());
    assert_eq!(production_draws(&old).len(), 1);
    set(&mut world, "cube", ComponentValue::MeshRenderer3D(original));
    let restored = tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert_eq!(production_draws(&restored).len(), 1);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU"]
fn gpu_external_plan_matches_headless_production_pixels_and_rejects_worker_switch() {
    use engine_runtime::render_thread::RenderThreadMode;
    use engine_runtime::runtime_renderer::RenderTarget;
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let fixture = Fixture::new(many(2), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let target = RenderTarget::surface("external-plan", 64, 64);
    let baseline = host.render_thread_for_target(target.clone());
    assert!(baseline.renderer_output.rhi_backend_report.is_some());
    assert!(host.set_external_rhi_enabled(true));
    let external = host.render_thread_for_target(target.clone());
    assert_eq!(external.report.rhi_status, "ok");
    assert_eq!(external.report.present_status, "not_submitted");
    assert!(external.renderer_output.rhi_backend_report.is_none());
    assert_eq!(
        external.renderer_output.rhi_command_plan.commands,
        baseline.renderer_output.rhi_command_plan.commands
    );
    assert_eq!(production_draws(&external).len(), 2);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let pixels = backend
        .render_plan_to_rgba_bytes(&baseline.renderer_output.rhi_command_plan, 64, 64)
        .unwrap();
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&external.renderer_output.rhi_command_plan, 64, 64)
            .unwrap(),
        pixels
    );
    assert_eq!(backend.geometry_upload_count(), 1);
    assert!(host.set_external_rhi_enabled(false));
    assert!(host
        .render_thread_for_target(target)
        .renderer_output
        .rhi_backend_report
        .is_some());
    let mut worker =
        EngineHostLoop::new_with_render_mode("main", RenderThreadMode::DedicatedThread);
    assert!(!worker.set_external_rhi_enabled(true));
    assert!(!worker.set_external_rhi_enabled(false));
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_dedicated_worker_alpha_packets_match_old_and_new_pixels() {
    use engine_runtime::game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec};
    use engine_runtime::render_thread::RenderThreadMode;
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let fixture = Fixture::new(many(1), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new_with_render_mode("main", RenderThreadMode::DedicatedThread);
    host.set_game_view_target(GameViewTargetSpec::new(
        64,
        64,
        GameViewScalePolicy::Stretch,
    ));
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    world.begin_spatial_tick(world.spatial_delta().tick + 1);
    let mut batch = engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::new();
    batch.spatial_command(
        "cube".into(),
        world.entity_generation(&"cube".into()).unwrap(),
        project_runtime_sdk::ProjectRuntimeSpatialIntent::SetLocalTransform {
            transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                translation: [2., 0., 0.],
                ..project_runtime_sdk::ProjectRuntimeTransform3D::identity()
            },
        },
    );
    batch.prepare(&world).unwrap().commit(&mut world).unwrap();
    world.finish_spatial_tick();
    let mut frame = |delta: f32| {
        let output = host
            .tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame).with_unscaled_delta_time(delta),
                &mut world,
                RuntimeFrameContext::new(&fixture.package, hydrator.instance_loader_mut()),
            )
            .render_thread_frame
            .unwrap();
        assert_eq!(output.report.rhi_status, "ok");
        output
    };
    let old = frame(engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME * 0.25);
    let new = frame(engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME * 0.5);
    near(production_draws(&old)[0].world_transform[12].to_f32(), 0.5);
    near(production_draws(&new)[0].world_transform[12].to_f32(), 1.5);
    near(
        world.spatial_world(&"cube".into()).unwrap().world_matrix[12],
        2.,
    );
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let old_pixels = backend
        .render_plan_to_rgba_bytes(&old.renderer_output.rhi_command_plan, 64, 64)
        .unwrap();
    let new_pixels = backend
        .render_plan_to_rgba_bytes(&new.renderer_output.rhi_command_plan, 64, 64)
        .unwrap();
    assert_ne!(old_pixels, new_pixels);
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&old.renderer_output.rhi_command_plan, 64, 64)
            .unwrap(),
        old_pixels
    );
    assert_eq!(backend.geometry_upload_count(), 1);
    assert_eq!(
        backend.retained_geometry_stats().parameter_uploaded_bytes,
        3 * 288 // 72 f32 geometry parameters, including fog
    );
}

#[test]
fn resource_memory_world_clones_use_physical_storage_and_snapshots_use_shared_storage() {
    use engine_runtime::resource_memory::{MemoryCoverage, ResourceMemorySample};
    let fixture = Fixture::new(many(128), None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut host = EngineHostLoop::new("main");
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let cloned_world = world.clone();
    assert_eq!(world.epoch(), cloned_world.epoch());
    let mut a = ResourceMemorySample::new(1, 1, 0);
    let mut b = ResourceMemorySample::new(1, 1, 0);
    world.observe_memory(&mut a);
    cloned_world.observe_memory(&mut b);
    assert!(!a.allocations.is_empty() && !b.allocations.is_empty());
    assert!(
        a.allocations.iter().all(|a| b
            .allocations
            .iter()
            .all(|b| a.allocation_id != b.allocation_id)),
        "a logical World epoch is not an allocation identity"
    );
    assert_eq!(a.coverage["cpu.owner.world"], MemoryCoverage::Partial);
    let snapshot = host.retained_geometry_snapshot().unwrap();
    let alias = snapshot.clone();
    let mut a = ResourceMemorySample::new(1, 1, 0);
    let mut b = ResourceMemorySample::new(1, 1, 0);
    snapshot.observe_memory(&mut a);
    alias.observe_memory(&mut b);
    assert!(!a.allocations.is_empty());
    assert_eq!(a.allocations, b.allocations);
    assert!(a.allocations.iter().any(|a| a.owner == "snapshots"));
}

#[test]
fn s2b_projected_lights_respect_layers_rotation_disable_and_view_origins() {
    use engine_runtime::render_state::{RenderTargetKind, RenderViewKind, Viewport};
    let mut scene = base();
    for (name, layers, rotation) in [
        ("sun", vec!["world"], [0., 90., 0.]),
        ("point", vec!["aux"], [0., 0., 0.]),
    ] {
        let mut n = node(name, None, local([0., 1., 2.], rotation, [2., 3., 4.]));
        n["spatial"]["light"] = json!({"schemaVersion":"light3d.v1","enabled":true,"color":[1,1,1],"layers":layers,"kind":if name=="sun"{json!({"type":"directional","illuminanceLux":1})}else{json!({"type":"point","intensityCandela":1,"rangeM":10})}});
        scene["entities"].as_array_mut().unwrap().push(n);
    }
    let mut second = scene["entities"][1].clone();
    second["id"] = json!("aux");
    second["spatial"]["camera"]["layers"] = json!(["aux"]);
    second["spatial"]["transform"]["translation"] = json!([-1000, 2, 5]);
    scene["entities"].as_array_mut().unwrap().push(second);
    let fixture = Fixture::new(scene, None);
    let (mut world, mut hydrator) = fixture.hydrate();
    let mut mutation =
        engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::default();
    mutation.spatial_command(
        "aux".into(),
        world.entity_generation(&"aux".into()).unwrap(),
        project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
            anchor: [1000., 0., 0.],
            preserve_world: false,
        },
    );
    mutation
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();
    let mut host = EngineHostLoop::new("main");
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let view = host
        .project_auxiliary_camera_view(
            &world,
            &"aux".into(),
            Viewport {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
            RenderViewKind::Preview,
            RenderTargetKind::ViewportTexture,
        )
        .unwrap();
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let old = host.retained_geometry_snapshot().unwrap().clone();
    let a = old
        .views
        .iter()
        .find(|v| v.camera.view.source_camera_entity_id.as_deref() == Some("camera"))
        .unwrap();
    let b = old
        .views
        .iter()
        .find(|v| v.camera.view.view_id == view.0)
        .unwrap();
    assert_eq!(a.camera.lights.len(), 1);
    assert_eq!(b.camera.lights.len(), 1);
    near3(a.camera.lights[0].direction, [-1., 0., 0.]);
    near3(b.camera.lights[0].position, [-1000., 1., 2.]);
    assert_eq!(b.camera.origin, [1000., 0., 0.]);
    let mut light = world.light3d(&"sun".into()).unwrap().clone();
    light.enabled = false;
    set(&mut world, "sun", ComponentValue::Light3D(light));
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    let next = host.retained_geometry_snapshot().unwrap();
    assert!(next.content_version > old.content_version);
    assert_eq!(next.updated_instances, 0);
    assert_eq!(
        old.views
            .iter()
            .map(|v| v.camera.lights.len())
            .sum::<usize>(),
        2
    );
    assert_eq!(
        next.views
            .iter()
            .map(|v| v.camera.lights.len())
            .sum::<usize>(),
        1
    );
    world.try_despawn_entity(&"point".into()).unwrap();
    tick(&mut host, &mut world, &mut hydrator, &fixture);
    assert!(host
        .retained_geometry_snapshot()
        .unwrap()
        .views
        .iter()
        .all(|v| v.camera.lights.is_empty()));
}
