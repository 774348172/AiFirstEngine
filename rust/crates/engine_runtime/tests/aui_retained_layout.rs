use engine_runtime::aui::*;
use engine_runtime::aui_instance::AuiInstance;
use engine_runtime::aui_layout_runtime::*;
use engine_runtime::aui_layout_schema::*;
use project_runtime_sdk::aui::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
struct Measure {
    calls: BTreeMap<String, usize>,
    invalid: bool,
}
impl AuiIntrinsicMeasure for Measure {
    fn measure(&mut self, input: AuiMeasureInput<'_>) -> AuiMeasureOutput {
        *self.calls.entry(input.node_id.into()).or_default() += 1;
        AuiMeasureOutput {
            width: if self.invalid {
                f32::NAN
            } else {
                input
                    .known_width
                    .unwrap_or(input.text.unwrap_or("").chars().count() as f32 * 10.)
            },
            height: input.known_height.unwrap_or(20.),
            first_baseline: None,
        }
    }
}

fn document(count: usize) -> Arc<AuiDocument> {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        ..Default::default()
    });
    let mut nodes = Vec::new();
    for i in 0..count {
        let id = format!("text-{i}");
        root.children.push(id.clone());
        let mut node = AuiNode::new(
            id,
            AuiNodeKind::Text,
            AuiRect::fixed_position(0., 0., 20., 20.),
        );
        node.parent = Some("root".into());
        node.text = Some("人".into());
        node.slot = Some(SlotSpec {
            width: Dimension::Content,
            height: Dimension::Fixed { value: 20. },
            flex_shrink: 0.,
            ..Default::default()
        });
        nodes.push(node);
    }
    nodes.insert(0, root);
    let mut doc = AuiDocument::new(
        "retained",
        vec![AuiCanvas::screen_overlay("main", 800., 600., "root")],
        nodes,
    );
    doc.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    Arc::new(doc)
}

fn text(instance: &AuiInstance, id: &str) -> UiTextRef {
    UiTextRef::try_new(instance, instance.node(id).unwrap()).unwrap()
}

#[test]
fn retained_clean_and_same_value_frames_do_no_document_or_geometry_work() {
    let mut instance = AuiInstance::mount(document(1000)).unwrap();
    let mut measure = Measure::default();
    instance.layout("main", &mut measure).unwrap();
    let label = text(&instance, "text-0");
    let ptr = instance.layout_snapshot("main").unwrap().nodes.as_ptr();
    for _ in 0..8 {
        let mut edit = instance.edit();
        label.set_text(&mut edit, "人").unwrap();
        assert_eq!(edit.commit().unwrap(), 0);
        measure.calls.clear();
        instance.layout("main", &mut measure).unwrap();
        assert!(measure.calls.is_empty());
        assert_eq!(
            instance.layout_snapshot("main").unwrap().nodes.as_ptr(),
            ptr
        );
        let stats = instance.layout_stats("main").unwrap();
        assert!(stats.clean_snapshot_reused);
        assert_eq!(
            stats.source_nodes_loaded
                + stats.document_nodes_validated
                + stats.property_nodes_updated
                + stats.geometry_nodes
                + stats.visited_nodes,
            0
        );
    }
}

#[test]
fn retained_text_updates_size_and_sibling_position_without_reloading_source() {
    let source = document(2);
    let mut instance = AuiInstance::mount(Arc::clone(&source)).unwrap();
    let mut measure = Measure::default();
    instance.layout("main", &mut measure).unwrap();
    instance.drain_invalidations(&mut Vec::new());
    let label = text(&instance, "text-0");
    let mut edit = instance.edit();
    label.set_text(&mut edit, "骑兵骑兵").unwrap();
    edit.commit().unwrap();
    let snapshot = instance.layout("main", &mut measure).unwrap();
    assert_eq!(snapshot.node("text-0").unwrap().rect.width, 40.);
    assert_eq!(snapshot.node("text-1").unwrap().rect.x, 40.);
    assert_eq!(source.nodes[1].text.as_deref(), Some("人"));
    let stats = instance.layout_stats("main").unwrap();
    assert_eq!(
        stats.source_nodes_loaded + stats.document_nodes_validated,
        0
    );
    assert_eq!(stats.property_nodes_updated, 1);
    assert_eq!(stats.geometry_nodes, 2); // Changed text and displaced sibling; fixed root is reused.
    let mut dirty = Vec::new();
    instance.drain_invalidations(&mut dirty);
    assert_eq!(dirty.len(), 1);
    assert!(dirty
        .iter()
        .any(|entry| entry.handle == label.node().handle() && entry.invalidation.paint));
}

#[test]
fn retained_translation_and_interactable_do_not_relayout() {
    let mut instance = AuiInstance::mount(document(2)).unwrap();
    let mut measure = Measure::default();
    let before = instance.layout("main", &mut measure).unwrap().clone();
    let label = text(&instance, "text-0");
    let mut edit = instance.edit();
    label.node().set_translation(&mut edit, [50., 30.]).unwrap();
    label.node().set_interactable(&mut edit, false).unwrap();
    edit.commit().unwrap();
    measure.calls.clear();
    assert_eq!(&before, instance.layout("main", &mut measure).unwrap());
    assert!(measure.calls.is_empty());
    assert!(instance.layout_stats("main").unwrap().clean_snapshot_reused);
    assert_eq!(
        instance
            .read(label.node().handle(), UiProperty::Translation)
            .unwrap(),
        UiValue::Translation([50., 30.])
    );
}

#[test]
fn retained_hidden_keeps_space_and_collapsed_reflows_then_restores() {
    let mut instance = AuiInstance::mount(document(2)).unwrap();
    let label = text(&instance, "text-0");
    let mut measure = Measure::default();
    instance.layout("main", &mut measure).unwrap();
    for (visibility, expected_x) in [
        (UiVisibility::Hidden, 10.),
        (UiVisibility::Collapsed, 0.),
        (UiVisibility::Visible, 10.),
    ] {
        let mut edit = instance.edit();
        label.node().set_visibility(&mut edit, visibility).unwrap();
        edit.commit().unwrap();
        measure.calls.clear();
        let snapshot = instance.layout("main", &mut measure).unwrap();
        assert_eq!(snapshot.node("text-1").unwrap().rect.x, expected_x);
        if visibility == UiVisibility::Hidden {
            assert_eq!(
                snapshot.node("text-0").unwrap().visibility,
                LayoutVisibility::Hidden
            );
            assert!(measure.calls.is_empty());
        }
    }
}

#[test]
fn retained_inactive_subtree_collapses_then_restores_its_own_visibility() {
    let mut instance = AuiInstance::mount(document(2)).unwrap();
    let root = instance.node("root").unwrap();
    let hidden = text(&instance, "text-0").node();
    let mut measure = Measure::default();
    instance.layout("main", &mut measure).unwrap();

    let mut edit = instance.edit();
    hidden
        .set_visibility(&mut edit, UiVisibility::Hidden)
        .unwrap();
    edit.commit().unwrap();
    assert_eq!(
        instance
            .layout("main", &mut measure)
            .unwrap()
            .node("text-0")
            .unwrap()
            .visibility,
        LayoutVisibility::Hidden
    );

    let mut edit = instance.edit();
    UiNodeRef::new(root).set_active(&mut edit, false).unwrap();
    edit.commit().unwrap();
    let inactive = instance.layout("main", &mut measure).unwrap();
    assert_eq!(
        inactive.node("root").unwrap().visibility,
        LayoutVisibility::Collapsed
    );
    assert_eq!(
        inactive.node("text-1").unwrap().visibility,
        LayoutVisibility::Collapsed
    );

    let mut edit = instance.edit();
    UiNodeRef::new(root).set_active(&mut edit, true).unwrap();
    edit.commit().unwrap();
    let restored = instance.layout("main", &mut measure).unwrap();
    assert_eq!(
        restored.node("text-0").unwrap().visibility,
        LayoutVisibility::Hidden
    );
    assert_eq!(
        restored.node("text-1").unwrap().visibility,
        LayoutVisibility::Visible
    );
}

#[test]
fn retained_failed_measure_keeps_last_good_geometry_and_can_retry() {
    let mut instance = AuiInstance::mount(document(2)).unwrap();
    let mut measure = Measure::default();
    let good = instance.layout("main", &mut measure).unwrap().clone();
    let label = text(&instance, "text-0");
    let mut edit = instance.edit();
    label.set_text(&mut edit, "骑兵").unwrap();
    edit.commit().unwrap();
    measure.invalid = true;
    assert!(instance
        .layout("main", &mut measure)
        .unwrap_err()
        .iter()
        .any(|e| e.code == "aui_layout_measure_invalid"));
    assert_eq!(instance.layout_snapshot("main"), Some(&good));
    measure.invalid = false;
    assert_eq!(
        instance
            .layout("main", &mut measure)
            .unwrap()
            .node("text-0")
            .unwrap()
            .rect
            .width,
        20.
    );
}

#[test]
fn retained_late_canvas_mount_uses_committed_properties_and_dispose_clears_layout() {
    let mut doc = (*document(2)).clone();
    doc.canvases
        .push(AuiCanvas::screen_overlay("second", 400., 300., "root"));
    let mut instance = AuiInstance::mount(Arc::new(doc)).unwrap();
    let mut other = AuiInstance::mount(document(2)).unwrap();
    let label = text(&instance, "text-0");
    let mut edit = instance.edit();
    label.set_text(&mut edit, "骑兵骑兵").unwrap();
    edit.commit().unwrap();
    let mut measure = Measure::default();
    for canvas in ["main", "second"] {
        assert_eq!(
            instance
                .layout(canvas, &mut measure)
                .unwrap()
                .node("text-0")
                .unwrap()
                .rect
                .width,
            40.
        );
    }
    let mut edit = instance.edit();
    label.set_text(&mut edit, "骑兵").unwrap();
    edit.commit().unwrap();
    for canvas in ["main", "second"] {
        assert_eq!(
            instance
                .layout(canvas, &mut measure)
                .unwrap()
                .node("text-0")
                .unwrap()
                .rect
                .width,
            20.
        );
    }
    assert_eq!(
        other
            .layout("main", &mut measure)
            .unwrap()
            .node("text-0")
            .unwrap()
            .rect
            .width,
        10.
    );
    assert!(instance.layout("missing", &mut measure).is_err());
    instance.dispose();
    assert!(instance.layout_snapshot("main").is_none());
    assert!(instance.layout("main", &mut measure).is_err());
}

#[test]
fn retained_aborted_edits_do_not_invalidate_or_change_geometry() {
    let mut instance = AuiInstance::mount(document(2)).unwrap();
    let mut measure = Measure::default();
    let before = instance.layout("main", &mut measure).unwrap().clone();
    let label = text(&instance, "text-0");
    {
        let mut edit = instance.edit();
        label.set_text(&mut edit, "骑兵").unwrap();
    }
    {
        let mut edit = instance.edit();
        label.set_text(&mut edit, "骑兵").unwrap();
        assert!(label
            .node()
            .set_translation(&mut edit, [f32::NAN, 0.])
            .is_err());
        assert!(edit.commit().is_err());
    }
    assert_eq!(instance.layout("main", &mut measure).unwrap(), &before);
    assert!(instance.layout_stats("main").unwrap().clean_snapshot_reused);
}

#[test]
fn retained_fixed_subtree_reuses_unrelated_measurements_but_content_parent_propagates() {
    for fixed in [true, false] {
        let mut doc = (*document(2)).clone();
        doc.nodes[0].children = vec!["left".into(), "right".into()];
        for (i, id) in ["left", "right"].iter().enumerate() {
            doc.nodes[i + 1].parent = Some((*id).into());
            let mut parent = AuiNode::new(*id, AuiNodeKind::Panel, AuiRect::stretch_full());
            parent.parent = Some("root".into());
            parent.children = vec![format!("text-{i}")];
            parent.layout = Some(LayoutSpec {
                mode: LayoutMode::Row,
                ..Default::default()
            });
            parent.slot = Some(SlotSpec {
                width: if fixed || i == 1 {
                    Dimension::Fixed { value: 100. }
                } else {
                    Dimension::Content
                },
                height: Dimension::Fixed { value: 40. },
                flex_shrink: 0.,
                ..Default::default()
            });
            doc.nodes.push(parent);
        }
        let mut instance = AuiInstance::mount(Arc::new(doc)).unwrap();
        let mut measure = Measure::default();
        instance.layout("main", &mut measure).unwrap();
        let label = text(&instance, "text-0");
        let mut edit = instance.edit();
        label.set_text(&mut edit, "骑兵骑兵").unwrap();
        edit.commit().unwrap();
        measure.calls.clear();
        let snapshot = instance.layout("main", &mut measure).unwrap().clone();
        assert_eq!(
            snapshot.node("right").unwrap().rect.x,
            if fixed { 100. } else { 40. }
        );
        assert!(
            !measure.calls.contains_key("text-1"),
            "unrelated text measured: {:?}",
            measure.calls
        );
        assert!(measure.calls.contains_key("text-0"));
        assert_eq!(
            instance.layout_stats("main").unwrap().geometry_nodes,
            if fixed { 1 } else { 4 }
        );
        let mut expected_doc = instance.source().clone();
        expected_doc
            .nodes
            .iter_mut()
            .find(|node| node.node_id == "text-0")
            .unwrap()
            .text = Some("骑兵骑兵".into());
        let mut fresh = AuiLayoutSolver::new();
        let expected = fresh
            .solve(&expected_doc, "main", 1, &mut Measure::default())
            .unwrap();
        assert_eq!(snapshot.nodes, expected.nodes);
        assert_eq!(snapshot.overflow_node_ids, expected.overflow_node_ids);
    }
}

#[test]
fn gate_c_fixed_manual_island_does_not_visit_one_thousand_sibling_islands() {
    let mut doc = (*document(1000)).clone();
    doc.nodes[0].layout = Some(LayoutSpec::default());
    doc.nodes[0].children.clear();
    for i in 0..1000 {
        let id = format!("island-{i}");
        doc.nodes[0].children.push(id.clone());
        doc.nodes[i + 1].parent = Some(id.clone());
        let mut island = AuiNode::new(
            &id,
            AuiNodeKind::Panel,
            AuiRect::fixed_position(i as f32 * 100., 0., 100., 40.),
        );
        island.parent = Some("root".into());
        island.children = vec![format!("text-{i}")];
        island.layout = Some(LayoutSpec {
            mode: LayoutMode::Row,
            ..Default::default()
        });
        doc.nodes.push(island);
    }
    let mut instance = AuiInstance::mount(Arc::new(doc)).unwrap();
    let mut measure = Measure::default();
    instance.layout("main", &mut measure).unwrap();
    let label = text(&instance, "text-0");
    let mut edit = instance.edit();
    label.set_text(&mut edit, "骑兵骑兵").unwrap();
    edit.commit().unwrap();
    measure.calls.clear();
    let actual = instance.layout("main", &mut measure).unwrap().clone();
    let stats = instance.layout_stats("main").unwrap();
    assert!(
        stats.visited_nodes < 20,
        "visited {} nodes",
        stats.visited_nodes
    );
    assert_eq!(measure.calls.len(), 1);
    let mut expected_doc = instance.source().clone();
    expected_doc.nodes[1].text = Some("骑兵骑兵".into());
    let expected = AuiLayoutSolver::new()
        .solve(&expected_doc, "main", 1, &mut Measure::default())
        .unwrap()
        .clone();
    assert_eq!(actual.nodes, expected.nodes);
    assert_eq!(actual.overflow_node_ids, expected.overflow_node_ids);
}

#[test]
fn gate_c_fixed_island_keeps_custom_ancestor_layout_dependency() {
    struct CustomMeasure {
        inner: Measure,
        layouts: usize,
    }
    impl AuiIntrinsicMeasure for CustomMeasure {
        fn measure(&mut self, input: AuiMeasureInput<'_>) -> AuiMeasureOutput {
            self.inner.measure(input)
        }
        fn layout_children(
            &mut self,
            node_id: &str,
            _: [f32; 2],
            _: &[String],
        ) -> Option<Vec<AuiCustomChildLayout>> {
            if node_id == "custom" {
                self.layouts += 1;
            }
            None
        }
    }
    let mut doc = (*document(1)).clone();
    doc.nodes[0].layout = Some(LayoutSpec::default());
    doc.nodes[0].children = vec!["custom".into()];
    doc.nodes[1].parent = Some("island".into());
    let mut custom = AuiNode::new(
        "custom",
        AuiNodeKind::Custom,
        AuiRect::fixed_position(0., 0., 200., 100.),
    )
    .with_parent("root");
    custom.custom = Some(engine_runtime::aui_extension::AuiCustomNodeSpec {
        type_id: "test.custom".into(),
        schema_version: 1,
        properties: Default::default(),
    });
    custom.children = vec!["island".into()];
    let mut island = AuiNode::new(
        "island",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0., 0., 100., 40.),
    )
    .with_parent("custom");
    island.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        ..Default::default()
    });
    island.children = vec!["text-0".into()];
    doc.nodes.extend([custom, island]);
    let mut instance = AuiInstance::mount(Arc::new(doc)).unwrap();
    let mut measure = CustomMeasure {
        inner: Measure::default(),
        layouts: 0,
    };
    instance.layout("main", &mut measure).unwrap();
    measure.layouts = 0;
    let label = text(&instance, "text-0");
    let mut edit = instance.edit();
    label.set_text(&mut edit, "骑兵骑兵").unwrap();
    edit.commit().unwrap();
    let actual = instance.layout("main", &mut measure).unwrap().clone();
    assert!(
        measure.layouts > 0,
        "custom dependency must propagate beyond the fixed island"
    );
    let mut expected_doc = instance.source().clone();
    expected_doc.nodes[1].text = Some("骑兵骑兵".into());
    let expected = AuiLayoutSolver::new()
        .solve(
            &expected_doc,
            "main",
            1,
            &mut CustomMeasure {
                inner: Measure::default(),
                layouts: 0,
            },
        )
        .unwrap()
        .clone();
    assert_eq!(actual.nodes, expected.nodes);
}

#[test]
fn b10_single_fixed_node_update_reuses_geometry_storage_among_one_thousand_siblings() {
    let mut doc = (*document(1000)).clone();
    for node in doc.nodes.iter_mut().skip(1) {
        node.slot.as_mut().unwrap().width = Dimension::Fixed { value: 20. };
    }
    let mut instance = AuiInstance::mount(Arc::new(doc)).unwrap();
    let mut measure = Measure::default();
    let before = instance.layout("main", &mut measure).unwrap().clone();
    let ptr = instance.layout_snapshot("main").unwrap().nodes.as_ptr();
    let label = text(&instance, "text-500");
    let mut edit = instance.edit();
    label.set_text(&mut edit, "骑兵骑兵").unwrap();
    edit.commit().unwrap();
    let actual = instance.layout("main", &mut measure).unwrap();
    assert_eq!(actual.nodes.as_ptr(), ptr);
    assert_eq!(actual.nodes, before.nodes);
    assert_eq!(actual.overflow_node_ids, before.overflow_node_ids);
    assert_eq!(instance.layout_stats("main").unwrap().geometry_nodes, 1);
}

#[test]
fn b10_resize_visibility_and_overflow_match_full_collection_across_updates() {
    let mut doc = (*document(3)).clone();
    doc.canvases[0].reference_resolution.x = 50.;
    let mut instance = AuiInstance::mount(Arc::new(doc.clone())).unwrap();
    let mut measure = Measure::default();
    instance.layout("main", &mut measure).unwrap();
    let label = text(&instance, "text-0");
    for (value, visibility) in [
        ("骑兵骑兵骑兵", UiVisibility::Visible),
        ("人", UiVisibility::Hidden),
        ("人", UiVisibility::Collapsed),
        ("人", UiVisibility::Visible),
    ] {
        let mut edit = instance.edit();
        label.set_text(&mut edit, value).unwrap();
        label.node().set_visibility(&mut edit, visibility).unwrap();
        edit.commit().unwrap();
        let actual = instance.layout("main", &mut measure).unwrap().clone();
        doc.nodes[1].text = Some(value.into());
        doc.nodes[1].layout_visibility = Some(match visibility {
            UiVisibility::Visible => LayoutVisibility::Visible,
            UiVisibility::Hidden => LayoutVisibility::Hidden,
            UiVisibility::Collapsed => LayoutVisibility::Collapsed,
        });
        let mut fresh = AuiLayoutSolver::new();
        let expected = fresh
            .solve(&doc, "main", 1, &mut Measure::default())
            .unwrap();
        assert_eq!(actual.nodes, expected.nodes);
        assert_eq!(actual.overflow_node_ids, expected.overflow_node_ids);
    }
}

#[test]
fn b14_single_overflow_flip_does_not_scan_the_full_snapshot_summary() {
    let mut doc = (*document(1000)).clone();
    doc.canvases[0].reference_resolution.x = 50.;
    doc.nodes[0].layout.as_mut().unwrap().mode = LayoutMode::Overlay;
    for node in doc.nodes.iter_mut().skip(1) {
        node.slot.as_mut().unwrap().width = Dimension::Fixed { value: 20. };
    }
    doc.nodes[501].slot.as_mut().unwrap().width = Dimension::Fixed { value: 60. };
    let mut instance = AuiInstance::mount(Arc::new(doc.clone())).unwrap();
    let mut measure = Measure::default();
    instance.layout("main", &mut measure).unwrap();

    let label = text(&instance, "text-500");
    let mut edit = instance.edit();
    label
        .node()
        .set_visibility(&mut edit, UiVisibility::Collapsed)
        .unwrap();
    edit.commit().unwrap();
    let actual = instance.layout("main", &mut measure).unwrap().clone();
    let stats = instance.layout_stats("main").unwrap();

    assert_eq!(stats.overflow_summary_entries_updated, 1);
    assert_eq!(stats.overflow_summary_nodes_scanned, 0);
    let mut expected_doc = doc;
    expected_doc.nodes[501].layout_visibility = Some(LayoutVisibility::Collapsed);
    let mut fresh = AuiLayoutSolver::new();
    let expected = fresh
        .solve(&expected_doc, "main", 1, &mut Measure::default())
        .unwrap();
    assert_eq!(actual.nodes, expected.nodes);
    assert_eq!(actual.overflow_node_ids, expected.overflow_node_ids);
}

#[test]
fn retained_mounted_layout_can_move_with_its_exclusive_session_owner() {
    let mut instance = AuiInstance::mount(document(2)).unwrap();
    instance.layout("main", &mut Measure::default()).unwrap();
    let instance = std::thread::spawn(move || {
        let label = text(&instance, "text-0");
        let mut edit = instance.edit();
        label.set_text(&mut edit, "骑兵").unwrap();
        edit.commit().unwrap();
        assert_eq!(
            instance
                .layout("main", &mut Measure::default())
                .unwrap()
                .node("text-0")
                .unwrap()
                .rect
                .width,
            20.
        );
        instance
    })
    .join()
    .unwrap();
    assert_eq!(
        instance
            .layout_snapshot("main")
            .unwrap()
            .node("text-0")
            .unwrap()
            .rect
            .width,
        20.
    );
}
