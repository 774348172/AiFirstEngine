use crate::{
    FontAtlasProfileAsset, FontAtlasProfileRole, FontFaceAsset, FontFaceDeclaredMetadata,
    FontFaceSource, FontFamilyAsset, FontFamilyFace, FontGlyphSet, FontHintingMode,
    FontMissingGlyphPolicy, FontMissingStylePolicy, FontPackingProfile, FontRasterPolicy,
    FontRasterProfile, FontSourceKind, FontStackAsset, FontStyle, ProjectFontAssetSet,
    ProjectFontBundleBuilder, ProjectFontCookModule, ProjectFontCookRequest,
    FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION, FONT_FACE_ASSET_SCHEMA_VERSION,
    FONT_FAMILY_ASSET_SCHEMA_VERSION, FONT_STACK_ASSET_SCHEMA_VERSION, PROJECT_FONT_RECIPE_VERSION,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::font_bundle::{
    RuntimeFontBundleLoader, RuntimePackageSourceFontBundle, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const ENGINE_BUILT_IN_FONT_PACK_ID: &str = "aife-default-zh-cn-common-v1";
pub const ENGINE_DEFAULT_GLYPH_SET_SPEC_SCHEMA_VERSION: &str = "engine-default-glyph-set-spec.v1";
pub const ENGINE_DEFAULT_GLYPH_SET_LOCK_SCHEMA_VERSION: &str = "engine-default-glyph-set-lock.v1";
pub const ENGINE_BUILT_IN_FONT_PACK_MANIFEST_SCHEMA_VERSION: &str =
    "engine-built-in-font-pack-manifest.v1";
const FONT_SOURCE_ID: &str = "engine-font-source-noto-sans-sc-regular";
const FONT_FACE_ID: &str = "engine-font-face-noto-sans-sc-regular";
const FONT_FAMILY_ID: &str = "engine-font-family-default-zh-cn";
const FONT_STACK_ID: &str = "engine-font-stack-default-zh-cn";
const FONT_PROFILE_ID: &str = ENGINE_BUILT_IN_FONT_PACK_ID;

mod embedded {
    include!(concat!(
        env!("OUT_DIR"),
        "/engine_builtin_font_pack_embedded.rs"
    ));
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineDefaultGlyphSetSpec {
    pub schema_version: String,
    pub glyph_set_id: String,
    pub locale: String,
    pub include_ascii_printable: bool,
    pub punctuation: String,
    pub ui_symbols: String,
    pub replacement_codepoint: String,
    pub ranked_corpus_path: String,
    pub ranked_han_limit: usize,
    pub catalogs: Vec<EngineDefaultGlyphCatalogSpec>,
    pub minimum_han_codepoints: usize,
    pub maximum_han_codepoints: usize,
    pub maximum_total_codepoints: usize,
    pub maximum_raw_bundle_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineDefaultGlyphCatalogSpec {
    pub source_tag: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineDefaultGlyphSetEntry {
    pub codepoint: u32,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineDefaultGlyphSetLock {
    pub schema_version: String,
    pub glyph_set_id: String,
    pub entries: Vec<EngineDefaultGlyphSetEntry>,
    pub han_codepoint_count: usize,
    pub total_codepoint_count: usize,
    pub digest: String,
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

pub struct EngineBuiltInFontPack;

impl EngineBuiltInFontPack {
    pub fn resolve_glyph_set(
        pack_root: &Path,
    ) -> Result<EngineDefaultGlyphSetLock, EngineBuiltInFontPackError> {
        let spec_path = pack_root.join("glyph-set/spec.json");
        let spec: EngineDefaultGlyphSetSpec =
            read_json(&spec_path, "EngineDefaultGlyphSetSpecInvalid")?;
        validate_spec(&spec)?;
        let mut entries = BTreeMap::<u32, BTreeSet<String>>::new();
        if spec.include_ascii_printable {
            for codepoint in 0x20..=0x7e {
                add_source(&mut entries, codepoint, "asciiPrintable");
            }
        }
        for character in spec.punctuation.chars() {
            add_source(&mut entries, character.into(), "zhCnPunctuation");
        }
        for character in spec.ui_symbols.chars() {
            add_source(&mut entries, character.into(), "uiSymbols");
        }
        let replacement = parse_codepoint(&spec.replacement_codepoint)?;
        add_source(&mut entries, replacement, "replacement");

        for catalog in &spec.catalogs {
            if catalog.source_tag.trim().is_empty() {
                return Err(error(
                    "EngineDefaultGlyphSetSourceInvalid",
                    "catalog sourceTag is empty",
                ));
            }
            let value: serde_json::Value = read_json(
                &pack_root.join(&catalog.path),
                "EngineDefaultGlyphSetCatalogInvalid",
            )?;
            let mut strings = Vec::new();
            collect_json_strings(&value, &mut strings);
            for value in strings {
                for character in value.chars() {
                    add_source(&mut entries, character.into(), &catalog.source_tag);
                }
            }
        }

        let ranked_path = pack_root.join(&spec.ranked_corpus_path);
        let ranked = fs::read_to_string(&ranked_path).map_err(|cause| {
            error(
                "EngineDefaultGlyphSetCorpusInvalid",
                format!("{}: {cause}", ranked_path.display()),
            )
        })?;
        let mut ranked_count = 0usize;
        for (line_index, line) in ranked.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let token = line.split('\t').next().unwrap_or_default();
            let codepoint = parse_codepoint(token).map_err(|_| {
                error(
                    "EngineDefaultGlyphSetCorpusInvalid",
                    format!("line {} has invalid codepoint {token}", line_index + 1),
                )
            })?;
            if !is_han(codepoint) {
                return Err(error(
                    "EngineDefaultGlyphSetCorpusInvalid",
                    format!("line {} is not a Han scalar", line_index + 1),
                ));
            }
            if ranked_count < spec.ranked_han_limit {
                add_source(&mut entries, codepoint, "rankedHan");
                ranked_count += 1;
            }
        }
        if ranked_count != spec.ranked_han_limit {
            return Err(error(
                "EngineDefaultGlyphSetCorpusInvalid",
                format!(
                    "expected {} ranked Han entries, found {ranked_count}",
                    spec.ranked_han_limit
                ),
            ));
        }

        let resolved_entries = entries
            .into_iter()
            .map(|(codepoint, sources)| EngineDefaultGlyphSetEntry {
                codepoint,
                sources: sources.into_iter().collect(),
            })
            .collect::<Vec<_>>();
        let han_codepoint_count = resolved_entries
            .iter()
            .filter(|entry| is_han(entry.codepoint))
            .count();
        let total_codepoint_count = resolved_entries.len();
        if !(spec.minimum_han_codepoints..=spec.maximum_han_codepoints)
            .contains(&han_codepoint_count)
        {
            return Err(error(
                "EngineDefaultGlyphSetBudgetExceeded",
                format!(
                    "Han count {han_codepoint_count} is outside {}..={}",
                    spec.minimum_han_codepoints, spec.maximum_han_codepoints
                ),
            ));
        }
        if total_codepoint_count > spec.maximum_total_codepoints {
            return Err(error(
                "EngineDefaultGlyphSetBudgetExceeded",
                format!(
                    "total count {total_codepoint_count} exceeds {}",
                    spec.maximum_total_codepoints
                ),
            ));
        }
        let mut lock = EngineDefaultGlyphSetLock {
            schema_version: ENGINE_DEFAULT_GLYPH_SET_LOCK_SCHEMA_VERSION.to_string(),
            glyph_set_id: spec.glyph_set_id,
            entries: resolved_entries,
            han_codepoint_count,
            total_codepoint_count,
            digest: String::new(),
        };
        lock.digest = glyph_lock_digest(&lock)?;
        Ok(lock)
    }

    pub fn validate_glyph_set_lock(
        lock: &EngineDefaultGlyphSetLock,
    ) -> Result<(), EngineBuiltInFontPackError> {
        if lock.schema_version != ENGINE_DEFAULT_GLYPH_SET_LOCK_SCHEMA_VERSION
            || lock.total_codepoint_count != lock.entries.len()
            || lock
                .entries
                .windows(2)
                .any(|pair| pair[0].codepoint >= pair[1].codepoint)
            || lock.entries.iter().any(|entry| {
                entry.sources.is_empty() || entry.sources.windows(2).any(|pair| pair[0] >= pair[1])
            })
            || lock.han_codepoint_count
                != lock
                    .entries
                    .iter()
                    .filter(|entry| is_han(entry.codepoint))
                    .count()
        {
            return Err(error(
                "EngineDefaultGlyphSetLockInvalid",
                "glyph lock structure is not canonical",
            ));
        }
        if glyph_lock_digest(lock)? != lock.digest {
            return Err(error(
                "EngineDefaultGlyphSetLockDigestMismatch",
                "glyph lock digest does not match its entries",
            ));
        }
        Ok(())
    }

    pub fn produce(
        pack_root: &Path,
    ) -> Result<EngineBuiltInFontPackManifest, EngineBuiltInFontPackError> {
        let pack_root = fs::canonicalize(pack_root).map_err(|cause| {
            error(
                "EngineBuiltInFontPackSourceInvalid",
                format!("{}: {cause}", pack_root.display()),
            )
        })?;
        let lock = Self::resolve_glyph_set(&pack_root)?;
        Self::validate_glyph_set_lock(&lock)?;
        write_json(&pack_root.join("glyph-set/lock.json"), &lock)?;

        let source_path = fs::canonicalize(pack_root.join("source/NotoSansSC-Regular.ttf"))
            .map_err(|cause| error("EngineBuiltInFontPackSourceInvalid", cause.to_string()))?;
        let source_bytes = fs::read(&source_path).map_err(|cause| {
            error(
                "EngineBuiltInFontPackSourceInvalid",
                format!("{}: {cause}", source_path.display()),
            )
        })?;
        let source_sha256 = sha256_prefixed(&source_bytes);
        let expected_source_sha256 =
            "sha256:d45f67f0a7c0ca3f256950777ce6a61cc7ce5f9696d02900cbbaac25f8aa7d16";
        if source_bytes.len() != 10_559_284 || source_sha256 != expected_source_sha256 {
            return Err(error(
                "EngineBuiltInFontPackSourceInvalid",
                "Noto Sans SC source identity does not match provenance",
            ));
        }

        let profile = built_in_profile(&lock);
        let mut source_paths = BTreeMap::new();
        source_paths.insert(FONT_SOURCE_ID.to_string(), source_path);
        let mut output = ProjectFontCookModule::cook(ProjectFontCookRequest {
            project_root: pack_root.clone(),
            assets: built_in_assets(profile.clone(), &source_sha256),
            source_paths,
            aui_documents: Vec::new(),
            localization_texts: Vec::new(),
            text_sources: Vec::new(),
            profile_id: FONT_PROFILE_ID.to_string(),
        })
        .map_err(|failure| {
            error(
                "EngineBuiltInFontPackCookFailed",
                failure
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        add_replacement_alias(&mut output)?;
        let bundle =
            ProjectFontBundleBuilder::build_bitmap_v2(&profile, &output).map_err(|failure| {
                error(
                    "EngineBuiltInFontPackBuildFailed",
                    failure
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            })?;

        let cooked_root = pack_root.join("cooked");
        fs::create_dir_all(&cooked_root)
            .map_err(|cause| error("EngineBuiltInFontPackWriteFailed", cause.to_string()))?;
        let metadata_path = "font-bundle.json".to_string();
        let metadata_bytes = pretty_json_bytes(&bundle.metadata)?;
        fs::write(cooked_root.join(&metadata_path), &metadata_bytes)
            .map_err(|cause| error("EngineBuiltInFontPackWriteFailed", cause.to_string()))?;
        let mut page_paths = Vec::new();
        let mut page_sha256 = Vec::new();
        for (page, payload) in bundle.metadata.pages.iter().zip(&bundle.page_payloads) {
            let extension = if page.format == "r8Unorm" {
                "r8"
            } else {
                "rgba8"
            };
            let relative = format!("page-{:03}.{extension}", page.page_index);
            fs::write(cooked_root.join(&relative), payload)
                .map_err(|cause| error("EngineBuiltInFontPackWriteFailed", cause.to_string()))?;
            page_paths.push(relative);
            page_sha256.push(sha256_prefixed(payload));
        }
        let raw_page_bytes = bundle.page_payloads.iter().map(Vec::len).sum::<usize>();
        let spec: EngineDefaultGlyphSetSpec = read_json(
            &pack_root.join("glyph-set/spec.json"),
            "EngineDefaultGlyphSetSpecInvalid",
        )?;
        if raw_page_bytes > spec.maximum_raw_bundle_bytes {
            return Err(error(
                "EngineBuiltInFontPackBudgetExceeded",
                format!(
                    "raw pages use {raw_page_bytes} bytes, budget is {}",
                    spec.maximum_raw_bundle_bytes
                ),
            ));
        }
        let manifest = EngineBuiltInFontPackManifest {
            schema_version: ENGINE_BUILT_IN_FONT_PACK_MANIFEST_SCHEMA_VERSION.to_string(),
            pack_id: ENGINE_BUILT_IN_FONT_PACK_ID.to_string(),
            source_sha256,
            glyph_set_digest: lock.digest,
            recipe_version: PROJECT_FONT_RECIPE_VERSION.to_string(),
            bundle_schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
            bundle_digest: bundle.metadata.bundle_digest.clone(),
            bundle_metadata_path: metadata_path,
            bundle_metadata_sha256: sha256_prefixed(&metadata_bytes),
            page_paths,
            page_sha256,
            codepoint_count: lock.total_codepoint_count,
            han_codepoint_count: lock.han_codepoint_count,
            bitmap_variant_count: output.hinted_variants.len(),
            msdf_variant_count: output.msdf_variants.len(),
            bitmap_page_count: bundle
                .metadata
                .pages
                .iter()
                .filter(|page| page.format == "r8Unorm")
                .count(),
            msdf_page_count: bundle
                .metadata
                .pages
                .iter()
                .filter(|page| page.format == "rgba8Unorm")
                .count(),
            raw_page_bytes,
            maximum_raw_bundle_bytes: spec.maximum_raw_bundle_bytes,
            replacement_alias_from: "U+25A1".to_string(),
        };
        write_json(&cooked_root.join("manifest.json"), &manifest)?;
        Ok(manifest)
    }

    pub fn load_embedded() -> Result<RuntimePackageSourceFontBundle, EngineBuiltInFontPackError> {
        if embedded::BUILTIN_FONT_PACK_MANIFEST_BYTES.is_empty() {
            return Err(error(
                "EngineBuiltInFontPackMissing",
                "sealed built-in FontPack was not present when editor_core was compiled",
            ));
        }
        let manifest: EngineBuiltInFontPackManifest = serde_json::from_slice(
            embedded::BUILTIN_FONT_PACK_MANIFEST_BYTES,
        )
        .map_err(|cause| error("EngineBuiltInFontPackManifestInvalid", cause.to_string()))?;
        let metadata = serde_json::from_slice(embedded::BUILTIN_FONT_BUNDLE_BYTES)
            .map_err(|cause| error("EngineBuiltInFontPackMetadataInvalid", cause.to_string()))?;
        let bundle = RuntimePackageSourceFontBundle {
            metadata,
            page_payloads: embedded::BUILTIN_FONT_PAGE_BYTES
                .iter()
                .map(|bytes| bytes.to_vec())
                .collect(),
        };
        validate_manifest(&manifest, embedded::BUILTIN_FONT_BUNDLE_BYTES, &bundle)?;
        RuntimeFontBundleLoader::load(bundle.clone())
            .map_err(|cause| error("EngineBuiltInFontPackBundleInvalid", format!("{cause:?}")))?;
        Ok(bundle)
    }
}

fn validate_spec(spec: &EngineDefaultGlyphSetSpec) -> Result<(), EngineBuiltInFontPackError> {
    if spec.schema_version != ENGINE_DEFAULT_GLYPH_SET_SPEC_SCHEMA_VERSION
        || spec.glyph_set_id.trim().is_empty()
        || spec.locale != "zh-CN"
        || spec.ranked_han_limit == 0
        || spec.minimum_han_codepoints > spec.maximum_han_codepoints
        || spec.maximum_han_codepoints > spec.maximum_total_codepoints
        || spec.maximum_raw_bundle_bytes == 0
    {
        return Err(error(
            "EngineDefaultGlyphSetSpecInvalid",
            "glyph spec violates v1 constraints",
        ));
    }
    Ok(())
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

fn built_in_profile(lock: &EngineDefaultGlyphSetLock) -> FontAtlasProfileAsset {
    let literals = lock
        .entries
        .iter()
        .filter(|entry| entry.codepoint != 0xfffd)
        .map(|entry| char::from_u32(entry.codepoint).expect("validated Unicode scalar"))
        .collect::<String>();
    FontAtlasProfileAsset {
        schema_version: FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION.to_string(),
        asset_id: FONT_PROFILE_ID.to_string(),
        role: FontAtlasProfileRole::DefaultUi,
        font_stack: FONT_STACK_ID.to_string(),
        glyph_set: FontGlyphSet {
            include_runtime_text_sources: false,
            unicode_ranges: Vec::new(),
            literals: vec![literals],
            locales: vec!["zh-CN".to_string()],
        },
        raster: FontRasterProfile {
            policy: FontRasterPolicy::AutoHybrid,
            bitmap_pixel_sizes: vec![16, 24, 32],
            bitmap_hinting: FontHintingMode::FontDefault,
            msdf_em_size: 64,
            msdf_pixel_range: 8,
        },
        packing: FontPackingProfile {
            page_width: 1024,
            page_height: 1024,
            padding: 1,
            max_bitmap_pages: 8,
            max_msdf_pages: 8,
        },
    }
}

fn add_replacement_alias(
    output: &mut crate::ProjectFontCookOutput,
) -> Result<(), EngineBuiltInFontPackError> {
    const SOURCE: u32 = 0x25a1;
    const TARGET: u32 = 0xfffd;
    let used_glyph_ids = output
        .resolutions
        .iter()
        .map(|resolution| resolution.glyph_id)
        .collect::<BTreeSet<_>>();
    let alias_glyph_id = (0..=u16::MAX)
        .rev()
        .find(|glyph_id| !used_glyph_ids.contains(glyph_id))
        .ok_or_else(|| {
            error(
                "EngineBuiltInFontPackReplacementAliasMissing",
                "no synthetic glyph id is available for U+FFFD",
            )
        })?;
    let mut resolution = output
        .resolutions
        .iter()
        .find(|resolution| resolution.codepoint == SOURCE)
        .cloned()
        .ok_or_else(|| {
            error(
                "EngineBuiltInFontPackReplacementAliasMissing",
                "U+25A1 is unavailable for the U+FFFD replacement alias",
            )
        })?;
    resolution.codepoint = TARGET;
    resolution.glyph_id = alias_glyph_id;
    output.resolutions.push(resolution);
    let hinted = output
        .hinted_variants
        .iter()
        .filter(|variant| variant.codepoint == SOURCE)
        .cloned()
        .map(|mut variant| {
            variant.codepoint = TARGET;
            variant.glyph_id = alias_glyph_id;
            variant
        })
        .collect::<Vec<_>>();
    let msdf = output
        .msdf_variants
        .iter()
        .filter(|variant| variant.codepoint == SOURCE)
        .cloned()
        .map(|mut variant| {
            variant.codepoint = TARGET;
            variant.glyph_id = alias_glyph_id;
            variant
        })
        .collect::<Vec<_>>();
    if hinted.is_empty() || msdf.is_empty() {
        return Err(error(
            "EngineBuiltInFontPackReplacementAliasMissing",
            "U+25A1 raster variants are incomplete",
        ));
    }
    output.hinted_variants.extend(hinted);
    output.msdf_variants.extend(msdf);
    output.required_codepoints.push(TARGET);
    output.required_codepoints.sort_unstable();
    output.resolutions.sort_by_key(|entry| entry.codepoint);
    Ok(())
}

fn built_in_assets(profile: FontAtlasProfileAsset, source_sha256: &str) -> ProjectFontAssetSet {
    ProjectFontAssetSet {
        faces: vec![FontFaceAsset {
            schema_version: FONT_FACE_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: FONT_FACE_ID.to_string(),
            source: FontFaceSource {
                kind: FontSourceKind::ProjectFile,
                asset_ref: FONT_SOURCE_ID.to_string(),
                face_index: 0,
                source_sha256: source_sha256.to_string(),
            },
            declared: FontFaceDeclaredMetadata {
                family: "Noto Sans SC".to_string(),
                style: FontStyle::Normal,
                weight: 400,
                stretch: 100,
            },
            hinting: FontHintingMode::FontDefault,
        }],
        families: vec![FontFamilyAsset {
            schema_version: FONT_FAMILY_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: FONT_FAMILY_ID.to_string(),
            faces: vec![FontFamilyFace {
                font_face: FONT_FACE_ID.to_string(),
                style: FontStyle::Normal,
                weight: 400,
            }],
            missing_style_policy: FontMissingStylePolicy::NearestWeightSameStyle,
        }],
        stacks: vec![FontStackAsset {
            schema_version: FONT_STACK_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: FONT_STACK_ID.to_string(),
            families: vec![FONT_FAMILY_ID.to_string()],
            missing_glyph_policy: FontMissingGlyphPolicy::Error,
            replacement_codepoint: "U+FFFD".to_string(),
        }],
        profiles: vec![profile],
    }
}

fn glyph_lock_digest(
    lock: &EngineDefaultGlyphSetLock,
) -> Result<String, EngineBuiltInFontPackError> {
    let mut canonical = lock.clone();
    canonical.digest.clear();
    serde_json::to_vec(&canonical)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|cause| error("EngineDefaultGlyphSetLockInvalid", cause.to_string()))
}

fn parse_codepoint(value: &str) -> Result<u32, EngineBuiltInFontPackError> {
    let digits = value.strip_prefix("U+").ok_or_else(|| {
        error(
            "EngineDefaultGlyphSetCodepointInvalid",
            format!("{value} must use U+XXXX"),
        )
    })?;
    let codepoint = u32::from_str_radix(digits, 16)
        .map_err(|cause| error("EngineDefaultGlyphSetCodepointInvalid", cause.to_string()))?;
    char::from_u32(codepoint).ok_or_else(|| {
        error(
            "EngineDefaultGlyphSetCodepointInvalid",
            format!("{value} is not a Unicode scalar"),
        )
    })?;
    Ok(codepoint)
}

fn add_source(entries: &mut BTreeMap<u32, BTreeSet<String>>, codepoint: u32, source: &str) {
    entries
        .entry(codepoint)
        .or_default()
        .insert(source.to_string());
}

fn collect_json_strings(value: &serde_json::Value, target: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) => target.push(value.clone()),
        serde_json::Value::Array(values) => values
            .iter()
            .for_each(|value| collect_json_strings(value, target)),
        serde_json::Value::Object(values) => values
            .values()
            .for_each(|value| collect_json_strings(value, target)),
        _ => {}
    }
}

fn is_han(codepoint: u32) -> bool {
    matches!(
        codepoint,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    )
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    code: &'static str,
) -> Result<T, EngineBuiltInFontPackError> {
    let bytes =
        fs::read(path).map_err(|cause| error(code, format!("{}: {cause}", path.display())))?;
    serde_json::from_slice(&bytes)
        .map_err(|cause| error(code, format!("{}: {cause}", path.display())))
}

fn pretty_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, EngineBuiltInFontPackError> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|cause| error("EngineBuiltInFontPackEncodeFailed", cause.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), EngineBuiltInFontPackError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|cause| error("EngineBuiltInFontPackWriteFailed", cause.to_string()))?;
    }
    fs::write(path, pretty_json_bytes(value)?)
        .map_err(|cause| error("EngineBuiltInFontPackWriteFailed", cause.to_string()))
}

fn error(code: &'static str, message: impl Into<String>) -> EngineBuiltInFontPackError {
    EngineBuiltInFontPackError {
        code,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pack_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/runtime/font-packs/aife-default-zh-cn-common-v1")
    }

    #[test]
    fn engine_builtin_font_pack_source_identity_is_pinned() {
        let bytes = fs::read(pack_root().join("source/NotoSansSC-Regular.ttf")).unwrap();
        assert_eq!(bytes.len(), 10_559_284);
        assert_eq!(
            sha256_prefixed(&bytes),
            "sha256:d45f67f0a7c0ca3f256950777ce6a61cc7ce5f9696d02900cbbaac25f8aa7d16"
        );
    }

    #[test]
    fn engine_default_glyph_set_is_deterministic_and_within_budget() {
        let first = EngineBuiltInFontPack::resolve_glyph_set(&pack_root()).unwrap();
        let second = EngineBuiltInFontPack::resolve_glyph_set(&pack_root()).unwrap();
        assert_eq!(first, second);
        EngineBuiltInFontPack::validate_glyph_set_lock(&first).unwrap();
        assert!((1200..=1500).contains(&first.han_codepoint_count));
        assert!(first.total_codepoint_count <= 1800);
        for required in [' ', 'A', '中', '，', '→', '\u{fffd}'] {
            assert!(
                first
                    .entries
                    .iter()
                    .any(|entry| entry.codepoint == u32::from(required)),
                "missing {required}"
            );
        }
    }

    #[test]
    fn engine_default_glyph_set_digest_rejects_mutation() {
        let mut lock = EngineBuiltInFontPack::resolve_glyph_set(&pack_root()).unwrap();
        lock.entries[0].sources = vec!["mutatedSource".to_string()];
        assert_eq!(
            EngineBuiltInFontPack::validate_glyph_set_lock(&lock)
                .unwrap_err()
                .code,
            "EngineDefaultGlyphSetLockDigestMismatch"
        );
    }

    #[test]
    fn engine_builtin_font_pack_embedded_seal_loads_when_present() {
        if embedded::BUILTIN_FONT_PACK_MANIFEST_BYTES.is_empty() {
            return;
        }
        let first = EngineBuiltInFontPack::load_embedded().unwrap();
        let second = EngineBuiltInFontPack::load_embedded().unwrap();
        assert_eq!(first, second);
        assert!(first
            .metadata
            .glyphs
            .iter()
            .any(|glyph| glyph.codepoint == u32::from('中')));
    }
}
