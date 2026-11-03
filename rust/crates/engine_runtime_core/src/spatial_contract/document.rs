use super::*;

pub const RUNTIME_SCENE_V2: &str = "runtime-scene.v2";
pub const RUNTIME_PREFAB_V2: &str = "runtime-prefab.v2";
pub const RUNTIME_ENTITY_V2: &str = "runtime-entity.v2";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "domain", deny_unknown_fields)]
pub enum RuntimeSpatial {
    #[serde(rename = "world3d")]
    World3D {
        transform: CanonicalTransform3D,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visibility: Option<Visibility3D>,
        #[serde(
            default,
            rename = "meshRenderer",
            skip_serializing_if = "Option::is_none"
        )]
        mesh_renderer: Option<MeshRenderer3D>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        camera: Option<Camera3D>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        light: Option<Light3D>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeEntityV2 {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub parent_id: Option<String>,
    pub sibling_order: i32,
    pub spatial: RuntimeSpatial,
    /// Non-spatial data stays in the existing component channel; builtins are individually validated.
    #[serde(default)]
    pub components: Vec<ProjectDataComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDataComponent {
    pub component_type: String,
    pub data: Value,
}

impl RuntimeEntityV2 {
    pub fn validate_and_canonicalize(&mut self) -> ContractResult<()> {
        version(&self.schema_version, RUNTIME_ENTITY_V2)?;
        let RuntimeSpatial::World3D {
            transform,
            visibility,
            mesh_renderer,
            camera,
            light,
        } = &mut self.spatial;
        transform
            .validate()
            .map_err(|e| e.at("/spatial/transform"))?;
        if let Some(v) = visibility {
            version(&v.schema_version, "visibility3d.v1")
                .map_err(|e| e.at("/spatial/visibility"))?;
        }
        if let Some(v) = mesh_renderer {
            v.validate().map_err(|e| e.at("/spatial/meshRenderer"))?;
        }
        if let Some(v) = camera {
            v.validate().map_err(|e| e.at("/spatial/camera"))?;
        }
        if let Some(v) = light {
            v.validate().map_err(|e| e.at("/spatial/light"))?;
        }
        names(
            &self
                .components
                .iter()
                .map(|c| c.component_type.clone())
                .collect::<Vec<_>>(),
            "/components",
        )?;
        for (i, c) in self.components.iter().enumerate() {
            if let Some(result) =
                crate::model_lod3d::validate_component(&c.component_type, c.data.clone())
            {
                result?;
                continue;
            }
            if c.component_type == "engine.audio_source" {
                crate::audio::decode_audio_source(&c.data).map_err(|e| {
                    SpatialContractError::new("audio_invalid", &format!("/components/{i}/data"), e)
                })?;
                continue;
            }
            if c.component_type == "engine.spatial_audio3d" {
                let value: crate::runtime_audio::SpatialAudioSource3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate().map_err(|e| {
                    SpatialContractError::new("audio_invalid", &format!("/components/{i}/data"), e)
                })?;
                continue;
            }
            if c.component_type == "engine.audio_listener3d" {
                let value: crate::runtime_audio::SpatialAudioListener3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate().map_err(|e| {
                    SpatialContractError::new("audio_invalid", &format!("/components/{i}/data"), e)
                })?;
                continue;
            }
            if c.component_type == "engine.streaming_guard3d" {
                let value: crate::world_streaming::StreamingGuard3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.world_streaming3d" {
                let value: crate::world_streaming::WorldStreaming3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.world_persistence3d" {
                let value: crate::world_persistence::WorldPersistence3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                crate::world_persistence::validate_binding(&value)?;
                continue;
            }
            if c.component_type == "engine.navigation3d" {
                let value: crate::navigation_3d::NavigationSurface3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.navigation_agent3d" {
                let value: crate::navigation_3d::NavigationAgent3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.navigation_obstacle3d" {
                let value: crate::navigation_3d::NavigationObstacle3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.body3d" {
                let value: crate::physics_3d::Body3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.joint3d" {
                let value: crate::physics_3d::Joint3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.motor3d" {
                let value: crate::physics_3d::CharacterMotor3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                value.validate()?;
                continue;
            }
            if c.component_type == "engine.character3d" {
                let binding: crate::character_animation::CharacterBinding3D =
                    decode(c.data.clone(), &format!("/components/{i}/data"))?;
                binding
                    .validate()
                    .map_err(|e| e.at(&format!("/components/{i}/data")))?;
                continue;
            }
            if c.component_type == "engine.particle_effect" {
                crate::runtime_particles::decode_particle_effect(&c.data).map_err(|e| {
                    SpatialContractError::new(
                        "particle_invalid",
                        &format!("/components/{i}/data"),
                        e,
                    )
                })?;
                continue;
            }
            if !c.component_type.starts_with("project.") || c.component_type == "project." {
                return Err(SpatialContractError::new(
                    "component_unknown",
                    &format!("/components/{i}/componentType"),
                    "Only project.* or validated engine.particle_effect data may use this channel; spatial fields are typed.",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum RuntimeEntityDocument {
    Legacy(RuntimeEntity),
    Spatial(RuntimeEntityV2),
}
impl<'de> Deserialize<'de> for RuntimeEntityDocument {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        match value.get("schemaVersion").and_then(Value::as_str) {
            Some(RUNTIME_ENTITY_V2) => decode(value, "").map(Self::Spatial),
            Some("runtime-entity.v1") => reject_legacy_entity_extension(&value)
                .and_then(|()| decode(value, ""))
                .map(Self::Legacy),
            _ => Err(SpatialContractError::new(
                "schema_unsupported",
                "/schemaVersion",
                "Expected runtime-entity.v1 or v2.",
            )),
        }
        .map_err(serde::de::Error::custom)
    }
}
impl RuntimeEntityDocument {
    pub fn id(&self) -> &str {
        match self {
            Self::Legacy(v) => &v.id,
            Self::Spatial(v) => &v.id,
        }
    }
    pub fn parent_id(&self) -> Option<&str> {
        match self {
            Self::Legacy(v) => v.parent_id.as_deref(),
            Self::Spatial(v) => v.parent_id.as_deref(),
        }
    }
    pub fn enabled(&self) -> bool {
        match self {
            Self::Legacy(v) => v.enabled,
            Self::Spatial(v) => v.enabled,
        }
    }
    pub fn is_3d(&self) -> bool {
        matches!(self, Self::Spatial(_))
    }
    pub fn spatial(&self) -> Option<&RuntimeSpatial> {
        match self {
            Self::Spatial(v) => Some(&v.spatial),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSceneV2 {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub gravity: f32,
    pub background: String,
    pub sky_color: String,
    pub main_camera: Option<String>,
    pub entities: Vec<RuntimeEntityDocument>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimePrefabV2 {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub root_entity_id: String,
    pub entities: Vec<RuntimeEntityDocument>,
}

impl RuntimeSceneV2 {
    pub fn validate_and_canonicalize(&mut self) -> ContractResult<()> {
        version(&self.schema_version, RUNTIME_SCENE_V2)?;
        finite(&[self.gravity], "/gravity")?;
        names(std::slice::from_ref(&self.id), "/id")?;
        validate_entity_tree(&mut self.entities)?;
        let has_mesh = self.entities.iter().any(|e| {
            matches!(
                e.spatial(),
                Some(RuntimeSpatial::World3D {
                    mesh_renderer: Some(_),
                    ..
                })
            ) || matches!(e, RuntimeEntityDocument::Spatial(v) if v.components.iter().any(|c| c.component_type == "engine.particle_effect"))
        });
        if has_mesh && self.main_camera.is_none() {
            return Err(SpatialContractError::new(
                "main_camera_missing",
                "/mainCamera",
                "A scene containing 3D renderers must explicitly select its camera.",
            ));
        }
        if let Some(id) = &self.main_camera {
            let entity = self.entities.iter().find(|e| e.id() == id);
            if !matches!(
                entity.and_then(RuntimeEntityDocument::spatial),
                Some(RuntimeSpatial::World3D {
                    camera: Some(Camera3D { enabled: true, .. }),
                    ..
                })
            ) {
                return Err(SpatialContractError::new(
                    "main_camera_invalid",
                    "/mainCamera",
                    "Selected identity must resolve to an enabled Camera3D.",
                ));
            }
            let by_id: BTreeMap<_, _> = self.entities.iter().map(|e| (e.id(), e)).collect();
            let mut current = entity;
            while let Some(e) = current {
                if !e.enabled() {
                    return Err(SpatialContractError::new(
                        "main_camera_disabled",
                        "/mainCamera",
                        "Selected camera or its ancestor is disabled.",
                    ));
                }
                current = e.parent_id().and_then(|id| by_id.get(id).copied());
            }
        }
        Ok(())
    }
}
impl RuntimePrefabV2 {
    pub fn validate_and_canonicalize(&mut self) -> ContractResult<()> {
        version(&self.schema_version, RUNTIME_PREFAB_V2)?;
        names(std::slice::from_ref(&self.id), "/id")?;
        validate_entity_tree(&mut self.entities)?;
        let roots: Vec<_> = self
            .entities
            .iter()
            .filter(|e| e.parent_id().is_none())
            .map(RuntimeEntityDocument::id)
            .collect();
        if roots != [self.root_entity_id.as_str()] {
            return Err(SpatialContractError::new(
                "prefab_root_invalid",
                "/rootEntityId",
                "Prefab must have exactly one root matching rootEntityId.",
            ));
        }
        Ok(())
    }
}

pub fn validate_entity_tree(entities: &mut [RuntimeEntityDocument]) -> ContractResult<()> {
    for (i, e) in entities.iter_mut().enumerate() {
        let path = format!("/entities/{i}");
        match e {
            RuntimeEntityDocument::Spatial(v) => {
                v.validate_and_canonicalize().map_err(|e| e.at(&path))?
            }
            RuntimeEntityDocument::Legacy(v) => {
                version(&v.schema_version, "runtime-entity.v1").map_err(|e| e.at(&path))?;
                reject_legacy_entity_extension(
                    &serde_json::to_value(&*v).expect("serializable entity"),
                )
                .map_err(|e| e.at(&path))?;
                if v.transform.is_none() {
                    return Err(SpatialContractError::new(
                        "transform_missing",
                        &format!("{path}/transform"),
                        "Legacy spatial entity requires transform.",
                    ));
                }
            }
        }
    }
    let mut by_id = BTreeMap::new();
    for (i, e) in entities.iter().enumerate() {
        if e.id().trim().is_empty() || by_id.insert(e.id(), i).is_some() {
            return Err(SpatialContractError::new(
                "entity_id_invalid",
                &format!("/entities/{i}/id"),
                "Entity identities must be nonempty and unique.",
            ));
        }
    }
    for (i, e) in entities.iter().enumerate() {
        if let Some(parent) = e.parent_id() {
            let p = by_id.get(parent).ok_or_else(|| {
                SpatialContractError::new(
                    "parent_missing",
                    &format!("/entities/{i}/parentId"),
                    "Parent identity does not exist.",
                )
            })?;
            if entities[*p].is_3d() != e.is_3d() {
                return Err(SpatialContractError::new(
                    "domain_conflict",
                    &format!("/entities/{i}/parentId"),
                    "Spatial parent and child must belong to the same domain.",
                ));
            }
        }
    }
    // Complete each chain once, so a deep tree does not require a quadratic cycle scan.
    let mut done = BTreeSet::new();
    for i in 0..entities.len() {
        let mut chain = BTreeSet::new();
        let mut current = Some(i);
        while let Some(j) = current {
            if done.contains(&j) {
                break;
            }
            if !chain.insert(j) {
                return Err(SpatialContractError::new(
                    "parent_cycle",
                    &format!("/entities/{j}/parentId"),
                    "Hierarchy contains a cycle.",
                ));
            }
            current = entities[j].parent_id().map(|id| by_id[id]);
        }
        done.extend(chain);
    }
    for (i, e) in entities.iter().enumerate() {
        if matches!(
            e.spatial(),
            Some(RuntimeSpatial::World3D {
                camera: Some(_),
                ..
            })
        ) {
            let mut current = Some(i);
            while let Some(j) = current {
                if let Some(RuntimeSpatial::World3D { transform, .. }) = entities[j].spatial() {
                    if transform.scale != [1.0; 3] {
                        return Err(SpatialContractError::new(
                            "camera_scale_unsupported",
                            &format!("/entities/{j}/spatial/transform/scale"),
                            "Camera and its ancestors require unit scale.",
                        ));
                    }
                }
                current = entities[j].parent_id().map(|id| by_id[id]);
            }
        }
    }
    Ok(())
}

/// Guard ONLY new reserved semantics. Unrelated legacy extension fields stay compatible.
pub fn reject_legacy_entity_extension(value: &Value) -> ContractResult<()> {
    for key in [
        "spatial",
        "transform3D",
        "visibility3D",
        "meshRenderer3D",
        "camera3D",
    ] {
        if value.get(key).is_some() {
            return Err(SpatialContractError::new(
                "envelope_upgrade_required",
                &format!("/{key}"),
                "Spatial fields require a v2 entity and scene envelope.",
            ));
        }
    }
    if value.get("schemaVersion").and_then(Value::as_str) == Some(RUNTIME_ENTITY_V2) {
        return Err(SpatialContractError::new(
            "envelope_upgrade_required",
            "/schemaVersion",
            "A v2 entity requires a v2 scene/prefab.",
        ));
    }
    if let Some(components) = value.get("components").and_then(Value::as_array) {
        for (i, c) in components.iter().enumerate() {
            let kind = c
                .get("componentType")
                .or_else(|| c.get("component_type"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if [
                "Transform3D",
                "Visibility3D",
                "MeshRenderer3D",
                "Camera3D",
                "Light3D",
            ]
            .iter()
            .any(|name| kind.eq_ignore_ascii_case(name))
                || c.pointer("/data/schemaVersion")
                    .and_then(Value::as_str)
                    .is_some_and(|v| {
                        [
                            "transform3d.v1",
                            "visibility3d.v1",
                            "mesh-renderer3d.v1",
                            "camera3d.v1",
                            "light3d.v1",
                        ]
                        .contains(&v)
                    })
            {
                return Err(SpatialContractError::new(
                    "envelope_upgrade_required",
                    &format!("/components/{i}"),
                    "3D components cannot be hidden in a legacy envelope.",
                ));
            }
        }
    }
    Ok(())
}
pub fn reject_legacy_document_extension(value: &Value) -> ContractResult<()> {
    if value.get("mainCamera").is_some() {
        return Err(SpatialContractError::new(
            "envelope_upgrade_required",
            "/mainCamera",
            "mainCamera requires a v2 scene.",
        ));
    }
    if let Some(items) = value.get("entities").and_then(Value::as_array) {
        for (i, e) in items.iter().enumerate() {
            reject_legacy_entity_extension(e).map_err(|e| e.at(&format!("/entities/{i}")))?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub enum RuntimeSceneDocument {
    Legacy(RuntimeScene),
    Spatial(RuntimeSceneV2),
}
#[derive(Debug, Clone)]
pub enum RuntimePrefabDocument {
    Legacy(RuntimePrefabData),
    Spatial(RuntimePrefabV2),
}

pub fn decode_runtime_scene(bytes: &[u8]) -> ContractResult<RuntimeSceneDocument> {
    decode_runtime_scene_observed(bytes, |_| ())
}

// The borrowing observer may name actual parsed backing while this owner holds
// it. It must finish before from_value consumes it; the strict v2 parse keeps
// the existing duplicate-key rejection instead of decoding a permissive Value.
pub(crate) fn decode_runtime_scene_observed(
    bytes: &[u8],
    mut observe: impl FnMut(&Value),
) -> ContractResult<RuntimeSceneDocument> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|e| SpatialContractError::new("schema_invalid", "", e.to_string()))?;
    observe(&value);
    match value.get("schemaVersion").and_then(Value::as_str) {
        Some(RUNTIME_SCENE_V2) => {
            drop(value);
            let strict = strict_json(bytes)?;
            observe(&strict);
            let mut scene: RuntimeSceneV2 = decode(strict, "")?;
            scene.validate_and_canonicalize()?;
            Ok(RuntimeSceneDocument::Spatial(scene))
        }
        Some("runtime-scene.v1") => {
            reject_legacy_document_extension(&value)?;
            decode(value, "").map(RuntimeSceneDocument::Legacy)
        }
        _ => Err(SpatialContractError::new(
            "schema_unsupported",
            "/schemaVersion",
            "Expected runtime-scene.v1 or v2.",
        )),
    }
}
pub fn decode_runtime_prefab(value: Value) -> ContractResult<RuntimePrefabDocument> {
    match value.get("schemaVersion").and_then(Value::as_str) {
        Some(RUNTIME_PREFAB_V2) => {
            let mut prefab: RuntimePrefabV2 = decode(value, "")?;
            prefab.validate_and_canonicalize()?;
            Ok(RuntimePrefabDocument::Spatial(prefab))
        }
        Some("runtime-prefab.v1") => {
            reject_legacy_document_extension(&value)?;
            decode(value, "").map(RuntimePrefabDocument::Legacy)
        }
        _ => Err(SpatialContractError::new(
            "schema_unsupported",
            "/schemaVersion",
            "Expected runtime-prefab.v1 or v2.",
        )),
    }
}
