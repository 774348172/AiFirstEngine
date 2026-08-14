use crate::{DrawCommand, UiDrawList};

pub(crate) struct EditorUiMetrics;

impl EditorUiMetrics {
    pub(crate) const FONT_SIZE_DELTA: f32 = 5.0;
    pub(crate) const MENU_BAR_HEIGHT: f32 = 27.0;
    pub(crate) const TOOLBAR_HEIGHT: f32 = 35.0;
    pub(crate) const PANEL_HEADER_HEIGHT: f32 = 29.0;
    pub(crate) const COMPACT_CONTROL_HEIGHT: f32 = 29.0;
    pub(crate) const POPUP_ROW_HEIGHT: f32 = 33.0;
    pub(crate) const LIST_ROW_HEIGHT: f32 = 29.0;

    pub(crate) fn apply_typography_scale(list: &mut UiDrawList) {
        for command in &mut list.commands {
            Self::scale_command(command);
        }
    }

    fn scale_command(command: &mut DrawCommand) {
        match command {
            DrawCommand::Clipped { command, .. } => Self::scale_command(command),
            DrawCommand::Text { rect, size, .. } => {
                *size += Self::FONT_SIZE_DELTA;
                rect.height += Self::FONT_SIZE_DELTA;
            }
            _ => {}
        }
    }
}
