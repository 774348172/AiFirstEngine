use super::*;
use engine_runtime::{
    compiled_rule_slot::{validate_slot, RuleSlotContract},
    rule_compiler::{RuleCompileRequest, RuleCompileStatus, RuleCompiler},
    rule_ir::ProjectRuleIr,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SlotDeclarations {
    schema_version: String,
    slots: Vec<RuleSlotContract>,
}

pub(super) struct GeneratedSlots {
    pub source: String,
    pub source_map: Vec<GeneratedRuntimeGlueSourceMapEntry>,
}

fn error(message: impl Into<String>) -> GeneratedRuntimeGlueError {
    glue_error(
        "generated_runtime_glue.rule_slot_invalid",
        message.into(),
        "Repair the leased RuleSlot contract/IR and prepare again.",
    )
}

pub(super) fn generate(
    view: &CompilerSourceView,
) -> Result<GeneratedSlots, GeneratedRuntimeGlueError> {
    let mut generated = GeneratedSlots {
        source: String::new(),
        source_map: vec![],
    };
    let Some(bytes) = view.bytes("Rules/rule-slots.json") else {
        return Ok(generated);
    };
    let declarations: SlotDeclarations =
        serde_json::from_slice(bytes).map_err(|e| error(e.to_string()))?;
    if declarations.schema_version != "project-rule-slots.v1"
        || declarations.slots.is_empty()
        || declarations.slots.len() > 32
    {
        return Err(error("unsupported or empty RuleSlot declarations"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut modules = String::new();
    let mut arms = String::new();
    for (index, contract) in declarations.slots.iter().enumerate() {
        if !seen.insert(&contract.rule_id) {
            return Err(error("duplicate RuleSlot id"));
        }
        crate::ProjectRelativePath::parse(&contract.ir_source).map_err(|e| error(e.to_string()))?;
        if !contract.ir_source.starts_with("Rules/") {
            return Err(error("RuleSlot IR must be a leased Rules source"));
        }
        let bytes = view
            .bytes(&contract.ir_source)
            .ok_or_else(|| error(format!("missing RuleSlot IR: {}", contract.ir_source)))?;
        let ir: ProjectRuleIr = serde_json::from_slice(bytes).map_err(|e| error(e.to_string()))?;
        validate_slot(&ir, contract).map_err(error)?;
        let report = RuleCompiler::compile(
            &RuleCompileRequest::dev_desktop("RuntimeGlue/GeneratedRules"),
            &ir,
            None,
        );
        if report.status != RuleCompileStatus::Success {
            return Err(error(format!("{:?}", report.diagnostics)));
        }
        let source = report.generated_source.unwrap();
        let symbol = format!(
            "generated_{}",
            ir.rule_id
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                })
                .collect::<String>()
        );
        modules.push_str(&format!(
            "#[allow(unused_imports)] mod slot_{index} {{\n{source}\n}}\n"
        ));
        let contract_json = serde_json::to_string(contract).unwrap();
        arms.push_str(&format!(r#"{id} => {{
            static CONTRACT: std::sync::OnceLock<engine_runtime::compiled_rule_slot::RuleSlotContract> = std::sync::OnceLock::new();
            let contract = CONTRACT.get_or_init(|| serde_json::from_str({contract_json}).expect("Compiler-validated RuleSlot contract"));
            engine_runtime::compiled_rule_slot::execute(contract, facts, slot_{index}::{symbol})
                .map_err(|e| project_game_sdk::GameError::new(project_game_sdk::GameErrorCode::Failed, e))
        }},
"#, id=rust_string(&contract.rule_id), contract_json=rust_string(&contract_json)));
        generated
            .source_map
            .push(GeneratedRuntimeGlueSourceMapEntry {
                generated_item: format!("compiled_rule_slots::slot_{index}::{symbol}"),
                origin_kind: "compiled_rule_ir".into(),
                source_path: contract.ir_source.clone(),
                source_symbol: contract.rule_id.clone(),
            });
    }
    generated.source = format!(
        r#"mod compiled_rule_slots {{
        {modules}
        pub fn dispatch(id: &str, facts: &project_game_sdk::rule_slots::RuleSlotFacts) -> project_game_sdk::GameResult<project_game_sdk::rule_slots::RuleSlotOutput> {{
            match id {{ {arms} _ => Err(project_game_sdk::GameError::new(project_game_sdk::GameErrorCode::Unsupported, format!("unknown compiled RuleSlot: {{id}}"))) }}
        }}
    }}"#
    );
    Ok(generated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::rule_ir::{
        ProjectRulePhase, RuleOperation, RuleQueryLiteral, RuleRuntimeValue, RuleStatement,
    };

    fn fixture(value: i64) -> CompilerSourceView {
        let mut view = super::super::tests::source_view("pub fn project_game() {}", "{}");
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
                    value: RuleRuntimeValue::I64 { value },
                },
            }],
        });
        view.files.insert(
            "Rules/test.rule.ir.json".into(),
            serde_json::to_vec(&ir).unwrap(),
        );
        view.files.insert("Rules/rule-slots.json".into(), serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"project-rule-slots.v1", "slots":[{
                "ruleId":"test.slot", "irSource":"Rules/test.rule.ir.json", "inputComponents":["project.request"], "outputFields":{"project.request":["answer"]}
            }]
        })).unwrap());
        view
    }
    #[test]
    fn compiled_slots_use_real_codegen_and_change_glue_identity() {
        let first = generate_runtime_glue(
            &fixture(2),
            Some(&super::super::tests::runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        let changed = generate_runtime_glue(
            &fixture(1),
            Some(&super::super::tests::runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        assert!(first.normalized_source.contains("RuntimeValue::I64(2)"));
        assert!(first.normalized_source.contains("install_compiled_slots"));
        assert!(first.normalized_manifest.contains("engine_runtime"));
        assert_ne!(first.generation_digest(), changed.generation_digest());
        assert!(first
            .report
            .source_map
            .iter()
            .any(|s| s.source_path == "Rules/test.rule.ir.json"));
    }
    #[test]
    fn compiled_slots_reject_missing_leased_ir_and_undeclared_output() {
        let mut view = fixture(2);
        view.files.remove("Rules/test.rule.ir.json");
        assert!(generate(&view).is_err());
        let mut view = fixture(2);
        let mut declarations: serde_json::Value =
            serde_json::from_slice(view.bytes("Rules/rule-slots.json").unwrap()).unwrap();
        declarations["slots"][0]["outputFields"]["project.request"][0] = "other".into();
        view.files.insert(
            "Rules/rule-slots.json".into(),
            serde_json::to_vec(&declarations).unwrap(),
        );
        assert!(generate(&view).is_err());
    }
    #[test]
    fn projects_without_slots_keep_the_previous_dependency_surface() {
        let view = super::super::tests::source_view("pub fn project_game() {}", "{}");
        let glue = generate_runtime_glue(
            &view,
            Some(&super::super::tests::runtime_module()),
            TargetProfile::WindowsDev,
        )
        .unwrap()
        .unwrap();
        assert!(!glue.normalized_manifest.contains("engine_runtime"));
        assert!(!glue
            .normalized_source
            .contains("__AIFE_COMPILED_RULE_SLOTS__"));
        assert!(!glue.normalized_source.contains("install_compiled_slots"));
    }
}
