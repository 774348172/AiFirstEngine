use super::*;
use crate::game_project_compiler::CompilerSourceView;
use authoring_project_context::ProjectSnapshot;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialProjectInspection {
    pub schema_version: String,
    pub supported_schemas: Vec<&'static str>,
    /// Source support is not a rendering, performance or platform qualification.
    pub qualification: &'static str,
    pub asset_count: usize,
    pub assets: Vec<SpatialAssetInspection>,
    pub scenes: Vec<SpatialSceneInspection>,
    pub prefabs: Vec<SpatialPrefabInspection>,
    pub diagnostics: Vec<SpatialInspectionDiagnostic>,
    pub truncated: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialAssetInspection {
    pub source_path: String,
    pub schema_version: String,
    pub asset_id: Option<String>,
    pub asset_guid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_budgets: Option<ImportBudgets>,
    /// Primary source bytes only; external buffers/images are charged by check/Cook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_bytes: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialSceneInspection {
    pub source_path: String,
    pub scene_id: String,
    pub main_camera: Option<String>,
    pub entity_count: usize,
    pub entities: Vec<SpatialEntityInspection>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialPrefabInspection {
    pub source_path: String,
    pub prefab_id: String,
    pub root_entity_id: String,
    pub entity_count: usize,
    pub entity_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialEntityInspection {
    pub entity_id: String,
    pub components: Vec<String>,
    pub mesh_renderer: Option<MeshRenderer3D>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<Light3D>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialInspectionDiagnostic {
    pub source_path: String,
    pub code: String,
    pub field_path: String,
    pub message: String,
}

/// Snapshot-only authoring inspection. The Provider never interprets spatial source data.
pub fn inspect_spatial_project_snapshot(snapshot: &ProjectSnapshot) -> SpatialProjectInspection {
    let mut report = SpatialProjectInspection {
        schema_version: "spatial-project-inspection.v1".into(),
        supported_schemas: vec![
            AUTHORING_SCENE_V2,
            AUTHORING_PREFAB_V2,
            "transform3d.v1",
            "camera3d.v1",
            "light3d.v1",
            "mesh-renderer3d.v1",
            "mesh-asset.v2",
            "material-asset.v2",
            "model-import.v1",
            "character3d.v1",
            "collision3d-asset.v1",
            "navigation3d-asset.v1",
            "terrain3d-asset.v1",
            "vegetation3d-asset.v1",
            "environment-asset.v1",
            "particle-effect.v1",
        ],
        qualification: "not_assessed_by_inspect",
        asset_count: 0,
        assets: Vec::new(),
        scenes: Vec::new(),
        prefabs: Vec::new(),
        diagnostics: Vec::new(),
        truncated: false,
    };
    let source = CompilerSourceView {
        files: snapshot
            .files
            .iter()
            .map(|f| (f.relative_path.clone(), f.bytes.clone()))
            .collect(),
    };
    for file in snapshot.files.iter().filter(|file| {
        file.relative_path.starts_with("Assets/")
            && (file.relative_path.ends_with(".json") || file.relative_path.ends_with(".asset"))
    }) {
        report.asset_count += 1;
        if report.assets.len() == 64 {
            report.truncated = true;
            continue;
        }
        // Only retain compact descriptor facts, never model/mesh/image payloads.
        let Ok(value) = serde_json::from_slice::<Value>(&file.bytes) else {
            continue;
        };
        let schema = value["schemaVersion"].as_str().unwrap_or_default();
        let import = (schema == "model-import.v1")
            .then(|| serde_json::from_value::<ModelImportV1>(value.clone()).ok())
            .flatten();
        let source_bytes = import
            .as_ref()
            .and_then(|import| source.bytes(&import.source))
            .map(|bytes| bytes.len() as u64);
        report.assets.push(SpatialAssetInspection {
            source_path: file.relative_path.clone(),
            schema_version: schema.into(),
            asset_id: value["assetId"].as_str().map(str::to_string),
            asset_guid: value["assetGuid"].as_str().map(str::to_string),
            effective_budgets: import.map(|import| import.budgets),
            source_bytes,
        });
    }
    let manifest = source.project_manifest_value().ok();
    let schema = manifest
        .as_ref()
        .and_then(|m| m["schemaVersion"].as_str())
        .unwrap_or_default();
    if let Err(failure) = super::check::validate_source(&source, schema) {
        report.diagnostics.push(SpatialInspectionDiagnostic {
            source_path: failure.source_path,
            code: failure.error.code,
            field_path: failure.error.field_path,
            message: failure.error.message,
        });
    }
    let mut remaining = 64;
    for file in snapshot
        .files
        .iter()
        .filter(|f| f.relative_path.starts_with("Scenes/") && f.relative_path.ends_with(".json"))
    {
        let Ok(scene) = serde_json::from_slice::<SpatialSourceScene>(&file.bytes) else {
            continue;
        };
        if scene.schema_version != AUTHORING_SCENE_V2 {
            continue;
        }
        if report.scenes.len() == 64 {
            report.truncated = true;
            break;
        }
        let entities = scene
            .entities
            .iter()
            .take(remaining)
            .map(|e| SpatialEntityInspection {
                entity_id: e.id.clone(),
                components: e
                    .components
                    .iter()
                    .map(|c| c.component_type.clone())
                    .collect(),
                light: e
                    .components
                    .iter()
                    .find(|c| c.component_type == "Light3D")
                    .and_then(|c| serde_json::from_value(c.data.clone()).ok()),
                mesh_renderer: e
                    .components
                    .iter()
                    .find(|c| c.component_type == "MeshRenderer3D")
                    .and_then(|c| serde_json::from_value(c.data.clone()).ok()),
            })
            .collect::<Vec<_>>();
        remaining -= entities.len();
        report.truncated |= entities.len() < scene.entities.len();
        report.scenes.push(SpatialSceneInspection {
            source_path: file.relative_path.clone(),
            scene_id: scene.id,
            main_camera: scene.main_camera,
            entity_count: scene.entities.len(),
            entities,
        });
    }
    for file in snapshot
        .files
        .iter()
        .filter(|f| f.relative_path.starts_with("Prefabs/") && f.relative_path.ends_with(".json"))
    {
        let Ok(prefab) = serde_json::from_slice::<SpatialSourcePrefab>(&file.bytes) else {
            continue;
        };
        if prefab.schema_version != AUTHORING_PREFAB_V2 {
            continue;
        }
        if report.scenes.len() + report.prefabs.len() == 64 {
            report.truncated = true;
            break;
        }
        let ids = prefab
            .entities
            .iter()
            .take(remaining)
            .map(|e| e.id.clone())
            .collect::<Vec<_>>();
        remaining -= ids.len();
        report.truncated |= ids.len() < prefab.entities.len();
        report.prefabs.push(SpatialPrefabInspection {
            source_path: file.relative_path.clone(),
            prefab_id: prefab.id,
            root_entity_id: prefab.root_entity_id,
            entity_count: prefab.entities.len(),
            entity_ids: ids,
        });
    }
    report
}
