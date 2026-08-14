use crate::gameplay_command::{GameplayCommand, GameplayCommandApplyRecord, GameplayCommandId};
use crate::gameplay_trace::GameplayTraceRecord;
use crate::input_action::InputTraceSummary;
use crate::logic_executor::LogicResult;
use crate::physics2d::Physics2DTraceRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTraceEvent {
    pub frame: u64,
    pub system_id: String,
    pub phase: String,
    pub message: String,
    pub entity_count: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTrace {
    pub events: Vec<RuntimeTraceEvent>,
    pub gameplay_records: Vec<GameplayTraceRecord>,
    pub physics2d_records: Vec<Physics2DTraceRecord>,
}

impl RuntimeTrace {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            gameplay_records: Vec::new(),
            physics2d_records: Vec::new(),
        }
    }

    pub fn record(
        &mut self,
        frame: u64,
        system_id: impl Into<String>,
        phase: impl Into<String>,
        message: impl Into<String>,
        entity_count: Option<usize>,
    ) {
        self.events.push(RuntimeTraceEvent {
            frame,
            system_id: system_id.into(),
            phase: phase.into(),
            message: message.into(),
            entity_count,
        });
    }

    pub fn record_logic_result(
        &mut self,
        frame: u64,
        phase: impl Into<String>,
        result: &LogicResult,
    ) {
        let phase = phase.into();
        for query in &result.queries {
            self.gameplay_records.push(GameplayTraceRecord {
                frame_index: frame,
                phase: phase.clone(),
                rule_id: result.rule_id.clone(),
                operation: "query".to_string(),
                entity_id: None,
                component_type: None,
                field_path: None,
                before: None,
                after: Some(format!("result_count={}", query.result_count)),
                command_id: None,
                source: None,
                result: "ok".to_string(),
                error_code: None,
            });
        }
        for read in &result.reads {
            self.gameplay_records.push(GameplayTraceRecord {
                frame_index: frame,
                phase: phase.clone(),
                rule_id: result.rule_id.clone(),
                operation: "read".to_string(),
                entity_id: Some(read.entity_id.clone()),
                component_type: Some(read.component_type.clone()),
                field_path: None,
                before: None,
                after: None,
                command_id: None,
                source: None,
                result: "ok".to_string(),
                error_code: None,
            });
        }
        if result.writes.is_empty() {
            let message = if result.errors.is_empty() {
                format!(
                    "{} {}",
                    result.executor_kind.as_str(),
                    result.status.as_str()
                )
            } else {
                format!(
                    "{} {} {}",
                    result.executor_kind.as_str(),
                    result.status.as_str(),
                    result.errors[0].code
                )
            };
            self.record(
                frame,
                format!("project.rule.{}", result.rule_id),
                phase,
                message,
                None,
            );
            return;
        }

        for write in &result.writes {
            self.gameplay_records.push(GameplayTraceRecord::write(
                frame,
                phase.clone(),
                result.rule_id.clone(),
                write.entity_id.clone(),
                write.component_type.clone(),
                write.field.clone(),
                write.before.clone(),
                write.after.clone(),
            ));
            self.record(
                frame,
                format!("project.rule.{}", result.rule_id),
                phase.clone(),
                format!(
                    "{} {} {}.{}",
                    result.executor_kind.as_str(),
                    result.status.as_str(),
                    write.component_type,
                    write.field
                ),
                None,
            );
        }
    }

    pub fn record_command_enqueues(
        &mut self,
        frame: u64,
        phase: impl Into<String>,
        rule_id: &str,
        commands: &[(GameplayCommandId, GameplayCommand)],
    ) {
        let phase = phase.into();
        for (command_id, command) in commands {
            self.gameplay_records.push(GameplayTraceRecord {
                frame_index: frame,
                phase: phase.clone(),
                rule_id: rule_id.to_string(),
                operation: "command_enqueue".to_string(),
                entity_id: Some(command_entity_id(command)),
                component_type: command_component_type(command),
                field_path: None,
                before: None,
                after: command_enqueue_summary(command),
                command_id: Some(*command_id),
                source: command_source(command),
                result: "ok".to_string(),
                error_code: None,
            });
        }
    }

    pub fn record_command_apply_records(
        &mut self,
        frame: u64,
        phase: impl Into<String>,
        records: &[GameplayCommandApplyRecord],
    ) {
        let phase = phase.into();
        for record in records {
            self.gameplay_records.push(GameplayTraceRecord {
                frame_index: frame,
                phase: phase.clone(),
                rule_id: "engine.command_buffer".to_string(),
                operation: "command_apply".to_string(),
                entity_id: Some(record.entity_id.clone()),
                component_type: None,
                field_path: None,
                before: None,
                after: command_apply_summary(record),
                command_id: Some(record.command_id),
                source: record.prefab_ref_id.clone(),
                result: record.result.to_string(),
                error_code: record.error_code.map(str::to_string),
            });
        }
    }

    pub fn record_input_summary(&mut self, summary: &InputTraceSummary) {
        let route = summary.route_kind.as_deref().unwrap_or("None");
        let reason = summary.route_reason.as_deref().unwrap_or("none");
        let viewport = summary.viewport_id.as_deref().unwrap_or("none");
        let actions = if summary.action_ids.is_empty() {
            "none".to_string()
        } else {
            summary.action_ids.join(",")
        };
        self.record(
            summary.frame_id,
            "engine.input",
            "InputSnapshotReady",
            format!(
                "viewport={} route={} reason={} action_count={} actions={}",
                viewport, route, reason, summary.action_count, actions
            ),
            None,
        );
    }

    pub fn record_physics2d(&mut self, record: Physics2DTraceRecord) {
        self.physics2d_records.push(record);
    }
}

fn command_entity_id(command: &GameplayCommand) -> crate::ids::EntityId {
    match command {
        GameplayCommand::SpawnEntity { entity_id, .. }
        | GameplayCommand::DespawnEntity { entity_id }
        | GameplayCommand::AddComponent { entity_id, .. }
        | GameplayCommand::RemoveComponent { entity_id, .. }
        | GameplayCommand::SetParent { entity_id, .. } => entity_id.clone(),
        GameplayCommand::InstantiatePrefab { prefab_ref, .. } => {
            crate::ids::EntityId::from(format!("prefab:{}", prefab_ref.id))
        }
        GameplayCommand::DespawnPrefabInstance { instance_id } => {
            crate::ids::EntityId::from(format!("prefab-instance:{}", instance_id))
        }
    }
}

fn command_component_type(command: &GameplayCommand) -> Option<crate::components::ComponentTypeId> {
    match command {
        GameplayCommand::AddComponent { component_type, .. }
        | GameplayCommand::RemoveComponent { component_type, .. } => Some(component_type.clone()),
        _ => None,
    }
}

fn command_source(command: &GameplayCommand) -> Option<String> {
    match command {
        GameplayCommand::InstantiatePrefab { prefab_ref, .. } => Some(prefab_ref.id.clone()),
        GameplayCommand::DespawnPrefabInstance { instance_id } => {
            Some(format!("prefab-instance:{}", instance_id))
        }
        _ => None,
    }
}

fn command_enqueue_summary(command: &GameplayCommand) -> Option<String> {
    match command {
        GameplayCommand::InstantiatePrefab {
            parent_entity,
            target_scene_instance,
            ..
        } => Some(format!(
            "parent={} target_scene={}",
            parent_entity
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".to_string()),
            target_scene_instance
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string())
        )),
        GameplayCommand::DespawnPrefabInstance { instance_id } => {
            Some(format!("instance_id={}", instance_id))
        }
        _ => None,
    }
}

fn command_apply_summary(record: &GameplayCommandApplyRecord) -> Option<String> {
    if record.instance_id.is_none()
        && record.prefab_ref_id.is_none()
        && record.root_entity_id.is_none()
        && record.created_entity_count == 0
    {
        return None;
    }
    Some(format!(
        "instance_id={} root_entity={} created_entity_count={}",
        record
            .instance_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".to_string()),
        record
            .root_entity_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".to_string()),
        record.created_entity_count
    ))
}
