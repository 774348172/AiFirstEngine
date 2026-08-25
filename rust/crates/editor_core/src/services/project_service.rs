use editor_ui_model::{InputMappingReportLevel, WorkspaceViewMode};
use engine_input::{InputActionValueType, InputControlCatalog};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    scan_input_action_references, CommandResult, CommandStatus, CommandTransaction,
    EditorSceneDocument, EditorSession, InputMappingAuthoringService, InputMappingEditCommand,
    ProjectCreateError, SceneSelection, SceneUndoStack, StateChangeSummary,
};

pub(crate) struct ToolProjectCreateOutcome {
    pub requested_project_root: String,
    pub canonical_project_root: String,
    pub project_name: String,
    pub project_identity: String,
}
pub(crate) fn normalize_project_relative_path(path: &str) -> PathBuf {
    PathBuf::from(path.replace('\\', "/"))
}

pub(crate) fn is_input_mapping_relative_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    lower.starts_with("input/")
        && lower.ends_with(".json")
        && (lower.contains("input") || lower.ends_with(".input-mapping.json"))
}
impl EditorSession {
    pub(crate) fn start_create_project_with_ai(
        &mut self,
        transaction: &mut CommandTransaction,
        draft_path: Option<String>,
    ) -> CommandResult {
        transaction
            .write_set
            .push("project_intent_workflow.pre_project_draft".to_string());
        if self.active_project_session.is_some() {
            self.push_error(
                transaction,
                "project_intent.pre_project_requires_launcher",
                "Create with AI starts from the Project Launcher, not an open project.",
                Some("Close the project or continue its existing intent workflow."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let path = draft_path
            .map(PathBuf::from)
            .unwrap_or_else(default_pre_project_intent_draft_path);
        match crate::ProjectIntentWorkflow::open_pre_project_draft(&path) {
            Ok(workflow) => {
                self.project_intent_workflow = workflow;
                transaction.state_changes.push(StateChangeSummary {
                    kind: "project_intent.pre_project_started".to_string(),
                    path: "project_intent_workflow.pre_project_draft".to_string(),
                    before_summary: None,
                    after_summary: Some(path.display().to_string()),
                });
                self.push_info(
                    transaction,
                    "project_intent.pre_project_started",
                    "Started a local Create with AI draft. No project directory was created.",
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => {
                self.push_error(
                    transaction,
                    &error.code,
                    error.message,
                    Some(&error.next_action),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn create_project(
        &mut self,
        transaction: &mut CommandTransaction,
        path: &Path,
        name: &str,
    ) -> CommandResult {
        transaction.write_set.push("project_session".to_string());
        transaction
            .write_set
            .push("project_launcher.recent_projects".to_string());
        match self.project_launcher.create_project(path, name) {
            Ok(session) => {
                let before = self
                    .active_project_session
                    .as_ref()
                    .map(|session| session.project_root.display().to_string());
                let project_root = session.project_root.display().to_string();
                if let Err(error) = self.adopt_created_project(session) {
                    self.push_error(
                        transaction,
                        &error.code,
                        error.message,
                        Some(&error.next_action),
                    );
                }
                transaction.state_changes.push(StateChangeSummary {
                    kind: "project.created".to_string(),
                    path: "project_session.project_root".to_string(),
                    before_summary: before,
                    after_summary: Some(project_root.clone()),
                });
                self.push_info(
                    transaction,
                    "editor.project.created",
                    format!("Created project {name} at {project_root}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.project.create_failed",
                    message,
                    Some("Choose a writable folder and a non-empty project name."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn create_project_for_ai_tool(
        &mut self,
        path: &Path,
        name: &str,
    ) -> Result<ToolProjectCreateOutcome, ProjectCreateError> {
        if self.active_project_session.is_some() {
            return Err(ProjectCreateError {
                code: "project_create.launcher_required".to_string(),
                message: "project.create is available only in the Editor launcher.".to_string(),
                cleanup_outcome: crate::ProjectCreateCleanupOutcome::NotRequired,
            });
        }
        let outcome = self.project_launcher.create_project_owned(path, name)?;
        let facts = ToolProjectCreateOutcome {
            requested_project_root: outcome.requested_project_root.display().to_string(),
            canonical_project_root: outcome.canonical_project_root.display().to_string(),
            project_name: outcome.session.manifest.project_name.clone(),
            project_identity: outcome.session.manifest.project_id.clone(),
        };
        self.adopt_created_project(outcome.session)
            .map_err(|error| ProjectCreateError {
                code: "project_create.context_transition_failed".to_string(),
                message: error.message,
                cleanup_outcome: crate::ProjectCreateCleanupOutcome::CleanupFailed,
            })?;
        Ok(facts)
    }

    fn adopt_created_project(
        &mut self,
        session: crate::ProjectSession,
    ) -> Result<(), crate::ProjectIntentWorkflowError> {
        let default_scene = session.project_root.join(&session.manifest.default_scene);
        if let Some(target) = session.settings.preferred_game_view_target() {
            self.game_view_target = target;
        }
        self.clear_project_patch_history();
        self.selected_project_browser_path = Some(session.manifest.default_scene.clone());
        self.input_mapping_editor_state = None;
        self.active_project_session = Some(session);
        self.reload_project_intent_workflow()?;
        let _ = self.reload_save_reload_rebuild_report_cache();
        let _ = self.reload_release_profile_cache();
        let _ = self.reload_release_package_report_cache();
        self.initialize_asset_browser();
        if default_scene.exists() {
            let _ = self.open_scene_document_for_launcher(&default_scene);
        }
        Ok(())
    }

    pub(crate) fn open_project(
        &mut self,
        transaction: &mut CommandTransaction,
        path: &Path,
    ) -> CommandResult {
        transaction
            .read_set
            .push("project_manifest.project.aife.json".to_string());
        transaction.write_set.push("project_session".to_string());
        transaction
            .write_set
            .push("project_launcher.recent_projects".to_string());
        match self.project_launcher.open_project(path) {
            Ok(session) => {
                if let Err((code, message)) = self.validate_open_project_composition(&session) {
                    self.push_error(
                        transaction,
                        code,
                        message,
                        Some(
                            "Prepare the project-specific Editor composition and complete handoff.",
                        ),
                    );
                    return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
                }
                let before = self
                    .active_project_session
                    .as_ref()
                    .map(|session| session.project_root.display().to_string());
                let project_root = session.project_root.display().to_string();
                let default_scene = session.project_root.join(&session.manifest.default_scene);
                let game_view_target = session.settings.resolved_game_view_target();
                self.clear_project_patch_history();
                self.selected_project_browser_path = Some(session.manifest.default_scene.clone());
                self.input_mapping_editor_state = None;
                self.active_project_session = Some(session);
                self.game_view_target = game_view_target;
                if let Err(error) = self.reload_project_intent_workflow() {
                    self.push_error(
                        transaction,
                        &error.code,
                        error.message,
                        Some(&error.next_action),
                    );
                }
                let _ = self.reload_save_reload_rebuild_report_cache();
                let _ = self.reload_release_profile_cache();
                let _ = self.reload_release_package_report_cache();
                self.initialize_asset_browser();
                if default_scene.exists() {
                    let _ = self.open_scene_document_for_launcher(&default_scene);
                }
                transaction.state_changes.push(StateChangeSummary {
                    kind: "project.opened".to_string(),
                    path: "project_session.project_root".to_string(),
                    before_summary: before,
                    after_summary: Some(project_root.clone()),
                });
                self.push_info(
                    transaction,
                    "editor.project.opened",
                    format!("Opened project {project_root}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.project.open_failed",
                    message,
                    Some("Select a folder containing project.aife.json."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn select_recent_project(
        &mut self,
        transaction: &mut CommandTransaction,
        path: &Path,
    ) -> CommandResult {
        match self.project_launcher.select_recent_project(path) {
            Ok(session) => {
                if let Err((code, message)) = self.validate_open_project_composition(&session) {
                    self.push_error(
                        transaction,
                        code,
                        message,
                        Some(
                            "Prepare the project-specific Editor composition and complete handoff.",
                        ),
                    );
                    return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
                }
                let project_root = session.project_root.display().to_string();
                let default_scene = session.project_root.join(&session.manifest.default_scene);
                let game_view_target = session.settings.resolved_game_view_target();
                self.clear_project_patch_history();
                self.selected_project_browser_path = Some(session.manifest.default_scene.clone());
                self.input_mapping_editor_state = None;
                self.active_project_session = Some(session);
                self.game_view_target = game_view_target;
                if let Err(error) = self.reload_project_intent_workflow() {
                    self.push_error(
                        transaction,
                        &error.code,
                        error.message,
                        Some(&error.next_action),
                    );
                }
                let _ = self.reload_save_reload_rebuild_report_cache();
                let _ = self.reload_release_profile_cache();
                let _ = self.reload_release_package_report_cache();
                self.initialize_asset_browser();
                if default_scene.exists() {
                    let _ = self.open_scene_document_for_launcher(&default_scene);
                }
                transaction.state_changes.push(StateChangeSummary {
                    kind: "project.recent_selected".to_string(),
                    path: "project_session.project_root".to_string(),
                    before_summary: None,
                    after_summary: Some(project_root.clone()),
                });
                self.push_info(
                    transaction,
                    "editor.project.recent_selected",
                    format!("Opened recent project {project_root}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.project.recent_open_failed",
                    message,
                    Some("Remove the missing project or select its current folder."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    fn clear_project_patch_history(&mut self) {
        self.patch_history.entries.clear();
        self.patch_file_snapshot_history.clear();
        self.reset_project_preview_frame_state();
    }

    pub(crate) fn refresh_recent_projects(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction
            .write_set
            .push("project_launcher.recent_projects".to_string());
        self.project_launcher.refresh_recent_projects();
        self.push_info(
            transaction,
            "editor.project.recent_refreshed",
            "Refreshed recent projects.",
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn select_project_browser_entry(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        transaction
            .write_set
            .push("workspace.selected_asset".to_string());
        let before = self.selected_project_browser_path.clone();
        self.selected_project_browser_path = Some(path.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "project_browser.selected".to_string(),
            path: "workspace.selected_asset".to_string(),
            before_summary: before,
            after_summary: Some(path.clone()),
        });
        self.push_info(
            transaction,
            "editor.project_browser.selected",
            format!("Selected project entry {path}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn open_project_browser_entry(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.project_browser.no_project",
                "Cannot open a ProjectBrowser entry before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let project_root = session.project_root.clone();
        self.selected_project_browser_path = Some(path.clone());
        if path.ends_with(".scene.json") {
            let full_path = project_root.join(normalize_project_relative_path(&path));
            transaction
                .read_set
                .push("project_browser.entry".to_string());
            transaction
                .write_set
                .push("editor_scene_document".to_string());
            return self.open_scene_document(transaction, &full_path);
        }
        if is_input_mapping_relative_path(&path) {
            return self.open_input_mapping(transaction, path);
        }
        self.push_info(
            transaction,
            "editor.project_browser.open_ignored",
            format!("Project entry {path} is selected but not openable in C-min."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn create_default_input_mapping(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.input_mapping.no_project",
                "Cannot create InputMapping before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let mapping = InputMappingAuthoringService::create_default();
        transaction.write_set.push(format!("input_mapping.{path}"));
        match InputMappingAuthoringService::save_in_scope(session.write_scope(), &path, &mapping) {
            Ok(()) => {
                let before = self.selected_project_browser_path.clone();
                self.selected_project_browser_path = Some(path.clone());
                self.input_mapping_editor_state =
                    InputMappingAuthoringService::open_editor_state(&session.project_root, &path)
                        .ok();
                transaction.state_changes.push(StateChangeSummary {
                    kind: "input_mapping.created".to_string(),
                    path: "workspace.selected_asset".to_string(),
                    before_summary: before,
                    after_summary: Some(path.clone()),
                });
                self.push_info(
                    transaction,
                    "editor.input_mapping.created",
                    format!("Created default InputMapping at {path}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.input_mapping.save_failed",
                    message,
                    Some("Check that the project Input folder is writable."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn delete_input_mapping(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        if !is_input_mapping_relative_path(&path) {
            self.push_error(
                transaction,
                "editor.input_mapping.path_invalid",
                "InputMapping path must be a JSON file under Input/.",
                Some("Choose an InputMapping asset inside the open project."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.input_mapping.no_project",
                "Cannot delete InputMapping before opening a project.",
                Some("Open a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.write_set.push(format!("input_mapping.{path}"));
        match session.write_scope().remove_file(&path) {
            Ok(_) => {
                if self
                    .input_mapping_editor_state
                    .as_ref()
                    .is_some_and(|state| state.selected_path == path)
                {
                    self.input_mapping_editor_state = None;
                }
                if self.selected_project_browser_path.as_deref() == Some(path.as_str()) {
                    self.selected_project_browser_path = None;
                }
                self.push_info(
                    transaction,
                    "editor.input_mapping.deleted",
                    format!("Deleted InputMapping at {path}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => {
                self.push_error(
                    transaction,
                    "editor.input_mapping.delete_failed",
                    error.to_string(),
                    Some("Check that the InputMapping exists inside the open project."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn open_input_mapping(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.input_mapping.no_project",
                "Cannot open InputMapping before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("input_mapping.{path}"));
        match InputMappingAuthoringService::open_editor_state(&session.project_root, &path) {
            Ok(state) => {
                self.selected_project_browser_path = Some(path.clone());
                self.input_mapping_editor_state = Some(state);
                self.push_info(
                    transaction,
                    "editor.input_mapping.opened",
                    format!("Opened InputMapping working copy {path}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.input_mapping.load_failed",
                    message,
                    Some("Select a valid InputMapping asset."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn discard_input_mapping_draft(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.input_mapping.no_project",
                "Cannot discard InputMapping before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("input_mapping.{path}"));
        match InputMappingAuthoringService::open_editor_state(&session.project_root, &path) {
            Ok(state) => {
                self.input_mapping_editor_state = Some(state);
                self.selected_project_browser_path = Some(path.clone());
                self.push_info(
                    transaction,
                    "editor.input_mapping.discarded",
                    format!("Discarded InputMapping draft {path}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.input_mapping.discard_failed",
                    message,
                    Some("Reload a valid InputMapping asset."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn select_input_context(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        context_id: String,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        let state = self.input_mapping_editor_state.as_mut().unwrap();
        if !state
            .draft_mapping
            .contexts
            .iter()
            .any(|context| context.id == context_id)
        {
            return self.fail_input_mapping_state(
                transaction,
                format!("Input context does not exist: {context_id}"),
            );
        }
        state.selected_context_id = Some(context_id.clone());
        self.push_info(
            transaction,
            "editor.input_mapping.context_selected",
            format!("Selected input context {context_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn select_input_action(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        action_id: String,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        let state = self.input_mapping_editor_state.as_mut().unwrap();
        if !state
            .draft_mapping
            .actions
            .iter()
            .any(|action| action.id == action_id)
        {
            return self.fail_input_mapping_state(
                transaction,
                format!("Input action does not exist: {action_id}"),
            );
        }
        state.selected_action_id = Some(action_id.clone());
        self.push_info(
            transaction,
            "editor.input_mapping.action_selected",
            format!("Selected input action {action_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn select_input_binding(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        binding_id: String,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        let state = self.input_mapping_editor_state.as_mut().unwrap();
        if !state
            .draft_mapping
            .bindings
            .iter()
            .any(|binding| binding.binding_id == binding_id)
        {
            return self.fail_input_mapping_state(
                transaction,
                format!("Input binding does not exist: {binding_id}"),
            );
        }
        state.selected_binding_id = Some(binding_id.clone());
        self.push_info(
            transaction,
            "editor.input_mapping.binding_selected",
            format!("Selected input binding {binding_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn begin_input_binding_capture(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        binding_id: String,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        let state = self.input_mapping_editor_state.as_mut().unwrap();
        if !state
            .draft_mapping
            .bindings
            .iter()
            .any(|binding| binding.binding_id == binding_id)
        {
            return self.fail_input_mapping_state(
                transaction,
                format!("Input binding does not exist: {binding_id}"),
            );
        }
        state.selected_binding_id = Some(binding_id.clone());
        state.capture_binding_id = Some(binding_id.clone());
        self.push_info(
            transaction,
            "editor.input_mapping.capture_started",
            format!("Capturing input for {binding_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn cancel_input_binding_capture(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        self.input_mapping_editor_state
            .as_mut()
            .unwrap()
            .capture_binding_id = None;
        self.push_info(
            transaction,
            "editor.input_mapping.capture_cancelled",
            "Cancelled input binding capture.",
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn commit_captured_input_binding(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        binding_id: String,
        device_path: String,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        if !InputControlCatalog::supported().supports_device_path(&device_path) {
            return self.fail_input_mapping_state(
                transaction,
                format!("Unsupported captured device path: {device_path}"),
            );
        }
        let state = self.input_mapping_editor_state.as_mut().unwrap();
        if state.capture_binding_id.as_deref() != Some(binding_id.as_str()) {
            return self.fail_input_mapping_state(
                transaction,
                "Captured binding does not match the active capture session.".to_string(),
            );
        }
        if device_path.eq_ignore_ascii_case("mouse/Position") {
            let binding = state
                .draft_mapping
                .bindings
                .iter()
                .find(|binding| binding.binding_id == binding_id)
                .unwrap();
            let pointer_compatible = state
                .draft_mapping
                .actions
                .iter()
                .find(|action| action.id == binding.action_id)
                .is_some_and(|action| action.value_type == InputActionValueType::Pointer);
            if !pointer_compatible {
                return self.fail_input_mapping_state(
                    transaction,
                    "mouse/Position capture requires a Pointer action.".to_string(),
                );
            }
        }
        if let Err(message) = InputMappingAuthoringService::apply(
            &mut state.draft_mapping,
            InputMappingEditCommand::SetBindingDevicePathById {
                binding_id: binding_id.clone(),
                device_path: device_path.clone(),
            },
        ) {
            return self.fail_input_mapping_state(transaction, message);
        }
        state.capture_binding_id = None;
        state.dirty = true;
        transaction
            .write_set
            .push(format!("input_mapping_draft.{path}"));
        self.push_info(
            transaction,
            "editor.input_mapping.capture_committed",
            format!("Captured {device_path} for {binding_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn preview_input_mapping(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        device_path: Option<String>,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        let state = self.input_mapping_editor_state.as_mut().unwrap();
        let device_path = device_path.or_else(|| {
            let binding_id = state.selected_binding_id.as_deref()?;
            state
                .draft_mapping
                .bindings
                .iter()
                .find(|binding| binding.binding_id == binding_id)
                .map(|binding| binding.device_path.clone())
        });
        let Some(device_path) = device_path else {
            return self.fail_input_mapping_state(
                transaction,
                "Select a binding or provide a device path before Preview.".to_string(),
            );
        };
        state.preview = Some(InputMappingAuthoringService::preview(
            &state.draft_mapping,
            &device_path,
        ));
        transaction
            .write_set
            .push("input_mapping_editor.preview".to_string());
        self.push_info(
            transaction,
            "editor.input_mapping.previewed",
            format!("Previewed InputMapping with {device_path}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_input_mapping_report_level(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        level: InputMappingReportLevel,
    ) -> CommandResult {
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        self.input_mapping_editor_state
            .as_mut()
            .unwrap()
            .report_level = level;
        transaction
            .write_set
            .push("input_mapping_editor.report_level".to_string());
        self.push_info(
            transaction,
            "editor.input_mapping.report_level",
            format!("Input Mapping report level set to {level:?}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn edit_input_mapping(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        command: InputMappingEditCommand,
    ) -> CommandResult {
        if self.active_project_session.is_none() {
            self.push_error(
                transaction,
                "editor.input_mapping.no_project",
                "Cannot edit InputMapping before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        if let InputMappingEditCommand::RemoveAction { action_id } = &command {
            let project_root = self
                .active_project_session
                .as_ref()
                .unwrap()
                .project_root
                .clone();
            let references = scan_input_action_references(&project_root, action_id);
            if !references.is_empty() {
                self.push_error(
                    transaction,
                    "input_mapping.external_action_reference_impact",
                    format!(
                        "Cannot remove action '{action_id}'; referenced by {}.",
                        references.join(", ")
                    ),
                    Some("Remove or update project references before removing the action."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        }
        transaction.read_set.push(format!("input_mapping.{path}"));
        transaction
            .write_set
            .push(format!("input_mapping_draft.{path}"));
        let state = self.input_mapping_editor_state.as_mut().unwrap();
        let before = format!(
            "actions={} bindings={}",
            state.draft_mapping.actions.len(),
            state.draft_mapping.bindings.len()
        );
        if let Err(message) = InputMappingAuthoringService::apply(&mut state.draft_mapping, command)
        {
            self.push_error(
                transaction,
                "editor.input_mapping.edit_failed",
                message,
                Some("Check the action id, binding index, and device path."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        state.dirty = true;
        let after = format!(
            "actions={} bindings={} dirty=true",
            state.draft_mapping.actions.len(),
            state.draft_mapping.bindings.len()
        );
        transaction.state_changes.push(StateChangeSummary {
            kind: "input_mapping.draft_edited".to_string(),
            path: format!("input_mapping_draft.{path}"),
            before_summary: Some(before),
            after_summary: Some(after),
        });
        self.selected_project_browser_path = Some(path.clone());
        self.push_info(
            transaction,
            "editor.input_mapping.draft_edited",
            format!("Edited InputMapping working copy {path}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn validate_input_mapping(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        save_only: bool,
    ) -> CommandResult {
        let Some(write_scope) = self
            .active_project_session
            .as_ref()
            .map(|session| session.write_scope().clone())
        else {
            self.push_error(
                transaction,
                "editor.input_mapping.no_project",
                "Cannot validate InputMapping before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if let Err(message) = self.ensure_input_mapping_editor_state(&path) {
            return self.fail_input_mapping_state(transaction, message);
        }
        transaction.read_set.push(format!("input_mapping.{path}"));
        let mapping = self
            .input_mapping_editor_state
            .as_ref()
            .unwrap()
            .draft_mapping
            .clone();
        let validation = mapping.validate();
        let has_errors = validation.has_errors();
        for diagnostic in validation.diagnostics {
            let suggested = diagnostic
                .suggested_fix
                .as_deref()
                .unwrap_or("Fix the InputMapping asset and validate again.");
            match diagnostic.severity {
                engine_input::InputDiagnosticSeverity::Warning => self.push_warning(
                    transaction,
                    &diagnostic.code,
                    diagnostic.message,
                    Some(suggested),
                ),
                engine_input::InputDiagnosticSeverity::Error => self.push_error(
                    transaction,
                    &diagnostic.code,
                    diagnostic.message,
                    Some(suggested),
                ),
            }
        }
        if has_errors {
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        if save_only {
            transaction.write_set.push(format!("input_mapping.{path}"));
            let save_result = InputMappingAuthoringService::save_editor_state_in_scope(
                &write_scope,
                self.input_mapping_editor_state.as_mut().unwrap(),
            );
            if let Err(message) = save_result {
                let code = if message.starts_with("input_mapping.stale_source_hash") {
                    "input_mapping.stale_source_hash"
                } else {
                    "editor.input_mapping.save_failed"
                };
                self.push_error(
                    transaction,
                    code,
                    message,
                    Some("Discard/reload external changes or save after resolving diagnostics."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        }
        self.selected_project_browser_path = Some(path.clone());
        self.push_info(
            transaction,
            if save_only {
                "editor.input_mapping.saved"
            } else {
                "editor.input_mapping.validated"
            },
            if save_only {
                format!("InputMapping {path} saved from working copy.")
            } else {
                format!("InputMapping {path} working copy validated.")
            },
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    fn ensure_input_mapping_editor_state(&mut self, path: &str) -> Result<(), String> {
        if self
            .input_mapping_editor_state
            .as_ref()
            .is_some_and(|state| state.selected_path == path)
        {
            return Ok(());
        }
        let project_root = self
            .active_project_session
            .as_ref()
            .ok_or_else(|| "Open or create a project first.".to_string())?
            .project_root
            .clone();
        self.input_mapping_editor_state = Some(InputMappingAuthoringService::open_editor_state(
            &project_root,
            path,
        )?);
        self.selected_project_browser_path = Some(path.to_string());
        Ok(())
    }

    fn fail_input_mapping_state(
        &mut self,
        transaction: &mut CommandTransaction,
        message: String,
    ) -> CommandResult {
        self.push_error(
            transaction,
            "editor.input_mapping.state_failed",
            message,
            Some("Open or reload a valid InputMapping asset."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Failed)
    }

    pub(crate) fn set_workspace_view_mode(
        &mut self,
        transaction: &mut CommandTransaction,
        mode: WorkspaceViewMode,
    ) -> CommandResult {
        transaction
            .write_set
            .push("workspace.view_mode".to_string());
        let before = format!("{:?}", self.workspace_view_mode);
        self.workspace_view_mode = mode;
        transaction.state_changes.push(StateChangeSummary {
            kind: "workspace.view_mode.changed".to_string(),
            path: "workspace.view_mode".to_string(),
            before_summary: Some(before),
            after_summary: Some(format!("{mode:?}")),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn open_scene_document_for_launcher(&mut self, path: &Path) -> bool {
        match EditorSceneDocument::load_from_path(path) {
            Ok(document) => {
                self.scene_path = Some(path.to_path_buf());
                self.editor_scene_document = Some(document);
                self.scene_selection = SceneSelection::default();
                self.scene_undo_stack = SceneUndoStack::default();
                self.last_scene_edit_report = None;
                self.last_preview_world_sync_report = None;
                self.selected_entity_id = None;
                self.selected_entity_source = None;
                true
            }
            Err(_) => false,
        }
    }

    fn validate_open_project_composition(
        &self,
        project: &crate::ProjectSession,
    ) -> Result<(), (&'static str, String)> {
        let requested = &project.manifest.runtime_module;
        if requested.resolved_source_kind() == crate::ProjectRuntimeSourceKind::ProjectRust {
            // ProjectRust authoring does not require an already loaded module. Play performs the
            // exact native-module identity check after asynchronous preparation publishes ready.
            return Ok(());
        }
        let linked = self
            .linked_project_runtimes
            .only_descriptor()
            .map_err(|error| {
                (
                    "project_editor_composition.module_not_linked",
                    error.message,
                )
            })?;
        if linked.module_id != requested.module_id
            || linked.interface_version != requested.interface_version
        {
            return Err((
                "project_editor_composition.handoff_required",
                "The running Editor composition does not match this built-in project runtime."
                    .to_string(),
            ));
        }
        Ok(())
    }
}

fn default_pre_project_intent_draft_path() -> PathBuf {
    let local_root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    local_root
        .join("AI First Engine")
        .join("IntentDrafts")
        .join(format!("draft-{stamp}.json"))
}
