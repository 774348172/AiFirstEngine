use editor_ui_model::ProjectRuntimeTrustPromptModel;

use crate::{
    ActivationPolicy, ControlPseudoStateSet, DrawCommand, EditorWidgetDeclaration, HitTarget,
    UiColor, UiDrawList, UiRect, UiRendererConfig, WidgetRole,
};

use super::resolve_and_paint_control;

pub(crate) fn push_project_runtime_trust_prompt(
    list: &mut UiDrawList,
    prompt: &ProjectRuntimeTrustPromptModel,
    width: f32,
    height: f32,
    config: &UiRendererConfig,
) -> (UiRect, Vec<EditorWidgetDeclaration>) {
    let modal_width = width.clamp(320.0, 620.0);
    let modal_height = height.clamp(280.0, 390.0);
    let rect = UiRect {
        x: ((width - modal_width) * 0.5).max(0.0),
        y: ((height - modal_height) * 0.5).max(0.0),
        width: modal_width,
        height: modal_height,
    };
    list.commands.push(DrawCommand::Rect {
        rect: UiRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        },
        color: UiColor {
            r: 6,
            g: 8,
            b: 12,
            a: 190,
        },
        corner_radius: 0.0,
    });
    list.commands.push(DrawCommand::Rect {
        rect,
        color: UiColor::PANEL,
        corner_radius: 6.0,
    });
    let lines = [
        ("Trust Project Runtime", UiColor::TEXT, 18.0),
        (prompt.project_name.as_str(), UiColor::ACCENT, 14.0),
        (
            prompt.canonical_project_root.as_str(),
            UiColor::TEXT_MUTED,
            11.0,
        ),
        (prompt.module_id.as_str(), UiColor::TEXT, 12.0),
    ];
    for (index, (text, color, size)) in lines.into_iter().enumerate() {
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: rect.x + 24.0,
                y: rect.y + 22.0 + index as f32 * 30.0,
                width: (rect.width - 48.0).max(0.0),
                height: 22.0,
            },
            text: text.to_string(),
            color,
            size,
        });
    }
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            x: rect.x + 24.0,
            y: rect.y + 148.0,
            width: (rect.width - 48.0).max(0.0),
            height: 70.0,
        },
        text: format!(
            "Native Rust code will run inside the Editor. Dependencies: {}{}",
            prompt.dependency_summary.join(", "),
            if prompt.identity_changed {
                " (identity changed)"
            } else {
                ""
            }
        ),
        color: if prompt.identity_changed {
            UiColor::WARNING
        } else {
            UiColor::TEXT_MUTED
        },
        size: 11.0,
    });
    let button_y = rect.y + rect.height - 52.0;
    let specs = [
        (
            "cancel",
            "Cancel",
            rect.x + rect.width - 300.0,
            UiColor::PANEL_DARK,
        ),
        (
            "deny",
            "Deny",
            rect.x + rect.width - 204.0,
            UiColor::PANEL_LIGHT,
        ),
        (
            "approve",
            "Approve",
            rect.x + rect.width - 108.0,
            UiColor::ACCENT,
        ),
    ];
    let mut interactions = Vec::new();
    for (action, label, x, _color) in specs {
        let button = UiRect {
            x,
            y: button_y,
            width: 84.0,
            height: 30.0,
        };
        let hit_id = format!("hit.project_runtime_trust.{action}");
        let model = ControlPseudoStateSet::empty()
            .with(crate::ControlPseudoState::Selected, action == "approve");
        let style = resolve_and_paint_control(
            list,
            button,
            WidgetRole::Button,
            "decision-control",
            config.control_pseudo_states(&hit_id, model, true),
        );
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: button.x + 10.0,
                y: button.y + 7.0,
                width: 64.0,
                height: 16.0,
            },
            text: label.to_string(),
            color: style.foreground,
            size: 11.0,
        });
        let mut interaction = super::widget_interaction(super::WidgetInteractionSpec {
            id: hit_id,
            rect: button,
            role: WidgetRole::Button,
            target: HitTarget::ProjectRuntimeTrustDecision {
                request_id: prompt.request_id.clone(),
                action: action.to_string(),
            },
            enabled: true,
            command_id: format!("{action}_project_runtime_trust"),
            reason_disabled: None,
        });
        interaction.control_classes = crate::ControlClassSet::new(["decision-control"]);
        interaction.model_pseudo_states = model;
        interaction.activation_policy = ActivationPolicy::ReleaseInside;
        interactions.push(interaction);
    }
    (rect, interactions)
}
