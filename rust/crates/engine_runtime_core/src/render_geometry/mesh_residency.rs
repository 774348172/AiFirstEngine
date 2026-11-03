//! Original package receipt and explicit CPU mesh leases in the Prepare owner.
use super::{GeometryBuffers, Provenance};
use crate::render_contract::{CookedRenderAsset, RenderAssetData, RenderContract};
use crate::runtime_asset::{CookedAssetRecord, RuntimeAssetRecord};
use crate::runtime_asset_loader::RuntimeAssetLoader;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub(super) struct PackageMeshBytes {
    package_dir: PathBuf,
    record: RuntimeAssetRecord,
    cooked: CookedAssetRecord,
    expected: GeometryBuffers,
    resident: Mutex<Option<Arc<GeometryBuffers>>>,
}

impl PackageMeshBytes {
    pub(super) fn new(
        loader: &RuntimeAssetLoader,
        buffers: &Arc<GeometryBuffers>,
        provenance: &Provenance,
    ) -> Option<Self> {
        let record = loader
            .asset_index()
            .record_by_guid(provenance.asset_id.as_deref()?)?
            .clone();
        let cooked = loader
            .asset_index()
            .cooked_asset(&record.cooked_asset_id)?
            .clone();
        // Incomplete/legacy or compressed receipts retain their original bytes.
        if record.asset_type != "mesh"
            || record.hash.is_none()
            || record.hash != cooked.hash
            || record.hash != provenance.content_digest
            || record.size.is_none()
            || record.size != cooked.size
            || record.bundle_id != cooked.bundle_id
            || cooked.offset.is_some_and(|n| n != 0)
            || cooked.compression.as_deref().is_some_and(|s| s != "none")
        {
            return None;
        }
        crate::runtime_package_path::safe_join_runtime_package(
            loader.package_dir(),
            cooked.path.as_deref()?,
        )
        .ok()?;
        Some(Self {
            package_dir: loader.package_dir().to_path_buf(),
            record,
            cooked,
            expected: buffers.without_bytes(),
            resident: Mutex::new(Some(buffers.clone())),
        })
    }

    pub(super) fn acquire(&self) -> Result<Arc<GeometryBuffers>, String> {
        let mut resident = self
            .resident
            .lock()
            .map_err(|_| "geometry.source_poisoned")?;
        if let Some(source) = &*resident {
            return Ok(source.clone());
        }
        let path = crate::runtime_package_path::safe_join_runtime_package(
            &self.package_dir,
            self.cooked.path.as_deref().ok_or("geometry.source_path")?,
        )
        .map_err(|e| e.to_string())?;
        let bytes = std::fs::read(path).map_err(|e| format!("geometry.source_read:{e}"))?;
        let digest = crate::canonical_digest::sha256_prefixed(&bytes);
        if self.record.hash.as_deref() != Some(&digest)
            || self.cooked.hash.as_deref() != Some(&digest)
            || self.record.size != Some(bytes.len() as u64)
            || self.cooked.size != Some(bytes.len() as u64)
            || self.record.bundle_id != self.cooked.bundle_id
        {
            return Err("geometry.source_identity_mismatch".into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("geometry.source_utf8:{e}"))?;
        let asset = RenderContract::<CookedRenderAsset>::decode(text)
            .map_err(|e| e.to_string())?
            .data;
        let RenderAssetData::Mesh(mesh) = asset.asset else {
            return Err("geometry.source_kind".into());
        };
        let source = Arc::new(GeometryBuffers::from_cooked(&mesh).map_err(|e| e.to_string())?);
        if !self.expected.same_identity(&source) {
            return Err("geometry.source_identity_mismatch".into());
        }
        *resident = Some(source.clone());
        Ok(source)
    }

    pub(super) fn release(&self) {
        if let Ok(mut resident) = self.resident.lock() {
            *resident = None;
        }
    }

    pub(super) fn observe_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        self.expected.observe_memory(sample, role);
        if let Ok(resident) = self.resident.lock() {
            if let Some(source) = &*resident {
                sample.cpu_arc(source, "assets", &[role]);
                source.observe_memory(sample, role);
            }
        }
        self.record.observe_memory(sample, "assets", role);
        self.cooked.observe_memory(sample, "assets", role);
        sample.cpu_storage(
            self.package_dir.as_os_str().as_encoded_bytes().as_ptr() as usize,
            self.package_dir.capacity(),
            "assets",
            &[role],
        );
    }
}
