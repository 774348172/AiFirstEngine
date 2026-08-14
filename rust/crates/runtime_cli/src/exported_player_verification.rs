use engine_runtime::release_package_manifest::{
    validate_release_package_manifest, ReleasePackageManifest,
    RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_path::safe_join_runtime_package;
use engine_runtime::windowed_player::WindowedPlayerRunReport;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const EXPORTED_PLAYER_PROCESS_VERIFICATION_REPORT_SCHEMA_VERSION: &str =
    "exported-player-process-verification-report.v2";
static RELEASE_VERIFICATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedPlayerProcessVerificationRequest {
    pub exported_package_dir: PathBuf,
    pub mode: String,
    pub frame_limit: u64,
    pub report_path: Option<PathBuf>,
    pub timeout_ms: u64,
    pub screenshot: bool,
    pub screenshot_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExportedPlayerProcessVerificationOptions {
    pub input_script_path: Option<PathBuf>,
    pub runtime_report_level: Option<String>,
    pub performance_warmup_frames: u64,
    pub performance_sample_frames: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportedPlayerProcessVerificationStatus {
    Passed,
    Failed,
    EnvironmentBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedPlayerProcessVerificationDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl ExportedPlayerProcessVerificationDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "warning".to_string(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedPlayerProcessVerificationReport {
    pub schema_version: String,
    pub status: ExportedPlayerProcessVerificationStatus,
    pub exported_package_dir: String,
    pub game_exe_path: String,
    pub package_manifest_path: String,
    pub runtime_package_path: String,
    pub child_report_path: String,
    pub package_kind: String,
    pub entrypoint_relative_path: String,
    pub mode: String,
    pub frame_limit: u64,
    pub process_exit_code: Option<i32>,
    pub process_exit_reason: String,
    pub process_id: Option<u32>,
    pub process_elapsed_ms: u128,
    pub child_player_exit_code: Option<i32>,
    pub child_present_status: Option<String>,
    pub child_frames_completed: Option<u64>,
    pub screenshot_requested: bool,
    pub screenshot_status: Option<String>,
    pub screenshot_path: Option<String>,
    pub stdout_summary: String,
    pub stderr_summary: String,
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub process_spawn_error: Option<String>,
    pub process_kill_error: Option<String>,
    pub process_wait_error: Option<String>,
    pub process_reader_join_error: Option<String>,
    pub diagnostics: Vec<ExportedPlayerProcessVerificationDiagnostic>,
    pub exit_code: Option<i32>,
}

impl ExportedPlayerProcessVerificationReport {
    fn failed(request: &ResolvedVerificationPaths, mode: &str, frame_limit: u64) -> Self {
        Self {
            schema_version: EXPORTED_PLAYER_PROCESS_VERIFICATION_REPORT_SCHEMA_VERSION.to_string(),
            status: ExportedPlayerProcessVerificationStatus::Failed,
            exported_package_dir: request.exported_package_dir.display().to_string(),
            game_exe_path: request.game_exe_path.display().to_string(),
            package_manifest_path: request.package_manifest_path.display().to_string(),
            runtime_package_path: request.runtime_package_path.display().to_string(),
            child_report_path: request.child_report_path.display().to_string(),
            package_kind: request.package_kind.clone(),
            entrypoint_relative_path: request.entrypoint_relative_path.clone(),
            mode: mode.to_string(),
            frame_limit,
            process_exit_code: None,
            process_exit_reason: "not_started".to_string(),
            process_id: None,
            process_elapsed_ms: 0,
            child_player_exit_code: None,
            child_present_status: None,
            child_frames_completed: None,
            screenshot_requested: false,
            screenshot_status: None,
            screenshot_path: None,
            stdout_summary: String::new(),
            stderr_summary: String::new(),
            stdout_total_bytes: 0,
            stderr_total_bytes: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            process_spawn_error: None,
            process_kill_error: None,
            process_wait_error: None,
            process_reader_join_error: None,
            diagnostics: Vec::new(),
            exit_code: Some(1),
        }
    }

    fn recompute_status(&mut self) {
        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
        {
            self.status = ExportedPlayerProcessVerificationStatus::Failed;
            self.exit_code = Some(1);
            return;
        }
        if self.process_exit_code == Some(0) && self.child_player_exit_code == Some(0) {
            self.status = ExportedPlayerProcessVerificationStatus::Passed;
            self.exit_code = Some(0);
        } else {
            self.status = ExportedPlayerProcessVerificationStatus::Failed;
            self.exit_code = Some(1);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedVerificationPaths {
    exported_package_dir: PathBuf,
    game_exe_path: PathBuf,
    package_manifest_path: PathBuf,
    runtime_package_path: PathBuf,
    child_report_path: PathBuf,
    child_screenshot_path: PathBuf,
    parent_report_path: PathBuf,
    package_kind: String,
    entrypoint_relative_path: String,
    resolution_diagnostics: Vec<ExportedPlayerProcessVerificationDiagnostic>,
}

pub fn verify_exported_player_process(
    request: ExportedPlayerProcessVerificationRequest,
) -> ExportedPlayerProcessVerificationReport {
    verify_exported_player_process_with_options(
        request,
        ExportedPlayerProcessVerificationOptions::default(),
    )
}

pub fn verify_exported_player_process_with_options(
    request: ExportedPlayerProcessVerificationRequest,
    options: ExportedPlayerProcessVerificationOptions,
) -> ExportedPlayerProcessVerificationReport {
    let paths = resolve_paths(&request);
    let mut report = ExportedPlayerProcessVerificationReport::failed(
        &paths,
        &request.mode,
        request.frame_limit.max(1),
    );
    validate_exported_package(&paths, &mut report);
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
    {
        finalize_report(&paths.parent_report_path, report)
    } else {
        run_child_process(&request, &options, &paths, &mut report);
        report.recompute_status();
        finalize_report(&paths.parent_report_path, report)
    }
}

fn resolve_paths(request: &ExportedPlayerProcessVerificationRequest) -> ResolvedVerificationPaths {
    let exported_package_dir = if request.exported_package_dir.is_absolute() {
        request.exported_package_dir.clone()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&request.exported_package_dir)
    };
    let package_manifest_path = exported_package_dir.join("package-manifest.json");
    let (
        package_kind,
        entrypoint_relative_path,
        game_exe_path,
        runtime_package_path,
        resolution_diagnostics,
    ) = resolve_package_contract(&exported_package_dir, &package_manifest_path);
    let default_report_root = if package_kind == "release" {
        let sequence = RELEASE_VERIFICATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join("aife-release-verification")
            .join(format!("{}-{sequence}", std::process::id()))
    } else {
        exported_package_dir.join("reports")
    };
    let report_root = request
        .report_path
        .as_ref()
        .and_then(|path| path.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_report_root.clone());
    let child_report_path = report_root.join("windowed-player-run-report.json");
    let child_screenshot_path = request
        .screenshot_path
        .clone()
        .unwrap_or_else(|| report_root.join("windowed-player-screenshot.png"));
    let parent_report_path = request.report_path.clone().unwrap_or_else(|| {
        default_report_root.join("exported-player-process-verification-report.json")
    });
    ResolvedVerificationPaths {
        exported_package_dir,
        game_exe_path,
        package_manifest_path,
        runtime_package_path,
        child_report_path,
        child_screenshot_path,
        parent_report_path,
        package_kind,
        entrypoint_relative_path,
        resolution_diagnostics,
    }
}

fn resolve_package_contract(
    exported_package_dir: &Path,
    package_manifest_path: &Path,
) -> (
    String,
    String,
    PathBuf,
    PathBuf,
    Vec<ExportedPlayerProcessVerificationDiagnostic>,
) {
    let legacy_entrypoint = if cfg!(windows) { "Game.exe" } else { "Game" };
    let legacy = || {
        (
            "legacy-dev".to_string(),
            legacy_entrypoint.to_string(),
            exported_package_dir.join(legacy_entrypoint),
            exported_package_dir.join("data").join("runtime_package"),
            Vec::new(),
        )
    };
    let Ok(text) = fs::read_to_string(package_manifest_path) else {
        return legacy();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return legacy();
    };
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        != Some(RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION)
    {
        return legacy();
    }
    let manifest = match serde_json::from_value::<ReleasePackageManifest>(value) {
        Ok(manifest) => manifest,
        Err(error) => {
            return (
                "release".to_string(),
                String::new(),
                exported_package_dir.join("invalid-release-entrypoint"),
                exported_package_dir.join("invalid-runtime-package"),
                vec![ExportedPlayerProcessVerificationDiagnostic::error(
                    "release_manifest_invalid",
                    format!("failed to parse release manifest: {error}"),
                )
                .with_path(package_manifest_path.display().to_string())],
            );
        }
    };
    let mut diagnostics = validate_release_package_manifest(&manifest)
        .into_iter()
        .map(|diagnostic| {
            ExportedPlayerProcessVerificationDiagnostic::error(
                diagnostic.code,
                format!("{}: {}", diagnostic.path, diagnostic.message),
            )
            .with_path(package_manifest_path.display().to_string())
        })
        .collect::<Vec<_>>();
    let entrypoint = safe_join_runtime_package(exported_package_dir, &manifest.entrypoint)
        .map_err(|error| error.to_string());
    let runtime_package =
        safe_join_runtime_package(exported_package_dir, &manifest.runtime_package)
            .map_err(|error| error.to_string());
    if let Err(message) = &entrypoint {
        diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error("release_path_escape", message)
                .with_path(manifest.entrypoint.clone()),
        );
    }
    if let Err(message) = &runtime_package {
        diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error("release_path_escape", message)
                .with_path(manifest.runtime_package.clone()),
        );
    }
    (
        "release".to_string(),
        manifest.entrypoint,
        entrypoint.unwrap_or_else(|_| exported_package_dir.join("invalid-release-entrypoint")),
        runtime_package.unwrap_or_else(|_| exported_package_dir.join("invalid-runtime-package")),
        diagnostics,
    )
}

fn validate_exported_package(
    paths: &ResolvedVerificationPaths,
    report: &mut ExportedPlayerProcessVerificationReport,
) {
    report
        .diagnostics
        .extend(paths.resolution_diagnostics.iter().cloned());
    if !paths.exported_package_dir.exists() {
        report.diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error(
                "exported_package_dir_missing",
                "exported package directory does not exist",
            )
            .with_path(paths.exported_package_dir.display().to_string()),
        );
    }
    if !paths.game_exe_path.exists() {
        report.diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error(
                if paths.package_kind == "release" {
                    "release_entrypoint_missing"
                } else {
                    "game_exe_missing"
                },
                "entrypoint executable is missing from exported package",
            )
            .with_path(paths.game_exe_path.display().to_string()),
        );
    }
    if !paths.package_manifest_path.exists() {
        report.diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error(
                "package_manifest_missing",
                "package-manifest.json is missing from exported package",
            )
            .with_path(paths.package_manifest_path.display().to_string()),
        );
    }
    if !paths.runtime_package_path.join("manifest.json").exists() {
        report.diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error(
                "runtime_package_missing",
                "data/runtime_package/manifest.json is missing from exported package",
            )
            .with_path(paths.runtime_package_path.display().to_string()),
        );
    }
}

fn run_child_process(
    request: &ExportedPlayerProcessVerificationRequest,
    options: &ExportedPlayerProcessVerificationOptions,
    paths: &ResolvedVerificationPaths,
    report: &mut ExportedPlayerProcessVerificationReport,
) {
    let _ = fs::remove_file(&paths.child_report_path);
    let mut args = vec![
        "run-native-player".into(),
        "--package".into(),
        paths.runtime_package_path.as_os_str().to_owned(),
        "--mode".into(),
        request.mode.clone().into(),
        "--frames".into(),
        request.frame_limit.max(1).to_string().into(),
        "--report".into(),
        paths.child_report_path.as_os_str().to_owned(),
    ];
    if let Some(path) = &options.input_script_path {
        args.push("--input-script".into());
        args.push(path.as_os_str().to_owned());
    }
    if let Some(level) = &options.runtime_report_level {
        args.push("--runtime-report-level".into());
        args.push(level.into());
    }
    if options.performance_warmup_frames > 0 || options.performance_sample_frames > 0 {
        args.push("--performance-warmup-frames".into());
        args.push(options.performance_warmup_frames.to_string().into());
        args.push("--performance-sample-frames".into());
        args.push(options.performance_sample_frames.to_string().into());
    }
    args.extend(screenshot_args(request, paths).into_iter().map(Into::into));
    let process = run_bounded_child_process(BoundedChildProcessRequest {
        executable: paths.game_exe_path.clone(),
        args,
        current_dir: paths.exported_package_dir.clone(),
        environment: Vec::new(),
        timeout: Duration::from_millis(request.timeout_ms.max(1)),
        stdout_capture_limit_bytes: 64 * 1024,
        stderr_capture_limit_bytes: 64 * 1024,
        priority: crate::BoundedChildProcessPriority::Normal,
    });
    apply_process_result(report, paths, process);

    read_child_report(paths, report);
}

fn apply_process_result(
    report: &mut ExportedPlayerProcessVerificationReport,
    paths: &ResolvedVerificationPaths,
    process: BoundedChildProcessResult,
) {
    report.process_id = process.process_id;
    report.process_exit_code = process.exit_code;
    report.process_elapsed_ms = process.elapsed_ms;
    report.process_exit_reason = match process.exit_reason {
        BoundedChildProcessExitReason::Completed => "completed",
        BoundedChildProcessExitReason::Failed => "failed",
        BoundedChildProcessExitReason::Cancelled => "cancelled",
        BoundedChildProcessExitReason::Timeout => "timeout",
        BoundedChildProcessExitReason::WaitFailed => "wait_failed",
        BoundedChildProcessExitReason::SpawnFailed => "spawn_failed",
    }
    .to_string();
    report.stdout_summary = summarize(&process.stdout_summary, 2_000);
    report.stderr_summary = summarize(&process.stderr_summary, 2_000);
    report.stdout_total_bytes = process.stdout_total_bytes;
    report.stderr_total_bytes = process.stderr_total_bytes;
    report.stdout_truncated =
        process.stdout_truncated || process.stdout_summary.chars().count() > 2_000;
    report.stderr_truncated =
        process.stderr_truncated || process.stderr_summary.chars().count() > 2_000;
    report.process_spawn_error = process.spawn_error.clone();
    report.process_kill_error = process.kill_error.clone();
    report.process_wait_error = process.wait_error.clone();
    report.process_reader_join_error = process.reader_join_error.clone();

    let path = paths.game_exe_path.display().to_string();
    for (code, error) in [
        ("process_spawn_failed", process.spawn_error),
        ("process_kill_failed", process.kill_error),
        ("process_wait_failed", process.wait_error),
        ("process_reader_join_failed", process.reader_join_error),
    ] {
        if let Some(error) = error {
            report.diagnostics.push(
                ExportedPlayerProcessVerificationDiagnostic::error(code, error)
                    .with_path(path.clone()),
            );
        }
    }
    if process.exit_reason == BoundedChildProcessExitReason::Timeout {
        report.diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error(
                "process_timeout",
                "entrypoint executable exceeded its bounded execution timeout",
            )
            .with_path(path),
        );
    }
}

fn read_child_report(
    paths: &ResolvedVerificationPaths,
    report: &mut ExportedPlayerProcessVerificationReport,
) {
    let text = match fs::read_to_string(&paths.child_report_path) {
        Ok(text) => text,
        Err(error) => {
            report.diagnostics.push(
                ExportedPlayerProcessVerificationDiagnostic::error(
                    "child_report_missing",
                    format!("entrypoint executable did not write child report: {error}"),
                )
                .with_path(paths.child_report_path.display().to_string()),
            );
            return;
        }
    };
    let child_report = match serde_json::from_str::<WindowedPlayerRunReport>(&text) {
        Ok(report) => report,
        Err(error) => {
            report.diagnostics.push(
                ExportedPlayerProcessVerificationDiagnostic::error(
                    "child_report_parse_failed",
                    format!("failed to parse child report: {error}"),
                )
                .with_path(paths.child_report_path.display().to_string()),
            );
            return;
        }
    };
    report.child_player_exit_code = child_report.exit_code;
    report.child_present_status = Some(child_report.status.present.clone());
    report.child_frames_completed = Some(child_report.counters.frames_completed);
    read_child_screenshot_summary(&text, report);
    if child_report.exit_code != Some(0) {
        report.diagnostics.push(
            ExportedPlayerProcessVerificationDiagnostic::error(
                "child_player_failed",
                format!(
                    "entrypoint executable child player report failed: {}",
                    child_report.exit_reason
                ),
            )
            .with_path(paths.child_report_path.display().to_string()),
        );
    }
}

fn screenshot_args(
    request: &ExportedPlayerProcessVerificationRequest,
    paths: &ResolvedVerificationPaths,
) -> Vec<String> {
    if !request.screenshot {
        return Vec::new();
    }
    vec![
        "--screenshot".to_string(),
        "--screenshot-path".to_string(),
        paths.child_screenshot_path.display().to_string(),
    ]
}

fn read_child_screenshot_summary(
    child_report_text: &str,
    report: &mut ExportedPlayerProcessVerificationReport,
) {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(child_report_text) {
        let screenshot = value
            .get("screenshotSummary")
            .or_else(|| value.get("screenshot"));
        if let Some(screenshot) = screenshot {
            report.screenshot_requested = screenshot
                .get("requested")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            report.screenshot_status = screenshot
                .get("status")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
            report.screenshot_path = screenshot
                .get("path")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
    }
}

fn summarize(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_string()
    } else {
        let retained = limit.saturating_sub(3);
        format!("{}...", text.chars().take(retained).collect::<String>())
    }
}

fn finalize_report(
    parent_report_path: &Path,
    mut report: ExportedPlayerProcessVerificationReport,
) -> ExportedPlayerProcessVerificationReport {
    report.recompute_status();
    if let Some(parent) = parent_report_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(&report) {
        Ok(text) => {
            if let Err(error) = fs::write(parent_report_path, text) {
                report.diagnostics.push(
                    ExportedPlayerProcessVerificationDiagnostic::error(
                        "parent_report_write_failed",
                        format!("failed to write parent verification report: {error}"),
                    )
                    .with_path(parent_report_path.display().to_string()),
                );
                report.recompute_status();
            }
        }
        Err(error) => {
            report
                .diagnostics
                .push(ExportedPlayerProcessVerificationDiagnostic::error(
                    "parent_report_serialize_failed",
                    format!("failed to serialize parent verification report: {error}"),
                ));
            report.recompute_status();
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::canonical_digest::sha256_prefixed;
    use engine_runtime::release_package_manifest::{
        release_payload_hash, ReleasePackageApplication, ReleasePackageFile,
        ReleasePackageFileRole, ReleasePackageLaunch, ReleasePackageTarget,
        RELEASE_PACKAGE_MANIFEST_FILE_NAME,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn exported_player_process_verification_report_serializes() {
        let root = temp_root("report-serializes");
        let request = ExportedPlayerProcessVerificationRequest {
            exported_package_dir: root.join("Build").join("Windows").join("dev"),
            mode: "headless-gate".to_string(),
            frame_limit: 3,
            report_path: None,
            timeout_ms: 100,
            screenshot: false,
            screenshot_path: None,
        };
        let paths = resolve_paths(&request);
        let report = ExportedPlayerProcessVerificationReport::failed(
            &paths,
            &request.mode,
            request.frame_limit,
        );

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains(EXPORTED_PLAYER_PROCESS_VERIFICATION_REPORT_SCHEMA_VERSION));
        assert!(json.contains("headless-gate"));
        assert!(json.contains("\"processElapsedMs\":0"));
        assert!(json.contains("\"stdoutTotalBytes\":0"));
        assert!(json.contains("\"processReaderJoinError\":null"));
    }

    #[test]
    fn process_report_summary_includes_ellipsis_within_unicode_limit() {
        let summary = summarize(&"输".repeat(2_100), 2_000);

        assert_eq!(summary.chars().count(), 2_000);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn exported_player_process_verification_reports_missing_game_exe() {
        let root = temp_root("missing-game-exe");
        let exported = root.join("Build").join("Windows").join("dev");
        fs::create_dir_all(exported.join("data").join("runtime_package")).unwrap();
        fs::write(exported.join("package-manifest.json"), "{}").unwrap();
        fs::write(
            exported
                .join("data")
                .join("runtime_package")
                .join("manifest.json"),
            "{}",
        )
        .unwrap();

        let report = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
            exported_package_dir: exported,
            mode: "headless-gate".to_string(),
            frame_limit: 1,
            report_path: None,
            timeout_ms: 100,
            screenshot: false,
            screenshot_path: None,
        });

        assert_eq!(
            report.status,
            ExportedPlayerProcessVerificationStatus::Failed
        );
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "game_exe_missing"));
    }

    #[test]
    fn release_package_verification_resolves_manifest_entrypoint_and_external_reports() {
        let root = temp_root("release-manifest-entrypoint");
        let exported = root.join("ComplexShooter");
        let runtime_package = exported.join("data/runtime_package");
        fs::create_dir_all(&runtime_package).unwrap();
        let entrypoint = exported.join("ComplexShooter.exe");
        fs::write(&entrypoint, b"entrypoint").unwrap();
        fs::write(runtime_package.join("manifest.json"), b"runtime").unwrap();
        let entrypoint_bytes = fs::read(&entrypoint).unwrap();
        let runtime_bytes = fs::read(runtime_package.join("manifest.json")).unwrap();
        let files = vec![
            ReleasePackageFile {
                path: "ComplexShooter.exe".to_string(),
                size: entrypoint_bytes.len() as u64,
                sha256: sha256_prefixed(&entrypoint_bytes),
                roles: vec![
                    ReleasePackageFileRole::Entrypoint,
                    ReleasePackageFileRole::Runtime,
                ],
            },
            ReleasePackageFile {
                path: "data/runtime_package/manifest.json".to_string(),
                size: runtime_bytes.len() as u64,
                sha256: sha256_prefixed(&runtime_bytes),
                roles: vec![ReleasePackageFileRole::RuntimePayload],
            },
        ];
        let manifest = ReleasePackageManifest {
            schema_version: RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION.to_string(),
            application: ReleasePackageApplication {
                display_name: "Complex Shooter".to_string(),
                executable_name: "ComplexShooter".to_string(),
                company_name: "AI First Engine Studio".to_string(),
                file_description: "Complex Shooter".to_string(),
                display_version: "1.0.0".to_string(),
                windows_file_version: [1, 0, 0, 0],
                windows_product_version: [1, 0, 0, 0],
                copyright: "Copyright AI First Engine Studio".to_string(),
            },
            target: ReleasePackageTarget {
                platform: "windows".to_string(),
                architecture: "x86_64".to_string(),
                profile: "release".to_string(),
            },
            launch: ReleasePackageLaunch {
                user_frame_limit: None,
            },
            entrypoint: "ComplexShooter.exe".to_string(),
            runtime_package: "data/runtime_package".to_string(),
            runtime_content_hash: format!("sha256:{}", "a".repeat(64)),
            release_payload_hash: release_payload_hash(&files),
            files,
        };
        fs::write(
            exported.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let paths = resolve_paths(&ExportedPlayerProcessVerificationRequest {
            exported_package_dir: exported.clone(),
            mode: "headless-gate".to_string(),
            frame_limit: 2,
            report_path: None,
            timeout_ms: 100,
            screenshot: false,
            screenshot_path: None,
        });

        assert_eq!(paths.package_kind, "release");
        assert_eq!(paths.entrypoint_relative_path, "ComplexShooter.exe");
        assert_eq!(paths.game_exe_path, entrypoint);
        assert_eq!(paths.runtime_package_path, runtime_package);
        assert!(!paths.child_report_path.starts_with(&exported));
        assert!(!paths.parent_report_path.starts_with(&exported));
        assert!(paths.resolution_diagnostics.is_empty());
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("exported-player-verification-{name}-{stamp}"))
    }
}
use crate::{
    run_bounded_child_process, BoundedChildProcessExitReason, BoundedChildProcessRequest,
    BoundedChildProcessResult,
};
