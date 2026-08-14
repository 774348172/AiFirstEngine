use engine_runtime::components::ComponentTypeId;
use engine_runtime::physics2d::{Collider2D, Shape2D};
use serde::{Deserialize, Serialize};
use super::{editor_component_to_collider2d, EditorSceneDocument, EditorVec3, SceneSelection};
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


