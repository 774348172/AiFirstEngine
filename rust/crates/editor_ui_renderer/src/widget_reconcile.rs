use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    EditorWidgetDeclaration, EditorWidgetNode, EditorWidgetTree, UiRect, WidgetId,
    WidgetLocalState, WidgetTreeError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileDiagnostic {
    pub code: String,
    pub widget_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub reused: usize,
    pub created: usize,
    pub removed: Vec<WidgetId>,
    pub replaced: usize,
    pub reordered: usize,
    pub diagnostics: Vec<ReconcileDiagnostic>,
}

pub fn reconcile_widget_tree(
    previous: Option<&EditorWidgetTree>,
    declaration: &EditorWidgetDeclaration,
) -> Result<(EditorWidgetTree, ReconcileReport), WidgetTreeError> {
    let mut report = ReconcileReport::default();
    let mut nodes = BTreeMap::new();
    let mut declared = BTreeSet::new();
    build_node(
        previous,
        declaration,
        None,
        &mut nodes,
        &mut declared,
        &mut report,
    )?;
    if let Some(previous) = previous {
        report.removed = previous
            .nodes
            .keys()
            .filter(|id| !declared.contains(*id))
            .cloned()
            .collect();
    }
    let tree = EditorWidgetTree {
        root: declaration.id.clone(),
        nodes,
    };
    tree.validate()?;
    Ok((tree, report))
}

fn build_node(
    previous: Option<&EditorWidgetTree>,
    declaration: &EditorWidgetDeclaration,
    parent: Option<WidgetId>,
    nodes: &mut BTreeMap<WidgetId, EditorWidgetNode>,
    declared: &mut BTreeSet<WidgetId>,
    report: &mut ReconcileReport,
) -> Result<(), WidgetTreeError> {
    if !declared.insert(declaration.id.clone()) {
        return Err(WidgetTreeError::DuplicateId(declaration.id.clone()));
    }
    let prior = previous.and_then(|tree| tree.nodes.get(&declaration.id));
    let local_state = match prior {
        Some(node) if node.role == declaration.role => {
            report.reused += 1;
            node.local_state.clone()
        }
        Some(_) => {
            report.replaced += 1;
            report.diagnostics.push(ReconcileDiagnostic {
                code: "widget_reconcile.incompatible_role_replaced".into(),
                widget_id: declaration.id.as_str().into(),
            });
            WidgetLocalState::default()
        }
        None => {
            report.created += 1;
            WidgetLocalState::default()
        }
    };
    let children: Vec<_> = declaration
        .children
        .iter()
        .map(|child| child.id.clone())
        .collect();
    if let Some(prior) = prior {
        if prior.children != children {
            report.reordered += 1;
        }
    }
    nodes.insert(
        declaration.id.clone(),
        EditorWidgetNode {
            id: declaration.id.clone(),
            role: declaration.role,
            parent: parent.clone(),
            children,
            style: declaration.style.clone(),
            visibility: declaration.visibility,
            enabled: declaration.enabled,
            control_classes: declaration.control_classes.clone(),
            model_pseudo_states: declaration.model_pseudo_states,
            activation_policy: declaration.activation_policy,
            binding: declaration.binding.clone(),
            hit_region_id: declaration.hit_region_id.clone(),
            paint: declaration.paint.clone(),
            local_state,
            logical_rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            effective_clip: None,
            resolved_z: 0,
        },
    );
    for child in &declaration.children {
        build_node(
            previous,
            child,
            Some(declaration.id.clone()),
            nodes,
            declared,
            report,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetRole;

    fn declaration(children: &[&str]) -> EditorWidgetDeclaration {
        let mut root =
            EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
        root.children = children
            .iter()
            .map(|id| {
                EditorWidgetDeclaration::new(WidgetId::semantic(*id).unwrap(), WidgetRole::Button)
            })
            .collect();
        root
    }

    #[test]
    fn widget_reconcile_preserves_keyed_state_across_reorder() {
        let (mut first, _) = reconcile_widget_tree(None, &declaration(&["a", "b"])).unwrap();
        first
            .node_mut(&WidgetId::semantic("a").unwrap())
            .unwrap()
            .local_state
            .expanded = true;
        let (second, report) =
            reconcile_widget_tree(Some(&first), &declaration(&["b", "a"])).unwrap();
        assert!(
            second
                .node(&WidgetId::semantic("a").unwrap())
                .unwrap()
                .local_state
                .expanded
        );
        assert_eq!(report.reused, 3);
        assert_eq!(report.reordered, 1);
    }

    #[test]
    fn widget_dynamic_list_reports_removed_ids() {
        let (first, _) = reconcile_widget_tree(None, &declaration(&["a", "b"])).unwrap();
        let (_, report) = reconcile_widget_tree(Some(&first), &declaration(&["b"])).unwrap();
        assert_eq!(report.removed, vec![WidgetId::semantic("a").unwrap()]);
    }
}
