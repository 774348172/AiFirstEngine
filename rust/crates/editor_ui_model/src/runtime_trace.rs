use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTraceModel {
    pub frame: u64,
    pub entries: Vec<RuntimeTraceEntryView>,
    pub selected_entry_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeTraceEntryView {
    pub entry_id: String,
    pub frame: u64,
    pub phase: String,
    pub system_id: String,
    pub message: String,
    pub entity_id: Option<String>,
    pub level: TraceLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceLevel {
    Info,
    Warning,
    Error,
}
