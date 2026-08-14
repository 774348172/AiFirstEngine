use serde::{Deserialize, Serialize};

use super::{
    AiPanelModel, Animator2DAuthoringModel, AssetBrowserModel, AuthoringWorkflowModel,
    BuildExportModel, ConsoleModel, EditorDiagnostic, HierarchyModel, InputMappingAuthoringModel,
    InspectorModel, PanelLayoutModel, ProjectAuthoringWorkspaceModel, ProjectBrowserModel,
    ProjectIntentWorkspaceModel, ProjectLauncherModel, ProjectRuntimeTrustPromptModel,
    ReportPanelModel, RuleAuthoringModel, RuntimePackageSummary, RuntimeTraceModel, ToolbarModel,
    UiCommandSource, ViewportModel,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorUiModel {
    pub revision: u64,
    pub frame: u64,
    pub mode: EditorUiMode,
    pub project_launcher: ProjectLauncherModel,
    pub project_intent: ProjectIntentWorkspaceModel,
    pub project_browser: ProjectBrowserModel,
    pub asset_browser: AssetBrowserModel,
    pub build_export: BuildExportModel,
    pub report_panel: ReportPanelModel,
    pub input_mapping_authoring: InputMappingAuthoringModel,
    pub rule_authoring: RuleAuthoringModel,
    #[serde(default)]
    pub animator2d_authoring: Animator2DAuthoringModel,
    pub project_authoring_workspace: ProjectAuthoringWorkspaceModel,
    pub authoring_workflow: AuthoringWorkflowModel,
    pub workspace_view_mode: WorkspaceViewMode,
    pub active_runtime_package: Option<RuntimePackageSummary>,
    pub panels: PanelLayoutModel,
    pub toolbar: ToolbarModel,
    pub hierarchy: HierarchyModel,
    pub inspector: InspectorModel,
    pub viewport: ViewportModel,
    pub console: ConsoleModel,
    pub runtime_trace: RuntimeTraceModel,
    pub ai_panel: AiPanelModel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_runtime_trust_prompt: Option<ProjectRuntimeTrustPromptModel>,
    pub interaction_feedback: Option<EditorCommandFeedback>,
    pub diagnostics: Vec<EditorDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCommandFeedback {
    pub command_id: String,
    pub status: EditorCommandFeedbackStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<String>,
    pub message: String,
    pub reason: Option<String>,
    pub source: UiCommandSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorCommandFeedbackStatus {
    Committed,
    Rejected,
    Failed,
    Disabled,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorUiMode {
    ProjectLauncher,
    AuthoringWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceViewMode {
    SceneView,
    GameView,
}
