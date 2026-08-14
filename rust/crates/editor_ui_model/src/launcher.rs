use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectLauncherModel {
    pub title: String,
    pub search_query: String,
    pub selected_project_path: Option<String>,
    pub recent_projects: Vec<RecentProjectEntry>,
    pub commands: Vec<ProjectLauncherCommand>,
    pub activity: Option<ProjectOpenActivityModel>,
    pub empty_message: String,
}

impl ProjectLauncherModel {
    pub fn empty() -> Self {
        Self {
            title: "Projects".to_string(),
            search_query: String::new(),
            selected_project_path: None,
            recent_projects: Vec::new(),
            commands: vec![
                ProjectLauncherCommand::new("open_project", "Open Project", true, None),
                ProjectLauncherCommand::new("create_project", "Create Project", true, None),
                ProjectLauncherCommand::new("create_with_ai", "Create with AI", true, None),
                ProjectLauncherCommand::new("refresh_recent_projects", "Refresh", true, None),
            ],
            activity: None,
            empty_message: "No recent projects. Open or create a project to begin.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpenActivityPhase {
    Inspecting,
    TrustCheck,
    CacheCheck,
    CacheLookup,
    Promoting,
    Warming,
    Staging,
    Compiling,
    Sealing,
    Launching,
    WaitingReadiness,
    ReadingProject,
    ComputingDigest,
    LoadingWorkspace,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectOpenActivityModel {
    pub operation_id: String,
    pub project_display_name: String,
    pub phase: ProjectOpenActivityPhase,
    pub completed_units: Option<u64>,
    pub total_units: Option<u64>,
    pub elapsed_ms: u64,
    pub cancellable: bool,
    pub diagnostic_code: Option<String>,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectLauncherCommand {
    pub command_id: String,
    pub label: String,
    pub enabled: bool,
    pub reason_disabled: Option<String>,
}

impl ProjectLauncherCommand {
    pub fn new(
        command_id: impl Into<String>,
        label: impl Into<String>,
        enabled: bool,
        reason_disabled: Option<String>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            label: label.into(),
            enabled,
            reason_disabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_open_activity_phase_roundtrips_composition_lifecycle_phases() {
        for phase in [
            ProjectOpenActivityPhase::Inspecting,
            ProjectOpenActivityPhase::CacheLookup,
            ProjectOpenActivityPhase::Promoting,
            ProjectOpenActivityPhase::Warming,
            ProjectOpenActivityPhase::Compiling,
            ProjectOpenActivityPhase::Sealing,
            ProjectOpenActivityPhase::Cancelled,
        ] {
            let encoded = serde_json::to_string(&phase).unwrap();
            assert_eq!(
                serde_json::from_str::<ProjectOpenActivityPhase>(&encoded).unwrap(),
                phase
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentProjectEntry {
    pub name: String,
    pub path: String,
    pub engine_version: String,
    pub last_opened_at: Option<String>,
    pub last_modified_at: Option<String>,
    pub valid: bool,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePackageSummary {
    pub package_dir: String,
    pub project_name: String,
    pub project_version: String,
    pub active_scene_id: String,
}
