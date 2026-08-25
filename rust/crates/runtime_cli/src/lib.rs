use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use engine_runtime::release_package_manifest::{
    validate_release_package_manifest, ReleasePackageFileRole, ReleasePackageManifest,
    RELEASE_PACKAGE_MANIFEST_FILE_NAME,
};
#[cfg(feature = "real-window")]
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package_path::safe_join_runtime_package;
use engine_runtime::runtime_run::{RuntimeRunDiagnostic, RuntimeRunMode, RuntimeRunReport};
use engine_runtime::windowed_player::{
    WindowedPlayerHost, WindowedPlayerMode, WindowedPlayerRunReport, WindowedPlayerRunRequest,
    WindowedPlayerRuntimeReportLevel, WindowedPlayerScreenshotSummary,
};
use runtime_player_winit::{
    NativePlayerInputScript, NativePlayerWindowRunRequest, NativeWindowPresentStatus,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod bounded_child_process;
pub mod exported_player_verification;
pub use bounded_child_process::{
    run_bounded_child_process, run_bounded_child_process_cancellable,
    BoundedChildProcessCancellation, BoundedChildProcessExitReason, BoundedChildProcessPriority,
    BoundedChildProcessPriorityEvidence, BoundedChildProcessRequest, BoundedChildProcessResult,
    BoundedProcessOwnershipEvidence, BoundedProcessOwnershipKind,
};
pub use exported_player_verification::{
    verify_exported_player_process, verify_exported_player_process_with_options,
    ExportedPlayerProcessVerificationDiagnostic, ExportedPlayerProcessVerificationOptions,
    ExportedPlayerProcessVerificationReport, ExportedPlayerProcessVerificationRequest,
    ExportedPlayerProcessVerificationStatus,
    EXPORTED_PLAYER_PROCESS_VERIFICATION_REPORT_SCHEMA_VERSION,
};

pub fn run_from_env() -> i32 {
    run_from_env_with_linked_modules(Arc::new(LinkedProjectRuntimeSet::explicit_empty()))
}

pub fn run_from_env_with_linked_modules(linked_modules: Arc<LinkedProjectRuntimeSet>) -> i32 {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(error) => {
                eprintln!(
                    "{}",
                    usage(format!(
                        "missing arguments; current executable unavailable: {error}"
                    ))
                );
                return 2;
            }
        };
        return match run_packaged_release(&executable, linked_modules) {
            Ok(exit_code) => exit_code,
            Err(error) => {
                eprintln!(
                    "{}",
                    usage(format!(
                        "missing arguments; packaged entrypoint unavailable: {error}"
                    ))
                );
                2
            }
        };
    }
    run_from_args_with_linked_modules(args, linked_modules)
}

pub fn run_from_args<I, S>(args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run_from_args_with_linked_modules(args, Arc::new(LinkedProjectRuntimeSet::explicit_empty()))
}

pub fn run_from_args_with_linked_modules<I, S>(
    args: I,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let cli = RuntimeCliArgs::parse(args.into_iter().map(Into::into).collect());
    match cli {
        Ok(cli) => run_cli(cli, linked_modules),
        Err(message) => {
            eprintln!("{message}");
            2
        }
    }
}

fn run_cli(cli: RuntimeCliArgs, linked_modules: Arc<LinkedProjectRuntimeSet>) -> i32 {
    if cli.verify_exported_player {
        let package_dir = cli
            .package
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let request = ExportedPlayerProcessVerificationRequest {
            exported_package_dir: package_dir,
            mode: cli
                .mode
                .clone()
                .unwrap_or_else(|| "headless-gate".to_string()),
            frame_limit: cli.frames,
            report_path: cli.report.clone(),
            timeout_ms: 30_000,
            screenshot: cli.screenshot,
            screenshot_path: cli.screenshot_path.clone(),
        };
        let report = verify_exported_player_process_with_options(
            request,
            ExportedPlayerProcessVerificationOptions {
                input_script_path: cli.input_script.clone(),
                runtime_report_level: Some(
                    match cli.runtime_report_level {
                        WindowedPlayerRuntimeReportLevel::Off => "off",
                        WindowedPlayerRuntimeReportLevel::Summary => "summary",
                        WindowedPlayerRuntimeReportLevel::Trace => "trace",
                    }
                    .to_string(),
                ),
                performance_warmup_frames: cli.performance_warmup_frames,
                performance_sample_frames: cli.performance_sample_frames,
            },
        );
        let exit_code = report.exit_code.unwrap_or(1);
        if cli.report.is_none() {
            match serde_json::to_string_pretty(&report) {
                Ok(text) => println!("{text}"),
                Err(error) => {
                    eprintln!("failed to serialize exported player verification report: {error}");
                    return 1;
                }
            }
        }
        return exit_code;
    }
    run_native_player_cli(cli, linked_modules)
}

fn run_native_player_cli(cli: RuntimeCliArgs, linked_modules: Arc<LinkedProjectRuntimeSet>) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| cwd.clone());
    run_native_player_cli_with_dirs(cli, &exe_dir, &cwd, linked_modules)
}

fn run_native_player_cli_with_dirs(
    cli: RuntimeCliArgs,
    exe_dir: &Path,
    cwd: &Path,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
) -> i32 {
    let (report, report_path) = run_native_player_with_dirs(&cli, exe_dir, cwd, linked_modules);
    let exit_code = report.exit_code.unwrap_or(1);
    if let Some(report_path) = report_path {
        if let Err(error) = write_windowed_player_report(&report_path, &report) {
            eprintln!(
                "failed to write native player report {}: {}",
                report_path.display(),
                error
            );
            return 1;
        }
    }
    exit_code
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagedEntrypoint {
    pub package_root: PathBuf,
    pub entrypoint: PathBuf,
    pub runtime_package: PathBuf,
    pub user_frame_limit: Option<u64>,
}

const DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "desktop-package-manifest.v1";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackagedManifestSchemaProbe {
    schema_version: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopDevPackageManifest {
    schema_version: String,
    target: String,
    profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagedEntrypointError {
    pub code: &'static str,
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for PackagedEntrypointError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} for {}: {}",
            self.code,
            self.path.display(),
            self.message
        )
    }
}

impl std::error::Error for PackagedEntrypointError {}

pub fn resolve_packaged_entrypoint(
    executable: &Path,
) -> Result<PackagedEntrypoint, PackagedEntrypointError> {
    let package_root = executable
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            packaged_error(
                "release_entrypoint_missing",
                executable,
                "current executable has no package parent directory",
            )
        })?
        .to_path_buf();
    let manifest_path = package_root.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME);
    let text = fs::read_to_string(&manifest_path).map_err(|error| {
        packaged_error(
            "release_manifest_invalid",
            &manifest_path,
            error.to_string(),
        )
    })?;
    let schema: PackagedManifestSchemaProbe = serde_json::from_str(&text).map_err(|error| {
        packaged_error(
            "release_manifest_invalid",
            &manifest_path,
            error.to_string(),
        )
    })?;
    if schema.schema_version == DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION {
        return resolve_desktop_dev_packaged_entrypoint(
            executable,
            package_root,
            &manifest_path,
            &text,
        );
    }
    let manifest: ReleasePackageManifest = serde_json::from_str(&text).map_err(|error| {
        packaged_error(
            "release_manifest_invalid",
            &manifest_path,
            error.to_string(),
        )
    })?;
    let diagnostics = validate_release_package_manifest(&manifest);
    if !diagnostics.is_empty() {
        return Err(packaged_error(
            "release_manifest_invalid",
            &manifest_path,
            diagnostics
                .iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let entrypoint =
        safe_join_runtime_package(&package_root, &manifest.entrypoint).map_err(|error| {
            packaged_error("release_path_escape", &manifest_path, error.to_string())
        })?;
    let runtime_package = safe_join_runtime_package(&package_root, &manifest.runtime_package)
        .map_err(|error| {
            packaged_error("release_path_escape", &manifest_path, error.to_string())
        })?;
    let expected = fs::canonicalize(&entrypoint).map_err(|error| {
        packaged_error("release_entrypoint_missing", &entrypoint, error.to_string())
    })?;
    let current = fs::canonicalize(executable).map_err(|error| {
        packaged_error("release_entrypoint_missing", executable, error.to_string())
    })?;
    if expected != current
        || !manifest.files.iter().any(|file| {
            file.path == manifest.entrypoint
                && file.roles.contains(&ReleasePackageFileRole::Entrypoint)
        })
    {
        return Err(packaged_error(
            "release_entrypoint_missing",
            executable,
            format!(
                "manifest entrypoint {} does not identify current executable",
                manifest.entrypoint
            ),
        ));
    }
    if !runtime_package.join("manifest.json").is_file() {
        return Err(packaged_error(
            "release_runtime_package_load_failed",
            &runtime_package,
            "manifest.runtimePackage has no RuntimePackage manifest.json",
        ));
    }
    Ok(PackagedEntrypoint {
        package_root,
        entrypoint,
        runtime_package,
        user_frame_limit: manifest.launch.user_frame_limit,
    })
}

fn resolve_desktop_dev_packaged_entrypoint(
    executable: &Path,
    package_root: PathBuf,
    manifest_path: &Path,
    text: &str,
) -> Result<PackagedEntrypoint, PackagedEntrypointError> {
    let manifest: DesktopDevPackageManifest = serde_json::from_str(text).map_err(|error| {
        packaged_error("release_manifest_invalid", manifest_path, error.to_string())
    })?;
    if manifest.schema_version != DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION
        || manifest.target != "windows"
        || manifest.profile != "dev"
    {
        return Err(packaged_error(
            "release_manifest_invalid",
            manifest_path,
            "desktop package manifest must target windows/dev",
        ));
    }
    fs::canonicalize(executable).map_err(|error| {
        packaged_error("release_entrypoint_missing", executable, error.to_string())
    })?;
    let runtime_package = safe_join_runtime_package(&package_root, "data/runtime_package")
        .map_err(|error| packaged_error("release_path_escape", manifest_path, error.to_string()))?;
    if !runtime_package.join("manifest.json").is_file() {
        return Err(packaged_error(
            "release_runtime_package_load_failed",
            &runtime_package,
            "desktop package has no data/runtime_package/manifest.json",
        ));
    }
    Ok(PackagedEntrypoint {
        package_root,
        entrypoint: executable.to_path_buf(),
        runtime_package,
        user_frame_limit: None,
    })
}

fn run_packaged_release(
    executable: &Path,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
) -> Result<i32, PackagedEntrypointError> {
    let packaged = resolve_packaged_entrypoint(executable)?;
    let cli = RuntimeCliArgs {
        package: Some(packaged.runtime_package.clone()),
        mode: Some("windowed".to_string()),
        frames: packaged.user_frame_limit.unwrap_or(u64::MAX),
        report: None,
        report_off: true,
        native_player: true,
        verify_exported_player: false,
        screenshot: false,
        screenshot_path: None,
        input_script: None,
        runtime_report_level: WindowedPlayerRuntimeReportLevel::Off,
        performance_warmup_frames: 0,
        performance_sample_frames: 0,
    };
    let exe_dir = packaged
        .entrypoint
        .parent()
        .unwrap_or(&packaged.package_root);
    Ok(run_native_player_cli_with_dirs(
        cli,
        exe_dir,
        &packaged.package_root,
        linked_modules,
    ))
}

fn packaged_error(
    code: &'static str,
    path: &Path,
    message: impl Into<String>,
) -> PackagedEntrypointError {
    PackagedEntrypointError {
        code,
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn write_windowed_player_report(
    path: &Path,
    report: &WindowedPlayerRunReport,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(report)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}

fn package_dir_from_arg(path: &Path) -> PathBuf {
    if path.file_name().and_then(|name| name.to_str()) == Some("manifest.json") {
        return path.parent().unwrap_or(path).to_path_buf();
    }
    path.to_path_buf()
}

fn run_native_player_with_dirs(
    cli: &RuntimeCliArgs,
    exe_dir: &Path,
    cwd: &Path,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
) -> (WindowedPlayerRunReport, Option<PathBuf>) {
    let paths = resolve_native_player_paths(
        cli.package.as_deref(),
        cli.report.as_deref(),
        cli.report_off,
        exe_dir,
        cwd,
    );
    let mode = match cli.mode.as_deref() {
        Some("headless") | Some("headless-gate") => WindowedPlayerMode::HeadlessGate,
        _ => WindowedPlayerMode::Windowed,
    };
    if mode == WindowedPlayerMode::HeadlessGate {
        return (
            run_headless_native_player_with_host(
                &paths.package,
                cli.frames,
                cli,
                linked_modules.as_ref(),
            ),
            paths.report,
        );
    }
    #[cfg(feature = "real-window")]
    if mode == WindowedPlayerMode::Windowed {
        return (
            run_windowed_native_player_with_real_host(
                &paths.package,
                cli.frames,
                cli.screenshot,
                cli.screenshot_path.as_deref(),
                cli,
                linked_modules,
            ),
            paths.report,
        );
    }
    let mut request = match mode {
        WindowedPlayerMode::HeadlessGate => WindowedPlayerRunRequest::headless_gate(&paths.package),
        WindowedPlayerMode::Windowed => WindowedPlayerRunRequest::windowed(&paths.package),
    };
    request.project_path = paths
        .package
        .parent()
        .unwrap_or(&paths.package)
        .to_path_buf();
    request.frame_limit = cli.frames;
    request.scenario_id = "native_player_productization_v1".to_string();
    (WindowedPlayerHost::run_headless_gate(request), paths.report)
}

#[cfg(feature = "real-window")]
fn run_windowed_native_player_with_real_host(
    package: &Path,
    frames: u64,
    screenshot: bool,
    screenshot_path: Option<&Path>,
    cli: &RuntimeCliArgs,
    linked_modules: Arc<LinkedProjectRuntimeSet>,
) -> WindowedPlayerRunReport {
    let mut request = NativePlayerWindowRunRequest::windowed(package);
    if let Some(target) = load_runtime_package(package)
        .value
        .and_then(|runtime_package| runtime_package.manifest.project.game_view_target)
    {
        request = request.with_game_view_target(target);
    }
    request.frame_limit = frames;
    configure_native_evidence_request(&mut request, cli);
    if let Some(path) = screenshot_path {
        request = request.with_screenshot(path);
    } else if screenshot {
        request = request.with_screenshot(
            package
                .parent()
                .unwrap_or(package)
                .join("reports")
                .join("windowed-player-screenshot.png"),
        );
    }
    let native_report =
        runtime_player_winit::run_windowed_native_player_from_package_with_linked_modules(
            request,
            linked_modules,
        );
    native_window_report(package, frames, WindowedPlayerMode::Windowed, native_report)
}

fn run_headless_native_player_with_host(
    package: &Path,
    frames: u64,
    cli: &RuntimeCliArgs,
    linked_modules: &LinkedProjectRuntimeSet,
) -> WindowedPlayerRunReport {
    let mut request = NativePlayerWindowRunRequest::headless_surface_gate(package);
    request.frame_limit = frames;
    configure_native_evidence_request(&mut request, cli);
    let native_report =
        runtime_player_winit::run_headless_native_player_from_package_with_linked_modules(
            request,
            linked_modules,
        );
    native_window_report(
        package,
        frames,
        WindowedPlayerMode::HeadlessGate,
        native_report,
    )
}

fn configure_native_evidence_request(
    request: &mut NativePlayerWindowRunRequest,
    cli: &RuntimeCliArgs,
) {
    request.runtime_report_level = cli.runtime_report_level;
    request.performance_warmup_frames = cli.performance_warmup_frames;
    request.performance_sample_frames = cli.performance_sample_frames;
    request.input_script = cli.input_script.as_ref().map(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<NativePlayerInputScript>(&text).ok())
            .unwrap_or_else(|| NativePlayerInputScript {
                schema_version: format!("input_script_load_failed:{}", path.display()),
                script_id: "invalid".to_string(),
                frames: Vec::new(),
            })
    });
}

fn native_window_report(
    package: &Path,
    frames: u64,
    mode: WindowedPlayerMode,
    native_report: runtime_player_winit::NativeWindowHostReport,
) -> WindowedPlayerRunReport {
    // Build the shared package/asset/counter fields through the headless report path. A Windowed
    // request here would deliberately emit native_window_host_required even though the native
    // host has already run and supplied the authoritative window evidence below.
    let mut player_request = WindowedPlayerRunRequest::headless_gate(package);
    player_request.frame_limit = frames;
    player_request.scenario_id = "native_player_productization_v1".to_string();
    let mut report = WindowedPlayerHost::run_headless_gate(player_request);
    report.mode = mode;
    report.status.package = native_report.package_status.clone();
    report.status.scene = native_report.scene_status.clone();
    report.status.world = native_report.world_status.clone();
    report.status.logic = native_report.logic_status.clone();
    report.status.input = native_report.input_status.clone();
    report.status.render = native_report.render_status.clone();
    report.status.rhi = native_report.rhi_status.clone();
    report.status.surface = native_report.surface_status.clone();
    report.status.present = native_report.present_status.as_str().to_string();
    report.screenshot_summary = Some(WindowedPlayerScreenshotSummary {
        requested: native_report.screenshot.requested,
        status: native_report.screenshot.status.as_str().to_string(),
        path: native_report.screenshot.path.clone(),
        width: native_report.screenshot.width,
        height: native_report.screenshot.height,
        byte_size: native_report.screenshot.byte_size,
    });
    report.project_runtime_bind_receipt = native_report.project_runtime_bind_receipt.clone();
    report.frame_performance_summary = native_report.frame_performance_summary.clone();
    report.gameplay_trace_summary = native_report.gameplay_trace_summary.clone();
    report.gameplay_trace_records = native_report.gameplay_trace_records.clone();
    report.exit_code = Some(native_report.exit_code);
    report.exit_reason = if native_report.exit_code == 0 {
        "completed".to_string()
    } else {
        native_report.present_status.as_str().to_string()
    };
    report.counters.frames_completed = native_report.frames_completed;
    report
        .diagnostics
        .extend(native_report.diagnostics.iter().map(|diagnostic| {
            engine_runtime::windowed_player::WindowedPlayerDiagnostic {
                severity: match diagnostic.severity {
                    runtime_player_winit::NativeWindowHostDiagnosticSeverity::Info => {
                        engine_runtime::windowed_player::WindowedPlayerDiagnosticSeverity::Info
                    }
                    runtime_player_winit::NativeWindowHostDiagnosticSeverity::Warning => {
                        engine_runtime::windowed_player::WindowedPlayerDiagnosticSeverity::Warning
                    }
                    runtime_player_winit::NativeWindowHostDiagnosticSeverity::Error => {
                        engine_runtime::windowed_player::WindowedPlayerDiagnosticSeverity::Error
                    }
                },
                code: format!("native_host.{}", diagnostic.code),
                layer: diagnostic.layer.clone(),
                message: diagnostic.message.clone(),
                path: diagnostic.path.clone(),
            }
        }));
    if native_report.present_status == NativeWindowPresentStatus::Presented {
        report.status.request = "ok".to_string();
    }
    report
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativePlayerResolvedPaths {
    package: PathBuf,
    report: Option<PathBuf>,
}

fn resolve_native_player_paths(
    package_override: Option<&Path>,
    report_override: Option<&Path>,
    report_off: bool,
    exe_dir: &Path,
    cwd: &Path,
) -> NativePlayerResolvedPaths {
    let (package, report_root) = if let Some(package) = package_override {
        (package_dir_from_arg(package), exe_dir.to_path_buf())
    } else {
        let exe_package = exe_dir.join("data").join("runtime_package");
        if exe_package.exists() {
            (exe_package, exe_dir.to_path_buf())
        } else {
            (cwd.join("data").join("runtime_package"), cwd.to_path_buf())
        }
    };
    let report = if report_off {
        None
    } else {
        Some(report_override.map(Path::to_path_buf).unwrap_or_else(|| {
            report_root
                .join("reports")
                .join("windowed-player-run-report.json")
        }))
    };
    NativePlayerResolvedPaths { package, report }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeCliArgs {
    package: Option<PathBuf>,
    mode: Option<String>,
    frames: u64,
    report: Option<PathBuf>,
    report_off: bool,
    native_player: bool,
    verify_exported_player: bool,
    screenshot: bool,
    screenshot_path: Option<PathBuf>,
    input_script: Option<PathBuf>,
    runtime_report_level: WindowedPlayerRuntimeReportLevel,
    performance_warmup_frames: u64,
    performance_sample_frames: u64,
}

impl RuntimeCliArgs {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        if args.is_empty() {
            return Err(usage("missing arguments"));
        }
        let mut package = None;
        let mut mode = None;
        let mut frames = None;
        let mut report = None;
        let mut native_player = false;
        let mut verify_exported_player = false;
        let mut screenshot = false;
        let mut screenshot_path = None;
        let mut input_script = None;
        let mut runtime_report_level = WindowedPlayerRuntimeReportLevel::Off;
        let mut performance_warmup_frames = 0;
        let mut performance_sample_frames = 0;
        let mut index = 0;
        while index < args.len() {
            let key = &args[index];
            match key.as_str() {
                "run-native-player" | "--native-player" => native_player = true,
                "verify-exported-player" | "--verify-exported-player" => {
                    verify_exported_player = true
                }
                "--package" => package = Some(read_value(&args, &mut index, key)?),
                "--mode" => mode = Some(read_value(&args, &mut index, key)?),
                "--frames" | "--frame-limit" => {
                    let value = read_value(&args, &mut index, key)?;
                    frames = Some(
                        value
                            .parse::<u64>()
                            .map_err(|_| usage(format!("{key} must be a positive integer")))?,
                    );
                }
                "--report" => report = Some(PathBuf::from(read_value(&args, &mut index, key)?)),
                "--screenshot" => screenshot = true,
                "--screenshot-path" => {
                    screenshot = true;
                    screenshot_path = Some(PathBuf::from(read_value(&args, &mut index, key)?));
                }
                "--input-script" => {
                    input_script = Some(PathBuf::from(read_value(&args, &mut index, key)?));
                }
                "--runtime-report-level" => {
                    runtime_report_level = match read_value(&args, &mut index, key)?.as_str() {
                        "off" => WindowedPlayerRuntimeReportLevel::Off,
                        "summary" => WindowedPlayerRuntimeReportLevel::Summary,
                        "trace" => WindowedPlayerRuntimeReportLevel::Trace,
                        value => {
                            return Err(usage(format!(
                                "--runtime-report-level must be off, summary, or trace; got {value}"
                            )))
                        }
                    };
                }
                "--performance-warmup-frames" => {
                    performance_warmup_frames = read_value(&args, &mut index, key)?
                        .parse::<u64>()
                        .map_err(|_| usage(format!("{key} must be a non-negative integer")))?;
                }
                "--performance-sample-frames" => {
                    performance_sample_frames = read_value(&args, &mut index, key)?
                        .parse::<u64>()
                        .map_err(|_| usage(format!("{key} must be a non-negative integer")))?;
                }
                "--headless" => mode = Some("headless".to_string()),
                "--headless-gate" => mode = Some("headless-gate".to_string()),
                other => return Err(usage(format!("unknown argument: {other}"))),
            }
            index += 1;
        }
        if [native_player, verify_exported_player]
            .into_iter()
            .filter(|value| *value)
            .count()
            > 1
        {
            return Err(usage(
                "run-native-player and verify-exported-player are separate entry points",
            ));
        }
        let mode = mode.unwrap_or_else(|| {
            if native_player {
                "windowed".to_string()
            } else if verify_exported_player {
                "headless-gate".to_string()
            } else {
                "headless".to_string()
            }
        });
        let native_mode_ok =
            native_player && matches!(mode.as_str(), "headless" | "headless-gate" | "windowed");
        let verify_mode_ok = verify_exported_player
            && matches!(mode.as_str(), "headless" | "headless-gate" | "windowed");
        if mode != "headless" && !native_mode_ok && !verify_mode_ok {
            return Err(usage(format!(
                "runtime CLI v1 only supports --mode headless outside run-native-player/verify-exported-player; got {mode}"
            )));
        }
        if !native_player && !verify_exported_player && package.is_none() {
            return Err(usage("missing --package"));
        }
        let frames = frames.unwrap_or(1);
        if frames == 0 {
            return Err(usage("--frames must be greater than 0"));
        }
        Ok(Self {
            package: package.map(PathBuf::from),
            mode: Some(mode),
            frames,
            report,
            report_off: false,
            native_player,
            verify_exported_player,
            screenshot,
            screenshot_path,
            input_script,
            runtime_report_level,
            performance_warmup_frames,
            performance_sample_frames,
        })
    }
}

fn read_value(args: &[String], index: &mut usize, key: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| usage(format!("{key} requires a value")))
}

fn usage(message: impl Into<String>) -> String {
    format!(
        "{}\nUsage: ai_engine_runtime_cli --package <runtime-package> --mode headless --frames <N> --report <windowed-player-run-report.json>\n       ai_engine_runtime_cli run-native-player [--package <runtime-package>] [--mode windowed|headless-gate] [--frames <N>] [--report <windowed-player-run-report.json>] [--screenshot] [--screenshot-path <png>]\n       ai_engine_runtime_cli verify-exported-player [--package <exported-package-dir>] [--mode headless-gate|windowed] [--frames <N>] [--report <exported-player-process-verification-report.json>] [--screenshot] [--screenshot-path <png>]",
        message.into()
    )
}

pub fn failure_report_for_cli_error(message: impl Into<String>) -> RuntimeRunReport {
    RuntimeRunReport::failed(
        RuntimeRunMode::Headless,
        0,
        vec![RuntimeRunDiagnostic::error("cli_error", message)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::canonical_digest::sha256_prefixed;
    use engine_runtime::release_package_manifest::{
        release_payload_hash, ReleasePackageApplication, ReleasePackageFile, ReleasePackageLaunch,
        ReleasePackageTarget, RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_cli_rejects_missing_package() {
        assert!(
            RuntimeCliArgs::parse(vec!["--mode".to_string(), "headless".to_string()])
                .unwrap_err()
                .contains("missing --package")
        );
    }

    #[test]
    fn runtime_cli_parses_package_frames_and_report() {
        let args = RuntimeCliArgs::parse(vec![
            "--package".to_string(),
            "runtime-package".to_string(),
            "--mode".to_string(),
            "headless".to_string(),
            "--frames".to_string(),
            "3".to_string(),
            "--report".to_string(),
            "reports/runtime-run-report.json".to_string(),
        ])
        .unwrap();

        assert_eq!(args.package, Some(PathBuf::from("runtime-package")));
        assert_eq!(args.mode.as_deref(), Some("headless"));
        assert_eq!(args.frames, 3);
        assert_eq!(
            args.report,
            Some(PathBuf::from("reports/runtime-run-report.json"))
        );
    }

    #[test]
    fn native_player_args_allow_missing_package_for_default_discovery() {
        let args = RuntimeCliArgs::parse(vec![
            "run-native-player".to_string(),
            "--headless-gate".to_string(),
            "--frames".to_string(),
            "2".to_string(),
        ])
        .unwrap();

        assert!(args.native_player);
        assert_eq!(args.package, None);
        assert_eq!(args.mode.as_deref(), Some("headless-gate"));
        assert_eq!(args.frames, 2);
    }

    #[test]
    fn verify_exported_player_args_allow_missing_package_for_current_dir() {
        let args = RuntimeCliArgs::parse(vec![
            "verify-exported-player".to_string(),
            "--headless-gate".to_string(),
            "--frames".to_string(),
            "2".to_string(),
        ])
        .unwrap();

        assert!(args.verify_exported_player);
        assert_eq!(args.package, None);
        assert_eq!(args.mode.as_deref(), Some("headless-gate"));
        assert_eq!(args.frames, 2);
    }

    #[test]
    fn verify_exported_player_args_parse_screenshot_path() {
        let args = RuntimeCliArgs::parse(vec![
            "verify-exported-player".to_string(),
            "--package".to_string(),
            "Build/Windows/dev".to_string(),
            "--mode".to_string(),
            "windowed".to_string(),
            "--screenshot-path".to_string(),
            "reports/player.png".to_string(),
        ])
        .unwrap();

        assert!(args.verify_exported_player);
        assert!(args.screenshot);
        assert_eq!(
            args.screenshot_path,
            Some(PathBuf::from("reports/player.png"))
        );
        assert_eq!(args.mode.as_deref(), Some("windowed"));
    }

    #[test]
    fn native_player_resolves_explicit_package() {
        let root = temp_root("native-player-explicit-package");
        let exe_dir = root.join("bin");
        let cwd = root.join("workspace");
        let package = root.join("staged").join("runtime_package");
        let report = root.join("custom").join("report.json");
        let paths =
            resolve_native_player_paths(Some(&package), Some(&report), false, &exe_dir, &cwd);

        assert_eq!(paths.package, package);
        assert_eq!(paths.report, Some(report));
    }

    #[test]
    fn native_player_resolves_default_cwd_package_when_exe_data_is_absent() {
        let root = temp_root("native-player-cwd-package");
        let exe_dir = root.join("bin");
        let cwd = root.join("workspace");
        let paths = resolve_native_player_paths(None, None, false, &exe_dir, &cwd);

        assert_eq!(paths.package, cwd.join("data").join("runtime_package"));
        assert_eq!(
            paths.report,
            Some(cwd.join("reports").join("windowed-player-run-report.json"))
        );
    }

    #[test]
    fn native_player_prefers_exe_dir_data_package() {
        let root = temp_root("native-player-exe-package");
        let exe_dir = root.join("bin");
        let cwd = root.join("workspace");
        fs::create_dir_all(exe_dir.join("data").join("runtime_package")).unwrap();

        let paths = resolve_native_player_paths(None, None, false, &exe_dir, &cwd);

        assert_eq!(paths.package, exe_dir.join("data").join("runtime_package"));
        assert_eq!(
            paths.report,
            Some(
                exe_dir
                    .join("reports")
                    .join("windowed-player-run-report.json")
            )
        );
    }

    #[test]
    fn packaged_entrypoint_resolves_manifest_without_cwd_guessing() {
        let root = temp_root("packaged-entrypoint-resolve");
        let executable = write_packaged_entrypoint_fixture(&root, None);

        let packaged = resolve_packaged_entrypoint(&executable).unwrap();

        assert_eq!(packaged.package_root, root);
        assert_eq!(packaged.entrypoint, executable);
        assert_eq!(
            packaged.runtime_package,
            packaged.package_root.join("data/runtime_package")
        );
        assert_eq!(packaged.user_frame_limit, None);
    }

    #[test]
    fn packaged_entrypoint_resolves_desktop_dev_manifest_from_package_layout() {
        let root = temp_root("packaged-entrypoint-desktop-dev");
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("Game.exe");
        fs::write(&executable, b"test-entrypoint").unwrap();
        let runtime_package = write_minimal_runtime_package(&root.join("data"), "runtime_package");
        fs::write(
            root.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "desktop-package-manifest.v1",
                "target": "windows",
                "profile": "dev",
                "packageDir": "C:\\machine-bound\\export",
                "runtimePackageDir": "C:\\machine-bound\\export\\data\\runtime_package",
                "reportsDir": "C:\\machine-bound\\export\\reports",
                "playerExecutable": "C:\\machine-bound\\export\\Game.exe",
                "playerExecutableStatus": "copied"
            }))
            .unwrap(),
        )
        .unwrap();

        let packaged = resolve_packaged_entrypoint(&executable).unwrap();

        assert_eq!(packaged.package_root, root);
        assert_eq!(packaged.entrypoint, executable);
        assert_eq!(packaged.runtime_package, runtime_package);
        assert_eq!(packaged.user_frame_limit, None);
    }

    #[test]
    fn packaged_entrypoint_rejects_manifest_entrypoint_that_is_not_current_exe() {
        let root = temp_root("packaged-entrypoint-mismatch");
        let executable = write_packaged_entrypoint_fixture(&root, None);
        let other = root.join("Other.exe");
        fs::write(&other, b"other").unwrap();

        let error = resolve_packaged_entrypoint(&other).unwrap_err();

        assert_eq!(error.code, "release_entrypoint_missing");
        assert!(error
            .message
            .contains("does not identify current executable"));
        assert!(executable.is_file());
    }

    #[test]
    fn packaged_entrypoint_runtime_report_is_off() {
        let root = temp_root("packaged-entrypoint-report-off");
        let executable = write_packaged_entrypoint_fixture(&root, Some(1));

        let _ = run_packaged_release(
            &executable,
            Arc::new(LinkedProjectRuntimeSet::explicit_empty()),
        )
        .unwrap();

        assert!(!root.join("reports").exists());
        assert!(!root.join("data/runtime_package/reports").exists());
    }

    #[test]
    fn native_player_headless_gate_writes_report() {
        let root = temp_root("native-player-headless-report");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let report_path = root.join("reports").join("windowed-player-run-report.json");

        let exit = run_from_args([
            "run-native-player",
            "--package",
            package.to_str().unwrap(),
            "--headless-gate",
            "--frames",
            "3",
            "--report",
            report_path.to_str().unwrap(),
        ]);

        assert_eq!(exit, 0);
        let report: WindowedPlayerRunReport =
            serde_json::from_str(&fs::read_to_string(report_path).unwrap()).unwrap();
        assert_eq!(report.mode, WindowedPlayerMode::HeadlessGate);
        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.counters.frames_completed, 3);
        assert_eq!(report.status.package, "ok");
    }

    #[test]
    fn native_window_report_uses_authoritative_native_status_without_windowed_sentinel() {
        let root = temp_root("native-player-windowed-report-conversion");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let mut request = NativePlayerWindowRunRequest::headless_surface_gate(&package);
        request.frame_limit = 3;
        let mut native_report =
            runtime_player_winit::run_headless_native_player_from_package(request);
        assert_eq!(native_report.exit_code, 0);
        native_report.surface_status = "ok".to_string();

        let report = native_window_report(&package, 3, WindowedPlayerMode::Windowed, native_report);

        assert_eq!(report.mode, WindowedPlayerMode::Windowed);
        assert_eq!(report.status.package, "ok");
        assert_eq!(report.status.scene, "ok");
        assert_eq!(report.status.world, "ok");
        assert_eq!(report.status.logic, "ok");
        assert_eq!(report.status.input, "ok");
        assert_eq!(report.status.render, "ok");
        assert_eq!(report.status.rhi, "ok");
        assert_eq!(report.status.surface, "ok");
        assert_eq!(report.status.present, "presented");
        assert_eq!(report.counters.frames_completed, 3);
        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.exit_reason, "completed");
        assert!(!report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "native_window_host_required"));
    }

    #[test]
    fn native_player_missing_package_writes_report() {
        let root = temp_root("native-player-missing-package");
        let report_path = root.join("reports").join("windowed-player-run-report.json");

        let exit = run_from_args([
            "run-native-player",
            "--package",
            root.join("missing-package").to_str().unwrap(),
            "--headless-gate",
            "--frames",
            "1",
            "--report",
            report_path.to_str().unwrap(),
        ]);

        assert_eq!(exit, 1);
        let report: WindowedPlayerRunReport =
            serde_json::from_str(&fs::read_to_string(report_path).unwrap()).unwrap();
        assert_eq!(report.status.package, "error");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.layer == "package"));
    }

    #[cfg(not(feature = "real-window"))]
    #[test]
    fn native_player_windowed_mode_reports_native_host_required() {
        let root = temp_root("native-player-windowed-required");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let report_path = root.join("reports").join("windowed-player-run-report.json");

        let exit = run_from_args([
            "run-native-player",
            "--package",
            package.to_str().unwrap(),
            "--mode",
            "windowed",
            "--frames",
            "1",
            "--report",
            report_path.to_str().unwrap(),
        ]);

        assert_eq!(exit, 1);
        let report: WindowedPlayerRunReport =
            serde_json::from_str(&fs::read_to_string(report_path).unwrap()).unwrap();
        assert_eq!(report.mode, WindowedPlayerMode::Windowed);
        assert_eq!(report.exit_reason, "native_host_required");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "native_window_host_required"));
    }

    #[test]
    fn runtime_cli_rejects_removed_default_game_entrypoint() {
        let exit = run_from_args(["run-default-game", "--package", "runtime-package"]);
        assert_eq!(exit, 2);
    }

    #[test]
    fn runtime_cli_runs_minimal_package_one_frame() {
        let root = temp_root("one-frame");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let report_path = root.join("reports").join("runtime-run-report.json");

        let exit = run_from_args([
            "--package",
            package.to_str().unwrap(),
            "--mode",
            "headless",
            "--frames",
            "1",
            "--report",
            report_path.to_str().unwrap(),
        ]);

        assert_eq!(exit, 0);
        let text = fs::read_to_string(report_path).unwrap();
        let report: WindowedPlayerRunReport = serde_json::from_str(&text).unwrap();
        assert_eq!(report.counters.frames_completed, 1);
        assert_eq!(report.exit_reason, "completed");
    }

    #[test]
    fn runtime_cli_writes_runtime_run_report() {
        let root = temp_root("writes-report");
        let package = write_minimal_runtime_package(&root, "runtime-package");
        let report_path = root.join("reports").join("runtime-run-report.json");

        let exit = run_from_args([
            "--package",
            package.to_str().unwrap(),
            "--mode",
            "headless",
            "--frames",
            "2",
            "--report",
            report_path.to_str().unwrap(),
        ]);

        assert_eq!(exit, 0);
        let report: WindowedPlayerRunReport =
            serde_json::from_str(&fs::read_to_string(report_path).unwrap()).unwrap();
        assert_eq!(report.schema_version, "windowed-player-run-report.v1");
        assert_eq!(report.mode, WindowedPlayerMode::HeadlessGate);
        assert_eq!(report.counters.frames_requested, 2);
        assert_eq!(report.counters.frames_completed, 2);
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("runtime-cli-{name}-{stamp}"))
    }

    fn write_minimal_runtime_package(root: &Path, name: &str) -> PathBuf {
        let package_dir = root.join(name);
        fs::create_dir_all(package_dir.join("scenes")).unwrap();
        fs::create_dir_all(package_dir.join("assets")).unwrap();
        fs::create_dir_all(package_dir.join("rules")).unwrap();
        fs::create_dir_all(package_dir.join("input")).unwrap();
        fs::write(
            package_dir.join("manifest.json"),
            r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "project-runtime-cli-test",
    "name": "Runtime CLI Test",
    "version": "0.0.2",
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
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.none", "mappingCount": 1 },
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
    "id": "entity-player",
    "name": "Player",
    "kind": "player",
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
  "defaultMappingId": "input.none",
  "mappings": [{ "id": "input.none", "path": "input/input.none.json", "enabled": true }]
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input.none.json"),
            serde_json::to_string_pretty(&engine_input::InputMappingAsset::explicit_empty(
                "input.none",
            ))
            .unwrap(),
        )
        .unwrap();
        package_dir
    }

    fn write_packaged_entrypoint_fixture(root: &Path, user_frame_limit: Option<u64>) -> PathBuf {
        fs::create_dir_all(root).unwrap();
        let executable = root.join("ComplexShooter.exe");
        fs::write(&executable, b"test-entrypoint").unwrap();
        let runtime_package = write_minimal_runtime_package(&root.join("data"), "runtime_package");
        let executable_bytes = fs::read(&executable).unwrap();
        let runtime_manifest_bytes = fs::read(runtime_package.join("manifest.json")).unwrap();
        let files = vec![
            ReleasePackageFile {
                path: "ComplexShooter.exe".to_string(),
                size: executable_bytes.len() as u64,
                sha256: sha256_prefixed(&executable_bytes),
                roles: vec![
                    ReleasePackageFileRole::Entrypoint,
                    ReleasePackageFileRole::Runtime,
                ],
            },
            ReleasePackageFile {
                path: "data/runtime_package/manifest.json".to_string(),
                size: runtime_manifest_bytes.len() as u64,
                sha256: sha256_prefixed(&runtime_manifest_bytes),
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
            launch: ReleasePackageLaunch { user_frame_limit },
            entrypoint: "ComplexShooter.exe".to_string(),
            runtime_package: "data/runtime_package".to_string(),
            runtime_content_hash: format!("sha256:{}", "a".repeat(64)),
            release_payload_hash: release_payload_hash(&files),
            files,
        };
        fs::write(
            root.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        executable
    }
}
