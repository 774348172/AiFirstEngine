//! Cooked Spine package data. Native objects stay in the optional, thread-confined backend.
#[cfg(feature = "spine")]
#[path = "spine_animation/control.rs"]
pub mod control;
#[cfg_attr(test, path = "spine_animation/measurement.rs")]
pub mod measurement;
#[cfg_attr(test, path = "spine_animation/schema.rs")]
pub mod schema;
#[cfg_attr(test, path = "spine_animation/session.rs")]
pub mod session;
use crate::canonical_digest::sha256_prefixed;
use crate::runtime_package_path::safe_join_runtime_package;
use serde::{Deserialize, Serialize};
#[cfg(feature = "spine")]
use std::fs;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const SPINE_CAPABILITY: &str = "animator.spine.4.2.v1";
pub const SPINE_BACKEND_COMMIT: &str = "e7dc1435fa4a0083ab431f1b28e083c14a1f5c68";
pub const SPINE_REGISTRY_SCHEMA: &str = "cooked-spine-animator-registry.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpineAnimatorBinding {
    pub controller_id: String,
    pub controller_index: u16,
    pub registry_digest: String,
}
impl SpineAnimatorBinding {
    pub fn from_entity(
        entity: &crate::runtime_package::RuntimeEntity,
    ) -> Result<Option<Self>, String> {
        let mut candidates = entity
            .components
            .iter()
            .filter(|c| c.component_type == "SpineAnimator");
        let Some(c) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() || entity.spatial.is_some() || entity.animator2d.is_some() {
            return Err("animator.binding.entity_conflict".into());
        }
        serde_json::from_value(c.data.clone())
            .map(Some)
            .map_err(|e| e.to_string())
    }
}

pub fn validate_scene_bindings(
    scene: &crate::runtime_package::RuntimeScene,
    registry: Option<&CookedSpineRegistry>,
) -> Result<(), String> {
    for entity in &scene.entities {
        if let Some(binding) = SpineAnimatorBinding::from_entity(entity)? {
            registry
                .ok_or("animator.runtime.registry_uninitialized")?
                .validate_binding(&binding)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpineManifestIndex {
    pub path: String,
    pub registry_digest: String,
    pub required_capability: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpineBackend {
    pub id: String,
    pub major_minor: String,
    pub commit: String,
    pub capabilities: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpinePage {
    pub name: String,
    pub texture_asset_id: String,
    pub pma: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedSpineAsset {
    pub asset_id: String,
    pub source_digest: String,
    pub payload_path: String,
    pub payload_digest: String,
    pub atlas_path: String,
    pub atlas_digest: String,
    pub pages: Vec<SpinePage>,
    pub animations: Vec<String>,
    pub skins: Vec<String>,
    pub bones: Vec<String>,
    pub slots: Vec<String>,
    pub events: Vec<serde_json::Value>,
    pub limits: BTreeMap<String, usize>,
    pub pixels_per_unit: f32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedSpineRegistry {
    pub schema_version: String,
    pub registry_digest: String,
    pub tick_rate: u32,
    pub backend: SpineBackend,
    pub assets: Vec<CookedSpineAsset>,
    pub controllers: Vec<schema::CookedSpineController>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpinePackageSource {
    pub registry: CookedSpineRegistry,
    pub payloads: BTreeMap<String, Vec<u8>>,
}
impl CookedSpineRegistry {
    pub fn validate_binding(&self, b: &SpineAnimatorBinding) -> Result<(), String> {
        if b.registry_digest != self.registry_digest
            || self
                .controllers
                .get(b.controller_index as usize)
                .is_none_or(|c| c.asset_id != b.controller_id)
        {
            return Err("animator.binding.registry_identity".into());
        }
        Ok(())
    }
    pub fn empty(tick_rate: u32) -> Self {
        Self {
            schema_version: SPINE_REGISTRY_SCHEMA.into(),
            registry_digest: String::new(),
            tick_rate,
            backend: SpineBackend {
                id: "spine-cpp".into(),
                major_minor: "4.2".into(),
                commit: SPINE_BACKEND_COMMIT.into(),
                capabilities: vec![SPINE_CAPABILITY.into()],
            },
            assets: Vec::new(),
            controllers: Vec::new(),
        }
    }
    fn digest(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self).map_err(|e| e.to_string())?;
        value.as_object_mut().unwrap().remove("registryDigest");
        Ok(sha256_prefixed(
            &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
        ))
    }
    pub fn seal(&mut self) -> Result<(), String> {
        self.registry_digest = self.digest()?;
        self.validate()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SPINE_REGISTRY_SCHEMA
            || self.tick_rate == 0
            || self.tick_rate > 1000
            || self.backend.id != "spine-cpp"
            || self.backend.major_minor != "4.2"
            || self.backend.commit != SPINE_BACKEND_COMMIT
            || self.backend.capabilities != [SPINE_CAPABILITY]
            || self.registry_digest != self.digest()?
            || self.assets.len() > 1024
            || self.controllers.len() > 1024
        {
            return Err("spine.package.registry_identity_or_unsupported_controller".into());
        }
        let mut ids = BTreeSet::new();
        for a in &self.assets {
            if a.asset_id.is_empty()
                || !ids.insert(&a.asset_id)
                || !a.pixels_per_unit.is_finite()
                || a.pixels_per_unit <= 0.0
                || a.pages.is_empty()
                || a.pages.len() > 8
                || a.bones.len() > 256
                || a.slots.len() > 256
                || a.animations.len() > 128
                || a.skins.len() > 16
                || [&a.source_digest, &a.payload_digest, &a.atlas_digest]
                    .iter()
                    .any(|v| !crate::render_contract::is_sha256_digest(v))
            {
                return Err("spine.package.invalid_asset".into());
            }
            for path in [&a.payload_path, &a.atlas_path] {
                safe_join_runtime_package(Path::new("."), path).map_err(|e| e.to_string())?;
            }
            for names in [&a.bones, &a.slots, &a.animations, &a.skins] {
                if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
                    return Err("spine.package.duplicate_name".into());
                }
            }
            if a.pages
                .iter()
                .map(|p| &p.name)
                .collect::<BTreeSet<_>>()
                .len()
                != a.pages.len()
            {
                return Err("spine.package.duplicate_page".into());
            }
        }
        let mut controller_ids = BTreeSet::new();
        for controller in &self.controllers {
            if !controller_ids.insert(&controller.asset_id) {
                return Err("animator.binding.duplicate_controller".into());
            }
            let asset = self
                .assets
                .iter()
                .find(|a| a.asset_id == controller.skeleton_asset_id)
                .ok_or("animator.binding.unknown_skeleton")?;
            controller.validate(asset, self.tick_rate)?;
        }
        Ok(())
    }
    #[cfg(feature = "spine")]
    pub fn validated_asset(
        &self,
        root: &Path,
        id: &str,
    ) -> Result<engine_spine_runtime::import::ValidatedSource, String> {
        self.validate()?;
        let a = self
            .assets
            .iter()
            .find(|a| a.asset_id == id)
            .ok_or("spine.package.asset_missing")?;
        let read = |path: &str, digest: &str, limit: u64| -> Result<String, String> {
            let path = safe_join_runtime_package(root, path).map_err(|e| e.to_string())?;
            if fs::metadata(&path).map_err(|e| e.to_string())?.len() > limit {
                return Err("spine.package.payload_capacity".into());
            }
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            if bytes.len() as u64 > limit || sha256_prefixed(&bytes) != digest {
                return Err("spine.package.payload_digest".into());
            }
            String::from_utf8(bytes).map_err(|e| e.to_string())
        };
        let json = read(&a.payload_path, &a.payload_digest, 32 * 1024 * 1024)?;
        let atlas = read(&a.atlas_path, &a.atlas_digest, 1024 * 1024)?;
        let refs = a
            .pages
            .iter()
            .map(|p| (p.name.clone(), p.texture_asset_id.clone()))
            .collect();
        let checked = engine_spine_runtime::import::validate(&json, &atlas, &refs)
            .map_err(|e| format!("{} {} {}", e.code, e.path, e.message))?;
        if checked.bones != a.bones
            || checked.slots != a.slots
            || checked.animations != a.animations
            || checked.skins != a.skins
            || checked.pages.len() != a.pages.len()
            || checked
                .pages
                .iter()
                .zip(&a.pages)
                .any(|(c, p)| c.pma != p.pma || c.name != p.name)
        {
            return Err("spine.package.metadata_mismatch".into());
        }
        Ok(checked)
    }
    #[cfg(feature = "spine")]
    pub(crate) fn validate_dependencies(
        &self,
        root: &Path,
        index: &crate::runtime_asset::RuntimeAssetIndex,
    ) -> Result<(), String> {
        use crate::runtime_package::RuntimeAssetRef;
        for asset in &self.assets {
            let checked = self.validated_asset(root, &asset.asset_id)?;
            let skeleton = index
                .resolve(&RuntimeAssetRef {
                    id: asset.asset_id.clone(),
                    asset_type: "spine-skeleton".into(),
                    guid: None,
                    sub_asset: None,
                })
                .map_err(|e| format!("spine.package.skeleton_reference: {e:?}"))?;
            let cooked = index
                .cooked_asset(&skeleton.cooked_asset_id)
                .ok_or("spine.package.skeleton_payload_missing")?;
            if cooked.path.as_deref() != Some(&asset.payload_path)
                || cooked.hash.as_ref() != Some(&asset.payload_digest)
                || skeleton.hash.as_ref() != Some(&asset.payload_digest)
            {
                return Err("spine.package.skeleton_identity".into());
            }
            let mut guids = BTreeSet::new();
            for (page, expected) in asset.pages.iter().zip(&checked.pages) {
                let reference = RuntimeAssetRef {
                    id: page.texture_asset_id.clone(),
                    asset_type: "texture".into(),
                    guid: None,
                    sub_asset: None,
                };
                let record = index
                    .resolve(&reference)
                    .map_err(|e| format!("spine.package.page_reference: {e:?}"))?;
                guids.insert(record.asset_guid.clone());
                let texture =
                    crate::runtime_texture::load_runtime_texture_payload(root, index, &reference)
                        .map_err(|e| format!("spine.package.page_payload: {e:?}"))?;
                if texture.width != expected.width
                    || texture.height != expected.height
                    || texture.width > 4096
                    || texture.height > 4096
                    || texture.format != "rgba8Unorm"
                    || texture.color_space != "linear"
                    || texture.mip_count != 1
                {
                    return Err("spine.package.page_format_or_extent".into());
                }
                // The original texture owner seals metadata + exact pixel bytes.
                let cooked = index
                    .cooked_asset(&record.cooked_asset_id)
                    .ok_or("spine.package.page_payload_missing")?;
                let path = safe_join_runtime_package(
                    root,
                    cooked
                        .path
                        .as_deref()
                        .ok_or("spine.package.page_metadata_missing")?,
                )
                .map_err(|e| e.to_string())?;
                let mut bytes = fs::read(path).map_err(|e| e.to_string())?;
                let metadata_len = bytes.len() as u64;
                bytes.extend_from_slice(texture.rgba8.as_slice());
                let digest = sha256_prefixed(&bytes);
                if record.hash.as_ref() != Some(&digest)
                    || cooked.hash.as_ref() != Some(&digest)
                    || record.size != Some(metadata_len)
                    || cooked.size != Some(metadata_len)
                {
                    return Err("spine.package.page_identity".into());
                }
            }
            if guids != skeleton.dependencies.iter().cloned().collect() {
                return Err("spine.package.page_dependency_closure".into());
            }
        }
        Ok(())
    }
    #[cfg(feature = "spine")]
    pub fn load_asset(&self, root: &Path, id: &str) -> Result<engine_spine_runtime::Asset, String> {
        self.validated_asset(root, id)?
            .load()
            .map_err(|e| format!("{} {} {}", e.code, e.path, e.message))
    }
}

pub(crate) fn load_registry(
    root: &Path,
    index: Option<&SpineManifestIndex>,
) -> Result<Option<CookedSpineRegistry>, String> {
    let Some(index) = index else {
        return Ok(None);
    };
    if index.required_capability != SPINE_CAPABILITY {
        return Err("spine.package.capability_unsupported".into());
    }
    #[cfg(not(feature = "spine"))]
    {
        let _ = root;
        Err("spine.package.backend_not_compiled".into())
    }
    #[cfg(feature = "spine")]
    {
        let path = safe_join_runtime_package(root, &index.path).map_err(|e| e.to_string())?;
        if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
            return Err("spine.package.registry_capacity".into());
        }
        let registry: CookedSpineRegistry =
            serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        registry.validate()?;
        if registry.registry_digest != index.registry_digest {
            return Err("spine.package.registry_digest".into());
        }
        for a in &registry.assets {
            registry.validated_asset(root, &a.asset_id)?;
        }
        Ok(Some(registry))
    }
}
