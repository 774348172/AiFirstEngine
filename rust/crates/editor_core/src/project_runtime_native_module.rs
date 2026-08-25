use engine_runtime::canonical_digest::{sha256_prefixed, ConsistencyDigest};
use engine_runtime::project_runtime_native_adapter::LoadedProjectRuntimeModuleAdapter;
use project_runtime_abi::{
    ProjectRuntimeApi, ProjectRuntimeEntry, PROJECT_RUNTIME_ABI_MAJOR,
    PROJECT_RUNTIME_API_STRUCT_SIZE, PROJECT_RUNTIME_ENTRY_SYMBOL,
};
use runtime_cli::{
    run_bounded_child_process, run_bounded_child_process_cancellable,
    BoundedChildProcessCancellation, BoundedChildProcessExitReason, BoundedChildProcessPriority,
    BoundedChildProcessRequest, BoundedChildProcessResult,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION: &str =
    "project-runtime-native-module-identity.v1";
pub const PROJECT_RUNTIME_NATIVE_MODULE_ARTIFACT_SCHEMA_VERSION: &str =
    "project-runtime-native-module-artifact.v1";
pub const PROJECT_RUNTIME_NATIVE_MODULE_DESCRIPTOR_SCHEMA_VERSION: &str =
    "project-runtime-native-module-descriptor.v1";
pub const PROJECT_RUNTIME_NATIVE_MODULE_BUILD_REPORT_SCHEMA_VERSION: &str =
    "project-runtime-native-module-build-report.v1";
pub const PROJECT_RUNTIME_NATIVE_MODULE_SEAL_SCHEMA_VERSION: &str =
    "project-runtime-native-module-seal.v1";
pub const PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION: &str =
    "project-runtime-native-module-builder.v1";
pub const PROJECT_RUNTIME_NATIVE_MODULE_LOAD_REPORT_SCHEMA_VERSION: &str =
    "project-runtime-native-module-load-report.v1";

const CACHE_ROOT_NAME: &str = "project-runtime-native-modules";
const DESCRIPTOR_FILE_NAME: &str = "project-runtime-native-module-descriptor.v1.json";
const BUILD_REPORT_FILE_NAME: &str = "project-runtime-native-module-build-report.v1.json";
const SEAL_FILE_NAME: &str = "project-runtime-native-module-seal.v1.json";
const BUILD_SCOPE: &str = "project_module_only";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectNativeModuleIdentity {
    pub schema_version: String,
    pub project_runtime_abi_digest: String,
    pub project_runtime_sdk_digest: String,
    pub project_id: String,
    pub module_id: String,
    pub logical_interface_version: String,
    pub aot_content_digest: String,
    pub normalized_manifest_digest: String,
    pub normalized_dependency_digest: String,
    pub dependency_lock_digest: String,
    pub toolchain_identity: String,
    pub target_triple: String,
    pub profile: String,
    pub features: Vec<String>,
    pub builder_schema_version: String,
}

impl ProjectNativeModuleIdentity {
    pub fn validate(&self) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
        if self.schema_version != PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION
            || self.builder_schema_version != PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION
        {
            return Err(diag(
                "project_runtime.native_module_identity_schema_unsupported",
                "validate_identity",
                "Native module identity or builder schema is unsupported.",
                None,
                "Regenerate the native module identity with the current SDK.",
            ));
        }
        for (field, value) in [
            ("projectId", self.project_id.as_str()),
            ("moduleId", self.module_id.as_str()),
            (
                "logicalInterfaceVersion",
                self.logical_interface_version.as_str(),
            ),
            ("toolchainIdentity", self.toolchain_identity.as_str()),
            ("targetTriple", self.target_triple.as_str()),
            ("profile", self.profile.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(diag(
                    "project_runtime.native_module_identity_field_required",
                    "validate_identity",
                    format!("Native module identity field {field} is required."),
                    None,
                    "Regenerate the identity from current project inputs.",
                ));
            }
        }
        for (field, value) in [
            (
                "projectRuntimeAbiDigest",
                self.project_runtime_abi_digest.as_str(),
            ),
            (
                "projectRuntimeSdkDigest",
                self.project_runtime_sdk_digest.as_str(),
            ),
            ("aotContentDigest", self.aot_content_digest.as_str()),
            (
                "normalizedManifestDigest",
                self.normalized_manifest_digest.as_str(),
            ),
            (
                "normalizedDependencyDigest",
                self.normalized_dependency_digest.as_str(),
            ),
            ("dependencyLockDigest", self.dependency_lock_digest.as_str()),
        ] {
            if !is_sha256(value) {
                return Err(diag(
                    "project_runtime.native_module_identity_digest_invalid",
                    "validate_identity",
                    format!(
                        "Native module identity field {field} must be sha256:<64 lowercase hex>."
                    ),
                    None,
                    "Regenerate the identity from canonical inputs.",
                ));
            }
        }
        if !is_portable_id(&self.module_id)
            || self.features.iter().any(|feature| !is_portable_id(feature))
        {
            return Err(diag(
                "project_runtime.native_module_identity_name_invalid",
                "validate_identity",
                "Module id and feature names must be portable identifiers.",
                None,
                "Use ASCII letters, digits, dot, dash, or underscore.",
            ));
        }
        let mut sorted = self.features.clone();
        sorted.sort();
        sorted.dedup();
        if sorted != self.features {
            return Err(diag(
                "project_runtime.native_module_identity_features_not_canonical",
                "validate_identity",
                "Native module features must be sorted and unique.",
                None,
                "Sort and deduplicate the feature list.",
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, ProjectRuntimeNativeModuleDiagnostic> {
        self.validate()?;
        ConsistencyDigest::sha256(
            "project-runtime-native-module-identity",
            PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION,
            self,
        )
        .map(|digest| digest.prefixed_value())
        .map_err(|error| {
            diag(
                "project_runtime.native_module_identity_digest_failed",
                "validate_identity",
                error.to_string(),
                None,
                "Regenerate the identity from canonical inputs.",
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeNativeModuleDescriptor {
    pub schema_version: String,
    pub identity: ProjectNativeModuleIdentity,
    pub identity_digest: String,
    pub artifact_hash: String,
    pub dll_file_name: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeNativeModuleSeal {
    pub schema_version: String,
    pub identity_digest: String,
    pub artifact_hash: String,
    pub descriptor_hash: String,
    pub build_report_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeNativeModuleBuildStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeNativeModuleCacheStatus {
    ExactHit,
    Rebuilt,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeNativeModuleBuildStep {
    pub stage: String,
    pub process: BoundedChildProcessResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeNativeModuleBuildReport {
    pub schema_version: String,
    pub status: ProjectRuntimeNativeModuleBuildStatus,
    pub cache_status: ProjectRuntimeNativeModuleCacheStatus,
    pub identity: ProjectNativeModuleIdentity,
    pub identity_digest: String,
    pub build_scope: String,
    pub dependency_packages: Vec<String>,
    pub artifact: Option<ProjectRuntimeNativeModuleArtifact>,
    pub steps: Vec<ProjectRuntimeNativeModuleBuildStep>,
    pub diagnostics: Vec<ProjectRuntimeNativeModuleDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeNativeModuleArtifact {
    pub schema_version: String,
    pub artifact_root: PathBuf,
    pub dll_path: PathBuf,
    pub descriptor_path: PathBuf,
    pub build_report_path: PathBuf,
    pub seal_path: PathBuf,
    pub descriptor: ProjectRuntimeNativeModuleDescriptor,
}

impl ProjectRuntimeNativeModuleArtifact {
    pub fn open(root: &Path) -> Result<Self, ProjectRuntimeNativeModuleDiagnostic> {
        let root = canonical_directory(root, "artifact root")?;
        let descriptor_path = root.join(DESCRIPTOR_FILE_NAME);
        let descriptor: ProjectRuntimeNativeModuleDescriptor = read_json(&descriptor_path)?;
        let artifact = Self {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_ARTIFACT_SCHEMA_VERSION.to_string(),
            dll_path: root.join("bin").join(&descriptor.dll_file_name),
            build_report_path: root.join(BUILD_REPORT_FILE_NAME),
            seal_path: root.join(SEAL_FILE_NAME),
            descriptor_path,
            artifact_root: root,
            descriptor,
        };
        verify_artifact(&artifact)?;
        Ok(artifact)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeNativeModuleBuildRequest {
    pub source_crate_root: PathBuf,
    pub engine_sdk_root: PathBuf,
    pub build_root: PathBuf,
    pub identity: ProjectNativeModuleIdentity,
    pub cargo_executable: Option<PathBuf>,
    pub metadata_hard_deadline_ms: u64,
    pub build_hard_deadline_ms: u64,
    pub capture_limit_bytes: usize,
}

impl ProjectRuntimeNativeModuleBuildRequest {
    pub fn validate(&self) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
        self.identity.validate()?;
        if self.metadata_hard_deadline_ms == 0
            || self.build_hard_deadline_ms == 0
            || self.capture_limit_bytes == 0
        {
            return Err(diag(
                "project_runtime.native_module_build_policy_invalid",
                "validate_request",
                "Native module deadlines and capture limit must be non-zero.",
                None,
                "Provide bounded build policy values.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeNativeModuleDiagnostic {
    pub code: String,
    pub stage: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: String,
}

impl std::fmt::Display for ProjectRuntimeNativeModuleDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectRuntimeNativeModuleDiagnostic {}

pub struct ProjectRuntimeNativeModuleBuilder;

#[derive(Clone, Default)]
pub struct ProjectRuntimeNativeModuleBuildControl {
    cancellation: BoundedChildProcessCancellation,
}

impl ProjectRuntimeNativeModuleBuildControl {
    pub fn request_cancel(&self) {
        self.cancellation.request_cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

impl ProjectRuntimeNativeModuleBuilder {
    pub fn prepare(
        request: &ProjectRuntimeNativeModuleBuildRequest,
    ) -> ProjectRuntimeNativeModuleBuildReport {
        prepare_with_runner(request, |process| run_bounded_child_process(process))
    }

    pub fn prepare_cancellable(
        request: &ProjectRuntimeNativeModuleBuildRequest,
        control: ProjectRuntimeNativeModuleBuildControl,
    ) -> ProjectRuntimeNativeModuleBuildReport {
        prepare_with_runner(request, move |process| {
            run_bounded_child_process_cancellable(process, control.cancellation.clone())
        })
    }
}

fn prepare_with_runner(
    request: &ProjectRuntimeNativeModuleBuildRequest,
    mut runner: impl FnMut(BoundedChildProcessRequest) -> BoundedChildProcessResult,
) -> ProjectRuntimeNativeModuleBuildReport {
    let identity_digest = request.identity.digest().unwrap_or_default();
    let mut report = ProjectRuntimeNativeModuleBuildReport {
        schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILD_REPORT_SCHEMA_VERSION.to_string(),
        status: ProjectRuntimeNativeModuleBuildStatus::Failed,
        cache_status: ProjectRuntimeNativeModuleCacheStatus::Failed,
        identity: request.identity.clone(),
        identity_digest,
        build_scope: BUILD_SCOPE.to_string(),
        dependency_packages: Vec::new(),
        artifact: None,
        steps: Vec::new(),
        diagnostics: Vec::new(),
    };
    if let Err(error) = prepare_inner(request, &mut runner, &mut report) {
        report.diagnostics.push(error);
    }
    report
}

fn prepare_inner(
    request: &ProjectRuntimeNativeModuleBuildRequest,
    runner: &mut impl FnMut(BoundedChildProcessRequest) -> BoundedChildProcessResult,
    report: &mut ProjectRuntimeNativeModuleBuildReport,
) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
    request.validate()?;
    let source_root = canonical_directory(&request.source_crate_root, "source crate root")?;
    let sdk_root = canonical_directory(&request.engine_sdk_root, "Engine SDK root")?;
    let build_root = prepare_build_root(&request.build_root, &source_root)?;
    let identity_digest = request.identity.digest()?;
    report.identity_digest.clone_from(&identity_digest);
    let key = identity_digest.trim_start_matches("sha256:");
    let cache_root = build_root.join(CACHE_ROOT_NAME);
    fs::create_dir_all(cache_root.join("cache"))
        .map_err(|error| io_diag("prepare_root", &cache_root, error))?;
    fs::create_dir_all(cache_root.join("staging"))
        .map_err(|error| io_diag("prepare_root", &cache_root, error))?;
    fs::create_dir_all(cache_root.join("ct"))
        .map_err(|error| io_diag("prepare_root", &cache_root, error))?;
    let artifact_root = cache_root.join("cache").join(key);
    if artifact_root.is_dir() {
        match ProjectRuntimeNativeModuleArtifact::open(&artifact_root) {
            Ok(artifact) if artifact.descriptor.identity == request.identity => {
                report.status = ProjectRuntimeNativeModuleBuildStatus::Success;
                report.cache_status = ProjectRuntimeNativeModuleCacheStatus::ExactHit;
                report.artifact = Some(artifact);
                return Ok(());
            }
            _ => remove_owned_child(&cache_root.join("cache"), &artifact_root)?,
        }
    }

    let staging_root = cache_root.join("staging").join(format!(
        "{}-{}-{}",
        &key[..16],
        std::process::id(),
        now_epoch_nanos()
    ));
    copy_source_tree(&source_root, &staging_root)?;
    let outcome = (|| {
        let (package_name, crate_name) = normalize_cdylib_manifest(&staging_root, &sdk_root)?;
        let cargo = request
            .cargo_executable
            .clone()
            .or_else(|| std::env::var_os("CARGO").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("cargo"));
        let target_root = cache_root
            .join("ct")
            .join(compilation_affinity_key(&request.identity)?);
        fs::create_dir_all(&target_root)
            .map_err(|error| io_diag("prepare_target", &target_root, error))?;
        let environment = vec![
            (
                OsString::from("CARGO_TARGET_DIR"),
                target_root.as_os_str().to_os_string(),
            ),
            (
                OsString::from("AIFE_PROJECT_RUNTIME_AOT_DIGEST"),
                OsString::from(&request.identity.aot_content_digest),
            ),
        ];
        let metadata = run_step(
            runner,
            report,
            "cargo_metadata",
            &cargo,
            vec![
                "metadata".into(),
                "--format-version".into(),
                "1".into(),
                "--locked".into(),
                "--offline".into(),
            ],
            &staging_root,
            &environment,
            request.metadata_hard_deadline_ms,
            request.capture_limit_bytes,
        )?;
        let dependencies = validate_dependency_scope(&metadata.stdout_summary, &package_name)?;
        report.dependency_packages = dependencies;

        let mut build_args = vec![
            OsString::from("build"),
            OsString::from("--release"),
            OsString::from("--locked"),
            OsString::from("--offline"),
            OsString::from("--no-default-features"),
        ];
        if !request.identity.features.is_empty() {
            build_args.push("--features".into());
            build_args.push(request.identity.features.join(",").into());
        }
        if request.identity.target_triple != "host" {
            build_args.push("--target".into());
            build_args.push(request.identity.target_triple.clone().into());
        }
        run_step(
            runner,
            report,
            "cargo_build_release",
            &cargo,
            build_args,
            &staging_root,
            &environment,
            request.build_hard_deadline_ms,
            request.capture_limit_bytes,
        )?;

        let mut built_root = target_root.clone();
        if request.identity.target_triple != "host" {
            built_root.push(&request.identity.target_triple);
        }
        built_root.push("release");
        let built_dll = built_root.join(format!("{}.dll", crate_name.replace('-', "_")));
        let dll_bytes = read_regular_bytes(&built_dll)?;
        let artifact_hash = sha256_prefixed(&dll_bytes);
        let dll_file_name = format!(
            "{}_{}.dll",
            safe_module_name(&request.identity.module_id),
            &artifact_hash.trim_start_matches("sha256:")[..16]
        );
        let descriptor = ProjectRuntimeNativeModuleDescriptor {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity: request.identity.clone(),
            identity_digest: identity_digest.clone(),
            artifact_hash: artifact_hash.clone(),
            dll_file_name: dll_file_name.clone(),
            created_at: now_epoch_seconds(),
        };
        let publish_root = staging_root.join("PublishedArtifact");
        fs::create_dir_all(publish_root.join("bin"))
            .map_err(|error| io_diag("publish", &publish_root, error))?;
        fs::write(publish_root.join("bin").join(&dll_file_name), dll_bytes)
            .map_err(|error| io_diag("publish", &publish_root, error))?;
        write_json(&publish_root.join(DESCRIPTOR_FILE_NAME), &descriptor)?;
        report.status = ProjectRuntimeNativeModuleBuildStatus::Success;
        report.cache_status = ProjectRuntimeNativeModuleCacheStatus::Rebuilt;
        report.artifact = None;
        write_json(&publish_root.join(BUILD_REPORT_FILE_NAME), report)?;
        let descriptor_hash = sha256_prefixed(&read_regular_bytes(
            &publish_root.join(DESCRIPTOR_FILE_NAME),
        )?);
        let build_report_hash = sha256_prefixed(&read_regular_bytes(
            &publish_root.join(BUILD_REPORT_FILE_NAME),
        )?);
        write_json(
            &publish_root.join(SEAL_FILE_NAME),
            &ProjectRuntimeNativeModuleSeal {
                schema_version: PROJECT_RUNTIME_NATIVE_MODULE_SEAL_SCHEMA_VERSION.to_string(),
                identity_digest: identity_digest.clone(),
                artifact_hash,
                descriptor_hash,
                build_report_hash,
            },
        )?;
        if artifact_root.exists() {
            remove_owned_child(&cache_root.join("cache"), &artifact_root)?;
        }
        fs::rename(&publish_root, &artifact_root)
            .map_err(|error| io_diag("publish", &artifact_root, error))?;
        let artifact = ProjectRuntimeNativeModuleArtifact::open(&artifact_root)?;
        report.artifact = Some(artifact);
        Ok(())
    })();
    let _ = remove_owned_child(&cache_root.join("staging"), &staging_root);
    outcome
}

fn run_step(
    runner: &mut impl FnMut(BoundedChildProcessRequest) -> BoundedChildProcessResult,
    report: &mut ProjectRuntimeNativeModuleBuildReport,
    stage: &str,
    executable: &Path,
    args: Vec<OsString>,
    current_dir: &Path,
    environment: &[(OsString, OsString)],
    timeout_ms: u64,
    capture_limit_bytes: usize,
) -> Result<BoundedChildProcessResult, ProjectRuntimeNativeModuleDiagnostic> {
    let process = runner(BoundedChildProcessRequest {
        executable: executable.to_path_buf(),
        args,
        current_dir: current_dir.to_path_buf(),
        environment: environment.to_vec(),
        timeout: Duration::from_millis(timeout_ms),
        stdout_capture_limit_bytes: capture_limit_bytes.min(1024 * 1024),
        stderr_capture_limit_bytes: capture_limit_bytes.min(1024 * 1024),
        priority: BoundedChildProcessPriority::BelowNormal,
    });
    let passed = process.exit_reason == BoundedChildProcessExitReason::Completed
        && process.exit_code == Some(0)
        && process.reader_join_error.is_none();
    report.steps.push(ProjectRuntimeNativeModuleBuildStep {
        stage: stage.to_string(),
        process: process.clone(),
    });
    if !passed {
        return Err(diag(
            "project_runtime.native_module_build_step_failed",
            stage,
            format!(
                "Bounded native module build step failed: {}",
                process.stderr_summary
            ),
            Some(current_dir),
            "Inspect the bounded process evidence and repair module-only inputs.",
        ));
    }
    Ok(process)
}

fn normalize_cdylib_manifest(
    staging_root: &Path,
    sdk_root: &Path,
) -> Result<(String, String), ProjectRuntimeNativeModuleDiagnostic> {
    let manifest_path = staging_root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest_path)
        .map_err(|error| io_diag("stage_manifest", &manifest_path, error))?;
    let mut manifest: toml::Value = toml::from_str(&text).map_err(|error| {
        diag(
            "project_runtime.native_module_manifest_invalid",
            "stage_manifest",
            error.to_string(),
            Some(&manifest_path),
            "Repair the project RuntimeModule Cargo manifest.",
        )
    })?;
    let root = manifest.as_table_mut().ok_or_else(|| {
        diag(
            "project_runtime.native_module_manifest_invalid",
            "stage_manifest",
            "Cargo manifest root must be a table.",
            Some(&manifest_path),
            "Repair the project RuntimeModule Cargo manifest.",
        )
    })?;
    let package = root
        .get("package")
        .and_then(toml::Value::as_table)
        .and_then(|value| value.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            diag(
                "project_runtime.native_module_manifest_invalid",
                "stage_manifest",
                "Cargo package name is required.",
                Some(&manifest_path),
                "Repair the project RuntimeModule Cargo manifest.",
            )
        })?
        .to_string();
    let crate_name = root
        .get("lib")
        .and_then(toml::Value::as_table)
        .and_then(|value| value.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or(&package)
        .to_string();
    let mut lib = root
        .remove("lib")
        .and_then(|value| value.as_table().cloned())
        .unwrap_or_default();
    lib.insert(
        "crate-type".to_string(),
        toml::Value::Array(vec![toml::Value::String("cdylib".to_string())]),
    );
    root.insert("lib".to_string(), toml::Value::Table(lib));
    let dependencies = root
        .entry("dependencies")
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| {
            diag(
                "project_runtime.native_module_manifest_invalid",
                "stage_manifest",
                "Cargo dependencies must be a table.",
                Some(&manifest_path),
                "Repair the project RuntimeModule Cargo manifest.",
            )
        })?;
    for name in ["project_runtime_sdk", "project_runtime_abi"] {
        let crate_root = canonical_directory(&sdk_root.join("crates").join(name), "SDK crate")?;
        dependencies.insert(
            name.to_string(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "path".to_string(),
                toml::Value::String(crate_root.display().to_string()),
            )])),
        );
    }
    let normalized = toml::to_string(&manifest).map_err(|error| {
        diag(
            "project_runtime.native_module_manifest_invalid",
            "stage_manifest",
            error.to_string(),
            Some(&manifest_path),
            "Repair the project RuntimeModule Cargo manifest.",
        )
    })?;
    fs::write(&manifest_path, normalized)
        .map_err(|error| io_diag("stage_manifest", &manifest_path, error))?;
    Ok((package, crate_name))
}

fn validate_dependency_scope(
    metadata_json: &str,
    root_package_name: &str,
) -> Result<Vec<String>, ProjectRuntimeNativeModuleDiagnostic> {
    let metadata: serde_json::Value = serde_json::from_str(metadata_json).map_err(|error| {
        diag(
            "project_runtime.native_module_metadata_invalid",
            "cargo_metadata",
            error.to_string(),
            None,
            "Capture complete Cargo metadata JSON for the module build.",
        )
    })?;
    let packages = metadata["packages"].as_array().ok_or_else(|| {
        diag(
            "project_runtime.native_module_metadata_invalid",
            "cargo_metadata",
            "Cargo metadata packages are missing.",
            None,
            "Capture complete Cargo metadata JSON.",
        )
    })?;
    let resolve_nodes = metadata["resolve"]["nodes"].as_array().ok_or_else(|| {
        diag(
            "project_runtime.native_module_metadata_invalid",
            "cargo_metadata",
            "Cargo metadata resolve graph is missing.",
            None,
            "Capture metadata with dependency resolution enabled.",
        )
    })?;
    let mut id_to_name = BTreeMap::new();
    let mut root_id = None;
    for package in packages {
        let id = package["id"].as_str().unwrap_or_default().to_string();
        let name = package["name"].as_str().unwrap_or_default().to_string();
        if name == root_package_name {
            root_id = Some(id.clone());
        }
        id_to_name.insert(id, name);
    }
    let root_id = root_id.ok_or_else(|| {
        diag(
            "project_runtime.native_module_root_missing",
            "cargo_metadata",
            "Root package is absent from Cargo metadata.",
            None,
            "Repair the staged module manifest.",
        )
    })?;
    let mut edges = BTreeMap::<String, Vec<String>>::new();
    for node in resolve_nodes {
        let id = node["id"].as_str().unwrap_or_default().to_string();
        let deps = node["deps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|dep| dep["pkg"].as_str().map(str::to_string))
            .collect();
        edges.insert(id, deps);
    }
    let mut queue = VecDeque::from([root_id]);
    let mut visited = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(deps) = edges.get(&id) {
            queue.extend(deps.iter().cloned());
        }
    }
    let mut names = visited
        .into_iter()
        .filter_map(|id| id_to_name.get(&id).cloned())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    for forbidden in [
        "engine_runtime",
        "engine_input",
        "editor_core",
        "ai_tool_gateway",
    ] {
        if names.iter().any(|name| name == forbidden) {
            return Err(diag(
                "project_runtime.native_module_dependency_scope_rejected",
                "cargo_metadata",
                format!("Module-only dependency graph contains forbidden crate '{forbidden}'."),
                None,
                "Depend only on project_runtime_sdk/project_runtime_abi and approved project dependencies.",
            ));
        }
    }
    if names.iter().any(|name| name.starts_with("editor_")) {
        return Err(diag(
            "project_runtime.native_module_dependency_scope_rejected",
            "cargo_metadata",
            "Module-only dependency graph contains an Editor crate.",
            None,
            "Remove Editor dependencies from the project RuntimeModule.",
        ));
    }
    let root_package = packages
        .iter()
        .find(|package| package["name"].as_str() == Some(root_package_name))
        .expect("root package was resolved above");
    let targets = root_package["targets"].as_array().ok_or_else(|| {
        diag(
            "project_runtime.native_module_metadata_invalid",
            "cargo_metadata",
            "Root package targets are missing.",
            None,
            "Repair Cargo metadata output.",
        )
    })?;
    if targets.iter().any(|target| {
        target["kind"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
    }) {
        return Err(diag(
            "project_runtime.native_module_dependency_scope_rejected",
            "cargo_metadata",
            "Module-only root contains a generated executable target.",
            None,
            "Build only the project cdylib target.",
        ));
    }
    Ok(names)
}

fn verify_artifact(
    artifact: &ProjectRuntimeNativeModuleArtifact,
) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
    if artifact.schema_version != PROJECT_RUNTIME_NATIVE_MODULE_ARTIFACT_SCHEMA_VERSION
        || artifact.descriptor.schema_version
            != PROJECT_RUNTIME_NATIVE_MODULE_DESCRIPTOR_SCHEMA_VERSION
    {
        return Err(diag(
            "project_runtime.native_module_artifact_schema_unsupported",
            "verify_artifact",
            "Native module artifact schema is unsupported.",
            Some(&artifact.artifact_root),
            "Rebuild the native module artifact.",
        ));
    }
    artifact.descriptor.identity.validate()?;
    if artifact.descriptor.identity.digest()? != artifact.descriptor.identity_digest {
        return Err(diag(
            "project_runtime.native_module_identity_mismatch",
            "verify_artifact",
            "Descriptor identity digest does not match its identity.",
            Some(&artifact.descriptor_path),
            "Rebuild the native module artifact.",
        ));
    }
    let canonical_root = canonical_directory(&artifact.artifact_root, "artifact root")?;
    let dll = canonical_regular_file(&artifact.dll_path, "native module DLL")?;
    let descriptor = canonical_regular_file(&artifact.descriptor_path, "native module descriptor")?;
    let report = canonical_regular_file(&artifact.build_report_path, "native module build report")?;
    let seal_path = canonical_regular_file(&artifact.seal_path, "native module seal")?;
    if !dll.starts_with(&canonical_root)
        || !descriptor.starts_with(&canonical_root)
        || !report.starts_with(&canonical_root)
        || !seal_path.starts_with(&canonical_root)
    {
        return Err(diag(
            "project_runtime.native_module_artifact_path_unsafe",
            "verify_artifact",
            "Artifact files must be canonical children of the artifact root.",
            Some(&canonical_root),
            "Restore the application-owned artifact layout.",
        ));
    }
    let seal: ProjectRuntimeNativeModuleSeal = read_json(&seal_path)?;
    if seal.schema_version != PROJECT_RUNTIME_NATIVE_MODULE_SEAL_SCHEMA_VERSION
        || seal.identity_digest != artifact.descriptor.identity_digest
        || seal.artifact_hash != artifact.descriptor.artifact_hash
        || seal.artifact_hash != sha256_prefixed(&read_regular_bytes(&dll)?)
        || seal.descriptor_hash != sha256_prefixed(&read_regular_bytes(&descriptor)?)
        || seal.build_report_hash != sha256_prefixed(&read_regular_bytes(&report)?)
    {
        return Err(diag(
            "project_runtime.native_module_artifact_seal_mismatch",
            "verify_artifact",
            "Native module seal does not bind the current artifact bytes.",
            Some(&seal_path),
            "Restore or rebuild the exact sealed artifact.",
        ));
    }
    if dll.file_name().and_then(OsStr::to_str) != Some(&artifact.descriptor.dll_file_name)
        || !artifact.descriptor.dll_file_name.ends_with(&format!(
            "_{}.dll",
            &artifact
                .descriptor
                .artifact_hash
                .trim_start_matches("sha256:")[..16]
        ))
    {
        return Err(diag(
            "project_runtime.native_module_artifact_name_mismatch",
            "verify_artifact",
            "Native module DLL name is not hash-qualified by its sealed bytes.",
            Some(&dll),
            "Rebuild the native module artifact.",
        ));
    }
    Ok(())
}

pub struct ProjectRuntimeNativeModuleLoader;

impl ProjectRuntimeNativeModuleLoader {
    #[cfg(windows)]
    pub fn load(
        artifact: &ProjectRuntimeNativeModuleArtifact,
    ) -> Result<LoadedProjectRuntimeModuleAdapter, ProjectRuntimeNativeModuleDiagnostic> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::LibraryLoader::{
            GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        };

        verify_artifact(artifact)?;
        let dll_path = canonical_regular_file(&artifact.dll_path, "native module DLL")?;
        if !dll_path.is_absolute() {
            return Err(diag(
                "project_runtime.native_module_load_path_unsafe",
                "load_library",
                "Native module DLL path must be canonical and absolute.",
                Some(&dll_path),
                "Load the qualified application-owned artifact path.",
            ));
        }
        let wide = dll_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: the canonical absolute path is NUL terminated and the flags exclude CWD/PATH search.
        let handle = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        if handle.is_null() {
            return Err(diag(
                "project_runtime.native_module_load_failed",
                "load_library",
                format!(
                    "LoadLibraryExW failed with OS error {}.",
                    std::io::Error::last_os_error()
                ),
                Some(&dll_path),
                "Inspect the sealed DLL and its declared system dependencies.",
            ));
        }
        let library = Arc::new(WindowsNativeLibrary(handle));
        // SAFETY: the library is live and the symbol name is the single ABI entry contract.
        let symbol = unsafe { GetProcAddress(handle, PROJECT_RUNTIME_ENTRY_SYMBOL.as_ptr()) };
        let symbol = symbol.ok_or_else(|| {
            diag(
                "project_runtime.native_module_symbol_missing",
                "resolve_symbol",
                "Native module does not export aife_project_runtime_entry_v1.",
                Some(&dll_path),
                "Rebuild the module with the SDK ABI facade.",
            )
        })?;
        // SAFETY: the uniquely named symbol is validated as the v1 entry function immediately below.
        let entry: ProjectRuntimeEntry = unsafe { std::mem::transmute(symbol) };
        // SAFETY: the ABI entry takes no borrowed inputs and returns a module-owned static table.
        let api = unsafe { entry() };
        if api.is_null() {
            return Err(diag(
                "project_runtime.native_module_entry_null",
                "validate_api",
                "Native module entry returned a null API table.",
                Some(&dll_path),
                "Repair the module ABI facade.",
            ));
        }
        // SAFETY: the table stays valid while the retained library guard is alive.
        let api: ProjectRuntimeApi = unsafe { *api };
        validate_loaded_api_header(&api, &dll_path)?;
        LoadedProjectRuntimeModuleAdapter::new_with_lifetime_guard(api, library).map_err(|error| {
            diag(
                "project_runtime.native_module_adapter_rejected",
                "validate_api",
                error.message,
                Some(&dll_path),
                "Repair the module descriptor/function table and rebuild.",
            )
        })
    }

    #[cfg(not(windows))]
    pub fn load(
        _artifact: &ProjectRuntimeNativeModuleArtifact,
    ) -> Result<LoadedProjectRuntimeModuleAdapter, ProjectRuntimeNativeModuleDiagnostic> {
        Err(diag(
            "project_runtime.native_module_platform_unsupported",
            "load_library",
            "Native project module loading v1 is Windows-only.",
            None,
            "Run the real loader gate on Windows.",
        ))
    }
}

fn validate_loaded_api_header(
    api: &ProjectRuntimeApi,
    dll_path: &Path,
) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
    if api.struct_size != PROJECT_RUNTIME_API_STRUCT_SIZE
        || api.abi_major != PROJECT_RUNTIME_ABI_MAJOR
        || api.contract_digest != project_runtime_sdk::project_runtime_contract_digest()
    {
        return Err(diag(
            "project_runtime.native_module_abi_mismatch",
            "validate_api",
            "Native module ABI version, digest, or struct size does not match the host.",
            Some(dll_path),
            "Rebuild the project module against the current ProjectRuntimeSdk.",
        ));
    }
    Ok(())
}

#[cfg(windows)]
struct WindowsNativeLibrary(windows_sys::Win32::Foundation::HMODULE);

#[cfg(windows)]
unsafe impl Send for WindowsNativeLibrary {}
#[cfg(windows)]
unsafe impl Sync for WindowsNativeLibrary {}

#[cfg(windows)]
impl Drop for WindowsNativeLibrary {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::FreeLibrary;
        // SAFETY: this owner holds one successful LoadLibraryExW reference.
        unsafe { FreeLibrary(self.0) };
    }
}

fn compilation_affinity_key(
    identity: &ProjectNativeModuleIdentity,
) -> Result<String, ProjectRuntimeNativeModuleDiagnostic> {
    #[derive(Serialize)]
    struct Affinity<'a> {
        abi: &'a str,
        sdk: &'a str,
        toolchain: &'a str,
        target: &'a str,
        profile: &'a str,
        features: &'a [String],
        lock: &'a str,
    }
    ConsistencyDigest::sha256(
        "project-runtime-native-module-compilation-affinity",
        "project-runtime-native-module-compilation-affinity.v1",
        &Affinity {
            abi: &identity.project_runtime_abi_digest,
            sdk: &identity.project_runtime_sdk_digest,
            toolchain: &identity.toolchain_identity,
            target: &identity.target_triple,
            profile: &identity.profile,
            features: &identity.features,
            lock: &identity.dependency_lock_digest,
        },
    )
    .map(|digest| digest.value[..32].to_string())
    .map_err(|error| {
        diag(
            "project_runtime.native_module_affinity_failed",
            "prepare_target",
            error.to_string(),
            None,
            "Regenerate the native module build request.",
        )
    })
}

fn copy_source_tree(
    source: &Path,
    destination: &Path,
) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
    fs::create_dir_all(destination).map_err(|error| io_diag("stage_source", destination, error))?;
    for entry in fs::read_dir(source).map_err(|error| io_diag("stage_source", source, error))? {
        let entry = entry.map_err(|error| io_diag("stage_source", source, error))?;
        if entry.file_name() == OsStr::new("target") {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| io_diag("stage_source", &entry.path(), error))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err(diag(
                "project_runtime.native_module_source_link_rejected",
                "stage_source",
                "Native module staging does not follow links or reparse points.",
                Some(&entry.path()),
                "Use a regular contained RuntimeModule source tree.",
            ));
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            copy_source_tree(&entry.path(), &target)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), &target)
                .map_err(|error| io_diag("stage_source", &target, error))?;
        }
    }
    Ok(())
}

fn prepare_build_root(
    build_root: &Path,
    source_root: &Path,
) -> Result<PathBuf, ProjectRuntimeNativeModuleDiagnostic> {
    if build_root.starts_with(source_root) || source_root.starts_with(build_root) {
        return Err(diag(
            "project_runtime.native_module_build_root_scope_rejected",
            "validate_request",
            "Build root and source crate root must be disjoint.",
            Some(build_root),
            "Use an application-owned build root outside the project source.",
        ));
    }
    fs::create_dir_all(build_root)
        .map_err(|error| io_diag("validate_request", build_root, error))?;
    let build_root = canonical_directory(build_root, "build root")?;
    if build_root.starts_with(source_root) || source_root.starts_with(&build_root) {
        return Err(diag(
            "project_runtime.native_module_build_root_scope_rejected",
            "validate_request",
            "Build root and source crate root must be disjoint.",
            Some(&build_root),
            "Use an application-owned build root outside the project source.",
        ));
    }
    Ok(build_root)
}

fn remove_owned_child(
    owner: &Path,
    target: &Path,
) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
    if !target.exists() {
        return Ok(());
    }
    let owner = canonical_directory(owner, "owned root")?;
    let parent = target.parent().ok_or_else(|| {
        diag(
            "project_runtime.native_module_cleanup_rejected",
            "cleanup",
            "Cleanup target has no parent.",
            Some(target),
            "Inspect the owned build root.",
        )
    })?;
    let parent = canonical_directory(parent, "cleanup parent")?;
    let metadata =
        fs::symlink_metadata(target).map_err(|error| io_diag("cleanup", target, error))?;
    if !parent.starts_with(&owner)
        || metadata.file_type().is_symlink()
        || is_reparse(&metadata)
        || !metadata.is_dir()
    {
        return Err(diag(
            "project_runtime.native_module_cleanup_rejected",
            "cleanup",
            "Cleanup target must be a regular strict child of the owned root.",
            Some(target),
            "Inspect the owned build root without following links.",
        ));
    }
    fs::remove_dir_all(target).map_err(|error| io_diag("cleanup", target, error))
}

fn canonical_directory(
    path: &Path,
    label: &str,
) -> Result<PathBuf, ProjectRuntimeNativeModuleDiagnostic> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| io_diag("validate_path", path, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(diag(
            "project_runtime.native_module_path_unsafe",
            "validate_path",
            format!("{label} must be a regular directory."),
            Some(path),
            "Use a regular canonical application-owned path.",
        ));
    }
    path.canonicalize()
        .map(normalize_path)
        .map_err(|error| io_diag("validate_path", path, error))
}

fn canonical_regular_file(
    path: &Path,
    label: &str,
) -> Result<PathBuf, ProjectRuntimeNativeModuleDiagnostic> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| io_diag("validate_path", path, error))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(diag(
            "project_runtime.native_module_path_unsafe",
            "validate_path",
            format!("{label} must be a regular file."),
            Some(path),
            "Use a regular canonical application-owned artifact file.",
        ));
    }
    path.canonicalize()
        .map(normalize_path)
        .map_err(|error| io_diag("validate_path", path, error))
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn normalize_path(path: PathBuf) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = value.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

#[cfg(not(windows))]
fn normalize_path(path: PathBuf) -> PathBuf {
    path
}

fn read_regular_bytes(path: &Path) -> Result<Vec<u8>, ProjectRuntimeNativeModuleDiagnostic> {
    let path = canonical_regular_file(path, "artifact file")?;
    fs::read(&path).map_err(|error| io_diag("read_artifact", &path, error))
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, ProjectRuntimeNativeModuleDiagnostic> {
    let bytes = read_regular_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        diag(
            "project_runtime.native_module_json_invalid",
            "read_artifact",
            error.to_string(),
            Some(path),
            "Restore or rebuild the typed artifact.",
        )
    })
}

fn write_json(
    path: &Path,
    value: &impl Serialize,
) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        diag(
            "project_runtime.native_module_json_encode_failed",
            "publish",
            error.to_string(),
            Some(path),
            "Repair the typed artifact contract.",
        )
    })?;
    fs::write(path, bytes).map_err(|error| io_diag("publish", path, error))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_portable_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn safe_module_name(module_id: &str) -> String {
    module_id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn io_diag(
    stage: &str,
    path: &Path,
    error: std::io::Error,
) -> ProjectRuntimeNativeModuleDiagnostic {
    diag(
        "project_runtime.native_module_io_failed",
        stage,
        error.to_string(),
        Some(path),
        "Inspect the application-owned native module path and retry.",
    )
}

fn diag(
    code: &str,
    stage: &str,
    message: impl Into<String>,
    path: Option<&Path>,
    next_action: impl Into<String>,
) -> ProjectRuntimeNativeModuleDiagnostic {
    ProjectRuntimeNativeModuleDiagnostic {
        code: code.to_string(),
        stage: stage.to_string(),
        message: message.into(),
        path: path.map(|value| value.display().to_string()),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn identity() -> ProjectNativeModuleIdentity {
        ProjectNativeModuleIdentity {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION.to_string(),
            project_runtime_abi_digest: digest('1'),
            project_runtime_sdk_digest: digest('2'),
            project_id: "fixture-project".to_string(),
            module_id: "fixture.runtime".to_string(),
            logical_interface_version: "project-runtime-module.v1".to_string(),
            aot_content_digest: digest('3'),
            normalized_manifest_digest: digest('4'),
            normalized_dependency_digest: digest('5'),
            dependency_lock_digest: digest('6'),
            toolchain_identity: "rustc-test".to_string(),
            target_triple: "host".to_string(),
            profile: "release".to_string(),
            features: vec!["fixture".to_string()],
            builder_schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION
                .to_string(),
        }
    }

    fn sealed_artifact(root: &Path, bytes: &[u8]) -> ProjectRuntimeNativeModuleArtifact {
        fs::create_dir_all(root.join("bin")).unwrap();
        let hash = sha256_prefixed(bytes);
        let file_name = format!("fixture_runtime_{}.dll", &hash[7..23]);
        fs::write(root.join("bin").join(&file_name), bytes).unwrap();
        let descriptor = ProjectRuntimeNativeModuleDescriptor {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity: identity(),
            identity_digest: identity().digest().unwrap(),
            artifact_hash: hash.clone(),
            dll_file_name: file_name,
            created_at: 1,
        };
        write_json(&root.join(DESCRIPTOR_FILE_NAME), &descriptor).unwrap();
        let report = ProjectRuntimeNativeModuleBuildReport {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILD_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectRuntimeNativeModuleBuildStatus::Success,
            cache_status: ProjectRuntimeNativeModuleCacheStatus::Rebuilt,
            identity: identity(),
            identity_digest: identity().digest().unwrap(),
            build_scope: BUILD_SCOPE.to_string(),
            dependency_packages: vec!["fixture".to_string(), "project_runtime_sdk".to_string()],
            artifact: None,
            steps: Vec::new(),
            diagnostics: Vec::new(),
        };
        write_json(&root.join(BUILD_REPORT_FILE_NAME), &report).unwrap();
        write_json(
            &root.join(SEAL_FILE_NAME),
            &ProjectRuntimeNativeModuleSeal {
                schema_version: PROJECT_RUNTIME_NATIVE_MODULE_SEAL_SCHEMA_VERSION.to_string(),
                identity_digest: descriptor.identity_digest.clone(),
                artifact_hash: hash,
                descriptor_hash: sha256_prefixed(
                    &fs::read(root.join(DESCRIPTOR_FILE_NAME)).unwrap(),
                ),
                build_report_hash: sha256_prefixed(
                    &fs::read(root.join(BUILD_REPORT_FILE_NAME)).unwrap(),
                ),
            },
        )
        .unwrap();
        ProjectRuntimeNativeModuleArtifact::open(root).unwrap()
    }

    #[test]
    fn project_runtime_native_module_identity_excludes_editor_and_engine_implementation() {
        let identity = identity();
        let first = identity.digest().unwrap();
        assert_eq!(first, identity.digest().unwrap());
        let json = serde_json::to_string(&identity).unwrap();
        assert!(!json.contains("editorShell"));
        assert!(!json.contains("engineRuntime"));
        let mut changed = identity.clone();
        changed.project_runtime_sdk_digest = digest('7');
        assert_ne!(first, changed.digest().unwrap());
    }

    #[test]
    fn project_runtime_native_module_invalidation_matrix() {
        let baseline = identity();
        let baseline_digest = baseline.digest().unwrap();
        let encoded = serde_json::to_string(&baseline).unwrap();
        for excluded in [
            "editorShell",
            "engineImplementation",
            "sceneDigest",
            "auiDigest",
            "inputDigest",
        ] {
            assert!(
                !encoded.contains(excluded),
                "unexpected identity input: {excluded}"
            );
        }

        let editor_shell_changed = baseline.clone();
        let engine_implementation_changed = baseline.clone();
        let scene_aui_input_changed = baseline.clone();
        assert_eq!(baseline_digest, editor_shell_changed.digest().unwrap());
        assert_eq!(
            baseline_digest,
            engine_implementation_changed.digest().unwrap()
        );
        assert_eq!(baseline_digest, scene_aui_input_changed.digest().unwrap());

        let mut abi_changed = baseline.clone();
        abi_changed.project_runtime_abi_digest = digest('7');
        assert_ne!(baseline_digest, abi_changed.digest().unwrap());
        let mut sdk_changed = baseline.clone();
        sdk_changed.project_runtime_sdk_digest = digest('8');
        assert_ne!(baseline_digest, sdk_changed.digest().unwrap());
        let mut project_source_changed = baseline;
        project_source_changed.aot_content_digest = digest('9');
        assert_ne!(baseline_digest, project_source_changed.digest().unwrap());

        let sdk_digest = project_runtime_sdk::project_runtime_aot_digest(
            "fixture.runtime",
            "project-runtime-module.v2",
            "RuntimeModule/Cargo.toml",
            "fixture_runtime",
            "fixture_player",
            [project_runtime_sdk::ProjectRuntimeAotDigestSource {
                relative_path: "RuntimeModule/src/lib.rs",
                bytes: b"pub fn fixture() {}",
            }],
        )
        .unwrap();
        let engine_digest = engine_runtime::project_runtime_module::project_runtime_aot_digest(
            "fixture.runtime",
            "project-runtime-module.v2",
            "RuntimeModule/Cargo.toml",
            "fixture_runtime",
            "fixture_player",
            [
                engine_runtime::project_runtime_module::ProjectRuntimeAotDigestSource {
                    relative_path: "RuntimeModule/src/lib.rs",
                    bytes: b"pub fn fixture() {}",
                },
            ],
        )
        .unwrap();
        assert_eq!(sdk_digest, engine_digest);
    }

    #[test]
    fn tower_loaded_project_runtime_source_contract_normalizes_to_module_only_cdylib() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap();
        let tower_module = repository_root.join("samples/tower_defense_project/RuntimeModule");
        let owned_root =
            std::env::temp_dir().join(format!("aife-tower-loaded-contract-{}", now_epoch_nanos()));
        let staged = owned_root.join("staged");
        copy_source_tree(&tower_module, &staged).unwrap();

        let (package_name, crate_name) =
            normalize_cdylib_manifest(&staged, &repository_root.join("rust")).unwrap();
        assert_eq!(package_name, "tower_defense_project_runtime");
        assert_eq!(crate_name, package_name);

        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(staged.join("Cargo.toml")).unwrap()).unwrap();
        let dependencies = manifest["dependencies"].as_table().unwrap();
        for required in ["project_runtime_abi", "project_runtime_sdk"] {
            assert!(dependencies.contains_key(required), "missing {required}");
        }
        for forbidden in [
            "engine_runtime",
            "engine_input",
            "editor_core",
            "ai_tool_gateway",
        ] {
            assert!(
                !dependencies.contains_key(forbidden),
                "forbidden {forbidden}"
            );
        }
        assert_eq!(
            manifest["lib"]["crate-type"].as_array().unwrap(),
            &[toml::Value::String("cdylib".to_string())]
        );

        let source = fs::read_to_string(staged.join("src/lib.rs")).unwrap();
        assert!(source.contains("aife_project_runtime_entry_v1"));
        assert!(source.contains("AIFE_PROJECT_RUNTIME_AOT_DIGEST"));
        fs::remove_dir_all(owned_root).unwrap();
    }

    #[test]
    fn project_runtime_native_module_artifact_rejects_hash_and_seal_tamper() {
        let root = std::env::temp_dir().join(format!("aife-native-artifact-{}", now_epoch_nanos()));
        fs::create_dir_all(root.join("bin")).unwrap();
        let bytes = b"fixture-dll";
        let hash = sha256_prefixed(bytes);
        let file_name = format!("fixture_runtime_{}.dll", &hash[7..23]);
        fs::write(root.join("bin").join(&file_name), bytes).unwrap();
        let descriptor = ProjectRuntimeNativeModuleDescriptor {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            identity: identity(),
            identity_digest: identity().digest().unwrap(),
            artifact_hash: hash.clone(),
            dll_file_name: file_name,
            created_at: 1,
        };
        write_json(&root.join(DESCRIPTOR_FILE_NAME), &descriptor).unwrap();
        let report = ProjectRuntimeNativeModuleBuildReport {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILD_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectRuntimeNativeModuleBuildStatus::Success,
            cache_status: ProjectRuntimeNativeModuleCacheStatus::Rebuilt,
            identity: identity(),
            identity_digest: identity().digest().unwrap(),
            build_scope: BUILD_SCOPE.to_string(),
            dependency_packages: vec!["fixture".to_string(), "project_runtime_sdk".to_string()],
            artifact: None,
            steps: Vec::new(),
            diagnostics: Vec::new(),
        };
        write_json(&root.join(BUILD_REPORT_FILE_NAME), &report).unwrap();
        write_json(
            &root.join(SEAL_FILE_NAME),
            &ProjectRuntimeNativeModuleSeal {
                schema_version: PROJECT_RUNTIME_NATIVE_MODULE_SEAL_SCHEMA_VERSION.to_string(),
                identity_digest: descriptor.identity_digest.clone(),
                artifact_hash: hash,
                descriptor_hash: sha256_prefixed(
                    &fs::read(root.join(DESCRIPTOR_FILE_NAME)).unwrap(),
                ),
                build_report_hash: sha256_prefixed(
                    &fs::read(root.join(BUILD_REPORT_FILE_NAME)).unwrap(),
                ),
            },
        )
        .unwrap();
        let artifact = ProjectRuntimeNativeModuleArtifact::open(&root).unwrap();
        fs::write(&artifact.dll_path, b"tampered").unwrap();
        let error = ProjectRuntimeNativeModuleArtifact::open(&root).unwrap_err();
        assert_eq!(
            error.code,
            "project_runtime.native_module_artifact_seal_mismatch"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_runtime_native_module_dependency_scope_rejects_engine_editor_and_bin() {
        let metadata = serde_json::json!({
            "packages": [
                {"id":"root","name":"fixture","targets":[{"kind":["cdylib"]}]},
                {"id":"sdk","name":"project_runtime_sdk","targets":[]},
                {"id":"engine","name":"engine_runtime","targets":[]}
            ],
            "resolve":{"nodes":[
                {"id":"root","deps":[{"pkg":"sdk"},{"pkg":"engine"}]},
                {"id":"sdk","deps":[]}, {"id":"engine","deps":[]}
            ]}
        });
        let error = validate_dependency_scope(&metadata.to_string(), "fixture").unwrap_err();
        assert_eq!(
            error.code,
            "project_runtime.native_module_dependency_scope_rejected"
        );
    }

    #[test]
    fn project_runtime_native_module_loader_rejects_unqualified_artifact_before_os_load() {
        let root = std::env::temp_dir().join(format!("aife-native-loader-{}", now_epoch_nanos()));
        fs::create_dir_all(&root).unwrap();
        let artifact = ProjectRuntimeNativeModuleArtifact {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_ARTIFACT_SCHEMA_VERSION.to_string(),
            artifact_root: root.clone(),
            dll_path: root.join("missing.dll"),
            descriptor_path: root.join(DESCRIPTOR_FILE_NAME),
            build_report_path: root.join(BUILD_REPORT_FILE_NAME),
            seal_path: root.join(SEAL_FILE_NAME),
            descriptor: ProjectRuntimeNativeModuleDescriptor {
                schema_version: PROJECT_RUNTIME_NATIVE_MODULE_DESCRIPTOR_SCHEMA_VERSION.to_string(),
                identity: identity(),
                identity_digest: identity().digest().unwrap(),
                artifact_hash: digest('a'),
                dll_file_name: "missing_aaaaaaaaaaaaaaaa.dll".to_string(),
                created_at: 1,
            },
        };
        let error = match ProjectRuntimeNativeModuleLoader::load(&artifact) {
            Ok(_) => panic!("unqualified artifact must fail before OS load"),
            Err(error) => error,
        };
        assert_eq!(error.stage, "validate_path");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_runtime_native_module_loader_rejects_abi_header_mismatch() {
        let api = ProjectRuntimeApi {
            struct_size: PROJECT_RUNTIME_API_STRUCT_SIZE,
            abi_major: PROJECT_RUNTIME_ABI_MAJOR + 1,
            abi_minor: 0,
            reserved: 0,
            capabilities: 0,
            module_context: Default::default(),
            contract_digest: project_runtime_sdk::project_runtime_contract_digest(),
            descriptor: None,
            create_session: None,
            destroy_session: None,
            session_id: None,
            invoke_rule: None,
            handle_aui_actions: None,
            fixed_update: None,
            resolve_ui_state: None,
            observe: None,
        };
        let error = validate_loaded_api_header(&api, Path::new("fixture.dll")).unwrap_err();
        assert_eq!(error.code, "project_runtime.native_module_abi_mismatch");
    }

    #[cfg(windows)]
    #[test]
    fn project_runtime_native_module_loader_rejects_missing_entry_symbol() {
        let system_dll = PathBuf::from(std::env::var_os("WINDIR").unwrap())
            .join("System32")
            .join("version.dll");
        let bytes = fs::read(system_dll).unwrap();
        let root = std::env::temp_dir().join(format!("aife-native-symbol-{}", now_epoch_nanos()));
        let artifact = sealed_artifact(&root, &bytes);
        let error = match ProjectRuntimeNativeModuleLoader::load(&artifact) {
            Ok(_) => panic!("system DLL must not expose the project runtime entry"),
            Err(error) => error,
        };
        assert_eq!(error.code, "project_runtime.native_module_symbol_missing");
        fs::remove_dir_all(root).unwrap();
    }
}
