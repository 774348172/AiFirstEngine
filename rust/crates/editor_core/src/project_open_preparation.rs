use crate::{
    CandidateProjectRevisionStore, ProjectCandidateProjectBinding, ProjectManifest,
    PROJECT_CANDIDATE_PROJECT_BINDING_SCHEMA_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_OPEN_PREPARATION_SCHEMA_VERSION: &str = "project-open-preparation.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpenPreparationPhase {
    ReadingProject,
    ComputingDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedProjectOpen {
    pub schema_version: String,
    pub project_root: PathBuf,
    pub project_id: String,
    pub manifest_digest: String,
    pub binding: ProjectCandidateProjectBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectOpenPreparationError {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub next_action: String,
}

impl std::fmt::Display for ProjectOpenPreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectOpenPreparationError {}

pub struct ProjectOpenPreparation;

impl ProjectOpenPreparation {
    pub fn prepare(
        project_root: &Path,
        progress: impl FnMut(ProjectOpenPreparationPhase),
    ) -> Result<PreparedProjectOpen, ProjectOpenPreparationError> {
        Self::prepare_cancellable(project_root, progress, || false)
    }

    pub fn prepare_cancellable(
        project_root: &Path,
        mut progress: impl FnMut(ProjectOpenPreparationPhase),
        is_cancelled: impl Fn() -> bool,
    ) -> Result<PreparedProjectOpen, ProjectOpenPreparationError> {
        progress(ProjectOpenPreparationPhase::ReadingProject);
        let project_root = project_root.canonicalize().map_err(|error| {
            preparation_error(
                "project_open.root_unavailable",
                format!("Project root cannot be canonicalized: {error}"),
                Some(project_root),
                "Choose an existing project directory and retry.",
            )
        })?;
        if !project_root.is_dir() {
            return Err(preparation_error(
                "project_open.root_not_directory",
                "Project root is not a directory.",
                Some(&project_root),
                "Choose a project directory and retry.",
            ));
        }
        let manifest_path = project_root.join("project.aife.json");
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
            preparation_error(
                "project_open.manifest_unreadable",
                format!("Project manifest cannot be read: {error}"),
                Some(&manifest_path),
                "Restore a readable project.aife.json and retry.",
            )
        })?;
        let manifest =
            serde_json::from_slice::<ProjectManifest>(&manifest_bytes).map_err(|error| {
                preparation_error(
                    "project_open.manifest_invalid",
                    format!("Project manifest cannot be parsed: {error}"),
                    Some(&manifest_path),
                    "Repair project.aife.json and retry.",
                )
            })?;
        let manifest_digest = sha256_prefixed(&manifest_bytes);

        progress(ProjectOpenPreparationPhase::ComputingDigest);
        let project_digest =
            CandidateProjectRevisionStore::project_digest_cancellable(&project_root, is_cancelled)
                .map_err(|error| {
                    preparation_error(
                        &error.code,
                        error.message,
                        error.path.as_deref().map(Path::new),
                        error.next_action,
                    )
                })?;
        let canonical_root = project_root.display().to_string();
        Ok(PreparedProjectOpen {
            schema_version: PROJECT_OPEN_PREPARATION_SCHEMA_VERSION.to_string(),
            project_root,
            project_id: manifest.project_id.clone(),
            manifest_digest,
            binding: ProjectCandidateProjectBinding {
                schema_version: PROJECT_CANDIDATE_PROJECT_BINDING_SCHEMA_VERSION.to_string(),
                project_id: manifest.project_id,
                project_root: canonical_root,
                project_digest,
            },
        })
    }
}

fn preparation_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: Option<&Path>,
    next_action: impl Into<String>,
) -> ProjectOpenPreparationError {
    ProjectOpenPreparationError {
        code: code.into(),
        message: message.into(),
        path: path.map(|value| value.display().to_string()),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn preparation_reports_phases_and_excludes_nested_cargo_target() {
        let root = fixture("phases");
        let cargo_root = root.join("Tests/Harness");
        fs::create_dir_all(cargo_root.join("target/debug")).unwrap();
        fs::write(cargo_root.join("Cargo.toml"), b"[workspace]\n").unwrap();
        fs::write(cargo_root.join("target/debug/generated.bin"), vec![7; 4096]).unwrap();
        let mut phases = Vec::new();
        let first = ProjectOpenPreparation::prepare(&root, |phase| phases.push(phase)).unwrap();
        fs::write(cargo_root.join("target/debug/generated.bin"), vec![8; 4096]).unwrap();
        let second = ProjectOpenPreparation::prepare(&root, |_| {}).unwrap();
        assert_eq!(
            phases,
            vec![
                ProjectOpenPreparationPhase::ReadingProject,
                ProjectOpenPreparationPhase::ComputingDigest
            ]
        );
        assert_eq!(first.binding.project_digest, second.binding.project_digest);
    }

    fn fixture(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("aife-project-open-{label}-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("project.aife.json"),
            br#"{
              "schemaVersion":"aife-project.v2",
              "projectId":"project-open-fixture",
              "projectName":"Fixture",
              "engineVersion":"0.0.1",
              "createdAt":"2026-08-03T00:00:00Z",
              "lastOpenedAt":null,
              "defaultScene":"Scenes/Main.scene.json",
              "assetRoot":"Assets",
              "settingsVersion":"aife-project-settings.v1",
              "runtimeModule":{
                "moduleId":"fixture.runtime",
                "interfaceVersion":"project-runtime-module.v2",
                "cargoManifest":"RuntimeModule/Cargo.toml",
                "cargoPackage":"fixture_runtime",
                "playerBinary":"fixture_player"
              }
            }"#,
        )
        .unwrap();
        root
    }
}
