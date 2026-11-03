//! Portable declared logical state. Entity incarnations and engine handles are excluded.
use crate::ProjectRuntimeValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldLogicalField3D {
    pub component: String,
    pub field: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldLogicalDeclaration3D {
    pub world_id: String,
    pub revision: u64,
    pub placements: Vec<String>,
    pub fields: Vec<WorldLogicalField3D>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldLogicalRecord3D {
    pub placement: String,
    pub prefab_guid: String,
    pub source_entity: String,
    pub component: String,
    pub field: String,
    pub value: ProjectRuntimeValue,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldLogicalSave3D {
    pub schema_version: String,
    pub world_id: String,
    pub revision: u64,
    pub declaration_digest: String,
    pub records: Vec<WorldLogicalRecord3D>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPersistence3D {
    pub schema_version: String,
    pub declaration: WorldLogicalDeclaration3D,
    #[serde(default)]
    pub request_id: u64,
    #[serde(default)]
    pub restore: Option<WorldLogicalSave3D>,
}
