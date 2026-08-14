use super::*;
use ai_tool_gateway::{
    ClientKind, GatewayMutationAccessState, GatewayRemoteAdapter, GatewayReplyPayload,
    GatewayRequestPayload,
};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

#[cfg(windows)]
#[test]
fn gateway_context_transition_real_pipe_keeps_adapter_without_grant_inheritance() {
    let first_project_root = write_editor_project_fixture_for_shell()
        .canonicalize()
        .expect("canonical first project root");
    let second_project_root = write_editor_project_fixture_for_shell()
        .canonicalize()
        .expect("canonical second project root");
    let mut session = EditorSession::new();
    let opened = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: first_project_root.display().to_string(),
        },
    ));
    assert_eq!(opened.status, CommandStatus::Committed);

    let (wake_sender, wake_receiver) = mpsc::channel();
    let gateway_wake: ai_tool_gateway::GatewayOwnerThreadWake = std::sync::Arc::new(move || {
        let _ = wake_sender.send(());
    });
    let mut app =
        NativeEditorApplication::with_project_manager_and_dialog_initial_directory_and_gateway(
            NativeEditorWindowConfig::default(),
            session,
            ProjectManagerController::default(),
            Box::<HeadlessFolderDialogBackend>::default(),
            default_project_dialog_initial_directory(),
            Some(gateway_wake),
            None,
        );
    app.frame(1280.0, 720.0);
    let first_discovery = app
        .gateway_discovery_path()
        .expect("first project Gateway discovery")
        .to_path_buf();
    let first_adapter = connect_adapter(
        &mut app,
        &wake_receiver,
        &first_discovery,
        "r4-real-pipe-first.v1",
    );
    let first_session_id = first_adapter.binding().client_session_id.clone();

    let first_binding = first_adapter.binding().clone();
    let first_context = first_binding
        .project_context
        .as_ref()
        .expect("first project context");
    app.request_gateway_goal_mutation_access(
        &first_session_id,
        editor_core::AiGoalBinding::new(
            "gateway-reconnect-test",
            "Apply a bounded change before reconnecting to another project.",
            first_context.project_identity.clone(),
            first_context.project_digest.clone(),
            editor_core::AiGoalCompletionPolicy::CommitVerified,
        )
        .unwrap(),
        editor_core::AiRiskEnvelope::default_project_owned_low_risk().unwrap(),
    )
    .unwrap();

    let approval_request_id = app
        .latest_model()
        .ai_panel
        .gateway_access
        .requests
        .iter()
        .find(|request| request.client_session_id == first_session_id)
        .expect("first Adapter mutation approval request")
        .request_id
        .clone();
    let approved = app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::ApproveGatewayAccessRequest {
            request_id: approval_request_id,
        },
    ));
    assert_eq!(approved.status, CommandStatus::Committed);
    assert_eq!(
        app.last_gateway_access_decision_receipt()
            .expect("first Adapter approval receipt")
            .mutation_state,
        GatewayMutationAccessState::Active
    );

    let switched = app.dispatch_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: second_project_root.display().to_string(),
        },
    ));
    assert_eq!(switched.status, CommandStatus::Committed);

    let (started_sender, started_receiver) = mpsc::channel();
    let old_dispatch = std::thread::spawn(move || {
        let mut adapter = first_adapter;
        started_sender.send(()).unwrap();
        let reply = adapter.dispatch(GatewayRequestPayload::SessionStatus);
        (adapter, reply)
    });
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("old Adapter dispatch thread started");
    wake_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("old Adapter request reached the Editor owner queue");

    let switch_frame_started = Instant::now();
    app.frame(1280.0, 720.0);
    let switch_frame_elapsed = switch_frame_started.elapsed();
    assert!(
        switch_frame_elapsed < Duration::from_secs(2),
        "project switch frame blocked for {switch_frame_elapsed:?}"
    );
    let (same_adapter, status_reply) = old_dispatch.join().expect("same Adapter dispatch thread");
    let status_reply = status_reply.expect("same Adapter status after project switch");
    let GatewayReplyPayload::SessionStatus(status) = status_reply.payload else {
        panic!("same Adapter must receive SessionStatus: {status_reply:?}");
    };
    assert_eq!(status.session.id, first_session_id);
    assert_eq!(
        status.access.mutation.state,
        GatewayMutationAccessState::NotRequested
    );
    assert!(status.access.mutation.grant_digest.is_none());
    assert!(!same_adapter.restart_required());
    assert!(first_discovery.exists());

    let second_discovery = app
        .gateway_discovery_path()
        .expect("second project Gateway discovery")
        .to_path_buf();
    assert_eq!(first_discovery, second_discovery);

    close_adapter(&mut app, &wake_receiver, same_adapter);
    drop(app);
    assert!(!second_discovery.exists());
    let _ = std::fs::remove_dir_all(first_project_root);
    let _ = std::fs::remove_dir_all(second_project_root);
}

#[cfg(windows)]
fn connect_adapter(
    app: &mut NativeEditorApplication,
    wake_receiver: &Receiver<()>,
    discovery_path: &Path,
    version: &'static str,
) -> GatewayRemoteAdapter {
    let discovery_path = discovery_path.to_path_buf();
    let connect = std::thread::spawn(move || {
        GatewayRemoteAdapter::connect_from_discovery(&discovery_path, ClientKind::Test, version)
    });
    wake_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("Adapter connect reached the Editor owner queue");
    app.frame(1280.0, 720.0);
    connect
        .join()
        .expect("Adapter connect thread")
        .expect("connect Adapter through real Named Pipe")
}

#[cfg(windows)]
fn close_adapter(
    app: &mut NativeEditorApplication,
    wake_receiver: &Receiver<()>,
    adapter: GatewayRemoteAdapter,
) {
    let close = std::thread::spawn(move || adapter.close());
    wake_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("Adapter close reached the Editor owner queue");
    app.frame(1280.0, 720.0);
    close
        .join()
        .expect("Adapter close thread")
        .expect("close new Adapter");
}
