use crate::{
    GameProjectCompiler, ProjectAssemblyProducerReport, ProjectAuthoringSession, TargetProfile,
};
use engine_runtime::game_view_presentation::GameViewTargetSpec;
use engine_runtime::runtime_package_builder::RuntimePackageBuildInput;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BUILD_PROFILE_SCHEMA_VERSION: &str = "build-profile.v2";
pub const BUILD_PROFILE_SCHEMA_VERSION_V1: &str = "build-profile.v1";
pub const PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION: &str =
    "project-runtime-package-assembly-report.v2";
pub const PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION: &str = "prefab-runtime-bake-report.v1";

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
    pub game_view_target: Option<GameViewTargetSpec>,
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
        let mut issues = Vec::new();
        if !matches!(
            self.schema_version.as_str(),
            BUILD_PROFILE_SCHEMA_VERSION | BUILD_PROFILE_SCHEMA_VERSION_V1
        ) {
            issues.push(BuildProfileValidationIssue::new(
                "release_profile_schema_unsupported",
                "schemaVersion",
                format!("Unsupported build profile schema {}.", self.schema_version),
                "Use build-profile.v1 for dev or build-profile.v2 for release.",
            ));
            return issues;
        }
        if !matches!(self.target.as_str(), "windows" | "android") {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "target",
                format!("Unsupported build target {}.", self.target),
                "Set target to windows or android.",
            ));
        }
        if self.runtime_package_mode != "debug-readable" {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "runtimePackageMode",
                format!(
                    "Unsupported RuntimePackage mode {}.",
                    self.runtime_package_mode
                ),
                "Use debug-readable.",
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
                "realWindowSmoke is invalid.",
                "Use disabled, optional, or required.",
            ));
        }
        if self.schema_version == BUILD_PROFILE_SCHEMA_VERSION_V1 {
            if self.profile != "dev"
                || self.architecture.is_some()
                || self.application.is_some()
                || self.release.is_some()
            {
                issues.push(BuildProfileValidationIssue::new(
                    "build_profile_v1_invalid",
                    "profile",
                    "build-profile.v1 only supports a dev profile without release fields.",
                    "Use profile=dev or migrate to build-profile.v2.",
                ));
            }
            return issues;
        }
        if self.target != "windows" || self.profile != "release" {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "profile",
                "build-profile.v2 requires the Windows release profile.",
                "Set target=windows and profile=release.",
            ));
        }
        if self.architecture.as_deref() != Some("x86_64") {
            issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "architecture",
                "Windows release architecture must be x86_64.",
                "Set architecture to x86_64.",
            ));
        }
        match &self.application {
            Some(application) => validate_application(application, &mut issues),
            None => issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "application",
                "Release profile is missing application identity.",
                "Add the complete application block.",
            )),
        }
        match &self.release {
            Some(release)
                if release.layout == "portable-directory-v1"
                    && !release.include_reports
                    && !release.include_debug_symbols => {}
            Some(_) => issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "release",
                "Release settings must use portable-directory-v1 without reports or symbols.",
                "Use the portable release defaults.",
            )),
            None => issues.push(BuildProfileValidationIssue::new(
                "release_identity_invalid",
                "release",
                "Release profile is missing release settings.",
                "Add the release block.",
            )),
        }
        issues
    }

    pub fn is_release_v2(&self) -> bool {
        self.schema_version == BUILD_PROFILE_SCHEMA_VERSION && self.profile == "release"
    }
}

fn validate_application(
    application: &BuildProfileApplication,
    issues: &mut Vec<BuildProfileValidationIssue>,
) {
    if application.display_name.trim().is_empty()
        || application.company_name.trim().is_empty()
        || application.file_description.trim().is_empty()
        || application.display_version.trim().is_empty()
        || application.copyright.trim().is_empty()
    {
        issues.push(BuildProfileValidationIssue::new(
            "release_identity_invalid",
            "application",
            "Release application metadata must not be empty.",
            "Provide complete application metadata.",
        ));
    }
    let executable = application.executable_name.as_str();
    if executable.is_empty()
        || executable.contains(['/', '\\'])
        || executable.ends_with(['.', ' '])
        || is_windows_reserved_device_name(executable)
        || !executable
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, ' ' | '-' | '_' | '.'))
    {
        issues.push(BuildProfileValidationIssue::new(
            "release_executable_name_invalid",
            "application.executableName",
            "Executable name is not a safe Windows file name.",
            "Use a stable ASCII Windows file name without path separators.",
        ));
    }
    if application.icon.asset_id.trim().is_empty()
        || application.icon.asset_id.contains(['/', '\\'])
    {
        issues.push(BuildProfileValidationIssue::new(
            "release_icon_asset_missing",
            "application.icon.assetId",
            "Application icon must be an AssetRef id.",
            "Select a project texture asset.",
        ));
    }
}

fn is_windows_reserved_device_name(value: &str) -> bool {
    let device_base = value
        .split('.')
        .next()
        .unwrap_or(value)
        .trim_end_matches(['.', ' '])
        .to_ascii_uppercase();
    matches!(device_base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(
            device_base.strip_prefix("COM"),
            Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        )
        || matches!(
            device_base.strip_prefix("LPT"),
            Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        )
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

pub struct ProjectRuntimePackageAssembler;

impl ProjectRuntimePackageAssembler {
    pub fn assemble(
        request: ProjectRuntimePackageAssemblyRequest,
    ) -> ProjectRuntimePackageAssemblyResult {
        match assemble_from_snapshot(&request) {
            Ok(result) => result,
            Err(diagnostic) => failed_result(&request.project_root, diagnostic),
        }
    }
}

fn assemble_from_snapshot(
    request: &ProjectRuntimePackageAssemblyRequest,
) -> Result<ProjectRuntimePackageAssemblyResult, ProjectRuntimePackageAssemblyDiagnostic> {
    let mut session = ProjectAuthoringSession::open(&request.project_root).map_err(|error| {
        adapter_error(
            "project_open_failed",
            error.to_string(),
            &request.project_root,
        )
    })?;
    let paths = session
        .source_inventory()
        .map_err(|error| {
            adapter_error(
                "source_inventory_failed",
                error.to_string(),
                &request.project_root,
            )
        })?
        .entries
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("editor-runtime-package-assembly", paths)
        .map_err(|error| {
            adapter_error(
                "snapshot_lease_failed",
                error.to_string(),
                &request.project_root,
            )
        })?;
    let target = target_profile(request.build_profile_path.as_deref());
    let compiler = GameProjectCompiler::bind(&lease).map_err(compiler_diagnostic)?;
    let prepared = match request.artifact_cache_root.as_deref() {
        Some(root) => compiler.prepare_with_artifact_cache(&lease, target, root),
        None => compiler.prepare(&lease, target),
    }
    .map_err(compiler_diagnostic)?;
    let input = prepared.runtime_package_build_input().clone();
    let active_scene_id = input.scenes.first().map(|scene| scene.id.clone());
    let build_profile = read_build_profile(&lease, request.build_profile_path.as_deref());
    let prefab_bake_report = prefab_bake_report(&request.project_root, lease.snapshot(), &input);
    let source_mappings = source_mappings(
        lease.snapshot(),
        &input,
        request.build_profile_path.as_deref(),
    );
    let report = ProjectRuntimePackageAssemblyReport {
        schema_version: PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION.to_string(),
        status: ProjectRuntimePackageAssemblyStatus::Success,
        project_root: request.project_root.display().to_string(),
        active_scene_id: active_scene_id.clone(),
        scene_count: input.scenes.len(),
        prefab_count: input.prefabs.len(),
        asset_count: input.assets.len(),
        rule_count: input
            .rule_manifest
            .as_ref()
            .map(|manifest| manifest.rules.len())
            .unwrap_or_default(),
        input_mapping_count: input.input_mappings.len(),
        aui_document_count: input.aui_documents.len(),
        font_atlas_count: input.font_atlases.len(),
        font_bundle_count: input.font_bundles.len(),
        prefab_bake_report,
        source_mappings,
        producer_reports: prepared.producer_reports().to_vec(),
        diagnostics: Vec::new(),
    };
    Ok(ProjectRuntimePackageAssemblyResult {
        status: ProjectRuntimePackageAssemblyStatus::Success,
        build_input: Some(input),
        active_scene_id,
        build_profile,
        report,
    })
}

fn target_profile(path: Option<&Path>) -> TargetProfile {
    let value = path
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or("windows.dev.json");
    if value.contains("android") {
        TargetProfile::AndroidDev
    } else if value.contains("release") {
        TargetProfile::WindowsRelease
    } else {
        TargetProfile::WindowsDev
    }
}

fn read_build_profile(
    lease: &authoring_project_context::ProjectSnapshotLease,
    path: Option<&Path>,
) -> Option<BuildProfile> {
    let file_name = path
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or("windows.dev.json");
    let relative = format!("BuildProfiles/{file_name}");
    lease
        .snapshot()
        .files
        .iter()
        .find(|file| file.relative_path == relative)
        .and_then(|file| serde_json::from_slice(&file.bytes).ok())
}

fn compiler_diagnostic(
    error: crate::GameProjectCompilerError,
) -> ProjectRuntimePackageAssemblyDiagnostic {
    ProjectRuntimePackageAssemblyDiagnostic::error(
        ProjectRuntimePackageAssemblyDomain::Project,
        error.code(),
        error.message(),
    )
    .with_stage("game-project-compiler-prepare")
    .with_suggestion(error.next_action())
}

fn adapter_error(
    code: &str,
    message: String,
    path: &Path,
) -> ProjectRuntimePackageAssemblyDiagnostic {
    ProjectRuntimePackageAssemblyDiagnostic::error(
        ProjectRuntimePackageAssemblyDomain::Project,
        code,
        message,
    )
    .with_path(path.display().to_string())
    .with_stage("editor-compiler-adapter")
    .with_suggestion("Repair the canonical project and acquire a fresh snapshot lease.")
}

fn failed_result(
    project_root: &Path,
    diagnostic: ProjectRuntimePackageAssemblyDiagnostic,
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
            diagnostics: vec![diagnostic],
        },
    }
}

fn prefab_bake_report(
    project_root: &Path,
    snapshot: &authoring_project_context::ProjectSnapshot,
    input: &RuntimePackageBuildInput,
) -> Option<PrefabRuntimeBakeReport> {
    let scene_file = snapshot.files.iter().find(|file| {
        file.relative_path.starts_with("Scenes/") && file.relative_path.ends_with(".scene.json")
    })?;
    let scene: serde_json::Value = serde_json::from_slice(&scene_file.bytes).ok()?;
    let runtime_scene = input.scenes.first()?;
    let mut instances = Vec::new();
    for entity in scene.get("entities")?.as_array()? {
        let scene_entity_id = entity.get("id")?.as_str()?;
        let component = entity
            .get("components")?
            .as_array()?
            .iter()
            .find(|component| {
                component
                    .get("componentType")
                    .and_then(serde_json::Value::as_str)
                    == Some("engine.prefab_instance")
            });
        let Some(data) = component.and_then(|component| component.get("data")) else {
            continue;
        };
        let prefab_id = data.get("source")?.get("id")?.as_str()?.to_string();
        let instance_id = data
            .get("instanceId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(scene_entity_id)
            .to_string();
        let root_source_entity_id = input
            .prefabs
            .iter()
            .find(|prefab| prefab.prefab_id == prefab_id)
            .and_then(|prefab| prefab.document.get("rootEntityId"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let emitted_entity_ids = runtime_scene
            .entities
            .iter()
            .filter(|runtime| {
                runtime.id == scene_entity_id
                    || runtime.id.starts_with(&format!("{scene_entity_id}__"))
            })
            .map(|runtime| runtime.id.clone())
            .collect::<Vec<_>>();
        instances.push(PrefabRuntimeBakeInstanceEntry {
            scene_entity_id: scene_entity_id.to_string(),
            instance_id,
            prefab_id,
            root_source_entity_id,
            root_runtime_entity_id: scene_entity_id.to_string(),
            emitted_entity_ids,
            applied_override_count: data
                .get("overrides")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
                .unwrap_or_default(),
            ignored_authoring_component_types: Vec::new(),
            local_runtime_component_warnings: Vec::new(),
            diagnostics: Vec::new(),
        });
    }
    Some(PrefabRuntimeBakeReport {
        schema_version: PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION.to_string(),
        status: ProjectRuntimePackageAssemblyStatus::Success,
        report_mode: "summary".to_string(),
        project_root: project_root.display().to_string(),
        scene_id: runtime_scene.id.clone(),
        prefab_asset_count: input.prefabs.len(),
        scene_prefab_instance_count: instances.len(),
        baked_instance_count: instances.len(),
        baked_entity_count: instances
            .iter()
            .map(|entry| entry.emitted_entity_ids.len())
            .sum(),
        instances,
        diagnostics: Vec::new(),
    })
}

fn source_mappings(
    snapshot: &authoring_project_context::ProjectSnapshot,
    input: &RuntimePackageBuildInput,
    profile: Option<&Path>,
) -> Vec<ProjectRuntimeSourceMapping> {
    let mut mappings = vec![ProjectRuntimeSourceMapping {
        domain: ProjectRuntimePackageAssemblyDomain::Project,
        source_path: "project.aife.json".to_string(),
        object_id: input.project.project_id.clone(),
        build_input_path: "project".to_string(),
        runtime_path: "manifest.json#project".to_string(),
    }];
    if let Some(file_name) = profile
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
    {
        mappings.push(ProjectRuntimeSourceMapping {
            domain: ProjectRuntimePackageAssemblyDomain::BuildProfile,
            source_path: format!("BuildProfiles/{file_name}"),
            object_id: file_name.to_string(),
            build_input_path: "buildProfile".to_string(),
            runtime_path: "build-profile".to_string(),
        });
    }
    for file in &snapshot.files {
        let (domain, object_id, target) = if file.relative_path.starts_with("Scenes/")
            && file.relative_path.ends_with(".scene.json")
        {
            (
                ProjectRuntimePackageAssemblyDomain::Scene,
                input
                    .scenes
                    .first()
                    .map(|scene| scene.id.clone())
                    .unwrap_or_default(),
                "scenes",
            )
        } else if file.relative_path.starts_with("Prefabs/")
            && file.relative_path.ends_with(".prefab.json")
        {
            (
                ProjectRuntimePackageAssemblyDomain::Prefab,
                file.relative_path.clone(),
                "prefabs",
            )
        } else if file.relative_path.starts_with("AUI/") && file.relative_path.ends_with(".json") {
            (
                ProjectRuntimePackageAssemblyDomain::Aui,
                file.relative_path.clone(),
                "auiDocuments",
            )
        } else if file.relative_path.starts_with("Input/") && file.relative_path.ends_with(".json")
        {
            (
                ProjectRuntimePackageAssemblyDomain::Input,
                file.relative_path.clone(),
                "inputMappings",
            )
        } else if file.relative_path == "Rules/rule-manifest.json"
            || file.relative_path.ends_with(".rule.json")
        {
            (
                ProjectRuntimePackageAssemblyDomain::Rule,
                file.relative_path.clone(),
                "ruleManifest",
            )
        } else if file.relative_path.starts_with("Assets/")
            && file.relative_path.ends_with(".asset")
        {
            (
                ProjectRuntimePackageAssemblyDomain::Asset,
                file.relative_path.clone(),
                "assets",
            )
        } else {
            continue;
        };
        mappings.push(ProjectRuntimeSourceMapping {
            domain,
            source_path: file.relative_path.clone(),
            object_id,
            build_input_path: target.to_string(),
            runtime_path: target.to_string(),
        });
    }
    mappings
}
