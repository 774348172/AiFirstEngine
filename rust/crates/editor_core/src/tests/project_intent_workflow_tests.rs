use super::*;
use editor_ui_model::InputActionValueKind;
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};

fn capture(workflow: &mut ProjectIntentWorkflow, command: &str, summary: &str) -> String {
    workflow
        .capture(IntentCaptureInput {
            command_id: command.to_string(),
            project_identity: None,
            occurred_at: Some("test-time".to_string()),
            source_kind: IntentSourceKind::UserMessage,
            source_identity: "test-user".to_string(),
            content_ref: None,
            sanitized_summary: summary.to_string(),
            attachment_refs: Vec::new(),
            related_event_ids: Vec::new(),
            privacy_class: IntentPrivacyClass::LocalOnly,
        })
        .unwrap()
        .event_id
}

fn ready_draft(event_id: &str, title: &str) -> WorkItemDraft {
    WorkItemDraft {
        kind: WorkItemKind::Change,
        title: title.to_string(),
        user_visible_outcome: format!("Outcome for {title}"),
        source_event_ids: vec![event_id.to_string()],
        status: WorkItemStatus::Ready,
        priority: WorkItemPriority::Normal,
        scope_hints: Vec::new(),
        constraints: Vec::new(),
        acceptance_criteria: vec![format!("{title} is visible")],
        open_questions: Vec::new(),
        evidence_refs: Vec::new(),
        relationship_refs: Vec::new(),
        latest_understanding: format!("Implement {title}"),
        explicitly_deferred: Vec::new(),
    }
}

fn payload_digest(payload: &ProjectCandidatePayload) -> String {
    let value = serde_json::to_value(payload).unwrap();
    sha256_prefixed(&canonical_json_bytes(&value).unwrap())
}

fn empty_patch_step(step_id: &str) -> CandidatePlanStep {
    let payload = ProjectCandidatePayload::ProjectPatch(ProjectPatchDocument::new(
        format!("patch-{step_id}"),
        format!("Patch {step_id}"),
        PatchSource::Test,
        Vec::new(),
    ));
    CandidatePlanStep {
        step_id: step_id.to_string(),
        depends_on: Vec::new(),
        payload_kind: CandidatePayloadKind::ProjectPatch,
        payload_source_digest: payload_digest(&payload),
        source_kind: ProjectCandidateSourceKind::ImportedCodex,
        source_label: "workflow-test".to_string(),
        payload,
        validation_profile: CandidateValidationProfile {
            controlled_source_patch: None,
            source_file_path: None,
            expected_source_digest: None,
        },
        expected_changed_domains: vec!["project".to_string()],
        user_visible_outcome: format!("Apply {step_id}"),
        failure_policy: "stop_and_review".to_string(),
    }
}

fn prepare_existing(
    workflow: &mut ProjectIntentWorkflow,
    binding: &ProjectCandidateProjectBinding,
    work_item_ids: Vec<String>,
    command_id: &str,
) -> ChangeSetProposal {
    let result = workflow
        .prepare_change(ChangePreparationRequest {
            command_id: command_id.to_string(),
            target_kind: ChangeSetTargetKind::ExistingProject,
            target_project_identity: Some(binding.project_id.clone()),
            project_create_spec: None,
            expected_base_project_digest: Some(binding.project_digest.clone()),
            selected_work_item_ids: work_item_ids,
            explicit_exclusions: Vec::new(),
            candidate_plan_steps: vec![empty_patch_step("one")],
            acceptance_checks: vec!["preview".to_string()],
            estimated_external_waits: Vec::new(),
            external_costs: Vec::new(),
            risks: Vec::new(),
            required_decisions: Vec::new(),
            repair_policy: "deterministic_only".to_string(),
        })
        .unwrap();
    let ChangePreparationResult::Ready(proposal) = result else {
        panic!("proposal should be ready: {result:?}");
    };
    proposal
}

fn approval_input(
    proposal: &ChangeSetProposal,
    target: String,
    base: Option<String>,
    command_id: &str,
) -> ChangeSetApprovalInput {
    ChangeSetApprovalInput {
        command_id: command_id.to_string(),
        approval_id: format!("approval-{command_id}"),
        approved_by: "local-user".to_string(),
        proposal_digest: proposal.proposal_digest.clone(),
        target_identity: target,
        expected_base_project_digest: base,
        approved_risk_classes: proposal.risks.clone(),
        approved_external_costs: proposal.external_costs.clone(),
        approved_repair_policy: proposal.repair_policy.clone(),
        approved_at: Some("approval-time".to_string()),
    }
}

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

#[test]
fn project_intent_workflow_capture_accepts_incomplete_and_contradictory_input() {
    let mut workflow = ProjectIntentWorkflow::in_memory().unwrap();
    let first = capture(
        &mut workflow,
        "capture-1",
        "Maybe make it faster, not sure how.",
    );
    let second = capture(
        &mut workflow,
        "capture-2",
        "Correction: perhaps slower; keep both statements for now.",
    );
    assert_ne!(first, second);
    let replay = workflow
        .capture(IntentCaptureInput {
            command_id: "capture-1".to_string(),
            project_identity: None,
            occurred_at: None,
            source_kind: IntentSourceKind::UserMessage,
            source_identity: "test-user".to_string(),
            content_ref: None,
            sanitized_summary: "different retry body is ignored by idempotent receipt".to_string(),
            attachment_refs: Vec::new(),
            related_event_ids: Vec::new(),
            privacy_class: IntentPrivacyClass::LocalOnly,
        })
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.event_id, first);
    let snapshot = workflow.observe(ProjectIntentQuery::All).unwrap();
    assert_eq!(snapshot.intent_events.len(), 2);
    assert_eq!(snapshot.pending_normalization_event_ids.len(), 2);
}

#[test]
fn project_intent_workflow_readiness_is_local_and_lineage_survives_lifecycle() {
    let mut workflow = ProjectIntentWorkflow::in_memory().unwrap();
    let vague_event = capture(
        &mut workflow,
        "capture-vague",
        "Maybe add a large mode later.",
    );
    let ready_event = capture(&mut workflow, "capture-ready", "Change the fire key now.");
    let mut vague = ready_draft(&vague_event, "Large mode");
    vague.status = WorkItemStatus::NeedsClarification;
    vague.open_questions = vec!["Which mode?".to_string()];
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-vague".to_string(),
                draft: vague,
            },
            None,
        )
        .unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-ready".to_string(),
                draft: ready_draft(&ready_event, "Fire key"),
            },
            None,
        )
        .unwrap();
    let snapshot = workflow.observe(ProjectIntentQuery::All).unwrap();
    let vague_id = snapshot.work_items[0].work_item_id.clone();
    let ready_id = snapshot.work_items[1].work_item_id.clone();
    assert!(!snapshot.work_item_summaries[0].ready);
    assert!(snapshot.work_item_summaries[1].ready);
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::ParkWorkItem {
                command_id: "park-vague".to_string(),
                work_item_id: vague_id.clone(),
            },
            None,
        )
        .unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::ResumeWorkItem {
                command_id: "resume-vague".to_string(),
                work_item_id: vague_id,
            },
            None,
        )
        .unwrap();
    let resumed = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items
        .into_iter()
        .find(|item| item.work_item_id == ready_id)
        .unwrap();
    assert_eq!(resumed.source_event_ids, vec![ready_event]);
}

#[test]
fn project_intent_workflow_diagnosis_is_read_only_until_change_set() {
    let mut workflow = ProjectIntentWorkflow::in_memory().unwrap();
    let event = capture(
        &mut workflow,
        "capture-bug",
        "Sometimes the player disappears.",
    );
    let mut bug = ready_draft(&event, "Player disappears");
    bug.kind = WorkItemKind::Bug;
    bug.status = WorkItemStatus::NeedsEvidence;
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-bug".to_string(),
                draft: bug,
            },
            None,
        )
        .unwrap();
    let work_item_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items[0]
        .work_item_id
        .clone();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::StartDiagnosis {
                command_id: "diagnosis-start".to_string(),
                work_item_id,
                base_project_digest: None,
            },
            None,
        )
        .unwrap();
    let diagnosis_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .active_diagnoses[0]
        .diagnosis_id
        .clone();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::UpdateDiagnosis {
                command_id: "diagnosis-read".to_string(),
                update: DiagnosisUpdate {
                    diagnosis_id: diagnosis_id.clone(),
                    state: DiagnosisState::Investigating,
                    reproduction_attempts: vec!["Run existing preview".to_string()],
                    observations: vec!["No project bytes changed".to_string()],
                    hypotheses: Vec::new(),
                    confirmed_cause: None,
                    evidence_refs: vec!["evidence://preview-1".to_string()],
                    proposed_fix_scope: Vec::new(),
                    requested_capabilities: vec![
                        DiagnosticCapability::ReadProject,
                        DiagnosticCapability::RunPreview,
                        DiagnosticCapability::WriteIsolatedEvidence,
                    ],
                },
            },
            None,
        )
        .unwrap();
    let before = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .journal_revision;
    let error = workflow
        .dispatch(
            ProjectIntentWorkflowCommand::UpdateDiagnosis {
                command_id: "diagnosis-instrument".to_string(),
                update: DiagnosisUpdate {
                    diagnosis_id,
                    state: DiagnosisState::Investigating,
                    reproduction_attempts: Vec::new(),
                    observations: Vec::new(),
                    hypotheses: Vec::new(),
                    confirmed_cause: None,
                    evidence_refs: Vec::new(),
                    proposed_fix_scope: vec!["Insert debug component".to_string()],
                    requested_capabilities: vec![DiagnosticCapability::AddInstrumentation],
                },
            },
            None,
        )
        .unwrap_err();
    assert_eq!(error.code, "project_intent.diagnosis_change_set_required");
    assert_eq!(
        workflow
            .observe(ProjectIntentQuery::All)
            .unwrap()
            .journal_revision,
        before
    );
}

#[test]
fn project_intent_workflow_unselected_change_keeps_approval_valid_but_selected_change_stales() {
    let (session, _) = created_session("IntentApproval");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let event_a = capture(&mut workflow, "capture-a", "Do A");
    let event_b = capture(&mut workflow, "capture-b", "Do B later");
    for (command, draft) in [
        ("work-a", ready_draft(&event_a, "A")),
        ("work-b", ready_draft(&event_b, "B")),
    ] {
        workflow
            .dispatch(
                ProjectIntentWorkflowCommand::CreateWorkItem {
                    command_id: command.to_string(),
                    draft,
                },
                None,
            )
            .unwrap();
    }
    let snapshot = workflow.observe(ProjectIntentQuery::All).unwrap();
    let selected_id = snapshot.work_items[0].work_item_id.clone();
    let unrelated_id = snapshot.work_items[1].work_item_id.clone();
    let proposal = prepare_existing(
        &mut workflow,
        &binding,
        vec![selected_id.clone()],
        "prepare-a",
    );
    let mut unrelated = ready_draft(&event_b, "B revised");
    unrelated.latest_understanding = "Discuss B later with more detail".to_string();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::ReviseWorkItem {
                command_id: "revise-b".to_string(),
                work_item_id: unrelated_id,
                draft: unrelated,
            },
            None,
        )
        .unwrap();
    let run = workflow
        .authorize(
            approval_input(
                &proposal,
                binding.project_id.clone(),
                Some(binding.project_digest.clone()),
                "approve-a",
            ),
            Some(&session),
        )
        .unwrap();
    assert_eq!(run.state, ProjectProductionRunState::Approved);

    let (session2, _) = created_session("IntentSelectedStale");
    let binding2 = ProjectCandidateEntry::inspect_project_binding(&session2).unwrap();
    let mut workflow2 = ProjectIntentWorkflow::open_project(&session2).unwrap();
    let event = capture(&mut workflow2, "capture-selected", "Do selected");
    workflow2
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-selected".to_string(),
                draft: ready_draft(&event, "Selected"),
            },
            None,
        )
        .unwrap();
    let selected_id = workflow2
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items[0]
        .work_item_id
        .clone();
    let proposal2 = prepare_existing(
        &mut workflow2,
        &binding2,
        vec![selected_id.clone()],
        "prepare-selected",
    );
    let mut revised = ready_draft(&event, "Selected changed");
    revised.user_visible_outcome = "A different visible outcome".to_string();
    workflow2
        .dispatch(
            ProjectIntentWorkflowCommand::ReviseWorkItem {
                command_id: "revise-selected".to_string(),
                work_item_id: selected_id,
                draft: revised,
            },
            None,
        )
        .unwrap();
    let error = workflow2
        .authorize(
            approval_input(
                &proposal2,
                binding2.project_id,
                Some(binding2.project_digest),
                "approve-selected",
            ),
            Some(&session2),
        )
        .unwrap_err();
    assert_eq!(error.code, "project_intent.approval_work_item_stale");
}

#[test]
fn project_intent_workflow_from_blank_waits_for_approval_then_attaches_journal_and_preview() {
    let local_root = fixtures::unique_editor_project_temp_dir();
    if local_root.exists() {
        fs::remove_dir_all(&local_root).unwrap();
    }
    let draft_path = local_root.join("launcher-state/intent-draft.json");
    let target_root = fixtures::unique_editor_project_temp_dir();
    if target_root.exists() {
        fs::remove_dir_all(&target_root).unwrap();
    }
    let mut workflow = ProjectIntentWorkflow::open_pre_project_draft(&draft_path).unwrap();
    let event = capture(
        &mut workflow,
        "capture-blank",
        "Create a tiny playable project.",
    );
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-blank".to_string(),
                draft: ready_draft(&event, "Tiny project"),
            },
            None,
        )
        .unwrap();
    let work_item_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items[0]
        .work_item_id
        .clone();
    let blank_payload = ProjectCandidatePayload::ProjectPatch(ProjectPatchDocument::new(
        "blank-input-patch",
        "Create default input mapping",
        PatchSource::Test,
        vec![PatchOperation::Input(
            InputPatchOperation::CreateDefaultInputMapping {
                operation_id: "blank-create-input".to_string(),
                depends_on: Vec::new(),
                path: "Input/input.default.json".to_string(),
            },
        )],
    ));
    let blank_step = CandidatePlanStep {
        step_id: "blank-input-step".to_string(),
        depends_on: Vec::new(),
        payload_kind: CandidatePayloadKind::ProjectPatch,
        payload_source_digest: payload_digest(&blank_payload),
        source_kind: ProjectCandidateSourceKind::ImportedCodex,
        source_label: "from-blank-test".to_string(),
        payload: blank_payload,
        validation_profile: CandidateValidationProfile {
            controlled_source_patch: None,
            source_file_path: None,
            expected_source_digest: None,
        },
        expected_changed_domains: vec!["input".to_string()],
        user_visible_outcome: "Default input mapping exists".to_string(),
        failure_policy: "stop_and_review".to_string(),
    };
    let result = workflow
        .prepare_change(ChangePreparationRequest {
            command_id: "prepare-blank".to_string(),
            target_kind: ChangeSetTargetKind::NewProject,
            target_project_identity: None,
            project_create_spec: Some(ProjectCreateSpec {
                project_root: target_root.display().to_string(),
                project_name: "Intent Blank".to_string(),
            }),
            expected_base_project_digest: None,
            selected_work_item_ids: vec![work_item_id],
            explicit_exclusions: Vec::new(),
            candidate_plan_steps: vec![blank_step],
            acceptance_checks: vec!["preview".to_string()],
            estimated_external_waits: Vec::new(),
            external_costs: Vec::new(),
            risks: Vec::new(),
            required_decisions: Vec::new(),
            repair_policy: "deterministic_only".to_string(),
        })
        .unwrap();
    let ChangePreparationResult::Ready(proposal) = result else {
        panic!("from-blank proposal should be ready");
    };
    assert!(!target_root.exists());
    let run = workflow
        .authorize(
            approval_input(
                &proposal,
                target_root.display().to_string(),
                None,
                "approve-blank",
            ),
            None,
        )
        .unwrap();
    assert!(!target_root.exists());
    let mut session = EditorSession::new();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::AdvanceRun {
                command_id: "advance-blank".to_string(),
                run_id: run.run_id,
            },
            Some(&mut session),
        )
        .unwrap();
    let after_apply = workflow.observe(ProjectIntentQuery::All).unwrap();
    let applied_run = after_apply.active_run.as_ref().unwrap();
    assert_eq!(applied_run.state, ProjectProductionRunState::Previewing);
    assert!(applied_run.step_snapshots[0].apply_receipt.is_some());
    assert!(target_root.join("Input/input.default.json").exists());
    assert!(applied_run.preview_evidence.is_none());
    assert_eq!(
        applied_run.decision_requests,
        vec!["preview_verification_required"]
    );
    assert!(target_root.join("project.aife.json").exists());
    assert!(target_root
        .join("Library/ProjectIntent/journal.json")
        .exists());
    assert!(!draft_path.exists());
    let reopened = ProjectIntentWorkflow::open_project(&session).unwrap();
    let snapshot = reopened.observe(ProjectIntentQuery::All).unwrap();
    assert!(snapshot.project_binding.is_some());
    assert_eq!(snapshot.intent_events.len(), 1);
}

#[test]
fn project_intent_workflow_existing_project_base_drift_fails_closed() {
    let (session, root) = created_session("IntentBaseDrift");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let event = capture(&mut workflow, "capture-drift", "Change one thing.");
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-drift".to_string(),
                draft: ready_draft(&event, "Drift test"),
            },
            None,
        )
        .unwrap();
    let work_item_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items[0]
        .work_item_id
        .clone();
    let proposal = prepare_existing(&mut workflow, &binding, vec![work_item_id], "prepare-drift");
    fs::write(root.join("manual-user-change.txt"), "preserve me").unwrap();
    let error = workflow
        .authorize(
            approval_input(
                &proposal,
                binding.project_id,
                Some(binding.project_digest),
                "approve-drift",
            ),
            Some(&session),
        )
        .unwrap_err();
    assert_eq!(error.code, "project_intent.approval_base_drifted");
    assert_eq!(
        fs::read_to_string(root.join("manual-user-change.txt")).unwrap(),
        "preserve me"
    );
}

#[test]
fn project_intent_workflow_candidate_execution_uses_common_entry() {
    let (mut session, _) = created_session("IntentExecution");
    let create_mapping = session.execute_command(command_for_test(
        UiCommandPayload::CreateDefaultInputMapping {
            path: "Input/input.default.json".to_string(),
        },
    ));
    assert_eq!(create_mapping.status, CommandStatus::Committed);
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let event = capture(&mut workflow, "capture-execute", "Add a dash action.");
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-execute".to_string(),
                draft: ready_draft(&event, "Dash action"),
            },
            None,
        )
        .unwrap();
    let work_item_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items[0]
        .work_item_id
        .clone();
    let payload = ProjectCandidatePayload::ProjectPatch(ProjectPatchDocument::new(
        "intent-dash-patch",
        "Add dash",
        PatchSource::Test,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: "add-dash".to_string(),
            depends_on: Vec::new(),
            path: "Input/input.default.json".to_string(),
            action_id: "action.dash".to_string(),
            value_type: InputActionValueKind::Button,
        })],
    ));
    let result = workflow
        .prepare_change(ChangePreparationRequest {
            command_id: "prepare-execute".to_string(),
            target_kind: ChangeSetTargetKind::ExistingProject,
            target_project_identity: Some(binding.project_id.clone()),
            project_create_spec: None,
            expected_base_project_digest: Some(binding.project_digest.clone()),
            selected_work_item_ids: vec![work_item_id],
            explicit_exclusions: Vec::new(),
            candidate_plan_steps: vec![CandidatePlanStep {
                step_id: "dash-step".to_string(),
                depends_on: Vec::new(),
                payload_kind: CandidatePayloadKind::ProjectPatch,
                payload_source_digest: payload_digest(&payload),
                source_kind: ProjectCandidateSourceKind::ImportedCodex,
                source_label: "workflow-execution-test".to_string(),
                payload,
                validation_profile: CandidateValidationProfile {
                    controlled_source_patch: None,
                    source_file_path: None,
                    expected_source_digest: None,
                },
                expected_changed_domains: vec!["input".to_string()],
                user_visible_outcome: "Dash action exists".to_string(),
                failure_policy: "stop_and_review".to_string(),
            }],
            acceptance_checks: vec!["input action exists".to_string()],
            estimated_external_waits: Vec::new(),
            external_costs: Vec::new(),
            risks: Vec::new(),
            required_decisions: Vec::new(),
            repair_policy: "deterministic_only".to_string(),
        })
        .unwrap();
    let ChangePreparationResult::Ready(proposal) = result else {
        panic!("execution proposal should be ready");
    };
    let run = workflow
        .authorize(
            approval_input(
                &proposal,
                binding.project_id,
                Some(binding.project_digest),
                "approve-execute",
            ),
            Some(&session),
        )
        .unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::AdvanceRun {
                command_id: "advance-execute".to_string(),
                run_id: run.run_id,
            },
            Some(&mut session),
        )
        .unwrap();
    let snapshot = workflow.observe(ProjectIntentQuery::All).unwrap();
    let run = snapshot.active_run.unwrap();
    assert_eq!(
        run.state,
        ProjectProductionRunState::Previewing,
        "run diagnostics: {:?}; steps: {:?}",
        run.diagnostics,
        run.step_snapshots
    );
    assert_eq!(run.step_snapshots[0].state, ProductionStepState::Applied);
    assert!(run.step_snapshots[0].apply_receipt.is_some());
}

#[test]
fn project_intent_workflow_selected_semantic_change_stales_active_run_and_lane_is_single() {
    let (mut session, _) = created_session("IntentRunStale");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let selected_event = capture(&mut workflow, "capture-run-selected", "Do selected now");
    let other_event = capture(&mut workflow, "capture-run-other", "Do other later");
    for (command, draft) in [
        (
            "work-run-selected",
            ready_draft(&selected_event, "Run selected"),
        ),
        ("work-run-other", ready_draft(&other_event, "Run other")),
    ] {
        workflow
            .dispatch(
                ProjectIntentWorkflowCommand::CreateWorkItem {
                    command_id: command.to_string(),
                    draft,
                },
                None,
            )
            .unwrap();
    }
    let snapshot = workflow.observe(ProjectIntentQuery::All).unwrap();
    let selected_id = snapshot.work_items[0].work_item_id.clone();
    let other_id = snapshot.work_items[1].work_item_id.clone();
    let proposal = prepare_existing(
        &mut workflow,
        &binding,
        vec![selected_id.clone()],
        "prepare-run-selected",
    );
    let run = workflow
        .authorize(
            approval_input(
                &proposal,
                binding.project_id.clone(),
                Some(binding.project_digest.clone()),
                "approve-run-selected",
            ),
            Some(&session),
        )
        .unwrap();
    let blocked = workflow
        .prepare_change(ChangePreparationRequest {
            command_id: "prepare-run-other".to_string(),
            target_kind: ChangeSetTargetKind::ExistingProject,
            target_project_identity: Some(binding.project_id.clone()),
            project_create_spec: None,
            expected_base_project_digest: Some(binding.project_digest.clone()),
            selected_work_item_ids: vec![other_id],
            explicit_exclusions: Vec::new(),
            candidate_plan_steps: vec![empty_patch_step("other")],
            acceptance_checks: Vec::new(),
            estimated_external_waits: Vec::new(),
            external_costs: Vec::new(),
            risks: Vec::new(),
            required_decisions: Vec::new(),
            repair_policy: "deterministic_only".to_string(),
        })
        .unwrap();
    let ChangePreparationResult::Blocked(blockers) = blocked else {
        panic!("second mutation lane must be blocked");
    };
    assert!(blockers
        .iter()
        .any(|blocker| blocker.code == "project_intent.mutation_lane_busy"));

    let mut revised = ready_draft(&selected_event, "Selected meaning changed");
    revised.user_visible_outcome = "A materially different result".to_string();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::ReviseWorkItem {
                command_id: "revise-active-selected".to_string(),
                work_item_id: selected_id,
                draft: revised,
            },
            None,
        )
        .unwrap();
    let snapshot = workflow
        .dispatch(
            ProjectIntentWorkflowCommand::AdvanceRun {
                command_id: "advance-stale-run".to_string(),
                run_id: run.run_id,
            },
            Some(&mut session),
        )
        .unwrap();
    assert_eq!(
        snapshot.active_run.unwrap().state,
        ProjectProductionRunState::Stale
    );
}

#[test]
fn project_intent_workflow_one_approval_drives_multiple_candidates() {
    let (mut session, _) = created_session("IntentMultipleCandidates");
    assert_eq!(
        session
            .execute_command(command_for_test(
                UiCommandPayload::CreateDefaultInputMapping {
                    path: "Input/input.multi.json".to_string(),
                }
            ))
            .status,
        CommandStatus::Committed
    );
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let event = capture(&mut workflow, "capture-multi", "Apply two related changes.");
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-multi".to_string(),
                draft: ready_draft(&event, "Two candidates"),
            },
            None,
        )
        .unwrap();
    let work_item_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items[0]
        .work_item_id
        .clone();
    let first_payload = ProjectCandidatePayload::ProjectPatch(ProjectPatchDocument::new(
        "multi-first-patch",
        "Add first action",
        PatchSource::Test,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: "multi-first-operation".to_string(),
            depends_on: Vec::new(),
            path: "Input/input.multi.json".to_string(),
            action_id: "action.multi_first".to_string(),
            value_type: InputActionValueKind::Button,
        })],
    ));
    let second_payload = ProjectCandidatePayload::ProjectPatch(ProjectPatchDocument::new(
        "multi-second-patch",
        "Add second action",
        PatchSource::Test,
        vec![PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: "multi-second-operation".to_string(),
            depends_on: Vec::new(),
            path: "Input/input.multi.json".to_string(),
            action_id: "action.multi_second".to_string(),
            value_type: InputActionValueKind::Button,
        })],
    ));
    let first = CandidatePlanStep {
        step_id: "first".to_string(),
        depends_on: Vec::new(),
        payload_kind: CandidatePayloadKind::ProjectPatch,
        payload_source_digest: payload_digest(&first_payload),
        source_kind: ProjectCandidateSourceKind::ImportedCodex,
        source_label: "workflow-multi-test".to_string(),
        payload: first_payload,
        validation_profile: CandidateValidationProfile {
            controlled_source_patch: None,
            source_file_path: None,
            expected_source_digest: None,
        },
        expected_changed_domains: vec!["input".to_string()],
        user_visible_outcome: "First action exists".to_string(),
        failure_policy: "stop_and_review".to_string(),
    };
    let mut second = CandidatePlanStep {
        step_id: "second".to_string(),
        depends_on: Vec::new(),
        payload_kind: CandidatePayloadKind::ProjectPatch,
        payload_source_digest: payload_digest(&second_payload),
        source_kind: ProjectCandidateSourceKind::ImportedCodex,
        source_label: "workflow-multi-test".to_string(),
        payload: second_payload,
        validation_profile: CandidateValidationProfile {
            controlled_source_patch: None,
            source_file_path: None,
            expected_source_digest: None,
        },
        expected_changed_domains: vec!["input".to_string()],
        user_visible_outcome: "Second action exists".to_string(),
        failure_policy: "stop_and_review".to_string(),
    };
    second.depends_on = vec!["first".to_string()];
    let result = workflow
        .prepare_change(ChangePreparationRequest {
            command_id: "prepare-multi".to_string(),
            target_kind: ChangeSetTargetKind::ExistingProject,
            target_project_identity: Some(binding.project_id.clone()),
            project_create_spec: None,
            expected_base_project_digest: Some(binding.project_digest.clone()),
            selected_work_item_ids: vec![work_item_id],
            explicit_exclusions: Vec::new(),
            candidate_plan_steps: vec![first, second],
            acceptance_checks: Vec::new(),
            estimated_external_waits: Vec::new(),
            external_costs: Vec::new(),
            risks: Vec::new(),
            required_decisions: Vec::new(),
            repair_policy: "deterministic_only".to_string(),
        })
        .unwrap();
    let ChangePreparationResult::Ready(proposal) = result else {
        panic!("multi-candidate proposal should be ready");
    };
    let run = workflow
        .authorize(
            approval_input(
                &proposal,
                binding.project_id,
                Some(binding.project_digest),
                "approve-multi",
            ),
            Some(&session),
        )
        .unwrap();
    for command_id in ["advance-multi-1", "advance-multi-2"] {
        workflow
            .dispatch(
                ProjectIntentWorkflowCommand::AdvanceRun {
                    command_id: command_id.to_string(),
                    run_id: run.run_id.clone(),
                },
                Some(&mut session),
            )
            .unwrap();
    }
    let run = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .active_run
        .unwrap();
    assert_eq!(
        run.state,
        ProjectProductionRunState::Previewing,
        "run diagnostics: {:?}; steps: {:?}",
        run.diagnostics,
        run.step_snapshots
    );
    assert_eq!(
        run.step_snapshots
            .iter()
            .filter(|step| step.apply_receipt.is_some())
            .count(),
        2
    );
}

#[test]
fn project_intent_workflow_project_journal_is_digest_excluded_and_tamper_fails() {
    let (session, root) = created_session("IntentJournalDigest");
    let before = ProjectCandidateEntry::inspect_project_binding(&session)
        .unwrap()
        .project_digest;
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    capture(
        &mut workflow,
        "capture-digest-excluded",
        "Local editor intent only.",
    );
    let after = ProjectCandidateEntry::inspect_project_binding(&session)
        .unwrap()
        .project_digest;
    assert_eq!(before, after);

    let journal_path = root.join("Library/ProjectIntent/journal.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    value["entries"][1]["record"]["record"]["sanitizedSummary"] =
        serde_json::Value::String("tampered summary".to_string());
    fs::write(&journal_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let error = ProjectIntentWorkflow::open_project(&session).unwrap_err();
    assert!(matches!(
        error.code.as_str(),
        "project_intent.entry_digest_mismatch" | "project_intent.bound_digest_mismatch"
    ));
}

#[test]
fn project_intent_workflow_merge_split_and_reopen_persist_lineage() {
    let draft_root = fixtures::unique_editor_project_temp_dir();
    let draft_path = draft_root.join("intent.json");
    let mut workflow = ProjectIntentWorkflow::open_pre_project_draft(&draft_path).unwrap();
    let events = [
        capture(&mut workflow, "capture-lineage-a", "A"),
        capture(&mut workflow, "capture-lineage-b", "B"),
        capture(&mut workflow, "capture-lineage-c", "C"),
    ];
    for (index, event) in events.iter().enumerate() {
        workflow
            .dispatch(
                ProjectIntentWorkflowCommand::CreateWorkItem {
                    command_id: format!("work-lineage-{index}"),
                    draft: ready_draft(event, &format!("Lineage {index}")),
                },
                None,
            )
            .unwrap();
    }
    let initial = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items;
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::MergeWorkItems {
                command_id: "merge-lineage".to_string(),
                source_work_item_ids: vec![
                    initial[0].work_item_id.clone(),
                    initial[1].work_item_id.clone(),
                ],
                merged: ready_draft(&events[0], "Merged lineage"),
            },
            None,
        )
        .unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::SplitWorkItem {
                command_id: "split-lineage".to_string(),
                source_work_item_id: initial[2].work_item_id.clone(),
                parts: vec![
                    ready_draft(&events[2], "Split one"),
                    ready_draft(&events[2], "Split two"),
                ],
            },
            None,
        )
        .unwrap();
    let reopened = ProjectIntentWorkflow::open_pre_project_draft(&draft_path).unwrap();
    let items = reopened
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items;
    assert_eq!(
        items
            .iter()
            .filter(|item| item.status == WorkItemStatus::Merged)
            .count(),
        2
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| item.status == WorkItemStatus::Split)
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| !item.prior_work_item_ids.is_empty())
            .count(),
        3
    );
}

#[test]
fn project_intent_workflow_launcher_create_with_ai_captures_without_project_creation() {
    let local_root = fixtures::unique_editor_project_temp_dir();
    let draft_path = local_root.join("launcher/intent.json");
    let unrelated_target = fixtures::unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let started = session.execute_command(command_for_test(
        UiCommandPayload::StartCreateProjectWithAi {
            draft_path: Some(draft_path.display().to_string()),
        },
    ));
    assert_eq!(started.status, CommandStatus::Committed, "{started:?}");
    assert!(draft_path.exists());
    assert!(
        session
            .build_ui_model()
            .project_intent
            .intent
            .pre_project_draft_active
    );
    assert!(!unrelated_target.exists());

    let submitted = session.execute_command(command_for_test(
        UiCommandPayload::GenerateProjectPatchFromPrompt {
            prompt: "I may want a small puzzle game, but the movement is undecided.".to_string(),
        },
    ));
    assert_eq!(submitted.status, CommandStatus::Committed, "{submitted:?}");
    assert!(session.active_project_session().is_none());
    assert!(!unrelated_target.exists());
    let snapshot = session.project_intent_snapshot().unwrap();
    assert_eq!(snapshot.intent_events.len(), 1);
    assert_eq!(snapshot.work_items.len(), 1);
    assert_eq!(snapshot.work_items[0].status, WorkItemStatus::Triaging);
    assert_eq!(
        session
            .build_ui_model()
            .project_intent
            .intent
            .latest_summary
            .as_deref(),
        Some("I may want a small puzzle game, but the movement is undecided.")
    );
}

#[test]
fn project_intent_workflow_approval_identity_cannot_be_replayed() {
    let (session, _) = created_session("IntentApprovalReplay");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let event = capture(&mut workflow, "capture-replay", "Apply one change.");
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "work-replay".to_string(),
                draft: ready_draft(&event, "Replay guarded"),
            },
            None,
        )
        .unwrap();
    let work_item_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .work_items[0]
        .work_item_id
        .clone();
    let first = prepare_existing(
        &mut workflow,
        &binding,
        vec![work_item_id.clone()],
        "prepare-replay-first",
    );
    let mut first_approval = approval_input(
        &first,
        binding.project_id.clone(),
        Some(binding.project_digest.clone()),
        "approve-replay-first",
    );
    first_approval.approval_id = "approval-single-use".to_string();
    let run = workflow.authorize(first_approval, Some(&session)).unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CancelRun {
                command_id: "cancel-replay-first".to_string(),
                run_id: run.run_id,
            },
            None,
        )
        .unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::ReviseWorkItem {
                command_id: "revise-replay".to_string(),
                work_item_id: work_item_id.clone(),
                draft: ready_draft(&event, "Replay guarded revision"),
            },
            None,
        )
        .unwrap();
    let second = prepare_existing(
        &mut workflow,
        &binding,
        vec![work_item_id],
        "prepare-replay-second",
    );
    let mut replay = approval_input(
        &second,
        binding.project_id,
        Some(binding.project_digest),
        "approve-replay-second",
    );
    replay.approval_id = "approval-single-use".to_string();
    let error = workflow.authorize(replay, Some(&session)).unwrap_err();
    assert_eq!(error.code, "project_intent.approval_replay_rejected");
}

#[test]
fn project_intent_workflow_imported_codex_adapters_are_strict_scoped_and_non_authorizing() {
    let (session, _) = created_session("IntentCodexAdapters");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let event_id = capture(
        &mut workflow,
        "capture-codex-adapter",
        "Make movement feel faster, but do not change combat.",
    );

    let context = workflow.export_sanitized_context().unwrap();
    let context_json = serde_json::to_string(&context).unwrap();
    assert!(!context_json.contains("sourceIdentity"));
    assert!(!context_json.contains("attachmentRefs"));
    assert!(!context_json.contains("contentRef"));

    let proposal = IntentNormalizationProposal {
        schema_version: INTENT_NORMALIZATION_PROPOSAL_SCHEMA_VERSION.to_string(),
        source_label: "codex-task-252".to_string(),
        base_journal_digest: context.journal_digest.clone(),
        work_items: vec![ready_draft(&event_id, "Faster movement")],
    };
    let mut unknown_field = serde_json::to_value(&proposal).unwrap();
    unknown_field
        .as_object_mut()
        .unwrap()
        .insert("claimsUserApproval".to_string(), serde_json::json!(true));
    let error = workflow
        .import_codex_normalization("normalize-unknown", &unknown_field.to_string())
        .unwrap_err();
    assert_eq!(error.code, "project_intent.normalization_parse_failed");

    let snapshot = workflow
        .import_codex_normalization(
            "normalize-codex",
            &serde_json::to_string(&proposal).unwrap(),
        )
        .unwrap();
    assert_eq!(snapshot.work_items.len(), 1);
    assert_eq!(
        snapshot.work_items[0].normalization_source_label.as_deref(),
        Some("codex-task-252")
    );
    assert!(snapshot.active_approval.is_none());
    assert!(snapshot.active_run.is_none());

    let stale_error = workflow
        .import_codex_normalization(
            "normalize-stale",
            &serde_json::to_string(&proposal).unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        stale_error.code,
        "project_intent.normalization_context_stale"
    );

    let current = workflow.observe(ProjectIntentQuery::All).unwrap();
    let mut step = empty_patch_step("codex-plan");
    step.source_label = "codex-task-252".to_string();
    let plan = ImportedChangePlanSource {
        schema_version: IMPORTED_CHANGE_PLAN_SOURCE_SCHEMA_VERSION.to_string(),
        source_label: "codex-task-252".to_string(),
        base_journal_digest: current.journal_digest,
        target_kind: ChangeSetTargetKind::ExistingProject,
        target_project_identity: Some(binding.project_id.clone()),
        project_create_spec: None,
        expected_base_project_digest: Some(binding.project_digest),
        selected_work_item_ids: vec![snapshot.work_items[0].work_item_id.clone()],
        explicit_exclusions: vec!["combat".to_string()],
        candidate_plan_steps: vec![step],
        acceptance_checks: vec!["movement preview".to_string()],
        estimated_external_waits: Vec::new(),
        external_costs: Vec::new(),
        risks: Vec::new(),
        required_decisions: Vec::new(),
        repair_policy: "deterministic_only".to_string(),
    };
    let prepared = workflow
        .import_codex_change_plan("import-codex-plan", &serde_json::to_string(&plan).unwrap())
        .unwrap();
    let ChangePreparationResult::Ready(prepared) = prepared else {
        panic!("imported plan should prepare a ChangeSet");
    };
    assert_eq!(prepared.explicit_exclusions, vec!["combat"]);
    let after = workflow.observe(ProjectIntentQuery::All).unwrap();
    assert!(after.active_approval.is_none());
    assert!(after.active_run.is_none());
}

#[test]
fn project_intent_workflow_golden_gate_supports_fragmented_iteration() {
    let (mut session, _) = created_session("IntentFragmentedGolden");
    let binding = ProjectCandidateEntry::inspect_project_binding(&session).unwrap();
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let fragments = [
        "Maybe add online co-op someday.",
        "There is a movement bug, but I cannot reproduce it yet.",
        "For now only add a dash input.",
        "Do not change combat.",
        "Actually keyboard first, controller can wait.",
        "The dash should be visible in the input asset.",
        "Ignore the menu idea for this change.",
        "The bug happened after pausing, perhaps.",
        "No, pausing may be unrelated.",
        "Also consider achievements later.",
    ];
    let event_ids = fragments
        .iter()
        .enumerate()
        .map(|(index, fragment)| {
            capture(&mut workflow, &format!("golden-capture-{index}"), fragment)
        })
        .collect::<Vec<_>>();

    let mut online = ready_draft(&event_ids[0], "Online co-op later");
    online.kind = WorkItemKind::Idea;
    online.status = WorkItemStatus::Parked;
    let mut bug = ready_draft(&event_ids[1], "Intermittent movement bug");
    bug.kind = WorkItemKind::Bug;
    bug.status = WorkItemStatus::NeedsEvidence;
    let dash = ready_draft(&event_ids[2], "Dash input now");
    for (command_id, draft) in [
        ("golden-work-online", online),
        ("golden-work-bug", bug),
        ("golden-work-dash", dash),
    ] {
        workflow
            .dispatch(
                ProjectIntentWorkflowCommand::CreateWorkItem {
                    command_id: command_id.to_string(),
                    draft,
                },
                None,
            )
            .unwrap();
    }
    let initial = workflow.observe(ProjectIntentQuery::All).unwrap();
    assert_eq!(initial.intent_events.len(), 10);
    let bug_id = initial
        .work_items
        .iter()
        .find(|item| item.kind == WorkItemKind::Bug)
        .unwrap()
        .work_item_id
        .clone();
    let dash_id = initial
        .work_items
        .iter()
        .find(|item| item.title == "Dash input now")
        .unwrap()
        .work_item_id
        .clone();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::StartDiagnosis {
                command_id: "golden-diagnosis-start".to_string(),
                work_item_id: bug_id,
                base_project_digest: Some(binding.project_digest.clone()),
            },
            None,
        )
        .unwrap();
    let diagnosis_id = workflow
        .observe(ProjectIntentQuery::All)
        .unwrap()
        .active_diagnoses[0]
        .diagnosis_id
        .clone();

    drop(workflow);
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::UpdateDiagnosis {
                command_id: "golden-diagnosis-evidence".to_string(),
                update: DiagnosisUpdate {
                    diagnosis_id,
                    state: DiagnosisState::Investigating,
                    reproduction_attempts: vec![
                        "Reloaded project and retried pause flow".to_string()
                    ],
                    observations: vec!["Issue still not reproduced".to_string()],
                    hypotheses: Vec::new(),
                    confirmed_cause: None,
                    evidence_refs: vec!["evidence://golden/reopen-attempt".to_string()],
                    proposed_fix_scope: Vec::new(),
                    requested_capabilities: vec![
                        DiagnosticCapability::ReadProject,
                        DiagnosticCapability::RunPreview,
                    ],
                },
            },
            None,
        )
        .unwrap();

    let payload = ProjectCandidatePayload::ProjectPatch(ProjectPatchDocument::new(
        "golden-dash-input",
        "Create dash input mapping",
        PatchSource::Test,
        vec![PatchOperation::Input(
            InputPatchOperation::CreateDefaultInputMapping {
                operation_id: "golden-create-input".to_string(),
                depends_on: Vec::new(),
                path: "Input/input.golden.json".to_string(),
            },
        )],
    ));
    let step = CandidatePlanStep {
        step_id: "golden-dash-step".to_string(),
        depends_on: Vec::new(),
        payload_kind: CandidatePayloadKind::ProjectPatch,
        payload_source_digest: payload_digest(&payload),
        source_kind: ProjectCandidateSourceKind::ImportedCodex,
        source_label: "golden-codex-plan".to_string(),
        payload,
        validation_profile: CandidateValidationProfile {
            controlled_source_patch: None,
            source_file_path: None,
            expected_source_digest: None,
        },
        expected_changed_domains: vec!["input".to_string()],
        user_visible_outcome: "Dash input mapping exists".to_string(),
        failure_policy: "stop_and_review".to_string(),
    };
    let prepared = workflow
        .prepare_change(ChangePreparationRequest {
            command_id: "golden-prepare".to_string(),
            target_kind: ChangeSetTargetKind::ExistingProject,
            target_project_identity: Some(binding.project_id.clone()),
            project_create_spec: None,
            expected_base_project_digest: Some(binding.project_digest.clone()),
            selected_work_item_ids: vec![dash_id.clone()],
            explicit_exclusions: vec![
                "online co-op".to_string(),
                "combat".to_string(),
                "achievements".to_string(),
            ],
            candidate_plan_steps: vec![step],
            acceptance_checks: vec!["preview".to_string()],
            estimated_external_waits: Vec::new(),
            external_costs: Vec::new(),
            risks: Vec::new(),
            required_decisions: Vec::new(),
            repair_policy: "deterministic_only".to_string(),
        })
        .unwrap();
    let ChangePreparationResult::Ready(proposal) = prepared else {
        panic!("parked and needs-evidence work must not block dash");
    };
    let run = workflow
        .authorize(
            approval_input(
                &proposal,
                binding.project_id,
                Some(binding.project_digest),
                "golden-approve",
            ),
            Some(&session),
        )
        .unwrap();

    let mut later = ready_draft(&event_ids[9], "Achievements later");
    later.kind = WorkItemKind::Idea;
    later.status = WorkItemStatus::Parked;
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: "golden-work-achievements".to_string(),
                draft: later,
            },
            None,
        )
        .unwrap();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::AdvanceRun {
                command_id: "golden-advance-apply".to_string(),
                run_id: run.run_id.clone(),
            },
            Some(&mut session),
        )
        .unwrap();
    let applied = workflow.observe(ProjectIntentQuery::All).unwrap();
    assert_eq!(
        applied.active_run.as_ref().unwrap().state,
        ProjectProductionRunState::Previewing
    );
    assert!(applied.active_run.as_ref().unwrap().step_snapshots[0]
        .apply_receipt
        .is_some());

    drop(workflow);
    let mut workflow = ProjectIntentWorkflow::open_project(&session).unwrap();
    let reopened = workflow.observe(ProjectIntentQuery::All).unwrap();
    assert_eq!(reopened.intent_events.len(), 10);
    assert!(reopened.active_diagnoses[0]
        .evidence_refs
        .contains(&"evidence://golden/reopen-attempt".to_string()));
    assert!(reopened.active_run.as_ref().unwrap().step_snapshots[0]
        .apply_receipt
        .is_some());

    let mut changed = ready_draft(&event_ids[2], "Dash input changed after approval");
    changed.user_visible_outcome = "Dash input now requires a different behavior".to_string();
    workflow
        .dispatch(
            ProjectIntentWorkflowCommand::ReviseWorkItem {
                command_id: "golden-revise-selected".to_string(),
                work_item_id: dash_id,
                draft: changed,
            },
            None,
        )
        .unwrap();
    let stale = workflow
        .dispatch(
            ProjectIntentWorkflowCommand::AdvanceRun {
                command_id: "golden-advance-stale".to_string(),
                run_id: run.run_id,
            },
            Some(&mut session),
        )
        .unwrap();
    assert_eq!(
        stale.active_run.unwrap().state,
        ProjectProductionRunState::Stale
    );
}
