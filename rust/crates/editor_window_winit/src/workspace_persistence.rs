use editor_ui_renderer::{
    EditorWorkspaceLayout, WorkspaceTopology, EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const EDITOR_WORKSPACE_LAYOUT_FILE_NAME: &str = "editor-workspace-layout.v2.json";
pub const LEGACY_EDITOR_WORKSPACE_LAYOUT_FILE_NAME: &str = "editor-workspace-layout.v1.json";
pub const EDITOR_WORKSPACE_LAYOUT_ROOT_OVERRIDE_ENV: &str = "AIFE_EDITOR_WORKSPACE_LAYOUT_ROOT";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePersistenceDiagnostic {
    pub code: String,
    pub path: String,
}

impl WorkspacePersistenceDiagnostic {
    fn new(code: &str, path: &Path) -> Self {
        Self {
            code: code.to_string(),
            path: path.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceLayoutLoad {
    pub topology: Option<WorkspaceTopology>,
    pub legacy_layout: Option<EditorWorkspaceLayout>,
    pub diagnostics: Vec<WorkspacePersistenceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLayoutSave {
    pub written: bool,
    pub diagnostics: Vec<WorkspacePersistenceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLayoutStore {
    path: PathBuf,
}

impl WorkspaceLayoutStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> WorkspaceLayoutLoad {
        let (content, loaded_legacy_path) = match fs::read_to_string(&self.path) {
            Ok(content) => (content, false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let legacy_path = self.legacy_path();
                match fs::read_to_string(&legacy_path) {
                    Ok(content) => (content, true),
                    Err(legacy_error) if legacy_error.kind() == std::io::ErrorKind::NotFound => {
                        return WorkspaceLayoutLoad {
                            topology: None,
                            legacy_layout: None,
                            diagnostics: vec![WorkspacePersistenceDiagnostic::new(
                                "workspace_layout_missing",
                                &self.path,
                            )],
                        };
                    }
                    Err(_) => {
                        return WorkspaceLayoutLoad {
                            topology: None,
                            legacy_layout: None,
                            diagnostics: vec![WorkspacePersistenceDiagnostic::new(
                                "workspace_layout_read_failed",
                                &legacy_path,
                            )],
                        };
                    }
                }
            }
            Err(_) => {
                return WorkspaceLayoutLoad {
                    topology: None,
                    legacy_layout: None,
                    diagnostics: vec![WorkspacePersistenceDiagnostic::new(
                        "workspace_layout_read_failed",
                        &self.path,
                    )],
                };
            }
        };
        if !loaded_legacy_path {
            if let Ok(topology) = serde_json::from_str::<WorkspaceTopology>(&content) {
                if topology.schema_version == EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION {
                    return WorkspaceLayoutLoad {
                        topology: Some(topology),
                        legacy_layout: None,
                        diagnostics: Vec::new(),
                    };
                }
            }
        }
        match serde_json::from_str::<EditorWorkspaceLayout>(&content) {
            Ok(layout) => {
                let migration_path = if loaded_legacy_path {
                    self.legacy_path()
                } else {
                    self.path.clone()
                };
                WorkspaceLayoutLoad {
                    topology: None,
                    legacy_layout: Some(layout),
                    diagnostics: vec![WorkspacePersistenceDiagnostic::new(
                        "workspace_layout_v1_migration_required",
                        &migration_path,
                    )],
                }
            }
            Err(_) => WorkspaceLayoutLoad {
                topology: None,
                legacy_layout: None,
                diagnostics: vec![WorkspacePersistenceDiagnostic::new(
                    "workspace_layout_malformed",
                    &self.path,
                )],
            },
        }
    }

    pub fn save(&self, topology: &WorkspaceTopology) -> WorkspaceLayoutSave {
        let content = match serde_json::to_vec_pretty(topology) {
            Ok(content) => content,
            Err(_) => return self.save_failure(),
        };
        let Some(parent) = self.path.parent() else {
            return self.save_failure();
        };
        if fs::create_dir_all(parent).is_err() {
            return self.save_failure();
        }
        let temp_path = self.temp_path();
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(&content)?;
            file.sync_all()?;
            if self.path.exists() {
                let backup = self.path.with_extension("json.last-good");
                let _ = fs::remove_file(&backup);
                fs::rename(&self.path, &backup)?;
                if let Err(error) = fs::rename(&temp_path, &self.path) {
                    let _ = fs::rename(&backup, &self.path);
                    return Err(error);
                }
                let _ = fs::remove_file(backup);
            } else {
                fs::rename(&temp_path, &self.path)?;
            }
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
            return self.save_failure();
        }
        WorkspaceLayoutSave {
            written: true,
            diagnostics: Vec::new(),
        }
    }

    fn legacy_path(&self) -> PathBuf {
        self.path
            .with_file_name(LEGACY_EDITOR_WORKSPACE_LAYOUT_FILE_NAME)
    }

    fn temp_path(&self) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(EDITOR_WORKSPACE_LAYOUT_FILE_NAME);
        self.path
            .with_file_name(format!("{file_name}.tmp-{}-{nonce}", std::process::id()))
    }

    fn save_failure(&self) -> WorkspaceLayoutSave {
        WorkspaceLayoutSave {
            written: false,
            diagnostics: vec![WorkspacePersistenceDiagnostic::new(
                "workspace_layout_write_failed",
                &self.path,
            )],
        }
    }
}

pub fn default_workspace_layout_store() -> Option<WorkspaceLayoutStore> {
    let root = std::env::var_os(EDITOR_WORKSPACE_LAYOUT_ROOT_OVERRIDE_ENV)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|home| home.join("AppData").join("Roaming"))
        })?;
    Some(workspace_layout_store_at_root(root))
}

pub fn workspace_layout_store_at_root(root: PathBuf) -> WorkspaceLayoutStore {
    WorkspaceLayoutStore::new(
        root.join("AiFirstGameEngine")
            .join("Editor")
            .join(EDITOR_WORKSPACE_LAYOUT_FILE_NAME),
    )
}
