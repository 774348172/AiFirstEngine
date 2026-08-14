use crate::{CommandResult, CommandStatus, CommandTransaction, EditorSession, UndoPolicy};

impl EditorSession {
    pub(crate) fn park_project_work_item(
        &mut self,
        transaction: &mut CommandTransaction,
        work_item_id: String,
    ) -> CommandResult {
        self.dispatch_project_intent_ui_command(
            transaction,
            crate::ProjectIntentWorkflowCommand::ParkWorkItem {
                command_id: format!("ui-park-{}", transaction.transaction_id),
                work_item_id,
            },
            "project_intent.work_item_parked",
            "WorkItem parked without blocking unrelated ready work.",
        )
    }

    pub(crate) fn resume_project_work_item(
        &mut self,
        transaction: &mut CommandTransaction,
        work_item_id: String,
    ) -> CommandResult {
        self.dispatch_project_intent_ui_command(
            transaction,
            crate::ProjectIntentWorkflowCommand::ResumeWorkItem {
                command_id: format!("ui-resume-{}", transaction.transaction_id),
                work_item_id,
            },
            "project_intent.work_item_resumed",
            "WorkItem resumed with its prior lineage intact.",
        )
    }

    pub(crate) fn reopen_project_work_item(
        &mut self,
        transaction: &mut CommandTransaction,
        work_item_id: String,
    ) -> CommandResult {
        self.dispatch_project_intent_ui_command(
            transaction,
            crate::ProjectIntentWorkflowCommand::ReopenWorkItem {
                command_id: format!("ui-reopen-{}", transaction.transaction_id),
                work_item_id,
                evidence_refs: Vec::new(),
            },
            "project_intent.work_item_reopened",
            "WorkItem reopened and linked to its prior production history.",
        )
    }

    pub(crate) fn approve_project_change(
        &mut self,
        transaction: &mut CommandTransaction,
        proposal_digest: String,
    ) -> CommandResult {
        transaction
            .read_set
            .push("project_intent_workflow.proposal".to_string());
        transaction
            .write_set
            .push("project_intent_workflow.run".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let snapshot = match self.project_intent_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => return self.finish_project_intent_error(transaction, error),
        };
        let Some(proposal) = snapshot.active_proposal else {
            return self.finish_project_intent_error(
                transaction,
                crate::ProjectIntentWorkflowError::new(
                    "project_intent.proposal_missing",
                    "There is no active ChangeSetProposal to approve.",
                    "Prepare a change from one or more ready WorkItems.",
                ),
            );
        };
        if proposal.proposal_digest != proposal_digest {
            return self.finish_project_intent_error(
                transaction,
                crate::ProjectIntentWorkflowError::new(
                    "project_intent.approval_proposal_mismatch",
                    "The visible ChangeSet changed before approval was dispatched.",
                    "Review and approve the current proposal.",
                ),
            );
        }
        let target_identity = match proposal.target_kind {
            crate::ChangeSetTargetKind::NewProject => proposal
                .project_create_spec
                .as_ref()
                .map(|spec| spec.project_root.clone()),
            crate::ChangeSetTargetKind::ExistingProject => proposal.target_project_identity.clone(),
        };
        let Some(target_identity) = target_identity else {
            return self.finish_project_intent_error(
                transaction,
                crate::ProjectIntentWorkflowError::new(
                    "project_intent.approval_target_missing",
                    "The ChangeSet has no approval target identity.",
                    "Reprepare the ChangeSet against a concrete project target.",
                ),
            );
        };
        let input = crate::ChangeSetApprovalInput {
            command_id: format!("ui-approve-{}", transaction.transaction_id),
            approval_id: format!("editor-approval-{}", transaction.transaction_id),
            approved_by: "editor-user".to_string(),
            proposal_digest,
            target_identity,
            expected_base_project_digest: proposal.expected_base_project_digest.clone(),
            approved_risk_classes: proposal.risks.clone(),
            approved_external_costs: proposal.external_costs.clone(),
            approved_repair_policy: proposal.repair_policy.clone(),
            approved_at: None,
        };
        match self.authorize_project_change(input) {
            Ok(run) => {
                self.push_info(
                    transaction,
                    "project_intent.change_approved",
                    format!(
                        "Approved ChangeSet and created production run {}.",
                        run.run_id
                    ),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => self.finish_project_intent_error(transaction, error),
        }
    }

    pub(crate) fn advance_project_production(
        &mut self,
        transaction: &mut CommandTransaction,
        run_id: String,
    ) -> CommandResult {
        self.dispatch_project_intent_ui_command(
            transaction,
            crate::ProjectIntentWorkflowCommand::AdvanceRun {
                command_id: format!("ui-advance-{}", transaction.transaction_id),
                run_id,
            },
            "project_intent.production_advanced",
            "Project production advanced through the approved ChangeSet.",
        )
    }

    pub(crate) fn cancel_project_production(
        &mut self,
        transaction: &mut CommandTransaction,
        run_id: String,
    ) -> CommandResult {
        self.dispatch_project_intent_ui_command(
            transaction,
            crate::ProjectIntentWorkflowCommand::CancelRun {
                command_id: format!("ui-cancel-run-{}", transaction.transaction_id),
                run_id,
            },
            "project_intent.production_cancelled",
            "Project production was cancelled at a workflow boundary.",
        )
    }

    pub(crate) fn recover_project_production(
        &mut self,
        transaction: &mut CommandTransaction,
        run_id: String,
    ) -> CommandResult {
        self.dispatch_project_intent_ui_command(
            transaction,
            crate::ProjectIntentWorkflowCommand::RecoverRun {
                command_id: format!("ui-recover-run-{}", transaction.transaction_id),
                run_id,
            },
            "project_intent.production_recovered",
            "Project production recovery was requested.",
        )
    }

    fn dispatch_project_intent_ui_command(
        &mut self,
        transaction: &mut CommandTransaction,
        command: crate::ProjectIntentWorkflowCommand,
        success_code: &str,
        success_message: &str,
    ) -> CommandResult {
        transaction
            .write_set
            .push("project_intent_workflow.journal".to_string());
        transaction.undo_policy = UndoPolicy::None;
        match self.dispatch_project_intent(command) {
            Ok(_) => {
                self.push_info(transaction, success_code, success_message);
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => self.finish_project_intent_error(transaction, error),
        }
    }

    fn finish_project_intent_error(
        &mut self,
        transaction: &mut CommandTransaction,
        error: crate::ProjectIntentWorkflowError,
    ) -> CommandResult {
        self.push_error(
            transaction,
            &error.code,
            error.message,
            Some(&error.next_action),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Rejected)
    }
}
