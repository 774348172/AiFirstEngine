use crate::canonical_digest::sha256_prefixed;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const COOKED_FONT_BUNDLE_SCHEMA_VERSION: &str = "cooked-font-bundle.v2";
pub const RUNTIME_FONT_BUNDLE_MANIFEST_SCHEMA_VERSION: &str = "runtime-font-bundle-manifest.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FontBundleRenderMode {
    BitmapR8,
    MsdfRgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FontBundleStyle {
    Normal,
    Italic,
    Oblique,
}

impl FontBundleRenderMode {
    fn bytes_per_pixel(self) -> usize {
        match self {
            Self::BitmapR8 => 1,
            Self::MsdfRgba8 => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedFontBundlePage {
    pub page_index: u32,
    pub render_mode: FontBundleRenderMode,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
    pub sha256: String,
    pub payload_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedFontBundleGlyph {
    pub font_family_id: String,
    pub font_face_id: String,
    pub style: FontBundleStyle,
    pub weight: u16,
    pub glyph_id: u16,
    pub codepoint: u32,
    pub render_mode: FontBundleRenderMode,
    pub pixel_size: u16,
    pub page_index: u32,
    pub pixel_rect: [u32; 4],
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance_per_em_millionths: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedFontBundleKerning {
    pub font_face_id: String,
    pub left_glyph_id: u16,
    pub right_glyph_id: u16,
    pub adjustment_per_em_millionths: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookedFontBundleAsset {
    pub schema_version: String,
    pub font_bundle_id: String,
    pub font_stack_id: String,
    pub generation: u64,
    pub max_bitmap_pages: u16,
    pub max_msdf_pages: u16,
    pub legacy_mode: bool,
    pub fallback_used: bool,
    pub quality_gate_eligible: bool,
    pub pages: Vec<CookedFontBundlePage>,
    pub glyphs: Vec<CookedFontBundleGlyph>,
    pub kerning_adjustments: Vec<CookedFontBundleKerning>,
    pub bundle_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageSourceFontBundle {
    pub metadata: CookedFontBundleAsset,
    pub page_payloads: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLoadedFontBundle {
    pub metadata: CookedFontBundleAsset,
    pub page_payloads: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeFontBundleManifest {
    pub schema_version: String,
    pub default_ui_font_bundle_id: Option<String>,
    pub bundles: Vec<RuntimeFontBundleManifestEntry>,
}

impl RuntimeFontBundleManifest {
    pub fn empty() -> Self {
        Self {
            schema_version: RUNTIME_FONT_BUNDLE_MANIFEST_SCHEMA_VERSION.to_string(),
            default_ui_font_bundle_id: None,
            bundles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeFontBundleManifestEntry {
    pub font_bundle_id: String,
    pub metadata_path: String,
    pub page_paths: Vec<String>,
    pub bundle_digest: String,
    pub legacy_mode: bool,
    pub fallback_used: bool,
    pub quality_gate_eligible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeFontBundleRegistry {
    pub bundles_by_id: BTreeMap<String, RuntimeLoadedFontBundle>,
    pub default_ui_font_bundle_id: Option<String>,
    pub diagnostics: Vec<RuntimeFontBundleDiagnostic>,
}

impl RuntimeFontBundleRegistry {
    pub fn default_bundle(&self) -> Option<&RuntimeLoadedFontBundle> {
        self.default_ui_font_bundle_id
            .as_deref()
            .and_then(|id| self.bundles_by_id.get(id))
            .or_else(|| self.bundles_by_id.values().next())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFontResolveRequest {
    pub font_bundle_id: Option<String>,
    pub font_family_id: Option<String>,
    pub style: FontBundleStyle,
    pub weight: u16,
    pub codepoint: u32,
    pub render_mode: FontBundleRenderMode,
    pub pixel_size: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeResolvedFontGlyph {
    pub font_bundle_id: String,
    pub glyph: CookedFontBundleGlyph,
    pub fallback_used: bool,
}

pub struct RuntimeFontRegistry<'a> {
    bundles: &'a RuntimeFontBundleRegistry,
}

impl<'a> RuntimeFontRegistry<'a> {
    pub fn new(bundles: &'a RuntimeFontBundleRegistry) -> Self {
        Self { bundles }
    }

    pub fn resolve(&self, request: RuntimeFontResolveRequest) -> Option<RuntimeResolvedFontGlyph> {
        let bundle = request
            .font_bundle_id
            .as_deref()
            .and_then(|id| self.bundles.bundles_by_id.get(id))
            .or_else(|| self.bundles.default_bundle())?;
        let exact = bundle
            .metadata
            .glyphs
            .iter()
            .filter(|glyph| glyph.codepoint == request.codepoint)
            .collect::<Vec<_>>();
        let (candidates, fallback_used) = if exact.is_empty() {
            (
                bundle
                    .metadata
                    .glyphs
                    .iter()
                    .filter(|glyph| glyph.codepoint == u32::from('?'))
                    .collect::<Vec<_>>(),
                true,
            )
        } else {
            (exact, false)
        };
        let glyph = candidates
            .into_iter()
            .filter(|glyph| glyph.render_mode == request.render_mode)
            .filter(|glyph| {
                request
                    .font_family_id
                    .as_ref()
                    .is_none_or(|family| glyph.font_family_id == *family)
            })
            .min_by_key(|glyph| {
                (
                    glyph.style != request.style,
                    glyph.weight.abs_diff(request.weight),
                    glyph.pixel_size.abs_diff(request.pixel_size),
                    glyph.font_face_id.as_str(),
                    glyph.glyph_id,
                )
            })?
            .clone();
        Some(RuntimeResolvedFontGlyph {
            font_bundle_id: bundle.metadata.font_bundle_id.clone(),
            glyph,
            fallback_used,
        })
    }

    pub fn kerning(
        &self,
        font_bundle_id: &str,
        left: &CookedFontBundleGlyph,
        right: &CookedFontBundleGlyph,
    ) -> i32 {
        if left.font_face_id != right.font_face_id {
            return 0;
        }
        self.bundles
            .bundles_by_id
            .get(font_bundle_id)
            .and_then(|bundle| {
                bundle
                    .metadata
                    .kerning_adjustments
                    .iter()
                    .find(|adjustment| {
                        adjustment.font_face_id == left.font_face_id
                            && adjustment.left_glyph_id == left.glyph_id
                            && adjustment.right_glyph_id == right.glyph_id
                    })
            })
            .map(|adjustment| adjustment.adjustment_per_em_millionths)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFontBundleLoadFailure {
    pub diagnostics: Vec<RuntimeFontBundleDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFontBundleDiagnostic {
    pub code: String,
    pub stage: String,
    pub source: String,
    pub message: String,
    pub next_action: String,
}

pub struct RuntimeFontBundleLoader;

impl RuntimeFontBundleLoader {
    pub fn load(
        source: RuntimePackageSourceFontBundle,
    ) -> Result<RuntimeLoadedFontBundle, RuntimeFontBundleLoadFailure> {
        let mut diagnostics = validate_metadata(&source.metadata);
        if source.metadata.pages.len() != source.page_payloads.len() {
            diagnostics.push(diagnostic(
                "FontBundlePagePayloadCountMismatch",
                &source.metadata.font_bundle_id,
                format!(
                    "metadata declares {} pages but {} payloads were supplied",
                    source.metadata.pages.len(),
                    source.page_payloads.len()
                ),
                "Supply every declared page payload in pageIndex order.",
            ));
        }
        for (index, page) in source.metadata.pages.iter().enumerate() {
            let Some(payload) = source.page_payloads.get(index) else {
                continue;
            };
            let expected_len =
                page.width as usize * page.height as usize * page.render_mode.bytes_per_pixel();
            if page.page_index as usize != index {
                diagnostics.push(diagnostic(
                    "FontBundlePageOrderInvalid",
                    &page.payload_path,
                    format!(
                        "pageIndex {} must equal canonical index {index}",
                        page.page_index
                    ),
                    "Sort pages by render mode and pageIndex, then renumber contiguously.",
                ));
            }
            if page.byte_len != payload.len() || expected_len != payload.len() {
                diagnostics.push(diagnostic(
                    "FontBundlePageByteLengthMismatch",
                    &page.payload_path,
                    format!(
                        "declared {}, expected {}, actual {} bytes",
                        page.byte_len,
                        expected_len,
                        payload.len()
                    ),
                    "Recook the complete FontBundle page set.",
                ));
            }
            if page.sha256 != sha256_prefixed(payload) {
                diagnostics.push(diagnostic(
                    "FontBundlePageHashMismatch",
                    &page.payload_path,
                    "page SHA-256 does not match payload bytes",
                    "Restore the declared page or recook the FontBundle.",
                ));
            }
            let expected_format = match page.render_mode {
                FontBundleRenderMode::BitmapR8 => "r8Unorm",
                FontBundleRenderMode::MsdfRgba8 => "rgba8Unorm",
            };
            if page.format != expected_format || page.width == 0 || page.height == 0 {
                diagnostics.push(diagnostic(
                    "FontBundlePageFormatInvalid",
                    &page.payload_path,
                    format!(
                        "{:?} requires {expected_format} with non-zero dimensions",
                        page.render_mode
                    ),
                    "Correct the page descriptor and recook.",
                ));
            }
        }
        let expected_digest = font_bundle_digest(&source.metadata);
        if source.metadata.bundle_digest != expected_digest {
            diagnostics.push(diagnostic(
                "FontBundleDigestMismatch",
                &source.metadata.font_bundle_id,
                format!(
                    "declared {} but canonical metadata digest is {expected_digest}",
                    source.metadata.bundle_digest
                ),
                "Recook metadata and all page payloads together.",
            ));
        }
        if diagnostics.is_empty() {
            Ok(RuntimeLoadedFontBundle {
                metadata: source.metadata,
                page_payloads: source.page_payloads,
            })
        } else {
            Err(RuntimeFontBundleLoadFailure { diagnostics })
        }
    }
}

pub fn font_bundle_digest(metadata: &CookedFontBundleAsset) -> String {
    let mut canonical = metadata.clone();
    canonical.bundle_digest.clear();
    let bytes = serde_json::to_vec(&canonical).expect("FontBundle metadata must serialize");
    sha256_prefixed(&bytes)
}

fn validate_metadata(metadata: &CookedFontBundleAsset) -> Vec<RuntimeFontBundleDiagnostic> {
    let mut diagnostics = Vec::new();
    if metadata.schema_version != COOKED_FONT_BUNDLE_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            "FontBundleSchemaMismatch",
            &metadata.font_bundle_id,
            format!(
                "schemaVersion must be {COOKED_FONT_BUNDLE_SCHEMA_VERSION}, found {}",
                metadata.schema_version
            ),
            "Rebuild the FontBundle with the current editor.",
        ));
    }
    if metadata.font_bundle_id.trim().is_empty()
        || metadata.font_stack_id.trim().is_empty()
        || metadata.pages.is_empty()
        || metadata.glyphs.is_empty()
    {
        diagnostics.push(diagnostic(
            "FontBundleMetadataIncomplete",
            &metadata.font_bundle_id,
            "bundle id, stack id, pages, and glyphs are required",
            "Recook a complete FontBundle.",
        ));
    }
    if metadata.legacy_mode && metadata.quality_gate_eligible {
        diagnostics.push(diagnostic(
            "FontBundleLegacyQualityInvalid",
            &metadata.font_bundle_id,
            "legacy FontBundle cannot be quality-gate eligible",
            "Keep the legacy adapter read-only and migrate the project to v2.",
        ));
    }
    let bitmap_pages = metadata
        .pages
        .iter()
        .filter(|page| page.render_mode == FontBundleRenderMode::BitmapR8)
        .count();
    let msdf_pages = metadata
        .pages
        .iter()
        .filter(|page| page.render_mode == FontBundleRenderMode::MsdfRgba8)
        .count();
    if bitmap_pages > usize::from(metadata.max_bitmap_pages)
        || msdf_pages > usize::from(metadata.max_msdf_pages)
    {
        diagnostics.push(diagnostic(
            "FontBundlePageBudgetExceeded",
            &metadata.font_bundle_id,
            format!(
                "bitmap pages {bitmap_pages}/{}, MSDF pages {msdf_pages}/{}",
                metadata.max_bitmap_pages, metadata.max_msdf_pages
            ),
            "Increase the explicit profile budget or reduce the glyph set.",
        ));
    }
    let mut keys = BTreeSet::new();
    for glyph in &metadata.glyphs {
        let key = (
            glyph.font_face_id.as_str(),
            glyph.glyph_id,
            glyph.render_mode,
            glyph.pixel_size,
        );
        if !keys.insert(key) {
            diagnostics.push(diagnostic(
                "FontBundleGlyphDuplicate",
                &metadata.font_bundle_id,
                format!(
                    "duplicate glyph key {}:{}:{:?}:{}",
                    glyph.font_face_id, glyph.glyph_id, glyph.render_mode, glyph.pixel_size
                ),
                "Deduplicate glyph variants before packing.",
            ));
        }
        let Some(page) = metadata.pages.get(glyph.page_index as usize) else {
            diagnostics.push(diagnostic(
                "FontBundleGlyphPageMissing",
                &metadata.font_bundle_id,
                format!("glyph references missing page {}", glyph.page_index),
                "Recook metadata and pages atomically.",
            ));
            continue;
        };
        let [x, y, width, height] = glyph.pixel_rect;
        if page.render_mode != glyph.render_mode
            || width == 0
            || height == 0
            || x.saturating_add(width) > page.width
            || y.saturating_add(height) > page.height
        {
            diagnostics.push(diagnostic(
                "FontBundleGlyphRectInvalid",
                &metadata.font_bundle_id,
                format!(
                    "glyph {} has invalid rect {:?} on page {}",
                    glyph.glyph_id, glyph.pixel_rect, glyph.page_index
                ),
                "Repack the glyph into a matching page.",
            ));
        }
    }
    diagnostics
}

fn diagnostic(
    code: &str,
    source: &str,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> RuntimeFontBundleDiagnostic {
    RuntimeFontBundleDiagnostic {
        code: code.to_string(),
        stage: "load".to_string(),
        source: source.to_string(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> RuntimePackageSourceFontBundle {
        let payload = vec![255; 16];
        let mut metadata = CookedFontBundleAsset {
            schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
            font_bundle_id: "font-ui".to_string(),
            font_stack_id: "stack-ui".to_string(),
            generation: 1,
            max_bitmap_pages: 2,
            max_msdf_pages: 2,
            legacy_mode: false,
            fallback_used: false,
            quality_gate_eligible: true,
            pages: vec![CookedFontBundlePage {
                page_index: 0,
                render_mode: FontBundleRenderMode::BitmapR8,
                format: "r8Unorm".to_string(),
                width: 4,
                height: 4,
                byte_len: payload.len(),
                sha256: sha256_prefixed(&payload),
                payload_path: "fonts/font-ui/page-000.r8".to_string(),
            }],
            glyphs: vec![CookedFontBundleGlyph {
                font_family_id: "family-ui".to_string(),
                font_face_id: "face-ui".to_string(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id: 7,
                codepoint: u32::from('中'),
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size: 16,
                page_index: 0,
                pixel_rect: [0, 0, 4, 4],
                bearing_x: 0,
                bearing_y: 4,
                advance_per_em_millionths: 1_000_000,
            }],
            kerning_adjustments: Vec::new(),
            bundle_digest: String::new(),
        };
        metadata.bundle_digest = font_bundle_digest(&metadata);
        RuntimePackageSourceFontBundle {
            metadata,
            page_payloads: vec![payload],
        }
    }

    #[test]
    fn cooked_font_bundle_loader_accepts_complete_bundle() {
        let loaded = RuntimeFontBundleLoader::load(source()).expect("complete bundle");
        assert_eq!(loaded.metadata.pages.len(), 1);
    }

    #[test]
    fn runtime_font_bundle_loader_rejects_half_bundle_and_corruption() {
        let mut missing = source();
        missing.page_payloads.clear();
        assert!(RuntimeFontBundleLoader::load(missing)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "FontBundlePagePayloadCountMismatch"));

        let mut corrupt = source();
        corrupt.page_payloads[0][0] = 0;
        assert!(RuntimeFontBundleLoader::load(corrupt)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "FontBundlePageHashMismatch"));
    }

    #[test]
    fn cooked_font_bundle_rejects_duplicate_glyph_budget_and_legacy_quality() {
        let mut invalid = source();
        invalid
            .metadata
            .glyphs
            .push(invalid.metadata.glyphs[0].clone());
        invalid.metadata.max_bitmap_pages = 0;
        invalid.metadata.legacy_mode = true;
        invalid.metadata.bundle_digest = font_bundle_digest(&invalid.metadata);
        let codes: BTreeSet<_> = RuntimeFontBundleLoader::load(invalid)
            .unwrap_err()
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(codes.contains("FontBundleGlyphDuplicate"));
        assert!(codes.contains("FontBundlePageBudgetExceeded"));
        assert!(codes.contains("FontBundleLegacyQualityInvalid"));
    }

    #[test]
    fn runtime_font_registry_resolves_mode_nearest_fallback_and_kerning() {
        let mut loaded = RuntimeFontBundleLoader::load(source()).unwrap();
        loaded.metadata.glyphs.extend([
            CookedFontBundleGlyph {
                font_family_id: "family-ui".to_string(),
                font_face_id: "face-ui".to_string(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id: 8,
                codepoint: u32::from('?'),
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size: 16,
                page_index: 0,
                pixel_rect: [0, 0, 4, 4],
                bearing_x: 0,
                bearing_y: 4,
                advance_per_em_millionths: 500_000,
            },
            CookedFontBundleGlyph {
                font_family_id: "family-ui".to_string(),
                font_face_id: "face-ui".to_string(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id: 7,
                codepoint: u32::from('中'),
                render_mode: FontBundleRenderMode::MsdfRgba8,
                pixel_size: 64,
                page_index: 0,
                pixel_rect: [0, 0, 4, 4],
                bearing_x: 0,
                bearing_y: 4,
                advance_per_em_millionths: 1_000_000,
            },
        ]);
        loaded.metadata.kerning_adjustments = vec![CookedFontBundleKerning {
            font_face_id: "face-ui".to_string(),
            left_glyph_id: 7,
            right_glyph_id: 7,
            adjustment_per_em_millionths: -50_000,
        }];
        let mut bundles = RuntimeFontBundleRegistry::default();
        bundles.default_ui_font_bundle_id = Some("font-ui".to_string());
        bundles.bundles_by_id.insert("font-ui".to_string(), loaded);
        let registry = RuntimeFontRegistry::new(&bundles);
        let resolved = registry
            .resolve(RuntimeFontResolveRequest {
                font_bundle_id: None,
                font_family_id: Some("family-ui".to_string()),
                style: FontBundleStyle::Normal,
                weight: 400,
                codepoint: u32::from('中'),
                render_mode: FontBundleRenderMode::MsdfRgba8,
                pixel_size: 40,
            })
            .unwrap();
        assert_eq!(resolved.glyph.pixel_size, 64);
        assert!(!resolved.fallback_used);
        let fallback = registry
            .resolve(RuntimeFontResolveRequest {
                font_bundle_id: None,
                font_family_id: None,
                style: FontBundleStyle::Normal,
                weight: 400,
                codepoint: u32::from('缺'),
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size: 16,
            })
            .unwrap();
        assert!(fallback.fallback_used);
        assert_eq!(fallback.glyph.codepoint, u32::from('?'));
        assert_eq!(
            registry.kerning("font-ui", &resolved.glyph, &resolved.glyph),
            -50_000
        );
    }
}
