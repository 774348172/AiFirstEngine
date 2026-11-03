//! Project cell demands. GPU ownership and completion never cross this interface.
use crate::ProjectRuntimeSpatialAssetRef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeCellEntity {
    pub entity_id: String,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeVegetationFields3D {
    pub component_type: String,
    pub source_id: String,
    pub cluster_key: String,
    pub cluster_asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectRuntimeCellIntent {
    Load {
        request_id: u64,
        placement_id: String,
        prefab: ProjectRuntimeSpatialAssetRef,
        parent: Option<ProjectRuntimeCellEntity>,
        render_required: bool,
    },
    Release {
        request_id: u64,
    },
    PromoteVegetation {
        request_id: u64,
        placement_id: String,
        owner: ProjectRuntimeCellEntity,
        source_instance_id: String,
        target: ProjectRuntimeCellEntity,
        fields: ProjectRuntimeVegetationFields3D,
    },
    Migrate {
        request_id: u64,
        persistent_request_id: u64,
        source: ProjectRuntimeCellEntity,
        persistent_placement_id: String,
        parent: Option<ProjectRuntimeCellEntity>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRuntimeCellStatus {
    Accepted,
    Loading,
    Prepared,
    Uploading,
    Active,
    Released,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeCellReceipt {
    pub request_id: u64,
    pub placement_id: String,
    pub status: ProjectRuntimeCellStatus,
    pub world_epoch: u64,
    pub asset_revision_epoch: u64,
    pub activation_epoch: Option<u64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// Exactly one of request_id / placement_id; source_entity_id resolves a non-owning soft reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeCellQuery {
    pub request_id: Option<u64>,
    pub placement_id: Option<String>,
    pub source_entity_id: Option<String>,
}
