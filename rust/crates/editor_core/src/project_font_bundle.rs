use crate::{
    FontAtlasProfileAsset, ProjectFontCookFailure, ProjectFontCookOutput,
    ProjectFontHintedGlyphVariant, ProjectFontMsdfGlyphVariant,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::font_bundle::{
    font_bundle_digest, CookedFontBundleAsset, CookedFontBundleGlyph, CookedFontBundleKerning,
    CookedFontBundlePage, FontBundleRenderMode, FontBundleStyle, RuntimePackageSourceFontBundle,
    COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
#[cfg(test)]
use std::collections::BTreeSet;

pub struct ProjectFontBundleBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontAutoHybridRequest {
    pub authored_pixel_size: u16,
    pub dpi_scale_millionths: u32,
    pub node_scale_millionths: u32,
    pub continuous_scale: bool,
    pub outline_width_millionths: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontAutoHybridDecision {
    pub render_mode: FontBundleRenderMode,
    pub physical_pixel_size_millionths: u64,
    pub reason: &'static str,
}

pub fn select_auto_hybrid(request: FontAutoHybridRequest) -> FontAutoHybridDecision {
    let physical = u64::from(request.authored_pixel_size)
        .saturating_mul(u64::from(request.dpi_scale_millionths))
        .saturating_mul(u64::from(request.node_scale_millionths))
        / 1_000_000;
    let bitmap_scale_ok = (875_000..=1_125_000).contains(&request.node_scale_millionths);
    let (render_mode, reason) = if physical > 32_000_000 {
        (FontBundleRenderMode::MsdfRgba8, "physical_px_above_32")
    } else if request.continuous_scale {
        (FontBundleRenderMode::MsdfRgba8, "continuous_scale")
    } else if request.outline_width_millionths > 1_000_000 {
        (FontBundleRenderMode::MsdfRgba8, "coarse_outline")
    } else if !bitmap_scale_ok {
        (FontBundleRenderMode::MsdfRgba8, "bitmap_scale_outside_band")
    } else {
        (
            FontBundleRenderMode::BitmapR8,
            "hinted_bitmap_small_physical_px",
        )
    };
    FontAutoHybridDecision {
        render_mode,
        physical_pixel_size_millionths: physical,
        reason,
    }
}

impl ProjectFontBundleBuilder {
    pub fn build_bitmap_v2(
        profile: &FontAtlasProfileAsset,
        cook: &ProjectFontCookOutput,
    ) -> Result<RuntimePackageSourceFontBundle, ProjectFontCookFailure> {
        let width = u32::from(profile.packing.page_width);
        let height = u32::from(profile.packing.page_height);
        let padding = u32::from(profile.packing.padding);
        let mut variants = cook.hinted_variants.iter().collect::<Vec<_>>();
        variants.sort_by_key(|variant| {
            (
                variant.font_face_id.as_str(),
                variant.glyph_id,
                variant.pixel_size,
            )
        });
        variants.dedup_by_key(|variant| {
            (
                variant.font_face_id.as_str(),
                variant.glyph_id,
                variant.pixel_size,
            )
        });

        let mut pages = Vec::<PackedPage>::new();
        let mut glyphs = Vec::new();
        for variant in variants {
            if variant.width.saturating_add(padding.saturating_mul(2)) > width
                || variant.height.saturating_add(padding.saturating_mul(2)) > height
            {
                return Err(bundle_failure(
                    "FontBundleGlyphExceedsPage",
                    format!(
                        "{}:{} at {}px cannot fit {}x{}",
                        variant.font_face_id, variant.glyph_id, variant.pixel_size, width, height
                    ),
                ));
            }
            let placement = match pages.last_mut().and_then(|page| page.place(variant)) {
                Some(placement) => placement,
                None => {
                    if pages.len() >= usize::from(profile.packing.max_bitmap_pages) {
                        return Err(bundle_failure(
                            "FontBundlePageBudgetExceeded",
                            format!(
                                "bitmap page budget {} is insufficient",
                                profile.packing.max_bitmap_pages
                            ),
                        ));
                    }
                    pages.push(PackedPage::new(width, height, padding));
                    pages
                        .last_mut()
                        .and_then(|page| page.place(variant))
                        .expect("validated glyph must fit an empty page")
                }
            };
            glyphs.push(CookedFontBundleGlyph {
                font_family_id: cook
                    .resolutions
                    .iter()
                    .find(|resolution| resolution.codepoint == variant.codepoint)
                    .map(|resolution| resolution.font_family_id.clone())
                    .unwrap_or_default(),
                font_face_id: variant.font_face_id.clone(),
                style: cook
                    .resolutions
                    .iter()
                    .find(|resolution| resolution.codepoint == variant.codepoint)
                    .map(|resolution| bundle_style(resolution.style))
                    .unwrap_or(FontBundleStyle::Normal),
                weight: cook
                    .resolutions
                    .iter()
                    .find(|resolution| resolution.codepoint == variant.codepoint)
                    .map(|resolution| resolution.weight)
                    .unwrap_or(400),
                glyph_id: variant.glyph_id,
                codepoint: variant.codepoint,
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size: variant.pixel_size,
                page_index: (pages.len() - 1) as u32,
                pixel_rect: [placement.0, placement.1, variant.width, variant.height],
                bearing_x: variant.bearing_x,
                bearing_y: variant.bearing_y,
                advance_per_em_millionths: variant.advance_per_em_millionths,
            });
        }
        if pages.is_empty() {
            return Err(bundle_failure(
                "FontBundleGlyphsEmpty",
                "no bitmap variants were available",
            ));
        }
        let mut page_payloads = pages
            .into_iter()
            .map(|page| page.pixels)
            .collect::<Vec<_>>();
        let mut page_descriptors = page_payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| CookedFontBundlePage {
                page_index: index as u32,
                render_mode: FontBundleRenderMode::BitmapR8,
                format: "r8Unorm".to_string(),
                width,
                height,
                byte_len: payload.len(),
                sha256: sha256_prefixed(payload),
                payload_path: format!("fonts/{}/bitmap-page-{index:03}.r8", profile.asset_id),
            })
            .collect::<Vec<_>>();

        let bitmap_page_count = page_payloads.len();
        let mut msdf_variants = cook.msdf_variants.iter().collect::<Vec<_>>();
        msdf_variants.sort_by_key(|variant| (variant.font_face_id.as_str(), variant.glyph_id));
        msdf_variants.dedup_by_key(|variant| (variant.font_face_id.as_str(), variant.glyph_id));
        let mut msdf_pages = Vec::<PackedRgbaPage>::new();
        for variant in msdf_variants {
            if variant.width.saturating_add(padding.saturating_mul(2)) > width
                || variant.height.saturating_add(padding.saturating_mul(2)) > height
            {
                return Err(bundle_failure(
                    "FontBundleGlyphExceedsPage",
                    format!(
                        "{}:{} MSDF cannot fit {}x{}",
                        variant.font_face_id, variant.glyph_id, width, height
                    ),
                ));
            }
            let placement = match msdf_pages.last_mut().and_then(|page| page.place(variant)) {
                Some(placement) => placement,
                None => {
                    if msdf_pages.len() >= usize::from(profile.packing.max_msdf_pages) {
                        return Err(bundle_failure(
                            "FontBundlePageBudgetExceeded",
                            format!(
                                "MSDF page budget {} is insufficient",
                                profile.packing.max_msdf_pages
                            ),
                        ));
                    }
                    msdf_pages.push(PackedRgbaPage::new(width, height, padding));
                    msdf_pages
                        .last_mut()
                        .and_then(|page| page.place(variant))
                        .expect("validated MSDF glyph must fit an empty page")
                }
            };
            glyphs.push(CookedFontBundleGlyph {
                font_family_id: cook
                    .resolutions
                    .iter()
                    .find(|resolution| resolution.codepoint == variant.codepoint)
                    .map(|resolution| resolution.font_family_id.clone())
                    .unwrap_or_default(),
                font_face_id: variant.font_face_id.clone(),
                style: cook
                    .resolutions
                    .iter()
                    .find(|resolution| resolution.codepoint == variant.codepoint)
                    .map(|resolution| bundle_style(resolution.style))
                    .unwrap_or(FontBundleStyle::Normal),
                weight: cook
                    .resolutions
                    .iter()
                    .find(|resolution| resolution.codepoint == variant.codepoint)
                    .map(|resolution| resolution.weight)
                    .unwrap_or(400),
                glyph_id: variant.glyph_id,
                codepoint: variant.codepoint,
                render_mode: FontBundleRenderMode::MsdfRgba8,
                pixel_size: variant.em_size,
                page_index: (bitmap_page_count + msdf_pages.len() - 1) as u32,
                pixel_rect: [placement.0, placement.1, variant.width, variant.height],
                bearing_x: variant.bearing_x,
                bearing_y: variant.bearing_y,
                advance_per_em_millionths: variant.advance_per_em_millionths,
            });
        }
        for (index, page) in msdf_pages.into_iter().enumerate() {
            let payload = page.pixels;
            let page_index = bitmap_page_count + index;
            page_descriptors.push(CookedFontBundlePage {
                page_index: page_index as u32,
                render_mode: FontBundleRenderMode::MsdfRgba8,
                format: "rgba8Unorm".to_string(),
                width,
                height,
                byte_len: payload.len(),
                sha256: sha256_prefixed(&payload),
                payload_path: format!("fonts/{}/msdf-page-{index:03}.rgba8", profile.asset_id),
            });
            page_payloads.push(payload);
        }
        let mut metadata = CookedFontBundleAsset {
            schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
            font_bundle_id: profile.asset_id.clone(),
            font_stack_id: profile.font_stack.clone(),
            generation: 1,
            max_bitmap_pages: profile.packing.max_bitmap_pages,
            max_msdf_pages: profile.packing.max_msdf_pages,
            legacy_mode: false,
            fallback_used: cook
                .resolutions
                .iter()
                .any(|resolution| resolution.fallback_index > 0),
            quality_gate_eligible: true,
            pages: page_descriptors,
            glyphs,
            kerning_adjustments: cook
                .kerning_adjustments
                .iter()
                .map(|adjustment| CookedFontBundleKerning {
                    font_face_id: adjustment.font_face_id.clone(),
                    left_glyph_id: adjustment.left_glyph_id,
                    right_glyph_id: adjustment.right_glyph_id,
                    adjustment_per_em_millionths: adjustment.adjustment_per_em_millionths,
                })
                .collect(),
            bundle_digest: String::new(),
        };
        metadata.bundle_digest = font_bundle_digest(&metadata);
        Ok(RuntimePackageSourceFontBundle {
            metadata,
            page_payloads,
        })
    }
}

struct PackedRgbaPage {
    width: u32,
    height: u32,
    padding: u32,
    x: u32,
    y: u32,
    row_height: u32,
    pixels: Vec<u8>,
}

impl PackedRgbaPage {
    fn new(width: u32, height: u32, padding: u32) -> Self {
        Self {
            width,
            height,
            padding,
            x: padding,
            y: padding,
            row_height: 0,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    fn place(&mut self, variant: &ProjectFontMsdfGlyphVariant) -> Option<(u32, u32)> {
        let mut x = self.x;
        let mut y = self.y;
        let mut row_height = self.row_height;
        if x + variant.width + self.padding > self.width {
            x = self.padding;
            y = y + row_height + self.padding;
            row_height = 0;
        }
        if y + variant.height + self.padding > self.height {
            return None;
        }
        for row in 0..variant.height {
            let source_start = (row * variant.stride) as usize;
            let source_end = source_start + (variant.width * 4) as usize;
            let target_start = (((y + row) * self.width + x) * 4) as usize;
            self.pixels[target_start..target_start + (variant.width * 4) as usize]
                .copy_from_slice(&variant.rgba8[source_start..source_end]);
        }
        self.x = x + variant.width + self.padding;
        self.y = y;
        self.row_height = row_height.max(variant.height);
        Some((x, y))
    }
}

struct PackedPage {
    width: u32,
    height: u32,
    padding: u32,
    x: u32,
    y: u32,
    row_height: u32,
    pixels: Vec<u8>,
}

impl PackedPage {
    fn new(width: u32, height: u32, padding: u32) -> Self {
        Self {
            width,
            height,
            padding,
            x: padding,
            y: padding,
            row_height: 0,
            pixels: vec![0; (width * height) as usize],
        }
    }

    fn place(&mut self, variant: &ProjectFontHintedGlyphVariant) -> Option<(u32, u32)> {
        let mut x = self.x;
        let mut y = self.y;
        let mut row_height = self.row_height;
        if x + variant.width + self.padding > self.width {
            x = self.padding;
            y = y + row_height + self.padding;
            row_height = 0;
        }
        if y + variant.height + self.padding > self.height {
            return None;
        }
        for row in 0..variant.height {
            let source_start = (row * variant.stride) as usize;
            let source_end = source_start + variant.width as usize;
            let target_start = ((y + row) * self.width + x) as usize;
            self.pixels[target_start..target_start + variant.width as usize]
                .copy_from_slice(&variant.alpha_r8[source_start..source_end]);
        }
        self.x = x + variant.width + self.padding;
        self.y = y;
        self.row_height = row_height.max(variant.height);
        Some((x, y))
    }
}

fn bundle_failure(code: &str, message: impl Into<String>) -> ProjectFontCookFailure {
    ProjectFontCookFailure {
        diagnostics: vec![crate::FontDiagnostic::error(
            code,
            crate::FontDiagnosticStage::Pack,
            None,
            message,
            "Adjust the FontAtlasProfile page size/budget or reduce the glyph set.",
        )],
    }
}

fn bundle_style(style: crate::FontStyle) -> FontBundleStyle {
    match style {
        crate::FontStyle::Normal => FontBundleStyle::Normal,
        crate::FontStyle::Italic => FontBundleStyle::Italic,
        crate::FontStyle::Oblique => FontBundleStyle::Oblique,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FontAtlasProfileRole, FontGlyphSet, FontHintingMode, FontPackingProfile, FontRasterPolicy,
        FontRasterProfile,
    };
    use engine_runtime::font_bundle::RuntimeFontBundleLoader;
    use engine_runtime::runtime_package::{CookedFontAtlasAsset, COOKED_FONT_ATLAS_SCHEMA_VERSION};
    use engine_runtime::runtime_package_builder::RuntimePackageSourceFontAtlas;

    fn profile(max_pages: u16) -> FontAtlasProfileAsset {
        FontAtlasProfileAsset {
            schema_version: crate::FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: "font-ui".to_string(),
            role: FontAtlasProfileRole::DefaultUi,
            font_stack: "stack-ui".to_string(),
            glyph_set: FontGlyphSet {
                include_runtime_text_sources: true,
                unicode_ranges: Vec::new(),
                literals: Vec::new(),
                locales: Vec::new(),
            },
            raster: FontRasterProfile {
                policy: FontRasterPolicy::AutoHybrid,
                bitmap_pixel_sizes: vec![16],
                bitmap_hinting: FontHintingMode::FontDefault,
                msdf_em_size: 64,
                msdf_pixel_range: 8,
            },
            packing: FontPackingProfile {
                page_width: 8,
                page_height: 8,
                padding: 1,
                max_bitmap_pages: max_pages,
                max_msdf_pages: 2,
            },
        }
    }

    fn cook() -> ProjectFontCookOutput {
        let variants = (0..3)
            .map(|index| ProjectFontHintedGlyphVariant {
                font_face_id: "face-ui".to_string(),
                glyph_id: index + 1,
                codepoint: u32::from('A') + u32::from(index),
                pixel_size: 16,
                width: 5,
                height: 5,
                stride: 5,
                bearing_x: 0,
                bearing_y: 5,
                advance_per_em_millionths: 600_000,
                alpha_r8: vec![128 + index as u8; 25],
            })
            .collect::<Vec<_>>();
        ProjectFontCookOutput {
            schema_version: crate::PROJECT_FONT_COOK_OUTPUT_SCHEMA_VERSION.to_string(),
            profile_id: "font-ui".to_string(),
            dependency_digest: "sha256:input".to_string(),
            output_digest: "sha256:output".to_string(),
            required_codepoints: variants.iter().map(|variant| variant.codepoint).collect(),
            resolutions: Vec::new(),
            face_metrics: Vec::new(),
            hinted_variants: variants,
            msdf_variants: Vec::new(),
            kerning_adjustments: Vec::new(),
            primary_atlas: RuntimePackageSourceFontAtlas {
                metadata: CookedFontAtlasAsset {
                    schema_version: COOKED_FONT_ATLAS_SCHEMA_VERSION.to_string(),
                    font_atlas_id: "legacy".to_string(),
                    font_asset_id: "legacy".to_string(),
                    font_source_kind: "legacy".to_string(),
                    font_asset_status: "legacy".to_string(),
                    atlas_image_path: "legacy.r8".to_string(),
                    atlas_format: "r8Alpha".to_string(),
                    atlas_width: 1,
                    atlas_height: 1,
                    atlas_generation: 1,
                    atlas_alpha_byte_len: 1,
                    glyphs: Vec::new(),
                    fallback_used: true,
                    diagnostics: Vec::new(),
                },
                atlas_alpha: vec![0],
            },
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn project_font_bundle_builder_packs_bitmap_multi_page_and_loads() {
        let first = ProjectFontBundleBuilder::build_bitmap_v2(&profile(3), &cook()).unwrap();
        let second = ProjectFontBundleBuilder::build_bitmap_v2(&profile(3), &cook()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.metadata.pages.len(), 3);
        assert_eq!(first.metadata.glyphs.len(), 3);
        assert!(first.metadata.quality_gate_eligible);
        RuntimeFontBundleLoader::load(first).expect("builder output must load");
    }

    #[test]
    fn project_font_bundle_builder_fails_closed_on_page_budget() {
        let error = ProjectFontBundleBuilder::build_bitmap_v2(&profile(2), &cook()).unwrap_err();
        assert_eq!(error.diagnostics[0].code, "FontBundlePageBudgetExceeded");
    }

    #[test]
    fn project_font_bundle_determinism_covers_all_page_bytes() {
        let mut changed = cook();
        let first = ProjectFontBundleBuilder::build_bitmap_v2(&profile(3), &changed).unwrap();
        changed.hinted_variants[0].alpha_r8[0] ^= 0xff;
        let second = ProjectFontBundleBuilder::build_bitmap_v2(&profile(3), &changed).unwrap();
        assert_ne!(
            first.metadata.pages[0].sha256,
            second.metadata.pages[0].sha256
        );
        assert_ne!(first.metadata.bundle_digest, second.metadata.bundle_digest);
        let unique: BTreeSet<_> = first
            .metadata
            .glyphs
            .iter()
            .map(|glyph| {
                (
                    glyph.font_face_id.as_str(),
                    glyph.glyph_id,
                    glyph.render_mode,
                    glyph.pixel_size,
                )
            })
            .collect();
        assert_eq!(unique.len(), first.metadata.glyphs.len());
    }

    #[test]
    fn project_font_auto_hybrid_uses_physical_px_scale_outline_and_continuity() {
        let bitmap = select_auto_hybrid(FontAutoHybridRequest {
            authored_pixel_size: 16,
            dpi_scale_millionths: 1_000_000,
            node_scale_millionths: 1_000_000,
            continuous_scale: false,
            outline_width_millionths: 0,
        });
        assert_eq!(bitmap.render_mode, FontBundleRenderMode::BitmapR8);
        assert_eq!(bitmap.physical_pixel_size_millionths, 16_000_000);

        for request in [
            FontAutoHybridRequest {
                authored_pixel_size: 24,
                dpi_scale_millionths: 2_000_000,
                node_scale_millionths: 1_000_000,
                continuous_scale: false,
                outline_width_millionths: 0,
            },
            FontAutoHybridRequest {
                authored_pixel_size: 16,
                dpi_scale_millionths: 1_000_000,
                node_scale_millionths: 1_200_000,
                continuous_scale: false,
                outline_width_millionths: 0,
            },
            FontAutoHybridRequest {
                authored_pixel_size: 16,
                dpi_scale_millionths: 1_000_000,
                node_scale_millionths: 1_000_000,
                continuous_scale: true,
                outline_width_millionths: 0,
            },
            FontAutoHybridRequest {
                authored_pixel_size: 16,
                dpi_scale_millionths: 1_000_000,
                node_scale_millionths: 1_000_000,
                continuous_scale: false,
                outline_width_millionths: 1_500_000,
            },
        ] {
            assert_eq!(
                select_auto_hybrid(request).render_mode,
                FontBundleRenderMode::MsdfRgba8
            );
        }
    }
}
