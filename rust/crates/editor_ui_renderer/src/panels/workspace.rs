use editor_ui_model::{AuthoringCommand, AuthoringCommandAvailability, EditorUiModel};

use crate::layout::push_border;
use crate::panels::{widget_interaction, WidgetInteractionSpec};
use crate::{
    DrawCommand, EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect, WidgetRole,
};

pub(crate) fn push_workspace_summary_panel(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    if model.authoring_workflow.active_step == editor_ui_model::AuthoringStepId::Input {
        push_input_mapping_workspace(list, rect, model, &mut interactions);
        return interactions;
    }
    let panel = UiRect {
        x: rect.x + rect.width * 0.56,
        y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
        width: rect.width * 0.18,
        height: (rect.height - 25.0) * 0.58,
    };
    list.commands.push(DrawCommand::Rect {
        rect: panel,
        color: UiColor::PANEL_DARK,
        corner_radius: 0.0,
    });
    push_border(list, panel);
    let x = panel.x + 8.0;
    let width = panel.width - 16.0;
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x,
            y: panel.y + 7.0,
            width,
            height: 14.0,
        },
        text: "Workspace".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: x + 86.0,
            y: panel.y + 7.0,
            width: width - 86.0,
            height: 14.0,
        },
        text: format!(
            "workflow {:?} play={} build={}",
            model.authoring_workflow.global_status,
            model.authoring_workflow.can_play,
            model.authoring_workflow.can_build
        ),
        color: UiColor::TEXT_MUTED,
        size: 9.0,
    });
    let workspace = &model.project_authoring_workspace;
    let project_text = workspace
        .project_id
        .as_ref()
        .map(|project_id| format!("Project {project_id}"))
        .unwrap_or_else(|| workspace.empty_message.clone());
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x,
            y: panel.y + 25.0,
            width,
            height: 12.0,
        },
        text: project_text,
        color: UiColor::TEXT_MUTED,
        size: 10.0,
    });
    let mut y = panel.y + 43.0;
    for step in model.authoring_workflow.steps.iter().take(10) {
        let row = UiRect {
            x,
            y,
            width,
            height: 14.0,
        };
        let active = step.id == model.authoring_workflow.active_step;
        if active {
            list.commands.push(DrawCommand::Rect {
                rect: row,
                color: UiColor::PANEL_LIGHT,
                corner_radius: 0.0,
            });
        }
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: row.x + 4.0,
                y: row.y + 2.0,
                width: row.width - 8.0,
                height: 10.0,
            },
            text: format!("{} {:?} items={}", step.title, step.status, step.item_count),
            color: workflow_status_color(step.status),
            size: 8.5,
        });
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.authoring_workflow_step.{}", step.id.as_str()),
            rect: row,
            role: WidgetRole::Button,
            target: HitTarget::AuthoringWorkflowStep {
                step_id: step.id.as_str().to_string(),
            },
            enabled: true,
            command_id: "set_authoring_workflow_step".to_string(),
            reason_disabled: None,
        }));
        y += 15.0;

        if let Some(command) = &step.primary_command {
            push_workflow_command_row(
                list,
                UiRect {
                    x: x + 8.0,
                    y,
                    width: width - 8.0,
                    height: 13.0,
                },
                &format!("> {}", command.label),
                command,
                format!(
                    "hit.authoring_workflow_command.{}.primary",
                    step.id.as_str()
                ),
                &mut interactions,
            );
            y += 14.0;
        }
    }

    y += 2.0;
    for task in model.authoring_workflow.recommended_tasks.iter().take(2) {
        let row = UiRect {
            x,
            y,
            width,
            height: 12.0,
        };
        list.commands.push(DrawCommand::Text {
            rect: row,
            text: format!("Task {:?}: {}", task.priority, task.title),
            color: UiColor::TEXT_MUTED,
            size: 9.0,
        });
        if let Some(command) = &task.command {
            interactions.push(workflow_command_widget(
                format!("hit.authoring_workflow_task.{}", task.id),
                row,
                command,
            ));
        }
        y += 14.0;
    }
    interactions
}

fn push_input_mapping_workspace(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let panel = UiRect {
        x: rect.x + 1.0,
        y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
        width: rect.width - 2.0,
        height: rect.height - 25.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: panel,
        color: UiColor::PANEL_DARK,
        corner_radius: 0.0,
    });
    push_border(list, panel);
    let mapping = &model.input_mapping_authoring;
    let Some(path) = mapping.selected_path.as_deref() else {
        push_text(
            list,
            UiRect {
                x: panel.x + 10.0,
                y: panel.y + 10.0,
                width: panel.width - 20.0,
                height: 18.0,
            },
            &mapping.empty_message,
            UiColor::TEXT_MUTED,
            10.0,
        );
        return;
    };

    let mut toolbar_x = panel.x + 8.0;
    push_text(
        list,
        UiRect {
            x: toolbar_x,
            y: panel.y + 6.0,
            width: panel.width * 0.34,
            height: 16.0,
        },
        &format!(
            "Input Mapping  {}{}",
            path,
            if mapping.dirty { " *" } else { "" }
        ),
        UiColor::TEXT,
        10.0,
    );
    toolbar_x += panel.width * 0.35;
    for (label, action, enabled) in [
        ("Open", "open", true),
        ("Validate", "validate", mapping.mapping_id.is_some()),
        ("Preview", "preview", mapping.selected_binding_id.is_some()),
        ("Save", "save", mapping.dirty),
        ("Discard", "discard", mapping.dirty),
    ] {
        let button = UiRect {
            x: toolbar_x,
            y: panel.y + 4.0,
            width: 62.0,
            height: 18.0,
        };
        push_input_button(
            list,
            button,
            label,
            action,
            path,
            (None, None, enabled),
            interactions,
        );
        toolbar_x += 66.0;
    }
    let next_report_level = match mapping.report_level {
        editor_ui_model::InputMappingReportLevel::Off => "Summary",
        editor_ui_model::InputMappingReportLevel::Summary => "Trace",
        editor_ui_model::InputMappingReportLevel::Trace => "Off",
    };
    let report_button = UiRect {
        x: toolbar_x,
        y: panel.y + 4.0,
        width: 72.0,
        height: 18.0,
    };
    push_input_button(
        list,
        report_button,
        &format!("Report {next_report_level}"),
        "report_level",
        path,
        (None, Some(next_report_level), true),
        interactions,
    );
    toolbar_x += 76.0;
    if let Some(binding_id) = mapping.selected_binding_id.as_deref() {
        let capturing = mapping.capture_binding_id.as_deref() == Some(binding_id);
        let button = UiRect {
            x: toolbar_x,
            y: panel.y + 4.0,
            width: 72.0,
            height: 18.0,
        };
        push_input_button(
            list,
            button,
            if capturing { "Cancel" } else { "Capture" },
            if capturing {
                "cancel_capture"
            } else {
                "begin_capture"
            },
            path,
            (Some(binding_id), None, true),
            interactions,
        );
    }

    let content_y = panel.y + 27.0;
    let content_height = panel.height - 31.0;
    let contexts = UiRect {
        x: panel.x + 5.0,
        y: content_y,
        width: panel.width * 0.20 - 7.0,
        height: content_height,
    };
    let actions = UiRect {
        x: panel.x + panel.width * 0.20,
        y: content_y,
        width: panel.width * 0.45,
        height: content_height,
    };
    let properties = UiRect {
        x: panel.x + panel.width * 0.65 + 2.0,
        y: content_y,
        width: panel.width * 0.35 - 7.0,
        height: content_height,
    };
    for section in [contexts, actions, properties] {
        list.commands.push(DrawCommand::Rect {
            rect: section,
            color: UiColor::PANEL,
            corner_radius: 0.0,
        });
        push_border(list, section);
    }
    push_input_contexts(list, contexts, mapping, path, interactions);
    push_input_actions(list, actions, mapping, path, interactions);
    push_input_properties(list, properties, mapping, path, interactions);
}

fn push_input_contexts(
    list: &mut UiDrawList,
    rect: UiRect,
    mapping: &editor_ui_model::InputMappingAuthoringModel,
    path: &str,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    push_section_header(list, rect, "Contexts");
    let add = UiRect {
        x: rect.x + rect.width - 24.0,
        y: rect.y + 3.0,
        width: 18.0,
        height: 16.0,
    };
    let next_id = format!("context.new.{}", mapping.contexts.len() + 1);
    push_input_button(
        list,
        add,
        "+",
        "add_context",
        path,
        (Some(&next_id), Some("0"), true),
        interactions,
    );
    let mut y = rect.y + 23.0;
    for context in mapping.contexts.iter().take(7) {
        let row = UiRect {
            x: rect.x + 5.0,
            y,
            width: rect.width - 10.0,
            height: 19.0,
        };
        if mapping.selected_context_id.as_deref() == Some(context.context_id.as_str()) {
            list.commands.push(DrawCommand::Rect {
                rect: row,
                color: UiColor::PANEL_LIGHT,
                corner_radius: 0.0,
            });
        }
        push_text(
            list,
            row,
            &format!(
                "{}  p={}{}",
                context.context_id,
                context.priority,
                if context.consume_input {
                    " consume"
                } else {
                    ""
                }
            ),
            UiColor::TEXT,
            9.0,
        );
        interactions.push(input_widget(
            format!("hit.input.context.{}", context.context_id),
            row,
            "select_context",
            path,
            Some(&context.context_id),
            None,
            true,
        ));
        let toggle = UiRect {
            x: row.x + row.width - 17.0,
            y: row.y + 1.0,
            width: 15.0,
            height: 15.0,
        };
        push_input_button(
            list,
            toggle,
            if context.consume_input { "C" } else { "c" },
            "set_context_consume",
            path,
            (
                Some(&context.context_id),
                Some(if context.consume_input {
                    "false"
                } else {
                    "true"
                }),
                true,
            ),
            interactions,
        );
        let priority = UiRect {
            x: toggle.x - 17.0,
            y: toggle.y,
            width: 15.0,
            height: 15.0,
        };
        push_input_button(
            list,
            priority,
            "+",
            "set_context_priority",
            path,
            (
                Some(&context.context_id),
                Some(&(context.priority + 1).to_string()),
                true,
            ),
            interactions,
        );
        y += 21.0;
    }
}

fn push_input_actions(
    list: &mut UiDrawList,
    rect: UiRect,
    mapping: &editor_ui_model::InputMappingAuthoringModel,
    path: &str,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    push_section_header(list, rect, "Actions / Bindings");
    let add = UiRect {
        x: rect.x + rect.width - 24.0,
        y: rect.y + 3.0,
        width: 18.0,
        height: 16.0,
    };
    let next_id = format!("action.new.{}", mapping.actions.len() + 1);
    push_input_button(
        list,
        add,
        "+",
        "add_action",
        path,
        (Some(&next_id), Some("Button"), true),
        interactions,
    );
    let mut y = rect.y + 22.0;
    for action in mapping.actions.iter().take(7) {
        let row = UiRect {
            x: rect.x + 5.0,
            y,
            width: rect.width - 10.0,
            height: 17.0,
        };
        if mapping.selected_action_id.as_deref() == Some(action.action_id.as_str()) {
            list.commands.push(DrawCommand::Rect {
                rect: row,
                color: UiColor::PANEL_LIGHT,
                corner_radius: 0.0,
            });
        }
        push_text(
            list,
            row,
            &format!("{}  {:?}", action.action_id, action.value_type),
            UiColor::TEXT,
            9.0,
        );
        interactions.push(input_widget(
            format!("hit.input.action.{}", action.action_id),
            row,
            "select_action",
            path,
            Some(&action.action_id),
            None,
            true,
        ));
        let next_type = match action.value_type {
            editor_ui_model::InputActionValueKind::Button => "Axis1",
            editor_ui_model::InputActionValueKind::Axis1 => "Axis2",
            editor_ui_model::InputActionValueKind::Axis2 => "Pointer",
            editor_ui_model::InputActionValueKind::Pointer => "Button",
        };
        let type_button = UiRect {
            x: row.x + row.width - 38.0,
            y: row.y + 1.0,
            width: 36.0,
            height: 14.0,
        };
        push_input_button(
            list,
            type_button,
            next_type,
            "set_action_type",
            path,
            (Some(&action.action_id), Some(next_type), true),
            interactions,
        );
        if let Some(context_id) = mapping.selected_context_id.as_deref().or_else(|| {
            mapping
                .contexts
                .first()
                .map(|context| context.context_id.as_str())
        }) {
            let default_path = match action.value_type {
                editor_ui_model::InputActionValueKind::Button => "keyboard/Space",
                editor_ui_model::InputActionValueKind::Axis1 => "mouse/Wheel",
                editor_ui_model::InputActionValueKind::Axis2 => "gamepad/LeftStick",
                editor_ui_model::InputActionValueKind::Pointer => "mouse/Position",
            };
            let add_binding_value = format!("{context_id}|{default_path}");
            let add_binding = UiRect {
                x: type_button.x - 18.0,
                y: type_button.y,
                width: 16.0,
                height: 14.0,
            };
            push_input_button(
                list,
                add_binding,
                "+",
                "add_binding",
                path,
                (Some(&action.action_id), Some(&add_binding_value), true),
                interactions,
            );
        }
        y += 18.0;
        for binding in mapping
            .bindings
            .iter()
            .filter(|binding| binding.action_id == action.action_id)
            .take(3)
        {
            let binding_row = UiRect {
                x: rect.x + 18.0,
                y,
                width: rect.width - 23.0,
                height: 15.0,
            };
            push_text(
                list,
                binding_row,
                &format!("{}  {}", binding.device_path, binding.trigger),
                if mapping.selected_binding_id.as_deref() == Some(binding.binding_id.as_str()) {
                    UiColor::ACCENT
                } else {
                    UiColor::TEXT_MUTED
                },
                8.0,
            );
            interactions.push(input_widget(
                format!("hit.input.binding.{}", binding.binding_id),
                binding_row,
                "select_binding",
                path,
                Some(&binding.binding_id),
                None,
                true,
            ));
            y += 16.0;
        }
        if y + 18.0 > rect.y + rect.height {
            break;
        }
    }
}

fn push_input_properties(
    list: &mut UiDrawList,
    rect: UiRect,
    mapping: &editor_ui_model::InputMappingAuthoringModel,
    path: &str,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    push_section_header(list, rect, "Properties / Diagnostics");
    let mut y = rect.y + 23.0;
    if let Some(binding_id) = mapping.selected_binding_id.as_deref() {
        if let Some(binding) = mapping
            .bindings
            .iter()
            .find(|binding| binding.binding_id == binding_id)
        {
            push_text(
                list,
                UiRect {
                    x: rect.x + 6.0,
                    y,
                    width: rect.width - 12.0,
                    height: 15.0,
                },
                &format!("{} / {}", binding.action_id, binding.binding_id),
                UiColor::TEXT,
                8.5,
            );
            y += 17.0;
            let next_trigger = if binding.trigger.starts_with("Pressed") {
                "Released"
            } else if binding.trigger.starts_with("Released") {
                "Down"
            } else {
                "Pressed"
            };
            let trigger_button = UiRect {
                x: rect.x + 6.0,
                y,
                width: (rect.width - 18.0) * 0.5,
                height: 17.0,
            };
            push_input_button(
                list,
                trigger_button,
                &format!("Trigger: {next_trigger}"),
                "set_trigger",
                path,
                (Some(binding_id), Some(next_trigger), true),
                interactions,
            );
            let next_processor = if binding.processor.starts_with("None") {
                "Invert"
            } else {
                "None"
            };
            let processor_button = UiRect {
                x: trigger_button.x + trigger_button.width + 4.0,
                y,
                width: trigger_button.width,
                height: 17.0,
            };
            push_input_button(
                list,
                processor_button,
                &format!("Proc: {next_processor}"),
                "set_processor",
                path,
                (Some(binding_id), Some(next_processor), true),
                interactions,
            );
            y += 20.0;
            for control in mapping
                .control_catalog
                .controls
                .iter()
                .filter(|entry| entry.selectable)
            {
                let row = UiRect {
                    x: rect.x + 6.0,
                    y,
                    width: rect.width - 12.0,
                    height: 15.0,
                };
                push_text(
                    list,
                    row,
                    &format!("{}  {}", control.label, control.device_path),
                    if binding
                        .device_path
                        .eq_ignore_ascii_case(&control.device_path)
                    {
                        UiColor::ACCENT
                    } else {
                        UiColor::TEXT_MUTED
                    },
                    8.0,
                );
                interactions.push(input_widget(
                    format!(
                        "hit.input.device.{}.{}",
                        binding_id,
                        control.device_path.replace('/', "_")
                    ),
                    row,
                    "set_device_path",
                    path,
                    Some(binding_id),
                    Some(&control.device_path),
                    true,
                ));
                y += 16.0;
                if y + 40.0 > rect.y + rect.height {
                    break;
                }
            }
        }
    }
    let status_color = match mapping.report.validation_status {
        editor_ui_model::InputMappingValidationStatus::Error => UiColor::ERROR,
        editor_ui_model::InputMappingValidationStatus::Warning => UiColor::WARNING,
        _ => UiColor::TEXT_MUTED,
    };
    push_text(
        list,
        UiRect {
            x: rect.x + 6.0,
            y: (rect.y + rect.height - 34.0).max(y),
            width: rect.width - 12.0,
            height: 30.0,
        },
        &format!(
            "validation={:?} diagnostics={} hash={}",
            mapping.report.validation_status,
            mapping.report.diagnostics.len(),
            mapping.source_hash.as_deref().unwrap_or("not-open")
        ),
        status_color,
        8.0,
    );
    if let Some(preview) = &mapping.preview {
        push_text(
            list,
            UiRect {
                x: rect.x + 6.0,
                y: rect.y + rect.height - 18.0,
                width: rect.width - 12.0,
                height: 14.0,
            },
            &format!(
                "preview={:?} {} actions={} shadowed={}",
                preview.status,
                preview.device_path,
                preview.actions.len(),
                preview.shadowed_binding_ids.len()
            ),
            UiColor::ACCENT,
            8.0,
        );
    }
}

fn push_section_header(list: &mut UiDrawList, rect: UiRect, title: &str) {
    push_text(
        list,
        UiRect {
            x: rect.x + 6.0,
            y: rect.y + 4.0,
            width: rect.width - 12.0,
            height: 15.0,
        },
        title,
        UiColor::TEXT,
        9.5,
    );
}

fn push_input_button(
    list: &mut UiDrawList,
    rect: UiRect,
    label: &str,
    action: &str,
    path: &str,
    control: (Option<&str>, Option<&str>, bool),
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    let (target_id, value, enabled) = control;
    list.commands.push(DrawCommand::Rect {
        rect,
        color: if enabled {
            UiColor::FIELD
        } else {
            UiColor::PANEL
        },
        corner_radius: 0.0,
    });
    push_text(
        list,
        rect,
        label,
        if enabled {
            UiColor::TEXT
        } else {
            UiColor::TEXT_MUTED
        },
        8.5,
    );
    interactions.push(input_widget(
        format!(
            "hit.input.command.{action}.{}",
            target_id.unwrap_or("asset")
        ),
        rect,
        action,
        path,
        target_id,
        value,
        enabled,
    ));
}

fn input_widget(
    id: String,
    rect: UiRect,
    action: &str,
    path: &str,
    target_id: Option<&str>,
    value: Option<&str>,
    enabled: bool,
) -> EditorWidgetDeclaration {
    widget_interaction(WidgetInteractionSpec {
        id,
        rect,
        role: WidgetRole::Button,
        target: HitTarget::InputMappingControl {
            action: action.to_string(),
            path: path.to_string(),
            target_id: target_id.map(str::to_string),
            value: value.map(str::to_string),
        },
        enabled,
        command_id: format!("input_mapping_{action}"),
        reason_disabled: (!enabled).then(|| format!("Input mapping {action} is disabled.")),
    })
}

fn push_text(list: &mut UiDrawList, rect: UiRect, text: &str, color: UiColor, size: f32) {
    list.commands.push(DrawCommand::Text {
        rect,
        text: text.to_string(),
        color,
        size,
    });
}

fn push_workflow_command_row(
    list: &mut UiDrawList,
    rect: UiRect,
    text: &str,
    command: &AuthoringCommand,
    hit_id: String,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    list.commands.push(DrawCommand::Text {
        rect,
        text: text.to_string(),
        color: UiColor::TEXT_MUTED,
        size: 8.0,
    });
    interactions.push(workflow_command_widget(hit_id, rect, command));
}

fn workflow_command_widget(
    id: String,
    rect: UiRect,
    command: &AuthoringCommand,
) -> EditorWidgetDeclaration {
    let enabled = command.availability == AuthoringCommandAvailability::Available;
    widget_interaction(WidgetInteractionSpec {
        id,
        rect,
        role: WidgetRole::Button,
        target: HitTarget::AuthoringWorkflowCommand {
            command_id: command.command_id.clone(),
            payload_kind: command.payload_kind.clone(),
            domain: command.domain.as_str().to_string(),
        },
        enabled,
        command_id: command.command_id.clone(),
        reason_disabled: if enabled {
            None
        } else {
            Some(format!("{} is disabled.", command.label))
        },
    })
}

fn workflow_status_color(status: editor_ui_model::AuthoringStepStatus) -> UiColor {
    match status {
        editor_ui_model::AuthoringStepStatus::Failed
        | editor_ui_model::AuthoringStepStatus::Blocked => UiColor::ERROR,
        editor_ui_model::AuthoringStepStatus::NeedsAttention
        | editor_ui_model::AuthoringStepStatus::Dirty => UiColor::WARNING,
        editor_ui_model::AuthoringStepStatus::Ready
        | editor_ui_model::AuthoringStepStatus::Complete => UiColor::TEXT,
        editor_ui_model::AuthoringStepStatus::NotAvailable
        | editor_ui_model::AuthoringStepStatus::Empty
        | editor_ui_model::AuthoringStepStatus::Running => UiColor::TEXT_MUTED,
    }
}
