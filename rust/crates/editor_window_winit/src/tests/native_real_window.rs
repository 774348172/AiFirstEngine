#[cfg(feature = "real-window")]
use super::*;

#[cfg(feature = "real-window")]
#[test]
fn real_window_feature_can_build_native_editor_app() {
    let report = RealNativeEditorWindowReport::new("winit-wgpu");
    assert_eq!(report.backend, "winit-wgpu");
}

#[cfg(feature = "real-wgpu-surface")]
#[test]
fn preview_presented_frame_pending_ticket_reuses_exact_runtime_frame() {
    let project_root = write_editor_project_fixture_for_shell();
    let mut session = EditorSession::new();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(UiCommandPayload::Play))
            .status,
        CommandStatus::Committed
    );
    let frame = session
        .last_game_view_runtime_frame()
        .cloned()
        .expect("Play must retain its first runtime frame");
    let binding = editor_core::ProjectCandidateEntry::inspect_project_binding(&session)
        .expect("inspect Preview project binding");
    let ticket = editor_core::ProjectPreviewFrameTicket {
        schema_version: editor_core::PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION.to_string(),
        operation_id: "preview-presented-frame-window".to_string(),
        project_identity: binding.project_id,
        expected_project_digest: binding.project_digest,
        game_view_session_id: frame.session_id.clone(),
        expected_texture_id: frame.texture_id.clone(),
        expected_frame_index: frame.frame_index,
        expected_runtime_frame_hash: frame.frame_hash.clone(),
    };
    session
        .begin_project_preview_frame_capture(ticket.clone())
        .expect("begin exact-frame capture");

    let app = NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let frame_count_before = app
        .session()
        .last_game_view_present_report()
        .expect("present report before read-only getter")
        .frame_count;
    let first = app
        .active_game_view_frame_for_window_present()
        .expect("pending Preview frame");
    let second = app
        .active_game_view_frame_for_window_present()
        .expect("pending Preview frame must remain retained");

    assert_eq!(first, frame);
    assert_eq!(second, frame);
    assert_eq!(
        app.session()
            .last_game_view_present_report()
            .expect("present report after read-only getter")
            .frame_count,
        frame_count_before,
        "the window-present getter must not advance Runtime",
    );
    assert!(
        crate::real_window::real_native_editor_window::validate_preview_ticket_frame(
            &ticket, &frame
        )
        .is_ok()
    );
    let mut mismatched_ticket = ticket.clone();
    mismatched_ticket.game_view_session_id = "other-owner".to_string();
    assert_eq!(
        crate::real_window::real_native_editor_window::validate_preview_ticket_frame(
            &mismatched_ticket,
            &frame,
        ),
        Err("project_preview_evidence.game_view_session_mismatch")
    );
    mismatched_ticket = ticket.clone();
    mismatched_ticket.expected_texture_id = "other-texture".to_string();
    assert_eq!(
        crate::real_window::real_native_editor_window::validate_preview_ticket_frame(
            &mismatched_ticket,
            &frame,
        ),
        Err("project_preview_evidence.texture_mismatch")
    );
    mismatched_ticket = ticket;
    mismatched_ticket.expected_frame_index += 1;
    assert_eq!(
        crate::real_window::real_native_editor_window::validate_preview_ticket_frame(
            &mismatched_ticket,
            &frame,
        ),
        Err("project_preview_evidence.frame_index_mismatch")
    );

    let readback = editor_wgpu_renderer::EditorViewportTextureReadback {
        texture_id: frame.texture_id.clone(),
        target_id: frame.target_id.clone(),
        owner_session_id: frame.session_id.clone(),
        frame_index: frame.frame_index,
        generation: 1,
        publication_index: 1,
        frame_hash: frame.frame_hash.clone(),
        submit_serial: 1,
        width: frame.width.max(1),
        height: frame.height.max(1),
        source_format: "Rgba8Unorm".to_string(),
        rgba8: Vec::new(),
    };
    assert!(
        crate::real_window::real_native_editor_window::validate_preview_readback_frame(
            &readback, &frame
        )
        .is_ok()
    );
    let mut mismatched_readback = readback.clone();
    mismatched_readback.owner_session_id = "other-owner".to_string();
    assert_eq!(
        crate::real_window::real_native_editor_window::validate_preview_readback_frame(
            &mismatched_readback,
            &frame,
        ),
        Err("project_preview_evidence.readback_owner_mismatch")
    );
    mismatched_readback = readback.clone();
    mismatched_readback.texture_id = "other-texture".to_string();
    assert_eq!(
        crate::real_window::real_native_editor_window::validate_preview_readback_frame(
            &mismatched_readback,
            &frame,
        ),
        Err("project_preview_evidence.readback_texture_mismatch")
    );
    mismatched_readback = readback;
    mismatched_readback.frame_index += 1;
    assert_eq!(
        crate::real_window::real_native_editor_window::validate_preview_readback_frame(
            &mismatched_readback,
            &frame,
        ),
        Err("project_preview_evidence.readback_frame_index_mismatch")
    );
    drop(app);
    std::fs::remove_dir_all(project_root).expect("remove Preview window fixture");
}

#[cfg(feature = "real-wgpu-surface")]
#[test]
fn preview_presented_frame_real_wgpu_executes_runtime_plan_and_completes_receipt() {
    use editor_core::{
        AiCapabilityGrant, AiCapabilityToolKernel, AiToolExecutionStatus, AiToolInvocation,
        AiToolInvocationPayload, AiToolOutput, AiToolStartOutcome, ProjectCandidateEntry,
        ProjectPreviewCaptureKind, ProjectPreviewEvidence, ProjectPreviewFrameReadback,
        ProjectPreviewPixelFormat, AI_TOOL_INVOCATION_SCHEMA_VERSION, TOOL_ID_PROJECT_PREVIEW,
    };

    let project_root = write_editor_project_fixture_for_shell();
    let mut session = EditorSession::new();
    assert_eq!(
        session
            .execute_command(editor_core::command_for_test(
                UiCommandPayload::OpenProject {
                    path: project_root.display().to_string(),
                },
            ))
            .status,
        CommandStatus::Committed
    );
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let grant = AiCapabilityGrant::read(
        "preview-real-wgpu-window-grant",
        binding.project_id,
        binding.project_digest.clone(),
        "preview-real-wgpu-window-test",
    )
    .unwrap();
    let mut kernel = AiCapabilityToolKernel::new();
    let AiToolStartOutcome::Accepted(accepted) = kernel.start(
        &session,
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: "preview-real-wgpu-window".to_string(),
            tool_id: TOOL_ID_PROJECT_PREVIEW.to_string(),
            expected_project_digest: binding.project_digest,
            payload: AiToolInvocationPayload::Preview,
        },
        &grant,
    ) else {
        panic!("Preview must enter the asynchronous frame evidence barrier")
    };
    kernel.pump_operations(&mut session, 3);
    assert_eq!(
        kernel.observe(&accepted.operation_id).unwrap().stage,
        "awaiting_frame_evidence"
    );
    let ticket = session
        .pending_project_preview_frame_ticket()
        .cloned()
        .expect("pending Preview ticket");
    let frame = session
        .last_game_view_runtime_frame()
        .cloned()
        .expect("retained Preview runtime frame");
    let plan = session
        .active_game_view_rhi_command_plan()
        .cloned()
        .expect("retained Preview RHI command plan");

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        flags: wgpu::InstanceFlags::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
    });
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    })) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!(
                "preview_presented_frame_local_environment_unavailable:request_adapter:{error}"
            );
            drop(session);
            std::fs::remove_dir_all(project_root).expect("remove Preview window fixture");
            return;
        }
    };
    let (device, queue) =
        match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("preview-presented-frame-test-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        })) {
            Ok(pair) => pair,
            Err(error) => {
                eprintln!(
                    "preview_presented_frame_local_environment_unavailable:request_device:{error}"
                );
                drop(session);
                std::fs::remove_dir_all(project_root).expect("remove Preview window fixture");
                return;
            }
        };
    let mut viewport_textures = editor_wgpu_renderer::EditorViewportTextureRegistry::new();
    let presented = crate::real_window::real_native_editor_window::render_game_view_plan_to_exact_shared_texture(
        &device,
        &queue,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        &mut viewport_textures,
        &frame,
        &plan,
        true,
    )
    .expect("render runtime RHI plan and read back the exact shared texture");
    let receipt = presented.receipt;
    let readback = presented.readback.expect("exact shared texture readback");
    assert_eq!(readback.owner_session_id, ticket.game_view_session_id);
    assert_eq!(readback.texture_id, ticket.expected_texture_id);
    assert_eq!(readback.frame_index, ticket.expected_frame_index);
    assert_eq!(readback.generation, receipt.publication.surface_generation);
    assert_eq!(
        readback.publication_index,
        receipt.publication.publication_index
    );
    assert_eq!(readback.frame_hash, receipt.content.frame_hash);
    assert_eq!(readback.submit_serial, receipt.submit_serial);
    assert_eq!(
        readback.rgba8.len(),
        (readback.width * readback.height * 4) as usize
    );
    let evidence = session
        .record_project_preview_presented_frame(ProjectPreviewFrameReadback {
            game_view_session_id: readback.owner_session_id,
            texture_id: readback.texture_id,
            frame_index: readback.frame_index,
            width: readback.width,
            height: readback.height,
            pixel_format: ProjectPreviewPixelFormat::Rgba8Unorm,
            capture_kind: ProjectPreviewCaptureKind::RealWgpuExactSharedTexture,
            rgba8: readback.rgba8,
        })
        .expect("persist real-WGPU exact frame receipt");
    kernel.pump_operations(&mut session, 1);
    let completed = kernel.observe(&accepted.operation_id).unwrap();
    let result = completed
        .result
        .expect("Preview result after exact receipt");
    assert_eq!(result.status, AiToolExecutionStatus::Completed);
    let Some(AiToolOutput::Preview(output)) = result.output else {
        panic!("Preview must return exact frame evidence")
    };
    assert_eq!(
        output.capture_kind,
        ProjectPreviewCaptureKind::RealWgpuExactSharedTexture
    );
    let persisted = ProjectPreviewEvidence::read_frame(
        session.active_project_session().unwrap().write_scope(),
        &output.frame_evidence_ref,
    )
    .unwrap();
    assert_eq!(persisted, evidence);
    assert_eq!(
        session
            .project_preview_frame_result()
            .and_then(|receipt| receipt.captured_evidence.as_ref()),
        Some(&persisted)
    );
    drop(session);
    std::fs::remove_dir_all(project_root).expect("remove Preview window fixture");
}

#[cfg(feature = "real-window")]
#[test]
fn real_window_feature_has_native_project_folder_dialog_backend() {
    let app = crate::real_window::real_native_editor_window::RealNativeEditorApp::new(
        NativeEditorWindowConfig::default(),
        None,
    );

    assert_eq!(app.shell_report().mode, EditorUiMode::ProjectLauncher);
    assert!(app.has_panel("asset_browser"));
}

#[cfg(feature = "real-window")]
#[test]
fn real_window_feature_uses_persistent_recent_project_store() {
    let app = crate::real_window::real_native_editor_window::RealNativeEditorApp::new(
        NativeEditorWindowConfig::default(),
        None,
    );

    let store_path = app.recent_store_path().expect("recent store path");
    assert!(app.project_dialog_initial_directory().is_absolute());
    assert!(app.project_dialog_initial_directory().is_dir());
    assert_eq!(
        store_path.file_name().and_then(|name| name.to_str()),
        Some("editor_recent_projects.json")
    );
}

#[cfg(feature = "real-window")]
#[test]
fn isolated_launch_options_use_run_local_dialog_and_recent_state() {
    let root = unique_project_launcher_temp_dir();
    let picker_start = root.join("picker-start");
    std::fs::create_dir_all(&picker_start).expect("create isolated picker start");
    let options = RealNativeEditorLaunchOptions::isolated_project_launch_root(&root)
        .expect("build isolated launch options");
    let expected_recent_store = root.join("state").join("editor_recent_projects.json");
    let default_recent_store = default_native_editor_recent_store_path();

    let app =
        crate::real_window::real_native_editor_window::RealNativeEditorApp::new_with_launch_options(
            NativeEditorWindowConfig::default(),
            None,
            options,
        );

    assert_eq!(app.project_dialog_initial_directory(), picker_start);
    assert_eq!(
        app.recent_store_path(),
        Some(expected_recent_store.as_path())
    );
    assert_ne!(
        app.recent_store_path(),
        Some(default_recent_store.as_path())
    );
    std::fs::remove_dir_all(root).expect("remove isolated options fixture");
}

#[cfg(feature = "real-window")]
#[test]
fn isolated_launch_options_revalidate_before_recent_store_load() {
    let root = unique_project_launcher_temp_dir();
    std::fs::create_dir_all(root.join("picker-start")).expect("create isolated picker start");
    let options = RealNativeEditorLaunchOptions::isolated_project_launch_root(&root)
        .expect("build initially valid isolated launch options");
    std::fs::create_dir_all(root.join("state"))
        .expect("mutate isolated state after options creation");

    let error = crate::real_window::real_native_editor_window::RealNativeEditorApp::try_new_with_launch_options(
        NativeEditorWindowConfig::default(),
        None,
        options,
    )
    .err()
    .expect("state mutation must fail before recent store load");

    assert!(error.starts_with("editor_host.isolated_recent_state_not_fresh"));
    std::fs::remove_dir_all(root).expect("remove isolated revalidation fixture");
}

#[cfg(feature = "real-window")]
#[test]
fn real_window_feature_uses_native_editor_application_shell() {
    let app = crate::real_window::real_native_editor_window::RealNativeEditorApp::new(
        NativeEditorWindowConfig::default(),
        Some(fixture_model()),
    );

    let manifest = editor_ui_renderer::native_editor_panel_manifest();
    assert!(app.shell_report().panel_count >= manifest.len());
    for entry in manifest {
        assert!(
            app.has_panel(entry.panel_id),
            "real window shell is missing retained panel {}",
            entry.panel_id
        );
    }
}

#[cfg(feature = "real-window")]
#[test]
#[ignore]
fn real_native_editor_window_smoke() {
    let report = run_real_native_editor_window_with_model(fixture_model());
    assert!(
        report.present_status == "presented"
            || report.present_status == "environment_blocked"
            || report.close_requested
    );
}

#[cfg(feature = "real-window")]
#[test]
#[ignore]
fn gameview_real_gpu_texture_present() {
    let report = run_real_native_editor_window_with_model(fixture_model());
    assert!(
        report.present_status == "presented"
            || report.present_status == "environment_blocked"
            || report.close_requested
    );
    if report.present_status == "presented" {
        assert_eq!(report.shared_gpu_context_status, "Available");
        assert_ne!(report.shared_gpu_backend, "headless");
    }
}

#[cfg(feature = "real-window")]
#[test]
#[ignore]
fn real_native_editor_actual_pixel_capture() {
    let outcome =
        run_real_native_editor_capture_once(1280, 720, EditorReachabilityReportLevel::Trace);
    assert_eq!(outcome.window_report.present_status, "presented");
    assert!(outcome.scale_factor > 0.0);
    let snapshot = outcome.snapshot.expect("retained widget snapshot");
    let capture = outcome.capture.expect("actual RGBA capture");
    assert_eq!(capture.width, outcome.physical_width);
    assert_eq!(capture.height, outcome.physical_height);
    assert_eq!(
        capture.rgba8.len(),
        (capture.width * capture.height * 4) as usize
    );
    assert_eq!(snapshot.frame_index, outcome.window_report.frame_index);
    assert!(capture.rgba8.chunks_exact(4).any(|pixel| pixel[3] != 0));
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
#[test]
#[ignore]
fn real_native_editor_os_click_routes_through_widget_path() {
    let outcome = run_real_native_editor_authority(RealNativeEditorAuthorityOptions {
        physical_width: 1280,
        physical_height: 720,
        report_level: EditorReachabilityReportLevel::Trace,
        project_root: None,
        workspace_layout_store_root: None,
        click_widget_id: Some("editor/control/hit.project_launcher.create_with_ai".to_string()),
        wheel_delta: None,
        drag_target_widget_id: None,
        drag_delta: None,
        scenario_path: None,
    });

    assert_eq!(
        outcome.window_report.present_status, "presented",
        "authority diagnostics: {:#?}",
        outcome.window_report.diagnostics
    );
    let evidence = outcome.input_replay.expect("OS input evidence");
    assert_eq!(evidence.route_status, EditorReachabilityStatus::Passed);
    assert!(evidence.foreground_verified);
    assert!(evidence.pointer_down_observed);
    assert!(evidence.pointer_up_observed);
    assert_eq!(evidence.command_id.as_deref(), Some("create_with_ai"));
    assert_eq!(
        evidence.after_command_id.as_deref(),
        Some("start_create_project_with_ai")
    );
    assert_eq!(
        evidence.focused_widget_id.as_deref(),
        Some("editor/control/hit.project_launcher.create_with_ai")
    );
    assert!(outcome.capture.is_some());
}
