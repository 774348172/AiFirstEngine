use editor_ui_model::{AiCommandReviewState, EditorUiModel, GatewayAccessInboxModel};

use crate::panels::{widget_interaction, WidgetInteractionSpec};
use crate::{
    ActivationPolicy, ControlPseudoStateSet, DrawCommand, EditorWidgetDeclaration, HitTarget,
    UiColor, UiDrawList, UiRect, UiRendererConfig, WidgetRole,
};

pub(crate) fn push_ai_panel(
    list: &mut UiDrawList,
    rect: UiRect,
    model: &EditorUiModel,
    config: &UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    let mut interactions = Vec::new();
    let content = UiRect {
        x: rect.x,
        y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
        width: rect.width,
        height: (rect.height - 25.0).max(0.0),
    };
    let narrow = rect.width < 480.0;
    let has_gateway_access = model.ai_panel.gateway_access.total_count > 0;
    let panel = if narrow {
        UiRect {
            height: if has_gateway_access {
                (content.height * 0.42).max(96.0).min(content.height)
            } else {
                content.height
            },
            width: (content.width - 1.0).max(0.0),
            ..content
        }
    } else {
        UiRect {
            x: rect.x + rect.width * 0.74,
            y: content.y,
            width: rect.width * 0.26 - 1.0,
            height: content.height,
        }
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

    push_gateway_access_inbox(
        list,
        rect,
        &model.ai_panel.gateway_access,
        config,
        &mut interactions,
    );

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

fn push_gateway_access_inbox(
    list: &mut UiDrawList,
    rect: UiRect,
    gateway_access: &GatewayAccessInboxModel,
    config: &UiRendererConfig,
    interactions: &mut Vec<EditorWidgetDeclaration>,
) {
    if gateway_access.total_count == 0 {
        return;
    }

    let content_height = (rect.height - 25.0).max(0.0);
    let panel = if rect.width < 480.0 {
        let ai_height = (content_height * 0.42).max(96.0).min(content_height);
        UiRect {
            x: rect.x,
            y: rect.y + 25.0 + ai_height,
            width: (rect.width - 1.0).max(0.0),
            height: (content_height - ai_height - 1.0).max(0.0),
        }
    } else {
        UiRect {
            x: rect.x,
            y: rect.y + crate::metrics::EditorUiMetrics::PANEL_HEADER_HEIGHT,
            width: rect.width * 0.74 - 1.0,
            height: content_height,
        }
    };
    list.commands.push(DrawCommand::Rect {
        rect: panel,
        color: UiColor::PANEL_DARK,
        corner_radius: 0.0,
    });
    let header = UiRect {
        x: panel.x + 8.0,
        y: panel.y + 5.0,
        width: panel.width - 16.0,
        height: 16.0,
    };
    list.commands.push(DrawCommand::Text {
        rect: header,
        text: format!(
            "Codex access requests  {}/{}  ({} pending)",
            gateway_access.page_index.saturating_add(1),
            gateway_access.page_count.max(1),
            gateway_access.total_count
        ),
        color: UiColor::TEXT_MUTED,
        size: 9.0,
    });

    if gateway_access.page_count > 1 {
        let previous = UiRect {
            x: header.x + header.width - 42.0,
            y: header.y,
            width: 18.0,
            height: 16.0,
        };
        let next = UiRect {
            x: header.x + header.width - 20.0,
            y: header.y,
            width: 18.0,
            height: 16.0,
        };
        for (button_rect, label) in [(previous, "<"), (next, ">")] {
            list.commands.push(DrawCommand::Text {
                rect: button_rect,
                text: label.to_string(),
                color: UiColor::TEXT,
                size: 10.0,
            });
        }
        let previous_page = gateway_access.page_index.saturating_sub(1);
        let next_page =
            (gateway_access.page_index + 1).min(gateway_access.page_count.saturating_sub(1));
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.gateway_access.page.previous.{previous_page}"),
            rect: previous,
            role: WidgetRole::Button,
            target: HitTarget::GatewayAccessPage {
                page_index: previous_page,
            },
            enabled: gateway_access.page_index > 0,
            command_id: "set_gateway_access_page".to_string(),
            reason_disabled: (gateway_access.page_index == 0)
                .then(|| "Already on the first access-request page.".to_string()),
        }));
        interactions.push(widget_interaction(WidgetInteractionSpec {
            id: format!("hit.gateway_access.page.next.{next_page}"),
            rect: next,
            role: WidgetRole::Button,
            target: HitTarget::GatewayAccessPage {
                page_index: next_page,
            },
            enabled: gateway_access.page_index + 1 < gateway_access.page_count,
            command_id: "set_gateway_access_page".to_string(),
            reason_disabled: (gateway_access.page_index + 1 >= gateway_access.page_count)
                .then(|| "Already on the last access-request page.".to_string()),
        }));
    }

    let mut y = panel.y + 25.0;
    for request in &gateway_access.requests {
        let row = UiRect {
            x: panel.x + 8.0,
            y,
            width: panel.width - 16.0,
            height: 58.0,
        };
        list.commands.push(DrawCommand::Rect {
            rect: row,
            color: UiColor::PANEL_LIGHT,
            corner_radius: 2.0,
        });
        let action_width = 78.0;
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: row.x + 6.0,
                y: row.y + 3.0,
                width: row.width - action_width - 12.0,
                height: 11.0,
            },
            text: format!(
                "{} {} [{}] | op {} | {} | {} | age {} ms | expires {} ms",
                request.client_kind,
                request.client_version,
                request.session_short_id,
                request.operation_short_id,
                request.project_identity,
                request.state,
                request.connected_age_ms,
                request.expires_in_ms
            ),
            color: UiColor::TEXT,
            size: 8.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: row.x + 6.0,
                y: row.y + 17.0,
                width: row.width - action_width - 12.0,
                height: 10.0,
            },
            text: format!(
                "{} | risk {} | allow: {} | blocked: {}",
                request.requested_profile,
                request.risk_class,
                request.capabilities.join(","),
                request.blocked_capabilities.join(",")
            ),
            color: UiColor::TEXT_MUTED,
            size: 8.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: row.x + 6.0,
                y: row.y + 29.0,
                width: row.width - action_width - 12.0,
                height: 10.0,
            },
            text: format!(
                "goal {} | {} | {}",
                request.goal_id, request.completion_policy, request.user_visible_outcome
            ),
            color: UiColor::TEXT,
            size: 8.0,
        });
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: row.x + 6.0,
                y: row.y + 41.0,
                width: row.width - action_width - 12.0,
                height: 10.0,
            },
            text: format!(
                "paths +[{}] -[{}] objects [{}] | budget m:{} t:{}ms c:{} | flags d:{} dep:{} net:{}",
                request.allowed_paths.join(","),
                request.denied_paths.join(","),
                request.allowed_objects.join(","),
                request.max_mutation_count,
                request.time_budget_ms,
                request.external_cost_budget_microunits,
                request.allow_delete,
                request.allow_dependency_change,
                request.allow_network
            ),
            color: UiColor::TEXT_MUTED,
            size: 8.0,
        });
        let approve = UiRect {
            x: row.x + row.width - 74.0,
            y: row.y + 17.0,
            width: 32.0,
            height: 24.0,
        };
        let reject = UiRect {
            x: row.x + row.width - 38.0,
            y: row.y + 17.0,
            width: 32.0,
            height: 24.0,
        };
        let enabled = request.state == "awaiting_user";
        for (button_rect, label, hit_id) in [
            (
                approve,
                "OK",
                format!("hit.gateway_access.approve.{}", request.request_id),
            ),
            (
                reject,
                "No",
                format!("hit.gateway_access.reject.{}", request.request_id),
            ),
        ] {
            let style = super::resolve_and_paint_control(
                list,
                button_rect,
                WidgetRole::Button,
                "decision-control",
                config.control_pseudo_states(&hit_id, ControlPseudoStateSet::empty(), enabled),
            );
            list.commands.push(DrawCommand::Text {
                rect: button_rect,
                text: label.to_string(),
                color: style.foreground,
                size: 9.0,
            });
        }
        let mut approve_interaction = widget_interaction(WidgetInteractionSpec {
            id: format!("hit.gateway_access.approve.{}", request.request_id),
            rect: approve,
            role: WidgetRole::Button,
            target: HitTarget::GatewayAccessDecision {
                request_id: request.request_id.clone(),
                approved: true,
            },
            enabled: request.state == "awaiting_user",
            command_id: "approve_gateway_access_request".to_string(),
            reason_disabled: (request.state != "awaiting_user")
                .then(|| "Gateway access request is no longer awaiting a decision.".to_string()),
        });
        approve_interaction.control_classes = crate::ControlClassSet::new(["decision-control"]);
        approve_interaction.activation_policy = ActivationPolicy::ReleaseInside;
        interactions.push(approve_interaction);
        let mut reject_interaction = widget_interaction(WidgetInteractionSpec {
            id: format!("hit.gateway_access.reject.{}", request.request_id),
            rect: reject,
            role: WidgetRole::Button,
            target: HitTarget::GatewayAccessDecision {
                request_id: request.request_id.clone(),
                approved: false,
            },
            enabled: request.state == "awaiting_user",
            command_id: "reject_gateway_access_request".to_string(),
            reason_disabled: (request.state != "awaiting_user")
                .then(|| "Gateway access request is no longer awaiting a decision.".to_string()),
        });
        reject_interaction.control_classes = crate::ControlClassSet::new(["decision-control"]);
        reject_interaction.activation_policy = ActivationPolicy::ReleaseInside;
        interactions.push(reject_interaction);
        y += 61.0;
    }
}
