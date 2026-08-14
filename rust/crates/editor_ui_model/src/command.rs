use serde::{Deserialize, Serialize};

use super::{
    AssetPlacementMode, AuthoringStepId, EditorGameViewScalePolicy, InputActionValueKind,
    InputMappingReportLevel, InputProcessorKind, InputTriggerKind, Vec3, WorkspaceViewMode,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiCommand {
    pub command_id: String,
    pub source: UiCommandSource,
    pub request_id: String,
    pub payload: UiCommandPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrefabStageMode {
    Isolated,
    InContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrefabStageSavePolicy {
    Save,
    Discard,
    KeepOpen,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiCommandSource {
    ProjectLauncher,
    ProjectBrowser,
    Toolbar,
    Hierarchy,
    Inspector,
    Viewport,
    Console,
    RuntimeTrace,
    AiAssistant,
    Test,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiCommandPayload {
    OpenProject {
        path: String,
    },
    CreateProject {
        path: String,
        name: String,
    },
    StartCreateProjectWithAi {
        draft_path: Option<String>,
    },
    SelectRecentProject {
        path: String,
    },
    RefreshRecentProjects,
    SelectProjectBrowserEntry {
        path: String,
    },
    OpenProjectBrowserEntry {
        path: String,
    },
    SelectAssetBrowserEntry {
        entry_key: super::AssetEntryKey,
        additive: bool,
        range: bool,
    },
    OpenAssetBrowserEntry {
        entry_key: super::AssetEntryKey,
    },
    SetAssetBrowserFolder {
        folder: Option<String>,
    },
    SetAssetBrowserSearch {
        search_text: String,
    },
    SetAssetBrowserKindFilter {
        kinds: Vec<super::AssetKind>,
    },
    AssetBrowserToolbar {
        action: super::AssetBrowserToolbarAction,
    },
    ScrollAssetBrowser {
        delta: f32,
    },
    BeginAssetPick {
        field_id: String,
    },
    ConfirmAssetPick,
    CancelAssetPick,
    DropAssetOnInspectorField {
        entry_key: super::AssetEntryKey,
        field_id: String,
    },
    CreateDefaultInputMapping {
        path: String,
    },
    DeleteInputMapping {
        path: String,
    },
    OpenInputMapping {
        path: String,
    },
    SaveInputMapping {
        path: String,
    },
    DiscardInputMappingDraft {
        path: String,
    },
    ValidateInputMapping {
        path: String,
    },
    SelectInputContext {
        path: String,
        context_id: String,
    },
    SelectInputAction {
        path: String,
        action_id: String,
    },
    SelectInputBinding {
        path: String,
        binding_id: String,
    },
    AddInputContext {
        path: String,
        context_id: String,
        priority: i32,
    },
    RemoveInputContext {
        path: String,
        context_id: String,
    },
    SetInputContextPriority {
        path: String,
        context_id: String,
        priority: i32,
    },
    SetInputContextConsumeInput {
        path: String,
        context_id: String,
        consume_input: bool,
    },
    AddInputAction {
        path: String,
        action_id: String,
        value_type: InputActionValueKind,
    },
    RemoveInputAction {
        path: String,
        action_id: String,
    },
    SetInputActionValueType {
        path: String,
        action_id: String,
        value_type: InputActionValueKind,
    },
    AddInputBinding {
        path: String,
        context_id: String,
        action_id: String,
        device_path: String,
    },
    RemoveInputBinding {
        path: String,
        binding_index: usize,
    },
    SetInputBindingDevicePath {
        path: String,
        binding_index: usize,
        device_path: String,
    },
    SetInputBindingProcessorByIndex {
        path: String,
        binding_index: usize,
        processor: InputProcessorKind,
    },
    RemoveInputBindingById {
        path: String,
        binding_id: String,
    },
    SetInputBindingDevicePathById {
        path: String,
        binding_id: String,
        device_path: String,
    },
    SetInputBindingTrigger {
        path: String,
        binding_id: String,
        trigger: InputTriggerKind,
    },
    SetInputBindingProcessor {
        path: String,
        binding_id: String,
        processor: InputProcessorKind,
    },
    BeginInputBindingCapture {
        path: String,
        binding_id: String,
    },
    CancelInputBindingCapture {
        path: String,
    },
    CommitCapturedInputBinding {
        path: String,
        binding_id: String,
        device_path: String,
    },
    PreviewInputMapping {
        path: String,
        device_path: Option<String>,
    },
    SetInputMappingReportLevel {
        path: String,
        level: InputMappingReportLevel,
    },
    RegisterExistingAsset {
        path: String,
        expected_kind: Option<super::AssetKind>,
    },
    GenerateMockImageAsset {
        prompt: String,
        target_folder: String,
        asset_name: String,
        image_kind: String,
        width: u32,
        height: u32,
        transparent_background: bool,
    },
    ValidateAssetBrowserIndex {
        query_kind: Option<super::AssetKind>,
    },
    CreateRuleAsset {
        path: String,
        rule_id: String,
        display_name: String,
        #[serde(default)]
        phase: Option<String>,
    },
    OpenRuleAsset {
        path: String,
    },
    SelectRuleAsset {
        path: String,
    },
    SetRuleTrigger {
        path: String,
        trigger: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    AddRuleStatement {
        path: String,
        statement: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    UpdateRuleStatement {
        path: String,
        statement_index: usize,
        statement: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    RemoveRuleStatement {
        path: String,
        statement_index: usize,
        expected_ir_hash: Option<String>,
    },
    AddRuleOperation {
        path: String,
        operation: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    UpdateRuleOperation {
        path: String,
        operation_index: usize,
        operation: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    RemoveRuleOperation {
        path: String,
        operation_index: usize,
        expected_ir_hash: Option<String>,
    },
    ValidateRuleAsset {
        path: String,
    },
    BuildRuleArtifact {
        path: String,
    },
    BuildProjectRuleManifest {
        path: String,
    },
    SaveRuleAsset {
        path: String,
    },
    OpenRuleDiagnostics {
        path: String,
    },
    SelectRuleCard {
        path: String,
        card_id: String,
    },
    SetRuleCardField {
        path: String,
        card_id: String,
        field_path: String,
        value: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    AddRuleCard {
        path: String,
        card_kind: String,
        value: serde_json::Value,
        expected_ir_hash: Option<String>,
    },
    RemoveRuleCard {
        path: String,
        card_id: String,
        expected_ir_hash: Option<String>,
    },
    SelectRuleGraphNode {
        path: String,
        node_id: String,
    },
    RefreshRuleGraphPreview {
        path: String,
    },
    CreatePrefabFromSelection {
        scene_path: Option<String>,
        root_entity_id: String,
        prefab_id: String,
        name: String,
        replace_selection_with_instance: bool,
    },
    OpenPrefabDocument {
        path: String,
    },
    EnterPrefabStage {
        path: String,
        mode: PrefabStageMode,
        opened_from_instance_entity_id: Option<String>,
    },
    ExitPrefabStage {
        save_policy: PrefabStageSavePolicy,
    },
    InstantiatePrefabInScene {
        prefab_id: String,
        parent_entity_id: Option<String>,
        local_position: Option<Vec3>,
    },
    SetPrefabStageEntityField {
        source_entity_id: String,
        component_type: Option<String>,
        field_path: String,
        value: serde_json::Value,
    },
    ApplyPrefabOverrideToAsset {
        instance_entity_id: String,
        target_source_entity_id: String,
        component_type: String,
        field_path: String,
    },
    SavePrefabDocument {
        path: String,
    },
    ValidatePrefabReferences {
        path: Option<String>,
    },
    RevertPrefabOverride {
        instance_entity_id: String,
        target_source_entity_id: String,
        component_type: String,
        field_path: String,
    },
    CreateAuiDocument {
        path: String,
        document_id: String,
        width: f32,
        height: f32,
    },
    OpenAuiDocument {
        path: String,
    },
    SelectAuiNode {
        document_path: String,
        document_id: String,
        node_id: String,
    },
    AddAuiNode {
        path: String,
        parent_node_id: String,
        node_id: String,
        kind: String,
        name: String,
        rect: serde_json::Value,
    },
    SetAuiNodeField {
        path: String,
        node_id: String,
        schema_path: String,
        value: serde_json::Value,
    },
    SetAuiBindingPath {
        path: String,
        node_id: String,
        target_field: String,
        binding_id: String,
        binding_path: String,
        fallback: Option<serde_json::Value>,
    },
    SetAuiActionRef {
        path: String,
        node_id: String,
        event: String,
        action_id: String,
        payload: Option<serde_json::Value>,
    },
    ValidateAuiDocument {
        path: String,
    },
    SaveAuiDocument {
        path: String,
    },
    PreviewAuiOverlay {
        path: String,
    },
    SaveAuiSubtreeAsTemplate {
        document_path: String,
        root_node_id: String,
        template_asset_path: String,
        template_id: String,
        display_name: String,
    },
    InstantiateAuiTemplate {
        template_asset_path: String,
        template_id: String,
        target_document_path: String,
        parent_node_id: String,
        insertion_index: Option<usize>,
        instance_id: String,
        node_id_prefix: String,
    },
    ValidateAuiTemplate {
        template_asset_path: String,
        template_id: String,
    },
    SetWorkspaceViewMode {
        mode: WorkspaceViewMode,
    },
    SetAuthoringWorkflowStep {
        step_id: AuthoringStepId,
    },
    OpenRuntimePackage {
        path: String,
    },
    OpenSceneDocument {
        path: String,
    },
    ReloadRuntimePackage,
    SelectEntity {
        entity_id: String,
    },
    SelectRuntimeEntity {
        entity_id: String,
    },
    PickRuntimeEntityAt {
        x: f32,
        y: f32,
        viewport_width: Option<f32>,
        viewport_height: Option<f32>,
        aui_consumed: bool,
    },
    SelectSceneEntity {
        entity_id: String,
    },
    CreateSceneEntity {
        parent_id: Option<String>,
        name: String,
    },
    PlaceAssetIntoScene {
        asset_id: String,
        asset_type: String,
        asset_guid: Option<String>,
        target_parent_id: Option<String>,
        local_position: Option<Vec3>,
        placement_mode: AssetPlacementMode,
    },
    DeleteSceneEntity {
        entity_id: String,
    },
    RenameSceneEntity {
        entity_id: String,
        name: String,
    },
    SetSceneTransform {
        entity_id: String,
        local_position: Option<Vec3>,
        local_rotation: Option<Vec3>,
        local_scale: Option<Vec3>,
    },
    AddSceneComponent {
        entity_id: String,
        component_type: String,
        fields: serde_json::Value,
    },
    RemoveSceneComponent {
        entity_id: String,
        component_type: String,
    },
    SetSceneComponentField {
        entity_id: String,
        component_type: String,
        field_path: String,
        value: serde_json::Value,
    },
    SetRuntimeComponentFieldTemporary {
        entity_id: String,
        component_type: String,
        field_path: String,
        value: serde_json::Value,
    },
    PreviewApplyRuntimeChangeToAuthoring,
    ApplyRuntimeChangeToAuthoring {
        edit_id: String,
        candidate_hash: String,
    },
    SaveSceneDocument {
        path: Option<String>,
    },
    UndoSceneEdit,
    RedoSceneEdit,
    TickOneFrame,
    Play,
    Pause,
    StepFrame,
    StopPlaySession,
    SetGameViewTarget {
        width: u32,
        height: u32,
        scale_policy: EditorGameViewScalePolicy,
    },
    SetGameViewMaximizeOnPlay {
        enabled: bool,
    },
    ToggleGameViewMaximizeOnPlay,
    ResetRuntime,
    ExportDesktopPackage {
        profile_id: Option<String>,
    },
    BuildAndRunDesktopPackage {
        profile_id: Option<String>,
    },
    BuildReleasePackage {
        profile_id: Option<String>,
    },
    SaveReleaseProfile,
    SetReleaseProfileIcon {
        asset_ref: super::EditorAssetRef,
    },
    OpenBuildOutput,
    OpenBuildReport,
    ClearConsole,
    SelectReportEntry {
        report_id: String,
    },
    RefreshReports,
    CopyReportAiContext {
        report_id: String,
    },
    OpenRawReport {
        report_id: String,
    },
    RevealReportPath {
        report_id: String,
    },
    OpenRelatedReportArtifact {
        report_id: String,
        artifact_id: String,
    },
    SelectTraceEntry {
        entry_id: String,
    },
    AiSubmitPrompt {
        prompt: String,
    },
    GenerateProjectPatchFromPrompt {
        prompt: String,
    },
    SetAiPromptDraft {
        prompt: String,
    },
    CancelLlmPatchRequest,
    ImportProjectPatch {
        source_label: String,
        raw_json: Option<String>,
        file_path: Option<String>,
        expected_patch_id: Option<String>,
        dry_run: bool,
    },
    PreviewImportedProjectPatch {
        source_label: String,
        raw_json: Option<String>,
        file_path: Option<String>,
        expected_patch_id: Option<String>,
    },
    ApplyImportedProjectPatch {
        proposal_id: String,
    },
    ParkProjectWorkItem {
        work_item_id: String,
    },
    ResumeProjectWorkItem {
        work_item_id: String,
    },
    ReopenProjectWorkItem {
        work_item_id: String,
    },
    ApproveProjectChange {
        proposal_digest: String,
    },
    AdvanceProjectProduction {
        run_id: String,
    },
    CancelProjectProduction {
        run_id: String,
    },
    RecoverProjectProduction {
        run_id: String,
    },
    ApproveGatewayAccessRequest {
        request_id: String,
    },
    RejectGatewayAccessRequest {
        request_id: String,
    },
    SetGatewayAccessPage {
        page_index: usize,
    },
    ApproveProjectRuntimeTrust {
        request_id: String,
    },
    DenyProjectRuntimeTrust {
        request_id: String,
    },
    CancelProjectRuntimeTrust {
        request_id: String,
    },
    AiAcceptProposedCommand {
        proposal_id: String,
    },
    AiRejectProposedCommand {
        proposal_id: String,
    },
}

pub fn ui_command_id_for_payload(payload: &UiCommandPayload) -> &'static str {
    match payload {
        UiCommandPayload::OpenProject { .. } => "open_project",
        UiCommandPayload::CreateProject { .. } => "create_project",
        UiCommandPayload::StartCreateProjectWithAi { .. } => "start_create_project_with_ai",
        UiCommandPayload::SelectRecentProject { .. } => "select_recent_project",
        UiCommandPayload::RefreshRecentProjects => "refresh_recent_projects",
        UiCommandPayload::SelectProjectBrowserEntry { .. } => "select_project_browser_entry",
        UiCommandPayload::OpenProjectBrowserEntry { .. } => "open_project_browser_entry",
        UiCommandPayload::SelectAssetBrowserEntry { .. } => "select_asset_browser_entry",
        UiCommandPayload::OpenAssetBrowserEntry { .. } => "open_asset_browser_entry",
        UiCommandPayload::SetAssetBrowserFolder { .. } => "set_asset_browser_folder",
        UiCommandPayload::SetAssetBrowserSearch { .. } => "set_asset_browser_search",
        UiCommandPayload::SetAssetBrowserKindFilter { .. } => "set_asset_browser_kind_filter",
        UiCommandPayload::AssetBrowserToolbar { .. } => "asset_browser_toolbar",
        UiCommandPayload::ScrollAssetBrowser { .. } => "scroll_asset_browser",
        UiCommandPayload::BeginAssetPick { .. } => "begin_asset_pick",
        UiCommandPayload::ConfirmAssetPick => "confirm_asset_pick",
        UiCommandPayload::CancelAssetPick => "cancel_asset_pick",
        UiCommandPayload::DropAssetOnInspectorField { .. } => "drop_asset_on_inspector_field",
        UiCommandPayload::CreateDefaultInputMapping { .. } => "create_default_input_mapping",
        UiCommandPayload::DeleteInputMapping { .. } => "delete_input_mapping",
        UiCommandPayload::OpenInputMapping { .. } => "open_input_mapping",
        UiCommandPayload::SaveInputMapping { .. } => "save_input_mapping",
        UiCommandPayload::DiscardInputMappingDraft { .. } => "discard_input_mapping_draft",
        UiCommandPayload::ValidateInputMapping { .. } => "validate_input_mapping",
        UiCommandPayload::SelectInputContext { .. } => "select_input_context",
        UiCommandPayload::SelectInputAction { .. } => "select_input_action",
        UiCommandPayload::SelectInputBinding { .. } => "select_input_binding",
        UiCommandPayload::AddInputContext { .. } => "add_input_context",
        UiCommandPayload::RemoveInputContext { .. } => "remove_input_context",
        UiCommandPayload::SetInputContextPriority { .. } => "set_input_context_priority",
        UiCommandPayload::SetInputContextConsumeInput { .. } => "set_input_context_consume_input",
        UiCommandPayload::AddInputAction { .. } => "add_input_action",
        UiCommandPayload::RemoveInputAction { .. } => "remove_input_action",
        UiCommandPayload::SetInputActionValueType { .. } => "set_input_action_value_type",
        UiCommandPayload::AddInputBinding { .. } => "add_input_binding",
        UiCommandPayload::RemoveInputBinding { .. } => "remove_input_binding",
        UiCommandPayload::SetInputBindingDevicePath { .. } => "set_input_binding_device_path",
        UiCommandPayload::SetInputBindingProcessorByIndex { .. } => {
            "set_input_binding_processor_by_index"
        }
        UiCommandPayload::RemoveInputBindingById { .. } => "remove_input_binding_by_id",
        UiCommandPayload::SetInputBindingDevicePathById { .. } => {
            "set_input_binding_device_path_by_id"
        }
        UiCommandPayload::SetInputBindingTrigger { .. } => "set_input_binding_trigger",
        UiCommandPayload::SetInputBindingProcessor { .. } => "set_input_binding_processor",
        UiCommandPayload::BeginInputBindingCapture { .. } => "begin_input_binding_capture",
        UiCommandPayload::CancelInputBindingCapture { .. } => "cancel_input_binding_capture",
        UiCommandPayload::CommitCapturedInputBinding { .. } => "commit_captured_input_binding",
        UiCommandPayload::PreviewInputMapping { .. } => "preview_input_mapping",
        UiCommandPayload::SetInputMappingReportLevel { .. } => "set_input_mapping_report_level",
        UiCommandPayload::RegisterExistingAsset { .. } => "register_existing_asset",
        UiCommandPayload::GenerateMockImageAsset { .. } => "generate_mock_image_asset",
        UiCommandPayload::ValidateAssetBrowserIndex { .. } => "validate_asset_browser_index",
        UiCommandPayload::CreateRuleAsset { .. } => "create_rule_asset",
        UiCommandPayload::OpenRuleAsset { .. } => "open_rule_asset",
        UiCommandPayload::SelectRuleAsset { .. } => "select_rule_asset",
        UiCommandPayload::SetRuleTrigger { .. } => "set_rule_trigger",
        UiCommandPayload::AddRuleStatement { .. } => "add_rule_statement",
        UiCommandPayload::UpdateRuleStatement { .. } => "update_rule_statement",
        UiCommandPayload::RemoveRuleStatement { .. } => "remove_rule_statement",
        UiCommandPayload::AddRuleOperation { .. } => "add_rule_operation",
        UiCommandPayload::UpdateRuleOperation { .. } => "update_rule_operation",
        UiCommandPayload::RemoveRuleOperation { .. } => "remove_rule_operation",
        UiCommandPayload::ValidateRuleAsset { .. } => "validate_rule_asset",
        UiCommandPayload::BuildRuleArtifact { .. } => "build_rule_artifact",
        UiCommandPayload::BuildProjectRuleManifest { .. } => "build_project_rule_manifest",
        UiCommandPayload::SaveRuleAsset { .. } => "save_rule_asset",
        UiCommandPayload::OpenRuleDiagnostics { .. } => "open_rule_diagnostics",
        UiCommandPayload::SelectRuleCard { .. } => "select_rule_card",
        UiCommandPayload::SetRuleCardField { .. } => "set_rule_card_field",
        UiCommandPayload::AddRuleCard { .. } => "add_rule_card",
        UiCommandPayload::RemoveRuleCard { .. } => "remove_rule_card",
        UiCommandPayload::SelectRuleGraphNode { .. } => "select_rule_graph_node",
        UiCommandPayload::RefreshRuleGraphPreview { .. } => "refresh_rule_graph_preview",
        UiCommandPayload::CreatePrefabFromSelection { .. } => "create_prefab_from_selection",
        UiCommandPayload::OpenPrefabDocument { .. } => "open_prefab_document",
        UiCommandPayload::EnterPrefabStage { .. } => "enter_prefab_stage",
        UiCommandPayload::ExitPrefabStage { .. } => "exit_prefab_stage",
        UiCommandPayload::InstantiatePrefabInScene { .. } => "instantiate_prefab_in_scene",
        UiCommandPayload::SetPrefabStageEntityField { .. } => "set_prefab_stage_entity_field",
        UiCommandPayload::ApplyPrefabOverrideToAsset { .. } => "apply_prefab_override_to_asset",
        UiCommandPayload::SavePrefabDocument { .. } => "save_prefab_document",
        UiCommandPayload::ValidatePrefabReferences { .. } => "validate_prefab_references",
        UiCommandPayload::RevertPrefabOverride { .. } => "revert_prefab_override",
        UiCommandPayload::CreateAuiDocument { .. } => "create_aui_document",
        UiCommandPayload::OpenAuiDocument { .. } => "open_aui_document",
        UiCommandPayload::SelectAuiNode { .. } => "select_aui_node",
        UiCommandPayload::AddAuiNode { .. } => "add_aui_node",
        UiCommandPayload::SetAuiNodeField { .. } => "set_aui_node_field",
        UiCommandPayload::SetAuiBindingPath { .. } => "set_aui_binding_path",
        UiCommandPayload::SetAuiActionRef { .. } => "set_aui_action_ref",
        UiCommandPayload::ValidateAuiDocument { .. } => "validate_aui_document",
        UiCommandPayload::SaveAuiDocument { .. } => "save_aui_document",
        UiCommandPayload::PreviewAuiOverlay { .. } => "preview_aui_overlay",
        UiCommandPayload::SaveAuiSubtreeAsTemplate { .. } => "save_aui_subtree_as_template",
        UiCommandPayload::InstantiateAuiTemplate { .. } => "instantiate_aui_template",
        UiCommandPayload::ValidateAuiTemplate { .. } => "validate_aui_template",
        UiCommandPayload::SetWorkspaceViewMode { .. } => "set_workspace_view_mode",
        UiCommandPayload::SetAuthoringWorkflowStep { .. } => "set_authoring_workflow_step",
        UiCommandPayload::OpenRuntimePackage { .. } => "open_runtime_package",
        UiCommandPayload::OpenSceneDocument { .. } => "open_scene_document",
        UiCommandPayload::ReloadRuntimePackage => "reload_runtime_package",
        UiCommandPayload::SelectEntity { .. } => "select_entity",
        UiCommandPayload::SelectRuntimeEntity { .. } => "select_runtime_entity",
        UiCommandPayload::PickRuntimeEntityAt { .. } => "pick_runtime_entity_at",
        UiCommandPayload::SelectSceneEntity { .. } => "select_scene_entity",
        UiCommandPayload::CreateSceneEntity { .. } => "create_scene_entity",
        UiCommandPayload::PlaceAssetIntoScene { .. } => "place_asset_into_scene",
        UiCommandPayload::DeleteSceneEntity { .. } => "delete_scene_entity",
        UiCommandPayload::RenameSceneEntity { .. } => "rename_scene_entity",
        UiCommandPayload::SetSceneTransform { .. } => "set_scene_transform",
        UiCommandPayload::AddSceneComponent { .. } => "add_scene_component",
        UiCommandPayload::RemoveSceneComponent { .. } => "remove_scene_component",
        UiCommandPayload::SetSceneComponentField { .. } => "set_scene_component_field",
        UiCommandPayload::SetRuntimeComponentFieldTemporary { .. } => {
            "set_runtime_component_field_temporary"
        }
        UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring => {
            "preview_apply_runtime_change_to_authoring"
        }
        UiCommandPayload::ApplyRuntimeChangeToAuthoring { .. } => {
            "apply_runtime_change_to_authoring"
        }
        UiCommandPayload::SaveSceneDocument { .. } => "save_scene_document",
        UiCommandPayload::UndoSceneEdit => "undo_scene_edit",
        UiCommandPayload::RedoSceneEdit => "redo_scene_edit",
        UiCommandPayload::TickOneFrame => "tick_one_frame",
        UiCommandPayload::Play => "play",
        UiCommandPayload::Pause => "pause",
        UiCommandPayload::StepFrame => "step_frame",
        UiCommandPayload::StopPlaySession => "stop_play_session",
        UiCommandPayload::SetGameViewTarget { .. } => "set_game_view_target",
        UiCommandPayload::SetGameViewMaximizeOnPlay { .. } => "set_game_view_maximize_on_play",
        UiCommandPayload::ToggleGameViewMaximizeOnPlay => "toggle_game_view_maximize_on_play",
        UiCommandPayload::ResetRuntime => "reset_runtime",
        UiCommandPayload::ExportDesktopPackage { .. } => "export_desktop_package",
        UiCommandPayload::BuildAndRunDesktopPackage { .. } => "build_and_run_desktop_package",
        UiCommandPayload::BuildReleasePackage { .. } => "build_release_package",
        UiCommandPayload::SaveReleaseProfile => "save_release_profile",
        UiCommandPayload::SetReleaseProfileIcon { .. } => "set_release_profile_icon",
        UiCommandPayload::OpenBuildOutput => "open_build_output",
        UiCommandPayload::OpenBuildReport => "open_build_report",
        UiCommandPayload::ClearConsole => "clear_console",
        UiCommandPayload::SelectReportEntry { .. } => "select_report_entry",
        UiCommandPayload::RefreshReports => "refresh_reports",
        UiCommandPayload::CopyReportAiContext { .. } => "copy_report_ai_context",
        UiCommandPayload::OpenRawReport { .. } => "open_raw_report",
        UiCommandPayload::RevealReportPath { .. } => "reveal_report_path",
        UiCommandPayload::OpenRelatedReportArtifact { .. } => "open_related_report_artifact",
        UiCommandPayload::SelectTraceEntry { .. } => "select_trace_entry",
        UiCommandPayload::AiSubmitPrompt { .. } => "ai_submit_prompt",
        UiCommandPayload::GenerateProjectPatchFromPrompt { .. } => {
            "generate_project_patch_from_prompt"
        }
        UiCommandPayload::SetAiPromptDraft { .. } => "set_ai_prompt_draft",
        UiCommandPayload::CancelLlmPatchRequest => "cancel_llm_patch_request",
        UiCommandPayload::ImportProjectPatch { .. } => "import_project_patch",
        UiCommandPayload::PreviewImportedProjectPatch { .. } => "preview_imported_project_patch",
        UiCommandPayload::ApplyImportedProjectPatch { .. } => "apply_imported_project_patch",
        UiCommandPayload::ParkProjectWorkItem { .. } => "park_project_work_item",
        UiCommandPayload::ResumeProjectWorkItem { .. } => "resume_project_work_item",
        UiCommandPayload::ReopenProjectWorkItem { .. } => "reopen_project_work_item",
        UiCommandPayload::ApproveProjectChange { .. } => "approve_project_change",
        UiCommandPayload::AdvanceProjectProduction { .. } => "advance_project_production",
        UiCommandPayload::CancelProjectProduction { .. } => "cancel_project_production",
        UiCommandPayload::RecoverProjectProduction { .. } => "recover_project_production",
        UiCommandPayload::ApproveGatewayAccessRequest { .. } => "approve_gateway_access_request",
        UiCommandPayload::RejectGatewayAccessRequest { .. } => "reject_gateway_access_request",
        UiCommandPayload::SetGatewayAccessPage { .. } => "set_gateway_access_page",
        UiCommandPayload::ApproveProjectRuntimeTrust { .. } => "approve_project_runtime_trust",
        UiCommandPayload::DenyProjectRuntimeTrust { .. } => "deny_project_runtime_trust",
        UiCommandPayload::CancelProjectRuntimeTrust { .. } => "cancel_project_runtime_trust",
        UiCommandPayload::AiAcceptProposedCommand { .. } => "ai_accept_proposed_command",
        UiCommandPayload::AiRejectProposedCommand { .. } => "ai_reject_proposed_command",
    }
}
