use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ANIMATOR2D_AUTHORING_MODEL_SCHEMA_VERSION: &str = "animator2d-authoring-model.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DPlaybackModel {
    Loop,
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DParameterKindModel {
    Bool,
    Trigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DTransitionTimingModel {
    Immediate,
    ClipEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DFrameModel {
    pub sprite_ref: String,
    pub duration_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DClipModel {
    pub path: String,
    pub asset_id: String,
    pub playback: Animator2DPlaybackModel,
    pub frames: Vec<Animator2DFrameModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DParameterModel {
    pub id: String,
    pub kind: Animator2DParameterKindModel,
    pub default_bool: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DStateModel {
    pub id: String,
    pub clip_ref: String,
    pub speed_permille: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DConditionModel {
    pub parameter: String,
    pub equals: Option<bool>,
    pub triggered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DTransitionModel {
    pub id: String,
    pub from: String,
    pub to: String,
    pub timing: Animator2DTransitionTimingModel,
    pub priority: i32,
    pub conditions: Vec<Animator2DConditionModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DControllerModel {
    pub path: String,
    pub asset_id: String,
    pub entry_state_id: String,
    pub parameters: Vec<Animator2DParameterModel>,
    pub states: Vec<Animator2DStateModel>,
    pub transitions: Vec<Animator2DTransitionModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DComponentModel {
    pub scene_path: String,
    pub entity_id: String,
    pub controller_ref: String,
    pub enabled: bool,
    pub initial_bools: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DPreviewRunState {
    Closed,
    Playing,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DPreviewModel {
    pub run_state: Animator2DPreviewRunState,
    pub controller_id: Option<String>,
    pub fixed_tick_index: u64,
    pub current_state_id: Option<String>,
    pub current_clip_id: Option<String>,
    pub current_frame_index: Option<u32>,
    pub current_sprite_ref: Option<String>,
    pub completed: bool,
    pub bools: BTreeMap<String, bool>,
    pub triggers: Vec<String>,
    pub diagnostics: Vec<Animator2DAuthoringDiagnostic>,
}

impl Default for Animator2DPreviewModel {
    fn default() -> Self {
        Self {
            run_state: Animator2DPreviewRunState::Closed,
            controller_id: None,
            fixed_tick_index: 0,
            current_state_id: None,
            current_clip_id: None,
            current_frame_index: None,
            current_sprite_ref: None,
            completed: false,
            bools: BTreeMap::new(),
            triggers: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DPlayObservationModel {
    pub entity_id: String,
    pub read_only: bool,
    pub state_id: String,
    pub clip_id: String,
    pub frame_index: u32,
    pub completed: bool,
    pub bools: BTreeMap<String, bool>,
    pub triggers: Vec<String>,
    pub recent_diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DRelationshipEdge {
    pub transition_id: String,
    pub from_state_id: String,
    pub to_state_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DAuthoringDiagnostic {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DControlModel {
    pub command_id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DAuthoringModel {
    pub schema_version: String,
    pub dirty: bool,
    pub clip: Option<Animator2DClipModel>,
    pub controller: Option<Animator2DControllerModel>,
    pub component: Option<Animator2DComponentModel>,
    pub preview: Animator2DPreviewModel,
    pub play_observations: Vec<Animator2DPlayObservationModel>,
    pub relationship_edges: Vec<Animator2DRelationshipEdge>,
    pub relationship_graph_editable: bool,
    pub sprite_picker_enabled: bool,
    pub controller_picker_enabled: bool,
    pub controls: Vec<Animator2DControlModel>,
    pub diagnostics: Vec<Animator2DAuthoringDiagnostic>,
}

impl Default for Animator2DAuthoringModel {
    fn default() -> Self {
        Self {
            schema_version: ANIMATOR2D_AUTHORING_MODEL_SCHEMA_VERSION.to_string(),
            dirty: false,
            clip: None,
            controller: None,
            component: None,
            preview: Animator2DPreviewModel::default(),
            play_observations: Vec::new(),
            relationship_edges: Vec::new(),
            relationship_graph_editable: false,
            sprite_picker_enabled: true,
            controller_picker_enabled: true,
            controls: default_controls(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Animator2DAuthoringCommand {
    CreateClip {
        path: String,
        asset_id: String,
    },
    OpenClip {
        path: String,
    },
    SetClipPlayback {
        playback: Animator2DPlaybackModel,
    },
    AddClipFrame {
        sprite_ref: String,
        duration_ticks: u32,
    },
    UpdateClipFrame {
        index: usize,
        sprite_ref: String,
        duration_ticks: u32,
    },
    MoveClipFrame {
        from_index: usize,
        to_index: usize,
    },
    RemoveClipFrame {
        index: usize,
    },
    CreateController {
        path: String,
        asset_id: String,
    },
    OpenController {
        path: String,
    },
    UpsertParameter {
        parameter: Animator2DParameterModel,
    },
    RemoveParameter {
        parameter_id: String,
    },
    UpsertState {
        state: Animator2DStateModel,
    },
    RemoveState {
        state_id: String,
    },
    SetEntryState {
        state_id: String,
    },
    UpsertTransition {
        transition: Animator2DTransitionModel,
    },
    RemoveTransition {
        transition_id: String,
    },
    SaveActive,
    ReloadActive,
    SetComponent {
        component: Animator2DComponentModel,
    },
    StartPreview {
        controller_id: String,
    },
    PreviewPlay,
    PreviewPause,
    PreviewRestart,
    PreviewStepTick,
    PreviewSetBool {
        parameter_id: String,
        value: bool,
    },
    PreviewSetTrigger {
        parameter_id: String,
    },
    PreviewResetTrigger {
        parameter_id: String,
    },
    ClosePreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animator2DAuthoringStatus {
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animator2DAuthoringResult {
    pub status: Animator2DAuthoringStatus,
    pub model: Animator2DAuthoringModel,
}

fn default_controls() -> Vec<Animator2DControlModel> {
    [
        ("animator2d.save", "Save"),
        ("animator2d.preview.play", "Play"),
        ("animator2d.preview.pause", "Pause"),
        ("animator2d.preview.restart", "Restart"),
        ("animator2d.preview.step", "Step Tick"),
        ("animator2d.preview.close", "Close Preview"),
    ]
    .into_iter()
    .map(|(command_id, label)| Animator2DControlModel {
        command_id: command_id.to_string(),
        label: label.to_string(),
        enabled: true,
    })
    .collect()
}
