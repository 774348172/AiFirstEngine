use serde::{Deserialize, Serialize};

use crate::{EditorWidgetTree, UiPoint, WidgetId, WidgetVisibility};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetPath(pub Vec<WidgetId>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickBlockReason {
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetPickResult {
    pub target: WidgetId,
    pub path: WidgetPath,
    pub blocked: Option<PickBlockReason>,
}

pub fn pick_widget(
    tree: &EditorWidgetTree,
    point: UiPoint,
    pointer_capture: Option<&WidgetId>,
) -> Option<WidgetPickResult> {
    let target = if let Some(capture) = pointer_capture {
        tree.node(capture).map(|_| capture.clone())
    } else {
        tree.nodes
            .values()
            .filter(|node| {
                node.visibility == WidgetVisibility::Visible
                    && node.logical_rect.contains(point)
                    && node.effective_clip.is_none_or(|clip| clip.contains(point))
            })
            .max_by_key(|node| node.resolved_z)
            .map(|node| node.id.clone())
    }?;
    let node = tree.node(&target)?;
    let blocked = (!node.enabled).then_some(PickBlockReason::Disabled);
    let mut ids = vec![target.clone()];
    let mut parent = node.parent.as_ref();
    while let Some(id) = parent {
        ids.push(id.clone());
        parent = tree.node(id).and_then(|node| node.parent.as_ref());
    }
    ids.reverse();
    Some(WidgetPickResult {
        target,
        path: WidgetPath(ids),
        blocked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{reconcile_widget_tree, EditorWidgetDeclaration, WidgetRole};

    #[test]
    fn widget_pick_disabled_top_target_does_not_pass_through() {
        let mut root =
            EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
        let mut bottom =
            EditorWidgetDeclaration::new(WidgetId::semantic("bottom").unwrap(), WidgetRole::Button);
        bottom.style.absolute = true;
        bottom.style.width = Some(100.0);
        bottom.style.height = Some(100.0);
        let mut top =
            EditorWidgetDeclaration::new(WidgetId::semantic("top").unwrap(), WidgetRole::Button);
        top.style.absolute = true;
        top.style.width = Some(100.0);
        top.style.height = Some(100.0);
        top.style.z_index = 1;
        top.enabled = false;
        root.children = vec![bottom, top];
        let (mut tree, _) = reconcile_widget_tree(None, &root).unwrap();
        crate::layout_widget_tree(
            &mut tree,
            100.0,
            100.0,
            &mut |_: &WidgetId, _: Option<f32>| (0.0, 0.0),
        )
        .unwrap();
        let result = pick_widget(&tree, UiPoint { x: 10.0, y: 10.0 }, None).unwrap();
        assert_eq!(result.target.as_str(), "top");
        assert_eq!(result.blocked, Some(PickBlockReason::Disabled));
        assert_eq!(result.path.0.len(), 2);
    }
}
