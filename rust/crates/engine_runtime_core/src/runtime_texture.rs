use engine_render_resources::texture_mips as mips;
use engine_render_resources::texture_sampler as sampler;
pub use mips::{
    generate_rgba8_mip_chain, rgba8_mip_byte_len, rgba8_mip_layout, texture_block_layout,
    texture_mip_byte_len, texture_mip_layout, TextureMipLevel,
};
#[cfg_attr(test, path = "runtime_texture/verified.rs")]
mod verified;
use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
use crate::runtime_asset::{RuntimeAssetIndex, RuntimeAssetResolveError};
use crate::runtime_package::{CookedTextureAsset, RuntimeAssetRef, COOKED_TEXTURE_SCHEMA_VERSION};
use crate::runtime_package_source::RuntimePackageSource;
pub use sampler::{TextureFilter, TextureSampler, TextureWrap};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::fs;
use std::path::Path;
pub use verified::VerifiedTexturePayload;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTexturePayload {
    pub asset_id: String,
    pub cooked_asset_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub mip_count: u32,
    /// Tight GPU texel bytes (RGBA8 or declared block format), immutable across sampler/material variants. Explicit
    /// pixel edits use Arc::make_mut; Serde retains the existing byte-array wire.
    pub rgba8: std::sync::Arc<Vec<u8>>,
    pub sampler: String,
    pub source_hash: String,
}

impl RuntimeTexturePayload {
    pub(crate) fn observe_memory(
        &self,
        s: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        s.cpu_arc(&self.rgba8, "assets", &[role]);
        s.cpu_vec(&self.rgba8, "assets", &[role]);
        for value in [
            &self.asset_id,
            &self.cooked_asset_id,
            &self.format,
            &self.color_space,
            &self.sampler,
            &self.source_hash,
        ] {
            s.cpu_string(value, "assets", &[role]);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextureBinding {
    pub handle: RenderResourceHandle,
    pub sampler: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTextureBindingContext {
    bindings_by_asset_id: std::sync::Arc<BTreeMap<String, RuntimeTextureBinding>>,
}

impl RuntimeTextureBindingContext {
    pub fn insert(
        &mut self,
        asset_id: impl Into<String>,
        handle: RenderResourceHandle,
        sampler: impl Into<String>,
    ) {
        self.insert_with_extent(asset_id, handle, sampler, 1, 1);
    }

    pub fn insert_with_extent(
        &mut self,
        asset_id: impl Into<String>,
        handle: RenderResourceHandle,
        sampler: impl Into<String>,
        width: u32,
        height: u32,
    ) {
        std::sync::Arc::make_mut(&mut self.bindings_by_asset_id).insert(
            asset_id.into(),
            RuntimeTextureBinding {
                handle,
                sampler: sampler.into(),
                width: width.max(1),
                height: height.max(1),
            },
        );
    }

    pub fn get(&self, asset_id: &str) -> Option<&RuntimeTextureBinding> {
        self.bindings_by_asset_id.get(asset_id)
    }

    pub(crate) fn shares_storage(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.bindings_by_asset_id, &other.bindings_by_asset_id)
    }

    pub fn bindings(&self) -> impl Iterator<Item = (&str, &RuntimeTextureBinding)> {
        self.bindings_by_asset_id
            .iter()
            .map(|(asset_id, binding)| (asset_id.as_str(), binding))
    }

    pub fn len(&self) -> usize {
        self.bindings_by_asset_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings_by_asset_id.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextureUpload {
    pub asset_id: String,
    pub handle: RenderResourceHandle,
    pub payload: RuntimeTexturePayload,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeTextureUploadRegistry {
    uploads_by_asset_id: BTreeMap<String, RuntimeTextureUpload>,
    diagnostics: Vec<RuntimeTextureLoadError>,
}

impl RuntimeTextureUploadRegistry {
    pub fn load(
        package_dir: &Path,
        index: &RuntimeAssetIndex,
        asset_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut registry = Self::default();
        for asset_id in asset_ids.into_iter().collect::<BTreeSet<_>>() {
            let asset_ref = RuntimeAssetRef {
                id: asset_id.clone(),
                asset_type: "texture".to_string(),
                guid: None,
                sub_asset: None,
            };
            match load_runtime_texture_payload(package_dir, index, &asset_ref) {
                Ok(payload) => {
                    let handle = runtime_texture_render_handle(&asset_id);
                    registry.uploads_by_asset_id.insert(
                        asset_id.clone(),
                        RuntimeTextureUpload {
                            asset_id,
                            handle,
                            payload,
                        },
                    );
                }
                Err(error) => registry.diagnostics.push(error),
            }
        }
        registry
    }

    pub fn uploads(&self) -> impl Iterator<Item = &RuntimeTextureUpload> {
        self.uploads_by_asset_id.values()
    }

    pub fn diagnostics(&self) -> &[RuntimeTextureLoadError] {
        &self.diagnostics
    }

    pub fn binding_context(&self) -> RuntimeTextureBindingContext {
        let mut context = RuntimeTextureBindingContext::default();
        for upload in self.uploads() {
            context.insert_with_extent(
                upload.asset_id.clone(),
                upload.handle,
                upload.payload.sampler.clone(),
                upload.payload.width,
                upload.payload.height,
            );
        }
        context
    }
}

pub fn runtime_texture_render_handle(asset_id: &str) -> RenderResourceHandle {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in b"runtime-texture:".iter().chain(asset_id.as_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    RenderResourceHandle {
        kind: RenderResourceKind::Texture,
        index: hash | (1u64 << 63),
        generation: 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeTextureLoadErrorCode {
    MissingRuntimeAsset,
    TypeMismatch,
    MissingCookedTextureMetadata,
    MetadataReadFailed,
    MetadataParseFailed,
    SchemaMismatch,
    MissingPixelPayload,
    ByteLengthMismatch,
    UnsupportedFormat,
    UnsafePackagePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTextureLoadError {
    pub code: RuntimeTextureLoadErrorCode,
    pub asset_ref_id: String,
    pub cooked_asset_id: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

impl RuntimeTextureLoadError {
    fn new(
        code: RuntimeTextureLoadErrorCode,
        asset_ref_id: impl Into<String>,
        cooked_asset_id: Option<String>,
        path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            asset_ref_id: asset_ref_id.into(),
            cooked_asset_id,
            path,
            message: message.into(),
        }
    }
}

/// Cold reads used after CPU eviction must reproduce the original package
/// receipt. Legacy sources without a complete receipt keep their old semantics.
pub(crate) fn load_checked_runtime_texture_payload(
    package_dir: &Path,
    index: &RuntimeAssetIndex,
    asset_ref: &RuntimeAssetRef,
) -> Result<RuntimeTexturePayload, String> {
    load_checked_runtime_texture_payload_from_source(
        &RuntimePackageSource::directory(package_dir),
        index,
        asset_ref,
    )
}

pub(crate) fn load_checked_runtime_texture_payload_from_source(
    source: &RuntimePackageSource,
    index: &RuntimeAssetIndex,
    asset_ref: &RuntimeAssetRef,
) -> Result<RuntimeTexturePayload, String> {
    use sha2::{Digest, Sha256};
    let payload = load_runtime_texture_payload_from_source(source, index, asset_ref)
        .map_err(|e| format!("texture.source:{e:?}"))?;
    let record = index.resolve(asset_ref).map_err(|e| format!("{e:?}"))?;
    let cooked = index
        .cooked_asset(&record.cooked_asset_id)
        .ok_or("texture.source_record")?;
    if record.hash.is_some()
        && cooked.hash.is_some()
        && record.size.is_some()
        && cooked.size.is_some()
    {
        let path = source
            .resolve(cooked.path.as_deref().ok_or("texture.source_path")?)
            .map_err(|e| format!("texture.source_path:{e}"))?;
        let metadata = source
            .read_path(&path)
            .map_err(|e| format!("texture.source_metadata:{e}"))?;
        let mut hash = Sha256::new();
        hash.update(&metadata);
        hash.update(payload.rgba8.as_slice());
        let digest = format!("sha256:{:x}", hash.finalize());
        if record.hash.as_deref() != Some(&digest)
            || cooked.hash.as_deref() != Some(&digest)
            || record.size != Some(metadata.len() as u64)
            || cooked.size != Some(metadata.len() as u64)
            || record.bundle_id != cooked.bundle_id
        {
            return Err("texture.source_identity_mismatch".into());
        }
    }
    Ok(payload)
}

pub fn load_runtime_texture_payload(
    package_dir: &Path,
    index: &RuntimeAssetIndex,
    asset_ref: &RuntimeAssetRef,
) -> Result<RuntimeTexturePayload, RuntimeTextureLoadError> {
    load_runtime_texture_payload_from_source(
        &RuntimePackageSource::directory(package_dir),
        index,
        asset_ref,
    )
}

pub fn load_runtime_texture_payload_from_source(
    source: &RuntimePackageSource,
    index: &RuntimeAssetIndex,
    asset_ref: &RuntimeAssetRef,
) -> Result<RuntimeTexturePayload, RuntimeTextureLoadError> {
    let record = index.resolve(asset_ref).map_err(|error| match error {
        RuntimeAssetResolveError::MissingAssetRef | RuntimeAssetResolveError::SubAssetMissing => {
            RuntimeTextureLoadError::new(
                RuntimeTextureLoadErrorCode::MissingRuntimeAsset,
                asset_ref.id.clone(),
                None,
                None,
                "Texture AssetRef could not resolve through RuntimeAssetIndex.",
            )
        }
        RuntimeAssetResolveError::TypeMismatch { expected, actual } => {
            RuntimeTextureLoadError::new(
                RuntimeTextureLoadErrorCode::TypeMismatch,
                asset_ref.id.clone(),
                None,
                None,
                format!("Texture AssetRef expected type {expected}, got {actual}."),
            )
        }
    })?;

    if record.asset_type != "texture" && record.loader_kind != "texture" {
        return Err(RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::TypeMismatch,
            record.asset_id.clone(),
            Some(record.cooked_asset_id.clone()),
            None,
            "Runtime asset is not a texture.",
        ));
    }

    let cooked = index.cooked_asset(&record.cooked_asset_id).ok_or_else(|| {
        RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::MissingCookedTextureMetadata,
            record.asset_id.clone(),
            Some(record.cooked_asset_id.clone()),
            None,
            "RuntimeAssetIndex points to a missing cooked texture metadata entry.",
        )
    })?;
    let Some(metadata_path) = cooked.path.as_ref() else {
        return Err(RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::MissingCookedTextureMetadata,
            record.asset_id.clone(),
            Some(record.cooked_asset_id.clone()),
            None,
            "Cooked texture entry has no metadata path.",
        ));
    };
    let metadata_abs = source.resolve(metadata_path).map_err(|error| {
        RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::UnsafePackagePath,
            record.asset_id.clone(),
            Some(record.cooked_asset_id.clone()),
            Some(metadata_path.clone()),
            error.to_string(),
        )
    })?;
    let metadata_text = source.read_string_path(&metadata_abs).map_err(|error| {
        RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::MetadataReadFailed,
            record.asset_id.clone(),
            Some(record.cooked_asset_id.clone()),
            Some(metadata_path.clone()),
            format!("Failed to read cooked texture metadata: {error}"),
        )
    })?;
    let metadata = serde_json::from_str::<CookedTextureAsset>(&metadata_text).map_err(|error| {
        RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::MetadataParseFailed,
            record.asset_id.clone(),
            Some(record.cooked_asset_id.clone()),
            Some(metadata_path.clone()),
            format!("Failed to parse cooked texture metadata: {error}"),
        )
    })?;
    validate_metadata(
        &metadata,
        record.asset_id.as_str(),
        record.cooked_asset_id.as_str(),
    )?;

    let payload_path = metadata.pixel_data_path.clone();
    let payload_abs = source.resolve(&payload_path).map_err(|error| {
        RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::UnsafePackagePath,
            metadata.asset_id.clone(),
            Some(metadata.cooked_asset_id.clone()),
            Some(payload_path.clone()),
            error.to_string(),
        )
    })?;
    let payload = source.read_path(&payload_abs).map_err(|error| {
        RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::MissingPixelPayload,
            metadata.asset_id.clone(),
            Some(metadata.cooked_asset_id.clone()),
            Some(payload_path.clone()),
            format!("Failed to read cooked texture pixel payload: {error}"),
        )
    })?;
    let expected_len = texture_mip_byte_len(
        &metadata.format,
        metadata.width,
        metadata.height,
        metadata.mip_count,
    )
    .expect("validated mip metadata");
    if metadata.byte_length != expected_len || payload.len() != expected_len {
        return Err(RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::ByteLengthMismatch,
            metadata.asset_id.clone(),
            Some(metadata.cooked_asset_id.clone()),
            Some(payload_path),
            format!(
                "Cooked texture byte length mismatch: metadata={}, payload={}, expected={}.",
                metadata.byte_length,
                payload.len(),
                expected_len
            ),
        ));
    }

    Ok(RuntimeTexturePayload {
        asset_id: metadata.asset_id,
        cooked_asset_id: metadata.cooked_asset_id,
        width: metadata.width,
        height: metadata.height,
        format: metadata.format,
        color_space: metadata.color_space,
        mip_count: metadata.mip_count,
        rgba8: payload.into(),
        sampler: metadata.sampler,
        source_hash: metadata.source_hash,
    })
}

pub(crate) fn validate_metadata(
    metadata: &CookedTextureAsset,
    asset_id: &str,
    cooked_asset_id: &str,
) -> Result<(), RuntimeTextureLoadError> {
    if metadata.schema_version != COOKED_TEXTURE_SCHEMA_VERSION {
        return Err(RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::SchemaMismatch,
            asset_id,
            Some(cooked_asset_id.to_string()),
            None,
            format!(
                "Cooked texture schemaVersion '{}' must be '{}'.",
                metadata.schema_version, COOKED_TEXTURE_SCHEMA_VERSION
            ),
        ));
    }
    if metadata.asset_id != asset_id || metadata.cooked_asset_id != cooked_asset_id {
        return Err(RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::MetadataParseFailed,
            asset_id,
            Some(cooked_asset_id.to_string()),
            None,
            "Cooked texture metadata id does not match RuntimeAssetIndex entry.",
        ));
    }
    if texture_block_layout(&metadata.format).is_none() {
        return Err(RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::UnsupportedFormat,
            asset_id,
            Some(cooked_asset_id.to_string()),
            None,
            format!(
                "Unsupported C-min cooked texture format: {}",
                metadata.format
            ),
        ));
    }
    let compressed = metadata.format.starts_with("bc3");
    if texture_mip_layout(
        &metadata.format,
        metadata.width,
        metadata.height,
        metadata.mip_count,
    )
    .is_none()
        || (compressed
            && (metadata.width % 4 != 0
                || metadata.height % 4 != 0
                || metadata.mip_count != 32 - metadata.width.max(metadata.height).leading_zeros()
                || metadata.color_space
                    != if metadata.format.ends_with("Srgb") {
                        "srgb"
                    } else {
                        "linear"
                    }))
    {
        return Err(RuntimeTextureLoadError::new(
            RuntimeTextureLoadErrorCode::UnsupportedFormat,
            asset_id,
            Some(cooked_asset_id.to_string()),
            None,
            "Textures require non-zero RGBA8 extents and a valid mipCount.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_asset::{CookedAssetRecord, RuntimeAssetRecord};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn bc3_metadata_admits_full_block_mips_and_rejects_unknown_formats() {
        let mut metadata = CookedTextureAsset {
            schema_version: COOKED_TEXTURE_SCHEMA_VERSION.into(),
            asset_id: "bc3".into(),
            cooked_asset_id: "cooked-bc3".into(),
            source_hash: "hash".into(),
            width: 4,
            height: 4,
            format: "bc3RgbaUnormSrgb".into(),
            color_space: "srgb".into(),
            mip_count: 3,
            byte_length: 48,
            pixel_data_path: "cooked/textures/bc3.blocks".into(),
            sampler: "linearMipmapLinear:linear:repeat:repeat".into(),
        };
        assert!(validate_metadata(&metadata, "bc3", "cooked-bc3").is_ok());
        metadata.format = "bc7RgbaUnorm".into();
        assert_eq!(
            validate_metadata(&metadata, "bc3", "cooked-bc3")
                .unwrap_err()
                .code,
            RuntimeTextureLoadErrorCode::UnsupportedFormat
        );
    }

    #[test]
    fn runtime_texture_render_handles_are_stable_and_asset_scoped() {
        assert_eq!(
            runtime_texture_render_handle("texture-a"),
            runtime_texture_render_handle("texture-a")
        );
        assert_ne!(
            runtime_texture_render_handle("texture-a"),
            runtime_texture_render_handle("texture-b")
        );
    }

    #[test]
    fn runtime_texture_registry_preserves_missing_asset_diagnostic() {
        let root = temp_dir("runtime-texture-registry-missing");
        let registry = RuntimeTextureUploadRegistry::load(
            &root,
            &RuntimeAssetIndex::new(Vec::new(), Vec::new()),
            vec!["texture-missing".to_string()],
        );

        assert!(registry.uploads().next().is_none());
        assert!(registry.binding_context().is_empty());
        assert_eq!(registry.diagnostics().len(), 1);
        assert_eq!(
            registry.diagnostics()[0].code,
            RuntimeTextureLoadErrorCode::MissingRuntimeAsset
        );
        assert_eq!(registry.diagnostics()[0].asset_ref_id, "texture-missing");
    }

    #[test]
    fn runtime_texture_payload_loads_cooked_rgba8() {
        let root = temp_dir("runtime-texture-loads");
        fs::create_dir_all(root.join("cooked/textures")).unwrap();
        let metadata = CookedTextureAsset {
            schema_version: COOKED_TEXTURE_SCHEMA_VERSION.to_string(),
            asset_id: "tex-main".to_string(),
            cooked_asset_id: "cooked-tex-main".to_string(),
            source_hash: "hash-main".to_string(),
            width: 1,
            height: 1,
            format: "rgba8UnormSrgb".to_string(),
            color_space: "srgb".to_string(),
            mip_count: 1,
            byte_length: 4,
            pixel_data_path: "cooked/textures/tex-main.rgba8".to_string(),
            sampler: "linearClamp".to_string(),
        };
        fs::write(
            root.join("cooked/textures/tex-main.texture.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("cooked/textures/tex-main.rgba8"),
            [255, 0, 0, 255],
        )
        .unwrap();
        let index = RuntimeAssetIndex::new(
            vec![record("tex-main", "cooked-tex-main")],
            vec![CookedAssetRecord {
                cooked_asset_id: "cooked-tex-main".to_string(),
                bundle_id: "startup".to_string(),
                path: Some("cooked/textures/tex-main.texture.json".to_string()),
                offset: None,
                size: Some(4),
                compression: Some("none".to_string()),
                hash: Some("hash-main".to_string()),
            }],
        );

        let payload = load_runtime_texture_payload(
            &root,
            &index,
            &RuntimeAssetRef {
                id: "tex-main".to_string(),
                asset_type: "texture".to_string(),
                guid: None,
                sub_asset: None,
            },
        )
        .unwrap();

        assert_eq!(payload.width, 1);
        assert_eq!(payload.height, 1);
        assert_eq!(payload.rgba8.as_slice(), &[255, 0, 0, 255]);
    }

    #[test]
    fn runtime_texture_payload_reports_missing_pixel_payload() {
        let root = temp_dir("runtime-texture-missing-payload");
        fs::create_dir_all(root.join("cooked/textures")).unwrap();
        let metadata = CookedTextureAsset {
            schema_version: COOKED_TEXTURE_SCHEMA_VERSION.to_string(),
            asset_id: "tex-main".to_string(),
            cooked_asset_id: "cooked-tex-main".to_string(),
            source_hash: "hash-main".to_string(),
            width: 1,
            height: 1,
            format: "rgba8UnormSrgb".to_string(),
            color_space: "srgb".to_string(),
            mip_count: 1,
            byte_length: 4,
            pixel_data_path: "cooked/textures/tex-main.rgba8".to_string(),
            sampler: "linearClamp".to_string(),
        };
        fs::write(
            root.join("cooked/textures/tex-main.texture.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        let index = RuntimeAssetIndex::new(
            vec![record("tex-main", "cooked-tex-main")],
            vec![CookedAssetRecord {
                cooked_asset_id: "cooked-tex-main".to_string(),
                bundle_id: "startup".to_string(),
                path: Some("cooked/textures/tex-main.texture.json".to_string()),
                offset: None,
                size: Some(4),
                compression: Some("none".to_string()),
                hash: Some("hash-main".to_string()),
            }],
        );

        let error = load_runtime_texture_payload(
            &root,
            &index,
            &RuntimeAssetRef {
                id: "tex-main".to_string(),
                asset_type: "texture".to_string(),
                guid: None,
                sub_asset: None,
            },
        )
        .unwrap_err();

        assert_eq!(error.code, RuntimeTextureLoadErrorCode::MissingPixelPayload);
        assert_eq!(
            error.path.as_deref(),
            Some("cooked/textures/tex-main.rgba8")
        );
    }

    fn record(asset_id: &str, cooked_asset_id: &str) -> RuntimeAssetRecord {
        RuntimeAssetRecord {
            asset_guid: asset_id.to_string(),
            asset_id: asset_id.to_string(),
            asset_type: "texture".to_string(),
            sub_asset_id: None,
            version: "1".to_string(),
            cooked_asset_id: cooked_asset_id.to_string(),
            bundle_id: "startup".to_string(),
            loader_kind: "texture".to_string(),
            dependencies: Vec::new(),
            hash: None,
            size: Some(4),
            flags: Vec::new(),
            source_map_debug: None,
        }
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("aife-{label}-{nanos}"));
        path
    }
}
