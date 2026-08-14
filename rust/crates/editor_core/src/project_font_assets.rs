use crate::{AssetGraphDocument, AssetGraphNode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const FONT_FACE_ASSET_SCHEMA_VERSION: &str = "font-face-asset.v2";
pub const FONT_FAMILY_ASSET_SCHEMA_VERSION: &str = "font-family-asset.v1";
pub const FONT_STACK_ASSET_SCHEMA_VERSION: &str = "font-stack-asset.v1";
pub const FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION: &str = "font-atlas-profile-asset.v1";
pub const PROJECT_FONT_ASSET_GRAPH_SCHEMA_VERSION: &str = "project-font-asset-graph.v1";
pub const PROJECT_FONT_RECIPE_VERSION: &str =
    "project-font-recipe.v1;swash=0.2.10;fdsm=0.8.0;fdsm-ttf-parser=0.2.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontFaceAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub source: FontFaceSource,
    pub declared: FontFaceDeclaredMetadata,
    pub hinting: FontHintingMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontFaceSource {
    pub kind: FontSourceKind,
    pub asset_ref: String,
    pub face_index: u32,
    pub source_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontSourceKind {
    ProjectFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontFaceDeclaredMetadata {
    pub family: String,
    pub style: FontStyle,
    pub weight: u16,
    pub stretch: u16,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontHintingMode {
    FontDefault,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontFamilyAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub faces: Vec<FontFamilyFace>,
    pub missing_style_policy: FontMissingStylePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontFamilyFace {
    pub font_face: String,
    pub style: FontStyle,
    pub weight: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontMissingStylePolicy {
    NearestWeightSameStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontStackAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub families: Vec<String>,
    pub missing_glyph_policy: FontMissingGlyphPolicy,
    pub replacement_codepoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontMissingGlyphPolicy {
    Error,
    Replacement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontAtlasProfileAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub role: FontAtlasProfileRole,
    pub font_stack: String,
    pub glyph_set: FontGlyphSet,
    pub raster: FontRasterProfile,
    pub packing: FontPackingProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontAtlasProfileRole {
    DefaultUi,
    Additional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontGlyphSet {
    pub include_runtime_text_sources: bool,
    pub unicode_ranges: Vec<String>,
    pub literals: Vec<String>,
    pub locales: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontRasterProfile {
    pub policy: FontRasterPolicy,
    pub bitmap_pixel_sizes: Vec<u16>,
    pub bitmap_hinting: FontHintingMode,
    pub msdf_em_size: u16,
    pub msdf_pixel_range: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontRasterPolicy {
    AutoHybrid,
    HintedBitmap,
    Msdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontPackingProfile {
    pub page_width: u16,
    pub page_height: u16,
    pub padding: u16,
    pub max_bitmap_pages: u16,
    pub max_msdf_pages: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontReportLevel {
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FontDiagnosticStage {
    AssetResolve,
    Parse,
    GlyphResolve,
    Raster,
    Pack,
    Package,
    Load,
    GpuPrepare,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontDiagnostic {
    pub code: String,
    pub severity: FontDiagnosticSeverity,
    pub domain: String,
    pub stage: FontDiagnosticStage,
    pub source: Option<String>,
    pub font_face_id: Option<String>,
    pub font_family_id: Option<String>,
    pub font_stack_id: Option<String>,
    pub font_atlas_profile_id: Option<String>,
    pub message: String,
    pub next_action: String,
}

impl FontDiagnostic {
    pub(crate) fn error(
        code: &str,
        stage: FontDiagnosticStage,
        source: Option<String>,
        message: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            severity: FontDiagnosticSeverity::Error,
            domain: "font".to_string(),
            stage,
            source,
            font_face_id: None,
            font_family_id: None,
            font_stack_id: None,
            font_atlas_profile_id: None,
            message: message.into(),
            next_action: next_action.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFontFaceSource {
    pub canonical_path: PathBuf,
    pub source_sha256: String,
    pub face_count: u32,
}

pub fn validate_font_face_source(
    project_root: &Path,
    source_path: &Path,
    asset: &FontFaceAsset,
) -> Result<ValidatedFontFaceSource, FontDiagnostic> {
    validate_face_schema(asset)?;
    let canonical_root = fs::canonicalize(project_root).map_err(|error| {
        FontDiagnostic::error(
            "FontSourcePathInvalid",
            FontDiagnosticStage::AssetResolve,
            Some(project_root.display().to_string()),
            format!("Project root cannot be resolved: {error}"),
            "Select an existing project root.",
        )
    })?;
    if source_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(source_path_invalid(source_path));
    }
    let candidate = if source_path.is_absolute() {
        source_path.to_path_buf()
    } else {
        canonical_root.join(source_path)
    };
    let canonical_path = fs::canonicalize(&candidate).map_err(|error| {
        FontDiagnostic::error(
            "FontSourcePathInvalid",
            FontDiagnosticStage::AssetResolve,
            Some(candidate.display().to_string()),
            format!("Font source cannot be resolved: {error}"),
            "Import the font into the project and update its AssetRef.",
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(source_path_invalid(&canonical_path));
    }
    let bytes = fs::read(&canonical_path).map_err(|error| {
        FontDiagnostic::error(
            "FontAssetParseFailed",
            FontDiagnosticStage::Parse,
            Some(asset.source.asset_ref.clone()),
            format!("Font source cannot be read: {error}"),
            "Re-import the font source.",
        )
    })?;
    let actual_hash = sha256_prefixed(&bytes);
    if actual_hash != asset.source.source_sha256.to_ascii_lowercase() {
        let mut diagnostic = FontDiagnostic::error(
            "FontSourceHashMismatch",
            FontDiagnosticStage::Parse,
            Some(asset.source.asset_ref.clone()),
            format!(
                "Expected {}, found {actual_hash}.",
                asset.source.source_sha256
            ),
            "Re-import the source or restore the recorded font bytes.",
        );
        diagnostic.font_face_id = Some(asset.asset_id.clone());
        return Err(diagnostic);
    }
    let face_count = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
    let face = ttf_parser::Face::parse(&bytes, asset.source.face_index).map_err(|error| {
        let mut diagnostic = FontDiagnostic::error(
            "FontFaceIndexOutOfRange",
            FontDiagnosticStage::Parse,
            Some(asset.source.asset_ref.clone()),
            format!(
                "Face index {} cannot be parsed: {error:?}.",
                asset.source.face_index
            ),
            "Select a valid TTF/OTF face or a valid TTC/OTC face index.",
        );
        diagnostic.font_face_id = Some(asset.asset_id.clone());
        diagnostic
    })?;
    validate_declared_metadata(asset, &face)?;
    Ok(ValidatedFontFaceSource {
        canonical_path,
        source_sha256: actual_hash,
        face_count,
    })
}

fn validate_face_schema(asset: &FontFaceAsset) -> Result<(), FontDiagnostic> {
    if asset.schema_version != FONT_FACE_ASSET_SCHEMA_VERSION
        || !valid_asset_id(&asset.asset_id)
        || !valid_asset_id(&asset.source.asset_ref)
        || asset.declared.family.trim().is_empty()
        || !(1..=1000).contains(&asset.declared.weight)
        || !(50..=200).contains(&asset.declared.stretch)
        || !valid_sha256(&asset.source.source_sha256)
    {
        let mut diagnostic = FontDiagnostic::error(
            "FontAssetSchemaInvalid",
            FontDiagnosticStage::AssetResolve,
            Some(asset.source.asset_ref.clone()),
            "FontFaceAsset does not satisfy the v2 schema constraints.",
            "Correct schemaVersion, stable ids, SHA-256, family, weight, and stretch.",
        );
        diagnostic.font_face_id = Some(asset.asset_id.clone());
        return Err(diagnostic);
    }
    Ok(())
}

fn validate_declared_metadata(
    asset: &FontFaceAsset,
    face: &ttf_parser::Face<'_>,
) -> Result<(), FontDiagnostic> {
    let parsed_style = if face.is_italic() {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    let parsed_weight = face.weight().to_number();
    let declared_matches =
        parsed_style == asset.declared.style && parsed_weight == asset.declared.weight;
    if !declared_matches {
        let mut diagnostic = FontDiagnostic::error(
            "FontDeclaredMetadataMismatch",
            FontDiagnosticStage::Parse,
            Some(asset.source.asset_ref.clone()),
            format!(
                "Declared style/weight {:?}/{} does not match parsed {:?}/{}.",
                asset.declared.style, asset.declared.weight, parsed_style, parsed_weight
            ),
            "Use the parsed face metadata or select the intended face index.",
        );
        diagnostic.font_face_id = Some(asset.asset_id.clone());
        return Err(diagnostic);
    }
    Ok(())
}

fn source_path_invalid(path: &Path) -> FontDiagnostic {
    FontDiagnostic::error(
        "FontSourcePathInvalid",
        FontDiagnosticStage::AssetResolve,
        Some(path.display().to_string()),
        "Font source resolves outside the project root.",
        "Import the font into the project and reference its project AssetRef.",
    )
}

pub fn resolve_font_family_face<'a>(
    family: &'a FontFamilyAsset,
    style: FontStyle,
    weight: u16,
) -> Result<&'a FontFamilyFace, FontDiagnostic> {
    let mut candidates: Vec<&FontFamilyFace> = family
        .faces
        .iter()
        .filter(|face| face.style == style)
        .collect();
    if candidates.is_empty() && style != FontStyle::Normal {
        candidates = family
            .faces
            .iter()
            .filter(|face| face.style == FontStyle::Normal)
            .collect();
    }
    candidates.sort_by_key(|face| {
        (
            face.weight.abs_diff(weight),
            face.weight,
            face.font_face.as_str(),
        )
    });
    candidates.first().copied().ok_or_else(|| {
        let mut diagnostic = FontDiagnostic::error(
            "FontFamilyResolutionFailed",
            FontDiagnosticStage::AssetResolve,
            Some(family.asset_id.clone()),
            format!("No face resolves {:?} weight {weight}.", style),
            "Add an exact or nearest-weight face to the family.",
        );
        diagnostic.font_family_id = Some(family.asset_id.clone());
        diagnostic
    })
}

pub fn validate_font_stack(
    stack_id: &str,
    stacks: &BTreeMap<String, FontStackAsset>,
    families: &BTreeMap<String, FontFamilyAsset>,
) -> Result<Vec<String>, FontDiagnostic> {
    fn visit(
        id: &str,
        stacks: &BTreeMap<String, FontStackAsset>,
        families: &BTreeMap<String, FontFamilyAsset>,
        visiting: &mut BTreeSet<String>,
        resolved: &mut Vec<String>,
    ) -> Result<(), FontDiagnostic> {
        if !visiting.insert(id.to_string()) {
            let mut diagnostic = FontDiagnostic::error(
                "FontStackCycle",
                FontDiagnosticStage::AssetResolve,
                Some(id.to_string()),
                format!("Font stack cycle reaches {id}."),
                "Remove the cyclic stack reference.",
            );
            diagnostic.font_stack_id = Some(id.to_string());
            return Err(diagnostic);
        }
        let stack = stacks.get(id).ok_or_else(|| {
            let mut diagnostic = FontDiagnostic::error(
                "FontStackMissing",
                FontDiagnosticStage::AssetResolve,
                Some(id.to_string()),
                format!("Font stack {id} is missing."),
                "Create the referenced stack or update the AssetRef.",
            );
            diagnostic.font_stack_id = Some(id.to_string());
            diagnostic
        })?;
        for reference in &stack.families {
            if families.contains_key(reference) {
                resolved.push(reference.clone());
            } else if stacks.contains_key(reference) {
                visit(reference, stacks, families, visiting, resolved)?;
            } else {
                let mut diagnostic = FontDiagnostic::error(
                    "FontFamilyMissing",
                    FontDiagnosticStage::AssetResolve,
                    Some(reference.clone()),
                    format!("Stack {id} references missing family {reference}."),
                    "Create the family or update the stack AssetRef.",
                );
                diagnostic.font_stack_id = Some(id.to_string());
                diagnostic.font_family_id = Some(reference.clone());
                return Err(diagnostic);
            }
        }
        visiting.remove(id);
        Ok(())
    }

    let mut resolved = Vec::new();
    visit(
        stack_id,
        stacks,
        families,
        &mut BTreeSet::new(),
        &mut resolved,
    )?;
    Ok(resolved)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectFontAssetSet {
    pub faces: Vec<FontFaceAsset>,
    pub families: Vec<FontFamilyAsset>,
    pub stacks: Vec<FontStackAsset>,
    pub profiles: Vec<FontAtlasProfileAsset>,
}

impl ProjectFontAssetSet {
    pub fn canonical_digest(&self) -> Result<String, serde_json::Error> {
        let mut normalized = self.clone();
        normalized.faces.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        normalized
            .families
            .sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        normalized
            .stacks
            .sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        normalized
            .profiles
            .sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        let bytes = serde_json::to_vec(&(
            PROJECT_FONT_RECIPE_VERSION,
            normalized.faces,
            normalized.families,
            normalized.stacks,
            normalized.profiles,
        ))?;
        Ok(sha256_prefixed(&bytes))
    }

    pub fn validate_references(&self) -> Vec<FontDiagnostic> {
        let mut diagnostics = Vec::new();
        let mut asset_ids = BTreeSet::new();
        for asset_id in self
            .faces
            .iter()
            .map(|asset| asset.asset_id.as_str())
            .chain(self.families.iter().map(|asset| asset.asset_id.as_str()))
            .chain(self.stacks.iter().map(|asset| asset.asset_id.as_str()))
            .chain(self.profiles.iter().map(|asset| asset.asset_id.as_str()))
        {
            if !asset_ids.insert(asset_id) {
                diagnostics.push(FontDiagnostic::error(
                    "FontAssetIdDuplicate",
                    FontDiagnosticStage::AssetResolve,
                    Some(asset_id.to_string()),
                    format!("Stable font asset id {asset_id} is duplicated."),
                    "Assign a unique stable asset id.",
                ));
            }
        }
        for face in &self.faces {
            if let Err(diagnostic) = validate_face_schema(face) {
                diagnostics.push(diagnostic);
            }
        }
        for family in &self.families {
            if family.schema_version != FONT_FAMILY_ASSET_SCHEMA_VERSION
                || !valid_asset_id(&family.asset_id)
                || family.faces.is_empty()
                || family.faces.iter().any(|face| {
                    !valid_asset_id(&face.font_face) || !(1..=1000).contains(&face.weight)
                })
            {
                let mut diagnostic = FontDiagnostic::error(
                    "FontAssetSchemaInvalid",
                    FontDiagnosticStage::AssetResolve,
                    Some(family.asset_id.clone()),
                    "FontFamilyAsset does not satisfy the v1 schema constraints.",
                    "Correct schemaVersion, stable ids, faces, and weights.",
                );
                diagnostic.font_family_id = Some(family.asset_id.clone());
                diagnostics.push(diagnostic);
            }
        }
        for stack in &self.stacks {
            let replacement_is_valid = parse_unicode_scalar(&stack.replacement_codepoint).is_some();
            if stack.schema_version != FONT_STACK_ASSET_SCHEMA_VERSION
                || !valid_asset_id(&stack.asset_id)
                || stack.families.is_empty()
                || stack.families.iter().any(|family| !valid_asset_id(family))
                || !replacement_is_valid
            {
                let mut diagnostic = FontDiagnostic::error(
                    "FontAssetSchemaInvalid",
                    FontDiagnosticStage::AssetResolve,
                    Some(stack.asset_id.clone()),
                    "FontStackAsset does not satisfy the v1 schema constraints.",
                    "Correct schemaVersion, stable ids, families, and replacementCodepoint.",
                );
                diagnostic.font_stack_id = Some(stack.asset_id.clone());
                diagnostics.push(diagnostic);
            }
        }
        for profile in &self.profiles {
            if profile.schema_version != FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION
                || !valid_asset_id(&profile.asset_id)
                || !valid_asset_id(&profile.font_stack)
                || profile.raster.msdf_em_size == 0
                || profile.raster.msdf_pixel_range == 0
                || profile.packing.page_width == 0
                || profile.packing.page_height == 0
                || profile.packing.max_bitmap_pages == 0
                || profile.packing.max_msdf_pages == 0
                || profile
                    .raster
                    .bitmap_pixel_sizes
                    .iter()
                    .any(|size| *size == 0)
            {
                let mut diagnostic = FontDiagnostic::error(
                    "FontAssetSchemaInvalid",
                    FontDiagnosticStage::AssetResolve,
                    Some(profile.asset_id.clone()),
                    "FontAtlasProfileAsset does not satisfy the v1 schema constraints.",
                    "Correct schemaVersion, stable ids, raster sizes, and page budgets.",
                );
                diagnostic.font_atlas_profile_id = Some(profile.asset_id.clone());
                diagnostics.push(diagnostic);
            }
        }
        let faces: BTreeSet<&str> = self
            .faces
            .iter()
            .map(|asset| asset.asset_id.as_str())
            .collect();
        let families: BTreeMap<String, FontFamilyAsset> = self
            .families
            .iter()
            .cloned()
            .map(|asset| (asset.asset_id.clone(), asset))
            .collect();
        let stacks: BTreeMap<String, FontStackAsset> = self
            .stacks
            .iter()
            .cloned()
            .map(|asset| (asset.asset_id.clone(), asset))
            .collect();
        for family in &self.families {
            for face in &family.faces {
                if !faces.contains(face.font_face.as_str()) {
                    let mut diagnostic = FontDiagnostic::error(
                        "FontFaceMissing",
                        FontDiagnosticStage::AssetResolve,
                        Some(face.font_face.clone()),
                        format!(
                            "Family {} references missing face {}.",
                            family.asset_id, face.font_face
                        ),
                        "Create the face or update the family AssetRef.",
                    );
                    diagnostic.font_family_id = Some(family.asset_id.clone());
                    diagnostic.font_face_id = Some(face.font_face.clone());
                    diagnostics.push(diagnostic);
                }
            }
        }
        for stack in &self.stacks {
            if let Err(diagnostic) = validate_font_stack(&stack.asset_id, &stacks, &families) {
                diagnostics.push(diagnostic);
            }
        }
        let default_profiles: Vec<_> = self
            .profiles
            .iter()
            .filter(|profile| profile.role == FontAtlasProfileRole::DefaultUi)
            .collect();
        if default_profiles.len() > 1 {
            diagnostics.push(FontDiagnostic::error(
                "FontDefaultUiStackInvalid",
                FontDiagnosticStage::AssetResolve,
                None,
                format!(
                    "Expected at most one defaultUi profile, found {}.",
                    default_profiles.len()
                ),
                "Keep at most one defaultUi FontAtlasProfile.",
            ));
        }
        for profile in &self.profiles {
            if !stacks.contains_key(&profile.font_stack) {
                let mut diagnostic = FontDiagnostic::error(
                    "FontStackMissing",
                    FontDiagnosticStage::AssetResolve,
                    Some(profile.font_stack.clone()),
                    format!(
                        "Profile {} references missing stack {}.",
                        profile.asset_id, profile.font_stack
                    ),
                    "Create the stack or update the profile AssetRef.",
                );
                diagnostic.font_atlas_profile_id = Some(profile.asset_id.clone());
                diagnostic.font_stack_id = Some(profile.font_stack.clone());
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    }

    pub fn asset_graph(&self, database_version: u64) -> AssetGraphDocument {
        let mut nodes = Vec::new();
        nodes.extend(self.faces.iter().map(|asset| AssetGraphNode {
            asset_guid: asset.asset_id.clone(),
            asset_id: asset.asset_id.clone(),
            direct_dependencies: vec![asset.source.asset_ref.clone()],
            source_paths: Vec::new(),
        }));
        nodes.extend(self.families.iter().map(|asset| {
            AssetGraphNode {
                asset_guid: asset.asset_id.clone(),
                asset_id: asset.asset_id.clone(),
                direct_dependencies: asset
                    .faces
                    .iter()
                    .map(|face| face.font_face.clone())
                    .collect(),
                source_paths: Vec::new(),
            }
        }));
        nodes.extend(self.stacks.iter().map(|asset| AssetGraphNode {
            asset_guid: asset.asset_id.clone(),
            asset_id: asset.asset_id.clone(),
            direct_dependencies: asset.families.clone(),
            source_paths: Vec::new(),
        }));
        nodes.extend(self.profiles.iter().map(|asset| AssetGraphNode {
            asset_guid: asset.asset_id.clone(),
            asset_id: asset.asset_id.clone(),
            direct_dependencies: vec![asset.font_stack.clone()],
            source_paths: Vec::new(),
        }));
        nodes.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        AssetGraphDocument {
            schema_version: PROJECT_FONT_ASSET_GRAPH_SCHEMA_VERSION.to_string(),
            built_from_database_version: database_version,
            nodes,
        }
    }
}

pub fn diagnostics_for_level(
    diagnostics: &[FontDiagnostic],
    level: FontReportLevel,
) -> Vec<FontDiagnostic> {
    match level {
        FontReportLevel::Off => Vec::new(),
        FontReportLevel::Summary => diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity != FontDiagnosticSeverity::Info)
            .cloned()
            .collect(),
        FontReportLevel::Trace => diagnostics.to_vec(),
    }
}

fn valid_asset_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_unicode_scalar(value: &str) -> Option<char> {
    let hexadecimal = value.strip_prefix("U+")?;
    let codepoint = u32::from_str_radix(hexadecimal, 16).ok()?;
    char::from_u32(codepoint)
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdsm::bezier::scanline::FillRule;
    use fdsm::correct_error::{correct_error_msdf, ErrorCorrectionConfig};
    use fdsm::generate::generate_msdf;
    use fdsm::render::correct_sign_msdf;
    use fdsm::shape::Shape;
    use fdsm::transform::Transform;
    use image::{ImageBuffer, Rgb};
    use nalgebra::{Affine2, Similarity2, Vector2};
    use std::fs;
    use swash::scale::{Render, ScaleContext, Source};
    use swash::zeno::Format;
    use swash::FontRef;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts/qualification")
            .join(name)
    }

    fn read_fixture(name: &str) -> Vec<u8> {
        fs::read(fixture(name)).expect("qualification fixture")
    }

    fn test_face(asset_id: &str, source_ref: &str) -> FontFaceAsset {
        FontFaceAsset {
            schema_version: FONT_FACE_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: asset_id.to_string(),
            source: FontFaceSource {
                kind: FontSourceKind::ProjectFile,
                asset_ref: source_ref.to_string(),
                face_index: 0,
                source_sha256:
                    "sha256:f70ecb32e5b312ba7bc724977352139a3f691566dc2491377be3828631c9fab2"
                        .to_string(),
            },
            declared: FontFaceDeclaredMetadata {
                family: "Aife Noto Sans SC Qualification".to_string(),
                style: FontStyle::Normal,
                weight: 400,
                stretch: 100,
            },
            hinting: FontHintingMode::FontDefault,
        }
    }

    fn family(id: &str, faces: &[(&str, FontStyle, u16)]) -> FontFamilyAsset {
        FontFamilyAsset {
            schema_version: FONT_FAMILY_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: id.to_string(),
            faces: faces
                .iter()
                .map(|(face, style, weight)| FontFamilyFace {
                    font_face: (*face).to_string(),
                    style: *style,
                    weight: *weight,
                })
                .collect(),
            missing_style_policy: FontMissingStylePolicy::NearestWeightSameStyle,
        }
    }

    fn stack(id: &str, families: &[&str]) -> FontStackAsset {
        FontStackAsset {
            schema_version: FONT_STACK_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: id.to_string(),
            families: families.iter().map(|value| (*value).to_string()).collect(),
            missing_glyph_policy: FontMissingGlyphPolicy::Error,
            replacement_codepoint: "U+FFFD".to_string(),
        }
    }

    fn profile(id: &str, stack_id: &str) -> FontAtlasProfileAsset {
        FontAtlasProfileAsset {
            schema_version: FONT_ATLAS_PROFILE_ASSET_SCHEMA_VERSION.to_string(),
            asset_id: id.to_string(),
            role: FontAtlasProfileRole::DefaultUi,
            font_stack: stack_id.to_string(),
            glyph_set: FontGlyphSet {
                include_runtime_text_sources: true,
                unicode_ranges: vec!["U+0020-U+007E".to_string()],
                literals: Vec::new(),
                locales: vec!["zh-CN".to_string()],
            },
            raster: FontRasterProfile {
                policy: FontRasterPolicy::AutoHybrid,
                bitmap_pixel_sizes: vec![12, 14, 16, 18, 20, 24, 28, 32],
                bitmap_hinting: FontHintingMode::FontDefault,
                msdf_em_size: 64,
                msdf_pixel_range: 8,
            },
            packing: FontPackingProfile {
                page_width: 2048,
                page_height: 2048,
                padding: 1,
                max_bitmap_pages: 16,
                max_msdf_pages: 16,
            },
        }
    }

    fn render_hinted(bytes: &[u8], index: usize, character: char) -> Vec<u8> {
        let font = FontRef::from_index(bytes, index).expect("swash font");
        let glyph = font.charmap().map(character);
        assert_ne!(glyph, 0, "qualification glyph must not become '?'");
        let mut context = ScaleContext::new();
        let mut scaler = context.builder(font).size(14.0).hint(true).build();
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, glyph)
            .expect("hinted glyph");
        assert!(image.placement.width > 0 && image.placement.height > 0);
        assert!(image.data.iter().any(|value| *value != 0));
        image.data
    }

    fn generate_qualified_msdf(bytes: &[u8], index: u32, character: char) -> Vec<u8> {
        let face = ttf_parser::Face::parse(bytes, index).expect("ttf-parser face");
        let glyph = face.glyph_index(character).expect("qualification glyph");
        let bbox = face.glyph_bounding_box(glyph).expect("glyph bbox");
        let mut shape =
            fdsm_ttf_parser::load_shape_from_face(&face, glyph).expect("glyph outline shape");
        let range = 4.0;
        let width = 64;
        let height = 64;
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
        let colored = Shape::edge_coloring_simple(shape, 0.03, 0xA1FE_261);
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
        assert!(image.as_raw().iter().all(|value| value.is_finite()));
        assert!(image.as_raw().iter().any(|value| *value > 0.0));
        image
            .as_raw()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn font_backend_qualification_real_ttf_otf_ttc_hinting_and_msdf_are_deterministic() {
        let ttf = read_fixture("AifeNotoSansSCQualification-Regular.ttf");
        let otf = read_fixture("AifeQualificationCFF-Regular.otf");
        let ttc = read_fixture("AifeNotoSCQualification.ttc");
        assert!(ttf_parser::Face::parse(&ttf, 0).is_ok());
        assert!(ttf_parser::Face::parse(&otf, 0).is_ok());
        assert!(ttf_parser::Face::parse(&ttc, 0).is_ok());
        assert!(ttf_parser::Face::parse(&ttc, 1).is_ok());
        assert!(ttf_parser::Face::parse(&ttc, 2).is_err());

        let hinted_a = render_hinted(&ttf, 0, '中');
        let hinted_b = render_hinted(&ttf, 0, '中');
        assert_eq!(hinted_a, hinted_b);

        for (bytes, character) in [(&ttf[..], '中'), (&otf[..], 'A'), (&otf[..], 'S')] {
            let first = generate_qualified_msdf(bytes, 0, character);
            let second = generate_qualified_msdf(bytes, 0, character);
            assert_eq!(first, second);
            assert!(first.iter().any(|value| *value != 0));
        }
    }

    #[test]
    fn project_font_asset_schema_path_hash_face_index_and_digest_are_closed() {
        let root = fixture("AifeNotoSansSCQualification-Regular.ttf")
            .parent()
            .unwrap()
            .to_path_buf();
        let asset = test_face("font-face-qualification", "asset-font-source-qualification");
        let result = validate_font_face_source(
            &root,
            Path::new("AifeNotoSansSCQualification-Regular.ttf"),
            &asset,
        )
        .expect("valid project font");
        assert_eq!(result.face_count, 1);

        let mut bad_hash = asset.clone();
        bad_hash.source.source_sha256 = format!("sha256:{}", "0".repeat(64));
        assert_eq!(
            validate_font_face_source(
                &root,
                Path::new("AifeNotoSansSCQualification-Regular.ttf"),
                &bad_hash,
            )
            .unwrap_err()
            .code,
            "FontSourceHashMismatch"
        );
        assert_eq!(
            validate_font_face_source(&root, Path::new("../outside.ttf"), &asset)
                .unwrap_err()
                .code,
            "FontSourcePathInvalid"
        );
        let mut invalid_face_index = asset.clone();
        invalid_face_index.source.asset_ref = "asset-font-source-collection".to_string();
        invalid_face_index.source.face_index = 2;
        invalid_face_index.source.source_sha256 =
            "sha256:278c89270cd70c8b3c9f4b284b54bfe8639f8c271e5c4fdce7cc0d90251b0d75".to_string();
        assert_eq!(
            validate_font_face_source(
                &root,
                Path::new("AifeNotoSCQualification.ttc"),
                &invalid_face_index,
            )
            .unwrap_err()
            .code,
            "FontFaceIndexOutOfRange"
        );

        let set = ProjectFontAssetSet {
            faces: vec![asset],
            families: Vec::new(),
            stacks: Vec::new(),
            profiles: Vec::new(),
        };
        assert_eq!(
            set.canonical_digest().unwrap(),
            set.canonical_digest().unwrap()
        );
        let reordered = ProjectFontAssetSet {
            faces: vec![
                test_face("font-face-z", "asset-font-source-z"),
                test_face("font-face-a", "asset-font-source-a"),
            ],
            ..ProjectFontAssetSet::default()
        };
        let mut reversed = reordered.clone();
        reversed.faces.reverse();
        assert_eq!(
            reordered.canonical_digest().unwrap(),
            reversed.canonical_digest().unwrap()
        );
    }

    #[test]
    fn font_family_resolution_uses_exact_nearest_lower_weight_and_stable_id() {
        let family = family(
            "font-family-ui",
            &[
                ("face-700", FontStyle::Normal, 700),
                ("face-300-z", FontStyle::Normal, 300),
                ("face-300-a", FontStyle::Normal, 300),
                ("face-italic", FontStyle::Italic, 400),
            ],
        );
        assert_eq!(
            resolve_font_family_face(&family, FontStyle::Italic, 400)
                .unwrap()
                .font_face,
            "face-italic"
        );
        assert_eq!(
            resolve_font_family_face(&family, FontStyle::Normal, 500)
                .unwrap()
                .font_face,
            "face-300-a"
        );
        assert_eq!(
            resolve_font_family_face(&family, FontStyle::Oblique, 650)
                .unwrap()
                .font_face,
            "face-700"
        );
    }

    #[test]
    fn font_stack_preserves_fallback_order_and_rejects_cycle() {
        let families = BTreeMap::from([
            (
                "family-latin".to_string(),
                family("family-latin", &[("face-latin", FontStyle::Normal, 400)]),
            ),
            (
                "family-cjk".to_string(),
                family("family-cjk", &[("face-cjk", FontStyle::Normal, 400)]),
            ),
        ]);
        let valid = BTreeMap::from([(
            "stack-ui".to_string(),
            stack("stack-ui", &["family-latin", "family-cjk"]),
        )]);
        assert_eq!(
            validate_font_stack("stack-ui", &valid, &families).unwrap(),
            vec!["family-latin", "family-cjk"]
        );
        let cyclic = BTreeMap::from([
            ("stack-a".to_string(), stack("stack-a", &["stack-b"])),
            ("stack-b".to_string(), stack("stack-b", &["stack-a"])),
        ]);
        assert_eq!(
            validate_font_stack("stack-a", &cyclic, &families)
                .unwrap_err()
                .code,
            "FontStackCycle"
        );
    }

    #[test]
    fn project_font_asset_graph_has_exact_source_family_stack_profile_dependencies() {
        let set = ProjectFontAssetSet {
            faces: vec![test_face("face-ui", "source-ui")],
            families: vec![family("family-ui", &[("face-ui", FontStyle::Normal, 400)])],
            stacks: vec![stack("stack-ui", &["family-ui"])],
            profiles: vec![profile("profile-ui", "stack-ui")],
        };
        assert!(set.validate_references().is_empty());
        let graph = set.asset_graph(7);
        let dependencies: BTreeMap<_, _> = graph
            .nodes
            .into_iter()
            .map(|node| (node.asset_id, node.direct_dependencies))
            .collect();
        assert_eq!(dependencies["face-ui"], vec!["source-ui"]);
        assert_eq!(dependencies["family-ui"], vec!["face-ui"]);
        assert_eq!(dependencies["stack-ui"], vec!["family-ui"]);
        assert_eq!(dependencies["profile-ui"], vec!["stack-ui"]);
    }

    #[test]
    fn font_diagnostic_has_domain_stage_source_next_action_and_report_levels() {
        let diagnostic = FontDiagnostic::error(
            "FontStackCycle",
            FontDiagnosticStage::AssetResolve,
            Some("stack-ui".to_string()),
            "cycle",
            "remove cycle",
        );
        assert_eq!(diagnostic.domain, "font");
        assert_eq!(diagnostic.source.as_deref(), Some("stack-ui"));
        assert!(!diagnostic.next_action.is_empty());
        assert!(diagnostics_for_level(&[diagnostic.clone()], FontReportLevel::Off).is_empty());
        assert_eq!(
            diagnostics_for_level(&[diagnostic.clone()], FontReportLevel::Summary),
            vec![diagnostic.clone()]
        );
        assert_eq!(
            diagnostics_for_level(&[diagnostic.clone()], FontReportLevel::Trace),
            vec![diagnostic]
        );
    }
}
