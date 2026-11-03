//! 356 M1 selection experiments. Not a production AUI adapter or paragraph API.
use engine_runtime::text_shaping::{layout_line, shape_text};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;
use taffy::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

#[path = "support/aui_c2_baseline_probe.rs"]
mod baseline_probe;

const EPSILON: f32 = 0.01;

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < EPSILON,
        "{actual} != {expected}"
    );
}

fn font() -> &'static [u8] {
    static FONT: OnceLock<Vec<u8>> = OnceLock::new();
    FONT.get_or_init(|| {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../samples/neon_lab_shared/assets/fonts/NotoSansSC-Regular.ttf"
        ))
        .expect("existing shared qualification font")
    })
}

fn text_size(text: &str) -> Size<f32> {
    let run = shape_text("shared-noto", font(), text).expect("valid font");
    let line = layout_line(&[run], 20.0, 0.0, false);
    Size {
        width: line.width,
        height: line.height,
    }
}

// Deliberately narrow probe: CJK/grapheme wrapping only, not Unicode line-breaking.
// Reshape whole candidate lines; never estimate width using character counts.
fn probe_paragraph(text: &str, limit: f32) -> Size<f32> {
    let mut current = String::new();
    let mut width = 0.0_f32;
    let mut lines = 1;
    for grapheme in text.graphemes(true) {
        let candidate = format!("{current}{grapheme}");
        if !current.is_empty() && text_size(&candidate).width > limit {
            width = width.max(text_size(&current).width);
            current.clear();
            lines += 1;
        }
        current.push_str(grapheme);
    }
    width = width.max(text_size(&current).width);
    Size {
        width,
        height: text_size("国Ag").height * lines as f32,
    }
}

fn solve_text(tree: &mut TaffyTree<String>, root: NodeId) -> HashMap<NodeId, usize> {
    let mut calls = HashMap::new();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |known, available, id, text, _| {
        let Some(text) = text else { return Size::ZERO };
        *calls.entry(id).or_default() += 1;
        let limit = known.width.unwrap_or(match available.width {
            AvailableSpace::Definite(width) => width,
            AvailableSpace::MinContent => text
                .graphemes(true)
                .map(|s| text_size(s).width)
                .fold(0.0, f32::max),
            AvailableSpace::MaxContent => f32::INFINITY,
        });
        let measured = probe_paragraph(text, limit);
        Size {
            width: known.width.unwrap_or(measured.width),
            height: known.height.unwrap_or(measured.height),
        }
    })
    .unwrap();
    calls
}

fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: Size {
            width: length(width),
            height: length(height),
        },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

#[test]
fn flex_matches_independent_weighted_space_oracle() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let widths = [20.0, 30.0, 40.0];
    let weights = [1.0, 2.0, 3.0];
    let children: Vec<_> = widths
        .iter()
        .zip(weights)
        .map(|(&width, weight)| {
            tree.new_leaf(Style {
                flex_basis: length(width),
                flex_grow: weight,
                min_size: Size {
                    width: zero(),
                    height: zero(),
                },
                ..Default::default()
            })
            .unwrap()
        })
        .collect();
    let root = tree
        .new_with_children(
            Style {
                gap: Size {
                    width: length(10.0),
                    height: zero(),
                },
                ..fixed(350.0, 40.0)
            },
            &children,
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let extra = 350.0 - widths.iter().sum::<f32>() - 20.0;
    let mut x = 0.0;
    for (i, child) in children.iter().enumerate() {
        let expected = widths[i] + extra * weights[i] / weights.iter().sum::<f32>();
        near(tree.layout(*child).unwrap().size.width, expected);
        near(tree.layout(*child).unwrap().location.x, x);
        x += expected + 10.0;
    }
}

#[test]
fn min_max_freezes_growth_and_redistributes_surplus() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let capped = tree
        .new_leaf(Style {
            flex_grow: 1.0,
            max_size: Size {
                width: length(60.0),
                height: auto(),
            },
            ..fixed(40.0, 20.0)
        })
        .unwrap();
    let other = tree
        .new_leaf(Style {
            flex_grow: 1.0,
            ..fixed(40.0, 20.0)
        })
        .unwrap();
    let root = tree
        .new_with_children(fixed(200.0, 20.0), &[capped, other])
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    near(tree.layout(capped).unwrap().size.width, 60.0);
    near(tree.layout(other).unwrap().size.width, 140.0);
}

#[test]
fn grid_tracks_span_and_nested_flex_have_one_geometry() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let a = tree
        .new_leaf(Style {
            flex_grow: 1.0,
            ..Default::default()
        })
        .unwrap();
    let b = tree
        .new_leaf(Style {
            flex_grow: 1.0,
            ..Default::default()
        })
        .unwrap();
    let row = tree
        .new_with_children(
            Style {
                grid_column: line(2),
                grid_row: line(1),
                ..Default::default()
            },
            &[a, b],
        )
        .unwrap();
    let span_node = tree
        .new_leaf(Style {
            grid_column: span(2),
            grid_row: line(2),
            ..Default::default()
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_template_columns: vec![length(100.0), fr(1.0)],
                grid_template_rows: vec![length(40.0), length(20.0)],
                gap: Size {
                    width: length(10.0),
                    height: length(5.0),
                },
                ..fixed(310.0, 65.0)
            },
            &[row, span_node],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    near(tree.layout(row).unwrap().location.x, 110.0);
    near(tree.layout(a).unwrap().size.width, 100.0);
    near(tree.layout(b).unwrap().location.x, 100.0);
    near(tree.layout(span_node).unwrap().size.width, 310.0);
    near(tree.layout(span_node).unwrap().location.y, 45.0);
}

#[test]
fn flex_wrap_increases_container_height() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let children: Vec<_> = (0..3)
        .map(|_| tree.new_leaf(fixed(60.0, 20.0)).unwrap())
        .collect();
    let root = tree
        .new_with_children(
            Style {
                flex_wrap: FlexWrap::Wrap,
                size: Size {
                    width: length(130.0),
                    height: auto(),
                },
                gap: Size {
                    width: length(10.0),
                    height: length(5.0),
                },
                ..Default::default()
            },
            &children,
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    near(tree.layout(root).unwrap().size.height, 45.0);
    near(tree.layout(children[2]).unwrap().location.y, 25.0);
}

#[test]
fn actual_font_width_changes_height_and_following_button_position() {
    let mut tree: TaffyTree<String> = TaffyTree::new();
    tree.disable_rounding();
    let text = "霓虹实验室自动布局验证文字";
    let label = tree
        .new_leaf_with_context(Style::default(), text.into())
        .unwrap();
    let button = tree.new_leaf(fixed(80.0, 30.0)).unwrap();
    let mut style = Style {
        flex_direction: FlexDirection::Column,
        size: Size {
            width: length(120.0),
            height: auto(),
        },
        gap: Size {
            width: zero(),
            height: length(8.0),
        },
        ..Default::default()
    };
    let root = tree
        .new_with_children(style.clone(), &[label, button])
        .unwrap();
    assert!(!solve_text(&mut tree, root).is_empty());
    let narrow = tree.layout(root).unwrap().size.height;
    near(
        tree.layout(label).unwrap().size.height,
        probe_paragraph(text, 120.0).height,
    );
    near(
        tree.layout(button).unwrap().location.y,
        tree.layout(label).unwrap().size.height + 8.0,
    );
    assert!(
        solve_text(&mut tree, root).is_empty(),
        "clean frame must reuse measured content"
    );
    style.size.width = length(300.0);
    tree.set_style(root, style).unwrap();
    assert!(!solve_text(&mut tree, root).is_empty());
    assert!(tree.layout(root).unwrap().size.height < narrow);
}

#[test]
fn dirty_text_does_not_remeasure_independent_sibling() {
    let mut tree: TaffyTree<String> = TaffyTree::new();
    let a = tree
        .new_leaf_with_context(Style::default(), "第一行".into())
        .unwrap();
    let b = tree
        .new_leaf_with_context(Style::default(), "第二行".into())
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: length(200.0),
                    height: auto(),
                },
                ..Default::default()
            },
            &[a, b],
        )
        .unwrap();
    solve_text(&mut tree, root);
    *tree.get_node_context_mut(a).unwrap() = "改变后的第一行".into();
    tree.mark_dirty(a).unwrap();
    let calls = solve_text(&mut tree, root);
    assert!(calls.contains_key(&a));
    assert!(
        !calls.contains_key(&b),
        "unrelated fixed-width paragraph remeasured: {calls:?}"
    );
}

#[test]
fn unbounded_fill_adapter_uses_intrinsic_size_without_infinity() {
    let mut tree: TaffyTree<String> = TaffyTree::new();
    tree.disable_rounding();
    let label = tree
        .new_leaf_with_context(Style::default(), "内容宽度".into())
        .unwrap();
    // Engine Fill in an indefinite axis is mapped to Auto, not Percent(1).
    let root = tree.new_with_children(Style::default(), &[label]).unwrap();
    solve_text(&mut tree, root);
    near(
        tree.layout(root).unwrap().size.width,
        text_size("内容宽度").width,
    );
    assert!(tree.layout(root).unwrap().size.height.is_finite());
}

#[test]
fn border_box_and_collapsed_child_are_explicit() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let collapsed = tree
        .new_leaf(Style {
            display: Display::None,
            ..fixed(900.0, 900.0)
        })
        .unwrap();
    let child = tree.new_leaf(fixed(40.0, 20.0)).unwrap();
    let root = tree
        .new_with_children(
            Style {
                padding: Rect {
                    left: length(10.0),
                    right: length(10.0),
                    top: length(10.0),
                    bottom: length(10.0),
                },
                border: Rect {
                    left: length(2.0),
                    right: length(2.0),
                    top: length(2.0),
                    bottom: length(2.0),
                },
                size: Size {
                    width: length(100.0),
                    height: auto(),
                },
                ..Default::default()
            },
            &[collapsed, child],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    near(tree.layout(root).unwrap().size.height, 44.0);
    near(tree.layout(child).unwrap().location.x, 12.0);
    near(tree.layout(collapsed).unwrap().size.width, 0.0);
}

// A minimal preflight probe: reject invalid engine trees before entering a solver.
fn validate_tree(edges: &[Vec<usize>], max_depth: usize) -> Result<(), &'static str> {
    let mut incoming = vec![0usize; edges.len()];
    for children in edges {
        for &child in children {
            if child >= edges.len() {
                return Err("missing_child");
            }
            incoming[child] += 1;
            if incoming[child] > 1 {
                return Err("multiple_parents");
            }
        }
    }
    let mut colors = vec![0u8; edges.len()];
    // Traverse real roots first so child-before-parent array order cannot hide depth.
    let roots = (0..edges.len())
        .filter(|&i| incoming[i] == 0)
        .chain((0..edges.len()).filter(|&i| incoming[i] != 0));
    for root in roots {
        if colors[root] != 0 {
            continue;
        }
        let mut stack = vec![(root, 0, false)];
        while let Some((node, depth, exit)) = stack.pop() {
            if exit {
                colors[node] = 2;
                continue;
            }
            if colors[node] == 1 {
                return Err("cycle");
            }
            if colors[node] == 2 {
                continue;
            }
            if depth > max_depth {
                return Err("depth_limit");
            }
            colors[node] = 1;
            stack.push((node, depth, true));
            for &child in edges[node].iter().rev() {
                stack.push((child, depth + 1, false));
            }
        }
    }
    Ok(())
}

#[test]
fn preflight_rejects_cycles_dangling_nodes_shared_children_and_depth() {
    assert_eq!(validate_tree(&[vec![1], vec![0]], 128), Err("cycle"));
    assert_eq!(validate_tree(&[vec![2]], 128), Err("missing_child"));
    assert_eq!(
        validate_tree(&[vec![2], vec![2], vec![]], 128),
        Err("multiple_parents")
    );
    assert_eq!(
        validate_tree(&[vec![1], vec![2], vec![]], 1),
        Err("depth_limit")
    );
    assert_eq!(
        validate_tree(&[vec![], vec![0], vec![1]], 1),
        Err("depth_limit")
    );
    assert_eq!(validate_tree(&[vec![1, 2], vec![], vec![]], 128), Ok(()));
}

#[test]
fn true_font_metrics_and_graphemes_are_required() {
    assert!(text_size("WWW").width > text_size("iii").width);
    let combined = "a\u{301}";
    assert_eq!(combined.graphemes(true).count(), 1);
    near(
        probe_paragraph(combined, 1.0).height,
        text_size("国Ag").height,
    );
    assert!(
        probe_paragraph(combined, 1.0).width > 1.0,
        "unbreakable cluster must retain overflow"
    );
}

#[test]
fn deterministic_rebuild_does_not_depend_on_allocator_ids() {
    fn build() -> Vec<(f32, f32)> {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let children: Vec<_> = (0..7)
            .map(|_| {
                tree.new_leaf(Style {
                    flex_grow: 1.0,
                    ..Default::default()
                })
                .unwrap()
            })
            .collect();
        let root = tree
            .new_with_children(fixed(100.0, 30.0), &children)
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        children
            .iter()
            .map(|&n| {
                let l = tree.layout(n).unwrap();
                (l.location.x, l.size.width)
            })
            .collect()
    }
    assert_eq!(build(), build());
}

// CPU geometry experiment only; does not qualify a GPU clipping implementation.
fn rounded_contains(x: f32, y: f32, width: f32, height: f32, radius: f32) -> bool {
    let radius = radius.clamp(0.0, width.min(height) * 0.5);
    let qx = (x - width * 0.5).abs() - (width * 0.5 - radius);
    let qy = (y - height * 0.5).abs() - (height * 0.5 - radius);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius <= 0.0
}

#[test]
fn rounded_background_and_parent_clip_are_separate_contracts() {
    assert!(rounded_contains(50.0, 20.0, 100.0, 40.0, 10.0));
    assert!(!rounded_contains(0.0, 0.0, 100.0, 40.0, 10.0));
    assert!(rounded_contains(0.0, 0.0, 100.0, 40.0, 0.0));
    let parent_clip_right = 60.0;
    assert!(rounded_contains(80.0, 20.0, 100.0, 40.0, 10.0));
    assert!(
        80.0 > parent_clip_right,
        "rounded fill alone cannot implement parent clipping"
    );
}

#[test]
#[ignore = "explicit M1 scaling experiment; not a product performance qualification"]
fn selection_scale_measurements() {
    for count in [100usize, 1_000, 10_000] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let children: Vec<_> = (0..count)
            .map(|_| tree.new_leaf(fixed(10.0, 10.0)).unwrap())
            .collect();
        let root = tree
            .new_with_children(
                Style {
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                &children,
            )
            .unwrap();
        let start = Instant::now();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let cold = start.elapsed();
        let start = Instant::now();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let clean = start.elapsed();
        tree.set_style(children[count / 2], fixed(10.0, 20.0))
            .unwrap();
        let start = Instant::now();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let dirty = start.elapsed();
        near(
            tree.layout(root).unwrap().size.height,
            count as f32 * 10.0 + 10.0,
        );
        tree.disable_rounding();
        let start = Instant::now();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let clean_without_rounding = start.elapsed();
        for &child in &children {
            tree.set_style(child, fixed(10.0, 12.0)).unwrap();
        }
        let start = Instant::now();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let all_dirty = start.elapsed();
        near(tree.layout(root).unwrap().size.height, count as f32 * 12.0);
        println!(
            "scale nodes={count} cold_us={} clean_us={} one_dirty_us={} clean_no_round_us={} all_dirty_no_round_us={}",
            cold.as_micros(),
            clean.as_micros(),
            dirty.as_micros(),
            clean_without_rounding.as_micros(),
            all_dirty.as_micros()
        );
    }
}

#[test]
fn unicode_break_candidates_preserve_combining_sequences_and_mandatory_breaks() {
    use unicode_linebreak::{linebreaks, BreakOpportunity};
    let text = "a\u{301} b\n中文字";
    let breaks: Vec<_> = linebreaks(text).collect();
    assert!(!breaks.iter().any(|&(offset, _)| offset == 1));
    let after_newline = text.find('\n').unwrap() + 1;
    assert!(breaks.contains(&(after_newline, BreakOpportunity::Mandatory)));
    assert!(breaks.contains(&(after_newline + "中".len(), BreakOpportunity::Allowed)));
    assert_eq!(
        breaks.last(),
        Some(&(text.len(), BreakOpportunity::Mandatory))
    );
    assert!(breaks
        .iter()
        .all(|&(offset, _)| text.is_char_boundary(offset)));
}

#[test]
fn bidi_keeps_logical_source_and_reorders_visual_runs() {
    let source = "abc אבג";
    let bidi = unicode_bidi::BidiInfo::new(source, None);
    let para = &bidi.paragraphs[0];
    assert_eq!(bidi.reorder_line(para, para.range.clone()), "abc גבא");
    assert_eq!(bidi.text, source);
    assert!(bidi.levels.iter().any(|level| level.is_rtl()));
    // This does not claim font coverage, caret mapping or Arabic shaping qualification.
}

#[test]
fn definite_parent_percentage_resolves_against_content_box() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let child = tree
        .new_leaf(Style {
            size: Size {
                width: percent(0.5),
                height: length(20.0),
            },
            ..Default::default()
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                padding: Rect {
                    left: length(10.0),
                    right: length(10.0),
                    top: zero(),
                    bottom: zero(),
                },
                ..fixed(220.0, 40.0)
            },
            &[child],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    near(tree.layout(child).unwrap().size.width, 100.0);
}

// Narrow adapter-policy probe: a child cannot use a content-sized parent's unknown
// width as the percentage basis. Reject it before solver invocation or old-frame reuse.
fn constrained_percent(parent_width: Option<f32>, ratio: f32) -> Result<f32, &'static str> {
    if !ratio.is_finite() || ratio < 0.0 {
        return Err("invalid_ratio");
    }
    let parent = parent_width.ok_or("indefinite_percentage_basis")?;
    if !parent.is_finite() || parent < 0.0 {
        return Err("invalid_parent_size");
    }
    let result = parent * ratio;
    if !result.is_finite() {
        return Err("size_overflow");
    }
    Ok(result)
}

#[test]
fn content_parent_percentage_child_cycle_is_rejected_before_solver() {
    assert_eq!(
        constrained_percent(None, 1.0),
        Err("indefinite_percentage_basis")
    );
    assert_eq!(constrained_percent(Some(200.0), 0.5), Ok(100.0));
    assert_eq!(
        constrained_percent(Some(200.0), f32::NAN),
        Err("invalid_ratio")
    );
    assert_eq!(
        constrained_percent(Some(-1.0), 0.5),
        Err("invalid_parent_size")
    );
    assert_eq!(
        constrained_percent(Some(f32::MAX), 2.0),
        Err("size_overflow")
    );
}
