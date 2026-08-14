use crate::ui_model_composer::report_path_for_desktop_export;
use crate::{
    BuildProfile, CommandResult, CommandStatus, CommandTransaction, DesktopExportPipeline,
    DesktopExportReport, DesktopExportRequest, DesktopExportStatus, EditorSession,
    ReleasePackageBuildRequest, ReleasePackageBuilder, ReleasePackageReport,
    ReleasePackageReportLevel, ReleasePackageStatus, StateChangeSummary, UndoPolicy,
    RELEASE_PACKAGE_REPORT_RELATIVE_PATH, RELEASE_PACKAGE_REPORT_SCHEMA_VERSION,
};
use editor_ui_model::EditorAssetRef;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::windowed_player::WindowedPlayerRunReport;
use runtime_cli::{
    run_bounded_child_process, BoundedChildProcessExitReason, BoundedChildProcessRequest,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::{Duration, Instant};

pub const EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION: &str = "editor-build-and-run-report.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorBuildAndRunMode {
    UserWindowed,
    HeadlessVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorBuildAndRunStatus {
    NotStarted,
    ExportFailed,
    LaunchFailed,
    Launched,
    VerificationPassed,
    VerificationFailed,
    EnvironmentBlocked,
}

impl EditorBuildAndRunStatus {
    pub fn is_success(self) -> bool {
        matches!(self, Self::Launched | Self::VerificationPassed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorBuildAndRunDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorBuildAndRunDiagnostic {
    pub severity: EditorBuildAndRunDiagnosticSeverity,
    pub code: String,
    pub domain: String,
    pub stage: String,
    pub path: Option<String>,
    pub message: String,
    pub next_action: Option<String>,
}

impl EditorBuildAndRunDiagnostic {
    fn error(
        code: impl Into<String>,
        stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: EditorBuildAndRunDiagnosticSeverity::Error,
            code: code.into(),
            domain: "build".to_string(),
            stage: stage.into(),
            path: None,
            message: message.into(),
            next_action: None,
        }
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn with_next_action(mut self, next_action: impl Into<String>) -> Self {
        self.next_action = Some(next_action.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorBuildAndRunDesktopExportSummary {
    pub status: String,
    pub package_dir: Option<String>,
    pub runtime_package_dir: Option<String>,
    pub game_exe_path: Option<String>,
    pub desktop_export_report_path: Option<String>,
    pub diagnostic_count: usize,
}

impl EditorBuildAndRunDesktopExportSummary {
    fn empty() -> Self {
        Self {
            status: "not_started".to_string(),
            package_dir: None,
            runtime_package_dir: None,
            game_exe_path: None,
            desktop_export_report_path: None,
            diagnostic_count: 0,
        }
    }

    fn from_report(report: &DesktopExportReport) -> Self {
        Self {
            status: match report.status {
                DesktopExportStatus::Success => "success".to_string(),
                DesktopExportStatus::Failed => "failed".to_string(),
            },
            package_dir: Some(report.package_dir.clone()),
            runtime_package_dir: Some(report.runtime_package_dir.clone()),
            game_exe_path: report.player_executable.clone(),
            desktop_export_report_path: Some(report_path_for_desktop_export(report)),
            diagnostic_count: report.diagnostics.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorBuildAndRunLaunchSummary {
    pub attempted: bool,
    pub started: bool,
    pub process_id: Option<u32>,
    pub working_dir: Option<String>,
    pub executable_path: Option<String>,
    pub args: Vec<String>,
    pub start_error: Option<String>,
}

impl EditorBuildAndRunLaunchSummary {
    fn empty() -> Self {
        Self {
            attempted: false,
            started: false,
            process_id: None,
            working_dir: None,
            executable_path: None,
            args: Vec::new(),
            start_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorBuildAndRunVerificationSummary {
    pub attempted: bool,
    pub status: String,
    pub verification_report_path: Option<String>,
    pub child_report_path: Option<String>,
    pub process_exit_reason: String,
    pub process_exit_code: Option<i32>,
    pub process_elapsed_ms: u128,
    pub child_player_exit_code: Option<i32>,
    pub child_frames_completed: Option<u64>,
    pub stdout_summary: String,
    pub stderr_summary: String,
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub process_kill_error: Option<String>,
    pub process_wait_error: Option<String>,
    pub process_reader_join_error: Option<String>,
}

impl EditorBuildAndRunVerificationSummary {
    fn empty() -> Self {
        Self {
            attempted: false,
            status: "not_requested".to_string(),
            verification_report_path: None,
            child_report_path: None,
            process_exit_reason: "not_started".to_string(),
            process_exit_code: None,
            process_elapsed_ms: 0,
            child_player_exit_code: None,
            child_frames_completed: None,
            stdout_summary: String::new(),
            stderr_summary: String::new(),
            stdout_total_bytes: 0,
            stderr_total_bytes: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            process_kill_error: None,
            process_wait_error: None,
            process_reader_join_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorBuildAndRunDurationSummary {
    pub export_duration_ms: u128,
    pub launch_duration_ms: u128,
    pub total_duration_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorBuildAndRunArtifact {
    pub artifact_id: String,
    pub label: String,
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorBuildAndRunReport {
    pub schema_version: String,
    pub status: EditorBuildAndRunStatus,
    pub project_root: Option<String>,
    pub profile_id: String,
    pub target: String,
    pub run_mode: EditorBuildAndRunMode,
    pub report_path: Option<String>,
    pub desktop_export: EditorBuildAndRunDesktopExportSummary,
    pub launch: EditorBuildAndRunLaunchSummary,
    pub verification: EditorBuildAndRunVerificationSummary,
    pub duration: EditorBuildAndRunDurationSummary,
    pub diagnostics: Vec<EditorBuildAndRunDiagnostic>,
    pub artifacts: Vec<EditorBuildAndRunArtifact>,
}

impl EditorBuildAndRunReport {
    fn new(profile_id: String, run_mode: EditorBuildAndRunMode) -> Self {
        Self {
            schema_version: EDITOR_BUILD_AND_RUN_REPORT_SCHEMA_VERSION.to_string(),
            status: EditorBuildAndRunStatus::NotStarted,
            project_root: None,
            profile_id,
            target: "windows".to_string(),
            run_mode,
            report_path: None,
            desktop_export: EditorBuildAndRunDesktopExportSummary::empty(),
            launch: EditorBuildAndRunLaunchSummary::empty(),
            verification: EditorBuildAndRunVerificationSummary::empty(),
            duration: EditorBuildAndRunDurationSummary::default(),
            diagnostics: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    fn from_export(
        profile_id: String,
        run_mode: EditorBuildAndRunMode,
        project_root: &Path,
        desktop_report: &DesktopExportReport,
    ) -> Self {
        let report_path = report_path_for_editor_build_and_run(&desktop_report.package_dir);
        let mut report = Self::new(profile_id, run_mode);
        report.project_root = Some(project_root.display().to_string());
        report.report_path = Some(report_path.clone());
        report.desktop_export = EditorBuildAndRunDesktopExportSummary::from_report(desktop_report);
        report.artifacts.push(EditorBuildAndRunArtifact {
            artifact_id: "editor-build-and-run-report".to_string(),
            label: "Editor Build And Run Report".to_string(),
            path: report_path,
            kind: "json".to_string(),
        });
        report.artifacts.push(EditorBuildAndRunArtifact {
            artifact_id: "desktop-export-report".to_string(),
            label: "Desktop Export Report".to_string(),
            path: report_path_for_desktop_export(desktop_report),
            kind: "json".to_string(),
        });
        report.artifacts.push(EditorBuildAndRunArtifact {
            artifact_id: "windowed-player-report".to_string(),
            label: "Windowed Player Report".to_string(),
            path: desktop_report.player_report_path.clone(),
            kind: "json".to_string(),
        });
        report
    }
}

impl EditorSession {
    pub(crate) fn reload_release_profile_cache(&mut self) -> Result<bool, String> {
        self.release_profile_cache = None;
        self.release_profile_source_hash = None;
        self.release_profile_dirty = false;
        let Some(project) = self.active_project_session.as_ref() else {
            return Ok(false);
        };
        let path = project
            .project_root
            .join("BuildProfiles/windows.release.json");
        if !path.is_file() {
            return Ok(false);
        }
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let profile = serde_json::from_slice::<BuildProfile>(&bytes)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        self.release_profile_source_hash = Some(sha256_prefixed(&bytes));
        self.release_profile_cache = Some(profile);
        Ok(true)
    }

    pub(crate) fn reload_release_package_report_cache(&mut self) -> Result<bool, String> {
        self.last_release_package_report = None;
        let Some(project) = self.active_project_session.as_ref() else {
            return Ok(false);
        };
        let path = project
            .project_root
            .join(RELEASE_PACKAGE_REPORT_RELATIVE_PATH);
        if !path.is_file() {
            return Ok(false);
        }
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let report = serde_json::from_slice::<ReleasePackageReport>(&bytes)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        if report.schema_version != RELEASE_PACKAGE_REPORT_SCHEMA_VERSION {
            return Err(format!(
                "release report schema '{}' is not supported",
                report.schema_version
            ));
        }
        if report.project_id != project.manifest.project_id {
            return Err(format!(
                "release report projectId '{}' does not match active projectId '{}'",
                report.project_id, project.manifest.project_id
            ));
        }
        self.last_release_package_report = Some(report);
        Ok(true)
    }

    pub(crate) fn set_release_profile_icon(
        &mut self,
        transaction: &mut CommandTransaction,
        asset_ref: EditorAssetRef,
    ) -> CommandResult {
        transaction
            .read_set
            .push("BuildProfiles/windows.release.json".to_string());
        transaction
            .write_set
            .push("release_profile.draft.application.icon".to_string());
        transaction.undo_policy = UndoPolicy::SnapshotReady;
        if !matches!(asset_ref.asset_type_id.as_str(), "texture" | "sprite") {
            self.push_error(
                transaction,
                "editor.release_profile.icon_type_invalid",
                "Release profile icon must reference a Texture or Sprite authoring asset.",
                Some("Choose a Texture or Sprite from Asset Picker."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let Some(profile) = self.release_profile_cache.as_mut() else {
            self.push_error(
                transaction,
                "editor.release_profile.not_loaded",
                "Cannot edit release icon before loading BuildProfiles/windows.release.json.",
                Some("Open a project with a valid windows.release profile."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let Some(application) = profile.application.as_mut() else {
            self.push_error(
                transaction,
                "editor.release_profile.application_missing",
                "Release profile has no application identity to receive an icon.",
                Some("Repair the build-profile.v2 application section."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let before = application.icon.asset_id.clone();
        application.icon.asset_id = asset_ref.asset_id.clone();
        self.release_profile_dirty = true;
        transaction.state_changes.push(StateChangeSummary {
            kind: "release_profile.icon_draft".to_string(),
            path: "BuildProfiles/windows.release.json#application.icon".to_string(),
            before_summary: Some(before),
            after_summary: Some(asset_ref.asset_id),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn save_release_profile(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction
            .read_set
            .push("release_profile.draft".to_string());
        transaction
            .write_set
            .push("BuildProfiles/windows.release.json".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let Some(project) = self.active_project_session.as_ref() else {
            self.push_error(
                transaction,
                "editor.release_profile.no_project",
                "Cannot save a release profile before opening a project.",
                Some("Open a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let Some(profile) = self.release_profile_cache.clone() else {
            self.push_error(
                transaction,
                "editor.release_profile.not_loaded",
                "No release profile draft is loaded.",
                Some("Create or repair BuildProfiles/windows.release.json."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let issues = profile.validation_issues();
        if let Some(issue) = issues.first() {
            self.push_error(
                transaction,
                issue.code,
                issue.message.clone(),
                Some(issue.next_action),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let path = project
            .project_root
            .join("BuildProfiles/windows.release.json");
        let current_bytes = match project
            .write_scope()
            .read("BuildProfiles/windows.release.json")
        {
            Ok(bytes) => bytes,
            Err(error) => {
                self.push_error(
                    transaction,
                    "editor.release_profile.source_read_failed",
                    format!("Failed to read release profile source before save: {error}"),
                    Some("Restore the profile file and reload the project."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        if self.release_profile_source_hash.as_deref()
            != Some(sha256_prefixed(&current_bytes).as_str())
        {
            self.push_error(
                transaction,
                "editor.release_profile.source_changed",
                "Release profile changed outside the active draft transaction.",
                Some("Reload the project and reapply the profile edit."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let mut bytes =
            serde_json::to_vec_pretty(&profile).expect("validated BuildProfile must serialize");
        bytes.push(b'\n');
        if let Err(error) = project
            .write_scope()
            .write_atomic("BuildProfiles/windows.release.json", &bytes)
        {
            self.push_error(
                transaction,
                "editor.release_profile.save_failed",
                error.to_string(),
                Some("Repair the BuildProfiles directory and retry Save Release Profile."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        self.release_profile_source_hash = Some(sha256_prefixed(&bytes));
        self.release_profile_dirty = false;
        self.push_info(
            transaction,
            "editor.release_profile.saved",
            format!("Saved release profile: {}", path.display()),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn build_release_package(
        &mut self,
        transaction: &mut CommandTransaction,
        profile_id: Option<String>,
    ) -> CommandResult {
        self.build_release_package_with_overrides(
            transaction,
            profile_id,
            None,
            None,
            ReleasePackageReportLevel::Summary,
            true,
        )
    }

    pub(crate) fn build_release_package_with_overrides(
        &mut self,
        transaction: &mut CommandTransaction,
        profile_id: Option<String>,
        player_executable: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        report_level: ReleasePackageReportLevel,
        verify_process: bool,
    ) -> CommandResult {
        transaction
            .read_set
            .push("BuildProfiles/windows.release.json".to_string());
        transaction
            .write_set
            .push("build.release_package.last_report".to_string());
        transaction
            .write_set
            .push(RELEASE_PACKAGE_REPORT_RELATIVE_PATH.to_string());
        transaction.undo_policy = UndoPolicy::None;
        let effective_profile = profile_id.unwrap_or_else(|| "windows-release".to_string());
        if effective_profile != "windows-release" {
            self.push_error(
                transaction,
                "editor.release_package.unsupported_profile",
                format!("Release profile {effective_profile} is not supported."),
                Some("Use the windows-release profile."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let Some(project) = self.active_project_session.as_ref() else {
            self.push_error(
                transaction,
                "editor.release_package.no_project",
                "Cannot build a release package before opening a project.",
                Some("Open a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        if self.release_profile_dirty {
            self.push_error(
                transaction,
                "editor.release_package.profile_dirty",
                "Release profile has unsaved draft changes.",
                Some("Save Release Profile before building."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let project_root = project.project_root.clone();
        let mut request = ReleasePackageBuildRequest::windows_release(&project_root);
        if let Some(player_executable) = player_executable {
            request.player_executable = Some(player_executable);
        }
        if let Some(output_dir) = output_dir {
            request.explicit_output = Some(crate::ExplicitExportOutput::from_user_selected(
                output_dir.clone(),
            ));
            request.output_dir = Some(output_dir);
        }
        request.report_level = report_level;
        request.verify_process = verify_process;
        let report = ReleasePackageBuilder::build(&request);
        let status = report.status;
        let output = report.output_dir.clone();
        let diagnostics = report.diagnostics.len();
        self.last_release_package_report = Some(report);
        transaction.state_changes.push(StateChangeSummary {
            kind: "build.release_package".to_string(),
            path: "build.release_package.last_report".to_string(),
            before_summary: None,
            after_summary: Some(format!(
                "{status:?} output={output} diagnostics={diagnostics}"
            )),
        });
        if status == ReleasePackageStatus::Success {
            self.push_info(
                transaction,
                "editor.release_package.success",
                format!("Built release package: {output}"),
            );
            self.finish_transaction(transaction.clone(), CommandStatus::Committed)
        } else {
            let report = self
                .last_release_package_report
                .as_ref()
                .expect("release report was cached");
            let first = report.diagnostics.first();
            self.push_error(
                transaction,
                first
                    .map(|diagnostic| diagnostic.code.as_str())
                    .unwrap_or("editor.release_package.failed"),
                first
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| "Release package build failed.".to_string()),
                first.map(|diagnostic| diagnostic.next_action.as_str()),
            );
            self.finish_transaction(transaction.clone(), CommandStatus::Failed)
        }
    }

    pub(crate) fn export_desktop_package(
        &mut self,
        transaction: &mut CommandTransaction,
        profile_id: Option<String>,
    ) -> CommandResult {
        transaction
            .read_set
            .push("project_manifest.project.aife.json".to_string());
        transaction
            .read_set
            .push("project.default_scene".to_string());
        transaction
            .write_set
            .push("build_export.last_report".to_string());
        transaction.write_set.push("Build/Windows/dev".to_string());
        transaction.undo_policy = UndoPolicy::None;

        if profile_id
            .as_deref()
            .is_some_and(|profile| profile != "windows-dev")
        {
            self.push_error(
                transaction,
                "editor.build_export.unsupported_profile",
                format!(
                    "Build profile {} is not supported in v1.",
                    profile_id.unwrap_or_default()
                ),
                Some("Use the windows-dev profile."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }

        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.build_export.no_project",
                "Cannot export before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };

        let project_root = session.project_root.clone();
        let report = export_windows_dev_package(&project_root);
        let status = report.status;
        let package_dir = report.package_dir.clone();
        let report_path = report_path_for_desktop_export(&report);
        let diagnostic_count = report.diagnostics.len();
        self.last_desktop_export_report = Some(report.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "build_export.desktop_export".to_string(),
            path: "build_export.last_report".to_string(),
            before_summary: None,
            after_summary: Some(format!(
                "{:?} package={} diagnostics={}",
                status, package_dir, diagnostic_count
            )),
        });
        if status == DesktopExportStatus::Success {
            self.push_info(
                transaction,
                "editor.build_export.success",
                format!("Exported Windows package: {package_dir}"),
            );
            self.finish_transaction(transaction.clone(), CommandStatus::Committed)
        } else {
            self.push_error(
                transaction,
                "editor.build_export.failed",
                format!("Desktop export failed. Read report: {report_path}"),
                Some("Open the build report and fix the first error diagnostic."),
            );
            self.finish_transaction(transaction.clone(), CommandStatus::Failed)
        }
    }

    pub(crate) fn build_and_run_desktop_package(
        &mut self,
        transaction: &mut CommandTransaction,
        profile_id: Option<String>,
    ) -> CommandResult {
        self.build_and_run_desktop_package_with_mode(
            transaction,
            profile_id,
            EditorBuildAndRunMode::UserWindowed,
            30_000,
            3,
        )
    }

    pub(crate) fn build_and_run_desktop_package_with_mode(
        &mut self,
        transaction: &mut CommandTransaction,
        profile_id: Option<String>,
        run_mode: EditorBuildAndRunMode,
        timeout_ms: u64,
        frame_limit: u64,
    ) -> CommandResult {
        transaction
            .read_set
            .push("project_manifest.project.aife.json".to_string());
        transaction
            .read_set
            .push("project.default_scene".to_string());
        transaction
            .write_set
            .push("build_export.last_report".to_string());
        transaction
            .write_set
            .push("build_and_run.last_report".to_string());
        transaction.write_set.push("Build/Windows/dev".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let effective_profile = profile_id.unwrap_or_else(|| "windows-dev".to_string());
        if effective_profile != "windows-dev" {
            let mut report = EditorBuildAndRunReport::new(effective_profile.clone(), run_mode);
            report.status = EditorBuildAndRunStatus::EnvironmentBlocked;
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.unsupported_profile",
                    "validate",
                    format!("Build profile {effective_profile} is not supported in v1."),
                )
                .with_next_action("Use the windows-dev profile."),
            );
            return self.finish_build_and_run_transaction(
                transaction,
                report,
                CommandStatus::Rejected,
            );
        }

        let Some(session) = &self.active_project_session else {
            let mut report = EditorBuildAndRunReport::new(effective_profile, run_mode);
            report.status = EditorBuildAndRunStatus::EnvironmentBlocked;
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.no_project",
                    "validate",
                    "Cannot Build And Run before opening a project.",
                )
                .with_next_action("Open or create a project first."),
            );
            return self.finish_build_and_run_transaction(
                transaction,
                report,
                CommandStatus::Rejected,
            );
        };

        let total_started = Instant::now();
        let project_root = session.project_root.clone();
        let export_started = Instant::now();
        let desktop_report = export_windows_dev_package(&project_root);
        let export_duration_ms = export_started.elapsed().as_millis();
        self.last_desktop_export_report = Some(desktop_report.clone());

        let mut report = EditorBuildAndRunReport::from_export(
            effective_profile,
            run_mode,
            &project_root,
            &desktop_report,
        );
        report.duration.export_duration_ms = export_duration_ms;
        if desktop_report.status != DesktopExportStatus::Success {
            report.status = EditorBuildAndRunStatus::ExportFailed;
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.export_failed",
                    "export",
                    "Desktop export failed before launch.",
                )
                .with_path(report_path_for_desktop_export(&desktop_report))
                .with_next_action(
                    "Open the desktop export report and fix the first error diagnostic.",
                ),
            );
            report.duration.total_duration_ms = total_started.elapsed().as_millis();
            return self.finish_build_and_run_transaction(
                transaction,
                report,
                CommandStatus::Failed,
            );
        }

        let Some(game_exe) = desktop_report.player_executable.as_ref().map(PathBuf::from) else {
            report.status = EditorBuildAndRunStatus::LaunchFailed;
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.game_exe_missing",
                    "launch",
                    "Desktop export succeeded but no staged Game executable is available.",
                )
                .with_path(desktop_report.package_dir.clone())
                .with_next_action("Run cargo build -p runtime_cli before Build And Run."),
            );
            report.duration.total_duration_ms = total_started.elapsed().as_millis();
            return self.finish_build_and_run_transaction(
                transaction,
                report,
                CommandStatus::Failed,
            );
        };

        if !game_exe.exists() {
            report.status = EditorBuildAndRunStatus::LaunchFailed;
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.game_exe_missing",
                    "launch",
                    "Staged Game executable is missing from the exported package.",
                )
                .with_path(game_exe.display().to_string())
                .with_next_action("Rebuild runtime_cli and run Build And Run again."),
            );
            report.duration.total_duration_ms = total_started.elapsed().as_millis();
            return self.finish_build_and_run_transaction(
                transaction,
                report,
                CommandStatus::Failed,
            );
        }

        let launch_started = Instant::now();
        match run_mode {
            EditorBuildAndRunMode::UserWindowed => {
                launch_user_windowed(
                    &mut report,
                    &game_exe,
                    Path::new(&desktop_report.package_dir),
                );
            }
            EditorBuildAndRunMode::HeadlessVerification => {
                launch_headless_verification(
                    &mut report,
                    &game_exe,
                    Path::new(&desktop_report.package_dir),
                    timeout_ms,
                    frame_limit,
                );
            }
        }
        report.duration.launch_duration_ms = launch_started.elapsed().as_millis();
        report.duration.total_duration_ms = total_started.elapsed().as_millis();

        let command_status = if report.status.is_success() {
            CommandStatus::Committed
        } else {
            CommandStatus::Failed
        };
        self.finish_build_and_run_transaction(transaction, report, command_status)
    }

    fn finish_build_and_run_transaction(
        &mut self,
        transaction: &mut CommandTransaction,
        mut report: EditorBuildAndRunReport,
        status: CommandStatus,
    ) -> CommandResult {
        write_editor_build_and_run_report(&mut report);
        let status_for_summary = report.status;
        let package_dir = report
            .desktop_export
            .package_dir
            .clone()
            .unwrap_or_else(|| "none".to_string());
        transaction.state_changes.push(StateChangeSummary {
            kind: "build_and_run.desktop_package".to_string(),
            path: "build_and_run.last_report".to_string(),
            before_summary: None,
            after_summary: Some(format!(
                "{:?} package={} diagnostics={}",
                status_for_summary,
                package_dir,
                report.diagnostics.len()
            )),
        });
        self.last_build_and_run_report = Some(report.clone());

        if status_for_summary.is_success() {
            let report_path = report.report_path.as_deref().unwrap_or("in-memory");
            self.push_info(
                transaction,
                "editor.build_and_run.launched",
                format!("Build And Run launched: {package_dir}. Report: {report_path}"),
            );
        } else {
            let first = report.diagnostics.first();
            let code = first
                .map(|diagnostic| diagnostic.code.as_str())
                .unwrap_or("editor.build_and_run.failed");
            let message = first
                .map(|diagnostic| diagnostic.message.clone())
                .unwrap_or_else(|| "Build And Run failed.".to_string());
            let next_action = first.and_then(|diagnostic| diagnostic.next_action.as_deref());
            self.push_error(transaction, code, message, next_action);
        }
        self.finish_transaction(transaction.clone(), status)
    }

    pub(crate) fn open_build_output(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction
            .read_set
            .push("build_export.last_report.package_dir".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let Some(report) = &self.last_desktop_export_report else {
            self.push_error(
                transaction,
                "editor.build_export.no_output",
                "Cannot open build output before exporting a package.",
                Some("Run Export first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        self.push_info(
            transaction,
            "editor.build_export.output_path",
            format!("Build output directory: {}", report.package_dir),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn open_build_report(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction
            .read_set
            .push("build_export.last_report.report_path".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let Some(report) = &self.last_desktop_export_report else {
            self.push_error(
                transaction,
                "editor.build_export.no_report",
                "Cannot open build report before exporting a package.",
                Some("Run Export first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        self.push_info(
            transaction,
            "editor.build_export.report_path",
            format!(
                "Desktop export report: {}",
                report_path_for_desktop_export(report)
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }
}

fn export_windows_dev_package(project_root: &Path) -> DesktopExportReport {
    DesktopExportPipeline::export(DesktopExportRequest::windows_dev(project_root))
}

fn launch_user_windowed(report: &mut EditorBuildAndRunReport, game_exe: &Path, package_dir: &Path) {
    let child_report_path = package_dir
        .join("reports")
        .join("windowed-player-run-report.json");
    let args = vec![
        "run-native-player".to_string(),
        "--mode".to_string(),
        "windowed".to_string(),
        "--frames".to_string(),
        "3".to_string(),
        "--report".to_string(),
        child_report_path.display().to_string(),
    ];
    report.launch.attempted = true;
    report.launch.working_dir = Some(package_dir.display().to_string());
    report.launch.executable_path = Some(game_exe.display().to_string());
    report.launch.args = args.clone();
    report.verification.child_report_path = Some(child_report_path.display().to_string());
    match ProcessCommand::new(game_exe)
        .args(&args)
        .current_dir(package_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            report.launch.started = true;
            report.launch.process_id = Some(child.id());
            report.status = EditorBuildAndRunStatus::Launched;
        }
        Err(error) => {
            report.launch.start_error = Some(error.to_string());
            report.status = EditorBuildAndRunStatus::LaunchFailed;
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.launch_spawn_failed",
                    "launch",
                    format!("Failed to spawn staged Game executable: {error}"),
                )
                .with_path(game_exe.display().to_string())
                .with_next_action("Inspect the staged Game executable and package permissions."),
            );
        }
    }
}

fn launch_headless_verification(
    report: &mut EditorBuildAndRunReport,
    game_exe: &Path,
    package_dir: &Path,
    timeout_ms: u64,
    frame_limit: u64,
) {
    let child_report_path = package_dir
        .join("reports")
        .join("windowed-player-run-report.json");
    let verification_report_path = package_dir
        .join("reports")
        .join("editor-build-and-run-process-verification-report.json");
    let args = vec![
        "run-native-player".to_string(),
        "--headless-gate".to_string(),
        "--frames".to_string(),
        frame_limit.max(1).to_string(),
        "--report".to_string(),
        child_report_path.display().to_string(),
    ];
    report.launch.attempted = true;
    report.launch.working_dir = Some(package_dir.display().to_string());
    report.launch.executable_path = Some(game_exe.display().to_string());
    report.launch.args = args.clone();
    report.verification.attempted = true;
    report.verification.status = "running".to_string();
    report.verification.verification_report_path =
        Some(verification_report_path.display().to_string());
    report.verification.child_report_path = Some(child_report_path.display().to_string());

    if let Ok(scope) = crate::ProjectWriteScope::open(package_dir) {
        let _ = scope.remove_file("reports/windowed-player-run-report.json");
    }
    let process = run_bounded_child_process(BoundedChildProcessRequest {
        executable: game_exe.to_path_buf(),
        args: args.iter().map(Into::into).collect(),
        current_dir: package_dir.to_path_buf(),
        environment: Vec::new(),
        timeout: Duration::from_millis(timeout_ms.max(1)),
        stdout_capture_limit_bytes: 64 * 1024,
        stderr_capture_limit_bytes: 64 * 1024,
        priority: runtime_cli::BoundedChildProcessPriority::Normal,
    });
    report.launch.process_id = process.process_id;
    report.launch.started = process.process_id.is_some();
    report.verification.process_exit_reason = match process.exit_reason {
        BoundedChildProcessExitReason::Completed => "completed",
        BoundedChildProcessExitReason::Failed => "failed",
        BoundedChildProcessExitReason::Cancelled => "cancelled",
        BoundedChildProcessExitReason::Timeout => "timeout",
        BoundedChildProcessExitReason::WaitFailed => "wait_failed",
        BoundedChildProcessExitReason::SpawnFailed => "spawn_failed",
    }
    .to_string();
    report.verification.process_exit_code = process.exit_code;
    report.verification.process_elapsed_ms = process.elapsed_ms;
    report.verification.stdout_summary = summarize(&process.stdout_summary, 2_000);
    report.verification.stderr_summary = summarize(&process.stderr_summary, 2_000);
    report.verification.stdout_total_bytes = process.stdout_total_bytes;
    report.verification.stderr_total_bytes = process.stderr_total_bytes;
    report.verification.stdout_truncated =
        process.stdout_truncated || process.stdout_summary.chars().count() > 2_000;
    report.verification.stderr_truncated =
        process.stderr_truncated || process.stderr_summary.chars().count() > 2_000;
    report.verification.process_kill_error = process.kill_error.clone();
    report.verification.process_wait_error = process.wait_error.clone();
    report.verification.process_reader_join_error = process.reader_join_error.clone();

    if let Some(error) = process.spawn_error {
        report.launch.start_error = Some(error.clone());
        report.status = EditorBuildAndRunStatus::LaunchFailed;
        report.diagnostics.push(
            EditorBuildAndRunDiagnostic::error(
                "editor.build_and_run.launch_spawn_failed",
                "launch",
                format!("Failed to spawn staged Game executable: {error}"),
            )
            .with_path(game_exe.display().to_string())
            .with_next_action("Inspect the staged Game executable and package permissions."),
        );
    }
    if process.exit_reason == BoundedChildProcessExitReason::Timeout {
        report.diagnostics.push(
            EditorBuildAndRunDiagnostic::error(
                "editor.build_and_run.process_timeout",
                "verification",
                format!("Staged Game executable did not exit within {timeout_ms} ms."),
            )
            .with_path(game_exe.display().to_string())
            .with_next_action("Inspect runtime startup or lower the frame workload."),
        );
    }
    for (code, error) in [
        (
            "editor.build_and_run.process_kill_failed",
            process.kill_error,
        ),
        (
            "editor.build_and_run.process_wait_failed",
            process.wait_error,
        ),
        (
            "editor.build_and_run.process_reader_join_failed",
            process.reader_join_error,
        ),
    ] {
        if let Some(error) = error {
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(code, "verification", error)
                    .with_path(game_exe.display().to_string()),
            );
        }
    }

    read_child_player_report(report, &child_report_path);
    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == EditorBuildAndRunDiagnosticSeverity::Error)
    {
        report.status = if report.status == EditorBuildAndRunStatus::LaunchFailed {
            EditorBuildAndRunStatus::LaunchFailed
        } else {
            EditorBuildAndRunStatus::VerificationFailed
        };
        report.verification.status = "failed".to_string();
    } else if report.verification.process_exit_code == Some(0)
        && report.verification.child_player_exit_code == Some(0)
    {
        report.status = EditorBuildAndRunStatus::VerificationPassed;
        report.verification.status = "passed".to_string();
    } else {
        report.status = EditorBuildAndRunStatus::VerificationFailed;
        report.verification.status = "failed".to_string();
        report.diagnostics.push(
            EditorBuildAndRunDiagnostic::error(
                "editor.build_and_run.verification_failed",
                "verification",
                format!(
                    "Staged Game executable exited with process={:?} child={:?}.",
                    report.verification.process_exit_code,
                    report.verification.child_player_exit_code
                ),
            )
            .with_path(child_report_path.display().to_string())
            .with_next_action("Open the Windowed Player report for the failed runtime layer."),
        );
    }

    write_verification_summary(&verification_report_path, report);
}

fn read_child_player_report(report: &mut EditorBuildAndRunReport, child_report_path: &Path) {
    let text = match fs::read_to_string(child_report_path) {
        Ok(text) => text,
        Err(error) => {
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.child_report_missing",
                    "verification",
                    format!("Staged Game executable did not write child report: {error}"),
                )
                .with_path(child_report_path.display().to_string())
                .with_next_action("Check runtime_cli run-native-player startup."),
            );
            return;
        }
    };
    let child_report = match serde_json::from_str::<WindowedPlayerRunReport>(&text) {
        Ok(report) => report,
        Err(error) => {
            report.diagnostics.push(
                EditorBuildAndRunDiagnostic::error(
                    "editor.build_and_run.child_report_parse_failed",
                    "verification",
                    format!("Failed to parse child WindowedPlayer report: {error}"),
                )
                .with_path(child_report_path.display().to_string()),
            );
            return;
        }
    };
    report.verification.child_player_exit_code = child_report.exit_code;
    report.verification.child_frames_completed = Some(child_report.counters.frames_completed);
    if child_report.exit_code != Some(0) {
        report.diagnostics.push(
            EditorBuildAndRunDiagnostic::error(
                "editor.build_and_run.verification_failed",
                "verification",
                format!(
                    "WindowedPlayer child report failed: {}.",
                    child_report.exit_reason
                ),
            )
            .with_path(child_report_path.display().to_string())
            .with_next_action("Open windowed-player-run-report.json for the failed layer."),
        );
    }
}

fn summarize(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_string()
    } else {
        let retained = limit.saturating_sub(3);
        format!("{}...", text.chars().take(retained).collect::<String>())
    }
}

fn report_path_for_editor_build_and_run(package_dir: &str) -> String {
    Path::new(package_dir)
        .join("reports")
        .join("editor-build-and-run-report.json")
        .display()
        .to_string()
}

fn write_editor_build_and_run_report(report: &mut EditorBuildAndRunReport) {
    let Some(path) = report.report_path.clone() else {
        return;
    };
    if let Err(error) = write_json(Path::new(&path), report) {
        report.diagnostics.push(
            EditorBuildAndRunDiagnostic::error(
                "editor.build_and_run.report_write_failed",
                "report",
                format!("Failed to write Build And Run report: {error}"),
            )
            .with_path(path),
        );
    }
}

fn write_verification_summary(path: &Path, report: &EditorBuildAndRunReport) {
    let value = serde_json::json!({
        "schemaVersion": "editor-build-and-run-process-verification-summary.v2",
        "status": report.verification.status,
        "processExitReason": report.verification.process_exit_reason,
        "processExitCode": report.verification.process_exit_code,
        "processElapsedMs": report.verification.process_elapsed_ms,
        "stdoutTotalBytes": report.verification.stdout_total_bytes,
        "stderrTotalBytes": report.verification.stderr_total_bytes,
        "stdoutTruncated": report.verification.stdout_truncated,
        "stderrTruncated": report.verification.stderr_truncated,
        "processKillError": report.verification.process_kill_error,
        "processWaitError": report.verification.process_wait_error,
        "processReaderJoinError": report.verification.process_reader_join_error,
        "childPlayerExitCode": report.verification.child_player_exit_code,
        "childFramesCompleted": report.verification.child_frames_completed,
        "childReportPath": report.verification.child_report_path,
    });
    let _ = write_json(path, &value);
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let package_root = path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| std::io::Error::other("build report path has no package root"))?;
    let relative = path
        .strip_prefix(package_root)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let scope = crate::ProjectWriteScope::open(package_root)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    scope
        .write_atomic(relative, text.as_bytes())
        .map(|_| ())
        .map_err(|error| std::io::Error::other(error.to_string()))
}
