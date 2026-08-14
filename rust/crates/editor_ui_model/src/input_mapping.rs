use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputMappingAuthoringModel {
    pub project_root: Option<String>,
    pub selected_path: Option<String>,
    pub mapping_id: Option<String>,
    pub selected_context_id: Option<String>,
    pub selected_action_id: Option<String>,
    pub selected_binding_id: Option<String>,
    pub source_hash: Option<String>,
    pub dirty: bool,
    pub capture_binding_id: Option<String>,
    pub capture_accepts_pointer_position: bool,
    pub preview: Option<InputMappingPreviewResult>,
    pub report_level: InputMappingReportLevel,
    pub actions: Vec<InputMappingActionSummary>,
    pub contexts: Vec<InputMappingContextSummary>,
    pub bindings: Vec<InputMappingBindingSummary>,
    pub control_catalog: InputControlCatalogModel,
    pub report: InputMappingAuthoringReport,
    pub commands: Vec<InputMappingAuthoringCommand>,
    pub empty_message: String,
}

impl InputMappingAuthoringModel {
    pub fn empty() -> Self {
        Self {
            project_root: None,
            selected_path: None,
            mapping_id: None,
            selected_context_id: None,
            selected_action_id: None,
            selected_binding_id: None,
            source_hash: None,
            dirty: false,
            capture_binding_id: None,
            capture_accepts_pointer_position: false,
            preview: None,
            report_level: InputMappingReportLevel::Summary,
            actions: Vec::new(),
            contexts: Vec::new(),
            bindings: Vec::new(),
            control_catalog: InputControlCatalogModel::default(),
            report: InputMappingAuthoringReport::default(),
            commands: Vec::new(),
            empty_message: "Open a project and select an InputMapping asset.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingActionSummary {
    pub action_id: String,
    pub value_type: InputActionValueKind,
    pub binding_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum InputActionValueKind {
    Button,
    Axis1,
    Axis2,
    Pointer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputTriggerKind {
    Down,
    Pressed,
    Released,
    Hold { seconds: f32 },
    Tap { max_seconds: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputProcessorKind {
    None,
    Deadzone { threshold: f32 },
    Normalize,
    Scale { factor: f32 },
    Invert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingContextSummary {
    pub context_id: String,
    pub priority: i32,
    pub consume_input: bool,
    pub enabled_by_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingBindingSummary {
    pub binding_id: String,
    pub binding_index: usize,
    pub context_id: String,
    pub action_id: String,
    pub device_path: String,
    pub processor: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputControlDeviceKindModel {
    Keyboard,
    Mouse,
    Gamepad,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputControlCatalogEntryModel {
    pub device_path: String,
    pub label: String,
    pub device_kind: InputControlDeviceKindModel,
    pub compatible_value_types: Vec<InputActionValueKind>,
    pub selectable: bool,
    pub capture_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputControlCatalogModel {
    pub schema_version: String,
    pub controls: Vec<InputControlCatalogEntryModel>,
}

impl Default for InputControlCatalogModel {
    fn default() -> Self {
        Self {
            schema_version: "input-control-catalog.v1".to_string(),
            controls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingAuthoringCommand {
    pub command_id: String,
    pub label: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

impl InputMappingAuthoringCommand {
    pub fn new(
        command_id: impl Into<String>,
        label: impl Into<String>,
        enabled: bool,
        disabled_reason: Option<String>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            label: label.into(),
            enabled,
            disabled_reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingAuthoringReport {
    pub schema_version: String,
    pub mapping_count: usize,
    pub action_count: usize,
    pub context_count: usize,
    pub binding_count: usize,
    pub validation_status: InputMappingValidationStatus,
    pub diagnostics: Vec<InputMappingAuthoringDiagnostic>,
}

impl Default for InputMappingAuthoringReport {
    fn default() -> Self {
        Self {
            schema_version: "input-mapping-authoring-report.v1".to_string(),
            mapping_count: 0,
            action_count: 0,
            context_count: 0,
            binding_count: 0,
            validation_status: InputMappingValidationStatus::Missing,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputMappingValidationStatus {
    Missing,
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingAuthoringDiagnostic {
    pub severity: InputMappingDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub context_id: Option<String>,
    pub action_id: Option<String>,
    pub binding_id: Option<String>,
    pub field_path: Option<String>,
    pub suggested_fix: Option<String>,
}

impl InputMappingAuthoringDiagnostic {
    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            severity: InputMappingDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path,
            context_id: None,
            action_id: None,
            binding_id: None,
            field_path: None,
            suggested_fix: None,
        }
    }

    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            severity: InputMappingDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            path,
            context_id: None,
            action_id: None,
            binding_id: None,
            field_path: None,
            suggested_fix: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputMappingDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputMappingPreviewStatus {
    Resolved,
    NoAction,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputMappingReportLevel {
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingPreviewAction {
    pub action_id: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMappingPreviewResult {
    pub schema_version: String,
    pub status: InputMappingPreviewStatus,
    pub device_path: String,
    pub input_event_kind: String,
    pub matched_binding_ids: Vec<String>,
    pub shadowed_binding_ids: Vec<String>,
    pub actions: Vec<InputMappingPreviewAction>,
    pub diagnostics: Vec<InputMappingAuthoringDiagnostic>,
}
