//! In-memory RuntimeEntity compatibility with explicit v1/v2 wire dispatch.
use super::*;
use crate::animator2d::RuntimeAnimator2D;
use crate::runtime_package::{
    RuntimeMesh, RuntimeProjectComponent, RuntimeSpriteRenderer2D, RuntimeTransform,
};
// Runtime's in-memory entity keeps old call sites, while serialization preserves
// the two explicit wire contracts (never a v1 object with hidden 3D fields).
impl From<RuntimeEntityV2> for RuntimeEntity {
    fn from(v: RuntimeEntityV2) -> Self {
        Self {
            spatial: Some(v.spatial),
            schema_version: v.schema_version,
            id: v.id,
            name: v.name,
            kind: "entity".into(),
            enabled: v.enabled,
            parent_id: v.parent_id,
            sibling_order: v.sibling_order,
            transform: None,
            mesh: None,
            sprite_renderer2d: None,
            animator2d: None,
            components: v
                .components
                .into_iter()
                .map(|c| crate::runtime_package::RuntimeProjectComponent {
                    component_type: c.component_type,
                    data: c.data,
                })
                .collect(),
        }
    }
}
impl RuntimeEntity {
    pub fn spatial_document(&self) -> ContractResult<RuntimeEntityDocument> {
        if let Some(spatial) = &self.spatial {
            if self.transform.is_some()
                || self.mesh.is_some()
                || self.sprite_renderer2d.is_some()
                || self.animator2d.is_some()
                || self
                    .components
                    .iter()
                    .any(|c| c.component_type == "SpineAnimator")
            {
                return Err(SpatialContractError::new(
                    "domain_conflict",
                    "/spatial",
                    "3D entities cannot carry legacy spatial fields.",
                ));
            }
            Ok(RuntimeEntityDocument::Spatial(RuntimeEntityV2 {
                schema_version: self.schema_version.clone(),
                id: self.id.clone(),
                name: self.name.clone(),
                enabled: self.enabled,
                parent_id: self.parent_id.clone(),
                sibling_order: self.sibling_order,
                spatial: spatial.clone(),
                components: self
                    .components
                    .iter()
                    .map(|c| ProjectDataComponent {
                        component_type: c.component_type.clone(),
                        data: c.data.clone(),
                    })
                    .collect(),
            }))
        } else {
            Ok(RuntimeEntityDocument::Legacy(self.clone()))
        }
    }
}
impl Serialize for RuntimeEntity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.spatial.is_some() {
            let RuntimeEntityDocument::Spatial(v) =
                self.spatial_document().map_err(serde::ser::Error::custom)?
            else {
                unreachable!()
            };
            return v.serialize(serializer);
        }
        let binding = crate::spine_animation::SpineAnimatorBinding::from_entity(self)
            .map_err(serde::ser::Error::custom)?;
        let components: Vec<_> = self
            .components
            .iter()
            .filter(|c| c.component_type != "SpineAnimator")
            .collect();
        let mut value = serde_json::json!({"schemaVersion":self.schema_version,"id":self.id,"name":self.name,"kind":self.kind,
            "enabled":self.enabled,"parentId":self.parent_id,"siblingOrder":self.sibling_order,
            "transform":self.transform,"mesh":self.mesh,"spriteRenderer2D":self.sprite_renderer2d,
            "animator2d":self.animator2d,"components":components});
        if let Some(binding) = binding {
            value["spineAnimator"] =
                serde_json::to_value(binding).map_err(serde::ser::Error::custom)?;
        }
        value.serialize(serializer)
    }
}
impl From<RuntimeSceneV2> for RuntimeScene {
    fn from(v: RuntimeSceneV2) -> Self {
        Self {
            main_camera: v.main_camera,
            schema_version: v.schema_version,
            id: v.id,
            name: v.name,
            gravity: v.gravity,
            background: v.background,
            sky_color: v.sky_color,
            entities: v
                .entities
                .into_iter()
                .map(|e| match e {
                    RuntimeEntityDocument::Legacy(e) => e,
                    RuntimeEntityDocument::Spatial(e) => e.into(),
                })
                .collect(),
        }
    }
}
impl RuntimeScene {
    pub fn validate_spatial(&self) -> ContractResult<()> {
        if self.schema_version == RUNTIME_SCENE_V2 {
            RuntimeSceneV2 {
                schema_version: self.schema_version.clone(),
                id: self.id.clone(),
                name: self.name.clone(),
                gravity: self.gravity,
                background: self.background.clone(),
                sky_color: self.sky_color.clone(),
                main_camera: self.main_camera.clone(),
                entities: self
                    .entities
                    .iter()
                    .map(RuntimeEntity::spatial_document)
                    .collect::<ContractResult<_>>()?,
            }
            .validate_and_canonicalize()
        } else {
            version(&self.schema_version, "runtime-scene.v1")?;
            reject_legacy_document_extension(
                &serde_json::to_value(self)
                    .map_err(|e| SpatialContractError::new("decode", "", e.to_string()))?,
            )
        }
    }
}
impl From<RuntimePrefabV2> for crate::runtime_package::RuntimePrefabData {
    fn from(v: RuntimePrefabV2) -> Self {
        Self {
            schema_version: v.schema_version,
            id: v.id,
            name: v.name,
            root_entity_id: Some(v.root_entity_id),
            entities: v
                .entities
                .into_iter()
                .map(|e| match e {
                    RuntimeEntityDocument::Legacy(e) => e,
                    RuntimeEntityDocument::Spatial(e) => e.into(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyRuntimeEntity {
    #[serde(default)]
    pub spine_animator: Option<crate::spine_animation::SpineAnimatorBinding>,
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

impl<'de> Deserialize<'de> for RuntimeEntity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str)
            == Some(crate::spatial_contract::RUNTIME_ENTITY_V2)
        {
            let mut entity: crate::spatial_contract::RuntimeEntityV2 =
                serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            entity
                .validate_and_canonicalize()
                .map_err(serde::de::Error::custom)?;
            Ok(entity.into())
        } else {
            crate::spatial_contract::reject_legacy_entity_extension(&value)
                .map_err(serde::de::Error::custom)?;
            let mut e: LegacyRuntimeEntity =
                serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            if let Some(binding) = e.spine_animator {
                if e.components
                    .iter()
                    .any(|c| c.component_type == "SpineAnimator")
                {
                    return Err(serde::de::Error::custom("duplicate SpineAnimator binding"));
                }
                e.components.push(RuntimeProjectComponent {
                    component_type: "SpineAnimator".into(),
                    data: serde_json::to_value(binding).map_err(serde::de::Error::custom)?,
                });
            }
            Ok(Self {
                spatial: None,
                schema_version: e.schema_version,
                id: e.id,
                name: e.name,
                kind: e.kind,
                enabled: e.enabled,
                parent_id: e.parent_id,
                sibling_order: e.sibling_order,
                transform: e.transform,
                mesh: e.mesh,
                sprite_renderer2d: e.sprite_renderer2d,
                animator2d: e.animator2d,
                components: e.components,
            })
        }
    }
}
