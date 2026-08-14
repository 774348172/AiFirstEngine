use crate::animator2d::{CookedAnimator2DRegistry, RuntimeAnimator2D};
use crate::aui::{AuiDocument, AUI_DOCUMENT_SCHEMA_VERSION};
use crate::diagnostics::{RuntimeDiagnostic, RuntimeDiagnostics, RuntimeLoadResult};
use crate::font_bundle::{
    CookedFontBundleAsset, RuntimeFontBundleLoader, RuntimeFontBundleManifest,
    RuntimeFontBundleRegistry, RuntimePackageSourceFontBundle,
};
use crate::project_observation::CookedProjectObservationContract;
use crate::rule_artifact::validate_runtime_rule_manifest_artifacts;
use crate::runtime_asset::{
    BundleRecord, CookedAssetRecord, RuntimeAssetDependencyRecord, RuntimeAssetIndex,
    RuntimeAssetRecord, RuntimePackageMountTable,
};
use crate::runtime_package_path::safe_join_runtime_package;
use engine_input::{InputDiagnosticSeverity, InputMappingAsset};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const LEGACY_RUNTIME_PACKAGE_SCHEMA_VERSION: &str = "runtime-package.v1";
pub const RUNTIME_PACKAGE_SCHEMA_VERSION: &str = "runtime-package.v2";
pub const RUNTIME_SCENE_SCHEMA_VERSION: &str = "runtime-scene.v1";
pub const RUNTIME_ENTITY_SCHEMA_VERSION: &str = "runtime-entity.v1";
pub const RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION: &str = "runtime-asset-manifest.v1";
pub const RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION: &str = "runtime-input-manifest.v1";
pub const RUNTIME_RULE_MANIFEST_SCHEMA_VERSION: &str = "runtime-rule-manifest.v1";
pub const RUNTIME_AUI_MANIFEST_SCHEMA_VERSION: &str = "runtime-aui-manifest.v1";
pub const RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION: &str = "runtime-font-atlas-manifest.v1";
pub const COOKED_FONT_ATLAS_SCHEMA_VERSION: &str = "cooked-font-atlas.v1";
pub const COOKED_TEXTURE_SCHEMA_VERSION: &str = "cooked-texture.v1";
pub const RUNTIME_AUI_DOCUMENT_LOAD_REPORT_SCHEMA_VERSION: &str =
    "runtime-aui-document-load-report.v1";
pub const RUNTIME_FONT_ATLAS_LOAD_REPORT_SCHEMA_VERSION: &str = "runtime-font-atlas-load-report.v1";
pub const RUNTIME_RULE_MANIFEST_MODE: &str = "rust-aot";
pub const RUNTIME_PACKAGE_MODE: &str = "debug-readable";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackageManifest {
    pub schema_version: String,
    pub package_mode: String,
    pub project: RuntimeProjectInfo,
    pub active_scene_id: String,
    pub scenes: Vec<RuntimeSceneManifestEntry>,
    pub assets: RuntimeManifestAssetIndex,
    pub rules: RuntimeManifestRuleIndex,
    pub input: RuntimeManifestInputIndex,
    #[serde(default)]
    pub aui: Option<RuntimeManifestAuiIndex>,
    #[serde(default)]
    pub font_atlases: Option<RuntimeManifestFontAtlasIndex>,
    #[serde(default)]
    pub font_bundles: Option<RuntimeManifestFontBundleIndex>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animator2d: Option<RuntimeManifestAnimator2DIndex>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_contract: Option<CookedProjectObservationContract>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProjectInfo {
    pub project_id: String,
    pub name: String,
    pub version: String,
    pub runtime_module: RuntimeProjectModuleRef,
}

impl RuntimeProjectInfo {
    pub fn new(
        project_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        runtime_module: RuntimeProjectModuleRef,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            name: name.into(),
            version: version.into(),
            runtime_module,
        }
    }

    pub fn explicit_empty(
        project_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self::new(
            project_id,
            name,
            version,
            RuntimeProjectModuleRef::explicit_empty(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProjectModuleRef {
    pub module_id: String,
    pub interface_version: String,
    pub aot_content_digest: String,
}

impl RuntimeProjectModuleRef {
    pub fn new(
        module_id: impl Into<String>,
        interface_version: impl Into<String>,
        aot_content_digest: impl Into<String>,
    ) -> Self {
        Self {
            module_id: module_id.into(),
            interface_version: interface_version.into(),
            aot_content_digest: aot_content_digest.into(),
        }
    }

    pub fn explicit_empty() -> Self {
        Self::new(
            "engine.empty.runtime",
            "project-runtime-module.v2",
            "sha256:engine-empty-runtime-v2",
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSceneManifestEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub entity_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifestAssetIndex {
    pub path: String,
    pub asset_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifestRuleIndex {
    pub path: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifestInputIndex {
    pub path: String,
    pub default_mapping_id: String,
    pub mapping_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifestAuiIndex {
    pub path: String,
    pub document_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifestFontAtlasIndex {
    pub path: String,
    pub atlas_count: usize,
    pub default_ui_font_atlas_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifestFontBundleIndex {
    pub path: String,
    pub bundle_count: usize,
    pub default_ui_font_bundle_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeManifestAnimator2DIndex {
    pub path: String,
    pub registry_digest: String,
    pub clip_count: usize,
    pub controller_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuiManifest {
    pub schema_version: String,
    pub documents: Vec<RuntimeAuiManifestEntry>,
}

impl RuntimeAuiManifest {
    pub fn empty() -> Self {
        Self {
            schema_version: RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.to_string(),
            documents: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuiManifestEntry {
    pub document_id: String,
    pub path: String,
    pub canvas_count: usize,
    pub node_count: usize,
    pub binding_count: usize,
    pub action_count: usize,
    #[serde(default)]
    pub asset_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFontAtlasManifest {
    pub schema_version: String,
    pub default_ui_font_atlas_id: Option<String>,
    pub atlases: Vec<RuntimeFontAtlasManifestEntry>,
}

impl RuntimeFontAtlasManifest {
    pub fn empty() -> Self {
        Self {
            schema_version: RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION.to_string(),
            default_ui_font_atlas_id: None,
            atlases: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFontAtlasManifestEntry {
    pub font_atlas_id: String,
    pub metadata_path: String,
    pub bitmap_path: String,
    pub glyph_count: usize,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub font_source_kind: String,
    pub font_asset_status: String,
    pub fallback_used: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookedFontAtlasAsset {
    pub schema_version: String,
    pub font_atlas_id: String,
    pub font_asset_id: String,
    pub font_source_kind: String,
    pub font_asset_status: String,
    pub atlas_image_path: String,
    pub atlas_format: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub atlas_generation: u64,
    pub atlas_alpha_byte_len: usize,
    pub glyphs: Vec<CookedFontAtlasGlyph>,
    pub fallback_used: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookedTextureAsset {
    pub schema_version: String,
    pub asset_id: String,
    pub cooked_asset_id: String,
    pub source_hash: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub mip_count: u32,
    pub byte_length: usize,
    pub pixel_data_path: String,
    pub sampler: String,
}

impl CookedFontAtlasAsset {
    pub fn glyph(&self, ch: char) -> Option<&CookedFontAtlasGlyph> {
        let codepoint = ch as u32;
        self.glyphs
            .iter()
            .find(|glyph| glyph.codepoint == codepoint)
            .or_else(|| {
                self.glyphs
                    .iter()
                    .find(|glyph| glyph.codepoint == '?' as u32)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookedFontAtlasGlyph {
    pub codepoint: u32,
    pub glyph_id: String,
    pub uv_rect: [f32; 4],
    pub pixel_rect: [u32; 4],
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
    pub page_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLoadedFontAtlas {
    pub metadata: CookedFontAtlasAsset,
    pub atlas_alpha: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuiFontAtlasRegistry {
    pub atlases_by_id: BTreeMap<String, RuntimeLoadedFontAtlas>,
    pub default_ui_font_atlas_id: Option<String>,
    pub load_report: RuntimeFontAtlasLoadReport,
}

impl RuntimeAuiFontAtlasRegistry {
    pub fn empty(package_path: impl Into<String>) -> Self {
        Self {
            atlases_by_id: BTreeMap::new(),
            default_ui_font_atlas_id: None,
            load_report: RuntimeFontAtlasLoadReport {
                schema_version: RUNTIME_FONT_ATLAS_LOAD_REPORT_SCHEMA_VERSION.to_string(),
                status: RuntimeFontAtlasLoadStatus::Success,
                package_path: package_path.into(),
                manifest_atlas_count: 0,
                loaded_atlas_count: 0,
                failed_atlas_count: 0,
                default_ui_font_atlas_id: None,
                atlases: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    pub fn default_atlas(&self) -> Option<&RuntimeLoadedFontAtlas> {
        self.default_ui_font_atlas_id
            .as_deref()
            .and_then(|id| self.atlases_by_id.get(id))
            .or_else(|| self.atlases_by_id.values().next())
    }

    pub fn len(&self) -> usize {
        self.atlases_by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.atlases_by_id.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeFontAtlasLoadStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFontAtlasLoadReport {
    pub schema_version: String,
    pub status: RuntimeFontAtlasLoadStatus,
    pub package_path: String,
    pub manifest_atlas_count: usize,
    pub loaded_atlas_count: usize,
    pub failed_atlas_count: usize,
    pub default_ui_font_atlas_id: Option<String>,
    pub atlases: Vec<RuntimeFontAtlasLoadEntry>,
    pub diagnostics: Vec<RuntimeFontAtlasLoadDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFontAtlasLoadEntry {
    pub font_atlas_id: String,
    pub metadata_path: String,
    pub bitmap_path: String,
    pub status: RuntimeFontAtlasLoadStatus,
    pub glyph_count: usize,
    pub atlas_alpha_byte_len: usize,
    pub font_source_kind: String,
    pub font_asset_status: String,
    pub fallback_used: bool,
    pub legacy_mode: bool,
    pub quality_gate_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFontAtlasLoadDiagnostic {
    pub code: String,
    pub message: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuiDocumentRegistry {
    pub documents_by_id: BTreeMap<String, AuiDocument>,
    pub load_report: RuntimeAuiDocumentLoadReport,
}

impl RuntimeAuiDocumentRegistry {
    pub fn empty(package_path: impl Into<String>) -> Self {
        Self {
            documents_by_id: BTreeMap::new(),
            load_report: RuntimeAuiDocumentLoadReport {
                schema_version: RUNTIME_AUI_DOCUMENT_LOAD_REPORT_SCHEMA_VERSION.to_string(),
                status: RuntimeAuiDocumentLoadStatus::Success,
                package_path: package_path.into(),
                manifest_document_count: 0,
                loaded_document_count: 0,
                failed_document_count: 0,
                documents: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }

    pub fn get(&self, document_id: &str) -> Option<&AuiDocument> {
        self.documents_by_id.get(document_id)
    }

    pub fn len(&self) -> usize {
        self.documents_by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents_by_id.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeAuiDocumentLoadStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuiDocumentLoadReport {
    pub schema_version: String,
    pub status: RuntimeAuiDocumentLoadStatus,
    pub package_path: String,
    pub manifest_document_count: usize,
    pub loaded_document_count: usize,
    pub failed_document_count: usize,
    pub documents: Vec<RuntimeAuiDocumentLoadEntry>,
    pub diagnostics: Vec<RuntimeAuiDocumentLoadDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuiDocumentLoadEntry {
    pub document_id: String,
    pub path: String,
    pub status: RuntimeAuiDocumentLoadStatus,
    pub node_count: usize,
    pub binding_count: usize,
    pub action_count: usize,
    pub asset_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuiDocumentLoadDiagnostic {
    pub code: String,
    pub message: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInputManifest {
    pub schema_version: String,
    pub default_mapping_id: String,
    pub mappings: Vec<RuntimeInputMappingManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInputMappingManifestEntry {
    pub id: String,
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeScene {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub gravity: f32,
    pub background: String,
    pub sky_color: String,
    pub entities: Vec<RuntimeEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEntity {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub parent_id: Option<String>,
    pub sibling_order: i32,
    pub transform: Option<RuntimeTransform>,
    pub mesh: Option<RuntimeMesh>,
    #[serde(default, rename = "spriteRenderer2D", alias = "spriteRenderer2d")]
    pub sprite_renderer2d: Option<RuntimeSpriteRenderer2D>,
    #[serde(default)]
    pub animator2d: Option<RuntimeAnimator2D>,
    #[serde(default)]
    pub components: Vec<RuntimeProjectComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProjectComponent {
    #[serde(alias = "componentType")]
    pub component_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePrefabData {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub root_entity_id: Option<String>,
    pub entities: Vec<RuntimeEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTransform {
    pub local_position: Vector3,
    pub local_rotation: Vector3,
    pub local_scale: Vector3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMesh {
    pub primitive: Option<String>,
    pub color: Option<String>,
    pub label: Option<String>,
    pub asset_ref: Option<RuntimeAssetRef>,
    pub material_ref: Option<RuntimeAssetRef>,
    pub texture_ref: Option<RuntimeAssetRef>,
    pub visible: bool,
    pub layer: String,
    pub metalness: Option<f32>,
    pub roughness: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSpriteRenderer2D {
    pub sprite_ref: Option<RuntimeAssetRef>,
    pub material_ref: Option<RuntimeAssetRef>,
    pub color: Option<[f32; 4]>,
    pub flip_x: Option<bool>,
    pub flip_y: Option<bool>,
    pub sorting_layer: Option<i16>,
    pub order_in_layer: Option<i32>,
    pub sort_z: Option<f32>,
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAssetRef {
    pub id: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    #[serde(default)]
    pub guid: Option<String>,
    #[serde(default)]
    pub sub_asset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAssetManifest {
    pub schema_version: String,
    pub assets: Vec<RuntimeAsset>,
    #[serde(default)]
    pub runtime_asset_index: Vec<RuntimeAssetRecord>,
    #[serde(default)]
    pub bundle_table: Vec<BundleRecord>,
    #[serde(default)]
    pub cooked_asset_table: Vec<CookedAssetRecord>,
    #[serde(default)]
    pub dependency_table: Vec<RuntimeAssetDependencyRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAsset {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub source: String,
    pub state: String,
    pub bundle_id: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRuleManifest {
    pub schema_version: String,
    pub mode: String,
    #[serde(default)]
    pub rules: Vec<RuntimeRuleManifestEntry>,
    #[serde(default)]
    pub modules: Vec<RuntimeRuleModuleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRuleManifestEntry {
    pub rule_id: String,
    pub phase: RuntimeRulePhase,
    pub enabled: bool,
    pub executor: RuntimeRuleExecutor,
    #[serde(default)]
    pub ir_source: Option<String>,
    #[serde(default)]
    pub ir_hash: Option<String>,
    #[serde(default)]
    pub artifact_id: Option<String>,
    #[serde(default)]
    pub source_map: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRuleModuleEntry {
    pub artifact_id: String,
    pub module_kind: RuntimeRuleModuleKind,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeRulePhase {
    FixedUpdate,
    Update,
    PostPhysics,
    EventHandler,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRuleExecutor {
    RustAot,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRuleModuleKind {
    StaticRegistry,
    DynamicValidationHost,
}

#[derive(Debug, Clone)]
pub struct RuntimePackage {
    pub package_dir: PathBuf,
    pub manifest: RuntimePackageManifest,
    pub active_scene: RuntimeScene,
    pub assets: RuntimeAssetManifest,
    pub runtime_asset_index: RuntimeAssetIndex,
    pub runtime_asset_mount_table: RuntimePackageMountTable,
    pub rules: RuntimeRuleManifest,
    pub aui_manifest: RuntimeAuiManifest,
    pub aui_documents: RuntimeAuiDocumentRegistry,
    pub font_atlas_manifest: RuntimeFontAtlasManifest,
    pub font_atlases: RuntimeAuiFontAtlasRegistry,
    pub font_bundle_manifest: RuntimeFontBundleManifest,
    pub font_bundles: RuntimeFontBundleRegistry,
    pub animator2d_registry: CookedAnimator2DRegistry,
    pub input_manifest: RuntimeInputManifest,
    pub input_mappings: Vec<InputMappingAsset>,
    pub default_input_mapping: Option<InputMappingAsset>,
}

pub fn load_runtime_package(package_dir: impl AsRef<Path>) -> RuntimeLoadResult<RuntimePackage> {
    let package_dir = package_dir.as_ref().to_path_buf();
    let mut diagnostics = RuntimeDiagnostics::new();
    let manifest_path = package_dir.join("manifest.json");
    let Some(manifest_value) =
        read_json::<serde_json::Value>(&manifest_path, "manifest", &mut diagnostics)
    else {
        return RuntimeLoadResult::failed(diagnostics);
    };
    let manifest_schema = manifest_value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str);
    if manifest_schema == Some(LEGACY_RUNTIME_PACKAGE_SCHEMA_VERSION) {
        diagnostics.error(
            "project_runtime.package_v1_rebuild_required",
            "RuntimePackage v1 has no required project runtime module identity; rebuild it as runtime-package.v2.",
        );
        return RuntimeLoadResult::failed(diagnostics);
    }
    let manifest = match serde_json::from_value::<RuntimePackageManifest>(manifest_value) {
        Ok(manifest) => manifest,
        Err(error) => {
            diagnostics.error(
                "manifest",
                format!("failed to parse {}: {}", manifest_path.display(), error),
            );
            return RuntimeLoadResult::failed(diagnostics);
        }
    };

    validate_manifest(&manifest, &mut diagnostics);

    let active_scene_entry = manifest
        .scenes
        .iter()
        .find(|scene| scene.id == manifest.active_scene_id);
    let active_scene_path = active_scene_entry.and_then(|scene| {
        safe_package_join(
            &package_dir,
            &scene.path,
            "manifest.scenes.path",
            &mut diagnostics,
        )
    });
    if active_scene_entry.is_none() {
        diagnostics.error(
            "activeSceneId",
            format!("activeSceneId does not exist: {}", manifest.active_scene_id),
        );
    }

    let scene = active_scene_path
        .as_deref()
        .and_then(|path| read_json::<RuntimeScene>(path, "scene", &mut diagnostics));
    let assets = safe_package_join(
        &package_dir,
        &manifest.assets.path,
        "manifest.assets.path",
        &mut diagnostics,
    )
    .and_then(|path| read_json::<RuntimeAssetManifest>(&path, "assets", &mut diagnostics));
    let rules = safe_package_join(
        &package_dir,
        &manifest.rules.path,
        "manifest.rules.path",
        &mut diagnostics,
    )
    .and_then(|path| read_json::<RuntimeRuleManifest>(&path, "rules", &mut diagnostics));

    let aui_manifest = load_aui_manifest(&package_dir, manifest.aui.as_ref(), &mut diagnostics);
    let font_atlas_manifest = load_font_atlas_manifest(
        &package_dir,
        manifest.font_atlases.as_ref(),
        &mut diagnostics,
    );
    let font_bundle_manifest = load_font_bundle_manifest(
        &package_dir,
        manifest.font_bundles.as_ref(),
        &mut diagnostics,
    );
    let animator2d_registry =
        load_animator2d_registry(&package_dir, manifest.animator2d.as_ref(), &mut diagnostics);

    let (Some(active_scene), Some(assets), Some(rules)) = (scene, assets, rules) else {
        return RuntimeLoadResult::failed(diagnostics);
    };

    validate_scene(&active_scene, &assets, &mut diagnostics);
    validate_scene_animator2d(&active_scene, &animator2d_registry, &mut diagnostics);
    validate_assets(&assets, &mut diagnostics);
    validate_rules(&rules, &mut diagnostics);
    validate_aui_manifest(&aui_manifest, manifest.aui.as_ref(), &mut diagnostics);
    validate_font_atlas_manifest(
        &font_atlas_manifest,
        manifest.font_atlases.as_ref(),
        &mut diagnostics,
    );
    let aui_documents = load_aui_documents(&package_dir, &aui_manifest, &mut diagnostics);
    let font_atlases = load_font_atlases(&package_dir, &font_atlas_manifest, &mut diagnostics);
    let font_bundles = load_font_bundles(&package_dir, &font_bundle_manifest, &mut diagnostics);
    let (input_manifest, input_mappings, default_input_mapping) =
        load_input_mappings(&package_dir, &manifest.input, &mut diagnostics);
    let runtime_asset_index = RuntimeAssetIndex::from_manifest(
        &assets,
        &assets.runtime_asset_index,
        &assets.cooked_asset_table,
        &assets.dependency_table,
    );
    let runtime_asset_mount_table = RuntimePackageMountTable::from_manifest(&assets);

    if diagnostics.has_errors() {
        RuntimeLoadResult::failed(diagnostics)
    } else {
        RuntimeLoadResult::ok(
            RuntimePackage {
                package_dir,
                manifest,
                active_scene,
                assets,
                runtime_asset_index,
                runtime_asset_mount_table,
                rules,
                aui_manifest,
                aui_documents,
                font_atlas_manifest,
                font_atlases,
                font_bundle_manifest,
                font_bundles,
                animator2d_registry,
                input_manifest,
                input_mappings,
                default_input_mapping,
            },
            diagnostics,
        )
    }
}

fn load_animator2d_registry(
    package_dir: &Path,
    index: Option<&RuntimeManifestAnimator2DIndex>,
    diagnostics: &mut RuntimeDiagnostics,
) -> CookedAnimator2DRegistry {
    let Some(index) = index else {
        return CookedAnimator2DRegistry::empty();
    };
    let Some(path) = safe_package_join(
        package_dir,
        &index.path,
        "manifest.animator2d.path",
        diagnostics,
    ) else {
        return CookedAnimator2DRegistry::empty();
    };
    let Some(registry) =
        read_json::<CookedAnimator2DRegistry>(&path, "animator2d.registry", diagnostics)
    else {
        return CookedAnimator2DRegistry::empty();
    };
    if let Err(registry_diagnostics) = registry.validate() {
        for diagnostic in registry_diagnostics {
            diagnostics.error(diagnostic.path, diagnostic.message);
        }
    }
    if registry.registry_digest != index.registry_digest {
        diagnostics.error(
            "manifest.animator2d.registryDigest",
            "Animator2D registry digest does not match the manifest index.",
        );
    }
    if registry.clips.len() != index.clip_count
        || registry.controllers.len() != index.controller_count
    {
        diagnostics.error(
            "manifest.animator2d",
            "Animator2D registry counts do not match the manifest index.",
        );
    }
    registry
}

fn validate_scene_animator2d(
    scene: &RuntimeScene,
    registry: &CookedAnimator2DRegistry,
    diagnostics: &mut RuntimeDiagnostics,
) {
    for entity in &scene.entities {
        let Some(animator) = &entity.animator2d else {
            continue;
        };
        let path = format!("scene.entities.{}.animator2d", entity.id);
        if entity.sprite_renderer2d.is_none() {
            diagnostics.error(
                format!("{path}.spriteRenderer2D"),
                "Animator2D requires SpriteRenderer2D on the same entity.",
            );
        }
        if animator.registry_digest != registry.registry_digest {
            diagnostics.error(
                format!("{path}.registryDigest"),
                "Animator2D component registry digest does not match the package registry.",
            );
        }
        let controller = usize::try_from(animator.controller_index)
            .ok()
            .and_then(|index| registry.controllers.get(index));
        if controller.is_none_or(|controller| controller.id != animator.controller_id) {
            diagnostics.error(
                format!("{path}.controllerId"),
                "Animator2D controller identity does not resolve in the package registry.",
            );
        }
    }
}

fn load_font_bundle_manifest(
    package_dir: &Path,
    index: Option<&RuntimeManifestFontBundleIndex>,
    diagnostics: &mut RuntimeDiagnostics,
) -> RuntimeFontBundleManifest {
    let Some(index) = index else {
        return RuntimeFontBundleManifest::empty();
    };
    safe_package_join(
        package_dir,
        &index.path,
        "manifest.fontBundles.path",
        diagnostics,
    )
    .and_then(|path| read_json::<RuntimeFontBundleManifest>(&path, "fontBundles", diagnostics))
    .unwrap_or_else(RuntimeFontBundleManifest::empty)
}

fn load_font_bundles(
    package_dir: &Path,
    manifest: &RuntimeFontBundleManifest,
    diagnostics: &mut RuntimeDiagnostics,
) -> RuntimeFontBundleRegistry {
    let mut registry = RuntimeFontBundleRegistry {
        default_ui_font_bundle_id: manifest.default_ui_font_bundle_id.clone(),
        ..RuntimeFontBundleRegistry::default()
    };
    for (bundle_index, entry) in manifest.bundles.iter().enumerate() {
        let source_path = format!("fontBundles.bundles[{bundle_index}]");
        let Some(metadata_path) = safe_package_join(
            package_dir,
            &entry.metadata_path,
            &format!("{source_path}.metadataPath"),
            diagnostics,
        ) else {
            continue;
        };
        let Some(metadata) =
            read_json::<CookedFontBundleAsset>(&metadata_path, "fontBundle.metadata", diagnostics)
        else {
            continue;
        };
        let mut payloads = Vec::new();
        let mut missing = false;
        for (page_index, page_path) in entry.page_paths.iter().enumerate() {
            let Some(path) = safe_package_join(
                package_dir,
                page_path,
                &format!("{source_path}.pagePaths[{page_index}]"),
                diagnostics,
            ) else {
                missing = true;
                continue;
            };
            match fs::read(&path) {
                Ok(payload) => payloads.push(payload),
                Err(error) => {
                    diagnostics.error(
                        format!("{source_path}.pagePaths[{page_index}]"),
                        format!("failed to read FontBundle page {}: {error}", path.display()),
                    );
                    missing = true;
                }
            }
        }
        if missing {
            continue;
        }
        match RuntimeFontBundleLoader::load(RuntimePackageSourceFontBundle {
            metadata,
            page_payloads: payloads,
        }) {
            Ok(bundle) => {
                if bundle.metadata.font_bundle_id != entry.font_bundle_id
                    || bundle.metadata.bundle_digest != entry.bundle_digest
                {
                    diagnostics.error(
                        &source_path,
                        "FontBundle manifest identity differs from loaded metadata",
                    );
                } else {
                    registry
                        .bundles_by_id
                        .insert(bundle.metadata.font_bundle_id.clone(), bundle);
                }
            }
            Err(failure) => {
                for diagnostic in failure.diagnostics {
                    diagnostics.error(&diagnostic.source, &diagnostic.message);
                    registry.diagnostics.push(diagnostic);
                }
            }
        }
    }
    registry
}

fn validate_runtime_aui_document_schema(
    document: &AuiDocument,
    manifest_index: usize,
) -> Option<RuntimeAuiDocumentLoadDiagnostic> {
    (document.schema_version != AUI_DOCUMENT_SCHEMA_VERSION).then(|| {
        RuntimeAuiDocumentLoadDiagnostic {
            code: "AuiDocumentSchemaMismatch".to_string(),
            message: format!(
                "AUI document '{}' schemaVersion '{}' must be normalized '{}'",
                document.document_id, document.schema_version, AUI_DOCUMENT_SCHEMA_VERSION
            ),
            path: format!("aui.documents[{manifest_index}].schemaVersion"),
        }
    })
}

fn load_aui_documents(
    package_dir: &Path,
    aui_manifest: &RuntimeAuiManifest,
    diagnostics: &mut RuntimeDiagnostics,
) -> RuntimeAuiDocumentRegistry {
    let mut registry = RuntimeAuiDocumentRegistry::empty(package_dir.display().to_string());
    registry.load_report.manifest_document_count = aui_manifest.documents.len();

    for (index, entry) in aui_manifest.documents.iter().enumerate() {
        let report_path = format!("aui.documents[{index}].path");
        let Some(document_path) =
            safe_package_join(package_dir, &entry.path, &report_path, diagnostics)
        else {
            registry
                .load_report
                .diagnostics
                .push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiDocumentUnsafePath".to_string(),
                    message: format!("declared AUI document path is unsafe: {}", entry.path),
                    path: report_path.clone(),
                });
            registry
                .load_report
                .documents
                .push(RuntimeAuiDocumentLoadEntry {
                    document_id: entry.document_id.clone(),
                    path: entry.path.clone(),
                    status: RuntimeAuiDocumentLoadStatus::Failed,
                    node_count: 0,
                    binding_count: 0,
                    action_count: 0,
                    asset_refs: entry.asset_refs.clone(),
                });
            continue;
        };
        let Some(document) = read_json::<AuiDocument>(&document_path, "aui.document", diagnostics)
        else {
            let diagnostic = RuntimeAuiDocumentLoadDiagnostic {
                code: "AuiDocumentMissing".to_string(),
                message: format!("declared AUI document body is missing: {}", entry.path),
                path: report_path.clone(),
            };
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            registry
                .load_report
                .documents
                .push(RuntimeAuiDocumentLoadEntry {
                    document_id: entry.document_id.clone(),
                    path: entry.path.clone(),
                    status: RuntimeAuiDocumentLoadStatus::Failed,
                    node_count: 0,
                    binding_count: 0,
                    action_count: 0,
                    asset_refs: entry.asset_refs.clone(),
                });
            continue;
        };

        let mut document_failed = false;
        if let Some(diagnostic) = validate_runtime_aui_document_schema(&document, index) {
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            document_failed = true;
        }
        if document.document_id != entry.document_id {
            let diagnostic = RuntimeAuiDocumentLoadDiagnostic {
                code: "AuiDocumentIdMismatch".to_string(),
                message: format!(
                    "AUI manifest documentId '{}' differs from document body '{}'",
                    entry.document_id, document.document_id
                ),
                path: format!("aui.documents[{index}].documentId"),
            };
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            document_failed = true;
        }
        for diagnostic in validate_aui_document_body(&document, index) {
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            document_failed = true;
        }

        let load_status = if document_failed {
            RuntimeAuiDocumentLoadStatus::Failed
        } else {
            RuntimeAuiDocumentLoadStatus::Success
        };
        registry
            .load_report
            .documents
            .push(RuntimeAuiDocumentLoadEntry {
                document_id: entry.document_id.clone(),
                path: entry.path.clone(),
                status: load_status,
                node_count: document.nodes.len(),
                binding_count: document
                    .nodes
                    .iter()
                    .map(|node| node.binding_refs.len())
                    .sum(),
                action_count: document
                    .nodes
                    .iter()
                    .map(|node| node.action_refs.len())
                    .sum(),
                asset_refs: entry.asset_refs.clone(),
            });
        if document_failed {
            continue;
        }
        registry
            .documents_by_id
            .insert(document.document_id.clone(), document);
    }

    registry.load_report.loaded_document_count = registry.documents_by_id.len();
    registry.load_report.failed_document_count = registry
        .load_report
        .documents
        .iter()
        .filter(|entry| entry.status == RuntimeAuiDocumentLoadStatus::Failed)
        .count();
    registry.load_report.status = if registry.load_report.failed_document_count == 0 {
        RuntimeAuiDocumentLoadStatus::Success
    } else {
        RuntimeAuiDocumentLoadStatus::Failed
    };
    registry
}

fn load_font_atlas_manifest(
    package_dir: &Path,
    font_index: Option<&RuntimeManifestFontAtlasIndex>,
    diagnostics: &mut RuntimeDiagnostics,
) -> RuntimeFontAtlasManifest {
    let Some(font_index) = font_index else {
        return RuntimeFontAtlasManifest::empty();
    };
    safe_package_join(
        package_dir,
        &font_index.path,
        "manifest.fontAtlases.path",
        diagnostics,
    )
    .and_then(|path| read_json::<RuntimeFontAtlasManifest>(&path, "fontAtlases", diagnostics))
    .unwrap_or_else(RuntimeFontAtlasManifest::empty)
}

fn load_font_atlases(
    package_dir: &Path,
    font_manifest: &RuntimeFontAtlasManifest,
    diagnostics: &mut RuntimeDiagnostics,
) -> RuntimeAuiFontAtlasRegistry {
    let mut registry = RuntimeAuiFontAtlasRegistry::empty(package_dir.display().to_string());
    registry.load_report.manifest_atlas_count = font_manifest.atlases.len();
    registry.default_ui_font_atlas_id = font_manifest.default_ui_font_atlas_id.clone();
    registry.load_report.default_ui_font_atlas_id = font_manifest.default_ui_font_atlas_id.clone();

    for (index, entry) in font_manifest.atlases.iter().enumerate() {
        let path = format!("fontAtlases.atlases[{index}]");
        let Some(metadata_path) = safe_package_join(
            package_dir,
            &entry.metadata_path,
            &format!("{path}.metadataPath"),
            diagnostics,
        ) else {
            registry
                .load_report
                .diagnostics
                .push(RuntimeFontAtlasLoadDiagnostic {
                    code: "FontAtlasUnsafeMetadataPath".to_string(),
                    message: format!(
                        "declared FontAtlas metadata path is unsafe: {}",
                        entry.metadata_path
                    ),
                    path: format!("{path}.metadataPath"),
                });
            continue;
        };
        let Some(metadata) =
            read_json::<CookedFontAtlasAsset>(&metadata_path, "fontAtlas.metadata", diagnostics)
        else {
            let diagnostic = RuntimeFontAtlasLoadDiagnostic {
                code: "FontAtlasMetadataMissing".to_string(),
                message: format!(
                    "declared FontAtlas metadata is missing: {}",
                    entry.metadata_path
                ),
                path: format!("{path}.metadataPath"),
            };
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            registry
                .load_report
                .atlases
                .push(RuntimeFontAtlasLoadEntry {
                    font_atlas_id: entry.font_atlas_id.clone(),
                    metadata_path: entry.metadata_path.clone(),
                    bitmap_path: entry.bitmap_path.clone(),
                    status: RuntimeFontAtlasLoadStatus::Failed,
                    glyph_count: 0,
                    atlas_alpha_byte_len: 0,
                    font_source_kind: entry.font_source_kind.clone(),
                    font_asset_status: entry.font_asset_status.clone(),
                    fallback_used: entry.fallback_used,
                    legacy_mode: true,
                    quality_gate_eligible: false,
                });
            continue;
        };

        let Some(bitmap_path) = safe_package_join(
            package_dir,
            &entry.bitmap_path,
            &format!("{path}.bitmapPath"),
            diagnostics,
        ) else {
            registry
                .load_report
                .diagnostics
                .push(RuntimeFontAtlasLoadDiagnostic {
                    code: "FontAtlasUnsafeBitmapPath".to_string(),
                    message: format!(
                        "declared FontAtlas bitmap path is unsafe: {}",
                        entry.bitmap_path
                    ),
                    path: format!("{path}.bitmapPath"),
                });
            continue;
        };
        let atlas_alpha = match fs::read(&bitmap_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                let diagnostic = RuntimeFontAtlasLoadDiagnostic {
                    code: "FontAtlasBitmapMissing".to_string(),
                    message: format!(
                        "declared FontAtlas bitmap is missing: {} ({error})",
                        entry.bitmap_path
                    ),
                    path: format!("{path}.bitmapPath"),
                };
                diagnostics.error(&diagnostic.path, &diagnostic.message);
                registry.load_report.diagnostics.push(diagnostic);
                registry
                    .load_report
                    .atlases
                    .push(RuntimeFontAtlasLoadEntry {
                        font_atlas_id: entry.font_atlas_id.clone(),
                        metadata_path: entry.metadata_path.clone(),
                        bitmap_path: entry.bitmap_path.clone(),
                        status: RuntimeFontAtlasLoadStatus::Failed,
                        glyph_count: metadata.glyphs.len(),
                        atlas_alpha_byte_len: 0,
                        font_source_kind: metadata.font_source_kind.clone(),
                        font_asset_status: metadata.font_asset_status.clone(),
                        fallback_used: metadata.fallback_used,
                        legacy_mode: true,
                        quality_gate_eligible: false,
                    });
                continue;
            }
        };

        let mut failed = false;
        if metadata.schema_version != COOKED_FONT_ATLAS_SCHEMA_VERSION {
            let diagnostic = RuntimeFontAtlasLoadDiagnostic {
                code: "FontAtlasSchemaMismatch".to_string(),
                message: format!(
                    "FontAtlas '{}' schemaVersion '{}' must be '{}'",
                    entry.font_atlas_id, metadata.schema_version, COOKED_FONT_ATLAS_SCHEMA_VERSION
                ),
                path: format!("{path}.schemaVersion"),
            };
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            failed = true;
        }
        if metadata.font_atlas_id != entry.font_atlas_id {
            let diagnostic = RuntimeFontAtlasLoadDiagnostic {
                code: "FontAtlasIdMismatch".to_string(),
                message: format!(
                    "FontAtlas manifest id '{}' differs from metadata '{}'",
                    entry.font_atlas_id, metadata.font_atlas_id
                ),
                path: format!("{path}.fontAtlasId"),
            };
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            failed = true;
        }
        if metadata.atlas_alpha_byte_len != atlas_alpha.len() {
            let diagnostic = RuntimeFontAtlasLoadDiagnostic {
                code: "FontAtlasBitmapSizeMismatch".to_string(),
                message: format!(
                    "FontAtlas '{}' expected {} alpha bytes, got {}",
                    entry.font_atlas_id,
                    metadata.atlas_alpha_byte_len,
                    atlas_alpha.len()
                ),
                path: format!("{path}.atlasAlphaByteLen"),
            };
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            failed = true;
        }
        if metadata.glyphs.is_empty() {
            let diagnostic = RuntimeFontAtlasLoadDiagnostic {
                code: "FontAtlasGlyphsEmpty".to_string(),
                message: format!(
                    "FontAtlas '{}' contains no glyph metrics.",
                    entry.font_atlas_id
                ),
                path: format!("{path}.glyphs"),
            };
            diagnostics.error(&diagnostic.path, &diagnostic.message);
            registry.load_report.diagnostics.push(diagnostic);
            failed = true;
        }

        let status = if failed {
            RuntimeFontAtlasLoadStatus::Failed
        } else {
            RuntimeFontAtlasLoadStatus::Success
        };
        registry
            .load_report
            .atlases
            .push(RuntimeFontAtlasLoadEntry {
                font_atlas_id: entry.font_atlas_id.clone(),
                metadata_path: entry.metadata_path.clone(),
                bitmap_path: entry.bitmap_path.clone(),
                status,
                glyph_count: metadata.glyphs.len(),
                atlas_alpha_byte_len: atlas_alpha.len(),
                font_source_kind: metadata.font_source_kind.clone(),
                font_asset_status: metadata.font_asset_status.clone(),
                fallback_used: metadata.fallback_used,
                legacy_mode: true,
                quality_gate_eligible: false,
            });
        if failed {
            continue;
        }
        registry.atlases_by_id.insert(
            metadata.font_atlas_id.clone(),
            RuntimeLoadedFontAtlas {
                metadata,
                atlas_alpha,
            },
        );
    }

    registry.load_report.loaded_atlas_count = registry.atlases_by_id.len();
    registry.load_report.failed_atlas_count = registry
        .load_report
        .atlases
        .iter()
        .filter(|entry| entry.status == RuntimeFontAtlasLoadStatus::Failed)
        .count();
    registry.load_report.status = if registry.load_report.failed_atlas_count == 0 {
        RuntimeFontAtlasLoadStatus::Success
    } else {
        RuntimeFontAtlasLoadStatus::Failed
    };
    registry
}

fn validate_aui_document_body(
    document: &AuiDocument,
    manifest_index: usize,
) -> Vec<RuntimeAuiDocumentLoadDiagnostic> {
    let mut diagnostics = Vec::new();
    if document.document_id.trim().is_empty() {
        diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
            code: "AuiDocumentIdMissing".to_string(),
            message: "AUI document body document_id is required.".to_string(),
            path: format!("aui.documents[{manifest_index}].documentId"),
        });
    }
    if document.canvases.is_empty() {
        diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
            code: "AuiDocumentCanvasMissing".to_string(),
            message: "AUI document requires at least one canvas.".to_string(),
            path: format!("aui.documents[{manifest_index}].canvases"),
        });
    }
    let mut node_ids = HashSet::new();
    for node in &document.nodes {
        if node.node_id.trim().is_empty() {
            diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                code: "AuiNodeIdMissing".to_string(),
                message: "AUI node_id is required.".to_string(),
                path: format!("aui.documents[{manifest_index}].nodes"),
            });
        } else if !node_ids.insert(node.node_id.as_str()) {
            diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                code: "AuiNodeIdDuplicate".to_string(),
                message: format!("duplicate AUI node_id '{}'", node.node_id),
                path: format!("aui.documents[{manifest_index}].nodes.{}", node.node_id),
            });
        }
    }
    for canvas in &document.canvases {
        if !node_ids.contains(canvas.root_node.as_str()) {
            diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                code: "AuiCanvasRootMissing".to_string(),
                message: format!(
                    "AUI canvas '{}' references missing root node '{}'",
                    canvas.canvas_id, canvas.root_node
                ),
                path: format!(
                    "aui.documents[{manifest_index}].canvases.{}",
                    canvas.canvas_id
                ),
            });
        }
    }
    for node in &document.nodes {
        if let Some(parent) = &node.parent {
            if !node_ids.contains(parent.as_str()) {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiNodeParentMissing".to_string(),
                    message: format!(
                        "AUI node '{}' references missing parent '{}'",
                        node.node_id, parent
                    ),
                    path: format!("aui.documents[{manifest_index}].nodes.{}", node.node_id),
                });
            }
        }
        for child in &node.children {
            if !node_ids.contains(child.as_str()) {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiNodeChildMissing".to_string(),
                    message: format!(
                        "AUI node '{}' references missing child '{}'",
                        node.node_id, child
                    ),
                    path: format!("aui.documents[{manifest_index}].nodes.{}", node.node_id),
                });
            }
        }
    }
    let mut feedback_profile_ids = HashSet::new();
    if let Some(registry) = &document.interaction_feedback {
        if registry.motion_scale_permille > 2000 {
            diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                code: "AuiFeedbackMotionScaleInvalid".to_string(),
                message: format!(
                    "motion_scale_permille {} is outside 0..=2000",
                    registry.motion_scale_permille
                ),
                path: format!(
                    "aui.documents[{manifest_index}].interactionFeedback.motionScalePermille"
                ),
            });
        }
        for (profile_index, profile) in registry.profiles.iter().enumerate() {
            let path = format!(
                "aui.documents[{manifest_index}].interactionFeedback.profiles[{profile_index}]"
            );
            if profile.profile_id.trim().is_empty()
                || matches!(profile.profile_id.as_str(), "auto" | "none")
            {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileIdInvalid".to_string(),
                    message: format!(
                        "feedback profile id '{}' is empty or reserved",
                        profile.profile_id
                    ),
                    path: format!("{path}.profileId"),
                });
            } else if !feedback_profile_ids.insert(profile.profile_id.as_str()) {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileDuplicate".to_string(),
                    message: format!("duplicate feedback profile id '{}'", profile.profile_id),
                    path: format!("{path}.profileId"),
                });
            }
            if [
                profile.hover_scale_permille,
                profile.pressed_scale_permille,
                profile.activated_scale_permille,
            ]
            .into_iter()
            .any(|value| !(500..=1500).contains(&value))
            {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileScaleInvalid".to_string(),
                    message: format!(
                        "feedback profile '{}' scale must be within 500..=1500 permille",
                        profile.profile_id
                    ),
                    path: path.clone(),
                });
            }
            if [
                profile.hover_opacity_permille,
                profile.pressed_opacity_permille,
                profile.activated_opacity_permille,
                profile.disabled_opacity_permille,
            ]
            .into_iter()
            .any(|value| value > 1000)
            {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileOpacityInvalid".to_string(),
                    message: format!(
                        "feedback profile '{}' opacity must be within 0..=1000 permille",
                        profile.profile_id
                    ),
                    path: path.clone(),
                });
            }
            if [
                profile.hover_brightness_permille,
                profile.pressed_brightness_permille,
                profile.activated_brightness_permille,
            ]
            .into_iter()
            .any(|value| !(-1000..=1000).contains(&value))
            {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileBrightnessInvalid".to_string(),
                    message: format!(
                        "feedback profile '{}' brightness must be within -1000..=1000 permille",
                        profile.profile_id
                    ),
                    path: path.clone(),
                });
            }
            if [
                profile.hover_in_ms,
                profile.hover_out_ms,
                profile.press_in_ms,
                profile.release_ms,
                profile.activated_ms,
                profile.cancel_ms,
            ]
            .into_iter()
            .any(|value| value > 5000)
            {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileDurationInvalid".to_string(),
                    message: format!(
                        "feedback profile '{}' duration must be within 0..=5000ms",
                        profile.profile_id
                    ),
                    path: path.clone(),
                });
            }
            if !profile.pressed_offset.x.is_finite()
                || !profile.pressed_offset.y.is_finite()
                || profile.pressed_offset.x.abs() > 2000.0
                || profile.pressed_offset.y.abs() > 2000.0
            {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileTranslationInvalid".to_string(),
                    message: format!(
                        "feedback profile '{}' translation must be finite and within +/-2000",
                        profile.profile_id
                    ),
                    path,
                });
            }
        }
        if let Some(default_profile) = registry.default_button_profile.as_deref() {
            if !feedback_profile_ids.contains(default_profile) {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileMissing".to_string(),
                    message: format!("default feedback profile '{default_profile}' is missing"),
                    path: format!(
                        "aui.documents[{manifest_index}].interactionFeedback.defaultButtonProfile"
                    ),
                });
            }
        }
    }
    for node in &document.nodes {
        if let Some(profile_id) = node.feedback.profile_id() {
            if !feedback_profile_ids.contains(profile_id) {
                diagnostics.push(RuntimeAuiDocumentLoadDiagnostic {
                    code: "AuiFeedbackProfileMissing".to_string(),
                    message: format!(
                        "AUI node '{}' references missing feedback profile '{}'",
                        node.node_id, profile_id
                    ),
                    path: format!(
                        "aui.documents[{manifest_index}].nodes.{}.feedback",
                        node.node_id
                    ),
                });
            }
        }
    }
    diagnostics
}

fn load_input_mappings(
    package_dir: &Path,
    input_index: &RuntimeManifestInputIndex,
    diagnostics: &mut RuntimeDiagnostics,
) -> (
    RuntimeInputManifest,
    Vec<InputMappingAsset>,
    Option<InputMappingAsset>,
) {
    let Some(manifest_path) = safe_package_join(
        package_dir,
        &input_index.path,
        "manifest.input.path",
        diagnostics,
    ) else {
        return (empty_input_manifest(), Vec::new(), None);
    };
    let Some(input_manifest) =
        read_json::<RuntimeInputManifest>(&manifest_path, "input", diagnostics)
    else {
        return (empty_input_manifest(), Vec::new(), None);
    };

    validate_input_manifest(&input_manifest, input_index, diagnostics);

    let mut mappings = Vec::new();
    let mut default_mapping = None;
    for entry in input_manifest.mappings.iter().filter(|entry| entry.enabled) {
        let Some(mapping_path) =
            safe_package_join(package_dir, &entry.path, "input.mappings.path", diagnostics)
        else {
            continue;
        };
        let Some(mapping) =
            read_json::<InputMappingAsset>(&mapping_path, "input.mapping", diagnostics)
        else {
            continue;
        };
        validate_input_mapping(entry, &mapping, diagnostics);
        if entry.id == input_manifest.default_mapping_id {
            default_mapping = Some(mapping.clone());
        }
        mappings.push(mapping);
    }

    if default_mapping.is_none() {
        diagnostics.error(
            "input.defaultMappingId",
            format!(
                "default InputMappingAsset '{}' was not found; RuntimePackage v2 does not provide an engine fallback.",
                input_manifest.default_mapping_id
            ),
        );
    }

    (input_manifest, mappings, default_mapping)
}

fn load_aui_manifest(
    package_dir: &Path,
    aui_index: Option<&RuntimeManifestAuiIndex>,
    diagnostics: &mut RuntimeDiagnostics,
) -> RuntimeAuiManifest {
    let Some(aui_index) = aui_index else {
        diagnostics.warning(
            "manifest.aui",
            "RuntimePackage manifest has no AUI index; using empty AUI manifest.",
        );
        return RuntimeAuiManifest::empty();
    };
    safe_package_join(
        package_dir,
        &aui_index.path,
        "manifest.aui.path",
        diagnostics,
    )
    .and_then(|path| read_json::<RuntimeAuiManifest>(&path, "aui", diagnostics))
    .unwrap_or_else(RuntimeAuiManifest::empty)
}

fn empty_input_manifest() -> RuntimeInputManifest {
    RuntimeInputManifest {
        schema_version: RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION.to_string(),
        default_mapping_id: String::new(),
        mappings: Vec::new(),
    }
}

fn validate_input_manifest(
    input_manifest: &RuntimeInputManifest,
    input_index: &RuntimeManifestInputIndex,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if input_manifest.schema_version != RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION {
        diagnostics.error(
            "input.schemaVersion",
            format!(
                "input schemaVersion must be {}",
                RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION
            ),
        );
    }
    if input_manifest.default_mapping_id.trim().is_empty() {
        diagnostics.error("input.defaultMappingId", "defaultMappingId is required");
    }
    if input_index.default_mapping_id != input_manifest.default_mapping_id {
        diagnostics.warning(
            "manifest.input.defaultMappingId",
            format!(
                "manifest defaultMappingId '{}' differs from input manifest defaultMappingId '{}'",
                input_index.default_mapping_id, input_manifest.default_mapping_id
            ),
        );
    }
    let enabled_count = input_manifest
        .mappings
        .iter()
        .filter(|entry| entry.enabled)
        .count();
    if input_index.mapping_count != enabled_count {
        diagnostics.warning(
            "manifest.input.mappingCount",
            format!(
                "manifest mappingCount {} differs from enabled input manifest count {}",
                input_index.mapping_count, enabled_count
            ),
        );
    }
    let mut ids = HashSet::new();
    for (index, entry) in input_manifest.mappings.iter().enumerate() {
        let path = format!("input.mappings[{}]", index);
        if entry.id.trim().is_empty() {
            diagnostics.error(format!("{}.id", path), "mapping id is required");
        } else if !ids.insert(entry.id.as_str()) {
            diagnostics.error(
                format!("{}.id", path),
                format!("duplicate mapping id: {}", entry.id),
            );
        }
        if entry.path.trim().is_empty() {
            diagnostics.error(format!("{}.path", path), "mapping path is required");
        }
    }
}

fn validate_input_mapping(
    entry: &RuntimeInputMappingManifestEntry,
    mapping: &InputMappingAsset,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if mapping.asset_id != entry.id {
        diagnostics.warning(
            format!("input.mappings[{}].assetId", entry.id),
            format!(
                "input mapping entry id '{}' differs from asset_id '{}'",
                entry.id, mapping.asset_id
            ),
        );
    }
    for diagnostic in mapping.validate().diagnostics {
        let path = format!("input.mappings[{}].{}", entry.id, diagnostic.code);
        match diagnostic.severity {
            InputDiagnosticSeverity::Warning => diagnostics.warning(path, diagnostic.message),
            InputDiagnosticSeverity::Error => diagnostics.error(path, diagnostic.message),
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    label: &str,
    diagnostics: &mut RuntimeDiagnostics,
) -> Option<T> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.error(
                label,
                format!("failed to read {}: {}", path.display(), error),
            );
            return None;
        }
    };
    match serde_json::from_str::<T>(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            diagnostics.error(
                label,
                format!("failed to parse {}: {}", path.display(), error),
            );
            None
        }
    }
}

fn safe_package_join(
    package_dir: &Path,
    relative_path: &str,
    label: &str,
    diagnostics: &mut RuntimeDiagnostics,
) -> Option<PathBuf> {
    match safe_join_runtime_package(package_dir, relative_path) {
        Ok(path) => Some(path),
        Err(error) => {
            diagnostics.error(label, error.to_string());
            None
        }
    }
}

fn validate_manifest(manifest: &RuntimePackageManifest, diagnostics: &mut RuntimeDiagnostics) {
    if manifest.schema_version != RUNTIME_PACKAGE_SCHEMA_VERSION {
        diagnostics.error(
            "schemaVersion",
            format!("schemaVersion must be {}", RUNTIME_PACKAGE_SCHEMA_VERSION),
        );
    }
    if manifest.package_mode != RUNTIME_PACKAGE_MODE {
        diagnostics.error(
            "packageMode",
            format!("packageMode must be {}", RUNTIME_PACKAGE_MODE),
        );
    }
    if let Some(contract) = &manifest.observation_contract {
        if let Err(contract_diagnostics) = contract.validate() {
            for diagnostic in contract_diagnostics {
                diagnostics.push(
                    RuntimeDiagnostic::error(
                        diagnostic
                            .path
                            .as_deref()
                            .map(|path| format!("manifest.observationContract.{path}"))
                            .unwrap_or_else(|| "manifest.observationContract".to_string()),
                        diagnostic.message,
                    )
                    .with_code(diagnostic.code)
                    .with_stage("runtime-package-load")
                    .with_next_action(diagnostic.next_action),
                );
            }
        }
    }
    if manifest.active_scene_id.is_empty() {
        diagnostics.error("activeSceneId", "activeSceneId is required");
    }
    if manifest.project.project_id.trim().is_empty() {
        diagnostics.error("project.projectId", "projectId is required");
    }
    if manifest.project.runtime_module.module_id.trim().is_empty() {
        diagnostics.error(
            "project.runtimeModule.moduleId",
            "runtime moduleId is required",
        );
    }
    if manifest
        .project
        .runtime_module
        .interface_version
        .trim()
        .is_empty()
    {
        diagnostics.error(
            "project.runtimeModule.interfaceVersion",
            "runtime module interfaceVersion is required",
        );
    }
    if manifest
        .project
        .runtime_module
        .aot_content_digest
        .trim()
        .is_empty()
    {
        diagnostics.error(
            "project.runtimeModule.aotContentDigest",
            "runtime module aotContentDigest is required",
        );
    }
}

fn validate_scene(
    scene: &RuntimeScene,
    assets: &RuntimeAssetManifest,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if scene.schema_version != RUNTIME_SCENE_SCHEMA_VERSION {
        diagnostics.error(
            "scene.schemaVersion",
            format!(
                "scene schemaVersion must be {}",
                RUNTIME_SCENE_SCHEMA_VERSION
            ),
        );
    }

    let mut asset_ids: HashSet<&str> = assets
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect();
    asset_ids.extend(
        assets
            .runtime_asset_index
            .iter()
            .map(|asset| asset.asset_id.as_str()),
    );
    for (index, entity) in scene.entities.iter().enumerate() {
        let entity_path = format!("scene.entities[{}]", index);
        if entity.schema_version != RUNTIME_ENTITY_SCHEMA_VERSION {
            diagnostics.error(
                format!("{}.schemaVersion", entity_path),
                format!(
                    "entity schemaVersion must be {}",
                    RUNTIME_ENTITY_SCHEMA_VERSION
                ),
            );
        }
        if entity.transform.is_none() {
            diagnostics.error(
                format!("{}.transform", entity_path),
                "transform is required",
            );
        }
        if let Some(mesh) = &entity.mesh {
            validate_mesh(mesh, &entity_path, &asset_ids, diagnostics);
        }
        if let Some(sprite) = &entity.sprite_renderer2d {
            validate_sprite_renderer2d(sprite, &entity_path, &asset_ids, diagnostics);
        }
    }
}

fn validate_mesh(
    mesh: &RuntimeMesh,
    entity_path: &str,
    asset_ids: &HashSet<&str>,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if mesh.primitive.as_deref() == Some("model") && mesh.asset_ref.is_none() {
        diagnostics.error(
            format!("{}.mesh.assetRef", entity_path),
            "model mesh assetRef is required",
        );
    }
    for (field, asset_ref) in [
        ("assetRef", mesh.asset_ref.as_ref()),
        ("materialRef", mesh.material_ref.as_ref()),
        ("textureRef", mesh.texture_ref.as_ref()),
    ] {
        if let Some(asset_ref) = asset_ref {
            if asset_ref.id.is_empty() {
                diagnostics.error(
                    format!("{}.mesh.{}.id", entity_path, field),
                    format!("{}.id is required", field),
                );
            } else if !asset_ids.contains(asset_ref.id.as_str()) {
                diagnostics.error(
                    format!("{}.mesh.{}", entity_path, field),
                    format!("{} points to missing asset: {}", field, asset_ref.id),
                );
            }
        }
    }
}

fn validate_sprite_renderer2d(
    sprite: &RuntimeSpriteRenderer2D,
    entity_path: &str,
    asset_ids: &HashSet<&str>,
    diagnostics: &mut RuntimeDiagnostics,
) {
    for (field, asset_ref) in [
        ("spriteRef", sprite.sprite_ref.as_ref()),
        ("materialRef", sprite.material_ref.as_ref()),
    ] {
        if let Some(asset_ref) = asset_ref {
            if asset_ref.id.is_empty() {
                diagnostics.error(
                    format!("{}.spriteRenderer2D.{}.id", entity_path, field),
                    format!("{}.id is required", field),
                );
            } else if !asset_ids.contains(asset_ref.id.as_str()) {
                diagnostics.error(
                    format!("{}.spriteRenderer2D.{}", entity_path, field),
                    format!("{} points to missing asset: {}", field, asset_ref.id),
                );
            }
        }
    }
}

fn validate_assets(assets: &RuntimeAssetManifest, diagnostics: &mut RuntimeDiagnostics) {
    if assets.schema_version != RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION {
        diagnostics.error(
            "assets.schemaVersion",
            format!(
                "asset manifest schemaVersion must be {}",
                RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION
            ),
        );
    }
}

fn validate_rules(rules: &RuntimeRuleManifest, diagnostics: &mut RuntimeDiagnostics) {
    if rules.schema_version != RUNTIME_RULE_MANIFEST_SCHEMA_VERSION {
        diagnostics.error(
            "rules.schemaVersion",
            format!(
                "rule manifest schemaVersion must be {}",
                RUNTIME_RULE_MANIFEST_SCHEMA_VERSION
            ),
        );
    }
    if rules.mode != "none" && rules.mode != RUNTIME_RULE_MANIFEST_MODE {
        diagnostics.error(
            "rules.mode",
            format!(
                "runtime package v1 rule manifest mode must be none or {}",
                RUNTIME_RULE_MANIFEST_MODE
            ),
        );
    }
    let mut rule_ids = HashSet::new();
    for (index, rule) in rules.rules.iter().enumerate() {
        let rule_path = format!("rules.rules[{}]", index);
        if rule.rule_id.trim().is_empty() {
            diagnostics.error(format!("{}.ruleId", rule_path), "ruleId is required");
        } else if !rule_ids.insert(rule.rule_id.as_str()) {
            diagnostics.error(
                format!("{}.ruleId", rule_path),
                format!("duplicate ruleId: {}", rule.rule_id),
            );
        }
        if rule.executor != RuntimeRuleExecutor::RustAot {
            diagnostics.error(
                format!("{}.executor", rule_path),
                "M2 v1 only supports rustAot executor",
            );
        }
        if rules.mode == RUNTIME_RULE_MANIFEST_MODE && rule.artifact_id.is_none() {
            diagnostics.error(
                format!("{}.artifactId", rule_path),
                "rust-aot rule manifest entry requires artifactId",
            );
        }
    }
    let artifact_report = validate_runtime_rule_manifest_artifacts(None, rules);
    for issue in artifact_report.issues {
        diagnostics.error(issue.path, issue.message);
    }
}

fn validate_aui_manifest(
    aui_manifest: &RuntimeAuiManifest,
    aui_index: Option<&RuntimeManifestAuiIndex>,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if aui_manifest.schema_version != RUNTIME_AUI_MANIFEST_SCHEMA_VERSION {
        diagnostics.error(
            "aui.schemaVersion",
            format!(
                "aui manifest schemaVersion must be {}",
                RUNTIME_AUI_MANIFEST_SCHEMA_VERSION
            ),
        );
    }
    if let Some(aui_index) = aui_index {
        if aui_index.document_count != aui_manifest.documents.len() {
            diagnostics.error(
                "manifest.aui.documentCount",
                format!(
                    "manifest documentCount {} differs from AUI manifest document count {}",
                    aui_index.document_count,
                    aui_manifest.documents.len()
                ),
            );
        }
    }
    let mut document_ids = HashSet::new();
    for (index, document) in aui_manifest.documents.iter().enumerate() {
        let path = format!("aui.documents[{}]", index);
        if document.document_id.trim().is_empty() {
            diagnostics.error(format!("{}.documentId", path), "documentId is required");
        } else if !document_ids.insert(document.document_id.as_str()) {
            diagnostics.error(
                format!("{}.documentId", path),
                format!("duplicate AUI documentId '{}'", document.document_id),
            );
        }
        if document.path.trim().is_empty() {
            diagnostics.error(format!("{}.path", path), "path is required");
        }
    }
}

fn validate_font_atlas_manifest(
    font_manifest: &RuntimeFontAtlasManifest,
    font_index: Option<&RuntimeManifestFontAtlasIndex>,
    diagnostics: &mut RuntimeDiagnostics,
) {
    if font_manifest.schema_version != RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION {
        diagnostics.error(
            "fontAtlases.schemaVersion",
            format!(
                "font atlas manifest schemaVersion must be {}",
                RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION
            ),
        );
    }
    if let Some(font_index) = font_index {
        if font_index.atlas_count != font_manifest.atlases.len() {
            diagnostics.error(
                "manifest.fontAtlases.atlasCount",
                format!(
                    "manifest atlasCount {} differs from font atlas manifest count {}",
                    font_index.atlas_count,
                    font_manifest.atlases.len()
                ),
            );
        }
        if font_index.default_ui_font_atlas_id != font_manifest.default_ui_font_atlas_id {
            diagnostics.warning(
                "manifest.fontAtlases.defaultUiFontAtlasId",
                "manifest default UI font atlas id differs from font atlas manifest.",
            );
        }
    }
    let mut atlas_ids = HashSet::new();
    for (index, atlas) in font_manifest.atlases.iter().enumerate() {
        let path = format!("fontAtlases.atlases[{index}]");
        if atlas.font_atlas_id.trim().is_empty() {
            diagnostics.error(format!("{path}.fontAtlasId"), "fontAtlasId is required");
        } else if !atlas_ids.insert(atlas.font_atlas_id.as_str()) {
            diagnostics.error(
                format!("{path}.fontAtlasId"),
                format!("duplicate fontAtlasId '{}'", atlas.font_atlas_id),
            );
        }
        if atlas.metadata_path.trim().is_empty() {
            diagnostics.error(format!("{path}.metadataPath"), "metadataPath is required");
        }
        if atlas.bitmap_path.trim().is_empty() {
            diagnostics.error(format!("{path}.bitmapPath"), "bitmapPath is required");
        }
        if atlas.glyph_count == 0 {
            diagnostics.error(
                format!("{path}.glyphCount"),
                "glyphCount must be greater than zero",
            );
        }
        if atlas.atlas_width == 0 || atlas.atlas_height == 0 {
            diagnostics.error(
                format!("{path}.atlasSize"),
                "atlasWidth and atlasHeight must be greater than zero",
            );
        }
    }
    if let Some(default_id) = &font_manifest.default_ui_font_atlas_id {
        if !font_manifest
            .atlases
            .iter()
            .any(|atlas| atlas.font_atlas_id == *default_id)
        {
            diagnostics.error(
                "fontAtlases.defaultUiFontAtlasId",
                format!("default UI font atlas id '{default_id}' does not exist"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_package_rejects_invalid_aui_feedback_legacy_schema() {
        let mut document = AuiDocument::new("legacy", Vec::new(), Vec::new());
        document.schema_version = crate::aui::LEGACY_AUI_DOCUMENT_SCHEMA_VERSION.to_string();

        let diagnostic = validate_runtime_aui_document_schema(&document, 0)
            .expect("runtime package must reject source v1 AUI document");

        assert_eq!(diagnostic.code, "AuiDocumentSchemaMismatch");
        assert!(diagnostic.message.contains(AUI_DOCUMENT_SCHEMA_VERSION));
    }

    #[test]
    fn runtime_package_rejects_invalid_aui_feedback_v2_values() {
        let mut document = AuiDocument::new("invalid-feedback", Vec::new(), Vec::new());
        let mut profile = crate::aui::AuiInteractionFeedbackProfile::new("ink.invalid");
        profile.hover_scale_permille = 1600;
        profile.pressed_opacity_permille = 1001;
        profile.activated_ms = 5001;
        document.interaction_feedback = Some(crate::aui::AuiInteractionFeedbackRegistry {
            motion_scale_permille: 2001,
            default_button_profile: Some("ink.missing".to_string()),
            profiles: vec![profile],
        });

        let diagnostics = validate_aui_document_body(&document, 0);
        for code in [
            "AuiFeedbackMotionScaleInvalid",
            "AuiFeedbackProfileScaleInvalid",
            "AuiFeedbackProfileOpacityInvalid",
            "AuiFeedbackProfileDurationInvalid",
            "AuiFeedbackProfileMissing",
        ] {
            assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == code));
        }
    }

    #[test]
    fn runtime_package_loader_loads_valid_package() {
        let package_dir = write_fixture("valid", true, true, true);
        let result = load_runtime_package(&package_dir);
        assert!(
            result.diagnostics.is_ok(),
            "{:?}",
            result.diagnostics.issues
        );
        let package = result.value.expect("package should load");
        assert_eq!(package.manifest.active_scene_id, "scene-main");
        assert_eq!(package.active_scene.entities.len(), 1);
        let asset_ref = RuntimeAssetRef {
            id: "model-ship".to_string(),
            asset_type: "model".to_string(),
            guid: None,
            sub_asset: None,
        };
        assert_eq!(
            package
                .runtime_asset_index
                .resolve(&asset_ref)
                .unwrap()
                .cooked_asset_id,
            "model-ship"
        );
    }

    #[test]
    fn runtime_package_loader_reads_runtime_asset_tables() {
        let package_dir = write_fixture("runtime-asset-tables", true, true, true);
        fs::write(
            package_dir.join("assets").join("asset-manifest.json"),
            r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [],
  "runtimeAssetIndex": [{
    "assetGuid": "guid-model",
    "assetId": "model-ship",
    "assetType": "model",
    "version": "2",
    "cookedAssetId": "cooked-model",
    "bundleId": "startup",
    "loaderKind": "model",
    "dependencies": [],
    "flags": ["runtime_table"]
  }, {
    "assetGuid": "guid-texture",
    "assetId": "texture-main",
    "assetType": "texture",
    "version": "2",
    "cookedAssetId": "cooked-texture",
    "bundleId": "startup",
    "loaderKind": "texture",
    "dependencies": [],
    "flags": ["runtime_table"]
  }],
  "bundleTable": [{
    "bundleId": "startup",
    "uri": "bundles/startup",
    "mounted": true
  }],
  "cookedAssetTable": [{
    "cookedAssetId": "cooked-model",
    "bundleId": "startup",
    "path": "cooked/model.bin"
  }, {
    "cookedAssetId": "cooked-texture",
    "bundleId": "startup",
    "path": "cooked/texture.bin"
  }],
  "dependencyTable": []
}"#,
        )
        .unwrap();

        let result = load_runtime_package(&package_dir);
        assert!(
            result.diagnostics.is_ok(),
            "{:?}",
            result.diagnostics.issues
        );
        let package = result.value.expect("package should load");
        let asset_ref = RuntimeAssetRef {
            id: "texture-main".to_string(),
            asset_type: "texture".to_string(),
            guid: Some("guid-texture".to_string()),
            sub_asset: None,
        };
        let record = package.runtime_asset_index.resolve(&asset_ref).unwrap();
        assert_eq!(record.version, "2");
        assert!(package
            .runtime_asset_mount_table
            .is_bundle_mounted("startup"));
    }

    #[test]
    fn runtime_entity_reads_sprite_renderer2d_from_json() {
        let entity: RuntimeEntity = serde_json::from_str(
            r##"{
  "schemaVersion": "runtime-entity.v1",
  "id": "sprite-entity",
  "name": "Sprite",
  "kind": "actor",
  "enabled": true,
  "parentId": null,
  "siblingOrder": 0,
  "transform": {
    "localPosition": { "x": 0, "y": 0, "z": 0 },
    "localRotation": { "x": 0, "y": 0, "z": 0 },
    "localScale": { "x": 1, "y": 1, "z": 1 }
  },
  "spriteRenderer2D": {
    "spriteRef": { "id": "sprite-ship", "type": "texture" },
    "materialRef": { "id": "material-sprite", "type": "material" },
    "color": [0.2, 0.4, 0.6, 1.0],
    "flipX": true,
    "sortingLayer": 2,
    "orderInLayer": 7,
    "sortZ": 0.5,
    "visible": true
  }
}"##,
        )
        .expect("entity should deserialize");

        let sprite = entity.sprite_renderer2d.expect("sprite renderer");
        assert_eq!(sprite.sprite_ref.unwrap().id, "sprite-ship");
        assert_eq!(sprite.material_ref.unwrap().id, "material-sprite");
        assert_eq!(sprite.color, Some([0.2, 0.4, 0.6, 1.0]));
        assert_eq!(sprite.flip_x, Some(true));
        assert_eq!(sprite.order_in_layer, Some(7));
    }

    #[test]
    fn sprite_renderer2d_missing_asset_reports_diagnostics() {
        let package_dir = write_fixture("missing-sprite-asset", true, true, true);
        fs::write(
            package_dir.join("scenes").join("scene-main.json"),
            r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
    "id": "ship",
    "name": "Ship",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 1, "y": 2, "z": 3 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },
    "spriteRenderer2D": {
      "spriteRef": { "id": "missing-sprite", "type": "texture" }
    }
  }]
}"##,
        )
        .unwrap();

        let result = load_runtime_package(&package_dir);

        assert!(result.value.is_none());
        assert!(result
            .diagnostics
            .issues
            .iter()
            .any(|issue| issue.path.contains("spriteRenderer2D.spriteRef")));
    }

    #[test]
    fn invalid_runtime_package_reports_diagnostics() {
        let package_dir = write_fixture("missing-asset", true, false, true);
        let result = load_runtime_package(&package_dir);
        assert!(result.value.is_none());
        assert!(result
            .diagnostics
            .issues
            .iter()
            .any(|issue| issue.path.contains("mesh.assetRef")));
    }

    #[test]
    fn missing_transform_reports_diagnostics() {
        let package_dir = write_fixture("missing-transform", false, true, true);
        let result = load_runtime_package(&package_dir);
        assert!(result.value.is_none());
        assert!(result
            .diagnostics
            .issues
            .iter()
            .any(|issue| issue.path.contains("transform")));
    }

    #[test]
    fn rule_mode_rejects_unknown_modes() {
        let package_dir = write_fixture("bad-rule-mode", true, true, false);
        let result = load_runtime_package(&package_dir);
        assert!(result.value.is_none());
        assert!(result
            .diagnostics
            .issues
            .iter()
            .any(|issue| issue.path == "rules.mode"));
    }

    #[test]
    fn runtime_package_loader_accepts_rust_aot_rule_manifest() {
        let package_dir = write_fixture("rust-aot-rule-mode", true, true, true);
        fs::write(
            package_dir.join("rules").join("rule-manifest.json"),
            r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "rust-aot",
  "rules": [{
    "ruleId": "project.rule.move",
    "phase": "Update",
    "enabled": true,
    "executor": "rustAot",
    "irSource": "Rules/move.rule.ir.json",
    "irHash": "hash",
    "artifactId": "rule-artifact:project.rule.move:hash"
  }],
  "modules": [{
    "artifactId": "rule-artifact:project.rule.move:hash",
    "moduleKind": "staticRegistry"
  }]
}"#,
        )
        .unwrap();

        let result = load_runtime_package(&package_dir);

        assert!(
            result.diagnostics.is_ok(),
            "{:?}",
            result.diagnostics.issues
        );
        let package = result.value.expect("package should load");
        assert_eq!(package.rules.mode, RUNTIME_RULE_MANIFEST_MODE);
        assert_eq!(package.rules.rules[0].rule_id, "project.rule.move");
    }

    #[test]
    fn runtime_package_loader_reads_project_default_input_mapping() {
        let package_dir = write_fixture("project-input-mapping", true, true, true);
        let custom_mapping = InputMappingAsset::new(
            "input.project",
            vec![engine_input::InputActionDefinition::new(
                "action.launch",
                engine_input::InputActionValueType::Button,
            )],
            vec![engine_input::InputContextDefinition::new("gameplay", 0)],
            vec![engine_input::InputBindingDefinition::button(
                "action.launch",
                "KeyL",
            )],
        );
        fs::write(
            package_dir.join("input").join("input-manifest.json"),
            r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.project",
  "mappings": [{ "id": "input.project", "path": "input/input.project.json", "enabled": true }]
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input.project.json"),
            serde_json::to_string_pretty(&custom_mapping).unwrap(),
        )
        .unwrap();
        let manifest_path = package_dir.join("manifest.json");
        let manifest_text = fs::read_to_string(&manifest_path).unwrap().replace(
            r#""input": { "path": "input/input-manifest.json", "defaultMappingId": "input.default", "mappingCount": 1 }"#,
            r#""input": { "path": "input/input-manifest.json", "defaultMappingId": "input.project", "mappingCount": 1 }"#,
        );
        fs::write(manifest_path, manifest_text).unwrap();

        let result = load_runtime_package(&package_dir);

        assert!(
            result.diagnostics.is_ok(),
            "{:?}",
            result.diagnostics.issues
        );
        let package = result.value.expect("package should load");
        let mapping = package
            .default_input_mapping
            .as_ref()
            .expect("default mapping");
        assert_eq!(mapping.asset_id, "input.project");
        assert_eq!(mapping.bindings[0].device_path, "keyboard/KeyL");
    }

    #[test]
    fn runtime_package_loader_rejects_missing_input_index() {
        let package_dir = write_fixture("missing-input-index", true, true, true);
        let manifest_path = package_dir.join("manifest.json");
        let manifest_text = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace(
                ",\n  \"input\": { \"path\": \"input/input-manifest.json\", \"defaultMappingId\": \"input.default\", \"mappingCount\": 1 }",
                "",
            );
        fs::write(manifest_path, manifest_text).unwrap();

        let result = load_runtime_package(&package_dir);

        assert!(result.value.is_none());
        assert!(result.diagnostics.issues.iter().any(
            |issue| issue.path == "manifest" && issue.message.contains("missing field `input`")
        ));
    }

    fn write_fixture(
        name: &str,
        include_transform: bool,
        include_asset: bool,
        rules_none: bool,
    ) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("engine-runtime-{}-{}", name, stamp));
        let package_dir = root.join("runtime-package");
        fs::create_dir_all(package_dir.join("scenes")).unwrap();
        fs::create_dir_all(package_dir.join("assets")).unwrap();
        fs::create_dir_all(package_dir.join("input")).unwrap();
        fs::create_dir_all(package_dir.join("rules")).unwrap();

        fs::write(
            package_dir.join("manifest.json"),
            r##"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "project-fixture",
    "name": "Fixture",
    "version": "0.0.1",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 1 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "none" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.default", "mappingCount": 1 }
}"##,
        )
        .unwrap();

        let transform = if include_transform {
            r#""transform": {
      "localPosition": { "x": 1, "y": 2, "z": 3 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },"#
        } else {
            ""
        };
        fs::write(
            package_dir.join("scenes").join("scene-main.json"),
            format!(
                r##"{{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{{
    "schemaVersion": "runtime-entity.v1",
    "id": "ship",
    "name": "Ship",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    {}
    "mesh": {{
      "primitive": "model",
      "assetRef": {{ "id": "model-ship", "type": "model" }},
      "visible": true,
      "layer": "default"
    }}
  }}]
}}"##,
                transform
            ),
        )
        .unwrap();
        let assets = if include_asset {
            r#"[{ "id": "model-ship", "name": "Ship", "type": "model", "source": "ship.glb", "state": "available", "bundleId": "startup" }]"#
        } else {
            "[]"
        };
        fs::write(
            package_dir.join("assets").join("asset-manifest.json"),
            format!(
                r#"{{ "schemaVersion": "runtime-asset-manifest.v1", "assets": {} }}"#,
                assets
            ),
        )
        .unwrap();
        fs::write(
            package_dir.join("rules").join("rule-manifest.json"),
            format!(
                r#"{{ "schemaVersion": "runtime-rule-manifest.v1", "mode": "{}", "rules": [], "modules": [] }}"#,
                if rules_none { "none" } else { "compiled" }
            ),
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input-manifest.json"),
            r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.default",
  "mappings": [{ "id": "input.default", "path": "input/input.default.json", "enabled": true }]
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input.default.json"),
            serde_json::to_string_pretty(&InputMappingAsset::gameplay_default()).unwrap(),
        )
        .unwrap();
        package_dir
    }
}
