mod control_style;
mod control_style_coverage;
mod control_texture;
mod draw_list;
mod hit_test;
mod layout;
mod localization;
mod metrics;
mod panel_manifest;
mod panels;
mod renderer;
mod theme;
mod widget_extract;
mod widget_layout;
mod widget_pick;
mod widget_reconcile;
mod widget_tree;
mod workspace_docking;

pub use draw_list::{
    DrawCommand, HitRegion, HitTarget, UiColor, UiDrawList, UiPoint, UiRect, UiRendererConfig,
    UiUvRect,
};
pub use hit_test::{hit_test, hit_test_any};
pub use layout::editor_workspace_rect;
pub use localization::localize_editor_draw_list;
pub use panel_manifest::{
    native_editor_panel_manifest, NativeEditorPanelManifestEntry, PanelCommandSource,
    PanelScrollOwner, PanelStateOwner, NATIVE_EDITOR_PANEL_MANIFEST_VERSION,
};
pub use renderer::{RetainedEditorUiRenderer, SelfUiRenderer};
pub use theme::{
    EditorBorderTheme, EditorOverlayTheme, EditorSelectionTheme, EditorStatusTheme,
    EditorSurfaceTheme, EditorTextTheme, EditorTheme,
};
pub use widget_extract::{extract_widget_tree, WidgetExtractOutput};
pub use widget_layout::{layout_widget_tree, TextMeasure, WidgetLayoutError};
pub use widget_pick::{pick_widget, PickBlockReason, WidgetPath, WidgetPickResult};
pub use widget_reconcile::{reconcile_widget_tree, ReconcileDiagnostic, ReconcileReport};
pub use widget_tree::{
    ActivationPolicy, EditorCommandBinding, EditorWidgetAction, EditorWidgetDeclaration,
    EditorWidgetLayoutStyle, EditorWidgetNode, EditorWidgetTree, WidgetDirection, WidgetId,
    WidgetLocalState, WidgetPaint, WidgetRole, WidgetTreeError, WidgetVisibility,
};
pub use workspace_docking::{
    validate_workspace_layout, validate_workspace_topology, DockNode, DockSplitAxis,
    EditorWorkspaceDockingModule, EditorWorkspaceLayout, LayoutNodeId, PanelDescriptor, PanelId,
    PanelRegistry, PanelSize, WorkspaceDisplay, WorkspaceDragPreview, WorkspaceDragWindowFacts,
    WorkspaceIntent, WorkspaceLayoutDiagnostic, WorkspaceResolvedDockTargetToken, WorkspaceRestore,
    WorkspaceSnapshot, WorkspaceSplitter, WorkspaceTopology, WorkspaceUpdate, WorkspaceWindowId,
    WorkspaceWindowPlacement, WorkspaceWindowPlan, WorkspaceWindowPlanEntry, WorkspaceWindowRoot,
    EDITOR_WORKSPACE_LAYOUT_SCHEMA_VERSION, EDITOR_WORKSPACE_TOPOLOGY_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests;
pub use control_style::{
    dark_neutral_control_style, dark_neutral_control_style_summary, ControlBrush, ControlClassSet,
    ControlContentOffset, ControlPseudoState, ControlPseudoStateSet, ControlSliceInsets,
    ControlStyleBorder, ControlStyleDiagnostic, ControlStyleQuery, ControlStyleResolution,
    ControlStyleSummary, ControlStyleTrace, EditorControlStyleModule, ResolvedControlStyle,
    EDITOR_STYLE_SHEET_SCHEMA_VERSION,
};
pub use control_style_coverage::{
    control_style_coverage_report, ControlStyleCoverageLevel, EditorControlStyleCoverageReport,
};
pub use control_texture::{
    dark_neutral_control_textures, paint_control_brush, BuiltInControlTexture,
    ControlBrushPaintOutput, ControlTextureDiagnostic,
    EDITOR_THEME_TEXTURE_MANIFEST_SCHEMA_VERSION,
};
