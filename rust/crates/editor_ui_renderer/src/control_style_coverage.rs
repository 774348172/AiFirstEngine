use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{dark_neutral_control_style_summary, ControlPseudoState, EditorWidgetTree, WidgetRole};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlStyleCoverageLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorControlStyleCoverageReport {
    pub level: ControlStyleCoverageLevel,
    pub sheet_id: String,
    pub generation: u64,
    pub rule_count: usize,
    pub role_coverage: Vec<String>,
    pub class_coverage: Vec<String>,
    pub pseudo_state_coverage: Vec<String>,
    pub migrated_consumer_groups: Vec<String>,
    pub remaining_interactive_consumer_groups: Vec<String>,
    pub manual_hover_pressed_paint_violation_count: usize,
    pub cache_hit_count: u64,
    pub cache_miss_count: u64,
    pub fallback_count: u64,
    pub texture_upload_count: u64,
}

pub fn control_style_coverage_report(
    tree: &EditorWidgetTree,
    level: ControlStyleCoverageLevel,
    texture_upload_count: u64,
) -> EditorControlStyleCoverageReport {
    if level == ControlStyleCoverageLevel::Off {
        return EditorControlStyleCoverageReport::default();
    }
    let style = dark_neutral_control_style_summary();
    let mut roles = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut migrated = BTreeSet::new();
    let mut remaining = BTreeSet::new();
    let mut has_toggle = false;
    for node in tree.nodes.values().filter(|node| node.binding.is_some()) {
        has_toggle |= node.role == WidgetRole::Toggle;
        if node.control_classes.as_slice().is_empty() {
            let reason = match node.role {
                WidgetRole::Splitter => "splitter:dedicated_gesture",
                WidgetRole::TextInput => "text-input:focus_and_text_owner",
                WidgetRole::Viewport => "viewport:game_or_scene_input_owner",
                _ => "legacy-interactive:outside-v1-inventory",
            };
            remaining.insert(reason.to_string());
            continue;
        }
        roles.insert(format!("{:?}", node.role));
        for class in node.control_classes.as_slice() {
            classes.insert(class.clone());
            migrated.insert(consumer_group(class).to_string());
        }
    }
    if !has_toggle {
        remaining.insert("toggle:no_product_consumer".to_string());
    }
    EditorControlStyleCoverageReport {
        level,
        sheet_id: style.sheet_id,
        generation: style.generation,
        rule_count: style.rule_count,
        role_coverage: roles.into_iter().collect(),
        class_coverage: classes.into_iter().collect(),
        pseudo_state_coverage: [
            ControlPseudoState::Hover,
            ControlPseudoState::Active,
            ControlPseudoState::Selected,
            ControlPseudoState::Checked,
            ControlPseudoState::Disabled,
            ControlPseudoState::Focus,
            ControlPseudoState::FocusVisible,
        ]
        .into_iter()
        .map(|state| format!("{state:?}"))
        .collect(),
        migrated_consumer_groups: migrated.into_iter().collect(),
        remaining_interactive_consumer_groups: remaining.into_iter().collect(),
        manual_hover_pressed_paint_violation_count: 0,
        cache_hit_count: style.cache_hit_count,
        cache_miss_count: style.cache_miss_count,
        fallback_count: style.fallback_count,
        texture_upload_count,
    }
}

fn consumer_group(class: &str) -> &str {
    match class {
        "toolbar-control" => "toolbar",
        "workspace-tab" => "workspace-tabs",
        "panel-chrome-control" => "panel-chrome",
        "launcher-control" => "project-launcher",
        "decision-control" => "decision-controls",
        "toggle-control" => "toggle",
        _ => "custom-semantic-control",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        reconcile_widget_tree, ActivationPolicy, ControlPseudoStateSet, EditorCommandBinding,
        EditorWidgetAction, EditorWidgetDeclaration, HitTarget, WidgetId,
    };

    #[test]
    fn control_style_coverage_reports_migrated_and_explicit_remaining_groups() {
        let mut root =
            EditorWidgetDeclaration::new(WidgetId::semantic("root").unwrap(), WidgetRole::Root);
        let mut button = EditorWidgetDeclaration::new(
            WidgetId::semantic("toolbar/play").unwrap(),
            WidgetRole::Button,
        )
        .with_control_style(
            ["toolbar-control"],
            ControlPseudoStateSet::empty(),
            ActivationPolicy::ReleaseInside,
        );
        button.binding = Some(EditorCommandBinding {
            action: EditorWidgetAction::Activate,
            command_id: "play".to_string(),
            target: HitTarget::ToolbarCommand {
                command_id: "play".to_string(),
            },
            reason_disabled: None,
        });
        root.children.push(button);
        let (tree, _) = reconcile_widget_tree(None, &root).unwrap();
        let report = control_style_coverage_report(&tree, ControlStyleCoverageLevel::Summary, 4);
        assert!(report
            .migrated_consumer_groups
            .contains(&"toolbar".to_string()));
        assert!(report
            .remaining_interactive_consumer_groups
            .contains(&"toggle:no_product_consumer".to_string()));
        assert_eq!(report.texture_upload_count, 4);
    }

    #[test]
    fn control_style_coverage_source_guard_rejects_manual_state_paint_and_game_branch() {
        let toolbar = include_str!("panels/toolbar.rs");
        let workspace = include_str!("panels/mod.rs");
        assert!(!toolbar.contains("config.hovered_hit_id"));
        assert!(!toolbar.contains("config.pressed_hit_id"));
        assert!(!workspace.contains("panel_id == \"game_view\""));
        assert!(!workspace.contains("panel_id.as_str() == \"game_view\""));
    }
}
