use super::fixtures::*;
use super::*;
use std::io::Write;
use std::thread;
use std::time::Duration;

fn opened_editor_scene_session(scene_path: &Path) -> EditorSession {
    let project_root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "AI Candidate Test".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);
    let project_scene_path = project_root.join("Scenes").join("Main.scene.json");
    std::fs::copy(scene_path, &project_scene_path).expect("copy scene fixture into owned project");
    let open = session.open_scene_document_for_test(&project_scene_path);
    assert_eq!(open.status, CommandStatus::Committed);
    session
}

fn pump_llm_until_settled(session: &mut EditorSession) {
    for _ in 0..200 {
        let _ = session.pump_llm_patch_request();
        if !session.has_active_llm_patch_request() {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("LLM patch request did not settle");
}

#[test]
fn editor_session_ai_prompt_proposes_command() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));

    let result = session.execute_command(command_for_test(UiCommandPayload::AiSubmitPrompt {
        prompt: "重命名为 Hero".to_string(),
    }));
    let model = session.build_ui_model();

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(model.ai_panel.proposed_commands.len(), 1);
    assert!(matches!(
        model.ai_panel.proposed_commands[0].command,
        UiCommandPayload::RenameSceneEntity { .. }
    ));
}

#[test]
fn ai_project_patch_create_prompt_is_project_patch_planned() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(UiCommandPayload::AiSubmitPrompt {
        prompt: "创建 \"Patch Planned Entity\"".to_string(),
    }));
    let model = session.build_ui_model();

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(model.ai_panel.proposed_commands.len(), 1);
    assert_eq!(
        model.ai_panel.proposed_commands[0].explanation,
        "Create a general empty scene entity from a ProjectPatch plan."
    );
    let evidence = model.ai_panel.proposed_commands[0]
        .project_patch
        .as_ref()
        .expect("create proposal should expose ProjectPatch evidence");
    assert_eq!(evidence.patch_id, "ai-patch-1-create");
    assert_eq!(evidence.touched_domains, vec!["Scene".to_string()]);
    assert_eq!(evidence.operation_count, 1);
    assert!(evidence.validation_status);
    assert!(matches!(
        &model.ai_panel.proposed_commands[0].command,
        UiCommandPayload::CreateSceneEntity { name, .. } if name == "Patch Planned Entity"
    ));
}

#[test]
fn editor_session_accept_ai_project_patch_executes_patch_transaction() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_command(command_for_test(UiCommandPayload::AiSubmitPrompt {
        prompt: "创建 \"Patch Accepted Entity\"".to_string(),
    }));
    let proposal_id = session.build_ui_model().ai_panel.proposed_commands[0]
        .proposal_id
        .clone();

    let result = session.execute_command(command_for_test(
        UiCommandPayload::AiAcceptProposedCommand { proposal_id },
    ));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(result.command_id, "ai_accept_project_patch");
    assert_eq!(session.patch_history().entries.len(), 1);
    assert!(session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-patch-accepted-entity"));
}

#[test]
fn imported_project_patch_preview_stages_ai_panel_proposal_without_mutation() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let patch = ProjectPatchDocument::new(
        "imported-preview",
        "Imported preview",
        PatchSource::ImportedPatch,
        vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: "op-imported-preview".to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: "Imported Preview Entity".to_string(),
        })],
    );
    let raw_json = serde_json::to_string(&patch).unwrap();

    let result = session.execute_command(command_for_test(
        UiCommandPayload::PreviewImportedProjectPatch {
            source_label: "test-json".to_string(),
            raw_json: Some(raw_json),
            file_path: None,
            expected_patch_id: Some("imported-preview".to_string()),
        },
    ));
    pump_llm_until_settled(&mut session);
    let model = session.build_ui_model();

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.patch_history().entries.len(), 0);
    assert_eq!(model.ai_panel.proposed_commands.len(), 1);
    assert!(matches!(
        model.ai_panel.proposed_commands[0].command,
        UiCommandPayload::ApplyImportedProjectPatch { .. }
    ));
    assert!(model.ai_panel.proposed_commands[0]
        .imported_project_patch
        .as_ref()
        .is_some_and(|evidence| evidence.parse_status == "Parsed"
            && evidence.validation_status == Some(true)));
    assert!(!model
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-imported-preview-entity"));
}

#[test]
fn apply_imported_project_patch_executes_patch_transaction() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let patch = ProjectPatchDocument::new(
        "imported-apply",
        "Imported apply",
        PatchSource::ImportedPatch,
        vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: "op-imported-apply".to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: "Imported Apply Entity".to_string(),
        })],
    );
    let raw_json = serde_json::to_string(&patch).unwrap();
    session.execute_command(command_for_test(UiCommandPayload::ImportProjectPatch {
        source_label: "test-json".to_string(),
        raw_json: Some(raw_json),
        file_path: None,
        expected_patch_id: Some("imported-apply".to_string()),
        dry_run: true,
    }));
    let proposal_id = session.build_ui_model().ai_panel.proposed_commands[0]
        .proposal_id
        .clone();

    let result = session.execute_command(command_for_test(
        UiCommandPayload::ApplyImportedProjectPatch { proposal_id },
    ));

    assert_eq!(
        result.status,
        CommandStatus::Committed,
        "{:#?}",
        result.diagnostics
    );
    assert_eq!(result.command_id, "apply_imported_project_patch");
    assert_eq!(session.patch_history().entries.len(), 1);
    assert!(session
        .build_ui_model()
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-imported-apply-entity"));
}

#[test]
fn imported_project_patch_file_drift_is_rejected_before_apply() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let patch = ProjectPatchDocument::new(
        "imported-file-drift",
        "Imported file drift",
        PatchSource::ImportedPatch,
        vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: "op-imported-file-drift".to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: "Imported File Drift Entity".to_string(),
        })],
    );
    let source_path = unique_editor_project_temp_dir().with_extension("json");
    fs::write(&source_path, serde_json::to_vec(&patch).unwrap()).unwrap();
    let preview = session.execute_command(command_for_test(
        UiCommandPayload::PreviewImportedProjectPatch {
            source_label: "test-file".to_string(),
            raw_json: None,
            file_path: Some(source_path.display().to_string()),
            expected_patch_id: Some("imported-file-drift".to_string()),
        },
    ));
    assert_eq!(preview.status, CommandStatus::Committed);
    let proposal_id = session.build_ui_model().ai_panel.proposed_commands[0]
        .proposal_id
        .clone();
    fs::OpenOptions::new()
        .append(true)
        .open(&source_path)
        .unwrap()
        .write_all(b"\n")
        .unwrap();

    let apply = session.execute_command(command_for_test(
        UiCommandPayload::ApplyImportedProjectPatch { proposal_id },
    ));
    assert_eq!(apply.status, CommandStatus::Failed);
    assert!(apply
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_candidate_entry.source_drifted"));
    assert!(session.patch_history().entries.is_empty());
}

#[test]
fn provider_and_imported_project_patch_share_candidate_entry_and_apply_path() {
    let provider_scene = write_editor_scene_fixture();
    let mut provider_session = opened_editor_scene_session(&provider_scene);
    provider_session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create provider candidate entity".to_string(),
        },
    ));
    pump_llm_until_settled(&mut provider_session);
    let provider_proposal = provider_session.project_candidate_proposals[0].clone();
    let provider_change = provider_session
        .project_intent_snapshot()
        .unwrap()
        .active_proposal
        .unwrap();
    assert_eq!(
        provider_change.candidate_plan_steps[0].source_kind,
        ProjectCandidateSourceKind::BuiltInProvider
    );

    let imported_scene = write_editor_scene_fixture();
    let mut imported_session = opened_editor_scene_session(&imported_scene);
    let imported_patch = ProjectPatchDocument::new(
        "imported-common-entry",
        "Imported common entry",
        PatchSource::ImportedPatch,
        vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: "op-imported-common-entry".to_string(),
            depends_on: Vec::new(),
            parent_id: None,
            name: "Imported Common Entry Entity".to_string(),
        })],
    );
    imported_session.execute_command(command_for_test(
        UiCommandPayload::PreviewImportedProjectPatch {
            source_label: "imported-common-entry".to_string(),
            raw_json: Some(serde_json::to_string(&imported_patch).unwrap()),
            file_path: None,
            expected_patch_id: Some(imported_patch.patch_id.clone()),
        },
    ));
    let imported_proposal = imported_session.project_candidate_proposals[0].clone();
    let imported_change = imported_session
        .project_intent_snapshot()
        .unwrap()
        .active_proposal
        .unwrap();
    assert_eq!(
        imported_change.candidate_plan_steps[0].source_kind,
        ProjectCandidateSourceKind::ImportedCodex
    );
    assert!(matches!(
        (
            &provider_change.candidate_plan_steps[0].payload,
            &imported_change.candidate_plan_steps[0].payload
        ),
        (
            ProjectCandidatePayload::ProjectPatch(_),
            ProjectCandidatePayload::ProjectPatch(_)
        )
    ));

    let provider_apply = provider_session.execute_command(command_for_test(
        UiCommandPayload::ApplyImportedProjectPatch {
            proposal_id: provider_proposal.proposal_id,
        },
    ));
    let imported_apply = imported_session.execute_command(command_for_test(
        UiCommandPayload::ApplyImportedProjectPatch {
            proposal_id: imported_proposal.proposal_id,
        },
    ));
    assert_eq!(
        provider_apply.status,
        CommandStatus::Committed,
        "{:#?}",
        provider_apply.diagnostics
    );
    assert_eq!(
        imported_apply.status,
        CommandStatus::Committed,
        "{:#?}",
        imported_apply.diagnostics
    );
    assert_eq!(provider_apply.command_id, imported_apply.command_id);
    for session in [&provider_session, &imported_session] {
        let run = session
            .project_intent_snapshot()
            .unwrap()
            .active_run
            .unwrap();
        let receipt = run.step_snapshots[0].apply_receipt.as_ref().unwrap();
        assert!(!receipt.candidate_digest.is_empty());
        assert!(!receipt.validation_digest.is_empty());
    }
}

#[test]
fn imported_project_patch_invalid_json_rejected_without_mutation() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(
        UiCommandPayload::PreviewImportedProjectPatch {
            source_label: "bad-json".to_string(),
            raw_json: Some("{not-json".to_string()),
            file_path: None,
            expected_patch_id: None,
        },
    ));

    assert_eq!(result.status, CommandStatus::Rejected);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "project_patch_import.parse_failed"));
    assert!(session.patch_history().entries.is_empty());
    assert!(session
        .build_ui_model()
        .ai_panel
        .proposed_commands
        .is_empty());
}

#[test]
fn generate_project_patch_from_prompt_stages_ai_structured_output_without_mutation() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create \"LLM Preview Entity\"".to_string(),
        },
    ));
    pump_llm_until_settled(&mut session);
    let model = session.build_ui_model();

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.patch_history().entries.len(), 0);
    assert_eq!(model.ai_panel.proposed_commands.len(), 1);
    let proposal = &model.ai_panel.proposed_commands[0];
    assert!(matches!(
        proposal.command,
        UiCommandPayload::ApplyImportedProjectPatch { .. }
    ));
    assert!(proposal
        .imported_project_patch
        .as_ref()
        .is_some_and(|evidence| evidence.source_kind == "AiStructuredOutput"
            && evidence.parse_status == "Parsed"
            && evidence.validation_status == Some(true)));
    assert!(!model
        .hierarchy
        .roots
        .iter()
        .any(|node| node.entity_id == "entity-llm-preview-entity"));
}

#[test]
fn llm_repair_loop_repairs_initial_invalid_json_once_without_mutation() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "invalid_json".to_string(),
        },
    ));

    assert_eq!(result.status, CommandStatus::Committed);
    pump_llm_until_settled(&mut session);
    assert!(session.patch_history().entries.is_empty());
    let model = session.build_ui_model();
    assert_eq!(model.ai_panel.proposed_commands.len(), 1);
    assert!(model
        .ai_panel
        .status_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("repaired once")));
}

#[test]
fn generate_project_patch_from_prompt_provider_error_returns_diagnostic_only() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "provider_error".to_string(),
        },
    ));

    assert_eq!(result.status, CommandStatus::Committed);
    pump_llm_until_settled(&mut session);
    assert!(session
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "llm_patch_source.provider_error"));
    assert!(session.patch_history().entries.is_empty());
    assert!(session
        .build_ui_model()
        .ai_panel
        .proposed_commands
        .is_empty());
}

#[test]
fn llm_background_request_returns_before_blocked_http_and_pumps_proposal() {
    use super::llm_patch_source_tests::{
        gated_fake_http_server, openai_config, success_envelope, FakeHttpResponse,
    };

    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let (base_url, requests, release_response) = gated_fake_http_server(FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: success_envelope(),
        delay_ms: 0,
    });
    session.set_llm_patch_source_config_for_test(openai_config(base_url));
    let result = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create background entity".to_string(),
        },
    ));

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(session.has_active_llm_patch_request());
    assert!(session.build_ui_model().ai_panel.busy);
    requests
        .recv_timeout(Duration::from_secs(1))
        .expect("background transport must reach the gated HTTP server");
    release_response
        .send(())
        .expect("command returned before the gated response was released");
    pump_llm_until_settled(&mut session);
    let model = session.build_ui_model();
    assert!(!model.ai_panel.busy);
    assert_eq!(
        model.ai_panel.stage,
        editor_ui_model::AiPanelStage::Reviewing
    );
    assert_eq!(model.ai_panel.proposed_commands.len(), 1);
}

#[test]
fn llm_cancel_drops_late_provider_result_and_duplicate_submit_is_rejected() {
    use super::llm_patch_source_tests::{
        gated_fake_http_server, openai_config, success_envelope, FakeHttpResponse,
    };

    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let (base_url, requests, release_response) = gated_fake_http_server(FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: success_envelope(),
        delay_ms: 0,
    });
    session.set_llm_patch_source_config_for_test(openai_config(base_url));
    let first = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create cancellable entity".to_string(),
        },
    ));
    requests
        .recv_timeout(Duration::from_secs(1))
        .expect("cancellable transport must reach the gated HTTP server");
    let duplicate = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "duplicate".to_string(),
        },
    ));
    let cancel = session.execute_command(command_for_test(UiCommandPayload::CancelLlmPatchRequest));

    assert_eq!(first.status, CommandStatus::Committed);
    assert_eq!(duplicate.status, CommandStatus::Rejected);
    assert!(duplicate
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "llm_patch_source.request_busy"));
    assert_eq!(cancel.status, CommandStatus::Committed);
    assert_eq!(
        session.build_ui_model().ai_panel.stage,
        editor_ui_model::AiPanelStage::Cancelling
    );
    assert!(session.has_active_llm_patch_request());
    let resubmit_while_cancelling = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "resubmit while cancelling".to_string(),
        },
    ));
    assert_eq!(resubmit_while_cancelling.status, CommandStatus::Rejected);
    pump_llm_until_settled(&mut session);
    assert!(!session.has_active_llm_patch_request());
    assert_eq!(
        session.build_ui_model().ai_panel.stage,
        editor_ui_model::AiPanelStage::Cancelled
    );
    let report = session.last_llm_patch_report.as_ref().unwrap();
    assert_eq!(report.lifecycle_state, LlmLifecycleState::CancelledJoined);
    assert_eq!(report.task_join_status, LlmTaskJoinStatus::Joined);
    assert_eq!(
        report.credential_owner_status,
        CredentialOwnerStatus::Released
    );
    assert!(report.transport_abort_observed);
    assert!(session
        .build_ui_model()
        .ai_panel
        .proposed_commands
        .is_empty());
    drop(release_response);
}

#[test]
fn llm_session_shutdown_cancels_joins_and_drains_executor() {
    use super::llm_patch_source_tests::{
        fake_http_server, openai_config, success_envelope, FakeHttpResponse,
    };

    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    let (base_url, _) = fake_http_server(vec![FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: success_envelope(),
        delay_ms: 500,
    }]);
    session.set_llm_patch_source_config_for_test(openai_config(base_url));
    session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "shutdown active request".to_string(),
        },
    ));

    let receipt = session.shutdown_llm(Duration::from_secs(2));

    assert_eq!(receipt.task_join_status, LlmTaskJoinStatus::Joined);
    assert_eq!(receipt.active_task_count, 0);
    assert_eq!(receipt.reaper_count, 0);
    assert!(receipt.diagnostic.is_none());
    assert!(!session.has_active_llm_patch_request());
    assert_eq!(
        session
            .last_llm_shutdown_receipt()
            .map(|receipt| receipt.state),
        Some(LlmLifecycleState::CancelledJoined)
    );
}

#[test]
fn llm_context_stale_rejects_semantic_change_but_allows_revision_only_change() {
    use super::llm_patch_source_tests::{
        fake_http_server, openai_config, success_envelope, FakeHttpResponse,
    };

    let scene_path = write_editor_scene_fixture();
    let mut stale_session = opened_editor_scene_session(&scene_path);
    let (stale_url, _) = fake_http_server(vec![FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: success_envelope(),
        delay_ms: 50,
    }]);
    stale_session.set_llm_patch_source_config_for_test(openai_config(stale_url));
    stale_session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create stale entity".to_string(),
        },
    ));
    stale_session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));
    pump_llm_until_settled(&mut stale_session);
    assert!(stale_session
        .build_ui_model()
        .ai_panel
        .proposed_commands
        .is_empty());
    assert!(stale_session
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "llm_patch_source.context_stale"));

    let mut stable_session = opened_editor_scene_session(&scene_path);
    let (stable_url, _) = fake_http_server(vec![FakeHttpResponse {
        status: 200,
        headers: Vec::new(),
        body: success_envelope(),
        delay_ms: 50,
    }]);
    stable_session.set_llm_patch_source_config_for_test(openai_config(stable_url));
    stable_session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create stable entity".to_string(),
        },
    ));
    stable_session.execute_command(command_for_test(UiCommandPayload::SetAiPromptDraft {
        prompt: "revision-only UI state".to_string(),
    }));
    pump_llm_until_settled(&mut stable_session);
    assert_eq!(
        stable_session
            .build_ui_model()
            .ai_panel
            .proposed_commands
            .len(),
        1
    );
}

#[test]
fn ai_project_patch_review_exposes_risk_confirmation_and_repair_state() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "invalid_json".to_string(),
        },
    ));
    pump_llm_until_settled(&mut session);

    let model = session.build_ui_model();
    let evidence = model.ai_panel.proposed_commands[0]
        .project_patch
        .as_ref()
        .unwrap();
    assert_eq!(evidence.risk_level, "Low");
    assert!(evidence.repaired_once);
    assert!(evidence.requires_confirmation);
    assert!(session.patch_history().entries.is_empty());
    let repair_scope = session
        .last_llm_patch_report
        .as_ref()
        .and_then(|report| report.repair_scope.as_ref())
        .expect("repair report must include bounded scope evidence");
    assert_eq!(
        repair_scope.status,
        RepairScopeValidationStatus::ScopeUnprovableRestricted
    );
    assert_eq!(repair_scope.initial_operation_count, None);
    assert_eq!(repair_scope.repaired_operation_count, 1);
}

#[test]
fn report_panel_llm_summary_trace_and_off_are_redacted_and_tiered() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.set_llm_patch_report_level(LlmPatchReportLevel::Trace);
    session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "create report entity".to_string(),
        },
    ));
    pump_llm_until_settled(&mut session);
    let model = session.build_ui_model();
    let report = model
        .report_panel
        .reports
        .iter()
        .find(|report| report.provider_id == "project.patch")
        .unwrap();
    assert!(report.evidence.len() >= 2);
    let encoded = serde_json::to_string(report).unwrap();
    assert!(!encoded.contains("gate-b-secret"));
    assert!(!encoded.contains("G:\\"));

    session.set_llm_patch_report_level(LlmPatchReportLevel::Off);
    let model = session.build_ui_model();
    let report = model
        .report_panel
        .reports
        .iter()
        .find(|report| report.provider_id == "project.patch")
        .expect("ProjectPatch history report remains registered while LLM evidence is Off");
    assert!(report
        .evidence
        .iter()
        .all(|evidence| !evidence.evidence_id.starts_with("project_patch.llm")));
}

#[test]
fn llm_privacy_report_v3_excludes_request_owned_data() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.set_llm_patch_report_level(LlmPatchReportLevel::Trace);
    session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "private prompt value must not be in report".to_string(),
        },
    ));
    pump_llm_until_settled(&mut session);
    let encoded = serde_json::to_string(session.last_llm_patch_report().unwrap()).unwrap();
    assert!(!encoded.contains("private prompt value must not be in report"));
    assert!(!encoded.contains("Authorization"));
    assert!(!encoded.contains("api_key"));
}

#[test]
fn editor_session_accept_ai_command_executes_scene_edit() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);
    session.execute_command(command_for_test(UiCommandPayload::SelectSceneEntity {
        entity_id: "entity-player".to_string(),
    }));
    session.execute_command(command_for_test(UiCommandPayload::AiSubmitPrompt {
        prompt: "重命名为 Hero".to_string(),
    }));
    let proposal_id = session.build_ui_model().ai_panel.proposed_commands[0]
        .proposal_id
        .clone();

    let result = session.execute_command(command_for_test(
        UiCommandPayload::AiAcceptProposedCommand { proposal_id },
    ));

    assert_eq!(result.status, CommandStatus::Committed);
    assert_eq!(session.build_ui_model().hierarchy.roots[0].label, "Hero");
}

#[test]
fn ai_scene_edit_command_uses_editor_session_transaction_path() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_scene_edit_for_test(SceneEditCommand::CreateEntity {
        parent_id: None,
        name: "AiSpawned".to_string(),
        mesh: None,
        components: Vec::new(),
        local_transform: EditorTransform::identity(),
        sibling_order: None,
    });

    assert_eq!(result.status, CommandStatus::Committed);
    assert!(result.command_id.starts_with("scene_edit_"));
    assert!(session
        .last_scene_edit_report
        .as_ref()
        .expect("scene edit report")
        .write_set
        .iter()
        .any(|path| path.contains("scene.entities.entity-aispawned")));
}

#[test]
fn ai_scene_edit_command_failure_reports_console_diagnostic() {
    let scene_path = write_editor_scene_fixture();
    let mut session = opened_editor_scene_session(&scene_path);

    let result = session.execute_scene_edit_for_test(SceneEditCommand::DeleteEntity {
        entity_id: "missing".to_string(),
        delete_children: true,
    });

    assert_eq!(result.status, CommandStatus::Rejected);
    assert!(session.build_ui_model().console.unread_error_count > 0);
}
