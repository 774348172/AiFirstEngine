use crate::metrics::EditorUiMetrics;
use crate::{DrawCommand, UiColor, UiDrawList, UiRect};

#[derive(Debug, Clone, Copy)]
pub(crate) struct EditorShellLayout {
    pub(crate) menu_bar: UiRect,
    pub(crate) toolbar: UiRect,
    pub(crate) workspace: UiRect,
}

impl EditorShellLayout {
    pub(crate) fn resolve(width: f32, height: f32) -> Self {
        let width = finite_non_negative(width);
        let height = finite_non_negative(height);
        let menu_h = EditorUiMetrics::MENU_BAR_HEIGHT;
        let toolbar_h = EditorUiMetrics::TOOLBAR_HEIGHT;
        let top_h = menu_h + toolbar_h;
        Self {
            menu_bar: UiRect {
                x: 0.0,
                y: 0.0,
                width,
                height: menu_h.min(height),
            },
            toolbar: UiRect {
                x: 0.0,
                y: menu_h.min(height),
                width,
                height: toolbar_h.min((height - menu_h).max(0.0)),
            },
            workspace: UiRect {
                x: 0.0,
                y: top_h.min(height),
                width,
                height: (height - top_h).max(0.0),
            },
        }
    }
}

pub fn editor_workspace_rect(width: f32, height: f32) -> UiRect {
    EditorShellLayout::resolve(width, height).workspace
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

pub(crate) fn push_border(list: &mut UiDrawList, rect: UiRect) {
    let color = UiColor::BORDER;
    let lines = [
        UiRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: 1.0,
        },
        UiRect {
            x: rect.x,
            y: rect.y + rect.height - 1.0,
            width: rect.width,
            height: 1.0,
        },
        UiRect {
            x: rect.x,
            y: rect.y,
            width: 1.0,
            height: rect.height,
        },
        UiRect {
            x: rect.x + rect.width - 1.0,
            y: rect.y,
            width: 1.0,
            height: rect.height,
        },
    ];
    for line in lines {
        list.commands.push(DrawCommand::Rect {
            rect: line,
            color,
            corner_radius: 0.0,
        });
    }
}

pub(crate) fn content_rect(rect: UiRect) -> UiRect {
    let content_top = EditorUiMetrics::PANEL_HEADER_HEIGHT + 27.0;
    UiRect {
        x: rect.x + 2.0,
        y: rect.y + content_top,
        width: rect.width - 4.0,
        height: rect.height - content_top - 2.0,
    }
}

pub(crate) fn command_width(label: &str) -> f32 {
    (label.chars().count() as f32 * 9.5 + 24.0).clamp(62.0, 220.0)
}
