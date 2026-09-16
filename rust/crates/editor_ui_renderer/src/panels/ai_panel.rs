use editor_ui_model::{AiCommandReviewState, EditorUiModel};

use crate::panels::{widget_interaction, WidgetInteractionSpec};
use crate::{
    DrawCommand, EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect, UiRendererConfig,
    WidgetRole,
};

pub(crate) fn push_ai_panel(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
    _config: &UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let content = UiRect {
        x: rect.x,
        y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
        width: rect.width,
        height: (rect.height - 25.0).max(0.0),
    };
    let panel = UiRect {
        width: (content.width - 1.0).max(0.0),
        ..content
    };
    list.commands.push(DrawCommand::Rect {
        rect: panel,
        color: UiColor::PANEL_DARK,
        corner_radius: 0.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: panel.x + 8.0,
            y: panel.y + 5.0,
            width: panel.width - 16.0,
            height: 16.0,
        },
        text: "AI Panel".to_string(),
        color: UiColor::TEXT,
        size: 11.0,
    });
    let prompt = UiRect {
        x: panel.x + 8.0,
        y: panel.y + 26.0,
        width: (panel.width - 86.0).max(40.0),
        height: 22.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: prompt,
        color: UiColor::PANEL,
        corner_radius: 3.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: prompt.x + 6.0,
            y: prompt.y + 5.0,
            width: prompt.width - 12.0,
            height: 12.0,
        },
        text: if model.ai_panel.prompt_draft.is_empty() {
            model.ai_panel.prompt_placeholder.clone()
        } else {
            model.ai_panel.prompt_draft.clone()
        },
        color: if model.ai_panel.prompt_draft.is_empty() {
            UiColor::TEXT_MUTED
        } else {
            UiColor::TEXT
        },
        size: 10.0,
    });
    interactions.push(widget_interaction(WidgetInteractionSpec {
        id: "hit.ai_panel.prompt".to_string(),
        rect: prompt,
        role: WidgetRole::TextInput,
        target: HitTarget::AiPromptField,
        enabled: !model.ai_panel.busy,
        command_id: "set_ai_prompt_draft".to_string(),
        reason_disabled: model
            .ai_panel
            .busy
            .then(|| "Cancel the active request before editing the prompt.".to_string()),
    }));
    let submit = UiRect {
        x: panel.x + panel.width - 70.0,
        y: panel.y + 26.0,
        width: 58.0,
        height: 22.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: submit,
        color: UiColor::ACCENT,
        corner_radius: 3.0,
    });
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: submit.x + 7.0,
            y: submit.y + 5.0,
            width: submit.width - 14.0,
            height: 12.0,
        },
        text: if model.ai_panel.busy {
            "Cancel".to_string()
        } else {
            "Submit".to_string()
        },
        color: UiColor::TEXT,
        size: 10.0,
    });
    interactions.push(widget_interaction(WidgetInteractionSpec {
        id: "hit.ai_panel.submit".to_string(),
        rect: submit,
        role: WidgetRole::Button,
        target: HitTarget::AiPanelAction {
            action_id: if model.ai_panel.busy {
                "cancel".to_string()
            } else {
                format!("submit:{}", model.ai_panel.prompt_draft)
            },
        },
        enabled: model.ai_panel.busy || !model.ai_panel.prompt_draft.trim().is_empty(),
        command_id: if model.ai_panel.busy {
            "cancel_llm_patch_request"
        } else {
            "generate_project_patch_from_prompt"
        }
        .to_string(),
        reason_disabled: (!model.ai_panel.busy && model.ai_panel.prompt_draft.trim().is_empty())
            .then(|| "Enter a ProjectPatch request before submitting.".to_string()),
    }));

    let mut y = panel.y + 56.0;
    if let Some(status) = &model.ai_panel.status_summary {
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y,
                width: panel.width - 16.0,
                height: 16.0,
            },
            text: status.clone(),
            color: UiColor::TEXT_MUTED,
            size: 9.0,
        });
        y += 18.0;
    }
    for message in model.ai_panel.messages.iter().rev().take(2) {
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: panel.x + 8.0,
                y,
                width: panel.width - 16.0,
                height: 18.0,
            },
            text: message.text.clone(),
            color: UiColor::TEXT_MUTED,
            size: 10.0,
        });
        y += 20.0;
    }
    for proposal in model.ai_panel.proposed_commands.iter().take(3) {
        let row = UiRect {
            x: panel.x + 8.0,
            y,
            width: panel.width - 16.0,
            height: 28.0,
        };
        list.commands.push(DrawCommand::Rect {
            rect: row,
            color: if proposal.review_state == AiCommandReviewState::Proposed {
                UiColor::PANEL_LIGHT
            } else {
                UiColor::PANEL
            },
            corner_radius: 2.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: row.x + 6.0,
                y: row.y + 7.0,
                width: row.width - 70.0,
                height: 14.0,
            },
            text: proposal.label.clone(),
            color: UiColor::TEXT,
            size: 10.0,
        });
        let accept = UiRect {
            x: row.x + row.width - 62.0,
            y: row.y + 4.0,
            width: 26.0,
            height: 20.0,
        };
        let reject = UiRect {
            x: row.x + row.width - 32.0,
            y: row.y + 4.0,
            width: 24.0,
            height: 20.0,
        };
        list.commands.push(DrawCommand::Text {
            rect: accept,
            text: "OK".to_string(),
            color: UiColor::TEXT,
            size: 10.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: reject,
            text: "No".to_string(),
            color: UiColor::WARNING,
            size: 10.0,
        });
        let proposal_enabled = proposal.review_state == AiCommandReviewState::Proposed;
        let proposal_reason =
            (!proposal_enabled).then(|| "AI proposal is no longer proposed.".to_string());
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.ai_proposal.accept.{}", proposal.proposal_id),
            rect: accept,
            role: WidgetRole::Button,
            target: HitTarget::AiProposedCommand {
                proposal_id: proposal.proposal_id.clone(),
            },
            enabled: proposal_enabled,
            command_id: "ai_accept_proposed_command".to_string(),
            reason_disabled: proposal_reason.clone(),
        }));
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.ai_panel.reject.{}", proposal.proposal_id),
            rect: reject,
            role: WidgetRole::Button,
            target: HitTarget::AiPanelAction {
                action_id: format!("reject:{}", proposal.proposal_id),
            },
            enabled: proposal_enabled,
            command_id: "ai_reject_proposed_command".to_string(),
            reason_disabled: proposal_reason,
        }));
        y += 32.0;
    }
    interactions
}
