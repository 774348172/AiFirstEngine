use crate::runtime_package::{RuntimeAssetManifest, RuntimeAssetRef};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAssetRecord {
    #[serde(alias = "asset_guid")]
    pub asset_guid: String,
    #[serde(alias = "asset_id")]
    pub asset_id: String,
    #[serde(rename = "type", alias = "assetType", alias = "asset_type")]
    pub asset_type: String,
    #[serde(default, alias = "sub_asset_id")]
    pub sub_asset_id: Option<String>,
    #[serde(default)]
    pub version: String,
    #[serde(alias = "cooked_asset_id")]
    pub cooked_asset_id: String,
    #[serde(alias = "bundle_id")]
    pub bundle_id: String,
    #[serde(alias = "loader_kind")]
    pub loader_kind: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub source_map_debug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleRecord {
    #[serde(alias = "bundle_id")]
    pub bundle_id: String,
    #[serde(default, alias = "mount_id")]
    pub mount_id: Option<String>,
    pub uri: String,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub mounted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CookedAssetRecord {
    #[serde(alias = "cooked_asset_id")]
    pub cooked_asset_id: String,
    #[serde(alias = "bundle_id")]
    pub bundle_id: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub compression: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAssetDependencyRecord {
    #[serde(alias = "asset_guid")]
    pub asset_guid: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeAssetResolveError {
    MissingAssetRef,
    TypeMismatch { expected: String, actual: String },
    SubAssetMissing,
}

#[derive(Debug, Clone)]
pub struct RuntimeAssetIndex {
    records_by_guid: HashMap<String, RuntimeAssetRecord>,
    guid_by_id: HashMap<String, String>,
    cooked_assets: HashMap<String, CookedAssetRecord>,
}

impl RuntimeAssetIndex {
    pub fn new(records: Vec<RuntimeAssetRecord>, cooked_assets: Vec<CookedAssetRecord>) -> Self {
        let mut records_by_guid = HashMap::new();
        let mut guid_by_id = HashMap::new();
        for record in records {
            guid_by_id.insert(record.asset_id.clone(), record.asset_guid.clone());
            records_by_guid.insert(record.asset_guid.clone(), record);
        }
        let cooked_assets = cooked_assets
            .into_iter()
            .map(|record| (record.cooked_asset_id.clone(), record))
            .collect();
        Self {
            records_by_guid,
            guid_by_id,
            cooked_assets,
        }
    }

    pub fn from_manifest(
        manifest: &RuntimeAssetManifest,
        runtime_records: &[RuntimeAssetRecord],
        cooked_assets: &[CookedAssetRecord],
        dependency_records: &[RuntimeAssetDependencyRecord],
    ) -> Self {
        if !runtime_records.is_empty() {
            let mut records = runtime_records.to_vec();
            apply_dependency_table(&mut records, dependency_records);
            return Self::new(records, cooked_assets.to_vec());
        }

        let mut records = Vec::new();
        let mut derived_cooked_assets = Vec::new();
        for asset in &manifest.assets {
            records.push(RuntimeAssetRecord {
                asset_guid: asset.id.clone(),
                asset_id: asset.id.clone(),
                asset_type: asset.asset_type.clone(),
                sub_asset_id: None,
                version: "legacy".to_string(),
                cooked_asset_id: asset.id.clone(),
                bundle_id: asset.bundle_id.clone(),
                loader_kind: asset.asset_type.clone(),
                dependencies: Vec::new(),
                hash: None,
                size: None,
                flags: vec!["compatibility_derived".to_string()],
                source_map_debug: Some(asset.source.clone()),
            });
            derived_cooked_assets.push(CookedAssetRecord {
                cooked_asset_id: asset.id.clone(),
                bundle_id: asset.bundle_id.clone(),
                path: None,
                offset: None,
                size: None,
                compression: None,
                hash: None,
            });
        }
        apply_dependency_table(&mut records, dependency_records);
        Self::new(records, derived_cooked_assets)
    }

    pub fn resolve(
        &self,
        asset_ref: &RuntimeAssetRef,
    ) -> Result<&RuntimeAssetRecord, RuntimeAssetResolveError> {
        if asset_ref.id.is_empty() && asset_ref.guid.as_deref().unwrap_or_default().is_empty() {
            return Err(RuntimeAssetResolveError::MissingAssetRef);
        }

        let record = if let Some(guid) = &asset_ref.guid {
            self.records_by_guid.get(guid)
        } else {
            self.guid_by_id
                .get(&asset_ref.id)
                .and_then(|guid| self.records_by_guid.get(guid))
        }
        .ok_or(RuntimeAssetResolveError::MissingAssetRef)?;

        if record.asset_type != asset_ref.asset_type {
            return Err(RuntimeAssetResolveError::TypeMismatch {
                expected: asset_ref.asset_type.clone(),
                actual: record.asset_type.clone(),
            });
        }
        if asset_ref.sub_asset.as_ref() != record.sub_asset_id.as_ref()
            && asset_ref.sub_asset.is_some()
        {
            return Err(RuntimeAssetResolveError::SubAssetMissing);
        }
        Ok(record)
    }

    pub fn record_by_guid(&self, guid: &str) -> Option<&RuntimeAssetRecord> {
        self.records_by_guid.get(guid)
    }

    pub fn cooked_asset(&self, cooked_asset_id: &str) -> Option<&CookedAssetRecord> {
        self.cooked_assets.get(cooked_asset_id)
    }

    pub fn merge_patch(&mut self, patch: RuntimeAssetIndex) {
        for (_, record) in patch.records_by_guid {
            self.guid_by_id
                .insert(record.asset_id.clone(), record.asset_guid.clone());
            self.records_by_guid
                .insert(record.asset_guid.clone(), record);
        }
        for (id, cooked) in patch.cooked_assets {
            self.cooked_assets.insert(id, cooked);
        }
    }
}

fn apply_dependency_table(
    records: &mut [RuntimeAssetRecord],
    dependency_records: &[RuntimeAssetDependencyRecord],
) {
    if dependency_records.is_empty() {
        return;
    }
    let dependencies_by_guid: HashMap<&str, &Vec<String>> = dependency_records
        .iter()
        .map(|record| (record.asset_guid.as_str(), &record.dependencies))
        .collect();
    for record in records {
        if let Some(dependencies) = dependencies_by_guid.get(record.asset_guid.as_str()) {
            record.dependencies = (*dependencies).clone();
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimePackageMountTable {
    bundles: HashMap<String, BundleRecord>,
    mounted: HashSet<String>,
}

impl RuntimePackageMountTable {
    pub fn new(bundle_records: Vec<BundleRecord>) -> Self {
        let mut bundles = HashMap::new();
        let mut mounted = HashSet::new();
        for record in bundle_records {
            if record.mounted {
                mounted.insert(record.bundle_id.clone());
            }
            bundles.insert(record.bundle_id.clone(), record);
        }
        Self { bundles, mounted }
    }

    pub fn from_manifest(manifest: &RuntimeAssetManifest) -> Self {
        if !manifest.bundle_table.is_empty() {
            return Self::new(manifest.bundle_table.clone());
        }

        let mut bundles = HashMap::new();
        for asset in &manifest.assets {
            bundles
                .entry(asset.bundle_id.clone())
                .or_insert_with(|| BundleRecord {
                    bundle_id: asset.bundle_id.clone(),
                    mount_id: None,
                    uri: format!("bundles/{}", asset.bundle_id),
                    hash: None,
                    version: None,
                    mounted: false,
                });
        }
        Self::new(bundles.into_values().collect())
    }

    pub fn mount_bundle(&mut self, bundle_id: &str) -> bool {
        if self.bundles.contains_key(bundle_id) {
            self.mounted.insert(bundle_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn unmount_bundle(&mut self, bundle_id: &str) {
        self.mounted.remove(bundle_id);
    }

    pub fn is_bundle_mounted(&self, bundle_id: &str) -> bool {
        self.mounted.contains(bundle_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeAssetLoadState {
    Loading,
    Ready,
    Failed,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAssetHandle {
    pub handle_id: u64,
    pub asset_guid: String,
    pub asset_id: String,
    pub asset_type: String,
    pub sub_asset_id: Option<String>,
    pub cooked_asset_id: String,
    pub bundle_id: String,
    pub runtime_resource_id: Option<String>,
    pub state: RuntimeAssetLoadState,
    pub generation: u64,
    pub ref_count: u32,
    pub loader_kind: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedRuntimeResourceKind {
    Texture,
    Model,
    Material,
    Audio,
    Scene,
    Prefab,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRuntimeResource {
    pub resource_id: String,
    pub kind: PreparedRuntimeResourceKind,
}

#[derive(Debug, Clone)]
pub struct DecodedAsset {
    pub cooked_asset_id: String,
    pub asset_type: String,
    pub bytes_len: usize,
    pub source_debug: String,
    pub ref_count: u32,
}

pub fn prepared_kind(loader_kind: &str) -> PreparedRuntimeResourceKind {
    match loader_kind {
        "texture" => PreparedRuntimeResourceKind::Texture,
        "model" => PreparedRuntimeResourceKind::Model,
        "material" => PreparedRuntimeResourceKind::Material,
        "audio" => PreparedRuntimeResourceKind::Audio,
        "scene" => PreparedRuntimeResourceKind::Scene,
        "prefab" => PreparedRuntimeResourceKind::Prefab,
        _ => PreparedRuntimeResourceKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_asset_by_id_and_guid() {
        let index = RuntimeAssetIndex::new(
            vec![record("guid-texture", "texture-main", "texture")],
            vec![cooked("cooked-texture", "startup")],
        );
        let by_id = RuntimeAssetRef {
            id: "texture-main".to_string(),
            asset_type: "texture".to_string(),
            guid: None,
            sub_asset: None,
        };
        assert_eq!(index.resolve(&by_id).unwrap().asset_guid, "guid-texture");

        let by_guid = RuntimeAssetRef {
            id: "ignored".to_string(),
            asset_type: "texture".to_string(),
            guid: Some("guid-texture".to_string()),
            sub_asset: None,
        };
        assert_eq!(index.resolve(&by_guid).unwrap().asset_id, "texture-main");
    }

    #[test]
    fn type_mismatch_is_explicit() {
        let index = RuntimeAssetIndex::new(
            vec![record("guid-texture", "texture-main", "texture")],
            vec![cooked("cooked-texture", "startup")],
        );
        let asset_ref = RuntimeAssetRef {
            id: "texture-main".to_string(),
            asset_type: "material".to_string(),
            guid: None,
            sub_asset: None,
        };
        assert!(matches!(
            index.resolve(&asset_ref),
            Err(RuntimeAssetResolveError::TypeMismatch { .. })
        ));
    }

    fn record(guid: &str, id: &str, asset_type: &str) -> RuntimeAssetRecord {
        RuntimeAssetRecord {
            asset_guid: guid.to_string(),
            asset_id: id.to_string(),
            asset_type: asset_type.to_string(),
            sub_asset_id: None,
            version: "1".to_string(),
            cooked_asset_id: format!("cooked-{}", id),
            bundle_id: "startup".to_string(),
            loader_kind: asset_type.to_string(),
            dependencies: Vec::new(),
            hash: None,
            size: None,
            flags: Vec::new(),
            source_map_debug: None,
        }
    }

    fn cooked(cooked_asset_id: &str, bundle_id: &str) -> CookedAssetRecord {
        CookedAssetRecord {
            cooked_asset_id: cooked_asset_id.to_string(),
            bundle_id: bundle_id.to_string(),
            path: None,
            offset: None,
            size: None,
            compression: None,
            hash: None,
        }
    }
}
