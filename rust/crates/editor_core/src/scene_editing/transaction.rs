use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use super::{
    is_supported_component_field_path, set_json_object_path, EditorSceneDocument,
    EditorSceneEntity, SceneEditCommand, SceneEditDiagnostic, SceneSelection, SceneUndoRecord,
    SceneUndoStack, SCENE_EDIT_TRANSACTION_REPORT_SCHEMA_VERSION,
};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneEditTransaction {
    pub transaction_id: String,
    pub request_id: String,
    pub command: SceneEditCommand,
    pub status: SceneEditTransactionStatus,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub before_summary: Option<String>,
    pub after_summary: Option<String>,
    pub diagnostics: Vec<SceneEditDiagnostic>,
    pub undo_record: Option<SceneUndoRecord>,
}

impl SceneEditTransaction {
    pub fn apply(
        transaction_id: impl Into<String>,
        document: &mut EditorSceneDocument,
        selection: &mut SceneSelection,
        undo_stack: &mut SceneUndoStack,
        request: SceneEditRequest,
    ) -> SceneEditTransactionReport {
        let transaction_id = transaction_id.into();
        let before = document.clone();
        let before_count = document.entities.len();
        let mut transaction = SceneEditTransaction {
            transaction_id: transaction_id.clone(),
            request_id: request.request_id.clone(),
            command: request.command.clone(),
            status: SceneEditTransactionStatus::Pending,
            read_set: Vec::new(),
            write_set: Vec::new(),
            before_summary: Some(format!("entity_count={before_count}")),
            after_summary: None,
            diagnostics: Vec::new(),
            undo_record: None,
        };

        let mut dirty = false;
        match &request.command {
            SceneEditCommand::SelectEntity { entity_id } => {
                transaction.read_set.push("scene.entities".to_string());
                transaction
                    .write_set
                    .push("scene.selection.primary_entity_id".to_string());
                if selection.select_single(document, entity_id) {
                    transaction.status = SceneEditTransactionStatus::Committed;
                } else {
                    transaction.status = SceneEditTransactionStatus::Rejected;
                    transaction.diagnostics.push(
                        SceneEditDiagnostic::warning(
                            "scene.selection.entity_missing",
                            "scene.selection",
                            format!("Cannot select missing entity: {entity_id}"),
                        )
                        .with_entity_id(entity_id.clone()),
                    );
                }
            }
            SceneEditCommand::CreateEntity {
                parent_id,
                name,
                mesh,
                components,
                local_transform,
                sibling_order,
            } => {
                transaction.read_set.push("scene.entities".to_string());
                transaction.write_set.push("scene.entities".to_string());
                if let Some(parent_id) = parent_id {
                    if !document.has_entity(parent_id) {
                        reject(
                            &mut transaction,
                            "scene.entity.parent_missing",
                            format!("Cannot create entity under missing parent: {parent_id}"),
                        );
                    }
                }
                if transaction.status != SceneEditTransactionStatus::Rejected {
                    let entity_id = document.next_entity_id(name);
                    let mut entity = EditorSceneEntity::new(entity_id.clone(), name.clone());
                    entity.parent_id = parent_id.clone();
                    entity.sibling_order = sibling_order.unwrap_or(document.entities.len() as i32);
                    entity.transform = Some(*local_transform);
                    entity.mesh = mesh.clone();
                    entity.components = components.clone();
                    document.entities.push(entity);
                    selection.select_single(document, &entity_id);
                    transaction
                        .write_set
                        .push(format!("scene.entities.{entity_id}"));
                    transaction
                        .write_set
                        .push("scene.selection.primary_entity_id".to_string());
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                }
            }
            SceneEditCommand::DeleteEntity {
                entity_id,
                delete_children,
            } => {
                transaction
                    .read_set
                    .push(format!("scene.entities.{entity_id}"));
                transaction.write_set.push("scene.entities".to_string());
                if !*delete_children {
                    reject(
                        &mut transaction,
                        "scene.entity.delete_children_required",
                        "Scene Editing v1 only supports subtree delete.",
                    );
                } else if !document.has_entity(entity_id) {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot delete missing entity: {entity_id}"),
                    );
                } else {
                    let removed = document.remove_subtree(entity_id);
                    for id in removed {
                        transaction.write_set.push(format!("scene.entities.{id}"));
                    }
                    if selection
                        .primary_entity_id
                        .as_deref()
                        .is_some_and(|selected| !document.has_entity(selected))
                    {
                        selection.clear();
                    }
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                }
            }
            SceneEditCommand::RenameEntity { entity_id, name } => {
                transaction
                    .read_set
                    .push(format!("scene.entities.{entity_id}.name"));
                transaction
                    .write_set
                    .push(format!("scene.entities.{entity_id}.name"));
                let next_name = name.trim();
                if next_name.is_empty() {
                    reject(
                        &mut transaction,
                        "scene.entity.name_required",
                        "Entity name cannot be empty.",
                    );
                } else if let Some(entity) = document.entity_mut(entity_id) {
                    entity.name = next_name.to_string();
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                } else {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot rename missing entity: {entity_id}"),
                    );
                }
            }
            SceneEditCommand::DuplicateEntity { entity_id } => {
                transaction
                    .read_set
                    .push(format!("scene.entities.{entity_id}"));
                transaction.write_set.push("scene.entities".to_string());
                if let Some(source) = document.entity(entity_id).cloned() {
                    let mut duplicate = source;
                    duplicate.entity_id = document.next_entity_id(&duplicate.name);
                    duplicate.name = format!("{} Copy", duplicate.name);
                    transaction
                        .write_set
                        .push(format!("scene.entities.{}", duplicate.entity_id));
                    document.entities.push(duplicate);
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                } else {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot duplicate missing entity: {entity_id}"),
                    );
                }
            }
            SceneEditCommand::ReparentEntity {
                entity_id,
                new_parent_id,
                sibling_order,
                keep_world_transform: _,
            } => {
                transaction
                    .read_set
                    .push(format!("scene.entities.{entity_id}"));
                transaction
                    .write_set
                    .push(format!("scene.entities.{entity_id}.parentId"));
                if !document.has_entity(entity_id) {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot reparent missing entity: {entity_id}"),
                    );
                } else if let Some(parent_id) = new_parent_id {
                    if !document.has_entity(parent_id) {
                        reject(
                            &mut transaction,
                            "scene.entity.parent_missing",
                            format!("Cannot reparent to missing parent: {parent_id}"),
                        );
                    } else if would_create_cycle(document, entity_id, parent_id) {
                        reject(
                            &mut transaction,
                            "scene.entity.reparent_cycle",
                            "Cannot create cyclic parent-child relationship.",
                        );
                    }
                }
                if transaction.status != SceneEditTransactionStatus::Rejected {
                    let entity = document
                        .entity_mut(entity_id)
                        .expect("entity existence was validated");
                    entity.parent_id = new_parent_id.clone();
                    if let Some(order) = sibling_order {
                        entity.sibling_order = *order;
                    }
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                }
            }
            SceneEditCommand::SetTransform {
                entity_id,
                local_position,
                local_rotation,
                local_scale,
            } => {
                transaction
                    .read_set
                    .push(format!("scene.entities.{entity_id}.transform"));
                transaction
                    .write_set
                    .push(format!("scene.entities.{entity_id}.transform"));
                let Some(entity) = document.entity_mut(entity_id) else {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot transform missing entity: {entity_id}"),
                    );
                    return finish_transaction(
                        document,
                        before,
                        selection,
                        undo_stack,
                        transaction,
                        dirty,
                    );
                };
                let Some(transform) = entity.transform.as_mut() else {
                    reject(
                        &mut transaction,
                        "scene.entity.transform_required",
                        format!("Entity {entity_id} does not have a Transform."),
                    );
                    return finish_transaction(
                        document,
                        before,
                        selection,
                        undo_stack,
                        transaction,
                        dirty,
                    );
                };
                if let Some(value) = local_position {
                    transform.local_position = *value;
                    transaction.write_set.push(format!(
                        "scene.entities.{entity_id}.transform.localPosition"
                    ));
                }
                if let Some(value) = local_rotation {
                    transform.local_rotation = *value;
                    transaction.write_set.push(format!(
                        "scene.entities.{entity_id}.transform.localRotation"
                    ));
                }
                if let Some(value) = local_scale {
                    transform.local_scale = *value;
                    transaction
                        .write_set
                        .push(format!("scene.entities.{entity_id}.transform.localScale"));
                }
                transaction.status = SceneEditTransactionStatus::Committed;
                dirty = true;
            }
            SceneEditCommand::SetComponentField {
                entity_id,
                component_type,
                field_path,
                value,
            } => {
                transaction.read_set.push(format!(
                    "scene.entities.{entity_id}.components.{component_type}"
                ));
                transaction.write_set.push(format!(
                    "scene.entities.{entity_id}.components.{component_type}.{field_path}"
                ));
                let Some(entity) = document.entity_mut(entity_id) else {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot edit component on missing entity: {entity_id}"),
                    );
                    return finish_transaction(
                        document,
                        before,
                        selection,
                        undo_stack,
                        transaction,
                        dirty,
                    );
                };
                let Some(component) = entity
                    .components
                    .iter_mut()
                    .find(|component| component.component_type == *component_type)
                else {
                    reject(
                        &mut transaction,
                        "scene.component.missing",
                        format!("Entity {entity_id} does not have component {component_type}."),
                    );
                    return finish_transaction(
                        document,
                        before,
                        selection,
                        undo_stack,
                        transaction,
                        dirty,
                    );
                };
                if !is_supported_component_field_path(field_path) {
                    reject(
                        &mut transaction,
                        "scene.component.field_path_unsupported",
                        "Scene Editing supports dot-separated object field paths without array indexes.",
                    );
                    return finish_transaction(
                        document,
                        before,
                        selection,
                        undo_stack,
                        transaction,
                        dirty,
                    );
                }
                if !component.fields.is_object() {
                    component.fields = Value::Object(Map::new());
                }
                let fields = component
                    .fields
                    .as_object_mut()
                    .expect("component fields were converted to object");
                set_json_object_path(fields, field_path, value.clone());
                transaction.status = SceneEditTransactionStatus::Committed;
                dirty = true;
            }
            SceneEditCommand::SaveScene { .. } => {
                transaction.status = SceneEditTransactionStatus::Committed;
            }
            SceneEditCommand::Undo => {
                if let Some(record) = undo_stack.undo(document) {
                    transaction.undo_record = Some(record);
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                } else {
                    reject(
                        &mut transaction,
                        "scene.undo.empty",
                        "There is no scene edit to undo.",
                    );
                }
            }
            SceneEditCommand::Redo => {
                if let Some(record) = undo_stack.redo(document) {
                    transaction.undo_record = Some(record);
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                } else {
                    reject(
                        &mut transaction,
                        "scene.redo.empty",
                        "There is no scene edit to redo.",
                    );
                }
            }
        }

        finish_transaction(document, before, selection, undo_stack, transaction, dirty)
    }
}

fn finish_transaction(
    document: &mut EditorSceneDocument,
    before: EditorSceneDocument,
    _selection: &mut SceneSelection,
    undo_stack: &mut SceneUndoStack,
    mut transaction: SceneEditTransaction,
    dirty: bool,
) -> SceneEditTransactionReport {
    if dirty && transaction.status == SceneEditTransactionStatus::Committed {
        let after = document.clone();
        document.mark_dirty(transaction.transaction_id.clone());
        let record = SceneUndoRecord {
            transaction_id: transaction.transaction_id.clone(),
            command_kind: transaction.command.kind().to_string(),
            before_document_snapshot: before,
            after_document_snapshot: after,
        };
        if !matches!(
            transaction.command,
            SceneEditCommand::Undo | SceneEditCommand::Redo | SceneEditCommand::SaveScene { .. }
        ) {
            undo_stack.push(record.clone());
        }
        transaction.undo_record = Some(record);
    }
    transaction.after_summary = Some(format!("entity_count={}", document.entities.len()));
    SceneEditTransactionReport {
        schema_version: SCENE_EDIT_TRANSACTION_REPORT_SCHEMA_VERSION.to_string(),
        transaction_id: transaction.transaction_id,
        request_id: transaction.request_id,
        command_kind: transaction.command.kind().to_string(),
        status: transaction.status,
        target_scene_id: document.scene_id.clone(),
        affected_entity_ids: affected_entities(&transaction.write_set),
        read_set: transaction.read_set,
        write_set: transaction.write_set,
        diagnostics: transaction.diagnostics,
        dirty_after: document.dirty_state.dirty,
        preview_sync_status: None,
    }
}

fn reject(transaction: &mut SceneEditTransaction, code: &str, message: impl Into<String>) {
    transaction.status = SceneEditTransactionStatus::Rejected;
    transaction.diagnostics.push(SceneEditDiagnostic::error(
        code,
        "scene.transaction",
        message,
    ));
}

fn would_create_cycle(
    document: &EditorSceneDocument,
    entity_id: &str,
    new_parent_id: &str,
) -> bool {
    if entity_id == new_parent_id {
        return true;
    }
    let mut cursor = Some(new_parent_id.to_string());
    while let Some(current) = cursor {
        if current == entity_id {
            return true;
        }
        cursor = document
            .entity(&current)
            .and_then(|entity| entity.parent_id.clone());
    }
    false
}

fn affected_entities(write_set: &[String]) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for path in write_set {
        if let Some(rest) = path.strip_prefix("scene.entities.") {
            if let Some(id) = rest.split('.').next() {
                if !id.is_empty() {
                    ids.insert(id.to_string());
                }
            }
        }
    }
    ids.into_iter().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneEditTransactionStatus {
    Pending,
    Committed,
    Rejected,
    Failed,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneEditTransactionReport {
    pub schema_version: String,
    pub transaction_id: String,
    pub request_id: String,
    pub command_kind: String,
    pub status: SceneEditTransactionStatus,
    pub target_scene_id: String,
    pub affected_entity_ids: Vec<String>,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub diagnostics: Vec<SceneEditDiagnostic>,
    pub dirty_after: bool,
    pub preview_sync_status: Option<String>,
}


