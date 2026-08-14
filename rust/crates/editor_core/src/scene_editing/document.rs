use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    EditorSceneEntity, SceneEditDiagnostic, SceneEditDiagnosticSeverity,
    EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSceneDocument {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "id")]
    pub scene_id: String,
    pub name: String,
    #[serde(default)]
    pub gravity: f32,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub sky_color: String,
    #[serde(default)]
    pub entities: Vec<EditorSceneEntity>,
    #[serde(skip)]
    pub scene_path: Option<PathBuf>,
    #[serde(skip)]
    pub dirty_state: SceneDirtyState,
    #[serde(skip)]
    pub revision: u64,
}

impl EditorSceneDocument {
    pub fn new(scene_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION.to_string(),
            scene_id: scene_id.into(),
            name: name.into(),
            gravity: 0.0,
            background: "#000000".to_string(),
            sky_color: "#111111".to_string(),
            entities: Vec::new(),
            scene_path: None,
            dirty_state: SceneDirtyState::default(),
            revision: 0,
        }
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, Vec<SceneEditDiagnostic>> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|error| {
            vec![SceneEditDiagnostic::error(
                "scene.document.read_failed",
                "scene.document",
                format!("Failed to read scene file {}: {}", path.display(), error),
            )
            .with_path(path.display().to_string())]
        })?;
        let mut document = serde_json::from_str::<Self>(&text).map_err(|error| {
            vec![SceneEditDiagnostic::error(
                "scene.document.parse_failed",
                "scene.document",
                format!("Failed to parse scene file {}: {}", path.display(), error),
            )
            .with_path(path.display().to_string())]
        })?;
        document.scene_path = Some(path.to_path_buf());
        document.dirty_state = SceneDirtyState::default();
        document.revision = 0;
        let diagnostics = document.validate();
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == SceneEditDiagnosticSeverity::Error)
        {
            Err(diagnostics)
        } else {
            Ok(document)
        }
    }

    pub fn to_stable_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn validate(&self) -> Vec<SceneEditDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.scene_id.trim().is_empty() {
            diagnostics.push(SceneEditDiagnostic::error(
                "scene.document.scene_id_required",
                "scene.document",
                "Scene id is required.",
            ));
        }
        let mut ids = BTreeSet::new();
        for (index, entity) in self.entities.iter().enumerate() {
            let path = format!("entities[{index}]");
            if entity.entity_id.trim().is_empty() {
                diagnostics.push(
                    SceneEditDiagnostic::error(
                        "scene.entity.id_required",
                        "scene.entity",
                        "Entity id is required.",
                    )
                    .with_path(format!("{path}.id")),
                );
            }
            if !ids.insert(entity.entity_id.clone()) {
                diagnostics.push(
                    SceneEditDiagnostic::error(
                        "scene.entity.duplicate_id",
                        "scene.entity",
                        format!("Duplicate entity id: {}", entity.entity_id),
                    )
                    .with_path(format!("{path}.id"))
                    .with_entity_id(entity.entity_id.clone()),
                );
            }
            if entity.transform.is_none() {
                diagnostics.push(
                    SceneEditDiagnostic::error(
                        "scene.entity.transform_required",
                        "scene.entity",
                        format!("Entity {} requires a Transform.", entity.entity_id),
                    )
                    .with_path(format!("{path}.transform"))
                    .with_entity_id(entity.entity_id.clone()),
                );
            }
            if let Some(parent_id) = &entity.parent_id {
                if !self.has_entity(parent_id) {
                    diagnostics.push(
                        SceneEditDiagnostic::error(
                            "scene.entity.parent_missing",
                            "scene.entity",
                            format!("Parent entity does not exist: {parent_id}"),
                        )
                        .with_path(format!("{path}.parentId"))
                        .with_entity_id(entity.entity_id.clone()),
                    );
                }
            }
        }
        diagnostics
    }

    pub fn has_entity(&self, entity_id: &str) -> bool {
        self.entities
            .iter()
            .any(|entity| entity.entity_id == entity_id)
    }

    pub fn entity(&self, entity_id: &str) -> Option<&EditorSceneEntity> {
        self.entities
            .iter()
            .find(|entity| entity.entity_id == entity_id)
    }

    pub fn entity_mut(&mut self, entity_id: &str) -> Option<&mut EditorSceneEntity> {
        self.entities
            .iter_mut()
            .find(|entity| entity.entity_id == entity_id)
    }

    pub fn mark_dirty(&mut self, transaction_id: impl Into<String>) {
        self.revision = self.revision.saturating_add(1);
        self.dirty_state.dirty = true;
        self.dirty_state.revision = self.revision;
        self.dirty_state.last_transaction_id = Some(transaction_id.into());
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_state.dirty = false;
        self.dirty_state.last_transaction_id = None;
    }

    pub fn next_entity_id(&self, name: &str) -> String {
        let slug = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        let base = if slug.is_empty() {
            "entity".to_string()
        } else {
            format!("entity-{slug}")
        };
        if !self.has_entity(&base) {
            return base;
        }
        for index in 2.. {
            let candidate = format!("{base}-{index}");
            if !self.has_entity(&candidate) {
                return candidate;
            }
        }
        unreachable!("unbounded entity id generation should not terminate")
    }

    pub(super) fn remove_subtree(&mut self, root_id: &str) -> Vec<String> {
        let mut removed = Vec::new();
        let mut queue = VecDeque::from([root_id.to_string()]);
        while let Some(entity_id) = queue.pop_front() {
            for child in self
                .entities
                .iter()
                .filter(|entity| entity.parent_id.as_deref() == Some(entity_id.as_str()))
            {
                queue.push_back(child.entity_id.clone());
            }
            removed.push(entity_id);
        }
        self.entities
            .retain(|entity| !removed.iter().any(|id| id == &entity.entity_id));
        removed
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneDirtyState {
    pub dirty: bool,
    pub revision: u64,
    pub last_transaction_id: Option<String>,
}
