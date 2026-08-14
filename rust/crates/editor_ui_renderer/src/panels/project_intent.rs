use editor_ui_model::EditorUiModel;

use crate::{DrawCommand, EditorWidgetDeclaration, UiColor, UiDrawList, UiRect};
use crate::{HitTarget, WidgetRole};

pub(crate) fn push_project_intent_panel(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let intent = &model.project_intent.intent;
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: rect.x + 14.0,
            y: rect.y + 34.0,
            width: 92.0,
            height: 18.0,
        },
        text: "Intent".to_string(),
        color: UiColor::TEXT,
        size: 13.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: rect.x + 108.0,
            y: rect.y + 35.0,
            width: (rect.width - 122.0).max(0.0),
            height: 16.0,
        },
        text: format!(
            "{} active  {} parked  {} needs evidence  {} untriaged",
            intent.active_count,
            intent.parked_count,
            intent.needs_evidence_count,
            intent.pending_normalization_count
        ),
        color: UiColor::TEXT_MUTED,
        size: 11.0,
    });

    let review = &model.project_intent.change_review;
    if let Some(proposal_id) = &review.proposal_id {
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + rect.width * 0.52,
                y: rect.y + 35.0,
                width: (rect.width * 0.46).max(0.0),
                height: 16.0,
            },
            text: format!(
                "Change {}  {} selected  {} risk(s)",
                proposal_id,
                review.selected_work_item_count,
                review.risks.len()
            ),
            color: if review.approval_ready {
                UiColor::ACCENT
            } else {
                UiColor::WARNING
            },
            size: 11.0,
        });
        if review.approval_ready {
            if let Some(proposal_digest) = &review.proposal_digest {
                push_intent_action(
                    list,
                    &mut interactions,
                    UiRect {
                        x: rect.x + rect.width - 88.0,
                        y: rect.y + 28.0,
                        width: 74.0,
                        height: 24.0,
                    },
                    "approve",
                    proposal_digest,
                    "Approve",
                    true,
                    None,
                );
            }
        }
    }

    let header_y = rect.y + 62.0;
    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            x: rect.x,
            y: header_y,
            width: rect.width,
            height: 24.0,
        },
        color: UiColor::PANEL_DARK,
        corner_radius: 0.0,
    });
    for (x, width, label) in [
        (rect.x + 14.0, rect.width * 0.48, "WORK ITEM"),
        (rect.x + rect.width * 0.52, 110.0, "STATUS"),
        (rect.x + rect.width - 154.0, 64.0, "REVISION"),
        (rect.x + rect.width - 82.0, 68.0, "ACTION"),
    ] {
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x,
                y: header_y + 6.0,
                width,
                height: 14.0,
            },
            text: label.to_string(),
            color: UiColor::TEXT_MUTED,
            size: 10.0,
        });
    }

    for (index, item) in intent.work_items.iter().take(6).enumerate() {
        let y = header_y + 26.0 + index as f32 * 24.0;
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + 14.0,
                y,
                width: (rect.width * 0.48 - 20.0).max(0.0),
                height: 16.0,
            },
            text: item.title.clone(),
            color: UiColor::TEXT,
            size: 11.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + rect.width * 0.52,
                y,
                width: 130.0,
                height: 16.0,
            },
            text: item.status.clone(),
            color: if item.ready {
                UiColor::ACCENT
            } else {
                UiColor::TEXT_MUTED
            },
            size: 11.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + rect.width - 154.0,
                y,
                width: 72.0,
                height: 16.0,
            },
            text: format!("r{}", item.revision),
            color: UiColor::TEXT_MUTED,
            size: 11.0,
        });
        let action = match item.status.as_str() {
            "parked" => Some(("resume", "Resume")),
            "done" => Some(("reopen", "Reopen")),
            "cancelled" | "merged" | "split" => None,
            _ => Some(("park", "Park")),
        };
        if let Some((action_id, label)) = action {
            push_intent_action(
                list,
                &mut interactions,
                UiRect {
                    x: rect.x + rect.width - 82.0,
                    y: y - 4.0,
                    width: 68.0,
                    height: 22.0,
                },
                action_id,
                &item.work_item_id,
                label,
                true,
                None,
            );
        }
    }

    let production = &model.project_intent.production;
    if let Some(state) = &production.state {
        let run_action = match state.as_str() {
            "approved" | "creating_project" | "executing" | "previewing" => {
                Some(("advance", "Continue"))
            }
            "failed" | "stale" => Some(("recover", "Recover")),
            _ => None,
        };
        if let (Some(run_id), Some((action_id, label))) = (&production.run_id, run_action) {
            push_intent_action(
                list,
                &mut interactions,
                UiRect {
                    x: rect.x + rect.width - 168.0,
                    y: rect.y + rect.height - 30.0,
                    width: 74.0,
                    height: 22.0,
                },
                action_id,
                run_id,
                label,
                true,
                None,
            );
        }
        if let Some(run_id) = &production.run_id {
            let cancellable = matches!(
                state.as_str(),
                "approved"
                    | "creating_project"
                    | "executing"
                    | "previewing"
                    | "paused_for_decision"
            );
            if cancellable {
                push_intent_action(
                    list,
                    &mut interactions,
                    UiRect {
                        x: rect.x + rect.width - 86.0,
                        y: rect.y + rect.height - 30.0,
                        width: 72.0,
                        height: 22.0,
                    },
                    "cancel",
                    run_id,
                    "Cancel",
                    true,
                    None,
                );
            }
        }
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + 14.0,
                y: rect.y + rect.height - 24.0,
                width: (rect.width - 210.0).max(0.0),
                height: 16.0,
            },
            text: format!(
                "Production {}  {}/{} steps{}",
                state,
                production.completed_steps,
                production.total_steps,
                production
                    .waiting_reason
                    .as_ref()
                    .map(|reason| format!("  waiting: {reason}"))
                    .unwrap_or_default()
            ),
            color: UiColor::TEXT_MUTED,
            size: 11.0,
        });
    }
    interactions
}

#[allow(clippy::too_many_arguments)]
fn push_intent_action(
    list: &mut UiDrawList,
    interactions: &mut Vec<EditorWidgetDeclaration>,
    rect: UiRect,
    action_id: &str,
    subject_id: &str,
    label: &str,
    enabled: bool,
    reason_disabled: Option<String>,
) {
    list.commands.push(DrawCommand::Rect {
        rect,
        color: if enabled {
            UiColor::PANEL_LIGHT
        } else {
            UiColor::PANEL_DARK
        },
        corner_radius: 4.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: rect.x + 8.0,
            y: rect.y + 5.0,
            width: (rect.width - 16.0).max(0.0),
            height: 14.0,
        },
        text: label.to_string(),
        color: if enabled {
            UiColor::TEXT
        } else {
            UiColor::TEXT_MUTED
        },
        size: 10.0,
    });
    interactions.push(super::widget_interaction(super::WidgetInteractionSpec {
        id: format!("hit.project_intent.{action_id}.{subject_id}"),
        rect,
        role: WidgetRole::Button,
        target: HitTarget::ProjectIntentAction {
            action_id: action_id.to_string(),
            subject_id: subject_id.to_string(),
        },
        enabled,
        command_id: action_id.to_string(),
        reason_disabled,
    }));
}
