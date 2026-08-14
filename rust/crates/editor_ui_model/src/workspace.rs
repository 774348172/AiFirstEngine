use serde::{Deserialize, Serialize};

use super::{UiCommand, UiCommandPayload, UiCommandSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAuthoringWorkspaceModel {
    pub project_root: Option<String>,
    pub project_id: Option<String>,
    pub active_scene_id: Option<String>,
    pub active_document: Option<WorkspaceDocumentSummary>,
    pub selection: WorkspaceSelectionSummary,
    pub domains: Vec<WorkspaceDomainSummary>,
    pub dirty_domains: Vec<WorkspaceDomainKind>,
    pub diagnostics: WorkspaceDiagnosticsSummary,
    pub empty_message: String,
    pub report: WorkspaceReportSummary,
}

impl ProjectAuthoringWorkspaceModel {
    pub fn empty() -> Self {
        Self {
            project_root: None,
            project_id: None,
            active_scene_id: None,
            active_document: None,
            selection: WorkspaceSelectionSummary::default(),
            domains: WorkspaceDomainKind::all()
                .into_iter()
                .map(WorkspaceDomainSummary::not_configured)
                .collect(),
            dirty_domains: Vec::new(),
            diagnostics: WorkspaceDiagnosticsSummary::default(),
            empty_message: "Open a project to edit workspace domains.".to_string(),
            report: WorkspaceReportSummary::empty(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceDomainKind {
    Project,
    Scene,
    Asset,
    Prefab,
    Rule,
    Aui,
    Input,
    Play,
    Build,
    Report,
}

impl WorkspaceDomainKind {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Project,
            Self::Scene,
            Self::Asset,
            Self::Prefab,
            Self::Rule,
            Self::Aui,
            Self::Input,
            Self::Play,
            Self::Build,
            Self::Report,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Scene => "scene",
            Self::Asset => "asset",
            Self::Prefab => "prefab",
            Self::Rule => "rule",
            Self::Aui => "aui",
            Self::Input => "input",
            Self::Play => "play",
            Self::Build => "build",
            Self::Report => "report",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceDomainStatus {
    NotConfigured,
    Empty,
    Ready,
    Dirty,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDomainSummary {
    pub kind: WorkspaceDomainKind,
    pub label: String,
    pub status: WorkspaceDomainStatus,
    pub item_count: usize,
    pub dirty: bool,
    pub selected_id: Option<String>,
    pub active_document_path: Option<String>,
    pub diagnostics: WorkspaceDiagnosticsSummary,
    pub summary: String,
}

impl WorkspaceDomainSummary {
    pub fn new(
        kind: WorkspaceDomainKind,
        label: impl Into<String>,
        status: WorkspaceDomainStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            status,
            item_count: 0,
            dirty: false,
            selected_id: None,
            active_document_path: None,
            diagnostics: WorkspaceDiagnosticsSummary::default(),
            summary: summary.into(),
        }
    }

    pub fn not_configured(kind: WorkspaceDomainKind) -> Self {
        Self::new(
            kind,
            workspace_domain_label(kind),
            WorkspaceDomainStatus::NotConfigured,
            "not_configured",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDocumentSummary {
    pub document_kind: String,
    pub document_id: Option<String>,
    pub path: Option<String>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSelectionSummary {
    pub primary: Option<WorkspaceSelectionTarget>,
    pub secondary: Vec<WorkspaceSelectionTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceSelectionTarget {
    Entity {
        entity_id: String,
    },
    Asset {
        asset_ref: String,
    },
    Prefab {
        prefab_id: String,
    },
    Rule {
        rule_id: String,
    },
    AuiDocument {
        aui_id: String,
    },
    AuiNode {
        document_path: String,
        document_id: String,
        node_id: String,
    },
    InputAction {
        action_id: String,
    },
    BuildProfile {
        profile_id: String,
    },
    ReportEntry {
        entry_id: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDiagnosticsSummary {
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub last_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCommandSummary {
    pub command_id: String,
    pub source: UiCommandSource,
    pub target_domain: WorkspaceDomainKind,
    pub payload_kind: String,
    pub request_id: String,
}

impl WorkspaceCommandSummary {
    pub fn from_ui_command(command: &UiCommand) -> Self {
        Self {
            command_id: command.command_id.clone(),
            source: command.source.clone(),
            target_domain: workspace_domain_for_payload(&command.payload),
            payload_kind: workspace_payload_kind(&command.payload).to_string(),
            request_id: command.request_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTransactionSummary {
    pub transaction_id: String,
    pub command_id: String,
    pub target_domain: WorkspaceDomainKind,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub status: String,
    pub diagnostics: WorkspaceDiagnosticsSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceReportSummary {
    pub project_status: String,
    pub dirty_domains: Vec<WorkspaceDomainKind>,
    pub diagnostics: WorkspaceDiagnosticsSummary,
    pub report_count: usize,
    pub evidence_count: usize,
    pub next_action_count: usize,
    pub last_command: Option<WorkspaceCommandSummary>,
    pub last_transaction: Option<WorkspaceTransactionSummary>,
    pub build_status: Option<String>,
    pub play_status: Option<String>,
}

impl WorkspaceReportSummary {
    pub fn empty() -> Self {
        Self {
            project_status: "no_project".to_string(),
            dirty_domains: Vec::new(),
            diagnostics: WorkspaceDiagnosticsSummary::default(),
            report_count: 0,
            evidence_count: 0,
            next_action_count: 0,
            last_command: None,
            last_transaction: None,
            build_status: None,
            play_status: None,
        }
    }
}

pub fn workspace_domain_for_payload(payload: &UiCommandPayload) -> WorkspaceDomainKind {
    match payload {
        UiCommandPayload::OpenProject { .. }
        | UiCommandPayload::CreateProject { .. }
        | UiCommandPayload::StartCreateProjectWithAi { .. }
        | UiCommandPayload::SelectRecentProject { .. }
        | UiCommandPayload::RefreshRecentProjects
        | UiCommandPayload::ApproveProjectRuntimeTrust { .. }
        | UiCommandPayload::DenyProjectRuntimeTrust { .. }
        | UiCommandPayload::CancelProjectRuntimeTrust { .. } => WorkspaceDomainKind::Project,
        UiCommandPayload::SelectProjectBrowserEntry { .. }
        | UiCommandPayload::OpenProjectBrowserEntry { .. }
        | UiCommandPayload::SelectAssetBrowserEntry { .. }
        | UiCommandPayload::OpenAssetBrowserEntry { .. }
        | UiCommandPayload::SetAssetBrowserFolder { .. }
        | UiCommandPayload::SetAssetBrowserSearch { .. }
        | UiCommandPayload::SetAssetBrowserKindFilter { .. }
        | UiCommandPayload::AssetBrowserToolbar { .. }
        | UiCommandPayload::ScrollAssetBrowser { .. }
        | UiCommandPayload::BeginAssetPick { .. }
        | UiCommandPayload::ConfirmAssetPick
        | UiCommandPayload::CancelAssetPick
        | UiCommandPayload::DropAssetOnInspectorField { .. }
        | UiCommandPayload::RegisterExistingAsset { .. }
        | UiCommandPayload::GenerateMockImageAsset { .. }
        | UiCommandPayload::ValidateAssetBrowserIndex { .. }
        | UiCommandPayload::PlaceAssetIntoScene { .. } => WorkspaceDomainKind::Asset,
        UiCommandPayload::CreateDefaultInputMapping { .. }
        | UiCommandPayload::DeleteInputMapping { .. }
        | UiCommandPayload::OpenInputMapping { .. }
        | UiCommandPayload::SaveInputMapping { .. }
        | UiCommandPayload::DiscardInputMappingDraft { .. }
        | UiCommandPayload::ValidateInputMapping { .. }
        | UiCommandPayload::SelectInputContext { .. }
        | UiCommandPayload::SelectInputAction { .. }
        | UiCommandPayload::SelectInputBinding { .. }
        | UiCommandPayload::AddInputContext { .. }
        | UiCommandPayload::RemoveInputContext { .. }
        | UiCommandPayload::SetInputContextPriority { .. }
        | UiCommandPayload::SetInputContextConsumeInput { .. }
        | UiCommandPayload::AddInputAction { .. }
        | UiCommandPayload::RemoveInputAction { .. }
        | UiCommandPayload::SetInputActionValueType { .. }
        | UiCommandPayload::AddInputBinding { .. }
        | UiCommandPayload::RemoveInputBinding { .. }
        | UiCommandPayload::SetInputBindingDevicePath { .. }
        | UiCommandPayload::SetInputBindingProcessorByIndex { .. }
        | UiCommandPayload::RemoveInputBindingById { .. }
        | UiCommandPayload::SetInputBindingDevicePathById { .. }
        | UiCommandPayload::SetInputBindingTrigger { .. }
        | UiCommandPayload::SetInputBindingProcessor { .. }
        | UiCommandPayload::BeginInputBindingCapture { .. }
        | UiCommandPayload::CancelInputBindingCapture { .. }
        | UiCommandPayload::CommitCapturedInputBinding { .. }
        | UiCommandPayload::PreviewInputMapping { .. }
        | UiCommandPayload::SetInputMappingReportLevel { .. } => WorkspaceDomainKind::Input,
        UiCommandPayload::CreateRuleAsset { .. }
        | UiCommandPayload::OpenRuleAsset { .. }
        | UiCommandPayload::SelectRuleAsset { .. }
        | UiCommandPayload::SetRuleTrigger { .. }
        | UiCommandPayload::AddRuleStatement { .. }
        | UiCommandPayload::UpdateRuleStatement { .. }
        | UiCommandPayload::RemoveRuleStatement { .. }
        | UiCommandPayload::AddRuleOperation { .. }
        | UiCommandPayload::UpdateRuleOperation { .. }
        | UiCommandPayload::RemoveRuleOperation { .. }
        | UiCommandPayload::ValidateRuleAsset { .. }
        | UiCommandPayload::BuildRuleArtifact { .. }
        | UiCommandPayload::BuildProjectRuleManifest { .. }
        | UiCommandPayload::SaveRuleAsset { .. }
        | UiCommandPayload::OpenRuleDiagnostics { .. }
        | UiCommandPayload::SelectRuleCard { .. }
        | UiCommandPayload::SetRuleCardField { .. }
        | UiCommandPayload::AddRuleCard { .. }
        | UiCommandPayload::RemoveRuleCard { .. }
        | UiCommandPayload::SelectRuleGraphNode { .. }
        | UiCommandPayload::RefreshRuleGraphPreview { .. } => WorkspaceDomainKind::Rule,
        UiCommandPayload::CreatePrefabFromSelection { .. }
        | UiCommandPayload::OpenPrefabDocument { .. }
        | UiCommandPayload::EnterPrefabStage { .. }
        | UiCommandPayload::ExitPrefabStage { .. }
        | UiCommandPayload::InstantiatePrefabInScene { .. }
        | UiCommandPayload::SetPrefabStageEntityField { .. }
        | UiCommandPayload::ApplyPrefabOverrideToAsset { .. }
        | UiCommandPayload::SavePrefabDocument { .. }
        | UiCommandPayload::ValidatePrefabReferences { .. }
        | UiCommandPayload::RevertPrefabOverride { .. } => WorkspaceDomainKind::Prefab,
        UiCommandPayload::CreateAuiDocument { .. }
        | UiCommandPayload::OpenAuiDocument { .. }
        | UiCommandPayload::SelectAuiNode { .. }
        | UiCommandPayload::AddAuiNode { .. }
        | UiCommandPayload::SetAuiNodeField { .. }
        | UiCommandPayload::SetAuiBindingPath { .. }
        | UiCommandPayload::SetAuiActionRef { .. }
        | UiCommandPayload::ValidateAuiDocument { .. }
        | UiCommandPayload::SaveAuiDocument { .. }
        | UiCommandPayload::PreviewAuiOverlay { .. }
        | UiCommandPayload::SaveAuiSubtreeAsTemplate { .. }
        | UiCommandPayload::InstantiateAuiTemplate { .. }
        | UiCommandPayload::ValidateAuiTemplate { .. } => WorkspaceDomainKind::Aui,
        UiCommandPayload::OpenSceneDocument { .. }
        | UiCommandPayload::SelectEntity { .. }
        | UiCommandPayload::SelectSceneEntity { .. }
        | UiCommandPayload::CreateSceneEntity { .. }
        | UiCommandPayload::DeleteSceneEntity { .. }
        | UiCommandPayload::RenameSceneEntity { .. }
        | UiCommandPayload::SetSceneTransform { .. }
        | UiCommandPayload::AddSceneComponent { .. }
        | UiCommandPayload::RemoveSceneComponent { .. }
        | UiCommandPayload::SetSceneComponentField { .. }
        | UiCommandPayload::SaveSceneDocument { .. }
        | UiCommandPayload::UndoSceneEdit
        | UiCommandPayload::RedoSceneEdit
        | UiCommandPayload::SetWorkspaceViewMode { .. } => WorkspaceDomainKind::Scene,
        UiCommandPayload::SetAuthoringWorkflowStep { step_id } => step_id.domain(),
        UiCommandPayload::OpenRuntimePackage { .. }
        | UiCommandPayload::ReloadRuntimePackage
        | UiCommandPayload::SelectRuntimeEntity { .. }
        | UiCommandPayload::PickRuntimeEntityAt { .. }
        | UiCommandPayload::SetRuntimeComponentFieldTemporary { .. }
        | UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring
        | UiCommandPayload::ApplyRuntimeChangeToAuthoring { .. }
        | UiCommandPayload::Play
        | UiCommandPayload::Pause
        | UiCommandPayload::StepFrame
        | UiCommandPayload::StopPlaySession
        | UiCommandPayload::SetGameViewTarget { .. }
        | UiCommandPayload::SetGameViewMaximizeOnPlay { .. }
        | UiCommandPayload::ToggleGameViewMaximizeOnPlay
        | UiCommandPayload::TickOneFrame
        | UiCommandPayload::ResetRuntime
        | UiCommandPayload::SelectTraceEntry { .. } => WorkspaceDomainKind::Play,
        UiCommandPayload::ExportDesktopPackage { .. }
        | UiCommandPayload::BuildAndRunDesktopPackage { .. }
        | UiCommandPayload::BuildReleasePackage { .. }
        | UiCommandPayload::SaveReleaseProfile
        | UiCommandPayload::SetReleaseProfileIcon { .. }
        | UiCommandPayload::OpenBuildOutput
        | UiCommandPayload::OpenBuildReport => WorkspaceDomainKind::Build,
        UiCommandPayload::AiSubmitPrompt { .. }
        | UiCommandPayload::GenerateProjectPatchFromPrompt { .. }
        | UiCommandPayload::SetAiPromptDraft { .. }
        | UiCommandPayload::CancelLlmPatchRequest
        | UiCommandPayload::ImportProjectPatch { .. }
        | UiCommandPayload::PreviewImportedProjectPatch { .. }
        | UiCommandPayload::ApplyImportedProjectPatch { .. }
        | UiCommandPayload::ParkProjectWorkItem { .. }
        | UiCommandPayload::ResumeProjectWorkItem { .. }
        | UiCommandPayload::ReopenProjectWorkItem { .. }
        | UiCommandPayload::ApproveProjectChange { .. }
        | UiCommandPayload::AdvanceProjectProduction { .. }
        | UiCommandPayload::CancelProjectProduction { .. }
        | UiCommandPayload::RecoverProjectProduction { .. }
        | UiCommandPayload::ApproveGatewayAccessRequest { .. }
        | UiCommandPayload::RejectGatewayAccessRequest { .. }
        | UiCommandPayload::SetGatewayAccessPage { .. }
        | UiCommandPayload::AiAcceptProposedCommand { .. }
        | UiCommandPayload::AiRejectProposedCommand { .. } => WorkspaceDomainKind::Report,
        UiCommandPayload::ClearConsole
        | UiCommandPayload::SelectReportEntry { .. }
        | UiCommandPayload::RefreshReports
        | UiCommandPayload::CopyReportAiContext { .. }
        | UiCommandPayload::OpenRawReport { .. }
        | UiCommandPayload::RevealReportPath { .. }
        | UiCommandPayload::OpenRelatedReportArtifact { .. } => WorkspaceDomainKind::Report,
    }
}

pub fn workspace_payload_kind(payload: &UiCommandPayload) -> &'static str {
    match payload {
        UiCommandPayload::OpenProject { .. } => "OpenProject",
        UiCommandPayload::CreateProject { .. } => "CreateProject",
        UiCommandPayload::StartCreateProjectWithAi { .. } => "StartCreateProjectWithAi",
        UiCommandPayload::SelectRecentProject { .. } => "SelectRecentProject",
        UiCommandPayload::RefreshRecentProjects => "RefreshRecentProjects",
        UiCommandPayload::SelectProjectBrowserEntry { .. } => "SelectProjectBrowserEntry",
        UiCommandPayload::OpenProjectBrowserEntry { .. } => "OpenProjectBrowserEntry",
        UiCommandPayload::SelectAssetBrowserEntry { .. } => "SelectAssetBrowserEntry",
        UiCommandPayload::OpenAssetBrowserEntry { .. } => "OpenAssetBrowserEntry",
        UiCommandPayload::SetAssetBrowserFolder { .. } => "SetAssetBrowserFolder",
        UiCommandPayload::SetAssetBrowserSearch { .. } => "SetAssetBrowserSearch",
        UiCommandPayload::SetAssetBrowserKindFilter { .. } => "SetAssetBrowserKindFilter",
        UiCommandPayload::AssetBrowserToolbar { .. } => "AssetBrowserToolbar",
        UiCommandPayload::ScrollAssetBrowser { .. } => "ScrollAssetBrowser",
        UiCommandPayload::BeginAssetPick { .. } => "BeginAssetPick",
        UiCommandPayload::ConfirmAssetPick => "ConfirmAssetPick",
        UiCommandPayload::CancelAssetPick => "CancelAssetPick",
        UiCommandPayload::DropAssetOnInspectorField { .. } => "DropAssetOnInspectorField",
        UiCommandPayload::RegisterExistingAsset { .. } => "RegisterExistingAsset",
        UiCommandPayload::GenerateMockImageAsset { .. } => "GenerateMockImageAsset",
        UiCommandPayload::ValidateAssetBrowserIndex { .. } => "ValidateAssetBrowserIndex",
        UiCommandPayload::CreateDefaultInputMapping { .. } => "CreateDefaultInputMapping",
        UiCommandPayload::DeleteInputMapping { .. } => "DeleteInputMapping",
        UiCommandPayload::OpenInputMapping { .. } => "OpenInputMapping",
        UiCommandPayload::SaveInputMapping { .. } => "SaveInputMapping",
        UiCommandPayload::DiscardInputMappingDraft { .. } => "DiscardInputMappingDraft",
        UiCommandPayload::ValidateInputMapping { .. } => "ValidateInputMapping",
        UiCommandPayload::SelectInputContext { .. } => "SelectInputContext",
        UiCommandPayload::SelectInputAction { .. } => "SelectInputAction",
        UiCommandPayload::SelectInputBinding { .. } => "SelectInputBinding",
        UiCommandPayload::AddInputContext { .. } => "AddInputContext",
        UiCommandPayload::RemoveInputContext { .. } => "RemoveInputContext",
        UiCommandPayload::SetInputContextPriority { .. } => "SetInputContextPriority",
        UiCommandPayload::SetInputContextConsumeInput { .. } => "SetInputContextConsumeInput",
        UiCommandPayload::AddInputAction { .. } => "AddInputAction",
        UiCommandPayload::RemoveInputAction { .. } => "RemoveInputAction",
        UiCommandPayload::SetInputActionValueType { .. } => "SetInputActionValueType",
        UiCommandPayload::AddInputBinding { .. } => "AddInputBinding",
        UiCommandPayload::RemoveInputBinding { .. } => "RemoveInputBinding",
        UiCommandPayload::SetInputBindingDevicePath { .. } => "SetInputBindingDevicePath",
        UiCommandPayload::SetInputBindingProcessorByIndex { .. } => {
            "SetInputBindingProcessorByIndex"
        }
        UiCommandPayload::RemoveInputBindingById { .. } => "RemoveInputBindingById",
        UiCommandPayload::SetInputBindingDevicePathById { .. } => "SetInputBindingDevicePathById",
        UiCommandPayload::SetInputBindingTrigger { .. } => "SetInputBindingTrigger",
        UiCommandPayload::SetInputBindingProcessor { .. } => "SetInputBindingProcessor",
        UiCommandPayload::BeginInputBindingCapture { .. } => "BeginInputBindingCapture",
        UiCommandPayload::CancelInputBindingCapture { .. } => "CancelInputBindingCapture",
        UiCommandPayload::CommitCapturedInputBinding { .. } => "CommitCapturedInputBinding",
        UiCommandPayload::PreviewInputMapping { .. } => "PreviewInputMapping",
        UiCommandPayload::SetInputMappingReportLevel { .. } => "SetInputMappingReportLevel",
        UiCommandPayload::CreateRuleAsset { .. } => "CreateRuleAsset",
        UiCommandPayload::OpenRuleAsset { .. } => "OpenRuleAsset",
        UiCommandPayload::SelectRuleAsset { .. } => "SelectRuleAsset",
        UiCommandPayload::SetRuleTrigger { .. } => "SetRuleTrigger",
        UiCommandPayload::AddRuleStatement { .. } => "AddRuleStatement",
        UiCommandPayload::UpdateRuleStatement { .. } => "UpdateRuleStatement",
        UiCommandPayload::RemoveRuleStatement { .. } => "RemoveRuleStatement",
        UiCommandPayload::AddRuleOperation { .. } => "AddRuleOperation",
        UiCommandPayload::UpdateRuleOperation { .. } => "UpdateRuleOperation",
        UiCommandPayload::RemoveRuleOperation { .. } => "RemoveRuleOperation",
        UiCommandPayload::ValidateRuleAsset { .. } => "ValidateRuleAsset",
        UiCommandPayload::BuildRuleArtifact { .. } => "BuildRuleArtifact",
        UiCommandPayload::BuildProjectRuleManifest { .. } => "BuildProjectRuleManifest",
        UiCommandPayload::SaveRuleAsset { .. } => "SaveRuleAsset",
        UiCommandPayload::OpenRuleDiagnostics { .. } => "OpenRuleDiagnostics",
        UiCommandPayload::SelectRuleCard { .. } => "SelectRuleCard",
        UiCommandPayload::SetRuleCardField { .. } => "SetRuleCardField",
        UiCommandPayload::AddRuleCard { .. } => "AddRuleCard",
        UiCommandPayload::RemoveRuleCard { .. } => "RemoveRuleCard",
        UiCommandPayload::SelectRuleGraphNode { .. } => "SelectRuleGraphNode",
        UiCommandPayload::RefreshRuleGraphPreview { .. } => "RefreshRuleGraphPreview",
        UiCommandPayload::CreatePrefabFromSelection { .. } => "CreatePrefabFromSelection",
        UiCommandPayload::OpenPrefabDocument { .. } => "OpenPrefabDocument",
        UiCommandPayload::EnterPrefabStage { .. } => "EnterPrefabStage",
        UiCommandPayload::ExitPrefabStage { .. } => "ExitPrefabStage",
        UiCommandPayload::InstantiatePrefabInScene { .. } => "InstantiatePrefabInScene",
        UiCommandPayload::SetPrefabStageEntityField { .. } => "SetPrefabStageEntityField",
        UiCommandPayload::ApplyPrefabOverrideToAsset { .. } => "ApplyPrefabOverrideToAsset",
        UiCommandPayload::SavePrefabDocument { .. } => "SavePrefabDocument",
        UiCommandPayload::ValidatePrefabReferences { .. } => "ValidatePrefabReferences",
        UiCommandPayload::RevertPrefabOverride { .. } => "RevertPrefabOverride",
        UiCommandPayload::CreateAuiDocument { .. } => "CreateAuiDocument",
        UiCommandPayload::OpenAuiDocument { .. } => "OpenAuiDocument",
        UiCommandPayload::SelectAuiNode { .. } => "SelectAuiNode",
        UiCommandPayload::AddAuiNode { .. } => "AddAuiNode",
        UiCommandPayload::SetAuiNodeField { .. } => "SetAuiNodeField",
        UiCommandPayload::SetAuiBindingPath { .. } => "SetAuiBindingPath",
        UiCommandPayload::SetAuiActionRef { .. } => "SetAuiActionRef",
        UiCommandPayload::ValidateAuiDocument { .. } => "ValidateAuiDocument",
        UiCommandPayload::SaveAuiDocument { .. } => "SaveAuiDocument",
        UiCommandPayload::PreviewAuiOverlay { .. } => "PreviewAuiOverlay",
        UiCommandPayload::SaveAuiSubtreeAsTemplate { .. } => "SaveAuiSubtreeAsTemplate",
        UiCommandPayload::InstantiateAuiTemplate { .. } => "InstantiateAuiTemplate",
        UiCommandPayload::ValidateAuiTemplate { .. } => "ValidateAuiTemplate",
        UiCommandPayload::SetWorkspaceViewMode { .. } => "SetWorkspaceViewMode",
        UiCommandPayload::SetAuthoringWorkflowStep { .. } => "SetAuthoringWorkflowStep",
        UiCommandPayload::OpenRuntimePackage { .. } => "OpenRuntimePackage",
        UiCommandPayload::OpenSceneDocument { .. } => "OpenSceneDocument",
        UiCommandPayload::ReloadRuntimePackage => "ReloadRuntimePackage",
        UiCommandPayload::SelectEntity { .. } => "SelectEntity",
        UiCommandPayload::SelectRuntimeEntity { .. } => "SelectRuntimeEntity",
        UiCommandPayload::PickRuntimeEntityAt { .. } => "PickRuntimeEntityAt",
        UiCommandPayload::SelectSceneEntity { .. } => "SelectSceneEntity",
        UiCommandPayload::CreateSceneEntity { .. } => "CreateSceneEntity",
        UiCommandPayload::PlaceAssetIntoScene { .. } => "PlaceAssetIntoScene",
        UiCommandPayload::DeleteSceneEntity { .. } => "DeleteSceneEntity",
        UiCommandPayload::RenameSceneEntity { .. } => "RenameSceneEntity",
        UiCommandPayload::SetSceneTransform { .. } => "SetSceneTransform",
        UiCommandPayload::AddSceneComponent { .. } => "AddSceneComponent",
        UiCommandPayload::RemoveSceneComponent { .. } => "RemoveSceneComponent",
        UiCommandPayload::SetSceneComponentField { .. } => "SetSceneComponentField",
        UiCommandPayload::SetRuntimeComponentFieldTemporary { .. } => {
            "SetRuntimeComponentFieldTemporary"
        }
        UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring => {
            "PreviewApplyRuntimeChangeToAuthoring"
        }
        UiCommandPayload::ApplyRuntimeChangeToAuthoring { .. } => "ApplyRuntimeChangeToAuthoring",
        UiCommandPayload::SaveSceneDocument { .. } => "SaveSceneDocument",
        UiCommandPayload::UndoSceneEdit => "UndoSceneEdit",
        UiCommandPayload::RedoSceneEdit => "RedoSceneEdit",
        UiCommandPayload::TickOneFrame => "TickOneFrame",
        UiCommandPayload::Play => "Play",
        UiCommandPayload::Pause => "Pause",
        UiCommandPayload::StepFrame => "StepFrame",
        UiCommandPayload::StopPlaySession => "StopPlaySession",
        UiCommandPayload::SetGameViewTarget { .. } => "SetGameViewTarget",
        UiCommandPayload::SetGameViewMaximizeOnPlay { .. } => "SetGameViewMaximizeOnPlay",
        UiCommandPayload::ToggleGameViewMaximizeOnPlay => "ToggleGameViewMaximizeOnPlay",
        UiCommandPayload::ResetRuntime => "ResetRuntime",
        UiCommandPayload::ExportDesktopPackage { .. } => "ExportDesktopPackage",
        UiCommandPayload::BuildAndRunDesktopPackage { .. } => "BuildAndRunDesktopPackage",
        UiCommandPayload::BuildReleasePackage { .. } => "BuildReleasePackage",
        UiCommandPayload::SaveReleaseProfile => "SaveReleaseProfile",
        UiCommandPayload::SetReleaseProfileIcon { .. } => "SetReleaseProfileIcon",
        UiCommandPayload::OpenBuildOutput => "OpenBuildOutput",
        UiCommandPayload::OpenBuildReport => "OpenBuildReport",
        UiCommandPayload::ClearConsole => "ClearConsole",
        UiCommandPayload::SelectTraceEntry { .. } => "SelectTraceEntry",
        UiCommandPayload::AiSubmitPrompt { .. } => "AiSubmitPrompt",
        UiCommandPayload::GenerateProjectPatchFromPrompt { .. } => "GenerateProjectPatchFromPrompt",
        UiCommandPayload::SetAiPromptDraft { .. } => "SetAiPromptDraft",
        UiCommandPayload::CancelLlmPatchRequest => "CancelLlmPatchRequest",
        UiCommandPayload::ImportProjectPatch { .. } => "ImportProjectPatch",
        UiCommandPayload::PreviewImportedProjectPatch { .. } => "PreviewImportedProjectPatch",
        UiCommandPayload::ApplyImportedProjectPatch { .. } => "ApplyImportedProjectPatch",
        UiCommandPayload::ParkProjectWorkItem { .. } => "ParkProjectWorkItem",
        UiCommandPayload::ResumeProjectWorkItem { .. } => "ResumeProjectWorkItem",
        UiCommandPayload::ReopenProjectWorkItem { .. } => "ReopenProjectWorkItem",
        UiCommandPayload::ApproveProjectChange { .. } => "ApproveProjectChange",
        UiCommandPayload::AdvanceProjectProduction { .. } => "AdvanceProjectProduction",
        UiCommandPayload::CancelProjectProduction { .. } => "CancelProjectProduction",
        UiCommandPayload::RecoverProjectProduction { .. } => "RecoverProjectProduction",
        UiCommandPayload::ApproveGatewayAccessRequest { .. } => "ApproveGatewayAccessRequest",
        UiCommandPayload::RejectGatewayAccessRequest { .. } => "RejectGatewayAccessRequest",
        UiCommandPayload::SetGatewayAccessPage { .. } => "SetGatewayAccessPage",
        UiCommandPayload::ApproveProjectRuntimeTrust { .. } => "ApproveProjectRuntimeTrust",
        UiCommandPayload::DenyProjectRuntimeTrust { .. } => "DenyProjectRuntimeTrust",
        UiCommandPayload::CancelProjectRuntimeTrust { .. } => "CancelProjectRuntimeTrust",
        UiCommandPayload::AiAcceptProposedCommand { .. } => "AiAcceptProposedCommand",
        UiCommandPayload::AiRejectProposedCommand { .. } => "AiRejectProposedCommand",
        UiCommandPayload::SelectReportEntry { .. } => "SelectReportEntry",
        UiCommandPayload::RefreshReports => "RefreshReports",
        UiCommandPayload::CopyReportAiContext { .. } => "CopyReportAiContext",
        UiCommandPayload::OpenRawReport { .. } => "OpenRawReport",
        UiCommandPayload::RevealReportPath { .. } => "RevealReportPath",
        UiCommandPayload::OpenRelatedReportArtifact { .. } => "OpenRelatedReportArtifact",
    }
}

fn workspace_domain_label(kind: WorkspaceDomainKind) -> &'static str {
    match kind {
        WorkspaceDomainKind::Project => "Project",
        WorkspaceDomainKind::Scene => "Scene",
        WorkspaceDomainKind::Asset => "Asset",
        WorkspaceDomainKind::Prefab => "Prefab",
        WorkspaceDomainKind::Rule => "Rule",
        WorkspaceDomainKind::Aui => "AUI",
        WorkspaceDomainKind::Input => "Input",
        WorkspaceDomainKind::Play => "Play",
        WorkspaceDomainKind::Build => "Build",
        WorkspaceDomainKind::Report => "Report",
    }
}
