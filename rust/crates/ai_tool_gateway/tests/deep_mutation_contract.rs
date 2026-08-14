use ai_tool_gateway::{
    ClientHello, ClientKind, ClientSessionBinding, GatewayAccessDecision, GatewayCore,
    GatewayReplyPayload, GatewayRequest, GatewayRequestPayload,
    GATEWAY_CLIENT_HELLO_SCHEMA_VERSION, GATEWAY_PROTOCOL_VERSION, GATEWAY_REQUEST_SCHEMA_VERSION,
};
use editor_core::{
    command_for_test, AiCapabilityGrant, AiGoalBinding, AiGoalCompletionPolicy, AiGoalGrantSpec,
    AiRiskEnvelope, AiRiskEnvelopeSpec, AiToolContractRegistry, AiToolInvocation,
    AiToolInvocationPayload, AiToolOperationState, CommandStatus, EditorSession,
    ExternalProjectMutationChange, ExternalProjectMutationGoal, ExternalProjectMutationIntent,
    InputPatchOperation, PatchOperation, PatchSource, ProjectCandidateEntry, ProjectPatchDocument,
    AI_TOOL_CATALOG_SCHEMA_VERSION, AI_TOOL_INVOCATION_SCHEMA_VERSION,
    EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION, TOOL_ID_PROJECT_MUTATE,
};
use editor_ui_model::{InputActionValueKind, UiCommandPayload};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn project_patch_value() -> Value {
    serde_json::to_value(ProjectPatchDocument::new(
        "deep-mutation-contract",
        "Deep mutation contract",
        PatchSource::AiAssistant,
        Vec::new(),
    ))
    .unwrap()
}

fn caller_input() -> Value {
    json!({
        "goal": {"outcome": "Add a player jump action."},
        "change": {
            "kind": "project_patch",
            "payload": project_patch_value()
        }
    })
}

#[test]
fn caller_schema_exposes_only_goal_and_change_and_decodes_project_patch() {
    let registry = AiToolContractRegistry::new();
    let descriptor = registry
        .descriptor(TOOL_ID_PROJECT_MUTATE)
        .expect("project.mutate descriptor");
    let properties = descriptor.input_schema["properties"]
        .as_object()
        .expect("project.mutate properties");
    assert_eq!(
        properties.keys().cloned().collect::<Vec<_>>(),
        vec!["change".to_string(), "goal".to_string()]
    );
    assert_eq!(properties["goal"]["additionalProperties"], false);
    assert_eq!(properties["change"]["additionalProperties"], false);

    let payload = registry
        .decode_invocation_payload(TOOL_ID_PROJECT_MUTATE, caller_input())
        .unwrap();
    let AiToolInvocationPayload::ProjectMutationIntent(intent) = payload else {
        panic!("project.mutate must decode to caller intent");
    };
    assert_eq!(intent.goal.outcome, "Add a player jump action.");
    assert!(matches!(
        intent.change,
        ExternalProjectMutationChange::ProjectPatch(_)
    ));
}

#[test]
fn caller_schema_rejects_internal_owner_fields_and_unsupported_change() {
    let registry = AiToolContractRegistry::new();
    for forbidden in [
        "goalId",
        "riskIntent",
        "projectRoot",
        "expectedProjectDigest",
        "readGeneration",
        "projectPatchContextHash",
        "candidateId",
        "revisionId",
        "grantId",
        "operationId",
        "receiptId",
    ] {
        let mut input = caller_input();
        input
            .as_object_mut()
            .unwrap()
            .insert(forbidden.to_string(), json!("caller-owned"));
        let error = registry
            .decode_invocation_payload(TOOL_ID_PROJECT_MUTATE, input)
            .unwrap_err();
        assert_eq!(error.code, "ai_tool.direct_input_schema_invalid");
    }

    let mut unsupported = caller_input();
    unsupported["change"]["kind"] = json!("controlled_source_patch");
    let error = registry
        .decode_invocation_payload(TOOL_ID_PROJECT_MUTATE, unsupported)
        .unwrap_err();
    assert_eq!(error.code, "ai_tool.direct_input_schema_invalid");
}

#[test]
fn same_operation_approve_once_continues_and_reuses_eligible_goal_grant() {
    let (mut session, root) = created_project("same-operation-reuse");
    let mut core = GatewayCore::new();
    let binding = core
        .connect(&mut session, hello("same-operation.v1"))
        .unwrap();
    let outcome = "Add bounded player input actions.";

    let first_invocation =
        mutation_invocation(&session, "same-operation-first", outcome, "jump", false);
    let accepted = execute_goal_mutation(&mut core, &mut session, &binding, first_invocation);
    assert_eq!(accepted.state, AiToolOperationState::AwaitingUser);
    let request = core
        .approval_inbox(now_epoch_ms())
        .into_iter()
        .next()
        .expect("same operation approval request");
    assert_eq!(
        request.operation_id.as_deref(),
        Some(accepted.operation_id.as_str())
    );
    let approval = core
        .decide_access(
            &session,
            &request.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();
    assert!(approval.grant_digest.is_some());
    assert_eq!(
        core.decide_access(
            &session,
            &request.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap_err()
        .code,
        "gateway.access.request_stale"
    );
    assert!(core.pump_operations(&mut session, 8) > 0);
    let first = observe(&mut core, &mut session, &binding, &accepted.operation_id);
    assert_eq!(first.state, AiToolOperationState::Completed);
    assert_eq!(
        first.transitions.first().map(|transition| transition.state),
        Some(AiToolOperationState::AwaitingUser)
    );

    let reused_invocation =
        mutation_invocation(&session, "same-operation-second", outcome, "dash", false);
    let reused = execute_goal_mutation(&mut core, &mut session, &binding, reused_invocation);
    assert_ne!(reused.state, AiToolOperationState::AwaitingUser);
    assert!(core.approval_inbox(now_epoch_ms()).is_empty());
    assert!(core.pump_operations(&mut session, 8) > 0);
    let second = observe(&mut core, &mut session, &binding, &reused.operation_id);
    assert_eq!(second.state, AiToolOperationState::Completed);
    let status = core
        .session_status(&session, &binding.client_session_id, now_epoch_ms())
        .unwrap();
    assert_eq!(status.access.mutation.remaining_mutation_count, Some(14));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn same_operation_reject_ttl_cancel_drift_and_disconnect_are_terminal() {
    for scenario in [
        "reject",
        "ttl",
        "cancel",
        "drift",
        "disconnect",
        "disconnect_after_approval",
    ] {
        let (mut session, root) = created_project(scenario);
        let initial = ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_digest;
        let mut core = GatewayCore::new();
        let binding = core.connect(&mut session, hello(scenario)).unwrap();
        let invocation = mutation_invocation(
            &session,
            &format!("{scenario}-operation"),
            "Apply one bounded input change.",
            scenario,
            false,
        );
        let accepted = execute_goal_mutation(&mut core, &mut session, &binding, invocation);
        let request = core
            .approval_inbox(now_epoch_ms())
            .into_iter()
            .next()
            .expect("pending approval request");

        let expected_terminal_digest = match scenario {
            "reject" => {
                core.decide_access(
                    &session,
                    &request.request_id,
                    GatewayAccessDecision::Reject,
                    "native-editor-user",
                    now_epoch_ms(),
                )
                .unwrap();
                assert_eq!(
                    observe(&mut core, &mut session, &binding, &accepted.operation_id).state,
                    AiToolOperationState::Failed
                );
                initial.clone()
            }
            "ttl" => {
                assert_eq!(
                    core.decide_access(
                        &session,
                        &request.request_id,
                        GatewayAccessDecision::Approve,
                        "native-editor-user",
                        request.expires_at_epoch_ms,
                    )
                    .unwrap_err()
                    .code,
                    "gateway.access.request_expired"
                );
                assert_eq!(
                    observe(&mut core, &mut session, &binding, &accepted.operation_id).state,
                    AiToolOperationState::Failed
                );
                initial.clone()
            }
            "cancel" => {
                let reply = core.dispatch(
                    &mut session,
                    bound_request(
                        &binding,
                        "cancel-awaiting",
                        GatewayRequestPayload::CancelSessionBound {
                            operation_id: accepted.operation_id.clone(),
                        },
                    ),
                );
                assert!(matches!(
                    reply.payload,
                    GatewayReplyPayload::Cancellation(_)
                ));
                assert_eq!(
                    observe(&mut core, &mut session, &binding, &accepted.operation_id).state,
                    AiToolOperationState::Cancelled
                );
                initial.clone()
            }
            "drift" => {
                std::fs::write(root.join("external-drift.txt"), "drift").unwrap();
                let drifted_digest = ProjectCandidateEntry::inspect_project_binding(&session)
                    .unwrap()
                    .project_digest;
                let snapshot = observe(&mut core, &mut session, &binding, &accepted.operation_id);
                assert_eq!(snapshot.state, AiToolOperationState::Failed);
                assert_eq!(
                    snapshot.result.unwrap().diagnostics[0].code,
                    "gateway.operation.project_drifted"
                );
                drifted_digest
            }
            "disconnect" => {
                core.close(&binding.client_session_id);
                assert!(core.approval_inbox(now_epoch_ms()).is_empty());
                assert_eq!(core.pump_operations(&mut session, 8), 0);
                initial.clone()
            }
            "disconnect_after_approval" => {
                core.decide_access(
                    &session,
                    &request.request_id,
                    GatewayAccessDecision::Approve,
                    "native-editor-user",
                    now_epoch_ms(),
                )
                .unwrap();
                core.close(&binding.client_session_id);
                assert!(core.approval_inbox(now_epoch_ms()).is_empty());
                assert_eq!(core.pump_operations(&mut session, 8), 0);
                initial.clone()
            }
            _ => unreachable!(),
        };
        assert_eq!(
            ProjectCandidateEntry::inspect_project_binding(&session)
                .unwrap()
                .project_digest,
            expected_terminal_digest,
            "{scenario} must not apply after its terminal boundary"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn clean_scene_save_preserves_pending_approval_but_dirty_save_terminalizes_drift() {
    let (mut clean_session, clean_root) = created_legacy_scene_project("clean-save-approval");
    let clean_initial = ProjectCandidateEntry::inspect_project_binding(&clean_session)
        .unwrap()
        .project_digest;
    let mut clean_core = GatewayCore::new();
    let clean_binding = clean_core
        .connect(&mut clean_session, hello("clean-save-approval.v1"))
        .unwrap();
    let clean_invocation = mutation_invocation(
        &clean_session,
        "clean-save-approval",
        "Add input after a clean Scene save.",
        "clean-save",
        false,
    );
    let clean_accepted = execute_goal_mutation(
        &mut clean_core,
        &mut clean_session,
        &clean_binding,
        clean_invocation,
    );
    let clean_request = clean_core.approval_inbox(now_epoch_ms()).remove(0);

    let clean_save =
        clean_session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
            path: None,
        }));
    assert_eq!(
        clean_save.status,
        CommandStatus::Committed,
        "{clean_save:?}"
    );
    assert_eq!(
        ProjectCandidateEntry::inspect_project_binding(&clean_session)
            .unwrap()
            .project_digest,
        clean_initial
    );
    clean_core
        .decide_access(
            &clean_session,
            &clean_request.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();
    assert!(clean_core.pump_operations(&mut clean_session, 8) > 0);
    assert_eq!(
        observe(
            &mut clean_core,
            &mut clean_session,
            &clean_binding,
            &clean_accepted.operation_id,
        )
        .state,
        AiToolOperationState::Completed
    );
    let _ = std::fs::remove_dir_all(clean_root);

    let (mut dirty_session, dirty_root) = created_legacy_scene_project("dirty-save-drift");
    let dirty_initial = ProjectCandidateEntry::inspect_project_binding(&dirty_session)
        .unwrap()
        .project_digest;
    let mut dirty_core = GatewayCore::new();
    let dirty_binding = dirty_core
        .connect(&mut dirty_session, hello("dirty-save-drift.v1"))
        .unwrap();
    let dirty_invocation = mutation_invocation(
        &dirty_session,
        "dirty-save-drift",
        "Do not apply after a real Scene change.",
        "dirty-save",
        false,
    );
    let dirty_accepted = execute_goal_mutation(
        &mut dirty_core,
        &mut dirty_session,
        &dirty_binding,
        dirty_invocation,
    );
    assert_eq!(dirty_accepted.state, AiToolOperationState::AwaitingUser);

    let rename =
        dirty_session.execute_command(command_for_test(UiCommandPayload::RenameSceneEntity {
            entity_id: "entity-puzzle-switch".to_string(),
            name: "Changed During Approval".to_string(),
        }));
    assert_eq!(rename.status, CommandStatus::Committed, "{rename:?}");
    let dirty_save =
        dirty_session.execute_command(command_for_test(UiCommandPayload::SaveSceneDocument {
            path: None,
        }));
    assert_eq!(
        dirty_save.status,
        CommandStatus::Committed,
        "{dirty_save:?}"
    );
    assert_ne!(
        ProjectCandidateEntry::inspect_project_binding(&dirty_session)
            .unwrap()
            .project_digest,
        dirty_initial
    );
    let dirty_snapshot = observe(
        &mut dirty_core,
        &mut dirty_session,
        &dirty_binding,
        &dirty_accepted.operation_id,
    );
    assert_eq!(dirty_snapshot.state, AiToolOperationState::Failed);
    assert_eq!(
        dirty_snapshot.result.unwrap().diagnostics[0].code,
        "gateway.operation.project_drifted"
    );
    assert!(!dirty_root.join("Input/dirty-save.input.json").exists());
    let _ = std::fs::remove_dir_all(dirty_root);
}

#[test]
fn same_operation_goal_risk_budget_and_session_changes_do_not_reuse_grant() {
    let (mut session, root) = created_project("non-reuse");
    let mut core = GatewayCore::new();
    let binding = core.connect(&mut session, hello("non-reuse.v1")).unwrap();
    let outcome = "Maintain the input mapping.";
    let first_invocation =
        mutation_invocation(&session, "non-reuse-first", outcome, "first", false);
    let first = execute_goal_mutation(&mut core, &mut session, &binding, first_invocation);
    let request = core.approval_inbox(now_epoch_ms()).remove(0);
    core.decide_access(
        &session,
        &request.request_id,
        GatewayAccessDecision::Approve,
        "native-editor-user",
        now_epoch_ms(),
    )
    .unwrap();
    core.pump_operations(&mut session, 8);
    assert_eq!(
        observe(&mut core, &mut session, &binding, &first.operation_id).state,
        AiToolOperationState::Completed
    );

    let risk_changed_invocation =
        mutation_invocation(&session, "non-reuse-risk", outcome, "first", true);
    let risk_changed =
        execute_goal_mutation(&mut core, &mut session, &binding, risk_changed_invocation);
    assert_eq!(risk_changed.state, AiToolOperationState::AwaitingUser);
    let elevated = core.approval_inbox(now_epoch_ms()).remove(0);
    assert_eq!(elevated.requested_profile, "elevated");
    cancel(
        &mut core,
        &mut session,
        &binding,
        &risk_changed.operation_id,
    );

    let goal_changed_invocation = mutation_invocation(
        &session,
        "non-reuse-goal",
        "Use a different user-visible outcome.",
        "different",
        false,
    );
    let goal_changed =
        execute_goal_mutation(&mut core, &mut session, &binding, goal_changed_invocation);
    assert_eq!(goal_changed.state, AiToolOperationState::AwaitingUser);
    cancel(
        &mut core,
        &mut session,
        &binding,
        &goal_changed.operation_id,
    );
    let _ = std::fs::remove_dir_all(root);

    let (mut budget_session, budget_root) = created_project("budget-non-reuse");
    let mut budget_core = GatewayCore::new();
    let budget_binding = budget_core
        .connect(&mut budget_session, hello("budget.v1"))
        .unwrap();
    let project = ProjectCandidateEntry::inspect_project_binding(&budget_session).unwrap();
    let goal = AiGoalBinding::new(
        "budget-goal",
        outcome,
        project.project_id.clone(),
        project.project_digest.clone(),
        AiGoalCompletionPolicy::CommitVerified,
    )
    .unwrap();
    let narrow_risk = AiRiskEnvelope::new(AiRiskEnvelopeSpec {
        risk_class: editor_core::AiGoalRiskClass::ProjectOwnedLowRisk,
        allowed_paths: Vec::new(),
        denied_paths: Vec::new(),
        allowed_objects: Vec::new(),
        max_mutation_count: 1,
        time_budget_ms: 900_000,
        external_cost_budget_microunits: 0,
        allow_delete: false,
        allow_dependency_change: false,
        allow_network: false,
    })
    .unwrap();
    let spec = AiGoalGrantSpec::new(
        goal,
        narrow_risk,
        budget_binding.client_session_id.clone(),
        "native-editor-user",
        Some(now_epoch_ms() + 60_000),
    )
    .unwrap();
    let grant = AiCapabilityGrant::project_owned_low_risk_for_goal(spec).unwrap();
    budget_core
        .issue_grant_ref(&budget_session, &budget_binding.client_session_id, grant)
        .unwrap();
    let budget_changed_invocation = mutation_invocation(
        &budget_session,
        "non-reuse-budget",
        outcome,
        "budget",
        false,
    );
    let budget_changed = execute_goal_mutation(
        &mut budget_core,
        &mut budget_session,
        &budget_binding,
        budget_changed_invocation,
    );
    assert_eq!(budget_changed.state, AiToolOperationState::AwaitingUser);
    let _ = std::fs::remove_dir_all(budget_root);

    let (mut session_changed, session_root) = created_project("session-non-reuse");
    let mut session_core = GatewayCore::new();
    let first_binding = session_core
        .connect(&mut session_changed, hello("session-first.v1"))
        .unwrap();
    let session_first_invocation = mutation_invocation(
        &session_changed,
        "session-first",
        outcome,
        "session-first",
        false,
    );
    let session_first = execute_goal_mutation(
        &mut session_core,
        &mut session_changed,
        &first_binding,
        session_first_invocation,
    );
    let session_request = session_core.approval_inbox(now_epoch_ms()).remove(0);
    session_core
        .decide_access(
            &session_changed,
            &session_request.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .unwrap();
    session_core.pump_operations(&mut session_changed, 8);
    assert_eq!(
        observe(
            &mut session_core,
            &mut session_changed,
            &first_binding,
            &session_first.operation_id,
        )
        .state,
        AiToolOperationState::Completed
    );
    session_core.close(&first_binding.client_session_id);
    let second_binding = session_core
        .connect(&mut session_changed, hello("session-second.v1"))
        .unwrap();
    let session_second_invocation = mutation_invocation(
        &session_changed,
        "session-second",
        outcome,
        "session-second",
        false,
    );
    let session_second = execute_goal_mutation(
        &mut session_core,
        &mut session_changed,
        &second_binding,
        session_second_invocation,
    );
    assert_eq!(session_second.state, AiToolOperationState::AwaitingUser);
    let _ = std::fs::remove_dir_all(session_root);
}

fn mutation_invocation(
    session: &EditorSession,
    invocation_id: &str,
    outcome: &str,
    action_id: &str,
    delete: bool,
) -> AiToolInvocation {
    let path = format!("Input/{action_id}.input.json");
    let operations = if delete {
        vec![PatchOperation::Input(
            InputPatchOperation::DeleteInputMapping {
                operation_id: format!("delete-{action_id}"),
                depends_on: Vec::new(),
                path,
            },
        )]
    } else {
        vec![
            PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping {
                operation_id: format!("create-{action_id}"),
                depends_on: Vec::new(),
                path: path.clone(),
            }),
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: format!("action-{action_id}"),
                depends_on: vec![format!("create-{action_id}")],
                path,
                action_id: action_id.to_string(),
                value_type: InputActionValueKind::Button,
            }),
        ]
    };
    AiToolInvocation {
        schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
        invocation_id: invocation_id.to_string(),
        tool_id: TOOL_ID_PROJECT_MUTATE.to_string(),
        expected_project_digest: ProjectCandidateEntry::inspect_project_binding(session)
            .unwrap()
            .project_digest,
        payload: AiToolInvocationPayload::ProjectMutationIntent(ExternalProjectMutationIntent {
            schema_version: EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION.to_string(),
            goal: ExternalProjectMutationGoal {
                outcome: outcome.to_string(),
            },
            change: ExternalProjectMutationChange::ProjectPatch(ProjectPatchDocument::new(
                format!("patch-{invocation_id}"),
                outcome,
                PatchSource::AiAssistant,
                operations,
            )),
        }),
    }
}

fn execute_goal_mutation(
    core: &mut GatewayCore,
    session: &mut EditorSession,
    binding: &ClientSessionBinding,
    invocation: AiToolInvocation,
) -> editor_core::AiToolAccepted {
    let reply = core.dispatch(
        session,
        bound_request(
            binding,
            &format!("request-{}", invocation.invocation_id),
            GatewayRequestPayload::ExecuteSessionBound { invocation },
        ),
    );
    let GatewayReplyPayload::Accepted(accepted) = reply.payload else {
        panic!(
            "project.mutate must return one operation: {:?}",
            reply.payload
        );
    };
    accepted
}

fn observe(
    core: &mut GatewayCore,
    session: &mut EditorSession,
    binding: &ClientSessionBinding,
    operation_id: &str,
) -> editor_core::AiToolOperationSnapshot {
    let reply = core.dispatch(
        session,
        bound_request(
            binding,
            &format!("observe-{operation_id}"),
            GatewayRequestPayload::Observe {
                operation_id: operation_id.to_string(),
            },
        ),
    );
    let GatewayReplyPayload::Operation(snapshot) = reply.payload else {
        panic!("observe must return operation: {:?}", reply.payload);
    };
    snapshot
}

fn cancel(
    core: &mut GatewayCore,
    session: &mut EditorSession,
    binding: &ClientSessionBinding,
    operation_id: &str,
) {
    let reply = core.dispatch(
        session,
        bound_request(
            binding,
            &format!("cancel-{operation_id}"),
            GatewayRequestPayload::CancelSessionBound {
                operation_id: operation_id.to_string(),
            },
        ),
    );
    assert!(matches!(
        reply.payload,
        GatewayReplyPayload::Cancellation(_)
    ));
}

fn created_project(label: &str) -> (EditorSession, PathBuf) {
    let root = unique_temp_root(label);
    let mut session = EditorSession::new();
    let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: format!("C3 {label}"),
    }));
    assert_eq!(created.status, CommandStatus::Committed);
    (session, root)
}

fn created_legacy_scene_project(label: &str) -> (EditorSession, PathBuf) {
    let (mut session, root) = created_project(label);
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/switch_puzzle_project/Scenes/Main.scene.json")
        .canonicalize()
        .unwrap();
    let target = session
        .active_project_session()
        .unwrap()
        .project_root
        .join("Scenes/Main.scene.json");
    std::fs::copy(source, &target).unwrap();
    let open = session.open_scene_document_for_test(&target);
    assert_eq!(open.status, CommandStatus::Committed);
    assert_eq!(session.scene_dirty(), Some(false));
    (session, root)
}

fn hello(client_version: &str) -> ClientHello {
    ClientHello {
        schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
        gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
        client_kind: ClientKind::Test,
        client_version: client_version.to_string(),
        supported_schema_versions: vec![AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
        expected_editor_instance_id: ai_tool_gateway::default_editor_instance_id(),
        requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
    }
}

fn bound_request(
    binding: &ClientSessionBinding,
    request_id: &str,
    payload: GatewayRequestPayload,
) -> GatewayRequest {
    GatewayRequest {
        schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
        gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
        request_id: request_id.to_string(),
        client_session_id: binding.client_session_id.clone(),
        deadline_epoch_ms: None,
        response_limit_bytes: 1024 * 1024,
        payload,
    }
}

fn unique_temp_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("gateway-c3-{label}-{}-{stamp}", std::process::id()))
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
