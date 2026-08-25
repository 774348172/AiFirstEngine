use crate::aui_font_atlas_cooker::AuiFontAtlasCookerCmin;
use crate::{
    resolve_font_family_face, validate_font_face_source, validate_font_stack,
    AssetDatabaseDocument, FontAtlasProfileAsset, FontDiagnostic, FontDiagnosticSeverity,
    FontDiagnosticStage, FontFaceAsset, FontFamilyAsset, FontRasterPolicy, FontStackAsset,
    FontStyle, ProjectFontAssetSet, PROJECT_FONT_RECIPE_VERSION,
};
use crate::{
    ProjectAssemblyArtifactCache, ProjectAssemblyArtifactCacheStatus,
    ProjectAssemblyArtifactPublishStatus, ProjectAssemblyProducerReport,
    ProjectAssemblyProducerSubstageReport, PROJECT_ASSEMBLY_PRODUCER_REPORT_SCHEMA_VERSION,
};
use engine_runtime::font_bundle::{
    RuntimePackageSourceFontBundle, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package::{
    CookedFontAtlasAsset, CookedFontAtlasGlyph, COOKED_FONT_ATLAS_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::{
    RuntimePackageSourceFontAtlas, RuntimePackageSourceJson,
};
use fdsm::bezier::scanline::FillRule;
use fdsm::correct_error::{correct_error_msdf, ErrorCorrectionConfig};
use fdsm::generate::generate_msdf;
use fdsm::render::correct_sign_msdf;
use fdsm::shape::Shape;
use fdsm::transform::Transform;
use image::{ImageBuffer, Rgb};
use nalgebra::{Affine2, Similarity2, Vector2};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;
use swash::FontRef;

pub const PROJECT_FONT_COOK_OUTPUT_SCHEMA_VERSION: &str = "project-font-cook-output.v1";
pub const PROJECT_TEXT_SOURCE_SCHEMA_VERSION: &str = "project-text-source.v1";
const ASSET_DATABASE_PATH: &str = "Library/AssetPipeline/asset-database.json";

pub struct ProjectFontCookModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFontProfileInventoryEntry {
    pub asset_id: String,
    pub role: crate::FontAtlasProfileRole,
}

#[derive(Debug, Clone)]
pub struct ProjectFontCookRequest {
    pub project_root: PathBuf,
    pub assets: ProjectFontAssetSet,
    pub source_paths: BTreeMap<String, PathBuf>,
    pub aui_documents: Vec<RuntimePackageSourceJson>,
    pub localization_texts: Vec<String>,
    pub text_sources: Vec<ProjectTextSourceAsset>,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTextSourceAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub texts: Vec<String>,
    pub unicode_ranges: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFontCookOutput {
    pub schema_version: String,
    pub profile_id: String,
    pub dependency_digest: String,
    pub output_digest: String,
    pub required_codepoints: Vec<u32>,
    pub resolutions: Vec<ProjectFontCodepointResolution>,
    pub face_metrics: Vec<ProjectFontFaceMetrics>,
    pub hinted_variants: Vec<ProjectFontHintedGlyphVariant>,
    pub msdf_variants: Vec<ProjectFontMsdfGlyphVariant>,
    pub kerning_adjustments: Vec<ProjectFontKerningAdjustment>,
    pub primary_atlas: RuntimePackageSourceFontAtlas,
    pub diagnostics: Vec<FontDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFontCodepointResolution {
    pub codepoint: u32,
    pub font_stack_id: String,
    pub font_family_id: String,
    pub font_face_id: String,
    pub style: FontStyle,
    pub weight: u16,
    pub glyph_id: u16,
    pub fallback_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFontFaceMetrics {
    pub font_face_id: String,
    pub units_per_em: u16,
    pub ascent_per_em_millionths: i32,
    pub descent_per_em_millionths: i32,
    pub line_gap_per_em_millionths: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFontHintedGlyphVariant {
    pub font_face_id: String,
    pub glyph_id: u16,
    pub codepoint: u32,
    pub pixel_size: u16,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance_per_em_millionths: i32,
    pub alpha_r8: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFontMsdfGlyphVariant {
    pub font_face_id: String,
    pub glyph_id: u16,
    pub codepoint: u32,
    pub em_size: u16,
    pub pixel_range: u16,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance_per_em_millionths: i32,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFontKerningAdjustment {
    pub font_face_id: String,
    pub left_glyph_id: u16,
    pub right_glyph_id: u16,
    pub adjustment_per_em_millionths: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFontCookFailure {
    pub diagnostics: Vec<FontDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFontRuntimePackageCook {
    pub legacy_atlas: Option<RuntimePackageSourceFontAtlas>,
    pub font_bundle: Option<RuntimePackageSourceFontBundle>,
}

struct PreparedProjectFontCook {
    request: ProjectFontCookRequest,
    profile: FontAtlasProfileAsset,
    dependency_digest: String,
    substages: Vec<ProjectAssemblyProducerSubstageReport>,
}

struct TimedProjectFontCookOutput {
    output: ProjectFontCookOutput,
    substages: Vec<ProjectAssemblyProducerSubstageReport>,
}

impl ProjectFontCookModule {
    pub fn cook(
        request: ProjectFontCookRequest,
    ) -> Result<ProjectFontCookOutput, ProjectFontCookFailure> {
        Self::cook_timed(request).map(|timed| timed.output)
    }

    fn cook_timed(
        request: ProjectFontCookRequest,
    ) -> Result<TimedProjectFontCookOutput, ProjectFontCookFailure> {
        let mut substages = Vec::new();
        let resolve_assets_started = Instant::now();
        let mut diagnostics = request.assets.validate_references();
        if !diagnostics.is_empty() {
            return Err(ProjectFontCookFailure { diagnostics });
        }
        let profile = request
            .assets
            .profiles
            .iter()
            .find(|profile| profile.asset_id == request.profile_id)
            .cloned()
            .ok_or_else(|| failure("FontAtlasProfileMissing", &request.profile_id))?;
        let families: BTreeMap<String, FontFamilyAsset> = request
            .assets
            .families
            .iter()
            .cloned()
            .map(|asset| (asset.asset_id.clone(), asset))
            .collect();
        let stacks: BTreeMap<String, FontStackAsset> = request
            .assets
            .stacks
            .iter()
            .cloned()
            .map(|asset| (asset.asset_id.clone(), asset))
            .collect();
        let faces: BTreeMap<String, FontFaceAsset> = request
            .assets
            .faces
            .iter()
            .cloned()
            .map(|asset| (asset.asset_id.clone(), asset))
            .collect();
        let family_order =
            validate_font_stack(&profile.font_stack, &stacks, &families).map_err(single_failure)?;

        let mut source_bytes = BTreeMap::<String, Vec<u8>>::new();
        let mut source_hashes = BTreeMap::<String, String>::new();
        for face in faces.values() {
            let source_path = request
                .source_paths
                .get(&face.source.asset_ref)
                .ok_or_else(|| failure("FontSourceAssetRefMissing", &face.source.asset_ref))?;
            let validated = validate_font_face_source(&request.project_root, source_path, face)
                .map_err(single_failure)?;
            let bytes = fs::read(&validated.canonical_path).map_err(|error| {
                single_failure(FontDiagnostic::error(
                    "FontAssetParseFailed",
                    FontDiagnosticStage::Parse,
                    Some(face.source.asset_ref.clone()),
                    format!("Validated font source cannot be read: {error}"),
                    "Re-import the font source.",
                ))
            })?;
            source_hashes.insert(face.source.asset_ref.clone(), validated.source_sha256);
            source_bytes.insert(face.source.asset_ref.clone(), bytes);
        }
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "resolve_font_assets",
            elapsed_ms(resolve_assets_started),
        ));

        let collect_requirements_started = Instant::now();
        let collected = collect_required_text(&request, &profile).map_err(single_failure)?;
        if collected.codepoints.is_empty() {
            return Err(failure(
                "RequiredGlyphSetEmpty",
                "No reachable project text or explicit glyph source was found.",
            ));
        }
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "collect_text_requirements",
            elapsed_ms(collect_requirements_started),
        ));
        let resolve_glyphs_started = Instant::now();
        let mut resolutions = Vec::new();
        for codepoint in &collected.codepoints {
            let character = char::from_u32(*codepoint).ok_or_else(|| {
                failure(
                    "RequiredGlyphInvalid",
                    &format!("U+{codepoint:04X} is not a Unicode scalar."),
                )
            })?;
            let mut resolved = None;
            for (fallback_index, family_id) in family_order.iter().enumerate() {
                let family = &families[family_id];
                let family_face = resolve_font_family_face(family, FontStyle::Normal, 400)
                    .map_err(single_failure)?;
                let face_asset = faces
                    .get(&family_face.font_face)
                    .ok_or_else(|| failure("FontFaceMissing", family_face.font_face.as_str()))?;
                let bytes = &source_bytes[&face_asset.source.asset_ref];
                let parsed = ttf_parser::Face::parse(bytes, face_asset.source.face_index)
                    .map_err(|error| failure("FontAssetParseFailed", &format!("{error:?}")))?;
                if let Some(glyph_id) = parsed.glyph_index(character) {
                    resolved = Some(ProjectFontCodepointResolution {
                        codepoint: *codepoint,
                        font_stack_id: profile.font_stack.clone(),
                        font_family_id: family_id.clone(),
                        font_face_id: face_asset.asset_id.clone(),
                        style: face_asset.declared.style,
                        weight: face_asset.declared.weight,
                        glyph_id: glyph_id.0,
                        fallback_index: fallback_index as u16,
                    });
                    if fallback_index > 0 {
                        diagnostics.push(FontDiagnostic {
                            code: "FontFallbackResolved".to_string(),
                            severity: FontDiagnosticSeverity::Info,
                            domain: "font".to_string(),
                            stage: FontDiagnosticStage::GlyphResolve,
                            source: Some(face_asset.source.asset_ref.clone()),
                            font_face_id: Some(face_asset.asset_id.clone()),
                            font_family_id: Some(family_id.clone()),
                            font_stack_id: Some(profile.font_stack.clone()),
                            font_atlas_profile_id: Some(profile.asset_id.clone()),
                            message: format!(
                                "U+{codepoint:04X} resolved through fallback index {fallback_index}."
                            ),
                            next_action: "No action is required unless fallback was unexpected."
                                .to_string(),
                        });
                    }
                    break;
                }
            }
            let Some(resolution) = resolved else {
                return Err(single_failure(FontDiagnostic {
                    code: "RequiredGlyphMissing".to_string(),
                    severity: FontDiagnosticSeverity::Error,
                    domain: "font".to_string(),
                    stage: FontDiagnosticStage::GlyphResolve,
                    source: Some(format!("U+{codepoint:04X}")),
                    font_face_id: None,
                    font_family_id: None,
                    font_stack_id: Some(profile.font_stack.clone()),
                    font_atlas_profile_id: Some(profile.asset_id.clone()),
                    message: format!(
                        "Required glyph U+{codepoint:04X} is missing from the complete stack."
                    ),
                    next_action:
                        "Add a font face containing the glyph or remove the required text."
                            .to_string(),
                }));
            };
            resolutions.push(resolution);
        }
        resolutions.sort_by_key(|resolution| resolution.codepoint);
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "resolve_glyphs",
            elapsed_ms(resolve_glyphs_started),
        ));

        let metrics_started = Instant::now();
        let face_metrics = collect_face_metrics(&resolutions, &faces, &source_bytes)?;
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "collect_metrics",
            elapsed_ms(metrics_started),
        ));
        let bitmap_started = Instant::now();
        let hinted_variants =
            raster_hinted_variants(&profile, &resolutions, &faces, &source_bytes)?;
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "raster_bitmap",
            elapsed_ms(bitmap_started),
        ));
        let msdf_started = Instant::now();
        let msdf_variants = raster_msdf_variants(&profile, &resolutions, &faces, &source_bytes)?;
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "raster_msdf",
            elapsed_ms(msdf_started),
        ));
        let kerning_started = Instant::now();
        let kerning_adjustments =
            collect_kerning(&collected.sequences, &resolutions, &faces, &source_bytes)?;
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "collect_kerning",
            elapsed_ms(kerning_started),
        ));
        let atlas_started = Instant::now();
        let primary_atlas =
            build_primary_atlas(&profile, &resolutions, &hinted_variants, &face_metrics)?;
        substages.push(ProjectAssemblyProducerSubstageReport::completed(
            "pack_atlas",
            elapsed_ms(atlas_started),
        ));
        let dependency_digest = dependency_digest(
            &request,
            &profile,
            &source_hashes,
            &collected.canonical_texts,
        )?;
        let mut output = ProjectFontCookOutput {
            schema_version: PROJECT_FONT_COOK_OUTPUT_SCHEMA_VERSION.to_string(),
            profile_id: profile.asset_id,
            dependency_digest,
            output_digest: String::new(),
            required_codepoints: collected.codepoints.into_iter().collect(),
            resolutions,
            face_metrics,
            hinted_variants,
            msdf_variants,
            kerning_adjustments,
            primary_atlas,
            diagnostics,
        };
        output.output_digest = output_digest(&output)?;
        Ok(TimedProjectFontCookOutput { output, substages })
    }

    pub fn cook_for_runtime_package(
        project_root: &Path,
        documents: &[RuntimePackageSourceJson],
    ) -> Result<ProjectFontRuntimePackageCook, ProjectFontCookFailure> {
        Self::cook_for_runtime_package_with_cache(project_root, documents, None)
            .map(|(cook, _)| cook)
    }

    pub fn cook_for_runtime_package_with_cache(
        project_root: &Path,
        documents: &[RuntimePackageSourceJson],
        cache: Option<&ProjectAssemblyArtifactCache>,
    ) -> Result<
        (ProjectFontRuntimePackageCook, ProjectAssemblyProducerReport),
        ProjectFontCookFailure,
    > {
        Self::cook_profile_for_runtime_package_with_cache(project_root, documents, cache, None)
    }

    pub fn cook_named_profile_for_runtime_package_with_cache(
        project_root: &Path,
        documents: &[RuntimePackageSourceJson],
        cache: Option<&ProjectAssemblyArtifactCache>,
        profile_id: &str,
    ) -> Result<
        (ProjectFontRuntimePackageCook, ProjectAssemblyProducerReport),
        ProjectFontCookFailure,
    > {
        Self::cook_profile_for_runtime_package_with_cache(
            project_root,
            documents,
            cache,
            Some(profile_id),
        )
    }

    fn cook_profile_for_runtime_package_with_cache(
        project_root: &Path,
        documents: &[RuntimePackageSourceJson],
        cache: Option<&ProjectAssemblyArtifactCache>,
        profile_id: Option<&str>,
    ) -> Result<
        (ProjectFontRuntimePackageCook, ProjectAssemblyProducerReport),
        ProjectFontCookFailure,
    > {
        let total_started = Instant::now();
        let recipe_started = Instant::now();
        let prepared = prepare_project_font_cook(project_root, documents, profile_id)?;
        let recipe_duration_ms = elapsed_ms(recipe_started);
        let Some(mut prepared) = prepared else {
            let produce_started = Instant::now();
            let cook = ProjectFontRuntimePackageCook {
                legacy_atlas: Some(AuiFontAtlasCookerCmin::cook_for_documents(
                    project_root,
                    documents,
                )),
                font_bundle: None,
            };
            let produce_duration_ms = elapsed_ms(produce_started);
            return Ok((
                cook,
                ProjectAssemblyProducerReport {
                    schema_version: PROJECT_ASSEMBLY_PRODUCER_REPORT_SCHEMA_VERSION.to_string(),
                    producer_id: "font-cook".to_string(),
                    producer_recipe_version: "legacy-font-atlas-cmin.v1".to_string(),
                    status: "success".to_string(),
                    duration_ms: elapsed_ms(total_started),
                    recipe_duration_ms,
                    lookup_duration_ms: 0,
                    produce_duration_ms,
                    validate_duration_ms: 0,
                    publish_duration_ms: 0,
                    cache_status: ProjectAssemblyArtifactCacheStatus::Disabled,
                    recipe_key: None,
                    output_digest: None,
                    miss_reason: Some("legacy_font_atlas_cache_bypassed".to_string()),
                    artifact_path: None,
                    substages: vec![ProjectAssemblyProducerSubstageReport::completed(
                        "legacy_font_atlas_cook",
                        produce_duration_ms,
                    )],
                    diagnostics: Vec::new(),
                },
            ));
        };

        let recipe_key = prepared.dependency_digest.clone();
        let mut report = ProjectAssemblyProducerReport {
            schema_version: PROJECT_ASSEMBLY_PRODUCER_REPORT_SCHEMA_VERSION.to_string(),
            producer_id: "font-cook".to_string(),
            producer_recipe_version: PROJECT_FONT_RECIPE_VERSION.to_string(),
            status: "success".to_string(),
            duration_ms: 0,
            recipe_duration_ms,
            lookup_duration_ms: 0,
            produce_duration_ms: 0,
            validate_duration_ms: 0,
            publish_duration_ms: 0,
            cache_status: ProjectAssemblyArtifactCacheStatus::Disabled,
            recipe_key: Some(recipe_key.clone()),
            output_digest: None,
            miss_reason: None,
            artifact_path: None,
            substages: std::mem::take(&mut prepared.substages),
            diagnostics: Vec::new(),
        };

        if let Some(cache) = cache {
            let lookup_started = Instant::now();
            let lookup = cache.lookup_json::<ProjectFontRuntimePackageCook>(
                "font-cook",
                &recipe_key,
                PROJECT_FONT_RECIPE_VERSION,
            );
            report.lookup_duration_ms = elapsed_ms(lookup_started);
            report.cache_status = lookup.status;
            report.artifact_path = lookup
                .artifact_path
                .as_ref()
                .map(|path| path.display().to_string());
            report.miss_reason = lookup.reason.clone();
            if let (Some(cook), Some(envelope)) = (lookup.artifact, lookup.envelope) {
                let validate_started = Instant::now();
                let validated_digest = runtime_package_cook_digest(&cook)?;
                let valid = validated_digest == envelope.output_digest
                    && validate_runtime_package_cook(&cook).is_ok();
                report.validate_duration_ms = elapsed_ms(validate_started);
                if valid {
                    report.output_digest = Some(validated_digest);
                    report.substages.extend(skipped_font_produce_substages());
                    report.duration_ms = elapsed_ms(total_started);
                    return Ok((cook, report));
                }
                report.cache_status = ProjectAssemblyArtifactCacheStatus::Corrupt;
                report.miss_reason = Some("artifact_output_validation_failed".to_string());
                report.diagnostics.push(
                    "Cached FontBundle failed typed output validation and was quarantined."
                        .to_string(),
                );
                let _ = cache.quarantine("font-cook", &recipe_key);
            }
        } else {
            report.miss_reason = Some("cache_disabled".to_string());
        }

        let produce_started = Instant::now();
        let timed = Self::cook_timed(prepared.request)?;
        if timed.output.dependency_digest != recipe_key {
            return Err(failure(
                "FontRecipeDigestDrifted",
                "Prepared font recipe digest changed during deterministic production.",
            ));
        }
        report.substages.extend(timed.substages);
        let bundle_started = Instant::now();
        let font_bundle =
            crate::ProjectFontBundleBuilder::build_bitmap_v2(&prepared.profile, &timed.output)?;
        report
            .substages
            .push(ProjectAssemblyProducerSubstageReport::completed(
                "build_font_bundle",
                elapsed_ms(bundle_started),
            ));
        let cook = ProjectFontRuntimePackageCook {
            legacy_atlas: None,
            font_bundle: Some(font_bundle),
        };
        report.produce_duration_ms = elapsed_ms(produce_started);
        let output_digest = runtime_package_cook_digest(&cook)?;
        report.output_digest = Some(output_digest.clone());

        if let Some(cache) = cache {
            let publish_started = Instant::now();
            match cache.publish_json(
                "font-cook",
                PROJECT_FONT_RECIPE_VERSION,
                &recipe_key,
                &prepared.dependency_digest,
                &output_digest,
                &cook,
            ) {
                Ok(published) => {
                    report.cache_status = match published.status {
                        ProjectAssemblyArtifactPublishStatus::Produced => {
                            ProjectAssemblyArtifactCacheStatus::Produced
                        }
                        ProjectAssemblyArtifactPublishStatus::PublishRaceReused => {
                            ProjectAssemblyArtifactCacheStatus::PublishRaceReused
                        }
                    };
                    report.artifact_path = Some(published.artifact_path.display().to_string());
                }
                Err(error) => {
                    report.cache_status = ProjectAssemblyArtifactCacheStatus::Failed;
                    report.diagnostics.push(error.to_string());
                }
            }
            report.publish_duration_ms = elapsed_ms(publish_started);
        }
        report.duration_ms = elapsed_ms(total_started);
        Ok((cook, report))
    }

    pub fn discover_project_profiles(
        project_root: &Path,
    ) -> Result<Vec<ProjectFontProfileInventoryEntry>, ProjectFontCookFailure> {
        let mut profiles = Vec::new();
        for path in collect_json_paths(&project_root.join("Assets")) {
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            if value
                .get("schemaVersion")
                .and_then(serde_json::Value::as_str)
                == Some(crate::FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION)
            {
                let profile: FontAtlasProfileAsset = serde_json::from_slice(&bytes)
                    .map_err(|error| failure("FontAssetParseFailed", &error.to_string()))?;
                profiles.push(ProjectFontProfileInventoryEntry {
                    asset_id: profile.asset_id,
                    role: profile.role,
                });
            }
        }
        profiles.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
        Ok(profiles)
    }
}

fn prepare_project_font_cook(
    project_root: &Path,
    documents: &[RuntimePackageSourceJson],
    profile_id: Option<&str>,
) -> Result<Option<PreparedProjectFontCook>, ProjectFontCookFailure> {
    let load_started = Instant::now();
    let loaded = load_project_request(project_root, documents, profile_id)?;
    let load_duration_ms = elapsed_ms(load_started);
    let Some(request) = loaded else {
        return Ok(None);
    };
    let prepare_started = Instant::now();
    let diagnostics = request.assets.validate_references();
    if !diagnostics.is_empty() {
        return Err(ProjectFontCookFailure { diagnostics });
    }
    let profile = request
        .assets
        .profiles
        .iter()
        .find(|profile| profile.asset_id == request.profile_id)
        .cloned()
        .ok_or_else(|| failure("FontAtlasProfileMissing", &request.profile_id))?;
    let mut source_hashes = BTreeMap::new();
    for face in &request.assets.faces {
        let source_path = request
            .source_paths
            .get(&face.source.asset_ref)
            .ok_or_else(|| failure("FontSourceAssetRefMissing", &face.source.asset_ref))?;
        let validated =
            validate_font_face_source(project_root, source_path, face).map_err(single_failure)?;
        source_hashes.insert(face.source.asset_ref.clone(), validated.source_sha256);
    }
    let collected = collect_required_text(&request, &profile).map_err(single_failure)?;
    if collected.codepoints.is_empty() {
        return Err(failure(
            "RequiredGlyphSetEmpty",
            "No reachable project text or explicit glyph source was found.",
        ));
    }
    let dependency_digest = dependency_digest(
        &request,
        &profile,
        &source_hashes,
        &collected.canonical_texts,
    )?;
    Ok(Some(PreparedProjectFontCook {
        request,
        profile,
        dependency_digest,
        substages: vec![
            ProjectAssemblyProducerSubstageReport::completed(
                "load_project_font_request",
                load_duration_ms,
            ),
            ProjectAssemblyProducerSubstageReport::completed(
                "prepare_recipe_dependencies",
                elapsed_ms(prepare_started),
            ),
        ],
    }))
}

fn runtime_package_cook_digest(
    cook: &ProjectFontRuntimePackageCook,
) -> Result<String, ProjectFontCookFailure> {
    serde_json::to_vec(cook)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| failure("FontCanonicalEncodeFailed", &error.to_string()))
}

fn validate_runtime_package_cook(cook: &ProjectFontRuntimePackageCook) -> Result<(), String> {
    if cook.legacy_atlas.is_some() {
        return Err("Cached project FontBundle unexpectedly contains a legacy atlas.".to_string());
    }
    let bundle = cook
        .font_bundle
        .as_ref()
        .ok_or_else(|| "Cached project FontBundle payload is missing.".to_string())?;
    if bundle.metadata.schema_version != COOKED_FONT_BUNDLE_SCHEMA_VERSION {
        return Err(format!(
            "Cached FontBundle schema is {}, expected {}.",
            bundle.metadata.schema_version, COOKED_FONT_BUNDLE_SCHEMA_VERSION
        ));
    }
    if bundle.metadata.bundle_digest.is_empty() {
        return Err("Cached FontBundle digest is empty.".to_string());
    }
    if bundle.metadata.pages.len() != bundle.page_payloads.len() {
        return Err("Cached FontBundle page metadata/payload count differs.".to_string());
    }
    Ok(())
}

fn skipped_font_produce_substages() -> Vec<ProjectAssemblyProducerSubstageReport> {
    [
        "resolve_font_assets",
        "collect_text_requirements",
        "resolve_glyphs",
        "collect_metrics",
        "raster_bitmap",
        "raster_msdf",
        "collect_kerning",
        "pack_atlas",
        "build_font_bundle",
    ]
    .into_iter()
    .map(ProjectAssemblyProducerSubstageReport::skipped)
    .collect()
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

struct CollectedText {
    codepoints: BTreeSet<u32>,
    sequences: Vec<Vec<u32>>,
    canonical_texts: Vec<String>,
}

fn collect_required_text(
    request: &ProjectFontCookRequest,
    profile: &FontAtlasProfileAsset,
) -> Result<CollectedText, FontDiagnostic> {
    let mut texts = Vec::new();
    if profile.glyph_set.include_runtime_text_sources {
        for document in &request.aui_documents {
            collect_aui_strings(&document.document, &mut texts);
        }
        texts.extend(request.localization_texts.iter().cloned());
        for source in &request.text_sources {
            if source.schema_version != PROJECT_TEXT_SOURCE_SCHEMA_VERSION {
                return Err(FontDiagnostic::error(
                    "ProjectTextSourceSchemaInvalid",
                    FontDiagnosticStage::AssetResolve,
                    Some(source.asset_id.clone()),
                    "Project text source schemaVersion is invalid.",
                    "Use project-text-source.v1.",
                ));
            }
            texts.extend(source.texts.iter().cloned());
        }
    }
    texts.extend(profile.glyph_set.literals.iter().cloned());
    texts.sort();
    texts.dedup();
    let mut codepoints = BTreeSet::new();
    let mut sequences = Vec::new();
    for text in &texts {
        let sequence: Vec<u32> = text.chars().map(u32::from).collect();
        codepoints.extend(sequence.iter().copied());
        sequences.push(sequence);
    }
    let mut ranges = profile.glyph_set.unicode_ranges.clone();
    for source in &request.text_sources {
        ranges.extend(source.unicode_ranges.iter().cloned());
    }
    ranges.sort();
    ranges.dedup();
    for range in ranges {
        let (start, end) = parse_unicode_range(&range).ok_or_else(|| {
            FontDiagnostic::error(
                "FontUnicodeRangeInvalid",
                FontDiagnosticStage::AssetResolve,
                Some(range.clone()),
                "Unicode range must be U+XXXX or U+XXXX-U+YYYY.",
                "Correct the profile or Project Text Source range.",
            )
        })?;
        if end.saturating_sub(start) > 65_535 {
            return Err(FontDiagnostic::error(
                "FontUnicodeRangeTooLarge",
                FontDiagnosticStage::AssetResolve,
                Some(range),
                "A single Unicode range exceeds 65,536 codepoints.",
                "Split or narrow the explicit range.",
            ));
        }
        for codepoint in start..=end {
            if char::from_u32(codepoint).is_some() {
                codepoints.insert(codepoint);
            }
        }
    }
    Ok(CollectedText {
        codepoints,
        sequences,
        canonical_texts: texts,
    })
}

fn collect_aui_strings(value: &serde_json::Value, texts: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "text" | "placeholder" | "fallbackText" | "fallback_text"
                ) {
                    if let Some(text) = value.as_str() {
                        texts.push(text.to_string());
                    }
                } else {
                    collect_aui_strings(value, texts);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_aui_strings(value, texts);
            }
        }
        _ => {}
    }
}

fn parse_unicode_range(value: &str) -> Option<(u32, u32)> {
    let mut parts = value.split('-');
    let start = parse_codepoint(parts.next()?)?;
    let end = match parts.next() {
        Some(value) => parse_codepoint(value)?,
        None => start,
    };
    if parts.next().is_some() || start > end {
        return None;
    }
    Some((start, end))
}

fn parse_codepoint(value: &str) -> Option<u32> {
    u32::from_str_radix(value.strip_prefix("U+")?, 16).ok()
}

fn collect_face_metrics(
    resolutions: &[ProjectFontCodepointResolution],
    faces: &BTreeMap<String, FontFaceAsset>,
    source_bytes: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<ProjectFontFaceMetrics>, ProjectFontCookFailure> {
    let ids: BTreeSet<&str> = resolutions
        .iter()
        .map(|resolution| resolution.font_face_id.as_str())
        .collect();
    let mut metrics = Vec::new();
    for id in ids {
        let asset = &faces[id];
        let face = ttf_parser::Face::parse(
            &source_bytes[&asset.source.asset_ref],
            asset.source.face_index,
        )
        .map_err(|error| failure("FontAssetParseFailed", &format!("{error:?}")))?;
        let units = face.units_per_em();
        metrics.push(ProjectFontFaceMetrics {
            font_face_id: id.to_string(),
            units_per_em: units,
            ascent_per_em_millionths: per_em(face.ascender(), units),
            descent_per_em_millionths: per_em(face.descender(), units),
            line_gap_per_em_millionths: per_em(face.line_gap(), units),
        });
    }
    Ok(metrics)
}

fn raster_hinted_variants(
    profile: &FontAtlasProfileAsset,
    resolutions: &[ProjectFontCodepointResolution],
    faces: &BTreeMap<String, FontFaceAsset>,
    source_bytes: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<ProjectFontHintedGlyphVariant>, ProjectFontCookFailure> {
    let mut sizes = profile.raster.bitmap_pixel_sizes.clone();
    sizes.sort_unstable();
    sizes.dedup();
    let mut context = ScaleContext::new();
    let mut variants = Vec::new();
    for resolution in resolutions {
        let asset = &faces[&resolution.font_face_id];
        let bytes = &source_bytes[&asset.source.asset_ref];
        let parsed = ttf_parser::Face::parse(bytes, asset.source.face_index)
            .map_err(|error| failure("FontAssetParseFailed", &format!("{error:?}")))?;
        let units = parsed.units_per_em();
        let advance = parsed
            .glyph_hor_advance(ttf_parser::GlyphId(resolution.glyph_id))
            .unwrap_or_default();
        let font = FontRef::from_index(bytes, asset.source.face_index as usize)
            .ok_or_else(|| failure("FontAssetParseFailed", &asset.asset_id))?;
        for size in &sizes {
            let mut scaler = context
                .builder(font)
                .size(f32::from(*size))
                .hint(true)
                .build();
            let image = Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .render(&mut scaler, resolution.glyph_id);
            let (width, height, bearing_x, bearing_y, alpha_r8) = match image {
                Some(image) => {
                    if image.data.len() != (image.placement.width * image.placement.height) as usize
                        || image.data.iter().all(|value| *value == 0)
                    {
                        if char::from_u32(resolution.codepoint).is_some_and(char::is_whitespace) {
                            (1, 1, 0, 0, vec![0])
                        } else {
                            return Err(failure(
                                "BitmapRasterFailed",
                                &format!(
                                    "{} glyph {} at {}px produced invalid R8.",
                                    asset.asset_id, resolution.glyph_id, size
                                ),
                            ));
                        }
                    } else {
                        (
                            image.placement.width,
                            image.placement.height,
                            image.placement.left,
                            image.placement.top,
                            image.data,
                        )
                    }
                }
                None if char::from_u32(resolution.codepoint).is_some_and(char::is_whitespace) => {
                    (1, 1, 0, 0, vec![0])
                }
                None => {
                    return Err(failure(
                        "BitmapRasterFailed",
                        &format!(
                            "{} glyph {} at {}px produced no outline.",
                            asset.asset_id, resolution.glyph_id, size
                        ),
                    ));
                }
            };
            variants.push(ProjectFontHintedGlyphVariant {
                font_face_id: asset.asset_id.clone(),
                glyph_id: resolution.glyph_id,
                codepoint: resolution.codepoint,
                pixel_size: *size,
                width,
                height,
                stride: width,
                bearing_x,
                bearing_y,
                advance_per_em_millionths: per_em(advance as i16, units),
                alpha_r8,
            });
        }
    }
    variants.sort_by_key(|variant| {
        (
            variant.font_face_id.clone(),
            variant.glyph_id,
            variant.pixel_size,
        )
    });
    Ok(variants)
}

fn collect_kerning(
    sequences: &[Vec<u32>],
    resolutions: &[ProjectFontCodepointResolution],
    faces: &BTreeMap<String, FontFaceAsset>,
    source_bytes: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<ProjectFontKerningAdjustment>, ProjectFontCookFailure> {
    let by_codepoint: BTreeMap<u32, &ProjectFontCodepointResolution> = resolutions
        .iter()
        .map(|resolution| (resolution.codepoint, resolution))
        .collect();
    let mut pairs = BTreeSet::new();
    for sequence in sequences {
        for pair in sequence.windows(2) {
            pairs.insert((pair[0], pair[1]));
        }
    }
    let mut adjustments = Vec::new();
    for (left, right) in pairs {
        let (Some(left), Some(right)) = (by_codepoint.get(&left), by_codepoint.get(&right)) else {
            continue;
        };
        if left.font_face_id != right.font_face_id {
            continue;
        }
        let asset = &faces[&left.font_face_id];
        let parsed = ttf_parser::Face::parse(
            &source_bytes[&asset.source.asset_ref],
            asset.source.face_index,
        )
        .map_err(|error| failure("FontAssetParseFailed", &format!("{error:?}")))?;
        let adjustment = parsed.tables().kern.and_then(|table| {
            table
                .subtables
                .into_iter()
                .filter(|subtable| {
                    subtable.horizontal && !subtable.variable && !subtable.has_cross_stream
                })
                .find_map(|subtable| {
                    subtable.glyphs_kerning(
                        ttf_parser::GlyphId(left.glyph_id),
                        ttf_parser::GlyphId(right.glyph_id),
                    )
                })
        });
        if let Some(adjustment) = adjustment.filter(|value| *value != 0) {
            adjustments.push(ProjectFontKerningAdjustment {
                font_face_id: left.font_face_id.clone(),
                left_glyph_id: left.glyph_id,
                right_glyph_id: right.glyph_id,
                adjustment_per_em_millionths: per_em(adjustment, parsed.units_per_em()),
            });
        }
    }
    adjustments.sort_by_key(|adjustment| {
        (
            adjustment.font_face_id.clone(),
            adjustment.left_glyph_id,
            adjustment.right_glyph_id,
        )
    });
    Ok(adjustments)
}

fn raster_msdf_variants(
    profile: &FontAtlasProfileAsset,
    resolutions: &[ProjectFontCodepointResolution],
    faces: &BTreeMap<String, FontFaceAsset>,
    source_bytes: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<ProjectFontMsdfGlyphVariant>, ProjectFontCookFailure> {
    if profile.raster.policy == FontRasterPolicy::HintedBitmap {
        return Ok(Vec::new());
    }
    let width = u32::from(profile.raster.msdf_em_size);
    let height = width;
    let range = f64::from(profile.raster.msdf_pixel_range);
    let mut variants = Vec::new();
    for resolution in resolutions {
        let asset = &faces[&resolution.font_face_id];
        let bytes = &source_bytes[&asset.source.asset_ref];
        let face = ttf_parser::Face::parse(bytes, asset.source.face_index)
            .map_err(|error| failure("FontAssetParseFailed", &format!("{error:?}")))?;
        let glyph = ttf_parser::GlyphId(resolution.glyph_id);
        let units = face.units_per_em();
        let advance = face.glyph_hor_advance(glyph).unwrap_or_default();
        let Some(bbox) = face.glyph_bounding_box(glyph) else {
            if char::from_u32(resolution.codepoint).is_some_and(|ch| ch.is_whitespace()) {
                continue;
            }
            return Err(failure(
                "MsdfRasterFailed",
                &format!(
                    "{} glyph {} has no outline",
                    asset.asset_id, resolution.glyph_id
                ),
            ));
        };
        let mut shape = fdsm_ttf_parser::load_shape_from_face(&face, glyph).ok_or_else(|| {
            failure(
                "MsdfRasterFailed",
                &format!(
                    "{} glyph {} outline conversion failed",
                    asset.asset_id, glyph.0
                ),
            )
        })?;
        let glyph_width = f64::from(bbox.x_max - bbox.x_min).max(1.0);
        let glyph_height = f64::from(bbox.y_max - bbox.y_min).max(1.0);
        let scale = ((f64::from(width) - 2.0 * range) / glyph_width)
            .min((f64::from(height) - 2.0 * range) / glyph_height);
        let transform = nalgebra::convert::<_, Affine2<f64>>(Similarity2::new(
            Vector2::new(
                range - f64::from(bbox.x_min) * scale,
                range - f64::from(bbox.y_min) * scale,
            ),
            0.0,
            scale,
        ));
        shape.transform(&transform);
        let colored = Shape::edge_coloring_simple(shape, 0.03, 0xA1FE_0261 ^ u64::from(glyph.0));
        let prepared = colored.prepare();
        let mut image: ImageBuffer<Rgb<f32>, Vec<f32>> = ImageBuffer::new(width, height);
        generate_msdf(&prepared, range, &mut image);
        correct_error_msdf(
            &mut image,
            &colored,
            &prepared,
            range,
            &ErrorCorrectionConfig::default(),
        );
        correct_sign_msdf(&mut image, &prepared, FillRule::Nonzero);
        if image.as_raw().iter().any(|value| !value.is_finite()) {
            return Err(failure(
                "MsdfRasterFailed",
                &format!(
                    "{} glyph {} produced non-finite values",
                    asset.asset_id, glyph.0
                ),
            ));
        }
        // Font outlines are Y-up while cooked texture rows use a top-left origin.
        let mut rgba8 = Vec::with_capacity((width * height * 4) as usize);
        for y in (0..height).rev() {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                rgba8.extend_from_slice(&[
                    quantize_msdf(pixel[0]),
                    quantize_msdf(pixel[1]),
                    quantize_msdf(pixel[2]),
                    255,
                ]);
            }
        }
        variants.push(ProjectFontMsdfGlyphVariant {
            font_face_id: asset.asset_id.clone(),
            glyph_id: glyph.0,
            codepoint: resolution.codepoint,
            em_size: profile.raster.msdf_em_size,
            pixel_range: profile.raster.msdf_pixel_range,
            width,
            height,
            stride: width * 4,
            bearing_x: i32::from(bbox.x_min),
            bearing_y: i32::from(bbox.y_max),
            advance_per_em_millionths: per_em(advance as i16, units),
            rgba8,
        });
    }
    variants.sort_by_key(|variant| (variant.font_face_id.clone(), variant.glyph_id));
    variants.dedup_by_key(|variant| (variant.font_face_id.clone(), variant.glyph_id));
    Ok(variants)
}

fn quantize_msdf(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn build_primary_atlas(
    profile: &FontAtlasProfileAsset,
    resolutions: &[ProjectFontCodepointResolution],
    variants: &[ProjectFontHintedGlyphVariant],
    _metrics: &[ProjectFontFaceMetrics],
) -> Result<RuntimePackageSourceFontAtlas, ProjectFontCookFailure> {
    let primary_size = profile
        .raster
        .bitmap_pixel_sizes
        .iter()
        .copied()
        .find(|size| *size == 16)
        .or_else(|| profile.raster.bitmap_pixel_sizes.first().copied())
        .ok_or_else(|| failure("BitmapRasterProfileEmpty", &profile.asset_id))?;
    let width = u32::from(profile.packing.page_width);
    let height = u32::from(profile.packing.page_height);
    let padding = u32::from(profile.packing.padding);
    let mut atlas_alpha = vec![0; (width * height) as usize];
    let mut x = padding;
    let mut y = padding;
    let mut row_height = 0;
    let mut glyphs = Vec::new();
    for resolution in resolutions {
        let variant = variants
            .iter()
            .find(|variant| {
                variant.font_face_id == resolution.font_face_id
                    && variant.glyph_id == resolution.glyph_id
                    && variant.pixel_size == primary_size
            })
            .ok_or_else(|| failure("BitmapRasterVariantMissing", &resolution.font_face_id))?;
        if x + variant.width + padding > width {
            x = padding;
            y += row_height + padding;
            row_height = 0;
        }
        if y + variant.height + padding > height {
            return Err(failure(
                "FontAtlasPageOverflow",
                "Window B primary bitmap page cannot contain all required glyphs.",
            ));
        }
        for row in 0..variant.height {
            let source_start = (row * variant.stride) as usize;
            let source_end = source_start + variant.width as usize;
            let target_start = ((y + row) * width + x) as usize;
            atlas_alpha[target_start..target_start + variant.width as usize]
                .copy_from_slice(&variant.alpha_r8[source_start..source_end]);
        }
        glyphs.push(CookedFontAtlasGlyph {
            codepoint: resolution.codepoint,
            glyph_id: format!("{}:{}", resolution.font_face_id, resolution.glyph_id),
            uv_rect: [
                x as f32 / width as f32,
                y as f32 / height as f32,
                (x + variant.width) as f32 / width as f32,
                (y + variant.height) as f32 / height as f32,
            ],
            pixel_rect: [x, y, variant.width, variant.height],
            bearing_x: variant.bearing_x as f32,
            bearing_y: variant.bearing_y as f32,
            advance: variant.advance_per_em_millionths as f32 / 1_000_000.0
                * f32::from(primary_size),
            page_index: 0,
        });
        x += variant.width + padding;
        row_height = row_height.max(variant.height);
    }
    Ok(RuntimePackageSourceFontAtlas {
        metadata: CookedFontAtlasAsset {
            schema_version: COOKED_FONT_ATLAS_SCHEMA_VERSION.to_string(),
            font_atlas_id: profile.asset_id.clone(),
            font_asset_id: profile.font_stack.clone(),
            font_source_kind: "project_font_face_v2".to_string(),
            font_asset_status: "qualified".to_string(),
            atlas_image_path: format!("fonts/{}.fontatlas.r8", profile.asset_id),
            atlas_format: "r8Alpha".to_string(),
            atlas_width: width,
            atlas_height: height,
            atlas_generation: 1,
            atlas_alpha_byte_len: atlas_alpha.len(),
            glyphs,
            fallback_used: resolutions
                .iter()
                .any(|resolution| resolution.fallback_index > 0),
            diagnostics: Vec::new(),
        },
        atlas_alpha,
    })
}

fn dependency_digest(
    request: &ProjectFontCookRequest,
    profile: &FontAtlasProfileAsset,
    source_hashes: &BTreeMap<String, String>,
    texts: &[String],
) -> Result<String, ProjectFontCookFailure> {
    let asset_digest = request
        .assets
        .canonical_digest()
        .map_err(|error| failure("FontCanonicalEncodeFailed", &error.to_string()))?;
    let codepoints = texts
        .iter()
        .flat_map(|text| text.chars().map(u32::from))
        .collect::<BTreeSet<_>>();
    let kerning_pairs = texts
        .iter()
        .flat_map(|text| {
            let chars = text.chars().map(u32::from).collect::<Vec<_>>();
            chars
                .windows(2)
                .map(|pair| [pair[0], pair[1]])
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    hash_json(&serde_json::json!({
        "recipeVersion": PROJECT_FONT_RECIPE_VERSION,
        "profile": profile,
        "assetDigest": asset_digest,
        "sourceHashes": source_hashes,
        "codepoints": codepoints,
        "kerningPairs": kerning_pairs,
    }))
}

fn output_digest(output: &ProjectFontCookOutput) -> Result<String, ProjectFontCookFailure> {
    hash_json(&serde_json::json!({
        "schemaVersion": output.schema_version,
        "profileId": output.profile_id,
        "dependencyDigest": output.dependency_digest,
        "requiredCodepoints": output.required_codepoints,
        "resolutions": output.resolutions,
        "faceMetrics": output.face_metrics,
        "hintedVariants": output.hinted_variants,
        "msdfVariants": output.msdf_variants,
        "kerningAdjustments": output.kerning_adjustments,
        "primaryAtlas": {
            "metadata": output.primary_atlas.metadata,
            "alphaSha256": sha256_prefixed(&output.primary_atlas.atlas_alpha),
        },
    }))
}

fn hash_json(value: &serde_json::Value) -> Result<String, ProjectFontCookFailure> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| failure("FontCanonicalEncodeFailed", &error.to_string()))
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn per_em(value: i16, units: u16) -> i32 {
    ((i64::from(value) * 1_000_000) / i64::from(units.max(1))) as i32
}

fn load_project_request(
    project_root: &Path,
    documents: &[RuntimePackageSourceJson],
    requested_profile_id: Option<&str>,
) -> Result<Option<ProjectFontCookRequest>, ProjectFontCookFailure> {
    let mut assets = ProjectFontAssetSet::default();
    let mut text_sources = Vec::new();
    let mut localization_texts = Vec::new();
    for path in collect_json_paths(&project_root.join("Assets")) {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        match value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str)
        {
            Some(crate::FONT_FACE_ASSET_SCHEMA_VERSION) => parse_push(&bytes, &mut assets.faces)?,
            Some(crate::FONT_FAMILY_ASSET_SCHEMA_VERSION) => {
                parse_push(&bytes, &mut assets.families)?
            }
            Some(crate::FONT_STACK_ASSET_SCHEMA_VERSION) => parse_push(&bytes, &mut assets.stacks)?,
            Some(crate::FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION) => {
                parse_push(&bytes, &mut assets.profiles)?
            }
            Some(PROJECT_TEXT_SOURCE_SCHEMA_VERSION) => parse_push(&bytes, &mut text_sources)?,
            _ => {}
        }
    }
    for path in collect_json_paths(&project_root.join("Localization")) {
        if let Ok(value) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .ok_or(())
        {
            collect_all_strings(&value, &mut localization_texts);
        }
    }
    if assets.profiles.is_empty() {
        return Ok(None);
    }
    let database_path = project_root.join(ASSET_DATABASE_PATH);
    let database: AssetDatabaseDocument =
        serde_json::from_slice(&fs::read(&database_path).map_err(|error| {
            failure(
                "FontAssetDatabaseMissing",
                &format!("{}: {error}", database_path.display()),
            )
        })?)
        .map_err(|error| failure("FontAssetDatabaseInvalid", &error.to_string()))?;
    let source_paths = database
        .assets
        .iter()
        .map(|record| (record.asset_id.clone(), PathBuf::from(&record.source_path)))
        .collect();
    let profile = match requested_profile_id {
        Some(profile_id) => assets
            .profiles
            .iter()
            .find(|profile| profile.asset_id == profile_id)
            .map(|profile| profile.asset_id.clone())
            .ok_or_else(|| failure("FontAtlasProfileMissing", profile_id))?,
        None => assets
            .profiles
            .iter()
            .find(|profile| profile.role == crate::FontAtlasProfileRole::DefaultUi)
            .map(|profile| profile.asset_id.clone())
            .ok_or_else(|| failure("FontDefaultUiStackInvalid", "defaultUi profile missing"))?,
    };
    Ok(Some(ProjectFontCookRequest {
        project_root: project_root.to_path_buf(),
        assets,
        source_paths,
        aui_documents: documents.to_vec(),
        localization_texts,
        text_sources,
        profile_id: profile,
    }))
}

fn parse_push<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    target: &mut Vec<T>,
) -> Result<(), ProjectFontCookFailure> {
    target.push(
        serde_json::from_slice(bytes)
            .map_err(|error| failure("FontAssetParseFailed", &error.to_string()))?,
    );
    Ok(())
}

fn collect_json_paths(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, paths: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, paths);
            } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
                paths.push(path);
            }
        }
    }
    let mut paths = Vec::new();
    visit(root, &mut paths);
    paths.sort();
    paths
}

fn collect_all_strings(value: &serde_json::Value, texts: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) => texts.push(value.clone()),
        serde_json::Value::Array(values) => {
            for value in values {
                collect_all_strings(value, texts);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_all_strings(value, texts);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FontAtlasProfileRole, FontFaceDeclaredMetadata, FontFaceSource, FontFamilyFace,
        FontGlyphSet, FontHintingMode, FontMissingGlyphPolicy, FontMissingStylePolicy,
        FontPackingProfile, FontRasterPolicy, FontRasterProfile, FontSourceKind,
        FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION, FONT_FACE_ASSET_SCHEMA_VERSION,
        FONT_FAMILY_ASSET_SCHEMA_VERSION, FONT_STACK_ASSET_SCHEMA_VERSION,
    };
    use engine_runtime::font_bundle::FontBundleRenderMode;

    const SOURCE_ID: &str = "font-source-qualification";
    const FACE_ID: &str = "font-face-qualification";
    const FAMILY_ID: &str = "font-family-ui";
    const STACK_ID: &str = "font-stack-ui";
    const PROFILE_ID: &str = "font-profile-ui";
    const SOURCE_HASH: &str =
        "sha256:f70ecb32e5b312ba7bc724977352139a3f691566dc2491377be3828631c9fab2";

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/qualification")
            .canonicalize()
            .expect("qualification fixture root")
    }

    fn request(aui_text: &str) -> ProjectFontCookRequest {
        let face = FontFaceAsset {
            schema_version: FONT_FACE_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: FACE_ID.to_string(),
            source: FontFaceSource {
                kind: FontSourceKind::ProjectFile,
                asset_ref: SOURCE_ID.to_string(),
                face_index: 0,
                source_sha256: SOURCE_HASH.to_string(),
            },
            declared: FontFaceDeclaredMetadata {
                family: "Aife Noto Sans SC Qualification".to_string(),
                style: FontStyle::Normal,
                weight: 400,
                stretch: 100,
            },
            hinting: FontHintingMode::FontDefault,
        };
        let family = FontFamilyAsset {
            schema_version: FONT_FAMILY_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: FAMILY_ID.to_string(),
            faces: vec![FontFamilyFace {
                font_face: FACE_ID.to_string(),
                style: FontStyle::Normal,
                weight: 400,
            }],
            missing_style_policy: FontMissingStylePolicy::NearestWeightSameStyle,
        };
        let stack = FontStackAsset {
            schema_version: FONT_STACK_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: STACK_ID.to_string(),
            families: vec![FAMILY_ID.to_string()],
            missing_glyph_policy: FontMissingGlyphPolicy::Error,
            replacement_codepoint: "U+FFFD".to_string(),
        };
        let profile = FontAtlasProfileAsset {
            schema_version: FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: PROFILE_ID.to_string(),
            role: FontAtlasProfileRole::DefaultUi,
            font_stack: STACK_ID.to_string(),
            glyph_set: FontGlyphSet {
                include_runtime_text_sources: true,
                unicode_ranges: Vec::new(),
                literals: Vec::new(),
                locales: vec!["zh-CN".to_string()],
            },
            raster: FontRasterProfile {
                policy: FontRasterPolicy::AutoHybrid,
                bitmap_pixel_sizes: vec![16],
                bitmap_hinting: FontHintingMode::FontDefault,
                msdf_em_size: 64,
                msdf_pixel_range: 8,
            },
            packing: FontPackingProfile {
                page_width: 128,
                page_height: 128,
                padding: 1,
                max_bitmap_pages: 4,
                max_msdf_pages: 4,
            },
        };
        ProjectFontCookRequest {
            project_root: fixture_root(),
            assets: ProjectFontAssetSet {
                faces: vec![face],
                families: vec![family],
                stacks: vec![stack],
                profiles: vec![profile],
            },
            source_paths: BTreeMap::from([(
                SOURCE_ID.to_string(),
                PathBuf::from("AifeNotoSansSCQualification-Regular.ttf"),
            )]),
            aui_documents: vec![RuntimePackageSourceJson {
                id: "hud".to_string(),
                document: serde_json::json!({
                    "root": {
                        "text": aui_text,
                        "children": [{"placeholder": "A1?"}]
                    }
                }),
            }],
            localization_texts: vec!["界".to_string()],
            text_sources: vec![ProjectTextSourceAsset {
                schema_version: PROJECT_TEXT_SOURCE_SCHEMA_VERSION.to_string(),
                asset_id: "dynamic-combat-text".to_string(),
                texts: vec!["曲".to_string()],
                unicode_ranges: Vec::new(),
            }],
            profile_id: PROFILE_ID.to_string(),
        }
    }

    #[test]
    fn project_font_cook_collects_real_cjk_and_is_deterministic() {
        let first = ProjectFontCookModule::cook(request("A1?中")).expect("first cook");
        let second = ProjectFontCookModule::cook(request("A1?中")).expect("second cook");
        for character in ['A', '1', '?', '中', '界', '曲'] {
            assert!(first.required_codepoints.contains(&u32::from(character)));
        }
        let cjk = first
            .resolutions
            .iter()
            .find(|resolution| resolution.codepoint == u32::from('中'))
            .expect("CJK resolution");
        let replacement = first
            .resolutions
            .iter()
            .find(|resolution| resolution.codepoint == u32::from('?'))
            .expect("replacement resolution");
        assert_ne!(cjk.glyph_id, replacement.glyph_id);
        assert_eq!(first.dependency_digest, second.dependency_digest);
        assert_eq!(first.output_digest, second.output_digest);
        assert_eq!(first.hinted_variants, second.hinted_variants);
        assert_eq!(
            serde_json::to_vec(&first.primary_atlas).unwrap(),
            serde_json::to_vec(&second.primary_atlas).unwrap()
        );
        assert_eq!(
            first.primary_atlas.metadata.font_source_kind,
            "project_font_face_v2"
        );
    }

    #[test]
    fn project_font_glyph_collection_changes_dependency_digest() {
        let first = ProjectFontCookModule::cook(request("A1?中")).expect("first cook");
        let second = ProjectFontCookModule::cook(request("A2?中")).expect("changed text cook");
        assert_ne!(first.required_codepoints, second.required_codepoints);
        assert_ne!(first.dependency_digest, second.dependency_digest);
        assert_ne!(first.output_digest, second.output_digest);
    }

    #[test]
    fn project_font_hot_reload_reuses_dependency_when_glyphs_and_pairs_are_unchanged() {
        let first = ProjectFontCookModule::cook(request("A1A1A")).expect("first cook");
        let reordered = ProjectFontCookModule::cook(request("1A1A1")).expect("reordered cook");
        let added = ProjectFontCookModule::cook(request("A1A12")).expect("added glyph cook");

        assert_eq!(first.required_codepoints, reordered.required_codepoints);
        assert_eq!(first.dependency_digest, reordered.dependency_digest);
        assert_ne!(first.dependency_digest, added.dependency_digest);
    }

    #[test]
    fn project_font_cook_required_glyph_missing_fails_closed() {
        let error = ProjectFontCookModule::cook(request("\u{9f98}"))
            .expect_err("fixture intentionally lacks U+9F98");
        assert!(error
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "RequiredGlyphMissing"
                && diagnostic.stage == FontDiagnosticStage::GlyphResolve));
    }

    #[test]
    fn project_font_hinting_r8_has_valid_stride_metrics_and_real_pixels() {
        let output = ProjectFontCookModule::cook(request("中")).expect("hinted cook");
        let variant = output
            .hinted_variants
            .iter()
            .find(|variant| variant.codepoint == u32::from('中'))
            .expect("hinted CJK variant");
        assert!(variant.width > 0 && variant.height > 0);
        assert_eq!(variant.stride, variant.width);
        assert_eq!(
            variant.alpha_r8.len(),
            (variant.stride * variant.height) as usize
        );
        assert!(variant.alpha_r8.iter().any(|alpha| *alpha != 0));
        assert!(variant.advance_per_em_millionths > 0);
        assert!(output
            .face_metrics
            .iter()
            .all(|metrics| metrics.units_per_em > 0));
    }

    #[test]
    fn project_font_msdf_is_deterministic_and_nonempty() {
        let first = ProjectFontCookModule::cook(request("中")).expect("first MSDF cook");
        let second = ProjectFontCookModule::cook(request("中")).expect("second MSDF cook");
        let first = first
            .msdf_variants
            .iter()
            .find(|variant| variant.codepoint == u32::from('中'))
            .expect("CJK MSDF");
        let second = second
            .msdf_variants
            .iter()
            .find(|variant| variant.codepoint == u32::from('中'))
            .expect("second CJK MSDF");
        assert_eq!(first, second);
        assert_eq!(first.stride, first.width * 4);
        assert_eq!(first.rgba8.len(), (first.stride * first.height) as usize);
        assert!(first
            .rgba8
            .chunks_exact(4)
            .any(|pixel| pixel[0] != pixel[1] || pixel[1] != pixel[2]));
    }

    #[test]
    fn project_font_msdf_orientation_matches_bitmap_asymmetric_a() {
        let output = ProjectFontCookModule::cook(request("A")).expect("hybrid A cook");
        let bitmap = output
            .hinted_variants
            .iter()
            .find(|variant| variant.codepoint == u32::from('A'))
            .expect("bitmap A");
        let msdf = output
            .msdf_variants
            .iter()
            .find(|variant| variant.codepoint == u32::from('A'))
            .expect("MSDF A");

        let bitmap_top = bitmap
            .alpha_r8
            .chunks_exact(bitmap.stride as usize)
            .take(bitmap.height as usize / 2)
            .flatten()
            .filter(|alpha| **alpha > 32)
            .count();
        let bitmap_bottom = bitmap
            .alpha_r8
            .chunks_exact(bitmap.stride as usize)
            .skip(bitmap.height as usize / 2)
            .flatten()
            .filter(|alpha| **alpha > 32)
            .count();
        assert_ne!(
            bitmap_top, bitmap_bottom,
            "fixture A must be vertically asymmetric"
        );

        let filled_per_row = msdf
            .rgba8
            .chunks_exact(msdf.stride as usize)
            .map(|row| {
                row.chunks_exact(4)
                    .filter(|pixel| {
                        let mut channels = [pixel[0], pixel[1], pixel[2]];
                        channels.sort_unstable();
                        channels[1] > 128
                    })
                    .count()
            })
            .collect::<Vec<_>>();
        let msdf_top: usize = filled_per_row.iter().take(msdf.height as usize / 2).sum();
        let msdf_bottom: usize = filled_per_row.iter().skip(msdf.height as usize / 2).sum();
        assert_eq!(
            msdf_top.cmp(&msdf_bottom),
            bitmap_top.cmp(&bitmap_bottom),
            "MSDF A must use the same top-left row orientation as bitmap A: bitmap={bitmap_top}/{bitmap_bottom}, msdf={msdf_top}/{msdf_bottom}"
        );
    }

    #[test]
    fn project_font_variant_metrics_are_shared_between_bitmap_and_msdf() {
        let output = ProjectFontCookModule::cook(request("中")).expect("hybrid cook");
        let bitmap = output
            .hinted_variants
            .iter()
            .find(|variant| variant.codepoint == u32::from('中'))
            .expect("bitmap variant");
        let msdf = output
            .msdf_variants
            .iter()
            .find(|variant| variant.codepoint == u32::from('中'))
            .expect("MSDF variant");
        assert_eq!(
            bitmap.advance_per_em_millionths,
            msdf.advance_per_em_millionths
        );
    }

    #[test]
    fn project_font_msdf_bundle_uses_rgba8_multi_page() {
        let mut request = request("中");
        request.assets.profiles[0].packing.page_width = 130;
        request.assets.profiles[0].packing.page_height = 130;
        request.assets.profiles[0].packing.max_msdf_pages = 16;
        let profile = request.assets.profiles[0].clone();
        let output = ProjectFontCookModule::cook(request).expect("hybrid cook");
        let source =
            crate::ProjectFontBundleBuilder::build_bitmap_v2(&profile, &output).expect("bundle");
        let msdf_pages = source
            .metadata
            .pages
            .iter()
            .filter(|page| page.render_mode == FontBundleRenderMode::MsdfRgba8)
            .collect::<Vec<_>>();
        assert!(msdf_pages.len() > 1);
        assert!(msdf_pages.iter().all(|page| page.format == "rgba8Unorm"));
        engine_runtime::font_bundle::RuntimeFontBundleLoader::load(source)
            .expect("hybrid bundle must load");
    }

    #[test]
    fn project_font_cook_reuses_same_face_and_variant_key() {
        let output = ProjectFontCookModule::cook(request("中中中")).expect("deduplicated cook");
        let keys: BTreeSet<_> = output
            .hinted_variants
            .iter()
            .map(|variant| {
                (
                    variant.font_face_id.as_str(),
                    variant.glyph_id,
                    variant.pixel_size,
                )
            })
            .collect();
        assert_eq!(keys.len(), output.hinted_variants.len());
        assert_eq!(
            output
                .hinted_variants
                .iter()
                .filter(|variant| variant.codepoint == u32::from('中'))
                .count(),
            1
        );
    }

    #[test]
    fn project_font_cook_accepts_additional_profile_without_project_default() {
        let mut request = request("中");
        request.assets.profiles[0].role = FontAtlasProfileRole::Additional;
        let output = ProjectFontCookModule::cook(request).expect("additional profile cook");
        assert!(output.required_codepoints.contains(&u32::from('中')));
    }
}

fn failure(code: &str, source: &str) -> ProjectFontCookFailure {
    single_failure(FontDiagnostic::error(
        code,
        FontDiagnosticStage::AssetResolve,
        Some(source.to_string()),
        source,
        "Inspect the font asset graph and correct the referenced input.",
    ))
}

fn single_failure(diagnostic: FontDiagnostic) -> ProjectFontCookFailure {
    ProjectFontCookFailure {
        diagnostics: vec![diagnostic],
    }
}
