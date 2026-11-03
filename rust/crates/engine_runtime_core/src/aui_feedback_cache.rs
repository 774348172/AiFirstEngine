//! Private feedback scheduling state. Inputs are immutable shared products, not caller revisions.
use crate::aui::*;
use crate::aui_control_feedback::*;
use crate::aui_theme::AuiThemeRegistry;
use crate::font_bundle::RuntimeFontBundleRegistry;
use crate::game_view_presentation::ResolvedGameViewPresentation;
use crate::runtime_package::RuntimeAuiFontAtlasRegistry;
use std::sync::Arc;

pub(crate) const MAX_FEEDBACK_VARIANTS: usize = 4;
pub(crate) const MAX_FEEDBACK_VARIANT_NODES: usize = 4_096;

pub(crate) fn feedback_variant_is_bounded(node_count: usize) -> bool {
    node_count <= MAX_FEEDBACK_VARIANT_NODES
}

#[derive(Debug, Clone)]
pub(crate) struct CachedFeedbackPresentation {
    output: AuiRuntimePresentOutput,
    interaction: AuiControlInteractionSnapshot,
    atlases: RuntimeAuiFontAtlasRegistry,
    bundles: RuntimeFontBundleRegistry,
    themes: AuiThemeRegistry,
    presentation: Option<ResolvedGameViewPresentation>,
    pub frame: AuiControlFeedbackFrame,
}

fn same_optional<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn same_interaction(a: &AuiControlInteractionSnapshot, b: &AuiControlInteractionSnapshot) -> bool {
    a.session_id == b.session_id
        && a.hovered_node == b.hovered_node
        && a.pressed_node == b.pressed_node
        && a.pressed_inside == b.pressed_inside
        && a.pointer_id == b.pointer_id
        && a.pointer_device_kind == b.pointer_device_kind
        && a.pointer_hover_capable == b.pointer_hover_capable
        && a.focused_node == b.focused_node
        && a.focus_visible == b.focus_visible
        && a.active_modal_root == b.active_modal_root
        && a.active_screen_id == b.active_screen_id
}

#[derive(Debug, Clone)]
pub(crate) struct CachedFeedbackStyles {
    source: Arc<AuiDocument>,
    themes: AuiThemeRegistry,
    interaction: AuiControlInteractionSnapshot,
    pub document: Arc<AuiDocument>,
}

impl PartialEq for CachedFeedbackStyles {
    fn eq(&self, other: &Self) -> bool {
        self.matches(&other.source, &other.themes, &other.interaction)
            && self.document == other.document
    }
}

impl CachedFeedbackStyles {
    pub fn matches(
        &self,
        source: &Arc<AuiDocument>,
        themes: &AuiThemeRegistry,
        interaction: &AuiControlInteractionSnapshot,
    ) -> bool {
        Arc::ptr_eq(&self.source, source)
            && self.themes.shares_storage(themes)
            && same_interaction(&self.interaction, interaction)
    }

    pub fn new(
        source: Arc<AuiDocument>,
        themes: AuiThemeRegistry,
        interaction: AuiControlInteractionSnapshot,
        document: Arc<AuiDocument>,
    ) -> Self {
        Self {
            source,
            themes,
            interaction,
            document,
        }
    }
}

impl PartialEq for CachedFeedbackPresentation {
    fn eq(&self, other: &Self) -> bool {
        self.matches(
            &other.output,
            &other.interaction,
            &other.atlases,
            &other.bundles,
            &other.themes,
            other.presentation.as_ref(),
        ) && self.frame == other.frame
    }
}

impl CachedFeedbackPresentation {
    pub fn new(
        output: &AuiRuntimePresentOutput,
        interaction: &AuiControlInteractionSnapshot,
        atlases: &RuntimeAuiFontAtlasRegistry,
        bundles: &RuntimeFontBundleRegistry,
        themes: &AuiThemeRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
        frame: AuiControlFeedbackFrame,
    ) -> Self {
        Self {
            output: output.clone(),
            interaction: interaction.clone(),
            atlases: atlases.clone(),
            bundles: bundles.clone(),
            themes: themes.clone(),
            presentation: presentation.cloned(),
            frame,
        }
    }

    pub fn matches(
        &self,
        output: &AuiRuntimePresentOutput,
        interaction: &AuiControlInteractionSnapshot,
        atlases: &RuntimeAuiFontAtlasRegistry,
        bundles: &RuntimeFontBundleRegistry,
        themes: &AuiThemeRegistry,
        presentation: Option<&ResolvedGameViewPresentation>,
    ) -> bool {
        Arc::ptr_eq(&self.output.resolved_document, &output.resolved_document)
            && Arc::ptr_eq(&self.output.layout, &output.layout)
            && Arc::ptr_eq(&self.output.paragraph_layouts, &output.paragraph_layouts)
            && Arc::ptr_eq(&self.output.overlay.draw_items, &output.overlay.draw_items)
            && same_optional(&self.output.overlay.glyph_plan, &output.overlay.glyph_plan)
            && self.output.composition.shares_drawing(&output.composition)
            && same_interaction(&self.interaction, interaction)
            && (Arc::ptr_eq(&self.atlases.atlases_by_id, &atlases.atlases_by_id)
                || (self.atlases.is_empty() && atlases.is_empty()))
            && self.atlases.default_ui_font_atlas_id == atlases.default_ui_font_atlas_id
            && (Arc::ptr_eq(&self.bundles.bundles_by_id, &bundles.bundles_by_id)
                || (self.bundles.bundles_by_id.is_empty() && bundles.bundles_by_id.is_empty()))
            && self.bundles.default_ui_font_bundle_id == bundles.default_ui_font_bundle_id
            && self.themes.shares_storage(themes)
            && match (&self.presentation, presentation) {
                (Some(a), Some(b)) => a.same_ui_mapping(b),
                (None, None) => true,
                _ => false,
            }
    }

    pub fn matches_feedback_state(
        &self,
        interaction: &AuiControlInteractionSnapshot,
        overrides: &AuiVisualOverrideSet,
    ) -> bool {
        same_interaction(&self.interaction, interaction) && &self.frame.overrides == overrides
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e7_feedback_variants_exclude_large_documents() {
        assert!(feedback_variant_is_bounded(4_096));
        assert!(!feedback_variant_is_bounded(4_097));
        assert!(!feedback_variant_is_bounded(10_000));
    }
}
