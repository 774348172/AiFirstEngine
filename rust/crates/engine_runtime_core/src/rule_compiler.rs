use crate::rule_ir::{
    stable_ir_hash, ProjectRuleIr, RuleCondition, RuleIrDiagnostic, RuleIrValidationStatus,
    RuleOperation, RuleQueryLiteral, RuleRuntimeValue, RuleStatement, RuleTrigger,
    PROJECT_RULE_IR_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const RULE_COMPILER_VERSION: &str = "rule-compiler.v1";
pub const ENGINE_RULE_ABI_VERSION: &str = "engine-rule-abi.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleCompileRequest {
    pub project_id: String,
    pub target: String,
    pub build_profile: String,
    pub generated_root: PathBuf,
}

impl RuleCompileRequest {
    pub fn dev_desktop(generated_root: impl Into<PathBuf>) -> Self {
        Self {
            project_id: "project".to_string(),
            target: "dev-desktop".to_string(),
            build_profile: "debug".to_string(),
            generated_root: generated_root.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleBuildCacheKey {
    pub rule_id: String,
    pub ir_hash: String,
    pub schema_version: String,
    pub compiler_version: String,
    pub engine_rule_abi_version: String,
    pub target: String,
    pub build_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleBuildCacheRecord {
    pub key: RuleBuildCacheKey,
    pub artifact_id: String,
    pub generated_source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleBuildDecision {
    pub cache_hit: bool,
    pub reason: String,
    pub key: RuleBuildCacheKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleCompileStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleCompileDiagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

impl From<RuleIrDiagnostic> for RuleCompileDiagnostic {
    fn from(value: RuleIrDiagnostic) -> Self {
        Self {
            code: value.code,
            message: value.message,
            path: value.path,
            suggestion: value.suggestion,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleCompileReport {
    pub status: RuleCompileStatus,
    pub rule_id: String,
    pub ir_hash: String,
    pub artifact_id: Option<String>,
    pub generated_source_path: Option<String>,
    pub generated_source: Option<String>,
    pub build_decision: RuleBuildDecision,
    pub diagnostics: Vec<RuleCompileDiagnostic>,
}

pub struct RuleCompiler;

impl RuleCompiler {
    pub fn compile(
        request: &RuleCompileRequest,
        ir: &ProjectRuleIr,
        previous: Option<&RuleBuildCacheRecord>,
    ) -> RuleCompileReport {
        let ir_hash = stable_ir_hash(ir);
        let key = build_cache_key(request, ir, &ir_hash);
        let build_decision = build_decision(key.clone(), previous);
        let validation = ir.validate();
        let mut diagnostics = validation
            .diagnostics
            .into_iter()
            .map(RuleCompileDiagnostic::from)
            .collect::<Vec<_>>();

        if validation.status == RuleIrValidationStatus::Failed {
            return RuleCompileReport {
                status: RuleCompileStatus::Failed,
                rule_id: ir.rule_id.clone(),
                ir_hash,
                artifact_id: None,
                generated_source_path: None,
                generated_source: None,
                build_decision,
                diagnostics,
            };
        }

        let generated_source = generate_rust_source(ir);
        let artifact_id = artifact_id(ir, &ir_hash);
        let generated_source_path = generated_source_path(request, ir, &ir_hash);
        if generated_source.contains("unsupported_rule_operation") {
            diagnostics.push(RuleCompileDiagnostic {
                code: "UnsupportedRuleOperation".to_string(),
                message: "RuleCompiler generated an unsupported operation marker.".to_string(),
                path: Some("operations".to_string()),
                suggestion: Some("Use supported Rule IR operations only.".to_string()),
            });
        }
        RuleCompileReport {
            status: if diagnostics.is_empty() {
                RuleCompileStatus::Success
            } else {
                RuleCompileStatus::Failed
            },
            rule_id: ir.rule_id.clone(),
            ir_hash,
            artifact_id: Some(artifact_id),
            generated_source_path: Some(generated_source_path.display().to_string()),
            generated_source: Some(generated_source),
            build_decision,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedRuleRegistrySource {
    pub module_name: String,
    pub source: String,
    #[serde(default)]
    pub rule_ids: Vec<String>,
}

pub fn generate_static_registry_source(rules: &[ProjectRuleIr]) -> GeneratedRuleRegistrySource {
    let mut registrations = String::new();
    let mut rule_ids = Vec::new();
    for ir in rules {
        let fn_name = generated_fn_name(&ir.rule_id);
        rule_ids.push(ir.rule_id.clone());
        registrations.push_str(&format!(
            r#"    registry.register_generated_rule("{rule_id}", {fn_name});
"#,
            rule_id = escape(&ir.rule_id),
            fn_name = fn_name
        ));
    }
    GeneratedRuleRegistrySource {
        module_name: "generated_registry".to_string(),
        rule_ids,
        source: format!(
            r#"use engine_runtime::rule_registry::RuleModuleRegistry;
use super::generated_rules::*;

pub fn register_generated_rules(registry: &mut RuleModuleRegistry) {{
{registrations}}}
"#
        ),
    }
}

pub fn build_cache_key(
    request: &RuleCompileRequest,
    ir: &ProjectRuleIr,
    ir_hash: &str,
) -> RuleBuildCacheKey {
    RuleBuildCacheKey {
        rule_id: ir.rule_id.clone(),
        ir_hash: ir_hash.to_string(),
        schema_version: PROJECT_RULE_IR_SCHEMA_VERSION.to_string(),
        compiler_version: RULE_COMPILER_VERSION.to_string(),
        engine_rule_abi_version: ENGINE_RULE_ABI_VERSION.to_string(),
        target: request.target.clone(),
        build_profile: request.build_profile.clone(),
    }
}

pub fn build_decision(
    key: RuleBuildCacheKey,
    previous: Option<&RuleBuildCacheRecord>,
) -> RuleBuildDecision {
    let cache_hit = previous.is_some_and(|record| record.key == key);
    RuleBuildDecision {
        cache_hit,
        reason: if cache_hit {
            "cache-key-match".to_string()
        } else {
            "cache-miss-or-missing-record".to_string()
        },
        key,
    }
}

pub fn generate_rust_source(ir: &ProjectRuleIr) -> String {
    let fn_name = generated_fn_name(&ir.rule_id);
    let mut body = String::new();
    body.push_str(&generate_trigger_source(&ir.trigger, &ir.rule_id));
    if ir.statements.is_empty() {
        for operation in &ir.operations {
            body.push_str(&generate_operation_source(operation));
        }
    } else {
        for statement in &ir.statements {
            body.push_str(&generate_statement_source(statement, 0));
        }
    }
    format!(
        r#"use engine_runtime::logic_executor::{{ExecutorKind, LogicContext, LogicResult}};
use engine_runtime::components::ComponentTypeId;
use engine_runtime::field_path::FieldPath;
use engine_runtime::ids::EntityId;
use engine_runtime::component_value::RuntimeValue;
use engine_runtime::math::Vec3;
use engine_runtime::query::QuerySpec;
use engine_runtime::runtime_package::RuntimeAssetRef;
use engine_runtime::runtime_instance::RuntimeInstanceId;

pub fn {fn_name}(context: &mut LogicContext<'_>) -> LogicResult {{
    let mut result = LogicResult::applied("{rule_id}", ExecutorKind::RustAot);
{body}
    result
}}
"#,
        fn_name = fn_name,
        rule_id = ir.rule_id,
        body = indent_source(&body, 4)
    )
}

fn generate_trigger_source(trigger: &RuleTrigger, rule_id: &str) -> String {
    match trigger {
        RuleTrigger::Always => String::new(),
        RuleTrigger::ActionPressed { action_id } => format!(
            r#"if !context.action_pressed("{action_id}") {{
    return LogicResult::skipped("{rule_id}", ExecutorKind::RustAot);
}}
"#,
            action_id = escape(action_id),
            rule_id = escape(rule_id)
        ),
        RuleTrigger::EventReceived { event_type } => format!(
            r#"result.status = engine_runtime::logic_executor::LogicStatus::Unsupported;
result.errors.push(engine_runtime::logic_executor::LogicError {{
    code: "event_trigger_not_implemented",
    message: "eventReceived trigger '{}' requires RuntimeEventQueue and is not enabled in this build".to_string(),
}});
return result;
"#,
            escape(event_type)
        ),
    }
}

fn generate_statement_source(statement: &RuleStatement, depth: usize) -> String {
    match statement {
        RuleStatement::Operation { operation } => generate_operation_source(operation),
        RuleStatement::When {
            condition,
            statements,
        } => {
            let mut source = format!("if {} {{\n", condition_source(condition));
            for statement in statements {
                source.push_str(&indent_source(
                    &generate_statement_source(statement, depth + 1),
                    4,
                ));
                source.push('\n');
            }
            source.push_str("}\n");
            source
        }
        RuleStatement::ForEachQuery { query, statements } => {
            let variable = format!("entity_id_{}", depth);
            let mut source = format!(
                r#"for {variable} in context.query({query}) {{
"#,
                variable = variable,
                query = query_source(query)
            );
            for statement in statements {
                source.push_str(&indent_source(
                    &generate_statement_source_for_entity(statement, &variable, depth + 1),
                    4,
                ));
                source.push('\n');
            }
            source.push_str("}\n");
            source
        }
    }
}

fn generate_statement_source_for_entity(
    statement: &RuleStatement,
    entity_variable: &str,
    depth: usize,
) -> String {
    match statement {
        RuleStatement::Operation { operation } => {
            generate_operation_source_for_entity(operation, entity_variable)
        }
        RuleStatement::When {
            condition,
            statements,
        } => {
            let mut source = format!("if {} {{\n", condition_source(condition));
            for statement in statements {
                source.push_str(&indent_source(
                    &generate_statement_source_for_entity(statement, entity_variable, depth + 1),
                    4,
                ));
                source.push('\n');
            }
            source.push_str("}\n");
            source
        }
        RuleStatement::ForEachQuery { .. } => generate_statement_source(statement, depth),
    }
}

fn condition_source(condition: &RuleCondition) -> String {
    match condition {
        RuleCondition::Always => "true".to_string(),
        RuleCondition::ActionPressed { action_id } => {
            format!("context.action_pressed(\"{}\")", escape(action_id))
        }
        RuleCondition::EventReceived { event_type } => format!(
            "{{ result.status = engine_runtime::logic_executor::LogicStatus::Unsupported; result.errors.push(engine_runtime::logic_executor::LogicError {{ code: \"event_condition_not_implemented\", message: \"eventReceived condition '{}' requires RuntimeEventQueue\".to_string() }}); false }}",
            escape(event_type)
        ),
    }
}

fn query_source(query: &RuleQueryLiteral) -> String {
    let all = query
        .all
        .iter()
        .map(|component| format!("ComponentTypeId::from(\"{}\")", escape(component)))
        .collect::<Vec<_>>()
        .join(", ");
    let none = query
        .none
        .iter()
        .map(|component| format!("ComponentTypeId::from(\"{}\")", escape(component)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = format!("QuerySpec::all([{all}])");
    if !none.is_empty() {
        source.push_str(&format!(".excluding([{none}])"));
    }
    if query.include_disabled {
        source.push_str(".include_disabled()");
    }
    if let Some(limit) = query.limit {
        source.push_str(&format!(".limit({})", limit));
    }
    source
}

fn generate_operation_source(operation: &RuleOperation) -> String {
    generate_operation_source_inner(operation, None)
}

fn generate_operation_source_for_entity(
    operation: &RuleOperation,
    entity_variable: &str,
) -> String {
    generate_operation_source_inner(operation, Some(entity_variable))
}

fn generate_operation_source_inner(
    operation: &RuleOperation,
    entity_variable: Option<&str>,
) -> String {
    match operation {
        RuleOperation::WriteComponentField {
            entity_id,
            component_type,
            field_path,
            value,
        } => {
            let entity_source = if entity_id == "$entity" {
                entity_variable
                    .map(|variable| variable.to_string())
                    .unwrap_or_else(|| "EntityId::from(\"$entity\")".to_string())
            } else {
                format!("EntityId::from(\"{}\")", escape(entity_id))
            };
            format!(
                r#"let field_path = FieldPath::parse("{field_path}").expect("generated field path must validate");
match context.write_component_field({entity_source}, ComponentTypeId::from("{component_type}"), &field_path, {value}) {{
    Ok(write) => result.writes.push(write),
    Err(error) => {{
        result.status = engine_runtime::logic_executor::LogicStatus::Failed;
        result.errors.push(error.into());
    }}
}}
"#,
                entity_source = entity_source,
                component_type = escape(component_type),
                field_path = escape(field_path),
                value = runtime_value_source(value)
            )
        }
        RuleOperation::SpawnEntity {
            entity_id,
            name,
            kind,
            ..
        } => format!(
            r#"let _command_id = context.enqueue_command(engine_runtime::gameplay_command::GameplayCommand::SpawnEntity {{
    entity_id: EntityId::from("{entity_id}"),
    name: "{name}".to_string(),
    kind: "{kind}".to_string(),
    enabled: true,
    parent_id: None,
    components: Vec::new(),
}});
"#,
            entity_id = escape(entity_id),
            name = escape(name),
            kind = escape(kind)
        ),
        RuleOperation::InstantiatePrefab {
            prefab_ref,
            parent_entity,
            target_scene_instance,
        } => format!(
            r#"let _command_id = context.request_instantiate_prefab(
    RuntimeAssetRef {{
        id: "{prefab_id}".to_string(),
        asset_type: "{asset_type}".to_string(),
        guid: {guid},
        sub_asset: {sub_asset},
    }},
    {parent_entity},
    {target_scene_instance},
);
"#,
            prefab_id = escape(&prefab_ref.id),
            asset_type = escape(&prefab_ref.asset_type),
            guid = option_string_source(prefab_ref.guid.as_deref()),
            sub_asset = option_string_source(prefab_ref.sub_asset.as_deref()),
            parent_entity = parent_entity
                .as_deref()
                .map(|value| format!(
                    "Some(engine_runtime::ids::SourceEntityId::from(\"{}\"))",
                    escape(value)
                ))
                .unwrap_or_else(|| "None".to_string()),
            target_scene_instance = target_scene_instance
                .map(|value| format!("Some(RuntimeInstanceId({}))", value))
                .unwrap_or_else(|| "None".to_string())
        ),
        RuleOperation::DespawnEntity { entity_id } => format!(
            r#"let _command_id = context.request_despawn_entity(EntityId::from("{entity_id}"));
"#,
            entity_id = escape(entity_id)
        ),
        RuleOperation::DespawnPrefabInstance { instance_id } => format!(
            r#"let _command_id = context.request_despawn_prefab_instance(RuntimeInstanceId({instance_id}));
"#
        ),
        RuleOperation::EmitEvent { event_type, .. } => format!(
            r#"result.status = engine_runtime::logic_executor::LogicStatus::Unsupported;
result.errors.push(engine_runtime::logic_executor::LogicError {{
    code: "emit_event_not_implemented",
    message: "emitEvent '{}' requires RuntimeEventQueue and is not enabled in this build".to_string(),
}});
"#,
            escape(event_type)
        ),
    }
}

fn runtime_value_source(value: &RuleRuntimeValue) -> String {
    match value {
        RuleRuntimeValue::Null => "RuntimeValue::Null".to_string(),
        RuleRuntimeValue::Bool { value } => format!("RuntimeValue::Bool({})", value),
        RuleRuntimeValue::I64 { value } => format!("RuntimeValue::I64({})", value),
        RuleRuntimeValue::F64 { value } => format!("RuntimeValue::F64({})", value),
        RuleRuntimeValue::String { value } => {
            format!("RuntimeValue::String(\"{}\".to_string())", escape(value))
        }
        RuleRuntimeValue::Vec3 { x, y, z } => {
            format!("RuntimeValue::Vec3(Vec3 {{ x: {x:?}, y: {y:?}, z: {z:?} }})")
        }
        RuleRuntimeValue::Object { .. } | RuleRuntimeValue::Array { .. } => {
            "RuntimeValue::Null /* unsupported nested literal in codegen v1 */".to_string()
        }
    }
}

fn generated_source_path(
    request: &RuleCompileRequest,
    ir: &ProjectRuleIr,
    ir_hash: &str,
) -> PathBuf {
    request
        .generated_root
        .join(&request.project_id)
        .join(format!("{}-{}", sanitize_id(&ir.rule_id), ir_hash))
        .join("src")
        .join("lib.rs")
}

fn artifact_id(ir: &ProjectRuleIr, ir_hash: &str) -> String {
    format!("rule-artifact:{}:{}", ir.rule_id, ir_hash)
}

fn generated_fn_name(rule_id: &str) -> String {
    format!("generated_{}", sanitize_id(rule_id))
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn option_string_source(value: Option<&str>) -> String {
    value
        .map(|value| format!("Some(\"{}\".to_string())", escape(value)))
        .unwrap_or_else(|| "None".to_string())
}

fn indent_source(value: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    value
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_ir::{
        ProjectRulePhase, RuleOperation, RuleQueryLiteral, RuleRuntimeValue, RuleStatement,
        RuleTrigger,
    };

    fn move_ir() -> ProjectRuleIr {
        let mut ir = ProjectRuleIr::new("project.rule.move", ProjectRulePhase::Update);
        ir.operations.push(RuleOperation::WriteComponentField {
            entity_id: "entity-a".to_string(),
            component_type: "Transform".to_string(),
            field_path: "localPosition".to_string(),
            value: RuleRuntimeValue::Vec3 {
                x: 3.0,
                y: 0.0,
                z: 0.0,
            },
        });
        ir
    }

    #[test]
    fn rule_compiler_generates_rust_source_for_write_field() {
        let request = RuleCompileRequest::dev_desktop("target/generated-rules");
        let report = RuleCompiler::compile(&request, &move_ir(), None);

        assert_eq!(report.status, RuleCompileStatus::Success);
        let source = report.generated_source.expect("source should exist");
        assert!(source.contains("pub fn generated_project_rule_move"));
        assert!(source.contains("write_component_field"));
        assert!(source.contains("localPosition"));
    }

    #[test]
    fn rule_compiler_reports_invalid_ir() {
        let request = RuleCompileRequest::dev_desktop("target/generated-rules");
        let mut ir = move_ir();
        ir.operations = vec![RuleOperation::WriteComponentField {
            entity_id: "entity-a".to_string(),
            component_type: "Transform".to_string(),
            field_path: "bad[0]".to_string(),
            value: RuleRuntimeValue::I64 { value: 1 },
        }];

        let report = RuleCompiler::compile(&request, &ir, None);

        assert_eq!(report.status, RuleCompileStatus::Failed);
        assert_eq!(report.diagnostics[0].code, "InvalidFieldPath");
        assert!(report.generated_source.is_none());
    }

    #[test]
    fn rule_compiler_detects_cache_hit() {
        let request = RuleCompileRequest::dev_desktop("target/generated-rules");
        let ir = move_ir();
        let first = RuleCompiler::compile(&request, &ir, None);
        let previous = RuleBuildCacheRecord {
            key: first.build_decision.key.clone(),
            artifact_id: first.artifact_id.clone().unwrap(),
            generated_source_path: first.generated_source_path.clone().unwrap(),
        };
        let second = RuleCompiler::compile(&request, &ir, Some(&previous));

        assert!(second.build_decision.cache_hit);
    }

    #[test]
    fn rule_compiler_generates_action_trigger_for_statement_ir() {
        let request = RuleCompileRequest::dev_desktop("target/generated-rules");
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

        let report = RuleCompiler::compile(&request, &ir, None);

        assert_eq!(report.status, RuleCompileStatus::Success);
        let source = report.generated_source.expect("source should exist");
        assert!(source.contains("context.action_pressed(\"fire\")"));
        assert!(source.contains("request_instantiate_prefab"));
        assert!(source.contains("asset.prefab.projectile"));
    }

    #[test]
    fn rule_compiler_generates_for_each_query_loop_for_statement_ir() {
        let request = RuleCompileRequest::dev_desktop("target/generated-rules");
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

        let report = RuleCompiler::compile(&request, &ir, None);

        assert_eq!(report.status, RuleCompileStatus::Success);
        let source = report.generated_source.expect("source should exist");
        assert!(source.contains("context.query(QuerySpec::all"));
        assert!(source.contains("project.ProjectileMotion"));
        assert!(source.contains("write_component_field(entity_id_0"));
    }

    #[test]
    fn rule_compiler_generates_static_registry_source() {
        let rules = vec![
            ProjectRuleIr::new("project.rule.fire", ProjectRulePhase::Update),
            ProjectRuleIr::new("project.rule.move", ProjectRulePhase::Update),
        ];

        let registry = generate_static_registry_source(&rules);

        assert_eq!(registry.module_name, "generated_registry");
        assert_eq!(registry.rule_ids.len(), 2);
        assert!(registry.source.contains("register_generated_rules"));
        assert!(registry.source.contains("generated_project_rule_fire"));
        assert!(registry.source.contains("project.rule.move"));
    }
}
