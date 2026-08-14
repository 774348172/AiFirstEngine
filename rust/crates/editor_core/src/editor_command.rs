use editor_ui_model::{UiCommandPayload, UiCommandSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EditorCommandId(pub String);

impl EditorCommandId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorCommandCategory {
    Project,
    Workspace,
    Scene,
    Asset,
    Prefab,
    Rule,
    Aui,
    Runtime,
    Build,
    Console,
    Report,
    Ai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorCommandOwnerDomain {
    ProjectLauncher,
    AuthoringWorkspace,
    SceneEditing,
    AssetBrowser,
    PrefabAuthoring,
    AuiAuthoring,
    InputMapping,
    RuleAuthoring,
    RuntimeSession,
    BuildExport,
    Console,
    ReportPanel,
    AiPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorCommandPayloadKind {
    Project,
    Workspace,
    Scene,
    Asset,
    Prefab,
    Aui,
    InputMapping,
    Rule,
    Runtime,
    Build,
    Console,
    Trace,
    Report,
    Ai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCommandDescriptor {
    pub command_id: &'static str,
    pub title: &'static str,
    pub category: EditorCommandCategory,
    pub owner_domain: EditorCommandOwnerDomain,
    pub payload_kind: EditorCommandPayloadKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EditorCommandPayload {
    Ui(UiCommandPayload),
}

impl EditorCommandPayload {
    pub fn as_ui_payload(&self) -> &UiCommandPayload {
        match self {
            Self::Ui(payload) => payload,
        }
    }

    pub fn into_ui_payload(self) -> UiCommandPayload {
        match self {
            Self::Ui(payload) => payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorCommandRequest {
    pub command_id: String,
    pub source: UiCommandSource,
    pub request_id: String,
    pub payload: EditorCommandPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCommandAvailability {
    pub enabled: bool,
    pub visible: bool,
    pub checked: bool,
    pub disabled_reason: Option<String>,
}

impl EditorCommandAvailability {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            visible: true,
            checked: false,
            disabled_reason: None,
        }
    }

    pub fn disabled(reason: impl Into<String>) -> Self {
        Self {
            enabled: false,
            visible: true,
            checked: false,
            disabled_reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCommandContext {
    pub has_active_project: bool,
    pub has_runtime_package: bool,
    pub has_scene_document: bool,
    pub has_selection: bool,
    pub is_playing: bool,
}
