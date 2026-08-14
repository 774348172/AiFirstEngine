use editor_ui_model::ToolbarCommand;

use crate::layout::{command_width, push_border};
use crate::metrics::EditorUiMetrics;
use crate::{
    ActivationPolicy, ControlPseudoStateSet, DrawCommand, EditorCommandBinding, EditorWidgetAction,
    EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect, UiRendererConfig, WidgetId,
    WidgetRole,
};

use super::resolve_and_paint_control;

pub(crate) fn push_toolbar(
    list: &mut UiDrawList,
    rect: UiRect,
    commands: &[ToolbarCommand],
    config: &UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    list.commands.push(DrawCommand::Rect {
        rect,
        color: UiColor::TOOLBAR,
        corner_radius: 0.0,
    });
    push_border(list, rect);
    let mut x = rect.x + 8.0;
    let mut declarations = Vec::new();
    let total_width: f32 = commands
        .iter()
        .map(|command| command_width(&command.label) + 6.0)
        .sum();
    let has_overflow = total_width > (rect.width - 16.0).max(0.0);
    let command_right = rect.x + rect.width - 8.0 - if has_overflow { 30.0 } else { 0.0 };
    let mut hidden = Vec::new();
    for command in commands {
        let width = command_width(&command.label);
        if x + width > command_right {
            hidden.push(command);
            continue;
        }
        let button = UiRect {
            x,
            y: rect.y + 4.0,
            width,
            height: EditorUiMetrics::COMPACT_CONTROL_HEIGHT - 4.0,
        };
        let hit_id = format!("hit.toolbar.{}", command.command_id);
        let pseudo =
            config.control_pseudo_states(&hit_id, ControlPseudoStateSet::empty(), command.enabled);
        let style =
            resolve_and_paint_control(list, button, WidgetRole::Button, "toolbar-control", pseudo);
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: button.x + 8.0,
                y: button.y + 4.0 + style.content_offset.y,
                width: button.width - 16.0,
                height: 16.0,
            },
            text: command.label.clone(),
            color: style.foreground,
            size: 11.0,
        });
        let mut declaration = EditorWidgetDeclaration::new(
            WidgetId::semantic(format!("editor/shell/toolbar/{}", command.command_id))
                .expect("toolbar command id must be semantic"),
            WidgetRole::Button,
        );
        declaration.style.absolute = true;
        declaration.style.inset_left = Some(button.x);
        declaration.style.inset_top = Some(button.y);
        declaration.style.width = Some(button.width);
        declaration.style.height = Some(button.height);
        declaration.style.z_index = 90_000 + declarations.len() as i32;
        declaration.enabled = command.enabled;
        declaration.hit_region_id = Some(hit_id);
        declaration.binding = Some(EditorCommandBinding {
            action: EditorWidgetAction::Activate,
            command_id: command.command_id.clone(),
            target: HitTarget::ToolbarCommand {
                command_id: command.command_id.clone(),
            },
            reason_disabled: command.reason_disabled.clone(),
        });
        declaration.control_classes = crate::ControlClassSet::new(["toolbar-control"]);
        declaration.model_pseudo_states = ControlPseudoStateSet::empty();
        declaration.activation_policy = ActivationPolicy::ReleaseInside;
        declarations.push(declaration);
        x += width + 6.0;
    }
    if has_overflow {
        let overflow_rect = UiRect {
            x: rect.x + rect.width - 34.0,
            y: rect.y + 4.0,
            width: EditorUiMetrics::COMPACT_CONTROL_HEIGHT,
            height: EditorUiMetrics::COMPACT_CONTROL_HEIGHT - 4.0,
        };
        let overflow_hit_id = "hit.toolbar.overflow";
        let overflow_model = ControlPseudoStateSet::empty().with(
            crate::ControlPseudoState::Selected,
            config.toolbar_overflow_open,
        );
        let overflow_style = resolve_and_paint_control(
            list,
            overflow_rect,
            WidgetRole::Button,
            "toolbar-control",
            config.control_pseudo_states(overflow_hit_id, overflow_model, true),
        );
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                y: overflow_rect.y + overflow_style.content_offset.y,
                ..overflow_rect
            },
            text: "...".to_string(),
            color: overflow_style.foreground,
            size: 11.0,
        });
        declarations.push(interaction_declaration(ToolbarInteractionSpec {
            widget_id: "editor/shell/toolbar/overflow".to_string(),
            hit_id: "hit.toolbar.overflow".to_string(),
            rect: overflow_rect,
            target: HitTarget::ToolbarOverflow,
            enabled: true,
            command_id: "toggle_toolbar_overflow".to_string(),
            reason_disabled: None,
            z_index: 120_000,
        }));
        if config.toolbar_overflow_open {
            declarations.push(interaction_declaration(ToolbarInteractionSpec {
                widget_id: "editor/shell/toolbar/overflow/barrier".to_string(),
                hit_id: "hit.toolbar.overflow.barrier".to_string(),
                rect: UiRect {
                    x: 0.0,
                    y: 0.0,
                    width: config.width,
                    height: config.height,
                },
                target: HitTarget::ToolbarOverflow,
                enabled: true,
                command_id: "close_toolbar_overflow".to_string(),
                reason_disabled: None,
                z_index: 100_000,
            }));
            let popup_width = 220.0_f32.min(rect.width.max(1.0));
            for (index, command) in hidden.into_iter().enumerate() {
                let popup_rect = UiRect {
                    x: (rect.x + rect.width - popup_width).max(rect.x),
                    y: rect.y
                        + rect.height
                        + index as f32 * EditorUiMetrics::COMPACT_CONTROL_HEIGHT,
                    width: popup_width,
                    height: EditorUiMetrics::COMPACT_CONTROL_HEIGHT,
                };
                let mut declaration = interaction_declaration(ToolbarInteractionSpec {
                    widget_id: format!("editor/shell/toolbar/overflow/{}", command.command_id),
                    hit_id: format!("hit.toolbar.overflow.{}", command.command_id),
                    rect: popup_rect,
                    target: HitTarget::ToolbarCommand {
                        command_id: command.command_id.clone(),
                    },
                    enabled: command.enabled,
                    command_id: command.command_id.clone(),
                    reason_disabled: command.reason_disabled.clone(),
                    z_index: 130_000 + index as i32,
                });
                let hit_id = declaration.hit_region_id.as_deref().unwrap_or_default();
                let style = crate::dark_neutral_control_style(&crate::ControlStyleQuery::new(
                    WidgetRole::Button,
                    ["toolbar-control"],
                    config.control_pseudo_states(
                        hit_id,
                        ControlPseudoStateSet::empty(),
                        command.enabled,
                    ),
                ));
                if let crate::ControlBrush::Solid { color } = style.background {
                    declaration.paint.push(crate::WidgetPaint::Rect {
                        color,
                        corner_radius: style.border.corner_radius,
                    });
                }
                declaration.paint.push(crate::WidgetPaint::Text {
                    text: command.label.clone(),
                    color: style.foreground,
                    size: 11.0,
                });
                declarations.push(declaration);
            }
        }
    }
    declarations
}

struct ToolbarInteractionSpec {
    widget_id: String,
    hit_id: String,
    rect: UiRect,
    target: HitTarget,
    enabled: bool,
    command_id: String,
    reason_disabled: Option<String>,
    z_index: i32,
}

fn interaction_declaration(spec: ToolbarInteractionSpec) -> EditorWidgetDeclaration {
    let mut declaration = EditorWidgetDeclaration::new(
        WidgetId::semantic(spec.widget_id).expect("toolbar widget id must be semantic"),
        WidgetRole::Button,
    );
    declaration.style.absolute = true;
    declaration.style.inset_left = Some(spec.rect.x);
    declaration.style.inset_top = Some(spec.rect.y);
    declaration.style.width = Some(spec.rect.width);
    declaration.style.height = Some(spec.rect.height);
    declaration.style.z_index = spec.z_index;
    declaration.enabled = spec.enabled;
    declaration.hit_region_id = Some(spec.hit_id);
    declaration.binding = Some(EditorCommandBinding {
        action: EditorWidgetAction::Activate,
        command_id: spec.command_id,
        target: spec.target,
        reason_disabled: spec.reason_disabled,
    });
    declaration.control_classes = crate::ControlClassSet::new(["toolbar-control"]);
    declaration.model_pseudo_states = ControlPseudoStateSet::empty();
    declaration.activation_policy = ActivationPolicy::ReleaseInside;
    declaration
}
