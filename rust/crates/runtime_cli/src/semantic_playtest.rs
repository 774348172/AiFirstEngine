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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    unpaced: bool,
}

/// Private process receipt, not a Tool or RuntimePackage schema. The job and
/// engine run have different owners; neither identity may be rewritten.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkerResultBinding {
    job_run_id: String,
    runtime_run_id: String,
    request_digest: String,
    report_digest: String,
    worker_process_id: u32,
}

struct VerifiedPlayerReport {
    job_run_id: String,
    player: NativeWindowHostReport,
}

pub fn run_bounded_semantic_playtest(
    request: SemanticPlaytestProcessRequest,
) -> Result<SemanticPlaytestProcessReport, String> {
    run_bounded_semantic_playtest_with_pacing(request, false)
}

/// Explicit functional-trajectory execution using the existing Player's
/// Immediate/uncapped presentation. Fixed simulation ticks and all timeout,
/// ownership, artifact and capture checks remain identical. Not wall-time qualification.
pub fn run_bounded_semantic_playtest_unpaced(
    request: SemanticPlaytestProcessRequest,
) -> Result<SemanticPlaytestProcessReport, String> {
    run_bounded_semantic_playtest_with_pacing(request, true)
}

fn run_bounded_semantic_playtest_with_pacing(
    request: SemanticPlaytestProcessRequest,
    unpaced: bool,
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
        unpaced,
    };
    let input_path = output.join("request.json");
    let player_report_path = output.join("player.json");
    let request_bytes = serde_json::to_vec(&worker_input).map_err(|error| error.to_string())?;
    fs::write(&input_path, &request_bytes).map_err(|error| error.to_string())?;
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
    let player = read_bound_player_report(
        &player_report_path,
        &run_id,
        &request_bytes,
        process.process_id,
    );
    let mut report = summarize_process(run_id, process, player, &request.scenario);
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
    player: Result<VerifiedPlayerReport, String>,
    scenario: &PlaytestScenario,
) -> SemanticPlaytestProcessReport {
    let mut diagnostics = Vec::new();
    let mut outcome = SemanticOutcome {
        technical: OutcomeStatus::Failed,
        gameplay: OutcomeStatus::NotProducedYet,
        visual: OutcomeStatus::NotChecked,
        delivery: OutcomeStatus::NotChecked,
    };
    let mut job_matches = false;
    let player = match player {
        Ok(verified) => {
            job_matches = verified.job_run_id == run_id;
            Some(verified.player)
        }
        Err(error) => {
            diagnostics.push(format!("playtest.report_missing_or_invalid: {error}"));
            None
        }
    };
    if let Some(player) = &player {
        if job_matches && process.exit_code == Some(player.exit_code) {
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
        let spatial_required = scenario
            .assertions
            .iter()
            .any(|a| a.path.starts_with("engine.spatial/"))
            || scenario
                .spatial_observation
                .as_ref()
                .is_some_and(|r| !r.entities.is_empty());
        if spatial_required
            && capture.spatial.as_ref().is_none_or(|s| {
                s.schema_version != "spatial-observation.v1"
                    || !s.matches_projection
                    || s.frame_stage != engine_runtime::render_geometry::SpatialFrameStage::Captured
                    || s.committed_frame != capture.presentation_frame
                    || s.view.is_none()
            })
        {
            return Err("playtest.capture_spatial_frame_mismatch".into());
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

fn write_bound_player_report(
    path: &Path,
    job_run_id: &str,
    request_bytes: &[u8],
    report: &NativeWindowHostReport,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(report).map_err(|error| error.to_string())?;
    let binding = WorkerResultBinding {
        job_run_id: job_run_id.into(),
        runtime_run_id: report.run_id.clone(),
        request_digest: engine_runtime::canonical_digest::sha256_prefixed(request_bytes),
        report_digest: engine_runtime::canonical_digest::sha256_prefixed(&bytes),
        worker_process_id: std::process::id(),
    };
    fs::write(path, bytes).map_err(|error| error.to_string())?;
    // Publish the receipt last. A crash or timeout with only a report is not a
    // verified result, even when an older worker exits successfully.
    fs::write(
        path.with_extension("binding.json"),
        serde_json::to_vec(&binding).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn verify_player_binding(
    report_bytes: &[u8],
    binding_bytes: &[u8],
    job_run_id: &str,
    request_bytes: &[u8],
    owned_process_id: Option<u32>,
) -> Result<VerifiedPlayerReport, String> {
    let binding: WorkerResultBinding = serde_json::from_slice(binding_bytes)
        .map_err(|error| format!("playtest.worker_binding_invalid: {error}"))?;
    if binding.job_run_id != job_run_id
        || binding.request_digest
            != engine_runtime::canonical_digest::sha256_prefixed(request_bytes)
        || binding.report_digest != engine_runtime::canonical_digest::sha256_prefixed(report_bytes)
        || Some(binding.worker_process_id) != owned_process_id
        || binding.worker_process_id == 0
        || binding.runtime_run_id.is_empty()
    {
        return Err("playtest.worker_binding_mismatch".into());
    }
    let player: NativeWindowHostReport =
        serde_json::from_slice(report_bytes).map_err(|error| error.to_string())?;
    if binding.runtime_run_id != player.run_id {
        return Err("playtest.worker_runtime_identity_mismatch".into());
    }
    Ok(VerifiedPlayerReport {
        job_run_id: binding.job_run_id,
        player,
    })
}

fn read_bound_player_report(
    path: &Path,
    job_run_id: &str,
    request_bytes: &[u8],
    owned_process_id: Option<u32>,
) -> Result<VerifiedPlayerReport, String> {
    let binding = read_limited(&path.with_extension("binding.json"), 4096)
        .map_err(|error| format!("playtest.worker_binding_missing: {error}"))?;
    verify_player_binding(
        &read_limited(path, MAX_PLAYER_REPORT_BYTES)?,
        &binding,
        job_run_id,
        request_bytes,
        owned_process_id,
    )
}

fn semantic_player_request(
    runtime_package: &Path,
    target: PlaytestTarget,
    game_view_target: Option<engine_runtime::game_view_presentation::GameViewTargetSpec>,
) -> NativePlayerWindowRunRequest {
    let mut request = match target {
        PlaytestTarget::WindowsHeadless => {
            NativePlayerWindowRunRequest::headless_surface_gate(runtime_package)
        }
        PlaytestTarget::WindowsWindowed => NativePlayerWindowRunRequest::windowed(runtime_package),
    };
    if let Some(target) = game_view_target {
        request = request.with_game_view_target(target);
    }
    request
}

fn apply_semantic_pacing(request: &mut NativePlayerWindowRunRequest, unpaced: bool) {
    if unpaced {
        request.config.target_fps = 0;
        request.config.present_mode = "Immediate".into();
    }
}

pub(crate) fn run_worker(
    args: &[String],
    linked: &Arc<LinkedProjectRuntimeSet>,
) -> Result<i32, String> {
    if args.len() != 3 {
        return Err("playtest.worker_arguments_invalid".into());
    }
    let request_bytes = read_limited(Path::new(&args[1]), MAX_PLAYTEST_SCENARIO_BYTES + 32 * 1024)?;
    let input: WorkerInput =
        serde_json::from_slice(&request_bytes).map_err(|error| error.to_string())?;
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("playtest.executable_parent_missing")?
        .to_path_buf();
    let game_view_target =
        engine_runtime::runtime_package::load_runtime_package(&input.runtime_package)
            .value
            .and_then(|runtime| runtime.manifest.project.game_view_target);
    let mut request = semantic_player_request(
        &input.runtime_package,
        input.scenario.target,
        game_view_target,
    );
    apply_semantic_pacing(&mut request, input.unpaced);
    request.frame_limit = input.scenario.max_presentation_frames;
    let report = if crate::requires_engine_dll(&exe_dir, &input.runtime_package) {
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
            PlaytestTarget::WindowsHeadless => {
                run_headless_semantic_playtest_with_linked_modules(request, linked, input.scenario)
            }
            PlaytestTarget::WindowsWindowed => {
                runtime_player_winit::run_windowed_semantic_playtest_with_linked_modules(
                    request,
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
    write_bound_player_report(Path::new(&args[2]), &input.run_id, &request_bytes, &report)?;
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
    fn worker_binding_keeps_runtime_identity_and_rejects_swapped_partial_or_stale_results() {
        let mut player = NativeWindowHostReport::base(
            &NativePlayerWindowRunRequest::headless_surface_gate("not-loaded"),
        );
        player.run_id = "original-runtime".into();
        let bytes = serde_json::to_vec(&player).unwrap();
        let input = b"exact owned request";
        let binding = serde_json::json!({
            "jobRunId":"job", "runtimeRunId":"original-runtime",
            "requestDigest":engine_runtime::canonical_digest::sha256_prefixed(input),
            "reportDigest":engine_runtime::canonical_digest::sha256_prefixed(&bytes),
            "workerProcessId":42
        });
        let encoded = serde_json::to_vec(&binding).unwrap();
        let verified = verify_player_binding(&bytes, &encoded, "job", input, Some(42)).unwrap();
        assert_eq!(verified.job_run_id, "job");
        assert_eq!(verified.player.run_id, "original-runtime");
        assert_ne!(verified.job_run_id, verified.player.run_id);
        for field in [
            "jobRunId",
            "runtimeRunId",
            "requestDigest",
            "reportDigest",
            "workerProcessId",
        ] {
            let mut stale = binding.clone();
            stale[field] = if field == "workerProcessId" {
                serde_json::json!(43)
            } else {
                serde_json::json!("stale")
            };
            assert!(
                verify_player_binding(
                    &bytes,
                    &serde_json::to_vec(&stale).unwrap(),
                    "job",
                    input,
                    Some(42)
                )
                .is_err(),
                "must reject {field}"
            );
        }
        let mut changed = bytes.clone();
        changed.push(b' '); // Still valid JSON; binding must seal the exact bytes.
        assert!(verify_player_binding(&changed, &encoded, "job", input, Some(42)).is_err());
        assert!(
            verify_player_binding(&bytes, &encoded, "job", b"other request", Some(42)).is_err()
        );
        assert!(verify_player_binding(&bytes, &encoded, "job", input, None).is_err());
        assert!(verify_player_binding(&bytes, b"", "job", input, Some(42)).is_err());
        let mut extended = binding;
        extended["fallback"] = serde_json::json!(true);
        assert!(verify_player_binding(
            &bytes,
            &serde_json::to_vec(&extended).unwrap(),
            "job",
            input,
            Some(42)
        )
        .is_err());
    }

    /// Local composition diagnostic: consume an existing delivery through the
    /// production DLL loader, with the original scenario and bounded process owner.
    #[cfg(windows)]
    #[test]
    #[ignore = "requires an existing project-linked delivery and real Windows GPU"]
    fn existing_delivery_unpaced_semantic_cycle() {
        let delivery = PathBuf::from(std::env::var_os("S2D_K1_DELIVERY").expect("delivery"));
        let output = PathBuf::from(std::env::var_os("S2D_K1_OUTPUT").expect("fresh output"));
        let scenario_path = PathBuf::from(std::env::var_os("S2D_K1_SCENARIO").expect("scenario"));
        let request_bytes = fs::read(&scenario_path).unwrap();
        let scenario: PlaytestScenario = serde_json::from_slice(&request_bytes).unwrap();
        let package = delivery.join("data/runtime_package");
        let player_path = output.join("player.json");
        let run_id = "371-k1-existing-delivery-unpaced".to_string();
        if std::env::var("S2D_K1_CHILD").as_deref() == Ok("1") {
            let target = engine_runtime::runtime_package::load_runtime_package(&package)
                .value
                .unwrap()
                .manifest
                .project
                .game_view_target;
            let mut request = semantic_player_request(&package, scenario.target, target);
            request.frame_limit = scenario.max_presentation_frames;
            // Async streaming diagnostics can retain normal presentation pacing
            // while using the same bounded, filtered Trace consumer.
            apply_semantic_pacing(
                &mut request,
                std::env::var("S2D_K1_PACED").as_deref() != Ok("1"),
            );
            // Explicit diagnostics reuse the real delivery and semantic clock.
            // One or two requested GPU samples keep Trace bounded and include
            // the final project observation without full-frame history.
            if let Ok(frame) = std::env::var("S2D_K1_TRACE_FRAME") {
                let frame: u64 = frame.parse().expect("trace frame");
                let count: u64 = std::env::var("S2D_K1_TRACE_COUNT")
                    .map(|v| v.parse().expect("trace count"))
                    .unwrap_or(1);
                assert!((1..=2).contains(&count));
                assert!((1..=request.frame_limit).contains(&frame));
                assert!(frame <= request.frame_limit - (count - 1));
                request = request
                    .with_frame_performance_sample(frame - 1, count)
                    .with_measurement_policy(
                        runtime_player_winit::measurement::MeasurementPolicy {
                            level: runtime_player_winit::measurement::MeasurementLevel::Trace,
                            first_frame: Some(frame),
                            end_frame: Some(frame + count),
                            ..Default::default()
                        },
                    );
            } else if std::env::var("S2D_K1_MEASURE").as_deref() == Ok("1") {
                // Keep historical tiny diagnostics explicit; 720 frames uses
                // the 371-A quick-feedback window, never formal qualification.
                let warmup = if scenario.max_presentation_frames == 720 {
                    120
                } else {
                    4
                };
                assert!((warmup + 2..=warmup + 600).contains(&scenario.max_presentation_frames));
                request = request
                    .with_frame_performance_sample(
                        warmup,
                        scenario.max_presentation_frames - warmup,
                    )
                    .with_measurement_policy(
                        runtime_player_winit::measurement::MeasurementPolicy {
                            level: runtime_player_winit::measurement::MeasurementLevel::Summary,
                            ..Default::default()
                        },
                    );
            }
            let execution =
                runtime_player_winit::engine_dll_execution::EngineRuntimeExecutionRequest {
                    request,
                    scenario: Some(scenario),
                    report_path: output.join("engine-player.json"),
                    capture_directory: Some(output.clone()),
                };
            let report = if let Some(dll) = std::env::var_os("S2D_K1_DIAGNOSTIC_DLL") {
                // Explicit source-diagnostic consumer: keep the original delivery
                // authenticated; never rewrite its manifest or replace its DLL.
                crate::validate_desktop_dev_package(&delivery).unwrap();
                crate::engine_dll_loader::execute(Path::new(&dll), &execution)
            } else {
                crate::execute_packaged_request(&delivery, execution)
            }
            .expect("existing production DLL composition");
            write_bound_player_report(&player_path, &run_id, &request_bytes, &report).unwrap();
            return;
        }
        assert!(scenario.timeout_ms <= MAX_PLAYTEST_TIMEOUT_MS);
        fs::create_dir(&output).expect("fresh output");
        let before_package = runtime_package_digest(&package).unwrap();
        let before_dll = file_digest(&delivery.join("engine_runtime.dll")).unwrap();
        let before_player = file_digest(&delivery.join("Game.exe")).unwrap();
        let diagnostic_dll = std::env::var_os("S2D_K1_DIAGNOSTIC_DLL").map(PathBuf::from);
        let diagnostic_hash = diagnostic_dll.as_deref().map(|p| file_digest(p).unwrap());
        fs::write(
            output.join("consumer-identity.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "diagnosticEngineDll": diagnostic_dll,
                "diagnosticEngineSha256": diagnostic_hash,
                "deliveryEngineSha256": before_dll,
                "normalPacing": std::env::var("S2D_K1_PACED").as_deref() == Ok("1"),
                "scope": "explicit diagnostic override is not original delivery qualification",
            }))
            .unwrap(),
        )
        .unwrap();
        let process = run_bounded_child_process(BoundedChildProcessRequest {
            executable: std::env::current_exe().unwrap(),
            args: vec![
                "semantic_playtest::tests::existing_delivery_unpaced_semantic_cycle".into(),
                "--exact".into(),
                "--ignored".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            current_dir: output.clone(),
            environment: vec![("S2D_K1_CHILD".into(), "1".into())],
            timeout: Duration::from_millis(scenario.timeout_ms),
            stdout_capture_limit_bytes: 16 * 1024,
            stderr_capture_limit_bytes: 16 * 1024,
            priority: BoundedChildProcessPriority::Normal,
        });
        let player =
            read_bound_player_report(&player_path, &run_id, &request_bytes, process.process_id);
        let mut report = summarize_process(run_id, process, player, &scenario);
        report.scenario_digest = engine_runtime::canonical_digest::sha256_prefixed(
            &serde_json::to_vec(&scenario).unwrap(),
        );
        report.package_digest = before_package.clone();
        report.player_digest = before_player.clone();
        if let Err(error) = validate_capture_evidence(&report, &scenario, &output) {
            report.outcome.visual = OutcomeStatus::Failed;
            report.diagnostics.push(error);
        }
        report.overall = report.outcome.overall(&scenario, false);
        fs::write(
            output.join("result.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(runtime_package_digest(&package).unwrap(), before_package);
        assert_eq!(
            file_digest(&delivery.join("engine_runtime.dll")).unwrap(),
            before_dll
        );
        assert_eq!(
            file_digest(&delivery.join("Game.exe")).unwrap(),
            before_player
        );
        assert_eq!(report.overall, OutcomeStatus::Passed, "{report:?}");
        assert_eq!(
            diagnostic_dll.as_deref().map(|p| file_digest(p).unwrap()),
            diagnostic_hash
        );
        let semantic = report
            .player
            .as_ref()
            .unwrap()
            .semantic_playtest
            .as_ref()
            .unwrap();
        assert_eq!(semantic.simulation_ticks, scenario.max_simulation_ticks);
        assert_eq!(report.process.exit_code, Some(0));
        assert!(report.process.owned_process_cleanup_confirmed());
    }

    #[cfg(windows)]
    #[test]
    fn semantic_bounded_execution_checks_four_hour_limit_before_io() {
        let scenario: PlaytestScenario = serde_json::from_value(serde_json::json!({
            "schemaVersion":"playtest-scenario.v2", "scenarioId":"bounded-world-k2",
            "initialSceneId":"main", "target":"windows-windowed",
            "maxSimulationTicks":48000, "maxPresentationFrames":50000, "timeoutMs":14400000
        }))
        .unwrap();
        scenario.validate(None).unwrap();
        let missing = std::env::temp_dir().join(format!(
            "semantic-budget-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let expected_io_error = missing.canonicalize().unwrap_err().to_string();
        let request = SemanticPlaytestProcessRequest {
            player_executable: missing.clone(),
            runtime_package: missing.join("package"),
            scenario,
            output_dir: missing.join("output"),
        };
        assert_eq!(
            run_bounded_semantic_playtest(request.clone()).unwrap_err(),
            expected_io_error,
            "valid finite budget must reach the existing executable boundary"
        );
        for invalid in [0, MAX_PLAYTEST_TIMEOUT_MS + 1] {
            let mut rejected = request.clone();
            rejected.scenario.timeout_ms = invalid;
            assert_eq!(
                run_bounded_semantic_playtest(rejected).unwrap_err(),
                "playtest.timeout_invalid"
            );
        }
        assert!(!missing.exists());
    }

    #[test]
    fn semantic_player_request_applies_project_game_view_target_for_all_execution_paths() {
        let target =
            engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_720x1280();
        for mode in [
            PlaytestTarget::WindowsHeadless,
            PlaytestTarget::WindowsWindowed,
        ] {
            let request = semantic_player_request(Path::new("runtime-package"), mode, Some(target));
            assert_eq!(request.game_view_target, target);
            assert_eq!(request.config.width, 720);
            assert_eq!(request.config.height, 1280);
            assert_eq!(request.config.target_fps, 60);
            assert_eq!(request.config.present_mode, "Fifo");
            let mut unpaced = request.clone();
            apply_semantic_pacing(&mut unpaced, true);
            assert_eq!(unpaced.config.target_fps, 0);
            assert_eq!(unpaced.config.present_mode, "Immediate");
            assert_eq!(unpaced.game_view_target, target);
        }
    }

    #[test]
    fn semantic_unpaced_worker_transport_is_explicit_and_defaults_to_original_pacing() {
        let legacy = serde_json::json!({"runId":"pacing","runtimePackage":"package","scenario":{
            "schemaVersion":"playtest-scenario.v2","scenarioId":"pacing","initialSceneId":"main","target":"windows-windowed",
            "maxSimulationTicks":7201,"maxPresentationFrames":7201,"timeoutMs":120000
        }});
        let decoded: WorkerInput = serde_json::from_value(legacy.clone()).unwrap();
        assert!(!decoded.unpaced);
        assert!(serde_json::to_value(&decoded)
            .unwrap()
            .get("unpaced")
            .is_none());
        let mut explicit = legacy.clone();
        explicit["unpaced"] = serde_json::json!(true);
        let decoded: WorkerInput = serde_json::from_value(explicit).unwrap();
        assert!(decoded.unpaced);
        assert_eq!(decoded.scenario.timeout_ms, 120_000);
        let mut unknown = legacy;
        unknown["pacingFallback"] = serde_json::json!(true);
        assert!(serde_json::from_value::<WorkerInput>(unknown).is_err());
    }

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
            presented_frame: None,
            submitted_frame: None,
            fixed_delta_seconds: 1.0 / 60.0,
            injected_transition_count: 0,
            aui: None,
            spatial: None,
            aui_events: Vec::new(),
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
        let summarize = |p, r| {
            summarize_process(
                "run".into(),
                p,
                Ok(VerifiedPlayerReport {
                    job_run_id: "run".into(),
                    player: r,
                }),
                &scenario,
            )
        };
        assert_eq!(
            summarize(process.clone(), player.clone()).overall,
            OutcomeStatus::Passed
        );
        let mut original_runtime = player.clone();
        original_runtime.run_id = "original-engine-runtime".into();
        assert_eq!(
            summarize(process.clone(), original_runtime).overall,
            OutcomeStatus::Passed,
            "job identity must not overwrite the original runtime identity"
        );
        let mut unconfirmed = process.clone();
        unconfirmed.ownership.process_group_release_completed = false;
        assert_eq!(
            summarize(unconfirmed, player.clone()).overall,
            OutcomeStatus::Failed
        );
        assert_eq!(
            summarize_process(
                "run".into(),
                process.clone(),
                Ok(VerifiedPlayerReport {
                    job_run_id: "stale-job".into(),
                    player: player.clone()
                }),
                &scenario,
            )
            .overall,
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
                spatial: None,
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
