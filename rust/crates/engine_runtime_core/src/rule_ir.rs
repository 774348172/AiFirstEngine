use crate::component_value::RuntimeValue;
use crate::components::ComponentTypeId;
use crate::field_path::FieldPath;
use crate::ids::EntityId;
use crate::logic_executor::RulePhase;
use crate::math::Vec3;
use crate::runtime_package::RuntimeAssetRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

pub const PROJECT_RULE_IR_SCHEMA_VERSION: &str = "project-rule-ir.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRuleIr {
    pub schema_version: String,
    pub rule_id: String,
    pub phase: ProjectRulePhase,
    pub enabled: bool,
    #[serde(default)]
    pub trigger: RuleTrigger,
    #[serde(default)]
    pub statements: Vec<RuleStatement>,
    #[serde(default)]
    pub operations: Vec<RuleOperation>,
    #[serde(default)]
    pub source_map: Option<String>,
}

impl ProjectRuleIr {
    pub fn new(rule_id: impl Into<String>, phase: ProjectRulePhase) -> Self {
        Self {
            schema_version: PROJECT_RULE_IR_SCHEMA_VERSION.to_string(),
            rule_id: rule_id.into(),
            phase,
            enabled: true,
            trigger: RuleTrigger::Always,
            statements: Vec::new(),
            operations: Vec::new(),
            source_map: None,
        }
    }

    pub fn validate(&self) -> RuleIrValidationReport {
        validate_rule_ir(self)
    }

    pub fn stable_hash(&self) -> String {
        stable_ir_hash(self)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProjectRulePhase {
    FixedUpdate,
    Update,
    PostPhysics,
    EventHandler,
}

impl ProjectRulePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixedUpdate => "FixedUpdate",
            Self::Update => "Update",
            Self::PostPhysics => "PostPhysics",
            Self::EventHandler => "EventHandler",
        }
    }

    pub fn to_runtime_phase(self) -> RulePhase {
        match self {
            Self::FixedUpdate => RulePhase::FixedUpdate,
            Self::Update => RulePhase::FrameUpdate,
            Self::PostPhysics => RulePhase::PostPhysics,
            Self::EventHandler => RulePhase::EventHandler,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleTrigger {
    Always,
    ActionPressed { action_id: String },
    EventReceived { event_type: String },
}

impl Default for RuleTrigger {
    fn default() -> Self {
        Self::Always
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleStatement {
    Operation {
        operation: RuleOperation,
    },
    When {
        condition: RuleCondition,
        #[serde(default)]
        statements: Vec<RuleStatement>,
    },
    ForEachQuery {
        query: RuleQueryLiteral,
        #[serde(default)]
        statements: Vec<RuleStatement>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleCondition {
    Always,
    ActionPressed { action_id: String },
    EventReceived { event_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuleQueryLiteral {
    #[serde(default)]
    pub all: Vec<String>,
    #[serde(default)]
    pub none: Vec<String>,
    #[serde(default)]
    pub include_disabled: bool,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleValueExpr {
    Literal {
        value: RuleRuntimeValue,
    },
    Field {
        component_type: String,
        field_path: String,
    },
    DeltaTime,
    Add {
        left: Box<RuleValueExpr>,
        right: Box<RuleValueExpr>,
    },
    Sub {
        left: Box<RuleValueExpr>,
        right: Box<RuleValueExpr>,
    },
    Mul {
        left: Box<RuleValueExpr>,
        right: Box<RuleValueExpr>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum RuleOperation {
    WriteComponentField {
        entity_id: String,
        component_type: String,
        field_path: String,
        value: RuleRuntimeValue,
    },
    SpawnEntity {
        entity_id: String,
        name: String,
        kind: String,
        #[serde(default)]
        components: Vec<RuleComponentLiteral>,
    },
    InstantiatePrefab {
        prefab_ref: RuntimeAssetRef,
        #[serde(default)]
        parent_entity: Option<String>,
        #[serde(default)]
        target_scene_instance: Option<u64>,
    },
    DespawnEntity {
        entity_id: String,
    },
    DespawnPrefabInstance {
        instance_id: u64,
    },
    EmitEvent {
        event_type: String,
        #[serde(default)]
        payload: Option<RuleRuntimeValue>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuleComponentLiteral {
    pub component_type: String,
    pub value: RuleRuntimeValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuleRuntimeValue {
    Null,
    Bool {
        value: bool,
    },
    I64 {
        value: i64,
    },
    F64 {
        value: f64,
    },
    String {
        value: String,
    },
    Vec3 {
        x: f32,
        y: f32,
        z: f32,
    },
    Object {
        fields: BTreeMap<String, RuleRuntimeValue>,
    },
    Array {
        values: Vec<RuleRuntimeValue>,
    },
}

impl RuleRuntimeValue {
    pub fn to_runtime_value(&self) -> RuntimeValue {
        match self {
            Self::Null => RuntimeValue::Null,
            Self::Bool { value } => RuntimeValue::Bool(*value),
            Self::I64 { value } => RuntimeValue::I64(*value),
            Self::F64 { value } => RuntimeValue::F64(*value),
            Self::String { value } => RuntimeValue::String(value.clone()),
            Self::Vec3 { x, y, z } => RuntimeValue::Vec3(Vec3 {
                x: *x,
                y: *y,
                z: *z,
            }),
            Self::Object { fields } => RuntimeValue::Object(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_runtime_value()))
                    .collect(),
            ),
            Self::Array { values } => RuntimeValue::Array(
                values
                    .iter()
                    .map(RuleRuntimeValue::to_runtime_value)
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleIrValidationReport {
    pub status: RuleIrValidationStatus,
    pub diagnostics: Vec<RuleIrDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleIrValidationStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleIrDiagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

impl RuleIrDiagnostic {
    fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path,
            suggestion,
        }
    }
}

pub fn validate_rule_ir(ir: &ProjectRuleIr) -> RuleIrValidationReport {
    let mut diagnostics = Vec::new();
    if ir.schema_version != PROJECT_RULE_IR_SCHEMA_VERSION {
        diagnostics.push(RuleIrDiagnostic::error(
            "InvalidRuleIrSchema",
            format!("schemaVersion must be {}", PROJECT_RULE_IR_SCHEMA_VERSION),
            Some("schemaVersion".to_string()),
            None,
        ));
    }
    if ir.rule_id.trim().is_empty() {
        diagnostics.push(RuleIrDiagnostic::error(
            "MissingRuleId",
            "ruleId is required",
            Some("ruleId".to_string()),
            Some("Use a stable project rule id.".to_string()),
        ));
    }
    validate_trigger(&ir.trigger, "trigger", &mut diagnostics);
    for (index, statement) in ir.statements.iter().enumerate() {
        validate_statement(
            statement,
            format!("statements[{}]", index),
            &mut diagnostics,
        );
    }
    for (index, operation) in ir.operations.iter().enumerate() {
        validate_operation(operation, index, &mut diagnostics);
    }
    RuleIrValidationReport {
        status: if diagnostics.is_empty() {
            RuleIrValidationStatus::Success
        } else {
            RuleIrValidationStatus::Failed
        },
        diagnostics,
    }
}

fn validate_trigger(trigger: &RuleTrigger, path: &str, diagnostics: &mut Vec<RuleIrDiagnostic>) {
    match trigger {
        RuleTrigger::Always => {}
        RuleTrigger::ActionPressed { action_id } => {
            if action_id.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingActionId",
                    "actionPressed trigger requires actionId",
                    Some(format!("{}.actionId", path)),
                    None,
                ));
            }
        }
        RuleTrigger::EventReceived { event_type } => {
            if event_type.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEventType",
                    "eventReceived trigger requires eventType",
                    Some(format!("{}.eventType", path)),
                    None,
                ));
            }
        }
    }
}

fn validate_condition(
    condition: &RuleCondition,
    path: &str,
    diagnostics: &mut Vec<RuleIrDiagnostic>,
) {
    match condition {
        RuleCondition::Always => {}
        RuleCondition::ActionPressed { action_id } => {
            if action_id.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingActionId",
                    "actionPressed condition requires actionId",
                    Some(format!("{}.actionId", path)),
                    None,
                ));
            }
        }
        RuleCondition::EventReceived { event_type } => {
            if event_type.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEventType",
                    "eventReceived condition requires eventType",
                    Some(format!("{}.eventType", path)),
                    None,
                ));
            }
        }
    }
}

fn validate_statement(
    statement: &RuleStatement,
    path: String,
    diagnostics: &mut Vec<RuleIrDiagnostic>,
) {
    match statement {
        RuleStatement::Operation { operation } => {
            validate_operation_at_path(operation, &format!("{}.operation", path), diagnostics)
        }
        RuleStatement::When {
            condition,
            statements,
        } => {
            validate_condition(condition, &format!("{}.condition", path), diagnostics);
            if statements.is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "EmptyWhenStatement",
                    "when statement requires at least one child statement",
                    Some(format!("{}.statements", path)),
                    None,
                ));
            }
            for (index, child) in statements.iter().enumerate() {
                validate_statement(
                    child,
                    format!("{}.statements[{}]", path, index),
                    diagnostics,
                );
            }
        }
        RuleStatement::ForEachQuery { query, statements } => {
            validate_query(query, &format!("{}.query", path), diagnostics);
            if statements.is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "EmptyForEachQueryStatement",
                    "forEachQuery statement requires at least one child statement",
                    Some(format!("{}.statements", path)),
                    None,
                ));
            }
            for (index, child) in statements.iter().enumerate() {
                validate_statement(
                    child,
                    format!("{}.statements[{}]", path, index),
                    diagnostics,
                );
            }
        }
    }
}

fn validate_query(query: &RuleQueryLiteral, path: &str, diagnostics: &mut Vec<RuleIrDiagnostic>) {
    if query.all.is_empty() {
        diagnostics.push(RuleIrDiagnostic::error(
            "EmptyQueryAll",
            "forEachQuery requires at least one component in all",
            Some(format!("{}.all", path)),
            None,
        ));
    }
    for (index, component_type) in query.all.iter().enumerate() {
        if component_type.trim().is_empty() {
            diagnostics.push(RuleIrDiagnostic::error(
                "MissingComponentType",
                "query component type cannot be empty",
                Some(format!("{}.all[{}]", path, index)),
                None,
            ));
        }
    }
    for (index, component_type) in query.none.iter().enumerate() {
        if component_type.trim().is_empty() {
            diagnostics.push(RuleIrDiagnostic::error(
                "MissingComponentType",
                "query excluded component type cannot be empty",
                Some(format!("{}.none[{}]", path, index)),
                None,
            ));
        }
    }
}

fn validate_operation(
    operation: &RuleOperation,
    index: usize,
    diagnostics: &mut Vec<RuleIrDiagnostic>,
) {
    validate_operation_at_path(operation, &format!("operations[{}]", index), diagnostics)
}

fn validate_operation_at_path(
    operation: &RuleOperation,
    path: &str,
    diagnostics: &mut Vec<RuleIrDiagnostic>,
) {
    match operation {
        RuleOperation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            ..
        } => {
            if entity_id.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEntityId",
                    "writeComponentField requires entityId",
                    Some(format!("{}.entityId", path)),
                    None,
                ));
            }
            if ComponentTypeId::from(component_type.as_str())
                .as_str()
                .is_empty()
            {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingComponentType",
                    "writeComponentField requires componentType",
                    Some(format!("{}.componentType", path)),
                    None,
                ));
            }
            if let Err(error) = FieldPath::parse(field_path) {
                diagnostics.push(RuleIrDiagnostic::error(
                    "InvalidFieldPath",
                    format!("invalid fieldPath: {}", error.code),
                    Some(format!("{}.fieldPath", path)),
                    Some("Use simple dot paths without array indexes.".to_string()),
                ));
            }
        }
        RuleOperation::SpawnEntity {
            entity_id,
            name,
            kind,
            ..
        } => {
            if EntityId::from(entity_id.as_str()).as_str().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEntityId",
                    "spawnEntity requires entityId",
                    Some(format!("{}.entityId", path)),
                    None,
                ));
            }
            if name.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEntityName",
                    "spawnEntity requires name",
                    Some(format!("{}.name", path)),
                    None,
                ));
            }
            if kind.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEntityKind",
                    "spawnEntity requires kind",
                    Some(format!("{}.kind", path)),
                    None,
                ));
            }
        }
        RuleOperation::InstantiatePrefab { prefab_ref, .. } => {
            if prefab_ref.id.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingPrefabRef",
                    "instantiatePrefab requires prefabRef.id",
                    Some(format!("{}.prefabRef.id", path)),
                    None,
                ));
            }
        }
        RuleOperation::DespawnEntity { entity_id } => {
            if entity_id.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEntityId",
                    "despawnEntity requires entityId",
                    Some(format!("{}.entityId", path)),
                    None,
                ));
            }
        }
        RuleOperation::DespawnPrefabInstance { .. } => {}
        RuleOperation::EmitEvent { event_type, .. } => {
            if event_type.trim().is_empty() {
                diagnostics.push(RuleIrDiagnostic::error(
                    "MissingEventType",
                    "emitEvent requires eventType",
                    Some(format!("{}.eventType", path)),
                    None,
                ));
            }
        }
    }
}

pub fn stable_ir_hash(ir: &ProjectRuleIr) -> String {
    let canonical = serde_json::to_string(ir).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_ir_validates_write_component_field() {
        let mut ir = ProjectRuleIr::new("project.rule.move", ProjectRulePhase::Update);
        ir.operations.push(RuleOperation::WriteComponentField {
            entity_id: "entity-a".to_string(),
            component_type: "Transform".to_string(),
            field_path: "localPosition".to_string(),
            value: RuleRuntimeValue::Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        });

        let report = ir.validate();

        assert_eq!(report.status, RuleIrValidationStatus::Success);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn rule_ir_rejects_invalid_field_path() {
        let mut ir = ProjectRuleIr::new("project.rule.bad", ProjectRulePhase::Update);
        ir.operations.push(RuleOperation::WriteComponentField {
            entity_id: "entity-a".to_string(),
            component_type: "Transform".to_string(),
            field_path: "items[0].count".to_string(),
            value: RuleRuntimeValue::I64 { value: 1 },
        });

        let report = ir.validate();

        assert_eq!(report.status, RuleIrValidationStatus::Failed);
        assert_eq!(report.diagnostics[0].code, "InvalidFieldPath");
    }

    #[test]
    fn rule_ir_hash_is_stable_for_same_content() {
        let ir_a = ProjectRuleIr::new("project.rule.same", ProjectRulePhase::FixedUpdate);
        let ir_b = ProjectRuleIr::new("project.rule.same", ProjectRulePhase::FixedUpdate);

        assert_eq!(ir_a.stable_hash(), ir_b.stable_hash());
    }

    #[test]
    fn rule_ir_validates_action_trigger_instantiate_prefab_statement() {
        let mut ir = ProjectRuleIr::new("project.rule.fire", ProjectRulePhase::Update);
        ir.trigger = RuleTrigger::ActionPressed {
            action_id: "fire".to_string(),
        };
        ir.statements.push(RuleStatement::Operation {
            operation: RuleOperation::InstantiatePrefab {
                prefab_ref: crate::runtime_package::RuntimeAssetRef {
                    id: "asset.prefab.projectile".to_string(),
                    asset_type: "prefab".to_string(),
                    guid: None,
                    sub_asset: None,
                },
                parent_entity: Some("entity.player".to_string()),
                target_scene_instance: None,
            },
        });

        let report = ir.validate();

        assert_eq!(report.status, RuleIrValidationStatus::Success);
    }

    #[test]
    fn rule_ir_validates_for_each_query_statement() {
        let mut ir = ProjectRuleIr::new("project.rule.move_projectiles", ProjectRulePhase::Update);
        ir.statements.push(RuleStatement::ForEachQuery {
            query: RuleQueryLiteral {
                all: vec![
                    "Transform".to_string(),
                    "project.ProjectileMotion".to_string(),
                ],
                ..RuleQueryLiteral::default()
            },
            statements: vec![RuleStatement::Operation {
                operation: RuleOperation::WriteComponentField {
                    entity_id: "$entity".to_string(),
                    component_type: "Transform".to_string(),
                    field_path: "local_position.x".to_string(),
                    value: RuleRuntimeValue::F64 { value: 1.0 },
                },
            }],
        });

        let report = ir.validate();

        assert_eq!(report.status, RuleIrValidationStatus::Success);
    }

    #[test]
    fn rule_ir_rejects_empty_for_each_query() {
        let mut ir = ProjectRuleIr::new("project.rule.bad_query", ProjectRulePhase::Update);
        ir.statements.push(RuleStatement::ForEachQuery {
            query: RuleQueryLiteral::default(),
            statements: vec![RuleStatement::Operation {
                operation: RuleOperation::EmitEvent {
                    event_type: "project.event.hit".to_string(),
                    payload: None,
                },
            }],
        });

        let report = ir.validate();

        assert_eq!(report.status, RuleIrValidationStatus::Failed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "EmptyQueryAll"));
    }
}
