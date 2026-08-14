use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildExportModel {
    pub selected_profile_id: Option<String>,
    pub profiles: Vec<BuildProfileSummary>,
    pub release_profile: Option<ReleaseBuildProfileModel>,
    pub commands: Vec<BuildExportCommand>,
    pub last_report: Option<BuildExportReportSummary>,
    pub last_release_report: Option<ReleasePackageReportSummary>,
    pub empty_message: String,
}

impl BuildExportModel {
    pub fn empty() -> Self {
        Self {
            selected_profile_id: None,
            profiles: Vec::new(),
            release_profile: None,
            commands: Vec::new(),
            last_report: None,
            last_release_report: None,
            empty_message: "Open a project to export a desktop build.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBuildProfileModel {
    pub profile_id: String,
    pub display_name: String,
    pub executable_name: String,
    pub company_name: String,
    pub file_description: String,
    pub display_version: String,
    pub architecture: String,
    pub icon_asset_id: String,
    pub output_preview: String,
    pub dirty: bool,
    pub validation_diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildProfileSummary {
    pub profile_id: String,
    pub label: String,
    pub target: String,
    pub output_dir: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildExportCommand {
    pub command_id: String,
    pub label: String,
    pub enabled: bool,
    pub reason_disabled: Option<String>,
}

impl BuildExportCommand {
    pub fn new(
        command_id: impl Into<String>,
        label: impl Into<String>,
        enabled: bool,
        reason_disabled: Option<String>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            label: label.into(),
            enabled,
            reason_disabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildExportReportSummary {
    pub status: String,
    pub profile: String,
    pub target: String,
    pub package_dir: String,
    pub report_path: String,
    pub runtime_package_dir: String,
    pub player_exit_code: Option<i32>,
    pub player_exit_reason: String,
    pub diagnostic_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePackageReportSummary {
    pub status: String,
    pub product_name: String,
    pub display_version: String,
    pub entrypoint: String,
    pub release_payload_hash: String,
    pub diagnostic_count: usize,
    pub next_action: String,
}
