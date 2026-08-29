use crate::project_runtime_player_staging::{
    ProjectRuntimePlayerDependencyIdentity, ProjectRuntimePlayerStagingPlan,
    ProjectRuntimeProductionStaging,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor;
use engine_runtime::runtime_package::RuntimeProjectModuleRef;
use runtime_cli::{
    run_bounded_child_process, BoundedChildProcessExitReason, BoundedChildProcessRequest,
    BoundedChildProcessResult,
};
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const PROJECT_PLAYER_ARTIFACT_SCHEMA_VERSION: &str = "project-player-artifact.v1";
pub const PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REQUEST_SCHEMA_VERSION: &str =
    "project-runtime-player-artifact-build-request.v1";
pub const PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REPORT_SCHEMA_VERSION: &str =
    "project-runtime-player-artifact-build-report.v1";

const DEFAULT_BUILD_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_CAPTURE_LIMIT_BYTES: usize = 128 * 1024;
const ENGINE_PLAYER_RUNTIME_CRATES: [&str; 4] = [
    "engine_input",
    "engine_runtime",
    "runtime_cli",
    "runtime_player_winit",
];
static ARTIFACT_BUILD_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlayerArtifact {
    pub schema_version: String,
    pub executable_path: PathBuf,
    pub module_descriptor: ProjectRuntimeModuleDescriptor,
    pub source_executable_hash: String,
    pub build_report_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimePlayerArtifactBuildRequest {
    pub schema_version: String,
    pub project_root: PathBuf,
    pub engine_sdk_root: PathBuf,
    pub build_root: PathBuf,
    pub expected_module: RuntimeProjectModuleRef,
    pub cargo_executable: Option<PathBuf>,
    pub step_timeout_ms: u64,
    pub capture_limit_bytes: usize,
}

impl ProjectRuntimePlayerArtifactBuildRequest {
    pub fn new(
        project_root: impl Into<PathBuf>,
        engine_sdk_root: impl Into<PathBuf>,
        expected_module: RuntimeProjectModuleRef,
    ) -> Self {
        Self {
            schema_version: PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REQUEST_SCHEMA_VERSION
                .to_string(),
            project_root: project_root.into(),
            engine_sdk_root: engine_sdk_root.into(),
            build_root: default_project_runtime_player_build_root(),
            expected_module,
            cargo_executable: None,
            step_timeout_ms: DEFAULT_BUILD_TIMEOUT_MS,
            capture_limit_bytes: DEFAULT_CAPTURE_LIMIT_BYTES,
        }
    }

    pub fn with_build_root(mut self, build_root: impl Into<PathBuf>) -> Self {
        self.build_root = build_root.into();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimePlayerArtifactBuildStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimePlayerArtifactBuildDiagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimePlayerArtifactBuildStep {
    pub stage: String,
    pub command: Vec<String>,
    pub timeout_ms: u64,
    pub process: BoundedChildProcessResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimePlayerArtifactBuildReport {
    pub schema_version: String,
    pub status: ProjectRuntimePlayerArtifactBuildStatus,
    pub project_root: String,
    pub engine_sdk_root: String,
    pub build_root: String,
    pub artifact_root: Option<String>,
    pub source_digest: Option<String>,
    pub engine_sdk_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staging_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_manifest_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_dependency_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub normalized_dependencies: Vec<ProjectRuntimePlayerDependencyIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_lock_digest: Option<String>,
    pub cache_status: String,
    pub host_manifest_path: Option<String>,
    pub executable_path: Option<String>,
    pub expected_module: RuntimeProjectModuleRef,
    pub actual_descriptor: Option<ProjectRuntimeModuleDescriptor>,
    pub executable_hash: Option<String>,
    pub build_report_path: Option<String>,
    pub cleanup_status: String,
    pub steps: Vec<ProjectRuntimePlayerArtifactBuildStep>,
    pub artifact: Option<ProjectPlayerArtifact>,
    pub diagnostics: Vec<ProjectRuntimePlayerArtifactBuildDiagnostic>,
    pub next_actions: Vec<String>,
}

impl ProjectPlayerArtifact {
    pub fn debug_executable_path(binary_name: &str) -> PathBuf {
        workspace_debug_executable(binary_name)
    }

    pub fn ensure_built(
        executable_path: &Path,
        cargo_package: &str,
    ) -> Result<(), ProjectPlayerArtifactError> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        let workspace_root = workspace_root();
        let result = run_bounded_child_process(BoundedChildProcessRequest {
            executable: PathBuf::from(cargo),
            args: vec![
                OsString::from("build"),
                OsString::from("-p"),
                OsString::from(cargo_package),
            ],
            current_dir: workspace_root,
            environment: Vec::new(),
            timeout: Duration::from_secs(300),
            stdout_capture_limit_bytes: 128 * 1024,
            stderr_capture_limit_bytes: 128 * 1024,
            priority: runtime_cli::BoundedChildProcessPriority::Normal,
        });
        if result.exit_reason != BoundedChildProcessExitReason::Completed
            || result.exit_code != Some(0)
            || !executable_path.is_file()
        {
            return Err(ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_build_failed",
                format!(
                    "Project Player build failed for package '{}' ({:?}, exit {:?}): {}",
                    cargo_package, result.exit_reason, result.exit_code, result.stderr_summary
                ),
            ));
        }
        Ok(())
    }

    pub fn inspect(
        executable_path: impl Into<PathBuf>,
        expected: &RuntimeProjectModuleRef,
    ) -> Result<Self, ProjectPlayerArtifactError> {
        inspect_with_process(executable_path.into(), expected).map(|(artifact, _)| artifact)
    }

    pub fn build_project_rust(
        request: ProjectRuntimePlayerArtifactBuildRequest,
    ) -> ProjectRuntimePlayerArtifactBuildReport {
        build_project_rust_report(request)
    }
}

fn inspect_with_process(
    executable_path: PathBuf,
    expected: &RuntimeProjectModuleRef,
) -> Result<(ProjectPlayerArtifact, BoundedChildProcessResult), ProjectPlayerArtifactError> {
    if !executable_path.is_file() {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_missing",
            format!(
                "Project Player executable is missing: {}",
                executable_path.display()
            ),
        ));
    }
    let result = run_bounded_child_process(BoundedChildProcessRequest {
        executable: executable_path.clone(),
        args: vec![OsString::from("--describe-project-runtime-module")],
        current_dir: executable_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        environment: Vec::new(),
        timeout: Duration::from_secs(10),
        stdout_capture_limit_bytes: 64 * 1024,
        stderr_capture_limit_bytes: 64 * 1024,
        priority: runtime_cli::BoundedChildProcessPriority::Normal,
    });
    if result.exit_reason != BoundedChildProcessExitReason::Completed || result.exit_code != Some(0)
    {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_descriptor_query_failed",
            format!(
                "Project Player descriptor query failed ({:?}, exit {:?}): {}",
                result.exit_reason, result.exit_code, result.stderr_summary
            ),
        ));
    }
    let descriptor =
        serde_json::from_str::<ProjectRuntimeModuleDescriptor>(result.stdout_summary.trim())
            .map_err(|error| {
                ProjectPlayerArtifactError::new(
                    "project_runtime.player_artifact_descriptor_invalid",
                    format!("Project Player returned an invalid module descriptor: {error}"),
                )
            })?;
    if descriptor.module_id != expected.module_id
        || descriptor.interface_version != expected.interface_version
        || descriptor.aot_content_digest != expected.aot_content_digest
    {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_mismatch",
            format!(
                "Project Player descriptor {:?} does not match RuntimePackage descriptor {:?}.",
                descriptor, expected
            ),
        ));
    }
    let executable_bytes = fs::read(&executable_path).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_read_failed",
            format!("Failed to read Project Player executable: {error}"),
        )
    })?;
    Ok((
        ProjectPlayerArtifact {
            schema_version: PROJECT_PLAYER_ARTIFACT_SCHEMA_VERSION.to_string(),
            executable_path,
            module_descriptor: descriptor,
            source_executable_hash: sha256_prefixed(&executable_bytes),
            build_report_path: None,
        },
        result,
    ))
}

fn build_project_rust_report(
    request: ProjectRuntimePlayerArtifactBuildRequest,
) -> ProjectRuntimePlayerArtifactBuildReport {
    let mut report = ProjectRuntimePlayerArtifactBuildReport {
        schema_version: PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REPORT_SCHEMA_VERSION.to_string(),
        status: ProjectRuntimePlayerArtifactBuildStatus::Failed,
        project_root: request.project_root.display().to_string(),
        engine_sdk_root: request.engine_sdk_root.display().to_string(),
        build_root: request.build_root.display().to_string(),
        artifact_root: None,
        source_digest: None,
        engine_sdk_digest: None,
        staging_policy: None,
        normalized_manifest_digest: None,
        normalized_dependency_digest: None,
        normalized_dependencies: Vec::new(),
        trusted_lock_digest: None,
        cache_status: "not_checked".to_string(),
        host_manifest_path: None,
        executable_path: None,
        expected_module: request.expected_module.clone(),
        actual_descriptor: None,
        executable_hash: None,
        build_report_path: None,
        cleanup_status: "not_started".to_string(),
        steps: Vec::new(),
        artifact: None,
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };
    let mut staging_root = None;
    let result = build_project_rust_inner(&request, &mut report, &mut staging_root);
    if let Err(error) = result {
        report.diagnostics.push(build_diagnostic(&error));
        report.next_actions.push(error.next_action);
        report.build_report_path = Some(
            request
                .build_root
                .join("last-project-runtime-player-build-report.json")
                .display()
                .to_string(),
        );
        if let Some(staging_root) = staging_root {
            report.cleanup_status = match fs::remove_dir_all(&staging_root) {
                Ok(()) => "failed_build_staging_removed".to_string(),
                Err(cleanup_error) => {
                    report.diagnostics.push(ProjectRuntimePlayerArtifactBuildDiagnostic {
                        code: "project_runtime.player_artifact_cleanup_failed".to_string(),
                        message: cleanup_error.to_string(),
                        path: Some(staging_root.display().to_string()),
                        next_action: "Close processes using the artifact staging directory and remove it manually."
                            .to_string(),
                    });
                    "failed_build_staging_retained".to_string()
                }
            };
        }
    }
    write_build_report(&request, &mut report);
    report
}

fn build_project_rust_inner(
    request: &ProjectRuntimePlayerArtifactBuildRequest,
    report: &mut ProjectRuntimePlayerArtifactBuildReport,
    staging_slot: &mut Option<PathBuf>,
) -> Result<(), ProjectPlayerArtifactError> {
    if request.schema_version != PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REQUEST_SCHEMA_VERSION {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_request_schema_invalid",
            format!(
                "Unsupported build request schema: {}",
                request.schema_version
            ),
        ));
    }
    let source =
        ProjectRuntimeProductionStaging::plan(&request.project_root, &request.engine_sdk_root)
            .map_err(|error| ProjectPlayerArtifactError::new(error.code, error.message))?;
    if source.manifest.runtime_module.module_id != request.expected_module.module_id
        || source.manifest.runtime_module.interface_version
            != request.expected_module.interface_version
    {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_manifest_mismatch",
            "Project manifest runtime module identity does not match the RuntimePackage.",
        ));
    }

    fs::create_dir_all(&request.build_root).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_build_root_failed",
            format!("Artifact build root cannot be created: {error}"),
        )
    })?;
    let build_root = request.build_root.canonicalize().map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_build_root_failed",
            format!("Artifact build root cannot be resolved: {error}"),
        )
    })?;
    let project_root = request.project_root.canonicalize().map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_project_root_failed",
            format!("Project root cannot be resolved: {error}"),
        )
    })?;
    if build_root.starts_with(&project_root) || build_root.starts_with(&source.sdk_root) {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_build_root_not_isolated",
            "Artifact build root must be outside the project root and trusted Engine SDK root.",
        ));
    }
    report.build_root = build_root.display().to_string();
    report.staging_policy = Some("project-runtime-player-production-staging.v1".to_string());
    report.normalized_manifest_digest = Some(source.normalized_manifest_digest.clone());
    report.normalized_dependency_digest = Some(source.normalized_dependency_digest.clone());
    report.normalized_dependencies = source.normalized_dependencies.clone();
    report.trusted_lock_digest = Some(source.trusted_lock_digest.clone());

    let source_digest = runtime_module_source_digest(&project_root)?;
    report.source_digest = Some(source_digest.clone());
    let engine_sdk_digest = engine_sdk_source_digest(&source.sdk_root)?;
    report.engine_sdk_digest = Some(engine_sdk_digest.clone());
    let artifact_key = sha256_prefixed(
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            source.manifest.project_id,
            source_digest,
            engine_sdk_digest,
            source.sdk_root.display(),
            source.runtime_player_winit_root.display(),
            request.expected_module.interface_version,
            request.expected_module.aot_content_digest,
            source.normalized_manifest_digest,
            source.normalized_dependency_digest,
            source.trusted_lock_digest
        )
        .as_bytes(),
    );
    let artifact_key_hex = artifact_key.trim_start_matches("sha256:");
    let artifact_root = build_root.join(format!("a-{}", &artifact_key_hex[..24]));
    let host_manifest = artifact_root.join("Host").join("Cargo.toml");
    let executable = generated_host_executable(&artifact_root);
    let build_report_path = artifact_root.join("project-runtime-player-build-report.json");
    report.artifact_root = Some(artifact_root.display().to_string());
    report.host_manifest_path = Some(host_manifest.display().to_string());
    report.executable_path = Some(executable.display().to_string());
    report.build_report_path = Some(build_report_path.display().to_string());

    if executable.is_file() {
        match inspect_with_process(executable.clone(), &request.expected_module) {
            Ok((mut artifact, process)) => {
                report.steps.push(ProjectRuntimePlayerArtifactBuildStep {
                    stage: "describe_cached_artifact".to_string(),
                    command: vec![
                        executable.display().to_string(),
                        "--describe-project-runtime-module".to_string(),
                    ],
                    timeout_ms: 10_000,
                    process,
                });
                artifact.build_report_path = Some(build_report_path);
                complete_success_report(report, artifact, "hit");
                report.cleanup_status = "cache_reused".to_string();
                return Ok(());
            }
            Err(error) => {
                report
                    .diagnostics
                    .push(ProjectRuntimePlayerArtifactBuildDiagnostic {
                        code: "project_runtime.player_artifact_cache_invalidated".to_string(),
                        message: format!(
                            "Cached Project Player failed validation and will be rebuilt: {}",
                            error.message
                        ),
                        path: Some(executable.display().to_string()),
                        next_action:
                            "Inspect the cached artifact validation failure if rebuilding repeats."
                                .to_string(),
                    });
                fs::remove_dir_all(&artifact_root).map_err(|error| {
                    ProjectPlayerArtifactError::new(
                        "project_runtime.player_artifact_stale_cleanup_failed",
                        format!("Stale artifact cannot be removed: {error}"),
                    )
                })?;
            }
        }
    }
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root).map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_stale_cleanup_failed",
                format!("Incomplete artifact cannot be removed: {error}"),
            )
        })?;
    }

    let sequence = ARTIFACT_BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let staging_root = build_root.join(format!(".s-{}-{sequence}", std::process::id()));
    fs::create_dir(&staging_root).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_staging_failed",
            format!("Artifact staging root cannot be created: {error}"),
        )
    })?;
    *staging_slot = Some(staging_root.clone());
    ProjectRuntimeProductionStaging::stage(&project_root, &staging_root, &source)
        .map_err(|error| ProjectPlayerArtifactError::new(error.code, error.message))?;
    write_generated_host(&staging_root, &source)?;

    let cargo = request
        .cargo_executable
        .clone()
        .or_else(|| std::env::var_os("CARGO").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let timeout_ms = request.step_timeout_ms.max(1).min(600_000);
    let capture_limit = request.capture_limit_bytes.max(1).min(1024 * 1024);
    let target_root = staging_root.join("target");
    let environment = vec![
        (
            OsString::from("CARGO_TARGET_DIR"),
            target_root.clone().into_os_string(),
        ),
        (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
        (OsString::from("CARGO_INCREMENTAL"), OsString::from("0")),
        (
            OsString::from("CARGO_PROFILE_DEV_DEBUG"),
            OsString::from("0"),
        ),
        (
            OsString::from("AIFE_PROJECT_RUNTIME_AOT_DIGEST"),
            OsString::from(&request.expected_module.aot_content_digest),
        ),
    ];
    run_required_cargo_step(
        report,
        "validate_project_runtime_format",
        &cargo,
        [
            "fmt",
            "--manifest-path",
            "Cargo.toml",
            "--all",
            "--",
            "--check",
        ],
        &staging_root.join("RuntimeModuleBuild"),
        &environment,
        timeout_ms,
        capture_limit,
    )?;
    if !source.has_source_lock {
        run_required_cargo_step(
            report,
            "lock_project_runtime_dependencies",
            &cargo,
            ["generate-lockfile", "--offline"],
            &staging_root.join("RuntimeModuleBuild"),
            &environment,
            timeout_ms,
            capture_limit,
        )?;
    }
    run_required_cargo_step(
        report,
        "validate_project_runtime_tests_compile",
        &cargo,
        ["test", "--no-run", "--locked", "--offline"],
        &staging_root.join("RuntimeModuleBuild"),
        &environment,
        timeout_ms,
        capture_limit,
    )?;
    run_required_cargo_step(
        report,
        "lock_project_runtime_host_dependencies",
        &cargo,
        ["generate-lockfile", "--offline"],
        &staging_root.join("Host"),
        &environment,
        timeout_ms,
        capture_limit,
    )?;
    run_required_cargo_step(
        report,
        "build_project_runtime_dev_host",
        &cargo,
        [
            "build",
            "--manifest-path",
            "Cargo.toml",
            "--locked",
            "--offline",
        ],
        &staging_root.join("Host"),
        &environment,
        timeout_ms,
        capture_limit,
    )?;

    let staged_executable = generated_host_executable(&staging_root);
    let (staged_artifact, describe_process) =
        inspect_with_process(staged_executable.clone(), &request.expected_module)?;
    report.steps.push(ProjectRuntimePlayerArtifactBuildStep {
        stage: "describe_project_runtime_artifact".to_string(),
        command: vec![
            staged_executable.display().to_string(),
            "--describe-project-runtime-module".to_string(),
        ],
        timeout_ms: 10_000,
        process: describe_process,
    });

    publish_validated_artifact_staging(&staging_root, &artifact_root).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_publish_failed",
            format!("Validated artifact cannot be published atomically: {error}"),
        )
    })?;
    *staging_slot = None;
    let mut artifact = ProjectPlayerArtifact {
        executable_path: generated_host_executable(&artifact_root),
        build_report_path: Some(build_report_path),
        ..staged_artifact
    };
    artifact.executable_path = generated_host_executable(&artifact_root);
    complete_success_report(report, artifact, "rebuilt");
    report.cleanup_status = "staging_published".to_string();
    Ok(())
}

fn publish_validated_artifact_staging(
    staging_root: &Path,
    artifact_root: &Path,
) -> std::io::Result<()> {
    const RETRY_DELAYS_MS: [u64; 8] = [0, 25, 50, 100, 200, 400, 800, 1_000];
    let mut last_error = None;
    for delay_ms in RETRY_DELAYS_MS {
        if delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
        match fs::rename(staging_root, artifact_root) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.expect("publish retry loop records a permission error"))
}

fn complete_success_report(
    report: &mut ProjectRuntimePlayerArtifactBuildReport,
    artifact: ProjectPlayerArtifact,
    cache_status: &str,
) {
    report.status = ProjectRuntimePlayerArtifactBuildStatus::Success;
    report.cache_status = cache_status.to_string();
    report.actual_descriptor = Some(artifact.module_descriptor.clone());
    report.executable_hash = Some(artifact.source_executable_hash.clone());
    report.executable_path = Some(artifact.executable_path.display().to_string());
    report.artifact = Some(artifact);
    report.next_actions.clear();
}

fn run_required_cargo_step<const N: usize>(
    report: &mut ProjectRuntimePlayerArtifactBuildReport,
    stage: &str,
    cargo: &Path,
    args: [&str; N],
    current_dir: &Path,
    environment: &[(OsString, OsString)],
    timeout_ms: u64,
    capture_limit: usize,
) -> Result<(), ProjectPlayerArtifactError> {
    let command = std::iter::once(cargo.display().to_string())
        .chain(args.iter().map(|value| value.to_string()))
        .collect::<Vec<_>>();
    let process = run_bounded_child_process(BoundedChildProcessRequest {
        executable: cargo.to_path_buf(),
        args: args.iter().map(OsString::from).collect(),
        current_dir: current_dir.to_path_buf(),
        environment: environment.to_vec(),
        timeout: Duration::from_millis(timeout_ms),
        stdout_capture_limit_bytes: capture_limit,
        stderr_capture_limit_bytes: capture_limit,
        priority: runtime_cli::BoundedChildProcessPriority::Normal,
    });
    let passed = process.exit_reason == BoundedChildProcessExitReason::Completed
        && process.exit_code == Some(0)
        && process.reader_join_error.is_none();
    let failure_summary = [process.stderr_summary.trim(), process.stdout_summary.trim()]
        .into_iter()
        .filter(|summary| !summary.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    report.steps.push(ProjectRuntimePlayerArtifactBuildStep {
        stage: stage.to_string(),
        command,
        timeout_ms,
        process,
    });
    if !passed {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_build_step_failed",
            format!("Artifact build stage '{stage}' failed: {failure_summary}"),
        ));
    }
    Ok(())
}

fn write_generated_host(
    artifact_root: &Path,
    source: &ProjectRuntimePlayerStagingPlan,
) -> Result<(), ProjectPlayerArtifactError> {
    let host_root = artifact_root.join("Host");
    let source_root = host_root.join("src");
    fs::create_dir_all(&source_root).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_host_write_failed",
            format!("Generated host source directory cannot be created: {error}"),
        )
    })?;

    let mut package = toml::map::Map::new();
    package.insert(
        "name".to_string(),
        toml::Value::String("ai_project_runtime_player".to_string()),
    );
    package.insert(
        "version".to_string(),
        toml::Value::String("0.0.3".to_string()),
    );
    package.insert(
        "edition".to_string(),
        toml::Value::String("2021".to_string()),
    );
    package.insert("publish".to_string(), toml::Value::Boolean(false));

    let mut project_dependency = toml::map::Map::new();
    project_dependency.insert(
        "package".to_string(),
        toml::Value::String(source.manifest.runtime_module.cargo_package.clone()),
    );
    project_dependency.insert(
        "path".to_string(),
        toml::Value::String("../RuntimeModuleBuild".to_string()),
    );
    let mut runtime_cli_dependency = toml::map::Map::new();
    runtime_cli_dependency.insert(
        "path".to_string(),
        toml::Value::String(source.runtime_cli_root.display().to_string()),
    );
    runtime_cli_dependency.insert(
        "features".to_string(),
        toml::Value::Array(vec![toml::Value::String("real-window".to_string())]),
    );
    let mut engine_runtime_dependency = toml::map::Map::new();
    engine_runtime_dependency.insert(
        "path".to_string(),
        toml::Value::String(
            source
                .sdk_root
                .join("crates/engine_runtime")
                .display()
                .to_string(),
        ),
    );
    let mut dependencies = toml::map::Map::new();
    dependencies.insert(
        "project_runtime".to_string(),
        toml::Value::Table(project_dependency),
    );
    dependencies.insert(
        "runtime_cli".to_string(),
        toml::Value::Table(runtime_cli_dependency),
    );
    dependencies.insert(
        "engine_runtime".to_string(),
        toml::Value::Table(engine_runtime_dependency),
    );
    dependencies.insert(
        "serde_json".to_string(),
        toml::Value::String("1".to_string()),
    );

    let mut manifest = toml::map::Map::new();
    manifest.insert("package".to_string(), toml::Value::Table(package));
    manifest.insert("dependencies".to_string(), toml::Value::Table(dependencies));
    let manifest_text = toml::to_string(&toml::Value::Table(manifest)).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_host_manifest_invalid",
            format!("Generated host manifest cannot be encoded: {error}"),
        )
    })?;
    fs::write(host_root.join("Cargo.toml"), manifest_text).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_host_write_failed",
            format!("Generated host manifest cannot be written: {error}"),
        )
    })?;
    fs::write(host_root.join("Cargo.lock"), &source.dependency_lock_bytes).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_host_write_failed",
            format!("Generated host lock seed cannot be written: {error}"),
        )
    })?;
    fs::write(
        source_root.join("main.rs"),
        r#"use std::sync::Arc;

fn main() {
    // SAFETY: the statically linked project exports a process-static API table.
    let api = unsafe { *project_runtime::aife_project_runtime_entry_v1() };
    let linked_modules = match engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_api(api) {
        Ok(linked_modules) => linked_modules,
        Err(error) => {
            eprintln!("project runtime link failed: {error}");
            std::process::exit(1);
        }
    };
    if std::env::args().nth(1).as_deref() == Some("--describe-project-runtime-module") {
        let descriptor = match linked_modules.only_descriptor() {
            Ok(descriptor) => descriptor,
            Err(error) => {
                eprintln!("project runtime descriptor query failed: {error}");
                std::process::exit(1);
            }
        };
        match serde_json::to_string(descriptor) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("project runtime descriptor serialization failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    std::process::exit(runtime_cli::run_from_env_with_linked_modules(Arc::new(
        linked_modules,
    )));
}
"#,
    )
    .map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_host_write_failed",
            format!("Generated host source cannot be written: {error}"),
        )
    })
}

pub(crate) fn runtime_module_source_digest(
    project_root: &Path,
) -> Result<String, ProjectPlayerArtifactError> {
    let runtime_root = project_root.join("RuntimeModule");
    let mut files = Vec::new();
    collect_runtime_module_sources(&runtime_root, &runtime_root, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_source_empty",
            "Project contains no production staging source files.",
        ));
    }
    let mut digest_input = Vec::new();
    let mut total_bytes = 0_usize;
    for path in files {
        let relative = path.strip_prefix(&runtime_root).map_err(|_| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_escaped",
                "Project RuntimeModule source escaped its root.",
            )
        })?;
        let bytes = fs::read(&path).map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_read_failed",
                format!("Project RuntimeModule source cannot be read: {error}"),
            )
        })?;
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > 32 * 1024 * 1024 {
            return Err(ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_too_large",
                "Project production staging source exceeds 32 MiB.",
            ));
        }
        digest_input.extend_from_slice(relative.to_string_lossy().replace('\\', "/").as_bytes());
        digest_input.push(0);
        digest_input.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        digest_input.extend_from_slice(&bytes);
    }
    Ok(sha256_prefixed(&digest_input))
}

fn engine_sdk_source_digest(sdk_root: &Path) -> Result<String, ProjectPlayerArtifactError> {
    let mut files = Vec::new();
    for relative in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        let path = sdk_root.join(relative);
        if path.is_file() {
            files.push(path);
        }
    }
    let crates_root = sdk_root.join("crates");
    for crate_name in ENGINE_PLAYER_RUNTIME_CRATES {
        let crate_root = crates_root.join(crate_name);
        collect_engine_sdk_sources(&crate_root, &crate_root, &mut files)?;
    }
    files.sort();
    if files.is_empty() {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_engine_sdk_empty",
            "Trusted Engine SDK contains no build inputs.",
        ));
    }

    let mut digest_input = Vec::new();
    let mut total_bytes = 0_usize;
    for path in files {
        let relative = path.strip_prefix(sdk_root).map_err(|_| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_engine_sdk_escaped",
                "Engine SDK build input escaped its trusted root.",
            )
        })?;
        let bytes = fs::read(&path).map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_engine_sdk_read_failed",
                format!("Engine SDK build input cannot be read: {error}"),
            )
        })?;
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > 64 * 1024 * 1024 {
            return Err(ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_engine_sdk_too_large",
                "Trusted Engine SDK build inputs exceed 64 MiB.",
            ));
        }
        digest_input.extend_from_slice(relative.to_string_lossy().replace('\\', "/").as_bytes());
        digest_input.push(0);
        digest_input.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        digest_input.extend_from_slice(&bytes);
    }
    Ok(sha256_prefixed(&digest_input))
}

fn collect_engine_sdk_sources(
    crates_root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ProjectPlayerArtifactError> {
    if !crates_root.is_dir() {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_engine_sdk_crates_missing",
            "Trusted Engine SDK crates directory is missing.",
        ));
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_engine_sdk_read_failed",
                format!("Engine SDK directory cannot be read: {error}"),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_engine_sdk_read_failed",
                format!("Engine SDK directory entry cannot be read: {error}"),
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_engine_sdk_sources(crates_root, &path, files)?;
        } else if path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| matches!(extension, "rs" | "toml" | "json" | "wgsl"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn collect_runtime_module_sources(
    runtime_root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ProjectPlayerArtifactError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_read_failed",
                format!("Project RuntimeModule directory cannot be read: {error}"),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_read_failed",
                format!("Project RuntimeModule entry cannot be read: {error}"),
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_read_failed",
                format!("Project RuntimeModule metadata cannot be read: {error}"),
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_link_rejected",
                format!("Project RuntimeModule contains a link: {}", path.display()),
            ));
        }
        let relative = path.strip_prefix(runtime_root).map_err(|_| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_escaped",
                "Project RuntimeModule entry escaped its root.",
            )
        })?;
        if metadata.is_dir() {
            if relative.components().any(|component| {
                component.as_os_str().to_str().is_some_and(|name| {
                    matches!(name, "target" | ".git" | ".cargo" | ".aife" | "Build")
                })
            }) || relative.file_name().and_then(|name| name.to_str()) == Some(".gitignore")
            {
                continue;
            }
            collect_runtime_module_sources(runtime_root, &path, files)?;
        } else if metadata.is_file() {
            if relative.file_name().and_then(|name| name.to_str()) == Some(".gitignore") {
                continue;
            }
            files.push(path);
        } else {
            return Err(ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_source_special_file_rejected",
                format!(
                    "Project RuntimeModule contains a special file: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn generated_host_executable(artifact_root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "ai_project_runtime_player.exe"
    } else {
        "ai_project_runtime_player"
    };
    artifact_root.join("target").join("debug").join(name)
}

fn build_diagnostic(
    error: &ProjectPlayerArtifactError,
) -> ProjectRuntimePlayerArtifactBuildDiagnostic {
    ProjectRuntimePlayerArtifactBuildDiagnostic {
        code: error.code.to_string(),
        message: error.message.clone(),
        path: None,
        next_action: error.next_action.clone(),
    }
}

fn write_build_report(
    request: &ProjectRuntimePlayerArtifactBuildRequest,
    report: &mut ProjectRuntimePlayerArtifactBuildReport,
) {
    let path = report
        .build_report_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            request
                .build_root
                .join("last-project-runtime-player-build-report.json")
        });
    report.build_report_path = Some(path.display().to_string());
    let write_result = serde_json::to_vec_pretty(report)
        .map_err(std::io::Error::other)
        .and_then(|bytes| {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, bytes)
        });
    if let Err(error) = write_result {
        report
            .diagnostics
            .push(ProjectRuntimePlayerArtifactBuildDiagnostic {
                code: "project_runtime.player_artifact_report_write_failed".to_string(),
                message: error.to_string(),
                path: Some(path.display().to_string()),
                next_action: "Repair the isolated artifact report directory and rebuild."
                    .to_string(),
            });
    }
}

pub fn default_project_runtime_player_build_root() -> PathBuf {
    resolve_default_project_runtime_player_build_root(
        std::env::var_os("AIFE_PROJECT_RUNTIME_PLAYER_BUILD_ROOT"),
        std::env::var_os("LOCALAPPDATA"),
        std::env::temp_dir(),
    )
}

fn resolve_default_project_runtime_player_build_root(
    explicit: Option<OsString>,
    local_app_data: Option<OsString>,
    temp_root: PathBuf,
) -> PathBuf {
    explicit.map(PathBuf::from).unwrap_or_else(|| {
        local_app_data
            .map(PathBuf::from)
            .unwrap_or(temp_root)
            .join("AI First Engine")
            .join("BuildCache")
            .join("project-runtime-player-artifacts")
    })
}

pub fn default_engine_sdk_root() -> PathBuf {
    workspace_root()
}

pub(crate) fn workspace_debug_executable(binary_name: &str) -> PathBuf {
    let executable_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    workspace_target_dir().join("debug").join(executable_name)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn workspace_target_dir() -> PathBuf {
    let root = workspace_root();
    resolve_workspace_target_dir(&root, std::env::var_os("CARGO_TARGET_DIR").as_deref())
}

fn resolve_workspace_target_dir(workspace_root: &Path, configured: Option<&OsStr>) -> PathBuf {
    let Some(configured) = configured else {
        return workspace_root.join("target");
    };
    let configured = PathBuf::from(configured);
    if configured.is_absolute() {
        configured
    } else {
        workspace_root.join(configured)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPlayerArtifactError {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

impl ProjectPlayerArtifactError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            next_action:
                "Build the project-specific Player and RuntimePackage from the same runtime module inputs."
                    .to_string(),
        }
    }
}

impl std::fmt::Display for ProjectPlayerArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectPlayerArtifactError {}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::project_runtime_module::{
        project_runtime_aot_digest, ProjectRuntimeAotDigestSource,
        PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn project_runtime_player_artifact_cache_identity_tracks_engine_sdk_sources() {
        let root = temp_root("project-runtime-player-sdk-digest");
        let sdk = root.join("sdk");
        for crate_name in ENGINE_PLAYER_RUNTIME_CRATES {
            fs::create_dir_all(sdk.join("crates").join(crate_name).join("src")).unwrap();
            fs::write(
                sdk.join("crates").join(crate_name).join("Cargo.toml"),
                format!("[package]\nname='{crate_name}'\nversion='0.0.3'\n"),
            )
            .unwrap();
            fs::write(
                sdk.join("crates").join(crate_name).join("src/lib.rs"),
                b"pub fn unchanged() {}\n",
            )
            .unwrap();
        }
        fs::write(sdk.join("Cargo.toml"), b"[workspace]\n").unwrap();
        let runtime_source = sdk.join("crates/engine_runtime/src/lib.rs");
        fs::write(
            &runtime_source,
            b"pub fn renderer_revision() -> u32 { 1 }\n",
        )
        .unwrap();

        let before = engine_sdk_source_digest(&sdk).unwrap();
        fs::write(
            &runtime_source,
            b"pub fn renderer_revision() -> u32 { 2 }\n",
        )
        .unwrap();
        let after = engine_sdk_source_digest(&sdk).unwrap();

        assert_ne!(before, after);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_runtime_player_production_staging_source_digest_tracks_copied_inputs_only() {
        let root = temp_root("project-runtime-player-source-digest");
        let runtime = root.join("RuntimeModule");
        fs::create_dir_all(runtime.join("src")).unwrap();
        fs::create_dir_all(runtime.join("examples")).unwrap();
        fs::create_dir_all(runtime.join("target/debug")).unwrap();
        fs::write(runtime.join("Cargo.toml"), b"[package]\nname='fixture'\n").unwrap();
        fs::write(runtime.join("src/lib.rs"), b"pub fn value() -> u32 { 1 }\n").unwrap();
        fs::write(
            runtime.join("examples/sample.rs"),
            b"fn main() { println!(\"one\"); }\n",
        )
        .unwrap();
        fs::write(runtime.join(".gitignore"), b"target\n").unwrap();
        fs::write(runtime.join("target/debug/stale.json"), b"one").unwrap();
        fs::create_dir_all(root.join("AUI")).unwrap();
        fs::write(root.join("AUI/hud.aui.json"), b"{\"revision\":1}").unwrap();

        let before = runtime_module_source_digest(&root).unwrap();
        fs::write(runtime.join(".gitignore"), b"target\nCargo.lock\n").unwrap();
        fs::write(runtime.join("target/debug/stale.json"), b"two").unwrap();
        fs::write(root.join("AUI/hud.aui.json"), b"{\"revision\":2}").unwrap();
        assert_eq!(runtime_module_source_digest(&root).unwrap(), before);

        fs::write(
            runtime.join("examples/sample.rs"),
            b"fn main() { println!(\"two\"); }\n",
        )
        .unwrap();
        assert_ne!(runtime_module_source_digest(&root).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }

    fn complex_player_path() -> PathBuf {
        workspace_debug_executable("complex_shooter_player")
    }

    #[test]
    fn workspace_target_dir_respects_absolute_and_relative_cargo_overrides() {
        let workspace = Path::new("workspace");
        assert_eq!(
            resolve_workspace_target_dir(workspace, None),
            workspace.join("target")
        );
        assert_eq!(
            resolve_workspace_target_dir(workspace, Some(OsStr::new("custom-target"))),
            workspace.join("custom-target")
        );
        let absolute = std::env::temp_dir().join("aife-custom-target");
        assert_eq!(
            resolve_workspace_target_dir(workspace, Some(absolute.as_os_str())),
            absolute
        );
    }

    #[test]
    fn validated_artifact_staging_publish_moves_the_complete_tree() {
        let root = temp_root("project-player-artifact-publish");
        let staging = root.join(".staging");
        let artifact = root.join("artifact");
        fs::create_dir_all(staging.join("target/debug")).unwrap();
        fs::write(staging.join("target/debug/player.exe"), b"player").unwrap();

        publish_validated_artifact_staging(&staging, &artifact).unwrap();

        assert!(!staging.exists());
        assert_eq!(
            fs::read(artifact.join("target/debug/player.exe")).unwrap(),
            b"player"
        );
    }

    #[test]
    fn project_player_artifact_accepts_exact_descriptor_and_rejects_wrong_module() {
        let executable = complex_player_path();
        ProjectPlayerArtifact::ensure_built(&executable, "complex_shooter_player").unwrap();
        let query = run_bounded_child_process(BoundedChildProcessRequest {
            executable: executable.clone(),
            args: vec![OsString::from("--describe-project-runtime-module")],
            current_dir: executable.parent().unwrap().to_path_buf(),
            environment: Vec::new(),
            timeout: Duration::from_secs(10),
            stdout_capture_limit_bytes: 64 * 1024,
            stderr_capture_limit_bytes: 64 * 1024,
            priority: runtime_cli::BoundedChildProcessPriority::Normal,
        });
        let descriptor: ProjectRuntimeModuleDescriptor =
            serde_json::from_str(query.stdout_summary.trim()).unwrap();
        let exact = RuntimeProjectModuleRef::new(
            descriptor.module_id.clone(),
            descriptor.interface_version.clone(),
            descriptor.aot_content_digest.clone(),
        );

        let artifact = ProjectPlayerArtifact::inspect(&executable, &exact).unwrap();
        assert_eq!(artifact.module_descriptor, descriptor);
        assert!(artifact.source_executable_hash.starts_with("sha256:"));

        let wrong = RuntimeProjectModuleRef::new(
            "sample.wrong.runtime",
            exact.interface_version,
            exact.aot_content_digest,
        );
        let error = ProjectPlayerArtifact::inspect(&executable, &wrong).unwrap_err();
        assert_eq!(error.code, "project_runtime.player_artifact_mismatch");
    }

    #[test]
    fn project_runtime_player_artifact_project_rust_preview_project_rust_export_share_identity() {
        let root = temp_root("project-runtime-player-artifact");
        let _root_cleanup = TestDirectoryGuard(root.clone());
        let project = root.join("project");
        let project_id = format!(
            "fixture-project-runtime-player-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let runtime_root = project.join("RuntimeModule");
        fs::create_dir_all(runtime_root.join("src")).unwrap();
        let cargo_manifest = r#"[package]
name = "fixture_project_runtime"
version = "0.0.3"
edition = "2021"
publish = false

[dependencies]
project_runtime_abi = "=0.0.3"
project_runtime_sdk = "=0.0.3"
serde = { version = "1", features = ["derive"] }
"#;
        let lib_source =
            include_str!("../../../fixtures/project_runtime_native_module_minimal/src/lib.rs")
                .replace("fixture.native.runtime", "fixture.project.runtime")
                .replace(
                    "project-runtime-module.v1",
                    PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
                )
                .replace(
                    "aot_content_digest: AOT_CONTENT_DIGEST.to_string(),",
                    concat!(
                        "aot_content_digest: option_env!(\"AIFE_PROJECT_RUNTIME_AOT_DIGEST\")\n",
                        "                .unwrap_or(AOT_CONTENT_DIGEST)\n",
                        "                .to_string(),",
                    ),
                );
        fs::write(runtime_root.join("Cargo.toml"), cargo_manifest).unwrap();
        fs::write(runtime_root.join("src/lib.rs"), &lib_source).unwrap();
        fs::write(
            project.join("project.aife.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "aife-project.v2",
                "projectId": project_id,
                "projectName": "Fixture Project Runtime Player",
                "engineVersion": "0.0.3",
                "createdAt": "0",
                "lastOpenedAt": null,
                "defaultScene": "Scenes/Main.scene.json",
                "assetRoot": "Assets",
                "settingsVersion": "aife-project-settings.v1",
                "runtimeModule": {
                    "sourceKind": "projectRust",
                    "moduleId": "fixture.project.runtime",
                    "interfaceVersion": "project-runtime-module.v2",
                    "cargoManifest": "RuntimeModule/Cargo.toml",
                    "cargoPackage": "fixture_project_runtime",
                    "playerBinary": "fixture_project_player"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let digest = project_runtime_aot_digest(
            "fixture.project.runtime",
            PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
            "RuntimeModule/Cargo.toml",
            "fixture_project_runtime",
            "fixture_project_player",
            [
                ProjectRuntimeAotDigestSource {
                    relative_path: "RuntimeModule/Cargo.toml",
                    bytes: cargo_manifest.as_bytes(),
                },
                ProjectRuntimeAotDigestSource {
                    relative_path: "RuntimeModule/src/lib.rs",
                    bytes: lib_source.as_bytes(),
                },
            ],
        )
        .unwrap();
        let expected = RuntimeProjectModuleRef::new(
            "fixture.project.runtime",
            PROJECT_RUNTIME_MODULE_INTERFACE_VERSION,
            digest,
        );
        let isolated_build_root = temp_root("pa-build");
        let _build_root_cleanup = TestDirectoryGuard(isolated_build_root.clone());
        let request = ProjectRuntimePlayerArtifactBuildRequest::new(
            &project,
            default_engine_sdk_root(),
            expected,
        )
        .with_build_root(&isolated_build_root);

        let first = ProjectPlayerArtifact::build_project_rust(request.clone());
        assert_eq!(
            first.status,
            ProjectRuntimePlayerArtifactBuildStatus::Success,
            "{:#?}",
            first.diagnostics
        );
        assert_eq!(first.cache_status, "rebuilt");
        let _artifact_cleanup =
            TestDirectoryGuard(PathBuf::from(first.artifact_root.as_ref().unwrap()));
        let artifact = first.artifact.as_ref().unwrap();
        assert!(artifact.executable_path.is_file());
        assert!(artifact.build_report_path.as_ref().unwrap().is_file());
        assert_eq!(
            artifact.module_descriptor.module_id,
            "fixture.project.runtime"
        );
        assert_eq!(first.steps.len(), 6);
        assert_eq!(
            first.staging_policy.as_deref(),
            Some("project-runtime-player-production-staging.v1")
        );
        assert!(first
            .normalized_manifest_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        assert!(first
            .normalized_dependency_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        assert!(first
            .trusted_lock_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        assert_eq!(first.normalized_dependencies.len(), 3);

        let second = ProjectPlayerArtifact::build_project_rust(request);
        assert_eq!(
            second.status,
            ProjectRuntimePlayerArtifactBuildStatus::Success
        );
        assert_eq!(
            second.cache_status, "hit",
            "cache diagnostics: {:#?}",
            second.diagnostics
        );
        assert_eq!(second.executable_hash, first.executable_hash);
        assert_eq!(second.actual_descriptor, first.actual_descriptor);

        fs::create_dir_all(project.join("Scenes")).unwrap();
        fs::create_dir_all(project.join("Input")).unwrap();
        fs::create_dir_all(project.join("Assets")).unwrap();
        fs::write(
            project.join("Scenes/Main.scene.json"),
            r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#111111",
  "entities": []
}"##,
        )
        .unwrap();
        fs::write(
            project.join("Input/input.none.json"),
            serde_json::to_vec_pretty(&engine_input::InputMappingAsset::explicit_empty(
                "input.none",
            ))
            .unwrap(),
        )
        .unwrap();

        let preview = crate::EditorPreviewPackageService::prepare(
            crate::EditorPreviewPackageRequest::editor_play(&project)
                .with_player_artifact_build_root(isolated_build_root.clone()),
        );
        assert_eq!(
            preview.status,
            crate::EditorPreviewPackageStatus::Success,
            "{:#?}",
            preview.diagnostics
        );
        assert_eq!(preview.player_artifact_status, "success_hit");
        let preview_artifact = preview.player_artifact.as_ref().unwrap();
        assert_eq!(
            preview_artifact.source_executable_hash,
            first.executable_hash.clone().unwrap()
        );
        assert_eq!(
            preview_artifact.module_descriptor,
            first.actual_descriptor.clone().unwrap()
        );

        let mut export_request = crate::DesktopExportRequest::windows_dev(&project);
        export_request.frame_limit = 1;
        export_request =
            export_request.with_player_artifact_build_root(isolated_build_root.clone());
        let export = crate::DesktopExportPipeline::export(export_request);
        assert_eq!(
            export.status,
            crate::DesktopExportStatus::Success,
            "{:#?}",
            export.diagnostics
        );
        assert_eq!(
            export.player_artifact_hash.as_deref(),
            Some(preview_artifact.source_executable_hash.as_str())
        );
        assert_eq!(
            export.player_module_descriptor.as_ref(),
            Some(&preview_artifact.module_descriptor)
        );
        assert_eq!(export.player_exit_code, Some(0));
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()))
    }

    struct TestDirectoryGuard(PathBuf);

    impl Drop for TestDirectoryGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn project_runtime_player_default_build_root_is_application_owned_and_overridable() {
        let local = resolve_default_project_runtime_player_build_root(
            None,
            Some(OsString::from("C:/Users/fixture/AppData/Local")),
            PathBuf::from("C:/Temp"),
        );
        assert_eq!(
            local,
            PathBuf::from(
                "C:/Users/fixture/AppData/Local/AI First Engine/BuildCache/project-runtime-player-artifacts"
            )
        );
        let explicit = resolve_default_project_runtime_player_build_root(
            Some(OsString::from("G:/run-owned/player-artifacts")),
            Some(OsString::from("C:/Users/fixture/AppData/Local")),
            PathBuf::from("C:/Temp"),
        );
        assert_eq!(explicit, PathBuf::from("G:/run-owned/player-artifacts"));
    }
}
