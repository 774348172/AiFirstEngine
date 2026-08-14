use editor_ui_model::{
    AiPanelMessage, AiProposedCommand, Animator2DAuthoringCommand, Animator2DAuthoringModel,
    Animator2DAuthoringResult, AuthoringStepId, ConsoleEntry, ConsoleLevel, ConsoleSource,
    DiagnosticSeverity, DiagnosticSource, EditorDiagnostic, UiCommand, UiCommandPayload,
    WorkspaceSelectionTarget, WorkspaceViewMode,
};
use engine_runtime::frame_loop::{FrameLoop, FrameOutput};
use engine_runtime::game_view_presentation::GameViewTargetSpec;
use engine_runtime::input_mapping::RuntimeInputFrame;
use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use engine_runtime::rhi_command_plan::RhiCommandPlan;
use engine_runtime::runtime_package::RuntimePackage;
use engine_runtime::world::World;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::project_patch::ProjectFileSnapshotSet;
use crate::services::rule_service::{
    decode_add_operation_command, decode_add_statement_command, decode_trigger_command,
    decode_update_operation_command, decode_update_statement_command,
};
use crate::services::scene_service::ui_vec3_to_editor;
use crate::{
    command_id_for_payload, execute_editor_command, ui_command_to_editor_command_request,
    ApplyRuntimeChangeReport, AssetBrowserSessionState, AssetPlacementRequest, BuildProfile,
    CommandResult, CommandStatus, CommandTransaction, ComponentSchemaRegistry,
    ConsistencyReportLevel, DesktopExportReport, EditorBuildAndRunMode, EditorBuildAndRunReport,
    EditorPlayPreviewPackageReport, EditorRuntimePlayInstance, EditorSceneDocument,
    EditorTransform, EntitySelectionSource, GameViewPresentDiagnostic, GameViewPresentReport,
    GameViewRuntimeFrame, InputMappingEditCommand, InputMappingEditorState, InspectorSourceData,
    PatchHistory, PlaySessionController, PlaySessionReport, PlaySessionState, PrefabAuthoringModel,
    PreviewWorldSyncReport, ProjectIntentWorkflow, ProjectLauncherState, ProjectPatchDocument,
    ProjectPreviewEvidence, ProjectPreviewEvidenceError, ProjectPreviewFrameCapture,
    ProjectPreviewFrameEvidence, ProjectPreviewFrameReadback, ProjectPreviewFrameResult,
    ProjectPreviewFrameTicket, ProjectSession, PropertyTreeBuildResult, PropertyTreeBuilder,
    ReleasePackageReport, RuntimeWorldPickRequest, SaveReloadRebuildConsistencyReport,
    SceneEditCommand, SceneEditTransactionReport, SceneSelection, SceneUndoStack, UndoPolicy,
};

pub(crate) enum PreparedProjectOpenBinding<'a> {
    Missing,
    Valid(&'a crate::ProjectCandidateProjectBinding),
    Invalid,
}

pub struct EditorSession {
    pub(crate) project_launcher: ProjectLauncherState,
    pub(crate) active_project_session: Option<ProjectSession>,
    pub(crate) project_intent_workflow: ProjectIntentWorkflow,
    pub(crate) prepared_project_open: Option<crate::PreparedProjectOpen>,
    pub(crate) asset_browser_state: AssetBrowserSessionState,
    pub(crate) selected_project_browser_path: Option<String>,
    pub(crate) input_mapping_editor_state: Option<InputMappingEditorState>,
    pub(crate) animator2d_authoring: crate::Animator2DAuthoringService,
    pub(crate) workspace_view_mode: WorkspaceViewMode,
    pub(crate) active_authoring_step: AuthoringStepId,
    pub(crate) runtime_package_path: Option<PathBuf>,
    pub(crate) runtime_package: Option<RuntimePackage>,
    pub(crate) editor_scene_document: Option<EditorSceneDocument>,
    pub(crate) scene_selection: SceneSelection,
    pub(crate) selected_aui_node: Option<WorkspaceSelectionTarget>,
    pub(crate) scene_undo_stack: SceneUndoStack,
    pub(crate) scene_path: Option<PathBuf>,
    pub(crate) last_scene_edit_report: Option<SceneEditTransactionReport>,
    pub(crate) last_preview_world_sync_report: Option<PreviewWorldSyncReport>,
    pub(crate) prefab_authoring: PrefabAuthoringModel,
    pub(crate) world: Option<World>,
    pub(crate) frame_loop: Option<FrameLoop>,
    pub(crate) last_frame_output: Option<FrameOutput>,
    pub(crate) selected_entity_id: Option<String>,
    pub(crate) selected_entity_source: Option<EntitySelectionSource>,
    pub(crate) selected_trace_entry_id: Option<String>,
    pub(crate) console_entries: Vec<ConsoleEntry>,
    pub(crate) diagnostics: Vec<EditorDiagnostic>,
    pub(crate) ai_panel_messages: Vec<AiPanelMessage>,
    pub(crate) ai_proposed_commands: Vec<AiProposedCommand>,
    pub(crate) project_candidate_proposals: Vec<ProjectCandidateProposal>,
    pub(crate) patch_history: PatchHistory,
    pub(crate) patch_file_snapshot_history: Vec<(String, ProjectFileSnapshotSet)>,
    pub(crate) ai_prompt_counter: u64,
    pub(crate) ai_prompt_draft: String,
    pub(crate) ai_panel_stage: editor_ui_model::AiPanelStage,
    pub(crate) ai_panel_status_summary: Option<String>,
    pub(crate) llm_patch_request_generation: u64,
    pub(crate) active_llm_patch_request: Option<ActiveLlmPatchRequest>,
    pub(crate) llm_request_controller: crate::LlmRequestController,
    pub(crate) llm_patch_source_override: Option<crate::LlmPatchSourceConfig>,
    pub(crate) llm_patch_report_level: crate::LlmPatchReportLevel,
    pub(crate) last_llm_patch_report: Option<crate::LlmPatchRequestReport>,
    pub(crate) last_llm_shutdown_receipt: Option<crate::LlmShutdownReceipt>,
    pub(crate) revision: u64,
    pub(crate) transaction_counter: u64,
    pub(crate) play_session_controller: PlaySessionController,
    pub(crate) linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    pub(crate) project_editor_composition_identity: Option<crate::ProjectEditorCompositionIdentity>,
    pub(crate) last_play_session_report: Option<PlaySessionReport>,
    pub(crate) last_editor_preview_package_report: Option<EditorPlayPreviewPackageReport>,
    pub(crate) prepared_editor_play_report: Option<EditorPlayPreviewPackageReport>,
    pub(crate) editor_runtime_play_instance: Option<EditorRuntimePlayInstance>,
    pub(crate) last_game_view_runtime_frame: Option<GameViewRuntimeFrame>,
    pub(crate) last_game_view_present_report: Option<GameViewPresentReport>,
    pub(crate) pending_project_preview_frame_ticket: Option<ProjectPreviewFrameTicket>,
    pub(crate) project_preview_frame_result: Option<ProjectPreviewFrameResult>,
    pub(crate) last_runtime_apply_report: Option<ApplyRuntimeChangeReport>,
    pub(crate) game_view_target: GameViewTargetSpec,
    pub(crate) game_view_maximize_on_play: bool,
    pub(crate) is_game_view_maximized: bool,
    pub(crate) game_view_restore_workspace_region: Option<String>,
    pub(crate) game_view_maximize_reason: Option<String>,
    pub(crate) last_desktop_export_report: Option<DesktopExportReport>,
    pub(crate) last_build_and_run_report: Option<EditorBuildAndRunReport>,
    pub(crate) release_profile_cache: Option<BuildProfile>,
    pub(crate) release_profile_source_hash: Option<String>,
    pub(crate) release_profile_dirty: bool,
    pub(crate) last_release_package_report: Option<ReleasePackageReport>,
    pub(crate) save_reload_rebuild_report: Option<SaveReloadRebuildConsistencyReport>,
    pub(crate) save_reload_rebuild_report_level: ConsistencyReportLevel,
    pub(crate) selected_report_id: Option<String>,
    pub(crate) selected_rule_card_id: Option<String>,
    pub(crate) selected_rule_graph_node_id: Option<String>,
}

impl Default for EditorSession {
    fn default() -> Self {
        Self {
            project_launcher: ProjectLauncherState::default(),
            active_project_session: None,
            project_intent_workflow: ProjectIntentWorkflow::default(),
            prepared_project_open: None,
            asset_browser_state: AssetBrowserSessionState::default(),
            selected_project_browser_path: None,
            input_mapping_editor_state: None,
            animator2d_authoring: crate::Animator2DAuthoringService::default(),
            workspace_view_mode: WorkspaceViewMode::SceneView,
            active_authoring_step: AuthoringStepId::Project,
            runtime_package_path: None,
            runtime_package: None,
            editor_scene_document: None,
            scene_selection: SceneSelection::default(),
            selected_aui_node: None,
            scene_undo_stack: SceneUndoStack::default(),
            scene_path: None,
            last_scene_edit_report: None,
            last_preview_world_sync_report: None,
            prefab_authoring: PrefabAuthoringModel::default(),
            world: None,
            frame_loop: None,
            last_frame_output: None,
            selected_entity_id: None,
            selected_entity_source: None,
            selected_trace_entry_id: None,
            console_entries: Vec::new(),
            diagnostics: Vec::new(),
            ai_panel_messages: Vec::new(),
            ai_proposed_commands: Vec::new(),
            project_candidate_proposals: Vec::new(),
            patch_history: PatchHistory::default(),
            patch_file_snapshot_history: Vec::new(),
            ai_prompt_counter: 0,
            ai_prompt_draft: String::new(),
            ai_panel_stage: editor_ui_model::AiPanelStage::Idle,
            ai_panel_status_summary: None,
            llm_patch_request_generation: 0,
            active_llm_patch_request: None,
            llm_request_controller: crate::LlmRequestController::default(),
            llm_patch_source_override: None,
            llm_patch_report_level: crate::LlmPatchReportLevel::Summary,
            last_llm_patch_report: None,
            last_llm_shutdown_receipt: None,
            revision: 0,
            transaction_counter: 0,
            play_session_controller: PlaySessionController::new(),
            linked_project_runtimes: Arc::new(LinkedProjectRuntimeSet::explicit_empty()),
            project_editor_composition_identity: None,
            last_play_session_report: None,
            last_editor_preview_package_report: None,
            prepared_editor_play_report: None,
            editor_runtime_play_instance: None,
            last_game_view_runtime_frame: None,
            last_game_view_present_report: None,
            pending_project_preview_frame_ticket: None,
            project_preview_frame_result: None,
            last_runtime_apply_report: None,
            game_view_target: GameViewTargetSpec::default(),
            game_view_maximize_on_play: false,
            is_game_view_maximized: false,
            game_view_restore_workspace_region: None,
            game_view_maximize_reason: None,
            last_desktop_export_report: None,
            last_build_and_run_report: None,
            release_profile_cache: None,
            release_profile_source_hash: None,
            release_profile_dirty: false,
            last_release_package_report: None,
            save_reload_rebuild_report: None,
            save_reload_rebuild_report_level: ConsistencyReportLevel::Summary,
            selected_report_id: None,
            selected_rule_card_id: None,
            selected_rule_graph_node_id: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectCandidateProposal {
    pub proposal_id: String,
    pub patch: ProjectPatchDocument,
}

pub(crate) struct ActiveLlmPatchRequest {
    pub request_id: crate::LlmRequestId,
    pub expected_post_start_revision: u64,
    pub context_hash: String,
    pub generation: u64,
    pub attempt_index: u8,
    pub maximum_candidate_bytes: usize,
    pub initial_candidate: Option<String>,
    pub initial_import: Option<crate::ProjectPatchImportResult>,
}

impl EditorSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute_animator2d_authoring_command(
        &mut self,
        command: Animator2DAuthoringCommand,
    ) -> Animator2DAuthoringResult {
        let result = self.animator2d_authoring.execute(command);
        if result.status == editor_ui_model::Animator2DAuthoringStatus::Applied {
            self.revision = self.revision.saturating_add(1);
        }
        result
    }

    pub fn tick_animator2d_preview(&mut self) -> Animator2DAuthoringResult {
        self.animator2d_authoring.tick_preview()
    }

    pub fn animator2d_authoring_model(&self) -> Animator2DAuthoringModel {
        self.animator2d_authoring.model()
    }

    pub(crate) fn sync_animator2d_play_observations(&mut self) {
        let observations = self
            .last_game_view_runtime_frame
            .as_ref()
            .map(|frame| frame.animator2d_play_observations.clone())
            .unwrap_or_default();
        self.animator2d_authoring
            .set_play_observations(observations);
    }

    pub fn set_game_view_target(&mut self, target: GameViewTargetSpec) {
        self.game_view_target = target;
    }

    pub fn game_view_target(&self) -> GameViewTargetSpec {
        self.game_view_target
    }

    pub fn with_linked_project_runtimes(
        linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    ) -> Self {
        Self {
            linked_project_runtimes,
            ..Self::default()
        }
    }

    pub fn with_project_editor_composition(
        linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
        identity: crate::ProjectEditorCompositionIdentity,
    ) -> Result<Self, crate::ProjectEditorCompositionContractError> {
        identity.validate()?;
        Ok(Self {
            linked_project_runtimes,
            project_editor_composition_identity: Some(identity),
            ..Self::default()
        })
    }

    pub fn project_editor_composition_identity(
        &self,
    ) -> Option<&crate::ProjectEditorCompositionIdentity> {
        self.project_editor_composition_identity.as_ref()
    }

    pub fn install_prepared_project_open(&mut self, prepared: crate::PreparedProjectOpen) {
        self.prepared_project_open = Some(prepared);
    }

    pub fn clear_prepared_project_open(&mut self) {
        self.prepared_project_open = None;
    }

    pub(crate) fn prepared_binding_for_active_project(&self) -> PreparedProjectOpenBinding<'_> {
        let Some(prepared) = self.prepared_project_open.as_ref() else {
            return PreparedProjectOpenBinding::Missing;
        };
        let Some(active) = self.active_project_session.as_ref() else {
            return PreparedProjectOpenBinding::Invalid;
        };
        let Ok(active_root) = active.project_root.canonicalize() else {
            return PreparedProjectOpenBinding::Invalid;
        };
        let Ok(manifest_bytes) = std::fs::read(active_root.join("project.aife.json")) else {
            return PreparedProjectOpenBinding::Invalid;
        };
        let manifest_digest = engine_runtime::canonical_digest::sha256_prefixed(&manifest_bytes);
        if prepared.schema_version == crate::PROJECT_OPEN_PREPARATION_SCHEMA_VERSION
            && prepared.project_root == active_root
            && prepared.project_id == active.manifest.project_id
            && prepared.manifest_digest == manifest_digest
            && prepared.binding.project_id == active.manifest.project_id
            && Path::new(&prepared.binding.project_root) == active_root
        {
            PreparedProjectOpenBinding::Valid(&prepared.binding)
        } else {
            PreparedProjectOpenBinding::Invalid
        }
    }

    pub fn execute_command(&mut self, command: UiCommand) -> CommandResult {
        execute_editor_command(self, ui_command_to_editor_command_request(command))
    }

    pub fn execute_build_and_run_desktop_package_for_test(
        &mut self,
        profile_id: Option<String>,
        run_mode: EditorBuildAndRunMode,
        timeout_ms: u64,
        frame_limit: u64,
    ) -> CommandResult {
        let payload = UiCommandPayload::BuildAndRunDesktopPackage { profile_id };
        let mut transaction = self.begin_transaction(UiCommand {
            command_id: command_id_for_payload(&payload).to_string(),
            source: editor_ui_model::UiCommandSource::Test,
            request_id: "request-build-and-run-test".to_string(),
            payload,
        });
        let profile_id = match transaction.payload.clone() {
            UiCommandPayload::BuildAndRunDesktopPackage { profile_id } => profile_id,
            _ => None,
        };
        self.build_and_run_desktop_package_with_mode(
            &mut transaction,
            profile_id,
            run_mode,
            timeout_ms,
            frame_limit,
        )
    }

    pub fn execute_build_release_package_for_test(
        &mut self,
        player_executable: PathBuf,
        output_dir: PathBuf,
        report_level: crate::ReleasePackageReportLevel,
        verify_process: bool,
    ) -> CommandResult {
        let payload = UiCommandPayload::BuildReleasePackage {
            profile_id: Some("windows-release".to_string()),
        };
        let mut transaction = self.begin_transaction(UiCommand {
            command_id: command_id_for_payload(&payload).to_string(),
            source: editor_ui_model::UiCommandSource::Test,
            request_id: "request-build-release-test".to_string(),
            payload,
        });
        self.build_release_package_with_overrides(
            &mut transaction,
            Some("windows-release".to_string()),
            Some(player_executable),
            Some(output_dir),
            report_level,
            verify_process,
        )
    }

    pub(crate) fn execute_ui_command_direct(&mut self, command: UiCommand) -> CommandResult {
        let mut transaction = self.begin_transaction(command);
        let result = match transaction.payload.clone() {
            UiCommandPayload::OpenProject { path } => {
                self.open_project(&mut transaction, Path::new(&path))
            }
            UiCommandPayload::CreateProject { path, name } => {
                self.create_project(&mut transaction, Path::new(&path), &name)
            }
            UiCommandPayload::StartCreateProjectWithAi { draft_path } => {
                self.start_create_project_with_ai(&mut transaction, draft_path)
            }
            UiCommandPayload::SelectRecentProject { path } => {
                self.select_recent_project(&mut transaction, Path::new(&path))
            }
            UiCommandPayload::RefreshRecentProjects => {
                self.refresh_recent_projects(&mut transaction)
            }
            UiCommandPayload::SelectProjectBrowserEntry { path } => {
                self.select_project_browser_entry(&mut transaction, path)
            }
            UiCommandPayload::OpenProjectBrowserEntry { path } => {
                self.open_project_browser_entry(&mut transaction, path)
            }
            UiCommandPayload::SelectAssetBrowserEntry {
                entry_key,
                additive,
                range,
            } => self.select_asset_browser_entry(&mut transaction, entry_key, additive, range),
            UiCommandPayload::OpenAssetBrowserEntry { entry_key } => {
                self.open_asset_browser_entry(&mut transaction, entry_key)
            }
            UiCommandPayload::SetAssetBrowserFolder { folder } => {
                self.set_asset_browser_folder(&mut transaction, folder)
            }
            UiCommandPayload::SetAssetBrowserSearch { search_text } => {
                self.set_asset_browser_search(&mut transaction, search_text)
            }
            UiCommandPayload::SetAssetBrowserKindFilter { kinds } => {
                self.set_asset_browser_kind_filter(&mut transaction, kinds)
            }
            UiCommandPayload::AssetBrowserToolbar { action } => {
                self.asset_browser_toolbar_action(&mut transaction, action)
            }
            UiCommandPayload::ScrollAssetBrowser { delta } => {
                self.scroll_asset_browser(&mut transaction, delta)
            }
            UiCommandPayload::BeginAssetPick { field_id } => {
                self.begin_asset_pick(&mut transaction, field_id)
            }
            UiCommandPayload::ConfirmAssetPick => self.confirm_asset_pick(&mut transaction),
            UiCommandPayload::CancelAssetPick => self.cancel_asset_pick(&mut transaction),
            UiCommandPayload::DropAssetOnInspectorField {
                entry_key,
                field_id,
            } => self.drop_asset_on_inspector_field(&mut transaction, entry_key, field_id),
            UiCommandPayload::CreateDefaultInputMapping { path } => {
                self.create_default_input_mapping(&mut transaction, path)
            }
            UiCommandPayload::DeleteInputMapping { path } => {
                self.delete_input_mapping(&mut transaction, path)
            }
            UiCommandPayload::OpenInputMapping { path } => {
                self.open_input_mapping(&mut transaction, path)
            }
            UiCommandPayload::SaveInputMapping { path } => {
                self.validate_input_mapping(&mut transaction, path, true)
            }
            UiCommandPayload::DiscardInputMappingDraft { path } => {
                self.discard_input_mapping_draft(&mut transaction, path)
            }
            UiCommandPayload::ValidateInputMapping { path } => {
                self.validate_input_mapping(&mut transaction, path, false)
            }
            UiCommandPayload::SelectInputContext { path, context_id } => {
                self.select_input_context(&mut transaction, path, context_id)
            }
            UiCommandPayload::SelectInputAction { path, action_id } => {
                self.select_input_action(&mut transaction, path, action_id)
            }
            UiCommandPayload::SelectInputBinding { path, binding_id } => {
                self.select_input_binding(&mut transaction, path, binding_id)
            }
            UiCommandPayload::AddInputContext {
                path,
                context_id,
                priority,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::AddContext {
                    context_id,
                    priority,
                },
            ),
            UiCommandPayload::RemoveInputContext { path, context_id } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::RemoveContext { context_id },
            ),
            UiCommandPayload::SetInputContextPriority {
                path,
                context_id,
                priority,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetContextPriority {
                    context_id,
                    priority,
                },
            ),
            UiCommandPayload::SetInputContextConsumeInput {
                path,
                context_id,
                consume_input,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetContextConsumeInput {
                    context_id,
                    consume_input,
                },
            ),
            UiCommandPayload::AddInputAction {
                path,
                action_id,
                value_type,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::AddAction {
                    action_id,
                    value_type,
                },
            ),
            UiCommandPayload::RemoveInputAction { path, action_id } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::RemoveAction { action_id },
            ),
            UiCommandPayload::SetInputActionValueType {
                path,
                action_id,
                value_type,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetActionValueType {
                    action_id,
                    value_type,
                },
            ),
            UiCommandPayload::AddInputBinding {
                path,
                context_id,
                action_id,
                device_path,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::AddBinding {
                    context_id,
                    action_id,
                    device_path,
                },
            ),
            UiCommandPayload::RemoveInputBinding {
                path,
                binding_index,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::RemoveBinding { binding_index },
            ),
            UiCommandPayload::SetInputBindingDevicePath {
                path,
                binding_index,
                device_path,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetBindingDevicePath {
                    binding_index,
                    device_path,
                },
            ),
            UiCommandPayload::SetInputBindingProcessorByIndex {
                path,
                binding_index,
                processor,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetBindingProcessorByIndex {
                    binding_index,
                    processor,
                },
            ),
            UiCommandPayload::RemoveInputBindingById { path, binding_id } => self
                .edit_input_mapping(
                    &mut transaction,
                    path,
                    InputMappingEditCommand::RemoveBindingById { binding_id },
                ),
            UiCommandPayload::SetInputBindingDevicePathById {
                path,
                binding_id,
                device_path,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetBindingDevicePathById {
                    binding_id,
                    device_path,
                },
            ),
            UiCommandPayload::SetInputBindingTrigger {
                path,
                binding_id,
                trigger,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetBindingTrigger {
                    binding_id,
                    trigger,
                },
            ),
            UiCommandPayload::SetInputBindingProcessor {
                path,
                binding_id,
                processor,
            } => self.edit_input_mapping(
                &mut transaction,
                path,
                InputMappingEditCommand::SetBindingProcessor {
                    binding_id,
                    processor,
                },
            ),
            UiCommandPayload::BeginInputBindingCapture { path, binding_id } => {
                self.begin_input_binding_capture(&mut transaction, path, binding_id)
            }
            UiCommandPayload::CancelInputBindingCapture { path } => {
                self.cancel_input_binding_capture(&mut transaction, path)
            }
            UiCommandPayload::CommitCapturedInputBinding {
                path,
                binding_id,
                device_path,
            } => {
                self.commit_captured_input_binding(&mut transaction, path, binding_id, device_path)
            }
            UiCommandPayload::PreviewInputMapping { path, device_path } => {
                self.preview_input_mapping(&mut transaction, path, device_path)
            }
            UiCommandPayload::SetInputMappingReportLevel { path, level } => {
                self.set_input_mapping_report_level(&mut transaction, path, level)
            }
            UiCommandPayload::RegisterExistingAsset {
                path,
                expected_kind,
            } => self.register_existing_asset(&mut transaction, path, expected_kind),
            UiCommandPayload::GenerateMockImageAsset {
                prompt,
                target_folder,
                asset_name,
                image_kind,
                width,
                height,
                transparent_background,
            } => self.generate_mock_image_asset(
                &mut transaction,
                prompt,
                target_folder,
                asset_name,
                image_kind,
                width,
                height,
                transparent_background,
            ),
            UiCommandPayload::ValidateAssetBrowserIndex { query_kind } => {
                self.validate_asset_browser_index(&mut transaction, query_kind)
            }
            UiCommandPayload::CreateRuleAsset {
                path,
                rule_id,
                display_name,
                phase,
            } => self.create_rule_asset(&mut transaction, path, rule_id, display_name, phase),
            UiCommandPayload::OpenRuleAsset { path } => {
                self.open_rule_asset(&mut transaction, path)
            }
            UiCommandPayload::SelectRuleAsset { path } => {
                self.select_rule_asset(&mut transaction, path)
            }
            UiCommandPayload::SetRuleTrigger {
                path,
                trigger,
                expected_ir_hash,
            } => match decode_trigger_command(trigger) {
                Ok(command) => {
                    self.edit_rule_asset(&mut transaction, path, command, expected_ir_hash)
                }
                Err(message) => {
                    self.push_error(
                        &mut transaction,
                        "editor.rule_authoring.decode_failed",
                        message,
                        Some("Send a valid RuleTrigger payload."),
                    );
                    self.finish_transaction(transaction, CommandStatus::Failed)
                }
            },
            UiCommandPayload::AddRuleStatement {
                path,
                statement,
                expected_ir_hash,
            } => match decode_add_statement_command(statement) {
                Ok(command) => {
                    self.edit_rule_asset(&mut transaction, path, command, expected_ir_hash)
                }
                Err(message) => {
                    self.push_error(
                        &mut transaction,
                        "editor.rule_authoring.decode_failed",
                        message,
                        Some("Send a valid RuleStatement payload."),
                    );
                    self.finish_transaction(transaction, CommandStatus::Failed)
                }
            },
            UiCommandPayload::UpdateRuleStatement {
                path,
                statement_index,
                statement,
                expected_ir_hash,
            } => match decode_update_statement_command(statement_index, statement) {
                Ok(command) => {
                    self.edit_rule_asset(&mut transaction, path, command, expected_ir_hash)
                }
                Err(message) => {
                    self.push_error(
                        &mut transaction,
                        "editor.rule_authoring.decode_failed",
                        message,
                        Some("Send a valid RuleStatement payload."),
                    );
                    self.finish_transaction(transaction, CommandStatus::Failed)
                }
            },
            UiCommandPayload::RemoveRuleStatement {
                path,
                statement_index,
                expected_ir_hash,
            } => self.edit_rule_asset(
                &mut transaction,
                path,
                crate::RuleAuthoringEditCommand::RemoveStatement {
                    index: statement_index,
                },
                expected_ir_hash,
            ),
            UiCommandPayload::AddRuleOperation {
                path,
                operation,
                expected_ir_hash,
            } => match decode_add_operation_command(operation) {
                Ok(command) => {
                    self.edit_rule_asset(&mut transaction, path, command, expected_ir_hash)
                }
                Err(message) => {
                    self.push_error(
                        &mut transaction,
                        "editor.rule_authoring.decode_failed",
                        message,
                        Some("Send a valid RuleOperation payload."),
                    );
                    self.finish_transaction(transaction, CommandStatus::Failed)
                }
            },
            UiCommandPayload::UpdateRuleOperation {
                path,
                operation_index,
                operation,
                expected_ir_hash,
            } => match decode_update_operation_command(operation_index, operation) {
                Ok(command) => {
                    self.edit_rule_asset(&mut transaction, path, command, expected_ir_hash)
                }
                Err(message) => {
                    self.push_error(
                        &mut transaction,
                        "editor.rule_authoring.decode_failed",
                        message,
                        Some("Send a valid RuleOperation payload."),
                    );
                    self.finish_transaction(transaction, CommandStatus::Failed)
                }
            },
            UiCommandPayload::RemoveRuleOperation {
                path,
                operation_index,
                expected_ir_hash,
            } => self.edit_rule_asset(
                &mut transaction,
                path,
                crate::RuleAuthoringEditCommand::RemoveOperation {
                    index: operation_index,
                },
                expected_ir_hash,
            ),
            UiCommandPayload::ValidateRuleAsset { path } => {
                self.validate_rule_asset(&mut transaction, path)
            }
            UiCommandPayload::BuildRuleArtifact { path } => {
                self.build_rule_artifact(&mut transaction, path)
            }
            UiCommandPayload::BuildProjectRuleManifest { path } => {
                self.build_project_rule_manifest(&mut transaction, path)
            }
            UiCommandPayload::SaveRuleAsset { path } => {
                self.save_rule_asset(&mut transaction, path)
            }
            UiCommandPayload::OpenRuleDiagnostics { path } => {
                self.open_rule_diagnostics(&mut transaction, path)
            }
            UiCommandPayload::SelectRuleCard { path, card_id } => {
                self.select_rule_card(&mut transaction, path, card_id)
            }
            UiCommandPayload::SetRuleCardField {
                path,
                card_id,
                field_path,
                value,
                expected_ir_hash,
            } => self.set_rule_card_field(
                &mut transaction,
                path,
                card_id,
                field_path,
                value,
                expected_ir_hash,
            ),
            UiCommandPayload::AddRuleCard {
                path,
                card_kind,
                value,
                expected_ir_hash,
            } => self.add_rule_card(&mut transaction, path, card_kind, value, expected_ir_hash),
            UiCommandPayload::RemoveRuleCard {
                path,
                card_id,
                expected_ir_hash,
            } => self.remove_rule_card(&mut transaction, path, card_id, expected_ir_hash),
            UiCommandPayload::SelectRuleGraphNode { path, node_id } => {
                self.select_rule_graph_node(&mut transaction, path, node_id)
            }
            UiCommandPayload::RefreshRuleGraphPreview { path } => {
                self.refresh_rule_graph_preview(&mut transaction, path)
            }
            UiCommandPayload::CreatePrefabFromSelection {
                scene_path,
                root_entity_id,
                prefab_id,
                name,
                replace_selection_with_instance,
            } => self.create_prefab_from_selection(
                &mut transaction,
                scene_path,
                root_entity_id,
                prefab_id,
                name,
                replace_selection_with_instance,
            ),
            UiCommandPayload::OpenPrefabDocument { path } => {
                self.open_prefab_document(&mut transaction, path)
            }
            UiCommandPayload::EnterPrefabStage {
                path,
                mode,
                opened_from_instance_entity_id,
            } => self.enter_prefab_stage(
                &mut transaction,
                path,
                mode,
                opened_from_instance_entity_id,
            ),
            UiCommandPayload::ExitPrefabStage { save_policy } => {
                self.exit_prefab_stage(&mut transaction, save_policy)
            }
            UiCommandPayload::InstantiatePrefabInScene {
                prefab_id,
                parent_entity_id,
                local_position,
            } => self.instantiate_prefab_in_scene(
                &mut transaction,
                prefab_id,
                parent_entity_id,
                local_position,
            ),
            UiCommandPayload::SetPrefabStageEntityField {
                source_entity_id,
                component_type,
                field_path,
                value,
            } => self.set_prefab_stage_entity_field(
                &mut transaction,
                source_entity_id,
                component_type,
                field_path,
                value,
            ),
            UiCommandPayload::ApplyPrefabOverrideToAsset {
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path,
            } => self.apply_prefab_override_to_asset(
                &mut transaction,
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path,
            ),
            UiCommandPayload::SavePrefabDocument { path } => {
                self.save_prefab_document(&mut transaction, path)
            }
            UiCommandPayload::ValidatePrefabReferences { path } => {
                self.validate_prefab_references(&mut transaction, path)
            }
            UiCommandPayload::RevertPrefabOverride {
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path,
            } => self.revert_prefab_override(
                &mut transaction,
                instance_entity_id,
                target_source_entity_id,
                component_type,
                field_path,
            ),
            UiCommandPayload::CreateAuiDocument {
                path,
                document_id,
                width,
                height,
            } => self.create_aui_document(&mut transaction, path, document_id, width, height),
            UiCommandPayload::OpenAuiDocument { path } => {
                self.open_aui_document(&mut transaction, path)
            }
            UiCommandPayload::SelectAuiNode {
                document_path,
                document_id,
                node_id,
            } => self.select_aui_node(&mut transaction, document_path, document_id, node_id),
            UiCommandPayload::AddAuiNode {
                path,
                parent_node_id,
                node_id,
                kind,
                name,
                rect,
            } => self.add_aui_node(
                &mut transaction,
                path,
                parent_node_id,
                node_id,
                kind,
                name,
                rect,
            ),
            UiCommandPayload::SetAuiNodeField {
                path,
                node_id,
                schema_path,
                value,
            } => self.set_aui_node_field(&mut transaction, path, node_id, schema_path, value),
            UiCommandPayload::SetAuiBindingPath {
                path,
                node_id,
                target_field,
                binding_id,
                binding_path,
                fallback,
            } => self.set_aui_binding_path(
                &mut transaction,
                path,
                node_id,
                target_field,
                binding_id,
                binding_path,
                fallback,
            ),
            UiCommandPayload::SetAuiActionRef {
                path,
                node_id,
                event,
                action_id,
                payload,
            } => {
                self.set_aui_action_ref(&mut transaction, path, node_id, event, action_id, payload)
            }
            UiCommandPayload::ValidateAuiDocument { path } => {
                self.validate_aui_document(&mut transaction, path)
            }
            UiCommandPayload::SaveAuiDocument { path } => {
                self.save_aui_document(&mut transaction, path)
            }
            UiCommandPayload::PreviewAuiOverlay { path } => {
                self.preview_aui_overlay(&mut transaction, path)
            }
            UiCommandPayload::SaveAuiSubtreeAsTemplate {
                document_path,
                root_node_id,
                template_asset_path,
                template_id,
                display_name,
            } => self.save_aui_subtree_as_template(
                &mut transaction,
                document_path,
                root_node_id,
                template_asset_path,
                template_id,
                display_name,
            ),
            UiCommandPayload::InstantiateAuiTemplate {
                template_asset_path,
                template_id,
                target_document_path,
                parent_node_id,
                insertion_index,
                instance_id,
                node_id_prefix,
            } => self.instantiate_aui_template(
                &mut transaction,
                template_asset_path,
                template_id,
                target_document_path,
                parent_node_id,
                insertion_index,
                instance_id,
                node_id_prefix,
            ),
            UiCommandPayload::ValidateAuiTemplate {
                template_asset_path,
                template_id,
            } => self.validate_aui_template(&mut transaction, template_asset_path, template_id),
            UiCommandPayload::SetWorkspaceViewMode { mode } => {
                self.set_workspace_view_mode(&mut transaction, mode)
            }
            UiCommandPayload::SetAuthoringWorkflowStep { step_id } => {
                self.set_authoring_workflow_step(&mut transaction, step_id)
            }
            UiCommandPayload::OpenRuntimePackage { path } => {
                self.open_runtime_package(&mut transaction, Path::new(&path))
            }
            UiCommandPayload::OpenSceneDocument { path } => {
                self.open_scene_document(&mut transaction, Path::new(&path))
            }
            UiCommandPayload::ReloadRuntimePackage => self.reload_runtime_package(&mut transaction),
            UiCommandPayload::SelectEntity { entity_id } => {
                self.select_entity(&mut transaction, &entity_id)
            }
            UiCommandPayload::SelectRuntimeEntity { entity_id } => {
                self.select_runtime_entity(&mut transaction, &entity_id)
            }
            UiCommandPayload::PickRuntimeEntityAt {
                x,
                y,
                viewport_width,
                viewport_height,
                aui_consumed,
            } => self.pick_runtime_entity_at(
                &mut transaction,
                RuntimeWorldPickRequest {
                    x,
                    y,
                    viewport_width,
                    viewport_height,
                    aui_consumed,
                },
            ),
            UiCommandPayload::SelectSceneEntity { entity_id } => self.execute_scene_edit(
                &mut transaction,
                SceneEditCommand::SelectEntity { entity_id },
            ),
            UiCommandPayload::CreateSceneEntity { parent_id, name } => self.execute_scene_edit(
                &mut transaction,
                SceneEditCommand::CreateEntity {
                    parent_id,
                    name,
                    mesh: None,
                    components: Vec::new(),
                    local_transform: EditorTransform::identity(),
                    sibling_order: None,
                },
            ),
            UiCommandPayload::PlaceAssetIntoScene {
                asset_id,
                asset_type,
                asset_guid,
                target_parent_id,
                local_position,
                placement_mode,
            } => self.place_asset_into_scene(
                &mut transaction,
                AssetPlacementRequest {
                    asset_id,
                    asset_type,
                    asset_guid,
                    target_parent_id,
                    local_position: local_position.map(ui_vec3_to_editor),
                    placement_mode,
                },
            ),
            UiCommandPayload::DeleteSceneEntity { entity_id } => self.execute_scene_edit(
                &mut transaction,
                SceneEditCommand::DeleteEntity {
                    entity_id,
                    delete_children: true,
                },
            ),
            UiCommandPayload::RenameSceneEntity { entity_id, name } => self.execute_scene_edit(
                &mut transaction,
                SceneEditCommand::RenameEntity { entity_id, name },
            ),
            UiCommandPayload::SetSceneTransform {
                entity_id,
                local_position,
                local_rotation,
                local_scale,
            } => self.execute_scene_edit(
                &mut transaction,
                SceneEditCommand::SetTransform {
                    entity_id,
                    local_position: local_position.map(ui_vec3_to_editor),
                    local_rotation: local_rotation.map(ui_vec3_to_editor),
                    local_scale: local_scale.map(ui_vec3_to_editor),
                },
            ),
            UiCommandPayload::AddSceneComponent {
                entity_id,
                component_type,
                fields,
            } => self.execute_scene_edit(
                &mut transaction,
                SceneEditCommand::AddComponent {
                    entity_id,
                    component_type,
                    fields,
                },
            ),
            UiCommandPayload::RemoveSceneComponent {
                entity_id,
                component_type,
            } => self.execute_scene_edit(
                &mut transaction,
                SceneEditCommand::RemoveComponent {
                    entity_id,
                    component_type,
                },
            ),
            UiCommandPayload::SetSceneComponentField {
                entity_id,
                component_type,
                field_path,
                value,
            } => {
                if let Some(result) = self.set_prefab_instance_override_field(
                    &mut transaction,
                    entity_id.clone(),
                    component_type.clone(),
                    field_path.clone(),
                    value.clone(),
                ) {
                    result
                } else {
                    self.execute_scene_edit(
                        &mut transaction,
                        SceneEditCommand::SetComponentField {
                            entity_id,
                            component_type,
                            field_path,
                            value,
                        },
                    )
                }
            }
            UiCommandPayload::SetRuntimeComponentFieldTemporary {
                entity_id,
                component_type,
                field_path,
                value,
            } => self.set_runtime_component_field_temporary(
                &mut transaction,
                entity_id,
                component_type,
                field_path,
                value,
            ),
            UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring => {
                self.preview_apply_runtime_change_to_authoring(&mut transaction)
            }
            UiCommandPayload::ApplyRuntimeChangeToAuthoring {
                edit_id,
                candidate_hash,
            } => self.apply_runtime_change_to_authoring(&mut transaction, edit_id, candidate_hash),
            UiCommandPayload::SaveSceneDocument { path } => {
                self.save_scene_document(&mut transaction, path.map(PathBuf::from))
            }
            UiCommandPayload::UndoSceneEdit => {
                self.execute_scene_edit(&mut transaction, SceneEditCommand::Undo)
            }
            UiCommandPayload::RedoSceneEdit => {
                self.execute_scene_edit(&mut transaction, SceneEditCommand::Redo)
            }
            UiCommandPayload::TickOneFrame => self.tick_one_frame(&mut transaction),
            UiCommandPayload::Play => self.start_play_session(&mut transaction),
            UiCommandPayload::Pause => self.pause_active_game_view_play_session(&mut transaction),
            UiCommandPayload::StepFrame => {
                self.step_active_game_view_play_session(&mut transaction)
            }
            UiCommandPayload::StopPlaySession => self.stop_play_session(&mut transaction),
            UiCommandPayload::SetGameViewTarget {
                width,
                height,
                scale_policy,
            } => self.set_current_game_view_target(&mut transaction, width, height, scale_policy),
            UiCommandPayload::SetGameViewMaximizeOnPlay { enabled } => {
                self.set_game_view_maximize_on_play(&mut transaction, enabled)
            }
            UiCommandPayload::ToggleGameViewMaximizeOnPlay => {
                self.toggle_game_view_maximize_on_play(&mut transaction)
            }
            UiCommandPayload::ResetRuntime => self.reset_runtime(&mut transaction),
            UiCommandPayload::ExportDesktopPackage { profile_id } => {
                self.export_desktop_package(&mut transaction, profile_id)
            }
            UiCommandPayload::BuildAndRunDesktopPackage { profile_id } => {
                self.build_and_run_desktop_package(&mut transaction, profile_id)
            }
            UiCommandPayload::BuildReleasePackage { profile_id } => {
                self.build_release_package(&mut transaction, profile_id)
            }
            UiCommandPayload::SaveReleaseProfile => self.save_release_profile(&mut transaction),
            UiCommandPayload::SetReleaseProfileIcon { asset_ref } => {
                self.set_release_profile_icon(&mut transaction, asset_ref)
            }
            UiCommandPayload::OpenBuildOutput => self.open_build_output(&mut transaction),
            UiCommandPayload::OpenBuildReport => self.open_build_report(&mut transaction),
            UiCommandPayload::ClearConsole => self.clear_console(&mut transaction),
            UiCommandPayload::SelectReportEntry { report_id } => {
                self.select_report_entry(transaction, report_id)
            }
            UiCommandPayload::RefreshReports => self.refresh_reports(transaction),
            UiCommandPayload::CopyReportAiContext { report_id } => {
                self.copy_report_ai_context(transaction, report_id)
            }
            UiCommandPayload::OpenRawReport { report_id } => {
                self.open_raw_report(transaction, report_id)
            }
            UiCommandPayload::RevealReportPath { report_id } => {
                self.reveal_report_path(transaction, report_id)
            }
            UiCommandPayload::OpenRelatedReportArtifact {
                report_id,
                artifact_id,
            } => self.open_related_report_artifact(transaction, report_id, artifact_id),
            UiCommandPayload::SelectTraceEntry { entry_id } => {
                self.select_trace_entry(&mut transaction, &entry_id)
            }
            UiCommandPayload::AiSubmitPrompt { prompt } => {
                self.submit_ai_prompt(&mut transaction, prompt)
            }
            UiCommandPayload::GenerateProjectPatchFromPrompt { prompt } => {
                self.generate_project_patch_from_prompt(&mut transaction, prompt)
            }
            UiCommandPayload::SetAiPromptDraft { prompt } => {
                self.set_ai_prompt_draft(&mut transaction, prompt)
            }
            UiCommandPayload::CancelLlmPatchRequest => {
                self.cancel_llm_patch_request(&mut transaction)
            }
            UiCommandPayload::ImportProjectPatch {
                source_label,
                raw_json,
                file_path,
                expected_patch_id,
                dry_run,
            } => self.import_project_patch(
                &mut transaction,
                source_label,
                raw_json,
                file_path,
                expected_patch_id,
                dry_run,
            ),
            UiCommandPayload::PreviewImportedProjectPatch {
                source_label,
                raw_json,
                file_path,
                expected_patch_id,
            } => self.preview_imported_project_patch(
                &mut transaction,
                source_label,
                raw_json,
                file_path,
                expected_patch_id,
            ),
            UiCommandPayload::ApplyImportedProjectPatch { proposal_id } => {
                self.apply_imported_project_patch(&mut transaction, &proposal_id)
            }
            UiCommandPayload::ParkProjectWorkItem { work_item_id } => {
                self.park_project_work_item(&mut transaction, work_item_id)
            }
            UiCommandPayload::ResumeProjectWorkItem { work_item_id } => {
                self.resume_project_work_item(&mut transaction, work_item_id)
            }
            UiCommandPayload::ReopenProjectWorkItem { work_item_id } => {
                self.reopen_project_work_item(&mut transaction, work_item_id)
            }
            UiCommandPayload::ApproveProjectChange { proposal_digest } => {
                self.approve_project_change(&mut transaction, proposal_digest)
            }
            UiCommandPayload::AdvanceProjectProduction { run_id } => {
                self.advance_project_production(&mut transaction, run_id)
            }
            UiCommandPayload::CancelProjectProduction { run_id } => {
                self.cancel_project_production(&mut transaction, run_id)
            }
            UiCommandPayload::RecoverProjectProduction { run_id } => {
                self.recover_project_production(&mut transaction, run_id)
            }
            UiCommandPayload::ApproveGatewayAccessRequest { .. }
            | UiCommandPayload::RejectGatewayAccessRequest { .. }
            | UiCommandPayload::SetGatewayAccessPage { .. }
            | UiCommandPayload::ApproveProjectRuntimeTrust { .. }
            | UiCommandPayload::DenyProjectRuntimeTrust { .. }
            | UiCommandPayload::CancelProjectRuntimeTrust { .. } => {
                self.push_error(
                    &mut transaction,
                    "editor.command.native_host_decision_required",
                    "Security decision commands must be handled by the Native Editor host.",
                    Some("Review the active Native Editor decision prompt and retry there."),
                );
                self.finish_transaction(transaction, CommandStatus::Rejected)
            }
            UiCommandPayload::AiAcceptProposedCommand { proposal_id } => {
                self.accept_ai_proposed_command(&mut transaction, &proposal_id)
            }
            UiCommandPayload::AiRejectProposedCommand { proposal_id } => {
                self.reject_ai_proposed_command(&mut transaction, &proposal_id)
            }
        };
        result
    }

    pub fn open_scene_document_for_test(&mut self, path: impl AsRef<Path>) -> CommandResult {
        self.execute_command(UiCommand {
            command_id: "open_scene_document".to_string(),
            source: editor_ui_model::UiCommandSource::Test,
            request_id: "request-open-scene-document".to_string(),
            payload: UiCommandPayload::OpenSceneDocument {
                path: path.as_ref().display().to_string(),
            },
        })
    }

    pub fn execute_scene_edit_for_test(&mut self, command: SceneEditCommand) -> CommandResult {
        let command_id = format!("scene_edit_{}", command.kind());
        let ui_command = UiCommand {
            command_id,
            source: editor_ui_model::UiCommandSource::Test,
            request_id: format!("request-scene-edit-{}", self.transaction_counter + 1),
            payload: UiCommandPayload::ClearConsole,
        };
        let mut transaction = self.begin_transaction(ui_command);
        self.execute_scene_edit(&mut transaction, command)
    }

    pub fn save_scene_document_for_test(&mut self, path: Option<PathBuf>) -> CommandResult {
        self.execute_command(UiCommand {
            command_id: "save_scene_document".to_string(),
            source: editor_ui_model::UiCommandSource::Test,
            request_id: format!("request-save-scene-{}", self.transaction_counter + 1),
            payload: UiCommandPayload::SaveSceneDocument {
                path: path.map(|path| path.display().to_string()),
            },
        })
    }

    pub fn scene_dirty(&self) -> Option<bool> {
        self.editor_scene_document
            .as_ref()
            .map(|document| document.dirty_state.dirty)
    }

    pub fn editor_scene_document(&self) -> Option<&EditorSceneDocument> {
        self.editor_scene_document.as_ref()
    }

    pub fn build_property_tree_for_selected_entity(
        &self,
        registry: &ComponentSchemaRegistry,
    ) -> Option<PropertyTreeBuildResult> {
        let document = self.editor_scene_document.as_ref()?;
        let selected = self.scene_selection.primary_entity_id.as_ref()?;
        let entity = document.entity(selected)?;
        let source = InspectorSourceData::from_scene_entity(entity);
        Some(PropertyTreeBuilder::build(&source, registry))
    }

    pub fn active_project_session(&self) -> Option<&ProjectSession> {
        self.active_project_session.as_ref()
    }

    pub fn project_intent_workflow(&self) -> &ProjectIntentWorkflow {
        &self.project_intent_workflow
    }

    pub fn project_intent_snapshot(
        &self,
    ) -> Result<crate::ProjectIntentSnapshot, crate::ProjectIntentWorkflowError> {
        self.project_intent_workflow
            .observe(crate::ProjectIntentQuery::All)
    }

    pub fn capture_project_intent(
        &mut self,
        input: crate::IntentCaptureInput,
    ) -> Result<crate::IntentCaptureReceipt, crate::ProjectIntentWorkflowError> {
        self.project_intent_workflow.capture(input)
    }

    pub fn prepare_project_change(
        &mut self,
        request: crate::ChangePreparationRequest,
    ) -> Result<crate::ChangePreparationResult, crate::ProjectIntentWorkflowError> {
        self.project_intent_workflow.prepare_change(request)
    }

    pub fn authorize_project_change(
        &mut self,
        input: crate::ChangeSetApprovalInput,
    ) -> Result<crate::ProjectProductionRun, crate::ProjectIntentWorkflowError> {
        let mut workflow = std::mem::take(&mut self.project_intent_workflow);
        let result = workflow.authorize(input, Some(self));
        self.project_intent_workflow = workflow;
        result
    }

    pub fn dispatch_project_intent(
        &mut self,
        command: crate::ProjectIntentWorkflowCommand,
    ) -> Result<crate::ProjectIntentSnapshot, crate::ProjectIntentWorkflowError> {
        let mut workflow = std::mem::take(&mut self.project_intent_workflow);
        let result = workflow.dispatch(command, Some(self));
        self.project_intent_workflow = workflow;
        result
    }

    pub(crate) fn reload_project_intent_workflow(
        &mut self,
    ) -> Result<(), crate::ProjectIntentWorkflowError> {
        self.project_intent_workflow = ProjectIntentWorkflow::open_project(self)?;
        Ok(())
    }

    pub fn project_launcher_state(&self) -> &ProjectLauncherState {
        &self.project_launcher
    }

    pub fn load_recent_projects_for_launcher(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<bool, String> {
        self.project_launcher.load_recent_projects(path)
    }

    pub fn save_recent_projects_for_launcher(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), String> {
        self.project_launcher.save_recent_projects(path)
    }

    pub fn last_play_session_report(&self) -> Option<&PlaySessionReport> {
        self.last_play_session_report.as_ref()
    }

    pub fn last_editor_preview_package_report(&self) -> Option<&EditorPlayPreviewPackageReport> {
        self.last_editor_preview_package_report.as_ref()
    }

    pub fn last_game_view_runtime_frame(&self) -> Option<&GameViewRuntimeFrame> {
        self.last_game_view_runtime_frame.as_ref()
    }

    pub fn active_game_view_aui_action_targets(&self) -> &[crate::GameViewAuiActionTarget] {
        self.editor_runtime_play_instance
            .as_ref()
            .map(EditorRuntimePlayInstance::last_aui_action_targets)
            .unwrap_or_default()
    }

    pub fn set_active_game_view_project_runtime_report_level(
        &mut self,
        level: engine_runtime::project_runtime_session::ProjectRuntimeSessionReportLevel,
    ) -> bool {
        let Some(instance) = self.editor_runtime_play_instance.as_mut() else {
            return false;
        };
        instance.set_project_runtime_session_report_level(level);
        true
    }

    pub fn last_game_view_present_report(&self) -> Option<&GameViewPresentReport> {
        self.last_game_view_present_report.as_ref()
    }

    pub fn last_game_view_project_observation_state(
        &self,
    ) -> Option<&engine_runtime::project_observation::ProjectRuntimeObservationState> {
        self.last_game_view_present_report
            .as_ref()
            .and_then(|report| report.project_observation_state.as_ref())
    }

    pub fn pending_project_preview_frame_ticket(&self) -> Option<&ProjectPreviewFrameTicket> {
        self.pending_project_preview_frame_ticket.as_ref()
    }

    pub fn project_preview_frame_result(&self) -> Option<&ProjectPreviewFrameResult> {
        self.project_preview_frame_result.as_ref()
    }

    pub fn begin_project_preview_frame_capture(
        &mut self,
        ticket: ProjectPreviewFrameTicket,
    ) -> Result<(), ProjectPreviewEvidenceError> {
        ProjectPreviewEvidence::validate_ticket(&ticket)?;
        if let Some(pending) = &self.pending_project_preview_frame_ticket {
            if pending.operation_id != ticket.operation_id {
                return Err(ProjectPreviewEvidenceError::new(
                    "project_preview_evidence.capture_already_pending",
                    format!(
                        "Preview operation '{}' still owns the pending frame capture.",
                        pending.operation_id
                    ),
                ));
            }
        }
        self.pending_project_preview_frame_ticket = Some(ticket);
        self.project_preview_frame_result = None;
        Ok(())
    }

    pub fn record_project_preview_presented_frame(
        &mut self,
        readback: ProjectPreviewFrameReadback,
    ) -> Result<ProjectPreviewFrameEvidence, ProjectPreviewEvidenceError> {
        let ticket = self
            .pending_project_preview_frame_ticket
            .clone()
            .ok_or_else(|| {
                ProjectPreviewEvidenceError::new(
                    "project_preview_evidence.ticket_missing",
                    "No Preview operation is waiting for a presented frame.",
                )
            })?;
        let project = self.active_project_session.as_ref().ok_or_else(|| {
            ProjectPreviewEvidenceError::new(
                "project_preview_evidence.project_missing",
                "The Preview project closed before frame evidence was captured.",
            )
        })?;
        let present_report_path = self
            .last_game_view_present_report
            .as_ref()
            .and_then(|report| report.report_path.as_deref())
            .ok_or_else(|| {
                ProjectPreviewEvidenceError::new(
                    "project_preview_evidence.present_report_missing",
                    "The presented GameView frame has no report path.",
                )
            })?;
        let present_report_ref = Path::new(present_report_path)
            .strip_prefix(&project.project_root)
            .map_err(|_| {
                ProjectPreviewEvidenceError::new(
                    "project_preview_evidence.present_report_outside_project",
                    "The GameView present report is outside the active project root.",
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let capture = ProjectPreviewFrameCapture {
            project_digest: ticket.expected_project_digest.clone(),
            game_view_session_id: readback.game_view_session_id,
            texture_id: readback.texture_id,
            frame_index: readback.frame_index,
            runtime_frame_hash: ticket.expected_runtime_frame_hash.clone(),
            width: readback.width,
            height: readback.height,
            pixel_format: readback.pixel_format,
            capture_kind: readback.capture_kind,
            present_report_ref,
            rgba8: readback.rgba8,
        };
        match ProjectPreviewEvidence::persist_frame(&project.write_scope(), &ticket, capture) {
            Ok(evidence) => {
                self.project_preview_frame_result = Some(ProjectPreviewFrameResult::captured(
                    ProjectPreviewEvidence::frame_evidence_ref(&evidence.operation_id)?,
                    evidence.clone(),
                ));
                Ok(evidence)
            }
            Err(error) => {
                self.project_preview_frame_result = Some(ProjectPreviewFrameResult::failed(
                    ticket.operation_id,
                    error.code,
                    error.message.clone(),
                ));
                Err(error)
            }
        }
    }

    pub fn fail_project_preview_frame_capture(
        &mut self,
        operation_id: &str,
        diagnostic_code: impl Into<String>,
        diagnostic_message: impl Into<String>,
    ) -> bool {
        if !self
            .pending_project_preview_frame_ticket
            .as_ref()
            .is_some_and(|ticket| ticket.operation_id == operation_id)
        {
            return false;
        }
        self.project_preview_frame_result = Some(ProjectPreviewFrameResult::failed(
            operation_id,
            diagnostic_code,
            diagnostic_message,
        ));
        true
    }

    pub fn complete_project_preview_frame_capture(&mut self, operation_id: &str) -> bool {
        if !self
            .pending_project_preview_frame_ticket
            .as_ref()
            .is_some_and(|ticket| ticket.operation_id == operation_id)
        {
            return false;
        }
        self.pending_project_preview_frame_ticket = None;
        true
    }

    pub fn discard_project_preview_frame_capture(&mut self, operation_id: &str) -> bool {
        if !self
            .pending_project_preview_frame_ticket
            .as_ref()
            .is_some_and(|ticket| ticket.operation_id == operation_id)
        {
            return false;
        }
        self.pending_project_preview_frame_ticket = None;
        self.project_preview_frame_result = None;
        true
    }

    pub(crate) fn reset_project_preview_frame_state(&mut self) {
        self.pending_project_preview_frame_ticket = None;
        self.project_preview_frame_result = None;
    }

    pub fn last_runtime_apply_report(&self) -> Option<&ApplyRuntimeChangeReport> {
        self.last_runtime_apply_report.as_ref()
    }

    pub fn last_build_and_run_report(&self) -> Option<&EditorBuildAndRunReport> {
        self.last_build_and_run_report.as_ref()
    }

    pub fn last_release_package_report(&self) -> Option<&ReleasePackageReport> {
        self.last_release_package_report.as_ref()
    }

    pub fn has_active_editor_runtime_play_instance(&self) -> bool {
        self.editor_runtime_play_instance.is_some()
    }

    pub fn has_prepared_editor_play_report(&self) -> bool {
        self.prepared_editor_play_report.is_some()
    }

    pub fn tick_active_game_view_runtime_descriptor_frame(
        &mut self,
    ) -> Option<GameViewPresentReport> {
        let instance = self.editor_runtime_play_instance.as_mut()?;
        let report = instance.tick_next_descriptor_frame();
        self.last_game_view_runtime_frame = report.last_frame.clone();
        self.sync_animator2d_play_observations();
        self.last_game_view_present_report = Some(report.clone());
        Some(report)
    }

    pub fn tick_active_game_view_runtime_descriptor_frame_with_input(
        &mut self,
        runtime_input_frame: RuntimeInputFrame,
    ) -> Option<GameViewPresentReport> {
        let instance = self.editor_runtime_play_instance.as_mut()?;
        let report = instance.tick_next_descriptor_frame_with_runtime_input(runtime_input_frame);
        self.last_game_view_runtime_frame = report.last_frame.clone();
        self.sync_animator2d_play_observations();
        self.last_game_view_present_report = Some(report.clone());
        Some(report)
    }

    pub fn route_active_game_view_aui_input(
        &mut self,
        runtime_input_frame: RuntimeInputFrame,
    ) -> Option<GameViewPresentReport> {
        let instance = self.editor_runtime_play_instance.as_mut()?;
        let report = instance.route_aui_input_immediately(runtime_input_frame);
        self.last_game_view_runtime_frame = report.last_frame.clone();
        self.last_game_view_present_report = Some(report.clone());
        Some(report)
    }

    pub fn cancel_active_game_view_input(&mut self) {
        if let Some(instance) = self.editor_runtime_play_instance.as_mut() {
            instance.cancel_pending_game_view_input();
        }
    }

    pub fn active_game_view_rhi_command_plan(&self) -> Option<&RhiCommandPlan> {
        self.editor_runtime_play_instance
            .as_ref()
            .and_then(EditorRuntimePlayInstance::last_rhi_command_plan)
    }

    pub fn active_game_view_font_bundles(
        &self,
    ) -> Option<&engine_runtime::font_bundle::RuntimeFontBundleRegistry> {
        self.editor_runtime_play_instance
            .as_ref()
            .map(EditorRuntimePlayInstance::font_bundles)
    }

    pub fn active_game_view_runtime_texture_uploads(
        &self,
    ) -> Option<&engine_runtime::runtime_texture::RuntimeTextureUploadRegistry> {
        self.editor_runtime_play_instance
            .as_ref()
            .map(EditorRuntimePlayInstance::runtime_texture_uploads)
    }

    pub fn mark_active_game_view_gpu_present_result(
        &mut self,
        gpu_present_status: impl Into<String>,
        shared_gpu_context_status: impl Into<String>,
        diagnostics: Vec<GameViewPresentDiagnostic>,
    ) -> Option<GameViewPresentReport> {
        let instance = self.editor_runtime_play_instance.as_mut()?;
        let report = instance.apply_gpu_present_result(
            gpu_present_status,
            shared_gpu_context_status,
            diagnostics,
        );
        self.last_game_view_runtime_frame = report.last_frame.clone();
        self.last_game_view_present_report = Some(report.clone());
        Some(report)
    }

    pub(crate) fn begin_transaction(&mut self, command: UiCommand) -> CommandTransaction {
        self.transaction_counter += 1;
        CommandTransaction {
            transaction_id: format!("tx-{}", self.transaction_counter),
            request_id: command.request_id,
            command_id: command.command_id,
            source: format!("{:?}", command.source),
            payload: command.payload,
            status: CommandStatus::Pending,
            read_set: Vec::new(),
            write_set: Vec::new(),
            diagnostics: Vec::new(),
            state_changes: Vec::new(),
            undo_policy: UndoPolicy::None,
        }
    }

    pub(crate) fn reject_unknown_editor_command(
        &mut self,
        request_id: String,
        source: editor_ui_model::UiCommandSource,
        payload: UiCommandPayload,
    ) -> CommandResult {
        let mut transaction = self.begin_transaction(UiCommand {
            command_id: command_id_for_payload(&payload).to_string(),
            source,
            request_id,
            payload,
        });
        self.push_error(
            &mut transaction,
            "editor.command.unknown",
            "Editor command is not registered in EditorCommandRegistry.",
            Some("Register the command descriptor before routing it to EditorCommandExecutor."),
        );
        self.finish_transaction(transaction, CommandStatus::Rejected)
    }

    pub(crate) fn finish_transaction(
        &mut self,
        mut transaction: CommandTransaction,
        status: CommandStatus,
    ) -> CommandResult {
        transaction.status = status;
        let console_entries = transaction
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic_to_console_entry(diagnostic, self.console_entries.len()))
            .collect::<Vec<_>>();
        self.console_entries.extend(console_entries.clone());
        self.diagnostics.extend(transaction.diagnostics.clone());
        if status == CommandStatus::Committed {
            self.revision += 1;
            if transaction.write_set.iter().any(|write| {
                [
                    "asset.generated",
                    "input_mapping.",
                    "rule_asset.",
                    "prefab_asset.",
                    "aui_document.",
                ]
                .iter()
                .any(|prefix| write.starts_with(prefix))
            }) {
                self.refresh_asset_browser_now(format!("transaction:{}", transaction.command_id));
            }
        }
        CommandResult {
            transaction_id: transaction.transaction_id,
            request_id: transaction.request_id,
            command_id: transaction.command_id,
            status,
            diagnostics: transaction.diagnostics,
            console_entries,
            state_changes: transaction.state_changes,
            ui_model_revision: self.revision,
        }
    }

    pub(crate) fn push_info(
        &self,
        transaction: &mut CommandTransaction,
        code: &str,
        message: impl Into<String>,
    ) {
        transaction.diagnostics.push(self.make_diagnostic(
            transaction,
            DiagnosticSeverity::Info,
            code,
            message,
            None,
        ));
    }

    pub(crate) fn push_error(
        &self,
        transaction: &mut CommandTransaction,
        code: &str,
        message: impl Into<String>,
        suggested_action: Option<&str>,
    ) {
        transaction.diagnostics.push(self.make_diagnostic(
            transaction,
            DiagnosticSeverity::Error,
            code,
            message,
            suggested_action,
        ));
    }

    pub(crate) fn push_warning(
        &self,
        transaction: &mut CommandTransaction,
        code: &str,
        message: impl Into<String>,
        suggested_action: Option<&str>,
    ) {
        transaction.diagnostics.push(self.make_diagnostic(
            transaction,
            DiagnosticSeverity::Warning,
            code,
            message,
            suggested_action,
        ));
    }

    pub(crate) fn push_play_session_report(
        &self,
        transaction: &mut CommandTransaction,
        report: &PlaySessionReport,
    ) {
        if report.state == PlaySessionState::Failed {
            let first = report.diagnostics.first();
            let layer = first.map_or("session", |diagnostic| diagnostic.layer.as_str());
            let code = first.map_or("editor.play_session.failed", |diagnostic| {
                diagnostic.code.as_str()
            });
            let message = first.map_or_else(
                || {
                    format!(
                        "Play session failed: {} layer={layer} code={code}",
                        report.session_id
                    )
                },
                |diagnostic| {
                    format!(
                        "Play session failed: {} layer={} code={} message={}",
                        report.session_id, diagnostic.layer, diagnostic.code, diagnostic.message
                    )
                },
            );
            self.push_error(
                transaction,
                code,
                message,
                Some("Open RuntimeTrace or report details."),
            );
            return;
        }
        let frames_completed = report
            .runtime_report
            .as_ref()
            .map(|runtime| runtime.frames_completed)
            .or(report.game_view_frame_count)
            .unwrap_or(0);
        let status_label = if report.state == PlaySessionState::Running {
            "running"
        } else {
            "completed"
        };
        self.push_info(
            transaction,
            "editor.play_session.completed",
            format!(
                "Play session {status_label}: {} frames_completed={frames_completed} runner={}",
                report.session_id,
                report.runner_kind.as_deref().unwrap_or("default")
            ),
        );
    }

    pub(crate) fn make_diagnostic(
        &self,
        transaction: &CommandTransaction,
        severity: DiagnosticSeverity,
        code: &str,
        message: impl Into<String>,
        suggested_action: Option<&str>,
    ) -> EditorDiagnostic {
        EditorDiagnostic {
            severity,
            code: code.to_string(),
            message: message.into(),
            source: DiagnosticSource::Command,
            command_id: Some(transaction.command_id.clone()),
            request_id: Some(transaction.request_id.clone()),
            path: None,
            entity_id: None,
            trace_entry_id: None,
            suggested_action: suggested_action.map(str::to_string),
        }
    }
}

fn diagnostic_to_console_entry(diagnostic: &EditorDiagnostic, index: usize) -> ConsoleEntry {
    ConsoleEntry {
        entry_id: format!("console-{}", index + 1),
        level: match diagnostic.severity {
            DiagnosticSeverity::Info => ConsoleLevel::Info,
            DiagnosticSeverity::Warning => ConsoleLevel::Warning,
            DiagnosticSeverity::Error => ConsoleLevel::Error,
        },
        source: match diagnostic.source {
            DiagnosticSource::Runtime | DiagnosticSource::RuntimePackage => ConsoleSource::Runtime,
            DiagnosticSource::Command => ConsoleSource::Command,
            DiagnosticSource::UiBackend | DiagnosticSource::EditorCore => ConsoleSource::Editor,
        },
        message: diagnostic.message.clone(),
        frame: None,
        timestamp_ms: None,
    }
}
