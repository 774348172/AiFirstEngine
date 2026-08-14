use editor_ui_model::EditorLocalizationSnapshot;

use crate::{DrawCommand, UiDrawList};

pub fn localize_editor_draw_list(
    draw_list: &mut UiDrawList,
    snapshot: &EditorLocalizationSnapshot,
) {
    for command in &mut draw_list.commands {
        localize_command(command, snapshot);
    }
    for region in &mut draw_list.hit_regions {
        if let Some(reason) = region.reason_disabled.as_mut() {
            if let Some(localized) = snapshot.localize_native_exact(reason) {
                *reason = localized;
            }
        }
    }
}

fn localize_command(command: &mut DrawCommand, snapshot: &EditorLocalizationSnapshot) {
    match command {
        DrawCommand::Clipped { command, .. } => localize_command(command, snapshot),
        DrawCommand::Text { text, .. } => {
            if let Some(localized) = snapshot.localize_native_exact(text) {
                *text = localized;
            }
        }
        DrawCommand::Rect { .. }
        | DrawCommand::ViewportTextureSlot { .. }
        | DrawCommand::ImageTextureSlot { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UiColor, UiRect};

    #[test]
    fn localization_translates_cataloged_editor_text_and_preserves_invariants() {
        let mut draw_list = UiDrawList {
            revision: 1,
            frame: 2,
            surface_width: 1280.0,
            surface_height: 720.0,
            commands: vec![
                DrawCommand::Text {
                    rect: UiRect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 24.0,
                    },
                    text: "Open Project".to_string(),
                    color: UiColor::TEXT,
                    size: 16.0,
                },
                DrawCommand::Text {
                    rect: UiRect {
                        x: 0.0,
                        y: 24.0,
                        width: 100.0,
                        height: 24.0,
                    },
                    text: "C:\\Projects\\Demo".to_string(),
                    color: UiColor::TEXT,
                    size: 16.0,
                },
            ],
            hit_regions: Vec::new(),
        };
        localize_editor_draw_list(&mut draw_list, &EditorLocalizationSnapshot::default());
        let texts = draw_list
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["打开项目", "C:\\Projects\\Demo"]);
    }
}
