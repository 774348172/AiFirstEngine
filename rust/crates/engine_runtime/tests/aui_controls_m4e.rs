use engine_input::{RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton};
use engine_runtime::aui::*;
use engine_runtime::aui_layout_runtime::{
    AuiIntrinsicMeasure, AuiLayoutSolver, AuiMeasureInput, AuiMeasureOutput,
};
use engine_runtime::aui_layout_schema::{
    Dimension, LayoutMode, LayoutSpec, Participation, SlotSpec, AUI_LAYOUT_SCHEMA_VERSION,
};
use engine_runtime::aui_virtual_list::{
    AuiVirtualListEngine, AuiVirtualListItem, AuiVirtualListSnapshot, AuiVirtualListSpec,
    AuiVirtualListState,
};

fn fixed(width: f32, height: f32) -> SlotSpec {
    SlotSpec {
        width: Dimension::Fixed { value: width },
        height: Dimension::Fixed { value: height },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

fn absolute(width: f32, height: f32) -> SlotSpec {
    SlotSpec {
        participation: Participation::OutOfFlow,
        ..fixed(width, height)
    }
}

fn control_document() -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["toggle", "slider"]);
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    let mut toggle = AuiNode::new(
        "toggle",
        AuiNodeKind::Toggle,
        AuiRect::fixed_position(0.0, 0.0, 100.0, 40.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_toggle_checked(false)
    .with_action(AuiActionRef::value_changed("settings.sound"));
    toggle.slot = Some(absolute(100.0, 40.0));
    let mut slider = AuiNode::new(
        "slider",
        AuiNodeKind::Slider,
        AuiRect::fixed_position(0.0, 60.0, 100.0, 40.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_slider(0.25, 0.0, 1.0, 0.1)
    .with_action(AuiActionRef::value_changed("settings.volume"));
    slider.slot = Some(absolute(100.0, 40.0));
    let mut document = AuiDocument::new(
        "controls-m4e",
        vec![AuiCanvas::screen_overlay("main", 200.0, 140.0, "root")],
        vec![root, toggle, slider],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

fn frame(frame_id: u64, events: Vec<RuntimeInputEvent>) -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(frame_id, "game-view");
    frame.events = events;
    frame
}

#[test]
fn typed_toggle_slider_schema_and_bindings_fail_closed() {
    let document = control_document();
    assert!(AuiLayoutEngine::validate(&document, None).ok);
    let json = serde_json::to_string(&document).unwrap();
    let roundtrip: AuiDocument = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.nodes[1].toggle_checked, Some(false));
    assert_eq!(roundtrip.nodes[2].slider_value, Some(0.25));

    let mut bound = document.clone();
    bound.nodes[1].binding_refs.push(AuiBindingRef::new(
        "sound",
        AuiBindingTarget::ToggleChecked,
        "settings.sound",
        None,
    ));
    bound.nodes[2].binding_refs.push(AuiBindingRef::new(
        "volume",
        AuiBindingTarget::SliderValue,
        "settings.volume",
        None,
    ));
    let snapshot = ProjectUiStateSnapshot {
        frame_index: 1,
        values: [
            ("settings.sound".into(), AuiBindingValue::Bool(true)),
            ("settings.volume".into(), AuiBindingValue::Number(0.74)),
        ]
        .into_iter()
        .collect(),
        virtual_lists: Default::default(),
    };
    let (resolved, report) = AuiRuntimeResolver::resolve_bindings(&bound, &snapshot);
    assert_eq!(report.resolved_count, 2);
    assert_eq!(resolved.nodes[1].toggle_checked, Some(true));
    assert_eq!(resolved.nodes[2].slider_value, Some(0.7));

    let mut invalid = document;
    invalid.nodes[1].slider_min = Some(2.0);
    invalid.nodes[1].slider_max = Some(1.0);
    let report = AuiLayoutEngine::validate(&invalid, None);
    assert!(!report.ok);
    assert!(report
        .report_items
        .iter()
        .any(|item| item.code == "aui_control_value_invalid"));
}

#[test]
fn legacy_v2_document_and_click_action_remain_compatible() {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["button"]);
    let button = AuiNode::new(
        "button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 100.0, 40.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_action(AuiActionRef::click("legacy.click"));
    let document = AuiDocument::new(
        "legacy-v2",
        vec![AuiCanvas::screen_overlay("main", 100.0, 40.0, "root")],
        vec![root, button],
    );
    let decoded: AuiDocument =
        serde_json::from_value(serde_json::to_value(document).unwrap()).unwrap();
    assert_eq!(decoded.schema_version, AUI_DOCUMENT_SCHEMA_VERSION);
    let layout = AuiLayoutEngine::layout(&decoded, 1);
    let result = AuiInteractionSystem::process(
        &decoded,
        &layout,
        &frame(
            1,
            vec![
                RuntimeInputEvent::PointerDown {
                    x: 20.0,
                    y: 20.0,
                    button: RuntimePointerButton::Primary,
                },
                RuntimeInputEvent::PointerUp {
                    x: 20.0,
                    y: 20.0,
                    button: RuntimePointerButton::Primary,
                },
            ],
        ),
    );
    assert!(result.actions.iter().any(|action| {
        action.action_id == "legacy.click" && action.event == AuiActionEvent::Click
    }));
}

#[test]
fn toggle_and_slider_share_pointer_keyboard_and_typed_value_payloads() {
    let document = control_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    assert!(layout.diagnostics.is_empty());
    let mut state = AuiInteractionState::default();

    let toggle = AuiInteractionSystem::process_session_with_state(
        &document,
        &layout,
        &frame(
            1,
            vec![
                RuntimeInputEvent::PointerDown {
                    x: 20.0,
                    y: 20.0,
                    button: RuntimePointerButton::Primary,
                },
                RuntimeInputEvent::PointerUp {
                    x: 20.0,
                    y: 20.0,
                    button: RuntimePointerButton::Primary,
                },
            ],
        ),
        &mut state,
        AuiInteractionConfig::default(),
        "session-a",
    );
    assert_eq!(state.toggle_value("toggle"), Some(true));
    let action = toggle
        .actions
        .iter()
        .find(|action| action.event == AuiActionEvent::ValueChanged)
        .unwrap();
    let payload: AuiControlValuePayload =
        serde_json::from_str(action.payload.as_deref().unwrap()).unwrap();
    assert_eq!(payload.value, AuiControlValue::Bool(true));

    let slider = AuiInteractionSystem::process_session_with_state(
        &document,
        &layout,
        &frame(
            2,
            vec![
                RuntimeInputEvent::PointerDown {
                    x: 76.0,
                    y: 80.0,
                    button: RuntimePointerButton::Primary,
                },
                RuntimeInputEvent::PointerUp {
                    x: 76.0,
                    y: 80.0,
                    button: RuntimePointerButton::Primary,
                },
                RuntimeInputEvent::KeyDown {
                    key: "ArrowRight".into(),
                },
            ],
        ),
        &mut state,
        AuiInteractionConfig::default(),
        "session-a",
    );
    assert_eq!(state.slider_value("slider"), Some(0.9));
    assert!(slider
        .actions
        .iter()
        .any(|action| action.event == AuiActionEvent::ValueChanged));

    AuiInteractionSystem::process_session_with_state(
        &document,
        &layout,
        &frame(3, Vec::new()),
        &mut state,
        AuiInteractionConfig::default(),
        "session-b",
    );
    assert_eq!(state.toggle_value("toggle"), Some(false));
    assert_eq!(state.slider_value("slider"), Some(0.3));
}

#[test]
fn composed_button_decorations_do_not_become_activation_targets() {
    let mut root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["button"]);
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    let mut button = AuiNode::new(
        "button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 180.0, 48.0),
    )
    .with_parent("root")
    .with_children(["icon", "label"])
    .with_interactable(true)
    .with_action(AuiActionRef::activate("menu.play"));
    button.slot = Some(absolute(180.0, 48.0));
    button.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        ..Default::default()
    });
    let mut icon = AuiNode::new(
        "icon",
        AuiNodeKind::Image,
        AuiRect::fixed_position(0.0, 0.0, 32.0, 32.0),
    )
    .with_parent("button");
    icon.slot = Some(fixed(32.0, 32.0));
    let mut label = AuiNode::new(
        "label",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(32.0, 0.0, 120.0, 32.0),
    )
    .with_parent("button");
    label.slot = Some(fixed(120.0, 32.0));
    let mut document = AuiDocument::new(
        "composed-button",
        vec![AuiCanvas::screen_overlay("main", 200.0, 80.0, "root")],
        vec![root, button, icon, label],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();
    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame(
            1,
            vec![
                RuntimeInputEvent::PointerDown {
                    x: 20.0,
                    y: 20.0,
                    button: RuntimePointerButton::Primary,
                },
                RuntimeInputEvent::PointerUp {
                    x: 20.0,
                    y: 20.0,
                    button: RuntimePointerButton::Primary,
                },
            ],
        ),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert!(
        result.actions.iter().any(|action| {
            action.node_id == "button"
                && action.action_id == "menu.play"
                && action.event == AuiActionEvent::Activate
        }),
        "diagnostics={:?}; layout={:?}; hit={:?}; commands={:?}; actions={:?}",
        layout.diagnostics,
        layout.computed_nodes,
        AuiInteractionSystem::hit_test(&document, &layout, 20.0, 20.0),
        result.commands,
        result.actions
    );
    assert!(result
        .commands
        .iter()
        .all(|command| command.source_node != "icon" && command.source_node != "label"));
}

#[test]
fn virtual_list_selection_is_stable_by_key_across_slot_recycling() {
    let spec = AuiVirtualListSpec::fixed("inventory", 60.0, 20.0).with_overscan(0.0);
    let snapshot = AuiVirtualListSnapshot::new(
        (0..10_000)
            .map(|index| AuiVirtualListItem::new(format!("item-{index}"), None))
            .collect(),
    );
    let mut state = AuiVirtualListState::default();
    let mut interaction = AuiInteractionState::default();
    let selected =
        AuiVirtualListEngine::select_item(&spec, &snapshot, &mut state, &mut interaction, "item-1")
            .unwrap();
    assert_eq!(selected.selected_key.as_deref(), Some("item-1"));
    let moved = AuiVirtualListEngine::scroll_to_item(
        &spec,
        &snapshot,
        &mut state,
        &mut interaction,
        "item-9000",
        engine_runtime::aui_virtual_list::AuiVirtualListAlignment::Start,
    )
    .unwrap();
    assert_eq!(moved.selected_key.as_deref(), Some("item-1"));
    assert!(moved.instances.len() <= 4);

    let without_selected = AuiVirtualListSnapshot::new(
        snapshot
            .items
            .iter()
            .filter(|item| item.item_key != "item-1")
            .cloned()
            .collect(),
    );
    let reconciled =
        AuiVirtualListEngine::update(&spec, &without_selected, &mut state, &mut interaction)
            .unwrap();
    assert!(reconciled.selected_key.is_none());
}

#[derive(Default)]
struct FixedMeasure;

impl AuiIntrinsicMeasure for FixedMeasure {
    fn measure(&mut self, _input: AuiMeasureInput<'_>) -> AuiMeasureOutput {
        AuiMeasureOutput {
            width: 10.0,
            height: 10.0,
            first_baseline: None,
        }
    }
}

#[test]
fn node_scale_matrix_records_cold_clean_local_and_full_invalidation_counts() {
    for node_count in [100usize, 1_000, 10_000] {
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.layout = Some(LayoutSpec {
            mode: LayoutMode::Column,
            ..Default::default()
        });
        root.children = (0..node_count)
            .map(|index| format!("item-{index}"))
            .collect();
        let mut nodes = Vec::with_capacity(node_count + 1);
        nodes.push(root);
        nodes.extend((0..node_count).map(|index| {
            let mut node = AuiNode::new(
                format!("item-{index}"),
                AuiNodeKind::Panel,
                AuiRect::fixed_position(0.0, 0.0, 10.0, 1.0),
            )
            .with_parent("root");
            node.slot = Some(fixed(10.0, 1.0));
            node
        }));
        let mut document = AuiDocument::new(
            format!("scale-{node_count}"),
            vec![AuiCanvas::screen_overlay("main", 100.0, 10_000.0, "root")],
            nodes,
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        let mut solver = AuiLayoutSolver::new();
        solver
            .solve(&document, "main", 1, &mut FixedMeasure)
            .unwrap();
        let cold = solver.last_stats();
        solver
            .solve(&document, "main", 1, &mut FixedMeasure)
            .unwrap();
        let clean = solver.last_stats();
        assert!(cold.visited_nodes >= node_count);
        assert!(clean.clean_snapshot_reused);
        assert_eq!(clean.measure_calls, 0);

        solver.mark_dirty("item-0").unwrap();
        solver
            .solve(&document, "main", 1, &mut FixedMeasure)
            .unwrap();
        let local = solver.last_stats();
        assert!(!local.clean_snapshot_reused);

        document.nodes[0].layout.as_mut().unwrap().gap.row = 1.0;
        solver
            .solve(&document, "main", 2, &mut FixedMeasure)
            .unwrap();
        let full = solver.last_stats();
        assert!(!full.clean_snapshot_reused);
        println!(
            "m4e-scale nodes={node_count} cold={cold:?} clean={clean:?} local={local:?} full={full:?}"
        );
    }
}
