use super::*;

#[test]
#[ignore = "explicit Compiler delivery; real owned OS window maximize/restore regression"]
fn real_window_packaged_maximize_restore_preserves_frames() {
    struct TransitionApp {
        inner: RealWindowApp,
        phase: u8,
        transition_frame: u64,
        minimized_at: Option<Instant>,
        deferred_resize: Option<winit::dpi::PhysicalSize<u32>>,
        started: Instant,
    }
    impl ApplicationHandler for TransitionApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.inner.resumed(event_loop);
            if let Some(host) = &self.inner.host {
                // Retain the first GPU validation error instead of losing it to
                // a later invalid-encoder error while unwinding a surface frame.
                host.device.push_error_scope(wgpu::ErrorFilter::Validation);
            }
        }
        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            if let WindowEvent::Resized(size) = &event {
                if matches!(self.phase, 1 | 4) && self.deferred_resize.is_none() {
                    // The OS size is visible before its queued notification is
                    // consumed. Exercise one frame at that real boundary.
                    self.deferred_resize = Some(*size);
                    if let Some(host) = &self.inner.host {
                        host.window.request_redraw();
                    }
                    return;
                }
            }
            let resize_after_frame = matches!(event, WindowEvent::RedrawRequested)
                .then(|| self.deferred_resize.take())
                .flatten();
            self.inner.window_event(event_loop, window_id, event);
            if let Some(size) = resize_after_frame {
                self.inner
                    .window_event(event_loop, window_id, WindowEvent::Resized(size));
            }
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            assert!(self.started.elapsed() < std::time::Duration::from_secs(20));
            self.inner.about_to_wait(event_loop);
            let Some(host) = &mut self.inner.host else {
                return;
            };
            let frame = host.report.frames_completed;
            match self.phase {
                0 if frame >= 5 => {
                    self.transition_frame = frame;
                    host.window.set_maximized(true);
                    self.phase = 1;
                }
                1 if host.window.is_maximized() && frame >= self.transition_frame + 5 => {
                    assert_eq!(host.window.inner_size().width, host.report.surface.width);
                    assert_eq!(host.window.inner_size().height, host.report.surface.height);
                    self.transition_frame = frame;
                    host.window.set_minimized(true);
                    self.minimized_at = Some(Instant::now());
                    self.phase = 2;
                }
                2 if host.window.is_minimized() == Some(true)
                    && self.minimized_at.unwrap().elapsed()
                        >= std::time::Duration::from_millis(200) =>
                {
                    host.window.set_minimized(false);
                    self.phase = 3;
                }
                3 if host.window.is_minimized() == Some(false)
                    && frame >= self.transition_frame + 5 =>
                {
                    self.transition_frame = frame;
                    host.window.set_maximized(false);
                    self.phase = 4;
                }
                4 if !host.window.is_maximized() && frame >= self.transition_frame + 5 => {
                    assert_eq!(host.window.inner_size().width, host.report.surface.width);
                    assert_eq!(host.window.inner_size().height, host.report.surface.height);
                    self.phase = 5;
                }
                _ => {}
            }
            if self.phase == 2 {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + std::time::Duration::from_millis(10),
                ));
            }
        }
    }
    let delivery =
        PathBuf::from(std::env::var_os("AIFE_S5_WINDOW_DELIVERY").expect("Compiler delivery"));
    let evidence =
        PathBuf::from(std::env::var_os("AIFE_S5_WINDOW_EVIDENCE").expect("fresh evidence"));
    fs::create_dir(&evidence).unwrap();
    let module = fs::read_dir(delivery.join("data/bin"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|v| v == "dll"))
        .expect("actual packaged project module");
    let linked =
        engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_dll(
            &module,
        )
        .unwrap();
    let mut request = NativePlayerWindowRunRequest::windowed(delivery.join("data/runtime_package"));
    request.config.target_fps = 60;
    request.frame_limit = 180;
    let mut app = TransitionApp {
        inner: RealWindowApp::new(request, Arc::new(linked), None),
        phase: 0,
        transition_frame: 0,
        minimized_at: None,
        deferred_resize: None,
        started: Instant::now(),
    };
    EventLoop::builder()
        .with_any_thread(true)
        .build()
        .unwrap()
        .run_app(&mut app)
        .unwrap();
    let error = pollster::block_on(app.inner.host.as_ref().unwrap().device.pop_error_scope());
    let report = app.inner.report.as_ref().expect("terminal report");
    fs::write(
        evidence.join("report.json"),
        serde_json::to_vec_pretty(report).unwrap(),
    )
    .unwrap();
    fs::write(
        evidence.join("first-gpu-validation.txt"),
        format!("{error:#?}\n"),
    )
    .unwrap();
    assert!(error.is_none(), "first GPU validation error: {error:#?}");
    assert_eq!(
        app.phase, 5,
        "maximize/minimize/restore transitions must finish"
    );
    assert_eq!(report.frames_completed, 180);
    assert_eq!(report.completion_exit_code(), 0, "{:?}", report.diagnostics);
}

#[test]
#[ignore = "local Windows OS/GPU lifecycle; changes only its owned window"]
fn real_window_twenty_resize_minimize_restore_cycles_keep_production_geometry() {
    use std::time::Duration;
    use winit::dpi::PhysicalSize;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Phase {
        Warm,
        Resize,
        Minimize,
        Restore,
        Finished,
    }

    struct LifecycleApp {
        inner: RealWindowApp,
        phase: Phase,
        phase_started: Instant,
        started: Instant,
        size: PhysicalSize<u32>,
        resized: bool,
        minimized: bool,
        before_frame: u64,
        cycles: Vec<serde_json::Value>,
    }

    impl ApplicationHandler for LifecycleApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.inner.resumed(event_loop);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            if let WindowEvent::Resized(size) = &event {
                if self.phase == Phase::Resize && *size == self.size {
                    self.resized = true;
                }
            }
            self.inner.window_event(event_loop, window_id, event);
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            assert!(
                self.started.elapsed() < Duration::from_secs(90),
                "lifecycle deadline: phase={:?}, cycles={}",
                self.phase,
                self.cycles.len()
            );
            self.inner.about_to_wait(event_loop);
            let Some(host) = &mut self.inner.host else {
                return;
            };
            if self.phase != Phase::Finished {
                assert!(
                    self.phase_started.elapsed() < Duration::from_secs(5),
                    "OS transition deadline: phase={:?}, cycles={}, actual={:?}, minimized={:?}",
                    self.phase,
                    self.cycles.len(),
                    host.window.inner_size(),
                    host.window.is_minimized()
                );
            }
            match self.phase {
                Phase::Warm if host.report.frames_completed >= 2 => {
                    self.size = if self.cycles.len().is_multiple_of(2) {
                        PhysicalSize::new(400, 300)
                    } else {
                        PhysicalSize::new(360, 260)
                    };
                    self.before_frame = host.report.frames_completed;
                    self.resized = false;
                    self.minimized = false;
                    self.phase = Phase::Resize;
                    self.phase_started = Instant::now();
                    // Ask the real OS window. Only its delivered Resized event
                    // enters the production surface-reconfiguration handler.
                    let _ = host.window.request_inner_size(self.size);
                }
                Phase::Resize
                    if self.resized
                        && host.report.surface.width == self.size.width
                        && host.report.surface.height == self.size.height
                        && host.report.frames_completed > self.before_frame =>
                {
                    assert_eq!(host.window.inner_size(), self.size);
                    assert_eq!(host.report.window.width, self.size.width);
                    assert_eq!(host.report.window.height, self.size.height);
                    host.window.set_minimized(true);
                    self.phase = Phase::Minimize;
                    self.phase_started = Instant::now();
                }
                Phase::Minimize => {
                    self.minimized |= host.window.is_minimized() == Some(true);
                    if self.minimized && self.phase_started.elapsed() >= Duration::from_millis(120)
                    {
                        self.before_frame = host.report.frames_completed;
                        host.window.set_minimized(false);
                        self.phase = Phase::Restore;
                        self.phase_started = Instant::now();
                    }
                }
                Phase::Restore
                    if host.window.is_minimized() == Some(false)
                        && host.report.frames_completed > self.before_frame =>
                {
                    assert!(self.resized && self.minimized);
                    assert_eq!(host.window.inner_size(), self.size);
                    assert_eq!(host.report.surface.width, self.size.width);
                    assert_eq!(host.report.surface.height, self.size.height);
                    assert!(host.report.surface.last_error.is_none());
                    assert!(host.report.surface.presented_frame_count > 0);
                    self.cycles.push(serde_json::json!({
                        "cycle": self.cycles.len() + 1,
                        "extent": [self.size.width, self.size.height],
                        "actualResizeEvent": self.resized,
                        "osMinimizedObserved": self.minimized,
                        "osRestoredObserved": true,
                        "beforeRestoreFrame": self.before_frame,
                        "afterRestoreFrame": host.report.frames_completed,
                        "presentedFrames": host.report.surface.presented_frame_count,
                        "deviceEpoch": host.backend.gpu_lifecycle_stats().device_epoch,
                    }));
                    eprintln!("F05 actual window cycle: {}", self.cycles.last().unwrap());
                    self.phase = if self.cycles.len() == 20 {
                        Phase::Finished
                    } else {
                        Phase::Warm
                    };
                    self.phase_started = Instant::now();
                }
                _ => {}
            }
            // Keep the test controller polling actual OS state while minimized;
            // rendering and resize handling remain in the production app.
            if self.phase != Phase::Finished {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(10),
                ));
            }
        }
    }

    let root = std::env::var_os("S1_F05_WINDOW_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::tests::temp_root("f05-window-lifecycle"));
    fs::create_dir_all(&root).unwrap();
    let package = crate::tests::write_spatial_screenshot_runtime_package(&root);
    let screenshot = root.join("reports").join("restored.png");
    let mut request = NativePlayerWindowRunRequest::windowed(package).with_screenshot(&screenshot);
    request.config.width = 320;
    request.config.height = 240;
    request.config.target_fps = 60;
    request.config.present_mode = "Immediate".to_string();
    request.frame_limit = 2400;
    let mut app = LifecycleApp {
        inner: RealWindowApp::new(
            request,
            Arc::new(LinkedProjectRuntimeSet::explicit_empty()),
            None,
        ),
        phase: Phase::Warm,
        phase_started: Instant::now(),
        started: Instant::now(),
        size: PhysicalSize::new(320, 240),
        resized: false,
        minimized: false,
        before_frame: 0,
        cycles: Vec::new(),
    };
    EventLoop::builder()
        .with_any_thread(true)
        .build()
        .unwrap()
        .run_app(&mut app)
        .unwrap();
    let report = app.inner.report.expect("production completion report");
    fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("cycles.json"),
        serde_json::to_vec_pretty(&app.cycles).unwrap(),
    )
    .unwrap();
    assert_eq!(app.cycles.len(), 20, "all actual OS cycles must finish");
    assert_eq!(report.completion_exit_code(), 0, "{:?}", report.diagnostics);
    assert_eq!(report.frames_completed, 2400);
    assert_eq!(report.present_status, NativeWindowPresentStatus::Presented);
    assert_eq!(
        report.screenshot.status,
        NativeWindowScreenshotStatus::Captured
    );
    let mut png = png::Decoder::new(std::io::BufReader::new(
        fs::File::open(&screenshot).unwrap(),
    ))
    .read_info()
    .unwrap();
    let mut pixels = vec![0; png.output_buffer_size()];
    let image = png.next_frame(&mut pixels).unwrap();
    assert_eq!(image.color_type, png::ColorType::Rgba);
    let at = ((image.height / 2 * image.width + image.width / 2) * 4) as usize;
    assert_eq!(&pixels[at..at + 4], &[0, 0, 255, 255]);
}

#[test]
#[ignore = "local Windows window/GPU consumer; injects elapsed time without changing clock limits"]
fn real_window_terminal_clock_fault_preserves_first_failure_without_rendering_old_frame() {
    struct ClockFaultApp {
        request: NativePlayerWindowRunRequest,
        report: Option<NativeWindowHostReport>,
    }

    impl ApplicationHandler for ClockFaultApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let mut host = RealWindowHost::new(
                event_loop,
                self.request.clone(),
                Arc::new(LinkedProjectRuntimeSet::explicit_empty()),
                None,
            )
            .expect("real window and GPU must initialize");
            let deadline = Instant::now() + std::time::Duration::from_secs(30);
            loop {
                assert!(Instant::now() < deadline, "bounded fixture did not finish");
                let completed = host.report.frames_completed;
                let presented = host.report.surface.presented_frame_count;
                let prepared = host.frame_update_ns.len();
                if completed > 0 {
                    // The production presentation path caps each elapsed input at
                    // 250 ms. Repeated inputs accumulate debt under its original
                    // four-tick limit until the original overload threshold fires.
                    host.previous_presentation_started =
                        Some(Instant::now() - std::time::Duration::from_millis(250));
                }
                let advance = host.present_next_frame(Vec::new());
                if advance == RealWindowFrameAdvance::Complete {
                    assert!(completed > 0, "need a previously committed frame");
                    let fault = host
                        .report
                        .project_runtime_session_report
                        .as_ref()
                        .expect("actual clock fault must reach the Player");
                    assert!(fault.terminal_fault, "{fault:?}");
                    assert_eq!(host.report.frames_completed, completed);
                    assert_eq!(host.report.surface.presented_frame_count, presented);
                    assert_eq!(
                        host.frame_update_ns.len(),
                        prepared,
                        "terminal output entered render preparation with the old snapshot"
                    );
                    assert!(host.pending_geometry_frame.is_none());
                    assert!(host.pending_frame.is_none());
                    self.report = Some(host.finish(false));
                    event_loop.exit();
                    break;
                }
            }
        }

        fn window_event(
            &mut self,
            _: &ActiveEventLoop,
            _: winit::window::WindowId,
            _: WindowEvent,
        ) {
        }
    }

    let root = crate::tests::temp_root("terminal-clock-consumer");
    println!("owned_fixture={}", root.display());
    let package = crate::tests::write_minimal_runtime_package(&root, "runtime-package");
    let mut scene: serde_json::Value = serde_json::from_str(include_str!(
        "../../../engine_runtime/tests/fixtures/three_d_a0/runtime-scene-v2.json"
    ))
    .unwrap();
    scene["id"] = serde_json::json!("scene-main");
    scene["entities"] = serde_json::json!([scene["entities"][1].clone()]);
    fs::write(
        package.join("scenes/scene-main.json"),
        serde_json::to_vec(&scene).unwrap(),
    )
    .unwrap();
    let mut request =
        NativePlayerWindowRunRequest::windowed(package).with_frame_performance_sample(0, 32);
    request.config.width = 320;
    request.config.height = 240;
    request.config.target_fps = 0;
    request.frame_limit = 32;
    request.runtime_report_level = WindowedPlayerRuntimeReportLevel::Summary;
    let mut app = ClockFaultApp {
        request,
        report: None,
    };
    EventLoop::builder()
        .with_any_thread(true)
        .build()
        .unwrap()
        .run_app(&mut app)
        .unwrap();
    let report = app.report.expect("terminal Player completion report");
    if let Some(path) = std::env::var_os("S1_TERMINAL_EVIDENCE") {
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    assert_eq!(report.completion_exit_code(), 1);
    let fault = report.project_runtime_session_report.as_ref().unwrap();
    assert_eq!(fault.session_id, "engine.spatial_clock");
    assert_eq!(fault.status, "three_d.simulation_overloaded");
    assert!(!report
        .diagnostics
        .iter()
        .any(|d| d.code == "render.frame_identity_mismatch"));
}
