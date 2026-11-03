//! Real Host/Projection consumers; controlled RHI failures cover publication policy.
#[path = "generated_content_collision_tests.rs"]
mod collision_tests;
use super::*;
use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use crate::engine_rhi::*;
use crate::geometry_upload::{GeometryUploadState, GeometryUploadTicket};
use crate::render_contract::*;
use crate::render_geometry::{generated::binding_tests, PreparedGeometry};
use crate::runtime_package_builder::*;
use crate::runtime_renderer::RenderTarget;

fn player_upload_budget(backend: &mut crate::wgpu_backend::real::RealWgpuBackend) {
    backend
        .set_geometry_upload_budget(crate::wgpu_backend::real::GeometryUploadBudget {
            resident_bytes: 4 * 1024 * 1024 * 1024,
            staging_bytes: 128 * 1024 * 1024,
            bytes_per_frame: 16 * 1024 * 1024,
            max_tickets: 256,
        })
        .unwrap();
}

fn native_fixture(textured: bool) -> (FrameFixture, RuntimeInstanceLoader, World, EngineHostLoop) {
    let (_, material, _) = binding_tests::fixture();
    let mut asset = material.asset;
    let RenderAssetData::Material(params) = &mut asset.asset else {
        unreachable!()
    };
    params.preset = MaterialPreset::Unlit;
    params.shader_source = UNLIT_SHADER_SOURCE.into();
    params.parameters.retain(|k, _| k == "baseColor");
    let mut assets = vec![];
    let mut textures = vec![];
    if textured {
        params.textures.insert(
            "baseColorTexture".into(),
            RenderAssetReference {
                expected_kind: RenderAssetKind::Texture,
                reference: RuntimeAssetRef {
                    id: "surface-texture".into(),
                    asset_type: "texture".into(),
                    guid: Some("texture-guid".into()),
                    sub_asset: None,
                },
            },
        );
        let mut source = RuntimePackageSourceAsset::new(
            "surface-texture",
            "Texture",
            "texture",
            "Assets/texture.asset",
            "cooked/texture.asset",
        );
        source.asset_guid = Some("texture-guid".into());
        assets.push(source);
        textures.push(RuntimePackageSourceTexture {
            metadata: crate::runtime_package::CookedTextureAsset {
                schema_version: crate::runtime_package::COOKED_TEXTURE_SCHEMA_VERSION.into(),
                asset_id: "surface-texture".into(),
                cooked_asset_id: "surface-texture-cooked".into(),
                source_hash: "376-texture".into(),
                width: 1,
                height: 1,
                format: "rgba8UnormSrgb".into(),
                color_space: "srgb".into(),
                mip_count: 1,
                byte_length: 4,
                pixel_data_path: "cooked/textures/surface.rgba8".into(),
                sampler: "linearClamp".into(),
            },
            rgba8: vec![255; 4].into(),
        });
    }
    params.validate().unwrap();
    let bytes = serde_json::to_vec(&RenderContract::new(asset)).unwrap();
    let mut source = RuntimePackageSourceAsset::new(
        "surface-material",
        "Surface",
        "material",
        "Assets/surface-material.json",
        "cooked/surface-material.json",
    );
    source.asset_guid = Some("material-guid".into());
    source.hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
    if textured {
        source.dependencies.push("texture-guid".into());
    }
    assets.push(source.with_runtime_payload(bytes));
    let fixture = frame_fixture_with_content(assets, textures);
    let mut loader = RuntimeAssetLoader::new(
        &fixture.root,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    assert!(loader.mount_bundle("startup"));
    let loader = RuntimeInstanceLoader::new(loader);
    let mut world = World::new();
    spawn(&mut world, "surface");
    spawn(&mut world, "camera");
    let mut transform = Transform3D::identity();
    transform.translation[2] = 2.;
    world.insert_component_value(
        "camera".into(),
        ComponentValue::Transform3D(transform.canonicalize().unwrap()),
    );
    let mut camera = crate::spatial_contract::Camera3D::perspective_template();
    camera.layers.clear();
    world.insert_component_value("camera".into(), ComponentValue::Camera3D(camera));
    world.select_main_camera(Some("camera".into())).unwrap();
    let mut host = EngineHostLoop::new("main");
    host.set_external_rhi_enabled(true);
    (fixture, loader, world, host)
}

fn request(world: &World, id: u64, expected: u64) -> GeneratedContentRequest {
    let mut request = replace(world, id);
    request.expected_applied_revision = expected;
    let GeneratedOperation3D::Replace { geometry, .. } = &mut request.operation else {
        unreachable!()
    };
    *geometry = binding_tests::geometry(3, [0., 0., 1.], [1., 0., 0., 1.]);
    geometry.sections[0].material.id = "surface-material".into();
    request
}
fn candidate(
    loader: &mut RuntimeInstanceLoader,
    world: &World,
    request: GeneratedContentRequest,
) -> ContentTicket {
    let ticket = loader.accept_generated_content(request, world).unwrap();
    loader.advance_generated_content_cpu(world);
    finish_cpu(loader, world);
    assert_eq!(receipt(loader, ticket).status, ContentStatus::Preparing);
    ticket
}
#[track_caller]
fn tick(
    host: &mut EngineHostLoop,
    fixture: &FrameFixture,
    loader: &mut RuntimeInstanceLoader,
    world: &mut World,
    backend: &mut (dyn EngineRhiBackend + 'static),
) -> u64 {
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_deferred_render_submission()
            .with_fixed_step_count(1),
        world,
        crate::frame_loop::RuntimeFrameContext {
            cell_resources: None,
            cell_backend: Some(backend),
            package: &fixture.package,
            instance_loader: loader,
        },
    );
    assert!(
        output.runtime_advanced,
        "{:?}",
        output.project_runtime_session_report
    );
    assert_eq!(
        host.retained_geometry_snapshot()
            .unwrap()
            .preparation_error(),
        None
    );
    output.frame_index
}
fn count(host: &EngineHostLoop) -> usize {
    host.retained_geometry_snapshot()
        .unwrap()
        .prepared_geometries()
        .count()
}
fn patch(
    world: &World,
    id: u64,
    revision: u64,
    patch: AttributePatch3D,
) -> GeneratedContentRequest {
    GeneratedContentRequest {
        operation: GeneratedOperation3D::UpdateAttributes { patch },
        ..request(world, id, revision)
    }
}

#[derive(Default)]
struct ControlledBackend {
    ready: bool,
    deny_release: bool,
    fail_poll: bool,
    uploads: BTreeMap<u64, Vec<PreparedGeometry>>,
}
impl EngineRhiBackend for ControlledBackend {
    fn begin_geometry_upload(
        &mut self,
        geometry: &[PreparedGeometry],
        _: &[crate::render_asset_production::PreparedTextureAsset],
    ) -> Result<GeometryUploadTicket, String> {
        let ticket = GeometryUploadTicket::issue_for_backend(376);
        self.uploads
            .insert(ticket.backend_parts().0, geometry.to_vec());
        Ok(ticket)
    }
    fn geometry_upload_state(
        &mut self,
        ticket: GeometryUploadTicket,
    ) -> Result<GeometryUploadState, String> {
        if self.fail_poll {
            return Err("test.poll_failed".into());
        }
        if !self.uploads.contains_key(&ticket.backend_parts().0) {
            return Err("geometry.unknown_upload_ticket".into());
        }
        Ok(if self.ready {
            GeometryUploadState::Ready
        } else {
            GeometryUploadState::Uploading
        })
    }
    fn release_geometry_upload(&mut self, ticket: GeometryUploadTicket) -> Result<(), String> {
        if self.deny_release {
            return Err("test.retirement_pressure".into());
        }
        self.uploads
            .remove(&ticket.backend_parts().0)
            .ok_or_else(|| "geometry.unknown_upload_ticket".into())
            .map(|_| ())
    }
    fn backend_kind(&self) -> &'static str {
        "controlled-376-policy"
    }
    fn begin_frame(&mut self, _: EngineRhiFrame) {
        unreachable!()
    }
    fn resolve_color_3d(&mut self, _: &crate::color_pipeline::ColorResolve3D) {
        unreachable!()
    }
    fn begin_view(&mut self, _: &str, _: &ExecutableRenderView, _: Option<&str>) {
        unreachable!()
    }
    fn clear(&mut self, _: &str, _: [crate::render_graph::OrderedF32; 4]) {
        unreachable!()
    }
    fn draw(&mut self, _: EngineRhiDrawCall) {
        unreachable!()
    }
    fn reconcile_particles(&mut self, _: &[crate::particle_render_contract::ParticleSourceFrame]) {
        unreachable!()
    }
    fn particle_step(&mut self, _: u64, _: &crate::particle_render_contract::ParticleRenderStep) {
        unreachable!()
    }
    fn submit(&mut self) {
        unreachable!()
    }
    fn present(&mut self, _: &str) {
        unreachable!()
    }
    fn finish_report(&mut self, _: &crate::rhi_command_plan::RhiCommandPlan) -> RhiBackendReport {
        unreachable!()
    }
}

#[test]
fn generated_content_376_native_host_ready_projection_patches_and_release_are_one_revision() {
    let (fixture, mut loader, mut world, mut host) = native_fixture(true);
    collision_tests::set_body(&mut world, &collision_tests::authored_body());
    let mut backend = ControlledBackend::default();
    let first = candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, first).status, ContentStatus::Preparing);
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert_eq!(count(&host), 0);
    backend.ready = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, first).status, ContentStatus::Applied);
    assert_eq!(receipt(&loader, first).commit_epoch, Some(1));
    assert_eq!(count(&host), 1);
    assert_eq!(
        host.retained_geometry_snapshot().unwrap().textures.len(),
        1,
        "package textures enter the original snapshot"
    );
    let old = world.mesh_renderer3d(&"surface".into()).cloned().unwrap();
    let held = host.retained_geometry_snapshot().unwrap().clone();
    let old_buffers = held.prepared_geometries().next().unwrap().upload_source().1;
    let positions = AttributePatch3D::positions(2, &[[0., 1.2, 0.]]).unwrap();
    let moved = candidate(&mut loader, &world, patch(&world, 2, 1, positions));
    backend.ready = false;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(world.mesh_renderer3d(&"surface".into()), Some(&old));
    backend.ready = true;
    backend.deny_release = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, moved).status, ContentStatus::Preparing);
    assert_eq!(world.mesh_renderer3d(&"surface".into()), Some(&old));
    backend.deny_release = false;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, moved).commit_epoch, Some(2));
    let observation = host
        .observe_spatial(&world, &["surface".into()], DiagnosticLevel::Trace)
        .unwrap();
    assert_eq!(observation.entities[0].bounds.unwrap().1[1], 1.2);
    assert_ne!(
        host.retained_geometry_snapshot()
            .unwrap()
            .prepared_geometries()
            .next()
            .unwrap()
            .upload_source()
            .1
            .vertex_bytes(),
        old_buffers.vertex_bytes()
    );
    let colored = candidate(
        &mut loader,
        &world,
        patch(
            &world,
            3,
            2,
            AttributePatch3D::colors(0, &[[0., 1., 0., 1.]; 3]).unwrap(),
        ),
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, colored).commit_epoch, Some(3));
    assert_eq!(
        host.observe_spatial(&world, &["surface".into()], DiagnosticLevel::Trace)
            .unwrap()
            .entities[0]
            .bounds,
        observation.entities[0].bounds
    );
    for visible in [false, true] {
        let mut renderer = world.mesh_renderer3d(&"surface".into()).cloned().unwrap();
        renderer.visible = visible;
        world.insert_component_value("surface".into(), ComponentValue::MeshRenderer3D(renderer));
        tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
        assert_eq!(receipt(&loader, colored).applied_revision, 3);
    }
    let release = candidate(
        &mut loader,
        &world,
        GeneratedContentRequest {
            operation: GeneratedOperation3D::Release,
            ..request(&world, 4, 3)
        },
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, release).commit_epoch, Some(4));
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert_eq!(
        count(&host),
        0,
        "original Projection removed the generated component"
    );
    assert!(backend.uploads.is_empty());
    drop(held);
    drop(old_buffers);
    drop(host);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_native_host_cancel_poll_failure_and_external_binding_preserve_old_content()
{
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    let first = candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, first).status, ContentStatus::Applied);
    let old = world.mesh_renderer3d(&"surface".into()).cloned().unwrap();
    let cancelled = candidate(&mut loader, &world, request(&world, 2, 1));
    backend.ready = false;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    loader.cancel_generated_content(cancelled).unwrap();
    backend.deny_release = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert!(loader.generated_content.records[&2].upload.is_some());
    assert_eq!(world.mesh_renderer3d(&"surface".into()), Some(&old));
    backend.deny_release = false;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert!(loader.generated_content.records[&2].upload.is_none());
    let failed = candidate(&mut loader, &world, request(&world, 3, 1));
    backend.fail_poll = true;
    backend.deny_release = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, failed).status, ContentStatus::Failed);
    assert!(
        loader.generated_content.records[&3].upload.is_some(),
        "poll error must not discard the native pin"
    );
    assert_eq!(world.mesh_renderer3d(&"surface".into()), Some(&old));
    backend.fail_poll = false;
    backend.deny_release = false;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(backend.uploads.len(), 1);
    world.remove_component_value(
        &"surface".into(),
        &crate::components::ComponentTypeId::mesh_renderer3d(),
    );
    assert_eq!(
        loader
            .accept_generated_content(
                patch(
                    &world,
                    4,
                    1,
                    AttributePatch3D::positions(0, &[[0., 0., 0.]]).unwrap()
                ),
                &world
            )
            .unwrap_err()
            .field_path,
        "patch.baseBinding"
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert!(backend.uploads.is_empty());
    assert_eq!(count(&host), 0);
    drop(host);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
#[ignore = "exclusive real GPU; original Host safe point, package textures, pixels and device replacement"]
fn generated_content_376_native_host_real_gpu_publication_pixels_recovery_and_drain() {
    use crate::wgpu_backend::real::RealWgpuBackend;
    use std::time::Duration;
    let (fixture, mut loader, mut world, mut host) = native_fixture(true);
    collision_tests::set_body(&mut world, &collision_tests::authored_body());
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    player_upload_budget(&mut backend);
    let mut frame = 0;
    fn settle(
        host: &mut EngineHostLoop,
        fixture: &FrameFixture,
        loader: &mut RuntimeInstanceLoader,
        world: &mut World,
        backend: &mut RealWgpuBackend,
        frame: &mut u64,
        ticket: ContentTicket,
    ) {
        for _ in 0..16 {
            *frame = tick(host, fixture, loader, world, backend);
            if receipt(loader, ticket).status == ContentStatus::Applied {
                return;
            }
            assert_eq!(
                receipt(loader, ticket).status,
                ContentStatus::Preparing,
                "{:?}",
                receipt(loader, ticket)
            );
            assert!(backend.pump_geometry_uploads(*frame).unwrap() <= 16 * 1024 * 1024);
            if let Some(token) = backend.last_gpu_submission() {
                backend
                    .wait_gpu_submission(token, Duration::from_secs(5))
                    .unwrap();
            }
        }
        panic!("Host candidate never applied after actual GPU readiness");
    }
    let render = |host: &mut EngineHostLoop, backend: &mut RealWgpuBackend| {
        let output =
            host.render_thread_for_target(RenderTarget::headless_texture("surface-main", 64, 64));
        backend
            .render_plan_to_rgba_bytes(&output.renderer_output.rhi_command_plan, 64, 64)
            .unwrap()
    };
    let mut first_request = request(&world, 1, 0);
    let GeneratedOperation3D::Replace { collision, .. } = &mut first_request.operation else {
        unreachable!()
    };
    *collision = collision_tests::mesh_collision();
    let first = candidate(&mut loader, &world, first_request);
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, first).status, ContentStatus::Preparing);
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert!(backend.geometry_upload_snapshot().pending_resources > 0);
    assert!((collision_tests::hit(&mut host, &world).unwrap().position[1] - 0.5).abs() < 1e-4);
    settle(
        &mut host,
        &fixture,
        &mut loader,
        &mut world,
        &mut backend,
        &mut frame,
        first,
    );
    assert_eq!(count(&host), 1);
    assert_eq!(host.retained_geometry_snapshot().unwrap().textures.len(), 1);
    assert_eq!(
        receipt(&loader, first).collision_revision,
        receipt(&loader, first).applied_revision
    );
    assert!((collision_tests::hit(&mut host, &world).unwrap().position[1] - 1.).abs() < 1e-4);
    let red = render(&mut host, &mut backend);
    let center = (32 * 64 + 32) * 4;
    assert!(
        red[center] > 200 && red[center + 1] < 20,
        "actual original Host red pixel: {:?}",
        &red[center..center + 4]
    );
    let moved = candidate(
        &mut loader,
        &world,
        patch(
            &world,
            2,
            1,
            AttributePatch3D::positions(2, &[[0., 1.2, 0.]]).unwrap(),
        ),
    );
    settle(
        &mut host,
        &fixture,
        &mut loader,
        &mut world,
        &mut backend,
        &mut frame,
        moved,
    );
    let bounds = host
        .observe_spatial(&world, &["surface".into()], DiagnosticLevel::Trace)
        .unwrap()
        .entities[0]
        .bounds;
    assert_eq!(bounds.unwrap().1[1], 1.2);
    let colored = candidate(
        &mut loader,
        &world,
        patch(
            &world,
            3,
            2,
            AttributePatch3D::colors(0, &[[0., 1., 0., 1.]; 3]).unwrap(),
        ),
    );
    settle(
        &mut host,
        &fixture,
        &mut loader,
        &mut world,
        &mut backend,
        &mut frame,
        colored,
    );
    let green = render(&mut host, &mut backend);
    assert!(
        green[center + 1] > 200 && green[center] < 20,
        "actual original Host green pixel: {:?}",
        &green[center..center + 4]
    );
    assert_ne!(red, green);
    let held = host
        .retained_geometry_snapshot()
        .unwrap()
        .prepared_geometries()
        .next()
        .unwrap()
        .upload_source()
        .1;
    let pointer = held.vertex_bytes().as_ptr();
    let before = world.mesh_renderer3d(&"surface".into()).cloned();
    backend.abandon_device_epoch();
    assert!(backend.gpu_lifecycle_stats().abandoned);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    let mut rebuilt = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    player_upload_budget(&mut rebuilt);
    frame = tick(&mut host, &fixture, &mut loader, &mut world, &mut rebuilt);
    assert!(rebuilt.geometry_upload_snapshot().pending_resources > 0);
    rebuilt.pump_geometry_uploads(frame).unwrap();
    if let Some(token) = rebuilt.last_gpu_submission() {
        rebuilt
            .wait_gpu_submission(token, Duration::from_secs(5))
            .unwrap();
    }
    tick(&mut host, &fixture, &mut loader, &mut world, &mut rebuilt);
    assert_eq!(world.mesh_renderer3d(&"surface".into()), before.as_ref());
    assert_eq!(receipt(&loader, colored).commit_epoch, Some(3));
    assert_eq!(
        host.retained_geometry_snapshot()
            .unwrap()
            .prepared_geometries()
            .next()
            .unwrap()
            .upload_source()
            .1
            .vertex_bytes()
            .as_ptr(),
        pointer
    );
    assert_eq!(render(&mut host, &mut rebuilt), green);
    assert_eq!(receipt(&loader, colored).collision_revision, 1);
    assert!((collision_tests::hit(&mut host, &world).unwrap().position[1] - 1.).abs() < 1e-4);
    let release = candidate(
        &mut loader,
        &world,
        GeneratedContentRequest {
            operation: GeneratedOperation3D::Release,
            ..request(&world, 4, 3)
        },
    );
    settle(
        &mut host,
        &fixture,
        &mut loader,
        &mut world,
        &mut rebuilt,
        &mut frame,
        release,
    );
    assert_eq!(count(&host), 0);
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert!(collision_tests::hit(&mut host, &world).is_none());
    drop(held);
    // Exercise the same Host/cache/backend across bounded real replacements and
    // same-frame destruction/recreation. Each cycle must release its real ticket.
    for cycle in 0..8 {
        let id = 5 + cycle * 3;
        let expected = if cycle == 0 { 4 } else { 0 };
        let mut r = request(&world, id, expected);
        let GeneratedOperation3D::Replace { collision, .. } = &mut r.operation else {
            unreachable!()
        };
        *collision = collision_tests::mesh_collision();
        let first = candidate(&mut loader, &world, r);
        settle(
            &mut host,
            &fixture,
            &mut loader,
            &mut world,
            &mut rebuilt,
            &mut frame,
            first,
        );
        let applied_revision = receipt(&loader, first).applied_revision;
        let replacement = candidate(
            &mut loader,
            &world,
            request(&world, id + 1, applied_revision),
        );
        settle(
            &mut host,
            &fixture,
            &mut loader,
            &mut world,
            &mut rebuilt,
            &mut frame,
            replacement,
        );
        assert_eq!(rebuilt.geometry_upload_snapshot().tickets, 1);
        assert_eq!(count(&host), 1);
        assert_eq!(
            receipt(&loader, replacement).collision_revision,
            applied_revision
        );
        assert!((collision_tests::hit(&mut host, &world).unwrap().position[1] - 1.).abs() < 1e-4);
        assert!(!render(&mut host, &mut rebuilt).is_empty());
        world.despawn_entity(&"surface".into()).unwrap();
        spawn(&mut world, "surface");
        tick(&mut host, &fixture, &mut loader, &mut world, &mut rebuilt);
        assert_eq!(count(&host), 0);
        assert_eq!(rebuilt.geometry_upload_snapshot().tickets, 0);
        assert!(collision_tests::hit(&mut host, &world).is_none());
        // The last submitted Host packet may still own its immutable source.
        // Consume the new empty projection and actual completion before asserting
        // CPU drain; deferred simulation alone does not consume a render packet.
        render(&mut host, &mut rebuilt);
        if let Some(token) = rebuilt.last_gpu_submission() {
            rebuilt
                .wait_gpu_submission(token, Duration::from_secs(5))
                .unwrap();
        }
        rebuilt.poll_gpu_completion().unwrap();
        // Accounting is reaped at the next original loader safe point, after
        // extraction/rendering have dropped their previous immutable readers.
        tick(&mut host, &fixture, &mut loader, &mut world, &mut rebuilt);
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            0
        );
    }
    drop(host);
    loader.close_generated_content();
    let drained = rebuilt.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert!(!drained.abandoned);
    assert_eq!(drained.completed, drained.submitted);
    assert_eq!(
        (
            drained.in_flight,
            drained.retiring_objects,
            drained.retiring_bytes
        ),
        (0, 0, 0)
    );
    let pending = rebuilt.geometry_upload_snapshot();
    assert_eq!(
        (
            pending.tickets,
            pending.pending_resources,
            pending.staging_bytes
        ),
        (0, 0, 0)
    );
    assert_eq!(rebuilt.geometry_residency().physical_mesh_bytes, 0);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
    eprintln!("376 original Host: Ready-only publication, package texture, red to green actual pixels, unchanged canonical on explicit Device replacement, Release and tracked drain passed; green={}",crate::canonical_digest::sha256_prefixed(&green));
}

#[test]
fn generated_content_376_native_host_exit_and_fault_retry_actual_retirement() {
    use crate::project_runtime_session::*;
    use std::sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    };
    struct Session(Arc<AtomicU8>);
    impl ProjectRuntimeSession for Session {
        fn session_id(&self) -> &str {
            "376-native-close"
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
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut output =
                ProjectRuntimeSessionOutput::applied(ProjectRuntimeMutationBuffer::new());
            match self.0.load(Ordering::Relaxed) {
                1 => output.host_control = Some(ProjectRuntimeHostControl::Exit),
                2 => output.status = ProjectRuntimeSessionStatus::Faulted,
                _ => {}
            }
            output
        }
    }
    for mode in [1, 2] {
        let (fixture, mut loader, mut world, _) = native_fixture(false);
        let mode_flag = Arc::new(AtomicU8::new(0));
        let mut host = EngineHostLoop::with_project_runtime_session(
            "main",
            crate::project_logic::ProjectLogicRunner::empty(),
            Box::new(Session(mode_flag.clone())),
        );
        host.set_external_rhi_enabled(true);
        let mut backend = ControlledBackend {
            ready: true,
            ..Default::default()
        };
        let mut r = request(&world, 1, 0);
        let GeneratedOperation3D::Replace { collision, .. } = &mut r.operation else {
            unreachable!()
        };
        *collision = collision_tests::mesh_collision();
        let applied = candidate(&mut loader, &world, r);
        tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
        assert_eq!(receipt(&loader, applied).status, ContentStatus::Applied);
        let mut r = request(&world, 2, 1);
        let GeneratedOperation3D::Replace { collision, .. } = &mut r.operation else {
            unreachable!()
        };
        *collision = collision_tests::mesh_collision();
        let pending = candidate(&mut loader, &world, r);
        backend.ready = false;
        tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
        backend.deny_release = true;
        mode_flag.store(mode, Ordering::Relaxed);
        let mut close_tick = |host: &mut EngineHostLoop, backend: &mut ControlledBackend| {
            host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_deferred_render_submission()
                    .with_fixed_step_count(1),
                &mut world,
                crate::frame_loop::RuntimeFrameContext {
                    cell_resources: None,
                    cell_backend: Some(backend),
                    package: &fixture.package,
                    instance_loader: &mut loader,
                },
            )
        };
        let closed = close_tick(&mut host, &mut backend);
        if mode == 1 {
            assert_eq!(closed.host_control, Some(ProjectRuntimeHostControl::Exit));
        } else {
            assert!(
                closed
                    .project_runtime_session_report
                    .unwrap()
                    .terminal_fault
            );
        }
        assert_eq!(
            backend.uploads.len(),
            2,
            "retirement pressure must retain actual native readers"
        );
        backend.deny_release = false;
        close_tick(&mut host, &mut backend);
        assert!(backend.uploads.is_empty());
        assert!(world.mesh_renderer3d(&"surface".into()).is_none());
        assert!(world
            .component_value(
                &"surface".into(),
                &crate::components::ComponentTypeId::new("engine.body3d")
            )
            .is_none());
        assert!(collision_tests::hit(&mut host, &world).is_none());
        assert_eq!(receipt(&loader, pending).status, ContentStatus::Cancelled);
        assert!(!loader.generated_content.accepting);
        drop(host);
        loader.close_generated_content();
        assert_eq!(
            loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
            0
        );
    }
}

#[test]
#[ignore = "exclusive real GPU; legal SDK envelopes through original Host and Player upload budget"]
fn generated_content_376_native_host_backlog_applies_only_ready_targets_across_frames() {
    use crate::wgpu_backend::real::RealWgpuBackend;
    use std::time::Duration;
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    player_upload_budget(&mut backend);
    let mut tickets = Vec::new();
    let mut total = 0;
    for index in 0..7 {
        let target = format!("backlog-{index}");
        spawn(&mut world, &target);
        let mut request = request(&world, index + 1, 0);
        request.target.entity_id = target.clone();
        request.target.generation = world.entity_generation(&target.into()).unwrap();
        let GeneratedOperation3D::Replace { geometry, .. } = &mut request.operation else {
            unreachable!()
        };
        *geometry = binding_tests::geometry(60_000, [0., 0., 1.], [1., 0., 0., 1.]);
        geometry.sections[0].material.id = "surface-material".into();
        total += geometry.vertex_bytes.len().next_multiple_of(4)
            + geometry.index_bytes.len().next_multiple_of(4);
        let envelope = project_runtime_sdk::spatial_bulk::encode_session(
            project_runtime_sdk::ProjectRuntimeSessionOutput {
                mutations: vec![
                    project_runtime_sdk::ProjectRuntimeDeferredMutation::GeneratedContent {
                        request: request.clone(),
                    },
                ],
                ..project_runtime_sdk::ProjectRuntimeSessionOutput::no_op()
            },
        )
        .unwrap();
        assert!(envelope.len() <= project_runtime_sdk::spatial_bulk::MAX_SESSION_BYTES);
        let ticket = candidate(&mut loader, &world, request);
        tickets.push(ticket);
    }
    const FRAME_BYTES: usize = 16 * 1024 * 1024;
    assert!(total > FRAME_BYTES);
    let mut frame = tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(count(&host), 0);
    let mut uploads = Vec::new();
    let mut saw_partial = false;
    for _ in 0..8 {
        let bytes = backend.pump_geometry_uploads(frame).unwrap();
        assert!(bytes <= FRAME_BYTES);
        uploads.push(bytes);
        assert_eq!(backend.pump_geometry_uploads(frame).unwrap(), 0);
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(5))
                .unwrap();
        }
        frame = tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
        let applied = tickets
            .iter()
            .filter(|t| receipt(&loader, **t).status == ContentStatus::Applied)
            .count();
        for (index, ticket) in tickets.iter().enumerate() {
            let status = receipt(&loader, *ticket).status;
            assert!(matches!(
                status,
                ContentStatus::Preparing | ContentStatus::Applied
            ));
            assert_eq!(
                world
                    .mesh_renderer3d(&format!("backlog-{index}").into())
                    .is_some(),
                status == ContentStatus::Applied
            );
        }
        assert_eq!(count(&host), applied);
        saw_partial |= applied > 0 && applied < 7;
        if applied == 7 {
            break;
        }
    }
    assert!(
        saw_partial,
        "Host must publish ready targets while retaining the rest as Preparing"
    );
    assert_eq!(uploads.iter().sum::<usize>(), total);
    assert_eq!(count(&host), 7);
    loader.close_generated_content();
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(count(&host), 0);
    drop(host);
    let drained = backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert_eq!(
        (
            drained.in_flight,
            drained.retiring_objects,
            drained.retiring_bytes
        ),
        (0, 0, 0)
    );
    assert_eq!(drained.completed, drained.submitted);
    assert!(!drained.abandoned);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    assert_eq!(backend.geometry_residency().physical_mesh_bytes, 0);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
    eprintln!("376 original Host backlog: total={total}, uploaded={uploads:?}, real Ready-only partial publication and drain passed");
}

#[test]
fn generated_content_376_native_host_superseded_destroyed_candidates_cannot_publish() {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let old = world.mesh_renderer3d(&"surface".into()).cloned();
    backend.ready = false;
    let stale = candidate(&mut loader, &world, request(&world, 2, 1));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let current = candidate(&mut loader, &world, request(&world, 3, 1));
    assert_eq!(receipt(&loader, stale).status, ContentStatus::Superseded);
    backend.ready = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, current).status, ContentStatus::Applied);
    assert_ne!(world.mesh_renderer3d(&"surface".into()), old.as_ref());
    assert_eq!(backend.uploads.len(), 1);
    let mut collision = request(&world, 4, 3);
    let GeneratedOperation3D::Replace {
        collision: update, ..
    } = &mut collision.operation
    else {
        unreachable!()
    };
    *update = CollisionUpdate3D::Clear;
    let cleared = candidate(&mut loader, &world, collision);
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, cleared).status, ContentStatus::Applied);
    assert_eq!(receipt(&loader, cleared).collision_revision, 4);
    assert_eq!(receipt(&loader, current).applied_revision, 4);
    backend.ready = false;
    let destroyed = candidate(&mut loader, &world, request(&world, 5, 4));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    world.despawn_entity(&"surface".into()).unwrap();
    spawn(&mut world, "surface");
    backend.ready = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, destroyed).status, ContentStatus::Failed);
    assert_eq!(
        receipt(&loader, destroyed).error.unwrap().code,
        ContentErrorCode::StaleTarget
    );
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert_eq!(count(&host), 0);
    assert!(backend.uploads.is_empty());
    drop(host);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_native_host_terminal_receipt_window_keeps_actual_upload_owner() {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let mut backend = ControlledBackend::default();
    let pinned = candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    loader.cancel_generated_content(pinned).unwrap();
    backend.deny_release = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    for id in 2..=130 {
        let ticket = loader
            .accept_generated_content(request(&world, id, 0), &world)
            .unwrap();
        loader.cancel_generated_content(ticket).unwrap();
    }
    assert_eq!(
        loader.generated_content.records.len(),
        GENERATED_RECEIPT_LIMIT
    );
    assert_eq!(receipt(&loader, pinned).status, ContentStatus::Cancelled);
    assert!(loader.generated_content.records[&1].upload.is_some());
    assert_eq!(backend.uploads.len(), 1);
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        150
    );
    assert_eq!(
        loader
            .generated_content_receipt(ContentTicket {
                request_id: 2,
                ..pinned
            })
            .unwrap_err()
            .code,
        ContentErrorCode::ReceiptExpired
    );
    backend.deny_release = false;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert!(backend.uploads.is_empty());
    assert!(loader.generated_content.records[&1].upload.is_none());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
    let next = loader
        .accept_generated_content(request(&world, 131, 0), &world)
        .unwrap();
    loader.cancel_generated_content(next).unwrap();
    assert_eq!(
        loader.generated_content_receipt(pinned).unwrap_err().code,
        ContentErrorCode::ReceiptExpired
    );
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    loader.close_generated_content();
}

#[test]
fn generated_content_376_native_host_world_replacement_retires_without_touching_new_incarnation() {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    backend.ready = false;
    let pending = candidate(&mut loader, &world, request(&world, 2, 1));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let (_, _, mut replacement, _) = native_fixture(false);
    // Same source identity and generation in an independently owned World.
    assert_eq!(
        replacement.entity_generation(&"surface".into()),
        world.entity_generation(&"surface".into())
    );
    tick(
        &mut host,
        &fixture,
        &mut loader,
        &mut replacement,
        &mut backend,
    );
    assert_eq!(receipt(&loader, pending).status, ContentStatus::Cancelled);
    assert!(replacement.mesh_renderer3d(&"surface".into()).is_none());
    assert!(backend.uploads.is_empty());
    assert_eq!(count(&host), 0);
    drop(host);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}
