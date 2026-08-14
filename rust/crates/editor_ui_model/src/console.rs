use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsoleModel {
    pub entries: Vec<ConsoleEntry>,
    pub unread_error_count: u32,
    pub unread_warning_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub entry_id: String,
    pub level: ConsoleLevel,
    pub source: ConsoleSource,
    pub message: String,
    pub frame: Option<u64>,
    pub timestamp_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsoleLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsoleSource {
    Editor,
    Runtime,
    Command,
    Package,
}
