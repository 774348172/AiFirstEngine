use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use editor_ui_model::{ConsoleEntry, DiagnosticSeverity, EditorDiagnostic, UiCommand};
use engine_runtime::world::World;

use crate::{
    command_id_for_payload, EditorSceneDocument, EditorSession, EditorTransform,
    InputMappingAuthoringService, InputMappingEditCommand, InputMappingEditorState,
    PrefabWorkflowService, PreviewWorldSyncReport, ProjectRelativePath, ProjectWriteScope,
    SceneEditTransactionReport, SceneSelection, SceneUndoStack,
};

use super::{
    AssetPatchOperation, AuiPatchOperation, InputBindingProcessorPatch, InputPatchOperation,
    PatchApplier, PatchApplyReport, PatchApplyStatus, PatchDiagnostic, PatchDiagnosticSeverity,
    PatchHistoryEntry, PatchOperation, PatchOperationApplyStatus, PatchOperationResult,
    PatchSource, PatchValidator, PrefabPatchOperation, ProjectPatchDocument, RulePatchOperation,
    ScenePatchOperation,
};

struct EditorSessionPatchRollbackSnapshot {
    selected_project_browser_path: Option<String>,
    input_mapping_editor_state: Option<InputMappingEditorState>,
    editor_scene_document: Option<EditorSceneDocument>,
    scene_selection: SceneSelection,
    scene_undo_stack: SceneUndoStack,
    scene_path: Option<PathBuf>,
    last_scene_edit_report: Option<SceneEditTransactionReport>,
    last_preview_world_sync_report: Option<PreviewWorldSyncReport>,
    world: Option<World>,
    selected_entity_id: Option<String>,
    console_entries: Vec<ConsoleEntry>,
    diagnostics: Vec<EditorDiagnostic>,
    revision: u64,
}

#[derive(Debug, Clone)]
struct ProjectFileSnapshot {
    relative_path: ProjectRelativePath,
    existed_before: bool,
    before_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProjectFileSnapshotSet {
    scope: Option<ProjectWriteScope>,
    snapshots: Vec<ProjectFileSnapshot>,
}

impl ProjectFileSnapshotSet {
    fn capture(
        scope: &ProjectWriteScope,
        relative_paths: Vec<String>,
    ) -> Result<Self, PatchDiagnostic> {
        let mut snapshots = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for relative_path in relative_paths {
            if relative_path.trim().is_empty() {
                continue;
            }
            let relative_path = ProjectRelativePath::parse(&relative_path).map_err(|error| {
                PatchDiagnostic::error(
                    "project_patch.file_snapshot.path_outside_project",
                    format!("Project file snapshot path is invalid: {error}"),
                    None,
                    Some(relative_path.clone()),
                )
            })?;
            if !seen.insert(relative_path.to_string()) {
                continue;
            }
            let existed_before = scope.try_exists(relative_path.as_path()).map_err(|error| {
                PatchDiagnostic::error(
                    "project_patch.file_snapshot.read_failed",
                    format!("Failed to inspect project file snapshot: {error}"),
                    None,
                    Some(relative_path.to_string()),
                )
            })?;
            let before_bytes = if existed_before {
                Some(scope.read(relative_path.as_path()).map_err(|error| {
                    PatchDiagnostic::error(
                        "project_patch.file_snapshot.read_failed",
                        format!("Failed to snapshot project file: {error}"),
                        None,
                        Some(relative_path.to_string()),
                    )
                })?)
            } else {
                None
            };
            snapshots.push(ProjectFileSnapshot {
                relative_path,
                existed_before,
                before_bytes,
            });
        }
        Ok(Self {
            scope: Some(scope.clone()),
            snapshots,
        })
    }

    fn restore(&self) -> Result<(), PatchDiagnostic> {
        let Some(scope) = &self.scope else {
            return Ok(());
        };
        for snapshot in self.snapshots.iter().rev() {
            if snapshot.existed_before {
                let Some(before_bytes) = snapshot.before_bytes.as_ref() else {
                    return Err(PatchDiagnostic::error(
                        "project_patch.file_snapshot.restore_missing_bytes",
                        "Cannot restore project file snapshot because before bytes are missing.",
                        None,
                        Some(snapshot.relative_path.to_string()),
                    ));
                };
                scope
                    .write_atomic(snapshot.relative_path.as_path(), before_bytes)
                    .map_err(|error| {
                        PatchDiagnostic::error(
                            "project_write.rollback_containment_changed",
                            format!("Failed to restore project file snapshot: {error}"),
                            None,
                            Some(snapshot.relative_path.to_string()),
                        )
                    })?;
            } else {
                scope
                    .remove_file(snapshot.relative_path.as_path())
                    .map_err(|error| {
                        PatchDiagnostic::error(
                            "project_write.rollback_containment_changed",
                            format!(
                            "Failed to remove newly-created project file during rollback: {error}"
                        ),
                            None,
                            Some(snapshot.relative_path.to_string()),
                        )
                    })?;
            }
        }
        Ok(())
    }
}

impl EditorSession {
    pub fn execute_patch_as_transaction(
        &mut self,
        patch: ProjectPatchDocument,
    ) -> PatchApplyReport {
        self.execute_patch_inner(patch, true, true)
    }

    fn execute_patch_inner(
        &mut self,
        patch: ProjectPatchDocument,
        record_history: bool,
        require_inverse: bool,
    ) -> PatchApplyReport {
        let mut validation = PatchValidator::validate(self, &patch);
        if !validation.accepted {
            return PatchApplyReport {
                patch_id: patch.patch_id.clone(),
                status: PatchApplyStatus::Rejected,
                validation,
                operation_results: Vec::new(),
                inverse_patch: None,
            };
        }

        let mut project_file_paths = project_file_snapshot_paths(&patch);
        if patch_mutates_scene(&patch) {
            let Some(scene_path) = self.scene_path.as_ref() else {
                validation.accepted = false;
                validation.diagnostics.push(PatchDiagnostic::error(
                    "project_patch.file_snapshot.scene_path_missing",
                    "Scene-mutating ProjectPatch requires an active Scene path.",
                    None,
                    Some("scene.active_path".to_string()),
                ));
                return PatchApplyReport {
                    patch_id: patch.patch_id.clone(),
                    status: PatchApplyStatus::Rejected,
                    validation,
                    operation_results: Vec::new(),
                    inverse_patch: None,
                };
            };
            let Some(project_session) = self.active_project_session.as_ref() else {
                validation.accepted = false;
                validation.diagnostics.push(PatchDiagnostic::error(
                    "project_patch.file_snapshot.no_project",
                    "Scene file snapshot requires an active project.",
                    None,
                    Some("project_session".to_string()),
                ));
                return PatchApplyReport {
                    patch_id: patch.patch_id.clone(),
                    status: PatchApplyStatus::Rejected,
                    validation,
                    operation_results: Vec::new(),
                    inverse_patch: None,
                };
            };
            match project_relative_scene_snapshot_path(scene_path, &project_session.project_root) {
                Ok(relative_path) => project_file_paths.push(relative_path),
                Err(diagnostic) => {
                    validation.accepted = false;
                    validation.diagnostics.push(diagnostic);
                    return PatchApplyReport {
                        patch_id: patch.patch_id.clone(),
                        status: PatchApplyStatus::Rejected,
                        validation,
                        operation_results: Vec::new(),
                        inverse_patch: None,
                    };
                }
            }
        }
        let file_snapshot = if project_file_paths.is_empty() {
            ProjectFileSnapshotSet::default()
        } else {
            let Some(project_session) = self.active_project_session.as_ref() else {
                validation.accepted = false;
                validation.diagnostics.push(PatchDiagnostic::error(
                    "project_patch.file_snapshot.no_project",
                    "Project file snapshot requires an active project.",
                    None,
                    Some("project_session".to_string()),
                ));
                return PatchApplyReport {
                    patch_id: patch.patch_id.clone(),
                    status: PatchApplyStatus::Rejected,
                    validation,
                    operation_results: Vec::new(),
                    inverse_patch: None,
                };
            };
            match ProjectFileSnapshotSet::capture(project_session.write_scope(), project_file_paths)
            {
                Ok(snapshot) => snapshot,
                Err(diagnostic) => {
                    validation.accepted = false;
                    validation.diagnostics.push(diagnostic);
                    return PatchApplyReport {
                        patch_id: patch.patch_id.clone(),
                        status: PatchApplyStatus::Rejected,
                        validation,
                        operation_results: Vec::new(),
                        inverse_patch: None,
                    };
                }
            }
        };

        let inverse_patch = match (require_inverse, self.build_inverse_patch(&patch)) {
            (_, Ok(inverse_patch)) => Some(inverse_patch),
            (false, Err(_)) => None,
            (true, Err(diagnostic)) => {
                let mut validation = validation;
                validation.accepted = false;
                validation.diagnostics.push(diagnostic);
                return PatchApplyReport {
                    patch_id: patch.patch_id.clone(),
                    status: PatchApplyStatus::Rejected,
                    validation,
                    operation_results: Vec::new(),
                    inverse_patch: None,
                };
            }
        };

        let snapshot = self.patch_rollback_snapshot();
        let commands = PatchApplier::expand(&patch);
        let mut operation_results = Vec::new();
        let mut dirty_input_paths = BTreeSet::new();
        let mut dirty_scene = false;
        for (operation, payload) in patch.operations.iter().zip(commands) {
            if matches!(operation, PatchOperation::Build(_)) && !dirty_input_paths.is_empty() {
                if let Err(diagnostic) =
                    self.commit_input_patch_drafts(&patch.patch_id, &dirty_input_paths)
                {
                    self.restore_patch_rollback_snapshot(snapshot);
                    let _ = file_snapshot.restore();
                    operation_results.push(PatchOperationResult {
                        operation_id: operation.operation_id().to_string(),
                        kind: operation.kind().to_string(),
                        status: PatchOperationApplyStatus::Failed,
                        command_id: Some("save_input_mapping".to_string()),
                        diagnostics: vec![diagnostic],
                    });
                    for pending in patch.operations.iter().skip(operation_results.len()) {
                        operation_results.push(PatchOperationResult {
                            operation_id: pending.operation_id().to_string(),
                            kind: pending.kind().to_string(),
                            status: PatchOperationApplyStatus::Skipped,
                            command_id: None,
                            diagnostics: Vec::new(),
                        });
                    }
                    return PatchApplyReport {
                        patch_id: patch.patch_id.clone(),
                        status: PatchApplyStatus::Failed,
                        validation,
                        operation_results,
                        inverse_patch,
                    };
                }
                dirty_input_paths.clear();
            }
            if matches!(operation, PatchOperation::Build(_)) && dirty_scene {
                if let Err(diagnostic) = self.commit_scene_patch_draft(&patch.patch_id) {
                    self.restore_patch_rollback_snapshot(snapshot);
                    let _ = file_snapshot.restore();
                    operation_results.push(PatchOperationResult {
                        operation_id: operation.operation_id().to_string(),
                        kind: operation.kind().to_string(),
                        status: PatchOperationApplyStatus::Failed,
                        command_id: Some("save_scene_document".to_string()),
                        diagnostics: vec![diagnostic],
                    });
                    for pending in patch.operations.iter().skip(operation_results.len()) {
                        operation_results.push(PatchOperationResult {
                            operation_id: pending.operation_id().to_string(),
                            kind: pending.kind().to_string(),
                            status: PatchOperationApplyStatus::Skipped,
                            command_id: None,
                            diagnostics: Vec::new(),
                        });
                    }
                    return PatchApplyReport {
                        patch_id: patch.patch_id.clone(),
                        status: PatchApplyStatus::Failed,
                        validation,
                        operation_results,
                        inverse_patch,
                    };
                }
                dirty_scene = false;
            }
            let command_id = command_id_for_payload(&payload).to_string();
            let result = self.execute_command(UiCommand {
                command_id: command_id.clone(),
                source: editor_ui_model::UiCommandSource::AiAssistant,
                request_id: format!(
                    "request-project-patch-{}-{}",
                    patch.patch_id,
                    operation.operation_id()
                ),
                payload,
            });
            let status = match result.status {
                crate::CommandStatus::Committed => PatchOperationApplyStatus::Committed,
                crate::CommandStatus::Rejected => PatchOperationApplyStatus::Rejected,
                crate::CommandStatus::Failed => PatchOperationApplyStatus::Failed,
                crate::CommandStatus::Pending | crate::CommandStatus::Validated => {
                    PatchOperationApplyStatus::Failed
                }
            };
            let diagnostics = result
                .diagnostics
                .iter()
                .map(|diagnostic| PatchDiagnostic {
                    severity: match diagnostic.severity {
                        DiagnosticSeverity::Info => PatchDiagnosticSeverity::Info,
                        DiagnosticSeverity::Warning => PatchDiagnosticSeverity::Warning,
                        DiagnosticSeverity::Error => PatchDiagnosticSeverity::Error,
                    },
                    code: diagnostic.code.clone(),
                    message: diagnostic.message.clone(),
                    operation_id: Some(operation.operation_id().to_string()),
                    target: Some(operation.target_summary()),
                })
                .collect::<Vec<_>>();
            operation_results.push(PatchOperationResult {
                operation_id: operation.operation_id().to_string(),
                kind: operation.kind().to_string(),
                status,
                command_id: Some(command_id),
                diagnostics,
            });
            if status == PatchOperationApplyStatus::Committed {
                if let PatchOperation::Input(input_operation) = operation {
                    match input_operation {
                        InputPatchOperation::DeleteInputMapping { path, .. } => {
                            dirty_input_paths.remove(path);
                        }
                        _ => {
                            dirty_input_paths.insert(input_operation.path().to_string());
                        }
                    }
                }
                dirty_scene |= operation_mutates_scene(operation);
            }
            if status != PatchOperationApplyStatus::Committed {
                if !matches!(operation, PatchOperation::Build(_)) {
                    self.restore_patch_rollback_snapshot(snapshot);
                    if let Err(diagnostic) = file_snapshot.restore() {
                        if let Some(result) = operation_results.last_mut() {
                            result.diagnostics.push(diagnostic);
                        }
                    }
                }
                for pending in patch.operations.iter().skip(operation_results.len()) {
                    operation_results.push(PatchOperationResult {
                        operation_id: pending.operation_id().to_string(),
                        kind: pending.kind().to_string(),
                        status: PatchOperationApplyStatus::Skipped,
                        command_id: None,
                        diagnostics: Vec::new(),
                    });
                }
                return PatchApplyReport {
                    patch_id: patch.patch_id.clone(),
                    status: PatchApplyStatus::Failed,
                    validation,
                    operation_results,
                    inverse_patch,
                };
            }
        }

        if let Err(diagnostic) = self.commit_input_patch_drafts(&patch.patch_id, &dirty_input_paths)
        {
            self.restore_patch_rollback_snapshot(snapshot);
            let _ = file_snapshot.restore();
            if let Some(result) = operation_results.iter_mut().rev().find(|result| {
                patch.operations.iter().any(|operation| {
                    operation.operation_id() == result.operation_id
                        && matches!(operation, PatchOperation::Input(_))
                })
            }) {
                result.status = PatchOperationApplyStatus::Failed;
                result.diagnostics.push(diagnostic);
            }
            return PatchApplyReport {
                patch_id: patch.patch_id.clone(),
                status: PatchApplyStatus::Failed,
                validation,
                operation_results,
                inverse_patch,
            };
        }
        if dirty_scene {
            if let Err(diagnostic) = self.commit_scene_patch_draft(&patch.patch_id) {
                self.restore_patch_rollback_snapshot(snapshot);
                let _ = file_snapshot.restore();
                if let Some(result) = operation_results.iter_mut().rev().find(|result| {
                    patch.operations.iter().any(|operation| {
                        operation.operation_id() == result.operation_id
                            && operation_mutates_scene(operation)
                    })
                }) {
                    result.status = PatchOperationApplyStatus::Failed;
                    result.diagnostics.push(diagnostic);
                }
                return PatchApplyReport {
                    patch_id: patch.patch_id.clone(),
                    status: PatchApplyStatus::Failed,
                    validation,
                    operation_results,
                    inverse_patch,
                };
            }
        }

        let report = PatchApplyReport {
            patch_id: patch.patch_id.clone(),
            status: PatchApplyStatus::Committed,
            validation,
            operation_results,
            inverse_patch: inverse_patch.clone(),
        };
        if record_history {
            if let Some(inverse_patch) = inverse_patch {
                self.patch_file_snapshot_history
                    .push((patch.patch_id.clone(), file_snapshot));
                self.patch_history.record(PatchHistoryEntry {
                    patch_id: patch.patch_id.clone(),
                    applied_at: patch.created_at.clone(),
                    original_patch: patch,
                    inverse_patch,
                    apply_report: report.clone(),
                });
            }
        }
        report
    }

    pub fn revert_last_patch_for_test(&mut self) -> Option<PatchApplyReport> {
        let inverse_patch = self.patch_history.last()?.inverse_patch.clone();
        let file_snapshot = self
            .patch_file_snapshot_history
            .last()
            .map(|(_, snapshot)| snapshot.clone())
            .unwrap_or_default();
        let report = self.execute_patch_inner(inverse_patch, false, false);
        Some(restore_recorded_file_snapshot(report, &file_snapshot))
    }

    pub(crate) fn rollback_last_project_patch(
        &mut self,
        patch_id: &str,
    ) -> Result<PatchApplyReport, PatchDiagnostic> {
        let Some(entry) = self.patch_history.last() else {
            return Err(PatchDiagnostic::error(
                "project_candidate_entry.project_patch_history_empty",
                "ProjectPatch rollback requires the candidate apply to be the latest history entry.",
                None,
                Some(patch_id.to_string()),
            ));
        };
        if entry.patch_id != patch_id {
            return Err(PatchDiagnostic::error(
                "project_candidate_entry.project_patch_not_last",
                "ProjectPatch rollback is only allowed for the exact latest candidate apply.",
                None,
                Some(patch_id.to_string()),
            ));
        }
        let inverse_patch = entry.inverse_patch.clone();
        let Some((snapshot_patch_id, file_snapshot)) =
            self.patch_file_snapshot_history.last().cloned()
        else {
            return Err(PatchDiagnostic::error(
                "project_candidate_entry.project_patch_snapshot_history_empty",
                "ProjectPatch rollback requires the latest file snapshot history entry.",
                None,
                Some(patch_id.to_string()),
            ));
        };
        if snapshot_patch_id != patch_id {
            return Err(PatchDiagnostic::error(
                "project_candidate_entry.project_patch_snapshot_not_last",
                "ProjectPatch rollback file snapshot is not the exact latest history entry.",
                None,
                Some(patch_id.to_string()),
            ));
        }
        let report = restore_recorded_file_snapshot(
            self.execute_patch_inner(inverse_patch, false, false),
            &file_snapshot,
        );
        if report.status == PatchApplyStatus::Committed {
            self.patch_history
                .pop_last_if_patch_id(patch_id)
                .expect("validated latest patch history entry must still exist");
            self.patch_file_snapshot_history.pop();
        }
        Ok(report)
    }

    pub fn patch_history(&self) -> &super::PatchHistory {
        &self.patch_history
    }

    fn build_inverse_patch(
        &self,
        patch: &ProjectPatchDocument,
    ) -> Result<ProjectPatchDocument, PatchDiagnostic> {
        let document = self.editor_scene_document.as_ref();
        let mut virtual_entity_ids = document
            .into_iter()
            .flat_map(|document| document.entities.iter())
            .map(|entity| entity.entity_id.clone())
            .collect::<BTreeSet<_>>();
        let mut created_by_operation = BTreeMap::new();
        for operation in &patch.operations {
            if let PatchOperation::Scene(ScenePatchOperation::CreateEntity {
                operation_id,
                name,
                ..
            }) = operation
            {
                let entity_id = next_patch_entity_id(&virtual_entity_ids, name);
                virtual_entity_ids.insert(entity_id.clone());
                created_by_operation.insert(operation_id.clone(), entity_id);
            }
        }
        let created_entity_ids = created_by_operation
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        let input_inverses = self.build_input_inverses(patch)?;
        let mut inverse_operations = Vec::new();
        for operation in patch.operations.iter().rev() {
            match operation {
                PatchOperation::Scene(scene_operation) => {
                    if let ScenePatchOperation::CreateEntity { operation_id, .. } = scene_operation
                    {
                        let entity_id =
                            created_by_operation.get(operation_id).ok_or_else(|| {
                                PatchDiagnostic::error(
                                    "project_patch.inverse.created_entity_identity_missing",
                                    "CreateEntity inverse lost its deterministic entity identity.",
                                    Some(operation_id.clone()),
                                    None,
                                )
                            })?;
                        inverse_operations.push(PatchOperation::Scene(
                            ScenePatchOperation::DeleteEntity {
                                operation_id: format!("inverse-{operation_id}"),
                                depends_on: Vec::new(),
                                entity_id: entity_id.clone(),
                            },
                        ));
                    } else if scene_operation_entity_id(scene_operation)
                        .is_some_and(|entity_id| created_entity_ids.contains(entity_id))
                    {
                        continue;
                    } else {
                        inverse_operations.push(PatchOperation::Scene(
                            self.inverse_scene_operation(scene_operation)?,
                        ));
                    }
                }
                PatchOperation::Input(input_operation) => {
                    let inverse = input_inverses
                        .get(input_operation.operation_id())
                        .ok_or_else(|| {
                            PatchDiagnostic::error(
                                "project_patch.inverse.input_operation_missing",
                                "Input inverse planning lost an operation.",
                                Some(input_operation.operation_id().to_string()),
                                Some(input_operation.path().to_string()),
                            )
                        })?;
                    inverse_operations.push(PatchOperation::Input(inverse.clone()));
                }
                PatchOperation::Asset(_)
                | PatchOperation::Prefab(_)
                | PatchOperation::Aui(_)
                | PatchOperation::Rule(_) => {}
                PatchOperation::Build(_) => {}
            }
        }
        Ok(ProjectPatchDocument::new(
            format!("{}-inverse", patch.patch_id),
            format!("Revert {}", patch.title),
            PatchSource::Test,
            inverse_operations,
        ))
    }

    fn inverse_scene_operation(
        &self,
        operation: &ScenePatchOperation,
    ) -> Result<ScenePatchOperation, PatchDiagnostic> {
        let document = self.editor_scene_document.as_ref().ok_or_else(|| {
            PatchDiagnostic::error(
                "project_patch.inverse.scene_not_loaded",
                "Cannot build inverse patch without an open Scene document.",
                Some(operation.operation_id().to_string()),
                None,
            )
        })?;
        match operation {
            ScenePatchOperation::CreateEntity {
                operation_id, name, ..
            } => Ok(ScenePatchOperation::DeleteEntity {
                operation_id: format!("inverse-{operation_id}"),
                depends_on: Vec::new(),
                entity_id: document.next_entity_id(name),
            }),
            ScenePatchOperation::DeleteEntity {
                operation_id,
                entity_id,
                ..
            } => Err(PatchDiagnostic::error(
                "project_patch.inverse.delete_not_supported",
                "DeleteEntity inverse requires subtree snapshot and is not enabled in C-min.",
                Some(operation_id.clone()),
                Some(entity_id.clone()),
            )),
            ScenePatchOperation::RenameEntity {
                operation_id,
                entity_id,
                ..
            } => {
                let entity = document.entity(entity_id).ok_or_else(|| {
                    PatchDiagnostic::error(
                        "project_patch.inverse.entity_missing",
                        format!(
                            "Cannot build RenameEntity inverse for missing entity: {entity_id}"
                        ),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    )
                })?;
                Ok(ScenePatchOperation::RenameEntity {
                    operation_id: format!("inverse-{operation_id}"),
                    depends_on: Vec::new(),
                    entity_id: entity_id.clone(),
                    name: entity.name.clone(),
                })
            }
            ScenePatchOperation::SetTransform {
                operation_id,
                entity_id,
                ..
            } => {
                let entity = document.entity(entity_id).ok_or_else(|| {
                    PatchDiagnostic::error(
                        "project_patch.inverse.entity_missing",
                        format!(
                            "Cannot build SetTransform inverse for missing entity: {entity_id}"
                        ),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    )
                })?;
                let transform = entity.transform.unwrap_or_else(EditorTransform::identity);
                Ok(ScenePatchOperation::SetTransform {
                    operation_id: format!("inverse-{operation_id}"),
                    depends_on: Vec::new(),
                    entity_id: entity_id.clone(),
                    local_position: Some(crate::services::scene_service::editor_vec3_to_ui(
                        transform.local_position,
                    )),
                    local_rotation: Some(crate::services::scene_service::editor_vec3_to_ui(
                        transform.local_rotation,
                    )),
                    local_scale: Some(crate::services::scene_service::editor_vec3_to_ui(
                        transform.local_scale,
                    )),
                })
            }
            ScenePatchOperation::SetComponentField {
                operation_id,
                entity_id,
                component_type,
                field_path,
                ..
            } => {
                let entity = document.entity(entity_id).ok_or_else(|| {
                    PatchDiagnostic::error(
                        "project_patch.inverse.entity_missing",
                        format!("Cannot build SetComponentField inverse for missing entity: {entity_id}"),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    )
                })?;
                let Some(component) = entity
                    .components
                    .iter()
                    .find(|component| component.component_type == *component_type)
                else {
                    return Err(PatchDiagnostic::error(
                        "project_patch.inverse.component_missing",
                        format!("Cannot build inverse for missing component: {component_type}"),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                };
                let pointer = format!("/{}", field_path.replace('.', "/"));
                let Some(value) = component.fields.pointer(&pointer) else {
                    return Err(PatchDiagnostic::error(
                        "project_patch.inverse.field_missing",
                        format!("Cannot build inverse for missing field path: {field_path}"),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    ));
                };
                Ok(ScenePatchOperation::SetComponentField {
                    operation_id: format!("inverse-{operation_id}"),
                    depends_on: Vec::new(),
                    entity_id: entity_id.clone(),
                    component_type: component_type.clone(),
                    field_path: field_path.clone(),
                    value: value.clone(),
                })
            }
            ScenePatchOperation::AddComponent {
                operation_id,
                entity_id,
                component_type,
                ..
            } => Ok(ScenePatchOperation::RemoveComponent {
                operation_id: format!("inverse-{operation_id}"),
                depends_on: Vec::new(),
                entity_id: entity_id.clone(),
                component_type: component_type.clone(),
            }),
            ScenePatchOperation::RemoveComponent {
                operation_id,
                entity_id,
                component_type,
                ..
            } => {
                let entity = document.entity(entity_id).ok_or_else(|| {
                    PatchDiagnostic::error(
                        "project_patch.inverse.entity_missing",
                        format!(
                            "Cannot build RemoveComponent inverse for missing entity: {entity_id}"
                        ),
                        Some(operation_id.clone()),
                        Some(entity_id.clone()),
                    )
                })?;
                let component = entity
                    .components
                    .iter()
                    .find(|component| component.component_type == *component_type)
                    .ok_or_else(|| {
                        PatchDiagnostic::error(
                            "project_patch.inverse.component_missing",
                            format!("Cannot build inverse for missing component: {component_type}"),
                            Some(operation_id.clone()),
                            Some(entity_id.clone()),
                        )
                    })?;
                Ok(ScenePatchOperation::AddComponent {
                    operation_id: format!("inverse-{operation_id}"),
                    depends_on: Vec::new(),
                    entity_id: entity_id.clone(),
                    component_type: component_type.clone(),
                    fields: component.fields.clone(),
                })
            }
            ScenePatchOperation::PlaceAssetIntoScene {
                operation_id,
                asset_id,
                ..
            } => Ok(ScenePatchOperation::DeleteEntity {
                operation_id: format!("inverse-{operation_id}"),
                depends_on: Vec::new(),
                entity_id: document.next_entity_id(asset_id),
            }),
        }
    }

    fn build_input_inverses(
        &self,
        patch: &ProjectPatchDocument,
    ) -> Result<BTreeMap<String, InputPatchOperation>, PatchDiagnostic> {
        let project_session = self.active_project_session.as_ref().ok_or_else(|| {
            PatchDiagnostic::error(
                "project_patch.inverse.input_no_project",
                "Cannot build input inverse without an active project.",
                None,
                Some("project_session".to_string()),
            )
        })?;
        let mut virtual_mappings =
            BTreeMap::<String, Option<engine_input::InputMappingAsset>>::new();
        let mut inverses = BTreeMap::new();
        for operation in &patch.operations {
            let PatchOperation::Input(operation) = operation else {
                continue;
            };
            let operation_id = operation.operation_id().to_string();
            let path = operation.path().to_string();
            if !virtual_mappings.contains_key(&path) {
                let relative_path = ProjectRelativePath::parse(&path).map_err(|error| {
                    PatchDiagnostic::error(
                        "project_patch.inverse.input_path_invalid",
                        format!("Input inverse path is invalid: {error}"),
                        Some(operation_id.clone()),
                        Some(path.clone()),
                    )
                })?;
                let exists = project_session
                    .write_scope()
                    .try_exists(relative_path.as_path())
                    .map_err(|error| {
                        PatchDiagnostic::error(
                            "project_patch.inverse.input_inspect_failed",
                            format!("Failed to inspect InputMapping: {error}"),
                            Some(operation_id.clone()),
                            Some(path.clone()),
                        )
                    })?;
                let mapping = if exists {
                    Some(
                        InputMappingAuthoringService::load(&project_session.project_root, &path)
                            .map_err(|message| {
                                PatchDiagnostic::error(
                                    "project_patch.inverse.input_load_failed",
                                    message,
                                    Some(operation_id.clone()),
                                    Some(path.clone()),
                                )
                            })?,
                    )
                } else {
                    None
                };
                virtual_mappings.insert(path.clone(), mapping);
            }
            let virtual_mapping = virtual_mappings
                .get_mut(&path)
                .expect("input path was initialized above");
            let inverse = match operation {
                InputPatchOperation::CreateDefaultInputMapping { .. } => {
                    if virtual_mapping.is_some() {
                        return Err(PatchDiagnostic::error(
                            "project_patch.inverse.input_mapping_already_exists",
                            "CreateDefaultInputMapping cannot overwrite an existing mapping.",
                            Some(operation_id.clone()),
                            Some(path.clone()),
                        ));
                    }
                    *virtual_mapping = Some(InputMappingAuthoringService::create_default());
                    InputPatchOperation::DeleteInputMapping {
                        operation_id: format!("inverse-{operation_id}"),
                        depends_on: Vec::new(),
                        path: path.clone(),
                    }
                }
                InputPatchOperation::DeleteInputMapping { .. } => {
                    return Err(PatchDiagnostic::error(
                        "project_patch.inverse.input_delete_mapping_not_supported",
                        "DeleteInputMapping is reserved for inverse execution because restoring exact file bytes requires a snapshot.",
                        Some(operation_id.clone()),
                        Some(path.clone()),
                    ));
                }
                InputPatchOperation::AddInputAction {
                    action_id,
                    value_type,
                    ..
                } => {
                    let mapping =
                        require_virtual_input_mapping(virtual_mapping, &operation_id, &path)?;
                    let inverse = InputPatchOperation::RemoveInputAction {
                        operation_id: format!("inverse-{operation_id}"),
                        depends_on: Vec::new(),
                        path: path.clone(),
                        action_id: action_id.clone(),
                    };
                    apply_virtual_input_edit(
                        mapping,
                        InputMappingEditCommand::AddAction {
                            action_id: action_id.clone(),
                            value_type: *value_type,
                        },
                        &operation_id,
                        &path,
                    )?;
                    inverse
                }
                InputPatchOperation::AddInputBinding {
                    context_id,
                    action_id,
                    device_path,
                    ..
                } => {
                    let mapping =
                        require_virtual_input_mapping(virtual_mapping, &operation_id, &path)?;
                    let inverse = InputPatchOperation::RemoveInputBinding {
                        operation_id: format!("inverse-{operation_id}"),
                        depends_on: Vec::new(),
                        path: path.clone(),
                        binding_index: mapping.bindings.len(),
                    };
                    apply_virtual_input_edit(
                        mapping,
                        InputMappingEditCommand::AddBinding {
                            context_id: context_id.clone(),
                            action_id: action_id.clone(),
                            device_path: device_path.clone(),
                        },
                        &operation_id,
                        &path,
                    )?;
                    inverse
                }
                InputPatchOperation::RemoveInputAction { .. } => {
                    return Err(PatchDiagnostic::error(
                        "project_patch.inverse.input_remove_action_not_supported",
                        "RemoveInputAction inverse requires action and binding snapshots.",
                        Some(operation_id.clone()),
                        Some(path.clone()),
                    ));
                }
                InputPatchOperation::RemoveInputBinding { .. } => {
                    return Err(PatchDiagnostic::error(
                        "project_patch.inverse.input_remove_binding_not_supported",
                        "RemoveInputBinding inverse requires a binding snapshot.",
                        Some(operation_id.clone()),
                        Some(path.clone()),
                    ));
                }
                InputPatchOperation::SetInputBindingDevicePath {
                    binding_index,
                    device_path,
                    ..
                } => {
                    let mapping =
                        require_virtual_input_mapping(virtual_mapping, &operation_id, &path)?;
                    let old_path = mapping
                        .bindings
                        .get(*binding_index)
                        .ok_or_else(|| {
                            PatchDiagnostic::error(
                                "project_patch.inverse.input_binding_missing",
                                format!("Cannot build inverse for missing binding index: {binding_index}"),
                                Some(operation_id.clone()),
                                Some(path.clone()),
                            )
                        })?
                        .device_path
                        .clone();
                    apply_virtual_input_edit(
                        mapping,
                        InputMappingEditCommand::SetBindingDevicePath {
                            binding_index: *binding_index,
                            device_path: device_path.clone(),
                        },
                        &operation_id,
                        &path,
                    )?;
                    InputPatchOperation::SetInputBindingDevicePath {
                        operation_id: format!("inverse-{operation_id}"),
                        depends_on: Vec::new(),
                        path: path.clone(),
                        binding_index: *binding_index,
                        device_path: old_path,
                    }
                }
                InputPatchOperation::SetInputBindingProcessor {
                    binding_index,
                    processor,
                    ..
                } => {
                    let mapping =
                        require_virtual_input_mapping(virtual_mapping, &operation_id, &path)?;
                    let old_processor = mapping
                        .bindings
                        .get(*binding_index)
                        .ok_or_else(|| {
                            PatchDiagnostic::error(
                                "project_patch.inverse.input_binding_missing",
                                format!("Cannot build inverse for missing binding index: {binding_index}"),
                                Some(operation_id.clone()),
                                Some(path.clone()),
                            )
                        })?
                        .processor
                        .clone();
                    apply_virtual_input_edit(
                        mapping,
                        InputMappingEditCommand::SetBindingProcessorByIndex {
                            binding_index: *binding_index,
                            processor: patch_processor_to_ui(processor),
                        },
                        &operation_id,
                        &path,
                    )?;
                    InputPatchOperation::SetInputBindingProcessor {
                        operation_id: format!("inverse-{operation_id}"),
                        depends_on: Vec::new(),
                        path: path.clone(),
                        binding_index: *binding_index,
                        processor: runtime_processor_to_patch(&old_processor),
                    }
                }
            };
            inverses.insert(operation_id, inverse);
        }
        Ok(inverses)
    }

    fn patch_rollback_snapshot(&self) -> EditorSessionPatchRollbackSnapshot {
        EditorSessionPatchRollbackSnapshot {
            selected_project_browser_path: self.selected_project_browser_path.clone(),
            input_mapping_editor_state: self.input_mapping_editor_state.clone(),
            editor_scene_document: self.editor_scene_document.clone(),
            scene_selection: self.scene_selection.clone(),
            scene_undo_stack: self.scene_undo_stack.clone(),
            scene_path: self.scene_path.clone(),
            last_scene_edit_report: self.last_scene_edit_report.clone(),
            last_preview_world_sync_report: self.last_preview_world_sync_report.clone(),
            world: self.world.clone(),
            selected_entity_id: self.selected_entity_id.clone(),
            console_entries: self.console_entries.clone(),
            diagnostics: self.diagnostics.clone(),
            revision: self.revision,
        }
    }

    fn restore_patch_rollback_snapshot(&mut self, snapshot: EditorSessionPatchRollbackSnapshot) {
        self.selected_project_browser_path = snapshot.selected_project_browser_path;
        self.input_mapping_editor_state = snapshot.input_mapping_editor_state;
        self.editor_scene_document = snapshot.editor_scene_document;
        self.scene_selection = snapshot.scene_selection;
        self.scene_undo_stack = snapshot.scene_undo_stack;
        self.scene_path = snapshot.scene_path;
        self.last_scene_edit_report = snapshot.last_scene_edit_report;
        self.last_preview_world_sync_report = snapshot.last_preview_world_sync_report;
        self.world = snapshot.world;
        self.selected_entity_id = snapshot.selected_entity_id;
        self.console_entries = snapshot.console_entries;
        self.diagnostics = snapshot.diagnostics;
        self.revision = snapshot.revision;
    }

    fn commit_input_patch_drafts(
        &mut self,
        patch_id: &str,
        paths: &BTreeSet<String>,
    ) -> Result<(), PatchDiagnostic> {
        for path in paths {
            let result = self.execute_command(UiCommand {
                command_id: "save_input_mapping".to_string(),
                source: editor_ui_model::UiCommandSource::AiAssistant,
                request_id: format!("request-project-patch-{patch_id}-save-input"),
                payload: editor_ui_model::UiCommandPayload::SaveInputMapping { path: path.clone() },
            });
            if result.status != crate::CommandStatus::Committed {
                let message = result
                    .diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| format!("Failed to save InputMapping patch draft: {path}"));
                return Err(PatchDiagnostic::error(
                    "project_patch.input_commit_failed",
                    message,
                    None,
                    Some(path.clone()),
                ));
            }
        }
        Ok(())
    }

    fn commit_scene_patch_draft(&mut self, patch_id: &str) -> Result<(), PatchDiagnostic> {
        let result = self.execute_command(UiCommand {
            command_id: "save_scene_document".to_string(),
            source: editor_ui_model::UiCommandSource::AiAssistant,
            request_id: format!("request-project-patch-{patch_id}-save-scene"),
            payload: editor_ui_model::UiCommandPayload::SaveSceneDocument { path: None },
        });
        if result.status == crate::CommandStatus::Committed {
            return Ok(());
        }
        let message = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| "Failed to save Scene patch draft.".to_string());
        Err(PatchDiagnostic::error(
            "project_patch.scene_commit_failed",
            message,
            None,
            self.scene_path
                .as_ref()
                .map(|path| path.display().to_string()),
        ))
    }
}

fn restore_recorded_file_snapshot(
    mut report: PatchApplyReport,
    file_snapshot: &ProjectFileSnapshotSet,
) -> PatchApplyReport {
    if report.status == PatchApplyStatus::Committed {
        if let Err(diagnostic) = file_snapshot.restore() {
            report.status = PatchApplyStatus::Failed;
            report.validation.diagnostics.push(diagnostic);
        }
    }
    report
}

fn require_virtual_input_mapping<'a>(
    mapping: &'a mut Option<engine_input::InputMappingAsset>,
    operation_id: &str,
    path: &str,
) -> Result<&'a mut engine_input::InputMappingAsset, PatchDiagnostic> {
    mapping.as_mut().ok_or_else(|| {
        PatchDiagnostic::error(
            "project_patch.inverse.input_mapping_missing",
            "Input operation requires an existing mapping or an earlier CreateDefaultInputMapping.",
            Some(operation_id.to_string()),
            Some(path.to_string()),
        )
    })
}

fn apply_virtual_input_edit(
    mapping: &mut engine_input::InputMappingAsset,
    command: InputMappingEditCommand,
    operation_id: &str,
    path: &str,
) -> Result<(), PatchDiagnostic> {
    InputMappingAuthoringService::apply(mapping, command).map_err(|message| {
        PatchDiagnostic::error(
            "project_patch.inverse.input_virtual_apply_failed",
            message,
            Some(operation_id.to_string()),
            Some(path.to_string()),
        )
    })
}

fn patch_processor_to_ui(
    processor: &InputBindingProcessorPatch,
) -> editor_ui_model::InputProcessorKind {
    match processor {
        InputBindingProcessorPatch::None => editor_ui_model::InputProcessorKind::None,
        InputBindingProcessorPatch::Deadzone { threshold } => {
            editor_ui_model::InputProcessorKind::Deadzone {
                threshold: *threshold,
            }
        }
        InputBindingProcessorPatch::Normalize => editor_ui_model::InputProcessorKind::Normalize,
        InputBindingProcessorPatch::Scale { factor } => {
            editor_ui_model::InputProcessorKind::Scale { factor: *factor }
        }
        InputBindingProcessorPatch::Invert => editor_ui_model::InputProcessorKind::Invert,
    }
}

fn runtime_processor_to_patch(
    processor: &engine_input::InputProcessorPreset,
) -> InputBindingProcessorPatch {
    match processor {
        engine_input::InputProcessorPreset::None => InputBindingProcessorPatch::None,
        engine_input::InputProcessorPreset::Deadzone { threshold } => {
            InputBindingProcessorPatch::Deadzone {
                threshold: *threshold,
            }
        }
        engine_input::InputProcessorPreset::Normalize => InputBindingProcessorPatch::Normalize,
        engine_input::InputProcessorPreset::Scale { factor } => {
            InputBindingProcessorPatch::Scale { factor: *factor }
        }
        engine_input::InputProcessorPreset::Invert => InputBindingProcessorPatch::Invert,
    }
}

fn scene_operation_entity_id(operation: &ScenePatchOperation) -> Option<&String> {
    match operation {
        ScenePatchOperation::DeleteEntity { entity_id, .. }
        | ScenePatchOperation::RenameEntity { entity_id, .. }
        | ScenePatchOperation::SetTransform { entity_id, .. }
        | ScenePatchOperation::AddComponent { entity_id, .. }
        | ScenePatchOperation::RemoveComponent { entity_id, .. }
        | ScenePatchOperation::SetComponentField { entity_id, .. } => Some(entity_id),
        ScenePatchOperation::CreateEntity { .. }
        | ScenePatchOperation::PlaceAssetIntoScene { .. } => None,
    }
}

fn next_patch_entity_id(existing: &BTreeSet<String>, name: &str) -> String {
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if slug.is_empty() {
        "entity".to_string()
    } else {
        format!("entity-{slug}")
    };
    if !existing.contains(&base) {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded entity id generation should not terminate")
}

fn project_file_snapshot_paths(patch: &ProjectPatchDocument) -> Vec<String> {
    let mut paths = Vec::new();
    for operation in &patch.operations {
        match operation {
            PatchOperation::Input(input_operation) => {
                paths.push(input_operation.path().to_string());
            }
            PatchOperation::Asset(AssetPatchOperation::GenerateMockImageAsset {
                target_folder,
                asset_name,
                ..
            }) => {
                let safe_name = sanitize_asset_name_for_snapshot(asset_name);
                if !safe_name.is_empty() {
                    paths.push(join_project_relative(
                        target_folder,
                        &format!("{safe_name}.png"),
                    ));
                    paths.push(join_project_relative(
                        target_folder,
                        &format!("{safe_name}.ai.json"),
                    ));
                }
            }
            PatchOperation::Asset(
                AssetPatchOperation::RegisterExistingAsset { .. }
                | AssetPatchOperation::ValidateAssetBrowserIndex { .. },
            ) => {}
            PatchOperation::Prefab(prefab_operation) => match prefab_operation {
                PrefabPatchOperation::CreateFromSceneSelection { prefab_id, .. } => {
                    paths.push(PrefabWorkflowService::prefab_path_for_id(prefab_id));
                }
                PrefabPatchOperation::SaveDocument { path, .. }
                | PrefabPatchOperation::OpenDocument { path, .. } => {
                    paths.push(path.clone());
                }
                PrefabPatchOperation::ValidateReferences { path, .. } => {
                    if let Some(path) = path {
                        paths.push(path.clone());
                    }
                }
                PrefabPatchOperation::SetStageEntityField { .. }
                | PrefabPatchOperation::InstantiateInScene { .. }
                | PrefabPatchOperation::ApplyOverrideToAsset { .. }
                | PrefabPatchOperation::RevertOverride { .. } => {}
            },
            PatchOperation::Aui(aui_operation) => match aui_operation {
                AuiPatchOperation::CreateDocument { path, .. }
                | AuiPatchOperation::AddNode { path, .. }
                | AuiPatchOperation::SetNodeField { path, .. }
                | AuiPatchOperation::SetBindingPath { path, .. }
                | AuiPatchOperation::SetActionRef { path, .. }
                | AuiPatchOperation::SaveDocument { path, .. } => paths.push(path.clone()),
                AuiPatchOperation::OpenDocument { .. }
                | AuiPatchOperation::ValidateDocument { .. }
                | AuiPatchOperation::PreviewOverlay { .. } => {}
            },
            PatchOperation::Rule(rule_operation) => match rule_operation {
                RulePatchOperation::CreateAsset { path, .. }
                | RulePatchOperation::SetTrigger { path, .. }
                | RulePatchOperation::AddStatement { path, .. }
                | RulePatchOperation::UpdateStatement { path, .. }
                | RulePatchOperation::RemoveStatement { path, .. }
                | RulePatchOperation::AddOperation { path, .. }
                | RulePatchOperation::UpdateOperation { path, .. }
                | RulePatchOperation::RemoveOperation { path, .. }
                | RulePatchOperation::BuildProjectManifest { path, .. } => paths.push(path.clone()),
                RulePatchOperation::OpenAsset { .. }
                | RulePatchOperation::ValidateAsset { .. }
                | RulePatchOperation::BuildArtifact { .. } => {}
            },
            PatchOperation::Scene(_) | PatchOperation::Build(_) => {}
        }
    }
    paths
}

fn project_relative_scene_snapshot_path(
    scene_path: &Path,
    project_root: &Path,
) -> Result<String, PatchDiagnostic> {
    let relative_path = if scene_path.is_absolute() {
        scene_path.strip_prefix(project_root).map_err(|_| {
            PatchDiagnostic::error(
                "project_patch.file_snapshot.scene_path_outside_project",
                "Active Scene path is outside the active project root.",
                None,
                Some(scene_path.display().to_string()),
            )
        })?
    } else {
        scene_path
    };
    ProjectRelativePath::parse(relative_path)
        .map(|path| path.to_string())
        .map_err(|error| {
            PatchDiagnostic::error(
                "project_patch.file_snapshot.scene_path_invalid",
                format!("Active Scene path is not a valid project-relative path: {error}"),
                None,
                Some(scene_path.display().to_string()),
            )
        })
}

fn patch_mutates_scene(patch: &ProjectPatchDocument) -> bool {
    patch.operations.iter().any(operation_mutates_scene)
}

fn operation_mutates_scene(operation: &PatchOperation) -> bool {
    match operation {
        PatchOperation::Scene(_) => true,
        PatchOperation::Prefab(PrefabPatchOperation::CreateFromSceneSelection {
            replace_selection_with_instance,
            ..
        }) => *replace_selection_with_instance,
        PatchOperation::Prefab(
            PrefabPatchOperation::InstantiateInScene { .. }
            | PrefabPatchOperation::ApplyOverrideToAsset { .. }
            | PrefabPatchOperation::RevertOverride { .. },
        ) => true,
        _ => false,
    }
}

fn join_project_relative(folder: &str, file_name: &str) -> String {
    let folder = folder.replace('\\', "/").trim_end_matches('/').to_string();
    if folder.is_empty() {
        file_name.to_string()
    } else {
        format!("{folder}/{file_name}")
    }
}

fn sanitize_asset_name_for_snapshot(value: &str) -> String {
    let mut safe = String::new();
    let mut last_was_dash = false;
    for ch in value.trim().chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch == '_' || ch == '-' || ch.is_whitespace() {
            Some('-')
        } else {
            None
        };
        if let Some(ch) = normalized {
            if ch == '-' {
                if !last_was_dash && !safe.is_empty() {
                    safe.push(ch);
                    last_was_dash = true;
                }
            } else {
                safe.push(ch);
                last_was_dash = false;
            }
        }
    }
    safe.trim_matches('-').to_string()
}

#[cfg(test)]
mod containment_tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn project_write_containment_patch_rollback_rejects_swapped_parent_link() {
        let root = test_root("rollback");
        let outside = test_root("rollback-outside");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(root.join("Assets/value.json"), b"before").unwrap();
        fs::write(outside.join("value.json"), b"outside-sentinel").unwrap();
        let scope = ProjectWriteScope::open(&root).unwrap();
        let snapshots =
            ProjectFileSnapshotSet::capture(&scope, vec!["Assets/value.json".to_string()]).unwrap();

        fs::rename(root.join("Assets"), root.join("Assets-parked")).unwrap();
        create_directory_link(&outside, &root.join("Assets")).unwrap();

        let error = snapshots.restore().unwrap_err();
        assert_eq!(error.code, "project_write.rollback_containment_changed");
        assert_eq!(
            fs::read(outside.join("value.json")).unwrap(),
            b"outside-sentinel"
        );
    }

    #[test]
    fn scene_snapshot_path_converts_project_absolute_path_to_relative() {
        let project_root = test_root("scene-snapshot-path");
        let scene_path = project_root.join("Scenes/Main.scene.json");

        assert_eq!(
            project_relative_scene_snapshot_path(&scene_path, &project_root).unwrap(),
            "Scenes/Main.scene.json"
        );
    }

    #[test]
    fn scene_snapshot_path_rejects_absolute_path_outside_project() {
        let project_root = test_root("scene-snapshot-project");
        let outside_root = test_root("scene-snapshot-outside");
        let error = project_relative_scene_snapshot_path(
            &outside_root.join("Main.scene.json"),
            &project_root,
        )
        .unwrap_err();

        assert_eq!(
            error.code,
            "project_patch.file_snapshot.scene_path_outside_project"
        );
    }

    fn test_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "aife-project-patch-containment-{label}-{}-{stamp}-{sequence}",
            std::process::id()
        ))
    }

    #[cfg(unix)]
    fn create_directory_link(target: &PathBuf, link: &PathBuf) -> Result<(), String> {
        std::os::unix::fs::symlink(target, link).map_err(|error| error.to_string())
    }

    #[cfg(windows)]
    fn create_directory_link(target: &PathBuf, link: &PathBuf) -> Result<(), String> {
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "mklink /J failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}
