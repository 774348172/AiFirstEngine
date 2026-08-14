use crate::transaction::EditorTransactionService;
use editor_core::{command_id_for_payload, CommandResult, CommandStatus, EditorSession};
use editor_ui_model::{
    EditorCommandFeedback, EditorCommandFeedbackStatus, UiCommand, UiCommandPayload,
    UiCommandSource,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCommandDescriptor {
    pub command_id: String,
    pub title: String,
    pub undoable: bool,
}

pub struct EditorCommandSystem {
    pub(crate) commands: BTreeMap<String, EditorCommandDescriptor>,
    request_counter: u64,
}

impl Default for EditorCommandSystem {
    fn default() -> Self {
        Self::standard_editor()
    }
}

impl EditorCommandSystem {
    pub fn standard_editor() -> Self {
        let mut system = Self {
            commands: BTreeMap::new(),
            request_counter: 0,
        };
        for (id, title, undoable) in [
            ("select_scene_entity", "Select Scene Entity", false),
            ("select_runtime_entity", "Select Runtime Entity", false),
            (
                "set_runtime_component_field_temporary",
                "Set Runtime Component Field Temporary",
                false,
            ),
            ("open_project", "Open Project", false),
            ("create_project", "Create Project", false),
            ("select_recent_project", "Select Recent Project", false),
            ("refresh_recent_projects", "Refresh Recent Projects", false),
            (
                "select_project_browser_entry",
                "Select Project Browser Entry",
                false,
            ),
            (
                "open_project_browser_entry",
                "Open Project Browser Entry",
                false,
            ),
            (
                "select_asset_browser_entry",
                "Select Asset Browser Entry",
                false,
            ),
            (
                "open_asset_browser_entry",
                "Open Asset Browser Entry",
                false,
            ),
            (
                "set_asset_browser_folder",
                "Set Asset Browser Folder",
                false,
            ),
            (
                "set_asset_browser_search",
                "Set Asset Browser Search",
                false,
            ),
            (
                "set_asset_browser_kind_filter",
                "Set Asset Browser Kind Filter",
                false,
            ),
            ("asset_browser_toolbar", "Asset Browser Toolbar", false),
            ("scroll_asset_browser", "Scroll Asset Browser", false),
            ("begin_asset_pick", "Begin Asset Pick", false),
            ("confirm_asset_pick", "Confirm Asset Pick", true),
            ("cancel_asset_pick", "Cancel Asset Pick", false),
            (
                "drop_asset_on_inspector_field",
                "Drop Asset On Inspector Field",
                true,
            ),
            (
                "create_default_input_mapping",
                "Create Default Input Mapping",
                false,
            ),
            ("open_input_mapping", "Open Input Mapping", false),
            ("save_input_mapping", "Save Input Mapping", false),
            (
                "discard_input_mapping_draft",
                "Discard Input Mapping Draft",
                false,
            ),
            ("validate_input_mapping", "Validate Input Mapping", false),
            ("select_input_context", "Select Input Context", false),
            ("select_input_action", "Select Input Action", false),
            ("select_input_binding", "Select Input Binding", false),
            ("add_input_context", "Add Input Context", true),
            ("remove_input_context", "Remove Input Context", true),
            (
                "set_input_context_priority",
                "Set Input Context Priority",
                true,
            ),
            (
                "set_input_context_consume_input",
                "Set Input Context Consume Input",
                true,
            ),
            ("add_input_action", "Add Input Action", true),
            ("remove_input_action", "Remove Input Action", true),
            (
                "set_input_action_value_type",
                "Set Input Action Value Type",
                true,
            ),
            ("add_input_binding", "Add Input Binding", true),
            ("remove_input_binding", "Remove Input Binding", true),
            (
                "set_input_binding_device_path",
                "Set Input Binding Device Path",
                true,
            ),
            (
                "remove_input_binding_by_id",
                "Remove Input Binding By Id",
                true,
            ),
            (
                "set_input_binding_device_path_by_id",
                "Set Input Binding Device Path By Id",
                true,
            ),
            (
                "set_input_binding_trigger",
                "Set Input Binding Trigger",
                true,
            ),
            (
                "set_input_binding_processor",
                "Set Input Binding Processor",
                true,
            ),
            (
                "begin_input_binding_capture",
                "Begin Input Binding Capture",
                false,
            ),
            (
                "cancel_input_binding_capture",
                "Cancel Input Binding Capture",
                false,
            ),
            (
                "commit_captured_input_binding",
                "Commit Captured Input Binding",
                true,
            ),
            ("preview_input_mapping", "Preview Input Mapping", false),
            (
                "set_input_mapping_report_level",
                "Set Input Mapping Report Level",
                false,
            ),
            ("set_workspace_view_mode", "Set Workspace View Mode", false),
            ("create_scene_entity", "Create Scene Entity", true),
            ("delete_scene_entity", "Delete Scene Entity", true),
            ("rename_scene_entity", "Rename Scene Entity", true),
            ("set_scene_transform", "Set Scene Transform", true),
            (
                "set_scene_component_field",
                "Set Scene Component Field",
                true,
            ),
            ("save_scene_document", "Save Scene Document", false),
            ("undo_scene_edit", "Undo Scene Edit", false),
            ("redo_scene_edit", "Redo Scene Edit", false),
            ("ai_submit_prompt", "AI Submit Prompt", false),
            (
                "ai_accept_proposed_command",
                "AI Accept Proposed Command",
                true,
            ),
            (
                "ai_reject_proposed_command",
                "AI Reject Proposed Command",
                false,
            ),
            ("clear_console", "Clear Console", false),
            ("play", "Play", false),
            ("pause", "Pause", false),
            ("step_frame", "Step Frame", false),
            ("stop_play_session", "Stop Play Session", false),
            ("set_game_view_target", "Set GameView Target", false),
            (
                "set_game_view_maximize_on_play",
                "Set GameView Maximize on Play",
                false,
            ),
            (
                "toggle_game_view_maximize_on_play",
                "Toggle GameView Maximize on Play",
                false,
            ),
            ("tick_one_frame", "Tick One Frame", false),
            ("export_desktop_package", "Export Desktop Package", false),
            (
                "build_and_run_desktop_package",
                "Build And Run Desktop Package",
                false,
            ),
            ("build_release_package", "Build Release Package", false),
            ("save_release_profile", "Save Release Profile", false),
            ("set_release_profile_icon", "Set Release Profile Icon", true),
            ("open_build_output", "Open Build Output", false),
            ("open_build_report", "Open Build Report", false),
        ] {
            system.register(id, title, undoable);
        }
        system
    }

    pub fn register(&mut self, command_id: &str, title: &str, undoable: bool) {
        self.commands.insert(
            command_id.to_string(),
            EditorCommandDescriptor {
                command_id: command_id.to_string(),
                title: title.to_string(),
                undoable,
            },
        );
    }

    pub fn contains(&self, command_id: &str) -> bool {
        self.commands.contains_key(command_id)
    }

    pub fn count(&self) -> usize {
        self.commands.len()
    }

    pub fn shortcut_command(&mut self, shortcut: &str) -> Option<UiCommand> {
        let payload = match shortcut {
            "Ctrl+Z" => UiCommandPayload::UndoSceneEdit,
            "Ctrl+Y" | "Ctrl+Shift+Z" => UiCommandPayload::RedoSceneEdit,
            _ => return None,
        };
        Some(self.command("shortcut", UiCommandSource::Toolbar, payload))
    }

    pub fn dispatch(
        &mut self,
        command: UiCommand,
        session: &mut EditorSession,
        transaction_service: &mut EditorTransactionService,
    ) -> CommandResult {
        let normalized = self.normalize_command(command);
        let result = session.execute_command(normalized);
        transaction_service.record(&result);
        result
    }

    fn normalize_command(&self, command: UiCommand) -> UiCommand {
        match (&command.source, &command.payload) {
            (UiCommandSource::Hierarchy, UiCommandPayload::SelectEntity { entity_id }) => {
                UiCommand {
                    command_id: "select_scene_entity".to_string(),
                    source: UiCommandSource::Hierarchy,
                    request_id: command.request_id,
                    payload: UiCommandPayload::SelectSceneEntity {
                        entity_id: entity_id.clone(),
                    },
                }
            }
            _ => command,
        }
    }

    fn command(
        &mut self,
        request_prefix: &str,
        source: UiCommandSource,
        payload: UiCommandPayload,
    ) -> UiCommand {
        self.request_counter += 1;
        UiCommand {
            command_id: command_id_for_shell_payload(&payload).to_string(),
            source,
            request_id: format!("{request_prefix}-{}", self.request_counter),
            payload,
        }
    }
}

pub(crate) fn command_id_for_shell_payload(payload: &UiCommandPayload) -> &'static str {
    match payload {
        UiCommandPayload::OpenProject { .. } => "open_project",
        UiCommandPayload::CreateProject { .. } => "create_project",
        UiCommandPayload::SelectRecentProject { .. } => "select_recent_project",
        UiCommandPayload::RefreshRecentProjects => "refresh_recent_projects",
        UiCommandPayload::SelectProjectBrowserEntry { .. } => "select_project_browser_entry",
        UiCommandPayload::OpenProjectBrowserEntry { .. } => "open_project_browser_entry",
        UiCommandPayload::SelectAssetBrowserEntry { .. } => "select_asset_browser_entry",
        UiCommandPayload::OpenAssetBrowserEntry { .. } => "open_asset_browser_entry",
        UiCommandPayload::SetAssetBrowserFolder { .. } => "set_asset_browser_folder",
        UiCommandPayload::SetAssetBrowserSearch { .. } => "set_asset_browser_search",
        UiCommandPayload::SetAssetBrowserKindFilter { .. } => "set_asset_browser_kind_filter",
        UiCommandPayload::AssetBrowserToolbar { .. } => "asset_browser_toolbar",
        UiCommandPayload::ScrollAssetBrowser { .. } => "scroll_asset_browser",
        UiCommandPayload::CreateDefaultInputMapping { .. } => "create_default_input_mapping",
        UiCommandPayload::OpenInputMapping { .. } => "open_input_mapping",
        UiCommandPayload::SaveInputMapping { .. } => "save_input_mapping",
        UiCommandPayload::DiscardInputMappingDraft { .. } => "discard_input_mapping_draft",
        UiCommandPayload::ValidateInputMapping { .. } => "validate_input_mapping",
        UiCommandPayload::SelectInputContext { .. } => "select_input_context",
        UiCommandPayload::SelectInputAction { .. } => "select_input_action",
        UiCommandPayload::SelectInputBinding { .. } => "select_input_binding",
        UiCommandPayload::AddInputContext { .. } => "add_input_context",
        UiCommandPayload::RemoveInputContext { .. } => "remove_input_context",
        UiCommandPayload::SetInputContextPriority { .. } => "set_input_context_priority",
        UiCommandPayload::SetInputContextConsumeInput { .. } => "set_input_context_consume_input",
        UiCommandPayload::AddInputAction { .. } => "add_input_action",
        UiCommandPayload::RemoveInputAction { .. } => "remove_input_action",
        UiCommandPayload::SetInputActionValueType { .. } => "set_input_action_value_type",
        UiCommandPayload::AddInputBinding { .. } => "add_input_binding",
        UiCommandPayload::RemoveInputBinding { .. } => "remove_input_binding",
        UiCommandPayload::SetInputBindingDevicePath { .. } => "set_input_binding_device_path",
        UiCommandPayload::RemoveInputBindingById { .. } => "remove_input_binding_by_id",
        UiCommandPayload::SetInputBindingDevicePathById { .. } => {
            "set_input_binding_device_path_by_id"
        }
        UiCommandPayload::SetInputBindingTrigger { .. } => "set_input_binding_trigger",
        UiCommandPayload::SetInputBindingProcessor { .. } => "set_input_binding_processor",
        UiCommandPayload::BeginInputBindingCapture { .. } => "begin_input_binding_capture",
        UiCommandPayload::CancelInputBindingCapture { .. } => "cancel_input_binding_capture",
        UiCommandPayload::CommitCapturedInputBinding { .. } => "commit_captured_input_binding",
        UiCommandPayload::PreviewInputMapping { .. } => "preview_input_mapping",
        UiCommandPayload::SetInputMappingReportLevel { .. } => "set_input_mapping_report_level",
        UiCommandPayload::SetWorkspaceViewMode { .. } => "set_workspace_view_mode",
        UiCommandPayload::SetAuthoringWorkflowStep { .. } => "set_authoring_workflow_step",
        UiCommandPayload::UndoSceneEdit => "undo_scene_edit",
        UiCommandPayload::RedoSceneEdit => "redo_scene_edit",
        UiCommandPayload::SelectSceneEntity { .. } => "select_scene_entity",
        UiCommandPayload::CreateSceneEntity { .. } => "create_scene_entity",
        UiCommandPayload::DeleteSceneEntity { .. } => "delete_scene_entity",
        UiCommandPayload::RenameSceneEntity { .. } => "rename_scene_entity",
        UiCommandPayload::SetSceneTransform { .. } => "set_scene_transform",
        UiCommandPayload::SetSceneComponentField { .. } => "set_scene_component_field",
        UiCommandPayload::SelectRuntimeEntity { .. } => "select_runtime_entity",
        UiCommandPayload::SetRuntimeComponentFieldTemporary { .. } => {
            "set_runtime_component_field_temporary"
        }
        UiCommandPayload::SaveSceneDocument { .. } => "save_scene_document",
        UiCommandPayload::AiSubmitPrompt { .. } => "ai_submit_prompt",
        UiCommandPayload::ApproveGatewayAccessRequest { .. } => "approve_gateway_access_request",
        UiCommandPayload::RejectGatewayAccessRequest { .. } => "reject_gateway_access_request",
        UiCommandPayload::SetGatewayAccessPage { .. } => "set_gateway_access_page",
        UiCommandPayload::AiAcceptProposedCommand { .. } => "ai_accept_proposed_command",
        UiCommandPayload::AiRejectProposedCommand { .. } => "ai_reject_proposed_command",
        UiCommandPayload::ClearConsole => "clear_console",
        UiCommandPayload::Play => "play",
        UiCommandPayload::Pause => "pause",
        UiCommandPayload::StepFrame => "step_frame",
        UiCommandPayload::StopPlaySession => "stop_play_session",
        UiCommandPayload::SetGameViewTarget { .. } => "set_game_view_target",
        UiCommandPayload::SetGameViewMaximizeOnPlay { .. } => "set_game_view_maximize_on_play",
        UiCommandPayload::ToggleGameViewMaximizeOnPlay => "toggle_game_view_maximize_on_play",
        UiCommandPayload::TickOneFrame => "tick_one_frame",
        UiCommandPayload::ResetRuntime => "reset_runtime",
        UiCommandPayload::ExportDesktopPackage { .. } => "export_desktop_package",
        UiCommandPayload::BuildAndRunDesktopPackage { .. } => "build_and_run_desktop_package",
        UiCommandPayload::BuildReleasePackage { .. } => "build_release_package",
        UiCommandPayload::SaveReleaseProfile => "save_release_profile",
        UiCommandPayload::SetReleaseProfileIcon { .. } => "set_release_profile_icon",
        UiCommandPayload::OpenBuildOutput => "open_build_output",
        UiCommandPayload::OpenBuildReport => "open_build_report",
        UiCommandPayload::ReloadRuntimePackage => "reload_runtime_package",
        UiCommandPayload::OpenRuntimePackage { .. } => "open_runtime_package",
        UiCommandPayload::OpenSceneDocument { .. } => "open_scene_document",
        UiCommandPayload::SelectEntity { .. } => "select_entity",
        UiCommandPayload::SelectTraceEntry { .. } => "select_trace_entry",
        UiCommandPayload::PlaceAssetIntoScene { .. } => "place_asset_into_scene",
        _ => command_id_for_payload(payload),
    }
}

pub(crate) fn command_feedback_from_result(
    command: &UiCommand,
    result: &CommandResult,
) -> EditorCommandFeedback {
    let status = match result.status {
        CommandStatus::Committed => EditorCommandFeedbackStatus::Committed,
        CommandStatus::Rejected => EditorCommandFeedbackStatus::Rejected,
        CommandStatus::Failed => EditorCommandFeedbackStatus::Failed,
        CommandStatus::Pending | CommandStatus::Validated => EditorCommandFeedbackStatus::Info,
    };
    let reason = result
        .diagnostics
        .last()
        .map(|diagnostic| diagnostic.message.clone());
    let diagnostic_code = result
        .diagnostics
        .last()
        .map(|diagnostic| diagnostic.code.clone());
    let message = reason
        .clone()
        .unwrap_or_else(|| format!("Command {} {:?}.", result.command_id, result.status));
    EditorCommandFeedback {
        command_id: result.command_id.clone(),
        status,
        diagnostic_code,
        message,
        reason,
        source: command.source.clone(),
    }
}
