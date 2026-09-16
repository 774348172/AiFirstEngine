use crate::ProjectRelativePath;
use serde::{Deserialize, Serialize};

pub const LEGACY_PROJECT_MANIFEST_SCHEMA_VERSION: &str = "aife-project.v1";
pub const PROJECT_MANIFEST_SCHEMA_VERSION: &str = "aife-project.v2";
pub const PROJECT_RUNTIME_MODULE_INTERFACE_VERSION: &str = "project-runtime-module.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "engineVersion")]
    pub engine_version: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "lastOpenedAt")]
    pub last_opened_at: Option<String>,
    #[serde(rename = "defaultScene")]
    pub default_scene: String,
    #[serde(rename = "assetRoot")]
    pub asset_root: String,
    #[serde(rename = "settingsVersion")]
    pub settings_version: String,
    #[serde(rename = "runtimeModule")]
    pub runtime_module: ProjectRuntimeModuleBuildSpec,
    #[serde(
        rename = "observationContract",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub observation_contract: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeModuleBuildSpec {
    #[serde(
        rename = "sourceKind",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub source_kind: Option<ProjectRuntimeSourceKind>,
    pub module_id: String,
    pub interface_version: String,
    pub cargo_manifest: String,
    pub cargo_package: String,
    pub player_binary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project_game_sdk: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRuntimeSourceKind {
    BuiltInEmpty,
    ProjectRust,
}

impl ProjectRuntimeModuleBuildSpec {
    pub fn explicit_empty() -> Self {
        Self {
            source_kind: Some(ProjectRuntimeSourceKind::BuiltInEmpty),
            module_id: "engine.empty.runtime".to_string(),
            interface_version: PROJECT_RUNTIME_MODULE_INTERFACE_VERSION.to_string(),
            cargo_manifest: "RuntimeModule/Cargo.toml".to_string(),
            cargo_package: "empty_project_runtime".to_string(),
            player_binary: "empty_project_player".to_string(),
            project_game_sdk: String::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.module_id.trim().is_empty()
            || self.interface_version.trim().is_empty()
            || self.cargo_package.trim().is_empty()
            || self.player_binary.trim().is_empty()
        {
            return Err(
                "project_runtime.project_manifest_runtime_module_fields_required".to_string(),
            );
        }
        match self.resolved_source_kind() {
            ProjectRuntimeSourceKind::BuiltInEmpty => {
                if self.module_id != "engine.empty.runtime" {
                    return Err("project_runtime.builtin_empty_module_id_mismatch".to_string());
                }
            }
            ProjectRuntimeSourceKind::ProjectRust => {
                if self.module_id == "engine.empty.runtime" {
                    return Err(
                        "project_runtime.project_rust_cannot_use_empty_module_id".to_string()
                    );
                }
                ProjectRelativePath::parse(&self.cargo_manifest).map_err(|error| {
                    format!("project_runtime.invalid_cargo_manifest_path: {error}")
                })?;
                if self.project_game_sdk != project_game_sdk::PROJECT_GAME_SDK_CONTRACT_ID {
                    return Err("project_runtime.project_game_sdk_contract_unsupported".to_string());
                }
            }
        }
        Ok(())
    }

    pub fn resolved_source_kind(&self) -> ProjectRuntimeSourceKind {
        self.source_kind.unwrap_or_else(|| {
            if self.module_id == "engine.empty.runtime" {
                ProjectRuntimeSourceKind::BuiltInEmpty
            } else {
                ProjectRuntimeSourceKind::ProjectRust
            }
        })
    }
}
