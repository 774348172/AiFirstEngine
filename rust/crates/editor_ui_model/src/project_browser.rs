use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectBrowserModel {
    pub project_root: Option<String>,
    pub selected_path: Option<String>,
    pub entries: Vec<ProjectBrowserEntry>,
    pub empty_message: String,
}

impl ProjectBrowserModel {
    pub fn empty() -> Self {
        Self {
            project_root: None,
            selected_path: None,
            entries: Vec::new(),
            empty_message: "No project is open.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectBrowserEntry {
    pub path: String,
    pub label: String,
    pub kind: ProjectBrowserEntryKind,
    pub exists: bool,
    pub selected: bool,
    pub openable: bool,
}

impl ProjectBrowserEntry {
    pub fn new(
        path: impl Into<String>,
        label: impl Into<String>,
        kind: ProjectBrowserEntryKind,
        exists: bool,
        selected: bool,
        openable: bool,
    ) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
            kind,
            exists,
            selected,
            openable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectBrowserEntryKind {
    Folder,
    Scene,
    Asset,
    Settings,
    Unknown,
}
