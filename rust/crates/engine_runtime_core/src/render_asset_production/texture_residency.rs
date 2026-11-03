//! Reusable package origin and CPU upload lease owned by the existing Prepare seam.
use crate::runtime_asset::{CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord};
use crate::runtime_asset_loader::RuntimeAssetLoader;
use crate::runtime_package::RuntimeAssetRef;
use crate::runtime_texture::{RuntimeTexturePayload, VerifiedTexturePayload};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub(crate) struct PackageTexturePixels {
    package_dir: PathBuf,
    record: RuntimeAssetRecord,
    cooked: CookedAssetRecord,
    pub(super) descriptor: VerifiedTexturePayload,
    pub(super) identity: String,
    pub(super) byte_len: usize,
    resident: Mutex<Option<Arc<RuntimeTexturePayload>>>,
}

impl PartialEq for PackageTexturePixels {
    fn eq(&self, other: &Self) -> bool {
        self.package_dir == other.package_dir
            && self.record == other.record
            && self.cooked == other.cooked
            && self.identity == other.identity
    }
}
impl Eq for PackageTexturePixels {}

impl PackageTexturePixels {
    pub(super) fn new(
        loader: &RuntimeAssetLoader,
        verified: &VerifiedTexturePayload,
    ) -> Option<Self> {
        let payload = verified.payload();
        let reference = RuntimeAssetRef {
            id: payload.asset_id.clone(),
            asset_type: "texture".into(),
            guid: None,
            sub_asset: None,
        };
        let record = loader.asset_index().resolve(&reference).ok()?.clone();
        let cooked = loader
            .asset_index()
            .cooked_asset(&record.cooked_asset_id)?
            .clone();
        // Legacy/in-memory sources without a complete reproducible receipt keep
        // their original CPU retention; never invent a package reload origin.
        if record.cooked_asset_id != payload.cooked_asset_id
            || cooked.path.is_none()
            || record.hash.is_none()
            || record.hash != cooked.hash
            || record.size.is_none()
            || record.size != cooked.size
            || record.bundle_id != cooked.bundle_id
        {
            return None;
        }
        Some(Self {
            package_dir: loader.package_dir().to_path_buf(),
            record,
            cooked,
            descriptor: verified.without_pixels(),
            identity: verified.identity_for(payload),
            byte_len: payload.rgba8.len(),
            resident: Mutex::new(Some(payload.clone())),
        })
    }

    fn reference(&self) -> RuntimeAssetRef {
        RuntimeAssetRef {
            id: self.record.asset_id.clone(),
            asset_type: "texture".into(),
            guid: Some(self.record.asset_guid.clone()),
            sub_asset: self.record.sub_asset_id.clone(),
        }
    }

    pub(super) fn release_loader_pixels(&self, loader: &mut RuntimeAssetLoader) {
        loader.release_texture_cpu_payload(&self.reference());
    }

    pub(super) fn acquire(&self) -> Result<Arc<RuntimeTexturePayload>, String> {
        let mut resident = self
            .resident
            .lock()
            .map_err(|_| "texture.pixel_source_poisoned")?;
        if let Some(payload) = &*resident {
            return Ok(payload.clone());
        }
        let index = RuntimeAssetIndex::new(vec![self.record.clone()], vec![self.cooked.clone()]);
        let payload = crate::runtime_texture::load_checked_runtime_texture_payload(
            &self.package_dir,
            &index,
            &self.reference(),
        )?;
        let mut verified = VerifiedTexturePayload::new(payload);
        let expected = self.descriptor.payload();
        if verified.payload().format != expected.format
            || verified.payload().color_space != expected.color_space
            || verified.payload().sampler != expected.sampler
            || verified.payload().mip_count != expected.mip_count
        {
            verified = verified
                .with_material_sampling(expected.sampler.clone(), expected.format.ends_with("Srgb"))
                .map_err(|e| format!("texture.source_sampling:{e}"))?;
        }
        super::validate_texture_payload(verified.payload()).map_err(|e| e.to_string())?;
        if verified.identity_for(verified.payload()) != self.identity {
            return Err("texture.source_identity_mismatch".into());
        }
        let payload = verified.payload().clone();
        *resident = Some(payload.clone());
        Ok(payload)
    }

    pub(super) fn release(&self) {
        if let Ok(mut resident) = self.resident.lock() {
            *resident = None;
        }
    }

    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        if let Ok(resident) = self.resident.lock() {
            if let Some(payload) = &*resident {
                payload.observe_memory(sample, "snapshot");
            }
        }
        self.record.observe_memory(sample, "assets", "snapshot");
        self.cooked.observe_memory(sample, "assets", "snapshot");
    }
}
