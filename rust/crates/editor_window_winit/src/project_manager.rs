use crate::dialog::ProjectFolderDialogResponse;
use editor_core::{EditorSession, ProjectRecentStore};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManagerController {
    pub recent_store_path: Option<PathBuf>,
    pub last_dialog_response: Option<ProjectFolderDialogResponse>,
    pub last_persistence_error: Option<String>,
}

fn non_empty_env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn default_native_editor_recent_store_path() -> PathBuf {
    if let Some(app_data) = non_empty_env_path("APPDATA") {
        return app_data
            .join("AI First Engine")
            .join("editor_recent_projects.json");
    }
    if let Some(config_home) = non_empty_env_path("XDG_CONFIG_HOME") {
        return config_home
            .join("ai-first-engine")
            .join("editor_recent_projects.json");
    }
    if let Some(home) = non_empty_env_path("HOME") {
        return home
            .join(".ai-first-engine")
            .join("editor_recent_projects.json");
    }
    std::env::temp_dir()
        .join("ai-first-engine")
        .join("editor_recent_projects.json")
}

impl ProjectManagerController {
    pub fn with_recent_store_path(path: impl Into<PathBuf>) -> Self {
        Self {
            recent_store_path: Some(path.into()),
            last_dialog_response: None,
            last_persistence_error: None,
        }
    }

    pub fn load_recent_projects(&mut self, session: &mut EditorSession) {
        let Some(path) = self.recent_store_path.clone() else {
            return;
        };
        match session.load_recent_projects_for_launcher(&path) {
            Ok(true) => self.save_recent_projects(session),
            Ok(false) => {}
            Err(message) => self.last_persistence_error = Some(message),
        }
    }

    pub fn save_recent_projects(&mut self, session: &EditorSession) {
        let Some(path) = &self.recent_store_path else {
            return;
        };
        let document = editor_core::ProjectRecentProjectsDocument::new(
            session.project_launcher_state().recent_projects.clone(),
        );
        if let Err(message) = ProjectRecentStore::save(path, &document) {
            self.last_persistence_error = Some(message);
        }
    }
}
