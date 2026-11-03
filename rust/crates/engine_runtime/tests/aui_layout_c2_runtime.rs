use engine_runtime::aui::*;
use engine_runtime::aui_layout_runtime::*;
use engine_runtime::aui_layout_schema::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct ProbeMeasure {
    calls: BTreeMap<String, usize>,
    widths: BTreeMap<String, f32>,
    invalid_node: Option<String>,
}

impl AuiIntrinsicMeasure for ProbeMeasure {
    fn measure(&mut self, input: AuiMeasureInput<'_>) -> AuiMeasureOutput {
        *self.calls.entry(input.node_id.to_owned()).or_default() += 1;
        let width = self.widths.get(input.node_id).copied().unwrap_or(40.0);
        let intrinsic_height = match input.node_id {
            "tall" => 40.0,
            _ if input.text.is_some() => 20.0,
            _ => 10.0,
        };
        let height = input.known_height.unwrap_or(intrinsic_height);
        let mut output = AuiMeasureOutput {
            width: input.known_width.unwrap_or(width),
            height,
            first_baseline: input.text.map(|_| height * 0.75),
        };
        if self.invalid_node.as_deref() == Some(input.node_id) {
            output.width = f32::NAN;
        }
        output
    }
}

fn node(id: &str) -> AuiNode {
    AuiNode::new(
        id,
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 10.0, 10.0),
    )
}

fn fixed(width: f32, height: f32) -> SlotSpec {
    SlotSpec {
        width: Dimension::Fixed { value: width },
        height: Dimension::Fixed { value: height },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

fn document(mode: LayoutMode, mut children: Vec<AuiNode>) -> AuiDocument {
    let mut root = node("root");
    root.rect = AuiRect::stretch_full();
    root.layout = Some(LayoutSpec {
        mode,
        ..Default::default()
    });
    root.children = children.iter().map(|child| child.node_id.clone()).collect();
    for child in &mut children {
        child.parent = Some("root".into());
    }
    let mut nodes = vec![root];
    nodes.extend(children);
    let mut document = AuiDocument::new(
        "runtime",
        vec![AuiCanvas::screen_overlay("main", 300.0, 200.0, "root")],
        nodes,
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.01, "{actual} != {expected}");
}

#[test]
fn row_column_flex_and_grid_share_deterministic_absolute_geometry() {
    for mode in [LayoutMode::Row, LayoutMode::Column, LayoutMode::Flex] {
        let mut a = node("a");
        a.slot = Some(fixed(30.0, 20.0));
        let mut b = node("b");
        b.slot = Some(fixed(40.0, 25.0));
        let doc = document(mode, vec![a, b]);
        let mut solver = AuiLayoutSolver::new();
        let snapshot = solver
            .solve(&doc, "main", 1, &mut ProbeMeasure::default())
            .unwrap()
            .clone();
        if mode == LayoutMode::Column {
            near(snapshot.node("b").unwrap().rect.y, 20.0);
        } else {
            near(snapshot.node("b").unwrap().rect.x, 30.0);
        }
        assert_eq!(
            snapshot.nodes,
            solver
                .solve(&doc, "main", 1, &mut ProbeMeasure::default())
                .unwrap()
                .nodes
        );
    }

    let mut a = node("a");
    a.slot = Some(fixed(30.0, 20.0));
    let mut b = node("b");
    b.slot = Some(fixed(40.0, 25.0));
    let mut doc = document(LayoutMode::Grid, vec![a, b]);
    doc.nodes[0].layout.as_mut().unwrap().grid_columns = vec![
        GridTrack::Fixed { value: 50.0 },
        GridTrack::Fr { value: 1.0 },
    ];
    doc.nodes[1].slot.as_mut().unwrap().grid_column.start = Some(1);
    doc.nodes[2].slot.as_mut().unwrap().grid_column.start = Some(2);
    let mut solver = AuiLayoutSolver::new();
    let snapshot = solver
        .solve(&doc, "main", 1, &mut ProbeMeasure::default())
        .unwrap();
    near(snapshot.node("b").unwrap().rect.x, 50.0);
}

#[test]
fn anchor_and_overlay_mix_keeps_legacy_anchor_semantics() {
    let mut flow = node("flow");
    flow.slot = Some(fixed(80.0, 30.0));
    let mut floating = node("floating");
    floating.slot = Some(SlotSpec {
        participation: Participation::OutOfFlow,
        ..fixed(10.0, 10.0)
    });
    floating.rect = AuiRect::fixed_position(25.0, 35.0, 10.0, 10.0);
    let doc = document(LayoutMode::Overlay, vec![flow, floating]);
    let mut solver = AuiLayoutSolver::new();
    let snapshot = solver
        .solve(&doc, "main", 1, &mut ProbeMeasure::default())
        .unwrap();
    near(snapshot.node("flow").unwrap().rect.width, 80.0);
    assert_eq!(
        snapshot.node("floating").unwrap().rect,
        doc.nodes[2].rect.resolve(AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 200.0
        })
    );
}

#[test]
fn text_first_baseline_reaches_flex_alignment() {
    for mode in [LayoutMode::Row, LayoutMode::Grid] {
        let mut short = node("short");
        short.kind = AuiNodeKind::Text;
        short.text = Some("short".into());
        short.slot = Some(fixed(40.0, 20.0));
        let mut tall = node("tall");
        tall.kind = AuiNodeKind::Text;
        tall.text = Some("tall".into());
        tall.slot = Some(fixed(40.0, 40.0));
        let mut doc = document(mode, vec![short, tall]);
        let layout = doc.nodes[0].layout.as_mut().unwrap();
        layout.align_items = AlignItems::Baseline;
        if mode == LayoutMode::Grid {
            layout.grid_columns = vec![
                GridTrack::Fixed { value: 40.0 },
                GridTrack::Fixed { value: 40.0 },
            ];
        }
        let mut measure = ProbeMeasure::default();
        let snapshot = AuiLayoutSolver::new()
            .solve(&doc, "main", 1, &mut measure)
            .unwrap()
            .clone();
        let first = snapshot.node("short").unwrap();
        let second = snapshot.node("tall").unwrap();
        near(first.rect.y + 15.0, second.rect.y + 30.0);
    }
}

#[test]
fn hidden_keeps_space_while_collapsed_is_zero_sized() {
    let mut hidden = node("hidden");
    hidden.slot = Some(fixed(20.0, 20.0));
    hidden.layout_visibility = Some(LayoutVisibility::Hidden);
    let mut collapsed = node("collapsed");
    collapsed.slot = Some(fixed(80.0, 80.0));
    collapsed.layout_visibility = Some(LayoutVisibility::Collapsed);
    let mut visible = node("visible");
    visible.slot = Some(fixed(20.0, 20.0));
    let doc = document(LayoutMode::Row, vec![hidden, collapsed, visible]);
    let mut solver = AuiLayoutSolver::new();
    let snapshot = solver
        .solve(&doc, "main", 1, &mut ProbeMeasure::default())
        .unwrap();
    near(snapshot.node("visible").unwrap().rect.x, 20.0);
    near(snapshot.node("collapsed").unwrap().rect.width, 0.0);
    assert_eq!(
        snapshot.node("hidden").unwrap().visibility,
        LayoutVisibility::Hidden
    );
}

#[test]
fn overflow_is_computed_from_final_shared_rectangles() {
    let mut child = node("wide");
    child.slot = Some(fixed(400.0, 20.0));
    let mut doc = document(LayoutMode::Row, vec![child]);
    doc.nodes[0].overflow = Some(Overflow::Clip);
    let mut solver = AuiLayoutSolver::new();
    let snapshot = solver
        .solve(&doc, "main", 1, &mut ProbeMeasure::default())
        .unwrap();
    assert_eq!(snapshot.overflow_node_ids, vec!["wide"]);
    assert!(snapshot.node("wide").unwrap().overflows_parent);
}

#[test]
fn percent_with_indefinite_parent_basis_is_rejected() {
    let mut parent = node("content-parent");
    parent.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        ..Default::default()
    });
    let mut child = node("percent-child");
    child.parent = Some("content-parent".into());
    child.slot = Some(SlotSpec {
        width: Dimension::Percent { value: 0.5 },
        height: Dimension::Fixed { value: 10.0 },
        flex_shrink: 0.0,
        ..Default::default()
    });
    parent.children.push(child.node_id.clone());
    let mut doc = document(LayoutMode::Row, vec![parent, child]);
    doc.nodes[0].children = vec!["content-parent".into()];
    doc.nodes[2].parent = Some("content-parent".into());
    let error = AuiLayoutSolver::new()
        .solve(&doc, "main", 1, &mut ProbeMeasure::default())
        .unwrap_err();
    assert!(error
        .iter()
        .any(|item| item.code == "aui_layout_indefinite_percent_basis"));
}

#[test]
fn percent_and_fill_use_each_axis_definiteness_independently() {
    let mut parent = node("partial-parent");
    parent.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        ..Default::default()
    });
    parent.slot = Some(SlotSpec {
        width: Dimension::Fixed { value: 200.0 },
        height: Dimension::Content,
        ..Default::default()
    });
    let mut percent_child = node("percent");
    percent_child.parent = Some("partial-parent".into());
    percent_child.slot = Some(SlotSpec {
        width: Dimension::Percent { value: 0.5 },
        height: Dimension::Fixed { value: 10.0 },
        flex_shrink: 0.0,
        ..Default::default()
    });
    let mut fill_child = node("fill");
    fill_child.parent = Some("partial-parent".into());
    fill_child.slot = Some(SlotSpec {
        width: Dimension::Fill,
        height: Dimension::Fixed { value: 10.0 },
        flex_shrink: 0.0,
        ..Default::default()
    });
    parent.children = vec!["percent".into(), "fill".into()];
    let mut doc = document(LayoutMode::Column, vec![parent, percent_child, fill_child]);
    doc.nodes[0].children = vec!["partial-parent".into()];
    doc.nodes[2].parent = Some("partial-parent".into());
    doc.nodes[3].parent = Some("partial-parent".into());
    let mut solver = AuiLayoutSolver::new();
    let snapshot = solver
        .solve(&doc, "main", 1, &mut ProbeMeasure::default())
        .unwrap();
    near(snapshot.node("percent").unwrap().rect.width, 100.0);
    assert!(snapshot.node("fill").unwrap().rect.width.is_finite());
}

#[test]
fn clean_revision_skips_measure_and_layout_work() {
    let mut text = node("text");
    text.kind = AuiNodeKind::Text;
    text.text = Some("cached".into());
    let doc = document(LayoutMode::Row, vec![text]);
    let mut solver = AuiLayoutSolver::new();
    let mut measure = ProbeMeasure::default();
    solver.solve(&doc, "main", 9, &mut measure).unwrap();
    let first_calls = measure.calls.clone();
    solver.solve(&doc, "main", 9, &mut measure).unwrap();
    assert_eq!(measure.calls, first_calls);
    assert!(solver.last_stats().clean_snapshot_reused);
    assert_eq!(solver.last_stats().visited_nodes, 0);
}

#[test]
fn dirty_text_does_not_remeasure_independent_sibling() {
    let mut a = node("a");
    a.kind = AuiNodeKind::Text;
    a.text = Some("a".into());
    let mut b = node("b");
    b.kind = AuiNodeKind::Text;
    b.text = Some("b".into());
    let doc = document(LayoutMode::Column, vec![a, b]);
    let mut solver = AuiLayoutSolver::new();
    let mut measure = ProbeMeasure::default();
    solver.solve(&doc, "main", 1, &mut measure).unwrap();
    measure.calls.clear();
    measure.widths.insert("a".into(), 90.0);
    solver.mark_dirty("a").unwrap();
    solver.solve(&doc, "main", 1, &mut measure).unwrap();
    assert!(measure.calls.contains_key("a"));
    assert!(
        !measure.calls.contains_key("b"),
        "calls={:?}",
        measure.calls
    );
}

#[test]
fn revision_text_change_invalidates_only_its_measure_path() {
    let mut a = node("a");
    a.kind = AuiNodeKind::Text;
    a.text = Some("a".into());
    let mut b = node("b");
    b.kind = AuiNodeKind::Text;
    b.text = Some("b".into());
    let mut doc = document(LayoutMode::Column, vec![a, b]);
    let mut solver = AuiLayoutSolver::new();
    let mut measure = ProbeMeasure::default();
    solver.solve(&doc, "main", 1, &mut measure).unwrap();
    measure.calls.clear();
    doc.nodes[1].text = Some("changed".into());
    solver.solve(&doc, "main", 2, &mut measure).unwrap();
    assert!(measure.calls.contains_key("a"));
    assert!(
        !measure.calls.contains_key("b"),
        "calls={:?}",
        measure.calls
    );
}

#[test]
fn schema_budgets_and_runtime_measure_budget_fail_closed() {
    let mut deep = document(LayoutMode::Column, vec![]);
    for index in 0..=MAX_LAYOUT_DEPTH {
        let id = format!("n{index}");
        let parent = if index == 0 {
            "root".into()
        } else {
            format!("n{}", index - 1)
        };
        let mut item = node(&id);
        item.parent = Some(parent.clone());
        if let Some(parent_node) = deep.nodes.iter_mut().find(|node| node.node_id == parent) {
            parent_node.children.push(id);
        }
        deep.nodes.push(item);
    }
    let error = AuiLayoutSolver::new()
        .solve(&deep, "main", 1, &mut ProbeMeasure::default())
        .unwrap_err();
    assert!(error
        .iter()
        .any(|item| item.code == "aui_layout_depth_budget"));
    assert_eq!(MAX_LAYOUT_MEASURE_CALLS, 1_000_000);
    assert_eq!(MAX_LAYOUT_NODES, 65_536);

    let mut too_many = document(LayoutMode::Column, vec![]);
    too_many.nodes = (0..=MAX_LAYOUT_NODES)
        .map(|index| node(&format!("n{index}")))
        .collect();
    too_many.canvases[0].root_node = "n0".into();
    let error = AuiLayoutSolver::new()
        .solve(&too_many, "main", 1, &mut ProbeMeasure::default())
        .unwrap_err();
    assert!(error
        .iter()
        .any(|item| item.code == "aui_layout_node_budget"));

    let mut a = node("a");
    a.kind = AuiNodeKind::Text;
    let mut b = node("b");
    b.kind = AuiNodeKind::Text;
    let doc = document(LayoutMode::Row, vec![a, b]);
    let mut solver = AuiLayoutSolver::new();
    solver.set_measure_budget(1);
    let error = solver
        .solve(&doc, "main", 1, &mut ProbeMeasure::default())
        .unwrap_err();
    assert!(error
        .iter()
        .any(|item| item.code == "aui_layout_measure_budget"));

    let mut invalid_solver = AuiLayoutSolver::new();
    let mut invalid_measure = ProbeMeasure {
        invalid_node: Some("a".into()),
        ..Default::default()
    };
    let error = invalid_solver
        .solve(&doc, "main", 1, &mut invalid_measure)
        .unwrap_err();
    assert!(error
        .iter()
        .any(|item| item.code == "aui_layout_measure_invalid"));
}
