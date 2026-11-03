//! Authoring contracts only. Import/cook and runtime execution are gated until A1/A2.
use engine_runtime::render_contract::{
    MaterialBlend, MaterialCull, MaterialDepth, MaterialPreset, RenderAssetKind,
};
use engine_runtime::runtime_package::{
    RuntimeEntity, RuntimeMesh, RuntimeProjectComponent, RuntimeTransform,
};
use engine_runtime::spatial_contract::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

mod check;
pub(crate) use check::validate_source;

pub const AUTHORING_SCENE_V2: &str = "authoring-scene.v2";
pub const AUTHORING_PREFAB_V2: &str = "authoring-prefab-asset.v2";

/// Existing source-side Prefab instance shape, also consumed in spatial scenes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialPrefabInstance {
    pub source: SpatialAssetRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub overrides: Vec<SpatialPrefabOverride>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialPrefabOverride {
    pub target_source_entity_id: String,
    pub component_type: String,
    pub field_path: String,
    pub value: Value,
}
impl SpatialPrefabInstance {
    pub fn validate(&self) -> ContractResult<()> {
        if self.source.0.asset_type != "prefab"
            || self.source.0.id.trim().is_empty()
            || self.overrides.len() > 4096
        {
            return Err(SpatialContractError::new(
                "prefab_instance_invalid",
                "/source",
                "Expected a Prefab AssetRef and at most 4096 explicit overrides.",
            ));
        }
        let mut targets = BTreeSet::new();
        for (i, value) in self.overrides.iter().enumerate() {
            if value.target_source_entity_id.is_empty()
                || value.component_type.is_empty()
                || value.field_path.is_empty()
                || !targets.insert((
                    &value.target_source_entity_id,
                    &value.component_type,
                    &value.field_path,
                ))
            {
                return Err(SpatialContractError::new(
                    "prefab_override_invalid",
                    &format!("/overrides/{i}"),
                    "Overrides require a stable target, component and unique field path.",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialSourceEntity {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub parent_id: Option<String>,
    pub sibling_order: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<RuntimeTransform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<RuntimeMesh>,
    pub components: Vec<ProjectDataComponent>,
}
impl SpatialSourceEntity {
    pub fn canonicalize(&self) -> ContractResult<RuntimeEntityDocument> {
        let mut transform = None;
        let mut visibility = None;
        let mut mesh_renderer = None;
        let mut camera = None;
        let mut light = None;
        let mut project = Vec::new();
        let mut legacy = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, c) in self.components.iter().enumerate() {
            let path = format!("/components/{i}/data");
            if !seen.insert(&c.component_type) {
                return Err(SpatialContractError::new(
                    "component_duplicate",
                    &format!("/components/{i}/componentType"),
                    "An entity cannot contain duplicate component types.",
                ));
            }
            if matches!(
                c.component_type.as_str(),
                "engine.model_lod3d" | "engine.hlod3d" | "engine.vegetation3d"
            ) {
                engine_runtime::model_lod3d::validate_presentation_component(
                    &c.component_type,
                    c.data.clone(),
                )?;
                project.push(c.clone());
                continue;
            }
            match c.component_type.as_str() {
                "engine.audio_source" => {
                    engine_runtime::audio::decode_audio_source(&c.data)
                        .map_err(|e| SpatialContractError::new("audio_invalid", &path, e))?;
                    project.push(c.clone());
                }
                "engine.spatial_audio3d" => {
                    let value: engine_runtime::runtime_audio::SpatialAudioSource3D =
                        decode(c.data.clone(), &path)?;
                    value
                        .validate()
                        .map_err(|e| SpatialContractError::new("audio_invalid", &path, e))?;
                    project.push(c.clone());
                }
                "engine.audio_listener3d" => {
                    let value: engine_runtime::runtime_audio::SpatialAudioListener3D =
                        decode(c.data.clone(), &path)?;
                    value
                        .validate()
                        .map_err(|e| SpatialContractError::new("audio_invalid", &path, e))?;
                    project.push(c.clone());
                }
                "engine.world_persistence3d" => {
                    let value: engine_runtime::world_persistence::WorldPersistence3D =
                        decode(c.data.clone(), &path)?;
                    engine_runtime::world_persistence::validate_binding(&value)?;
                    project.push(c.clone());
                }
                "engine.streaming_guard3d" => {
                    let value: engine_runtime::world_streaming::StreamingGuard3D =
                        decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.world_streaming3d" => {
                    let value: engine_runtime::world_streaming::WorldStreaming3D =
                        decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.navigation3d" => {
                    let value: engine_runtime::navigation_3d::NavigationSurface3D =
                        decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.navigation_agent3d" => {
                    let value: engine_runtime::navigation_3d::NavigationAgent3D =
                        decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.navigation_obstacle3d" => {
                    let value: engine_runtime::navigation_3d::NavigationObstacle3D =
                        decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.body3d" => {
                    let value: engine_runtime::physics_3d::Body3D = decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.joint3d" => {
                    let value: engine_runtime::physics_3d::Joint3D = decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.motor3d" => {
                    let value: engine_runtime::physics_3d::CharacterMotor3D =
                        decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.character3d" => {
                    let value: engine_runtime::character_animation::CharacterBinding3D =
                        decode(c.data.clone(), &path)?;
                    value.validate()?;
                    project.push(c.clone());
                }
                "engine.prefab_instance" => {
                    return Err(SpatialContractError::new("prefab_expansion_required", &path, "Source instances require the Compiler's Prefab assembly before runtime canonicalization."));
                }
                "Transform3D" => {
                    transform = Some(
                        decode::<Transform3D>(c.data.clone(), &path)?
                            .canonicalize()
                            .map_err(|e| e.at(&path))?,
                    )
                }
                "Visibility3D" => {
                    let v: Visibility3D = decode(c.data.clone(), &path)?;
                    version(&v.schema_version, "visibility3d.v1").map_err(|e| e.at(&path))?;
                    visibility = Some(v);
                }
                "MeshRenderer3D" => {
                    let v: MeshRenderer3D = decode(c.data.clone(), &path)?;
                    v.validate().map_err(|e| e.at(&path))?;
                    mesh_renderer = Some(v);
                }
                "Light3D" => {
                    let v: Light3D = decode(c.data.clone(), &path)?;
                    v.validate().map_err(|e| e.at(&path))?;
                    light = Some(v);
                }
                "Camera3D" => {
                    let v: Camera3D = decode(c.data.clone(), &path)?;
                    v.validate().map_err(|e| e.at(&path))?;
                    camera = Some(v);
                }
                "ParticleEffect" | "engine.particle_effect" => {
                    engine_runtime::runtime_particles::decode_particle_effect(&c.data)
                        .map_err(|e| SpatialContractError::new("particle_invalid", &path, e))?;
                    if project.iter().any(|c: &ProjectDataComponent| {
                        c.component_type == "engine.particle_effect"
                    }) {
                        return Err(SpatialContractError::new(
                            "component_duplicate",
                            &path,
                            "Keep one ParticleEffect.",
                        ));
                    }
                    project.push(ProjectDataComponent {
                        component_type: "engine.particle_effect".into(),
                        data: c.data.clone(),
                    });
                }
                "SpriteRenderer2D" | "Collider2D" | "Animator2D" => legacy.push(c.clone()),
                kind if kind.starts_with("project.") && kind != "project." => {
                    project.push(c.clone())
                }
                _ => {
                    return Err(SpatialContractError::new(
                        "component_unknown",
                        &format!("/components/{i}/componentType"),
                        "Unknown built-in component. Project-owned data uses project.* names.",
                    ))
                }
            }
        }
        if let Some(transform) = transform {
            if self.transform.is_some() || self.mesh.is_some() || !legacy.is_empty() {
                return Err(SpatialContractError::new(
                    "domain_conflict",
                    "/transform",
                    "Transform3D cannot coexist with legacy transform/mesh or 2D components.",
                ));
            }
            Ok(RuntimeEntityDocument::Spatial(RuntimeEntityV2 {
                schema_version: RUNTIME_ENTITY_V2.into(),
                id: self.id.clone(),
                name: self.name.clone(),
                enabled: self.enabled,
                parent_id: self.parent_id.clone(),
                sibling_order: self.sibling_order,
                spatial: RuntimeSpatial::World3D {
                    transform,
                    visibility,
                    mesh_renderer,
                    camera,
                    light,
                },
                components: project,
            }))
        } else {
            if visibility.is_some()
                || mesh_renderer.is_some()
                || camera.is_some()
                || light.is_some()
                || self.transform.is_none()
            {
                return Err(SpatialContractError::new("transform_missing", "/components", "Each spatial entity needs exactly one transform; 3D components require Transform3D."));
            }
            legacy.extend(project);
            Ok(RuntimeEntityDocument::Legacy(RuntimeEntity {
                spatial: None,
                schema_version: "runtime-entity.v1".into(),
                id: self.id.clone(),
                name: self.name.clone(),
                kind: "entity".into(),
                enabled: self.enabled,
                parent_id: self.parent_id.clone(),
                sibling_order: self.sibling_order,
                transform: self.transform.clone(),
                mesh: self.mesh.clone(),
                sprite_renderer2d: None,
                animator2d: None,
                components: legacy
                    .into_iter()
                    .map(|c| RuntimeProjectComponent {
                        component_type: c.component_type,
                        data: c.data,
                    })
                    .collect(),
            }))
        }
    }
}
fn canonical_entities(
    entities: &[SpatialSourceEntity],
) -> ContractResult<Vec<RuntimeEntityDocument>> {
    entities
        .iter()
        .enumerate()
        .map(|(i, e)| {
            e.canonicalize()
                .map_err(|e| e.at(&format!("/entities/{i}")))
        })
        .collect()
}
fn source_field(
    mut error: SpatialContractError,
    entities: &[SpatialSourceEntity],
) -> SpatialContractError {
    // Tree validation consumes runtime DTOs; translate its typed-field paths back to source.
    for (i, entity) in entities.iter().enumerate() {
        for (runtime_field, component) in [
            ("transform", "Transform3D"),
            ("visibility", "Visibility3D"),
            ("meshRenderer", "MeshRenderer3D"),
            ("camera", "Camera3D"),
            ("light", "Light3D"),
        ] {
            let prefix = format!("/entities/{i}/spatial/{runtime_field}");
            if let Some(suffix) = error.field_path.strip_prefix(&prefix) {
                if let Some(j) = entity
                    .components
                    .iter()
                    .position(|c| c.component_type == component)
                {
                    error.field_path = format!("/entities/{i}/components/{j}/data{suffix}");
                    return error;
                }
            }
        }
    }
    error
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialSourceScene {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub gravity: f32,
    pub background: String,
    pub sky_color: String,
    pub main_camera: Option<String>,
    pub entities: Vec<SpatialSourceEntity>,
}
impl SpatialSourceScene {
    /// Validate placement roots for the existing assembler; public canonicalization
    /// still rejects unexpanded source instances instead of silently dropping them.
    pub(crate) fn canonicalize_placements(&self) -> ContractResult<RuntimeSceneV2> {
        let mut placements = self.clone();
        for (i, entity) in placements.entities.iter_mut().enumerate() {
            let instances: Vec<_> = entity
                .components
                .iter()
                .enumerate()
                .filter(|(_, c)| c.component_type == "engine.prefab_instance")
                .collect();
            if instances.is_empty() {
                continue;
            }
            let path = format!("/entities/{i}/components");
            if instances.len() != 1 {
                return Err(SpatialContractError::new(
                    "component_duplicate",
                    &path,
                    "Only one Prefab instance is allowed per placement root.",
                ));
            }
            if !entity
                .components
                .iter()
                .any(|c| c.component_type == "Transform3D")
                || entity.components.iter().any(|c| {
                    ["MeshRenderer3D", "Camera3D", "Light3D"].contains(&c.component_type.as_str())
                })
            {
                return Err(SpatialContractError::new("prefab_instance_invalid",&path,"Use Transform3D for instance placement and explicit overrides for prefab fields."));
            }
            let (j, component) = instances[0];
            let path = format!("{path}/{j}/data");
            let instance: SpatialPrefabInstance = decode(component.data.clone(), &path)?;
            instance.validate().map_err(|e| e.at(&path))?;
            entity
                .components
                .retain(|c| c.component_type != "engine.prefab_instance");
        }
        placements.canonicalize()
    }
    pub fn canonicalize(&self) -> ContractResult<RuntimeSceneV2> {
        version(&self.schema_version, AUTHORING_SCENE_V2)?;
        let mut scene = RuntimeSceneV2 {
            schema_version: RUNTIME_SCENE_V2.into(),
            id: self.id.clone(),
            name: self.name.clone(),
            gravity: self.gravity,
            background: self.background.clone(),
            sky_color: self.sky_color.clone(),
            main_camera: self.main_camera.clone(),
            entities: canonical_entities(&self.entities)?,
        };
        scene
            .validate_and_canonicalize()
            .map_err(|e| source_field(e, &self.entities))?;
        Ok(scene)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialSourcePrefab {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub root_entity_id: String,
    pub entities: Vec<SpatialSourceEntity>,
}
impl SpatialSourcePrefab {
    pub fn canonicalize(&self) -> ContractResult<RuntimePrefabV2> {
        version(&self.schema_version, AUTHORING_PREFAB_V2)?;
        if self.entities.iter().any(|e| {
            e.components
                .iter()
                .any(|c| c.component_type == "engine.prefab_instance")
        }) {
            return Err(SpatialContractError::new(
                "prefab_nested_unsupported",
                "/entities",
                "Nested spatial prefab instances are not supported.",
            ));
        }
        let mut prefab = RuntimePrefabV2 {
            schema_version: RUNTIME_PREFAB_V2.into(),
            id: self.id.clone(),
            name: self.name.clone(),
            root_entity_id: self.root_entity_id.clone(),
            entities: canonical_entities(&self.entities)?,
        };
        prefab
            .validate_and_canonicalize()
            .map_err(|e| source_field(e, &self.entities))?;
        Ok(prefab)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "primitive", rename_all = "camelCase", deny_unknown_fields)]
pub enum GeneratedPrimitive {
    Cube { size: [f32; 3] },
    Plane { size: [f32; 2] },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum MeshSource {
    Generated { geometry: GeneratedPrimitive },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshAssetV2 {
    pub schema_version: String,
    pub asset_id: String,
    pub asset_guid: String,
    pub display_name: String,
    pub source: MeshSource,
}
impl MeshAssetV2 {
    pub fn validate(&self) -> ContractResult<()> {
        version(&self.schema_version, "mesh-asset.v2")?;
        asset_identity(&self.asset_id, &self.asset_guid)?;
        let MeshSource::Generated { geometry } = &self.source;
        let size: &[f32] = match geometry {
            GeneratedPrimitive::Cube { size } => size,
            GeneratedPrimitive::Plane { size } => size,
        };
        finite(size, "/source/geometry/size")?;
        if size.iter().any(|v| *v <= 0.0 || *v * 0.5 == 0.0) {
            return Err(SpatialContractError::new(
                "primitive_size_invalid",
                "/source/geometry/size",
                "Full edge lengths must be positive and representable.",
            ));
        }
        Ok(())
    }
    pub fn material_slots(&self) -> Vec<String> {
        vec!["surface".into()]
    }
}
fn asset_identity(id: &str, guid: &str) -> ContractResult<()> {
    if id.trim().is_empty() || guid.trim().is_empty() {
        return Err(SpatialContractError::new(
            "asset_identity_missing",
            "/assetGuid",
            "assetId and persistent assetGuid are required.",
        ));
    }
    Ok(())
}
pub use engine_runtime::runtime_texture::{TextureFilter, TextureWrap};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BaseColorTexture {
    pub texture: SpatialAssetRef,
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub wrap_s: TextureWrap,
    pub wrap_t: TextureWrap,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialAssetV2 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decal_bias: Option<i32>,
    pub schema_version: String,
    pub asset_id: String,
    pub asset_guid: String,
    pub display_name: String,
    pub preset: MaterialPreset,
    pub base_color: [f32; 4],
    /// UnlitFlat: preserve material color and ignore a mesh COLOR_0 attribute.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignore_vertex_color: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<BaseColorTexture>,
    pub blend: MaterialBlend,
    pub cull: MaterialCull,
    pub depth: MaterialDepth,
}
impl MaterialAssetV2 {
    pub fn validate(&self) -> ContractResult<()> {
        version(&self.schema_version, "material-asset.v2")?;
        if self.decal_bias.is_some_and(|v| {
            !(-16..=-1).contains(&v)
                || self.blend != MaterialBlend::AlphaBlend
                || self.depth != MaterialDepth::TestOnly
        }) {
            return Err(SpatialContractError::new(
                "decal_invalid",
                "/decalBias",
                "Mesh decal requires bias -16..-1, alphaBlend and testOnly.",
            ));
        }
        if self.preset != MaterialPreset::Unlit {
            return Err(SpatialContractError::new("material_model_unsupported", "/preset", "material-asset.v2 is Unlit; StandardLit uses the model import strategy or native cooked contract."));
        }
        asset_identity(&self.asset_id, &self.asset_guid)?;
        finite(&self.base_color, "/baseColor")?;
        if self.depth == MaterialDepth::Disabled {
            return Err(SpatialContractError::new(
                "depth_invalid",
                "/depth",
                "World3D materials require testAndWrite or testOnly.",
            ));
        }
        if let Some(texture) = &self.base_color_texture {
            texture
                .texture
                .validate(RenderAssetKind::Texture, "/baseColorTexture/texture")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImportMaterialStrategy {
    #[default]
    StrictUnlit,
    BaseColorOnly,
    StandardLit,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportUpAxis {
    Y,
    Z,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportHandedness {
    Right,
    Left,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportCoordinates {
    pub up_axis: ImportUpAxis,
    pub handedness: ImportHandedness,
    pub meters_per_unit: f32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportBudgets {
    pub source_bytes: u64,
    pub image_pixels: u64,
    pub decoded_image_bytes: u64,
    pub vertices: u64,
    pub indices: u64,
    pub nodes: u64,
    pub depth: u64,
    pub cooked_bytes: u64,
}
impl Default for ImportBudgets {
    fn default() -> Self {
        Self {
            source_bytes: 256 * 1024 * 1024,
            image_pixels: 16_777_216,
            decoded_image_bytes: 256 * 1024 * 1024,
            vertices: 2_000_000,
            indices: 6_000_000,
            nodes: 10_000,
            depth: 128,
            cooked_bytes: 512 * 1024 * 1024,
        }
    }
}
impl ImportBudgets {
    pub fn validate(&self) -> ContractResult<()> {
        let current = serde_json::to_value(self).expect("integer budgets");
        let maximum = serde_json::to_value(Self::default()).expect("integer budgets");
        for (key, value) in current.as_object().unwrap() {
            let n = value.as_u64().unwrap();
            if n == 0 || n > maximum[key].as_u64().unwrap() {
                return Err(SpatialContractError::new(
                    "budget_invalid",
                    &format!("/budgets/{key}"),
                    "Budgets may only tighten the qualified v1 profile and must be positive.",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImportOutputKind {
    Character,
    Mesh,
    Material,
    Texture,
    Prefab,
}
impl ImportOutputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character3d",
            Self::Mesh => "mesh",
            Self::Material => "material",
            Self::Texture => "texture",
            Self::Prefab => "prefab",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportMaterialSlot {
    pub key: String,
    pub name: String,
    pub source_locator: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportOutput {
    pub key: String,
    pub asset_id: String,
    pub asset_guid: String,
    pub kind: ImportOutputKind,
    pub source_locator: String,
    pub source_fingerprint: String,
    pub material_slots: Vec<ImportMaterialSlot>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportNodeMapping {
    pub key: String,
    pub entity_id: String,
    pub source_locator: String,
    pub source_fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelImportV1 {
    /// Explicit offline target encoding. Omission preserves the RGBA8 contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_compression: Option<String>,
    pub schema_version: String,
    pub asset_id: String,
    pub asset_guid: String,
    pub display_name: String,
    pub source: String,
    pub scene: u32,
    pub coordinates: ImportCoordinates,
    #[serde(default)]
    pub material_strategy: ImportMaterialStrategy,
    #[serde(default)]
    pub budgets: ImportBudgets,
    pub outputs: Vec<ImportOutput>,
    pub nodes: Vec<ImportNodeMapping>,
}
impl ModelImportV1 {
    pub fn validate(&self) -> ContractResult<()> {
        version(&self.schema_version, "model-import.v1")?;
        if self
            .texture_compression
            .as_deref()
            .is_some_and(|v| v != "bc3" && v != "none")
        {
            return Err(SpatialContractError::new(
                "texture_compression_unsupported",
                "/textureCompression",
                "Windows supports explicit bc3 or none.",
            ));
        }
        asset_identity(&self.asset_id, &self.asset_guid)?;
        if crate::ProjectRelativePath::parse(&self.source).is_err()
            || ![".gltf", ".glb"]
                .iter()
                .any(|s| self.source.to_ascii_lowercase().ends_with(s))
        {
            return Err(SpatialContractError::new(
                "model_source_invalid",
                "/source",
                "Expected a project-relative glTF/GLB source.",
            ));
        }
        if !self.coordinates.meters_per_unit.is_finite()
            || self.coordinates.meters_per_unit <= 0.0
            || !(1.0 / self.coordinates.meters_per_unit).is_finite()
        {
            return Err(SpatialContractError::new(
                "unit_scale_invalid",
                "/coordinates/metersPerUnit",
                "Unit conversion must be finite, positive and invertible.",
            ));
        }
        self.budgets.validate()?;
        names(
            &self
                .outputs
                .iter()
                .map(|o| o.key.clone())
                .collect::<Vec<_>>(),
            "/outputs/key",
        )?;
        names(
            &self
                .outputs
                .iter()
                .map(|o| o.asset_id.clone())
                .collect::<Vec<_>>(),
            "/outputs/assetId",
        )?;
        names(
            &self
                .outputs
                .iter()
                .map(|o| o.asset_guid.clone())
                .collect::<Vec<_>>(),
            "/outputs/assetGuid",
        )?;
        for (i, o) in self.outputs.iter().enumerate() {
            let path = format!("/outputs/{i}");
            asset_identity(&o.asset_id, &o.asset_guid).map_err(|e| e.at(&path))?;
            if o.source_locator.trim().is_empty()
                || !engine_runtime::render_contract::is_sha256_digest(&o.source_fingerprint)
            {
                return Err(SpatialContractError::new("mapping_invalid", &path, "Persistent output key/GUID requires locator and SHA-256 fingerprint matching evidence."));
            }
            if o.kind != ImportOutputKind::Mesh && !o.material_slots.is_empty() {
                return Err(SpatialContractError::new(
                    "mapping_invalid",
                    &path,
                    "Only mesh outputs declare material slots.",
                ));
            }
            names(
                &o.material_slots
                    .iter()
                    .map(|s| s.name.clone())
                    .collect::<Vec<_>>(),
                &format!("{path}/materialSlots/name"),
            )?;
            names(
                &o.material_slots
                    .iter()
                    .map(|s| s.key.clone())
                    .collect::<Vec<_>>(),
                &format!("{path}/materialSlots/key"),
            )?;
            if o.material_slots
                .iter()
                .any(|s| s.source_locator.trim().is_empty())
            {
                return Err(SpatialContractError::new(
                    "mapping_invalid",
                    &path,
                    "Material slot locator is required.",
                ));
            }
        }
        names(
            &self.nodes.iter().map(|n| n.key.clone()).collect::<Vec<_>>(),
            "/nodes/key",
        )?;
        names(
            &self
                .nodes
                .iter()
                .map(|n| n.entity_id.clone())
                .collect::<Vec<_>>(),
            "/nodes/entityId",
        )?;
        if self.nodes.iter().any(|n| {
            n.source_locator.trim().is_empty()
                || !engine_runtime::render_contract::is_sha256_digest(&n.source_fingerprint)
        }) {
            return Err(SpatialContractError::new(
                "mapping_invalid",
                "/nodes",
                "Node mapping requires locator and SHA-256 fingerprint evidence.",
            ));
        }
        Ok(())
    }
}

mod inspection;
pub use inspection::*;
