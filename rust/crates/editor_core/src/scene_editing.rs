use engine_runtime::archetype::ComponentValue;
use engine_runtime::components::{ComponentTypeId, Hierarchy, Renderable, Transform};
use engine_runtime::ids::EntityId;
use engine_runtime::math::{Vec2, Vec3};
use engine_runtime::physics2d::{Collider2D, PhysicsLayer, PhysicsMask, Shape2D};
use engine_runtime::world::World;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

mod document;
mod entity;
mod transform;

pub use document::{EditorSceneDocument, SceneDirtyState};
pub use entity::{EditorAssetRef, EditorMesh, EditorSceneComponent, EditorSceneEntity};
pub use transform::{EditorTransform, EditorVec3};

pub const SCENE_EDIT_TRANSACTION_REPORT_SCHEMA_VERSION: &str = "scene-edit-transaction-report.v1";
pub const PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION: &str = "preview-world-sync-report.v1";
pub const EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION: &str = "editor-scene-document.v1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSelection {
    pub selected_entity_ids: Vec<String>,
    pub primary_entity_id: Option<String>,
}

impl SceneSelection {
    pub fn select_single(&mut self, document: &EditorSceneDocument, entity_id: &str) -> bool {
        if !document.has_entity(entity_id) {
            self.clear();
            return false;
        }
        self.selected_entity_ids = vec![entity_id.to_string()];
        self.primary_entity_id = Some(entity_id.to_string());
        true
    }

    pub fn clear(&mut self) {
        self.selected_entity_ids.clear();
        self.primary_entity_id = None;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneEditRequest {
    pub request_id: String,
    pub source: SceneEditRequestSource,
    pub target_scene_id: String,
    pub command: SceneEditCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneEditRequestSource {
    SceneView,
    Hierarchy,
    Inspector,
    Toolbar,
    Ai,
    Test,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "commandType",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SceneEditCommand {
    SelectEntity {
        entity_id: String,
    },
    CreateEntity {
        parent_id: Option<String>,
        name: String,
        #[serde(default)]
        mesh: Option<EditorMesh>,
        #[serde(default)]
        components: Vec<EditorSceneComponent>,
        local_transform: EditorTransform,
        sibling_order: Option<i32>,
    },
    DeleteEntity {
        entity_id: String,
        delete_children: bool,
    },
    RenameEntity {
        entity_id: String,
        name: String,
    },
    DuplicateEntity {
        entity_id: String,
    },
    ReparentEntity {
        entity_id: String,
        new_parent_id: Option<String>,
        sibling_order: Option<i32>,
        keep_world_transform: bool,
    },
    SetTransform {
        entity_id: String,
        local_position: Option<EditorVec3>,
        local_rotation: Option<EditorVec3>,
        local_scale: Option<EditorVec3>,
    },
    AddComponent {
        entity_id: String,
        component_type: String,
        fields: Value,
    },
    RemoveComponent {
        entity_id: String,
        component_type: String,
    },
    SetComponentField {
        entity_id: String,
        component_type: String,
        field_path: String,
        value: Value,
    },
    SaveScene {
        scene_id: String,
        path: Option<PathBuf>,
    },
    Undo,
    Redo,
}

impl SceneEditCommand {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SelectEntity { .. } => "SelectEntity",
            Self::CreateEntity { .. } => "CreateEntity",
            Self::DeleteEntity { .. } => "DeleteEntity",
            Self::RenameEntity { .. } => "RenameEntity",
            Self::DuplicateEntity { .. } => "DuplicateEntity",
            Self::ReparentEntity { .. } => "ReparentEntity",
            Self::SetTransform { .. } => "SetTransform",
            Self::AddComponent { .. } => "AddComponent",
            Self::RemoveComponent { .. } => "RemoveComponent",
            Self::SetComponentField { .. } => "SetComponentField",
            Self::SaveScene { .. } => "SaveScene",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
        }
    }
}

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
            SceneEditCommand::AddComponent {
                entity_id,
                component_type,
                fields,
            } => {
                transaction
                    .read_set
                    .push(format!("scene.entities.{entity_id}.components"));
                transaction.write_set.push(format!(
                    "scene.entities.{entity_id}.components.{component_type}"
                ));
                let Some(entity) = document.entity_mut(entity_id) else {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot add component to missing entity: {entity_id}"),
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
                if component_type.trim().is_empty() || !fields.is_object() {
                    reject(
                        &mut transaction,
                        "scene.component.invalid",
                        "Scene component type must be non-empty and fields must be an object.",
                    );
                } else if entity
                    .components
                    .iter()
                    .any(|component| component.component_type == *component_type)
                {
                    reject(
                        &mut transaction,
                        "scene.component.duplicate",
                        format!("Entity {entity_id} already has component {component_type}."),
                    );
                } else {
                    entity.components.push(EditorSceneComponent {
                        component_type: component_type.clone(),
                        fields: fields.clone(),
                    });
                    transaction.status = SceneEditTransactionStatus::Committed;
                    dirty = true;
                }
            }
            SceneEditCommand::RemoveComponent {
                entity_id,
                component_type,
            } => {
                transaction.read_set.push(format!(
                    "scene.entities.{entity_id}.components.{component_type}"
                ));
                transaction.write_set.push(format!(
                    "scene.entities.{entity_id}.components.{component_type}"
                ));
                let Some(entity) = document.entity_mut(entity_id) else {
                    reject(
                        &mut transaction,
                        "scene.entity.missing",
                        format!("Cannot remove component from missing entity: {entity_id}"),
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
                let Some(index) = entity
                    .components
                    .iter()
                    .position(|component| component.component_type == *component_type)
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
                entity.components.remove(index);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneEditDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneEditDiagnostic {
    pub severity: SceneEditDiagnosticSeverity,
    pub code: String,
    pub layer: String,
    pub message: String,
    pub path: Option<String>,
    pub entity_id: Option<String>,
}

impl SceneEditDiagnostic {
    pub fn info(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(SceneEditDiagnosticSeverity::Info, code, layer, message)
    }

    pub fn warning(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(SceneEditDiagnosticSeverity::Warning, code, layer, message)
    }

    pub fn error(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(SceneEditDiagnosticSeverity::Error, code, layer, message)
    }

    fn new(
        severity: SceneEditDiagnosticSeverity,
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            layer: layer.into(),
            message: message.into(),
            path: None,
            entity_id: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_entity_id(mut self, entity_id: impl Into<String>) -> Self {
        self.entity_id = Some(entity_id.into());
        self
    }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneUndoRecord {
    pub transaction_id: String,
    pub command_kind: String,
    pub before_document_snapshot: EditorSceneDocument,
    pub after_document_snapshot: EditorSceneDocument,
}

#[derive(Debug, Clone, Default)]
pub struct SceneUndoStack {
    undo_stack: Vec<SceneUndoRecord>,
    redo_stack: Vec<SceneUndoRecord>,
}

impl SceneUndoStack {
    pub fn push(&mut self, record: SceneUndoRecord) {
        self.undo_stack.push(record);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, document: &mut EditorSceneDocument) -> Option<SceneUndoRecord> {
        let record = self.undo_stack.pop()?;
        *document = record.before_document_snapshot.clone();
        document.mark_dirty(format!("undo-{}", record.transaction_id));
        self.redo_stack.push(record.clone());
        Some(record)
    }

    pub fn redo(&mut self, document: &mut EditorSceneDocument) -> Option<SceneUndoRecord> {
        let record = self.redo_stack.pop()?;
        *document = record.after_document_snapshot.clone();
        document.mark_dirty(format!("redo-{}", record.transaction_id));
        self.undo_stack.push(record.clone());
        Some(record)
    }
}

pub struct PreviewWorldSync;

impl PreviewWorldSync {
    pub fn full_rebuild(
        document: &EditorSceneDocument,
    ) -> Result<(World, PreviewWorldSyncReport), PreviewWorldSyncReport> {
        let mut diagnostics = document.validate();
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == SceneEditDiagnosticSeverity::Error)
        {
            return Err(PreviewWorldSyncReport {
                schema_version: PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION.to_string(),
                scene_id: document.scene_id.clone(),
                sync_mode: "full_rebuild".to_string(),
                entity_count: 0,
                component_count: 0,
                diagnostics,
            });
        }

        let mut world = World::new();
        let mut component_count = 0;
        for entity in &document.entities {
            let transform = entity.transform.map(|value| Transform {
                local_position: to_runtime_vec3(value.local_position),
                local_rotation: to_runtime_vec3(value.local_rotation),
                local_scale: to_runtime_vec3(value.local_scale),
            });
            if transform.is_some() {
                component_count += 1;
            }
            let renderable = entity.mesh.as_ref().map(|mesh| Renderable {
                mesh_ref: mesh
                    .asset_ref
                    .as_ref()
                    .map(|asset_ref| asset_ref.asset_id.clone()),
                material_ref: mesh
                    .material_ref
                    .as_ref()
                    .map(|asset_ref| asset_ref.asset_id.clone()),
                visible: mesh.visible,
                layer: mesh.layer.clone(),
            });
            if renderable.is_some() {
                component_count += 1;
            }
            component_count += entity.components.len();
            if let Err(error) = world.try_spawn_with_components(
                EntityId::new(entity.entity_id.clone()),
                entity.name.clone(),
                entity.kind.clone(),
                entity.enabled,
                Hierarchy {
                    parent_id: entity.parent_id.clone().map(EntityId::new),
                    sibling_order: entity.sibling_order,
                },
                transform,
                renderable,
            ) {
                diagnostics.push(preview_world_mutation_diagnostic(error));
                return Err(failed_preview_world_sync_report(document, diagnostics));
            }
            for component in &entity.components {
                let entity_id = EntityId::new(entity.entity_id.clone());
                if let Some(collider) = editor_component_to_collider2d(component) {
                    let value = ComponentValue::Collider2D(collider);
                    if let Err(error) = world.try_insert_component_value(
                        entity_id,
                        ComponentTypeId::collider2d(),
                        value,
                    ) {
                        diagnostics.push(preview_world_mutation_diagnostic(error));
                        return Err(failed_preview_world_sync_report(document, diagnostics));
                    }
                } else {
                    if let Err(error) = world.try_insert_dynamic_component(
                        entity_id,
                        ComponentTypeId::new(component.component_type.clone()),
                        component.fields.to_string(),
                    ) {
                        diagnostics.push(preview_world_mutation_diagnostic(error));
                        return Err(failed_preview_world_sync_report(document, diagnostics));
                    }
                }
            }
        }
        diagnostics.push(SceneEditDiagnostic::info(
            "scene.preview_world.full_rebuild",
            "scene.preview_world",
            format!(
                "PreviewWorld full rebuild completed: entities={}",
                document.entities.len()
            ),
        ));
        let report = PreviewWorldSyncReport {
            schema_version: PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION.to_string(),
            scene_id: document.scene_id.clone(),
            sync_mode: "full_rebuild".to_string(),
            entity_count: document.entities.len(),
            component_count,
            diagnostics,
        };
        Ok((world, report))
    }
}

fn preview_world_mutation_diagnostic(
    error: engine_runtime::world::WorldMutationError,
) -> SceneEditDiagnostic {
    let entity_id = error.source_entity_id.as_ref().map(ToString::to_string);
    let mut diagnostic = SceneEditDiagnostic::error(
        error.code,
        "scene.preview_world",
        match error.suggested_fix {
            Some(next_action) => format!("{} Next action: {next_action}", error.message),
            None => error.message,
        },
    );
    if let Some(entity_id) = entity_id {
        diagnostic = diagnostic.with_entity_id(entity_id);
    }
    diagnostic
}

fn failed_preview_world_sync_report(
    document: &EditorSceneDocument,
    diagnostics: Vec<SceneEditDiagnostic>,
) -> PreviewWorldSyncReport {
    PreviewWorldSyncReport {
        schema_version: PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION.to_string(),
        scene_id: document.scene_id.clone(),
        sync_mode: "full_rebuild".to_string(),
        entity_count: 0,
        component_count: 0,
        diagnostics,
    }
}

fn editor_component_to_collider2d(component: &EditorSceneComponent) -> Option<Collider2D> {
    if component.component_type != ComponentTypeId::collider2d().as_str() {
        return None;
    }
    let fields = component.fields.as_object()?;
    let shape_name = fields
        .get("shape")
        .and_then(Value::as_str)
        .unwrap_or("aabb")
        .to_ascii_lowercase();
    let shape = match shape_name.as_str() {
        "circle" => Shape2D::Circle {
            radius: fields.get("radius").and_then(Value::as_f64).unwrap_or(0.5) as f32,
        },
        _ => {
            let half_extents = fields.get("halfExtents");
            Shape2D::Aabb {
                half_extents: Vec2 {
                    x: half_extents
                        .and_then(|value| value.get("x"))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.5) as f32,
                    y: half_extents
                        .and_then(|value| value.get("y"))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.5) as f32,
                },
            }
        }
    };
    let offset = fields.get("offset");
    Some(Collider2D {
        shape,
        offset: Vec2 {
            x: offset
                .and_then(|value| value.get("x"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
            y: offset
                .and_then(|value| value.get("y"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
        },
        layer: fields
            .get("layer")
            .and_then(Value::as_u64)
            .map(|value| PhysicsLayer(value as u32))
            .unwrap_or(PhysicsLayer::DEFAULT),
        mask: fields
            .get("mask")
            .and_then(Value::as_u64)
            .map(|value| PhysicsMask(value as u32))
            .unwrap_or(PhysicsMask::ALL),
        enabled: fields
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        is_sensor: fields
            .get("isSensor")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn is_supported_component_field_path(field_path: &str) -> bool {
    let trimmed = field_path.trim();
    !trimmed.is_empty()
        && !trimmed.contains("..")
        && trimmed.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        })
}

fn set_json_object_path(object: &mut Map<String, Value>, field_path: &str, value: Value) {
    let mut segments = field_path.split('.').peekable();
    let mut current = object;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            current.insert(segment.to_string(), value);
            return;
        }
        let entry = current
            .entry(segment.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        current = entry
            .as_object_mut()
            .expect("entry was converted to object");
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderDebugDrawList {
    pub schema_version: String,
    pub scene_id: String,
    pub collider_count: usize,
    pub draw_item_count: usize,
    pub selected_entity_id: Option<String>,
    pub invalid_collider_count: usize,
    pub missing_transform_count: usize,
    pub draw_items: Vec<ColliderDebugDrawItem>,
    pub diagnostics: Vec<ColliderDebugDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderDebugDrawItem {
    pub entity_id: String,
    pub shape: ColliderDebugShape,
    pub center: EditorVec3,
    pub enabled: bool,
    pub sensor: bool,
    pub selected: bool,
    pub layer: u32,
    pub mask: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "shapeKind")]
pub enum ColliderDebugShape {
    Aabb { half_extents: EditorVec3 },
    Circle { radius: f32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderDebugDiagnostic {
    pub severity: String,
    pub entity_id: Option<String>,
    pub component_type: String,
    pub field_path: String,
    pub message: String,
    pub suggestion: String,
}

impl ColliderDebugDrawList {
    pub fn build(document: &EditorSceneDocument, selection: &SceneSelection) -> Self {
        let selected = selection.primary_entity_id.clone();
        let mut list = Self {
            schema_version: "collider-debug-draw-list.v1".to_string(),
            scene_id: document.scene_id.clone(),
            selected_entity_id: selected.clone(),
            ..Self::default()
        };

        for entity in &document.entities {
            for component in entity.components.iter().filter(|component| {
                component.component_type == ComponentTypeId::collider2d().as_str()
            }) {
                list.collider_count += 1;
                let Some(collider) = editor_component_to_collider2d(component) else {
                    list.invalid_collider_count += 1;
                    list.diagnostics.push(ColliderDebugDiagnostic {
                        severity: "error".to_string(),
                        entity_id: Some(entity.entity_id.clone()),
                        component_type: component.component_type.clone(),
                        field_path: "shape".to_string(),
                        message: "Collider2D fields could not be parsed.".to_string(),
                        suggestion: "Use Aabb or Circle collider fields.".to_string(),
                    });
                    continue;
                };
                if let Some(message) = collider_validation_message(&collider) {
                    list.invalid_collider_count += 1;
                    list.diagnostics.push(ColliderDebugDiagnostic {
                        severity: "error".to_string(),
                        entity_id: Some(entity.entity_id.clone()),
                        component_type: component.component_type.clone(),
                        field_path: "shape".to_string(),
                        message,
                        suggestion: "Use positive collider dimensions.".to_string(),
                    });
                    continue;
                }
                let Some(transform) = entity.transform else {
                    list.missing_transform_count += 1;
                    list.diagnostics.push(ColliderDebugDiagnostic {
                        severity: "warning".to_string(),
                        entity_id: Some(entity.entity_id.clone()),
                        component_type: component.component_type.clone(),
                        field_path: "transform".to_string(),
                        message: "Collider2D entity is missing Transform.".to_string(),
                        suggestion: "Add Transform before drawing collider overlay.".to_string(),
                    });
                    continue;
                };
                let center = EditorVec3 {
                    x: transform.local_position.x + collider.offset.x,
                    y: transform.local_position.y + collider.offset.y,
                    z: transform.local_position.z,
                };
                list.draw_items.push(ColliderDebugDrawItem {
                    entity_id: entity.entity_id.clone(),
                    shape: match collider.shape {
                        Shape2D::Aabb { half_extents } => ColliderDebugShape::Aabb {
                            half_extents: EditorVec3 {
                                x: half_extents.x,
                                y: half_extents.y,
                                z: 0.0,
                            },
                        },
                        Shape2D::Circle { radius } => ColliderDebugShape::Circle { radius },
                    },
                    center,
                    enabled: collider.enabled,
                    sensor: collider.is_sensor,
                    selected: selected.as_deref() == Some(entity.entity_id.as_str()),
                    layer: collider.layer.0,
                    mask: collider.mask.0,
                });
            }
        }
        list.draw_item_count = list.draw_items.len();
        list
    }
}

fn collider_validation_message(collider: &Collider2D) -> Option<String> {
    match collider.shape {
        Shape2D::Aabb { half_extents } => {
            if half_extents.x <= 0.0 || half_extents.y <= 0.0 {
                Some("Aabb Collider2D halfExtents must be positive.".to_string())
            } else {
                None
            }
        }
        Shape2D::Circle { radius } => {
            if radius <= 0.0 {
                Some("Circle Collider2D radius must be positive.".to_string())
            } else {
                None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWorldSyncReport {
    pub schema_version: String,
    pub scene_id: String,
    pub sync_mode: String,
    pub entity_count: usize,
    pub component_count: usize,
    pub diagnostics: Vec<SceneEditDiagnostic>,
}

pub struct SceneSavePipeline;

impl SceneSavePipeline {
    pub fn save(
        document: &mut EditorSceneDocument,
        project_root: impl AsRef<Path>,
        path: Option<impl AsRef<Path>>,
    ) -> SceneSaveReport {
        let scope = match crate::ProjectWriteScope::open(project_root.as_ref()) {
            Ok(scope) => scope,
            Err(error) => {
                return SceneSaveReport::failed(
                    &document.scene_id,
                    PathBuf::new(),
                    error.code,
                    error.to_string(),
                    document.dirty_state.dirty,
                );
            }
        };
        Self::save_in_scope(document, &scope, path)
    }

    pub fn save_in_scope(
        document: &mut EditorSceneDocument,
        scope: &crate::ProjectWriteScope,
        path: Option<impl AsRef<Path>>,
    ) -> SceneSaveReport {
        let target_path = match path {
            Some(path) => path.as_ref().to_path_buf(),
            None => match document.scene_path.clone() {
                Some(path) => path,
                None => {
                    return SceneSaveReport::failed(
                        &document.scene_id,
                        PathBuf::new(),
                        "scene.save.path_required",
                        "SaveScene requires a path for documents that were not loaded from disk.",
                        document.dirty_state.dirty,
                    );
                }
            },
        };
        let project_root = normalize_path(scope.display_root());
        let normalized_target = normalize_path(&target_path);
        if !normalized_target.starts_with(&project_root) {
            return SceneSaveReport::failed(
                &document.scene_id,
                target_path,
                "scene.save.path_outside_project",
                "Scene save path must be inside the project root.",
                document.dirty_state.dirty,
            );
        }
        if normalized_target.components().any(|component| {
            matches!(component, Component::Normal(value) if {
                let lower = value.to_string_lossy().to_ascii_lowercase();
                lower == "runtime-package" || lower == "runtime_package"
            })
        }) {
            return SceneSaveReport::failed(
                &document.scene_id,
                target_path,
                "scene.save.runtime_package_output",
                "Scene source files cannot be saved into Runtime Package output directories.",
                document.dirty_state.dirty,
            );
        }
        let diagnostics = document.validate();
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == SceneEditDiagnosticSeverity::Error)
        {
            return SceneSaveReport {
                scene_id: document.scene_id.clone(),
                path: target_path,
                status: SceneSaveStatus::Failed,
                diagnostics,
                dirty_after: document.dirty_state.dirty,
            };
        }
        let relative_target = normalized_target
            .strip_prefix(&project_root)
            .expect("target containment was checked above");
        let same_current_path = document
            .scene_path
            .as_deref()
            .map(normalize_path)
            .is_some_and(|current| current == normalized_target);
        if same_current_path && !document.dirty_state.dirty && scope.read(relative_target).is_ok() {
            return SceneSaveReport::unchanged(&document.scene_id, normalized_target);
        }
        let text = match document.to_stable_json() {
            Ok(text) => text,
            Err(error) => {
                return SceneSaveReport::failed(
                    &document.scene_id,
                    target_path,
                    "scene.save.serialize_failed",
                    format!("Failed to serialize scene: {error}"),
                    true,
                );
            }
        };
        if scope
            .read(relative_target)
            .is_ok_and(|current| current == text.as_bytes())
        {
            document.scene_path = Some(normalized_target.clone());
            document.clear_dirty();
            return SceneSaveReport::unchanged(&document.scene_id, normalized_target);
        }
        if let Err(error) = scope.write_atomic(relative_target, text.as_bytes()) {
            return SceneSaveReport::failed(
                &document.scene_id,
                target_path,
                "scene.save.atomic_replace_failed",
                format!("Failed to atomically save scene file: {error}"),
                true,
            );
        }
        document.scene_path = Some(normalized_target.clone());
        document.clear_dirty();
        SceneSaveReport {
            scene_id: document.scene_id.clone(),
            path: normalized_target,
            status: SceneSaveStatus::Saved,
            diagnostics: Vec::new(),
            dirty_after: document.dirty_state.dirty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneSaveStatus {
    Saved,
    Unchanged,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSaveReport {
    pub scene_id: String,
    pub path: PathBuf,
    pub status: SceneSaveStatus,
    pub diagnostics: Vec<SceneEditDiagnostic>,
    pub dirty_after: bool,
}

impl SceneSaveReport {
    fn unchanged(scene_id: &str, path: PathBuf) -> Self {
        Self {
            scene_id: scene_id.to_string(),
            path,
            status: SceneSaveStatus::Unchanged,
            diagnostics: Vec::new(),
            dirty_after: false,
        }
    }

    fn failed(
        scene_id: &str,
        path: PathBuf,
        code: &str,
        message: impl Into<String>,
        dirty_after: bool,
    ) -> Self {
        Self {
            scene_id: scene_id.to_string(),
            path,
            status: SceneSaveStatus::Failed,
            diagnostics: vec![SceneEditDiagnostic::error(code, "scene.save", message)],
            dirty_after,
        }
    }
}

fn to_runtime_vec3(value: EditorVec3) -> Vec3 {
    Vec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(value) => normalized.push(value),
        }
    }
    normalized
}

#[cfg(test)]
mod tests;
