#![cfg(feature = "real-wgpu")]
use engine_runtime::render_asset_production::*;
use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_resource::*;
use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;

#[path = "support/geometry_fixture.rs"]
mod geometry_fixture;
#[path = "support/standard_lit_fixture.rs"]
mod standard_lit_fixture;
use geometry_fixture::*;
use standard_lit_fixture::*;

use engine_runtime::wgpu_backend::real::RealWgpuBackend;

#[test]
#[ignore = "S2-D real GPU block uploads"]
fn s2d_bc3_segmented_blocks_obey_all_limits_share_pins_and_retire() {
    use engine_runtime::runtime_texture::RuntimeTexturePayload;
    use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, GeometryUploadState};
    let mut manager = RenderResourceManager::new();
    // Independent BC3 red block: alpha endpoint 255, RGB565 endpoint 0xf800, indices zero.
    let block = [255, 0, 0, 0, 0, 0, 0, 0, 0, 248, 0, 0, 0, 0, 0, 0];
    let payload = RuntimeTexturePayload {
        asset_id: "bc3-stream".into(),
        cooked_asset_id: "bc3-stream.cooked".into(),
        width: 8,
        height: 4,
        format: "bc3RgbaUnormSrgb".into(),
        color_space: "srgb".into(),
        mip_count: 4,
        rgba8: block.repeat(5).into(),
        sampler: "nearestMipmapNearest:nearest:clampToEdge:clampToEdge".into(),
        source_hash: "independent-bc3".into(),
    };
    let prepared = RuntimeRenderAssetProducer::new()
        .prepare_texture_payload(0, payload, RenderBindingKind::Texture, &mut manager)
        .unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let budget = |bytes| GeometryUploadBudget {
        resident_bytes: 80,
        staging_bytes: 16,
        bytes_per_frame: bytes,
        max_tickets: 4,
    };
    backend.set_geometry_upload_budget(budget(8)).unwrap();
    assert!(backend
        .begin_geometry_upload(&[], std::slice::from_ref(&prepared))
        .unwrap_err()
        .contains("block_budget"));
    assert_eq!(backend.geometry_upload_snapshot().pending_resources, 0);
    backend.set_geometry_upload_budget(budget(16)).unwrap();
    let a = backend
        .begin_geometry_upload(&[], std::slice::from_ref(&prepared))
        .unwrap();
    let b = backend
        .begin_geometry_upload(&[], std::slice::from_ref(&prepared))
        .unwrap();
    let mut total = 0;
    for frame in 1..=5 {
        let n = backend.pump_geometry_uploads(frame).unwrap();
        let again = backend.pump_geometry_uploads(frame).unwrap();
        assert_eq!(n + again, 16);
        total += n + again;
        assert!(backend.geometry_upload_snapshot().staging_bytes <= 16);
        backend
            .wait_gpu_submission(
                backend.last_gpu_submission().unwrap(),
                std::time::Duration::from_secs(5),
            )
            .unwrap();
    }
    assert_eq!(total, 80);
    assert_eq!(backend.geometry_upload_snapshot().uploaded_bytes, 80);
    assert_eq!(
        backend.geometry_upload_state(a).unwrap(),
        GeometryUploadState::Ready
    );
    backend.release_geometry_upload(a).unwrap();
    assert_eq!(
        backend.geometry_upload_state(b).unwrap(),
        GeometryUploadState::Ready
    );
    backend.release_geometry_upload(b).unwrap();
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    let cancelled = backend
        .begin_geometry_upload(&[], std::slice::from_ref(&prepared))
        .unwrap();
    assert_eq!(backend.pump_geometry_uploads(6).unwrap(), 16);
    backend.release_geometry_upload(cancelled).unwrap();
    assert_eq!(backend.geometry_upload_snapshot().pending_resources, 0);
    assert!(backend.geometry_upload_state(cancelled).is_err());
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
}

#[test]
#[ignore = "S2-D direct block upload and failed replacement pixels"]
fn s2d_bc3_direct_upload_samples_blocks_and_preserves_failed_replacement() {
    use engine_runtime::render_graph::*;
    use engine_runtime::runtime_texture::{runtime_texture_render_handle, RuntimeTexturePayload};
    let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
    let handle = runtime_texture_render_handle("bc3-direct");
    let red = [255, 0, 0, 0, 0, 0, 0, 0, 0, 248, 0, 0, 0, 0, 0, 0];
    let mut payload = RuntimeTexturePayload {
        asset_id: "bc3-direct".into(),
        cooked_asset_id: "cooked-bc3-direct".into(),
        width: 4,
        height: 4,
        mip_count: 3,
        format: "bc3RgbaUnormSrgb".into(),
        color_space: "srgb".into(),
        sampler: "nearestClamp".into(),
        rgba8: red.repeat(3).into(),
        source_hash: "red".into(),
    };
    backend.register_cooked_texture(handle, &payload).unwrap();
    backend
        .set_geometry_visual_resident_limit(4096 * 64 + 95)
        .unwrap();
    let blue = [255, 0, 0, 0, 0, 0, 0, 0, 31, 0, 0, 0, 0, 0, 0, 0];
    payload.rgba8 = blue.repeat(3).into();
    assert_eq!(
        backend
            .register_cooked_texture(handle, &payload)
            .unwrap_err(),
        "geometry.visual_resident_budget"
    );
    let mut graph = RenderGraph::new("bc3-direct", 1);
    graph.output_target = Some("main".into());
    graph
        .resources
        .push(RenderResource::surface_backbuffer("main", 16, 16));
    graph.passes.push(RenderPass {
        pass_id: "sprite".into(),
        pass_name: "sprite".into(),
        pass_kind: RenderPassKind::DrawSpriteTextured,
        view_id: "view-1".into(),
        reads: vec![],
        writes: vec!["main".into()],
        color_targets: vec!["main".into()],
        depth_target: None,
        debug_source: None,
        commands: vec![
            RenderPassCommand::Clear {
                target: "main".into(),
                color: color([0., 0., 0., 1.]),
            },
            RenderPassCommand::DrawSpriteTextured {
                target: "main".into(),
                sprite_ref: "bc3".into(),
                material_ref: None,
                sort_key: "sort".into(),
                texture: Some(handle),
                binding: None,
                fallback_used: false,
                vertices: [
                    ([-0.8, -0.8], [0., 1.]),
                    ([0.8, -0.8], [1., 1.]),
                    ([0.8, 0.8], [1., 0.]),
                    ([-0.8, -0.8], [0., 1.]),
                    ([0.8, 0.8], [1., 0.]),
                    ([-0.8, 0.8], [0., 0.]),
                ]
                .into_iter()
                .map(|(position, uv)| RenderDrawVertex::new(position, [1.; 4], uv))
                .collect(),
            },
        ]
        .into(),
    });
    let plan = compile_render_graph_to_rhi_plan(&graph);
    // The sprite's transient vertices now share the resident pool too. Keep
    // the texture-replacement negative above independent of draw admission.
    backend
        .set_geometry_visual_resident_limit(4096 * 64 + 48 + 6 * 32 + 16 * 16 * 4 + 256 * 16)
        .unwrap();
    let image = backend.render_plan_to_rgba_bytes(&plan, 16, 16).unwrap();
    assert!(image.chunks_exact(4).any(|p| p == [255, 0, 0, 255]));
    backend
        .set_geometry_visual_resident_limit(4096 * 64 + 96)
        .unwrap();
    backend.register_cooked_texture(handle, &payload).unwrap();
    backend
        .set_geometry_visual_resident_limit(4096 * 64 + 48 + 6 * 32 + 16 * 16 * 4 + 256 * 16)
        .unwrap();
    let image = backend.render_plan_to_rgba_bytes(&plan, 16, 16).unwrap();
    assert!(image.chunks_exact(4).any(|p| p == [0, 0, 255, 255]));
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_shared_pages_preserve_mixed_offsets_independent_pins_and_reuse() {
    use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, GeometryUploadState};
    use std::time::Duration;
    let mut manager = RenderResourceManager::new();
    let a = prepare("page-a", RenderAssetData::Mesh(triangle()), &mut manager);
    let mut mesh = triangle();
    mesh.vertex_bytes = mesh
        .vertex_bytes
        .chunks_exact(12)
        .flat_map(|v| v.iter().copied().chain([0; 4]).collect::<Vec<_>>())
        .collect();
    mesh.vertex_stride = 16;
    mesh.index_format = GeometryIndexFormat::Uint32;
    mesh.index_bytes = [0u32, 0, 0, 0, 1, 2]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    mesh.index_count = 6;
    let prefix = mesh.submeshes[0].clone();
    mesh.submeshes[0].first_index = 3;
    mesh.submeshes.insert(0, prefix);
    let b = prepare("page-b", RenderAssetData::Mesh(mesh), &mut manager);
    let red = prepare(
        "page-red",
        RenderAssetData::Material(unlit([1., 0., 0., 1.])),
        &mut manager,
    );
    let green = prepare(
        "page-green",
        RenderAssetData::Material(unlit([0., 1., 0., 1.])),
        &mut manager,
    );
    let a = PreparedGeometry::from_assets(&a, &red, 0, &manager).unwrap();
    let b = PreparedGeometry::from_assets(&b, &green, 1, &manager).unwrap();
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let make_plan = |both: bool| {
        let mut instances = vec![instance(&b, 2, [0.55, 0., -2.])];
        if both {
            instances.push(instance(&a, 1, [-0.55, 0., -2.]));
        }
        let f = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &instances,
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        compile_render_graph_to_rhi_plan(&graph(&f))
    };
    let both = make_plan(true);
    let neighbor = make_plan(false);
    // A separately allocated reference exercises identical content at zero offsets.
    let mut reference = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    reference
        .begin_geometry_upload(std::slice::from_ref(&a), &[])
        .unwrap();
    let reference_ticket = reference
        .begin_geometry_upload(std::slice::from_ref(&b), &[])
        .unwrap();
    reference.pump_geometry_uploads(1).unwrap();
    reference
        .wait_gpu_submission(
            reference.last_gpu_submission().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(
        reference.geometry_upload_state(reference_ticket).unwrap(),
        GeometryUploadState::Ready
    );
    let expected = reference.render_plan_to_rgba_bytes(&both, 64, 64).unwrap();
    let expected_neighbor = reference
        .render_plan_to_rgba_bytes(&neighbor, 64, 64)
        .unwrap();
    assert!(expected.chunks_exact(4).any(|p| p == [255, 0, 0, 255]));
    assert!(expected.chunks_exact(4).any(|p| p == [0, 255, 0, 255]));
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 128,
            // Physical mesh pages remain bounded to 128 bytes. Drawing also
            // uploads two 128-byte instance records, one 176-byte view and indices.
            staging_bytes: 1024,
            bytes_per_frame: 1024,
            max_tickets: 4,
        })
        .unwrap();
    let batch = backend
        .begin_geometry_upload(&[a.clone(), b.clone()], &[])
        .unwrap();
    let pin_a = backend
        .begin_geometry_upload(std::slice::from_ref(&a), &[])
        .unwrap();
    let pin_b = backend
        .begin_geometry_upload(std::slice::from_ref(&b), &[])
        .unwrap();
    assert_eq!(backend.geometry_residency().physical_mesh_bytes, 128);
    backend.pump_geometry_uploads(1).unwrap();
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(
        backend.geometry_upload_state(batch).unwrap(),
        GeometryUploadState::Ready
    );
    assert_eq!(
        backend.render_plan_to_rgba_bytes(&both, 64, 64).unwrap(),
        expected
    );
    backend.release_geometry_upload(batch).unwrap();
    backend.release_geometry_upload(pin_a).unwrap();
    backend.poll_gpu_completion().unwrap();
    assert_eq!(
        backend.geometry_residency().physical_mesh_bytes,
        128,
        "neighbor keeps shared pages resident"
    );
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&neighbor, 64, 64)
            .unwrap(),
        expected_neighbor
    );
    // Reacquire the released source within the same physical budget: free ranges
    // must be usable without charging logical mesh bytes as another physical page.
    let reused = backend
        .begin_geometry_upload(std::slice::from_ref(&a), &[])
        .unwrap();
    backend.pump_geometry_uploads(2).unwrap();
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(backend.geometry_residency().physical_mesh_bytes, 128);
    assert_eq!(
        backend.geometry_upload_state(reused).unwrap(),
        GeometryUploadState::Ready
    );
    assert_eq!(
        backend.render_plan_to_rgba_bytes(&both, 64, 64).unwrap(),
        expected
    );
    backend.release_geometry_upload(reused).unwrap();
    backend.release_geometry_upload(pin_b).unwrap();
    backend.poll_gpu_completion().unwrap();
    assert_eq!(backend.geometry_residency().physical_mesh_bytes, 0);
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_static_geometry_plan_reuses_validation_but_changed_draw_is_rejected() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let mut plan = compile_render_graph_to_rhi_plan(&graph(&frame));
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut backend = RealWgpuBackend::from_shared_device_queue(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        64,
        64,
        "371 intent reuse",
    );
    backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    let first = backend.retained_geometry_stats().validation_draw_visits;
    assert_eq!(first, 1);
    let first_parameters = backend.retained_geometry_stats().parameter_draw_visits;
    assert_eq!(first_parameters, 1);
    backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert_eq!(
        backend.retained_geometry_stats().parameter_draw_visits,
        first_parameters,
        "identical installed plan prepared every parameter again"
    );
    assert_eq!(
        backend.retained_geometry_stats().validation_draw_visits,
        first,
        "identical static plan revalidated every instance"
    );
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("371 intent consumer"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
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
    // A readback installed the native plan but did not record any intent.
    let cold = backend.execute_plan_to_surface_view(&plan, &view);
    assert!(cold.diagnostics.is_empty(), "{cold:?}");
    let replay = backend.execute_plan_to_surface_view(&plan, &view);
    assert!(replay.diagnostics.is_empty(), "{replay:?}");
    assert_eq!(cold.draw_count, 1);
    assert_eq!(cold.binding_count, 2);
    assert_eq!(replay.draw_count, 2);
    assert_eq!(replay.binding_count, 4);
    assert_eq!(cold.uploaded_resource_count, replay.uploaded_resource_count);
    assert_eq!(cold.failed_resource_count, replay.failed_resource_count);
    for command in &mut plan.commands {
        if let engine_runtime::rhi_command_plan::RhiCommand::Draw {
            payload: engine_runtime::rhi_command_plan::RhiDrawPayload::Geometry { draw },
            ..
        } = command
        {
            std::sync::Arc::make_mut(draw).index_count = 1;
        }
    }
    assert!(backend.render_plan_to_rgba_bytes(&plan, 64, 64).is_err());
    assert!(backend.retained_geometry_stats().validation_draw_visits > first);
}

#[test]
#[ignore = "requires a real GPU"]
fn loading_gpu_receipts_reject_other_backends_and_report_device_abandonment() {
    use engine_runtime::wgpu_backend::real::GeometryUploadState;
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut owner = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let mut other = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let receipt = owner
        .begin_geometry_upload(std::slice::from_ref(&source), &[])
        .unwrap();
    assert!(other
        .geometry_upload_state(receipt)
        .unwrap_err()
        .contains("stale_upload_ticket"));
    assert!(other
        .release_geometry_upload(receipt)
        .unwrap_err()
        .contains("stale_upload_ticket"));
    owner.abandon_device_epoch();
    assert_eq!(
        owner.geometry_upload_state(receipt).unwrap(),
        GeometryUploadState::Abandoned
    );
    owner.release_geometry_upload(receipt).unwrap();
    assert_eq!(owner.geometry_upload_snapshot().tickets, 0);
    assert_eq!(owner.geometry_residency().mesh_bytes, 0);
    assert!(owner.begin_geometry_upload(&[source], &[]).is_err());
}

#[test]
#[ignore = "requires a real GPU"]
fn loading_gpu_segmented_upload_shares_completion_and_last_release() {
    use engine_runtime::wgpu_backend::real::{
        GeometryUploadBudget, GeometryUploadState, GpuSubmissionStatus,
    };
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 1024,
            staging_bytes: 16,
            bytes_per_frame: 8,
            max_tickets: 2,
        })
        .unwrap();
    let a = backend
        .begin_geometry_upload(std::slice::from_ref(&source), &[])
        .unwrap();
    let b = backend
        .begin_geometry_upload(std::slice::from_ref(&source), &[])
        .unwrap();
    assert!(backend
        .begin_geometry_upload(std::slice::from_ref(&source), &[])
        .is_err());
    assert_eq!(
        backend.geometry_upload_state(a).unwrap(),
        GeometryUploadState::Uploading
    );
    let plan = compile_render_graph_to_rhi_plan(&graph(&frame(&source, &manager)));
    assert!(backend
        .render_plan_to_rgba_bytes(&plan, 64, 64)
        .unwrap_err()
        .contains("upload_not_completed"));
    let mut frames = 0;
    for index in 1..32 {
        let first = backend.pump_geometry_uploads(index).unwrap();
        let second = backend.pump_geometry_uploads(index).unwrap();
        assert!(
            first + second <= 8,
            "same-frame calls must share the byte budget"
        );
        assert!(backend.geometry_upload_snapshot().staging_bytes <= 16);
        if first + second != 0 {
            frames += 1;
            let token = backend.last_gpu_submission().unwrap();
            assert_eq!(
                backend
                    .wait_gpu_submission(token, std::time::Duration::from_secs(5))
                    .unwrap(),
                GpuSubmissionStatus::Completed
            );
        }
        if backend.geometry_upload_state(a).unwrap() == GeometryUploadState::Ready {
            break;
        }
    }
    assert!(frames > 1);
    assert_eq!(
        backend.geometry_upload_state(b).unwrap(),
        GeometryUploadState::Ready
    );
    assert_eq!(
        backend.geometry_upload_count(),
        1,
        "two tickets must share one physical upload"
    );
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&plan, 64, 64)
            .unwrap_err(),
        "geometry.visual_upload_capacity",
        "the original 8-byte loading profile cannot upload an indivisible material slot"
    );
    backend.release_geometry_upload(a).unwrap();
    assert_eq!(backend.geometry_residency().meshes, 1);
    assert!(backend.release_geometry_upload(a).is_err());
    backend.release_geometry_upload(b).unwrap();
    assert_eq!(backend.geometry_residency().meshes, 0);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(backend.poll_gpu_completion().unwrap().retiring_bytes, 0);
    // Keep the tiny segmented-loading assertions above. Raster validation needs
    // its own legal parameter-upload allowance, after all old pins are released.
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 1024,
            staging_bytes: 1024,
            bytes_per_frame: 1024,
            max_tickets: 2,
        })
        .unwrap();
    let raster = backend
        .begin_geometry_upload(std::slice::from_ref(&source), &[])
        .unwrap();
    backend.pump_geometry_uploads(100).unwrap();
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(
        backend.geometry_upload_state(raster).unwrap(),
        GeometryUploadState::Ready
    );
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert!(pixels.chunks_exact(4).any(|p| p[0] > 200 && p[1] < 20));
    backend.release_geometry_upload(raster).unwrap();
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn loading_gpu_budget_rejects_before_allocating_and_partial_cancel_retires() {
    use engine_runtime::wgpu_backend::real::GeometryUploadBudget;
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 4,
            staging_bytes: 8,
            bytes_per_frame: 8,
            max_tickets: 1,
        })
        .unwrap();
    assert!(backend
        .begin_geometry_upload(std::slice::from_ref(&source), &[])
        .unwrap_err()
        .contains("resident_budget"));
    assert_eq!(backend.geometry_residency().meshes, 0);
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 1024,
            staging_bytes: 8,
            bytes_per_frame: 8,
            max_tickets: 1,
        })
        .unwrap();
    let ticket = backend
        .begin_geometry_upload(std::slice::from_ref(&source), &[])
        .unwrap();
    assert_eq!(backend.pump_geometry_uploads(1).unwrap(), 8);
    backend.release_geometry_upload(ticket).unwrap();
    assert_eq!(backend.geometry_residency().meshes, 0);
    backend
        .wait_gpu_submission(
            backend.last_gpu_submission().unwrap(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(backend.poll_gpu_completion().unwrap().retiring_bytes, 0);
}
#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_static_batch_and_sparse_upload() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut frame = frame(&source, &manager);
    let template = frame.views[0].draws[0].clone();
    frame.views[0].draws = (0..100)
        .map(|id| {
            let mut d = template.clone();
            std::sync::Arc::make_mut(&mut d).instance_id = id;
            d
        })
        .collect();
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).expect("real GPU required");
    let plan = compile_render_graph_to_rhi_plan(&graph(&frame));
    let old = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    let first = backend.retained_geometry_stats();
    assert_eq!(
        first.draws, 1,
        "100 compatible instances must use one real indexed draw"
    );
    assert_eq!(first.instances, 100);
    let second_pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    let second = backend.retained_geometry_stats();
    assert_eq!(old, second_pixels);
    assert_eq!(
        second.parameter_uploaded_bytes, first.parameter_uploaded_bytes,
        "clean frames must not upload parameters"
    );
    assert_eq!(second.bind_groups_created, first.bind_groups_created);
    std::sync::Arc::make_mut(&mut frame.views[0].draws[99]).base_color =
        [0., 1., 0., 1.].map(Into::into);
    let edited = compile_render_graph_to_rhi_plan(&graph(&frame));
    let edited_pixels = backend.render_plan_to_rgba_bytes(&edited, 64, 64).unwrap();
    assert_ne!(
        old, edited_pixels,
        "fixture must observe the changed content"
    );
    let changed = backend.retained_geometry_stats();
    assert_eq!(
        changed.parameter_uploaded_bytes - second.parameter_uploaded_bytes,
        128, // one instance record; unchanged view/camera/fog remain resident
        "one sparse instance edit"
    );
    let old_again = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert_eq!(
        old, old_again,
        "old packet must re-install its own parameter content"
    );
    eprintln!("retained stats: {:?}", backend.retained_geometry_stats());
}

fn read(backend: &mut RealWgpuBackend, frame: &GeometryFrame) -> Vec<u8> {
    backend
        .render_plan_to_rgba_bytes(
            &compile_render_graph_to_rhi_plan(&graph(frame)),
            frame.extent[0],
            frame.extent[1],
        )
        .unwrap()
}
fn pixel(bytes: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    bytes[(y * width + x) * 4..(y * width + x) * 4 + 4]
        .try_into()
        .unwrap()
}
#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_native_flat_variant_changes_shader_and_preserves_old_packet() {
    let mut manager = RenderResourceManager::new();
    let mut geometry = triangle();
    let positions = geometry.vertex_bytes.clone();
    geometry.vertex_bytes.clear();
    for p in positions.chunks_exact(12) {
        geometry.vertex_bytes.extend_from_slice(p);
        geometry
            .vertex_bytes
            .extend([1f32, 0., 0., 1.].into_iter().flat_map(f32::to_le_bytes));
    }
    geometry.vertex_stride = 28;
    geometry.attributes.push(VertexAttribute {
        semantic: VertexSemantic::Color0,
        format: VertexFormat::Float32x4,
        offset: 12,
    });
    let mesh = prepare("colored", RenderAssetData::Mesh(geometry), &mut manager);
    let material = prepare(
        "green",
        RenderAssetData::Material(unlit([0., 1., 0., 1.])),
        &mut manager,
    );
    let source = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
    let old = frame(&source, &manager);
    let mut flat = unlit([0., 1., 0., 1.]);
    flat.ignore_vertex_color = true;
    let flat = prepare("flat", RenderAssetData::Material(flat), &mut manager);
    let flat_source = PreparedGeometry::from_assets(&mesh, &flat, 0, &manager).unwrap();
    let new = frame(&flat_source, &manager);
    assert_ne!(
        old.views[0].draws[0].pipeline_identity("Rgba8Unorm"),
        new.views[0].draws[0].pipeline_identity("Rgba8Unorm")
    );
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let dark = read(&mut backend, &old);
    let green = read(&mut backend, &new);
    assert_eq!(pixel(&dark, 64, 32, 32), [0, 0, 0, 255]);
    assert_eq!(pixel(&green, 64, 32, 32), [0, 255, 0, 255]);
    assert_eq!(read(&mut backend, &old), dark);
    assert_eq!(backend.geometry_upload_count(), 1);
    assert_eq!(backend.retained_geometry_stats().draws, 3);
}
#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_transparent_order_and_incompatible_draws_share_only_pass() {
    let mut manager = RenderResourceManager::new();
    let mesh = prepare("mesh", RenderAssetData::Mesh(triangle()), &mut manager);
    let mut red = unlit([1., 0., 0., 0.5]);
    red.blend = MaterialBlend::AlphaBlend;
    red.depth = MaterialDepth::TestOnly;
    let mut green = red.clone();
    green.parameters.insert(
        "baseColor".into(),
        MaterialParameter::Vector4([0., 1., 0., 0.5]),
    );
    let red = prepare("red", RenderAssetData::Material(red), &mut manager);
    let green = prepare("green", RenderAssetData::Material(green), &mut manager);
    let red = PreparedGeometry::from_assets(&mesh, &red, 0, &manager).unwrap();
    let green = PreparedGeometry::from_assets(&mesh, &green, 0, &manager).unwrap();
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &[
            instance(&red, 1, [0., 0., -4.]),
            instance(&green, 2, [0., 0., -3.]),
            instance(&red, 3, [0., 0., -2.]),
        ],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    assert_eq!(
        frame.views[0]
            .draws
            .iter()
            .map(|d| d.instance_id)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let bytes = read(&mut backend, &frame);
    let p = pixel(&bytes, 64, 32, 32);
    assert!(
        (i32::from(p[0]) - 159).abs() <= 2 && (i32::from(p[1]) - 64).abs() <= 2,
        "{p:?}"
    );
    let stats = backend.retained_geometry_stats();
    assert_eq!(stats.draws, 3, "nonadjacent transparent runs cannot merge");
    assert_eq!(
        stats.passes, 3,
        "one depth clear, one color clear, one shared draw pass"
    );
    assert_eq!(stats.instances, 3);
    let mut unique = frame.clone();
    let other = prepare("other", RenderAssetData::Mesh(triangle()), &mut manager);
    let other =
        PreparedGeometry::from_assets(&other, &red_material(&mut manager), 0, &manager).unwrap();
    unique.views[0].draws[1] = frame_for(&other, &manager).views[0].draws[0].clone();
    std::sync::Arc::make_mut(&mut unique.views[0].draws[1]).instance_id = 2;
    read(&mut backend, &unique);
    assert_eq!(backend.geometry_residency().meshes, 2);
}
fn red_material(manager: &mut RenderResourceManager) -> PreparedCookedRenderAsset {
    prepare(
        "opaque-red",
        RenderAssetData::Material(unlit([1., 0., 0., 1.])),
        manager,
    )
}
fn frame_for(source: &PreparedGeometry, manager: &RenderResourceManager) -> GeometryFrame {
    frame(source, manager)
}
#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_two_views_resize_and_replay_do_not_share_slots_or_depth() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut left = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    left.viewport = [0, 0, 32, 64];
    let mut right = left.clone();
    right.view_id = 2;
    right.viewport = [32, 0, 32, 64];
    let mut old = GeometryFrame::build(
        "main",
        [64, 64],
        &[
            GeometryViewInput {
                view: &left,
                pose: CameraPose::default(),
            },
            GeometryViewInput {
                view: &right,
                pose: CameraPose::default(),
            },
        ],
        &[instance(&source, 7, [0., 0., -2.])],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    std::sync::Arc::make_mut(&mut old.views[1].draws[0]).base_color =
        [0., 1., 0., 1.].map(Into::into);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let original = read(&mut backend, &old);
    assert_eq!(pixel(&original, 64, 16, 32), [255, 0, 0, 255]);
    assert_eq!(pixel(&original, 64, 48, 32), [0, 255, 0, 255]);
    assert_eq!(backend.geometry_residency().depth_targets, 2);
    let mut resized = old.clone();
    resized.extent = [128, 64];
    for (i, v) in resized.views.iter_mut().enumerate() {
        v.view.viewport = [i as u32 * 64, 0, 64, 64];
        v.view.target_extent = [128, 64];
        std::sync::Arc::make_mut(&mut v.draws[0]).view = v.view.clone().into();
    }
    let bytes = read(&mut backend, &resized);
    assert_eq!(pixel(&bytes, 128, 32, 32), [255, 0, 0, 255]);
    assert_eq!(pixel(&bytes, 128, 96, 32), [0, 255, 0, 255]);
    assert_eq!(read(&mut backend, &old), original);
    assert_eq!(backend.geometry_upload_count(), 1);
    assert_eq!(backend.retained_geometry_stats().instances, 6);
}
#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_duplicate_and_capacity_reject_before_upload() {
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let plan = compile_render_graph_to_rhi_plan(&graph(&frame));
    let command = plan
        .commands
        .iter()
        .find(|c| {
            matches!(
                c,
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { .. },
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    let mut duplicate = plan.clone();
    let at = duplicate
        .commands
        .iter()
        .position(|c| matches!(c, RhiCommand::Submit))
        .unwrap();
    duplicate.commands.insert(at, command.clone());
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let error = backend
        .render_plan_to_rgba_bytes(&duplicate, 64, 64)
        .unwrap_err();
    assert!(error.contains("duplicate_view_instance"), "{error}");
    assert_eq!(backend.geometry_upload_count(), 0);
    assert_eq!(
        backend.retained_geometry_stats().parameter_uploaded_bytes,
        0
    );
    let mut capacity = plan.clone();
    capacity.commands.retain(|c| {
        !matches!(
            c,
            RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { .. },
                ..
            }
        )
    });
    let at = capacity
        .commands
        .iter()
        .position(|c| matches!(c, RhiCommand::Submit))
        .unwrap();
    capacity.commands.splice(
        at..at,
        (0..65537).map(|id| {
            let mut c = command.clone();
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } = &mut c
            {
                std::sync::Arc::make_mut(draw).instance_id = id;
            }
            c
        }),
    );
    let error = backend
        .render_plan_to_rgba_bytes(&capacity, 64, 64)
        .unwrap_err();
    assert!(error.contains("instance_capacity"), "{error}");
    assert_eq!(backend.geometry_upload_count(), 0);
    assert_eq!(
        backend.retained_geometry_stats().resident_parameter_bytes,
        0
    );
    read(&mut backend, &frame);
    assert_eq!(backend.geometry_upload_count(), 1);
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_three_in_flight_submissions_bind_each_packet_content() {
    retained_three_in_flight(false);
}
#[test]
#[ignore = "requires a real GPU"]
fn gpu_s2a_retained_three_in_flight_standard_lit_packets_keep_each_material_version() {
    retained_three_in_flight(true);
}
fn retained_three_in_flight(lit: bool) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    eprintln!("371 adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut backend = RealWgpuBackend::from_shared_device_queue(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        64,
        64,
        "371 in-flight",
    );
    let mut manager = RenderResourceManager::new();
    let mut material_textures = Vec::new();
    let source = if lit {
        let texture = flight_texture(&mut manager, [255, 0, 0, 255]);
        let source = flight_lit_source("old", &texture, &mut manager);
        material_textures.push(texture);
        source
    } else {
        source(&mut manager)
    };
    let old = frame(&source, &manager);
    let mut new = old.clone();
    if lit {
        let texture = flight_texture(&mut manager, [0, 255, 0, 255]);
        assert_ne!(texture.handle, material_textures[0].handle);
        let source = flight_lit_source("new", &texture, &mut manager);
        new = frame(&source, &manager);
        material_textures.push(texture);
    }

    std::sync::Arc::make_mut(&mut new.views[0].draws[0]).world_transform[12] = 0.3.into();
    std::sync::Arc::make_mut(&mut new.views[0].draws[0]).base_color =
        [0., 1., 0., 1.].map(Into::into);
    if lit {
        std::sync::Arc::make_mut(&mut new.views[0].draws[0])
            .standard_lit
            .as_mut()
            .unwrap()
            .emissive = [0., 1., 0., 0.].map(Into::into);
    }
    let plans = [
        compile_render_graph_to_rhi_plan(&graph(&old)),
        compile_render_graph_to_rhi_plan(&graph(&new)),
        compile_render_graph_to_rhi_plan(&graph(&old)),
    ];
    let textures: Vec<_> = (0..3)
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
    // Deliberately no poll/readback/completion wait between the three submissions.
    for (index, (plan, texture)) in plans.iter().zip(&textures).enumerate() {
        if lit {
            let selected = if index == 1 { 1 } else { 0 };
            backend
                .prepare_geometry_textures(std::slice::from_ref(&material_textures[selected]))
                .unwrap();
            if index != 0
                && backend.gpu_lifecycle_stats().submitted > backend.gpu_lifecycle_stats().completed
            {
                assert!(
                    backend.gpu_lifecycle_stats().retiring_bytes >= 4,
                    "old material texture must remain charged until real completion"
                );
            }
        }

        let report =
            backend.execute_plan_to_surface_view(plan, &texture.create_view(&Default::default()));
        assert!(report.diagnostics.is_empty(), "{report:?}");
    }
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("371 three packet readback"),
        size: 3 * 64 * 256,
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
    assert_eq!(&mapped[..size], &mapped[2 * size..3 * size]);
    assert_ne!(&mapped[..size], &mapped[size..2 * size]);
    assert_eq!(pixel(&mapped[..size], 64, 32, 32), [255, 0, 0, 255]);
    assert_eq!(pixel(&mapped[size..2 * size], 64, 32, 32), [0, 255, 0, 255]);
    assert_eq!(
        backend.retained_geometry_stats().parameter_uploaded_bytes,
        3 * 128 + 176 // three changed instance versions, one unchanged view
    );
    assert_eq!(
        backend.retained_geometry_stats().bind_groups_created,
        if lit { 5 } else { 2 }
    );
    drop(mapped);
    buffer.unmap();
    backend.prepare_geometry_textures(&[]).unwrap();
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_retained_mirrored_winding_splits_batch_and_preserves_pixels() {
    let mut manager = RenderResourceManager::new();
    let mesh = prepare("m", RenderAssetData::Mesh(triangle()), &mut manager);
    let mut material = unlit([1., 0., 0., 1.]);
    material.cull = MaterialCull::Back;
    let material = prepare(
        "back-cull",
        RenderAssetData::Material(material),
        &mut manager,
    );
    let source = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let first = instance(&source, 1, [-0.5, 0., -2.]);
    let mut second = instance(&source, 2, [0.5, 0., -2.]);
    second.world_transform[0] = -1.;
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &[first, second],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let bytes = read(&mut backend, &frame);
    assert_eq!(pixel(&bytes, 64, 16, 32), [255, 0, 0, 255]);
    assert_eq!(pixel(&bytes, 64, 48, 32), [255, 0, 0, 255]);
    assert_eq!(backend.retained_geometry_stats().draws, 2);
    assert_eq!(backend.geometry_residency().pipelines, 2);
    assert_eq!(backend.geometry_residency().meshes, 1);
}

fn flight_texture(manager: &mut RenderResourceManager, color: [u8; 4]) -> PreparedTextureAsset {
    RuntimeRenderAssetProducer::new()
        .prepare_texture_payload(
            0,
            engine_runtime::runtime_texture::RuntimeTexturePayload {
                asset_id: "flight-emissive".into(),
                cooked_asset_id: "flight-emissive-cooked".into(),
                width: 1,
                height: 1,
                format: "rgba8UnormSrgb".into(),
                color_space: "srgb".into(),
                mip_count: 1,
                rgba8: color.to_vec().into(),
                sampler: "nearestClamp".into(),
                source_hash: format!("flight-{color:?}"),
            },
            RenderBindingKind::Texture,
            manager,
        )
        .unwrap()
}
fn flight_lit_source(
    id: &str,
    texture: &PreparedTextureAsset,
    manager: &mut RenderResourceManager,
) -> PreparedGeometry {
    let mut g = lit_triangle();
    g.vertex_bytes = g
        .vertex_bytes
        .chunks_exact(24)
        .flat_map(|v| v.iter().copied().chain([0u8; 8]))
        .collect();
    g.vertex_stride = 32;
    g.attributes.push(VertexAttribute {
        semantic: VertexSemantic::TexCoord0,
        format: VertexFormat::Float32x2,
        offset: 24,
    });
    let mesh = prepare("lit-flight-mesh", RenderAssetData::Mesh(g), manager);
    let mut material = standard_lit([1.; 4]);
    material.textures.insert(
        "emissiveTexture".into(),
        RenderAssetReference {
            expected_kind: RenderAssetKind::Texture,
            reference: engine_runtime::runtime_package::RuntimeAssetRef {
                id: "flight-emissive".into(),
                asset_type: "texture".into(),
                guid: None,
                sub_asset: None,
            },
        },
    );
    material.texture_sampling.insert(
        "emissiveTexture".into(),
        engine_runtime::runtime_texture::TextureSampler::parse("nearestClamp").unwrap(),
    );
    let mut material = prepare(
        &format!("lit-flight-{id}"),
        RenderAssetData::Material(material),
        manager,
    );
    material.binding.slots.push(NamedRenderBinding {
        name: "emissiveTexture".into(),
        resource: BindingResource::Texture(texture.handle),
    });
    PreparedGeometry::from_assets(&mesh, &material, 0, manager).unwrap()
}
