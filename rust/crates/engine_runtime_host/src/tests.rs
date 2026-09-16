use super::*;
use runtime_player_winit::semantic_outcome::{PlaytestScenario, PLAYTEST_SCENARIO_SCHEMA_VERSION};
use runtime_player_winit::NativePlayerInputScript;
use std::sync::atomic::{AtomicU64, Ordering};

fn request(mode: NativePlayerWindowRunMode) -> EngineRuntimeExecutionRequest {
    let mut request = NativePlayerWindowRunRequest::headless_surface_gate("missing-package");
    request.mode = mode;
    EngineRuntimeExecutionRequest {
        request,
        scenario: None,
        report_path: "player.json".into(),
        capture_directory: None,
    }
}

fn scenario(target: PlaytestTarget) -> PlaytestScenario {
    PlaytestScenario {
        schema_version: PLAYTEST_SCENARIO_SCHEMA_VERSION.into(),
        scenario_id: "engine-dll-test".into(),
        initial_scene_id: "scene.main".into(),
        target,
        max_simulation_ticks: 1,
        max_presentation_frames: 1,
        timeout_ms: 1000,
        seed: None,
        inputs: vec![],
        assertions: vec![runtime_player_winit::semantic_outcome::PlaytestAssertion {
            assertion_id: "alive".into(),
            from_simulation_tick: 1,
            through_simulation_tick: 1,
            path: "project.alive".into(),
            equals: engine_runtime::project_observation::ProjectObservationValue::Bool(true),
        }],
        captures: vec![],
    }
}

fn temp_directory() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "aife-engine-dll-test-{}-{nanos}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}

#[test]
fn execution_api_advertises_additive_capability_and_preserves_old_entrypoints() {
    let info = aife_engine_runtime_api_info_v1();
    assert_eq!(info.api_major, 1);
    assert_eq!(info.api_minor, ENGINE_RUNTIME_EXECUTION_MIN_API_MINOR);
    assert_eq!(info.capabilities & 0b1_1111, 0b1_1111);
    assert_ne!(info.capabilities & ENGINE_RUNTIME_EXECUTION_CAPABILITY, 0);
    assert_eq!(aife_engine_runtime_start_v1(), ENGINE_RUNTIME_OK);
    assert_eq!(aife_engine_runtime_stop_v1(), ENGINE_RUNTIME_OK);
    let invalid = unsafe { aife_engine_runtime_run_headless_v1(std::ptr::null()) };
    assert_eq!(invalid.status, ENGINE_RUNTIME_INVALID_ARGUMENT);
    assert_eq!(
        invalid.struct_size as usize,
        std::mem::size_of::<EngineRuntimeRunResultV1>()
    );
}

#[test]
fn execution_rejects_invalid_byte_ranges_and_malformed_payloads_before_reading() {
    let bytes = b"{";
    for (pointer, length) in [
        (std::ptr::null(), 1),
        (bytes.as_ptr(), 0),
        (bytes.as_ptr(), ENGINE_RUNTIME_EXECUTION_MAX_BYTES + 1),
        (bytes.as_ptr(), bytes.len()),
    ] {
        let result = unsafe { aife_engine_runtime_execute_v1(pointer, length) };
        assert_eq!(result.status, ENGINE_RUNTIME_INVALID_ARGUMENT);
        assert_eq!(result.frames_completed, 0);
    }
}

#[test]
fn execution_accepts_existing_plain_and_semantic_modes_without_coercion() {
    for (mode, target) in [
        (
            NativePlayerWindowRunMode::HeadlessSurfaceGate,
            PlaytestTarget::WindowsHeadless,
        ),
        (
            NativePlayerWindowRunMode::Windowed,
            PlaytestTarget::WindowsWindowed,
        ),
    ] {
        let mut execution = request(mode);
        assert!(validate_execution(&execution).is_ok());
        execution.scenario = Some(scenario(target));
        execution.capture_directory = Some("captures".into());
        assert!(validate_execution(&execution).is_ok());
        let bytes = serde_json::to_vec(&execution).unwrap();
        let decoded: EngineRuntimeExecutionRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, execution);
    }
}

#[test]
fn execution_rejects_semantic_mode_input_and_capture_conflicts() {
    let mut execution = request(NativePlayerWindowRunMode::Windowed);
    execution.scenario = Some(scenario(PlaytestTarget::WindowsHeadless));
    assert!(validate_execution(&execution).is_err());
    execution.scenario = Some(scenario(PlaytestTarget::WindowsWindowed));
    assert!(
        validate_execution(&execution).is_err(),
        "capture directory is required"
    );
    execution.capture_directory = Some("captures".into());
    execution.request.input_script = Some(NativePlayerInputScript::new("input", vec![]));
    assert!(
        validate_execution(&execution).is_err(),
        "scenario owns input"
    );
    execution.request.input_script = None;
    execution.request.screenshot.enabled = true;
    assert!(
        validate_execution(&execution).is_err(),
        "scenario owns capture"
    );
}

#[test]
fn execution_missing_package_reports_failure_in_requested_mode() {
    let root = temp_directory();
    let report_path = root.join("player.json");
    let mut execution = request(NativePlayerWindowRunMode::Windowed);
    execution.request.runtime_package_path = root.join("missing-package");
    execution.report_path = report_path.clone();
    let bytes = serde_json::to_vec(&execution).unwrap();
    let result = unsafe { aife_engine_runtime_execute_v1(bytes.as_ptr(), bytes.len()) };
    assert_eq!(result.status, ENGINE_RUNTIME_EXECUTION_FAILED);
    assert_eq!(result.frames_completed, 0);
    let report: NativeWindowHostReport =
        serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    assert_eq!(report.mode, NativePlayerWindowRunMode::Windowed);
    assert_ne!(report.exit_code, 0);
    assert!(!report.diagnostics.is_empty());
    fs::remove_file(report_path).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn execution_report_write_failure_cannot_return_success() {
    let root = temp_directory();
    let mut report = NativeWindowHostReport::base(
        &request(NativePlayerWindowRunMode::HeadlessSurfaceGate).request,
    );
    report.exit_code = 0;
    report.frames_completed = 7;
    let result = finish_report(&root, &report);
    assert_eq!(result, (ENGINE_RUNTIME_EXECUTION_FAILED, 7));
    fs::remove_dir(root).unwrap();
}

#[test]
fn legacy_headless_rejects_invalid_path_ranges() {
    let path = b"package";
    let request = EngineRuntimeRunRequestV1 {
        struct_size: std::mem::size_of::<EngineRuntimeRunRequestV1>() as u32,
        package_path: path.as_ptr(),
        package_path_len: 0,
        report_path: path.as_ptr(),
        report_path_len: path.len(),
        frame_limit: 1,
    };
    let result = unsafe { aife_engine_runtime_run_headless_v1(&request) };
    assert_eq!(result.status, ENGINE_RUNTIME_INVALID_ARGUMENT);
    assert_eq!(result.frames_completed, 0);
}
