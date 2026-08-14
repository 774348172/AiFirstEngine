use editor_ui_model::{
    EditorMessageArgs, EditorMessageKey, EditorMessageValue, EditorUiModel, ProjectLauncherCommand,
    ProjectOpenActivityPhase, RecentProjectEntry,
};

use crate::panels::{widget_interaction, WidgetInteractionSpec};
use crate::{
    ActivationPolicy, ControlPseudoStateSet, DrawCommand, EditorWidgetDeclaration, HitTarget,
    UiColor, UiDrawList, UiRect, UiRendererConfig, WidgetRole,
};

use super::resolve_and_paint_control;

pub(crate) fn push_project_launcher(
    list: &mut UiDrawList,
    model: &EditorUiModel,
    config: &UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let busy = model.project_launcher.activity.is_some();
    let left_w = 240.0_f32.min(config.width * 0.32);
    let title_h = 54.0;
    let left = UiRect {
        x: 0.0,
        y: 0.0,
        width: left_w,
        height: config.height,
    };
    let main = UiRect {
        x: left_w,
        y: 0.0,
        width: (config.width - left_w).max(0.0),
        height: config.height,
    };

    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            x: 0.0,
            y: 0.0,
            width: config.width,
            height: config.height,
        },
        color: crate::EditorTheme::DARK_NEUTRAL.surface.root,
        corner_radius: 0.0,
    });
    list.commands.push(DrawCommand::Rect {
        rect: left,
        color: crate::EditorTheme::DARK_NEUTRAL.surface.popup,
        corner_radius: 0.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: left.x + 16.0,
            y: 16.0,
            width: left.width - 32.0,
            height: 20.0,
        },
        text: "AI First Engine".to_string(),
        color: UiColor::TEXT,
        size: 13.0,
    });

    push_launcher_nav_button(
        list,
        UiRect {
            x: 16.0,
            y: 74.0,
            width: left.width - 32.0,
            height: 38.0,
        },
        "open_project",
        "Open Project",
        &model.project_launcher.commands,
        busy,
        &mut LauncherControlContext {
            config,
            interactions: &mut interactions,
        },
    );
    push_launcher_nav_button(
        list,
        UiRect {
            x: 16.0,
            y: 170.0,
            width: left.width - 32.0,
            height: 38.0,
        },
        "create_with_ai",
        "Create with AI",
        &model.project_launcher.commands,
        busy,
        &mut LauncherControlContext {
            config,
            interactions: &mut interactions,
        },
    );
    push_launcher_nav_button(
        list,
        UiRect {
            x: 16.0,
            y: 122.0,
            width: left.width - 32.0,
            height: 38.0,
        },
        "create_project",
        "Create Project",
        &model.project_launcher.commands,
        busy,
        &mut LauncherControlContext {
            config,
            interactions: &mut interactions,
        },
    );

    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: main.x + 32.0,
            y: 34.0,
            width: main.width - 64.0,
            height: 30.0,
        },
        text: model.project_launcher.title.clone(),
        color: UiColor::TEXT,
        size: 28.0,
    });

    let open_rect = UiRect {
        x: main.x + main.width - 238.0,
        y: 32.0,
        width: 92.0,
        height: 36.0,
    };
    let create_ai_rect = UiRect {
        x: main.x + main.width - 370.0,
        y: 32.0,
        width: 122.0,
        height: 36.0,
    };
    let create_rect = UiRect {
        x: main.x + main.width - 136.0,
        y: 32.0,
        width: 112.0,
        height: 36.0,
    };
    push_launcher_command_button(
        list,
        open_rect,
        "open_project",
        "Open",
        true,
        !busy,
        &mut LauncherControlContext {
            config,
            interactions: &mut interactions,
        },
    );
    push_launcher_command_button(
        list,
        create_ai_rect,
        "create_with_ai",
        "Create with AI",
        true,
        !busy,
        &mut LauncherControlContext {
            config,
            interactions: &mut interactions,
        },
    );
    push_launcher_command_button(
        list,
        create_rect,
        "create_project",
        "New project",
        false,
        !busy,
        &mut LauncherControlContext {
            config,
            interactions: &mut interactions,
        },
    );

    let search = UiRect {
        x: if model.project_intent.intent.pre_project_draft_active {
            main.x + 32.0
        } else {
            main.x + main.width - 278.0
        },
        y: 96.0,
        width: if model.project_intent.intent.pre_project_draft_active {
            (main.width - 188.0).max(120.0)
        } else {
            254.0
        },
        height: 36.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: search,
        color: UiColor::PANEL_DARK,
        corner_radius: 4.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: search.x + 14.0,
            y: search.y + 9.0,
            width: search.width - 28.0,
            height: 18.0,
        },
        text: if model.project_intent.intent.pre_project_draft_active {
            if model.ai_panel.prompt_draft.is_empty() {
                "What would you like to make?".to_string()
            } else {
                model.ai_panel.prompt_draft.clone()
            }
        } else if model.project_launcher.search_query.is_empty() {
            "Search...".to_string()
        } else {
            model.project_launcher.search_query.clone()
        },
        color: UiColor::TEXT_MUTED,
        size: 13.0,
    });
    if model.project_intent.intent.pre_project_draft_active {
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: "hit.project_launcher.intent_prompt".to_string(),
            rect: search,
            role: WidgetRole::TextInput,
            target: HitTarget::AiPromptField,
            enabled: true,
            command_id: "set_ai_prompt_draft".to_string(),
            reason_disabled: None,
        }));
        let submit = UiRect {
            x: search.x + search.width + 10.0,
            y: search.y,
            width: 112.0,
            height: search.height,
        };
        let submit_hit_id = "hit.project_launcher.intent_submit";
        let submit_enabled = !model.ai_panel.prompt_draft.trim().is_empty();
        let submit_style = resolve_and_paint_control(
            list,
            submit,
            WidgetRole::Button,
            "launcher-control",
            config.control_pseudo_states(
                submit_hit_id,
                ControlPseudoStateSet::empty(),
                submit_enabled,
            ),
        );
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: submit.x + 14.0,
                y: submit.y + 10.0,
                width: submit.width - 28.0,
                height: 16.0,
            },
            text: "Add to draft".to_string(),
            color: submit_style.foreground,
            size: 12.0,
        });
        let mut interaction = widget_interaction(WidgetInteractionSpec {
            id: submit_hit_id.to_string(),
            rect: submit,
            role: WidgetRole::Button,
            target: HitTarget::AiPanelAction {
                action_id: format!("submit:{}", model.ai_panel.prompt_draft),
            },
            enabled: submit_enabled,
            command_id: "generate_project_patch_from_prompt".to_string(),
            reason_disabled: model
                .ai_panel
                .prompt_draft
                .trim()
                .is_empty()
                .then(|| "Enter a project idea first.".to_string()),
        });
        style_launcher_interaction(&mut interaction);
        interactions.push(interaction);
    }

    let header_y = title_h + 92.0;
    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            x: main.x,
            y: header_y,
            width: main.width,
            height: 42.0,
        },
        color: crate::EditorTheme::DARK_NEUTRAL.surface.panel,
        corner_radius: 0.0,
    });
    push_launcher_table_text(
        list,
        main.x + 100.0,
        header_y + 13.0,
        "NAME",
        UiColor::TEXT_MUTED,
    );
    push_launcher_table_text(
        list,
        main.x + main.width * 0.53,
        header_y + 13.0,
        "MODIFIED",
        UiColor::TEXT_MUTED,
    );
    push_launcher_table_text(
        list,
        main.x + main.width * 0.72,
        header_y + 13.0,
        "ENGINE VERSION",
        UiColor::TEXT_MUTED,
    );

    if model.project_launcher.recent_projects.is_empty() {
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: main.x + 96.0,
                y: header_y + 84.0,
                width: main.width - 192.0,
                height: 24.0,
            },
            text: model.project_launcher.empty_message.clone(),
            color: UiColor::TEXT_MUTED,
            size: 14.0,
        });
    } else {
        for (index, project) in model.project_launcher.recent_projects.iter().enumerate() {
            push_recent_project_row(
                list,
                main,
                header_y + 42.0,
                index,
                project,
                busy,
                &mut LauncherControlContext {
                    config,
                    interactions: &mut interactions,
                },
            );
        }
    }
    if let Some(activity) = &model.project_launcher.activity {
        push_project_open_activity(list, activity, config);
    }
    interactions
}

struct LauncherControlContext<'a> {
    config: &'a UiRendererConfig,
    interactions: &'a mut Vec<EditorWidgetDeclaration>,
}

fn push_launcher_nav_button(
    list: &mut UiDrawList,
    rect: UiRect,
    action_id: &str,
    label: &str,
    commands: &[ProjectLauncherCommand],
    busy: bool,
    context: &mut LauncherControlContext<'_>,
) {
    let enabled = !busy
        && commands
            .iter()
            .find(|command| command.command_id == action_id)
            .is_none_or(|command| command.enabled);
    let reason_disabled = if busy {
        Some("editor.project_open.busy".to_string())
    } else {
        commands
            .iter()
            .find(|command| command.command_id == action_id)
            .and_then(|command| command.reason_disabled.clone())
    };
    let hit_id = format!("hit.project_launcher.{action_id}");
    let style = resolve_and_paint_control(
        list,
        rect,
        WidgetRole::Button,
        "launcher-control",
        context
            .config
            .control_pseudo_states(&hit_id, ControlPseudoStateSet::empty(), enabled),
    );
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: rect.x + 14.0,
            y: rect.y + 10.0,
            width: rect.width - 28.0,
            height: 18.0,
        },
        text: label.to_string(),
        color: style.foreground,
        size: 13.0,
    });
    let mut interaction = widget_interaction(WidgetInteractionSpec {
        id: hit_id,
        rect,
        role: WidgetRole::Button,
        target: HitTarget::ProjectLauncherAction {
            action_id: action_id.to_string(),
        },
        enabled,
        command_id: action_id.to_string(),
        reason_disabled,
    });
    style_launcher_interaction(&mut interaction);
    context.interactions.push(interaction);
}

fn push_launcher_command_button(
    list: &mut UiDrawList,
    rect: UiRect,
    action_id: &str,
    label: &str,
    primary: bool,
    enabled: bool,
    context: &mut LauncherControlContext<'_>,
) {
    let hit_id = format!("hit.project_launcher.{action_id}.top");
    let model = ControlPseudoStateSet::empty().with(crate::ControlPseudoState::Selected, primary);
    let style = resolve_and_paint_control(
        list,
        rect,
        WidgetRole::Button,
        "launcher-control",
        context
            .config
            .control_pseudo_states(&hit_id, model, enabled),
    );
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: rect.x + 12.0,
            y: rect.y + 10.0,
            width: rect.width - 24.0,
            height: 16.0,
        },
        text: label.to_string(),
        color: style.foreground,
        size: 13.0,
    });
    let mut interaction = widget_interaction(WidgetInteractionSpec {
        id: hit_id,
        rect,
        role: WidgetRole::Button,
        target: HitTarget::ProjectLauncherAction {
            action_id: action_id.to_string(),
        },
        enabled,
        command_id: action_id.to_string(),
        reason_disabled: (!enabled).then(|| "editor.project_open.busy".to_string()),
    });
    interaction.model_pseudo_states = model;
    style_launcher_interaction(&mut interaction);
    context.interactions.push(interaction);
}

fn push_launcher_table_text(list: &mut UiDrawList, x: f32, y: f32, text: &str, color: UiColor) {
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x,
            y,
            width: 180.0,
            height: 16.0,
        },
        text: text.to_string(),
        color,
        size: 11.0,
    });
}

fn push_recent_project_row(
    list: &mut UiDrawList,
    main: UiRect,
    start_y: f32,
    index: usize,
    project: &RecentProjectEntry,
    busy: bool,
    context: &mut LauncherControlContext<'_>,
) {
    let row = UiRect {
        x: main.x + 72.0,
        y: start_y + index as f32 * 58.0,
        width: main.width - 96.0,
        height: 54.0,
    };
    let hit_id = format!("hit.project_launcher.recent.{index}");
    let enabled = project.valid && !busy;
    if index % 2 == 1 {
        list.commands.push(DrawCommand::Rect {
            rect: row,
            color: crate::EditorTheme::DARK_NEUTRAL.surface.panel_recessed,
            corner_radius: 0.0,
        });
    }
    resolve_and_paint_control(
        list,
        row,
        WidgetRole::Button,
        "launcher-control",
        context
            .config
            .control_pseudo_states(&hit_id, ControlPseudoStateSet::empty(), enabled),
    );
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: row.x + 24.0,
            y: row.y + 10.0,
            width: row.width * 0.42,
            height: 18.0,
        },
        text: project.name.clone(),
        color: if project.valid {
            UiColor::TEXT
        } else {
            UiColor::WARNING
        },
        size: 13.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: row.x + 24.0,
            y: row.y + 30.0,
            width: row.width * 0.42,
            height: 16.0,
        },
        text: project.path.clone(),
        color: UiColor::TEXT_MUTED,
        size: 11.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: main.x + main.width * 0.53,
            y: row.y + 20.0,
            width: 130.0,
            height: 16.0,
        },
        text: project
            .last_modified_at
            .as_deref()
            .or(project.last_opened_at.as_deref())
            .map(format_recent_project_date)
            .unwrap_or_else(|| "-".to_string()),
        color: UiColor::TEXT_MUTED,
        size: 11.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: main.x + main.width * 0.72,
            y: row.y + 20.0,
            width: 150.0,
            height: 16.0,
        },
        text: project.engine_version.clone(),
        color: UiColor::TEXT_MUTED,
        size: 11.0,
    });
    let mut interaction = widget_interaction(WidgetInteractionSpec {
        id: hit_id,
        rect: row,
        role: WidgetRole::Button,
        target: HitTarget::ProjectLauncherRecentProject {
            project_path: project.path.clone(),
        },
        enabled,
        command_id: "select_recent_project".to_string(),
        reason_disabled: if busy {
            Some("editor.project_open.busy".to_string())
        } else {
            (!project.valid).then(|| format!("Recent project is {}.", project.status))
        },
    });
    style_launcher_interaction(&mut interaction);
    context.interactions.push(interaction);
}

fn style_launcher_interaction(interaction: &mut EditorWidgetDeclaration) {
    interaction.control_classes = crate::ControlClassSet::new(["launcher-control"]);
    interaction.activation_policy = ActivationPolicy::ReleaseInside;
}

fn push_project_open_activity(
    list: &mut UiDrawList,
    activity: &editor_ui_model::ProjectOpenActivityModel,
    config: &UiRendererConfig,
) {
    let panel_width = (config.width - 48.0).clamp(360.0, 520.0);
    let panel = UiRect {
        x: ((config.width - panel_width) * 0.5).max(0.0),
        y: ((config.height - 176.0) * 0.5).max(0.0),
        width: panel_width,
        height: 176.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: panel,
        color: crate::EditorTheme::DARK_NEUTRAL.surface.popup,
        corner_radius: 6.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 24.0,
            y: panel.y + 22.0,
            width: panel.width - 48.0,
            height: 22.0,
        },
        text: activity.project_display_name.clone(),
        color: UiColor::TEXT,
        size: 16.0,
    });
    let phase_key = match activity.phase {
        ProjectOpenActivityPhase::Inspecting => "editor.project_open.inspecting",
        ProjectOpenActivityPhase::TrustCheck => "editor.project_open.trust_check",
        ProjectOpenActivityPhase::CacheCheck => "editor.project_open.cache_check",
        ProjectOpenActivityPhase::CacheLookup => "editor.project_open.cache_lookup",
        ProjectOpenActivityPhase::Promoting => "editor.project_open.promoting",
        ProjectOpenActivityPhase::Warming => "editor.project_open.warming",
        ProjectOpenActivityPhase::Staging => "editor.project_open.staging",
        ProjectOpenActivityPhase::Compiling => "editor.project_open.compiling",
        ProjectOpenActivityPhase::Sealing => "editor.project_open.sealing",
        ProjectOpenActivityPhase::Launching => "editor.project_open.launching",
        ProjectOpenActivityPhase::WaitingReadiness => "editor.project_open.waiting_readiness",
        ProjectOpenActivityPhase::ReadingProject => "editor.project_open.reading_project",
        ProjectOpenActivityPhase::ComputingDigest => "editor.project_open.computing_digest",
        ProjectOpenActivityPhase::LoadingWorkspace => "editor.project_open.loading_workspace",
        ProjectOpenActivityPhase::Cancelled => "editor.project_open.cancelled",
        ProjectOpenActivityPhase::Failed => "editor.project_open.failed",
    };
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 24.0,
            y: panel.y + 58.0,
            width: panel.width - 48.0,
            height: 20.0,
        },
        text: config.localization.text(phase_key),
        color: UiColor::TEXT,
        size: 14.0,
    });
    let bar = UiRect {
        x: panel.x + 24.0,
        y: panel.y + 94.0,
        width: panel.width - 48.0,
        height: 6.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: bar,
        color: UiColor::PANEL_DARK,
        corner_radius: 3.0,
    });
    let segment_width = (bar.width * 0.28).max(48.0);
    let travel = (bar.width - segment_width).max(0.0);
    let progress = (activity.elapsed_ms % 1_400) as f32 / 1_400.0;
    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            x: bar.x + travel * progress,
            y: bar.y,
            width: segment_width,
            height: bar.height,
        },
        color: UiColor::ACCENT,
        corner_radius: 3.0,
    });
    let mut args = EditorMessageArgs::new();
    args.insert(
        "seconds".to_string(),
        EditorMessageValue::U64(activity.elapsed_ms / 1_000),
    );
    let elapsed = EditorMessageKey::parse("editor.project_open.elapsed_seconds".to_string())
        .ok()
        .and_then(|key| config.localization.resolve(&key, &args).ok())
        .unwrap_or_default();
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 24.0,
            y: panel.y + 120.0,
            width: panel.width - 48.0,
            height: 18.0,
        },
        text: elapsed,
        color: UiColor::TEXT_MUTED,
        size: 12.0,
    });
}

fn format_recent_project_date(value: &str) -> String {
    const MAX_UNIX_SECONDS: i64 = 253_402_300_799;

    let Ok(seconds) = value.parse::<i64>() else {
        return value.to_string();
    };
    if !(0..=MAX_UNIX_SECONDS).contains(&seconds) {
        return value.to_string();
    }
    let days = seconds.div_euclid(86_400);
    let (year, month, day) = gregorian_date_from_unix_days(days);
    format!("{year}.{month}.{day}")
}

fn gregorian_date_from_unix_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}
