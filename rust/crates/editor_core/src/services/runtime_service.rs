use editor_ui_model::{DiagnosticSeverity, DiagnosticSource, EditorDiagnostic};
use engine_runtime::diagnostics::DiagnosticSeverity as RuntimeDiagnosticSeverity;
use engine_runtime::frame_loop::FrameLoop;
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::scene_loader::load_scene_into_world;
use std::path::Path;

use crate::ui_model_composer::trace_entry_id;
use crate::{
    CommandResult, CommandStatus, CommandTransaction, EditorSession, StateChangeSummary, UndoPolicy,
};

pub(crate) fn runtime_diagnostics_to_editor(
    command_id: &str,
    request_id: &str,
    diagnostics: &engine_runtime::diagnostics::RuntimeDiagnostics,
) -> Vec<EditorDiagnostic> {
    diagnostics
        .issues
        .iter()
        .map(|issue| EditorDiagnostic {
            severity: match issue.severity {
                RuntimeDiagnosticSeverity::Error => DiagnosticSeverity::Error,
                RuntimeDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            },
            code: "editor.runtime.diagnostic".to_string(),
            message: issue.message.clone(),
            source: DiagnosticSource::RuntimePackage,
            command_id: Some(command_id.to_string()),
            request_id: Some(request_id.to_string()),
            path: Some(issue.path.clone()),
            entity_id: None,
            trace_entry_id: None,
            suggested_action: Some("Check the Runtime Package input.".to_string()),
        })
        .collect()
}

impl EditorSession {
    pub(crate) fn open_runtime_package(
        &mut self,
        transaction: &mut CommandTransaction,
        path: &Path,
    ) -> CommandResult {
        transaction
            .read_set
            .push("runtime_package.manifest".to_string());
        transaction.write_set.push("runtime_package".to_string());
        transaction.write_set.push("runtime.world".to_string());
        transaction.undo_policy = UndoPolicy::FutureUndoable;

        let package_result = load_runtime_package(path);
        if package_result.value.is_none() {
            transaction
                .diagnostics
                .extend(runtime_diagnostics_to_editor(
                    &transaction.command_id,
                    &transaction.request_id,
                    &package_result.diagnostics,
                ));
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let package = package_result.value.expect("checked package result value");
        let world_result = load_scene_into_world(&package.active_scene);
        if world_result.value.is_none() {
            transaction
                .diagnostics
                .extend(runtime_diagnostics_to_editor(
                    &transaction.command_id,
                    &transaction.request_id,
                    &world_result.diagnostics,
                ));
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let previous = self
            .runtime_package_path
            .as_ref()
            .map(|value| value.display().to_string());
        let active_scene_id = package.manifest.active_scene_id.clone();
        self.runtime_package_path = Some(path.to_path_buf());
        self.frame_loop = Some(FrameLoop::new(active_scene_id.clone()));
        self.runtime_package = Some(package);
        self.world = world_result.value;
        self.last_frame_output = None;
        self.selected_entity_id = None;
        self.selected_entity_source = None;
        self.selected_trace_entry_id = None;
        transaction.state_changes.push(StateChangeSummary {
            kind: "runtime_package.opened".to_string(),
            path: "runtime_package.path".to_string(),
            before_summary: previous,
            after_summary: Some(path.display().to_string()),
        });
        self.push_info(
            transaction,
            "editor.runtime_package.opened",
            format!("Opened Runtime Package {}", path.display()),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn reload_runtime_package(
        &mut self,
        transaction: &mut CommandTransaction,
    ) -> CommandResult {
        transaction.undo_policy = UndoPolicy::FutureUndoable;
        let Some(path) = self.runtime_package_path.clone() else {
            self.push_error(
                transaction,
                "editor.runtime_package.not_loaded",
                "Cannot reload before opening a Runtime Package.",
                Some("Open a Runtime Package first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        self.open_runtime_package(transaction, &path)
    }

    pub(crate) fn tick_one_frame(&mut self, transaction: &mut CommandTransaction) -> CommandResult {
        transaction.read_set.push("runtime.world".to_string());
        transaction.write_set.push("runtime.frame".to_string());
        transaction
            .write_set
            .push("runtime.render_snapshot".to_string());
        transaction.write_set.push("runtime.trace".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let Some(world) = &self.world else {
            self.push_error(
                transaction,
                "editor.runtime_package.not_loaded",
                "Cannot tick runtime before opening a Runtime Package.",
                Some("Open a Runtime Package first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(frame_loop) = &mut self.frame_loop else {
            self.push_error(
                transaction,
                "editor.runtime.not_initialized",
                "Runtime frame loop is not initialized.",
                Some("Reload the Runtime Package."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let before = self
            .last_frame_output
            .as_ref()
            .map_or(0, |output| output.frame);
        let output = frame_loop.tick(world);
        let after = output.frame;
        self.last_frame_output = Some(output);
        transaction.state_changes.push(StateChangeSummary {
            kind: "runtime.frame_advanced".to_string(),
            path: "runtime.frame".to_string(),
            before_summary: Some(before.to_string()),
            after_summary: Some(after.to_string()),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn reset_runtime(&mut self, transaction: &mut CommandTransaction) -> CommandResult {
        transaction.undo_policy = UndoPolicy::FutureUndoable;
        let Some(package) = &self.runtime_package else {
            self.push_error(
                transaction,
                "editor.runtime_package.not_loaded",
                "Cannot reset runtime before opening a Runtime Package.",
                Some("Open a Runtime Package first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let world_result = load_scene_into_world(&package.active_scene);
        if world_result.value.is_none() {
            transaction
                .diagnostics
                .extend(runtime_diagnostics_to_editor(
                    &transaction.command_id,
                    &transaction.request_id,
                    &world_result.diagnostics,
                ));
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        self.world = world_result.value;
        self.frame_loop = Some(FrameLoop::new(package.manifest.active_scene_id.clone()));
        self.last_frame_output = None;
        transaction.state_changes.push(StateChangeSummary {
            kind: "runtime.reset".to_string(),
            path: "runtime".to_string(),
            before_summary: None,
            after_summary: Some("reset".to_string()),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn select_trace_entry(
        &mut self,
        transaction: &mut CommandTransaction,
        entry_id: &str,
    ) -> CommandResult {
        transaction.read_set.push("runtime.trace".to_string());
        transaction
            .write_set
            .push("runtime_trace.selected_entry_id".to_string());
        transaction.undo_policy = UndoPolicy::None;
        let Some(trace) = self.last_trace() else {
            self.push_error(
                transaction,
                "editor.runtime_trace.empty",
                "Cannot select a trace entry before runtime has produced trace.",
                Some("Tick one frame first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let exists = trace
            .events
            .iter()
            .enumerate()
            .any(|(index, _)| trace_entry_id(index) == entry_id);
        if !exists {
            let mut diagnostic = self.make_diagnostic(
                transaction,
                DiagnosticSeverity::Warning,
                "editor.runtime_trace.entry_not_found",
                format!("Trace entry {} does not exist.", entry_id),
                Some("Select an existing trace entry."),
            );
            diagnostic.trace_entry_id = Some(entry_id.to_string());
            transaction.diagnostics.push(diagnostic);
            return self.finish_transaction(transaction.clone(), CommandStatus::Rejected);
        }
        let before = self.selected_trace_entry_id.clone();
        self.selected_trace_entry_id = Some(entry_id.to_string());
        transaction.state_changes.push(StateChangeSummary {
            kind: "runtime_trace.selection.changed".to_string(),
            path: "runtime_trace.selected_entry_id".to_string(),
            before_summary: before,
            after_summary: Some(entry_id.to_string()),
        });
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }
}
