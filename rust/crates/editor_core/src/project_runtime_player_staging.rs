use crate::{ProjectManifest, ProjectRuntimeSourceKind, PROJECT_MANIFEST_SCHEMA_VERSION};
use engine_runtime::canonical_digest::sha256_prefixed;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_MANIFEST_PATH: &str = "project.aife.json";
const RUNTIME_MANIFEST_PATH: &str = "RuntimeModule/Cargo.toml";
const CONTROLLED_THIRD_PARTY_DEPENDENCIES: &[&str] = &["serde", "serde_json"];
const TRUSTED_ENGINE_DEPENDENCIES: &[&str] = &["engine_input", "engine_runtime"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimePlayerDependencyIdentity {
    pub name: String,
    pub dependency_kind: String,
    pub resolved_version: String,
    pub source_identity: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectRuntimePlayerStagingPlan {
    pub manifest: ProjectManifest,
    pub sdk_root: PathBuf,
    pub runtime_cli_root: PathBuf,
    pub runtime_player_winit_root: PathBuf,
    pub source_manifest_text: String,
    pub normalized_manifest_text: String,
    pub normalized_manifest_digest: String,
    pub normalized_dependency_digest: String,
    pub normalized_dependencies: Vec<ProjectRuntimePlayerDependencyIdentity>,
    pub dependency_lock_bytes: Vec<u8>,
    pub has_source_lock: bool,
    pub trusted_lock_digest: String,
}

#[derive(Debug)]
pub(crate) struct ProjectRuntimePlayerStagingError {
    pub code: String,
    pub message: String,
}

impl ProjectRuntimePlayerStagingError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

pub(crate) struct ProjectRuntimeProductionStaging;

impl ProjectRuntimeProductionStaging {
    pub(crate) fn plan(
        project_root: &Path,
        engine_sdk_root: &Path,
    ) -> Result<ProjectRuntimePlayerStagingPlan, ProjectRuntimePlayerStagingError> {
        plan_project_runtime_player_staging(project_root, engine_sdk_root)
    }

    pub(crate) fn stage(
        project_root: &Path,
        destination_root: &Path,
        plan: &ProjectRuntimePlayerStagingPlan,
    ) -> Result<(), ProjectRuntimePlayerStagingError> {
        stage_project_runtime_player_source(project_root, destination_root, plan)
    }
}

#[derive(Debug, Clone)]
struct LockedPackage {
    version: Version,
    source: String,
    checksum: Option<String>,
}

pub(crate) fn plan_project_runtime_player_staging(
    project_root: &Path,
    engine_sdk_root: &Path,
) -> Result<ProjectRuntimePlayerStagingPlan, ProjectRuntimePlayerStagingError> {
    let manifest_path = project_root.join(PROJECT_MANIFEST_PATH);
    let manifest_text = read_regular_utf8(&manifest_path)?;
    let manifest: ProjectManifest = serde_json::from_str(&manifest_text).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_invalid",
            format!("Project manifest is invalid: {error}"),
        )
    })?;
    if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION
        || manifest.runtime_module.resolved_source_kind() != ProjectRuntimeSourceKind::ProjectRust
        || manifest.runtime_module.cargo_manifest != RUNTIME_MANIFEST_PATH
    {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_policy_rejected",
            "Production staging requires an aife-project.v2 ProjectRust module at RuntimeModule/Cargo.toml.",
        ));
    }
    manifest.runtime_module.validate().map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_policy_rejected",
            error,
        )
    })?;
    validate_project_source_policy(project_root)?;

    let sdk_root = canonical_directory(engine_sdk_root, "trusted Engine SDK root")?;
    let trusted_lock_path = sdk_root.join("Cargo.lock");
    let trusted_lock_bytes = read_regular_bytes(&trusted_lock_path)?;
    let trusted_lock_digest = sha256_prefixed(&trusted_lock_bytes);
    let runtime_manifest_path = project_root.join(RUNTIME_MANIFEST_PATH);
    let runtime_manifest_text = read_regular_utf8(&runtime_manifest_path)?;
    let source_lock_path = project_root.join("RuntimeModule/Cargo.lock");
    let has_source_lock = source_lock_path.is_file();
    let dependency_lock_bytes = if has_source_lock {
        read_regular_bytes(&source_lock_path)?
    } else {
        trusted_lock_bytes.clone()
    };
    let locked_packages = parse_trusted_lock(&dependency_lock_bytes)?;
    let mut runtime_manifest: toml::Value =
        toml::from_str(&runtime_manifest_text).map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_invalid",
                format!("RuntimeModule Cargo manifest is invalid TOML: {error}"),
            )
        })?;
    let root = runtime_manifest.as_table_mut().ok_or_else(|| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_invalid",
            "RuntimeModule Cargo manifest root must be a table.",
        )
    })?;
    validate_root_policy(root)?;
    validate_package_policy(root, &manifest)?;
    normalize_lib_policy(root)?;

    let dependencies = root
        .get_mut("dependencies")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                "RuntimeModule Cargo manifest requires [dependencies].",
            )
        })?;
    let mut normalized_identities = Vec::new();
    let dependency_names = dependencies.keys().cloned().collect::<Vec<_>>();
    for name in dependency_names {
        let source_value = dependencies.get(&name).cloned().ok_or_else(|| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_invalid",
                "Dependency disappeared during normalization.",
            )
        })?;
        if TRUSTED_ENGINE_DEPENDENCIES.contains(&name.as_str()) {
            let crate_root = resolve_trusted_engine_dependency(
                &sdk_root,
                runtime_manifest_path
                    .parent()
                    .unwrap_or_else(|| Path::new(".")),
                &name,
                &manifest.engine_version,
                &source_value,
            )?;
            let mut normalized = toml::map::Map::new();
            normalized.insert(
                "path".to_string(),
                toml::Value::String(crate_root.display().to_string()),
            );
            dependencies.insert(name.clone(), toml::Value::Table(normalized));
            normalized_identities.push(ProjectRuntimePlayerDependencyIdentity {
                name,
                dependency_kind: "engine_sdk".to_string(),
                resolved_version: manifest.engine_version.clone(),
                source_identity: crate_root.display().to_string(),
                features: Vec::new(),
            });
        } else if CONTROLLED_THIRD_PARTY_DEPENDENCIES.contains(&name.as_str()) {
            let (requirement, default_features, features) =
                parse_third_party_requirement(&name, &source_value)?;
            let package = resolve_locked_package(&locked_packages, &name, &requirement)?;
            let mut normalized = toml::map::Map::new();
            normalized.insert(
                "version".to_string(),
                toml::Value::String(format!("={}", package.version)),
            );
            if !default_features {
                normalized.insert("default-features".to_string(), toml::Value::Boolean(false));
            }
            if !features.is_empty() {
                normalized.insert(
                    "features".to_string(),
                    toml::Value::Array(features.iter().cloned().map(toml::Value::String).collect()),
                );
            }
            dependencies.insert(name.clone(), toml::Value::Table(normalized));
            let checksum = package
                .checksum
                .as_deref()
                .map(|value| format!("#{value}"))
                .unwrap_or_default();
            normalized_identities.push(ProjectRuntimePlayerDependencyIdentity {
                name,
                dependency_kind: "crates_io".to_string(),
                resolved_version: package.version.to_string(),
                source_identity: format!("{}{}", package.source, checksum),
                features,
            });
        } else {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_third_party_dependency_unsupported",
                format!(
                    "Dependency '{name}' is not in the production staging v1 controlled allowlist."
                ),
            ));
        }
    }
    if !dependencies.contains_key("engine_runtime") {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_policy_rejected",
            "RuntimeModule dependency engine_runtime is required.",
        ));
    }
    normalized_identities.sort_by(|left, right| left.name.cmp(&right.name));
    let mut build_lib = toml::map::Map::new();
    build_lib.insert(
        "path".to_string(),
        toml::Value::String("../RuntimeModule/src/lib.rs".to_string()),
    );
    root.insert("lib".to_string(), toml::Value::Table(build_lib));
    add_integration_test_targets(root, project_root)?;

    let normalized_manifest_text = toml::to_string(&runtime_manifest).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_invalid",
            format!("Normalized RuntimeModule manifest cannot be encoded: {error}"),
        )
    })?;
    let normalized_manifest_digest = sha256_prefixed(normalized_manifest_text.as_bytes());
    let dependency_bytes = serde_json::to_vec(&normalized_identities).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_invalid",
            format!("Normalized dependency identity cannot be encoded: {error}"),
        )
    })?;
    let normalized_dependency_digest = sha256_prefixed(&dependency_bytes);
    let runtime_cli_root = resolve_sdk_crate(&sdk_root, "runtime_cli", &manifest.engine_version)?;
    let runtime_player_winit_root =
        resolve_sdk_crate(&sdk_root, "runtime_player_winit", &manifest.engine_version)?;

    Ok(ProjectRuntimePlayerStagingPlan {
        manifest,
        sdk_root,
        runtime_cli_root,
        runtime_player_winit_root,
        source_manifest_text: runtime_manifest_text,
        normalized_manifest_text,
        normalized_manifest_digest,
        normalized_dependency_digest,
        normalized_dependencies: normalized_identities,
        dependency_lock_bytes,
        has_source_lock,
        trusted_lock_digest,
    })
}

fn validate_project_source_policy(
    project_root: &Path,
) -> Result<(), ProjectRuntimePlayerStagingError> {
    let forbidden_paths = [
        project_root.join("RuntimeModule/build.rs"),
        project_root.join(".cargo/config"),
        project_root.join(".cargo/config.toml"),
        project_root.join("RuntimeModule/.cargo/config"),
        project_root.join("RuntimeModule/.cargo/config.toml"),
    ];
    if let Some(path) = forbidden_paths.iter().find(|path| path.exists()) {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_policy_rejected",
            format!(
                "Production staging forbids project build scripts and Cargo config: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn add_integration_test_targets(
    root: &mut toml::map::Map<String, toml::Value>,
    project_root: &Path,
) -> Result<(), ProjectRuntimePlayerStagingError> {
    let tests_root = project_root.join("RuntimeModule/tests");
    if !tests_root.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&tests_root).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_tree_rejected",
            format!("RuntimeModule integration-test directory cannot be inspected: {error}"),
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_tree_rejected",
            "RuntimeModule integration-test root must be a regular directory.",
        ));
    }
    let mut entries = fs::read_dir(&tests_root)
        .map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!("RuntimeModule integration-test directory cannot be read: {error}"),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!("RuntimeModule integration-test entry cannot be read: {error}"),
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut targets = Vec::new();
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!("RuntimeModule integration-test entry cannot be inspected: {error}"),
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!(
                    "RuntimeModule integration-test entry cannot be a link: {}",
                    path.display()
                ),
            ));
        }
        if !metadata.is_file() || path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_source_tree_rejected",
                    "RuntimeModule integration-test filename must be UTF-8.",
                )
            })?;
        let mut target = toml::map::Map::new();
        target.insert("name".to_string(), toml::Value::String(name.to_string()));
        target.insert(
            "path".to_string(),
            toml::Value::String(format!("../RuntimeModule/tests/{name}.rs")),
        );
        targets.push(toml::Value::Table(target));
    }
    if !targets.is_empty() {
        root.insert("test".to_string(), toml::Value::Array(targets));
    }
    Ok(())
}

pub(crate) fn stage_project_runtime_player_source(
    project_root: &Path,
    destination_root: &Path,
    plan: &ProjectRuntimePlayerStagingPlan,
) -> Result<(), ProjectRuntimePlayerStagingError> {
    let destination = destination_root.join("RuntimeModule");
    let build_destination = destination_root.join("RuntimeModuleBuild");
    copy_runtime_tree(project_root, destination_root, project_root)?;
    fs::write(destination.join("Cargo.toml"), &plan.source_manifest_text).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_normalized_manifest_write_failed",
            format!("Normalized RuntimeModule manifest cannot be written: {error}"),
        )
    })?;
    fs::create_dir_all(&build_destination).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_normalized_manifest_write_failed",
            format!("Normalized RuntimeModule build directory cannot be created: {error}"),
        )
    })?;
    fs::write(
        build_destination.join("Cargo.toml"),
        &plan.normalized_manifest_text,
    )
    .map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_normalized_manifest_write_failed",
            format!("Normalized RuntimeModule build manifest cannot be written: {error}"),
        )
    })?;
    fs::write(
        build_destination.join("Cargo.lock"),
        &plan.dependency_lock_bytes,
    )
    .map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_normalized_manifest_write_failed",
            format!("Normalized RuntimeModule lock seed cannot be written: {error}"),
        )
    })
}

fn validate_root_policy(
    root: &toml::map::Map<String, toml::Value>,
) -> Result<(), ProjectRuntimePlayerStagingError> {
    for forbidden in [
        "workspace",
        "patch",
        "replace",
        "target",
        "build-dependencies",
        "dev-dependencies",
        "features",
        "bin",
        "example",
        "test",
        "bench",
    ] {
        if root.contains_key(forbidden) {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                format!("Cargo manifest field/table '{forbidden}' is forbidden in production staging v1."),
            ));
        }
    }
    for key in root.keys() {
        if !matches!(key.as_str(), "package" | "lib" | "dependencies") {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                format!("Cargo manifest root field/table '{key}' is unsupported."),
            ));
        }
    }
    Ok(())
}

fn validate_package_policy(
    root: &toml::map::Map<String, toml::Value>,
    manifest: &ProjectManifest,
) -> Result<(), ProjectRuntimePlayerStagingError> {
    let package = root
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                "Cargo [package] is required.",
            )
        })?;
    if package.get("name").and_then(toml::Value::as_str)
        != Some(manifest.runtime_module.cargo_package.as_str())
    {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_policy_rejected",
            "Cargo package.name does not match project runtimeModule.cargoPackage.",
        ));
    }
    for forbidden in ["build", "links", "workspace"] {
        if package.contains_key(forbidden) {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                format!("Cargo package.{forbidden} is forbidden."),
            ));
        }
    }
    for key in package.keys() {
        if !matches!(
            key.as_str(),
            "name" | "version" | "edition" | "license" | "publish" | "description" | "authors"
        ) {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                format!("Cargo package.{key} is unsupported in production staging v1."),
            ));
        }
    }
    Ok(())
}

fn normalize_lib_policy(
    root: &mut toml::map::Map<String, toml::Value>,
) -> Result<(), ProjectRuntimePlayerStagingError> {
    let Some(lib) = root.get_mut("lib").and_then(toml::Value::as_table_mut) else {
        return Ok(());
    };
    if let Some(path) = lib.get("path").and_then(toml::Value::as_str) {
        if path.replace('\\', "/") != "src/lib.rs" {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                "Only the default Cargo lib path src/lib.rs is supported.",
            ));
        }
        lib.remove("path");
    }
    if lib.contains_key("crate-type") || lib.contains_key("proc-macro") {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_policy_rejected",
            "Cargo lib.crate-type and lib.proc-macro are forbidden.",
        ));
    }
    if lib.is_empty() {
        root.remove("lib");
    }
    Ok(())
}

fn parse_third_party_requirement(
    name: &str,
    value: &toml::Value,
) -> Result<(VersionReq, bool, Vec<String>), ProjectRuntimePlayerStagingError> {
    let (version, default_features, mut features) = match value {
        toml::Value::String(version) => (version.as_str(), true, Vec::new()),
        toml::Value::Table(table) => {
            for key in table.keys() {
                if !matches!(key.as_str(), "version" | "default-features" | "features") {
                    return Err(ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_manifest_policy_rejected",
                        format!("Dependency '{name}' field '{key}' is forbidden."),
                    ));
                }
            }
            let version = table
                .get("version")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_manifest_policy_rejected",
                        format!("Dependency '{name}' requires a crates.io version."),
                    )
                })?;
            let default_features = match table.get("default-features") {
                Some(value) => value.as_bool().ok_or_else(|| {
                    ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_manifest_invalid",
                        format!("Dependency '{name}' default-features must be boolean."),
                    )
                })?,
                None => true,
            };
            let features = table
                .get("features")
                .map(parse_features)
                .transpose()?
                .unwrap_or_default();
            (version, default_features, features)
        }
        _ => {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_policy_rejected",
                format!("Dependency '{name}' must be a version or controlled dependency table."),
            ));
        }
    };
    features.sort();
    features.dedup();
    let requirement = VersionReq::parse(version).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_invalid",
            format!("Dependency '{name}' version requirement is invalid: {error}"),
        )
    })?;
    Ok((requirement, default_features, features))
}

fn parse_features(value: &toml::Value) -> Result<Vec<String>, ProjectRuntimePlayerStagingError> {
    value
        .as_array()
        .ok_or_else(|| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_invalid",
                "Dependency features must be an array.",
            )
        })?
        .iter()
        .map(|feature| {
            feature.as_str().map(str::to_string).ok_or_else(|| {
                ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_manifest_invalid",
                    "Dependency feature names must be strings.",
                )
            })
        })
        .collect()
}

fn resolve_trusted_engine_dependency(
    sdk_root: &Path,
    runtime_module_root: &Path,
    name: &str,
    engine_version: &str,
    value: &toml::Value,
) -> Result<PathBuf, ProjectRuntimePlayerStagingError> {
    let crate_root = resolve_sdk_crate(sdk_root, name, engine_version)?;
    match value {
        toml::Value::String(requirement) => {
            let requirement = VersionReq::parse(requirement).map_err(|error| {
                ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_manifest_invalid",
                    format!("Engine dependency '{name}' version is invalid: {error}"),
                )
            })?;
            let version = Version::parse(engine_version).map_err(|error| {
                ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_manifest_invalid",
                    format!("Project engineVersion is invalid: {error}"),
                )
            })?;
            if !requirement.matches(&version) {
                return Err(ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_dependency_version_mismatch",
                    format!(
                        "Engine dependency '{name}' does not match engineVersion {engine_version}."
                    ),
                ));
            }
        }
        toml::Value::Table(table) => {
            for key in table.keys() {
                if !matches!(key.as_str(), "path" | "version") {
                    return Err(ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_engine_dependency_untrusted",
                        format!("Engine dependency '{name}' field '{key}' is unsupported."),
                    ));
                }
            }
            if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
                let declared = Path::new(path);
                let resolved = if declared.is_absolute() {
                    declared.to_path_buf()
                } else {
                    runtime_module_root.join(declared)
                };
                if resolved.canonicalize().ok().as_ref() != Some(&crate_root) {
                    return Err(ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_engine_dependency_untrusted",
                        format!("Engine dependency '{name}' path does not resolve to the trusted Engine SDK."),
                    ));
                }
            } else if let Some(version) = table.get("version").and_then(toml::Value::as_str) {
                let requirement = VersionReq::parse(version).map_err(|error| {
                    ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_manifest_invalid",
                        format!("Engine dependency '{name}' version is invalid: {error}"),
                    )
                })?;
                let engine_version = Version::parse(engine_version).map_err(|error| {
                    ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_manifest_invalid",
                        format!("Project engineVersion is invalid: {error}"),
                    )
                })?;
                if !requirement.matches(&engine_version) {
                    return Err(ProjectRuntimePlayerStagingError::new(
                        "project_runtime.player_artifact_staging_dependency_version_mismatch",
                        format!(
                            "Engine dependency '{name}' version does not match the trusted SDK."
                        ),
                    ));
                }
            } else {
                return Err(ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_engine_dependency_untrusted",
                    format!("Engine dependency '{name}' needs a trusted path or matching version."),
                ));
            }
        }
        _ => {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_engine_dependency_untrusted",
                format!("Engine dependency '{name}' has an unsupported declaration."),
            ));
        }
    }
    Ok(crate_root)
}

fn resolve_sdk_crate(
    sdk_root: &Path,
    name: &str,
    engine_version: &str,
) -> Result<PathBuf, ProjectRuntimePlayerStagingError> {
    let expected = sdk_root.join("crates").join(name);
    let canonical = canonical_directory(&expected, &format!("Engine SDK crate '{name}'"))?;
    if canonical.parent().and_then(Path::parent) != Some(sdk_root) {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_engine_dependency_untrusted",
            format!("Engine SDK crate '{name}' escaped the trusted SDK root."),
        ));
    }
    let manifest_text = read_regular_utf8(&canonical.join("Cargo.toml"))?;
    let manifest: toml::Value = toml::from_str(&manifest_text).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_invalid",
            format!("Engine SDK crate '{name}' manifest is invalid: {error}"),
        )
    })?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_manifest_invalid",
                format!("Engine SDK crate '{name}' has no package table."),
            )
        })?;
    if package.get("name").and_then(toml::Value::as_str) != Some(name)
        || package.get("version").and_then(toml::Value::as_str) != Some(engine_version)
    {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_dependency_version_mismatch",
            format!("Engine SDK crate '{name}' does not match engineVersion {engine_version}."),
        ));
    }
    Ok(canonical)
}

fn parse_trusted_lock(
    bytes: &[u8],
) -> Result<BTreeMap<String, Vec<LockedPackage>>, ProjectRuntimePlayerStagingError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_trusted_lock_missing",
            format!("Trusted Engine SDK Cargo.lock is not UTF-8: {error}"),
        )
    })?;
    let lock: toml::Value = toml::from_str(text).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_trusted_lock_missing",
            format!("Trusted Engine SDK Cargo.lock is invalid: {error}"),
        )
    })?;
    let mut packages = BTreeMap::<String, Vec<LockedPackage>>::new();
    for package in lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(table) = package.as_table() else {
            continue;
        };
        let (Some(name), Some(version), Some(source)) = (
            table.get("name").and_then(toml::Value::as_str),
            table.get("version").and_then(toml::Value::as_str),
            table.get("source").and_then(toml::Value::as_str),
        ) else {
            continue;
        };
        if !source.starts_with("registry+https://github.com/rust-lang/crates.io-index") {
            continue;
        }
        let version = Version::parse(version).map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_trusted_lock_missing",
                format!("Trusted lock package '{name}' has invalid version: {error}"),
            )
        })?;
        packages
            .entry(name.to_string())
            .or_default()
            .push(LockedPackage {
                version,
                source: source.to_string(),
                checksum: table
                    .get("checksum")
                    .and_then(toml::Value::as_str)
                    .map(str::to_string),
            });
    }
    Ok(packages)
}

fn resolve_locked_package<'a>(
    packages: &'a BTreeMap<String, Vec<LockedPackage>>,
    name: &str,
    requirement: &VersionReq,
) -> Result<&'a LockedPackage, ProjectRuntimePlayerStagingError> {
    let matches = packages
        .get(name)
        .into_iter()
        .flatten()
        .filter(|package| requirement.matches(&package.version))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_trusted_lock_dependency_missing",
            format!(
                "Trusted Engine SDK lock must contain exactly one '{name}' package matching {requirement}; found {}.",
                matches.len()
            ),
        ));
    }
    Ok(matches[0])
}

fn copy_runtime_tree(
    source_root: &Path,
    destination_root: &Path,
    directory: &Path,
) -> Result<(), ProjectRuntimePlayerStagingError> {
    let metadata = fs::symlink_metadata(directory).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_tree_rejected",
            format!("RuntimeModule source cannot be inspected: {error}"),
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_tree_rejected",
            "RuntimeModule source must be a regular directory.",
        ));
    }
    fs::create_dir_all(destination_root).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_tree_rejected",
            format!("RuntimeModule staging directory cannot be created: {error}"),
        )
    })?;
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!("RuntimeModule source cannot be read: {error}"),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!("RuntimeModule source entry cannot be read: {error}"),
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source = entry.path();
        let relative = source.strip_prefix(source_root).map_err(|_| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                "RuntimeModule source escaped its root.",
            )
        })?;
        if relative.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|name| {
                matches!(name, "target" | ".git" | ".cargo" | ".aife" | "Build")
            })
        }) || relative.file_name().and_then(|name| name.to_str()) == Some(".gitignore")
        {
            continue;
        }
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!("RuntimeModule source metadata cannot be read: {error}"),
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                format!("RuntimeModule source contains a link: {}", source.display()),
            ));
        }
        let destination = destination_root.join(relative);
        if metadata.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| {
                ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_source_tree_rejected",
                    format!("RuntimeModule staging directory cannot be created: {error}"),
                )
            })?;
            copy_runtime_tree(source_root, destination_root, &source)?;
        } else if metadata.is_file() {
            fs::copy(&source, &destination).map_err(|error| {
                ProjectRuntimePlayerStagingError::new(
                    "project_runtime.player_artifact_staging_source_tree_rejected",
                    format!("RuntimeModule source cannot be copied: {error}"),
                )
            })?;
        } else {
            return Err(ProjectRuntimePlayerStagingError::new(
                "project_runtime.player_artifact_staging_source_tree_rejected",
                "RuntimeModule source contains a special file.",
            ));
        }
    }
    Ok(())
}

fn canonical_directory(
    path: &Path,
    label: &str,
) -> Result<PathBuf, ProjectRuntimePlayerStagingError> {
    let canonical = path.canonicalize().map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_engine_dependency_untrusted",
            format!("{label} cannot be resolved: {error}"),
        )
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_engine_dependency_untrusted",
            format!("{label} cannot be inspected: {error}"),
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_engine_dependency_untrusted",
            format!("{label} must be a regular directory."),
        ));
    }
    Ok(canonical)
}

fn read_regular_utf8(path: &Path) -> Result<String, ProjectRuntimePlayerStagingError> {
    let bytes = read_regular_bytes(path)?;
    String::from_utf8(bytes).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_manifest_invalid",
            format!("File is not UTF-8 ({}): {error}", path.display()),
        )
    })
}

fn read_regular_bytes(path: &Path) -> Result<Vec<u8>, ProjectRuntimePlayerStagingError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_trusted_lock_missing",
            format!(
                "Required regular file is unavailable ({}): {error}",
                path.display()
            ),
        )
    })?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_tree_rejected",
            format!("Required file is not regular: {}", path.display()),
        ));
    }
    fs::read(path).map_err(|error| {
        ProjectRuntimePlayerStagingError::new(
            "project_runtime.player_artifact_staging_source_tree_rejected",
            format!("Required file cannot be read ({}): {error}", path.display()),
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn project_runtime_player_production_staging_normalizes_trusted_and_crates_io_dependencies() {
        let root = temp_root("production-staging");
        let project = root.join("project");
        let staged = root.join("staged");
        let runtime = project.join("RuntimeModule");
        fs::create_dir_all(runtime.join("src")).unwrap();
        fs::create_dir_all(runtime.join("tests")).unwrap();
        fs::create_dir_all(runtime.join("target/debug")).unwrap();
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let engine_runtime = sdk
            .join("crates/engine_runtime")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        let cargo = format!(
            r#"[package]
name = "fixture_production_runtime"
version = "0.0.1"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
engine_runtime = {{ path = "{engine_runtime}" }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#
        );
        let project_manifest = serde_json::json!({
            "schemaVersion": "aife-project.v2",
            "projectId": "fixture.production.staging",
            "projectName": "Fixture",
            "engineVersion": "0.0.1",
            "createdAt": "0",
            "lastOpenedAt": null,
            "defaultScene": "Scenes/Main.scene.json",
            "assetRoot": "Assets",
            "settingsVersion": "aife-project-settings.v1",
            "runtimeModule": {
                "sourceKind": "projectRust",
                "moduleId": "fixture.production.runtime",
                "interfaceVersion": "project-runtime-module.v2",
                "cargoManifest": "RuntimeModule/Cargo.toml",
                "cargoPackage": "fixture_production_runtime",
                "playerBinary": "fixture_production_player"
            }
        });
        fs::write(runtime.join("Cargo.toml"), cargo.as_bytes()).unwrap();
        fs::write(
            runtime.join("Cargo.lock"),
            fs::read(sdk.join("Cargo.lock")).unwrap(),
        )
        .unwrap();
        fs::write(
            runtime.join("src/lib.rs"),
            br#"use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct Fixture { pub value: serde_json::Value }
"#,
        )
        .unwrap();
        fs::write(
            runtime.join("tests/production_contract.rs"),
            b"#[test]\nfn production_contract() {}\n",
        )
        .unwrap();
        fs::write(runtime.join("target/debug/stale.json"), b"stale").unwrap();
        fs::write(
            project.join("project.aife.json"),
            serde_json::to_vec_pretty(&project_manifest).unwrap(),
        )
        .unwrap();
        let source_manifest_before = fs::read(runtime.join("Cargo.toml")).unwrap();
        let source_lock_before = fs::read(runtime.join("Cargo.lock")).unwrap();

        let plan = plan_project_runtime_player_staging(&project, &sdk).unwrap();
        stage_project_runtime_player_source(&project, &staged, &plan).unwrap();

        assert_eq!(
            fs::read(runtime.join("Cargo.toml")).unwrap(),
            source_manifest_before
        );
        assert_eq!(
            fs::read(runtime.join("Cargo.lock")).unwrap(),
            source_lock_before
        );
        assert!(!staged.join("RuntimeModule/target").exists());
        assert_eq!(
            fs::read(staged.join("RuntimeModule/Cargo.lock")).unwrap(),
            plan.dependency_lock_bytes
        );
        assert_eq!(
            fs::read(staged.join("RuntimeModule/Cargo.toml")).unwrap(),
            source_manifest_before
        );
        let engine_identity = plan
            .normalized_dependencies
            .iter()
            .find(|dependency| dependency.name == "engine_runtime")
            .unwrap();
        assert_eq!(
            engine_identity.source_identity,
            sdk.join("crates/engine_runtime")
                .canonicalize()
                .unwrap()
                .display()
                .to_string()
        );
        let serde_identity = plan
            .normalized_dependencies
            .iter()
            .find(|dependency| dependency.name == "serde")
            .unwrap();
        assert_eq!(serde_identity.resolved_version, "1.0.228");
        let serde_json_identity = plan
            .normalized_dependencies
            .iter()
            .find(|dependency| dependency.name == "serde_json")
            .unwrap();
        assert_eq!(serde_json_identity.resolved_version, "1.0.150");
        let normalized: toml::Value = toml::from_str(
            &fs::read_to_string(staged.join("RuntimeModuleBuild/Cargo.toml")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            normalized["dependencies"]["engine_runtime"]["path"].as_str(),
            Some(engine_identity.source_identity.as_str())
        );
        assert_eq!(
            normalized["lib"]["path"].as_str(),
            Some("../RuntimeModule/src/lib.rs")
        );
        assert_eq!(
            normalized["test"][0]["path"].as_str(),
            Some("../RuntimeModule/tests/production_contract.rs")
        );
        assert!(!staged.join(".cargo").exists());
        assert_eq!(plan.normalized_dependencies.len(), 3);
        assert!(plan.normalized_manifest_digest.starts_with("sha256:"));
        assert!(plan.normalized_dependency_digest.starts_with("sha256:"));
        assert!(plan.trusted_lock_digest.starts_with("sha256:"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_runtime_player_production_staging_rejects_uncontrolled_dependency() {
        let root = temp_root("production-staging-reject");
        let project = root.join("project");
        let runtime = project.join("RuntimeModule");
        fs::create_dir_all(runtime.join("src")).unwrap();
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        fs::write(
            runtime.join("Cargo.toml"),
            format!(
                "[package]\nname='fixture_reject_runtime'\nversion='0.0.1'\nedition='2021'\n\
                 [dependencies]\nengine_runtime={{path='{}'}}\nreqwest='0.12'\n",
                sdk.join("crates/engine_runtime")
                    .canonicalize()
                    .unwrap()
                    .display()
                    .to_string()
                    .replace('\\', "/")
            ),
        )
        .unwrap();
        fs::write(runtime.join("src/lib.rs"), b"pub fn linked_set() {}\n").unwrap();
        fs::write(
            project.join("project.aife.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "aife-project.v2",
                "projectId": "fixture.production.reject",
                "projectName": "Fixture",
                "engineVersion": "0.0.1",
                "createdAt": "0",
                "lastOpenedAt": null,
                "defaultScene": "Scenes/Main.scene.json",
                "assetRoot": "Assets",
                "settingsVersion": "aife-project-settings.v1",
                "runtimeModule": {
                    "sourceKind": "projectRust",
                    "moduleId": "fixture.production.reject",
                    "interfaceVersion": "project-runtime-module.v2",
                    "cargoManifest": "RuntimeModule/Cargo.toml",
                    "cargoPackage": "fixture_reject_runtime",
                    "playerBinary": "fixture_reject_player"
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let error = plan_project_runtime_player_staging(&project, &sdk).unwrap_err();
        assert_eq!(
            error.code,
            "project_runtime.player_artifact_staging_third_party_dependency_unsupported"
        );

        let engine_runtime = sdk
            .join("crates/engine_runtime")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        let manifest_with = |extra_package: &str, extra_root: &str, dependency: &str| {
            format!(
                "[package]\nname='fixture_reject_runtime'\nversion='0.0.1'\nedition='2021'\n{extra_package}\n\
                 [dependencies]\nengine_runtime={{path='{engine_runtime}'}}\n{dependency}\n{extra_root}\n"
            )
        };
        for cargo in [
            manifest_with("", "", "serde={path='../serde'}"),
            manifest_with("", "", "serde={git='https://example.invalid/serde'}"),
            manifest_with("build='build.rs'", "", ""),
            manifest_with("", "[target.'cfg(windows)'.dependencies]", ""),
        ] {
            fs::write(runtime.join("Cargo.toml"), cargo).unwrap();
            assert_eq!(
                plan_project_runtime_player_staging(&project, &sdk)
                    .unwrap_err()
                    .code,
                "project_runtime.player_artifact_staging_manifest_policy_rejected"
            );
        }

        fs::write(runtime.join("Cargo.toml"), manifest_with("", "", "")).unwrap();
        fs::write(runtime.join("build.rs"), "fn main() {}\n").unwrap();
        assert_eq!(
            plan_project_runtime_player_staging(&project, &sdk)
                .unwrap_err()
                .code,
            "project_runtime.player_artifact_staging_source_policy_rejected"
        );
        fs::remove_file(runtime.join("build.rs")).unwrap();

        fs::create_dir_all(project.join(".cargo")).unwrap();
        fs::write(project.join(".cargo/config.toml"), "[net]\noffline=true\n").unwrap();
        assert_eq!(
            plan_project_runtime_player_staging(&project, &sdk)
                .unwrap_err()
                .code,
            "project_runtime.player_artifact_staging_source_policy_rejected"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_runtime_player_production_staging_resolves_relative_sdk_from_arbitrary_build_root() {
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let project = sdk
            .join("../samples/tower_defense_project")
            .canonicalize()
            .unwrap();
        let source_manifest = fs::read_to_string(project.join(RUNTIME_MANIFEST_PATH)).unwrap();
        assert!(source_manifest.contains("../../../rust/crates/engine_runtime"));

        let staged = temp_root("production-staging-relative-sdk");
        let plan = plan_project_runtime_player_staging(&project, &sdk).unwrap();
        stage_project_runtime_player_source(&project, &staged, &plan).unwrap();
        let normalized = fs::read_to_string(staged.join("RuntimeModuleBuild/Cargo.toml")).unwrap();
        assert!(normalized.contains(
            &sdk.join("crates/engine_runtime")
                .canonicalize()
                .unwrap()
                .display()
                .to_string()
        ));

        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .args([
                "metadata",
                "--manifest-path",
                "RuntimeModuleBuild/Cargo.toml",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version",
                "1",
            ])
            .current_dir(&staged)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "normalized staging metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = fs::remove_dir_all(staged);
    }

    #[test]
    fn project_editor_composition_staging_reuses_player_production_owner() {
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let project = sdk
            .join("../samples/tower_defense_project")
            .canonicalize()
            .unwrap();
        let staged = temp_root("editor-composition-shared-staging");

        let plan = ProjectRuntimeProductionStaging::plan(&project, &sdk).unwrap();
        ProjectRuntimeProductionStaging::stage(&project, &staged, &plan).unwrap();

        assert_eq!(
            fs::read(staged.join("RuntimeModule/Cargo.toml")).unwrap(),
            fs::read(project.join("RuntimeModule/Cargo.toml")).unwrap()
        );
        assert!(staged.join("RuntimeModuleBuild/Cargo.toml").is_file());
        assert!(staged.join("RuntimeModuleBuild/Cargo.lock").is_file());
        assert!(!staged.join(".cargo").exists());
        assert!(!staged.join("RuntimeModule/target").exists());
        assert!(plan.normalized_manifest_digest.starts_with("sha256:"));
        assert!(plan.normalized_dependency_digest.starts_with("sha256:"));
        assert!(plan.trusted_lock_digest.starts_with("sha256:"));
        assert!(plan
            .normalized_dependencies
            .iter()
            .any(|dependency| dependency.name == "serde"));
        assert!(plan
            .normalized_dependencies
            .iter()
            .any(|dependency| dependency.name == "serde_json"));
        let _ = fs::remove_dir_all(staged);
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()))
    }
}
