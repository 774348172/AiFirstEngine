use super::*;

#[test]
fn viewport_host_registers_resizes_focuses_and_rejects_second_scene_viewport() {
    let mut host = ViewportHost::new();
    host.register_scene_viewport("scene-view", rect(10.0, 20.0, 300.0, 200.0))
        .expect("scene viewport should register");
    assert!(host
        .register_scene_viewport("second", rect(0.0, 0.0, 100.0, 100.0))
        .is_err());
    host.update_scene_rect(rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    host.focus_scene(true).unwrap();
    let viewport = host.scene_viewport().unwrap();
    assert_eq!(viewport.rect.width, 640.0);
    assert!(viewport.focused);
}

#[test]
fn headless_runtime_renderer_hash_is_stable_and_changes_with_camera() {
    let mut host = ViewportHost::new();
    host.register_scene_viewport("scene-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    let first = HeadlessRuntimeRenderer::render(1, host.scene_viewport().unwrap());
    let second = HeadlessRuntimeRenderer::render(1, host.scene_viewport().unwrap());
    assert_eq!(first.frame_hash, second.frame_hash);

    let mut camera = SceneCameraState::default();
    camera.position.x = 10.0;
    host.set_camera_state(camera).unwrap();
    let changed = HeadlessRuntimeRenderer::render(1, host.scene_viewport().unwrap());
    assert_ne!(first.frame_hash, changed.frame_hash);
}

#[test]
fn runtime_viewport_texture_flows_into_ui_draw_slot() {
    let mut scene = RenderSceneState::new();
    scene.register_view(RenderViewState::new(
        RenderViewId(2),
        RenderViewKind::SceneView,
        RenderTargetKind::ViewportTexture,
    ));
    let output = RuntimeRenderer::new().build(RuntimeRendererInput {
        frame_index: 9,
        render_scene_state: &scene,
        render_view_state: scene.view(RenderViewId(2)),
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::viewport_texture("viewport-scene", 640, 360),
    });
    let descriptor = output
        .texture_descriptor
        .as_ref()
        .expect("runtime renderer should output viewport texture descriptor");

    let mut host = ViewportHost::new();
    host.register_scene_viewport("scene-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    let summary = RuntimeViewportFrameSummary::from_descriptor("scene-view", descriptor);
    host.ingest_runtime_frame(summary.clone())
        .expect("viewport host should ingest runtime texture frame");
    assert_eq!(host.latest_runtime_frame(), Some(&summary));

    let mut model = fixture_model();
    model.viewport.frame = summary.frame_index;
    model.viewport.texture_id = Some(summary.texture_id.clone());
    model.viewport.target_id = Some(summary.target_id.clone());
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));

    assert!(draw_list.commands.iter().any(|command| {
        matches!(
            command,
            editor_ui_renderer::DrawCommand::ViewportTextureSlot {
                frame,
                texture_id: Some(texture_id),
                target_id: Some(target_id),
                ..
            } if *frame == summary.frame_index
                && texture_id == &summary.texture_id
                && target_id == &summary.target_id
        )
    }));
}

#[test]
fn minimal_runtime_to_editor_viewport_loop_outputs_traceable_texture_slot() {
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
            Some(Transform {
                local_position: Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                local_rotation: Vec3::ZERO,
                local_scale: Vec3::ONE,
            }),
            Some(Renderable {
                mesh_ref: Some("mesh-player".to_string()),
                material_ref: Some("material-player".to_string()),
                visible: true,
                layer: "default".to_string(),
            }),
        )
        .expect("viewport runtime fixture must be valid");
    world
        .try_insert_transform(entity_id, Transform::identity())
        .expect("viewport runtime fixture entity must exist");

    let mut engine_host = EngineHostLoop::new("scene-main");
    engine_host
        .render_scene_mut()
        .register_view(RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::SceneView,
            RenderTargetKind::ViewportTexture,
        ));

    let engine_frame = engine_host.tick(
        EngineFrameInput::new(EngineHostMode::EditorStep),
        &mut world,
    );
    assert!(engine_frame.runtime_advanced);
    assert!(engine_frame.render_built);
    assert_eq!(
        engine_frame
            .renderer_feature_frame
            .as_ref()
            .expect("renderer feature frame")
            .draw_items
            .len(),
        1
    );

    let render_output = RuntimeRenderer::new().build(RuntimeRendererInput {
        frame_index: engine_frame.frame_index,
        render_scene_state: engine_host.render_scene(),
        render_view_state: engine_host.render_scene().view(RenderViewId(1)),
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::viewport_texture("viewport-scene", 640, 360),
    });
    assert_eq!(render_output.render_frame_report.draw_item_count, 1);
    let descriptor = render_output
        .texture_descriptor
        .as_ref()
        .expect("runtime renderer should produce viewport texture descriptor");

    let mut viewport_host = ViewportHost::new();
    viewport_host
        .register_scene_viewport("scene-view", rect(0.0, 0.0, 640.0, 360.0))
        .unwrap();
    let summary = RuntimeViewportFrameSummary::from_descriptor("scene-view", descriptor);
    viewport_host.ingest_runtime_frame(summary.clone()).unwrap();

    let mut model = fixture_model();
    model.viewport.frame = engine_frame.frame_index;
    model.viewport.frame_hash = engine_frame.frame_hash.clone();
    model.viewport.renderable_count = render_output.render_frame_report.draw_item_count;
    model.viewport.texture_id = Some(summary.texture_id.clone());
    model.viewport.target_id = Some(summary.target_id.clone());

    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let slot = draw_list
        .commands
        .iter()
        .find_map(|command| match command {
            editor_ui_renderer::DrawCommand::ViewportTextureSlot {
                frame,
                texture_id,
                target_id,
                ..
            } => Some((frame, texture_id, target_id)),
            _ => None,
        })
        .expect("draw list should contain viewport texture slot");

    assert_eq!(*slot.0, engine_frame.frame_index);
    assert_eq!(slot.1.as_deref(), Some(summary.texture_id.as_str()));
    assert_eq!(slot.2.as_deref(), Some(summary.target_id.as_str()));
    assert_eq!(
        viewport_host
            .latest_runtime_frame()
            .expect("latest runtime frame")
            .texture_id,
        summary.texture_id
    );
}
