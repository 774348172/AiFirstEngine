use editor_ui_model::RecentProjectEntry;
use engine_input::InputMappingAsset;
use engine_runtime::game_view_presentation::GameViewTargetSpec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{ProjectRelativePath, ProjectWriteScope};

pub const LEGACY_PROJECT_MANIFEST_SCHEMA_VERSION: &str = "aife-project.v1";
pub const PROJECT_MANIFEST_SCHEMA_VERSION: &str = "aife-project.v2";
pub const PROJECT_RUNTIME_MODULE_INTERFACE_VERSION: &str = "project-runtime-module.v2";
pub const PROJECT_SETTINGS_SCHEMA_VERSION: &str = "aife-project-settings.v1";
pub const PROJECT_LAUNCHER_EVENT_SCHEMA_VERSION: &str = "project-launcher-event.v1";
pub const EDITOR_RECENT_PROJECTS_SCHEMA_VERSION: &str = "editor-recent-projects.v1";
pub const PROJECT_TEMPLATE_REGISTRY_SCHEMA_VERSION: &str = "project-template-registry.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSettingsDocument {
    pub schema_version: String,
    pub project_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_preview: Option<ProjectEditorPreviewSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEditorPreviewSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_view_target: Option<GameViewTargetSpec>,
}

impl ProjectSettingsDocument {
    pub fn new(project_name: impl Into<String>) -> Self {
        Self {
            schema_version: PROJECT_SETTINGS_SCHEMA_VERSION.to_string(),
            project_name: project_name.into(),
            editor_preview: None,
        }
    }

    pub fn preferred_game_view_target(&self) -> Option<GameViewTargetSpec> {
        self.editor_preview
            .as_ref()
            .and_then(|preview| preview.game_view_target)
    }

    pub fn resolved_game_view_target(&self) -> GameViewTargetSpec {
        self.preferred_game_view_target().unwrap_or_default()
    }

    fn validate(&self, manifest: &ProjectManifest) -> Result<(), String> {
        if self.schema_version != PROJECT_SETTINGS_SCHEMA_VERSION
            || self.schema_version != manifest.settings_version
        {
            return Err(format!(
                "project_settings.schema_version_mismatch: expected={}, manifest={}, actual={}",
                PROJECT_SETTINGS_SCHEMA_VERSION, manifest.settings_version, self.schema_version
            ));
        }
        if self.project_name != manifest.project_name {
            return Err(format!(
                "project_settings.project_name_mismatch: manifest={}, settings={}",
                manifest.project_name, self.project_name
            ));
        }
        if let Some(target) = self.preferred_game_view_target() {
            target.validate().map_err(|error| {
                format!("project_settings.game_view_target_invalid: {}", error.code)
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "engineVersion")]
    pub engine_version: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "lastOpenedAt")]
    pub last_opened_at: Option<String>,
    #[serde(rename = "defaultScene")]
    pub default_scene: String,
    #[serde(rename = "assetRoot")]
    pub asset_root: String,
    #[serde(rename = "settingsVersion")]
    pub settings_version: String,
    #[serde(rename = "runtimeModule")]
    pub runtime_module: ProjectRuntimeModuleBuildSpec,
    #[serde(
        rename = "observationContract",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub observation_contract: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRuntimeModuleBuildSpec {
    #[serde(
        rename = "sourceKind",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub source_kind: Option<ProjectRuntimeSourceKind>,
    pub module_id: String,
    pub interface_version: String,
    pub cargo_manifest: String,
    pub cargo_package: String,
    pub player_binary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRuntimeSourceKind {
    BuiltInEmpty,
    ProjectRust,
}

impl ProjectRuntimeModuleBuildSpec {
    pub fn explicit_empty() -> Self {
        Self {
            source_kind: Some(ProjectRuntimeSourceKind::BuiltInEmpty),
            module_id: "engine.empty.runtime".to_string(),
            interface_version: PROJECT_RUNTIME_MODULE_INTERFACE_VERSION.to_string(),
            cargo_manifest: "RuntimeModule/Cargo.toml".to_string(),
            cargo_package: "empty_project_runtime".to_string(),
            player_binary: "empty_project_player".to_string(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.module_id.trim().is_empty()
            || self.interface_version.trim().is_empty()
            || self.cargo_package.trim().is_empty()
            || self.player_binary.trim().is_empty()
        {
            return Err(
                "project_runtime.project_manifest_runtime_module_fields_required".to_string(),
            );
        }
        match self.resolved_source_kind() {
            ProjectRuntimeSourceKind::BuiltInEmpty => {
                if self.module_id != "engine.empty.runtime" {
                    return Err("project_runtime.builtin_empty_module_id_mismatch".to_string());
                }
            }
            ProjectRuntimeSourceKind::ProjectRust => {
                if self.module_id == "engine.empty.runtime" {
                    return Err(
                        "project_runtime.project_rust_cannot_use_empty_module_id".to_string()
                    );
                }
                ProjectRelativePath::parse(&self.cargo_manifest).map_err(|error| {
                    format!("project_runtime.invalid_cargo_manifest_path: {error}")
                })?;
            }
        }
        Ok(())
    }

    pub fn resolved_source_kind(&self) -> ProjectRuntimeSourceKind {
        self.source_kind.unwrap_or_else(|| {
            if self.module_id == "engine.empty.runtime" {
                ProjectRuntimeSourceKind::BuiltInEmpty
            } else {
                ProjectRuntimeSourceKind::ProjectRust
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectLauncherEventKind {
    OpenProject,
    CreateProject,
    SelectRecentProject,
    RefreshRecentProjects,
    ValidateProject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectLauncherEventResult {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectLauncherEvent {
    pub schema_version: String,
    pub kind: ProjectLauncherEventKind,
    pub project_path: Option<String>,
    pub result: ProjectLauncherEventResult,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectSession {
    pub project_root: PathBuf,
    pub manifest: ProjectManifest,
    pub settings: ProjectSettingsDocument,
    write_scope: ProjectWriteScope,
}

impl ProjectSession {
    pub fn write_scope(&self) -> &ProjectWriteScope {
        &self.write_scope
    }
}

impl PartialEq for ProjectSession {
    fn eq(&self, other: &Self) -> bool {
        self.project_root == other.project_root
            && self.manifest == other.manifest
            && self.settings == other.settings
    }
}

impl Eq for ProjectSession {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecentProjectsDocument {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "recentProjects")]
    pub recent_projects: Vec<RecentProjectEntry>,
}

impl ProjectRecentProjectsDocument {
    pub fn new(recent_projects: Vec<RecentProjectEntry>) -> Self {
        Self {
            schema_version: EDITOR_RECENT_PROJECTS_SCHEMA_VERSION.to_string(),
            recent_projects,
        }
    }
}

pub struct ProjectRecentStore;

impl ProjectRecentStore {
    pub fn load(path: impl AsRef<Path>) -> Result<ProjectRecentProjectsDocument, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(ProjectRecentProjectsDocument::new(Vec::new()));
        }
        let text = fs::read_to_string(path)
            .map_err(|err| format!("Failed to read recent projects {}: {err}", path.display()))?;
        let document: ProjectRecentProjectsDocument =
            serde_json::from_str(&text).map_err(|err| err.to_string())?;
        if document.schema_version != EDITOR_RECENT_PROJECTS_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported recent projects schema: {}",
                document.schema_version
            ));
        }
        Ok(document)
    }

    pub fn save(
        path: impl AsRef<Path>,
        document: &ProjectRecentProjectsDocument,
    ) -> Result<(), String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        write_json_pretty(path, document)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectValidationStatus {
    Ready,
    Missing,
    Incomplete,
    InvalidManifest,
    UnsupportedVersion,
}

impl ProjectValidationStatus {
    pub fn as_status_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Incomplete => "incomplete",
            Self::InvalidManifest => "invalid_manifest",
            Self::UnsupportedVersion => "unsupported_version",
        }
    }

    pub fn valid(self) -> bool {
        self == Self::Ready
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTemplateDescriptor {
    pub template_id: String,
    pub label: String,
    pub description: String,
    pub default_scene: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTemplateRegistry {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub templates: Vec<ProjectTemplateDescriptor>,
}

impl Default for ProjectTemplateRegistry {
    fn default() -> Self {
        Self::c_min()
    }
}

impl ProjectTemplateRegistry {
    pub fn c_min() -> Self {
        Self {
            schema_version: PROJECT_TEMPLATE_REGISTRY_SCHEMA_VERSION.to_string(),
            templates: vec![ProjectTemplateDescriptor {
                template_id: "empty_project".to_string(),
                label: "Empty Project".to_string(),
                description: "Minimum AI First Engine project skeleton.".to_string(),
                default_scene: "Scenes/Main.scene.json".to_string(),
            }],
        }
    }

    pub fn default_template(&self) -> Option<&ProjectTemplateDescriptor> {
        self.templates.first()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectLauncherState {
    pub recent_projects: Vec<RecentProjectEntry>,
    pub selected_project_path: Option<String>,
    pub events: Vec<ProjectLauncherEvent>,
    pub engine_version: String,
    pub template_registry: ProjectTemplateRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCreateCleanupOutcome {
    NotRequired,
    RemovedOwnedTarget,
    CleanupFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCreateError {
    pub code: String,
    pub message: String,
    pub cleanup_outcome: ProjectCreateCleanupOutcome,
}

impl std::fmt::Display for ProjectCreateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

#[derive(Debug)]
pub struct ProjectCreateOwnedOutcome {
    pub requested_project_root: PathBuf,
    pub canonical_project_root: PathBuf,
    pub session: ProjectSession,
    pub cleanup_outcome: ProjectCreateCleanupOutcome,
}

const PROJECT_CREATE_CLAIM_FILE: &str = ".aife-project-create-claim";

impl Default for ProjectLauncherState {
    fn default() -> Self {
        Self::new("0.0.3")
    }
}

impl ProjectLauncherState {
    pub fn new(engine_version: impl Into<String>) -> Self {
        Self {
            recent_projects: Vec::new(),
            selected_project_path: None,
            events: Vec::new(),
            engine_version: engine_version.into(),
            template_registry: ProjectTemplateRegistry::c_min(),
        }
    }

    pub fn apply_recent_projects(&mut self, recent_projects: Vec<RecentProjectEntry>) {
        self.recent_projects = normalize_recent_project_entries(recent_projects);
        self.refresh_recent_projects();
    }

    pub fn load_recent_projects(&mut self, path: impl AsRef<Path>) -> Result<bool, String> {
        match ProjectRecentStore::load(path.as_ref()) {
            Ok(document) => {
                let stored_projects = document.recent_projects.clone();
                self.apply_recent_projects(document.recent_projects);
                Ok(self.recent_projects != stored_projects)
            }
            Err(message) => {
                self.record_event(
                    ProjectLauncherEventKind::RefreshRecentProjects,
                    Some(path.as_ref()),
                    ProjectLauncherEventResult::Failed,
                    vec![message.clone()],
                );
                Err(message)
            }
        }
    }

    pub fn save_recent_projects(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let document = ProjectRecentProjectsDocument::new(self.recent_projects.clone());
        match ProjectRecentStore::save(path.as_ref(), &document) {
            Ok(()) => Ok(()),
            Err(message) => {
                self.record_event(
                    ProjectLauncherEventKind::RefreshRecentProjects,
                    Some(path.as_ref()),
                    ProjectLauncherEventResult::Failed,
                    vec![message.clone()],
                );
                Err(message)
            }
        }
    }

    pub fn create_project(
        &mut self,
        project_root: impl AsRef<Path>,
        project_name: impl Into<String>,
    ) -> Result<ProjectSession, String> {
        self.create_project_owned(project_root, project_name)
            .map(|outcome| outcome.session)
            .map_err(|error| error.to_string())
    }

    pub fn create_project_owned(
        &mut self,
        project_root: impl AsRef<Path>,
        project_name: impl Into<String>,
    ) -> Result<ProjectCreateOwnedOutcome, ProjectCreateError> {
        let project_root = project_root.as_ref();
        let project_name = project_name.into();
        let requested_project_root = project_root.to_path_buf();
        let canonical_project_root =
            match validate_project_create_request(project_root, &project_name) {
                Ok(root) => root,
                Err(error) => {
                    self.record_create_failure(project_root, &error);
                    return Err(error);
                }
            };

        let template = self
            .template_registry
            .default_template()
            .cloned()
            .ok_or_else(|| {
                project_create_error(
                    "project_create.template_missing",
                    "No project template is registered.",
                    ProjectCreateCleanupOutcome::NotRequired,
                )
            })?;
        // Selecting the root is the one ambient-authority boundary. All child
        // project writes use the capability opened immediately afterwards.
        if let Err(error) = fs::create_dir(&canonical_project_root) {
            let code = if error.kind() == std::io::ErrorKind::AlreadyExists {
                "project_create.target_exists"
            } else {
                "project_create.target_claim_failed"
            };
            let error = project_create_error(
                code,
                format!("Could not exclusively claim the project target: {error}"),
                ProjectCreateCleanupOutcome::NotRequired,
            );
            self.record_create_failure(project_root, &error);
            return Err(error);
        }
        let claim_token = format!(
            "{}:{}",
            stable_project_stamp(&canonical_project_root),
            timestamp_string()
        );
        if let Err(error) = fs::write(
            canonical_project_root.join(PROJECT_CREATE_CLAIM_FILE),
            claim_token.as_bytes(),
        ) {
            let cleanup_outcome = if fs::remove_dir(&canonical_project_root).is_ok() {
                ProjectCreateCleanupOutcome::RemovedOwnedTarget
            } else {
                ProjectCreateCleanupOutcome::CleanupFailed
            };
            let error = project_create_error(
                "project_create.target_claim_failed",
                format!("Could not seal the exclusive project target claim: {error}"),
                cleanup_outcome,
            );
            self.record_create_failure(project_root, &error);
            return Err(error);
        }
        let build_result = (|| -> Result<ProjectSession, String> {
            let write_scope =
                ProjectWriteScope::open(&canonical_project_root).map_err(|err| err.to_string())?;
            for directory in [
                "Assets", "Scenes", "Packages", "Settings", "Library", "Input",
            ] {
                write_scope
                    .create_dir_all(directory)
                    .map_err(|err| err.to_string())?;
            }

            let now = timestamp_string();
            let manifest = ProjectManifest {
                schema_version: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
                project_id: format!("project-{}", stable_project_stamp(&canonical_project_root)),
                project_name: project_name.clone(),
                engine_version: self.engine_version.clone(),
                created_at: now.clone(),
                last_opened_at: Some(now),
                default_scene: template.default_scene,
                asset_root: "Assets".to_string(),
                settings_version: PROJECT_SETTINGS_SCHEMA_VERSION.to_string(),
                runtime_module: ProjectRuntimeModuleBuildSpec::explicit_empty(),
                observation_contract: None,
            };
            write_project_json_pretty(&write_scope, "project.aife.json", &manifest)?;
            let settings = ProjectSettingsDocument::new(project_name.clone());
            write_project_json_pretty(&write_scope, "Settings/project_settings.json", &settings)?;
            write_project_json_pretty(
                &write_scope,
                "Input/input.none.json",
                &InputMappingAsset::explicit_empty("input.none"),
            )?;
            write_default_scene(&write_scope, &manifest.default_scene)?;
            fs::remove_file(canonical_project_root.join(PROJECT_CREATE_CLAIM_FILE))
                .map_err(|error| format!("Could not release project create claim: {error}"))?;

            Ok(ProjectSession {
                project_root: canonical_project_root.clone(),
                manifest,
                settings,
                write_scope,
            })
        })();
        let session = match build_result {
            Ok(session) => session,
            Err(message) => {
                let cleanup_outcome =
                    cleanup_owned_project_target(&canonical_project_root, &claim_token);
                let code = if cleanup_outcome == ProjectCreateCleanupOutcome::CleanupFailed {
                    "project_create.cleanup_failed"
                } else {
                    "project_create.initialize_failed"
                };
                let error = project_create_error(code, message, cleanup_outcome);
                self.record_create_failure(project_root, &error);
                return Err(error);
            }
        };

        self.add_recent_project(&session);
        self.record_event(
            ProjectLauncherEventKind::CreateProject,
            Some(&canonical_project_root),
            ProjectLauncherEventResult::Succeeded,
            Vec::new(),
        );
        Ok(ProjectCreateOwnedOutcome {
            requested_project_root,
            canonical_project_root,
            session,
            cleanup_outcome: ProjectCreateCleanupOutcome::NotRequired,
        })
    }

    pub fn open_project(
        &mut self,
        project_root: impl AsRef<Path>,
    ) -> Result<ProjectSession, String> {
        let project_root = project_root.as_ref();
        if project_root.as_os_str().is_empty() {
            let message = "Project path is empty. A folder must be selected.".to_string();
            self.record_event(
                ProjectLauncherEventKind::OpenProject,
                None,
                ProjectLauncherEventResult::Failed,
                vec![message.clone()],
            );
            return Err(message);
        }
        let write_scope = ProjectWriteScope::open(project_root).map_err(|err| err.to_string())?;
        let manifest_text = String::from_utf8(
            write_scope
                .read("project.aife.json")
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("Project manifest is not UTF-8: {err}"))?;
        let manifest_value: serde_json::Value =
            serde_json::from_str(&manifest_text).map_err(|err| err.to_string())?;
        if manifest_value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str)
            == Some(LEGACY_PROJECT_MANIFEST_SCHEMA_VERSION)
        {
            let message = "project_runtime.project_manifest_v1_migration_required: aife-project.v1 has no required runtimeModule build spec; migrate it explicitly before opening."
                .to_string();
            self.record_event(
                ProjectLauncherEventKind::OpenProject,
                Some(project_root),
                ProjectLauncherEventResult::Failed,
                vec![message.clone()],
            );
            return Err(message);
        }
        let manifest: ProjectManifest =
            serde_json::from_value(manifest_value).map_err(|err| err.to_string())?;
        if manifest.schema_version != PROJECT_MANIFEST_SCHEMA_VERSION {
            let message = format!(
                "Unsupported project manifest schema: {}",
                manifest.schema_version
            );
            self.record_event(
                ProjectLauncherEventKind::OpenProject,
                Some(project_root),
                ProjectLauncherEventResult::Failed,
                vec![message.clone()],
            );
            return Err(message);
        }
        if let Err(message) = manifest.runtime_module.validate() {
            self.record_event(
                ProjectLauncherEventKind::OpenProject,
                Some(project_root),
                ProjectLauncherEventResult::Failed,
                vec![message.clone()],
            );
            return Err(message);
        }
        let settings = match load_project_settings(&write_scope, &manifest) {
            Ok(settings) => settings,
            Err(message) => {
                self.record_event(
                    ProjectLauncherEventKind::OpenProject,
                    Some(project_root),
                    ProjectLauncherEventResult::Failed,
                    vec![message.clone()],
                );
                return Err(message);
            }
        };
        let readiness = crate::ProjectReadiness::inspect(project_root, &self.engine_version);
        if readiness.status != crate::ProjectReadinessStatus::Ready {
            let message = format!(
                "project_readiness.open_rejected: status={:?}; diagnostics={}",
                readiness.status,
                readiness.diagnostics.join(",")
            );
            self.record_event(
                ProjectLauncherEventKind::OpenProject,
                Some(project_root),
                ProjectLauncherEventResult::Failed,
                vec![message.clone()],
            );
            return Err(message);
        }
        let session = ProjectSession {
            project_root: project_root.to_path_buf(),
            manifest,
            settings,
            write_scope,
        };
        self.add_recent_project(&session);
        self.record_event(
            ProjectLauncherEventKind::OpenProject,
            Some(project_root),
            ProjectLauncherEventResult::Succeeded,
            Vec::new(),
        );
        Ok(session)
    }

    pub fn select_recent_project(
        &mut self,
        project_root: impl AsRef<Path>,
    ) -> Result<ProjectSession, String> {
        let path = project_root.as_ref().to_path_buf();
        self.selected_project_path = Some(path.display().to_string());
        let result = self.open_project(&path);
        let event_result = if result.is_ok() {
            ProjectLauncherEventResult::Succeeded
        } else {
            ProjectLauncherEventResult::Failed
        };
        self.record_event(
            ProjectLauncherEventKind::SelectRecentProject,
            Some(&path),
            event_result,
            result
                .as_ref()
                .err()
                .map(|err| vec![err.clone()])
                .unwrap_or_default(),
        );
        result
    }

    pub fn refresh_recent_projects(&mut self) {
        for entry in &mut self.recent_projects {
            let status = validate_project_root(Path::new(&entry.path), &self.engine_version);
            entry.valid = status.valid();
            entry.status = status.as_status_str().to_string();
        }
        self.record_event(
            ProjectLauncherEventKind::RefreshRecentProjects,
            None,
            ProjectLauncherEventResult::Succeeded,
            Vec::new(),
        );
    }

    fn add_recent_project(&mut self, session: &ProjectSession) {
        let path = normalized_recent_project_display_path(&session.project_root);
        let identity = recent_project_path_identity(&path);
        let last_modified_at = fs::metadata(session.project_root.join("project.aife.json"))
            .and_then(|metadata| metadata.modified())
            .ok()
            .map(timestamp_from_system_time);
        self.recent_projects
            .retain(|entry| recent_project_path_identity(&entry.path) != identity);
        self.recent_projects.insert(
            0,
            RecentProjectEntry {
                name: session.manifest.project_name.clone(),
                path: path.clone(),
                engine_version: session.manifest.engine_version.clone(),
                last_opened_at: Some(timestamp_string()),
                last_modified_at,
                valid: true,
                status: "ready".to_string(),
            },
        );
        self.selected_project_path = Some(path);
    }

    fn record_event(
        &mut self,
        kind: ProjectLauncherEventKind,
        project_path: Option<&Path>,
        result: ProjectLauncherEventResult,
        diagnostics: Vec<String>,
    ) {
        self.events.push(ProjectLauncherEvent {
            schema_version: PROJECT_LAUNCHER_EVENT_SCHEMA_VERSION.to_string(),
            kind,
            project_path: project_path.map(|path| path.display().to_string()),
            result,
            diagnostics,
        });
    }

    fn record_create_failure(&mut self, project_root: &Path, error: &ProjectCreateError) {
        self.record_event(
            ProjectLauncherEventKind::CreateProject,
            (!project_root.as_os_str().is_empty()).then_some(project_root),
            ProjectLauncherEventResult::Failed,
            vec![error.to_string()],
        );
    }
}

fn normalize_recent_project_entries(
    recent_projects: Vec<RecentProjectEntry>,
) -> Vec<RecentProjectEntry> {
    let mut normalized = Vec::<RecentProjectEntry>::with_capacity(recent_projects.len());
    let mut index_by_identity = HashMap::<String, usize>::new();
    for mut entry in recent_projects {
        entry.path = normalized_recent_project_display_path(Path::new(&entry.path));
        let identity = recent_project_path_identity(&entry.path);
        if let Some(index) = index_by_identity.get(&identity).copied() {
            if recent_project_entry_is_newer(&entry, &normalized[index]) {
                normalized[index] = entry;
            }
        } else {
            index_by_identity.insert(identity, normalized.len());
            normalized.push(entry);
        }
    }
    normalized
}

fn recent_project_entry_is_newer(
    candidate: &RecentProjectEntry,
    current: &RecentProjectEntry,
) -> bool {
    recent_project_timestamp(candidate) > recent_project_timestamp(current)
}

fn recent_project_timestamp(entry: &RecentProjectEntry) -> Option<u128> {
    entry
        .last_opened_at
        .as_deref()
        .and_then(|value| value.parse().ok())
}

fn normalized_recent_project_display_path(path: &Path) -> String {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    normalize_windows_verbatim_path(resolved.display().to_string())
}

fn recent_project_path_identity(path: &str) -> String {
    let display = normalized_recent_project_display_path(Path::new(path));
    #[cfg(windows)]
    {
        let mut identity = display.replace('/', "\\").to_lowercase();
        while identity.ends_with('\\') && !identity.ends_with(":\\") {
            identity.pop();
        }
        identity
    }
    #[cfg(not(windows))]
    {
        display
    }
}

#[cfg(windows)]
fn normalize_windows_verbatim_path(path: String) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(&path).to_string()
}

#[cfg(not(windows))]
fn normalize_windows_verbatim_path(path: String) -> String {
    path
}

fn load_project_settings(
    write_scope: &ProjectWriteScope,
    manifest: &ProjectManifest,
) -> Result<ProjectSettingsDocument, String> {
    let bytes = write_scope
        .read("Settings/project_settings.json")
        .map_err(|error| format!("project_settings.read_failed: {error}"))?;
    let settings = serde_json::from_slice::<ProjectSettingsDocument>(&bytes)
        .map_err(|error| format!("project_settings.parse_failed: {error}"))?;
    settings.validate(manifest)?;
    Ok(settings)
}

fn cleanup_owned_project_target(root: &Path, claim_token: &str) -> ProjectCreateCleanupOutcome {
    let claim_matches = fs::read(root.join(PROJECT_CREATE_CLAIM_FILE))
        .is_ok_and(|bytes| bytes == claim_token.as_bytes());
    if !claim_matches || !owned_project_target_shape_is_intact(root) {
        return ProjectCreateCleanupOutcome::CleanupFailed;
    }
    match fs::remove_dir_all(root) {
        Ok(()) => ProjectCreateCleanupOutcome::RemovedOwnedTarget,
        Err(_) => ProjectCreateCleanupOutcome::CleanupFailed,
    }
}

fn owned_project_target_shape_is_intact(root: &Path) -> bool {
    const OWNED_PATHS: &[&str] = &[
        ".aife-project-create-claim",
        "Assets",
        "Scenes",
        "Scenes/Main.scene.json",
        "Packages",
        "Settings",
        "Settings/project_settings.json",
        "Library",
        "Input",
        "Input/input.none.json",
        "project.aife.json",
    ];

    fn inspect(root: &Path, directory: &Path) -> bool {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => return false,
        };
        for entry in entries {
            let Ok(entry) = entry else {
                return false;
            };
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return false;
            };
            if metadata.file_type().is_symlink() {
                return false;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                return false;
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if !OWNED_PATHS.contains(&relative.as_str()) {
                return false;
            }
            if metadata.is_dir() && !inspect(root, &path) {
                return false;
            }
        }
        true
    }

    inspect(root, root)
}

fn validate_project_create_request(
    project_root: &Path,
    project_name: &str,
) -> Result<PathBuf, ProjectCreateError> {
    if project_root.as_os_str().is_empty() {
        return Err(project_create_error(
            "project_create.root_empty",
            "Project root is empty.",
            ProjectCreateCleanupOutcome::NotRequired,
        ));
    }
    if !project_root.is_absolute() {
        return Err(project_create_error(
            "project_create.root_not_absolute",
            "Project root must be an absolute path.",
            ProjectCreateCleanupOutcome::NotRequired,
        ));
    }
    if project_root.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(project_create_error(
            "project_create.root_not_canonical",
            "Project root cannot contain '.' or '..' components.",
            ProjectCreateCleanupOutcome::NotRequired,
        ));
    }
    let root_text = project_root.to_string_lossy();
    if root_text.contains('%') || root_text.contains('$') || root_text.starts_with('~') {
        return Err(project_create_error(
            "project_create.root_expansion_forbidden",
            "Project root must not contain environment or home expansion syntax.",
            ProjectCreateCleanupOutcome::NotRequired,
        ));
    }
    let name = project_name.trim();
    if name.is_empty()
        || name != project_name
        || name.chars().count() > 128
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|character| character.is_control() || "\\/:*?\"<>|".contains(character))
    {
        return Err(project_create_error(
            "project_create.name_invalid",
            "Project name is invalid.",
            ProjectCreateCleanupOutcome::NotRequired,
        ));
    }
    let parent = project_root.parent().ok_or_else(|| {
        project_create_error(
            "project_create.root_is_filesystem_root",
            "A filesystem root cannot be used as a project target.",
            ProjectCreateCleanupOutcome::NotRequired,
        )
    })?;
    if !parent.is_dir() {
        return Err(project_create_error(
            "project_create.parent_missing",
            "Project parent directory does not exist or is not a directory.",
            ProjectCreateCleanupOutcome::NotRequired,
        ));
    }
    let file_name = project_root.file_name().ok_or_else(|| {
        project_create_error(
            "project_create.root_is_filesystem_root",
            "A filesystem root cannot be used as a project target.",
            ProjectCreateCleanupOutcome::NotRequired,
        )
    })?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        project_create_error(
            "project_create.parent_unusable",
            format!("Project parent cannot be canonicalized: {error}"),
            ProjectCreateCleanupOutcome::NotRequired,
        )
    })?;
    Ok(platform_display_path(canonical_parent).join(file_name))
}

fn platform_display_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{unc}"));
        }
        if let Some(local) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(local);
        }
    }
    path
}

fn project_create_error(
    code: impl Into<String>,
    message: impl Into<String>,
    cleanup_outcome: ProjectCreateCleanupOutcome,
) -> ProjectCreateError {
    ProjectCreateError {
        code: code.into(),
        message: message.into(),
        cleanup_outcome,
    }
}

pub fn validate_project_root(
    project_root: &Path,
    current_engine_version: &str,
) -> ProjectValidationStatus {
    crate::ProjectReadiness::inspect(project_root, current_engine_version).launcher_status()
}

fn write_json_pretty(path: impl AsRef<Path>, value: &impl Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    fs::write(path.as_ref(), text).map_err(|err| err.to_string())
}

fn write_project_json_pretty(
    scope: &ProjectWriteScope,
    relative_path: impl AsRef<Path>,
    value: &impl Serialize,
) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    scope
        .write_atomic(relative_path, text.as_bytes())
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn write_default_scene(
    scope: &ProjectWriteScope,
    relative_path: impl AsRef<Path>,
) -> Result<(), String> {
    scope
        .write_atomic(
            relative_path,
            r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": []
}"##
            .as_bytes(),
        )
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn timestamp_string() -> String {
    timestamp_from_system_time(SystemTime::now())
}

fn timestamp_from_system_time(timestamp: SystemTime) -> String {
    let seconds = timestamp
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

fn stable_project_stamp(path: &Path) -> u64 {
    path.display()
        .to_string()
        .bytes()
        .fold(1469598103934665603_u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(1099511628211)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_project_writes_minimum_project_skeleton() {
        let root = unique_temp_dir("project-launcher-create");
        let mut launcher = ProjectLauncherState::new("0.0.3");

        let session = launcher
            .create_project(&root, "PlaneGame")
            .expect("project should be created");

        assert_eq!(session.manifest.project_name, "PlaneGame");
        assert!(root.join("project.aife.json").exists());
        assert!(root.join("Assets").exists());
        assert!(root.join("Scenes").join("Main.scene.json").exists());
        assert_eq!(launcher.recent_projects.len(), 1);
        assert_eq!(launcher.recent_projects[0].name, "PlaneGame");
    }

    #[test]
    fn project_create_rejects_existing_target_without_writing() {
        let root = unique_temp_dir("project-launcher-existing");
        fs::create_dir_all(&root).unwrap();
        let sentinel = root.join("caller-owned.txt");
        fs::write(&sentinel, b"unchanged").unwrap();
        let mut launcher = ProjectLauncherState::new("0.0.3");

        let error = launcher
            .create_project(&root, "MustNotOverwrite")
            .expect_err("existing target must be rejected");

        assert!(error.contains("target_exists"));
        assert_eq!(fs::read(&sentinel).unwrap(), b"unchanged");
        assert!(!root.join("project.aife.json").exists());
    }

    #[test]
    fn project_create_rejects_non_absolute_root_and_missing_parent() {
        let mut launcher = ProjectLauncherState::new("0.0.3");
        let relative_error = launcher
            .create_project("relative/project", "Relative")
            .expect_err("relative target must be rejected");
        assert!(relative_error.contains("root_not_absolute"));

        let parent = unique_temp_dir("project-launcher-missing-parent");
        let target = parent.join("child");
        let missing_parent_error = launcher
            .create_project(&target, "MissingParent")
            .expect_err("missing parent must be rejected");
        assert!(missing_parent_error.contains("parent_missing"));
        assert!(!target.exists());
    }

    #[test]
    fn project_create_rejects_target_file_and_invalid_name_without_writing() {
        let target = unique_temp_dir("project-launcher-target-file");
        fs::write(&target, b"caller-owned").unwrap();
        let mut launcher = ProjectLauncherState::new("0.0.3");

        let file_error = launcher
            .create_project(&target, "TargetFile")
            .expect_err("target file must be rejected");
        assert!(file_error.contains("target_exists"));
        assert_eq!(fs::read(&target).unwrap(), b"caller-owned");

        let invalid_target = unique_temp_dir("project-launcher-invalid-name");
        let name_error = launcher
            .create_project(&invalid_target, "bad/name")
            .expect_err("invalid name must be rejected");
        assert!(name_error.contains("name_invalid"));
        assert!(!invalid_target.exists());
    }

    #[test]
    fn project_create_exclusive_claim_allows_exactly_one_owner() {
        use std::sync::{Arc, Barrier};

        let target = unique_temp_dir("project-launcher-contention");
        let barrier = Arc::new(Barrier::new(2));
        let workers = (0..2)
            .map(|index| {
                let target = target.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut launcher = ProjectLauncherState::new("0.0.3");
                    barrier.wait();
                    launcher.create_project(&target, format!("Owner{index}"))
                })
            })
            .collect::<Vec<_>>();
        let results = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| result
                    .as_ref()
                    .is_err_and(|error| error.contains("target_exists")))
                .count(),
            1
        );
    }

    #[test]
    fn project_create_initialization_failure_removes_owned_target() {
        let target = unique_temp_dir("project-launcher-owned-cleanup");
        let mut launcher = ProjectLauncherState::new("0.0.3");
        launcher.template_registry.templates[0].default_scene = "../escape.scene.json".to_string();

        let error = launcher
            .create_project_owned(&target, "CleanupOwnedTarget")
            .expect_err("escaping template path must fail");

        assert_eq!(error.code, "project_create.initialize_failed");
        assert_eq!(
            error.cleanup_outcome,
            ProjectCreateCleanupOutcome::RemovedOwnedTarget
        );
        assert!(!target.exists());
    }

    #[test]
    fn project_create_cleanup_preserves_target_when_ownership_is_uncertain() {
        let target = unique_temp_dir("project-launcher-uncertain-cleanup");
        fs::create_dir(&target).unwrap();
        fs::write(target.join(PROJECT_CREATE_CLAIM_FILE), b"claim").unwrap();
        fs::write(target.join("external-takeover.txt"), b"external").unwrap();

        let outcome = cleanup_owned_project_target(&target, "claim");

        assert_eq!(outcome, ProjectCreateCleanupOutcome::CleanupFailed);
        assert_eq!(
            fs::read(target.join("external-takeover.txt")).unwrap(),
            b"external"
        );
    }

    #[test]
    fn open_project_validates_manifest_and_updates_recent_list() {
        let root = unique_temp_dir("project-launcher-open");
        let mut launcher = ProjectLauncherState::new("0.0.3");
        launcher.create_project(&root, "OpenMe").unwrap();
        launcher.recent_projects.clear();

        let session = launcher.open_project(&root).expect("project should open");

        assert_eq!(session.manifest.project_name, "OpenMe");
        assert_eq!(launcher.recent_projects.len(), 1);
        assert_eq!(
            fs::canonicalize(launcher.selected_project_path.as_deref().unwrap()).unwrap(),
            fs::canonicalize(root).unwrap()
        );
    }

    #[test]
    fn open_project_keeps_manifest_bytes_unchanged_and_updates_editor_recent_state() {
        let root = unique_temp_dir("project-launcher-open-read-only");
        let mut launcher = ProjectLauncherState::new("0.0.3");
        launcher.create_project(&root, "ReadOnlyOpen").unwrap();
        let manifest_path = root.join("project.aife.json");
        let manifest_before = fs::read(&manifest_path).unwrap();
        launcher.recent_projects[0].last_opened_at = Some("0".to_string());

        let session = launcher.open_project(&root).expect("project should open");

        assert_eq!(fs::read(&manifest_path).unwrap(), manifest_before);
        assert_eq!(
            session.manifest.last_opened_at,
            Some(session.manifest.created_at)
        );
        assert_ne!(
            launcher.recent_projects[0].last_opened_at.as_deref(),
            Some("0")
        );
        assert!(launcher.recent_projects[0].last_modified_at.is_some());
    }

    #[test]
    fn project_settings_game_view_target_legacy_default_and_invalid_target_are_explicit() {
        let root = unique_temp_dir("project-launcher-game-view-settings");
        let mut launcher = ProjectLauncherState::new("0.0.3");
        let created = launcher.create_project(&root, "GameViewSettings").unwrap();
        assert_eq!(
            created.settings.resolved_game_view_target(),
            GameViewTargetSpec::default()
        );

        fs::write(
            root.join("Settings/project_settings.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": PROJECT_SETTINGS_SCHEMA_VERSION,
                "projectName": "GameViewSettings",
                "editorPreview": {
                    "gameViewTarget": {
                        "extent": { "width": 0, "height": 1280 },
                        "scalePolicy": "contain"
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let error = launcher
            .open_project(&root)
            .expect_err("invalid target must reject project open");
        assert!(error.contains("project_settings.game_view_target_invalid"));
        assert!(error.contains("game_view.presentation.target_extent_invalid"));
    }

    #[test]
    fn refresh_recent_projects_marks_missing_project_invalid() {
        let mut launcher = ProjectLauncherState::new("0.0.3");
        launcher.recent_projects.push(RecentProjectEntry {
            name: "Missing".to_string(),
            path: unique_temp_dir("project-launcher-missing")
                .join("deleted")
                .display()
                .to_string(),
            engine_version: "0.0.3".to_string(),
            last_opened_at: None,
            last_modified_at: None,
            valid: true,
            status: "ready".to_string(),
        });

        launcher.refresh_recent_projects();

        assert!(!launcher.recent_projects[0].valid);
        assert_eq!(launcher.recent_projects[0].status, "missing");
    }

    #[test]
    fn recent_projects_store_roundtrips_json_document() {
        let root = unique_temp_dir("project-launcher-recent-store");
        let store_path = root.join("editor_recent_projects.json");
        let document = ProjectRecentProjectsDocument::new(vec![RecentProjectEntry {
            name: "StoredProject".to_string(),
            path: "D:/Projects/StoredProject".to_string(),
            engine_version: "0.0.3".to_string(),
            last_opened_at: Some("1".to_string()),
            last_modified_at: Some("1".to_string()),
            valid: true,
            status: "ready".to_string(),
        }]);

        ProjectRecentStore::save(&store_path, &document).expect("save recent projects");
        let loaded = ProjectRecentStore::load(&store_path).expect("load recent projects");

        assert_eq!(loaded.schema_version, EDITOR_RECENT_PROJECTS_SCHEMA_VERSION);
        assert_eq!(loaded.recent_projects[0].name, "StoredProject");
    }

    #[test]
    fn launcher_loads_and_validates_recent_projects() {
        let project_root = unique_temp_dir("project-launcher-valid-recent");
        let mut setup = ProjectLauncherState::new("0.0.3");
        setup.create_project(&project_root, "ValidProject").unwrap();
        let store_path = unique_temp_dir("project-launcher-store").join("recent.json");
        setup.save_recent_projects(&store_path).unwrap();

        let mut launcher = ProjectLauncherState::new("0.0.3");
        launcher.load_recent_projects(&store_path).unwrap();

        assert_eq!(launcher.recent_projects.len(), 1);
        assert_eq!(launcher.recent_projects[0].status, "ready");
        assert!(launcher.recent_projects[0].valid);
    }

    #[cfg(windows)]
    #[test]
    fn launcher_collapses_windows_verbatim_and_display_paths_for_the_same_project() {
        let project_root = unique_temp_dir("project-launcher-verbatim-recent");
        let mut setup = ProjectLauncherState::new("0.0.3");
        setup.create_project(&project_root, "SameProject").unwrap();
        let display_path = project_root.display().to_string();
        let verbatim_path = format!(r"\\?\{display_path}");
        let mut older = setup.recent_projects[0].clone();
        older.path = display_path.clone();
        older.last_opened_at = Some("10".to_string());
        let mut newer = older.clone();
        newer.path = verbatim_path;
        newer.last_opened_at = Some("20".to_string());

        let mut launcher = ProjectLauncherState::new("0.0.3");
        launcher.apply_recent_projects(vec![older, newer]);

        assert_eq!(launcher.recent_projects.len(), 1);
        assert!(!launcher.recent_projects[0].path.starts_with(r"\\?\"));
        assert_eq!(
            fs::canonicalize(&launcher.recent_projects[0].path).unwrap(),
            fs::canonicalize(project_root).unwrap()
        );
        assert_eq!(
            launcher.recent_projects[0].last_opened_at.as_deref(),
            Some("20")
        );
    }

    #[test]
    fn validate_project_root_detects_invalid_manifest() {
        let root = unique_temp_dir("project-launcher-invalid");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("project.aife.json"), "{not-json").unwrap();

        let status = validate_project_root(&root, "0.0.3");

        assert_eq!(status, ProjectValidationStatus::InvalidManifest);
        assert!(!status.valid());
        assert_eq!(status.as_status_str(), "invalid_manifest");
    }

    #[test]
    fn template_registry_has_empty_project_template() {
        let registry = ProjectTemplateRegistry::c_min();

        let template = registry.default_template().expect("default template");

        assert_eq!(template.template_id, "empty_project");
        assert_eq!(template.default_scene, "Scenes/Main.scene.json");
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{stamp}"))
    }
}
