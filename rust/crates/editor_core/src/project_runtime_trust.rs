use crate::project_player_artifact::runtime_module_source_digest;
use crate::project_runtime_player_staging::ProjectRuntimeProductionStaging;
use crate::{ProjectRelativePath, ProjectWriteScope};
use engine_runtime::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_RUNTIME_TRUST_DECISION_SCHEMA_VERSION: &str = "project-runtime-trust-decision.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeTrustDecisionKind {
    Trusted,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeTrustDecisionSource {
    ExplicitUser,
    RepositoryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeTrustDecision {
    pub schema_version: String,
    pub project_canonical_root_identity: String,
    pub project_id: String,
    pub runtime_module_source_digest: String,
    pub normalized_manifest_digest: String,
    pub normalized_dependency_digest: String,
    pub editor_build_identity: String,
    pub decision: ProjectRuntimeTrustDecisionKind,
    pub decided_at: u64,
    pub decision_source: ProjectRuntimeTrustDecisionSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeTrustRequest {
    pub project_root: PathBuf,
    pub project_id: String,
    pub runtime_module_source_digest: String,
    pub normalized_manifest_digest: String,
    pub normalized_dependency_digest: String,
    pub editor_build_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRuntimeTrustStatus {
    Trusted,
    Denied,
    Stale,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeTrustEvaluation {
    pub status: ProjectRuntimeTrustStatus,
    pub decision: Option<ProjectRuntimeTrustDecision>,
    pub diagnostic_code: Option<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeTrustInspection {
    pub canonical_project_root: PathBuf,
    pub project_name: String,
    pub module_id: String,
    pub dependency_summary: Vec<String>,
    pub request: ProjectRuntimeTrustRequest,
}

impl ProjectRuntimeTrustInspection {
    pub fn inspect(
        project_root: impl AsRef<Path>,
        engine_sdk_root: impl AsRef<Path>,
        editor_build_identity: impl Into<String>,
    ) -> Result<Self, ProjectRuntimeTrustError> {
        let project_root = project_root.as_ref();
        let canonical_project_root = canonical_directory(project_root, "project root")?;
        let plan = ProjectRuntimeProductionStaging::plan(
            &canonical_project_root,
            engine_sdk_root.as_ref(),
        )
        .map_err(|error| ProjectRuntimeTrustError::new(&error.code, error.message))?;
        let runtime_module_source_digest = runtime_module_source_digest(&canonical_project_root)
            .map_err(|error| ProjectRuntimeTrustError::new(&error.code, error.message))?;
        let dependency_summary = plan
            .normalized_dependencies
            .iter()
            .map(|dependency| {
                format!(
                    "{} {} ({})",
                    dependency.name, dependency.resolved_version, dependency.dependency_kind
                )
            })
            .collect();
        Ok(Self {
            canonical_project_root: canonical_project_root.clone(),
            project_name: plan.manifest.project_name.clone(),
            module_id: plan.manifest.runtime_module.module_id.clone(),
            dependency_summary,
            request: ProjectRuntimeTrustRequest {
                project_root: canonical_project_root,
                project_id: plan.manifest.project_id,
                runtime_module_source_digest,
                normalized_manifest_digest: plan.normalized_manifest_digest,
                normalized_dependency_digest: plan.normalized_dependency_digest,
                editor_build_identity: editor_build_identity.into(),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeRepositoryTrustEntry {
    pub project_id: String,
    pub project_relative_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectRuntimeRepositoryTrustPolicy {
    repository_root: PathBuf,
    allowed_projects: BTreeMap<String, PathBuf>,
}

impl ProjectRuntimeRepositoryTrustPolicy {
    pub fn explicit(
        repository_root: impl AsRef<Path>,
        entries: impl IntoIterator<Item = ProjectRuntimeRepositoryTrustEntry>,
    ) -> Result<Self, ProjectRuntimeTrustError> {
        let repository_root = canonical_directory(repository_root.as_ref(), "repository root")?;
        let mut allowed_projects = BTreeMap::new();
        for entry in entries {
            let relative =
                ProjectRelativePath::parse(&entry.project_relative_path).map_err(|error| {
                    ProjectRuntimeTrustError::new(
                        "project_editor_composition.repository_policy_invalid",
                        error.to_string(),
                    )
                })?;
            let project_root = canonical_directory(
                &repository_root.join(relative.as_path()),
                "repository policy project root",
            )?;
            if !project_root.starts_with(&repository_root) || project_root == repository_root {
                return Err(ProjectRuntimeTrustError::new(
                    "project_editor_composition.repository_policy_invalid",
                    "Repository policy project must be a strict child of the repository root.",
                ));
            }
            if allowed_projects
                .insert(entry.project_id.clone(), project_root)
                .is_some()
            {
                return Err(ProjectRuntimeTrustError::new(
                    "project_editor_composition.repository_policy_invalid",
                    format!(
                        "Duplicate repository policy project id: {}",
                        entry.project_id
                    ),
                ));
            }
        }
        Ok(Self {
            repository_root,
            allowed_projects,
        })
    }

    fn permits(&self, project_id: &str, canonical_project_root: &Path) -> bool {
        canonical_project_root.starts_with(&self.repository_root)
            && self
                .allowed_projects
                .get(project_id)
                .is_some_and(|allowed| allowed == canonical_project_root)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimeTrustError {
    pub code: String,
    pub message: String,
}

impl ProjectRuntimeTrustError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ProjectRuntimeTrustError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProjectRuntimeTrustError {}

#[derive(Clone)]
pub struct ProjectRuntimeTrustModule {
    state_root: PathBuf,
    scope: ProjectWriteScope,
}

impl ProjectRuntimeTrustModule {
    pub fn open(state_root: impl AsRef<Path>) -> Result<Self, ProjectRuntimeTrustError> {
        let supplied_state_root = state_root.as_ref();
        fs::create_dir_all(supplied_state_root).map_err(|error| {
            ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_store_unavailable",
                format!("Project Runtime trust store cannot be created: {error}"),
            )
        })?;
        let metadata = fs::symlink_metadata(supplied_state_root).map_err(|error| {
            ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_store_unavailable",
                format!("Project Runtime trust store cannot be inspected: {error}"),
            )
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_store_unavailable",
                "Project Runtime trust store cannot be a link or reparse point.",
            ));
        }
        let state_root = canonical_directory(supplied_state_root, "trust store root")?;
        let scope = ProjectWriteScope::open(&state_root).map_err(|error| {
            ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_store_unavailable",
                error.to_string(),
            )
        })?;
        Ok(Self { state_root, scope })
    }

    pub fn evaluate(
        &self,
        request: &ProjectRuntimeTrustRequest,
        repository_policy: Option<&ProjectRuntimeRepositoryTrustPolicy>,
    ) -> Result<ProjectRuntimeTrustEvaluation, ProjectRuntimeTrustError> {
        let canonical_project_root = self.validate_request_root(request)?;
        let expected = expected_decision(
            request,
            &canonical_project_root,
            ProjectRuntimeTrustDecisionKind::Trusted,
            ProjectRuntimeTrustDecisionSource::ExplicitUser,
            0,
        );
        if repository_policy.is_some_and(|policy| {
            policy.permits(&request.project_id, canonical_project_root.as_path())
        }) {
            let mut decision = expected;
            decision.decision_source = ProjectRuntimeTrustDecisionSource::RepositoryPolicy;
            return Ok(ProjectRuntimeTrustEvaluation {
                status: ProjectRuntimeTrustStatus::Trusted,
                decision: Some(decision),
                diagnostic_code: None,
                next_action: "continue".to_string(),
            });
        }

        let relative = self.receipt_relative_path(
            &expected.project_canonical_root_identity,
            &request.project_id,
        )?;
        let bytes = match self.scope.read(relative.as_path()) {
            Ok(bytes) => bytes,
            Err(error) if error.code == "project_write.capability_denied" => {
                return Ok(ProjectRuntimeTrustEvaluation {
                    status: ProjectRuntimeTrustStatus::Required,
                    decision: None,
                    diagnostic_code: Some("project_editor_composition.trust_required".to_string()),
                    next_action: "Ask the user to approve or deny this ProjectRust identity."
                        .to_string(),
                });
            }
            Err(error) => {
                return Err(ProjectRuntimeTrustError::new(
                    "project_editor_composition.trust_store_read_failed",
                    error.to_string(),
                ));
            }
        };
        let decision: ProjectRuntimeTrustDecision =
            serde_json::from_slice(&bytes).map_err(|error| {
                ProjectRuntimeTrustError::new(
                    "project_editor_composition.trust_store_invalid",
                    format!("Project Runtime trust receipt is invalid: {error}"),
                )
            })?;
        if !same_identity(&decision, &expected) {
            return Ok(ProjectRuntimeTrustEvaluation {
                status: ProjectRuntimeTrustStatus::Stale,
                decision: Some(decision),
                diagnostic_code: Some("project_editor_composition.trust_stale".to_string()),
                next_action: "Ask the user to review the changed ProjectRust identity.".to_string(),
            });
        }
        let status = match decision.decision {
            ProjectRuntimeTrustDecisionKind::Trusted => ProjectRuntimeTrustStatus::Trusted,
            ProjectRuntimeTrustDecisionKind::Denied => ProjectRuntimeTrustStatus::Denied,
        };
        Ok(ProjectRuntimeTrustEvaluation {
            status,
            diagnostic_code: (status == ProjectRuntimeTrustStatus::Denied)
                .then(|| "project_editor_composition.trust_denied".to_string()),
            next_action: if status == ProjectRuntimeTrustStatus::Trusted {
                "continue".to_string()
            } else {
                "Keep the current Editor and do not build or launch project native code."
                    .to_string()
            },
            decision: Some(decision),
        })
    }

    pub fn record_explicit(
        &self,
        request: &ProjectRuntimeTrustRequest,
        decision: ProjectRuntimeTrustDecisionKind,
        decided_at: u64,
    ) -> Result<ProjectRuntimeTrustDecision, ProjectRuntimeTrustError> {
        let canonical_project_root = self.validate_request_root(request)?;
        let receipt = expected_decision(
            request,
            &canonical_project_root,
            decision,
            ProjectRuntimeTrustDecisionSource::ExplicitUser,
            decided_at,
        );
        let relative = self.receipt_relative_path(
            &receipt.project_canonical_root_identity,
            &receipt.project_id,
        )?;
        let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| {
            ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_store_write_failed",
                format!("Project Runtime trust receipt cannot be serialized: {error}"),
            )
        })?;
        self.scope
            .write_atomic(relative.as_path(), &bytes)
            .map_err(|error| {
                ProjectRuntimeTrustError::new(
                    "project_editor_composition.trust_store_write_failed",
                    error.to_string(),
                )
            })?;
        Ok(receipt)
    }

    fn validate_request_root(
        &self,
        request: &ProjectRuntimeTrustRequest,
    ) -> Result<PathBuf, ProjectRuntimeTrustError> {
        if request.project_id.trim().is_empty()
            || request.runtime_module_source_digest.trim().is_empty()
            || request.normalized_manifest_digest.trim().is_empty()
            || request.normalized_dependency_digest.trim().is_empty()
            || request.editor_build_identity.trim().is_empty()
        {
            return Err(ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_request_invalid",
                "Project Runtime trust request identity fields are required.",
            ));
        }
        let project_root = canonical_directory(&request.project_root, "project root")?;
        if self.state_root.starts_with(&project_root) {
            return Err(ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_store_inside_project",
                "Project Runtime trust receipts cannot be stored inside the project root.",
            ));
        }
        Ok(project_root)
    }

    fn receipt_relative_path(
        &self,
        root_identity: &str,
        project_id: &str,
    ) -> Result<ProjectRelativePath, ProjectRuntimeTrustError> {
        let key = sha256_prefixed(format!("{root_identity}\0{project_id}").as_bytes())
            .trim_start_matches("sha256:")
            .to_string();
        ProjectRelativePath::parse(format!("receipts/{key}.json")).map_err(|error| {
            ProjectRuntimeTrustError::new(
                "project_editor_composition.trust_store_invalid",
                error.to_string(),
            )
        })
    }
}

fn expected_decision(
    request: &ProjectRuntimeTrustRequest,
    canonical_project_root: &Path,
    decision: ProjectRuntimeTrustDecisionKind,
    decision_source: ProjectRuntimeTrustDecisionSource,
    decided_at: u64,
) -> ProjectRuntimeTrustDecision {
    ProjectRuntimeTrustDecision {
        schema_version: PROJECT_RUNTIME_TRUST_DECISION_SCHEMA_VERSION.to_string(),
        project_canonical_root_identity: sha256_prefixed(
            canonical_project_root.to_string_lossy().as_bytes(),
        ),
        project_id: request.project_id.clone(),
        runtime_module_source_digest: request.runtime_module_source_digest.clone(),
        normalized_manifest_digest: request.normalized_manifest_digest.clone(),
        normalized_dependency_digest: request.normalized_dependency_digest.clone(),
        editor_build_identity: request.editor_build_identity.clone(),
        decision,
        decided_at,
        decision_source,
    }
}

fn same_identity(
    actual: &ProjectRuntimeTrustDecision,
    expected: &ProjectRuntimeTrustDecision,
) -> bool {
    actual.schema_version == PROJECT_RUNTIME_TRUST_DECISION_SCHEMA_VERSION
        && actual.project_canonical_root_identity == expected.project_canonical_root_identity
        && actual.project_id == expected.project_id
        && actual.runtime_module_source_digest == expected.runtime_module_source_digest
        && actual.normalized_manifest_digest == expected.normalized_manifest_digest
        && actual.normalized_dependency_digest == expected.normalized_dependency_digest
        && actual.editor_build_identity == expected.editor_build_identity
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ProjectRuntimeTrustError> {
    let canonical = path.canonicalize().map_err(|error| {
        ProjectRuntimeTrustError::new(
            "project_editor_composition.trust_path_invalid",
            format!("{label} cannot be canonicalized: {error}"),
        )
    })?;
    if !canonical.is_dir() {
        return Err(ProjectRuntimeTrustError::new(
            "project_editor_composition.trust_path_invalid",
            format!("{label} is not a directory."),
        ));
    }
    Ok(canonical)
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("aife-262-trust-{label}-{stamp}"))
    }

    fn fixture(label: &str) -> (PathBuf, PathBuf, ProjectRuntimeTrustRequest) {
        let root = temp_root(label);
        let project = root.join("project");
        let state = root.join("application-state");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&state).unwrap();
        let request = ProjectRuntimeTrustRequest {
            project_root: project,
            project_id: "fixture.project".to_string(),
            runtime_module_source_digest: "sha256:source".to_string(),
            normalized_manifest_digest: "sha256:manifest".to_string(),
            normalized_dependency_digest: "sha256:dependencies".to_string(),
            editor_build_identity: "sha256:editor".to_string(),
        };
        (root, state, request)
    }

    #[cfg(windows)]
    fn create_directory_link(link: &Path, target: &Path) {
        let output = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction creation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(not(windows))]
    fn create_directory_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).unwrap();
    }

    #[cfg(not(windows))]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).unwrap();
    }

    #[test]
    fn project_runtime_trust_requires_explicit_decision_and_round_trips_strict_schema() {
        let (root, state, request) = fixture("round-trip");
        let trust = ProjectRuntimeTrustModule::open(&state).unwrap();
        let required = trust.evaluate(&request, None).unwrap();
        assert_eq!(required.status, ProjectRuntimeTrustStatus::Required);

        let recorded = trust
            .record_explicit(&request, ProjectRuntimeTrustDecisionKind::Trusted, 42)
            .unwrap();
        assert_eq!(recorded.decided_at, 42);
        let trusted = trust.evaluate(&request, None).unwrap();
        assert_eq!(trusted.status, ProjectRuntimeTrustStatus::Trusted);
        assert_eq!(trusted.decision, Some(recorded.clone()));

        let mut value = serde_json::to_value(recorded).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        assert!(serde_json::from_value::<ProjectRuntimeTrustDecision>(value).is_err());
        drop(trust);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_runtime_trust_tracks_source_manifest_dependency_and_editor_identity() {
        let (root, state, request) = fixture("stale");
        let trust = ProjectRuntimeTrustModule::open(&state).unwrap();
        trust
            .record_explicit(&request, ProjectRuntimeTrustDecisionKind::Trusted, 42)
            .unwrap();
        let mutations: [fn(&mut ProjectRuntimeTrustRequest); 4] = [
            |value| value.runtime_module_source_digest.push_str(".changed"),
            |value| value.normalized_manifest_digest.push_str(".changed"),
            |value| value.normalized_dependency_digest.push_str(".changed"),
            |value| value.editor_build_identity.push_str(".changed"),
        ];
        for mutate in mutations {
            let mut changed = request.clone();
            mutate(&mut changed);
            let evaluation = trust.evaluate(&changed, None).unwrap();
            assert_eq!(evaluation.status, ProjectRuntimeTrustStatus::Stale);
            assert_eq!(
                evaluation.diagnostic_code.as_deref(),
                Some("project_editor_composition.trust_stale")
            );
        }
        drop(trust);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_runtime_trust_denial_is_terminal_for_exact_identity() {
        let (root, state, request) = fixture("denied");
        let trust = ProjectRuntimeTrustModule::open(&state).unwrap();
        trust
            .record_explicit(&request, ProjectRuntimeTrustDecisionKind::Denied, 42)
            .unwrap();
        let denied = trust.evaluate(&request, None).unwrap();
        assert_eq!(denied.status, ProjectRuntimeTrustStatus::Denied);
        assert_eq!(
            denied.diagnostic_code.as_deref(),
            Some("project_editor_composition.trust_denied")
        );
        drop(trust);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_runtime_trust_store_cannot_live_inside_project() {
        let root = temp_root("inside-project");
        let project = root.join("project");
        let state = project.join("forged-trust");
        fs::create_dir_all(&state).unwrap();
        let trust = ProjectRuntimeTrustModule::open(&state).unwrap();
        let request = ProjectRuntimeTrustRequest {
            project_root: project,
            project_id: "fixture.project".to_string(),
            runtime_module_source_digest: "sha256:source".to_string(),
            normalized_manifest_digest: "sha256:manifest".to_string(),
            normalized_dependency_digest: "sha256:dependencies".to_string(),
            editor_build_identity: "sha256:editor".to_string(),
        };
        let error = trust.evaluate(&request, None).unwrap_err();
        assert_eq!(
            error.code,
            "project_editor_composition.trust_store_inside_project"
        );
        drop(trust);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_runtime_trust_store_rejects_supplied_link_or_reparse_root() {
        let root = temp_root("reparse-root");
        let actual = root.join("actual-state");
        let linked = root.join("linked-state");
        fs::create_dir_all(&actual).unwrap();
        create_directory_link(&linked, &actual);

        let error = match ProjectRuntimeTrustModule::open(&linked) {
            Ok(_) => panic!("linked trust store root must be rejected"),
            Err(error) => error,
        };
        assert_eq!(
            error.code,
            "project_editor_composition.trust_store_unavailable"
        );

        remove_directory_link(&linked);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_runtime_trust_repository_policy_requires_exact_id_and_path() {
        let root = temp_root("repository-policy");
        let repository = root.join("repository");
        let project = repository.join("samples/allowed");
        let sibling = repository.join("samples/sibling");
        let state = root.join("application-state");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::create_dir_all(&state).unwrap();
        let policy = ProjectRuntimeRepositoryTrustPolicy::explicit(
            &repository,
            [ProjectRuntimeRepositoryTrustEntry {
                project_id: "fixture.allowed".to_string(),
                project_relative_path: PathBuf::from("samples/allowed"),
            }],
        )
        .unwrap();
        let trust = ProjectRuntimeTrustModule::open(&state).unwrap();
        let mut request = ProjectRuntimeTrustRequest {
            project_root: project,
            project_id: "fixture.allowed".to_string(),
            runtime_module_source_digest: "sha256:source".to_string(),
            normalized_manifest_digest: "sha256:manifest".to_string(),
            normalized_dependency_digest: "sha256:dependencies".to_string(),
            editor_build_identity: "sha256:editor".to_string(),
        };
        let allowed = trust.evaluate(&request, Some(&policy)).unwrap();
        assert_eq!(allowed.status, ProjectRuntimeTrustStatus::Trusted);
        assert_eq!(
            allowed.decision.unwrap().decision_source,
            ProjectRuntimeTrustDecisionSource::RepositoryPolicy
        );

        request.project_root = sibling;
        assert_eq!(
            trust.evaluate(&request, Some(&policy)).unwrap().status,
            ProjectRuntimeTrustStatus::Required
        );
        request.project_root = repository.join("samples/allowed");
        request.project_id = "fixture.wrong".to_string();
        assert_eq!(
            trust.evaluate(&request, Some(&policy)).unwrap().status,
            ProjectRuntimeTrustStatus::Required
        );
        drop(trust);
        fs::remove_dir_all(root).unwrap();
    }
}
