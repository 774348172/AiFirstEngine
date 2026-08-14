use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source: DiagnosticSource,
    pub command_id: Option<String>,
    pub request_id: Option<String>,
    pub path: Option<String>,
    pub entity_id: Option<String>,
    pub trace_entry_id: Option<String>,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSource {
    EditorCore,
    UiBackend,
    Runtime,
    RuntimePackage,
    Command,
}
