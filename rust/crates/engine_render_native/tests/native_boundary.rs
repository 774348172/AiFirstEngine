use engine_render_native::wgpu_backend::WgpuBackend;
use engine_runtime_core::engine_rhi::EngineRhiBackend;

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU; dependency render-bundle lifetime regression"]
fn render_bundle_last_alias_releases_the_wgpu_registry_entry() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, _) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let count = || {
        instance
            .generate_report()
            .unwrap()
            .hub
            .render_bundles
            .num_allocated
    };
    let baseline = count();
    let encoder = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: Some("374-bundle-lifetime"),
        color_formats: &[],
        depth_stencil: None,
        sample_count: 1,
        multiview: None,
    });
    let bundle = encoder.finish(&Default::default());
    let alias = bundle.clone();
    assert_eq!(count(), baseline + 1);
    drop(bundle);
    assert_eq!(
        count(),
        baseline + 1,
        "an alias must retain the exact bundle"
    );
    drop(alias);
    assert_eq!(
        count(),
        baseline,
        "last bundle alias leaked a wgpu registry entry"
    );
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU; changes admission limits without allocating GiB of backing"]
fn windows_four_gib_visual_resident_limit_preserves_refusal_above_the_ceiling() {
    use engine_render_native::wgpu_backend::real::RealWgpuBackend;
    const GIB: u64 = 1024 * 1024 * 1024;
    let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
    backend.set_geometry_visual_resident_limit(4 * GIB).unwrap();
    assert_eq!(
        backend.set_geometry_visual_resident_limit(4 * GIB + 1),
        Err("geometry.invalid_visual_resident_limit".into())
    );
    assert_eq!(
        backend.set_geometry_visual_resident_limit(0),
        Err("geometry.invalid_visual_resident_limit".into())
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
fn unsupported_upload_reader_rejects_every_operation() {
    let mut backend = WgpuBackend::new_unavailable();
    let reader: &mut dyn EngineRhiBackend = &mut backend;
    let ticket = engine_runtime_core::geometry_upload::GeometryUploadTicket::issue_for_backend(1);
    assert_eq!(
        reader.begin_geometry_upload(&[], &[]).unwrap_err(),
        "geometry.upload.unsupported_backend"
    );
    assert_eq!(
        reader.geometry_upload_state(ticket).unwrap_err(),
        "geometry.upload.unsupported_backend"
    );
    assert_eq!(
        reader.release_geometry_upload(ticket).unwrap_err(),
        "geometry.upload.unsupported_backend"
    );
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real GPU"]
fn rhi_upload_delegates_to_the_same_native_ticket_and_epoch_owner() {
    use engine_render_native::wgpu_backend::real::{
        GeometryUploadState, GeometryUploadTicket, RealWgpuBackend,
    };
    let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
    let reader: &mut dyn EngineRhiBackend = &mut backend;
    let ticket: engine_runtime_core::geometry_upload::GeometryUploadTicket =
        reader.begin_geometry_upload(&[], &[]).unwrap();
    let same: GeometryUploadTicket = ticket;
    assert_eq!(
        reader.geometry_upload_state(same).unwrap(),
        GeometryUploadState::Ready
    );
    let (_, epoch) = same.backend_parts();
    let unregistered = GeometryUploadTicket::issue_for_backend(epoch);
    assert_ne!(unregistered, same);
    assert_eq!(
        reader.geometry_upload_state(unregistered).unwrap_err(),
        "geometry.unknown_upload_ticket"
    );
    assert_eq!(
        reader.release_geometry_upload(unregistered).unwrap_err(),
        "geometry.unknown_upload_ticket"
    );
    let stale = GeometryUploadTicket::issue_for_backend(epoch.wrapping_add(1));
    assert_eq!(
        reader.geometry_upload_state(stale).unwrap_err(),
        "geometry.stale_upload_ticket"
    );
    assert_eq!(
        reader.release_geometry_upload(stale).unwrap_err(),
        "geometry.stale_upload_ticket"
    );
    // A rejected receipt must not release the genuine one.
    assert_eq!(
        reader.geometry_upload_state(same).unwrap(),
        GeometryUploadState::Ready
    );
    reader.release_geometry_upload(same).unwrap();
    assert_eq!(
        reader.geometry_upload_state(same).unwrap_err(),
        "geometry.unknown_upload_ticket"
    );
    assert_eq!(
        reader.release_geometry_upload(same).unwrap_err(),
        "geometry.unknown_upload_ticket"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
