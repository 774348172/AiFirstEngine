use editor_ui_model::AuthoringStepId;

use crate::{CommandResult, CommandStatus, CommandTransaction, EditorSession, StateChangeSummary};

impl EditorSession {
    pub(crate) fn set_authoring_workflow_step(
        &mut self,
        transaction: &mut CommandTransaction,
        step_id: AuthoringStepId,
    ) -> CommandResult {
        transaction
            .write_set
            .push("authoring_workflow.active_step".to_string());
        let before = self.active_authoring_step.as_str().to_string();
        self.active_authoring_step = step_id;
        transaction.state_changes.push(StateChangeSummary {
            kind: "authoring_workflow.active_step.changed".to_string(),
            path: "authoring_workflow.active_step".to_string(),
            before_summary: Some(before),
            after_summary: Some(step_id.as_str().to_string()),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }
}
