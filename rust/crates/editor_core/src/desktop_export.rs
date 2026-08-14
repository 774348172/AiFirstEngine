use crate::{
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyDiagnostic,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblySeverity,
    ProjectRuntimePackageAssemblyStatus,
};
use engine_runtime::runtime_package::RuntimeProjectModuleRef;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationRequest,
    ExportedPlayerProcessVerificationStatus,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DESKTOP_EXPORT_REPORT_SCHEMA_VERSION: &str = "desktop-export-report.v1";
pub const DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION: &str = "desktop-package-manifest.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopExportTarget {
    Windows,
}

impl DesktopExportTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Windows => "windows",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopExportRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub profile: String,
    pub target: DesktopExportTarget,
    pub frame_limit: u64,
    pub player_executable: Option<PathBuf>,
    #[serde(skip)]
    player_artifact_build_root: Option<PathBuf>,
    #[serde(skip)]
    explicit_output: Option<ExplicitExportOutput>,
    #[serde(skip)]
    project_relative_output: Option<crate::ProjectRelativePath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitExportOutput {
    root: PathBuf,
}

impl ExplicitExportOutput {
    pub fn from_user_selected(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub(crate) fn authorizes(&self, path: &Path) -> bool {
        path == self.root || path.starts_with(&self.root)
    }
}

impl DesktopExportRequest {
    pub fn windows_dev(project_root: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        let player_executable = default_player_executable_for_project(&project_root);
        Self {
            output_root: project_root.join("Build").join("Windows"),
            project_root,
            profile: "dev".to_string(),
            target: DesktopExportTarget::Windows,
            frame_limit: 3,
            player_executable,
            player_artifact_build_root: None,
            explicit_output: None,
            project_relative_output: None,
        }
    }

    pub fn with_explicit_output(mut self, output: ExplicitExportOutput) -> Self {
        self.output_root = output.root.clone();
        self.explicit_output = Some(output);
        self.project_relative_output = None;
        self
    }

    pub fn with_player_artifact_build_root(mut self, build_root: PathBuf) -> Self {
        self.player_artifact_build_root = Some(build_root);
        self
    }

    pub(crate) fn with_project_relative_output(
        mut self,
        output: crate::ProjectRelativePath,
    ) -> Self {
        self.output_root = self.project_root.join(output.as_path());
        self.project_relative_output = Some(output);
        self.explicit_output = None;
        self
    }

    pub fn package_dir(&self) -> PathBuf {
        self.output_root.join(&self.profile)
    }

    fn has_project_contained_output(&self) -> bool {
        self.output_root == self.project_root.join("Build").join("Windows")
            || self
                .project_relative_output
                .as_ref()
                .is_some_and(|output| self.output_root == self.project_root.join(output.as_path()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DesktopExportStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DesktopExportDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopExportDiagnostic {
    pub severity: DesktopExportDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

impl DesktopExportDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DesktopExportDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path: None,
            suggestion: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DesktopExportDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path: None,
            suggestion: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl From<ProjectRuntimePackageAssemblyDiagnostic> for DesktopExportDiagnostic {
    fn from(diagnostic: ProjectRuntimePackageAssemblyDiagnostic) -> Self {
        Self {
            severity: match diagnostic.severity {
                ProjectRuntimePackageAssemblySeverity::Info => {
                    DesktopExportDiagnosticSeverity::Info
                }
                ProjectRuntimePackageAssemblySeverity::Warning => {
                    DesktopExportDiagnosticSeverity::Warning
                }
                ProjectRuntimePackageAssemblySeverity::Error => {
                    DesktopExportDiagnosticSeverity::Error
                }
            },
            code: format!("Assembly::{:?}::{}", diagnostic.domain, diagnostic.code),
            message: diagnostic.message,
            path: diagnostic.path,
            suggestion: diagnostic.suggestion,
        }
    }
}

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
    pub player_module_descriptor:
        Option<engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopExportReport {
    pub schema_version: String,
    pub status: DesktopExportStatus,
    pub target: String,
    pub profile: String,
    pub project_root: String,
    pub package_dir: String,
    pub runtime_package_dir: String,
    pub package_manifest_path: String,
    pub runtime_package_report_path: String,
    pub player_report_path: String,
    pub player_executable: Option<String>,
    pub player_executable_status: String,
    pub player_artifact_build_report_path: Option<String>,
    pub player_artifact_hash: Option<String>,
    pub player_module_descriptor:
        Option<engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor>,
    pub runtime_package_status: RuntimePackageBuildStatus,
    pub player_exit_code: Option<i32>,
    pub player_exit_reason: String,
    pub diagnostics: Vec<DesktopExportDiagnostic>,
}

pub struct DesktopExportPipeline;

impl DesktopExportPipeline {
    pub fn export(request: DesktopExportRequest) -> DesktopExportReport {
        let package_dir = request.package_dir();
        let data_dir = package_dir.join("data");
        let runtime_package_dir = data_dir.join("runtime_package");
        let reports_dir = package_dir.join("reports");
        let package_manifest_path = package_dir.join("package-manifest.json");
        let desktop_report_path = reports_dir.join("desktop-export-report.json");
        let player_report_path = reports_dir.join("windowed-player-run-report.json");
        let runtime_report_path = runtime_package_dir
            .join("reports")
            .join("build-runtime-package-report.json");
        let mut diagnostics = Vec::new();

        let project_scope = match project_output_scope(&request) {
            Ok(scope) => scope,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                return DesktopExportReport {
                    schema_version: DESKTOP_EXPORT_REPORT_SCHEMA_VERSION.to_string(),
                    status: DesktopExportStatus::Failed,
                    target: request.target.as_str().to_string(),
                    profile: request.profile,
                    project_root: request.project_root.display().to_string(),
                    package_dir: package_dir.display().to_string(),
                    runtime_package_dir: runtime_package_dir.display().to_string(),
                    package_manifest_path: package_manifest_path.display().to_string(),
                    runtime_package_report_path: runtime_report_path.display().to_string(),
                    player_report_path: player_report_path.display().to_string(),
                    player_executable: None,
                    player_executable_status: "not_checked".to_string(),
                    player_artifact_build_report_path: None,
                    player_artifact_hash: None,
                    player_module_descriptor: None,
                    runtime_package_status: RuntimePackageBuildStatus::Failed,
                    player_exit_code: None,
                    player_exit_reason: "not_started".to_string(),
                    diagnostics,
                };
            }
        };

        if let Some(scope) = &project_scope {
            for path in [&runtime_package_dir, &data_dir.join("assets"), &reports_dir] {
                let relative = path
                    .strip_prefix(&request.project_root)
                    .expect("default export paths are project relative");
                if let Err(error) = scope.ensure_directory(relative) {
                    diagnostics.push(
                        DesktopExportDiagnostic::error(
                            error.code,
                            format!("Desktop export containment failed: {error}"),
                        )
                        .with_path(path.display().to_string()),
                    );
                    return DesktopExportReport {
                        schema_version: DESKTOP_EXPORT_REPORT_SCHEMA_VERSION.to_string(),
                        status: DesktopExportStatus::Failed,
                        target: request.target.as_str().to_string(),
                        profile: request.profile,
                        project_root: request.project_root.display().to_string(),
                        package_dir: package_dir.display().to_string(),
                        runtime_package_dir: runtime_package_dir.display().to_string(),
                        package_manifest_path: package_manifest_path.display().to_string(),
                        runtime_package_report_path: runtime_report_path.display().to_string(),
                        player_report_path: player_report_path.display().to_string(),
                        player_executable: None,
                        player_executable_status: "not_checked".to_string(),
                        player_artifact_build_report_path: None,
                        player_artifact_hash: None,
                        player_module_descriptor: None,
                        runtime_package_status: RuntimePackageBuildStatus::Failed,
                        player_exit_code: None,
                        player_exit_reason: "not_started".to_string(),
                        diagnostics,
                    };
                }
            }
        } else {
            let _ = fs::create_dir_all(&runtime_package_dir);
            let _ = fs::create_dir_all(data_dir.join("assets"));
            let _ = fs::create_dir_all(&reports_dir);
        }

        let assembly_request = ProjectRuntimePackageAssemblyRequest::new(&request.project_root)
            .with_build_profile_path(request.project_root.join("BuildProfiles").join(format!(
                "{}.{}.json",
                request.target.as_str(),
                request.profile
            )));
        let assembly_result = ProjectRuntimePackageAssembler::assemble(assembly_request);
        diagnostics.extend(
            assembly_result
                .report
                .diagnostics
                .iter()
                .cloned()
                .map(DesktopExportDiagnostic::from),
        );
        if assembly_result.status == ProjectRuntimePackageAssemblyStatus::Failed {
            return write_final_report(
                &request,
                DesktopExportReport {
                    schema_version: DESKTOP_EXPORT_REPORT_SCHEMA_VERSION.to_string(),
                    status: DesktopExportStatus::Failed,
                    target: request.target.as_str().to_string(),
                    profile: request.profile.clone(),
                    project_root: request.project_root.display().to_string(),
                    package_dir: package_dir.display().to_string(),
                    runtime_package_dir: runtime_package_dir.display().to_string(),
                    package_manifest_path: package_manifest_path.display().to_string(),
                    runtime_package_report_path: runtime_report_path.display().to_string(),
                    player_report_path: player_report_path.display().to_string(),
                    player_executable: None,
                    player_executable_status: "not_checked".to_string(),
                    player_artifact_build_report_path: None,
                    player_artifact_hash: None,
                    player_module_descriptor: None,
                    runtime_package_status: RuntimePackageBuildStatus::Failed,
                    player_exit_code: None,
                    player_exit_reason: "not_started".to_string(),
                    diagnostics,
                },
                &desktop_report_path,
            );
        }
        let runtime_input = assembly_result
            .build_input
            .expect("successful assembly should produce RuntimePackageBuildInput");
        let active_scene_id = assembly_result
            .active_scene_id
            .expect("successful assembly should produce an active scene id");
        let frame_limit = assembly_result
            .build_profile
            .as_ref()
            .map(|profile| profile.frame_limit)
            .unwrap_or(request.frame_limit);
        let package_request =
            RuntimePackageBuildRequest::dev_desktop(&runtime_package_dir, active_scene_id);
        let runtime_report = RuntimePackageBuilder::build(&package_request, &runtime_input);

        let staged_player = stage_player_executable(
            &request,
            &package_dir,
            &runtime_input.project.runtime_module,
            &mut diagnostics,
        );
        let player_copy = staged_player
            .as_ref()
            .map(|staged| staged.destination.clone());
        let manifest = DesktopPackageManifest {
            schema_version: DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION.to_string(),
            target: request.target.as_str().to_string(),
            profile: request.profile.clone(),
            package_dir: package_dir.display().to_string(),
            runtime_package_dir: runtime_package_dir.display().to_string(),
            reports_dir: reports_dir.display().to_string(),
            player_executable: player_copy.as_ref().map(|path| path.display().to_string()),
            player_executable_status: player_executable_status(&player_copy),
            player_artifact_build_report_path: staged_player.as_ref().and_then(|staged| {
                staged
                    .artifact
                    .build_report_path
                    .as_ref()
                    .map(|path| path.display().to_string())
            }),
            player_artifact_hash: staged_player
                .as_ref()
                .map(|staged| staged.artifact.source_executable_hash.clone()),
            player_module_descriptor: staged_player
                .as_ref()
                .map(|staged| staged.artifact.module_descriptor.clone()),
        };
        let _ = write_export_json(&request, &package_manifest_path, &manifest);

        let process_report_path =
            reports_dir.join("exported-player-process-verification-report.json");
        let player_report = if matches!(runtime_report.status, RuntimePackageBuildStatus::Success)
            && player_copy.is_some()
        {
            Some(verify_exported_player_process(
                ExportedPlayerProcessVerificationRequest {
                    exported_package_dir: package_dir.clone(),
                    mode: "headless".to_string(),
                    frame_limit: frame_limit.max(1),
                    report_path: Some(process_report_path),
                    timeout_ms: 30_000,
                    screenshot: false,
                    screenshot_path: None,
                },
            ))
        } else {
            None
        };

        let player_exit_code = player_report
            .as_ref()
            .and_then(|report| report.child_player_exit_code);
        let player_exit_reason = player_report
            .as_ref()
            .map(|report| report.process_exit_reason.clone())
            .unwrap_or_else(|| "not_started".to_string());
        if let Some(report) = &player_report {
            for diagnostic in &report.diagnostics {
                let mapped = if diagnostic.severity == "error" {
                    DesktopExportDiagnostic::error(
                        diagnostic.code.clone(),
                        diagnostic.message.clone(),
                    )
                } else {
                    DesktopExportDiagnostic::warning(
                        diagnostic.code.clone(),
                        diagnostic.message.clone(),
                    )
                };
                diagnostics.push(if let Some(path) = &diagnostic.path {
                    mapped.with_path(path.clone())
                } else {
                    mapped
                });
            }
        }
        if player_report.as_ref().is_some_and(|report| {
            report.status != ExportedPlayerProcessVerificationStatus::Passed
                || report.process_exit_code != Some(0)
                || report.child_player_exit_code != Some(0)
        }) {
            diagnostics.push(
                DesktopExportDiagnostic::error(
                    "PlayerGateFailed",
                    "Staged project Player process failed for the exported runtime package.",
                )
                .with_path(player_report_path.display().to_string())
                .with_suggestion(
                    "Read exported-player-process-verification-report.json and windowed-player-run-report.json.",
                ),
            );
        }

        let status = if matches!(runtime_report.status, RuntimePackageBuildStatus::Success)
            && player_exit_code == Some(0)
            && !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == DesktopExportDiagnosticSeverity::Error)
        {
            DesktopExportStatus::Success
        } else {
            DesktopExportStatus::Failed
        };

        write_final_report(
            &request,
            DesktopExportReport {
                schema_version: DESKTOP_EXPORT_REPORT_SCHEMA_VERSION.to_string(),
                status,
                target: request.target.as_str().to_string(),
                profile: request.profile.clone(),
                project_root: request.project_root.display().to_string(),
                package_dir: package_dir.display().to_string(),
                runtime_package_dir: runtime_package_dir.display().to_string(),
                package_manifest_path: package_manifest_path.display().to_string(),
                runtime_package_report_path: runtime_report_path.display().to_string(),
                player_report_path: player_report_path.display().to_string(),
                player_executable: player_copy.as_ref().map(|path| path.display().to_string()),
                player_executable_status: manifest.player_executable_status,
                player_artifact_build_report_path: manifest.player_artifact_build_report_path,
                player_artifact_hash: manifest.player_artifact_hash,
                player_module_descriptor: manifest.player_module_descriptor,
                runtime_package_status: runtime_report.status,
                player_exit_code,
                player_exit_reason,
                diagnostics,
            },
            &desktop_report_path,
        )
    }
}

struct StagedProjectPlayer {
    destination: PathBuf,
    artifact: crate::ProjectPlayerArtifact,
}

fn stage_player_executable(
    request: &DesktopExportRequest,
    package_dir: &Path,
    expected_module: &RuntimeProjectModuleRef,
    diagnostics: &mut Vec<DesktopExportDiagnostic>,
) -> Option<StagedProjectPlayer> {
    let project_manifest = fs::read_to_string(request.project_root.join("project.aife.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<crate::ProjectManifest>(&text).ok());
    let artifact = if project_manifest.as_ref().is_some_and(|manifest| {
        manifest.runtime_module.source_kind == Some(crate::ProjectRuntimeSourceKind::ProjectRust)
    }) {
        let mut build_request = crate::ProjectRuntimePlayerArtifactBuildRequest::new(
            &request.project_root,
            crate::default_engine_sdk_root(),
            expected_module.clone(),
        );
        if let Some(build_root) = &request.player_artifact_build_root {
            build_request = build_request.with_build_root(build_root);
        }
        let build = crate::ProjectPlayerArtifact::build_project_rust(build_request);
        if build.status != crate::ProjectRuntimePlayerArtifactBuildStatus::Success {
            for diagnostic in build.diagnostics {
                diagnostics.push(
                    DesktopExportDiagnostic::error(diagnostic.code, diagnostic.message)
                        .with_suggestion(diagnostic.next_action),
                );
            }
            return None;
        }
        let Some(artifact) = build.artifact else {
            diagnostics.push(
                DesktopExportDiagnostic::error(
                    "PlayerArtifactMissingAfterBuild",
                    "ProjectRust Player artifact build succeeded without an artifact.",
                )
                .with_suggestion("Inspect the ProjectRuntime Player artifact build report."),
            );
            return None;
        };
        artifact
    } else {
        let Some(source) = &request.player_executable else {
            diagnostics.push(
                DesktopExportDiagnostic::warning(
                    "PlayerExecutableNotConfigured",
                    "No player executable path was configured; package keeps reports and data only.",
                )
                .with_suggestion(
                    "Build runtime_cli or WindowedPlayer before producing a distributable package.",
                ),
            );
            return None;
        };
        let cargo_package = source
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if let Err(error) = crate::ProjectPlayerArtifact::ensure_built(source, cargo_package) {
            diagnostics.push(
                DesktopExportDiagnostic::error(error.code, error.message)
                    .with_path(source.display().to_string())
                    .with_suggestion(error.next_action),
            );
            return None;
        }
        match crate::ProjectPlayerArtifact::inspect(source, expected_module) {
            Ok(artifact) => artifact,
            Err(error) => {
                diagnostics.push(
                    DesktopExportDiagnostic::error(error.code, error.message)
                        .with_path(source.display().to_string())
                        .with_suggestion(error.next_action),
                );
                return None;
            }
        }
    };
    let source = &artifact.executable_path;
    let destination = package_dir.join("Game.exe");
    let copy_result = if request.has_project_contained_output() {
        fs::read(source).and_then(|bytes| {
            let relative = destination
                .strip_prefix(&request.project_root)
                .map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "player destination is outside project root",
                    )
                })?;
            let scope = crate::ProjectWriteScope::open(&request.project_root)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            scope
                .write_atomic(relative, &bytes)
                .map(|_| bytes.len() as u64)
                .map_err(|error| std::io::Error::other(error.to_string()))
        })
    } else {
        fs::copy(source, &destination)
    };
    if let Err(error) = copy_result {
        diagnostics.push(
            DesktopExportDiagnostic::warning(
                "PlayerExecutableCopyFailed",
                format!("Failed to copy player executable: {error}"),
            )
            .with_path(destination.display().to_string()),
        );
        return None;
    }
    Some(StagedProjectPlayer {
        destination,
        artifact,
    })
}

fn player_executable_status(path: &Option<PathBuf>) -> String {
    if path.is_some() {
        "copied".to_string()
    } else {
        "not_available".to_string()
    }
}

pub(crate) fn default_player_executable_for_project(project_root: &Path) -> Option<PathBuf> {
    let manifest = fs::read_to_string(project_root.join("project.aife.json")).ok()?;
    let project = serde_json::from_str::<crate::ProjectManifest>(&manifest).ok()?;
    Some(crate::ProjectPlayerArtifact::debug_executable_path(
        &project.runtime_module.player_binary,
    ))
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}

fn write_final_report(
    request: &DesktopExportRequest,
    report: DesktopExportReport,
    report_path: &Path,
) -> DesktopExportReport {
    let _ = write_export_json(request, report_path, &report);
    report
}

fn project_output_scope(
    request: &DesktopExportRequest,
) -> Result<Option<crate::ProjectWriteScope>, DesktopExportDiagnostic> {
    if request.has_project_contained_output() {
        return crate::ProjectWriteScope::open(&request.project_root)
            .map(Some)
            .map_err(|error| {
                DesktopExportDiagnostic::error(error.code, error.to_string())
                    .with_path(request.output_root.display().to_string())
            });
    }
    if request
        .explicit_output
        .as_ref()
        .is_some_and(|output| output.authorizes(&request.output_root))
    {
        Ok(None)
    } else {
        Err(DesktopExportDiagnostic::error(
            "project_write.explicit_export_required",
            "External desktop export requires ExplicitExportOutput authorization.",
        )
        .with_path(request.output_root.display().to_string()))
    }
}

fn write_export_json<T: Serialize>(
    request: &DesktopExportRequest,
    path: &Path,
    value: &T,
) -> std::io::Result<()> {
    if request.has_project_contained_output() {
        let relative = path.strip_prefix(&request.project_root).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "desktop export path is outside project root",
            )
        })?;
        let scope = crate::ProjectWriteScope::open(&request.project_root)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        return scope
            .write_atomic(relative, text.as_bytes())
            .map(|_| ())
            .map_err(|error| std::io::Error::other(error.to_string()));
    }
    write_json(path, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InputMappingAuthoringService, InputMappingEditCommand, ProjectLauncherState};
    use engine_runtime::runtime_package::load_runtime_package;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn desktop_export_builds_runtime_package_from_saved_project() {
        let project_root = create_project_with_scene("desktop-export-runtime-package");
        let request = DesktopExportRequest::windows_dev(&project_root);

        let report = DesktopExportPipeline::export(request);

        assert_eq!(
            report.runtime_package_status,
            RuntimePackageBuildStatus::Success
        );
        assert!(Path::new(&report.runtime_package_dir)
            .join("manifest.json")
            .exists());
        assert!(Path::new(&report.runtime_package_dir)
            .join("scenes")
            .join("scene-main.json")
            .exists());
    }

    #[test]
    fn desktop_export_stages_windows_package_layout() {
        let project_root = create_project_with_scene("desktop-export-stage");
        let request = DesktopExportRequest::windows_dev(&project_root);

        let report = DesktopExportPipeline::export(request);
        let package_dir = Path::new(&report.package_dir);

        assert!(package_dir.join("package-manifest.json").exists());
        assert!(package_dir.join("data").join("runtime_package").exists());
        assert!(package_dir.join("data").join("assets").exists());
        assert!(package_dir
            .join("reports")
            .join("desktop-export-report.json")
            .exists());
    }

    #[test]
    fn desktop_export_runs_player_gate_from_staged_package() {
        let project_root = create_project_with_scene("desktop-export-player-gate");
        let mut request = DesktopExportRequest::windows_dev(&project_root);
        request.frame_limit = 2;

        let report = DesktopExportPipeline::export(request);

        assert_eq!(report.status, DesktopExportStatus::Success);
        assert_eq!(report.player_exit_code, Some(0));
        assert_eq!(report.player_exit_reason, "completed");
        assert!(Path::new(&report.player_report_path).exists());
    }

    #[test]
    fn desktop_export_input_mapping_uses_project_mapping_in_runtime_package() {
        let project_root = create_project_with_scene("desktop-export-input-mapping");
        let mut mapping = InputMappingAuthoringService::create_default();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::AddAction {
                action_id: "action.test".to_string(),
                value_type: editor_ui_model::InputActionValueKind::Button,
            },
        )
        .unwrap();
        InputMappingAuthoringService::apply(
            &mut mapping,
            InputMappingEditCommand::AddBinding {
                context_id: "gameplay".to_string(),
                action_id: "action.test".to_string(),
                device_path: "keyboard/T".to_string(),
            },
        )
        .unwrap();
        InputMappingAuthoringService::save(&project_root, "Input/input.default.json", &mapping)
            .unwrap();

        let report =
            DesktopExportPipeline::export(DesktopExportRequest::windows_dev(&project_root));
        let package = load_runtime_package(Path::new(&report.runtime_package_dir))
            .value
            .expect("runtime package should load");

        let default_mapping = package
            .default_input_mapping
            .expect("default input mapping should be loaded from package");
        assert_eq!(default_mapping.asset_id, "input.default");
        assert!(default_mapping
            .actions
            .iter()
            .any(|action| action.id == "action.test"));
        assert!(default_mapping
            .bindings
            .iter()
            .any(|binding| binding.device_path == "keyboard/T"));
    }

    #[test]
    fn desktop_export_complex_shooter_package_contains_assembled_domains() {
        let source_root = complex_shooter_project_fixture_root();
        let source_report_path = source_root
            .join("Build")
            .join("Windows")
            .join("dev")
            .join("reports")
            .join("desktop-export-report.json");
        let source_report_before = fs::read(&source_report_path).unwrap();
        let project_root = copy_complex_shooter_project_fixture("desktop-export-complex-shooter");
        let report =
            DesktopExportPipeline::export(DesktopExportRequest::windows_dev(&project_root));

        assert_eq!(report.status, DesktopExportStatus::Success);
        assert_eq!(fs::read(source_report_path).unwrap(), source_report_before);
        let package = load_runtime_package(Path::new(&report.runtime_package_dir))
            .value
            .expect("runtime package should load");
        let player_report_text = fs::read_to_string(&report.player_report_path).unwrap();
        let player_report: serde_json::Value = serde_json::from_str(&player_report_text).unwrap();
        assert_eq!(player_report["counters"]["framesRequested"], 6);
        assert!(Path::new(&report.runtime_package_dir)
            .join("prefabs")
            .join("prefab-player-bullet.json")
            .exists());
        assert!(package.rules.rules.len() >= 3);
        assert!(package.aui_manifest.documents.len() >= 1);
        assert!(Path::new(&report.runtime_package_dir)
            .join("cooked")
            .join("textures")
            .join("tex-player-ship.texture.json")
            .exists());
        assert!(Path::new(&report.runtime_package_dir)
            .join("cooked")
            .join("textures")
            .join("tex-player-ship.rgba8")
            .exists());
        assert!(Path::new(&report.runtime_package_dir)
            .join("cooked")
            .join("font-main.asset")
            .exists());
    }

    #[test]
    fn complex_shooter_export_fixture_has_isolated_build_ownership() {
        let source_root = complex_shooter_project_fixture_root();
        let source_manifest = source_root
            .join("Build")
            .join("Windows")
            .join("dev")
            .join("package-manifest.json");
        let source_manifest_before = fs::read(&source_manifest).unwrap();
        let project_root = copy_complex_shooter_project_fixture("desktop-export-ownership");
        let copied_manifest = project_root
            .join("Build")
            .join("Windows")
            .join("dev")
            .join("package-manifest.json");

        fs::write(&copied_manifest, b"isolated fixture mutation").unwrap();

        assert_ne!(
            fs::canonicalize(&project_root).unwrap(),
            fs::canonicalize(&source_root).unwrap()
        );
        assert_eq!(fs::read(source_manifest).unwrap(), source_manifest_before);
        assert_eq!(
            DesktopExportRequest::windows_dev(&project_root).package_dir(),
            project_root.join("Build").join("Windows").join("dev")
        );
    }

    #[test]
    fn desktop_export_project_relative_output_does_not_mint_external_authority() {
        let project_root = unique_temp_dir("desktop-export-project-relative-authority");
        let relative = crate::ProjectRelativePath::parse(
            "Library/AiCapability/Deliveries/operation-1/Windows",
        )
        .unwrap();
        let request = DesktopExportRequest::windows_dev(&project_root)
            .with_project_relative_output(relative.clone());

        assert_eq!(request.output_root, project_root.join(relative.as_path()));
        assert_eq!(request.project_relative_output, Some(relative));
        assert!(request.explicit_output.is_none());
        assert!(request.has_project_contained_output());
    }

    #[cfg(windows)]
    #[test]
    fn desktop_export_project_relative_output_rejects_delivery_junction() {
        let project_root = create_project_with_scene("desktop-export-project-junction");
        let outside = unique_temp_dir("desktop-export-project-junction-outside");
        fs::create_dir_all(&outside).unwrap();
        let delivery_parent = project_root.join("Library").join("AiCapability");
        fs::create_dir_all(&delivery_parent).unwrap();
        let delivery_root = delivery_parent.join("Deliveries");
        create_directory_junction(&outside, &delivery_root);
        let relative = crate::ProjectRelativePath::parse(
            "Library/AiCapability/Deliveries/operation-1/Windows",
        )
        .unwrap();

        let report = DesktopExportPipeline::export(
            DesktopExportRequest::windows_dev(&project_root).with_project_relative_output(relative),
        );
        let escaped_output_exists = outside.join("operation-1").exists();

        fs::remove_dir(&delivery_root).unwrap();
        let _ = fs::remove_dir_all(&project_root);
        let _ = fs::remove_dir_all(&outside);

        assert_eq!(report.status, DesktopExportStatus::Failed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.starts_with("project_write.") }));
        assert!(!escaped_output_exists);
    }

    fn create_project_with_scene(name: &str) -> PathBuf {
        let root = unique_temp_dir(name);
        let mut launcher = ProjectLauncherState::new("0.0.1");
        launcher.create_project(&root, "ExportGame").unwrap();
        fs::write(root.join("Scenes").join("Main.scene.json"), scene_json()).unwrap();
        root
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{stamp}"))
    }

    fn complex_shooter_project_fixture_root() -> PathBuf {
        workspace_root()
            .join("samples")
            .join("complex_shooter_project")
    }

    fn copy_complex_shooter_project_fixture(name: &str) -> PathBuf {
        let source = complex_shooter_project_fixture_root();
        let destination = unique_temp_dir(name);
        copy_directory_tree(&source, &destination);
        destination
    }

    fn copy_directory_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_directory_tree(&source_path, &destination_path);
            } else {
                fs::copy(source_path, destination_path).unwrap();
            }
        }
    }

    #[cfg(windows)]
    fn create_directory_junction(target: &Path, link: &Path) {
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .expect("launch mklink /J");
        assert!(
            output.status.success(),
            "mklink /J failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
    }

    fn scene_json() -> &'static str {
        r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#111111",
  "entities": [
    {
      "schemaVersion": "editor-scene-entity.v1",
      "id": "entity-player",
      "name": "Player",
      "kind": "actor",
      "enabled": true,
      "siblingOrder": 0,
      "transform": {
        "localPosition": { "x": 0, "y": 0, "z": 0 },
        "localRotation": { "x": 0, "y": 0, "z": 0 },
        "localScale": { "x": 1, "y": 1, "z": 1 }
      },
      "components": [
        {
          "componentType": "project.marker",
          "data": { "value": "spawn" }
        }
      ]
    }
  ]
}"##
    }
}
