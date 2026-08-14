use editor_ui_model::{
    EditorLocalizationSnapshot, EditorMessageArgs, EditorMessageKey, EditorMessageValue,
    EditorUiModel,
};

use crate::layout::push_border;
use crate::panels::{widget_interaction, WidgetInteractionSpec};
use crate::{
    DrawCommand, EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect, WidgetRole,
};

pub(crate) fn push_build_export_panel(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
    localization: &EditorLocalizationSnapshot,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let panel = UiRect {
        x: rect.x + rect.width * 0.48,
        y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
        width: rect.width * 0.26 - 1.0,
        height: rect.height - 25.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: panel,
        color: UiColor::PANEL_DARK,
        corner_radius: 0.0,
    });
    push_border(list, panel);
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 8.0,
            y: panel.y + 7.0,
            width: panel.width - 16.0,
            height: 14.0,
        },
        text: "Build Export".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });

    let profile_text = model
        .build_export
        .profiles
        .iter()
        .find(|profile| profile.active)
        .map(|profile| {
            resolve_text(
                localization,
                "editor.build_export.profile",
                EditorMessageArgs::from([
                    (
                        "label".to_string(),
                        EditorMessageValue::StringInvariant(profile.label.clone()),
                    ),
                    (
                        "target".to_string(),
                        EditorMessageValue::StringInvariant(profile.target.clone()),
                    ),
                ]),
            )
        })
        .unwrap_or_else(|| model.build_export.empty_message.clone());
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 8.0,
            y: panel.y + 27.0,
            width: panel.width - 16.0,
            height: 12.0,
        },
        text: profile_text,
        color: UiColor::TEXT_MUTED,
        size: 10.0,
    });

    if let Some(release) = &model.build_export.release_profile {
        let release_text = format!(
            "Release: {} {}{}",
            release.display_name,
            release.display_version,
            if release.dirty { " | unsaved" } else { "" }
        );
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y: panel.y + 43.0,
                width: panel.width - 16.0,
                height: 12.0,
            },
            text: release_text,
            color: if release.validation_diagnostics.is_empty() {
                UiColor::TEXT_MUTED
            } else {
                UiColor::WARNING
            },
            size: 9.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y: panel.y + 57.0,
                width: panel.width - 16.0,
                height: 12.0,
            },
            text: format!(
                "Exe: {}.exe | {}",
                release.executable_name, release.company_name
            ),
            color: UiColor::TEXT_MUTED,
            size: 9.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y: panel.y + 71.0,
                width: panel.width - 16.0,
                height: 12.0,
            },
            text: format!(
                "Arch: {} | Icon: {}",
                release.architecture, release.icon_asset_id
            ),
            color: UiColor::TEXT_MUTED,
            size: 9.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y: panel.y + 85.0,
                width: panel.width - 16.0,
                height: 12.0,
            },
            text: format!("Output: {}", release.output_preview),
            color: UiColor::TEXT_MUTED,
            size: 9.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y: panel.y + 99.0,
                width: panel.width - 16.0,
                height: 12.0,
            },
            text: if release.validation_diagnostics.is_empty() {
                localization.text("editor.build_export.validation_ready")
            } else {
                resolve_text(
                    localization,
                    "editor.build_export.validation_issues",
                    EditorMessageArgs::from([(
                        "count".to_string(),
                        EditorMessageValue::U64(release.validation_diagnostics.len() as u64),
                    )]),
                )
            },
            color: if release.validation_diagnostics.is_empty() {
                UiColor::TEXT_MUTED
            } else {
                UiColor::WARNING
            },
            size: 9.0,
        });
    }

    let report_text = model
        .build_export
        .last_release_report
        .as_ref()
        .map(|report| {
            format!(
                "Release {} | {} | diagnostics={}",
                report.status, report.entrypoint, report.diagnostic_count
            )
        })
        .or_else(|| {
            model.build_export.last_report.as_ref().map(|report| {
                format!(
                    "{} diagnostics={} exit={}",
                    report.status, report.diagnostic_count, report.player_exit_reason
                )
            })
        })
        .unwrap_or_else(|| "No export report yet.".to_string());
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 8.0,
            y: panel.y + 111.0,
            width: panel.width - 16.0,
            height: 10.0,
        },
        text: report_text,
        color: if model
            .build_export
            .last_release_report
            .as_ref()
            .is_some_and(|report| report.status == "failed")
            || model
                .build_export
                .last_report
                .as_ref()
                .is_some_and(|report| report.status == "failed")
        {
            UiColor::WARNING
        } else {
            UiColor::TEXT_MUTED
        },
        size: 9.0,
    });

    let mut x = panel.x + 8.0;
    let mut button_y = panel.y + panel.height - 46.0;
    for command in &model.build_export.commands {
        let width = match command.command_id.as_str() {
            "build_and_run_desktop_package" => 72.0,
            "build_release_package" => 82.0,
            "save_release_profile" => 76.0,
            "begin_asset_pick" => 62.0,
            _ => 54.0,
        };
        if x + width > panel.x + panel.width - 8.0 {
            x = panel.x + 8.0;
            button_y += 24.0;
        }
        let button = UiRect {
            x,
            y: button_y,
            width,
            height: 20.0,
        };
        list.commands.push(DrawCommand::Rect {
            rect: button,
            color: if command.enabled {
                UiColor::PANEL_LIGHT
            } else {
                UiColor::PANEL
            },
            corner_radius: 2.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: button.x + 7.0,
                y: button.y + 4.0,
                width: button.width - 14.0,
                height: 12.0,
            },
            text: command.label.clone(),
            color: if command.enabled {
                UiColor::TEXT
            } else {
                UiColor::TEXT_MUTED
            },
            size: 10.0,
        });
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.build_export.{}", command.command_id),
            rect: button,
            role: WidgetRole::Button,
            target: HitTarget::ToolbarCommand {
                command_id: command.command_id.clone(),
            },
            enabled: command.enabled,
            command_id: command.command_id.clone(),
            reason_disabled: command.reason_disabled.clone(),
        }));
        x += width + 6.0;
    }
    interactions
}

fn resolve_text(
    localization: &EditorLocalizationSnapshot,
    key: &str,
    args: EditorMessageArgs,
) -> String {
    let key = EditorMessageKey::parse(key).expect("trusted Editor message key");
    localization
        .resolve(&key, &args)
        .unwrap_or_else(|_| localization.text(key.as_str()))
}
