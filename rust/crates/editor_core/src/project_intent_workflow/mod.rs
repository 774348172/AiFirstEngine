mod execution;
mod journal;
mod model;

pub use model::*;

use crate::{EditorSession, ProjectCandidateEntry};
use journal::{
    append_record, digest_record, load_local_draft, load_project_journal, persist,
    rebuild_snapshot, timestamp_string, ProjectIntentStorage,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectIntentWorkflow {
    journal: ProjectIntentJournalDocument,
    storage: ProjectIntentStorage,
}

impl Default for ProjectIntentWorkflow {
    fn default() -> Self {
        Self::in_memory().expect("empty workflow journal must be valid")
    }
}

impl ProjectIntentWorkflow {
    pub fn in_memory() -> Result<Self, ProjectIntentWorkflowError> {
        let journal = journal::finalize_journal(ProjectIntentJournalDocument::default())?;
        Ok(Self {
            journal,
            storage: ProjectIntentStorage::InMemory,
        })
    }

    pub fn open_pre_project_draft(
        draft_path: impl Into<PathBuf>,
    ) -> Result<Self, ProjectIntentWorkflowError> {
        let draft_path = draft_path.into();
        if draft_path.as_os_str().is_empty() {
            return Err(workflow_error(
                "project_intent.draft_path_missing",
                "Pre-project intent capture requires an explicit Launcher-local draft path.",
                "Choose a file under the Launcher local state directory.",
            ));
        }
        let journal = load_local_draft(&draft_path)?;
        let workflow = Self {
            journal,
            storage: ProjectIntentStorage::LocalDraft(draft_path),
        };
        persist(&workflow.storage, &workflow.journal)?;
        Ok(workflow)
    }

    pub fn open_project(session: &EditorSession) -> Result<Self, ProjectIntentWorkflowError> {
        let project = session.active_project_session().ok_or_else(|| {
            workflow_error(
                "project_intent.project_not_open",
                "Opening a project intent journal requires an active project.",
                "Open or create the project first.",
            )
        })?;
        let journal = load_project_journal(project.write_scope())?;
        let mut workflow = Self {
            journal,
            storage: ProjectIntentStorage::Project {
                write_scope: project.write_scope().clone(),
            },
        };
        let binding = match session.prepared_binding_for_active_project() {
            crate::session::PreparedProjectOpenBinding::Valid(binding) => binding.clone(),
            crate::session::PreparedProjectOpenBinding::Missing => {
                ProjectCandidateEntry::inspect_project_binding(session)
                    .map_err(candidate_entry_error)?
            }
            crate::session::PreparedProjectOpenBinding::Invalid => {
                return Err(workflow_error(
                    "project_open.prepared_identity_drift",
                    "Project identity changed after background preparation completed.",
                    "Retry opening the project from its current manifest and source state.",
                ));
            }
        };
        if let Some(stored) = &workflow.journal.project_binding {
            if stored.project_id != binding.project_id
                || stored.project_root != binding.project_root
            {
                return Err(workflow_error(
                    "project_intent.project_binding_mismatch",
                    "Stored intent journal belongs to a different project identity.",
                    "Keep the journal isolated and open the matching project.",
                ));
            }
        } else {
            workflow.attach_project_binding(
                session,
                "open-existing-project",
                "project_intent.attach.open_existing",
            )?;
        }
        Ok(workflow)
    }

    pub fn capture(
        &mut self,
        input: IntentCaptureInput,
    ) -> Result<IntentCaptureReceipt, ProjectIntentWorkflowError> {
        validate_capture_input(&input)?;
        if let Some(entry) = self
            .journal
            .entries
            .iter()
            .find(|entry| entry.command_id == input.command_id)
        {
            let ProjectIntentJournalRecord::IntentCaptured(event) = &entry.record else {
                return Err(command_replay_mismatch(&input.command_id));
            };
            return Ok(IntentCaptureReceipt {
                schema_version: "project-intent-capture-receipt.v1".to_string(),
                command_id: input.command_id,
                event_id: event.event_id.clone(),
                journal_revision: entry.revision,
                journal_digest: self.journal.journal_digest.clone(),
                replayed: true,
            });
        }
        let event_id = next_id("intent", self.journal.revision + 1, 0);
        let mut event = IntentEvent {
            schema_version: INTENT_EVENT_SCHEMA_VERSION.to_string(),
            event_id: event_id.clone(),
            project_identity: input.project_identity,
            occurred_at: input.occurred_at.unwrap_or_else(timestamp_string),
            source_kind: input.source_kind,
            source_identity: input.source_identity,
            content_ref: input.content_ref,
            sanitized_summary: input.sanitized_summary,
            attachment_refs: deduplicate(input.attachment_refs),
            related_event_ids: deduplicate(input.related_event_ids),
            privacy_class: input.privacy_class,
            content_digest: String::new(),
        };
        event.content_digest = digest_with_empty_field(&event, "contentDigest")?;
        let (journal_revision, replayed) = append_record(
            &mut self.journal,
            &self.storage,
            &input.command_id,
            ProjectIntentJournalRecord::IntentCaptured(event),
        )?;
        Ok(IntentCaptureReceipt {
            schema_version: "project-intent-capture-receipt.v1".to_string(),
            command_id: input.command_id,
            event_id,
            journal_revision,
            journal_digest: self.journal.journal_digest.clone(),
            replayed,
        })
    }

    pub fn storage_kind(&self) -> ProjectIntentStorageKind {
        match self.storage {
            ProjectIntentStorage::InMemory => ProjectIntentStorageKind::InMemory,
            ProjectIntentStorage::LocalDraft(_) => ProjectIntentStorageKind::PreProjectDraft,
            ProjectIntentStorage::Project { .. } => ProjectIntentStorageKind::Project,
        }
    }

    pub fn observe(
        &self,
        query: ProjectIntentQuery,
    ) -> Result<ProjectIntentSnapshot, ProjectIntentWorkflowError> {
        let mut snapshot = rebuild_snapshot(&self.journal)?;
        if let ProjectIntentQuery::WorkItems { work_item_ids } = query {
            let ids = work_item_ids.into_iter().collect::<BTreeSet<_>>();
            snapshot
                .work_items
                .retain(|item| ids.contains(&item.work_item_id));
            snapshot
                .work_item_summaries
                .retain(|item| ids.contains(&item.work_item_id));
            snapshot
                .active_diagnoses
                .retain(|diagnosis| ids.contains(&diagnosis.work_item_id));
            snapshot
                .active_diagnosis_summaries
                .retain(|diagnosis| ids.contains(&diagnosis.work_item_id));
        }
        Ok(snapshot)
    }

    pub fn export_sanitized_context(
        &self,
    ) -> Result<SanitizedProjectIntentContext, ProjectIntentWorkflowError> {
        let snapshot = rebuild_snapshot(&self.journal)?;
        Ok(SanitizedProjectIntentContext {
            schema_version: SANITIZED_PROJECT_INTENT_CONTEXT_SCHEMA_VERSION.to_string(),
            project_identity: snapshot
                .project_binding
                .as_ref()
                .map(|binding| binding.project_id.clone()),
            journal_revision: snapshot.journal_revision,
            journal_digest: snapshot.journal_digest,
            intent_events: snapshot
                .intent_events
                .into_iter()
                .map(|event| SanitizedIntentEventContext {
                    event_id: event.event_id,
                    source_kind: event.source_kind,
                    sanitized_summary: event.sanitized_summary,
                    related_event_ids: event.related_event_ids,
                    content_digest: event.content_digest,
                })
                .collect(),
            work_items: snapshot
                .work_items
                .into_iter()
                .map(|item| SanitizedWorkItemContext {
                    work_item_id: item.work_item_id,
                    kind: item.kind,
                    title: item.title,
                    status: item.status,
                    source_event_ids: item.source_event_ids,
                    latest_understanding: item.latest_understanding,
                    open_questions: item.open_questions,
                    revision: item.revision,
                    work_item_digest: item.work_item_digest,
                })
                .collect(),
        })
    }

    pub fn import_codex_normalization(
        &mut self,
        command_id: &str,
        raw_json: &str,
    ) -> Result<ProjectIntentSnapshot, ProjectIntentWorkflowError> {
        if raw_json.len() > 1024 * 1024 {
            return Err(workflow_error(
                "project_intent.normalization_too_large",
                "Imported normalization exceeds the 1 MiB limit.",
                "Export a smaller scoped context and retry.",
            ));
        }
        if self
            .journal
            .entries
            .iter()
            .any(|entry| entry.command_id == command_id)
        {
            return rebuild_snapshot(&self.journal);
        }
        let proposal =
            serde_json::from_str::<IntentNormalizationProposal>(raw_json).map_err(|error| {
                workflow_error(
                    "project_intent.normalization_parse_failed",
                    format!("Imported Codex normalization is not strict valid JSON: {error}"),
                    "Use the exact IntentNormalizationProposal schema and remove unknown fields.",
                )
            })?;
        if proposal.schema_version != INTENT_NORMALIZATION_PROPOSAL_SCHEMA_VERSION {
            return Err(workflow_error(
                "project_intent.normalization_schema_unsupported",
                "Imported Codex normalization uses an unsupported schema version.",
                "Regenerate it from the current sanitized context contract.",
            ));
        }
        if proposal.source_label.trim().is_empty() {
            return Err(workflow_error(
                "project_intent.normalization_source_missing",
                "Imported normalization requires a source label.",
                "Identify the Codex task or exported artifact that produced it.",
            ));
        }
        if proposal.base_journal_digest != self.journal.journal_digest {
            return Err(workflow_error(
                "project_intent.normalization_context_stale",
                "Imported normalization is bound to a different intent journal digest.",
                "Export the current sanitized context and normalize again.",
            ));
        }
        if proposal.work_items.is_empty() {
            return Err(workflow_error(
                "project_intent.normalization_empty",
                "Imported normalization contains no WorkItems.",
                "Return at least one scoped interpretation or leave the events pending.",
            ));
        }
        let source_label = proposal.source_label;
        let mut work_items = Vec::with_capacity(proposal.work_items.len());
        for (offset, draft) in proposal.work_items.into_iter().enumerate() {
            if !matches!(
                draft.status,
                WorkItemStatus::Captured
                    | WorkItemStatus::Triaging
                    | WorkItemStatus::NeedsClarification
                    | WorkItemStatus::NeedsEvidence
                    | WorkItemStatus::Ready
                    | WorkItemStatus::Parked
            ) {
                return Err(workflow_error(
                    "project_intent.normalization_status_forbidden",
                    "Imported normalization cannot claim approval, execution, verification, or completion state.",
                    "Use only captured, triaging, needs-clarification, needs-evidence, ready, or parked.",
                ));
            }
            let mut item = self.new_work_item(draft, Vec::new(), offset)?;
            item.normalization_source_label = Some(source_label.clone());
            work_items.push(item);
        }
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::WorkItemsChanged(work_items),
        )?;
        rebuild_snapshot(&self.journal)
    }

    pub fn import_codex_change_plan(
        &mut self,
        command_id: &str,
        raw_json: &str,
    ) -> Result<ChangePreparationResult, ProjectIntentWorkflowError> {
        if raw_json.len() > 8 * 1024 * 1024 {
            return Err(workflow_error(
                "project_intent.change_plan_too_large",
                "Imported change plan exceeds the 8 MiB limit.",
                "Split the plan into a smaller reviewable ChangeSet.",
            ));
        }
        let source =
            serde_json::from_str::<ImportedChangePlanSource>(raw_json).map_err(|error| {
                workflow_error(
                    "project_intent.change_plan_parse_failed",
                    format!("Imported Codex change plan is not strict valid JSON: {error}"),
                    "Use the exact ImportedChangePlanSource schema and remove unknown fields.",
                )
            })?;
        if source.schema_version != IMPORTED_CHANGE_PLAN_SOURCE_SCHEMA_VERSION {
            return Err(workflow_error(
                "project_intent.change_plan_schema_unsupported",
                "Imported Codex change plan uses an unsupported schema version.",
                "Regenerate it from the current change-plan contract.",
            ));
        }
        if source.source_label.trim().is_empty() {
            return Err(workflow_error(
                "project_intent.change_plan_source_missing",
                "Imported change plan requires a source label.",
                "Identify the Codex task or exported artifact that produced it.",
            ));
        }
        if source.base_journal_digest != self.journal.journal_digest {
            return Err(workflow_error(
                "project_intent.change_plan_context_stale",
                "Imported change plan is bound to a different intent journal digest.",
                "Export the current context and prepare the plan again.",
            ));
        }
        if source.candidate_plan_steps.iter().any(|step| {
            step.source_kind != crate::ProjectCandidateSourceKind::ImportedCodex
                || step.source_label != source.source_label
        }) {
            return Err(workflow_error(
                "project_intent.change_plan_source_mismatch",
                "Imported Codex change steps must retain the imported Codex source kind and label.",
                "Do not relabel built-in, file, or test-fixture candidates as imported Codex output.",
            ));
        }
        self.prepare_change(ChangePreparationRequest {
            command_id: command_id.to_string(),
            target_kind: source.target_kind,
            target_project_identity: source.target_project_identity,
            project_create_spec: source.project_create_spec,
            expected_base_project_digest: source.expected_base_project_digest,
            selected_work_item_ids: source.selected_work_item_ids,
            explicit_exclusions: source.explicit_exclusions,
            candidate_plan_steps: source.candidate_plan_steps,
            acceptance_checks: source.acceptance_checks,
            estimated_external_waits: source.estimated_external_waits,
            external_costs: source.external_costs,
            risks: source.risks,
            required_decisions: source.required_decisions,
            repair_policy: source.repair_policy,
        })
    }

    pub fn prepare_change(
        &mut self,
        request: ChangePreparationRequest,
    ) -> Result<ChangePreparationResult, ProjectIntentWorkflowError> {
        if let Some(entry) = self
            .journal
            .entries
            .iter()
            .find(|entry| entry.command_id == request.command_id)
        {
            let ProjectIntentJournalRecord::ChangeSetPrepared { proposal, .. } = &entry.record
            else {
                return Err(command_replay_mismatch(&request.command_id));
            };
            return Ok(ChangePreparationResult::Ready(proposal.clone()));
        }
        let snapshot = rebuild_snapshot(&self.journal)?;
        let mut blockers = validate_change_request(&request, &snapshot, &self.storage)?;
        let by_id = snapshot
            .work_items
            .iter()
            .map(|item| (item.work_item_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let summaries = snapshot
            .work_item_summaries
            .iter()
            .map(|summary| (summary.work_item_id.as_str(), summary))
            .collect::<BTreeMap<_, _>>();
        let mut selected = Vec::new();
        for work_item_id in &request.selected_work_item_ids {
            let Some(work_item) = by_id.get(work_item_id.as_str()) else {
                blockers.push(change_blocker(
                    "project_intent.work_item_missing",
                    Some(work_item_id.clone()),
                    "Selected WorkItem does not exist.",
                    "Select an existing WorkItem revision.",
                ));
                continue;
            };
            if !summaries
                .get(work_item_id.as_str())
                .is_some_and(|summary| summary.ready)
            {
                blockers.push(change_blocker(
                    "project_intent.work_item_not_ready",
                    Some(work_item_id.clone()),
                    "Selected WorkItem is not locally ready.",
                    "Resolve only this WorkItem's questions or explicit dependencies.",
                ));
                continue;
            }
            selected.push((*work_item).clone());
        }
        if !blockers.is_empty() {
            return Ok(ChangePreparationResult::Blocked(blockers));
        }
        let selected_bindings = selected.iter().map(work_item_binding).collect::<Vec<_>>();
        let mut proposal = ChangeSetProposal {
            schema_version: CHANGE_SET_PROPOSAL_SCHEMA_VERSION.to_string(),
            proposal_id: next_id("change-set", self.journal.revision + 1, 0),
            target_kind: request.target_kind,
            target_project_identity: request.target_project_identity,
            project_create_spec: request.project_create_spec,
            expected_base_project_digest: request.expected_base_project_digest,
            selected_work_item_revisions: selected_bindings,
            user_visible_outcomes: selected
                .iter()
                .map(|item| item.user_visible_outcome.clone())
                .collect(),
            explicit_exclusions: deduplicate(request.explicit_exclusions),
            candidate_plan_steps: request.candidate_plan_steps,
            acceptance_checks: deduplicate(request.acceptance_checks),
            estimated_external_waits: deduplicate(request.estimated_external_waits),
            external_costs: deduplicate(request.external_costs),
            risks: deduplicate(request.risks),
            required_decisions: Vec::new(),
            repair_policy: request.repair_policy,
            proposal_digest: String::new(),
        };
        proposal.proposal_digest = digest_with_empty_field(&proposal, "proposalDigest")?;
        for item in &mut selected {
            item.status = WorkItemStatus::Proposed;
        }
        append_record(
            &mut self.journal,
            &self.storage,
            &request.command_id,
            ProjectIntentJournalRecord::ChangeSetPrepared {
                proposal: proposal.clone(),
                work_items: selected,
            },
        )?;
        Ok(ChangePreparationResult::Ready(proposal))
    }

    pub fn authorize(
        &mut self,
        input: ChangeSetApprovalInput,
        session: Option<&EditorSession>,
    ) -> Result<ProjectProductionRun, ProjectIntentWorkflowError> {
        if let Some(entry) = self
            .journal
            .entries
            .iter()
            .find(|entry| entry.command_id == input.command_id)
        {
            let ProjectIntentJournalRecord::RunAuthorized { run, .. } = &entry.record else {
                return Err(command_replay_mismatch(&input.command_id));
            };
            return Ok(run.clone());
        }
        let snapshot = rebuild_snapshot(&self.journal)?;
        let proposal = snapshot.active_proposal.as_ref().ok_or_else(|| {
            workflow_error(
                "project_intent.proposal_missing",
                "There is no active ChangeSetProposal to approve.",
                "Prepare a ChangeSet from selected ready WorkItems first.",
            )
        })?;
        if self.journal.entries.iter().any(|entry| {
            matches!(
                &entry.record,
                ProjectIntentJournalRecord::RunAuthorized { approval, .. }
                    if approval.approval_id == input.approval_id
            )
        }) {
            return Err(workflow_error(
                "project_intent.approval_replay_rejected",
                "Approval identity was already consumed by another authorization command.",
                "Record a new explicit approval for the current ChangeSetProposal.",
            ));
        }
        validate_approval_input(&input, proposal, &snapshot, session)?;
        if snapshot.active_run.as_ref().is_some_and(|run| {
            run.state.holds_mutation_lane() && run.target_project_identity == input.target_identity
        }) {
            return Err(workflow_error(
                "project_intent.mutation_lane_busy",
                "Another ProjectProductionRun already owns this project's mutation lane.",
                "Complete, cancel, or recover the active run before authorizing another.",
            ));
        }
        let mut approval = ChangeSetApproval {
            schema_version: CHANGE_SET_APPROVAL_SCHEMA_VERSION.to_string(),
            approval_id: input.approval_id,
            approved_by: input.approved_by,
            proposal_digest: proposal.proposal_digest.clone(),
            selected_work_item_digests: proposal
                .selected_work_item_revisions
                .iter()
                .map(|binding| binding.work_item_digest.clone())
                .collect(),
            target_identity: input.target_identity.clone(),
            expected_base_project_digest: input.expected_base_project_digest,
            approved_risk_classes: deduplicate(input.approved_risk_classes),
            approved_external_costs: deduplicate(input.approved_external_costs),
            approved_repair_policy: input.approved_repair_policy,
            approved_at: input.approved_at.unwrap_or_else(timestamp_string),
            approval_digest: String::new(),
        };
        approval.approval_digest = digest_with_empty_field(&approval, "approvalDigest")?;
        let mut work_items = selected_work_items(proposal, &snapshot)?;
        for item in &mut work_items {
            item.status = WorkItemStatus::InProgress;
        }
        let run_kind = match proposal.target_kind {
            ChangeSetTargetKind::NewProject => ProjectProductionRunKind::FromBlank,
            ChangeSetTargetKind::ExistingProject => ProjectProductionRunKind::ScopedChange,
        };
        let run = ProjectProductionRun {
            schema_version: PROJECT_PRODUCTION_RUN_SCHEMA_VERSION.to_string(),
            run_id: next_id("production-run", self.journal.revision + 1, 0),
            run_kind,
            proposal_id: proposal.proposal_id.clone(),
            change_set_approval_digest: approval.approval_digest.clone(),
            target_project_identity: input.target_identity,
            base_project_digest: proposal.expected_base_project_digest.clone(),
            current_project_digest: proposal.expected_base_project_digest.clone(),
            state: ProjectProductionRunState::Approved,
            active_step_id: None,
            step_snapshots: proposal
                .candidate_plan_steps
                .iter()
                .map(|step| ProductionStepSnapshot {
                    step_id: step.step_id.clone(),
                    state: ProductionStepState::Pending,
                    candidate_id: None,
                    candidate_digest: None,
                    validation_digest: None,
                    apply_receipt: None,
                    diagnostics: Vec::new(),
                })
                .collect(),
            linked_work_item_revisions: proposal.selected_work_item_revisions.clone(),
            decision_requests: Vec::new(),
            recovery_options: vec![
                "retry".to_string(),
                "reprepare".to_string(),
                "keep_current_project_and_stop".to_string(),
                "park_affected_work_items".to_string(),
            ],
            preview_evidence: None,
            diagnostics: Vec::new(),
            started_at: timestamp_string(),
            completed_at: None,
        };
        append_record(
            &mut self.journal,
            &self.storage,
            &input.command_id,
            ProjectIntentJournalRecord::RunAuthorized {
                approval,
                run: run.clone(),
                work_items,
            },
        )?;
        Ok(run)
    }

    pub fn dispatch(
        &mut self,
        command: ProjectIntentWorkflowCommand,
        session: Option<&mut EditorSession>,
    ) -> Result<ProjectIntentSnapshot, ProjectIntentWorkflowError> {
        if self
            .journal
            .entries
            .iter()
            .any(|entry| entry.command_id == command.command_id())
        {
            return rebuild_snapshot(&self.journal);
        }
        match command {
            ProjectIntentWorkflowCommand::CreateWorkItem { command_id, draft } => {
                let item = self.new_work_item(draft, Vec::new(), 0)?;
                self.append_work_item(&command_id, item)?;
            }
            ProjectIntentWorkflowCommand::ReviseWorkItem {
                command_id,
                work_item_id,
                draft,
            } => {
                let current = self.work_item(&work_item_id)?;
                let mut item = self.new_work_item(draft, current.prior_work_item_ids.clone(), 0)?;
                item.work_item_id = current.work_item_id;
                item.source_event_ids = union(current.source_event_ids, item.source_event_ids);
                item.revision = current.revision + 1;
                item.work_item_digest = work_item_semantic_digest(&item)?;
                self.append_work_item(&command_id, item)?;
            }
            ProjectIntentWorkflowCommand::ParkWorkItem {
                command_id,
                work_item_id,
            } => self.set_work_item_status(&command_id, &work_item_id, WorkItemStatus::Parked)?,
            ProjectIntentWorkflowCommand::ResumeWorkItem {
                command_id,
                work_item_id,
            } => {
                let item = self.work_item(&work_item_id)?;
                if item.status != WorkItemStatus::Parked {
                    return Err(invalid_transition(&item, "resume"));
                }
                let status = if item.open_questions.is_empty() {
                    WorkItemStatus::Ready
                } else {
                    WorkItemStatus::NeedsClarification
                };
                self.set_work_item_status(&command_id, &work_item_id, status)?;
            }
            ProjectIntentWorkflowCommand::CancelWorkItem {
                command_id,
                work_item_id,
            } => {
                self.set_work_item_status(&command_id, &work_item_id, WorkItemStatus::Cancelled)?
            }
            ProjectIntentWorkflowCommand::ReopenWorkItem {
                command_id,
                work_item_id,
                evidence_refs,
            } => self.reopen_work_item(&command_id, &work_item_id, evidence_refs)?,
            ProjectIntentWorkflowCommand::MergeWorkItems {
                command_id,
                source_work_item_ids,
                merged,
            } => self.merge_work_items(&command_id, source_work_item_ids, merged)?,
            ProjectIntentWorkflowCommand::SplitWorkItem {
                command_id,
                source_work_item_id,
                parts,
            } => self.split_work_item(&command_id, &source_work_item_id, parts)?,
            ProjectIntentWorkflowCommand::StartDiagnosis {
                command_id,
                work_item_id,
                base_project_digest,
            } => self.start_diagnosis(&command_id, &work_item_id, base_project_digest)?,
            ProjectIntentWorkflowCommand::UpdateDiagnosis { command_id, update } => {
                self.update_diagnosis(&command_id, update)?
            }
            ProjectIntentWorkflowCommand::AdvanceRun { command_id, run_id } => {
                let session = session.ok_or_else(|| {
                    workflow_error(
                        "project_intent.execution_context_required",
                        "AdvanceRun requires the active EditorSession execution context.",
                        "Dispatch the command through EditorSession.",
                    )
                })?;
                execution::advance_run(self, &command_id, &run_id, session)?;
            }
            ProjectIntentWorkflowCommand::CancelRun { command_id, run_id } => {
                self.change_run_state(&command_id, &run_id, ProjectProductionRunState::Cancelled)?;
            }
            ProjectIntentWorkflowCommand::RecoverRun { command_id, run_id } => {
                self.change_run_state(&command_id, &run_id, ProjectProductionRunState::Approved)?;
            }
        }
        rebuild_snapshot(&self.journal)
    }

    pub fn project_goal_snapshot(
        &self,
        work_item_ids: &[String],
    ) -> Result<ProjectGoalSnapshot, ProjectIntentWorkflowError> {
        let snapshot = rebuild_snapshot(&self.journal)?;
        let ids = work_item_ids.iter().collect::<BTreeSet<_>>();
        let selected = snapshot
            .work_items
            .iter()
            .filter(|item| ids.contains(&item.work_item_id))
            .collect::<Vec<_>>();
        if selected.len() != ids.len() {
            return Err(workflow_error(
                "project_intent.goal_work_item_missing",
                "ProjectGoalSnapshot references an unknown WorkItem.",
                "Select only current WorkItem ids.",
            ));
        }
        let mut goal = ProjectGoalSnapshot {
            schema_version: "project-goal-snapshot.v1".to_string(),
            snapshot_id: next_id("goal-snapshot", self.journal.revision, 0),
            project_identity: snapshot
                .project_binding
                .as_ref()
                .map(|binding| binding.project_id.clone()),
            included_work_item_revisions: selected
                .iter()
                .map(|item| work_item_binding(item))
                .collect(),
            goals: selected
                .iter()
                .map(|item| item.user_visible_outcome.clone())
                .collect(),
            constraints: selected
                .iter()
                .flat_map(|item| item.constraints.iter().cloned())
                .collect(),
            explicitly_deferred: selected
                .iter()
                .flat_map(|item| item.explicitly_deferred.iter().cloned())
                .collect(),
            snapshot_digest: String::new(),
        };
        goal.snapshot_digest = digest_with_empty_field(&goal, "snapshotDigest")?;
        Ok(goal)
    }

    pub fn attach_created_project(
        &mut self,
        session: &EditorSession,
        create_command_id: &str,
    ) -> Result<ProjectIntentProjectBinding, ProjectIntentWorkflowError> {
        let previous_draft_path = match &self.storage {
            ProjectIntentStorage::LocalDraft(path) => Some(path.clone()),
            ProjectIntentStorage::InMemory => None,
            ProjectIntentStorage::Project { .. } => {
                return Err(workflow_error(
                    "project_intent.already_project_bound",
                    "Intent workflow is already attached to a project journal.",
                    "Open the bound project or start a separate pre-project draft.",
                ));
            }
        };
        let project = session.active_project_session().ok_or_else(|| {
            workflow_error(
                "project_intent.created_project_missing",
                "CreateProject completed without an active project session.",
                "Do not discard the pre-project draft; retry formal project creation.",
            )
        })?;
        let next_storage = ProjectIntentStorage::Project {
            write_scope: project.write_scope().clone(),
        };
        let original_storage = self.storage.clone();
        self.storage = next_storage;
        let result = self.attach_project_binding(
            session,
            create_command_id,
            &format!("project_intent.attach.created.{create_command_id}"),
        );
        if result.is_err() {
            self.storage = original_storage;
            return result;
        }
        if let Some(path) = previous_draft_path {
            let _ = fs::remove_file(path);
        }
        result
    }

    pub(crate) fn journal(&self) -> &ProjectIntentJournalDocument {
        &self.journal
    }

    pub(crate) fn append_run(
        &mut self,
        command_id: &str,
        run: ProjectProductionRun,
        work_items: Vec<WorkItem>,
    ) -> Result<(), ProjectIntentWorkflowError> {
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::RunChanged { run, work_items },
        )?;
        Ok(())
    }

    fn attach_project_binding(
        &mut self,
        session: &EditorSession,
        receipt_identity: &str,
        command_id: &str,
    ) -> Result<ProjectIntentProjectBinding, ProjectIntentWorkflowError> {
        let candidate_binding = ProjectCandidateEntry::inspect_project_binding(session)
            .map_err(candidate_entry_error)?;
        let creation_receipt_digest = digest_record(&serde_json::json!({
            "receiptIdentity": receipt_identity,
            "projectId": candidate_binding.project_id,
            "projectRoot": candidate_binding.project_root,
            "initialProjectDigest": candidate_binding.project_digest,
        }))?;
        let binding = ProjectIntentProjectBinding {
            schema_version: PROJECT_INTENT_PROJECT_BINDING_SCHEMA_VERSION.to_string(),
            project_id: candidate_binding.project_id,
            project_root: candidate_binding.project_root,
            initial_project_digest: candidate_binding.project_digest,
            creation_receipt_digest,
        };
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::ProjectAttached(binding.clone()),
        )?;
        Ok(binding)
    }

    fn new_work_item(
        &self,
        draft: WorkItemDraft,
        prior_work_item_ids: Vec<String>,
        offset: usize,
    ) -> Result<WorkItem, ProjectIntentWorkflowError> {
        validate_work_item_draft(&draft, &rebuild_snapshot(&self.journal)?)?;
        let mut item = WorkItem {
            schema_version: WORK_ITEM_SCHEMA_VERSION.to_string(),
            work_item_id: next_id("work-item", self.journal.revision + 1, offset),
            kind: draft.kind,
            title: draft.title,
            user_visible_outcome: draft.user_visible_outcome,
            source_event_ids: deduplicate(draft.source_event_ids),
            status: draft.status,
            priority: draft.priority,
            scope_hints: deduplicate(draft.scope_hints),
            constraints: deduplicate(draft.constraints),
            acceptance_criteria: deduplicate(draft.acceptance_criteria),
            open_questions: deduplicate(draft.open_questions),
            evidence_refs: deduplicate(draft.evidence_refs),
            relationship_refs: deduplicate_by_key(draft.relationship_refs, |relation| {
                format!("{:?}:{}", relation.kind, relation.target_work_item_id)
            }),
            latest_understanding: draft.latest_understanding,
            explicitly_deferred: deduplicate(draft.explicitly_deferred),
            prior_work_item_ids: deduplicate(prior_work_item_ids),
            revision: 1,
            work_item_digest: String::new(),
            normalization_source_label: None,
        };
        item.work_item_digest = work_item_semantic_digest(&item)?;
        Ok(item)
    }

    fn work_item(&self, work_item_id: &str) -> Result<WorkItem, ProjectIntentWorkflowError> {
        rebuild_snapshot(&self.journal)?
            .work_items
            .into_iter()
            .find(|item| item.work_item_id == work_item_id)
            .ok_or_else(|| {
                workflow_error(
                    "project_intent.work_item_missing",
                    format!("WorkItem {work_item_id} does not exist."),
                    "Use a current WorkItem id from observe().",
                )
            })
    }

    fn append_work_item(
        &mut self,
        command_id: &str,
        item: WorkItem,
    ) -> Result<(), ProjectIntentWorkflowError> {
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::WorkItemChanged(item),
        )?;
        Ok(())
    }

    fn set_work_item_status(
        &mut self,
        command_id: &str,
        work_item_id: &str,
        status: WorkItemStatus,
    ) -> Result<(), ProjectIntentWorkflowError> {
        let mut item = self.work_item(work_item_id)?;
        if item.status.is_terminal() && status != WorkItemStatus::Ready {
            return Err(invalid_transition(&item, "change status"));
        }
        item.status = status;
        self.append_work_item(command_id, item)
    }

    fn reopen_work_item(
        &mut self,
        command_id: &str,
        work_item_id: &str,
        evidence_refs: Vec<String>,
    ) -> Result<(), ProjectIntentWorkflowError> {
        let snapshot = rebuild_snapshot(&self.journal)?;
        let mut item = snapshot
            .work_items
            .iter()
            .find(|item| item.work_item_id == work_item_id)
            .cloned()
            .ok_or_else(|| {
                workflow_error(
                    "project_intent.work_item_missing",
                    "WorkItem does not exist.",
                    "Use a current WorkItem id.",
                )
            })?;
        if item.status != WorkItemStatus::Done {
            return Err(invalid_transition(&item, "reopen"));
        }
        let mut links = evidence_refs;
        if let Some(proposal) = &snapshot.active_proposal {
            if proposal
                .selected_work_item_revisions
                .iter()
                .any(|binding| binding.work_item_id == work_item_id)
            {
                links.push(format!("proposal:{}", proposal.proposal_id));
            }
        }
        if let Some(run) = &snapshot.active_run {
            if run
                .linked_work_item_revisions
                .iter()
                .any(|binding| binding.work_item_id == work_item_id)
            {
                links.extend(run.step_snapshots.iter().filter_map(|step| {
                    step.apply_receipt
                        .as_ref()
                        .map(|receipt| format!("receipt:{}", receipt.receipt_binding_digest))
                }));
                if let Some(verification) = &run.preview_evidence {
                    links.push(format!("verification:{verification}"));
                }
            }
        }
        item.evidence_refs = union(item.evidence_refs, links);
        item.status = if item.evidence_refs.is_empty() {
            WorkItemStatus::NeedsEvidence
        } else {
            WorkItemStatus::Ready
        };
        item.revision += 1;
        item.work_item_digest = work_item_semantic_digest(&item)?;
        self.append_work_item(command_id, item)
    }

    fn merge_work_items(
        &mut self,
        command_id: &str,
        source_ids: Vec<String>,
        mut draft: WorkItemDraft,
    ) -> Result<(), ProjectIntentWorkflowError> {
        let source_ids = deduplicate(source_ids);
        if source_ids.len() < 2 {
            return Err(workflow_error(
                "project_intent.merge_requires_multiple",
                "MergeWorkItems requires at least two distinct source WorkItems.",
                "Select two or more related WorkItems.",
            ));
        }
        let mut sources = source_ids
            .iter()
            .map(|id| self.work_item(id))
            .collect::<Result<Vec<_>, _>>()?;
        for source in &sources {
            draft.source_event_ids = union(draft.source_event_ids, source.source_event_ids.clone());
        }
        let merged = self.new_work_item(draft, source_ids, sources.len())?;
        for source in &mut sources {
            source.status = WorkItemStatus::Merged;
        }
        sources.push(merged);
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::WorkItemsChanged(sources),
        )?;
        Ok(())
    }

    fn split_work_item(
        &mut self,
        command_id: &str,
        source_id: &str,
        parts: Vec<WorkItemDraft>,
    ) -> Result<(), ProjectIntentWorkflowError> {
        if parts.len() < 2 {
            return Err(workflow_error(
                "project_intent.split_requires_multiple",
                "SplitWorkItem requires at least two resulting WorkItems.",
                "Provide two or more independent outcomes.",
            ));
        }
        let mut source = self.work_item(source_id)?;
        let source_events = source.source_event_ids.clone();
        source.status = WorkItemStatus::Split;
        let mut changed = vec![source];
        for (index, mut draft) in parts.into_iter().enumerate() {
            draft.source_event_ids = union(source_events.clone(), draft.source_event_ids);
            changed.push(self.new_work_item(draft, vec![source_id.to_string()], index + 1)?);
        }
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::WorkItemsChanged(changed),
        )?;
        Ok(())
    }

    fn start_diagnosis(
        &mut self,
        command_id: &str,
        work_item_id: &str,
        base_project_digest: Option<String>,
    ) -> Result<(), ProjectIntentWorkflowError> {
        let mut work_item = self.work_item(work_item_id)?;
        if work_item.kind != WorkItemKind::Bug {
            return Err(workflow_error(
                "project_intent.diagnosis_requires_bug",
                "ProjectDiagnosisSession can only be started for a Bug WorkItem.",
                "Create or revise the WorkItem as kind=bug.",
            ));
        }
        let snapshot = rebuild_snapshot(&self.journal)?;
        if snapshot
            .active_diagnoses
            .iter()
            .any(|diagnosis| diagnosis.work_item_id == work_item_id)
        {
            return Err(workflow_error(
                "project_intent.diagnosis_already_active",
                "Bug WorkItem already has an active diagnosis session.",
                "Continue the existing diagnosis instead of creating a duplicate.",
            ));
        }
        work_item.status = WorkItemStatus::NeedsEvidence;
        let mut diagnosis = ProjectDiagnosisSession {
            schema_version: PROJECT_DIAGNOSIS_SCHEMA_VERSION.to_string(),
            diagnosis_id: next_id("diagnosis", self.journal.revision + 1, 0),
            work_item_id: work_item_id.to_string(),
            base_project_digest,
            state: DiagnosisState::NeedsEvidence,
            reproduction_attempts: Vec::new(),
            observations: Vec::new(),
            hypotheses: Vec::new(),
            confirmed_cause: None,
            evidence_refs: Vec::new(),
            proposed_fix_scope: Vec::new(),
            diagnosis_digest: String::new(),
        };
        diagnosis.diagnosis_digest = digest_with_empty_field(&diagnosis, "diagnosisDigest")?;
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::DiagnosisChanged(diagnosis),
        )?;
        let status_command = format!("{command_id}/work-item-state");
        self.append_work_item(&status_command, work_item)
    }

    fn update_diagnosis(
        &mut self,
        command_id: &str,
        update: DiagnosisUpdate,
    ) -> Result<(), ProjectIntentWorkflowError> {
        if update
            .requested_capabilities
            .iter()
            .any(|capability| !capability.is_read_only())
        {
            return Err(workflow_error(
                "project_intent.diagnosis_change_set_required",
                "Diagnosis requested a project mutation or external authority.",
                "Prepare and approve a ChangeSetProposal for instrumentation, dependency, project, or network changes.",
            ));
        }
        let snapshot = rebuild_snapshot(&self.journal)?;
        let mut diagnosis = snapshot
            .active_diagnoses
            .into_iter()
            .find(|diagnosis| diagnosis.diagnosis_id == update.diagnosis_id)
            .ok_or_else(|| {
                workflow_error(
                    "project_intent.diagnosis_missing",
                    "Active diagnosis session does not exist.",
                    "Use a current diagnosis id from observe().",
                )
            })?;
        diagnosis.state = update.state;
        diagnosis.reproduction_attempts = union(
            diagnosis.reproduction_attempts,
            update.reproduction_attempts,
        );
        diagnosis.observations = union(diagnosis.observations, update.observations);
        diagnosis.hypotheses = deduplicate_by_key(
            diagnosis
                .hypotheses
                .into_iter()
                .chain(update.hypotheses)
                .collect(),
            |hypothesis| hypothesis.hypothesis_id.clone(),
        );
        diagnosis.confirmed_cause = update.confirmed_cause.or(diagnosis.confirmed_cause);
        diagnosis.evidence_refs = union(diagnosis.evidence_refs, update.evidence_refs);
        diagnosis.proposed_fix_scope =
            union(diagnosis.proposed_fix_scope, update.proposed_fix_scope);
        diagnosis.diagnosis_digest = digest_with_empty_field(&diagnosis, "diagnosisDigest")?;
        append_record(
            &mut self.journal,
            &self.storage,
            command_id,
            ProjectIntentJournalRecord::DiagnosisChanged(diagnosis),
        )?;
        Ok(())
    }

    fn change_run_state(
        &mut self,
        command_id: &str,
        run_id: &str,
        state: ProjectProductionRunState,
    ) -> Result<(), ProjectIntentWorkflowError> {
        let snapshot = rebuild_snapshot(&self.journal)?;
        let mut run = snapshot.active_run.ok_or_else(|| {
            workflow_error(
                "project_intent.run_missing",
                "There is no active ProjectProductionRun.",
                "Authorize a current ChangeSetProposal first.",
            )
        })?;
        if run.run_id != run_id {
            return Err(workflow_error(
                "project_intent.run_identity_mismatch",
                "Run command does not target the active run.",
                "Use the active run id from observe().",
            ));
        }
        run.state = state;
        if matches!(state, ProjectProductionRunState::Cancelled) {
            run.completed_at = Some(timestamp_string());
        }
        self.append_run(command_id, run, Vec::new())
    }
}

fn validate_capture_input(input: &IntentCaptureInput) -> Result<(), ProjectIntentWorkflowError> {
    if input.command_id.trim().is_empty() || input.source_identity.trim().is_empty() {
        return Err(workflow_error(
            "project_intent.capture_identity_missing",
            "Intent capture requires stable command and source identities.",
            "Provide local command_id and source_identity values.",
        ));
    }
    if input.sanitized_summary.trim().is_empty()
        && input
            .content_ref
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && input.attachment_refs.is_empty()
    {
        return Err(workflow_error(
            "project_intent.capture_content_missing",
            "Intent capture has no summary, source reference, or attachment.",
            "Preserve at least one source representation; it may remain incomplete or contradictory.",
        ));
    }
    Ok(())
}

fn validate_work_item_draft(
    draft: &WorkItemDraft,
    snapshot: &ProjectIntentSnapshot,
) -> Result<(), ProjectIntentWorkflowError> {
    if draft.title.trim().is_empty()
        || draft.user_visible_outcome.trim().is_empty()
        || draft.latest_understanding.trim().is_empty()
    {
        return Err(workflow_error(
            "project_intent.work_item_summary_missing",
            "WorkItem requires a title, user-visible outcome, and current understanding.",
            "Normalize only what is currently understood; keep uncertainty in openQuestions.",
        ));
    }
    let event_ids = snapshot
        .intent_events
        .iter()
        .map(|event| event.event_id.as_str())
        .collect::<BTreeSet<_>>();
    if draft.source_event_ids.is_empty()
        || draft
            .source_event_ids
            .iter()
            .any(|event_id| !event_ids.contains(event_id.as_str()))
    {
        return Err(workflow_error(
            "project_intent.work_item_lineage_invalid",
            "WorkItem must retain at least one valid source IntentEvent.",
            "Use event ids returned by capture().",
        ));
    }
    let work_item_ids = snapshot
        .work_items
        .iter()
        .map(|item| item.work_item_id.as_str())
        .collect::<BTreeSet<_>>();
    if draft.relationship_refs.iter().any(|relation| {
        relation.target_work_item_id.trim().is_empty()
            || !work_item_ids.contains(relation.target_work_item_id.as_str())
    }) {
        return Err(workflow_error(
            "project_intent.work_item_relationship_invalid",
            "WorkItem relationship references an unknown target.",
            "Use only current WorkItem ids and the fixed relationship set.",
        ));
    }
    Ok(())
}

fn validate_change_request(
    request: &ChangePreparationRequest,
    snapshot: &ProjectIntentSnapshot,
    storage: &ProjectIntentStorage,
) -> Result<Vec<ChangePreparationBlocker>, ProjectIntentWorkflowError> {
    if request.command_id.trim().is_empty() {
        return Err(workflow_error(
            "project_intent.command_id_missing",
            "Change preparation requires a stable command id.",
            "Generate one id and reuse it only for exact retries.",
        ));
    }
    let mut blockers = Vec::new();
    if snapshot
        .active_run
        .as_ref()
        .is_some_and(|run| run.state.holds_mutation_lane())
    {
        blockers.push(change_blocker(
            "project_intent.mutation_lane_busy",
            None,
            "An active ProjectProductionRun already owns the project mutation lane.",
            "Continue discussing WorkItems, but complete or cancel the active run before preparing another ChangeSet.",
        ));
    }
    if request.selected_work_item_ids.is_empty() {
        blockers.push(change_blocker(
            "project_intent.change_set_empty",
            None,
            "ChangeSetProposal must select at least one WorkItem.",
            "Select only the ready outcomes to implement now.",
        ));
    }
    if request.candidate_plan_steps.is_empty() {
        blockers.push(change_blocker(
            "project_intent.change_plan_empty",
            None,
            "ChangeSetProposal has no project mutation steps.",
            "Add at least one ProjectPatch, AssetImport, or ControlledSourcePatch plan step.",
        ));
    }
    if !request.required_decisions.is_empty() {
        blockers.push(change_blocker(
            "project_intent.change_set_decisions_pending",
            None,
            "Selected ChangeSet still has implementation-changing decisions.",
            "Resolve only decisions directly required by this ChangeSet.",
        ));
    }
    let mut step_ids = BTreeSet::new();
    for step in &request.candidate_plan_steps {
        if step.step_id.trim().is_empty() || !step_ids.insert(step.step_id.as_str()) {
            blockers.push(change_blocker(
                "project_intent.change_step_identity_invalid",
                None,
                "Candidate plan step ids must be non-empty and unique.",
                "Assign one stable id per planned mutation.",
            ));
        }
        if payload_kind(&step.payload) != step.payload_kind {
            blockers.push(change_blocker(
                "project_intent.change_step_payload_kind_mismatch",
                None,
                format!("Step {} declares the wrong payload kind.", step.step_id),
                "Match payloadKind to the strict candidate payload schema.",
            ));
        }
        if digest_record(&step.payload)? != step.payload_source_digest {
            blockers.push(change_blocker(
                "project_intent.change_step_source_digest_mismatch",
                None,
                format!("Step {} source digest is stale or tampered.", step.step_id),
                "Recompute the payload source digest from canonical content.",
            ));
        }
    }
    for step in &request.candidate_plan_steps {
        if step
            .depends_on
            .iter()
            .any(|id| !step_ids.contains(id.as_str()))
        {
            blockers.push(change_blocker(
                "project_intent.change_step_dependency_missing",
                None,
                format!("Step {} depends on an unknown step.", step.step_id),
                "Reference only steps in this ChangeSetProposal.",
            ));
        }
    }
    match request.target_kind {
        ChangeSetTargetKind::NewProject => {
            let Some(spec) = &request.project_create_spec else {
                blockers.push(change_blocker(
                    "project_intent.project_create_spec_missing",
                    None,
                    "New-project ChangeSet requires a ProjectCreateSpec.",
                    "Choose the project name and target root for approval review.",
                ));
                return Ok(blockers);
            };
            if Path::new(&spec.project_root).exists() {
                blockers.push(change_blocker(
                    "project_intent.from_blank_target_exists",
                    None,
                    "From-blank target root already exists before approval.",
                    "Choose a new absent project root.",
                ));
            }
            if let ProjectIntentStorage::LocalDraft(draft_path) = storage {
                if path_overlaps(draft_path, Path::new(&spec.project_root)) {
                    blockers.push(change_blocker(
                        "project_intent.draft_target_overlap",
                        None,
                        "Launcher draft storage overlaps the from-blank target root.",
                        "Keep pre-project intent storage outside the future project root.",
                    ));
                }
            }
            if request.expected_base_project_digest.is_some() {
                blockers.push(change_blocker(
                    "project_intent.from_blank_base_forbidden",
                    None,
                    "From-blank ChangeSet cannot bind a pre-existing project digest.",
                    "Leave expectedBaseProjectDigest empty until formal CreateProject completes.",
                ));
            }
        }
        ChangeSetTargetKind::ExistingProject => {
            let Some(binding) = &snapshot.project_binding else {
                blockers.push(change_blocker(
                    "project_intent.project_binding_missing",
                    None,
                    "Existing-project ChangeSet requires an attached project journal.",
                    "Open the project and attach its journal first.",
                ));
                return Ok(blockers);
            };
            if request.target_project_identity.as_deref() != Some(binding.project_id.as_str()) {
                blockers.push(change_blocker(
                    "project_intent.target_identity_mismatch",
                    None,
                    "ChangeSet target does not match the attached project identity.",
                    "Reprepare against the currently open project.",
                ));
            }
            if request.expected_base_project_digest.is_none() {
                blockers.push(change_blocker(
                    "project_intent.base_digest_missing",
                    None,
                    "Existing-project ChangeSet requires an expected base digest.",
                    "Inspect and bind the current project digest.",
                ));
            }
        }
    }
    Ok(blockers)
}

fn validate_approval_input(
    input: &ChangeSetApprovalInput,
    proposal: &ChangeSetProposal,
    snapshot: &ProjectIntentSnapshot,
    session: Option<&EditorSession>,
) -> Result<(), ProjectIntentWorkflowError> {
    if input.approval_id.trim().is_empty() || input.approved_by.trim().is_empty() {
        return Err(workflow_error(
            "project_intent.approval_identity_missing",
            "ChangeSet approval requires approval and actor identities.",
            "Record the explicit user approval identity.",
        ));
    }
    if input.proposal_digest != proposal.proposal_digest {
        return Err(workflow_error(
            "project_intent.approval_proposal_mismatch",
            "Approval does not bind the active ChangeSetProposal digest.",
            "Review and approve the exact current proposal.",
        ));
    }
    if sorted(&input.approved_risk_classes) != sorted(&proposal.risks)
        || sorted(&input.approved_external_costs) != sorted(&proposal.external_costs)
        || input.approved_repair_policy != proposal.repair_policy
    {
        return Err(workflow_error(
            "project_intent.approval_scope_mismatch",
            "Approval risk, cost, or repair scope differs from the proposal.",
            "Approve the exact reviewed scope without expansion or omission.",
        ));
    }
    for binding in &proposal.selected_work_item_revisions {
        let current = snapshot
            .work_items
            .iter()
            .find(|item| item.work_item_id == binding.work_item_id)
            .ok_or_else(|| {
                workflow_error(
                    "project_intent.approval_work_item_missing",
                    "Selected WorkItem disappeared.",
                    "Reprepare the ChangeSet.",
                )
            })?;
        if current.revision != binding.revision
            || current.work_item_digest != binding.work_item_digest
        {
            return Err(workflow_error(
                "project_intent.approval_work_item_stale",
                format!(
                    "Selected WorkItem {} changed semantically.",
                    binding.work_item_id
                ),
                "Reprepare and approve a proposal bound to its current revision.",
            ));
        }
    }
    match proposal.target_kind {
        ChangeSetTargetKind::NewProject => {
            let spec = proposal.project_create_spec.as_ref().ok_or_else(|| {
                workflow_error(
                    "project_intent.project_create_spec_missing",
                    "ProjectCreateSpec is missing.",
                    "Reprepare the proposal.",
                )
            })?;
            if input.target_identity != spec.project_root {
                return Err(workflow_error(
                    "project_intent.approval_target_mismatch",
                    "Approval target does not match the reviewed from-blank root.",
                    "Approve the exact reviewed target root.",
                ));
            }
            if Path::new(&spec.project_root).exists() {
                return Err(workflow_error(
                    "project_intent.from_blank_target_drifted",
                    "From-blank target was created before authorized execution.",
                    "Choose a fresh absent root and reprepare.",
                ));
            }
        }
        ChangeSetTargetKind::ExistingProject => {
            let session = session.ok_or_else(|| {
                workflow_error(
                    "project_intent.execution_context_required",
                    "Existing-project authorization requires the active EditorSession.",
                    "Authorize through EditorSession.",
                )
            })?;
            let binding = ProjectCandidateEntry::inspect_project_binding(session)
                .map_err(candidate_entry_error)?;
            if input.target_identity != binding.project_id
                || proposal.target_project_identity.as_deref() != Some(binding.project_id.as_str())
                || proposal.expected_base_project_digest.as_deref()
                    != Some(binding.project_digest.as_str())
                || input.expected_base_project_digest.as_deref()
                    != Some(binding.project_digest.as_str())
            {
                return Err(workflow_error(
                    "project_intent.approval_base_drifted",
                    "Project identity or base digest changed before authorization.",
                    "Preserve manual changes and reprepare against the current project.",
                ));
            }
        }
    }
    Ok(())
}

fn selected_work_items(
    proposal: &ChangeSetProposal,
    snapshot: &ProjectIntentSnapshot,
) -> Result<Vec<WorkItem>, ProjectIntentWorkflowError> {
    proposal
        .selected_work_item_revisions
        .iter()
        .map(|binding| {
            snapshot
                .work_items
                .iter()
                .find(|item| item.work_item_id == binding.work_item_id)
                .cloned()
                .ok_or_else(|| {
                    workflow_error(
                        "project_intent.work_item_missing",
                        "Selected WorkItem is missing.",
                        "Reprepare the ChangeSet.",
                    )
                })
        })
        .collect()
}

fn work_item_binding(item: &WorkItem) -> WorkItemRevisionBinding {
    WorkItemRevisionBinding {
        work_item_id: item.work_item_id.clone(),
        revision: item.revision,
        work_item_digest: item.work_item_digest.clone(),
    }
}

fn work_item_semantic_digest(item: &WorkItem) -> Result<String, ProjectIntentWorkflowError> {
    digest_record(&serde_json::json!({
        "schemaVersion": item.schema_version,
        "workItemId": item.work_item_id,
        "kind": item.kind,
        "title": item.title,
        "userVisibleOutcome": item.user_visible_outcome,
        "sourceEventIds": item.source_event_ids,
        "priority": item.priority,
        "scopeHints": item.scope_hints,
        "constraints": item.constraints,
        "acceptanceCriteria": item.acceptance_criteria,
        "openQuestions": item.open_questions,
        "relationshipRefs": item.relationship_refs,
        "latestUnderstanding": item.latest_understanding,
        "explicitlyDeferred": item.explicitly_deferred,
        "priorWorkItemIds": item.prior_work_item_ids,
        "revision": item.revision,
    }))
}

fn payload_kind(payload: &crate::ProjectCandidatePayload) -> CandidatePayloadKind {
    match payload {
        crate::ProjectCandidatePayload::AssetImport { .. } => CandidatePayloadKind::AssetImport,
        crate::ProjectCandidatePayload::ProjectPatch(_) => CandidatePayloadKind::ProjectPatch,
        crate::ProjectCandidatePayload::ControlledSourcePatch { .. } => {
            CandidatePayloadKind::ControlledSourcePatch
        }
    }
}

fn next_id(prefix: &str, revision: u64, offset: usize) -> String {
    format!("{prefix}-{revision:016}-{offset:04}")
}

fn digest_with_empty_field<T: Serialize>(
    value: &T,
    field: &str,
) -> Result<String, ProjectIntentWorkflowError> {
    let mut value = serde_json::to_value(value).map_err(|error| {
        workflow_error(
            "project_intent.digest_serialize_failed",
            error.to_string(),
            "Fix the schema before computing its digest.",
        )
    })?;
    value
        .as_object_mut()
        .ok_or_else(|| {
            workflow_error(
                "project_intent.digest_shape_invalid",
                "Digest-bound schema is not an object.",
                "Use a strict object schema.",
            )
        })?
        .insert(field.to_string(), serde_json::Value::String(String::new()));
    digest_record(&value)
}

fn sorted(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    values
}

fn deduplicate(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty() && seen.insert(value.clone()))
        .collect()
}

fn union(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    deduplicate(left.into_iter().chain(right).collect())
}

fn deduplicate_by_key<T>(values: Vec<T>, key: impl Fn(&T) -> String) -> Vec<T> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(key(value)))
        .collect()
}

fn path_overlaps(left: &Path, right: &Path) -> bool {
    let left = normalize_absolute(left);
    let right = normalize_absolute(right);
    left.starts_with(&right) || right.starts_with(&left)
}

fn normalize_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn change_blocker(
    code: impl Into<String>,
    work_item_id: Option<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> ChangePreparationBlocker {
    ChangePreparationBlocker {
        code: code.into(),
        work_item_id,
        message: message.into(),
        next_action: next_action.into(),
    }
}

fn invalid_transition(item: &WorkItem, operation: &str) -> ProjectIntentWorkflowError {
    workflow_error(
        "project_intent.work_item_transition_invalid",
        format!(
            "Cannot {operation} WorkItem {} from {:?}.",
            item.work_item_id, item.status
        ),
        "Use a lifecycle operation valid for the current WorkItem state.",
    )
}

fn command_replay_mismatch(command_id: &str) -> ProjectIntentWorkflowError {
    workflow_error(
        "project_intent.command_replay_mismatch",
        format!("Command id {command_id} was already used for another operation."),
        "Reuse a command id only for an exact retry of the original operation.",
    )
}

fn candidate_entry_error(error: crate::ProjectCandidateError) -> ProjectIntentWorkflowError {
    workflow_error(error.code, error.message, error.next_action)
}

fn workflow_error(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> ProjectIntentWorkflowError {
    ProjectIntentWorkflowError::new(code, message, next_action)
}
