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

mod engine_player;
mod incremental;

pub const PROJECT_PLAYER_ARTIFACT_SCHEMA_VERSION: &str = "project-player-artifact.v1";
pub const PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REQUEST_SCHEMA_VERSION: &str =
    "project-runtime-player-artifact-build-request.v1";
pub const PROJECT_RUNTIME_PLAYER_ARTIFACT_BUILD_REPORT_SCHEMA_VERSION: &str =
    "project-runtime-player-artifact-build-report.v1";

const DEFAULT_BUILD_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_CAPTURE_LIMIT_BYTES: usize = 1024 * 1024;
const fn default_compile_tests() -> bool {
    true
}
const ENGINE_PLAYER_RUNTIME_CRATES: [&str; 8] = [
    "project_game_sdk",
    "project_runtime_sdk",
    "project_runtime_abi",
    "engine_input",
    "engine_runtime",
    "engine_runtime_host",
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
    #[serde(default = "default_compile_tests")]
    pub compile_tests: bool,
    #[serde(skip)]
    pub prepared_runtime_glue: Option<crate::PreparedRuntimeGlue>,
    #[serde(skip)]
    pub(crate) prepared_source: Option<crate::CompilerSourceView>,
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
            compile_tests: true,
            prepared_runtime_glue: None,
            prepared_source: None,
        }
    }

    pub fn with_build_root(mut self, build_root: impl Into<PathBuf>) -> Self {
        self.build_root = build_root.into();
        self
    }

    pub fn with_compile_tests(mut self, enabled: bool) -> Self {
        self.compile_tests = enabled;
        self
    }

    pub fn with_prepared_runtime_glue(mut self, glue: crate::PreparedRuntimeGlue) -> Self {
        self.prepared_runtime_glue = Some(glue);
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
    #[serde(default)]
    pub compile_workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_player_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_player_cache_status: Option<String>,
    #[serde(default)]
    pub cargo_fresh_artifacts: usize,
    #[serde(default)]
    pub cargo_rebuilt_artifacts: usize,
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
        let executable = executable_path.into();
        let module = engine_player::sealed_artifact_module(&executable)?;
        inspect_with_process(executable, expected, module.as_deref()).map(|(artifact, _)| artifact)
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
    module_path: Option<&Path>,
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
    let mut args = vec![OsString::from("--describe-project-runtime-module")];
    if let Some(path) = module_path {
        args.extend([
            OsString::from("--project-runtime-dll"),
            path.as_os_str().to_owned(),
        ]);
    }
    let result = run_bounded_child_process(BoundedChildProcessRequest {
        executable: executable_path.clone(),
        args,
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
        compile_workspace: None,
        engine_player_identity: None,
        engine_player_cache_status: None,
        cargo_fresh_artifacts: 0,
        cargo_rebuilt_artifacts: 0,
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
    fs::create_dir_all(&request.build_root).map_err(incremental::io_error)?;
    let build_root = request
        .build_root
        .canonicalize()
        .map_err(incremental::io_error)?;
    let project_root = request
        .project_root
        .canonicalize()
        .map_err(incremental::io_error)?;
    let sdk_root = request
        .engine_sdk_root
        .canonicalize()
        .map_err(incremental::io_error)?;
    if build_root.starts_with(&project_root) || build_root.starts_with(&sdk_root) {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_build_root_not_isolated",
            "Artifact build root must be outside the project root and trusted Engine SDK root.",
        ));
    }
    // Freeze only the Compiler's retained bytes; never reopen project semantics after prepare.
    let frozen_root = request.prepared_source.as_ref().map(|_| {
        build_root.join(format!(
            ".source-{}-{}",
            std::process::id(),
            ARTIFACT_BUILD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    });
    if let Some(root) = &frozen_root {
        fs::create_dir(root).map_err(incremental::io_error)?;
    }
    let _frozen_cleanup = frozen_root.clone().map(incremental::FrozenSource);
    if let (Some(view), Some(root)) = (&request.prepared_source, &frozen_root) {
        for (path, bytes) in &view.files {
            crate::ProjectRelativePath::parse(path).map_err(|error| {
                ProjectPlayerArtifactError::new(
                    "project_runtime.source_path_invalid",
                    error.to_string(),
                )
            })?;
            let destination = root.join(path);
            fs::create_dir_all(destination.parent().unwrap()).map_err(incremental::io_error)?;
            fs::write(destination, bytes).map_err(incremental::io_error)?;
        }
    }
    let source_root = frozen_root.as_deref().unwrap_or(&request.project_root);
    let source = ProjectRuntimeProductionStaging::plan(source_root, &request.engine_sdk_root)
        .map_err(|error| ProjectPlayerArtifactError::new(error.code, error.message))?;
    if !source
        .manifest
        .runtime_module
        .project_game_sdk
        .trim()
        .is_empty()
        && request.prepared_runtime_glue.is_none()
    {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_generated_glue_missing",
            "Project Game SDK modules require Compiler-prepared generated runtime glue.",
        ));
    }
    if source.manifest.runtime_module.module_id != request.expected_module.module_id
        || source.manifest.runtime_module.interface_version
            != request.expected_module.interface_version
    {
        return Err(ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_manifest_mismatch",
            "Project manifest runtime module identity does not match the RuntimePackage.",
        ));
    }

    report.build_root = build_root.display().to_string();
    report.staging_policy = Some("project-runtime-player-production-staging.v1".to_string());
    report.normalized_manifest_digest = Some(source.normalized_manifest_digest.clone());
    report.normalized_dependency_digest = Some(source.normalized_dependency_digest.clone());
    report.normalized_dependencies = source.normalized_dependencies.clone();
    report.trusted_lock_digest = Some(source.trusted_lock_digest.clone());

    let source_digest = runtime_module_source_digest(source_root)?;
    report.source_digest = Some(source_digest.clone());
    let engine_sdk_digest = engine_sdk_source_digest(&source.sdk_root)?;
    report.engine_sdk_digest = Some(engine_sdk_digest.clone());
    let toolchain: toml::Value = toml::from_str(
        &fs::read_to_string(source.sdk_root.join("rust-toolchain.toml"))
            .map_err(incremental::io_error)?,
    )
    .map_err(|error| {
        ProjectPlayerArtifactError::new("project_runtime.toolchain_invalid", error.to_string())
    })?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|v| v.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            ProjectPlayerArtifactError::new(
                "project_runtime.toolchain_invalid",
                "Missing SDK toolchain channel",
            )
        })?;
    let host_target = format!(
        "{}-pc-windows-{}",
        std::env::consts::ARCH,
        if cfg!(target_env = "msvc") {
            "msvc"
        } else {
            "gnu"
        }
    );
    let environment_identity: std::collections::BTreeMap<_, _> = std::env::vars()
        .filter(|(name, _)| {
            name.starts_with("CARGO_")
                || name.starts_with("RUST")
                || matches!(name.as_str(), "PATH" | "LIB" | "INCLUDE" | "CC" | "CXX")
        })
        .collect();
    let compatibility_key = incremental::compatibility_key(
        &project_root,
        &source,
        &request.cargo_executable,
        channel,
        &host_target,
        &environment_identity,
    )?;
    let workspace = incremental::CompileWorkspace::acquire(&build_root, &compatibility_key)?;
    report.compile_workspace = Some(workspace.root.display().to_string());
    let artifact_key = sha256_prefixed(
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            source.manifest.project_id,
            source_digest,
            engine_sdk_digest,
            source.sdk_root.display(),
            source.runtime_player_winit_root.display(),
            request.expected_module.interface_version,
            request.expected_module.aot_content_digest,
            source.normalized_manifest_digest,
            source.normalized_dependency_digest,
            source.trusted_lock_digest,
            request
                .prepared_runtime_glue
                .as_ref()
                .map(crate::PreparedRuntimeGlue::generation_digest)
                .unwrap_or("legacy-manual-abi")
        )
        .as_bytes(),
    );
    let artifact_key = sha256_prefixed(
        format!("fixed-engine-player.v1|{artifact_key}|{compatibility_key}").as_bytes(),
    );
    let artifact_key_hex = artifact_key.trim_start_matches("sha256:");
    let artifact_root = build_root.join(format!("a-{}", &artifact_key_hex[..24]));
    let executable = generated_host_executable(&artifact_root);
    let build_report_path = artifact_root.join("project-runtime-player-build-report.json");
    report.artifact_root = Some(artifact_root.display().to_string());
    report.host_manifest_path = Some(
        source
            .sdk_root
            .join("crates/runtime_cli/Cargo.toml")
            .display()
            .to_string(),
    );
    report.executable_path = Some(executable.display().to_string());
    report.build_report_path = Some(build_report_path.display().to_string());

    if executable.is_file()
        && artifact_root.join("engine_runtime.dll").is_file()
        && artifact_root.join("project_runtime_module.dll").is_file()
    {
        match engine_player::validate_artifact_files(&artifact_root).and_then(|identity| {
            report.engine_player_identity = Some(identity);
            report.engine_player_cache_status = Some("artifact_hit".into());
            inspect_cached_with_process(
                executable.clone(),
                &artifact_root.join("engine_runtime.dll"),
                &request.expected_module,
            )
        }) {
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
    ProjectRuntimeProductionStaging::stage(source_root, &staging_root, &source)
        .map_err(|error| ProjectPlayerArtifactError::new(error.code, error.message))?;
    let module_library = if let Some(glue) = &request.prepared_runtime_glue {
        glue.materialize(
            &staging_root.join("RuntimeGlue"),
            &source.sdk_root,
            Path::new("../RuntimeModuleBuild"),
        )
        .map_err(|error| ProjectPlayerArtifactError::new(error.code, error.message))?;
        "aife_generated_runtime_glue".to_string()
    } else {
        stage_legacy_module_library(
            &staging_root.join("RuntimeModuleBuild/Cargo.toml"),
            &source.manifest.runtime_module.cargo_package,
        )?
    };
    if let Some(root) = &frozen_root {
        fs::remove_dir_all(root).map_err(incremental::io_error)?;
    }
    workspace.sync(&staging_root)?;
    let compile_root = &workspace.root;

    let cargo = request
        .cargo_executable
        .clone()
        .or_else(|| std::env::var_os("CARGO").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let timeout_ms = request.step_timeout_ms.max(1).min(600_000);
    let capture_limit = request.capture_limit_bytes.max(1).min(1024 * 1024);
    let target_root = compile_root.join("target");
    let environment = vec![
        (
            OsString::from("CARGO_TARGET_DIR"),
            target_root.clone().into_os_string(),
        ),
        (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
        (OsString::from("CARGO_INCREMENTAL"), OsString::from("1")),
        (OsString::from("RUSTUP_AUTO_INSTALL"), OsString::from("0")),
        (OsString::from("RUSTUP_TOOLCHAIN"), OsString::from(channel)),
        (
            OsString::from("CARGO_BUILD_TARGET"),
            OsString::from(&host_target),
        ),
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
        ["fmt", "--manifest-path", "Cargo.toml", "--", "--check"],
        &compile_root.join("RuntimeModuleBuild"),
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
            &compile_root.join("RuntimeModuleBuild"),
            &environment,
            timeout_ms,
            capture_limit,
        )?;
    }
    if request.compile_tests {
        run_required_cargo_step(
            report,
            "validate_project_runtime_tests_compile",
            &cargo,
            [
                "test",
                "--no-run",
                "--locked",
                "--offline",
                "--message-format=json",
            ],
            &compile_root.join("RuntimeModuleBuild"),
            &environment,
            timeout_ms,
            capture_limit,
        )?;
    }
    let module_root = compile_root.join(if request.prepared_runtime_glue.is_some() {
        "RuntimeGlue"
    } else {
        "RuntimeModuleBuild"
    });
    run_required_cargo_step(
        report,
        "lock_project_runtime_module_dependencies",
        &cargo,
        ["generate-lockfile", "--offline"],
        &module_root,
        &environment,
        timeout_ms,
        capture_limit,
    )?;
    run_required_cargo_step(
        report,
        "build_project_runtime_module",
        &cargo,
        [
            "build",
            "--manifest-path",
            "Cargo.toml",
            "--locked",
            "--offline",
            "--message-format=json",
        ],
        &module_root,
        &environment,
        timeout_ms,
        capture_limit,
    )?;

    let staged_executable = generated_host_executable(&staging_root);
    engine_player::prepare_and_stage(
        report,
        &build_root,
        &source.sdk_root,
        &staging_root,
        &cargo,
        channel,
        &host_target,
        &environment_identity,
        timeout_ms,
        capture_limit,
    )?;
    ensure_project_runtime_module_dll(
        &compile_root.join("target"),
        &host_target,
        &staging_root,
        &module_library,
    )?;
    let (staged_artifact, describe_process) = inspect_with_process(
        staged_executable.clone(),
        &request.expected_module,
        Some(&staging_root.join("project_runtime_module.dll")),
    )?;
    report.steps.push(ProjectRuntimePlayerArtifactBuildStep {
        stage: "describe_project_runtime_artifact".to_string(),
        command: vec![
            staged_executable.display().to_string(),
            "--describe-project-runtime-module".to_string(),
        ],
        timeout_ms: 10_000,
        process: describe_process,
    });

    engine_player::seal_artifact_files(
        &staging_root,
        report
            .engine_player_identity
            .as_deref()
            .expect("prepared engine identity"),
    )?;

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

fn inspect_cached_with_process(
    executable_path: PathBuf,
    engine_dll: &Path,
    expected: &RuntimeProjectModuleRef,
) -> Result<(ProjectPlayerArtifact, BoundedChildProcessResult), ProjectPlayerArtifactError> {
    // Qualify both the Engine DLL and the actual separately loaded project DLL.
    validate_engine_runtime_dll(engine_dll)?;
    inspect_with_process(
        executable_path,
        expected,
        Some(&engine_dll.with_file_name("project_runtime_module.dll")),
    )
}

fn validate_engine_runtime_dll(path: &Path) -> Result<(), ProjectPlayerArtifactError> {
    #[cfg(windows)]
    runtime_cli::engine_dll_loader::load_and_probe(path)
        .and_then(runtime_cli::engine_dll_loader::require_execution_api)
        .map_err(|error| ProjectPlayerArtifactError::new(
            "project_runtime.engine_runtime_dll_incompatible",
            format!("Engine DLL cannot execute current run/playtest requests: {error}. Build engine_runtime_host before exporting."),
        ))?;
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

fn stage_legacy_module_library(
    manifest_path: &Path,
    package: &str,
) -> Result<String, ProjectPlayerArtifactError> {
    let mut manifest: toml::Value =
        toml::from_str(&fs::read_to_string(manifest_path).map_err(incremental::io_error)?)
            .map_err(|error| {
                ProjectPlayerArtifactError::new(
                    "project_runtime.player_artifact_module_manifest_invalid",
                    error.to_string(),
                )
            })?;
    let table = manifest
        .as_table_mut()
        .expect("normalized Cargo manifest is a table");
    let lib = table
        .entry("lib")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let lib = lib.as_table_mut().expect("normalized Cargo lib is a table");
    let library_name = lib
        .get("name")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| package.replace('-', "_"));
    lib.insert(
        "crate-type".into(),
        toml::Value::Array(vec!["rlib".into(), "cdylib".into()]),
    );
    fs::write(
        manifest_path,
        toml::to_string(&manifest).expect("normalized Cargo manifest serializes"),
    )
    .map_err(incremental::io_error)?;
    Ok(library_name)
}

fn ensure_project_runtime_module_dll(
    target_root: &Path,
    host_target: &str,
    staging_root: &Path,
    module_library: &str,
) -> Result<(), ProjectPlayerArtifactError> {
    let name = module_library;
    let file_name = if cfg!(windows) {
        format!("{name}.dll")
    } else {
        format!("lib{name}.so")
    };
    let candidates = [
        target_root.join(host_target).join("debug").join(&file_name),
        target_root
            .join(host_target)
            .join("debug")
            .join("deps")
            .join(&file_name),
        target_root.join("debug").join(&file_name),
        target_root.join("debug").join("deps").join(&file_name),
    ];
    let source = candidates
        .iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            ProjectPlayerArtifactError::new(
                "project_runtime.player_artifact_module_dll_missing",
                "Generated RuntimeModule build completed without a loadable module DLL.",
            )
        })?;
    fs::copy(source, staging_root.join("project_runtime_module.dll")).map_err(|error| {
        ProjectPlayerArtifactError::new(
            "project_runtime.player_artifact_module_dll_stage_failed",
            format!("Generated RuntimeModule DLL cannot be staged: {error}"),
        )
    })?;
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
        && process.owned_process_cleanup_confirmed()
        && !process.stdout_truncated
        && !process.stderr_truncated;
    if !process.owned_process_cleanup_confirmed() {
        if let Some(root) = &report.compile_workspace {
            fs::write(
                Path::new(root).join("ownership-unclosed"),
                b"Child ownership requires manual recovery.",
            )
            .map_err(incremental::io_error)?;
        }
    }
    if args.contains(&"--message-format=json") && passed {
        for line in process.stdout_summary.lines() {
            let message: serde_json::Value = serde_json::from_str(line).map_err(|error| {
                ProjectPlayerArtifactError::new(
                    "project_runtime.cargo_output_invalid",
                    error.to_string(),
                )
            })?;
            if message["reason"] == "compiler-artifact" {
                match message["fresh"].as_bool() {
                    Some(true) => report.cargo_fresh_artifacts += 1,
                    Some(false) => report.cargo_rebuilt_artifacts += 1,
                    None => {
                        return Err(ProjectPlayerArtifactError::new(
                            "project_runtime.cargo_output_invalid",
                            "Missing Cargo freshness flag",
                        ))
                    }
                }
            }
        }
    }
    let rust_errors = process
        .stdout_summary
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-message" && message["message"]["level"] == "error"
        })
        .filter_map(|message| message["message"]["rendered"].as_str().map(str::to_string))
        .collect::<Vec<_>>()
        .join("\n");
    let process_status = format!(
        "exit={:?}, reason={:?}, stdoutTruncated={}, stderrTruncated={}",
        process.exit_code, process.exit_reason, process.stdout_truncated, process.stderr_truncated
    );
    let failure_summary = [
        process_status.as_str(),
        process.stderr_summary.trim(),
        if rust_errors.is_empty() {
            ""
        } else {
            &rust_errors
        },
    ]
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

pub fn runtime_module_source_digest(
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
    for relative in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo/config",
        ".cargo/config.toml",
    ] {
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
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(windows)]
    #[test]
    fn cached_player_rejects_invalid_engine_dll_before_descriptor_query() {
        let root = temp_root("cached-player-engine-dll-probe");
        fs::create_dir_all(&root).unwrap();
        let engine_dll = root.join("engine_runtime.dll");
        fs::write(&engine_dll, b"not an Engine DLL").unwrap();
        assert!(engine_dll.is_file());
        let error = inspect_cached_with_process(
            root.join("must-not-run.exe"),
            &engine_dll,
            &RuntimeProjectModuleRef::explicit_empty(),
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            "project_runtime.engine_runtime_dll_incompatible"
        );
        fs::remove_file(engine_dll).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn project_player_artifact_cache_identity_tracks_engine_sdk_sources() {
        let root = temp_root("project-runtime-player-sdk-digest");
        let sdk = root.join("sdk");
        for crate_name in ENGINE_PLAYER_RUNTIME_CRATES {
            fs::create_dir_all(sdk.join("crates").join(crate_name).join("src")).unwrap();
            fs::write(
                sdk.join("crates").join(crate_name).join("Cargo.toml"),
                format!("[package]\nname='{crate_name}'\nversion='0.1.0'\n"),
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
    fn legacy_module_derivation_preserves_custom_library_name_and_source() {
        let root = temp_root("legacy-module-library");
        fs::create_dir_all(&root).unwrap();
        for (extra, expected) in [
            ("", "project_runtime"),
            (
                "[lib]\nname='custom_runtime'\npath='src/game.rs'\n",
                "custom_runtime",
            ),
        ] {
            let source = format!("[package]\nname='project-runtime'\nversion='0.1.0'\n{extra}");
            let original = root.join("source.toml");
            let derived = root.join("derived.toml");
            fs::write(&original, &source).unwrap();
            fs::copy(&original, &derived).unwrap();
            assert_eq!(
                stage_legacy_module_library(&derived, "project-runtime").unwrap(),
                expected
            );
            assert_eq!(fs::read_to_string(&original).unwrap(), source);
            let result: toml::Value =
                toml::from_str(&fs::read_to_string(derived).unwrap()).unwrap();
            assert_eq!(
                result["lib"]["crate-type"].as_array().unwrap(),
                &vec![toml::Value::from("rlib"), toml::Value::from("cdylib")]
            );
            if !extra.is_empty() {
                assert_eq!(result["lib"]["path"].as_str(), Some("src/game.rs"));
            }
        }
        fs::remove_dir_all(root).unwrap();
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

    fn temp_root(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()))
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
