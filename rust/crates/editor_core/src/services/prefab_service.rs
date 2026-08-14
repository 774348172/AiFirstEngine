use editor_ui_model::{
    DiagnosticSeverity, DiagnosticSource, EditorDiagnostic, PrefabStageMode, PrefabStageSavePolicy,
    Vec3,
};

use crate::{
    CommandResult, CommandStatus, CommandTransaction, EditorSceneEntity, EditorSession,
    PrefabAuthoringReport, PrefabAuthoringStatus, PrefabDiagnostic, PrefabDiagnosticSeverity,
    PrefabInstance, PrefabRef, PrefabWorkflowService, PreviewWorldSync, PropertyEditCommand,
    PropertyEditTarget, PropertyPath, PropertyTransactionRouter, PropertyValue, StateChangeSummary,
    UndoPolicy,
};

impl EditorSession {
    pub(crate) fn create_prefab_from_selection(
        &mut self,
        transaction: &mut CommandTransaction,
        _scene_path: Option<String>,
        root_entity_id: String,
        prefab_id: String,
        name: String,
        replace_selection_with_instance: bool,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_project",
                "Cannot create a Prefab before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(document) = &mut self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_scene",
                "Cannot create a Prefab before opening a Scene document.",
                Some("Open a Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if root_entity_id.trim().is_empty() || prefab_id.trim().is_empty() {
            self.push_error(
                transaction,
                "editor.prefab_authoring.context_required",
                "CreatePrefabFromSelection requires root_entity_id and prefab_id.",
                Some("Select a Scene entity and provide a Prefab id."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let Some(root) = document.entity(&root_entity_id).cloned() else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.source_missing",
                format!("Selected prefab root entity does not exist: {root_entity_id}"),
                Some("Select an existing Scene entity."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let display_name = if name.trim().is_empty() {
            root.name.clone()
        } else {
            name
        };
        let mut asset = PrefabWorkflowService::create_prefab_asset_from_entity_tree(
            prefab_id.clone(),
            display_name.clone(),
            &root,
            &document.entities,
        );
        let relative_path = PrefabWorkflowService::prefab_path_for_id(&prefab_id);
        asset.source_path = Some(relative_path.clone());
        if let Err(message) = PrefabWorkflowService::save_asset_in_scope(
            session.write_scope(),
            &relative_path,
            &asset,
        ) {
            self.push_error(
                transaction,
                "editor.prefab_authoring.save_failed",
                message,
                Some("Check that the Prefabs folder is writable."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        transaction
            .write_set
            .push(format!("prefab_asset.{relative_path}"));
        self.prefab_authoring
            .open_prefab_paths
            .push(relative_path.clone());
        self.prefab_authoring.validation_report.created_prefab_paths = vec![relative_path.clone()];
        if replace_selection_with_instance {
            if let Some(entity) = document.entity_mut(&root_entity_id) {
                write_instance_component(entity, PrefabRef::new(prefab_id.clone()));
            }
        }
        self.selected_project_browser_path = Some(relative_path.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "prefab_asset.created".to_string(),
            path: format!("prefab_asset.{relative_path}"),
            before_summary: None,
            after_summary: Some(prefab_id),
        });
        self.push_info(
            transaction,
            "editor.prefab_authoring.created",
            format!("Created Prefab {display_name} at {relative_path}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn open_prefab_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        self.enter_prefab_stage(transaction, path, PrefabStageMode::Isolated, None)
    }

    pub(crate) fn enter_prefab_stage(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        mode: PrefabStageMode,
        opened_from_instance_entity_id: Option<String>,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_project",
                "Cannot open a Prefab before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if path.trim().is_empty() {
            self.push_error(
                transaction,
                "editor.prefab_authoring.context_required",
                "OpenPrefabDocument requires a Prefab path.",
                Some("Select a Prefab asset."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        transaction.read_set.push(format!("prefab_asset.{path}"));
        let asset = match PrefabWorkflowService::load_asset(&session.project_root, &path) {
            Ok(asset) => asset,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.prefab_authoring.load_failed",
                    message,
                    Some("Select a valid .prefab.json asset."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let opened_instance = opened_from_instance_entity_id
            .as_deref()
            .and_then(|entity_id| self.editor_scene_document.as_ref()?.entity(entity_id))
            .and_then(|entity| PrefabInstance::from_scene_entity(entity).ok());
        let stage =
            PrefabWorkflowService::enter_stage(path.clone(), mode, asset, opened_instance.as_ref());
        let stage_id = stage.stage_id.clone();
        self.prefab_authoring.active_stage = Some(stage);
        if !self.prefab_authoring.open_prefab_paths.contains(&path) {
            self.prefab_authoring.open_prefab_paths.push(path.clone());
        }
        self.selected_project_browser_path = Some(path.clone());
        transaction
            .write_set
            .push("prefab_stage.active".to_string());
        self.push_info(
            transaction,
            "editor.prefab_authoring.stage_opened",
            format!("Opened Prefab Stage {stage_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_prefab_stage_entity_field(
        &mut self,
        transaction: &mut CommandTransaction,
        source_entity_id: String,
        component_type: Option<String>,
        field_path: String,
        value: serde_json::Value,
    ) -> CommandResult {
        transaction.read_set.push("prefab_stage.active".to_string());
        transaction
            .write_set
            .push("prefab_stage.active".to_string());
        transaction.undo_policy = UndoPolicy::SnapshotReady;
        let Some(stage) = &mut self.prefab_authoring.active_stage else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_active_stage",
                "Cannot edit a Prefab field without an active Prefab Stage.",
                Some("Open a Prefab document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        match PrefabWorkflowService::edit_stage_entity_field(
            stage,
            &source_entity_id,
            component_type.as_deref(),
            &field_path,
            value,
        ) {
            Ok(()) => {
                transaction.state_changes.push(StateChangeSummary {
                    kind: "prefab_stage.edited".to_string(),
                    path: format!("prefab_stage.{}.{}", source_entity_id, field_path),
                    before_summary: None,
                    after_summary: Some("dirty".to_string()),
                });
                self.push_info(
                    transaction,
                    "editor.prefab_authoring.stage_edited",
                    "Prefab Stage field edit committed.",
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => {
                transaction
                    .diagnostics
                    .push(prefab_diagnostic_to_editor(transaction, error));
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn set_prefab_instance_override_field(
        &mut self,
        transaction: &mut CommandTransaction,
        entity_id: String,
        component_type: String,
        field_path: String,
        value: serde_json::Value,
    ) -> Option<CommandResult> {
        let Some(document) = &mut self.editor_scene_document else {
            return None;
        };
        let Some(entity) = document.entity_mut(&entity_id) else {
            return None;
        };
        if PrefabInstance::from_scene_entity(entity).is_err() {
            return None;
        }
        let command = PropertyEditCommand::SetValue {
            target: PropertyEditTarget {
                entity_id: Some(entity_id.clone()),
                persistence: editor_ui_model::InspectorPersistence::PersistentAuthoring,
                path: PropertyPath::parse(format!("components.{component_type}.{field_path}"))
                    .unwrap_or_else(|_| PropertyPath::parse("components").unwrap()),
                component_type: Some(component_type),
                field_path: Some(field_path),
            },
            value: PropertyValue::Json(value),
        };
        match PropertyTransactionRouter::apply_prefab_override(entity, command) {
            Ok(_) => {
                document.mark_dirty(transaction.transaction_id.clone());
                refresh_preview_after_prefab_scene_change(self, transaction);
                transaction
                    .write_set
                    .push(format!("scene.entity.{entity_id}"));
                self.push_info(
                    transaction,
                    "editor.prefab_authoring.override_written",
                    "Prefab instance edit was recorded as an override.",
                );
                Some(self.finish_transaction(transaction.clone(), CommandStatus::Committed))
            }
            Err(error) => {
                transaction.diagnostics.push(EditorDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: format!("editor.prefab_authoring.inspector.{:?}", error.code),
                    message: error.message,
                    source: DiagnosticSource::EditorCore,
                    command_id: Some(transaction.command_id.clone()),
                    request_id: Some(transaction.request_id.clone()),
                    path: error.path,
                    entity_id: Some(entity_id),
                    trace_entry_id: None,
                    suggested_action: Some(
                        "Inspect the Prefab instance override field.".to_string(),
                    ),
                });
                Some(self.finish_transaction(transaction.clone(), CommandStatus::Failed))
            }
        }
    }

    pub(crate) fn save_prefab_document(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_project",
                "Cannot save a Prefab before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(stage) = &mut self.prefab_authoring.active_stage else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_active_stage",
                "Cannot save without an active Prefab Stage.",
                Some("Open a Prefab document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let save_path = if path.trim().is_empty() {
            stage.source_prefab_path.clone()
        } else {
            path
        };
        let asset = PrefabWorkflowService::save_stage(stage);
        if let Err(message) =
            PrefabWorkflowService::save_asset_in_scope(session.write_scope(), &save_path, &asset)
        {
            self.push_error(
                transaction,
                "editor.prefab_authoring.save_failed",
                message,
                Some("Check that the Prefabs folder is writable."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        PrefabWorkflowService::mark_stage_saved(stage);
        transaction
            .write_set
            .push(format!("prefab_asset.{save_path}"));
        self.prefab_authoring.validation_report.status = PrefabAuthoringStatus::Saved;
        self.push_info(
            transaction,
            "editor.prefab_authoring.saved",
            format!("Saved Prefab document {save_path}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn exit_prefab_stage(
        &mut self,
        transaction: &mut CommandTransaction,
        save_policy: PrefabStageSavePolicy,
    ) -> CommandResult {
        let Some(stage) = self.prefab_authoring.active_stage.as_ref() else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_active_stage",
                "Cannot exit Prefab Stage because no stage is active.",
                Some("Open a Prefab document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if save_policy == PrefabStageSavePolicy::Save {
            let path = stage.source_prefab_path.clone();
            return self.save_prefab_document(transaction, path);
        }
        if save_policy == PrefabStageSavePolicy::KeepOpen {
            self.push_info(
                transaction,
                "editor.prefab_authoring.stage_kept_open",
                "Prefab Stage remains open.",
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        self.prefab_authoring.active_stage = None;
        transaction
            .write_set
            .push("prefab_stage.active".to_string());
        self.push_info(
            transaction,
            "editor.prefab_authoring.stage_closed",
            "Prefab Stage closed without saving.",
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn instantiate_prefab_in_scene(
        &mut self,
        transaction: &mut CommandTransaction,
        prefab_id: String,
        parent_entity_id: Option<String>,
        local_position: Option<Vec3>,
    ) -> CommandResult {
        let Some(document) = &mut self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_scene",
                "Cannot instantiate a Prefab before opening a Scene document.",
                Some("Open a Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        if prefab_id.trim().is_empty() {
            self.push_error(
                transaction,
                "editor.prefab_authoring.context_required",
                "InstantiatePrefabInScene requires prefab_id.",
                Some("Select a Prefab asset."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        let name = prefab_id
            .rsplit(['/', '.'])
            .find(|part| !part.is_empty())
            .unwrap_or("Prefab");
        let entity_id = document.next_entity_id(name);
        let entity = PrefabWorkflowService::create_scene_instance_entity(
            PrefabRef::new(prefab_id.clone()),
            entity_id.clone(),
            name.to_string(),
            parent_entity_id,
            local_position,
        );
        document.entities.push(entity);
        document.mark_dirty(transaction.transaction_id.clone());
        self.scene_selection.select_single(document, &entity_id);
        self.selected_entity_id = Some(entity_id.clone());
        self.prefab_authoring
            .validation_report
            .instantiated_entity_ids
            .push(entity_id.clone());
        refresh_preview_after_prefab_scene_change(self, transaction);
        transaction
            .write_set
            .push(format!("scene.entity.{entity_id}"));
        self.push_info(
            transaction,
            "editor.prefab_authoring.instantiated",
            format!("Instantiated Prefab {prefab_id} as {entity_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn apply_prefab_override_to_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        instance_entity_id: String,
        target_source_entity_id: String,
        component_type: String,
        field_path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_project",
                "Cannot apply a Prefab override before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(document) = &mut self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_scene",
                "Cannot apply a Prefab override before opening a Scene document.",
                Some("Open a Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(entity) = document.entity_mut(&instance_entity_id) else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.instance_missing",
                format!("Prefab instance entity is missing: {instance_entity_id}"),
                Some("Select an existing Prefab instance."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let instance = match PrefabInstance::from_scene_entity(entity) {
            Ok(instance) => instance,
            Err(error) => {
                transaction
                    .diagnostics
                    .push(prefab_diagnostic_to_editor(transaction, error));
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let path = PrefabWorkflowService::prefab_path_for_id(&instance.prefab_ref.id);
        let mut asset = match PrefabWorkflowService::load_asset(&session.project_root, &path) {
            Ok(asset) => asset,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.prefab_authoring.load_failed",
                    message,
                    Some("Create or select a valid source Prefab asset."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        match PrefabWorkflowService::apply_override_to_asset(
            &mut asset,
            entity,
            &target_source_entity_id,
            &component_type,
            &field_path,
        ) {
            Ok(_) => {
                if let Err(message) =
                    PrefabWorkflowService::save_asset_in_scope(session.write_scope(), &path, &asset)
                {
                    self.push_error(
                        transaction,
                        "editor.prefab_authoring.save_failed",
                        message,
                        Some("Check that the Prefabs folder is writable."),
                    );
                    return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
                }
                document.mark_dirty(transaction.transaction_id.clone());
                refresh_preview_after_prefab_scene_change(self, transaction);
                transaction.write_set.push(format!("prefab_asset.{path}"));
                transaction
                    .write_set
                    .push(format!("scene.entity.{instance_entity_id}"));
                self.prefab_authoring
                    .validation_report
                    .applied_override_count += 1;
                self.push_info(
                    transaction,
                    "editor.prefab_authoring.override_applied",
                    format!("Applied Prefab override {component_type}.{field_path}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(error) => {
                transaction
                    .diagnostics
                    .push(prefab_diagnostic_to_editor(transaction, error));
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn revert_prefab_override(
        &mut self,
        transaction: &mut CommandTransaction,
        instance_entity_id: String,
        target_source_entity_id: String,
        component_type: String,
        field_path: String,
    ) -> CommandResult {
        let Some(document) = &mut self.editor_scene_document else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.no_scene",
                "Cannot revert a Prefab override before opening a Scene document.",
                Some("Open a Scene document first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let Some(entity) = document.entity_mut(&instance_entity_id) else {
            self.push_error(
                transaction,
                "editor.prefab_authoring.instance_missing",
                format!("Prefab instance entity is missing: {instance_entity_id}"),
                Some("Select an existing Prefab instance."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        match PrefabWorkflowService::revert_override(
            entity,
            &target_source_entity_id,
            &component_type,
            &field_path,
        ) {
            Ok(Some(_)) => {
                document.mark_dirty(transaction.transaction_id.clone());
                refresh_preview_after_prefab_scene_change(self, transaction);
                self.prefab_authoring
                    .validation_report
                    .reverted_override_count += 1;
                self.push_info(
                    transaction,
                    "editor.prefab_authoring.override_reverted",
                    format!("Reverted Prefab override {component_type}.{field_path}"),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Ok(None) => {
                self.push_error(
                    transaction,
                    "editor.prefab_authoring.override_missing",
                    "Prefab override does not exist on this instance.",
                    Some("Select an existing override."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Rejected)
            }
            Err(error) => {
                transaction
                    .diagnostics
                    .push(prefab_diagnostic_to_editor(transaction, error));
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn validate_prefab_references(
        &mut self,
        transaction: &mut CommandTransaction,
        _path: Option<String>,
    ) -> CommandResult {
        let project_root = self
            .active_project_session
            .as_ref()
            .map(|session| session.project_root.display().to_string());
        let instances = self.collect_scene_prefab_instances();
        let assets = collect_open_and_scene_prefab_assets(self);
        let previous_report = self.prefab_authoring.validation_report.clone();
        let mut report = PrefabAuthoringReport::from_parts(
            project_root,
            &assets,
            &instances,
            self.prefab_authoring.active_stage.as_ref(),
        );
        report.created_prefab_paths = previous_report.created_prefab_paths;
        report.instantiated_entity_ids = previous_report.instantiated_entity_ids;
        report.applied_override_count = previous_report.applied_override_count;
        report.reverted_override_count = previous_report.reverted_override_count;
        let status = report.status;
        self.prefab_authoring.validation_report = report;
        self.push_info(
            transaction,
            "editor.prefab_authoring.validated",
            format!("Prefab authoring validation status: {status:?}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub fn prefab_authoring_report(&self) -> &PrefabAuthoringReport {
        &self.prefab_authoring.validation_report
    }

    fn collect_scene_prefab_instances(&self) -> Vec<PrefabInstance> {
        self.editor_scene_document
            .as_ref()
            .into_iter()
            .flat_map(|document| document.entities.iter())
            .filter_map(|entity| PrefabInstance::from_scene_entity(entity).ok())
            .collect()
    }
}

fn write_instance_component(entity: &mut EditorSceneEntity, prefab_ref: PrefabRef) {
    let instance = PrefabInstance::new(
        format!("prefab-instance-{}", entity.entity_id),
        prefab_ref,
        entity.entity_id.clone(),
    );
    let component = instance.to_scene_component();
    if let Some(existing) = entity
        .components
        .iter_mut()
        .find(|component| component.component_type == crate::PREFAB_INSTANCE_COMPONENT_TYPE)
    {
        *existing = component;
    } else {
        entity.components.push(component);
    }
}

fn collect_open_and_scene_prefab_assets(session: &EditorSession) -> Vec<crate::PrefabAsset> {
    let Some(project) = &session.active_project_session else {
        return Vec::new();
    };
    let mut paths = session.prefab_authoring.open_prefab_paths.clone();
    if let Some(stage) = &session.prefab_authoring.active_stage {
        paths.push(stage.source_prefab_path.clone());
    }
    for instance in session.collect_scene_prefab_instances() {
        paths.push(PrefabWorkflowService::prefab_path_for_id(
            &instance.prefab_ref.id,
        ));
    }
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| PrefabWorkflowService::load_asset(&project.project_root, &path).ok())
        .collect()
}

fn refresh_preview_after_prefab_scene_change(
    session: &mut EditorSession,
    transaction: &mut CommandTransaction,
) {
    if let Some(document) = &session.editor_scene_document {
        match PreviewWorldSync::full_rebuild(document) {
            Ok((preview_world, sync_report)) => {
                session.world = Some(preview_world);
                session.last_preview_world_sync_report = Some(sync_report);
            }
            Err(sync_report) => {
                session.last_preview_world_sync_report = Some(sync_report);
                transaction.diagnostics.push(EditorDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: "editor.prefab_authoring.preview_sync_failed".to_string(),
                    message: "Preview world sync failed after Prefab authoring change.".to_string(),
                    source: DiagnosticSource::EditorCore,
                    command_id: Some(transaction.command_id.clone()),
                    request_id: Some(transaction.request_id.clone()),
                    path: None,
                    entity_id: None,
                    trace_entry_id: None,
                    suggested_action: Some("Check Scene diagnostics.".to_string()),
                });
            }
        }
    }
}

fn prefab_diagnostic_to_editor(
    transaction: &CommandTransaction,
    diagnostic: PrefabDiagnostic,
) -> EditorDiagnostic {
    EditorDiagnostic {
        severity: match diagnostic.severity {
            PrefabDiagnosticSeverity::Info => DiagnosticSeverity::Info,
            PrefabDiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            PrefabDiagnosticSeverity::Error => DiagnosticSeverity::Error,
        },
        code: format!("editor.prefab_authoring.{}", diagnostic.code.as_str()),
        message: diagnostic.message,
        source: DiagnosticSource::EditorCore,
        command_id: Some(transaction.command_id.clone()),
        request_id: Some(transaction.request_id.clone()),
        path: diagnostic.field_path,
        entity_id: diagnostic.source_entity_id,
        trace_entry_id: None,
        suggested_action: Some("Inspect the PrefabAuthoringReport diagnostics.".to_string()),
    }
}
