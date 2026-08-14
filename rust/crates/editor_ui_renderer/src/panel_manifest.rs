use serde::{Deserialize, Serialize};

use crate::WidgetRole;

pub const NATIVE_EDITOR_PANEL_MANIFEST_VERSION: &str = "native-editor-panel-manifest.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelStateOwner {
    EditorUiModel,
    DockLayout,
    WidgetLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelScrollOwner {
    None,
    WidgetLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelCommandSource {
    ProjectLauncher,
    Toolbar,
    Hierarchy,
    Inspector,
    Viewport,
    ProjectBrowser,
    AuthoringWorkflow,
    BuildPanel,
    AiAssistant,
    RuntimeTrace,
    InputMapping,
    EditorShell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeEditorPanelManifestEntry {
    pub panel_id: &'static str,
    pub root_widget_id: &'static str,
    pub dockable: bool,
    pub state_owner: PanelStateOwner,
    pub scroll_owner: PanelScrollOwner,
    pub command_source: PanelCommandSource,
    pub required_roles: &'static [WidgetRole],
}

const TOOLBAR: &[WidgetRole] = &[WidgetRole::Panel, WidgetRole::Button];
const SCROLL: &[WidgetRole] = &[WidgetRole::Panel, WidgetRole::Scroll];
const TEXT_INPUT: &[WidgetRole] = &[WidgetRole::Panel, WidgetRole::TextInput];
const VIEWPORT: &[WidgetRole] = &[WidgetRole::Panel, WidgetRole::Viewport];

static NATIVE_EDITOR_PANEL_MANIFEST: &[NativeEditorPanelManifestEntry] = &[
    entry(
        "project_launcher",
        "editor/project-launcher",
        false,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::ProjectLauncher,
        SCROLL,
    ),
    entry(
        "menu",
        "editor/shell/menu",
        false,
        PanelScrollOwner::None,
        PanelCommandSource::EditorShell,
        TOOLBAR,
    ),
    entry(
        "toolbar",
        "editor/shell/toolbar",
        false,
        PanelScrollOwner::None,
        PanelCommandSource::Toolbar,
        TOOLBAR,
    ),
    entry(
        "hierarchy",
        "editor/panel/hierarchy",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::Hierarchy,
        SCROLL,
    ),
    entry(
        "viewport",
        "editor/panel/viewport",
        true,
        PanelScrollOwner::None,
        PanelCommandSource::Viewport,
        VIEWPORT,
    ),
    entry(
        "inspector",
        "editor/panel/inspector",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::Inspector,
        SCROLL,
    ),
    entry(
        "bottom_tabs",
        "editor/dock/bottom-tabs",
        false,
        PanelScrollOwner::None,
        PanelCommandSource::EditorShell,
        TOOLBAR,
    ),
    entry(
        "asset_browser",
        "editor/panel/asset-browser",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::ProjectBrowser,
        SCROLL,
    ),
    entry(
        "authoring_workflow",
        "editor/panel/authoring-workflow",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::AuthoringWorkflow,
        SCROLL,
    ),
    entry(
        "input_mapping",
        "editor/panel/input-mapping",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::InputMapping,
        SCROLL,
    ),
    entry(
        "build_export",
        "editor/panel/build-export",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::BuildPanel,
        SCROLL,
    ),
    entry(
        "ai_panel",
        "editor/panel/ai",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::AiAssistant,
        TEXT_INPUT,
    ),
    entry(
        "console",
        "editor/panel/console",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::EditorShell,
        SCROLL,
    ),
    entry(
        "runtime_trace",
        "editor/panel/runtime-trace",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::RuntimeTrace,
        SCROLL,
    ),
    entry(
        "report",
        "editor/panel/report",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::EditorShell,
        SCROLL,
    ),
    entry(
        "project_intent",
        "editor/panel/project-intent",
        true,
        PanelScrollOwner::WidgetLocal,
        PanelCommandSource::EditorShell,
        SCROLL,
    ),
];

pub fn native_editor_panel_manifest() -> &'static [NativeEditorPanelManifestEntry] {
    NATIVE_EDITOR_PANEL_MANIFEST
}

const fn entry(
    panel_id: &'static str,
    root_widget_id: &'static str,
    dockable: bool,
    scroll_owner: PanelScrollOwner,
    command_source: PanelCommandSource,
    required_roles: &'static [WidgetRole],
) -> NativeEditorPanelManifestEntry {
    NativeEditorPanelManifestEntry {
        panel_id,
        root_widget_id,
        dockable,
        state_owner: PanelStateOwner::EditorUiModel,
        scroll_owner,
        command_source,
        required_roles,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::WidgetId;

    #[test]
    fn panel_manifest_is_versioned_complete_and_semantic() {
        assert_eq!(
            NATIVE_EDITOR_PANEL_MANIFEST_VERSION,
            "native-editor-panel-manifest.v1"
        );
        let manifest = native_editor_panel_manifest();
        assert_eq!(manifest.len(), 16);
        let ids: BTreeSet<_> = manifest.iter().map(|entry| entry.panel_id).collect();
        assert_eq!(ids.len(), manifest.len());
        for entry in manifest {
            WidgetId::semantic(entry.root_widget_id).expect("semantic root WidgetId");
            assert!(entry.required_roles.contains(&WidgetRole::Panel));
            assert_eq!(
                entry.dockable,
                entry.root_widget_id.starts_with("editor/panel/"),
                "{} has inconsistent dock ownership",
                entry.panel_id
            );
        }
    }
}
