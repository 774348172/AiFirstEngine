//! F04: delayed real worker, immutable packet versions and real GPU consumption.
use super::*;
use crate::ids::{RuntimeEntityId, SourceEntityId};
use crate::render_contract::*;
use crate::render_projection_adapter::{CameraProjection, MeshProjection};
use crate::render_state::*;
use crate::runtime_renderer::{QualityProfile, RenderTarget};
#[allow(dead_code)]
#[path = "../../../engine_runtime/tests/support/geometry_fixture.rs"]
mod geometry_fixture;
#[allow(dead_code)]
#[path = "../../../engine_runtime/tests/support/streamed_cells.rs"]
mod streamed_cells;

fn scene(material: &str, x: f32) -> RenderSceneState {
    let mut scene = RenderSceneState::new();
    let mut view = RenderViewState::new(
        RenderViewId(1),
        RenderViewKind::Game,
        RenderTargetKind::ViewportTexture,
    );
    view.viewport.width = 64;
    view.viewport.height = 64;
    view.spatial = Some(CameraProjection {
        lights: Default::default(),
        view: RenderView::perspective(1, 64, 64, 60., 0.1, 10.),
        pose: Default::default(),
        suspended: false,
        origin: [0.; 3],
        coordinate_origin: [0.; 3],
    });
    scene.register_view(view);
    let mut matrix = [0.; 16];
    for i in [0, 5, 10, 15] {
        matrix[i] = 1.;
    }
    matrix[12] = x;
    matrix[14] = -2.;
    let projection = MeshProjection {
        presentation: None,
        deformation: None,
        runtime_entity_id: RuntimeEntityId::new(1, 1),
        component: crate::spatial_contract::MeshRenderer3D {
            schema_version: "mesh-renderer3d.v1".into(),
            mesh: crate::spatial_contract::SpatialAssetRef(streamed_cells::reference(
                "shared-mesh",
                "mesh",
            )),
            materials: [(
                "surface".into(),
                crate::spatial_contract::SpatialAssetRef(streamed_cells::reference(
                    material, "material",
                )),
            )]
            .into(),
            visible: true,
            layers: vec!["default".into()],
        },
        world: crate::world::SpatialWorldState {
            anchor: [0.; 3],
            world_matrix: matrix,
            previous_world_matrix: matrix,
            pose: Default::default(),
            enabled: true,
            authored_visible: true,
            inherited_visible: true,
            rigid: true,
            version: 1,
        },
        provenance: Provenance::unspecified(ProducerStage::RenderProjection),
        coordinate_origin: [0.; 3],
    };
    scene.insert_proxy(RenderProxy::from_descriptor(
        RenderProxyId(1),
        RuntimeEntityId::new(1, 1),
        SourceEntityId::new("f04-mesh"),
        crate::components::Transform::identity(),
        RenderProxyDescriptor {
            visible: true,
            layer: "default".into(),
            payload: RenderProxyPayload::Mesh3D(Box::new(projection)),
        },
    ));
    scene
}

#[test]
#[ignore = "requires a real GPU"]
fn foundation_f04_delayed_worker_keeps_three_packet_versions_after_logical_release() {
    delayed_worker_composition(false, 3, false);
}

#[test]
#[ignore = "requires a real GPU"]
fn foundation_f04_mesh_replacement_cell_unload_and_close_preserve_queued_packets() {
    delayed_worker_composition(true, 3, false);
}

#[test]
#[ignore = "requires a real GPU"]
fn foundation_f04_maximum_pending_gpu_work_rejects_then_drains_after_cell_and_worker_close() {
    delayed_worker_composition(true, 16, false);
}

#[test]
#[ignore = "requires a real GPU"]
fn foundation_f04_dual_32mib_cells_keep_three_versions_until_real_completion() {
    delayed_worker_composition(true, 3, true);
}

#[test]
#[ignore = "requires a real GPU"]
fn foundation_f04_dual_32mib_cells_at_maximum_lag_backpressure_then_retire() {
    delayed_worker_composition(true, 16, true);
}

fn upload_budget() -> crate::wgpu_backend::real::GeometryUploadBudget {
    crate::wgpu_backend::real::GeometryUploadBudget {
        resident_bytes: 2 * 1024 * 1024 * 1024,
        staging_bytes: 128 * 1024 * 1024,
        bytes_per_frame: 16 * 1024 * 1024,
        max_tickets: 128,
    }
}

fn delayed_worker_composition(cell_lifecycle: bool, submission_count: usize, full_cells: bool) {
    let fixture = if full_cells {
        streamed_cells::build_dual_32mib_cells()
    } else {
        streamed_cells::build(true)
    };
    let root = fixture.root.clone();
    let asset_loader = crate::runtime_asset_loader::RuntimeAssetLoader::new(
        &fixture.package.package_dir,
        fixture.package.runtime_asset_index.clone(),
        fixture.package.runtime_asset_mount_table.clone(),
    );
    let mut loader = crate::runtime_instance_loader::RuntimeInstanceLoader::new(asset_loader);
    assert!(loader.asset_loader_mut().mount_bundle("startup"));
    let mut dispatcher = RenderCommandDispatcher::dedicated_worker(RenderThreadConfig::default());
    let owner = dispatcher.resource_owner_for_fault_test();
    let mut resources = owner.lock().unwrap();
    let mut world = crate::world::World::new();
    let mut cell_backend = cell_lifecycle
        .then(|| crate::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap());
    let cells = cell_lifecycle.then(|| {
        use crate::runtime_instance_loader::{CellError, CellStatus};
        if full_cells {
            cell_backend
                .as_mut()
                .unwrap()
                .set_geometry_upload_budget(upload_budget())
                .unwrap();
        }
        let mut cells = Vec::new();
        let mut upload_frame = 0;
        for name in if full_cells {
            &["a", "b"][..]
        } else {
            &["a"][..]
        } {
            let ticket = loader
                .request_cell(streamed_cells::request(name, 1, true), &world)
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                match loader.prepare_cell(ticket, &world, 1, &mut resources) {
                    Ok(CellStatus::Prepared) => break,
                    Err(CellError::NotReady) => assert!(Instant::now() < deadline),
                    other => panic!("cell did not prepare: {other:?}"),
                }
                thread::yield_now();
            }
            let backend = cell_backend.as_mut().unwrap();
            loader.begin_cell_upload(ticket, &world, backend).unwrap();
            for attempt in 1..=64 {
                upload_frame += 1;
                backend.pump_geometry_uploads(upload_frame).unwrap();
                if let Some(token) = backend.last_gpu_submission() {
                    backend
                        .wait_gpu_submission(token, Duration::from_secs(5))
                        .unwrap();
                }
                if full_cells {
                    let uploads = backend.geometry_upload_snapshot();
                    assert!(uploads.frame_uploaded_bytes <= upload_budget().bytes_per_frame);
                    assert!(uploads.staging_bytes <= upload_budget().staging_bytes);
                }
                match loader.commit_cell_gpu(ticket, &mut world, backend) {
                    Ok(_) => break,
                    Err(CellError::NotReady) if attempt < 64 => (),
                    other => panic!("cell did not commit: {other:?}"),
                }
            }
            cells.push(ticket);
        }
        assert_eq!(world.entity_count(), if full_cells { 4 } else { 1 });
        if full_cells {
            assert_eq!(
                cell_backend
                    .as_ref()
                    .unwrap()
                    .geometry_residency()
                    .texture_bytes,
                64 * 1024 * 1024
            );
        }
        cells
    });
    let mut projected = scene("material-a", 0.);
    if cell_lifecycle {
        projected.remove_proxy(RenderProxyId(1)).unwrap();
        world.prepare_spatial_presentation(1, 1.).unwrap();
        let mut extract = crate::render_extract::RenderExtractContext::new();
        let merged = extract
            .extract_world_dirty(1, &mut world, &projected)
            .normalize_merge(&projected);
        assert!(crate::render_command::apply_batch(&mut projected, &merged).is_empty());
        assert_eq!(projected.proxies_len(), if full_cells { 4 } else { 1 });
    }
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let source = Arc::new(crate::render_thread::RenderFrameSourceIdentity {
        run_id: "f04-real-worker".into(),
        run_epoch: 1,
        session_id: "f04".into(),
        package_content_hash: None,
        world_epoch: 1,
    });
    let mut identities = Vec::new();
    let mut weak = Vec::new();
    let mut tickets = Vec::new();
    let mut lag = FrameLagController::default();
    for (i, (material, x, alpha)) in [
        ("material-a", -0.3, 0f32),
        ("material-b", 0., 0.5),
        ("material-a", 0.3, 0.75),
    ]
    .into_iter()
    .enumerate()
    {
        let frame = i as u64 + 1;
        let scene = if cell_lifecycle {
            let mesh =
                add_replacement_mesh(loader.asset_loader_mut(), &fixture.package.package_dir, i);
            let ids: Vec<_> = projected.proxies().map(|p| p.common.proxy_id).collect();
            for id in ids {
                let proxy = projected.proxy_mut(id).unwrap();
                let RenderProxyPayload::Mesh3D(p) = &mut proxy.payload else {
                    panic!("actual cell mesh expected")
                };
                p.component.mesh.0 = streamed_cells::reference(&mesh, "mesh");
                let material = if full_cells {
                    let original_b = p.world.world_matrix[12] > 0.;
                    let name = if original_b ^ (i == 1) { "b" } else { "a" };
                    let slot = usize::from(p.world.world_matrix[13] > 0.);
                    format!("material-{name}-{slot}")
                } else {
                    // Same placement isolates mesh replacement in red frames 1 and 3.
                    p.world.world_matrix[12] = 0.;
                    material.into()
                };
                p.component.materials.get_mut("surface").unwrap().0 =
                    streamed_cells::reference(&material, "material");
                p.world.previous_world_matrix = p.world.world_matrix;
                p.world.version = frame;
            }
            projected.clone()
        } else {
            scene(material, x)
        };
        let geometry = cache.snapshot(
            frame,
            &scene,
            "f04",
            Some(loader.asset_loader_mut()),
            &mut resources,
            None,
        );
        assert!(
            geometry.preparation_error().is_none(),
            "{:?}",
            geometry.preparation_error()
        );
        let identity = Arc::new(crate::render_thread::RenderFrameIdentity {
            source: source.clone(),
            frame_index: frame,
            tick_pair: [frame - 1, frame],
            presentation_alpha_bits: alpha.to_bits(),
            views: geometry.identity_views.clone(),
        });
        identities.push(identity.clone());
        weak.push(Arc::downgrade(&geometry));
        let packet = RenderFramePacket {
            frame_identity: Some(identity),
            geometry: Some(geometry),
            frame_index: frame,
            render_scene_state: scene,
            render_frame_report: None,
            resource_requests: vec![],
            resource_release_requests: vec![],
            aui_overlay: None,
            aui_composition: None,
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            game_view_presentation: None,
            view_id: Some(RenderViewId(1)),
            quality_profile: QualityProfile::default(),
            render_target: RenderTarget::headless_texture("main", 64, 64),
        };
        let (ticket, immediate) = dispatcher.submit_frame_output(packet);
        assert!(immediate.is_none());
        tickets.push(ticket);
        lag.update(frame, 0);
        assert_eq!(lag.should_wait(), frame == 3);
    }
    assert_eq!(lag.wait_target_frame(), Some(1));
    if let Some(cells) = cells {
        for cell in cells {
            loader
                .release_cell_gpu(
                    cell,
                    &mut world,
                    &mut resources,
                    4,
                    cell_backend.as_mut().unwrap(),
                )
                .unwrap();
            assert_eq!(
                loader.cell_status(cell).unwrap(),
                crate::runtime_instance_loader::CellStatus::Released
            );
        }
        assert_eq!(
            world.entity_count(),
            0,
            "actual cell must be unloaded before queued frames execute"
        );
    }
    cache.reset_world(4, &mut resources);
    assert!(weak.iter().all(|p| p.upgrade().is_some()));
    // Actual OS worker cannot prepare any frame while its original owner is locked.
    thread::sleep(Duration::from_millis(50));
    for ticket in &tickets {
        assert!(dispatcher.poll_submission_output(*ticket).is_none());
    }
    drop(resources);
    assert_eq!(
        dispatcher.flush(RenderFenceSyncDepth::RenderThread).status,
        RenderFenceStatus::Completed
    );
    // Retrieve in reverse order; worker execution remains FIFO.
    let mut outputs = Vec::new();
    for ticket in tickets.iter().rev() {
        let (output, report) = dispatcher.poll_submission_output(*ticket).unwrap();
        assert!(report.accepted, "{report:?}");
        assert_eq!(output.report.rdg_status, "ok", "{:?}", output.report);
        assert_eq!(
            output
                .renderer_output
                .rhi_command_plan
                .frame_identity
                .as_deref(),
            Some(identities[ticket.frame_index as usize - 1].as_ref())
        );
        outputs.push(output);
    }
    outputs.reverse();
    let gpu_lease_weak: Vec<_> = outputs
        .last()
        .unwrap()
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .filter_map(|command| command.geometry_payload())
        .filter_map(|payload| payload.geometry_draw())
        .map(|draw| Arc::downgrade(draw.native_resource_lease()))
        .collect();
    assert!(!gpu_lease_weak.is_empty());
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    eprintln!("F04 adapter {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut backend = crate::wgpu_backend::real::RealWgpuBackend::from_shared_device_queue(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        64,
        64,
        "f04-worker",
    );
    // Reuse the segmented upload owner for the original payload size. Directly
    // registering all 64 MiB would bypass the per-frame/staging contract.
    let full_upload = full_cells.then(|| {
        use crate::wgpu_backend::real::GeometryUploadState;
        backend.set_geometry_upload_budget(upload_budget()).unwrap();
        assert_eq!(outputs[0].geometry_textures.len(), 4);
        let ticket = backend
            .begin_geometry_upload(&[], &outputs[0].geometry_textures)
            .unwrap();
        for frame in 1..=64 {
            backend.pump_geometry_uploads(frame).unwrap();
            if let Some(token) = backend.last_gpu_submission() {
                backend
                    .wait_gpu_submission(token, Duration::from_secs(5))
                    .unwrap();
            }
            let uploads = backend.geometry_upload_snapshot();
            assert!(uploads.frame_uploaded_bytes <= upload_budget().bytes_per_frame);
            assert!(uploads.staging_bytes <= upload_budget().staging_bytes);
            if backend.geometry_upload_state(ticket).unwrap() == GeometryUploadState::Ready {
                assert_eq!(backend.geometry_residency().textures, 4);
                assert_eq!(backend.geometry_residency().texture_bytes, 64 * 1024 * 1024);
                backend.pump_geometry_uploads(frame + 1).unwrap();
                return ticket;
            }
        }
        panic!("full-sized textures never reached actual completion")
    });
    let uploaded = backend.gpu_lifecycle_stats();
    assert_eq!(uploaded.in_flight, 0);
    // Hold only the original owner's processing of callbacks already delivered
    // by the real queue. No fake completion and no claim that hardware is busy.
    let deferred = cell_lifecycle.then(|| backend.defer_gpu_completion_callbacks_for_test());
    let textures: Vec<_> = (0..submission_count)
        .map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("371 independent packet output"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        })
        .collect();
    for (i, texture) in textures.iter().enumerate() {
        let output = &outputs[i % outputs.len()];
        backend
            .prepare_geometry_textures(&output.geometry_textures)
            .unwrap();
        let report = backend.execute_plan_to_surface_view(
            &output.renderer_output.rhi_command_plan,
            &texture.create_view(&Default::default()),
        );
        assert!(report.diagnostics.is_empty(), "{report:?}");
    }
    if submission_count == 16 {
        let before = backend.gpu_lifecycle_stats();
        assert_eq!(before.submitted - uploaded.submitted, 16);
        assert_eq!(before.in_flight, 16);
        assert_eq!(before.completed, uploaded.completed);
        let refused = backend.execute_plan_to_surface_view(
            &outputs[0].renderer_output.rhi_command_plan,
            &textures[0].create_view(&Default::default()),
        );
        assert!(
            refused.diagnostics.iter().any(|d| {
                d.code == "wgpu.render_surface_failed" && d.message == "gpu.in_flight_backpressure"
            }),
            "maximum in-flight work must refuse before submitting: {refused:?}"
        );
        assert_eq!(backend.gpu_lifecycle_stats(), before);
    }
    let before_close = backend.gpu_lifecycle_stats();
    eprintln!("F04 before final wait lifecycle {before_close:?}");
    if cell_lifecycle {
        assert_eq!(before_close.in_flight, submission_count);
        assert_eq!(before_close.completed, uploaded.completed);
        let physical_mesh_bytes = backend.geometry_residency().physical_mesh_bytes;
        assert!(physical_mesh_bytes > 0);
        assert_eq!(dispatcher.shutdown().status, RenderFenceStatus::Completed);
        assert_eq!(
            backend.geometry_residency().physical_mesh_bytes,
            physical_mesh_bytes,
            "CPU shutdown must preserve native meshes used by unobserved GPU work"
        );
        assert_eq!(
            backend.gpu_lifecycle_stats().completed,
            before_close.completed,
            "CPU shutdown must not manufacture GPU completion"
        );
        // Remove every returned CPU plan. The actual final GPU submission must
        // now own its resource lease, independently of worker/packet retention.
        drop(std::mem::take(&mut outputs));
        assert!(gpu_lease_weak.iter().all(|lease| lease.upgrade().is_some()));
    }
    if let Some(ticket) = full_upload {
        backend.release_geometry_upload(ticket).unwrap();
        let retired = backend.gpu_lifecycle_stats();
        assert_eq!(retired.completed, uploaded.completed);
        assert_eq!(retired.in_flight, submission_count);
        assert!(retired.retiring_bytes >= 64 * 1024 * 1024);
        assert_eq!(backend.geometry_residency().textures, 0);
        assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    }
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("371 three packet readback"),
        size: submission_count as u64 * 64 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    for (i, texture) in textures.iter().enumerate() {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: i as u64 * 64 * 256,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
    }
    queue.submit(Some(encoder.finish()));
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        sender.send(r).unwrap();
    });
    device.poll(wgpu::PollType::wait()).unwrap();
    receiver.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let size = 64 * 256;
    let images: Vec<_> = mapped.chunks_exact(size).collect();
    for (i, image) in images.iter().enumerate() {
        if full_cells {
            // Every quadrant contains a different real 16 MiB texture. Swapped
            // material versions must remain tied to the corresponding packet.
            for (top, left) in [(true, true), (true, false), (false, true), (false, false)] {
                let red = left ^ (i % 3 == 1);
                let channel = if red { 0 } else { 2 };
                let count = image
                    .chunks_exact(4)
                    .enumerate()
                    .filter(|(pixel, p)| {
                        (*pixel / 64 < 32) == top
                            && (*pixel % 64 < 32) == left
                            && p[channel] > 40
                            && p[1] < 8
                            && p[2 - channel] < 8
                    })
                    .count();
                assert!(count > 5, "packet {i}, top={top}, left={left}: original textured triangle missing ({count})");
            }
            continue;
        }
        let color = if i % 3 == 1 { 2 } else { 0 };
        assert!(
            image
                .chunks_exact(4)
                .any(|p| p[color] > 240 && p[if color == 0 { 2 } else { 0 }] < 10),
            "packet {i} missing original material"
        );
    }
    assert_ne!(
        images[0], images[2],
        "original transform versions must differ"
    );
    drop(mapped);
    buffer.unmap();
    if let Some(deferred) = deferred {
        assert_eq!(backend.gpu_lifecycle_stats().completed, uploaded.completed);
        assert!(gpu_lease_weak.iter().all(|lease| lease.upgrade().is_some()));
        let callbacks = std::mem::take(&mut *deferred.lock().unwrap());
        assert_eq!(callbacks.len(), submission_count);
        for callback in callbacks {
            callback();
        }
        let completed = backend.poll_gpu_completion().unwrap();
        assert_eq!(
            completed.completed - uploaded.completed,
            submission_count as u64
        );
        assert_eq!(completed.in_flight, 0);
        assert_eq!(completed.retiring_bytes, 0);
        assert!(gpu_lease_weak.iter().all(|lease| lease.upgrade().is_none()));
    }
    backend.prepare_geometry_textures(&[]).unwrap();
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
    if !cell_lifecycle {
        assert_eq!(dispatcher.shutdown().status, RenderFenceStatus::Completed);
    }
    drop(outputs);
    drop(cache);
    drop(dispatcher);
    assert!(weak.iter().all(|p| p.upgrade().is_none()));
    let mut resources = owner.lock().unwrap();
    let final_resources = resources.end_frame(20);
    assert_eq!(final_resources.resident_bytes, 0);
    assert_eq!(final_resources.pending_release_count, 0);
    assert!(
        gpu_lease_weak.iter().all(|lease| lease.upgrade().is_none()),
        "all actual submit leases must retire after completion and CPU owner drop"
    );
    eprintln!("F04 final resource lifetime {:?}", resources.report(20));
    drop(resources);
    drop(owner);
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(5)));
    if let Some(backend) = &mut cell_backend {
        backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    }
    drop(cell_backend);
    drop(loader);
    drop(fixture);
    assert!(!root.exists());
}

// Publish distinct immutable cooked mesh receipts. Replacing a component's AssetRef
// is independent of same-GUID hot patch invalidation and preserves each old source.
#[test]
#[ignore = "explicit original BM10 package and real GPU; F12 resource-lock subcase only"]
fn bm11_original_bm10_full_queue_shutdown_releases_the_resource_lock_dependency() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal451 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    loop {
        frame += 1;
        assert!(
            frame <= 3000 && started.elapsed() < Duration::from_secs(180),
            "original BM10 setup did not reach its population before the bounded fault test"
        );
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "original BM10 setup fault: {:?}",
            output.project_runtime_session_report
        );
        let receipts = loader.project_cell_snapshot(&world).receipts;
        let all_active = receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|state| {
            state.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && state.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("original project must publish its actual population");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }
    let scene = host.render_scene().clone();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let resources = Arc::new(Mutex::new(RenderResourceManager::new()));
    let mut guard = resources.lock().unwrap();
    let geometry = cache.snapshot(
        frame,
        &scene,
        "bm11-original-bm10-f12",
        Some(loader.asset_loader_mut()),
        &mut guard,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    // The unchanged original nine-cell recipe contains 50K live instances.
    // Vegetation instances are not individual ECS entities. Count real retained
    // draw records here; this count can include material/LOD representations.
    let retained_draw_records = geometry.prepared_geometries().count();
    assert!(
        retained_draw_records >= 50_000,
        "the original workload must reach the actual snapshot: {retained_draw_records}"
    );
    let packet = RenderFramePacket {
        frame_identity: None,
        geometry: Some(geometry),
        frame_index: frame,
        render_scene_state: scene,
        render_frame_report: None,
        resource_requests: vec![],
        resource_release_requests: vec![],
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        view_id: None,
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::headless_texture("bm11-resource-lock", 128, 128),
    };
    let mut worker = RenderThreadWorker::spawn_with_resources(
        "bm11-original-bm10-resource-lock",
        RenderThreadConfig::default(),
        resources.clone(),
    );
    let fill_deadline = Instant::now() + Duration::from_secs(2);
    for sequence in 1..=5 {
        loop {
            match worker.submit_frame(
                packet.clone(),
                RenderSubmissionTicket {
                    frame_index: frame,
                    submit_sequence: sequence,
                },
            ) {
                Ok(()) => break,
                Err(d) => {
                    assert_eq!(d.code, "queue_full");
                    assert!(Instant::now() < fill_deadline);
                    std::thread::yield_now();
                }
            }
        }
    }
    // Five accepted packets proves one callback was taken and four remain queued.
    let rejected = worker
        .submit_frame(
            packet,
            RenderSubmissionTicket {
                frame_index: frame,
                submit_sequence: 6,
            },
        )
        .unwrap_err();
    assert_eq!(rejected.code, "queue_full");
    let shutdown_started = Instant::now();
    let shutdown = worker.shutdown(Duration::from_millis(5));
    assert_eq!(shutdown.status, RenderFenceStatus::Timeout);
    assert!(shutdown_started.elapsed() < Duration::from_millis(100));
    drop(guard);
    let join_deadline = Instant::now() + Duration::from_secs(10);
    while !worker.thread_handle.as_ref().unwrap().is_finished() {
        assert!(
            Instant::now() < join_deadline,
            "resource callback did not finish after releasing its actual lock"
        );
        std::thread::yield_now();
    }
    worker.thread_handle.take().unwrap().join().unwrap();
    eprintln!("BM11 F12 original BM10: frame={frame}, entities={}, retainedDrawRecords={retained_draw_records}, setup={:?}, held-lock shutdown={shutdown:?}; resource callback joined", world.entity_count(), started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

fn add_replacement_mesh(
    loader: &mut crate::runtime_asset_loader::RuntimeAssetLoader,
    root: &std::path::Path,
    version: usize,
) -> String {
    let mut record = loader
        .asset_index()
        .record_by_guid("shared-mesh")
        .unwrap()
        .clone();
    let mut cooked = loader
        .asset_index()
        .cooked_asset(&record.cooked_asset_id)
        .unwrap()
        .clone();
    let mut mesh = geometry_fixture::triangle();
    let scale = [1f32, 0.5, 0.75][version];
    for word in mesh.vertex_bytes.chunks_exact_mut(4) {
        let value = f32::from_le_bytes((&*word).try_into().unwrap()) * scale;
        word.copy_from_slice(&value.to_le_bytes());
    }
    let bytes = serde_json::to_vec(&RenderContract::new(CookedRenderAsset {
        source_digest: format!("sha256:{}", "1".repeat(64)),
        recipe: NATIVE_RENDER_COOK_RECIPE.into(),
        asset: RenderAssetData::Mesh(mesh),
    }))
    .unwrap();
    let id = format!("f04-mesh-{version}");
    record.asset_guid = id.clone();
    record.asset_id = id.clone();
    record.cooked_asset_id = format!("cooked-{id}");
    record.version = (version + 1).to_string();
    record.hash = Some(crate::canonical_digest::sha256_prefixed(&bytes));
    record.size = Some(bytes.len() as u64);
    cooked.cooked_asset_id = record.cooked_asset_id.clone();
    cooked.hash = record.hash.clone();
    cooked.size = record.size;
    cooked.path = Some(format!("cooked-assets/{id}.json"));
    std::fs::write(root.join(cooked.path.as_ref().unwrap()), bytes).unwrap();
    loader.mount_patch_index(crate::runtime_asset::RuntimeAssetIndex::new(
        vec![record],
        vec![cooked],
    ));
    id
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "explicit original BM10 package; F06 render worker failure with real in-flight and queued packets"]
fn bm11_original_bm10_render_worker_loss_releases_inflight_and_queued_packets() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal452 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    loop {
        frame += 1;
        assert!(
            frame <= 3000 && started.elapsed() < Duration::from_secs(180),
            "original BM10 setup did not reach its population before the bounded fault test"
        );
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "original BM10 setup fault: {:?}",
            output.project_runtime_session_report
        );
        let receipts = loader.project_cell_snapshot(&world).receipts;
        let all_active = receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|state| {
            state.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && state.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("original project must publish its actual population");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }
    let scene = host.render_scene().clone();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let resources = Arc::new(Mutex::new(RenderResourceManager::new()));
    let mut guard = resources.lock().unwrap();
    let geometry = cache.snapshot(
        frame,
        &scene,
        "bm11-original-bm10-f06-render",
        Some(loader.asset_loader_mut()),
        &mut guard,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    // The unchanged original nine-cell recipe contains 50K live instances.
    // Count real retained records; this count may include material/LOD representations.
    let retained_draw_records = geometry.prepared_geometries().count();
    assert!(
        retained_draw_records >= 50_000,
        "the original workload must reach the actual snapshot: {retained_draw_records}"
    );
    let packet = RenderFramePacket {
        frame_identity: None,
        geometry: Some(geometry),
        frame_index: frame,
        render_scene_state: scene,
        render_frame_report: None,
        resource_requests: vec![],
        resource_release_requests: vec![],
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        view_id: None,
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::headless_texture("bm11-render-loss", 128, 128),
    };
    let retained_snapshot = Arc::downgrade(packet.geometry.as_ref().unwrap());
    let baseline_refs = retained_snapshot.strong_count();
    let mut worker = RenderThreadWorker::spawn_with_resources(
        "bm11-original-bm10-render-loss",
        RenderThreadConfig::default(),
        resources.clone(),
    );
    let fill_deadline = Instant::now() + Duration::from_secs(2);
    for sequence in 1..=5 {
        loop {
            match worker.submit_frame(
                packet.clone(),
                RenderSubmissionTicket {
                    frame_index: frame,
                    submit_sequence: sequence,
                },
            ) {
                Ok(()) => break,
                Err(d) => {
                    assert_eq!(d.code, "queue_full");
                    assert!(Instant::now() < fill_deadline);
                    std::thread::yield_now();
                }
            }
        }
    }
    // Five accepted packets proves one callback was taken and four remain queued.
    let rejected = worker
        .submit_frame(
            packet.clone(),
            RenderSubmissionTicket {
                frame_index: frame,
                submit_sequence: 6,
            },
        )
        .unwrap_err();
    assert_eq!(rejected.code, "queue_full");
    assert_eq!(
        retained_snapshot.strong_count(),
        baseline_refs + 5,
        "one in-flight and four queued actual packets must hold the actual snapshot"
    );
    let failure_started = Instant::now();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _actual_resource_guard = guard;
            panic!("injected original BM10 render resource owner failure");
        }))
        .is_err()
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !worker.thread_handle.as_ref().unwrap().is_finished() {
        assert!(
            Instant::now() < deadline,
            "actual render worker did not enter a terminal state"
        );
        std::thread::yield_now();
    }
    assert!(
        worker.thread_handle.take().unwrap().join().is_err(),
        "the actual worker must have failed"
    );
    let report = worker.latest_report();
    assert_eq!(report.state, RenderWorkerState::WorkerLost);
    assert!(report.worker_lost);
    assert_eq!(report.last_completed_frame, 0);
    assert_eq!(report.last_submitted_frame, frame);
    for sequence in 1..=5 {
        assert!(
            worker
                .poll_submission_output(RenderSubmissionTicket {
                    frame_index: frame,
                    submit_sequence: sequence,
                })
                .is_none(),
            "an aborted packet cannot report a successful submission"
        );
    }
    assert_eq!(
        retained_snapshot.strong_count(),
        baseline_refs,
        "all five accepted packet snapshot leases must drop after actual worker failure"
    );
    assert_eq!(
        worker
            .submit_frame(
                packet.clone(),
                RenderSubmissionTicket {
                    frame_index: frame,
                    submit_sequence: 7,
                }
            )
            .unwrap_err()
            .code,
        "worker_stopped"
    );
    assert_eq!(
        worker.shutdown(Duration::from_secs(1)).status,
        RenderFenceStatus::WorkerLost
    );
    assert_eq!(retained_snapshot.strong_count(), baseline_refs);
    assert!(resources.is_poisoned());
    eprintln!("BM11 F06 actual original render worker failed with one packet in flight and four queued: WorkerLost, no completion/Present/GPU success, new admission closed, all five real snapshot owners returned to baseline={baseline_refs}; bounded={:?}",failure_started.elapsed());
    frame += 1;
    backend.pump_geometry_uploads(frame).unwrap();
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_fixed_step_count(1)
            .with_deferred_render_submission(),
        &mut world,
        RuntimeFrameContext {
            package: &package,
            instance_loader: &mut loader,
            cell_resources: None,
            cell_backend: Some(&mut backend),
        },
    );
    assert!(
        !output
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|r| r.terminal_fault),
        "{:?}",
        output.project_runtime_session_report
    );
    assert_eq!(world.entity_count(), 56_701);
    assert_eq!(
        loader
            .project_cell_snapshot(&world)
            .receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count(),
        9
    );
    let state = host.project_observation_state().unwrap();
    for key in ["k2.populationExact", "k2.allDomainsUpdated"] {
        assert_eq!(
            state.actual_value(key),
            Some(&ProjectObservationValue::Bool(true)),
            "{key}"
        );
    }
    eprintln!("BM11 original BM10 F06 render worker terminal: frame={frame} entities=56701 retainedDrawRecords={retained_draw_records} nineActive=9 populationExact/allDomainsUpdated=true; no timeout/recovery/device/full F06 claim; elapsed={:?}",failure_started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "explicit original BM10 package; actual render task timeout and late CPU completion"]
fn bm11_original_bm10_render_task_timeout_preserves_owners_until_actual_completion() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::frame_loop::RuntimeFrameContext;
    use crate::project_observation::{ProjectObservationValue, ProjectRuntimeObservationState};
    use crate::project_runtime_module::ProjectRuntimeBootstrap;
    use crate::project_runtime_native_adapter::linked_project_runtime_set_from_dll;
    use crate::project_runtime_session::ProjectRuntimeSessionReportLevel;
    use crate::runtime_asset_loader::StreamingBudget;
    use crate::runtime_instance_loader::RuntimeInstanceLoader;
    use crate::wgpu_backend::real::RealWgpuBackend;
    let package_path = std::path::PathBuf::from(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").expect("original normal452 package"),
    );
    let package = crate::runtime_package::load_runtime_package(&package_path)
        .value
        .unwrap();
    let linked = linked_project_runtime_set_from_dll(
        &package_path
            .parent()
            .unwrap()
            .join("bin/sample_371_bm10_k2_runtime.dll"),
    )
    .unwrap();
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .unwrap()
        .into_parts();
    let mut host = EngineHostLoop::with_project_runtime_session(
        package.active_scene.id.clone(),
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_project_runtime_session_report_level(ProjectRuntimeSessionReportLevel::Summary);
    assert!(host.set_external_rhi_enabled(true));
    let mut world = crate::world::World::new();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    loader
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_asset_bytes: 64 * 1024 * 1024,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let (_, report) = loader.load_active_scene_instance(&package, &mut world);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let started = Instant::now();
    let mut frame = 0;
    loop {
        frame += 1;
        assert!(
            frame <= 3000 && started.elapsed() < Duration::from_secs(180),
            "original BM10 setup did not reach its population before the bounded fault test"
        );
        backend.pump_geometry_uploads(frame).unwrap();
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(2))
                .unwrap();
        }
        let output = host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame)
                .with_fixed_step_count(1)
                .with_deferred_render_submission(),
            &mut world,
            RuntimeFrameContext {
                package: &package,
                instance_loader: &mut loader,
                cell_resources: None,
                cell_backend: Some(&mut backend),
            },
        );
        assert!(
            !output
                .project_runtime_session_report
                .as_ref()
                .is_some_and(|r| r.terminal_fault),
            "original BM10 setup fault: {:?}",
            output.project_runtime_session_report
        );
        let receipts = loader.project_cell_snapshot(&world).receipts;
        let all_active = receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count()
            == 9;
        let population_ready = host.project_observation_state().is_some_and(|state| {
            state.actual_value("k2.populationExact") == Some(&ProjectObservationValue::Bool(true))
                && state.actual_value("k2.allDomainsUpdated")
                    == Some(&ProjectObservationValue::Bool(true))
        });
        if all_active && population_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Some(ProjectRuntimeObservationState::Published { snapshot }) =
        host.project_observation_state()
    else {
        panic!("original project must publish its actual population");
    };
    for (name, count) in [
        ("k2.actors", 100),
        ("k2.dynamicBodies", 1000),
        ("k2.navAgents", 1000),
    ] {
        assert_eq!(
            snapshot.values.get(name),
            Some(&ProjectObservationValue::Integer(count)),
            "{name}"
        );
    }
    let scene = host.render_scene().clone();
    let mut cache = crate::render_geometry::ProductionGeometryCache::default();
    let mut dispatcher = RenderCommandDispatcher::dedicated_worker(RenderThreadConfig::default());
    let resources = dispatcher.resources.clone();
    let mut guard = resources.lock().unwrap();
    let geometry = cache.snapshot(
        frame,
        &scene,
        "bm11-original-bm10-f06-render",
        Some(loader.asset_loader_mut()),
        &mut guard,
        None,
    );
    assert!(
        geometry.preparation_error().is_none(),
        "{:?}",
        geometry.preparation_error()
    );
    // The unchanged original nine-cell recipe contains 50K live instances.
    // Count real retained records; this count may include material/LOD representations.
    let retained_draw_records = geometry.prepared_geometries().count();
    assert!(
        retained_draw_records >= 50_000,
        "the original workload must reach the actual snapshot: {retained_draw_records}"
    );
    let packet = RenderFramePacket {
        frame_identity: None,
        geometry: Some(geometry),
        frame_index: frame,
        render_scene_state: scene,
        render_frame_report: None,
        resource_requests: vec![],
        resource_release_requests: vec![],
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        view_id: None,
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::headless_texture("bm11-render-loss", 128, 128),
    };
    let retained_snapshot = Arc::downgrade(packet.geometry.as_ref().unwrap());
    let baseline_refs = retained_snapshot.strong_count();
    let failure_started = Instant::now();
    let (output, report) = dispatcher.submit_frame_output_and_wait(packet.clone());
    assert!(failure_started.elapsed() < Duration::from_secs(3));
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "render_worker_timeout"));
    assert!(!report.accepted && !report.submitted && !report.presented);
    assert_eq!(report.completed_frame_index, 0);
    assert_eq!(output.report.present_status, "not_presented");
    assert_eq!(
        retained_snapshot.strong_count(),
        baseline_refs + 1,
        "the actually blocked task must retain the real snapshot; output only retains texture bindings"
    );
    assert!(Arc::ptr_eq(
        &output.geometry_textures,
        &packet.geometry.as_ref().unwrap().textures
    ));
    let timeout_state = dispatcher.worker_report();
    assert_eq!(timeout_state.state, RenderWorkerState::ShuttingDown);
    assert_eq!(timeout_state.last_submitted_frame, frame);
    assert_eq!(timeout_state.last_completed_frame, 0);
    assert!(!dispatcher
        .worker
        .as_ref()
        .unwrap()
        .thread_handle
        .as_ref()
        .unwrap()
        .is_finished());
    let prepare_started = Instant::now();
    assert_eq!(
        dispatcher.preparation_error().unwrap().code,
        "render_worker_timeout"
    );
    assert!(prepare_started.elapsed() < Duration::from_millis(100));
    let (_, next) = dispatcher.submit_frame_output(packet.clone());
    let (next_output, next_report) = next.expect("timeout closes admission");
    assert!(!next_report.accepted && !next_report.submitted && !next_report.presented);
    assert!(next_report
        .diagnostics
        .iter()
        .any(|d| d.code == "worker_stopped"));
    drop(next_output);
    drop(output);
    assert_eq!(
        retained_snapshot.strong_count(),
        baseline_refs + 1,
        "timeout must not fabricate cancellation of the actually blocked task"
    );
    drop(guard);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dispatcher.worker_report().worker_lost {
        assert!(
            Instant::now() < deadline,
            "released task did not terminate within original bound"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        dispatcher.completed_frame_index(),
        frame,
        "the late CPU result must be real; no earlier completion was reported"
    );
    let (late_output, late_report) = dispatcher
        .poll_submission_output(RenderSubmissionTicket {
            frame_index: frame,
            submit_sequence: 1,
        })
        .expect("accepted task must leave an actual late result");
    assert_eq!(late_report.frame_index, frame);
    assert_eq!(late_report.completed_frame_index, frame);
    drop(late_output);
    assert_eq!(
        retained_snapshot.strong_count(),
        baseline_refs,
        "all accepted task and returned output snapshot leases must retire"
    );
    assert_eq!(dispatcher.shutdown().status, RenderFenceStatus::WorkerLost);
    if let Some(thread) = dispatcher.worker.as_mut().unwrap().thread_handle.take() {
        assert!(thread.is_finished());
        thread
            .join()
            .expect("timeout asks orderly teardown, not a worker panic");
    }
    assert!(!resources.is_poisoned());
    eprintln!("BM11 original BM10 F06 render task timeout: actual 2s dispatcher wait, no early CPU/GPU/Present success, admission closed, snapshot held until actual task exit, late CPU result consumed, actual thread joined, snapshot baseline={baseline_refs}; bounded={:?}",failure_started.elapsed());
    frame += 1;
    backend.pump_geometry_uploads(frame).unwrap();
    let output = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_fixed_step_count(1)
            .with_deferred_render_submission(),
        &mut world,
        RuntimeFrameContext {
            package: &package,
            instance_loader: &mut loader,
            cell_resources: None,
            cell_backend: Some(&mut backend),
        },
    );
    assert!(
        !output
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|r| r.terminal_fault),
        "{:?}",
        output.project_runtime_session_report
    );
    assert_eq!(world.entity_count(), 56_701);
    assert_eq!(
        loader
            .project_cell_snapshot(&world)
            .receipts
            .values()
            .filter(|r| r.status == project_runtime_sdk::ProjectRuntimeCellStatus::Active)
            .count(),
        9
    );
    let state = host.project_observation_state().unwrap();
    for key in ["k2.populationExact", "k2.allDomainsUpdated"] {
        assert_eq!(
            state.actual_value(key),
            Some(&ProjectObservationValue::Bool(true)),
            "{key}"
        );
    }
    eprintln!("BM11 original BM10 F06 render worker terminal: frame={frame} entities=56701 retainedDrawRecords={retained_draw_records} nineActive=9 populationExact/allDomainsUpdated=true; task timeout only; no recovery/device/full F06 claim; elapsed={:?}",failure_started.elapsed());
    assert!(loader
        .asset_loader_mut()
        .shutdown_streaming(Duration::from_secs(2)));
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}
