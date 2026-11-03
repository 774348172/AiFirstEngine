#[cfg(test)]
mod tests {
    use crate::components::{ComponentTypeId, Hierarchy, Transform};
    use crate::field_path::FieldPath;
    use crate::ids::EntityId;
    use crate::logic_executor::{ExecutorKind, LogicContext, LogicResult};
    use crate::math::Vec3;
    use crate::project_logic::ProjectLogicRunner;
    use crate::rule_artifact::expected_rule_artifact_id;
    use crate::rule_compiler::{RuleCompileRequest, RuleCompileStatus, RuleCompiler};
    use crate::rule_ir::{ProjectRuleIr, ProjectRulePhase, RuleOperation, RuleRuntimeValue};
    use crate::rule_registry::RuleModuleRegistry;
    use crate::runtime_package::{
        RuntimeRuleExecutor, RuntimeRuleManifest, RuntimeRuleManifestEntry, RuntimeRuleModuleEntry,
        RuntimeRuleModuleKind, RuntimeRulePhase, RUNTIME_RULE_MANIFEST_MODE,
        RUNTIME_RULE_MANIFEST_SCHEMA_VERSION,
    };
    use crate::runtime_trace::RuntimeTrace;
    use crate::world::World;

    fn hierarchy() -> Hierarchy {
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        }
    }

    fn world_with_entity(entity_id: &str) -> World {
        let mut world = World::new();
        let entity_id = EntityId::from(entity_id);
        world.spawn_entity(entity_id.clone(), "Entity", "actor", true, hierarchy());
        world.insert_transform(entity_id, Transform::identity());
        world.take_dirty_records();
        world
    }

    fn manifest(rule_id: &str, ir_hash: &str) -> RuntimeRuleManifest {
        let artifact_id = expected_rule_artifact_id(rule_id, ir_hash);
        RuntimeRuleManifest {
            schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.to_string(),
            mode: RUNTIME_RULE_MANIFEST_MODE.to_string(),
            rules: vec![RuntimeRuleManifestEntry {
                rule_id: rule_id.to_string(),
                phase: RuntimeRulePhase::Update,
                enabled: true,
                executor: RuntimeRuleExecutor::RustAot,
                ir_source: Some(format!("Rules/{}.rule.ir.json", rule_id)),
                ir_hash: Some(ir_hash.to_string()),
                artifact_id: Some(artifact_id.clone()),
                source_map: None,
            }],
            modules: vec![RuntimeRuleModuleEntry {
                artifact_id,
                module_kind: RuntimeRuleModuleKind::StaticRegistry,
                path: None,
            }],
        }
    }

    fn generated_move_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let mut result = LogicResult::applied("project.rule.move", ExecutorKind::RustAot);
        let field_path = FieldPath::parse("local_position.x").expect("field path should parse");
        match context.write_component_field(
            EntityId::from("entity-a"),
            ComponentTypeId::transform(),
            &field_path,
            crate::component_value::RuntimeValue::F64(4.0),
        ) {
            Ok(write) => result.writes.push(write),
            Err(error) => {
                result.status = crate::logic_executor::LogicStatus::Failed;
                result.errors.push(error.into());
            }
        }
        result
    }

    fn generated_spawn_rule(context: &mut LogicContext<'_>) -> LogicResult {
        let result = LogicResult::applied("project.rule.spawn", ExecutorKind::RustAot);
        context.enqueue_command(crate::gameplay_command::GameplayCommand::SpawnEntity {
            entity_id: EntityId::from("entity-spawned"),
            name: "Spawned".to_string(),
            kind: "actor".to_string(),
            enabled: true,
            parent_id: None,
            components: Vec::new(),
        });
        result
    }

    #[test]
    fn m2_rule_demo_a_ir_codegen_registry_runner_writes_transform() {
        let mut ir = ProjectRuleIr::new("project.rule.move", ProjectRulePhase::Update);
        ir.operations.push(RuleOperation::WriteComponentField {
            entity_id: "entity-a".to_string(),
            component_type: "engine.transform".to_string(),
            field_path: "local_position.x".to_string(),
            value: RuleRuntimeValue::F64 { value: 4.0 },
        });
        let compile_report = RuleCompiler::compile(
            &RuleCompileRequest::dev_desktop("target/generated-rules"),
            &ir,
            None,
        );
        assert_eq!(compile_report.status, RuleCompileStatus::Success);
        assert!(compile_report
            .generated_source
            .as_deref()
            .unwrap_or_default()
            .contains("write_component_field"));

        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule("project.rule.move", generated_move_rule);
        let runner = ProjectLogicRunner::from_rule_manifest_and_registry(
            &manifest("project.rule.move", &compile_report.ir_hash),
            &registry,
        )
        .expect("runner should build");
        let mut world = world_with_entity("entity-a");
        let mut trace = RuntimeTrace::new();

        let results = runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(results.len(), 1);
        assert_eq!(
            world
                .transform(&EntityId::from("entity-a"))
                .expect("transform should exist")
                .local_position
                .x,
            4.0
        );
        assert_eq!(results[0].writes[0].field, "local_position.x");
    }

    #[test]
    fn m2_rule_demo_b_spawn_entity_through_command_buffer() {
        let mut ir = ProjectRuleIr::new("project.rule.spawn", ProjectRulePhase::Update);
        ir.operations.push(RuleOperation::SpawnEntity {
            entity_id: "entity-spawned".to_string(),
            name: "Spawned".to_string(),
            kind: "actor".to_string(),
            components: Vec::new(),
        });
        let compile_report = RuleCompiler::compile(
            &RuleCompileRequest::dev_desktop("target/generated-rules"),
            &ir,
            None,
        );
        assert_eq!(compile_report.status, RuleCompileStatus::Success);
        assert!(compile_report
            .generated_source
            .as_deref()
            .unwrap_or_default()
            .contains("GameplayCommand::SpawnEntity"));

        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule("project.rule.spawn", generated_spawn_rule);
        let runner = ProjectLogicRunner::from_rule_manifest_and_registry(
            &manifest("project.rule.spawn", &compile_report.ir_hash),
            &registry,
        )
        .expect("runner should build");
        let mut world = World::new();
        let mut trace = RuntimeTrace::new();

        let results = runner.run_frame_update(1, &mut world, &mut trace);

        assert_eq!(results[0].command_ids.len(), 1);
        assert!(world.entity(&EntityId::from("entity-spawned")).is_some());
    }

    #[test]
    fn m2_rule_demo_c_invalid_ir_reports_diagnostics() {
        let mut ir = ProjectRuleIr::new("project.rule.invalid", ProjectRulePhase::Update);
        ir.operations.push(RuleOperation::WriteComponentField {
            entity_id: "entity-a".to_string(),
            component_type: "engine.transform".to_string(),
            field_path: "local_position[0]".to_string(),
            value: RuleRuntimeValue::F64 { value: 1.0 },
        });

        let compile_report = RuleCompiler::compile(
            &RuleCompileRequest::dev_desktop("target/generated-rules"),
            &ir,
            None,
        );

        assert_eq!(compile_report.status, RuleCompileStatus::Failed);
        assert_eq!(compile_report.diagnostics[0].code, "InvalidFieldPath");
        assert!(compile_report.artifact_id.is_none());
        assert!(compile_report.generated_source.is_none());
    }

    #[test]
    fn m2_rule_demo_missing_registry_entry_is_reported_before_runtime() {
        let ir = ProjectRuleIr::new("project.rule.missing", ProjectRulePhase::Update);
        let ir_hash = ir.stable_hash();
        let registry = RuleModuleRegistry::new();

        let error = ProjectLogicRunner::from_rule_manifest_and_registry(
            &manifest("project.rule.missing", &ir_hash),
            &registry,
        )
        .expect_err("missing registry should fail");

        assert_eq!(error.code, "missing_registered_rule");
    }

    #[test]
    fn m2_rule_demo_move_rule_does_not_change_other_axes() {
        let mut registry = RuleModuleRegistry::new();
        registry.register_generated_rule("project.rule.move", generated_move_rule);
        let ir = ProjectRuleIr::new("project.rule.move", ProjectRulePhase::Update);
        let runner = ProjectLogicRunner::from_rule_manifest_and_registry(
            &manifest("project.rule.move", &ir.stable_hash()),
            &registry,
        )
        .expect("runner should build");
        let mut world = world_with_entity("entity-a");
        let mut trace = RuntimeTrace::new();

        runner.run_frame_update(1, &mut world, &mut trace);
        let position = world
            .transform(&EntityId::from("entity-a"))
            .expect("transform should exist")
            .local_position;

        assert_eq!(
            position,
            Vec3 {
                x: 4.0,
                y: 0.0,
                z: 0.0
            }
        );
    }
}
