use crate::{
    CommandResult, CommandStatus, CommandTransaction, EditorSession, StateChangeSummary, UndoPolicy,
};

impl EditorSession {
    pub(crate) fn clear_console(&mut self, transaction: &mut CommandTransaction) -> CommandResult {
        transaction.write_set.push("console.entries".to_string());
        transaction.undo_policy = UndoPolicy::SnapshotReady;
        let before = self.console_entries.len();
        self.console_entries.clear();
        transaction.state_changes.push(StateChangeSummary {
            kind: "console.cleared".to_string(),
            path: "console.entries".to_string(),
            before_summary: Some(before.to_string()),
            after_summary: Some("0".to_string()),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }
}
