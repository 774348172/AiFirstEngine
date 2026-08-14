use engine_runtime::default_game_run::DefaultGameRunOrchestrator;
use engine_runtime::frame_loop::FrameLoop;
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::scene_loader::load_scene_into_world;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EditorPlayPreparationTicket {
    pub project_root: PathBuf,
    pub project_id: String,
    pub manifest_digest: String,
    pub request: EditorPreviewPackageRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPlayPreparationError {
    pub code: String,
    pub message: String,
}

use crate::{
    CommandResult, CommandStatus, CommandTransaction, EditorGameViewPlayRunner,
    EditorPlayPreviewPackageReport, EditorPreviewPackageRequest, EditorPreviewPackageService,
    EditorPreviewPackageStatus, EditorRuntimePlayState, EditorSession, EntitySelectionSource,
    PlaySessionMode, PlaySessionRequest, PlaySessionState, SceneSavePipeline, SceneSaveStatus,
    StateChangeSummary, UndoPolicy,
};

impl EditorSession {
    pub fn editor_play_preparation_ticket(
        &mut self,
    ) -> Result<Option<EditorPlayPreparationTicket>, EditorPlayPreparationError> {
        let Some(project_session) = self.active_project_session.clone() else {
            return Ok(None);
        };
        if let Some(document) = self.editor_scene_document.as_mut() {
            if document.dirty_state.dirty {
                let save_report = SceneSavePipeline::save_in_scope(
                    document,
                    project_session.write_scope(),
                    self.scene_path.as_ref(),
                );
                if save_report.status == SceneSaveStatus::Failed {
                    return Err(EditorPlayPreparationError {
                        code: "editor.preview_package.autosave_failed".to_string(),
                        message: format!(
                            "Cannot start Play because the active Scene could not be saved: {}",
                            save_report.path.display()
                        ),
                    });
                }
            }
        }
        let manifest_path = project_session.project_root.join("project.aife.json");
        let manifest_bytes =
            std::fs::read(&manifest_path).map_err(|error| EditorPlayPreparationError {
                code: "editor.play_preparation.manifest_read_failed".to_string(),
                message: format!("Project manifest cannot be read before Play: {error}"),
            })?;
        let active_scene_id = self
            .editor_scene_document
            .as_ref()
            .map(|scene| scene.scene_id.clone());
        Ok(Some(EditorPlayPreparationTicket {
            project_root: project_session.project_root.clone(),
            project_id: project_session.manifest.project_id.clone(),
            manifest_digest: engine_runtime::canonical_digest::sha256_prefixed(&manifest_bytes),
            request: EditorPreviewPackageRequest::editor_play(&project_session.project_root)
                .with_active_scene_id(active_scene_id)
                .without_player_artifact(),
        }))
    }

    pub fn install_prepared_editor_play_report(
        &mut self,
        ticket: &EditorPlayPreparationTicket,
        report: EditorPlayPreviewPackageReport,
    ) -> Result<(), EditorPlayPreparationError> {
        let Some(active) = self.active_project_session.as_ref() else {
            return Err(EditorPlayPreparationError {
                code: "editor.play_preparation.project_closed".to_string(),
                message: "The project was closed while Play was being prepared.".to_string(),
            });
        };
        let manifest_bytes =
            std::fs::read(active.project_root.join("project.aife.json")).map_err(|error| {
                EditorPlayPreparationError {
                    code: "editor.play_preparation.manifest_read_failed".to_string(),
                    message: format!("Project manifest cannot be re-read before Play: {error}"),
                }
            })?;
        let manifest_digest = engine_runtime::canonical_digest::sha256_prefixed(&manifest_bytes);
        if active.project_root != ticket.project_root
            || active.manifest.project_id != ticket.project_id
            || manifest_digest != ticket.manifest_digest
            || PathBuf::from(&report.project_root) != ticket.project_root
        {
            return Err(EditorPlayPreparationError {
                code: "editor.play_preparation.project_drifted".to_string(),
                message: "Project identity changed while Play was being prepared; retry Play from the current project state.".to_string(),
            });
        }
        self.prepared_editor_play_report = Some(report);
        Ok(())
    }

    pub(crate) fn start_play_session(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        if let Some(control_state) = self
            .editor_runtime_play_instance
            .as_ref()
            .map(|instance| instance.control_state())
        {
            if control_state == EditorRuntimePlayState::Paused {
                return self.resume_active_game_view_play_session(transaction);
            }
            self.push_info(
                transaction,
                "editor.gameview_play.already_running",
                format!("Editor GameView runtime is already {control_state:?}."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        transaction
            .write_set
            .push("play_session.current".to_string());
        transaction
            .write_set
            .push("editor_preview_package.last_report".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let mut preview_package_report_path = None;
        let mut preview_cache_status = None;
        let mut preview_dirty_domains = Vec::new();
        let mut preview_prepare_duration_ms = None;
        let mut project_root_for_run = None;
        let mut use_editor_gameview_runner = false;
        let runtime_package_path = if let Some(project_session) =
            self.active_project_session.clone()
        {
            let project_root = project_session.project_root.clone();
            project_root_for_run = Some(project_root.clone());
            use_editor_gameview_runner = true;
            if let Some(document) = self.editor_scene_document.as_mut() {
                if document.dirty_state.dirty {
                    let save_report = SceneSavePipeline::save_in_scope(
                        document,
                        project_session.write_scope(),
                        self.scene_path.as_ref(),
                    );
                    match save_report.status {
                        SceneSaveStatus::Saved => self.push_info(
                            transaction,
                            "editor.preview_package.autosave_scene",
                            format!(
                                "Saved dirty Scene before Play: {}",
                                save_report.path.display()
                            ),
                        ),
                        SceneSaveStatus::Unchanged => self.push_info(
                            transaction,
                            "editor.preview_package.autosave_scene_unchanged",
                            format!(
                                "Dirty Scene already matched its file before Play: {}",
                                save_report.path.display()
                            ),
                        ),
                        SceneSaveStatus::Failed => {
                            self.push_error(
                                transaction,
                                "editor.preview_package.autosave_failed",
                                format!(
                                    "Cannot start Play because the active Scene could not be saved: {}",
                                    save_report.path.display()
                                ),
                                Some("Save the Scene manually and retry Play."),
                            );
                            return self
                                .finish_transaction(transaction.clone(), CommandStatus::Failed);
                        }
                    }
                }
            }
            let active_scene_id = self
                .editor_scene_document
                .as_ref()
                .map(|scene| scene.scene_id.clone());
            let preview_report = self.prepared_editor_play_report.take().unwrap_or_else(|| {
                EditorPreviewPackageService::prepare(
                    EditorPreviewPackageRequest::editor_play(&project_root)
                        .with_active_scene_id(active_scene_id)
                        .without_player_artifact(),
                )
            });
            let report_path = preview_report.report_path.clone();
            let cache_status = preview_report.cache_status.as_report_str().to_string();
            let dirty_domains = preview_report.dirty_domain_labels();
            let duration_ms = preview_report.duration_total_ms;
            preview_package_report_path = report_path.clone();
            preview_cache_status = Some(cache_status.clone());
            preview_dirty_domains = dirty_domains.clone();
            preview_prepare_duration_ms = Some(duration_ms);
            self.last_editor_preview_package_report = Some(preview_report.clone());
            transaction.state_changes.push(StateChangeSummary {
                kind: "editor_preview_package.prepared".to_string(),
                path: "editor_preview_package.last_report".to_string(),
                before_summary: None,
                after_summary: Some(format!(
                    "status={:?} cache={} dirty={:?}",
                    preview_report.status, cache_status, dirty_domains
                )),
            });
            self.push_info(
                transaction,
                "editor.preview_package.prepared",
                format!(
                    "Prepared Editor Play preview package: cache={} dirty={:?} duration_ms={} report={}",
                    cache_status,
                    dirty_domains,
                    duration_ms,
                    report_path.as_deref().unwrap_or("none")
                ),
            );
            if preview_report.status == EditorPreviewPackageStatus::Failed {
                self.push_error(
                    transaction,
                    "editor.preview_package.prepare_failed",
                    format!(
                        "Editor Play preview package failed. Read report: {}",
                        report_path.as_deref().unwrap_or("report_not_written")
                    ),
                    Some("Open the preview package report and fix the first failed stage."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
            let Some(runtime_package_dir) = preview_report.runtime_package_dir.as_ref() else {
                self.push_error(
                    transaction,
                    "editor.preview_package.runtime_package_missing",
                    "Preview package succeeded without a runtime_package_dir.",
                    Some("Rebuild the preview package cache."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            };
            let runtime_package_path = PathBuf::from(runtime_package_dir);
            if let Err((code, message)) =
                self.validate_project_editor_composition_for_play(&runtime_package_path)
            {
                self.push_error(
                    transaction,
                    code,
                    message,
                    Some("Prepare and hand off to an Editor composition matching the RuntimePackage."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
            }
            self.load_prepared_runtime_package_for_editor(&runtime_package_path);
            runtime_package_path
        } else {
            let Some(runtime_package_path) = self.runtime_package_path.clone() else {
                self.push_error(
                    transaction,
                    "editor.play_session.runtime_package_required",
                    "Cannot start play session before opening a project or Runtime Package.",
                    Some("Open or create a project first, or open a Runtime Package for the debug path."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            };
            runtime_package_path
        };
        let base_request = if use_editor_gameview_runner {
            PlaySessionRequest::windowed_user_run(runtime_package_path.clone())
        } else {
            PlaySessionRequest::headless_gate(runtime_package_path.clone())
        };
        let mut request = PlaySessionRequest {
            session_id: format!("play-session-{}", self.revision + 1),
            frame_limit: 3,
            project_root: project_root_for_run.unwrap_or_else(|| runtime_package_path.clone()),
            preview_package_report_path,
            preview_cache_status,
            preview_dirty_domains,
            preview_prepare_duration_ms,
            ..base_request
        };
        request.game_view_target = self.game_view_target;
        if let Some(scene) = self.editor_scene_document.as_ref() {
            request.scene_ref = Some(scene.scene_id.clone());
        }
        let session_id = request.session_id.clone();
        let request_mode = request.mode;
        self.play_session_controller.queue_start(request);
        self.push_info(
            transaction,
            "editor.play_session.started",
            format!("Play session started: {session_id}"),
        );
        let report = if request_mode == PlaySessionMode::WindowedUserRun {
            let runner = EditorGameViewPlayRunner::with_linked_modules(std::sync::Arc::clone(
                &self.linked_project_runtimes,
            ));
            let report = self
                .play_session_controller
                .drain_queued_with_runner(&runner);
            if let Some(output) = runner.take_last_output() {
                self.editor_runtime_play_instance = output.instance;
                if self.selected_entity_source == Some(EntitySelectionSource::ActiveGameViewRuntime)
                {
                    self.selected_entity_id = None;
                    self.selected_entity_source = None;
                }
                self.last_game_view_runtime_frame = output.frame;
                self.sync_animator2d_play_observations();
                self.last_game_view_present_report = Some(output.present_report.clone());
                self.apply_maximize_on_play_start(transaction);
                self.push_info(
                    transaction,
                    "editor.gameview_present.reported",
                    format!(
                        "GameView present report: status={:?} frames={} descriptor={} report={}",
                        output.present_report.status,
                        output.present_report.frame_count,
                        output.present_report.texture_descriptor_status,
                        output
                            .present_report
                            .report_path
                            .as_deref()
                            .unwrap_or("none")
                    ),
                );
            }
            report
        } else {
            self.editor_runtime_play_instance = None;
            self.last_game_view_runtime_frame = None;
            self.animator2d_authoring.clear_play_observations();
            self.play_session_controller
                .drain_queued_with_runtime(&DefaultGameRunOrchestrator)
        };
        if let Some(report) = report {
            self.last_play_session_report = Some(report.clone());
            self.push_play_session_report(transaction, &report);
            transaction.state_changes.push(StateChangeSummary {
                kind: "play_session.completed".to_string(),
                path: "play_session.current".to_string(),
                before_summary: Some("queued".to_string()),
                after_summary: Some(format!("{:?}", report.state)),
            });
            let status = if report.state == PlaySessionState::Failed {
                CommandStatus::Failed
            } else {
                CommandStatus::Committed
            };
            return self.finish_transaction(transaction.clone(), status);
        }
        self.push_error(
            transaction,
            "editor.play_session.no_report",
            "Play session did not produce a report.",
            Some("Check PlaySessionController queue handling."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Failed)
    }

    pub(crate) fn stop_play_session(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction
            .write_set
            .push("play_session.current".to_string());
        transaction
            .write_set
            .push("editor_gameview_play.instance".to_string());
        transaction.undo_policy = UndoPolicy::None;
        transaction.state_changes.push(StateChangeSummary {
            kind: "play_session.stop_requested".to_string(),
            path: "play_session.current".to_string(),
            before_summary: Some(format!("{:?}", self.play_session_controller.state())),
            after_summary: Some("queued_stop".to_string()),
        });
        let controller_state_before_stop = self.play_session_controller.state();
        let had_gameview_instance = self.editor_runtime_play_instance.is_some();
        if let Some(instance) = self.editor_runtime_play_instance.take() {
            let stop_report = instance.stop();
            if let Some(discard_diagnostic) = stop_report
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == "runtime_temporary_edits_discarded")
            {
                self.push_info(
                    transaction,
                    &discard_diagnostic.code,
                    discard_diagnostic.message.clone(),
                );
                transaction.state_changes.push(StateChangeSummary {
                    kind: "runtime.temporary_edits.discarded".to_string(),
                    path: "editor_gameview_play.temporary_edit_summary".to_string(),
                    before_summary: Some(discard_diagnostic.message.clone()),
                    after_summary: None,
                });
            }
            self.last_game_view_runtime_frame = stop_report.last_frame.clone();
            self.animator2d_authoring.clear_play_observations();
            self.last_game_view_present_report = Some(stop_report.clone());
            if self.selected_entity_source == Some(EntitySelectionSource::ActiveGameViewRuntime) {
                self.selected_entity_id = None;
                self.selected_entity_source = None;
                transaction.state_changes.push(StateChangeSummary {
                    kind: "selection.runtime.cleared".to_string(),
                    path: "selection.selected_entity_source".to_string(),
                    before_summary: Some(
                        EntitySelectionSource::ActiveGameViewRuntime
                            .as_str()
                            .to_string(),
                    ),
                    after_summary: None,
                });
            }
            self.restore_game_view_maximize_after_play(transaction);
            self.push_info(
                transaction,
                "editor.gameview_present.stopped",
                format!(
                    "Stopped EditorRuntimePlayInstance: frames={} report={}",
                    stop_report.frame_count,
                    stop_report.report_path.as_deref().unwrap_or("none")
                ),
            );
        }
        self.animator2d_authoring.clear_play_observations();
        if had_gameview_instance && controller_state_before_stop != PlaySessionState::Running {
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        self.play_session_controller.queue_stop(None);
        let report = self
            .play_session_controller
            .drain_queued_with_runtime(&DefaultGameRunOrchestrator);
        if let Some(report) = report {
            self.last_play_session_report = Some(report.clone());
            self.push_play_session_report(transaction, &report);
            let status = if report.state == PlaySessionState::Failed {
                CommandStatus::Failed
            } else {
                CommandStatus::Committed
            };
            return self.finish_transaction(transaction.clone(), status);
        }
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn pause_active_game_view_play_session(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction.undo_policy = UndoPolicy::None;
        transaction
            .write_set
            .push("editor_gameview_play.control_state".to_string());
        let Some(instance) = self.editor_runtime_play_instance.as_mut() else {
            self.push_error(
                transaction,
                "editor.gameview_play.no_active_runtime",
                "Cannot pause because no active Editor GameView runtime exists.",
                Some("Start Play first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if instance.control_state() == EditorRuntimePlayState::Paused {
            self.push_info(
                transaction,
                "editor.gameview_play.already_paused",
                "Editor GameView runtime is already paused.",
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        let report = instance.pause();
        self.last_game_view_runtime_frame = report.last_frame.clone();
        self.sync_animator2d_play_observations();
        self.last_game_view_present_report = Some(report.clone());
        self.push_info(
            transaction,
            "editor.gameview_play.paused",
            format!(
                "Paused Editor GameView runtime at frame {}.",
                report.frame_count
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn resume_active_game_view_play_session(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction.undo_policy = UndoPolicy::None;
        transaction
            .write_set
            .push("editor_gameview_play.control_state".to_string());
        let Some(instance) = self.editor_runtime_play_instance.as_mut() else {
            self.push_error(
                transaction,
                "editor.gameview_play.no_active_runtime",
                "Cannot resume because no active Editor GameView runtime exists.",
                Some("Start Play first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if instance.control_state() == EditorRuntimePlayState::Running {
            self.push_info(
                transaction,
                "editor.gameview_play.already_running",
                "Editor GameView runtime is already running.",
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        let report = instance.resume();
        self.last_game_view_runtime_frame = report.last_frame.clone();
        self.sync_animator2d_play_observations();
        self.last_game_view_present_report = Some(report.clone());
        self.push_info(
            transaction,
            "editor.gameview_play.resumed",
            format!(
                "Resumed Editor GameView runtime at frame {}.",
                report.frame_count
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn step_active_game_view_play_session(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction.undo_policy = UndoPolicy::None;
        transaction
            .write_set
            .push("editor_gameview_play.control_state".to_string());
        let Some(instance) = self.editor_runtime_play_instance.as_mut() else {
            return self.tick_one_frame(transaction);
        };
        if instance.control_state() != EditorRuntimePlayState::Paused {
            self.push_error(
                transaction,
                "editor.gameview_play.pause_before_step",
                "StepFrame requires the active Editor GameView runtime to be paused.",
                Some("Pause Play, then run StepFrame."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let before = instance.control_state();
        let report = instance.step_next_frame();
        self.last_game_view_runtime_frame = report.last_frame.clone();
        self.sync_animator2d_play_observations();
        self.last_game_view_present_report = Some(report.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "editor_gameview_play.step_frame".to_string(),
            path: "editor_gameview_play.control_state".to_string(),
            before_summary: Some(format!("{before:?}")),
            after_summary: Some(format!(
                "{:?} frame={} target={}",
                report.control_state, report.frame_count, report.target_runtime_domain
            )),
        });
        self.push_info(
            transaction,
            "editor.gameview_play.step_frame",
            format!(
                "Stepped active Editor GameView runtime exactly one frame to {}.",
                report.frame_count
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_game_view_maximize_on_play(
        &mut self,
        transaction: &mut CommandTransaction,
        enabled: bool,
    ) -> CommandResult {
        transaction.undo_policy = UndoPolicy::None;
        transaction
            .write_set
            .push("editor_gameview_layout.maximize_on_play".to_string());
        let before = self.game_view_maximize_on_play;
        self.game_view_maximize_on_play = enabled;
        self.game_view_maximize_reason = Some("user_toggle".to_string());
        transaction.state_changes.push(StateChangeSummary {
            kind: "editor_gameview_layout.maximize_on_play".to_string(),
            path: "editor_gameview_layout.maximize_on_play".to_string(),
            before_summary: Some(before.to_string()),
            after_summary: Some(enabled.to_string()),
        });
        self.push_info(
            transaction,
            "editor.gameview_layout.maximize_on_play_set",
            format!("Maximize on Play set to {enabled}."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_current_game_view_target(
        &mut self,
        transaction: &mut CommandTransaction,
        width: u32,
        height: u32,
        scale_policy: editor_ui_model::EditorGameViewScalePolicy,
    ) -> CommandResult {
        transaction.undo_policy = UndoPolicy::None;
        transaction
            .write_set
            .push("editor_gameview.target".to_string());
        if self.editor_runtime_play_instance.is_some()
            || matches!(
                self.play_session_controller.state(),
                PlaySessionState::Preparing
                    | PlaySessionState::Building
                    | PlaySessionState::StagingPackage
                    | PlaySessionState::Launching
                    | PlaySessionState::Running
                    | PlaySessionState::Stopping
            )
        {
            self.push_error(
                transaction,
                "editor.gameview_target.play_active",
                "Stop the active Play session before changing the GameView target.",
                Some("Stop Play, then select the target for the next session."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let scale_policy = match scale_policy {
            editor_ui_model::EditorGameViewScalePolicy::Contain => {
                engine_runtime::game_view_presentation::GameViewScalePolicy::Contain
            }
            editor_ui_model::EditorGameViewScalePolicy::Stretch => {
                engine_runtime::game_view_presentation::GameViewScalePolicy::Stretch
            }
        };
        let target = engine_runtime::game_view_presentation::GameViewTargetSpec::new(
            width,
            height,
            scale_policy,
        );
        if let Err(error) = target.validate() {
            self.push_error(
                transaction,
                error.code,
                format!("Invalid GameView target {width}x{height}."),
                Some("Choose a positive target within the runtime capability limit."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let before = self.game_view_target;
        self.game_view_target = target;
        transaction.state_changes.push(StateChangeSummary {
            kind: "editor_gameview.target".to_string(),
            path: "editor_gameview.target".to_string(),
            before_summary: Some(format!(
                "{}x{} {:?}",
                before.extent.width, before.extent.height, before.scale_policy
            )),
            after_summary: Some(format!("{width}x{height} {scale_policy:?}")),
        });
        self.push_info(
            transaction,
            "editor.gameview_target.set",
            format!("GameView target set to {width}x{height} {scale_policy:?}."),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn toggle_game_view_maximize_on_play(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        self.set_game_view_maximize_on_play(transaction, !self.game_view_maximize_on_play)
    }

    fn load_prepared_runtime_package_for_editor(&mut self, runtime_package_path: &PathBuf) {
        let package_result = load_runtime_package(runtime_package_path);
        let Some(package) = package_result.value else {
            return;
        };
        let active_scene_id = package.manifest.active_scene_id.clone();
        let world_result = load_scene_into_world(&package.active_scene);
        let Some(world) = world_result.value else {
            return;
        };
        self.runtime_package_path = Some(runtime_package_path.clone());
        self.runtime_package = Some(package);
        self.world = Some(world);
        self.frame_loop = Some(FrameLoop::new(active_scene_id));
        self.last_frame_output = None;
        self.selected_trace_entry_id = None;
    }

    fn validate_project_editor_composition_for_play(
        &self,
        runtime_package_path: &PathBuf,
    ) -> Result<(), (&'static str, String)> {
        let load = load_runtime_package(runtime_package_path);
        let package = load.value.ok_or_else(|| {
            (
                "project_editor_composition.runtime_package_invalid",
                "Prepared RuntimePackage could not be loaded for composition validation."
                    .to_string(),
            )
        })?;
        self.validate_project_editor_composition_identity(
            &package.manifest.project.project_id,
            &package.manifest.project.runtime_module,
        )
    }

    fn validate_project_editor_composition_identity(
        &self,
        project_id: &str,
        requested: &engine_runtime::runtime_package::RuntimeProjectModuleRef,
    ) -> Result<(), (&'static str, String)> {
        let linked = self
            .linked_project_runtimes
            .only_descriptor()
            .map_err(|error| {
                (
                    "project_editor_composition.module_not_linked",
                    error.message,
                )
            })?;
        if let Some(identity) = &self.project_editor_composition_identity {
            if identity.project_id != project_id
                || identity.module_id != requested.module_id
                || identity.interface_version != requested.interface_version
                || identity.aot_content_digest != requested.aot_content_digest
                || linked.module_id != identity.module_id
                || linked.interface_version != identity.interface_version
                || linked.aot_content_digest != identity.aot_content_digest
            {
                return Err((
                    "project_editor_composition.handoff_required",
                    "The prepared RuntimePackage identity does not exactly match the running Editor composition."
                        .to_string(),
                ));
            }
        } else if linked.module_id != requested.module_id
            || linked.interface_version != requested.interface_version
            || linked.aot_content_digest != requested.aot_content_digest
        {
            return Err((
                "project_editor_composition.handoff_required",
                "The prepared RuntimePackage module is not linked into the running Editor."
                    .to_string(),
            ));
        }
        Ok(())
    }

    fn apply_maximize_on_play_start(&mut self, transaction: &mut CommandTransaction) {
        if !self.game_view_maximize_on_play {
            return;
        }
        let was_maximized = self.is_game_view_maximized;
        if !was_maximized {
            self.game_view_restore_workspace_region = Some("workspace.main".to_string());
        }
        self.is_game_view_maximized = true;
        self.game_view_maximize_reason = Some("play_started".to_string());
        transaction.state_changes.push(StateChangeSummary {
            kind: "editor_gameview_layout.maximized".to_string(),
            path: "editor_gameview_layout.is_game_view_maximized".to_string(),
            before_summary: Some(was_maximized.to_string()),
            after_summary: Some("true".to_string()),
        });
    }

    fn restore_game_view_maximize_after_play(&mut self, transaction: &mut CommandTransaction) {
        if self.game_view_maximize_reason.as_deref() != Some("play_started") {
            return;
        }
        let was_maximized = self.is_game_view_maximized;
        self.is_game_view_maximized = false;
        self.game_view_restore_workspace_region = None;
        self.game_view_maximize_reason = Some("stop_restore".to_string());
        transaction.state_changes.push(StateChangeSummary {
            kind: "editor_gameview_layout.restored".to_string(),
            path: "editor_gameview_layout.is_game_view_maximized".to_string(),
            before_summary: Some(was_maximized.to_string()),
            after_summary: Some("false".to_string()),
        });
    }
}

#[cfg(test)]
mod project_editor_composition_play_tests {
    use super::*;
    use engine_runtime::project_runtime_module::{
        LinkedProjectRuntimeSet, ProjectRuntimeError, ProjectRuntimeModule,
        ProjectRuntimeModuleDescriptor, ProjectRuntimeRegistration,
    };
    use engine_runtime::runtime_package::RuntimeProjectModuleRef;
    use std::sync::Arc;

    struct FixtureRuntime {
        descriptor: ProjectRuntimeModuleDescriptor,
    }
    impl ProjectRuntimeModule for FixtureRuntime {
        fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
            &self.descriptor
        }

        fn install(&self, _: &mut ProjectRuntimeRegistration) -> Result<(), ProjectRuntimeError> {
            Ok(())
        }
    }

    fn identity() -> crate::ProjectEditorCompositionIdentity {
        crate::ProjectEditorCompositionIdentity {
            schema_version: crate::PROJECT_EDITOR_COMPOSITION_IDENTITY_SCHEMA_VERSION.to_string(),
            project_id: "fixture.project".to_string(),
            module_id: "fixture.runtime".to_string(),
            interface_version: "project-runtime-module.v2".to_string(),
            aot_content_digest: format!("sha256:{}", "a".repeat(64)),
            editor_build_identity: format!("sha256:{}", "b".repeat(64)),
            engine_sdk_digest: format!("sha256:{}", "c".repeat(64)),
            toolchain_identity: "rustc-test".to_string(),
            target_triple: "x86_64-pc-windows-msvc".to_string(),
            profile: "release".to_string(),
            normalized_manifest_digest: format!("sha256:{}", "d".repeat(64)),
            normalized_dependency_digest: format!("sha256:{}", "e".repeat(64)),
            dependency_lock_digest: format!("sha256:{}", "f".repeat(64)),
        }
    }

    #[test]
    fn project_editor_composition_play_requires_project_module_and_aot_exact_match() {
        let identity = identity();
        let linked = LinkedProjectRuntimeSet::singleton(Arc::new(FixtureRuntime {
            descriptor: ProjectRuntimeModuleDescriptor::new(
                identity.module_id.clone(),
                identity.aot_content_digest.clone(),
            ),
        }))
        .unwrap();
        let session =
            EditorSession::with_project_editor_composition(Arc::new(linked), identity.clone())
                .unwrap();
        let exact = RuntimeProjectModuleRef::new(
            identity.module_id.clone(),
            identity.interface_version.clone(),
            identity.aot_content_digest.clone(),
        );
        assert!(session
            .validate_project_editor_composition_identity(&identity.project_id, &exact)
            .is_ok());

        let mut changed_aot = exact.clone();
        changed_aot.aot_content_digest = format!("sha256:{}", "1".repeat(64));
        assert_eq!(
            session
                .validate_project_editor_composition_identity(&identity.project_id, &changed_aot)
                .unwrap_err()
                .0,
            "project_editor_composition.handoff_required"
        );
        assert_eq!(
            session
                .validate_project_editor_composition_identity("another.project", &exact)
                .unwrap_err()
                .0,
            "project_editor_composition.handoff_required"
        );
        assert!(
            session
                .validate_project_editor_composition_identity(&identity.project_id, &exact)
                .is_ok(),
            "project data-only changes do not alter composition identity"
        );
    }
}
