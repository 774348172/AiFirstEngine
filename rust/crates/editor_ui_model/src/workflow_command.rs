use serde::{Deserialize, Serialize};

use super::{
    AuthoringCommand, AuthoringCommandAvailability, PrefabStageMode, PrefabStageSavePolicy,
    UiCommandPayload, WorkspaceDomainKind,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowCommandResolution {
    Command(UiCommandPayload),
    FocusDomainPanel {
        domain: WorkspaceDomainKind,
        reason: String,
    },
    Disabled {
        reason: String,
    },
    Unsupported {
        reason: String,
    },
}

pub struct WorkflowCommandResolver;

impl WorkflowCommandResolver {
    pub fn resolve(command: &AuthoringCommand) -> WorkflowCommandResolution {
        if command.availability == AuthoringCommandAvailability::Disabled {
            return WorkflowCommandResolution::Disabled {
                reason: format!("{} is disabled.", command.label),
            };
        }

        match command.command_id.as_str() {
            "refresh_recent_projects" => {
                WorkflowCommandResolution::Command(UiCommandPayload::RefreshRecentProjects)
            }
            "open_project" => WorkflowCommandResolution::Command(UiCommandPayload::OpenProject {
                path: String::new(),
            }),
            "select_project_browser_entry" => WorkflowCommandResolution::FocusDomainPanel {
                domain: WorkspaceDomainKind::Asset,
                reason: "Browse Assets needs the Project Browser selection context.".to_string(),
            },
            "open_scene_document" => {
                WorkflowCommandResolution::Command(UiCommandPayload::OpenSceneDocument {
                    path: String::new(),
                })
            }
            "create_scene_entity" => WorkflowCommandResolution::FocusDomainPanel {
                domain: WorkspaceDomainKind::Scene,
                reason: "Create Entity needs Scene/Hierarchy parameter context.".to_string(),
            },
            "create_default_input_mapping" => {
                WorkflowCommandResolution::Command(UiCommandPayload::CreateDefaultInputMapping {
                    path: String::new(),
                })
            }
            "create_rule_asset" => {
                WorkflowCommandResolution::Command(UiCommandPayload::CreateRuleAsset {
                    path: String::new(),
                    rule_id: String::new(),
                    display_name: String::new(),
                    phase: None,
                })
            }
            "open_rule_asset" => {
                WorkflowCommandResolution::Command(UiCommandPayload::OpenRuleAsset {
                    path: String::new(),
                })
            }
            "edit_rule_graph_or_dsl" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SetRuleCardField {
                    path: String::new(),
                    card_id: String::new(),
                    field_path: String::new(),
                    value: serde_json::Value::Null,
                    expected_ir_hash: None,
                })
            }
            "select_rule_card" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SelectRuleCard {
                    path: String::new(),
                    card_id: String::new(),
                })
            }
            "set_rule_card_field" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SetRuleCardField {
                    path: String::new(),
                    card_id: String::new(),
                    field_path: String::new(),
                    value: serde_json::Value::Null,
                    expected_ir_hash: None,
                })
            }
            "add_rule_card" => WorkflowCommandResolution::Command(UiCommandPayload::AddRuleCard {
                path: String::new(),
                card_kind: String::new(),
                value: serde_json::Value::Null,
                expected_ir_hash: None,
            }),
            "remove_rule_card" => {
                WorkflowCommandResolution::Command(UiCommandPayload::RemoveRuleCard {
                    path: String::new(),
                    card_id: String::new(),
                    expected_ir_hash: None,
                })
            }
            "select_rule_graph_node" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SelectRuleGraphNode {
                    path: String::new(),
                    node_id: String::new(),
                })
            }
            "refresh_rule_graph_preview" => {
                WorkflowCommandResolution::Command(UiCommandPayload::RefreshRuleGraphPreview {
                    path: String::new(),
                })
            }
            "validate_rule_asset" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ValidateRuleAsset {
                    path: String::new(),
                })
            }
            "build_rule_artifact" => {
                WorkflowCommandResolution::Command(UiCommandPayload::BuildRuleArtifact {
                    path: String::new(),
                })
            }
            "register_rule_artifact" => {
                WorkflowCommandResolution::Command(UiCommandPayload::BuildRuleArtifact {
                    path: String::new(),
                })
            }
            "inspect_rule_diagnostics" => {
                WorkflowCommandResolution::Command(UiCommandPayload::OpenRuleDiagnostics {
                    path: String::new(),
                })
            }
            "create_prefab_from_selection" => {
                WorkflowCommandResolution::Command(UiCommandPayload::CreatePrefabFromSelection {
                    scene_path: None,
                    root_entity_id: String::new(),
                    prefab_id: String::new(),
                    name: String::new(),
                    replace_selection_with_instance: true,
                })
            }
            "open_prefab_document" => {
                WorkflowCommandResolution::Command(UiCommandPayload::OpenPrefabDocument {
                    path: String::new(),
                })
            }
            "enter_prefab_stage" => {
                WorkflowCommandResolution::Command(UiCommandPayload::EnterPrefabStage {
                    path: String::new(),
                    mode: PrefabStageMode::Isolated,
                    opened_from_instance_entity_id: None,
                })
            }
            "exit_prefab_stage" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ExitPrefabStage {
                    save_policy: PrefabStageSavePolicy::KeepOpen,
                })
            }
            "instantiate_prefab_in_scene" => {
                WorkflowCommandResolution::Command(UiCommandPayload::InstantiatePrefabInScene {
                    prefab_id: String::new(),
                    parent_entity_id: None,
                    local_position: None,
                })
            }
            "set_prefab_stage_entity_field" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SetPrefabStageEntityField {
                    source_entity_id: String::new(),
                    component_type: None,
                    field_path: String::new(),
                    value: serde_json::Value::Null,
                })
            }
            "apply_prefab_changes" | "apply_prefab_override_to_asset" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ApplyPrefabOverrideToAsset {
                    instance_entity_id: String::new(),
                    target_source_entity_id: String::new(),
                    component_type: String::new(),
                    field_path: String::new(),
                })
            }
            "save_prefab_document" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SavePrefabDocument {
                    path: String::new(),
                })
            }
            "validate_prefab_references" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ValidatePrefabReferences {
                    path: None,
                })
            }
            "revert_prefab_override" => {
                WorkflowCommandResolution::Command(UiCommandPayload::RevertPrefabOverride {
                    instance_entity_id: String::new(),
                    target_source_entity_id: String::new(),
                    component_type: String::new(),
                    field_path: String::new(),
                })
            }
            "create_aui_document" => {
                WorkflowCommandResolution::Command(UiCommandPayload::CreateAuiDocument {
                    path: String::new(),
                    document_id: String::new(),
                    width: 1280.0,
                    height: 720.0,
                })
            }
            "open_aui_document" => {
                WorkflowCommandResolution::Command(UiCommandPayload::OpenAuiDocument {
                    path: String::new(),
                })
            }
            "add_aui_node" => WorkflowCommandResolution::Command(UiCommandPayload::AddAuiNode {
                path: String::new(),
                parent_node_id: String::new(),
                node_id: String::new(),
                kind: String::new(),
                name: String::new(),
                rect: serde_json::Value::Null,
            }),
            "set_aui_node_field" | "edit_aui_node_field" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SetAuiNodeField {
                    path: String::new(),
                    node_id: String::new(),
                    schema_path: String::new(),
                    value: serde_json::Value::Null,
                })
            }
            "set_aui_binding_path" | "edit_aui_binding_path" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SetAuiBindingPath {
                    path: String::new(),
                    node_id: String::new(),
                    target_field: String::new(),
                    binding_id: String::new(),
                    binding_path: String::new(),
                    fallback: None,
                })
            }
            "set_aui_action_ref" | "edit_aui_action_ref" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SetAuiActionRef {
                    path: String::new(),
                    node_id: String::new(),
                    event: "click".to_string(),
                    action_id: String::new(),
                    payload: None,
                })
            }
            "validate_aui_document" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ValidateAuiDocument {
                    path: String::new(),
                })
            }
            "save_aui_document" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SaveAuiDocument {
                    path: String::new(),
                })
            }
            "preview_aui_overlay" => {
                WorkflowCommandResolution::Command(UiCommandPayload::PreviewAuiOverlay {
                    path: String::new(),
                })
            }
            "save_aui_subtree_as_template" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SaveAuiSubtreeAsTemplate {
                    document_path: String::new(),
                    root_node_id: String::new(),
                    template_asset_path: String::new(),
                    template_id: String::new(),
                    display_name: String::new(),
                })
            }
            "instantiate_aui_template" => {
                WorkflowCommandResolution::Command(UiCommandPayload::InstantiateAuiTemplate {
                    template_asset_path: String::new(),
                    template_id: String::new(),
                    target_document_path: String::new(),
                    parent_node_id: String::new(),
                    insertion_index: None,
                    instance_id: String::new(),
                    node_id_prefix: String::new(),
                })
            }
            "validate_aui_template" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ValidateAuiTemplate {
                    template_asset_path: String::new(),
                    template_id: String::new(),
                })
            }
            "play" => WorkflowCommandResolution::Command(UiCommandPayload::Play),
            "export_desktop_package" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ExportDesktopPackage {
                    profile_id: None,
                })
            }
            "build_and_run_desktop_package" => {
                WorkflowCommandResolution::Command(UiCommandPayload::BuildAndRunDesktopPackage {
                    profile_id: None,
                })
            }
            "build_release_package" => {
                WorkflowCommandResolution::Command(UiCommandPayload::BuildReleasePackage {
                    profile_id: Some("windows-release".to_string()),
                })
            }
            "open_build_report" => {
                WorkflowCommandResolution::Command(UiCommandPayload::OpenBuildReport)
            }
            "clear_console" => WorkflowCommandResolution::Command(UiCommandPayload::ClearConsole),
            "refresh_reports" => {
                WorkflowCommandResolution::Command(UiCommandPayload::RefreshReports)
            }
            "select_report_entry" => {
                WorkflowCommandResolution::Command(UiCommandPayload::SelectReportEntry {
                    report_id: String::new(),
                })
            }
            "open_raw_report" => {
                WorkflowCommandResolution::Command(UiCommandPayload::OpenRawReport {
                    report_id: String::new(),
                })
            }
            "copy_report_ai_context" => {
                WorkflowCommandResolution::Command(UiCommandPayload::CopyReportAiContext {
                    report_id: String::new(),
                })
            }
            "generate_project_patch_from_prompt" => WorkflowCommandResolution::Command(
                UiCommandPayload::GenerateProjectPatchFromPrompt {
                    prompt: String::new(),
                },
            ),
            "import_project_patch" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ImportProjectPatch {
                    source_label: String::new(),
                    raw_json: None,
                    file_path: None,
                    expected_patch_id: None,
                    dry_run: true,
                })
            }
            "preview_imported_project_patch" => {
                WorkflowCommandResolution::Command(UiCommandPayload::PreviewImportedProjectPatch {
                    source_label: String::new(),
                    raw_json: None,
                    file_path: None,
                    expected_patch_id: None,
                })
            }
            "apply_imported_project_patch" => {
                WorkflowCommandResolution::Command(UiCommandPayload::ApplyImportedProjectPatch {
                    proposal_id: String::new(),
                })
            }
            "focus_prefab_panel" | "focus_rule_panel" | "focus_aui_panel" => {
                WorkflowCommandResolution::FocusDomainPanel {
                    domain: command.domain,
                    reason: format!("{} opens the existing domain panel.", command.label),
                }
            }
            other => WorkflowCommandResolution::Unsupported {
                reason: format!("Workflow command '{other}' is not supported by C-min resolver."),
            },
        }
    }

    pub fn resolve_parts(
        command_id: &str,
        payload_kind: &str,
        domain: WorkspaceDomainKind,
        availability: AuthoringCommandAvailability,
        label: &str,
    ) -> WorkflowCommandResolution {
        let command = AuthoringCommand::new(command_id, domain, label, availability, payload_kind);
        Self::resolve(&command)
    }
}
