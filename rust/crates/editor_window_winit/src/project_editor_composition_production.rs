use crate::{
    ApprovedProjectRuntimeTrustRequest, NativeEditorApplication,
    ProjectEditorCompositionPreparationAdapter, ProjectRuntimeTrustEnvironment,
    PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT,
};
use editor_core::{
    EditorCompositionCandidateProcessState, EditorCompositionClock, EditorCompositionExitAdapter,
    EditorCompositionProcessAdapter, EditorCompositionWorkspaceAdapter,
    ProjectEditorCompositionArtifact, ProjectEditorCompositionBuildRequest,
    ProjectEditorCompositionBuildStatus, ProjectEditorCompositionCachePolicy,
    ProjectEditorCompositionDiagnostic, ProjectEditorCompositionIdentity, ProjectManifest,
    ProjectRuntimeTrustInspection, ProjectRuntimeTrustModule,
    PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION,
    PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::{
    project_runtime_aot_digest, ProjectRuntimeAotDigestSource,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROJECT_EDITOR_COMPOSITION_STATE_ROOT_ENV: &str =
    "AIFE_PROJECT_EDITOR_COMPOSITION_STATE_ROOT";

pub struct NativeProjectEditorCompositionPreparer {
    engine_sdk_root: PathBuf,
    build_root: PathBuf,
    editor_build_identity: String,
}

impl NativeProjectEditorCompositionPreparer {
    pub fn new(
        engine_sdk_root: PathBuf,
        build_root: PathBuf,
        editor_build_identity: String,
    ) -> Self {
        Self {
            engine_sdk_root,
            build_root,
            editor_build_identity,
        }
    }
}

impl ProjectEditorCompositionPreparationAdapter for NativeProjectEditorCompositionPreparer {
    fn prepare(
        &self,
        approved: ApprovedProjectRuntimeTrustRequest,
        control: editor_core::ProjectEditorCompositionPreparationControl,
        progress: &mut dyn FnMut(editor_core::ProjectEditorCompositionPreparationPhase),
    ) -> Result<ProjectEditorCompositionArtifact, ProjectEditorCompositionDiagnostic> {
        let inspection = ProjectRuntimeTrustInspection::inspect(
            &approved.project_root,
            &self.engine_sdk_root,
            self.editor_build_identity.clone(),
        )
        .map_err(|error| diagnostic(error.code, "trust_revalidate", error.message))?;
        if inspection.request != approved.trust_request {
            return Err(diagnostic(
                "project_editor_composition.trust_stale",
                "trust_revalidate",
                "Project Runtime identity changed after approval and before composition preparation.",
            ));
        }
        let identity = composition_identity(
            &approved.project_root,
            &self.engine_sdk_root,
            &inspection,
            &self.editor_build_identity,
        )?;
        let report = ProjectEditorCompositionArtifact::prepare_with_progress(
            ProjectEditorCompositionBuildRequest {
                schema_version: PROJECT_EDITOR_COMPOSITION_BUILD_REQUEST_SCHEMA_VERSION.to_string(),
                project_root: approved.project_root,
                engine_sdk_root: self.engine_sdk_root.clone(),
                build_root: self.build_root.clone(),
                expected_identity: identity,
                cache_policy: ProjectEditorCompositionCachePolicy::default(),
                qos_policy: editor_core::ProjectEditorCompositionBuildQosPolicy::default(),
                deadline_policy: editor_core::ProjectEditorCompositionBuildDeadlinePolicy::default(
                ),
                cargo_executable: None,
                cargo_identity: cargo_identity()?,
                capture_limit_bytes: 256 * 1024,
            },
            control,
            progress,
        );
        if report.status != ProjectEditorCompositionBuildStatus::Success {
            return Err(report.diagnostics.into_iter().next().unwrap_or_else(|| {
                diagnostic(
                    "project_editor_composition.build_failed",
                    "prepare",
                    "Composition artifact preparation failed without a diagnostic.",
                )
            }));
        }
        report.artifact.ok_or_else(|| {
            diagnostic(
                "project_editor_composition.artifact_missing",
                "prepare",
                "Successful composition preparation returned no artifact.",
            )
        })
    }
}

pub struct NativeEditorCompositionClock;

impl EditorCompositionClock for NativeEditorCompositionClock {
    fn now_epoch_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }
}

pub struct NativeEditorCompositionWorkspaceAdapter {
    state_path: PathBuf,
}

impl NativeEditorCompositionWorkspaceAdapter {
    pub fn new(state_path: PathBuf) -> Self {
        Self { state_path }
    }
}

impl EditorCompositionWorkspaceAdapter for NativeEditorCompositionWorkspaceAdapter {
    fn save_recoverable_state(&self, project_root: &Path) -> Result<String, String> {
        let parent = self
            .state_path
            .parent()
            .ok_or_else(|| "workspace handoff state has no parent".to_string())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = self.state_path.with_extension("json.tmp");
        let payload = serde_json::json!({
            "schemaVersion": "project-editor-workspace-handoff-state.v1",
            "projectRoot": fs::canonicalize(project_root)
                .map_err(|error| error.to_string())?,
        });
        fs::write(&temporary, serde_json::to_vec_pretty(&payload).unwrap())
            .map_err(|error| error.to_string())?;
        fs::rename(&temporary, &self.state_path).map_err(|error| error.to_string())?;
        Ok(self.state_path.display().to_string())
    }
}

#[derive(Default)]
pub struct NativeEditorCompositionProcessAdapter {
    children: Mutex<BTreeMap<u32, Child>>,
}

impl EditorCompositionProcessAdapter for NativeEditorCompositionProcessAdapter {
    fn launch_candidate(&self, executable: &Path, ticket_path: &Path) -> Result<u32, String> {
        let child = Command::new(executable)
            .arg(PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT)
            .arg(ticket_path)
            .current_dir(
                executable
                    .parent()
                    .ok_or_else(|| "candidate executable has no parent".to_string())?,
            )
            .spawn()
            .map_err(|error| error.to_string())?;
        let process_id = child.id();
        self.children
            .lock()
            .map_err(|_| "candidate process registry is poisoned".to_string())?
            .insert(process_id, child);
        Ok(process_id)
    }

    fn candidate_state(
        &self,
        process_id: u32,
    ) -> Result<EditorCompositionCandidateProcessState, String> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| "candidate process registry is poisoned".to_string())?;
        let child = children
            .get_mut(&process_id)
            .ok_or_else(|| "candidate process is not owned by this launcher".to_string())?;
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => {
                let exit_code = status.code().unwrap_or(-1);
                children.remove(&process_id);
                Ok(EditorCompositionCandidateProcessState::Exited(exit_code))
            }
            None => Ok(EditorCompositionCandidateProcessState::Running),
        }
    }

    fn terminate_owned_candidate(&self, process_id: u32) -> Result<(), String> {
        let mut child = self
            .children
            .lock()
            .map_err(|_| "candidate process registry is poisoned".to_string())?
            .remove(&process_id)
            .ok_or_else(|| "candidate process is not owned by this launcher".to_string())?;
        child.kill().map_err(|error| error.to_string())?;
        child.wait().map_err(|error| error.to_string())?;
        Ok(())
    }
}

pub struct NativeEditorCompositionExitAdapter {
    requested: Arc<AtomicBool>,
}

impl NativeEditorCompositionExitAdapter {
    pub fn new(requested: Arc<AtomicBool>) -> Self {
        Self { requested }
    }
}

impl EditorCompositionExitAdapter for NativeEditorCompositionExitAdapter {
    fn request_graceful_exit(&self) -> Result<(), String> {
        self.requested.store(true, Ordering::Release);
        Ok(())
    }
}

pub fn install_project_editor_composition_production_services(
    app: &mut NativeEditorApplication,
    state_root: &Path,
    engine_sdk_root: PathBuf,
    editor_build_identity: String,
    graceful_exit_requested: Arc<AtomicBool>,
) -> Result<(), String> {
    let trust_root = state_root.join("project-runtime-trust");
    let build_root = state_root.to_path_buf();
    let handoff_root = state_root.join("project-editor-handoff");
    fs::create_dir_all(&build_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&handoff_root).map_err(|error| error.to_string())?;
    let trust_module =
        ProjectRuntimeTrustModule::open(&trust_root).map_err(|error| error.to_string())?;
    app.install_project_runtime_trust_environment(ProjectRuntimeTrustEnvironment {
        trust_module,
        engine_sdk_root: engine_sdk_root.clone(),
        editor_build_identity: editor_build_identity.clone(),
    });
    app.install_project_editor_composition_preparer(
        Arc::new(NativeProjectEditorCompositionPreparer::new(
            engine_sdk_root,
            build_root,
            editor_build_identity,
        )),
        handoff_root.clone(),
    );
    app.install_project_editor_composition_launcher(
        editor_core::EditorProjectCompositionLauncher::new(
            Arc::new(NativeEditorCompositionClock),
            Arc::new(NativeEditorCompositionWorkspaceAdapter::new(
                handoff_root.join("workspace-state.json"),
            )),
            Arc::new(NativeEditorCompositionProcessAdapter::default()),
            Arc::new(NativeEditorCompositionExitAdapter::new(
                graceful_exit_requested,
            )),
        ),
    );
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

fn cargo_identity() -> Result<String, ProjectEditorCompositionDiagnostic> {
    let output = Command::new("cargo")
        .args(["--version", "--verbose"])
        .output()
        .map_err(|error| {
            diagnostic(
                "project_editor_composition.cargo_unavailable",
                "identity",
                error.to_string(),
            )
        })?;
    if !output.status.success() {
        return Err(diagnostic(
            "project_editor_composition.cargo_unavailable",
            "identity",
            "cargo --version --verbose failed.",
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| {
            diagnostic(
                "project_editor_composition.cargo_unavailable",
                "identity",
                error.to_string(),
            )
        })
}

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
