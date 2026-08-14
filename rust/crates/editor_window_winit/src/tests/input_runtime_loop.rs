use super::*;

#[test]
fn construct_viewport_input_route_is_serializable() {
    let route = ViewportInputRoute {
        route_kind: ViewportInputRouteKind::RuntimeInputFrame,
        viewport_id: Some("game-view".to_string()),
        viewport_kind: Some(ViewportKind::Game),
        focused: true,
        hovered: true,
        input_event_kind: "KeyDown".to_string(),
        reason: "game_view_focused".to_string(),
        runtime_input_frame: Some(RuntimeInputFrame {
            frame_id: 1,
            viewport_id: "game-view".to_string(),
            events: vec![RuntimeInputEvent::KeyDown {
                key: "Space".to_string(),
            }],
            modifiers: Vec::new(),
            pointer_position: None,
        }),
    };

    let json = serde_json::to_string(&route).expect("route should serialize");
    assert!(json.contains("RuntimeInputFrame"));
    assert!(json.contains("game_view_focused"));
}

#[test]
fn runtime_input_frame_keeps_events_in_order() {
    let mut frame = RuntimeInputFrame::new(1, "game-view");
    frame.events.push(RuntimeInputEvent::KeyDown {
        key: "Space".to_string(),
    });
    frame.events.push(RuntimeInputEvent::KeyUp {
        key: "Space".to_string(),
    });

    assert_eq!(frame.events[0].kind(), "KeyDown");
    assert_eq!(frame.events[1].kind(), "KeyUp");
}

#[test]
fn ui_hit_consumes_pointer_down() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(100.0, 100.0, 640.0, 360.0))
        .unwrap();
    host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 120.0,
            y: 120.0,
            button: PointerButton::Primary,
        },
        true,
        &mut host,
    );

    assert_eq!(route.route_kind, ViewportInputRouteKind::UiConsumed);
    assert!(route.runtime_input_frame.is_none());
}

#[test]
fn scene_view_pointer_drag_routes_to_scene_camera_command() {
    let mut host = ViewportHost::new();
    host.register_scene_viewport("scene-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    host.focus_scene(true).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::PointerMove { x: 10.0, y: 10.0 },
        false,
        &mut host,
    );

    assert_eq!(route.route_kind, ViewportInputRouteKind::SceneCameraCommand);
    assert_eq!(route.viewport_id.as_deref(), Some("scene-view"));
    assert!(route.runtime_input_frame.is_none());
}

#[test]
fn game_view_pointer_coordinates_are_transformed_to_local_runtime_space() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(100.0, 50.0, 640.0, 360.0))
        .unwrap();
    host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 120.0,
            y: 90.0,
            button: PointerButton::Primary,
        },
        false,
        &mut host,
    );

    assert_eq!(route.route_kind, ViewportInputRouteKind::RuntimeInputFrame);
    assert_eq!(route.reason, "game_view_focused_local_coordinates");
    let frame = route.runtime_input_frame.expect("runtime frame");
    assert_eq!(
        frame.pointer_position,
        Some(engine_runtime::input_action::PointerPosition { x: 20.0, y: 40.0 })
    );
    assert!(matches!(
        frame.events.as_slice(),
        [RuntimeInputEvent::PointerDown { x, y, .. }] if *x == 20.0 && *y == 40.0
    ));
}

#[test]
fn game_view_pointer_coordinates_scale_from_display_rect_to_runtime_extent() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(100.0, 50.0, 640.0, 360.0))
        .unwrap();
    host.update_game_runtime_extent(1280, 720).unwrap();
    host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 420.0,
            y: 230.0,
            button: PointerButton::Primary,
        },
        false,
        &mut host,
    );

    let frame = route.runtime_input_frame.expect("runtime frame");
    assert_eq!(
        frame.pointer_position,
        Some(engine_runtime::input_action::PointerPosition { x: 640.0, y: 360.0 })
    );
    assert!(matches!(
        frame.events.as_slice(),
        [RuntimeInputEvent::PointerDown { x, y, .. }] if *x == 640.0 && *y == 360.0
    ));
}

#[test]
fn portrait_game_view_contains_content_and_rejects_display_gutters() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(0.0, 0.0, 1000.0, 600.0))
        .unwrap();
    host.update_game_presentation(
        720,
        1280,
        engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
    )
    .unwrap();
    host.focus_game(true).unwrap();
    let content = host.game_display_content_rect().expect("content rect");
    assert_eq!(content.height, 600.0);
    assert_eq!(content.width, 337.5);
    assert_eq!(content.x, 331.25);
    let mut gateway = ViewportInputGateway::new();

    let gutter = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 100.0,
            y: 300.0,
            button: PointerButton::Primary,
        },
        false,
        &mut host,
    );
    assert_eq!(gutter.route_kind, ViewportInputRouteKind::Ignored);
    assert_eq!(gutter.reason, "game_view_display_gutter");
    assert!(gutter.runtime_input_frame.is_none());

    let center = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 500.0,
            y: 300.0,
            button: PointerButton::Primary,
        },
        false,
        &mut host,
    );
    let frame = center.runtime_input_frame.expect("target-space frame");
    assert_eq!(
        frame.pointer_position,
        Some(engine_runtime::input_action::PointerPosition { x: 360.0, y: 640.0 })
    );
}

#[test]
fn game_view_presentation_is_idempotent_and_rejects_pointer_up_across_resize() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(0.0, 0.0, 1000.0, 600.0))
        .unwrap();
    host.update_game_presentation(
        720,
        1280,
        engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
    )
    .unwrap();
    let revision = host.game_presentation_revision().expect("revision");
    host.update_game_presentation(
        720,
        1280,
        engine_runtime::game_view_presentation::GameViewScalePolicy::Contain,
    )
    .unwrap();
    assert_eq!(host.game_presentation_revision(), Some(revision));
    host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();
    let down = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 500.0,
            y: 300.0,
            button: PointerButton::Primary,
        },
        false,
        &mut host,
    );
    assert_eq!(down.route_kind, ViewportInputRouteKind::RuntimeInputFrame);

    host.update_game_rect(rect(0.0, 0.0, 800.0, 600.0)).unwrap();
    assert!(host.game_presentation_revision().unwrap() > revision);
    let up = gateway.route_editor_input(
        EditorInputEvent::PointerUp {
            x: 400.0,
            y: 300.0,
            button: PointerButton::Primary,
        },
        false,
        &mut host,
    );
    assert_eq!(up.route_kind, ViewportInputRouteKind::Ignored);
    assert_eq!(up.reason, "game_view_presentation_revision_changed");
    assert!(up.runtime_input_frame.is_none());
}

#[test]
fn game_view_first_pointer_down_focuses_and_routes_the_same_event() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(100.0, 50.0, 640.0, 360.0))
        .unwrap();
    host.update_game_runtime_extent(1280, 720).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 420.0,
            y: 230.0,
            button: PointerButton::Primary,
        },
        false,
        &mut host,
    );

    assert_eq!(route.route_kind, ViewportInputRouteKind::RuntimeInputFrame);
    assert!(route.focused);
    assert!(host.game_viewport().unwrap().focused);
    assert!(matches!(
        route.runtime_input_frame.unwrap().events.as_slice(),
        [RuntimeInputEvent::PointerDown { .. }]
    ));
}

#[test]
fn game_view_focused_key_down_routes_to_runtime_input_frame() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::KeyDown {
            key: "Space".to_string(),
        },
        false,
        &mut host,
    );

    assert_eq!(route.route_kind, ViewportInputRouteKind::RuntimeInputFrame);
    assert_eq!(route.reason, "game_view_focused");
    assert_eq!(
        route
            .runtime_input_frame
            .as_ref()
            .expect("runtime frame")
            .events[0]
            .kind(),
        "KeyDown"
    );
}

#[test]
fn game_view_unfocused_key_down_is_ignored() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::KeyDown {
            key: "Space".to_string(),
        },
        false,
        &mut host,
    );

    assert_eq!(route.route_kind, ViewportInputRouteKind::Ignored);
    assert_eq!(route.reason, "viewport_not_focused");
    assert!(route.runtime_input_frame.is_none());
}

#[test]
fn space_key_generates_fire_action_snapshot() {
    let frame = RuntimeInputFrame {
        frame_id: 1,
        viewport_id: "game-view".to_string(),
        events: vec![RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        }],
        modifiers: Vec::new(),
        pointer_position: None,
    };
    let mapping = InputMappingAsset::gameplay_default();

    let snapshot = InputResolver::resolve(&frame, &mapping).action_snapshot;

    assert!(snapshot.button_pressed("action.fire"));
}

#[test]
fn pointer_move_generates_pointer_action_snapshot() {
    let frame = RuntimeInputFrame {
        frame_id: 1,
        viewport_id: "game-view".to_string(),
        events: vec![RuntimeInputEvent::PointerMove { x: 32.0, y: 64.0 }],
        modifiers: Vec::new(),
        pointer_position: Some(engine_runtime::input_action::PointerPosition { x: 32.0, y: 64.0 }),
    };
    let mapping = InputMappingAsset::gameplay_default();

    let snapshot = InputResolver::resolve(&frame, &mapping).action_snapshot;

    assert_eq!(
        snapshot.pointer("action.pointer"),
        Some(engine_runtime::input_action::PointerPosition { x: 32.0, y: 64.0 })
    );
}

#[test]
fn wasd_generates_move_axis2_action_snapshot() {
    let frame = RuntimeInputFrame {
        frame_id: 1,
        viewport_id: "game-view".to_string(),
        events: vec![RuntimeInputEvent::KeyDown {
            key: "D".to_string(),
        }],
        modifiers: Vec::new(),
        pointer_position: None,
    };
    let mapping = InputMappingAsset::gameplay_default();

    let snapshot = InputResolver::resolve(&frame, &mapping).action_snapshot;

    assert_eq!(
        snapshot.axis2("action.move"),
        Some(engine_runtime::input_action::Axis2 { x: 1.0, y: 0.0 })
    );
}

#[test]
fn trace_records_runtime_route_and_action_ids() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();
    let route = gateway.route_editor_input(
        EditorInputEvent::KeyDown {
            key: "Space".to_string(),
        },
        false,
        &mut host,
    );
    let runtime_frame = route.runtime_input_frame.as_ref().unwrap();
    let mapping = InputMappingAsset::gameplay_default();
    let snapshot = InputResolver::resolve(runtime_frame, &mapping).action_snapshot;

    let summary = route.input_trace_summary(Some(&snapshot));

    assert_eq!(summary.viewport_id.as_deref(), Some("game-view"));
    assert_eq!(summary.route_kind.as_deref(), Some("RuntimeInputFrame"));
    assert_eq!(summary.action_ids, vec!["action.fire"]);
}

#[test]
fn trace_records_ui_consumed_route() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::PointerDown {
            x: 1.0,
            y: 1.0,
            button: PointerButton::Primary,
        },
        true,
        &mut host,
    );
    let summary = route.input_trace_summary(None);

    assert_eq!(route.route_kind, ViewportInputRouteKind::UiConsumed);
    assert_eq!(summary.route_kind.as_deref(), Some("UiConsumed"));
    assert_eq!(summary.action_count, 0);
}

#[test]
fn trace_records_ignored_unfocused_game_view() {
    let mut host = ViewportHost::new();
    host.register_game_viewport("game-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::KeyDown {
            key: "Space".to_string(),
        },
        false,
        &mut host,
    );
    let summary = route.input_trace_summary(None);

    assert_eq!(route.route_kind, ViewportInputRouteKind::Ignored);
    assert_eq!(
        summary.route_reason.as_deref(),
        Some("viewport_not_focused")
    );
}

#[test]
fn game_view_space_key_drives_project_logic_and_render_extract() {
    let mut viewport_host = ViewportHost::new();
    viewport_host
        .register_game_viewport("game-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    viewport_host.focus_game(true).unwrap();
    let mut gateway = ViewportInputGateway::new();

    let route = gateway.route_editor_input(
        EditorInputEvent::KeyDown {
            key: "Space".to_string(),
        },
        false,
        &mut viewport_host,
    );
    let runtime_frame = route
        .runtime_input_frame
        .as_ref()
        .expect("game view focused route should create runtime input frame");
    let mapping = InputMappingAsset::gameplay_default();
    let action_snapshot = InputResolver::resolve(runtime_frame, &mapping).action_snapshot;
    let input_summary = route.input_trace_summary(Some(&action_snapshot));

    let entity_id = EntityId::from("entity-player");
    let mut world = World::new();
    world
        .try_spawn_with_components(
            entity_id.clone(),
            "Player",
            "actor",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
            Some(Transform::identity()),
            Some(Renderable {
                mesh_ref: Some("mesh-player".to_string()),
                material_ref: Some("material-player".to_string()),
                visible: true,
                layer: "default".to_string(),
            }),
        )
        .expect("input runtime fixture must be valid");
    world.take_dirty_records();
    let mut engine_host = EngineHostLoop::with_project_logic("scene-main", fire_move_runner());
    engine_host
        .render_scene_mut()
        .register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::SceneView,
            RenderTargetKind::ViewportTexture,
        ));

    let output = engine_host.tick(
        EngineFrameInput::new(EngineHostMode::EditorStep)
            .with_action_snapshot(action_snapshot)
            .with_input_trace_summary(input_summary),
        &mut world,
    );

    assert_eq!(world.transform(&entity_id).unwrap().local_position.x, 1.0);
    assert_eq!(
        output
            .render_frame_report
            .as_ref()
            .expect("render frame report")
            .counters
            .raw_command_count,
        1
    );
    assert!(output
        .runtime_trace
        .events
        .iter()
        .any(|event| event.phase == "InputSnapshotReady" && event.message.contains("action.fire")));
    assert!(output
        .runtime_trace
        .events
        .iter()
        .any(|event| event.system_id == "project.rule.project.fire_move"));
}
