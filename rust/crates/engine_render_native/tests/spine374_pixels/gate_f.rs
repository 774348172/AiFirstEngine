use super::*;
use engine_runtime::engine_rhi::{EngineRhiBackend, RhiBackendDiagnosticSeverity};
use std::{sync::Arc, time::Duration};

fn measurement_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Vec<u8> {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("measurement-test-readback"),
        size: u64::from(W) * u64::from(H) * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
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
                offset: 0,
                bytes_per_row: Some(W * 4),
                rows_per_image: Some(H),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        device.poll(wgpu::PollType::Poll).unwrap();
        if let Ok(r) = rx.try_recv() {
            r.unwrap();
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let bytes = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    bytes
}

#[test]
#[ignore = "requires real GPU timestamp queries"]
fn spine374_f_gpu_measurement_excludes_interleaved_ordinary_draws_and_reuses_exact_ranges() {
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    let source = checked(&region("normal"), ATLAS);
    let mut pose = source.load().unwrap().instance().unwrap();
    pose.step(0.).unwrap();
    let mut manager = RenderResourceManager::new();
    let identity = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let (frame, textures) = prepare_frame(
        &pose.geometry(1.).unwrap(),
        &pages(&source),
        &payloads(&source, &[[255, 128, 64, 128].repeat(4)]),
        identity,
        &mut manager,
    );
    let mut plan =
        engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&geometry_graph(&frame));
    let mut commands = Vec::new();
    for command in &plan.commands {
        if let Some(draw) = command.geometry_draw() {
            for (index, component) in ["SpineAnimator", "MeshRenderer", "SpineAnimator"]
                .into_iter()
                .enumerate()
            {
                let mut draw = draw.as_ref().clone();
                draw.instance_id += index as u64;
                Arc::make_mut(&mut draw.provenance).source_component = Some(component.into());
                commands.push(RhiCommand::Draw {
                    target: draw.target.clone(),
                    draw_kind: engine_runtime::rhi_command_plan::RhiDrawKind::Geometry,
                    vertex_count: draw.index_count,
                    payload: RhiDrawPayload::Geometry {
                        draw: Arc::new(draw),
                    },
                });
            }
        } else {
            commands.push(command.clone());
        }
    }
    plan.commands = commands.into();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: engine_render_native::gpu_frame_measurement::features(
            adapter.features(),
            true,
        ),
        required_limits: engine_render_native::particle_gpu::renderer_device_limits(
            adapter.limits(),
        ),
        ..Default::default()
    }))
    .unwrap();
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("spine-measurement-output"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = output.create_view(&Default::default());
    let mut backend = RealWgpuBackend::from_device_queue(
        device.clone(),
        queue.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
        W,
        H,
        format!("{:?}", info.backend),
    );
    backend.prepare_geometry_textures(&textures).unwrap();
    // Populate the full-range cache before enabling measurement. Then replace
    // it by source spans and finally return to the original unmeasured range.
    for i in 1..=3 {
        plan.frame_index = i;
        assert!(!backend
            .execute_plan_to_surface_view(&plan, &view)
            .diagnostics
            .iter()
            .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error));
    }
    let expected = measurement_pixels(&device, &queue, &output);
    backend.enable_frame_measurement(&info, 0, 3).unwrap();
    for i in 4..=6 {
        plan.frame_index = i;
        let result = backend.execute_plan_to_surface_view(&plan, &view);
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error),
            "{:?}",
            result.diagnostics
        );
    }
    let gpu = backend.finish_frame_measurement().unwrap();
    assert_eq!(
        measurement_pixels(&device, &queue, &output),
        expected,
        "measurement split changed actual translucent pixels"
    );
    assert_eq!(gpu.status, "complete");
    assert_eq!(gpu.samples.len(), 3);
    for sample in &gpu.samples {
        let value = serde_json::to_value(sample).unwrap();
        assert!(
            value["spineDrawMs"]
                .as_f64()
                .is_some_and(|ms| ms > 0. && ms <= sample.frame_ms),
            "missing real Spine timestamp interval: {value}"
        );
        assert_eq!(
            value["spineDrawCount"], 2,
            "ordinary draw entered Spine range"
        );
        assert_eq!(value["spineInstances"], 2);
        assert_eq!(sample.particle_draw_ms, 0.);
    }
    assert_eq!(
        backend.retained_geometry_stats().draws,
        12,
        "cached full range was replayed in a split interval"
    );
    for i in 7..=9 {
        plan.frame_index = i;
        assert!(!backend
            .execute_plan_to_surface_view(&plan, &view)
            .diagnostics
            .iter()
            .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error));
    }
    assert_eq!(
        backend.retained_geometry_stats().draws,
        15,
        "unmeasured cache reused a truncated measured span"
    );
    assert_eq!(
        measurement_pixels(&device, &queue, &output),
        expected,
        "returning to Off reused a truncated bundle"
    );
    // A legal mixed-source batch is one inseparable native draw. It must not
    // silently count all its instances as Spine or report a false zero budget.
    let head = plan
        .commands
        .iter()
        .find_map(RhiCommand::geometry_draw)
        .unwrap()
        .clone();
    let mut other = head.native_instance().as_ref().clone();
    other.instance_id += 100;
    Arc::make_mut(&mut other.provenance).source_component = Some("MeshRenderer".into());
    let batch = engine_runtime::render_geometry::GeometryBatch::new(
        head.clone(),
        Arc::new(vec![head.native_instance().clone(), Arc::new(other)]),
        vec![
            engine_runtime::render_geometry::GeometryVisibleInstance {
                instance_index: 0,
                camera_depth: head.camera_depth,
            },
            engine_runtime::render_geometry::GeometryVisibleInstance {
                instance_index: 1,
                camera_depth: head.camera_depth,
            },
        ],
    );
    batch.validate().unwrap();
    let mut mixed = plan.clone();
    let mut replaced = false;
    mixed.commands = plan
        .commands
        .iter()
        .filter_map(|c| {
            if c.geometry_draw().is_some() {
                if replaced {
                    return None;
                }
                replaced = true;
                return Some(RhiCommand::Draw {
                    target: head.target.clone(),
                    draw_kind: engine_runtime::rhi_command_plan::RhiDrawKind::Geometry,
                    vertex_count: head.index_count,
                    payload: RhiDrawPayload::GeometryBatch {
                        batch: Arc::new(batch.clone()),
                    },
                });
            }
            Some(c.clone())
        })
        .collect::<Vec<_>>()
        .into();
    backend.enable_frame_measurement(&info, 0, 1).unwrap();
    assert!(!backend
        .execute_plan(&mixed)
        .diagnostics
        .iter()
        .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error));
    let mixed = backend.finish_frame_measurement().unwrap();
    assert_eq!(mixed.status, "complete");
    assert_eq!(mixed.samples[0].spine_mixed_source_batches, 1);
    assert_eq!(mixed.samples[0].spine_draw_count, 0);
    assert_eq!(mixed.samples[0].spine_draw_ms, 0.);
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}

#[test]
#[ignore = "bounded dynamic mesh GPU allocation diagnosis, not qualification"]
fn spine374_f_dynamic_mesh_gpu_allocation_report() {
    let source = checked(
        &serde_json::from_str(include_str!(
            "../../../engine_spine_runtime/tests/fixtures/recipe-single.json"
        ))
        .unwrap(),
        ATLAS,
    );
    let asset = source.load().unwrap();
    let mut animator = asset.instance().unwrap();
    animator.play(0, "clip0", None, 0., true, false).unwrap();
    let texture = payloads(&source, &[vec![255; 16]]);
    let mut manager = RenderResourceManager::new();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("374-memory-owner"),
        required_features: wgpu::Features::empty(),
        required_limits: engine_render_native::particle_gpu::renderer_device_limits(
            adapter.limits(),
        ),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .unwrap();
    let mut backend = RealWgpuBackend::from_device_queue(
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        W,
        H,
        format!("{:?}", adapter.get_info().backend),
    );
    let identity = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let mut samples = Vec::new();
    let started = std::time::Instant::now();
    for tick in 1..=1200 {
        animator.step(1. / 60.).unwrap();
        let geometry = animator.geometry(100.).unwrap();
        let (frame, textures) =
            prepare_frame(&geometry, &pages(&source), &texture, identity, &mut manager);
        let handles = frame
            .views
            .iter()
            .flat_map(|v| &v.draws)
            .flat_map(|d| &d.geometry_binding.slots)
            .map(|s| s.resource.raw())
            .collect::<Vec<_>>();
        let mut graph = geometry_graph(&frame);
        graph.frame_index = tick;
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors());
        backend.prepare_geometry_textures(&textures).unwrap();
        let submitted = backend.execute_plan(&plan);
        assert!(
            !submitted
                .diagnostics
                .iter()
                .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error),
            "{:?}",
            submitted.diagnostics
        );
        backend
            .wait_gpu_submission(
                backend.last_gpu_submission().unwrap(),
                Duration::from_secs(5),
            )
            .unwrap();
        drop((plan, graph, frame, textures));
        for handle in handles {
            manager
                .release_resource(RenderResourceReleaseRequest {
                    handle,
                    frame_index: tick,
                })
                .unwrap();
        }
        manager.end_frame(tick);
        if tick % 200 == 0 {
            samples.push(json!({"tick":tick,"privateBytes":engine_runtime::windowed_player::sample_player_process_memory().map(|m|m.private_commit_bytes),
                "wgpu":format!("{:?}", instance.generate_report()),"residency":format!("{:?}",backend.geometry_residency()),
                "lifecycle":format!("{:?}", backend.gpu_lifecycle_stats()),"stats":format!("{:?}",backend.retained_geometry_stats())}));
        }
        assert!(started.elapsed().as_secs() < 90);
    }
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    drop(backend);
    samples.push(json!({"phase":"backend-dropped","wgpu":format!("{:?}",instance.generate_report()),"privateBytes":engine_runtime::windowed_player::sample_player_process_memory().map(|m|m.private_commit_bytes)}));
    let evidence = std::env::var_os("SPINE374_EVIDENCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("spine374-memory-{}", std::process::id()))
        });
    fs::create_dir_all(&evidence).unwrap();
    fs::write(
        evidence.join("dynamic-gpu-allocations.json"),
        serde_json::to_vec_pretty(&samples).unwrap(),
    )
    .unwrap();
    assert_eq!(
        instance
            .generate_report()
            .unwrap()
            .hub
            .render_bundles
            .num_allocated,
        0,
        "completed dynamic recordings leaked after the backend was dropped"
    );
}

#[test]
#[ignore = "requires real GPU submission and completion"]
fn spine374_f_dynamic_geometry_lease_survives_unload_until_gpu_completion() {
    let source = checked(
        &serde_json::from_str(include_str!(
            "../../../engine_spine_runtime/tests/fixtures/probe.json"
        ))
        .unwrap(),
        ATLAS,
    );
    let asset = source.load().unwrap();
    let mut instance = asset.instance().unwrap();
    instance.play(0, "motion", None, 0., true, false).unwrap();
    instance.step(0.2).unwrap();
    let first = instance.geometry(100.).unwrap();
    instance.step(0.2).unwrap();
    let second = instance.geometry(100.).unwrap();
    assert_ne!(
        first.vertices, second.vertices,
        "fixture must upload deforming geometry"
    );
    let texture = payloads(&source, &[vec![255; 16]]);
    let mut manager = RenderResourceManager::new();
    let transform = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let (frame, textures) =
        prepare_frame(&first, &pages(&source), &texture, transform, &mut manager);
    let frozen = frame.views[0].draws[0].geometry.vertex_bytes().to_vec();
    let weak = Arc::downgrade(frame.views[0].draws[0].native_resource_lease());
    let plan =
        engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&geometry_graph(&frame));
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let draw = &frame.views[0].draws[0];
    let handles = draw
        .geometry_binding
        .slots
        .iter()
        .chain(&draw.material_binding.slots)
        .map(|slot| slot.resource.raw())
        .collect::<Vec<_>>();
    let mut backend = RealWgpuBackend::new_offscreen(W, H).unwrap();
    backend.prepare_geometry_textures(&textures).unwrap();
    let submitted = backend.execute_plan(&plan);
    assert!(
        !submitted
            .diagnostics
            .iter()
            .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error),
        "{:?}",
        submitted.diagnostics
    );
    let token = backend.last_gpu_submission().unwrap();
    for handle in &handles {
        manager
            .release_resource(RenderResourceReleaseRequest {
                handle: *handle,
                frame_index: 1,
            })
            .unwrap();
    }
    assert_eq!(frame.views[0].draws[0].geometry.vertex_bytes(), frozen);
    drop((plan, frame, textures));
    assert!(
        weak.upgrade().is_some(),
        "submitted Spine packet lost its lease"
    );
    manager.end_frame(100);
    for handle in &handles {
        assert_eq!(
            manager.record(*handle).unwrap().state,
            RenderResourceState::PendingRelease
        );
    }
    let (next, next_textures) =
        prepare_frame(&second, &pages(&source), &texture, transform, &mut manager);
    assert_ne!(next.views[0].draws[0].geometry.vertex_bytes(), frozen);
    let next_handle = next.views[0].draws[0].geometry_binding.slots[0]
        .resource
        .raw();
    assert!(
        !handles.contains(&next_handle),
        "in-flight mesh handle was reused for changed vertices"
    );
    let next_plan =
        engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&geometry_graph(&next));
    backend.prepare_geometry_textures(&next_textures).unwrap();
    let submitted = backend.execute_plan(&next_plan);
    assert!(
        !submitted
            .diagnostics
            .iter()
            .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error),
        "{:?}",
        submitted.diagnostics
    );
    backend
        .wait_gpu_submission(token, Duration::from_secs(5))
        .unwrap();
    assert!(
        weak.upgrade().is_none(),
        "completed Spine packet retained the CPU lease"
    );
    manager.end_frame(101);
    for handle in &handles {
        assert_eq!(
            manager.record(*handle).unwrap().state,
            RenderResourceState::Released
        );
    }
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
}
