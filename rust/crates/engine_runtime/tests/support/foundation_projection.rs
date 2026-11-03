//! Exact-count foundation projection consumer; no performance qualification.
use engine_runtime::{
    engine_host_loop::*, frame_loop::RuntimeFrameContext, project_runtime_session::*,
    render_state::RenderProxyPayload, runtime_instance_loader::RuntimeInstanceLoader,
    scene_loader::load_scene_into_world, wgpu_backend::real::RealWgpuBackend,
};
use project_runtime_sdk::{ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D};
use std::sync::{Arc, Mutex};

#[test]
#[ignore = "requires real GPU; package mesh CPU backing lifetime"]
fn gpu_mesh_cpu_backing_releases_after_static_upload_with_live_snapshot() {
    let fixture = super::streamed_cells::build_spatial_scene(2);
    let mut world = load_scene_into_world(&fixture.package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(
        engine_runtime::game_view_presentation::GameViewTargetSpec::new(
            32,
            32,
            engine_runtime::game_view_presentation::GameViewScalePolicy::Stretch,
        ),
    );
    let output = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(1),
            &mut world,
            RuntimeFrameContext::new(&fixture.package, &mut loader),
        )
        .render_thread_frame
        .unwrap();
    let plan = &output.renderer_output.rhi_command_plan;
    let source = plan
        .commands
        .iter()
        .find_map(|command| {
            if let engine_runtime::rhi_command_plan::RhiCommand::Draw { payload, .. } = command {
                payload.geometry_draw().map(|draw| draw.geometry.clone())
            } else {
                None
            }
        })
        .expect("actual production mesh draw");
    let reader = source.acquire_upload_source().unwrap();
    let pixels = Arc::downgrade(reader.vertex_storage());
    let original = reader.vertex_bytes().to_vec();
    let original_wire = serde_json::to_value(reader.as_ref()).unwrap();
    let index_bytes = reader.index_bytes().to_vec();
    assert_eq!(source.vertex_byte_len(), original.len());
    assert_eq!(source.index_byte_len(), index_bytes.len());
    assert!(!original.is_empty());
    let mut backend = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    backend
        .prepare_geometry_textures(&output.geometry_textures)
        .unwrap();
    let original_image = backend.render_plan_to_rgba_bytes(plan, 32, 32).unwrap();
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(reader.vertex_bytes(), original);
    drop(reader);
    assert!(
        pixels.upgrade().is_none(),
        "completed upload still pins CPU mesh bytes in live snapshot/cache/native"
    );
    assert!(host
        .retained_geometry_snapshot()
        .unwrap()
        .preparation_error()
        .is_none());

    let path = fixture
        .package
        .package_dir
        .join("cooked-assets/shared-mesh.json");
    let cooked_bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let uploads = backend.geometry_residency().meshes;
    assert_eq!(
        backend.render_plan_to_rgba_bytes(plan, 32, 32).unwrap(),
        original_image
    );
    assert_eq!(backend.geometry_residency().meshes, uploads);
    assert!(source.acquire_upload_source().is_err());
    assert!(
        serde_json::to_value(source.as_ref()).is_err(),
        "serialization emitted an empty mesh"
    );
    let mut corrupt = cooked_bytes.clone();
    corrupt[0] ^= 1;
    std::fs::write(&path, corrupt).unwrap();
    assert!(source
        .acquire_upload_source()
        .unwrap_err()
        .contains("identity"));
    assert!(serde_json::to_value(source.as_ref()).is_err());
    std::fs::write(&path, &cooked_bytes).unwrap();
    let restored = source.acquire_upload_source().unwrap();
    assert_eq!(restored.vertex_bytes(), original);
    assert_eq!(restored.index_bytes(), index_bytes);
    let restored_bytes = Arc::downgrade(restored.vertex_storage());
    drop(restored);
    assert_eq!(
        serde_json::to_value(source.as_ref()).unwrap(),
        original_wire
    );
    let roundtrip: engine_runtime::render_geometry::GeometryBuffers =
        serde_json::from_value(original_wire.clone()).unwrap();
    assert_eq!(source.as_ref(), &roundtrip);
    assert_eq!(roundtrip.vertex_bytes(), original);
    let mut changed_wire = original_wire;
    let x = f32::from_le_bytes(original[..4].try_into().unwrap()) + 0.125;
    for (i, byte) in x.to_le_bytes().into_iter().enumerate() {
        changed_wire["vertexBytes"][i] = byte.into();
    }
    let changed: engine_runtime::render_geometry::GeometryBuffers =
        serde_json::from_value(changed_wire).unwrap();
    assert_ne!(
        source.as_ref(),
        &changed,
        "descriptor equality hid changed mesh bytes"
    );

    let mut fresh = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    fresh
        .prepare_geometry_textures(&output.geometry_textures)
        .unwrap();
    assert_eq!(
        fresh.render_plan_to_rgba_bytes(plan, 32, 32).unwrap(),
        original_image
    );
    fresh
        .wait_gpu_submission(
            fresh.last_gpu_submission().unwrap(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
    assert!(
        restored_bytes.upgrade().is_none(),
        "fresh physical upload retained regenerated CPU bytes"
    );

    use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, GeometryUploadState};
    let snapshot = host.retained_geometry_snapshot().unwrap();
    let prepared: Vec<_> = snapshot.prepared_geometries().cloned().collect();
    let segmented_reader = source.acquire_upload_source().unwrap();
    let segmented_bytes = Arc::downgrade(segmented_reader.vertex_storage());
    let mut segmented = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    segmented
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 16 * 1024 * 1024,
            staging_bytes: 128,
            bytes_per_frame: 64,
            max_tickets: 8,
        })
        .unwrap();
    let ticket = segmented.begin_geometry_upload(&prepared, &[]).unwrap();
    let shared = segmented.begin_geometry_upload(&prepared, &[]).unwrap();
    source.release_upload_source();
    drop(segmented_reader);
    assert!(
        segmented_bytes.upgrade().is_some(),
        "pending segmented upload lost its CPU lease"
    );
    let started = std::time::Instant::now();
    let mut frame = 0;
    while segmented.geometry_upload_state(ticket).unwrap() != GeometryUploadState::Ready {
        segmented.pump_geometry_uploads(frame).unwrap();
        frame += 1;
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        std::thread::yield_now();
    }
    assert_eq!(
        segmented.geometry_upload_state(shared).unwrap(),
        GeometryUploadState::Ready
    );
    assert!(
        segmented_bytes.upgrade().is_none(),
        "completed segmented upload retained CPU backing"
    );
    segmented.release_geometry_upload(ticket).unwrap();
    assert!(
        segmented.geometry_residency().meshes > 0,
        "one ticket released another ticket's resource"
    );
    segmented.release_geometry_upload(shared).unwrap();

    // Cancellation retires the pending upload source through the same actual
    // completion queue, even after the shared cache relinquishes its copy.
    let mut cancelled = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    cancelled
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 16 * 1024 * 1024,
            staging_bytes: 128,
            bytes_per_frame: 64,
            max_tickets: 8,
        })
        .unwrap();
    let pending_reader = source.acquire_upload_source().unwrap();
    let pending_bytes = Arc::downgrade(pending_reader.vertex_storage());
    let ticket = cancelled.begin_geometry_upload(&prepared, &[]).unwrap();
    assert!(cancelled.pump_geometry_uploads(0).unwrap() > 0);
    let token = cancelled.last_gpu_submission().unwrap();
    source.release_upload_source();
    drop(pending_reader);
    cancelled.release_geometry_upload(ticket).unwrap();
    assert!(
        pending_bytes.upgrade().is_some(),
        "cancellation dropped CPU upload lease before actual completion"
    );
    cancelled
        .wait_gpu_submission(token, std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        pending_bytes.upgrade().is_none(),
        "cancelled upload source did not retire"
    );

    let abandoned_reader = source.acquire_upload_source().unwrap();
    let abandoned_bytes = Arc::downgrade(abandoned_reader.vertex_storage());
    let mut abandoned = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    abandoned
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 16 * 1024 * 1024,
            staging_bytes: 128,
            bytes_per_frame: 64,
            max_tickets: 8,
        })
        .unwrap();
    let ticket = abandoned.begin_geometry_upload(&prepared, &[]).unwrap();
    abandoned.pump_geometry_uploads(0).unwrap();
    let completed = abandoned.gpu_lifecycle_stats().completed;
    source.release_upload_source();
    drop(abandoned_reader);
    assert!(abandoned_bytes.upgrade().is_some());
    abandoned.abandon_device_epoch();
    assert_eq!(abandoned.gpu_lifecycle_stats().completed, completed);
    assert_eq!(
        abandoned.geometry_upload_state(ticket).unwrap(),
        GeometryUploadState::Abandoned
    );
    assert!(
        abandoned_bytes.upgrade().is_none(),
        "destroyed device kept old CPU upload source"
    );
    assert_eq!(
        source.acquire_upload_source().unwrap().vertex_bytes(),
        original
    );
    source.release_upload_source();
}

#[test]
fn package_texture_mip_lease_reproduces_sampling_and_shares_material_sources() {
    let mut fixture = super::streamed_cells::build_spatial_scene_with_mip_sampling(2);
    let engine_runtime::spatial_contract::RuntimeSpatial::World3D { mesh_renderer, .. } =
        fixture.package.active_scene.entities[1]
            .spatial
            .as_mut()
            .unwrap();
    mesh_renderer
        .as_mut()
        .unwrap()
        .materials
        .get_mut("surface")
        .unwrap()
        .0 = super::streamed_cells::reference("material-b", "material");
    let mut world = load_scene_into_world(&fixture.package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut host = EngineHostLoop::new("main");
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(1),
        &mut world,
        RuntimeFrameContext {
            package: &fixture.package,
            instance_loader: &mut loader,
            cell_resources: None,
            cell_backend: None,
        },
    );
    let rendered = output.render_thread_frame.unwrap();
    assert!(host
        .retained_geometry_snapshot()
        .unwrap()
        .preparation_error()
        .is_none());
    assert_eq!(rendered.geometry_textures.len(), 1);
    let texture = &rendered.geometry_textures[0];
    assert!(texture.payload.rgba8.is_empty());
    let identity = texture.content_identity();
    let first = texture.acquire_upload_payload().unwrap();
    assert_eq!(first.mip_count, 3);
    assert_eq!(
        first.sampler,
        "nearestMipmapNearest:nearest:repeat:clampToEdge"
    );
    assert_eq!(first.rgba8.len(), 84);
    let expected = first.as_ref().clone();
    let pixels = Arc::downgrade(&first.rgba8);
    drop(first);
    texture.release_upload_payload();
    // Both distinct material preparations and the old snapshot stay alive.
    assert!(
        pixels.upgrade().is_some(),
        "explicit CPU consumer was invalidated"
    );
    let expected_bytes = expected.rgba8.as_ref().clone();
    drop(expected);
    assert!(
        pixels.upgrade().is_none(),
        "a second material source pinned mip pixels"
    );
    let restored = texture.acquire_upload_payload().unwrap();
    assert_eq!(restored.rgba8.as_slice(), expected_bytes);
    assert_eq!(restored.mip_count, 3);
    assert_eq!(restored.sampler, texture.payload.sampler);
    assert_eq!(texture.content_identity(), identity);
    let mut replaced = texture.clone();
    Arc::make_mut(&mut replaced.payload).sampler = "nearestClamp".into();
    assert!(replaced
        .acquire_upload_payload()
        .unwrap_err()
        .contains("identity"));
    texture.release_upload_payload();

    // Production released the loader's old decoded pixels too. A later material
    // preparation must not accept changed disk bytes as a new verified source.
    let path = fixture
        .package
        .package_dir
        .join("cooked-assets/texture.rgba8");
    let original = std::fs::read(&path).unwrap();
    let mut corrupted = original.clone();
    corrupted[0] ^= 1;
    std::fs::write(&path, &corrupted).unwrap();
    assert!(
        loader
            .asset_loader()
            .load_texture_payload(&super::streamed_cells::reference(
                "shared-texture",
                "texture"
            ))
            .is_err(),
        "cold loader accepted pixels outside its original package receipt"
    );
    std::fs::write(&path, original).unwrap();
}

#[test]
#[ignore = "requires real GPU; texture CPU backing lifetime"]
fn gpu_texture_cpu_pixels_release_after_completion_with_live_snapshot() {
    use engine_runtime::wgpu_backend::real::GeometryUploadState;
    use std::time::{Duration, Instant};
    let mut fixture = super::streamed_cells::build_spatial_scene_with_texture_extent(2, 8);
    for entity in &mut fixture.package.active_scene.entities {
        if let Some(engine_runtime::spatial_contract::RuntimeSpatial::World3D {
            transform,
            mesh_renderer: Some(_),
            ..
        }) = &mut entity.spatial
        {
            transform.translation = [0., 0., -2.];
            transform.scale = [0.75; 3];
        }
    }
    let mut world = load_scene_into_world(&fixture.package.active_scene)
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(engine_runtime::game_view_presentation::GameViewTargetSpec {
        extent: engine_runtime::game_view_presentation::GameViewExtent::new(32, 32),
        scale_policy: engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
    });
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(1),
        &mut world,
        RuntimeFrameContext {
            package: &fixture.package,
            instance_loader: &mut loader,
            cell_resources: None,
            cell_backend: None,
        },
    );
    let rendered = output.render_thread_frame.unwrap();
    assert_eq!(rendered.geometry_textures.len(), 1);
    let texture = &rendered.geometry_textures[0];
    let identity = texture.content_identity();
    let cpu_reader = texture.acquire_upload_payload().unwrap();
    let pixels = Arc::downgrade(&cpu_reader.rgba8);
    assert_eq!(cpu_reader.rgba8.len(), 256);
    let mut backend = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    backend
        .set_geometry_upload_budget(engine_runtime::wgpu_backend::real::GeometryUploadBudget {
            resident_bytes: 1024 * 1024,
            staging_bytes: 128,
            bytes_per_frame: 64,
            max_tickets: 8,
        })
        .unwrap();
    let ticket = backend
        .begin_geometry_upload(&[], &rendered.geometry_textures)
        .unwrap();
    let started = Instant::now();
    let mut frame = 0;
    while backend.geometry_upload_state(ticket).unwrap() != GeometryUploadState::Ready {
        backend.pump_geometry_uploads(frame).unwrap();
        frame += 1;
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::yield_now();
    }
    assert!(
        pixels.upgrade().is_some(),
        "a CPU reader must retain its pixels"
    );
    drop(cpu_reader);
    assert!(
        pixels.upgrade().is_none(),
        "completed GPU upload still pins CPU pixels through cache/snapshot"
    );
    assert!(host
        .retained_geometry_snapshot()
        .unwrap()
        .preparation_error()
        .is_none());
    assert_eq!(texture.content_identity(), identity);
    let uploaded = backend.geometry_upload_snapshot().uploaded_bytes;
    let plan = &rendered.renderer_output.rhi_command_plan;
    let record = fixture
        .package
        .runtime_asset_index
        .record_by_guid("shared-texture")
        .unwrap();
    let cooked = fixture
        .package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    let metadata: engine_runtime::runtime_package::CookedTextureAsset = serde_json::from_slice(
        &std::fs::read(
            fixture
                .package
                .package_dir
                .join(cooked.path.as_ref().unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    let path = fixture.package.package_dir.join(&metadata.pixel_data_path);
    let original = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let unchanged = backend
        .begin_geometry_upload(&[], &rendered.geometry_textures)
        .unwrap();
    assert_eq!(
        backend.geometry_upload_state(unchanged).unwrap(),
        GeometryUploadState::Ready
    );
    assert_eq!(backend.geometry_upload_snapshot().uploaded_bytes, uploaded);
    assert!(
        texture.acquire_upload_payload().is_err(),
        "missing source must fail explicit CPU reacquisition"
    );
    let mut corrupted = original.clone();
    corrupted[0] ^= 1;
    std::fs::write(&path, &corrupted).unwrap();
    assert!(texture
        .acquire_upload_payload()
        .unwrap_err()
        .contains("identity"));
    std::fs::write(&path, &original).unwrap();
    let reacquired = texture.acquire_upload_payload().unwrap();
    assert_eq!(reacquired.rgba8.as_slice(), original);
    let restored_pixels = Arc::downgrade(&reacquired.rgba8);
    drop(reacquired);
    // A fresh physical backend must regenerate the exact frozen source, even
    // while the old backend, logical asset and immutable snapshot remain live.
    let mut replacement = RealWgpuBackend::new_offscreen(32, 32).unwrap();
    let retry = replacement
        .begin_geometry_upload(&[], &rendered.geometry_textures)
        .unwrap();
    let started = Instant::now();
    while replacement.geometry_upload_state(retry).unwrap() != GeometryUploadState::Ready {
        replacement.pump_geometry_uploads(frame).unwrap();
        frame += 1;
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::yield_now();
    }
    assert!(
        restored_pixels.upgrade().is_none(),
        "replacement upload pinned regenerated pixels"
    );
    let restored_image = replacement.render_plan_to_rgba_bytes(plan, 32, 32).unwrap();
    assert!(restored_image
        .chunks_exact(4)
        .any(|p| p[0] > 200 && p[1] < 5 && p[2] < 5));
    assert_eq!(texture.content_identity(), identity);
    replacement.release_geometry_upload(retry).unwrap();
    backend.release_geometry_upload(unchanged).unwrap();
    backend.release_geometry_upload(ticket).unwrap();
}

#[derive(Default)]
struct Step {
    anchor: f64,
    round: usize,
}
struct Session(Arc<Mutex<Step>>);
impl ProjectRuntimeSession for Session {
    fn session_id(&self) -> &str {
        "foundation-projection"
    }
    fn handle_aui_actions(
        &mut self,
        _: ProjectRuntimeSessionContext<'_>,
        _: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }
    fn fixed_update(
        &mut self,
        ctx: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput {
        let step = self.0.lock().unwrap();
        let offset = step.round as f32 * 16.;
        let mut batch = ProjectRuntimeMutationBuffer::new();
        for i in 0..=1000 {
            let id: engine_runtime::ids::SourceEntityId = if i == 1000 {
                "camera".into()
            } else {
                format!("node-{i}").into()
            };
            let generation = ctx.world.entity_generation(&id).unwrap();
            batch.spatial_command(
                id.clone(),
                generation,
                Intent::SetWorldAnchor {
                    anchor: [step.anchor + f64::from(offset), 0., 0.],
                    preserve_world: step.round > 0,
                },
            );
            // Value writes are the final overlay in the OLD anchor, then
            // keep-world structure migration rebases that pose atomically.
            // Static objects/camera need only the anchor operation after round 0.
            if step.round > 0 && i >= 10 {
                continue;
            }
            let value_offset = if step.round == 0 { 0. } else { offset - 16. };
            let position = if i == 1000 {
                [0., 0., 0.]
            } else {
                [
                    (i % 40) as f32 * 0.08 - 1.56 - value_offset
                        + if i < 10 {
                            step.round as f32 * 0.001
                        } else {
                            0.
                        },
                    (i / 40) as f32 * 0.08 - 0.96,
                    -4.,
                ]
            };
            batch.spatial_command(
                id,
                generation,
                Intent::SetLocalTransform {
                    transform: ProjectRuntimeTransform3D {
                        translation: position,
                        scale: if i == 1000 { [1.; 3] } else { [0.025; 3] },
                        ..ProjectRuntimeTransform3D::identity()
                    },
                },
            );
        }
        ProjectRuntimeSessionOutput::applied(batch)
    }
}

fn project(matrix: &[f32; 16], position: [f32; 3]) -> [f64; 2] {
    // Independent homogeneous evaluation in f64. Expected lens/pose below are
    // authored constants, never derived from the engine's evaluated matrices.
    let p = [
        f64::from(position[0]),
        f64::from(position[1]),
        f64::from(position[2]),
        1.,
    ];
    let clip: [f64; 4] =
        std::array::from_fn(|r| (0..4).map(|c| f64::from(matrix[c * 4 + r]) * p[c]).sum());
    [
        (clip[0] / clip[3] + 1.) * 128.,
        (1. - clip[1] / clip[3]) * 128.,
    ]
}

#[test]
#[ignore = "requires real GPU; foundation correctness, not a Release timing window"]
fn bm09_package_host_gpu_three_anchors_twenty_rebases_match_independent_projection() {
    let fixture = super::streamed_cells::build_spatial_scene(1000);
    assert_eq!(fixture.package.active_scene.entities.len(), 1001);
    let mut backend = RealWgpuBackend::new_offscreen(256, 256).unwrap();
    let mut references = Vec::new();
    let mut max_position_error = 0f64;
    let mut max_screen_error = 0f64;
    let mut rows = Vec::new();
    for anchor in [0., 10000., 1000000.] {
        let mut world = load_scene_into_world(&fixture.package.active_scene)
            .value
            .unwrap();
        let mut loader = RuntimeInstanceLoader::from_package(&fixture.package);
        let step = Arc::new(Mutex::new(Step::default()));
        let mut host = EngineHostLoop::with_project_runtime_session(
            "main",
            Default::default(),
            Box::new(Session(step.clone())),
        );
        host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
        host.set_game_view_target(engine_runtime::game_view_presentation::GameViewTargetSpec {
            extent: engine_runtime::game_view_presentation::GameViewExtent::new(256, 256),
            scale_policy: engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
        });
        let mut old: Option<(
            Arc<engine_runtime::render_geometry::ProductionGeometrySnapshot>,
            engine_runtime::render_geometry::RetainedGeometryView,
        )> = None;
        // Round zero initializes the anchor; twenty actual keep-world
        // relocations follow. Initialization is not one of those relocations.
        for round in 0..=20 {
            *step.lock().unwrap() = Step { anchor, round };
            let output = host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(1),
                &mut world,
                RuntimeFrameContext {
                    package: &fixture.package,
                    instance_loader: &mut loader,
                    cell_resources: None,
                    cell_backend: Some(&mut backend),
                },
            );
            assert!(
                !output
                    .project_runtime_session_report
                    .as_ref()
                    .unwrap()
                    .terminal_fault,
                "{output:?}"
            );
            let snapshot = host.retained_geometry_snapshot().unwrap();
            assert!(
                snapshot.preparation_error().is_none(),
                "{:?}",
                snapshot.preparation_error()
            );
            assert_eq!(snapshot.views.len(), 1);
            assert_eq!(
                snapshot.views[0].previous_camera, snapshot.views[0].camera,
                "origin relocation must reset camera history rather than produce false motion"
            );
            let current = snapshot.views[0].clone();
            if let Some((previous, frozen)) = old.take() {
                assert_eq!(
                    previous.views[0], frozen,
                    "in-flight packet camera history was mutated"
                );
            }
            old = Some((snapshot.clone(), current));
            let camera = snapshot.views[0].camera.executable().unwrap().unwrap();
            let matrix = camera.view_projection();
            let mut consumed = 0;
            for i in 0..1000 {
                let id = format!("node-{i}").into();
                let proxy = host
                    .render_scene()
                    .proxy(host.render_scene().proxy_for_source(&id).unwrap())
                    .unwrap();
                let RenderProxyPayload::Mesh3D(mesh) = &proxy.payload else {
                    panic!("missing mesh projection")
                };
                let point = [
                    mesh.world.world_matrix[12],
                    mesh.world.world_matrix[13],
                    mesh.world.world_matrix[14],
                ];
                let expected = [
                    (i % 40) as f64 * 0.08 - 1.56 + if i < 10 { round as f64 * 0.001 } else { 0. },
                    (i / 40) as f64 * 0.08 - 0.96,
                    -4.,
                ];
                let global = mesh.coordinate_origin[0] + f64::from(point[0]);
                let error = (global - anchor - expected[0]).abs();
                max_position_error = max_position_error.max(error);
                assert!(
                    error <= 0.001,
                    "anchor={anchor} round={round} object={i}: position error={error}; point={point:?}, origin={:?}, worldAnchor={:?}, local={:?}, camera={:?}",
                    mesh.coordinate_origin, mesh.world.anchor, world.transform3d(&id).unwrap().translation,
                    snapshot.views[0].camera.pose.position
                );
                let screen = project(&matrix, point);
                let focal = 1. / (30f64.to_radians()).tan();
                let expected_pixel = [
                    (1. + expected[0] * focal / 4.) * 128.,
                    (1. - expected[1] * focal / 4.) * 128.,
                ];
                for axis in 0..2 {
                    let error = (screen[axis] - expected_pixel[axis]).abs();
                    max_screen_error = max_screen_error.max(error);
                    assert!(
                        error <= 0.25,
                        "anchor={anchor} round={round} object={i}: pixel error={error}"
                    );
                }
                consumed += 1;
            }
            assert_eq!(consumed, 1000);
            let rendered = output.render_thread_frame.unwrap();
            backend
                .prepare_geometry_textures(&rendered.geometry_textures)
                .unwrap();
            let plan = rendered.renderer_output.rhi_command_plan;
            let pixels = backend.render_plan_to_rgba_bytes(&plan, 256, 256).unwrap();
            assert!(
                pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] > 200 && p[1] < 5 && p[2] < 5)
                    .count()
                    > 100,
                "authored red objects were not actually drawn"
            );
            if anchor == 0. {
                references.push(pixels);
            } else {
                let reference = &references[round];
                let max = pixels
                    .iter()
                    .zip(reference)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                assert!(
                    max <= 1,
                    "anchor={anchor} round={round}: GPU pixels differ by {max}/255"
                );
            }
            rows.push(
                serde_json::json!({"anchor":anchor,"round":round,"initialization":round == 0,"projectedObjects":consumed}),
            );
        }
        assert!(loader
            .asset_loader_mut()
            .shutdown_streaming(std::time::Duration::from_secs(2)));
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    if let Ok(path) = std::env::var("ENGINE_FOUNDATION_PROJECTION_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({
            "recipe":"BM-09/foundation-v1", "claim":"package-host-projection-GPU correctness only",
            "maxPositionErrorMeters":max_position_error,"maxScreenErrorPixels":max_screen_error,
            "objectsPerFrame":1000,"motionProbes":10,"relocationsPerAnchor":20,
            "initializationFrames":3,"rows":rows,"formalQualified":false
        })).unwrap()).unwrap();
    }
}
