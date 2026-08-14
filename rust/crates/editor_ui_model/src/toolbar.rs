use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolbarModel {
    pub commands: Vec<ToolbarCommand>,
    pub runtime_state: RuntimeRunState,
    pub game_view_layout: GameViewLayoutState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeRunState {
    NoPackage,
    Paused,
    Playing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolbarCommand {
    pub command_id: String,
    pub label: String,
    pub enabled: bool,
    pub reason_disabled: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameViewLayoutState {
    pub maximize_on_play: bool,
    pub is_game_view_maximized: bool,
    pub restore_workspace_region: Option<String>,
    pub reason: Option<String>,
    pub target: EditorGameViewTarget,
    pub target_editable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorGameViewScalePolicy {
    Contain,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorGameViewTarget {
    pub width: u32,
    pub height: u32,
    pub scale_policy: EditorGameViewScalePolicy,
}

impl EditorGameViewTarget {
    pub const fn new(width: u32, height: u32, scale_policy: EditorGameViewScalePolicy) -> Self {
        Self {
            width,
            height,
            scale_policy,
        }
    }
}

impl Default for GameViewLayoutState {
    fn default() -> Self {
        Self {
            maximize_on_play: false,
            is_game_view_maximized: false,
            restore_workspace_region: None,
            reason: None,
            target: EditorGameViewTarget::new(1280, 720, EditorGameViewScalePolicy::Stretch),
            target_editable: true,
        }
    }
}
