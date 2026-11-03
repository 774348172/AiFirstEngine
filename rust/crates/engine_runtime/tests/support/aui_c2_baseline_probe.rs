//! Small low-level adapter experiment: preserve engine-provided text baselines.
use taffy::prelude::*;
use taffy::{LayoutInput, LayoutOutput, LayoutPartialTree, Point, TraversePartialTree};

struct ProbeNode {
    style: Style,
    children: Vec<NodeId>,
    layout: Layout,
    baseline: Option<f32>,
}

struct ProbeTree(Vec<ProbeNode>);

impl TraversePartialTree for ProbeTree {
    type ChildIter<'a> = std::iter::Copied<std::slice::Iter<'a, NodeId>>;
    fn child_ids(&self, parent: NodeId) -> Self::ChildIter<'_> {
        self.0[usize::from(parent)].children.iter().copied()
    }
    fn child_count(&self, parent: NodeId) -> usize {
        self.0[usize::from(parent)].children.len()
    }
    fn get_child_id(&self, parent: NodeId, index: usize) -> NodeId {
        self.0[usize::from(parent)].children[index]
    }
}

impl LayoutPartialTree for ProbeTree {
    type CustomIdent = String;
    type CoreContainerStyle<'a> = &'a Style;
    fn get_core_container_style(&self, node: NodeId) -> &Style {
        &self.0[usize::from(node)].style
    }
    fn set_unrounded_layout(&mut self, node: NodeId, layout: &Layout) {
        self.0[usize::from(node)].layout = *layout;
    }
    fn resolve_calc_value(&self, _: *const (), _: f32) -> f32 {
        panic!("calc is outside the selected engine contract")
    }
    fn compute_child_layout(&mut self, node: NodeId, input: LayoutInput) -> LayoutOutput {
        let index = usize::from(node);
        if self.0[index].children.is_empty() {
            let mut result = taffy::compute_leaf_layout(
                input,
                &self.0[index].style,
                |_, _| 0.0,
                |_, _| Size::ZERO,
            );
            result.first_baselines = Point {
                x: None,
                y: self.0[index].baseline,
            };
            result
        } else if self.0[index].style.display == Display::Grid {
            taffy::compute_grid_layout(self, node, input)
        } else {
            taffy::compute_flexbox_layout(self, node, input)
        }
    }
}

impl taffy::LayoutFlexboxContainer for ProbeTree {
    type FlexboxContainerStyle<'a> = &'a Style;
    type FlexboxItemStyle<'a> = &'a Style;
    fn get_flexbox_container_style(&self, node: NodeId) -> &Style {
        &self.0[usize::from(node)].style
    }
    fn get_flexbox_child_style(&self, node: NodeId) -> &Style {
        &self.0[usize::from(node)].style
    }
}

impl taffy::LayoutGridContainer for ProbeTree {
    type GridContainerStyle<'a> = &'a Style;
    type GridItemStyle<'a> = &'a Style;
    fn get_grid_container_style(&self, node: NodeId) -> &Style {
        &self.0[usize::from(node)].style
    }
    fn get_grid_child_style(&self, node: NodeId) -> &Style {
        &self.0[usize::from(node)].style
    }
}

fn node(style: Style, baseline: Option<f32>, children: Vec<NodeId>) -> ProbeNode {
    ProbeNode {
        style,
        baseline,
        children,
        layout: Layout::default(),
    }
}

#[test]
fn low_level_adapter_preserves_distinct_text_baselines_in_flex_and_grid() {
    for display in [Display::Flex, Display::Grid] {
        let mut tree = ProbeTree(vec![
            node(super::fixed(40.0, 20.0), Some(12.0), vec![]),
            node(super::fixed(40.0, 40.0), Some(30.0), vec![]),
            node(
                Style {
                    display,
                    align_items: Some(AlignItems::BASELINE),
                    grid_template_columns: vec![length(40.0), length(40.0)],
                    ..super::fixed(80.0, 50.0)
                },
                None,
                vec![NodeId::from(0usize), NodeId::from(1usize)],
            ),
        ]);
        taffy::compute_root_layout(&mut tree, NodeId::from(2usize), Size::MAX_CONTENT);
        let first = tree.0[0].layout.location.y + 12.0;
        let second = tree.0[1].layout.location.y + 30.0;
        super::near(first, second);
        super::near(tree.0[0].layout.location.y, 18.0);
    }
}
