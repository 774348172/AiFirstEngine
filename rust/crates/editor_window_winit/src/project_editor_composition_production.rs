use crate::{
    ApprovedProjectRuntimeTrustRequest, NativeEditorApplication, PreparedProjectRuntime,
    ProjectRuntimePreparationAdapter, ProjectRuntimePreparationPhase,
    ProjectRuntimeTrustEnvironment,
};
#[cfg(test)]
use editor_core::{
    ProjectEditorCompositionDiagnostic, ProjectEditorCompositionIdentity,
    PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
};
use editor_core::{
    ProjectManifest, ProjectNativeModuleIdentity, ProjectRuntimeNativeModuleBuildControl,
    ProjectRuntimeNativeModuleBuildRequest, ProjectRuntimeNativeModuleBuildStatus,
    ProjectRuntimeNativeModuleBuilder, ProjectRuntimeNativeModuleDiagnostic,
    ProjectRuntimeNativeModuleLoader, ProjectRuntimeTrustInspection, ProjectRuntimeTrustModule,
    PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION,
    PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::{
    project_runtime_aot_digest, ProjectRuntimeAotDigestSource,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

pub const PROJECT_EDITOR_COMPOSITION_STATE_ROOT_ENV: &str =
    "AIFE_PROJECT_EDITOR_COMPOSITION_STATE_ROOT";

pub struct NativeProjectRuntimePreparer {
    engine_sdk_root: PathBuf,
    build_root: PathBuf,
    trust_host_identity: String,
}

impl NativeProjectRuntimePreparer {
    pub fn new(engine_sdk_root: PathBuf, build_root: PathBuf, trust_host_identity: String) -> Self {
        Self {
            engine_sdk_root,
            build_root,
            trust_host_identity,
        }
    }
}

impl ProjectRuntimePreparationAdapter for NativeProjectRuntimePreparer {
    fn prepare(
        &self,
        approved: ApprovedProjectRuntimeTrustRequest,
        control: ProjectRuntimeNativeModuleBuildControl,
        progress: &mut dyn FnMut(ProjectRuntimePreparationPhase),
    ) -> Result<PreparedProjectRuntime, ProjectRuntimeNativeModuleDiagnostic> {
        progress(ProjectRuntimePreparationPhase::RevalidatingTrust);
        let inspection = ProjectRuntimeTrustInspection::inspect(
            &approved.project_root,
            &self.engine_sdk_root,
            self.trust_host_identity.clone(),
        )
        .map_err(|error| native_diagnostic(error.code, "trust_revalidate", error.message, None))?;
        if inspection.request != approved.trust_request {
            return Err(native_diagnostic(
                "project_runtime.trust_stale",
                "trust_revalidate",
                "Project Runtime identity changed after approval and before native module preparation.",
                Some(&approved.project_root),
            ));
        }
        let identity = native_module_identity(&approved.project_root, &inspection)?;
        let manifest: ProjectManifest = serde_json::from_slice(
            &fs::read(approved.project_root.join("project.aife.json")).map_err(|error| {
                native_diagnostic(
                    "project_runtime.manifest_read_failed",
                    "identity",
                    error.to_string(),
                    Some(&approved.project_root),
                )
            })?,
        )
        .map_err(|error| {
            native_diagnostic(
                "project_runtime.manifest_invalid",
                "identity",
                error.to_string(),
                Some(&approved.project_root),
            )
        })?;
        let manifest_path = approved
            .project_root
            .join(&manifest.runtime_module.cargo_manifest);
        let source_crate_root = manifest_path.parent().ok_or_else(|| {
            native_diagnostic(
                "project_runtime.cargo_manifest_parent_missing",
                "identity",
                "RuntimeModule Cargo manifest has no parent directory.",
                Some(&manifest_path),
            )
        })?;
        progress(ProjectRuntimePreparationPhase::PreparingArtifact);
        let report = ProjectRuntimeNativeModuleBuilder::prepare_cancellable(
            &ProjectRuntimeNativeModuleBuildRequest {
                source_crate_root: source_crate_root.to_path_buf(),
                engine_sdk_root: self.engine_sdk_root.clone(),
                build_root: self.build_root.clone(),
                identity: identity.clone(),
                cargo_executable: None,
                metadata_hard_deadline_ms: 120_000,
                build_hard_deadline_ms: 1_200_000,
                capture_limit_bytes: 1024 * 1024,
            },
            control,
        );
        if report.status != ProjectRuntimeNativeModuleBuildStatus::Success {
            return Err(report.diagnostics.into_iter().next().unwrap_or_else(|| {
                native_diagnostic(
                    "project_runtime.native_module_build_failed",
                    "prepare",
                    "Native project runtime build failed without a diagnostic.",
                    Some(source_crate_root),
                )
            }));
        }
        let artifact = report.artifact.ok_or_else(|| {
            native_diagnostic(
                "project_runtime.native_module_artifact_missing",
                "prepare",
                "Successful native project runtime build returned no artifact.",
                Some(source_crate_root),
            )
        })?;
        progress(ProjectRuntimePreparationPhase::LoadingModule);
        let loaded = ProjectRuntimeNativeModuleLoader::load(&artifact)?;
        let linked = engine_runtime::project_runtime_module::LinkedProjectRuntimeSet::singleton(
            Arc::new(loaded),
        )
        .map_err(|error| {
            native_diagnostic(
                error.code,
                "link_module",
                error.message,
                Some(&artifact.dll_path),
            )
        })?;
        Ok(PreparedProjectRuntime {
            identity,
            linked_project_runtimes: Arc::new(linked),
        })
    }
}

pub fn install_project_runtime_production_services(
    app: &mut NativeEditorApplication,
    state_root: &Path,
    engine_sdk_root: PathBuf,
) -> Result<(), String> {
    let trust_root = state_root.join("project-runtime-trust");
    let build_root = state_root.to_path_buf();
    fs::create_dir_all(&build_root).map_err(|error| error.to_string())?;
    let trust_module =
        ProjectRuntimeTrustModule::open(&trust_root).map_err(|error| error.to_string())?;
    let trust_host_identity = project_runtime_abi_identity();
    app.install_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
        trust_module,
        engine_sdk_root: engine_sdk_root.clone(),
        // The v1 receipt field retains its serialized name, but 292 binds it to the stable ABI
        // identity so Editor implementation-only updates do not invalidate project trust.
        editor_build_identity: trust_host_identity.clone(),
    });
    app.install_project_runtime_preparer(Arc::new(NativeProjectRuntimePreparer::new(
        engine_sdk_root,
        build_root,
        trust_host_identity,
    )));
    Ok(())
}

pub fn default_project_editor_composition_state_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("AI First Engine")
        .join("Editor")
}

pub fn project_editor_composition_state_root() -> Result<PathBuf, String> {
    resolve_project_editor_composition_state_root(std::env::var_os(
        PROJECT_EDITOR_COMPOSITION_STATE_ROOT_ENV,
    ))
}

fn resolve_project_editor_composition_state_root(
    override_value: Option<OsString>,
) -> Result<PathBuf, String> {
    let Some(value) = override_value else {
        return Ok(default_project_editor_composition_state_root());
    };
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.file_name().is_none()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(format!(
            "project_editor_composition.state_root_override_invalid: {}",
            path.display()
        ));
    }
    Ok(path)
}

pub fn current_editor_build_identity() -> Result<String, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    fs::read(executable)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| error.to_string())
}

fn project_runtime_abi_identity() -> String {
    format!(
        "sha256:{}",
        project_runtime_abi::project_runtime_abi_digest_hex()
    )
}

fn native_module_identity(
    project_root: &Path,
    inspection: &ProjectRuntimeTrustInspection,
) -> Result<ProjectNativeModuleIdentity, ProjectRuntimeNativeModuleDiagnostic> {
    let manifest_path = project_root.join("project.aife.json");
    let manifest: ProjectManifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
            native_diagnostic(
                "project_runtime.manifest_read_failed",
                "identity",
                error.to_string(),
                Some(&manifest_path),
            )
        })?)
        .map_err(|error| {
            native_diagnostic(
                "project_runtime.manifest_invalid",
                "identity",
                error.to_string(),
                Some(&manifest_path),
            )
        })?;
    let cargo_manifest_path = project_root.join(&manifest.runtime_module.cargo_manifest);
    let module_root = cargo_manifest_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            native_diagnostic(
                "project_runtime.cargo_manifest_parent_missing",
                "identity",
                "RuntimeModule Cargo manifest has no parent directory.",
                Some(&cargo_manifest_path),
            )
        })?;
    let lock_path = module_root.join("Cargo.lock");
    let lock_bytes = fs::read(&lock_path).map_err(|error| {
        native_diagnostic(
            "project_runtime.lock_read_failed",
            "identity",
            error.to_string(),
            Some(&lock_path),
        )
    })?;
    let mut sources = vec![cargo_manifest_path, lock_path.clone()];
    collect_native_rust_sources(project_root, &module_root.join("src"), &mut sources)?;
    sources.sort();
    sources.dedup();
    let source_bytes = sources
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(project_root).map_err(|_| {
                native_diagnostic(
                    "project_runtime.source_outside_project",
                    "identity",
                    "RuntimeModule source escaped the project root.",
                    Some(&path),
                )
            })?;
            let bytes = fs::read(&path).map_err(|error| {
                native_diagnostic(
                    "project_runtime.source_read_failed",
                    "identity",
                    error.to_string(),
                    Some(&path),
                )
            })?;
            Ok((relative.to_string_lossy().replace('\\', "/"), bytes))
        })
        .collect::<Result<Vec<_>, ProjectRuntimeNativeModuleDiagnostic>>()?;
    let aot_content_digest = project_runtime_aot_digest(
        &manifest.runtime_module.module_id,
        &manifest.runtime_module.interface_version,
        &manifest.runtime_module.cargo_manifest,
        &manifest.runtime_module.cargo_package,
        &manifest.runtime_module.player_binary,
        source_bytes
            .iter()
            .map(|(relative_path, bytes)| ProjectRuntimeAotDigestSource {
                relative_path,
                bytes,
            }),
    )
    .map_err(|error| {
        native_diagnostic(
            "project_runtime.aot_identity_failed",
            "identity",
            error.to_string(),
            Some(project_root),
        )
    })?;
    Ok(ProjectNativeModuleIdentity {
        schema_version: PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION.to_string(),
        project_runtime_abi_digest: project_runtime_abi_identity(),
        project_runtime_sdk_digest: format!(
            "sha256:{}",
            project_runtime_sdk::project_runtime_contract_digest_hex()
        ),
        project_id: manifest.project_id,
        module_id: manifest.runtime_module.module_id,
        logical_interface_version: manifest.runtime_module.interface_version,
        aot_content_digest,
        normalized_manifest_digest: inspection.request.normalized_manifest_digest.clone(),
        normalized_dependency_digest: inspection.request.normalized_dependency_digest.clone(),
        dependency_lock_digest: sha256_prefixed(&lock_bytes),
        toolchain_identity: native_rustc_identity()?,
        target_triple: "host".to_string(),
        profile: "release".to_string(),
        features: Vec::new(),
        builder_schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION.to_string(),
    })
}

fn collect_native_rust_sources(
    project_root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), ProjectRuntimeNativeModuleDiagnostic> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            native_diagnostic(
                "project_runtime.source_read_failed",
                "identity",
                error.to_string(),
                Some(directory),
            )
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            native_diagnostic(
                "project_runtime.source_read_failed",
                "identity",
                error.to_string(),
                Some(directory),
            )
        })?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            native_diagnostic(
                "project_runtime.source_read_failed",
                "identity",
                error.to_string(),
                Some(&path),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(native_diagnostic(
                "project_runtime.source_link_rejected",
                "identity",
                "RuntimeModule source links are not allowed.",
                Some(&path),
            ));
        }
        if metadata.is_dir() {
            collect_native_rust_sources(project_root, &path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            path.strip_prefix(project_root).map_err(|_| {
                native_diagnostic(
                    "project_runtime.source_outside_project",
                    "identity",
                    "RuntimeModule source escaped the project root.",
                    Some(&path),
                )
            })?;
            output.push(path);
        }
    }
    Ok(())
}

fn native_rustc_identity() -> Result<String, ProjectRuntimeNativeModuleDiagnostic> {
    let output = Command::new("rustc")
        .args(["--version", "--verbose"])
        .output()
        .map_err(|error| {
            native_diagnostic(
                "project_runtime.toolchain_unavailable",
                "identity",
                error.to_string(),
                None,
            )
        })?;
    if !output.status.success() {
        return Err(native_diagnostic(
            "project_runtime.toolchain_unavailable",
            "identity",
            "rustc --version --verbose failed.",
            None,
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| {
            native_diagnostic(
                "project_runtime.toolchain_unavailable",
                "identity",
                error.to_string(),
                None,
            )
        })
}

fn native_diagnostic(
    code: impl Into<String>,
    stage: impl Into<String>,
    message: impl Into<String>,
    path: Option<&Path>,
) -> ProjectRuntimeNativeModuleDiagnostic {
    ProjectRuntimeNativeModuleDiagnostic {
        code: code.into(),
        stage: stage.into(),
        message: message.into(),
        path: path.map(|value| value.display().to_string()),
        next_action: "Keep authoring available and repair the reported ProjectRuntime input."
            .to_string(),
    }
}

#[cfg(test)]
pub(crate) fn composition_identity(
    project_root: &Path,
    engine_sdk_root: &Path,
    inspection: &ProjectRuntimeTrustInspection,
    editor_build_identity: &str,
) -> Result<ProjectEditorCompositionIdentity, ProjectEditorCompositionDiagnostic> {
    let manifest: ProjectManifest = serde_json::from_slice(
        &fs::read(project_root.join("project.aife.json")).map_err(|error| {
            diagnostic(
                "project_editor_composition.manifest_read_failed",
                "identity",
                error.to_string(),
            )
        })?,
    )
    .map_err(|error| {
        diagnostic(
            "project_editor_composition.manifest_invalid",
            "identity",
            error.to_string(),
        )
    })?;
    let mut sources = vec![
        PathBuf::from("RuntimeModule/Cargo.toml"),
        PathBuf::from("RuntimeModule/Cargo.lock"),
    ];
    collect_rust_sources(
        project_root,
        &project_root.join("RuntimeModule/src"),
        &mut sources,
    )?;
    sources.sort();
    let source_bytes = sources
        .into_iter()
        .map(|relative| {
            fs::read(project_root.join(&relative))
                .map(|bytes| (relative.to_string_lossy().replace('\\', "/"), bytes))
                .map_err(|error| {
                    diagnostic(
                        "project_editor_composition.source_read_failed",
                        "identity",
                        error.to_string(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let aot_content_digest = project_runtime_aot_digest(
        &manifest.runtime_module.module_id,
        &manifest.runtime_module.interface_version,
        &manifest.runtime_module.cargo_manifest,
        &manifest.runtime_module.cargo_package,
        &manifest.runtime_module.player_binary,
        source_bytes
            .iter()
            .map(|(relative_path, bytes)| ProjectRuntimeAotDigestSource {
                relative_path,
                bytes,
            }),
    )
    .map_err(|error| {
        diagnostic(
            "project_editor_composition.aot_identity_failed",
            "identity",
            error.to_string(),
        )
    })?;
    let lock_bytes = fs::read(engine_sdk_root.join("Cargo.lock")).map_err(|error| {
        diagnostic(
            "project_editor_composition.engine_lock_read_failed",
            "identity",
            error.to_string(),
        )
    })?;
    let engine_sdk_digest = sha256_prefixed(&lock_bytes);
    let toolchain_identity = rustc_identity()?;
    Ok(ProjectEditorCompositionIdentity {
        schema_version: PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
        project_id: manifest.project_id,
        module_id: manifest.runtime_module.module_id,
        interface_version: manifest.runtime_module.interface_version,
        aot_content_digest,
        editor_build_identity: editor_build_identity.to_string(),
        engine_sdk_digest: engine_sdk_digest.clone(),
        toolchain_identity,
        target_triple: if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            "x86_64-pc-windows-msvc".to_string()
        } else {
            format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
        },
        profile: "release".to_string(),
        normalized_manifest_digest: inspection.request.normalized_manifest_digest.clone(),
        normalized_dependency_digest: inspection.request.normalized_dependency_digest.clone(),
        dependency_lock_digest: engine_sdk_digest,
    })
}

#[cfg(test)]
fn collect_rust_sources(
    project_root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), ProjectEditorCompositionDiagnostic> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            diagnostic(
                "project_editor_composition.source_read_failed",
                "identity",
                error.to_string(),
            )
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            diagnostic(
                "project_editor_composition.source_read_failed",
                "identity",
                error.to_string(),
            )
        })?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            diagnostic(
                "project_editor_composition.source_read_failed",
                "identity",
                error.to_string(),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(diagnostic(
                "project_editor_composition.source_link_rejected",
                "identity",
                format!(
                    "RuntimeModule source link is not allowed: {}",
                    path.display()
                ),
            ));
        }
        if metadata.is_dir() {
            collect_rust_sources(project_root, &path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path.strip_prefix(project_root).unwrap().to_path_buf());
        }
    }
    Ok(())
}

#[cfg(test)]
fn rustc_identity() -> Result<String, ProjectEditorCompositionDiagnostic> {
    let output = Command::new("rustc")
        .args(["--version", "--verbose"])
        .output()
        .map_err(|error| {
            diagnostic(
                "project_editor_composition.toolchain_unavailable",
                "identity",
                error.to_string(),
            )
        })?;
    if !output.status.success() {
        return Err(diagnostic(
            "project_editor_composition.toolchain_unavailable",
            "identity",
            "rustc --version --verbose failed.",
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| {
            diagnostic(
                "project_editor_composition.toolchain_unavailable",
                "identity",
                error.to_string(),
            )
        })
}

#[cfg(test)]
fn diagnostic(
    code: impl Into<String>,
    stage: impl Into<String>,
    message: impl Into<String>,
) -> ProjectEditorCompositionDiagnostic {
    ProjectEditorCompositionDiagnostic {
        code: code.into(),
        stage: stage.into(),
        message: message.into(),
        path: None,
        expected_identity: None,
        actual_identity: None,
        next_action: "Keep the current Editor open and repair the reported composition input."
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_editor_composition_state_root_override_requires_scoped_absolute_path() {
        assert!(
            resolve_project_editor_composition_state_root(Some(OsString::from("relative")))
                .unwrap_err()
                .starts_with("project_editor_composition.state_root_override_invalid")
        );
        let absolute = std::env::temp_dir().join("aife-262-state-root");
        assert_eq!(
            resolve_project_editor_composition_state_root(Some(absolute.clone().into_os_string()))
                .unwrap(),
            absolute
        );
    }
}
