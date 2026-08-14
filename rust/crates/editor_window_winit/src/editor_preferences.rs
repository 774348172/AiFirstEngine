use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use editor_ui_model::{
    EditorCatalogDiagnostic, EditorCatalogDiagnosticCode, EditorLocaleId,
    EditorLocalizationSnapshot,
};
use serde::{Deserialize, Serialize};

pub const EDITOR_PREFERENCES_SCHEMA_VERSION: &str = "editor-preferences.v1";
pub const EDITOR_PREFERENCES_FILE_NAME: &str = "editor_preferences.json";
pub const EDITOR_PREFERENCES_ROOT_OVERRIDE_ENV: &str = "AIFE_EDITOR_PREFERENCES_ROOT";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreferencesDocument {
    pub schema_version: String,
    pub locale: EditorLocaleId,
}

impl Default for EditorPreferencesDocument {
    fn default() -> Self {
        Self {
            schema_version: EDITOR_PREFERENCES_SCHEMA_VERSION.to_string(),
            locale: EditorLocaleId::zh_cn(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPreferencesLoad {
    pub preferences: EditorPreferencesDocument,
    pub diagnostic: Option<EditorCatalogDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPreferencesSave {
    pub written: bool,
    pub diagnostic: Option<EditorCatalogDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct EditorPreferenceStore {
    path: PathBuf,
}

impl EditorPreferenceStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn load(&self) -> EditorPreferencesLoad {
        if !self.path.exists() {
            return EditorPreferencesLoad {
                preferences: EditorPreferencesDocument::default(),
                diagnostic: None,
            };
        }
        let result = fs::read_to_string(&self.path)
            .ok()
            .and_then(|content| serde_json::from_str::<EditorPreferencesDocument>(&content).ok())
            .filter(|document| document.schema_version == EDITOR_PREFERENCES_SCHEMA_VERSION)
            .and_then(|document| {
                EditorLocaleId::parse(document.locale.as_str())
                    .ok()
                    .map(|locale| EditorPreferencesDocument { locale, ..document })
            });
        match result {
            Some(preferences) => EditorPreferencesLoad {
                preferences,
                diagnostic: None,
            },
            None => EditorPreferencesLoad {
                preferences: EditorPreferencesDocument::default(),
                diagnostic: Some(EditorCatalogDiagnostic::new(
                    EditorCatalogDiagnosticCode::PreferenceMalformed,
                    format!(
                        "Editor preferences at `{}` are malformed; using zh-CN",
                        self.path.display()
                    ),
                )),
            },
        }
    }

    pub fn save(&self, preferences: &EditorPreferencesDocument) -> EditorPreferencesSave {
        let content = match serde_json::to_vec_pretty(preferences) {
            Ok(content) => content,
            Err(error) => return self.save_failure(error.to_string()),
        };
        let Some(parent) = self.path.parent() else {
            return self.save_failure("preferences path has no parent".to_string());
        };
        if let Err(error) = fs::create_dir_all(parent) {
            return self.save_failure(error.to_string());
        }
        let temp_path = self.temp_path();
        let result = (|| -> std::io::Result<()> {
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
        if let Err(error) = result {
            let _ = fs::remove_file(&temp_path);
            return self.save_failure(error.to_string());
        }
        EditorPreferencesSave {
            written: true,
            diagnostic: None,
        }
    }

    fn temp_path(&self) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(EDITOR_PREFERENCES_FILE_NAME);
        self.path
            .with_file_name(format!("{name}.tmp-{}-{nonce}", std::process::id()))
    }

    fn save_failure(&self, reason: String) -> EditorPreferencesSave {
        EditorPreferencesSave {
            written: false,
            diagnostic: Some(EditorCatalogDiagnostic::new(
                EditorCatalogDiagnosticCode::PreferenceWriteFailed,
                format!(
                    "failed to write Editor preferences at `{}`: {reason}",
                    self.path.display()
                ),
            )),
        }
    }
}

pub fn editor_preference_store_at_root(root: PathBuf) -> EditorPreferenceStore {
    EditorPreferenceStore::new(
        root.join("AiFirstGameEngine")
            .join("Editor")
            .join(EDITOR_PREFERENCES_FILE_NAME),
    )
}

pub fn default_editor_preference_store() -> Option<EditorPreferenceStore> {
    let root = std::env::var_os(EDITOR_PREFERENCES_ROOT_OVERRIDE_ENV)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|home| home.join("AppData").join("Roaming"))
        })?;
    Some(editor_preference_store_at_root(root))
}

pub fn snapshot_from_preferences(
    preferences: &EditorPreferencesDocument,
    revision: u64,
) -> Result<EditorLocalizationSnapshot, EditorCatalogDiagnostic> {
    editor_ui_model::trusted_editor_localization_bundle()
        .snapshot(preferences.locale.clone(), revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aife-editor-preferences-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    #[test]
    fn editor_preferences_missing_store_defaults_to_chinese() {
        let root = temp_root("default");
        let store = editor_preference_store_at_root(root.clone());
        let loaded = store.load();
        assert_eq!(loaded.preferences.locale.as_str(), "zh-CN");
        assert!(loaded.diagnostic.is_none());
        assert!(!store.path().exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn editor_preferences_roundtrip_explicit_english_atomically() {
        let root = temp_root("roundtrip");
        let store = editor_preference_store_at_root(root.clone());
        let preferences = EditorPreferencesDocument {
            locale: EditorLocaleId::en_us(),
            ..EditorPreferencesDocument::default()
        };
        assert!(store.save(&preferences).written);
        let loaded = store.load();
        assert_eq!(loaded.preferences, preferences);
        assert!(loaded.diagnostic.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn editor_preferences_malformed_store_recovers_without_overwrite() {
        let root = temp_root("malformed");
        let store = editor_preference_store_at_root(root.clone());
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), b"not-json").unwrap();
        let loaded = store.load();
        assert_eq!(loaded.preferences.locale.as_str(), "zh-CN");
        assert_eq!(
            loaded.diagnostic.unwrap().code,
            EditorCatalogDiagnosticCode::PreferenceMalformed
        );
        assert_eq!(fs::read(store.path()).unwrap(), b"not-json");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn editor_preferences_write_failure_preserves_existing_document() {
        let root = temp_root("write-failure");
        let store = editor_preference_store_at_root(root.clone());
        let initial = EditorPreferencesDocument::default();
        assert!(store.save(&initial).written);
        let blocker = store.path().with_extension("json.last-good");
        fs::create_dir_all(&blocker).unwrap();
        let replacement = EditorPreferencesDocument {
            locale: EditorLocaleId::en_us(),
            ..EditorPreferencesDocument::default()
        };
        let save = store.save(&replacement);
        assert!(!save.written);
        assert_eq!(store.load().preferences, initial);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn editor_locale_application_switch_persists_before_publishing_revision() {
        let root = temp_root("application-switch");
        let store = editor_preference_store_at_root(root.clone());
        let mut application =
            crate::NativeEditorApplication::new(crate::NativeEditorWindowConfig::default())
                .with_editor_preference_store(store.clone());
        assert_eq!(application.localization_snapshot().locale.as_str(), "zh-CN");
        let result = application.change_editor_locale(EditorLocaleId::en_us());
        assert!(result.changed);
        assert_eq!(result.snapshot.locale.as_str(), "en-US");
        assert_eq!(result.snapshot.revision, 1);
        assert_eq!(store.load().preferences.locale.as_str(), "en-US");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn editor_locale_application_rejects_unpersisted_switch() {
        let mut application =
            crate::NativeEditorApplication::new(crate::NativeEditorWindowConfig::default());
        let before = application.localization_snapshot().clone();
        let result = application.change_editor_locale(EditorLocaleId::en_us());
        assert!(!result.changed);
        assert_eq!(result.snapshot, before);
        assert_eq!(
            result.diagnostic.unwrap().code,
            EditorCatalogDiagnosticCode::SwitchRejected
        );
    }
}
