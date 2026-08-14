use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use super::{EditorMesh, EditorSceneComponent, EditorTransform, EditorVec3};
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
            Self::SetComponentField { .. } => "SetComponentField",
            Self::SaveScene { .. } => "SaveScene",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
        }
    }
}


