use engine_runtime::render_contract::*;

mod support {
    use engine_runtime::components::{Hierarchy, SpriteRenderer2D, Transform};
    use engine_runtime::ids::SourceEntityId;
    use engine_runtime::render_command::{apply_batch, RenderCommand};
    use engine_runtime::render_extract::RenderExtractContext;
    use engine_runtime::render_state::RenderSceneState;
    use engine_runtime::world::World;

    pub fn spawn(
        world: &mut World,
        name: &str,
        sprite: SpriteRenderer2D,
        transform: Transform,
    ) -> SourceEntityId {
        let id = SourceEntityId::from(name);
        world
            .try_spawn_with_components(
                id.clone(),
                name,
                "test",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: 0,
                },
                Some(transform),
                None,
            )
            .unwrap();
        world
            .try_insert_sprite_renderer2d(id.clone(), sprite)
            .unwrap();
        id
    }

    pub fn project(
        world: &mut World,
        scene: &mut RenderSceneState,
        extract: &mut RenderExtractContext,
        frame: u64,
    ) -> Vec<RenderCommand> {
        let commands = extract
            .extract_world_dirty(frame, world, scene)
            .normalize_merge(scene);
        assert!(apply_batch(scene, &commands).is_empty());
        commands
    }
}

mod adapter {
    use super::*;
    use engine_runtime::components::{ComponentTypeId, Renderable, SpriteRenderer2D, Transform};
    use engine_runtime::render_command::RenderCommandType;
    use engine_runtime::render_extract::RenderExtractContext;
    use engine_runtime::render_projection_adapter::{
        RenderProjectionAdapter, SpriteProjection, SpriteRenderer2DAdapter,
    };
    use engine_runtime::render_state::{
        RenderPayloadKind, RenderProxyDescriptor, RenderProxyPayload, RenderSceneState,
        SpritePayload,
    };
    use engine_runtime::world::World;

    fn specimen(reference: Option<&str>) -> SpriteRenderer2D {
        SpriteRenderer2D {
            sprite_ref: reference.map(Into::into),
            material_ref: Some("material-a".into()),
            color: [0.2, 0.4, 0.6, 0.8],
            flip_x: true,
            flip_y: true,
            sorting_layer: -4,
            order_in_layer: 11,
            sort_z: -2.5,
            visible: false,
        }
    }

    #[test]
    fn sprite_conversion_preserves_fields_and_provenance_without_resolution() {
        for reference in [None, Some(""), Some("texture-a")] {
            let component = specimen(reference);
            let neutral = SpriteRenderer2DAdapter.project(&"entity".into(), &component);
            let expected = RenderProxyDescriptor {
                visible: false,
                layer: "sprite2d".into(),
                payload: RenderProxyPayload::Sprite(SpritePayload {
                    sprite_ref: reference.map(Into::into),
                    material_ref: Some("material-a".into()),
                    color: [0.2, 0.4, 0.6, 0.8],
                    flip_x: true,
                    flip_y: true,
                    sorting_layer: -4,
                    order_in_layer: 11,
                    sort_z: -2.5,
                }),
            };
            let old = neutral.to_legacy().unwrap();
            assert_eq!(old.payload, expected);
            assert_eq!(old.provenance.source_entity_id.as_deref(), Some("entity"));
            assert_eq!(old.provenance.source_path, None);
            assert_eq!(old.provenance.content_digest, None);
            assert_eq!(
                SpriteProjection::from_legacy(&old.payload, old.provenance.clone()).unwrap(),
                neutral
            );
            let json = serde_json::to_string(&RenderContract::new(neutral.clone())).unwrap();
            assert_eq!(
                RenderContract::<SpriteProjection>::decode(&json)
                    .unwrap()
                    .data,
                neutral
            );
        }
    }

    #[test]
    fn unsupported_projection_never_drops_layers_policy_or_visibility() {
        let original =
            SpriteRenderer2DAdapter.project(&"entity".into(), &specimen(Some("texture-a")));
        let mut new = original.clone();
        new.layers = LayerSelection::Named {
            names: vec!["a".into(), "b".into()],
        };
        assert_eq!(
            new.to_legacy().unwrap_err().code,
            ContractErrorCode::UnsupportedLegacyConversion
        );
        new = original.clone();
        new.policy = RenderPolicy::Opaque;
        assert!(new.to_legacy().is_err());
        assert!(new.validate().is_err());
        new = original.clone();
        new.visibility.inherited = Evaluation::Known(false);
        assert!(new.to_legacy().is_err());
        new = original;
        new.domain = RenderDomain::World3d;
        assert!(new.to_legacy().is_err());
    }

    #[test]
    fn strict_numeric_validation_does_not_change_legacy_extraction() {
        let mut component = specimen(None);
        component.color[0] = f32::NAN;
        component.sort_z = f32::INFINITY;
        let mut world = World::new();
        let id = support::spawn(
            &mut world,
            "old-input",
            component.clone(),
            Transform::identity(),
        );
        let mut scene = RenderSceneState::new();
        support::project(&mut world, &mut scene, &mut RenderExtractContext::new(), 1);
        let RenderProxyPayload::Sprite(payload) = &scene
            .proxy(scene.proxy_for_source(&id).unwrap())
            .unwrap()
            .payload
        else {
            panic!("sprite disappeared")
        };
        assert!(payload.color[0].is_nan());
        assert!(payload.sort_z.is_infinite());
        let neutral = SpriteRenderer2DAdapter.project(&id, &component);
        assert_eq!(
            neutral.validate().unwrap_err().code,
            ContractErrorCode::NonFinite
        );
    }

    #[test]
    fn world_extract_preserves_empty_refs_and_all_sprite_fields() {
        for reference in [None, Some(""), Some("texture-a")] {
            let mut world = World::new();
            let component = specimen(reference);
            let id = support::spawn(
                &mut world,
                "entity",
                component.clone(),
                Transform::identity(),
            );
            let mut scene = RenderSceneState::new();
            let commands =
                support::project(&mut world, &mut scene, &mut RenderExtractContext::new(), 1);
            assert_eq!(commands.len(), 1);
            assert_eq!(commands[0].command_type, RenderCommandType::AddProxy);
            let proxy = scene.proxy(scene.proxy_for_source(&id).unwrap()).unwrap();
            assert!(!proxy.common.visible);
            assert_eq!(proxy.common.layer, "sprite2d");
            assert_eq!(
                proxy.common.runtime_entity_id,
                world.runtime_id_for_source(&id).unwrap()
            );
            assert_eq!(
                proxy.payload,
                RenderProxyPayload::Sprite(SpritePayload {
                    sprite_ref: reference.map(Into::into),
                    material_ref: Some("material-a".into()),
                    color: [0.2, 0.4, 0.6, 0.8],
                    flip_x: true,
                    flip_y: true,
                    sorting_layer: -4,
                    order_in_layer: 11,
                    sort_z: -2.5
                })
            );
        }
    }

    #[test]
    fn lifecycle_keeps_identity_sprite_priority_and_mesh_fallback() {
        let mut world = World::new();
        let id = support::spawn(
            &mut world,
            "entity",
            SpriteRenderer2D::default(),
            Transform::identity(),
        );
        world
            .try_insert_renderable(
                id.clone(),
                Renderable {
                    mesh_ref: Some("mesh".into()),
                    material_ref: None,
                    visible: true,
                    layer: "default".into(),
                },
            )
            .unwrap();
        let mut scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        support::project(&mut world, &mut scene, &mut extract, 1);
        let proxy_id = scene.proxy_for_source(&id).unwrap();
        let runtime_id = world.runtime_id_for_source(&id).unwrap();
        assert_eq!(
            scene.proxy(proxy_id).unwrap().payload.kind(),
            RenderPayloadKind::Sprite
        );
        assert!(support::project(&mut world, &mut scene, &mut extract, 2).is_empty());
        let mut transform = Transform::identity();
        transform.local_position.x = 7.0;
        world
            .try_insert_transform(id.clone(), transform.clone())
            .unwrap();
        let commands = support::project(&mut world, &mut scene, &mut extract, 3);
        assert_eq!(commands[0].command_type, RenderCommandType::UpdateTransform);
        assert_eq!(scene.proxy(proxy_id).unwrap().common.transform, transform);
        world
            .try_insert_sprite_renderer2d(id.clone(), specimen(Some("updated")))
            .unwrap();
        let commands = support::project(&mut world, &mut scene, &mut extract, 4);
        assert_eq!(
            commands[0].command_type,
            RenderCommandType::UpdateRenderState
        );
        assert_eq!(scene.proxy_for_source(&id), Some(proxy_id));
        world
            .try_remove_component_value(&id, &ComponentTypeId::sprite_renderer2d())
            .unwrap();
        support::project(&mut world, &mut scene, &mut extract, 5);
        assert_eq!(
            scene.proxy(proxy_id).unwrap().payload.kind(),
            RenderPayloadKind::Mesh
        );
        world
            .try_remove_component_value(&id, &ComponentTypeId::renderable())
            .unwrap();
        let commands = support::project(&mut world, &mut scene, &mut extract, 6);
        assert_eq!(commands[0].command_type, RenderCommandType::RemoveProxy);
        assert!(scene.proxy(proxy_id).is_none());
        world
            .try_insert_sprite_renderer2d(id.clone(), SpriteRenderer2D::default())
            .unwrap();
        support::project(&mut world, &mut scene, &mut extract, 7);
        world.try_despawn_entity(&id).unwrap();
        assert!(world.runtime_id_for_source(&id).is_none());
        let commands = support::project(&mut world, &mut scene, &mut extract, 8);
        assert_eq!(commands[0].command_type, RenderCommandType::RemoveProxy);
        assert_eq!(commands[0].runtime_entity_id, runtime_id);
        assert_eq!(scene.proxies_len(), 0);
    }
}

mod consumer {
    use super::*;
    use engine_runtime::aui::{
        AuiCanvas, AuiCompositionFrame, AuiCompositionStage, AuiDocument, AuiLayoutEngine, AuiNode,
        AuiNodeKind, AuiRect, AuiRendererBridge, AuiStyle,
    };
    use engine_runtime::components::{SpriteRenderer2D, Transform};
    use engine_runtime::engine_rhi::{EngineRhiBackend, RhiBackendDiagnosticSeverity};
    use engine_runtime::headless_rhi_backend::HeadlessRhiBackend;
    use engine_runtime::math::Vec3;
    use engine_runtime::render_extract::RenderExtractContext;
    use engine_runtime::render_graph::{RenderDrawVertex, RenderPassCommand};
    use engine_runtime::render_resource::{RenderResourceHandle, RenderResourceKind};
    use engine_runtime::render_state::{
        RenderSceneState, RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
    };
    use engine_runtime::renderer_feature_builder::RendererFeatureBuilder;
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    use engine_runtime::runtime_renderer::{
        QualityProfile, RenderTarget, RuntimeRenderer, RuntimeRendererInput, RuntimeRendererOutput,
    };
    use engine_runtime::sprite2d_render_pipeline::{
        Sprite2DRenderPipeline, Sprite2DTextureBindingContext,
    };
    use engine_runtime::world::World;

    fn sprite() -> SpriteRenderer2D {
        SpriteRenderer2D {
            sprite_ref: Some("texture-a".into()),
            material_ref: Some("material-a".into()),
            color: [0.2, 0.4, 0.6, 0.8],
            flip_x: true,
            flip_y: true,
            ..Default::default()
        }
    }

    fn scene(sprite: SpriteRenderer2D) -> RenderSceneState {
        let mut world = World::new();
        support::spawn(
            &mut world,
            "sprite",
            sprite,
            Transform {
                local_position: Vec3 {
                    x: 2.0,
                    y: 3.0,
                    z: 4.0,
                },
                local_rotation: Vec3 {
                    x: 12.0,
                    y: 33.0,
                    z: 90.0,
                },
                local_scale: Vec3 {
                    x: -2.0,
                    y: 3.0,
                    z: 1.0,
                },
            },
        );
        let mut scene = RenderSceneState::new();
        support::project(&mut world, &mut scene, &mut RenderExtractContext::new(), 1);
        let mut view = RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::ViewportTexture,
        );
        view.source_entity_id = Some("camera".into());
        view.viewport.width = 640;
        view.viewport.height = 360;
        scene.register_view(view);
        scene
    }

    fn build(
        scene: &RenderSceneState,
        bindings: Option<&Sprite2DTextureBindingContext>,
        composition: Option<&AuiCompositionFrame>,
    ) -> RuntimeRendererOutput {
        RuntimeRenderer::new().build(RuntimeRendererInput {
            frame_index: 1,
            render_scene_state: scene,
            render_view_state: scene.view(RenderViewId(1)),
            aui_overlay: None,
            aui_composition: composition,
            sprite_texture_bindings: bindings,
            runtime_texture_bindings: None,
            game_view_presentation: None,
            quality_profile: QualityProfile::default(),
            render_target: RenderTarget::headless_texture("u0-target", 640, 360),
        })
    }

    fn vertices(output: &RuntimeRendererOutput) -> &[RenderDrawVertex] {
        output
            .render_graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| match c {
                RenderPassCommand::DrawSpriteTextured { vertices, .. } => Some(vertices.as_slice()),
                _ => None,
            })
            .expect("actual sprite geometry")
    }

    #[test]
    fn actual_extract_geometry_binding_and_rhi_match_fixed_legacy_values() {
        let scene = scene(sprite());
        let handle = RenderResourceHandle {
            kind: RenderResourceKind::Texture,
            index: 17,
            generation: 8,
        };
        let mut bindings = Sprite2DTextureBindingContext::new();
        bindings.insert_texture_handle("texture-a", handle, "nearest");
        let output = build(&scene, Some(&bindings), None);
        let expected_positions = [
            [5.0, 5.0],
            [5.0, 1.0],
            [-1.0, 1.0],
            [5.0, 5.0],
            [-1.0, 1.0],
            [-1.0, 5.0],
        ];
        let expected_uvs = [
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [1.0, 1.0],
        ];
        assert_eq!(vertices(&output).len(), 6);
        for (i, vertex) in vertices(&output).iter().enumerate() {
            for axis in 0..2 {
                assert!(
                    (vertex.position[axis].to_f32() - expected_positions[i][axis]).abs() < 0.00001
                );
            }
            assert_eq!(vertex.uv.map(|v| v.to_f32()), expected_uvs[i]);
            assert_eq!(vertex.color.map(|v| v.to_f32()), [0.2, 0.4, 0.6, 0.8]);
        }
        let draw = output
            .rhi_command_plan
            .commands
            .iter()
            .find_map(|c| match c {
                RhiCommand::Draw {
                    payload:
                        RhiDrawPayload::SpriteTextured {
                            sprite_ref,
                            material_ref,
                            vertices,
                            binding: Some(binding),
                            ..
                        },
                    ..
                } => Some((sprite_ref, material_ref, vertices, binding)),
                _ => None,
            })
            .expect("typed RHI sprite payload");
        assert_eq!(draw.0, "texture-a");
        assert_eq!(draw.1.as_deref(), Some("material-a"));
        assert_eq!(draw.2.as_slice(), vertices(&output));
        assert_eq!(draw.3.resources().collect::<Vec<_>>(), vec![handle]);
        assert_eq!(draw.3.texture("baseColorTexture").unwrap().raw(), handle);
        assert!(!draw.3.fallback_used);
        assert!(!output.rhi_command_plan.has_errors());
        let report = HeadlessRhiBackend::new().execute_plan(&output.rhi_command_plan);
        assert_eq!(
            (
                report.draw_count,
                report.binding_count,
                report.present_count
            ),
            (1, 1, 1)
        );
        assert!(report
            .diagnostics
            .iter()
            .all(|d| d.severity != RhiBackendDiagnosticSeverity::Error));
    }

    #[test]
    fn neutral_legacy_view_preserves_explicit_projection_and_placeholder_fallback() {
        for explicit_projection in [false, true] {
            let mut scene = scene(sprite());
            let mut view = scene.view(RenderViewId(1)).unwrap().clone();
            view.source_entity_id = None;
            let factors = if explicit_projection {
                view.projection_matrix[0] = 0.5;
                view.projection_matrix[5] = 0.25;
                [0.5, 0.25]
            } else {
                [1.0 / (7.5 * 640.0 / 360.0), 1.0 / 7.5]
            };
            let neutral = RenderView::from_legacy(&view);
            scene.register_view(neutral.to_legacy().unwrap().payload);
            let output = build(&scene, None, None);
            let first = vertices(&output)[0].position.map(|v| v.to_f32());
            for axis in 0..2 {
                assert!((first[axis] - 5.0 * factors[axis]).abs() < 0.00001);
            }
        }
    }

    #[test]
    fn sort_zero_fallback_quantization_and_proxy_tie_break_survive_projection() {
        let mut world = World::new();
        let mut scene = RenderSceneState::new();
        let mut extract = RenderExtractContext::new();
        let cases = [
            ("z-first", 0, 0, 1.0001, 9.0, true),
            ("a-second", 0, 0, 1.0004, 9.0, true),
            ("fallback", 0, 0, 0.0, -1.0, true),
            ("low-layer", -1, 0, 5.0, 0.0, true),
            ("high-order", 0, 1, -9.0, 0.0, true),
            ("hidden", -5, 0, 0.0, 0.0, false),
        ];
        for (i, (name, layer, order, sort_z, z, visible)) in cases.into_iter().enumerate() {
            let mut transform = Transform::identity();
            transform.local_position.z = z;
            support::spawn(
                &mut world,
                name,
                SpriteRenderer2D {
                    sorting_layer: layer,
                    order_in_layer: order,
                    sort_z,
                    visible,
                    ..sprite()
                },
                transform,
            );
            support::project(&mut world, &mut scene, &mut extract, i as u64 + 1);
        }
        let frame = RendererFeatureBuilder::new().build(1, &scene);
        assert_eq!(
            frame
                .draw_items
                .iter()
                .map(|d| d.source_entity_id.as_str())
                .collect::<Vec<_>>(),
            ["low-layer", "fallback", "z-first", "a-second", "high-order"]
        );
        assert_eq!(frame.draw_items[1].sort_key.sort_z_quantized, -1000);
        assert_eq!(frame.draw_items[2].sort_key.sort_z_quantized, 1000);
        assert_eq!(frame.draw_items[3].sort_key.sort_z_quantized, 1000);
        assert!(frame.draw_items[2].proxy_id < frame.draw_items[3].proxy_id);
        assert_eq!(frame.counters.skipped_invisible_count, 1);
        let mut view = RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        );
        view.layer_mask = Some("sprite2d".into());
        let plans = Sprite2DRenderPipeline::new().build(1, &frame, Some(&view));
        assert_eq!(
            plans
                .draw_plans
                .iter()
                .map(|d| d.source_entity_id.as_str())
                .collect::<Vec<_>>(),
            ["low-layer", "fallback", "z-first", "a-second", "high-order"]
        );
        view.layer_mask = Some("excluded".into());
        let excluded = Sprite2DRenderPipeline::new().build(1, &frame, Some(&view));
        assert!(excluded.draw_plans.is_empty());
        assert_eq!(
            excluded
                .diagnostics
                .iter()
                .filter(|d| d.code == "sprite_layer_mismatch")
                .count(),
            5
        );
    }

    #[test]
    fn missing_and_empty_sprite_keys_keep_existing_skip_diagnostic_without_geometry() {
        for reference in [None, Some(String::new())] {
            let output = build(
                &scene(SpriteRenderer2D {
                    sprite_ref: reference,
                    ..sprite()
                }),
                None,
                None,
            );
            assert!(output
                .render_frame_report
                .diagnostics
                .iter()
                .any(|d| d.code == "sprite_missing_ref"));
            assert!(!output
                .rhi_command_plan
                .commands
                .iter()
                .any(|c| matches!(c, RhiCommand::Draw { .. })));
        }
    }

    #[test]
    fn projected_world_sprite_keeps_aui_composition_order_and_geometry() {
        let mut canvases = Vec::new();
        let mut nodes = Vec::new();
        for (name, stage, color) in [
            ("before", AuiCompositionStage::BeforeWorld, "#ff0000"),
            ("overlay", AuiCompositionStage::ScreenOverlay, "#00ff00"),
            ("modal", AuiCompositionStage::Modal, "#0000ff"),
        ] {
            let mut canvas = AuiCanvas::screen_overlay(name, 640.0, 360.0, name);
            canvas.composition_stage = stage;
            canvases.push(canvas);
            let mut node = AuiNode::new(name, AuiNodeKind::Panel, AuiRect::stretch_full());
            node.style = Some(AuiStyle::color(color));
            nodes.push(node);
        }
        let document = AuiDocument::new("u0-composition", canvases, nodes);
        let layout = AuiLayoutEngine::layout(&document, 1);
        let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        let composition =
            AuiRendererBridge::build_composition_frame(1, &document, &layout, &draw_list);
        let output = build(&scene(sprite()), None, Some(&composition));
        let ids = output
            .render_graph
            .passes
            .iter()
            .map(|p| p.pass_id.as_str())
            .collect::<Vec<_>>();
        let before = ids
            .iter()
            .position(|v| *v == "draw-aui-before-world")
            .unwrap();
        let world = ids
            .iter()
            .position(|v| v.starts_with("draw-sprite2d-"))
            .unwrap();
        let overlay = ids
            .iter()
            .position(|v| *v == "draw-aui-screen-overlay")
            .unwrap();
        let modal = ids.iter().position(|v| *v == "draw-aui-modal").unwrap();
        let present = ids.iter().position(|v| *v == "present-main").unwrap();
        assert!(before < world && world < overlay && overlay < modal && modal < present);
        let ui = output
            .render_graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .filter_map(|c| match c {
                RenderPassCommand::DrawUiComposition {
                    stage, vertices, ..
                } => Some((stage.as_str(), vertices)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ui.iter().map(|v| v.0).collect::<Vec<_>>(),
            ["BeforeWorld", "ScreenOverlay", "Modal"]
        );
        for ((_, vertices), color) in ui.iter().zip([
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
        ]) {
            assert_eq!(vertices.len(), 6);
            // Existing resolved-paint polygon starts at the top-left corner.
            assert_eq!(vertices[0].position.map(|v| v.to_f32()), [-1.0, 1.0]);
            assert!(vertices
                .iter()
                .all(|v| v.color.map(|c| c.to_f32()) == color));
        }
        assert!(!output.rhi_command_plan.has_errors());
        let report = HeadlessRhiBackend::new().execute_plan(&output.rhi_command_plan);
        assert_eq!((report.draw_count, report.present_count), (4, 1));
    }
}

mod contract {
    use super::*;
    use engine_runtime::components::Transform;
    use engine_runtime::math::Vec3;
    use engine_runtime::render_state::{
        RenderTargetKind, RenderViewId, RenderViewKind, RenderViewState,
    };
    use engine_runtime::runtime_asset::{RuntimeAssetIndex, RuntimeAssetRecord};
    use engine_runtime::runtime_package::RuntimeAssetRef;

    fn asset() -> RenderAssetReference {
        RenderAssetReference {
            expected_kind: RenderAssetKind::Texture,
            reference: RuntimeAssetRef {
                id: "texture-a".into(),
                asset_type: "texture".into(),
                guid: Some("guid-a".into()),
                sub_asset: Some("region-a".into()),
            },
        }
    }

    fn index() -> RuntimeAssetIndex {
        RuntimeAssetIndex::new(
            vec![RuntimeAssetRecord {
                asset_guid: "guid-a".into(),
                asset_id: "texture-a".into(),
                asset_type: "texture".into(),
                sub_asset_id: Some("region-a".into()),
                version: "7".into(),
                cooked_asset_id: "cooked-a".into(),
                bundle_id: "main".into(),
                loader_kind: "texture".into(),
                dependencies: vec![],
                hash: Some("digest-a".into()),
                size: None,
                flags: vec![],
                source_map_debug: Some("assets/a.png".into()),
            }],
            vec![],
        )
    }

    #[test]
    fn strict_asset_schema_reuses_old_keys_and_roundtrips() {
        let json = r#"{"schemaVersion":"render-contract.v1","data":{"expectedKind":"texture","reference":{"id":"texture-a","type":"texture","guid":"guid-a","sub_asset":"region-a"}}}"#;
        let parsed = RenderContract::<RenderAssetReference>::decode(json).unwrap();
        assert_eq!(parsed.data, asset());
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
        assert_eq!(parsed.data.to_legacy().unwrap(), asset().reference);
        for bad in [
            json.replace("render-contract.v1", "render-contract.v9"),
            json.replace("expectedKind", "expectedKnd"),
            json.replace("texture\",\"reference", "sound\",\"reference"),
            json.replace("sub_asset", "subAsset"),
        ] {
            assert!(
                RenderContract::<RenderAssetReference>::decode(&bad).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn asset_resolution_requires_consistent_identity_and_never_claims_ready() {
        assert!(matches!(
            asset().resolve(None).unwrap(),
            RenderAssetResolution::Unresolved
        ));
        let index = index();
        let reference = asset();
        let RenderAssetResolution::Resolved(record) = reference.resolve(Some(&index)).unwrap()
        else {
            panic!("expected record")
        };
        assert_eq!(record.version, "7");
        assert_eq!(record.source_map_debug.as_deref(), Some("assets/a.png"));
        let mut invalid = asset();
        invalid.reference.id = "different-id".into();
        assert_eq!(
            invalid.resolve(Some(&index)).unwrap_err().code,
            ContractErrorCode::AssetIdentityConflict
        );
        invalid = asset();
        invalid.expected_kind = RenderAssetKind::Mesh;
        assert_eq!(
            invalid.validate().unwrap_err().code,
            ContractErrorCode::AssetTypeMismatch
        );
        invalid = asset();
        invalid.reference.sub_asset = Some("missing".into());
        assert_eq!(
            invalid.resolve(Some(&index)).unwrap_err().code,
            ContractErrorCode::AssetSubAssetMissing
        );
        invalid = asset();
        invalid.reference.guid = Some("missing".into());
        assert_eq!(
            invalid.resolve(Some(&index)).unwrap_err().code,
            ContractErrorCode::AssetMissing
        );
        assert_eq!(
            invalid.resolve(Some(&index)).unwrap_err().stage,
            ProducerStage::AssetResolution
        );
    }

    #[test]
    fn structural_errors_are_actionable_without_changing_the_old_decoder() {
        let old: RuntimeAssetRef =
            serde_json::from_str(r#"{"id":"old","type":"texture","legacyExtra":true}"#).unwrap();
        assert_eq!((old.guid, old.sub_asset), (None, None));
        let mut json = serde_json::to_value(RenderContract::new(RenderView::perspective(
            1, 640, 360, 60.0, 0.1, 10.0,
        )))
        .unwrap();
        json["data"]["projection"]["fovTypo"] = serde_json::json!(50);
        let error = RenderContract::<RenderView>::decode(&json.to_string()).unwrap_err();
        assert_eq!(error.compact().code, ContractErrorCode::InvalidSchema);
        let mut report = ContractDiagnostics::new(DiagnosticLevel::Summary);
        let source = Provenance::unspecified(ProducerStage::ContractValidation);
        report.record(error.compact(), RenderDomain::World3d, &source, || {
            DiagnosticDetails {
                actual: error.to_string(),
                dependency_chain: vec![],
            }
        });
        assert_eq!(report.entries[0].stage, ProducerStage::ContractValidation);
        assert!(report.entries[0].actual.contains("fovTypo"));
        assert!(!report.entries[0].next_action.is_empty());
        assert!(serde_json::to_value(&report.entries[0])
            .unwrap()
            .get("schemaVersion")
            .is_some());
    }

    #[test]
    fn new_view_layers_and_metadata_cannot_silently_downgrade() {
        let mut old = RenderViewState::new(
            RenderViewId(1),
            RenderViewKind::Game,
            RenderTargetKind::Window,
        );
        old.viewport.width = 0;
        let original = RenderView::from_legacy(&old);
        assert_eq!(original.to_legacy().unwrap().payload, old);
        let mut view = original.clone();
        view.layers = LayerSelection::Named {
            names: vec!["a".into(), "b".into()],
        };
        assert_eq!(
            view.to_legacy().unwrap_err().code,
            ContractErrorCode::UnsupportedLegacyConversion
        );
        view.layers = LayerSelection::Named {
            names: vec!["a".into(), "a".into()],
        };
        assert_eq!(
            view.validate().unwrap_err().code,
            ContractErrorCode::InvalidLayers
        );
        view = original.clone();
        view.clear_policy = ClearPolicy::Load;
        assert!(view.to_legacy().is_err());
        view = original;
        view.depth_policy = DepthPolicy::TestAndWrite;
        assert!(view.to_legacy().is_err());
        let mut typed = RenderView::perspective(1, 640, 360, 60.0, 0.1, 10.0);
        assert_eq!(
            typed.provenance.availability,
            SourceAvailability::InputContextNotSupplied
        );
        typed.domain = RenderDomain::World2d;
        assert_eq!(
            typed.validate().unwrap_err().code,
            ContractErrorCode::InvalidDomain
        );
    }

    #[test]
    fn legacy_transform_is_exact_and_new_world3d_cannot_downgrade() {
        let old = Transform {
            local_position: Vec3 {
                x: 800.0,
                y: -60.0,
                z: 2.0,
            },
            local_rotation: Vec3 {
                x: 12.0,
                y: 25.0,
                z: 90.0,
            },
            local_scale: Vec3 {
                x: -20.0,
                y: 4.0,
                z: 1.0,
            },
        };
        let mut neutral = TransformMetadata::from_legacy(&old);
        assert_eq!(neutral.to_legacy().unwrap(), old);
        assert_eq!(neutral.world_matrix, Evaluation::NotEvaluated);
        assert_eq!(neutral.previous_world_matrix, Evaluation::NotEvaluated);
        assert_eq!(
            TransformMetadata::from_legacy(&neutral.to_legacy().unwrap()),
            neutral
        );
        neutral.convention = CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ;
        assert_eq!(
            neutral.to_legacy().unwrap_err().code,
            ContractErrorCode::UnsupportedLegacyConversion
        );
        neutral.position[0] = f32::NAN;
        assert_eq!(
            neutral.validate().unwrap_err().code,
            ContractErrorCode::NonFinite
        );
    }

    #[test]
    fn legacy_view_preserves_camera_identity_matrices_and_all_targets() {
        for target in [
            RenderTargetKind::Window,
            RenderTargetKind::ViewportTexture,
            RenderTargetKind::RenderTexture,
            RenderTargetKind::ShadowMap,
            RenderTargetKind::PreviewTexture,
        ] {
            for camera in [None, Some("camera".into())] {
                let mut old = RenderViewState::new(
                    RenderViewId(19),
                    RenderViewKind::Reflection,
                    target.clone(),
                );
                old.source_entity_id = camera;
                old.version = 42;
                old.layer_mask = Some("sprite2d".into());
                old.viewport.x = 7;
                old.viewport.y = 5;
                old.viewport.width = 640;
                old.viewport.height = 360;
                old.clear_color = [0.1, 0.2, 0.3, 0.4];
                let view = RenderView::from_legacy(&old);
                let lowered = view.to_legacy().unwrap();
                assert_eq!(lowered.payload, old);
                assert_eq!(lowered.provenance, &view.provenance);
                assert_eq!(RenderView::from_legacy(&lowered.payload), view);
                let encoded = serde_json::to_string(&RenderContract::new(view.clone())).unwrap();
                assert_eq!(
                    RenderContract::<RenderView>::decode(&encoded).unwrap().data,
                    view
                );
            }
        }
    }

    #[test]
    fn typed_camera_keeps_strict_ranges_and_no_guessed_legacy_matrix() {
        let mut view = RenderView::perspective(3, 640, 360, 60.0, 0.1, 1000.0);
        assert!(view.validate().is_ok());
        assert_eq!(view.execution_support(), ExecutionSupport::NativeGeometry);
        assert_eq!(
            view.to_legacy().unwrap_err().code,
            ContractErrorCode::UnsupportedLegacyConversion
        );
        let valid = serde_json::to_string(&RenderContract::new(view.clone())).unwrap();
        assert!(RenderContract::<RenderView>::decode(&valid).is_ok());
        for (fov, near, far) in [
            (0.0, 0.1, 10.0),
            (180.0, 0.1, 10.0),
            (60.0, 0.0, 10.0),
            (60.0, 2.0, 1.0),
            (f32::INFINITY, 1.0, 10.0),
        ] {
            view.projection = Projection::Perspective {
                fov_y_degrees: fov,
                near,
                far,
            };
            assert!(view.validate().is_err());
        }
        view.projection = Projection::Orthographic {
            size: 10.0,
            near: -1.0,
            far: 1.0,
        };
        assert!(view.validate().is_ok());
        view.projection = Projection::Orthographic {
            size: -1.0,
            near: -1.0,
            far: 1.0,
        };
        assert!(view.validate().is_err());
        view.projection = Projection::Orthographic {
            size: 10.0,
            near: -1.0,
            far: 1.0,
        };
        view.viewport[2] = 0;
        assert_eq!(
            view.validate().unwrap_err().code,
            ContractErrorCode::InvalidViewport
        );
    }

    #[test]
    fn view_visibility_is_per_pair_and_legacy_bounds_are_only_approximate() {
        let a = ViewVisibility {
            view_id: 1,
            instance_id: 8,
            visibility: Evaluation::Known(true),
            culling: CullingDecision::NotEvaluated,
        };
        let b = ViewVisibility {
            view_id: 2,
            instance_id: 8,
            visibility: Evaluation::Known(false),
            culling: CullingDecision::LayerExcluded,
        };
        assert_ne!(a.key(), b.key());
        let results = std::collections::BTreeMap::from([(a.key(), a), (b.key(), b)]);
        assert_eq!(results[&(1, 8)].visibility, Evaluation::Known(true));
        assert_eq!(results[&(2, 8)].visibility, Evaluation::Known(false));
        let bounds = BoundsMetadata::from_legacy(
            &engine_runtime::render_state::Bounds::from_transform(&Transform::identity()),
        );
        assert_eq!(bounds.quality, BoundsQuality::LegacyApproximation);
        assert_eq!(bounds.space, BoundsSpace::LegacyTransform);
    }

    #[test]
    fn diagnostic_off_is_lazy_summary_bounded_and_trace_is_explicit() {
        let error = ContractError::new(ContractErrorCode::AssetMissing, "reference");
        let provenance = Provenance::legacy_sprite("sprite-entity");
        let mut report = ContractDiagnostics::default();
        report.record(error, RenderDomain::World2d, &provenance, || {
            panic!("Off must not construct details")
        });
        assert!(report.entries.is_empty());
        let mut report = ContractDiagnostics::new(DiagnosticLevel::Summary);
        for _ in 0..SUMMARY_LIMIT {
            report.record(error, RenderDomain::World2d, &provenance, || {
                DiagnosticDetails {
                    actual: "missing texture".into(),
                    dependency_chain: vec!["dependency".into(); 30],
                }
            });
        }
        report.record(error, RenderDomain::World2d, &provenance, || {
            panic!("bounded summary must stop before details")
        });
        assert_eq!(report.entries.len(), SUMMARY_LIMIT);
        assert!(report.entries[0].dependency_chain.len() <= SUMMARY_DEPENDENCY_LIMIT);
        assert_eq!(
            report.entries[0].provenance.source_entity_id.as_deref(),
            Some("sprite-entity")
        );
        assert!(!report.entries[0].expected.is_empty());
        assert!(!report.entries[0].next_action.is_empty());
        let mut trace = ContractDiagnostics::new(DiagnosticLevel::Trace);
        trace.record(error, RenderDomain::World2d, &provenance, || {
            DiagnosticDetails {
                actual: "missing texture".into(),
                dependency_chain: vec!["dependency".into(); 30],
            }
        });
        assert_eq!(trace.entries[0].dependency_chain.len(), 30);
    }
}
