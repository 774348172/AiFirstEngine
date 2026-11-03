use super::*;
use crate::render_resource::*;
use project_runtime_sdk::generated_content::{ContentTicket, GeometryBuilder3D, GeometrySection3D};

pub(crate) fn fixture() -> (RenderResourceManager, PreparedCookedRenderAsset, RenderView) {
    let (manager, template, view, _) =
        super::super::shadow::receiver_scene(MaterialBlend::Opaque, -2.);
    let params = (*template.material).clone();
    let pipeline_identity = Some(params.pipeline_identity().unwrap());
    let material = PreparedCookedRenderAsset {
        asset: CookedRenderAsset {
            source_digest: crate::canonical_digest::sha256_prefixed(b"376-material-fixture"),
            recipe: NATIVE_RENDER_COOK_RECIPE.into(),
            asset: RenderAssetData::Material(params),
        },
        binding: template.material_binding.as_ref().clone(),
        state: RenderAssetPreparationState::Prepared,
        textures: vec![],
        resource_request: RenderResourceRequest {
            key: RenderAssetKey::new(
                "material-guid",
                "fixture",
                RenderResourceKind::MaterialParams,
            ),
            source: RenderResourceSource::MaterialParamsDescriptor { param_count: 6 },
            byte_len: 64,
            reason: "376-binding-fixture".into(),
        },
        asset_guid: "material-guid".into(),
        cooked_digest: crate::canonical_digest::sha256_prefixed(b"376-material-fixture"),
        source_path: None,
        pipeline_identity,
    };
    (manager, material, view)
}

pub(crate) fn geometry(count: usize, normal: [f32; 3], color: [f32; 4]) -> Geometry3D {
    let mut positions = vec![[0., 0., 0.]; count];
    positions[..3].copy_from_slice(&[[-0.8, -0.8, 0.], [0.8, -0.8, 0.], [0., 0.8, 0.]]);
    GeometryBuilder3D::new(&positions)
        .normals(&vec![normal; count])
        .uv(&vec![[0., 0.]; count])
        .colors(&vec![color; count])
        .triangles_u16(
            &[0, 1, 2],
            vec![GeometrySection3D {
                first_index: 0,
                index_count: 3,
                material_slot: "surface".into(),
                material: ProjectRuntimeSpatialAssetRef {
                    id: "material".into(),
                    asset_type: "material".into(),
                    guid: Some("material-guid".into()),
                    sub_asset: None,
                },
            }],
        )
        .unwrap()
}

pub(crate) fn binding(
    manager: &mut RenderResourceManager,
    generated: &GeneratedGeometry,
    revision: u64,
) -> (Arc<RenderBindingSet>, Arc<Provenance>) {
    let request = generated.binding_request(
        ContentTicket {
            session_generation: 376,
            request_id: 1,
        },
        revision,
    );
    let asset_id = request.key.asset_id.clone();
    let handle = manager.request_resource(revision, request);
    let binding = Arc::new(
        RenderBindingSet::single(
            asset_id.clone(),
            RenderBindingKind::Mesh,
            handle,
            "linearClamp".into(),
            false,
            asset_id.clone(),
        )
        .unwrap(),
    );
    let provenance = Arc::new(Provenance {
        asset_id: Some(asset_id),
        availability: SourceAvailability::SuppliedContext,
        ..Provenance::unspecified(ProducerStage::AssetBinding)
    });
    (binding, provenance)
}

#[test]
fn generated_content_376_mesh_binding_shares_canonical_buffers_and_keeps_old_revision_resident() {
    let (mut manager, material, _) = fixture();
    let old = GeneratedGeometry::prepare(geometry(3, [0., 0., 1.], [1.; 4])).unwrap();
    let (old_binding, old_source) = binding(&mut manager, &old, 1);
    let old_prepared = old
        .bind(0, old_binding.clone(), old_source, &material, &manager)
        .unwrap();
    assert!(Arc::ptr_eq(&old.buffers, &old_prepared.buffers));
    let new = old
        .patch(AttributePatch3D::positions(2, &[[0., 10., 0.]]).unwrap())
        .unwrap();
    let (new_binding, new_source) = binding(&mut manager, &new, 2);
    let new_prepared = new
        .bind(0, new_binding.clone(), new_source, &material, &manager)
        .unwrap();
    assert_ne!(
        new_binding.resources().next(),
        old_binding.resources().next()
    );
    old_binding.validate_residency(&manager).unwrap();
    new_binding.validate_residency(&manager).unwrap();
    assert_eq!(old_prepared.bounds.max[1], 0.8);
    assert_eq!(new_prepared.bounds.max[1], 10.);
    assert!(Arc::ptr_eq(&new.buffers, &new_prepared.buffers));
    assert_eq!(
        old.buffers.index_bytes().as_ptr(),
        new.buffers.index_bytes().as_ptr()
    );
    assert_eq!(new_prepared.material_binding.as_ref(), &material.binding);
    let mut wrong_material = material.clone();
    wrong_material.asset_guid = "different-guid".into();
    assert_eq!(
        new.bind(
            0,
            new_binding.clone(),
            new_source_for_test(),
            &wrong_material,
            &manager
        )
        .unwrap_err()
        .field,
        "material.assetRef"
    );
    assert!(new
        .bind(1, new_binding, new_source_for_test(), &material, &manager)
        .is_err());
}

fn new_source_for_test() -> Arc<Provenance> {
    Arc::new(Provenance::unspecified(ProducerStage::AssetBinding))
}

#[test]
fn generated_content_376_mesh_binding_reuses_standard_lit_normal_and_uv_validation() {
    let (mut manager, material, _) = fixture();
    let zero = GeneratedGeometry::prepare(geometry(3, [0.; 3], [1.; 4])).unwrap();
    let (binding, source) = binding(&mut manager, &zero, 1);
    assert_eq!(
        zero.bind(0, binding.clone(), source.clone(), &material, &manager)
            .unwrap_err()
            .field,
        "material.zeroNormal"
    );
    let fixed = zero
        .patch(AttributePatch3D::normals(0, &[[0., 0., 1.]; 3]).unwrap())
        .unwrap();
    let prepared = fixed.bind(0, binding, source, &material, &manager).unwrap();
    assert!(Arc::ptr_eq(&fixed.buffers, &prepared.buffers));
    assert!(fixed
        .buffers
        .validate_standard_lit_inputs(0..3, true, true)
        .is_err());
    let uv = fixed
        .patch(AttributePatch3D::uv(0, &[[0., 0.], [1., 0.], [0., 1.]]).unwrap())
        .unwrap();
    uv.buffers
        .validate_standard_lit_inputs(0..3, true, true)
        .unwrap();
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires exclusive real GPU; position culling and canonical payload on a rebuilt device"]
fn generated_content_376_mesh_native_position_patch_and_device_rebuild_keep_real_pixels() {
    use crate::wgpu_backend::real::{GeometryUploadState, RealWgpuBackend};
    use std::time::{Duration, Instant};
    let (mut manager, mut material, view) = fixture();
    let RenderAssetData::Material(params) = &mut material.asset.asset else {
        unreachable!()
    };
    params.preset = MaterialPreset::Unlit;
    params.shader_source = UNLIT_SHADER_SOURCE.into();
    params.parameters.retain(|key, _| key == "baseColor");
    params.validate().unwrap();
    let old = GeneratedGeometry::prepare(geometry(3, [0., 0., 1.], [1., 0., 0., 1.])).unwrap();
    let moved = old
        .patch(
            AttributePatch3D::positions(0, &[[9.2, -0.8, 0.], [10.8, -0.8, 0.], [10., 0.8, 0.]])
                .unwrap(),
        )
        .unwrap();
    let colored = moved
        .patch(AttributePatch3D::colors(0, &[[0., 1., 0., 1.]; 3]).unwrap())
        .unwrap();
    assert_eq!(moved.bounds, colored.bounds);
    assert!(Arc::ptr_eq(
        moved.buffers.index_storage(),
        colored.buffers.index_storage()
    ));
    let mut prepared = vec![];
    for (index, generated) in [&old, &moved, &colored].into_iter().enumerate() {
        let (binding, source) = binding(&mut manager, generated, index as u64 + 1);
        prepared.push(
            generated
                .bind(0, binding, source, &material, &manager)
                .unwrap(),
        );
    }
    let frame = |source: &PreparedGeometry| {
        let mut transform = IDENTITY_MATRIX;
        transform[12] = -10.;
        transform[14] = -2.;
        GeometryFrame::build(
            "surface-main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &[GeometryInstance {
                instance_id: 1,
                source,
                world_transform: transform,
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named { names: vec![] },
                provenance: source.geometry_source.as_ref().clone(),
            }],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap()
    };
    assert_eq!(frame(&prepared[0]).views[0].draws.len(), 0);
    assert_eq!(
        frame(&prepared[1]).views[0].draws.len(),
        1,
        "position patch must update the culling bounds with its vertex data"
    );
    let render = |backend: &mut RealWgpuBackend, source: &PreparedGeometry| {
        let graph = super::super::batch_test_graph(&frame(source));
        backend
            .render_plan_to_rgba_bytes(
                &crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph),
                64,
                64,
            )
            .unwrap()
    };
    let ready = |backend: &mut RealWgpuBackend, source: &PreparedGeometry| {
        let ticket = backend
            .begin_geometry_upload(std::slice::from_ref(source), &[])
            .unwrap();
        let started = Instant::now();
        for index in 1..32 {
            assert!(started.elapsed() < Duration::from_secs(10));
            assert!(backend.pump_geometry_uploads(index).unwrap() <= 16 * 1024 * 1024);
            if let Some(token) = backend.last_gpu_submission() {
                backend
                    .wait_gpu_submission(token, Duration::from_secs(5))
                    .unwrap();
            }
            if backend.geometry_upload_state(ticket).unwrap() == GeometryUploadState::Ready {
                return ticket;
            }
        }
        panic!("original upload owner did not reach Ready")
    };
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let first = ready(&mut backend, &prepared[1]);
    let red = render(&mut backend, &prepared[1]);
    let second = ready(&mut backend, &prepared[2]);
    let green = render(&mut backend, &prepared[2]);
    let center = (32 * 64 + 32) * 4;
    assert!(red[center] > 200 && red[center + 1] < 20);
    assert!(green[center + 1] > 200 && green[center] < 20);
    assert_ne!(red, green);
    let pointer = colored.vertex_pointer();
    // Use the original explicit abandonment strategy, which destroys this Device.
    // This is recovery from explicit owner abandonment, not driver-loss qualification.
    backend.abandon_device_epoch();
    assert!(backend.gpu_lifecycle_stats().abandoned);
    assert_eq!(backend.geometry_residency().physical_mesh_bytes, 0);
    assert_eq!(backend.geometry_upload_snapshot().tickets, 0);
    let mut rebuilt = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    assert!(rebuilt.geometry_upload_state(first).is_err());
    assert!(rebuilt.geometry_upload_state(second).is_err());
    let recovered = ready(&mut rebuilt, &prepared[2]);
    assert_eq!(colored.vertex_pointer(), pointer);
    assert_eq!(render(&mut rebuilt, &prepared[2]), green);
    rebuilt.release_geometry_upload(recovered).unwrap();
    drop(prepared);
    let drained = rebuilt.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert!(!drained.abandoned);
    assert_eq!(drained.submitted, drained.completed);
    assert_eq!(
        (
            drained.in_flight,
            drained.retiring_objects,
            drained.retiring_bytes
        ),
        (0, 0, 0)
    );
    assert_eq!(rebuilt.geometry_residency().physical_mesh_bytes, 0);
    let pending = rebuilt.geometry_upload_snapshot();
    assert_eq!(
        (
            pending.tickets,
            pending.pending_resources,
            pending.staging_bytes
        ),
        (0, 0, 0)
    );
    eprintln!("376 original native position/attribute pixels and explicit-device-rebuild equality passed; green digest={}",
        crate::canonical_digest::sha256_prefixed(&green));
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires exclusive real GPU; generated canonical buffers to original upload owner"]
fn generated_content_376_mesh_native_upload_backlog_uses_original_frame_limit_and_drains() {
    use crate::wgpu_backend::real::{GeometryUploadBudget, GeometryUploadState, RealWgpuBackend};
    use std::time::{Duration, Instant};
    let (mut manager, mut material, view) = fixture();
    let RenderAssetData::Material(params) = &mut material.asset.asset else {
        unreachable!()
    };
    params.preset = MaterialPreset::Unlit;
    params.shader_source = UNLIT_SHADER_SOURCE.into();
    params.parameters.retain(|key, _| key == "baseColor");
    params.validate().unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    const FRAME_BYTES: usize = 16 * 1024 * 1024;
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 4 * 1024 * 1024 * 1024,
            staging_bytes: 128 * 1024 * 1024,
            bytes_per_frame: FRAME_BYTES,
            max_tickets: 256,
        })
        .unwrap();
    let mut prepared = Vec::new();
    let mut tickets = Vec::new();
    let mut total = 0;
    // Each legal request is ~2.88 MB including all attributes, below the 8 MiB
    // envelope. Seven distinct candidates exceed the unchanged 16 MiB frame budget.
    for index in 0..7 {
        let color = if index == 0 {
            [1., 0., 0., 1.]
        } else {
            [0., 1. - index as f32 * 0.05, 0., 1.]
        };
        let geometry = geometry(60_000, [0., 0., 1.], color);
        let request = project_runtime_sdk::generated_content::GeneratedContentRequest {
            request_id: index + 1,
            target: project_runtime_sdk::ProjectRuntimeCellEntity {
                entity_id: "fixture".into(),
                generation: 1,
            },
            expected_applied_revision: 0,
            operation: project_runtime_sdk::generated_content::GeneratedOperation3D::Replace {
                geometry,
                collision: project_runtime_sdk::generated_content::CollisionUpdate3D::Keep,
            },
        };
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
        let project_runtime_sdk::generated_content::GeneratedOperation3D::Replace {
            geometry, ..
        } = request.operation
        else {
            unreachable!()
        };
        let generated = GeneratedGeometry::prepare(geometry).unwrap();
        total += generated.buffers.vertex_byte_len().next_multiple_of(4)
            + generated.buffers.index_byte_len().next_multiple_of(4);
        let (binding, source) = binding(&mut manager, &generated, index + 1);
        let geometry = generated
            .bind(0, binding, source, &material, &manager)
            .unwrap();
        tickets.push(
            backend
                .begin_geometry_upload(std::slice::from_ref(&geometry), &[])
                .unwrap(),
        );
        prepared.push(geometry);
    }
    assert!(total > FRAME_BYTES);
    let started = Instant::now();
    let mut frame = 1;
    let mut uploads = Vec::new();
    loop {
        assert!(started.elapsed() < Duration::from_secs(30));
        let bytes = backend.pump_geometry_uploads(frame).unwrap();
        assert!(bytes <= FRAME_BYTES);
        uploads.push(bytes);
        if frame == 1 {
            assert!(tickets
                .iter()
                .any(|ticket| backend.geometry_upload_state(*ticket).unwrap()
                    == GeometryUploadState::Uploading));
            assert_eq!(
                backend.pump_geometry_uploads(frame).unwrap(),
                0,
                "same frame must share its allowance"
            );
        }
        if let Some(token) = backend.last_gpu_submission() {
            backend
                .wait_gpu_submission(token, Duration::from_secs(5))
                .unwrap();
        }
        if tickets.iter().all(|ticket| {
            backend.geometry_upload_state(*ticket).unwrap() == GeometryUploadState::Ready
        }) {
            break;
        }
        frame += 1;
        assert!(frame < 32);
    }
    assert!(uploads.len() >= 2);
    assert_eq!(uploads.iter().sum::<usize>(), total);
    let render = |backend: &mut RealWgpuBackend, source: &PreparedGeometry| {
        let mut transform = IDENTITY_MATRIX;
        transform[14] = -2.;
        let frame = GeometryFrame::build(
            "surface-main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &[GeometryInstance {
                instance_id: 1,
                source,
                world_transform: transform,
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named { names: vec![] },
                provenance: source.geometry_source.as_ref().clone(),
            }],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        let graph = super::super::batch_test_graph(&frame);
        backend
            .render_plan_to_rgba_bytes(
                &crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph),
                64,
                64,
            )
            .unwrap()
    };
    let red = render(&mut backend, &prepared[0]);
    let green = render(&mut backend, &prepared[1]);
    assert_ne!(red, green);
    let center = (32 * 64 + 32) * 4;
    assert!(
        red[center] > 200 && red[center + 1] < 20,
        "red center {:?}",
        &red[center..center + 4]
    );
    assert!(
        green[center + 1] > 180 && green[center] < 20,
        "green center {:?}",
        &green[center..center + 4]
    );
    for ticket in tickets {
        backend.release_geometry_upload(ticket).unwrap();
    }
    drop(prepared);
    let drained = backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert!(!drained.abandoned);
    assert_eq!(drained.submitted, drained.completed);
    assert_eq!(
        (
            drained.in_flight,
            drained.retiring_objects,
            drained.retiring_bytes
        ),
        (0, 0, 0)
    );
    let residency = backend.geometry_residency();
    assert_eq!((residency.meshes, residency.physical_mesh_bytes), (0, 0));
    let pending = backend.geometry_upload_snapshot();
    assert_eq!(
        (
            pending.tickets,
            pending.pending_resources,
            pending.staging_bytes
        ),
        (0, 0, 0)
    );
    eprintln!("376 original native owner: total={total}, frames={uploads:?}; real Ready, red/green readback and fence drain passed. No World/session Applied claim.");
}
