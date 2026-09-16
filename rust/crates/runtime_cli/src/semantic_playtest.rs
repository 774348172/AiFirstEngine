use crate::{
    run_bounded_child_process, BoundedChildProcessExitReason, BoundedChildProcessPriority,
    BoundedChildProcessRequest, BoundedChildProcessResult,
};
use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use runtime_player_winit::semantic_outcome::{
    OutcomeStatus, PlaytestScenario, PlaytestTarget, SemanticOutcome, MAX_PLAYTEST_SCENARIO_BYTES,
    MAX_PLAYTEST_TIMEOUT_MS,
};
use runtime_player_winit::{
    run_headless_semantic_playtest_with_linked_modules, NativePlayerWindowRunRequest,
    NativeWindowHostReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const MAX_PLAYER_REPORT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SemanticPlaytestProcessRequest {
    pub player_executable: PathBuf,
    pub runtime_package: PathBuf,
    pub scenario: PlaytestScenario,
    /// Must not exist; the caller owns this run's artifacts and their retention.
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPlaytestProcessReport {
    pub run_id: String,
    #[serde(default)]
    pub scenario_digest: String,
    #[serde(default)]
    pub player_digest: String,
    #[serde(default)]
    pub package_digest: String,
    pub process: BoundedChildProcessResult,
    pub player: Option<NativeWindowHostReport>,
    pub outcome: SemanticOutcome,
    pub overall: OutcomeStatus,
    pub diagnostics: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkerInput {
    run_id: String,
    runtime_package: PathBuf,
    scenario: PlaytestScenario,
}

pub fn run_bounded_semantic_playtest(
    request: SemanticPlaytestProcessRequest,
) -> Result<SemanticPlaytestProcessReport, String> {
    if !cfg!(windows) {
        return Err(
            "playtest.target_unsupported: bounded semantic playtest requires Windows".into(),
        );
    }
    if !(1..=MAX_PLAYTEST_TIMEOUT_MS).contains(&request.scenario.timeout_ms) {
        return Err("playtest.timeout_invalid".into());
    }
    let encoded = serde_json::to_vec(&request.scenario).map_err(|error| error.to_string())?;
    if encoded.len() > MAX_PLAYTEST_SCENARIO_BYTES {
        return Err("playtest.scenario_too_large".into());
    }
    let executable = request
        .player_executable
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let package = request
        .runtime_package
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let player_digest = file_digest(&executable)?;
    let package_digest = runtime_package_digest(&package)?;
    fs::create_dir(&request.output_dir)
        .map_err(|error| format!("playtest.output_not_fresh: {error}"))?;
    let output = request
        .output_dir
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let run_id = format!(
        "semantic-playtest:{}:{}:{}",
        output.display(),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let worker_input = WorkerInput {
        run_id: run_id.clone(),
        runtime_package: package.clone(),
        scenario: request.scenario.clone(),
    };
    let input_path = output.join("request.json");
    let player_report_path = output.join("player.json");
    fs::write(
        &input_path,
        serde_json::to_vec(&worker_input).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let process = run_bounded_child_process(BoundedChildProcessRequest {
        executable: executable.clone(),
        args: vec![
            "--semantic-playtest-worker".into(),
            input_path.into_os_string(),
            player_report_path.clone().into_os_string(),
        ],
        current_dir: output.clone(),
        environment: Vec::new(),
        timeout: Duration::from_millis(request.scenario.timeout_ms),
        stdout_capture_limit_bytes: 16 * 1024,
        stderr_capture_limit_bytes: 16 * 1024,
        priority: BoundedChildProcessPriority::Normal,
    });
    let mut report = summarize_process(
        run_id,
        process,
        read_player_report(&player_report_path),
        &request.scenario,
    );
    report.scenario_digest = engine_runtime::canonical_digest::sha256_prefixed(&encoded);
    report.player_digest = player_digest.clone();
    report.package_digest = package_digest.clone();
    if file_digest(&executable).ok().as_ref() != Some(&player_digest)
        || runtime_package_digest(&package).ok().as_ref() != Some(&package_digest)
    {
        report.outcome.technical = OutcomeStatus::Failed;
        report
            .diagnostics
            .push("playtest.artifact_changed_during_run".into());
    }
    if let Err(error) = validate_capture_evidence(&report, &request.scenario, &output) {
        report.outcome.visual = OutcomeStatus::Failed;
        report.diagnostics.push(error);
    }
    report.overall = report.outcome.overall(&request.scenario, false);
    if report.process.exit_reason == BoundedChildProcessExitReason::Timeout {
        report.diagnostics.push(
            "playtest.timeout: child and owned descendants exceeded the wall-clock limit".into(),
        );
    }
    fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(report)
}

fn summarize_process(
    run_id: String,
    process: BoundedChildProcessResult,
    player: Result<NativeWindowHostReport, String>,
    scenario: &PlaytestScenario,
) -> SemanticPlaytestProcessReport {
    let mut diagnostics = Vec::new();
    let mut outcome = SemanticOutcome {
        technical: OutcomeStatus::Failed,
        gameplay: OutcomeStatus::NotProducedYet,
        visual: OutcomeStatus::NotChecked,
        delivery: OutcomeStatus::NotChecked,
    };
    let player = match player {
        Ok(player) => Some(player),
        Err(error) => {
            diagnostics.push(format!("playtest.report_missing_or_invalid: {error}"));
            None
        }
    };
    if let Some(player) = &player {
        if player.run_id == run_id && process.exit_code == Some(player.exit_code) {
            if let Some(semantic) = &player.semantic_playtest {
                if semantic.scenario_id == scenario.scenario_id {
                    outcome = semantic.outcome.clone();
                } else {
                    diagnostics.push("playtest.scenario_identity_mismatch".into());
                }
            } else {
                diagnostics.push("playtest.semantic_result_missing".into());
            }
        } else {
            diagnostics.push("playtest.run_or_exit_mismatch".into());
        }
    }
    if !matches!(
        process.exit_reason,
        BoundedChildProcessExitReason::Completed | BoundedChildProcessExitReason::Failed
    ) || !process.owned_process_cleanup_confirmed()
    {
        outcome.technical = OutcomeStatus::Failed;
        diagnostics.push(format!(
            "playtest.process_failed: {:?}; cleanupConfirmed={}",
            process.exit_reason,
            process.owned_process_cleanup_confirmed()
        ));
    }
    if process.exit_code != Some(0) && outcome.overall(scenario, false) == OutcomeStatus::Passed {
        outcome.technical = OutcomeStatus::Failed;
        diagnostics.push("playtest.nonzero_exit_with_passed_outcome".into());
    }
    let overall = outcome.overall(scenario, false);
    SemanticPlaytestProcessReport {
        run_id,
        scenario_digest: String::new(),
        player_digest: String::new(),
        package_digest: String::new(),
        process,
        player,
        outcome,
        overall,
        diagnostics,
    }
}

fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > limit {
        return Err("playtest.input_or_report_too_large".into());
    }
    Ok(bytes)
}

pub fn semantic_file_digest(path: &Path) -> Result<String, String> {
    file_digest(path)
}

fn file_digest(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

pub fn runtime_package_digest(root: &Path) -> Result<String, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut directories = vec![root.clone()];
    let mut entries = std::collections::BTreeMap::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err("playtest.package_reparse_point".into());
                }
            }
            if metadata.file_type().is_symlink() {
                return Err("playtest.package_symlink".into());
            }
            if metadata.is_dir() {
                directories.push(entry.path());
            } else if metadata.is_file() {
                entries.insert(
                    entry
                        .path()
                        .strip_prefix(&root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    file_digest(&entry.path())?,
                );
            } else {
                return Err("playtest.package_file_type_unsupported".into());
            }
        }
    }
    Ok(engine_runtime::canonical_digest::sha256_prefixed(
        &serde_json::to_vec(&entries).map_err(|e| e.to_string())?,
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPlaytestEvidenceRef {
    pub run_id: String,
    pub report_path: PathBuf,
    pub report_digest: String,
}

pub fn retain_semantic_playtest_evidence(
    report_path: &Path,
) -> Result<SemanticPlaytestEvidenceRef, String> {
    let bytes = read_limited(report_path, MAX_PLAYER_REPORT_BYTES)?;
    let report: SemanticPlaytestProcessReport =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(SemanticPlaytestEvidenceRef {
        run_id: report.run_id,
        report_path: report_path.canonicalize().map_err(|e| e.to_string())?,
        report_digest: engine_runtime::canonical_digest::sha256_prefixed(&bytes),
    })
}

/// Reads an owner-retained reference. Provider must resolve references from its own run registry.
pub fn read_semantic_playtest_evidence(
    reference: &SemanticPlaytestEvidenceRef,
) -> Result<SemanticPlaytestProcessReport, String> {
    let bytes = read_limited(&reference.report_path, MAX_PLAYER_REPORT_BYTES)?;
    if engine_runtime::canonical_digest::sha256_prefixed(&bytes) != reference.report_digest {
        return Err("playtest.evidence_digest_mismatch".into());
    }
    let report: SemanticPlaytestProcessReport =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let directory = reference
        .report_path
        .parent()
        .ok_or("playtest.evidence_path_invalid")?;
    let input: WorkerInput = serde_json::from_slice(&read_limited(
        &directory.join("request.json"),
        MAX_PLAYTEST_SCENARIO_BYTES + 32 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    let digest = engine_runtime::canonical_digest::sha256_prefixed(
        &serde_json::to_vec(&input.scenario).map_err(|e| e.to_string())?,
    );
    if report.run_id != reference.run_id
        || input.run_id != reference.run_id
        || digest != report.scenario_digest
    {
        return Err("playtest.evidence_identity_mismatch".into());
    }
    validate_capture_evidence(&report, &input.scenario, directory)?;
    Ok(report)
}

fn validate_capture_evidence(
    report: &SemanticPlaytestProcessReport,
    scenario: &PlaytestScenario,
    directory: &Path,
) -> Result<(), String> {
    let Some(semantic) = report
        .player
        .as_ref()
        .and_then(|p| p.semantic_playtest.as_ref())
    else {
        return Ok(());
    };
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    let mut seen = std::collections::BTreeSet::new();
    for capture in &semantic.capture_evidence {
        let declared = scenario
            .captures
            .iter()
            .find(|c| c.capture_id == capture.capture_id)
            .ok_or("playtest.capture_undeclared")?;
        if !seen.insert(&capture.capture_id)
            || capture.session_id != semantic.session_id
            || capture.presentation_frame != declared.presentation_frame
            || capture.simulation_tick == 0
            || capture.simulation_tick > semantic.simulation_ticks
            || capture.width == 0
            || capture.height == 0
            || u64::from(capture.width) * u64::from(capture.height) > 4 * 1024 * 1024
        {
            return Err("playtest.capture_identity_invalid".into());
        }
        let path = Path::new(&capture.path)
            .canonicalize()
            .map_err(|e| format!("playtest.capture_missing: {e}"))?;
        if path != directory.join(format!("capture-{}.png", capture.presentation_frame)) {
            return Err("playtest.capture_path_invalid".into());
        }
        let bytes = read_limited(&path, 32 * 1024 * 1024)?;
        if bytes.len() as u64 != capture.byte_size
            || engine_runtime::canonical_digest::sha256_prefixed(&bytes) != capture.sha256
        {
            return Err("playtest.capture_digest_mismatch".into());
        }
    }
    for (id, status) in &semantic.captures {
        if *status == OutcomeStatus::Passed && !seen.contains(id) {
            return Err("playtest.capture_evidence_missing".into());
        }
    }
    Ok(())
}

fn read_player_report(path: &Path) -> Result<NativeWindowHostReport, String> {
    serde_json::from_slice(&read_limited(path, MAX_PLAYER_REPORT_BYTES)?)
        .map_err(|error| error.to_string())
}

pub(crate) fn run_worker(
    args: &[String],
    linked: &Arc<LinkedProjectRuntimeSet>,
) -> Result<i32, String> {
    if args.len() != 3 {
        return Err("playtest.worker_arguments_invalid".into());
    }
    let input: WorkerInput = serde_json::from_slice(&read_limited(
        Path::new(&args[1]),
        MAX_PLAYTEST_SCENARIO_BYTES + 32 * 1024,
    )?)
    .map_err(|error| error.to_string())?;
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("playtest.executable_parent_missing")?
        .to_path_buf();
    let mut report = if crate::requires_engine_dll(&exe_dir, &input.runtime_package) {
        let mut request = match input.scenario.target {
            PlaytestTarget::WindowsHeadless => {
                NativePlayerWindowRunRequest::headless_surface_gate(&input.runtime_package)
            }
            PlaytestTarget::WindowsWindowed => {
                NativePlayerWindowRunRequest::windowed(&input.runtime_package)
            }
        };
        if let Some(target) =
            engine_runtime::runtime_package::load_runtime_package(&input.runtime_package)
                .value
                .and_then(|runtime| runtime.manifest.project.game_view_target)
        {
            request = request.with_game_view_target(target);
        }
        request.frame_limit = input.scenario.max_presentation_frames;
        crate::execute_packaged_request(
            &exe_dir,
            runtime_player_winit::engine_dll_execution::EngineRuntimeExecutionRequest {
                request: request.clone(),
                scenario: Some(input.scenario),
                report_path: PathBuf::from(&args[2]),
                capture_directory: Some(
                    Path::new(&args[2])
                        .parent()
                        .ok_or("playtest.output_missing")?
                        .to_path_buf(),
                ),
            },
        )
        .unwrap_or_else(|error| {
            let mut report = NativeWindowHostReport::base(&request);
            report.exit_code = 1;
            report
                .diagnostics
                .push(runtime_player_winit::NativeWindowHostDiagnostic::error(
                    "engine_runtime.execution_failed",
                    "engine_runtime.dll",
                    error,
                ));
            report
        })
    } else {
        match input.scenario.target {
            PlaytestTarget::WindowsHeadless => run_headless_semantic_playtest_with_linked_modules(
                NativePlayerWindowRunRequest::headless_surface_gate(input.runtime_package),
                linked,
                input.scenario,
            ),
            PlaytestTarget::WindowsWindowed => {
                runtime_player_winit::run_windowed_semantic_playtest_with_linked_modules(
                    NativePlayerWindowRunRequest::windowed(input.runtime_package),
                    Arc::clone(linked),
                    input.scenario,
                    Path::new(&args[2])
                        .parent()
                        .ok_or("playtest.output_missing")?
                        .to_path_buf(),
                )
            }
        }
    };
    report.run_id = input.run_id;
    fs::write(
        &args[2],
        serde_json::to_vec(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(report.exit_code)
}

pub(crate) fn run_cli(args: &[String]) -> Result<i32, String> {
    let mut package = None;
    let mut scenario = None;
    let mut output = None;
    if args.len() != 7 {
        return Err("Usage: playtest --package <runtime-package> --scenario <json> --output-dir <fresh-directory>".into());
    }
    for pair in args[1..].chunks_exact(2) {
        let destination = match pair[0].as_str() {
            "--package" => &mut package,
            "--scenario" => &mut scenario,
            "--output-dir" => &mut output,
            _ => return Err("playtest.unknown_argument".into()),
        };
        if destination.replace(PathBuf::from(&pair[1])).is_some() {
            return Err("playtest.duplicate_argument".into());
        }
    }
    let scenario: PlaytestScenario = serde_json::from_slice(&read_limited(
        &scenario.ok_or("playtest.scenario_missing")?,
        MAX_PLAYTEST_SCENARIO_BYTES,
    )?)
    .map_err(|error| error.to_string())?;
    let report = run_bounded_semantic_playtest(SemanticPlaytestProcessRequest {
        player_executable: std::env::current_exe().map_err(|error| error.to_string())?,
        runtime_package: package.ok_or("playtest.package_missing")?,
        scenario,
        output_dir: output.ok_or("playtest.output_missing")?,
    })?;
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|error| error.to_string())?
    );
    Ok(i32::from(report.overall != OutcomeStatus::Passed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_process_summary_rejects_unconfirmed_cleanup_and_inconsistent_success() {
        let scenario: PlaytestScenario = serde_json::from_value(serde_json::json!({
            "schemaVersion":"playtest-scenario.v1", "scenarioId":"test", "initialSceneId":"scene-main",
            "target":"windows-headless", "maxSimulationTicks":1, "maxPresentationFrames":1, "timeoutMs":1000
        })).unwrap();
        let process = BoundedChildProcessResult {
            process_id: Some(1),
            exit_reason: BoundedChildProcessExitReason::Completed,
            exit_code: Some(0),
            elapsed_ms: 1,
            stdout_summary: String::new(),
            stderr_summary: String::new(),
            stdout_total_bytes: 0,
            stderr_total_bytes: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            spawn_error: None,
            kill_error: None,
            wait_error: None,
            reader_join_error: None,
            ownership: crate::BoundedProcessOwnershipEvidence {
                ownership_kind: crate::BoundedProcessOwnershipKind::WindowsJobObject,
                process_group_created: true,
                root_process_bound: true,
                termination_requested: false,
                root_process_wait_completed: true,
                process_group_release_completed: true,
                output_readers_joined: true,
            },
            priority: Default::default(),
        };
        // Only the summary fields matter here; package loading fails before any process or file writes.
        let mut player = runtime_player_winit::run_headless_native_player_from_package(
            NativePlayerWindowRunRequest::headless_surface_gate(
                std::env::temp_dir().join("semantic-summary-nonexistent-package"),
            ),
        );
        player.run_id = "run".into();
        player.exit_code = 0;
        player.semantic_playtest = Some(runtime_player_winit::SemanticPlaytestReport {
            scenario_id: scenario.scenario_id.clone(),
            session_id: "session".into(),
            contract_digest: None,
            simulation_ticks: 1,
            fixed_delta_seconds: 1.0 / 60.0,
            injected_transition_count: 0,
            assertions: Vec::new(),
            captures: Default::default(),
            capture_evidence: Vec::new(),
            outcome: scenario.summarize(
                OutcomeStatus::Passed,
                &Default::default(),
                &Default::default(),
                OutcomeStatus::NotChecked,
            ),
            overall: OutcomeStatus::Passed,
        });
        let summarize = |p, r| summarize_process("run".into(), p, Ok(r), &scenario);
        assert_eq!(
            summarize(process.clone(), player.clone()).overall,
            OutcomeStatus::Passed
        );
        let mut unconfirmed = process.clone();
        unconfirmed.ownership.process_group_release_completed = false;
        assert_eq!(
            summarize(unconfirmed, player.clone()).overall,
            OutcomeStatus::Failed
        );
        let mut stale = player.clone();
        stale.run_id = "other-run".into();
        assert_eq!(
            summarize(process.clone(), stale).overall,
            OutcomeStatus::Failed
        );
        let mut stale = player.clone();
        stale.semantic_playtest.as_mut().unwrap().scenario_id = "other-scenario".into();
        assert_eq!(
            summarize(process.clone(), stale).overall,
            OutcomeStatus::Failed
        );
        let mut failed = process;
        failed.exit_reason = BoundedChildProcessExitReason::Failed;
        failed.exit_code = Some(23);
        player.exit_code = 23;
        let report = summarize(failed, player.clone());
        assert_eq!(report.overall, OutcomeStatus::Failed);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d == "playtest.nonzero_exit_with_passed_outcome"));
        let root = std::env::temp_dir().join(format!(
            "semantic-capture-evidence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("capture-1.png");
        fs::write(
            &path,
            b"fixture bytes: identity validation, not a visual qualification",
        )
        .unwrap();
        let mut captured_scenario = scenario.clone();
        captured_scenario
            .captures
            .push(runtime_player_winit::semantic_outcome::PlaytestCapture {
                capture_id: "view".into(),
                presentation_frame: 1,
                required: true,
                subjective_review: false,
            });
        let semantic = player.semantic_playtest.as_mut().unwrap();
        semantic
            .captures
            .insert("view".into(), OutcomeStatus::Passed);
        semantic
            .capture_evidence
            .push(runtime_player_winit::PlaytestCaptureEvidence {
                capture_id: "view".into(),
                session_id: "session".into(),
                simulation_tick: 1,
                presentation_frame: 1,
                path: path.display().to_string(),
                sha256: file_digest(&path).unwrap(),
                width: 1,
                height: 1,
                byte_size: fs::metadata(&path).unwrap().len(),
            });
        let mut report = report;
        report.player = Some(player);
        validate_capture_evidence(&report, &captured_scenario, &root).unwrap();
        for invalid in ["session", "frame", "size", "missing-entry"] {
            let mut changed = report.clone();
            let semantic = changed
                .player
                .as_mut()
                .unwrap()
                .semantic_playtest
                .as_mut()
                .unwrap();
            match invalid {
                "session" => semantic.capture_evidence[0].session_id = "wrong".into(),
                "frame" => semantic.capture_evidence[0].presentation_frame = 2,
                "size" => semantic.capture_evidence[0].width = u32::MAX,
                _ => semantic.capture_evidence.clear(),
            }
            assert!(validate_capture_evidence(&changed, &captured_scenario, &root).is_err());
        }
        fs::write(&path, b"replaced capture").unwrap();
        assert!(
            validate_capture_evidence(&report, &captured_scenario, &root)
                .unwrap_err()
                .contains("digest_mismatch")
        );
        let large = fs::OpenOptions::new().write(true).open(&path).unwrap();
        large.set_len(32 * 1024 * 1024 + 1).unwrap();
        drop(large);
        assert!(
            validate_capture_evidence(&report, &captured_scenario, &root)
                .unwrap_err()
                .contains("too_large")
        );
        fs::remove_file(&path).unwrap();
        assert!(
            validate_capture_evidence(&report, &captured_scenario, &root)
                .unwrap_err()
                .contains("capture_missing")
        );
        fs::remove_dir(root).unwrap();
    }
}
