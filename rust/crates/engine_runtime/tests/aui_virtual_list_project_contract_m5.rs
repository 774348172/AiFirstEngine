use engine_runtime::aui::{
    AuiAction, AuiActionEvent, AuiActionRef, AuiCanvas, AuiDocument, AuiInteractionState,
    AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect, AuiScrollState, ProjectUiStateSnapshot,
};
use engine_runtime::aui_layout_schema::{LayoutVisibility, AUI_LAYOUT_SCHEMA_VERSION};
use engine_runtime::aui_virtual_list::{
    AuiVirtualListActionPayload, AuiVirtualListDeclaration, AuiVirtualListExtentMode,
    AuiVirtualListItem, AuiVirtualListPresentState, AuiVirtualListSnapshot,
};
use project_runtime_sdk::{
    ProjectRuntimeUiStateIdentity, ProjectRuntimeUiStateResolveOutput,
    ProjectRuntimeVirtualListItem, ProjectRuntimeVirtualListSnapshot,
};
use std::collections::{BTreeMap, HashMap};

fn document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["inventory"]);
    let inventory = AuiNode::new(
        "inventory",
        AuiNodeKind::List,
        AuiRect::fixed_position(20.0, 20.0, 400.0, 240.0),
    )
    .with_parent("root")
    .with_children(["item-template"]);
    let mut template = AuiNode::new(
        "item-template",
        AuiNodeKind::Text,
        AuiRect::fixed_position(0.0, 0.0, 400.0, 48.0),
    )
    .with_parent("inventory");
    template.layout_visibility = Some(LayoutVisibility::Collapsed);
    let mut document = AuiDocument::new(
        "virtual-list-project-contract",
        vec![AuiCanvas::screen_overlay("main", 640.0, 480.0, "root")],
        vec![root, inventory, template],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document.virtual_lists.push(AuiVirtualListDeclaration {
        list_node_id: "inventory".into(),
        data_path: "inventory.items".into(),
        item_template_node_id: "item-template".into(),
        overscan: 96.0,
        max_live_instances: 24,
        extent_mode: AuiVirtualListExtentMode::Variable {
            estimated_item_extent: 48.0,
        },
    });
    document
}

#[test]
fn virtual_list_declaration_and_snapshot_are_typed_and_roundtrip() {
    let document = document();
    assert!(AuiLayoutEngine::validate(&document, None).ok);
    let json = serde_json::to_value(&document).unwrap();
    assert_eq!(json["virtualLists"][0]["dataPath"], "inventory.items");
    let roundtrip: AuiDocument = serde_json::from_value(json).unwrap();
    assert_eq!(roundtrip.virtual_lists, document.virtual_lists);

    let list = AuiVirtualListSnapshot::new(vec![
        AuiVirtualListItem::new("item-1", Some(48.0)).with_text("能源电池 / الطاقة")
    ]);
    let mut snapshot = ProjectUiStateSnapshot::new(7);
    snapshot
        .virtual_lists
        .insert("inventory.items".into(), list.clone());
    let decoded: ProjectUiStateSnapshot =
        serde_json::from_value(serde_json::to_value(&snapshot).unwrap()).unwrap();
    assert_eq!(decoded.virtual_lists["inventory.items"], list);

    let sdk = ProjectRuntimeUiStateResolveOutput::ReplaceWithVirtualLists {
        identity: ProjectRuntimeUiStateIdentity {
            producer_epoch: 1,
            visible_revision: 2,
            binding_set_digest: "sha256:test".into(),
        },
        producer_id: "project.ui".into(),
        values: BTreeMap::new(),
        virtual_lists: BTreeMap::from([(
            "inventory.items".into(),
            ProjectRuntimeVirtualListSnapshot {
                items: vec![ProjectRuntimeVirtualListItem {
                    item_key: "item-1".into(),
                    text: "能源电池 / الطاقة".into(),
                    measured_extent: Some(48.0),
                }],
            },
        )]),
    };
    let decoded: ProjectRuntimeUiStateResolveOutput =
        serde_json::from_value(serde_json::to_value(&sdk).unwrap()).unwrap();
    assert_eq!(decoded, sdk);
}

#[test]
fn invalid_virtual_list_declarations_fail_closed() {
    let mut invalid_kind = document();
    invalid_kind.nodes[1].kind = AuiNodeKind::Panel;
    let report = AuiLayoutEngine::validate(&invalid_kind, None);
    assert!(!report.ok);
    assert!(report
        .report_items
        .iter()
        .any(|item| item.code == "aui_virtual_list_declaration_invalid"));

    let mut duplicate_key_contract = document();
    duplicate_key_contract
        .virtual_lists
        .push(duplicate_key_contract.virtual_lists[0].clone());
    let report = AuiLayoutEngine::validate(&duplicate_key_contract, None);
    assert!(!report.ok);

    let mut visible_template = document();
    visible_template.nodes[2].layout_visibility = Some(LayoutVisibility::Visible);
    assert!(!AuiLayoutEngine::validate(&visible_template, None).ok);
}

#[test]
fn materialized_item_actions_select_the_current_stable_key_after_slot_rebind() {
    let mut document = document();
    let template = document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "item-template")
        .unwrap();
    template.kind = AuiNodeKind::Button;
    template.interactable = true;
    template.action_refs = vec![AuiActionRef::activate("inventory.select")];
    let snapshot = AuiVirtualListSnapshot::new(
        (0..10_000)
            .map(|index| {
                AuiVirtualListItem::new(format!("item-{index:05}"), Some(48.0))
                    .with_text(format!("Item {index}"))
            })
            .collect(),
    );
    let snapshots = HashMap::from([("inventory.items".to_string(), snapshot)]);
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiVirtualListPresentState::default();
    let mut interaction = AuiInteractionState::default();

    let mut first = state
        .materialize(&document, &layout, &snapshots, &mut interaction)
        .unwrap();
    let first_context = first
        .document
        .nodes
        .iter()
        .find_map(|node| node.virtual_list_item.clone())
        .unwrap();
    let first_payload =
        serde_json::to_string(&AuiVirtualListActionPayload::from(&first_context)).unwrap();
    assert!(state.apply_actions(
        &[AuiAction {
            action_id: "inventory.select".into(),
            node_id: "inventory:virtual-item:0".into(),
            event: AuiActionEvent::Activate,
            payload: Some(first_payload),
        }],
        &mut first.windows,
    ));
    assert_eq!(
        first.windows[0].selected_key.as_deref(),
        Some(first_context.item_key.as_str())
    );

    interaction.scroll_offsets.insert(
        "inventory".into(),
        AuiScrollState {
            node_id: "inventory".into(),
            offset_y: 48.0 * 5_000.0,
            max_offset_y: 48.0 * 10_000.0 - 240.0,
            last_delta_y: -1.0,
        },
    );
    let mut rebound = state
        .materialize(&document, &layout, &snapshots, &mut interaction)
        .unwrap();
    let rebound_context = rebound
        .document
        .nodes
        .iter()
        .find(|node| node.node_id == "inventory:virtual-item:0")
        .and_then(|node| node.virtual_list_item.clone())
        .unwrap();
    assert_ne!(rebound_context.item_key, first_context.item_key);
    let rebound_payload =
        serde_json::to_string(&AuiVirtualListActionPayload::from(&rebound_context)).unwrap();
    assert!(state.apply_actions(
        &[AuiAction {
            action_id: "inventory.select".into(),
            node_id: "inventory:virtual-item:0".into(),
            event: AuiActionEvent::Activate,
            payload: Some(rebound_payload),
        }],
        &mut rebound.windows,
    ));
    assert_eq!(
        rebound.windows[0].selected_key.as_deref(),
        Some(rebound_context.item_key.as_str())
    );
}
