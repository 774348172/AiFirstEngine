use super::*;
use engine_runtime::aui::{
    AuiActionRef, AuiBindingRef, AuiBindingTarget, AuiBindingValue, AuiNode, AuiNodeKind, AuiRect,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project_path(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("aif_editor_core_aui_template_{name}_{stamp}"))
}

fn source_document() -> engine_runtime::aui::AuiDocument {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["slot"]);
    let slot = AuiNode::new(
        "slot",
        AuiNodeKind::Button,
        AuiRect::fixed_position(10.0, 20.0, 96.0, 96.0),
    )
    .with_parent("root")
    .with_children(["icon", "label"])
    .with_action(AuiActionRef::click("ui.open_equipment_detail"));
    let icon = AuiNode::new(
        "icon",
        AuiNodeKind::Image,
        AuiRect::fixed_position(16.0, 16.0, 48.0, 48.0),
    )
    .with_parent("slot")
    .with_image("tex-sword")
    .with_binding(AuiBindingRef::new(
        "bind.icon",
        AuiBindingTarget::ImageAssetRef,
        "equipment.icon",
        Some(AuiBindingValue::AssetRef(
            engine_runtime::aui::AuiAssetRef::new("tex-sword"),
        )),
    ));
    let label = AuiNode::new(
        "label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(16.0, 70.0, 80.0, 20.0),
    )
    .with_parent("slot")
    .with_text("Sword")
    .with_binding(AuiBindingRef::new(
        "bind.name",
        AuiBindingTarget::TextText,
        "equipment.name",
        Some(AuiBindingValue::String("Sword".to_string())),
    ));
    engine_runtime::aui::AuiDocument::new(
        "equipment-ui",
        vec![engine_runtime::aui::AuiCanvas::screen_overlay(
            "main", 1280.0, 720.0, "root",
        )],
        vec![root, slot, icon, label],
    )
}

#[test]
fn aui_template_schema_roundtrip() {
    let dir = temp_project_path("schema");
    let path = dir
        .join("AUI")
        .join("Templates")
        .join("slot.aui-template.json");
    let document = source_document();
    let asset = AuiTemplateAsset::from_document_subtree(
        &document,
        "AUI/equipment.aui.json",
        &path,
        "slot",
        "equipment_slot",
        "Equipment Slot",
        123,
    )
    .unwrap();

    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    asset.save(&path).unwrap();
    let reopened = AuiTemplateAsset::open(&path).unwrap();

    assert_eq!(reopened.schema_version, AUI_TEMPLATE_ASSET_SCHEMA_VERSION);
    assert_eq!(reopened.template_id, "equipment_slot");
    assert!(reopened.asset_guid.starts_with("aui-template-"));
    assert_eq!(reopened.guid_source, "deterministic_path_hash");
    assert!(!reopened.asset_db_integrated);
    assert_eq!(reopened.nodes.len(), 3);
    assert_eq!(
        serde_json::to_string(&reopened).unwrap(),
        serde_json::to_string(&asset).unwrap()
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn save_aui_subtree_as_template_collects_dependencies() {
    let dir = temp_project_path("collect");
    let path = dir
        .join("AUI")
        .join("Templates")
        .join("slot.aui-template.json");
    let document = source_document();

    let asset = AuiTemplateAsset::from_document_subtree(
        &document,
        "AUI/equipment.aui.json",
        &path,
        "slot",
        "equipment_slot",
        "Equipment Slot",
        123,
    )
    .unwrap();

    assert_eq!(asset.root_node_id, "slot");
    assert_eq!(asset.nodes.len(), 3);
    assert_eq!(asset.asset_refs.len(), 1);
    assert_eq!(asset.binding_refs.len(), 2);
    assert_eq!(asset.action_refs.len(), 1);
    assert!(asset
        .binding_refs
        .iter()
        .any(|binding| binding.value == "equipment.icon"));
    assert!(asset
        .action_refs
        .iter()
        .any(|action| action.value == "ui.open_equipment_detail"));
    let root = asset
        .nodes
        .iter()
        .find(|node| node.node_id == "slot")
        .expect("template root");
    assert!(root.parent.is_none());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn instantiate_aui_template_remaps_node_ids() {
    let dir = temp_project_path("remap");
    let path = dir
        .join("AUI")
        .join("Templates")
        .join("slot.aui-template.json");
    let asset = AuiTemplateAsset::from_document_subtree(
        &source_document(),
        "AUI/equipment.aui.json",
        &path,
        "slot",
        "equipment_slot",
        "Equipment Slot",
        123,
    )
    .unwrap();
    let mut target = engine_runtime::aui::AuiDocument::new(
        "target",
        vec![engine_runtime::aui::AuiCanvas::screen_overlay(
            "main", 1280.0, 720.0, "root",
        )],
        vec![AuiNode::new(
            "root",
            AuiNodeKind::Panel,
            AuiRect::stretch_full(),
        )],
    );
    let request = AuiTemplateInstantiateRequest {
        template_ref: AuiTemplateRef {
            asset_guid: asset.asset_guid.clone(),
            template_id: asset.template_id.clone(),
            asset_path: "AUI/Templates/slot.aui-template.json".to_string(),
        },
        target_document_path: "AUI/hud.aui.json".to_string(),
        parent_node_id: "root".to_string(),
        insertion_index: None,
        instance_id: "slot_01".to_string(),
        node_id_prefix: "slot01".to_string(),
    };

    let report = AuiTemplateWorkflow::instantiate_into_document(&asset, &request, &mut target);

    assert_eq!(report.inserted_node_count, 3);
    assert_eq!(target.nodes.len(), 4);
    assert!(target
        .nodes
        .iter()
        .any(|node| node.node_id == "slot01_slot"));
    let root = target
        .nodes
        .iter()
        .find(|node| node.node_id == "root")
        .unwrap();
    assert_eq!(root.children, vec!["slot01_slot"]);
    let slot = target
        .nodes
        .iter()
        .find(|node| node.node_id == "slot01_slot")
        .unwrap();
    assert_eq!(slot.parent.as_deref(), Some("root"));
    assert_eq!(slot.children, vec!["slot01_icon", "slot01_label"]);
    assert!(report
        .node_id_remap
        .iter()
        .any(|remap| remap.source_node_id == "icon" && remap.inserted_node_id == "slot01_icon"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn instantiate_aui_template_reports_copied_dependencies() {
    let dir = temp_project_path("deps");
    let path = dir
        .join("AUI")
        .join("Templates")
        .join("slot.aui-template.json");
    let asset = AuiTemplateAsset::from_document_subtree(
        &source_document(),
        "AUI/equipment.aui.json",
        &path,
        "slot",
        "equipment_slot",
        "Equipment Slot",
        123,
    )
    .unwrap();
    let mut target = engine_runtime::aui::AuiDocument::new(
        "target",
        vec![engine_runtime::aui::AuiCanvas::screen_overlay(
            "main", 1280.0, 720.0, "root",
        )],
        vec![AuiNode::new(
            "root",
            AuiNodeKind::Panel,
            AuiRect::stretch_full(),
        )],
    );
    let request = AuiTemplateInstantiateRequest {
        template_ref: AuiTemplateRef {
            asset_guid: asset.asset_guid.clone(),
            template_id: asset.template_id.clone(),
            asset_path: "AUI/Templates/slot.aui-template.json".to_string(),
        },
        target_document_path: "AUI/hud.aui.json".to_string(),
        parent_node_id: "root".to_string(),
        insertion_index: None,
        instance_id: "slot_01".to_string(),
        node_id_prefix: "slot01".to_string(),
    };

    let report = AuiTemplateWorkflow::instantiate_into_document(&asset, &request, &mut target);

    assert_eq!(report.status, AuiTemplateOperationStatus::Partial);
    assert_eq!(report.copied_binding_refs.len(), 2);
    assert_eq!(report.copied_action_refs.len(), 1);
    assert_eq!(report.copied_asset_refs.len(), 1);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "aui_template.binding_refs_unparameterized"));
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "aui_template.action_refs_unparameterized"));
    assert!(report
        .copied_binding_refs
        .iter()
        .any(|binding| binding.node_id == "slot01_icon" && binding.value == "equipment.icon"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn editor_session_saves_and_instantiates_aui_template() {
    let dir = temp_project_path("session");
    let mut session = EditorSession::new();
    let create_project =
        session.execute_command(command_for_test(UiCommandPayload::CreateProject {
            path: dir.display().to_string(),
            name: "AUI Template Project".to_string(),
        }));
    assert_eq!(create_project.status, CommandStatus::Committed);
    let create_doc =
        session.execute_command(command_for_test(UiCommandPayload::CreateAuiDocument {
            path: "AUI/hud.aui.json".to_string(),
            document_id: "hud".to_string(),
            width: 1280.0,
            height: 720.0,
        }));
    assert_eq!(create_doc.status, CommandStatus::Committed);
    let add = session.execute_command(command_for_test(UiCommandPayload::AddAuiNode {
        path: "AUI/hud.aui.json".to_string(),
        parent_node_id: "root".to_string(),
        node_id: "score_label".to_string(),
        kind: "text".to_string(),
        name: "Score Label".to_string(),
        rect: serde_json::json!({"x": 10.0, "y": 10.0, "width": 120.0, "height": 24.0}),
    }));
    assert_eq!(add.status, CommandStatus::Committed);
    let set_text = session.execute_command(command_for_test(UiCommandPayload::SetAuiNodeField {
        path: "AUI/hud.aui.json".to_string(),
        node_id: "score_label".to_string(),
        schema_path: "text".to_string(),
        value: serde_json::json!("Score"),
    }));
    assert_eq!(set_text.status, CommandStatus::Committed);

    let save_template = session.execute_command(command_for_test(
        UiCommandPayload::SaveAuiSubtreeAsTemplate {
            document_path: "AUI/hud.aui.json".to_string(),
            root_node_id: "score_label".to_string(),
            template_asset_path: "AUI/Templates/score.aui-template.json".to_string(),
            template_id: "score_label_template".to_string(),
            display_name: "Score Label".to_string(),
        },
    ));
    assert_eq!(save_template.status, CommandStatus::Committed);
    assert!(dir
        .join("AUI")
        .join("Templates")
        .join("score.aui-template.json")
        .exists());

    let instantiate =
        session.execute_command(command_for_test(UiCommandPayload::InstantiateAuiTemplate {
            template_asset_path: "AUI/Templates/score.aui-template.json".to_string(),
            template_id: "score_label_template".to_string(),
            target_document_path: "AUI/hud.aui.json".to_string(),
            parent_node_id: "root".to_string(),
            insertion_index: None,
            instance_id: "score_copy_instance".to_string(),
            node_id_prefix: "score_copy".to_string(),
        }));
    assert_eq!(instantiate.status, CommandStatus::Committed);

    let reopened = AuiAuthoringService::open(&dir.join("AUI").join("hud.aui.json")).unwrap();
    assert!(reopened
        .document()
        .nodes
        .iter()
        .any(|node| node.node_id == "score_copy_score_label"));
    let _ = fs::remove_dir_all(dir);
}
