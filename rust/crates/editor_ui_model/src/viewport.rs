use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AssetPlacementMode {
    WorldOrigin,
    UnderSelectedOrRoot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportModel {
    pub scene_id: Option<String>,
    pub frame: u64,
    pub frame_hash: Option<String>,
    pub texture_id: Option<String>,
    pub target_id: Option<String>,
    pub renderable_count: usize,
    pub selected_entity: Option<EntitySummary>,
    pub renderables: Vec<RenderableSummary>,
    pub collider_overlay: ColliderOverlayModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySummary {
    pub entity_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderableSummary {
    pub entity_id: String,
    pub mesh_ref: Option<String>,
    pub material_ref: Option<String>,
    pub local_position: Vec3,
    pub visible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderOverlayModel {
    pub collider_count: usize,
    pub draw_item_count: usize,
    pub selected_entity_id: Option<String>,
    pub invalid_collider_count: usize,
    pub missing_transform_count: usize,
    pub draw_items: Vec<ColliderOverlayItem>,
    pub diagnostics: Vec<ColliderOverlayDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderOverlayItem {
    pub entity_id: String,
    pub shape: ColliderOverlayShape,
    pub center: Vec3,
    pub enabled: bool,
    pub sensor: bool,
    pub selected: bool,
    pub layer: u32,
    pub mask: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "shapeKind")]
pub enum ColliderOverlayShape {
    Aabb { half_extents: Vec3 },
    Circle { radius: f32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderOverlayDiagnostic {
    pub severity: String,
    pub entity_id: Option<String>,
    pub component_type: String,
    pub field_path: String,
    pub message: String,
    pub suggestion: String,
}
