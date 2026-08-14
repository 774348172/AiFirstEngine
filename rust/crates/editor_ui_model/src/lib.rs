mod ai_panel;
mod animator2d;
mod asset_browser;
mod aui_scene_authoring;
mod authoring_workflow;
mod build_export;
mod command;
mod console;
mod diagnostics;
mod hierarchy;
mod input_mapping;
mod inspector;
mod launcher;
mod layout;
mod localization;
mod manual_walkthrough;
mod model;
mod project_browser;
mod project_intent;
mod project_runtime_trust;
mod report_panel;
mod rule_authoring;
mod runtime_trace;
mod toolbar;
mod viewport;
mod workflow_command;
mod workspace;

pub use ai_panel::{
    AiCommandReviewState, AiPanelMessage, AiPanelMessageRole, AiPanelModel, AiPanelResponse,
    AiPanelStage, AiProposedCommand, GatewayAccessInboxModel, GatewayAccessRequestModel,
    ImportedProjectPatchEvidence, ProjectPatchDiagnosticEvidence, ProjectPatchEvidence,
};
pub use animator2d::*;
pub use asset_browser::{
    AssetBrowserCommand, AssetBrowserDiagnostic, AssetBrowserDiagnosticSeverity, AssetBrowserEntry,
    AssetBrowserIndexProgress, AssetBrowserIndexStatus, AssetBrowserModel, AssetBrowserReport,
    AssetBrowserToolbarAction, AssetBrowserViewMode, AssetDragPayload, AssetDropTargetKind,
    AssetEntryKey, AssetEntryRole, AssetIdentityStatus, AssetKind, AssetPickRequest,
    AssetPickResult, AssetPickTargetKind, AssetPickerModel, AssetPreviewDescriptor,
    AssetPreviewKind, AssetPreviewStatus, AssetQuery, AssetSelection, AssetSourceStatus,
    AssetThumbnailAspectRatio, EditorAssetRef,
};
pub use aui_scene_authoring::{
    AuiAuthoringVec2, AuiComputedAuthoringRect, AuiNodeAuthoringProxy, AuiSceneHitTestStatus,
    AuiSceneReorderStatus, AuiSceneUnifiedAuthoringReport, AuiSceneUnifiedAuthoringStatus,
    AuiSceneViewProjection, AuiSourceRect, SceneVisualOrderAuthoringEntry,
    SceneVisualOrderAuthoringModel, SceneVisualOrderRenderSpace, SceneVisualOrderTargetKind,
    VisualOrderIntent, VisualOrderIntentRelation, VisualOrderKey,
    AUI_SCENE_UNIFIED_AUTHORING_REPORT_SCHEMA_VERSION,
    SCENE_VISUAL_ORDER_AUTHORING_MODEL_SCHEMA_VERSION,
};
pub use authoring_workflow::{
    AuiAuthoringAiContextSummary, AuthoringAiContext, AuthoringCommand,
    AuthoringCommandAvailability, AuthoringIssue, AuthoringIssueSeverity, AuthoringStepCompletion,
    AuthoringStepId, AuthoringStepStatus, AuthoringTask, AuthoringTaskPriority,
    AuthoringWorkflowModel, AuthoringWorkflowStep, PrefabAuthoringAiContextSummary,
    ProjectPatchAiContextSummary, AUTHORING_WORKFLOW_SCHEMA_VERSION,
};
pub use build_export::{
    BuildExportCommand, BuildExportModel, BuildExportReportSummary, BuildProfileSummary,
    ReleaseBuildProfileModel, ReleasePackageReportSummary,
};
pub use command::{
    ui_command_id_for_payload, PrefabStageMode, PrefabStageSavePolicy, UiCommand, UiCommandPayload,
    UiCommandSource,
};
pub use console::{ConsoleEntry, ConsoleLevel, ConsoleModel, ConsoleSource};
pub use diagnostics::{DiagnosticSeverity, DiagnosticSource, EditorDiagnostic};
pub use hierarchy::{HierarchyAuthoringView, HierarchyModel, HierarchyNode, HierarchySourceDomain};
pub use input_mapping::{
    InputActionValueKind, InputControlCatalogEntryModel, InputControlCatalogModel,
    InputControlDeviceKindModel, InputMappingActionSummary, InputMappingAuthoringCommand,
    InputMappingAuthoringDiagnostic, InputMappingAuthoringModel, InputMappingAuthoringReport,
    InputMappingBindingSummary, InputMappingContextSummary, InputMappingDiagnosticSeverity,
    InputMappingPreviewAction, InputMappingPreviewResult, InputMappingPreviewStatus,
    InputMappingReportLevel, InputMappingValidationStatus, InputProcessorKind, InputTriggerKind,
};
pub use inspector::{
    InspectorField, InspectorModel, InspectorPersistence, InspectorSection, InspectorValue,
    InspectorValueType, Vec3,
};
pub use launcher::{
    ProjectLauncherCommand, ProjectLauncherModel, ProjectOpenActivityModel,
    ProjectOpenActivityPhase, RecentProjectEntry, RuntimePackageSummary,
};
pub use layout::{PanelLayoutMode, PanelLayoutModel, PanelRegion};
pub use localization::{
    trusted_editor_localization_bundle, EditorCatalogDiagnostic, EditorCatalogDiagnosticCode,
    EditorInvariantText, EditorLocaleChangeResult, EditorLocaleDescriptor, EditorLocaleId,
    EditorLocalizationBundle, EditorLocalizationCatalog, EditorLocalizationSnapshot,
    EditorMessageArgType, EditorMessageArgs, EditorMessageKey, EditorMessageValue, EditorTextRef,
    EDITOR_LOCALE_EN_US, EDITOR_LOCALE_ZH_CN, EDITOR_LOCALIZATION_CATALOG_SCHEMA_VERSION,
};
pub use manual_walkthrough::{
    manual_authoring_operation_requirements, ManualAuthoringOperationRequirement,
    ManualAuthoringOperationStatus, ManualWalkthroughCoverageReport,
    ManualWalkthroughCoverageStatus, ManualWalkthroughCoverageSummary,
    ManualWalkthroughDomainSummary, ManualWalkthroughOperationCoverage,
    ManualWalkthroughRequiredContext, MissingOperationGap, MissingOperationSeverity,
    MANUAL_WALKTHROUGH_COVERAGE_REPORT_SCHEMA_VERSION,
};
pub use model::{
    EditorCommandFeedback, EditorCommandFeedbackStatus, EditorUiMode, EditorUiModel,
    WorkspaceViewMode,
};
pub use project_browser::{ProjectBrowserEntry, ProjectBrowserEntryKind, ProjectBrowserModel};
pub use project_intent::{
    ProjectChangeReviewModel, ProjectIntentModel, ProjectIntentReportLevel,
    ProjectIntentWorkItemModel, ProjectIntentWorkspaceModel, ProjectProductionModel,
};
pub use project_runtime_trust::ProjectRuntimeTrustPromptModel;
pub use report_panel::{
    EvidenceEntry, ReportAiContext, ReportArtifactRef, ReportCapability, ReportDescriptor,
    ReportPanelFilters, ReportPanelModel, ReportPanelSummary, ReportRegistrySummary,
    ReportSourceKind, ReportStatus, UnifiedReportEntry, REPORT_PANEL_SCHEMA_VERSION,
};
pub use rule_authoring::{
    RuleAuthoringCommand, RuleAuthoringDiagnostic, RuleAuthoringDiagnosticSeverity,
    RuleAuthoringDocument, RuleAuthoringModel, RuleAuthoringPatch, RuleAuthoringPatchOperation,
    RuleAuthoringPatchSource, RuleAuthoringReport, RuleAuthoringStageEvidence,
    RuleAuthoringStageStatus, RuleAuthoringStatus, RuleCardAuthoringModel, RuleCardAuthoringReport,
    RuleCardDiagnosticRef, RuleCardFieldModel, RuleCardFieldValueKind, RuleCardKind, RuleCardModel,
    RuleCardSourceMapping, RuleCardValidationState, RuleGraphPreviewEdge, RuleGraphPreviewEdgeKind,
    RuleGraphPreviewGroup, RuleGraphPreviewModel, RuleGraphPreviewNode, RuleGraphPreviewNodeKind,
    RuleGraphPreviewNodeStatus, RULE_AUTHORING_REPORT_SCHEMA_VERSION,
    RULE_CARD_AUTHORING_REPORT_SCHEMA_VERSION, RULE_GRAPH_PREVIEW_SCHEMA_VERSION,
};
pub use runtime_trace::{RuntimeTraceEntryView, RuntimeTraceModel, TraceLevel};
pub use toolbar::{
    EditorGameViewScalePolicy, EditorGameViewTarget, GameViewLayoutState, RuntimeRunState,
    ToolbarCommand, ToolbarModel,
};
pub use viewport::{
    AssetPlacementMode, ColliderOverlayDiagnostic, ColliderOverlayItem, ColliderOverlayModel,
    ColliderOverlayShape, EntitySummary, RenderableSummary, ViewportModel,
};
pub use workflow_command::{WorkflowCommandResolution, WorkflowCommandResolver};
pub use workspace::{
    workspace_domain_for_payload, workspace_payload_kind, ProjectAuthoringWorkspaceModel,
    WorkspaceCommandSummary, WorkspaceDiagnosticsSummary, WorkspaceDocumentSummary,
    WorkspaceDomainKind, WorkspaceDomainStatus, WorkspaceDomainSummary, WorkspaceReportSummary,
    WorkspaceSelectionSummary, WorkspaceSelectionTarget, WorkspaceTransactionSummary,
};

#[cfg(test)]
mod tests;
