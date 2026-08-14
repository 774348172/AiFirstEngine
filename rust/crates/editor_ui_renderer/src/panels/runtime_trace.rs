use editor_ui_model::EditorUiModel;

use crate::layout::push_border;
use crate::panels::{widget_interaction, WidgetInteractionSpec};
use crate::{
    DrawCommand, EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect, WidgetRole,
};

pub(crate) fn push_runtime_trace_entries(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let panel = UiRect {
        x: rect.x + rect.width * 0.56,
        y: rect.y
            + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT
            + (rect.height - crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT - 1.0) * 0.58,
        width: rect.width * 0.18,
        height: (rect.height - 25.0) * 0.42,
    };
    push_border(list, panel);
    let x = panel.x + 8.0;
    let width = panel.width - 16.0;
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x,
            y: panel.y + 8.0,
            width,
            height: 16.0,
        },
        text: "RuntimeTrace".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });
    let mut y = panel.y + 30.0;
    for (index, entry) in model.runtime_trace.entries.iter().take(3).enumerate() {
        let row = UiRect {
            x,
            y,
            width,
            height: 22.0,
        };
        list.commands.push(DrawCommand::Text {
            rect: row,
            text: format!("{}: {}", entry.system_id, entry.message),
            color: UiColor::TEXT_MUTED,
            size: 10.0,
        });
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.runtime_trace.{}.{}", entry.entry_id, index),
            rect: row,
            role: WidgetRole::Button,
            target: HitTarget::RuntimeTraceEntry {
                entry_id: entry.entry_id.clone(),
            },
            enabled: true,
            command_id: "select_trace_entry".to_string(),
            reason_disabled: None,
        }));
        y += 22.0;
    }
    interactions
}
