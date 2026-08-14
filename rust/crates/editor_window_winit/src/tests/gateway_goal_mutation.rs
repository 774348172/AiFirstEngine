use super::*;
use ai_tool_gateway::{
    ClientHello, ClientKind, GatewayReplyPayload, GatewayRequest, GatewayRequestPayload,
    GATEWAY_CLIENT_HELLO_SCHEMA_VERSION, GATEWAY_PROTOCOL_VERSION, GATEWAY_REQUEST_SCHEMA_VERSION,
};
use editor_core::{
    AiToolInvocation, AiToolInvocationPayload, AiToolOperationState, ExternalProjectMutationChange,
    ExternalProjectMutationGoal, ExternalProjectMutationIntent, InputPatchOperation,
    PatchOperation, PatchSource, ProjectPatchDocument, AI_TOOL_CATALOG_SCHEMA_VERSION,
    AI_TOOL_INVOCATION_SCHEMA_VERSION, EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION,
    TOOL_ID_PROJECT_MUTATE,
};
use editor_ui_model::InputActionValueKind;

#[test]
fn gateway_goal_mutation_uses_native_editor_approval_and_keeps_operation_identity() {
    let project_root = write_editor_project_fixture_for_shell();
    let session = opened_editor_project_session(&project_root);
    let mut app =
        NativeEditorApplication::with_session(NativeEditorWindowConfig::default(), session);
    let client = app.gateway_client();
    let connect = client
        .submit_connect(ClientHello {
            schema_version: GATEWAY_CLIENT_HELLO_SCHEMA_VERSION.to_string(),
            gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
            client_kind: ClientKind::Mcp,
            client_version: "native-editor-goal-mutation.v1".to_string(),
            supported_schema_versions: vec![AI_TOOL_CATALOG_SCHEMA_VERSION.to_string()],
            expected_editor_instance_id: app.editor_instance_id().to_string(),
            requested_read_scope: vec!["catalog".to_string(), "project".to_string()],
        })
        .unwrap();
    app.frame(1280.0, 720.0);
    let binding = connect.recv().unwrap().unwrap();
    let project_digest = binding
        .project_context
        .as_ref()
        .expect("opened project context")
        .project_digest
        .clone();
    let invocation = mutation_invocation(project_digest);
    let execute = client
        .submit_dispatch(request(
            &binding.client_session_id,
            "native-editor-goal-mutation-execute",
            GatewayRequestPayload::ExecuteSessionBound { invocation },
        ))
        .unwrap();

    app.frame(1280.0, 720.0);
    let reply = execute.recv().unwrap();
    let GatewayReplyPayload::Accepted(accepted) = reply.payload else {
        panic!("project.mutate must await Native Editor approval: {reply:?}");
    };
    assert_eq!(accepted.state, AiToolOperationState::AwaitingUser);
    let access_request = app
        .latest_model()
        .ai_panel
        .gateway_access
        .requests
        .first()
        .expect("Native Editor approval Inbox request");
    assert_eq!(
        access_request.operation_short_id,
        short_id(&accepted.operation_id)
    );
    assert_eq!(access_request.risk_class, "ProjectOwnedLowRisk");
    let access_request_id = access_request.request_id.clone();

    let approved = app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::ApproveGatewayAccessRequest {
            request_id: access_request_id,
        },
    ));
    assert_eq!(approved.status, CommandStatus::Committed);
    for _ in 0..8 {
        app.frame(1280.0, 720.0);
    }

    let observe = client
        .submit_dispatch(request(
            &binding.client_session_id,
            "native-editor-goal-mutation-observe",
            GatewayRequestPayload::Observe {
                operation_id: accepted.operation_id.clone(),
            },
        ))
        .unwrap();
    app.frame(1280.0, 720.0);
    let observed = observe.recv().unwrap();
    let GatewayReplyPayload::Operation(snapshot) = observed.payload else {
        panic!("Gateway observe must return the approved operation: {observed:?}");
    };
    assert_eq!(snapshot.operation_id, accepted.operation_id);
    assert_eq!(snapshot.state, AiToolOperationState::Completed);
    assert_eq!(
        snapshot
            .transitions
            .first()
            .map(|transition| transition.state),
        Some(AiToolOperationState::AwaitingUser)
    );
    let _ = std::fs::remove_dir_all(project_root);
}

fn mutation_invocation(project_digest: String) -> AiToolInvocation {
    let path = "Input/native-editor-goal-mutation.input.json".to_string();
    AiToolInvocation {
        schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
        invocation_id: "native-editor-goal-mutation".to_string(),
        tool_id: TOOL_ID_PROJECT_MUTATE.to_string(),
        expected_project_digest: project_digest,
        payload: AiToolInvocationPayload::ProjectMutationIntent(ExternalProjectMutationIntent {
            schema_version: EXTERNAL_PROJECT_MUTATION_INTENT_SCHEMA_VERSION.to_string(),
            goal: ExternalProjectMutationGoal {
                outcome: "Add a bounded Native Editor input action.".to_string(),
            },
            change: ExternalProjectMutationChange::ProjectPatch(ProjectPatchDocument::new(
                "native-editor-goal-mutation-patch",
                "Add a bounded Native Editor input action.",
                PatchSource::AiAssistant,
                vec![
                    PatchOperation::Input(InputPatchOperation::CreateDefaultInputMapping {
                        operation_id: "create-native-editor-mapping".to_string(),
                        depends_on: Vec::new(),
                        path: path.clone(),
                    }),
                    PatchOperation::Input(InputPatchOperation::AddInputAction {
                        operation_id: "add-native-editor-action".to_string(),
                        depends_on: vec!["create-native-editor-mapping".to_string()],
                        path,
                        action_id: "native-editor-action".to_string(),
                        value_type: InputActionValueKind::Button,
                    }),
                ],
            )),
        }),
    }
}

fn request(
    client_session_id: &str,
    request_id: &str,
    payload: GatewayRequestPayload,
) -> GatewayRequest {
    GatewayRequest {
        schema_version: GATEWAY_REQUEST_SCHEMA_VERSION.to_string(),
        gateway_protocol_version: GATEWAY_PROTOCOL_VERSION.to_string(),
        request_id: request_id.to_string(),
        client_session_id: client_session_id.to_string(),
        deadline_epoch_ms: None,
        response_limit_bytes: 1024 * 1024,
        payload,
    }
}

fn short_id(id: &str) -> String {
    let chars = id.chars().collect::<Vec<_>>();
    chars[chars.len().saturating_sub(10)..].iter().collect()
}
