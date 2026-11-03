//! Cooked graphics assets resolve only from the existing RuntimePackage index.
use crate::aui_graphics::GraphicsError;
use crate::runtime_asset::RuntimeAssetIndex;
use crate::runtime_package::RuntimeAssetManifest;
use crate::runtime_package_source::RuntimePackageSource;
use project_runtime_sdk::aui_draw2d::Graphics2DAsset;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

const MAX_GRAPHICS_ASSET_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GraphicsAssetRegistry {
    assets: Arc<BTreeMap<String, Graphics2DAsset>>,
}
impl GraphicsAssetRegistry {
    pub(crate) fn shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.assets, &other.assets)
    }
    pub fn from_assets(assets: Vec<Graphics2DAsset>) -> Result<Self, GraphicsError> {
        let mut values = BTreeMap::new();
        for asset in assets {
            asset
                .validate()
                .map_err(|e| error(&asset.asset_id, e.to_string()))?;
            if values.insert(asset.asset_id.clone(), asset).is_some() {
                return Err(error("", "duplicate graphics asset ID"));
            }
        }
        Ok(Self {
            assets: Arc::new(values),
        })
    }
    pub fn resolve(&self, id: &str, guid: &str) -> Result<&Graphics2DAsset, GraphicsError> {
        self.assets
            .get(id)
            .filter(|asset| asset.asset_guid == guid)
            .ok_or_else(|| error(id, "graphics ID/GUID not present in RuntimePackage"))
    }
    pub fn from_runtime_package(
        package_dir: &Path,
        manifest: &RuntimeAssetManifest,
        index: &RuntimeAssetIndex,
    ) -> Result<Self, GraphicsError> {
        Self::from_package_source(
            &RuntimePackageSource::directory(package_dir),
            package_dir,
            manifest,
            index,
        )
    }

    pub fn from_package_source(
        source: &RuntimePackageSource,
        _package_dir: &Path,
        manifest: &RuntimeAssetManifest,
        index: &RuntimeAssetIndex,
    ) -> Result<Self, GraphicsError> {
        let mut assets = Vec::new();
        for record in manifest
            .runtime_asset_index
            .iter()
            .filter(|record| record.asset_type == "graphics2d")
        {
            let cooked = index
                .cooked_asset(&record.cooked_asset_id)
                .ok_or_else(|| error(&record.asset_id, "cooked asset record missing"))?;
            let relative = cooked
                .path
                .as_deref()
                .ok_or_else(|| error(&record.asset_id, "cooked path missing"))?;
            let _path = source
                .resolve(relative)
                .map_err(|e| error(&record.asset_id, e.to_string()))?;
            let bytes = source
                .read_limited(relative, MAX_GRAPHICS_ASSET_BYTES as usize)
                .map_err(|e| error(&record.asset_id, e.to_string()))?;
            if bytes.len() as u64 > MAX_GRAPHICS_ASSET_BYTES {
                return Err(error(&record.asset_id, "graphics asset exceeds 1 MiB"));
            }
            if cooked
                .hash
                .as_ref()
                .is_some_and(|hash| *hash != crate::canonical_digest::sha256_prefixed(&bytes))
            {
                return Err(error(
                    &record.asset_id,
                    "cooked graphics content hash mismatch",
                ));
            }
            let asset: Graphics2DAsset = serde_json::from_slice(&bytes)
                .map_err(|e| error(&record.asset_id, e.to_string()))?;
            if asset.asset_id != record.asset_id || asset.asset_guid != record.asset_guid {
                return Err(error(&record.asset_id, "cooked graphics ID/GUID mismatch"));
            }
            asset
                .validate()
                .map_err(|e| error(&record.asset_id, e.to_string()))?;
            assets.push(asset);
        }
        Self::from_assets(assets)
    }
}
fn error(id: &str, message: impl AsRef<str>) -> GraphicsError {
    GraphicsError {
        code: "aui_graphics.asset_invalid",
        item: None,
        message: format!("asset {id}: {}", message.as_ref()),
    }
}
