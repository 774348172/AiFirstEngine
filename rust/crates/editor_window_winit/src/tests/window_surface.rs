use super::*;

#[test]
fn window_attributes_keep_native_editor_defaults() {
    let config = NativeEditorWindowConfig::default();
    let attributes = native_window_attribute_plan(&config);
    assert_eq!(attributes.title, "AI First Engine Editor");
    assert!(attributes.resizable);
    assert_eq!(attributes.scale_factor, 1.0);
}

#[cfg(feature = "real-window")]
#[test]
fn real_window_feature_can_build_winit_and_wgpu_descriptors() {
    let config = NativeEditorWindowConfig::default();
    let attributes = winit_window_attributes(&config);
    let _instance = wgpu_instance_descriptor();
    assert_eq!(attributes.title, "AI First Engine Editor");
}

#[test]
fn wgpu_surface_plan_documents_real_backend_boundary() {
    let plan = wgpu_surface_plan();
    assert_eq!(plan.backend, "wgpu");
    assert!(plan.requires_native_window);
    assert!(plan.creates_surface);
    assert!(plan.creates_device_queue);
    assert!(plan.configures_swapchain);
}

#[test]
fn window_skeleton_is_headless_testable() {
    let readiness = validate_window_skeleton(&NativeEditorWindowConfig::default());
    assert!(readiness.window_attributes_ready);
    assert!(readiness.wgpu_instance_ready);
    assert!(readiness.headless_test_only);
}

#[test]
fn headless_window_backend_tracks_resize_redraw_and_close() {
    let mut backend = HeadlessWindowBackend::create_window(&NativeEditorWindowConfig::default());
    backend.resize(1440, 900, 1.5);
    backend.request_redraw();
    backend.close();
    let state = backend.snapshot();
    assert!(state.created);
    assert_eq!(
        state.size,
        WindowSize {
            width: 1440,
            height: 900
        }
    );
    assert_eq!(state.scale_factor, 1.5);
    assert!(state.redraw_requested);
    assert!(state.close_requested);
}

#[test]
fn dpi_logical_physical_round_trip_supports_fractional_scale() {
    let logical = LogicalPoint { x: 80.0, y: 40.0 };
    let physical = logical_to_physical(logical, 1.5);
    assert_eq!(physical, PhysicalPoint { x: 120.0, y: 60.0 });
    assert_eq!(physical_to_logical(physical, 1.5), logical);
}

#[test]
fn headless_surface_backend_tracks_acquire_present_and_loss() {
    let mut surface = HeadlessSurfaceBackend::create_surface();
    surface.configure(1280, 720, "Bgra8UnormSrgb", "Fifo");
    surface.acquire();
    surface.present();
    assert_eq!(surface.snapshot().presented_frame, 1);
    surface.lose_surface("surface_lost");
    assert_eq!(
        surface.snapshot().last_error.as_deref(),
        Some("surface_lost")
    );
    assert!(!surface.snapshot().configured);
}

#[test]
fn headless_surface_backend_reconfigures_on_resize() {
    let mut surface = HeadlessSurfaceBackend::create_surface();
    surface.configure(640, 360, "Bgra8UnormSrgb", "Fifo");
    surface.acquire();
    surface.present();

    surface.resize(1280, 720, "Bgra8UnormSrgb", "Fifo");
    let resized = surface.snapshot();

    assert!(resized.configured);
    assert_eq!(resized.width, 1280);
    assert_eq!(resized.height, 720);
    assert_eq!(resized.acquired_frame, 0);
    assert_eq!(resized.presented_frame, 0);
    assert_eq!(resized.last_error, None);
}

#[test]
fn headless_surface_backend_reports_acquire_before_configure() {
    let mut surface = HeadlessSurfaceBackend::create_surface();

    surface.acquire();

    assert_eq!(
        surface.snapshot().last_error.as_deref(),
        Some("surface_not_configured")
    );
}
