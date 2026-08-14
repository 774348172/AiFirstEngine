use serde::{Deserialize, Serialize};

use crate::{DrawCommand, EditorWidgetTree, HitRegion, UiDrawList, WidgetPaint, WidgetVisibility};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetExtractOutput {
    pub draw_list: UiDrawList,
    pub extracted_widget_count: usize,
}

pub fn extract_widget_tree(
    tree: &EditorWidgetTree,
    revision: u64,
    frame: u64,
    width: f32,
    height: f32,
) -> WidgetExtractOutput {
    let mut ordered: Vec<_> = tree.nodes.values().collect();
    ordered.sort_by_key(|node| node.resolved_z);
    let mut commands = Vec::new();
    let mut hit_regions = Vec::new();
    let mut extracted_widget_count = 0;
    for node in ordered {
        if node.visibility != WidgetVisibility::Visible {
            continue;
        }
        extracted_widget_count += 1;
        for paint in &node.paint {
            let command = match paint {
                WidgetPaint::Rect {
                    color,
                    corner_radius,
                } => DrawCommand::Rect {
                    rect: node.logical_rect,
                    color: *color,
                    corner_radius: *corner_radius,
                },
                WidgetPaint::Text { text, color, size } => DrawCommand::Text {
                    rect: node.logical_rect,
                    text: text.clone(),
                    color: *color,
                    size: *size,
                },
                WidgetPaint::Image {
                    texture_id,
                    fallback_color,
                    source_uv,
                    tint,
                } => DrawCommand::ImageTextureSlot {
                    rect: node.logical_rect,
                    source_uv: *source_uv,
                    texture_id: texture_id.clone(),
                    fallback_color: *fallback_color,
                    tint: *tint,
                },
                WidgetPaint::Viewport {
                    scene_id,
                    frame,
                    texture_id,
                    target_id,
                } => DrawCommand::ViewportTextureSlot {
                    rect: node.logical_rect,
                    scene_id: scene_id.clone(),
                    frame: *frame,
                    texture_id: texture_id.clone(),
                    target_id: target_id.clone(),
                },
            };
            commands.push(command.with_clip(node.effective_clip));
        }
        if let Some(binding) = &node.binding {
            let Some(hit_rect) = node.effective_clip.map_or(Some(node.logical_rect), |clip| {
                node.logical_rect.intersection(clip)
            }) else {
                continue;
            };
            hit_regions.push(HitRegion {
                id: node
                    .hit_region_id
                    .clone()
                    .unwrap_or_else(|| format!("widget.{}", node.id.as_str())),
                rect: hit_rect,
                target: binding.target.clone(),
                enabled: node.enabled,
                command_id: Some(binding.command_id.clone()),
                reason_disabled: binding.reason_disabled.clone(),
            });
        }
    }
    WidgetExtractOutput {
        draw_list: UiDrawList {
            revision,
            frame,
            surface_width: width,
            surface_height: height,
            commands,
            hit_regions,
        },
        extracted_widget_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        reconcile_widget_tree, EditorCommandBinding, EditorWidgetAction, EditorWidgetDeclaration,
        HitTarget, UiColor, WidgetId, WidgetRole,
    };

    #[test]
    fn widget_extract_derives_draw_and_hit_from_one_geometry() {
        let mut root =
            EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
        root.style.clip = true;
        let mut button =
            EditorWidgetDeclaration::new(WidgetId::semantic("play").unwrap(), WidgetRole::Button);
        button.style.width = Some(80.0);
        button.style.height = Some(20.0);
        button.paint.push(WidgetPaint::Rect {
            color: UiColor {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            },
            corner_radius: 0.0,
        });
        button.binding = Some(EditorCommandBinding {
            action: EditorWidgetAction::Activate,
            command_id: "play".into(),
            target: HitTarget::ToolbarCommand {
                command_id: "play".into(),
            },
            reason_disabled: None,
        });
        root.children.push(button);
        let (mut tree, _) = reconcile_widget_tree(None, &root).unwrap();
        crate::layout_widget_tree(
            &mut tree,
            100.0,
            50.0,
            &mut |_: &WidgetId, _: Option<f32>| (0.0, 0.0),
        )
        .unwrap();
        let output = extract_widget_tree(&tree, 1, 2, 100.0, 50.0);
        assert_eq!(output.draw_list.commands.len(), 1);
        assert_eq!(output.draw_list.hit_regions.len(), 1);
        assert_eq!(
            output.draw_list.hit_regions[0].rect,
            tree.node(&WidgetId::semantic("play").unwrap())
                .unwrap()
                .logical_rect
        );
        assert!(output.draw_list.commands[0].clip().is_some());
    }
}
