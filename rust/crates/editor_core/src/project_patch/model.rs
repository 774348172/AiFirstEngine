use editor_ui_model::{AssetKind, AssetPlacementMode, InputActionValueKind, Vec3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PROJECT_PATCH_SCHEMA_VERSION: &str = "project-patch.v1";
pub const PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION: &str = "project-patch-import-request.v1";
pub const PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION: &str = "project-patch-import-result.v1";
pub const PROJECT_PATCH_IMPORT_PRODUCTIZATION_REPORT_SCHEMA_VERSION: &str =
    "project-patch-import-productization-report.v1";
pub const PROJECT_PATCH_PRODUCTIZATION_REPORT_SCHEMA_VERSION: &str =
    "project-patch-productization-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PatchSource {
    AiAssistant,
    Test,
    ImportedPatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PatchCapability {
    Scene,
    Input,
    Asset,
    Prefab,
    Aui,
    Rule,
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PatchRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectPatchImportSourceKind {
    JsonString,
    FilePath,
    TestFixture,
    AiStructuredOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectPatchImportParseStatus {
    Parsed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectPatchImportProductizationStatus {
    Pass,
    Partial,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatchImportRequest {
    pub schema_version: String,
    pub source_kind: ProjectPatchImportSourceKind,
    pub source_label: String,
    pub project_root: Option<String>,
    pub raw_json: Option<String>,
    pub file_path: Option<String>,
    pub expected_patch_id: Option<String>,
    pub dry_run: bool,
}

impl ProjectPatchImportRequest {
    pub fn json_string(source_label: impl Into<String>, raw_json: impl Into<String>) -> Self {
        Self {
            schema_version: PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION.to_string(),
            source_kind: ProjectPatchImportSourceKind::JsonString,
            source_label: source_label.into(),
            project_root: None,
            raw_json: Some(raw_json.into()),
            file_path: None,
            expected_patch_id: None,
            dry_run: true,
        }
    }

    pub fn file_path(source_label: impl Into<String>, file_path: impl Into<String>) -> Self {
        Self {
            schema_version: PROJECT_PATCH_IMPORT_REQUEST_SCHEMA_VERSION.to_string(),
            source_kind: ProjectPatchImportSourceKind::FilePath,
            source_label: source_label.into(),
            project_root: None,
            raw_json: None,
            file_path: Some(file_path.into()),
            expected_patch_id: None,
            dry_run: true,
        }
    }

    pub fn test_fixture(source_label: impl Into<String>, raw_json: impl Into<String>) -> Self {
        Self {
            source_kind: ProjectPatchImportSourceKind::TestFixture,
            ..Self::json_string(source_label, raw_json)
        }
    }

    pub fn ai_structured_output(
        source_label: impl Into<String>,
        raw_json: impl Into<String>,
    ) -> Self {
        Self {
            source_kind: ProjectPatchImportSourceKind::AiStructuredOutput,
            ..Self::json_string(source_label, raw_json)
        }
    }

    pub fn with_expected_patch_id(mut self, patch_id: impl Into<String>) -> Self {
        self.expected_patch_id = Some(patch_id.into());
        self
    }

    pub fn with_project_root(mut self, project_root: impl Into<String>) -> Self {
        self.project_root = Some(project_root.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatchImportResult {
    pub schema_version: String,
    pub source_kind: ProjectPatchImportSourceKind,
    pub source_label: String,
    pub parse_status: ProjectPatchImportParseStatus,
    pub parsed_patch: Option<ProjectPatchDocument>,
    pub schema_diagnostics: Vec<PatchDiagnostic>,
    pub capability_diagnostics: Vec<PatchDiagnostic>,
    pub validation: Option<PatchValidationReport>,
    pub review: Option<PatchReviewModel>,
    pub proposal_id: Option<String>,
    pub next_actions: Vec<String>,
}

impl ProjectPatchImportResult {
    pub fn rejected(
        request: &ProjectPatchImportRequest,
        diagnostics: Vec<PatchDiagnostic>,
        next_actions: Vec<String>,
    ) -> Self {
        Self {
            schema_version: PROJECT_PATCH_IMPORT_RESULT_SCHEMA_VERSION.to_string(),
            source_kind: request.source_kind,
            source_label: request.source_label.clone(),
            parse_status: ProjectPatchImportParseStatus::Rejected,
            parsed_patch: None,
            schema_diagnostics: diagnostics,
            capability_diagnostics: Vec::new(),
            validation: None,
            review: None,
            proposal_id: None,
            next_actions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatchDocument {
    pub schema_version: String,
    pub patch_id: String,
    pub title: String,
    pub source: PatchSource,
    pub intent_summary: String,
    pub target_project_root: Option<String>,
    pub required_capabilities: Vec<PatchCapability>,
    pub operations: Vec<PatchOperation>,
    pub expected_outcome: String,
    pub risk_level: PatchRiskLevel,
    pub created_at: String,
}

impl ProjectPatchDocument {
    pub fn new(
        patch_id: impl Into<String>,
        title: impl Into<String>,
        source: PatchSource,
        operations: Vec<PatchOperation>,
    ) -> Self {
        let required_capabilities = capabilities_for_operations(&operations);
        Self {
            schema_version: PROJECT_PATCH_SCHEMA_VERSION.to_string(),
            patch_id: patch_id.into(),
            title: title.into(),
            source,
            intent_summary: String::new(),
            target_project_root: None,
            required_capabilities,
            operations,
            expected_outcome: String::new(),
            risk_level: PatchRiskLevel::Low,
            created_at: "0".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "domain",
    content = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PatchOperation {
    Scene(ScenePatchOperation),
    Input(InputPatchOperation),
    Asset(AssetPatchOperation),
    Prefab(PrefabPatchOperation),
    Aui(AuiPatchOperation),
    Rule(RulePatchOperation),
    Build(BuildPatchOperation),
}

impl PatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Scene(operation) => operation.operation_id(),
            Self::Input(operation) => operation.operation_id(),
            Self::Asset(operation) => operation.operation_id(),
            Self::Prefab(operation) => operation.operation_id(),
            Self::Aui(operation) => operation.operation_id(),
            Self::Rule(operation) => operation.operation_id(),
            Self::Build(operation) => operation.operation_id(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Scene(operation) => operation.kind(),
            Self::Input(operation) => operation.kind(),
            Self::Asset(operation) => operation.kind(),
            Self::Prefab(operation) => operation.kind(),
            Self::Aui(operation) => operation.kind(),
            Self::Rule(operation) => operation.kind(),
            Self::Build(operation) => operation.kind(),
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::Scene(operation) => operation.depends_on(),
            Self::Input(operation) => operation.depends_on(),
            Self::Asset(operation) => operation.depends_on(),
            Self::Prefab(operation) => operation.depends_on(),
            Self::Aui(operation) => operation.depends_on(),
            Self::Rule(operation) => operation.depends_on(),
            Self::Build(operation) => operation.depends_on(),
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::Scene(operation) => operation.target_summary(),
            Self::Input(operation) => operation.target_summary(),
            Self::Asset(operation) => operation.target_summary(),
            Self::Prefab(operation) => operation.target_summary(),
            Self::Aui(operation) => operation.target_summary(),
            Self::Rule(operation) => operation.target_summary(),
            Self::Build(operation) => operation.target_summary(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScenePatchOperation {
    CreateEntity {
        operation_id: String,
        depends_on: Vec<String>,
        parent_id: Option<String>,
        name: String,
    },
    DeleteEntity {
        operation_id: String,
        depends_on: Vec<String>,
        entity_id: String,
    },
    RenameEntity {
        operation_id: String,
        depends_on: Vec<String>,
        entity_id: String,
        name: String,
    },
    SetTransform {
        operation_id: String,
        depends_on: Vec<String>,
        entity_id: String,
        local_position: Option<Vec3>,
        local_rotation: Option<Vec3>,
        local_scale: Option<Vec3>,
    },
    AddComponent {
        operation_id: String,
        depends_on: Vec<String>,
        entity_id: String,
        component_type: String,
        fields: serde_json::Value,
    },
    RemoveComponent {
        operation_id: String,
        depends_on: Vec<String>,
        entity_id: String,
        component_type: String,
    },
    SetComponentField {
        operation_id: String,
        depends_on: Vec<String>,
        entity_id: String,
        component_type: String,
        field_path: String,
        value: serde_json::Value,
    },
    PlaceAssetIntoScene {
        operation_id: String,
        depends_on: Vec<String>,
        asset_id: String,
        asset_type: String,
        asset_guid: Option<String>,
        target_parent_id: Option<String>,
        local_position: Option<Vec3>,
        placement_mode: AssetPlacementMode,
    },
}

impl ScenePatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::CreateEntity { operation_id, .. }
            | Self::DeleteEntity { operation_id, .. }
            | Self::RenameEntity { operation_id, .. }
            | Self::SetTransform { operation_id, .. }
            | Self::AddComponent { operation_id, .. }
            | Self::RemoveComponent { operation_id, .. }
            | Self::SetComponentField { operation_id, .. }
            | Self::PlaceAssetIntoScene { operation_id, .. } => operation_id,
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::CreateEntity { depends_on, .. }
            | Self::DeleteEntity { depends_on, .. }
            | Self::RenameEntity { depends_on, .. }
            | Self::SetTransform { depends_on, .. }
            | Self::AddComponent { depends_on, .. }
            | Self::RemoveComponent { depends_on, .. }
            | Self::SetComponentField { depends_on, .. }
            | Self::PlaceAssetIntoScene { depends_on, .. } => depends_on,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::CreateEntity { .. } => "Scene.CreateEntity",
            Self::DeleteEntity { .. } => "Scene.DeleteEntity",
            Self::RenameEntity { .. } => "Scene.RenameEntity",
            Self::SetTransform { .. } => "Scene.SetTransform",
            Self::AddComponent { .. } => "Scene.AddComponent",
            Self::RemoveComponent { .. } => "Scene.RemoveComponent",
            Self::SetComponentField { .. } => "Scene.SetComponentField",
            Self::PlaceAssetIntoScene { .. } => "Scene.PlaceAssetIntoScene",
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::CreateEntity { name, .. } => format!("entity.name={name}"),
            Self::DeleteEntity { entity_id, .. }
            | Self::RenameEntity { entity_id, .. }
            | Self::SetTransform { entity_id, .. }
            | Self::AddComponent { entity_id, .. }
            | Self::RemoveComponent { entity_id, .. }
            | Self::SetComponentField { entity_id, .. } => format!("entity.id={entity_id}"),
            Self::PlaceAssetIntoScene { asset_id, .. } => format!("asset.id={asset_id}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InputPatchOperation {
    CreateDefaultInputMapping {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    DeleteInputMapping {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    AddInputAction {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        action_id: String,
        value_type: InputActionValueKind,
    },
    AddInputBinding {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        context_id: String,
        action_id: String,
        device_path: String,
    },
    RemoveInputAction {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        action_id: String,
    },
    RemoveInputBinding {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        binding_index: usize,
    },
    SetInputBindingDevicePath {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        binding_index: usize,
        device_path: String,
    },
    SetInputBindingProcessor {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        binding_index: usize,
        processor: InputBindingProcessorPatch,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InputBindingProcessorPatch {
    None,
    Deadzone { threshold: f32 },
    Normalize,
    Scale { factor: f32 },
    Invert,
}

impl InputPatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::CreateDefaultInputMapping { operation_id, .. }
            | Self::DeleteInputMapping { operation_id, .. }
            | Self::AddInputAction { operation_id, .. }
            | Self::AddInputBinding { operation_id, .. }
            | Self::RemoveInputAction { operation_id, .. }
            | Self::RemoveInputBinding { operation_id, .. }
            | Self::SetInputBindingDevicePath { operation_id, .. }
            | Self::SetInputBindingProcessor { operation_id, .. } => operation_id,
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::CreateDefaultInputMapping { depends_on, .. }
            | Self::DeleteInputMapping { depends_on, .. }
            | Self::AddInputAction { depends_on, .. }
            | Self::AddInputBinding { depends_on, .. }
            | Self::RemoveInputAction { depends_on, .. }
            | Self::RemoveInputBinding { depends_on, .. }
            | Self::SetInputBindingDevicePath { depends_on, .. }
            | Self::SetInputBindingProcessor { depends_on, .. } => depends_on,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::CreateDefaultInputMapping { .. } => "Input.CreateDefaultInputMapping",
            Self::DeleteInputMapping { .. } => "Input.DeleteInputMapping",
            Self::AddInputAction { .. } => "Input.AddInputAction",
            Self::AddInputBinding { .. } => "Input.AddInputBinding",
            Self::RemoveInputAction { .. } => "Input.RemoveInputAction",
            Self::RemoveInputBinding { .. } => "Input.RemoveInputBinding",
            Self::SetInputBindingDevicePath { .. } => "Input.SetInputBindingDevicePath",
            Self::SetInputBindingProcessor { .. } => "Input.SetInputBindingProcessor",
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::CreateDefaultInputMapping { path, .. }
            | Self::DeleteInputMapping { path, .. }
            | Self::AddInputAction { path, .. }
            | Self::AddInputBinding { path, .. }
            | Self::RemoveInputAction { path, .. }
            | Self::RemoveInputBinding { path, .. }
            | Self::SetInputBindingDevicePath { path, .. }
            | Self::SetInputBindingProcessor { path, .. } => path,
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::CreateDefaultInputMapping { path, .. } => format!("input.path={path}"),
            Self::DeleteInputMapping { path, .. } => format!("input.path={path}"),
            Self::AddInputAction {
                path, action_id, ..
            } => format!("input.path={path} action={action_id}"),
            Self::AddInputBinding {
                path, action_id, ..
            } => format!("input.path={path} binding.action={action_id}"),
            Self::RemoveInputAction {
                path, action_id, ..
            } => format!("input.path={path} remove.action={action_id}"),
            Self::RemoveInputBinding {
                path,
                binding_index,
                ..
            } => format!("input.path={path} remove.binding.index={binding_index}"),
            Self::SetInputBindingDevicePath {
                path,
                binding_index,
                ..
            } => format!("input.path={path} binding.index={binding_index}"),
            Self::SetInputBindingProcessor {
                path,
                binding_index,
                ..
            } => format!("input.path={path} binding.index={binding_index}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AssetPatchOperation {
    RegisterExistingAsset {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        expected_kind: Option<AssetKind>,
    },
    GenerateMockImageAsset {
        operation_id: String,
        depends_on: Vec<String>,
        prompt: String,
        target_folder: String,
        asset_name: String,
        image_kind: String,
        width: u32,
        height: u32,
        transparent_background: bool,
    },
    ValidateAssetBrowserIndex {
        operation_id: String,
        depends_on: Vec<String>,
        query_kind: Option<AssetKind>,
    },
}

impl AssetPatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::RegisterExistingAsset { operation_id, .. }
            | Self::GenerateMockImageAsset { operation_id, .. }
            | Self::ValidateAssetBrowserIndex { operation_id, .. } => operation_id,
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::RegisterExistingAsset { depends_on, .. }
            | Self::GenerateMockImageAsset { depends_on, .. }
            | Self::ValidateAssetBrowserIndex { depends_on, .. } => depends_on,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::RegisterExistingAsset { .. } => "Asset.RegisterExistingAsset",
            Self::GenerateMockImageAsset { .. } => "Asset.GenerateMockImageAsset",
            Self::ValidateAssetBrowserIndex { .. } => "Asset.ValidateAssetBrowserIndex",
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Self::RegisterExistingAsset { path, .. } => Some(path),
            Self::GenerateMockImageAsset { target_folder, .. } => Some(target_folder),
            Self::ValidateAssetBrowserIndex { .. } => None,
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::RegisterExistingAsset { path, .. } => format!("asset.path={path}"),
            Self::GenerateMockImageAsset {
                target_folder,
                asset_name,
                ..
            } => format!("asset.target={target_folder}/{asset_name}"),
            Self::ValidateAssetBrowserIndex { query_kind, .. } => {
                format!("asset.index.query_kind={query_kind:?}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PrefabPatchOperation {
    CreateFromSceneSelection {
        operation_id: String,
        depends_on: Vec<String>,
        scene_path: Option<String>,
        root_entity_id: String,
        prefab_id: String,
        name: String,
        replace_selection_with_instance: bool,
    },
    OpenDocument {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    SetStageEntityField {
        operation_id: String,
        depends_on: Vec<String>,
        source_entity_id: String,
        component_type: Option<String>,
        field_path: String,
        value: serde_json::Value,
    },
    SaveDocument {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    InstantiateInScene {
        operation_id: String,
        depends_on: Vec<String>,
        prefab_id: String,
        parent_entity_id: Option<String>,
        local_position: Option<Vec3>,
    },
    ApplyOverrideToAsset {
        operation_id: String,
        depends_on: Vec<String>,
        instance_entity_id: String,
        target_source_entity_id: String,
        component_type: String,
        field_path: String,
    },
    RevertOverride {
        operation_id: String,
        depends_on: Vec<String>,
        instance_entity_id: String,
        target_source_entity_id: String,
        component_type: String,
        field_path: String,
    },
    ValidateReferences {
        operation_id: String,
        depends_on: Vec<String>,
        path: Option<String>,
    },
}

impl PrefabPatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::CreateFromSceneSelection { operation_id, .. }
            | Self::OpenDocument { operation_id, .. }
            | Self::SetStageEntityField { operation_id, .. }
            | Self::SaveDocument { operation_id, .. }
            | Self::InstantiateInScene { operation_id, .. }
            | Self::ApplyOverrideToAsset { operation_id, .. }
            | Self::RevertOverride { operation_id, .. }
            | Self::ValidateReferences { operation_id, .. } => operation_id,
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::CreateFromSceneSelection { depends_on, .. }
            | Self::OpenDocument { depends_on, .. }
            | Self::SetStageEntityField { depends_on, .. }
            | Self::SaveDocument { depends_on, .. }
            | Self::InstantiateInScene { depends_on, .. }
            | Self::ApplyOverrideToAsset { depends_on, .. }
            | Self::RevertOverride { depends_on, .. }
            | Self::ValidateReferences { depends_on, .. } => depends_on,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::CreateFromSceneSelection { .. } => "Prefab.CreateFromSceneSelection",
            Self::OpenDocument { .. } => "Prefab.OpenDocument",
            Self::SetStageEntityField { .. } => "Prefab.SetStageEntityField",
            Self::SaveDocument { .. } => "Prefab.SaveDocument",
            Self::InstantiateInScene { .. } => "Prefab.InstantiateInScene",
            Self::ApplyOverrideToAsset { .. } => "Prefab.ApplyOverrideToAsset",
            Self::RevertOverride { .. } => "Prefab.RevertOverride",
            Self::ValidateReferences { .. } => "Prefab.ValidateReferences",
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Self::CreateFromSceneSelection { scene_path, .. } => scene_path.as_deref(),
            Self::OpenDocument { path, .. } | Self::SaveDocument { path, .. } => Some(path),
            Self::ValidateReferences { path, .. } => path.as_deref(),
            Self::SetStageEntityField { .. }
            | Self::InstantiateInScene { .. }
            | Self::ApplyOverrideToAsset { .. }
            | Self::RevertOverride { .. } => None,
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::CreateFromSceneSelection {
                root_entity_id,
                prefab_id,
                ..
            } => format!("prefab.id={prefab_id} source.entity={root_entity_id}"),
            Self::OpenDocument { path, .. } | Self::SaveDocument { path, .. } => {
                format!("prefab.path={path}")
            }
            Self::SetStageEntityField {
                source_entity_id,
                field_path,
                ..
            } => format!("prefab.stage.entity={source_entity_id} field={field_path}"),
            Self::InstantiateInScene { prefab_id, .. } => format!("prefab.id={prefab_id}"),
            Self::ApplyOverrideToAsset {
                instance_entity_id,
                field_path,
                ..
            }
            | Self::RevertOverride {
                instance_entity_id,
                field_path,
                ..
            } => format!("prefab.instance={instance_entity_id} field={field_path}"),
            Self::ValidateReferences { path, .. } => {
                format!(
                    "prefab.references.path={}",
                    path.as_deref().unwrap_or("active")
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AuiPatchOperation {
    CreateDocument {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        document_id: String,
        width: f32,
        height: f32,
    },
    OpenDocument {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    AddNode {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        parent_node_id: String,
        node_id: String,
        node_kind: String,
        name: String,
        rect: serde_json::Value,
    },
    SetNodeField {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        node_id: String,
        schema_path: String,
        value: serde_json::Value,
    },
    SetBindingPath {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        node_id: String,
        target_field: String,
        binding_id: String,
        binding_path: String,
        fallback: Option<serde_json::Value>,
    },
    SetActionRef {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        node_id: String,
        event: String,
        action_id: String,
        payload: Option<serde_json::Value>,
    },
    ValidateDocument {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    SaveDocument {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    PreviewOverlay {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
}

impl AuiPatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::CreateDocument { operation_id, .. }
            | Self::OpenDocument { operation_id, .. }
            | Self::AddNode { operation_id, .. }
            | Self::SetNodeField { operation_id, .. }
            | Self::SetBindingPath { operation_id, .. }
            | Self::SetActionRef { operation_id, .. }
            | Self::ValidateDocument { operation_id, .. }
            | Self::SaveDocument { operation_id, .. }
            | Self::PreviewOverlay { operation_id, .. } => operation_id,
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::CreateDocument { depends_on, .. }
            | Self::OpenDocument { depends_on, .. }
            | Self::AddNode { depends_on, .. }
            | Self::SetNodeField { depends_on, .. }
            | Self::SetBindingPath { depends_on, .. }
            | Self::SetActionRef { depends_on, .. }
            | Self::ValidateDocument { depends_on, .. }
            | Self::SaveDocument { depends_on, .. }
            | Self::PreviewOverlay { depends_on, .. } => depends_on,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::CreateDocument { .. } => "Aui.CreateDocument",
            Self::OpenDocument { .. } => "Aui.OpenDocument",
            Self::AddNode { .. } => "Aui.AddNode",
            Self::SetNodeField { .. } => "Aui.SetNodeField",
            Self::SetBindingPath { .. } => "Aui.SetBindingPath",
            Self::SetActionRef { .. } => "Aui.SetActionRef",
            Self::ValidateDocument { .. } => "Aui.ValidateDocument",
            Self::SaveDocument { .. } => "Aui.SaveDocument",
            Self::PreviewOverlay { .. } => "Aui.PreviewOverlay",
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::CreateDocument { path, .. }
            | Self::OpenDocument { path, .. }
            | Self::AddNode { path, .. }
            | Self::SetNodeField { path, .. }
            | Self::SetBindingPath { path, .. }
            | Self::SetActionRef { path, .. }
            | Self::ValidateDocument { path, .. }
            | Self::SaveDocument { path, .. }
            | Self::PreviewOverlay { path, .. } => path,
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::CreateDocument {
                path, document_id, ..
            } => format!("aui.path={path} document={document_id}"),
            Self::OpenDocument { path, .. }
            | Self::ValidateDocument { path, .. }
            | Self::SaveDocument { path, .. }
            | Self::PreviewOverlay { path, .. } => format!("aui.path={path}"),
            Self::AddNode { path, node_id, .. }
            | Self::SetNodeField { path, node_id, .. }
            | Self::SetBindingPath { path, node_id, .. }
            | Self::SetActionRef { path, node_id, .. } => {
                format!("aui.path={path} node={node_id}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RulePatchOperation {
    CreateAsset {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        rule_id: String,
        display_name: String,
        #[serde(default)]
        phase: Option<String>,
    },
    OpenAsset {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    SetTrigger {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        trigger: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    AddStatement {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        statement: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    UpdateStatement {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        statement_index: usize,
        statement: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    RemoveStatement {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        statement_index: usize,
        expected_ir_hash: Option<String>,
    },
    AddOperation {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        operation: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    UpdateOperation {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        operation_index: usize,
        operation: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    RemoveOperation {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
        operation_index: usize,
        expected_ir_hash: Option<String>,
    },
    ValidateAsset {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    BuildArtifact {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
    BuildProjectManifest {
        operation_id: String,
        depends_on: Vec<String>,
        path: String,
    },
}

impl RulePatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::CreateAsset { operation_id, .. }
            | Self::OpenAsset { operation_id, .. }
            | Self::SetTrigger { operation_id, .. }
            | Self::AddStatement { operation_id, .. }
            | Self::UpdateStatement { operation_id, .. }
            | Self::RemoveStatement { operation_id, .. }
            | Self::AddOperation { operation_id, .. }
            | Self::UpdateOperation { operation_id, .. }
            | Self::RemoveOperation { operation_id, .. }
            | Self::ValidateAsset { operation_id, .. }
            | Self::BuildArtifact { operation_id, .. }
            | Self::BuildProjectManifest { operation_id, .. } => operation_id,
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::CreateAsset { depends_on, .. }
            | Self::OpenAsset { depends_on, .. }
            | Self::SetTrigger { depends_on, .. }
            | Self::AddStatement { depends_on, .. }
            | Self::UpdateStatement { depends_on, .. }
            | Self::RemoveStatement { depends_on, .. }
            | Self::AddOperation { depends_on, .. }
            | Self::UpdateOperation { depends_on, .. }
            | Self::RemoveOperation { depends_on, .. }
            | Self::ValidateAsset { depends_on, .. }
            | Self::BuildArtifact { depends_on, .. }
            | Self::BuildProjectManifest { depends_on, .. } => depends_on,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::CreateAsset { .. } => "Rule.CreateAsset",
            Self::OpenAsset { .. } => "Rule.OpenAsset",
            Self::SetTrigger { .. } => "Rule.SetTrigger",
            Self::AddStatement { .. } => "Rule.AddStatement",
            Self::UpdateStatement { .. } => "Rule.UpdateStatement",
            Self::RemoveStatement { .. } => "Rule.RemoveStatement",
            Self::AddOperation { .. } => "Rule.AddOperation",
            Self::UpdateOperation { .. } => "Rule.UpdateOperation",
            Self::RemoveOperation { .. } => "Rule.RemoveOperation",
            Self::ValidateAsset { .. } => "Rule.ValidateAsset",
            Self::BuildArtifact { .. } => "Rule.BuildArtifact",
            Self::BuildProjectManifest { .. } => "Rule.BuildProjectManifest",
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::CreateAsset { path, .. }
            | Self::OpenAsset { path, .. }
            | Self::SetTrigger { path, .. }
            | Self::AddStatement { path, .. }
            | Self::UpdateStatement { path, .. }
            | Self::RemoveStatement { path, .. }
            | Self::AddOperation { path, .. }
            | Self::UpdateOperation { path, .. }
            | Self::RemoveOperation { path, .. }
            | Self::ValidateAsset { path, .. }
            | Self::BuildArtifact { path, .. }
            | Self::BuildProjectManifest { path, .. } => path,
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::CreateAsset { path, rule_id, .. } => {
                format!("rule.path={path} rule={rule_id}")
            }
            Self::OpenAsset { path, .. }
            | Self::SetTrigger { path, .. }
            | Self::AddStatement { path, .. }
            | Self::ValidateAsset { path, .. }
            | Self::BuildArtifact { path, .. }
            | Self::BuildProjectManifest { path, .. } => format!("rule.path={path}"),
            Self::UpdateStatement {
                path,
                statement_index,
                ..
            }
            | Self::RemoveStatement {
                path,
                statement_index,
                ..
            } => format!("rule.path={path} statement.index={statement_index}"),
            Self::AddOperation { path, .. } => format!("rule.path={path} operation=new"),
            Self::UpdateOperation {
                path,
                operation_index,
                ..
            }
            | Self::RemoveOperation {
                path,
                operation_index,
                ..
            } => format!("rule.path={path} operation.index={operation_index}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BuildPatchOperation {
    ExportDesktopPackage {
        operation_id: String,
        depends_on: Vec<String>,
        profile_id: Option<String>,
    },
    OpenBuildReport {
        operation_id: String,
        depends_on: Vec<String>,
    },
    OpenBuildOutput {
        operation_id: String,
        depends_on: Vec<String>,
    },
}

impl BuildPatchOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::ExportDesktopPackage { operation_id, .. }
            | Self::OpenBuildReport { operation_id, .. }
            | Self::OpenBuildOutput { operation_id, .. } => operation_id,
        }
    }

    pub fn depends_on(&self) -> &[String] {
        match self {
            Self::ExportDesktopPackage { depends_on, .. }
            | Self::OpenBuildReport { depends_on, .. }
            | Self::OpenBuildOutput { depends_on, .. } => depends_on,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::ExportDesktopPackage { .. } => "Build.ExportDesktopPackage",
            Self::OpenBuildReport { .. } => "Build.OpenBuildReport",
            Self::OpenBuildOutput { .. } => "Build.OpenBuildOutput",
        }
    }

    pub fn target_summary(&self) -> String {
        match self {
            Self::ExportDesktopPackage { profile_id, .. } => {
                format!(
                    "build.profile={}",
                    profile_id.as_deref().unwrap_or("default")
                )
            }
            Self::OpenBuildReport { .. } => "build.report".to_string(),
            Self::OpenBuildOutput { .. } => "build.output".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchDiagnostic {
    pub severity: PatchDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub operation_id: Option<String>,
    pub target: Option<String>,
}

impl PatchDiagnostic {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        operation_id: Option<String>,
        target: Option<String>,
    ) -> Self {
        Self {
            severity: PatchDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            operation_id,
            target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchValidationReport {
    pub patch_id: String,
    pub accepted: bool,
    pub operation_count: usize,
    pub diagnostics: Vec<PatchDiagnostic>,
}

impl PatchValidationReport {
    pub fn accepted(patch: &ProjectPatchDocument) -> Self {
        Self {
            patch_id: patch.patch_id.clone(),
            accepted: true,
            operation_count: patch.operations.len(),
            diagnostics: Vec::new(),
        }
    }

    pub fn rejected(patch: &ProjectPatchDocument, diagnostics: Vec<PatchDiagnostic>) -> Self {
        Self {
            patch_id: patch.patch_id.clone(),
            accepted: false,
            operation_count: patch.operations.len(),
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchReviewModel {
    pub patch_id: String,
    pub title: String,
    pub summary: String,
    pub operation_count: usize,
    pub touched_domains: Vec<PatchCapability>,
    pub read_set_preview: Vec<String>,
    pub write_set_preview: Vec<String>,
    pub risk_level: PatchRiskLevel,
    pub validation_status: bool,
    pub diagnostics: Vec<PatchDiagnostic>,
    pub requires_confirmation: bool,
}

impl PatchReviewModel {
    pub fn from_patch(patch: &ProjectPatchDocument, validation: PatchValidationReport) -> Self {
        Self {
            patch_id: patch.patch_id.clone(),
            title: patch.title.clone(),
            summary: patch.intent_summary.clone(),
            operation_count: patch.operations.len(),
            touched_domains: capabilities_for_operations(&patch.operations),
            read_set_preview: patch
                .operations
                .iter()
                .map(|operation| operation.target_summary())
                .collect(),
            write_set_preview: patch
                .operations
                .iter()
                .map(|operation| format!("{} {}", operation.kind(), operation.target_summary()))
                .collect(),
            risk_level: patch.risk_level,
            validation_status: validation.accepted,
            diagnostics: validation.diagnostics,
            requires_confirmation: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchApplyStatus {
    Committed,
    Rejected,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchOperationApplyStatus {
    Committed,
    Rejected,
    Failed,
    Reverted,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOperationResult {
    pub operation_id: String,
    pub kind: String,
    pub status: PatchOperationApplyStatus,
    pub command_id: Option<String>,
    pub diagnostics: Vec<PatchDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchApplyReport {
    pub patch_id: String,
    pub status: PatchApplyStatus,
    pub validation: PatchValidationReport,
    pub operation_results: Vec<PatchOperationResult>,
    pub inverse_patch: Option<ProjectPatchDocument>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectPatchProductizationStatus {
    Pass,
    Partial,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchHistorySummary {
    pub applied_count: usize,
    pub last_patch_id: Option<String>,
    pub last_status: Option<PatchApplyStatus>,
    pub reversible_count: usize,
    pub diagnostics: Vec<PatchDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatchProductizationReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ProjectPatchProductizationStatus,
    pub patch_id: String,
    pub source: PatchSource,
    pub validation: PatchValidationReport,
    pub review: PatchReviewModel,
    pub apply_report: Option<PatchApplyReport>,
    pub history_summary: PatchHistorySummary,
    pub supported_capabilities: Vec<PatchCapability>,
    pub unsupported_capabilities: Vec<PatchCapability>,
    pub next_actions: Vec<String>,
    pub artifacts: Vec<String>,
}

impl ProjectPatchProductizationReport {
    pub fn from_parts(
        scenario_id: impl Into<String>,
        patch: &ProjectPatchDocument,
        validation: PatchValidationReport,
        review: PatchReviewModel,
        apply_report: Option<PatchApplyReport>,
        history_summary: PatchHistorySummary,
        artifacts: Vec<String>,
    ) -> Self {
        let supported_capabilities = patch
            .required_capabilities
            .iter()
            .copied()
            .filter(|capability| is_supported_capability(*capability))
            .collect::<Vec<_>>();
        let unsupported_capabilities = patch
            .required_capabilities
            .iter()
            .copied()
            .filter(|capability| !is_supported_capability(*capability))
            .collect::<Vec<_>>();
        let mut next_actions = unsupported_capabilities
            .iter()
            .map(|capability| unsupported_capability_next_action(*capability))
            .collect::<Vec<_>>();
        if !validation.accepted {
            next_actions.push("fix_project_patch_validation_diagnostics".to_string());
        }
        let status = if !validation.accepted {
            ProjectPatchProductizationStatus::Fail
        } else if !unsupported_capabilities.is_empty() {
            ProjectPatchProductizationStatus::Partial
        } else if apply_report.as_ref().is_some_and(|report| {
            matches!(
                report.status,
                PatchApplyStatus::Committed | PatchApplyStatus::Reverted
            )
        }) {
            ProjectPatchProductizationStatus::Pass
        } else {
            ProjectPatchProductizationStatus::Partial
        };
        Self {
            schema_version: PROJECT_PATCH_PRODUCTIZATION_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: scenario_id.into(),
            status,
            patch_id: patch.patch_id.clone(),
            source: patch.source.clone(),
            validation,
            review,
            apply_report,
            history_summary,
            supported_capabilities,
            unsupported_capabilities,
            next_actions,
            artifacts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatchImportProductizationReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ProjectPatchImportProductizationStatus,
    pub source_kind: ProjectPatchImportSourceKind,
    pub source_label: String,
    pub parse_status: ProjectPatchImportParseStatus,
    pub patch_id: Option<String>,
    pub validation: Option<PatchValidationReport>,
    pub review: Option<PatchReviewModel>,
    pub apply_report: Option<PatchApplyReport>,
    pub history_summary: PatchHistorySummary,
    pub supported_capabilities: Vec<PatchCapability>,
    pub unsupported_capabilities: Vec<PatchCapability>,
    pub diagnostics: Vec<PatchDiagnostic>,
    pub next_actions: Vec<String>,
    pub artifacts: Vec<String>,
}

impl ProjectPatchImportProductizationReport {
    pub fn from_parts(
        scenario_id: impl Into<String>,
        import_result: ProjectPatchImportResult,
        apply_report: Option<PatchApplyReport>,
        history_summary: PatchHistorySummary,
        artifacts: Vec<String>,
    ) -> Self {
        let patch = import_result.parsed_patch.as_ref();
        let supported_capabilities = patch
            .map(|patch| {
                patch
                    .required_capabilities
                    .iter()
                    .copied()
                    .filter(|capability| is_supported_capability(*capability))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let unsupported_capabilities = patch
            .map(|patch| {
                patch
                    .required_capabilities
                    .iter()
                    .copied()
                    .filter(|capability| !is_supported_capability(*capability))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut diagnostics = import_result.schema_diagnostics.clone();
        diagnostics.extend(import_result.capability_diagnostics.clone());
        if let Some(validation) = &import_result.validation {
            diagnostics.extend(validation.diagnostics.clone());
        }
        if let Some(apply_report) = &apply_report {
            diagnostics.extend(apply_report.validation.diagnostics.clone());
            diagnostics.extend(
                apply_report
                    .operation_results
                    .iter()
                    .flat_map(|operation| operation.diagnostics.clone()),
            );
        }

        let mut next_actions = import_result.next_actions.clone();
        next_actions.extend(
            unsupported_capabilities
                .iter()
                .map(|capability| unsupported_capability_next_action(*capability)),
        );
        if import_result.parse_status == ProjectPatchImportParseStatus::Rejected {
            next_actions.push("fix_project_patch_import_parse_or_schema".to_string());
        }
        if import_result
            .validation
            .as_ref()
            .is_some_and(|validation| !validation.accepted)
        {
            next_actions.push("fix_project_patch_validation_diagnostics".to_string());
        }
        next_actions.sort();
        next_actions.dedup();

        let status = if import_result.parse_status == ProjectPatchImportParseStatus::Rejected
            || import_result
                .validation
                .as_ref()
                .is_some_and(|validation| !validation.accepted)
        {
            ProjectPatchImportProductizationStatus::Fail
        } else if !unsupported_capabilities.is_empty() {
            ProjectPatchImportProductizationStatus::Partial
        } else if apply_report
            .as_ref()
            .is_some_and(|report| report.status == PatchApplyStatus::Committed)
        {
            ProjectPatchImportProductizationStatus::Pass
        } else {
            ProjectPatchImportProductizationStatus::Partial
        };

        Self {
            schema_version: PROJECT_PATCH_IMPORT_PRODUCTIZATION_REPORT_SCHEMA_VERSION.to_string(),
            scenario_id: scenario_id.into(),
            status,
            source_kind: import_result.source_kind,
            source_label: import_result.source_label,
            parse_status: import_result.parse_status,
            patch_id: patch.map(|patch| patch.patch_id.clone()),
            validation: import_result.validation,
            review: import_result.review,
            apply_report,
            history_summary,
            supported_capabilities,
            unsupported_capabilities,
            diagnostics,
            next_actions,
            artifacts,
        }
    }
}

pub fn summarize_patch_history(
    entries: &[super::history::PatchHistoryEntry],
) -> PatchHistorySummary {
    let applied_count = entries.len();
    let last = entries.last();
    let reversible_count = entries
        .iter()
        .filter(|entry| !entry.inverse_patch.operations.is_empty())
        .count();
    PatchHistorySummary {
        applied_count,
        last_patch_id: last.map(|entry| entry.patch_id.clone()),
        last_status: last.map(|entry| entry.apply_report.status),
        reversible_count,
        diagnostics: Vec::new(),
    }
}

fn capabilities_for_operations(operations: &[PatchOperation]) -> Vec<PatchCapability> {
    let mut capabilities = Vec::new();
    for operation in operations {
        let capability = match operation {
            PatchOperation::Scene(_) => PatchCapability::Scene,
            PatchOperation::Input(_) => PatchCapability::Input,
            PatchOperation::Asset(_) => PatchCapability::Asset,
            PatchOperation::Prefab(_) => PatchCapability::Prefab,
            PatchOperation::Aui(_) => PatchCapability::Aui,
            PatchOperation::Rule(_) => PatchCapability::Rule,
            PatchOperation::Build(_) => PatchCapability::Build,
        };
        if !capabilities.contains(&capability) {
            capabilities.push(capability);
        }
    }
    capabilities
}

fn is_supported_capability(capability: PatchCapability) -> bool {
    matches!(
        capability,
        PatchCapability::Scene
            | PatchCapability::Input
            | PatchCapability::Asset
            | PatchCapability::Prefab
            | PatchCapability::Aui
            | PatchCapability::Rule
            | PatchCapability::Build
    )
}

fn unsupported_capability_next_action(capability: PatchCapability) -> String {
    match capability {
        PatchCapability::Asset => "asset_patch_capability_v2".to_string(),
        PatchCapability::Prefab => "prefab_patch_capability_v2".to_string(),
        PatchCapability::Aui => "aui_authoring_productization_or_patch_capability_v2".to_string(),
        PatchCapability::Rule => "rule_patch_capability_v2".to_string(),
        PatchCapability::Build => "build_patch_capability_v2".to_string(),
        PatchCapability::Scene | PatchCapability::Input => "none".to_string(),
    }
}
