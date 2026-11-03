use engine_input::{RuntimeInputEvent, RuntimeInputFrame};
use engine_runtime::aui::*;
use engine_runtime::aui_layout_runtime::{AuiAvailableSpace, AuiIntrinsicMeasure, AuiMeasureInput};
use engine_runtime::aui_layout_schema::*;
use engine_runtime::aui_paragraph::*;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::font_bundle::*;
use std::sync::Arc;

#[test]
fn gate_c_layout_input_edits_keep_mounted_solver_and_match_fresh() {
    let fonts = font_registry();
    let mut document = c2_document("人马骑兵");
    let mut runtime = AuiC2Runtime::default();
    runtime.layout(&document, 1, &fonts);
    let builds = runtime.solver_build_count();
    for frame in 2..=4 {
        match frame {
            2 => document.nodes[1].slot.as_mut().unwrap().width = Dimension::Fixed { value: 65.0 },
            3 => document.nodes[1].style.as_mut().unwrap().font_size = Some(25.0),
            _ => document.canvases[0].reference_resolution.x = 260.0,
        }
        let (actual, paragraphs) = runtime.layout(&document, frame, &fonts);
        let (fresh, fresh_paragraphs) = AuiC2Runtime::default().layout(&document, frame, &fonts);
        assert_eq!(actual, fresh, "frame {frame}");
        assert_eq!(paragraphs, fresh_paragraphs, "frame {frame}");
        assert_eq!(
            runtime.solver_build_count(),
            builds,
            "frame {frame} rebuilt solver"
        );
        assert_eq!(runtime.layout_stats("main").unwrap().source_nodes_loaded, 0);
    }
}

#[test]
fn gate_c_independent_canvases_share_source_and_only_rebuild_changed_topology() {
    let fonts = font_registry();
    let mut document = c2_document("人");
    let mut root = document.nodes[0].clone();
    root.node_id = "root-2".into();
    root.children = vec!["label-2".into()];
    let mut label = document.nodes[1].clone();
    label.node_id = "label-2".into();
    label.parent = Some("root-2".into());
    document.nodes.extend([root, label]);
    document
        .canvases
        .push(AuiCanvas::screen_overlay("second", 320., 240., "root-2"));
    let mut runtime = AuiC2Runtime::default();
    let (initial, _) = runtime.layout(&document, 1, &fonts);
    assert!(initial.diagnostics.is_empty());
    assert_eq!(runtime.layout_source_nodes_copied(), 4);
    assert_eq!(runtime.solver_node_count("main"), Some(2));
    assert_eq!(runtime.solver_node_count("second"), Some(2));
    let mut builds = 2;
    for frame in 2..=7 {
        match frame {
            2 => document.nodes[1].slot.as_mut().unwrap().width = Dimension::Fixed { value: 95. },
            3 => document.nodes[1].style.as_mut().unwrap().font_size = Some(30.),
            4 => document.canvases[1].reference_resolution.x = 400.,
            5 => {
                let extra = AuiNode::new(
                    "extra",
                    AuiNodeKind::Panel,
                    AuiRect::fixed_position(5., 6., 10., 10.),
                )
                .with_parent("root");
                document.nodes[0].children.push("extra".into());
                // Shift every global source slot while the second root remains unchanged.
                document.nodes.insert(0, extra);
                builds += 1;
            }
            6 => {
                document.nodes.remove(0);
                document.nodes[0].children.retain(|id| id != "extra");
                builds += 1;
            }
            _ => document.nodes[3].text = Some("骑兵".into()),
        }
        let (actual, paragraphs) = runtime.layout(&document, frame, &fonts);
        let (fresh, fresh_paragraphs) = AuiC2Runtime::default().layout(&document, frame, &fonts);
        assert_eq!(actual, fresh, "frame {frame}");
        assert_eq!(paragraphs, fresh_paragraphs, "frame {frame}");
        assert_eq!(runtime.solver_build_count(), builds, "frame {frame}");
        let unchanged = if matches!(frame, 4 | 7) {
            "main"
        } else {
            "second"
        };
        assert!(
            runtime
                .layout_stats(unchanged)
                .unwrap()
                .clean_snapshot_reused,
            "frame {frame}"
        );
        assert_eq!(
            runtime.layout_stats(unchanged).unwrap().source_nodes_loaded,
            0
        );
        if matches!(frame, 2..=4 | 7) {
            assert!(
                runtime.geometry_update_stats().final_base_reused,
                "frame {frame}"
            );
            assert_eq!(runtime.layout_source_nodes_copied(), 0);
        } else {
            assert_eq!(runtime.layout_source_nodes_copied(), document.nodes.len());
        }
    }
}

#[test]
fn gate_c_translation_journal_compares_only_changed_entries_among_ten_thousand_offsets() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    for count in [400, 1000, 10_000] {
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.layout = Some(LayoutSpec::default());
        root.clip_policy = AuiClipPolicy::Rect;
        let mut nodes = vec![];
        for i in 0..count {
            let id = format!("leaf-{i}");
            root.children.push(id.clone());
            nodes.push(
                AuiNode::new(
                    &id,
                    AuiNodeKind::Panel,
                    AuiRect::fixed_position(i as f32 * 20., 0., 10., 10.),
                )
                .with_parent("root"),
            );
        }
        nodes.insert(0, root);
        let mut document = AuiDocument::new(
            "translated",
            vec![AuiCanvas::screen_overlay(
                "main",
                count as f32 * 20.,
                100.,
                "root",
            )],
            nodes,
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        for start in (0..count).step_by(2000) {
            let handles: Vec<_> = (start..(start + 2000).min(count))
                .map(|i| instance.node(&format!("leaf-{i}")).unwrap())
                .collect();
            let mut edit = instance.edit();
            for handle in handles {
                edit.write(
                    handle,
                    UiProperty::Translation,
                    UiValue::Translation([1., 2.]),
                )
                .unwrap();
            }
            edit.commit().unwrap();
        }
        let fonts = RuntimeFontBundleRegistry::default();
        let mut runtime = AuiC2Runtime::default();
        runtime.observe_retained(Some(&instance));
        runtime.layout(&document, 1, &fonts);
        let a = instance.node("leaf-0").unwrap();
        let b = instance.node("leaf-1").unwrap();
        // Two observations without a presentation must accumulate both changed IDs.
        for (handle, offset) in [(a, [4., 5.]), (b, [6., 7.])] {
            let mut edit = instance.edit();
            edit.write(
                handle,
                UiProperty::Translation,
                UiValue::Translation(offset),
            )
            .unwrap();
            edit.commit().unwrap();
            runtime.observe_retained(Some(&instance));
        }
        for frame in 2..=4 {
            if frame == 3 {
                let mut edit = instance.edit();
                edit.write(a, UiProperty::Translation, UiValue::Translation([0., 0.]))
                    .unwrap();
                edit.commit().unwrap();
                runtime.observe_retained(Some(&instance));
            }
            let (actual, _) = runtime.layout(&document, frame, &fonts);
            assert_eq!(
                runtime.geometry_update_stats().translation_entries_compared,
                match frame {
                    2 => 2,
                    3 => 1,
                    _ => 0,
                },
                "{count} nodes/frame {frame}"
            );
            assert_eq!(runtime.layout_stats("main").unwrap().measure_calls, 0);
            let mut fresh = AuiC2Runtime::default();
            fresh.observe_retained(Some(&instance));
            let (expected, _) = fresh.layout(&document, frame, &fonts);
            assert_eq!(actual, expected, "{count} nodes/frame {frame}");
            assert_eq!(
                AuiInteractionSystem::hit_test(&document, &actual, 5., 5.),
                AuiInteractionSystem::hit_test(&document, &expected, 5., 5.)
            );
        }
        runtime.observe_retained(None);
        let (restored, _) = runtime.layout(&document, 5, &fonts);
        let (expected, _) = AuiC2Runtime::default().layout(&document, 5, &fonts);
        assert_eq!(restored, expected);
        assert_eq!(
            runtime.geometry_update_stats().translation_entries_compared,
            count
        );
    }
}

#[test]
fn gate_c_invalid_parent_constraints_do_not_poison_restored_mounted_inputs() {
    let fonts = RuntimeFontBundleRegistry::default();
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        ..Default::default()
    });
    root.children = vec!["parent".into()];
    let mut parent =
        AuiNode::new("parent", AuiNodeKind::Panel, AuiRect::stretch_full()).with_parent("root");
    parent.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        ..Default::default()
    });
    parent.slot = Some(SlotSpec {
        width: Dimension::Fixed { value: 80. },
        height: Dimension::Fixed { value: 40. },
        ..Default::default()
    });
    parent.children = vec!["child".into()];
    let mut child =
        AuiNode::new("child", AuiNodeKind::Panel, AuiRect::stretch_full()).with_parent("parent");
    child.slot = Some(SlotSpec {
        width: Dimension::Percent { value: 0.5 },
        height: Dimension::Fixed { value: 10. },
        ..Default::default()
    });
    let mut document = AuiDocument::new(
        "constraints",
        vec![AuiCanvas::screen_overlay("main", 160., 100., "root")],
        vec![root, parent, child],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    let mut runtime = AuiC2Runtime::default();
    let (initial, _) = runtime.layout(&document, 1, &fonts);
    assert!(initial.diagnostics.is_empty());
    let mut batch_document = document.clone();
    batch_document.nodes[1].slot.as_mut().unwrap().width = Dimension::Content;
    batch_document.nodes[2].slot.as_mut().unwrap().width = Dimension::Fixed { value: 40. };
    let mut batch_runtime = AuiC2Runtime::default();
    batch_runtime.layout(&document, 1, &fonts);
    let (batch, _) = batch_runtime.layout(&batch_document, 2, &fonts);
    let (batch_fresh, _) = AuiC2Runtime::default().layout(&batch_document, 2, &fonts);
    assert_eq!(
        batch, batch_fresh,
        "all changed inputs must be installed before deriving constraints"
    );
    document.nodes[1].slot.as_mut().unwrap().width = Dimension::Content;
    let (failed, _) = runtime.layout(&document, 2, &fonts);
    assert!(failed
        .diagnostics
        .iter()
        .any(|d| d.code == "aui_layout_indefinite_percent_basis"));
    document.nodes[1].slot.as_mut().unwrap().width = Dimension::Fixed { value: 80. };
    let (restored, _) = runtime.layout(&document, 3, &fonts);
    assert_eq!(restored.computed_nodes, initial.computed_nodes);
    document.nodes[2].slot.as_mut().unwrap().height = Dimension::Fixed { value: 20. };
    let (changed, _) = runtime.layout(&document, 4, &fonts);
    assert!(changed.diagnostics.is_empty(), "{:?}", changed.diagnostics);
    let (fresh, _) = AuiC2Runtime::default().layout(&document, 4, &fonts);
    assert_eq!(changed, fresh);
}

#[test]
fn gate_c_same_frame_drag_capture_uses_updated_geometry_for_move_and_drop() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.children = vec!["source".into(), "target".into()];
    root.clip_policy = AuiClipPolicy::Rect;
    let source = AuiNode::new(
        "source",
        AuiNodeKind::Button,
        AuiRect::fixed_position(20., 10., 20., 20.),
    )
    .with_parent("root")
    .with_draggable();
    let target = AuiNode::new(
        "target",
        AuiNodeKind::Button,
        AuiRect::fixed_position(140., 10., 20., 20.),
    )
    .with_parent("root")
    .with_drop_target();
    let mut document = AuiDocument::new(
        "drag",
        vec![AuiCanvas::screen_overlay("main", 200., 100., "root")],
        vec![root, source, target],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let fonts = RuntimeFontBundleRegistry::default();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let (initial, _) = runtime.layout(&document, 1, &fonts);
    let mut state = AuiInteractionState::default();
    let mut frame = RuntimeInputFrame::new(2, "game-view");
    frame.events = vec![RuntimeInputEvent::PointerDown {
        x: 25.,
        y: 15.,
        button: engine_input::RuntimePointerButton::Primary,
    }];
    AuiInteractionSystem::process_with_state(
        &document,
        &initial,
        &frame,
        &mut state,
        Default::default(),
    );
    let mut fresh_state = state.clone();
    let source = instance.node("source").unwrap();
    let target = instance.node("target").unwrap();
    let mut edit = instance.edit();
    edit.write(
        source,
        UiProperty::Translation,
        UiValue::Translation([30., 0.]),
    )
    .unwrap();
    edit.write(
        target,
        UiProperty::Translation,
        UiValue::Translation([10., 0.]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let (updated, _) = runtime.layout(&document, 2, &fonts);
    frame.events = vec![
        RuntimeInputEvent::PointerMove { x: 85., y: 15. },
        RuntimeInputEvent::PointerUp {
            x: 155.,
            y: 15.,
            button: engine_input::RuntimePointerButton::Primary,
        },
    ];
    let actual = AuiInteractionSystem::process_with_state(
        &document,
        &updated,
        &frame,
        &mut state,
        Default::default(),
    );
    assert!(actual
        .commands
        .iter()
        .any(|c| c.command_kind == AuiCommandKind::DragStart));
    let drop = actual
        .commands
        .iter()
        .find(|c| c.command_kind == AuiCommandKind::Drop)
        .unwrap();
    let payload: AuiDragDropPayload =
        serde_json::from_str(drop.payload.as_deref().unwrap()).unwrap();
    assert_eq!(payload.target_node.as_deref(), Some("target"));
    assert!(!actual
        .commands
        .iter()
        .any(|c| c.command_kind == AuiCommandKind::Click));
    assert_eq!(state.hit_test_index_stats().full_rebuild_count, 1);
    let mut fresh = AuiC2Runtime::default();
    fresh.observe_retained(Some(&instance));
    let (full, _) = fresh.layout(&document, 2, &fonts);
    let expected = AuiInteractionSystem::process_with_state(
        &document,
        &full,
        &frame,
        &mut fresh_state,
        Default::default(),
    );
    assert_eq!(actual.commands, expected.commands);
    assert_eq!(actual.control_snapshot, expected.control_snapshot);
}

fn font_registry() -> RuntimeFontBundleRegistry {
    let bytes = include_bytes!("../../../resources/editor/fonts/NotoSansSC-VF.ttf").to_vec();
    let source_digest = sha256_prefixed(&bytes);
    let metadata = CookedFontBundleAsset {
        schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.into(),
        font_bundle_id: "test-font".into(),
        font_stack_id: "test-stack".into(),
        generation: 1,
        max_bitmap_pages: 1,
        max_msdf_pages: 1,
        legacy_mode: false,
        fallback_used: false,
        quality_gate_eligible: true,
        pages: Vec::new(),
        glyphs: Vec::new(),
        kerning_adjustments: Vec::new(),
        font_faces: vec![RuntimeFontFaceDescriptor {
            font_face_id: "noto".into(),
            face_index: 0,
            source_digest: source_digest.clone(),
            payload_path: "noto.ttf".into(),
        }],
        shaping_recipe: Some(engine_runtime::text_shaping::SHAPING_RECIPE.into()),
        bundle_digest: "sha256:test-font-bundle".into(),
    };
    let mut registry = RuntimeFontBundleRegistry {
        default_ui_font_bundle_id: Some("test-font".into()),
        ..Default::default()
    };
    std::sync::Arc::make_mut(&mut registry.bundles_by_id).insert(
        "test-font".into(),
        RuntimeLoadedFontBundle {
            metadata,
            page_payloads: Vec::new(),
            font_face_sources: vec![RuntimePackageSourceFontFace {
                font_face_id: "noto".into(),
                face_index: 0,
                source_digest,
                bytes,
            }],
        },
    );
    registry
}

fn renderable_font_registry(text: &str) -> RuntimeFontBundleRegistry {
    let mut registry = font_registry();
    let bundle = std::sync::Arc::make_mut(&mut registry.bundles_by_id)
        .get_mut("test-font")
        .unwrap();
    bundle.metadata.pages.push(CookedFontBundlePage {
        page_index: 0,
        render_mode: FontBundleRenderMode::BitmapR8,
        format: "r8Unorm".into(),
        width: 64,
        height: 64,
        byte_len: 4096,
        sha256: "sha256:test-page".into(),
        payload_path: "page.r8".into(),
    });
    bundle.page_payloads.push(vec![0; 4096]);
    let mut glyph_ids = engine_runtime::text_shaping::shape_text(
        "noto",
        &bundle.font_face_sources[0].bytes,
        &format!("{text}…"),
    )
    .unwrap()
    .items
    .into_iter()
    .filter(|glyph| glyph.has_outline)
    .map(|glyph| glyph.glyph_id as u16)
    .collect::<Vec<_>>();
    glyph_ids.sort_unstable();
    glyph_ids.dedup();
    bundle.metadata.glyphs = glyph_ids
        .into_iter()
        .enumerate()
        .map(|(index, glyph_id)| CookedFontBundleGlyph {
            font_family_id: "family-ui".into(),
            font_face_id: "noto".into(),
            style: FontBundleStyle::Normal,
            weight: 400,
            glyph_id,
            codepoint: u32::MAX,
            render_mode: FontBundleRenderMode::BitmapR8,
            pixel_size: 20,
            page_index: 0,
            pixel_rect: [(index as u32 % 7) * 8, (index as u32 / 7) * 12, 8, 12],
            bearing_x: 0,
            bearing_y: 12,
            advance_per_em_millionths: 600_000,
            plane_bounds_per_em_millionths: None,
        })
        .collect();
    registry
}

fn request<'a>(
    text: &'a str,
    text_layout: &'a TextLayoutSpec,
    width: f32,
    fonts: &'a RuntimeFontBundleRegistry,
) -> ParagraphRequest<'a> {
    ParagraphRequest {
        text,
        font_size: 20.0,
        font: None,
        spans: &[],
        text_layout,
        constraint_width: width,
        box_height: None,
        font_bundles: fonts,
    }
}

#[test]
fn b10_text_resize_updates_sibling_clip_draw_and_hit_like_a_fresh_presenter() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人马骑兵");
    let mut document = c2_document("人");
    document.canvases[0].reference_resolution.y = 60.;
    document.nodes[0].overflow = Some(Overflow::Clip);
    let mut sibling = AuiNode::new("sibling", AuiNodeKind::Button, AuiRect::stretch_full())
        .with_parent("root")
        .with_interactable(true);
    sibling.slot = Some(SlotSpec {
        width: Dimension::Fixed { value: 60. },
        height: Dimension::Fixed { value: 20. },
        flex_shrink: 0.,
        ..Default::default()
    });
    document.nodes[1].slot.as_mut().unwrap().flex_shrink = 0.;
    document.nodes[0].children.push("sibling".into());
    document.nodes.push(sibling);
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let label = UiTextRef::try_new(&instance, instance.node("label").unwrap()).unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let initial = retained_present(&document, &fonts, &mut runtime);
    assert_eq!(
        AuiInteractionSystem::hit_test(&initial.resolved_document, &initial.layout, 10., 30.)
            .hit_node
            .as_deref(),
        Some("sibling")
    );
    for value in ["人马骑兵人马骑兵", "人", "人马骑兵"] {
        let mut edit = instance.edit();
        label.set_text(&mut edit, value).unwrap();
        edit.commit().unwrap();
        runtime.observe_retained(Some(&instance));
        let actual = retained_present(&document, &fonts, &mut runtime);
        assert_eq!(actual.report.status, AuiRuntimePresentStatus::Success);
        assert_eq!(runtime.layout_stats("main").unwrap().geometry_nodes, 2);
        assert_eq!(runtime.geometry_update_stats().nodes_updated, 2);
        let mut fresh = AuiC2Runtime::default();
        fresh.observe_retained(Some(&instance));
        let expected = retained_present(&document, &fonts, &mut fresh);
        assert_eq!(actual.layout, expected.layout);
        assert_eq!(actual.overlay, expected.overlay);
        for (x, y) in [(10., 10.), (10., 30.), (10., 55.), (10., 75.)] {
            assert_eq!(
                AuiInteractionSystem::hit_test(&actual.resolved_document, &actual.layout, x, y),
                AuiInteractionSystem::hit_test(&expected.resolved_document, &expected.layout, x, y)
            );
        }
        let hit =
            AuiInteractionSystem::hit_test(&actual.resolved_document, &actual.layout, 10., 30.);
        assert_eq!(
            hit.hit_node.as_deref(),
            Some(if value == "人" { "sibling" } else { "label" })
        );
    }
}

#[test]
fn b11_fixed_text_change_refreshes_one_of_one_thousand_final_nodes_and_paragraphs() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人马骑兵");
    let mut document = c2_document("人");
    document.nodes.clear();
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    document.canvases[0].root_node = "root".into();
    document.nodes.push(root);
    for index in 0..1000 {
        let id = format!("label-{index}");
        document.nodes[0].children.push(id.clone());
        let mut label = AuiNode::new(
            id,
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 60., 24.),
        );
        label.parent = Some("root".into());
        label.text = Some("人".into());
        label.style = Some(AuiStyle::text("#fff", 20.));
        label.slot = Some(SlotSpec {
            width: Dimension::Fixed { value: 60. },
            height: Dimension::Fixed { value: 24. },
            ..Default::default()
        });
        document.nodes.push(label);
    }
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let target = UiTextRef::try_new(&instance, instance.node("label-500").unwrap()).unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let first = retained_present(&document, &fonts, &mut runtime);
    let unrelated = first.paragraph_layouts["label-499"].clone();

    let mut edit = instance.edit();
    target.set_text(&mut edit, "骑兵").unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let actual = retained_present(&document, &fonts, &mut runtime);
    let stats = runtime.geometry_update_stats();
    assert!(stats.final_base_reused);
    assert!(!stats.base_reused);
    assert_eq!(stats.nodes_updated, 1);
    assert_eq!(stats.paragraphs_updated, 1);
    assert!(Arc::ptr_eq(
        &unrelated,
        &actual.paragraph_layouts["label-499"]
    ));

    let mut fresh = AuiC2Runtime::default();
    fresh.observe_retained(Some(&instance));
    let expected = retained_present(&document, &fonts, &mut fresh);
    assert_eq!(actual.layout, expected.layout);
    assert_eq!(actual.overlay, expected.overlay);
}

#[test]
fn b12_hit_index_coarsens_queries_and_updates_one_translated_slot() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiWrite};

    let fonts = font_registry();
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    let mut nodes = vec![root];
    for index in 0..1000 {
        let id = format!("cell-{index}");
        nodes[0].children.push(id.clone());
        let mut node = AuiNode::new(
            id,
            AuiNodeKind::Button,
            AuiRect::fixed_position(index as f32 * 320.0, 20.0, 40.0, 40.0),
        )
        .with_parent("root")
        .with_interactable(true);
        node.slot = Some(SlotSpec {
            participation: Participation::OutOfFlow,
            width: Dimension::Fixed { value: 40.0 },
            height: Dimension::Fixed { value: 40.0 },
            ..Default::default()
        });
        nodes.push(node);
    }
    let mut document = AuiDocument::new(
        "b12-hit-index",
        vec![AuiCanvas::screen_overlay("main", 320_000.0, 100.0, "root")],
        nodes,
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let target = instance.node("cell-500").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let first = retained_present(&document, &fonts, &mut runtime);
    let mut state = AuiInteractionState::default();
    let mut frame = RuntimeInputFrame::new(1, "game-view");
    frame.events = vec![RuntimeInputEvent::PointerMove {
        x: 500.0 * 320.0 + 10.0,
        y: 30.0,
    }];
    let hit = AuiInteractionSystem::process_with_state(
        &first.resolved_document,
        &first.layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(
        hit.control_snapshot.hovered_node.as_deref(),
        Some("cell-500")
    );
    let initial = state.hit_test_index_stats();
    assert_eq!(initial.full_rebuild_count, 1);
    assert_eq!(initial.node_table_rebuild_count, 1);
    assert!(initial.candidates_visited < 16, "{initial:?}");

    frame.frame_id = 2;
    AuiInteractionSystem::process_with_state(
        &first.resolved_document,
        &first.layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    let clean = state.hit_test_index_stats();
    assert_eq!(clean.full_rebuild_count, initial.full_rebuild_count);
    assert_eq!(
        clean.node_table_rebuild_count,
        initial.node_table_rebuild_count
    );
    assert_eq!(clean.slots_updated, initial.slots_updated);

    let mut edit = instance.edit();
    edit.write(
        target,
        UiProperty::Translation,
        UiValue::Translation([160.0, 0.0]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let moved = retained_present(&document, &fonts, &mut runtime);
    frame.frame_id = 3;
    frame.events = vec![RuntimeInputEvent::PointerMove {
        x: 500.0 * 320.0 + 170.0,
        y: 30.0,
    }];
    let hit = AuiInteractionSystem::process_with_state(
        &moved.resolved_document,
        &moved.layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(
        hit.control_snapshot.hovered_node.as_deref(),
        Some("cell-500")
    );
    let updated = state.hit_test_index_stats();
    assert_eq!(updated.full_rebuild_count, initial.full_rebuild_count);
    assert_eq!(
        updated.node_table_rebuild_count,
        initial.node_table_rebuild_count
    );
    assert_eq!(updated.slots_updated - initial.slots_updated, 1);
    assert_eq!(
        hit.control_snapshot.hovered_node,
        AuiInteractionSystem::hit_test(
            &moved.resolved_document,
            &moved.layout,
            500.0 * 320.0 + 170.0,
            30.0,
        )
        .hit_node
    );

    // Input may legitimately skip presentation frames. The retained journal must replay both
    // missing local updates rather than rebuilding or using an intermediate position.
    for offset in [480.0, 800.0] {
        let mut edit = instance.edit();
        edit.write(
            target,
            UiProperty::Translation,
            UiValue::Translation([offset, 0.0]),
        )
        .unwrap();
        edit.commit().unwrap();
        runtime.observe_retained(Some(&instance));
        let present = retained_present(&document, &fonts, &mut runtime);
        if offset == 800.0 {
            frame.frame_id = 4;
            frame.events = vec![RuntimeInputEvent::PointerMove {
                x: 500.0 * 320.0 + offset + 10.0,
                y: 30.0,
            }];
            let result = AuiInteractionSystem::process_with_state(
                &present.resolved_document,
                &present.layout,
                &frame,
                &mut state,
                AuiInteractionConfig::default(),
            );
            assert_eq!(
                result.control_snapshot.hovered_node.as_deref(),
                Some("cell-500")
            );
        }
    }
    let replayed = state.hit_test_index_stats();
    assert_eq!(replayed.full_rebuild_count, initial.full_rebuild_count);
    assert_eq!(replayed.slots_updated - updated.slots_updated, 2);

    // A different retained runtime has a distinct structural generation even when node count and
    // local revision happen to match, so stale buckets cannot be reused across sessions.
    let mut replacement_runtime = AuiC2Runtime::default();
    replacement_runtime.observe_retained(Some(&instance));
    let replacement = retained_present(&document, &fonts, &mut replacement_runtime);
    frame.frame_id = 5;
    AuiInteractionSystem::process_with_state(
        &replacement.resolved_document,
        &replacement.layout,
        &frame,
        &mut state,
        AuiInteractionConfig::default(),
    );
    assert_eq!(
        state.hit_test_index_stats().full_rebuild_count,
        replayed.full_rebuild_count + 1
    );
}

#[test]
fn b11_clip_radius_and_visibility_propagate_only_through_the_affected_subtree() {
    let fonts = renderable_font_registry("人");
    let mut document = c2_document("人");
    document.nodes[0].layout.as_mut().unwrap().mode = LayoutMode::Anchor;
    document.nodes[0].children = vec!["group".into(), "unrelated".into()];
    document.nodes[1].parent = Some("group".into());
    document.nodes[1].slot.as_mut().unwrap().height = Dimension::Fixed { value: 24. };
    let mut group = AuiNode::new(
        "group",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0., 0., 50., 30.),
    );
    group.parent = Some("root".into());
    group.children = vec!["label".into()];
    group.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    group.clip_policy = AuiClipPolicy::Rounded;
    group.clip_radius = Some(4.);
    let mut unrelated = AuiNode::new(
        "unrelated",
        AuiNodeKind::Text,
        AuiRect::fixed_position(80., 0., 40., 24.),
    );
    unrelated.parent = Some("root".into());
    unrelated.text = Some("人".into());
    unrelated.style = Some(AuiStyle::text("#fff", 20.));
    document.nodes.push(group);
    document.nodes.push(unrelated);

    let mut runtime = AuiC2Runtime::default();
    let (_, first_paragraphs) = runtime.layout(&document, 1, &fonts);
    let unrelated_paragraph = first_paragraphs["unrelated"].clone();
    let states = [
        (true, LayoutVisibility::Visible, 12., 2),
        (false, LayoutVisibility::Visible, 12., 2),
        // The group remains effectively invisible, so its child state is already current.
        (true, LayoutVisibility::Hidden, 12., 1),
        (true, LayoutVisibility::Collapsed, 12., 2),
        (true, LayoutVisibility::Visible, 6., 2),
    ];
    for (frame, (visible, visibility, radius, updated)) in states.into_iter().enumerate() {
        document.nodes[2].visible = visible;
        document.nodes[2].layout_visibility = Some(visibility);
        document.nodes[2].clip_radius = Some(radius);
        let (actual, paragraphs) = runtime.layout(&document, frame as u64 + 2, &fonts);
        let stats = runtime.geometry_update_stats();
        assert!(stats.final_base_reused, "frame {frame}");
        assert_eq!(stats.nodes_updated, updated, "frame {frame}");
        assert!(Arc::ptr_eq(&unrelated_paragraph, &paragraphs["unrelated"]));
        let label = actual
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "label")
            .unwrap();
        assert_eq!(
            label.effective_clips.last().map(|clip| clip.radius),
            Some(radius),
            "frame {frame}"
        );

        let mut fresh = AuiC2Runtime::default();
        let (expected, _) = fresh.layout(&document, frame as u64 + 2, &fonts);
        assert_eq!(actual, expected, "frame {frame}");
        for (x, y) in [(1., 1.), (55., 1.), (85., 1.)] {
            assert_eq!(
                AuiInteractionSystem::hit_test(&document, &actual, x, y),
                AuiInteractionSystem::hit_test(&document, &expected, x, y),
                "frame {frame} at ({x}, {y})"
            );
        }
    }
}

#[test]
fn b11_nested_clip_reset_matches_fresh_runtime_after_local_refresh() {
    let fonts = renderable_font_registry("人");
    let mut document = c2_document("人");
    document.nodes[0].layout.as_mut().unwrap().mode = LayoutMode::Anchor;
    document.nodes[0].clip_policy = AuiClipPolicy::Rect;
    document.nodes[0].children = vec!["outside-clip".into()];
    document.nodes[1].parent = Some("reset-clip".into());
    let mut outside = AuiNode::new(
        "outside-clip",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(200., 0., 50., 40.),
    );
    outside.parent = Some("root".into());
    outside.children = vec!["reset-clip".into()];
    outside.layout = Some(LayoutSpec {
        mode: LayoutMode::Anchor,
        ..Default::default()
    });
    outside.clip_policy = AuiClipPolicy::Rect;
    let mut reset = AuiNode::new(
        "reset-clip",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0., 0., 40., 30.),
    );
    reset.parent = Some("outside-clip".into());
    reset.children = vec!["label".into()];
    reset.layout = Some(LayoutSpec {
        mode: LayoutMode::Anchor,
        ..Default::default()
    });
    reset.clip_policy = AuiClipPolicy::Rounded;
    reset.clip_radius = Some(4.);
    document.nodes.push(outside);
    document.nodes.push(reset);

    let mut runtime = AuiC2Runtime::default();
    let (first, _) = runtime.layout(&document, 1, &fonts);
    let first_label = first
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "label")
        .unwrap();
    assert!(!first_label.effective_visible);
    assert!(first_label.effective_clip_rect.is_some());

    document.nodes[3].clip_radius = Some(12.);
    let (actual, _) = runtime.layout(&document, 2, &fonts);
    assert!(runtime.geometry_update_stats().final_base_reused);
    let mut fresh = AuiC2Runtime::default();
    let (expected, _) = fresh.layout(&document, 2, &fonts);
    assert_eq!(actual, expected);
    let label = actual
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "label")
        .unwrap();
    assert_eq!(
        label.effective_clips.last().map(|clip| clip.radius),
        Some(12.)
    );
    assert!(label.effective_clip_rect.is_some());
}

#[test]
fn gate_e_visibility_reindex_reuses_unrelated_glyph_raster_products() {
    use engine_runtime::aui_control_feedback::AuiControlFeedbackState;
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人马骑兵");
    let mut document = c2_document("人");
    document.nodes[0].layout.as_mut().unwrap().mode = LayoutMode::Overlay;
    document.nodes[1].visible = false;
    document.nodes[1].slot.as_mut().unwrap().participation = Participation::OutOfFlow;
    document.nodes[1].slot.as_mut().unwrap().height = Dimension::Fixed { value: 24. };
    for i in 0..80 {
        let mut sibling = document.nodes[1].clone();
        sibling.node_id = format!("sibling-{i}");
        sibling.visible = true;
        sibling.text = Some("骑兵".into());
        if i == 0 {
            sibling.kind = AuiNodeKind::Button;
        }
        document.nodes[0].children.push(sibling.node_id.clone());
        document.nodes.push(sibling);
    }
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let label = instance.node("label").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let old_frame = retained_present(&document, &fonts, &mut runtime);
    assert_eq!(old_frame.report.rendered_glyph_count, 160);
    let old_overlay = old_frame.overlay.clone();
    let mut feedback = AuiControlFeedbackState::default();
    let mut interaction = AuiInteractionResult::default();
    interaction.control_snapshot.pressed_node = Some("sibling-0".into());
    interaction.control_snapshot.pressed_inside = true;
    let atlases = engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("retained");
    let advance = |output: &mut AuiRuntimePresentOutput,
                   state: &mut AuiControlFeedbackState,
                   interaction: &AuiInteractionResult| {
        AuiRuntimePresenter::apply_control_feedback_with_fonts(
            output,
            interaction,
            state,
            8_000,
            &atlases,
            &fonts,
        )
    };
    advance(&mut old_frame.clone(), &mut feedback, &interaction);
    for visible in [true, false, true] {
        let before = runtime.drawing_stats();
        let mut edit = instance.edit();
        edit.write(
            label,
            UiProperty::Visibility,
            UiValue::Visibility(if visible {
                UiVisibility::Visible
            } else {
                UiVisibility::Hidden
            }),
        )
        .unwrap();
        edit.commit().unwrap();
        runtime.observe_retained(Some(&instance));
        let mut output = retained_present(&document, &fonts, &mut runtime);
        let mut fresh = AuiC2Runtime::default();
        fresh.observe_retained(Some(&instance));
        let mut expected = retained_present(&document, &fonts, &mut fresh);
        assert_eq!(output.overlay, expected.overlay);
        assert_eq!(output.composition, expected.composition);
        assert_eq!(output.overlay.glyph_plan, expected.overlay.glyph_plan);
        assert_eq!(
            old_frame.report.rendered_glyph_count, 160,
            "old frame stays immutable"
        );
        let delta = runtime.drawing_stats().since(before);
        assert_eq!(delta.nodes_built, u64::from(visible));
        assert!(
            delta.text_items_built <= 1,
            "visibility changed one text but rerasterized unrelated siblings: {delta:?}"
        );
        assert_eq!(delta.text_items_reused, 80);
        let before = feedback.drawing_stats();
        let mut expected_feedback = feedback.clone();
        interaction.control_snapshot.frame_id += 1;
        advance(&mut output, &mut feedback, &interaction);
        advance(&mut expected, &mut expected_feedback, &interaction);
        assert_eq!(
            output.overlay, expected.overlay,
            "feedback matches fresh base with the same motion state"
        );
        assert_eq!(output.composition, expected.composition);
        let delta = feedback.drawing_stats().since(before);
        assert!(
            delta.text_items_built <= 2,
            "feedback rerasterized unrelated siblings: {delta:?}"
        );
        assert_eq!(
            old_frame.overlay, old_overlay,
            "retained old glyph IDs/quads stay immutable"
        );
    }
}

#[test]
fn b8_presenter_and_feedback_reuse_unrelated_draw_and_glyph_fragments() {
    use engine_runtime::aui_control_feedback::AuiControlFeedbackState;
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人马骑兵");
    let mut document = c2_document("人马");
    document.nodes[0].layout.as_mut().unwrap().mode = LayoutMode::Overlay;
    document.nodes[1].kind = AuiNodeKind::Button;
    document.nodes[1].slot.as_mut().unwrap().height = Dimension::Fixed { value: 60. };
    let mut sibling = document.nodes[1].clone();
    sibling.node_id = "sibling".into();
    sibling.kind = AuiNodeKind::Text;
    sibling.text = Some("骑兵".into());
    document.nodes[0].children.push("sibling".into());
    document.nodes.push(sibling);
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let label = instance.node("label").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let first = retained_present(&document, &fonts, &mut runtime);
    assert!(first.report.rendered_glyph_count > 0);
    let before = runtime.drawing_stats();
    let mut edit = instance.edit();
    edit.write(
        label,
        UiProperty::Translation,
        UiValue::Translation([5., 0.]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let mut moved = retained_present(&document, &fonts, &mut runtime);
    let after = runtime.drawing_stats();
    assert_eq!(after.index_rebuilds, before.index_rebuilds);
    assert_eq!(after.index_entries_built, before.index_entries_built);
    assert!(Arc::ptr_eq(
        &moved.overlay.draw_items,
        &moved.composition.stages[0].draw_items
    ));
    assert_eq!(after.nodes_built - before.nodes_built, 1);
    assert_eq!(after.nodes_reused - before.nodes_reused, 2);
    assert_eq!(after.text_items_built - before.text_items_built, 1);
    assert_eq!(after.text_items_reused - before.text_items_reused, 1);
    let mut fresh = AuiC2Runtime::default();
    fresh.observe_retained(Some(&instance));
    assert_eq!(
        moved.overlay,
        retained_present(&document, &fonts, &mut fresh).overlay
    );

    let atlases = engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("b8");
    let mut feedback = AuiControlFeedbackState::default();
    let mut interaction = AuiInteractionResult::default();
    let advance = |output: &mut AuiRuntimePresentOutput,
                   interaction: &AuiInteractionResult,
                   state: &mut AuiControlFeedbackState| {
        AuiRuntimePresenter::apply_control_feedback_with_fonts(
            output,
            interaction,
            state,
            8_000,
            &atlases,
            &fonts,
        )
    };
    advance(&mut moved, &interaction, &mut feedback);
    interaction.control_snapshot.pressed_node = Some("label".into());
    interaction.control_snapshot.pressed_inside = true;
    interaction.control_snapshot.frame_id = 1;
    advance(&mut moved, &interaction, &mut feedback);
    let style_rebuilds = feedback.style_rebuild_count();
    let before = feedback.drawing_stats();
    interaction.control_snapshot.frame_id += 1;
    let result = advance(&mut moved, &interaction, &mut feedback);
    assert!(result.report.presentation_rebuilt);
    assert_eq!(feedback.style_rebuild_count(), style_rebuilds);
    let after = feedback.drawing_stats();
    assert_eq!(after.index_rebuilds, before.index_rebuilds);
    assert_eq!(after.index_entries_built, before.index_entries_built);
    assert!(Arc::ptr_eq(
        &moved.overlay.draw_items,
        &moved.composition.stages[0].draw_items
    ));
    assert_eq!(after.nodes_built - before.nodes_built, 1);
    assert_eq!(after.nodes_reused - before.nodes_reused, 2);
    assert_eq!(after.text_items_built - before.text_items_built, 1);
    assert_eq!(after.text_items_reused - before.text_items_reused, 1);
    assert_eq!(result.report.document_nodes_scanned, 0);
    interaction.control_snapshot.pressed_node = None;
    interaction.control_snapshot.pressed_inside = false;
    interaction.control_snapshot.frame_id += 1;
    advance(&mut moved, &interaction, &mut feedback);
    assert_eq!(
        feedback.style_rebuild_count(),
        style_rebuilds,
        "returning to cached Normal style must not rebuild it"
    );
    for _ in 0..40 {
        interaction.control_snapshot.frame_id += 1;
        advance(&mut moved, &interaction, &mut feedback);
    }
    let before = feedback.drawing_stats();
    interaction.control_snapshot.frame_id += 1;
    assert!(
        advance(&mut moved, &interaction, &mut feedback)
            .report
            .presentation_cache_hit
    );
    assert_eq!(feedback.drawing_stats(), before);
    feedback.reset();
    assert_eq!(feedback.drawing_stats(), Default::default());
    assert_eq!(feedback.style_rebuild_count(), 0);
}

#[test]
fn real_font_multiline_preserves_clusters_breaks_and_line_height() {
    let fonts = font_registry();
    let spec = TextLayoutSpec {
        wrap: TextWrap::GraphemeFallback,
        line_height: LineHeight::Multiple { value: 1.5 },
        ..Default::default()
    };
    let text = "ffi a\u{301} 中文\n第二行内容";
    let mut cache = AuiParagraphCache::default();
    let single = layout_paragraph(&mut cache, request(text, &spec, f32::INFINITY, &fonts)).unwrap();
    let wrapped = layout_paragraph(
        &mut cache,
        request(text, &spec, single.width * 0.45, &fonts),
    )
    .unwrap();

    assert!(wrapped.lines.len() >= 3, "{:#?}", wrapped.lines);
    assert!(wrapped.height > wrapped.lines.len() as f32 * 20.0);
    assert!(wrapped
        .glyphs
        .iter()
        .all(|glyph| glyph.cluster_end >= glyph.cluster_start));
    assert!(wrapped
        .glyphs
        .iter()
        .any(|glyph| glyph.cluster_end - glyph.cluster_start > 1));
    assert!(wrapped
        .lines
        .windows(2)
        .all(|lines| lines[0].source_end <= lines[1].source_start));
    assert!(wrapped.first_baseline.is_some());
}

#[test]
fn max_lines_and_ellipsis_share_the_final_paragraph_product() {
    let fonts = font_registry();
    let spec = TextLayoutSpec {
        wrap: TextWrap::GraphemeFallback,
        max_lines: Some(2),
        overflow: TextOverflow::Ellipsis,
        ..Default::default()
    };
    let mut cache = AuiParagraphCache::default();
    let layout = layout_paragraph(
        &mut cache,
        request("一二三四五六七八九十甲乙丙丁戊己庚辛", &spec, 45.0, &fonts),
    )
    .unwrap();
    assert_eq!(layout.lines.len(), 2);
    assert!(layout.truncated);
    let last = layout.lines.last().unwrap();
    assert!(last.width <= 45.01);
    assert!(layout.glyphs[last.glyph_start..last.glyph_end]
        .iter()
        .any(|glyph| glyph.cluster_start == glyph.cluster_end));

    let reused = layout_paragraph(
        &mut cache,
        request("一二三四五六七八九十甲乙丙丁戊己庚辛", &spec, 45.0, &fonts),
    )
    .unwrap();
    assert!(Arc::ptr_eq(&layout, &reused));
    assert_eq!(cache.stats().reshapes, 1);
    assert_eq!(cache.stats().hits, 1);
}

#[test]
fn cache_enforces_entry_byte_and_oversized_item_limits() {
    let fonts = font_registry();
    let spec = TextLayoutSpec::default();
    let mut entries = AuiParagraphCache::new(1024 * 1024, 2);
    for text in ["first", "second", "third"] {
        layout_paragraph(&mut entries, request(text, &spec, 200.0, &fonts)).unwrap();
    }
    assert_eq!(entries.len(), 2);
    assert_eq!(entries.stats().evictions, 1);
    assert!(entries.resident_bytes() <= 1024 * 1024);

    let mut tiny = AuiParagraphCache::new(1, 4096);
    layout_paragraph(&mut tiny, request("oversized", &spec, 200.0, &fonts)).unwrap();
    assert!(tiny.is_empty());
    assert_eq!(tiny.stats().oversized_not_cached, 1);
}

#[test]
fn intrinsic_measure_and_final_layout_reuse_one_identity() {
    let fonts = font_registry();
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Column,
        ..Default::default()
    });
    root.children = vec!["label".into()];
    let mut label = AuiNode::new(
        "label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(0.0, 0.0, 80.0, 20.0),
    );
    label.parent = Some("root".into());
    label.text = Some("复用同一个段落产物".into());
    label.text_layout = Some(TextLayoutSpec {
        wrap: TextWrap::GraphemeFallback,
        ..Default::default()
    });
    label.style = Some(AuiStyle::text("#fff", 20.0));
    let mut document = AuiDocument::new(
        "paragraph",
        vec![AuiCanvas::screen_overlay("main", 200.0, 100.0, "root")],
        vec![root, label],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    let mut cache = AuiParagraphCache::default();
    let mut context = AuiParagraphContext::new(&document, &fonts, &mut cache);
    let measured = context.measure(AuiMeasureInput {
        node_id: "label",
        text: Some("复用同一个段落产物"),
        known_width: Some(80.0),
        known_height: None,
        available_width: AuiAvailableSpace::Definite(80.0),
        available_height: AuiAvailableSpace::MaxContent,
    });
    let measured_layout = context.frame_layouts()["label"].clone();
    let final_layout = context.layout_node("label", 80.0, None).unwrap();
    assert_eq!(measured.height, final_layout.height);
    assert!(Arc::ptr_eq(&measured_layout, &final_layout));
    assert_eq!(final_layout.identity, measured_layout.identity);
}

#[test]
fn paragraph_limits_are_frozen() {
    assert_eq!(DEFAULT_PARAGRAPH_CACHE_BYTES, 32 * 1024 * 1024);
    assert_eq!(DEFAULT_PARAGRAPH_CACHE_ENTRIES, 4096);
    assert_eq!(MAX_PARAGRAPH_SOURCE_BYTES, 64 * 1024);
    assert_eq!(MAX_PARAGRAPH_LINES, 4096);
}

#[test]
fn justify_fills_nonfinal_lines_and_language_direction_change_cache_identity() {
    let fonts = font_registry();
    let text = "甲 乙 丙 丁 戊 己";
    let base = TextLayoutSpec {
        wrap: TextWrap::Word,
        align: TextAlign::Justify,
        language: Some("zh-CN".into()),
        direction: TextDirection::Ltr,
        ..Default::default()
    };
    let mut cache = AuiParagraphCache::default();
    let layout = layout_paragraph(&mut cache, request(text, &base, 70.0, &fonts)).unwrap();
    assert!(layout.lines.len() >= 2);
    assert!((layout.lines[0].width - 70.0).abs() < 0.01);

    let mut changed = base.clone();
    changed.language = Some("zh-Hant".into());
    changed.direction = TextDirection::Auto;
    let changed = layout_paragraph(&mut cache, request(text, &changed, 70.0, &fonts)).unwrap();
    assert_ne!(layout.identity, changed.identity);
    assert_eq!(cache.stats().reshapes, 2);
}

fn c2_document(text: &str) -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Column,
        ..Default::default()
    });
    root.children = vec!["label".into()];
    let mut label = AuiNode::new(
        "label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(0.0, 0.0, 60.0, 20.0),
    );
    label.parent = Some("root".into());
    label.interactable = true;
    label.text = Some(text.into());
    label.style = Some(AuiStyle::text("#fff", 20.0));
    label.slot = Some(SlotSpec {
        width: Dimension::Fixed { value: 60.0 },
        height: Dimension::Content,
        ..Default::default()
    });
    label.text_layout = Some(TextLayoutSpec {
        wrap: TextWrap::GraphemeFallback,
        line_height: LineHeight::Fixed { value: 24.0 },
        max_lines: Some(3),
        overflow: TextOverflow::Ellipsis,
        ..Default::default()
    });
    let mut document = AuiDocument::new(
        "c2-present",
        vec![AuiCanvas::screen_overlay("main", 160.0, 120.0, "root")],
        vec![root, label],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

#[test]
fn retained_real_font_measure_uses_committed_text_not_authoring_text() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::UiTextRef;
    let fonts = font_registry();
    let source = Arc::new(c2_document("人"));
    let mut instance = AuiInstance::mount(source.clone()).unwrap();
    let text = UiTextRef::try_new(&instance, instance.node("label").unwrap()).unwrap();
    let before = instance
        .layout(
            "main",
            &mut AuiParagraphContext::new(&source, &fonts, &mut AuiParagraphCache::default()),
        )
        .unwrap()
        .node("label")
        .unwrap()
        .rect;
    let mut edit = instance.edit();
    text.set_text(&mut edit, "人马合成为骑兵守卫城池").unwrap();
    edit.commit().unwrap();
    let after = instance
        .layout(
            "main",
            &mut AuiParagraphContext::new(&source, &fonts, &mut AuiParagraphCache::default()),
        )
        .unwrap()
        .node("label")
        .unwrap()
        .rect;
    assert!(
        after.height > before.height,
        "new text must wrap: {before:?} -> {after:?}"
    );
    assert_eq!(source.nodes[1].text.as_deref(), Some("人"));
}

fn retained_present(
    document: &AuiDocument,
    fonts: &RuntimeFontBundleRegistry,
    runtime: &mut AuiC2Runtime,
) -> AuiRuntimePresentOutput {
    AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
        document,
        ProjectUiStateSnapshotOutput::new(
            "retained",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(1),
        ),
        &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("retained"),
        fonts,
        None,
        runtime,
    )
}

#[test]
fn b13_presenter_publishes_shared_layout_and_paragraph_map() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiWrite};

    let fonts = renderable_font_registry("人");
    let document = c2_document("人");
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let label = instance.node("label").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let first = retained_present(&document, &fonts, &mut runtime);

    assert!(Arc::ptr_eq(
        &first.layout,
        runtime.current_layout().expect("published C2 layout")
    ));
    let paragraphs = first.paragraph_layouts.clone();

    let mut edit = instance.edit();
    edit.write(
        label,
        UiProperty::Translation,
        UiValue::Translation([12.0, 4.0]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let moved = retained_present(&document, &fonts, &mut runtime);

    // This direct cached-Presenter entry has no scheduler proof that snapshot/viewport inputs
    // were reused, so it keeps the conservative source comparison. Player cache tests cover the
    // production translation path's zero-comparison contract.
    assert_eq!(
        runtime.geometry_update_stats().source_nodes_compared,
        document.nodes.len()
    );
    assert!(Arc::ptr_eq(
        &moved.layout,
        runtime.current_layout().expect("updated C2 layout")
    ));
    assert!(Arc::ptr_eq(&paragraphs, &moved.paragraph_layouts));
}

#[test]
fn retained_observers_read_one_changed_property_without_draining_each_other() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiWrite};

    let fonts = renderable_font_registry("人");
    let mut document = c2_document("人");
    for index in 0..100 {
        let id = format!("fixed-{index}");
        document.nodes[0].children.push(id.clone());
        let mut node = AuiNode::new(
            id,
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0., 0., 10., 10.),
        );
        node.parent = Some("root".into());
        node.slot = Some(SlotSpec {
            width: Dimension::Fixed { value: 10. },
            height: Dimension::Fixed { value: 10. },
            ..Default::default()
        });
        document.nodes.push(node);
    }
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let handles = (0..100)
        .map(|index| instance.node(&format!("fixed-{index}")).unwrap())
        .collect::<Vec<_>>();
    let mut edit = instance.edit();
    for &handle in &handles {
        edit.write(
            handle,
            UiProperty::Translation,
            UiValue::Translation([1., 0.]),
        )
        .unwrap();
    }
    edit.commit().unwrap();

    let mut first = AuiC2Runtime::default();
    let mut second = AuiC2Runtime::default();
    assert!(first.observe_retained(Some(&instance)));
    assert_eq!(first.retained_properties_read(), 100);
    assert!(second.observe_retained(Some(&instance)));
    retained_present(&document, &fonts, &mut first);
    retained_present(&document, &fonts, &mut second);

    let mut edit = instance.edit();
    edit.write(
        handles[42],
        UiProperty::Translation,
        UiValue::Translation([9., 0.]),
    )
    .unwrap();
    edit.commit().unwrap();
    assert!(first.observe_retained(Some(&instance)));
    assert_eq!(first.retained_properties_read(), 1);
    assert!(second.observe_retained(Some(&instance)));
    assert_eq!(second.retained_properties_read(), 1);
    let first_present = retained_present(&document, &fonts, &mut first);
    let second_present = retained_present(&document, &fonts, &mut second);
    assert_eq!(first_present.layout, second_present.layout);
    assert_eq!(first_present.overlay, second_present.overlay);
}

#[test]
fn retained_observer_recovers_from_bounded_change_history() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiWrite};

    let mut document = c2_document("人");
    for index in 0..100 {
        let id = format!("fixed-{index}");
        document.nodes[0].children.push(id.clone());
        let mut node = AuiNode::new(
            id,
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0., 0., 10., 10.),
        );
        node.parent = Some("root".into());
        node.slot = Some(SlotSpec {
            width: Dimension::Fixed { value: 10. },
            height: Dimension::Fixed { value: 10. },
            ..Default::default()
        });
        document.nodes.push(node);
    }
    let mut instance = AuiInstance::mount(Arc::new(document)).unwrap();
    let handles = (0..100)
        .map(|index| instance.node(&format!("fixed-{index}")).unwrap())
        .collect::<Vec<_>>();
    let mut edit = instance.edit();
    for &handle in &handles {
        edit.write(
            handle,
            UiProperty::Translation,
            UiValue::Translation([1., 0.]),
        )
        .unwrap();
    }
    edit.commit().unwrap();
    let mut observer = AuiC2Runtime::default();
    assert!(observer.observe_retained(Some(&instance)));
    for index in 0..9000 {
        let mut edit = instance.edit();
        edit.write(
            handles[42],
            UiProperty::Translation,
            UiValue::Translation([index as f32 + 2., 0.]),
        )
        .unwrap();
        edit.commit().unwrap();
    }
    assert!(observer.observe_retained(Some(&instance)));
    assert_eq!(observer.retained_properties_read(), 100);
    assert!(!observer.observe_retained(Some(&instance)));
    assert_eq!(observer.retained_properties_read(), 0);
}

#[test]
fn retained_active_false_stays_hidden_after_visibility_write() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiVisibility, UiWrite};

    let fonts = renderable_font_registry("人");
    let document = c2_document("人");
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let label = instance.node("label").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    retained_present(&document, &fonts, &mut runtime);

    let mut edit = instance.edit();
    edit.write(
        label,
        UiProperty::Visibility,
        UiValue::Visibility(UiVisibility::Visible),
    )
    .unwrap();
    edit.write(label, UiProperty::Active, UiValue::Bool(false))
        .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let hidden = retained_present(&document, &fonts, &mut runtime);
    assert!(!hidden.layout.computed_nodes[1].effective_visible);
    let mut fresh = AuiC2Runtime::default();
    fresh.observe_retained(Some(&instance));
    assert_eq!(
        hidden.layout,
        retained_present(&document, &fonts, &mut fresh).layout
    );

    let mut edit = instance.edit();
    edit.write(label, UiProperty::Active, UiValue::Bool(true))
        .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    assert!(
        retained_present(&document, &fonts, &mut runtime)
            .layout
            .computed_nodes[1]
            .effective_visible
    );
}

#[test]
fn b14_retained_c2_publishes_solver_snapshots_without_cloning_geometry() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiWrite};

    let fonts = renderable_font_registry("人");
    let mut document = c2_document("人");
    for index in 0..1000 {
        let id = format!("fixed-{index}");
        document.nodes[0].children.push(id.clone());
        let mut node = AuiNode::new(
            id,
            AuiNodeKind::Panel,
            AuiRect::fixed_position(0., 0., 10., 10.),
        );
        node.parent = Some("root".into());
        node.slot = Some(SlotSpec {
            width: Dimension::Fixed { value: 10. },
            height: Dimension::Fixed { value: 10. },
            ..Default::default()
        });
        document.nodes.push(node);
    }
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let label = instance.node("label").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    retained_present(&document, &fonts, &mut runtime);

    let mut edit = instance.edit();
    edit.write(
        label,
        UiProperty::Visibility,
        UiValue::Visibility(project_runtime_sdk::aui::UiVisibility::Hidden),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let actual = retained_present(&document, &fonts, &mut runtime);

    assert_eq!(runtime.geometry_update_stats().snapshot_nodes_copied, 0);
    let mut fresh = AuiC2Runtime::default();
    fresh.observe_retained(Some(&instance));
    let expected = retained_present(&document, &fonts, &mut fresh);
    assert_eq!(actual.layout, expected.layout);
    assert_eq!(actual.overlay, expected.overlay);
}

#[test]
fn retained_present_connects_text_translation_clip_hit_feedback_and_disposal() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人马合成为骑兵守卫城池");
    let mut document = c2_document("人");
    document.nodes[0].clip_policy = AuiClipPolicy::Rounded;
    document.nodes[0].clip_radius = Some(4.0);
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let mut runtime = AuiC2Runtime::default();
    assert!(runtime.observe_retained(Some(&instance)));
    let first = retained_present(&document, &fonts, &mut runtime);
    let old_rect = first.layout.computed_nodes[1].rect;
    let root = instance.node("root").unwrap();
    let label = instance.node("label").unwrap();
    let mut edit = instance.edit();
    edit.write(
        label,
        UiProperty::Text,
        UiValue::Text("人马合成为骑兵守卫城池".into()),
    )
    .unwrap();
    edit.write(
        root,
        UiProperty::Translation,
        UiValue::Translation([30., 20.]),
    )
    .unwrap();
    edit.commit().unwrap();
    assert!(runtime.observe_retained(Some(&instance)));
    let mut moved = retained_present(&document, &fonts, &mut runtime);
    assert_eq!(moved.report.status, AuiRuntimePresentStatus::Success);
    let rect = moved.layout.computed_nodes[1].rect;
    assert_eq!((rect.x, rect.y), (old_rect.x + 30., old_rect.y + 20.));
    assert!(rect.height > old_rect.height);
    let clip = &moved.layout.computed_nodes[1].effective_clips[0];
    assert_eq!((clip.rect.x, clip.rect.y, clip.radius), (30., 20., 4.));
    assert_eq!(
        AuiInteractionSystem::hit_test(
            &moved.resolved_document,
            &moved.layout,
            rect.x + 10.,
            rect.y + 10.
        )
        .hit_node
        .as_deref(),
        Some("label")
    );
    assert_ne!(
        AuiInteractionSystem::hit_test(
            &moved.resolved_document,
            &moved.layout,
            old_rect.x + 10.,
            old_rect.y + 10.
        )
        .hit_node
        .as_deref(),
        Some("label")
    );
    let paragraph = moved.paragraph_layouts["label"].clone();
    let glyphs = moved.overlay.glyph_plan.clone();
    AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation(
        &mut moved,
        &AuiInteractionResult::default(),
        &mut Default::default(),
        16_667,
        &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("retained"),
        &fonts,
        None,
    );
    assert_eq!(
        moved.overlay.glyph_plan, glyphs,
        "feedback must preserve multiline shaped glyphs"
    );
    assert!(Arc::ptr_eq(&paragraph, &moved.paragraph_layouts["label"]));
    assert!(!runtime.observe_retained(Some(&instance)));
    let mut edit = instance.edit();
    edit.write(
        root,
        UiProperty::Translation,
        UiValue::Translation([40., 25.]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let reshapes = runtime.paragraph_cache_stats().reshapes;
    let translated = retained_present(&document, &fonts, &mut runtime);
    assert_eq!(runtime.layout_stats("main").unwrap().measure_calls, 0);
    assert_eq!(runtime.layout_stats("main").unwrap().geometry_nodes, 0);
    assert_eq!(runtime.paragraph_cache_stats().reshapes, reshapes);
    assert!(Arc::ptr_eq(
        &paragraph,
        &translated.paragraph_layouts["label"]
    ));
    instance.dispose();
    assert!(runtime.observe_retained(Some(&instance)));
    let restored = retained_present(&document, &fonts, &mut runtime);
    assert_eq!(restored.layout.computed_nodes[1].rect, old_rect);
    assert_eq!(
        restored.resolved_document.nodes[1].text.as_deref(),
        Some("人")
    );
}

#[test]
fn retained_present_recomputes_clipped_visibility_and_interactable_after_translation() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人");
    let mut document = c2_document("人");
    document.nodes[0].clip_policy = AuiClipPolicy::Rect;
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let mut runtime = AuiC2Runtime::default();
    let label = instance.node("label").unwrap();
    for (x, interactable, visible) in [(200., true, false), (130., true, true), (0., false, true)] {
        let mut edit = instance.edit();
        edit.write(
            label,
            UiProperty::Translation,
            UiValue::Translation([x, 0.]),
        )
        .unwrap();
        edit.write(label, UiProperty::Interactable, UiValue::Bool(interactable))
            .unwrap();
        edit.commit().unwrap();
        runtime.observe_retained(Some(&instance));
        let present = retained_present(&document, &fonts, &mut runtime);
        let computed = &present.layout.computed_nodes[1];
        assert_eq!(computed.effective_visible, visible);
        let hit = AuiInteractionSystem::hit_test(
            &present.resolved_document,
            &present.layout,
            x + 10.,
            10.,
        );
        assert_eq!(hit.hit_node.as_deref() == Some("label"), visible);
        if visible {
            assert_eq!(
                hit.reason,
                if interactable {
                    AuiHitTestReason::HitInteractable
                } else {
                    AuiHitTestReason::HitNonInteractable
                }
            );
        }
        if x == 130. {
            assert_eq!(computed.effective_clip_rect.unwrap().width, 30.);
            assert_ne!(
                AuiInteractionSystem::hit_test(
                    &present.resolved_document,
                    &present.layout,
                    170.,
                    10.
                )
                .hit_node
                .as_deref(),
                Some("label")
            );
        }
    }
}

#[test]
fn c2_retained_solver_recovers_from_missing_font_and_updates_style_and_viewport() {
    let mut document = c2_document("人马骑兵");
    let mut runtime = AuiC2Runtime::default();
    let (failed, _) = runtime.layout(&document, 1, &RuntimeFontBundleRegistry::default());
    assert!(!failed.diagnostics.is_empty());
    let fonts = font_registry();
    let (recovered, _) = runtime.layout(&document, 2, &fonts);
    assert!(
        recovered.diagnostics.is_empty(),
        "{:?}",
        recovered.diagnostics
    );
    let old_height = recovered.computed_nodes[1].rect.height;
    let builds = runtime.solver_build_count();
    document.nodes[1].style.as_mut().unwrap().font_size = Some(40.);
    let (larger, _) = runtime.layout(&document, 3, &fonts);
    assert!(larger.diagnostics.is_empty());
    assert!(larger.computed_nodes[1].rect.height > old_height);
    assert_eq!(runtime.layout_stats("main").unwrap().source_nodes_loaded, 0);
    assert_eq!(runtime.solver_build_count(), builds);
    let (fresh, _) = AuiC2Runtime::default().layout(&document, 3, &fonts);
    assert_eq!(larger, fresh);
    document.nodes[1].layout_visibility = Some(LayoutVisibility::Collapsed);
    let (collapsed, _) = runtime.layout(&document, 4, &fonts);
    assert!(!collapsed.computed_nodes[1].effective_visible);
    assert_eq!(runtime.layout_stats("main").unwrap().source_nodes_loaded, 0);
    document.canvases[0].reference_resolution = AuiVec2::new(320., 240.);
    let (resized, _) = runtime.layout(&document, 5, &fonts);
    assert_eq!(resized.computed_nodes[0].rect.width, 320.);
    assert_eq!(runtime.layout_stats("main").unwrap().source_nodes_loaded, 0);
    assert_eq!(runtime.solver_build_count(), builds);
    let (fresh, _) = AuiC2Runtime::default().layout(&document, 5, &fonts);
    assert_eq!(resized, fresh);
}

#[test]
fn retained_translation_revealing_clipped_text_builds_its_final_paragraph() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人马骑兵");
    let mut document = c2_document("人马骑兵");
    document.nodes[0].layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    document.nodes[0].clip_policy = AuiClipPolicy::Rect;
    document.nodes[1].slot = Some(SlotSpec {
        participation: Participation::OutOfFlow,
        ..Default::default()
    });
    document.nodes[1].rect = AuiRect::fixed_position(200., 0., 60., 72.);
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let hidden = retained_present(&document, &fonts, &mut runtime);
    assert!(!hidden.layout.computed_nodes[1].effective_visible);
    let label = instance.node("label").unwrap();
    let mut edit = instance.edit();
    edit.write(
        label,
        UiProperty::Translation,
        UiValue::Translation([-200., 0.]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let revealed = retained_present(&document, &fonts, &mut runtime);
    assert!(revealed.layout.computed_nodes[1].effective_visible);
    assert_eq!(revealed.report.status, AuiRuntimePresentStatus::Success);
    assert!(revealed.paragraph_layouts["label"].lines.len() >= 2);
    assert!(revealed.overlay.glyph_plan.unwrap().rendered_glyph_count > 0);
    assert_eq!(runtime.layout_stats("main").unwrap().measure_calls, 0);
}

#[test]
fn c2_inline_control_labels_and_placeholder_keep_shaping_during_feedback() {
    let text = "人马合成为骑兵";
    let fonts = renderable_font_registry(text);
    for kind in [AuiNodeKind::Button, AuiNodeKind::InputField] {
        let mut document = c2_document(text);
        document.nodes[1].kind = kind;
        document.nodes[1].slot.as_mut().unwrap().height = Dimension::Fixed { value: 72. };
        if kind == AuiNodeKind::InputField {
            document.nodes[1].text = None;
            document.nodes[1].placeholder = Some(text.into());
        }
        let mut runtime = AuiC2Runtime::default();
        let mut present = retained_present(&document, &fonts, &mut runtime);
        assert_eq!(present.report.status, AuiRuntimePresentStatus::Success);
        let paragraph = present.paragraph_layouts["label"].clone();
        assert!(paragraph.lines.len() >= 2);
        let mut interaction = AuiInteractionResult::default();
        interaction.control_snapshot.pressed_node = Some("label".into());
        interaction.control_snapshot.pressed_inside = true;
        AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation(
            &mut present,
            &interaction,
            &mut Default::default(),
            16_667,
            &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("retained"),
            &fonts,
            None,
        );
        assert!(Arc::ptr_eq(&paragraph, &present.paragraph_layouts["label"]));
        let glyphs = present.overlay.glyph_plan.as_ref().unwrap();
        assert!(glyphs.rendered_glyph_count > 0, "{kind:?}");
        assert_eq!(glyphs.font_source_kind, "project_font_bundle_v2_paragraph");
        assert!(
            glyphs
                .quads
                .iter()
                .map(|q| (q.rect.y * 10.) as i32)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= 2
        );
    }
}

#[test]
fn b5_translation_updates_only_affected_geometry_and_reuses_final_text() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人马骑兵");
    let mut document = c2_document("人马骑兵");
    document.nodes[0].layout.as_mut().unwrap().mode = LayoutMode::Overlay;
    document.nodes[0].children = vec!["group".into()];
    document.nodes[0].clip_policy = AuiClipPolicy::Rect;
    document.nodes[1].parent = Some("group".into());
    let mut group = AuiNode::new(
        "group",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0., 0., 100., 100.),
    );
    group.parent = Some("root".into());
    group.children = vec!["label".into()];
    group.clip_policy = AuiClipPolicy::Rounded;
    group.clip_radius = Some(4.);
    group.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    document.nodes.push(group);
    for i in 0..1000 {
        let id = format!("static-{i}");
        let mut node = AuiNode::new(
            &id,
            AuiNodeKind::Panel,
            AuiRect::fixed_position(120., 80., 10., 10.),
        );
        node.parent = Some("root".into());
        document.nodes[0].children.push(id);
        document.nodes.push(node);
    }
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let label = instance.node("label").unwrap();
    let group = instance.node("group").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    let first = retained_present(&document, &fonts, &mut runtime);
    assert_eq!(first.report.status, AuiRuntimePresentStatus::Success);
    let paragraph = first.paragraph_layouts["label"].clone();
    let shape_stats = runtime.paragraph_cache_stats();
    // Leaf, ancestor + leaf in one commit, complete clipping, restoration and repeated animation.
    for (frame, (gx, lx, count)) in [
        (0., 10., 1),
        (20., 15., 2),
        (200., 15., 2),
        (0., 0., 2),
        (0., 5., 1),
        (0., 0., 1),
        (0., 0., 0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut edit = instance.edit();
        edit.write(
            group,
            UiProperty::Translation,
            UiValue::Translation([gx, 0.]),
        )
        .unwrap();
        edit.write(
            label,
            UiProperty::Translation,
            UiValue::Translation([lx, 0.]),
        )
        .unwrap();
        edit.commit().unwrap();
        runtime.observe_retained(Some(&instance));
        let present = retained_present(&document, &fonts, &mut runtime);
        assert!(runtime.geometry_update_stats().base_reused);
        assert_eq!(
            runtime.geometry_update_stats().nodes_updated,
            count,
            "frame {frame}"
        );
        assert_eq!(runtime.layout_stats("main").unwrap().geometry_nodes, 0);
        assert_eq!(runtime.layout_stats("main").unwrap().measure_calls, 0);
        assert_eq!(runtime.paragraph_cache_stats(), shape_stats);
        assert!(Arc::ptr_eq(&paragraph, &present.paragraph_layouts["label"]));
        // Rebuild from scratch is the semantic oracle, including clip chains, order and draw/glyphs.
        let mut fresh = AuiC2Runtime::default();
        fresh.observe_retained(Some(&instance));
        let expected = retained_present(&document, &fonts, &mut fresh);
        assert_eq!(present.layout, expected.layout, "frame {frame}");
        assert_eq!(present.overlay, expected.overlay, "frame {frame}");
        for (x, y) in [(gx + lx + 10., 10.), (125., 85.), (170., 10.)] {
            assert_eq!(
                AuiInteractionSystem::hit_test(&present.resolved_document, &present.layout, x, y),
                AuiInteractionSystem::hit_test(&expected.resolved_document, &expected.layout, x, y),
            );
        }
    }
    let mut edit = instance.edit();
    edit.write(
        group,
        UiProperty::Translation,
        UiValue::Translation([30., 0.]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    retained_present(&document, &fonts, &mut runtime);
    instance.dispose();
    runtime.observe_retained(Some(&instance));
    let restored = retained_present(&document, &fonts, &mut runtime);
    assert_eq!(runtime.geometry_update_stats().nodes_updated, 2);
    assert_eq!(restored.layout, first.layout);
}

#[test]
fn b5_geometry_font_identity_invalidates_measurements_and_failed_results() {
    let document = c2_document("人马骑兵");
    let fonts = font_registry();
    let mut runtime = AuiC2Runtime::default();
    let (first, _) = runtime.layout(&document, 1, &fonts);
    assert!(first.diagnostics.is_empty());
    runtime.layout(&document, 2, &fonts);
    assert!(runtime.geometry_update_stats().base_reused);
    // COW replacement with the same metadata generation must not reuse successful geometry.
    let mut broken = fonts.clone();
    Arc::make_mut(&mut broken.bundles_by_id)
        .get_mut("test-font")
        .unwrap()
        .font_face_sources
        .clear();
    let (failed, _) = runtime.layout(&document, 3, &broken);
    assert!(!failed.diagnostics.is_empty());
    assert!(!runtime.geometry_update_stats().base_reused);
    let (failed_again, _) = runtime.layout(&document, 4, &broken);
    assert!(!failed_again.diagnostics.is_empty());
    assert!(!runtime.geometry_update_stats().base_reused);
    let (restored, _) = runtime.layout(&document, 5, &fonts);
    assert!(restored.diagnostics.is_empty());
    assert_eq!(restored.computed_nodes, first.computed_nodes);
    assert!(runtime.layout_stats("main").unwrap().measure_calls > 0);
}

#[test]
fn b5_shared_canvas_root_keeps_translation_and_clip_indices_separate() {
    use engine_runtime::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::*;
    let fonts = renderable_font_registry("人");
    let mut document = c2_document("人");
    document.nodes[0].clip_policy = AuiClipPolicy::Rect;
    document
        .canvases
        .push(AuiCanvas::screen_overlay("second", 320., 240., "root"));
    let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
    let root = instance.node("root").unwrap();
    let mut runtime = AuiC2Runtime::default();
    runtime.observe_retained(Some(&instance));
    retained_present(&document, &fonts, &mut runtime);
    assert_eq!(runtime.layout_source_nodes_copied(), 2);
    assert_eq!(runtime.solver_node_count("main"), Some(2));
    assert_eq!(runtime.solver_node_count("second"), Some(2));
    let mut edit = instance.edit();
    edit.write(
        root,
        UiProperty::Translation,
        UiValue::Translation([30., 10.]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    let moved = retained_present(&document, &fonts, &mut runtime);
    assert!(runtime.geometry_update_stats().base_reused);
    assert_eq!(runtime.geometry_update_stats().nodes_updated, 4);
    for (canvas, width) in [("main", 160.), ("second", 320.)] {
        let node = moved
            .layout
            .computed_nodes
            .iter()
            .find(|n| n.canvas_id == canvas && n.node_id == "label")
            .unwrap();
        assert_eq!((node.rect.x, node.rect.y), (30., 10.));
        assert_eq!(node.effective_clips[0].rect.width, width);
        assert_eq!(node.effective_clips[0].rect.x, 30.);
    }
}

#[test]
fn c2_runtime_uses_solver_geometry_for_draw_glyph_and_hit() {
    let text = "第一行第二行第三行第四行";
    let fonts = renderable_font_registry(text);
    let document = c2_document(text);
    let snapshot = ProjectUiStateSnapshot::new(1);
    let output = ProjectUiStateSnapshotOutput::new(
        "paragraph-test",
        AuiSnapshotSource::TestSnapshot,
        snapshot,
    );
    let mut runtime = AuiC2Runtime::default();
    let present = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
        &document,
        output,
        &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("c2-test"),
        &fonts,
        None,
        &mut runtime,
    );
    assert!(
        present.layout.diagnostics.is_empty(),
        "{:#?}",
        present.layout.diagnostics
    );
    assert_eq!(present.report.status, AuiRuntimePresentStatus::Success);
    let label = present
        .layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "label")
        .unwrap();
    assert!((label.rect.width - 60.0).abs() < 0.01);
    let glyph_plan = present.overlay.glyph_plan.as_ref().unwrap();
    assert!(glyph_plan.rendered_glyph_count > 0);
    assert_eq!(
        glyph_plan.font_source_kind,
        "project_font_bundle_v2_paragraph"
    );
    assert!(glyph_plan
        .quads
        .iter()
        .all(|quad| quad.rect.x >= label.rect.x - 0.01));
    let line_y = glyph_plan
        .quads
        .iter()
        .map(|quad| (quad.rect.y * 10.0).round() as i32)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(line_y.len() >= 2);
    let hit = AuiInteractionSystem::hit_test(
        &present.resolved_document,
        &present.layout,
        label.rect.x + 1.0,
        label.rect.y + 1.0,
    );
    assert_eq!(hit.hit_node.as_deref(), Some("label"));
}

#[test]
fn b3_feedback_settles_to_shared_products_and_refreshes_pressed_then_restored() {
    let text = "人马合成为骑兵";
    let fonts = renderable_font_registry(text);
    let atlases = engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("b3");
    let mut document = c2_document(text);
    document.nodes[1].kind = AuiNodeKind::Button;
    document.nodes[1].slot.as_mut().unwrap().height = Dimension::Fixed { value: 72. };
    let mut runtime = AuiC2Runtime::default();
    let mut present = retained_present(&document, &fonts, &mut runtime);
    let mut state = Default::default();
    let mut interaction = AuiInteractionResult::default();
    let mut advance = |present: &mut engine_runtime::aui::AuiRuntimePresentOutput,
                       pressed: bool| {
        interaction.control_snapshot.frame_id += 1;
        interaction.control_snapshot.pressed_node = pressed.then(|| "label".into());
        interaction.control_snapshot.pressed_inside = pressed;
        AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation(
            present,
            &interaction,
            &mut state,
            16_667,
            &atlases,
            &fonts,
            None,
        )
    };
    for _ in 0..30 {
        advance(&mut present, false);
    }
    let normal_items = present.composition.stages[0].draw_items.clone();
    let normal_glyphs = present.composition.glyph_plan.clone().unwrap();
    for _ in 0..8 {
        let frame = advance(&mut present, false);
        assert!(frame.report.presentation_cache_hit);
        assert!(!frame.report.presentation_rebuilt);
        assert_eq!(frame.report.document_nodes_scanned, 0);
        assert_eq!(frame.report.resolved_node_count, 0);
        assert!(Arc::ptr_eq(
            &normal_items,
            &present.composition.stages[0].draw_items
        ));
        assert!(Arc::ptr_eq(
            &normal_glyphs,
            present.composition.glyph_plan.as_ref().unwrap()
        ));
    }
    for _ in 0..30 {
        advance(&mut present, true);
    }
    let pressed_items = present.composition.stages[0].draw_items.clone();
    assert_ne!(
        &*normal_items, &*pressed_items,
        "feedback must actually change the drawing"
    );
    for _ in 0..8 {
        advance(&mut present, true);
        assert!(Arc::ptr_eq(
            &pressed_items,
            &present.composition.stages[0].draw_items
        ));
    }
    for _ in 0..30 {
        advance(&mut present, false);
    }
    assert_eq!(&*normal_items, &*present.composition.stages[0].draw_items);
    assert_eq!(
        &*normal_glyphs,
        present.composition.glyph_plan.as_ref().unwrap().as_ref()
    );
}

#[test]
fn c2_runtime_reuses_paragraphs_across_rebuilds_and_hides_layout_nodes() {
    let text = "缓存复用文本";
    let fonts = renderable_font_registry(text);
    let mut document = c2_document(text);
    let mut runtime = AuiC2Runtime::default();
    let (first, _) = runtime.layout(&document, 1, &fonts);
    assert!(first.diagnostics.is_empty());
    let first_stats = runtime.paragraph_cache_stats();
    let (second, _) = runtime.layout(&document, 2, &fonts);
    assert!(second.diagnostics.is_empty());
    let second_stats = runtime.paragraph_cache_stats();
    assert_eq!(second_stats.reshapes, first_stats.reshapes);
    assert_eq!(
        second_stats, first_stats,
        "unchanged geometry skips even paragraph cache lookups"
    );
    assert!(runtime.geometry_update_stats().base_reused);
    assert_eq!(runtime.geometry_update_stats().nodes_updated, 0);

    document.nodes[1].layout_visibility = Some(LayoutVisibility::Hidden);
    let (hidden, _) = runtime.layout(&document, 3, &fonts);
    let label = hidden
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "label")
        .unwrap();
    assert!(!label.effective_visible);
    assert_ne!(
        AuiInteractionSystem::hit_test(&document, &hidden, 1.0, 1.0)
            .hit_node
            .as_deref(),
        Some("label")
    );

    document.nodes[1].layout_visibility = Some(LayoutVisibility::Collapsed);
    let (collapsed, _) = runtime.layout(&document, 4, &fonts);
    let label = collapsed
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "label")
        .unwrap();
    assert_eq!(label.rect.width, 0.0);
    assert_eq!(label.rect.height, 0.0);
}

#[test]
fn b4_feedback_cache_invalidates_shared_inputs_resources_scope_and_reset() {
    use engine_runtime::aui_control_feedback::AuiControlFeedbackState;
    use engine_runtime::aui_theme::AuiThemeRegistry;
    use engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry;
    let mut fonts = renderable_font_registry("人马");
    let atlases = RuntimeAuiFontAtlasRegistry::empty("b4");
    let mut runtime = AuiC2Runtime::default();
    let mut present = retained_present(&c2_document("人马"), &fonts, &mut runtime);
    let mut state = AuiControlFeedbackState::default();
    let mut interaction = AuiInteractionResult::default();
    // Non-neutral styling exercises invalidation; neutral feedback reuses the base.
    interaction.control_snapshot.hovered_node = Some("label".into());
    let themes = AuiThemeRegistry::builtin();
    let advance = |p: &mut AuiRuntimePresentOutput,
                   s: &mut AuiControlFeedbackState,
                   i: &AuiInteractionResult,
                   f: &RuntimeFontBundleRegistry| {
        AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
            p, i, s, 250_000, &atlases, f, &themes, None,
        )
    };
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_cache_hit
    );
    // A public COW mutation must never masquerade as the previously cached output.
    Arc::make_mut(&mut present.overlay.draw_items).clear();
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    assert!(!present.overlay.draw_items.is_empty());
    Arc::make_mut(&mut present.layout).computed_nodes[1].rect.x += 10.;
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    Arc::make_mut(&mut present.resolved_document).nodes[1]
        .style
        .as_mut()
        .unwrap()
        .text_color = Some("#ff0000".into());
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    let old_generation = fonts.default_bundle().unwrap().metadata.generation;
    Arc::make_mut(&mut fonts.bundles_by_id)
        .get_mut("test-font")
        .unwrap()
        .metadata
        .glyphs
        .clear();
    assert_eq!(
        fonts.default_bundle().unwrap().metadata.generation,
        old_generation
    );
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_cache_hit
    );
    interaction.control_snapshot.session_id = Some("replacement".into());
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    interaction.control_snapshot.active_screen_id = Some("settings".into());
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    state.reset();
    let reset = advance(&mut present, &mut state, &interaction, &fonts);
    assert!(reset.report.presentation_rebuilt);
    assert_eq!(
        reset.report.document_nodes_scanned,
        present.resolved_document.nodes.len()
    );
    Arc::make_mut(&mut present.paragraph_layouts).clear();
    assert!(
        advance(&mut present, &mut state, &interaction, &fonts)
            .report
            .presentation_rebuilt
    );
    let project_themes =
        AuiThemeRegistry::from_assets(vec![engine_runtime::aui_theme::AuiThemeAsset {
            schema_version: engine_runtime::aui_theme::AUI_THEME_ASSET_SCHEMA_VERSION.into(),
            asset_id: "project-theme".into(),
            extends: None,
            tokens: Default::default(),
            styles: Default::default(),
        }])
        .unwrap();
    let themed = AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
        &mut present,
        &interaction,
        &mut state,
        16_667,
        &atlases,
        &fonts,
        &project_themes,
        None,
    );
    assert!(themed.report.presentation_rebuilt);
    use engine_runtime::game_view_presentation::*;
    let mut mapping = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "b4".into(),
        target_id: "b4".into(),
        target_extent: GameViewExtent::new(800, 600),
        display_rect: GameViewRect::new(0., 0., 800., 600.),
        scale_policy: GameViewScalePolicy::Stretch,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 800, 600)],
    })
    .unwrap();
    for _ in 0..2 {
        let mapped =
            AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
                &mut present,
                &interaction,
                &mut state,
                16_667,
                &atlases,
                &fonts,
                &project_themes,
                Some(&mapping),
            );
        assert!(mapped.report.presentation_rebuilt);
        mapping.target_extent.width *= 2;
    }
}
