use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::aui::{
    builtin_button_feedback_profile_v1, AuiCommand, AuiCommandKind, AuiControlInteractionSnapshot,
    AuiDocument, AuiFeedbackEasing, AuiInteractionFeedbackProfile, AuiNodeKind, AuiVec2,
};

const MAX_PRESENTATION_DELTA_US: u64 = 250_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuiFeedbackDiagnosticsLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AuiControlVisualOverride {
    pub scale: f32,
    pub translation: AuiVec2,
    pub brightness_multiplier: f32,
    pub opacity_multiplier: f32,
}

impl Default for AuiControlVisualOverride {
    fn default() -> Self {
        Self {
            scale: 1.0,
            translation: AuiVec2::default(),
            brightness_multiplier: 1.0,
            opacity_multiplier: 1.0,
        }
    }
}

impl AuiControlVisualOverride {
    pub fn is_identity(self) -> bool {
        (self.scale - 1.0).abs() < f32::EPSILON
            && self.translation.x.abs() < f32::EPSILON
            && self.translation.y.abs() < f32::EPSILON
            && (self.brightness_multiplier - 1.0).abs() < f32::EPSILON
            && (self.opacity_multiplier - 1.0).abs() < f32::EPSILON
    }

    fn lerp(self, target: Self, progress: f32) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        Self {
            scale: self.scale + (target.scale - self.scale) * progress,
            translation: AuiVec2::new(
                self.translation.x + (target.translation.x - self.translation.x) * progress,
                self.translation.y + (target.translation.y - self.translation.y) * progress,
            ),
            brightness_multiplier: self.brightness_multiplier
                + (target.brightness_multiplier - self.brightness_multiplier) * progress,
            opacity_multiplier: self.opacity_multiplier
                + (target.opacity_multiplier - self.opacity_multiplier) * progress,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiVisualOverrideSet {
    overrides: BTreeMap<String, AuiControlVisualOverride>,
}

impl AuiVisualOverrideSet {
    pub fn get(&self, node_id: &str) -> Option<&AuiControlVisualOverride> {
        self.overrides.get(node_id)
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &AuiControlVisualOverride)> {
        self.overrides
            .iter()
            .map(|(id, visual)| (id.as_str(), visual))
    }

    pub fn set(&mut self, node_id: impl Into<String>, visual: AuiControlVisualOverride) {
        self.overrides.insert(node_id.into(), visual);
    }

    fn insert(&mut self, node_id: String, visual: AuiControlVisualOverride) {
        self.overrides.insert(node_id, visual);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuiControlFeedbackReport {
    pub frame_id: u64,
    pub resolved_node_count: usize,
    pub override_count: usize,
    pub builtin_profile_resolve_count: usize,
    pub project_profile_resolve_count: usize,
    pub fallback_profile_resolve_count: usize,
    pub activation_count: usize,
    pub reclaimed_state_count: usize,
    pub session_reconciliation_count: usize,
    pub scope_reconciliation_count: usize,
    pub time_reversal_count: usize,
    pub large_delta_clamp_count: usize,
    pub invalid_value_recovery_count: usize,
    pub resolved_profile_ids: BTreeSet<String>,
    pub traces: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiControlFeedbackFrame {
    pub overrides: AuiVisualOverrideSet,
    pub report: AuiControlFeedbackReport,
}

#[derive(Debug, Clone, Copy)]
pub struct AuiControlFeedbackFrameInput<'a> {
    pub document: &'a AuiDocument,
    pub interaction: &'a AuiControlInteractionSnapshot,
    pub commands: &'a [AuiCommand],
    pub presentation_delta_us: u64,
    pub diagnostics: AuiFeedbackDiagnosticsLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AuiControlBaseState {
    Normal,
    Hovered,
    Pressed,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AuiControlFeedbackNodeState {
    current: AuiControlVisualOverride,
    from: AuiControlVisualOverride,
    target: AuiControlVisualOverride,
    elapsed_us: u64,
    duration_us: u64,
    base_state: AuiControlBaseState,
    activation_remaining_us: u64,
    activation_presented_frame: Option<u64>,
}

impl Default for AuiControlFeedbackNodeState {
    fn default() -> Self {
        let identity = AuiControlVisualOverride::default();
        Self {
            current: identity,
            from: identity,
            target: identity,
            elapsed_us: 0,
            duration_us: 0,
            base_state: AuiControlBaseState::Normal,
            activation_remaining_us: 0,
            activation_presented_frame: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuiControlFeedbackState {
    nodes: BTreeMap<String, AuiControlFeedbackNodeState>,
    session_id: Option<String>,
    active_modal_root: Option<String>,
    active_screen_id: Option<String>,
    scope_initialized: bool,
    last_frame_id: Option<u64>,
}

impl AuiControlFeedbackState {
    pub fn active_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn presentation_delta_us_from_seconds(seconds: f32) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (f64::from(seconds) * 1_000_000.0)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
}

pub struct AuiControlFeedbackModule;

impl AuiControlFeedbackModule {
    pub fn advance(
        state: &mut AuiControlFeedbackState,
        input: AuiControlFeedbackFrameInput<'_>,
    ) -> AuiControlFeedbackFrame {
        let mut frame = AuiControlFeedbackFrame::default();
        frame.report.frame_id = input.interaction.frame_id;

        if state.session_id != input.interaction.session_id {
            if state.session_id.is_some() {
                frame.report.session_reconciliation_count = 1;
                frame.report.reclaimed_state_count += state.nodes.len();
            }
            state.nodes.clear();
            state.session_id = input.interaction.session_id.clone();
        }
        if state.scope_initialized
            && (state.active_modal_root != input.interaction.active_modal_root
                || state.active_screen_id != input.interaction.active_screen_id)
        {
            frame.report.scope_reconciliation_count = 1;
            frame.report.reclaimed_state_count += state.nodes.len();
            state.nodes.clear();
        }
        state.active_modal_root = input.interaction.active_modal_root.clone();
        state.active_screen_id = input.interaction.active_screen_id.clone();
        state.scope_initialized = true;
        if state
            .last_frame_id
            .is_some_and(|last| input.interaction.frame_id < last)
        {
            frame.report.time_reversal_count = 1;
        }
        state.last_frame_id = Some(input.interaction.frame_id);

        let delta_us = input.presentation_delta_us.min(MAX_PRESENTATION_DELTA_US);
        frame.report.large_delta_clamp_count =
            usize::from(input.presentation_delta_us > MAX_PRESENTATION_DELTA_US);

        let eligible: BTreeSet<&str> = input
            .document
            .nodes
            .iter()
            .filter(|node| {
                node.kind == AuiNodeKind::Button && node.visible && !node.feedback.is_none()
            })
            .map(|node| node.node_id.as_str())
            .collect();
        let stale: Vec<String> = state
            .nodes
            .keys()
            .filter(|node_id| !eligible.contains(node_id.as_str()))
            .cloned()
            .collect();
        for node_id in stale {
            state.nodes.remove(&node_id);
            frame.report.reclaimed_state_count += 1;
        }

        let activated: BTreeSet<&str> = input
            .commands
            .iter()
            .filter(|command| {
                matches!(
                    command.command_kind,
                    AuiCommandKind::Click | AuiCommandKind::Submit
                )
            })
            .map(|command| command.source_node.as_str())
            .collect();
        let cancelled: BTreeSet<&str> = input
            .commands
            .iter()
            .filter(|command| {
                matches!(
                    command.command_kind,
                    AuiCommandKind::PointerCancel
                        | AuiCommandKind::Cancel
                        | AuiCommandKind::DragCancel
                )
            })
            .map(|command| command.source_node.as_str())
            .collect();

        for node in input.document.nodes.iter().filter(|node| {
            node.kind == AuiNodeKind::Button && node.visible && !node.feedback.is_none()
        }) {
            let (profile, resolution) = resolve_profile(input.document, node.feedback.as_str());
            match resolution {
                ProfileResolution::Builtin => frame.report.builtin_profile_resolve_count += 1,
                ProfileResolution::Project => frame.report.project_profile_resolve_count += 1,
                ProfileResolution::Fallback => frame.report.fallback_profile_resolve_count += 1,
            }
            frame
                .report
                .resolved_profile_ids
                .insert(profile.profile_id.clone());
            frame.report.resolved_node_count += 1;

            let base_state = if !node.interactable {
                AuiControlBaseState::Disabled
            } else if input.interaction.pressed_inside
                && input.interaction.pressed_node.as_deref() == Some(node.node_id.as_str())
            {
                AuiControlBaseState::Pressed
            } else if input.interaction.hovered_node.as_deref() == Some(node.node_id.as_str()) {
                AuiControlBaseState::Hovered
            } else {
                AuiControlBaseState::Normal
            };
            let focus_visible = input.interaction.focus_visible
                && input.interaction.focused_node.as_deref() == Some(node.node_id.as_str());
            let is_activated = activated.contains(node.node_id.as_str()) && node.interactable;
            let needs_state = base_state != AuiControlBaseState::Normal
                || focus_visible
                || is_activated
                || state.nodes.contains_key(node.node_id.as_str());
            if !needs_state {
                continue;
            }

            let node_state = state.nodes.entry(node.node_id.clone()).or_default();
            if is_activated {
                node_state.activation_remaining_us = u64::from(profile.activated_ms) * 1_000;
                node_state.activation_presented_frame = None;
                frame.report.activation_count += 1;
            }
            let mut target = visual_for_base_state(&profile, base_state);
            if focus_visible && base_state != AuiControlBaseState::Disabled {
                target.brightness_multiplier += 0.02;
            }
            let activation_visible = node_state.activation_remaining_us > 0
                && node_state.activation_presented_frame != Some(input.interaction.frame_id);
            if activation_visible {
                target.scale *= f32::from(profile.activated_scale_permille) / 1000.0;
                target.brightness_multiplier +=
                    f32::from(profile.activated_brightness_permille) / 1000.0;
                target.opacity_multiplier *= f32::from(profile.activated_opacity_permille) / 1000.0;
                node_state.activation_presented_frame = Some(input.interaction.frame_id);
            }
            target = sanitize_visual(target, &mut frame.report.invalid_value_recovery_count);

            if target != node_state.target || base_state != node_state.base_state {
                node_state.from = node_state.current;
                node_state.target = target;
                node_state.elapsed_us = 0;
                node_state.duration_us = if motion_scale(input.document) == 0 {
                    0
                } else if activation_visible {
                    u64::from(profile.activated_ms)
                        * 1_000
                        * u64::from(motion_scale(input.document))
                        / 1000
                } else if cancelled.contains(node.node_id.as_str()) {
                    u64::from(profile.cancel_ms) * 1_000 * u64::from(motion_scale(input.document))
                        / 1000
                } else {
                    transition_duration_us(&profile, node_state.base_state, base_state)
                        * u64::from(motion_scale(input.document))
                        / 1000
                };
                node_state.base_state = base_state;
            }
            node_state.elapsed_us = node_state.elapsed_us.saturating_add(delta_us);
            let progress = if node_state.duration_us == 0 {
                1.0
            } else {
                node_state.elapsed_us as f32 / node_state.duration_us as f32
            };
            let easing = if activation_visible {
                profile.activated_easing
            } else {
                easing_for_state(&profile, base_state)
            };
            let eased = ease(progress, easing);
            node_state.current = node_state.from.lerp(node_state.target, eased);
            if activation_visible {
                node_state.activation_remaining_us =
                    node_state.activation_remaining_us.saturating_sub(delta_us);
            }
            if !node_state.current.is_identity() || activation_visible {
                frame
                    .overrides
                    .insert(node.node_id.clone(), node_state.current);
            }
        }

        let settled: Vec<String> = state
            .nodes
            .iter()
            .filter(|(_, node)| {
                node.base_state == AuiControlBaseState::Normal
                    && node.activation_remaining_us == 0
                    && node.current.is_identity()
            })
            .map(|(node_id, _)| node_id.clone())
            .collect();
        for node_id in settled {
            state.nodes.remove(&node_id);
            frame.report.reclaimed_state_count += 1;
        }
        frame.report.override_count = frame.overrides.len();
        if input.diagnostics == AuiFeedbackDiagnosticsLevel::Trace {
            frame.report.traces = frame
                .overrides
                .iter()
                .map(|(node_id, visual)| {
                    format!(
                        "{node_id}:scale={:.4}:opacity={:.4}",
                        visual.scale, visual.opacity_multiplier
                    )
                })
                .collect();
        }
        frame
    }
}

#[derive(Clone, Copy)]
enum ProfileResolution {
    Builtin,
    Project,
    Fallback,
}

fn motion_scale(document: &AuiDocument) -> u16 {
    document
        .interaction_feedback
        .as_ref()
        .map(|registry| registry.motion_scale_permille)
        .unwrap_or(1000)
}

fn resolve_profile(
    document: &AuiDocument,
    selection: &str,
) -> (AuiInteractionFeedbackProfile, ProfileResolution) {
    let registry = document.interaction_feedback.as_ref();
    let requested = if selection == "auto" {
        registry.and_then(|registry| registry.default_button_profile.as_deref())
    } else {
        Some(selection)
    };
    if let Some(profile_id) = requested {
        if let Some(profile) = registry.and_then(|registry| {
            registry
                .profiles
                .iter()
                .find(|profile| profile.profile_id == profile_id)
        }) {
            return (profile.clone(), ProfileResolution::Project);
        }
        return (
            builtin_button_feedback_profile_v1(),
            ProfileResolution::Fallback,
        );
    }
    (
        builtin_button_feedback_profile_v1(),
        ProfileResolution::Builtin,
    )
}

fn visual_for_base_state(
    profile: &AuiInteractionFeedbackProfile,
    state: AuiControlBaseState,
) -> AuiControlVisualOverride {
    match state {
        AuiControlBaseState::Normal => AuiControlVisualOverride::default(),
        AuiControlBaseState::Hovered => AuiControlVisualOverride {
            scale: f32::from(profile.hover_scale_permille) / 1000.0,
            brightness_multiplier: 1.0 + f32::from(profile.hover_brightness_permille) / 1000.0,
            opacity_multiplier: f32::from(profile.hover_opacity_permille) / 1000.0,
            ..Default::default()
        },
        AuiControlBaseState::Pressed => AuiControlVisualOverride {
            scale: f32::from(profile.pressed_scale_permille) / 1000.0,
            translation: profile.pressed_offset,
            brightness_multiplier: 1.0 + f32::from(profile.pressed_brightness_permille) / 1000.0,
            opacity_multiplier: f32::from(profile.pressed_opacity_permille) / 1000.0,
        },
        AuiControlBaseState::Disabled => AuiControlVisualOverride {
            opacity_multiplier: f32::from(profile.disabled_opacity_permille) / 1000.0,
            ..Default::default()
        },
    }
}

fn transition_duration_us(
    profile: &AuiInteractionFeedbackProfile,
    previous: AuiControlBaseState,
    next: AuiControlBaseState,
) -> u64 {
    u64::from(match next {
        AuiControlBaseState::Hovered => profile.hover_in_ms,
        AuiControlBaseState::Pressed => profile.press_in_ms,
        AuiControlBaseState::Disabled => 0,
        AuiControlBaseState::Normal if previous == AuiControlBaseState::Pressed => {
            profile.release_ms
        }
        AuiControlBaseState::Normal => profile.hover_out_ms,
    }) * 1_000
}

fn easing_for_state(
    profile: &AuiInteractionFeedbackProfile,
    state: AuiControlBaseState,
) -> AuiFeedbackEasing {
    match state {
        AuiControlBaseState::Hovered => profile.hover_easing,
        AuiControlBaseState::Pressed => profile.press_easing,
        AuiControlBaseState::Normal | AuiControlBaseState::Disabled => profile.release_easing,
    }
}

fn ease(progress: f32, easing: AuiFeedbackEasing) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    match easing {
        AuiFeedbackEasing::Linear => progress,
        AuiFeedbackEasing::EaseOutCubic => 1.0 - (1.0 - progress).powi(3),
        AuiFeedbackEasing::EaseOutBack => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            1.0 + c3 * (progress - 1.0).powi(3) + c1 * (progress - 1.0).powi(2)
        }
    }
}

fn sanitize_visual(
    mut visual: AuiControlVisualOverride,
    recovery_count: &mut usize,
) -> AuiControlVisualOverride {
    if !visual.scale.is_finite() {
        visual.scale = 1.0;
        *recovery_count += 1;
    }
    if !visual.translation.x.is_finite() {
        visual.translation.x = 0.0;
        *recovery_count += 1;
    }
    if !visual.translation.y.is_finite() {
        visual.translation.y = 0.0;
        *recovery_count += 1;
    }
    if !visual.brightness_multiplier.is_finite() {
        visual.brightness_multiplier = 1.0;
        *recovery_count += 1;
    }
    if !visual.opacity_multiplier.is_finite() {
        visual.opacity_multiplier = 1.0;
        *recovery_count += 1;
    }
    visual.scale = visual.scale.clamp(0.01, 8.0);
    visual.brightness_multiplier = visual.brightness_multiplier.clamp(0.0, 8.0);
    visual.opacity_multiplier = visual.opacity_multiplier.clamp(0.0, 1.0);
    visual
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui::{AuiFeedbackSelection, AuiNode, AuiRect};

    fn button_document() -> AuiDocument {
        let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        let mut button = AuiNode::new("button", AuiNodeKind::Button, AuiRect::stretch_full())
            .with_parent("root")
            .with_interactable(true);
        button.feedback = AuiFeedbackSelection::auto();
        AuiDocument::new("feedback", Vec::new(), vec![root, button])
    }

    fn command(source_node: &str, command_kind: AuiCommandKind) -> AuiCommand {
        AuiCommand {
            command_id: "test".to_string(),
            source_node: source_node.to_string(),
            command_kind,
            payload: None,
        }
    }

    #[test]
    fn aui_control_feedback_zero_config_resolves_builtin_and_animates_pressed() {
        let document = button_document();
        let snapshot = AuiControlInteractionSnapshot {
            frame_id: 1,
            pressed_node: Some("button".to_string()),
            pressed_inside: true,
            ..Default::default()
        };
        let mut state = AuiControlFeedbackState::default();
        let frame = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[],
                presentation_delta_us: 45_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        let visual = frame.overrides.get("button").expect("button override");
        assert_eq!(frame.report.builtin_profile_resolve_count, 1);
        assert!((visual.scale - 0.97).abs() < 0.001);
        assert!((visual.translation.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn aui_control_feedback_none_has_no_state_and_click_is_visible_one_frame() {
        let mut document = button_document();
        document.nodes[1].feedback = AuiFeedbackSelection::none();
        let snapshot = AuiControlInteractionSnapshot {
            frame_id: 1,
            ..Default::default()
        };
        let mut state = AuiControlFeedbackState::default();
        let none = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[],
                presentation_delta_us: 16_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Off,
            },
        );
        assert!(none.overrides.is_empty());
        assert_eq!(state.active_node_count(), 0);

        document.nodes[1].feedback = AuiFeedbackSelection::auto();
        let activated = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[command("button", AuiCommandKind::Click)],
                presentation_delta_us: 300_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        assert!(activated.overrides.get("button").unwrap().scale > 1.0);
        assert_eq!(activated.report.activation_count, 1);
        assert_eq!(activated.report.large_delta_clamp_count, 1);
    }

    #[test]
    fn aui_control_feedback_motion_zero_and_session_reconcile_are_deterministic() {
        let mut document = button_document();
        document.interaction_feedback = Some(Default::default());
        document
            .interaction_feedback
            .as_mut()
            .unwrap()
            .motion_scale_permille = 0;
        let snapshot = AuiControlInteractionSnapshot {
            frame_id: 3,
            session_id: Some("a".to_string()),
            hovered_node: Some("button".to_string()),
            ..Default::default()
        };
        let mut state = AuiControlFeedbackState::default();
        let first = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[],
                presentation_delta_us: 1,
                diagnostics: AuiFeedbackDiagnosticsLevel::Trace,
            },
        );
        assert!((first.overrides.get("button").unwrap().scale - 1.01).abs() < 0.001);
        let first_bytes = serde_json::to_vec(&first).unwrap();
        let mut parallel_state = AuiControlFeedbackState::default();
        let parallel = AuiControlFeedbackModule::advance(
            &mut parallel_state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[],
                presentation_delta_us: 1,
                diagnostics: AuiFeedbackDiagnosticsLevel::Trace,
            },
        );
        assert_eq!(first_bytes, serde_json::to_vec(&parallel).unwrap());

        let mut replaced = snapshot.clone();
        replaced.frame_id = 2;
        replaced.session_id = Some("b".to_string());
        replaced.hovered_node = None;
        let reconciled = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &replaced,
                commands: &[],
                presentation_delta_us: 1,
                diagnostics: AuiFeedbackDiagnosticsLevel::Trace,
            },
        );
        assert_eq!(reconciled.report.session_reconciliation_count, 1);
        assert_eq!(reconciled.report.time_reversal_count, 1);
        assert_eq!(state.active_node_count(), 0);
    }

    #[test]
    fn aui_control_feedback_project_profile_and_disabled_priority() {
        let mut document = button_document();
        let mut profile = AuiInteractionFeedbackProfile::new("project.button");
        profile.disabled_opacity_permille = 400;
        document.interaction_feedback = Some(crate::aui::AuiInteractionFeedbackRegistry {
            default_button_profile: Some("project.button".to_string()),
            profiles: vec![profile],
            ..Default::default()
        });
        document.nodes[1].interactable = false;
        let snapshot = AuiControlInteractionSnapshot {
            frame_id: 1,
            hovered_node: Some("button".to_string()),
            pressed_node: Some("button".to_string()),
            pressed_inside: true,
            ..Default::default()
        };
        let mut state = AuiControlFeedbackState::default();
        let frame = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[command("button", AuiCommandKind::Click)],
                presentation_delta_us: 1,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        let visual = frame.overrides.get("button").unwrap();
        assert!((visual.opacity_multiplier - 0.4).abs() < 0.001);
        assert_eq!(frame.report.project_profile_resolve_count, 1);
        assert_eq!(frame.report.activation_count, 0);
    }

    #[test]
    fn aui_control_feedback_hover_interpolation_covers_zero_mid_and_end() {
        let document = button_document();
        let mut snapshot = AuiControlInteractionSnapshot {
            frame_id: 1,
            hovered_node: Some("button".to_string()),
            ..Default::default()
        };
        let mut state = AuiControlFeedbackState::default();
        let zero = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[],
                presentation_delta_us: 0,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        assert!(zero.overrides.is_empty());
        snapshot.frame_id = 2;
        let mid = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[],
                presentation_delta_us: 35_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        let mid_scale = mid.overrides.get("button").unwrap().scale;
        assert!(mid_scale > 1.0 && mid_scale < 1.01);
        snapshot.frame_id = 3;
        let end = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &snapshot,
                commands: &[],
                presentation_delta_us: 35_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        assert!((end.overrides.get("button").unwrap().scale - 1.01).abs() < 0.001);
    }

    #[test]
    fn aui_control_feedback_modal_and_screen_change_reconcile_active_state() {
        let document = button_document();
        let first = AuiControlInteractionSnapshot {
            frame_id: 1,
            session_id: Some("session".to_string()),
            hovered_node: Some("button".to_string()),
            active_screen_id: Some("main".to_string()),
            ..Default::default()
        };
        let mut state = AuiControlFeedbackState::default();
        AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &first,
                commands: &[],
                presentation_delta_us: 70_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        let changed = AuiControlInteractionSnapshot {
            frame_id: 2,
            session_id: Some("session".to_string()),
            active_modal_root: Some("modal".to_string()),
            active_screen_id: Some("pause".to_string()),
            ..Default::default()
        };
        let frame = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &changed,
                commands: &[],
                presentation_delta_us: 1,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        assert_eq!(frame.report.scope_reconciliation_count, 1);
        assert!(frame.report.reclaimed_state_count >= 1);
        assert_eq!(state.active_node_count(), 0);
    }

    #[test]
    fn aui_control_feedback_cancel_recovers_and_repeated_click_does_not_stack_state() {
        let document = button_document();
        let pressed = AuiControlInteractionSnapshot {
            frame_id: 1,
            pressed_node: Some("button".to_string()),
            pressed_inside: true,
            ..Default::default()
        };
        let mut state = AuiControlFeedbackState::default();
        AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &pressed,
                commands: &[],
                presentation_delta_us: 45_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        let released = AuiControlInteractionSnapshot {
            frame_id: 2,
            ..Default::default()
        };
        let cancel = command("button", AuiCommandKind::PointerCancel);
        let recovering = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &released,
                commands: &[cancel],
                presentation_delta_us: 40_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        let recovering_scale = recovering.overrides.get("button").unwrap().scale;
        assert!(recovering_scale > 0.97 && recovering_scale < 1.0);

        let clicks = (0..32)
            .map(|_| command("button", AuiCommandKind::Click))
            .collect::<Vec<_>>();
        let clicked = AuiControlInteractionSnapshot {
            frame_id: 3,
            ..Default::default()
        };
        let activated = AuiControlFeedbackModule::advance(
            &mut state,
            AuiControlFeedbackFrameInput {
                document: &document,
                interaction: &clicked,
                commands: &clicks,
                presentation_delta_us: 16_000,
                diagnostics: AuiFeedbackDiagnosticsLevel::Summary,
            },
        );
        assert_eq!(activated.report.activation_count, 1);
        assert_eq!(state.active_node_count(), 1);
    }
}
