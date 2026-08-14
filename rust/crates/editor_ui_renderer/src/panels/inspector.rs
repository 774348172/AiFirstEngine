use editor_ui_model::{EditorUiModel, InspectorValue};

use crate::metrics::EditorUiMetrics;
use crate::{
    DrawCommand, EditorCommandBinding, EditorWidgetAction, EditorWidgetDeclaration, HitTarget,
    UiColor, UiDrawList, UiRect, WidgetId, WidgetRole,
};

pub(crate) fn push_inspector_fields(
    list: &mut UiDrawList,
    panel: UiRect,
    model: &EditorUiModel,
) -> Vec<EditorWidgetDeclaration> {
    let mut y = panel.y + EditorUiMetrics::PANEL_HEADER_HEIGHT + 8.0;
    let mut declarations = Vec::new();
    for section in &model.inspector.sections {
        if y > panel.y + panel.height - EditorUiMetrics::LIST_ROW_HEIGHT {
            break;
        }
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y,
                width: panel.width - 16.0,
                height: 16.0,
            },
            text: section.title.clone(),
            color: UiColor::TEXT,
            size: 11.0,
        });
        y += 25.0;
        for field in &section.fields {
            if y > panel.y + panel.height - EditorUiMetrics::LIST_ROW_HEIGHT {
                break;
            }
            let row = UiRect {
                x: panel.x + 8.0,
                y,
                width: panel.width - 16.0,
                height: EditorUiMetrics::LIST_ROW_HEIGHT,
            };
            list.commands.push(DrawCommand::Rect {
                rect: row,
                color: if field.editable {
                    UiColor::FIELD
                } else {
                    UiColor::PANEL_DARK
                },
                corner_radius: 0.0,
            });
            list.commands.push(DrawCommand::Text {
                rect: UiRect {
                    x: row.x + 6.0,
                    y: row.y + 5.0,
                    width: row.width * 0.32,
                    height: 14.0,
                },
                text: field.label.clone(),
                color: UiColor::TEXT_MUTED,
                size: 10.0,
            });
            list.commands.push(DrawCommand::Text {
                rect: UiRect {
                    x: row.x + row.width * 0.36,
                    y: row.y + 5.0,
                    width: if field.value_type == editor_ui_model::InspectorValueType::AssetRef
                        && field.editable
                    {
                        row.width * 0.62 - 22.0
                    } else {
                        row.width * 0.62
                    },
                    height: 14.0,
                },
                text: inspector_value_text(&field.value),
                color: UiColor::TEXT,
                size: 10.0,
            });
            declarations.push(
                EditorWidgetDeclaration::new(
                    WidgetId::semantic(format!("editor/panel/inspector/field/{}", field.field_id))
                        .expect("inspector field id must be semantic"),
                    WidgetRole::TextInput,
                )
                .with_absolute_rect(row, 70_000 + declarations.len() as i32)
                .with_interaction(
                    format!("hit.inspector_field.{}", field.field_id),
                    field.editable,
                    EditorCommandBinding {
                        action: EditorWidgetAction::Focus,
                        command_id: "focus_inspector_field".to_string(),
                        target: HitTarget::InspectorField {
                            field_id: field.field_id.clone(),
                        },
                        reason_disabled: (!field.editable)
                            .then(|| "Field is read-only.".to_string()),
                    },
                ),
            );
            if field.value_type == editor_ui_model::InspectorValueType::AssetRef && field.editable {
                let picker = UiRect {
                    x: row.x + row.width - 27.0,
                    y: row.y + 2.0,
                    width: 25.0,
                    height: 25.0,
                };
                list.commands.push(DrawCommand::Rect {
                    rect: picker,
                    color: UiColor::PANEL_LIGHT,
                    corner_radius: 2.0,
                });
                list.commands.push(DrawCommand::Text {
                    rect: UiRect {
                        x: picker.x + 5.0,
                        y: picker.y + 3.0,
                        width: 12.0,
                        height: 12.0,
                    },
                    text: "...".to_string(),
                    color: UiColor::TEXT,
                    size: 8.0,
                });
                declarations.push(
                    EditorWidgetDeclaration::new(
                        WidgetId::semantic(format!(
                            "editor/panel/inspector/picker/{}",
                            field.field_id
                        ))
                        .expect("inspector field id must be semantic"),
                        WidgetRole::Button,
                    )
                    .with_absolute_rect(picker, 80_000 + declarations.len() as i32)
                    .with_interaction(
                        format!("hit.inspector_asset_picker.{}", field.field_id),
                        true,
                        EditorCommandBinding {
                            action: EditorWidgetAction::Activate,
                            command_id: "begin_asset_pick".to_string(),
                            target: HitTarget::InspectorAssetPicker {
                                field_id: field.field_id.clone(),
                            },
                            reason_disabled: None,
                        },
                    ),
                );
            }
            y += 31.0;
        }
        y += 4.0;
    }
    declarations
}

fn inspector_value_text(value: &InspectorValue) -> String {
    match value {
        InspectorValue::String(value) => value.clone(),
        InspectorValue::Bool(value) => value.to_string(),
        InspectorValue::Number(value) => format!("{value:.3}"),
        InspectorValue::Vec3(value) => format!("{:.2}, {:.2}, {:.2}", value.x, value.y, value.z),
        InspectorValue::AssetRef(value) => value.asset_id.clone(),
        InspectorValue::EntityRef(value) => value.clone(),
        InspectorValue::Json(value) => value.to_string(),
        InspectorValue::Empty => "-".to_string(),
    }
}
