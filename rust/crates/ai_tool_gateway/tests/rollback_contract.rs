use ai_tool_gateway::{
    ClientHello, ClientKind, ClientSessionBinding, GatewayAccessDecision, GatewayCore,
    GatewayReplyPayload, GatewayRequest, GatewayRequestPayload,
    GATEWAY_CLIENT_HELLO_SCHEMA_VERSION, GATEWAY_PROTOCOL_VERSION, GATEWAY_REQUEST_SCHEMA_VERSION,
};
use editor_core::{
    command_for_test, AiToolContractRegistry, AiToolInvocation, AiToolInvocationPayload,
    AiToolOperationState, CommandStatus, EditorSession, ExternalProjectMutationChange,
    ExternalProjectMutationGoal, ExternalProjectMutationIntent, InputPatchOperation,
    PatchOperation, PatchSource, ProjectCandidateEntry, ProjectPatchDocument,
    AI_TOOL_CATALOG_SCHEMA_VERSION, AI_TOOL_INVOCATION_SCHEMA_VERSION,
    EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION, EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
    TOOL_ID_PROJECT_MUTATE, TOOL_ID_PROJECT_ROLLBACK,
};
use editor_ui_model::{InputActionValueKind, UiCommandPayload};
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn rollback_contract_exposes_only_opaque_ref_and_retires_candidate_tools() {
    let registry = AiToolContractRegistry::new();
    let descriptor = registry
        .descriptor(TOOL_ID_PROJECT_ROLLBACK)
        .expect("project.rollback descriptor");
    assert_eq!(
        descriptor.input_schema["required"],
        json!(["schemaVersion", "rollbackRef"])
    );
    assert_eq!(descriptor.input_schema["additionalProperties"], false);
    assert_eq!(
        descriptor.input_schema["properties"]["schemaVersion"]["const"],
        EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION
    );
    assert!(registry
        .decode_invocation_payload(
            TOOL_ID_PROJECT_ROLLBACK,
            json!({
                "schemaVersion": EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
                "rollbackRef": "rbk_12345678"
            }),
        )
        .is_ok());
    assert!(registry
        .decode_invocation_payload(
            TOOL_ID_PROJECT_ROLLBACK,
            json!({
                "schemaVersion": EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
                "rollbackRef": "rbk_12345678",
                "receipt": {}
            }),
        )
        .is_err());

    let retired_mutate = ["project", "mutate", "candidate"].join(".");
    let retired_rollback = ["project", "rollback", "candidate"].join(".");
    assert!(registry.descriptor(&retired_mutate).is_none());
    assert!(registry.descriptor(&retired_rollback).is_none());
}

#[test]
fn rollback_ref_restores_initial_digest_and_is_consumed_once() {
    let (mut core, mut session, binding, root, initial_digest, rollback_ref) =
        completed_mutation("rollback-success");

    let accepted = execute_rollback(
        &mut core,
        &mut session,
        &binding,
        "rollback-success",
        &rollback_ref,
    );
    assert!(core.pump_operations(&mut session, 8) > 0);
    let operation = observe(&mut core, &mut session, &binding, &accepted.operation_id);
    assert_eq!(operation.state, AiToolOperationState::Completed);
    assert_eq!(
        operation.result.as_ref().unwrap().tool_id,
        TOOL_ID_PROJECT_ROLLBACK
    );
    assert_eq!(
        ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_digest,
        initial_digest
    );

    let replay = dispatch_rollback(
        &mut core,
        &mut session,
        &binding,
        "rollback-replay",
        &rollback_ref,
    );
    let GatewayReplyPayload::Rejected(diagnostic) = replay.payload else {
        panic!(
            "consumed rollbackRef must be rejected: {:?}",
            replay.payload
        );
    };
    assert_eq!(diagnostic.code, "gateway.rollback_ref.consumed");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rollback_ref_rejects_unknown_session_and_project_drift() {
    let (mut core, mut session, binding, root, _, rollback_ref) =
        completed_mutation("rollback-negative");

    let unknown = dispatch_rollback(
        &mut core,
        &mut session,
        &binding,
        "rollback-unknown",
        "rbk_00000000",
    );
    assert_rejected(unknown, "gateway.rollback_ref.unknown");

    let second = core
        .connect(&mut session, hello("rollback-second-session.v1"))
        .unwrap();
    let wrong_session = dispatch_rollback(
        &mut core,
        &mut session,
        &second,
        "rollback-wrong-session",
        &rollback_ref,
    );
    assert_rejected(wrong_session, "gateway.rollback_ref.session_mismatch");

    std::fs::write(root.join("external-drift.txt"), "later user change").unwrap();
    let drifted = dispatch_rollback(
        &mut core,
        &mut session,
        &binding,
        "rollback-drifted",
        &rollback_ref,
    );
    assert_rejected(drifted, "gateway.rollback_ref.project_drifted");
    let _ = std::fs::remove_dir_all(root);
}

fn completed_mutation(
    label: &str,
) -> (
    GatewayCore,
    EditorSession,
    ClientSessionBinding,
    PathBuf,
    String,
    String,
) {
    let root = unique_temp_root(label);
    let mut session = EditorSession::new();
    let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: format!("C4 {label}"),
    }));
    assert_eq!(created.status, CommandStatus::Committed);
    let initial_digest = ProjectCandidateEntry::inspect_project_binding(&session)
        .unwrap()
        .project_digest;
    let mut core = GatewayCore::new();
    let binding = core.connect(&mut session, hello(label)).unwrap();
    let invocation = mutation_invocation(&session, label);
    let reply = core.dispatch(
        &mut session,
        request(
            &binding,
            &format!("{label}-mutate"),
            GatewayRequestPayload::ExecuteSessionBound { invocation },
        ),
    );
    let GatewayReplyPayload::Accepted(accepted) = reply.payload else {
        panic!("project.mutate must await approval: {:?}", reply.payload);
    };
    let approval = core
        .approval_inbox(now_epoch_ms())
        .into_iter()
        .find(|request| request.operation_id.as_deref() == Some(&accepted.operation_id))
        .expect("same-operation approval");
    core.decide_access(
        &session,
        &approval.request_id,
        GatewayAccessDecision::Approve,
        "native-editor-user",
        now_epoch_ms(),
    )
    .unwrap();
    assert!(core.pump_operations(&mut session, 8) > 0);
    let operation = observe(&mut core, &mut session, &binding, &accepted.operation_id);
    assert_eq!(operation.state, AiToolOperationState::Completed);
    let rollback_ref = operation
        .result
        .as_ref()
        .and_then(|result| result.rollback_ref.clone())
        .expect("completed mutation rollbackRef");
    assert!(rollback_ref.starts_with("rbk_"));
    (core, session, binding, root, initial_digest, rollback_ref)
}

fn mutation_invocation(session: &EditorSession, label: &str) -> AiToolInvocation {
    let path = format!("Input/{label}.input.json");
    let patch = ProjectPatchDocument::new(
        format!("patch-{label}"),
        "Add one bounded input action.",
        PatchSource::AiAssistant,
        vec![
            PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping {
                operation_id: format!("create-{label}"),
                depends_on: Vec::new(),
                path: path.clone(),
            }),
            PatchOperation::Input(InputPatchOperation::AddInputAction {
                operation_id: format!("action-{label}"),
                depends_on: vec![format!("create-{label}")],
                path,
                action_id: format!("action-{label}"),
                value_type: InputActionValueKind::Button,
            }),
        ],
    );
    AiToolInvocation {
        schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
        invocation_id: format!("mutation-{label}"),
        tool_id: TOOL_ID_PROJECT_MUTATE.to_string(),
        expected_project_digest: ProjectCandidateEntry::inspect_project_binding(session)
            .unwrap()
            .project_digest,
        payload: AiToolInvocationPayload::ProjectMutationIntent(ExternalProjectMutationIntent {
            schema_version: EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION.to_string(),
            goal: ExternalProjectMutationGoal {
                outcome: "Add one bounded input action.".to_string(),
            },
            change: ExternalProjectMutationChange::ProjectPatch(patch),
        }),
    }
}

fn execute_rollback(
    core: &mut GatewayCore,
    session: &mut EditorSession,
    binding: &ClientSessionBinding,
    invocation_id: &str,
    rollback_ref: &str,
) -> editor_core::AiToolAccepted {
    let reply = dispatch_rollback(core, session, binding, invocation_id, rollback_ref);
    let GatewayReplyPayload::Accepted(accepted) = reply.payload else {
        panic!(
            "project.rollback must start one operation: {:?}",
            reply.payload
        );
    };
    accepted
}

fn dispatch_rollback(
    core: &mut GatewayCore,
    session: &mut EditorSession,
    binding: &ClientSessionBinding,
    invocation_id: &str,
    rollback_ref: &str,
) -> ai_tool_gateway::GatewayReply {
    let payload = AiToolContractRegistry::new()
        .decode_invocation_payload(
            TOOL_ID_PROJECT_ROLLBACK,
            json!({
                "schemaVersion": EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION,
                "rollbackRef": rollback_ref
            }),
        )
        .unwrap();
    core.dispatch(
        session,
        request(
            binding,
            invocation_id,
            GatewayRequestPayload::ExecuteSessionBound {
                invocation: AiToolInvocation {
                    schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                    invocation_id: invocation_id.to_string(),
                    tool_id: TOOL_ID_PROJECT_ROLLBACK.to_string(),
                    expected_project_digest: ProjectCandidateEntry::inspect_project_binding(
                        session,
                    )
                    .unwrap()
                    .project_digest,
                    payload,
                },
            },
        ),
    )
}

fn observe(
    core: &mut GatewayCore,
    session: &mut EditorSession,
    binding: &ClientSessionBinding,
    operation_id: &str,
) -> editor_core::AiToolOperationSnapshot {
    let reply = core.dispatch(
        session,
        request(
            binding,
            &format!("observe-{operation_id}"),
            GatewayRequestPayload::Observe {
                operation_id: operation_id.to_string(),
            },
        ),
    );
    let GatewayReplyPayload::Operation(operation) = reply.payload else {
        panic!("observe must return operation: {:?}", reply.payload);
    };
    operation
}

fn request(
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

fn hello(version: &str) -> ClientHello {
    ClientHello {
        schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
        gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
        client_kind: ClientKind::Test,
        client_version: version.to_string(),
        supported_schema_versions: vec![AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
        expected_editor_instance_id: ai_tool_gateway::default_editor_instance_id(),
        requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
    }
}

fn assert_rejected(reply: ai_tool_gateway::GatewayReply, code: &str) {
    let GatewayReplyPayload::Rejected(diagnostic) = reply.payload else {
        panic!("expected {code}, got {:?}", reply.payload);
    };
    assert_eq!(diagnostic.code, code);
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn unique_temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aife-c4-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
