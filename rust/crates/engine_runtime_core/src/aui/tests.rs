use super::*;
use crate::aui_control_feedback::{AuiControlVisualOverride, AuiVisualOverrideSet};
use crate::font_bundle::{
    CookedFontBundleAsset, CookedFontBundleGlyph, CookedFontBundleKerning, CookedFontBundlePage,
    RuntimeFontBundleRegistry, RuntimeLoadedFontBundle, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
use crate::game_view_presentation::{
    GameViewExtent, GameViewPresentationModule, GameViewPresentationSpec, GameViewRect,
    GameViewScalePolicy,
};
use crate::input_mapping::{
    RuntimeInputEvent, RuntimeInputFrame, RuntimePointerButton, RuntimePointerDeviceKind,
    RuntimePointerEvent, RuntimePointerPhase,
};
use crate::runtime_package::{
    CookedFontAtlasAsset, CookedFontAtlasGlyph, RuntimeAuiFontAtlasRegistry,
    RuntimeLoadedFontAtlas, COOKED_FONT_ATLAS_SCHEMA_VERSION,
};

#[test]
fn aui_geometry_unrepresentable_parent_chain_is_diagnosed_without_panicking() {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.children = vec!["child".into()];
    root.presentation = Some(project_runtime_sdk::aui_presentation::NodePresentation {
        transform: project_runtime_sdk::aui_presentation::Transform2D {
            translation: [f32::MAX, 0.],
            ..Default::default()
        },
        ..Default::default()
    });
    let mut child = AuiNode::new(
        "child",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0., 0., 20., 20.),
    )
    .with_parent("root");
    child.presentation = root.presentation;
    let document = AuiDocument::new(
        "overflow",
        vec![AuiCanvas::screen_overlay("main", 300., 200., "root")],
        vec![root, child],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    assert!(layout
        .diagnostics
        .iter()
        .any(|d| d.code == "aui_presentation_invalid"));
    assert!(!layout.computed_nodes[1].effective_visible);
    assert!(layout.computed_nodes.iter().all(|n| {
        [n.rect.x, n.rect.y, n.rect.width, n.rect.height]
            .iter()
            .all(|v| v.is_finite())
    }));

    // Opposite terms cancel at the diagonal corner, but the other corners overflow.
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.children = vec!["child".into()];
    root.presentation = Some(project_runtime_sdk::aui_presentation::NodePresentation {
        transform: project_runtime_sdk::aui_presentation::Transform2D {
            scale: [1e38, 1.],
            pivot: [0.; 2],
            ..Default::default()
        },
        ..Default::default()
    });
    let mut child = AuiNode::new(
        "child",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0., 0., 10., 10.),
    )
    .with_parent("root");
    child.presentation = Some(project_runtime_sdk::aui_presentation::NodePresentation {
        transform: project_runtime_sdk::aui_presentation::Transform2D {
            rotation: std::f32::consts::FRAC_PI_4,
            pivot: [0.; 2],
            ..Default::default()
        },
        ..Default::default()
    });
    let document = AuiDocument::new(
        "corners",
        vec![AuiCanvas::screen_overlay("main", 1., 1., "root")],
        vec![root, child],
    );
    let layout = AuiLayoutEngine::layout(&document, 2);
    assert!(layout
        .diagnostics
        .iter()
        .any(|d| d.code == "aui_presentation_invalid" && d.node_id.as_deref() == Some("child")));
    assert!(!layout.computed_nodes[1].effective_visible);
}

#[test]
fn aui_geometry_rotated_mirrored_slider_uses_local_axis_for_value() {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.children = vec!["slider".into()];
    let mut slider = AuiNode::new(
        "slider",
        AuiNodeKind::Slider,
        AuiRect::fixed_position(60., 50., 100., 20.),
    )
    .with_parent("root");
    slider.slider_min = Some(0.);
    slider.slider_max = Some(1.);
    slider.slider_step = Some(0.01);
    slider.slider_value = Some(0.);
    slider.presentation = Some(project_runtime_sdk::aui_presentation::NodePresentation {
        transform: project_runtime_sdk::aui_presentation::Transform2D {
            rotation: std::f32::consts::FRAC_PI_2,
            scale: [-1., 2.],
            ..Default::default()
        },
        ..Default::default()
    });
    let mut document = AuiDocument::new(
        "slider",
        vec![AuiCanvas::screen_overlay("main", 300., 200., "root")],
        vec![root, slider],
    );
    document.schema_version = crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let node = &layout.computed_nodes[1];
    let matrix = project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(
        node.presentation.unwrap().local_to_canvas,
    )
    .unwrap();
    let point = matrix.transform_point([75., 10.]).unwrap();
    let lookup = AuiNodeLookup::new(
        &document,
        Arc::new(
            document
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (n.node_id.clone(), i))
                .collect(),
        ),
        Arc::new(vec![1]),
    );
    let mut state = AuiInteractionState::default();
    let mut commands = Vec::new();
    state.slider_values.insert(
        "slider".into(),
        AuiSliderValueState {
            source_value: 0.,
            value: 0.,
        },
    );
    assert!(AuiInteractionSystem::append_slider_pointer_value_change(
        &lookup,
        &layout,
        &mut state,
        &mut commands,
        0,
        Some("slider"),
        AuiPointer::new(point[0], point[1])
    ));
    assert!((state.slider_value("slider").unwrap() - 0.75).abs() < 1e-5);
}

#[test]
fn aui_geometry_affine_feedback_shares_hit_draw_and_inherited_clip_matrix() {
    use project_runtime_sdk::aui_presentation::{NodePresentation, Transform2D};
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.children = vec!["parent".into(), "sibling".into()];
    let mut parent = AuiNode::new(
        "parent",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(60., 50., 80., 60.),
    );
    parent.parent = Some("root".into());
    parent.children = vec!["child".into()];
    parent.clip_policy = AuiClipPolicy::Rounded;
    parent.clip_radius = Some(12.);
    let mut child = AuiNode::new("child", AuiNodeKind::Panel, AuiRect::stretch_full());
    child.parent = Some("parent".into());
    child.interactable = true;
    let mut sibling = AuiNode::new(
        "sibling",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(220., 20., 20., 20.),
    );
    sibling.parent = Some("root".into());
    let mut document = AuiDocument::new(
        "affine",
        vec![AuiCanvas::screen_overlay("main", 300., 200., "root")],
        vec![root, parent, child, sibling],
    );
    document.schema_version = crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
    document.nodes[1].presentation = Some(NodePresentation {
        transform: Transform2D {
            rotation: 0.6,
            scale: [-1.2, 0.8],
            pivot: [0.3, 0.7],
            ..Default::default()
        },
        opacity: 0.5,
        ..Default::default()
    });
    let base = Arc::new(AuiLayoutEngine::layout(&document, 1));
    let mut state = crate::aui_control_feedback::AuiControlFeedbackState::default();
    let mut overrides = AuiVisualOverrideSet::default();
    overrides.set(
        "parent",
        AuiControlVisualOverride {
            translation: AuiVec2::new(15., -8.),
            scale: 0.9,
            opacity_multiplier: 0.5,
            ..Default::default()
        },
    );
    let final_layout = compose_feedback_geometry(&document, &base, &overrides, &mut state);
    assert_eq!(
        final_layout.computed_nodes[3], base.computed_nodes[3],
        "feedback touched unrelated sibling"
    );
    let geometry = final_layout.computed_nodes[2].presentation.unwrap();
    let matrix = project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(
        geometry.local_to_canvas,
    )
    .unwrap();
    let center = matrix.transform_point([40., 30.]).unwrap();
    assert_eq!(
        AuiInteractionSystem::hit_test(&document, &final_layout, center[0], center[1])
            .hit_node
            .as_deref(),
        Some("child")
    );
    let corner = matrix.transform_point([1., 1.]).unwrap();
    let corner_hit = AuiInteractionSystem::hit_test(&document, &final_layout, corner[0], corner[1]);
    assert_ne!(corner_hit.hit_node.as_deref(), Some("child"));
    assert_eq!(corner_hit.clip_rejected_count, 1);
    assert!(
        !final_layout.computed_nodes[2].effective_clips[0].contains(AuiPointer {
            x: corner[0],
            y: corner[1],
        })
    );
    assert!(
        final_layout.computed_nodes[2].contains_presented_point(AuiPointer {
            x: center[0],
            y: center[1]
        })
    );
    assert!(
        final_layout.computed_nodes[2].effective_clips[0].contains(AuiPointer {
            x: center[0],
            y: center[1]
        })
    );
    assert!((geometry.modulation[3] - 0.25).abs() < 1e-6);
    assert_eq!(
        final_layout.computed_nodes[2].effective_clips[0].transform,
        Some(
            final_layout.computed_nodes[1]
                .presentation
                .unwrap()
                .vertex_transform
        )
    );
    let (draw, _) = AuiLayoutEngine::extract_draw_list(&document, &final_layout);
    let composition =
        AuiRendererBridge::build_composition_frame(1, &document, &final_layout, &draw);
    assert_eq!(
        composition
            .stage(AuiCompositionStage::ScreenOverlay)
            .unwrap()
            .draw_items
            .iter()
            .find(|i| i.node_id == "child")
            .unwrap()
            .presentation,
        Some(geometry)
    );
    let repeated = compose_feedback_geometry(&document, &base, &overrides, &mut state);
    assert_eq!(
        repeated.computed_nodes[2].presentation,
        Some(geometry),
        "feedback accumulated across frames"
    );
    assert!(Arc::ptr_eq(
        &compose_feedback_geometry(
            &document,
            &base,
            &AuiVisualOverrideSet::default(),
            &mut state
        ),
        &base
    ));
}

#[test]
fn b9_present_statistics_reuse_and_external_edit_fallback() {
    let document = scroll_document();
    let original = AuiRuntimePresenter::present_package_smoke(&document, 1);
    assert!(matches!(
        original.base_draw_report(),
        std::borrow::Cow::Borrowed(_)
    ));
    let expected =
        AuiLayoutEngine::extract_draw_list(&original.resolved_document, &original.layout).1;
    assert_eq!(*original.base_draw_report(), expected);
    assert!(expected.effective_clip_item_count > 0);
    assert!(expected.culled_draw_item_count > 0);
    assert_eq!(expected.scrollbar_visible_count, 1);
    assert_ne!(
        expected.culled_draw_item_count,
        original.overlay.report.culled_draw_item_count
    );

    let mut edited = original.clone();
    Arc::make_mut(&mut edited.layout).computed_nodes.clear();
    assert!(matches!(
        edited.base_draw_report(),
        std::borrow::Cow::Owned(_)
    ));
    assert_eq!(
        *edited.base_draw_report(),
        AuiLayoutEngine::extract_draw_list(&edited.resolved_document, &edited.layout).1
    );
    assert_ne!(*edited.base_draw_report(), expected);

    let mut edited = original.clone();
    Arc::make_mut(&mut edited.resolved_document).nodes.clear();
    assert!(matches!(
        edited.base_draw_report(),
        std::borrow::Cow::Owned(_)
    ));
    assert_eq!(
        *edited.base_draw_report(),
        AuiLayoutEngine::extract_draw_list(&edited.resolved_document, &edited.layout).1
    );
    assert_ne!(*edited.base_draw_report(), expected);
    assert!(matches!(
        original.base_draw_report(),
        std::borrow::Cow::Borrowed(_)
    ));

    let json = serde_json::to_value(&original).unwrap();
    assert!(json.get("baseDrawReport").is_none());
    let restored: AuiRuntimePresentOutput = serde_json::from_value(json).unwrap();
    assert_eq!(restored, original);
    assert!(matches!(
        restored.base_draw_report(),
        std::borrow::Cow::Owned(_)
    ));
    assert_eq!(*restored.base_draw_report(), expected);
}

#[test]
fn b9_drawing_and_composition_share_indices_without_stale_slot_data() {
    let (mut document, _) = sample_document();
    document.nodes.push(AuiNode::new(
        "extra",
        AuiNodeKind::Panel,
        AuiRect::stretch_full(),
    ));
    document.canvases.push(AuiCanvas::screen_overlay(
        "extra-canvas",
        100.,
        100.,
        "extra",
    ));
    let mut layout = AuiLayoutEngine::layout(&document, 1);
    let mut cache = AuiDrawingCache::default();
    let check = |document: &AuiDocument, layout: &AuiLayoutResult, cache: &mut AuiDrawingCache| {
        let actual_draw = AuiLayoutEngine::extract_draw_list_cached(document, layout, None, cache);
        assert_eq!(
            actual_draw,
            AuiLayoutEngine::extract_draw_list(document, layout)
        );
        let before = cache.stats;
        let actual = AuiRendererBridge::build_composition_frame_cached(
            1,
            document,
            layout,
            &actual_draw.0,
            cache,
        );
        assert_eq!(
            cache.stats.index_rebuilds, before.index_rebuilds,
            "composition must use the extraction indices"
        );
        assert_eq!(
            actual,
            AuiRendererBridge::build_composition_frame(1, document, layout, &actual_draw.0)
        );
        actual
    };
    let first = check(&document, &layout, &mut cache);
    assert_eq!(cache.stats.index_rebuilds, 3);
    assert_eq!(
        cache.stats.index_entries_built,
        (document.nodes.len() + layout.computed_nodes.len() + document.canvases.len()) as u64
    );
    let before = cache.stats;
    layout.computed_nodes[1].rect.x += 50.;
    document.nodes[3].text = Some("new content".into());
    document.canvases[0].layer += 3;
    let changed = check(&document, &layout, &mut cache);
    assert_ne!(changed, first);
    assert_eq!(cache.stats.index_rebuilds, before.index_rebuilds);
    assert_eq!(cache.stats.index_entries_built, before.index_entries_built);
    document.nodes.swap(0, 1);
    check(&document, &layout, &mut cache);
    assert_eq!(cache.stats.index_rebuilds, before.index_rebuilds + 1);
    document.canvases.swap(0, 1);
    layout.computed_nodes.swap(0, 1);
    check(&document, &layout, &mut cache);
    assert_eq!(cache.stats.index_rebuilds, before.index_rebuilds + 3);
    let (draw, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    layout.computed_nodes[0].node_id = "renamed".into();
    document.canvases.clear();
    let missing =
        AuiRendererBridge::build_composition_frame_cached(2, &document, &layout, &draw, &mut cache);
    assert_eq!(
        missing,
        AuiRendererBridge::build_composition_frame(2, &document, &layout, &draw)
    );
    assert!(!missing.report.diagnostics.is_empty());
    layout.computed_nodes =
        vec![layout.computed_nodes[0].clone(); drawing_cache::MAX_DRAW_NODES + 1];
    AuiRendererBridge::build_composition_frame_cached(3, &document, &layout, &draw, &mut cache);
    assert_eq!(
        cache.indices.computed.get("renamed"),
        None,
        "oversized index must not be retained"
    );
    document.nodes.clear();
    layout.computed_nodes.clear();
    check(&document, &layout, &mut cache);
    assert!(cache.indices.nodes.get("root").is_none());
}

#[test]
fn e4_drawing_delta_replays_visibility_and_falls_back_when_identity_is_untrusted() {
    use crate::aui_hit_test_index::{AuiInteractionIndexDelta, AuiInteractionIndexUpdate};
    use std::collections::VecDeque;

    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["label"]);
    let label = AuiNode::new(
        "label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(10., 10., 100., 20.),
    )
    .with_parent("root")
    .with_text("delta");
    let document = AuiDocument::new(
        "e4-drawing-delta",
        vec![AuiCanvas::screen_overlay("main", 200., 100., "root")],
        vec![root, label],
    );
    let mut layout = AuiLayoutEngine::layout(&document, 1);
    let generation = 7;
    layout.interaction_index_update = AuiInteractionIndexUpdate {
        generation,
        revision: 0,
        node_count: layout.computed_nodes.len(),
        history: Arc::default(),
    };
    let mut cache = AuiDrawingCache::default();

    let check = |layout: &AuiLayoutResult, cache: &mut AuiDrawingCache| {
        let actual = AuiLayoutEngine::extract_draw_list_cached(&document, layout, None, cache);
        assert_eq!(
            actual,
            AuiLayoutEngine::extract_draw_list(&document, layout)
        );
    };
    check(&layout, &mut cache);

    layout.computed_nodes[1].effective_visible = false;
    layout.report.visible_node_count -= 1;
    layout.interaction_index_update = AuiInteractionIndexUpdate {
        generation,
        revision: 1,
        node_count: 2,
        history: Arc::new(VecDeque::from([AuiInteractionIndexDelta {
            revision: 1,
            slots: Arc::from([1]),
        }])),
    };
    let before = cache.stats;
    check(&layout, &mut cache);
    assert_eq!(cache.stats.nodes_compared - before.nodes_compared, 1);
    assert_eq!(cache.stats.nodes_visited - before.nodes_visited, 1);
    assert!(cache.nodes[1].is_none());

    layout.computed_nodes[1].effective_visible = true;
    layout.report.visible_node_count += 1;
    layout.interaction_index_update.revision = 2;
    Arc::make_mut(&mut layout.interaction_index_update.history).push_back(
        AuiInteractionIndexDelta {
            revision: 2,
            slots: Arc::from([1]),
        },
    );
    let before = cache.stats;
    check(&layout, &mut cache);
    assert_eq!(cache.stats.nodes_compared - before.nodes_compared, 1);
    assert!(cache.nodes[1].is_some());

    layout.computed_nodes[0].rect.x += 3.;
    layout.computed_nodes[1].rect.x += 5.;
    layout.interaction_index_update.revision = 4;
    Arc::make_mut(&mut layout.interaction_index_update.history).extend([
        AuiInteractionIndexDelta {
            revision: 3,
            slots: Arc::from([0]),
        },
        AuiInteractionIndexDelta {
            revision: 4,
            slots: Arc::from([1]),
        },
    ]);
    let before = cache.stats;
    check(&layout, &mut cache);
    assert_eq!(cache.stats.nodes_compared - before.nodes_compared, 2);

    layout.computed_nodes[1].rect.y += 7.;
    layout.interaction_index_update = AuiInteractionIndexUpdate {
        generation,
        revision: 6,
        node_count: 2,
        history: Arc::new(VecDeque::from([AuiInteractionIndexDelta {
            revision: 6,
            slots: Arc::from([1]),
        }])),
    };
    let before = cache.stats;
    check(&layout, &mut cache);
    assert_eq!(cache.stats.nodes_compared - before.nodes_compared, 2);

    layout.interaction_index_update = AuiInteractionIndexUpdate {
        generation: generation.saturating_add(1),
        revision: 0,
        node_count: 2,
        history: Arc::default(),
    };
    let before = cache.stats;
    check(&layout, &mut cache);
    assert_eq!(cache.stats.nodes_compared - before.nodes_compared, 2);

    layout.interaction_index_update = AuiInteractionIndexUpdate::default();
    let before = cache.stats;
    check(&layout, &mut cache);
    assert_eq!(cache.stats.nodes_compared - before.nodes_compared, 2);
}

#[test]
fn b9_single_stage_overlay_shares_sorted_items_and_falls_back_for_external_order() {
    let (document, _) = sample_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (draw, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let mut frame = AuiRendererBridge::build_composition_frame(1, &document, &layout, &draw);
    let mut overlay = frame.to_overlay_frame();
    assert!(Arc::ptr_eq(
        &overlay.draw_items,
        &frame.stages[0].draw_items
    ));
    let unchanged = frame.stages[0].draw_items[0].rect;
    Arc::make_mut(&mut Arc::make_mut(&mut overlay.draw_items)[0])
        .rect
        .x += 30.;
    assert_eq!(frame.stages[0].draw_items[0].rect, unchanged);
    Arc::make_mut(&mut frame.stages[0].draw_items).reverse();
    let mut expected = frame.stages[0].draw_items.as_ref().clone();
    expected.sort_by_key(|i| i.sort_key);
    let restored = frame.to_overlay_frame();
    assert_eq!(&*restored.draw_items, &expected);
    assert!(!Arc::ptr_eq(
        &restored.draw_items,
        &frame.stages[0].draw_items
    ));
    let mut other = frame.stages[0].clone();
    other.stage = AuiCompositionStage::Modal;
    for item in Arc::make_mut(&mut other.draw_items) {
        Arc::make_mut(item).item_id.push_str("-modal");
    }
    frame.stages.push(other);
    let mut expected: Vec<_> = frame
        .stages
        .iter()
        .flat_map(|s| s.draw_items.iter().cloned())
        .collect();
    expected.sort_by_key(|i| i.sort_key);
    assert_eq!(
        &*frame.to_overlay_frame().draw_items,
        &expected,
        "equal sort keys retain original cross-stage order"
    );
    frame.stages.clear();
    assert!(frame.to_overlay_frame().draw_items.is_empty());
}

#[test]
fn b8_draw_fragments_match_uncached_for_edits_parent_feedback_and_removal() {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    let mut nodes = Vec::new();
    for i in 0..1000 {
        let id = format!("n{i}");
        root.children.push(id.clone());
        nodes.push(
            AuiNode::new(
                &id,
                AuiNodeKind::Button,
                AuiRect::fixed_position(10., 10., 50., 30.),
            )
            .with_parent("root")
            .with_text("A")
            .with_style(AuiStyle::text("#ffffff", 16.)),
        );
    }
    nodes.insert(0, root);
    nodes[1].children.push("child".into());
    nodes.push(
        AuiNode::new(
            "child",
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 10., 10.),
        )
        .with_parent("n0")
        .with_text("V"),
    );
    let mut document = AuiDocument::new(
        "b8",
        vec![AuiCanvas::screen_overlay("main", 200., 200., "root")],
        nodes,
    );
    let mut cache = AuiDrawingCache::default();
    let mut overrides = AuiVisualOverrideSet::default();
    let check = |document: &AuiDocument,
                 overrides: &AuiVisualOverrideSet,
                 cache: &mut AuiDrawingCache| {
        let layout = AuiLayoutEngine::layout(document, 1);
        let actual =
            AuiLayoutEngine::extract_draw_list_cached(document, &layout, Some(overrides), cache);
        assert_eq!(
            actual,
            AuiLayoutEngine::extract_draw_list_with_visual_overrides(document, &layout, overrides)
        );
        let mut composition = AuiRendererBridge::build_composition_frame_cached(
            1, document, &layout, &actual.0, cache,
        );
        cache.publish_projection_hint(&mut composition, None);
        (actual.0, composition)
    };
    let (first_draw, first_composition) = check(&document, &overrides, &mut cache);
    let before = cache.stats;
    document.nodes[500].text = Some("changed".into());
    let (changed_draw, changed_composition) = check(&document, &overrides, &mut cache);
    assert_eq!(cache.stats.nodes_built - before.nodes_built, 1);
    assert_eq!(cache.stats.nodes_reused - before.nodes_reused, 1001);
    assert_eq!(changed_draw.commands.len(), 2002);
    assert!(Arc::ptr_eq(
        &first_draw.commands[1],
        &changed_draw.commands[1]
    ));
    assert!(!Arc::ptr_eq(
        &first_draw.commands[1000],
        &changed_draw.commands[1000]
    ));
    assert!(Arc::ptr_eq(
        &first_composition.stages[0].draw_items[1],
        &changed_composition.stages[0].draw_items[1]
    ));
    assert!(!Arc::ptr_eq(
        &first_composition.stages[0].draw_items[1000],
        &changed_composition.stages[0].draw_items[1000]
    ));
    let projection_delta = changed_composition
        .projection_delta()
        .expect("bounded production composition must publish its retained delta");
    assert_eq!(
        projection_delta.stages[&AuiCompositionStage::ScreenOverlay]
            .changed_slots
            .as_slice(),
        &[1000, 1001]
    );
    assert_eq!(
        cache.stats.command_payloads_reused - before.command_payloads_reused,
        2000
    );
    assert_eq!(
        cache.stats.command_payloads_built - before.command_payloads_built,
        2
    );
    assert_eq!(
        cache.stats.item_payloads_reused - before.item_payloads_reused,
        2000
    );
    assert_eq!(
        cache.stats.item_payloads_built - before.item_payloads_built,
        2
    );
    let draw_wire = serde_json::to_value(&changed_draw).unwrap();
    assert!(draw_wire["commands"].is_array());
    assert_eq!(
        serde_json::from_value::<AuiDrawList>(draw_wire).unwrap(),
        changed_draw
    );
    let composition_wire = serde_json::to_value(&changed_composition).unwrap();
    assert!(composition_wire["stages"][0]["drawItems"].is_array());
    assert_eq!(
        serde_json::from_value::<AuiCompositionFrame>(composition_wire).unwrap(),
        changed_composition
    );
    let before = cache.stats;
    overrides.set(
        "n0",
        AuiControlVisualOverride {
            scale: 0.8,
            ..Default::default()
        },
    );
    check(&document, &overrides, &mut cache);
    assert_eq!(cache.stats.nodes_built - before.nodes_built, 2);
    assert_eq!(cache.stats.nodes_reused - before.nodes_reused, 1000);
    document.nodes[1].clip_policy = AuiClipPolicy::Rect;
    check(&document, &overrides, &mut cache);
    document.nodes[1].visible = false;
    check(&document, &overrides, &mut cache);
    document.nodes[1].visible = true;
    document.nodes[0].children.swap(0, 4);
    check(&document, &overrides, &mut cache);
    document.nodes[0].children.retain(|id| id != "n0");
    document
        .nodes
        .retain(|node| node.node_id != "n0" && node.node_id != "child");
    check(&document, &overrides, &mut cache);
    assert!(cache
        .nodes
        .iter()
        .flatten()
        .all(|n| n.node.node_id != "n0" && n.node.node_id != "child"));
    let mut layout = AuiLayoutEngine::layout(&document, 1);
    layout.computed_nodes =
        vec![layout.computed_nodes[0].clone(); drawing_cache::MAX_DRAW_NODES + 1];
    let actual = AuiLayoutEngine::extract_draw_list_cached(&document, &layout, None, &mut cache);
    assert_eq!(
        actual,
        AuiLayoutEngine::extract_draw_list(&document, &layout)
    );
    assert!(cache.nodes.is_empty());
}

#[test]
fn b8_c2_glyph_fragments_match_uncached_and_invalidate_actual_dependencies() {
    use crate::aui_paragraph::{ParagraphGlyph, ParagraphLayout};
    let mut fonts = v2_font_registry();
    let mut overlay = v2_text_overlay(16., AuiFontRasterPolicy::Bitmap);
    let mut sibling = overlay.draw_items[0].as_ref().clone();
    sibling.node_id = "sibling".into();
    sibling.item_id = "text-2".into();
    Arc::make_mut(&mut overlay.draw_items).push(Arc::new(sibling));
    overlay.report.text_count = 2;
    let paragraph = Arc::new(ParagraphLayout {
        identity: "p".into(),
        text_digest: "t".into(),
        font_identity: "f".into(),
        constraint_width: 300.,
        width: 16.,
        height: 20.,
        first_baseline: Some(16.),
        lines: Vec::new(),
        truncated: false,
        overflow: crate::aui_layout_schema::TextOverflow::Clip,
        glyphs: vec![ParagraphGlyph {
            font_bundle_id: "font-ui".into(),
            face_id: "face-ui".into(),
            glyph_id: 1,
            cluster_start: 0,
            cluster_end: 1,
            x: 0.,
            y: 16.,
            advance: 8.,
            has_outline: true,
            ink_bounds: None,
            fallback_used: false,
            font_size: 16.,
            raster_policy: AuiFontRasterPolicy::Bitmap,
            color: None,
        }],
    });
    let mut paragraphs = HashMap::from([
        ("label".into(), paragraph.clone()),
        ("sibling".into(), paragraph),
    ]);
    let mut cache = AuiDrawingCache::default();
    let check = |overlay: &AuiOverlayFrame,
                 fonts: &RuntimeFontBundleRegistry,
                 paragraphs: &HashMap<String, Arc<ParagraphLayout>>,
                 cache: &mut AuiDrawingCache| {
        let actual = build_c2_text_glyph_plan_cached(overlay, fonts, None, paragraphs, Some(cache));
        assert_eq!(
            actual,
            build_c2_text_glyph_plan(overlay, fonts, None, paragraphs)
        );
        actual.unwrap()
    };
    let mut previous = check(&overlay, &fonts, &paragraphs, &mut cache);
    assert_eq!(previous.quads.len(), 2);
    for phase in 0..4 {
        let before = cache.stats;
        let item = Arc::make_mut(&mut Arc::make_mut(&mut overlay.draw_items)[0]);
        match phase {
            0 => item.rect.x += 10.,
            1 => item.color = Some("#ff0000".into()),
            2 => {
                item.effective_clip_rect = Some(AuiComputedRect {
                    x: 0.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                })
            }
            _ => {
                Arc::make_mut(paragraphs.get_mut("label").unwrap()).glyphs[0].color =
                    Some("#0f0".into())
            }
        }
        let changed = check(&overlay, &fonts, &paragraphs, &mut cache);
        assert_eq!(cache.stats.text_items_built - before.text_items_built, 1);
        assert_eq!(cache.stats.text_items_reused - before.text_items_reused, 1);
        assert!(!Arc::ptr_eq(&previous.quads[0], &changed.quads[0]));
        assert!(Arc::ptr_eq(&previous.quads[1], &changed.quads[1]));
        assert_eq!(
            cache.stats.quad_payloads_built - before.quad_payloads_built,
            1
        );
        assert_eq!(
            cache.stats.quad_payloads_reused - before.quad_payloads_reused,
            1
        );
        assert_eq!(
            cache.stats.glyph_hash_leaves_built - before.glyph_hash_leaves_built,
            1
        );
        assert_eq!(
            cache.stats.glyph_hash_leaves_reused - before.glyph_hash_leaves_reused,
            1
        );
        assert_eq!(
            cache
                .pending_glyph_groups
                .as_ref()
                .expect("cached glyph build must publish changed groups")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["text-1"]
        );
        let wire = serde_json::to_value(&changed).unwrap();
        assert!(wire["quads"].is_array());
        assert_eq!(
            serde_json::from_value::<AuiTextGlyphPlan>(wire).unwrap(),
            changed
        );
        previous = changed;
    }
    let before = cache.stats;
    let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "b8".into(),
        target_id: "b8".into(),
        target_extent: GameViewExtent::new(400, 400),
        display_rect: GameViewRect::new(0., 0., 400., 400.),
        scale_policy: GameViewScalePolicy::Contain,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 200, 200)],
    })
    .unwrap();
    let scaled = build_c2_text_glyph_plan_cached(
        &overlay,
        &fonts,
        Some(&presentation),
        &paragraphs,
        Some(&mut cache),
    );
    assert_eq!(
        scaled,
        build_c2_text_glyph_plan(&overlay, &fonts, Some(&presentation), &paragraphs)
    );
    assert_eq!(cache.stats.text_items_built - before.text_items_built, 2);
    check(&overlay, &fonts, &paragraphs, &mut cache);
    let before = cache.stats;
    Arc::make_mut(&mut fonts.bundles_by_id)
        .get_mut("font-ui")
        .unwrap()
        .metadata
        .glyphs[0]
        .pixel_rect[0] += 1;
    check(&overlay, &fonts, &paragraphs, &mut cache);
    assert_eq!(cache.stats.text_items_built - before.text_items_built, 2);
    paragraphs.remove("label");
    assert!(check(&overlay, &fonts, &paragraphs, &mut cache).unsupported_glyph_count > 0);
    Arc::make_mut(paragraphs.get_mut("sibling").unwrap()).glyphs[0].glyph_id = 0;
    assert!(check(&overlay, &fonts, &paragraphs, &mut cache)
        .quads
        .is_empty());
    let mut large = paragraphs["sibling"].as_ref().clone();
    large.glyphs[0].glyph_id = 1;
    large.glyphs = vec![large.glyphs[0].clone(); drawing_cache::MAX_GLYPH_QUADS + 1];
    paragraphs.insert("sibling".into(), Arc::new(large));
    check(&overlay, &fonts, &paragraphs, &mut cache);
    assert!(cache.glyphs.is_empty() && cache.fonts.is_none());
    overlay.report.text_count = 0;
    assert!(
        build_c2_text_glyph_plan_cached(&overlay, &fonts, None, &paragraphs, Some(&mut cache))
            .is_none()
    );
    assert!(cache.glyphs.is_empty());
}

#[test]
fn aui_feedback_schema_selector_round_trip_is_stable() {
    for selection in [
        AuiFeedbackSelection::auto(),
        AuiFeedbackSelection::none(),
        AuiFeedbackSelection::profile("ink.button"),
    ] {
        let json = serde_json::to_string(&selection).unwrap();
        let round_trip: AuiFeedbackSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip, selection);
    }
    assert_eq!(AuiFeedbackSelection::default().as_str(), "auto");
}

#[test]
fn aui_feedback_schema_registry_rejects_unknown_fields() {
    let invalid_registry = serde_json::json!({
        "motion_scale_permille": 1000,
        "profiles": [],
        "arbitrary_script": "pulse_forever"
    });
    assert!(serde_json::from_value::<AuiInteractionFeedbackRegistry>(invalid_registry).is_err());

    let mut profile =
        serde_json::to_value(AuiInteractionFeedbackProfile::new("ink.button")).unwrap();
    profile
        .as_object_mut()
        .unwrap()
        .insert("keyframes".to_string(), serde_json::json!([]));
    assert!(serde_json::from_value::<AuiInteractionFeedbackProfile>(profile).is_err());
}

fn sample_document() -> (AuiDocument, AuiAssetManifest) {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["panel", "icon", "title"])
        .with_style(AuiStyle::color("#101820"));
    let panel = AuiNode::new(
        "panel",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(100.0, 80.0, 320.0, 180.0),
    )
    .with_parent("root")
    .with_style(AuiStyle::color("#223344"));
    let icon = AuiNode::new(
        "icon",
        AuiNodeKind::Image,
        AuiRect::fixed_position(120.0, 100.0, 64.0, 64.0),
    )
    .with_parent("root")
    .with_image("icon_asset");
    let title = AuiNode::new(
        "title",
        AuiNodeKind::Text,
        AuiRect::fixed_position(200.0, 112.0, 220.0, 40.0),
    )
    .with_parent("root")
    .with_text("Start")
    .with_style(AuiStyle::text("#ffffff", 24.0));
    let canvas = AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root");
    let manifest = AuiAssetManifest::new(
        "main_assets",
        vec![AuiAssetManifestEntry::image(
            "icon_asset",
            "asset://ui/icon.png",
            vec!["icon".to_string()],
        )],
    );
    (
        AuiDocument::new("sample", vec![canvas], vec![root, panel, icon, title]),
        manifest,
    )
}

#[test]
fn gate_e_c2_raster_selection_skips_unrelated_metadata() {
    let mut fonts = v2_font_registry();
    let bundle = Arc::make_mut(&mut fonts.bundles_by_id)
        .get_mut("font-ui")
        .unwrap();
    let template = bundle.metadata.glyphs[0].clone();
    for id in 100..5100 {
        let mut unrelated = template.clone();
        unrelated.glyph_id = id;
        bundle.metadata.glyphs.push(unrelated);
    }
    let mut cache = AuiDrawingCache::default();
    let index = cache.raster_index(&fonts).unwrap();
    assert!(Arc::ptr_eq(&index, &cache.raster_index(&fonts).unwrap()));
    let bundle = fonts.default_bundle().unwrap();
    let (selected, visited) = resolve_c2_raster_glyph(
        bundle,
        "face-ui",
        1,
        FontBundleRenderMode::BitmapR8,
        20,
        Some(&index),
    );
    assert_eq!(
        selected.unwrap().pixel_size,
        16,
        "equal-distance ties retain metadata order"
    );
    assert!(
        visited <= 3,
        "one glyph selection scanned unrelated font metadata: {visited}"
    );
    for face in ["face-ui", "unknown"] {
        for id in [1, 2, 4, 500, 65535] {
            for mode in [
                FontBundleRenderMode::BitmapR8,
                FontBundleRenderMode::MsdfRgba8,
            ] {
                for size in [8, 20, 28, 64] {
                    assert_eq!(
                        resolve_c2_raster_glyph(bundle, face, id, mode, size, Some(&index)).0,
                        resolve_c2_raster_glyph(bundle, face, id, mode, size, None).0
                    );
                }
            }
        }
    }
    let old_fonts = fonts.clone();
    Arc::make_mut(&mut fonts.bundles_by_id)
        .get_mut("font-ui")
        .unwrap()
        .metadata
        .glyphs[0]
        .pixel_rect[0] = 9;
    let replacement = cache.raster_index(&fonts).unwrap();
    assert!(!Arc::ptr_eq(&index, &replacement));
    let bundle = fonts.default_bundle().unwrap();
    assert_eq!(
        resolve_c2_raster_glyph(
            bundle,
            "face-ui",
            1,
            FontBundleRenderMode::BitmapR8,
            16,
            Some(&replacement)
        )
        .0
        .unwrap()
        .pixel_rect[0],
        9
    );
    assert_eq!(
        resolve_c2_raster_glyph(
            old_fonts.default_bundle().unwrap(),
            "face-ui",
            1,
            FontBundleRenderMode::BitmapR8,
            16,
            Some(&index)
        )
        .0
        .unwrap()
        .pixel_rect[0],
        0
    );
    let oversized = Arc::make_mut(&mut fonts.bundles_by_id)
        .get_mut("font-ui")
        .unwrap();
    oversized
        .metadata
        .glyphs
        .resize(drawing_cache::MAX_GLYPH_QUADS + 1, template);
    assert!(
        cache.raster_index(&fonts).is_none(),
        "oversized fonts retain the uncached fallback"
    );
}

fn v2_font_registry() -> RuntimeFontBundleRegistry {
    let pages = vec![
        CookedFontBundlePage {
            page_index: 0,
            render_mode: FontBundleRenderMode::BitmapR8,
            format: "r8Unorm".to_string(),
            width: 64,
            height: 64,
            byte_len: 4096,
            sha256: "sha256:bitmap".to_string(),
            payload_path: "bitmap.r8".to_string(),
        },
        CookedFontBundlePage {
            page_index: 1,
            render_mode: FontBundleRenderMode::MsdfRgba8,
            format: "rgba8Unorm".to_string(),
            width: 64,
            height: 64,
            byte_len: 16384,
            sha256: "sha256:msdf".to_string(),
            payload_path: "msdf.rgba8".to_string(),
        },
    ];
    let mut glyphs = Vec::new();
    for (index, character) in ['A', 'V', '中'].into_iter().enumerate() {
        for (render_mode, pixel_size, page_index, y) in [
            (FontBundleRenderMode::BitmapR8, 16, 0, 0),
            (FontBundleRenderMode::BitmapR8, 24, 0, 16),
            (FontBundleRenderMode::BitmapR8, 32, 0, 40),
            (FontBundleRenderMode::MsdfRgba8, 64, 1, 0),
        ] {
            glyphs.push(CookedFontBundleGlyph {
                font_family_id: "family-ui".to_string(),
                font_face_id: "face-ui".to_string(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id: index as u16 + 1,
                codepoint: u32::from(character),
                render_mode,
                pixel_size,
                page_index,
                pixel_rect: [index as u32 * 8, y, 8, 12],
                bearing_x: 0,
                bearing_y: 12,
                plane_bounds_per_em_millionths: None,
                advance_per_em_millionths: 600_000,
            });
        }
    }
    for (pixel_size, y) in [(16, 0), (24, 16), (32, 40)] {
        glyphs.push(CookedFontBundleGlyph {
            font_family_id: "family-ui".to_string(),
            font_face_id: "face-ui".to_string(),
            style: FontBundleStyle::Normal,
            weight: 400,
            glyph_id: 4,
            codepoint: u32::from(' '),
            render_mode: FontBundleRenderMode::BitmapR8,
            pixel_size,
            page_index: 0,
            pixel_rect: [24, y, 0, 0],
            bearing_x: 0,
            bearing_y: 0,
            plane_bounds_per_em_millionths: None,
            advance_per_em_millionths: 300_000,
        });
    }
    let metadata = CookedFontBundleAsset {
        schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
        font_bundle_id: "font-ui".to_string(),
        font_stack_id: "stack-ui".to_string(),
        generation: 7,
        max_bitmap_pages: 2,
        max_msdf_pages: 2,
        legacy_mode: false,
        fallback_used: false,
        quality_gate_eligible: true,
        pages,
        glyphs,
        kerning_adjustments: vec![CookedFontBundleKerning {
            font_face_id: "face-ui".to_string(),
            left_glyph_id: 1,
            right_glyph_id: 2,
            adjustment_per_em_millionths: -100_000,
        }],
        font_faces: Vec::new(),
        shaping_recipe: None,
        bundle_digest: "sha256:test".to_string(),
    };
    let mut registry = RuntimeFontBundleRegistry::default();
    registry.default_ui_font_bundle_id = Some("font-ui".to_string());
    std::sync::Arc::make_mut(&mut registry.bundles_by_id).insert(
        "font-ui".to_string(),
        RuntimeLoadedFontBundle {
            metadata,
            page_payloads: vec![vec![0; 4096], vec![0; 16384]],
            font_face_sources: Vec::new(),
        },
    );
    registry
}

fn v2_text_overlay(font_size: f32, raster_policy: AuiFontRasterPolicy) -> AuiOverlayFrame {
    AuiOverlayFrame {
        frame_index: 1,
        draw_items: vec![Arc::new(AuiOverlayDrawItem {
            presentation: None,
            item_id: "text-1".to_string(),
            canvas_id: "main".to_string(),
            composition_stage: AuiCompositionStage::ScreenOverlay,
            node_id: "label".to_string(),
            item_kind: AuiOverlayItemKind::Text,
            rect: AuiComputedRect {
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 80.0,
            },
            effective_clip_rect: None,
            effective_clips: Vec::new(),
            paint: None,
            color: Some("#fff".to_string()),
            asset_id: None,
            text: Some("AV中".to_string()),
            font_size: Some(font_size),
            font: Some(AuiFontStyle {
                font_bundle_id: Some("font-ui".to_string()),
                font_family_id: Some("family-ui".to_string()),
                style: AuiFontStyleKind::Normal,
                weight: 400,
                raster_policy,
            }),
            mesh: None,
            sort_key: AuiOverlaySortKey {
                canvas_layer: 0,
                canvas_sorting_order: 0,
                tree_order: 0,
            },
        })]
        .into(),
        report: AuiRenderReport {
            draw_command_count: 1,
            text_count: 1,
            ..AuiRenderReport::default()
        },
        glyph_plan: None,
    }
}

#[test]
fn gate_b_shaped_quads_use_clusters_metrics_and_selected_face() {
    use crate::font_bundle::RuntimePackageSourceFontFace;
    let bytes = include_bytes!("../../../../resources/editor/fonts/NotoSansSC-VF.ttf").to_vec();
    let text = "ffi a\u{301}中文，01Agpy";
    let source = RuntimePackageSourceFontFace {
        font_face_id: "real-face".into(),
        face_index: 0,
        source_digest: crate::canonical_digest::sha256_prefixed(&bytes),
        bytes,
    };
    let runs = shape_stack(&[&source], text).unwrap();
    assert!(runs.iter().map(|run| run.items.len()).sum::<usize>() < text.chars().count());
    let mut registry = v2_font_registry();
    let bundle = std::sync::Arc::make_mut(&mut registry.bundles_by_id)
        .get_mut("font-ui")
        .unwrap();
    let template = bundle.metadata.glyphs[0].clone();
    bundle.metadata.glyphs.clear();
    bundle.font_face_sources.push(source);
    for mode in [
        FontBundleRenderMode::BitmapR8,
        FontBundleRenderMode::MsdfRgba8,
    ] {
        for size in [14, 18, 24, 32, 48, 64] {
            for shaped in runs.iter().flat_map(|run| &run.items) {
                let mut glyph = template.clone();
                glyph.font_face_id = "real-face".into();
                glyph.glyph_id = shaped.glyph_id as u16;
                glyph.render_mode = mode;
                glyph.pixel_size = size;
                glyph.page_index = if mode == FontBundleRenderMode::BitmapR8 {
                    0
                } else {
                    1
                };
                glyph.bearing_y = i32::from(size) / 2;
                glyph.codepoint = u32::MAX; // prevents accidental codepoint resolver success
                glyph.plane_bounds_per_em_millionths = (mode == FontBundleRenderMode::MsdfRgba8)
                    .then_some([0, 500_000, 500_000, 500_000]);
                bundle.metadata.glyphs.push(glyph);
            }
        }
    }
    // Give the default bundle an unrelated face to catch accidental default-face shaping.
    let mut selected = bundle.clone();
    selected.metadata.font_bundle_id = "selected".into();
    let mut wrong_face = selected.metadata.glyphs[0].clone();
    wrong_face.font_face_id = "different-face-same-glyph-id".into();
    wrong_face.bearing_y = 10_000;
    selected.metadata.glyphs.insert(0, wrong_face);
    std::sync::Arc::make_mut(&mut registry.bundles_by_id).insert("selected".into(), selected);
    Arc::make_mut(&mut registry.bundles_by_id)
        .get_mut("font-ui")
        .unwrap()
        .font_face_sources
        .clear();
    for size in [14., 18., 24., 32., 48., 64.] {
        for policy in [AuiFontRasterPolicy::Bitmap, AuiFontRasterPolicy::Msdf] {
            let mut overlay = v2_text_overlay(size, policy);
            let item = Arc::make_mut(&mut Arc::make_mut(&mut overlay.draw_items)[0]);
            item.text = Some(text.into());
            item.node_id = "button:label".into();
            item.font.as_mut().unwrap().font_bundle_id = Some("selected".into());
            item.rect.width = 2000.;
            item.rect.height = 160.;
            let line = layout_line(&runs, size, item.rect.height, true);
            let plan = build_text_glyph_plan_from_bundles(&overlay, &registry).unwrap();
            assert_eq!(plan.unsupported_glyph_count, 0);
            assert!(plan.rendered_glyph_count < text.chars().count());
            assert!(plan
                .quads
                .iter()
                .all(|quad| quad.glyph_id.starts_with("real-face:")));
            assert!((plan.quads[0].rect.y + size * 0.5 - line.baseline).abs() < 0.001);
            assert!(plan.quads.iter().all(|quad| !quad.clipped));
            Arc::make_mut(&mut Arc::make_mut(&mut overlay.draw_items)[0]).effective_clip_rect =
                Some(AuiComputedRect {
                    x: 0.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                });
            assert!(
                build_text_glyph_plan_from_bundles(&overlay, &registry)
                    .unwrap()
                    .clipped_glyph_count
                    > 0
            );
        }
    }
}

#[test]
fn d3_virtual_list_text_uses_the_materialized_paragraph_identity() {
    use crate::aui_paragraph::{ParagraphGlyph, ParagraphLayout};

    let fonts = v2_font_registry();
    let mut overlay = v2_text_overlay(16.0, AuiFontRasterPolicy::Bitmap);
    let node_id = "inventory:virtual-item:0:item-template-text";
    let item = Arc::make_mut(&mut Arc::make_mut(&mut overlay.draw_items)[0]);
    item.node_id = node_id.into();
    item.text = Some("A".into());
    let paragraph = Arc::new(ParagraphLayout {
        identity: "virtual-paragraph".into(),
        text_digest: "virtual-text".into(),
        font_identity: "virtual-font".into(),
        constraint_width: 300.0,
        width: 8.0,
        height: 20.0,
        first_baseline: Some(16.0),
        lines: Vec::new(),
        truncated: false,
        overflow: crate::aui_layout_schema::TextOverflow::Clip,
        glyphs: vec![ParagraphGlyph {
            font_bundle_id: "font-ui".into(),
            face_id: "face-ui".into(),
            glyph_id: 1,
            cluster_start: 0,
            cluster_end: 1,
            x: 0.0,
            y: 16.0,
            advance: 8.0,
            has_outline: true,
            ink_bounds: None,
            fallback_used: false,
            font_size: 16.0,
            raster_policy: AuiFontRasterPolicy::Bitmap,
            color: None,
        }],
    });
    let plan = build_c2_text_glyph_plan(
        &overlay,
        &fonts,
        None,
        &HashMap::from([(node_id.into(), paragraph)]),
    )
    .unwrap();
    assert_eq!(plan.rendered_glyph_count, 1, "{plan:#?}");
    assert_eq!(plan.unsupported_glyph_count, 0, "{plan:#?}");
}

#[test]
fn d3_auto_hybrid_uses_nearest_bitmap_when_bundle_has_no_msdf_glyph() {
    use crate::aui_paragraph::{ParagraphGlyph, ParagraphLayout};

    let mut fonts = v2_font_registry();
    let bundle = Arc::make_mut(&mut fonts.bundles_by_id)
        .get_mut("font-ui")
        .unwrap();
    bundle
        .metadata
        .pages
        .retain(|page| page.render_mode == FontBundleRenderMode::BitmapR8);
    bundle
        .metadata
        .glyphs
        .retain(|glyph| glyph.render_mode == FontBundleRenderMode::BitmapR8);
    bundle.page_payloads.truncate(1);

    let mut overlay = v2_text_overlay(12.0, AuiFontRasterPolicy::AutoHybrid);
    let item = Arc::make_mut(&mut Arc::make_mut(&mut overlay.draw_items)[0]);
    item.text = Some("A".into());
    let paragraph = Arc::new(ParagraphLayout {
        identity: "bitmap-only-paragraph".into(),
        text_digest: "bitmap-only-text".into(),
        font_identity: "bitmap-only-font".into(),
        constraint_width: 300.0,
        width: 8.0,
        height: 20.0,
        first_baseline: Some(12.0),
        lines: Vec::new(),
        truncated: false,
        overflow: crate::aui_layout_schema::TextOverflow::Clip,
        glyphs: vec![ParagraphGlyph {
            font_bundle_id: "font-ui".into(),
            face_id: "face-ui".into(),
            glyph_id: 1,
            cluster_start: 0,
            cluster_end: 1,
            x: 0.0,
            y: 12.0,
            advance: 8.0,
            has_outline: true,
            ink_bounds: None,
            fallback_used: false,
            font_size: 12.0,
            raster_policy: AuiFontRasterPolicy::AutoHybrid,
            color: None,
        }],
    });

    let plan = build_c2_text_glyph_plan(
        &overlay,
        &fonts,
        None,
        &HashMap::from([("label".into(), paragraph)]),
    )
    .unwrap();
    assert_eq!(plan.rendered_glyph_count, 1, "{plan:#?}");
    assert_eq!(plan.unsupported_glyph_count, 0, "{plan:#?}");
    assert_eq!(plan.quads[0].render_mode, FontBundleRenderMode::BitmapR8);
}

#[test]
fn d3_slider_retained_value_updates_the_presented_control() {
    use project_runtime_sdk::aui::{UiProperty, UiValue};

    let mut slider = AuiNode::new(
        "speed",
        AuiNodeKind::Slider,
        AuiRect::fixed_position(0.0, 0.0, 200.0, 24.0),
    )
    .with_slider(0.5, 0.5, 2.0, 0.25);
    apply_retained_value(&mut slider, UiProperty::Value, &UiValue::Number(1.37), true);
    assert_eq!(slider.slider_value, Some(1.25));
}

#[test]
fn d3_slider_click_after_hidden_parent_becomes_visible_emits_value_changed() {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["settings"]);
    let mut settings = AuiNode::new(
        "settings",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(100.0, 100.0, 500.0, 300.0),
    )
    .with_parent("root")
    .with_children(["speed"]);
    settings.visible = false;
    let speed = AuiNode::new(
        "speed",
        AuiNodeKind::Slider,
        AuiRect::fixed_position(20.0, 20.0, 400.0, 48.0),
    )
    .with_parent("settings")
    .with_interactable(true)
    .with_slider(1.0, 1.0, 3.0, 1.0)
    .with_action(AuiActionRef::value_changed("settings.speed"));
    let mut document = AuiDocument::new(
        "slider-after-hidden-parent",
        vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
        vec![root, settings, speed],
    );
    document.schema_version = crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.to_string();
    let mut state = AuiInteractionState::default();
    let hidden_layout = AuiLayoutEngine::layout(&document, 1);
    AuiInteractionSystem::process_with_state(
        &document,
        &hidden_layout,
        &pointer_frame(Vec::new()),
        &mut state,
        AuiInteractionConfig::default(),
    );

    document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "settings")
        .unwrap()
        .visible = true;
    let visible_layout = AuiLayoutEngine::layout(&document, 2);
    let result = AuiInteractionSystem::process_with_state(
        &document,
        &visible_layout,
        &pointer_frame(vec![
            unified_mouse(RuntimePointerPhase::Move, 500.0, 140.0),
            unified_mouse(RuntimePointerPhase::Down, 500.0, 140.0),
            unified_mouse(RuntimePointerPhase::Up, 500.0, 140.0),
        ]),
        &mut state,
        AuiInteractionConfig::default(),
    );

    let action = result
        .actions
        .iter()
        .find(|action| action.action_id == "settings.speed")
        .unwrap_or_else(|| {
            panic!(
                "slider click emitted no ValueChanged: \
                     result={result:#?} state={state:#?} layout={visible_layout:#?}"
            )
        });
    assert_eq!(action.event, AuiActionEvent::ValueChanged);
    let payload: AuiControlValuePayload =
        serde_json::from_str(action.payload.as_deref().unwrap()).unwrap();
    assert_eq!(payload.value, AuiControlValue::Number(3.0));
}

#[test]
fn aui_font_style_round_trips_stack_override_style_weight_and_policy() {
    let style = AuiStyle {
        color: None,
        text_color: Some("#fff".to_string()),
        font_size: Some(40.0),
        font: Some(AuiFontStyle {
            font_bundle_id: Some("font-ui".to_string()),
            font_family_id: Some("family-ui".to_string()),
            style: AuiFontStyleKind::Italic,
            weight: 700,
            raster_policy: AuiFontRasterPolicy::Msdf,
        }),
    };
    let round_trip: AuiStyle =
        serde_json::from_slice(&serde_json::to_vec(&style).unwrap()).unwrap();
    assert_eq!(round_trip, style);
}

#[test]
fn aui_text_glyph_plan_uses_v2_bundle_chinese_mode_page_and_kerning() {
    let registry = v2_font_registry();
    let bitmap = build_text_glyph_plan_from_bundles(
        &v2_text_overlay(16.0, AuiFontRasterPolicy::AutoHybrid),
        &registry,
    )
    .unwrap();
    assert_eq!(bitmap.requested_glyph_count, 3);
    assert_eq!(bitmap.rendered_glyph_count, 3);
    assert_eq!(bitmap.unsupported_glyph_count, 0);
    assert_eq!(bitmap.quads[1].rect.x, 8.0);
    assert!(bitmap
        .quads
        .iter()
        .all(|quad| quad.render_mode == FontBundleRenderMode::BitmapR8 && quad.page_index == 0));

    let msdf = build_text_glyph_plan_from_bundles(
        &v2_text_overlay(40.0, AuiFontRasterPolicy::AutoHybrid),
        &registry,
    )
    .unwrap();
    assert!(msdf
        .quads
        .iter()
        .all(|quad| quad.render_mode == FontBundleRenderMode::MsdfRgba8 && quad.page_index == 1));
}

#[test]
fn aui_text_glyph_plan_uses_bitmap_metrics_for_whitespace_without_msdf_outline() {
    let registry = v2_font_registry();
    let mut overlay = v2_text_overlay(40.0, AuiFontRasterPolicy::AutoHybrid);
    Arc::make_mut(&mut Arc::make_mut(&mut overlay.draw_items)[0]).text = Some("A V".to_string());

    let plan = build_text_glyph_plan_from_bundles(&overlay, &registry).unwrap();

    assert_eq!(plan.requested_glyph_count, 3);
    assert_eq!(plan.rendered_glyph_count, 3);
    assert_eq!(plan.unsupported_glyph_count, 0);
    assert_eq!(plan.quads[0].render_mode, FontBundleRenderMode::MsdfRgba8);
    assert_eq!(plan.quads[1].codepoint, u32::from(' '));
    assert_eq!(plan.quads[1].render_mode, FontBundleRenderMode::BitmapR8);
    assert_eq!(plan.quads[1].rect.width, 0.0);
    assert_eq!(plan.quads[2].render_mode, FontBundleRenderMode::MsdfRgba8);
    assert!(plan.quads[2].rect.x > plan.quads[1].rect.x);
}

#[test]
fn aui_text_glyph_plan_uses_target_physical_pixels_for_auto_hybrid() {
    let registry = v2_font_registry();
    let presentation_720 = crate::game_view_presentation::GameViewPresentationModule::resolve(
        crate::game_view_presentation::GameViewPresentationSpec {
            session_id: "font-720".to_string(),
            target_id: "portrait".to_string(),
            target_extent: crate::game_view_presentation::GameViewExtent::new(720, 1280),
            display_rect: crate::game_view_presentation::GameViewRect::new(0.0, 0.0, 720.0, 1280.0),
            scale_policy: crate::game_view_presentation::GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![crate::game_view_presentation::CanvasReferenceFact::new(
                "main", 1080, 1920,
            )],
        },
    )
    .unwrap();
    let presentation_1080 = crate::game_view_presentation::GameViewPresentationModule::resolve(
        crate::game_view_presentation::GameViewPresentationSpec {
            session_id: "font-1080".to_string(),
            target_id: "portrait".to_string(),
            target_extent: crate::game_view_presentation::GameViewExtent::new(1080, 1920),
            display_rect: crate::game_view_presentation::GameViewRect::new(
                0.0, 0.0, 1080.0, 1920.0,
            ),
            scale_policy: crate::game_view_presentation::GameViewScalePolicy::Contain,
            surface_generation: 1,
            presentation_revision: 1,
            canvas_references: vec![crate::game_view_presentation::CanvasReferenceFact::new(
                "main", 1080, 1920,
            )],
        },
    )
    .unwrap();

    let plan_720 = build_text_glyph_plan_from_bundles_for_presentation(
        &v2_text_overlay(36.0, AuiFontRasterPolicy::AutoHybrid),
        &registry,
        Some(&presentation_720),
    )
    .unwrap();
    assert!(plan_720.quads.iter().all(|quad| {
        quad.render_mode == FontBundleRenderMode::BitmapR8 && quad.uv_rect[1] > 0.25
    }));

    let plan_1080 = build_text_glyph_plan_from_bundles_for_presentation(
        &v2_text_overlay(36.0, AuiFontRasterPolicy::AutoHybrid),
        &registry,
        Some(&presentation_1080),
    )
    .unwrap();
    assert!(plan_1080
        .quads
        .iter()
        .all(|quad| quad.render_mode == FontBundleRenderMode::MsdfRgba8));

    let body_720 = build_text_glyph_plan_from_bundles_for_presentation(
        &v2_text_overlay(24.0, AuiFontRasterPolicy::AutoHybrid),
        &registry,
        Some(&presentation_720),
    )
    .unwrap();
    assert!(body_720.quads.iter().all(|quad| {
        quad.render_mode == FontBundleRenderMode::BitmapR8 && quad.uv_rect[1] > 0.0
    }));
}

#[test]
fn aui_text_kerning_changes_second_glyph_position() {
    let registry = v2_font_registry();
    let plan = build_text_glyph_plan_from_bundles(
        &v2_text_overlay(16.0, AuiFontRasterPolicy::Bitmap),
        &registry,
    )
    .unwrap();
    let unkerned_second_x = 16.0 * 600_000.0 / 1_000_000.0;
    assert_eq!(plan.quads[1].rect.x, 8.0);
    assert!(plan.quads[1].rect.x < unkerned_second_x);
}

#[test]
fn aui_text_glyph_plan_applies_baseline_and_bearings() {
    let registry = v2_font_registry();
    let plan = build_text_glyph_plan_from_bundles(
        &v2_text_overlay(16.0, AuiFontRasterPolicy::Bitmap),
        &registry,
    )
    .unwrap();
    // The cooked fixture uses a non-zero vertical bearing. The glyph quad
    // must therefore be placed from the shared baseline, rather than at
    // the text rectangle's top edge.
    assert!(plan.quads.iter().all(|quad| quad.rect.y > 0.0));
    assert!(plan.quads.iter().all(|quad| quad.rect.y < 16.0));
}

fn interaction_document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["button_a", "button_b", "label"]);
    let button_a = AuiNode::new(
        "button_a",
        AuiNodeKind::Button,
        AuiRect::fixed_position(100.0, 100.0, 240.0, 80.0),
    )
    .with_parent("root")
    .with_interactable(true);
    let button_b = AuiNode::new(
        "button_b",
        AuiNodeKind::Button,
        AuiRect::fixed_position(140.0, 120.0, 240.0, 80.0),
    )
    .with_parent("root")
    .with_interactable(true)
    .with_action(AuiActionRef::click("ui.pause"));
    let label = AuiNode::new(
        "label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(20.0, 20.0, 100.0, 30.0),
    )
    .with_parent("root")
    .with_text("Info");
    AuiDocument::new(
        "interaction",
        vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
        vec![root, button_a, button_b, label],
    )
}

fn pointer_frame(events: Vec<RuntimeInputEvent>) -> RuntimeInputFrame {
    let mut frame = RuntimeInputFrame::new(7, "game-view");
    frame.events = events;
    frame
}

fn unified_mouse(phase: RuntimePointerPhase, x: f32, y: f32) -> RuntimeInputEvent {
    let button = matches!(
        phase,
        RuntimePointerPhase::Down
            | RuntimePointerPhase::Up
            | RuntimePointerPhase::Held
            | RuntimePointerPhase::Cancel
    )
    .then_some(RuntimePointerButton::Primary);
    RuntimeInputEvent::Pointer {
        pointer: RuntimePointerEvent::mouse(phase, 0, x, y, button),
    }
}

fn unified_touch(phase: RuntimePointerPhase, pointer_id: u64, x: f32, y: f32) -> RuntimeInputEvent {
    RuntimeInputEvent::Pointer {
        pointer: RuntimePointerEvent::touch(phase, pointer_id, x, y),
    }
}

fn drag_document() -> AuiDocument {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children([
        "drag_source",
        "drop_target",
        "other_target",
    ]);
    let drag_source = AuiNode::new(
        "drag_source",
        AuiNodeKind::Button,
        AuiRect::fixed_position(40.0, 40.0, 80.0, 80.0),
    )
    .with_parent("root")
    .with_draggable()
    .with_action(AuiActionRef::click("ui.source_click"))
    .with_action(AuiActionRef::drag_start("ui.drag_start"))
    .with_action(AuiActionRef::drag_move("ui.drag_move"))
    .with_action(AuiActionRef::drop("ui.drop"));
    let drop_target = AuiNode::new(
        "drop_target",
        AuiNodeKind::Button,
        AuiRect::fixed_position(180.0, 40.0, 100.0, 100.0),
    )
    .with_parent("root")
    .with_drop_target();
    let other_target = AuiNode::new(
        "other_target",
        AuiNodeKind::Button,
        AuiRect::fixed_position(320.0, 40.0, 100.0, 100.0),
    )
    .with_parent("root")
    .with_interactable(true);
    AuiDocument::new(
        "drag-doc",
        vec![AuiCanvas::screen_overlay("main", 640.0, 480.0, "root")],
        vec![root, drag_source, drop_target, other_target],
    )
}

fn modal_document() -> AuiDocument {
    let background_root = AuiNode::new(
        "background_root",
        AuiNodeKind::Panel,
        AuiRect::stretch_full(),
    )
    .with_children(["background_button"]);
    let background_button = AuiNode::new(
        "background_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 20.0, 160.0, 60.0),
    )
    .with_parent("background_root")
    .with_interactable(true)
    .with_action(AuiActionRef::click("game.fire"));
    let modal_root = AuiNode::new(
        "modal_root",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(200.0, 120.0, 300.0, 240.0),
    )
    .with_children(["modal_button_a", "modal_button_b"]);
    let modal_button_a = AuiNode::new(
        "modal_button_a",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 20.0, 120.0, 50.0),
    )
    .with_parent("modal_root")
    .with_interactable(true)
    .with_action(AuiActionRef::focus("ui.focus_a"));
    let modal_button_b = AuiNode::new(
        "modal_button_b",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 90.0, 120.0, 50.0),
    )
    .with_parent("modal_root")
    .with_interactable(true)
    .with_action(AuiActionRef::focus("ui.focus_b"))
    .with_action(AuiActionRef::cancel("ui.cancel"));
    let overlay = AuiCanvas::screen_overlay("overlay", 800.0, 600.0, "background_root");
    let mut modal = AuiCanvas::screen_overlay("modal", 800.0, 600.0, "modal_root");
    modal.composition_stage = AuiCompositionStage::Modal;
    modal.layer = 10;
    AuiDocument::new(
        "modal-doc",
        vec![overlay, modal],
        vec![
            background_root,
            background_button,
            modal_root,
            modal_button_a,
            modal_button_b,
        ],
    )
}

fn scroll_document() -> AuiDocument {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["list"]);
    let list = AuiNode::new(
        "list",
        AuiNodeKind::ScrollView,
        AuiRect::fixed_position(10.0, 10.0, 120.0, 100.0),
    )
    .with_parent("root")
    .with_children(["item_0", "item_1", "item_2"])
    .with_action(AuiActionRef::scroll("ui.scroll"));
    let item_0 = AuiNode::new(
        "item_0",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 120.0, 80.0),
    )
    .with_parent("list");
    let item_1 = AuiNode::new(
        "item_1",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 90.0, 120.0, 80.0),
    )
    .with_parent("list");
    let item_2 = AuiNode::new(
        "item_2",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 180.0, 120.0, 80.0),
    )
    .with_parent("list");
    AuiDocument::new(
        "scroll-doc",
        vec![AuiCanvas::screen_overlay("main", 320.0, 240.0, "root")],
        vec![root, list, item_0, item_1, item_2],
    )
}

fn navigation_text_entry_document() -> AuiDocument {
    let main_root = AuiNode::new("main_root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["play_button", "settings_button"]);
    let play_button = AuiNode::new(
        "play_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 20.0, 160.0, 40.0),
    )
    .with_parent("main_root")
    .with_interactable(true)
    .with_action(AuiActionRef::submit("ui.play"));
    let settings_button = AuiNode::new(
        "settings_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 80.0, 160.0, 40.0),
    )
    .with_parent("main_root")
    .with_interactable(true)
    .with_action(AuiActionRef::submit("ui.settings"));
    let pause_root = AuiNode::new(
        "pause_root",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(220.0, 40.0, 260.0, 180.0),
    )
    .with_children(["name_input", "resume_button"]);
    let name_input = AuiNode::new(
        "name_input",
        AuiNodeKind::InputField,
        AuiRect::fixed_position(20.0, 20.0, 180.0, 36.0),
    )
    .with_parent("pause_root")
    .with_interactable(true)
    .with_text("A")
    .with_action(AuiActionRef::text_changed("ui.name_changed"))
    .with_action(AuiActionRef::text_submitted("ui.name_submitted"))
    .with_action(AuiActionRef::text_cancelled("ui.name_cancelled"));
    let resume_button = AuiNode::new(
        "resume_button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 76.0, 180.0, 36.0),
    )
    .with_parent("pause_root")
    .with_interactable(true)
    .with_action(AuiActionRef::submit("ui.resume"));
    let mut main_canvas = AuiCanvas::screen_overlay("main", 640.0, 360.0, "main_root");
    main_canvas.default_focus_node_id = Some("play_button".to_string());
    let mut pause_canvas = AuiCanvas::screen_overlay("pause", 640.0, 360.0, "pause_root");
    pause_canvas.composition_stage = AuiCompositionStage::Modal;
    pause_canvas.layer = 10;
    pause_canvas.visible = false;
    pause_canvas.screen_id = Some("pause_screen".to_string());
    pause_canvas.default_focus_node_id = Some("name_input".to_string());
    pause_canvas.cancel_action_id = Some("ui.pause_cancel".to_string());
    AuiDocument::new(
        "navigation-text-entry-doc",
        vec![main_canvas, pause_canvas],
        vec![
            main_root,
            play_button,
            settings_button,
            pause_root,
            name_input,
            resume_button,
        ],
    )
}

fn font_registry_for_text(text: &str) -> RuntimeAuiFontAtlasRegistry {
    let mut chars = text.chars().collect::<std::collections::BTreeSet<_>>();
    chars.insert('?');
    let glyphs = chars
        .into_iter()
        .map(|ch| CookedFontAtlasGlyph {
            codepoint: ch as u32,
            glyph_id: format!("builtin-{:04X}", ch as u32),
            uv_rect: [0.0, 0.0, 0.625, 0.875],
            pixel_rect: [0, 0, 5, 7],
            bearing_x: 0.0,
            bearing_y: 7.0,
            advance: 6.0,
            page_index: 0,
        })
        .collect::<Vec<_>>();
    let metadata = CookedFontAtlasAsset {
        schema_version: COOKED_FONT_ATLAS_SCHEMA_VERSION.to_string(),
        font_atlas_id: "ui-default-cmin".to_string(),
        font_asset_id: "font-main".to_string(),
        font_source_kind: "engine_builtin_cooked_fallback".to_string(),
        font_asset_status: "placeholder".to_string(),
        atlas_image_path: "fonts/ui-default-cmin.fontatlas.r8".to_string(),
        atlas_format: "r8Alpha".to_string(),
        atlas_width: 8,
        atlas_height: 8,
        atlas_generation: 1,
        atlas_alpha_byte_len: 64,
        glyphs,
        fallback_used: true,
        diagnostics: Vec::new(),
    };
    let mut registry = RuntimeAuiFontAtlasRegistry::empty("test-package");
    registry.default_ui_font_atlas_id = Some(metadata.font_atlas_id.clone());
    std::sync::Arc::make_mut(&mut registry.atlases_by_id).insert(
        metadata.font_atlas_id.clone(),
        RuntimeLoadedFontAtlas {
            metadata,
            atlas_alpha: vec![255; 64],
        },
    );
    registry
}

#[test]
fn aui_layout_resolves_anchor_offsets_for_screen_overlay() {
    let (document, _) = sample_document();

    let layout = AuiLayoutEngine::layout(&document, 7);

    let root = layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "root")
        .unwrap();
    let panel = layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "panel")
        .unwrap();
    assert_eq!(
        root.rect,
        AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0
        }
    );
    assert_eq!(
        panel.rect,
        AuiComputedRect {
            x: 100.0,
            y: 80.0,
            width: 320.0,
            height: 180.0
        }
    );
    assert_eq!(layout.report.frame, 7);
    assert_eq!(layout.report.visible_node_count, 4);
}

#[test]
fn aui_draw_list_keeps_tree_order_for_panel_image_text() {
    let (document, manifest) = sample_document();
    assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);

    let layout = AuiLayoutEngine::layout(&document, 0);
    let (draw_list, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);

    assert_eq!(report.draw_command_count, 4);
    assert_eq!(report.image_count, 1);
    assert_eq!(report.text_count, 1);
    assert!(
        matches!(draw_list.commands[0].as_ref(), AuiDrawCommand::DrawRect { node_id, .. } if node_id == "root")
    );
    assert!(
        matches!(draw_list.commands[1].as_ref(), AuiDrawCommand::DrawRect { node_id, .. } if node_id == "panel")
    );
    assert!(
        matches!(draw_list.commands[2].as_ref(), AuiDrawCommand::DrawImage { node_id, .. } if node_id == "icon")
    );
    assert!(
        matches!(draw_list.commands[3].as_ref(), AuiDrawCommand::DrawText { node_id, .. } if node_id == "title")
    );
}

#[test]
fn aui_render_report_exposes_projection_summary() {
    let (document, manifest) = sample_document();
    assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);
    let layout = AuiLayoutEngine::layout(&document, 0);

    let (_, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let projection = report.projection_summary();

    assert_eq!(projection.kind, ProjectionKind::Ui);
    assert_eq!(projection.source_domain, ProjectionDomain::Ui);
    assert_eq!(projection.target_domain, ProjectionDomain::Render);
    assert_eq!(projection.projected_count, 4);
}

#[test]
fn aui_validation_reports_missing_image_asset() {
    let (document, _) = sample_document();
    let manifest = AuiAssetManifest::new("empty", Vec::new());

    let report = AuiLayoutEngine::validate(&document, Some(&manifest));

    assert!(!report.ok);
    assert_eq!(report.missing_asset_count, 1);
    assert!(report
        .report_items
        .iter()
        .any(|item| item.code == "missing_image_asset"
            && item.node_id.as_deref() == Some("icon")
            && item.asset_id.as_deref() == Some("icon_asset")));
}

#[test]
fn aui_node_kind_field_mismatch_validation() {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["icon_with_text", "text_with_image"]);
    let icon_with_text = AuiNode::new(
        "icon_with_text",
        AuiNodeKind::Image,
        AuiRect::fixed_position(10.0, 10.0, 64.0, 64.0),
    )
    .with_parent("root")
    .with_image("icon_asset")
    .with_text("Ignored");
    let text_with_image = AuiNode::new(
        "text_with_image",
        AuiNodeKind::Text,
        AuiRect::fixed_position(80.0, 10.0, 120.0, 32.0),
    )
    .with_parent("root")
    .with_text("Text")
    .with_image("text_icon_asset");
    let document = AuiDocument::new(
        "field_mismatch",
        vec![AuiCanvas::screen_overlay("main", 320.0, 200.0, "root")],
        vec![root, icon_with_text, text_with_image],
    );
    let manifest = AuiAssetManifest::new(
        "assets",
        vec![
            AuiAssetManifestEntry::image(
                "icon_asset",
                "asset://ui/icon.png",
                vec!["icon_with_text".to_string()],
            ),
            AuiAssetManifestEntry::image(
                "text_icon_asset",
                "asset://ui/text-icon.png",
                vec!["text_with_image".to_string()],
            ),
        ],
    );

    let report = AuiLayoutEngine::validate(&document, Some(&manifest));

    assert!(report.ok);
    assert_eq!(report.warning_count, 2);
    assert!(report.report_items.iter().any(|item| {
        item.code == "image_node_text_field_ignored"
            && item.node_id.as_deref() == Some("icon_with_text")
    }));
    assert!(report.report_items.iter().any(|item| {
        item.code == "text_node_image_field_ignored"
            && item.node_id.as_deref() == Some("text_with_image")
    }));
}

#[test]
fn aui_validation_rejects_full_screen_single_image_ui() {
    let canvas = AuiCanvas::screen_overlay("main", 1920.0, 1080.0, "screenshot");
    let screenshot = AuiNode::new("screenshot", AuiNodeKind::Image, AuiRect::stretch_full())
        .with_image("full_screen_art");
    let document = AuiDocument::new("bad_ui", vec![canvas], vec![screenshot]);
    let manifest = AuiAssetManifest::new(
        "assets",
        vec![AuiAssetManifestEntry::image(
            "full_screen_art",
            "asset://ui/screenshot.png",
            vec!["screenshot".to_string()],
        )],
    );

    let report = AuiLayoutEngine::validate(&document, Some(&manifest));

    assert!(!report.ok);
    assert!(report.full_screen_image_rejected);
    assert!(report
        .report_items
        .iter()
        .any(|item| item.code == "full_screen_image_ui_rejected"));
}

#[test]
fn aui_asset_manifest_maps_asset_to_node() {
    let (_, manifest) = sample_document();

    let asset = manifest
        .assets
        .iter()
        .find(|asset| asset.asset_id == "icon_asset")
        .unwrap();

    assert_eq!(asset.asset_ref, "asset://ui/icon.png");
    assert_eq!(asset.used_by_nodes, vec!["icon".to_string()]);
    assert_eq!(asset.text_policy, AuiAssetTextPolicy::RuntimeText);
}

#[test]
fn aui_renderer_bridge_converts_draw_list_to_overlay_frame() {
    let (document, manifest) = sample_document();
    assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);
    let layout = AuiLayoutEngine::layout(&document, 3);
    let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

    let overlay = AuiRendererBridge::build_overlay_frame(3, &draw_list);

    assert_eq!(overlay.frame_index, 3);
    assert_eq!(overlay.report.draw_command_count, 4);
    assert_eq!(overlay.report.image_count, 1);
    assert_eq!(overlay.report.text_count, 1);
    assert_eq!(overlay.draw_items[2].item_kind, AuiOverlayItemKind::Image);
    assert_eq!(
        overlay.draw_items[2].asset_id.as_deref(),
        Some("icon_asset")
    );
    assert_eq!(overlay.draw_items[3].item_kind, AuiOverlayItemKind::Text);
    assert_eq!(overlay.draw_items[3].text.as_deref(), Some("Start"));
}

#[test]
fn aui_renderer_bridge_keeps_overlay_tree_order() {
    let (document, manifest) = sample_document();
    assert!(AuiLayoutEngine::validate(&document, Some(&manifest)).ok);
    let layout = AuiLayoutEngine::layout(&document, 0);
    let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

    let overlay = AuiRendererBridge::build_overlay_frame(0, &draw_list);

    let order = overlay
        .draw_items
        .iter()
        .map(|item| (item.node_id.as_str(), item.sort_key.tree_order))
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        vec![("root", 0), ("panel", 1), ("icon", 2), ("title", 3)]
    );
}

#[test]
fn aui_composition_frame_splits_screen_space_canvas_stages() {
    let mut before = AuiCanvas::screen_overlay("before", 800.0, 600.0, "before-root");
    before.composition_stage = AuiCompositionStage::BeforeWorld;
    let overlay = AuiCanvas::screen_overlay("overlay", 800.0, 600.0, "overlay-root");
    let mut modal = AuiCanvas::screen_overlay("modal", 800.0, 600.0, "modal-root");
    modal.composition_stage = AuiCompositionStage::Modal;
    let document = AuiDocument::new(
        "stage-doc",
        vec![before, overlay, modal],
        vec![
            AuiNode::new("before-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            AuiNode::new("overlay-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            AuiNode::new("modal-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
        ],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

    let composition = AuiRendererBridge::build_composition_frame(1, &document, &layout, &draw_list);

    assert_eq!(composition.report.stage_count, 3);
    assert_eq!(composition.report.before_world_item_count, 1);
    assert_eq!(composition.report.screen_overlay_item_count, 1);
    assert_eq!(composition.report.modal_item_count, 1);
    assert_eq!(
        composition
            .canvas_references
            .iter()
            .map(|fact| (
                fact.canvas_id.as_str(),
                fact.reference_extent.width,
                fact.reference_extent.height,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("before", 800, 600),
            ("overlay", 800, 600),
            ("modal", 800, 600),
        ]
    );
    assert_eq!(
        composition
            .stage(AuiCompositionStage::BeforeWorld)
            .expect("before world stage")
            .draw_items[0]
            .composition_stage,
        AuiCompositionStage::BeforeWorld
    );
    assert!(composition.report.diagnostics.is_empty());
}

#[test]
fn aui_composition_frame_uses_canvas_layer_and_sorting_order() {
    let mut later = AuiCanvas::screen_overlay("later", 800.0, 600.0, "later-root");
    later.layer = 3;
    later.sorting_order = 20;
    let mut earlier = AuiCanvas::screen_overlay("earlier", 800.0, 600.0, "earlier-root");
    earlier.layer = -1;
    earlier.sorting_order = 2;
    let document = AuiDocument::new(
        "sort-doc",
        vec![later, earlier],
        vec![
            AuiNode::new("later-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            AuiNode::new("earlier-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
        ],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);

    let composition = AuiRendererBridge::build_composition_frame(1, &document, &layout, &draw_list);
    let items = &composition
        .stage(AuiCompositionStage::ScreenOverlay)
        .expect("screen overlay stage")
        .draw_items;

    assert_eq!(items[0].canvas_id, "earlier");
    assert_eq!(items[0].sort_key.canvas_layer, -1);
    assert_eq!(items[0].sort_key.canvas_sorting_order, 2);
    assert_eq!(items[1].canvas_id, "later");
    assert_eq!(items[1].sort_key.canvas_layer, 3);
    assert_eq!(items[1].sort_key.canvas_sorting_order, 20);
}

#[test]
fn aui_runtime_presenter_generates_glyph_plan_from_loaded_font_atlas() {
    let (document, _) = sample_document();
    let registry = font_registry_for_text("Start");
    let snapshot_output = ProjectUiStateSnapshotOutput::new(
        "test_snapshot",
        AuiSnapshotSource::TestSnapshot,
        ProjectUiStateSnapshot::new(9),
    );

    let output = AuiRuntimePresenter::present_project_snapshot_with_font_atlases(
        &document,
        snapshot_output,
        &registry,
    );

    assert_eq!(output.report.status, AuiRuntimePresentStatus::Success);
    assert!(output.report.glyph_present);
    assert!(output.report.font_atlas_present);
    assert_eq!(
        output.report.font_atlas_id.as_deref(),
        Some("ui-default-cmin")
    );
    assert_eq!(output.report.requested_glyph_count, 5);
    assert_eq!(output.report.rendered_glyph_count, 5);
    assert_eq!(output.report.unsupported_glyph_count, 0);
    assert!(output.report.text_pass_inserted);
    assert!(output.report.glyph_plan_hash.is_some());
    assert!(output.overlay.glyph_plan.is_some());
}

#[test]
fn b4_feedback_atlas_cow_invalidates_without_a_generation_change() {
    let (document, _) = sample_document();
    let mut atlases = font_registry_for_text("Start");
    let mut output = AuiRuntimePresenter::present_project_snapshot_with_font_atlases(
        &document,
        ProjectUiStateSnapshotOutput::new(
            "test",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(1),
        ),
        &atlases,
    );
    let mut state = crate::aui_control_feedback::AuiControlFeedbackState::default();
    let interaction = AuiInteractionResult::default();
    let bundles = RuntimeFontBundleRegistry::default();
    AuiRuntimePresenter::apply_control_feedback_with_fonts(
        &mut output,
        &interaction,
        &mut state,
        250_000,
        &atlases,
        &bundles,
    );
    let glyphs = output.overlay.glyph_plan.clone().unwrap();
    assert!(
        AuiRuntimePresenter::apply_control_feedback_with_fonts(
            &mut output,
            &interaction,
            &mut state,
            250_000,
            &atlases,
            &bundles,
        )
        .report
        .presentation_cache_hit
    );
    let atlas = Arc::make_mut(&mut atlases.atlases_by_id)
        .get_mut("ui-default-cmin")
        .unwrap();
    atlas
        .metadata
        .glyphs
        .iter_mut()
        .for_each(|glyph| glyph.bearing_x += 3.);
    assert_eq!(atlas.metadata.atlas_generation, 1);
    let changed = AuiRuntimePresenter::apply_control_feedback_with_fonts(
        &mut output,
        &interaction,
        &mut state,
        250_000,
        &atlases,
        &bundles,
    );
    assert!(changed.report.presentation_rebuilt);
    assert_ne!(glyphs, output.overlay.glyph_plan.clone().unwrap());
    assert!(
        AuiRuntimePresenter::apply_control_feedback_with_fonts(
            &mut output,
            &interaction,
            &mut state,
            250_000,
            &atlases,
            &bundles,
        )
        .report
        .presentation_cache_hit
    );
}

#[test]
fn e7_feedback_reuses_bounded_alternating_presentations() {
    let (document_a, _) = sample_document();
    let mut document_b = document_a.clone();
    document_b.nodes[0].style = Some(AuiStyle::color("#304050"));
    let atlases = RuntimeAuiFontAtlasRegistry::empty("e7-feedback");
    let bundles = RuntimeFontBundleRegistry::default();
    let themes = crate::aui_theme::AuiThemeRegistry::builtin();
    let snapshot = || {
        ProjectUiStateSnapshotOutput::new(
            "e7",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(1),
        )
    };
    let base_a = AuiRuntimePresenter::present_project_snapshot_with_font_atlases(
        &document_a,
        snapshot(),
        &atlases,
    );
    let base_b = AuiRuntimePresenter::present_project_snapshot_with_font_atlases(
        &document_b,
        snapshot(),
        &atlases,
    );
    let mut interaction = AuiInteractionResult::default();
    // Non-neutral styling exercises the bounded feedback cache; neutral styling
    // now reuses the base presentation directly.
    interaction.control_snapshot.hovered_node = Some("root".into());
    let mut state = crate::aui_control_feedback::AuiControlFeedbackState::default();

    let mut first_a = base_a.clone();
    assert!(
        AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
            &mut first_a,
            &interaction,
            &mut state,
            16_667,
            &atlases,
            &bundles,
            &themes,
            None,
        )
        .report
        .presentation_rebuilt
    );
    let mut first_b = base_b.clone();
    assert!(
        AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
            &mut first_b,
            &interaction,
            &mut state,
            16_667,
            &atlases,
            &bundles,
            &themes,
            None,
        )
        .report
        .presentation_rebuilt
    );
    let mut second_a = base_a;
    let reused = AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
        &mut second_a,
        &interaction,
        &mut state,
        16_667,
        &atlases,
        &bundles,
        &themes,
        None,
    );

    assert!(reused.report.presentation_cache_hit);
    assert!(!reused.report.presentation_rebuilt);
}

#[test]
fn e7_feedback_reuses_alternating_style_documents_when_presentation_identity_changes() {
    let (mut document_a, _) = sample_document();
    document_a.schema_version = crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.to_string();
    let mut document_b = document_a.clone();
    document_b.nodes[0].style = Some(AuiStyle::color("#506070"));
    let atlases = RuntimeAuiFontAtlasRegistry::empty("e7-style");
    let bundles = RuntimeFontBundleRegistry::default();
    let themes = crate::aui_theme::AuiThemeRegistry::builtin();
    let snapshot = || {
        ProjectUiStateSnapshotOutput::new(
            "e7-style",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(1),
        )
    };
    let base_a = AuiRuntimePresenter::present_project_snapshot_with_font_atlases(
        &document_a,
        snapshot(),
        &atlases,
    );
    let base_b = AuiRuntimePresenter::present_project_snapshot_with_font_atlases(
        &document_b,
        snapshot(),
        &atlases,
    );
    let detach_drawing_identity = |output: &mut AuiRuntimePresentOutput| {
        output.overlay.draw_items = Arc::new(output.overlay.draw_items.as_ref().clone());
        for stage in &mut output.composition.stages {
            stage.draw_items = Arc::new(stage.draw_items.as_ref().clone());
        }
    };
    let mut interaction = AuiInteractionResult::default();
    interaction.control_snapshot.hovered_node = Some("root".into());
    let mut state = crate::aui_control_feedback::AuiControlFeedbackState::default();

    for mut output in [base_a.clone(), base_b, base_a] {
        detach_drawing_identity(&mut output);
        AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
            &mut output,
            &interaction,
            &mut state,
            16_667,
            &atlases,
            &bundles,
            &themes,
            None,
        );
    }

    assert_eq!(state.style_rebuild_count(), 2);
}

#[test]
fn aui_runtime_resolver_updates_text_progress_visible_and_image() {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children([
        "score",
        "hp",
        "pause_overlay",
        "warning",
    ]);
    let score = AuiNode::new(
        "score",
        AuiNodeKind::Text,
        AuiRect::fixed_position(16.0, 16.0, 180.0, 32.0),
    )
    .with_parent("root")
    .with_text("Score: 0")
    .with_binding(AuiBindingRef::new(
        "bind.score",
        AuiBindingTarget::TextText,
        "game.score_text",
        Some(AuiBindingValue::String("Score: 0".to_string())),
    ));
    let hp = AuiNode::new(
        "hp",
        AuiNodeKind::ProgressBar,
        AuiRect::fixed_position(16.0, 56.0, 200.0, 20.0),
    )
    .with_parent("root")
    .with_progress_value(1.0)
    .with_binding(AuiBindingRef::new(
        "bind.hp",
        AuiBindingTarget::ProgressBarValue,
        "player.hp_ratio",
        Some(AuiBindingValue::Number(1.0)),
    ));
    let pause_overlay = AuiNode::new("pause_overlay", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_parent("root")
        .with_binding(AuiBindingRef::new(
            "bind.pause_visible",
            AuiBindingTarget::PanelVisible,
            "game.paused",
            Some(AuiBindingValue::Bool(false)),
        ));
    let warning = AuiNode::new(
        "warning",
        AuiNodeKind::Image,
        AuiRect::fixed_position(240.0, 16.0, 48.0, 48.0),
    )
    .with_parent("root")
    .with_image("warning_default")
    .with_binding(AuiBindingRef::new(
        "bind.warning_asset",
        AuiBindingTarget::ImageAssetRef,
        "warning.icon",
        None,
    ));
    let document = AuiDocument::new(
        "hud",
        vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
        vec![root, score, hp, pause_overlay, warning],
    );
    let snapshot = ProjectUiStateSnapshot::new(42)
        .with_value(
            "game.score_text",
            AuiBindingValue::String("Score: 1200".to_string()),
        )
        .with_value("player.hp_ratio", AuiBindingValue::Number(0.35))
        .with_value("game.paused", AuiBindingValue::Bool(true))
        .with_value(
            "warning.icon",
            AuiBindingValue::AssetRef(AuiAssetRef::new("warning_low_hp")),
        );

    let (resolved, report) = AuiRuntimeResolver::resolve_bindings(&document, &snapshot);

    assert!(report.ok());
    assert_eq!(report.binding_count, 4);
    assert_eq!(report.resolved_count, 4);
    let score = resolved
        .nodes
        .iter()
        .find(|node| node.node_id == "score")
        .unwrap();
    let hp = resolved
        .nodes
        .iter()
        .find(|node| node.node_id == "hp")
        .unwrap();
    let overlay = resolved
        .nodes
        .iter()
        .find(|node| node.node_id == "pause_overlay")
        .unwrap();
    let warning = resolved
        .nodes
        .iter()
        .find(|node| node.node_id == "warning")
        .unwrap();
    assert_eq!(score.text.as_deref(), Some("Score: 1200"));
    assert_eq!(hp.progress_value, Some(0.35));
    assert!(overlay.visible);
    assert_eq!(
        warning.image.as_ref().map(|image| image.asset_id.as_str()),
        Some("warning_low_hp")
    );
}

#[test]
fn feedback_bindings_move_layout_and_color_without_accepting_invalid_values() {
    let mut node = AuiNode::new(
        "score",
        AuiNodeKind::Text,
        AuiRect::fixed_position(0.0, 0.0, 100.0, 32.0),
    );
    for (target, value) in [
        (
            AuiBindingTarget::RectOffsetX,
            AuiBindingValue::Number(300.0),
        ),
        (
            AuiBindingTarget::RectOffsetY,
            AuiBindingValue::Number(240.0),
        ),
        (
            AuiBindingTarget::TextColor,
            AuiBindingValue::String("#9fefff80".into()),
        ),
        (AuiBindingTarget::Interactable, AuiBindingValue::Bool(false)),
    ] {
        apply_binding_value(
            &mut node,
            &AuiBindingRef::new("test", target, "test", None),
            value,
        )
        .unwrap();
    }
    assert_eq!(node.rect.offset_min, AuiVec2::new(300.0, 240.0));
    assert_eq!(
        node.style.as_ref().unwrap().text_color.as_deref(),
        Some("#9fefff80")
    );
    assert!(!node.interactable);
    assert!(apply_binding_value(
        &mut node,
        &AuiBindingRef::new("test", AuiBindingTarget::RectOffsetX, "test", None),
        AuiBindingValue::Number(f32::NAN)
    )
    .is_err());
    assert!(apply_binding_value(
        &mut node,
        &AuiBindingRef::new("test", AuiBindingTarget::TextColor, "test", None),
        AuiBindingValue::String("bad".into())
    )
    .is_err());
    assert_eq!(node.rect.offset_min.x, 300.0);
}

#[test]
fn aui_color_contract_encodes_linear_extension_rgb_without_encoding_alpha() {
    assert_eq!(extension_color([0.2158605, 0.5, 0., 0.5]), "#80bc0080");
}

#[test]
fn aui_color_contract_progress_fill_consumes_foreground_and_keeps_track_separate() {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["hp"]);
    let mut hp = AuiNode::new(
        "hp",
        AuiNodeKind::ProgressBar,
        AuiRect::fixed_position(10., 10., 200., 20.),
    )
    .with_parent("root")
    .with_progress_value(0.25)
    .with_style(AuiStyle::color("#303030"));
    hp.style.as_mut().unwrap().text_color = Some("#a34837".into());
    let mut document = AuiDocument::new(
        "hud",
        vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
        vec![root, hp],
    );
    for expected in ["#a34837", "#427e5c", "#41d17d"] {
        document.nodes[1].resolved_style = (expected == "#427e5c").then(|| {
            let mut style = crate::aui_theme::AuiResolvedStyle::default();
            style.paint.foreground = Some(expected.into());
            style
        });
        if expected == "#41d17d" {
            document.nodes[1].style.as_mut().unwrap().text_color = None;
        }
        let layout = AuiLayoutEngine::layout(&document, 1);
        let (draw, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
        match draw.commands[1].as_ref() {
            AuiDrawCommand::DrawRect { color, .. } => {
                assert_eq!(color.as_deref(), Some("#303030"))
            }
            other => panic!("expected progress track, got {other:?}"),
        }
        match draw.commands[2].as_ref() {
            AuiDrawCommand::DrawRect { color, rect, .. } => {
                assert_eq!(color.as_deref(), Some(expected));
                assert_eq!(rect.width, 50.);
            }
            other => panic!("expected progress fill, got {other:?}"),
        }
    }
}

#[test]
fn aui_progress_bar_uses_composite_draw_commands() {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["hp"]);
    let hp = AuiNode::new(
        "hp",
        AuiNodeKind::ProgressBar,
        AuiRect::fixed_position(10.0, 10.0, 200.0, 20.0),
    )
    .with_parent("root")
    .with_progress_value(0.25);
    let document = AuiDocument::new(
        "hud",
        vec![AuiCanvas::screen_overlay("main", 800.0, 600.0, "root")],
        vec![root, hp],
    );

    let layout = AuiLayoutEngine::layout(&document, 1);
    let (draw_list, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);

    assert_eq!(report.draw_command_count, 3);
    assert!(matches!(
        draw_list.commands[1].as_ref(),
        AuiDrawCommand::DrawRect { ref node_id, .. } if node_id == "hp:track"
    ));
    match draw_list.commands[2].as_ref() {
        AuiDrawCommand::DrawRect { node_id, rect, .. } => {
            assert_eq!(node_id, "hp:fill");
            assert_eq!(rect.width, 50.0);
        }
        other => panic!("expected progress fill rect, got {:?}", other),
    }
}

#[test]
fn aui_hit_test_picks_topmost_interactable_node() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);

    let hit = AuiInteractionSystem::hit_test(&document, &layout, 150.0, 130.0);

    assert_eq!(hit.hit_node.as_deref(), Some("button_b"));
    assert!(hit.consumed);
    assert_eq!(hit.reason, AuiHitTestReason::HitInteractable);
}

#[test]
fn aui_hit_test_ignores_invisible_or_non_interactable_nodes() {
    let mut document = interaction_document();
    document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "button_b")
        .expect("button_b")
        .visible = false;
    document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "button_a")
        .expect("button_a")
        .interactable = false;
    let layout = AuiLayoutEngine::layout(&document, 1);

    let hit = AuiInteractionSystem::hit_test(&document, &layout, 150.0, 130.0);

    assert_eq!(hit.hit_node.as_deref(), Some("button_a"));
    assert!(!hit.consumed);
    assert_eq!(hit.reason, AuiHitTestReason::HitNonInteractable);
}

#[test]
fn aui_control_interaction_mouse_capture_tracks_inside_and_exactly_one_click() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();

    let hover = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Move, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(
        hover.control_snapshot.hovered_node.as_deref(),
        Some("button_b")
    );
    assert_eq!(hover.control_snapshot.pressed_node, None);

    let down = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(
        down.control_snapshot.pressed_node.as_deref(),
        Some("button_b")
    );
    assert!(down.control_snapshot.pressed_inside);
    assert_eq!(
        down.control_snapshot.pointer_device_kind,
        Some(RuntimePointerDeviceKind::Mouse)
    );

    let outside = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Move, 700.0, 500.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(outside.control_snapshot.hovered_node, None);
    assert_eq!(
        outside.control_snapshot.pressed_node.as_deref(),
        Some("button_b")
    );
    assert!(!outside.control_snapshot.pressed_inside);
    assert_eq!(outside.consumed_event_indices, vec![0]);

    let inside = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Move, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert!(inside.control_snapshot.pressed_inside);

    let up = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Up, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(up.control_snapshot.pressed_node, None);
    assert_eq!(
        up.commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::Click)
            .count(),
        1
    );
    assert_eq!(
        up.actions
            .iter()
            .filter(|action| action.action_id == "ui.pause")
            .count(),
        1
    );
}

#[test]
fn aui_control_interaction_pointer_leave_preserves_capture_and_reentry_can_click() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();
    AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );

    let left = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(
            RuntimePointerPhase::Leave,
            150.0,
            130.0,
        )]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(left.control_snapshot.hovered_node, None);
    assert_eq!(
        left.control_snapshot.pressed_node.as_deref(),
        Some("button_b")
    );
    assert!(!left.control_snapshot.pressed_inside);
    assert_eq!(left.consumed_event_indices, vec![0]);
    assert!(!left.commands.iter().any(|command| matches!(
        command.command_kind,
        AuiCommandKind::PointerCancel | AuiCommandKind::Click
    )));

    let reentered = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Move, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert!(reentered.control_snapshot.pressed_inside);
    let released = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Up, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(
        released
            .commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::Click)
            .count(),
        1
    );
}

#[test]
fn aui_control_interaction_pointer_cancel_is_single_shot_and_never_clicks_or_drops() {
    let document = drag_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();
    AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![
            unified_mouse(RuntimePointerPhase::Down, 60.0, 60.0),
            unified_mouse(RuntimePointerPhase::Move, 140.0, 60.0),
        ]),
        &mut state,
        AuiInteractionConfig::default(),
    );

    let cancel = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(
            RuntimePointerPhase::Cancel,
            140.0,
            60.0,
        )]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(cancel.pointer_cancel_count, 1);
    assert_eq!(
        cancel
            .commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::PointerCancel)
            .count(),
        1
    );
    assert_eq!(
        cancel
            .commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::DragCancel)
            .count(),
        1
    );
    assert!(cancel.commands.iter().all(|command| !matches!(
        command.command_kind,
        AuiCommandKind::Click | AuiCommandKind::Drop
    )));

    let repeated = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(
            RuntimePointerPhase::Cancel,
            140.0,
            60.0,
        )]),
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(repeated.pointer_cancel_count, 0);
    assert!(repeated.commands.is_empty());
}

#[test]
fn aui_control_interaction_pointer_up_outside_clears_capture_without_click() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();
    AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );

    let up = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Up, 700.0, 500.0)]),
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(up.control_snapshot.pressed_node, None);
    assert_eq!(up.consumed_event_indices, vec![0]);
    assert!(!up
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::Click));
}

#[test]
fn aui_control_interaction_touch_click_and_cancel_leave_no_hover_residue() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut click_state = AuiInteractionState::default();
    let down = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_touch(
            RuntimePointerPhase::Down,
            42,
            150.0,
            130.0,
        )]),
        &mut click_state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(down.control_snapshot.hovered_node, None);
    assert_eq!(down.control_snapshot.pointer_id, Some(42));
    assert_eq!(
        down.control_snapshot.pointer_device_kind,
        Some(RuntimePointerDeviceKind::Touch)
    );
    let up = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_touch(
            RuntimePointerPhase::Up,
            42,
            150.0,
            130.0,
        )]),
        &mut click_state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(up.control_snapshot.hovered_node, None);
    assert_eq!(up.control_snapshot.pressed_node, None);
    assert_eq!(
        up.commands
            .iter()
            .filter(|command| command.command_kind == AuiCommandKind::Click)
            .count(),
        1
    );

    let mut cancel_state = AuiInteractionState::default();
    AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_touch(
            RuntimePointerPhase::Down,
            43,
            150.0,
            130.0,
        )]),
        &mut cancel_state,
        AuiInteractionConfig::default(),
    );
    let cancel = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_touch(
            RuntimePointerPhase::Cancel,
            43,
            150.0,
            130.0,
        )]),
        &mut cancel_state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(cancel.control_snapshot.hovered_node, None);
    assert_eq!(cancel.control_snapshot.pressed_node, None);
    assert_eq!(cancel.consumed_event_indices, vec![0]);
    assert!(cancel
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::PointerCancel));
    assert!(!cancel
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::Click));
}

#[test]
fn aui_control_interaction_reconciles_disabled_removed_modal_and_session_change() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();
    AuiInteractionSystem::process_session_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
        "session-a",
    );

    let session_changed = AuiInteractionSystem::process_session_with_state(
        &document,
        &layout,
        &pointer_frame(Vec::new()),
        &mut state,
        AuiInteractionConfig::default(),
        "session-b",
    );
    assert_eq!(session_changed.control_snapshot.pressed_node, None);
    assert!(session_changed.control_reconciliation_count >= 1);

    AuiInteractionSystem::process_session_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
        "session-b",
    );
    let mut disabled = document.clone();
    disabled
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "button_b")
        .unwrap()
        .interactable = false;
    let disabled_layout = AuiLayoutEngine::layout(&disabled, 2);
    let disabled_result = AuiInteractionSystem::process_session_with_state(
        &disabled,
        &disabled_layout,
        &pointer_frame(Vec::new()),
        &mut state,
        AuiInteractionConfig::default(),
        "session-b",
    );
    assert_eq!(disabled_result.control_snapshot.pressed_node, None);
    assert!(disabled_result.control_reconciliation_count >= 1);

    AuiInteractionSystem::process_session_with_state(
        &document,
        &layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 150.0, 130.0)]),
        &mut state,
        AuiInteractionConfig::default(),
        "session-b",
    );
    let mut removed = document.clone();
    removed.nodes.retain(|node| node.node_id != "button_b");
    removed
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "root")
        .unwrap()
        .children
        .retain(|node_id| node_id != "button_b");
    let removed_layout = AuiLayoutEngine::layout(&removed, 3);
    let removed_result = AuiInteractionSystem::process_session_with_state(
        &removed,
        &removed_layout,
        &pointer_frame(Vec::new()),
        &mut state,
        AuiInteractionConfig::default(),
        "session-b",
    );
    assert_eq!(removed_result.control_snapshot.pressed_node, None);
    assert!(removed_result.control_reconciliation_count >= 1);

    let modal = modal_document();
    let mut modal_state = AuiInteractionState::default();
    modal_state
        .canvas_visibility_overrides
        .insert("modal".to_string(), false);
    let background_layout = AuiLayoutEngine::layout_with_interaction_state(&modal, 1, &modal_state);
    AuiInteractionSystem::process_with_state(
        &modal,
        &background_layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 40.0, 40.0)]),
        &mut modal_state,
        AuiInteractionConfig::default(),
    );
    modal_state
        .canvas_visibility_overrides
        .insert("modal".to_string(), true);
    let modal_layout = AuiLayoutEngine::layout_with_interaction_state(&modal, 2, &modal_state);
    let modal_result = AuiInteractionSystem::process_with_state(
        &modal,
        &modal_layout,
        &pointer_frame(Vec::new()),
        &mut modal_state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(modal_result.control_snapshot.pressed_node, None);
    assert!(modal_result.control_reconciliation_count >= 1);

    let mut screen_document = navigation_text_entry_document();
    screen_document
        .canvases
        .iter_mut()
        .find(|canvas| canvas.canvas_id == "pause")
        .unwrap()
        .composition_stage = AuiCompositionStage::ScreenOverlay;
    let mut screen_state = AuiInteractionState::default();
    let main_layout =
        AuiLayoutEngine::layout_with_interaction_state(&screen_document, 1, &screen_state);
    AuiInteractionSystem::process_with_state(
        &screen_document,
        &main_layout,
        &pointer_frame(vec![unified_mouse(RuntimePointerPhase::Down, 40.0, 40.0)]),
        &mut screen_state,
        AuiInteractionConfig::default(),
    );
    AuiInteractionSystem::push_screen(&screen_document, &mut screen_state, "pause_screen")
        .expect("pause screen");
    let pause_layout =
        AuiLayoutEngine::layout_with_interaction_state(&screen_document, 2, &screen_state);
    let screen_result = AuiInteractionSystem::process_with_state(
        &screen_document,
        &pause_layout,
        &pointer_frame(Vec::new()),
        &mut screen_state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(screen_result.control_snapshot.pressed_node, None);
    assert_eq!(
        screen_result.control_snapshot.active_screen_id.as_deref(),
        Some("pause_screen")
    );
    assert!(screen_result.control_reconciliation_count >= 1);
}

#[test]
fn aui_control_interaction_keyboard_and_gamepad_submit_remain_exactly_once() {
    let document = navigation_text_entry_document();
    let layout = AuiLayoutEngine::layout(&document, 1);

    for event in [
        RuntimeInputEvent::KeyDown {
            key: "Enter".to_string(),
        },
        RuntimeInputEvent::GamepadButtonDown {
            gamepad_id: 0,
            button: "South".to_string(),
        },
    ] {
        let mut state = AuiInteractionState::default();
        state.focus.focused_node = Some("play_button".to_string());
        state.focus.focus_reason = AuiFocusReason::Keyboard;
        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &pointer_frame(vec![event]),
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert_eq!(
            result
                .commands
                .iter()
                .filter(|command| command.command_kind == AuiCommandKind::Submit)
                .count(),
            1
        );
        assert_eq!(
            result
                .actions
                .iter()
                .filter(|action| action.action_id == "ui.play")
                .count(),
            1
        );
        assert_eq!(
            result.control_snapshot.focused_node.as_deref(),
            Some("play_button")
        );
        assert!(result.control_snapshot.focus_visible);
    }
}

#[test]
fn aui_pointer_move_keeps_default_focus_logical_without_keyboard_focus_visual() {
    let document = navigation_text_entry_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![RuntimeInputEvent::PointerMove { x: 40.0, y: 40.0 }]);
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(result.default_focus_applied_count, 1);
    assert_eq!(state.focus.focused_node.as_deref(), Some("play_button"));
    assert_eq!(state.focus.focus_reason, AuiFocusReason::Pointer);
    assert!(!result.control_snapshot.focus_visible);
    assert_eq!(
        result.control_snapshot.hovered_node.as_deref(),
        Some("play_button")
    );
}

#[test]
fn aui_interaction_consumes_pointer_down_on_button() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![RuntimeInputEvent::PointerDown {
        x: 150.0,
        y: 130.0,
        button: RuntimePointerButton::Primary,
    }]);

    let result = AuiInteractionSystem::process(&document, &layout, &frame);

    assert!(result.consumed);
    assert!(result.commands.iter().any(|command| {
        command.source_node == "button_b" && command.command_kind == AuiCommandKind::PointerDown
    }));
    assert!(result.commands.iter().any(|command| {
        command.source_node == "button_b" && command.command_kind == AuiCommandKind::Focus
    }));
}

#[test]
fn aui_interaction_does_not_consume_pointer_outside_ui() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![RuntimeInputEvent::PointerDown {
        x: 700.0,
        y: 500.0,
        button: RuntimePointerButton::Primary,
    }]);

    let result = AuiInteractionSystem::process(&document, &layout, &frame);

    assert!(!result.consumed);
    assert!(result.commands.is_empty());
    assert_eq!(
        result.traces[0].reason,
        AuiHitTestReason::HitNonInteractable
    );
}

#[test]
fn aui_interaction_pointer_up_generates_click_when_pressed_same_node() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 150.0,
            y: 130.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerUp {
            x: 150.0,
            y: 130.0,
            button: RuntimePointerButton::Primary,
        },
    ]);

    let result = AuiInteractionSystem::process(&document, &layout, &frame);

    assert!(result.consumed);
    assert!(result
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::Click
            && command.source_node == "button_b"));
    assert_eq!(result.actions.len(), 1);
    assert_eq!(result.actions[0].action_id, "ui.pause");
    assert_eq!(result.actions[0].node_id, "button_b");
}

#[test]
fn aui_interaction_maps_target_space_per_canvas_and_filters_original_indices() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "session-portrait".to_string(),
        target_id: "game-view".to_string(),
        target_extent: GameViewExtent::new(720, 1280),
        display_rect: GameViewRect::new(0.0, 0.0, 720.0, 1280.0),
        scale_policy: GameViewScalePolicy::Contain,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 800, 600)],
    })
    .expect("portrait presentation");
    let target_point = presentation
        .reference_to_target("main", GameViewPoint::new(150.0, 130.0))
        .expect("button point maps to target");
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerMove { x: 10.0, y: 10.0 },
        RuntimeInputEvent::PointerDown {
            x: target_point.x,
            y: target_point.y,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerUp {
            x: target_point.x,
            y: target_point.y,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_target_space_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
        &presentation,
    );
    let gameplay_frame = frame.filter_consumed_events(&result.consumed_event_indices);

    assert_eq!(result.consumed_event_indices, vec![1, 2]);
    assert_eq!(result.actions.len(), 1);
    assert_eq!(result.actions[0].action_id, "ui.pause");
    assert_eq!(gameplay_frame.events, vec![frame.events[0].clone()]);
    assert_eq!(
        frame.events[1],
        RuntimeInputEvent::PointerDown {
            x: target_point.x,
            y: target_point.y,
            button: RuntimePointerButton::Primary,
        }
    );
}

#[test]
fn aui_interaction_trace_records_consumed_reason() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![RuntimeInputEvent::PointerMove { x: 150.0, y: 130.0 }]);

    let result = AuiInteractionSystem::process(&document, &layout, &frame);

    assert_eq!(result.traces.len(), 1);
    assert_eq!(result.traces[0].frame, 7);
    assert_eq!(
        result.traces[0].event_kind,
        AuiInteractionEventKind::PointerMove
    );
    assert_eq!(result.traces[0].hit_node.as_deref(), Some("button_b"));
    assert!(result.traces[0].consumed);
    assert_eq!(result.traces[0].reason, AuiHitTestReason::HitInteractable);
    assert_eq!(result.traces[0].command_count, 2);
}

#[test]
fn aui_drag_generates_drop_payload_and_actions() {
    let document = drag_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 60.0,
            y: 60.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove { x: 120.0, y: 60.0 },
        RuntimeInputEvent::PointerUp {
            x: 220.0,
            y: 80.0,
            button: RuntimePointerButton::Primary,
        },
    ]);

    let result = AuiInteractionSystem::process(&document, &layout, &frame);

    assert!(result.consumed);
    assert_eq!(result.consumed_event_indices, vec![0, 1, 2]);
    assert!(result
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::DragStart));
    let drop_command = result
        .commands
        .iter()
        .find(|command| command.command_kind == AuiCommandKind::Drop)
        .expect("drop command");
    let payload: AuiDragDropPayload =
        serde_json::from_str(drop_command.payload.as_deref().unwrap()).unwrap();
    assert_eq!(payload.schema_version, "aui-drag-drop-payload.v1");
    assert_eq!(payload.source_node, "drag_source");
    assert_eq!(payload.target_node.as_deref(), Some("drop_target"));
    assert_eq!(payload.drag_phase, "drop");
    assert!(result
        .actions
        .iter()
        .any(|action| action.event == AuiActionEvent::Drop
            && action.action_id == "ui.drop"
            && action.payload.is_some()));
}

#[test]
fn aui_drag_cancel_to_empty_area_does_not_generate_click() {
    let document = drag_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 60.0,
            y: 60.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove { x: 140.0, y: 60.0 },
        RuntimeInputEvent::PointerUp {
            x: 500.0,
            y: 400.0,
            button: RuntimePointerButton::Primary,
        },
    ]);

    let result = AuiInteractionSystem::process(&document, &layout, &frame);

    assert!(result
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::DragCancel));
    assert!(!result
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::Click));
    assert!(result
        .actions
        .iter()
        .all(|action| action.event != AuiActionEvent::Click));
}

#[test]
fn aui_drag_requires_primary_pointer() {
    let document = drag_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 60.0,
            y: 60.0,
            button: RuntimePointerButton::Secondary,
        },
        RuntimeInputEvent::PointerMove { x: 140.0, y: 60.0 },
        RuntimeInputEvent::PointerUp {
            x: 220.0,
            y: 80.0,
            button: RuntimePointerButton::Secondary,
        },
    ]);

    let result = AuiInteractionSystem::process(&document, &layout, &frame);

    assert!(!result.commands.iter().any(|command| matches!(
        command.command_kind,
        AuiCommandKind::DragStart | AuiCommandKind::DragMove | AuiCommandKind::Drop
    )));
}

#[test]
fn aui_modal_blocks_pointer_wheel_and_key_outside_modal_root() {
    let document = modal_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 40.0,
            y: 40.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::MouseWheel { delta: -1.0 },
        RuntimeInputEvent::KeyDown {
            key: "Space".to_string(),
        },
    ]);
    frame.pointer_position = Some(crate::input_action::PointerPosition { x: 40.0, y: 40.0 });
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(state.active_modal_root.as_deref(), Some("modal_root"));
    assert_eq!(state.focus.focus_scope_root.as_deref(), Some("modal_root"));
    assert_eq!(result.consumed_event_indices, vec![0, 1, 2]);
    assert_eq!(
        result.consumed_event_count_by_kind.get("PointerDown"),
        Some(&1)
    );
    assert_eq!(
        result.consumed_event_count_by_kind.get("MouseWheel"),
        Some(&1)
    );
    assert_eq!(result.consumed_event_count_by_kind.get("KeyDown"), Some(&1));
    assert!(result.commands.is_empty());
}

#[test]
fn aui_focus_trap_cycles_tab_and_escape_generates_cancel() {
    let document = modal_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 230.0,
            y: 150.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::KeyDown {
            key: "Tab".to_string(),
        },
        RuntimeInputEvent::KeyDown {
            key: "Escape".to_string(),
        },
    ]);
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(state.focus.focus_scope_root.as_deref(), Some("modal_root"));
    assert_eq!(state.focus.focused_node.as_deref(), Some("modal_button_b"));
    assert!(result
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::Focus
            && command.source_node == "modal_button_b"));
    assert!(result
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::Cancel
            && command.source_node == "modal_button_b"));
    assert!(result
        .actions
        .iter()
        .any(|action| action.event == AuiActionEvent::Cancel && action.action_id == "ui.cancel"));
}

#[test]
fn aui_scroll_wheel_updates_offset_and_layout_rects() {
    let document = scroll_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let before_item_0_y = layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "item_0")
        .expect("item_0")
        .rect
        .y;
    let mut frame = pointer_frame(vec![RuntimeInputEvent::MouseWheel { delta: -1.0 }]);
    frame.pointer_position = Some(crate::input_action::PointerPosition { x: 20.0, y: 20.0 });
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    let layout_after =
        AuiLayoutEngine::layout_with_scroll_offsets(&document, 2, &state.scroll_offsets);
    let after_item_0_y = layout_after
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "item_0")
        .expect("item_0")
        .rect
        .y;

    assert_eq!(
        result.consumed_event_count_by_kind.get("MouseWheel"),
        Some(&1)
    );
    assert!(result.scroll_offset_change_count > 0);
    assert!(layout_after.report.scroll_offset_applied);
    assert!(layout_after.report.scroll_applied_node_count >= 3);
    assert!(layout_after.report.clipped_node_count > 0);
    assert!(after_item_0_y < before_item_0_y);
}

#[test]
fn aui_scroll_drag_updates_offset() {
    let document = scroll_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 20.0,
            y: 80.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove { x: 20.0, y: 20.0 },
        RuntimeInputEvent::PointerUp {
            x: 20.0,
            y: 20.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(result.consumed_event_indices, vec![0, 1, 2]);
    assert!(result.scroll_offset_change_count > 0);
    assert!(state
        .scroll_offsets
        .get("list")
        .is_some_and(|scroll| scroll.offset_y > 0.0));
    assert!(state.active_scroll_capture().is_none());
}

#[test]
fn aui_scroll_child_button_click_completes_before_drag_threshold() {
    let mut document = scroll_document();
    let item = document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "item_0")
        .expect("item_0");
    item.kind = AuiNodeKind::Button;
    item.interactable = true;
    item.action_refs = vec![AuiActionRef::activate("ui.select")];
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
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
    ]);
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert!(result.commands.iter().any(|command| {
        command.command_kind == AuiCommandKind::Click && command.source_node == "item_0"
    }));
    assert!(result
        .actions
        .iter()
        .any(|action| { action.action_id == "ui.select" && action.node_id == "item_0" }));
    assert_eq!(result.scroll_offset_change_count, 0);
    assert!(state.active_scroll_capture().is_none());
    assert!(state.primary_press.is_none());
}

#[test]
fn aui_scroll_child_button_drag_cancels_click_after_threshold() {
    let mut document = scroll_document();
    let item = document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "item_0")
        .expect("item_0");
    item.kind = AuiNodeKind::Button;
    item.interactable = true;
    item.action_refs = vec![AuiActionRef::activate("ui.select")];
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 20.0,
            y: 70.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove { x: 20.0, y: 20.0 },
        RuntimeInputEvent::PointerUp {
            x: 20.0,
            y: 20.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert!(result.scroll_offset_change_count > 0);
    assert!(result.commands.iter().any(|command| {
        command.command_kind == AuiCommandKind::PointerCancel && command.source_node == "item_0"
    }));
    assert!(!result
        .commands
        .iter()
        .any(|command| command.command_kind == AuiCommandKind::Click));
    assert!(!result
        .actions
        .iter()
        .any(|action| action.action_id == "ui.select"));
    assert!(state.active_scroll_capture().is_none());
    assert!(state.primary_press.is_none());
}

#[test]
fn aui_clip_layout_extracts_effective_clip_and_culls_offscreen_items() {
    let document = scroll_document();

    let layout = AuiLayoutEngine::layout(&document, 1);
    let (draw_list, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let overlay = AuiRendererBridge::build_overlay_frame(1, &draw_list);

    assert_eq!(layout.report.clip_root_count, 1);
    assert!(layout.report.effective_clip_node_count > 0);
    assert!(layout.report.clipped_node_count > 0);
    assert!(report.effective_clip_item_count > 0);
    assert!(report.culled_draw_item_count > 0);
    assert_eq!(report.scrollbar_visible_count, 1);
    assert_eq!(overlay.report.scrollbar_visible_count, 1);
    assert!(overlay
        .draw_items
        .iter()
        .any(|item| item.item_kind == AuiOverlayItemKind::ScrollbarTrack));
    assert!(overlay
        .draw_items
        .iter()
        .any(|item| item.item_kind == AuiOverlayItemKind::ScrollbarThumb));
    assert!(layout.scrollbar_metrics.iter().any(|metrics| {
        metrics.scroll_node_id == "list" && metrics.visible && metrics.max_offset_y > 0.0
    }));
    assert!(!draw_list.commands.iter().any(|command| {
        matches!(command.as_ref(), AuiDrawCommand::DrawRect { node_id, .. } if node_id == "item_2")
    }));
}

#[test]
fn aui_hit_test_rejects_clipped_scroll_content() {
    let document = scroll_document();
    let layout = AuiLayoutEngine::layout(&document, 1);

    let hit = AuiInteractionSystem::hit_test(&document, &layout, 20.0, 200.0);

    assert_ne!(hit.hit_node.as_deref(), Some("item_2"));
    assert!(hit.clip_rejected_count > 0);
}

#[test]
fn aui_scrollbar_thumb_drag_updates_offset() {
    let document = scroll_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let metrics = layout
        .scrollbar_metrics
        .iter()
        .find(|metrics| metrics.scroll_node_id == "list" && metrics.visible)
        .expect("visible list scrollbar");
    let thumb_x = metrics.thumb_rect.x + metrics.thumb_rect.width * 0.5;
    let thumb_y = metrics.thumb_rect.y + metrics.thumb_rect.height * 0.5;
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: thumb_x,
            y: thumb_y,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove {
            x: thumb_x,
            y: thumb_y + 20.0,
        },
        RuntimeInputEvent::PointerUp {
            x: thumb_x,
            y: thumb_y + 20.0,
            button: RuntimePointerButton::Primary,
        },
    ]);
    let mut state = AuiInteractionState::default();

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert!(result.scroll_offset_change_count > 0);
    assert!(result.traces.iter().any(|trace| {
        trace
            .captured_node
            .as_deref()
            .is_some_and(|node| node.ends_with(":scrollbar-thumb"))
    }));
    assert!(state
        .scroll_offsets
        .get("list")
        .is_some_and(|scroll| scroll.offset_y > 0.0));
    assert!(state.active_scroll_capture().is_none());
}

#[test]
fn aui_navigation_arrow_down_moves_focus_and_scrolls_visible() {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["list"]);
    let list = AuiNode::new(
        "list",
        AuiNodeKind::ScrollView,
        AuiRect::fixed_position(10.0, 10.0, 160.0, 100.0),
    )
    .with_parent("root")
    .with_children(["item_0", "item_1", "item_2"]);
    let item_0 = AuiNode::new(
        "item_0",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 140.0, 40.0),
    )
    .with_parent("list")
    .with_interactable(true);
    let item_1 = AuiNode::new(
        "item_1",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 50.0, 140.0, 40.0),
    )
    .with_parent("list")
    .with_interactable(true);
    let item_2 = AuiNode::new(
        "item_2",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 150.0, 140.0, 40.0),
    )
    .with_parent("list")
    .with_interactable(true);
    let document = AuiDocument::new(
        "navigation-scroll-doc",
        vec![AuiCanvas::screen_overlay("main", 320.0, 240.0, "root")],
        vec![root, list, item_0, item_1, item_2],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
        key: "ArrowDown".to_string(),
    }]);
    let mut state = AuiInteractionState::default();
    state.focus.focused_node = Some("item_1".to_string());
    state.focus.focus_reason = AuiFocusReason::Keyboard;

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(state.focus.focused_node.as_deref(), Some("item_2"));
    assert_eq!(result.keyboard_navigation_event_count, 1);
    assert_eq!(result.focus_visible_scroll_count, 1);
    assert_eq!(result.scroll_offset_change_count, 1);
    assert!(state
        .scroll_offsets
        .get("list")
        .is_some_and(|scroll| scroll.offset_y > 0.0));
    assert!(result.commands.iter().any(|command| {
        command.command_kind == AuiCommandKind::Focus && command.source_node == "item_2"
    }));
    assert!(result.commands.iter().any(|command| {
        command.command_kind == AuiCommandKind::Scroll && command.source_node == "list"
    }));
}

#[test]
fn aui_rectclip_scrollbar_navigation_report_collects_gate_evidence() {
    let document = scroll_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (_, render_report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let metrics = layout
        .scrollbar_metrics
        .iter()
        .find(|metrics| metrics.scroll_node_id == "list" && metrics.visible)
        .expect("visible list scrollbar");
    let thumb_x = metrics.thumb_rect.x + metrics.thumb_rect.width * 0.5;
    let thumb_y = metrics.thumb_rect.y + metrics.thumb_rect.height * 0.5;
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: thumb_x,
            y: thumb_y,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::PointerMove {
            x: thumb_x,
            y: thumb_y + 20.0,
        },
    ]);
    let mut state = AuiInteractionState::default();
    let mut interaction = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    let clipped_hit = AuiInteractionSystem::hit_test(&document, &layout, 20.0, 200.0);
    interaction.hit_test_clip_rejected_count += clipped_hit.clip_rejected_count;
    interaction.keyboard_navigation_event_count = 1;
    interaction.focus_change_count = 1;
    interaction.focus_visible_scroll_count = 1;

    let report = AuiRectClipScrollbarNavigationProductizationReport::from_parts(
        &layout.report,
        &render_report,
        &interaction,
        Some("item_1".to_string()),
        Some("item_2".to_string()),
    );

    assert_eq!(
        report.schema_version,
        AUI_RECTCLIP_SCROLLBAR_NAVIGATION_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.status, "passed");
    assert!(report.clip_root_count > 0);
    assert!(report.effective_clip_item_count > 0);
    assert!(report.culled_draw_item_count > 0);
    assert!(report.hit_test_clip_rejected_count > 0);
    assert!(report.scrollbar_visible_count > 0);
    assert!(report.scrollbar_thumb_drag_count > 0);
    assert!(report.scrollbar_offset_change_count > 0);
    assert_eq!(report.keyboard_navigation_event_count, 1);
    assert_eq!(report.focus_visible_scroll_count, 1);
    assert!(report.stencil_mask_deferred);
    assert!(!report.nested_scroll_deferred);
    assert!(!report.virtualized_list_deferred);
    assert!(report.full_gamepad_navigation_deferred);
}

#[test]
fn aui_navigation_screenflow_textentry_schema_serializes() {
    let document = navigation_text_entry_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
        key: "Space".to_string(),
    }]);
    let mut state = AuiInteractionState::default();
    state.focus.focused_node = Some("play_button".to_string());

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    let filtered = frame.filter_consumed_events(&result.consumed_event_indices);
    let report = AuiRuntimeNavigationScreenFlowTextEntryProductizationReport::from_result(
        &document,
        frame.events.len(),
        filtered.events.len(),
        &result,
    );
    let json = serde_json::to_string(&report).expect("216 report should serialize");

    assert_eq!(
        report.schema_version,
        AUI_RUNTIME_NAVIGATION_SCREENFLOW_TEXTENTRY_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.status, "passed");
    assert_eq!(report.submit_count, 1);
    assert!(json.contains("aui-runtime-navigation-screenflow-textentry"));
    assert!(json.contains("imePlatformCoverage"));
}

#[test]
fn aui_submit_cancel_prioritizes_text_edit_before_screen_pop() {
    let document = navigation_text_entry_document();
    let mut state = AuiInteractionState::default();
    AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
        .expect("pause screen should push");
    let input_node = document
        .nodes
        .iter()
        .find(|node| node.node_id == "name_input")
        .expect("input node should exist");
    state.input_field = Some(AuiInputFieldState::start(input_node));
    state.input_mode = AuiInputMode::TextEditing {
        node_id: "name_input".to_string(),
    };
    let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
    let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
        key: "Enter".to_string(),
    }]);

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(result.text_submitted_count, 1);
    assert_eq!(result.screen_stack_pop_count, 0);
    assert_eq!(state.screen_stack.active_stack.len(), 1);
    assert!(result.commands.iter().any(|command| {
        command.command_kind == AuiCommandKind::TextSubmitted && command.source_node == "name_input"
    }));
}

#[test]
fn aui_effective_visibility_hidden_parent_keeps_layout_but_blocks_draw_and_hit() {
    let hidden_root = AuiNode::new("hidden-root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["visible-child"]);
    let child = AuiNode::new(
        "visible-child",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 30.0, 160.0, 48.0),
    )
    .with_parent("hidden-root")
    .with_interactable(true)
    .with_text("Hidden action");
    let mut document = AuiDocument::new(
        "effective-visibility",
        vec![AuiCanvas::screen_overlay(
            "main",
            320.0,
            180.0,
            "hidden-root",
        )],
        vec![hidden_root, child],
    );
    document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "hidden-root")
        .expect("root")
        .visible = false;

    let layout = AuiLayoutEngine::layout(&document, 1);
    let root = layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "hidden-root")
        .expect("root layout");
    let child = layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "visible-child")
        .expect("child layout must remain available");
    assert!(!root.local_visible);
    assert!(!root.effective_visible);
    assert!(child.local_visible);
    assert!(!child.effective_visible);
    assert_eq!(
        child.rect,
        AuiComputedRect {
            x: 20.0,
            y: 30.0,
            width: 160.0,
            height: 48.0,
        }
    );
    assert_eq!(layout.report.node_count, 2);
    assert_eq!(layout.report.visible_node_count, 0);

    let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    assert!(draw_list.commands.is_empty());
    let hit = AuiInteractionSystem::hit_test(&document, &layout, 40.0, 40.0);
    assert_eq!(hit.reason, AuiHitTestReason::OutsideUi);
    assert!(hit.hit_node.is_none());
}

#[test]
fn aui_effective_visibility_reconciles_hidden_interaction_state_once() {
    let hidden_root = AuiNode::new("hidden-root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["input"]);
    let input = AuiNode::new(
        "input",
        AuiNodeKind::InputField,
        AuiRect::fixed_position(10.0, 10.0, 180.0, 40.0),
    )
    .with_parent("hidden-root")
    .with_interactable(true);
    let mut modal_canvas = AuiCanvas::screen_overlay("modal", 320.0, 180.0, "hidden-root");
    modal_canvas.composition_stage = AuiCompositionStage::Modal;
    let mut document = AuiDocument::new(
        "effective-visibility-state",
        vec![modal_canvas],
        vec![hidden_root, input],
    );
    document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "hidden-root")
        .expect("root")
        .visible = false;
    let input_node = document
        .nodes
        .iter()
        .find(|node| node.node_id == "input")
        .expect("input");
    let mut input_field = AuiInputFieldState::start(input_node);
    input_field.composition = Some(AuiTextCompositionState {
        preedit_text: "draft".to_string(),
        cursor_start: 0,
        cursor_end: 5,
        active: true,
    });
    let mut state = AuiInteractionState {
        active_drag: Some(AuiActiveDrag {
            source_node: "input".to_string(),
            start_pointer: AuiPointer::new(12.0, 12.0),
            current_pointer: AuiPointer::new(20.0, 20.0),
            started: true,
            pointer_id: 0,
            device_kind: RuntimePointerDeviceKind::Mouse,
        }),
        primary_press: Some(AuiPrimaryPressCapture {
            node_id: "input".to_string(),
            pointer_id: 0,
            device_kind: RuntimePointerDeviceKind::Mouse,
            hover_capable: true,
            inside: true,
        }),
        focus: AuiFocusState {
            focused_node: Some("input".to_string()),
            focus_scope_root: Some("hidden-root".to_string()),
            focus_reason: AuiFocusReason::Pointer,
        },
        active_modal_root: Some("hidden-root".to_string()),
        input_mode: AuiInputMode::TextEditing {
            node_id: "input".to_string(),
        },
        input_field: Some(input_field),
        active_scroll_capture: Some(AuiActiveScrollCapture {
            node_id: "input".to_string(),
            captured_node_id: "input".to_string(),
            start_pointer: AuiPointer::new(12.0, 12.0),
            last_pointer: AuiPointer::new(20.0, 20.0),
            started: true,
            scroll_delta_per_pointer_delta_y: None,
            pointer_id: 0,
            device_kind: RuntimePointerDeviceKind::Mouse,
        }),
        ..Default::default()
    };
    let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
    let empty_frame = pointer_frame(Vec::new());

    let first = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &empty_frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert!(first.visibility_reconciliation_count >= 7);
    assert_eq!(first.ime_cancel_count, 1);
    assert!(first.actions.is_empty());
    assert!(first.commands.is_empty());
    assert!(state.focus.focused_node.is_none());
    assert!(state.focus.focus_scope_root.is_none());
    assert!(state.pressed_node().is_none());
    assert!(state.active_drag.is_none());
    assert!(state.active_scroll_capture.is_none());
    assert!(state.input_field.is_none());
    assert!(state.active_modal_root.is_none());
    assert_eq!(state.input_mode, AuiInputMode::Navigation);

    let second = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &empty_frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(second.visibility_reconciliation_count, 0);
    assert_eq!(second.ime_cancel_count, 0);
    assert!(second.actions.is_empty());
    assert!(second.commands.is_empty());
}

#[test]
fn aui_screen_flow_push_pop_uses_canvas_visibility_and_focus_restore() {
    let document = navigation_text_entry_document();
    let mut state = AuiInteractionState::default();
    state.focus.focused_node = Some("play_button".to_string());
    let initial_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
    assert!(initial_layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "name_input")
        .is_none_or(|node| !node.effective_visible));

    AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
        .expect("pause screen should push");
    let pushed_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 2, &state);
    assert!(pushed_layout
        .computed_nodes
        .iter()
        .any(|node| node.node_id == "name_input" && node.effective_visible));
    assert_eq!(state.focus.focused_node.as_deref(), Some("name_input"));

    let frame = pointer_frame(vec![RuntimeInputEvent::KeyDown {
        key: "Escape".to_string(),
    }]);
    let result = AuiInteractionSystem::process_with_state(
        &document,
        &pushed_layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    let popped_layout = AuiLayoutEngine::layout_with_interaction_state(&document, 3, &state);

    assert_eq!(result.screen_stack_push_count, 1);
    assert_eq!(result.screen_stack_pop_count, 1);
    assert_eq!(result.focus_restore_count, 1);
    assert_eq!(state.focus.focused_node.as_deref(), Some("play_button"));
    assert!(popped_layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "name_input")
        .is_none_or(|node| !node.effective_visible));
}

#[test]
fn aui_gamepad_navigation_moves_focus_and_submit() {
    let document = navigation_text_entry_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let mut state = AuiInteractionState::default();
    state.focus.focused_node = Some("play_button".to_string());
    let move_frame = pointer_frame(vec![RuntimeInputEvent::GamepadButtonDown {
        gamepad_id: 0,
        button: "DPadDown".to_string(),
    }]);
    let move_result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &move_frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(state.focus.focused_node.as_deref(), Some("settings_button"));
    assert_eq!(move_result.gamepad_intent_count, 1);
    assert_eq!(move_result.keyboard_navigation_event_count, 1);

    let submit_frame = pointer_frame(vec![RuntimeInputEvent::GamepadButtonDown {
        gamepad_id: 0,
        button: "South".to_string(),
    }]);
    let submit_result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &submit_frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(submit_result.submit_count, 1);
    assert_eq!(submit_result.gamepad_intent_count, 1);
    assert!(submit_result.actions.iter().any(|action| {
        action.event == AuiActionEvent::Submit && action.action_id == "ui.settings"
    }));
}

#[test]
fn aui_activate_maps_each_operation_once_and_explicit_binding_wins() {
    let mut document = navigation_text_entry_document();
    let button = document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "play_button")
        .unwrap();
    button.action_refs = vec![
        AuiActionRef::activate("start"),
        AuiActionRef::click("start"),
        AuiActionRef::activate("sound"),
    ];
    let commands = [
        AuiCommand::new(0, "play_button", AuiCommandKind::Click),
        AuiCommand::new(1, "play_button", AuiCommandKind::Submit),
        AuiCommand::new(2, "play_button", AuiCommandKind::Click),
    ];
    let actions = AuiActionMapper::map(&document, &commands);
    assert_eq!(actions.len(), 6);
    let starts: Vec<_> = actions
        .iter()
        .filter(|a| a.action_id == "start")
        .map(|a| a.event)
        .collect();
    assert_eq!(
        starts,
        vec![
            AuiActionEvent::Click,
            AuiActionEvent::Activate,
            AuiActionEvent::Click
        ]
    );
    let wire = serde_json::to_string(&document).unwrap();
    let decoded: AuiDocument = serde_json::from_str(&wire).unwrap();
    assert_eq!(AuiActionMapper::map(&decoded, &commands), actions);
}

#[test]
fn virtual_list_actions_project_stable_key_without_changing_authored_schema() {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["inventory:virtual-item:0"]);
    let mut item = AuiNode::new(
        "inventory:virtual-item:0",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 200.0, 40.0),
    )
    .with_parent("root")
    .with_action(AuiActionRef::activate("inventory.select"));
    item.virtual_list_item = Some(crate::aui_virtual_list::AuiVirtualListItemContext {
        list_node_id: "inventory".into(),
        item_key: "item-0042".into(),
        item_index: 42,
    });
    let document = AuiDocument::new(
        "virtual-actions",
        vec![AuiCanvas::screen_overlay("main", 320.0, 200.0, "root")],
        vec![root, item],
    );
    let commands = [AuiCommand::new(
        0,
        "inventory:virtual-item:0",
        AuiCommandKind::Submit,
    )];

    let action = AuiActionMapper::map(&document, &commands).remove(0);
    let payload: crate::aui_virtual_list::AuiVirtualListActionPayload =
        serde_json::from_str(action.payload.as_deref().unwrap()).unwrap();
    assert_eq!(payload.list_node_id, "inventory");
    assert_eq!(payload.item_key, "item-0042");
    assert_eq!(payload.item_index, 42);
    assert!(!serde_json::to_string(&document)
        .unwrap()
        .contains("virtual_list_item"));

    let mut explicit = commands[0].clone();
    explicit.payload = Some("project-payload".into());
    assert_eq!(
        AuiActionMapper::map(&document, &[explicit])[0]
            .payload
            .as_deref(),
        Some("project-payload")
    );
}

#[test]
fn aui_activate_pointer_and_keyboard_obey_focus_disabled_and_consumption() {
    let mut document = navigation_text_entry_document();
    document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "play_button")
        .unwrap()
        .action_refs = vec![AuiActionRef::activate("start")];
    let layout = AuiLayoutEngine::layout(&document, 1);
    for events in [
        vec![
            RuntimeInputEvent::PointerDown {
                x: 30.0,
                y: 30.0,
                button: RuntimePointerButton::Primary,
            },
            RuntimeInputEvent::PointerUp {
                x: 30.0,
                y: 30.0,
                button: RuntimePointerButton::Primary,
            },
        ],
        vec![RuntimeInputEvent::KeyDown {
            key: "Enter".into(),
        }],
        vec![RuntimeInputEvent::GamepadButtonDown {
            gamepad_id: 0,
            button: "South".into(),
        }],
    ] {
        let frame = pointer_frame(events);
        let mut state = AuiInteractionState::default();
        state.focus.focused_node = Some("play_button".into());
        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        let actions: Vec<_> = result
            .actions
            .iter()
            .filter(|a| a.action_id == "start")
            .collect();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].event, AuiActionEvent::Activate);
        assert!(!result.consumed_event_indices.is_empty());
    }
    document
        .nodes
        .iter_mut()
        .find(|node| node.node_id == "play_button")
        .unwrap()
        .interactable = false;
    let layout = AuiLayoutEngine::layout(&document, 2);
    for event in [
        RuntimeInputEvent::KeyDown {
            key: "Enter".into(),
        },
        RuntimeInputEvent::GamepadButtonDown {
            gamepad_id: 0,
            button: "South".into(),
        },
    ] {
        let mut state = AuiInteractionState::default();
        state.focus.focused_node = Some("play_button".into());
        let frame = pointer_frame(vec![event]);
        let result = AuiInteractionSystem::process_with_state(
            &document,
            &layout,
            &frame,
            &mut state,
            AuiInteractionConfig::default(),
        );
        assert!(!result.actions.iter().any(|a| a.action_id == "start"));
    }
}

#[test]
fn aui_input_field_text_edit_changes_submits_and_cancels() {
    let document = navigation_text_entry_document();
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 250.0,
            y: 70.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::TextInput {
            text: "B".to_string(),
        },
        RuntimeInputEvent::KeyDown {
            key: "Backspace".to_string(),
        },
        RuntimeInputEvent::KeyDown {
            key: "Escape".to_string(),
        },
    ]);
    let mut state = AuiInteractionState::default();
    AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
        .expect("pause screen should push");
    let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );

    assert_eq!(result.text_edit_session_count, 1);
    assert_eq!(result.text_changed_count, 2);
    assert_eq!(result.text_cancelled_count, 1);
    assert_eq!(state.input_mode, AuiInputMode::Navigation);
    assert!(result.actions.iter().any(|action| {
        action.event == AuiActionEvent::TextCancelled && action.action_id == "ui.name_cancelled"
    }));
}

#[test]
fn aui_ime_preedit_commit_and_cancel_updates_text_edit_state() {
    let document = navigation_text_entry_document();
    let mut state = AuiInteractionState::default();
    AuiInteractionSystem::push_screen(&document, &mut state, "pause_screen")
        .expect("pause screen should push");
    let layout = AuiLayoutEngine::layout_with_interaction_state(&document, 1, &state);
    let frame = pointer_frame(vec![
        RuntimeInputEvent::PointerDown {
            x: 250.0,
            y: 70.0,
            button: RuntimePointerButton::Primary,
        },
        RuntimeInputEvent::ImePreedit {
            text: "ni".to_string(),
            cursor_start: 0,
            cursor_end: 2,
        },
        RuntimeInputEvent::ImeCancel,
        RuntimeInputEvent::ImeCommit {
            text: "hao".to_string(),
        },
        RuntimeInputEvent::KeyDown {
            key: "Enter".to_string(),
        },
    ]);

    let result = AuiInteractionSystem::process_with_state(
        &document,
        &layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    let report = AuiRuntimeNavigationScreenFlowTextEntryProductizationReport::from_result(
        &document,
        frame.events.len(),
        frame
            .filter_consumed_events(&result.consumed_event_indices)
            .events
            .len(),
        &result,
    );

    assert_eq!(result.ime_preedit_count, 1);
    assert_eq!(result.ime_cancel_count, 1);
    assert_eq!(result.ime_commit_count, 1);
    assert_eq!(result.text_changed_count, 1);
    assert_eq!(result.text_submitted_count, 1);
    assert_eq!(report.ime_platform_coverage, AUI_IME_PLATFORM_COVERAGE);
    assert!(!report.ime_candidate_window_deferred);
    assert!(!report.rich_text_deferred);
}

#[test]
fn aui_feedback_draw_transforms_owned_subtree_without_mutating_layout_or_clip() {
    let root =
        AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full()).with_children(["button"]);
    let button = AuiNode::new(
        "button",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20.0, 30.0, 100.0, 60.0),
    )
    .with_parent("root")
    .with_children(["icon", "caption"])
    .with_interactable(true)
    .with_style(AuiStyle {
        color: Some("#808080".to_string()),
        text_color: Some("#ffffff".to_string()),
        font_size: Some(18.0),
        font: None,
    });
    let icon = AuiNode::new(
        "icon",
        AuiNodeKind::Image,
        AuiRect::fixed_position(10.0, 10.0, 20.0, 20.0),
    )
    .with_parent("button")
    .with_image("ui/icon")
    .with_style(AuiStyle {
        color: Some("#4080c0".to_string()),
        text_color: None,
        font_size: None,
        font: None,
    });
    let caption = AuiNode::new(
        "caption",
        AuiNodeKind::Text,
        AuiRect::fixed_position(40.0, 10.0, 50.0, 20.0),
    )
    .with_parent("button")
    .with_text("Play")
    .with_style(AuiStyle::text("#ffffff", 16.0));
    let document = AuiDocument::new(
        "feedback-draw",
        vec![AuiCanvas::screen_overlay("main", 200.0, 200.0, "root")],
        vec![root, button, icon, caption],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    let layout_bytes = serde_json::to_vec(&layout).unwrap();
    let (plain, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let mut overrides = AuiVisualOverrideSet::default();
    overrides.set(
        "button",
        AuiControlVisualOverride {
            scale: 0.9,
            translation: AuiVec2::new(2.0, 3.0),
            brightness_multiplier: 0.8,
            opacity_multiplier: 0.5,
        },
    );
    let (animated, _) =
        AuiLayoutEngine::extract_draw_list_with_visual_overrides(&document, &layout, &overrides);

    assert_eq!(plain.commands.len(), animated.commands.len());
    assert_eq!(serde_json::to_vec(&layout).unwrap(), layout_bytes);
    for (before, after) in plain.commands.iter().zip(&animated.commands) {
        assert_eq!(
            draw_command_effective_clip_rect(before),
            draw_command_effective_clip_rect(after)
        );
        assert_eq!(
            std::mem::discriminant(before.as_ref()),
            std::mem::discriminant(after.as_ref())
        );
    }
    let plain_icon = plain.commands.iter().find(|command| matches!(command.as_ref(), AuiDrawCommand::DrawImage { node_id, .. } if node_id == "icon")).unwrap();
    let animated_icon = animated.commands.iter().find(|command| matches!(command.as_ref(), AuiDrawCommand::DrawImage { node_id, .. } if node_id == "icon")).unwrap();
    let (plain_rect, animated_rect, animated_color) =
        match (plain_icon.as_ref(), animated_icon.as_ref()) {
            (
                AuiDrawCommand::DrawImage { rect: before, .. },
                AuiDrawCommand::DrawImage {
                    rect: after, color, ..
                },
            ) => (*before, *after, color.as_deref()),
            _ => unreachable!(),
        };
    assert!(animated_rect.width < plain_rect.width);
    assert!(animated_rect.x > plain_rect.x);
    assert_eq!(animated_color, Some("#3973ae80"));
}

#[test]
fn aui_feedback_draw_empty_override_is_byte_equivalent() {
    let document = interaction_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (plain, plain_report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let (overridden, overridden_report) = AuiLayoutEngine::extract_draw_list_with_visual_overrides(
        &document,
        &layout,
        &AuiVisualOverrideSet::default(),
    );
    assert_eq!(plain, overridden);
    assert_eq!(plain_report, overridden_report);
}

#[test]
fn aui_interaction_productization_report_exposes_deferred_flags() {
    let document = drag_document();
    let layout = AuiLayoutEngine::layout(&document, 1);
    let frame = pointer_frame(vec![RuntimeInputEvent::PointerDown {
        x: 60.0,
        y: 60.0,
        button: RuntimePointerButton::Primary,
    }]);
    let result = AuiInteractionSystem::process(&document, &layout, &frame);
    let filtered = frame.filter_consumed_events(&result.consumed_event_indices);

    let report = AuiInteractionProductizationReport::from_result(
        &document,
        frame.events.len(),
        filtered.events.len(),
        &result,
        AuiInteractionConfig::default(),
        None,
    );

    assert_eq!(
        report.schema_version,
        AUI_INTERACTION_PRODUCTIZATION_REPORT_SCHEMA_VERSION
    );
    assert_eq!(report.drag_threshold_px, 4.0);
    assert_eq!(report.snapshot_frame_lag, 1);
    assert!(report.authoring_action_payload_deferred);
    assert!(!report.modal_input_blocking_deferred);
    assert!(!report.editor_hit_test_deferred_to_209);
    assert!(!report.control_style_deferred);
    assert!(!report.slider_toggle_binding_target_deferred);
    assert_eq!(report.consumed_pointer_event_count, 1);
    assert_eq!(report.filtered_input_event_count, 0);
    assert_eq!(
        report.consumed_event_count_by_kind.get("PointerDown"),
        Some(&1)
    );
}
