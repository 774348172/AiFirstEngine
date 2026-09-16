use editor_core::{
    command_for_test, verify_release_package_directory, CommandStatus, ReleasePackageReport,
    ReleasePackageReportLevel, ReleasePackageStatus, RELEASE_PACKAGE_REPORT_RELATIVE_PATH,
};
use editor_ui_model::UiCommandPayload;
use engine_runtime::atomic_file_replace::atomic_file_replace;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::release_package_manifest::{
    ReleasePackageFileRole, ReleasePackageManifest, RELEASE_PACKAGE_MANIFEST_FILE_NAME,
};
use engine_runtime::runtime_package::load_runtime_package;
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationReport,
    ExportedPlayerProcessVerificationRequest, ExportedPlayerProcessVerificationStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const COMPLEX_SHOOTER_RELEASE_PACKAGE_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-release-package-e2e-report.v1";
pub const COMPLEX_SHOOTER_RELEASE_PACKAGE_SCENARIO_ID: &str =
    "complex-shooter-release-package-polish-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterReleasePackageStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterReleasePackageMetrics {
    pub open_project_committed: bool,
    pub release_profile_visible: bool,
    pub release_command_present: bool,
    pub first_build_committed: bool,
    pub second_build_committed: bool,
    pub report_panel_provider_present: bool,
    pub branded_entrypoint_present: bool,
    pub metadata_readback_passed: bool,
    pub icon_readback_passed: bool,
    pub relative_manifest_passed: bool,
    pub file_roles_passed: bool,
    pub runtime_formal_load_passed: bool,
    pub builder_process_verification_passed: bool,
    pub explicit_verification_passed: bool,
    pub no_arg_launch_passed: bool,
    pub user_report_off_passed: bool,
    pub stale_replacement_passed: bool,
    pub deterministic_payload_hash_passed: bool,
    pub sample_source_unchanged: bool,
    pub runtime_template_unchanged: bool,
    pub first_payload_hash: String,
    pub second_payload_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterReleasePackageReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterReleasePackageStatus,
    pub source_project: String,
    pub owned_project: String,
    pub output_dir: String,
    pub metrics: ComplexShooterReleasePackageMetrics,
    pub release_report: Option<ReleasePackageReport>,
    pub explicit_verification: Option<ExportedPlayerProcessVerificationReport>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterReleasePackageRequest {
    pub source_project: PathBuf,
    pub temp_root: PathBuf,
    pub player_executable: PathBuf,
    pub frame_limit: u64,
    pub timeout_ms: u64,
}

impl ComplexShooterReleasePackageRequest {
    pub fn new(
        source_project: impl Into<PathBuf>,
        temp_root: impl Into<PathBuf>,
        player_executable: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source_project: source_project.into(),
            temp_root: temp_root.into(),
            player_executable: player_executable.into(),
            frame_limit: 3,
            timeout_ms: 30_000,
        }
    }
}

pub fn run_complex_shooter_release_package_report(
    request: ComplexShooterReleasePackageRequest,
) -> ComplexShooterReleasePackageReport {
    let owned_project = request.temp_root.join("project");
    let output_dir = request.temp_root.join("release-final");
    let report_path = request
        .temp_root
        .join("reports/complex-shooter-release-package-e2e-report.json");
    let mut report = ComplexShooterReleasePackageReport {
        schema_version: COMPLEX_SHOOTER_RELEASE_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
        scenario_id: COMPLEX_SHOOTER_RELEASE_PACKAGE_SCENARIO_ID.to_string(),
        status: ComplexShooterReleasePackageStatus::Failed,
        source_project: request.source_project.display().to_string(),
        owned_project: owned_project.display().to_string(),
        output_dir: output_dir.display().to_string(),
        metrics: ComplexShooterReleasePackageMetrics::default(),
        release_report: None,
        explicit_verification: None,
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };
    let source_before = match tree_hashes(&request.source_project) {
        Ok(hashes) => hashes,
        Err(error) => return finish_failed(report, &report_path, error),
    };
    let player_package = request
        .player_executable
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if let Err(error) =
        editor_core::ProjectPlayerArtifact::ensure_built(&request.player_executable, player_package)
    {
        return finish_failed(report, &report_path, error.to_string());
    }
    let template_before = match fs::read(&request.player_executable) {
        Ok(bytes) => sha256_prefixed(&bytes),
        Err(error) => {
            return finish_failed(
                report,
                &report_path,
                format!("failed to read Runtime template: {error}"),
            );
        }
    };
    if let Err(error) = copy_tree(&request.source_project, &owned_project) {
        return finish_failed(report, &report_path, error);
    }

    let mut session = crate::complex_shooter_editor_session();
    let open = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: owned_project.display().to_string(),
    }));
    report.metrics.open_project_committed = open.status == CommandStatus::Committed;
    let build_model = session.build_ui_model().build_export;
    report.metrics.release_profile_visible = build_model.release_profile.is_some();
    report.metrics.release_command_present = build_model
        .commands
        .iter()
        .any(|command| command.command_id == "build_release_package" && command.enabled);

    let first = session.execute_build_release_package_for_test(
        request.player_executable.clone(),
        output_dir.clone(),
        ReleasePackageReportLevel::Trace,
        true,
    );
    report.metrics.first_build_committed = first.status == CommandStatus::Committed;
    let first_report = session.last_release_package_report().cloned();
    report.metrics.first_payload_hash = first_report
        .as_ref()
        .map(|report| report.release_payload_hash.clone())
        .unwrap_or_default();

    if output_dir.is_dir() {
        let _ = fs::write(output_dir.join("stale-from-previous-build.txt"), b"stale");
    }
    let second = session.execute_build_release_package_for_test(
        request.player_executable.clone(),
        output_dir.clone(),
        ReleasePackageReportLevel::Trace,
        true,
    );
    report.metrics.second_build_committed = second.status == CommandStatus::Committed;
    report.release_report = session.last_release_package_report().cloned();
    report.metrics.second_payload_hash = report
        .release_report
        .as_ref()
        .map(|report| report.release_payload_hash.clone())
        .unwrap_or_default();
    report.metrics.builder_process_verification_passed = report
        .release_report
        .as_ref()
        .is_some_and(|report| report.verification.explicit_process_passed);
    report.metrics.stale_replacement_passed =
        !output_dir.join("stale-from-previous-build.txt").exists();
    report.metrics.deterministic_payload_hash_passed =
        !report.metrics.first_payload_hash.is_empty()
            && report.metrics.first_payload_hash == report.metrics.second_payload_hash;

    let panel = session.build_ui_model().report_panel;
    report.metrics.report_panel_provider_present = panel
        .registry
        .descriptors
        .iter()
        .any(|descriptor| descriptor.provider_id == "build.release_package")
        && panel
            .reports
            .iter()
            .any(|entry| entry.provider_id == "build.release_package");

    if let Ok(verification) = verify_release_package_directory(&output_dir) {
        let entrypoint = output_dir.join(&verification.manifest.entrypoint);
        report.metrics.branded_entrypoint_present =
            verification.manifest.entrypoint == "ComplexShooter.exe" && entrypoint.is_file();
        report.metrics.metadata_readback_passed = verification.resource_readback.product_name
            == "Complex Shooter"
            && verification.resource_readback.product_version == "1.0.0"
            && verification.resource_readback.company_name == "AI First Engine Studio";
        report.metrics.icon_readback_passed =
            verification.resource_readback.icon_sizes == vec![16, 32, 48, 64, 128, 256];
        report.metrics.relative_manifest_passed =
            is_relative_release_path(&verification.manifest.entrypoint)
                && is_relative_release_path(&verification.manifest.runtime_package);
        report.metrics.file_roles_passed = release_roles_pass(&verification.manifest);
        report.metrics.runtime_formal_load_passed =
            load_runtime_package(&output_dir.join(&verification.manifest.runtime_package))
                .diagnostics
                .is_ok();
    } else {
        report
            .diagnostics
            .push("final release directory verification failed".to_string());
    }

    let explicit_report_path = request
        .temp_root
        .join("reports/explicit-release-process-verification.json");
    let explicit = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
        exported_package_dir: output_dir.clone(),
        mode: "headless-gate".to_string(),
        frame_limit: request.frame_limit.max(1),
        report_path: Some(explicit_report_path),
        timeout_ms: request.timeout_ms,
        screenshot: false,
        screenshot_path: None,
    });
    report.metrics.explicit_verification_passed = explicit.status
        == ExportedPlayerProcessVerificationStatus::Passed
        && explicit.process_exit_code == Some(0)
        && explicit.child_player_exit_code == Some(0);
    report.explicit_verification = Some(explicit);

    match run_finite_no_arg_release_copy(&output_dir, &request.temp_root, request.timeout_ms) {
        Ok(status) => report.metrics.no_arg_launch_passed = status.success(),
        Err(error) => report.diagnostics.push(error),
    }
    report.metrics.user_report_off_passed = !output_dir.join("reports").exists()
        && !output_dir.join("data/runtime_package/reports").exists()
        && !request.temp_root.join("noarg-release/reports").exists()
        && !request
            .temp_root
            .join("noarg-release/data/runtime_package/reports")
            .exists();

    report.metrics.sample_source_unchanged = tree_hashes(&request.source_project)
        .is_ok_and(|source_after| source_after == source_before);
    report.metrics.runtime_template_unchanged = fs::read(&request.player_executable)
        .is_ok_and(|bytes| sha256_prefixed(&bytes) == template_before);

    let passed = report.metrics.open_project_committed
        && report.metrics.release_profile_visible
        && report.metrics.release_command_present
        && report.metrics.first_build_committed
        && report.metrics.second_build_committed
        && report.metrics.report_panel_provider_present
        && report.metrics.branded_entrypoint_present
        && report.metrics.metadata_readback_passed
        && report.metrics.icon_readback_passed
        && report.metrics.relative_manifest_passed
        && report.metrics.file_roles_passed
        && report.metrics.runtime_formal_load_passed
        && report.metrics.builder_process_verification_passed
        && report.metrics.explicit_verification_passed
        && report.metrics.no_arg_launch_passed
        && report.metrics.user_report_off_passed
        && report.metrics.stale_replacement_passed
        && report.metrics.deterministic_payload_hash_passed
        && report.metrics.sample_source_unchanged
        && report.metrics.runtime_template_unchanged
        && report
            .release_report
            .as_ref()
            .is_some_and(|release| release.status == ReleasePackageStatus::Success)
        && owned_project
            .join(RELEASE_PACKAGE_REPORT_RELATIVE_PATH)
            .is_file();
    if passed {
        report.status = ComplexShooterReleasePackageStatus::Passed;
    } else {
        report
            .next_actions
            .push("inspect_release_package_e2e_metrics".to_string());
    }
    write_report(&report_path, &report);
    report
}

pub(crate) fn run_finite_no_arg_release_copy(
    output_dir: &Path,
    temp_root: &Path,
    timeout_ms: u64,
) -> Result<ExitStatus, String> {
    let package = temp_root.join("noarg-release");
    copy_tree(output_dir, &package)?;
    let manifest_path = package.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME);
    let mut manifest: ReleasePackageManifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    manifest.launch.user_frame_limit = Some(2);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let entrypoint = package.join(&manifest.entrypoint);
    let mut child = Command::new(&entrypoint)
        .current_dir(temp_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to spawn zero-argument entrypoint: {error}"))?;
    wait_with_timeout(&mut child, Duration::from_millis(timeout_ms.max(1)))
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<ExitStatus, String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("zero-argument entrypoint exceeded {timeout:?}"));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "failed while waiting for zero-argument entrypoint: {error}"
                ));
            }
        }
    }
}

fn release_roles_pass(manifest: &ReleasePackageManifest) -> bool {
    manifest.files.iter().any(|file| {
        file.path == manifest.entrypoint
            && file.roles.contains(&ReleasePackageFileRole::Entrypoint)
            && file.roles.contains(&ReleasePackageFileRole::Runtime)
    }) && manifest.files.iter().any(|file| {
        file.path
            .starts_with(&format!("{}/", manifest.runtime_package))
            && file.roles.contains(&ReleasePackageFileRole::RuntimePayload)
    })
}

fn is_relative_release_path(path: &str) -> bool {
    !path.is_empty()
        && !Path::new(path).is_absolute()
        && !path.contains(':')
        && !path.split('/').any(|component| component == "..")
}

fn tree_hashes(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut hashes = BTreeMap::new();
    collect_tree_hashes(root, root, &mut hashes)?;
    Ok(hashes)
}

fn collect_tree_hashes(
    root: &Path,
    directory: &Path,
    hashes: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            collect_tree_hashes(root, &entry.path(), hashes)?;
        } else if file_type.is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(entry.path()).map_err(|error| error.to_string())?;
            hashes.insert(relative, sha256_prefixed(&bytes));
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            return Err(format!(
                "test source contains symlink: {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            copy_tree(&entry.path(), &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn finish_failed(
    mut report: ComplexShooterReleasePackageReport,
    report_path: &Path,
    error: String,
) -> ComplexShooterReleasePackageReport {
    report.diagnostics.push(error);
    report
        .next_actions
        .push("inspect_release_package_e2e_setup".to_string());
    write_report(report_path, &report);
    report
}

fn write_report(path: &Path, report: &ComplexShooterReleasePackageReport) {
    if let Ok(bytes) = serde_json::to_vec_pretty(report) {
        let _ = atomic_file_replace(path, &bytes);
    }
}
