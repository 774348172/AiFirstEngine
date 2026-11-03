//! Real native query/indirect readback, intentionally run serially by the owner.
use super::*;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

fn plan() -> RhiCommandPlan {
    let graph = crate::rhi_command_plan::tests::retained_geometry_graph(32);
    let mut plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    let geometry:Arc<GeometryBuffers>=Arc::new(serde_json::from_value(serde_json::json!({
        "vertexBytes":([-0.8f32,-0.8,0.,0.8,-0.8,0.,-0.8,0.8,0.,0.8,0.8,0.].into_iter().flat_map(f32::to_le_bytes).collect::<Vec<_>>()),
        "indexBytes":[0,0,1,0,2,0,2,0,1,0,3,0],"vertexStride":12,"vertexCount":4,"indexCount":6,
        "indexFormat":"uint16","attributes":[{"semantic":"position","format":"float32x3","offset":0}]
    })).unwrap());
    let mut view = plan
        .commands
        .iter()
        .find_map(geometry_draw)
        .unwrap()
        .view
        .as_ref()
        .clone();
    view.depth_policy = DepthPolicy::TestAndWrite;
    // Ordinary typed World3D views clear depth in encode_geometry_begin;
    // depth_clear is reserved by the executable contract for shadow maps.
    view.depth_clear = None;
    let view = Arc::new(view);
    let mut ordinal = 0;
    for command in plan.commands.iter_mut() {
        match command {
            RhiCommand::BeginView {
                view: declared,
                depth_target,
                ..
            } => {
                *declared = view.clone();
                *depth_target = Some("scale-depth".into());
            }
            RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                vertex_count,
                ..
            } => {
                let draw = Arc::make_mut(draw);
                draw.view = view.clone();
                draw.depth_target = Some("scale-depth".into());
                draw.depth = MaterialDepth::TestAndWrite;
                draw.geometry = geometry.clone();
                draw.index_count = 6;
                // RHI retains the indexed draw count separately from payload.
                // Changing the real quad requires both ordinary contract fields.
                *vertex_count = 6;
                // Distinct ordinary mesh handles keep hidden objects in their
                // own real query batches instead of one visible instanced batch.
                let binding = Arc::make_mut(&mut draw.geometry_binding);
                let crate::render_contract::BindingResource::Geometry(handle) =
                    &mut binding.slots[0].resource
                else {
                    panic!("mesh binding required")
                };
                *handle = crate::render_contract::GeometryHandle::try_from(
                    crate::render_resource::RenderResourceHandle {
                        index: ordinal as u64 + 1,
                        ..handle.raw()
                    },
                )
                .unwrap();
                draw.world_transform[14] = if ordinal == 0 { -2f32 } else { -3f32 }.into();
                draw.base_color = crate::render_graph::color(if ordinal == 0 {
                    [1., 0., 0., 1.]
                } else {
                    [0., 1., 0., 1.]
                });
                ordinal += 1;
            }
            _ => {}
        }
    }
    assert_eq!(ordinal, 32);
    plan
}

fn target(backend: &mut RealWgpuBackend) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = backend.create_output_texture(WIDTH, HEIGHT).unwrap();
    let view = texture.create_view(&Default::default());
    (texture, view)
}

fn render(
    backend: &mut RealWgpuBackend,
    plan: &RhiCommandPlan,
    texture: &wgpu::Texture,
    view: &wgpu::TextureView,
) -> Vec<u8> {
    backend.render_to_view(plan, view, None).unwrap();
    backend.read_texture_rgba_bytes(texture).unwrap()
}

fn read_buffer(backend: &RealWgpuBackend, buffer: &wgpu::Buffer) -> Vec<u32> {
    let readback = backend.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-scale-test-readback"),
        size: buffer.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = backend.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(buffer, 0, &readback, 0, buffer.size());
    backend.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    backend.device.poll(wgpu::PollType::wait()).unwrap();
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let result = bytemuck::cast_slice::<u8, u32>(&readback.slice(..).get_mapped_range()).to_vec();
    readback.unmap();
    result
}

fn arguments(backend: &RealWgpuBackend) -> Vec<[u32; 5]> {
    backend
        .geometry
        .gpu_scale
        .ranges
        .values()
        .flat_map(|r| {
            read_buffer(backend, &r.arguments)
                .chunks_exact(5)
                .map(|c| c.try_into().unwrap())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
#[ignore = "requires a real GPU; owner runs serially"]
fn gpu_scale_real_occlusion_indirect_matches_cpu_and_invalidates_camera_and_occluder() {
    let mut backend = RealWgpuBackend::new_offscreen(WIDTH, HEIGHT).unwrap();
    assert!(GpuScaleState::supported(&backend.device));
    let (texture, view) = target(&mut backend);
    let original = plan();
    backend.set_geometry_gpu_scale_enabled(false);
    let cpu = render(&mut backend, &original, &texture, &view);
    assert!(
        cpu.chunks_exact(4).any(|p| p[0] > 200 && p[1] < 10),
        "occluder must be visible red pixels"
    );
    backend.set_geometry_gpu_scale_enabled(true);
    let warm = render(&mut backend, &original, &texture, &view);
    assert_eq!(cpu, warm);
    let indirect = render(&mut backend, &original, &texture, &view);
    assert_eq!(cpu, indirect);
    assert!(
        backend.geometry_gpu_scale_counts()[0] > 0 && backend.geometry_gpu_scale_counts()[1] > 0
    );
    let args = arguments(&backend);
    assert!(
        args.iter().any(|a| a[1] == 0),
        "real query must suppress an occluded draw"
    );
    assert!(
        args.iter().any(|a| a[1] > 0),
        "real visible occluder must survive"
    );
    assert_eq!(
        args[1][1], 0,
        "the second draw starts wholly behind the occluder"
    );
    let mut moved = original.clone();
    for command in moved.commands.iter_mut() {
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
    let direct_before = backend.geometry_gpu_scale_counts()[0];
    let changed = render(&mut backend, &moved, &texture, &view);
    assert!(
        backend.geometry_gpu_scale_counts()[0] > direct_before,
        "changed camera must get real fresh queries"
    );
    backend.set_geometry_gpu_scale_enabled(false);
    assert_eq!(changed, render(&mut backend, &moved, &texture, &view));
    backend.set_geometry_gpu_scale_enabled(true);
    let mut opened = original.clone();
    let first = opened
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
    Arc::make_mut(first).world_transform[12] = 3f32.into();
    let fresh = render(&mut backend, &opened, &texture, &view);
    let second = render(&mut backend, &opened, &texture, &view);
    assert_eq!(fresh, second);
    assert!(
        fresh.chunks_exact(4).any(|p| p[1] > 200 && p[0] < 10),
        "previously hidden green geometry must restore"
    );
    backend.set_geometry_gpu_scale_enabled(false);
    assert_eq!(fresh, render(&mut backend, &opened, &texture, &view));
    assert!(
        arguments(&backend)[1][1] > 0,
        "moving the occluder must restore a formerly zero argument"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU; owner runs serially"]
fn gpu_scale_budget_shortage_falls_back_to_same_cpu_pixels() {
    let mut backend = RealWgpuBackend::new_offscreen(WIDTH, HEIGHT).unwrap();
    assert!(GpuScaleState::supported(&backend.device));
    let (texture, view) = target(&mut backend);
    let plan = plan();
    backend.set_geometry_gpu_scale_enabled(false);
    let cpu = render(&mut backend, &plan, &texture, &view);
    backend
        .set_geometry_visual_resident_limit(backend.geometry_backing_bytes())
        .unwrap();
    backend.set_geometry_gpu_scale_enabled(true);
    let before = backend.geometry_gpu_scale_counts();
    backend.render_to_view(&plan, &view, None).unwrap();
    assert_eq!(backend.geometry_gpu_scale_counts()[0], before[0]);
    assert!(backend.geometry_gpu_scale_counts()[2] > before[2]);
    assert!(backend.geometry.gpu_scale.ranges.is_empty());
    // Readback has its own admitted staging allocation. Raise the test-only
    // sublimit after the bounded rendering verdict, before inspecting pixels.
    backend
        .set_geometry_visual_resident_limit(super::MAX_VISUAL_RESIDENT_BYTES)
        .unwrap();
    assert_eq!(cpu, backend.read_texture_rgba_bytes(&texture).unwrap());
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU; owner runs serially"]
fn gpu_scale_device_without_optional_feature_uses_cpu_fallback() {
    let instance = wgpu::Instance::new(&Default::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    assert!(!device
        .features()
        .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE));
    let mut backend = RealWgpuBackend::from_shared_device_queue(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        WIDTH,
        HEIGHT,
        "gpu scale no feature oracle",
    );
    let (texture, view) = target(&mut backend);
    let plan = plan();
    backend.set_geometry_gpu_scale_enabled(false);
    let cpu = render(&mut backend, &plan, &texture, &view);
    backend.set_geometry_gpu_scale_enabled(true);
    assert_eq!(cpu, render(&mut backend, &plan, &texture, &view));
    assert!(backend.geometry.gpu_scale.ranges.is_empty());
    assert_eq!(backend.geometry_gpu_scale_counts()[0], 0);
    assert!(backend.geometry_gpu_scale_counts()[2] > 0);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU; owner runs serially"]
fn gpu_scale_encoder_abort_before_first_submit_keeps_all_seed_instances_visible() {
    let mut backend = RealWgpuBackend::new_offscreen(WIDTH, HEIGHT).unwrap();
    assert!(GpuScaleState::supported(&backend.device));
    let (texture, view) = target(&mut backend);
    let plan = plan();
    backend.prepare_geometry_plan(&plan, &view, None).unwrap();
    let first = plan
        .commands
        .iter()
        .position(|c| geometry_draw(c).is_some())
        .unwrap();
    let end = first
        + plan.commands[first..]
            .iter()
            .take_while(|c| geometry_draw(c).is_some())
            .count();
    let draw = geometry_draw(&plan.commands[first]).unwrap();
    let viewport = geometry_viewport(&draw.view, WIDTH, HEIGHT, None).unwrap();
    let mut encoder = backend.device.create_command_encoder(&Default::default());
    backend
        .encode_geometry_begin(
            &mut encoder,
            &view,
            &draw.view,
            draw.depth_target.as_deref(),
            None,
        )
        .unwrap();
    assert!(backend
        .try_encode_gpu_scale(&mut encoder, &view, &plan.commands[first..end], 0, viewport)
        .unwrap());
    drop(encoder); // No queue submission of query resolve or draw commands.
    for range in backend.geometry.gpu_scale.ranges.values() {
        assert!(read_buffer(&backend, &range.results)
            .chunks_exact(2)
            .all(|hit| hit == [1, 0]));
    }
    let recovered = render(&mut backend, &plan, &texture, &view);
    assert!(
        arguments(&backend).iter().all(|args| args[1] > 0),
        "unsubmitted query may not produce false-hidden seeds"
    );
    backend.set_geometry_gpu_scale_enabled(false);
    assert_eq!(recovered, render(&mut backend, &plan, &texture, &view));
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
