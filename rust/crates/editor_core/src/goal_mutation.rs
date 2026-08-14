use crate::{
    AiCandidateToolInput, AiGoalRiskClass, EditorSession, PatchRiskLevel, PatchSource,
    ProjectCandidateEntry, ProjectCandidateProjectBinding, ProjectCandidateSourceKind,
    ProjectPatchDocument, ProjectPatchLlmContextSnapshot,
};
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};

pub const EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION: &str =
    "external-project-mutation-intent.v1";
pub const BOUND_GOAL_MUTATION_SCHEMA_VERSION: &str = "bound-goal-mutation.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalProjectMutationIntent {
    pub schema_version: String,
    pub goal: ExternalProjectMutationGoal,
    pub change: ExternalProjectMutationChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalProjectMutationGoal {
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ExternalProjectMutationChange {
    ProjectPatch(ProjectPatchDocument),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalMutationOwnerFacts {
    pub client_session_id: String,
    pub read_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundGoalMutation {
    pub schema_version: String,
    pub normalized_goal_outcome: String,
    pub goal_digest: String,
    pub client_session_id: String,
    pub read_generation: u64,
    pub project_binding: ProjectCandidateProjectBinding,
    pub project_patch_context_hash: String,
    pub derived_risk_class: AiGoalRiskClass,
    pub candidate_input: AiCandidateToolInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalMutationError {
    pub code: String,
    pub message: String,
    pub next_action: String,
}

impl std::fmt::Display for GoalMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for GoalMutationError {}

pub struct GoalMutationModule;

impl GoalMutationModule {
    pub fn bind(
        session: &EditorSession,
        intent: ExternalProjectMutationIntent,
        owner_facts: GoalMutationOwnerFacts,
    ) -> Result<BoundGoalMutation, GoalMutationError> {
        if intent.schema_version != EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION {
            return Err(error(
                "goal_mutation.intent_schema_unsupported",
                "Project mutation intent schema is unsupported.",
                "Refresh the Tool Catalog and submit the declared project.mutate input.",
            ));
        }
        let normalized_goal_outcome = normalize_goal_outcome(&intent.goal.outcome)?;
        if owner_facts.client_session_id.trim().is_empty() || owner_facts.read_generation == 0 {
            return Err(error(
                "goal_mutation.owner_facts_invalid",
                "Gateway-owned session facts are missing or invalid.",
                "Reconnect to the current Editor and inspect the active project.",
            ));
        }

        let project_binding = ProjectCandidateEntry::inspect_project_binding(session)
            .map_err(|source| error(source.code, source.message, source.next_action))?;
        let project_patch_context_hash =
            ProjectPatchLlmContextSnapshot::capture(session).context_hash;
        let (mut patch, derived_risk_class) = match intent.change {
            ExternalProjectMutationChange::ProjectPatch(patch) => {
                let risk = derive_project_patch_risk(&patch);
                (patch, risk)
            }
        };
        patch.source = PatchSource::ImportedPatch;
        patch.intent_summary = normalized_goal_outcome.clone();
        patch.expected_outcome = normalized_goal_outcome.clone();
        patch.target_project_root = Some(project_binding.project_root.clone());
        patch.risk_level = match derived_risk_class {
            AiGoalRiskClass::Elevated => PatchRiskLevel::High,
            _ => PatchRiskLevel::Low,
        };

        let goal_digest = digest(&(
            &normalized_goal_outcome,
            &owner_facts.client_session_id,
            owner_facts.read_generation,
            &project_binding,
            &project_patch_context_hash,
            &patch,
        ))?;
        let candidate_id = format!(
            "goal-mutation-{}",
            goal_digest
                .trim_start_matches("sha256:")
                .chars()
                .take(32)
                .collect::<String>()
        );
        let envelope = ProjectCandidateEntry::project_patch_envelope(
            session,
            candidate_id,
            ProjectCandidateSourceKind::ImportedCodex,
            "external-project-mutation-intent",
            patch,
        )
        .map_err(|source| error(source.code, source.message, source.next_action))?;

        Ok(BoundGoalMutation {
            schema_version: BOUND_GOAL_MUTATION_SCHEMA_VERSION.to_string(),
            normalized_goal_outcome,
            goal_digest,
            client_session_id: owner_facts.client_session_id,
            read_generation: owner_facts.read_generation,
            project_binding,
            project_patch_context_hash,
            derived_risk_class,
            candidate_input: AiCandidateToolInput {
                envelope,
                source_file_path: None,
                controlled_source_patch_validation: None,
            },
        })
    }

    pub fn revalidate(
        session: &EditorSession,
        bound: &BoundGoalMutation,
    ) -> Result<(), GoalMutationError> {
        if bound.schema_version != BOUND_GOAL_MUTATION_SCHEMA_VERSION {
            return Err(error(
                "goal_mutation.binding_schema_unsupported",
                "Bound goal mutation schema is unsupported.",
                "Discard the operation and submit a new project.mutate intent.",
            ));
        }
        let current = ProjectCandidateEntry::inspect_project_binding(session)
            .map_err(|source| error(source.code, source.message, source.next_action))?;
        let current_context = ProjectPatchLlmContextSnapshot::capture(session).context_hash;
        if current != bound.project_binding || current_context != bound.project_patch_context_hash {
            return Err(error(
                "goal_mutation.project_drifted",
                "Project facts changed after project.mutate was bound.",
                "Inspect the current project and submit a new explicit mutation intent.",
            ));
        }
        Ok(())
    }
}

fn normalize_goal_outcome(outcome: &str) -> Result<String, GoalMutationError> {
    let normalized = outcome.split_whitespace().collect::<Vec<_>>().join(" ");
    let length = normalized.chars().count();
    if length == 0 || length > 2048 {
        return Err(error(
            "goal_mutation.goal_outcome_invalid",
            "goal.outcome must contain 1 to 2048 normalized characters.",
            "Describe the intended user-visible result without lifecycle steps.",
        ));
    }
    Ok(normalized)
}

fn derive_project_patch_risk(patch: &ProjectPatchDocument) -> AiGoalRiskClass {
    if patch.operations.iter().any(|operation| {
        let kind = operation.kind().to_ascii_lowercase();
        kind.contains("delete") || kind.contains("remove")
    }) {
        AiGoalRiskClass::Elevated
    } else {
        AiGoalRiskClass::ProjectOwnedLowRisk
    }
}

fn digest<T: Serialize>(value: &T) -> Result<String, GoalMutationError> {
    serde_json::to_value(value)
        .map_err(|source| {
            error(
                "goal_mutation.binding_digest_failed",
                format!("Could not serialize mutation facts: {source}"),
                "Discard the operation and inspect the current project facts.",
            )
        })
        .and_then(|value| {
            canonical_json_bytes(&value)
                .map(|bytes| sha256_prefixed(&bytes))
                .map_err(|source| {
                    error(
                        "goal_mutation.binding_digest_failed",
                        format!("Could not bind mutation facts: {source}"),
                        "Discard the operation and inspect the current project facts.",
                    )
                })
        })
}

fn error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> GoalMutationError {
    GoalMutationError {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}
