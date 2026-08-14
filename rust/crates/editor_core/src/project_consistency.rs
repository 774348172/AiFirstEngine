#[cfg(test)]
use crate::BUILD_PROFILE_SCHEMA_VERSION_V1;
use crate::{
    BuildProfile, ProjectManifest, BUILD_PROFILE_SCHEMA_VERSION, PROJECT_MANIFEST_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::{CanonicalDigestError, ConsistencyDigest};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const BUILD_RECIPE_DIGEST_SCHEMA_VERSION: &str = "build-recipe-digest.v1";
pub const SAVE_RELOAD_REBUILD_CHECKPOINT_SCHEMA_VERSION: &str = "save-reload-rebuild-checkpoint.v1";
pub const SAVE_RELOAD_REBUILD_CONSISTENCY_REPORT_SCHEMA_VERSION: &str =
    "save-reload-rebuild-consistency-report.v1";
pub const SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH: &str =
    ".aife/reports/save-reload-rebuild/latest.json";

#[derive(Debug, Clone)]
pub struct BuildRecipeDigestInput<'a> {
    pub project: &'a ProjectManifest,
    pub build_profile: Option<&'a BuildProfile>,
    pub active_scene_id: &'a str,
    pub runtime_package_schema_version: &'a str,
    pub component_schema_cooker_version: &'a str,
    pub aui_document_cooker_version: &'a str,
    pub aui_font_atlas_cooker_version: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildRecipeDigest(pub ConsistencyDigest);

impl BuildRecipeDigest {
    pub fn calculate(input: &BuildRecipeDigestInput<'_>) -> Result<Self, CanonicalDigestError> {
        let payload = BuildRecipeDigestPayload {
            schema_version: BUILD_RECIPE_DIGEST_SCHEMA_VERSION,
            project: BuildRecipeProjectView {
                schema_version: &input.project.schema_version,
                project_id: &input.project.project_id,
                project_name: &input.project.project_name,
                engine_version: &input.project.engine_version,
                default_scene: &input.project.default_scene,
                asset_root: &input.project.asset_root,
                settings_version: &input.project.settings_version,
            },
            build_profile: input.build_profile,
            active_scene_id: input.active_scene_id,
            schema_and_cooker_versions: BuildRecipeVersions {
                project_manifest: PROJECT_MANIFEST_SCHEMA_VERSION,
                build_profile: input
                    .build_profile
                    .map(|profile| profile.schema_version.as_str())
                    .unwrap_or(BUILD_PROFILE_SCHEMA_VERSION),
                runtime_package: input.runtime_package_schema_version,
                component_schema_cooker: input.component_schema_cooker_version,
                aui_document_cooker: input.aui_document_cooker_version,
                aui_font_atlas_cooker: input.aui_font_atlas_cooker_version,
            },
        };
        Ok(Self(ConsistencyDigest::sha256(
            "build-recipe",
            BUILD_RECIPE_DIGEST_SCHEMA_VERSION,
            &payload,
        )?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveReloadRebuildStatus {
    NotRun,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyReportLevel {
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyDomainDigest {
    pub domain: String,
    pub semantic_digest: String,
    pub stable_ids: Vec<String>,
    pub source_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRuntimeWitness {
    pub domain: String,
    pub source_path: String,
    pub object_id: String,
    pub field_path: Option<String>,
    pub build_input_path: String,
    pub runtime_path: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReloadRebuildCheckpoint {
    pub schema_version: String,
    pub project_id: String,
    pub stage: String,
    pub invocation_id: String,
    pub parent_token_hash: String,
    pub process_id: u32,
    pub reopen_mode: String,
    pub domains: Vec<ConsistencyDomainDigest>,
    pub source_runtime_witnesses: Vec<SourceRuntimeWitness>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyComparison {
    pub comparison_id: String,
    pub left: String,
    pub right: String,
    pub equal: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyMutationEvidence {
    pub mutation_id: String,
    pub expected_effect: String,
    pub observed: bool,
    pub diagnostic_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyProcessEvidence {
    pub mode: String,
    pub invocation_id: String,
    pub executable: String,
    pub process_id: u32,
    pub exit_code: Option<i32>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReloadRebuildDiagnostic {
    pub code: String,
    pub message: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub object_id: Option<String>,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReloadRebuildConsistencyReport {
    pub schema_version: String,
    pub project_id: String,
    pub status: SaveReloadRebuildStatus,
    pub report_level: ConsistencyReportLevel,
    pub reopen_mode: String,
    pub checkpoints: Vec<SaveReloadRebuildCheckpoint>,
    pub processes: Vec<ConsistencyProcessEvidence>,
    pub comparisons: Vec<ConsistencyComparison>,
    pub mutations: Vec<ConsistencyMutationEvidence>,
    pub source_runtime_witnesses: Vec<SourceRuntimeWitness>,
    pub artifacts: Vec<String>,
    pub diagnostics: Vec<SaveReloadRebuildDiagnostic>,
    pub next_actions: Vec<String>,
}

impl SaveReloadRebuildConsistencyReport {
    pub fn new(project_id: impl Into<String>, report_level: ConsistencyReportLevel) -> Self {
        Self {
            schema_version: SAVE_RELOAD_REBUILD_CONSISTENCY_REPORT_SCHEMA_VERSION.to_string(),
            project_id: project_id.into(),
            status: SaveReloadRebuildStatus::NotRun,
            report_level,
            reopen_mode: "process_isolated".to_string(),
            checkpoints: Vec::new(),
            processes: Vec::new(),
            comparisons: Vec::new(),
            mutations: Vec::new(),
            source_runtime_witnesses: Vec::new(),
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    pub fn recompute_status(&mut self) {
        self.status = if self.diagnostics.is_empty()
            && self.comparisons.iter().all(|comparison| comparison.equal)
            && self.mutations.iter().all(|mutation| mutation.observed)
            && self
                .source_runtime_witnesses
                .iter()
                .all(|witness| witness.resolved)
        {
            SaveReloadRebuildStatus::Passed
        } else {
            SaveReloadRebuildStatus::Failed
        };
    }
}

pub fn write_consistency_report_atomic(
    path: &Path,
    report: &SaveReloadRebuildConsistencyReport,
) -> Result<(), String> {
    let project_root = path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| "consistency report path has no project root".to_string())?;
    let scope = crate::ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
    write_consistency_report_in_scope(&scope, SAVE_RELOAD_REBUILD_REPORT_RELATIVE_PATH, report)
}

pub fn write_consistency_report_in_scope(
    scope: &crate::ProjectWriteScope,
    relative_path: &str,
    report: &SaveReloadRebuildConsistencyReport,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("failed to serialize consistency report: {error}"))?;
    scope
        .write_atomic(relative_path, &bytes)
        .map(|_| ())
        .map_err(|error| format!("failed to atomically write consistency report: {error}"))
}

pub fn write_consistency_report_external_atomic(
    output: &crate::ExplicitExportOutput,
    path: &Path,
    report: &SaveReloadRebuildConsistencyReport,
) -> Result<(), String> {
    if !output.authorizes(path) {
        return Err("consistency report path is outside ExplicitExportOutput".to_string());
    }
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("failed to serialize consistency report: {error}"))?;
    engine_runtime::atomic_file_replace::atomic_file_replace(path, &bytes)
        .map_err(|error| format!("failed to write external consistency report: {error}"))
}

pub fn read_consistency_report(path: &Path) -> Result<SaveReloadRebuildConsistencyReport, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read consistency report: {error}"))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse consistency report: {error}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildRecipeDigestPayload<'a> {
    schema_version: &'a str,
    project: BuildRecipeProjectView<'a>,
    build_profile: Option<&'a BuildProfile>,
    active_scene_id: &'a str,
    schema_and_cooker_versions: BuildRecipeVersions<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildRecipeProjectView<'a> {
    schema_version: &'a str,
    project_id: &'a str,
    project_name: &'a str,
    engine_version: &'a str,
    default_scene: &'a str,
    asset_root: &'a str,
    settings_version: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildRecipeVersions<'a> {
    project_manifest: &'a str,
    build_profile: &'a str,
    runtime_package: &'a str,
    component_schema_cooker: &'a str,
    aui_document_cooker: &'a str,
    aui_font_atlas_cooker: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_consistency_digest_excludes_manifest_timestamps_but_tracks_build_recipe_fields() {
        let project = ProjectManifest {
            schema_version: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
            project_id: "project-main".to_string(),
            project_name: "Main Project".to_string(),
            engine_version: "0.0.1".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_opened_at: Some("2026-01-02T00:00:00Z".to_string()),
            default_scene: "Scenes/Main.scene.json".to_string(),
            asset_root: "Assets".to_string(),
            settings_version: "project-settings.v1".to_string(),
            runtime_module: crate::ProjectRuntimeModuleBuildSpec::explicit_empty(),
            observation_contract: None,
        };
        let profile = BuildProfile {
            schema_version: BUILD_PROFILE_SCHEMA_VERSION_V1.to_string(),
            profile: "windows-dev".to_string(),
            target: "windows".to_string(),
            runtime_package_mode: "dev-run".to_string(),
            frame_limit: 120,
            headless_surface_gate: true,
            real_window_smoke: "optional".to_string(),
            architecture: None,
            application: None,
            release: None,
        };
        let digest = |project: &ProjectManifest, profile: &BuildProfile| {
            BuildRecipeDigest::calculate(&BuildRecipeDigestInput {
                project,
                build_profile: Some(profile),
                active_scene_id: "scene-main",
                runtime_package_schema_version: "runtime-package.v2",
                component_schema_cooker_version: "component-schema.v1",
                aui_document_cooker_version: "aui-document-cook.v1",
                aui_font_atlas_cooker_version: "aui-font-atlas-cook.v1",
            })
            .unwrap()
        };

        let initial = digest(&project, &profile);
        let mut timestamp_only = project.clone();
        timestamp_only.created_at = "2030-01-01T00:00:00Z".to_string();
        timestamp_only.last_opened_at = None;
        assert_eq!(initial, digest(&timestamp_only, &profile));

        let mut recipe_change = profile.clone();
        recipe_change.frame_limit = 30;
        assert_ne!(initial, digest(&project, &recipe_change));
        recipe_change = profile.clone();
        recipe_change.headless_surface_gate = false;
        assert_ne!(initial, digest(&project, &recipe_change));
    }
}
