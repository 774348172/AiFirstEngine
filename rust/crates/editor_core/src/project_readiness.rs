use crate::{
    ProjectManifest, ProjectRuntimeSourceKind, ProjectValidationStatus,
    PROJECT_MANIFEST_SCHEMA_VERSION,
};
use engine_input::InputMappingAsset;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_READINESS_REPORT_SCHEMA_VERSION: &str = "project-readiness-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectReadinessStatus {
    Ready,
    Incomplete,
    Invalid,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectReadinessCheckStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReadinessCheck {
    pub code: String,
    pub status: ProjectReadinessCheckStatus,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReadinessReport {
    pub schema_version: String,
    pub status: ProjectReadinessStatus,
    pub project_root: String,
    pub project_kind: Option<ProjectRuntimeSourceKind>,
    pub checks: Vec<ProjectReadinessCheck>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl ProjectReadinessReport {
    pub fn launcher_status(&self) -> ProjectValidationStatus {
        match self.status {
            ProjectReadinessStatus::Ready => ProjectValidationStatus::Ready,
            ProjectReadinessStatus::Incomplete => ProjectValidationStatus::Incomplete,
            ProjectReadinessStatus::Invalid => {
                if self
                    .diagnostics
                    .iter()
                    .any(|code| code == "project_readiness.manifest_missing")
                {
                    ProjectValidationStatus::Missing
                } else {
                    ProjectValidationStatus::InvalidManifest
                }
            }
            ProjectReadinessStatus::Unsupported => ProjectValidationStatus::UnsupportedVersion,
        }
    }
}

pub struct ProjectReadiness;

impl ProjectReadiness {
    pub fn inspect(project_root: &Path, current_engine_version: &str) -> ProjectReadinessReport {
        let mut builder = ReadinessBuilder::new(project_root);
        let manifest_path = project_root.join("project.aife.json");
        if !manifest_path.is_file() {
            builder.invalid(
                "project_readiness.manifest_missing",
                Some(&manifest_path),
                "Project manifest is missing.",
                "Create a project or select a directory containing project.aife.json.",
            );
            return builder.finish(None);
        }

        let manifest_text = match fs::read_to_string(&manifest_path) {
            Ok(text) => text,
            Err(error) => {
                builder.invalid(
                    "project_readiness.manifest_unreadable",
                    Some(&manifest_path),
                    format!("Project manifest cannot be read: {error}"),
                    "Restore a readable project manifest.",
                );
                return builder.finish(None);
            }
        };
        let manifest = match serde_json::from_str::<ProjectManifest>(&manifest_text) {
            Ok(manifest) => manifest,
            Err(error) => {
                builder.invalid(
                    "project_readiness.manifest_invalid",
                    Some(&manifest_path),
                    format!("Project manifest is invalid: {error}"),
                    "Repair or restore project.aife.json.",
                );
                return builder.finish(None);
            }
        };
        builder.pass(
            "project_readiness.manifest_valid",
            Some(&manifest_path),
            "Project manifest parsed successfully.",
        );

        if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
            builder.invalid(
                "project_readiness.manifest_schema_unsupported",
                Some(&manifest_path),
                format!("Unsupported project schema: {}", manifest.schema_version),
                "Migrate the project manifest before opening it.",
            );
        }
        if manifest.engine_version != current_engine_version {
            builder.unsupported(
                "project_readiness.engine_version_unsupported",
                Some(&manifest_path),
                format!(
                    "Project engine version {} does not match {}.",
                    manifest.engine_version, current_engine_version
                ),
                "Open the project with a compatible engine or run an explicit migration.",
            );
        }
        if let Err(message) = manifest.runtime_module.validate() {
            builder.invalid(
                "project_readiness.runtime_source_contract_invalid",
                Some(&manifest_path),
                message,
                "Repair the runtimeModule source contract.",
            );
        }

        for directory in ["Assets", "Scenes", "Settings", "Input"] {
            let path = project_root.join(directory);
            if path.is_dir() {
                builder.pass(
                    format!(
                        "project_readiness.directory_{}_present",
                        directory.to_lowercase()
                    ),
                    Some(&path),
                    format!("Required project directory {directory} is present."),
                );
            } else {
                builder.incomplete(
                    format!(
                        "project_readiness.directory_{}_missing",
                        directory.to_lowercase()
                    ),
                    Some(&path),
                    format!("Required project directory {directory} is missing."),
                    format!("Restore the {directory} project directory."),
                );
            }
        }

        inspect_scene(&mut builder, project_root, &manifest);
        inspect_input(&mut builder, project_root);
        inspect_runtime_source(&mut builder, project_root, &manifest);
        builder.finish(Some(manifest.runtime_module.resolved_source_kind()))
    }
}

fn inspect_scene(builder: &mut ReadinessBuilder, root: &Path, manifest: &ProjectManifest) {
    let scene_path = root.join(&manifest.default_scene);
    let Ok(text) = fs::read_to_string(&scene_path) else {
        builder.incomplete(
            "project_readiness.default_scene_missing",
            Some(&scene_path),
            "Default Scene is missing or unreadable.",
            "Restore the default Scene referenced by project.aife.json.",
        );
        return;
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value)
            if value
                .get("schemaVersion")
                .and_then(serde_json::Value::as_str)
                == Some("editor-scene-document.v1")
                && value
                    .get("entities")
                    .is_some_and(serde_json::Value::is_array) =>
        {
            builder.pass(
                "project_readiness.default_scene_valid",
                Some(&scene_path),
                "Default Scene is present and parseable.",
            );
        }
        Ok(_) | Err(_) => builder.invalid(
            "project_readiness.default_scene_invalid",
            Some(&scene_path),
            "Default Scene is not a valid editor-scene-document.v1 document.",
            "Repair or restore the default Scene.",
        ),
    }
}

fn inspect_input(builder: &mut ReadinessBuilder, root: &Path) {
    let input_root = root.join("Input");
    let candidates = fs::read_dir(&input_root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        builder.incomplete(
            "project_readiness.input_mapping_missing",
            Some(&input_root),
            "No Input mapping asset exists.",
            "Create or restore an Input mapping asset.",
        );
        return;
    }
    if candidates.iter().any(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<InputMappingAsset>(&text).ok())
            .is_some()
    }) {
        builder.pass(
            "project_readiness.input_mapping_valid",
            Some(&input_root),
            "At least one Input mapping asset is parseable.",
        );
    } else {
        builder.invalid(
            "project_readiness.input_mapping_invalid",
            Some(&input_root),
            "Input mapping assets exist but none can be parsed.",
            "Repair or recreate the Input mapping asset.",
        );
    }
}

fn inspect_runtime_source(builder: &mut ReadinessBuilder, root: &Path, manifest: &ProjectManifest) {
    match manifest.runtime_module.resolved_source_kind() {
        ProjectRuntimeSourceKind::BuiltInEmpty => builder.pass(
            "project_readiness.runtime_builtin_empty_ready",
            None,
            "Built-in empty runtime does not require project-owned Rust source.",
        ),
        ProjectRuntimeSourceKind::ProjectRust => {
            let cargo_manifest = root.join(&manifest.runtime_module.cargo_manifest);
            if !cargo_manifest.is_file() {
                builder.incomplete(
                    "project_readiness.runtime_cargo_manifest_missing",
                    Some(&cargo_manifest),
                    "ProjectRust Cargo manifest is missing.",
                    "Restore the project RuntimeModule Cargo manifest.",
                );
                return;
            }
            builder.pass(
                "project_readiness.runtime_cargo_manifest_present",
                Some(&cargo_manifest),
                "ProjectRust Cargo manifest is present.",
            );
            let lib_rs = cargo_manifest
                .parent()
                .unwrap_or(root)
                .join("src")
                .join("lib.rs");
            if lib_rs.is_file() {
                builder.pass(
                    "project_readiness.runtime_lib_present",
                    Some(&lib_rs),
                    "ProjectRust library source is present.",
                );
            } else {
                builder.incomplete(
                    "project_readiness.runtime_lib_missing",
                    Some(&lib_rs),
                    "ProjectRust src/lib.rs is missing.",
                    "Restore the project RuntimeModule library source.",
                );
            }
        }
    }
}

struct ReadinessBuilder {
    project_root: PathBuf,
    status: ProjectReadinessStatus,
    checks: Vec<ProjectReadinessCheck>,
    diagnostics: Vec<String>,
    next_actions: Vec<String>,
}

impl ReadinessBuilder {
    fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            status: ProjectReadinessStatus::Ready,
            checks: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    fn pass(&mut self, code: impl Into<String>, path: Option<&Path>, message: impl Into<String>) {
        self.checks.push(ProjectReadinessCheck {
            code: code.into(),
            status: ProjectReadinessCheckStatus::Passed,
            path: path.map(|value| value.display().to_string()),
            message: message.into(),
        });
    }

    fn incomplete(
        &mut self,
        code: impl Into<String>,
        path: Option<&Path>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) {
        if self.status == ProjectReadinessStatus::Ready {
            self.status = ProjectReadinessStatus::Incomplete;
        }
        self.fail(code, path, message, next_action);
    }

    fn invalid(
        &mut self,
        code: impl Into<String>,
        path: Option<&Path>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) {
        self.status = ProjectReadinessStatus::Invalid;
        self.fail(code, path, message, next_action);
    }

    fn unsupported(
        &mut self,
        code: impl Into<String>,
        path: Option<&Path>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) {
        if self.status != ProjectReadinessStatus::Invalid {
            self.status = ProjectReadinessStatus::Unsupported;
        }
        self.fail(code, path, message, next_action);
    }

    fn fail(
        &mut self,
        code: impl Into<String>,
        path: Option<&Path>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) {
        let code = code.into();
        self.diagnostics.push(code.clone());
        self.next_actions.push(next_action.into());
        self.checks.push(ProjectReadinessCheck {
            code,
            status: ProjectReadinessCheckStatus::Failed,
            path: path.map(|value| value.display().to_string()),
            message: message.into(),
        });
    }

    fn finish(self, project_kind: Option<ProjectRuntimeSourceKind>) -> ProjectReadinessReport {
        ProjectReadinessReport {
            schema_version: PROJECT_READINESS_REPORT_SCHEMA_VERSION.to_string(),
            status: self.status,
            project_root: self.project_root.display().to_string(),
            project_kind,
            checks: self.checks,
            diagnostics: self.diagnostics,
            next_actions: self.next_actions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectLauncherState;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn newly_created_empty_project_is_ready_without_runtime_source() {
        let root = unique_temp_dir("empty-ready");
        ProjectLauncherState::new("0.0.3")
            .create_project(&root, "Ready")
            .unwrap();

        let report = ProjectReadiness::inspect(&root, "0.0.3");

        assert_eq!(report.status, ProjectReadinessStatus::Ready);
        assert_eq!(
            report.project_kind,
            Some(ProjectRuntimeSourceKind::BuiltInEmpty)
        );
        assert!(!root.join("RuntimeModule/Cargo.toml").exists());
        assert!(report.checks.iter().any(|check| {
            check.code == "project_readiness.runtime_builtin_empty_ready"
                && check.status == ProjectReadinessCheckStatus::Passed
        }));
    }

    #[test]
    fn project_rust_requires_manifest_and_library_source() {
        let root = unique_temp_dir("project-rust-incomplete");
        let mut session = ProjectLauncherState::new("0.0.3")
            .create_project(&root, "RustProject")
            .unwrap();
        session.manifest.runtime_module.source_kind = Some(ProjectRuntimeSourceKind::ProjectRust);
        session.manifest.runtime_module.module_id = "project.rust.runtime".to_string();
        fs::write(
            root.join("project.aife.json"),
            serde_json::to_string_pretty(&session.manifest).unwrap(),
        )
        .unwrap();

        let report = ProjectReadiness::inspect(&root, "0.0.3");

        assert_eq!(report.status, ProjectReadinessStatus::Incomplete);
        assert!(report
            .diagnostics
            .contains(&"project_readiness.runtime_cargo_manifest_missing".to_string()));
    }

    #[test]
    fn legacy_empty_manifest_without_source_kind_remains_ready() {
        let root = unique_temp_dir("legacy-empty-ready");
        ProjectLauncherState::new("0.0.3")
            .create_project(&root, "Legacy")
            .unwrap();
        let path = root.join("project.aife.json");
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        value["runtimeModule"]
            .as_object_mut()
            .unwrap()
            .remove("sourceKind");
        fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let report = ProjectReadiness::inspect(&root, "0.0.3");

        assert_eq!(report.status, ProjectReadinessStatus::Ready);
        assert_eq!(
            report.project_kind,
            Some(ProjectRuntimeSourceKind::BuiltInEmpty)
        );
    }

    #[test]
    fn missing_input_is_incomplete() {
        let root = unique_temp_dir("missing-input");
        ProjectLauncherState::new("0.0.3")
            .create_project(&root, "MissingInput")
            .unwrap();
        fs::remove_file(root.join("Input/input.none.json")).unwrap();

        let report = ProjectReadiness::inspect(&root, "0.0.3");

        assert_eq!(report.status, ProjectReadinessStatus::Incomplete);
        assert_eq!(
            report.launcher_status(),
            ProjectValidationStatus::Incomplete
        );
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("aife-{label}-{stamp}"))
    }
}
