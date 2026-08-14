use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use editor_ui_model::EditorAssetRef;

use super::EditorTransform;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSceneEntity {
    #[serde(rename = "schemaVersion", default = "default_entity_schema_version")]
    pub schema_version: String,
    #[serde(rename = "id")]
    pub entity_id: String,
    pub name: String,
    #[serde(default = "default_entity_kind")]
    pub kind: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub sibling_order: i32,
    #[serde(default)]
    pub transform: Option<EditorTransform>,
    #[serde(default)]
    pub mesh: Option<EditorMesh>,
    #[serde(default)]
    pub components: Vec<EditorSceneComponent>,
}

impl EditorSceneEntity {
    pub fn new(entity_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: default_entity_schema_version(),
            entity_id: entity_id.into(),
            name: name.into(),
            kind: default_entity_kind(),
            enabled: true,
            parent_id: None,
            sibling_order: 0,
            transform: Some(EditorTransform::identity()),
            mesh: None,
            components: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorMesh {
    #[serde(default)]
    pub primitive: Option<String>,
    #[serde(default)]
    pub asset_ref: Option<EditorAssetRef>,
    #[serde(default)]
    pub material_ref: Option<EditorAssetRef>,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_layer")]
    pub layer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSceneComponent {
    #[serde(alias = "componentType")]
    pub component_type: String,
    #[serde(default, alias = "data")]
    pub fields: Value,
}

fn default_entity_schema_version() -> String {
    "runtime-entity.v1".to_string()
}

fn default_entity_kind() -> String {
    "entity".to_string()
}

fn default_layer() -> String {
    "default".to_string()
}

fn default_true() -> bool {
    true
}
