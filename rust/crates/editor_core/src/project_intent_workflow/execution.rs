use super::{
    candidate_entry_error, rebuild_snapshot, timestamp_string, workflow_error, ChangeSetTargetKind,
    ProductionStepState, ProjectIntentWorkflow, ProjectIntentWorkflowError,
    ProjectProductionRunState, WorkItem, WorkItemStatus,
};
use crate::{
    CommandStatus, EditorSession, ProjectCandidateApproval, ProjectCandidateEntry,
    ProjectCandidateEnvelope, ProjectCandidatePrepareRequest, ProjectCandidateValidationContext,
    ProjectCandidateValidationStatus, ProjectPatchLlmContextSnapshot,
    PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION, PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION,
};
use editor_ui_model::{UiCommand, UiCommandPayload, UiCommandSource};

pub(super) fn advance_run(
    workflow: &mut ProjectIntentWorkflow,
    command_id: &str,
    run_id: &str,
    session: &mut EditorSession,
) -> Result<(), ProjectIntentWorkflowError> {
    let snapshot = rebuild_snapshot(workflow.journal())?;
    let proposal = snapshot.active_proposal.ok_or_else(|| {
        workflow_error(
            "project_intent.proposal_missing",
            "Active ProjectProductionRun has no ChangeSetProposal.",
            "Restore the exact journal or reprepare the ChangeSet.",
        )
    })?;
    let approval = snapshot.active_approval.ok_or_else(|| {
        workflow_error(
            "project_intent.approval_missing",
            "Active ProjectProductionRun has no ChangeSetApproval.",
            "Restore the exact approval record; do not infer authorization.",
        )
    })?;
    let mut run = snapshot.active_run.ok_or_else(|| {
        workflow_error(
            "project_intent.run_missing",
            "There is no active ProjectProductionRun.",
            "Authorize the current ChangeSetProposal first.",
        )
    })?;
    if run.run_id != run_id {
        return Err(workflow_error(
            "project_intent.run_identity_mismatch",
            "AdvanceRun does not target the active ProjectProductionRun.",
            "Use the run id returned by authorize().",
        ));
    }
    if matches!(
        run.state,
        ProjectProductionRunState::Completed | ProjectProductionRunState::Cancelled
    ) {
        return Err(workflow_error(
            "project_intent.run_terminal",
            "Terminal ProjectProductionRun cannot advance.",
            "Reopen affected WorkItems and prepare a new ChangeSet.",
        ));
    }
    if selected_semantics_changed(&run.linked_work_item_revisions, &snapshot.work_items) {
        run.state = ProjectProductionRunState::Stale;
        run.diagnostics
            .push("project_intent.run_selected_work_item_stale".to_string());
        workflow.append_run(command_id, run, Vec::new())?;
        return Ok(());
    }

    if run.state == ProjectProductionRunState::Previewing {
        return complete_preview(workflow, command_id, run, session, &snapshot.work_items);
    }

    if run.state == ProjectProductionRunState::Approved {
        match proposal.target_kind {
            ChangeSetTargetKind::NewProject => {
                let spec = proposal.project_create_spec.as_ref().ok_or_else(|| {
                    workflow_error(
                        "project_intent.project_create_spec_missing",
                        "Approved from-blank run has no ProjectCreateSpec.",
                        "Reprepare and approve a complete ChangeSetProposal.",
                    )
                })?;
                if session.active_project_session().is_some() {
                    fail_run(
                        workflow,
                        command_id,
                        run,
                        "project_intent.from_blank_active_project_conflict",
                        "Close the active project before executing this from-blank run.",
                    )?;
                    return Ok(());
                }
                run.state = ProjectProductionRunState::CreatingProject;
                let result = session.execute_command(UiCommand {
                    command_id: format!("intent-create-project-{}", run.run_id),
                    source: UiCommandSource::AiAssistant,
                    request_id: format!("intent-create-request-{}", run.run_id),
                    payload: UiCommandPayload::CreateProject {
                        path: spec.project_root.clone(),
                        name: spec.project_name.clone(),
                    },
                });
                if result.status != CommandStatus::Committed {
                    fail_run(
                        workflow,
                        command_id,
                        run,
                        "project_intent.create_project_failed",
                        "Inspect ProjectLauncher diagnostics and retry without discarding the pre-project draft.",
                    )?;
                    return Ok(());
                }
                workflow.attach_created_project(
                    session,
                    &format!("intent-create-project-{}", run.run_id),
                )?;
                let binding = ProjectCandidateEntry::inspect_project_binding(session)
                    .map_err(candidate_entry_error)?;
                run.base_project_digest = Some(binding.project_digest.clone());
                run.current_project_digest = Some(binding.project_digest);
                run.target_project_identity = binding.project_id;
                run.state = ProjectProductionRunState::Executing;
            }
            ChangeSetTargetKind::ExistingProject => {
                run.state = ProjectProductionRunState::Executing;
            }
        }
    }

    let binding =
        ProjectCandidateEntry::inspect_project_binding(session).map_err(candidate_entry_error)?;
    if run.current_project_digest.as_deref() != Some(binding.project_digest.as_str()) {
        run.state = ProjectProductionRunState::Stale;
        run.diagnostics
            .push("project_intent.run_base_project_drifted".to_string());
        workflow.append_run(command_id, run, Vec::new())?;
        return Ok(());
    }

    let Some(step_index) = run
        .step_snapshots
        .iter()
        .position(|step| step.state == ProductionStepState::Pending)
    else {
        run.state = ProjectProductionRunState::Previewing;
        run.active_step_id = None;
        run.decision_requests = vec!["preview_verification_required".to_string()];
        workflow.append_run(command_id, run, Vec::new())?;
        return Ok(());
    };
    let step_id = run.step_snapshots[step_index].step_id.clone();
    let plan_step = proposal
        .candidate_plan_steps
        .iter()
        .find(|step| step.step_id == step_id)
        .ok_or_else(|| {
            workflow_error(
                "project_intent.run_step_missing",
                "Run step no longer exists in its approved proposal.",
                "Restore the exact proposal or reprepare the ChangeSet.",
            )
        })?;
    let dependency_ready = plan_step.depends_on.iter().all(|dependency| {
        run.step_snapshots
            .iter()
            .any(|step| step.step_id == *dependency && step.state == ProductionStepState::Applied)
    });
    if !dependency_ready {
        fail_run(
            workflow,
            command_id,
            run,
            "project_intent.run_step_dependency_unmet",
            "Repair or reprepare the approved step ordering.",
        )?;
        return Ok(());
    }

    run.active_step_id = Some(step_id.clone());
    run.step_snapshots[step_index].state = ProductionStepState::Validating;
    let project_patch_context_hash = match &plan_step.payload {
        crate::ProjectCandidatePayload::ProjectPatch(_) => {
            Some(ProjectPatchLlmContextSnapshot::capture(session).context_hash)
        }
        _ => None,
    };
    let envelope = ProjectCandidateEnvelope {
        schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
        candidate_id: format!("{}-{}", run.run_id, step_id),
        source_kind: plan_step.source_kind,
        source_label: plan_step.source_label.clone(),
        target_project_id: binding.project_id,
        expected_base_project_digest: binding.project_digest,
        project_patch_context_hash,
        payload: plan_step.payload.clone(),
    };
    let result = execute_candidate(session, envelope, plan_step, &approval.approved_by);
    match result {
        Ok(receipt) => {
            let step = &mut run.step_snapshots[step_index];
            step.state = ProductionStepState::Applied;
            step.candidate_id = Some(receipt.candidate_id.clone());
            step.candidate_digest = Some(receipt.candidate_digest.clone());
            step.validation_digest = Some(receipt.validation_digest.clone());
            step.apply_receipt = Some(receipt.clone());
            run.current_project_digest = Some(receipt.applied_project_digest);
            run.active_step_id = None;
            if run
                .step_snapshots
                .iter()
                .all(|step| step.state == ProductionStepState::Applied)
            {
                run.state = ProjectProductionRunState::Previewing;
                run.decision_requests = vec!["preview_verification_required".to_string()];
            } else {
                run.state = ProjectProductionRunState::Executing;
            }
            workflow.append_run(command_id, run, Vec::new())?;
        }
        Err(error) => {
            run.step_snapshots[step_index].state = ProductionStepState::Failed;
            run.step_snapshots[step_index]
                .diagnostics
                .push(error.code.clone());
            run.state = ProjectProductionRunState::Failed;
            run.diagnostics.push(error.code);
            run.decision_requests.push(error.next_action);
            workflow.append_run(command_id, run, Vec::new())?;
        }
    }
    Ok(())
}

fn execute_candidate(
    session: &mut EditorSession,
    envelope: ProjectCandidateEnvelope,
    plan_step: &super::CandidatePlanStep,
    approved_by: &str,
) -> Result<crate::ProjectCandidateApplyReceipt, ProjectIntentWorkflowError> {
    let prepare_request = ProjectCandidatePrepareRequest { envelope };
    let candidate =
        if let Some(source_file_path) = &plan_step.validation_profile.source_file_path {
            ProjectCandidateEntry::prepare_with_source_file(
                session,
                prepare_request,
                source_file_path,
            )
        } else {
            ProjectCandidateEntry::prepare(session, prepare_request)
        }
        .map_err(candidate_entry_error)?;
    if plan_step
        .validation_profile
        .expected_source_digest
        .as_ref()
        .is_some_and(|expected| expected != &candidate.source_digest)
    {
        return Err(workflow_error(
            "project_candidate_entry.source_drifted",
            "Candidate source file changed after ChangeSet preparation.",
            "Prepare and approve a new ChangeSet from the current source file.",
        ));
    }
    let validation = ProjectCandidateEntry::validate(
        session,
        &candidate,
        &ProjectCandidateValidationContext {
            controlled_source_patch: plan_step.validation_profile.controlled_source_patch.clone(),
            cancellation: None,
        },
    )
    .map_err(candidate_entry_error)?;
    if validation.status != ProjectCandidateValidationStatus::Passed {
        return Err(workflow_error(
            "project_intent.candidate_validation_failed",
            format!(
                "Candidate {} failed validation.",
                candidate.envelope.candidate_id
            ),
            "Inspect validation diagnostics and reprepare within the approved repair policy.",
        ));
    }
    let approval = ProjectCandidateApproval {
        schema_version: PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION.to_string(),
        candidate_id: candidate.envelope.candidate_id.clone(),
        candidate_digest: candidate.candidate_digest.clone(),
        validation_digest: validation.validation_digest.clone(),
        approved_by: approved_by.to_string(),
        allow_replace: false,
    };
    ProjectCandidateEntry::apply(session, candidate, validation, approval)
        .map_err(candidate_entry_error)
}

fn complete_preview(
    workflow: &mut ProjectIntentWorkflow,
    command_id: &str,
    mut run: super::ProjectProductionRun,
    session: &mut EditorSession,
    work_items: &[WorkItem],
) -> Result<(), ProjectIntentWorkflowError> {
    let evidence = match crate::ai_capability_tool_kernel::execute_project_owned_preview(
        session,
        &format!("intent-compat-{}", run.run_id),
    ) {
        Ok(evidence) => evidence,
        Err(error) => {
            run.diagnostics.push(error.code);
            run.decision_requests.push(error.next_action);
            run.state = ProjectProductionRunState::Failed;
            workflow.append_run(command_id, run, Vec::new())?;
            return Ok(());
        }
    };
    run.preview_evidence = Some(evidence.runtime_bind_receipt_digest);
    run.state = ProjectProductionRunState::Completed;
    run.decision_requests.clear();
    run.completed_at = Some(timestamp_string());
    let linked = run
        .linked_work_item_revisions
        .iter()
        .map(|binding| binding.work_item_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut completed_items = work_items
        .iter()
        .filter(|item| linked.contains(item.work_item_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for item in &mut completed_items {
        item.status = WorkItemStatus::Done;
    }
    workflow.append_run(command_id, run, completed_items)
}

fn fail_run(
    workflow: &mut ProjectIntentWorkflow,
    command_id: &str,
    mut run: super::ProjectProductionRun,
    diagnostic: &str,
    next_action: &str,
) -> Result<(), ProjectIntentWorkflowError> {
    run.state = ProjectProductionRunState::Failed;
    run.diagnostics.push(diagnostic.to_string());
    run.decision_requests.push(next_action.to_string());
    workflow.append_run(command_id, run, Vec::new())
}

fn selected_semantics_changed(
    bindings: &[super::WorkItemRevisionBinding],
    work_items: &[WorkItem],
) -> bool {
    bindings.iter().any(|binding| {
        work_items
            .iter()
            .find(|item| item.work_item_id == binding.work_item_id)
            .is_none_or(|item| {
                item.revision != binding.revision
                    || item.work_item_digest != binding.work_item_digest
            })
    })
}
