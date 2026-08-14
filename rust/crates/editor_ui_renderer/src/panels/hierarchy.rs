use editor_ui_model::{EditorUiModel, HierarchyModel, HierarchyNode, HierarchySourceDomain};

use crate::layout::command_width;
use crate::metrics::EditorUiMetrics;
use crate::{
    DrawCommand, EditorCommandBinding, EditorWidgetAction, EditorWidgetDeclaration, HitTarget,
    UiColor, UiDrawList, UiRect, WidgetId, WidgetRole,
};

pub(crate) fn push_hierarchy_actions(
    list: &mut UiDrawList,
    panel: UiRect,
    model: &EditorUiModel,
) -> Vec<EditorWidgetDeclaration> {
    let actions = [
        ("create_scene_entity", "+"),
        ("rename_scene_entity", "Rename"),
        ("delete_scene_entity", "Delete"),
    ];
    let mut x = panel.x + 8.0;
    let mut declarations = Vec::new();
    let is_authoring_scene = model.hierarchy.source_domain == HierarchySourceDomain::AuthoringScene;
    for (action_id, label) in actions {
        let enabled = is_authoring_scene
            && (action_id == "create_scene_entity" || model.hierarchy.selected_entity_id.is_some());
        let width = command_width(label);
        let rect = UiRect {
            x,
            y: panel.y + EditorUiMetrics::PANEL_HEADER_HEIGHT + 4.0,
            width,
            height: EditorUiMetrics::COMPACT_CONTROL_HEIGHT - 4.0,
        };
        list.commands.push(DrawCommand::Rect {
            rect,
            color: if enabled {
                UiColor::PANEL_LIGHT
            } else {
                UiColor::PANEL_DARK
            },
            corner_radius: 3.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + 6.0,
                y: rect.y + 5.0,
                width: rect.width - 12.0,
                height: 12.0,
            },
            text: label.to_string(),
            color: if enabled {
                UiColor::TEXT
            } else {
                UiColor::TEXT_MUTED
            },
            size: 10.0,
        });
        let target_action_id = if action_id == "create_scene_entity" {
            action_id.to_string()
        } else if let Some(entity_id) = &model.hierarchy.selected_entity_id {
            format!("{action_id}:{entity_id}")
        } else {
            action_id.to_string()
        };
        declarations.push(
            EditorWidgetDeclaration::new(
                WidgetId::semantic(format!("editor/panel/hierarchy/action/{action_id}"))
                    .expect("static hierarchy action id"),
                WidgetRole::Button,
            )
            .with_absolute_rect(rect, 80_000 + declarations.len() as i32)
            .with_interaction(
                format!("hit.hierarchy_action.{action_id}"),
                enabled,
                EditorCommandBinding {
                    action: EditorWidgetAction::Activate,
                    command_id: action_id.to_string(),
                    target: HitTarget::HierarchyAction {
                        action_id: target_action_id,
                    },
                    reason_disabled: (!enabled).then(|| {
                        if is_authoring_scene {
                            "Select an entity first.".to_string()
                        } else {
                            "Scene editing actions are disabled while Hierarchy shows runtime data."
                                .to_string()
                        }
                    }),
                },
            ),
        );
        x += width + 6.0;
    }
    declarations
}

pub(crate) fn push_hierarchy(
    list: &mut UiDrawList,
    panel: UiRect,
    hierarchy: &HierarchyModel,
    depth: usize,
    declarations: &mut Vec<EditorWidgetDeclaration>,
) -> usize {
    let mut row_index = 0;
    let select_command_id =
        if hierarchy.source_domain == HierarchySourceDomain::ActiveGameViewRuntime {
            "select_runtime_entity"
        } else {
            "select_scene_entity"
        };
    let mut context = HierarchyRenderContext {
        list,
        panel,
        selected: &hierarchy.selected_entity_id,
        select_command_id,
        declarations,
    };
    for node in &hierarchy.roots {
        row_index += push_hierarchy_node(&mut context, node, depth, row_index);
    }
    row_index
}

struct HierarchyRenderContext<'a> {
    list: &'a mut UiDrawList,
    panel: UiRect,
    selected: &'a Option<String>,
    select_command_id: &'a str,
    declarations: &'a mut Vec<EditorWidgetDeclaration>,
}

fn push_hierarchy_node(
    context: &mut HierarchyRenderContext<'_>,
    node: &HierarchyNode,
    depth: usize,
    row_index: usize,
) -> usize {
    let row_h = EditorUiMetrics::LIST_ROW_HEIGHT;
    let y =
        context.panel.y + EditorUiMetrics::PANEL_HEADER_HEIGHT + 36.0 + row_index as f32 * row_h;
    let row = UiRect {
        x: context.panel.x + 4.0,
        y,
        width: context.panel.width - 8.0,
        height: row_h,
    };
    let is_selected = context.selected.as_deref() == Some(node.entity_id.as_str());
    if is_selected {
        context.list.commands.push(DrawCommand::Rect {
            rect: row,
            color: UiColor::ACCENT,
            corner_radius: 0.0,
        });
    }
    context.list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: row.x + 8.0 + depth as f32 * 14.0,
            y: row.y + 5.0,
            width: row.width - 16.0,
            height: 16.0,
        },
        text: node.label.clone(),
        color: if node.alive {
            UiColor::TEXT
        } else {
            UiColor::TEXT_MUTED
        },
        size: 11.0,
    });
    context.declarations.push(
        EditorWidgetDeclaration::new(
            WidgetId::semantic(format!("editor/panel/hierarchy/entity/{}", node.entity_id))
                .expect("entity id must be semantic"),
            WidgetRole::Button,
        )
        .with_absolute_rect(row, 70_000 + row_index as i32)
        .with_interaction(
            format!("hit.hierarchy.{}", node.entity_id),
            node.alive,
            EditorCommandBinding {
                action: EditorWidgetAction::Activate,
                command_id: context.select_command_id.to_string(),
                target: HitTarget::HierarchyEntity {
                    entity_id: node.entity_id.clone(),
                },
                reason_disabled: (!node.alive).then(|| "Entity is disabled.".to_string()),
            },
        ),
    );

    let mut consumed = 1;
    for child in &node.children {
        consumed += push_hierarchy_node(context, child, depth + 1, row_index + consumed);
    }
    consumed
}
