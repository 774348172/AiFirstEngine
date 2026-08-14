use serde::{Deserialize, Serialize};

use super::WorkspaceDomainKind;

pub const MANUAL_WALKTHROUGH_COVERAGE_REPORT_SCHEMA_VERSION: &str =
    "manual-walkthrough-coverage-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualAuthoringOperationRequirement {
    pub operation_id: String,
    pub domain: WorkspaceDomainKind,
    pub title: String,
    pub user_goal: String,
    pub required_for_play: bool,
    pub required_for_build: bool,
    pub required_for_complex_project: bool,
    pub expected_command_id: Option<String>,
    pub expected_payload_kind: Option<String>,
    pub required_context: Vec<ManualWalkthroughRequiredContext>,
    pub fallback_behavior: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManualWalkthroughRequiredContext {
    None,
    OpenProject,
    SelectedAsset,
    SelectedEntity,
    SelectedInputMapping,
    SelectedAuiDocument,
    SelectedPrefab,
    SelectedRule,
    BuildProfile,
    OpenSceneDocument,
    RuntimePackage,
    TraceEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManualAuthoringOperationStatus {
    ExecutableCommand,
    ExecutableCommandNeedsContext,
    FocusDomainPanel,
    MissingCommand,
    MissingDomainService,
    BlockedByDependency,
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualWalkthroughOperationCoverage {
    pub requirement: ManualAuthoringOperationRequirement,
    pub status: ManualAuthoringOperationStatus,
    pub resolution_summary: String,
    pub next_action: Option<String>,
    pub gap_id: Option<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManualWalkthroughCoverageStatus {
    Pass,
    Partial,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualWalkthroughCoverageReport {
    pub schema_version: String,
    pub project_id: Option<String>,
    pub scenario_id: String,
    pub status: ManualWalkthroughCoverageStatus,
    pub operation_count: usize,
    pub executable_count: usize,
    pub needs_context_count: usize,
    pub focus_panel_count: usize,
    pub missing_command_count: usize,
    pub missing_service_count: usize,
    pub blocked_count: usize,
    pub operations: Vec<ManualWalkthroughOperationCoverage>,
    pub domain_summaries: Vec<ManualWalkthroughDomainSummary>,
    pub blocking_gaps: Vec<MissingOperationGap>,
    pub next_actions: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl ManualWalkthroughCoverageReport {
    pub fn from_operations(
        project_id: Option<String>,
        scenario_id: impl Into<String>,
        operations: Vec<ManualWalkthroughOperationCoverage>,
        diagnostics: Vec<String>,
    ) -> Self {
        let operation_count = operations.len();
        let executable_count = count_status(
            &operations,
            ManualAuthoringOperationStatus::ExecutableCommand,
        );
        let needs_context_count = count_status(
            &operations,
            ManualAuthoringOperationStatus::ExecutableCommandNeedsContext,
        );
        let focus_panel_count = count_status(
            &operations,
            ManualAuthoringOperationStatus::FocusDomainPanel,
        );
        let missing_command_count =
            count_status(&operations, ManualAuthoringOperationStatus::MissingCommand);
        let missing_service_count = count_status(
            &operations,
            ManualAuthoringOperationStatus::MissingDomainService,
        );
        let blocked_count = count_status(
            &operations,
            ManualAuthoringOperationStatus::BlockedByDependency,
        );
        let blocking_gaps = operations
            .iter()
            .filter_map(|operation| operation_gap(operation))
            .collect::<Vec<_>>();
        let next_actions = next_actions_from_gaps(&blocking_gaps);
        let domain_summaries = WorkspaceDomainKind::all()
            .into_iter()
            .filter_map(|domain| summarize_domain(domain, &operations))
            .collect::<Vec<_>>();
        let status = if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("fail"))
        {
            ManualWalkthroughCoverageStatus::Fail
        } else if missing_command_count
            + missing_service_count
            + blocked_count
            + needs_context_count
            > 0
        {
            ManualWalkthroughCoverageStatus::Partial
        } else {
            ManualWalkthroughCoverageStatus::Pass
        };

        Self {
            schema_version: MANUAL_WALKTHROUGH_COVERAGE_REPORT_SCHEMA_VERSION.to_string(),
            project_id,
            scenario_id: scenario_id.into(),
            status,
            operation_count,
            executable_count,
            needs_context_count,
            focus_panel_count,
            missing_command_count,
            missing_service_count,
            blocked_count,
            operations,
            domain_summaries,
            blocking_gaps,
            next_actions,
            diagnostics,
        }
    }

    pub fn summary(&self) -> ManualWalkthroughCoverageSummary {
        ManualWalkthroughCoverageSummary {
            status: self.status,
            operation_count: self.operation_count,
            executable_count: self.executable_count,
            needs_context_count: self.needs_context_count,
            focus_panel_count: self.focus_panel_count,
            missing_command_count: self.missing_command_count,
            missing_service_count: self.missing_service_count,
            blocked_count: self.blocked_count,
            top_next_actions: self.next_actions.iter().take(5).cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualWalkthroughDomainSummary {
    pub domain: WorkspaceDomainKind,
    pub operation_count: usize,
    pub executable_count: usize,
    pub needs_context_count: usize,
    pub focus_panel_count: usize,
    pub missing_count: usize,
    pub blocked_count: usize,
    pub status: ManualWalkthroughCoverageStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingOperationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingOperationGap {
    pub gap_id: String,
    pub domain: WorkspaceDomainKind,
    pub operation_id: String,
    pub severity: MissingOperationSeverity,
    pub reason: String,
    pub suggested_system: String,
    pub suggested_next_action: String,
    pub blocks_manual_walkthrough: bool,
    pub blocks_play: bool,
    pub blocks_build: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualWalkthroughCoverageSummary {
    pub status: ManualWalkthroughCoverageStatus,
    pub operation_count: usize,
    pub executable_count: usize,
    pub needs_context_count: usize,
    pub focus_panel_count: usize,
    pub missing_command_count: usize,
    pub missing_service_count: usize,
    pub blocked_count: usize,
    pub top_next_actions: Vec<String>,
}

pub fn manual_authoring_operation_requirements() -> Vec<ManualAuthoringOperationRequirement> {
    vec![
        op(
            "open_project",
            WorkspaceDomainKind::Project,
            "Open Project",
            "Open an existing project for editing.",
            true,
            true,
            true,
            Some("open_project"),
            Some("OpenProject"),
            &[ManualWalkthroughRequiredContext::None],
            "Use the project launcher or recent projects list.",
        ),
        op(
            "create_project",
            WorkspaceDomainKind::Project,
            "Create Project",
            "Create a new editable project.",
            false,
            false,
            true,
            Some("create_project"),
            Some("CreateProject"),
            &[ManualWalkthroughRequiredContext::None],
            "Use the project launcher create flow.",
        ),
        op(
            "refresh_recent_projects",
            WorkspaceDomainKind::Project,
            "Refresh Recent Projects",
            "Refresh known project entries.",
            false,
            false,
            false,
            Some("refresh_recent_projects"),
            Some("RefreshRecentProjects"),
            &[ManualWalkthroughRequiredContext::None],
            "Use launcher refresh.",
        ),
        op(
            "save_project",
            WorkspaceDomainKind::Project,
            "Save Project",
            "Persist project-level metadata.",
            true,
            true,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use project domain save when productized.",
        ),
        op(
            "reload_project",
            WorkspaceDomainKind::Project,
            "Reload Project",
            "Reload project metadata from disk.",
            false,
            true,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use project domain reload when productized.",
        ),
        op(
            "browse_assets",
            WorkspaceDomainKind::Asset,
            "Browse Assets",
            "Inspect project assets.",
            false,
            true,
            true,
            Some("select_project_browser_entry"),
            Some("SelectProjectBrowserEntry"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Focus the asset browser for selection.",
        ),
        op(
            "select_asset",
            WorkspaceDomainKind::Asset,
            "Select Asset",
            "Select an asset for inspection or placement.",
            false,
            true,
            true,
            Some("select_project_browser_entry"),
            Some("SelectProjectBrowserEntry"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Focus the asset browser when no asset is selected.",
        ),
        op(
            "open_asset",
            WorkspaceDomainKind::Asset,
            "Open Asset",
            "Open an asset document or preview.",
            false,
            true,
            true,
            Some("open_project_browser_entry"),
            Some("OpenProjectBrowserEntry"),
            &[ManualWalkthroughRequiredContext::SelectedAsset],
            "Use asset browser open when a selected asset exists.",
        ),
        op(
            "import_asset",
            WorkspaceDomainKind::Asset,
            "Import Asset",
            "Import a new source asset into the project library.",
            false,
            true,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use asset import productization when available.",
        ),
        op(
            "place_asset_into_scene",
            WorkspaceDomainKind::Asset,
            "Place Asset Into Scene",
            "Place a selected asset in the active scene.",
            true,
            true,
            true,
            Some("place_asset_into_scene"),
            Some("PlaceAssetIntoScene"),
            &[
                ManualWalkthroughRequiredContext::SelectedAsset,
                ManualWalkthroughRequiredContext::OpenSceneDocument,
            ],
            "Focus asset and scene domains until selection context exists.",
        ),
        op(
            "validate_asset_references",
            WorkspaceDomainKind::Asset,
            "Validate Asset References",
            "Check asset references used by project documents.",
            false,
            true,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use asset validation when productized.",
        ),
        op(
            "open_scene_document",
            WorkspaceDomainKind::Scene,
            "Open Scene Document",
            "Open an editable scene document.",
            true,
            true,
            true,
            Some("open_scene_document"),
            Some("OpenSceneDocument"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Focus scene or project browser when no scene path exists.",
        ),
        op(
            "create_scene_document",
            WorkspaceDomainKind::Scene,
            "Create Scene Document",
            "Create a new editable scene document.",
            true,
            true,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use scene document creation when productized.",
        ),
        op(
            "create_scene_entity",
            WorkspaceDomainKind::Scene,
            "Create Scene Entity",
            "Create an entity in the active scene.",
            true,
            true,
            true,
            Some("create_scene_entity"),
            Some("CreateSceneEntity"),
            &[ManualWalkthroughRequiredContext::OpenSceneDocument],
            "Focus scene panel when parent/name context is missing.",
        ),
        op(
            "select_scene_entity",
            WorkspaceDomainKind::Scene,
            "Select Scene Entity",
            "Select a scene entity for inspection.",
            true,
            false,
            true,
            Some("select_scene_entity"),
            Some("SelectSceneEntity"),
            &[ManualWalkthroughRequiredContext::OpenSceneDocument],
            "Use hierarchy selection.",
        ),
        op(
            "rename_scene_entity",
            WorkspaceDomainKind::Scene,
            "Rename Scene Entity",
            "Rename the selected scene entity.",
            false,
            false,
            true,
            Some("rename_scene_entity"),
            Some("RenameSceneEntity"),
            &[ManualWalkthroughRequiredContext::SelectedEntity],
            "Use inspector or hierarchy rename.",
        ),
        op(
            "delete_scene_entity",
            WorkspaceDomainKind::Scene,
            "Delete Scene Entity",
            "Delete the selected scene entity.",
            false,
            false,
            true,
            Some("delete_scene_entity"),
            Some("DeleteSceneEntity"),
            &[ManualWalkthroughRequiredContext::SelectedEntity],
            "Use hierarchy delete.",
        ),
        op(
            "set_scene_transform",
            WorkspaceDomainKind::Scene,
            "Set Scene Transform",
            "Edit transform fields on the selected entity.",
            true,
            true,
            true,
            Some("set_scene_transform"),
            Some("SetSceneTransform"),
            &[ManualWalkthroughRequiredContext::SelectedEntity],
            "Use inspector transform editing.",
        ),
        op(
            "add_component",
            WorkspaceDomainKind::Scene,
            "Add Component",
            "Add a component to the selected entity.",
            true,
            true,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::SelectedEntity],
            "Use schema-driven inspector component actions when productized.",
        ),
        op(
            "set_scene_component_field",
            WorkspaceDomainKind::Scene,
            "Set Scene Component Field",
            "Edit a component field on the selected entity.",
            true,
            true,
            true,
            Some("set_scene_component_field"),
            Some("SetSceneComponentField"),
            &[ManualWalkthroughRequiredContext::SelectedEntity],
            "Use schema-driven inspector field editing.",
        ),
        op(
            "save_scene_document",
            WorkspaceDomainKind::Scene,
            "Save Scene Document",
            "Save the active scene document.",
            true,
            true,
            true,
            Some("save_scene_document"),
            Some("SaveSceneDocument"),
            &[ManualWalkthroughRequiredContext::OpenSceneDocument],
            "Use toolbar save when a scene is open.",
        ),
        op(
            "undo_scene_edit",
            WorkspaceDomainKind::Scene,
            "Undo Scene Edit",
            "Undo the latest scene edit.",
            false,
            false,
            true,
            Some("undo_scene_edit"),
            Some("UndoSceneEdit"),
            &[ManualWalkthroughRequiredContext::OpenSceneDocument],
            "Use scene undo stack.",
        ),
        op(
            "redo_scene_edit",
            WorkspaceDomainKind::Scene,
            "Redo Scene Edit",
            "Redo the latest scene edit.",
            false,
            false,
            true,
            Some("redo_scene_edit"),
            Some("RedoSceneEdit"),
            &[ManualWalkthroughRequiredContext::OpenSceneDocument],
            "Use scene undo stack.",
        ),
        op(
            "create_prefab_from_selection",
            WorkspaceDomainKind::Prefab,
            "Create Prefab From Selection",
            "Create a reusable prefab from selected scene content.",
            true,
            true,
            true,
            Some("create_prefab_from_selection"),
            Some("CreatePrefabFromSelection"),
            &[ManualWalkthroughRequiredContext::SelectedEntity],
            "Use Prefab Authoring Productization v1 create command.",
        ),
        op(
            "open_prefab_document",
            WorkspaceDomainKind::Prefab,
            "Open Prefab Document",
            "Open a prefab document for editing.",
            true,
            true,
            true,
            Some("open_prefab_document"),
            Some("OpenPrefabDocument"),
            &[ManualWalkthroughRequiredContext::SelectedPrefab],
            "Open the selected prefab asset into Prefab Stage.",
        ),
        op(
            "enter_prefab_stage",
            WorkspaceDomainKind::Prefab,
            "Enter Prefab Stage",
            "Enter an isolated prefab editing context.",
            false,
            true,
            true,
            Some("enter_prefab_stage"),
            Some("EnterPrefabStage"),
            &[ManualWalkthroughRequiredContext::SelectedPrefab],
            "Use the Prefab Stage command with selected prefab context.",
        ),
        op(
            "exit_prefab_stage",
            WorkspaceDomainKind::Prefab,
            "Exit Prefab Stage",
            "Exit the prefab editing context with an explicit save policy.",
            false,
            true,
            true,
            Some("exit_prefab_stage"),
            Some("ExitPrefabStage"),
            &[ManualWalkthroughRequiredContext::SelectedPrefab],
            "Use save, discard, or keep-open policy.",
        ),
        op(
            "instantiate_prefab_in_scene",
            WorkspaceDomainKind::Prefab,
            "Instantiate Prefab In Scene",
            "Place a prefab instance in the active scene.",
            true,
            true,
            true,
            Some("instantiate_prefab_in_scene"),
            Some("InstantiatePrefabInScene"),
            &[
                ManualWalkthroughRequiredContext::SelectedPrefab,
                ManualWalkthroughRequiredContext::OpenSceneDocument,
            ],
            "Use prefab instantiate command in the active scene.",
        ),
        op(
            "set_prefab_stage_entity_field",
            WorkspaceDomainKind::Prefab,
            "Edit Prefab Stage Field",
            "Edit a prefab asset field inside Prefab Stage.",
            false,
            true,
            true,
            Some("set_prefab_stage_entity_field"),
            Some("SetPrefabStageEntityField"),
            &[ManualWalkthroughRequiredContext::SelectedPrefab],
            "Use Prefab Stage field edit command.",
        ),
        op(
            "apply_prefab_changes",
            WorkspaceDomainKind::Prefab,
            "Apply Prefab Changes",
            "Apply edited prefab overrides.",
            false,
            true,
            true,
            Some("apply_prefab_changes"),
            Some("ApplyPrefabOverrideToAsset"),
            &[ManualWalkthroughRequiredContext::SelectedPrefab],
            "Apply one prefab override to the source asset.",
        ),
        op(
            "revert_prefab_override",
            WorkspaceDomainKind::Prefab,
            "Revert Prefab Override",
            "Revert one prefab instance override.",
            false,
            true,
            true,
            Some("revert_prefab_override"),
            Some("RevertPrefabOverride"),
            &[ManualWalkthroughRequiredContext::SelectedPrefab],
            "Remove one override from the prefab instance.",
        ),
        op(
            "save_prefab_document",
            WorkspaceDomainKind::Prefab,
            "Save Prefab Document",
            "Save prefab edits.",
            true,
            true,
            true,
            Some("save_prefab_document"),
            Some("SavePrefabDocument"),
            &[ManualWalkthroughRequiredContext::SelectedPrefab],
            "Save the active Prefab Stage working asset.",
        ),
        op(
            "validate_prefab_references",
            WorkspaceDomainKind::Prefab,
            "Validate Prefab References",
            "Validate prefab references and overrides.",
            true,
            true,
            true,
            Some("validate_prefab_references"),
            Some("ValidatePrefabReferences"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use prefab validation report.",
        ),
        op(
            "create_rule_asset",
            WorkspaceDomainKind::Rule,
            "Create Rule Asset",
            "Create a project rule asset.",
            true,
            true,
            true,
            Some("create_rule_asset"),
            Some("CreateRuleAsset"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use rule authoring productization.",
        ),
        op(
            "open_rule_asset",
            WorkspaceDomainKind::Rule,
            "Open Rule Asset",
            "Open a rule asset for editing.",
            true,
            true,
            true,
            Some("open_rule_asset"),
            Some("OpenRuleAsset"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Open the selected rule asset.",
        ),
        op(
            "edit_rule_graph_or_dsl",
            WorkspaceDomainKind::Rule,
            "Edit Rule Card",
            "Edit project rule logic through editable Rule Cards.",
            true,
            true,
            true,
            Some("set_rule_card_field"),
            Some("SetRuleCardField"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Use Rule Card authoring productization.",
        ),
        op(
            "add_rule_card",
            WorkspaceDomainKind::Rule,
            "Add Rule Card",
            "Add a statement or operation card to a rule asset.",
            true,
            true,
            true,
            Some("add_rule_card"),
            Some("AddRuleCard"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Add statements or operations through RuleAuthoringService.",
        ),
        op(
            "preview_rule_graph",
            WorkspaceDomainKind::Rule,
            "Preview Rule Graph",
            "Refresh the generated read-only Rule Graph Preview.",
            false,
            true,
            true,
            Some("refresh_rule_graph_preview"),
            Some("RefreshRuleGraphPreview"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Use the generated read-only graph preview.",
        ),
        op(
            "select_rule_graph_node",
            WorkspaceDomainKind::Rule,
            "Select Rule Graph Node",
            "Select a graph node and locate its source card/path.",
            false,
            true,
            true,
            Some("select_rule_graph_node"),
            Some("SelectRuleGraphNode"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Selection is editor-only and does not mutate the Rule asset.",
        ),
        op(
            "validate_rule_asset",
            WorkspaceDomainKind::Rule,
            "Validate Rule Asset",
            "Validate a rule asset before build.",
            true,
            true,
            true,
            Some("validate_rule_asset"),
            Some("ValidateRuleAsset"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Use rule validation productization.",
        ),
        op(
            "build_rule_artifact",
            WorkspaceDomainKind::Rule,
            "Build Rule Artifact",
            "Build a runtime rule artifact.",
            true,
            true,
            true,
            Some("build_rule_artifact"),
            Some("BuildRuleArtifact"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Use rule artifact build pipeline.",
        ),
        op(
            "register_rule_artifact",
            WorkspaceDomainKind::Rule,
            "Register Rule Artifact",
            "Register a rule artifact for runtime loading.",
            true,
            true,
            true,
            Some("register_rule_artifact"),
            Some("BuildRuleArtifact"),
            &[ManualWalkthroughRequiredContext::SelectedRule],
            "Use rule artifact lifecycle productization.",
        ),
        op(
            "inspect_rule_diagnostics",
            WorkspaceDomainKind::Rule,
            "Inspect Rule Diagnostics",
            "Inspect rule validation and build diagnostics.",
            false,
            true,
            true,
            Some("inspect_rule_diagnostics"),
            Some("OpenRuleDiagnostics"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Focus rule diagnostics.",
        ),
        op(
            "create_default_input_mapping",
            WorkspaceDomainKind::Input,
            "Create Default Input Mapping",
            "Create the default input mapping asset.",
            true,
            true,
            true,
            Some("create_default_input_mapping"),
            Some("CreateDefaultInputMapping"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use input mapping authoring service.",
        ),
        op(
            "select_input_mapping",
            WorkspaceDomainKind::Input,
            "Select Input Mapping",
            "Select an input mapping asset.",
            true,
            true,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use input mapping panel selection.",
        ),
        op(
            "add_input_action",
            WorkspaceDomainKind::Input,
            "Add Input Action",
            "Add an input action.",
            true,
            true,
            true,
            Some("add_input_action"),
            Some("AddInputAction"),
            &[ManualWalkthroughRequiredContext::SelectedInputMapping],
            "Use input mapping panel.",
        ),
        op(
            "add_input_binding",
            WorkspaceDomainKind::Input,
            "Add Input Binding",
            "Add a binding for an input action.",
            true,
            true,
            true,
            Some("add_input_binding"),
            Some("AddInputBinding"),
            &[ManualWalkthroughRequiredContext::SelectedInputMapping],
            "Use input mapping panel.",
        ),
        op(
            "set_input_binding_device_path",
            WorkspaceDomainKind::Input,
            "Set Input Binding Device Path",
            "Set the device path for a binding.",
            true,
            true,
            true,
            Some("set_input_binding_device_path"),
            Some("SetInputBindingDevicePath"),
            &[ManualWalkthroughRequiredContext::SelectedInputMapping],
            "Use input mapping panel.",
        ),
        op(
            "validate_input_mapping",
            WorkspaceDomainKind::Input,
            "Validate Input Mapping",
            "Validate an input mapping asset.",
            true,
            true,
            true,
            Some("validate_input_mapping"),
            Some("ValidateInputMapping"),
            &[ManualWalkthroughRequiredContext::SelectedInputMapping],
            "Use input mapping validation.",
        ),
        op(
            "save_input_mapping",
            WorkspaceDomainKind::Input,
            "Save Input Mapping",
            "Save input mapping edits.",
            true,
            true,
            true,
            Some("save_input_mapping"),
            Some("SaveInputMapping"),
            &[ManualWalkthroughRequiredContext::SelectedInputMapping],
            "Use input mapping save.",
        ),
        op(
            "create_aui_document",
            WorkspaceDomainKind::Aui,
            "Create AUI Document",
            "Create a runtime UI document.",
            false,
            true,
            true,
            Some("create_aui_document"),
            Some("CreateAuiDocument"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use AUI authoring productization.",
        ),
        op(
            "open_aui_document",
            WorkspaceDomainKind::Aui,
            "Open AUI Document",
            "Open a runtime UI document.",
            false,
            true,
            true,
            Some("open_aui_document"),
            Some("OpenAuiDocument"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Open the selected AUI document through AUI authoring productization.",
        ),
        op(
            "add_aui_node",
            WorkspaceDomainKind::Aui,
            "Add AUI Node",
            "Add a node to the UI document.",
            false,
            true,
            true,
            Some("add_aui_node"),
            Some("AddAuiNode"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Use AUI authoring productization.",
        ),
        op(
            "edit_aui_node_field",
            WorkspaceDomainKind::Aui,
            "Edit AUI Node Field",
            "Edit a UI node field.",
            false,
            true,
            true,
            Some("set_aui_node_field"),
            Some("SetAuiNodeField"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Use AUI authoring productization.",
        ),
        op(
            "edit_aui_binding_path",
            WorkspaceDomainKind::Aui,
            "Edit AUI Binding Path",
            "Edit a UI binding path.",
            false,
            true,
            true,
            Some("set_aui_binding_path"),
            Some("SetAuiBindingPath"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Use AUI binding authoring.",
        ),
        op(
            "edit_aui_action_ref",
            WorkspaceDomainKind::Aui,
            "Edit AUI Action Ref",
            "Edit a UI action reference.",
            false,
            true,
            true,
            Some("set_aui_action_ref"),
            Some("SetAuiActionRef"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Use AUI action authoring.",
        ),
        op(
            "validate_aui_document",
            WorkspaceDomainKind::Aui,
            "Validate AUI Document",
            "Validate a UI document before packaging.",
            false,
            true,
            true,
            Some("validate_aui_document"),
            Some("ValidateAuiDocument"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Use AUI validation.",
        ),
        op(
            "save_aui_document",
            WorkspaceDomainKind::Aui,
            "Save AUI Document",
            "Save UI document edits.",
            false,
            true,
            true,
            Some("save_aui_document"),
            Some("SaveAuiDocument"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Use AUI document save.",
        ),
        op(
            "preview_aui_overlay",
            WorkspaceDomainKind::Aui,
            "Preview AUI Overlay",
            "Preview the UI overlay in the authoring context.",
            false,
            true,
            true,
            Some("preview_aui_overlay"),
            Some("PreviewAuiOverlay"),
            &[ManualWalkthroughRequiredContext::SelectedAuiDocument],
            "Use AUI preview report.",
        ),
        op(
            "open_runtime_package",
            WorkspaceDomainKind::Play,
            "Open Runtime Package",
            "Open a built runtime package.",
            true,
            false,
            true,
            Some("open_runtime_package"),
            Some("OpenRuntimePackage"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use package open command.",
        ),
        op(
            "reload_runtime_package",
            WorkspaceDomainKind::Play,
            "Reload Runtime Package",
            "Reload the active runtime package.",
            true,
            false,
            true,
            Some("reload_runtime_package"),
            Some("ReloadRuntimePackage"),
            &[ManualWalkthroughRequiredContext::RuntimePackage],
            "Use package reload command.",
        ),
        op(
            "play",
            WorkspaceDomainKind::Play,
            "Play",
            "Start play mode.",
            true,
            false,
            true,
            Some("play"),
            Some("Play"),
            &[ManualWalkthroughRequiredContext::RuntimePackage],
            "Build or open a runtime package first.",
        ),
        op(
            "pause",
            WorkspaceDomainKind::Play,
            "Pause",
            "Pause play mode.",
            false,
            false,
            true,
            Some("pause"),
            Some("Pause"),
            &[ManualWalkthroughRequiredContext::RuntimePackage],
            "Use play controls.",
        ),
        op(
            "step_frame",
            WorkspaceDomainKind::Play,
            "Step Frame",
            "Advance one frame.",
            false,
            false,
            true,
            Some("step_frame"),
            Some("StepFrame"),
            &[ManualWalkthroughRequiredContext::RuntimePackage],
            "Use play controls.",
        ),
        op(
            "reset_runtime",
            WorkspaceDomainKind::Play,
            "Reset Runtime",
            "Reset runtime state.",
            false,
            false,
            true,
            Some("reset_runtime"),
            Some("ResetRuntime"),
            &[ManualWalkthroughRequiredContext::RuntimePackage],
            "Use play controls.",
        ),
        op(
            "export_desktop_package",
            WorkspaceDomainKind::Build,
            "Export Desktop Package",
            "Export a desktop runtime package.",
            false,
            true,
            true,
            Some("export_desktop_package"),
            Some("ExportDesktopPackage"),
            &[ManualWalkthroughRequiredContext::BuildProfile],
            "Use build export panel.",
        ),
        op(
            "build_and_run_desktop_package",
            WorkspaceDomainKind::Build,
            "Build And Run Desktop Package",
            "Export a desktop runtime package and launch the staged Game executable.",
            false,
            true,
            true,
            Some("build_and_run_desktop_package"),
            Some("BuildAndRunDesktopPackage"),
            &[ManualWalkthroughRequiredContext::BuildProfile],
            "Use Build & Run in the build export panel.",
        ),
        op(
            "open_build_output",
            WorkspaceDomainKind::Build,
            "Open Build Output",
            "Open the build output directory.",
            false,
            true,
            true,
            Some("open_build_output"),
            Some("OpenBuildOutput"),
            &[ManualWalkthroughRequiredContext::BuildProfile],
            "Use build output command.",
        ),
        op(
            "open_build_report",
            WorkspaceDomainKind::Build,
            "Open Build Report",
            "Open the build report.",
            false,
            true,
            true,
            Some("open_build_report"),
            Some("OpenBuildReport"),
            &[ManualWalkthroughRequiredContext::BuildProfile],
            "Use build report command.",
        ),
        op(
            "open_runtime_report",
            WorkspaceDomainKind::Report,
            "Open Runtime Report",
            "Open runtime report evidence.",
            false,
            false,
            true,
            Some("open_raw_report"),
            Some("OpenRawReport"),
            &[ManualWalkthroughRequiredContext::RuntimePackage],
            "Use report panel productization.",
        ),
        op(
            "open_authoring_walkthrough_report",
            WorkspaceDomainKind::Report,
            "Open Authoring Walkthrough Report",
            "Open the manual walkthrough coverage report.",
            false,
            false,
            true,
            Some("select_report_entry"),
            Some("SelectReportEntry"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use report panel productization.",
        ),
        op(
            "import_project_patch",
            WorkspaceDomainKind::Report,
            "Import Project Patch",
            "Import a structured ProjectPatch document from JSON or file.",
            false,
            false,
            true,
            Some("import_project_patch"),
            Some("ImportProjectPatch"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Provide ProjectPatch JSON or a ProjectPatch file path.",
        ),
        op(
            "generate_project_patch_from_prompt",
            WorkspaceDomainKind::Report,
            "Generate Project Patch From Prompt",
            "Generate ProjectPatch JSON through the thin LLM patch source and stage it for review.",
            false,
            false,
            true,
            Some("generate_project_patch_from_prompt"),
            Some("GenerateProjectPatchFromPrompt"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Provide a prompt and review the generated imported ProjectPatch proposal.",
        ),
        op(
            "preview_imported_project_patch",
            WorkspaceDomainKind::Report,
            "Preview Imported Project Patch",
            "Parse, validate, and review an imported ProjectPatch without applying it.",
            false,
            false,
            true,
            Some("preview_imported_project_patch"),
            Some("PreviewImportedProjectPatch"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Provide ProjectPatch JSON or a ProjectPatch file path.",
        ),
        op(
            "apply_imported_project_patch",
            WorkspaceDomainKind::Report,
            "Apply Imported Project Patch",
            "Confirm and apply a previously imported ProjectPatch proposal.",
            false,
            false,
            true,
            Some("apply_imported_project_patch"),
            Some("ApplyImportedProjectPatch"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Select an imported ProjectPatch proposal first.",
        ),
        op(
            "inspect_project_patch_report",
            WorkspaceDomainKind::Report,
            "Inspect Project Patch Report",
            "Inspect ProjectPatch import, validation, review, apply, and history evidence.",
            false,
            false,
            true,
            Some("select_report_entry"),
            Some("SelectReportEntry"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use imported ProjectPatch productization reports.",
        ),
        op(
            "ai_project_patch_scene_input",
            WorkspaceDomainKind::Report,
            "AI Project Patch Scene/Input",
            "Review and apply a structured ProjectPatch for Scene/Input edits.",
            false,
            false,
            true,
            Some("ai_accept_project_patch"),
            Some("ProjectPatchDocument"),
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use ProjectPatch All-Domain Capability v2.",
        ),
        op(
            "ai_project_patch_asset_prefab_aui_rule",
            WorkspaceDomainKind::Report,
            "AI Project Patch All Domains",
            "Review and apply structured Asset/Prefab/AUI/Rule/Build ProjectPatch capabilities.",
            false,
            false,
            true,
            None,
            None,
            &[ManualWalkthroughRequiredContext::OpenProject],
            "Use ProjectPatch All-Domain A-min operation schemas.",
        ),
        op(
            "clear_console",
            WorkspaceDomainKind::Report,
            "Clear Console",
            "Clear console entries.",
            false,
            false,
            false,
            Some("clear_console"),
            Some("ClearConsole"),
            &[ManualWalkthroughRequiredContext::None],
            "Use console clear command.",
        ),
        op(
            "select_trace_entry",
            WorkspaceDomainKind::Report,
            "Select Trace Entry",
            "Select a trace or report entry for inspection.",
            false,
            false,
            true,
            Some("select_trace_entry"),
            Some("SelectTraceEntry"),
            &[ManualWalkthroughRequiredContext::TraceEntry],
            "Use report panel selection.",
        ),
    ]
}

fn op(
    operation_id: &str,
    domain: WorkspaceDomainKind,
    title: &str,
    user_goal: &str,
    required_for_play: bool,
    required_for_build: bool,
    required_for_complex_project: bool,
    expected_command_id: Option<&str>,
    expected_payload_kind: Option<&str>,
    required_context: &[ManualWalkthroughRequiredContext],
    fallback_behavior: &str,
) -> ManualAuthoringOperationRequirement {
    ManualAuthoringOperationRequirement {
        operation_id: operation_id.to_string(),
        domain,
        title: title.to_string(),
        user_goal: user_goal.to_string(),
        required_for_play,
        required_for_build,
        required_for_complex_project,
        expected_command_id: expected_command_id.map(str::to_string),
        expected_payload_kind: expected_payload_kind.map(str::to_string),
        required_context: required_context.to_vec(),
        fallback_behavior: fallback_behavior.to_string(),
    }
}

fn count_status(
    operations: &[ManualWalkthroughOperationCoverage],
    status: ManualAuthoringOperationStatus,
) -> usize {
    operations
        .iter()
        .filter(|operation| operation.status == status)
        .count()
}

fn summarize_domain(
    domain: WorkspaceDomainKind,
    operations: &[ManualWalkthroughOperationCoverage],
) -> Option<ManualWalkthroughDomainSummary> {
    let domain_operations = operations
        .iter()
        .filter(|operation| operation.requirement.domain == domain)
        .collect::<Vec<_>>();
    if domain_operations.is_empty() {
        return None;
    }
    let operation_count = domain_operations.len();
    let executable_count = domain_operations
        .iter()
        .filter(|operation| operation.status == ManualAuthoringOperationStatus::ExecutableCommand)
        .count();
    let needs_context_count = domain_operations
        .iter()
        .filter(|operation| {
            operation.status == ManualAuthoringOperationStatus::ExecutableCommandNeedsContext
        })
        .count();
    let focus_panel_count = domain_operations
        .iter()
        .filter(|operation| operation.status == ManualAuthoringOperationStatus::FocusDomainPanel)
        .count();
    let missing_count = domain_operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.status,
                ManualAuthoringOperationStatus::MissingCommand
                    | ManualAuthoringOperationStatus::MissingDomainService
            )
        })
        .count();
    let blocked_count = domain_operations
        .iter()
        .filter(|operation| operation.status == ManualAuthoringOperationStatus::BlockedByDependency)
        .count();
    let status = if missing_count + blocked_count + needs_context_count > 0 {
        ManualWalkthroughCoverageStatus::Partial
    } else {
        ManualWalkthroughCoverageStatus::Pass
    };

    Some(ManualWalkthroughDomainSummary {
        domain,
        operation_count,
        executable_count,
        needs_context_count,
        focus_panel_count,
        missing_count,
        blocked_count,
        status,
    })
}

fn operation_gap(operation: &ManualWalkthroughOperationCoverage) -> Option<MissingOperationGap> {
    let severity = match operation.status {
        ManualAuthoringOperationStatus::ExecutableCommand
        | ManualAuthoringOperationStatus::FocusDomainPanel
        | ManualAuthoringOperationStatus::Deferred => return None,
        ManualAuthoringOperationStatus::ExecutableCommandNeedsContext => {
            MissingOperationSeverity::Info
        }
        ManualAuthoringOperationStatus::MissingCommand => MissingOperationSeverity::Warning,
        ManualAuthoringOperationStatus::MissingDomainService => MissingOperationSeverity::Error,
        ManualAuthoringOperationStatus::BlockedByDependency => MissingOperationSeverity::Critical,
    };
    let operation_id = operation.requirement.operation_id.clone();
    let suggested_next_action = operation
        .next_action
        .clone()
        .unwrap_or_else(|| format!("complete_{}", operation_id));
    Some(MissingOperationGap {
        gap_id: operation
            .gap_id
            .clone()
            .unwrap_or_else(|| format!("gap.{}", operation_id)),
        domain: operation.requirement.domain,
        operation_id,
        severity,
        reason: operation.resolution_summary.clone(),
        suggested_system: suggested_system_for_domain(operation.requirement.domain).to_string(),
        suggested_next_action,
        blocks_manual_walkthrough: operation.requirement.required_for_complex_project,
        blocks_play: operation.requirement.required_for_play,
        blocks_build: operation.requirement.required_for_build,
    })
}

fn suggested_system_for_domain(domain: WorkspaceDomainKind) -> &'static str {
    match domain {
        WorkspaceDomainKind::Project => "project_authoring_productization",
        WorkspaceDomainKind::Scene => "scene_authoring_productization",
        WorkspaceDomainKind::Asset => "asset_import_productization",
        WorkspaceDomainKind::Prefab => "prefab_authoring_productization",
        WorkspaceDomainKind::Rule => "rule_authoring_productization",
        WorkspaceDomainKind::Aui => "aui_authoring_productization",
        WorkspaceDomainKind::Input => "input_mapping_authoring_productization",
        WorkspaceDomainKind::Play => "play_runtime_package_productization",
        WorkspaceDomainKind::Build => "build_export_productization",
        WorkspaceDomainKind::Report => "unified_report_panel_productization",
    }
}

fn next_actions_from_gaps(gaps: &[MissingOperationGap]) -> Vec<String> {
    let mut actions = gaps
        .iter()
        .map(|gap| gap.suggested_next_action.clone())
        .collect::<Vec<_>>();
    actions.sort();
    actions.dedup();
    actions
}
