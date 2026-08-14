use ai_tool_gateway::{
    ClientHello, ClientKind, ClientSessionBinding, GatewayAccessDecision, GatewayCore,
    GatewayMutationAccessState, GatewayReplyPayload, GatewayRequest, GatewayRequestPayload,
    GATEWAY_CLIENT_HELLO_SCHEMA_VERSION, GATEWAY_PROTOCOL_VERSION, GATEWAY_REQUEST_SCHEMA_VERSION,
};
use editor_core::{
    command_for_test, AiGoalBinding, AiGoalCompletionPolicy, AiRiskEnvelope, CommandStatus,
    EditorSession, AI_TOOL_CATALOG_SCHEMA_VERSION,
};
use editor_ui_model::UiCommandPayload;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn gateway_context_transition_keeps_session_and_invalidates_project_authority() {
    let (mut session, first_root) = created_project("Reconnect First", "first");
    let (_second_session, second_root) = created_project("Reconnect Second", "second");
    let mut core = GatewayCore::new();

    let first_hello = hello(&session, "fixed-adapter-before-switch.v1");
    let first_binding = core
        .connect(&mut session, first_hello)
        .expect("connect fixed adapter to first project");
    request_test_goal(&mut core, &session, &first_binding);
    let access_request = core
        .approval_inbox(now_epoch_ms())
        .into_iter()
        .find(|request| request.client_session_id == first_binding.client_session_id)
        .expect("first session mutation approval request");
    let approved = core
        .decide_access(
            &session,
            &access_request.request_id,
            GatewayAccessDecision::Approve,
            "native-editor-user",
            now_epoch_ms(),
        )
        .expect("approve first session mutation grant");
    let old_grant_ref = approved.grant_ref.expect("approved mutation grant ref");
    assert!(core.client_has_active_grant(&first_binding.client_session_id));

    let switched = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: second_root.display().to_string(),
    }));
    assert_eq!(switched.status, CommandStatus::Committed);

    let old_status = core.dispatch(
        &mut session,
        bound_request(
            &first_binding,
            "old-status-after-switch",
            GatewayRequestPayload::SessionStatus,
        ),
    );
    let GatewayReplyPayload::SessionStatus(new_status) = old_status.payload else {
        panic!("same Gateway session must survive project switch");
    };
    assert_eq!(new_status.session.id, first_binding.client_session_id);
    assert_eq!(
        new_status
            .project
            .as_ref()
            .expect("new project context")
            .identity,
        editor_core::ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_id
    );
    assert_eq!(core.active_client_bindings().len(), 1);
    assert!(!core.client_has_active_grant(&first_binding.client_session_id));
    assert_eq!(
        core.revoke_grant_ref(&old_grant_ref)
            .expect_err("project switch must remove the old mutation grant")
            .code,
        "gateway.grant_ref.unknown"
    );

    let current_read = core.dispatch(
        &mut session,
        bound_request(
            &first_binding,
            "current-read-after-switch",
            GatewayRequestPayload::Catalog(editor_core::AiToolCatalogRequest::default()),
        ),
    );
    assert!(matches!(
        current_read.payload,
        GatewayReplyPayload::Catalog(_)
    ));
    assert_eq!(
        new_status.access.mutation.state,
        GatewayMutationAccessState::NotRequested
    );
    assert!(new_status.access.mutation.grant_digest.is_none());
    assert!(!new_status.reconnect_required);

    drop(session);
    let _ = std::fs::remove_dir_all(first_root);
    let _ = std::fs::remove_dir_all(second_root);
}

fn request_test_goal(
    core: &mut GatewayCore,
    session: &EditorSession,
    binding: &ClientSessionBinding,
) {
    let project = binding
        .project_context
        .as_ref()
        .expect("goal requires project context");
    let goal = AiGoalBinding::new(
        "explicit-reconnect-goal",
        "Apply a bounded project change before reconnect validation.",
        project.project_identity.clone(),
        project.project_digest.clone(),
        AiGoalCompletionPolicy::CommitVerified,
    )
    .unwrap();
    core.request_goal_mutation_access(
        session,
        &binding.client_session_id,
        goal,
        AiRiskEnvelope::default_project_owned_low_risk().unwrap(),
    )
    .unwrap();
}

fn created_project(name: &str, label: &str) -> (EditorSession, PathBuf) {
    let root = unique_temp_root(label);
    let mut session = EditorSession::new();
    let created = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: name.to_string(),
    }));
    assert_eq!(created.status, CommandStatus::Committed);
    (session, root)
}

fn hello(_session: &EditorSession, client_version: &str) -> ClientHello {
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
        .expect("system time after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "gateway-explicit-reconnect-{label}-{}-{stamp}",
        std::process::id()
    ))
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_millis() as u64
}
