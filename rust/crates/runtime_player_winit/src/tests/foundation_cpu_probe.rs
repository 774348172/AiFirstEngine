use super::*;

#[test]
fn spine374_f_off_probe_keeps_ticks_without_retaining_render_identity() {
    use engine_runtime::render_thread::{RenderFrameIdentity, RenderFrameSourceIdentity};
    use std::sync::Arc;
    for level in [
        measurement::MeasurementLevel::Off,
        measurement::MeasurementLevel::Summary,
        measurement::MeasurementLevel::Trace,
    ] {
        let identity = Arc::new(RenderFrameIdentity {
            source: Arc::new(RenderFrameSourceIdentity {
                run_id: "probe".into(),
                run_epoch: 1,
                session_id: "main".into(),
                package_content_hash: None,
                world_epoch: 1,
            }),
            frame_index: 601,
            tick_pair: [599, 600],
            presentation_alpha_bits: 0,
            views: Arc::new(Vec::new()),
        });
        let weak = Arc::downgrade(&identity);
        let mut work = AuiFrameWorkCounters::default();
        measurement::capture_render_identity(
            &mut work,
            measurement::MeasurementPolicy {
                level,
                cpu_probe: true,
                ..Default::default()
            },
            Some(&identity),
        );
        drop(identity);
        assert_eq!(work.tick_pair, Some([599, 600]));
        assert_eq!(
            weak.upgrade().is_some(),
            level != measurement::MeasurementLevel::Off
        );
        assert_eq!(
            work.frame_identity.is_some(),
            level != measurement::MeasurementLevel::Off
        );
        measurement::capture_render_identity(&mut work, Default::default(), None);
        assert!(work.tick_pair.is_none() && work.frame_identity.is_none());
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn spine374_f_off_memory_window_preserves_warmup_and_tail_failures() {
    let mut history = (1..=4200)
        .map(|frame_index| AuiFrameWorkCounters {
            frame_index,
            process_memory: Some(
                engine_runtime::windowed_player::WindowedPlayerProcessMemorySample {
                    working_set_bytes: 1,
                    private_commit_bytes: 8_000_000_000 + frame_index * 1024,
                    system_available_bytes: Some(8_000_000_000),
                    sampling_cpu_ns: 1,
                },
            ),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let (available, growth) = measurement::memory_window(&history, 4200);
    assert_eq!(available, Some(8_000_000_000));
    assert!((growth.unwrap() - 1024.).abs() < 1e-8);
    assert_eq!(
        measurement::memory_percentiles(&history[600..]),
        Some([8_000_000_000 + 4020 * 1024, 8_000_000_000 + 4164 * 1024])
    );
    assert_eq!(measurement::memory_percentiles(&[]), None);
    history[0]
        .process_memory
        .as_mut()
        .unwrap()
        .private_commit_bytes = 9_000_000_000;
    assert_eq!(
        measurement::requested_memory_peak(&history, 4200),
        Some(9_000_000_000)
    );
    assert_eq!(
        measurement::requested_memory_peak(&history[1..], 4199),
        None
    );
    assert_eq!(
        measurement::requested_memory_peak(&history[..4199], 4200),
        None
    );
    assert_eq!(measurement::requested_memory_peak(&[], 0), None);
    assert_eq!(
        measurement::memory_percentiles(&history[600..]),
        Some([8_000_000_000 + 4020 * 1024, 8_000_000_000 + 4164 * 1024])
    );
    history[0]
        .process_memory
        .as_mut()
        .unwrap()
        .system_available_bytes = Some(4_831_838_207);
    assert_eq!(
        measurement::memory_window(&history, 4200).0,
        Some(4_831_838_207)
    );
    for (i, frame) in history.iter_mut().skip(3000).enumerate() {
        frame.process_memory.as_mut().unwrap().private_commit_bytes += i as u64;
    }
    assert!((measurement::memory_window(&history, 4200).1.unwrap() - 1025.).abs() < 1e-8);
    history[3200].process_memory = None;
    assert_eq!(measurement::requested_memory_peak(&history, 4200), None);
    assert_eq!(measurement::memory_percentiles(&history), None);
    assert_eq!(measurement::memory_window(&history, 4200), (None, None));
    history[3200].process_memory = history[3199].process_memory;
    history[3200].frame_index = 1;
    assert_eq!(measurement::requested_memory_peak(&history, 4200), None);
    assert_eq!(measurement::memory_window(&history, 4200), (None, None));
    assert_eq!(measurement::memory_window(&[], 4200), (None, None));
    assert_eq!(measurement::memory_window(&history[..1199], 1199).1, None);
}

#[test]
fn spine374_f_measurement_history_has_fixed_backing_before_the_first_frame() {
    for (level, probe) in [
        (measurement::MeasurementLevel::Off, false),
        (measurement::MeasurementLevel::Off, true),
        (measurement::MeasurementLevel::Summary, false),
        (measurement::MeasurementLevel::Trace, false),
    ] {
        let mut request = NativePlayerWindowRunRequest::windowed("package")
            .with_frame_performance_sample(600, 3600)
            .with_measurement_policy(measurement::MeasurementPolicy {
                level,
                cpu_probe: probe,
                ..Default::default()
            });
        request.frame_limit = 4200;
        let enabled = request.cpu_sampling_enabled();
        let mut history = cpu_frame_history::<AuiFrameWorkCounters>(&request);
        let capacity = history.capacity();
        let pointer = history.as_ptr();
        let mut evaluations = 0;
        if enabled {
            assert!(
                capacity >= 4200,
                "CPU history starts at {capacity}, so it must grow during measurement"
            );
        } else {
            assert_eq!(capacity, 0, "ordinary Off allocated a measurement history");
        }
        for frame in 1..=4200 {
            record_cpu_frame_sample(enabled, &mut history, || {
                evaluations += 1;
                AuiFrameWorkCounters {
                    frame_index: frame,
                    ..Default::default()
                }
            });
            assert_eq!(history.capacity(), capacity);
            assert_eq!(history.as_ptr(), pointer);
        }
        if enabled {
            assert_eq!(evaluations, 4200);
            assert_eq!(history.first().unwrap().frame_index, 1);
            assert_eq!(history.last().unwrap().frame_index, 4200);
            eprintln!(
                "level={level:?} probe={probe} historyElementBytes={} initialBackingBytes={}",
                std::mem::size_of::<AuiFrameWorkCounters>(),
                capacity * std::mem::size_of::<AuiFrameWorkCounters>()
            );
        } else {
            assert_eq!(evaluations, 0);
            assert!(history.is_empty());
        }
    }
}

#[test]
fn foundation_off_cpu_probe_is_explicit_bounded_and_not_full_measurement() {
    let policy: measurement::MeasurementPolicy =
        serde_json::from_value(serde_json::json!({"level":"off","cpuProbe":true}))
            .expect("explicit Off CPU probe must parse");
    let mut request = NativePlayerWindowRunRequest::windowed("package")
        .with_frame_performance_sample(1, 3)
        .with_measurement_policy(policy);
    request.frame_limit = 4;
    assert!(
        !request.measurement_enabled(),
        "GPU/UI/native measurement must stay off"
    );
    let mut report = NativeWindowHostReport::base(&request);
    assert!(validate_run_evidence_request(&request, &mut report));
    let values = [10, 20, 30, 40];
    let work = vec![AuiFrameWorkCounters::default(); 4];
    finalize_frame_performance(
        &mut report,
        &request,
        FramePerformanceSamples {
            cpu_ns: &values,
            aui_cpu_ns: &values,
            aui_work: &work,
            update_ns: &values,
            render_submit_ns: &values,
            present_wait_ns: &[0; 4],
        },
    );
    let summary = report
        .frame_performance_summary
        .expect("same CPU probe must yield Off baseline");
    assert_eq!(summary.observed_sample_frames, 3);
    assert!(summary.raw_cpu_frames.is_empty());
    assert!(summary.gpu.is_none());
    request.frame_limit = measurement::MAX_CAPTURE_FRAMES + 1;
    let mut report = NativeWindowHostReport::base(&request);
    assert!(!validate_run_evidence_request(&request, &mut report));
    request.frame_limit = 4;
    request.performance_sample_frames = 0;
    let mut report = NativeWindowHostReport::base(&request);
    assert!(!validate_run_evidence_request(&request, &mut report));
}
