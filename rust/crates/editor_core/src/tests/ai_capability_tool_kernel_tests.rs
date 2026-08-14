use super::*;
use editor_ui_model::InputActionValueKind;
use engine_runtime::aui::{AuiActionRef, AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::{
    LinkedProjectRuntimeSet, ProjectRuntimeError, ProjectRuntimeModule,
    ProjectRuntimeModuleDescriptor, ProjectRuntimeRegistration,
};
use std::collections::BTreeSet;
use std::sync::Arc;

const RETIRED_MUTATE_CANDIDATE_TOOL_ID: &str = concat!("project", ".mutate", ".candidate");
const RETIRED_ROLLBACK_CANDIDATE_TOOL_ID: &str = concat!("project", ".rollback", ".candidate");

fn created_session(name: &str) -> (EditorSession, PathBuf) {
    let root = fixtures::unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: root.display().to_string(),
        name: name.to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    (session, root)
}

fn read_invocation(
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

fn write_hidden_start_button_fixture(root: &Path) {
    std::fs::create_dir_all(root.join("AUI")).unwrap();
    std::fs::create_dir_all(root.join("RuntimeModule/src")).unwrap();
    let root_node = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["start-button"]);
    let mut start_button = AuiNode::new(
        "start-button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(40.0, 40.0, 220.0, 64.0),
    )
    .with_parent("root")
    .with_text("Start Game")
    .with_action(AuiActionRef::click("menu.start_game"));
    start_button.name = "Primary Start Button".to_string();
    start_button.visible = false;
    let document = AuiDocument::new(
        "main-menu",
        vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
        vec![root_node, start_button],
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

fn capture_visual_issue_fixture(session: &mut EditorSession, operation_id: &str) -> String {
    let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
    let preview_operation_id = "preview-visual-fixture";
    let ticket = ProjectPreviewFrameTicket {
        schema_version: PROJECT_PREVIEW_FRAME_TICKET_SCHEMA_VERSION.to_string(),
        operation_id: preview_operation_id.to_string(),
        project_identity: binding.project_id.clone(),
        expected_project_digest: binding.project_digest.clone(),
        game_view_session_id: "game-view-visual-fixture".to_string(),
        expected_texture_id: "texture-visual-fixture".to_string(),
        expected_frame_index: 1,
        expected_runtime_frame_hash: "runtime-frame-visual-fixture".to_string(),
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
            present_report_ref: "Library/Reports/visual-fixture-present.json".to_string(),
            rgba8: vec![0; 16],
        },
    )
    .unwrap();
    let frame_evidence_ref =
        ProjectPreviewEvidence::frame_evidence_ref(&evidence.operation_id).unwrap();
    session.project_preview_frame_result = Some(ProjectPreviewFrameResult::captured(
        &frame_evidence_ref,
        evidence,
    ));
    ProjectVisualDiagnostics::capture_issue(
        session,
        operation_id,
        &ProjectRuntimeCaptureIssueInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            frame_evidence_ref,
            symptom: None,
        },
    )
    .unwrap()
    .issue_bundle_ref
}

#[test]
fn ai_capability_tool_kernel_ui_locate_finds_invisible_named_node() {
    let (mut session, root) = created_session("UiLocate");
    write_hidden_start_button_fixture(&root);
    let (invocation, grant) = read_invocation(
        &session,
        "locate-hidden-start",
        TOOL_ID_UI_LOCATE,
        AiToolInvocationPayload::UiLocate(ProjectUiLocateInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            query: "start game".to_string(),
            issue_bundle_ref: None,
        }),
    );
    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);
    let Some(AiToolOutput::UiLocated(located)) = result.output else {
        panic!("expected located UI node: {:?}", result.diagnostics);
    };
    assert_eq!(located.candidates.len(), 1);
    assert_eq!(located.candidates[0].node_id, "start-button");
    assert_eq!(
        located.candidates[0].document_path,
        "AUI/main-menu.aui.json"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_ui_visibility_reports_first_semantic_failure() {
    let (mut session, root) = created_session("UiVisibility");
    write_hidden_start_button_fixture(&root);
    let issue_bundle_ref = capture_visual_issue_fixture(&mut session, "capture-hidden-start");
    let (invocation, grant) = read_invocation(
        &session,
        "explain-hidden-start",
        TOOL_ID_UI_EXPLAIN_VISIBILITY,
        AiToolInvocationPayload::UiExplainVisibility(ProjectUiExplainInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            document_path: "AUI/main-menu.aui.json".to_string(),
            node_id: "start-button".to_string(),
            issue_bundle_ref,
        }),
    );
    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);
    let Some(AiToolOutput::VisualIssue(bundle)) = result.output else {
        panic!("expected visual issue bundle: {:?}", result.diagnostics);
    };
    assert_eq!(bundle.node.node_id, "start-button");
    assert_eq!(bundle.node.first_failure_stage, "authored_visibility");
    assert!(!bundle.node.draw_command_present);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_ui_owner_trace_reaches_project_source() {
    let (mut session, root) = created_session("UiOwnerTrace");
    write_hidden_start_button_fixture(&root);
    let (invocation, grant) = read_invocation(
        &session,
        "trace-hidden-start",
        TOOL_ID_PROJECT_TRACE_UI_OWNER,
        AiToolInvocationPayload::ProjectTraceUiOwner(ProjectUiOwnerTraceInput {
            schema_version: PROJECT_UI_DIAGNOSTIC_INPUT_SCHEMA_VERSION.to_string(),
            document_path: "AUI/main-menu.aui.json".to_string(),
            node_id: "start-button".to_string(),
            issue_bundle_ref: None,
        }),
    );
    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);
    let Some(AiToolOutput::UiOwnerTrace(trace)) = result.output else {
        panic!("expected UI owner trace: {:?}", result.diagnostics);
    };
    assert_eq!(trace.action_ids, vec!["menu.start_game"]);
    assert!(trace
        .referenced_objects
        .iter()
        .any(|reference| reference.starts_with("AUI/main-menu.aui.json#")));
    assert!(trace
        .project_source_symbols
        .contains(&"RuntimeModule/src/menu.rs::start_game".to_string()));
    let _ = std::fs::remove_dir_all(root);
}

fn build_export_with_kernel(
    session: &mut EditorSession,
    invocation_id: &str,
) -> ProjectBuildExportEvidence {
    let (invocation, grant) = read_invocation(
        session,
        invocation_id,
        TOOL_ID_PROJECT_BUILD_EXPORT,
        AiToolInvocationPayload::ProjectBuildExport(ProjectBuildExportInput {
            schema_version: PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION.to_string(),
            profile: "windows-dev".to_string(),
        }),
    );
    let result = AiCapabilityToolKernel::new().execute(session, invocation, &grant);
    assert_eq!(
        result.status,
        AiToolExecutionStatus::Completed,
        "build diagnostics: {:?}",
        result.diagnostics
    );
    let Some(AiToolOutput::ProjectBuildExport(evidence)) = result.output else {
        panic!("expected build export evidence")
    };
    evidence
}

#[test]
fn ai_capability_tool_kernel_build_export_uses_isolated_delivery_root() {
    let (mut session, root) = created_session("GatewayBuildExport");
    let evidence = build_export_with_kernel(&mut session, "gateway-build-export");
    assert!(evidence
        .package_dir
        .starts_with("Library/AiCapability/Deliveries/"));
    assert!(root.join(&evidence.package_dir).join("Game.exe").exists());
    assert!(root
        .join(&evidence.package_dir)
        .join("package-manifest.json")
        .exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_delivery_verify_runs_existing_package() {
    let (mut session, root) = created_session("GatewayDeliveryVerify");
    let build = build_export_with_kernel(&mut session, "gateway-delivery-build");
    let (invocation, grant) = read_invocation(
        &session,
        "gateway-delivery-verify",
        TOOL_ID_PROJECT_DELIVERY_VERIFY,
        AiToolInvocationPayload::ProjectDeliveryVerify(ProjectDeliveryVerifyInput {
            schema_version: PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION.to_string(),
            package_dir: build.package_dir,
            mode: "headless".to_string(),
            timeout_ms: 30_000,
            frame_limit: 2,
            screenshot: false,
        }),
    );
    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);
    assert_eq!(
        result.status,
        AiToolExecutionStatus::Completed,
        "delivery diagnostics: {:?}",
        result.diagnostics
    );
    let Some(AiToolOutput::ProjectDeliveryVerify(evidence)) = result.output else {
        panic!("expected delivery verification evidence")
    };
    assert_eq!(evidence.report.process_exit_code, Some(0));
    assert_eq!(evidence.report.child_player_exit_code, Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn ai_capability_tool_kernel_delivery_root_containment_rejects_build_junction_escape() {
    let (mut session, root) = created_session("GatewayBuildJunctionEscape");
    let outside = fixtures::unique_editor_project_temp_dir();
    std::fs::create_dir_all(&outside).unwrap();
    let delivery_parent = root.join("Library").join("AiCapability");
    std::fs::create_dir_all(&delivery_parent).unwrap();
    let delivery_root = delivery_parent.join("Deliveries");
    create_directory_junction(&outside, &delivery_root);

    let (invocation, grant) = read_invocation(
        &session,
        "gateway-build-junction-escape",
        TOOL_ID_PROJECT_BUILD_EXPORT,
        AiToolInvocationPayload::ProjectBuildExport(ProjectBuildExportInput {
            schema_version: PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION.to_string(),
            profile: "windows-dev".to_string(),
        }),
    );
    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);
    let escaped_package = outside
        .join("gateway-build-junction-escape")
        .join("Windows")
        .join("dev");
    let escaped_package_exists = escaped_package.exists();

    std::fs::remove_dir(&delivery_root).unwrap();
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);

    assert_eq!(result.status, AiToolExecutionStatus::Failed);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ai_tool.build_export_rejected"),
        "build must fail at the controlled Delivery root boundary: {:?}",
        result.diagnostics
    );
    assert!(
        !escaped_package_exists,
        "Gateway build wrote through a Delivery-root junction"
    );
}

#[cfg(windows)]
#[test]
fn ai_capability_tool_kernel_delivery_root_containment_rejects_verify_junction_escape() {
    let (session, root) = created_session("GatewayVerifyJunctionEscape");
    let in_project_non_delivery = root.join("Build").join("forged-delivery");
    std::fs::create_dir_all(in_project_non_delivery.join("dev")).unwrap();
    let delivery_operation = root
        .join("Library")
        .join("AiCapability")
        .join("Deliveries")
        .join("forged-operation");
    std::fs::create_dir_all(&delivery_operation).unwrap();
    let forged_windows_dir = delivery_operation.join("Windows");
    create_directory_junction(&in_project_non_delivery, &forged_windows_dir);

    let result = ProjectDeliveryTools::verify_delivery(
        &session,
        &ProjectDeliveryVerifyInput {
            schema_version: PROJECT_DELIVERY_TOOL_INPUT_SCHEMA_VERSION.to_string(),
            package_dir: "Library/AiCapability/Deliveries/forged-operation/Windows/dev".to_string(),
            mode: "headless".to_string(),
            timeout_ms: 1_000,
            frame_limit: 1,
            screenshot: false,
        },
    );

    std::fs::remove_dir(&forged_windows_dir).unwrap();
    let _ = std::fs::remove_dir_all(root);

    let error = result.expect_err("verification must reject a package outside Delivery root");
    assert!(
        error.contains("Gateway delivery root"),
        "unexpected containment diagnostic: {error}"
    );
}

#[cfg(windows)]
fn create_directory_junction(target: &Path, link: &Path) {
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .expect("launch mklink /J");
    assert!(
        output.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ai_capability_tool_kernel_async_operation_is_accepted_before_work_runs() {
    let (mut session, root) = created_session("AsyncToolOperation");
    let (invocation, grant) = read_invocation(
        &session,
        "async-preview",
        TOOL_ID_PROJECT_PREVIEW,
        AiToolInvocationPayload::Preview,
    );
    let mut kernel = AiCapabilityToolKernel::new();
    let AiToolStartOutcome::Accepted(accepted) = kernel.start(&session, invocation, &grant) else {
        panic!("preview must be accepted asynchronously");
    };
    assert_eq!(accepted.state, AiToolOperationState::Queued);
    assert!(kernel
        .observe(&accepted.operation_id)
        .unwrap()
        .result
        .is_none());

    assert_eq!(kernel.pump_operations(&mut session, 1), 1);
    assert_eq!(
        kernel.observe(&accepted.operation_id).unwrap().state,
        AiToolOperationState::Preflight
    );
    kernel.pump_operations(&mut session, 1);
    assert_eq!(
        kernel.observe(&accepted.operation_id).unwrap().state,
        AiToolOperationState::Prepared
    );
    kernel.pump_operations(&mut session, 1);
    let awaiting = kernel.observe(&accepted.operation_id).unwrap();
    assert_eq!(awaiting.state, AiToolOperationState::Running);
    assert_eq!(awaiting.stage, "awaiting_frame_evidence");
    assert!(awaiting.result.is_none());
    assert_eq!(
        session
            .pending_project_preview_frame_ticket()
            .map(|ticket| ticket.operation_id.as_str()),
        Some(accepted.operation_id.as_str())
    );
    assert_eq!(
        awaiting
            .transitions
            .iter()
            .map(|transition| transition.state)
            .collect::<Vec<_>>(),
        vec![
            AiToolOperationState::Queued,
            AiToolOperationState::Preflight,
            AiToolOperationState::Prepared,
            AiToolOperationState::Running,
            AiToolOperationState::Running,
        ]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_cancel_worker_is_durable_before_commit() {
    let (mut session, root) = created_session("CancelToolWorker");
    let (invocation, grant) = read_invocation(
        &session,
        "cancel-preview",
        TOOL_ID_PROJECT_PREVIEW,
        AiToolInvocationPayload::Preview,
    );
    let mut kernel = AiCapabilityToolKernel::new();
    let AiToolStartOutcome::Accepted(accepted) = kernel.start(&session, invocation, &grant) else {
        panic!("preview must be accepted asynchronously");
    };
    kernel.pump_operations(&mut session, 1);
    let cancellation = kernel
        .cancel_durable(&session, &accepted.operation_id, &grant)
        .unwrap();
    assert_eq!(cancellation.status, AiToolCancellationStatus::Cancelled);
    assert!(cancellation.signal_sent);
    assert!(!cancellation.commit_started);
    assert!(!cancellation.terminal);
    assert_eq!(
        kernel.observe(&accepted.operation_id).unwrap().state,
        AiToolOperationState::Cancelling
    );
    kernel.pump_operations(&mut session, 1);
    let terminal = kernel.observe(&accepted.operation_id).unwrap();
    assert_eq!(terminal.state, AiToolOperationState::Cancelled);
    assert_eq!(
        terminal.result.unwrap().diagnostics[0].code,
        "ai_tool.operation_cancelled"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_recovery_reconciles_abandoned_nonterminal_operation() {
    let (session, root) = created_session("RecoverToolOperation");
    let (invocation, grant) = read_invocation(
        &session,
        "recover-preview",
        TOOL_ID_PROJECT_PREVIEW,
        AiToolInvocationPayload::Preview,
    );
    let operation_id = {
        let mut kernel = AiCapabilityToolKernel::new();
        let AiToolStartOutcome::Accepted(accepted) = kernel.start(&session, invocation, &grant)
        else {
            panic!("preview must be accepted asynchronously");
        };
        accepted.operation_id
    };

    let mut reopened = AiCapabilityToolKernel::new();
    reopened
        .inspect(&session, AiToolInspectRequest::project())
        .unwrap();
    let recovered = reopened.observe(&operation_id).unwrap();
    assert_eq!(recovered.state, AiToolOperationState::Interrupted);
    assert_eq!(
        recovered.result.unwrap().diagnostics[0].code,
        "ai_tool.operation_interrupted"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_project_aware_catalog() {
    let kernel = AiCapabilityToolKernel::new();
    let empty = EditorSession::new();
    let empty_catalog = kernel
        .catalog_for_session(&empty, AiToolCatalogRequest::default())
        .unwrap();
    assert!(!empty_catalog.tools.is_empty());
    for tool in &empty_catalog.tools {
        let availability = empty_catalog
            .availability(&tool.tool_id)
            .expect("every registered tool has v2 availability");
        if tool.tool_id == TOOL_ID_PROJECT_CREATE {
            assert_eq!(availability.state, crate::AiToolAvailabilityState::Ready);
            continue;
        }
        assert_eq!(availability.state, crate::AiToolAvailabilityState::Blocked);
        assert!(availability
            .reasons
            .iter()
            .any(|reason| reason.code == "ai_tool.availability.project_required"));
    }

    let (session, root) = created_session("ProjectAwareCatalog");
    let catalog = kernel
        .catalog_for_session(&session, AiToolCatalogRequest::default())
        .unwrap();
    for tool_id in [
        TOOL_ID_PROJECT_SEARCH,
        TOOL_ID_PROJECT_READ_OBJECT,
        TOOL_ID_PROJECT_REFERENCES,
        TOOL_ID_PROJECT_SOURCE_SYMBOLS,
        TOOL_ID_PROJECT_DIAGNOSTICS,
        TOOL_ID_EVIDENCE_READ,
    ] {
        assert!(catalog.tools.iter().any(|tool| tool.tool_id == tool_id));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_project_search_finds_name_text_and_source_without_compile() {
    let (mut session, root) = created_session("ProjectSearch");
    std::fs::create_dir_all(root.join("AUI")).unwrap();
    std::fs::create_dir_all(root.join("RuntimeModule/src")).unwrap();
    std::fs::write(
        root.join("AUI/main.aui.json"),
        r#"{"nodeId":"start-button","text":"Start Game","action":"action.start_game"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("RuntimeModule/src/lib.rs"),
        "pub fn start_game_from_menu() {}\n",
    )
    .unwrap();

    let (invocation, grant) = read_invocation(
        &session,
        "search-start-button",
        TOOL_ID_PROJECT_SEARCH,
        AiToolInvocationPayload::ProjectSearch(ProjectSearchInput {
            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
            query: "Start Game".to_string(),
            kinds: vec!["aui".to_string()],
            continuation_token: None,
            page_size: 25,
        }),
    );
    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);
    let Some(AiToolOutput::ProjectObservation(ProjectObservationResult::Search(page))) =
        result.output
    else {
        panic!("expected project search output: {:?}", result.diagnostics);
    };
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].project_relative_path, "AUI/main.aui.json");

    let index = ProjectObservationIndex::build(&session).unwrap();
    let symbols = index
        .source_symbols(&ProjectSourceSymbolsInput {
            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
            query: "start_game".to_string(),
            continuation_token: None,
            page_size: 25,
        })
        .unwrap();
    assert_eq!(symbols.symbols[0].name, "start_game_from_menu");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_references_are_stable_and_paginated() {
    let (session, root) = created_session("ProjectReferences");
    std::fs::create_dir_all(root.join("AUI")).unwrap();
    std::fs::create_dir_all(root.join("Rules")).unwrap();
    std::fs::write(
        root.join("AUI/main.aui.json"),
        r#"{"action":"action.start_game"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("Rules/menu.rule.json"),
        r#"{"dispatch":"action.start_game"}"#,
    )
    .unwrap();
    let index = ProjectObservationIndex::build(&session).unwrap();
    let first = index
        .references(&ProjectReferencesInput {
            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
            symbol_or_value: "action.start_game".to_string(),
            continuation_token: None,
            page_size: 1,
        })
        .unwrap();
    assert_eq!(first.references.len(), 1);
    let second = index
        .references(&ProjectReferencesInput {
            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
            symbol_or_value: "action.start_game".to_string(),
            continuation_token: first.next_continuation_token,
            page_size: 1,
        })
        .unwrap();
    assert_eq!(second.references.len(), 1);
    assert!(first.references[0].project_relative_path < second.references[0].project_relative_path);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_evidence_read_is_bounded_and_scope_checked() {
    let (session, root) = created_session("EvidenceRead");
    std::fs::create_dir_all(root.join("Library/Reports")).unwrap();
    std::fs::write(
        root.join("Library/Reports/preview.json"),
        r#"{"status":"passed","diagnostics":[]}"#,
    )
    .unwrap();
    let index = ProjectObservationIndex::build(&session).unwrap();
    let evidence = index
        .read_evidence_input(&ProjectEvidenceReadInput {
            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
            evidence_ref: "project-evidence:Library/Reports/preview.json".to_string(),
            max_bytes: 65536,
        })
        .unwrap();
    assert_eq!(evidence.content["status"], "passed");
    assert!(!evidence.truncated);
    assert!(index
        .read_evidence_input(&ProjectEvidenceReadInput {
            schema_version: PROJECT_OBSERVATION_INPUT_SCHEMA_VERSION.to_string(),
            evidence_ref: "project-evidence:project.aife.json".to_string(),
            max_bytes: 65536,
        })
        .unwrap_err()
        .contains("evidence_scope_rejected"));
    let _ = std::fs::remove_dir_all(root);
}

fn scoped_grant(
    session: &EditorSession,
    grant_id: &str,
    domains: Vec<&str>,
    mutation_kinds: Vec<AiMutationKind>,
) -> AiCapabilityGrant {
    let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
    AiCapabilityGrant::scoped_mutation(
        grant_id,
        binding.project_id,
        sha256_prefixed(format!("outcome-{grant_id}").as_bytes()),
        binding.project_digest,
        domains.into_iter().map(str::to_string).collect(),
        mutation_kinds,
        "local-user",
    )
    .unwrap()
}

fn input_candidate(
    session: &EditorSession,
    candidate_id: &str,
    action_id: &str,
) -> AiCandidateToolInput {
    input_candidate_at(session, candidate_id, action_id, "Input/input.none.json")
}

fn input_candidate_at(
    session: &EditorSession,
    candidate_id: &str,
    action_id: &str,
    mapping_path: &str,
) -> AiCandidateToolInput {
    let patch = ProjectPatchDocument::new(
        format!("patch-{candidate_id}"),
        format!("Add {action_id}"),
        PatchSource::Test,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: format!("operation-{candidate_id}"),
            depends_on: Vec::new(),
            path: mapping_path.to_string(),
            action_id: action_id.to_string(),
            value_type: InputActionValueKind::Button,
        })],
    );
    let envelope = ProjectCandidateEntry::project_patch_envelope(
        session,
        candidate_id,
        ProjectCandidateSourceKind::ImportedCodex,
        "ai-capability-tool-kernel-test",
        patch,
    )
    .unwrap();
    AiCandidateToolInput {
        envelope,
        source_file_path: None,
        controlled_source_patch_validation: None,
    }
}

fn candidate_invocation(
    session: &EditorSession,
    invocation_id: &str,
    input: AiCandidateToolInput,
) -> AiToolInvocation {
    let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
    AiToolInvocation {
        schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
        invocation_id: invocation_id.to_string(),
        tool_id: TOOL_ID_PROJECT_MUTATE.to_string(),
        expected_project_digest: binding.project_digest,
        payload: AiToolInvocationPayload::Candidate(input),
    }
}

fn preview_invocation(session: &EditorSession, invocation_id: &str) -> AiToolInvocation {
    let binding = ProjectCandidateEntry::inspect_project_binding(session).unwrap();
    AiToolInvocation {
        schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
        invocation_id: invocation_id.to_string(),
        tool_id: TOOL_ID_PROJECT_PREVIEW.to_string(),
        expected_project_digest: binding.project_digest,
        payload: AiToolInvocationPayload::Preview,
    }
}

fn diagnostic_code(result: &AiToolResult) -> &str {
    result
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.code.as_str())
        .expect("failed tool result has a diagnostic")
}

fn applied_receipt(result: &AiToolResult) -> ProjectCandidateApplyReceipt {
    match result.output.as_ref().expect("completed output") {
        AiToolOutput::CandidateApplied(receipt) => receipt.candidate_receipt.clone(),
        other => panic!("expected CandidateApplied, got {other:?}"),
    }
}

#[test]
fn ai_capability_tool_kernel_schema_and_catalog_are_small_and_strict() {
    let kernel = AiCapabilityToolKernel::new();
    let catalog = kernel.catalog(AiToolCatalogRequest::default()).unwrap();
    let ids = catalog
        .tools
        .iter()
        .map(|tool| tool.tool_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            TOOL_ID_PROJECT_CREATE,
            TOOL_ID_PROJECT_INSPECT,
            TOOL_ID_PROJECT_MUTATE,
            TOOL_ID_PROJECT_ROLLBACK,
            TOOL_ID_PROJECT_PREVIEW,
            TOOL_ID_PROJECT_SEARCH,
            TOOL_ID_PROJECT_READ_OBJECT,
            TOOL_ID_PROJECT_REFERENCES,
            TOOL_ID_PROJECT_SOURCE_SYMBOLS,
            TOOL_ID_PROJECT_DIAGNOSTICS,
            TOOL_ID_EVIDENCE_READ,
            TOOL_ID_RUNTIME_CAPTURE_ISSUE,
            TOOL_ID_UI_LOCATE,
            TOOL_ID_UI_EXPLAIN_VISIBILITY,
            TOOL_ID_PROJECT_TRACE_UI_OWNER,
            TOOL_ID_PROJECT_BUILD_EXPORT,
            TOOL_ID_PROJECT_DELIVERY_VERIFY,
        ]
    );
    assert!(catalog.tools.iter().all(|tool| {
        tool.schema_version == AI_TOOL_DESCRIPTOR_SCHEMA_VERSION
            && tool.tool_version == AI_TOOL_IMPLEMENTATION_VERSION_V1
            && !tool.minimal_input_example.is_null()
            && !tool.completion_evidence.is_empty()
            && !tool.preconditions.is_empty()
    }));
    assert!(serde_json::from_str::<AiToolCatalogRequest>(
        r#"{"schemaVersion":"ai-tool-catalog.v1","unknown":true}"#
    )
    .is_err());
}

fn assert_direct_input_schema_is_recursively_strict(schema: &serde_json::Value, path: &str) {
    match schema {
        serde_json::Value::Object(object) => {
            let describes_object = object.get("type").and_then(serde_json::Value::as_str)
                == Some("object")
                || object.contains_key("properties");
            if describes_object {
                assert_eq!(
                    object.get("additionalProperties"),
                    Some(&serde_json::Value::Bool(false)),
                    "object schema at {path} must reject unknown fields"
                );
            }
            for (key, child) in object {
                assert_direct_input_schema_is_recursively_strict(child, &format!("{path}/{key}"));
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                assert_direct_input_schema_is_recursively_strict(child, &format!("{path}/{index}"));
            }
        }
        _ => {}
    }
}

fn assert_payload_matches_tool_id(tool_id: &str, payload: &AiToolInvocationPayload) {
    assert!(
        matches!(
            (tool_id, payload),
            (
                TOOL_ID_PROJECT_CREATE,
                AiToolInvocationPayload::ProjectCreate(_)
            ) | (
                TOOL_ID_PROJECT_MUTATE,
                AiToolInvocationPayload::ProjectMutationIntent(_)
            ) | (
                TOOL_ID_PROJECT_ROLLBACK,
                AiToolInvocationPayload::ProjectRollbackRef(_)
            ) | (TOOL_ID_PROJECT_PREVIEW, AiToolInvocationPayload::Preview)
                | (
                    TOOL_ID_PROJECT_SEARCH,
                    AiToolInvocationPayload::ProjectSearch(_)
                )
                | (
                    TOOL_ID_PROJECT_READ_OBJECT,
                    AiToolInvocationPayload::ProjectReadObject(_)
                )
                | (
                    TOOL_ID_PROJECT_REFERENCES,
                    AiToolInvocationPayload::ProjectReferences(_)
                )
                | (
                    TOOL_ID_PROJECT_SOURCE_SYMBOLS,
                    AiToolInvocationPayload::ProjectSourceSymbols(_)
                )
                | (
                    TOOL_ID_PROJECT_DIAGNOSTICS,
                    AiToolInvocationPayload::ProjectDiagnostics(_)
                )
                | (
                    TOOL_ID_EVIDENCE_READ,
                    AiToolInvocationPayload::EvidenceRead(_)
                )
                | (
                    TOOL_ID_RUNTIME_CAPTURE_ISSUE,
                    AiToolInvocationPayload::RuntimeCaptureIssue(_)
                )
                | (TOOL_ID_UI_LOCATE, AiToolInvocationPayload::UiLocate(_))
                | (
                    TOOL_ID_UI_EXPLAIN_VISIBILITY,
                    AiToolInvocationPayload::UiExplainVisibility(_)
                )
                | (
                    TOOL_ID_PROJECT_TRACE_UI_OWNER,
                    AiToolInvocationPayload::ProjectTraceUiOwner(_)
                )
                | (
                    TOOL_ID_PROJECT_BUILD_EXPORT,
                    AiToolInvocationPayload::ProjectBuildExport(_)
                )
                | (
                    TOOL_ID_PROJECT_DELIVERY_VERIFY,
                    AiToolInvocationPayload::ProjectDeliveryVerify(_)
                )
        ),
        "registry decoded {tool_id} to the wrong payload variant: {payload:?}"
    );
}

#[test]
fn ai_tool_contract_registry_exposes_strict_direct_inputs_and_canonical_decode() {
    let registry = AiToolContractRegistry::new();
    let catalog = AiCapabilityToolKernel::new()
        .catalog(AiToolCatalogRequest::default())
        .unwrap();
    assert_eq!(registry.descriptors(), catalog.tools.as_slice());
    let distinct_input_schemas = catalog
        .tools
        .iter()
        .map(|tool| serde_json::to_string(&tool.input_schema).unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(distinct_input_schemas.len(), catalog.tools.len());

    for tool in &catalog.tools {
        assert_eq!(tool.input_schema["type"], "object");
        assert_eq!(tool.input_schema["additionalProperties"], false);
        assert_direct_input_schema_is_recursively_strict(&tool.input_schema, &tool.tool_id);
        assert_eq!(tool.output_schema["type"], "object");
        assert_eq!(tool.output_schema["additionalProperties"], false);
        assert_eq!(tool.progress_event_schema["additionalProperties"], false);
        assert_eq!(tool.tool_version, AI_TOOL_IMPLEMENTATION_VERSION_V1);
        if tool.tool_id == TOOL_ID_PROJECT_ROLLBACK {
            assert_eq!(
                tool.input_schema["properties"]["schemaVersion"]["const"],
                EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION
            );
            assert_eq!(
                tool.minimal_input_example["schemaVersion"],
                EXTERNAL_PROJECT_ROLLBACK_SCHEMA_VERSION
            );
        } else {
            assert!(tool.input_schema["properties"]
                .get("schemaVersion")
                .is_none());
            assert!(tool.minimal_input_example.get("schemaVersion").is_none());
        }
        assert!(tool.minimal_input_example.get("payloadKind").is_none());
        registry
            .validate_direct_input(&tool.tool_id, &tool.minimal_input_example)
            .unwrap();

        if tool.tool_id == TOOL_ID_PROJECT_INSPECT {
            assert_eq!(
                registry
                    .decode_inspect_request(tool.minimal_input_example.clone())
                    .unwrap(),
                AiToolInspectRequest::project()
            );
        } else {
            let payload = registry
                .decode_invocation_payload(&tool.tool_id, tool.minimal_input_example.clone())
                .unwrap();
            assert_payload_matches_tool_id(&tool.tool_id, &payload);
        }

        for forbidden in [
            "schemaVersion",
            "projectIdentity",
            "toolVersion",
            "payloadKind",
            "grantRef",
        ] {
            let mut unknown_top_level = tool.minimal_input_example.clone();
            unknown_top_level
                .as_object_mut()
                .unwrap()
                .insert(forbidden.to_string(), serde_json::json!("forged"));
            if tool.tool_id == TOOL_ID_PROJECT_INSPECT {
                assert!(registry.decode_inspect_request(unknown_top_level).is_err());
            } else {
                assert!(registry
                    .decode_invocation_payload(&tool.tool_id, unknown_top_level)
                    .is_err());
            }
        }

        match tool.tool_id.as_str() {
            TOOL_ID_PROJECT_INSPECT => {
                assert!(registry
                    .decode_invocation_payload(
                        TOOL_ID_PROJECT_INSPECT,
                        tool.minimal_input_example.clone()
                    )
                    .is_err());
            }
            TOOL_ID_PROJECT_CREATE
            | TOOL_ID_PROJECT_MUTATE
            | TOOL_ID_PROJECT_ROLLBACK
            | TOOL_ID_PROJECT_PREVIEW
            | TOOL_ID_PROJECT_SEARCH
            | TOOL_ID_PROJECT_READ_OBJECT
            | TOOL_ID_PROJECT_REFERENCES
            | TOOL_ID_PROJECT_SOURCE_SYMBOLS
            | TOOL_ID_PROJECT_DIAGNOSTICS
            | TOOL_ID_EVIDENCE_READ
            | TOOL_ID_RUNTIME_CAPTURE_ISSUE
            | TOOL_ID_UI_LOCATE
            | TOOL_ID_UI_EXPLAIN_VISIBILITY
            | TOOL_ID_PROJECT_TRACE_UI_OWNER
            | TOOL_ID_PROJECT_BUILD_EXPORT
            | TOOL_ID_PROJECT_DELIVERY_VERIFY => {}
            other => panic!("unexpected catalog tool {other}"),
        }
    }

    assert!(registry.descriptor("project.unknown").is_none());
    assert!(registry
        .decode_invocation_payload(TOOL_ID_PROJECT_SEARCH, serde_json::json!({}))
        .is_err());
}

#[test]
fn ai_tool_contract_evidence_read_exposes_exact_scope_and_preview_consumer() {
    let registry = AiToolContractRegistry::new();
    let descriptor = registry.descriptor(TOOL_ID_EVIDENCE_READ).unwrap();
    assert!(descriptor.summary.contains("project-evidence:"));
    assert!(descriptor.summary.contains("Library/Reports/"));
    assert!(descriptor.summary.contains("Library/AiToolKernel/"));
    assert!(descriptor.summary.contains("runtime.capture_issue"));
    assert_eq!(
        descriptor.input_schema["properties"]["evidenceRef"]["oneOf"][0]["pattern"],
        "^project-evidence:Library/Reports/"
    );
    assert_eq!(
        descriptor.input_schema["properties"]["evidenceRef"]["oneOf"][1]["pattern"],
        "^project-evidence:Library/AiToolKernel/"
    );
    assert!(
        descriptor.input_schema["properties"]["evidenceRef"]["description"]
            .as_str()
            .unwrap()
            .contains("not a Preview frameEvidenceRef")
    );
}

#[test]
fn ai_tool_contract_registry_rejects_retired_candidate_public_entries() {
    let registry = AiToolContractRegistry::new();
    for retired_tool_id in [
        RETIRED_MUTATE_CANDIDATE_TOOL_ID,
        RETIRED_ROLLBACK_CANDIDATE_TOOL_ID,
    ] {
        assert!(registry.descriptor(retired_tool_id).is_none());
        assert!(registry
            .decode_invocation_payload(retired_tool_id, serde_json::json!({}))
            .is_err());
    }
}

#[test]
fn ai_tool_project_patch_prepare_failure_is_terminal_before_operation_queue() {
    let (session, root) = created_session("CandidatePrepareBeforeQueue");
    let mut input = input_candidate(&session, "prepare-before-queue", "action.prepare");
    input.envelope.project_patch_context_hash = None;
    let invocation = candidate_invocation(&session, "prepare-before-queue", input);
    let grant = scoped_grant(
        &session,
        "prepare-before-queue-grant",
        vec!["input"],
        vec![AiMutationKind::ProjectPatch],
    );
    let mut kernel = AiCapabilityToolKernel::new();

    let AiToolStartOutcome::Terminal(result) = kernel.start(&session, invocation, &grant) else {
        panic!("prepare-invalid ProjectPatch must not enter the durable operation queue");
    };
    assert_eq!(
        result.diagnostics[0].code,
        "project_candidate_entry.digest_invalid"
    );
    assert!(kernel.observe(&result.operation_id).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_grant_rejects_tamper_expiry_and_domain_escalation() {
    let (mut session, _) = created_session("ToolGrantPolicy");
    let mut kernel = AiCapabilityToolKernel::new();

    let mut tampered = scoped_grant(
        &session,
        "grant-tampered",
        vec!["input"],
        vec![AiMutationKind::ProjectPatch],
    );
    tampered.allowed_domains.push("scene".to_string());
    let input = input_candidate(&session, "tampered", "action.tampered");
    let invocation = candidate_invocation(&session, "invoke-tampered", input);
    let result = kernel.execute(&mut session, invocation, &tampered);
    assert_eq!(result.status, AiToolExecutionStatus::Failed);
    assert_eq!(diagnostic_code(&result), "ai_tool.grant_digest_mismatch");

    let mut expired = scoped_grant(
        &session,
        "grant-expired",
        vec!["input"],
        vec![AiMutationKind::ProjectPatch],
    );
    expired.expires_at_epoch_ms = Some(0);
    expired = expired.seal().unwrap();
    let input = input_candidate(&session, "expired", "action.expired");
    let invocation = candidate_invocation(&session, "invoke-expired", input);
    let result = kernel.execute(&mut session, invocation, &expired);
    assert_eq!(diagnostic_code(&result), "ai_tool.grant_expired");

    let wrong_domain = scoped_grant(
        &session,
        "grant-wrong-domain",
        vec!["scene"],
        vec![AiMutationKind::ProjectPatch],
    );
    let input = input_candidate(&session, "wrong-domain", "action.wrong-domain");
    let invocation = candidate_invocation(&session, "invoke-wrong-domain", input);
    let result = kernel.execute(&mut session, invocation, &wrong_domain);
    assert_eq!(diagnostic_code(&result), "ai_tool.domain_not_granted");

    let inspection = kernel
        .inspect(&session, AiToolInspectRequest::project())
        .unwrap();
    let AiToolInspectPayload::Project(inspection) = inspection.payload else {
        panic!("project inspection expected");
    };
    assert_eq!(inspection.recorded_operation_count, 0);
}

#[test]
fn ai_capability_tool_kernel_scope_mode_distinguishes_exact_low_risk_and_elevated() {
    let (session, root) = created_session("ToolGrantScopeMode");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let exact = AiCapabilityGrant::scoped_mutation(
        "exact",
        binding.project_id.clone(),
        "sha256:outcome-exact",
        binding.project_digest.clone(),
        vec!["aui".to_string()],
        vec![AiMutationKind::ProjectPatch],
        "local-user",
    )
    .unwrap();
    assert_eq!(exact.scope_mode, AiCapabilityScopeMode::ExactDomains);

    let low_risk = AiCapabilityGrant::project_owned_low_risk(
        "low-risk",
        binding.project_id.clone(),
        "sha256:outcome-low-risk",
        binding.project_digest.clone(),
        "local-user",
    )
    .unwrap();
    assert_eq!(
        low_risk.scope_mode,
        AiCapabilityScopeMode::ProjectOwnedLowRisk
    );
    for domain in ["aui", "rule", "rollback", "runtime_module"] {
        assert!(low_risk.allowed_domains.contains(&domain.to_string()));
    }
    assert!(!low_risk.allow_delete);
    assert!(!low_risk.allow_dependency_change);

    let elevated = AiCapabilityGrant::elevated(AiElevatedGrantSpec {
        grant_id: "elevated".to_string(),
        project_identity: binding.project_id,
        user_visible_outcome_digest: "sha256:outcome-elevated".to_string(),
        base_digest: binding.project_digest,
        allowed_domains: vec!["runtime_module".to_string()],
        allowed_mutation_kinds: vec![AiMutationKind::ControlledSourcePatch],
        allow_delete: true,
        allow_dependency_change: true,
        allow_network: false,
        issued_by: "local-maintainer".to_string(),
    })
    .unwrap();
    assert_eq!(elevated.scope_mode, AiCapabilityScopeMode::Elevated);
    assert_eq!(elevated.kind, AiCapabilityGrantKind::Elevated);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_generic_source_patch_uses_low_risk_escape_lane() {
    let (mut session, root) = created_session("GenericSourcePatch");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let input = AiCandidateToolInput {
        envelope: ProjectCandidateEnvelope {
            schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
            candidate_id: "generic-source".to_string(),
            source_kind: ProjectCandidateSourceKind::ImportedCodex,
            source_label: "generic-source-test".to_string(),
            target_project_id: binding.project_id.clone(),
            expected_base_project_digest: binding.project_digest.clone(),
            project_patch_context_hash: None,
            payload: ProjectCandidatePayload::ControlledSourcePatch {
                request: ControlledSourcePatchPrepareRequest {
                    revision_id: "generic_source_revision".to_string(),
                    project_root: root.clone(),
                    candidate_store_root: fixtures::unique_editor_project_temp_dir(),
                    source_patch: ControlledSourcePatchDocument {
                        schema_version: CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION.to_string(),
                        patch_id: "generic-source-patch".to_string(),
                        operations: vec![ControlledSourcePatchOperation::CreateOrReplace {
                            path: "RuntimeModule/src/lib.rs".to_string(),
                            text: "pub fn generic_project_feature() {}\n".to_string(),
                        }],
                    },
                },
            },
        },
        source_file_path: None,
        controlled_source_patch_validation: None,
    };
    let grant = AiCapabilityGrant::project_owned_low_risk(
        "generic-source-grant",
        binding.project_id,
        "sha256:generic-source-outcome",
        binding.project_digest,
        "local-user",
    )
    .unwrap();
    let invocation = candidate_invocation(&session, "invoke-generic-source", input);
    let result = AiCapabilityToolKernel::new().execute(&mut session, invocation, &grant);
    assert_ne!(diagnostic_code(&result), "ai_tool.domain_not_granted");
    assert_ne!(
        diagnostic_code(&result),
        "ai_tool.mutation_kind_not_granted"
    );
    assert!(!root.join("RuntimeModule/src/lib.rs").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_generic_mutation_rejects_low_risk_escalation() {
    let (session, root) = created_session("GenericMutationRejects");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut grant = AiCapabilityGrant::project_owned_low_risk(
        "low-risk-tamper",
        binding.project_id,
        "sha256:low-risk",
        binding.project_digest,
        "local-user",
    )
    .unwrap();
    grant.allow_delete = true;
    grant = grant.seal().unwrap();
    assert_eq!(
        grant.validate_integrity().unwrap_err().code,
        "ai_tool.low_risk_grant_escalated"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ai_capability_tool_kernel_grant_rejects_delete_and_dependency_escalation() {
    let (mut session, root) = created_session("ToolGrantEscalation");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let delete_patch = ProjectPatchDocument::new(
        "delete-input-patch",
        "Delete input mapping",
        PatchSource::Test,
        vec![PatchOperation::Input(
            InputPatchOperation::DeleteInputMapping {
                operation_id: "delete-input".to_string(),
                depends_on: Vec::new(),
                path: "Input/input.none.json".to_string(),
            },
        )],
    );
    let delete_input = AiCandidateToolInput {
        envelope: ProjectCandidateEntry::project_patch_envelope(
            &session,
            "delete-input",
            ProjectCandidateSourceKind::ImportedCodex,
            "delete-test",
            delete_patch,
        )
        .unwrap(),
        source_file_path: None,
        controlled_source_patch_validation: None,
    };
    let delete_grant = scoped_grant(
        &session,
        "grant-no-delete",
        vec!["input"],
        vec![AiMutationKind::ProjectPatch],
    );
    let mut kernel = AiCapabilityToolKernel::new();
    let invocation = candidate_invocation(&session, "invoke-delete", delete_input);
    let result = kernel.execute(&mut session, invocation, &delete_grant);
    assert_eq!(diagnostic_code(&result), "ai_tool.delete_not_granted");

    let source_input = AiCandidateToolInput {
        envelope: ProjectCandidateEnvelope {
            schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
            candidate_id: "dependency-change".to_string(),
            source_kind: ProjectCandidateSourceKind::ImportedCodex,
            source_label: "dependency-test".to_string(),
            target_project_id: binding.project_id,
            expected_base_project_digest: binding.project_digest,
            project_patch_context_hash: None,
            payload: ProjectCandidatePayload::ControlledSourcePatch {
                request: ControlledSourcePatchPrepareRequest {
                    revision_id: "dependency_change_revision".to_string(),
                    project_root: root,
                    candidate_store_root: fixtures::unique_editor_project_temp_dir(),
                    source_patch: ControlledSourcePatchDocument {
                        schema_version: CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION.to_string(),
                        patch_id: "dependency-change-patch".to_string(),
                        operations: vec![ControlledSourcePatchOperation::CreateOrReplace {
                            path: "RuntimeModule/Cargo.toml".to_string(),
                            text: "[package]\nname = \"dependency_change\"\nversion = \"0.0.1\"\n"
                                .to_string(),
                        }],
                    },
                },
            },
        },
        source_file_path: None,
        controlled_source_patch_validation: None,
    };
    let dependency_grant = scoped_grant(
        &session,
        "grant-no-dependency",
        vec!["runtime_module"],
        vec![AiMutationKind::ControlledSourcePatch],
    );
    let invocation = candidate_invocation(&session, "invoke-dependency", source_input);
    let result = kernel.execute(&mut session, invocation, &dependency_grant);
    assert_eq!(
        diagnostic_code(&result),
        "ai_tool.dependency_change_not_granted"
    );
}

#[test]
fn ai_capability_tool_kernel_lineage_allows_replanning_and_reopens_from_journal() {
    let (mut session, root) = created_session("ToolLineage");
    let grant = scoped_grant(
        &session,
        "grant-lineage",
        vec!["input"],
        vec![AiMutationKind::ProjectPatch],
    );
    let mut kernel = AiCapabilityToolKernel::new();

    let first_input = input_candidate(&session, "lineage-one", "action.lineage-one");
    let first_invocation = candidate_invocation(&session, "invoke-lineage-one", first_input);
    let first = kernel.execute(&mut session, first_invocation, &grant);
    assert_eq!(first.status, AiToolExecutionStatus::Completed);

    let second_input = input_candidate(&session, "lineage-two", "action.lineage-two");
    let second_invocation = candidate_invocation(&session, "invoke-lineage-two", second_input);
    let second = kernel.execute(&mut session, second_invocation, &grant);
    assert_eq!(second.status, AiToolExecutionStatus::Completed);

    let lineage = kernel
        .inspect(
            &session,
            AiToolInspectRequest {
                schema_version: AI_TOOL_INSPECT_REQUEST_SCHEMA_VERSION.to_string(),
                kind: AiToolInspectKind::GrantLineage {
                    grant_digest: grant.grant_digest.clone(),
                },
            },
        )
        .unwrap();
    let AiToolInspectPayload::GrantLineage(Some(lineage)) = lineage.payload else {
        panic!("grant lineage should be recorded");
    };
    assert_eq!(lineage.mutation_count, 2);
    assert_eq!(lineage.receipt_digests.len(), 2);

    let mut reopened = AiCapabilityToolKernel::new();
    reopened
        .inspect(&session, AiToolInspectRequest::project())
        .unwrap();
    let observed = reopened.observe(&first.operation_id).unwrap();
    assert_eq!(observed.state, AiToolOperationState::Completed);

    fs::write(root.join("untracked-authoring-drift.txt"), "outside grant").unwrap();
    let drifted_input = input_candidate(&session, "lineage-drift", "action.lineage-drift");
    let drifted_invocation = candidate_invocation(&session, "invoke-lineage-drift", drifted_input);
    let drifted = reopened.execute(&mut session, drifted_invocation, &grant);
    assert_eq!(diagnostic_code(&drifted), "ai_tool.grant_lineage_drifted");
}

#[test]
fn ai_capability_tool_kernel_rejects_mutation_beyond_goal_grant_budget() {
    let (mut session, _) = created_session("ToolMutationBudget");
    let mut grant = scoped_grant(
        &session,
        "grant-mutation-budget",
        vec!["input"],
        vec![AiMutationKind::ProjectPatch],
    );
    grant.max_mutation_count = 1;
    let grant = grant.seal().unwrap();
    let mut kernel = AiCapabilityToolKernel::new();

    let first_input = input_candidate(&session, "budget-first", "action.budget-first");
    let first_invocation = candidate_invocation(&session, "invoke-budget-first", first_input);
    let first = kernel.execute(&mut session, first_invocation, &grant);
    assert_eq!(first.status, AiToolExecutionStatus::Completed);

    let second_input = input_candidate(&session, "budget-second", "action.budget-second");
    let second_invocation = candidate_invocation(&session, "invoke-budget-second", second_input);
    let second = kernel.execute(&mut session, second_invocation, &grant);
    assert_eq!(
        diagnostic_code(&second),
        "ai_tool.mutation_budget_exhausted"
    );

    let lineage = kernel
        .grant_lineage(&grant.grant_digest)
        .expect("the accepted mutation must retain its lineage");
    assert_eq!(lineage.mutation_count, 1);
    assert_eq!(lineage.receipt_digests.len(), 1);
}

#[test]
fn ai_capability_tool_kernel_low_risk_candidate_can_rollback_with_the_same_grant() {
    let (mut session, _) = created_session("ToolRollback");
    let mapping_path = "Input/input.rollback.json";
    let create_mapping = session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: mapping_path.to_string(),
        },
    ));
    assert_eq!(create_mapping.status, CommandStatus::Committed);
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let before = binding.project_digest.clone();
    let grant = AiCapabilityGrant::project_owned_low_risk(
        "grant-rollback",
        binding.project_id,
        "sha256:rollback-outcome",
        binding.project_digest,
        "local-user",
    )
    .unwrap();
    let mut kernel = AiCapabilityToolKernel::new();
    let input = input_candidate_at(&session, "rollback-source", "action.rollback", mapping_path);
    let invocation = candidate_invocation(&session, "invoke-rollback-source", input);
    let applied = kernel.execute(&mut session, invocation, &grant);
    assert_eq!(applied.status, AiToolExecutionStatus::Completed);
    let receipt = applied_receipt(&applied);
    let current = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let rollback = kernel.execute(
        &mut session,
        AiToolInvocation {
            schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
            invocation_id: "invoke-rollback".to_string(),
            tool_id: TOOL_ID_PROJECT_ROLLBACK.to_string(),
            expected_project_digest: current.project_digest,
            payload: AiToolInvocationPayload::RollbackCandidate { receipt },
        },
        &grant,
    );
    assert_eq!(
        rollback.status,
        AiToolExecutionStatus::Completed,
        "rollback diagnostics: {:?}",
        rollback.diagnostics
    );
    assert_eq!(
        ProjectCandidateEntry::inspect_project_binding(&session)
            .unwrap()
            .project_digest,
        before
    );
}

#[test]
fn ai_capability_tool_kernel_preview_requires_project_owned_runtime_binding() {
    let (mut empty_session, _) = created_session("ToolPreviewEmpty");
    let empty_binding = ProjectCandidateEntry::inspect_project_binding(&empty_session).unwrap();
    let read_grant = AiCapabilityGrant::read(
        "grant-preview-empty",
        empty_binding.project_id,
        empty_binding.project_digest,
        "local-user",
    )
    .unwrap();
    let mut kernel = AiCapabilityToolKernel::new();
    let invocation = preview_invocation(&empty_session, "invoke-preview-empty");
    let AiToolStartOutcome::Accepted(preview) =
        kernel.start(&empty_session, invocation, &read_grant)
    else {
        panic!("linked empty-project Preview must be accepted");
    };
    kernel.pump_operations(&mut empty_session, 3);
    let operation = kernel.observe(&preview.operation_id).unwrap();
    assert_eq!(operation.state, AiToolOperationState::Running);
    assert_eq!(operation.stage, "awaiting_frame_evidence");
    assert!(empty_session
        .pending_project_preview_frame_ticket()
        .is_some());

    let (mut project_rust_session, _) = created_session("ToolPreviewProjectRust");
    let runtime = &mut project_rust_session
        .active_project_session
        .as_mut()
        .unwrap()
        .manifest
        .runtime_module;
    runtime.source_kind = Some(ProjectRuntimeSourceKind::ProjectRust);
    runtime.module_id = "project.runtime.not-linked".to_string();
    runtime.cargo_package = "project_runtime_not_linked".to_string();
    runtime.player_binary = "project_runtime_not_linked_player".to_string();
    let binding = ProjectCandidateEntry::inspect_project_binding(&project_rust_session).unwrap();
    let read_grant = AiCapabilityGrant::read(
        "grant-preview-project-rust",
        binding.project_id,
        binding.project_digest,
        "local-user",
    )
    .unwrap();
    let invocation = preview_invocation(&project_rust_session, "invoke-preview-project-rust");
    let mut kernel = AiCapabilityToolKernel::new();
    let AiToolStartOutcome::Accepted(preview) =
        kernel.start(&project_rust_session, invocation, &read_grant)
    else {
        panic!("runtime binding is checked during asynchronous prepare");
    };
    kernel.pump_operations(&mut project_rust_session, 3);
    let preview = kernel
        .observe(&preview.operation_id)
        .unwrap()
        .result
        .expect("unlinked runtime Preview must fail");
    assert_eq!(preview.status, AiToolExecutionStatus::Failed);
    assert_eq!(
        diagnostic_code(&preview),
        "ai_tool.preview_project_runtime_not_linked"
    );
}

struct UnrelatedProjectRuntimeModule {
    descriptor: ProjectRuntimeModuleDescriptor,
}

impl UnrelatedProjectRuntimeModule {
    fn new() -> Self {
        Self {
            descriptor: ProjectRuntimeModuleDescriptor::new(
                "project.runtime.unrelated",
                "sha256:unrelated-runtime",
            ),
        }
    }
}

impl ProjectRuntimeModule for UnrelatedProjectRuntimeModule {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        _registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeError> {
        Ok(())
    }
}

#[test]
fn ai_capability_tool_kernel_preview_rejects_multi_module_host() {
    let (mut session, _) = created_session("ToolPreviewMultiModuleHost");
    let mut linked = LinkedProjectRuntimeSet::explicit_empty();
    linked
        .add(Arc::new(UnrelatedProjectRuntimeModule::new()))
        .unwrap();
    session.linked_project_runtimes = Arc::new(linked);
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let read_grant = AiCapabilityGrant::read(
        "grant-preview-multi-module",
        binding.project_id,
        binding.project_digest,
        "local-user",
    )
    .unwrap();
    let invocation = preview_invocation(&session, "invoke-preview-multi-module");

    let mut kernel = AiCapabilityToolKernel::new();
    let AiToolStartOutcome::Accepted(preview) = kernel.start(&session, invocation, &read_grant)
    else {
        panic!("multi-module Preview must be accepted");
    };
    kernel.pump_operations(&mut session, 3);
    let operation = kernel.observe(&preview.operation_id).unwrap();
    assert_eq!(operation.state, AiToolOperationState::Failed);
    assert_eq!(operation.stage, "terminal");
    assert_eq!(
        operation
            .result
            .as_ref()
            .and_then(|result| result.diagnostics.first())
            .map(|diagnostic| diagnostic.code.as_str()),
        Some("ai_tool.preview_failed")
    );
    assert!(session.pending_project_preview_frame_ticket().is_none());
}

#[test]
fn project_intent_workflow_mutation_lane_excludes_previewing() {
    assert!(ProjectProductionRunState::Executing.holds_mutation_lane());
    assert!(!ProjectProductionRunState::Previewing.holds_mutation_lane());
}
