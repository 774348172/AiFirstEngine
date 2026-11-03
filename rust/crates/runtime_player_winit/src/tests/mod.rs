use super::*;

#[test]
fn geometry_pending_wait_is_bounded_and_never_retries_terminal_errors() {
    use engine_runtime::render_contract::{ContractError, ContractErrorCode};
    let t = Instant::now();
    let mut since = None;
    let pending = Some(ContractError::new(
        ContractErrorCode::ResourceNotReady,
        "environment.async",
    ));
    assert_eq!(defer_unready_geometry(pending, &mut since, t), Ok(true));
    assert_eq!(since, Some(t));
    assert_eq!(
        defer_unready_geometry(pending, &mut since, t + std::time::Duration::from_secs(119)),
        Ok(true)
    );
    assert_eq!(since, Some(t), "retry reset the deadline");
    assert_eq!(
        defer_unready_geometry(pending, &mut since, t + std::time::Duration::from_secs(120)),
        Err("three_d.resource_timeout")
    );
    for code in [
        ContractErrorCode::MissingCookedPayload,
        ContractErrorCode::StaleResourceHandle,
        ContractErrorCode::InvalidBinding,
    ] {
        since = Some(t);
        assert_eq!(
            defer_unready_geometry(
                Some(ContractError::new(code, "environment.async")),
                &mut since,
                t
            ),
            Ok(false)
        );
        assert!(since.is_none());
    }
    since = Some(t);
    assert_eq!(defer_unready_geometry(None, &mut since, t), Ok(false));
    assert!(since.is_none());
}

#[cfg(target_os = "windows")]
#[test]
fn windows_3d_player_admits_lit_live_leases_without_relaxing_transfer_limits() {
    use engine_runtime::runtime_asset_loader::StreamingBudget;
    let root = temp_root("windows-3d-resource-profile");
    let path = write_minimal_runtime_package(&root, "runtime-package");
    let mut package = load_runtime_package(&path).value.unwrap();
    let legacy = StreamingBudget::default();
    #[cfg(feature = "real-window")]
    assert!(windows_3d_geometry_upload_budget(&package).is_none());
    let request = NativePlayerWindowRunRequest::headless_surface_gate(&path);
    let mut report = NativeWindowHostReport::base(&request);
    let (_, hydrator) = hydrate_active_scene_for_player(&package, &mut report).unwrap();
    assert_eq!(
        hydrator.instance_loader().asset_loader().streaming_budget(),
        legacy
    );

    let mut scene: serde_json::Value = serde_json::from_str(include_str!(
        "../../../engine_runtime/tests/fixtures/three_d_a0/runtime-scene-v2.json"
    ))
    .unwrap();
    scene["id"] = serde_json::json!("scene-main");
    scene["entities"] = serde_json::json!([scene["entities"][1].clone()]);
    fs::write(
        path.join("scenes/scene-main.json"),
        serde_json::to_vec(&scene).unwrap(),
    )
    .unwrap();
    package = load_runtime_package(&path).value.unwrap();
    let (_, hydrator) = hydrate_active_scene_for_player(&package, &mut report)
        .unwrap_or_else(|| panic!("3D fixture hydration failed: {:?}", report.diagnostics));
    let budget = hydrator.instance_loader().asset_loader().streaming_budget();
    // The real ordinary lit package retains 50,614 consumer leases before IBL admission.
    assert!(
        budget.max_handles > 50_614,
        "native Player cannot admit the existing lit package: {budget:?}"
    );
    assert!(budget.max_cpu_resident_bytes >= 558_023_911);
    assert_eq!(budget.max_asset_bytes, 64 * 1024 * 1024);
    assert_eq!(budget.max_in_flight_bytes, legacy.max_in_flight_bytes);
    assert_eq!(budget.max_decode_bytes, legacy.max_decode_bytes);
    assert_eq!(budget.max_dependency_edges, legacy.max_dependency_edges);
    assert_eq!(
        budget.max_completions_per_poll,
        legacy.max_completions_per_poll
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    #[cfg(feature = "real-window")]
    {
        let gpu = windows_3d_geometry_upload_budget(&package).expect("Windows 3D GPU profile");
        assert!(
            gpu.resident_bytes >= 727_310_360,
            "native GPU profile cannot admit the existing lit package: {gpu:?}"
        );
        assert_eq!(gpu.resident_bytes, 4 * 1024 * 1024 * 1024);
        assert_eq!(gpu.staging_bytes, 128 * 1024 * 1024);
        assert_eq!(gpu.bytes_per_frame, 16 * 1024 * 1024);
        assert_eq!(gpu.max_tickets, 256);
    }
}

#[test]
fn graphics_performance_summary_excludes_warmup_and_reports_residency() {
    let counters = [(1, 63, 100), (2, 64, 240), (2, 65, 200)].map(|(builds, hits, bytes)| {
        AuiFrameWorkCounters {
            graphics_tessellation_builds: builds,
            graphics_cache_hits: hits,
            graphics_cache_resident_bytes: bytes,
            ..Default::default()
        }
    });
    let request =
        NativePlayerWindowRunRequest::windowed("package").with_frame_performance_sample(1, 2);
    let summary = summarize_aui_work(&counters, &request);
    assert_eq!(summary.graphics_tessellation_builds, 1);
    assert_eq!(summary.graphics_cache_hits, 2);
    assert_eq!(summary.graphics_cache_high_water_bytes, 240);
    let mut legacy = serde_json::to_value(&summary).unwrap();
    for key in [
        "graphicsTessellationBuilds",
        "graphicsCacheHits",
        "graphicsCacheHighWaterBytes",
    ] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    assert!(
        serde_json::from_value::<engine_runtime::windowed_player::WindowedPlayerAuiWorkSummary>(
            legacy
        )
        .is_ok()
    );
}

#[test]
fn qualification_dpi_and_gpu_bounds_preserve_cpu_window() {
    for (mode, expected) in [
        (NativeAuiPerformanceLoad::Dpi100, 1.0),
        (NativeAuiPerformanceLoad::Dpi150, 1.5),
        (NativeAuiPerformanceLoad::Dpi200, 2.0),
    ] {
        assert_eq!(mode.fixed_dpi_scale(), Some(expected));
        let decoded: NativeAuiPerformanceLoad =
            serde_json::from_str(&serde_json::to_string(&mode).unwrap()).unwrap();
        assert_eq!(decoded, mode);
    }
    assert_eq!(NativeAuiPerformanceLoad::ThemeDpi.fixed_dpi_scale(), None);
    let request =
        NativePlayerWindowRunRequest::windowed("package").with_frame_performance_sample(300, 3600);
    assert_eq!(request.performance_sample_frames, 3600);
    assert_eq!(
        gpu_performance_sample_frames(request.performance_sample_frames),
        3600
    );
    assert_eq!(gpu_performance_sample_frames(600), 600);
}

#[test]
fn allocation_frame_requires_actual_sample_window_and_real_window_mode() {
    use crate::measurement::{MeasurementLevel, MeasurementPolicy};
    let mut request = NativePlayerWindowRunRequest::windowed("package")
        .with_frame_performance_sample(2, 5)
        .with_measurement_policy(MeasurementPolicy {
            level: MeasurementLevel::Trace,
            allocation_frame: Some(3),
            ..Default::default()
        });
    request.frame_limit = 10;
    assert!(request.validate_measurement().is_ok());
    for frame in [0, 2, 8, 11] {
        request.measurement.as_mut().unwrap().allocation_frame = Some(frame);
        assert!(request.validate_measurement().is_err());
    }
    request.measurement.as_mut().unwrap().allocation_frame = Some(3);
    request.measurement.as_mut().unwrap().allocation_end_frame = Some(8);
    assert!(request.validate_measurement().is_ok());
    request.measurement.as_mut().unwrap().allocation_end_frame = Some(9);
    assert!(
        request.validate_measurement().is_err(),
        "interval tail outside sampled window"
    );
    request.measurement.as_mut().unwrap().allocation_end_frame = Some(8);
    request.frame_limit = 6;
    assert!(
        request.validate_measurement().is_err(),
        "interval tail outside run limit"
    );
    request.frame_limit = 10;
    request.mode = NativePlayerWindowRunMode::HeadlessSurfaceGate;
    assert!(request.validate_measurement().is_err());
}

#[test]
fn explicit_measurement_policy_overrides_legacy_without_changing_gameplay_reports() {
    use measurement::{MeasurementLevel, MeasurementPolicy};
    let legacy =
        NativePlayerWindowRunRequest::windowed("package").with_frame_performance_sample(1, 3);
    assert!(legacy.measurement_enabled());
    assert!(legacy.measurement_policy().native_events);
    let explicit_off = legacy
        .clone()
        .with_measurement_policy(MeasurementPolicy::default());
    assert!(!explicit_off.measurement_enabled());
    assert_eq!(
        explicit_off.runtime_report_level,
        legacy.runtime_report_level
    );
    let mut value = serde_json::to_value(&explicit_off).unwrap();
    value.as_object_mut().unwrap().remove("measurement");
    let old: NativePlayerWindowRunRequest = serde_json::from_value(value).unwrap();
    assert_eq!(old.measurement_policy(), legacy.measurement_policy());
    let summary = legacy.clone().with_measurement_policy(MeasurementPolicy {
        level: MeasurementLevel::Summary,
        ..Default::default()
    });
    assert!(summary.measurement_enabled());
    assert!(!summary.measurement_policy().native_events);
    let mut report = serde_json::to_value(NativeWindowHostReport::base(&summary)).unwrap();
    report.as_object_mut().unwrap().remove("measurementPolicy");
    let report: NativeWindowHostReport = serde_json::from_value(report).unwrap();
    assert_eq!(report.measurement_policy, None);
    let invalid = NativePlayerWindowRunRequest::windowed("package").with_measurement_policy(
        MeasurementPolicy {
            level: MeasurementLevel::Summary,
            ..Default::default()
        },
    );
    let mut rejected = NativeWindowHostReport::base(&invalid);
    assert!(!validate_run_evidence_request(&invalid, &mut rejected));
    assert!(rejected
        .diagnostics
        .iter()
        .any(|d| d.code == "measurement_requires_samples"));
    let mut oversized = summary;
    oversized.frame_limit = measurement::MAX_CAPTURE_FRAMES + 1;
    let mut rejected = NativeWindowHostReport::base(&oversized);
    assert!(!validate_run_evidence_request(&oversized, &mut rejected));
    assert!(rejected
        .diagnostics
        .iter()
        .any(|d| d.code == "measurement_frame_limit_exceeded"));
    oversized.measurement = Some(MeasurementPolicy::default());
    let mut off = NativeWindowHostReport::base(&oversized);
    assert!(validate_run_evidence_request(&oversized, &mut off));
}

#[test]
fn headless_measurement_modes_keep_aggregates_and_filter_original_frame_indices() {
    use measurement::{MeasurementLevel, MeasurementPolicy};
    let root = temp_root("measurement-policy");
    let package = write_minimal_runtime_package(&root, "runtime-package");
    for level in [
        MeasurementLevel::Off,
        MeasurementLevel::Summary,
        MeasurementLevel::Trace,
    ] {
        let policy = MeasurementPolicy {
            level,
            first_frame: (level == MeasurementLevel::Trace).then_some(2),
            end_frame: (level == MeasurementLevel::Trace).then_some(4),
            ..Default::default()
        };
        let mut request = NativePlayerWindowRunRequest::headless_surface_gate(&package)
            .with_frame_performance_sample(1, 3)
            .with_measurement_policy(policy);
        request.frame_limit = 4;
        let report = run_headless_native_player_from_package(request);
        assert_eq!(report.package_status, "ok");
        assert_eq!(report.frames_completed, 4);
        assert_eq!(report.measurement_policy, Some(policy));
        match level {
            MeasurementLevel::Off => assert!(report.frame_performance_summary.is_none()),
            _ => {
                let summary = report.frame_performance_summary.unwrap();
                assert_eq!(summary.observed_sample_frames, 3);
                assert_eq!(summary.update.observed_sample_frames, 3);
                let indices: Vec<_> = summary
                    .raw_cpu_frames
                    .iter()
                    .map(|f| f.frame_index)
                    .collect();
                assert_eq!(
                    indices,
                    if level == MeasurementLevel::Summary {
                        vec![]
                    } else {
                        vec![2, 3]
                    }
                );
            }
        }
    }
}

#[test]
fn off_frame_capture_retains_no_history_and_does_not_read_counters() {
    let request = NativePlayerWindowRunRequest::windowed("package");
    let reads = std::cell::Cell::new(0);
    let mut samples = Vec::new();
    for frame in 0..12000 {
        record_cpu_frame_sample(request.measurement_enabled(), &mut samples, || {
            reads.set(reads.get() + 1);
            frame
        });
    }
    assert!(samples.is_empty());
    assert_eq!(samples.capacity(), 0);
    assert_eq!(reads.get(), 0);
    record_cpu_frame_sample(true, &mut samples, || 12001);
    assert_eq!(samples, [12001]);
}

#[test]
fn frame_deadline_waits_only_for_unused_frame_budget() {
    let started = Instant::now();
    let finished = started + std::time::Duration::from_micros(10_000);
    assert_eq!(
        frame_deadline(started, finished).duration_since(finished),
        std::time::Duration::from_micros(6_667)
    );
}

#[test]
fn spatial_sixty_fps_does_not_accumulate_timer_wakeup_delay() {
    let origin = Instant::now();
    let mut scheduled = origin;
    let interval = std::time::Duration::from_secs_f64(1. / 60.);
    for frame in 0..600u32 {
        let started = scheduled + std::time::Duration::from_micros(400);
        let finished = started + std::time::Duration::from_millis(1);
        scheduled = next_redraw_deadline(scheduled, started, finished, 60, true);
        let expected = origin + interval * (frame + 1);
        assert!(
            scheduled.duration_since(expected) <= std::time::Duration::from_micros(1),
            "wake delay became pacing debt at frame {frame}"
        );
    }
    let late = scheduled + std::time::Duration::from_millis(100);
    assert_eq!(next_redraw_deadline(scheduled, late, late, 60, true), late);
    assert_eq!(
        next_redraw_deadline(origin, origin, origin, 60, false),
        origin + REAL_WINDOW_FRAME_INTERVAL
    );
}

#[test]
fn presentation_240fps_uses_four_millisecond_budget_and_preserves_phase() {
    let scheduled = Instant::now();
    let finished = scheduled + std::time::Duration::from_millis(2);
    let next = presentation_deadline(scheduled, finished, 240);
    assert!((next.duration_since(scheduled).as_secs_f64() - 1. / 240.).abs() < 1e-8);
    let late = scheduled + std::time::Duration::from_millis(10);
    assert_eq!(presentation_deadline(scheduled, late, 240), late);
    assert_eq!(presentation_deadline(scheduled, finished, 0), finished);
}

#[test]
fn high_presentation_rates_keep_sixty_fixed_ticks_per_second() {
    for fps in [60_u32, 120, 240, 1000] {
        let mut clock = PresentationFixedClock::default();
        let ticks: usize = (0..u64::from(fps))
            .map(|frame| {
                // Partition exactly one wall second at integer timer precision.
                let begin = frame * 1_000_000_000 / u64::from(fps);
                let end = (frame + 1) * 1_000_000_000 / u64::from(fps);
                clock.advance(std::time::Duration::from_nanos(end - begin))
            })
            .sum();
        assert_eq!(
            ticks, 60,
            "presentation rate {fps} changed simulation speed"
        );
    }
    let mut clock = PresentationFixedClock::default();
    assert_eq!(clock.advance(std::time::Duration::from_secs(10)), 15);
}

#[test]
fn press_and_release_between_fixed_ticks_delivers_press_once() {
    let mut latch = FixedActionLatch::default();
    let pressed = ActionSnapshot::with_actions(
        1,
        vec![engine_input::InputActionState::button(
            "jump",
            engine_input::ActionPhase::Pressed,
        )],
    );
    let released = ActionSnapshot::with_actions(
        2,
        vec![engine_input::InputActionState::button(
            "jump",
            engine_input::ActionPhase::Released,
        )],
    );
    latch.resolve(pressed, false);
    latch.resolve(released.clone(), false);
    assert!(latch.resolve(released.clone(), true).button_pressed("jump"));
    assert!(!latch.resolve(released, true).button_pressed("jump"));
}

#[test]
fn frame_deadline_overrun_is_ready_without_wait_or_catch_up_debt() {
    let started = Instant::now();
    for elapsed_us in [16_667, 35_000, 5_000_000] {
        let finished = started + std::time::Duration::from_micros(elapsed_us);
        assert_eq!(frame_deadline(started, finished), finished);
        let next_finished = finished + std::time::Duration::from_micros(1_000);
        assert_eq!(
            frame_deadline(finished, next_finished).duration_since(next_finished),
            std::time::Duration::from_micros(15_667)
        );
    }
}

#[cfg(feature = "real-window")]
#[test]
fn player_wgpu_backends_uses_gl_only_for_android_x86_64() {
    assert_eq!(
        real_window::player_wgpu_backends(true, true),
        wgpu::Backends::GL
    );
    assert_eq!(
        real_window::player_wgpu_backends(true, false),
        wgpu::Backends::PRIMARY
    );
    assert_eq!(
        real_window::player_wgpu_backends(false, true),
        wgpu::Backends::PRIMARY
    );
}

#[test]
fn primary_touch_mapping_preserves_physical_pixels_and_cancel_phase() {
    let started = primary_touch_raw_event(
        7,
        "android-main",
        42,
        NativePrimaryTouchPhase::Started,
        123.5,
        456.25,
    );
    let cancelled = primary_touch_raw_event(
        8,
        "android-main",
        42,
        NativePrimaryTouchPhase::Cancelled,
        124.0,
        458.0,
    );

    assert_eq!(
        started.event_kind,
        engine_input::RawInputEventKind::TouchStart
    );
    assert_eq!(
        cancelled.event_kind,
        engine_input::RawInputEventKind::TouchCancel
    );
    assert_eq!(
        started.value,
        engine_input::RawInputValue::Touch {
            touch_id: 42,
            x: 123.5,
            y: 456.25,
        }
    );
}

#[test]
fn native_mouse_leave_uses_last_cursor_position_and_deterministic_fallback() {
    for (last_position, expected) in [(Some((125.0, 240.0)), (125.0, 240.0)), (None, (0.0, 0.0))] {
        let raw = native_mouse_leave_raw_event(7, "main-window", last_position);
        let mut state = InputDeviceState::new();
        state.apply_raw_event(&raw);
        let pointer = state
            .to_runtime_input_frame(7, "main-window")
            .events
            .iter()
            .find_map(engine_input::RuntimeInputEvent::pointer_event)
            .expect("mouse leave pointer event");
        assert_eq!(
            pointer.phase,
            engine_input::input_mapping::RuntimePointerPhase::Leave
        );
        assert_eq!((pointer.x, pointer.y), expected);
    }
}

#[test]
fn suspend_resume_recreates_surface_without_recreating_gameplay_session() {
    let mut lifecycle = NativePlayerLifecycleState::default();
    lifecycle.resume();
    let session_generation = lifecycle.gameplay_session_generation;
    assert!(lifecycle.should_present());

    lifecycle.suspend();
    assert!(!lifecycle.should_present());
    lifecycle.resume();

    assert_eq!(lifecycle.surface_generation, 2);
    assert_eq!(lifecycle.gameplay_session_generation, session_generation);
}
use engine_runtime::aui::{
    AuiActionRef, AuiBindingValue, AuiCanvas, AuiComputedRect, AuiDocument, AuiImePlatformRequest,
    AuiNode, AuiNodeKind, AuiRect, ProjectUiStateIdentity, ProjectUiStateProducerContext,
    ProjectUiStateResolve, ProjectUiStateResolveError, ProjectUiStateSnapshot,
    ProjectUiStateSnapshotOutput,
};
use engine_runtime::font_bundle::RuntimeFontBundleRegistry;
use engine_runtime::font_bundle::{
    CookedFontBundleAsset, RuntimeFontFaceDescriptor, RuntimeLoadedFontBundle,
    RuntimePackageSourceFontFace, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn source_font_registry() -> RuntimeFontBundleRegistry {
    let bytes = include_bytes!("../../../../resources/editor/fonts/NotoSansSC-VF.ttf").to_vec();
    let source_digest = engine_runtime::canonical_digest::sha256_prefixed(&bytes);
    let metadata = CookedFontBundleAsset {
        schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.into(),
        font_bundle_id: "native-c2-font".into(),
        font_stack_id: "native-c2-stack".into(),
        generation: 1,
        max_bitmap_pages: 0,
        max_msdf_pages: 0,
        legacy_mode: false,
        fallback_used: false,
        quality_gate_eligible: true,
        pages: Vec::new(),
        glyphs: Vec::new(),
        kerning_adjustments: Vec::new(),
        font_faces: vec![RuntimeFontFaceDescriptor {
            font_face_id: "noto".into(),
            face_index: 0,
            source_digest: source_digest.clone(),
            payload_path: "noto.ttf".into(),
        }],
        shaping_recipe: Some(engine_runtime::text_shaping::SHAPING_RECIPE.into()),
        bundle_digest: "sha256:native-c2-font".into(),
    };
    let mut registry = RuntimeFontBundleRegistry {
        default_ui_font_bundle_id: Some("native-c2-font".into()),
        ..Default::default()
    };
    std::sync::Arc::make_mut(&mut registry.bundles_by_id).insert(
        "native-c2-font".into(),
        RuntimeLoadedFontBundle {
            metadata,
            page_payloads: Vec::new(),
            font_face_sources: vec![RuntimePackageSourceFontFace {
                font_face_id: "noto".into(),
                face_index: 0,
                source_digest,
                bytes,
            }],
        },
    );
    registry
}

#[test]
fn native_window_host_report_serializes() {
    let report = NativeWindowHostReport::base(
        &NativePlayerWindowRunRequest::headless_surface_gate("runtime-package"),
    );

    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains(NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION));
    assert!(json.contains("native-player-window-host"));
}

struct SequencedUiProducer {
    values: Vec<String>,
    call_count: usize,
}

impl ProjectUiStateSnapshotProducer for SequencedUiProducer {
    fn producer_id(&self) -> &str {
        "sequenced-test-ui"
    }

    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput {
        let value_index = self.call_count.min(self.values.len().saturating_sub(1));
        self.call_count = self.call_count.saturating_add(1);
        ProjectUiStateSnapshotOutput::new(
            self.producer_id(),
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(context.frame_index).with_value(
                "game.score_text",
                AuiBindingValue::String(self.values[value_index].clone()),
            ),
        )
    }

    fn resolve(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
        let identity = ProjectUiStateIdentity {
            producer_epoch: 1,
            visible_revision: if context.frame_index < 3 { 1 } else { 2 },
            binding_set: context.binding_set.identity().clone(),
        };
        if context.previous_identity.as_ref() == Some(&identity) {
            return Ok(ProjectUiStateResolve::Reuse { identity });
        }
        let output = self.produce(context);
        Ok(ProjectUiStateResolve::Replace { identity, output })
    }
}

struct UncacheableUiProducer {
    call_count: usize,
}

impl ProjectUiStateSnapshotProducer for UncacheableUiProducer {
    fn producer_id(&self) -> &str {
        "uncacheable-extension-test-ui"
    }

    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput {
        self.call_count = self.call_count.saturating_add(1);
        ProjectUiStateSnapshotOutput::new(
            self.producer_id(),
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(context.frame_index)
                .with_value("game.score_text", AuiBindingValue::String("FPS: 60".into())),
        )
    }
}

#[derive(Default)]
struct OscillatingExtensionHost {
    calls: usize,
    allow_publish: Vec<bool>,
}

impl engine_runtime::aui_present_cache::AuiExtensionFrameHost for OscillatingExtensionHost {
    fn retained_ui(&self) -> Option<&engine_runtime::aui_session::AuiSession> {
        None
    }

    fn refresh_ui_extensions_for_frame(
        &mut self,
        frame: &engine_runtime::aui_extension::AuiCustomFrame,
        allow_publish: bool,
    ) -> Result<engine_runtime::aui_extension::AuiCustomFrameResult, String> {
        assert_eq!(frame.document_id, "hud");
        assert_eq!(frame.nodes.len(), 1);
        self.calls = self.calls.saturating_add(1);
        self.allow_publish.push(allow_publish);
        Ok(if allow_publish {
            engine_runtime::aui_extension::AuiCustomFrameResult {
                changed: true,
                stabilization_limit_exceeded: false,
            }
        } else {
            engine_runtime::aui_extension::AuiCustomFrameResult {
                changed: false,
                stabilization_limit_exceeded: true,
            }
        })
    }
}

#[derive(Default)]
struct StableExtensionHost {
    calls: usize,
}

impl engine_runtime::aui_present_cache::AuiExtensionFrameHost for StableExtensionHost {
    fn retained_ui(&self) -> Option<&engine_runtime::aui_session::AuiSession> {
        None
    }

    fn refresh_ui_extensions_for_frame(
        &mut self,
        frame: &engine_runtime::aui_extension::AuiCustomFrame,
        _: bool,
    ) -> Result<engine_runtime::aui_extension::AuiCustomFrameResult, String> {
        assert_eq!(frame.nodes.len(), 1);
        self.calls = self.calls.saturating_add(1);
        Ok(engine_runtime::aui_extension::AuiCustomFrameResult::default())
    }
}

fn add_test_custom_node(package: &mut RuntimePackage) {
    let document = package
        .aui_documents
        .documents_by_id
        .get_mut("hud")
        .unwrap();
    document.nodes[0].children.push("custom".into());
    let mut custom = AuiNode::new(
        "custom",
        AuiNodeKind::Custom,
        AuiRect::fixed_position(100.0, 100.0, 80.0, 80.0),
    )
    .with_parent("root");
    custom.custom = Some(engine_runtime::aui_extension::AuiCustomNodeSpec {
        type_id: "sample.oscillating".into(),
        schema_version: 1,
        properties: std::collections::BTreeMap::new(),
    });
    document.nodes.push(custom);
}

struct VirtualListUiProducer {
    call_count: usize,
}

impl ProjectUiStateSnapshotProducer for VirtualListUiProducer {
    fn producer_id(&self) -> &str {
        "virtual-list-test-ui"
    }

    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput {
        use engine_runtime::aui_virtual_list::{AuiVirtualListItem, AuiVirtualListSnapshot};

        self.call_count = self.call_count.saturating_add(1);
        let first_index = usize::from(context.frame_index >= 3);
        let items = (first_index..first_index + 10_000)
            .map(|index| {
                AuiVirtualListItem::new(format!("item-{index}"), None)
                    .with_text(format!("道具 {index} / العربية"))
            })
            .collect();
        let mut snapshot = ProjectUiStateSnapshot::new(context.frame_index);
        snapshot
            .virtual_lists
            .insert("inventory.items".into(), AuiVirtualListSnapshot::new(items));
        ProjectUiStateSnapshotOutput::new(
            self.producer_id(),
            AuiSnapshotSource::TestSnapshot,
            snapshot,
        )
    }

    fn resolve(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
        let identity = ProjectUiStateIdentity {
            producer_epoch: 1,
            visible_revision: if context.frame_index < 3 { 1 } else { 2 },
            binding_set: context.binding_set.identity().clone(),
        };
        if context.previous_identity.as_ref() == Some(&identity) {
            return Ok(ProjectUiStateResolve::Reuse { identity });
        }
        let output = self.produce(context);
        Ok(ProjectUiStateResolve::Replace { identity, output })
    }
}

#[test]
fn aui_present_cache_stops_oscillation_after_eight_rounds_without_final_publish() {
    let root = temp_root("aui-extension-stabilization");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let mut package = load_runtime_package(package_dir).value.unwrap();
    add_test_custom_node(&mut package);

    let world = World::new();
    let mut producer = UncacheableUiProducer { call_count: 0 };
    let mut host = OscillatingExtensionHost::default();
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();
    let output = cache
        .take_or_rebuild_stabilized(
            &package,
            &world,
            1,
            &mut producer,
            &mut host,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("the last valid frame remains presentable");

    assert_eq!(host.calls, 8);
    assert_eq!(producer.call_count, 8);
    assert_eq!(
        host.allow_publish,
        vec![true, true, true, true, true, true, true, false]
    );
    assert_eq!(output.report.status, AuiRuntimePresentStatus::Partial);
    assert!(output
        .report
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "aui_extension.stabilization_limit_exceeded" }));
}

#[test]
fn aui_present_cache_clean_hit_skips_extension_frame_scan_and_callback() {
    let root = temp_root("aui-extension-clean-hit");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let mut package = load_runtime_package(package_dir).value.unwrap();
    add_test_custom_node(&mut package);

    let world = World::new();
    let mut producer = SequencedUiProducer {
        values: vec!["FPS: 60".into()],
        call_count: 0,
    };
    let mut host = StableExtensionHost::default();
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();
    let first = cache
        .take_or_rebuild_stabilized(
            &package,
            &world,
            1,
            &mut producer,
            &mut host,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .unwrap();
    assert_eq!(host.calls, 1);
    cache.store(Some(first));

    let second = cache
        .take_or_rebuild_stabilized(
            &package,
            &world,
            2,
            &mut producer,
            &mut host,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .unwrap();
    assert_eq!(host.calls, 1, "clean cache hit must not cross the DLL");
    assert_eq!(cache.hit_count, 1);
    cache.store(Some(second));
}

#[test]
fn aui_present_cache_materializes_bounded_virtual_list_and_reconciles_recycled_slots() {
    use engine_runtime::aui::{AuiClipPolicy, AuiScrollState};
    use engine_runtime::aui_layout_schema::{
        Dimension, LayoutMode, LayoutSpec, LayoutVisibility, SlotSpec, AUI_LAYOUT_SCHEMA_VERSION,
    };
    use engine_runtime::aui_virtual_list::{AuiVirtualListDeclaration, AuiVirtualListExtentMode};

    let root = temp_root("aui-virtual-list-present-cache");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let load = load_runtime_package(package_dir);
    assert!(!load.diagnostics.has_errors(), "{:#?}", load.diagnostics);
    let mut package = load.value.expect("minimal AUI runtime package");
    let document = package
        .aui_documents
        .documents_by_id
        .get_mut("hud")
        .unwrap();
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document.virtual_lists = vec![AuiVirtualListDeclaration {
        list_node_id: "inventory".into(),
        data_path: "inventory.items".into(),
        item_template_node_id: "item-template".into(),
        overscan: 96.0,
        max_live_instances: 24,
        extent_mode: AuiVirtualListExtentMode::Fixed { item_extent: 48.0 },
    }];
    let mut root_node = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["inventory"]);
    root_node.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    let mut inventory = AuiNode::new(
        "inventory",
        AuiNodeKind::List,
        AuiRect::fixed_position(20.0, 20.0, 400.0, 240.0),
    )
    .with_parent("root")
    .with_children(["item-template"]);
    inventory.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    inventory.slot = Some(SlotSpec {
        width: Dimension::Fixed { value: 400.0 },
        height: Dimension::Fixed { value: 240.0 },
        ..Default::default()
    });
    inventory.clip_policy = AuiClipPolicy::Rect;
    let mut template = AuiNode::new(
        "item-template",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 400.0, 48.0),
    )
    .with_parent("inventory")
    .with_children(["item-label"])
    .with_interactable(true)
    .with_action(AuiActionRef::activate("inventory.select"));
    template.layout_visibility = Some(LayoutVisibility::Collapsed);
    template.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    let label = AuiNode::new(
        "item-label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(8.0, 8.0, 384.0, 32.0),
    )
    .with_parent("item-template");
    document.nodes = vec![root_node, inventory, template, label];
    package.font_bundles = source_font_registry();

    let world = World::new();
    let mut producer = VirtualListUiProducer { call_count: 0 };
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();

    let first = cache
        .take_or_rebuild(
            &package,
            &world,
            1,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("first virtual-list present");
    let first_window = &first.virtual_lists[0];
    let initial_instance_count = first_window.instances.len() as u64;
    assert_eq!(first_window.total_item_count, 10_000);
    assert!(first_window.instances.len() <= 24);
    assert!(first.resolved_document.nodes.len() <= 4 + 24 * 2 + 1);
    assert!(first.resolved_document.nodes.iter().any(|node| {
        node.text
            .as_deref()
            .is_some_and(|text| text.contains("道具") && text.contains("العربية"))
    }));
    let first_instance = first_window.instances[0].clone();
    let first_layout = first
        .layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == first_instance.node_id)
        .expect("materialized item participates in the committed layout");
    assert!(first_layout.effective_visible);
    assert!(first_layout.rect.width > 0.0);
    assert!(first_layout.rect.height > 0.0);
    let initial_lifecycle = cache.virtual_list_lifecycle_stats();
    assert_eq!(
        initial_lifecycle.materialized_instance_count,
        initial_instance_count
    );
    assert_eq!(initial_lifecycle.recycled_instance_count, 0);
    assert_eq!(initial_lifecycle.disposed_instance_count, 0);
    let mut click = engine_runtime::input_mapping::RuntimeInputFrame::new(1, "test");
    click.events = vec![
        engine_runtime::input_mapping::RuntimeInputEvent::PointerMove {
            x: first_layout.rect.x + 2.0,
            y: first_layout.rect.y + first_layout.rect.height - 2.0,
        },
        engine_runtime::input_mapping::RuntimeInputEvent::PointerDown {
            x: first_layout.rect.x + 2.0,
            y: first_layout.rect.y + first_layout.rect.height - 2.0,
            button: engine_runtime::input_mapping::RuntimePointerButton::Primary,
        },
        engine_runtime::input_mapping::RuntimeInputEvent::PointerUp {
            x: first_layout.rect.x + 2.0,
            y: first_layout.rect.y + first_layout.rect.height - 2.0,
            button: engine_runtime::input_mapping::RuntimePointerButton::Primary,
        },
    ];
    let click = AuiInteractionSystem::process_with_state(
        &first.resolved_document,
        &first.layout,
        &click,
        &mut interaction,
        engine_runtime::aui::AuiInteractionConfig::default(),
    );
    assert!(
        click.actions.iter().any(|action| {
            action.action_id == "inventory.select"
                && action
                    .payload
                    .as_deref()
                    .is_some_and(|payload| payload.contains("item-0"))
        }),
        "{click:#?}"
    );
    let focused_descendant = format!("{}:item-label", first_instance.node_id);
    cache.store(Some(first));

    interaction.focus.focused_node = Some(focused_descendant);
    interaction.scroll_offsets.insert(
        "inventory".into(),
        AuiScrollState {
            node_id: "inventory".into(),
            offset_y: 240_000.0,
            max_offset_y: 479_760.0,
            last_delta_y: 240_000.0,
        },
    );
    let second = cache
        .take_or_rebuild(
            &package,
            &world,
            2,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("scrolled virtual-list present");
    let second_window = &second.virtual_lists[0];
    assert!(second_window.first_materialized_index.unwrap() > 4_000);
    assert_eq!(second_window.focused_key.as_deref(), Some("item-0"));
    assert!(second_window.reconciliation.rebound_instance_count > 0);
    assert_eq!(second_window.reconciliation.cleared_focus_count, 1);
    assert!(interaction.focus.focused_node.is_none());
    let rebound = second_window
        .instances
        .iter()
        .find(|instance| instance.slot_index == first_instance.slot_index)
        .expect("the bounded slot should be reused at the remote range");
    assert_ne!(rebound.item_key, first_instance.item_key);
    let scrolled_lifecycle = cache.virtual_list_lifecycle_stats();
    assert!(scrolled_lifecycle.materialized_instance_count > initial_instance_count);
    assert!(scrolled_lifecycle.recycled_instance_count > 0);
    assert_eq!(
        scrolled_lifecycle.disposed_instance_count,
        scrolled_lifecycle.recycled_instance_count
    );
    assert_eq!(producer.call_count, 1, "scroll rebuild reuses the snapshot");
    cache.store(Some(second));

    let third = cache
        .take_or_rebuild(
            &package,
            &world,
            3,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("changed virtual-list snapshot present");
    assert_eq!(third.virtual_lists[0].total_item_count, 10_000);
    assert!(third.virtual_lists[0].focused_key.is_none());
    assert_eq!(producer.call_count, 2);
    cache.store(Some(third));

    let fourth = cache
        .take_or_rebuild(
            &package,
            &world,
            4,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("unchanged virtual-list present");
    assert_eq!(fourth.virtual_lists[0].total_item_count, 10_000);
    assert_eq!(cache.rebuild_count, 3);
    assert_eq!(cache.hit_count, 1);
}

#[test]
fn aui_present_cache_reuses_clean_frame_and_rebuilds_changed_values() {
    let root = temp_root("aui-present-cache");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let load = load_runtime_package(package_dir);
    assert!(!load.diagnostics.has_errors(), "{:#?}", load.diagnostics);
    let package = load.value.expect("minimal AUI runtime package");
    let world = World::new();
    let mut producer = SequencedUiProducer {
        values: vec!["FPS: 60".to_string(), "FPS: 59".to_string()],
        call_count: 0,
    };
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();

    let first = cache
        .take_or_rebuild(
            &package,
            &world,
            1,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("first present");
    let first_composition = first.composition.clone();
    cache.store(Some(first));
    let second = cache
        .take_or_rebuild(
            &package,
            &world,
            2,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("cached present");
    assert_eq!(first_composition, second.composition);
    assert_eq!(cache.rebuild_count, 1);
    assert_eq!(cache.hit_count, 1);
    assert_eq!(cache.presentation_revision, 1);

    let second_document = second.resolved_document.clone();
    cache.store(Some(second));
    let third = cache
        .take_or_rebuild(
            &package,
            &world,
            3,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("changed present");
    assert_ne!(second_document, third.resolved_document);
    assert_eq!(producer.call_count, 2);
    assert_eq!(cache.rebuild_count, 2);
    assert_eq!(cache.hit_count, 1);
    assert_eq!(cache.presentation_revision, 2);
}

#[test]
fn aui_present_cache_respects_binding_ownership_takeover_and_release() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_game_sdk::aui::*;
    use std::sync::Arc;

    let root = temp_root("aui-binding-ownership-present-cache");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let mut package = load_runtime_package(package_dir).value.unwrap();
    package.font_bundles = source_font_registry();
    let source = Arc::new(
        package
            .aui_documents
            .documents_by_id
            .get("hud")
            .unwrap()
            .clone(),
    );
    let mut instance = AuiInstance::mount(source).unwrap();
    instance.drain_invalidations(&mut Vec::new());
    let score = instance.node("score").unwrap();
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();
    let mut producer = SequencedUiProducer {
        values: vec!["绑定A".into(), "绑定B".into()],
        call_count: 0,
    };
    let world = World::new();

    let present = |cache: &mut NativeAuiPresentCache,
                   instance: &AuiInstance,
                   interaction: &mut AuiInteractionState,
                   producer: &mut SequencedUiProducer,
                   frame_index| {
        let mut context = NativeAuiPresentContext::new(None, None, interaction);
        context.retained_instance = Some(instance);
        cache
            .take_or_rebuild(&package, &world, frame_index, producer, context)
            .unwrap()
    };
    let text = |output: &engine_runtime::aui::AuiRuntimePresentOutput| {
        output
            .resolved_document
            .nodes
            .iter()
            .find(|node| node.node_id == "score")
            .and_then(|node| node.text.as_deref())
            .unwrap()
            .to_string()
    };

    let initial = present(&mut cache, &instance, &mut interaction, &mut producer, 1);
    assert_eq!(text(&initial), "绑定A");
    cache.store(Some(initial));

    let mut edit = instance.edit();
    edit.transfer_property_ownership(score, UiProperty::Text, UiPropertyOwner::Controller)
        .unwrap();
    edit.write(score, UiProperty::Text, UiValue::Text("控制器".into()))
        .unwrap();
    edit.commit().unwrap();
    let controlled = present(&mut cache, &instance, &mut interaction, &mut producer, 2);
    assert_eq!(text(&controlled), "控制器");
    cache.store(Some(controlled));

    let changed_snapshot = present(&mut cache, &instance, &mut interaction, &mut producer, 3);
    assert_eq!(text(&changed_snapshot), "控制器");
    cache.store(Some(changed_snapshot));

    let mut edit = instance.edit();
    edit.transfer_property_ownership(score, UiProperty::Text, UiPropertyOwner::Binding)
        .unwrap();
    edit.commit().unwrap();
    let released = present(&mut cache, &instance, &mut interaction, &mut producer, 3);
    assert_eq!(text(&released), "绑定B");
    assert_eq!(producer.call_count, 2, "release reuses the latest snapshot");
}

#[test]
fn aui_present_cache_retained_edits_bypass_snapshot_reuse_without_overwriting_bindings() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_game_sdk::aui::*;
    use std::sync::Arc;
    let root = temp_root("aui-retained-present-cache");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let mut package = load_runtime_package(package_dir).value.unwrap();
    package.font_bundles = source_font_registry();
    for schema in [
        engine_runtime::aui::AUI_DOCUMENT_SCHEMA_VERSION,
        engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION,
    ] {
        let document = package
            .aui_documents
            .documents_by_id
            .get_mut("hud")
            .unwrap();
        document.schema_version = schema.into();
        if document.nodes.len() == 2 {
            document.nodes[0].children.push("unit".into());
            let mut unit = AuiNode::new(
                "unit",
                AuiNodeKind::Text,
                AuiRect::fixed_position(30., 100., 200., 40.),
            )
            .with_parent("root")
            .with_text("人")
            .with_interactable(true)
            .with_action(AuiActionRef::activate("unit.select"));
            unit.style = Some(engine_runtime::aui::AuiStyle::text("#fff", 20.));
            document.nodes.push(unit);
            for index in 0..1000 {
                let node_id = format!("unrelated-{index}");
                document.nodes[0].children.push(node_id.clone());
                document.nodes.push(
                    AuiNode::new(
                        node_id,
                        AuiNodeKind::Panel,
                        AuiRect::fixed_position(0., 0., 1., 1.),
                    )
                    .with_parent("root"),
                );
            }
        }
        let source = Arc::new(document.clone());
        let mut instance = AuiInstance::mount(source.clone()).unwrap();
        let unit = instance.node("unit").unwrap();
        let mut cache = NativeAuiPresentCache::for_package(&package);
        let mut interaction = AuiInteractionState::default();
        let mut producer = SequencedUiProducer {
            values: vec!["粮草100".into(), "粮草99".into()],
            call_count: 0,
        };
        let world = World::new();
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&instance);
        let initial = cache
            .take_or_rebuild(&package, &world, 1, &mut producer, context)
            .unwrap();
        let rect = initial
            .layout
            .computed_nodes
            .iter()
            .find(|n| n.node_id == "unit")
            .unwrap()
            .rect;
        let layout_nodes_ptr = initial.layout.computed_nodes.as_ptr();
        cache.store(Some(initial));
        let mut edit = instance.edit();
        edit.write(unit, UiProperty::Text, UiValue::Text("骑兵".into()))
            .unwrap();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([100., 20.]),
        )
        .unwrap();
        edit.commit().unwrap();
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&instance);
        let updated = cache
            .take_or_rebuild(&package, &world, 2, &mut producer, context)
            .unwrap();
        assert_eq!(
            producer.call_count, 1,
            "typed updates must not need a new snapshot"
        );
        assert_eq!(cache.rebuild_count, 2);
        assert_eq!(
            updated.resolved_document.nodes[1].text.as_deref(),
            Some("粮草100")
        );
        if schema == engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION {
            let canvas_id = &updated.resolved_document.canvases[0].canvas_id;
            assert_eq!(
                cache
                    .c2_runtime()
                    .layout_stats(canvas_id)
                    .unwrap()
                    .geometry_nodes,
                1
            );
            assert_eq!(
                cache
                    .c2_runtime()
                    .geometry_update_stats()
                    .source_nodes_compared,
                1,
                "the scheduler-owned retained edit must not compare the full source"
            );
            assert_eq!(
                updated.layout.computed_nodes.as_ptr(),
                layout_nodes_ptr,
                "dropping the superseded cache output must allow in-place COW layout update"
            );
        }
        assert_eq!(
            updated.resolved_document.nodes[2].text.as_deref(),
            Some("骑兵")
        );
        let moved = updated
            .layout
            .computed_nodes
            .iter()
            .find(|n| n.node_id == "unit")
            .unwrap()
            .rect;
        assert_eq!((moved.x, moved.y), (rect.x + 100., rect.y + 20.));
        assert_eq!(
            AuiInteractionSystem::hit_test(
                &updated.resolved_document,
                &updated.layout,
                moved.x + 10.,
                moved.y + 10.
            )
            .hit_node
            .as_deref(),
            Some("unit")
        );
        let mut device = InputDeviceState::new();
        for (x, y, expected) in [
            (rect.x + 10., rect.y + 10., 0),
            (moved.x + 10., moved.y + 10., 1),
        ] {
            let events = vec![
                RawInputEvent::mouse_move(2, "retained", x, y),
                RawInputEvent::mouse_button_down(
                    2,
                    "retained",
                    engine_input::RuntimePointerButton::Primary,
                ),
                RawInputEvent::mouse_button_up(
                    2,
                    "retained",
                    engine_input::RuntimePointerButton::Primary,
                ),
            ];
            let (_, _, _, result) = resolve_native_input_frame_with_aui(
                &mut device,
                &InputMappingAsset::gameplay_default(),
                "retained-test",
                None,
                &events,
                2,
                "retained",
                "headless",
                "windows",
                Some(&updated),
                None,
                &mut interaction,
            );
            assert_eq!(
                result
                    .unwrap()
                    .result
                    .actions
                    .iter()
                    .filter(|action| action.action_id == "unit.select")
                    .count(),
                expected
            );
        }
        let draw = updated.composition.clone();
        cache.store(Some(updated));
        // Same value is a no-op, including the consumer's presentation identity.
        let key = instance.presentation_key();
        let mut edit = instance.edit();
        edit.write(unit, UiProperty::Text, UiValue::Text("骑兵".into()))
            .unwrap();
        assert_eq!(edit.commit().unwrap(), 0);
        assert_eq!(instance.presentation_key(), key);
        for _ in 0..8 {
            let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
            context.retained_instance = Some(&instance);
            let clean = cache
                .take_or_rebuild(&package, &world, 2, &mut producer, context)
                .unwrap();
            assert_eq!(clean.composition, draw);
            cache.store(Some(clean));
        }
        assert_eq!(cache.rebuild_count, 2);
        assert_eq!(cache.hit_count, 8);
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&instance);
        let binding_changed = cache
            .take_or_rebuild(&package, &world, 3, &mut producer, context)
            .unwrap();
        assert_eq!(
            binding_changed.resolved_document.nodes[1].text.as_deref(),
            Some("粮草99")
        );
        assert_eq!(
            binding_changed.resolved_document.nodes[2].text.as_deref(),
            Some("骑兵")
        );
        assert!(cache.c2_runtime().prepared_input_reused());
        assert_eq!(cache.c2_runtime().prepared_binding_nodes_rebuilt(), 1);
        let prepared_document = binding_changed.resolved_document.clone();
        let drawing_before_translation = cache.c2_runtime().drawing_stats();
        cache.store(Some(binding_changed));
        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([120., 20.]),
        )
        .unwrap();
        edit.commit().unwrap();
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&instance);
        let translated = cache
            .take_or_rebuild(&package, &world, 3, &mut producer, context)
            .unwrap();
        assert!(cache.c2_runtime().prepared_input_reused());
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        let drawing_after_translation = cache.c2_runtime().drawing_stats();
        assert!(matches!(
            translated.base_draw_report(),
            std::borrow::Cow::Borrowed(_)
        ));
        let summary = NativeAuiPresentSummary::from_present_output(&package, &translated, "main");
        let expected =
            AuiLayoutEngine::extract_draw_list(&translated.resolved_document, &translated.layout).1;
        assert_eq!(
            summary.aui_effective_clip_item_count,
            expected.effective_clip_item_count
        );
        assert_eq!(
            summary.aui_culled_draw_item_count,
            expected.culled_draw_item_count
        );
        assert_eq!(
            summary.aui_scrollbar_visible_count,
            expected.scrollbar_visible_count
        );
        assert_eq!(
            drawing_after_translation.index_rebuilds,
            drawing_before_translation.index_rebuilds
        );
        assert_eq!(
            drawing_after_translation.index_entries_built,
            drawing_before_translation.index_entries_built
        );
        assert!(Arc::ptr_eq(
            &translated.overlay.draw_items,
            &translated.composition.stages[0].draw_items
        ));
        assert_eq!(
            drawing_after_translation.nodes_built - drawing_before_translation.nodes_built,
            1
        );
        if schema == engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION {
            assert_eq!(
                drawing_after_translation.nodes_compared
                    - drawing_before_translation.nodes_compared,
                1,
                "a one-node retained translation must compare only its drawing fragment"
            );
        }
        assert!(drawing_after_translation.nodes_reused > drawing_before_translation.nodes_reused);
        assert!(Arc::ptr_eq(
            &prepared_document,
            &translated.resolved_document
        ));
        if schema == engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION {
            let stats = cache.c2_runtime().geometry_update_stats();
            assert!(stats.base_reused);
            assert_eq!(stats.nodes_updated, 1);
        }
        assert_eq!(
            translated
                .layout
                .computed_nodes
                .iter()
                .find(|n| n.node_id == "unit")
                .unwrap()
                .rect
                .x,
            rect.x + 120.
        );
        let prepared_nodes_ptr = translated.resolved_document.nodes.as_ptr();
        drop(prepared_document);
        cache.store(Some(translated));
        let mut edit = instance.edit();
        edit.write(unit, UiProperty::Text, UiValue::Text("弩兵".into()))
            .unwrap();
        edit.commit().unwrap();
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&instance);
        let in_place = cache
            .take_or_rebuild(&package, &world, 3, &mut producer, context)
            .unwrap();
        assert!(cache.c2_runtime().prepared_input_reused());
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        assert_eq!(
                in_place.resolved_document.nodes.as_ptr(),
                prepared_nodes_ptr,
                "without an external old output, a one-node retained edit must not copy the 1003-node prepared document"
            );
        assert_eq!(
            in_place.resolved_document.nodes[2].text.as_deref(),
            Some("弩兵")
        );
        cache.store(Some(in_place));
        // Typed content deltas reuse the prepared binding/theme/viewport result. The
        // resolved document remains COW so an output retained by the caller is immutable.
        for (property, value) in [
            (UiProperty::Text, UiValue::Text("弓兵".into())),
            (UiProperty::Interactable, UiValue::Bool(false)),
            (
                UiProperty::Visibility,
                UiValue::Visibility(UiVisibility::Hidden),
            ),
            (
                UiProperty::Visibility,
                UiValue::Visibility(UiVisibility::Visible),
            ),
        ] {
            let old = cache.current().unwrap().resolved_document.clone();
            let old_unit = old.nodes[2].clone();
            let mut edit = instance.edit();
            edit.write(unit, property, value).unwrap();
            edit.commit().unwrap();
            let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
            context.retained_instance = Some(&instance);
            let changed = cache
                .take_or_rebuild(&package, &world, 3, &mut producer, context)
                .unwrap();
            assert!(cache.c2_runtime().prepared_input_reused());
            assert_eq!(
                cache.c2_runtime().prepared_document_nodes_copied(),
                old.nodes.len()
            );
            assert!(!Arc::ptr_eq(&old, &changed.resolved_document));
            assert_eq!(
                old.nodes[2], old_unit,
                "the caller's old frame is immutable"
            );
            assert_eq!(
                changed.resolved_document.nodes[2].text.as_deref(),
                Some("弓兵")
            );
            cache.store(Some(changed));
        }
        let mut edit = instance.edit();
        edit.write(unit, UiProperty::Active, UiValue::Bool(false))
            .unwrap();
        edit.commit().unwrap();
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&instance);
        let inactive = cache
            .take_or_rebuild(&package, &world, 3, &mut producer, context)
            .unwrap();
        assert!(inactive
            .layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "unit")
            .is_none_or(|node| !node.effective_visible));
        assert_ne!(
            AuiInteractionSystem::hit_test(
                &inactive.resolved_document,
                &inactive.layout,
                rect.x + 130.,
                rect.y + 30.,
            )
            .hit_node
            .as_deref(),
            Some("unit")
        );
        cache.store(Some(inactive));
        let mut edit = instance.edit();
        edit.write(unit, UiProperty::Active, UiValue::Bool(true))
            .unwrap();
        edit.commit().unwrap();
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&instance);
        let reenabled = cache
            .take_or_rebuild(&package, &world, 3, &mut producer, context)
            .unwrap();
        let reenabled_source = reenabled
            .resolved_document
            .nodes
            .iter()
            .find(|node| node.node_id == "unit")
            .unwrap();
        assert!(reenabled_source.visible);
        assert_ne!(
            reenabled_source.layout_visibility,
            Some(engine_runtime::aui_layout_schema::LayoutVisibility::Collapsed)
        );
        assert!(
            reenabled
                .layout
                .computed_nodes
                .iter()
                .find(|node| node.node_id == "unit")
                .unwrap_or_else(|| panic!("reenabled unit missing for schema {schema}"))
                .effective_visible
        );
        cache.store(Some(reenabled));
        let replacement = AuiInstance::mount(source.clone()).unwrap();
        let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(&replacement);
        let replaced = cache
            .take_or_rebuild(&package, &world, 4, &mut producer, context)
            .unwrap();
        assert!(!cache.c2_runtime().prepared_input_reused());
        assert_eq!(
            replaced.resolved_document.nodes[2].text.as_deref(),
            Some("人")
        );
        assert_eq!(
            replaced
                .layout
                .computed_nodes
                .iter()
                .find(|n| n.node_id == "unit")
                .unwrap()
                .rect,
            rect
        );
        assert_eq!(source.nodes[2].text.as_deref(), Some("人"));
    }
}

#[test]
fn aui_present_cache_rebuilds_for_viewport_resize_and_dpi_then_reuses() {
    use engine_runtime::aui::AuiScaleMode;
    use engine_runtime::aui_layout_schema::{LayoutMode, LayoutSpec, AUI_LAYOUT_SCHEMA_VERSION};
    use engine_runtime::aui_viewport::{AuiSafeAreaPolicy, AuiViewportLayoutSpec};

    let root = temp_root("aui-viewport-present-cache");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let load = load_runtime_package(package_dir);
    assert!(!load.diagnostics.has_errors(), "{:#?}", load.diagnostics);
    let mut package = load.value.expect("minimal AUI runtime package");
    let document = package
        .aui_documents
        .documents_by_id
        .get_mut("hud")
        .unwrap();
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.to_string();
    document.canvases[0].scale_mode = AuiScaleMode::ViewportLayout;
    document.canvases[0].viewport_layout = Some(AuiViewportLayoutSpec {
        safe_area_policy: AuiSafeAreaPolicy::InsetRoot,
        conditions: Vec::new(),
    });
    document.nodes[0].layout = Some(LayoutSpec {
        mode: LayoutMode::Column,
        ..Default::default()
    });

    // Viewport C2 layout measures the bound text; it requires a real font bundle.
    package.font_bundles = source_font_registry();
    // Theme edits in this test use the package document as the source.

    let world = World::new();
    let mut producer = SequencedUiProducer {
        values: vec!["FPS: 60".to_string()],
        call_count: 0,
    };
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();
    let landscape = native_window_viewport_input(1200, 800, 1.0);
    let first = cache
        .take_or_rebuild(&package, &world, 1, &mut producer, {
            let context = NativeAuiPresentContext::new(None, Some(&landscape), &mut interaction);
            context
        })
        .expect("first viewport present");
    let first_root = first
        .layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "root")
        .unwrap()
        .rect;
    assert_eq!((first_root.width, first_root.height), (1200.0, 800.0));
    assert_eq!(first.report.viewport_status, "resolved");
    cache.store(Some(first));

    let hidpi = native_window_viewport_input(1200, 800, 2.0);
    let second = cache
        .take_or_rebuild(&package, &world, 2, &mut producer, {
            let context = NativeAuiPresentContext::new(None, Some(&hidpi), &mut interaction);
            context
        })
        .expect("DPI-changed viewport present");
    let second_root = second
        .layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "root")
        .unwrap()
        .rect;
    assert_eq!((second_root.width, second_root.height), (600.0, 400.0));
    assert_eq!(producer.call_count, 1, "viewport rebuild reuses UI values");
    assert_eq!(cache.rebuild_count, 2);
    cache.store(Some(second));

    let third = cache
        .take_or_rebuild(&package, &world, 2, &mut producer, {
            let context = NativeAuiPresentContext::new(None, Some(&hidpi), &mut interaction);
            context
        })
        .expect("stable viewport present");
    assert_eq!(third.layout.computed_nodes[0].rect, second_root);
    assert_eq!(cache.hit_count, 1);
    assert_eq!(cache.rebuild_count, 2);
    cache.store(Some(third));

    let alternate = cache
        .take_or_rebuild(&package, &world, 2, &mut producer, {
            let context = NativeAuiPresentContext::new(None, Some(&landscape), &mut interaction);
            context
        })
        .expect("previously resolved viewport variant");
    assert_eq!(alternate.layout.computed_nodes[0].rect, first_root);
    assert_eq!(producer.call_count, 1);
    assert_eq!(
        cache.rebuild_count, 2,
        "A->B->A must reuse bounded variants"
    );
    assert_eq!(cache.hit_count, 2);
    cache.store(Some(alternate));

    package
        .aui_documents
        .documents_by_id
        .get_mut("hud")
        .unwrap()
        .theme = Some(engine_runtime::runtime_package::RuntimeAssetRef {
        id: engine_runtime::aui_theme::BUILTIN_AUI_THEME_ID.to_string(),
        asset_type: "aui-theme".to_string(),
        guid: None,
        sub_asset: None,
    });
    let themed = cache
        .take_or_rebuild(&package, &world, 2, &mut producer, {
            let context = NativeAuiPresentContext::new(None, Some(&landscape), &mut interaction);
            context
        })
        .expect("theme-only rebuild");
    assert_eq!(cache.rebuild_count, 3);
    cache.store(Some(themed));

    package
        .aui_documents
        .documents_by_id
        .get_mut("hud")
        .unwrap()
        .theme = None;
    let original_theme = cache
        .take_or_rebuild(&package, &world, 2, &mut producer, {
            let context = NativeAuiPresentContext::new(None, Some(&landscape), &mut interaction);
            context
        })
        .expect("previously resolved theme variant");
    assert_eq!(original_theme.layout.computed_nodes[0].rect, first_root);
    assert_eq!(producer.call_count, 1);
    assert_eq!(cache.rebuild_count, 3);
    assert_eq!(cache.hit_count, 3);
    cache.store(Some(original_theme));

    package
        .aui_documents
        .documents_by_id
        .get_mut("hud")
        .unwrap()
        .theme = Some(engine_runtime::runtime_package::RuntimeAssetRef {
        id: engine_runtime::aui_theme::BUILTIN_AUI_THEME_ID.to_string(),
        asset_type: "aui-theme".to_string(),
        guid: None,
        sub_asset: None,
    });
    let repeated_theme = cache
        .take_or_rebuild(&package, &world, 2, &mut producer, {
            let context = NativeAuiPresentContext::new(None, Some(&landscape), &mut interaction);
            context
        })
        .expect("package baseline survives a variant hit");
    assert_eq!(repeated_theme.layout.computed_nodes[0].rect, first_root);
    assert_eq!(cache.rebuild_count, 3);
    assert_eq!(cache.hit_count, 4);
}

#[test]
fn aui_present_cache_snapshot_failure_does_not_swallow_accepted_retained_updates() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_game_sdk::aui::*;
    struct Producer {
        source: SequencedUiProducer,
        fail: bool,
    }
    impl ProjectUiStateSnapshotProducer for Producer {
        fn producer_id(&self) -> &str {
            "retained-failure-test"
        }
        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            self.source.produce(context)
        }
        fn resolve(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
            if self.fail {
                Err(ProjectUiStateResolveError::new(
                    "test.failure",
                    "binding snapshot unavailable",
                ))
            } else {
                self.source.resolve(context)
            }
        }
    }
    let root = temp_root("aui-retained-snapshot-failure");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let package = load_runtime_package(package_dir).value.unwrap();
    let mut instance = AuiInstance::mount(std::sync::Arc::new(
        package.aui_documents.get("hud").unwrap().clone(),
    ))
    .unwrap();
    let handle = instance.node("root").unwrap();
    let mut producer = Producer {
        source: SequencedUiProducer {
            values: vec!["粮草100".into()],
            call_count: 0,
        },
        fail: false,
    };
    let world = World::new();
    let mut interaction = AuiInteractionState::default();
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
    context.retained_instance = Some(&instance);
    let first = cache
        .take_or_rebuild(&package, &world, 1, &mut producer, context)
        .unwrap();
    cache.store(Some(first));
    producer.fail = true;
    let mut edit = instance.edit();
    edit.write(
        handle,
        UiProperty::Translation,
        UiValue::Translation([40., 25.]),
    )
    .unwrap();
    edit.commit().unwrap();
    let mut context = NativeAuiPresentContext::new(None, None, &mut interaction);
    context.retained_instance = Some(&instance);
    let updated = cache
        .take_or_rebuild(&package, &world, 2, &mut producer, context)
        .unwrap();
    assert_eq!(
        (
            updated.layout.computed_nodes[0].rect.x,
            updated.layout.computed_nodes[0].rect.y
        ),
        (40., 25.)
    );
    assert_eq!(
        updated.resolved_document.nodes[1].text.as_deref(),
        Some("粮草100")
    );
    assert_eq!(cache.rebuild_count, 2);
}

#[test]
fn aui_present_cache_reference_canvas_ignores_viewport_identity_changes() {
    let root = temp_root("aui-reference-present-cache");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let load = load_runtime_package(package_dir);
    assert!(!load.diagnostics.has_errors(), "{:#?}", load.diagnostics);
    let package = load.value.expect("minimal AUI runtime package");
    let world = World::new();
    let mut producer = SequencedUiProducer {
        values: vec!["FPS: 60".to_string()],
        call_count: 0,
    };
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();
    let first_viewport = native_window_viewport_input(1200, 800, 1.0);
    let first = cache
        .take_or_rebuild(
            &package,
            &world,
            1,
            &mut producer,
            NativeAuiPresentContext::new(None, Some(&first_viewport), &mut interaction),
        )
        .expect("first reference-canvas present");
    cache.store(Some(first));

    let changed_viewport = native_window_viewport_input(600, 1000, 2.0);
    let second = cache
        .take_or_rebuild(
            &package,
            &world,
            2,
            &mut producer,
            NativeAuiPresentContext::new(None, Some(&changed_viewport), &mut interaction),
        )
        .expect("reference-canvas cache hit");

    assert_eq!(second.report.viewport_status, "reference_canvas");
    assert_eq!(producer.call_count, 1);
    assert_eq!(cache.rebuild_count, 1);
    assert_eq!(cache.hit_count, 1);
}

#[test]
fn native_window_viewport_input_preserves_real_window_metrics() {
    let input = native_window_viewport_input(2560, 1440, 1.5);
    assert_eq!(input.physical_width, 2560);
    assert_eq!(input.physical_height, 1440);
    assert_eq!(input.scale_factor, 1.5);
    assert_eq!(input.physical_safe_insets, Default::default());
    assert!(input.compact_identity().is_ok());
}

#[test]
fn aui_present_cache_reuses_c2_paragraphs_and_reshapes_changed_text() {
    use engine_runtime::aui_layout_schema::{
        Dimension, LayoutMode, LayoutSpec, SlotSpec, TextLayoutSpec, TextWrap,
        AUI_LAYOUT_SCHEMA_VERSION,
    };
    let root = temp_root("aui-c2-present-cache");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    add_minimal_aui_document(&package_dir);
    let load = load_runtime_package(package_dir);
    assert!(!load.diagnostics.has_errors(), "{:#?}", load.diagnostics);
    let mut package = load.value.expect("minimal AUI runtime package");
    let document = package
        .aui_documents
        .documents_by_id
        .get_mut("hud")
        .unwrap();
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document.nodes[0].layout = Some(LayoutSpec {
        mode: LayoutMode::Column,
        ..Default::default()
    });
    document.nodes[1].slot = Some(SlotSpec {
        width: Dimension::Fixed { value: 260.0 },
        height: Dimension::Content,
        ..Default::default()
    });
    document.nodes[1].text_layout = Some(TextLayoutSpec {
        wrap: TextWrap::GraphemeFallback,
        ..Default::default()
    });
    package.font_bundles = source_font_registry();
    let world = World::new();
    let mut producer = SequencedUiProducer {
        values: vec!["第一帧文字".to_string(), "第三帧变化文字".to_string()],
        call_count: 0,
    };
    let mut cache = NativeAuiPresentCache::for_package(&package);
    let mut interaction = AuiInteractionState::default();

    let first = cache
        .take_or_rebuild(
            &package,
            &world,
            1,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("first C2 present");
    assert!(
        first.layout.diagnostics.is_empty(),
        "{:#?}",
        first.layout.diagnostics
    );
    let first_stats = cache.c2_runtime().paragraph_cache_stats();
    assert!(first_stats.reshapes > 0);
    cache.store(Some(first));

    let second = cache
        .take_or_rebuild(
            &package,
            &world,
            2,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("clean C2 present");
    assert_eq!(cache.c2_runtime().paragraph_cache_stats(), first_stats);
    cache.store(Some(second));

    let third = cache
        .take_or_rebuild(
            &package,
            &world,
            3,
            &mut producer,
            NativeAuiPresentContext::new(None, None, &mut interaction),
        )
        .expect("changed C2 present");
    assert!(third.layout.diagnostics.is_empty());
    assert!(cache.c2_runtime().paragraph_cache_stats().reshapes > first_stats.reshapes);
    assert_eq!(cache.rebuild_count, 2);
    assert_eq!(cache.hit_count, 1);
}

#[test]
fn runtime_sprite_texture_collection_includes_scene_and_all_animator_frames() {
    use engine_runtime::animator2d::{
        Animator2DPlayback, CookedAnimator2DRegistry, CookedSpriteAnimationClip2D,
        CookedSpriteAnimationFrame2D,
    };
    use engine_runtime::runtime_package::{RuntimeAssetRef, RuntimeSpriteRenderer2D};

    let root = temp_root("sprite-texture-collection");
    let package_dir = write_minimal_runtime_package(&root, "runtime-package");
    let load = load_runtime_package(package_dir);
    assert!(
        !load.diagnostics.has_errors(),
        "diagnostics={:#?}",
        load.diagnostics
    );
    let mut package = load.value.expect("minimal runtime package");
    package.active_scene.entities[0].sprite_renderer2d = Some(RuntimeSpriteRenderer2D {
        sprite_ref: Some(RuntimeAssetRef {
            id: "enemy-idle".to_string(),
            asset_type: "texture".to_string(),
            guid: None,
            sub_asset: None,
        }),
        material_ref: None,
        color: None,
        flip_x: None,
        flip_y: None,
        sorting_layer: None,
        order_in_layer: None,
        sort_z: None,
        visible: None,
    });
    package.animator2d_registry = CookedAnimator2DRegistry::from_parts(
        vec![CookedSpriteAnimationClip2D {
            id: "enemy-move".to_string(),
            playback: Animator2DPlayback::Loop,
            frames: vec![
                CookedSpriteAnimationFrame2D {
                    sprite_asset_id: "enemy-move-0".to_string(),
                    duration_ticks: 4,
                },
                CookedSpriteAnimationFrame2D {
                    sprite_asset_id: "enemy-move-1".to_string(),
                    duration_ticks: 4,
                },
            ],
        }],
        Vec::new(),
    )
    .expect("Animator2D fixture registry");

    let mut prefab_entity = package.active_scene.entities[0].clone();
    prefab_entity
        .sprite_renderer2d
        .as_mut()
        .unwrap()
        .sprite_ref
        .as_mut()
        .unwrap()
        .id = "prefab-only-texture".to_string();
    package
        .assets
        .assets
        .push(engine_runtime::runtime_package::RuntimeAsset {
            id: "prefab-only".to_string(),
            name: "Prefab".to_string(),
            asset_type: "prefab".to_string(),
            source: String::new(),
            state: "available".to_string(),
            bundle_id: "startup".to_string(),
            data: Some(
                serde_json::to_value(engine_runtime::runtime_package::RuntimePrefabData {
                    schema_version: "runtime-prefab.v1".to_string(),
                    id: "prefab-only".to_string(),
                    name: "Prefab".to_string(),
                    root_entity_id: Some(prefab_entity.id.clone()),
                    entities: vec![prefab_entity],
                })
                .unwrap(),
            ),
        });
    let asset_ids = runtime_sprite_texture_asset_ids(&package);

    assert!(
        asset_ids.contains("prefab-only-texture"),
        "runtime-spawned prefab textures must be resident before their first frame"
    );
    assert_eq!(asset_ids.len(), 4);
    assert!(asset_ids.contains("enemy-idle"));
    assert!(asset_ids.contains("enemy-move-0"));
    assert!(asset_ids.contains("enemy-move-1"));
}

#[test]
fn ordinary_spatial_sixty_fps_uses_real_elapsed_but_semantic_keeps_explicit_ticks() {
    assert!(
        uses_presentation_elapsed(60, true, false),
        "60FPS spatial simulation must not slow when presents drop"
    );
    assert!(uses_presentation_elapsed(120, true, false));
    assert!(!uses_presentation_elapsed(60, true, true));
    assert!(
        !uses_presentation_elapsed(60, false, false),
        "retain existing 2D60 policy"
    );
    assert!(uses_presentation_elapsed(120, false, false));
}
#[test]
fn frame_performance_reports_update_render_submit_and_present_wait() {
    let mut request = NativePlayerWindowRunRequest::headless_surface_gate("runtime-package");
    request.performance_warmup_frames = 1;
    request.performance_sample_frames = 3;
    let mut report = NativeWindowHostReport::base(&request);
    let work = [
        AuiFrameWorkCounters {
            present_hits: 1,
            drawing_nodes_built: 2,
            paragraph_cache_resident_bytes: 100,
            ..Default::default()
        },
        AuiFrameWorkCounters {
            present_hits: 2,
            layout_nodes_visited: 4,
            drawing_nodes_built: 3,
            paragraph_cache_resident_bytes: 120,
            virtual_list_materialized_instances: 7,
            virtual_list_recycled_instances: 2,
            virtual_list_disposed_instances: 2,
            theme_change_events: 1,
            dpi_change_events: 1,
            ..Default::default()
        },
        AuiFrameWorkCounters {
            present_hits: 3,
            layout_nodes_visited: 9,
            drawing_nodes_built: 3,
            paragraph_cache_resident_bytes: 110,
            virtual_list_materialized_instances: 9,
            virtual_list_recycled_instances: 4,
            virtual_list_disposed_instances: 4,
            theme_change_events: 1,
            dpi_change_events: 1,
            ..Default::default()
        },
        AuiFrameWorkCounters {
            present_hits: 4,
            layout_nodes_visited: 15,
            drawing_nodes_built: 4,
            paragraph_cache_resident_bytes: 90,
            virtual_list_materialized_instances: 12,
            virtual_list_recycled_instances: 7,
            virtual_list_disposed_instances: 7,
            theme_change_events: 1,
            dpi_change_events: 1,
            ..Default::default()
        },
    ];
    finalize_frame_performance(
        &mut report,
        &request,
        FramePerformanceSamples {
            cpu_ns: &[9_000_000, 10_000_000, 20_000_000, 30_000_000],
            aui_cpu_ns: &[500_000, 1_000_000, 3_000_000, 5_000_000],
            aui_work: &work,
            update_ns: &[1_000_000, 2_000_000, 4_000_000, 6_000_000],
            render_submit_ns: &[3_000_000, 4_000_000, 8_000_000, 12_000_000],
            present_wait_ns: &[0, 1_000_000, 2_000_000, 3_000_000],
        },
    );

    let performance = report.frame_performance_summary.unwrap();
    assert_eq!(performance.observed_sample_frames, 3);
    let excluding_wait = performance.cpu_excluding_present_wait.as_ref().unwrap();
    assert_eq!(excluding_wait.observed_sample_frames, 3);
    assert_eq!(excluding_wait.mean_ms, 18.0);
    assert_eq!(excluding_wait.p95_ms, 27.0);
    assert!(
        performance.sampling_window.is_none(),
        "headless summary must not invent wall time"
    );
    assert_eq!(performance.aui_cpu.observed_sample_frames, 3);
    assert_eq!(performance.update.observed_sample_frames, 3);
    assert_eq!(performance.render_submit.observed_sample_frames, 3);
    assert_eq!(performance.present_wait.observed_sample_frames, 3);
    assert!((performance.update.mean_ms - 4.0).abs() < f64::EPSILON);
    assert!((performance.aui_cpu.p50_ms - 3.0).abs() < f64::EPSILON);
    assert!((performance.aui_cpu.max_ms - 5.0).abs() < f64::EPSILON);
    assert!((performance.p50_cpu_frame_ms - 20.0).abs() < f64::EPSILON);
    assert!((performance.max_cpu_frame_ms - 30.0).abs() < f64::EPSILON);
    assert_eq!(performance.aui_work.observed_sample_frames, 3);
    assert_eq!(performance.aui_work.present_hits, 3);
    assert_eq!(performance.aui_work.layout_nodes_visited, 15);
    assert_eq!(performance.aui_work.drawing_nodes_built, 2);
    assert_eq!(performance.aui_work.paragraph_cache_high_water_bytes, 120);
    assert_eq!(performance.aui_work.virtual_list_materialized_instances, 12);
    assert_eq!(performance.aui_work.virtual_list_recycled_instances, 7);
    assert_eq!(performance.aui_work.virtual_list_disposed_instances, 7);
    assert_eq!(performance.aui_work.theme_change_events, 3);
    assert_eq!(performance.aui_work.dpi_change_events, 3);
    assert!((performance.render_submit.mean_ms - 8.0).abs() < f64::EPSILON);
    assert!((performance.present_wait.mean_ms - 2.0).abs() < f64::EPSILON);
}

#[test]
fn gate_d_raw_frames_preserve_intervals_and_difference_owner_counters() {
    use engine_runtime::aui::{AuiDrawingStats, AuiUpdateTimings, AuiUpdateWork};
    let mut request = NativePlayerWindowRunRequest::headless_surface_gate("runtime-package");
    request.performance_warmup_frames = 1;
    request.performance_sample_frames = 2;
    let work = [1_u64, 3, 8].map(|value| AuiFrameWorkCounters {
        spine: Some(
            engine_runtime::spine_animation::measurement::SpineCpuFrameSample {
                control_ns: value * 11,
                official_apply_world_ns: value * 13,
                vertices_clipping_ns: value * 17,
                projection_encode_ns: value * 19,
                evaluated_instances: value,
                ..Default::default()
            },
        ),
        spatial_gpu: Some(
            engine_runtime::windowed_player::WindowedPlayerSpatialGpuSample {
                cumulative_draws: value * 10,
                cumulative_instances: value * 10000,
                frame_uploaded_bytes: value * 1024,
                cumulative_parameter_uploaded_bytes: value * 64,
                requested_gpu_backing_bytes: Some(value * 4096),
            },
        ),
        sampling_wall_ns: Some(value * 1000),
        streaming_resources: Some(
            engine_runtime::windowed_player::WindowedPlayerStreamingResourceSample {
                active_cells: value as usize,
                cpu_resident_bytes: value * 100,
                gpu_texture_bytes: value * 1000,
                gpu_uploaded_bytes: value * 2000,
                ..Default::default()
            },
        ),
        input_dispatch_ime_ns: value * 7,
        frame_index: value,
        timings: AuiUpdateTimings {
            production_exchange_diff_ns: value * 10,
            ..Default::default()
        },
        work: AuiUpdateWork {
            layout_nodes_visited: value * 2,
            ..Default::default()
        },
        drawing: AuiDrawingStats {
            nodes_visited: value,
            ..Default::default()
        },
        feedback_cpu_ns: value * 5,
        ui_uploaded_bytes: value * 100,
        ui_partial_uploads: value,
        ..Default::default()
    });
    let mut report = NativeWindowHostReport::base(&request);
    let samples = || FramePerformanceSamples {
        cpu_ns: &[1_000, 2_000, 3_000],
        aui_cpu_ns: &[100, 200, 50],
        aui_work: &work,
        update_ns: &[10, 20, 30],
        render_submit_ns: &[40, 50, 60],
        present_wait_ns: &[70, 80, 90],
    };
    finalize_frame_performance(&mut report, &request, samples());
    let summary = report.frame_performance_summary.as_ref().unwrap();
    assert_eq!(summary.raw_cpu_frames.len(), 3);
    let first = &summary.raw_cpu_frames[0];
    assert!(first.warmup && !first.sampled);
    let second = &summary.raw_cpu_frames[1];
    assert!(!second.warmup && second.sampled);
    assert_eq!(second.frame_index, 3);
    assert_eq!(
        second.spine, work[1].spine,
        "Spine intervals are per-frame, never cumulative deltas"
    );
    let mut old = serde_json::to_value(second).unwrap();
    old.as_object_mut().unwrap().remove("spine");
    assert!(
        serde_json::from_value::<engine_runtime::windowed_player::WindowedPlayerCpuFrameSample>(
            old
        )
        .unwrap()
        .spine
        .is_none()
    );
    assert_eq!(second.sampling_wall_ns, Some(3000));
    assert_eq!(second.cpu_frame_ns, 2_000);
    assert_eq!(second.aui_cpu_ns, 200);
    assert_eq!(second.aui_stages.production_exchange_diff_ns, 20);
    assert_eq!(second.preparation_layout_geometry.layout_nodes_visited, 4);
    assert_eq!(second.drawing.nodes_visited, 2);
    assert_eq!(second.feedback_cpu_ns, 10);
    assert_eq!(second.input_dispatch_ime_ns, 21);
    assert_eq!(second.input_ime_schedule_other_ns, 149);
    assert_eq!(second.ui_uploaded_bytes, 200);
    assert_eq!(second.ui_partial_uploads, 2);
    assert!(second.aui_intervals_consistent);
    assert!(second.projection.is_none());
    let resources = second.streaming_resources.unwrap();
    assert_eq!(resources.active_cells, 3);
    assert_eq!(resources.cpu_resident_bytes, 300);
    assert_eq!(resources.gpu_texture_bytes, 3000);
    assert_eq!(resources.gpu_uploaded_bytes, 6000); // Cumulative, never differenced into residency.
    let spatial = second.spatial_gpu.unwrap();
    assert_eq!(spatial.cumulative_instances, 30000);
    assert_eq!(spatial.cumulative_draws, 30);
    assert_eq!(spatial.frame_uploaded_bytes, 3072);
    let mut legacy = serde_json::to_value(&second).unwrap();
    legacy.as_object_mut().unwrap().remove("spatialGpu");
    let legacy: engine_runtime::windowed_player::WindowedPlayerCpuFrameSample =
        serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.spatial_gpu, None);
    let mut legacy = serde_json::to_value(second).unwrap();
    legacy.as_object_mut().unwrap().remove("streamingResources");
    assert!(
        serde_json::from_value::<engine_runtime::windowed_player::WindowedPlayerCpuFrameSample>(
            legacy
        )
        .unwrap()
        .streaming_resources
        .is_none()
    );
    let third = &summary.raw_cpu_frames[2];
    assert!(
        !third.aui_intervals_consistent,
        "overlap must not silently shrink total AUI"
    );
    assert_eq!(third.aui_cpu_ns, 50);
    assert_eq!(third.aui_stages.production_exchange_diff_ns, 50);
    assert_eq!(third.feedback_cpu_ns, 25);
    request.performance_sample_frames = 0;
    let mut off = NativeWindowHostReport::base(&request);
    finalize_frame_performance(&mut off, &request, samples());
    assert!(off.frame_performance_summary.is_none());
}

#[test]
fn screenshot_request_is_reported_without_running_real_window() {
    let request = NativePlayerWindowRunRequest::windowed("runtime-package")
        .with_screenshot("reports/screenshot.png");
    let report = NativeWindowHostReport::base(&request);

    assert!(report.screenshot.requested);
    assert_eq!(
        report.screenshot.status,
        NativeWindowScreenshotStatus::Unsupported
    );
    assert_eq!(
        report.screenshot.path.as_deref(),
        Some("reports/screenshot.png")
    );
}

struct UnavailableAudioOutput {
    prepare_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl engine_runtime::runtime_audio::AudioOutput for UnavailableAudioOutput {
    fn kind(&self) -> &'static str {
        "test-unavailable-output"
    }

    fn prepare(&mut self) -> Result<(), String> {
        self.prepare_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err("test audio device is unavailable".into())
    }

    fn play(
        &mut self,
        _: engine_runtime::ids::RuntimeEntityId,
        _: std::sync::Arc<engine_runtime::audio::DecodedAudioClip>,
        _: f32,
        _: bool,
    ) -> Result<(), String> {
        panic!("failed device preparation must prevent playback")
    }

    fn stop(&mut self, _: engine_runtime::ids::RuntimeEntityId) {}

    fn set_paused(&mut self, _: engine_runtime::ids::RuntimeEntityId, _: bool) {}

    fn finished(&self, _: engine_runtime::ids::RuntimeEntityId) -> bool {
        false
    }
}

#[test]
fn audio_device_failure_reaches_completion_diagnostics_even_when_reports_are_off() {
    use engine_runtime::archetype::ComponentValue;
    use engine_runtime::components::{ComponentTypeId, Hierarchy};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let mut world = World::new();
    let source_id = engine_runtime::ids::EntityId::from("speaker");
    world
        .try_spawn_entity(
            source_id.clone(),
            "Speaker",
            "audio",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    world
        .try_insert_component_value(
            source_id,
            ComponentTypeId::audio_source(),
            ComponentValue::AudioSource(engine_runtime::audio::AudioSource {
                clip_ref: engine_runtime::runtime_package::RuntimeAssetRef {
                    id: "audio-test".into(),
                    asset_type: "audio".into(),
                    guid: None,
                    sub_asset: None,
                },
                volume: 0.5,
            }),
        )
        .unwrap();
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let mut host = EngineHostLoop::new("audio-failure-report");
    host.set_audio_output(
        Box::new(UnavailableAudioOutput {
            prepare_calls: prepare_calls.clone(),
        }),
        false,
    );
    host.tick(
        EngineFrameInput::new(EngineHostMode::HeadlessServer).with_fixed_step_count(0),
        &mut world,
    );
    assert_eq!(prepare_calls.load(Ordering::SeqCst), 1);

    for level in [
        WindowedPlayerRuntimeReportLevel::Off,
        WindowedPlayerRuntimeReportLevel::Summary,
    ] {
        let request = NativePlayerWindowRunRequest::headless_surface_gate("package")
            .with_runtime_report_level(level);
        let mut report = NativeWindowHostReport::base(&request);
        report.present_status = NativeWindowPresentStatus::Presented;
        report.mark_runtime_advanced();
        assert!(!report.has_errors());
        record_runtime_audio(&mut report, &host, level);
        record_runtime_audio(&mut report, &host, level);
        assert!(report.has_errors());
        assert_eq!(report.completion_exit_code(), 1);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "audio.output_unavailable");
        assert_eq!(report.diagnostics[0].layer, "runtime_audio");
        assert_eq!(
            report.diagnostics[0].message,
            "test audio device is unavailable"
        );
        assert_eq!(
            report.audio.is_some(),
            level != WindowedPlayerRuntimeReportLevel::Off
        );
        report.mark_runtime_advanced();
        assert_eq!(report.completion_exit_code(), 1);
    }
}

#[test]
fn audio_absent_project_does_not_prepare_a_device_or_emit_an_audio_report() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let mut host = EngineHostLoop::new("no-audio-report");
    host.set_audio_output(
        Box::new(UnavailableAudioOutput {
            prepare_calls: prepare_calls.clone(),
        }),
        false,
    );
    host.tick(
        EngineFrameInput::new(EngineHostMode::HeadlessServer).with_fixed_step_count(0),
        &mut World::new(),
    );
    assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
    let request = NativePlayerWindowRunRequest::headless_surface_gate("package")
        .with_runtime_report_level(WindowedPlayerRuntimeReportLevel::Summary);
    let mut report = NativeWindowHostReport::base(&request);
    record_runtime_audio(
        &mut report,
        &host,
        WindowedPlayerRuntimeReportLevel::Summary,
    );
    report.present_status = NativeWindowPresentStatus::Presented;
    assert!(!report.has_errors());
    assert!(report.diagnostics.is_empty());
    assert!(report.audio.is_none());
    assert!(serde_json::to_value(&report)
        .unwrap()
        .get("audio")
        .is_none());
    assert_eq!(report.completion_exit_code(), 0);
}

#[test]
fn summary_reports_rule_failure_and_off_still_marks_logic_error() {
    use engine_runtime::logic_executor::{ExecutorKind, LogicResult};
    for level in [
        WindowedPlayerRuntimeReportLevel::Off,
        WindowedPlayerRuntimeReportLevel::Summary,
    ] {
        let request = NativePlayerWindowRunRequest::headless_surface_gate("runtime-package")
            .with_runtime_report_level(level);
        let mut report = NativeWindowHostReport::base(&request);
        let mut trace = engine_runtime::runtime_trace::RuntimeTrace::new();
        let mut failure = LogicResult::failed(
            "rule.fade",
            ExecutorKind::RustAot,
            "world.component.unsupported_field",
            "color is not writable",
        );
        failure.failure_location = Some(engine_runtime::logic_executor::LogicFailureLocation {
            entity_id: "fx".into(),
            component_type: "engine.sprite_renderer2d".into(),
            field_path: Some("color".into()),
        });
        trace.record_logic_result(9, "Update", &failure);
        record_runtime_trace(&mut report, &trace, level);
        assert_eq!(report.logic_status, "error");
        if level == WindowedPlayerRuntimeReportLevel::Summary {
            assert_eq!(
                report
                    .gameplay_trace_summary
                    .as_ref()
                    .unwrap()
                    .failed_record_count,
                1
            );
            let summary = report.gameplay_trace_summary.as_ref().unwrap();
            assert_eq!(
                summary.write_count, 0,
                "failed writes are not successful writes"
            );
            assert_eq!(summary.failure_details[0].entity_id.as_deref(), Some("fx"));
            assert_eq!(
                summary.failure_details[0].field_path.as_deref(),
                Some("color")
            );
        } else {
            assert!(report.gameplay_trace_summary.is_none());
        }
        assert!(report.gameplay_trace_records.is_empty());
        report.mark_runtime_advanced();
        record_runtime_trace(
            &mut report,
            &engine_runtime::runtime_trace::RuntimeTrace::new(),
            level,
        );
        assert_eq!(
            report.logic_status, "error",
            "later successful frames cannot clear failures"
        );
        report.present_status = NativeWindowPresentStatus::Presented;
        assert_eq!(
            report.completion_exit_code(),
            1,
            "presenting does not mean gameplay succeeded"
        );
    }
}

#[test]
fn summary_keeps_bounded_failure_details_and_reads_old_reports() {
    use engine_runtime::logic_executor::{ExecutorKind, LogicResult};
    let request = NativePlayerWindowRunRequest::headless_surface_gate("package")
        .with_runtime_report_level(WindowedPlayerRuntimeReportLevel::Summary);
    let mut report = NativeWindowHostReport::base(&request);
    let mut trace = engine_runtime::runtime_trace::RuntimeTrace::new();
    for frame in 1..=20 {
        trace.record_logic_result(
            frame,
            "Update",
            &LogicResult::failed(
                "rule.fade",
                ExecutorKind::RustAot,
                "world.component.unsupported_field",
                "错误".repeat(2000),
            ),
        );
    }
    record_runtime_trace(
        &mut report,
        &trace,
        WindowedPlayerRuntimeReportLevel::Summary,
    );
    let summary = report.gameplay_trace_summary.unwrap();
    assert_eq!(summary.failed_record_count, 20);
    assert_eq!(summary.failure_details.len(), 16);
    assert_eq!(summary.omitted_failure_count, 4);
    assert_eq!(summary.failure_details[0].frame_index, 1);
    assert_eq!(summary.failure_details[15].frame_index, 16);
    assert!(summary
        .failure_details
        .iter()
        .all(|d| d.message.as_ref().unwrap().len() <= 1024));
    let mut json = serde_json::to_value(&summary).unwrap();
    json.as_object_mut().unwrap().remove("failureDetails");
    json.as_object_mut().unwrap().remove("omittedFailureCount");
    let old: WindowedPlayerGameplayTraceSummary = serde_json::from_value(json).unwrap();
    assert!(old.failure_details.is_empty());
    assert_eq!(old.omitted_failure_count, 0);
    assert!(report.gameplay_trace_records.is_empty());
}

#[test]
fn summary_command_failure_preserves_entity_and_error_without_full_trace() {
    let request = NativePlayerWindowRunRequest::headless_surface_gate("package")
        .with_runtime_report_level(WindowedPlayerRuntimeReportLevel::Summary);
    let mut report = NativeWindowHostReport::base(&request);
    let mut trace = engine_runtime::runtime_trace::RuntimeTrace::new();
    let mut record = engine_runtime::gameplay_trace::GameplayTraceRecord::write(
        1260,
        "PostPhysics",
        "engine.command_buffer",
        "enemy".into(),
        "unused".into(),
        "",
        None,
        None,
    );
    record.operation = "command_apply".into();
    record.component_type = None;
    record.field_path = None;
    record.result = "failed".into();
    record.error_code = Some("world.entity.missing".into());
    trace.gameplay_records.push(record);
    record_runtime_trace(
        &mut report,
        &trace,
        WindowedPlayerRuntimeReportLevel::Summary,
    );
    let summary = report.gameplay_trace_summary.as_ref().unwrap();
    assert_eq!(summary.failed_record_count, 1);
    assert_eq!(
        summary.failure_details[0].entity_id.as_deref(),
        Some("enemy")
    );
    assert_eq!(
        summary.failure_details[0].error_code.as_deref(),
        Some("world.entity.missing")
    );
    assert_eq!(summary.failure_details[0].frame_index, 1260);
    assert!(report.gameplay_trace_records.is_empty());
}

#[test]
fn windowed_session_advances_past_first_frame_until_limit() {
    assert!(windowed_session_has_more_frames(1, 3));
    assert!(windowed_session_has_more_frames(2, 3));
    assert!(!windowed_session_has_more_frames(3, 3));
    assert!(windowed_session_has_more_frames(1, u64::MAX));
}

#[test]
fn player_target_updates_initial_window_extent() {
    let request = NativePlayerWindowRunRequest::windowed("runtime-package")
        .with_game_view_target(GameViewTargetSpec::portrait_720x1280());

    assert_eq!(request.config.width, 720);
    assert_eq!(request.config.height, 1280);
    assert_eq!(
        request.game_view_target,
        GameViewTargetSpec::portrait_720x1280()
    );
}

#[test]
fn ime_candidate_rect_uses_shared_reference_to_window_mapping() {
    use engine_runtime::game_view_presentation::{
        CanvasReferenceFact, GameViewExtent, GameViewPresentationModule, GameViewPresentationSpec,
        GameViewRect, GameViewScalePolicy,
    };
    let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "ime-candidate".into(),
        target_id: "main".into(),
        target_extent: GameViewExtent::new(400, 200),
        display_rect: GameViewRect::new(20.0, 10.0, 800.0, 400.0),
        scale_policy: GameViewScalePolicy::Stretch,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 200, 100)],
    })
    .unwrap();
    let request = native_ime_platform_request(
        &AuiImePlatformRequest {
            enabled: true,
            node_id: Some("input".into()),
            canvas_id: Some("main".into()),
            cursor_rect: Some(AuiComputedRect {
                x: 50.0,
                y: 25.0,
                width: 1.0,
                height: 20.0,
            }),
        },
        &presentation,
    );
    assert!(request.enabled);
    assert_eq!(request.node_id.as_deref(), Some("input"));
    assert_eq!(
        request.cursor_area,
        Some(NativeImeCursorArea {
            x: 220.0,
            y: 110.0,
            width: 4.0,
            height: 80.0,
        })
    );
    assert!(!native_ime_platform_request(&AuiImePlatformRequest::default(), &presentation).enabled);
}

#[test]
fn portrait_player_window_fits_inside_work_area_without_changing_aspect() {
    let fitted = fit_player_window_inner_extent(720, 1280, 2048, 1104, 16, 39);

    assert_eq!(fitted, (599, 1065));
    assert!((u64::from(fitted.0) * 1280).abs_diff(u64::from(fitted.1) * 720) <= 720);
}

#[test]
fn player_window_keeps_requested_extent_when_work_area_can_contain_it() {
    assert_eq!(
        fit_player_window_inner_extent(1280, 720, 2048, 1104, 16, 39),
        (1280, 720)
    );
}

#[test]
fn rgba_png_writer_outputs_png_file() {
    let root = temp_root("png-writer");
    let path = root.join("reports").join("screenshot.png");
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];

    let size = write_rgba_png(&path, 2, 2, &rgba).unwrap();

    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len() as u64, size);
    assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

#[test]
fn headless_native_player_runs_package_one_frame() {
    let root = temp_root("headless-one-frame");
    let package = write_minimal_runtime_package(&root, "runtime-package");
    let mut request = NativePlayerWindowRunRequest::headless_surface_gate(package);
    request.frame_limit = 1;

    let report = run_headless_native_player_from_package(request);

    assert_eq!(report.exit_code, 0, "diagnostics={:#?}", report.diagnostics);
    assert_eq!(report.package_status, "ok");
    assert_eq!(report.scene_status, "ok");
    assert_eq!(report.world_status, "ok");
    assert_eq!(report.logic_status, "ok");
    assert_eq!(report.render_status, "ok");
    assert_eq!(report.rhi_status, "ok");
    assert_eq!(report.present_status, NativeWindowPresentStatus::Presented);
    assert_eq!(report.frames_completed, 1);
    assert!(report.rhi_command_count > 0);
}

#[test]
fn runtime_player_project_runtime_session_headless_uses_shared_composition_and_reports_summary() {
    let root = temp_root("project-runtime-session");
    let package = write_minimal_runtime_package(&root, "runtime-package");
    let mut request = NativePlayerWindowRunRequest::headless_surface_gate(package);
    request.frame_limit = 1;

    let report = run_headless_native_player_from_package(request);

    let receipt = report
        .project_runtime_bind_receipt
        .as_ref()
        .expect("project runtime bind receipt");
    let session = report
        .project_runtime_session_report
        .as_ref()
        .expect("project runtime session summary");
    assert_eq!(receipt.session_id, session.session_id);
    assert_eq!(receipt.session_status, "ready");
    assert_eq!(session.status, "advanced");
    assert_eq!(session.stages.len(), 1);
    assert_eq!(
        session.stages[0].stage,
        engine_runtime::project_runtime_session::ProjectRuntimeSessionStage::FixedUpdate
    );
    assert_eq!(session.stages[0].action_count, 0);
    assert_eq!(session.stages[0].staged_mutation_count, 0);
    assert_eq!(session.stages[0].committed_mutation_count, 0);
    assert_eq!(receipt.producer_id, "engine_empty_project_ui_state");
    assert_ne!(receipt.producer_id, receipt.session_id);
}

#[test]
fn headless_native_player_reports_missing_package() {
    let root = temp_root("missing-package");
    let report = run_headless_native_player_from_package(
        NativePlayerWindowRunRequest::headless_surface_gate(root.join("missing-package")),
    );

    assert_eq!(report.exit_code, 1);
    assert_eq!(report.package_status, "error");
    assert_eq!(
        report.present_status,
        NativeWindowPresentStatus::PackageFailed
    );
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.layer == "package"));
}

#[test]
#[cfg(not(feature = "real-window"))]
fn feature_disabled_windowed_reports_native_host_required() {
    let root = temp_root("feature-disabled");
    let package = write_minimal_runtime_package(&root, "runtime-package");

    let report =
        run_windowed_native_player_from_package(NativePlayerWindowRunRequest::windowed(package));

    assert_eq!(report.exit_code, 1);
    assert_eq!(
        report.present_status,
        NativeWindowPresentStatus::FeatureNotEnabled
    );
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "native_window_feature_not_enabled"));
}

#[test]
#[cfg(not(feature = "real-window"))]
fn feature_disabled_windowed_screenshot_reports_unsupported() {
    let root = temp_root("feature-disabled-screenshot");
    let package = write_minimal_runtime_package(&root, "runtime-package");

    let report = run_windowed_native_player_from_package(
        NativePlayerWindowRunRequest::windowed(package)
            .with_screenshot(root.join("reports").join("window.png")),
    );

    assert_eq!(report.exit_code, 1);
    assert!(report.screenshot.requested);
    assert_eq!(
        report.screenshot.status,
        NativeWindowScreenshotStatus::Unsupported
    );
    assert!(report.screenshot.path.is_some());
}

#[test]
fn native_input_space_key_generates_fire_action() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::gameplay_default();
    let raw_events = vec![RawInputEvent::keyboard_down(1, "main-window", "Space")];

    let (snapshot, trace, summary) = resolve_native_input_frame(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
    );

    assert!(snapshot.button_pressed("action.fire"));
    assert_eq!(trace.action_count, snapshot.action_count());
    assert_eq!(summary.raw_event_count, 1);
    assert!(summary.last_action_ids.contains(&"action.fire".to_string()));
}

#[test]
fn native_input_held_key_drives_axis_across_frames() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::gameplay_default();
    let raw_events = vec![RawInputEvent::keyboard_down(1, "main-window", "D")];

    let (first_snapshot, _, _) = resolve_native_input_frame(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
    );
    let (second_snapshot, _, summary) = resolve_native_input_frame(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &[],
        2,
        "main-window",
        "headless-script",
        "test",
    );

    assert_eq!(
        first_snapshot.axis2("action.move"),
        Some(engine_input::Axis2 { x: 1.0, y: 0.0 })
    );
    assert_eq!(
        second_snapshot.axis2("action.move"),
        Some(engine_input::Axis2 { x: 1.0, y: 0.0 })
    );
    assert_eq!(summary.pressed_key_count, 1);
}

#[test]
fn native_input_mouse_pointer_button_and_wheel_are_resolved() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::new(
        "input.mouse-test",
        vec![
            engine_input::InputActionDefinition::new(
                "action.fire",
                engine_input::InputActionValueType::Button,
            ),
            engine_input::InputActionDefinition::new(
                "action.pointer",
                engine_input::InputActionValueType::Pointer,
            ),
            engine_input::InputActionDefinition::new(
                "action.scroll",
                engine_input::InputActionValueType::Axis1,
            ),
        ],
        vec![engine_input::InputContextDefinition::new("gameplay", 0)],
        vec![
            engine_input::InputBindingDefinition::new("gameplay", "action.fire", "mouse/Left"),
            engine_input::InputBindingDefinition::pointer("action.pointer"),
            engine_input::InputBindingDefinition::mouse_wheel("action.scroll"),
        ],
    );
    let raw_events = vec![
        RawInputEvent::mouse_move(1, "main-window", 32.0, 64.0),
        RawInputEvent::mouse_button_down(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
        RawInputEvent::mouse_wheel(1, "main-window", -1.0),
    ];

    let (snapshot, _, summary) = resolve_native_input_frame(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
    );

    assert_eq!(
        snapshot.pointer("action.pointer"),
        Some(engine_input::PointerPosition { x: 32.0, y: 64.0 })
    );
    assert!(snapshot.button_pressed("action.fire"));
    assert_eq!(
        snapshot.axis1("action.scroll"),
        Some(engine_input::Axis1 { value: -1.0 })
    );
    assert_eq!(
        summary.pointer_position,
        Some(engine_input::PointerPosition { x: 32.0, y: 64.0 })
    );
    assert_eq!(summary.pressed_mouse_button_count, 1);
}

#[test]
fn pointer_replay_reaches_real_input_resolver_and_old_keyboard_scripts_still_parse() {
    let script: NativePlayerInputScript = serde_json::from_str(r#"{"schemaVersion":"native-player-input-script.v1","scriptId":"pointer","frames":[{"frameIndex":1,"pointerPosition":[1000,600]}]}"#).unwrap();
    script.validate().unwrap();
    let request =
        NativePlayerWindowRunRequest::headless_surface_gate("unused").with_input_script(script);
    let mapping = InputMappingAsset::new(
        "pointer",
        vec![engine_input::InputActionDefinition::new(
            "action.pointer",
            engine_input::InputActionValueType::Pointer,
        )],
        vec![engine_input::InputContextDefinition::new("gameplay", 0)],
        vec![engine_input::InputBindingDefinition::pointer(
            "action.pointer",
        )],
    );
    let mut state = InputDeviceState::new();
    let (snapshot, _, summary) = resolve_native_input_frame(
        &mut state,
        &mapping,
        "test",
        None,
        &scripted_input_events(&request, 1),
        1,
        "window",
        "script",
        "test",
    );
    assert_eq!(
        snapshot.pointer("action.pointer"),
        Some(engine_input::PointerPosition {
            x: 1000.0,
            y: 600.0
        })
    );
    assert_eq!(summary.pointer_position, snapshot.pointer("action.pointer"));
    assert!(scripted_input_events(&request, 2).is_empty());
    let legacy: NativePlayerInputScript = serde_json::from_str(r#"{"schemaVersion":"native-player-input-script.v1","scriptId":"keyboard","frames":[{"frameIndex":1,"keyDown":["D"]}]}"#).unwrap();
    assert_eq!(legacy.frames[0].pointer_position, None);
    assert!(!serde_json::to_string(&legacy)
        .unwrap()
        .contains("pointerPosition"));
}

#[test]
fn ordered_input_aui_filters_mouse_fire_and_dispatches_click_action() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::new(
        "input.mouse-fire",
        vec![engine_input::InputActionDefinition::new(
            "action.fire",
            engine_input::InputActionValueType::Button,
        )],
        vec![engine_input::InputContextDefinition::new("gameplay", 0)],
        vec![engine_input::InputBindingDefinition::new(
            "gameplay",
            "action.fire",
            "mouse/Left",
        )],
    );
    let aui_present = test_aui_present_output();
    let mut aui_state = AuiInteractionState::default();
    let script: NativePlayerInputScript = serde_json::from_value(serde_json::json!({
        "schemaVersion":"native-player-input-script.v2","scriptId":"aui-click",
        "frames":[{"frameIndex":1,"events":[
            {"type":"pointerMove","position":[120,120]},
            {"type":"mouseDown","button":"Left"},
            {"type":"mouseUp","button":"Left"}
        ]}]
    }))
    .unwrap();
    script.validate().unwrap();
    let request =
        NativePlayerWindowRunRequest::headless_surface_gate("unused").with_input_script(script);
    let raw_events = scripted_input_events(&request, 1);
    let (snapshot, trace, summary, interaction) = resolve_native_input_frame_with_aui(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
        Some(&aui_present),
        None,
        &mut aui_state,
    );

    let interaction = interaction.expect("aui interaction should run");
    assert!(!snapshot.button_pressed("action.fire"));
    assert_eq!(summary.raw_event_count, 3);
    assert_eq!(summary.runtime_event_count, 0);
    assert_eq!(
        trace.route_kind.as_deref(),
        Some("RuntimeInputFrameFilteredByAui")
    );
    assert_eq!(interaction.report.consumed_pointer_event_count, 3);
    assert!(interaction.report.input_event_count > interaction.report.filtered_input_event_count);
    assert!(interaction
        .result
        .actions
        .iter()
        .any(|action| action.action_id == "ui.pause"));
}

#[test]
fn ordered_input_aui_dispatches_m4e_toggle_and_slider_values() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::new(
        "input.mouse-fire",
        vec![engine_input::InputActionDefinition::new(
            "action.fire",
            engine_input::InputActionValueType::Button,
        )],
        vec![engine_input::InputContextDefinition::new("gameplay", 0)],
        vec![engine_input::InputBindingDefinition::new(
            "gameplay",
            "action.fire",
            "mouse/Left",
        )],
    );
    let aui_present = test_aui_controls_present_output();
    let mut aui_state = AuiInteractionState::default();
    let raw_events = vec![
        RawInputEvent::mouse_move(1, "main-window", 20.0, 20.0),
        RawInputEvent::mouse_button_down(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
        RawInputEvent::mouse_button_up(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
        RawInputEvent::mouse_move(1, "main-window", 75.0, 80.0),
        RawInputEvent::mouse_button_down(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
        RawInputEvent::mouse_button_up(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
        RawInputEvent::keyboard_down(1, "main-window", "ArrowRight"),
    ];
    let (snapshot, _, summary, interaction) = resolve_native_input_frame_with_aui(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
        Some(&aui_present),
        None,
        &mut aui_state,
    );

    let interaction = interaction.expect("AUI interaction should run");
    assert!(!snapshot.button_pressed("action.fire"));
    assert!(summary.runtime_event_count < raw_events.len());
    assert!(interaction.report.consumed_keyboard_event_count > 0);
    assert_eq!(aui_state.toggle_value("toggle"), Some(true));
    assert_eq!(aui_state.slider_value("slider"), Some(0.9));
    assert!(interaction.result.actions.iter().any(|action| {
        action.node_id == "toggle"
            && action.event == engine_runtime::aui::AuiActionEvent::ValueChanged
    }));
    assert!(interaction.result.actions.iter().any(|action| {
        action.node_id == "slider"
            && action.event == engine_runtime::aui::AuiActionEvent::ValueChanged
    }));
}

#[test]
fn portrait_target_space_input_uses_shared_presentation_inverse() {
    use engine_runtime::game_view_presentation::{
        CanvasReferenceFact, GameViewExtent, GameViewPoint, GameViewPresentationModule,
        GameViewPresentationSpec, GameViewRect, GameViewScalePolicy,
    };

    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::gameplay_default();
    let aui_present = test_aui_present_output();
    let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "player-portrait-test".to_string(),
        target_id: "main-surface".to_string(),
        target_extent: GameViewExtent::new(720, 1280),
        display_rect: GameViewRect::new(0.0, 0.0, 720.0, 1280.0),
        scale_policy: GameViewScalePolicy::Contain,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 1280, 720)],
    })
    .expect("portrait presentation");
    let target = presentation
        .reference_to_target("main", GameViewPoint::new(170.0, 120.0))
        .expect("button center maps into target space");
    let raw_events = vec![
        RawInputEvent::mouse_move(1, "main-window", target.x, target.y),
        RawInputEvent::mouse_button_down(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
        RawInputEvent::mouse_button_up(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
    ];
    let mut aui_state = AuiInteractionState::default();

    let (_, _, _, interaction) = resolve_native_input_frame_with_aui(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
        Some(&aui_present),
        Some(&presentation),
        &mut aui_state,
    );

    let interaction = interaction.expect("AUI interaction should run");
    assert!(interaction
        .result
        .actions
        .iter()
        .any(|action| action.action_id == "ui.pause"));
}

#[test]
fn aui_feedback_player_pointer_down_updates_same_frame_overlay() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::gameplay_default();
    let mut present = test_aui_present_output();
    let original_button_rect = present
        .overlay
        .draw_items
        .iter()
        .find(|item| item.node_id == "pause_button")
        .map(|item| {
            item.presentation
                .map_or(item.rect, |geometry| geometry.bounds)
        })
        .unwrap();
    let mut interaction_state = AuiInteractionState::default();
    let raw_events = vec![
        RawInputEvent::mouse_move(1, "main-window", 120.0, 120.0),
        RawInputEvent::mouse_button_down(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
    ];
    let (_, _, _, interaction) = resolve_native_input_frame_with_aui(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
        Some(&present),
        None,
        &mut interaction_state,
    );
    let mut feedback_state = AuiControlFeedbackState::default();
    let feedback = AuiRuntimePresenter::apply_control_feedback_with_fonts(
        &mut present,
        &interaction.unwrap().result,
        &mut feedback_state,
        presentation_delta_us_from_seconds(engine_runtime::runtime_time::DEFAULT_FIXED_DELTA_TIME),
        &RuntimeAuiFontAtlasRegistry::empty("test"),
        &RuntimeFontBundleRegistry::default(),
    );
    let animated_button_rect = present
        .overlay
        .draw_items
        .iter()
        .find(|item| item.node_id == "pause_button")
        .map(|item| {
            item.presentation
                .map_or(item.rect, |geometry| geometry.bounds)
        })
        .unwrap();
    assert_eq!(feedback.overrides.len(), 1);
    assert!(feedback.overrides.get("pause_button").unwrap().scale < 1.0);
    assert!(animated_button_rect.width < original_button_rect.width);
    assert!(feedback
        .report
        .resolved_profile_ids
        .contains(engine_runtime::aui::AUI_BUILTIN_BUTTON_FEEDBACK_PROFILE_ID));
}

#[test]
fn aui_text_ime_events_are_consumed_and_reported_by_native_input_chain() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::gameplay_default();
    let aui_present = test_aui_text_entry_present_output();
    let mut aui_state = AuiInteractionState::default();
    let raw_events = vec![
        RawInputEvent::mouse_move(1, "main-window", 96.0, 96.0),
        RawInputEvent::mouse_button_down(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
        RawInputEvent::text_input(1, "main-window", "B"),
        RawInputEvent::ime_preedit(1, "main-window", "ni", 0, 2),
        RawInputEvent::ime_cancel(1, "main-window"),
        RawInputEvent::ime_commit(1, "main-window", "hao"),
        RawInputEvent::keyboard_down(1, "main-window", "Enter"),
        RawInputEvent::keyboard_up(1, "main-window", "Enter"),
    ];

    let (_snapshot, _trace, summary, interaction) = resolve_native_input_frame_with_aui(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
        Some(&aui_present),
        None,
        &mut aui_state,
    );

    let interaction = interaction.expect("aui interaction should run");
    let mut present_summary = NativeAuiPresentSummary::empty();
    present_summary.apply_interaction_report(&interaction.report);
    present_summary.apply_navigation_screenflow_textentry_report(
        &interaction.navigation_screenflow_textentry_report,
    );

    assert!(summary.runtime_event_count < raw_events.len());
    assert_eq!(
        interaction
            .navigation_screenflow_textentry_report
            .ime_platform_coverage,
        engine_runtime::aui::AUI_IME_PLATFORM_COVERAGE
    );
    assert_eq!(present_summary.aui_text_edit_session_count, 1);
    assert_eq!(present_summary.aui_text_changed_count, 2);
    assert_eq!(present_summary.aui_text_submitted_count, 1);
    assert_eq!(present_summary.aui_ime_preedit_count, 1);
    assert_eq!(present_summary.aui_ime_cancel_count, 1);
    assert_eq!(present_summary.aui_ime_commit_count, 1);
    assert!(present_summary.aui_action_prompt_reported);
    assert!(present_summary.gameplay_input_filtered);
}

#[test]
fn native_input_focus_lost_releases_pressed_inputs() {
    let mut device_state = InputDeviceState::new();
    let mapping = InputMappingAsset::gameplay_default();
    let raw_events = vec![
        RawInputEvent::keyboard_down(1, "main-window", "Space"),
        RawInputEvent::mouse_button_down(
            1,
            "main-window",
            engine_input::RuntimePointerButton::Primary,
        ),
    ];
    let _ = resolve_native_input_frame(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &raw_events,
        1,
        "main-window",
        "headless-script",
        "test",
    );

    let (_snapshot, _trace, summary) = resolve_native_input_frame(
        &mut device_state,
        &mapping,
        "test-fixture",
        None,
        &[RawInputEvent::focus_lost(2, "main-window")],
        2,
        "main-window",
        "headless-script",
        "test",
    );

    assert!(!summary.focused);
    assert_eq!(summary.pressed_key_count, 0);
    assert_eq!(summary.pressed_mouse_button_count, 0);
}

#[test]
fn headless_player_reports_runtime_package_input_mapping() {
    let root = temp_root("runtime-package-input");
    let package = write_minimal_runtime_package(&root, "runtime-package");
    let custom_mapping = InputMappingAsset::new(
        "input.project",
        vec![engine_input::InputActionDefinition::new(
            "action.launch",
            engine_input::InputActionValueType::Button,
        )],
        vec![engine_input::InputContextDefinition::new("gameplay", 0)],
        vec![engine_input::InputBindingDefinition::button(
            "action.launch",
            "KeyL",
        )],
    );
    fs::write(
        package.join("input").join("input-manifest.json"),
        r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.project",
  "mappings": [{ "id": "input.project", "path": "input/input.project.json", "enabled": true }]
}"#,
    )
    .unwrap();
    fs::write(
        package.join("input").join("input.project.json"),
        serde_json::to_string_pretty(&custom_mapping).unwrap(),
    )
    .unwrap();
    let manifest_path = package.join("manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path).unwrap().replace(
            r#""input": { "path": "input/input-manifest.json", "defaultMappingId": "input.default", "mappingCount": 1 }"#,
            r#""input": { "path": "input/input-manifest.json", "defaultMappingId": "input.project", "mappingCount": 1 }"#,
        );
    fs::write(manifest_path, manifest_text).unwrap();

    let report = run_headless_native_player_from_package(
        NativePlayerWindowRunRequest::headless_surface_gate(package),
    );

    assert_eq!(report.package_status, "ok");
    assert_eq!(report.input.mapping_source, "runtime-package");
    assert_eq!(report.input.mapping_id.as_deref(), Some("input.project"));
    assert_eq!(report.input.mapping_status, "ok");
}

#[cfg(feature = "real-window")]
#[test]
#[ignore = "real OS window / GPU smoke gate is local-only"]
fn real_windowed_player_smoke() {
    let root = temp_root("real-windowed-smoke");
    let package = write_minimal_runtime_package(&root, "runtime-package");
    let report =
        run_windowed_native_player_from_package(NativePlayerWindowRunRequest::windowed(package));

    assert_eq!(
        report.schema_version,
        NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.present_status, NativeWindowPresentStatus::Presented);
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
#[test]
#[ignore = "run-owned existing package; real Windows Player consumer, no Cook/export"]
fn real_windowed_existing_lit_package() {
    let sample_frames = std::env::var("S2D_LIT_SAMPLE_FRAMES")
        .map(|v| v.parse::<u64>().expect("sample count"))
        .unwrap_or(8);
    assert!((2..=600).contains(&sample_frames));
    let warmup_frames = std::env::var("S2D_LIT_WARMUP_FRAMES")
        .map(|v| v.parse::<u64>().expect("warmup count"))
        .unwrap_or(4);
    assert!((4..=120).contains(&warmup_frames));
    let capture = std::env::var("S2D_LIT_NO_SCREENSHOT").as_deref() != Ok("1");
    let package =
        PathBuf::from(std::env::var_os("S2D_LIT_PACKAGE").expect("existing owned package"));
    let output =
        PathBuf::from(std::env::var_os("S2D_LIT_EVIDENCE").expect("owned evidence directory"));
    fs::create_dir_all(&output).unwrap();
    let trace = std::env::var("S2D_LIT_TRACE").as_deref() == Ok("1");
    let mut request = NativePlayerWindowRunRequest::windowed(package)
        .with_game_view_target(GameViewTargetSpec::new(
            1920,
            1080,
            engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
        ))
        .with_frame_performance_sample(warmup_frames, sample_frames)
        .with_measurement_policy(measurement::MeasurementPolicy {
            level: if trace {
                measurement::MeasurementLevel::Trace
            } else {
                measurement::MeasurementLevel::Summary
            },
            first_frame: trace.then_some(warmup_frames + 1),
            end_frame: trace.then_some(warmup_frames + sample_frames + 1),
            ..Default::default()
        });
    if capture {
        request = request.with_screenshot(output.join("player.png"));
    }
    request.config.width = 1920;
    request.config.height = 1080;
    request.config.target_fps = 0;
    if let Ok(mode) = std::env::var("S2D_LIT_PRESENT_MODE") {
        assert!(matches!(mode.as_str(), "Fifo" | "Immediate" | "Mailbox"));
        request.config.present_mode = mode;
    }
    request.runtime_report_level = WindowedPlayerRuntimeReportLevel::Summary;
    request.frame_limit = warmup_frames + sample_frames;
    let report = run_windowed_native_player_from_package(request);
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(report.exit_code, 0, "{:?}", report.diagnostics);
    assert_eq!(report.frames_completed, warmup_frames + sample_frames);
    assert_eq!(report.present_status, NativeWindowPresentStatus::Presented);
    let loading = report
        .geometry_loading
        .as_ref()
        .expect("real segmented texture loading");
    assert!(loading.presented_frames > 0);
    assert_eq!(
        report.surface.presented_frame_count,
        report.frames_completed + loading.presented_frames
    );
    assert!(
        loading.uploaded_bytes >= 536_873_472,
        "loading now includes production nontexture writes as well as the original textures"
    );
    assert!(loading.max_frame_uploaded_bytes <= 16 * 1024 * 1024);
    assert!(loading.peak_staging_bytes <= 128 * 1024 * 1024);
    assert!(loading.peak_in_flight <= 2);
    if capture {
        assert_eq!(
            report.screenshot.status,
            NativeWindowScreenshotStatus::Captured
        );
    } else {
        assert!(!report.screenshot.requested);
    }
    let summary = report.frame_performance_summary.as_ref().unwrap();
    assert_eq!(summary.observed_sample_frames, sample_frames);
    assert!(summary
        .gpu
        .as_ref()
        .is_some_and(|gpu| gpu.status == "complete"));
}

#[cfg(feature = "real-window")]
#[test]
#[ignore = "real OS window / GPU screenshot smoke gate is local-only"]
fn real_windowed_player_screenshot_smoke() {
    let root = temp_root("real-windowed-screenshot-smoke");
    let package = write_spatial_screenshot_runtime_package(&root);
    let screenshot = root.join("reports").join("windowed-player-screenshot.png");
    let mut request = NativePlayerWindowRunRequest::windowed(package).with_screenshot(&screenshot);
    let sample_frames = std::env::var("S1D_GPU_SAMPLE_FRAMES")
        .map(|v| v.parse::<u64>().expect("S1D sample count"))
        .unwrap_or(8);
    assert!((8..=7200).contains(&sample_frames));
    request.frame_limit = sample_frames + 4;
    request.performance_sample_frames = sample_frames;
    request.performance_warmup_frames = 2;
    let matrix_policy = std::env::var("S1D_MEASUREMENT_POLICY").ok().map(|value| {
        use measurement::{MeasurementLevel, MeasurementPolicy};
        match value.as_str() {
            "off" => MeasurementPolicy::default(),
            "summary" => MeasurementPolicy {
                level: MeasurementLevel::Summary,
                ..Default::default()
            },
            "trace" => MeasurementPolicy {
                level: MeasurementLevel::Trace,
                first_frame: Some(3),
                end_frame: Some(7),
                ..Default::default()
            },
            _ => panic!("unknown S1D measurement policy"),
        }
    });
    if let Some(policy) = matrix_policy {
        request.measurement = Some(policy);
    }
    let report = run_windowed_native_player_from_package(request);
    if let Some(path) = std::env::var_os("S1D_GPU_EVIDENCE_DIR") {
        std::fs::write(
            std::path::Path::new(&path).join("pc-window-report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }

    assert_eq!(
        report.schema_version,
        NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        report.present_status,
        NativeWindowPresentStatus::Presented,
        "{report:?}"
    );
    assert_eq!(
        report.screenshot.status,
        NativeWindowScreenshotStatus::Captured
    );
    assert!(screenshot.exists());
    let mut png = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&screenshot).unwrap(),
    ))
    .read_info()
    .unwrap();
    let mut pixels = vec![0; png.output_buffer_size()];
    let image = png.next_frame(&mut pixels).unwrap();
    assert_eq!(image.color_type, png::ColorType::Rgba);
    let at = ((image.height / 2 * image.width + image.width / 2) * 4) as usize;
    assert_eq!(
        &pixels[at..at + 4],
        &[0, 0, 255, 255],
        "real package geometry must reach the captured production frame"
    );
    if let Some(policy) = matrix_policy {
        assert_eq!(report.measurement_policy, Some(policy));
        if policy.level == measurement::MeasurementLevel::Off {
            assert!(report.frame_performance_summary.is_none());
        } else {
            let summary = report.frame_performance_summary.as_ref().unwrap();
            assert_eq!(summary.observed_sample_frames, sample_frames);
            let gpu = summary.gpu.as_ref().unwrap();
            assert_eq!(gpu.status, "complete");
            let native = gpu.native_present.as_ref().unwrap();
            assert!(
                native.windows_etw.is_none(),
                "explicit policy did not opt in to ETW"
            );
            if policy.level == measurement::MeasurementLevel::Summary {
                assert!(summary.raw_cpu_frames.is_empty());
                assert!(gpu.samples.is_empty() && gpu.submissions.is_empty());
                assert!(native.frames.is_empty());
            } else {
                assert_eq!(
                    summary
                        .raw_cpu_frames
                        .iter()
                        .map(|f| f.frame_index)
                        .collect::<Vec<_>>(),
                    vec![3, 4, 5, 6]
                );
                assert_eq!(
                    gpu.samples
                        .iter()
                        .map(|f| f.frame_index)
                        .collect::<Vec<_>>(),
                    vec![3, 4, 5, 6]
                );
                assert_eq!(
                    native
                        .frames
                        .iter()
                        .map(|f| f.submission.frame_index)
                        .collect::<Vec<_>>(),
                    vec![3, 4, 5, 6]
                );
                assert!(gpu
                    .submissions
                    .iter()
                    .all(|s| policy.trace_frame(s.token.frame_index)));
            }
        }
        if let Some(path) = std::env::var_os("S1D_GPU_EVIDENCE_DIR") {
            std::fs::copy(
                &screenshot,
                std::path::Path::new(&path).join("pc-window-capture.png"),
            )
            .unwrap();
        }
        return;
    }
    let gpu = report
        .frame_performance_summary
        .as_ref()
        .unwrap()
        .gpu
        .as_ref()
        .unwrap();
    assert_eq!(gpu.status, "complete");
    assert!(gpu
        .samples
        .iter()
        .all(|s| s.submission.is_some() && s.frame_ms.is_finite() && s.frame_ms > 0.));
    let summary = report.frame_performance_summary.as_ref().unwrap();
    assert_eq!(gpu.samples.len() as u64, sample_frames);
    assert_eq!(gpu.submission_observations_omitted, 0);
    assert!(gpu.measurement_buffer_bytes <= 64 * 1024 * 1024);
    assert!(gpu.measurement_used_bytes <= gpu.measurement_buffer_bytes);
    let native = gpu
        .native_present
        .as_ref()
        .expect("native presentation capability report");
    assert_eq!(native.frames.len() as u64, sample_frames + 4);
    assert!(native.frames.iter().all(|f| f.swapchain_epoch > 0
        && f.image_index < 16
        && f.frame_identity
            .as_ref()
            .is_some_and(|id| id.frame_index == f.submission.frame_index)));
    assert!(native.discarded_frames.is_none() && native.repeated_displays.is_none());
    #[cfg(windows)]
    if let Some(etw) = &native.windows_etw {
        assert_eq!(etw.process_id, std::process::id());
        assert!(etw.qpc_frequency.is_some_and(|f| f > 0));
        assert_eq!(etw.decode_status, "not_attempted_no_capture");
        assert!(etw.decoded.is_none());
        assert!(native.frames.iter().all(|f| f
            .application_present_qpc
            .is_some_and(|q| q[0] > 0 && q[1] >= q[0])));
        assert!(native
            .frames
            .iter()
            .all(|f| f.application_present_thread_id.is_some_and(|id| id > 0)));
        assert!(native
            .frames
            .iter()
            .all(|f| f.actual_present_time_ns.is_none()));
        if etw.start_error.is_some() {
            assert_eq!(etw.cleanup_status, "not_created");
            assert_eq!(etw.stop_attempts, 0);
            assert!(etw.events_lost.is_none());
        }
        assert_ne!(etw.cleanup_status, "owned_session_active");
        assert_ne!(etw.cleanup_status, "stop_failed_session_may_remain");
    }
    assert!(native
        .frames
        .iter()
        .all(|f| gpu.submissions.iter().any(|s| s.token == f.submission
            && s.surface_present_calls == 1
            && s.frame_identity == f.frame_identity)));
    #[cfg(windows)]
    assert!(summary
        .raw_cpu_frames
        .iter()
        .all(|s| s.process_memory.is_some_and(|m| m.working_set_bytes > 0
            && m.private_commit_bytes > 0
            && m.sampling_cpu_ns > 0)));
    for sample in &gpu.samples {
        let token = sample.submission.unwrap();
        assert_eq!(token.frame_index, sample.frame_index);
        let identity = sample.frame_identity.as_ref().expect("GPU frozen source");
        assert_eq!(report.run_id, identity.source.run_id);
        assert_eq!(identity.frame_index, token.frame_index);
        assert!(identity.source.run_epoch > 0 && identity.source.world_epoch > 0);
        assert!(!identity.views.is_empty());
        assert!(summary
            .raw_cpu_frames
            .iter()
            .any(|cpu| cpu.frame_index == token.frame_index
                && cpu.frame_identity.as_ref() == Some(identity)));
        assert!(summary
            .raw_cpu_frames
            .iter()
            .any(|cpu| cpu.frame_index == token.frame_index && cpu.sampled && !cpu.warmup));
        assert!(gpu
            .submissions
            .iter()
            .any(|s| s.token == token && !s.offscreen_capture && s.surface_present_calls == 1));
        assert!(gpu
            .submissions
            .iter()
            .any(|s| s.token == token && s.frame_identity.as_ref() == Some(identity)));
    }
    assert!(gpu.submissions.iter().any(|s| s.surface_present_calls == 1));
    assert!(gpu.submissions.iter().any(|s| s.capture_count == 1));
    let captured = report.screenshot.submission.unwrap();
    let capture_identity = report
        .screenshot
        .frame_identity
        .as_ref()
        .expect("capture frozen source");
    assert_eq!(capture_identity.frame_index, captured.frame_index);
    assert!(gpu
        .submissions
        .iter()
        .any(|s| s.token == captured && s.frame_identity.as_ref() == Some(capture_identity)));
    assert!(gpu
        .submissions
        .iter()
        .any(|s| s.token == captured && s.capture_count == 1 && s.surface_present_calls == 0));
    assert!(gpu
        .submissions
        .iter()
        .filter(|s| s.surface_present_calls > 0)
        .all(|s| s.surface_present_calls == 1));
    if let Some(path) = std::env::var_os("S1D_GPU_EVIDENCE_DIR") {
        let path = std::path::Path::new(&path);
        std::fs::write(
            path.join("pc-window-report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        std::fs::copy(&screenshot, path.join("pc-window-capture.png")).unwrap();
    }
}

pub(crate) fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("runtime-player-winit-{name}-{stamp}"))
}

fn test_aui_present_output() -> AuiRuntimePresentOutput {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["pause_button"]);
    let pause_button = AuiNode::new(
        "pause_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(80.0, 80.0, 180.0, 80.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_action(AuiActionRef::click("ui.pause"));
    let document = AuiDocument::new(
        "runtime-interaction-test",
        vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
        vec![root, pause_button],
    );
    AuiRuntimePresenter::present_package_smoke(&document, 1)
}

fn test_aui_controls_present_output() -> AuiRuntimePresentOutput {
    use engine_runtime::aui::{
        AuiActionRef, AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect,
    };
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["toggle", "slider"]);
    let toggle = AuiNode::new(
        "toggle",
        AuiNodeKind::Toggle,
        AuiRect::fixed_position(0.0, 0.0, 100.0, 40.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_toggle_checked(false)
    .with_action(AuiActionRef::value_changed("settings.toggle"));
    let slider = AuiNode::new(
        "slider",
        AuiNodeKind::Slider,
        AuiRect::fixed_position(0.0, 60.0, 100.0, 40.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_slider(0.25, 0.0, 1.0, 0.1)
    .with_action(AuiActionRef::value_changed("settings.slider"));
    let mut document = AuiDocument::new(
        "controls",
        vec![AuiCanvas::screen_overlay("main", 200.0, 120.0, "root")],
        vec![root, toggle, slider],
    );
    document.schema_version = engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
    AuiRuntimePresenter::present_package_smoke(&document, 1)
}

fn test_aui_text_entry_present_output() -> AuiRuntimePresentOutput {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["name_input"]);
    let input = AuiNode::new(
        "name_input",
        AuiNodeKind::InputField,
        AuiRect::fixed_position(80.0, 80.0, 220.0, 44.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_text("A")
    .with_action(AuiActionRef::text_changed("ui.name_changed"))
    .with_action(AuiActionRef::text_submitted("ui.name_submitted"))
    .with_action(AuiActionRef::text_cancelled("ui.name_cancelled"));
    let mut canvas = AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root");
    canvas.default_focus_node_id = Some("name_input".to_string());
    let document = AuiDocument::new("runtime-text-entry-test", vec![canvas], vec![root, input]);
    AuiRuntimePresenter::present_package_smoke(&document, 1)
}

pub(crate) fn write_minimal_runtime_package(root: &Path, name: &str) -> PathBuf {
    let package_dir = root.join(name);
    fs::create_dir_all(package_dir.join("scenes")).unwrap();
    fs::create_dir_all(package_dir.join("assets")).unwrap();
    fs::create_dir_all(package_dir.join("input")).unwrap();
    fs::create_dir_all(package_dir.join("rules")).unwrap();
    fs::write(
            package_dir.join("manifest.json"),
            r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "project-runtime-player-test",
    "name": "Runtime Player Test",
    "version": "0.1.0",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 1 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "none" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.default", "mappingCount": 1 },
  "contentHash": "testhash"
}"#,
        )
        .unwrap();
    fs::write(
        package_dir.join("scenes").join("scene-main.json"),
        r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#101010",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
    "id": "entity-main",
    "name": "Main Entity",
    "kind": "actor",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    }
  }]
}"##,
    )
    .unwrap();
    fs::write(
        package_dir.join("assets").join("asset-manifest.json"),
        r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [{
    "id": "scene-main",
    "name": "Main",
    "type": "scene",
    "source": "scenes/scene-main.json",
    "state": "available",
    "bundleId": "startup"
  }],
  "runtimeAssetIndex": [{
    "assetGuid": "scene-main",
    "assetId": "scene-main",
    "assetType": "scene",
    "subAssetId": null,
    "version": "1",
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "loaderKind": "scene",
    "dependencies": [],
    "hash": null,
    "size": null,
    "flags": ["test"]
  }],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [{
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "path": "scenes/scene-main.json",
    "offset": null,
    "size": null,
    "compression": "none",
    "hash": null
  }],
  "dependencyTable": []
}"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("rules").join("rule-manifest.json"),
        r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "none",
  "rules": [],
  "modules": []
}"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("input").join("input-manifest.json"),
        r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.default",
  "mappings": [{ "id": "input.default", "path": "input/input.default.json", "enabled": true }]
}"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("input").join("input.default.json"),
        serde_json::to_string_pretty(&InputMappingAsset::gameplay_default()).unwrap(),
    )
    .unwrap();
    package_dir
}

fn add_minimal_aui_document(package_dir: &Path) {
    fs::create_dir_all(package_dir.join("aui").join("documents")).unwrap();
    let manifest_path = package_dir.join("manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path).unwrap().replace(
            "\"input\": { \"path\": \"input/input-manifest.json\", \"defaultMappingId\": \"input.default\", \"mappingCount\": 1 },",
            "\"input\": { \"path\": \"input/input-manifest.json\", \"defaultMappingId\": \"input.default\", \"mappingCount\": 1 },\n  \"aui\": { \"path\": \"aui/aui-manifest.json\", \"documentCount\": 1 },",
        );
    fs::write(manifest_path, manifest_text).unwrap();
    fs::write(
        package_dir.join("aui").join("aui-manifest.json"),
        r#"{
  "schemaVersion": "runtime-aui-manifest.v1",
  "documents": [{
    "documentId": "hud",
    "path": "aui/documents/hud.aui.json",
    "canvasCount": 1,
    "nodeCount": 2,
    "bindingCount": 1,
    "actionCount": 0,
    "assetRefs": []
  }]
}"#,
    )
    .unwrap();
    fs::write(
        package_dir
            .join("aui")
            .join("documents")
            .join("hud.aui.json"),
        r##"{
  "schema_version": "aui-document.v2",
  "document_id": "hud",
  "canvases": [{
    "canvas_id": "main",
    "mode": "ScreenOverlay",
    "layer": 0,
    "sorting_order": 0,
    "reference_resolution": { "x": 1280.0, "y": 720.0 },
    "scale_mode": "ConstantPixelSize",
    "root_node": "root"
  }],
  "nodes": [{
    "node_id": "root",
    "name": "root",
    "kind": "Panel",
    "parent": null,
    "children": ["score"],
    "rect": {
      "anchor_min": { "x": 0.0, "y": 0.0 },
      "anchor_max": { "x": 1.0, "y": 1.0 },
      "offset_min": { "x": 0.0, "y": 0.0 },
      "offset_max": { "x": 0.0, "y": 0.0 },
      "pivot": { "x": 0.5, "y": 0.5 },
      "size": { "x": 0.0, "y": 0.0 }
    },
    "visible": true,
    "interactable": false,
    "consume_input": true,
    "style": { "color": "#101820", "text_color": null, "font_size": null },
    "text": null,
    "image": null,
    "progress_value": null,
    "binding_refs": [],
    "action_refs": []
  }, {
    "node_id": "score",
    "name": "score",
    "kind": "Text",
    "parent": "root",
    "children": [],
    "rect": {
      "anchor_min": { "x": 0.0, "y": 0.0 },
      "anchor_max": { "x": 0.0, "y": 0.0 },
      "offset_min": { "x": 24.0, "y": 24.0 },
      "offset_max": { "x": 0.0, "y": 0.0 },
      "pivot": { "x": 0.0, "y": 0.0 },
      "size": { "x": 260.0, "y": 36.0 }
    },
    "visible": true,
    "interactable": false,
    "consume_input": true,
    "style": { "color": null, "text_color": "#ffffff", "font_size": 24.0 },
    "text": "SCORE",
    "image": null,
    "progress_value": null,
    "binding_refs": [{
      "binding_id": "score.text",
      "target_field": "TextText",
      "path": "game.score_text",
      "fallback": "SCORE 000000"
    }],
    "action_refs": []
  }]
}"##,
    )
    .unwrap();
}

#[cfg(feature = "real-window")]
pub(crate) fn write_spatial_screenshot_runtime_package(root: &Path) -> PathBuf {
    use engine_runtime::render_contract::*;
    use engine_runtime::runtime_package::*;
    use engine_runtime::runtime_package_builder::*;
    use engine_runtime::spatial_contract::{decode_runtime_scene, RuntimeSceneDocument};
    let package = root.join("runtime-package");
    let mut document: serde_json::Value = serde_json::from_str(include_str!(
        "../../../engine_runtime/tests/fixtures/three_d_a0/runtime-scene-v2.json"
    ))
    .unwrap();
    document["entities"][1]["spatial"]["transform"]["translation"] = serde_json::json!([0, 0, 3]);
    let RuntimeSceneDocument::Spatial(scene) =
        decode_runtime_scene(&serde_json::to_vec(&document).unwrap()).unwrap()
    else {
        panic!("spatial scene required")
    };
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "s1d-window",
        "S1-D geometry",
        "1",
    ));
    input.scenes.push(scene.into());
    input.assets.push(RuntimePackageSourceAsset::new(
        "main",
        "Main",
        "scene",
        "Scenes/main.json",
        "scenes/main.json",
    ));
    let mapping = InputMappingAsset::gameplay_default();
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    for (id, kind, text) in [
        (
            "cube",
            "mesh",
            include_str!("../../../engine_runtime/tests/fixtures/three_d_a12/cube.json"),
        ),
        (
            "blue",
            "material",
            include_str!("../../../engine_runtime/tests/fixtures/three_d_a12/blue.json"),
        ),
    ] {
        let asset = RenderContract::<CookedRenderAsset>::decode(text)
            .unwrap()
            .data;
        let bytes = serde_json::to_vec(&RenderContract::new(asset)).unwrap();
        let mut source = RuntimePackageSourceAsset::new(
            id,
            id,
            kind,
            format!("Assets/{id}.json"),
            format!("cooked/{id}.json"),
        );
        source.hash = Some(engine_runtime::canonical_digest::sha256_prefixed(&bytes));
        input.assets.push(source.with_runtime_payload(bytes));
    }
    let built = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&package, "main"),
        &input,
    );
    assert_eq!(
        built.status,
        RuntimePackageBuildStatus::Success,
        "{built:?}"
    );
    package
}
