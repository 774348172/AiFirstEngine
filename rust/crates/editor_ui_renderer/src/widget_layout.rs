use std::collections::BTreeMap;

use taffy::prelude::*;

use crate::{
    EditorWidgetLayoutStyle, EditorWidgetTree, UiRect, WidgetDirection, WidgetId, WidgetRole,
    WidgetVisibility,
};

pub trait TextMeasure {
    fn measure(&mut self, widget_id: &WidgetId, known_width: Option<f32>) -> (f32, f32);
}

impl<F> TextMeasure for F
where
    F: FnMut(&WidgetId, Option<f32>) -> (f32, f32),
{
    fn measure(&mut self, widget_id: &WidgetId, known_width: Option<f32>) -> (f32, f32) {
        self(widget_id, known_width)
    }
}

#[derive(Debug)]
pub enum WidgetLayoutError {
    Tree(String),
    Taffy(String),
}

impl std::fmt::Display for WidgetLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for WidgetLayoutError {}

pub fn layout_widget_tree(
    tree: &mut EditorWidgetTree,
    width: f32,
    height: f32,
    measure: &mut dyn TextMeasure,
) -> Result<(), WidgetLayoutError> {
    tree.validate()
        .map_err(|error| WidgetLayoutError::Tree(error.to_string()))?;
    let mut taffy = TaffyTree::<WidgetId>::new();
    let mut handles = BTreeMap::new();
    create_taffy_nodes(tree, &tree.root.clone(), &mut taffy, &mut handles)?;
    let root = handles[&tree.root];
    let mut root_style = taffy
        .style(root)
        .map_err(|error| WidgetLayoutError::Taffy(error.to_string()))?
        .clone();
    root_style.size = Size {
        width: Dimension::length(width),
        height: Dimension::length(height),
    };
    taffy
        .set_style(root, root_style)
        .map_err(|error| WidgetLayoutError::Taffy(error.to_string()))?;
    taffy
        .compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::Definite(height),
            },
            |known, available, _node_id, context, _style| {
                let Some(widget_id) = context else {
                    return Size::ZERO;
                };
                let known_width = known.width.or(match available.width {
                    AvailableSpace::Definite(value) => Some(value),
                    _ => None,
                });
                let (measured_width, measured_height) = measure.measure(widget_id, known_width);
                Size {
                    width: known.width.unwrap_or(measured_width),
                    height: known.height.unwrap_or(measured_height),
                }
            },
        )
        .map_err(|error| WidgetLayoutError::Taffy(error.to_string()))?;
    assign_geometry(
        tree,
        &tree.root.clone(),
        &taffy,
        &handles,
        GeometryParent::default(),
    )?;
    Ok(())
}

fn create_taffy_nodes(
    tree: &EditorWidgetTree,
    id: &WidgetId,
    taffy: &mut TaffyTree<WidgetId>,
    handles: &mut BTreeMap<WidgetId, NodeId>,
) -> Result<NodeId, WidgetLayoutError> {
    let node = &tree.nodes[id];
    let mut child_handles = Vec::new();
    for child in &node.children {
        child_handles.push(create_taffy_nodes(tree, child, taffy, handles)?);
    }
    let style = to_taffy_style(&node.style, node.visibility);
    let handle = if node.role == WidgetRole::Label {
        taffy.new_leaf_with_context(style, id.clone())
    } else {
        taffy.new_with_children(style, &child_handles)
    }
    .map_err(|error| WidgetLayoutError::Taffy(error.to_string()))?;
    handles.insert(id.clone(), handle);
    Ok(handle)
}

fn length(value: Option<f32>) -> Dimension {
    value.map(Dimension::length).unwrap_or(Dimension::auto())
}

fn inset(value: Option<f32>) -> LengthPercentageAuto {
    value
        .map(LengthPercentageAuto::length)
        .unwrap_or(LengthPercentageAuto::auto())
}
fn to_taffy_style(style: &EditorWidgetLayoutStyle, visibility: WidgetVisibility) -> Style {
    Style {
        display: if visibility == WidgetVisibility::Collapsed {
            Display::None
        } else {
            Display::Flex
        },
        position: if style.absolute {
            Position::Absolute
        } else {
            Position::Relative
        },
        inset: Rect {
            left: inset(style.inset_left),
            right: LengthPercentageAuto::auto(),
            top: inset(style.inset_top),
            bottom: LengthPercentageAuto::auto(),
        },
        size: Size {
            width: length(style.width),
            height: length(style.height),
        },
        min_size: Size {
            width: length(style.min_width),
            height: length(style.min_height),
        },
        max_size: Size {
            width: length(style.max_width),
            height: length(style.max_height),
        },
        flex_direction: match style.direction {
            WidgetDirection::Row => FlexDirection::Row,
            WidgetDirection::Column => FlexDirection::Column,
        },
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        gap: Size {
            width: LengthPercentage::length(style.gap),
            height: LengthPercentage::length(style.gap),
        },
        ..Style::default()
    }
}

#[derive(Clone, Copy, Default)]
struct GeometryParent {
    x: f32,
    y: f32,
    clip: Option<UiRect>,
    z: i64,
}

fn assign_geometry(
    tree: &mut EditorWidgetTree,
    id: &WidgetId,
    taffy: &TaffyTree<WidgetId>,
    handles: &BTreeMap<WidgetId, NodeId>,
    parent: GeometryParent,
) -> Result<(), WidgetLayoutError> {
    let layout = taffy
        .layout(handles[id])
        .map_err(|error| WidgetLayoutError::Taffy(error.to_string()))?;
    let rect = UiRect {
        x: parent.x + layout.location.x,
        y: parent.y + layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    };
    let (children, own_clip, z, scroll_x, scroll_y) = {
        let node = tree.nodes.get_mut(id).expect("validated node");
        node.logical_rect = rect;
        node.resolved_z = parent.z + i64::from(node.style.z_index);
        let clip = if node.style.clip {
            intersect_clip(parent.clip, rect)
        } else {
            parent.clip
        };
        node.effective_clip = clip;
        let (scroll_x, scroll_y) = if node.role == WidgetRole::Scroll {
            (node.local_state.scroll_x, node.local_state.scroll_y)
        } else {
            (0.0, 0.0)
        };
        (
            node.children.clone(),
            clip,
            node.resolved_z,
            scroll_x,
            scroll_y,
        )
    };
    for (index, child) in children.iter().enumerate() {
        assign_geometry(
            tree,
            child,
            taffy,
            handles,
            GeometryParent {
                x: rect.x - scroll_x,
                y: rect.y - scroll_y,
                clip: own_clip,
                z: z * 1_000 + index as i64,
            },
        )?;
    }
    Ok(())
}

fn intersect_clip(parent: Option<UiRect>, own: UiRect) -> Option<UiRect> {
    let Some(parent) = parent else {
        return Some(own);
    };
    let x = parent.x.max(own.x);
    let y = parent.y.max(own.y);
    let right = (parent.x + parent.width).min(own.x + own.width);
    let bottom = (parent.y + parent.height).min(own.y + own.height);
    Some(UiRect {
        x,
        y,
        width: (right - x).max(0.0),
        height: (bottom - y).max(0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{reconcile_widget_tree, EditorWidgetDeclaration, WidgetRole};

    #[test]
    fn widget_layout_flex_and_clip_are_deterministic() {
        let mut root =
            EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
        root.style.direction = WidgetDirection::Row;
        root.style.clip = true;
        for id in ["left", "right"] {
            let mut child =
                EditorWidgetDeclaration::new(WidgetId::semantic(id).unwrap(), WidgetRole::Panel);
            child.style.flex_grow = 1.0;
            root.children.push(child);
        }
        let (mut tree, _) = reconcile_widget_tree(None, &root).unwrap();
        layout_widget_tree(
            &mut tree,
            200.0,
            100.0,
            &mut |_: &WidgetId, _: Option<f32>| (0.0, 0.0),
        )
        .unwrap();
        assert_eq!(
            tree.node(&WidgetId::semantic("left").unwrap())
                .unwrap()
                .logical_rect
                .width,
            100.0
        );
        assert_eq!(
            tree.node(&WidgetId::semantic("right").unwrap())
                .unwrap()
                .logical_rect
                .x,
            100.0
        );
        assert_eq!(
            tree.node(&WidgetId::semantic("right").unwrap())
                .unwrap()
                .effective_clip
                .unwrap()
                .width,
            200.0
        );
    }
}
