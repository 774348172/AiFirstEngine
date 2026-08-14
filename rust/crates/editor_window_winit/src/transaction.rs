use editor_core::{CommandResult, CommandStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EditorTransactionService {
    pub committed_count: u64,
    pub rejected_count: u64,
    pub failed_count: u64,
    pub last_transaction_id: Option<String>,
    pub last_status: Option<CommandStatus>,
}

impl EditorTransactionService {
    pub fn record(&mut self, result: &CommandResult) {
        self.last_transaction_id = Some(result.transaction_id.clone());
        self.last_status = Some(result.status);
        match result.status {
            CommandStatus::Committed => self.committed_count += 1,
            CommandStatus::Rejected => self.rejected_count += 1,
            CommandStatus::Failed => self.failed_count += 1,
            CommandStatus::Pending | CommandStatus::Validated => {}
        }
    }
}
