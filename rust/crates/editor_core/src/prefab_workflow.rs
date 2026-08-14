use crate::{EditorSceneComponent, EditorSceneEntity, EditorTransform};
use editor_ui_model::{PrefabStageMode, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const PREFAB_WORKFLOW_REPORT_SCHEMA_VERSION: &str = "prefab-workflow-report.v1";
pub const PREFAB_AUTHORING_REPORT_SCHEMA_VERSION: &str = "prefab-authoring-report.v1";
pub const PREFAB_STAGE_REPORT_SCHEMA_VERSION: &str = "prefab-stage-report.v1";
pub const PREFAB_ASSET_SCHEMA_VERSION: &str = "authoring-prefab-asset.v1";
pub const PREFAB_INSTANCE_COMPONENT_TYPE: &str = "engine.prefab_instance";
pub const PREFAB_OVERRIDE_COMPONENT_TYPE: &str = "engine.prefab_overrides";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabAsset {
    pub schema_version: String,
    pub prefab_id: String,
    pub name: String,
    pub source_path: Option<String>,
    pub root_entity_id: String,
    pub entities: Vec<PrefabEntity>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl PrefabAsset {
    pub fn new(
        prefab_id: impl Into<String>,
        name: impl Into<String>,
        root_entity_id: impl Into<String>,
        entities: Vec<PrefabEntity>,
    ) -> Self {
        Self {
            schema_version: PREFAB_ASSET_SCHEMA_VERSION.to_string(),
            prefab_id: prefab_id.into(),
            name: name.into(),
            source_path: None,
            root_entity_id: root_entity_id.into(),
            entities,
            metadata: BTreeMap::new(),
        }
    }

    pub fn from_entity_tree(
        prefab_id: impl Into<String>,
        name: impl Into<String>,
        root: &EditorSceneEntity,
        children_by_parent: &BTreeMap<String, Vec<EditorSceneEntity>>,
    ) -> Self {
        let mut entities = Vec::new();
        collect_prefab_entities(root, children_by_parent, &mut entities);
        Self::new(prefab_id, name, root.entity_id.clone(), entities)
    }

    pub fn entity(&self, source_entity_id: &str) -> Option<&PrefabEntity> {
        self.entities
            .iter()
            .find(|entity| entity.source_entity_id == source_entity_id)
    }

    pub fn entity_mut(&mut self, source_entity_id: &str) -> Option<&mut PrefabEntity> {
        self.entities
            .iter_mut()
            .find(|entity| entity.source_entity_id == source_entity_id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabEntity {
    pub source_entity_id: String,
    pub name: String,
    pub parent_source_entity_id: Option<String>,
    pub sibling_order: i32,
    pub enabled: bool,
    pub transform: EditorTransform,
    #[serde(default)]
    pub components: Vec<EditorSceneComponent>,
    #[serde(default)]
    pub asset_refs: Vec<PrefabAssetRef>,
}

impl PrefabEntity {
    pub fn from_scene_entity(entity: &EditorSceneEntity) -> Self {
        Self {
            source_entity_id: entity.entity_id.clone(),
            name: entity.name.clone(),
            parent_source_entity_id: entity.parent_id.clone(),
            sibling_order: entity.sibling_order,
            enabled: entity.enabled,
            transform: entity.transform.unwrap_or_else(EditorTransform::identity),
            components: entity.components.clone(),
            asset_refs: prefab_asset_refs_from_entity(entity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabAssetRef {
    pub id: String,
    pub asset_type: String,
    #[serde(default)]
    pub guid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabInstance {
    pub instance_id: String,
    pub prefab_ref: PrefabRef,
    pub scene_parent_entity_id: Option<String>,
    pub instance_root_entity_id: String,
    #[serde(default)]
    pub overrides: Vec<PrefabOverride>,
}

impl PrefabInstance {
    pub fn new(
        instance_id: impl Into<String>,
        prefab_ref: PrefabRef,
        instance_root_entity_id: impl Into<String>,
    ) -> Self {
        Self {
            instance_id: instance_id.into(),
            prefab_ref,
            scene_parent_entity_id: None,
            instance_root_entity_id: instance_root_entity_id.into(),
            overrides: Vec::new(),
        }
    }

    pub fn from_scene_entity(entity: &EditorSceneEntity) -> Result<Self, PrefabDiagnostic> {
        let component = entity
            .components
            .iter()
            .find(|component| component.component_type == PREFAB_INSTANCE_COMPONENT_TYPE)
            .ok_or_else(|| {
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::InvalidPrefabRef,
                    format!("Entity {} is not a PrefabInstance.", entity.entity_id),
                )
                .with_source_entity_id(entity.entity_id.clone())
            })?;
        let source = component.fields.get("source").ok_or_else(|| {
            PrefabDiagnostic::error(
                PrefabDiagnosticCode::InvalidPrefabRef,
                "Prefab instance component requires source.",
            )
            .with_source_entity_id(entity.entity_id.clone())
        })?;
        let prefab_id = source
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::InvalidPrefabRef,
                    "Prefab instance source.id is required.",
                )
                .with_source_entity_id(entity.entity_id.clone())
            })?;
        let guid = source
            .get("guid")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let instance_id = component
            .fields
            .get("instanceId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("prefab-instance-{}", entity.entity_id));
        let mut instance = Self {
            instance_id,
            prefab_ref: PrefabRef {
                id: prefab_id.to_string(),
                guid,
            },
            scene_parent_entity_id: entity.parent_id.clone(),
            instance_root_entity_id: entity.entity_id.clone(),
            overrides: Vec::new(),
        };
        if let Some(overrides) = component.fields.get("overrides") {
            instance.overrides = serde_json::from_value(overrides.clone()).map_err(|_| {
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::InvalidOverrideField,
                    "Prefab instance overrides are invalid.",
                )
                .with_source_entity_id(entity.entity_id.clone())
            })?;
        }
        Ok(instance)
    }

    pub fn to_scene_component(&self) -> EditorSceneComponent {
        EditorSceneComponent {
            component_type: PREFAB_INSTANCE_COMPONENT_TYPE.to_string(),
            fields: serde_json::json!({
                "source": {
                    "id": self.prefab_ref.id,
                    "type": "prefab",
                    "guid": self.prefab_ref.guid,
                },
                "instanceId": self.instance_id,
                "overrides": self.overrides,
            }),
        }
    }

    pub fn set_override(&mut self, override_value: PrefabOverride) {
        if let Some(existing) = self.overrides.iter_mut().find(|existing| {
            existing.target_source_entity_id == override_value.target_source_entity_id
                && existing.component_type == override_value.component_type
                && existing.field_path == override_value.field_path
        }) {
            *existing = override_value;
        } else {
            self.overrides.push(override_value);
        }
    }

    pub fn remove_override(
        &mut self,
        target_source_entity_id: &str,
        component_type: &str,
        field_path: &str,
    ) -> Option<PrefabOverride> {
        let index = self.overrides.iter().position(|existing| {
            existing.target_source_entity_id == target_source_entity_id
                && existing.component_type == component_type
                && existing.field_path == field_path
        })?;
        Some(self.overrides.remove(index))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabRef {
    pub id: String,
    #[serde(default)]
    pub guid: Option<String>,
}

impl PrefabRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            guid: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabOverride {
    pub target_source_entity_id: String,
    pub component_type: String,
    pub field_path: String,
    pub value: serde_json::Value,
}

impl PrefabOverride {
    pub fn new(
        target_source_entity_id: impl Into<String>,
        component_type: impl Into<String>,
        field_path: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        Self {
            target_source_entity_id: target_source_entity_id.into(),
            component_type: component_type.into(),
            field_path: field_path.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPrefabView {
    pub prefab_ref: PrefabRef,
    pub instance_id: String,
    pub resolved_entities: Vec<ResolvedPrefabEntity>,
    pub applied_overrides: Vec<PrefabOverride>,
    pub diagnostics: Vec<PrefabDiagnostic>,
}

impl ResolvedPrefabView {
    pub fn resolve(asset: &PrefabAsset, instance: &PrefabInstance) -> Self {
        let mut diagnostics = Vec::new();
        if asset.prefab_id != instance.prefab_ref.id {
            diagnostics.push(
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::InvalidPrefabRef,
                    format!(
                        "PrefabInstance references {}, but asset is {}.",
                        instance.prefab_ref.id, asset.prefab_id
                    ),
                )
                .with_prefab_ref(instance.prefab_ref.id.clone())
                .with_instance_id(instance.instance_id.clone()),
            );
        }

        let mut resolved_entities = asset
            .entities
            .iter()
            .map(ResolvedPrefabEntity::from_prefab_entity)
            .collect::<Vec<_>>();
        let mut applied_overrides = Vec::new();

        for override_value in &instance.overrides {
            match apply_override(&mut resolved_entities, override_value) {
                Ok(()) => applied_overrides.push(override_value.clone()),
                Err(mut diagnostic) => {
                    diagnostic.prefab_ref = Some(instance.prefab_ref.id.clone());
                    diagnostic.instance_id = Some(instance.instance_id.clone());
                    diagnostics.push(diagnostic);
                }
            }
        }

        Self {
            prefab_ref: instance.prefab_ref.clone(),
            instance_id: instance.instance_id.clone(),
            resolved_entities,
            applied_overrides,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPrefabEntity {
    pub source_entity_id: String,
    pub name: String,
    pub parent_source_entity_id: Option<String>,
    pub sibling_order: i32,
    pub enabled: bool,
    pub transform: EditorTransform,
    pub components: Vec<EditorSceneComponent>,
}

impl ResolvedPrefabEntity {
    fn from_prefab_entity(entity: &PrefabEntity) -> Self {
        Self {
            source_entity_id: entity.source_entity_id.clone(),
            name: entity.name.clone(),
            parent_source_entity_id: entity.parent_source_entity_id.clone(),
            sibling_order: entity.sibling_order,
            enabled: entity.enabled,
            transform: entity.transform.clone(),
            components: entity.components.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefabDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefabDiagnosticCode {
    MissingPrefabAsset,
    InvalidPrefabRef,
    MissingSourceEntity,
    InvalidOverrideField,
    CyclicPrefabReference,
    ResolveFailed,
    ApplyOverrideFailed,
    RevertOverrideFailed,
    RuntimeExpandFailed,
}

impl PrefabDiagnosticCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingPrefabAsset => "missing_prefab_asset",
            Self::InvalidPrefabRef => "invalid_prefab_ref",
            Self::MissingSourceEntity => "missing_source_entity",
            Self::InvalidOverrideField => "invalid_override_field",
            Self::CyclicPrefabReference => "cyclic_prefab_reference",
            Self::ResolveFailed => "resolve_failed",
            Self::ApplyOverrideFailed => "apply_override_failed",
            Self::RevertOverrideFailed => "revert_override_failed",
            Self::RuntimeExpandFailed => "runtime_expand_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabDiagnostic {
    pub severity: PrefabDiagnosticSeverity,
    pub code: PrefabDiagnosticCode,
    pub message: String,
    pub prefab_ref: Option<String>,
    pub instance_id: Option<String>,
    pub source_entity_id: Option<String>,
    pub field_path: Option<String>,
}

impl PrefabDiagnostic {
    pub fn error(code: PrefabDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: PrefabDiagnosticSeverity::Error,
            code,
            message: message.into(),
            prefab_ref: None,
            instance_id: None,
            source_entity_id: None,
            field_path: None,
        }
    }

    pub fn with_prefab_ref(mut self, prefab_ref: impl Into<String>) -> Self {
        self.prefab_ref = Some(prefab_ref.into());
        self
    }

    pub fn with_instance_id(mut self, instance_id: impl Into<String>) -> Self {
        self.instance_id = Some(instance_id.into());
        self
    }

    pub fn with_source_entity_id(mut self, source_entity_id: impl Into<String>) -> Self {
        self.source_entity_id = Some(source_entity_id.into());
        self
    }

    pub fn with_field_path(mut self, field_path: impl Into<String>) -> Self {
        self.field_path = Some(field_path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabWorkflowReport {
    pub schema_version: String,
    pub prefab_assets_count: usize,
    pub prefab_instances_count: usize,
    pub overrides_count: usize,
    pub resolved_instances_count: usize,
    pub failed_instances_count: usize,
    pub diagnostics: Vec<PrefabDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrefabAuthoringStatus {
    Ready,
    Dirty,
    Saved,
    Invalid,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabStageModel {
    pub schema_version: String,
    pub stage_id: String,
    pub mode: PrefabStageMode,
    pub source_prefab_path: String,
    pub source_prefab_id: String,
    pub working_prefab: PrefabAsset,
    pub selected_source_entity_id: Option<String>,
    pub opened_from_instance_entity_id: Option<String>,
    pub opened_from_instance_id: Option<String>,
    pub dirty: bool,
    pub preview: ResolvedPrefabView,
    pub diagnostics: Vec<PrefabDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabStageReport {
    pub schema_version: String,
    pub stage_id: String,
    pub mode: PrefabStageMode,
    pub source_prefab_path: String,
    pub source_prefab_id: String,
    pub dirty: bool,
    pub selected_source_entity_id: Option<String>,
    pub entity_count: usize,
    pub component_count: usize,
    pub override_count_from_opened_instance: usize,
    pub diagnostics: Vec<PrefabDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabAuthoringModel {
    pub schema_version: String,
    pub active_stage: Option<PrefabStageModel>,
    pub open_prefab_paths: Vec<String>,
    pub validation_report: PrefabAuthoringReport,
}

impl Default for PrefabAuthoringModel {
    fn default() -> Self {
        Self {
            schema_version: "prefab-authoring-model.v1".to_string(),
            active_stage: None,
            open_prefab_paths: Vec::new(),
            validation_report: PrefabAuthoringReport::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefabAuthoringReport {
    pub schema_version: String,
    pub status: PrefabAuthoringStatus,
    pub project_root: Option<String>,
    pub active_stage_id: Option<String>,
    pub active_prefab_path: Option<String>,
    pub prefab_assets_count: usize,
    pub prefab_instances_count: usize,
    pub created_prefab_paths: Vec<String>,
    pub instantiated_entity_ids: Vec<String>,
    pub overrides_count: usize,
    pub applied_override_count: usize,
    pub reverted_override_count: usize,
    pub stage_report: Option<PrefabStageReport>,
    pub diagnostics: Vec<PrefabDiagnostic>,
    pub next_actions: Vec<String>,
}

impl PrefabAuthoringReport {
    pub fn empty() -> Self {
        Self {
            schema_version: PREFAB_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            status: PrefabAuthoringStatus::Ready,
            project_root: None,
            active_stage_id: None,
            active_prefab_path: None,
            prefab_assets_count: 0,
            prefab_instances_count: 0,
            created_prefab_paths: Vec::new(),
            instantiated_entity_ids: Vec::new(),
            overrides_count: 0,
            applied_override_count: 0,
            reverted_override_count: 0,
            stage_report: None,
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }

    pub fn from_parts(
        project_root: Option<String>,
        assets: &[PrefabAsset],
        instances: &[PrefabInstance],
        active_stage: Option<&PrefabStageModel>,
    ) -> Self {
        let (_views, workflow_report) = PrefabWorkflowService::resolve_instances(assets, instances);
        let stage_report = active_stage.map(PrefabStageReport::from_stage);
        let mut next_actions = Vec::new();
        if assets.is_empty() {
            next_actions.push("create_prefab_from_selection".to_string());
        }
        if instances.is_empty() {
            next_actions.push("instantiate_prefab_in_scene".to_string());
        }
        if workflow_report.failed_instances_count > 0 {
            next_actions.push("validate_prefab_references".to_string());
        }
        let status = if workflow_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == PrefabDiagnosticSeverity::Error)
        {
            PrefabAuthoringStatus::Invalid
        } else if active_stage.map_or(false, |stage| stage.dirty) {
            PrefabAuthoringStatus::Dirty
        } else {
            PrefabAuthoringStatus::Ready
        };
        Self {
            schema_version: PREFAB_AUTHORING_REPORT_SCHEMA_VERSION.to_string(),
            status,
            project_root,
            active_stage_id: active_stage.map(|stage| stage.stage_id.clone()),
            active_prefab_path: active_stage.map(|stage| stage.source_prefab_path.clone()),
            prefab_assets_count: assets.len(),
            prefab_instances_count: instances.len(),
            created_prefab_paths: Vec::new(),
            instantiated_entity_ids: Vec::new(),
            overrides_count: workflow_report.overrides_count,
            applied_override_count: 0,
            reverted_override_count: 0,
            stage_report,
            diagnostics: workflow_report.diagnostics,
            next_actions,
        }
    }
}

pub struct PrefabWorkflowService;

impl PrefabWorkflowService {
    pub fn load_asset(project_root: &Path, relative_path: &str) -> Result<PrefabAsset, String> {
        let path = project_root.join(normalize_project_relative_path(relative_path));
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read PrefabAsset {}: {error}", path.display()))?;
        let mut asset = serde_json::from_str::<PrefabAsset>(&text)
            .map_err(|error| format!("Failed to parse PrefabAsset {}: {error}", path.display()))?;
        asset.source_path = Some(relative_path.replace('\\', "/"));
        Ok(asset)
    }

    pub fn save_asset(
        project_root: &Path,
        relative_path: &str,
        asset: &PrefabAsset,
    ) -> Result<(), String> {
        let scope =
            crate::ProjectWriteScope::open(project_root).map_err(|error| error.to_string())?;
        Self::save_asset_in_scope(&scope, relative_path, asset)
    }

    pub fn save_asset_in_scope(
        scope: &crate::ProjectWriteScope,
        relative_path: &str,
        asset: &PrefabAsset,
    ) -> Result<(), String> {
        let json = serde_json::to_string_pretty(asset)
            .map_err(|error| format!("Failed to serialize PrefabAsset: {error}"))?;
        scope
            .write_atomic(relative_path, json.as_bytes())
            .map(|_| ())
            .map_err(|error| {
                format!("Failed to atomically save PrefabAsset {relative_path}: {error}")
            })
    }

    pub fn prefab_path_for_id(prefab_id: &str) -> String {
        let slug = prefab_id
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
        let slug = if slug.is_empty() { "prefab" } else { &slug };
        format!("Prefabs/{slug}.prefab.json")
    }

    pub fn enter_stage(
        source_prefab_path: impl Into<String>,
        mode: PrefabStageMode,
        asset: PrefabAsset,
        opened_from_instance: Option<&PrefabInstance>,
    ) -> PrefabStageModel {
        let source_prefab_path = source_prefab_path.into();
        let stage_id = format!("prefab-stage-{}", asset.prefab_id);
        let preview_instance = opened_from_instance.cloned().unwrap_or_else(|| {
            PrefabInstance::new(
                format!("preview-{}", asset.prefab_id),
                PrefabRef::new(asset.prefab_id.clone()),
                asset.root_entity_id.clone(),
            )
        });
        let preview = ResolvedPrefabView::resolve(&asset, &preview_instance);
        let diagnostics = validate_prefab_asset(&asset)
            .into_iter()
            .chain(preview.diagnostics.iter().cloned())
            .collect::<Vec<_>>();
        PrefabStageModel {
            schema_version: "prefab-stage-model.v1".to_string(),
            stage_id,
            mode,
            source_prefab_path,
            source_prefab_id: asset.prefab_id.clone(),
            selected_source_entity_id: Some(asset.root_entity_id.clone()),
            working_prefab: asset,
            opened_from_instance_entity_id: opened_from_instance
                .map(|instance| instance.instance_root_entity_id.clone()),
            opened_from_instance_id: opened_from_instance
                .map(|instance| instance.instance_id.clone()),
            dirty: false,
            preview,
            diagnostics,
        }
    }

    pub fn edit_stage_entity_field(
        stage: &mut PrefabStageModel,
        source_entity_id: &str,
        component_type: Option<&str>,
        field_path: &str,
        value: serde_json::Value,
    ) -> Result<(), PrefabDiagnostic> {
        if field_path.trim().is_empty() {
            return Err(PrefabDiagnostic::error(
                PrefabDiagnosticCode::InvalidOverrideField,
                "Prefab stage field path cannot be empty.",
            )
            .with_source_entity_id(source_entity_id)
            .with_field_path(field_path));
        }
        let entity = stage
            .working_prefab
            .entity_mut(source_entity_id)
            .ok_or_else(|| {
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::MissingSourceEntity,
                    format!("Prefab stage source entity is missing: {source_entity_id}"),
                )
                .with_prefab_ref(stage.source_prefab_id.clone())
                .with_source_entity_id(source_entity_id)
                .with_field_path(field_path)
            })?;
        match component_type {
            Some("engine.transform") | None if field_path.starts_with("local") => {
                apply_prefab_entity_transform_field(entity, field_path, value)?;
            }
            Some(component_type) => {
                let component = entity
                    .components
                    .iter_mut()
                    .find(|component| component.component_type == component_type)
                    .ok_or_else(|| {
                        PrefabDiagnostic::error(
                            PrefabDiagnosticCode::InvalidOverrideField,
                            format!(
                                "Prefab stage source entity {source_entity_id} does not have component {component_type}."
                            ),
                        )
                        .with_prefab_ref(stage.source_prefab_id.clone())
                        .with_source_entity_id(source_entity_id)
                        .with_field_path(field_path)
                    })?;
                set_json_field(&mut component.fields, field_path, value).map_err(|message| {
                    PrefabDiagnostic::error(PrefabDiagnosticCode::InvalidOverrideField, message)
                        .with_prefab_ref(stage.source_prefab_id.clone())
                        .with_source_entity_id(source_entity_id)
                        .with_field_path(field_path)
                })?;
            }
            None => {
                return Err(PrefabDiagnostic::error(
                    PrefabDiagnosticCode::InvalidOverrideField,
                    "Prefab stage component_type is required for non-transform fields.",
                )
                .with_prefab_ref(stage.source_prefab_id.clone())
                .with_source_entity_id(source_entity_id)
                .with_field_path(field_path));
            }
        }
        stage.selected_source_entity_id = Some(source_entity_id.to_string());
        stage.dirty = true;
        Self::refresh_stage_preview(stage);
        Ok(())
    }

    pub fn save_stage(stage: &mut PrefabStageModel) -> PrefabAsset {
        stage.working_prefab.clone()
    }

    pub fn mark_stage_saved(stage: &mut PrefabStageModel) {
        stage.dirty = false;
        Self::refresh_stage_preview(stage);
    }

    pub fn apply_override_to_asset(
        asset: &mut PrefabAsset,
        instance_entity: &mut EditorSceneEntity,
        target_source_entity_id: &str,
        component_type: &str,
        field_path: &str,
    ) -> Result<PrefabOverride, PrefabDiagnostic> {
        let instance = PrefabInstance::from_scene_entity(instance_entity)?;
        let override_value = instance
            .overrides
            .iter()
            .find(|existing| {
                existing.target_source_entity_id == target_source_entity_id
                    && existing.component_type == component_type
                    && existing.field_path == field_path
            })
            .cloned()
            .ok_or_else(|| {
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::ApplyOverrideFailed,
                    "Prefab override does not exist on this instance.",
                )
                .with_prefab_ref(instance.prefab_ref.id.clone())
                .with_instance_id(instance.instance_id.clone())
                .with_source_entity_id(target_source_entity_id)
                .with_field_path(field_path)
            })?;
        Self::apply_override_to_prefab_asset(asset, &override_value)?;
        Self::revert_override(
            instance_entity,
            target_source_entity_id,
            component_type,
            field_path,
        )?;
        Ok(override_value)
    }

    pub fn apply_override_to_prefab_asset(
        asset: &mut PrefabAsset,
        override_value: &PrefabOverride,
    ) -> Result<(), PrefabDiagnostic> {
        let prefab_id = asset.prefab_id.clone();
        let entity = asset
            .entity_mut(&override_value.target_source_entity_id)
            .ok_or_else(|| {
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::MissingSourceEntity,
                    format!(
                        "Cannot apply override to missing source entity: {}",
                        override_value.target_source_entity_id
                    ),
                )
                .with_prefab_ref(prefab_id.clone())
                .with_source_entity_id(override_value.target_source_entity_id.clone())
                .with_field_path(override_value.field_path.clone())
            })?;
        if override_value.component_type == "engine.transform" {
            apply_prefab_entity_transform_field(
                entity,
                &override_value.field_path,
                override_value.value.clone(),
            )
        } else {
            let component = entity
                .components
                .iter_mut()
                .find(|component| component.component_type == override_value.component_type)
                .ok_or_else(|| {
                    PrefabDiagnostic::error(
                        PrefabDiagnosticCode::InvalidOverrideField,
                        format!(
                            "Cannot apply override to missing component {}.",
                            override_value.component_type
                        ),
                    )
                    .with_prefab_ref(prefab_id.clone())
                    .with_source_entity_id(override_value.target_source_entity_id.clone())
                    .with_field_path(override_value.field_path.clone())
                })?;
            set_json_field(
                &mut component.fields,
                &override_value.field_path,
                override_value.value.clone(),
            )
            .map_err(|message| {
                PrefabDiagnostic::error(PrefabDiagnosticCode::InvalidOverrideField, message)
                    .with_prefab_ref(prefab_id.clone())
                    .with_source_entity_id(override_value.target_source_entity_id.clone())
                    .with_field_path(override_value.field_path.clone())
            })
        }
    }

    pub fn refresh_stage_preview(stage: &mut PrefabStageModel) {
        let instance = PrefabInstance::new(
            format!("preview-{}", stage.working_prefab.prefab_id),
            PrefabRef::new(stage.working_prefab.prefab_id.clone()),
            stage.working_prefab.root_entity_id.clone(),
        );
        stage.preview = ResolvedPrefabView::resolve(&stage.working_prefab, &instance);
        stage.diagnostics = validate_prefab_asset(&stage.working_prefab)
            .into_iter()
            .chain(stage.preview.diagnostics.iter().cloned())
            .collect();
    }

    pub fn resolve_instances(
        assets: &[PrefabAsset],
        instances: &[PrefabInstance],
    ) -> (Vec<ResolvedPrefabView>, PrefabWorkflowReport) {
        let assets_by_id = assets
            .iter()
            .map(|asset| (asset.prefab_id.as_str(), asset))
            .collect::<BTreeMap<_, _>>();
        let mut views = Vec::new();
        let mut missing_diagnostics = Vec::new();
        for instance in instances {
            if let Some(asset) = assets_by_id.get(instance.prefab_ref.id.as_str()) {
                views.push(ResolvedPrefabView::resolve(asset, instance));
            } else {
                missing_diagnostics.push(
                    PrefabDiagnostic::error(
                        PrefabDiagnosticCode::MissingPrefabAsset,
                        format!("Prefab asset is missing: {}", instance.prefab_ref.id),
                    )
                    .with_prefab_ref(instance.prefab_ref.id.clone())
                    .with_instance_id(instance.instance_id.clone()),
                );
            }
        }
        let overrides_count = instances
            .iter()
            .map(|instance| instance.overrides.len())
            .sum::<usize>();
        let mut report = PrefabWorkflowReport::from_views(
            assets.len(),
            instances.len(),
            overrides_count,
            &views,
        );
        report.diagnostics.extend(missing_diagnostics);
        report.failed_instances_count += report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PrefabDiagnosticCode::MissingPrefabAsset)
            .count();
        report.resolved_instances_count = views
            .iter()
            .filter(|view| {
                !view
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == PrefabDiagnosticSeverity::Error)
            })
            .count();
        (views, report)
    }

    pub fn create_prefab_asset_from_entity_tree(
        prefab_id: impl Into<String>,
        name: impl Into<String>,
        root: &EditorSceneEntity,
        scene_entities: &[EditorSceneEntity],
    ) -> PrefabAsset {
        let mut children_by_parent: BTreeMap<String, Vec<EditorSceneEntity>> = BTreeMap::new();
        for entity in scene_entities {
            if let Some(parent_id) = &entity.parent_id {
                children_by_parent
                    .entry(parent_id.clone())
                    .or_default()
                    .push(entity.clone());
            }
        }
        PrefabAsset::from_entity_tree(prefab_id, name, root, &children_by_parent)
    }

    pub fn create_scene_instance_entity(
        prefab_ref: PrefabRef,
        entity_id: impl Into<String>,
        name: impl Into<String>,
        parent_id: Option<String>,
        local_position: Option<Vec3>,
    ) -> EditorSceneEntity {
        let entity_id = entity_id.into();
        let mut transform = EditorTransform::identity();
        if let Some(local_position) = local_position {
            transform.local_position = crate::EditorVec3 {
                x: local_position.x,
                y: local_position.y,
                z: local_position.z,
            };
        }
        let mut instance = PrefabInstance::new(
            format!("prefab-instance-{entity_id}"),
            prefab_ref,
            entity_id.clone(),
        );
        instance.scene_parent_entity_id = parent_id.clone();
        EditorSceneEntity {
            schema_version: crate::EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION.to_string(),
            entity_id,
            name: name.into(),
            kind: "prefab_instance".to_string(),
            enabled: true,
            parent_id,
            sibling_order: 0,
            transform: Some(transform),
            mesh: None,
            components: vec![instance.to_scene_component()],
        }
    }

    pub fn override_from_inspector_edit(
        instance_entity: &EditorSceneEntity,
        component_type: impl Into<String>,
        field_path: impl Into<String>,
        value: serde_json::Value,
    ) -> Result<PrefabOverride, PrefabDiagnostic> {
        let _instance = PrefabInstance::from_scene_entity(instance_entity)?;
        let component_type = component_type.into();
        let field_path = field_path.into();
        let target_source_entity_id =
            source_entity_id_for_instance_edit(instance_entity, &component_type)?;
        Ok(PrefabOverride::new(
            target_source_entity_id,
            component_type,
            field_path,
            value,
        ))
    }

    pub fn write_override_to_instance_entity(
        instance_entity: &mut EditorSceneEntity,
        override_value: PrefabOverride,
    ) -> Result<PrefabInstance, PrefabDiagnostic> {
        let mut instance = PrefabInstance::from_scene_entity(instance_entity)?;
        instance.set_override(override_value);
        let Some(component) = instance_entity
            .components
            .iter_mut()
            .find(|component| component.component_type == PREFAB_INSTANCE_COMPONENT_TYPE)
        else {
            return Err(PrefabDiagnostic::error(
                PrefabDiagnosticCode::InvalidPrefabRef,
                "Prefab instance component is missing.",
            )
            .with_source_entity_id(instance_entity.entity_id.clone()));
        };
        component.fields = instance.to_scene_component().fields;
        Ok(instance)
    }

    pub fn revert_override(
        instance_entity: &mut EditorSceneEntity,
        target_source_entity_id: &str,
        component_type: &str,
        field_path: &str,
    ) -> Result<Option<PrefabOverride>, PrefabDiagnostic> {
        let mut instance = PrefabInstance::from_scene_entity(instance_entity)?;
        let removed = instance.remove_override(target_source_entity_id, component_type, field_path);
        let Some(component) = instance_entity
            .components
            .iter_mut()
            .find(|component| component.component_type == PREFAB_INSTANCE_COMPONENT_TYPE)
        else {
            return Err(PrefabDiagnostic::error(
                PrefabDiagnosticCode::InvalidPrefabRef,
                "Prefab instance component is missing.",
            )
            .with_source_entity_id(instance_entity.entity_id.clone()));
        };
        component.fields = instance.to_scene_component().fields;
        Ok(removed)
    }
}

impl PrefabWorkflowReport {
    pub fn from_views(
        prefab_assets_count: usize,
        prefab_instances_count: usize,
        overrides_count: usize,
        views: &[ResolvedPrefabView],
    ) -> Self {
        let diagnostics = views
            .iter()
            .flat_map(|view| view.diagnostics.iter().cloned())
            .collect::<Vec<_>>();
        let failed_instances = views
            .iter()
            .filter(|view| {
                view.diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == PrefabDiagnosticSeverity::Error)
            })
            .count();
        Self {
            schema_version: PREFAB_WORKFLOW_REPORT_SCHEMA_VERSION.to_string(),
            prefab_assets_count,
            prefab_instances_count,
            overrides_count,
            resolved_instances_count: views.len().saturating_sub(failed_instances),
            failed_instances_count: failed_instances,
            diagnostics,
        }
    }
}

impl PrefabStageReport {
    pub fn from_stage(stage: &PrefabStageModel) -> Self {
        let component_count = stage
            .working_prefab
            .entities
            .iter()
            .map(|entity| entity.components.len())
            .sum::<usize>();
        let override_count_from_opened_instance = stage.preview.applied_overrides.len();
        let mut next_actions = Vec::new();
        if stage.dirty {
            next_actions.push("save_prefab_document".to_string());
        }
        if stage
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == PrefabDiagnosticSeverity::Error)
        {
            next_actions.push("fix_prefab_stage_diagnostics".to_string());
        }
        Self {
            schema_version: PREFAB_STAGE_REPORT_SCHEMA_VERSION.to_string(),
            stage_id: stage.stage_id.clone(),
            mode: stage.mode,
            source_prefab_path: stage.source_prefab_path.clone(),
            source_prefab_id: stage.source_prefab_id.clone(),
            dirty: stage.dirty,
            selected_source_entity_id: stage.selected_source_entity_id.clone(),
            entity_count: stage.working_prefab.entities.len(),
            component_count,
            override_count_from_opened_instance,
            diagnostics: stage.diagnostics.clone(),
            next_actions,
        }
    }
}

pub fn validate_prefab_asset(asset: &PrefabAsset) -> Vec<PrefabDiagnostic> {
    let mut diagnostics = Vec::new();
    let entity_ids = asset
        .entities
        .iter()
        .map(|entity| entity.source_entity_id.as_str())
        .collect::<BTreeSet<_>>();
    if !entity_ids.contains(asset.root_entity_id.as_str()) {
        diagnostics.push(
            PrefabDiagnostic::error(
                PrefabDiagnosticCode::MissingSourceEntity,
                format!("Prefab root entity is missing: {}", asset.root_entity_id),
            )
            .with_prefab_ref(asset.prefab_id.clone())
            .with_source_entity_id(asset.root_entity_id.clone()),
        );
    }
    for entity in &asset.entities {
        if let Some(parent_id) = &entity.parent_source_entity_id {
            if !entity_ids.contains(parent_id.as_str()) {
                diagnostics.push(
                    PrefabDiagnostic::error(
                        PrefabDiagnosticCode::MissingSourceEntity,
                        format!(
                            "Prefab entity {} references missing parent {}.",
                            entity.source_entity_id, parent_id
                        ),
                    )
                    .with_prefab_ref(asset.prefab_id.clone())
                    .with_source_entity_id(entity.source_entity_id.clone()),
                );
            }
        }
    }
    diagnostics
}

pub fn detect_cyclic_prefab_references(
    prefabs: &[PrefabAsset],
    references: &BTreeMap<String, Vec<String>>,
) -> Vec<PrefabDiagnostic> {
    let known = prefabs
        .iter()
        .map(|prefab| prefab.prefab_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();
    for prefab in prefabs {
        let mut visiting = BTreeSet::new();
        if has_prefab_cycle(&prefab.prefab_id, references, &known, &mut visiting) {
            diagnostics.push(
                PrefabDiagnostic::error(
                    PrefabDiagnosticCode::CyclicPrefabReference,
                    format!("Prefab has cyclic reference: {}", prefab.prefab_id),
                )
                .with_prefab_ref(prefab.prefab_id.clone()),
            );
        }
    }
    diagnostics
}

fn has_prefab_cycle<'a>(
    prefab_id: &'a str,
    references: &'a BTreeMap<String, Vec<String>>,
    known: &BTreeSet<&'a str>,
    visiting: &mut BTreeSet<&'a str>,
) -> bool {
    if !visiting.insert(prefab_id) {
        return true;
    }
    for target in references.get(prefab_id).into_iter().flatten() {
        if known.contains(target.as_str())
            && has_prefab_cycle(target.as_str(), references, known, visiting)
        {
            return true;
        }
    }
    visiting.remove(prefab_id);
    false
}

fn apply_override(
    resolved_entities: &mut [ResolvedPrefabEntity],
    override_value: &PrefabOverride,
) -> Result<(), PrefabDiagnostic> {
    let entity = resolved_entities
        .iter_mut()
        .find(|entity| entity.source_entity_id == override_value.target_source_entity_id)
        .ok_or_else(|| {
            PrefabDiagnostic::error(
                PrefabDiagnosticCode::MissingSourceEntity,
                format!(
                    "Override references missing source entity: {}",
                    override_value.target_source_entity_id
                ),
            )
            .with_source_entity_id(override_value.target_source_entity_id.clone())
            .with_field_path(override_value.field_path.clone())
        })?;

    if override_value.component_type == "engine.transform" {
        return apply_transform_override(entity, override_value);
    }

    let component = entity
        .components
        .iter_mut()
        .find(|component| component.component_type == override_value.component_type)
        .ok_or_else(|| {
            PrefabDiagnostic::error(
                PrefabDiagnosticCode::InvalidOverrideField,
                format!(
                    "Override references missing component {} on entity {}.",
                    override_value.component_type, override_value.target_source_entity_id
                ),
            )
            .with_source_entity_id(override_value.target_source_entity_id.clone())
            .with_field_path(override_value.field_path.clone())
        })?;

    set_json_field(
        &mut component.fields,
        &override_value.field_path,
        override_value.value.clone(),
    )
    .map_err(|message| {
        PrefabDiagnostic::error(PrefabDiagnosticCode::InvalidOverrideField, message)
            .with_source_entity_id(override_value.target_source_entity_id.clone())
            .with_field_path(override_value.field_path.clone())
    })
}

fn apply_transform_override(
    entity: &mut ResolvedPrefabEntity,
    override_value: &PrefabOverride,
) -> Result<(), PrefabDiagnostic> {
    match override_value.field_path.as_str() {
        "localPosition" => {
            entity.transform.local_position = serde_json::from_value(override_value.value.clone())
                .map_err(|_| invalid_transform_override(override_value))?;
        }
        "localRotation" => {
            entity.transform.local_rotation = serde_json::from_value(override_value.value.clone())
                .map_err(|_| invalid_transform_override(override_value))?;
        }
        "localScale" => {
            entity.transform.local_scale = serde_json::from_value(override_value.value.clone())
                .map_err(|_| invalid_transform_override(override_value))?;
        }
        _ => return Err(invalid_transform_override(override_value)),
    }
    Ok(())
}

fn invalid_transform_override(override_value: &PrefabOverride) -> PrefabDiagnostic {
    PrefabDiagnostic::error(
        PrefabDiagnosticCode::InvalidOverrideField,
        format!(
            "Invalid transform override field: {}",
            override_value.field_path
        ),
    )
    .with_source_entity_id(override_value.target_source_entity_id.clone())
    .with_field_path(override_value.field_path.clone())
}

fn apply_prefab_entity_transform_field(
    entity: &mut PrefabEntity,
    field_path: &str,
    value: serde_json::Value,
) -> Result<(), PrefabDiagnostic> {
    match field_path {
        "localPosition" => {
            entity.transform.local_position =
                serde_json::from_value(value).map_err(|_| invalid_stage_transform(field_path))?;
        }
        "localRotation" => {
            entity.transform.local_rotation =
                serde_json::from_value(value).map_err(|_| invalid_stage_transform(field_path))?;
        }
        "localScale" => {
            entity.transform.local_scale =
                serde_json::from_value(value).map_err(|_| invalid_stage_transform(field_path))?;
        }
        _ => return Err(invalid_stage_transform(field_path)),
    }
    Ok(())
}

fn invalid_stage_transform(field_path: &str) -> PrefabDiagnostic {
    PrefabDiagnostic::error(
        PrefabDiagnosticCode::InvalidOverrideField,
        format!("Invalid prefab transform field: {field_path}"),
    )
    .with_field_path(field_path)
}

fn set_json_field(
    root: &mut serde_json::Value,
    field_path: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let segments = field_path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err("Override field path cannot be empty.".to_string());
    }
    let mut current = root;
    for segment in &segments[..segments.len() - 1] {
        let Some(next) = current.get_mut(*segment) else {
            return Err(format!("Override field segment is missing: {segment}"));
        };
        current = next;
    }
    let last = segments[segments.len() - 1];
    let Some(object) = current.as_object_mut() else {
        return Err(format!(
            "Override field parent is not an object: {}",
            field_path
        ));
    };
    if !object.contains_key(last) {
        return Err(format!("Override field is missing: {field_path}"));
    }
    object.insert(last.to_string(), value);
    Ok(())
}

fn collect_prefab_entities(
    entity: &EditorSceneEntity,
    children_by_parent: &BTreeMap<String, Vec<EditorSceneEntity>>,
    output: &mut Vec<PrefabEntity>,
) {
    output.push(PrefabEntity::from_scene_entity(entity));
    if let Some(children) = children_by_parent.get(&entity.entity_id) {
        for child in children {
            collect_prefab_entities(child, children_by_parent, output);
        }
    }
}

fn prefab_asset_refs_from_entity(entity: &EditorSceneEntity) -> Vec<PrefabAssetRef> {
    let mut refs = Vec::new();
    if let Some(mesh) = &entity.mesh {
        for asset_ref in [mesh.asset_ref.as_ref(), mesh.material_ref.as_ref()]
            .into_iter()
            .flatten()
        {
            refs.push(PrefabAssetRef {
                id: asset_ref.asset_id.clone(),
                asset_type: asset_ref.asset_type_id.clone(),
                guid: None,
            });
        }
    }
    refs
}

fn source_entity_id_for_instance_edit(
    instance_entity: &EditorSceneEntity,
    component_type: &str,
) -> Result<String, PrefabDiagnostic> {
    if component_type == "engine.transform" {
        return Ok(instance_entity.entity_id.clone());
    }
    Ok(instance_entity.entity_id.clone())
}

fn normalize_project_relative_path(path: &str) -> PathBuf {
    let normalized = path.replace('\\', "/");
    normalized
        .split('/')
        .filter(|segment| {
            !segment.is_empty() && *segment != "." && *segment != ".." && !segment.contains(':')
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EditorSceneComponent, EditorSceneEntity, EditorTransform, EditorVec3};

    fn entity(id: &str, parent_id: Option<&str>) -> EditorSceneEntity {
        EditorSceneEntity {
            schema_version: crate::EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION.to_string(),
            entity_id: id.to_string(),
            name: id.to_string(),
            kind: "entity".to_string(),
            enabled: true,
            parent_id: parent_id.map(str::to_string),
            sibling_order: 0,
            transform: Some(EditorTransform::identity()),
            mesh: None,
            components: vec![EditorSceneComponent {
                component_type: "project.stats".to_string(),
                fields: serde_json::json!({ "speed": 1.0, "flags": { "enabled": true } }),
            }],
        }
    }

    #[test]
    fn prefab_asset_can_be_created_from_entity_tree() {
        let root = entity("entity-root", None);
        let child = entity("entity-child", Some("entity-root"));
        let mut children = BTreeMap::new();
        children.insert("entity-root".to_string(), vec![child]);

        let asset = PrefabAsset::from_entity_tree("prefab-ship", "Ship", &root, &children);

        assert_eq!(asset.prefab_id, "prefab-ship");
        assert_eq!(asset.root_entity_id, "entity-root");
        assert_eq!(asset.entities.len(), 2);
        assert!(validate_prefab_asset(&asset).is_empty());
    }

    #[test]
    fn prefab_instance_applies_component_override_to_resolved_view() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let mut instance =
            PrefabInstance::new("instance-1", PrefabRef::new("prefab-ship"), "scene-root");
        instance.set_override(PrefabOverride::new(
            "entity-root",
            "project.stats",
            "speed",
            serde_json::json!(4.0),
        ));

        let view = ResolvedPrefabView::resolve(&asset, &instance);

        assert!(view.diagnostics.is_empty());
        assert_eq!(view.applied_overrides.len(), 1);
        assert_eq!(
            view.resolved_entities[0].components[0].fields["speed"],
            serde_json::json!(4.0)
        );
    }

    #[test]
    fn prefab_instance_can_roundtrip_through_scene_component() {
        let instance = PrefabInstance::new(
            "instance-1",
            PrefabRef {
                id: "prefab-ship".to_string(),
                guid: Some("guid-ship".to_string()),
            },
            "scene-root",
        );
        let mut scene_entity = entity("scene-root", None);
        scene_entity.components = vec![instance.to_scene_component()];

        let parsed = PrefabInstance::from_scene_entity(&scene_entity).unwrap();

        assert_eq!(parsed.instance_id, "instance-1");
        assert_eq!(parsed.prefab_ref.id, "prefab-ship");
        assert_eq!(parsed.prefab_ref.guid.as_deref(), Some("guid-ship"));
    }

    #[test]
    fn prefab_workflow_service_writes_override_to_instance_entity() {
        let mut scene_entity = PrefabWorkflowService::create_scene_instance_entity(
            PrefabRef::new("prefab-ship"),
            "scene-root",
            "Ship",
            None,
            None,
        );
        let override_value = PrefabWorkflowService::override_from_inspector_edit(
            &scene_entity,
            "project.stats",
            "speed",
            serde_json::json!(8.0),
        )
        .unwrap();

        let instance = PrefabWorkflowService::write_override_to_instance_entity(
            &mut scene_entity,
            override_value,
        )
        .unwrap();

        assert_eq!(instance.overrides.len(), 1);
        assert_eq!(
            scene_entity.components[0].fields["overrides"][0]["fieldPath"],
            serde_json::json!("speed")
        );
    }

    #[test]
    fn prefab_stage_model_edits_working_prefab_and_reports_dirty() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let mut stage = PrefabWorkflowService::enter_stage(
            "Prefabs/ship.prefab.json",
            PrefabStageMode::Isolated,
            asset,
            None,
        );

        PrefabWorkflowService::edit_stage_entity_field(
            &mut stage,
            "entity-root",
            Some("project.stats"),
            "speed",
            serde_json::json!(6.0),
        )
        .unwrap();

        let report = PrefabStageReport::from_stage(&stage);
        assert!(stage.dirty);
        assert_eq!(report.entity_count, 1);
        assert_eq!(report.component_count, 1);
        assert_eq!(
            stage.working_prefab.entities[0].components[0].fields["speed"],
            serde_json::json!(6.0)
        );

        let saved = PrefabWorkflowService::save_stage(&mut stage);
        assert!(stage.dirty);
        assert_eq!(
            saved.entities[0].components[0].fields["speed"],
            serde_json::json!(6.0)
        );
        PrefabWorkflowService::mark_stage_saved(&mut stage);
        assert!(!stage.dirty);
    }

    #[test]
    fn prefab_stage_model_edits_transform_field() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let mut stage = PrefabWorkflowService::enter_stage(
            "Prefabs/ship.prefab.json",
            PrefabStageMode::Isolated,
            asset,
            None,
        );

        PrefabWorkflowService::edit_stage_entity_field(
            &mut stage,
            "entity-root",
            Some("engine.transform"),
            "localPosition",
            serde_json::json!({ "x": 1.0, "y": 2.0, "z": 3.0 }),
        )
        .unwrap();

        assert_eq!(
            stage.working_prefab.entities[0].transform.local_position,
            EditorVec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        );
    }

    #[test]
    fn prefab_override_can_apply_to_asset_and_revert_instance_override() {
        let mut asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "scene-root",
            vec![PrefabEntity::from_scene_entity(&entity("scene-root", None))],
        );
        let mut instance_entity = PrefabWorkflowService::create_scene_instance_entity(
            PrefabRef::new("prefab-ship"),
            "scene-root",
            "Ship",
            None,
            None,
        );
        let override_value = PrefabOverride::new(
            "scene-root",
            "project.stats",
            "speed",
            serde_json::json!(9.0),
        );
        PrefabWorkflowService::write_override_to_instance_entity(
            &mut instance_entity,
            override_value,
        )
        .unwrap();

        let applied = PrefabWorkflowService::apply_override_to_asset(
            &mut asset,
            &mut instance_entity,
            "scene-root",
            "project.stats",
            "speed",
        )
        .unwrap();
        let instance = PrefabInstance::from_scene_entity(&instance_entity).unwrap();

        assert_eq!(applied.value, serde_json::json!(9.0));
        assert!(instance.overrides.is_empty());
        assert_eq!(
            asset.entities[0].components[0].fields["speed"],
            serde_json::json!(9.0)
        );
    }

    #[test]
    fn prefab_authoring_report_counts_assets_instances_and_next_actions() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let instance =
            PrefabInstance::new("instance-1", PrefabRef::new("prefab-ship"), "scene-root");
        let report = PrefabAuthoringReport::from_parts(
            Some("project".to_string()),
            &[asset],
            &[instance],
            None,
        );

        assert_eq!(report.prefab_assets_count, 1);
        assert_eq!(report.prefab_instances_count, 1);
        assert_eq!(report.status, PrefabAuthoringStatus::Ready);
        assert!(report.next_actions.is_empty());
    }

    #[test]
    fn prefab_instance_applies_transform_override_to_resolved_view() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let mut instance =
            PrefabInstance::new("instance-1", PrefabRef::new("prefab-ship"), "scene-root");
        instance.set_override(PrefabOverride::new(
            "entity-root",
            "engine.transform",
            "localPosition",
            serde_json::json!({ "x": 3.0, "y": 2.0, "z": 0.0 }),
        ));

        let view = ResolvedPrefabView::resolve(&asset, &instance);

        assert!(view.diagnostics.is_empty());
        assert_eq!(
            view.resolved_entities[0].transform.local_position,
            EditorVec3 {
                x: 3.0,
                y: 2.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn prefab_instance_reports_invalid_override_field() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let mut instance =
            PrefabInstance::new("instance-1", PrefabRef::new("prefab-ship"), "scene-root");
        instance.set_override(PrefabOverride::new(
            "entity-root",
            "project.stats",
            "missing",
            serde_json::json!(4.0),
        ));

        let view = ResolvedPrefabView::resolve(&asset, &instance);

        assert_eq!(view.diagnostics.len(), 1);
        assert_eq!(
            view.diagnostics[0].code,
            PrefabDiagnosticCode::InvalidOverrideField
        );
    }

    #[test]
    fn prefab_workflow_report_counts_failed_instances() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let mut instance =
            PrefabInstance::new("instance-1", PrefabRef::new("prefab-ship"), "scene-root");
        instance.set_override(PrefabOverride::new(
            "missing-entity",
            "project.stats",
            "speed",
            serde_json::json!(2.0),
        ));
        let view = ResolvedPrefabView::resolve(&asset, &instance);

        let report = PrefabWorkflowReport::from_views(1, 1, 1, &[view]);

        assert_eq!(report.prefab_assets_count, 1);
        assert_eq!(report.failed_instances_count, 1);
        assert_eq!(report.diagnostics[0].code.as_str(), "missing_source_entity");
    }

    #[test]
    fn prefab_workflow_report_valid_project_has_zero_failed_instances() {
        let asset = PrefabAsset::new(
            "prefab-ship",
            "Ship",
            "entity-root",
            vec![PrefabEntity::from_scene_entity(&entity(
                "entity-root",
                None,
            ))],
        );
        let instance =
            PrefabInstance::new("instance-1", PrefabRef::new("prefab-ship"), "scene-root");

        let (_views, report) = PrefabWorkflowService::resolve_instances(&[asset], &[instance]);

        assert_eq!(report.resolved_instances_count, 1);
        assert_eq!(report.failed_instances_count, 0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn prefab_workflow_report_missing_prefab_has_diagnostic() {
        let instance =
            PrefabInstance::new("instance-1", PrefabRef::new("missing-prefab"), "scene-root");

        let (_views, report) = PrefabWorkflowService::resolve_instances(&[], &[instance]);

        assert_eq!(report.failed_instances_count, 1);
        assert_eq!(
            report.diagnostics[0].code,
            PrefabDiagnosticCode::MissingPrefabAsset
        );
        assert_eq!(
            report.diagnostics[0].prefab_ref.as_deref(),
            Some("missing-prefab")
        );
    }

    #[test]
    fn cyclic_prefab_reference_is_reported() {
        let a = PrefabAsset::new(
            "prefab-a",
            "A",
            "entity-a",
            vec![PrefabEntity::from_scene_entity(&entity("entity-a", None))],
        );
        let b = PrefabAsset::new(
            "prefab-b",
            "B",
            "entity-b",
            vec![PrefabEntity::from_scene_entity(&entity("entity-b", None))],
        );
        let mut references = BTreeMap::new();
        references.insert("prefab-a".to_string(), vec!["prefab-b".to_string()]);
        references.insert("prefab-b".to_string(), vec!["prefab-a".to_string()]);

        let diagnostics = detect_cyclic_prefab_references(&[a, b], &references);

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == PrefabDiagnosticCode::CyclicPrefabReference));
    }
}
