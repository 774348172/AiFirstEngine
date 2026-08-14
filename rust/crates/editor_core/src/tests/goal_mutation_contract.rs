use super::fixtures::*;
use super::*;
use std::fs;

fn mutation_session(name: &str) -> (EditorSession, PathBuf) {
    let root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: name.to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    (session, root)
}

fn intent(outcome: &str) -> ExternalProjectMutationIntent {
    ExternalProjectMutationIntent {
        schema_version: EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION.to_string(),
        goal: ExternalProjectMutationGoal {
            outcome: outcome.to_string(),
        },
        change: ExternalProjectMutationChange::ProjectPatch(ProjectPatchDocument::new(
            "goal-mutation-patch",
            "Goal mutation",
            PatchSource::Test,
            Vec::new(),
        )),
    }
}

fn owner_facts() -> GoalMutationOwnerFacts {
    GoalMutationOwnerFacts {
        client_session_id: "gateway-session-goal-mutation".to_string(),
        read_generation: 7,
    }
}

#[test]
fn goal_mutation_contract_binds_engine_owned_project_facts_and_normalizes_goal() {
    let (session, _) = mutation_session("GoalMutationBinding");
    let bound =
        GoalMutationModule::bind(&session, intent("  add   a player jump  "), owner_facts())
            .unwrap();

    assert_eq!(bound.normalized_goal_outcome, "add a player jump");
    assert_eq!(bound.read_generation, 7);
    assert_eq!(
        bound.candidate_input.envelope.target_project_id,
        bound.project_binding.project_id
    );
    assert_eq!(
        bound.candidate_input.envelope.expected_base_project_digest,
        bound.project_binding.project_digest
    );
    assert_eq!(
        bound.candidate_input.envelope.project_patch_context_hash,
        Some(bound.project_patch_context_hash.clone())
    );
    assert_eq!(
        bound.derived_risk_class,
        AiGoalRiskClass::ProjectOwnedLowRisk
    );
    GoalMutationModule::revalidate(&session, &bound).unwrap();
}

#[test]
fn goal_mutation_contract_rejects_no_project_and_project_drift() {
    let error = GoalMutationModule::bind(&EditorSession::new(), intent("change"), owner_facts())
        .unwrap_err();
    assert_eq!(error.code, "project_candidate_entry.no_active_project");

    let (session, root) = mutation_session("GoalMutationDrift");
    let bound = GoalMutationModule::bind(&session, intent("change"), owner_facts()).unwrap();
    fs::write(root.join("goal-mutation-drift.txt"), "drift").unwrap();
    let error = GoalMutationModule::revalidate(&session, &bound).unwrap_err();
    assert_eq!(error.code, "goal_mutation.project_drifted");
}
