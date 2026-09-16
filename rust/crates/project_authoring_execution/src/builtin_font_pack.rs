use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::font_bundle::{
    RuntimeFontBundleLoader, RuntimePackageSourceFontBundle, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

pub const ENGINE_BUILT_IN_FONT_PACK_ID: &str = "aife-default-zh-cn-common-v1";
pub const ENGINE_BUILT_IN_FONT_PACK_MANIFEST_SCHEMA_VERSION: &str =
    "engine-built-in-font-pack-manifest.v1";

mod embedded {
    include!(concat!(
        env!("OUT_DIR"),
        "/engine_builtin_font_pack_embedded.rs"
    ));
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineBuiltInFontPackManifest {
    pub schema_version: String,
    pub pack_id: String,
    pub source_sha256: String,
    pub glyph_set_digest: String,
    pub recipe_version: String,
    pub bundle_schema_version: String,
    pub bundle_digest: String,
    pub bundle_metadata_path: String,
    pub bundle_metadata_sha256: String,
    pub page_paths: Vec<String>,
    pub page_sha256: Vec<String>,
    pub codepoint_count: usize,
    pub han_codepoint_count: usize,
    pub bitmap_variant_count: usize,
    pub msdf_variant_count: usize,
    pub bitmap_page_count: usize,
    pub msdf_page_count: usize,
    pub raw_page_bytes: usize,
    pub maximum_raw_bundle_bytes: usize,
    pub replacement_alias_from: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineBuiltInFontPackError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for EngineBuiltInFontPackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EngineBuiltInFontPackError {}

pub fn load_engine_builtin_font_pack(
) -> Result<RuntimePackageSourceFontBundle, EngineBuiltInFontPackError> {
    if embedded::BUILTIN_FONT_PACK_MANIFEST_BYTES.is_empty() {
        return Err(error(
            "EngineBuiltInFontPackMissing",
            "sealed built-in FontPack was not present when project_authoring_execution was compiled",
        ));
    }
    let manifest: EngineBuiltInFontPackManifest =
        serde_json::from_slice(embedded::BUILTIN_FONT_PACK_MANIFEST_BYTES)
            .map_err(|cause| error("EngineBuiltInFontPackManifestInvalid", cause.to_string()))?;
    let metadata = serde_json::from_slice(embedded::BUILTIN_FONT_BUNDLE_BYTES)
        .map_err(|cause| error("EngineBuiltInFontPackMetadataInvalid", cause.to_string()))?;
    let bundle = RuntimePackageSourceFontBundle {
        metadata,
        page_payloads: embedded::BUILTIN_FONT_PAGE_BYTES
            .iter()
            .map(|bytes| bytes.to_vec())
            .collect(),
        font_face_sources: Vec::new(),
    };
    validate_manifest(&manifest, embedded::BUILTIN_FONT_BUNDLE_BYTES, &bundle)?;
    RuntimeFontBundleLoader::load(bundle.clone())
        .map_err(|cause| error("EngineBuiltInFontPackBundleInvalid", format!("{cause:?}")))?;
    Ok(bundle)
}

fn validate_manifest(
    manifest: &EngineBuiltInFontPackManifest,
    metadata_bytes: &[u8],
    bundle: &RuntimePackageSourceFontBundle,
) -> Result<(), EngineBuiltInFontPackError> {
    if manifest.schema_version != ENGINE_BUILT_IN_FONT_PACK_MANIFEST_SCHEMA_VERSION
        || manifest.pack_id != ENGINE_BUILT_IN_FONT_PACK_ID
        || manifest.bundle_schema_version != COOKED_FONT_BUNDLE_SCHEMA_VERSION
        || manifest.bundle_digest != bundle.metadata.bundle_digest
        || manifest.bundle_metadata_sha256 != sha256_prefixed(metadata_bytes)
        || manifest.page_paths.len() != bundle.page_payloads.len()
        || manifest.page_sha256.len() != bundle.page_payloads.len()
        || manifest.raw_page_bytes != bundle.page_payloads.iter().map(Vec::len).sum::<usize>()
        || manifest.raw_page_bytes > manifest.maximum_raw_bundle_bytes
    {
        return Err(error(
            "EngineBuiltInFontPackManifestMismatch",
            "sealed manifest does not match embedded metadata or budget",
        ));
    }
    for ((expected, payload), page) in manifest
        .page_sha256
        .iter()
        .zip(&bundle.page_payloads)
        .zip(&bundle.metadata.pages)
    {
        if expected != &sha256_prefixed(payload) || expected != &page.sha256 {
            return Err(error(
                "EngineBuiltInFontPackPageDigestMismatch",
                format!("page {} digest mismatch", page.page_index),
            ));
        }
    }
    Ok(())
}

fn error(code: &'static str, message: impl Into<String>) -> EngineBuiltInFontPackError {
    EngineBuiltInFontPackError {
        code,
        message: message.into(),
    }
}
