use crate::{
    default_engine_sdk_root, BuildProfile, CandidateProjectRevisionStore,
    ProjectAssemblyProducerReport, ProjectManifest, ProjectPlayerArtifact,
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyDiagnostic,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblySeverity,
    ProjectRuntimePackageAssemblyStatus, ProjectRuntimePlayerArtifactBuildRequest,
    ProjectRuntimePlayerArtifactBuildStatus, ProjectRuntimeSourceKind,
    BUILD_PROFILE_SCHEMA_VERSION, PROJECT_MANIFEST_SCHEMA_VERSION,
    PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION,
};
use engine_runtime::runtime_package::{
    load_runtime_package, RUNTIME_AUI_MANIFEST_SCHEMA_VERSION, RUNTIME_ENTITY_SCHEMA_VERSION,
    RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION, RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION,
    RUNTIME_PACKAGE_SCHEMA_VERSION, RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
    RUNTIME_SCENE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::{
    PreviousPackageManifest, RuntimePackageBuildInput, RuntimePackageBuildRequest,
    RuntimePackageBuildStatus, RuntimePackageBuilder, BUILD_RUNTIME_PACKAGE_REPORT_SCHEMA_VERSION,
    RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION, RUNTIME_PACKAGE_DIFF_REPORT_SCHEMA_VERSION,
    RUNTIME_PACKAGE_VALIDATION_REPORT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION: &str =
    "editor-play-preview-package-report.v1";
pub const EDITOR_PREVIEW_PACKAGE_CACHE_MANIFEST_SCHEMA_VERSION: &str =
    "editor-preview-package-cache-manifest.v1";
pub const EDITOR_PREVIEW_PACKAGE_REQUEST_SCHEMA_VERSION: &str = "editor-preview-package-request.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreviewPackageRequest {
    pub schema_version: String,
    pub project_root: PathBuf,
    pub active_scene_id: Option<String>,
    pub build_profile: String,
    pub requested_by: String,
    pub allow_autosave: bool,
    pub allow_last_good: bool,
    pub force_rebuild: bool,
    pub frame_limit: u64,
    #[serde(default = "default_prepare_player_artifact")]
    pub prepare_player_artifact: bool,
    #[serde(skip)]
    player_artifact_build_root: Option<PathBuf>,
}

impl EditorPreviewPackageRequest {
    pub fn editor_play(project_root: impl Into<PathBuf>) -> Self {
        Self {
            schema_version: EDITOR_PREVIEW_PACKAGE_REQUEST_SCHEMA_VERSION.to_string(),
            project_root: project_root.into(),
            active_scene_id: None,
            build_profile: "windows-dev".to_string(),
            requested_by: "toolbar".to_string(),
            allow_autosave: true,
            allow_last_good: false,
            force_rebuild: false,
            frame_limit: 3,
            prepare_player_artifact: true,
            player_artifact_build_root: None,
        }
    }

    pub fn with_active_scene_id(mut self, active_scene_id: Option<String>) -> Self {
        self.active_scene_id = active_scene_id;
        self
    }

    pub fn with_player_artifact_build_root(mut self, build_root: PathBuf) -> Self {
        self.player_artifact_build_root = Some(build_root);
        self
    }

    pub fn without_player_artifact(mut self) -> Self {
        self.prepare_player_artifact = false;
        self
    }
}

fn default_prepare_player_artifact() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EditorPreviewPackageDirtyDomain {
    Project,
    BuildProfile,
    Scene,
    Prefab,
    Asset,
    Rule,
    Aui,
    Input,
    FontAtlas,
    RuntimeModule,
    EngineSchema,
}

impl EditorPreviewPackageDirtyDomain {
    pub fn as_report_str(&self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::BuildProfile => "BuildProfile",
            Self::Scene => "Scene",
            Self::Prefab => "Prefab",
            Self::Asset => "Asset",
            Self::Rule => "Rule",
            Self::Aui => "Aui",
            Self::Input => "Input",
            Self::FontAtlas => "FontAtlas",
            Self::RuntimeModule => "RuntimeModule",
            Self::EngineSchema => "EngineSchema",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreviewPackageFingerprint {
    pub schema_version: String,
    pub project_id: String,
    pub active_scene_id: String,
    pub build_profile: String,
    pub engine_schema_hash: String,
    pub project_hash: String,
    pub build_profile_hash: String,
    pub scene_hash: String,
    pub prefab_hash: String,
    pub asset_hash: String,
    pub rule_hash: String,
    pub aui_hash: String,
    pub input_hash: String,
    pub font_atlas_seed_hash: String,
    pub runtime_module_hash: String,
    pub combined_hash: String,
}

impl EditorPreviewPackageFingerprint {
    pub fn dirty_domains_since(
        &self,
        previous: &EditorPreviewPackageFingerprint,
    ) -> Vec<EditorPreviewPackageDirtyDomain> {
        let mut domains = Vec::new();
        if self.project_hash != previous.project_hash || self.project_id != previous.project_id {
            domains.push(EditorPreviewPackageDirtyDomain::Project);
        }
        if self.build_profile_hash != previous.build_profile_hash
            || self.build_profile != previous.build_profile
        {
            domains.push(EditorPreviewPackageDirtyDomain::BuildProfile);
        }
        if self.scene_hash != previous.scene_hash
            || self.active_scene_id != previous.active_scene_id
        {
            domains.push(EditorPreviewPackageDirtyDomain::Scene);
        }
        if self.prefab_hash != previous.prefab_hash {
            domains.push(EditorPreviewPackageDirtyDomain::Prefab);
        }
        if self.asset_hash != previous.asset_hash {
            domains.push(EditorPreviewPackageDirtyDomain::Asset);
        }
        if self.rule_hash != previous.rule_hash {
            domains.push(EditorPreviewPackageDirtyDomain::Rule);
        }
        if self.aui_hash != previous.aui_hash {
            domains.push(EditorPreviewPackageDirtyDomain::Aui);
        }
        if self.input_hash != previous.input_hash {
            domains.push(EditorPreviewPackageDirtyDomain::Input);
        }
        if self.font_atlas_seed_hash != previous.font_atlas_seed_hash {
            domains.push(EditorPreviewPackageDirtyDomain::FontAtlas);
        }
        if self.runtime_module_hash != previous.runtime_module_hash {
            domains.push(EditorPreviewPackageDirtyDomain::RuntimeModule);
        }
        if self.engine_schema_hash != previous.engine_schema_hash {
            domains.push(EditorPreviewPackageDirtyDomain::EngineSchema);
        }
        domains
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreviewPackageCacheManifest {
    pub schema_version: String,
    pub project_root: String,
    pub active_scene_id: String,
    pub build_profile: String,
    pub cache_key: String,
    pub fingerprint: EditorPreviewPackageFingerprint,
    pub runtime_package_dir: String,
    pub build_report_path: String,
    pub validation_report_path: String,
    pub diff_report_path: String,
    pub last_success_at: String,
    pub last_success_package_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EditorPreviewPackageStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EditorPreviewPackageCacheStatus {
    None,
    Hit,
    Stale,
    Rebuilt,
    Failed,
    LastGoodAvailable,
}

impl EditorPreviewPackageCacheStatus {
    pub fn as_report_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Hit => "Hit",
            Self::Stale => "Stale",
            Self::Rebuilt => "Rebuilt",
            Self::Failed => "Failed",
            Self::LastGoodAvailable => "LastGoodAvailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorPreviewPackageStageStatus {
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorPreviewPackageDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreviewPackageDiagnostic {
    pub severity: EditorPreviewPackageDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

impl EditorPreviewPackageDiagnostic {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: EditorPreviewPackageDiagnosticSeverity::Info,
            code: code.into(),
            message: message.into(),
            path: None,
            suggestion: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: EditorPreviewPackageDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path: None,
            suggestion: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: EditorPreviewPackageDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path: None,
            suggestion: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl From<ProjectRuntimePackageAssemblyDiagnostic> for EditorPreviewPackageDiagnostic {
    fn from(diagnostic: ProjectRuntimePackageAssemblyDiagnostic) -> Self {
        Self {
            severity: match diagnostic.severity {
                ProjectRuntimePackageAssemblySeverity::Info => {
                    EditorPreviewPackageDiagnosticSeverity::Info
                }
                ProjectRuntimePackageAssemblySeverity::Warning => {
                    EditorPreviewPackageDiagnosticSeverity::Warning
                }
                ProjectRuntimePackageAssemblySeverity::Error => {
                    EditorPreviewPackageDiagnosticSeverity::Error
                }
            },
            code: format!("Assembly::{:?}::{}", diagnostic.domain, diagnostic.code),
            message: diagnostic.message,
            path: diagnostic.path,
            suggestion: diagnostic.suggestion,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreviewPackageStageReport {
    pub stage_id: String,
    pub status: EditorPreviewPackageStageStatus,
    pub duration_ms: u64,
    pub skipped: bool,
    pub cache_status: Option<EditorPreviewPackageCacheStatus>,
    pub dirty_domains: Vec<EditorPreviewPackageDirtyDomain>,
    #[serde(default)]
    pub producer_reports: Vec<ProjectAssemblyProducerReport>,
    pub diagnostics: Vec<EditorPreviewPackageDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPlayPreviewPackageReport {
    pub schema_version: String,
    pub status: EditorPreviewPackageStatus,
    pub project_root: String,
    pub active_scene_id: String,
    pub build_profile: String,
    pub cache_dir: String,
    pub runtime_package_dir: Option<String>,
    pub cache_status: EditorPreviewPackageCacheStatus,
    pub cache_key: String,
    pub previous_cache_key: Option<String>,
    pub dirty_domains: Vec<EditorPreviewPackageDirtyDomain>,
    pub source_fingerprint: EditorPreviewPackageFingerprint,
    pub previous_fingerprint: Option<EditorPreviewPackageFingerprint>,
    pub autosave_summary: String,
    pub stage_reports: Vec<EditorPreviewPackageStageReport>,
    pub runtime_package_build_report_path: Option<String>,
    pub runtime_package_validation_report_path: Option<String>,
    pub runtime_package_diff_report_path: Option<String>,
    pub runtime_package_load_status: String,
    pub player_artifact_status: String,
    pub player_artifact_build_report_path: Option<String>,
    pub player_artifact: Option<ProjectPlayerArtifact>,
    pub play_session_id: Option<String>,
    pub play_session_report_schema: Option<String>,
    pub duration_total_ms: u64,
    pub diagnostics: Vec<EditorPreviewPackageDiagnostic>,
    pub next_actions: Vec<String>,
    pub deferred_flags: Vec<String>,
    pub report_path: Option<String>,
}

impl EditorPlayPreviewPackageReport {
    pub fn has_errors(&self) -> bool {
        self.status == EditorPreviewPackageStatus::Failed
            || self.diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == EditorPreviewPackageDiagnosticSeverity::Error
            })
    }

    pub fn dirty_domain_labels(&self) -> Vec<String> {
        self.dirty_domains
            .iter()
            .map(|domain| domain.as_report_str().to_string())
            .collect()
    }
}

pub struct EditorPreviewPackageService;

impl EditorPreviewPackageService {
    pub fn prepare(request: EditorPreviewPackageRequest) -> EditorPlayPreviewPackageReport {
        let total_started = Instant::now();
        let mut diagnostics = Vec::new();
        let mut stages = Vec::new();

        let fingerprint_started = Instant::now();
        let fingerprint = compute_fingerprint(&request, &mut diagnostics);
        stages.push(stage(
            "fingerprint_sources",
            if diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == EditorPreviewPackageDiagnosticSeverity::Error
            }) {
                EditorPreviewPackageStageStatus::Failed
            } else {
                EditorPreviewPackageStageStatus::Success
            },
            fingerprint_started.elapsed(),
            false,
            None,
            Vec::new(),
            Vec::new(),
        ));

        let cache_dir = preview_cache_dir(
            &request.project_root,
            &request.build_profile,
            &fingerprint.active_scene_id,
        );
        let runtime_package_dir = cache_dir.join("runtime_package");
        let artifact_cache_root = request.project_root.join(".aife").join("derived-data");
        let reports_dir = runtime_package_dir.join("reports");
        let report_path = reports_dir.join("editor-play-preview-package-report.json");
        let manifest_path = cache_dir.join("preview-cache-manifest.json");
        let build_report_path = reports_dir.join("build-runtime-package-report.json");
        let validation_report_path = reports_dir.join("runtime-package-validation-report.json");
        let diff_report_path = reports_dir.join("runtime-package-diff-report.json");

        match crate::ProjectWriteScope::open(&request.project_root).and_then(|scope| {
            let relative = cache_dir.strip_prefix(&request.project_root).map_err(|_| {
                crate::ProjectWriteError {
                    code: "project_write.path_not_relative",
                    operation: crate::ProjectWriteOperation::CreateDirectory,
                    relative_path: Some(cache_dir.display().to_string()),
                    source: None,
                    rollback_error: None,
                }
            })?;
            scope.ensure_directory(relative)?;
            let artifact_relative = artifact_cache_root
                .strip_prefix(&request.project_root)
                .map_err(|_| crate::ProjectWriteError {
                    code: "project_write.path_not_relative",
                    operation: crate::ProjectWriteOperation::CreateDirectory,
                    relative_path: Some(artifact_cache_root.display().to_string()),
                    source: None,
                    rollback_error: None,
                })?;
            scope.ensure_directory(artifact_relative)
        }) {
            Ok(()) => {}
            Err(error) => diagnostics.push(
                EditorPreviewPackageDiagnostic::error(
                    error.code,
                    format!("Preview output containment failed: {error}"),
                )
                .with_path(cache_dir.display().to_string()),
            ),
        }

        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == EditorPreviewPackageDiagnosticSeverity::Error)
        {
            return write_final_report(EditorPlayPreviewPackageReport {
                schema_version: EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
                status: EditorPreviewPackageStatus::Failed,
                project_root: request.project_root.display().to_string(),
                active_scene_id: fingerprint.active_scene_id.clone(),
                build_profile: request.build_profile,
                cache_dir: cache_dir.display().to_string(),
                runtime_package_dir: None,
                cache_status: EditorPreviewPackageCacheStatus::Failed,
                cache_key: fingerprint.combined_hash.clone(),
                previous_cache_key: None,
                dirty_domains: Vec::new(),
                source_fingerprint: fingerprint,
                previous_fingerprint: None,
                autosave_summary: "autosave_not_started_fingerprint_failed".to_string(),
                stage_reports: stages,
                runtime_package_build_report_path: None,
                runtime_package_validation_report_path: None,
                runtime_package_diff_report_path: None,
                runtime_package_load_status: "not_started".to_string(),
                player_artifact_status: "not_started".to_string(),
                player_artifact_build_report_path: None,
                player_artifact: None,
                play_session_id: None,
                play_session_report_schema: None,
                duration_total_ms: elapsed_ms(total_started.elapsed()),
                diagnostics,
                next_actions: vec!["fix_project_files_referenced_by_fingerprint".to_string()],
                deferred_flags: deferred_flags(),
                report_path: Some(report_path.display().to_string()),
            });
        }

        let check_cache_started = Instant::now();
        let previous_manifest = read_cache_manifest(&manifest_path, &mut diagnostics);
        let previous_fingerprint = previous_manifest
            .as_ref()
            .map(|manifest| manifest.fingerprint.clone());
        let previous_cache_key = previous_manifest
            .as_ref()
            .map(|manifest| manifest.cache_key.clone());
        let dirty_domains = previous_fingerprint
            .as_ref()
            .map(|previous| fingerprint.dirty_domains_since(previous))
            .unwrap_or_else(|| {
                vec![
                    EditorPreviewPackageDirtyDomain::Project,
                    EditorPreviewPackageDirtyDomain::BuildProfile,
                    EditorPreviewPackageDirtyDomain::Scene,
                    EditorPreviewPackageDirtyDomain::Prefab,
                    EditorPreviewPackageDirtyDomain::Asset,
                    EditorPreviewPackageDirtyDomain::Rule,
                    EditorPreviewPackageDirtyDomain::Aui,
                    EditorPreviewPackageDirtyDomain::Input,
                    EditorPreviewPackageDirtyDomain::FontAtlas,
                    EditorPreviewPackageDirtyDomain::RuntimeModule,
                    EditorPreviewPackageDirtyDomain::EngineSchema,
                ]
            });
        let cache_hit = previous_manifest.as_ref().is_some_and(|manifest| {
            !request.force_rebuild
                && manifest.schema_version == EDITOR_PREVIEW_PACKAGE_CACHE_MANIFEST_SCHEMA_VERSION
                && manifest.cache_key == fingerprint.combined_hash
                && Path::new(&manifest.runtime_package_dir)
                    .join("manifest.json")
                    .exists()
        });
        let initial_cache_status = if cache_hit {
            EditorPreviewPackageCacheStatus::Hit
        } else if previous_manifest.is_some() {
            EditorPreviewPackageCacheStatus::Stale
        } else {
            EditorPreviewPackageCacheStatus::None
        };
        stages.push(stage(
            "check_cache",
            EditorPreviewPackageStageStatus::Success,
            check_cache_started.elapsed(),
            false,
            Some(initial_cache_status),
            if cache_hit {
                Vec::new()
            } else {
                dirty_domains.clone()
            },
            Vec::new(),
        ));

        if cache_hit {
            stages.push(stage(
                "autosave_dirty_documents",
                EditorPreviewPackageStageStatus::Skipped,
                std::time::Duration::from_millis(0),
                true,
                Some(EditorPreviewPackageCacheStatus::Hit),
                Vec::new(),
                vec![EditorPreviewPackageDiagnostic::info(
                    "editor.preview_package.autosave_skipped_cache_hit",
                    "No preview package rebuild was required, so autosave was not requested by the preview package service.",
                )],
            ));
            stages.push(stage(
                "assemble_project_runtime_package_input",
                EditorPreviewPackageStageStatus::Skipped,
                std::time::Duration::from_millis(0),
                true,
                Some(EditorPreviewPackageCacheStatus::Hit),
                Vec::new(),
                Vec::new(),
            ));
            stages.push(stage(
                "build_runtime_package",
                EditorPreviewPackageStageStatus::Skipped,
                std::time::Duration::from_millis(0),
                true,
                Some(EditorPreviewPackageCacheStatus::Hit),
                Vec::new(),
                Vec::new(),
            ));
            let load_started = Instant::now();
            let load = load_runtime_package(&runtime_package_dir);
            let load_diagnostics = load_diagnostics_to_preview(&load.diagnostics.issues);
            let load_status = if load.value.is_some() {
                EditorPreviewPackageStageStatus::Success
            } else {
                EditorPreviewPackageStageStatus::Failed
            };
            diagnostics.extend(load_diagnostics.clone());
            stages.push(stage(
                "load_validate_runtime_package",
                load_status,
                load_started.elapsed(),
                false,
                Some(EditorPreviewPackageCacheStatus::Hit),
                Vec::new(),
                load_diagnostics,
            ));
            let player_started = Instant::now();
            let prepared_player = load.value.as_ref().and_then(|package| {
                request.prepare_player_artifact.then(|| {
                    prepare_project_player_artifact(
                        &request.project_root,
                        request.player_artifact_build_root.as_deref(),
                        package,
                    )
                })
            });
            if let Some(prepared) = &prepared_player {
                diagnostics.extend(prepared.diagnostics.clone());
                stages.push(stage(
                    "build_project_runtime_player_artifact",
                    prepared.stage_status,
                    player_started.elapsed(),
                    prepared.stage_status == EditorPreviewPackageStageStatus::Skipped,
                    Some(EditorPreviewPackageCacheStatus::Hit),
                    Vec::new(),
                    prepared.diagnostics.clone(),
                ));
            } else {
                stages.push(stage(
                    "build_project_runtime_player_artifact",
                    EditorPreviewPackageStageStatus::Skipped,
                    player_started.elapsed(),
                    true,
                    Some(EditorPreviewPackageCacheStatus::Hit),
                    Vec::new(),
                    vec![EditorPreviewPackageDiagnostic::info(
                        "editor.preview_package.player_artifact_not_required_in_process",
                        "In-process Editor GameView uses the linked RuntimeModule and does not require a standalone Player artifact.",
                    )],
                ));
            }
            let player_failed = prepared_player.as_ref().is_some_and(|prepared| {
                prepared.stage_status == EditorPreviewPackageStageStatus::Failed
            });
            let failed = load.value.is_none() || player_failed;
            return write_final_report(EditorPlayPreviewPackageReport {
                schema_version: EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
                status: if failed {
                    EditorPreviewPackageStatus::Failed
                } else {
                    EditorPreviewPackageStatus::Success
                },
                project_root: request.project_root.display().to_string(),
                active_scene_id: fingerprint.active_scene_id.clone(),
                build_profile: request.build_profile,
                cache_dir: cache_dir.display().to_string(),
                runtime_package_dir: Some(runtime_package_dir.display().to_string()),
                cache_status: if failed {
                    EditorPreviewPackageCacheStatus::Failed
                } else {
                    EditorPreviewPackageCacheStatus::Hit
                },
                cache_key: fingerprint.combined_hash.clone(),
                previous_cache_key,
                dirty_domains: Vec::new(),
                source_fingerprint: fingerprint,
                previous_fingerprint,
                autosave_summary: "cache_hit_no_autosave_requested".to_string(),
                stage_reports: stages,
                runtime_package_build_report_path: Some(build_report_path.display().to_string()),
                runtime_package_validation_report_path: Some(
                    validation_report_path.display().to_string(),
                ),
                runtime_package_diff_report_path: Some(diff_report_path.display().to_string()),
                runtime_package_load_status: if failed {
                    "failed".to_string()
                } else {
                    "success".to_string()
                },
                player_artifact_status: prepared_player
                    .as_ref()
                    .map(|prepared| prepared.status.clone())
                    .unwrap_or_else(|| "not_required_in_process".to_string()),
                player_artifact_build_report_path: prepared_player
                    .as_ref()
                    .and_then(|prepared| prepared.build_report_path.clone()),
                player_artifact: prepared_player.and_then(|prepared| prepared.artifact),
                play_session_id: None,
                play_session_report_schema: None,
                duration_total_ms: elapsed_ms(total_started.elapsed()),
                diagnostics,
                next_actions: if failed {
                    vec!["delete_preview_cache_and_rebuild".to_string()]
                } else {
                    Vec::new()
                },
                deferred_flags: deferred_flags(),
                report_path: Some(report_path.display().to_string()),
            });
        }

        stages.push(stage(
            "autosave_dirty_documents",
            EditorPreviewPackageStageStatus::Success,
            std::time::Duration::from_millis(0),
            false,
            Some(initial_cache_status),
            dirty_domains.clone(),
            vec![EditorPreviewPackageDiagnostic::info(
                "editor.preview_package.autosave_owned_by_play_service",
                "The preview package service reads saved project files; editor document autosave is handled before prepare by PlayService when available.",
            )],
        ));

        let assemble_started = Instant::now();
        let assembly_request = ProjectRuntimePackageAssemblyRequest::new(&request.project_root)
            .with_build_profile_path(build_profile_path(
                &request.project_root,
                &request.build_profile,
            ))
            .with_artifact_cache_root(&artifact_cache_root);
        let assembly_result = ProjectRuntimePackageAssembler::assemble(assembly_request);
        let assembly_diagnostics = assembly_result
            .report
            .diagnostics
            .iter()
            .cloned()
            .map(EditorPreviewPackageDiagnostic::from)
            .collect::<Vec<_>>();
        diagnostics.extend(assembly_diagnostics.clone());
        let mut assembly_stage = stage(
            "assemble_project_runtime_package_input",
            if assembly_result.status == ProjectRuntimePackageAssemblyStatus::Success {
                EditorPreviewPackageStageStatus::Success
            } else {
                EditorPreviewPackageStageStatus::Failed
            },
            assemble_started.elapsed(),
            false,
            Some(initial_cache_status),
            dirty_domains.clone(),
            assembly_diagnostics,
        );
        assembly_stage.producer_reports = assembly_result.report.producer_reports.clone();
        stages.push(assembly_stage);
        if assembly_result.status == ProjectRuntimePackageAssemblyStatus::Failed {
            stages.push(stage(
                "build_runtime_package",
                EditorPreviewPackageStageStatus::Skipped,
                std::time::Duration::from_millis(0),
                true,
                Some(EditorPreviewPackageCacheStatus::Failed),
                dirty_domains.clone(),
                Vec::new(),
            ));
            return write_final_report(failed_rebuild_report(
                request,
                fingerprint,
                previous_fingerprint,
                previous_cache_key,
                cache_dir,
                runtime_package_dir,
                report_path,
                build_report_path,
                validation_report_path,
                diff_report_path,
                stages,
                diagnostics,
                total_started,
                "assembly_failed",
            ));
        }

        let runtime_input = assembly_result
            .build_input
            .expect("successful assembly should produce runtime package input");
        let active_scene_id = assembly_result
            .active_scene_id
            .unwrap_or_else(|| fingerprint.active_scene_id.clone());
        let build_started = Instant::now();
        let mut build_request =
            RuntimePackageBuildRequest::dev_desktop(&runtime_package_dir, active_scene_id.clone());
        if let Some(previous) = previous_manifest.as_ref() {
            build_request.previous_package_manifest = Some(PreviousPackageManifest {
                package_id: previous.cache_key.clone(),
                hash: previous.last_success_package_hash.clone(),
            });
        }
        let build_report = RuntimePackageBuilder::build(&build_request, &runtime_input);
        stages.push(stage(
            "build_runtime_package",
            if build_report.status == RuntimePackageBuildStatus::Success {
                EditorPreviewPackageStageStatus::Success
            } else {
                EditorPreviewPackageStageStatus::Failed
            },
            build_started.elapsed(),
            false,
            Some(initial_cache_status),
            dirty_domains.clone(),
            build_report
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    EditorPreviewPackageDiagnostic {
                        severity: match diagnostic.severity {
                            engine_runtime::runtime_package_builder::RuntimePackageDiagnosticSeverity::Info => {
                                EditorPreviewPackageDiagnosticSeverity::Info
                            }
                            engine_runtime::runtime_package_builder::RuntimePackageDiagnosticSeverity::Warning => {
                                EditorPreviewPackageDiagnosticSeverity::Warning
                            }
                            engine_runtime::runtime_package_builder::RuntimePackageDiagnosticSeverity::Error => {
                                EditorPreviewPackageDiagnosticSeverity::Error
                            }
                        },
                        code: format!("RuntimePackageBuilder::{}", diagnostic.code),
                        message: diagnostic.message.clone(),
                        path: diagnostic.path.clone(),
                        suggestion: diagnostic.suggestion.clone(),
                    }
                })
                .collect(),
        ));
        if build_report.status == RuntimePackageBuildStatus::Failed {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::error(
                    "editor.preview_package.runtime_package_build_failed",
                    "RuntimePackageBuilder failed while preparing the editor preview package.",
                )
                .with_path(build_report.outputs.package_dir.clone())
                .with_suggestion("Open build-runtime-package-report.json and fix the first error."),
            );
            return write_final_report(failed_rebuild_report(
                request,
                fingerprint,
                previous_fingerprint,
                previous_cache_key,
                cache_dir,
                runtime_package_dir,
                report_path,
                build_report_path,
                validation_report_path,
                diff_report_path,
                stages,
                diagnostics,
                total_started,
                "build_failed",
            ));
        }

        let copy_started = Instant::now();
        let copy_diagnostics =
            copy_runtime_assets(&request.project_root, &runtime_package_dir, &runtime_input);
        let copy_failed = copy_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == EditorPreviewPackageDiagnosticSeverity::Error);
        diagnostics.extend(copy_diagnostics.clone());
        stages.push(stage(
            "copy_runtime_assets",
            if copy_failed {
                EditorPreviewPackageStageStatus::Failed
            } else {
                EditorPreviewPackageStageStatus::Success
            },
            copy_started.elapsed(),
            false,
            Some(initial_cache_status),
            dirty_domains.clone(),
            copy_diagnostics,
        ));
        if copy_failed {
            return write_final_report(failed_rebuild_report(
                request,
                fingerprint,
                previous_fingerprint,
                previous_cache_key,
                cache_dir,
                runtime_package_dir,
                report_path,
                build_report_path,
                validation_report_path,
                diff_report_path,
                stages,
                diagnostics,
                total_started,
                "asset_copy_failed",
            ));
        }

        let load_started = Instant::now();
        let load = load_runtime_package(&runtime_package_dir);
        let load_diagnostics = load_diagnostics_to_preview(&load.diagnostics.issues);
        let load_failed = load.value.is_none();
        diagnostics.extend(load_diagnostics.clone());
        stages.push(stage(
            "load_validate_runtime_package",
            if load_failed {
                EditorPreviewPackageStageStatus::Failed
            } else {
                EditorPreviewPackageStageStatus::Success
            },
            load_started.elapsed(),
            false,
            Some(EditorPreviewPackageCacheStatus::Rebuilt),
            dirty_domains.clone(),
            load_diagnostics,
        ));
        if load_failed {
            return write_final_report(failed_rebuild_report(
                request,
                fingerprint,
                previous_fingerprint,
                previous_cache_key,
                cache_dir,
                runtime_package_dir,
                report_path,
                build_report_path,
                validation_report_path,
                diff_report_path,
                stages,
                diagnostics,
                total_started,
                "load_validate_failed",
            ));
        }

        let player_started = Instant::now();
        let prepared_player = request.prepare_player_artifact.then(|| {
            prepare_project_player_artifact(
                &request.project_root,
                request.player_artifact_build_root.as_deref(),
                load.value
                    .as_ref()
                    .expect("successful RuntimePackage load must return a value"),
            )
        });
        if let Some(prepared_player) = &prepared_player {
            diagnostics.extend(prepared_player.diagnostics.clone());
        }
        stages.push(stage(
            "build_project_runtime_player_artifact",
            prepared_player
                .as_ref()
                .map(|value| value.stage_status)
                .unwrap_or(EditorPreviewPackageStageStatus::Skipped),
            player_started.elapsed(),
            prepared_player.as_ref().is_none_or(|value| {
                value.stage_status == EditorPreviewPackageStageStatus::Skipped
            }),
            Some(EditorPreviewPackageCacheStatus::Rebuilt),
            dirty_domains.clone(),
            prepared_player
                .as_ref()
                .map(|value| value.diagnostics.clone())
                .unwrap_or_else(|| {
                    vec![EditorPreviewPackageDiagnostic::info(
                        "editor.preview_package.player_artifact_not_required_in_process",
                        "In-process Editor GameView uses the linked RuntimeModule and does not require a standalone Player artifact.",
                    )]
                }),
        ));
        if prepared_player
            .as_ref()
            .is_some_and(|value| value.stage_status == EditorPreviewPackageStageStatus::Failed)
        {
            return write_final_report(failed_rebuild_report_with_player(
                request,
                fingerprint,
                previous_fingerprint,
                previous_cache_key,
                cache_dir,
                runtime_package_dir,
                report_path,
                build_report_path,
                validation_report_path,
                diff_report_path,
                stages,
                diagnostics,
                total_started,
                prepared_player.expect("failed player preparation must exist"),
                "project_runtime_player_artifact_failed",
            ));
        }

        let manifest = EditorPreviewPackageCacheManifest {
            schema_version: EDITOR_PREVIEW_PACKAGE_CACHE_MANIFEST_SCHEMA_VERSION.to_string(),
            project_root: request.project_root.display().to_string(),
            active_scene_id: active_scene_id.clone(),
            build_profile: request.build_profile.clone(),
            cache_key: fingerprint.combined_hash.clone(),
            fingerprint: fingerprint.clone(),
            runtime_package_dir: runtime_package_dir.display().to_string(),
            build_report_path: build_report_path.display().to_string(),
            validation_report_path: validation_report_path.display().to_string(),
            diff_report_path: diff_report_path.display().to_string(),
            last_success_at: unix_seconds_string(),
            last_success_package_hash: fingerprint.combined_hash.clone(),
        };
        if let Err(error) = write_project_json(&request.project_root, &manifest_path, &manifest) {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::error(
                    "editor.preview_package.cache_manifest_write_failed",
                    format!("Failed to write preview cache manifest: {error}"),
                )
                .with_path(manifest_path.display().to_string()),
            );
        }

        write_final_report(EditorPlayPreviewPackageReport {
            schema_version: EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
            status: if diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == EditorPreviewPackageDiagnosticSeverity::Error
            }) {
                EditorPreviewPackageStatus::Failed
            } else {
                EditorPreviewPackageStatus::Success
            },
            project_root: request.project_root.display().to_string(),
            active_scene_id,
            build_profile: request.build_profile,
            cache_dir: cache_dir.display().to_string(),
            runtime_package_dir: Some(runtime_package_dir.display().to_string()),
            cache_status: if diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == EditorPreviewPackageDiagnosticSeverity::Error
            }) {
                EditorPreviewPackageCacheStatus::Failed
            } else {
                EditorPreviewPackageCacheStatus::Rebuilt
            },
            cache_key: fingerprint.combined_hash.clone(),
            previous_cache_key,
            dirty_domains,
            source_fingerprint: fingerprint,
            previous_fingerprint,
            autosave_summary: "saved_project_files_read_by_assembler".to_string(),
            stage_reports: stages,
            runtime_package_build_report_path: Some(build_report_path.display().to_string()),
            runtime_package_validation_report_path: Some(
                validation_report_path.display().to_string(),
            ),
            runtime_package_diff_report_path: Some(diff_report_path.display().to_string()),
            runtime_package_load_status: "success".to_string(),
            player_artifact_status: prepared_player
                .as_ref()
                .map(|value| value.status.clone())
                .unwrap_or_else(|| "not_required_in_process".to_string()),
            player_artifact_build_report_path: prepared_player
                .as_ref()
                .and_then(|value| value.build_report_path.clone()),
            player_artifact: prepared_player.and_then(|value| value.artifact),
            play_session_id: None,
            play_session_report_schema: None,
            duration_total_ms: elapsed_ms(total_started.elapsed()),
            diagnostics,
            next_actions: Vec::new(),
            deferred_flags: deferred_flags(),
            report_path: Some(report_path.display().to_string()),
        })
    }
}

struct PreparedProjectPlayerArtifact {
    stage_status: EditorPreviewPackageStageStatus,
    status: String,
    build_report_path: Option<String>,
    artifact: Option<ProjectPlayerArtifact>,
    diagnostics: Vec<EditorPreviewPackageDiagnostic>,
}

fn prepare_project_player_artifact(
    project_root: &Path,
    player_artifact_build_root: Option<&Path>,
    package: &engine_runtime::runtime_package::RuntimePackage,
) -> PreparedProjectPlayerArtifact {
    let manifest_path = project_root.join("project.aife.json");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            serde_json::from_str::<ProjectManifest>(&text).map_err(|error| error.to_string())
        });
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(error) => {
            return PreparedProjectPlayerArtifact {
                stage_status: EditorPreviewPackageStageStatus::Failed,
                status: "manifest_invalid".to_string(),
                build_report_path: None,
                artifact: None,
                diagnostics: vec![EditorPreviewPackageDiagnostic::error(
                    "editor.preview_package.player_artifact_manifest_invalid",
                    format!("Project manifest cannot select a Player artifact: {error}"),
                )
                .with_path(manifest_path.display().to_string())
                .with_suggestion("Repair project.aife.json and rebuild Preview.")],
            };
        }
    };
    if manifest.runtime_module.source_kind == Some(ProjectRuntimeSourceKind::BuiltInEmpty) {
        return PreparedProjectPlayerArtifact {
            stage_status: EditorPreviewPackageStageStatus::Skipped,
            status: "built_in_empty".to_string(),
            build_report_path: None,
            artifact: None,
            diagnostics: Vec::new(),
        };
    }
    if manifest.runtime_module.source_kind != Some(ProjectRuntimeSourceKind::ProjectRust) {
        return PreparedProjectPlayerArtifact {
            stage_status: EditorPreviewPackageStageStatus::Skipped,
            status: "legacy_static_linked".to_string(),
            build_report_path: None,
            artifact: None,
            diagnostics: Vec::new(),
        };
    }

    let mut build_request = ProjectRuntimePlayerArtifactBuildRequest::new(
        project_root,
        default_engine_sdk_root(),
        package.manifest.project.runtime_module.clone(),
    );
    if let Some(build_root) = player_artifact_build_root {
        build_request = build_request.with_build_root(build_root);
    }
    let build = ProjectPlayerArtifact::build_project_rust(build_request);
    let diagnostics = build
        .diagnostics
        .iter()
        .map(|diagnostic| {
            EditorPreviewPackageDiagnostic::error(
                format!("PlayerArtifact::{}", diagnostic.code),
                diagnostic.message.clone(),
            )
            .with_suggestion(diagnostic.next_action.clone())
        })
        .collect::<Vec<_>>();
    let passed = build.status == ProjectRuntimePlayerArtifactBuildStatus::Success
        && build.artifact.is_some();
    PreparedProjectPlayerArtifact {
        stage_status: if passed {
            EditorPreviewPackageStageStatus::Success
        } else {
            EditorPreviewPackageStageStatus::Failed
        },
        status: if passed {
            format!("success_{}", build.cache_status)
        } else {
            "failed".to_string()
        },
        build_report_path: build.build_report_path,
        artifact: build.artifact,
        diagnostics,
    }
}

#[allow(clippy::too_many_arguments)]
fn failed_rebuild_report_with_player(
    request: EditorPreviewPackageRequest,
    fingerprint: EditorPreviewPackageFingerprint,
    previous_fingerprint: Option<EditorPreviewPackageFingerprint>,
    previous_cache_key: Option<String>,
    cache_dir: PathBuf,
    runtime_package_dir: PathBuf,
    report_path: PathBuf,
    build_report_path: PathBuf,
    validation_report_path: PathBuf,
    diff_report_path: PathBuf,
    stages: Vec<EditorPreviewPackageStageReport>,
    diagnostics: Vec<EditorPreviewPackageDiagnostic>,
    total_started: Instant,
    prepared_player: PreparedProjectPlayerArtifact,
    next_action: &str,
) -> EditorPlayPreviewPackageReport {
    let mut report = failed_rebuild_report(
        request,
        fingerprint,
        previous_fingerprint,
        previous_cache_key,
        cache_dir,
        runtime_package_dir,
        report_path,
        build_report_path,
        validation_report_path,
        diff_report_path,
        stages,
        diagnostics,
        total_started,
        next_action,
    );
    report.player_artifact_status = prepared_player.status;
    report.player_artifact_build_report_path = prepared_player.build_report_path;
    report.player_artifact = prepared_player.artifact;
    report
}

fn failed_rebuild_report(
    request: EditorPreviewPackageRequest,
    fingerprint: EditorPreviewPackageFingerprint,
    previous_fingerprint: Option<EditorPreviewPackageFingerprint>,
    previous_cache_key: Option<String>,
    cache_dir: PathBuf,
    runtime_package_dir: PathBuf,
    report_path: PathBuf,
    build_report_path: PathBuf,
    validation_report_path: PathBuf,
    diff_report_path: PathBuf,
    stages: Vec<EditorPreviewPackageStageReport>,
    diagnostics: Vec<EditorPreviewPackageDiagnostic>,
    total_started: Instant,
    next_action: &str,
) -> EditorPlayPreviewPackageReport {
    EditorPlayPreviewPackageReport {
        schema_version: EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
        status: EditorPreviewPackageStatus::Failed,
        project_root: request.project_root.display().to_string(),
        active_scene_id: fingerprint.active_scene_id.clone(),
        build_profile: request.build_profile,
        cache_dir: cache_dir.display().to_string(),
        runtime_package_dir: Some(runtime_package_dir.display().to_string()),
        cache_status: EditorPreviewPackageCacheStatus::Failed,
        cache_key: fingerprint.combined_hash.clone(),
        previous_cache_key,
        dirty_domains: previous_fingerprint
            .as_ref()
            .map(|previous| fingerprint.dirty_domains_since(previous))
            .unwrap_or_default(),
        source_fingerprint: fingerprint,
        previous_fingerprint,
        autosave_summary: "saved_project_files_read_by_assembler".to_string(),
        stage_reports: stages,
        runtime_package_build_report_path: Some(build_report_path.display().to_string()),
        runtime_package_validation_report_path: Some(validation_report_path.display().to_string()),
        runtime_package_diff_report_path: Some(diff_report_path.display().to_string()),
        runtime_package_load_status: "failed".to_string(),
        player_artifact_status: "not_started".to_string(),
        player_artifact_build_report_path: None,
        player_artifact: None,
        play_session_id: None,
        play_session_report_schema: None,
        duration_total_ms: elapsed_ms(total_started.elapsed()),
        diagnostics,
        next_actions: vec![next_action.to_string()],
        deferred_flags: deferred_flags(),
        report_path: Some(report_path.display().to_string()),
    }
}

fn compute_fingerprint(
    request: &EditorPreviewPackageRequest,
    diagnostics: &mut Vec<EditorPreviewPackageDiagnostic>,
) -> EditorPreviewPackageFingerprint {
    let manifest = read_project_manifest(&request.project_root, diagnostics);
    let project_id = manifest
        .as_ref()
        .map(|manifest| manifest.project_id.clone())
        .unwrap_or_else(|| "unknown-project".to_string());
    let active_scene_id = request
        .active_scene_id
        .clone()
        .or_else(|| {
            manifest.as_ref().and_then(|manifest| {
                read_scene_id(&request.project_root.join(&manifest.default_scene)).ok()
            })
        })
        .unwrap_or_else(|| "unknown-scene".to_string());

    let project_hash = hash_domain(
        &request.project_root,
        &[PathBuf::from("project.aife.json")],
        diagnostics,
    );
    let build_profile_hash = hash_dir(&request.project_root, "BuildProfiles", diagnostics);
    let scene_hash = hash_dir(&request.project_root, "Scenes", diagnostics);
    let prefab_hash = hash_dir(&request.project_root, "Prefabs", diagnostics);
    let asset_hash = hash_dir(&request.project_root, "Assets", diagnostics);
    let rule_hash = hash_dir(&request.project_root, "Rules", diagnostics);
    let aui_hash = hash_dir(&request.project_root, "AUI", diagnostics);
    let input_hash = hash_dir(&request.project_root, "Input", diagnostics);
    let font_atlas_seed_hash = stable_hash(&format!(
        "{}|{}|{}",
        "aui-font-atlas-cmin", RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION, aui_hash
    ));
    let runtime_module_hash = hash_runtime_module_sources(&request.project_root, diagnostics);
    let engine_schema_hash = stable_hash(
        &[
            EDITOR_PREVIEW_PACKAGE_REQUEST_SCHEMA_VERSION,
            EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION,
            EDITOR_PREVIEW_PACKAGE_CACHE_MANIFEST_SCHEMA_VERSION,
            PROJECT_MANIFEST_SCHEMA_VERSION,
            BUILD_PROFILE_SCHEMA_VERSION,
            PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION,
            RUNTIME_PACKAGE_SCHEMA_VERSION,
            RUNTIME_SCENE_SCHEMA_VERSION,
            RUNTIME_ENTITY_SCHEMA_VERSION,
            RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION,
            RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
            RUNTIME_AUI_MANIFEST_SCHEMA_VERSION,
            RUNTIME_FONT_ATLAS_MANIFEST_SCHEMA_VERSION,
            RUNTIME_PACKAGE_BUILD_REQUEST_SCHEMA_VERSION,
            BUILD_RUNTIME_PACKAGE_REPORT_SCHEMA_VERSION,
            RUNTIME_PACKAGE_VALIDATION_REPORT_SCHEMA_VERSION,
            RUNTIME_PACKAGE_DIFF_REPORT_SCHEMA_VERSION,
        ]
        .join("|"),
    );

    let mut fingerprint = EditorPreviewPackageFingerprint {
        schema_version: "editor-preview-package-fingerprint.v1".to_string(),
        project_id,
        active_scene_id,
        build_profile: request.build_profile.clone(),
        engine_schema_hash,
        project_hash,
        build_profile_hash,
        scene_hash,
        prefab_hash,
        asset_hash,
        rule_hash,
        aui_hash,
        input_hash,
        font_atlas_seed_hash,
        runtime_module_hash,
        combined_hash: String::new(),
    };
    fingerprint.combined_hash = stable_hash(
        &[
            fingerprint.schema_version.clone(),
            fingerprint.project_id.clone(),
            fingerprint.active_scene_id.clone(),
            fingerprint.build_profile.clone(),
            fingerprint.engine_schema_hash.clone(),
            fingerprint.project_hash.clone(),
            fingerprint.build_profile_hash.clone(),
            fingerprint.scene_hash.clone(),
            fingerprint.prefab_hash.clone(),
            fingerprint.asset_hash.clone(),
            fingerprint.rule_hash.clone(),
            fingerprint.aui_hash.clone(),
            fingerprint.input_hash.clone(),
            fingerprint.font_atlas_seed_hash.clone(),
            fingerprint.runtime_module_hash.clone(),
        ]
        .join("|"),
    );
    fingerprint
}

fn read_project_manifest(
    project_root: &Path,
    diagnostics: &mut Vec<EditorPreviewPackageDiagnostic>,
) -> Option<ProjectManifest> {
    let path = project_root.join("project.aife.json");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::error(
                    "editor.preview_package.project_manifest_read_failed",
                    format!("Failed to read project manifest: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            return None;
        }
    };
    match serde_json::from_str::<ProjectManifest>(&text) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::error(
                    "editor.preview_package.project_manifest_parse_failed",
                    format!("Failed to parse project manifest: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            None
        }
    }
}

fn read_scene_id(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value =
        serde_json::from_str::<serde_json::Value>(&text).map_err(|error| error.to_string())?;
    value
        .get("id")
        .or_else(|| value.get("sceneId"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "scene id missing".to_string())
}

fn preview_cache_dir(project_root: &Path, build_profile: &str, active_scene_id: &str) -> PathBuf {
    project_root
        .join(".aife")
        .join("editor-preview")
        .join(sanitize_path_segment(build_profile))
        .join(sanitize_path_segment(active_scene_id))
}

fn build_profile_path(project_root: &Path, build_profile: &str) -> PathBuf {
    let file_name = match build_profile {
        "windows-dev" | "windows.dev" => "windows.dev.json".to_string(),
        other => format!("{}.json", other.replace('-', ".")),
    };
    project_root.join("BuildProfiles").join(file_name)
}

fn sanitize_path_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "default".to_string()
    } else {
        sanitized
    }
}

fn hash_dir(
    project_root: &Path,
    relative_dir: &str,
    diagnostics: &mut Vec<EditorPreviewPackageDiagnostic>,
) -> String {
    let dir = project_root.join(relative_dir);
    let mut paths = Vec::new();
    collect_files_recursive(&dir, &mut paths, diagnostics);
    let relative_paths = paths
        .iter()
        .filter_map(|path| path.strip_prefix(project_root).ok())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    hash_domain(project_root, &relative_paths, diagnostics)
}

fn hash_runtime_module_sources(
    project_root: &Path,
    diagnostics: &mut Vec<EditorPreviewPackageDiagnostic>,
) -> String {
    let runtime_root = project_root.join("RuntimeModule");
    if !runtime_root.exists() {
        return stable_hash("runtime-module-source-empty");
    }
    match CandidateProjectRevisionStore::project_digest(&runtime_root) {
        Ok(digest) => digest,
        Err(error) => {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::error(
                    "editor.preview_package.runtime_module_fingerprint_failed",
                    format!("RuntimeModule source fingerprint failed: {error}"),
                )
                .with_path(runtime_root.display().to_string())
                .with_suggestion(error.next_action),
            );
            stable_hash("runtime-module-fingerprint-failed")
        }
    }
}

fn collect_files_recursive(
    path: &Path,
    paths: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<EditorPreviewPackageDiagnostic>,
) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                diagnostics.push(EditorPreviewPackageDiagnostic::warning(
                    "editor.preview_package.fingerprint_dir_entry_failed",
                    format!("Failed to read directory entry: {error}"),
                ));
                continue;
            }
        };
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_files_recursive(&entry_path, paths, diagnostics);
        } else if entry_path.is_file() {
            paths.push(entry_path);
        }
    }
}

fn hash_domain(
    project_root: &Path,
    relative_paths: &[PathBuf],
    diagnostics: &mut Vec<EditorPreviewPackageDiagnostic>,
) -> String {
    let mut files = relative_paths.to_vec();
    files.sort();
    let mut parts = Vec::new();
    for relative_path in files {
        let full_path = project_root.join(&relative_path);
        let normalized = normalize_relative_path(&relative_path);
        match fs::read(&full_path) {
            Ok(bytes) => {
                parts.push(normalized);
                parts.push(stable_hash_bytes(&bytes));
            }
            Err(error) if full_path.exists() => {
                diagnostics.push(
                    EditorPreviewPackageDiagnostic::error(
                        "editor.preview_package.fingerprint_file_read_failed",
                        format!("Failed to read fingerprint input: {error}"),
                    )
                    .with_path(full_path.display().to_string()),
                );
            }
            Err(_) => {
                parts.push(format!("{normalized}:missing"));
            }
        }
    }
    stable_hash(&parts.join("|"))
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/")
}

fn read_cache_manifest(
    path: &Path,
    diagnostics: &mut Vec<EditorPreviewPackageDiagnostic>,
) -> Option<EditorPreviewPackageCacheManifest> {
    if !path.exists() {
        return None;
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::warning(
                    "editor.preview_package.cache_manifest_read_failed",
                    format!("Failed to read preview cache manifest: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            return None;
        }
    };
    match serde_json::from_str::<EditorPreviewPackageCacheManifest>(&text) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::warning(
                    "editor.preview_package.cache_manifest_parse_failed",
                    format!("Failed to parse preview cache manifest: {error}"),
                )
                .with_path(path.display().to_string()),
            );
            None
        }
    }
}

fn copy_runtime_assets(
    project_root: &Path,
    runtime_package_dir: &Path,
    input: &RuntimePackageBuildInput,
) -> Vec<EditorPreviewPackageDiagnostic> {
    let mut diagnostics = Vec::new();
    let scope = match crate::ProjectWriteScope::open(project_root) {
        Ok(scope) => scope,
        Err(error) => {
            return vec![EditorPreviewPackageDiagnostic::error(
                error.code,
                format!("Failed to open preview project write scope: {error}"),
            )];
        }
    };
    for asset in &input.assets {
        let source = project_root.join(&asset.source);
        let destination = runtime_package_dir.join(&asset.runtime_uri);
        let relative = match destination.strip_prefix(project_root) {
            Ok(relative) => relative,
            Err(error) => {
                diagnostics.push(
                    EditorPreviewPackageDiagnostic::error(
                        "project_write.path_not_relative",
                        format!("Runtime asset destination escaped project root: {error}"),
                    )
                    .with_path(destination.display().to_string()),
                );
                continue;
            }
        };
        if asset.asset_type == "scene" && scope.try_exists(relative).unwrap_or(false) {
            continue;
        }
        let bytes = if source.exists() {
            match fs::read(&source) {
                Ok(bytes) => bytes,
                Err(error) => {
                    diagnostics.push(
                        EditorPreviewPackageDiagnostic::error(
                            "editor.preview_package.asset_copy_failed",
                            format!("Failed to read runtime asset {}: {error}", asset.asset_id),
                        )
                        .with_path(source.display().to_string()),
                    );
                    continue;
                }
            }
        } else {
            Vec::new()
        };
        if let Err(error) = scope.write_atomic(relative, &bytes) {
            diagnostics.push(
                EditorPreviewPackageDiagnostic::error(
                    "editor.preview_package.asset_placeholder_write_failed",
                    format!("Failed to write runtime asset {}: {error}", asset.asset_id),
                )
                .with_path(destination.display().to_string()),
            );
        }
    }
    diagnostics
}

fn load_diagnostics_to_preview(
    issues: &[engine_runtime::diagnostics::RuntimeDiagnostic],
) -> Vec<EditorPreviewPackageDiagnostic> {
    issues
        .iter()
        .map(|issue| EditorPreviewPackageDiagnostic {
            severity: match issue.severity {
                engine_runtime::diagnostics::DiagnosticSeverity::Error => {
                    EditorPreviewPackageDiagnosticSeverity::Error
                }
                engine_runtime::diagnostics::DiagnosticSeverity::Warning => {
                    EditorPreviewPackageDiagnosticSeverity::Warning
                }
            },
            code: "editor.preview_package.runtime_package_load_diagnostic".to_string(),
            message: issue.message.clone(),
            path: Some(issue.path.clone()),
            suggestion: Some("Fix the generated RuntimePackage input.".to_string()),
        })
        .collect()
}

fn stage(
    stage_id: impl Into<String>,
    status: EditorPreviewPackageStageStatus,
    duration: std::time::Duration,
    skipped: bool,
    cache_status: Option<EditorPreviewPackageCacheStatus>,
    dirty_domains: Vec<EditorPreviewPackageDirtyDomain>,
    diagnostics: Vec<EditorPreviewPackageDiagnostic>,
) -> EditorPreviewPackageStageReport {
    EditorPreviewPackageStageReport {
        stage_id: stage_id.into(),
        status,
        duration_ms: elapsed_ms(duration),
        skipped,
        cache_status,
        dirty_domains,
        producer_reports: Vec::new(),
        diagnostics,
    }
}

fn write_final_report(
    mut report: EditorPlayPreviewPackageReport,
) -> EditorPlayPreviewPackageReport {
    let Some(report_path) = report.report_path.clone() else {
        return report;
    };
    let path = PathBuf::from(&report_path);
    let project_root = PathBuf::from(&report.project_root);
    if let Err(error) = write_project_json(&project_root, &path, &report) {
        report.status = EditorPreviewPackageStatus::Failed;
        report.cache_status = EditorPreviewPackageCacheStatus::Failed;
        report.diagnostics.push(
            EditorPreviewPackageDiagnostic::error(
                "editor.preview_package.report_write_failed",
                format!("Failed to write editor preview package report: {error}"),
            )
            .with_path(report_path),
        );
        let _ = write_project_json(&project_root, &path, &report);
    }
    report
}

fn write_project_json<T: Serialize>(
    project_root: &Path,
    path: &Path,
    value: &T,
) -> std::io::Result<()> {
    let relative = path.strip_prefix(project_root).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "preview report path is outside project root",
        )
    })?;
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let scope = crate::ProjectWriteScope::open(project_root)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    scope
        .write_atomic(relative, text.as_bytes())
        .map(|_| ())
        .map_err(|error| std::io::Error::other(error.to_string()))
}

fn elapsed_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn stable_hash(value: &str) -> String {
    stable_hash_bytes(value.as_bytes())
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn unix_seconds_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn deferred_flags() -> Vec<String> {
    vec![
        "true_incremental_asset_cook_deferred=true".to_string(),
        "async_background_preview_build_deferred=true".to_string(),
        "run_last_good_button_deferred=true".to_string(),
        "unsaved_memory_snapshot_preview_deferred=true".to_string(),
        "embedded_game_view_deferred=true".to_string(),
        "multi_instance_play_deferred=true".to_string(),
        "remote_device_play_deferred=true".to_string(),
        "rust_aot_hot_compile_on_play_deferred=true".to_string(),
        "windowed_game_view_play_runner_deferred=true".to_string(),
        "aui_input_snapshot_runner_unification_deferred=true".to_string(),
    ]
}

#[allow(dead_code)]
fn _build_profile_summary(profile: Option<&BuildProfile>) -> String {
    profile
        .map(|profile| {
            format!(
                "profile={} target={} frame_limit={}",
                profile.profile, profile.target, profile.frame_limit
            )
        })
        .unwrap_or_else(|| "profile=default".to_string())
}

#[allow(dead_code)]
fn _domain_hash_debug_map(
    fingerprint: &EditorPreviewPackageFingerprint,
) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("project", fingerprint.project_hash.clone()),
        ("buildProfile", fingerprint.build_profile_hash.clone()),
        ("scene", fingerprint.scene_hash.clone()),
        ("prefab", fingerprint.prefab_hash.clone()),
        ("asset", fingerprint.asset_hash.clone()),
        ("rule", fingerprint.rule_hash.clone()),
        ("aui", fingerprint.aui_hash.clone()),
        ("input", fingerprint.input_hash.clone()),
        ("fontAtlas", fingerprint.font_atlas_seed_hash.clone()),
        ("engineSchema", fingerprint.engine_schema_hash.clone()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn editor_preview_package_schema_serializes() {
        let request = EditorPreviewPackageRequest::editor_play("project");
        let mut diagnostics = Vec::new();
        let fingerprint = compute_fingerprint(&request, &mut diagnostics);
        let report = EditorPlayPreviewPackageReport {
            schema_version: EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION.to_string(),
            status: EditorPreviewPackageStatus::Success,
            project_root: "project".to_string(),
            active_scene_id: fingerprint.active_scene_id.clone(),
            build_profile: "windows-dev".to_string(),
            cache_dir: "cache".to_string(),
            runtime_package_dir: Some("cache/runtime_package".to_string()),
            cache_status: EditorPreviewPackageCacheStatus::Hit,
            cache_key: fingerprint.combined_hash.clone(),
            previous_cache_key: Some(fingerprint.combined_hash.clone()),
            dirty_domains: Vec::new(),
            source_fingerprint: fingerprint,
            previous_fingerprint: None,
            autosave_summary: "none".to_string(),
            stage_reports: Vec::new(),
            runtime_package_build_report_path: None,
            runtime_package_validation_report_path: None,
            runtime_package_diff_report_path: None,
            runtime_package_load_status: "success".to_string(),
            player_artifact_status: "not_started".to_string(),
            player_artifact_build_report_path: None,
            player_artifact: None,
            play_session_id: None,
            play_session_report_schema: None,
            duration_total_ms: 0,
            diagnostics,
            next_actions: Vec::new(),
            deferred_flags: deferred_flags(),
            report_path: None,
        };

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains(EDITOR_PLAY_PREVIEW_PACKAGE_REPORT_SCHEMA_VERSION));
        assert!(json.contains("windowed_game_view_play_runner_deferred=true"));
    }

    #[test]
    fn editor_preview_package_cache_rebuilds_then_hits() {
        let project_root = copy_sample_project("preview-cache-hit");
        let first = EditorPreviewPackageService::prepare(EditorPreviewPackageRequest::editor_play(
            &project_root,
        ));
        assert_eq!(first.status, EditorPreviewPackageStatus::Success);
        assert_eq!(first.cache_status, EditorPreviewPackageCacheStatus::Rebuilt);

        let second = EditorPreviewPackageService::prepare(
            EditorPreviewPackageRequest::editor_play(&project_root),
        );

        assert_eq!(second.status, EditorPreviewPackageStatus::Success);
        assert_eq!(second.cache_status, EditorPreviewPackageCacheStatus::Hit);
        assert!(second
            .stage_reports
            .iter()
            .any(|stage| { stage.stage_id == "build_runtime_package" && stage.skipped }));
    }

    #[test]
    fn editor_preview_package_prepare_detects_scene_dirty_domain() {
        let project_root = copy_sample_project("preview-scene-dirty");
        let _ = EditorPreviewPackageService::prepare(EditorPreviewPackageRequest::editor_play(
            &project_root,
        ));
        let scene_path = project_root.join("Scenes").join("Main.scene.json");
        let mut text = fs::read_to_string(&scene_path).unwrap();
        text.push_str("\n");
        fs::write(&scene_path, text).unwrap();

        let report = EditorPreviewPackageService::prepare(
            EditorPreviewPackageRequest::editor_play(&project_root),
        );

        assert_eq!(report.status, EditorPreviewPackageStatus::Success);
        assert_eq!(
            report.cache_status,
            EditorPreviewPackageCacheStatus::Rebuilt
        );
        assert!(report
            .dirty_domains
            .contains(&EditorPreviewPackageDirtyDomain::Scene));
    }

    #[test]
    fn editor_preview_package_runtime_module_fingerprint_excludes_cargo_targets() {
        let project_root = copy_sample_project("preview-runtime-target-excluded");
        let request = EditorPreviewPackageRequest::editor_play(&project_root);
        let mut diagnostics = Vec::new();
        let before = compute_fingerprint(&request, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let target_file = project_root
            .join("RuntimeModule")
            .join("target")
            .join("debug")
            .join("deps")
            .join("generated.rlib");
        fs::create_dir_all(target_file.parent().unwrap()).unwrap();
        fs::write(&target_file, vec![0x5a; 1024 * 1024]).unwrap();
        let after_target = compute_fingerprint(&request, &mut diagnostics);
        assert_eq!(before.runtime_module_hash, after_target.runtime_module_hash);
        assert_eq!(before.combined_hash, after_target.combined_hash);

        let source_file = project_root
            .join("RuntimeModule")
            .join("src")
            .join("lib.rs");
        let mut source = fs::read_to_string(&source_file).unwrap();
        source.push_str("\n// fingerprint source change\n");
        fs::write(source_file, source).unwrap();
        let after_source = compute_fingerprint(&request, &mut diagnostics);
        assert_ne!(before.runtime_module_hash, after_source.runtime_module_hash);
        assert_ne!(before.combined_hash, after_source.combined_hash);
    }

    fn copy_sample_project(name: &str) -> PathBuf {
        let source = workspace_root()
            .join("samples")
            .join("complex_shooter_project");
        let destination = unique_temp_dir(name);
        copy_dir_recursive(&source, &destination);
        destination
    }

    fn copy_dir_recursive(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap().flatten() {
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                if matches!(
                    entry.file_name().to_string_lossy().as_ref(),
                    "Build" | ".aife"
                ) {
                    continue;
                }
                copy_dir_recursive(&source_path, &destination_path);
            } else {
                copy_file_with_retry(&source_path, &destination_path).unwrap();
            }
        }
    }

    fn copy_file_with_retry(source: &Path, destination: &Path) -> std::io::Result<u64> {
        let mut last_error = None;
        for _ in 0..5 {
            match fs::copy(source, destination) {
                Ok(bytes) => return Ok(bytes),
                Err(error) => {
                    last_error = Some(error);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
        Err(last_error.expect("copy should have produced an error"))
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
        std::env::temp_dir().join(format!("editor-preview-package-{name}-{stamp}"))
    }
}
