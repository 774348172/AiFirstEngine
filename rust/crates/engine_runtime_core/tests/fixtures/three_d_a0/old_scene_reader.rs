// Source runtime_package.rs sha256:f7b2f4b042ca08c9a77c4d562d517d648fce2df3889b56017a70f1c9a2f184c2
// Frozen A0 baseline extraction, not the new parser in a legacy mode.
use engine_runtime::animator2d::RuntimeAnimator2D;
use engine_runtime::diagnostics::RuntimeDiagnostics;
use engine_runtime::runtime_package::RuntimeAssetManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
const RUNTIME_SCENE_SCHEMA_VERSION: &str = "runtime-scene.v1";
const RUNTIME_ENTITY_SCHEMA_VERSION: &str = "runtime-entity.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeScene {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub gravity: f32,
    pub background: String,
    pub sky_color: String,
    pub entities: Vec<RuntimeEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEntity {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub parent_id: Option<String>,
    pub sibling_order: i32,
    pub transform: Option<RuntimeTransform>,
    pub mesh: Option<RuntimeMesh>,
    #[serde(default, rename = "spriteRenderer2D", alias = "spriteRenderer2d")]
    pub sprite_renderer2d: Option<RuntimeSpriteRenderer2D>,
    #[serde(default)]
    pub animator2d: Option<RuntimeAnimator2D>,
    #[serde(default)]
    pub components: Vec<RuntimeProjectComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProjectComponent {
    #[serde(alias = "componentType")]
    pub component_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePrefabData {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub root_entity_id: Option<String>,
    pub entities: Vec<RuntimeEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTransform {
    pub local_position: Vector3,
    pub local_rotation: Vector3,
    pub local_scale: Vector3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMesh {
    pub primitive: Option<String>,
    pub color: Option<String>,
    pub label: Option<String>,
    pub asset_ref: Option<RuntimeAssetRef>,
    pub material_ref: Option<RuntimeAssetRef>,
    pub texture_ref: Option<RuntimeAssetRef>,
    pub visible: bool,
    pub layer: String,
    pub metalness: Option<f32>,
    pub roughness: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSpriteRenderer2D {
    pub sprite_ref: Option<RuntimeAssetRef>,
    pub material_ref: Option<RuntimeAssetRef>,
    pub color: Option<[f32; 4]>,
    pub flip_x: Option<bool>,
    pub flip_y: Option<bool>,
    pub sorting_layer: Option<i16>,
    pub order_in_layer: Option<i32>,
    pub sort_z: Option<f32>,
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAssetRef {
    pub id: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    #[serde(default)]
    pub guid: Option<String>,
    #[serde(default)]
    pub sub_asset: Option<String>,
}

fn validate_scene(
    scene: &RuntimeScene,
    assets: &RuntimeAssetManifest,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if scene.schema_version != RUNTIME_SCENE_SCHEMA_VERSION {
        diagnostics.error(
            "scene.schemaVersion",
            format!(
                "scene schemaVersion must be {}",
                RUNTIME_SCENE_SCHEMA_VERSION
            ),
        );
    }

    let mut asset_ids: HashSet<&str> = assets
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect();
    asset_ids.extend(
        assets
            .runtime_asset_index
            .iter()
            .map(|asset| asset.asset_id.as_str()),
    );
    for (index, entity) in scene.entities.iter().enumerate() {
        let entity_path = format!("scene.entities[{}]", index);
        if entity.schema_version != RUNTIME_ENTITY_SCHEMA_VERSION {
            diagnostics.error(
                format!("{}.schemaVersion", entity_path),
                format!(
                    "entity schemaVersion must be {}",
                    RUNTIME_ENTITY_SCHEMA_VERSION
                ),
            );
        }
        if entity.transform.is_none() {
            diagnostics.error(
                format!("{}.transform", entity_path),
                "transform is required",
            );
        }
        if let Some(mesh) = &entity.mesh {
            validate_mesh(mesh, &entity_path, &asset_ids, diagnostics);
        }
        if let Some(sprite) = &entity.sprite_renderer2d {
            validate_sprite_renderer2d(sprite, &entity_path, &asset_ids, diagnostics);
        }
    }
}

fn validate_mesh(
    mesh: &RuntimeMesh,
    entity_path: &str,
    asset_ids: &HashSet<&str>,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if mesh.primitive.as_deref() == Some("model") && mesh.asset_ref.is_none() {
        diagnostics.error(
            format!("{}.mesh.assetRef", entity_path),
            "model mesh assetRef is required",
        );
    }
    for (field, asset_ref) in [
        ("assetRef", mesh.asset_ref.as_ref()),
        ("materialRef", mesh.material_ref.as_ref()),
        ("textureRef", mesh.texture_ref.as_ref()),
    ] {
        if let Some(asset_ref) = asset_ref {
            if asset_ref.id.is_empty() {
                diagnostics.error(
                    format!("{}.mesh.{}.id", entity_path, field),
                    format!("{}.id is required", field),
                );
            } else if !asset_ids.contains(asset_ref.id.as_str()) {
                diagnostics.error(
                    format!("{}.mesh.{}", entity_path, field),
                    format!("{} points to missing asset: {}", field, asset_ref.id),
                );
            }
        }
    }
}

fn validate_sprite_renderer2d(
    sprite: &RuntimeSpriteRenderer2D,
    entity_path: &str,
    asset_ids: &HashSet<&str>,
    diagnostics: &mut RuntimeDiagnostics,
) {
    for (field, asset_ref) in [
        ("spriteRef", sprite.sprite_ref.as_ref()),
        ("materialRef", sprite.material_ref.as_ref()),
    ] {
        if let Some(asset_ref) = asset_ref {
            if asset_ref.id.is_empty() {
                diagnostics.error(
                    format!("{}.spriteRenderer2D.{}.id", entity_path, field),
                    format!("{}.id is required", field),
                );
            } else if !asset_ids.contains(asset_ref.id.as_str()) {
                diagnostics.error(
                    format!("{}.spriteRenderer2D.{}", entity_path, field),
                    format!("{} points to missing asset: {}", field, asset_ref.id),
                );
            }
        }
    }
}

pub fn read_scene(bytes: &[u8]) -> RuntimeDiagnostics {
    let mut diagnostics = RuntimeDiagnostics::new();
    let scene: RuntimeScene = match serde_json::from_slice(bytes) {
        Ok(scene) => scene,
        Err(error) => {
            diagnostics.error("scene", error.to_string());
            return diagnostics;
        }
    };
    let assets = serde_json::from_str::<RuntimeAssetManifest>(
        r#"{"schemaVersion":"runtime-asset-manifest.v1","assets":[]}"#,
    )
    .unwrap();
    validate_scene(&scene, &assets, &mut diagnostics);
    diagnostics
}
