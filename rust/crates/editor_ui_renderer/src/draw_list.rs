use serde::{Deserialize, Serialize};

use engine_runtime::game_view_presentation::GameViewTargetSpec;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UiRect {
    pub fn contains(&self, point: UiPoint) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        (right > x && bottom > y).then_some(Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiUvRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl UiUvRect {
    pub const FULL: Self = Self {
        u0: 0.0,
        v0: 0.0,
        u1: 1.0,
        v1: 1.0,
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DrawCommand {
    Clipped {
        clip: UiRect,
        command: Box<DrawCommand>,
    },
    Rect {
        rect: UiRect,
        color: UiColor,
        corner_radius: f32,
    },
    Text {
        rect: UiRect,
        text: String,
        color: UiColor,
        size: f32,
    },
    ViewportTextureSlot {
        rect: UiRect,
        scene_id: Option<String>,
        frame: u64,
        texture_id: Option<String>,
        target_id: Option<String>,
    },
    ImageTextureSlot {
        rect: UiRect,
        source_uv: UiUvRect,
        texture_id: Option<String>,
        fallback_color: UiColor,
        tint: UiColor,
    },
}

impl DrawCommand {
    pub fn with_clip(self, clip: Option<UiRect>) -> Self {
        match clip {
            Some(clip) => Self::Clipped {
                clip,
                command: Box::new(self),
            },
            None => self,
        }
    }

    pub fn clip(&self) -> Option<UiRect> {
        match self {
            Self::Clipped { clip, .. } => Some(*clip),
            _ => None,
        }
    }

    pub fn unclipped(&self) -> &Self {
        match self {
            Self::Clipped { command, .. } => command.unclipped(),
            command => command,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitTarget {
    ProjectLauncherAction {
        action_id: String,
    },
    ProjectLauncherRecentProject {
        project_path: String,
    },
    ProjectBrowserEntry {
        path: String,
    },
    ProjectBrowserOpen {
        path: String,
    },
    AssetBrowserEntry {
        entry_key: editor_ui_model::AssetEntryKey,
        path: String,
    },
    AssetBrowserOpen {
        entry_key: editor_ui_model::AssetEntryKey,
        path: String,
    },
    AssetBrowserFolder {
        path: String,
    },
    AssetBrowserAction {
        action: editor_ui_model::AssetBrowserToolbarAction,
    },
    AssetBrowserSearch,
    AiPromptField,
    AssetPickerConfirm,
    AssetPickerCancel,
    AuthoringWorkflowStep {
        step_id: String,
    },
    AuthoringWorkflowCommand {
        command_id: String,
        payload_kind: String,
        domain: String,
    },
    ToolbarCommand {
        command_id: String,
    },
    HierarchyEntity {
        entity_id: String,
    },
    HierarchyAction {
        action_id: String,
    },
    AuiSceneNode {
        document_path: String,
        document_id: String,
        node_id: String,
    },
    InspectorField {
        field_id: String,
    },
    InspectorAssetPicker {
        field_id: String,
    },
    ConsoleEntry {
        entry_id: String,
    },
    RuntimeTraceEntry {
        entry_id: String,
    },
    AiPanelAction {
        action_id: String,
    },
    AiProposedCommand {
        proposal_id: String,
    },
    GatewayAccessDecision {
        request_id: String,
        approved: bool,
    },
    GatewayAccessPage {
        page_index: usize,
    },
    ProjectRuntimeTrustDecision {
        request_id: String,
        action: String,
    },
    ProjectIntentAction {
        action_id: String,
        subject_id: String,
    },
    InputMappingControl {
        action: String,
        path: String,
        target_id: Option<String>,
        value: Option<String>,
    },
    DockTab {
        panel_id: String,
    },
    WorkspaceSplitter {
        node_id: String,
    },
    WorkspaceWindowMenu,
    EditorLanguageMenu,
    SetEditorLocale {
        locale: editor_ui_model::EditorLocaleId,
    },
    WorkspacePanelVisibility {
        panel_id: String,
        visible: bool,
    },
    WorkspaceResetLayout,
    WorkspacePanelLock {
        stack_id: String,
        panel_id: String,
        locked: bool,
    },
    WorkspacePanelMore {
        stack_id: String,
        panel_id: String,
    },
    WorkspacePanelClose {
        stack_id: String,
        panel_id: String,
    },
    ToolbarOverflow,
    GameViewTarget {
        width: u32,
        height: u32,
        scale_policy: editor_ui_model::EditorGameViewScalePolicy,
    },
    Viewport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitRegion {
    pub id: String,
    pub rect: UiRect,
    pub target: HitTarget,
    pub enabled: bool,
    pub command_id: Option<String>,
    pub reason_disabled: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiDrawList {
    pub revision: u64,
    pub frame: u64,
    pub surface_width: f32,
    pub surface_height: f32,
    pub commands: Vec<DrawCommand>,
    pub hit_regions: Vec<HitRegion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiRendererConfig {
    pub width: f32,
    pub height: f32,
    pub hovered_hit_id: Option<String>,
    pub pressed_hit_id: Option<String>,
    pub focused_hit_id: Option<String>,
    pub focus_visible_hit_id: Option<String>,
    pub active_bottom_panel_id: Option<String>,
    pub toolbar_overflow_open: bool,
    pub workspace_menu_open: bool,
    pub language_menu_open: bool,
    pub workspace_snapshot: Option<crate::WorkspaceSnapshot>,
    pub workspace_panel_popup_stack_id: Option<String>,
    pub localization: editor_ui_model::EditorLocalizationSnapshot,
    pub game_view_target: Option<GameViewTargetSpec>,
}

impl UiRendererConfig {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            hovered_hit_id: None,
            pressed_hit_id: None,
            focused_hit_id: None,
            focus_visible_hit_id: None,
            active_bottom_panel_id: None,
            toolbar_overflow_open: false,
            workspace_menu_open: false,
            language_menu_open: false,
            workspace_snapshot: None,
            workspace_panel_popup_stack_id: None,
            localization: editor_ui_model::EditorLocalizationSnapshot::default(),
            game_view_target: None,
        }
    }

    pub fn with_game_view_target(mut self, target: Option<GameViewTargetSpec>) -> Self {
        self.game_view_target = target;
        self
    }

    pub fn with_interaction(
        mut self,
        hovered_hit_id: Option<String>,
        pressed_hit_id: Option<String>,
    ) -> Self {
        self.hovered_hit_id = hovered_hit_id;
        self.pressed_hit_id = pressed_hit_id;
        self
    }

    pub fn with_control_interaction(
        mut self,
        hovered_hit_id: Option<String>,
        active_hit_id: Option<String>,
        focused_hit_id: Option<String>,
        focus_visible_hit_id: Option<String>,
    ) -> Self {
        self.hovered_hit_id = hovered_hit_id;
        self.pressed_hit_id = active_hit_id;
        self.focused_hit_id = focused_hit_id;
        self.focus_visible_hit_id = focus_visible_hit_id;
        self
    }

    pub fn control_pseudo_states(
        &self,
        hit_id: &str,
        mut model: crate::ControlPseudoStateSet,
        enabled: bool,
    ) -> crate::ControlPseudoStateSet {
        use crate::ControlPseudoState;
        model = model
            .with(
                ControlPseudoState::Hover,
                self.hovered_hit_id.as_deref() == Some(hit_id),
            )
            .with(
                ControlPseudoState::Active,
                self.pressed_hit_id.as_deref() == Some(hit_id),
            )
            .with(ControlPseudoState::Disabled, !enabled)
            .with(
                ControlPseudoState::Focus,
                self.focused_hit_id.as_deref() == Some(hit_id),
            )
            .with(
                ControlPseudoState::FocusVisible,
                self.focus_visible_hit_id.as_deref() == Some(hit_id),
            );
        model
    }

    pub fn with_active_bottom_panel(mut self, panel_id: Option<String>) -> Self {
        self.active_bottom_panel_id = panel_id;
        self
    }

    pub fn with_toolbar_overflow_open(mut self, open: bool) -> Self {
        self.toolbar_overflow_open = open;
        self
    }

    pub fn with_workspace_menu_open(mut self, open: bool) -> Self {
        self.workspace_menu_open = open;
        self
    }

    pub fn with_language_menu_open(mut self, open: bool) -> Self {
        self.language_menu_open = open;
        self
    }

    pub fn with_workspace_snapshot(mut self, snapshot: crate::WorkspaceSnapshot) -> Self {
        self.workspace_snapshot = Some(snapshot);
        self
    }

    pub fn with_workspace_panel_chrome(mut self, popup_stack_id: Option<String>) -> Self {
        self.workspace_panel_popup_stack_id = popup_stack_id;
        self
    }

    pub fn with_localization(
        mut self,
        localization: editor_ui_model::EditorLocalizationSnapshot,
    ) -> Self {
        self.localization = localization;
        self
    }
}
