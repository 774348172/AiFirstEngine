use crate::{
    resolve_release_icon_asset, stamp_windows_executable_resources,
    verify_windows_executable_resource_contract, BuildProfile, BuildProfileApplication,
    BuildProfileRelease, BuildProfileValidationIssue, ProjectManifest,
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimePackageAssemblySeverity, ProjectRuntimePackageAssemblyStatus,
    WindowsExecutableResourceExpectation, WindowsExecutableResourceReadback,
};
use engine_runtime::atomic_directory_publish::atomic_directory_publish;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::release_package_manifest::{
    release_payload_hash, validate_release_package_manifest, ReleasePackageApplication,
    ReleasePackageFile, ReleasePackageFileRole, ReleasePackageLaunch, ReleasePackageManifest,
    ReleasePackageTarget, RELEASE_PACKAGE_MANIFEST_FILE_NAME,
    RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION,
};
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use engine_runtime::runtime_package_path::safe_join_runtime_package;
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationReport,
    ExportedPlayerProcessVerificationRequest, ExportedPlayerProcessVerificationStatus,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const RELEASE_PACKAGE_PLAN_SCHEMA_VERSION: &str = "release-package-plan.v1";
pub const RELEASE_PACKAGE_REPORT_SCHEMA_VERSION: &str = "release-package-report.v1";
pub const RELEASE_PACKAGE_REPORT_RELATIVE_PATH: &str = ".aife/reports/release-package/latest.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePackagePlan {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub profile: String,
    pub target: String,
    pub architecture: String,
    pub runtime_package_mode: String,
    pub frame_limit: u64,
    pub headless_surface_gate: bool,
    pub real_window_smoke: String,
    pub application: BuildProfileApplication,
    pub release: BuildProfileRelease,
    pub output_dir: PathBuf,
    pub entrypoint_relative_path: String,
    pub runtime_package_relative_path: String,
    pub manifest_relative_path: String,
}

impl ReleasePackagePlan {
    pub fn from_profile(
        project_root: &Path,
        project: &ProjectManifest,
        profile: &BuildProfile,
    ) -> Result<Self, Vec<BuildProfileValidationIssue>> {
        let issues = profile.validation_issues();
        if !issues.is_empty() {
            return Err(issues);
        }
        let architecture = profile
            .architecture
            .clone()
            .expect("validated release profile has architecture");
        let application = profile
            .application
            .clone()
            .expect("validated release profile has application identity");
        let release = profile
            .release
            .clone()
            .expect("validated release profile has release settings");
        let entrypoint_relative_path = format!("{}.exe", application.executable_name);
        let output_dir = project_root
            .join("Build")
            .join("Windows")
            .join(&architecture)
            .join(&profile.profile)
            .join(&application.executable_name);
        Ok(Self {
            schema_version: RELEASE_PACKAGE_PLAN_SCHEMA_VERSION.to_string(),
            project_id: project.project_id.clone(),
            project_name: project.project_name.clone(),
            profile: profile.profile.clone(),
            target: profile.target.clone(),
            architecture,
            runtime_package_mode: profile.runtime_package_mode.clone(),
            frame_limit: profile.frame_limit,
            headless_surface_gate: profile.headless_surface_gate,
            real_window_smoke: profile.real_window_smoke.clone(),
            application,
            release,
            output_dir,
            entrypoint_relative_path,
            runtime_package_relative_path: "data/runtime_package".to_string(),
            manifest_relative_path: "package-manifest.json".to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleasePackageStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleasePackageReportLevel {
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageDiagnostic {
    pub code: String,
    pub stage: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageReport {
    pub schema_version: String,
    pub status: ReleasePackageStatus,
    pub report_level: ReleasePackageReportLevel,
    pub project_id: String,
    pub profile: String,
    pub target: String,
    pub architecture: String,
    pub display_name: String,
    pub display_version: String,
    pub entrypoint: String,
    pub runtime_package: String,
    pub report_path: String,
    pub output_dir: String,
    pub manifest_path: String,
    pub runtime_content_hash: String,
    pub release_payload_hash: String,
    pub payload_file_count: usize,
    pub resource_readback: Option<WindowsExecutableResourceReadback>,
    pub application: ReleasePackageApplicationReport,
    pub runtime: ReleasePackageRuntimeReport,
    pub entrypoint_evidence: ReleasePackageEntrypointReport,
    pub resource: ReleasePackageResourceReport,
    pub layout: ReleasePackageLayoutReport,
    pub payload_hash: ReleasePackagePayloadHashReport,
    pub verification: ReleasePackageVerificationReport,
    pub diagnostics: Vec<ReleasePackageDiagnostic>,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageApplicationReport {
    pub display_name: String,
    pub executable_name: String,
    pub company_name: String,
    pub display_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageRuntimeReport {
    pub relative_path: String,
    pub content_hash: String,
    pub formal_load_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageEntrypointReport {
    pub relative_path: String,
    pub exists: bool,
    pub role_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageResourceReport {
    pub stamp_readback_verified: bool,
    pub icon_sizes: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageLayoutReport {
    pub kind: String,
    pub portable: bool,
    pub include_reports: bool,
    pub payload_file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackagePayloadHashReport {
    pub algorithm: String,
    pub value: String,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageVerificationReport {
    pub manifest_valid: bool,
    pub inventory_valid: bool,
    pub runtime_load_passed: bool,
    pub resource_readback_passed: bool,
    pub publish_validated: bool,
    pub explicit_process_status: String,
    pub explicit_process_passed: bool,
    pub process_exit_code: Option<i32>,
    pub child_player_exit_code: Option<i32>,
    pub child_frames_completed: Option<u64>,
    pub process_report_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePackageBuildRequest {
    pub project_root: PathBuf,
    pub build_profile_path: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub player_executable: Option<PathBuf>,
    pub report_path: Option<PathBuf>,
    pub report_level: ReleasePackageReportLevel,
    pub verify_process: bool,
    pub process_timeout_ms: u64,
    pub explicit_output: Option<crate::ExplicitExportOutput>,
    pub explicit_report_output: Option<crate::ExplicitExportOutput>,
}

impl ReleasePackageBuildRequest {
    pub fn windows_release(project_root: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        Self {
            build_profile_path: project_root.join("BuildProfiles/windows.release.json"),
            output_dir: None,
            player_executable: crate::desktop_export::default_player_executable_for_project(
                &project_root,
            ),
            report_path: Some(project_root.join(RELEASE_PACKAGE_REPORT_RELATIVE_PATH)),
            project_root,
            report_level: ReleasePackageReportLevel::Summary,
            verify_process: true,
            process_timeout_ms: 30_000,
            explicit_output: None,
            explicit_report_output: None,
        }
    }

    pub fn with_explicit_output(mut self, output: crate::ExplicitExportOutput) -> Self {
        self.explicit_output = Some(output);
        self
    }

    pub fn with_explicit_report_output(mut self, output: crate::ExplicitExportOutput) -> Self {
        self.explicit_report_output = Some(output);
        self
    }
}

pub struct ReleasePackageBuilder;

impl ReleasePackageBuilder {
    pub fn build(request: &ReleasePackageBuildRequest) -> ReleasePackageReport {
        let project =
            match read_json::<ProjectManifest>(&request.project_root.join("project.aife.json")) {
                Ok(project) => project,
                Err(message) => {
                    return failed_report(
                        request,
                        "",
                        diagnostic(
                            "release_identity_invalid",
                            "read_project_manifest",
                            message,
                            Some(request.project_root.join("project.aife.json")),
                            "Repair the typed project manifest before building a release package.",
                        ),
                    );
                }
            };
        let profile = match read_json::<BuildProfile>(&request.build_profile_path) {
            Ok(profile) => profile,
            Err(message) => {
                return failed_report(
                    request,
                    &project.project_id,
                    diagnostic(
                        "release_profile_schema_unsupported",
                        "read_build_profile",
                        message,
                        Some(request.build_profile_path.clone()),
                        "Repair BuildProfiles/windows.release.json as build-profile.v2.",
                    ),
                );
            }
        };
        let mut plan =
            match ReleasePackagePlan::from_profile(&request.project_root, &project, &profile) {
                Ok(plan) => plan,
                Err(issues) => {
                    let mut report = empty_report(request, &project.project_id);
                    report.diagnostics.extend(issues.into_iter().map(|issue| {
                        diagnostic(
                            issue.code,
                            issue.field,
                            issue.message,
                            Some(request.build_profile_path.clone()),
                            issue.next_action,
                        )
                    }));
                    report.next_action = report
                        .diagnostics
                        .first()
                        .map(|diagnostic| diagnostic.next_action.clone())
                        .unwrap_or_default();
                    return finish_report(request, report);
                }
            };
        if let Some(output_dir) = &request.output_dir {
            plan.output_dir = output_dir.clone();
        }
        if plan.output_dir.starts_with(&request.project_root) {
            let scope = match crate::ProjectWriteScope::open(&request.project_root) {
                Ok(scope) => scope,
                Err(error) => {
                    return failed_report(
                        request,
                        &project.project_id,
                        diagnostic(
                            error.code,
                            "prepare_release_output",
                            error.to_string(),
                            Some(plan.output_dir.clone()),
                            "Repair the project Build output path and retry.",
                        ),
                    );
                }
            };
            let relative = plan
                .output_dir
                .strip_prefix(&request.project_root)
                .expect("project-owned output has a relative path");
            if let Err(error) = scope.ensure_directory(relative) {
                return failed_report(
                    request,
                    &project.project_id,
                    diagnostic(
                        error.code,
                        "prepare_release_output",
                        error.to_string(),
                        Some(plan.output_dir.clone()),
                        "Remove the escaping link or reparse point and retry.",
                    ),
                );
            }
        } else if !request
            .explicit_output
            .as_ref()
            .is_some_and(|output| output.authorizes(&plan.output_dir))
        {
            return failed_report(
                request,
                &project.project_id,
                diagnostic(
                    "project_write.explicit_export_required",
                    "authorize_release_output",
                    "External release output requires ExplicitExportOutput authorization.",
                    Some(plan.output_dir.clone()),
                    "Choose the external export directory through the trusted editor picker.",
                ),
            );
        }
        let assembly = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&request.project_root)
                .with_build_profile_path(&request.build_profile_path),
        );
        if assembly.status != ProjectRuntimePackageAssemblyStatus::Success {
            let mut report = report_from_plan(request, &plan);
            report.diagnostics.extend(
                assembly
                    .report
                    .diagnostics
                    .into_iter()
                    .filter(|diagnostic| {
                        diagnostic.severity == ProjectRuntimePackageAssemblySeverity::Error
                    })
                    .map(|assembly| {
                        diagnostic(
                            &assembly.code,
                            "assemble_runtime_package",
                            assembly.message,
                            assembly.path.map(PathBuf::from),
                            assembly.suggestion.as_deref().unwrap_or(
                                "Fix the project source reported by ProjectRuntimePackageAssembler.",
                            ),
                        )
                    }),
            );
            report.next_action = first_next_action(&report.diagnostics);
            return finish_report(request, report);
        }
        let Some(build_input) = assembly.build_input else {
            return failed_report(
                request,
                &project.project_id,
                diagnostic(
                    "release_runtime_package_load_failed",
                    "assemble_runtime_package",
                    "successful assembly did not return RuntimePackageBuildInput",
                    None,
                    "Inspect the ProjectRuntimePackageAssembler result contract.",
                ),
            );
        };
        let Some(active_scene_id) = assembly.active_scene_id else {
            return failed_report(
                request,
                &project.project_id,
                diagnostic(
                    "release_runtime_package_load_failed",
                    "assemble_runtime_package",
                    "successful assembly did not return activeSceneId",
                    None,
                    "Save a valid default Scene before building a release package.",
                ),
            );
        };
        let player_executable = match resolve_release_player_executable(
            request,
            &project,
            &build_input.project.runtime_module,
        ) {
            Ok(player_executable) => player_executable,
            Err(diagnostics) => {
                let mut report = report_from_plan(request, &plan);
                report.diagnostics = diagnostics;
                report.next_action = first_next_action(&report.diagnostics);
                return finish_report(request, report);
            }
        };
        if let Err(error) = crate::ProjectPlayerArtifact::inspect(
            &player_executable,
            &build_input.project.runtime_module,
        ) {
            return failed_report(
                request,
                &project.project_id,
                diagnostic(
                    error.code,
                    "verify_project_player_artifact",
                    error.message,
                    Some(player_executable.clone()),
                    error.next_action,
                ),
            );
        }
        let icon = match resolve_release_icon_asset(&request.project_root, &plan.application.icon) {
            Ok(icon) => icon,
            Err(error) => {
                return failed_report(
                    request,
                    &project.project_id,
                    diagnostic(
                        error.code,
                        error.stage,
                        error.message,
                        Some(error.path),
                        error.next_action,
                    ),
                );
            }
        };

        let mut process_verification = None;
        let publish_result = atomic_directory_publish(
            &plan.output_dir,
            |staging_dir| {
                write_release_staging(
                    staging_dir,
                    &request.project_root,
                    &plan,
                    &player_executable,
                    &icon,
                    &active_scene_id,
                    &build_input,
                )?;
                if request.verify_process {
                    let verification =
                        verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
                            exported_package_dir: staging_dir.to_path_buf(),
                            mode: "headless-gate".to_string(),
                            frame_limit: plan.frame_limit.max(1),
                            report_path: Some(release_process_report_path(request)),
                            timeout_ms: request.process_timeout_ms.max(1),
                            screenshot: false,
                            screenshot_path: None,
                        });
                    let passed = verification.status
                        == ExportedPlayerProcessVerificationStatus::Passed
                        && verification.process_exit_code == Some(0)
                        && verification.child_player_exit_code == Some(0);
                    let summary = format!(
                        "status={:?} process={:?} child={:?} frames={:?}",
                        verification.status,
                        verification.process_exit_code,
                        verification.child_player_exit_code,
                        verification.child_frames_completed
                    );
                    process_verification = Some(verification);
                    if !passed {
                        return Err(format!("release_process_verification_failed: {summary}"));
                    }
                }
                Ok(())
            },
            |package_dir| {
                verify_release_package_directory(package_dir)
                    .map(|_| ())
                    .map_err(|diagnostics| format_verification_diagnostics(&diagnostics))
            },
        );
        if let Err(error) = publish_result {
            let code = if error
                .message
                .contains("release_process_verification_failed")
            {
                "release_launch_failed"
            } else {
                match error.code {
                    "output_publish_busy" => "release_publish_busy",
                    "output_publish_rollback_failed" => "release_publish_rollback_failed",
                    _ => "release_publish_failed",
                }
            };
            let mut report = report_from_plan(request, &plan);
            apply_process_verification(&mut report, process_verification.as_ref());
            if request.verify_process {
                report.verification.process_report_path =
                    Some(release_process_report_path(request).display().to_string());
            }
            report.diagnostics.push(diagnostic(
                code,
                error.code,
                error.message,
                Some(error.path),
                if code == "release_launch_failed" {
                    "Inspect the package-external process verification report and fix the first Runtime diagnostic."
                } else {
                    error.next_action
                },
            ));
            report.next_action = first_next_action(&report.diagnostics);
            return finish_report(request, report);
        }
        match verify_release_package_directory(&plan.output_dir) {
            Ok(verification) => {
                let mut report = report_from_plan(request, &plan);
                report.status = ReleasePackageStatus::Success;
                report.runtime_content_hash = verification.manifest.runtime_content_hash.clone();
                report.release_payload_hash = verification.manifest.release_payload_hash.clone();
                report.payload_file_count = verification.manifest.files.len();
                report.application = ReleasePackageApplicationReport {
                    display_name: verification.manifest.application.display_name.clone(),
                    executable_name: verification.manifest.application.executable_name.clone(),
                    company_name: verification.manifest.application.company_name.clone(),
                    display_version: verification.manifest.application.display_version.clone(),
                };
                report.runtime.content_hash = verification.manifest.runtime_content_hash.clone();
                report.runtime.formal_load_passed = true;
                report.entrypoint_evidence.exists = true;
                report.entrypoint_evidence.role_verified = true;
                report.resource.stamp_readback_verified = true;
                report.resource.icon_sizes = verification.resource_readback.icon_sizes.clone();
                report.layout.payload_file_count = verification.manifest.files.len();
                report.payload_hash.value = verification.manifest.release_payload_hash.clone();
                report.payload_hash.verified = true;
                report.verification = ReleasePackageVerificationReport {
                    manifest_valid: true,
                    inventory_valid: true,
                    runtime_load_passed: true,
                    resource_readback_passed: true,
                    publish_validated: true,
                    ..ReleasePackageVerificationReport::default()
                };
                apply_process_verification(&mut report, process_verification.as_ref());
                if request.verify_process {
                    report.verification.process_report_path =
                        Some(release_process_report_path(request).display().to_string());
                }
                report.resource_readback = Some(verification.resource_readback);
                report.next_action =
                    "Release package is ready at the published output directory.".to_string();
                finish_report(request, report)
            }
            Err(diagnostics) => {
                let mut report = report_from_plan(request, &plan);
                report.diagnostics = diagnostics;
                report.next_action = first_next_action(&report.diagnostics);
                finish_report(request, report)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePackageVerification {
    pub manifest: ReleasePackageManifest,
    pub resource_readback: WindowsExecutableResourceReadback,
}

pub fn verify_release_package_directory(
    package_dir: &Path,
) -> Result<ReleasePackageVerification, Vec<ReleasePackageDiagnostic>> {
    let manifest_path = package_dir.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME);
    let manifest = read_json::<ReleasePackageManifest>(&manifest_path).map_err(|message| {
        vec![diagnostic(
            "release_manifest_invalid",
            "read_release_manifest",
            message,
            Some(manifest_path.clone()),
            "Rebuild the release package manifest from staging payloads.",
        )]
    })?;
    let mut diagnostics = validate_release_package_manifest(&manifest)
        .into_iter()
        .map(|manifest| {
            diagnostic(
                manifest.code,
                "validate_release_manifest",
                manifest.message,
                Some(PathBuf::from(manifest.path)),
                manifest.next_action,
            )
        })
        .collect::<Vec<_>>();
    let actual_paths = match collect_release_files(package_dir) {
        Ok(files) => files
            .into_iter()
            .filter(|path| path != RELEASE_PACKAGE_MANIFEST_FILE_NAME)
            .collect::<Vec<_>>(),
        Err(message) => {
            diagnostics.push(diagnostic(
                "release_manifest_invalid",
                "scan_release_payload",
                message,
                Some(package_dir.to_path_buf()),
                "Remove symlinks and rebuild the portable release directory.",
            ));
            Vec::new()
        }
    };
    let declared_paths = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let actual_path_set = actual_paths
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if declared_paths != actual_path_set {
        diagnostics.push(diagnostic(
            "release_manifest_invalid",
            "validate_payload_inventory",
            format!(
                "declared payload paths differ from disk: declared={declared_paths:?}, actual={actual_path_set:?}"
            ),
            Some(package_dir.to_path_buf()),
            "Rebuild the package so manifest files exactly own the release payload.",
        ));
    }
    for file in &manifest.files {
        match safe_join_runtime_package(package_dir, &file.path) {
            Ok(path) if path.is_file() => match fs::read(&path) {
                Ok(bytes) => {
                    if file.size != bytes.len() as u64 || file.sha256 != sha256_prefixed(&bytes) {
                        diagnostics.push(diagnostic(
                            "release_payload_hash_mismatch",
                            "verify_payload_file",
                            format!("payload size/hash mismatch for {}", file.path),
                            Some(PathBuf::from(&file.path)),
                            "Rebuild the release package from verified staging payloads.",
                        ));
                    }
                }
                Err(error) => diagnostics.push(diagnostic(
                    "release_manifest_invalid",
                    "read_payload_file",
                    error.to_string(),
                    Some(PathBuf::from(&file.path)),
                    "Repair the release payload and rebuild.",
                )),
            },
            Ok(_) => diagnostics.push(diagnostic(
                "release_manifest_invalid",
                "verify_payload_file",
                format!("declared payload file is missing: {}", file.path),
                Some(PathBuf::from(&file.path)),
                "Rebuild the release payload inventory.",
            )),
            Err(error) => diagnostics.push(diagnostic(
                "release_path_escape",
                "verify_payload_path",
                error.to_string(),
                Some(PathBuf::from(&file.path)),
                "Use package-relative paths within the release root.",
            )),
        }
        if file
            .path
            .split('/')
            .any(|segment| segment.eq_ignore_ascii_case("reports"))
        {
            diagnostics.push(diagnostic(
                "release_manifest_invalid",
                "validate_release_layout",
                format!("release payload must exclude reports: {}", file.path),
                Some(PathBuf::from(&file.path)),
                "Keep editor reports outside the portable release directory.",
            ));
        }
    }
    let runtime_package_dir =
        match safe_join_runtime_package(package_dir, &manifest.runtime_package) {
            Ok(path) => path,
            Err(error) => {
                diagnostics.push(diagnostic(
                    "release_path_escape",
                    "resolve_runtime_package",
                    error.to_string(),
                    Some(PathBuf::from(&manifest.runtime_package)),
                    "Repair manifest.runtimePackage and rebuild.",
                ));
                package_dir.join("invalid-runtime-package")
            }
        };
    let load = load_runtime_package(&runtime_package_dir);
    if load.diagnostics.has_errors() {
        diagnostics.push(diagnostic(
            "release_runtime_package_load_failed",
            "load_runtime_package",
            load.diagnostics
                .issues
                .iter()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; "),
            Some(PathBuf::from(&manifest.runtime_package)),
            "Fix RuntimePackage generation before publishing the outer release directory.",
        ));
    } else if load
        .value
        .as_ref()
        .and_then(|package| package.manifest.content_hash.as_deref())
        != Some(manifest.runtime_content_hash.as_str())
    {
        diagnostics.push(diagnostic(
            "release_payload_hash_mismatch",
            "verify_runtime_content_hash",
            "release manifest runtimeContentHash differs from RuntimePackage manifest",
            Some(PathBuf::from(&manifest.runtime_package)),
            "Rebuild the release manifest from the formal RuntimePackage loader result.",
        ));
    }
    let entrypoint_path = match safe_join_runtime_package(package_dir, &manifest.entrypoint) {
        Ok(path) => path,
        Err(error) => {
            diagnostics.push(diagnostic(
                "release_path_escape",
                "resolve_entrypoint",
                error.to_string(),
                Some(PathBuf::from(&manifest.entrypoint)),
                "Repair manifest.entrypoint and rebuild.",
            ));
            package_dir.join("invalid-entrypoint.exe")
        }
    };
    let expectation =
        WindowsExecutableResourceExpectation::from_release_manifest(&manifest.application);
    let resource_readback =
        match verify_windows_executable_resource_contract(&entrypoint_path, &expectation) {
            Ok(readback) => readback,
            Err(error) => {
                diagnostics.push(diagnostic(
                    error.code,
                    error.stage,
                    error.message,
                    Some(error.path),
                    error.next_action,
                ));
                WindowsExecutableResourceReadback {
                    product_name: String::new(),
                    company_name: String::new(),
                    file_description: String::new(),
                    product_version: String::new(),
                    file_version: String::new(),
                    copyright: String::new(),
                    original_filename: String::new(),
                    fixed_file_version: [0; 4],
                    fixed_product_version: [0; 4],
                    icon_sizes: Vec::new(),
                    manifest_present: false,
                }
            }
        };
    if diagnostics.is_empty() {
        Ok(ReleasePackageVerification {
            manifest,
            resource_readback,
        })
    } else {
        Err(diagnostics)
    }
}

fn resolve_release_player_executable(
    request: &ReleasePackageBuildRequest,
    project: &ProjectManifest,
    expected_module: &engine_runtime::runtime_package::RuntimeProjectModuleRef,
) -> Result<PathBuf, Vec<ReleasePackageDiagnostic>> {
    if project.runtime_module.source_kind == Some(crate::ProjectRuntimeSourceKind::ProjectRust) {
        if let Some(player_executable) = request
            .player_executable
            .as_ref()
            .filter(|path| path.is_file())
        {
            return Ok(player_executable.clone());
        }

        let build = crate::ProjectPlayerArtifact::build_project_rust(
            crate::ProjectRuntimePlayerArtifactBuildRequest::new(
                &request.project_root,
                crate::default_engine_sdk_root(),
                expected_module.clone(),
            ),
        );
        if build.status == crate::ProjectRuntimePlayerArtifactBuildStatus::Success {
            if let Some(artifact) = build.artifact {
                return Ok(artifact.executable_path);
            }
        }
        let mut diagnostics = build
            .diagnostics
            .into_iter()
            .map(|error| {
                diagnostic(
                    error.code,
                    "build_project_player_artifact",
                    error.message,
                    error.path.map(PathBuf::from),
                    error.next_action,
                )
            })
            .collect::<Vec<_>>();
        if diagnostics.is_empty() {
            diagnostics.push(diagnostic(
                "project_runtime.player_artifact_missing_after_build",
                "build_project_player_artifact",
                "ProjectRust Player artifact build did not return an executable.",
                request.player_executable.clone(),
                "Inspect the ProjectRuntime Player artifact build report.",
            ));
        }
        return Err(diagnostics);
    }

    let Some(player_executable) = request.player_executable.as_ref() else {
        return Err(vec![diagnostic(
            "release_player_template_missing",
            "resolve_player_template",
            "Windows Runtime executable template is missing",
            None,
            "Run cargo build for the configured built-in Player before building a release package.",
        )]);
    };
    if let Err(error) = crate::ProjectPlayerArtifact::ensure_built(
        player_executable,
        &project.runtime_module.player_binary,
    ) {
        return Err(vec![diagnostic(
            error.code,
            "build_project_player_artifact",
            error.message,
            Some(player_executable.clone()),
            error.next_action,
        )]);
    }
    if !player_executable.is_file() {
        return Err(vec![diagnostic(
            "release_player_template_missing",
            "resolve_player_template",
            "Windows Runtime executable template is missing",
            Some(player_executable.clone()),
            "Build the configured built-in Player before building a release package.",
        )]);
    }
    Ok(player_executable.clone())
}

fn write_release_staging(
    staging_dir: &Path,
    project_root: &Path,
    plan: &ReleasePackagePlan,
    player_executable: &Path,
    icon: &crate::ResolvedReleaseIcon,
    active_scene_id: &str,
    build_input: &engine_runtime::runtime_package_builder::RuntimePackageBuildInput,
) -> Result<(), String> {
    let entrypoint_path = staging_dir.join(&plan.entrypoint_relative_path);
    fs::copy(player_executable, &entrypoint_path).map_err(|error| {
        format!(
            "failed to copy Runtime executable template {} to {}: {error}",
            player_executable.display(),
            entrypoint_path.display()
        )
    })?;
    stamp_windows_executable_resources(&entrypoint_path, &plan.application, icon)
        .map_err(|error| error.to_string())?;
    let runtime_package_dir = staging_dir.join(&plan.runtime_package_relative_path);
    let mut runtime_request =
        RuntimePackageBuildRequest::dev_desktop(&runtime_package_dir, active_scene_id);
    runtime_request.project_root = project_root.to_path_buf();
    let runtime_report = RuntimePackageBuilder::build(&runtime_request, build_input);
    if runtime_report.status != RuntimePackageBuildStatus::Success {
        return Err(format!(
            "RuntimePackage build failed: {}",
            runtime_report
                .diagnostics
                .iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let runtime_reports = runtime_package_dir.join("reports");
    if runtime_reports.exists() {
        fs::remove_dir_all(&runtime_reports)
            .map_err(|error| format!("failed to remove release RuntimePackage reports: {error}"))?;
    }
    let load = load_runtime_package(&runtime_package_dir);
    if load.diagnostics.has_errors() {
        return Err(format!(
            "formal RuntimePackage loader rejected release staging: {}",
            load.diagnostics
                .issues
                .iter()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let runtime_content_hash = load
        .value
        .as_ref()
        .and_then(|package| package.manifest.content_hash.clone())
        .ok_or_else(|| "formal RuntimePackage result has no contentHash".to_string())?;
    let mut files = Vec::new();
    for relative_path in collect_release_files(staging_dir)? {
        if relative_path == RELEASE_PACKAGE_MANIFEST_FILE_NAME {
            continue;
        }
        let path = safe_join_runtime_package(staging_dir, &relative_path)
            .map_err(|error| error.to_string())?;
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let roles = if relative_path == plan.entrypoint_relative_path {
            vec![
                ReleasePackageFileRole::Entrypoint,
                ReleasePackageFileRole::Runtime,
            ]
        } else {
            vec![ReleasePackageFileRole::RuntimePayload]
        };
        files.push(ReleasePackageFile {
            path: relative_path,
            size: bytes.len() as u64,
            sha256: sha256_prefixed(&bytes),
            roles,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = ReleasePackageManifest {
        schema_version: RELEASE_PACKAGE_MANIFEST_SCHEMA_VERSION.to_string(),
        application: ReleasePackageApplication {
            display_name: plan.application.display_name.clone(),
            executable_name: plan.application.executable_name.clone(),
            company_name: plan.application.company_name.clone(),
            file_description: plan.application.file_description.clone(),
            display_version: plan.application.display_version.clone(),
            windows_file_version: plan.application.windows_file_version,
            windows_product_version: plan.application.windows_product_version,
            copyright: plan.application.copyright.clone(),
        },
        target: ReleasePackageTarget {
            platform: plan.target.clone(),
            architecture: plan.architecture.clone(),
            profile: plan.profile.clone(),
        },
        launch: ReleasePackageLaunch {
            user_frame_limit: None,
        },
        entrypoint: plan.entrypoint_relative_path.clone(),
        runtime_package: plan.runtime_package_relative_path.clone(),
        runtime_content_hash,
        release_payload_hash: release_payload_hash(&files),
        files,
    };
    let diagnostics = validate_release_package_manifest(&manifest);
    if !diagnostics.is_empty() {
        return Err(diagnostics
            .into_iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; "));
    }
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(staging_dir.join(RELEASE_PACKAGE_MANIFEST_FILE_NAME), bytes)
        .map_err(|error| error.to_string())
}

fn collect_release_files(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    collect_release_files_recursive(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_release_files_recursive(
    root: &Path,
    directory: &Path,
    files: &mut Vec<String>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            return Err(format!(
                "release payload cannot contain symlink: {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_release_files_recursive(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn release_process_report_path(request: &ReleasePackageBuildRequest) -> PathBuf {
    request
        .report_path
        .as_ref()
        .and_then(|path| path.parent())
        .map(|parent| parent.join("process-verification.json"))
        .unwrap_or_else(|| {
            request
                .project_root
                .join(".aife/reports/release-package/process-verification.json")
        })
}

fn apply_process_verification(
    report: &mut ReleasePackageReport,
    verification: Option<&ExportedPlayerProcessVerificationReport>,
) {
    let Some(verification) = verification else {
        report.verification.explicit_process_status = "not_requested".to_string();
        return;
    };
    report.verification.explicit_process_status = match verification.status {
        ExportedPlayerProcessVerificationStatus::Passed => "passed",
        ExportedPlayerProcessVerificationStatus::Failed => "failed",
        ExportedPlayerProcessVerificationStatus::EnvironmentBlocked => "environment_blocked",
    }
    .to_string();
    report.verification.explicit_process_passed = verification.status
        == ExportedPlayerProcessVerificationStatus::Passed
        && verification.process_exit_code == Some(0)
        && verification.child_player_exit_code == Some(0);
    report.verification.process_exit_code = verification.process_exit_code;
    report.verification.child_player_exit_code = verification.child_player_exit_code;
    report.verification.child_frames_completed = verification.child_frames_completed;
}

fn report_from_plan(
    request: &ReleasePackageBuildRequest,
    plan: &ReleasePackagePlan,
) -> ReleasePackageReport {
    ReleasePackageReport {
        schema_version: RELEASE_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
        status: ReleasePackageStatus::Failed,
        report_level: request.report_level,
        project_id: plan.project_id.clone(),
        profile: plan.profile.clone(),
        target: plan.target.clone(),
        architecture: plan.architecture.clone(),
        display_name: plan.application.display_name.clone(),
        display_version: plan.application.display_version.clone(),
        entrypoint: plan.entrypoint_relative_path.clone(),
        runtime_package: plan.runtime_package_relative_path.clone(),
        report_path: request
            .report_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        output_dir: plan.output_dir.display().to_string(),
        manifest_path: plan
            .output_dir
            .join(RELEASE_PACKAGE_MANIFEST_FILE_NAME)
            .display()
            .to_string(),
        runtime_content_hash: String::new(),
        release_payload_hash: String::new(),
        payload_file_count: 0,
        resource_readback: None,
        application: ReleasePackageApplicationReport {
            display_name: plan.application.display_name.clone(),
            executable_name: plan.application.executable_name.clone(),
            company_name: plan.application.company_name.clone(),
            display_version: plan.application.display_version.clone(),
        },
        runtime: ReleasePackageRuntimeReport {
            relative_path: plan.runtime_package_relative_path.clone(),
            ..ReleasePackageRuntimeReport::default()
        },
        entrypoint_evidence: ReleasePackageEntrypointReport {
            relative_path: plan.entrypoint_relative_path.clone(),
            ..ReleasePackageEntrypointReport::default()
        },
        resource: ReleasePackageResourceReport::default(),
        layout: ReleasePackageLayoutReport {
            kind: plan.release.layout.clone(),
            portable: plan.release.layout == "portable-directory-v1",
            include_reports: plan.release.include_reports,
            payload_file_count: 0,
        },
        payload_hash: ReleasePackagePayloadHashReport {
            algorithm: "sha256".to_string(),
            ..ReleasePackagePayloadHashReport::default()
        },
        verification: ReleasePackageVerificationReport::default(),
        diagnostics: Vec::new(),
        next_action: String::new(),
    }
}

fn empty_report(request: &ReleasePackageBuildRequest, project_id: &str) -> ReleasePackageReport {
    ReleasePackageReport {
        schema_version: RELEASE_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
        status: ReleasePackageStatus::Failed,
        report_level: request.report_level,
        project_id: project_id.to_string(),
        profile: "release".to_string(),
        target: "windows".to_string(),
        architecture: "x86_64".to_string(),
        display_name: String::new(),
        display_version: String::new(),
        entrypoint: String::new(),
        runtime_package: "data/runtime_package".to_string(),
        report_path: request
            .report_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        output_dir: request
            .output_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        manifest_path: String::new(),
        runtime_content_hash: String::new(),
        release_payload_hash: String::new(),
        payload_file_count: 0,
        resource_readback: None,
        application: ReleasePackageApplicationReport::default(),
        runtime: ReleasePackageRuntimeReport {
            relative_path: "data/runtime_package".to_string(),
            ..ReleasePackageRuntimeReport::default()
        },
        entrypoint_evidence: ReleasePackageEntrypointReport::default(),
        resource: ReleasePackageResourceReport::default(),
        layout: ReleasePackageLayoutReport::default(),
        payload_hash: ReleasePackagePayloadHashReport {
            algorithm: "sha256".to_string(),
            ..ReleasePackagePayloadHashReport::default()
        },
        verification: ReleasePackageVerificationReport::default(),
        diagnostics: Vec::new(),
        next_action: String::new(),
    }
}

fn failed_report(
    request: &ReleasePackageBuildRequest,
    project_id: &str,
    diagnostic: ReleasePackageDiagnostic,
) -> ReleasePackageReport {
    let mut report = empty_report(request, project_id);
    report.next_action = diagnostic.next_action.clone();
    report.diagnostics.push(diagnostic);
    finish_report(request, report)
}

fn finish_report(
    request: &ReleasePackageBuildRequest,
    mut report: ReleasePackageReport,
) -> ReleasePackageReport {
    if request.report_level == ReleasePackageReportLevel::Off {
        return report;
    }
    if let Some(path) = &request.report_path {
        match serde_json::to_vec_pretty(&report) {
            Ok(bytes) => {
                let write_result = if path.starts_with(&request.project_root) {
                    crate::ProjectWriteScope::open(&request.project_root)
                        .and_then(|scope| {
                            let relative =
                                path.strip_prefix(&request.project_root).map_err(|_| {
                                    crate::ProjectWriteError {
                                        code: "project_write.path_not_relative",
                                        operation: crate::ProjectWriteOperation::WriteAtomic,
                                        relative_path: Some(path.display().to_string()),
                                        source: None,
                                        rollback_error: None,
                                    }
                                })?;
                            scope.write_atomic(relative, &bytes).map(|_| ())
                        })
                        .map_err(|error| error.to_string())
                } else if request
                    .explicit_report_output
                    .as_ref()
                    .is_some_and(|output| output.authorizes(path))
                    || request
                        .explicit_output
                        .as_ref()
                        .is_some_and(|output| output.authorizes(path))
                {
                    engine_runtime::atomic_file_replace::atomic_file_replace(path, &bytes)
                        .map_err(|error| error.to_string())
                } else {
                    Err("external report path lacks ExplicitExportOutput authorization".to_string())
                };
                if let Err(error) = write_result {
                    report.status = ReleasePackageStatus::Failed;
                    report.diagnostics.push(diagnostic(
                        "release_report_write_failed",
                        "write_release_report",
                        error.to_string(),
                        Some(path.clone()),
                        "Repair the project .aife report path; the published package remains independently verifiable.",
                    ));
                    report.next_action = first_next_action(&report.diagnostics);
                }
            }
            Err(error) => {
                report.status = ReleasePackageStatus::Failed;
                report.diagnostics.push(diagnostic(
                    "release_report_write_failed",
                    "serialize_release_report",
                    error.to_string(),
                    Some(path.clone()),
                    "Inspect ReleasePackageReport serialization fields.",
                ));
                report.next_action = first_next_action(&report.diagnostics);
            }
        }
    }
    report
}

fn diagnostic(
    code: impl Into<String>,
    stage: impl Into<String>,
    message: impl Into<String>,
    path: Option<PathBuf>,
    next_action: impl Into<String>,
) -> ReleasePackageDiagnostic {
    ReleasePackageDiagnostic {
        code: code.into(),
        stage: stage.into(),
        message: message.into(),
        path: path.map(|path| path.display().to_string()),
        next_action: next_action.into(),
    }
}

fn first_next_action(diagnostics: &[ReleasePackageDiagnostic]) -> String {
    diagnostics
        .first()
        .map(|diagnostic| diagnostic.next_action.clone())
        .unwrap_or_default()
}

fn format_verification_diagnostics(diagnostics: &[ReleasePackageDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{}@{}: {}",
                diagnostic.code, diagnostic.stage, diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BUILD_PROFILE_SCHEMA_VERSION, BUILD_PROFILE_SCHEMA_VERSION_V1};
    use engine_runtime::runtime_package::RuntimeProjectModuleRef;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_profile_v2_release_plan_is_deterministic() {
        let root = sample_project_root();
        let profile: BuildProfile = serde_json::from_str(
            &fs::read_to_string(root.join("BuildProfiles/windows.release.json")).unwrap(),
        )
        .unwrap();
        let project: ProjectManifest =
            serde_json::from_str(&fs::read_to_string(root.join("project.aife.json")).unwrap())
                .unwrap();

        assert_eq!(profile.schema_version, BUILD_PROFILE_SCHEMA_VERSION);
        assert!(profile.is_release_v2());
        let first = ReleasePackagePlan::from_profile(&root, &project, &profile).unwrap();
        let second = ReleasePackagePlan::from_profile(&root, &project, &profile).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.architecture, "x86_64");
        assert_eq!(first.entrypoint_relative_path, "ComplexShooter.exe");
        assert_eq!(first.runtime_package_relative_path, "data/runtime_package");
        assert!(first
            .output_dir
            .ends_with("Build/Windows/x86_64/release/ComplexShooter"));
    }

    #[test]
    fn project_rust_release_reuses_existing_verified_player_candidate() {
        let project_root = sample_project_root();
        let mut project: ProjectManifest = serde_json::from_str(
            &fs::read_to_string(project_root.join("project.aife.json")).unwrap(),
        )
        .unwrap();
        project.runtime_module.source_kind = Some(crate::ProjectRuntimeSourceKind::ProjectRust);
        let root = unique_temp_dir("release-project-rust-player");
        fs::create_dir_all(&root).unwrap();
        let player = root.join("Game.exe");
        fs::write(&player, b"already-built-project-player").unwrap();
        let mut request = ReleasePackageBuildRequest::windows_release(&project_root);
        request.player_executable = Some(player.clone());

        let resolved = resolve_release_player_executable(
            &request,
            &project,
            &RuntimeProjectModuleRef::explicit_empty(),
        )
        .unwrap();

        assert_eq!(resolved, player);
    }

    #[test]
    fn release_report_can_use_separate_explicit_output_capability() {
        let project_root = unique_temp_dir("release-report-project");
        let release_root = unique_temp_dir("release-report-package");
        let evidence_root = unique_temp_dir("release-report-evidence");
        let report_path = evidence_root.join("release-package-report.json");
        let mut request = ReleasePackageBuildRequest::windows_release(&project_root)
            .with_explicit_output(crate::ExplicitExportOutput::from_user_selected(
                release_root,
            ))
            .with_explicit_report_output(crate::ExplicitExportOutput::from_user_selected(
                &evidence_root,
            ));
        request.report_path = Some(report_path.clone());
        request.report_level = ReleasePackageReportLevel::Trace;

        let report = finish_report(&request, empty_report(&request, "test-project"));

        assert!(report_path.is_file());
        assert!(!report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "release_report_write_failed"));
    }

    #[test]
    fn build_profile_v2_invalid_matrix_fails_closed() {
        let root = sample_project_root();
        let text = fs::read_to_string(root.join("BuildProfiles/windows.release.json")).unwrap();
        let profile: BuildProfile = serde_json::from_str(&text).unwrap();

        let mut invalid = profile.clone();
        invalid.application.as_mut().unwrap().executable_name = "CON".to_string();
        assert_issue(&invalid, "release_executable_name_invalid");

        let mut invalid = profile.clone();
        invalid.architecture = Some("arm64".to_string());
        assert_issue(&invalid, "release_identity_invalid");

        let mut invalid = profile.clone();
        invalid.release.as_mut().unwrap().include_reports = true;
        assert_issue(&invalid, "release_identity_invalid");

        let mut invalid = profile.clone();
        invalid.application.as_mut().unwrap().icon.asset_id = "../icon.png".to_string();
        assert_issue(&invalid, "release_icon_asset_missing");

        let mut unknown: serde_json::Value = serde_json::from_str(&text).unwrap();
        unknown["application"]["unknownIdentityField"] = serde_json::json!(true);
        let error = serde_json::from_value::<BuildProfile>(unknown).unwrap_err();
        assert!(error.to_string().contains("unknown field"));

        let mut invalid_version: serde_json::Value = serde_json::from_str(&text).unwrap();
        invalid_version["application"]["windowsFileVersion"] = serde_json::json!([1, 2, 3, 65536]);
        assert!(serde_json::from_value::<BuildProfile>(invalid_version).is_err());
    }

    #[test]
    fn build_profile_v1_dev_serialization_remains_compatible() {
        let root = sample_project_root();
        let text = fs::read_to_string(root.join("BuildProfiles/windows.dev.json")).unwrap();
        let profile: BuildProfile = serde_json::from_str(&text).unwrap();

        assert_eq!(profile.schema_version, BUILD_PROFILE_SCHEMA_VERSION_V1);
        assert!(profile.validation_issues().is_empty());
        assert!(profile.architecture.is_none());
        assert!(profile.application.is_none());
        assert!(profile.release.is_none());
        let serialized = serde_json::to_value(&profile).unwrap();
        assert!(serialized.get("architecture").is_none());
        assert!(serialized.get("application").is_none());
        assert!(serialized.get("release").is_none());
    }

    #[test]
    fn release_package_builds_portable_manifest_and_replaces_stale_payloads() {
        let project_root = sample_project_root();
        let output_root = unique_temp_dir("release-package");
        fs::create_dir_all(&output_root).unwrap();
        let output_dir = output_root.join("ComplexShooter");
        let report_path = output_root.join("reports/latest.json");
        let mut request = ReleasePackageBuildRequest::windows_release(&project_root);
        let source_template = request
            .player_executable
            .clone()
            .expect("sample project declares a project Player");
        let project: ProjectManifest = read_json(&project_root.join("project.aife.json")).unwrap();
        crate::ProjectPlayerArtifact::ensure_built(
            &source_template,
            &project.runtime_module.player_binary,
        )
        .unwrap();
        let source_hash = fs::read(&source_template).unwrap();
        request.output_dir = Some(output_dir.clone());
        request.report_path = Some(report_path.clone());
        request.explicit_output = Some(crate::ExplicitExportOutput::from_user_selected(
            output_root.clone(),
        ));
        request.report_level = ReleasePackageReportLevel::Trace;
        request.verify_process = false;

        let first = ReleasePackageBuilder::build(&request);
        assert_eq!(
            first.status,
            ReleasePackageStatus::Success,
            "{:?}",
            first.diagnostics
        );
        assert!(first.verification.manifest_valid);
        assert!(first.verification.runtime_load_passed);
        assert!(first.resource.stamp_readback_verified);
        assert!(first.payload_hash.verified);
        assert_eq!(first.application.display_name, "Complex Shooter");
        assert!(report_path.is_file());
        assert!(!output_dir.join("reports").exists());
        assert!(!output_dir.join("data/runtime_package/reports").exists());
        assert!(output_dir.join("ComplexShooter.exe").is_file());
        assert!(output_dir.join("package-manifest.json").is_file());
        assert!(output_dir
            .join("data/runtime_package/cooked/font-main.asset")
            .is_file());
        fs::write(output_dir.join("stale-sentinel.bin"), b"stale").unwrap();

        let second = ReleasePackageBuilder::build(&request);
        assert_eq!(
            second.status,
            ReleasePackageStatus::Success,
            "{:?}",
            second.diagnostics
        );
        assert_eq!(first.release_payload_hash, second.release_payload_hash);
        assert!(!output_dir.join("stale-sentinel.bin").exists());
        assert_eq!(fs::read(&source_template).unwrap(), source_hash);

        let verification = verify_release_package_directory(&output_dir).unwrap();
        assert_eq!(verification.manifest.entrypoint, "ComplexShooter.exe");
        assert!(verification.manifest.files.iter().any(|file| {
            file.path == "ComplexShooter.exe"
                && file.roles.contains(&ReleasePackageFileRole::Entrypoint)
                && file.roles.contains(&ReleasePackageFileRole::Runtime)
        }));
        assert!(verification
            .manifest
            .files
            .iter()
            .all(|file| !file.path.contains("reports/")));
        let manifest_json = fs::read_to_string(output_dir.join("package-manifest.json")).unwrap();
        assert!(!manifest_json.contains(&project_root.display().to_string()));
        assert!(!manifest_json.contains(&output_root.display().to_string()));
    }

    fn assert_issue(profile: &BuildProfile, code: &str) {
        assert!(profile
            .validation_issues()
            .iter()
            .any(|issue| issue.code == code));
    }

    fn sample_project_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("samples")
            .join("complex_shooter_project")
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{stamp}"))
    }
}
