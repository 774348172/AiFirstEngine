use crate::{
    runtime_package_digest, semantic_file_digest, ExportedPlayerProcessVerificationDiagnostic,
};
use engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor;
use engine_runtime::runtime_package::{
    RuntimeProjectModuleRef, LEGACY_RUNTIME_PACKAGE_SCHEMA_VERSION, RUNTIME_PACKAGE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_path::safe_join_runtime_package;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "desktop-package-manifest.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPackageManifest {
    pub schema_version: String,
    pub target: String,
    pub profile: String,
    pub package_dir: String,
    pub runtime_package_dir: String,
    pub reports_dir: String,
    pub player_executable: Option<String>,
    pub player_executable_status: String,
    pub player_artifact_build_report_path: Option<String>,
    pub player_artifact_hash: Option<String>,
    pub player_module_descriptor: Option<ProjectRuntimeModuleDescriptor>,
    #[serde(default)]
    pub engine_runtime_hash: Option<String>,
    #[serde(default)]
    pub project_runtime_module_hash: Option<String>,
    #[serde(default)]
    pub runtime_package_digest: Option<String>,
}

type Diagnostic = ExportedPlayerProcessVerificationDiagnostic;

pub fn project_runtime_module_relative_path(module_id: &str) -> String {
    let stem = module_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    format!("data/bin/{stem}.dll")
}

pub fn validate_desktop_dev_package(root: &Path) -> Result<DesktopPackageManifest, Diagnostic> {
    let path = checked_path(root, "package-manifest.json")?;
    let bytes = fs::read(&path)
        .map_err(|error| diagnostic("desktop_dev_manifest_invalid", &path, error.to_string()))?;
    let manifest = serde_json::from_slice(&bytes)
        .map_err(|error| diagnostic("desktop_dev_manifest_invalid", &path, error.to_string()))?;
    validate_desktop_dev_manifest(root, &manifest)?;
    Ok(manifest)
}

/// Validate the files of a dev delivery without starting a Player or loading its assets.
/// Historical absolute paths in the manifest describe provenance, not lookup locations.
pub fn validate_desktop_dev_manifest(
    root: &Path,
    manifest: &DesktopPackageManifest,
) -> Result<(), Diagnostic> {
    let manifest_path = root.join("package-manifest.json");
    if manifest.schema_version != DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION
        || manifest.target != "windows"
        || manifest.profile != "dev"
    {
        return Err(diagnostic(
            "desktop_dev_manifest_invalid",
            &manifest_path,
            "Desktop package manifest must target windows/dev.",
        ));
    }
    let required = [
        (
            "playerArtifactHash",
            manifest.player_artifact_hash.as_deref(),
        ),
        ("engineRuntimeHash", manifest.engine_runtime_hash.as_deref()),
        (
            "projectRuntimeModuleHash",
            manifest.project_runtime_module_hash.as_deref(),
        ),
        (
            "runtimePackageDigest",
            manifest.runtime_package_digest.as_deref(),
        ),
    ];
    for (name, value) in required {
        if value.is_none_or(str::is_empty) {
            return Err(diagnostic(
                "desktop_dev_identity_missing",
                &manifest_path,
                format!("Desktop package is missing {name}; export the project again."),
            ));
        }
    }
    let descriptor = manifest
        .player_module_descriptor
        .as_ref()
        .filter(|descriptor| {
            !descriptor.module_id.is_empty()
                && !descriptor.interface_version.is_empty()
                && !descriptor.aot_content_digest.is_empty()
        })
        .ok_or_else(|| {
            diagnostic(
                "desktop_dev_identity_missing",
                &manifest_path,
                "Desktop package is missing its module identity; export the project again.",
            )
        })?;
    for (name, value) in required {
        let value = value.unwrap_or_default();
        if !value.strip_prefix("sha256:").is_some_and(|hash| {
            hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(diagnostic(
                "desktop_dev_manifest_invalid",
                &manifest_path,
                format!("{name} must be a SHA-256 digest; export the project again."),
            ));
        }
    }

    for duplicate in ["data/data", "reports/reports"] {
        let path = checked_path(root, duplicate)?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(diagnostic(
                "desktop_dev_layout_invalid",
                &path,
                "Desktop package contains a duplicated managed output directory.",
            ));
        }
    }
    // Legacy loader candidates must not shadow the module whose hash we check.
    for legacy_module in [
        "data/project_runtime.dll",
        "data/runtime_package/project_runtime.dll",
    ] {
        let path = checked_path(root, legacy_module)?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(diagnostic(
                "desktop_dev_layout_invalid",
                &path,
                "Desktop delivery contains a legacy Project DLL outside its fixed module path.",
            ));
        }
    }
    let module_relative_path = project_runtime_module_relative_path(&descriptor.module_id);
    validate_project_dll_directory(root, &module_relative_path)?;
    for (relative, expected) in [
        ("Game.exe", manifest.player_artifact_hash.as_deref()),
        (
            "engine_runtime.dll",
            manifest.engine_runtime_hash.as_deref(),
        ),
        (
            module_relative_path.as_str(),
            manifest.project_runtime_module_hash.as_deref(),
        ),
    ] {
        let path = checked_path(root, relative)?;
        if !path.is_file() {
            return Err(diagnostic(
                "desktop_dev_file_missing",
                &path,
                "Required desktop runtime file is missing; export the project again.",
            ));
        }
        let actual = semantic_file_digest(&path)
            .map_err(|message| diagnostic("desktop_dev_file_missing", &path, message))?;
        if Some(actual.as_str()) != expected {
            return Err(diagnostic(
                "desktop_dev_file_hash_mismatch",
                &path,
                format!(
                    "Runtime file digest {actual} does not match {}.",
                    expected.unwrap_or_default()
                ),
            ));
        }
    }

    let runtime_manifest_path = checked_path(root, "data/runtime_package/manifest.json")?;
    let runtime_bytes = fs::read(&runtime_manifest_path).map_err(|error| {
        diagnostic(
            "desktop_dev_runtime_package_invalid",
            &runtime_manifest_path,
            error.to_string(),
        )
    })?;
    let runtime: serde_json::Value = serde_json::from_slice(&runtime_bytes).map_err(|error| {
        diagnostic(
            "desktop_dev_runtime_package_invalid",
            &runtime_manifest_path,
            error.to_string(),
        )
    })?;
    if !matches!(
        runtime
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str),
        Some(RUNTIME_PACKAGE_SCHEMA_VERSION | LEGACY_RUNTIME_PACKAGE_SCHEMA_VERSION)
    ) {
        return Err(diagnostic(
            "desktop_dev_runtime_package_invalid",
            &runtime_manifest_path,
            "RuntimePackage manifest has an unsupported schemaVersion.",
        ));
    }
    let runtime_module: RuntimeProjectModuleRef = serde_json::from_value(
        runtime
            .pointer("/project/runtimeModule")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|error| {
        diagnostic(
            "desktop_dev_runtime_package_invalid",
            &runtime_manifest_path,
            format!("RuntimePackage module identity is invalid: {error}"),
        )
    })?;
    if descriptor.module_id != runtime_module.module_id
        || descriptor.interface_version != runtime_module.interface_version
        || descriptor.aot_content_digest != runtime_module.aot_content_digest
    {
        return Err(diagnostic(
            "desktop_dev_module_mismatch",
            &runtime_manifest_path,
            "Desktop manifest module identity does not match RuntimePackage project.runtimeModule.",
        ));
    }
    let runtime_root = checked_path(root, "data/runtime_package")?;
    let actual = runtime_package_digest(&runtime_root).map_err(|message| {
        diagnostic(
            "desktop_dev_runtime_package_invalid",
            &runtime_root,
            message,
        )
    })?;
    if manifest.runtime_package_digest.as_deref() != Some(actual.as_str()) {
        return Err(diagnostic(
            "desktop_dev_runtime_package_digest_mismatch",
            &runtime_root,
            format!("RuntimePackage digest {actual} does not match the desktop manifest."),
        ));
    }
    Ok(())
}

fn validate_project_dll_directory(root: &Path, expected: &str) -> Result<(), Diagnostic> {
    let bin = checked_path(root, "data/bin")?;
    if !bin.is_dir() {
        return Err(diagnostic(
            "desktop_dev_file_missing",
            &bin,
            "Project DLL directory is missing.",
        ));
    }
    let mut directories = vec![bin];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| {
            diagnostic("desktop_dev_path_invalid", &directory, error.to_string())
        })? {
            let entry = entry.map_err(|error| {
                diagnostic("desktop_dev_path_invalid", &directory, error.to_string())
            })?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|error| diagnostic("desktop_dev_path_invalid", &path, error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            let path = checked_path(root, &relative)?;
            if path.is_dir() {
                directories.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
                && !relative.eq_ignore_ascii_case(expected)
            {
                return Err(diagnostic(
                    "desktop_dev_layout_invalid",
                    &path,
                    "Project DLL directory contains a module outside this delivery's identity.",
                ));
            }
        }
    }
    Ok(())
}

fn checked_path(root: &Path, relative: &str) -> Result<PathBuf, Diagnostic> {
    let path = safe_join_runtime_package(root, relative).map_err(|error| {
        diagnostic(
            "desktop_dev_path_invalid",
            &root.join(relative),
            error.to_string(),
        )
    })?;
    let mut current = root.to_path_buf();
    reject_link(&current)?;
    for segment in Path::new(relative).components() {
        current.push(segment);
        reject_link(&current)?;
    }
    Ok(path)
}

fn reject_link(path: &Path) -> Result<(), Diagnostic> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(diagnostic(
                "desktop_dev_path_invalid",
                path,
                error.to_string(),
            ))
        }
    };
    let linked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        linked || metadata.file_attributes() & 0x400 != 0
    };
    if linked {
        return Err(diagnostic(
            "desktop_dev_path_invalid",
            path,
            "Desktop runtime paths cannot contain symbolic links or reparse points.",
        ));
    }
    Ok(())
}

fn diagnostic(code: &str, path: &Path, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(code, message).with_path(path.display().to_string())
}
