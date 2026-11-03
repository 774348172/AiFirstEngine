//! Bounded fact invocation of existing AOT rules, separate from a live scene World.
use crate::{
    archetype::ComponentValue,
    component_value::RuntimeValue,
    components::{ComponentTypeId, Hierarchy},
    ids::EntityId,
    logic_executor::{
        LogicContext, LogicExecutor, LogicResult, LogicStatus, RulePhase, RustAotExecutor,
        RustAotRule,
    },
    rule_ir::{
        ProjectRuleIr, RuleCondition, RuleOperation, RuleRuntimeValue, RuleStatement, RuleTrigger,
    },
    world::World,
    world_api::WorldWriteApi,
};
use project_runtime_sdk::ProjectRuntimeValue as Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleSlotContract {
    pub rule_id: String,
    pub ir_source: String,
    pub input_components: Vec<String>,
    pub output_fields: BTreeMap<String, Vec<String>>,
}

pub fn validate_slot(ir: &ProjectRuleIr, contract: &RuleSlotContract) -> Result<(), String> {
    if ir.validate().status == crate::rule_ir::RuleIrValidationStatus::Failed {
        return Err("invalid Rule IR".into());
    }
    if ir.rule_id != contract.rule_id
        || !ir.enabled
        || !matches!(ir.trigger, RuleTrigger::Always)
        || ir.phase != crate::rule_ir::ProjectRulePhase::FixedUpdate
        || contract.input_components.is_empty()
        || contract.input_components.len() > 16
        || contract
            .input_components
            .iter()
            .any(|c| c.is_empty() || c.starts_with("engine."))
        || contract.output_fields.is_empty()
    {
        return Err("invalid or unsupported RuleSlot contract/trigger/phase".into());
    }
    for (component, fields) in &contract.output_fields {
        if !contract.input_components.contains(component)
            || fields.is_empty()
            || fields.len() > 16
            || fields
                .iter()
                .any(|f| f.is_empty() || !f.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            return Err("RuleSlot outputs must name declared components and flat fields".into());
        }
    }
    fn operation(op: &RuleOperation, c: &RuleSlotContract, in_query: bool) -> Result<(), String> {
        match op {
            RuleOperation::WriteComponentField {
                entity_id,
                component_type,
                field_path,
                value,
            } if in_query
                && entity_id == "$entity"
                && c.output_fields
                    .get(component_type)
                    .is_some_and(|f| f.contains(field_path))
                && matches!(
                    value,
                    RuleRuntimeValue::Bool { .. }
                        | RuleRuntimeValue::I64 { .. }
                        | RuleRuntimeValue::F64 { .. }
                        | RuleRuntimeValue::String { .. }
                ) =>
            {
                Ok(())
            }
            _ => Err(
                "RuleSlot only permits declared scalar field writes to its queried fact entity"
                    .into(),
            ),
        }
    }
    fn statement(s: &RuleStatement, c: &RuleSlotContract, in_query: bool) -> Result<(), String> {
        match s {
            RuleStatement::Operation { operation: op } => operation(op, c, in_query),
            RuleStatement::ForEachQuery { query, statements } => {
                if query
                    .all
                    .iter()
                    .chain(&query.none)
                    .any(|v| !c.input_components.contains(v))
                {
                    return Err("RuleSlot query references undeclared input component".into());
                }
                for s in statements {
                    statement(s, c, true)?;
                }
                Ok(())
            }
            RuleStatement::When {
                condition: RuleCondition::Always,
                statements,
            } => {
                for s in statements {
                    statement(s, c, in_query)?;
                }
                Ok(())
            }
            _ => Err("RuleSlot does not support input actions or events".into()),
        }
    }
    if !ir.statements.is_empty() && !ir.operations.is_empty() {
        return Err("ambiguous RuleSlot statements and operations".into());
    }
    for s in &ir.statements {
        statement(s, contract, false)?;
    }
    for op in &ir.operations {
        operation(op, contract, false)?;
    }
    Ok(())
}

fn scalar(value: &Value) -> Result<RuntimeValue, String> {
    match value {
        Value::Bool(v) => Ok(RuntimeValue::Bool(*v)),
        Value::Integer(v) => Ok(RuntimeValue::I64(*v)),
        Value::Number(v) if v.is_finite() => Ok(RuntimeValue::F64(*v)),
        Value::String(v) if v.len() <= 1024 => Ok(RuntimeValue::String(v.clone())),
        _ => Err("RuleSlot facts require finite scalar values or flat scalar objects".into()),
    }
}

/// The generated glue supplies a compiler-validated contract and its actual generated AOT function.
pub fn execute(
    contract: &RuleSlotContract,
    facts: &BTreeMap<String, Value>,
    rule: for<'a> fn(&mut LogicContext<'a>) -> LogicResult,
) -> Result<BTreeMap<String, BTreeMap<String, Value>>, String> {
    if facts.len() > 16 || facts.keys().any(|c| !contract.input_components.contains(c)) {
        return Err("RuleSlot facts contain undeclared components".into());
    }
    let mut world = World::new();
    let entity = EntityId::from("rule-slot.input");
    world
        .try_spawn_entity(
            entity.clone(),
            "Rule facts",
            "rule-facts",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
    for (component, value) in facts {
        let value = match value {
            Value::Object(fields) if fields.len() <= 32 => RuntimeValue::Object(
                fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), scalar(v)?)))
                    .collect::<Result<_, String>>()?,
            ),
            other => scalar(other)?,
        };
        world
            .try_insert_dynamic_component(
                entity.clone(),
                ComponentTypeId::from(component.as_str()),
                value,
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    let mut context = LogicContext::new(
        0,
        1.0 / 60.0,
        RulePhase::FixedUpdate,
        WorldWriteApi::new(&mut world),
    );
    let mut executor = RustAotExecutor::new();
    executor.register_rule(&contract.rule_id, RustAotRule::new(rule));
    let result = executor.run(&contract.rule_id, &mut context);
    if result.status != LogicStatus::Applied
        || !result.errors.is_empty()
        || !result.command_ids.is_empty()
    {
        return Err(format!("RuleSlot execution failed: {:?}", result.errors));
    }
    drop(context);
    let mut output: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    for write in result.writes {
        let component = write.component_type.as_str();
        if write.entity_id != entity
            || !contract
                .output_fields
                .get(component)
                .is_some_and(|f| f.contains(&write.field))
        {
            return Err("RuleSlot attempted an undeclared output".into());
        }
        let Some(ComponentValue::Dynamic {
            value: RuntimeValue::Object(fields),
            ..
        }) = world.component_value(&entity, &write.component_type)
        else {
            return Err("RuleSlot output component is missing".into());
        };
        let value = match fields.get(&write.field) {
            Some(RuntimeValue::Bool(v)) => Value::Bool(*v),
            Some(RuntimeValue::I64(v)) => Value::Integer(*v),
            Some(RuntimeValue::F64(v)) if v.is_finite() => Value::Number(*v),
            Some(RuntimeValue::String(v)) => Value::String(v.clone()),
            _ => return Err("invalid RuleSlot scalar output".into()),
        };
        output
            .entry(component.to_string())
            .or_default()
            .insert(write.field, value);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        field_path::FieldPath,
        logic_executor::ExecutorKind,
        rule_ir::{ProjectRulePhase, RuleQueryLiteral},
    };

    fn contract() -> RuleSlotContract {
        RuleSlotContract {
            rule_id: "test.slot".into(),
            ir_source: "Rules/test.rule.ir.json".into(),
            input_components: vec!["project.request".into(), "project.ready".into()],
            output_fields: BTreeMap::from([("project.request".into(), vec!["answer".into()])]),
        }
    }
    fn rule(context: &mut LogicContext<'_>) -> LogicResult {
        let mut result = LogicResult::applied("test.slot", ExecutorKind::RustAot);
        for entity in context.query(crate::query::QuerySpec::all([
            ComponentTypeId::from("project.ready"),
            ComponentTypeId::from("project.request"),
        ])) {
            result.writes.push(
                context
                    .write_component_field(
                        entity,
                        ComponentTypeId::from("project.request"),
                        &FieldPath::parse("answer").unwrap(),
                        RuntimeValue::I64(2),
                    )
                    .unwrap(),
            );
        }
        result
    }
    fn facts(ready: bool) -> BTreeMap<String, Value> {
        let mut facts = BTreeMap::from([(
            "project.request".into(),
            Value::Object(BTreeMap::from([("answer".into(), Value::Integer(0))])),
        )]);
        if ready {
            facts.insert("project.ready".into(), Value::Bool(true));
        }
        facts
    }
    #[test]
    fn slot_executes_aot_and_does_not_retain_previous_facts() {
        let output = execute(&contract(), &facts(true), rule).unwrap();
        assert_eq!(output["project.request"]["answer"], Value::Integer(2));
        assert!(execute(&contract(), &facts(false), rule)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn slot_rejects_unknown_facts_and_undeclared_writes() {
        let mut input = facts(true);
        input.insert("project.secret".into(), Value::Bool(true));
        assert!(execute(&contract(), &input, rule).is_err());
        let mut contract = contract();
        contract.output_fields.get_mut("project.request").unwrap()[0] = "other".into();
        assert!(execute(&contract, &facts(true), rule).is_err());
    }
    #[test]
    fn slot_compilation_rejects_events_and_noncontract_fields() {
        let mut ir = ProjectRuleIr::new("test.slot", ProjectRulePhase::FixedUpdate);
        ir.statements.push(RuleStatement::ForEachQuery {
            query: RuleQueryLiteral {
                all: vec!["project.request".into()],
                ..Default::default()
            },
            statements: vec![RuleStatement::Operation {
                operation: RuleOperation::WriteComponentField {
                    entity_id: "$entity".into(),
                    component_type: "project.request".into(),
                    field_path: "answer".into(),
                    value: RuleRuntimeValue::I64 { value: 2 },
                },
            }],
        });
        assert!(validate_slot(&ir, &contract()).is_ok());
        ir.trigger = RuleTrigger::EventReceived {
            event_type: "pickup".into(),
        };
        assert!(validate_slot(&ir, &contract()).is_err());
        ir.trigger = RuleTrigger::Always;
        let mut contract = contract();
        contract.output_fields.get_mut("project.request").unwrap()[0] = "other".into();
        assert!(validate_slot(&ir, &contract).is_err());
    }
}
