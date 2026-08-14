use editor_ui_model::{ConsoleEntry, EditorDiagnostic, UiCommand, UiCommandPayload};
use serde::{Deserialize, Serialize};

use crate::command_id_for_payload;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandStatus {
    Pending,
    Validated,
    Committed,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UndoPolicy {
    None,
    SnapshotReady,
    FutureUndoable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateChangeSummary {
    pub kind: String,
    pub path: String,
    pub before_summary: Option<String>,
    pub after_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandTransaction {
    pub transaction_id: String,
    pub request_id: String,
    pub command_id: String,
    pub source: String,
    pub payload: UiCommandPayload,
    pub status: CommandStatus,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
    pub diagnostics: Vec<EditorDiagnostic>,
    pub state_changes: Vec<StateChangeSummary>,
    pub undo_policy: UndoPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub transaction_id: String,
    pub request_id: String,
    pub command_id: String,
    pub status: CommandStatus,
    pub diagnostics: Vec<EditorDiagnostic>,
    pub console_entries: Vec<ConsoleEntry>,
    pub state_changes: Vec<StateChangeSummary>,
    pub ui_model_revision: u64,
}

pub fn command_for_test(payload: UiCommandPayload) -> UiCommand {
    UiCommand {
        command_id: command_id_for_payload(&payload).to_string(),
        source: editor_ui_model::UiCommandSource::Test,
        request_id: "request-test".to_string(),
        payload,
    }
}
