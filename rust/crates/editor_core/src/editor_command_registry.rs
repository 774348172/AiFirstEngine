use crate::{
    EditorCommandCategory, EditorCommandDescriptor, EditorCommandOwnerDomain,
    EditorCommandPayloadKind,
};
use editor_ui_model::{ui_command_id_for_payload, UiCommandPayload};
use std::collections::HashSet;

pub struct EditorCommandRegistry {
    descriptors: Vec<EditorCommandDescriptor>,
}

impl EditorCommandRegistry {
    pub fn new(descriptors: Vec<EditorCommandDescriptor>) -> Result<Self, String> {
        validate_unique_command_ids(&descriptors)?;
        Ok(Self { descriptors })
    }

    pub fn builtin() -> Self {
        Self::new(builtin_editor_command_descriptors())
            .expect("builtin editor command descriptors must be unique")
    }

    pub fn descriptors(&self) -> &[EditorCommandDescriptor] {
        &self.descriptors
    }

    pub fn descriptor(&self, command_id: &str) -> Option<&EditorCommandDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.command_id == command_id)
    }

    pub fn descriptor_for_payload(
        &self,
        payload: &UiCommandPayload,
    ) -> Option<&EditorCommandDescriptor> {
        self.descriptor(command_id_for_payload(payload))
    }
}

pub fn command_id_for_payload(payload: &UiCommandPayload) -> &'static str {
    ui_command_id_for_payload(payload)
}

pub fn builtin_editor_command_descriptors() -> Vec<EditorCommandDescriptor> {
    use EditorCommandCategory as Category;
    use EditorCommandOwnerDomain as Owner;
    use EditorCommandPayloadKind as Payload;

    vec![
        descriptor(
            "open_project",
            "Open Project",
            Category::Project,
            Owner::ProjectLauncher,
            Payload::Project,
        ),
        descriptor(
            "create_project",
            "Create Project",
            Category::Project,
            Owner::ProjectLauncher,
            Payload::Project,
        ),
        descriptor(
            "start_create_project_with_ai",
            "Create with AI",
            Category::Project,
            Owner::ProjectLauncher,
            Payload::Project,
        ),
        descriptor(
            "select_recent_project",
            "Select Recent Project",
            Category::Project,
            Owner::ProjectLauncher,
            Payload::Project,
        ),
        descriptor(
            "refresh_recent_projects",
            "Refresh Recent Projects",
            Category::Project,
            Owner::ProjectLauncher,
            Payload::Project,
        ),
        descriptor(
            "select_project_browser_entry",
            "Select Project Browser Entry",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "open_project_browser_entry",
            "Open Project Browser Entry",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "select_asset_browser_entry",
            "Select Asset Browser Entry",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "open_asset_browser_entry",
            "Open Asset Browser Entry",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "set_asset_browser_folder",
            "Set Asset Browser Folder",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "set_asset_browser_search",
            "Set Asset Browser Search",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "set_asset_browser_kind_filter",
            "Set Asset Browser Kind Filter",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "asset_browser_toolbar",
            "Asset Browser Toolbar",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "scroll_asset_browser",
            "Scroll Asset Browser",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "begin_asset_pick",
            "Begin Asset Pick",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "confirm_asset_pick",
            "Confirm Asset Pick",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "cancel_asset_pick",
            "Cancel Asset Pick",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "drop_asset_on_inspector_field",
            "Drop Asset On Inspector Field",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "register_existing_asset",
            "Register Existing Asset",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "generate_mock_image_asset",
            "Generate Mock Image Asset",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "validate_asset_browser_index",
            "Validate Asset Browser Index",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "create_default_input_mapping",
            "Create Default Input Mapping",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "delete_input_mapping",
            "Delete Input Mapping",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "open_input_mapping",
            "Open Input Mapping",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "save_input_mapping",
            "Save Input Mapping",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "discard_input_mapping_draft",
            "Discard Input Mapping Draft",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "validate_input_mapping",
            "Validate Input Mapping",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "select_input_context",
            "Select Input Context",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "select_input_action",
            "Select Input Action",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "select_input_binding",
            "Select Input Binding",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "add_input_context",
            "Add Input Context",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "remove_input_context",
            "Remove Input Context",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_context_priority",
            "Set Input Context Priority",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_context_consume_input",
            "Set Input Context Consume Input",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "add_input_action",
            "Add Input Action",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "remove_input_action",
            "Remove Input Action",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_action_value_type",
            "Set Input Action Value Type",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "add_input_binding",
            "Add Input Binding",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "remove_input_binding",
            "Remove Input Binding",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_binding_device_path",
            "Set Input Binding Device Path",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_binding_processor_by_index",
            "Set Input Binding Processor By Index",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "remove_input_binding_by_id",
            "Remove Input Binding By Id",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_binding_device_path_by_id",
            "Set Input Binding Device Path By Id",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_binding_trigger",
            "Set Input Binding Trigger",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_binding_processor",
            "Set Input Binding Processor",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "begin_input_binding_capture",
            "Begin Input Binding Capture",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "cancel_input_binding_capture",
            "Cancel Input Binding Capture",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "commit_captured_input_binding",
            "Commit Captured Input Binding",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "preview_input_mapping",
            "Preview Input Mapping",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "set_input_mapping_report_level",
            "Set Input Mapping Report Level",
            Category::Workspace,
            Owner::InputMapping,
            Payload::InputMapping,
        ),
        descriptor(
            "create_rule_asset",
            "Create Rule Asset",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "open_rule_asset",
            "Open Rule Asset",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "select_rule_asset",
            "Select Rule Asset",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "set_rule_trigger",
            "Set Rule Trigger",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "add_rule_statement",
            "Add Rule Statement",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "update_rule_statement",
            "Update Rule Statement",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "remove_rule_statement",
            "Remove Rule Statement",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "add_rule_operation",
            "Add Rule Operation",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "update_rule_operation",
            "Update Rule Operation",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "remove_rule_operation",
            "Remove Rule Operation",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "validate_rule_asset",
            "Validate Rule Asset",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "build_rule_artifact",
            "Build Rule Artifact",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "build_project_rule_manifest",
            "Build Project Rule Manifest",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "save_rule_asset",
            "Save Rule Asset",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "open_rule_diagnostics",
            "Open Rule Diagnostics",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "select_rule_card",
            "Select Rule Card",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "set_rule_card_field",
            "Set Rule Card Field",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "add_rule_card",
            "Add Rule Card",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "remove_rule_card",
            "Remove Rule Card",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "select_rule_graph_node",
            "Select Rule Graph Node",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "refresh_rule_graph_preview",
            "Refresh Rule Graph Preview",
            Category::Rule,
            Owner::RuleAuthoring,
            Payload::Rule,
        ),
        descriptor(
            "create_prefab_from_selection",
            "Create Prefab From Selection",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "open_prefab_document",
            "Open Prefab Document",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "enter_prefab_stage",
            "Enter Prefab Stage",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "exit_prefab_stage",
            "Exit Prefab Stage",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "instantiate_prefab_in_scene",
            "Instantiate Prefab In Scene",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "set_prefab_stage_entity_field",
            "Set Prefab Stage Entity Field",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "apply_prefab_override_to_asset",
            "Apply Prefab Override To Asset",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "save_prefab_document",
            "Save Prefab Document",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "validate_prefab_references",
            "Validate Prefab References",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "revert_prefab_override",
            "Revert Prefab Override",
            Category::Prefab,
            Owner::PrefabAuthoring,
            Payload::Prefab,
        ),
        descriptor(
            "create_aui_document",
            "Create AUI Document",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "open_aui_document",
            "Open AUI Document",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "select_aui_node",
            "Select AUI Node",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "add_aui_node",
            "Add AUI Node",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "set_aui_node_field",
            "Set AUI Node Field",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "set_aui_binding_path",
            "Set AUI Binding Path",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "set_aui_action_ref",
            "Set AUI Action Ref",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "validate_aui_document",
            "Validate AUI Document",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "save_aui_document",
            "Save AUI Document",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "preview_aui_overlay",
            "Preview AUI Overlay",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "save_aui_subtree_as_template",
            "Save AUI Subtree As Template",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "instantiate_aui_template",
            "Instantiate AUI Template",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "validate_aui_template",
            "Validate AUI Template",
            Category::Aui,
            Owner::AuiAuthoring,
            Payload::Aui,
        ),
        descriptor(
            "set_workspace_view_mode",
            "Set Workspace View Mode",
            Category::Workspace,
            Owner::AuthoringWorkspace,
            Payload::Workspace,
        ),
        descriptor(
            "set_authoring_workflow_step",
            "Set Authoring Workflow Step",
            Category::Workspace,
            Owner::AuthoringWorkspace,
            Payload::Workspace,
        ),
        descriptor(
            "open_runtime_package",
            "Open Runtime Package",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "open_scene_document",
            "Open Scene Document",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "reload_runtime_package",
            "Reload Runtime Package",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "select_entity",
            "Select Entity",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "select_runtime_entity",
            "Select Runtime Entity",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "pick_runtime_entity_at",
            "Pick Runtime Entity At",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "select_scene_entity",
            "Select Scene Entity",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "create_scene_entity",
            "Create Scene Entity",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "place_asset_into_scene",
            "Place Asset Into Scene",
            Category::Asset,
            Owner::AssetBrowser,
            Payload::Asset,
        ),
        descriptor(
            "delete_scene_entity",
            "Delete Scene Entity",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "rename_scene_entity",
            "Rename Scene Entity",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "set_scene_transform",
            "Set Scene Transform",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "add_scene_component",
            "Add Scene Component",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "remove_scene_component",
            "Remove Scene Component",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "set_scene_component_field",
            "Set Scene Component Field",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "set_runtime_component_field_temporary",
            "Set Runtime Component Field Temporary",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "preview_apply_runtime_change_to_authoring",
            "Preview Apply Runtime Change To Authoring",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "apply_runtime_change_to_authoring",
            "Apply Runtime Change To Authoring",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "save_scene_document",
            "Save Scene Document",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "undo_scene_edit",
            "Undo Scene Edit",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "redo_scene_edit",
            "Redo Scene Edit",
            Category::Scene,
            Owner::SceneEditing,
            Payload::Scene,
        ),
        descriptor(
            "tick_one_frame",
            "Tick One Frame",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "play",
            "Play",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "pause",
            "Pause",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "step_frame",
            "Step Frame",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "stop_play_session",
            "Stop Play Session",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "set_game_view_target",
            "Set GameView Target",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "set_game_view_maximize_on_play",
            "Set GameView Maximize on Play",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "toggle_game_view_maximize_on_play",
            "Toggle GameView Maximize on Play",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "reset_runtime",
            "Reset Runtime",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Runtime,
        ),
        descriptor(
            "export_desktop_package",
            "Export Desktop Package",
            Category::Build,
            Owner::BuildExport,
            Payload::Build,
        ),
        descriptor(
            "build_and_run_desktop_package",
            "Build And Run Desktop Package",
            Category::Build,
            Owner::BuildExport,
            Payload::Build,
        ),
        descriptor(
            "build_release_package",
            "Build Release Package",
            Category::Build,
            Owner::BuildExport,
            Payload::Build,
        ),
        descriptor(
            "save_release_profile",
            "Save Release Profile",
            Category::Build,
            Owner::BuildExport,
            Payload::Build,
        ),
        descriptor(
            "set_release_profile_icon",
            "Set Release Profile Icon",
            Category::Build,
            Owner::BuildExport,
            Payload::Build,
        ),
        descriptor(
            "open_build_output",
            "Open Build Output",
            Category::Build,
            Owner::BuildExport,
            Payload::Build,
        ),
        descriptor(
            "open_build_report",
            "Open Build Report",
            Category::Build,
            Owner::BuildExport,
            Payload::Build,
        ),
        descriptor(
            "clear_console",
            "Clear Console",
            Category::Console,
            Owner::Console,
            Payload::Console,
        ),
        descriptor(
            "select_report_entry",
            "Select Report Entry",
            Category::Report,
            Owner::ReportPanel,
            Payload::Report,
        ),
        descriptor(
            "refresh_reports",
            "Refresh Reports",
            Category::Report,
            Owner::ReportPanel,
            Payload::Report,
        ),
        descriptor(
            "copy_report_ai_context",
            "Copy Report AI Context",
            Category::Report,
            Owner::ReportPanel,
            Payload::Report,
        ),
        descriptor(
            "open_raw_report",
            "Open Raw Report",
            Category::Report,
            Owner::ReportPanel,
            Payload::Report,
        ),
        descriptor(
            "reveal_report_path",
            "Reveal Report Path",
            Category::Report,
            Owner::ReportPanel,
            Payload::Report,
        ),
        descriptor(
            "open_related_report_artifact",
            "Open Related Report Artifact",
            Category::Report,
            Owner::ReportPanel,
            Payload::Report,
        ),
        descriptor(
            "select_trace_entry",
            "Select Trace Entry",
            Category::Runtime,
            Owner::RuntimeSession,
            Payload::Trace,
        ),
        descriptor(
            "ai_submit_prompt",
            "AI Submit Prompt",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "generate_project_patch_from_prompt",
            "Generate Project Patch From Prompt",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "set_ai_prompt_draft",
            "Set AI Prompt Draft",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "cancel_llm_patch_request",
            "Cancel LLM Patch Request",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "import_project_patch",
            "Import Project Patch",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "preview_imported_project_patch",
            "Preview Imported Project Patch",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "apply_imported_project_patch",
            "Apply Imported Project Patch",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "ai_accept_proposed_command",
            "AI Accept Proposed Command",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
        descriptor(
            "ai_reject_proposed_command",
            "AI Reject Proposed Command",
            Category::Ai,
            Owner::AiPanel,
            Payload::Ai,
        ),
    ]
}

fn descriptor(
    command_id: &'static str,
    title: &'static str,
    category: EditorCommandCategory,
    owner_domain: EditorCommandOwnerDomain,
    payload_kind: EditorCommandPayloadKind,
) -> EditorCommandDescriptor {
    EditorCommandDescriptor {
        command_id,
        title,
        category,
        owner_domain,
        payload_kind,
    }
}

fn validate_unique_command_ids(descriptors: &[EditorCommandDescriptor]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for descriptor in descriptors {
        if !ids.insert(descriptor.command_id) {
            return Err(format!(
                "duplicate editor command id: {}",
                descriptor.command_id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_ui_model::{
        AssetPlacementMode, PrefabStageMode, PrefabStageSavePolicy, UiCommandPayload, Vec3,
    };

    #[test]
    fn command_registry_rejects_duplicate_ids() {
        let duplicate = vec![
            descriptor(
                "scene.select",
                "A",
                EditorCommandCategory::Scene,
                EditorCommandOwnerDomain::SceneEditing,
                EditorCommandPayloadKind::Scene,
            ),
            descriptor(
                "scene.select",
                "B",
                EditorCommandCategory::Scene,
                EditorCommandOwnerDomain::SceneEditing,
                EditorCommandPayloadKind::Scene,
            ),
        ];

        assert!(EditorCommandRegistry::new(duplicate).is_err());
    }

    #[test]
    fn all_builtin_payloads_have_descriptor() {
        let registry = EditorCommandRegistry::builtin();
        for payload in representative_payloads() {
            assert!(
                registry.descriptor_for_payload(&payload).is_some(),
                "missing descriptor for {}",
                command_id_for_payload(&payload)
            );
        }
    }

    #[test]
    fn command_id_helper_uses_ui_model_truth() {
        assert_eq!(
            command_id_for_payload(&UiCommandPayload::SetSceneTransform {
                entity_id: "entity".to_string(),
                local_position: Some(Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0
                }),
                local_rotation: None,
                local_scale: None,
            }),
            "set_scene_transform"
        );
    }

    fn representative_payloads() -> Vec<UiCommandPayload> {
        vec![
            UiCommandPayload::OpenProject {
                path: "project".to_string(),
            },
            UiCommandPayload::CreateProject {
                path: "root".to_string(),
                name: "Game".to_string(),
            },
            UiCommandPayload::SelectRecentProject {
                path: "project".to_string(),
            },
            UiCommandPayload::RefreshRecentProjects,
            UiCommandPayload::SelectProjectBrowserEntry {
                path: "Assets".to_string(),
            },
            UiCommandPayload::OpenProjectBrowserEntry {
                path: "Assets".to_string(),
            },
            UiCommandPayload::CreateDefaultInputMapping {
                path: "input.json".to_string(),
            },
            UiCommandPayload::DeleteInputMapping {
                path: "input.json".to_string(),
            },
            UiCommandPayload::SaveInputMapping {
                path: "input.json".to_string(),
            },
            UiCommandPayload::ValidateInputMapping {
                path: "input.json".to_string(),
            },
            UiCommandPayload::AddInputAction {
                path: "input.json".to_string(),
                action_id: "move".to_string(),
                value_type: editor_ui_model::InputActionValueKind::Button,
            },
            UiCommandPayload::RemoveInputAction {
                path: "input.json".to_string(),
                action_id: "move".to_string(),
            },
            UiCommandPayload::AddInputBinding {
                path: "input.json".to_string(),
                context_id: "gameplay".to_string(),
                action_id: "move".to_string(),
                device_path: "keyboard.space".to_string(),
            },
            UiCommandPayload::RemoveInputBinding {
                path: "input.json".to_string(),
                binding_index: 0,
            },
            UiCommandPayload::SetInputBindingDevicePath {
                path: "input.json".to_string(),
                binding_index: 0,
                device_path: "keyboard.enter".to_string(),
            },
            UiCommandPayload::SetInputBindingProcessorByIndex {
                path: "input.json".to_string(),
                binding_index: 0,
                processor: editor_ui_model::InputProcessorKind::Invert,
            },
            UiCommandPayload::RegisterExistingAsset {
                path: "Assets/ship.png".to_string(),
                expected_kind: Some(editor_ui_model::AssetKind::Texture),
            },
            UiCommandPayload::GenerateMockImageAsset {
                prompt: "small ship sprite".to_string(),
                target_folder: "Assets/Generated".to_string(),
                asset_name: "ship".to_string(),
                image_kind: "sprite".to_string(),
                width: 32,
                height: 32,
                transparent_background: true,
            },
            UiCommandPayload::ValidateAssetBrowserIndex {
                query_kind: Some(editor_ui_model::AssetKind::Sprite),
            },
            UiCommandPayload::CreateRuleAsset {
                path: "Rules/fire.rule.json".to_string(),
                rule_id: "project.rule.fire".to_string(),
                display_name: "Fire".to_string(),
                phase: None,
            },
            UiCommandPayload::OpenRuleAsset {
                path: "Rules/fire.rule.json".to_string(),
            },
            UiCommandPayload::SelectRuleAsset {
                path: "Rules/fire.rule.json".to_string(),
            },
            UiCommandPayload::SetRuleTrigger {
                path: "Rules/fire.rule.json".to_string(),
                trigger: serde_json::json!({
                    "kind": "actionPressed",
                    "actionId": "fire"
                }),
                expected_ir_hash: None,
            },
            UiCommandPayload::AddRuleStatement {
                path: "Rules/fire.rule.json".to_string(),
                statement: serde_json::json!({
                    "kind": "operation",
                    "operation": {
                        "op": "emitEvent",
                        "eventType": "project.event"
                    }
                }),
                expected_ir_hash: None,
            },
            UiCommandPayload::UpdateRuleStatement {
                path: "Rules/fire.rule.json".to_string(),
                statement_index: 0,
                statement: serde_json::json!({
                    "kind": "operation",
                    "operation": {
                        "op": "emitEvent",
                        "eventType": "project.event.updated"
                    }
                }),
                expected_ir_hash: None,
            },
            UiCommandPayload::RemoveRuleStatement {
                path: "Rules/fire.rule.json".to_string(),
                statement_index: 0,
                expected_ir_hash: None,
            },
            UiCommandPayload::AddRuleOperation {
                path: "Rules/fire.rule.json".to_string(),
                operation: serde_json::json!({
                    "op": "emitEvent",
                    "eventType": "project.event"
                }),
                expected_ir_hash: None,
            },
            UiCommandPayload::UpdateRuleOperation {
                path: "Rules/fire.rule.json".to_string(),
                operation_index: 0,
                operation: serde_json::json!({
                    "op": "emitEvent",
                    "eventType": "project.event.updated"
                }),
                expected_ir_hash: None,
            },
            UiCommandPayload::RemoveRuleOperation {
                path: "Rules/fire.rule.json".to_string(),
                operation_index: 0,
                expected_ir_hash: None,
            },
            UiCommandPayload::ValidateRuleAsset {
                path: "Rules/fire.rule.json".to_string(),
            },
            UiCommandPayload::BuildRuleArtifact {
                path: "Rules/fire.rule.json".to_string(),
            },
            UiCommandPayload::BuildProjectRuleManifest {
                path: "Rules/rule-manifest.json".to_string(),
            },
            UiCommandPayload::SaveRuleAsset {
                path: "Rules/fire.rule.json".to_string(),
            },
            UiCommandPayload::OpenRuleDiagnostics {
                path: "Rules/fire.rule.json".to_string(),
            },
            UiCommandPayload::SelectRuleCard {
                path: "Rules/fire.rule.json".to_string(),
                card_id: "card:trigger".to_string(),
            },
            UiCommandPayload::SetRuleCardField {
                path: "Rules/fire.rule.json".to_string(),
                card_id: "card:trigger".to_string(),
                field_path: "canonicalIr.trigger.actionId".to_string(),
                value: serde_json::json!("action.fire"),
                expected_ir_hash: None,
            },
            UiCommandPayload::AddRuleCard {
                path: "Rules/fire.rule.json".to_string(),
                card_kind: "operation".to_string(),
                value: serde_json::json!({
                    "op": "emitEvent",
                    "eventType": "project.event"
                }),
                expected_ir_hash: None,
            },
            UiCommandPayload::RemoveRuleCard {
                path: "Rules/fire.rule.json".to_string(),
                card_id: "card:operation:0".to_string(),
                expected_ir_hash: None,
            },
            UiCommandPayload::SelectRuleGraphNode {
                path: "Rules/fire.rule.json".to_string(),
                node_id: "node:trigger".to_string(),
            },
            UiCommandPayload::RefreshRuleGraphPreview {
                path: "Rules/fire.rule.json".to_string(),
            },
            UiCommandPayload::CreatePrefabFromSelection {
                scene_path: None,
                root_entity_id: "entity-root".to_string(),
                prefab_id: "prefab-ship".to_string(),
                name: "Ship".to_string(),
                replace_selection_with_instance: true,
            },
            UiCommandPayload::OpenPrefabDocument {
                path: "Prefabs/ship.prefab.json".to_string(),
            },
            UiCommandPayload::EnterPrefabStage {
                path: "Prefabs/ship.prefab.json".to_string(),
                mode: PrefabStageMode::Isolated,
                opened_from_instance_entity_id: None,
            },
            UiCommandPayload::ExitPrefabStage {
                save_policy: PrefabStageSavePolicy::KeepOpen,
            },
            UiCommandPayload::InstantiatePrefabInScene {
                prefab_id: "prefab-ship".to_string(),
                parent_entity_id: None,
                local_position: None,
            },
            UiCommandPayload::SetPrefabStageEntityField {
                source_entity_id: "entity-root".to_string(),
                component_type: Some("project.stats".to_string()),
                field_path: "speed".to_string(),
                value: serde_json::json!(2.0),
            },
            UiCommandPayload::ApplyPrefabOverrideToAsset {
                instance_entity_id: "entity-instance".to_string(),
                target_source_entity_id: "entity-root".to_string(),
                component_type: "project.stats".to_string(),
                field_path: "speed".to_string(),
            },
            UiCommandPayload::SavePrefabDocument {
                path: "Prefabs/ship.prefab.json".to_string(),
            },
            UiCommandPayload::ValidatePrefabReferences { path: None },
            UiCommandPayload::RevertPrefabOverride {
                instance_entity_id: "entity-instance".to_string(),
                target_source_entity_id: "entity-root".to_string(),
                component_type: "project.stats".to_string(),
                field_path: "speed".to_string(),
            },
            UiCommandPayload::CreateAuiDocument {
                path: "AUI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                width: 1280.0,
                height: 720.0,
            },
            UiCommandPayload::OpenAuiDocument {
                path: "AUI/hud.aui.json".to_string(),
            },
            UiCommandPayload::SelectAuiNode {
                document_path: "AUI/hud.aui.json".to_string(),
                document_id: "hud".to_string(),
                node_id: "score_text".to_string(),
            },
            UiCommandPayload::AddAuiNode {
                path: "AUI/hud.aui.json".to_string(),
                parent_node_id: "root".to_string(),
                node_id: "score_text".to_string(),
                kind: "text".to_string(),
                name: "Score Text".to_string(),
                rect: serde_json::json!({
                    "x": 16.0,
                    "y": 16.0,
                    "width": 220.0,
                    "height": 40.0
                }),
            },
            UiCommandPayload::SetAuiNodeField {
                path: "AUI/hud.aui.json".to_string(),
                node_id: "score_text".to_string(),
                schema_path: "text".to_string(),
                value: serde_json::json!("Score: 0"),
            },
            UiCommandPayload::SetAuiBindingPath {
                path: "AUI/hud.aui.json".to_string(),
                node_id: "score_text".to_string(),
                target_field: "text.text".to_string(),
                binding_id: "bind.score".to_string(),
                binding_path: "game.score_text".to_string(),
                fallback: Some(serde_json::json!("Score: 0")),
            },
            UiCommandPayload::SetAuiActionRef {
                path: "AUI/hud.aui.json".to_string(),
                node_id: "pause_button".to_string(),
                event: "click".to_string(),
                action_id: "ui.pause".to_string(),
                payload: None,
            },
            UiCommandPayload::ValidateAuiDocument {
                path: "AUI/hud.aui.json".to_string(),
            },
            UiCommandPayload::SaveAuiDocument {
                path: "AUI/hud.aui.json".to_string(),
            },
            UiCommandPayload::PreviewAuiOverlay {
                path: "AUI/hud.aui.json".to_string(),
            },
            UiCommandPayload::SaveAuiSubtreeAsTemplate {
                document_path: "AUI/hud.aui.json".to_string(),
                root_node_id: "score_text".to_string(),
                template_asset_path: "AUI/Templates/score_text.aui-template.json".to_string(),
                template_id: "score_text_template".to_string(),
                display_name: "Score Text Template".to_string(),
            },
            UiCommandPayload::InstantiateAuiTemplate {
                template_asset_path: "AUI/Templates/score_text.aui-template.json".to_string(),
                template_id: "score_text_template".to_string(),
                target_document_path: "AUI/hud.aui.json".to_string(),
                parent_node_id: "root".to_string(),
                insertion_index: None,
                instance_id: "score_text_instance".to_string(),
                node_id_prefix: "score_copy".to_string(),
            },
            UiCommandPayload::ValidateAuiTemplate {
                template_asset_path: "AUI/Templates/score_text.aui-template.json".to_string(),
                template_id: "score_text_template".to_string(),
            },
            UiCommandPayload::SetWorkspaceViewMode {
                mode: editor_ui_model::WorkspaceViewMode::SceneView,
            },
            UiCommandPayload::SetAuthoringWorkflowStep {
                step_id: editor_ui_model::AuthoringStepId::Scene,
            },
            UiCommandPayload::OpenRuntimePackage {
                path: "package".to_string(),
            },
            UiCommandPayload::OpenSceneDocument {
                path: "scene.json".to_string(),
            },
            UiCommandPayload::ReloadRuntimePackage,
            UiCommandPayload::SelectEntity {
                entity_id: "entity".to_string(),
            },
            UiCommandPayload::SelectRuntimeEntity {
                entity_id: "entity".to_string(),
            },
            UiCommandPayload::SelectSceneEntity {
                entity_id: "entity".to_string(),
            },
            UiCommandPayload::CreateSceneEntity {
                parent_id: None,
                name: "Entity".to_string(),
            },
            UiCommandPayload::PlaceAssetIntoScene {
                asset_id: "asset".to_string(),
                asset_type: "Sprite".to_string(),
                asset_guid: None,
                target_parent_id: None,
                local_position: None,
                placement_mode: AssetPlacementMode::WorldOrigin,
            },
            UiCommandPayload::DeleteSceneEntity {
                entity_id: "entity".to_string(),
            },
            UiCommandPayload::RenameSceneEntity {
                entity_id: "entity".to_string(),
                name: "Renamed".to_string(),
            },
            UiCommandPayload::SetSceneTransform {
                entity_id: "entity".to_string(),
                local_position: None,
                local_rotation: None,
                local_scale: None,
            },
            UiCommandPayload::AddSceneComponent {
                entity_id: "entity".to_string(),
                component_type: "project.state".to_string(),
                fields: serde_json::json!({}),
            },
            UiCommandPayload::RemoveSceneComponent {
                entity_id: "entity".to_string(),
                component_type: "project.state".to_string(),
            },
            UiCommandPayload::SetSceneComponentField {
                entity_id: "entity".to_string(),
                component_type: "SpriteRenderer2D".to_string(),
                field_path: "visible".to_string(),
                value: serde_json::Value::Bool(true),
            },
            UiCommandPayload::SetRuntimeComponentFieldTemporary {
                entity_id: "entity".to_string(),
                component_type: "Transform".to_string(),
                field_path: "local_position.x".to_string(),
                value: serde_json::json!(1.0),
            },
            UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring,
            UiCommandPayload::ApplyRuntimeChangeToAuthoring {
                edit_id: "runtime-edit-1".to_string(),
                candidate_hash: "hash".to_string(),
            },
            UiCommandPayload::SaveSceneDocument { path: None },
            UiCommandPayload::UndoSceneEdit,
            UiCommandPayload::RedoSceneEdit,
            UiCommandPayload::TickOneFrame,
            UiCommandPayload::PickRuntimeEntityAt {
                x: 400.0,
                y: 300.0,
                viewport_width: Some(800.0),
                viewport_height: Some(600.0),
                aui_consumed: false,
            },
            UiCommandPayload::Play,
            UiCommandPayload::Pause,
            UiCommandPayload::StepFrame,
            UiCommandPayload::ResetRuntime,
            UiCommandPayload::ExportDesktopPackage { profile_id: None },
            UiCommandPayload::BuildAndRunDesktopPackage { profile_id: None },
            UiCommandPayload::OpenBuildOutput,
            UiCommandPayload::OpenBuildReport,
            UiCommandPayload::ClearConsole,
            UiCommandPayload::SelectReportEntry {
                report_id: "report-build-export".to_string(),
            },
            UiCommandPayload::RefreshReports,
            UiCommandPayload::CopyReportAiContext {
                report_id: "report-build-export".to_string(),
            },
            UiCommandPayload::OpenRawReport {
                report_id: "report-build-export".to_string(),
            },
            UiCommandPayload::RevealReportPath {
                report_id: "report-build-export".to_string(),
            },
            UiCommandPayload::OpenRelatedReportArtifact {
                report_id: "report-build-export".to_string(),
                artifact_id: "desktop-export-report".to_string(),
            },
            UiCommandPayload::SelectTraceEntry {
                entry_id: "trace".to_string(),
            },
            UiCommandPayload::AiSubmitPrompt {
                prompt: "create".to_string(),
            },
            UiCommandPayload::GenerateProjectPatchFromPrompt {
                prompt: "create".to_string(),
            },
            UiCommandPayload::SetAiPromptDraft {
                prompt: "create".to_string(),
            },
            UiCommandPayload::CancelLlmPatchRequest,
            UiCommandPayload::ImportProjectPatch {
                source_label: "fixture".to_string(),
                raw_json: Some("{}".to_string()),
                file_path: None,
                expected_patch_id: None,
                dry_run: true,
            },
            UiCommandPayload::PreviewImportedProjectPatch {
                source_label: "fixture".to_string(),
                raw_json: Some("{}".to_string()),
                file_path: None,
                expected_patch_id: None,
            },
            UiCommandPayload::ApplyImportedProjectPatch {
                proposal_id: "proposal".to_string(),
            },
            UiCommandPayload::AiAcceptProposedCommand {
                proposal_id: "proposal".to_string(),
            },
            UiCommandPayload::AiRejectProposedCommand {
                proposal_id: "proposal".to_string(),
            },
        ]
    }
}
