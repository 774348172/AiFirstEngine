use engine_runtime::aui::{
    AuiActionRef, AuiCanvas, AuiInteractionConfig, AuiInteractionState, AuiInteractionSystem,
    AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect, AuiScrollState,
};
use engine_runtime::aui_virtual_list::{
    AuiVirtualListAlignment, AuiVirtualListEngine, AuiVirtualListItem, AuiVirtualListNavigation,
    AuiVirtualListSnapshot, AuiVirtualListSpec, AuiVirtualListState,
};
use engine_runtime::input_action::PointerPosition;
use engine_runtime::input_mapping::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};

fn frame(events: Vec<RuntimeInputEvent>, x: f32, y: f32) -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(1, "game-view");
    frame.events = events;
    frame.pointer_position = Some(PointerPosition { x, y });
    frame
}

fn nested_scroll_document() -> engine_runtime::aui::AuiDocument {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["outer"]);
    let outer = AuiNode::new(
        "outer",
        AuiNodeKind::ScrollView,
        AuiRect::fixed_position(0.0, 0.0, 160.0, 120.0),
    )
    .with_parent("root")
    .with_children(["inner", "outer_tail"]);
    let inner = AuiNode::new(
        "inner",
        AuiNodeKind::ScrollView,
        AuiRect::fixed_position(0.0, 0.0, 140.0, 80.0),
    )
    .with_parent("outer")
    .with_children(["inner_content"]);
    let inner_content = AuiNode::new(
        "inner_content",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 120.0, 240.0),
    )
    .with_parent("inner");
    let outer_tail = AuiNode::new(
        "outer_tail",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 300.0, 120.0, 40.0),
    )
    .with_parent("outer");
    engine_runtime::aui::AuiDocument::new(
        "nested-scroll",
        vec![AuiCanvas::screen_overlay("main", 320.0, 240.0, "root")],
        vec![root, outer, inner, inner_content, outer_tail],
    )
}

#[test]
fn nested_wheel_consumes_inner_then_hands_remaining_delta_to_parent_at_boundary() {
    let document = nested_scroll_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();

    let first = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame(
            vec![RuntimeInputEvent::MouseWheel { delta: -1.0 }],
            20.0,
            20.0,
        ),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(first.scroll_offset_change_count, 1);
    assert_eq!(state.scroll_offsets["inner"].offset_y, 48.0);
    assert!(!state.scroll_offsets.contains_key("outer"));

    let inner_max = state.scroll_offsets["inner"].max_offset_y;
    state.scroll_offsets.insert(
        "inner".into(),
        AuiScrollState {
            node_id: "inner".into(),
            offset_y: inner_max - 12.0,
            max_offset_y: inner_max,
            last_delta_y: 0.0,
        },
    );
    let at_boundary = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame(
            vec![RuntimeInputEvent::MouseWheel { delta: -1.0 }],
            20.0,
            20.0,
        ),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(at_boundary.scroll_offset_change_count, 2);
    assert_eq!(state.scroll_offsets["inner"].offset_y, inner_max);
    assert_eq!(state.scroll_offsets["outer"].offset_y, 36.0);
    assert!(at_boundary
        .commands
        .iter()
        .any(|command| command.source_node == "inner"));
    assert!(at_boundary
        .commands
        .iter()
        .any(|command| command.source_node == "outer"));
}

fn items(count: usize, extent: Option<f32>) -> AuiVirtualListSnapshot {
    AuiVirtualListSnapshot::new(
        (0..count)
            .map(|index| AuiVirtualListItem::new(format!("item-{index}"), extent))
            .collect(),
    )
}

#[test]
fn ten_thousand_items_materialize_only_viewport_and_overscan() {
    let spec = AuiVirtualListSpec::fixed("inventory", 240.0, 24.0)
        .with_overscan(48.0)
        .with_max_live_instances(32);
    let snapshot = items(10_000, None);
    let mut state = AuiVirtualListState::default();
    let mut interaction = AuiInteractionState::default();

    let window =
        AuiVirtualListEngine::update(&spec, &snapshot, &mut state, &mut interaction).unwrap();

    assert_eq!(window.total_item_count, 10_000);
    assert!(window.instances.len() <= 16);
    assert!(window.instances.len() <= spec.max_live_instances);
    assert_eq!(window.total_extent, 240_000.0);
    assert!(window
        .instances
        .iter()
        .all(|instance| instance.item_key.starts_with("item-")));
}

#[test]
fn duplicate_keys_and_invalid_extents_fail_without_mutating_state() {
    let spec = AuiVirtualListSpec::variable("inventory", 100.0, 20.0);
    let snapshot = AuiVirtualListSnapshot::new(vec![
        AuiVirtualListItem::new("same", Some(20.0)),
        AuiVirtualListItem::new("same", Some(0.0)),
    ]);
    let mut state = AuiVirtualListState::default();
    state.scroll_offset = 12.0;
    let before = serde_json::to_vec(&state).unwrap();
    let mut interaction = AuiInteractionState::default();

    let error =
        AuiVirtualListEngine::update(&spec, &snapshot, &mut state, &mut interaction).unwrap_err();

    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.virtual_list.duplicate_key"));
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.virtual_list.invalid_extent"));
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
}

#[test]
fn variable_height_update_preserves_anchor_and_supports_scroll_to_item() {
    let spec = AuiVirtualListSpec::variable("inventory", 60.0, 20.0)
        .with_overscan(0.0)
        .with_max_live_instances(16);
    let initial = items(20, Some(20.0));
    let mut state = AuiVirtualListState::default();
    state.scroll_offset = 45.0;
    let mut interaction = AuiInteractionState::default();
    let first =
        AuiVirtualListEngine::update(&spec, &initial, &mut state, &mut interaction).unwrap();
    let anchor = first.anchor.clone().unwrap();
    assert_eq!(anchor.item_key, "item-2");
    assert_eq!(anchor.local_offset, 5.0);

    let mut changed = items(20, Some(20.0));
    changed.items[0].measured_extent = Some(40.0);
    let second =
        AuiVirtualListEngine::update(&spec, &changed, &mut state, &mut interaction).unwrap();
    assert_eq!(state.scroll_offset, 65.0);
    assert_eq!(second.anchor.unwrap().item_key, "item-2");

    let target = AuiVirtualListEngine::scroll_to_item(
        &spec,
        &changed,
        &mut state,
        &mut interaction,
        "item-10",
        AuiVirtualListAlignment::Center,
    )
    .unwrap();
    assert!(target
        .instances
        .iter()
        .any(|instance| instance.item_key == "item-10"));
    assert_eq!(target.requested_key.as_deref(), Some("item-10"));
}

#[test]
fn logical_navigation_uses_item_keys_and_reveals_offscreen_target() {
    let spec = AuiVirtualListSpec::fixed("inventory", 60.0, 20.0)
        .with_overscan(0.0)
        .with_max_live_instances(8);
    let snapshot = items(100, None);
    let mut state = AuiVirtualListState::default();
    state.focused_key = Some("item-50".into());
    let mut interaction = AuiInteractionState::default();

    let window = AuiVirtualListEngine::navigate(
        &spec,
        &snapshot,
        &mut state,
        &mut interaction,
        AuiVirtualListNavigation::Next,
    )
    .unwrap();

    assert_eq!(state.focused_key.as_deref(), Some("item-51"));
    assert!(window
        .instances
        .iter()
        .any(|instance| instance.item_key == "item-51"));
    assert_eq!(
        window.focused_instance_node_id.as_deref(),
        window
            .instances
            .iter()
            .find(|instance| instance.item_key == "item-51")
            .map(|instance| instance.node_id.as_str())
    );
}

#[test]
fn recycled_instance_clears_pointer_capture_focus_and_pending_click() {
    let spec = AuiVirtualListSpec::fixed("inventory", 40.0, 20.0)
        .with_overscan(0.0)
        .with_max_live_instances(2);
    let snapshot = items(100, None);
    let mut virtual_state = AuiVirtualListState::default();
    let mut interaction = AuiInteractionState::default();
    let first =
        AuiVirtualListEngine::update(&spec, &snapshot, &mut virtual_state, &mut interaction)
            .unwrap();
    let first_node = first.instances[0].node_id.clone();
    interaction.focus.focused_node = Some(first_node.clone());

    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children([first_node.as_str()]);
    let button = AuiNode::new(
        first_node.clone(),
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 100.0, 20.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_action(AuiActionRef::click("ui.activate_item"));
    let document = engine_runtime::aui::AuiDocument::new(
        "virtual-recycle",
        vec![AuiCanvas::screen_overlay("main", 100.0, 40.0, "root")],
        vec![root, button],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    let down = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame(
            vec![RuntimeInputEvent::PointerDown {
                x: 10.0,
                y: 10.0,
                button: RuntimePointerButton::Primary,
            }],
            10.0,
            10.0,
        ),
        &mut interaction,
        AuiInteractionConfig::default(),
    );
    assert!(down.consumed);

    virtual_state.scroll_offset = 1_000.0;
    let moved =
        AuiVirtualListEngine::update(&spec, &snapshot, &mut virtual_state, &mut interaction)
            .unwrap();
    assert!(moved.reconciliation.rebound_instance_count > 0);
    assert!(moved.reconciliation.cleared_pointer_capture_count > 0);
    assert!(moved.reconciliation.cleared_focus_count > 0);
    assert!(interaction.focus.focused_node.is_none());

    let up = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame(
            vec![RuntimeInputEvent::PointerUp {
                x: 10.0,
                y: 10.0,
                button: RuntimePointerButton::Primary,
            }],
            10.0,
            10.0,
        ),
        &mut interaction,
        AuiInteractionConfig::default(),
    );
    assert!(up.actions.is_empty());
    assert!(!up.commands.iter().any(|command| {
        matches!(
            command.command_kind,
            engine_runtime::aui::AuiCommandKind::Click
        )
    }));
}
