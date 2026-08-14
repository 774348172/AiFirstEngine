use crate::{
    EditorAssetRef, EditorMesh, EditorSceneComponent, EditorSceneDocument, EditorTransform,
    EditorVec3, SceneEditCommand,
};
use editor_ui_model::AssetPlacementMode;
use serde::{Deserialize, Serialize};

pub const ASSET_PLACEMENT_REPORT_SCHEMA_VERSION: &str = "asset-placement-report.v1";
pub const ASSET_AUTHORING_LOOP_REPORT_SCHEMA_VERSION: &str = "asset-authoring-loop-report.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPlacementRequest {
    pub asset_id: String,
    pub asset_type: String,
    pub asset_guid: Option<String>,
    pub target_parent_id: Option<String>,
    pub local_position: Option<EditorVec3>,
    pub placement_mode: AssetPlacementMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPlacementPlan {
    pub scene_commands: Vec<SceneEditCommand>,
    pub selected_entity_id: Option<String>,
    pub created_entity_ids: Vec<String>,
    pub created_component_types: Vec<String>,
    pub report: AssetPlacementReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPlacementReport {
    pub schema_version: String,
    pub asset_id: String,
    pub asset_type: String,
    pub selected_entity_id: Option<String>,
    pub created_entity_ids: Vec<String>,
    pub created_component_types: Vec<String>,
    pub diagnostics: Vec<AssetPlacementDiagnostic>,
}

impl AssetPlacementReport {
    fn new(request: &AssetPlacementRequest) -> Self {
        Self {
            schema_version: ASSET_PLACEMENT_REPORT_SCHEMA_VERSION.to_string(),
            asset_id: request.asset_id.clone(),
            asset_type: request.asset_type.clone(),
            selected_entity_id: None,
            created_entity_ids: Vec::new(),
            created_component_types: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AssetPlacementDiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPlacementDiagnostic {
    pub severity: AssetPlacementDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

impl AssetPlacementDiagnostic {
    pub fn error(
        code: impl Into<String>,
        source_stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: AssetPlacementDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            source_stage: source_stage.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetPlacementDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

pub struct AssetPlacementResolver;

impl AssetPlacementResolver {
    pub fn resolve(
        document: &EditorSceneDocument,
        selected_entity_id: Option<&str>,
        request: AssetPlacementRequest,
    ) -> AssetPlacementPlan {
        let mut report = AssetPlacementReport::new(&request);
        let parent_id = parent_id_for_request(document, selected_entity_id, &request);
        if let Some(parent_id) = &parent_id {
            if !document.has_entity(parent_id) {
                report.diagnostics.push(AssetPlacementDiagnostic::error(
                    "asset_placement.parent_missing",
                    "resolve_parent",
                    format!("Cannot place asset under missing parent: {parent_id}"),
                ));
                return empty_plan(report);
            }
        }

        let asset_type = request.asset_type.trim().to_ascii_lowercase();
        let name = entity_name_from_asset_id(&request.asset_id);
        let created_entity_id = document.next_entity_id(&name);
        let local_transform = transform_for_request(&request);
        let mut created_component_types = Vec::new();
        let command = match asset_type.as_str() {
            "mesh" | "model" | "texture" | "sprite" => SceneEditCommand::CreateEntity {
                parent_id,
                name,
                mesh: Some(EditorMesh {
                    primitive: Some(asset_type.clone()),
                    asset_ref: Some(EditorAssetRef {
                        asset_id: request.asset_id.clone(),
                        asset_type_id: request.asset_type.clone(),
                        guid: request.asset_guid.clone(),
                        sub_asset_id: None,
                    }),
                    material_ref: None,
                    visible: true,
                    layer: "default".to_string(),
                }),
                components: Vec::new(),
                local_transform,
                sibling_order: None,
            },
            "prefab" => {
                let component_type = "engine.prefab_instance".to_string();
                created_component_types.push(component_type.clone());
                SceneEditCommand::CreateEntity {
                    parent_id,
                    name,
                    mesh: None,
                    components: vec![EditorSceneComponent {
                        component_type,
                        fields: serde_json::json!({
                            "source": {
                                "id": request.asset_id,
                                "type": request.asset_type,
                                "guid": request.asset_guid,
                            }
                        }),
                    }],
                    local_transform,
                    sibling_order: None,
                }
            }
            _ => {
                report.diagnostics.push(AssetPlacementDiagnostic::error(
                    "asset_placement.asset_type_unsupported",
                    "resolve_asset_type",
                    format!(
                        "Unsupported asset type for Scene placement: {}",
                        request.asset_type
                    ),
                ));
                return empty_plan(report);
            }
        };

        report.selected_entity_id = Some(created_entity_id.clone());
        report.created_entity_ids = vec![created_entity_id.clone()];
        report.created_component_types = created_component_types.clone();
        AssetPlacementPlan {
            scene_commands: vec![command],
            selected_entity_id: Some(created_entity_id),
            created_entity_ids: report.created_entity_ids.clone(),
            created_component_types,
            report,
        }
    }
}

fn empty_plan(report: AssetPlacementReport) -> AssetPlacementPlan {
    AssetPlacementPlan {
        scene_commands: Vec::new(),
        selected_entity_id: None,
        created_entity_ids: Vec::new(),
        created_component_types: Vec::new(),
        report,
    }
}

fn parent_id_for_request(
    document: &EditorSceneDocument,
    selected_entity_id: Option<&str>,
    request: &AssetPlacementRequest,
) -> Option<String> {
    match request.placement_mode {
        AssetPlacementMode::WorldOrigin => request.target_parent_id.clone(),
        AssetPlacementMode::UnderSelectedOrRoot => request.target_parent_id.clone().or_else(|| {
            selected_entity_id
                .filter(|id| document.has_entity(id))
                .map(str::to_string)
        }),
    }
}

fn transform_for_request(request: &AssetPlacementRequest) -> EditorTransform {
    let mut transform = EditorTransform::identity();
    if let Some(local_position) = request.local_position {
        transform.local_position = local_position;
    }
    transform
}

fn entity_name_from_asset_id(asset_id: &str) -> String {
    let name = asset_id
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(asset_id)
        .trim()
        .trim_end_matches(".glb")
        .trim_end_matches(".fbx")
        .trim_end_matches(".png")
        .trim_end_matches(".jpg")
        .trim_end_matches(".prefab")
        .replace(['_', '-'], " ");
    if name.is_empty() {
        "Asset".to_string()
    } else {
        let mut chars = name.chars();
        match chars.next() {
            Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
            None => "Asset".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_placement_mesh_generates_create_entity_with_mesh_ref() {
        let document = EditorSceneDocument::new("scene-main", "Main");
        let plan = AssetPlacementResolver::resolve(
            &document,
            None,
            AssetPlacementRequest {
                asset_id: "model-player".to_string(),
                asset_type: "model".to_string(),
                asset_guid: Some("guid-player".to_string()),
                target_parent_id: None,
                local_position: Some(EditorVec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                }),
                placement_mode: AssetPlacementMode::WorldOrigin,
            },
        );

        assert_eq!(plan.created_entity_ids, vec!["entity-model-player"]);
        assert_eq!(plan.scene_commands.len(), 1);
        match &plan.scene_commands[0] {
            SceneEditCommand::CreateEntity {
                mesh,
                local_transform,
                ..
            } => {
                let mesh = mesh.as_ref().expect("mesh should be generated");
                assert_eq!(mesh.asset_ref.as_ref().unwrap().asset_id, "model-player");
                assert_eq!(local_transform.local_position.x, 1.0);
            }
            _ => panic!("expected CreateEntity"),
        }
    }

    #[test]
    fn asset_placement_prefab_generates_prefab_instance_component() {
        let document = EditorSceneDocument::new("scene-main", "Main");
        let plan = AssetPlacementResolver::resolve(
            &document,
            None,
            AssetPlacementRequest {
                asset_id: "prefab-enemy".to_string(),
                asset_type: "prefab".to_string(),
                asset_guid: None,
                target_parent_id: None,
                local_position: None,
                placement_mode: AssetPlacementMode::WorldOrigin,
            },
        );

        assert_eq!(
            plan.created_component_types,
            vec!["engine.prefab_instance".to_string()]
        );
        match &plan.scene_commands[0] {
            SceneEditCommand::CreateEntity {
                mesh, components, ..
            } => {
                assert!(mesh.is_none());
                assert_eq!(components[0].component_type, "engine.prefab_instance");
            }
            _ => panic!("expected CreateEntity"),
        }
    }

    #[test]
    fn asset_placement_unsupported_type_returns_diagnostic_without_commands() {
        let document = EditorSceneDocument::new("scene-main", "Main");
        let plan = AssetPlacementResolver::resolve(
            &document,
            None,
            AssetPlacementRequest {
                asset_id: "sound-laser".to_string(),
                asset_type: "audio".to_string(),
                asset_guid: None,
                target_parent_id: None,
                local_position: None,
                placement_mode: AssetPlacementMode::WorldOrigin,
            },
        );

        assert!(plan.scene_commands.is_empty());
        assert!(plan.report.has_errors());
        assert_eq!(
            plan.report.diagnostics[0].code,
            "asset_placement.asset_type_unsupported"
        );
    }
}
