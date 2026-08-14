use editor_ui_model::EditorUiModel;

use crate::layout::push_border;
use crate::panels::{widget_interaction, WidgetInteractionSpec};
use crate::{
    DrawCommand, EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect, WidgetRole,
};

pub(crate) fn push_console_entries(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let console = UiRect {
        x: rect.x + 1.0,
        y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
        width: rect.width * 0.56,
        height: rect.height - 25.0,
    };
    push_border(list, console);
    let mut y = console.y + 8.0;
    for (index, entry) in model.console.entries.iter().take(6).enumerate() {
        let row = UiRect {
            x: console.x + 8.0,
            y,
            width: console.width - 16.0,
            height: 22.0,
        };
        let color = match entry.level {
            editor_ui_model::ConsoleLevel::Info => UiColor::TEXT,
            editor_ui_model::ConsoleLevel::Warning => UiColor::WARNING,
            editor_ui_model::ConsoleLevel::Error => UiColor::ERROR,
        };
        list.commands.push(DrawCommand::Text {
            rect: row,
            text: entry.message.clone(),
            color,
            size: 11.0,
        });
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.console.{}.{}", entry.entry_id, index),
            rect: row,
            role: WidgetRole::Button,
            target: HitTarget::ConsoleEntry {
                entry_id: entry.entry_id.clone(),
            },
            enabled: true,
            command_id: "select_console_entry".to_string(),
            reason_disabled: None,
        }));
        y += 22.0;
    }
    interactions
}
