use super::*;
use engine_runtime::aui::{AuiActionRef, AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect};
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};

fn created_preview_session(name: &str) -> (EditorSession, PathBuf) {
    let root = fixtures::unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: name.to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    (session, root)
}

fn preview_invocation(
    session: &EditorSession,
    invocation_id: &str,
    tool_id: &str,
    payload: AiToolInvocationPayload,
) -> (AiToolInvocation, AiCapabilityGrant) {
    let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
    let grant = AiCapabilityGrant::read(
        format!("grant-{invocation_id}"),
        binding.project_id,
        binding.project_digest.clone(),
        "local-user",
    )
    .unwrap();
    (
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: invocation_id.to_string(),
            tool_id: tool_id.to_string(),
            expected_project_digest: binding.project_digest,
            payload,
        },
        grant,
    )
}

fn start_preview_awaiting_frame(
    session: &mut EditorSession,
    invocation_id: &str,
) -> (
    AiCapabilityToolKernel,
    AiToolAccepted,
    ProjectPreviewFrameTicket,
    AiCapabilityGrant,
) {
    let (invocation, grant) = preview_invocation(
        session,
        invocation_id,
        TOOL_ID_PROJECT_PREVIEW,
        AiToolInvocationPayload::Preview,
    );
    let mut kernel = AiCapabilityToolKernel::new();
    let AiToolStartOutcome::Accepted(accepted) = kernel.start(session, invocation, &grant) else {
        panic!("Preview must enter the asynchronous operation path");
    };
    assert_eq!(kernel.pump_operations(session, 3), 3);
    let operation = kernel.observe(&accepted.operation_id).unwrap();
    assert_eq!(operation.state, AiToolOperationState::Running);
    assert_eq!(operation.stage, "awaiting_frame_evidence");
    assert!(operation.result.is_none());
    let ticket = session
        .pending_project_preview_frame_ticket()
        .cloned()
        .expect("Preview must retain an exact-frame ticket");
    assert_eq!(ticket.operation_id, accepted.operation_id);
    (kernel, accepted, ticket, grant)
}

fn deterministic_readback(
    session: &EditorSession,
    ticket: &ProjectPreviewFrameTicket,
) -> ProjectPreviewFrameReadback {
    let frame = session
        .last_game_view_runtime_frame()
        .expect("Preview runtime frame");
    ProjectPreviewFrameReadback {
        game_view_session_id: ticket.game_view_session_id.clone(),
        texture_id: ticket.expected_texture_id.clone(),
        frame_index: ticket.expected_frame_index,
        width: frame.width.max(1),
        height: frame.height.max(1),
        pixel_format: ProjectPreviewPixelFormat::Rgba8Unorm,
        capture_kind: ProjectPreviewCaptureKind::DeterministicTestAdapter,
        rgba8: vec![0; (frame.width.max(1) * frame.height.max(1) * 4) as usize],
    }
}

fn replace_frame_and_rehash(
    root: &Path,
    evidence_ref: &str,
    mut evidence: ProjectPreviewFrameEvidence,
) -> ProjectPreviewFrameEvidence {
    let rgba8 = vec![0xff; (evidence.width * evidence.height * 4) as usize];
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, evidence.width, evidence.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&rgba8).unwrap();
    }
    evidence.frame_digest = sha256_prefixed(&rgba8);
    evidence.screenshot_digest = sha256_prefixed(&png_bytes);
    evidence.evidence_digest.clear();
    let canonical = canonical_json_bytes(&serde_json::to_value(&evidence).unwrap()).unwrap();
    evidence.evidence_digest = sha256_prefixed(&canonical);
    std::fs::write(root.join(&evidence.screenshot_ref), png_bytes).unwrap();
    std::fs::write(
        root.join(evidence_ref),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    evidence
}

fn terminal_diagnostic_code(kernel: &AiCapabilityToolKernel, operation_id: &str) -> String {
    kernel
        .observe(operation_id)
        .unwrap()
        .result
        .expect("terminal operation result")
        .diagnostics
        .first()
        .expect("terminal diagnostic")
        .code
        .clone()
}

#[test]
fn preview_frame_evidence_awaits_receipt_then_completes_from_deterministic_readback() {
    let (mut session, root) = created_preview_session("PreviewAwaitingFrame");
    let (mut kernel, accepted, ticket, _) =
        start_preview_awaiting_frame(&mut session, "preview-awaiting-frame");

    let evidence = session
        .record_project_preview_presented_frame(deterministic_readback(&session, &ticket))
        .expect("record deterministic exact-frame receipt");
    assert_eq!(evidence.operation_id, accepted.operation_id);
    assert_eq!(evidence.frame_index, ticket.expected_frame_index);
    assert_eq!(
        evidence.capture_kind,
        ProjectPreviewCaptureKind::DeterministicTestAdapter
    );

    assert_eq!(kernel.pump_operations(&mut session, 1), 1);
    let operation = kernel.observe(&accepted.operation_id).unwrap();
    assert_eq!(operation.state, AiToolOperationState::Completed);
    let result = operation.result.expect("completed Preview result");
    assert_eq!(result.status, AiToolExecutionStatus::Completed);
    let Some(AiToolOutput::Preview(output)) = result.output else {
        panic!("Preview output must include exact frame evidence");
    };
    assert_eq!(output.frame_evidence_digest, evidence.evidence_digest);
    assert_eq!(output.screenshot_digest, evidence.screenshot_digest);
    assert_eq!(output.frame_digest, evidence.frame_digest);
    assert!(session.pending_project_preview_frame_ticket().is_none());
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_rejects_frame_index_mismatch() {
    let (mut session, root) = created_preview_session("PreviewFrameMismatch");
    let (mut kernel, accepted, ticket, _) =
        start_preview_awaiting_frame(&mut session, "preview-frame-mismatch");
    let mut readback = deterministic_readback(&session, &ticket);
    readback.frame_index += 1;

    let error = session
        .record_project_preview_presented_frame(readback)
        .expect_err("mismatched frame index must fail");
    assert_eq!(error.code, "project_preview_evidence.frame_index_mismatch");
    kernel.pump_operations(&mut session, 1);
    assert_eq!(
        terminal_diagnostic_code(&kernel, &accepted.operation_id),
        "project_preview_evidence.frame_index_mismatch"
    );
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_rejects_cross_operation_result() {
    let (mut session, root) = created_preview_session("PreviewOperationMismatch");
    let (mut kernel, accepted, ticket, _) =
        start_preview_awaiting_frame(&mut session, "preview-operation-mismatch");
    session
        .record_project_preview_presented_frame(deterministic_readback(&session, &ticket))
        .unwrap();
    let mut mismatched = session.project_preview_frame_result().cloned().unwrap();
    mismatched.operation_id = "different-preview-operation".to_string();
    session.project_preview_frame_result = Some(mismatched);

    kernel.pump_operations(&mut session, 1);
    assert_eq!(
        terminal_diagnostic_code(&kernel, &accepted.operation_id),
        "ai_tool.preview_frame_result_operation_mismatch"
    );
    assert!(session.pending_project_preview_frame_ticket().is_none());
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_rejects_project_drift_while_awaiting() {
    let (mut session, root) = created_preview_session("PreviewProjectDrift");
    let (mut kernel, accepted, _, _) =
        start_preview_awaiting_frame(&mut session, "preview-project-drift");
    std::fs::write(root.join("Assets/preview-drift.txt"), "drift").unwrap();

    kernel.pump_operations(&mut session, 1);
    assert_eq!(
        terminal_diagnostic_code(&kernel, &accepted.operation_id),
        "ai_tool.preview_project_drifted_while_awaiting_frame"
    );
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_rejects_runtime_frame_hash_mismatch() {
    let (mut session, root) = created_preview_session("PreviewHashMismatch");
    let (mut kernel, accepted, ticket, _) =
        start_preview_awaiting_frame(&mut session, "preview-hash-mismatch");
    let evidence = session
        .record_project_preview_presented_frame(deterministic_readback(&session, &ticket))
        .unwrap();
    let evidence_path =
        root.join(&ProjectPreviewEvidence::frame_evidence_ref(&accepted.operation_id).unwrap());
    let mut tampered = evidence;
    tampered.runtime_frame_hash = "sha256:wrong-runtime-frame".to_string();
    tampered.evidence_digest.clear();
    let canonical = canonical_json_bytes(&serde_json::to_value(&tampered).unwrap()).unwrap();
    tampered.evidence_digest = sha256_prefixed(&canonical);
    std::fs::write(evidence_path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();

    kernel.pump_operations(&mut session, 1);
    assert_eq!(
        terminal_diagnostic_code(&kernel, &accepted.operation_id),
        "project_preview_evidence.runtime_frame_hash_mismatch"
    );
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_rejects_forged_rehash_against_capture_receipt() {
    let (mut session, root) = created_preview_session("PreviewForgedRehash");
    let (mut kernel, accepted, ticket, _) =
        start_preview_awaiting_frame(&mut session, "preview-forged-rehash");
    let evidence = session
        .record_project_preview_presented_frame(deterministic_readback(&session, &ticket))
        .unwrap();
    let evidence_ref = ProjectPreviewEvidence::frame_evidence_ref(&accepted.operation_id).unwrap();
    let forged = replace_frame_and_rehash(&root, &evidence_ref, evidence.clone());
    assert_ne!(forged.frame_digest, evidence.frame_digest);
    ProjectPreviewEvidence::validate_frame(
        session.active_project_session().unwrap().write_scope(),
        &ticket,
        &evidence_ref,
    )
    .expect("forged artifact remains internally self-consistent");

    kernel.pump_operations(&mut session, 1);

    assert_eq!(
        terminal_diagnostic_code(&kernel, &accepted.operation_id),
        "ai_tool.preview_frame_receipt_mismatch"
    );
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_cancel_after_receipt_discards_trusted_result() {
    let (mut session, root) = created_preview_session("PreviewCancelAfterReceipt");
    let (mut kernel, accepted, ticket, grant) =
        start_preview_awaiting_frame(&mut session, "preview-cancel-after-receipt");
    session
        .record_project_preview_presented_frame(deterministic_readback(&session, &ticket))
        .unwrap();
    assert!(session.project_preview_frame_result().is_some());

    kernel.cancel(&accepted.operation_id, &grant).unwrap();
    kernel.pump_operations(&mut session, 1);

    assert!(session.pending_project_preview_frame_ticket().is_none());
    assert!(session.project_preview_frame_result().is_none());
    assert_eq!(
        terminal_diagnostic_code(&kernel, &accepted.operation_id),
        "ai_tool.operation_cancelled"
    );
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_dirty_scene_fails_without_ticket() {
    let (mut session, root) = created_preview_session("PreviewDirtyScene");
    session
        .editor_scene_document
        .as_mut()
        .expect("active Scene")
        .dirty_state
        .dirty = true;
    let (invocation, grant) = preview_invocation(
        &session,
        "preview-dirty-scene",
        TOOL_ID_PROJECT_PREVIEW,
        AiToolInvocationPayload::Preview,
    );
    let mut kernel = AiCapabilityToolKernel::new();
    let AiToolStartOutcome::Accepted(accepted) = kernel.start(&session, invocation, &grant) else {
        panic!("Preview must be accepted before prepare");
    };

    kernel.pump_operations(&mut session, 3);
    assert_eq!(
        terminal_diagnostic_code(&kernel, &accepted.operation_id),
        "ai_tool.preview_dirty_scene_requires_save"
    );
    assert!(session.pending_project_preview_frame_ticket().is_none());
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_frame_evidence_direct_execute_fails_closed_without_ticket() {
    let (mut session, root) = created_preview_session("PreviewDirectExecute");
    let (invocation, grant) = preview_invocation(
        &session,
        "preview-direct-execute",
        TOOL_ID_PROJECT_PREVIEW,
        AiToolInvocationPayload::Preview,
    );

    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);

    assert_eq!(result.status, AiToolExecutionStatus::Failed);
    assert_eq!(
        result.diagnostics[0].code,
        "ai_tool.preview_async_execution_required"
    );
    assert!(session.pending_project_preview_frame_ticket().is_none());
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

fn write_hidden_button_fixture(root: &Path) {
    std::fs::create_dir_all(root.join("AUI")).unwrap();
    std::fs::create_dir_all(root.join("RuntimeModule/src")).unwrap();
    let root_node = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["start-button"]);
    let mut button = AuiNode::new(
        "start-button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(40.0, 40.0, 220.0, 64.0),
    )
    .with_parent("root")
    .with_text("Start Game")
    .with_action(AuiActionRef::click("menu.start_game"));
    button.name = "Primary Start Button".to_string();
    button.visible = false;
    let document = AuiDocument::new(
        "main-menu",
        vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
        vec![root_node, button],
    );
    std::fs::write(
        root.join("AUI/main-menu.aui.json"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("RuntimeModule/src/menu.rs"),
        "pub fn start_game() {}\n",
    )
    .unwrap();
}

fn retained_visual_frame(session: &mut EditorSession, operation_id: &str) -> String {
    let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
    let ticket = ProjectPreviewFrameTicket {
        schema_version: PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION.to_string(),
        operation_id: operation_id.to_string(),
        project_identity: binding.project_id,
        expected_project_digest: binding.project_digest.clone(),
        game_view_session_id: "visual-game-view".to_string(),
        expected_texture_id: "visual-texture".to_string(),
        expected_frame_index: 7,
        expected_runtime_frame_hash: "visual-runtime-frame".to_string(),
    };
    let evidence = ProjectPreviewEvidence::persist_frame(
        session.active_project_session().unwrap().write_scope(),
        &ticket,
        ProjectPreviewFrameCapture {
            project_digest: binding.project_digest,
            game_view_session_id: ticket.game_view_session_id.clone(),
            texture_id: ticket.expected_texture_id.clone(),
            frame_index: ticket.expected_frame_index,
            runtime_frame_hash: ticket.expected_runtime_frame_hash.clone(),
            width: 2,
            height: 2,
            pixel_format: ProjectPreviewPixelFormat::Rgba8Unorm,
            capture_kind: ProjectPreviewCaptureKind::DeterministicTestAdapter,
            present_report_ref: "Library/Reports/visual-present.json".to_string(),
            rgba8: vec![0; 16],
        },
    )
    .unwrap();
    let evidence_ref = ProjectPreviewEvidence::frame_evidence_ref(operation_id).unwrap();
    session.project_preview_frame_result = Some(ProjectPreviewFrameResult::captured(
        &evidence_ref,
        evidence.clone(),
    ));
    assert_eq!(evidence.operation_id, operation_id);
    evidence_ref
}

#[test]
fn visual_evidence_ref_typed_capture_locate_explain_trace_chain() {
    let (mut session, root) = created_preview_session("VisualEvidenceChain");
    write_hidden_button_fixture(&root);
    let frame_evidence_ref = retained_visual_frame(&mut session, "visual-chain-frame");

    let (capture, grant) = preview_invocation(
        &session,
        "visual-chain-capture",
        TOOL_ID_RUNTIME_CAPTURE_ISSUE,
        AiToolInvocationPayload::RuntimeCaptureIssue(ProjectRuntimeCaptureIssueInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            frame_evidence_ref,
            symptom: Some("Start Game is hidden".to_string()),
        }),
    );
    let capture = AiCapabilityToolKernel::new().execute(&mut session, capture, &grant);
    let Some(AiToolOutput::VisualIssueCaptured(captured)) = capture.output else {
        panic!("capture must return an operation-owned issue bundle");
    };

    let issue_bundle_ref = captured.issue_bundle_ref;
    let (locate, grant) = preview_invocation(
        &session,
        "visual-chain-locate",
        TOOL_ID_UI_LOCATE,
        AiToolInvocationPayload::UiLocate(ProjectUiLocateInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            query: "start game".to_string(),
            issue_bundle_ref: Some(issue_bundle_ref.clone()),
        }),
    );
    let locate = AiCapabilityToolKernel::new().execute(&mut session, locate, &grant);
    let Some(AiToolOutput::UiLocated(located)) = locate.output else {
        panic!("locate must consume the captured issue bundle");
    };
    assert_eq!(located.candidates[0].node_id, "start-button");

    let (explain, grant) = preview_invocation(
        &session,
        "visual-chain-explain",
        TOOL_ID_UI_EXPLAIN_VISIBILITY,
        AiToolInvocationPayload::UiExplainVisibility(ProjectUiExplainInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            document_path: "AUI/main-menu.aui.json".to_string(),
            node_id: "start-button".to_string(),
            issue_bundle_ref: issue_bundle_ref.clone(),
        }),
    );
    let explain = AiCapabilityToolKernel::new().execute(&mut session, explain, &grant);
    let Some(AiToolOutput::VisualIssue(explained)) = explain.output else {
        panic!("explain must consume the captured issue bundle");
    };
    assert_eq!(explained.node.first_failure_stage, "authored_visibility");

    let (trace, grant) = preview_invocation(
        &session,
        "visual-chain-trace",
        TOOL_ID_PROJECT_TRACE_UI_OWNER,
        AiToolInvocationPayload::ProjectTraceUiOwner(ProjectUiOwnerTraceInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            document_path: "AUI/main-menu.aui.json".to_string(),
            node_id: "start-button".to_string(),
            issue_bundle_ref: Some(issue_bundle_ref),
        }),
    );
    let trace = AiCapabilityToolKernel::new().execute(&mut session, trace, &grant);
    let Some(AiToolOutput::UiOwnerTrace(trace)) = trace.output else {
        panic!("trace must consume the captured issue bundle");
    };
    assert!(trace
        .project_source_symbols
        .contains(&"RuntimeModule/src/menu.rs::start_game".to_string()));
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn visual_evidence_ref_rejects_stale_retained_frame_ref() {
    let (mut session, root) = created_preview_session("VisualStaleRetainedFrame");
    let stale_ref = retained_visual_frame(&mut session, "visual-stale-frame");
    let current_ref = retained_visual_frame(&mut session, "visual-current-frame");
    assert_ne!(stale_ref, current_ref);
    let (capture, grant) = preview_invocation(
        &session,
        "capture-stale-frame",
        TOOL_ID_RUNTIME_CAPTURE_ISSUE,
        AiToolInvocationPayload::RuntimeCaptureIssue(ProjectRuntimeCaptureIssueInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            frame_evidence_ref: stale_ref,
            symptom: None,
        }),
    );

    let result = AiCapabilityToolKernel::new().execute(&mut session, capture, &grant);

    assert_eq!(result.status, AiToolExecutionStatus::Failed);
    assert_eq!(
        result.diagnostics[0].code,
        "ai_tool.runtime_capture_issue_failed"
    );
    assert!(result.diagnostics[0].message.contains("stale"));
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn visual_evidence_ref_rejects_cross_project_frame() {
    let (mut source_session, source_root) = created_preview_session("VisualSourceProject");
    let frame_operation_id = "visual-cross-project-frame";
    let frame_evidence_ref = retained_visual_frame(&mut source_session, frame_operation_id);
    let evidence = ProjectPreviewEvidence::read_frame(
        source_session
            .active_project_session()
            .unwrap()
            .write_scope(),
        &frame_evidence_ref,
    )
    .unwrap();

    let (mut target_session, target_root) = created_preview_session("VisualTargetProject");
    for relative in [&frame_evidence_ref, &evidence.screenshot_ref] {
        let destination = target_root.join(relative);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(source_root.join(relative), destination).unwrap();
    }
    target_session.project_preview_frame_result = Some(ProjectPreviewFrameResult::captured(
        &frame_evidence_ref,
        evidence.clone(),
    ));
    let (capture, grant) = preview_invocation(
        &target_session,
        "capture-cross-project-frame",
        TOOL_ID_RUNTIME_CAPTURE_ISSUE,
        AiToolInvocationPayload::RuntimeCaptureIssue(ProjectRuntimeCaptureIssueInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            frame_evidence_ref,
            symptom: None,
        }),
    );

    let result = AiCapabilityToolKernel::new().execute(&mut target_session, capture, &grant);

    assert_eq!(result.status, AiToolExecutionStatus::Failed);
    assert_eq!(
        result.diagnostics[0].code,
        "ai_tool.runtime_capture_issue_failed"
    );
    assert!(result.diagnostics[0].message.contains("different project"));
    drop(source_session);
    drop(target_session);
    std::fs::remove_dir_all(source_root).unwrap();
    std::fs::remove_dir_all(target_root).unwrap();
}

#[test]
fn visual_evidence_ref_rejects_tampered_png_and_metadata() {
    for tamper_metadata in [false, true] {
        let (mut session, root) = created_preview_session(if tamper_metadata {
            "VisualMetadataTamper"
        } else {
            "VisualPngTamper"
        });
        let frame_operation_id = if tamper_metadata {
            "visual-metadata-frame"
        } else {
            "visual-png-frame"
        };
        let frame_evidence_ref = retained_visual_frame(&mut session, frame_operation_id);
        let evidence = ProjectPreviewEvidence::read_frame(
            session.active_project_session().unwrap().write_scope(),
            &frame_evidence_ref,
        )
        .unwrap();
        let tampered_path = if tamper_metadata {
            root.join(&frame_evidence_ref)
        } else {
            root.join(&evidence.screenshot_ref)
        };
        std::fs::write(tampered_path, b"tampered").unwrap();
        let (capture, grant) = preview_invocation(
            &session,
            if tamper_metadata {
                "capture-metadata-tamper"
            } else {
                "capture-png-tamper"
            },
            TOOL_ID_RUNTIME_CAPTURE_ISSUE,
            AiToolInvocationPayload::RuntimeCaptureIssue(ProjectRuntimeCaptureIssueInput {
                schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
                frame_evidence_ref,
                symptom: None,
            }),
        );

        let result = AiCapabilityToolKernel::new().execute(&mut session, capture, &grant);
        assert_eq!(result.status, AiToolExecutionStatus::Failed);
        assert_eq!(
            result.diagnostics[0].code,
            "ai_tool.runtime_capture_issue_failed"
        );
        drop(session);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn visual_evidence_ref_rejects_forged_rehash_against_trusted_receipt() {
    let (mut session, root) = created_preview_session("VisualForgedRehash");
    let frame_operation_id = "visual-forged-rehash-frame";
    let frame_evidence_ref = retained_visual_frame(&mut session, frame_operation_id);
    let evidence = ProjectPreviewEvidence::read_frame(
        session.active_project_session().unwrap().write_scope(),
        &frame_evidence_ref,
    )
    .unwrap();
    let forged = replace_frame_and_rehash(&root, &frame_evidence_ref, evidence.clone());
    assert_ne!(forged.frame_digest, evidence.frame_digest);
    let (capture, grant) = preview_invocation(
        &session,
        "capture-forged-rehash",
        TOOL_ID_RUNTIME_CAPTURE_ISSUE,
        AiToolInvocationPayload::RuntimeCaptureIssue(ProjectRuntimeCaptureIssueInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            frame_evidence_ref,
            symptom: None,
        }),
    );

    let result = AiCapabilityToolKernel::new().execute(&mut session, capture, &grant);

    assert_eq!(result.status, AiToolExecutionStatus::Failed);
    assert_eq!(
        result.diagnostics[0].code,
        "ai_tool.runtime_capture_issue_failed"
    );
    assert!(result.diagnostics[0].message.contains("trusted receipt"));
    drop(session);
    std::fs::remove_dir_all(root).unwrap();
}
