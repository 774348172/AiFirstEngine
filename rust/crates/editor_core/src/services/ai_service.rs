use crate::session::ProjectCandidateProposal;
use crate::{
    execute_ui_payload_as_editor_command, CancelSource, CommandResult, CommandStatus,
    CommandTransaction, CredentialOwnerStatus, EditorSession, LlmAttemptDecision,
    LlmLifecycleState, LlmLocalExecutionStatus, LlmRepairSpec, LlmRequestEvent, LlmRequestId,
    LlmRequestSpec, LlmTaskJoinStatus, LlmTerminalStatus, PatchApplier, PatchOperation,
    PatchReviewModel, PatchSource, PatchValidator, ProjectCandidateEntry,
    ProjectCandidatePrepareRequest, ProjectCandidateSourceKind, ProjectCandidateValidationContext,
    ProjectPatchDocument, ProjectPatchImportParseStatus, ProjectPatchImportRequest,
    ProjectPatchImportResult, ProjectPatchImportService, ProjectPatchImportSourceKind,
    ScenePatchOperation, StateChangeSummary, UndoPolicy,
};
use editor_ui_model::{
    AiCommandReviewState, AiPanelMessage, AiPanelMessageRole, AiPanelResponse, AiProposedCommand,
    DiagnosticSeverity, ImportedProjectPatchEvidence, ProjectPatchDiagnosticEvidence,
    ProjectPatchEvidence, UiCommand, UiCommandPayload, UiCommandSource,
};

impl EditorSession {
    pub(crate) fn import_project_patch(
        &mut self,
        transaction: &mut CommandTransaction,
        source_label: String,
        raw_json: Option<String>,
        file_path: Option<String>,
        expected_patch_id: Option<String>,
        dry_run: bool,
    ) -> CommandResult {
        let _ = dry_run;
        self.preview_imported_project_patch(
            transaction,
            source_label,
            raw_json,
            file_path,
            expected_patch_id,
        )
    }

    pub(crate) fn preview_imported_project_patch(
        &mut self,
        transaction: &mut CommandTransaction,
        source_label: String,
        raw_json: Option<String>,
        file_path: Option<String>,
        expected_patch_id: Option<String>,
    ) -> CommandResult {
        transaction
            .read_set
            .push("project_patch.import".to_string());
        transaction.write_set.push("ai_panel.proposals".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let source_file_path = file_path.clone();
        let import_result = if let Some(raw_json) = raw_json {
            let mut request = ProjectPatchImportRequest::json_string(source_label, raw_json);
            request.expected_patch_id = expected_patch_id;
            ProjectPatchImportService::from_json_string(self, request)
        } else if let Some(file_path) = file_path {
            let mut request = ProjectPatchImportRequest::file_path(source_label, file_path);
            request.expected_patch_id = expected_patch_id;
            ProjectPatchImportService::from_file(self, request)
        } else {
            let request = ProjectPatchImportRequest {
                schema_version: crate::PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION.to_string(),
                source_kind: ProjectPatchImportSourceKind::JsonString,
                source_label,
                project_root: None,
                raw_json: None,
                file_path: None,
                expected_patch_id,
                dry_run: true,
            };
            ProjectPatchImportService::from_json_string(self, request)
        };

        self.stage_imported_project_patch_result(
            transaction,
            import_result,
            false,
            source_file_path,
        )
    }

    fn preview_ai_structured_project_patch(
        &mut self,
        transaction: &mut CommandTransaction,
        source_label: String,
        raw_json: String,
        repaired_once: bool,
    ) -> CommandResult {
        transaction
            .read_set
            .push("project_patch.ai_structured_output".to_string());
        transaction.write_set.push("ai_panel.proposals".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let request = ProjectPatchImportRequest::ai_structured_output(source_label, raw_json);
        let import_result = ProjectPatchImportService::from_json_string(self, request);
        self.stage_imported_project_patch_result(transaction, import_result, repaired_once, None)
    }

    fn stage_imported_project_patch_result(
        &mut self,
        transaction: &mut CommandTransaction,
        import_result: ProjectPatchImportResult,
        repaired_once: bool,
        source_file_path: Option<String>,
    ) -> CommandResult {
        push_import_result_diagnostics(self, transaction, &import_result);
        if import_result.parse_status == ProjectPatchImportParseStatus::Rejected {
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }

        let Some(patch) = import_result.parsed_patch.as_ref().cloned() else {
            self.push_error(
                transaction,
                "editor.project_patch_import.patch_missing",
                "ProjectPatch import did not produce a parsed patch.",
                Some("Fix the imported ProjectPatch JSON."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let Some(review) = import_result.review.as_ref() else {
            self.push_error(
                transaction,
                "editor.project_patch_import.review_missing",
                "ProjectPatch import did not produce a review model.",
                Some("Fix ProjectPatch validation diagnostics."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };

        let proposal_id = format!("imported-project-patch-{}", patch.patch_id);
        let project_patch = project_patch_evidence(review, repaired_once);
        let imported_project_patch =
            imported_project_patch_evidence(&import_result, Some(&proposal_id));
        let source_kind = match import_result.source_kind {
            ProjectPatchImportSourceKind::AiStructuredOutput => {
                ProjectCandidateSourceKind::BuiltInProvider
            }
            ProjectPatchImportSourceKind::FilePath => ProjectCandidateSourceKind::ImportedFile,
            ProjectPatchImportSourceKind::TestFixture => ProjectCandidateSourceKind::TestFixture,
            ProjectPatchImportSourceKind::JsonString => ProjectCandidateSourceKind::ImportedCodex,
        };
        let envelope = match ProjectCandidateEntry::project_patch_envelope(
            self,
            proposal_id.clone(),
            source_kind,
            import_result.source_label.clone(),
            patch.clone(),
        ) {
            Ok(envelope) => envelope,
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        let prepare_request = ProjectCandidatePrepareRequest { envelope };
        let prepared = if let Some(source_file_path) = &source_file_path {
            ProjectCandidateEntry::prepare_with_source_file(self, prepare_request, source_file_path)
        } else {
            ProjectCandidateEntry::prepare(self, prepare_request)
        };
        let candidate = match prepared {
            Ok(candidate) => candidate,
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        if let Err(error) = ProjectCandidateEntry::validate(
            self,
            &candidate,
            &ProjectCandidateValidationContext::default(),
        ) {
            self.push_error(
                transaction,
                &error.code,
                error.message,
                Some(&error.next_action),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let project_identity = candidate.project_binding.project_id.clone();
        let source_label = import_result.source_label.clone();
        let capture = match self.capture_project_intent(crate::IntentCaptureInput {
            command_id: format!("import-plan-event-{proposal_id}"),
            project_identity: Some(project_identity.clone()),
            occurred_at: None,
            source_kind: crate::IntentSourceKind::ImportedContext,
            source_identity: source_label.clone(),
            content_ref: None,
            sanitized_summary: format!("Imported change plan: {}", patch.title),
            attachment_refs: Vec::new(),
            related_event_ids: Vec::new(),
            privacy_class: crate::IntentPrivacyClass::Sanitized,
        }) {
            Ok(capture) => capture,
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        let expected_outcome = if patch.expected_outcome.trim().is_empty() {
            patch.title.clone()
        } else {
            patch.expected_outcome.clone()
        };
        if let Err(error) =
            self.dispatch_project_intent(crate::ProjectIntentWorkflowCommand::CreateWorkItem {
                command_id: format!("import-plan-work-item-{proposal_id}"),
                draft: crate::WorkItemDraft {
                    kind: crate::WorkItemKind::Change,
                    title: patch.title.clone(),
                    user_visible_outcome: expected_outcome.clone(),
                    source_event_ids: vec![capture.event_id.clone()],
                    status: crate::WorkItemStatus::Ready,
                    priority: crate::WorkItemPriority::Normal,
                    scope_hints: patch
                        .required_capabilities
                        .iter()
                        .map(|capability| format!("{capability:?}").to_ascii_lowercase())
                        .collect(),
                    constraints: Vec::new(),
                    acceptance_criteria: vec![expected_outcome.clone()],
                    open_questions: Vec::new(),
                    evidence_refs: Vec::new(),
                    relationship_refs: Vec::new(),
                    latest_understanding: format!(
                        "Apply the imported change plan titled {}.",
                        patch.title
                    ),
                    explicitly_deferred: Vec::new(),
                },
            })
        {
            self.push_error(
                transaction,
                &error.code,
                error.message,
                Some(&error.next_action),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let intent_snapshot = match self.project_intent_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        let Some(work_item_id) = intent_snapshot
            .work_items
            .iter()
            .find(|item| item.source_event_ids.contains(&capture.event_id))
            .map(|item| item.work_item_id.clone())
        else {
            self.push_error(
                transaction,
                "project_intent.imported_plan_work_item_missing",
                "Imported change plan did not retain its source-event lineage.",
                Some("Re-import the exact ProjectPatch through the workflow front door."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let change = self.prepare_project_change(crate::ChangePreparationRequest {
            command_id: format!("import-plan-change-set-{proposal_id}"),
            target_kind: crate::ChangeSetTargetKind::ExistingProject,
            target_project_identity: Some(project_identity),
            project_create_spec: None,
            expected_base_project_digest: Some(candidate.project_binding.project_digest.clone()),
            selected_work_item_ids: vec![work_item_id],
            explicit_exclusions: Vec::new(),
            candidate_plan_steps: vec![crate::CandidatePlanStep {
                step_id: format!("project-patch-{}", patch.patch_id),
                depends_on: Vec::new(),
                payload_kind: crate::CandidatePayloadKind::ProjectPatch,
                payload_source_digest: candidate.payload_digest.clone(),
                source_kind,
                source_label: source_label.clone(),
                payload: crate::ProjectCandidatePayload::ProjectPatch(patch.clone()),
                validation_profile: crate::CandidateValidationProfile {
                    controlled_source_patch: None,
                    source_file_path: source_file_path.clone(),
                    expected_source_digest: source_file_path
                        .as_ref()
                        .map(|_| candidate.source_digest.clone()),
                },
                expected_changed_domains: patch
                    .required_capabilities
                    .iter()
                    .map(|capability| format!("{capability:?}").to_ascii_lowercase())
                    .collect(),
                user_visible_outcome: expected_outcome,
                failure_policy: "stop_and_review".to_string(),
            }],
            acceptance_checks: vec!["editor_preview".to_string()],
            estimated_external_waits: Vec::new(),
            external_costs: Vec::new(),
            risks: vec![format!("{:?}", patch.risk_level).to_ascii_lowercase()],
            required_decisions: Vec::new(),
            repair_policy: "deterministic_validation_only".to_string(),
        });
        match change {
            Ok(crate::ChangePreparationResult::Ready(_)) => {}
            Ok(crate::ChangePreparationResult::Blocked(blockers)) => {
                for blocker in blockers {
                    self.push_error(
                        transaction,
                        &blocker.code,
                        blocker.message,
                        Some(&blocker.next_action),
                    );
                }
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        }
        self.project_candidate_proposals
            .retain(|proposal| proposal.proposal_id != proposal_id);
        self.project_candidate_proposals
            .push(ProjectCandidateProposal {
                proposal_id: proposal_id.clone(),
                patch: patch.clone(),
            });
        self.ai_proposed_commands
            .retain(|proposal| proposal.proposal_id != proposal_id);
        self.ai_proposed_commands.push(AiProposedCommand {
            proposal_id: proposal_id.clone(),
            label: format!("Apply imported ProjectPatch: {}", patch.title),
            explanation:
                "Imported ProjectPatch was parsed, validated, and staged for explicit apply."
                    .to_string(),
            command: UiCommandPayload::ApplyImportedProjectPatch {
                proposal_id: proposal_id.clone(),
            },
            project_patch: Some(project_patch),
            imported_project_patch: Some(imported_project_patch),
            review_state: AiCommandReviewState::Proposed,
        });
        transaction.state_changes.push(StateChangeSummary {
            kind: "project_patch_import.preview".to_string(),
            path: format!("ai_panel.proposals.{proposal_id}"),
            before_summary: None,
            after_summary: Some(format!("{:?}", import_result.parse_status)),
        });
        self.push_info(
            transaction,
            "editor.project_patch_import.preview_created",
            format!("Imported ProjectPatch proposal {proposal_id} is ready for review."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn generate_project_patch_from_prompt(
        &mut self,
        transaction: &mut CommandTransaction,
        prompt: String,
    ) -> CommandResult {
        let prompt = prompt.trim().to_string();
        transaction.read_set.push("ai_panel.prompt".to_string());
        transaction
            .read_set
            .push("project_patch.llm_source".to_string());
        transaction
            .write_set
            .push("project_patch.llm_request".to_string());
        transaction.undo_policy = UndoPolicy::None;
        if prompt.is_empty() {
            self.push_error(
                transaction,
                "llm_patch_source.prompt_required",
                "Enter a ProjectPatch request before submitting.",
                Some("Describe the project change in the AI Panel prompt field."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        if prompt.len() > 16 * 1024 {
            self.push_error(
                transaction,
                "llm_patch_source.prompt_too_large",
                "The AI Panel prompt exceeds the 16 KiB editor limit.",
                Some("Reduce the request to the relevant project change."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let capture = match self.capture_ai_prompt_intent(transaction, &prompt) {
            Ok(capture) => capture,
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        if self.active_project_session.is_none() {
            self.ai_prompt_counter = self.ai_prompt_counter.saturating_add(1);
            self.ai_panel_messages.push(AiPanelMessage {
                message_id: format!("intent-user-{}", self.ai_prompt_counter),
                role: AiPanelMessageRole::User,
                text: prompt,
            });
            self.ai_panel_messages.push(AiPanelMessage {
                message_id: format!("intent-assistant-{}", self.ai_prompt_counter),
                role: AiPanelMessageRole::Assistant,
                text: "Saved to the project draft.".to_string(),
            });
            self.ai_prompt_draft.clear();
            self.push_info(
                transaction,
                "project_intent.pre_project_captured",
                format!(
                    "Captured {} in the local Create with AI draft.",
                    capture.event_id
                ),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        if self.llm_request_controller.is_busy() || self.active_llm_patch_request.is_some() {
            self.push_error(
                transaction,
                "llm_patch_source.request_busy",
                "An LLM ProjectPatch request is already active.",
                Some("Cancel or wait for the current request before submitting another."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }

        let context = crate::ProjectPatchLlmContextSnapshot::capture(self);
        let context_json = context.prompt_json();
        let had_override = self.llm_patch_source_override.is_some();
        let mut config = self
            .llm_patch_source_override
            .take()
            .unwrap_or_else(crate::LlmPatchSourceConfig::openai_compatible_from_env);
        if !had_override && !config.enabled {
            config = crate::LlmPatchSourceConfig::deterministic_mock();
        }
        self.llm_patch_request_generation = self.llm_patch_request_generation.saturating_add(1);
        let generation = self.llm_patch_request_generation;
        let request_id = LlmRequestId::new(format!("llm-patch-request-{generation}"));
        let expected_post_start_revision = self.revision.saturating_add(1);
        let maximum_candidate_bytes = config.maximum_candidate_bytes;
        self.last_llm_patch_report =
            (self.llm_patch_report_level != crate::LlmPatchReportLevel::Off).then(|| {
                crate::LlmPatchRequestReport::started(
                    self.llm_patch_report_level,
                    request_id.to_string(),
                    config.provider_id.clone(),
                    config.model.clone(),
                    config.structured_output_mode,
                    context.context_hash.clone(),
                    context.project_patch_schema_hash.clone(),
                )
            });
        if let Some(report) = &mut self.last_llm_patch_report {
            report.lifecycle_state = LlmLifecycleState::RunningGenerate;
            report.local_execution_status = LlmLocalExecutionStatus::Running;
        }
        if let Err(diagnostic) = self.llm_request_controller.start(LlmRequestSpec {
            request_id: request_id.clone(),
            prompt: prompt.clone(),
            context_json,
            config,
        }) {
            self.push_error(
                transaction,
                &diagnostic.code,
                diagnostic.message,
                Some("Wait for the current request to join before submitting another."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        self.active_llm_patch_request = Some(crate::session::ActiveLlmPatchRequest {
            request_id: request_id.clone(),
            expected_post_start_revision,
            context_hash: context.context_hash,
            generation,
            attempt_index: 0,
            maximum_candidate_bytes,
            initial_candidate: None,
            initial_import: None,
        });
        self.ai_panel_stage = editor_ui_model::AiPanelStage::Generating;
        self.ai_panel_status_summary = Some(format!("Generating ProjectPatch ({request_id})"));
        self.ai_prompt_draft.clear();
        self.ai_prompt_counter = self.ai_prompt_counter.saturating_add(1);
        self.ai_panel_messages.push(AiPanelMessage {
            message_id: format!("llm-patch-user-{}", self.ai_prompt_counter),
            role: AiPanelMessageRole::User,
            text: prompt,
        });
        self.push_info(
            transaction,
            "llm_patch_source.request_started",
            format!("Started owned ProjectPatch request {request_id}."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_ai_prompt_draft(
        &mut self,
        transaction: &mut CommandTransaction,
        prompt: String,
    ) -> CommandResult {
        transaction
            .write_set
            .push("ai_panel.prompt_draft".to_string());
        transaction.undo_policy = UndoPolicy::None;
        if prompt.len() > 16 * 1024 {
            self.push_error(
                transaction,
                "llm_patch_source.prompt_too_large",
                "The AI Panel prompt exceeds the 16 KiB editor limit.",
                Some("Reduce the request to the relevant project change."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        self.ai_prompt_draft = prompt;
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn cancel_llm_patch_request(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction
            .write_set
            .push("project_patch.llm_request".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let Some(active) = self.active_llm_patch_request.as_ref() else {
            self.push_error(
                transaction,
                "llm_patch_source.no_active_request",
                "There is no active LLM ProjectPatch request to cancel.",
                Some("Submit a prompt before using Cancel."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let request_id = active.request_id.clone();
        let receipt = self
            .llm_request_controller
            .cancel(&request_id, CancelSource::User);
        if !receipt.accepted {
            self.push_error(
                transaction,
                "llm_request_controller.request_not_found",
                "The LLM request is no longer cancellable.",
                Some("Wait for the current lifecycle event and retry if needed."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        self.ai_panel_stage = editor_ui_model::AiPanelStage::Cancelling;
        self.ai_panel_status_summary = Some(format!("Cancelling {request_id}"));
        if let Some(report) = &mut self.last_llm_patch_report {
            report.final_status = "cancelling".to_string();
            report.lifecycle_state = LlmLifecycleState::Cancelling;
            report.cancel_requested = true;
            report.cancel_source = CancelSource::User;
            report.transport_abort_requested = receipt.transport_abort_requested;
            report.remote_execution_status = receipt.remote_execution_status;
        }
        self.push_info(
            transaction,
            "llm_request_controller.cancel_requested",
            format!("Requested cancellation for ProjectPatch request {request_id}."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub fn set_llm_patch_source_config_for_test(&mut self, config: crate::LlmPatchSourceConfig) {
        self.llm_patch_source_override = Some(config);
    }

    pub fn set_llm_patch_report_level(&mut self, level: crate::LlmPatchReportLevel) {
        self.llm_patch_report_level = level;
        if level == crate::LlmPatchReportLevel::Off {
            self.last_llm_patch_report = None;
        }
    }

    pub fn llm_patch_report_level(&self) -> crate::LlmPatchReportLevel {
        self.llm_patch_report_level
    }

    pub fn last_llm_patch_report(&self) -> Option<&crate::LlmPatchRequestReport> {
        self.last_llm_patch_report.as_ref()
    }

    pub fn has_active_llm_patch_request(&self) -> bool {
        self.llm_request_controller.is_busy()
    }

    pub fn pump_llm_patch_request(&mut self) -> bool {
        let Some(event) = self.llm_request_controller.poll().into_iter().next() else {
            return false;
        };
        match event {
            LlmRequestEvent::CancelledJoined { receipt } => {
                let generation = self
                    .active_llm_patch_request
                    .take()
                    .map(|active| active.generation)
                    .unwrap_or_default();
                self.ai_panel_stage = editor_ui_model::AiPanelStage::Cancelled;
                self.ai_panel_status_summary = Some(format!("Cancelled {}", receipt.request_id));
                if let Some(report) = &mut self.last_llm_patch_report {
                    report.final_status = "cancelled".to_string();
                    report.cancelled = true;
                    report.lifecycle_state = LlmLifecycleState::CancelledJoined;
                    report.terminal_status = Some(LlmTerminalStatus::Cancelled);
                    report.cancel_requested = true;
                    report.cancel_source = receipt.cancel_source;
                    report.transport_abort_requested = receipt.transport_abort_requested;
                    report.transport_abort_observed = receipt.transport_abort_observed;
                    report.task_join_status = receipt.task_join_status;
                    report.credential_owner_status = receipt.credential_owner_status;
                    report.local_execution_status = receipt.local_execution_status;
                    report.remote_execution_status = receipt.remote_execution_status;
                    report.cancel_latency_ms = receipt.cancel_latency_ms;
                }
                self.ai_panel_messages.push(AiPanelMessage {
                    message_id: format!("llm-patch-cancelled-{generation}"),
                    role: AiPanelMessageRole::System,
                    text: "ProjectPatch generation cancelled locally after task join.".to_string(),
                });
                return true;
            }
            LlmRequestEvent::FailedJoined {
                request_id,
                diagnostic,
                task_join_status,
            } => {
                self.active_llm_patch_request.take();
                self.ai_panel_stage = editor_ui_model::AiPanelStage::Failed;
                self.ai_panel_status_summary = Some(diagnostic.message.clone());
                if let Some(report) = &mut self.last_llm_patch_report {
                    report.final_status = "task_failed".to_string();
                    report.lifecycle_state = LlmLifecycleState::FailedJoined;
                    report.terminal_status = Some(LlmTerminalStatus::Failed);
                    report.task_join_status = task_join_status;
                    report.credential_owner_status = CredentialOwnerStatus::Released;
                    report.local_execution_status = LlmLocalExecutionStatus::Stopped;
                    report.diagnostic_codes.push(diagnostic.code.clone());
                }
                let command = UiCommand {
                    command_id: "complete_llm_patch_request".to_string(),
                    source: UiCommandSource::AiAssistant,
                    request_id: request_id.to_string(),
                    payload: UiCommandPayload::GenerateProjectPatchFromPrompt {
                        prompt: String::new(),
                    },
                };
                let mut transaction = self.begin_transaction(command);
                self.push_error(
                    &mut transaction,
                    &diagnostic.code,
                    diagnostic.message,
                    Some("Retry the request and inspect the editor provider configuration."),
                );
                let _ = self.finish_transaction(transaction, CommandStatus::Rejected);
                return true;
            }
            LlmRequestEvent::AttemptJoined {
                request_id,
                attempt_index,
                result,
            } => self.handle_joined_llm_attempt(request_id, attempt_index, result),
        }
    }

    fn handle_joined_llm_attempt(
        &mut self,
        request_id: LlmRequestId,
        attempt_index: u8,
        source_result: crate::LlmPatchSourceResult,
    ) -> bool {
        let mut active = self
            .active_llm_patch_request
            .take()
            .expect("active LLM request must exist while pumping");
        if active.generation != self.llm_patch_request_generation || active.request_id != request_id
        {
            let _ = self.llm_request_controller.resolve_attempt(
                &request_id,
                LlmAttemptDecision::Fail {
                    diagnostic_summary: "stale request generation".to_string(),
                },
            );
            return true;
        }
        active.attempt_index = attempt_index;

        let command = UiCommand {
            command_id: "complete_llm_patch_request".to_string(),
            source: UiCommandSource::AiAssistant,
            request_id: active.request_id.to_string(),
            payload: UiCommandPayload::GenerateProjectPatchFromPrompt {
                prompt: String::new(),
            },
        };
        let mut transaction = self.begin_transaction(command);
        transaction
            .write_set
            .push("project_patch.llm_request".to_string());
        if self.revision != active.expected_post_start_revision {
            let current = crate::ProjectPatchLlmContextSnapshot::capture(self);
            if current.context_hash != active.context_hash {
                self.ai_panel_stage = editor_ui_model::AiPanelStage::Failed;
                self.ai_panel_status_summary = Some("Project context changed".to_string());
                if let Some(report) = &mut self.last_llm_patch_report {
                    report.final_status = "context_stale".to_string();
                    report.context_stale = true;
                    report
                        .diagnostic_codes
                        .push("llm_patch_source.context_stale".to_string());
                    report.lifecycle_state = LlmLifecycleState::FailedJoined;
                    report.terminal_status = Some(LlmTerminalStatus::Failed);
                    report.task_join_status = LlmTaskJoinStatus::Joined;
                    report.credential_owner_status = CredentialOwnerStatus::Released;
                }
                let _ = self.llm_request_controller.resolve_attempt(
                    &request_id,
                    LlmAttemptDecision::Fail {
                        diagnostic_summary: "context stale".to_string(),
                    },
                );
                self.push_error(
                    &mut transaction,
                    "llm_patch_source.context_stale",
                    "The project context changed while the LLM request was running.",
                    Some("Review the current project state and submit the request again."),
                );
                let _ = self.finish_transaction(transaction, CommandStatus::Rejected);
                return true;
            }
        }

        if let Some(report) = &mut self.last_llm_patch_report {
            report.attempts.push(crate::LlmPatchAttemptSummary {
                attempt_kind: if active.attempt_index == 0 {
                    "generate".to_string()
                } else {
                    "repair".to_string()
                },
                attempt_index: active.attempt_index,
                status: source_result.status,
                latency_ms: source_result.latency_ms,
                http_status_class: (report.report_level == crate::LlmPatchReportLevel::Trace)
                    .then(|| source_result.http_status_class.clone())
                    .flatten(),
                transport_attempt_count: source_result.transport_attempt_count,
            });
            report.lifecycle_state = LlmLifecycleState::WaitingForMainThreadDecision;
            report.task_join_status = LlmTaskJoinStatus::Joined;
        }
        if let Some(raw_json) = source_result.raw_json {
            let source_label = format!("{}:{}", source_result.provider_id, source_result.model);
            let import_request = ProjectPatchImportRequest::ai_structured_output(
                source_label.clone(),
                raw_json.clone(),
            );
            let import = ProjectPatchImportService::from_json_string(self, import_request);
            if !crate::project_patch_import_accepted(&import) {
                if active.attempt_index == 0
                    && crate::repair_decision(&import) == crate::RepairDecision::Eligible
                {
                    let repair_diagnostic_codes = crate::import_diagnostics(&import)
                        .into_iter()
                        .map(|diagnostic| diagnostic.code)
                        .collect::<Vec<_>>();
                    active.attempt_index = 1;
                    active.initial_candidate = Some(raw_json);
                    active.initial_import = Some(import.clone());
                    active.expected_post_start_revision = self.revision;
                    if let Err(diagnostic) = self.llm_request_controller.resolve_attempt(
                        &request_id,
                        LlmAttemptDecision::ContinueRepair {
                            repair_spec: LlmRepairSpec {
                                candidate_json: active
                                    .initial_candidate
                                    .clone()
                                    .expect("repair candidate must be retained"),
                                import,
                                maximum_candidate_bytes: active.maximum_candidate_bytes,
                            },
                        },
                    ) {
                        self.ai_panel_stage = editor_ui_model::AiPanelStage::Failed;
                        self.push_error(
                            &mut transaction,
                            &diagnostic.code,
                            diagnostic.message,
                            Some("Retry the request after the controller returns to idle."),
                        );
                        let _ = self.finish_transaction(transaction, CommandStatus::Rejected);
                        return true;
                    }
                    self.ai_panel_stage = editor_ui_model::AiPanelStage::Repairing;
                    self.ai_panel_status_summary =
                        Some(format!("Repairing ProjectPatch ({})", active.request_id));
                    self.active_llm_patch_request = Some(active);
                    if let Some(report) = &mut self.last_llm_patch_report {
                        report.final_status = "repairing".to_string();
                        report.lifecycle_state = LlmLifecycleState::RunningRepair;
                        report.local_execution_status = LlmLocalExecutionStatus::Running;
                        report.repair_attempt_count = 1;
                        report.diagnostic_codes = repair_diagnostic_codes;
                    }
                    return true;
                }

                self.ai_panel_stage = editor_ui_model::AiPanelStage::Failed;
                if active.attempt_index == 1 {
                    let initial_fingerprint = active
                        .initial_import
                        .as_ref()
                        .map(crate::diagnostic_fingerprint);
                    let repaired_fingerprint = crate::diagnostic_fingerprint(&import);
                    let (code, message) =
                        if initial_fingerprint.as_deref() == Some(repaired_fingerprint.as_str()) {
                            (
                                "llm_patch_repair.no_progress",
                                "The one-shot repair returned the same diagnostic fingerprint.",
                            )
                        } else {
                            (
                            "llm_patch_repair.attempt_limit_reached",
                            "The one-shot repair candidate still failed ProjectPatch validation.",
                        )
                        };
                    self.push_error(
                        &mut transaction,
                        code,
                        message,
                        Some("Review the diagnostics and submit a narrower request."),
                    );
                    if let Some(report) = &mut self.last_llm_patch_report {
                        report.final_status =
                            code.trim_start_matches("llm_patch_repair.").to_string();
                        report.diagnostic_codes = crate::import_diagnostics(&import)
                            .into_iter()
                            .map(|diagnostic| diagnostic.code)
                            .collect();
                    }
                }
                self.ai_panel_status_summary =
                    Some("ProjectPatch candidate failed validation".to_string());
                let _ = self.llm_request_controller.resolve_attempt(
                    &request_id,
                    LlmAttemptDecision::Fail {
                        diagnostic_summary: "ProjectPatch candidate failed validation".to_string(),
                    },
                );
                if let Some(report) = &mut self.last_llm_patch_report {
                    report.lifecycle_state = LlmLifecycleState::FailedJoined;
                    report.terminal_status = Some(LlmTerminalStatus::Failed);
                    report.credential_owner_status = CredentialOwnerStatus::Released;
                    report.local_execution_status = LlmLocalExecutionStatus::Stopped;
                }
                let _ = self.preview_ai_structured_project_patch(
                    &mut transaction,
                    source_label,
                    raw_json,
                    active.attempt_index == 1,
                );
                return true;
            }

            if active.attempt_index == 1 {
                let repaired = import
                    .parsed_patch
                    .as_ref()
                    .expect("accepted ProjectPatch import must contain parsed patch");
                let initial_import = active
                    .initial_import
                    .as_ref()
                    .expect("repair attempt must retain the initial import");
                let scope_validation = crate::validate_repair_scope(
                    initial_import,
                    repaired,
                    crate::RepairScopePolicy::new(PatchValidator::MAX_OPERATION_COUNT),
                );
                if let Some(report) = &mut self.last_llm_patch_report {
                    report.repair_scope = Some((&scope_validation).into());
                }
                if !scope_validation.accepted() {
                    let reason = scope_validation
                        .rejection_code
                        .as_deref()
                        .unwrap_or("repair_scope_rejected");
                    self.ai_panel_stage = editor_ui_model::AiPanelStage::Failed;
                    self.ai_panel_status_summary =
                        Some("Repair violated the ProjectPatch scope contract".to_string());
                    self.push_error(
                        &mut transaction,
                        "llm_patch_repair.scope_rejected",
                        format!("The repair candidate violated the scope contract: {reason}."),
                        Some("Submit a narrower request without expanding risk or domains."),
                    );
                    if let Some(report) = &mut self.last_llm_patch_report {
                        report.final_status = "scope_rejected".to_string();
                        report.diagnostic_codes.push(reason.to_string());
                    }
                    let _ = self.llm_request_controller.resolve_attempt(
                        &request_id,
                        LlmAttemptDecision::Fail {
                            diagnostic_summary: "repair scope rejected".to_string(),
                        },
                    );
                    if let Some(report) = &mut self.last_llm_patch_report {
                        report.lifecycle_state = LlmLifecycleState::FailedJoined;
                        report.terminal_status = Some(LlmTerminalStatus::Failed);
                        report.credential_owner_status = CredentialOwnerStatus::Released;
                        report.local_execution_status = LlmLocalExecutionStatus::Stopped;
                    }
                    let _ = self.finish_transaction(transaction, CommandStatus::Rejected);
                    return true;
                }
            }

            self.ai_panel_stage = editor_ui_model::AiPanelStage::Reviewing;
            self.ai_panel_status_summary = Some(format!(
                "Candidate ready from {}:{}{}",
                source_result.provider_id,
                source_result.model,
                if active.attempt_index == 1 {
                    " (repaired once)"
                } else {
                    ""
                }
            ));
            self.ai_panel_messages.push(AiPanelMessage {
                message_id: format!("llm-patch-assistant-{}", self.ai_prompt_counter),
                role: AiPanelMessageRole::Assistant,
                text: "Generated a ProjectPatch JSON proposal for review.".to_string(),
            });
            if let Some(report) = &mut self.last_llm_patch_report {
                report.final_status = "reviewing".to_string();
                report.repair_attempt_count = active.attempt_index;
                report.candidate_hash = Some(engine_runtime::canonical_digest::sha256_prefixed(
                    raw_json.as_bytes(),
                ));
                report.diagnostic_codes.clear();
                report.lifecycle_state = LlmLifecycleState::CompletedJoined;
                report.terminal_status = Some(LlmTerminalStatus::Completed);
                report.task_join_status = LlmTaskJoinStatus::Joined;
                report.credential_owner_status = CredentialOwnerStatus::Released;
                report.local_execution_status = LlmLocalExecutionStatus::Completed;
            }
            let _ = self
                .llm_request_controller
                .resolve_attempt(&request_id, LlmAttemptDecision::Complete);
            let _ = self.preview_ai_structured_project_patch(
                &mut transaction,
                source_label,
                raw_json,
                active.attempt_index == 1,
            );
            return true;
        }

        self.ai_panel_stage = editor_ui_model::AiPanelStage::Failed;
        let code = source_result
            .error_code
            .unwrap_or_else(|| "llm_patch_source.failed".to_string());
        let message = source_result
            .error_message
            .unwrap_or_else(|| "LLM patch source did not return ProjectPatch JSON.".to_string());
        self.ai_panel_status_summary = Some(message.clone());
        if let Some(report) = &mut self.last_llm_patch_report {
            report.final_status = format!("{:?}", source_result.status).to_lowercase();
            report.diagnostic_codes = vec![code.clone()];
            report.lifecycle_state = LlmLifecycleState::FailedJoined;
            report.terminal_status = Some(LlmTerminalStatus::Failed);
            report.task_join_status = LlmTaskJoinStatus::Joined;
            report.credential_owner_status = CredentialOwnerStatus::Released;
            report.local_execution_status = LlmLocalExecutionStatus::Stopped;
        }
        let _ = self.llm_request_controller.resolve_attempt(
            &request_id,
            LlmAttemptDecision::Fail {
                diagnostic_summary: code.clone(),
            },
        );
        self.ai_panel_messages.push(AiPanelMessage {
            message_id: format!("llm-patch-assistant-{}", self.ai_prompt_counter),
            role: AiPanelMessageRole::Assistant,
            text: message.clone(),
        });
        let next_action = source_result
            .next_action
            .as_deref()
            .unwrap_or("Check the editor provider configuration and retry.");
        self.push_error(&mut transaction, &code, message, Some(next_action));
        let _ = self.finish_transaction(transaction, CommandStatus::Rejected);
        true
    }

    pub fn shutdown_llm(&mut self, deadline: std::time::Duration) -> crate::LlmShutdownReceipt {
        let receipt = self.llm_request_controller.shutdown(deadline);
        self.active_llm_patch_request.take();
        if let Some(report) = &mut self.last_llm_patch_report {
            report.shutdown_latency_ms = Some(receipt.shutdown_latency_ms);
            report.task_join_status = receipt.task_join_status;
            report.lifecycle_state = receipt.state;
            report.credential_owner_status = CredentialOwnerStatus::Released;
            report.local_execution_status = LlmLocalExecutionStatus::Stopped;
            if receipt.state == LlmLifecycleState::ShutdownJoinTimedOut {
                report.terminal_status = Some(LlmTerminalStatus::ShutdownJoinTimedOut);
                report.final_status = "shutdown_join_timed_out".to_string();
            } else if report.terminal_status.is_none() {
                report.terminal_status = Some(LlmTerminalStatus::Cancelled);
                report.final_status = "cancelled".to_string();
                report.cancelled = true;
                report.cancel_requested = true;
                report.cancel_source = CancelSource::SessionShutdown;
            }
        }
        self.last_llm_shutdown_receipt = Some(receipt.clone());
        receipt
    }

    pub fn last_llm_shutdown_receipt(&self) -> Option<&crate::LlmShutdownReceipt> {
        self.last_llm_shutdown_receipt.as_ref()
    }

    pub(crate) fn apply_imported_project_patch(
        &mut self,
        transaction: &mut CommandTransaction,
        proposal_id: &str,
    ) -> CommandResult {
        transaction.read_set.push("ai_panel.proposals".to_string());
        transaction
            .read_set
            .push("project_intent_workflow.proposal".to_string());
        transaction
            .write_set
            .push("project_intent_workflow.run".to_string());
        transaction.write_set.push("ai_panel.proposals".to_string());
        let Some(proposal) = self
            .project_candidate_proposals
            .iter()
            .find(|proposal| proposal.proposal_id == proposal_id)
            .cloned()
        else {
            self.push_error(
                transaction,
                "editor.project_patch_import.proposal_missing",
                format!("Imported ProjectPatch proposal {proposal_id} does not exist."),
                Some("Preview or import a ProjectPatch first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };

        if let Some(model) = self
            .ai_proposed_commands
            .iter_mut()
            .find(|model| model.proposal_id == proposal_id)
        {
            model.review_state = AiCommandReviewState::Accepted;
        }
        let snapshot = match self.project_intent_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        let Some(change_set) = snapshot.active_proposal else {
            self.push_error(
                transaction,
                "project_intent.proposal_missing",
                "Imported ProjectPatch has no active ChangeSetProposal.",
                Some("Preview or import the ProjectPatch again."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        if !change_set.candidate_plan_steps.iter().any(|step| {
            matches!(&step.payload, crate::ProjectCandidatePayload::ProjectPatch(patch) if patch.patch_id == proposal.patch.patch_id)
        }) {
            self.push_error(
                transaction,
                "project_intent.imported_plan_mismatch",
                "The active ChangeSet does not contain the reviewed imported ProjectPatch.",
                Some("Review and approve the current ChangeSet instead."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let Some(target_identity) = change_set.target_project_identity.clone() else {
            self.push_error(
                transaction,
                "project_intent.approval_target_missing",
                "Imported ProjectPatch ChangeSet has no target identity.",
                Some("Re-import against the active project."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let approval = crate::ChangeSetApprovalInput {
            command_id: format!("legacy-import-approve-{}", transaction.transaction_id),
            approval_id: format!("legacy-import-approval-{}", transaction.transaction_id),
            approved_by: "editor-user".to_string(),
            proposal_digest: change_set.proposal_digest.clone(),
            target_identity,
            expected_base_project_digest: change_set.expected_base_project_digest.clone(),
            approved_risk_classes: change_set.risks.clone(),
            approved_external_costs: change_set.external_costs.clone(),
            approved_repair_policy: change_set.repair_policy.clone(),
            approved_at: None,
        };
        let mut run = match self.authorize_project_change(approval) {
            Ok(run) => run,
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
        };
        for index in 0..=change_set.candidate_plan_steps.len() {
            if matches!(
                run.state,
                crate::ProjectProductionRunState::Completed
                    | crate::ProjectProductionRunState::Previewing
                    | crate::ProjectProductionRunState::Failed
                    | crate::ProjectProductionRunState::Stale
                    | crate::ProjectProductionRunState::Cancelled
            ) {
                break;
            }
            let command = crate::ProjectIntentWorkflowCommand::AdvanceRun {
                command_id: format!(
                    "legacy-import-advance-{}-{index}",
                    transaction.transaction_id
                ),
                run_id: run.run_id.clone(),
            };
            if let Err(error) = self.dispatch_project_intent(command) {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
            run = self
                .project_intent_snapshot()
                .ok()
                .and_then(|snapshot| snapshot.active_run)
                .unwrap_or(run);
        }
        if run.state == crate::ProjectProductionRunState::Completed {
            if let Some(model) = self
                .ai_proposed_commands
                .iter_mut()
                .find(|model| model.proposal_id == proposal_id)
            {
                model.review_state = AiCommandReviewState::Executed;
                if let Some(evidence) = &mut model.imported_project_patch {
                    evidence.review_state = "Executed".to_string();
                }
            }
            self.push_info(
                transaction,
                "project_intent.imported_change_completed",
                format!(
                    "Imported ProjectPatch completed through production run {}.",
                    run.run_id
                ),
            );
            transaction.state_changes.push(StateChangeSummary {
                kind: "project_intent.production_run".to_string(),
                path: format!("project_intent.runs.{}", run.run_id),
                before_summary: Some("approved".to_string()),
                after_summary: Some("completed".to_string()),
            });
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        if run.state == crate::ProjectProductionRunState::Previewing {
            self.push_info(
                transaction,
                "project_intent.imported_change_awaiting_preview",
                format!(
                    "Imported ProjectPatch was applied by production run {}; exact presented-frame verification remains pending.",
                    run.run_id
                ),
            );
            transaction.state_changes.push(StateChangeSummary {
                kind: "project_intent.production_run".to_string(),
                path: format!("project_intent.runs.{}", run.run_id),
                before_summary: Some("approved".to_string()),
                after_summary: Some("previewing".to_string()),
            });
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        for diagnostic in &run.diagnostics {
            self.push_error(
                transaction,
                diagnostic,
                "Project production stopped on a bound validation failure.",
                run.decision_requests.first().map(String::as_str),
            );
        }
        self.push_error(
            transaction,
            "project_intent.production_not_completed",
            format!("Production run stopped in state {:?}.", run.state),
            run.decision_requests.first().map(String::as_str).or(Some(
                "Inspect the Intent panel and recover or reprepare the ChangeSet.",
            )),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Failed)
    }

    pub(crate) fn submit_ai_prompt(
        &mut self,
        transaction: &mut CommandTransaction,
        prompt: String,
    ) -> CommandResult {
        transaction.read_set.push("editor_ui_model".to_string());
        transaction.write_set.push("ai_panel".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            self.push_error(
                transaction,
                "project_intent.prompt_required",
                "Enter a project idea, request, question, or problem before submitting.",
                Some("Your wording may be incomplete or uncertain."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        if let Err(error) = self.capture_ai_prompt_intent(transaction, &prompt) {
            self.push_error(
                transaction,
                &error.code,
                error.message,
                Some(&error.next_action),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        self.ai_prompt_counter = self.ai_prompt_counter.saturating_add(1);
        self.ai_panel_messages.push(AiPanelMessage {
            message_id: format!("ai-user-{}", self.ai_prompt_counter),
            role: AiPanelMessageRole::User,
            text: prompt.clone(),
        });
        let (response, project_patch_proposals) = self.plan_ai_response(&prompt, transaction);
        self.ai_panel_messages.push(AiPanelMessage {
            message_id: format!("ai-assistant-{}", self.ai_prompt_counter),
            role: AiPanelMessageRole::Assistant,
            text: response.explanation.clone(),
        });
        self.project_candidate_proposals = project_patch_proposals;
        self.ai_proposed_commands = response.proposed_commands;
        transaction.diagnostics.extend(response.diagnostics);
        self.push_info(
            transaction,
            "editor.ai_panel.plan_created",
            format!(
                "AI Panel produced {} proposed command(s).",
                self.ai_proposed_commands.len()
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    fn capture_ai_prompt_intent(
        &mut self,
        transaction: &mut CommandTransaction,
        prompt: &str,
    ) -> Result<crate::IntentCaptureReceipt, crate::ProjectIntentWorkflowError> {
        transaction
            .write_set
            .push("project_intent_workflow.journal".to_string());
        let project_identity = self
            .active_project_session
            .as_ref()
            .map(|project| project.manifest.project_id.clone());
        let receipt = self.capture_project_intent(crate::IntentCaptureInput {
            command_id: format!("intent-prompt-{}", transaction.transaction_id),
            project_identity,
            occurred_at: None,
            source_kind: crate::IntentSourceKind::UserMessage,
            source_identity: transaction.source.clone(),
            content_ref: None,
            sanitized_summary: prompt.to_string(),
            attachment_refs: Vec::new(),
            related_event_ids: Vec::new(),
            privacy_class: crate::IntentPrivacyClass::LocalOnly,
        })?;
        let kind = if prompt.to_ascii_lowercase().contains("bug")
            || prompt.contains("错误")
            || prompt.contains("不对")
        {
            crate::WorkItemKind::Bug
        } else if prompt.contains('?') || prompt.contains('？') {
            crate::WorkItemKind::Question
        } else {
            crate::WorkItemKind::Change
        };
        let status = match kind {
            crate::WorkItemKind::Bug => crate::WorkItemStatus::NeedsEvidence,
            crate::WorkItemKind::Question => crate::WorkItemStatus::NeedsClarification,
            _ => crate::WorkItemStatus::Triaging,
        };
        self.dispatch_project_intent(crate::ProjectIntentWorkflowCommand::CreateWorkItem {
            command_id: format!("normalize-prompt-{}", transaction.transaction_id),
            draft: crate::WorkItemDraft {
                kind,
                title: prompt.chars().take(72).collect(),
                user_visible_outcome: prompt.to_string(),
                source_event_ids: vec![receipt.event_id.clone()],
                status,
                priority: crate::WorkItemPriority::Normal,
                scope_hints: Vec::new(),
                constraints: Vec::new(),
                acceptance_criteria: Vec::new(),
                open_questions: (status == crate::WorkItemStatus::NeedsClarification)
                    .then(|| "Clarify the intended outcome when convenient.".to_string())
                    .into_iter()
                    .collect(),
                evidence_refs: Vec::new(),
                relationship_refs: Vec::new(),
                latest_understanding: prompt.to_string(),
                explicitly_deferred: Vec::new(),
            },
        })?;
        Ok(receipt)
    }

    pub(crate) fn accept_ai_proposed_command(
        &mut self,
        transaction: &mut CommandTransaction,
        proposal_id: &str,
    ) -> CommandResult {
        transaction.read_set.push("ai_panel.proposals".to_string());
        transaction.write_set.push("ai_panel.proposals".to_string());
        let Some(index) = self
            .ai_proposed_commands
            .iter()
            .position(|proposal| proposal.proposal_id == proposal_id)
        else {
            self.push_error(
                transaction,
                "editor.ai_panel.proposal_missing",
                format!("AI proposal {proposal_id} does not exist."),
                Some("Submit a new AI request or select an existing proposal."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        let payload = self.ai_proposed_commands[index].command.clone();
        self.ai_proposed_commands[index].review_state = AiCommandReviewState::Accepted;
        let patch = self
            .project_candidate_proposals
            .iter()
            .find(|proposal| proposal.proposal_id == proposal_id)
            .map(|proposal| proposal.patch.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "ai_panel.proposal.accepted".to_string(),
            path: format!("ai_panel.proposals.{proposal_id}"),
            before_summary: Some("Proposed".to_string()),
            after_summary: Some("Accepted".to_string()),
        });
        let accept_result = self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        let mut execution_result = if let Some(patch) = patch {
            let report = self.execute_patch_as_transaction(patch);
            let status = match report.status {
                crate::PatchApplyStatus::Committed => CommandStatus::Committed,
                crate::PatchApplyStatus::Rejected => CommandStatus::Rejected,
                crate::PatchApplyStatus::Failed | crate::PatchApplyStatus::Reverted => {
                    CommandStatus::Failed
                }
            };
            CommandResult {
                transaction_id: format!("patch-{}", report.patch_id),
                request_id: format!("request-ai-proposal-{proposal_id}"),
                command_id: "ai_accept_project_patch".to_string(),
                status,
                diagnostics: Vec::new(),
                console_entries: Vec::new(),
                state_changes: vec![StateChangeSummary {
                    kind: "project_patch.apply".to_string(),
                    path: format!("project_patch.{}", report.patch_id),
                    before_summary: Some("validated".to_string()),
                    after_summary: Some(format!("{:?}", report.status)),
                }],
                ui_model_revision: self.revision,
            }
        } else {
            execute_ui_payload_as_editor_command(
                self,
                editor_ui_model::UiCommandSource::AiAssistant,
                format!("request-ai-proposal-{proposal_id}"),
                payload,
            )
        };
        execution_result
            .diagnostics
            .extend(accept_result.diagnostics);
        execution_result
            .console_entries
            .extend(accept_result.console_entries);
        execution_result
            .state_changes
            .extend(accept_result.state_changes);
        execution_result
    }

    pub(crate) fn reject_ai_proposed_command(
        &mut self,
        transaction: &mut CommandTransaction,
        proposal_id: &str,
    ) -> CommandResult {
        transaction.read_set.push("ai_panel.proposals".to_string());
        transaction.write_set.push("ai_panel.proposals".to_string());
        let Some(proposal) = self
            .ai_proposed_commands
            .iter_mut()
            .find(|proposal| proposal.proposal_id == proposal_id)
        else {
            self.push_error(
                transaction,
                "editor.ai_panel.proposal_missing",
                format!("AI proposal {proposal_id} does not exist."),
                Some("Submit a new AI request or select an existing proposal."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        };
        proposal.review_state = AiCommandReviewState::Rejected;
        self.push_info(
            transaction,
            "editor.ai_panel.proposal_rejected",
            format!("Rejected AI proposal {proposal_id}."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn plan_ai_response(
        &self,
        prompt: &str,
        transaction: &CommandTransaction,
    ) -> (AiPanelResponse, Vec<ProjectCandidateProposal>) {
        let normalized = prompt.to_ascii_lowercase();
        let selected = self.scene_selection.primary_entity_id.clone();
        let mut proposed_commands = Vec::new();
        let mut project_patch_proposals = Vec::new();

        if normalized.contains("create") || prompt.contains("创建") || prompt.contains("新建") {
            let patch = ProjectPatchDocument::new(
                format!("ai-patch-{}-create", self.ai_prompt_counter),
                "Create empty entity",
                PatchSource::AiAssistant,
                vec![PatchOperation::Scene(ScenePatchOperation::CreateEntity {
                    operation_id: format!("ai-op-{}-create", self.ai_prompt_counter),
                    depends_on: Vec::new(),
                    parent_id: None,
                    name: extract_quoted_text(prompt).unwrap_or_else(|| "AI Entity".to_string()),
                })],
            );
            let command = PatchApplier::expand(&patch)
                .into_iter()
                .next()
                .expect("create entity patch should expand to one command");
            let proposal_id = format!("ai-proposal-{}-create", self.ai_prompt_counter);
            let validation = PatchValidator::validate(self, &patch);
            let review = PatchReviewModel::from_patch(&patch, validation);
            let evidence = project_patch_evidence(&review, false);
            proposed_commands.push(AiProposedCommand {
                proposal_id: proposal_id.clone(),
                label: "Create empty entity".to_string(),
                explanation: "Create a general empty scene entity from a ProjectPatch plan."
                    .to_string(),
                command,
                project_patch: Some(evidence),
                imported_project_patch: None,
                review_state: AiCommandReviewState::Proposed,
            });
            project_patch_proposals.push(ProjectCandidateProposal { proposal_id, patch });
        } else if normalized.contains("rename")
            || prompt.contains("重命名")
            || prompt.contains("改名")
        {
            if let Some(entity_id) = selected {
                proposed_commands.push(AiProposedCommand {
                    proposal_id: format!("ai-proposal-{}-rename", self.ai_prompt_counter),
                    label: "Rename selected entity".to_string(),
                    explanation: "Rename the currently selected scene entity.".to_string(),
                    command: UiCommandPayload::RenameSceneEntity {
                        entity_id,
                        name: extract_quoted_text(prompt).unwrap_or_else(|| {
                            extract_name_after_keyword(prompt)
                                .unwrap_or_else(|| "AI Renamed Entity".to_string())
                        }),
                    },
                    project_patch: None,
                    imported_project_patch: None,
                    review_state: AiCommandReviewState::Proposed,
                });
            }
        } else if normalized.contains("delete") || prompt.contains("删除") {
            if let Some(entity_id) = selected {
                proposed_commands.push(AiProposedCommand {
                    proposal_id: format!("ai-proposal-{}-delete", self.ai_prompt_counter),
                    label: "Delete selected entity".to_string(),
                    explanation: "Delete the currently selected scene entity subtree.".to_string(),
                    command: UiCommandPayload::DeleteSceneEntity { entity_id },
                    project_patch: None,
                    imported_project_patch: None,
                    review_state: AiCommandReviewState::Proposed,
                });
            }
        }

        let diagnostics = if proposed_commands.is_empty() {
            vec![self.make_diagnostic(
                transaction,
                DiagnosticSeverity::Warning,
                "editor.ai_panel.no_plan",
                "AI Panel mock planner could not map the prompt to an allowed editor command.",
                Some("Try create, rename, or delete a selected scene entity."),
            )]
        } else {
            Vec::new()
        };

        (
            AiPanelResponse {
                explanation: if proposed_commands.is_empty() {
                    "I could not produce a safe editor command from this prompt.".to_string()
                } else {
                    "I produced a reviewable editor command plan. Confirm it before applying."
                        .to_string()
                },
                proposed_commands,
                risk_summary: Some(
                    "The command will run through EditorSession and CommandTransaction."
                        .to_string(),
                ),
                requires_confirmation: true,
                diagnostics,
            },
            project_patch_proposals,
        )
    }
}

fn project_patch_evidence(review: &PatchReviewModel, repaired_once: bool) -> ProjectPatchEvidence {
    ProjectPatchEvidence {
        patch_id: review.patch_id.clone(),
        patch_title: review.title.clone(),
        touched_domains: review
            .touched_domains
            .iter()
            .map(|domain| format!("{domain:?}"))
            .collect(),
        operation_count: review.operation_count,
        validation_status: review.validation_status,
        risk_level: format!("{:?}", review.risk_level),
        repaired_once,
        diagnostics: review
            .diagnostics
            .iter()
            .map(|diagnostic| ProjectPatchDiagnosticEvidence {
                severity: format!("{:?}", diagnostic.severity),
                code: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                operation_id: diagnostic.operation_id.clone(),
                target: diagnostic.target.clone(),
            })
            .collect(),
        requires_confirmation: review.requires_confirmation,
    }
}

fn imported_project_patch_evidence(
    result: &ProjectPatchImportResult,
    proposal_id: Option<&str>,
) -> ImportedProjectPatchEvidence {
    ImportedProjectPatchEvidence {
        source_kind: format!("{:?}", result.source_kind),
        source_label: result.source_label.clone(),
        patch_id: result
            .parsed_patch
            .as_ref()
            .map(|patch| patch.patch_id.clone()),
        parse_status: format!("{:?}", result.parse_status),
        validation_status: result
            .validation
            .as_ref()
            .map(|validation| validation.accepted),
        review_state: proposal_id
            .map(|_| "Proposed".to_string())
            .unwrap_or_else(|| "Rejected".to_string()),
    }
}

fn push_import_result_diagnostics(
    session: &EditorSession,
    transaction: &mut CommandTransaction,
    result: &ProjectPatchImportResult,
) {
    for diagnostic in result
        .schema_diagnostics
        .iter()
        .chain(result.capability_diagnostics.iter())
        .chain(
            result
                .validation
                .iter()
                .flat_map(|validation| validation.diagnostics.iter()),
        )
    {
        let severity = match diagnostic.severity {
            crate::PatchDiagnosticSeverity::Info => DiagnosticSeverity::Info,
            crate::PatchDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            crate::PatchDiagnosticSeverity::Error => DiagnosticSeverity::Error,
        };
        let mut editor_diagnostic = session.make_diagnostic(
            transaction,
            severity,
            &diagnostic.code,
            diagnostic.message.clone(),
            Some("Open ProjectPatch import report or fix the imported JSON."),
        );
        editor_diagnostic.path = diagnostic.target.clone();
        transaction.diagnostics.push(editor_diagnostic);
    }
}

fn extract_quoted_text(prompt: &str) -> Option<String> {
    let mut quote_start = None;
    for (index, ch) in prompt.char_indices() {
        if matches!(ch, '"' | '\'' | '“' | '”') {
            if let Some(start) = quote_start {
                let value = prompt[start..index].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
                quote_start = None;
            } else {
                quote_start = Some(index + ch.len_utf8());
            }
        }
    }
    None
}

fn extract_name_after_keyword(prompt: &str) -> Option<String> {
    for keyword in ["叫", "为", "成", "to", "as"] {
        if let Some((_, value)) = prompt.rsplit_once(keyword) {
            let name = value
                .trim()
                .trim_matches(|ch: char| ch == ':' || ch == '：' || ch.is_whitespace());
            if !name.is_empty() && name.chars().count() <= 32 {
                return Some(name.to_string());
            }
        }
    }
    None
}
