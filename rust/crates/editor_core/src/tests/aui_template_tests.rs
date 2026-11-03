use super::*;
use engine_runtime::aui::{
    AuiActionEvent, AuiActionRef, AuiBindingRef, AuiBindingTarget, AuiBindingValue, AuiNode,
    AuiNodeKind, AuiRect,
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
    let mut legacy_value = serde_json::to_value(&asset).unwrap();
    legacy_value["schema_version"] = serde_json::json!("aui-template-asset.v1");
    let legacy: AuiTemplateAsset = serde_json::from_value(legacy_value).unwrap();
    assert!(legacy.validate().is_empty());
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
        parameters: Vec::new(),
        slot_fills: Vec::new(),
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
        parameters: Vec::new(),
        slot_fills: Vec::new(),
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
fn instantiate_composable_template_applies_typed_parameters_slots_parts_and_source_map() {
    let dir = temp_project_path("composable");
    let path = dir
        .join("AUI")
        .join("Templates")
        .join("slot.aui-template.json");
    let mut asset = AuiTemplateAsset::from_document_subtree(
        &source_document(),
        "AUI/equipment.aui.json",
        &path,
        "slot",
        "equipment_slot",
        "Equipment Slot",
        123,
    )
    .unwrap();
    asset.parameters = vec![
        AuiTemplateParameterDefinition {
            parameter_id: "title".to_string(),
            parameter_type: AuiTemplateParameterType::String,
            required: true,
            default_value: None,
        },
        AuiTemplateParameterDefinition {
            parameter_id: "icon_asset".to_string(),
            parameter_type: AuiTemplateParameterType::AssetRef,
            required: false,
            default_value: Some(AuiTemplateParameterValue::AssetRef(
                "tex-default".to_string(),
            )),
        },
        AuiTemplateParameterDefinition {
            parameter_id: "binding_path".to_string(),
            parameter_type: AuiTemplateParameterType::String,
            required: true,
            default_value: None,
        },
        AuiTemplateParameterDefinition {
            parameter_id: "icon_binding_path".to_string(),
            parameter_type: AuiTemplateParameterType::String,
            required: true,
            default_value: None,
        },
        AuiTemplateParameterDefinition {
            parameter_id: "action_id".to_string(),
            parameter_type: AuiTemplateParameterType::String,
            required: true,
            default_value: None,
        },
    ];
    asset.parameter_bindings = vec![
        AuiTemplateParameterBinding {
            parameter_id: "title".to_string(),
            node_id: "label".to_string(),
            target: AuiTemplateParameterTarget::Text,
        },
        AuiTemplateParameterBinding {
            parameter_id: "icon_asset".to_string(),
            node_id: "icon".to_string(),
            target: AuiTemplateParameterTarget::ImageAssetId,
        },
        AuiTemplateParameterBinding {
            parameter_id: "binding_path".to_string(),
            node_id: "label".to_string(),
            target: AuiTemplateParameterTarget::BindingPath {
                binding_id: "bind.name".to_string(),
            },
        },
        AuiTemplateParameterBinding {
            parameter_id: "icon_binding_path".to_string(),
            node_id: "icon".to_string(),
            target: AuiTemplateParameterTarget::BindingPath {
                binding_id: "bind.icon".to_string(),
            },
        },
        AuiTemplateParameterBinding {
            parameter_id: "action_id".to_string(),
            node_id: "slot".to_string(),
            target: AuiTemplateParameterTarget::ActionId {
                event: AuiActionEvent::Click,
            },
        },
    ];
    asset.slots = vec![AuiTemplateSlotDefinition {
        slot_id: "content".to_string(),
        host_node_id: "slot".to_string(),
        required: true,
        max_children: Some(1),
        allowed_kinds: vec![AuiNodeKind::Text],
    }];
    asset.named_parts = vec![
        AuiTemplateNamedPartDefinition {
            part_name: "root".to_string(),
            node_id: "slot".to_string(),
            required_kind: Some(AuiNodeKind::Button),
        },
        AuiTemplateNamedPartDefinition {
            part_name: "label".to_string(),
            node_id: "label".to_string(),
            required_kind: Some(AuiNodeKind::Text),
        },
    ];

    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["external_content"]);
    let content = AuiNode::new(
        "external_content",
        AuiNodeKind::Text,
        AuiRect::fixed_position(0.0, 0.0, 40.0, 20.0),
    )
    .with_parent("root")
    .with_text("project content");
    let mut target = engine_runtime::aui::AuiDocument::new(
        "target",
        vec![engine_runtime::aui::AuiCanvas::screen_overlay(
            "main", 1280.0, 720.0, "root",
        )],
        vec![root, content],
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
        parameters: vec![
            AuiTemplateParameterAssignment::new(
                "title",
                AuiTemplateParameterValue::String("Potion".to_string()),
            ),
            AuiTemplateParameterAssignment::new(
                "binding_path",
                AuiTemplateParameterValue::String("inventory.active.name".to_string()),
            ),
            AuiTemplateParameterAssignment::new(
                "icon_binding_path",
                AuiTemplateParameterValue::String("inventory.active.icon".to_string()),
            ),
            AuiTemplateParameterAssignment::new(
                "action_id",
                AuiTemplateParameterValue::String("ui.inspect_item".to_string()),
            ),
        ],
        slot_fills: vec![AuiTemplateSlotFill {
            slot_id: "content".to_string(),
            node_ids: vec!["external_content".to_string()],
        }],
    };

    let report = AuiTemplateWorkflow::instantiate_into_document(&asset, &request, &mut target);

    assert_eq!(report.status, AuiTemplateOperationStatus::Passed);
    assert_eq!(report.inserted_node_count, 3);
    assert_eq!(report.source_map.len(), 3);
    assert!(report.source_map.iter().any(|entry| {
        entry.template_id == "equipment_slot"
            && entry.instance_id == "slot_01"
            && entry.source_document_path == "AUI/equipment.aui.json"
            && entry.source_node_id == "label"
            && entry.inserted_node_id == "slot01_label"
    }));
    assert_eq!(report.named_parts.len(), 2);
    assert!(report
        .named_parts
        .iter()
        .any(|part| { part.part_name == "label" && part.inserted_node_id == "slot01_label" }));
    let label = target
        .nodes
        .iter()
        .find(|node| node.node_id == "slot01_label")
        .unwrap();
    assert_eq!(label.text.as_deref(), Some("Potion"));
    assert_eq!(label.binding_refs[0].path, "inventory.active.name");
    let icon = target
        .nodes
        .iter()
        .find(|node| node.node_id == "slot01_icon")
        .unwrap();
    assert_eq!(icon.image.as_ref().unwrap().asset_id, "tex-default");
    let slot = target
        .nodes
        .iter()
        .find(|node| node.node_id == "slot01_slot")
        .unwrap();
    assert_eq!(slot.action_refs[0].action_id, "ui.inspect_item");
    assert_eq!(
        slot.children,
        vec!["slot01_icon", "slot01_label", "external_content"]
    );
    let content = target
        .nodes
        .iter()
        .find(|node| node.node_id == "external_content")
        .unwrap();
    assert_eq!(content.parent.as_deref(), Some("slot01_slot"));
    assert!(!target
        .nodes
        .iter()
        .find(|node| node.node_id == "root")
        .unwrap()
        .children
        .contains(&"external_content".to_string()));
    assert!(!report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.ends_with("unparameterized")));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn composable_template_validation_failures_are_atomic() {
    let dir = temp_project_path("atomic");
    let path = dir
        .join("AUI")
        .join("Templates")
        .join("slot.aui-template.json");
    let base_asset = AuiTemplateAsset::from_document_subtree(
        &source_document(),
        "AUI/equipment.aui.json",
        &path,
        "slot",
        "equipment_slot",
        "Equipment Slot",
        123,
    )
    .unwrap();
    let make_target = || {
        engine_runtime::aui::AuiDocument::new(
            "target",
            vec![engine_runtime::aui::AuiCanvas::screen_overlay(
                "main", 1280.0, 720.0, "root",
            )],
            vec![AuiNode::new(
                "root",
                AuiNodeKind::Panel,
                AuiRect::stretch_full(),
            )],
        )
    };
    let make_request = |asset: &AuiTemplateAsset| AuiTemplateInstantiateRequest {
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
        parameters: Vec::new(),
        slot_fills: Vec::new(),
    };

    let mut cases = Vec::new();
    let mut missing_parameter = base_asset.clone();
    missing_parameter.parameters = vec![AuiTemplateParameterDefinition {
        parameter_id: "title".to_string(),
        parameter_type: AuiTemplateParameterType::String,
        required: true,
        default_value: None,
    }];
    cases.push((
        missing_parameter,
        "aui_template.required_parameter_missing",
        None,
    ));

    let mut wrong_type = base_asset.clone();
    wrong_type.parameters = vec![AuiTemplateParameterDefinition {
        parameter_id: "title".to_string(),
        parameter_type: AuiTemplateParameterType::String,
        required: true,
        default_value: None,
    }];
    cases.push((
        wrong_type,
        "aui_template.parameter_type_mismatch",
        Some(AuiTemplateParameterAssignment::new(
            "title",
            AuiTemplateParameterValue::Boolean(true),
        )),
    ));

    let mut wrong_default = base_asset.clone();
    wrong_default.parameters = vec![AuiTemplateParameterDefinition {
        parameter_id: "title".to_string(),
        parameter_type: AuiTemplateParameterType::String,
        required: false,
        default_value: Some(AuiTemplateParameterValue::Boolean(true)),
    }];
    cases.push((
        wrong_default,
        "aui_template.default_parameter_type_mismatch",
        None,
    ));

    let mut missing_target = base_asset.clone();
    missing_target.parameters = vec![AuiTemplateParameterDefinition {
        parameter_id: "path".to_string(),
        parameter_type: AuiTemplateParameterType::String,
        required: false,
        default_value: Some(AuiTemplateParameterValue::String("ui.path".to_string())),
    }];
    missing_target.parameter_bindings = vec![AuiTemplateParameterBinding {
        parameter_id: "path".to_string(),
        node_id: "label".to_string(),
        target: AuiTemplateParameterTarget::BindingPath {
            binding_id: "missing.binding".to_string(),
        },
    }];
    cases.push((
        missing_target,
        "aui_template.parameter_target_field_missing",
        None,
    ));

    let mut missing_slot = base_asset.clone();
    missing_slot.slots = vec![AuiTemplateSlotDefinition {
        slot_id: "content".to_string(),
        host_node_id: "slot".to_string(),
        required: true,
        max_children: Some(1),
        allowed_kinds: Vec::new(),
    }];
    cases.push((missing_slot, "aui_template.required_slot_missing", None));

    let mut missing_part = base_asset;
    missing_part.named_parts = vec![AuiTemplateNamedPartDefinition {
        part_name: "required_label".to_string(),
        node_id: "missing".to_string(),
        required_kind: Some(AuiNodeKind::Text),
    }];
    cases.push((missing_part, "aui_template.named_part_node_missing", None));

    for (asset, expected_code, assignment) in cases {
        let mut target = make_target();
        let before = serde_json::to_vec(&target).unwrap();
        let mut request = make_request(&asset);
        if let Some(assignment) = assignment {
            request.parameters.push(assignment);
        }
        let report = AuiTemplateWorkflow::instantiate_into_document(&asset, &request, &mut target);
        assert_eq!(report.status, AuiTemplateOperationStatus::Failed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected_code));
        assert_eq!(serde_json::to_vec(&target).unwrap(), before);
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn template_source_mapping_is_stable_when_sibling_order_changes() {
    let dir = temp_project_path("stable_mapping");
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
    let mut reordered = asset.clone();
    reordered.nodes.reverse();
    reordered
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "slot")
        .unwrap()
        .children
        .reverse();
    let request = AuiTemplateInstantiateRequest {
        template_ref: AuiTemplateRef {
            asset_guid: asset.asset_guid.clone(),
            template_id: asset.template_id.clone(),
            asset_path: "AUI/Templates/slot.aui-template.json".to_string(),
        },
        target_document_path: "AUI/hud.aui.json".to_string(),
        parent_node_id: "root".to_string(),
        insertion_index: None,
        instance_id: "stable_instance".to_string(),
        node_id_prefix: "stable".to_string(),
        parameters: Vec::new(),
        slot_fills: Vec::new(),
    };
    let make_target = || {
        engine_runtime::aui::AuiDocument::new(
            "target",
            vec![engine_runtime::aui::AuiCanvas::screen_overlay(
                "main", 1280.0, 720.0, "root",
            )],
            vec![AuiNode::new(
                "root",
                AuiNodeKind::Panel,
                AuiRect::stretch_full(),
            )],
        )
    };
    let mut first_target = make_target();
    let mut second_target = make_target();
    let first = AuiTemplateWorkflow::instantiate_into_document(&asset, &request, &mut first_target);
    let second =
        AuiTemplateWorkflow::instantiate_into_document(&reordered, &request, &mut second_target);
    let mut first_map = first
        .source_map
        .into_iter()
        .map(|entry| (entry.source_node_id, entry.inserted_node_id))
        .collect::<Vec<_>>();
    let mut second_map = second
        .source_map
        .into_iter()
        .map(|entry| (entry.source_node_id, entry.inserted_node_id))
        .collect::<Vec<_>>();
    first_map.sort();
    second_map.sort();
    assert_eq!(first_map, second_map);
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
