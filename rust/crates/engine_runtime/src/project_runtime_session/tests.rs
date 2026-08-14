use super::*;
use crate::components::Hierarchy;
use crate::math::Vec3;

const STATS_COMPONENT: &str = "sample.stats";

fn transform(x: f32) -> Transform {
    Transform {
        local_position: Vec3 { x, y: 0.0, z: 0.0 },
        local_rotation: Vec3::ZERO,
        local_scale: Vec3::ONE,
    }
}

fn world_with_entity(entity: &str, x: f32) -> World {
    let mut world = World::new();
    let entity_id = EntityId::from(entity);
    world
        .try_spawn_entity(
            entity_id.clone(),
            entity,
            "sample",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    world
        .try_insert_transform(entity_id.clone(), transform(x))
        .unwrap();
    world
        .try_insert_dynamic_component(
            entity_id,
            ComponentTypeId::from(STATS_COMPONENT),
            RuntimeValue::object([("score", RuntimeValue::I64(1))]),
        )
        .unwrap();
    world
}

fn score(world: &World, entity: &str) -> i64 {
    let ComponentValue::Dynamic {
        value: RuntimeValue::Object(fields),
        ..
    } = world
        .component_value(
            &EntityId::from(entity),
            &ComponentTypeId::from(STATS_COMPONENT),
        )
        .expect("stats component")
    else {
        panic!("expected dynamic stats component");
    };
    let RuntimeValue::I64(value) = fields.get("score").expect("score field") else {
        panic!("expected integer score");
    };
    *value
}

#[test]
fn project_runtime_mutation_invalid_component_replacement_leaves_world_unchanged() {
    let world = world_with_entity("entity-a", 1.0);
    let dirty_before = world.dirty_records().len();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.replace_component(
        EntityId::from("entity-a"),
        ComponentTypeId::from(STATS_COMPONENT),
        ComponentValue::Transform(transform(8.0)),
    );

    let error = mutations.prepare(&world).unwrap_err();

    assert_eq!(error.code, "project_runtime.mutation_preflight_failed");
    assert_eq!(score(&world, "entity-a"), 1);
    assert_eq!(world.dirty_records().len(), dirty_before);
}

#[test]
fn project_runtime_mutation_invalid_field_write_leaves_world_unchanged() {
    let world = world_with_entity("entity-a", 1.0);
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_component_field(
        EntityId::from("entity-a"),
        ComponentTypeId::from(STATS_COMPONENT),
        FieldPath::parse("missing.value").unwrap(),
        RuntimeValue::I64(9),
    );

    let error = mutations.prepare(&world).unwrap_err();

    assert_eq!(error.code, "project_runtime.mutation_preflight_failed");
    assert_eq!(score(&world, "entity-a"), 1);
}

#[test]
fn project_runtime_mutation_missing_transform_leaves_world_unchanged() {
    let mut world = World::new();
    world
        .try_spawn_entity(
            EntityId::from("entity-a"),
            "A",
            "sample",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_transform(EntityId::from("entity-a"), transform(4.0));

    let error = mutations.prepare(&world).unwrap_err();

    assert_eq!(error.code, "project_runtime.mutation_preflight_failed");
    assert!(world.transform(&EntityId::from("entity-a")).is_none());
}

#[test]
fn project_runtime_mutation_mixed_valid_invalid_batch_commits_nothing() {
    let world = world_with_entity("entity-a", 1.0);
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_transform(EntityId::from("entity-a"), transform(2.0));
    mutations.write_transform(EntityId::from("missing"), transform(3.0));

    let error = mutations.prepare(&world).unwrap_err();

    assert_eq!(error.operation_index, 1);
    assert_eq!(
        world
            .transform(&EntityId::from("entity-a"))
            .unwrap()
            .local_position
            .x,
        1.0
    );
    assert_eq!(error.report.staged_count, 2);
    assert_eq!(error.report.committed_count, 0);
    assert_eq!(error.report.rejected_count, 2);
}

#[test]
fn project_runtime_mutation_prepared_batch_preserves_operation_order() {
    let mut world = world_with_entity("entity-a", 1.0);
    let second = world_with_entity("entity-b", 2.0);
    let component = second
        .component_value(&EntityId::from("entity-b"), &ComponentTypeId::entity_meta())
        .unwrap();
    let ComponentValue::EntityMeta(meta) = component else {
        unreachable!();
    };
    world
        .try_spawn_entity(
            EntityId::from("entity-b"),
            meta.name,
            meta.kind,
            meta.enabled,
            meta.hierarchy,
        )
        .unwrap();
    world
        .try_insert_transform(EntityId::from("entity-b"), transform(2.0))
        .unwrap();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_transform(EntityId::from("entity-b"), transform(20.0));
    mutations.write_transform(EntityId::from("entity-a"), transform(10.0));

    let report = mutations
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();

    assert_eq!(report.records[0].entity_id, EntityId::from("entity-b"));
    assert_eq!(report.records[1].entity_id, EntityId::from("entity-a"));
}

#[test]
fn project_runtime_mutation_rejected_output_drops_batch() {
    let world = world_with_entity("entity-a", 1.0);
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_transform(EntityId::from("entity-a"), transform(8.0));
    let mut output = ProjectRuntimeSessionOutput::applied(mutations);
    output.status = ProjectRuntimeSessionStatus::Rejected;

    let preparation = output.prepare_mutations(&world).unwrap();

    let ProjectRuntimeMutationPreparation::Dropped(report) = preparation else {
        panic!("rejected output must drop mutations");
    };
    assert_eq!(report.staged_count, 1);
    assert_eq!(report.committed_count, 0);
    assert_eq!(
        world
            .transform(&EntityId::from("entity-a"))
            .unwrap()
            .local_position
            .x,
        1.0
    );
}

#[test]
fn animator2d_intent_batch_preserves_order_and_commits_atomically() {
    let world = world_with_entity("entity-a", 1.0);
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.animator2d_set_bool(EntityId::from("entity-a"), "moving", true);
    mutations.animator2d_set_trigger(EntityId::from("entity-a"), "attack");
    mutations.animator2d_reset_trigger(EntityId::from("entity-a"), "attack");

    let mut world = world;
    let report = mutations
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();

    assert_eq!(report.staged_count, 3);
    assert_eq!(report.committed_count, 3);
    assert!(matches!(
        &report.animator2d_commands[0],
        crate::animator2d::Animator2DCommand::SetBool { parameter_id, value: true, .. }
            if parameter_id == "moving"
    ));
    assert!(matches!(
        &report.animator2d_commands[1],
        crate::animator2d::Animator2DCommand::SetTrigger { parameter_id, .. }
            if parameter_id == "attack"
    ));
    assert!(matches!(
        &report.animator2d_commands[2],
        crate::animator2d::Animator2DCommand::ResetTrigger { parameter_id, .. }
            if parameter_id == "attack"
    ));
}

#[test]
fn animator2d_intent_is_not_exposed_when_world_mutation_preflight_fails() {
    let world = world_with_entity("entity-a", 1.0);
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.animator2d_set_trigger(EntityId::from("entity-a"), "attack");
    mutations.write_transform(EntityId::from("missing"), transform(2.0));

    let error = mutations.prepare(&world).unwrap_err();

    assert_eq!(error.code, "project_runtime.mutation_preflight_failed");
    assert_eq!(error.report.committed_count, 0);
    assert!(error.report.animator2d_commands.is_empty());
}

#[test]
fn project_runtime_mutation_structural_preflight_failure_does_not_activate_instances() {
    let world = world_with_entity("entity-a", 1.0);
    let entity_count_before = world.entity_count();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.push_gameplay_command(GameplayCommand::DespawnEntity {
        entity_id: EntityId::from("entity-a"),
    });

    let error = mutations.prepare(&world).unwrap_err();

    assert_eq!(error.code, "project_runtime.mutation_unsupported");
    assert_eq!(world.entity_count(), entity_count_before);
    assert!(world
        .component_value(&EntityId::from("entity-a"), &ComponentTypeId::entity_meta())
        .is_some());
}

#[test]
fn project_runtime_mutation_successful_batch_commits_writes_once() {
    let mut world = world_with_entity("entity-a", 1.0);
    let dirty_before = world.dirty_records().len();
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_component_field(
        EntityId::from("entity-a"),
        ComponentTypeId::from(STATS_COMPONENT),
        FieldPath::parse("score").unwrap(),
        RuntimeValue::I64(5),
    );
    mutations.write_transform(EntityId::from("entity-a"), transform(7.0));

    let report = mutations
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();

    assert_eq!(score(&world, "entity-a"), 5);
    assert_eq!(
        world
            .transform(&EntityId::from("entity-a"))
            .unwrap()
            .local_position
            .x,
        7.0
    );
    assert_eq!(report.committed_count, 2);
    assert_eq!(world.dirty_records().len(), dirty_before + 2);
}

#[test]
fn project_runtime_mutation_report_separates_staged_committed_rejected_counts() {
    let mut world = world_with_entity("entity-a", 1.0);
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_transform(EntityId::from("entity-a"), transform(3.0));

    let report = mutations
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();

    assert_eq!(report.staged_count, 1);
    assert_eq!(report.committed_count, 1);
    assert_eq!(report.rejected_count, 0);
    assert_eq!(report.records.len(), 1);
}

#[derive(Clone, Copy)]
enum ObservationBehavior {
    Missing,
    ExtraAndWrongType,
    Panic,
}

struct ObservationTestSession {
    behavior: ObservationBehavior,
}

impl ProjectRuntimeSession for ObservationTestSession {
    fn session_id(&self) -> &str {
        "test.observation.session"
    }

    fn handle_aui_actions(
        &mut self,
        _context: ProjectRuntimeSessionContext<'_>,
        batch: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        let mut output = ProjectRuntimeSessionOutput::no_op();
        output.status = ProjectRuntimeSessionStatus::Unhandled;
        output.unhandled_action_count = batch.len();
        output
    }

    fn fixed_update(
        &mut self,
        _context: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }

    fn observe(
        &self,
        _context: ProjectRuntimeObservationContext<'_>,
    ) -> ProjectRuntimeObservationOutput {
        match self.behavior {
            ObservationBehavior::Missing => ProjectRuntimeObservationOutput::empty(),
            ObservationBehavior::ExtraAndWrongType => ProjectRuntimeObservationOutput::empty()
                .with_value(
                    "test.value",
                    crate::project_observation::ProjectObservationValue::String(
                        "wrong".to_string(),
                    ),
                )
                .with_value(
                    "test.extra",
                    crate::project_observation::ProjectObservationValue::Bool(true),
                ),
            ObservationBehavior::Panic => panic!("observation panic"),
        }
    }
}

fn cooked_observation_contract() -> crate::project_observation::CookedProjectObservationContract {
    crate::project_observation::ProjectObservationContract {
        schema_version: crate::project_observation::PROJECT_OBSERVATION_CONTRACT_SCHEMA_VERSION
            .to_string(),
        contract_id: "test.runtime-observations".to_string(),
        observations: vec![crate::project_observation::ProjectObservationEntry {
            path: "test.value".to_string(),
            value_type: crate::project_observation::ProjectObservationType::Integer,
            description: "Test integer".to_string(),
            allowed_values: None,
        }],
    }
    .cook()
    .unwrap()
}

#[test]
fn project_runtime_session_observation_missing_extra_and_type_mismatch_fail_closed() {
    let world = World::new();
    let contract = cooked_observation_contract();
    let time = crate::runtime_time::RuntimeTime::new().context();
    let missing = ObservationTestSession {
        behavior: ObservationBehavior::Missing,
    };

    let missing_state = execute_project_runtime_observation(
        &missing,
        1,
        time,
        &world,
        &contract,
        ProjectRuntimeSessionReportLevel::Summary,
    );

    let ProjectRuntimeObservationState::ContractViolated { diagnostics, .. } = missing_state else {
        panic!("missing value must violate the contract");
    };
    assert_eq!(diagnostics[0].code, "project_observation.value_missing");

    let invalid = ObservationTestSession {
        behavior: ObservationBehavior::ExtraAndWrongType,
    };
    let invalid_state = execute_project_runtime_observation(
        &invalid,
        2,
        time,
        &world,
        &contract,
        ProjectRuntimeSessionReportLevel::Summary,
    );
    let ProjectRuntimeObservationState::ContractViolated { diagnostics, .. } = invalid_state else {
        panic!("extra and wrong type values must violate the contract");
    };
    let codes = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"project_observation.value_undeclared"));
    assert!(codes.contains(&"project_observation.value_type_mismatch"));
}

#[test]
fn project_runtime_session_observation_panic_fails_closed_without_snapshot() {
    let world = World::new();
    let contract = cooked_observation_contract();
    let session = ObservationTestSession {
        behavior: ObservationBehavior::Panic,
    };

    let state = execute_project_runtime_observation(
        &session,
        7,
        crate::runtime_time::RuntimeTime::new().context(),
        &world,
        &contract,
        ProjectRuntimeSessionReportLevel::Trace,
    );

    let ProjectRuntimeObservationState::ContractViolated {
        runtime_frame,
        diagnostics,
        ..
    } = state
    else {
        panic!("panic must not publish a snapshot");
    };
    assert_eq!(runtime_frame, 7);
    assert_eq!(diagnostics[0].code, "project_observation.observe_panicked");
}
