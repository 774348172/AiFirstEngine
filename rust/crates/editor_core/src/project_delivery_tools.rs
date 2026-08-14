use crate::{
    DesktopExportPipeline, DesktopExportReport, DesktopExportRequest, EditorSession,
    ProjectRelativePath, ProjectWriteScope,
};
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationReport,
    ExportedPlayerProcessVerificationRequest,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION: &str = "project-delivery-tool-input.v1";
const DELIVERY_ROOT: &str = "Library/AiCapability/Deliveries";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectBuildExportInput {
    pub schema_version: String,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDeliveryVerifyInput {
    pub schema_version: String,
    pub package_dir: String,
    pub mode: String,
    pub timeout_ms: u64,
    pub frame_limit: u64,
    pub screenshot: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectBuildExportEvidence {
    pub package_dir: String,
    pub report: DesktopExportReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDeliveryVerifyEvidence {
    pub package_dir: String,
    pub report: ExportedPlayerProcessVerificationReport,
}

pub struct ProjectDeliveryTools;

impl ProjectDeliveryTools {
    pub fn build_export(
        session: &EditorSession,
        operation_id: &str,
        input: &ProjectBuildExportInput,
    ) -> Result<ProjectBuildExportEvidence, String> {
        validate_schema(&input.schema_version)?;
        if input.profile != "windows-dev" {
            return Err("Build export only supports the windows-dev profile.".to_string());
        }
        let project = session
            .active_project_session()
            .ok_or_else(|| "Build export requires an active project.".to_string())?;
        let relative_output =
            ProjectRelativePath::parse(format!("{DELIVERY_ROOT}/{operation_id}/Windows"))
                .map_err(|error| error.to_string())?;
        prepare_delivery_output(&project.project_root, &relative_output)?;
        let report = DesktopExportPipeline::export(
            DesktopExportRequest::windows_dev(&project.project_root)
                .with_project_relative_output(relative_output),
        );
        let package_dir = project_relative_package_dir(&project.project_root, &report.package_dir)?;
        if report.status == crate::DesktopExportStatus::Success {
            let _ = resolve_delivery_package(&project.project_root, &package_dir)?;
        }
        Ok(ProjectBuildExportEvidence {
            package_dir,
            report,
        })
    }

    pub fn verify_delivery(
        session: &EditorSession,
        input: &ProjectDeliveryVerifyInput,
    ) -> Result<ProjectDeliveryVerifyEvidence, String> {
        validate_schema(&input.schema_version)?;
        if !matches!(input.mode.as_str(), "headless" | "windowed") {
            return Err("Delivery verification mode must be headless or windowed.".to_string());
        }
        if !(1..=120_000).contains(&input.timeout_ms) || !(1..=600).contains(&input.frame_limit) {
            return Err(
                "Delivery verification requires timeoutMs 1-120000 and frameLimit 1-600."
                    .to_string(),
            );
        }
        let project = session
            .active_project_session()
            .ok_or_else(|| "Delivery verification requires an active project.".to_string())?;
        let relative =
            ProjectRelativePath::parse(&input.package_dir).map_err(|error| error.to_string())?;
        if !relative
            .as_str()
            .replace('\\', "/")
            .starts_with(&format!("{DELIVERY_ROOT}/"))
        {
            return Err("Delivery package must come from the Gateway delivery root.".to_string());
        }
        let canonical_package = resolve_delivery_package(&project.project_root, relative.as_str())?;
        let screenshot_path = input
            .screenshot
            .then(|| canonical_package.join("reports/gateway-delivery.png"));
        let report = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
            exported_package_dir: canonical_package,
            mode: input.mode.clone(),
            frame_limit: input.frame_limit,
            report_path: None,
            timeout_ms: input.timeout_ms,
            screenshot: input.screenshot,
            screenshot_path,
        });
        Ok(ProjectDeliveryVerifyEvidence {
            package_dir: relative.as_str().to_string(),
            report,
        })
    }
}

fn validate_schema(schema_version: &str) -> Result<(), String> {
    (schema_version == PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION)
        .then_some(())
        .ok_or_else(|| "Project delivery tool input schema is unsupported.".to_string())
}

fn project_relative_package_dir(
    project_root: &std::path::Path,
    package_dir: &str,
) -> Result<String, String> {
    PathBuf::from(package_dir)
        .strip_prefix(project_root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|_| "Build export package escaped the active project.".to_string())
}

fn prepare_delivery_output(
    project_root: &std::path::Path,
    relative_output: &ProjectRelativePath,
) -> Result<PathBuf, String> {
    let scope = ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
    scope
        .ensure_directory(relative_output.as_path())
        .map_err(|error| format!("Gateway delivery root rejected the build output: {error}"))?;
    let output_root = project_root.join(relative_output.as_path());
    let _ = canonical_delivery_member(project_root, &output_root, "build output")?;
    Ok(output_root)
}

fn resolve_delivery_package(
    project_root: &std::path::Path,
    relative_package: &str,
) -> Result<PathBuf, String> {
    let relative =
        ProjectRelativePath::parse(relative_package).map_err(|error| error.to_string())?;
    canonical_delivery_member(
        project_root,
        &project_root.join(relative.as_path()),
        "package",
    )
}

fn canonical_delivery_member(
    project_root: &std::path::Path,
    member: &std::path::Path,
    member_label: &str,
) -> Result<PathBuf, String> {
    let canonical_project_root = project_root
        .canonicalize()
        .map_err(|error| format!("Project root canonicalization failed: {error}"))?;
    let canonical_delivery_root = project_root
        .join(DELIVERY_ROOT)
        .canonicalize()
        .map_err(|error| format!("Gateway delivery root canonicalization failed: {error}"))?;
    if !canonical_delivery_root.starts_with(&canonical_project_root) {
        return Err("Gateway delivery root resolves outside the active project.".to_string());
    }
    let canonical_member = member
        .canonicalize()
        .map_err(|error| format!("Delivery {member_label} canonicalization failed: {error}"))?;
    if !canonical_member.starts_with(&canonical_delivery_root) {
        return Err(format!(
            "Delivery {member_label} resolves outside the Gateway delivery root."
        ));
    }
    Ok(canonical_member)
}
