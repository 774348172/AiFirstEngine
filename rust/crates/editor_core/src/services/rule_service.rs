use crate::{
    decode_rule_operation, decode_rule_statement, decode_rule_trigger,
    services::project_service::normalize_project_relative_path, CommandResult, CommandStatus,
    CommandTransaction, EditorSession, RuleAuthoringEditCommand, RuleAuthoringService,
    StateChangeSummary,
};
use engine_runtime::rule_ir::{ProjectRulePhase, RuleOperation, RuleStatement, RuleTrigger};

impl EditorSession {
    pub(crate) fn create_rule_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        rule_id: String,
        display_name: String,
        phase: Option<String>,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_authoring.no_project",
                "Cannot create a Rule asset before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.write_set.push(format!("rule_asset.{path}"));
        let phase = match phase.as_deref().map(parse_project_rule_phase).transpose() {
            Ok(phase) => phase.unwrap_or(ProjectRulePhase::Update),
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_authoring.invalid_phase",
                    message,
                    Some("Use FixedUpdate, Update, PostPhysics, or EventHandler."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        match RuleAuthoringService::create_asset_with_phase_in_scope(
            session.write_scope(),
            &path,
            &rule_id,
            &display_name,
            phase,
        ) {
            Ok(asset) => {
                let before = self.selected_project_browser_path.clone();
                self.selected_project_browser_path = Some(path.clone());
                transaction.state_changes.push(StateChangeSummary {
                    kind: "rule_asset.created".to_string(),
                    path: "workspace.selected_asset".to_string(),
                    before_summary: before,
                    after_summary: Some(path.clone()),
                });
                self.push_info(
                    transaction,
                    "editor.rule_authoring.created",
                    format!("Created Rule asset {} at {path}", asset.rule_id),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_authoring.create_failed",
                    message,
                    Some("Use a writable Rules path and a non-empty rule id."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn open_rule_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_authoring.no_project",
                "Cannot open a Rule asset before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("rule_asset.{path}"));
        match RuleAuthoringService::load(&session.project_root, &path) {
            Ok(asset) => {
                let before = self.selected_project_browser_path.clone();
                self.selected_project_browser_path = Some(path.clone());
                transaction.state_changes.push(StateChangeSummary {
                    kind: "rule_asset.opened".to_string(),
                    path: "workspace.selected_asset".to_string(),
                    before_summary: before,
                    after_summary: Some(path.clone()),
                });
                self.push_info(
                    transaction,
                    "editor.rule_authoring.opened",
                    format!("Opened Rule asset {}", asset.rule_id),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_authoring.open_failed",
                    message,
                    Some("Select a valid .rule.json asset."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn select_rule_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        transaction
            .write_set
            .push("workspace.selected_asset".to_string());
        let before = self.selected_project_browser_path.clone();
        self.selected_project_browser_path = Some(path.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "rule_asset.selected".to_string(),
            path: "workspace.selected_asset".to_string(),
            before_summary: before,
            after_summary: Some(path.clone()),
        });
        self.push_info(
            transaction,
            "editor.rule_authoring.selected",
            format!("Selected Rule asset {path}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn edit_rule_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        command: RuleAuthoringEditCommand,
        expected_ir_hash: Option<String>,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_authoring.no_project",
                "Cannot edit a Rule asset before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("rule_asset.{path}"));
        transaction.write_set.push(format!("rule_asset.{path}"));
        let mut asset = match RuleAuthoringService::load(&session.project_root, &path) {
            Ok(asset) => asset,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_authoring.load_failed",
                    message,
                    Some("Create or select a valid .rule.json asset."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let before = asset.ir_hash();
        let changed_paths =
            match RuleAuthoringService::apply(&mut asset, command, expected_ir_hash.as_deref()) {
                Ok(paths) => paths,
                Err(message) => {
                    self.push_error(
                        transaction,
                        "editor.rule_authoring.edit_failed",
                        message,
                        Some("Reload the Rule asset and apply a valid structured command."),
                    );
                    return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
                }
            };
        if let Err(message) =
            RuleAuthoringService::save_in_scope(session.write_scope(), &path, &asset)
        {
            self.push_error(
                transaction,
                "editor.rule_authoring.save_failed",
                message,
                Some("Check that the Rules folder is writable."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        }
        self.selected_project_browser_path = Some(path.clone());
        transaction.state_changes.push(StateChangeSummary {
            kind: "rule_asset.edited".to_string(),
            path: format!("rule_asset.{path}"),
            before_summary: Some(before),
            after_summary: Some(asset.ir_hash()),
        });
        self.push_info(
            transaction,
            "editor.rule_authoring.edited",
            format!(
                "Edited Rule asset {path}; changed_paths={}",
                changed_paths.join(",")
            ),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn validate_rule_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_authoring.no_project",
                "Cannot validate a Rule asset before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("rule_asset.{path}"));
        let asset = match RuleAuthoringService::load(&session.project_root, &path) {
            Ok(asset) => asset,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_authoring.load_failed",
                    message,
                    Some("Create or select a valid .rule.json asset."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let report = RuleAuthoringService::validate(&asset);
        push_rule_authoring_report(transaction, self, &report);
        self.selected_project_browser_path = Some(path);
        let status = if report.diagnostics.is_empty() {
            CommandStatus::Committed
        } else {
            CommandStatus::Failed
        };
        self.finish_transaction(transaction.clone(), status)
    }

    pub(crate) fn build_rule_artifact(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_authoring.no_project",
                "Cannot build a Rule artifact before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push(format!("rule_asset.{path}"));
        transaction
            .write_set
            .push("rule_artifact.report".to_string());
        match RuleAuthoringService::build(&session.project_root, &path) {
            Ok(report) => {
                push_rule_authoring_report(transaction, self, &report);
                self.selected_project_browser_path = Some(path);
                let status = if report.diagnostics.is_empty() {
                    CommandStatus::Committed
                } else {
                    CommandStatus::Failed
                };
                self.finish_transaction(transaction.clone(), status)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_authoring.build_failed",
                    message,
                    Some("Fix rule validation diagnostics and rebuild."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn build_project_rule_manifest(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_authoring.no_project",
                "Cannot build the project Rule manifest before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        transaction.read_set.push("rule_assets.saved".to_string());
        transaction.write_set.push(path.clone());
        match RuleAuthoringService::build_project_manifest(&session.project_root, &path) {
            Ok(manifest) => {
                self.push_info(
                    transaction,
                    "editor.rule_authoring.project_manifest_built",
                    format!(
                        "Built project Rule manifest with {} rules.",
                        manifest.rules.len()
                    ),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Committed)
            }
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_authoring.project_manifest_failed",
                    message,
                    Some("Fix saved RuleAsset diagnostics and rebuild the project manifest."),
                );
                self.finish_transaction(transaction.clone(), CommandStatus::Failed)
            }
        }
    }

    pub(crate) fn open_rule_diagnostics(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        if path.is_empty() {
            self.push_info(
                transaction,
                "editor.rule_authoring.diagnostics",
                "Rule diagnostics panel opened; select a Rule asset for detailed diagnostics.",
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Committed);
        }
        self.validate_rule_asset(transaction, path)
    }

    pub(crate) fn save_rule_asset(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_authoring.no_project",
                "Cannot save a Rule asset before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let full_path = session
            .project_root
            .join(normalize_project_relative_path(&path));
        if full_path.exists() {
            self.open_rule_asset(transaction, path)
        } else {
            self.push_error(
                transaction,
                "editor.rule_authoring.save_missing",
                "Cannot save a missing Rule asset.",
                Some("Create the Rule asset first."),
            );
            self.finish_transaction(transaction.clone(), CommandStatus::Failed)
        }
    }

    pub(crate) fn select_rule_card(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        card_id: String,
    ) -> CommandResult {
        let before_card = self.selected_rule_card_id.clone();
        let before_path = self.selected_project_browser_path.clone();
        self.selected_project_browser_path = Some(path.clone());
        self.selected_rule_card_id = Some(card_id.clone());
        self.selected_rule_graph_node_id = node_id_for_card_id(&card_id);
        transaction.read_set.push(format!("rule_asset.{path}"));
        transaction
            .write_set
            .push("editor.rule_authoring.selection".to_string());
        transaction.state_changes.push(StateChangeSummary {
            kind: "rule_card.selected".to_string(),
            path: "editor.rule_authoring.selected_card".to_string(),
            before_summary: before_card.or(before_path),
            after_summary: Some(card_id.clone()),
        });
        self.push_info(
            transaction,
            "editor.rule_card.selected",
            format!("Selected Rule Card {card_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn set_rule_card_field(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        card_id: String,
        field_path: String,
        value: serde_json::Value,
        expected_ir_hash: Option<String>,
    ) -> CommandResult {
        let Some(session) = &self.active_project_session else {
            self.push_error(
                transaction,
                "editor.rule_card.no_project",
                "Cannot edit a Rule Card before opening a project.",
                Some("Open or create a project first."),
            );
            return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
        };
        let asset = match RuleAuthoringService::load(&session.project_root, &path) {
            Ok(asset) => asset,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_card.load_failed",
                    message,
                    Some("Select a valid .rule.json asset."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        let command = match decode_set_rule_card_field_command(&asset, &card_id, &field_path, value)
        {
            Ok(command) => command,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_card.decode_failed",
                    message,
                    Some("Send a valid card field edit for an existing Rule Card."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        self.selected_rule_card_id = Some(card_id);
        self.selected_rule_graph_node_id = None;
        self.edit_rule_asset(transaction, path, command, expected_ir_hash)
    }

    pub(crate) fn add_rule_card(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        card_kind: String,
        value: serde_json::Value,
        expected_ir_hash: Option<String>,
    ) -> CommandResult {
        let command = match decode_add_rule_card_command(&card_kind, value) {
            Ok(command) => command,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_card.decode_failed",
                    message,
                    Some("AddRuleCard only supports statement or operation cards in v1."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        self.selected_rule_card_id = None;
        self.selected_rule_graph_node_id = None;
        self.edit_rule_asset(transaction, path, command, expected_ir_hash)
    }

    pub(crate) fn remove_rule_card(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        card_id: String,
        expected_ir_hash: Option<String>,
    ) -> CommandResult {
        let command = match decode_remove_rule_card_command(&card_id) {
            Ok(command) => command,
            Err(message) => {
                self.push_error(
                    transaction,
                    "editor.rule_card.decode_failed",
                    message,
                    Some("Only statement and operation cards can be removed in v1."),
                );
                return self.finish_transaction(transaction.clone(), CommandStatus::Failed);
            }
        };
        self.selected_rule_card_id = None;
        self.selected_rule_graph_node_id = None;
        self.edit_rule_asset(transaction, path, command, expected_ir_hash)
    }

    pub(crate) fn select_rule_graph_node(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
        node_id: String,
    ) -> CommandResult {
        let before = self.selected_rule_graph_node_id.clone();
        self.selected_project_browser_path = Some(path.clone());
        self.selected_rule_graph_node_id = Some(node_id.clone());
        self.selected_rule_card_id = card_id_for_node_id(&node_id);
        transaction.read_set.push(format!("rule_asset.{path}"));
        transaction
            .write_set
            .push("editor.rule_authoring.graph_selection".to_string());
        transaction.state_changes.push(StateChangeSummary {
            kind: "rule_graph_node.selected".to_string(),
            path: "editor.rule_authoring.selected_graph_node".to_string(),
            before_summary: before,
            after_summary: Some(node_id.clone()),
        });
        self.push_info(
            transaction,
            "editor.rule_graph.selected",
            format!("Selected Rule Graph node {node_id}"),
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }

    pub(crate) fn refresh_rule_graph_preview(
        &mut self,
        transaction: &mut CommandTransaction,
        path: String,
    ) -> CommandResult {
        transaction.read_set.push(format!("rule_asset.{path}"));
        self.selected_project_browser_path = Some(path);
        self.push_info(
            transaction,
            "editor.rule_graph.refreshed",
            "Rule Graph preview will be regenerated from the Rule asset.",
        );
        self.finish_transaction(transaction.clone(), CommandStatus::Committed)
    }
}

fn parse_project_rule_phase(value: &str) -> Result<ProjectRulePhase, String> {
    match value {
        "FixedUpdate" => Ok(ProjectRulePhase::FixedUpdate),
        "Update" => Ok(ProjectRulePhase::Update),
        "PostPhysics" => Ok(ProjectRulePhase::PostPhysics),
        "EventHandler" => Ok(ProjectRulePhase::EventHandler),
        _ => Err(format!("Unsupported Rule phase: {value}")),
    }
}

pub(crate) fn decode_trigger_command(
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    Ok(RuleAuthoringEditCommand::SetTrigger(decode_rule_trigger(
        value,
    )?))
}

pub(crate) fn decode_add_statement_command(
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    Ok(RuleAuthoringEditCommand::AddStatement(
        decode_rule_statement(value)?,
    ))
}

pub(crate) fn decode_update_statement_command(
    index: usize,
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    Ok(RuleAuthoringEditCommand::UpdateStatement {
        index,
        statement: decode_rule_statement(value)?,
    })
}

pub(crate) fn decode_add_operation_command(
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    Ok(RuleAuthoringEditCommand::AddOperation(
        decode_rule_operation(value)?,
    ))
}

pub(crate) fn decode_update_operation_command(
    index: usize,
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    Ok(RuleAuthoringEditCommand::UpdateOperation {
        index,
        operation: decode_rule_operation(value)?,
    })
}

pub(crate) fn decode_set_rule_card_field_command(
    asset: &engine_runtime::project_rule_asset::ProjectRuleAsset,
    card_id: &str,
    field_path: &str,
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    if card_id == "card:trigger" {
        return decode_trigger_card_field(&asset.canonical_ir.trigger, field_path, value);
    }
    if let Some(index) = card_index(card_id, "card:statement:") {
        let statement = asset
            .canonical_ir
            .statements
            .get(index)
            .ok_or_else(|| format!("Rule statement card index out of range: {index}"))?;
        let updated = decode_statement_card_field(statement, field_path, value)?;
        return Ok(RuleAuthoringEditCommand::UpdateStatement {
            index,
            statement: updated,
        });
    }
    if let Some(index) = card_index(card_id, "card:operation:") {
        let operation = asset
            .canonical_ir
            .operations
            .get(index)
            .ok_or_else(|| format!("Rule operation card index out of range: {index}"))?;
        let updated = decode_operation_card_field(operation, field_path, value)?;
        return Ok(RuleAuthoringEditCommand::UpdateOperation {
            index,
            operation: updated,
        });
    }
    Err(format!(
        "Unsupported Rule Card id for field edit: {card_id}"
    ))
}

pub(crate) fn decode_add_rule_card_command(
    card_kind: &str,
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    match card_kind {
        "statement" | "Statement" => Ok(RuleAuthoringEditCommand::AddStatement(
            decode_rule_statement(value)?,
        )),
        "operation" | "Operation" => Ok(RuleAuthoringEditCommand::AddOperation(
            decode_rule_operation(value)?,
        )),
        other => Err(format!(
            "AddRuleCard only supports statement or operation cards in v1, got {other}"
        )),
    }
}

pub(crate) fn decode_remove_rule_card_command(
    card_id: &str,
) -> Result<RuleAuthoringEditCommand, String> {
    if let Some(index) = card_index(card_id, "card:statement:") {
        return Ok(RuleAuthoringEditCommand::RemoveStatement { index });
    }
    if let Some(index) = card_index(card_id, "card:operation:") {
        return Ok(RuleAuthoringEditCommand::RemoveOperation { index });
    }
    Err(format!(
        "RemoveRuleCard is only supported for statement/operation cards in v1: {card_id}"
    ))
}

fn decode_trigger_card_field(
    current: &RuleTrigger,
    field_path: &str,
    value: serde_json::Value,
) -> Result<RuleAuthoringEditCommand, String> {
    let trigger = if field_path == "canonicalIr.trigger" || field_path == "trigger.json" {
        decode_rule_trigger(value)?
    } else if field_path.ends_with(".kind") {
        let kind = value_as_string(value)?;
        match kind.as_str() {
            "always" => RuleTrigger::Always,
            "actionPressed" => RuleTrigger::ActionPressed {
                action_id: match current {
                    RuleTrigger::ActionPressed { action_id } => action_id.clone(),
                    _ => String::new(),
                },
            },
            "eventReceived" => RuleTrigger::EventReceived {
                event_type: match current {
                    RuleTrigger::EventReceived { event_type } => event_type.clone(),
                    _ => String::new(),
                },
            },
            _ => return Err(format!("Unsupported trigger kind: {kind}")),
        }
    } else if field_path.ends_with(".actionId") {
        RuleTrigger::ActionPressed {
            action_id: value_as_string(value)?,
        }
    } else if field_path.ends_with(".eventType") {
        RuleTrigger::EventReceived {
            event_type: value_as_string(value)?,
        }
    } else {
        return Err(format!("Unsupported trigger card field path: {field_path}"));
    };
    Ok(RuleAuthoringEditCommand::SetTrigger(trigger))
}

fn decode_statement_card_field(
    current: &RuleStatement,
    field_path: &str,
    value: serde_json::Value,
) -> Result<RuleStatement, String> {
    if field_path.contains("statements[") || field_path == "statement.json" {
        return decode_rule_statement(value);
    }
    let _ = current;
    Err(format!(
        "Unsupported statement card field path: {field_path}"
    ))
}

fn decode_operation_card_field(
    current: &RuleOperation,
    field_path: &str,
    value: serde_json::Value,
) -> Result<RuleOperation, String> {
    if field_path.contains("operations[") && !field_path.contains("].") {
        return decode_rule_operation(value);
    }
    let mut operation = current.clone();
    match &mut operation {
        RuleOperation::WriteComponentField {
            entity_id,
            component_type,
            field_path: target_field_path,
            ..
        } => {
            if field_path.ends_with(".entityId") {
                *entity_id = value_as_string(value)?;
            } else if field_path.ends_with(".componentType") {
                *component_type = value_as_string(value)?;
            } else if field_path.ends_with(".fieldPath") {
                *target_field_path = value_as_string(value)?;
            } else {
                return Err(format!(
                    "Unsupported writeComponentField card field path: {field_path}"
                ));
            }
        }
        RuleOperation::SpawnEntity {
            entity_id,
            name,
            kind,
            ..
        } => {
            if field_path.ends_with(".entityId") {
                *entity_id = value_as_string(value)?;
            } else if field_path.ends_with(".name") {
                *name = value_as_string(value)?;
            } else if field_path.ends_with(".kind") {
                *kind = value_as_string(value)?;
            } else {
                return Err(format!(
                    "Unsupported spawnEntity card field path: {field_path}"
                ));
            }
        }
        RuleOperation::InstantiatePrefab { prefab_ref, .. } => {
            if field_path.ends_with(".prefabRef.id") {
                prefab_ref.id = value_as_string(value)?;
            } else {
                return Err(format!(
                    "Unsupported instantiatePrefab card field path: {field_path}"
                ));
            }
        }
        RuleOperation::DespawnEntity { entity_id } => {
            if field_path.ends_with(".entityId") {
                *entity_id = value_as_string(value)?;
            } else {
                return Err(format!(
                    "Unsupported despawnEntity card field path: {field_path}"
                ));
            }
        }
        RuleOperation::DespawnPrefabInstance { instance_id } => {
            if field_path.ends_with(".instanceId") {
                *instance_id = value
                    .as_u64()
                    .ok_or_else(|| "instanceId must be an unsigned integer.".to_string())?;
            } else {
                return Err(format!(
                    "Unsupported despawnPrefabInstance card field path: {field_path}"
                ));
            }
        }
        RuleOperation::EmitEvent { event_type, .. } => {
            if field_path.ends_with(".eventType") {
                *event_type = value_as_string(value)?;
            } else {
                return Err(format!(
                    "Unsupported emitEvent card field path: {field_path}"
                ));
            }
        }
    }
    Ok(operation)
}

fn value_as_string(value: serde_json::Value) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "Card field value must be a string.".to_string())
}

fn card_index(card_id: &str, prefix: &str) -> Option<usize> {
    card_id.strip_prefix(prefix)?.parse::<usize>().ok()
}

fn node_id_for_card_id(card_id: &str) -> Option<String> {
    if card_id == "card:trigger" {
        return Some("node:trigger".to_string());
    }
    if let Some(index) = card_index(card_id, "card:statement:") {
        return Some(format!("node:statement:{index}"));
    }
    if let Some(index) = card_index(card_id, "card:operation:") {
        return Some(format!("node:operation:{index}"));
    }
    if let Some(index) = card_index(card_id, "card:diagnostic:") {
        return Some(format!("node:diagnostic:{index}"));
    }
    None
}

fn card_id_for_node_id(node_id: &str) -> Option<String> {
    if node_id == "node:trigger" {
        return Some("card:trigger".to_string());
    }
    if let Some(index) = card_index(node_id, "node:statement:") {
        return Some(format!("card:statement:{index}"));
    }
    if let Some(index) = card_index(node_id, "node:operation:") {
        return Some(format!("card:operation:{index}"));
    }
    if let Some(index) = card_index(node_id, "node:diagnostic:") {
        return Some(format!("card:diagnostic:{index}"));
    }
    None
}

fn push_rule_authoring_report(
    transaction: &mut CommandTransaction,
    session: &EditorSession,
    report: &editor_ui_model::RuleAuthoringReport,
) {
    if report.diagnostics.is_empty() {
        session.push_info(
            transaction,
            "editor.rule_authoring.report",
            report.human_summary.clone(),
        );
        return;
    }
    for diagnostic in &report.diagnostics {
        session.push_error(
            transaction,
            &diagnostic.code,
            format!("{} {}", diagnostic.message, diagnostic.human_explanation),
            diagnostic.suggested_fix.as_deref(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::project_rule_asset::{ProjectRuleAsset, ProjectRuleAssetSourceKind};
    use engine_runtime::rule_ir::{ProjectRuleIr, ProjectRulePhase};

    #[test]
    fn rule_card_field_edit_lowers_to_existing_rule_authoring_edit_command() {
        let mut asset = ProjectRuleAsset::new(
            "asset.rule.fire",
            "Fire",
            ProjectRuleAssetSourceKind::UserAuthored,
            ProjectRuleIr::new("project.rule.fire", ProjectRulePhase::Update),
        );
        asset
            .canonical_ir
            .operations
            .push(RuleOperation::EmitEvent {
                event_type: "project.fire".to_string(),
                payload: None,
            });

        let command = decode_set_rule_card_field_command(
            &asset,
            "card:operation:0",
            "canonicalIr.operations[0].eventType",
            serde_json::json!("project.fire.updated"),
        )
        .unwrap();

        match command {
            RuleAuthoringEditCommand::UpdateOperation {
                index,
                operation: RuleOperation::EmitEvent { event_type, .. },
            } => {
                assert_eq!(index, 0);
                assert_eq!(event_type, "project.fire.updated");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn rule_card_add_remove_are_limited_to_statement_and_operation_cards() {
        let add = decode_add_rule_card_command(
            "operation",
            serde_json::json!({
                "op": "emitEvent",
                "event_type": "project.event"
            }),
        )
        .unwrap();
        assert!(matches!(add, RuleAuthoringEditCommand::AddOperation(_)));

        let remove = decode_remove_rule_card_command("card:operation:3").unwrap();
        assert!(matches!(
            remove,
            RuleAuthoringEditCommand::RemoveOperation { index: 3 }
        ));

        assert!(decode_add_rule_card_command("trigger", serde_json::json!({})).is_err());
        assert!(decode_remove_rule_card_command("card:trigger").is_err());
    }
}
