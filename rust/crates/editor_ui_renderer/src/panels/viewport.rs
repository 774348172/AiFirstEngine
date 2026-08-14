use editor_ui_model::{EditorGameViewScalePolicy, EditorGameViewTarget, GameViewLayoutState};

use crate::{
    ActivationPolicy, ControlPseudoState, ControlPseudoStateSet, DrawCommand, EditorCommandBinding,
    EditorWidgetAction, EditorWidgetDeclaration, HitTarget, UiColor, UiDrawList, UiRect,
    UiRendererConfig, WidgetId, WidgetRole,
};

use super::resolve_and_paint_control;

pub(crate) fn push_viewport_header(
    list: &mut UiDrawList,
    rect: UiRect,
    layout: &GameViewLayoutState,
    config: &UiRendererConfig,
) -> Vec<EditorWidgetDeclaration> {
    let header = UiRect {
        x: rect.x + 1.0,
        y: rect.y + 22.0,
        width: (rect.width - 2.0).max(0.0),
        height: 22.0,
    };
    list.commands.push(DrawCommand::Rect {
        rect: header,
        color: UiColor::TAB,
        corner_radius: 0.0,
    });

    let show_display_label = header.width >= 380.0;
    if show_display_label {
        list.commands.push(DrawCommand::Text {
            rect: UiRect {
                x: header.x + 6.0,
                y: header.y + 4.0,
                width: 58.0,
                height: 14.0,
            },
            text: "Display 1".to_string(),
            color: UiColor::TEXT_MUTED,
            size: 10.0,
        });
    }

    let mut declarations = Vec::new();
    let mut x = header.x + if show_display_label { 66.0 } else { 4.0 };
    let right = header.x + header.width - 4.0;
    let target_controls = [
        ("1280x720", 58.0, 1280, 720),
        ("1080x1920", 68.0, 1080, 1920),
        ("720x1280", 62.0, 720, 1280),
    ];
    for (label, width, target_width, target_height) in target_controls {
        if x + width > right {
            break;
        }
        let target = EditorGameViewTarget::new(
            target_width,
            target_height,
            EditorGameViewScalePolicy::Contain,
        );
        let control_id = format!("preset/{target_width}x{target_height}");
        declarations.push(push_target_control(
            list,
            config,
            TargetControlSpec {
                rect: UiRect {
                    x,
                    y: header.y + 2.0,
                    width,
                    height: 18.0,
                },
                control_id: &control_id,
                label,
                target,
                selected: layout.target.width == target_width
                    && layout.target.height == target_height,
                enabled: layout.target_editable,
                index: declarations.len(),
            },
        ));
        x += width + 3.0;
    }

    for (label, width, policy) in [
        ("Contain", 48.0, EditorGameViewScalePolicy::Contain),
        ("Stretch", 48.0, EditorGameViewScalePolicy::Stretch),
    ] {
        if x + width > right {
            break;
        }
        let target = EditorGameViewTarget::new(layout.target.width, layout.target.height, policy);
        let control_id = format!(
            "policy/{}",
            match policy {
                EditorGameViewScalePolicy::Contain => "contain",
                EditorGameViewScalePolicy::Stretch => "stretch",
            }
        );
        declarations.push(push_target_control(
            list,
            config,
            TargetControlSpec {
                rect: UiRect {
                    x,
                    y: header.y + 2.0,
                    width,
                    height: 18.0,
                },
                control_id: &control_id,
                label,
                target,
                selected: layout.target.scale_policy == policy,
                enabled: layout.target_editable,
                index: declarations.len(),
            },
        ));
        x += width + 3.0;
    }

    declarations
}

struct TargetControlSpec<'a> {
    rect: UiRect,
    control_id: &'a str,
    label: &'a str,
    target: EditorGameViewTarget,
    selected: bool,
    enabled: bool,
    index: usize,
}

fn push_target_control(
    list: &mut UiDrawList,
    config: &UiRendererConfig,
    spec: TargetControlSpec<'_>,
) -> EditorWidgetDeclaration {
    let TargetControlSpec {
        rect,
        control_id,
        label,
        target,
        selected,
        enabled,
        index,
    } = spec;
    let hit_id = format!(
        "hit.viewport.game_view_target.{}",
        control_id.replace('/', ".")
    );
    let model_pseudo = ControlPseudoStateSet::empty().with(ControlPseudoState::Selected, selected);
    let style = resolve_and_paint_control(
        list,
        rect,
        WidgetRole::Button,
        "game-view-target-control",
        config.control_pseudo_states(&hit_id, model_pseudo, enabled),
    );
    list.commands.push(DrawCommand::Text {
        rect: UiRect {
            y: rect.y + 3.0 + style.content_offset.y,
            height: 12.0,
            ..rect
        },
        text: label.to_string(),
        color: style.foreground,
        size: 9.0,
    });

    let mut declaration = EditorWidgetDeclaration::new(
        WidgetId::semantic(format!(
            "editor/panel/viewport/game-view-target/{control_id}"
        ))
        .expect("game view target widget id must be semantic"),
        WidgetRole::Button,
    );
    declaration.style.absolute = true;
    declaration.style.inset_left = Some(rect.x);
    declaration.style.inset_top = Some(rect.y);
    declaration.style.width = Some(rect.width);
    declaration.style.height = Some(rect.height);
    declaration.style.z_index = 90_100 + index as i32;
    declaration.enabled = enabled;
    declaration.hit_region_id = Some(hit_id);
    declaration.binding = Some(EditorCommandBinding {
        action: EditorWidgetAction::Activate,
        command_id: "set_game_view_target".to_string(),
        target: HitTarget::GameViewTarget {
            width: target.width,
            height: target.height,
            scale_policy: target.scale_policy,
        },
        reason_disabled: (!enabled)
            .then(|| "Stop Play before changing the GameView target.".to_string()),
    });
    declaration.control_classes = crate::ControlClassSet::new(["game-view-target-control"]);
    declaration.model_pseudo_states = model_pseudo;
    declaration.activation_policy = ActivationPolicy::ReleaseInside;
    declaration
}
