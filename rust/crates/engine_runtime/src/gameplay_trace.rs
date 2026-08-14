use crate::components::ComponentTypeId;
use crate::gameplay_command::GameplayCommandId;
use crate::ids::EntityId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayTraceRecord {
    pub frame_index: u64,
    pub phase: String,
    pub rule_id: String,
    pub operation: String,
    pub entity_id: Option<EntityId>,
    pub component_type: Option<ComponentTypeId>,
    pub field_path: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub command_id: Option<GameplayCommandId>,
    pub source: Option<String>,
    pub result: String,
    pub error_code: Option<String>,
}

impl GameplayTraceRecord {
    pub fn write(
        frame_index: u64,
        phase: impl Into<String>,
        rule_id: impl Into<String>,
        entity_id: EntityId,
        component_type: ComponentTypeId,
        field_path: impl Into<String>,
        before: Option<String>,
        after: Option<String>,
    ) -> Self {
        Self {
            frame_index,
            phase: phase.into(),
            rule_id: rule_id.into(),
            operation: "write".to_string(),
            entity_id: Some(entity_id),
            component_type: Some(component_type),
            field_path: Some(field_path.into()),
            before,
            after,
            command_id: None,
            source: None,
            result: "ok".to_string(),
            error_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameplay_trace_record_uses_neutral_fields() {
        let record = GameplayTraceRecord::write(
            1,
            "Update",
            "project.rule",
            EntityId::from("entity-a"),
            ComponentTypeId::from("project.marker"),
            "count",
            Some("I64(1)".to_string()),
            Some("I64(2)".to_string()),
        );

        assert_eq!(record.operation, "write");
        assert_eq!(record.result, "ok");
    }
}
