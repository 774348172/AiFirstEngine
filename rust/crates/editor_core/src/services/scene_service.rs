use editor_ui_model::{DiagnosticSeverity, DiagnosticSource, EditorDiagnostic, Vec3};
use engine_runtime::ids::EntityId;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::editor_gameview_play::{scene_component_type_for_runtime, scene_field_path_for_runtime};
use crate::{
    ApplyRuntimeChangeCandidate, ApplyRuntimeChangeCandidateStatus, ApplyRuntimeChangeReport,
    AssetPlacementDiagnosticSeverity, AssetPlacementReport, AssetPlacementRequest,
    AssetPlacementResolver, CommandResult, CommandStatus, CommandTransaction, EditorSceneDocument,
    EditorSession, EditorVec3, EntitySelectionSource, PreviewWorldSync, RuntimePickStatus,
    RuntimeWorldPickRequest, SceneEditCommand, SceneEditDiagnostic, SceneEditDiagnosticSeverity,
    SceneEditRequest, SceneEditRequestSource, SceneEditTransaction, SceneEditTransactionStatus,
    SceneSavePipeline, SceneSaveStatus, SceneUndoStack, StateChangeSummary, UndoPolicy,
    WorldPickCollector,
};

pub(crate) fn editor_vec3_to_ui(value: EditorVec3) -> Vec3 {
    Vec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

pub(crate) fn runtime_vec3_to_ui(value: engine_runtime::math::Vec3) -> Vec3 {
    Vec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

pub(crate) fn ui_vec3_to_editor(value: Vec3) -> EditorVec3 {
    EditorVec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

pub(crate) fn scene_edit_source_for_ui_source(source: &str) -> SceneEditRequestSource {
    match source {
        "Viewport" => SceneEditRequestSource::SceneView,
        "Hierarchy" => SceneEditRequestSource::Hierarchy,
        "Inspector" => SceneEditRequestSource::Inspector,
        "Toolbar" => SceneEditRequestSource::Toolbar,
        "AiAssistant" => SceneEditRequestSource::Ai,
        "Test" => SceneEditRequestSource::Test,
        _ => SceneEditRequestSource::Test,
    }
}

pub(crate) fn scene_diagnostics_to_editor(
    transaction: &CommandTransaction,
    diagnostics: Vec<SceneEditDiagnostic>,
) -> Vec<EditorDiagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| EditorDiagnostic {
            severity: match diagnostic.severity {
                SceneEditDiagnosticSeverity::Info => DiagnosticSeverity::Info,
                SceneEditDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
                SceneEditDiagnosticSeverity::Error => DiagnosticSeverity::Error,
            },
            code: diagnostic.code,
            message: diagnostic.message,
            source: DiagnosticSource::EditorCore,
            command_id: Some(transaction.command_id.clone()),
            request_id: Some(transaction.request_id.clone()),
            path: diagnostic.path,
            entity_id: diagnostic.entity_id,
            trace_entry_id: None,
            suggested_action: None,
        })
        .collect()
}

pub(crate) fn asset_placement_diagnostics_to_editor(
    transaction: &CommandTransaction,
    report: &AssetPlacementReport,
) -> Vec<EditorDiagnostic> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| EditorDiagnostic {
            severity: match diagnostic.severity {
                AssetPlacementDiagnosticSeverity::Info => DiagnosticSeverity::Info,
                AssetPlacementDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
                AssetPlacementDiagnosticSeverity::Error => DiagnosticSeverity::Error,
            },
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            source: DiagnosticSource::EditorCore,
            command_id: Some(transaction.command_id.clone()),
            request_id: Some(transaction.request_id.clone()),
            path: None,
            entity_id: report.selected_entity_id.clone(),
            trace_entry_id: None,
            suggested_action: Some("Use a supported Scene placement asset type.".to_string()),
        })
        .collect()
}

fn editor_vec3_from_runtime_value(value: &Value) -> Result<EditorVec3, String> {
    if let Some(array) = value.as_array() {
        if array.len() == 3 {
            return Ok(EditorVec3 {
                x: array[0]
                    .as_f64()
                    .ok_or_else(|| "Vec3.x must be a number.".to_string())?
                    as f32,
                y: array[1]
                    .as_f64()
                    .ok_or_else(|| "Vec3.y must be a number.".to_string())?
                    as f32,
                z: array[2]
                    .as_f64()
                    .ok_or_else(|| "Vec3.z must be a number.".to_string())?
                    as f32,
            });
        }
    }
    let object = value
        .as_object()
        .ok_or_else(|| "Vec3 value must be an object or [x, y, z] array.".to_string())?;
    Ok(EditorVec3 {
        x: object
            .get("x")
            .and_then(Value::as_f64)
            .ok_or_else(|| "Vec3.x must be a number.".to_string())? as f32,
        y: object
            .get("y")
            .and_then(Value::as_f64)
            .ok_or_else(|| "Vec3.y must be a number.".to_string())? as f32,
        z: object
            .get("z")
            .and_then(Value::as_f64)
            .ok_or_else(|| "Vec3.z must be a number.".to_string())? as f32,
    })
}

fn scene_edit_command_for_runtime_apply(
    document: &EditorSceneDocument,
    candidate: &ApplyRuntimeChangeCandidate,
) -> Result<SceneEditCommand, String> {
    if candidate.status != ApplyRuntimeChangeCandidateStatus::Ready {
        return Err("Apply candidate is not ready.".to_string());
    }
    let entity_id = candidate
        .target_authoring_entity_id
        .clone()
        .ok_or_else(|| "Apply candidate has no target authoring entity.".to_string())?;
    let entity = document
        .entity(&entity_id)
        .ok_or_else(|| format!("Authoring entity {entity_id} does not exist."))?;

    if candidate.component_type == "transform" {
        let transform = entity
            .transform
            .ok_or_else(|| format!("Authoring entity {entity_id} has no Transform."))?;
        let mut local_position = transform.local_position;
        let mut local_rotation = transform.local_rotation;
        let mut local_scale = transform.local_scale;
        let mut set_position = false;
        let mut set_rotation = false;
        let mut set_scale = false;
        match candidate.field_path.as_str() {
            "local_position" => {
                local_position = editor_vec3_from_runtime_value(&candidate.runtime_value)?;
                set_position = true;
            }
            "local_rotation" => {
                local_rotation = editor_vec3_from_runtime_value(&candidate.runtime_value)?;
                set_rotation = true;
            }
            "local_scale" => {
                local_scale = editor_vec3_from_runtime_value(&candidate.runtime_value)?;
                set_scale = true;
            }
            "local_position.x" => {
                local_position.x = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_position.x must be a number.".to_string())?
                    as f32;
                set_position = true;
            }
            "local_position.y" => {
                local_position.y = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_position.y must be a number.".to_string())?
                    as f32;
                set_position = true;
            }
            "local_position.z" => {
                local_position.z = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_position.z must be a number.".to_string())?
                    as f32;
                set_position = true;
            }
            "local_rotation.x" => {
                local_rotation.x = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_rotation.x must be a number.".to_string())?
                    as f32;
                set_rotation = true;
            }
            "local_rotation.y" => {
                local_rotation.y = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_rotation.y must be a number.".to_string())?
                    as f32;
                set_rotation = true;
            }
            "local_rotation.z" => {
                local_rotation.z = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_rotation.z must be a number.".to_string())?
                    as f32;
                set_rotation = true;
            }
            "local_scale.x" => {
                local_scale.x = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_scale.x must be a number.".to_string())?
                    as f32;
                set_scale = true;
            }
            "local_scale.y" => {
                local_scale.y = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_scale.y must be a number.".to_string())?
                    as f32;
                set_scale = true;
            }
            "local_scale.z" => {
                local_scale.z = candidate
                    .runtime_value
                    .as_f64()
                    .ok_or_else(|| "Transform local_scale.z must be a number.".to_string())?
                    as f32;
                set_scale = true;
            }
            _ => {
                return Err(format!(
                    "Transform field {} is not applyable to authoring.",
                    candidate.field_path
                ));
            }
        }
        return Ok(SceneEditCommand::SetTransform {
            entity_id,
            local_position: set_position.then_some(local_position),
            local_rotation: set_rotation.then_some(local_rotation),
            local_scale: set_scale.then_some(local_scale),
        });
    }

    let scene_component_type = scene_component_type_for_runtime(&candidate.component_type);
    if scene_component_type == "Dynamic" {
        return Err(format!(
            "Runtime component {} cannot be mapped to an authoring component.",
            candidate.component_type
        ));
    }
    let field_path = scene_field_path_for_runtime(&candidate.field_path);
    Ok(SceneEditCommand::SetComponentField {
        entity_id,
        component_type: scene_component_type.to_string(),
        field_path,
        value: candidate.runtime_value.clone(),
    })
}

impl EditorSession {
    pub(crate) fn open_scene_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: &Path,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_scene_document.file".to_string());
        transaction
            .write_set
            .push("editor_scene_document".to_string());
        transaction.write_set.push("preview_world".to_string());
        transaction.undo_policy = UndoPolicy::FutureUndoable;

        let document = match EditorSceneDocument::load_from_path(path) {
            Ok(document) => document,
            Err(diagnostics) => {
                transaction
                    .diagnostics
                    .extend(scene_diagnostics_to_editor(transaction, diagnostics));
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let (preview_world, sync_report) = match PreviewWorldSync::full_rebuild(&document) {
            Ok(result) => result,
            Err(report) => {
                transaction.diagnostics.extend(scene_diagnostics_to_editor(
                    transaction,
                    report.diagnostics.clone(),
                ));
                self.last_preview_world_sync_report = Some(report);
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        self.runtime_package_path = None;
        self.runtime_package = None;
        self.frame_loop = None;
        self.last_frame_output = None;
        self.selected_trace_entry_id = None;
        self.selected_entity_id = None;
        self.selected_entity_source = None;
        self.selected_aui_node = None;
        self.scene_selection.clear();
        self.scene_undo_stack = SceneUndoStack::default();
        self.scene_path = Some(path.to_path_buf());
        self.editor_scene_document = Some(document);
        self.world = Some(preview_world);
        self.last_preview_world_sync_report = Some(sync_report);
        self.push_info(
            transaction,
            "editor.scene_document.opened",
            format!("Opened editable Scene {}", path.display()),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn execute_scene_edit(
        &mut self,
        transaction: &mut CommandTransaction,
        command: SceneEditCommand,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_scene_document".to_string());
        transaction
            .write_set
            .push("editor_scene_document".to_string());
        transaction.write_set.push("preview_world".to_string());
        transaction.undo_policy = UndoPolicy::SnapshotReady;
        let Some(document) = &mut self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.scene_document.not_loaded",
                "Cannot edit Scene before opening an editable Scene document.",
                Some("Open a Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let request = SceneEditRequest {
            request_id: transaction.request_id.clone(),
            source: scene_edit_source_for_ui_source(&transaction.source),
            target_scene_id: document.scene_id.clone(),
            command,
        };
        let report = SceneEditTransaction::apply(
            transaction.transaction_id.clone(),
            document,
            &mut self.scene_selection,
            &mut self.scene_undo_stack,
            request,
        );
        transaction.diagnostics.extend(scene_diagnostics_to_editor(
            transaction,
            report.diagnostics.clone(),
        ));
        transaction.read_set.extend(report.read_set.clone());
        transaction.write_set.extend(report.write_set.clone());
        self.selected_entity_id = self.scene_selection.primary_entity_id.clone();
        self.selected_entity_source = self
            .selected_entity_id
            .as_ref()
            .map(|_| EntitySelectionSource::AuthoringScene);
        if self.selected_entity_id.is_some() {
            self.selected_aui_node = None;
        }
        self.last_scene_edit_report = Some(report.clone());

        let status = match report.status {
            SceneEditTransactionStatus::Committed => {
                match PreviewWorldSync::full_rebuild(document) {
                    Ok((preview_world, sync_report)) => {
                        self.world = Some(preview_world);
                        self.last_preview_world_sync_report = Some(sync_report);
                    }
                    Err(sync_report) => {
                        transaction.diagnostics.extend(scene_diagnostics_to_editor(
                            transaction,
                            sync_report.diagnostics.clone(),
                        ));
                        self.last_preview_world_sync_report = Some(sync_report);
                        return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
                    }
                }
                self.push_info(
                    transaction,
                    "editor.scene_edit.committed",
                    format!("Scene edit committed: {}", report.command_kind),
                );
                CommandStatus::Committed
            }
            SceneEditTransactionStatus::Rejected => {
                self.push_error(
                    transaction,
                    "editor.scene_edit.rejected",
                    format!("Scene edit rejected: {}", report.command_kind),
                    Some("Check Console diagnostics for the rejected SceneEditCommand."),
                );
                CommandStatus::Rejected
            }
            SceneEditTransactionStatus::Failed => CommandStatus::Failed,
            SceneEditTransactionStatus::Pending => CommandStatus::Failed,
        };
        self.finish_transaction(transaction.clone(), status)
    }

    pub(crate) fn place_asset_into_scene(
        &mut self,
        transaction: &mut CommandTransaction,
        request: AssetPlacementRequest,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_scene_document".to_string());
        transaction
            .write_set
            .push("editor_scene_document".to_string());
        let Some(document) = &self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.scene_document.not_loaded",
                "Cannot place an asset before opening an editable Scene document.",
                Some("Open a Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let plan = AssetPlacementResolver::resolve(
            document,
            self.scene_selection.primary_entity_id.as_deref(),
            request,
        );
        if plan.scene_commands.is_empty() {
            transaction
                .diagnostics
                .extend(asset_placement_diagnostics_to_editor(
                    transaction,
                    &plan.report,
                ));
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        transaction
            .diagnostics
            .extend(asset_placement_diagnostics_to_editor(
                transaction,
                &plan.report,
            ));
        let command = plan
            .scene_commands
            .into_iter()
            .next()
            .expect("non-empty asset placement plan should contain a command");
        self.execute_scene_edit(transaction, command)
    }

    pub(crate) fn save_scene_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: Option<PathBuf>,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_scene_document".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let fallback_root = path
            .as_ref()
            .and_then(|path| path.parent())
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| {
                self.scene_path
                    .as_ref()
                    .and_then(|path| path.parent())
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
            });
        let write_scope = self
            .active_project_session
            .as_ref()
            .map(|session| session.write_scope().clone())
            .or_else(|| {
                fallback_root
                    .as_ref()
                    .and_then(|root| crate::ProjectWriteScope::open(root).ok())
            });
        let Some(write_scope) = write_scope else {
            self.push_error(
                transaction,
                "editor.scene_document.no_project",
                "Cannot save Scene without a project or legacy fixture write scope.",
                Some("Open a project or save beneath the opened Scene fixture root."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(document) = &mut self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.scene_document.not_loaded",
                "Cannot save Scene before opening an editable Scene document.",
                Some("Open a Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let report = SceneSavePipeline::save_in_scope(document, &write_scope, path.as_ref());
        transaction.diagnostics.extend(scene_diagnostics_to_editor(
            transaction,
            report.diagnostics.clone(),
        ));
        match report.status {
            SceneSaveStatus::Saved => {
                transaction
                    .write_set
                    .push("editor_scene_document.file".to_string());
                self.push_info(
                    transaction,
                    "editor.scene_document.saved",
                    format!("Saved Scene {}", report.path.display()),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            SceneSaveStatus::Unchanged => {
                self.push_info(
                    transaction,
                    "editor.scene_document.unchanged",
                    format!("No Scene changes to save for {}", report.path.display()),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            SceneSaveStatus::Failed => {
                self.push_error(
                    transaction,
                    "editor.scene_document.save_failed",
                    "Scene save failed.",
                    Some("Check path and validation diagnostics."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn select_entity(
        &mut self,
        transaction: &mut CommandTransaction,
        entity_id: &str,
    ) -> CommandResult {
        transaction
            .read_set
            .push("runtime.world.entities".to_string());
        transaction
            .write_set
            .push("selection.selected_entity_id".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let Some(world) = &self.world else {
            self.push_error(
                transaction,
                "editor.runtime_package.not_loaded",
                "Cannot select an entity before opening a Runtime Package.",
                Some("Open a Runtime Package first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if world
            .entity(&EntityId::new(entity_id.to_string()))
            .is_none()
        {
            let mut diagnostic = self.make_diagnostic(
                transaction,
                DiagnosticSeverity::Warning,
                "editor.entity.not_found",
                format!(
                    "Entity {} does not exist in current runtime scene.",
                    entity_id
                ),
                Some("Refresh hierarchy or select an existing entity."),
            );
            diagnostic.entity_id = Some(entity_id.to_string());
            transaction.diagnostics.push(diagnostic);
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let previous = self.selected_entity_id.clone();
        let previous_source = self
            .selected_entity_source
            .map(EntitySelectionSource::as_str);
        self.selected_entity_id = Some(entity_id.to_string());
        self.selected_entity_source = Some(EntitySelectionSource::OpenedRuntimePackage);
        self.selected_aui_node = None;
        transaction.state_changes.push(StateChangeSummary {
            kind: "selection.changed".to_string(),
            path: "selection.selected_entity_id".to_string(),
            before_summary: previous,
            after_summary: Some(entity_id.to_string()),
        });
        transaction.state_changes.push(StateChangeSummary {
            kind: "selection.source.changed".to_string(),
            path: "selection.selected_entity_source".to_string(),
            before_summary: previous_source.map(str::to_string),
            after_summary: Some(
                EntitySelectionSource::OpenedRuntimePackage
                    .as_str()
                    .to_string(),
            ),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn pick_runtime_entity_at(
        &mut self,
        transaction: &mut CommandTransaction,
        request: RuntimeWorldPickRequest,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_gameview_play.runtime_world".to_string());
        transaction
            .write_set
            .push("selection.selected_entity_id".to_string());
        transaction
            .write_set
            .push("selection.selected_entity_source".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let Some(instance) = self.editor_runtime_play_instance.as_ref() else {
            self.push_error(
                transaction,
                "editor.gameview_play.no_active_runtime",
                "Cannot pick a runtime entity because no active Editor GameView runtime exists.",
                Some("Start Play first, then pause or use runtime inspect mode."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };

        let pick = WorldPickCollector::pick(instance.runtime_world(), request);
        let mut diagnostic = self.make_diagnostic(
            transaction,
            DiagnosticSeverity::Info,
            "editor.runtime_selection.pick",
            format!(
                "Runtime pick status={} candidates={} diagnostic={}",
                pick.status.as_str(),
                pick.candidate_count,
                pick.diagnostic
            ),
            None,
        );
        diagnostic.entity_id = pick.selected_entity_id.clone();
        transaction.diagnostics.push(diagnostic);

        if pick.status != RuntimePickStatus::Hit {
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }

        let Some(entity_id) = pick.selected_entity_id.clone() else {
            self.push_error(
                transaction,
                "editor.runtime_selection.hit_without_entity",
                "Runtime pick returned hit without an entity id.",
                Some("Inspect WorldPickCollector diagnostics."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };

        let previous = self.selected_entity_id.clone();
        let previous_source = self
            .selected_entity_source
            .map(EntitySelectionSource::as_str);
        self.selected_entity_id = Some(entity_id.clone());
        self.selected_entity_source = Some(EntitySelectionSource::ActiveGameViewRuntime);
        self.selected_aui_node = None;
        transaction.state_changes.push(StateChangeSummary {
            kind: "selection.changed".to_string(),
            path: "selection.selected_entity_id".to_string(),
            before_summary: previous,
            after_summary: Some(entity_id),
        });
        transaction.state_changes.push(StateChangeSummary {
            kind: "selection.source.changed".to_string(),
            path: "selection.selected_entity_source".to_string(),
            before_summary: previous_source.map(str::to_string),
            after_summary: Some(
                EntitySelectionSource::ActiveGameViewRuntime
                    .as_str()
                    .to_string(),
            ),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn select_runtime_entity(
        &mut self,
        transaction: &mut CommandTransaction,
        entity_id: &str,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_gameview_play.runtime_world".to_string());
        transaction
            .write_set
            .push("selection.selected_entity_id".to_string());
        transaction
            .write_set
            .push("selection.selected_entity_source".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let Some(instance) = self.editor_runtime_play_instance.as_ref() else {
            self.push_error(
                transaction,
                "editor.gameview_play.no_active_runtime",
                "Cannot select a runtime entity because no active Editor GameView runtime exists.",
                Some("Start Play first, then select a runtime hierarchy node."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if instance
            .runtime_world()
            .entity(&EntityId::new(entity_id.to_string()))
            .is_none()
        {
            let mut diagnostic = self.make_diagnostic(
                transaction,
                DiagnosticSeverity::Warning,
                "editor.runtime_selection.entity_not_found",
                format!(
                    "Runtime entity {} does not exist in the active runtime World.",
                    entity_id
                ),
                Some("Refresh the runtime Hierarchy or select an existing runtime entity."),
            );
            diagnostic.entity_id = Some(entity_id.to_string());
            transaction.diagnostics.push(diagnostic);
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }

        let previous = self.selected_entity_id.clone();
        let previous_source = self
            .selected_entity_source
            .map(EntitySelectionSource::as_str);
        self.selected_entity_id = Some(entity_id.to_string());
        self.selected_entity_source = Some(EntitySelectionSource::ActiveGameViewRuntime);
        self.selected_aui_node = None;
        transaction.state_changes.push(StateChangeSummary {
            kind: "selection.changed".to_string(),
            path: "selection.selected_entity_id".to_string(),
            before_summary: previous,
            after_summary: Some(entity_id.to_string()),
        });
        transaction.state_changes.push(StateChangeSummary {
            kind: "selection.source.changed".to_string(),
            path: "selection.selected_entity_source".to_string(),
            before_summary: previous_source.map(str::to_string),
            after_summary: Some(
                EntitySelectionSource::ActiveGameViewRuntime
                    .as_str()
                    .to_string(),
            ),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn preview_apply_runtime_change_to_authoring(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_gameview_play.temporary_edit_records".to_string());
        transaction
            .read_set
            .push("editor_scene_document.scene_id".to_string());
        transaction.undo_policy = UndoPolicy::None;

        let authoring_scene_id = self
            .editor_scene_document
            .as_ref()
            .map(|document| document.scene_id.clone());
        let Some(instance) = self.editor_runtime_play_instance.as_ref() else {
            self.push_error(
                transaction,
                "editor.runtime_apply.no_active_runtime",
                "Cannot preview runtime apply candidates because no active Editor GameView runtime exists.",
                Some("Start Play first, then make a temporary runtime Inspector edit."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let report =
            instance.preview_apply_runtime_change_to_authoring(authoring_scene_id.as_deref());
        transaction.state_changes.push(StateChangeSummary {
            kind: "runtime.apply.preview".to_string(),
            path: "editor_gameview_play.apply_runtime_change_report".to_string(),
            before_summary: None,
            after_summary: Some(format!(
                "candidate_count={} ready_count={} blocked_count={}",
                report.candidate_count, report.ready_count, report.blocked_count
            )),
        });
        self.last_runtime_apply_report = Some(report.clone());
        self.push_info(
            transaction,
            "editor.runtime_apply.preview_ready",
            format!(
                "Runtime apply preview generated: candidates={} ready={} blocked={}.",
                report.candidate_count, report.ready_count, report.blocked_count
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_runtime_component_field_temporary(
        &mut self,
        transaction: &mut CommandTransaction,
        entity_id: String,
        component_type: String,
        field_path: String,
        value: serde_json::Value,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_gameview_play.runtime_world".to_string());
        transaction
            .write_set
            .push("editor_gameview_play.runtime_world.temporary_edit".to_string());
        transaction
            .write_set
            .push("editor_gameview_play.temporary_edit_summary".to_string());
        transaction
            .write_set
            .push("editor_gameview_play.temporary_edit_records".to_string());
        transaction.undo_policy = UndoPolicy::None;

        if self.selected_entity_source != Some(EntitySelectionSource::ActiveGameViewRuntime) {
            self.push_error(
                transaction,
                "editor.runtime_temporary_edit.entity_not_active_runtime",
                "Runtime temporary edits require an active runtime selection.",
                Some("Select an entity from the Play Mode Hierarchy or GameView first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        if self.selected_entity_id.as_deref() != Some(entity_id.as_str()) {
            self.push_error(
                transaction,
                "editor.runtime_temporary_edit.selection_mismatch",
                "Runtime temporary edit entity does not match the active runtime selection.",
                Some("Refresh the Inspector or select the entity again."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }

        let Some(instance) = self.editor_runtime_play_instance.as_mut() else {
            self.push_error(
                transaction,
                "editor.gameview_play.no_active_runtime",
                "Cannot edit a runtime entity because no active Editor GameView runtime exists.",
                Some("Start Play first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };

        match instance.apply_temporary_component_edit(
            &entity_id,
            &component_type,
            &field_path,
            value,
        ) {
            Ok(record) => {
                transaction.state_changes.push(StateChangeSummary {
                    kind: "runtime.temporary_edit.applied".to_string(),
                    path: format!("runtime.{}.{}.{}", entity_id, component_type, field_path),
                    before_summary: record.before_summary.clone(),
                    after_summary: Some(record.after_summary.clone()),
                });
                transaction.state_changes.push(StateChangeSummary {
                    kind: "runtime.temporary_edit.summary".to_string(),
                    path: "editor_gameview_play.temporary_edit_summary".to_string(),
                    before_summary: None,
                    after_summary: Some(format!(
                        "edited_entity_count={} edited_field_count={} last_field={}",
                        instance.temporary_edit_summary().edited_entity_count(),
                        instance.temporary_edit_summary().edited_field_count,
                        instance
                            .temporary_edit_summary()
                            .last_edited_field_path
                            .as_deref()
                            .unwrap_or("none")
                    )),
                });
                self.push_info(
                    transaction,
                    "editor.runtime_temporary_edit.applied",
                    format!(
                        "Applied Play Mode temporary edit: entity={} component={} field={}.",
                        record.entity_id, record.component_type, record.field_path
                    ),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => {
                self.push_error(
                    transaction,
                    &format!("editor.runtime_temporary_edit.{}", error.code),
                    error.message,
                    Some("Use a C-min allowed runtime temporary field and matching value type."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Rejected)
            }
        }
    }

    pub(crate) fn apply_runtime_change_to_authoring(
        &mut self,
        transaction: &mut CommandTransaction,
        edit_id: String,
        candidate_hash: String,
    ) -> CommandResult {
        transaction
            .read_set
            .push("editor_gameview_play.runtime_world".to_string());
        transaction
            .read_set
            .push("editor_gameview_play.temporary_edit_records".to_string());
        transaction
            .read_set
            .push("editor_scene_document".to_string());
        transaction
            .write_set
            .push("editor_scene_document".to_string());
        transaction.write_set.push("preview_world".to_string());
        transaction
            .write_set
            .push("editor_gameview_play.temporary_edit_records.applied".to_string());
        transaction.undo_policy = UndoPolicy::SnapshotReady;

        let Some(authoring_scene_id) = self
            .editor_scene_document
            .as_ref()
            .map(|document| document.scene_id.clone())
        else {
            self.push_error(
                transaction,
                "editor.runtime_apply.authoring_scene_not_loaded",
                "Cannot apply runtime change because no editable authoring Scene is loaded.",
                Some("Open the matching Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };

        let candidate = {
            let Some(instance) = self.editor_runtime_play_instance.as_ref() else {
                self.push_error(
                    transaction,
                    "editor.runtime_apply.no_active_runtime",
                    "Cannot apply runtime change because no active Editor GameView runtime exists.",
                    Some("Start Play first, then make a temporary runtime Inspector edit."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            };
            match instance.confirm_apply_runtime_change_candidate(
                &edit_id,
                &candidate_hash,
                Some(&authoring_scene_id),
            ) {
                Ok(candidate) => candidate,
                Err(report) => {
                    let status = if report
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.contains("stale_candidate_hash"))
                    {
                        CommandStatus::Rejected
                    } else {
                        CommandStatus::Rejected
                    };
                    self.last_runtime_apply_report = Some(report.clone());
                    self.push_error(
                        transaction,
                        "editor.runtime_apply.candidate_rejected",
                        format!(
                            "Runtime apply candidate was rejected: {}.",
                            report.diagnostics.join(", ")
                        ),
                        Some("Refresh the preview candidates and apply the latest candidate hash."),
                    );
                    return self.finish_transaction(transaction.clone(), status);
                }
            }
        };

        let scene_command = {
            let document = self
                .editor_scene_document
                .as_ref()
                .expect("authoring scene was checked above");
            match scene_edit_command_for_runtime_apply(document, &candidate) {
                Ok(command) => command,
                Err(message) => {
                    let report = ApplyRuntimeChangeReport::from_candidates(
                        &authoring_scene_id,
                        "apply_runtime_change_to_authoring",
                        vec![candidate],
                        vec![format!("error:authoring_command_build_failed:{message}")],
                        None,
                    );
                    self.last_runtime_apply_report = Some(report);
                    self.push_error(
                        transaction,
                        "editor.runtime_apply.command_build_failed",
                        message,
                        Some(
                            "Only B-min mapped Scene-origin fields can be applied in this version.",
                        ),
                    );
                    return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
                }
            }
        };

        let Some(document) = &mut self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.runtime_apply.authoring_scene_not_loaded",
                "Cannot apply runtime change because no editable authoring Scene is loaded.",
                Some("Open the matching Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };

        let request = SceneEditRequest {
            request_id: transaction.request_id.clone(),
            source: scene_edit_source_for_ui_source(&transaction.source),
            target_scene_id: document.scene_id.clone(),
            command: scene_command,
        };
        let report = SceneEditTransaction::apply(
            transaction.transaction_id.clone(),
            document,
            &mut self.scene_selection,
            &mut self.scene_undo_stack,
            request,
        );
        transaction.diagnostics.extend(scene_diagnostics_to_editor(
            transaction,
            report.diagnostics.clone(),
        ));
        transaction.read_set.extend(report.read_set.clone());
        transaction.write_set.extend(report.write_set.clone());
        self.last_scene_edit_report = Some(report.clone());

        let status = match report.status {
            SceneEditTransactionStatus::Committed => {
                match PreviewWorldSync::full_rebuild(document) {
                    Ok((preview_world, sync_report)) => {
                        self.world = Some(preview_world);
                        self.last_preview_world_sync_report = Some(sync_report);
                    }
                    Err(sync_report) => {
                        transaction.diagnostics.extend(scene_diagnostics_to_editor(
                            transaction,
                            sync_report.diagnostics.clone(),
                        ));
                        self.last_preview_world_sync_report = Some(sync_report);
                        return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
                    }
                }
                if let Some(instance) = self.editor_runtime_play_instance.as_mut() {
                    instance.mark_runtime_temporary_edit_applied(&edit_id);
                }
                let apply_report = ApplyRuntimeChangeReport::from_candidates(
                    &authoring_scene_id,
                    "apply_runtime_change_to_authoring",
                    vec![candidate.clone()],
                    vec!["info:authoring_scene_updated".to_string()],
                    Some(edit_id.clone()),
                );
                self.last_runtime_apply_report = Some(apply_report);
                transaction.state_changes.push(StateChangeSummary {
                    kind: "runtime.apply.authoring_committed".to_string(),
                    path: candidate
                        .target_authoring_path
                        .clone()
                        .unwrap_or_else(|| "editor_scene_document".to_string()),
                    before_summary: None,
                    after_summary: Some(format!(
                        "edit_id={} candidate_hash={}",
                        candidate.edit_id, candidate.candidate_hash
                    )),
                });
                self.push_info(
                    transaction,
                    "editor.runtime_apply.authoring_committed",
                    format!(
                        "Applied runtime change {} to authoring Scene.",
                        candidate.edit_id
                    ),
                );
                CommandStatus::Committed
            }
            SceneEditTransactionStatus::Rejected => {
                self.push_error(
                    transaction,
                    "editor.runtime_apply.scene_edit_rejected",
                    "Runtime apply target SceneEditCommand was rejected.",
                    Some("Check Console diagnostics for the rejected SceneEditCommand."),
                );
                CommandStatus::Rejected
            }
            SceneEditTransactionStatus::Failed | SceneEditTransactionStatus::Pending => {
                CommandStatus::Failed
            }
        };
        self.finish_transaction(transaction.clone(), status)
    }
}
