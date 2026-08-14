use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::EditorAssetRef;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectorModel {
    pub selected_entity_id: Option<String>,
    pub title: String,
    pub sections: Vec<InspectorSection>,
    pub readonly: bool,
    pub persistence: InspectorPersistence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectorSection {
    pub section_id: String,
    pub title: String,
    pub fields: Vec<InspectorField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectorField {
    pub field_id: String,
    pub label: String,
    pub value: InspectorValue,
    pub value_type: InspectorValueType,
    pub path: String,
    pub readonly: bool,
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InspectorValue {
    String(String),
    Bool(bool),
    Number(f64),
    Vec3(Vec3),
    AssetRef(EditorAssetRef),
    EntityRef(String),
    Json(serde_json::Value),
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectorValueType {
    String,
    Bool,
    Number,
    Vec3,
    AssetRef,
    EntityRef,
    Json,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectorPersistence {
    PersistentAuthoring,
    TemporaryPlaySession,
    ReadOnlyRuntimePackage,
    ReadOnly,
}

impl Default for InspectorPersistence {
    fn default() -> Self {
        Self::ReadOnly
    }
}
