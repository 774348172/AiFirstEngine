use crate::{
    scan_input_mapping_paths, Animator2DAssetCooker, AnimatorController2DAsset,
    AuiDocumentCookRequest, AuiDocumentCooker, EditorAssetRef, EditorMesh, EditorSceneComponent,
    EditorSceneDocument, EditorSceneEntity, EditorTransform, EditorVec3, EngineBuiltInFontPack,
    FontAtlasProfileRole, InputMappingAuthoringService, PrefabAsset, PrefabDiagnostic,
    PrefabDiagnosticSeverity, PrefabInstance, ProjectAssemblyArtifactCache,
    ProjectAssemblyProducerReport, ProjectAssetImport, ProjectFontCookFailure,
    ProjectFontCookModule, ProjectManifest, ProjectRelativePath, ResolvedPrefabEntity,
    ResolvedPrefabView, SpriteAnimationClip2DAsset, PREFAB_ASSET_SCHEMA_VERSION,
    PREFAB_INSTANCE_COMPONENT_TYPE, PROJECT_MANIFEST_SCHEMA_VERSION,
};
use engine_input::InputDiagnosticSeverity;
use engine_runtime::animator2d::{CookedAnimator2DRegistry, RuntimeAnimator2D};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_observation::ProjectObservationContract;
use engine_runtime::project_runtime_module::{
    project_runtime_aot_digest, ProjectRuntimeAotDigestSource, EMPTY_PROJECT_RUNTIME_AOT_DIGEST,
    EMPTY_PROJECT_RUNTIME_MODULE_ID,
};
use engine_runtime::rule_artifact::validate_runtime_rule_manifest_artifacts;
use engine_runtime::runtime_package::{
    CookedTextureAsset, RuntimeAssetRef, RuntimeAuiManifest, RuntimeAuiManifestEntry,
    RuntimeEntity, RuntimeMesh, RuntimeProjectComponent, RuntimeProjectInfo,
    RuntimeProjectModuleRef, RuntimeRuleManifest, RuntimeScene, RuntimeSpriteRenderer2D,
    RuntimeTransform, Vector3, COOKED_TEXTURE_SCHEMA_VERSION, RUNTIME_AUI_MANIFEST_SCHEMA_VERSION,
    RUNTIME_ENTITY_SCHEMA_VERSION, RUNTIME_SCENE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildInput, RuntimePackageSourceAsset, RuntimePackageSourceJson,
    RuntimePackageSourcePrefab, RuntimePackageSourceTexture,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const BUILD_PROFILE_SCHEMA_VERSION: &str = "build-profile.v2";
pub const BUILD_PROFILE_SCHEMA_VERSION_V1: &str = "build-profile.v1";
pub const PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION: &str =
    "project-runtime-package-assembly-report.v2";
pub const PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION: &str = "prefab-runtime-bake-report.v1";
const PREFAB_RUNTIME_ENTITY_ID_SEPARATOR: &str = "__";
const AUI_SOURCE_ROOTS: [&str; 3] = ["AUI", "UI", "Assets/UI"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimePackageAssemblyRequest {
    pub project_root: PathBuf,
    pub build_profile_path: Option<PathBuf>,
    pub artifact_cache_root: Option<PathBuf>,
}

impl ProjectRuntimePackageAssemblyRequest {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            build_profile_path: None,
            artifact_cache_root: None,
        }
    }

    pub fn with_build_profile_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.build_profile_path = Some(path.into());
        self
    }

    pub fn with_artifact_cache_root(mut self, path: impl Into<PathBuf>) -> Self {
        self.artifact_cache_root = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildProfile {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub profile: String,
    pub target: String,
    pub runtime_package_mode: String,
    pub frame_limit: u64,
    pub headless_surface_gate: bool,
    pub real_window_smoke: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<BuildProfileApplication>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<BuildProfileRelease>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildProfileApplication {
    pub display_name: String,
    pub executable_name: String,
    pub company_name: String,
    pub file_description: String,
    pub display_version: String,
    pub windows_file_version: [u16; 4],
    pub windows_product_version: [u16; 4],
    pub copyright: String,
    pub icon: BuildProfileIconRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildProfileIconRef {
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildProfileRelease {
    pub layout: String,
    pub include_reports: bool,
    #[serde(default)]
    pub include_debug_symbols: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProfileValidationIssue {
    pub code: &'static str,
    pub field: &'static str,
    pub message: String,
    pub next_action: &'static str,
}

impl BuildProfileValidationIssue {
    fn new(
        code: &'static str,
        field: &'static str,
        message: impl Into<String>,
        next_action: &'static str,
    ) -> Self {
        Self {
            code,
            field,
            message: message.into(),
            next_action,
        }
    }
}

impl BuildProfile {
    pub fn validation_issues(&self) -> Vec<BuildProfileValidationIssue> {
        if self.schema_version == BUILD_PROFILE_SCHEMA_VERSION_V1 {
            return self.v1_validation_issues();
        }
        if self.schema_version != BUILD_PROFILE_SCHEMA_VERSION {
            return vec![BuildProfileValidationIssue::new(
                "release_profile_schema_unsupported",
                "schemaVersion",
                format!(
                    "Build profile schema must be {} or {}, got {}.",
                    BUILD_PROFILE_SCHEMA_VERSION_V1,
                    BUILD_PROFILE_SCHEMA_VERSION,
                    self.schema_version
                ),
                "Use build-profile.v1 for dev or build-profile.v2 for release.",
            )];
        }

        let mut issues = self.common_validation_issues();
        if self.profile != "release" {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "profile",
                "build-profile.v2 is reserved for the release profile.",
                "Set profile to release.",
            ));
        }
        match self.architecture.as_deref() {
            Some("x86_64") => {}
            Some(value) => issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "architecture",
                format!("Unsupported Windows release architecture: {value}."),
                "Set architecture to x86_64 for portable-directory-v1.",
            )),
            None => issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "architecture",
                "Release profile is missing architecture.",
                "Add architecture: x86_64.",
            )),
        }
        match &self.application {
            Some(application) => validate_release_application(application, &mut issues),
            None => issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "application",
                "Release profile is missing application identity.",
                "Add the complete application metadata block.",
            )),
        }
        match &self.release {
            Some(release) => validate_release_settings(release, &mut issues),
            None => issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "release",
                "Release profile is missing release settings.",
                "Add the portable-directory-v1 release block.",
            )),
        }
        issues
    }

    pub fn is_release_v2(&self) -> bool {
        self.schema_version == BUILD_PROFILE_SCHEMA_VERSION && self.profile == "release"
    }

    fn v1_validation_issues(&self) -> Vec<BuildProfileValidationIssue> {
        let mut issues = self.common_validation_issues();
        if self.profile != "dev" {
            issues.push(BuildProfileValidationIssue::new(
                "build_profile_v1_invalid",
                "profile",
                "build-profile.v1 only supports the dev profile.",
                "Set profile to dev or migrate the release profile to build-profile.v2.",
            ));
        }
        if self.architecture.is_some() || self.application.is_some() || self.release.is_some() {
            issues.push(BuildProfileValidationIssue::new(
                "build_profile_v1_invalid",
                "schemaVersion",
                "build-profile.v1 cannot contain v2 release fields.",
                "Remove release fields or set schemaVersion to build-profile.v2.",
            ));
        }
        issues
    }

    fn common_validation_issues(&self) -> Vec<BuildProfileValidationIssue> {
        let mut issues = Vec::new();
        if self.target != "windows" {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "target",
                format!("Unsupported build target: {}.", self.target),
                "Set target to windows.",
            ));
        }
        if self.runtime_package_mode != "debug-readable" {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "runtimePackageMode",
                format!(
                    "Unsupported RuntimePackage mode: {}.",
                    self.runtime_package_mode
                ),
                "Use debug-readable until a cooked/archive RuntimePackage system is designed.",
            ));
        }
        if self.frame_limit == 0 {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "frameLimit",
                "frameLimit must be greater than zero.",
                "Set a positive deterministic frame limit.",
            ));
        }
        if !matches!(
            self.real_window_smoke.as_str(),
            "disabled" | "optional" | "required"
        ) {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "realWindowSmoke",
                format!(
                    "Unsupported realWindowSmoke value: {}.",
                    self.real_window_smoke
                ),
                "Use disabled, optional, or required.",
            ));
        }
        issues
    }
}

fn validate_release_application(
    application: &BuildProfileApplication,
    issues: &mut Vec<BuildProfileValidationIssue>,
) {
    for (field, value) in [
        ("application.displayName", application.display_name.as_str()),
        ("application.companyName", application.company_name.as_str()),
        (
            "application.fileDescription",
            application.file_description.as_str(),
        ),
        (
            "application.displayVersion",
            application.display_version.as_str(),
        ),
        ("application.copyright", application.copyright.as_str()),
    ] {
        if value.trim().is_empty() {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                field,
                format!("{field} must not be empty."),
                "Provide explicit release application metadata.",
            ));
        }
    }
    if let Err(message) = validate_windows_executable_name(&application.executable_name) {
        issues.push(BuildProfileValidationIssue::new(
            "release_executable_name_invalid",
            "application.executableName",
            message,
            "Use a stable ASCII Windows file name without an extension or path separators.",
        ));
    }
    if application.icon.asset_id.trim().is_empty()
        || application.icon.asset_id == "."
        || application.icon.asset_id == ".."
        || application.icon.asset_id.contains(['/', '\\'])
    {
        issues.push(BuildProfileValidationIssue::new(
            "release_icon_asset_missing",
            "application.icon.assetId",
            "Application icon must be a non-empty AssetRef assetId, not a path.",
            "Select a project Texture/Sprite asset through the Asset Picker.",
        ));
    }
}

fn validate_release_settings(
    release: &BuildProfileRelease,
    issues: &mut Vec<BuildProfileValidationIssue>,
) {
    if release.layout != "portable-directory-v1" {
        issues.push(BuildProfileValidationIssue::new(
            "release_identity_invalid",
            "release.layout",
            format!("Unsupported release layout: {}.", release.layout),
            "Set release.layout to portable-directory-v1.",
        ));
    }
    if release.include_reports {
        issues.push(BuildProfileValidationIssue::new(
            "release_identity_invalid",
            "release.includeReports",
            "portable-directory-v1 must exclude editor reports.",
            "Set release.includeReports to false.",
        ));
    }
    if release.include_debug_symbols {
        issues.push(BuildProfileValidationIssue::new(
            "release_identity_invalid",
            "release.includeDebugSymbols",
            "portable-directory-v1 does not publish debug symbols.",
            "Set release.includeDebugSymbols to false.",
        ));
    }
}

fn validate_windows_executable_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.trim() != name || name == "." || name == ".." {
        return Err("Executable name is empty, dot-only, or has surrounding whitespace.".into());
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("Executable name cannot end with a dot or space.".into());
    }
    if name.contains(['/', '\\']) || Path::new(name).is_absolute() {
        return Err("Executable name must be a single Windows path segment.".into());
    }
    if !name.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.')
    }) {
        return Err("Executable name contains unsupported Windows file-name characters.".into());
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            });
    if reserved {
        return Err(format!(
            "Executable name uses reserved Windows device name {stem}."
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ProjectRuntimePackageAssemblyResult {
    pub status: ProjectRuntimePackageAssemblyStatus,
    pub build_input: Option<RuntimePackageBuildInput>,
    pub active_scene_id: Option<String>,
    pub build_profile: Option<BuildProfile>,
    pub report: ProjectRuntimePackageAssemblyReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectRuntimePackageAssemblyStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabRuntimeBakeReport {
    pub schema_version: String,
    pub status: ProjectRuntimePackageAssemblyStatus,
    pub report_mode: String,
    pub project_root: String,
    pub scene_id: String,
    pub prefab_asset_count: usize,
    pub scene_prefab_instance_count: usize,
    pub baked_instance_count: usize,
    pub baked_entity_count: usize,
    pub instances: Vec<PrefabRuntimeBakeInstanceEntry>,
    pub diagnostics: Vec<ProjectRuntimePackageAssemblyDiagnostic>,
}

impl PrefabRuntimeBakeReport {
    fn new(project_root: &Path, scene: &EditorSceneDocument, prefab_asset_count: usize) -> Self {
        Self {
            schema_version: PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectRuntimePackageAssemblyStatus::Success,
            report_mode: "summary".to_string(),
            project_root: project_root.display().to_string(),
            scene_id: scene.scene_id.clone(),
            prefab_asset_count,
            scene_prefab_instance_count: 0,
            baked_instance_count: 0,
            baked_entity_count: 0,
            instances: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn finish(&mut self) {
        self.status =
            if self.diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == ProjectRuntimePackageAssemblySeverity::Error
            }) {
                ProjectRuntimePackageAssemblyStatus::Failed
            } else {
                ProjectRuntimePackageAssemblyStatus::Success
            };
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabRuntimeBakeInstanceEntry {
    pub scene_entity_id: String,
    pub instance_id: String,
    pub prefab_id: String,
    pub root_source_entity_id: String,
    pub root_runtime_entity_id: String,
    pub emitted_entity_ids: Vec<String>,
    pub applied_override_count: usize,
    pub ignored_authoring_component_types: Vec<String>,
    pub local_runtime_component_warnings: Vec<String>,
    pub diagnostics: Vec<ProjectRuntimePackageAssemblyDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimePackageAssemblyReport {
    pub schema_version: String,
    pub status: ProjectRuntimePackageAssemblyStatus,
    pub project_root: String,
    pub active_scene_id: Option<String>,
    pub scene_count: usize,
    pub prefab_count: usize,
    pub asset_count: usize,
    pub rule_count: usize,
    pub input_mapping_count: usize,
    pub aui_document_count: usize,
    pub font_atlas_count: usize,
    #[serde(default)]
    pub font_bundle_count: usize,
    pub prefab_bake_report: Option<PrefabRuntimeBakeReport>,
    #[serde(default)]
    pub source_mappings: Vec<ProjectRuntimeSourceMapping>,
    #[serde(default)]
    pub producer_reports: Vec<ProjectAssemblyProducerReport>,
    pub diagnostics: Vec<ProjectRuntimePackageAssemblyDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimeSourceMapping {
    pub domain: ProjectRuntimePackageAssemblyDomain,
    pub source_path: String,
    pub object_id: String,
    pub build_input_path: String,
    pub runtime_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRuntimePackageAssemblyDomain {
    Project,
    BuildProfile,
    Scene,
    Prefab,
    Asset,
    Rule,
    Aui,
    Input,
    Animator2D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRuntimePackageAssemblySeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuntimePackageAssemblyDiagnostic {
    pub severity: ProjectRuntimePackageAssemblySeverity,
    pub domain: ProjectRuntimePackageAssemblyDomain,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub suggestion: Option<String>,
}

impl ProjectRuntimePackageAssemblyDiagnostic {
    pub fn error(
        domain: ProjectRuntimePackageAssemblyDomain,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: ProjectRuntimePackageAssemblySeverity::Error,
            domain,
            code: code.into(),
            message: message.into(),
            path: None,
            stage: None,
            suggestion: None,
        }
    }

    pub fn warning(
        domain: ProjectRuntimePackageAssemblyDomain,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: ProjectRuntimePackageAssemblySeverity::Warning,
            domain,
            code: code.into(),
            message: message.into(),
            path: None,
            stage: None,
            suggestion: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_stage(mut self, stage: impl Into<String>) -> Self {
        self.stage = Some(stage.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextureAssetDescriptor {
    #[serde(rename = "schemaVersion", alias = "schema_version")]
    schema_version: String,
    #[serde(default, rename = "assetId", alias = "asset_id")]
    asset_id: Option<String>,
    #[serde(rename = "sourceImage", alias = "source_image")]
    source_image: String,
    #[serde(default)]
    importer: TextureImporterDescriptor,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextureImporterDescriptor {
    #[serde(default = "default_texture_format")]
    format: String,
    #[serde(default = "default_texture_color_space")]
    color_space: String,
    #[serde(default = "default_texture_sampler")]
    sampler: String,
}

impl Default for TextureImporterDescriptor {
    fn default() -> Self {
        Self {
            format: default_texture_format(),
            color_space: default_texture_color_space(),
            sampler: default_texture_sampler(),
        }
    }
}

fn default_texture_format() -> String {
    "png".to_string()
}

fn default_texture_color_space() -> String {
    "srgb".to_string()
}

fn default_texture_sampler() -> String {
    "linearClamp".to_string()
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

pub struct ProjectRuntimePackageAssembler;

impl ProjectRuntimePackageAssembler {
    pub fn assemble(
        request: ProjectRuntimePackageAssemblyRequest,
    ) -> ProjectRuntimePackageAssemblyResult {
        let project_scene_started = Instant::now();
        let mut producer_reports = Vec::new();
        let mut diagnostics = Vec::new();
        let project = match read_project_manifest(&request.project_root) {
            Ok(project) => project,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                return failed_result(&request.project_root, diagnostics);
            }
        };
        let build_profile = read_build_profile(&request, &mut diagnostics);
        let scene_path = request.project_root.join(&project.default_scene);
        let scene = match EditorSceneDocument::load_from_path(&scene_path) {
            Ok(scene) => scene,
            Err(scene_diagnostics) => {
                diagnostics.extend(scene_diagnostics.into_iter().map(|diagnostic| {
                    ProjectRuntimePackageAssemblyDiagnostic::error(
                        ProjectRuntimePackageAssemblyDomain::Scene,
                        diagnostic.code,
                        diagnostic.message,
                    )
                    .with_path(scene_path.display().to_string())
                }));
                return failed_result(&request.project_root, diagnostics);
            }
        };

        let runtime_module_digest = match runtime_module_digest(&request.project_root, &project) {
            Ok(digest) => digest,
            Err(error) => {
                diagnostics.push(ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Project,
                    "ProjectRuntimeModuleDigestFailed",
                    error.to_string(),
                ));
                return failed_result(&request.project_root, diagnostics);
            }
        };
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::new(
            project.project_id.clone(),
            project.project_name.clone(),
            project.engine_version.clone(),
            RuntimeProjectModuleRef::new(
                project.runtime_module.module_id.clone(),
                project.runtime_module.interface_version.clone(),
                runtime_module_digest,
            ),
        ));
        input.observation_contract =
            collect_project_observation_contract(&request.project_root, &project, &mut diagnostics);
        producer_reports.push(ProjectAssemblyProducerReport::uncached(
            "project-scene",
            duration_ms(project_scene_started.elapsed()),
        ));
        let prefab_started = Instant::now();
        let mut assets = BTreeMap::<String, RuntimePackageSourceAsset>::new();
        register_runtime_scene_asset(&scene, &mut assets);
        let runtime_source_prefabs =
            collect_prefabs(&request.project_root, &mut assets, &mut diagnostics);
        let prefab_bake_catalog = build_prefab_bake_catalog(
            &request.project_root,
            runtime_source_prefabs,
            &mut diagnostics,
        );
        let (runtime_scene, prefab_bake_report) = editor_scene_to_runtime(
            &scene,
            &request.project_root,
            &mut assets,
            &prefab_bake_catalog,
        );
        diagnostics.extend(prefab_bake_report.diagnostics.iter().cloned());
        input.scenes.push(runtime_scene);
        input.prefabs = prefab_bake_catalog.runtime_source_prefabs.clone();
        producer_reports.push(ProjectAssemblyProducerReport::uncached(
            "prefab",
            duration_ms(prefab_started.elapsed()),
        ));
        let input_started = Instant::now();
        input.input_mappings = collect_input_mappings(&request.project_root, &mut diagnostics);
        producer_reports.push(ProjectAssemblyProducerReport::uncached(
            "input",
            duration_ms(input_started.elapsed()),
        ));
        let rule_started = Instant::now();
        input.rule_manifest = collect_rule_manifest(&request.project_root, &mut diagnostics);
        producer_reports.push(ProjectAssemblyProducerReport::uncached(
            "rule",
            duration_ms(rule_started.elapsed()),
        ));
        let aui_started = Instant::now();
        let (aui_manifest, aui_documents) =
            collect_aui_documents(&request.project_root, &mut assets, &mut diagnostics);
        input.aui_manifest = aui_manifest;
        input.aui_documents = aui_documents;
        producer_reports.push(ProjectAssemblyProducerReport::uncached(
            "aui",
            duration_ms(aui_started.elapsed()),
        ));
        let artifact_cache = request.artifact_cache_root.as_ref().and_then(|root| {
            match ProjectAssemblyArtifactCache::open(root) {
                Ok(cache) => Some(cache),
                Err(error) => {
                    diagnostics.push(ProjectRuntimePackageAssemblyDiagnostic::warning(
                        ProjectRuntimePackageAssemblyDomain::Aui,
                        "AssemblyArtifactCacheUnavailable",
                        error.to_string(),
                    ));
                    None
                }
            }
        });
        if !input.aui_documents.is_empty() {
            let font_started = Instant::now();
            let font_documents = input.aui_documents.clone();
            match ProjectFontCookModule::discover_project_profiles(&request.project_root) {
                Ok(profiles) => {
                    let selection = select_font_profiles(&profiles);
                    if let Err(default_count) = selection {
                        diagnostics.push(ProjectRuntimePackageAssemblyDiagnostic::error(
                            ProjectRuntimePackageAssemblyDomain::Aui,
                            "FontDefaultUiStackInvalid",
                            format!(
                                "Expected at most one defaultUi font profile, found {}.",
                                default_count
                            ),
                        ).with_path("Assets").with_suggestion(
                            "Keep exactly one project defaultUi profile, or remove all project defaultUi profiles to use the engine built-in default.",
                        ));
                    } else if let Ok((default_profile, additional)) = selection {
                        if default_profile.is_none() {
                            match EngineBuiltInFontPack::load_embedded() {
                                Ok(bundle) => {
                                    let mut report = ProjectAssemblyProducerReport::uncached(
                                        "font-built-in-default",
                                        duration_ms(font_started.elapsed()),
                                    );
                                    report.producer_recipe_version =
                                        "engine-built-in-font-pack-manifest.v1".to_string();
                                    report.output_digest =
                                        Some(bundle.metadata.bundle_digest.clone());
                                    report.miss_reason = Some("embedded_sealed_pack".to_string());
                                    report.diagnostics = vec![
                                        "fontSelection=builtInDefault".to_string(),
                                        "fontRasterDurationMs=0".to_string(),
                                    ];
                                    producer_reports.push(report);
                                    input.font_bundles.push(bundle);
                                }
                                Err(error) => diagnostics.push(
                                    ProjectRuntimePackageAssemblyDiagnostic::error(
                                        ProjectRuntimePackageAssemblyDomain::Aui,
                                        error.code,
                                        error.message,
                                    )
                                    .with_path("engine://font-packs/aife-default-zh-cn-common-v1")
                                    .with_suggestion(
                                        "Rebuild the Editor with a valid sealed built-in FontPack.",
                                    ),
                                ),
                            }
                        } else {
                            append_project_font_profile(
                                &request.project_root,
                                &font_documents,
                                artifact_cache.as_ref(),
                                None,
                                &mut input,
                                &mut producer_reports,
                                &mut diagnostics,
                            );
                        }
                        for profile_id in additional {
                            append_project_font_profile(
                                &request.project_root,
                                &font_documents,
                                artifact_cache.as_ref(),
                                Some(&profile_id),
                                &mut input,
                                &mut producer_reports,
                                &mut diagnostics,
                            );
                        }
                    }
                }
                Err(failure) => append_font_diagnostics(failure, &mut diagnostics),
            }
        }
        let asset_started = Instant::now();
        collect_project_asset_files(&request.project_root, &mut assets, &mut diagnostics);
        let available_sprite_ids = assets.keys().cloned().collect::<BTreeSet<_>>();
        input.animator2d_registry = collect_animator2d_registry(
            &request.project_root,
            &available_sprite_ids,
            &mut diagnostics,
        );
        resolve_scene_animator2d_components(
            &mut input.scenes,
            &input.animator2d_registry,
            &mut diagnostics,
        );
        input.assets = assets.into_values().collect();
        producer_reports.push(ProjectAssemblyProducerReport::uncached(
            "asset",
            duration_ms(asset_started.elapsed()),
        ));
        let texture_started = Instant::now();
        input.texture_payloads =
            cook_texture_payloads(&request.project_root, &input.assets, &mut diagnostics);
        producer_reports.push(ProjectAssemblyProducerReport::uncached(
            "texture",
            duration_ms(texture_started.elapsed()),
        ));

        let has_errors = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ProjectRuntimePackageAssemblySeverity::Error);
        let status = if has_errors {
            ProjectRuntimePackageAssemblyStatus::Failed
        } else {
            ProjectRuntimePackageAssemblyStatus::Success
        };
        let report = report_for(
            &request.project_root,
            status,
            Some(scene.scene_id.clone()),
            Some(project.default_scene.clone()),
            &input,
            Some(prefab_bake_report),
            producer_reports,
            diagnostics,
        );
        ProjectRuntimePackageAssemblyResult {
            status,
            build_input: if status == ProjectRuntimePackageAssemblyStatus::Success {
                Some(input)
            } else {
                None
            },
            active_scene_id: Some(scene.scene_id),
            build_profile,
            report,
        }
    }
}

fn runtime_module_digest(project_root: &Path, project: &ProjectManifest) -> Result<String, String> {
    if project.runtime_module.module_id == EMPTY_PROJECT_RUNTIME_MODULE_ID {
        return Ok(EMPTY_PROJECT_RUNTIME_AOT_DIGEST.to_string());
    }
    let cargo_manifest_path = project_root.join(&project.runtime_module.cargo_manifest);
    let module_root = cargo_manifest_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Project runtime cargo manifest has no parent directory.".to_string())?;
    let mut paths = vec![cargo_manifest_path];
    collect_runtime_module_sources(&module_root.join("src"), &mut paths)?;
    let project_lock = module_root.join("Cargo.lock");
    if project_lock.is_file() {
        paths.push(project_lock);
    }
    paths.sort();
    paths.dedup();

    let mut source_bytes = Vec::<(String, Vec<u8>)>::new();
    for path in paths {
        let relative = path
            .strip_prefix(project_root)
            .map_err(|_| {
                format!(
                    "Project runtime source escaped project root: {}",
                    path.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "Failed to read project runtime source {}: {error}",
                path.display()
            )
        })?;
        source_bytes.push((relative, bytes));
    }
    project_runtime_aot_digest(
        &project.runtime_module.module_id,
        &project.runtime_module.interface_version,
        &project.runtime_module.cargo_manifest,
        &project.runtime_module.cargo_package,
        &project.runtime_module.player_binary,
        source_bytes
            .iter()
            .map(|(relative_path, bytes)| ProjectRuntimeAotDigestSource {
                relative_path,
                bytes,
            }),
    )
    .map_err(|error| error.to_string())
}

fn collect_project_observation_contract(
    project_root: &Path,
    project: &ProjectManifest,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Option<ProjectObservationContract> {
    let Some(reference) = project.observation_contract.as_deref() else {
        return None;
    };
    let relative = match ProjectRelativePath::parse(reference) {
        Ok(relative) => relative,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Project,
                    "project_observation.contract_path_invalid",
                    format!("Observation contract path is not project-relative: {error}"),
                )
                .with_path(reference)
                .with_stage("project-observation-contract-read")
                .with_suggestion(
                    "Set observationContract to a canonical project-relative JSON path.",
                ),
            );
            return None;
        }
    };
    let canonical_root = match fs::canonicalize(project_root) {
        Ok(root) => root,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Project,
                    "project_observation.project_root_unavailable",
                    format!("Failed to resolve project root: {error}"),
                )
                .with_path(project_root.display().to_string())
                .with_stage("project-observation-contract-read")
                .with_suggestion("Open an existing readable project root and retry assembly."),
            );
            return None;
        }
    };
    let source_path = canonical_root.join(relative.as_path());
    let canonical_source = match fs::canonicalize(&source_path) {
        Ok(path) if path.starts_with(&canonical_root) => path,
        Ok(_) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Project,
                    "project_observation.contract_path_outside_project",
                    "Observation contract path resolves outside the project root.",
                )
                .with_path(relative.as_str())
                .with_stage("project-observation-contract-read")
                .with_suggestion("Move the contract into the project and remove escaping links."),
            );
            return None;
        }
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Project,
                    "project_observation.contract_read_failed",
                    format!("Failed to resolve observation contract: {error}"),
                )
                .with_path(relative.as_str())
                .with_stage("project-observation-contract-read")
                .with_suggestion("Create a readable contract file at observationContract."),
            );
            return None;
        }
    };
    let text = match fs::read_to_string(&canonical_source) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Project,
                    "project_observation.contract_read_failed",
                    format!("Failed to read observation contract: {error}"),
                )
                .with_path(relative.as_str())
                .with_stage("project-observation-contract-read")
                .with_suggestion("Make the observation contract readable and retry assembly."),
            );
            return None;
        }
    };
    let contract = match serde_json::from_str::<ProjectObservationContract>(&text) {
        Ok(contract) => contract,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Project,
                    "project_observation.contract_parse_failed",
                    format!("Failed to parse observation contract JSON: {error}"),
                )
                .with_path(relative.as_str())
                .with_stage("project-observation-contract-validate")
                .with_suggestion("Fix the contract JSON to match project-observation-contract.v1."),
            );
            return None;
        }
    };
    if let Err(contract_diagnostics) = contract.validate() {
        diagnostics.extend(contract_diagnostics.into_iter().map(|diagnostic| {
            let detail = diagnostic
                .path
                .as_deref()
                .map(|path| format!("{} ({path})", relative.as_str()))
                .unwrap_or_else(|| relative.as_str().to_string());
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Project,
                diagnostic.code,
                diagnostic.message,
            )
            .with_path(detail)
            .with_stage("project-observation-contract-validate")
            .with_suggestion(diagnostic.next_action)
        }));
        return None;
    }
    Some(contract)
}

fn collect_runtime_module_sources(
    directory: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory).map_err(|error| {
        format!(
            "Failed to read project runtime source directory {}: {error}",
            directory.display()
        )
    })?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            collect_runtime_module_sources(&path, paths)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
    Ok(())
}

fn append_project_font_profile(
    project_root: &Path,
    documents: &[RuntimePackageSourceJson],
    cache: Option<&ProjectAssemblyArtifactCache>,
    profile_id: Option<&str>,
    input: &mut RuntimePackageBuildInput,
    producer_reports: &mut Vec<ProjectAssemblyProducerReport>,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) {
    let result = match profile_id {
        Some(profile_id) => {
            ProjectFontCookModule::cook_named_profile_for_runtime_package_with_cache(
                project_root,
                documents,
                cache,
                profile_id,
            )
        }
        None => ProjectFontCookModule::cook_for_runtime_package_with_cache(
            project_root,
            documents,
            cache,
        ),
    };
    match result {
        Ok((font_cook, mut producer_report)) => {
            if let Some(profile_id) = profile_id {
                producer_report.producer_id = format!("font-cook:{profile_id}");
            }
            producer_reports.push(producer_report);
            if let Some(atlas) = font_cook.legacy_atlas {
                input.font_atlases.push(atlas);
            }
            if let Some(bundle) = font_cook.font_bundle {
                input.font_bundles.push(bundle);
            }
        }
        Err(failure) => append_font_diagnostics(failure, diagnostics),
    }
}

fn select_font_profiles(
    profiles: &[crate::ProjectFontProfileInventoryEntry],
) -> Result<(Option<String>, Vec<String>), usize> {
    let defaults = profiles
        .iter()
        .filter(|profile| profile.role == FontAtlasProfileRole::DefaultUi)
        .map(|profile| profile.asset_id.clone())
        .collect::<Vec<_>>();
    if defaults.len() > 1 {
        return Err(defaults.len());
    }
    let additional = profiles
        .iter()
        .filter(|profile| profile.role == FontAtlasProfileRole::Additional)
        .map(|profile| profile.asset_id.clone())
        .collect::<Vec<_>>();
    Ok((defaults.into_iter().next(), additional))
}

fn append_font_diagnostics(
    failure: ProjectFontCookFailure,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) {
    diagnostics.extend(failure.diagnostics.into_iter().map(|diagnostic| {
        ProjectRuntimePackageAssemblyDiagnostic::error(
            ProjectRuntimePackageAssemblyDomain::Aui,
            diagnostic.code,
            diagnostic.message,
        )
        .with_path(diagnostic.source.unwrap_or_else(|| "font".to_string()))
        .with_suggestion(diagnostic.next_action)
    }));
}

fn failed_result(
    project_root: &Path,
    diagnostics: Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> ProjectRuntimePackageAssemblyResult {
    ProjectRuntimePackageAssemblyResult {
        status: ProjectRuntimePackageAssemblyStatus::Failed,
        build_input: None,
        active_scene_id: None,
        build_profile: None,
        report: ProjectRuntimePackageAssemblyReport {
            schema_version: PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION.to_string(),
            status: ProjectRuntimePackageAssemblyStatus::Failed,
            project_root: project_root.display().to_string(),
            active_scene_id: None,
            scene_count: 0,
            prefab_count: 0,
            asset_count: 0,
            rule_count: 0,
            input_mapping_count: 0,
            aui_document_count: 0,
            font_atlas_count: 0,
            font_bundle_count: 0,
            prefab_bake_report: None,
            source_mappings: Vec::new(),
            producer_reports: Vec::new(),
            diagnostics,
        },
    }
}

fn report_for(
    project_root: &Path,
    status: ProjectRuntimePackageAssemblyStatus,
    active_scene_id: Option<String>,
    active_scene_source_path: Option<String>,
    input: &RuntimePackageBuildInput,
    prefab_bake_report: Option<PrefabRuntimeBakeReport>,
    producer_reports: Vec<ProjectAssemblyProducerReport>,
    diagnostics: Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> ProjectRuntimePackageAssemblyReport {
    ProjectRuntimePackageAssemblyReport {
        schema_version: PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION.to_string(),
        status,
        project_root: project_root.display().to_string(),
        active_scene_id,
        scene_count: input.scenes.len(),
        prefab_count: input.prefabs.len(),
        asset_count: input.assets.len(),
        rule_count: input
            .rule_manifest
            .as_ref()
            .map(|manifest| manifest.rules.len())
            .unwrap_or_default(),
        input_mapping_count: input.input_mappings.len(),
        aui_document_count: input
            .aui_manifest
            .as_ref()
            .map(|manifest| manifest.documents.len())
            .unwrap_or_default(),
        font_atlas_count: input.font_atlases.len(),
        font_bundle_count: input.font_bundles.len(),
        prefab_bake_report,
        source_mappings: build_source_mappings(
            project_root,
            active_scene_source_path.as_deref(),
            input,
        ),
        producer_reports,
        diagnostics,
    }
}

fn build_source_mappings(
    project_root: &Path,
    active_scene_source_path: Option<&str>,
    input: &RuntimePackageBuildInput,
) -> Vec<ProjectRuntimeSourceMapping> {
    let project_object_id = read_project_manifest(project_root)
        .map(|manifest| manifest.project_id)
        .unwrap_or_else(|_| input.project.name.clone());
    let mut mappings = vec![ProjectRuntimeSourceMapping {
        domain: ProjectRuntimePackageAssemblyDomain::Project,
        source_path: "project.aife.json".to_string(),
        object_id: project_object_id,
        build_input_path: "project".to_string(),
        runtime_path: "manifest.json#project".to_string(),
    }];
    let build_profile_path = project_root.join("BuildProfiles").join("windows.dev.json");
    if build_profile_path.is_file() {
        mappings.push(ProjectRuntimeSourceMapping {
            domain: ProjectRuntimePackageAssemblyDomain::BuildProfile,
            source_path: "BuildProfiles/windows.dev.json".to_string(),
            object_id: "windows.dev".to_string(),
            build_input_path: "buildProfile".to_string(),
            runtime_path: "manifest.json#buildRecipe".to_string(),
        });
    }
    if let (Some(source_path), Some(scene)) = (active_scene_source_path, input.scenes.first()) {
        mappings.push(ProjectRuntimeSourceMapping {
            domain: ProjectRuntimePackageAssemblyDomain::Scene,
            source_path: source_path.replace('\\', "/"),
            object_id: scene.id.clone(),
            build_input_path: format!("scenes[{}]", scene.id),
            runtime_path: format!("scenes/{}.json", scene.id),
        });
    }
    for prefab in &input.prefabs {
        let source_path =
            find_json_source_path_by_id(project_root, "Prefabs", "prefabId", &prefab.prefab_id)
                .unwrap_or_else(|| format!("Prefabs/{}.prefab.json", prefab.prefab_id));
        mappings.push(ProjectRuntimeSourceMapping {
            domain: ProjectRuntimePackageAssemblyDomain::Prefab,
            source_path,
            object_id: prefab.prefab_id.clone(),
            build_input_path: format!("prefabs[{}]", prefab.prefab_id),
            runtime_path: format!("prefabs/{}.json", prefab.prefab_id),
        });
    }
    for mapping in &input.input_mappings {
        let source_path =
            find_json_source_path_by_id(project_root, "Input", "asset_id", &mapping.id)
                .or_else(|| {
                    find_json_source_path_by_id(project_root, "Input", "assetId", &mapping.id)
                })
                .unwrap_or_else(|| format!("Input/{}.json", mapping.id));
        mappings.push(ProjectRuntimeSourceMapping {
            domain: ProjectRuntimePackageAssemblyDomain::Input,
            source_path,
            object_id: mapping.id.clone(),
            build_input_path: format!("inputMappings[{}]", mapping.id),
            runtime_path: format!("input/{}.json", mapping.id),
        });
    }
    for document in &input.aui_documents {
        let source_path = AUI_SOURCE_ROOTS
            .iter()
            .find_map(|root| {
                find_json_source_path_by_id(project_root, root, "documentId", &document.id).or_else(
                    || find_json_source_path_by_id(project_root, root, "document_id", &document.id),
                )
            })
            .unwrap_or_else(|| format!("AUI/{}.aui.json", document.id));
        let runtime_path = input
            .aui_manifest
            .as_ref()
            .and_then(|manifest| {
                manifest
                    .documents
                    .iter()
                    .find(|entry| entry.document_id == document.id)
            })
            .map(|entry| entry.path.clone())
            .unwrap_or_else(|| format!("aui/documents/{}.aui.json", document.id));
        mappings.push(ProjectRuntimeSourceMapping {
            domain: ProjectRuntimePackageAssemblyDomain::Aui,
            source_path,
            object_id: document.id.clone(),
            build_input_path: format!("auiDocuments[{}]", document.id),
            runtime_path,
        });
    }
    if let Some(rule_manifest) = &input.rule_manifest {
        for rule in &rule_manifest.rules {
            let source_path =
                find_json_source_path_by_id(project_root, "Rules", "ruleId", &rule.rule_id)
                    .unwrap_or_else(|| "Rules/rule-manifest.json".to_string());
            mappings.push(ProjectRuntimeSourceMapping {
                domain: ProjectRuntimePackageAssemblyDomain::Rule,
                source_path,
                object_id: rule.rule_id.clone(),
                build_input_path: format!("ruleManifest.rules[{}]", rule.rule_id),
                runtime_path: format!("rules/rule-manifest.json#{}", rule.rule_id),
            });
        }
    }
    for asset in &input.assets {
        let runtime_path = input
            .texture_payloads
            .iter()
            .find(|texture| texture.metadata.asset_id == asset.asset_id)
            .map(|texture| format!("cooked/textures/{}.texture.json", texture.metadata.asset_id))
            .unwrap_or_else(|| asset.runtime_uri.clone());
        mappings.push(ProjectRuntimeSourceMapping {
            domain: ProjectRuntimePackageAssemblyDomain::Asset,
            source_path: asset.source.replace('\\', "/"),
            object_id: asset.asset_id.clone(),
            build_input_path: format!("assets[{}]", asset.asset_id),
            runtime_path,
        });
    }
    mappings.sort_by(|left, right| {
        format!("{:?}:{}", left.domain, left.object_id)
            .cmp(&format!("{:?}:{}", right.domain, right.object_id))
    });
    mappings
}

fn find_json_source_path_by_id(
    project_root: &Path,
    directory: &str,
    id_field: &str,
    object_id: &str,
) -> Option<String> {
    let root = project_root.join(directory);
    let entries = fs::read_dir(&root).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let value = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
        if value
            .as_ref()
            .and_then(|value| value.get(id_field))
            .and_then(serde_json::Value::as_str)
            == Some(object_id)
        {
            return path
                .strip_prefix(project_root)
                .ok()
                .map(|relative| relative.to_string_lossy().replace('\\', "/"));
        }
    }
    None
}

fn read_project_manifest(
    project_root: &Path,
) -> Result<ProjectManifest, ProjectRuntimePackageAssemblyDiagnostic> {
    let manifest_path = project_root.join("project.aife.json");
    let text = fs::read_to_string(&manifest_path).map_err(|error| {
        ProjectRuntimePackageAssemblyDiagnostic::error(
            ProjectRuntimePackageAssemblyDomain::Project,
            "ProjectManifestReadFailed",
            format!("Failed to read project manifest: {error}"),
        )
        .with_path(manifest_path.display().to_string())
    })?;
    let manifest = serde_json::from_str::<ProjectManifest>(&text).map_err(|error| {
        ProjectRuntimePackageAssemblyDiagnostic::error(
            ProjectRuntimePackageAssemblyDomain::Project,
            "ProjectManifestParseFailed",
            format!("Failed to parse project manifest: {error}"),
        )
        .with_path(manifest_path.display().to_string())
    })?;
    if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
        return Err(ProjectRuntimePackageAssemblyDiagnostic::error(
            ProjectRuntimePackageAssemblyDomain::Project,
            "UnsupportedProjectManifestSchema",
            format!(
                "Project manifest schema must be {}, got {}.",
                PROJECT_MANIFEST_SCHEMA_VERSION, manifest.schema_version
            ),
        )
        .with_path(manifest_path.display().to_string()));
    }
    manifest.runtime_module.validate().map_err(|message| {
        ProjectRuntimePackageAssemblyDiagnostic::error(
            ProjectRuntimePackageAssemblyDomain::Project,
            "InvalidProjectRuntimeModuleBuildSpec",
            message,
        )
        .with_path(manifest_path.display().to_string())
    })?;
    Ok(manifest)
}

fn read_build_profile(
    request: &ProjectRuntimePackageAssemblyRequest,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Option<BuildProfile> {
    let path = request.build_profile_path.clone().unwrap_or_else(|| {
        request
            .project_root
            .join("BuildProfiles")
            .join("windows.dev.json")
    });
    if !path.exists() {
        return None;
    }
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::BuildProfile,
                    "BuildProfileReadFailed",
                    format!("Failed to read build profile: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            return None;
        }
    };
    let profile = match serde_json::from_str::<BuildProfile>(&text) {
        Ok(profile) => profile,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::BuildProfile,
                    "BuildProfileParseFailed",
                    format!("Failed to parse build profile: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            return None;
        }
    };
    for issue in profile.validation_issues() {
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::BuildProfile,
                issue.code,
                format!("{}: {}", issue.field, issue.message),
            )
            .with_path(path.display().to_string())
            .with_suggestion(issue.next_action),
        );
    }
    Some(profile)
}

fn collect_input_mappings(
    project_root: &Path,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Vec<RuntimePackageSourceJson> {
    scan_input_mapping_paths(project_root)
        .into_iter()
        .filter_map(|relative_path| {
            let mapping = match InputMappingAuthoringService::load(project_root, &relative_path) {
                Ok(mapping) => mapping,
                Err(message) => {
                    diagnostics.push(
                        ProjectRuntimePackageAssemblyDiagnostic::error(
                            ProjectRuntimePackageAssemblyDomain::Input,
                            "InputMappingLoadFailed",
                            message,
                        )
                        .with_path(relative_path),
                    );
                    return None;
                }
            };
            let validation = mapping.validate();
            for diagnostic in &validation.diagnostics {
                let mut assembly_diagnostic = match diagnostic.severity {
                    InputDiagnosticSeverity::Warning => {
                        ProjectRuntimePackageAssemblyDiagnostic::warning(
                            ProjectRuntimePackageAssemblyDomain::Input,
                            "InputMappingValidationWarning",
                            diagnostic.message.clone(),
                        )
                    }
                    InputDiagnosticSeverity::Error => {
                        ProjectRuntimePackageAssemblyDiagnostic::error(
                            ProjectRuntimePackageAssemblyDomain::Input,
                            "InputMappingValidationError",
                            diagnostic.message.clone(),
                        )
                    }
                }
                .with_path(relative_path.clone());
                assembly_diagnostic.suggestion =
                    Some("Fix the project InputMappingAsset before export.".to_string());
                diagnostics.push(assembly_diagnostic);
            }
            if validation.has_errors() {
                return None;
            }
            Some(RuntimePackageSourceJson {
                id: mapping.asset_id.clone(),
                document: serde_json::to_value(mapping)
                    .expect("InputMappingAsset should serialize to JSON value"),
            })
        })
        .collect()
}

fn collect_prefabs(
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Vec<RuntimePackageSourcePrefab> {
    collect_json_files(&project_root.join("Prefabs"))
        .into_iter()
        .filter_map(|path| {
            let document = match read_json_value(&path) {
                Ok(document) => document,
                Err(message) => {
                    diagnostics.push(
                        ProjectRuntimePackageAssemblyDiagnostic::error(
                            ProjectRuntimePackageAssemblyDomain::Prefab,
                            "PrefabReadFailed",
                            message,
                        )
                        .with_path(path.display().to_string()),
                    );
                    return None;
                }
            };
            collect_asset_refs_from_value(&document, assets, project_root);
            let prefab_id = document
                .get("prefabId")
                .or_else(|| document.get("id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("prefab")
                        .to_string()
                });
            Some(RuntimePackageSourcePrefab {
                prefab_id,
                document,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct PrefabBakeCatalog {
    runtime_source_prefabs: Vec<RuntimePackageSourcePrefab>,
    authoring_prefab_by_id: BTreeMap<String, PrefabAsset>,
}

fn build_prefab_bake_catalog(
    _project_root: &Path,
    runtime_source_prefabs: Vec<RuntimePackageSourcePrefab>,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> PrefabBakeCatalog {
    let mut authoring_prefab_by_id = BTreeMap::new();
    for source in &runtime_source_prefabs {
        let schema_version = source
            .document
            .get("schemaVersion")
            .or_else(|| source.document.get("schema_version"))
            .and_then(serde_json::Value::as_str);
        if schema_version != Some(PREFAB_ASSET_SCHEMA_VERSION) {
            continue;
        }

        let asset = match serde_json::from_value::<PrefabAsset>(source.document.clone()) {
            Ok(asset) => asset,
            Err(error) => {
                diagnostics.push(
                    ProjectRuntimePackageAssemblyDiagnostic::error(
                        ProjectRuntimePackageAssemblyDomain::Prefab,
                        "AuthoringPrefabAssetParseFailed",
                        format!(
                            "Failed to parse authoring PrefabAsset {} for runtime bake: {error}",
                            source.prefab_id
                        ),
                    )
                    .with_suggestion("Fix Prefabs/*.prefab.json authoring-prefab-asset.v1 shape."),
                );
                continue;
            }
        };
        if asset.prefab_id != source.prefab_id {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Prefab,
                    "PrefabIdMismatch",
                    format!(
                        "Prefab source id {} does not match authoring PrefabAsset id {}.",
                        source.prefab_id, asset.prefab_id
                    ),
                )
                .with_suggestion(
                    "Keep RuntimePackageSourcePrefab.prefab_id equal to PrefabAsset.prefabId.",
                ),
            );
            continue;
        }
        authoring_prefab_by_id.insert(asset.prefab_id.clone(), asset);
    }

    PrefabBakeCatalog {
        runtime_source_prefabs,
        authoring_prefab_by_id,
    }
}

fn collect_rule_manifest(
    project_root: &Path,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Option<RuntimeRuleManifest> {
    let path = project_root.join("Rules").join("rule-manifest.json");
    if !path.exists() {
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::warning(
                ProjectRuntimePackageAssemblyDomain::Rule,
                "RuleManifestMissing",
                "No runtime rule manifest exists; project logic will not run.",
            )
            .with_path(path.display().to_string()),
        );
        return None;
    }
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Rule,
                    "RuleManifestReadFailed",
                    format!("Failed to read rule manifest: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            return None;
        }
    };
    let manifest = match serde_json::from_str::<RuntimeRuleManifest>(&text) {
        Ok(manifest) => manifest,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Rule,
                    "RuleManifestParseFailed",
                    format!("Failed to parse rule manifest: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            return None;
        }
    };
    let artifact_report = validate_runtime_rule_manifest_artifacts(None, &manifest);
    for diagnostic in artifact_report.issues {
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Rule,
                diagnostic.code,
                diagnostic.message,
            )
            .with_path(diagnostic.path),
        );
    }
    Some(manifest)
}

fn collect_aui_documents(
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> (Option<RuntimeAuiManifest>, Vec<RuntimePackageSourceJson>) {
    let mut runtime_documents = Vec::new();
    let mut source_paths = AUI_SOURCE_ROOTS
        .iter()
        .flat_map(|root| collect_json_files(&project_root.join(root)))
        .collect::<Vec<_>>();
    source_paths.sort_by_key(|path| {
        path.strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    });
    source_paths.dedup();
    let documents = source_paths
        .into_iter()
        .filter_map(|path| {
            let document = match read_json_value(&path) {
                Ok(document) => document,
                Err(message) => {
                    diagnostics.push(
                        ProjectRuntimePackageAssemblyDiagnostic::error(
                            ProjectRuntimePackageAssemblyDomain::Aui,
                            "AuiDocumentReadFailed",
                            message,
                        )
                        .with_path(path.display().to_string()),
                    );
                    return None;
                }
            };
            collect_asset_refs_from_value(&document, assets, project_root);
            let cook = match AuiDocumentCooker::cook(AuiDocumentCookRequest {
                source_path: path.clone(),
                document,
            }) {
                Ok(cook) => cook,
                Err(report) => {
                    for cook_diagnostic in report.diagnostics {
                        diagnostics.push(
                            ProjectRuntimePackageAssemblyDiagnostic::error(
                                ProjectRuntimePackageAssemblyDomain::Aui,
                                cook_diagnostic.code,
                                cook_diagnostic.message,
                            )
                            .with_path(path.display().to_string()),
                        );
                    }
                    return None;
                }
            };
            for cook_diagnostic in &cook.report.diagnostics {
                let mut diagnostic = match cook_diagnostic.severity {
                    crate::AuiDocumentCookDiagnosticSeverity::Warning => {
                        ProjectRuntimePackageAssemblyDiagnostic::warning(
                            ProjectRuntimePackageAssemblyDomain::Aui,
                            cook_diagnostic.code.clone(),
                            cook_diagnostic.message.clone(),
                        )
                    }
                    crate::AuiDocumentCookDiagnosticSeverity::Error => {
                        ProjectRuntimePackageAssemblyDiagnostic::error(
                            ProjectRuntimePackageAssemblyDomain::Aui,
                            cook_diagnostic.code.clone(),
                            cook_diagnostic.message.clone(),
                        )
                    }
                };
                diagnostic = diagnostic.with_path(path.display().to_string());
                if let Some(suggestion) = &cook_diagnostic.suggestion {
                    diagnostic = diagnostic.with_suggestion(suggestion.clone());
                }
                diagnostics.push(diagnostic);
            }
            let document_value = serde_json::to_value(&cook.document)
                .unwrap_or_else(|_| serde_json::json!({ "schemaVersion": "aui-document.v1" }));
            runtime_documents.push(RuntimePackageSourceJson {
                id: cook.document.document_id.clone(),
                document: document_value,
            });
            Some(RuntimeAuiManifestEntry {
                document_id: cook.document.document_id.clone(),
                path: cook.package_path,
                canvas_count: cook.document.canvases.len(),
                node_count: cook.document.nodes.len(),
                binding_count: cook
                    .document
                    .nodes
                    .iter()
                    .map(|node| node.binding_refs.len())
                    .sum(),
                action_count: cook
                    .document
                    .nodes
                    .iter()
                    .map(|node| node.action_refs.len())
                    .sum(),
                asset_refs: collect_asset_ref_ids(&serde_json::to_value(&cook.document).ok()?)
                    .into_iter()
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    if documents.is_empty() {
        (None, runtime_documents)
    } else {
        (
            Some(RuntimeAuiManifest {
                schema_version: RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.to_string(),
                documents,
            }),
            runtime_documents,
        )
    }
}

fn register_runtime_scene_asset(
    scene: &EditorSceneDocument,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
) {
    assets.entry(scene.scene_id.clone()).or_insert_with(|| {
        RuntimePackageSourceAsset::new(
            scene.scene_id.clone(),
            scene.name.clone(),
            "scene",
            format!("Scenes/{}.scene.json", scene.name),
            format!("scenes/{}.json", scene.scene_id),
        )
    });
}

fn editor_scene_to_runtime(
    scene: &EditorSceneDocument,
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
    catalog: &PrefabBakeCatalog,
) -> (RuntimeScene, PrefabRuntimeBakeReport) {
    let mut prefab_bake_report =
        PrefabRuntimeBakeReport::new(project_root, scene, catalog.authoring_prefab_by_id.len());
    let mut runtime_entities = Vec::new();
    for entity in &scene.entities {
        if has_prefab_instance_component(entity) {
            prefab_bake_report.scene_prefab_instance_count += 1;
            runtime_entities.extend(bake_prefab_instance_entity(
                entity,
                project_root,
                assets,
                catalog,
                &mut prefab_bake_report,
            ));
        } else {
            runtime_entities.push(editor_entity_to_runtime(entity, project_root, assets));
        }
    }
    prefab_bake_report.finish();
    (
        RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.to_string(),
            id: scene.scene_id.clone(),
            name: scene.name.clone(),
            gravity: scene.gravity,
            background: scene.background.clone(),
            sky_color: scene.sky_color.clone(),
            entities: runtime_entities,
        },
        prefab_bake_report,
    )
}

fn has_prefab_instance_component(entity: &EditorSceneEntity) -> bool {
    entity
        .components
        .iter()
        .any(|component| component.component_type == PREFAB_INSTANCE_COMPONENT_TYPE)
}

fn bake_prefab_instance_entity(
    entity: &EditorSceneEntity,
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
    catalog: &PrefabBakeCatalog,
    report: &mut PrefabRuntimeBakeReport,
) -> Vec<RuntimeEntity> {
    let instance = match PrefabInstance::from_scene_entity(entity) {
        Ok(instance) => instance,
        Err(diagnostic) => {
            report
                .diagnostics
                .push(prefab_diagnostic_to_assembly_diagnostic(diagnostic));
            return Vec::new();
        }
    };

    let mut entry = PrefabRuntimeBakeInstanceEntry {
        scene_entity_id: entity.entity_id.clone(),
        instance_id: instance.instance_id.clone(),
        prefab_id: instance.prefab_ref.id.clone(),
        root_source_entity_id: String::new(),
        root_runtime_entity_id: entity.entity_id.clone(),
        emitted_entity_ids: Vec::new(),
        applied_override_count: 0,
        ignored_authoring_component_types: vec![PREFAB_INSTANCE_COMPONENT_TYPE.to_string()],
        local_runtime_component_warnings: Vec::new(),
        diagnostics: Vec::new(),
    };

    for component in entity
        .components
        .iter()
        .filter(|component| component.component_type != PREFAB_INSTANCE_COMPONENT_TYPE)
    {
        let diagnostic = ProjectRuntimePackageAssemblyDiagnostic::warning(
            ProjectRuntimePackageAssemblyDomain::Prefab,
            "prefab_instance_local_runtime_component_shadowed",
            format!(
                "PrefabInstance {} has local runtime component {} that is ignored by C-min bake.",
                entity.entity_id, component.component_type
            ),
        )
        .with_suggestion(
            "Move runtime components into the PrefabAsset or express changes as PrefabOverride.",
        );
        entry
            .local_runtime_component_warnings
            .push(component.component_type.clone());
        push_prefab_bake_diagnostic(report, &mut entry, diagnostic);
    }

    let Some(asset) = catalog.authoring_prefab_by_id.get(&instance.prefab_ref.id) else {
        let code = if catalog
            .runtime_source_prefabs
            .iter()
            .any(|prefab| prefab.prefab_id == instance.prefab_ref.id)
        {
            "scene_prefab_instance_requires_authoring_prefab_asset"
        } else {
            "missing_prefab_asset"
        };
        let diagnostic = ProjectRuntimePackageAssemblyDiagnostic::error(
            ProjectRuntimePackageAssemblyDomain::Prefab,
            code,
            format!(
                "Scene PrefabInstance {} references {}, but no authoring PrefabAsset is available for bake.",
                entity.entity_id, instance.prefab_ref.id
            ),
        )
        .with_suggestion(
            "Keep scene-placed PrefabInstance backed by Prefabs/*.prefab.json authoring-prefab-asset.v1.",
        );
        push_prefab_bake_diagnostic(report, &mut entry, diagnostic);
        report.instances.push(entry);
        return Vec::new();
    };
    entry.root_source_entity_id = asset.root_entity_id.clone();

    for prefab_entity in &asset.entities {
        if prefab_entity
            .source_entity_id
            .contains(PREFAB_RUNTIME_ENTITY_ID_SEPARATOR)
        {
            let diagnostic = ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Prefab,
                "prefab_source_entity_id_contains_reserved_separator",
                format!(
                    "Prefab source entity id {} contains reserved separator {}.",
                    prefab_entity.source_entity_id, PREFAB_RUNTIME_ENTITY_ID_SEPARATOR
                ),
            )
            .with_suggestion(
                "Rename prefab source entity id or use a later escaped-id remap scheme.",
            );
            push_prefab_bake_diagnostic(report, &mut entry, diagnostic);
            report.instances.push(entry);
            return Vec::new();
        }
    }

    let resolved = ResolvedPrefabView::resolve(asset, &instance);
    for diagnostic in &resolved.diagnostics {
        push_prefab_bake_diagnostic(
            report,
            &mut entry,
            prefab_diagnostic_to_assembly_diagnostic(diagnostic.clone()),
        );
    }
    if entry
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ProjectRuntimePackageAssemblySeverity::Error)
    {
        report.instances.push(entry);
        return Vec::new();
    }

    let Some(root_entity) = resolved
        .resolved_entities
        .iter()
        .find(|resolved_entity| resolved_entity.source_entity_id == asset.root_entity_id)
    else {
        let diagnostic = ProjectRuntimePackageAssemblyDiagnostic::error(
            ProjectRuntimePackageAssemblyDomain::Prefab,
            "missing_source_entity",
            format!(
                "Prefab {} root source entity {} is missing.",
                asset.prefab_id, asset.root_entity_id
            ),
        )
        .with_suggestion("Fix PrefabAsset.rootEntityId or entities[].sourceEntityId.");
        push_prefab_bake_diagnostic(report, &mut entry, diagnostic);
        report.instances.push(entry);
        return Vec::new();
    };
    if resolved.applied_overrides.iter().any(|override_value| {
        override_value.target_source_entity_id == asset.root_entity_id
            && override_value.component_type == "engine.transform"
    }) {
        let diagnostic = ProjectRuntimePackageAssemblyDiagnostic::warning(
            ProjectRuntimePackageAssemblyDomain::Prefab,
            "root_transform_override_shadowed_by_scene_placement",
            format!(
                "PrefabInstance {} root transform override is shadowed by Scene placement.",
                entity.entity_id
            ),
        )
        .with_suggestion(
            "Put root placement on the Scene entity and internal offsets on child transform overrides.",
        );
        push_prefab_bake_diagnostic(report, &mut entry, diagnostic);
    }

    let source_to_runtime = build_prefab_source_to_runtime_map(
        entity,
        &asset.root_entity_id,
        &resolved.resolved_entities,
    );
    for resolved_entity in &resolved.resolved_entities {
        if resolved_entity.source_entity_id != asset.root_entity_id {
            if let Some(parent_source_id) = &resolved_entity.parent_source_entity_id {
                if !source_to_runtime.contains_key(parent_source_id) {
                    let diagnostic = ProjectRuntimePackageAssemblyDiagnostic::error(
                        ProjectRuntimePackageAssemblyDomain::Prefab,
                        "missing_source_entity",
                        format!(
                            "Prefab child {} references missing parent source entity {}.",
                            resolved_entity.source_entity_id, parent_source_id
                        ),
                    )
                    .with_suggestion("Fix PrefabAsset parentSourceEntityId.");
                    push_prefab_bake_diagnostic(report, &mut entry, diagnostic);
                    report.instances.push(entry);
                    return Vec::new();
                }
            }
        }
    }

    let mut emitted = Vec::new();
    for resolved_entity in &resolved.resolved_entities {
        let runtime_entity = resolved_prefab_entity_to_runtime(
            entity,
            root_entity,
            resolved_entity,
            &asset.root_entity_id,
            &source_to_runtime,
            project_root,
            assets,
        );
        entry.emitted_entity_ids.push(runtime_entity.id.clone());
        emitted.push(runtime_entity);
    }
    entry.applied_override_count = resolved.applied_overrides.len();
    report.baked_instance_count += 1;
    report.baked_entity_count += emitted.len();
    report.instances.push(entry);
    emitted
}

fn build_prefab_source_to_runtime_map(
    scene_entity: &EditorSceneEntity,
    root_source_entity_id: &str,
    resolved_entities: &[ResolvedPrefabEntity],
) -> BTreeMap<String, String> {
    resolved_entities
        .iter()
        .map(|resolved_entity| {
            let runtime_id = if resolved_entity.source_entity_id == root_source_entity_id {
                scene_entity.entity_id.clone()
            } else {
                format!(
                    "{}{}{}",
                    scene_entity.entity_id,
                    PREFAB_RUNTIME_ENTITY_ID_SEPARATOR,
                    resolved_entity.source_entity_id
                )
            };
            (resolved_entity.source_entity_id.clone(), runtime_id)
        })
        .collect()
}

fn resolved_prefab_entity_to_runtime(
    scene_entity: &EditorSceneEntity,
    root_entity: &ResolvedPrefabEntity,
    resolved_entity: &ResolvedPrefabEntity,
    root_source_entity_id: &str,
    source_to_runtime: &BTreeMap<String, String>,
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
) -> RuntimeEntity {
    let is_root = resolved_entity.source_entity_id == root_source_entity_id;
    let (sprite_renderer2d, components) =
        split_sprite_component(&resolved_entity.components, project_root, assets);
    RuntimeEntity {
        schema_version: RUNTIME_ENTITY_SCHEMA_VERSION.to_string(),
        id: source_to_runtime
            .get(&resolved_entity.source_entity_id)
            .cloned()
            .unwrap_or_else(|| resolved_entity.source_entity_id.clone()),
        name: if is_root {
            scene_entity.name.clone()
        } else {
            resolved_entity.name.clone()
        },
        kind: if is_root {
            scene_entity.kind.clone()
        } else {
            "prefab_entity".to_string()
        },
        enabled: if is_root {
            scene_entity.enabled && root_entity.enabled
        } else {
            resolved_entity.enabled
        },
        parent_id: if is_root {
            scene_entity.parent_id.clone()
        } else {
            resolved_entity
                .parent_source_entity_id
                .as_ref()
                .and_then(|source_id| source_to_runtime.get(source_id).cloned())
        },
        sibling_order: if is_root {
            scene_entity.sibling_order
        } else {
            resolved_entity.sibling_order
        },
        transform: Some(if is_root {
            editor_transform_to_runtime(
                scene_entity
                    .transform
                    .unwrap_or_else(EditorTransform::identity),
            )
        } else {
            editor_transform_to_runtime(resolved_entity.transform)
        }),
        mesh: None,
        sprite_renderer2d,
        animator2d: None,
        components,
    }
}

fn push_prefab_bake_diagnostic(
    report: &mut PrefabRuntimeBakeReport,
    entry: &mut PrefabRuntimeBakeInstanceEntry,
    diagnostic: ProjectRuntimePackageAssemblyDiagnostic,
) {
    entry.diagnostics.push(diagnostic.clone());
    report.diagnostics.push(diagnostic);
}

fn prefab_diagnostic_to_assembly_diagnostic(
    diagnostic: PrefabDiagnostic,
) -> ProjectRuntimePackageAssemblyDiagnostic {
    ProjectRuntimePackageAssemblyDiagnostic {
        severity: match diagnostic.severity {
            PrefabDiagnosticSeverity::Info => ProjectRuntimePackageAssemblySeverity::Info,
            PrefabDiagnosticSeverity::Warning => ProjectRuntimePackageAssemblySeverity::Warning,
            PrefabDiagnosticSeverity::Error => ProjectRuntimePackageAssemblySeverity::Error,
        },
        domain: ProjectRuntimePackageAssemblyDomain::Prefab,
        code: diagnostic.code.as_str().to_string(),
        message: diagnostic.message,
        path: diagnostic
            .source_entity_id
            .or(diagnostic.prefab_ref)
            .or(diagnostic.instance_id),
        stage: None,
        suggestion: diagnostic.field_path,
    }
}

fn editor_entity_to_runtime(
    entity: &EditorSceneEntity,
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
) -> RuntimeEntity {
    let (sprite_renderer2d, components) =
        split_sprite_component(&entity.components, project_root, assets);
    RuntimeEntity {
        schema_version: RUNTIME_ENTITY_SCHEMA_VERSION.to_string(),
        id: entity.entity_id.clone(),
        name: entity.name.clone(),
        kind: entity.kind.clone(),
        enabled: entity.enabled,
        parent_id: entity.parent_id.clone(),
        sibling_order: entity.sibling_order,
        transform: entity.transform.map(editor_transform_to_runtime),
        mesh: entity
            .mesh
            .as_ref()
            .map(|mesh| editor_mesh_to_runtime(mesh, project_root, assets)),
        sprite_renderer2d,
        animator2d: None,
        components,
    }
}

fn editor_transform_to_runtime(transform: EditorTransform) -> RuntimeTransform {
    RuntimeTransform {
        local_position: vec3(transform.local_position),
        local_rotation: vec3(transform.local_rotation),
        local_scale: vec3(transform.local_scale),
    }
}

fn editor_mesh_to_runtime(
    mesh: &EditorMesh,
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
) -> RuntimeMesh {
    let asset_ref = mesh
        .asset_ref
        .as_ref()
        .map(|asset| editor_asset_ref_to_runtime(asset, project_root, assets));
    let material_ref = mesh
        .material_ref
        .as_ref()
        .map(|asset| editor_asset_ref_to_runtime(asset, project_root, assets));
    RuntimeMesh {
        primitive: mesh.primitive.clone(),
        color: None,
        label: None,
        asset_ref,
        material_ref,
        texture_ref: None,
        visible: mesh.visible,
        layer: mesh.layer.clone(),
        metalness: None,
        roughness: None,
    }
}

fn split_sprite_component(
    components: &[EditorSceneComponent],
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
) -> (
    Option<RuntimeSpriteRenderer2D>,
    Vec<RuntimeProjectComponent>,
) {
    let mut sprite = None;
    let mut rest = Vec::new();
    for component in components {
        collect_asset_refs_from_value(&component.fields, assets, project_root);
        if component.component_type == "SpriteRenderer2D" {
            sprite = Some(sprite_component_to_runtime(component, project_root, assets));
        } else {
            rest.push(RuntimeProjectComponent {
                component_type: component.component_type.clone(),
                data: component.fields.clone(),
            });
        }
    }
    (sprite, rest)
}

fn sprite_component_to_runtime(
    component: &EditorSceneComponent,
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
) -> RuntimeSpriteRenderer2D {
    let sprite_ref = component
        .fields
        .get("spriteRef")
        .or_else(|| component.fields.get("sprite_ref"))
        .and_then(parse_asset_ref_value)
        .map(|asset| editor_asset_ref_to_runtime(&asset, project_root, assets));
    RuntimeSpriteRenderer2D {
        sprite_ref,
        material_ref: None,
        color: None,
        flip_x: component
            .fields
            .get("flipX")
            .and_then(serde_json::Value::as_bool),
        flip_y: component
            .fields
            .get("flipY")
            .and_then(serde_json::Value::as_bool),
        sorting_layer: component
            .fields
            .get("sortingLayer")
            .and_then(serde_json::Value::as_i64)
            .map(|value| value as i16),
        order_in_layer: component
            .fields
            .get("orderInLayer")
            .and_then(serde_json::Value::as_i64)
            .map(|value| value as i32),
        sort_z: component
            .fields
            .get("sortZ")
            .and_then(serde_json::Value::as_f64)
            .map(|value| value as f32),
        visible: component
            .fields
            .get("visible")
            .and_then(serde_json::Value::as_bool),
    }
}

fn parse_asset_ref_value(value: &serde_json::Value) -> Option<EditorAssetRef> {
    serde_json::from_value(value.clone()).ok()
}

fn collect_asset_refs_from_value(
    value: &serde_json::Value,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
    project_root: &Path,
) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(asset_ref) = parse_asset_ref_value(value) {
                let _ = editor_asset_ref_to_runtime(&asset_ref, project_root, assets);
            }
            for value in map.values() {
                collect_asset_refs_from_value(value, assets, project_root);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_asset_refs_from_value(value, assets, project_root);
            }
        }
        _ => {}
    }
}

fn collect_project_asset_files(
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) {
    let registered_ids = match collect_registered_project_assets(project_root, assets, diagnostics)
    {
        Ok(ids) => ids,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "AssetDatabaseInvalid",
                    error.to_string(),
                )
                .with_path("Library/AssetPipeline/asset-database.json")
                .with_suggestion(error.next_action),
            );
            BTreeSet::new()
        }
    };
    let mut paths = Vec::new();
    collect_legacy_asset_files(&project_root.join("Assets"), &mut paths);
    paths.sort();
    for path in paths {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        if !extension.eq_ignore_ascii_case("asset") && !extension.eq_ignore_ascii_case("png") {
            continue;
        }
        let Some(asset_id) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if registered_ids.contains(&asset_id) {
            continue;
        }
        let relative_source = path
            .strip_prefix(project_root)
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| format!("Assets/{asset_id}.{extension}"));
        let asset_type = if extension.eq_ignore_ascii_case("png") {
            "texture".to_string()
        } else {
            descriptor_asset_type(project_root, &relative_source, diagnostics)
        };
        let runtime_payload = (asset_type != "texture")
            .then(|| fs::read(&path).ok())
            .flatten();
        let asset = assets.entry(asset_id.clone()).or_insert_with(|| {
            RuntimePackageSourceAsset::new(
                asset_id.clone(),
                asset_id.clone(),
                asset_type.clone(),
                relative_source.clone(),
                format!("cooked/{asset_id}.asset"),
            )
        });
        if asset.asset_type != "texture" && asset.runtime_payload.is_none() {
            asset.runtime_payload = runtime_payload;
        }
    }
}

fn collect_registered_project_assets(
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Result<BTreeSet<String>, crate::ProjectAssetImportError> {
    let Some(database) = ProjectAssetImport::load_database(project_root)? else {
        return Ok(BTreeSet::new());
    };
    let mut ids = BTreeSet::new();
    for record in database.assets {
        ids.insert(record.asset_id.clone());
        let required_paths = [
            record.descriptor_path.as_str(),
            record.source_path.as_str(),
            record.meta_path.as_str(),
        ];
        if let Some(missing) = required_paths
            .iter()
            .find(|path| !project_root.join(path).is_file())
        {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "RegisteredAssetFileMissing",
                    format!(
                        "Registered asset {} is missing a required file.",
                        record.asset_id
                    ),
                )
                .with_path((*missing).to_string())
                .with_suggestion("Repair or explicitly reimport the registered asset."),
            );
            continue;
        }
        let source_bytes = match fs::read(project_root.join(&record.source_path)) {
            Ok(bytes) => bytes,
            Err(error) => {
                diagnostics.push(
                    ProjectRuntimePackageAssemblyDiagnostic::error(
                        ProjectRuntimePackageAssemblyDomain::Asset,
                        "RegisteredAssetSourceReadFailed",
                        format!(
                            "Registered asset {} source cannot be read: {error}",
                            record.asset_id
                        ),
                    )
                    .with_path(record.source_path.clone()),
                );
                continue;
            }
        };
        if sha256_prefixed(&source_bytes) != record.source_hash {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "RegisteredAssetSourceHashMismatch",
                    format!(
                        "Registered asset {} source hash does not match AssetDB.",
                        record.asset_id
                    ),
                )
                .with_path(record.source_path.clone())
                .with_suggestion("Reimport the source through ProjectAssetImport."),
            );
            continue;
        }
        if record.asset_type == crate::FONT_SOURCE_ASSET_TYPE {
            continue;
        }
        let mut asset = RuntimePackageSourceAsset::new(
            record.asset_id.clone(),
            record.display_name,
            record.asset_type,
            record.descriptor_path,
            format!("cooked/{}.asset", record.asset_id),
        );
        asset.asset_guid = Some(record.asset_guid);
        asset.hash = Some(record.source_hash);
        asset.dependencies = record.direct_dependencies;
        assets.insert(record.asset_id, asset);
    }
    Ok(ids)
}

fn collect_legacy_asset_files(directory: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(directory) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_legacy_asset_files(&path, paths);
        } else if metadata.is_file() {
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default();
            if extension.eq_ignore_ascii_case("asset") || extension.eq_ignore_ascii_case("png") {
                paths.push(path);
            }
        }
    }
}

fn collect_asset_ref_ids(value: &serde_json::Value) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    collect_asset_ref_ids_recursive(value, &mut ids);
    ids
}

fn collect_asset_ref_ids_recursive(value: &serde_json::Value, ids: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(id) = map.get("id").and_then(serde_json::Value::as_str) {
                if map
                    .get("type")
                    .or_else(|| map.get("assetType"))
                    .and_then(serde_json::Value::as_str)
                    .is_some()
                {
                    ids.insert(id.to_string());
                }
            }
            for value in map.values() {
                collect_asset_ref_ids_recursive(value, ids);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_asset_ref_ids_recursive(value, ids);
            }
        }
        _ => {}
    }
}

fn editor_asset_ref_to_runtime(
    asset: &EditorAssetRef,
    project_root: &Path,
    assets: &mut BTreeMap<String, RuntimePackageSourceAsset>,
) -> RuntimeAssetRef {
    assets.entry(asset.asset_id.clone()).or_insert_with(|| {
        let source = asset_source_for(project_root, &asset.asset_id);
        RuntimePackageSourceAsset::new(
            asset.asset_id.clone(),
            asset.asset_id.clone(),
            asset.asset_type_id.clone(),
            source,
            format!("cooked/{}.asset", asset.asset_id),
        )
    });
    RuntimeAssetRef {
        id: asset.asset_id.clone(),
        asset_type: asset.asset_type_id.clone(),
        guid: asset.guid.clone(),
        sub_asset: asset.sub_asset_id.clone(),
    }
}

fn asset_source_for(project_root: &Path, asset_id: &str) -> String {
    let asset_file = format!("Assets/{asset_id}.asset");
    if project_root.join(&asset_file).exists() {
        return asset_file;
    }
    let png_file = format!("Assets/{asset_id}.png");
    if project_root.join(&png_file).exists() {
        return png_file;
    }
    format!("Assets/{asset_id}")
}

fn descriptor_asset_type(
    project_root: &Path,
    relative_source: &str,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> String {
    let path = project_root.join(relative_source);
    let Ok(text) = fs::read_to_string(&path) else {
        return "asset".to_string();
    };
    if !text.contains("texture-asset.v1") {
        return "asset".to_string();
    }
    match serde_json::from_str::<TextureAssetDescriptor>(&text) {
        Ok(descriptor) if descriptor.schema_version == "texture-asset.v1" => "texture".to_string(),
        Ok(descriptor) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "UnsupportedTextureAssetSchema",
                    format!(
                        "Texture descriptor schema must be texture-asset.v1, got {}.",
                        descriptor.schema_version
                    ),
                )
                .with_path(relative_source.to_string()),
            );
            "texture".to_string()
        }
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "TextureDescriptorParseFailed",
                    format!("Failed to parse texture descriptor: {error}"),
                )
                .with_path(relative_source.to_string()),
            );
            "texture".to_string()
        }
    }
}

fn cook_texture_payloads(
    project_root: &Path,
    assets: &[RuntimePackageSourceAsset],
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Vec<RuntimePackageSourceTexture> {
    assets
        .iter()
        .filter(|asset| asset.asset_type == "texture")
        .filter_map(|asset| cook_texture_payload(project_root, asset, diagnostics))
        .collect()
}

fn cook_texture_payload(
    project_root: &Path,
    asset: &RuntimePackageSourceAsset,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Option<RuntimePackageSourceTexture> {
    let (source_image, importer) = resolve_texture_source(project_root, asset, diagnostics)?;
    if !importer.format.eq_ignore_ascii_case("png") {
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Asset,
                "UnsupportedTextureSourceFormat",
                format!(
                    "Texture {} uses unsupported source format {}.",
                    asset.asset_id, importer.format
                ),
            )
            .with_path(source_image.clone())
            .with_suggestion("C-min only supports PNG source images."),
        );
        return None;
    }
    let source_path = project_root.join(&source_image);
    let rgba8 = match decode_png_rgba8(&source_path) {
        Ok(decoded) => decoded,
        Err(message) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "TextureDecodeFailed",
                    format!("Failed to decode texture {}: {message}", asset.asset_id),
                )
                .with_path(source_image)
                .with_suggestion("Use a valid PNG image for texture-asset.v1 sourceImage."),
            );
            return None;
        }
    };
    let source_bytes = fs::read(&source_path).unwrap_or_default();
    let source_hash = stable_hash_bytes(&source_bytes);
    let metadata = CookedTextureAsset {
        schema_version: COOKED_TEXTURE_SCHEMA_VERSION.to_string(),
        asset_id: asset.asset_id.clone(),
        cooked_asset_id: format!("cooked-{}", asset.asset_id),
        source_hash,
        width: rgba8.width,
        height: rgba8.height,
        format: if importer.color_space.eq_ignore_ascii_case("srgb") {
            "rgba8UnormSrgb".to_string()
        } else {
            "rgba8Unorm".to_string()
        },
        color_space: importer.color_space,
        mip_count: 1,
        byte_length: rgba8.bytes.len(),
        pixel_data_path: format!("cooked/textures/{}.rgba8", asset.asset_id),
        sampler: importer.sampler,
    };
    Some(RuntimePackageSourceTexture {
        metadata,
        rgba8: rgba8.bytes,
    })
}

fn resolve_texture_source(
    project_root: &Path,
    asset: &RuntimePackageSourceAsset,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> Option<(String, TextureImporterDescriptor)> {
    if asset.source.to_ascii_lowercase().ends_with(".png") {
        if project_root.join(&asset.source).exists() {
            return Some((asset.source.clone(), TextureImporterDescriptor::default()));
        }
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Asset,
                "TextureSourceMissing",
                format!("Texture source image is missing: {}", asset.source),
            )
            .with_path(asset.source.clone()),
        );
        return None;
    }

    let descriptor_path = project_root.join(&asset.source);
    let text = match fs::read_to_string(&descriptor_path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "TextureDescriptorReadFailed",
                    format!("Failed to read texture descriptor: {error}"),
                )
                .with_path(asset.source.clone()),
            );
            return None;
        }
    };
    let descriptor = match serde_json::from_str::<TextureAssetDescriptor>(&text) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            diagnostics.push(
                ProjectRuntimePackageAssemblyDiagnostic::error(
                    ProjectRuntimePackageAssemblyDomain::Asset,
                    "TextureDescriptorParseFailed",
                    format!("Failed to parse texture descriptor: {error}"),
                )
                .with_path(asset.source.clone()),
            );
            return None;
        }
    };
    if descriptor.schema_version != "texture-asset.v1" {
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Asset,
                "UnsupportedTextureAssetSchema",
                format!(
                    "Texture descriptor schema must be texture-asset.v1, got {}.",
                    descriptor.schema_version
                ),
            )
            .with_path(asset.source.clone()),
        );
        return None;
    }
    if descriptor
        .asset_id
        .as_ref()
        .is_some_and(|descriptor_id| descriptor_id != &asset.asset_id)
    {
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Asset,
                "TextureAssetIdMismatch",
                format!(
                    "Texture descriptor assetId {:?} does not match RuntimeAsset id {}.",
                    descriptor.asset_id, asset.asset_id
                ),
            )
            .with_path(asset.source.clone()),
        );
        return None;
    }
    if !project_root.join(&descriptor.source_image).exists() {
        diagnostics.push(
            ProjectRuntimePackageAssemblyDiagnostic::error(
                ProjectRuntimePackageAssemblyDomain::Asset,
                "TextureSourceMissing",
                format!(
                    "Texture source image is missing: {}",
                    descriptor.source_image
                ),
            )
            .with_path(descriptor.source_image.clone()),
        );
        return None;
    }
    Some((descriptor.source_image, descriptor.importer))
}

struct DecodedRgbaImage {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

fn decode_png_rgba8(path: &Path) -> Result<DecodedRgbaImage, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let bytes = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks_exact(2)
            .flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err("indexed PNG was not expanded to RGB/RGBA".to_string());
        }
    };
    Ok(DecodedRgbaImage {
        width: info.width,
        height: info.height,
        bytes: rgba,
    })
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn collect_json_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths = read_dir
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn collect_animator2d_registry(
    project_root: &Path,
    available_sprite_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) -> CookedAnimator2DRegistry {
    let animation_root = project_root.join("Animations");
    let mut clips = Vec::<SpriteAnimationClip2DAsset>::new();
    let mut controllers = Vec::<AnimatorController2DAsset>::new();
    for path in collect_json_files(&animation_root) {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let source = path
            .strip_prefix(project_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                diagnostics.push(animator2d_assembly_diagnostic(
                    "animator2d.asset_read_failed",
                    &source,
                    error.to_string(),
                    "Make the animation asset readable and rebuild the RuntimePackage.",
                ));
                continue;
            }
        };
        let result = if file_name.ends_with(".sprite-animation-clip-2d.json") {
            Animator2DAssetCooker::parse_clip_json(&source, &text).map(|asset| clips.push(asset))
        } else if file_name.ends_with(".animator-controller-2d.json") {
            Animator2DAssetCooker::parse_controller_json(&source, &text)
                .map(|asset| controllers.push(asset))
        } else {
            continue;
        };
        if let Err(failure) = result {
            diagnostics.extend(failure.diagnostics.into_iter().map(|diagnostic| {
                animator2d_assembly_diagnostic(
                    diagnostic.code,
                    diagnostic.path,
                    diagnostic.message,
                    diagnostic.next_action,
                )
            }));
        }
    }
    match Animator2DAssetCooker::cook(clips, controllers, available_sprite_ids) {
        Ok(registry) => registry,
        Err(failure) => {
            diagnostics.extend(failure.diagnostics.into_iter().map(|diagnostic| {
                animator2d_assembly_diagnostic(
                    diagnostic.code,
                    diagnostic.path,
                    diagnostic.message,
                    diagnostic.next_action,
                )
            }));
            CookedAnimator2DRegistry::empty()
        }
    }
}

fn resolve_scene_animator2d_components(
    scenes: &mut [RuntimeScene],
    registry: &CookedAnimator2DRegistry,
    diagnostics: &mut Vec<ProjectRuntimePackageAssemblyDiagnostic>,
) {
    for scene in scenes {
        for entity in &mut scene.entities {
            let Some(position) = entity
                .components
                .iter()
                .position(|component| component.component_type == "Animator2D")
            else {
                continue;
            };
            let component = entity.components.remove(position);
            if entity.sprite_renderer2d.is_none() {
                diagnostics.push(animator2d_assembly_diagnostic(
                    "animator2d.sprite_renderer2d_missing",
                    format!("scene.entities.{}.Animator2D", entity.id),
                    "Animator2D requires SpriteRenderer2D on the same entity.",
                    "Add SpriteRenderer2D before attaching Animator2D.",
                ));
                continue;
            }
            let controller_id = component
                .data
                .get("controllerRef")
                .or_else(|| component.data.get("controllerId"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let Some(controller_index) = registry.controller_index(controller_id) else {
                diagnostics.push(animator2d_assembly_diagnostic(
                    "animator2d.controller_missing",
                    format!("scene.entities.{}.Animator2D.controllerRef", entity.id),
                    format!("AnimatorController2D is not available: {controller_id}."),
                    "Assign an existing controller asset from Animations/.",
                ));
                continue;
            };
            let initial_bools = component
                .data
                .get("initialBools")
                .cloned()
                .map(serde_json::from_value)
                .transpose();
            let initial_bools = match initial_bools {
                Ok(Some(values)) => values,
                Ok(None) => BTreeMap::new(),
                Err(error) => {
                    diagnostics.push(animator2d_assembly_diagnostic(
                        "animator2d.initial_bools_invalid",
                        format!("scene.entities.{}.Animator2D.initialBools", entity.id),
                        error.to_string(),
                        "Use an object whose values are Bool.",
                    ));
                    continue;
                }
            };
            entity.animator2d = Some(RuntimeAnimator2D {
                controller_id: controller_id.to_string(),
                controller_index,
                registry_digest: registry.registry_digest.clone(),
                enabled: component
                    .data
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
                initial_bools,
            });
        }
    }
}

fn animator2d_assembly_diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
    suggestion: impl Into<String>,
) -> ProjectRuntimePackageAssemblyDiagnostic {
    ProjectRuntimePackageAssemblyDiagnostic::error(
        ProjectRuntimePackageAssemblyDomain::Animator2D,
        code,
        message,
    )
    .with_path(path)
    .with_suggestion(suggestion)
}

fn read_json_value(path: &Path) -> Result<serde_json::Value, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("Failed to read JSON: {error}"))?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|error| format!("Failed to parse JSON: {error}"))
}

fn vec3(value: EditorVec3) -> Vector3 {
    Vector3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn engine_builtin_font_pack_selection_covers_default_and_additional_matrix() {
        let profile = |asset_id: &str, role| crate::ProjectFontProfileInventoryEntry {
            asset_id: asset_id.to_string(),
            role,
        };
        assert_eq!(select_font_profiles(&[]), Ok((None, Vec::new())));
        assert_eq!(
            select_font_profiles(&[profile("extra", FontAtlasProfileRole::Additional)]),
            Ok((None, vec!["extra".to_string()]))
        );
        assert_eq!(
            select_font_profiles(&[
                profile("default", FontAtlasProfileRole::DefaultUi),
                profile("extra", FontAtlasProfileRole::Additional),
            ]),
            Ok((Some("default".to_string()), vec!["extra".to_string()]))
        );
        assert_eq!(
            select_font_profiles(&[
                profile("default-a", FontAtlasProfileRole::DefaultUi),
                profile("default-b", FontAtlasProfileRole::DefaultUi),
            ]),
            Err(2)
        );
    }

    #[test]
    fn project_runtime_package_assembler_builds_complete_input_for_complex_shooter_sample() {
        let project_root = workspace_root()
            .join("samples")
            .join("complex_shooter_project");

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Success);
        let input = result.build_input.expect("build input should be assembled");
        assert!(input.scenes.len() >= 1);
        assert!(input.prefabs.len() >= 3);
        assert!(input.assets.len() >= 5);
        assert!(input.input_mappings.len() >= 1);
        assert!(input
            .rule_manifest
            .as_ref()
            .is_some_and(|manifest| manifest.rules.len() >= 3));
        assert!(input
            .aui_manifest
            .as_ref()
            .is_some_and(|manifest| manifest.documents.len() >= 1));
        assert!(input.font_atlases.is_empty());
        assert_eq!(input.font_bundles.len(), 1);
        assert_eq!(
            input.font_bundles[0].metadata.font_bundle_id,
            crate::ENGINE_BUILT_IN_FONT_PACK_ID
        );
        assert!(input.texture_payloads.len() >= 4);
        assert!(input.texture_payloads.iter().any(|texture| {
            texture.metadata.asset_id == "tex-player-ship"
                && texture.metadata.pixel_data_path == "cooked/textures/tex-player-ship.rgba8"
                && !texture.rgba8.is_empty()
        }));
        assert_eq!(result.report.font_atlas_count, input.font_atlases.len());
        assert_eq!(result.report.font_bundle_count, input.font_bundles.len());
        assert!(result.report.producer_reports.iter().any(|report| {
            report.producer_id == "font-built-in-default"
                && report.output_digest.as_deref()
                    == Some(input.font_bundles[0].metadata.bundle_digest.as_str())
                && report
                    .diagnostics
                    .iter()
                    .any(|entry| entry == "fontRasterDurationMs=0")
        }));
        assert!(result.report.source_mappings.iter().any(|mapping| {
            mapping.domain == ProjectRuntimePackageAssemblyDomain::Project
                && mapping.object_id == "project-complex-shooter-sample"
                && mapping.source_path == "project.aife.json"
        }));
        assert!(result.report.source_mappings.iter().any(|mapping| {
            mapping.domain == ProjectRuntimePackageAssemblyDomain::Scene
                && mapping.source_path == "Scenes/Main.scene.json"
                && mapping.runtime_path == "scenes/scene-main.json"
        }));
        assert!(result.report.source_mappings.iter().any(|mapping| {
            mapping.domain == ProjectRuntimePackageAssemblyDomain::Prefab
                && mapping.object_id == "prefab-player-bullet"
                && mapping.source_path == "Prefabs/player_bullet.prefab.json"
        }));
        assert!(result.report.source_mappings.iter().any(|mapping| {
            mapping.domain == ProjectRuntimePackageAssemblyDomain::Aui
                && mapping.object_id == "hud-main"
                && mapping.source_path == "AUI/hud.aui.json"
                && mapping.runtime_path == "aui/documents/hud-main.aui.json"
        }));
        assert!(input
            .assets
            .iter()
            .any(|asset| asset.source == "Assets/tex-player-ship.asset"));
    }

    #[test]
    fn project_runtime_package_assembler_collects_aui_documents_from_ui_root() {
        let project_root = copy_sample_project("aui-ui-root");
        fs::create_dir_all(project_root.join("UI")).unwrap();
        fs::rename(
            project_root.join("AUI").join("hud.aui.json"),
            project_root.join("UI").join("hud.aui.json"),
        )
        .unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Success);
        let input = result.build_input.expect("build input should be assembled");
        assert!(input.aui_manifest.as_ref().is_some_and(|manifest| {
            manifest
                .documents
                .iter()
                .any(|document| document.document_id == "hud-main")
        }));
        assert!(result.report.source_mappings.iter().any(|mapping| {
            mapping.domain == ProjectRuntimePackageAssemblyDomain::Aui
                && mapping.object_id == "hud-main"
                && mapping.source_path == "UI/hud.aui.json"
        }));
    }

    #[test]
    fn animator2d_package_assembler_discovers_and_cooks_animation_assets() {
        let project_root = copy_sample_project("animator2d-package");
        let animations = project_root.join("Animations");
        fs::create_dir_all(&animations).unwrap();
        fs::write(
            animations.join("idle.sprite-animation-clip-2d.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "sprite-animation-clip-2d.v1",
                "assetId": "clip-idle",
                "playback": "loop",
                "frames": [{"spriteRef": "tex-player-ship", "durationTicks": 2}]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            animations.join("main.animator-controller-2d.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "animator-controller-2d.v1",
                "assetId": "controller-main",
                "parameters": [],
                "entryStateId": "idle",
                "states": [{"id": "idle", "clipRef": "clip-idle", "speedPermille": 1000}],
                "transitions": []
            }))
            .unwrap(),
        )
        .unwrap();
        let scene_path = project_root.join("Scenes/main.scene.json");
        let mut scene: serde_json::Value =
            serde_json::from_slice(&fs::read(&scene_path).unwrap()).unwrap();
        let player = scene["entities"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entity| entity["id"] == "entity-player")
            .unwrap();
        player["components"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "componentType": "Animator2D",
                "data": {
                    "controllerRef": "controller-main",
                    "enabled": true,
                    "initialBools": {}
                }
            }));
        fs::write(&scene_path, serde_json::to_vec_pretty(&scene).unwrap()).unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(
            result.status,
            ProjectRuntimePackageAssemblyStatus::Success,
            "{:?}",
            result.report.diagnostics
        );
        let input = result.build_input.unwrap();
        let registry = &input.animator2d_registry;
        assert_eq!(registry.clips[0].id, "clip-idle");
        assert_eq!(registry.controllers[0].id, "controller-main");
        assert!(registry.registry_digest.starts_with("sha256:"));
        let player = input.scenes[0]
            .entities
            .iter()
            .find(|entity| entity.id == "entity-player")
            .unwrap();
        assert_eq!(
            player
                .animator2d
                .as_ref()
                .map(|animator| animator.controller_index),
            Some(0)
        );
        assert!(!player
            .components
            .iter()
            .any(|component| component.component_type == "Animator2D"));

        let mut scene: serde_json::Value =
            serde_json::from_slice(&fs::read(&scene_path).unwrap()).unwrap();
        let animator = scene["entities"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entity| entity["id"] == "entity-player")
            .unwrap()["components"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|component| component["componentType"] == "Animator2D")
            .unwrap();
        animator["data"]["controllerRef"] = serde_json::json!("missing-controller");
        fs::write(&scene_path, serde_json::to_vec_pretty(&scene).unwrap()).unwrap();
        let missing_controller = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );
        assert_eq!(
            missing_controller.status,
            ProjectRuntimePackageAssemblyStatus::Failed
        );
        assert!(missing_controller
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "animator2d.controller_missing" }));
    }

    #[test]
    fn project_runtime_package_assembler_excludes_project_intent_journal() {
        let project_root = copy_sample_project("exclude-project-intent-journal");
        let journal_path = project_root
            .join("Library")
            .join("ProjectIntent")
            .join("journal.json");
        fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
        fs::write(
            &journal_path,
            br#"{"privateMarker":"PROJECT_INTENT_MUST_NOT_SHIP"}"#,
        )
        .unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Success);
        let encoded_report = serde_json::to_string(&result.report).unwrap();
        let encoded_input = serde_json::to_string(result.build_input.as_ref().unwrap()).unwrap();
        for encoded in [&encoded_report, &encoded_input] {
            assert!(!encoded.contains("Library/ProjectIntent"));
            assert!(!encoded.contains("PROJECT_INTENT_MUST_NOT_SHIP"));
        }
        assert!(result.report.source_mappings.iter().all(|mapping| {
            !mapping.source_path.contains("ProjectIntent")
                && !mapping.runtime_path.contains("ProjectIntent")
        }));
        assert!(result.build_input.unwrap().assets.iter().all(|asset| {
            !asset.source.contains("ProjectIntent") && !asset.runtime_uri.contains("ProjectIntent")
        }));
    }

    #[test]
    fn project_consistency_assembly_report_reads_legacy_json_without_source_mappings() {
        let project_root = workspace_root()
            .join("samples")
            .join("complex_shooter_project");
        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );
        let mut legacy = serde_json::to_value(&result.report).unwrap();
        legacy.as_object_mut().unwrap().remove("sourceMappings");
        let decoded: ProjectRuntimePackageAssemblyReport = serde_json::from_value(legacy).unwrap();
        assert!(decoded.source_mappings.is_empty());
    }

    #[test]
    fn project_runtime_package_assembler_bakes_scene_prefab_instances() {
        let project_root = copy_sample_project("prefab-runtime-bake");

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Success);
        let prefab_bake_report = result
            .report
            .prefab_bake_report
            .as_ref()
            .expect("prefab bake report should be present");
        assert!(prefab_bake_report.prefab_asset_count >= 3);
        assert!(prefab_bake_report.scene_prefab_instance_count >= 1);
        assert!(prefab_bake_report.baked_instance_count >= 1);
        assert!(prefab_bake_report.instances.iter().any(|instance| {
            instance.scene_entity_id == "entity-enemy-a"
                && instance.prefab_id == "prefab-enemy-scout"
                && instance.root_source_entity_id == "entity-enemy-scout-root"
                && instance
                    .emitted_entity_ids
                    .contains(&"entity-enemy-a".to_string())
        }));

        let input = result.build_input.expect("build input should be assembled");
        let scene = input.scenes.first().expect("runtime scene should exist");
        let enemy = scene
            .entities
            .iter()
            .find(|entity| entity.id == "entity-enemy-a")
            .expect("prefab root should bake to scene placeholder id");
        assert!(!enemy
            .components
            .iter()
            .any(|component| component.component_type == PREFAB_INSTANCE_COMPONENT_TYPE));
        assert_eq!(
            enemy.sibling_order,
            scene_entity_sibling_order(&project_root, "entity-enemy-a")
        );
        assert_eq!(
            enemy
                .transform
                .as_ref()
                .map(|transform| transform.local_position.x),
            Some(scene_entity_local_position_x(
                &project_root,
                "entity-enemy-a"
            ))
        );
        let linear_motion = enemy
            .components
            .iter()
            .find(|component| component.component_type == "project.linearMotion")
            .expect("prefab component override should survive bake");
        assert_eq!(linear_motion.data["velocity"]["x"], serde_json::json!(0.6));
        assert_eq!(linear_motion.data["velocity"]["y"], serde_json::json!(-1.2));
    }

    #[test]
    fn project_runtime_package_assembler_bakes_prefab_children_with_deterministic_ids() {
        let project_root = copy_sample_project("prefab-runtime-bake-child");
        let prefab_path = project_root.join("Prefabs").join("enemy_scout.prefab.json");
        let mut prefab: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&prefab_path).unwrap()).unwrap();
        prefab["entities"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "sourceEntityId": "muzzle",
                "name": "Muzzle",
                "parentSourceEntityId": "entity-enemy-scout-root",
                "siblingOrder": 7,
                "enabled": false,
                "transform": {
                    "localPosition": { "x": 0.25, "y": -0.5, "z": 0 },
                    "localRotation": { "x": 0, "y": 0, "z": 0 },
                    "localScale": { "x": 1, "y": 1, "z": 1 }
                },
                "components": []
            }));
        fs::write(&prefab_path, serde_json::to_string_pretty(&prefab).unwrap()).unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Success);
        let input = result.build_input.expect("build input should be assembled");
        let scene = input.scenes.first().expect("runtime scene should exist");
        let child = scene
            .entities
            .iter()
            .find(|entity| entity.id == "entity-enemy-a__muzzle")
            .expect("prefab child should use deterministic scene-prefixed id");
        assert_eq!(child.parent_id.as_deref(), Some("entity-enemy-a"));
        assert_eq!(child.sibling_order, 7);
        assert!(!child.enabled);
        assert_eq!(
            child
                .transform
                .as_ref()
                .map(|transform| transform.local_position.x),
            Some(0.25)
        );
    }

    #[test]
    fn project_runtime_package_assembler_rejects_runtime_only_scene_prefab_instance() {
        let project_root = copy_sample_project("prefab-runtime-only-reference");
        let prefab_path = project_root.join("Prefabs").join("enemy_scout.prefab.json");
        fs::write(
            &prefab_path,
            r#"{
  "schemaVersion": "runtime-prefab.v1",
  "id": "prefab-enemy-scout",
  "name": "Enemy Scout",
  "rootEntityId": "entity-enemy-scout-root",
  "entities": []
}"#,
        )
        .unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Failed);
        assert!(result.report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "scene_prefab_instance_requires_authoring_prefab_asset"
        }));
    }

    #[test]
    fn project_runtime_package_assembler_rejects_prefab_source_id_reserved_separator() {
        let project_root = copy_sample_project("prefab-reserved-separator");
        let prefab_path = project_root.join("Prefabs").join("enemy_scout.prefab.json");
        let mut prefab: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&prefab_path).unwrap()).unwrap();
        prefab["rootEntityId"] = serde_json::json!("entity__enemy__root");
        prefab["entities"][0]["sourceEntityId"] = serde_json::json!("entity__enemy__root");
        fs::write(&prefab_path, serde_json::to_string_pretty(&prefab).unwrap()).unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Failed);
        assert!(result.report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "prefab_source_entity_id_contains_reserved_separator"
        }));
    }

    #[test]
    fn project_runtime_package_assembler_reports_invalid_rule_manifest() {
        let project_root = copy_sample_project("invalid-rule-manifest");
        fs::write(
            project_root.join("Rules").join("rule-manifest.json"),
            r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "rust-aot",
  "rules": [
    {
      "ruleId": "rule.bad",
      "phase": "Update",
      "enabled": true,
      "executor": "rustAot",
      "irHash": "abc",
      "artifactId": "wrong"
    }
  ],
  "modules": []
}"#,
        )
        .unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Failed);
        assert!(result
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.domain == ProjectRuntimePackageAssemblyDomain::Rule));
    }

    #[test]
    fn project_runtime_package_assembler_allows_missing_optional_domains() {
        let project_root = copy_sample_project("missing-optional-domains");
        let _ = fs::remove_file(project_root.join("Rules").join("rule-manifest.json"));
        let _ = fs::remove_dir_all(project_root.join("AUI"));

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Success);
        assert!(result.build_input.as_ref().unwrap().rule_manifest.is_none());
        assert!(result.build_input.as_ref().unwrap().aui_manifest.is_none());
        assert!(result
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "RuleManifestMissing"));
        assert!(result
            .build_input
            .as_ref()
            .unwrap()
            .observation_contract
            .is_none());
    }

    #[test]
    fn project_observation_contract_is_loaded_through_safe_project_path() {
        let project_root = copy_sample_project("project-observation-contract-valid");
        let manifest_path = project_root.join("project.aife.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["observationContract"] =
            serde_json::json!("Observations/project.observations.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(project_root.join("Observations")).unwrap();
        fs::write(
            project_root
                .join("Observations")
                .join("project.observations.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "project-observation-contract.v1",
                "contractId": "sample.runtime-observations",
                "observations": [{
                    "path": "sample.phase",
                    "type": "string",
                    "description": "Current authoritative phase",
                    "allowedValues": ["ready", "finished"]
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let result = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(result.status, ProjectRuntimePackageAssemblyStatus::Success);
        let contract = result
            .build_input
            .unwrap()
            .observation_contract
            .expect("typed observation contract");
        assert_eq!(contract.contract_id, "sample.runtime-observations");
        assert_eq!(contract.observations[0].path, "sample.phase");
    }

    #[test]
    fn project_observation_contract_rejects_escaping_or_invalid_contracts_with_actionable_diagnostics(
    ) {
        let project_root = copy_sample_project("project-observation-contract-invalid");
        let manifest_path = project_root.join("project.aife.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["observationContract"] = serde_json::json!("../outside.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let escaping = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(escaping.status, ProjectRuntimePackageAssemblyStatus::Failed);
        let diagnostic = escaping
            .report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "project_observation.contract_path_invalid")
            .expect("escaping path diagnostic");
        assert_eq!(
            diagnostic.stage.as_deref(),
            Some("project-observation-contract-read")
        );
        assert!(diagnostic.path.is_some());
        assert!(diagnostic.suggestion.is_some());

        manifest["observationContract"] =
            serde_json::json!("Observations/project.observations.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(project_root.join("Observations")).unwrap();
        fs::write(
            project_root
                .join("Observations")
                .join("project.observations.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "project-observation-contract.v9",
                "contractId": "sample.runtime-observations",
                "observations": []
            }))
            .unwrap(),
        )
        .unwrap();

        let invalid = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project_root),
        );

        assert_eq!(invalid.status, ProjectRuntimePackageAssemblyStatus::Failed);
        assert!(invalid.report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "project_observation.contract_schema_unsupported"
                && diagnostic.stage.as_deref() == Some("project-observation-contract-validate")
                && diagnostic.suggestion.is_some()
        }));
    }

    fn copy_sample_project(name: &str) -> PathBuf {
        let source = workspace_root()
            .join("samples")
            .join("complex_shooter_project");
        let destination = unique_temp_dir(name);
        copy_dir_recursive(&source, &destination);
        destination
    }

    fn scene_entity_sibling_order(project_root: &Path, entity_id: &str) -> i32 {
        let scene_path = project_root.join("Scenes").join("Main.scene.json");
        let text = fs::read_to_string(scene_path).unwrap();
        let scene: serde_json::Value = serde_json::from_str(&text).unwrap();
        scene["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["id"].as_str() == Some(entity_id))
            .and_then(|entity| entity["siblingOrder"].as_i64())
            .unwrap() as i32
    }

    fn scene_entity_local_position_x(project_root: &Path, entity_id: &str) -> f32 {
        let scene_path = project_root.join("Scenes").join("Main.scene.json");
        let text = fs::read_to_string(scene_path).unwrap();
        let scene: serde_json::Value = serde_json::from_str(&text).unwrap();
        scene["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["id"].as_str() == Some(entity_id))
            .and_then(|entity| entity["transform"]["localPosition"]["x"].as_f64())
            .unwrap() as f32
    }

    fn copy_dir_recursive(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap().flatten() {
            let source_path = entry.path();
            if source_path.is_dir() && entry.file_name() == "Build" {
                continue;
            }
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir_recursive(&source_path, &destination_path);
            } else {
                fs::copy(&source_path, &destination_path).unwrap();
            }
        }
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{stamp}"))
    }
}
