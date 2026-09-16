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
    runtime_package_digest, validate_desktop_dev_package, verify_exported_player_process,
    ExportedPlayerProcessVerificationRequest, ExportedPlayerProcessVerificationStatus,
};
pub use runtime_cli::{DesktopPackageManifest, DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DESKTOP_EXPORT_REPORT_SCHEMA_VERSION: &str = "desktop-export-report.v1";

const fn default_player_timeout_ms() -> u64 {
    30_000
}

const fn default_player_verification() -> bool {
    true
}

#[cfg(test)]
mod verification_policy_tests {
    use super::*;

    #[test]
    fn default_verification_survives_deserialization_and_can_be_disabled_internally() {
        let request = DesktopExportRequest::windows_dev("nonexistent-test-project");
        assert!(request.verify_player);
        let decoded: DesktopExportRequest =
            serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
        assert!(decoded.verify_player);
        assert!(!request.with_player_verification(false).verify_player);
    }

    #[test]
    fn required_manifest_and_report_write_failures_cannot_report_success() {
        let root = std::env::temp_dir().join(format!(
            "aife-export-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let request = DesktopExportRequest::windows_dev(&root).with_explicit_output(
            ExplicitExportOutput::from_user_selected(root.join("export")),
        );
        // A directory at a required JSON filename makes the actual writer fail.
        let blocked = request.package_dir().join("package-manifest.json");
        fs::create_dir_all(&blocked).unwrap();
        let manifest = DesktopPackageManifest {
            schema_version: DESKTOP_PACKAGE_MANIFEST_SCHEMA_VERSION.into(),
            target: "windows".into(),
            profile: "dev".into(),
            package_dir: String::new(),
            runtime_package_dir: String::new(),
            reports_dir: String::new(),
            player_executable: None,
            player_executable_status: "copied".into(),
            player_artifact_build_report_path: None,
            player_artifact_hash: None,
            player_module_descriptor: None,
            engine_runtime_hash: None,
            project_runtime_module_hash: None,
            runtime_package_digest: None,
        };
        let error = write_package_manifest(&request, &blocked, &manifest).unwrap_err();
        assert_eq!(error.code, "DesktopPackageManifestWriteFailed");
        assert_eq!(error.path.as_deref(), Some(blocked.to_str().unwrap()));
        let report = DesktopExportReport {
            schema_version: DESKTOP_EXPORT_REPORT_SCHEMA_VERSION.into(),
            status: DesktopExportStatus::Success,
            target: "windows".into(),
            profile: "dev".into(),
            project_root: root.display().to_string(),
            package_dir: String::new(),
            runtime_package_dir: String::new(),
            package_manifest_path: String::new(),
            runtime_package_report_path: String::new(),
            player_report_path: String::new(),
            player_executable: None,
            player_executable_status: "copied".into(),
            player_artifact_build_report_path: None,
            player_artifact_hash: None,
            player_module_descriptor: None,
            runtime_package_status: RuntimePackageBuildStatus::Success,
            player_exit_code: None,
            player_exit_reason: "not_started".into(),
            diagnostics: Vec::new(),
        };
        let failed = write_final_report(&request, report, &blocked);
        assert_eq!(failed.status, DesktopExportStatus::Failed);
        assert_eq!(failed.diagnostics[0].code, "DesktopExportReportWriteFailed");
        assert_eq!(failed.player_exit_reason, "not_started");
        fs::remove_dir_all(&root).unwrap();
    }
}

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
    #[serde(skip, default = "default_player_timeout_ms")]
    player_timeout_ms: u64,
    #[serde(skip, default = "default_player_verification")]
    verify_player: bool,
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

    #[doc(hidden)]
    pub fn authorizes(&self, path: &Path) -> bool {
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
            player_timeout_ms: default_player_timeout_ms(),
            verify_player: true,
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

    pub fn with_player_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.player_timeout_ms = timeout_ms.max(1);
        self
    }

    pub fn with_player_verification(mut self, enabled: bool) -> Self {
        self.verify_player = enabled;
        self
    }

    #[doc(hidden)]
    pub fn with_project_relative_output(mut self, output: crate::ProjectRelativePath) -> Self {
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
        Self::export_inner(request, None, None, None)
    }

    pub fn export_prepared(
        request: DesktopExportRequest,
        prepared: &crate::PreparedRuntimePackage,
    ) -> DesktopExportReport {
        Self::export_inner(
            request,
            Some(prepared.runtime_package_build_input()),
            prepared.generated_runtime_glue(),
            Some(prepared.source()),
        )
    }

    fn export_inner(
        request: DesktopExportRequest,
        prepared_runtime_input: Option<
            &engine_runtime::runtime_package_builder::RuntimePackageBuildInput,
        >,
        prepared_runtime_glue: Option<&crate::PreparedRuntimeGlue>,
        prepared_source: Option<&crate::CompilerSourceView>,
    ) -> DesktopExportReport {
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

        let (runtime_input, active_scene_id, frame_limit) = if let Some(runtime_input) =
            prepared_runtime_input
        {
            let Some(active_scene_id) = runtime_input.scenes.first().map(|scene| scene.id.clone())
            else {
                diagnostics.push(DesktopExportDiagnostic::error(
                    "PreparedRuntimePackageSceneMissing",
                    "Prepared RuntimePackage input has no active Scene for desktop export.",
                ));
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
            };
            (runtime_input.clone(), active_scene_id, request.frame_limit)
        } else {
            let assembly_request =
                ProjectRuntimePackageAssemblyRequest::new(&request.project_root)
                    .with_build_profile_path(request.project_root.join("BuildProfiles").join(
                        format!("{}.{}.json", request.target.as_str(), request.profile),
                    ));
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
            (runtime_input, active_scene_id, frame_limit)
        };
        let package_request =
            RuntimePackageBuildRequest::dev_desktop(&runtime_package_dir, active_scene_id);
        let runtime_report = RuntimePackageBuilder::build(&package_request, &runtime_input);

        let staged_player = stage_player_executable(
            &request,
            &package_dir,
            &runtime_input.project.runtime_module,
            prepared_runtime_glue,
            prepared_source,
            &mut diagnostics,
        );
        let player_copy = staged_player
            .as_ref()
            .map(|staged| staged.destination.clone());
        let payload_digest = if runtime_report.status == RuntimePackageBuildStatus::Success {
            match runtime_package_digest(&runtime_package_dir) {
                Ok(digest) => Some(digest),
                Err(message) => {
                    diagnostics.push(
                        DesktopExportDiagnostic::error("RuntimePackageDigestFailed", message)
                            .with_path(runtime_package_dir.display().to_string()),
                    );
                    None
                }
            }
        } else {
            None
        };
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
            engine_runtime_hash: staged_player
                .as_ref()
                .map(|staged| staged.engine_runtime_hash.clone()),
            project_runtime_module_hash: staged_player
                .as_ref()
                .map(|staged| staged.project_runtime_module_hash.clone()),
            runtime_package_digest: payload_digest,
        };
        if let Err(diagnostic) = write_package_manifest(&request, &package_manifest_path, &manifest)
        {
            diagnostics.push(diagnostic);
        } else if runtime_report.status == RuntimePackageBuildStatus::Success
            && staged_player.is_some()
        {
            if let Err(diagnostic) = validate_desktop_dev_package(&package_dir) {
                diagnostics.push(DesktopExportDiagnostic {
                    severity: DesktopExportDiagnosticSeverity::Error,
                    code: diagnostic.code,
                    message: diagnostic.message,
                    path: diagnostic.path,
                    suggestion: Some(
                        "Export the project again to produce one consistent delivery.".to_string(),
                    ),
                });
            }
        }

        let process_report_path =
            reports_dir.join("exported-player-process-verification-report.json");
        let player_report = if request.verify_player
            && matches!(runtime_report.status, RuntimePackageBuildStatus::Success)
            && player_copy.is_some()
            && !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == DesktopExportDiagnosticSeverity::Error)
        {
            Some(verify_exported_player_process(
                ExportedPlayerProcessVerificationRequest {
                    exported_package_dir: package_dir.clone(),
                    mode: "headless".to_string(),
                    frame_limit: frame_limit.max(1),
                    report_path: Some(process_report_path),
                    timeout_ms: request.player_timeout_ms,
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

        let player_gate_passed = if request.verify_player {
            player_exit_code == Some(0)
        } else {
            true
        };
        let status = if matches!(runtime_report.status, RuntimePackageBuildStatus::Success)
            && player_copy.is_some()
            && player_gate_passed
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
    engine_runtime_hash: String,
    project_runtime_module_hash: String,
}

fn stage_player_executable(
    request: &DesktopExportRequest,
    package_dir: &Path,
    expected_module: &RuntimeProjectModuleRef,
    prepared_runtime_glue: Option<&crate::PreparedRuntimeGlue>,
    prepared_source: Option<&crate::CompilerSourceView>,
    diagnostics: &mut Vec<DesktopExportDiagnostic>,
) -> Option<StagedProjectPlayer> {
    let project_manifest = match prepared_source {
        Some(source) => source
            .bytes("project.aife.json")
            .and_then(|bytes| serde_json::from_slice::<crate::ProjectManifest>(bytes).ok()),
        None => fs::read(request.project_root.join("project.aife.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()),
    };
    let artifact = if project_manifest.as_ref().is_some_and(|manifest| {
        manifest.runtime_module.resolved_source_kind()
            == crate::ProjectRuntimeSourceKind::ProjectRust
    }) {
        let mut build_request = crate::ProjectRuntimePlayerArtifactBuildRequest::new(
            &request.project_root,
            crate::default_engine_sdk_root(),
            expected_module.clone(),
        );
        build_request = build_request.with_compile_tests(request.verify_player);
        if let Some(build_root) = &request.player_artifact_build_root {
            build_request = build_request.with_build_root(build_root);
        }
        if let Some(glue) = prepared_runtime_glue {
            build_request = build_request.with_prepared_runtime_glue(glue.clone());
        }
        build_request.prepared_source = prepared_source.cloned();
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
    let copy_result = copy_runtime_file(request, source, &destination);
    if let Err(error) = copy_result {
        diagnostics.push(
            DesktopExportDiagnostic::error(
                "PlayerExecutableCopyFailed",
                format!("Failed to copy player executable: {error}"),
            )
            .with_path(destination.display().to_string()),
        );
        return None;
    }
    let engine_runtime_source = source
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|root| root.join("engine_runtime.dll"));
    let engine_runtime_destination = package_dir.join("engine_runtime.dll");
    let engine_runtime_result = engine_runtime_source
        .filter(|path| path.is_file())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "validated Project Player artifact has no engine_runtime.dll",
            )
        })
        .and_then(|source| copy_runtime_file(request, &source, &engine_runtime_destination));
    let engine_runtime_hash = match engine_runtime_result {
        Ok(hash) => hash,
        Err(error) => {
            diagnostics.push(
                DesktopExportDiagnostic::error(
                    "EngineRuntimeDllCopyFailed",
                    format!("Failed to copy engine runtime DLL: {error}"),
                )
                .with_path(engine_runtime_destination.display().to_string()),
            );
            return None;
        }
    };
    let host_target = format!(
        "{}-pc-windows-{}",
        std::env::consts::ARCH,
        if cfg!(target_env = "msvc") {
            "msvc"
        } else {
            "gnu"
        }
    );
    let project_runtime_source = source
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(|root| {
            let file_name = if cfg!(windows) {
                "aife_generated_runtime_glue.dll"
            } else {
                "libaife_generated_runtime_glue.so"
            };
            let staged_module = root.join("project_runtime_module.dll");
            if staged_module.is_file() {
                return Some(staged_module);
            }
            let root = root.join("RuntimeGlue").join("target");
            [
                root.join(&host_target).join("debug").join(file_name),
                root.join(&host_target)
                    .join("debug")
                    .join("deps")
                    .join(file_name),
                root.join("debug").join(file_name),
                root.join("debug").join("deps").join(file_name),
            ]
            .into_iter()
            .find(|path| path.is_file())
        });
    let project_runtime_destination = package_dir.join(
        runtime_cli::project_runtime_module_relative_path(&expected_module.module_id),
    );
    let project_runtime_result = project_runtime_source
        .filter(|path| path.is_file())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "validated Project Player artifact has no Generated RuntimeModule DLL",
            )
        })
        .and_then(|source| copy_runtime_file(request, &source, &project_runtime_destination));
    let project_runtime_module_hash = match project_runtime_result {
        Ok(hash) => hash,
        Err(error) => {
            diagnostics.push(
                DesktopExportDiagnostic::error(
                    "ProjectRuntimeModuleDllCopyFailed",
                    format!("Failed to copy Generated RuntimeModule DLL: {error}"),
                )
                .with_path(project_runtime_destination.display().to_string())
                .with_suggestion(
                    "Rebuild the Project Player artifact so RuntimeGlue produces a validated module DLL.",
                ),
            );
            return None;
        }
    };
    Some(StagedProjectPlayer {
        destination,
        artifact,
        engine_runtime_hash,
        project_runtime_module_hash,
    })
}

// Hash the exact source bytes passed to the writer. The shared package validator
// checks the resulting destination files before this export can succeed.
fn copy_runtime_file(
    request: &DesktopExportRequest,
    source: &Path,
    destination: &Path,
) -> std::io::Result<String> {
    let bytes = fs::read(source)?;
    let digest = engine_runtime::canonical_digest::sha256_prefixed(&bytes);
    if request.has_project_contained_output() {
        let relative = destination
            .strip_prefix(&request.project_root)
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "runtime destination is outside project root",
                )
            })?;
        let scope = crate::ProjectWriteScope::open(&request.project_root)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        scope
            .write_atomic(relative, &bytes)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(destination, bytes)?;
    }
    Ok(digest)
}

fn player_executable_status(path: &Option<PathBuf>) -> String {
    if path.is_some() {
        "copied".to_string()
    } else {
        "not_available".to_string()
    }
}

#[doc(hidden)]
pub fn default_player_executable_for_project(project_root: &Path) -> Option<PathBuf> {
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
    mut report: DesktopExportReport,
    report_path: &Path,
) -> DesktopExportReport {
    if let Err(error) = write_export_json(request, report_path, &report) {
        report.status = DesktopExportStatus::Failed;
        report.diagnostics.push(
            DesktopExportDiagnostic::error(
                "DesktopExportReportWriteFailed",
                format!("Failed to write required desktop export report: {error}"),
            )
            .with_path(report_path.display().to_string()),
        );
    }
    report
}

fn write_package_manifest(
    request: &DesktopExportRequest,
    path: &Path,
    manifest: &DesktopPackageManifest,
) -> Result<(), DesktopExportDiagnostic> {
    write_export_json(request, path, manifest).map_err(|error| {
        DesktopExportDiagnostic::error(
            "DesktopPackageManifestWriteFailed",
            format!("Failed to write required desktop package manifest: {error}"),
        )
        .with_path(path.display().to_string())
    })
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
