use engine_runtime::aui::*;
use engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION;
use engine_runtime::aui_theme::{AuiResolvedBrush, AuiThemeAsset, AuiThemeRegistry};
use engine_runtime::font_bundle::RuntimeFontBundleRegistry;
use engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry;

fn c2_document(nodes: Vec<AuiNode>) -> AuiDocument {
    let mut document = AuiDocument::new(
        "visual-c2",
        vec![AuiCanvas::screen_overlay("canvas", 320.0, 180.0, "root")],
        nodes,
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

#[test]
fn resolved_default_control_paint_reaches_ui_projection_data() {
    let mut button = AuiNode::new("root", AuiNodeKind::Button, AuiRect::stretch_full());
    button.interactable = true;
    let registry = AuiThemeRegistry::builtin();
    let resolved = registry
        .resolve_document(
            &c2_document(vec![button]),
            &AuiControlInteractionSnapshot::default(),
        )
        .unwrap();
    let layout = AuiLayoutEngine::layout(&resolved.document, 1);
    assert!(layout.diagnostics.is_empty(), "{:#?}", layout.diagnostics);
    let (draw, _) = AuiLayoutEngine::extract_draw_list(&resolved.document, &layout);
    let composition =
        AuiRendererBridge::build_composition_frame(1, &resolved.document, &layout, &draw);
    let item = &composition
        .stage(AuiCompositionStage::ScreenOverlay)
        .unwrap()
        .draw_items[0];
    let paint = item.paint.as_ref().expect("resolved paint");
    assert!(matches!(paint.background, AuiResolvedBrush::Solid { .. }));
    assert!(paint.radius > 0.0);
    assert!(paint.border.width > 0.0);
}

#[test]
fn rounded_clip_is_preserved_as_a_shape_chain_for_descendants() {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.clip_policy = AuiClipPolicy::Rounded;
    root.clip_radius = Some(18.0);
    root.children.push("child".into());
    let mut child = AuiNode::new(
        "child",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(-10.0, -10.0, 80.0, 80.0),
    );
    child.parent = Some("root".into());
    let registry = AuiThemeRegistry::builtin();
    let resolved = registry
        .resolve_document(
            &c2_document(vec![root, child]),
            &AuiControlInteractionSnapshot::default(),
        )
        .unwrap();
    let layout = AuiLayoutEngine::layout(&resolved.document, 1);
    let computed = layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "child")
        .unwrap();
    assert_eq!(computed.effective_clips.len(), 1);
    assert_eq!(computed.effective_clips[0].node_id, "root");
    assert_eq!(computed.effective_clips[0].radius, 18.0);
    assert!(computed.effective_clip_rect.is_some());
}

#[test]
fn nine_slice_brush_reaches_overlay_without_renderer_theme_lookup() {
    let theme: AuiThemeAsset = serde_json::from_value(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1",
        "assetId": "nine",
        "tokens": {},
        "styles": {
            "panel": {
                "role": "panel",
                "paint": {
                    "background": {
                        "kind": "nineSlice",
                        "asset": { "id": "frame", "type": "texture", "guid": "guid-frame" },
                        "tint": { "source": "value", "value": "#ffffffff" },
                        "slice": { "left": 8.0, "top": 10.0, "right": 12.0, "bottom": 14.0 }
                    },
                    "shadow": {
                        "color": { "source": "value", "value": "#00000080" },
                        "offsetX": { "source": "value", "value": 2.0 },
                        "offsetY": { "source": "value", "value": 3.0 },
                        "blur": { "source": "value", "value": 4.0 },
                        "spread": { "source": "value", "value": 1.0 }
                    }
                }
            }
        }
    }))
    .unwrap();
    let registry = AuiThemeRegistry::from_assets(vec![theme]).unwrap();
    let mut document = c2_document(vec![AuiNode::new(
        "root",
        AuiNodeKind::Panel,
        AuiRect::stretch_full(),
    )]);
    document.theme = Some(engine_runtime::runtime_package::RuntimeAssetRef {
        id: "nine".into(),
        asset_type: "aui-theme".into(),
        guid: Some("guid-nine".into()),
        sub_asset: None,
    });
    let resolved = registry
        .resolve_document(&document, &AuiControlInteractionSnapshot::default())
        .unwrap();
    let layout = AuiLayoutEngine::layout(&resolved.document, 1);
    let (draw, _) = AuiLayoutEngine::extract_draw_list(&resolved.document, &layout);
    let composition =
        AuiRendererBridge::build_composition_frame(1, &resolved.document, &layout, &draw);
    let paint = composition.stages[0].draw_items[0].paint.as_ref().unwrap();
    assert!(matches!(
        paint.background,
        AuiResolvedBrush::NineSlice { ref asset_id, .. } if asset_id == "frame"
    ));
    assert!(paint.shadow.visible());
}

#[test]
fn interaction_state_rebuilds_paint_without_rebuilding_layout() {
    let theme: AuiThemeAsset = serde_json::from_value(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1",
        "assetId": "interactive",
        "tokens": {},
        "styles": {
            "button": {
                "role": "button",
                "paint": {
                    "background": { "kind": "solid", "color": { "source": "value", "value": "#202020ff" } },
                    "foreground": { "source": "value", "value": "#ffffffff" }
                },
                "states": {
                    "hover": {
                        "background": { "kind": "solid", "color": { "source": "value", "value": "#404040ff" } },
                        "foreground": { "source": "value", "value": "#ff0000ff" }
                    }
                }
            }
        }
    }))
    .unwrap();
    let themes = AuiThemeRegistry::from_assets(vec![theme]).unwrap();
    let mut button = AuiNode::new("root", AuiNodeKind::Button, AuiRect::stretch_full());
    button.interactable = true;
    // This fixture verifies paint state, without a font bundle or text measurement.
    let mut document = c2_document(vec![button]);
    document.theme = Some(engine_runtime::runtime_package::RuntimeAssetRef {
        id: "interactive".into(),
        asset_type: "aui-theme".into(),
        guid: Some("guid-interactive".into()),
        sub_asset: None,
    });
    let mut c2_runtime = AuiC2Runtime::default();
    let fonts = RuntimeAuiFontAtlasRegistry::empty("theme-state-test");
    let font_bundles = RuntimeFontBundleRegistry::default();
    let mut output = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached_with_themes(
        &document,
        ProjectUiStateSnapshotOutput::new(
            "theme-state-test",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(1),
        ),
        &fonts,
        &font_bundles,
        &themes,
        None,
        &mut c2_runtime,
    );
    let original_layout = output.layout.clone();
    let original_solver_builds = c2_runtime.solver_build_count();
    let original_solver_stats = c2_runtime.layout_stats("canvas");
    let interaction = AuiInteractionResult {
        control_snapshot: AuiControlInteractionSnapshot {
            frame_id: 2,
            hovered_node: Some("root".into()),
            pointer_hover_capable: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut feedback_state =
        engine_runtime::aui_control_feedback::AuiControlFeedbackState::default();

    AuiRuntimePresenter::apply_control_feedback_with_fonts_and_themes(
        &mut output,
        &interaction,
        &mut feedback_state,
        16_667,
        &fonts,
        &font_bundles,
        &themes,
    );

    // Hover feedback changes final presentation geometry, not layout occupation.
    // Compare the layout contract and solver work rather than the hit/present snapshot.
    assert_eq!(output.layout.report, original_layout.report);
    assert_eq!(
        output.layout.computed_nodes.len(),
        original_layout.computed_nodes.len()
    );
    for (current, original) in output
        .layout
        .computed_nodes
        .iter()
        .zip(&original_layout.computed_nodes)
    {
        assert_eq!(current.node_id, original.node_id);
        assert_eq!(current.rect, original.rect);
        assert_eq!(current.effective_visible, original.effective_visible);
    }
    assert_eq!(c2_runtime.solver_build_count(), original_solver_builds);
    assert_eq!(c2_runtime.layout_stats("canvas"), original_solver_stats);
    assert_ne!(
        output.layout.computed_nodes[0].presentation,
        original_layout.computed_nodes[0].presentation
    );
    let paint = output
        .composition
        .stages
        .iter()
        .flat_map(|stage| stage.draw_items.iter())
        .find(|item| item.node_id == "root")
        .and_then(|item| item.paint.as_ref())
        .expect("button paint");
    assert_eq!(paint.foreground.as_deref(), Some("#ff0000ff"));
    assert_eq!(paint.background, AuiResolvedBrush::solid("#404040ff"));
}
