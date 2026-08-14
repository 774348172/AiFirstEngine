mod ai_panel;
mod build_export;
mod console;
mod hierarchy;
mod inspector;
mod launcher;
mod project_browser;
mod project_intent;
mod project_runtime_trust;
mod runtime_trace;
mod toolbar;
mod viewport;
mod workspace;

pub(crate) use ai_panel::push_ai_panel;
pub(crate) use build_export::push_build_export_panel;
pub(crate) use console::push_console_entries;
pub(crate) use hierarchy::{push_hierarchy, push_hierarchy_actions};
pub(crate) use inspector::push_inspector_fields;
pub(crate) use launcher::push_project_launcher;
pub(crate) use project_browser::push_project_browser;
pub(crate) use project_intent::push_project_intent_panel;
pub(crate) use project_runtime_trust::push_project_runtime_trust_prompt;
pub(crate) use runtime_trace::push_runtime_trace_entries;
pub(crate) use toolbar::push_toolbar;
pub(crate) use viewport::push_viewport_header;
pub(crate) use workspace::push_workspace_summary_panel;

use crate::layout::push_border;
use crate::metrics::EditorUiMetrics;
use crate::{
    dark_neutral_control_style, paint_control_brush, ControlPseudoStateSet, ControlStyleQuery,
    DrawCommand, EditorCommandBinding, EditorWidgetAction, EditorWidgetDeclaration, HitTarget,
    ResolvedControlStyle, UiColor, UiDrawList, UiRect, WidgetId, WidgetRole,
};

pub(crate) fn resolve_and_paint_control(
    list: &mut UiDrawList,
    rect: UiRect,
    role: WidgetRole,
    class: &str,
    pseudo_states: ControlPseudoStateSet,
) -> ResolvedControlStyle {
    let style = dark_neutral_control_style(&ControlStyleQuery::new(role, [class], pseudo_states));
    let output = paint_control_brush(rect, &style.background, style.opacity);
    for mut command in output.commands {
        if let DrawCommand::Rect { corner_radius, .. } = &mut command {
            *corner_radius = style.border.corner_radius;
        }
        list.commands.push(command);
    }
    push_control_border(list, rect, &style);
    style
}

fn push_control_border(list: &mut UiDrawList, rect: UiRect, style: &ResolvedControlStyle) {
    let width = style
        .border
        .width
        .min(rect.width * 0.5)
        .min(rect.height * 0.5);
    if width <= 0.0 {
        return;
    }
    for edge in [
        UiRect {
            width: rect.width,
            height: width,
            ..rect
        },
        UiRect {
            y: rect.y + rect.height - width,
            width: rect.width,
            height: width,
            ..rect
        },
        UiRect {
            width,
            height: rect.height,
            ..rect
        },
        UiRect {
            x: rect.x + rect.width - width,
            width,
            height: rect.height,
            ..rect
        },
    ] {
        list.commands.push(DrawCommand::Rect {
            rect: edge,
            color: style.border.color,
            corner_radius: 0.0,
        });
    }
}

pub(crate) struct WidgetInteractionSpec {
    pub id: String,
    pub rect: UiRect,
    pub role: WidgetRole,
    pub target: HitTarget,
    pub enabled: bool,
    pub command_id: String,
    pub reason_disabled: Option<String>,
}

pub(crate) fn widget_interaction(spec: WidgetInteractionSpec) -> EditorWidgetDeclaration {
    let widget_id = WidgetId::semantic(format!("editor/control/{}", spec.id))
        .or_else(|_| WidgetId::scoped("editor/control", &spec.id))
        .expect("editor interaction id");
    EditorWidgetDeclaration::new(widget_id, spec.role)
        .with_absolute_rect(spec.rect, 80_000)
        .with_interaction(
            spec.id,
            spec.enabled,
            EditorCommandBinding {
                action: if spec.role == WidgetRole::TextInput {
                    EditorWidgetAction::Focus
                } else {
                    EditorWidgetAction::Activate
                },
                command_id: spec.command_id,
                target: spec.target,
                reason_disabled: spec.reason_disabled,
            },
        )
}

pub(crate) fn push_workspace_tabs(
    list: &mut UiDrawList,
    rect: UiRect,
    stack_id: &str,
    tabs: &[(String, String)],
    active_panel_id: &str,
    config: &crate::UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    let rect = UiRect {
        height: rect.height.min(EditorUiMetrics::PANEL_HEADER_HEIGHT),
        ..rect
    };
    list.commands.push(DrawCommand::Rect {
        rect,
        color: UiColor::PANEL,
        corner_radius: 0.0,
    });
    push_border(list, rect);
    let mut offset = 0.0;
    let mut declarations = Vec::new();
    let tab_width_limit = (rect.width / tabs.len() as f32).max(1.0);
    for (index, (panel_id, label)) in tabs.iter().enumerate() {
        let preferred_width = (label.chars().count() as f32 * 9.5 + 28.0).clamp(58.0, 145.0);
        let width = preferred_width.min(tab_width_limit);
        let tab = UiRect {
            x: rect.x + offset,
            y: rect.y,
            width,
            height: EditorUiMetrics::PANEL_HEADER_HEIGHT,
        };
        let hit_id = if stack_id == "workspace/bottom" {
            format!("hit.dock_tab.{panel_id}")
        } else {
            format!("hit.dock_tab.{stack_id}.{panel_id}")
        };
        let model_pseudo = ControlPseudoStateSet::empty().with(
            crate::ControlPseudoState::Selected,
            panel_id.as_str() == active_panel_id,
        );
        let style = resolve_and_paint_control(
            list,
            tab,
            WidgetRole::Tab,
            "workspace-tab",
            config.control_pseudo_states(&hit_id, model_pseudo, true),
        );
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: tab.x + 8.0 + style.content_offset.x,
                y: tab.y + 5.0 + style.content_offset.y,
                width: tab.width - 16.0,
                height: 16.0,
            },
            text: label.clone(),
            color: style.foreground,
            size: 11.0,
        });
        let mut declaration = EditorWidgetDeclaration::new(
            WidgetId::semantic(if stack_id == "workspace/bottom" {
                format!("editor/dock/bottom-tabs/{panel_id}")
            } else {
                format!("editor/workspace/stack/{stack_id}/tabs/{panel_id}")
            })
            .expect("workspace tab id"),
            WidgetRole::Tab,
        );
        declaration.style.absolute = true;
        declaration.style.inset_left = Some(tab.x);
        declaration.style.inset_top = Some(tab.y);
        declaration.style.width = Some(tab.width);
        declaration.style.height = Some(tab.height);
        declaration.style.z_index = 100_000 + index as i32;
        declaration.hit_region_id = Some(hit_id);
        declaration.binding = Some(EditorCommandBinding {
            action: EditorWidgetAction::Activate,
            command_id: "activate_dock_tab".to_string(),
            target: HitTarget::DockTab {
                panel_id: panel_id.to_string(),
            },
            reason_disabled: None,
        });
        declaration.control_classes = crate::ControlClassSet::new(["workspace-tab"]);
        declaration.model_pseudo_states = model_pseudo;
        declaration.activation_policy = crate::ActivationPolicy::Press;
        declarations.push(declaration);
        offset += width;
    }
    declarations
}

pub(crate) struct WorkspacePanelChromeSpec<'a> {
    pub stack_rect: UiRect,
    pub stack_id: &'a str,
    pub panel_id: &'a str,
    pub lock_available: bool,
    pub locked: bool,
    pub closable: bool,
    pub popup_open: bool,
}

pub(crate) fn push_workspace_panel_chrome(
    list: &mut UiDrawList,
    spec: WorkspacePanelChromeSpec<'_>,
    config: &crate::UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    let WorkspacePanelChromeSpec {
        stack_rect,
        stack_id,
        panel_id,
        lock_available,
        locked,
        closable,
        popup_open,
    } = spec;
    const BUTTON: f32 = EditorUiMetrics::PANEL_HEADER_HEIGHT;
    let lock_rect = UiRect {
        x: stack_rect.x + stack_rect.width - BUTTON * 2.0,
        y: stack_rect.y,
        width: BUTTON,
        height: BUTTON,
    };
    let more_rect = UiRect {
        x: stack_rect.x + stack_rect.width - BUTTON,
        ..lock_rect
    };
    let lock_hit_id = format!("workspace.panel_lock.{stack_id}.{panel_id}");
    let lock_model =
        ControlPseudoStateSet::empty().with(crate::ControlPseudoState::Selected, locked);
    let lock_style = resolve_and_paint_control(
        list,
        lock_rect,
        WidgetRole::Button,
        "panel-chrome-control",
        config.control_pseudo_states(&lock_hit_id, lock_model, lock_available || locked),
    );
    let more_hit_id = format!("workspace.panel_more.{stack_id}.{panel_id}");
    let more_model =
        ControlPseudoStateSet::empty().with(crate::ControlPseudoState::Selected, popup_open);
    let more_style = resolve_and_paint_control(
        list,
        more_rect,
        WidgetRole::Button,
        "panel-chrome-control",
        config.control_pseudo_states(&more_hit_id, more_model, true),
    );
    push_lock_icon(list, lock_rect, locked, lock_style.foreground);
    push_more_icon(list, more_rect, more_style.foreground);
    let mut widgets = vec![
        widget_interaction(WidgetInteractionSpec {
            id: format!("workspace.panel_lock.{stack_id}.{panel_id}"),
            rect: lock_rect,
            role: WidgetRole::Button,
            target: HitTarget::WorkspacePanelLock {
                stack_id: stack_id.to_string(),
                panel_id: panel_id.to_string(),
                locked,
            },
            enabled: lock_available || locked,
            command_id: "toggle_inspector_context_lock".to_string(),
            reason_disabled: (!lock_available && !locked).then(|| {
                "Context Lock is only available for an Inspector entity context.".to_string()
            }),
        }),
        widget_interaction(WidgetInteractionSpec {
            id: format!("workspace.panel_more.{stack_id}.{panel_id}"),
            rect: more_rect,
            role: WidgetRole::Button,
            target: HitTarget::WorkspacePanelMore {
                stack_id: stack_id.to_string(),
                panel_id: panel_id.to_string(),
            },
            enabled: true,
            command_id: "open_workspace_panel_menu".to_string(),
            reason_disabled: None,
        }),
    ];
    for (widget, model) in widgets.iter_mut().zip([lock_model, more_model]) {
        widget.control_classes = crate::ControlClassSet::new(["panel-chrome-control"]);
        widget.model_pseudo_states = model;
        widget.activation_policy = crate::ActivationPolicy::ReleaseInside;
    }
    if popup_open {
        let popup = UiRect {
            x: (more_rect.x + more_rect.width - 128.0).max(stack_rect.x),
            y: more_rect.y + more_rect.height,
            width: 128.0,
            height: EditorUiMetrics::POPUP_ROW_HEIGHT,
        };
        list.commands.push(DrawCommand::Rect {
            rect: popup,
            color: UiColor::PANEL,
            corner_radius: 0.0,
        });
        push_border(list, popup);
        let close_hit_id = format!("workspace.panel_close.{stack_id}.{panel_id}");
        let close_style = resolve_and_paint_control(
            list,
            popup,
            WidgetRole::Button,
            "panel-chrome-control",
            config.control_pseudo_states(&close_hit_id, ControlPseudoStateSet::empty(), closable),
        );
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: popup.x + 8.0,
                y: popup.y + 6.0,
                width: popup.width - 16.0,
                height: 16.0,
            },
            text: "Close Tab".to_string(),
            color: close_style.foreground,
            size: 11.0,
        });
        let mut close = widget_interaction(WidgetInteractionSpec {
            id: close_hit_id,
            rect: popup,
            role: WidgetRole::Button,
            target: HitTarget::WorkspacePanelClose {
                stack_id: stack_id.to_string(),
                panel_id: panel_id.to_string(),
            },
            enabled: closable,
            command_id: "close_workspace_panel".to_string(),
            reason_disabled: (!closable).then(|| "This panel cannot be closed.".to_string()),
        });
        close.control_classes = crate::ControlClassSet::new(["panel-chrome-control"]);
        close.activation_policy = crate::ActivationPolicy::ReleaseInside;
        widgets.push(close);
    }
    widgets
}

fn push_lock_icon(list: &mut UiDrawList, rect: UiRect, locked: bool, color: UiColor) {
    push_icon_rect(list, rect.x + 8.0, rect.y + 11.0, 8.0, 7.0, color);
    push_icon_rect(list, rect.x + 10.0, rect.y + 6.0, 5.0, 2.0, color);
    push_icon_rect(list, rect.x + 14.0, rect.y + 7.0, 2.0, 4.0, color);
    if locked {
        push_icon_rect(list, rect.x + 8.0, rect.y + 7.0, 2.0, 4.0, color);
    }
}

fn push_more_icon(list: &mut UiDrawList, rect: UiRect, color: UiColor) {
    for y in [rect.y + 6.0, rect.y + 11.0, rect.y + 16.0] {
        push_icon_rect(list, rect.x + 11.0, y, 2.0, 2.0, color);
    }
}

fn push_icon_rect(list: &mut UiDrawList, x: f32, y: f32, width: f32, height: f32, color: UiColor) {
    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            x,
            y,
            width,
            height,
        },
        color,
        corner_radius: 0.0,
    });
}
