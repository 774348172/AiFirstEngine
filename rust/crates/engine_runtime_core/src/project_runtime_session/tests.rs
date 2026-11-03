use super::*;
use crate::components::Hierarchy;
use crate::math::Vec3;

const STATS_COMPONENT: &str = "sample.stats";

#[test]
fn immediate_spatial_replacement_work_is_single_validation_without_row_migration() {
    use crate::spatial_contract::{
        decode_runtime_scene, RuntimeEntityDocument, RuntimeSceneDocument,
    };
    use crate::world::mutation_work::{ROW_REMOVALS, VALIDATED_WRITES};
    let mut observed = Vec::new();
    for size in [1, 16, 128, 1024] {
        let RuntimeSceneDocument::Spatial(mut scene) = decode_runtime_scene(include_bytes!(
            "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
        ))
        .unwrap() else {
            panic!("spatial fixture")
        };
        let template = scene.entities[0].clone();
        let camera = scene.entities[1].clone();
        scene.entities = (0..size + 3)
            .map(|i| {
                let mut e = template.clone();
                if let RuntimeEntityDocument::Spatial(ref mut value) = e {
                    value.id = format!("node-{i}");
                    let crate::spatial_contract::RuntimeSpatial::World3D { mesh_renderer, .. } =
                        &mut value.spatial;
                    *mesh_renderer = None;
                }
                e
            })
            .collect();
        scene.entities.push(camera);
        let mut world = crate::scene_loader::load_scene_into_world(&scene.into())
            .value
            .unwrap();
        world.take_dirty_records();
        let mut public_world = world.clone();
        let make = |world: &World| {
            let mut b = ProjectRuntimeMutationBuffer::new();
            for i in 0..size {
                let id: EntityId = format!("node-{i}").into();
                b.spatial_command(
                    id.clone(),
                    world.entity_generation(&id).unwrap(),
                    project_runtime_sdk::ProjectRuntimeSpatialIntent::SetLocalTransform {
                        transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                            translation: [i as f32, 2.0, 3.0],
                            ..project_runtime_sdk::ProjectRuntimeTransform3D::identity()
                        },
                    },
                );
            }
            b
        };
        let locations: Vec<_> = (0..size + 3)
            .map(|i| world.entity_location(&format!("node-{i}").into()).unwrap())
            .collect();
        VALIDATED_WRITES.with(|n| n.set(0));
        ROW_REMOVALS.with(|n| n.set(0));
        let (report, _) = make(&world).commit_with_runtime(&mut world, None).unwrap();
        assert_eq!(report.committed_count, size);
        let host_work = (
            VALIDATED_WRITES.with(|n| n.get()),
            ROW_REMOVALS.with(|n| n.get()),
        );
        VALIDATED_WRITES.with(|n| n.set(0));
        ROW_REMOVALS.with(|n| n.set(0));
        let prepared = make(&public_world).prepare(&public_world).unwrap();
        let public = prepared.commit(&mut public_world).unwrap();
        assert_eq!(public.records.len(), size);
        let public_work = (
            VALIDATED_WRITES.with(|n| n.get()),
            ROW_REMOVALS.with(|n| n.get()),
        );
        for i in 0..size + 3 {
            let id: EntityId = format!("node-{i}").into();
            assert_eq!(world.transform3d(&id), public_world.transform3d(&id));
        }
        let stable_locations = (0..size + 3)
            .all(|i| world.entity_location(&format!("node-{i}").into()).unwrap() == locations[i]);
        observed.push((size, host_work, public_work, stable_locations));
    }
    eprintln!("actual validation/row removals: {observed:?}");
    assert!(
        observed
            .iter()
            .all(|(n, h, p, stable)| *h == (*n, 0) && *p == (2 * n, 0) && *stable),
        "actual validation/row removals: {observed:?}"
    );
}

#[test]
fn immediate_host_keeps_structural_overlay_history_and_final_camera_rejection_atomic() {
    use crate::spatial_contract::{decode_runtime_scene, RuntimeSceneDocument};
    use project_runtime_sdk::{
        ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D as Trs,
    };
    let scene = decode_runtime_scene(include_bytes!(
        "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
    ))
    .unwrap();
    let RuntimeSceneDocument::Spatial(scene) = scene else {
        panic!("spatial fixture")
    };
    let mut host = crate::scene_loader::load_scene_into_world(&scene.into())
        .value
        .unwrap();
    host.take_dirty_records();
    host.begin_spatial_tick(1);
    let mut public = host.clone();
    let write = |b: &mut ProjectRuntimeMutationBuffer, w: &World, id: &str, intent: Intent| {
        b.spatial_command(id.into(), w.entity_generation(&id.into()).unwrap(), intent);
    };
    // Applying an ordinary write plus keep-world anchor uses the same freshly
    // evaluated structural plan. Public prepared batches still recompute theirs.
    let make = |w: &World| {
        let mut b = ProjectRuntimeMutationBuffer::new();
        write(
            &mut b,
            w,
            "cube",
            Intent::SetLocalTransform {
                transform: Trs {
                    translation: [3.0, 2.0, 1.0],
                    ..Trs::identity()
                },
            },
        );
        write(
            &mut b,
            w,
            "cube",
            Intent::SetWorldAnchor {
                anchor: [10000.0, 0.0, 0.0],
                preserve_world: true,
            },
        );
        write(
            &mut b,
            w,
            "cube",
            Intent::SetInterpolation { enabled: false },
        );
        b
    };
    let (report, _) = make(&host).commit_with_runtime(&mut host, None).unwrap();
    let public_report = make(&public)
        .prepare(&public)
        .unwrap()
        .commit(&mut public)
        .unwrap();
    assert_eq!(report.committed_count, 3);
    assert_eq!(public_report.committed_count, 3);
    assert!(report.records.is_empty());
    assert_eq!(host.finish_spatial_tick(), public.finish_spatial_tick());
    assert_eq!(
        host.committed_spatial_pose(&"cube".into()),
        public.committed_spatial_pose(&"cube".into())
    );
    assert_eq!(
        host.prepare_spatial_presentation(1, 0.5).unwrap(),
        public.prepare_spatial_presentation(1, 0.5).unwrap()
    );
    let dirty = host.dirty_records().to_vec();
    let pose = host.committed_spatial_pose(&"cube".into());
    // A valid tail shape is not enough: the final camera/ancestor overlay must
    // still reject nonunit scale without publishing any preceding value.
    let mut bad = ProjectRuntimeMutationBuffer::new();
    write(
        &mut bad,
        &host,
        "cube",
        Intent::SetLocalTransform {
            transform: Trs {
                scale: [2.0, 1.0, 1.0],
                ..Trs::identity()
            },
        },
    );
    write(
        &mut bad,
        &host,
        "camera",
        Intent::Reparent {
            parent_entity_id: Some("cube".into()),
            parent_generation: host.entity_generation(&"cube".into()).unwrap(),
            preserve_world: false,
        },
    );
    let error = bad.commit_with_runtime(&mut host, None).unwrap_err();
    assert_eq!(error.code, "project_runtime.mutation_preflight_failed");
    assert!(
        error.message.starts_with("three_d.camera_scale:"),
        "{error:?}"
    );
    assert_eq!(error.report.committed_count, 0);
    assert_eq!(host.committed_spatial_pose(&"cube".into()), pose);
    assert_eq!(host.dirty_records(), dirty);
    assert!(host
        .hierarchy(&"camera".into())
        .unwrap()
        .parent_id
        .is_none());
}

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
fn host_commit_avoids_discarded_records_but_public_commit_preserves_ordered_history() {
    for size in [1, 16, 128, 1024] {
        let make_batch = || {
            let mut batch = ProjectRuntimeMutationBuffer::new();
            for i in 0..size {
                batch.write_transform(EntityId::from("entity-a"), transform(2.0 + i as f32));
            }
            batch
        };
        let mut host_world = world_with_entity("entity-a", 1.0);
        mutation_record_work::RECORDS.with(|count| count.set(0));
        let (host, receipts) = make_batch()
            .commit_with_runtime(&mut host_world, None)
            .unwrap();
        assert_eq!(host.committed_count, size);
        assert!(receipts.is_none());
        let actual_work = mutation_record_work::RECORDS.with(|count| count.get());
        assert_eq!(
            actual_work, 0,
            "host discarded {actual_work} generated records at size {size}"
        );
        assert!(host.records.is_empty());

        let mut public_world = world_with_entity("entity-a", 1.0);
        mutation_record_work::RECORDS.with(|count| count.set(0));
        let public = make_batch()
            .prepare(&public_world)
            .unwrap()
            .commit(&mut public_world)
            .unwrap();
        assert_eq!(
            mutation_record_work::RECORDS.with(|count| count.get()),
            size
        );
        assert_eq!(public.records.len(), size);
        for pair in public.records.windows(2) {
            assert_eq!(pair[0].after, pair[1].before);
        }
        assert_eq!(
            host_world.transform(&EntityId::from("entity-a")),
            public_world.transform(&EntityId::from("entity-a"))
        );
    }
}

#[test]
fn host_commit_preserves_atomic_rejection_and_prepared_generation_check() {
    let mut world = world_with_entity("entity-a", 1.0);
    let mut invalid = ProjectRuntimeMutationBuffer::new();
    invalid.write_transform("entity-a".into(), transform(8.0));
    invalid.write_transform("missing".into(), transform(9.0));
    let dirty = world.dirty_records().len();
    assert_eq!(
        invalid
            .commit_with_runtime(&mut world, None)
            .unwrap_err()
            .report
            .committed_count,
        0
    );
    assert_eq!(world.dirty_records().len(), dirty);
    assert_eq!(
        world
            .transform(&"entity-a".into())
            .unwrap()
            .local_position
            .x,
        1.0
    );

    let mut batch = ProjectRuntimeMutationBuffer::new();
    batch.write_transform("entity-a".into(), transform(99.0));
    let prepared = batch.prepare(&world).unwrap();
    world.despawn_entity(&"entity-a".into()).unwrap();
    let error = prepared.commit_for_host(&mut world).unwrap_err();
    assert_eq!(error.code, "three_d.stale_entity");
    assert_eq!(error.report.committed_count, 0);
    assert!(error.report.records.is_empty());
    assert!(world.transform(&"entity-a".into()).is_none());
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

#[test]
fn project_runtime_prepared_batch_does_not_resurrect_a_removed_component() {
    let mut world = world_with_entity("entity-a", 1.0);
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    mutations.write_transform("entity-a".into(), transform(9.0));
    mutations.write_component_field(
        "entity-a".into(),
        ComponentTypeId::from(STATS_COMPONENT),
        FieldPath::parse("score").unwrap(),
        RuntimeValue::I64(8),
    );
    let prepared = mutations.prepare(&world).unwrap();
    world
        .try_remove_component_value(&"entity-a".into(), &ComponentTypeId::from(STATS_COMPONENT))
        .unwrap();
    let dirty = world.dirty_records().len();
    let error = prepared.commit(&mut world).unwrap_err();
    assert_eq!(error.report.committed_count, 0);
    assert_eq!(
        world
            .transform(&"entity-a".into())
            .unwrap()
            .local_position
            .x,
        1.0
    );
    assert!(world
        .component_value(&"entity-a".into(), &ComponentTypeId::from(STATS_COMPONENT))
        .is_none());
    assert_eq!(world.dirty_records().len(), dirty);
}

#[test]
fn spatial_bulk_pure_host_evaluates_each_root_once_at_multiple_scales() {
    use crate::spatial_contract::{
        decode_runtime_scene, RuntimeEntityDocument, RuntimeSceneDocument, RuntimeSpatial,
    };
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut observed = Vec::new();
    for size in [1, 100, 1000, 10000] {
        let RuntimeSceneDocument::Spatial(mut scene) = decode_runtime_scene(include_bytes!(
            "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
        ))
        .unwrap() else {
            panic!("spatial")
        };
        let template = scene.entities[0].clone();
        let camera = scene.entities[1].clone();
        scene.entities = (0..size)
            .map(|i| {
                let mut e = template.clone();
                if let RuntimeEntityDocument::Spatial(ref mut v) = e {
                    v.id = format!("node-{i}");
                    let RuntimeSpatial::World3D { mesh_renderer, .. } = &mut v.spatial;
                    *mesh_renderer = None;
                }
                e
            })
            .collect();
        scene.entities.push(camera);
        let mut world = crate::scene_loader::load_scene_into_world(&scene.into())
            .value
            .unwrap();
        world.take_dirty_records();
        world.begin_spatial_tick(1);
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        for i in 0..size {
            buffer.spatial_local_transform(
                world
                    .spatial_entity_key(&format!("node-{i}").into())
                    .unwrap(),
                Trs {
                    translation: [i as f32, 2., 3.],
                    ..Trs::identity()
                },
            );
        }
        crate::spatial_contract::CANONICAL_VALIDATIONS.with(|n| n.set(0));
        World::spatial_parent_product_work(true);
        World::spatial_evaluation_work(true);
        World::spatial_affine_linear_work(true);
        World::spatial_publication_lookup_work(true);
        crate::world::mutation_work::SOURCE_LOOKUPS.with(|n| n.set(0));
        let (report, _) = buffer.commit_with_runtime(&mut world, None).unwrap();
        assert_eq!(World::spatial_affine_linear_work(false), 1,
            "equal linear matrices in one exclusive root batch must share cofactor work at {size}; every translation still needs its own inverse check");
        assert_eq!(
            World::spatial_publication_lookup_work(false),
            size,
            "one source lookup must serve both derived World state and history publication"
        );
        assert_eq!(
            World::spatial_parent_product_work(false),
            0,
            "root evaluation must not multiply its local matrix by identity at {size}"
        );
        let checks = crate::spatial_contract::CANONICAL_VALIDATIONS.with(|n| n.get());
        assert_eq!(
            checks, 1,
            "identical rotation/scale need one successful local linear validation per exclusive commit; translation and final affine checks remain per row"
        );
        let dirty = world.take_dirty_records();
        let calls = World::spatial_evaluation_work(false);
        let lookups = crate::world::mutation_work::SOURCE_LOOKUPS.with(|n| n.get());
        assert_eq!(report.committed_count, size);
        for i in 0..size {
            assert_eq!(
                world
                    .transform3d(&format!("node-{i}").into())
                    .unwrap()
                    .translation,
                [i as f32, 2., 3.]
            );
        }
        crate::world::mutation_work::SOURCE_LOOKUPS.with(|n| n.set(0));
        let mut extract = crate::render_extract::RenderExtractContext::new();
        let scene = crate::render_state::RenderSceneState::new();
        let _ = extract.extract_dirty_records(1, &world, &scene, dirty);
        let extract_lookups = crate::world::mutation_work::SOURCE_LOOKUPS.with(|n| n.get());
        assert_eq!(
            extract_lookups, 0,
            "a current all-spatial empty projection should not resolve individual roots"
        );
        for tick in 2..7 {
            World::spatial_overlay_work(true);
            World::spatial_sort_work(true);
            World::spatial_pose_allocation_work(true);
            world.begin_spatial_tick(tick);
            let mut b = ProjectRuntimeMutationBuffer::new();
            for i in 0..size {
                b.spatial_local_transform(
                    world
                        .spatial_entity_key(&format!("node-{i}").into())
                        .unwrap(),
                    Trs {
                        translation: [i as f32, tick as f32, 3.],
                        ..Trs::identity()
                    },
                );
            }
            b.commit_with_runtime(&mut world, None).unwrap();
            assert_eq!(
                World::spatial_overlay_work(false),
                0,
                "a complete matching unique input must not rebuild a deduplication map at {size}"
            );
            assert_eq!(World::spatial_sort_work(false), 0,
                "unchanged current handle order should reuse only its source-order permutation at {size}");
            world.take_dirty_records();
            if tick >= 4 {
                assert_eq!(
                    World::spatial_pose_allocation_work(false),
                    0,
                    "warm bulk publication allocates at scale {size}"
                );
            }
        }
        observed.push((size, calls, lookups));
    }
    eprintln!("actual evaluated spatial nodes: {observed:?}");
    assert!(
        observed
            .iter()
            .all(|(n, c, lookups)| n == c && *lookups == 0),
        "{observed:?}"
    );
}

fn spatial_bulk_world() -> World {
    let scene = crate::spatial_contract::decode_runtime_scene(include_bytes!(
        "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
    ))
    .unwrap();
    let crate::spatial_contract::RuntimeSceneDocument::Spatial(scene) = scene else {
        panic!("spatial fixture")
    };
    let mut world = crate::scene_loader::load_scene_into_world(&scene.into())
        .value
        .unwrap();
    world.take_dirty_records();
    world.begin_spatial_tick(1);
    world
}

#[test]
fn spatial_bulk_buffer_keeps_pure_rows_compact_and_normalizes_mixed_order_once() {
    use project_runtime_sdk::{spatial_bulk::SpatialEntityKey, ProjectRuntimeTransform3D as Trs};
    for size in [1usize, 100, 1000, 10000] {
        EXPANDED_SPATIAL_OPERATIONS.with(|n| n.set(0));
        let mut b = ProjectRuntimeMutationBuffer::new();
        for index in 0..size {
            b.spatial_local_transform(
                SpatialEntityKey {
                    world_epoch: 1,
                    index: index as u32,
                    generation: 0,
                },
                Trs::identity(),
            );
        }
        assert_eq!(b.len(), size);
        assert!(!b.is_empty());
        assert_eq!(
            EXPANDED_SPATIAL_OPERATIONS.with(|n| n.get()),
            0,
            "pure rows expanded into large mixed-operation enums at scale {size}"
        );
        let retained = b.clone();
        b.write_transform("legacy".into(), transform(2.));
        b.spatial_local_transform(
            SpatialEntityKey {
                world_epoch: 1,
                index: 0,
                generation: 0,
            },
            Trs::identity(),
        );
        assert_eq!(b.len(), size + 2);
        assert_eq!(retained.len(), size);
        assert!(matches!(
            b.operations[size],
            ProjectRuntimeMutationOperation::WriteTransform { .. }
        ));
        assert!(matches!(
            b.operations[size + 1],
            ProjectRuntimeMutationOperation::SpatialLocalTransform { .. }
        ));
        assert!(b.operations[..size].iter().all(|op| matches!(
            op,
            ProjectRuntimeMutationOperation::SpatialLocalTransform { .. }
        )));
    }
}

#[test]
fn spatial_bulk_leaf_presentation_does_not_build_unused_ancestor_cache_at_scale() {
    use crate::spatial_contract::{
        decode_runtime_scene, RuntimeEntityDocument, RuntimeSceneDocument, RuntimeSpatial,
    };
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut observed = Vec::new();
    for size in [1, 100, 1000, 10000] {
        let RuntimeSceneDocument::Spatial(mut scene) = decode_runtime_scene(include_bytes!(
            "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
        ))
        .unwrap() else {
            panic!("spatial fixture")
        };
        let template = scene.entities[0].clone();
        scene.entities = (0..size)
            .map(|i| {
                let mut e = template.clone();
                if let RuntimeEntityDocument::Spatial(ref mut e) = e {
                    e.id = format!("leaf-{i}");
                    let RuntimeSpatial::World3D { mesh_renderer, .. } = &mut e.spatial;
                    *mesh_renderer = None;
                }
                e
            })
            .collect();
        scene.main_camera = None;
        let mut w = crate::scene_loader::load_scene_into_world(&scene.into())
            .value
            .unwrap();
        w.prepare_spatial_presentation(0, 1.).unwrap();
        w.take_dirty_records();
        w.begin_spatial_tick(1);
        let old = w.committed_spatial_pose(&"leaf-0".into()).unwrap();
        let mut b = ProjectRuntimeMutationBuffer::new();
        for i in 0..size {
            b.spatial_local_transform(
                w.spatial_entity_key(&format!("leaf-{i}").into()).unwrap(),
                Trs {
                    translation: [4., 0., 0.],
                    ..Trs::identity()
                },
            );
        }
        b.commit_with_runtime(&mut w, None).unwrap();
        World::presentation_cache_work(true);
        for (frame, alpha) in [(1, 0.25), (2, 0.75)] {
            World::spatial_parent_product_work(true);
            let delta = w.prepare_spatial_presentation(frame, alpha).unwrap();
            assert_eq!(
                World::spatial_parent_product_work(false),
                0,
                "root presentation must not compose an identity parent"
            );
            assert_eq!(delta.changed.len(), size);
            assert!(delta.changed.windows(2).all(|a| a[0] < a[1]));
            assert_eq!(
                w.spatial_presentation(&"leaf-0".into())
                    .unwrap()
                    .pose
                    .position,
                [4. * alpha, 0., 0.]
            );
            assert_eq!(old.state.pose.position, [0.; 3]);
        }
        observed.push((size, World::presentation_cache_work(false)));
    }
    eprintln!("actual ancestor-cache inserts: {observed:?}");
    assert!(observed.iter().all(|(_, n)| *n == 0), "{observed:?}");
}

#[test]
fn spatial_bulk_order_reuse_rechecks_inputs_and_never_publishes_failed_permutations() {
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut world = spatial_bulk_world();
    let cube = world.spatial_entity_key(&"cube".into()).unwrap();
    let camera = world.spatial_entity_key(&"camera".into()).unwrap();
    let apply = |world: &mut World, keys: &[_], x: f32, invalid_tail: bool| {
        let mut b = ProjectRuntimeMutationBuffer::new();
        for &key in keys {
            b.spatial_local_transform(
                key,
                Trs {
                    translation: [x, 2., 3.],
                    ..Trs::identity()
                },
            );
        }
        if invalid_tail {
            b.spatial_local_transform(
                cube,
                Trs {
                    scale: [0.; 3],
                    ..Trs::identity()
                },
            );
        }
        World::spatial_sort_work(true);
        let result = b.commit_with_runtime(world, None);
        (result, World::spatial_sort_work(false))
    };
    assert!(apply(&mut world, &[cube, camera], 2., false).0.is_ok());
    let retained = world.committed_spatial_pose(&"cube".into()).unwrap();
    let (ok, work) = apply(&mut world, &[cube, camera], 3., false);
    assert!(ok.is_ok());
    assert_eq!(work, 0);
    let old = world.transform3d(&"cube".into()).unwrap().clone();
    assert!(apply(&mut world, &[camera, cube], 4., true).0.is_err());
    assert_eq!(world.transform3d(&"cube".into()), Some(&old));
    let (ok, work) = apply(&mut world, &[cube, camera], 5., false);
    assert!(ok.is_ok());
    assert_eq!(
        work, 0,
        "failed reversed input must not replace the published permutation"
    );
    let (ok, work) = apply(&mut world, &[camera, cube], 6., false);
    assert!(ok.is_ok());
    assert!(
        work > 0,
        "changed complete input sequence must reorder current sources"
    );
    assert_eq!(apply(&mut world, &[camera, cube], 7., false).1, 0);
    assert!(apply(&mut world, &[cube], 8., false).0.is_ok());
    assert!(apply(&mut world, &[camera, cube], 9., false).1 > 0);
    let mut clone = world.clone();
    assert_eq!(apply(&mut clone, &[camera, cube], 10., false).1, 0);
    assert_eq!(
        world.transform3d(&"cube".into()).unwrap().translation[0],
        9.
    );
    assert_eq!(retained.local.translation[0], 2.);
    world.despawn_entity(&"cube".into()).unwrap();
    assert!(
        apply(&mut world, &[camera, cube], 11., false).0.is_err(),
        "cached permutation is not a live-key proof"
    );
}

#[test]
fn spatial_bulk_unique_sequence_is_not_a_tail_validation_or_duplicate_proof() {
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut w = spatial_bulk_world();
    let cube = w.spatial_entity_key(&"cube".into()).unwrap();
    let camera = w.spatial_entity_key(&"camera".into()).unwrap();
    let mut first = ProjectRuntimeMutationBuffer::new();
    first.spatial_local_transform(cube, Trs::identity());
    first.spatial_local_transform(camera, Trs::identity());
    first.commit_with_runtime(&mut w, None).unwrap();
    let before_cube = w.transform3d(&"cube".into()).unwrap().clone();
    let before_camera = w.transform3d(&"camera".into()).unwrap().clone();
    let old = w.committed_spatial_pose(&"cube".into()).unwrap();
    let mut bad = ProjectRuntimeMutationBuffer::new();
    bad.spatial_local_transform(
        cube,
        Trs {
            translation: [5.; 3],
            ..Trs::identity()
        },
    );
    bad.spatial_local_transform(
        camera,
        Trs {
            scale: [0.; 3],
            ..Trs::identity()
        },
    );
    World::spatial_overlay_work(true);
    assert_eq!(
        bad.commit_with_runtime(&mut w, None)
            .unwrap_err()
            .report
            .committed_count,
        0
    );
    assert_eq!(World::spatial_overlay_work(false), 0);
    assert_eq!(w.transform3d(&"cube".into()), Some(&before_cube));
    assert_eq!(w.transform3d(&"camera".into()), Some(&before_camera));
    // Same length, different IDs: the duplicate must use the original overlay.
    let mut duplicate = ProjectRuntimeMutationBuffer::new();
    for x in [7., 9.] {
        duplicate.spatial_local_transform(
            cube,
            Trs {
                translation: [x, 0., 0.],
                ..Trs::identity()
            },
        );
    }
    World::spatial_overlay_work(true);
    duplicate.commit_with_runtime(&mut w, None).unwrap();
    assert_eq!(World::spatial_overlay_work(false), 2);
    assert_eq!(w.transform3d(&"cube".into()).unwrap().translation[0], 9.);
    assert_eq!(w.transform3d(&"camera".into()), Some(&before_camera));
    assert_eq!(old.local.translation[0], 0.);
}

#[test]
fn spatial_bulk_matches_public_history_and_mixed_command_order() {
    use project_runtime_sdk::{
        ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D as Trs,
    };
    for mixed in [false, true] {
        let mut host = spatial_bulk_world();
        let mut public = host.clone();
        let old = host.committed_spatial_pose(&"cube".into()).unwrap();
        let old_value = (*old).clone();
        let make = |world: &World| {
            let mut b = ProjectRuntimeMutationBuffer::new();
            for x in [2., 4., 8.] {
                b.spatial_local_transform(
                    world.spatial_entity_key(&"cube".into()).unwrap(),
                    Trs {
                        translation: [x, 2., 3.],
                        ..Trs::identity()
                    },
                );
                if mixed {
                    b.spatial_command(
                        "cube".into(),
                        world.entity_generation(&"cube".into()).unwrap(),
                        Intent::SetLocalTransform {
                            transform: Trs {
                                translation: [x + 1., 2., 3.],
                                ..Trs::identity()
                            },
                        },
                    );
                }
            }
            b
        };
        let (report, _) = make(&host).commit_with_runtime(&mut host, None).unwrap();
        let history = make(&public)
            .prepare(&public)
            .unwrap()
            .commit(&mut public)
            .unwrap();
        assert_eq!(report.committed_count, if mixed { 6 } else { 3 });
        assert_eq!(history.records.len(), report.committed_count);
        assert_eq!(
            host.transform3d(&"cube".into()).unwrap().translation[0],
            if mixed { 9. } else { 8. }
        );
        assert_eq!(host.finish_spatial_tick(), public.finish_spatial_tick());
        assert_eq!(
            host.committed_spatial_pose(&"cube".into()),
            public.committed_spatial_pose(&"cube".into())
        );
        assert_eq!(
            host.prepare_spatial_presentation(1, 0.5).unwrap(),
            public.prepare_spatial_presentation(1, 0.5).unwrap()
        );
        assert_eq!(*old, old_value, "old immutable packet must retain its pose");
        host.begin_spatial_tick(2);
        public.begin_spatial_tick(2);
        assert_eq!(host.finish_spatial_tick(), public.finish_spatial_tick());
    }
}

#[test]
fn spatial_bulk_invalid_tail_foreign_stale_and_budget_are_atomic() {
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut world = spatial_bulk_world();
    let key = world.spatial_entity_key(&"cube".into()).unwrap();
    let before = world.transform3d(&"cube".into()).unwrap().clone();
    let dirty = world.dirty_records().to_vec();
    for invalid in [
        Trs {
            scale: [0.; 3],
            ..Trs::identity()
        },
        Trs {
            translation: [f32::NAN, 0., 0.],
            ..Trs::identity()
        },
    ] {
        let mut b = ProjectRuntimeMutationBuffer::new();
        b.spatial_local_transform(
            key,
            Trs {
                translation: [9.; 3],
                ..Trs::identity()
            },
        );
        b.spatial_local_transform(key, invalid);
        assert_eq!(
            b.commit_with_runtime(&mut world, None)
                .unwrap_err()
                .report
                .committed_count,
            0
        );
        assert_eq!(world.transform3d(&"cube".into()), Some(&before));
        assert_eq!(world.dirty_records(), dirty);
    }
    let mut profile = world.spatial_profile();
    profile.max_value_intents = 2;
    world.set_spatial_profile(profile).unwrap();
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(key, Trs::identity());
    b.commit_with_runtime(&mut world, None).unwrap();
    let mut mixed = ProjectRuntimeMutationBuffer::new();
    mixed.spatial_command(
        "cube".into(),
        world.entity_generation(&"cube".into()).unwrap(),
        project_runtime_sdk::ProjectRuntimeSpatialIntent::SetLocalTransform {
            transform: Trs::identity(),
        },
    );
    mixed.commit_with_runtime(&mut world, None).unwrap();
    let mut over = ProjectRuntimeMutationBuffer::new();
    over.spatial_local_transform(key, Trs::identity());
    assert!(over
        .commit_with_runtime(&mut world, None)
        .unwrap_err()
        .message
        .contains("spatial_budget_exceeded"));
    let mut foreign = spatial_bulk_world();
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(key, Trs::identity());
    assert!(b.prepare(&foreign).is_err());
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(key, Trs::identity());
    // A prepared transaction cannot be transferred to an unrelated World with the same source IDs.
    world.begin_spatial_tick(2);
    let prepared = b.prepare(&world).unwrap();
    assert_eq!(
        prepared.commit(&mut foreign).unwrap_err().code,
        "three_d.stale_entity"
    );
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(key, Trs::identity());
    let prepared = b.prepare(&world).unwrap();
    world.despawn_entity(&"cube".into()).unwrap();
    assert_eq!(
        prepared.commit(&mut world).unwrap_err().code,
        "three_d.stale_entity"
    );
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(key, Trs::identity());
    assert!(b.commit_with_runtime(&mut world, None).is_err());
}

#[test]
fn spatial_bulk_keeps_camera_ancestor_checks_and_pending_dirty_propagation() {
    use project_runtime_sdk::{
        ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D as Trs,
    };
    let mut world = spatial_bulk_world();
    let mut parent = ProjectRuntimeMutationBuffer::new();
    parent.spatial_command(
        "camera".into(),
        world.entity_generation(&"camera".into()).unwrap(),
        Intent::Reparent {
            parent_entity_id: Some("cube".into()),
            parent_generation: world.entity_generation(&"cube".into()).unwrap(),
            preserve_world: true,
        },
    );
    parent.commit_with_runtime(&mut world, None).unwrap();
    let key = world.spatial_entity_key(&"cube".into()).unwrap();
    let old = world.transform3d(&"cube".into()).unwrap().clone();
    let mut bad = ProjectRuntimeMutationBuffer::new();
    bad.spatial_local_transform(
        key,
        Trs {
            scale: [2., 1., 1.],
            ..Trs::identity()
        },
    );
    assert!(bad
        .commit_with_runtime(&mut world, None)
        .unwrap_err()
        .message
        .contains("camera_scale"));
    assert_eq!(world.transform3d(&"cube".into()), Some(&old));
    let mut good = ProjectRuntimeMutationBuffer::new();
    good.spatial_local_transform(
        key,
        Trs {
            translation: [3., 0., 0.],
            ..Trs::identity()
        },
    );
    let prepared = good.prepare(&world).unwrap();
    // A delayed prepared write is revalidated against current camera/ancestor constraints.
    let mut add = ProjectRuntimeMutationBuffer::new();
    add.spatial_local_transform(
        world.spatial_entity_key(&"camera".into()).unwrap(),
        Trs {
            translation: [0., 1., 0.],
            ..Trs::identity()
        },
    );
    add.commit_with_runtime(&mut world, None).unwrap();
    prepared.commit(&mut world).unwrap();
    world.finish_spatial_tick();
    assert_eq!(
        world.spatial_world(&"camera".into()).unwrap().pose.position,
        [3., 1., 0.]
    );
}

#[test]
fn spatial_bulk_same_source_respawn_does_not_rebind_old_key() {
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut world = spatial_bulk_world();
    let key = world.spatial_entity_key(&"cube".into()).unwrap();
    world.despawn_entity(&"cube".into()).unwrap();
    let scene = crate::spatial_contract::decode_runtime_scene(include_bytes!(
        "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
    ))
    .unwrap();
    let crate::spatial_contract::RuntimeSceneDocument::Spatial(scene) = scene else {
        panic!("spatial")
    };
    let runtime: crate::runtime_package::RuntimeScene = scene.into();
    crate::runtime_entity_hydration::PreparedRuntimeEntities::prepare_scene(
        &runtime.entities[..1],
        &world,
    )
    .unwrap()
    .commit(&mut world);
    let current = world.spatial_entity_key(&"cube".into()).unwrap();
    assert_ne!(key, current);
    let original = world.transform3d(&"cube".into()).cloned();
    let mut stale = ProjectRuntimeMutationBuffer::new();
    stale.spatial_local_transform(
        key,
        Trs {
            translation: [9.; 3],
            ..Trs::identity()
        },
    );
    assert!(stale.commit_with_runtime(&mut world, None).is_err());
    assert_eq!(world.transform3d(&"cube".into()).cloned(), original);
    let mut live = ProjectRuntimeMutationBuffer::new();
    live.spatial_local_transform(
        current,
        Trs {
            translation: [7.; 3],
            ..Trs::identity()
        },
    );
    live.commit_with_runtime(&mut world, None).unwrap();
    assert_eq!(
        world.transform3d(&"cube".into()).unwrap().translation,
        [7.; 3]
    );
}

#[test]
fn spatial_bulk_root_rows_validate_final_overlay_and_preserve_noop_notifications() {
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut host = spatial_bulk_world();
    let mut public = host.clone();
    let make = |w: &World| {
        let mut b = ProjectRuntimeMutationBuffer::new();
        let key = w.spatial_entity_key(&"cube".into()).unwrap();
        // Both inputs are canonical. Only the final overlay is published by
        // either owner path; the intermediate scale must not produce a pose.
        b.spatial_local_transform(
            key,
            Trs {
                scale: [2.; 3],
                ..Trs::identity()
            },
        );
        b.spatial_local_transform(
            key,
            Trs {
                translation: [2., 0., 0.],
                ..Trs::identity()
            },
        );
        b
    };
    make(&host).commit_with_runtime(&mut host, None).unwrap();
    make(&public)
        .prepare(&public)
        .unwrap()
        .commit(&mut public)
        .unwrap();
    assert_eq!(host.finish_spatial_tick(), public.finish_spatial_tick());
    assert_eq!(host.take_dirty_records(), public.take_dirty_records());
    let pose = host.committed_spatial_pose(&"cube".into()).unwrap();
    let mut no_op = ProjectRuntimeMutationBuffer::new();
    no_op.spatial_local_transform(
        host.spatial_entity_key(&"cube".into()).unwrap(),
        Trs {
            translation: [2., 0., 0.],
            ..Trs::identity()
        },
    );
    no_op.commit_with_runtime(&mut host, None).unwrap();
    assert!(host.take_dirty_records().is_empty());
    assert!(std::sync::Arc::ptr_eq(
        &pose,
        &host.committed_spatial_pose(&"cube".into()).unwrap()
    ));
}

#[test]
fn spatial_bulk_delayed_prepare_rechecks_new_camera_descendants_and_pending_dirty() {
    use project_runtime_sdk::{
        ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D as Trs,
    };
    let mut world = spatial_bulk_world();
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(
        world.spatial_entity_key(&"cube".into()).unwrap(),
        Trs {
            scale: [2., 1., 1.],
            ..Trs::identity()
        },
    );
    let prepared = b.prepare(&world).unwrap();
    let mut reparent = ProjectRuntimeMutationBuffer::new();
    reparent.spatial_command(
        "camera".into(),
        world.entity_generation(&"camera".into()).unwrap(),
        Intent::Reparent {
            parent_entity_id: Some("cube".into()),
            parent_generation: world.entity_generation(&"cube".into()).unwrap(),
            preserve_world: false,
        },
    );
    reparent.commit_with_runtime(&mut world, None).unwrap();
    assert!(prepared
        .commit(&mut world)
        .unwrap_err()
        .message
        .contains("camera_scale"));
    assert_eq!(world.transform3d(&"cube".into()).unwrap().scale, [1.; 3]);
    // Public commit leaves pending dirties. A following hot commit publishes both
    // its changed camera and the pending ancestor, through the original timeline.
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(
        world.spatial_entity_key(&"cube".into()).unwrap(),
        Trs {
            translation: [4., 0., 0.],
            ..Trs::identity()
        },
    );
    b.prepare(&world).unwrap().commit(&mut world).unwrap();
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_local_transform(
        world.spatial_entity_key(&"camera".into()).unwrap(),
        Trs {
            translation: [0., 2., 0.],
            ..Trs::identity()
        },
    );
    b.commit_with_runtime(&mut world, None).unwrap();
    assert_eq!(
        world.spatial_world(&"camera".into()).unwrap().pose.position,
        [4., 2., 0.]
    );
    assert_eq!(
        world
            .committed_spatial_pose(&"cube".into())
            .unwrap()
            .state
            .pose
            .position,
        [4., 0., 0.]
    );
}

#[test]
fn spatial_bulk_tick_settle_reuses_unshared_pose_and_cow_preserves_external_history() {
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    use std::sync::Arc;
    let mut world = spatial_bulk_world();
    let id = "cube".into();
    let write = |world: &mut World, x: f32| {
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        buffer.spatial_local_transform(
            world.spatial_entity_key(&id).unwrap(),
            Trs {
                translation: [x, 0., 0.],
                ..Trs::identity()
            },
        );
        buffer.commit_with_runtime(world, None).unwrap();
        world.take_dirty_records();
    };
    write(&mut world, 2.);
    let before = Arc::as_ptr(&world.committed_spatial_pose(&id).unwrap());
    world.begin_spatial_tick(2);
    let after = Arc::as_ptr(&world.committed_spatial_pose(&id).unwrap());
    assert_eq!(
        before, after,
        "settling an unshared pose needlessly allocates a new body"
    );
    write(&mut world, 3.);
    let retained = world.committed_spatial_pose(&id).unwrap();
    let original = retained.as_ref().clone();
    let delta = world.spatial_delta();
    world.begin_spatial_tick(3);
    let settled = world.committed_spatial_pose(&id).unwrap();
    assert!(!Arc::ptr_eq(&retained, &settled));
    assert_eq!(*retained, original);
    assert_eq!(
        settled.state.previous_world_matrix,
        settled.state.world_matrix
    );
    assert_eq!(
        delta
            .changes
            .iter()
            .find(|c| c.entity_id == id)
            .unwrap()
            .pose
            .as_deref(),
        Some(&original)
    );
}

#[test]
fn spatial_bulk_memory_observes_real_pose_and_source_backing_with_shared_dedup() {
    use crate::resource_memory::{memory_totals, MemoryCoverage, ResourceMemorySample};
    let world = spatial_bulk_world();
    let id = EntityId::from("cube");
    let pose = world.committed_spatial_pose(&id).unwrap();
    let pose_id = format!("cpu-storage:{:x}", std::sync::Arc::as_ptr(&pose) as usize);
    let key = world.runtime_id_for_source(&id).unwrap();
    let source = &world.try_resolve_runtime_entity(key).unwrap().source_id;
    let source_id = format!("cpu-storage:{:x}", source.as_str().as_ptr() as usize);
    let mut sample = ResourceMemorySample::new(1, 1, 0);
    world.observe_memory(&mut sample);
    assert!(sample
        .allocations
        .iter()
        .any(|a| a.allocation_id == pose_id
            && a.bytes == std::mem::size_of_val(pose.as_ref()) as u64));
    assert!(sample
        .allocations
        .iter()
        .any(|a| a.allocation_id == source_id && a.bytes == source.as_str().len() as u64));
    let once = memory_totals(&sample.allocations).unwrap();
    world.observe_memory(&mut sample);
    assert_eq!(memory_totals(&sample.allocations).unwrap(), once);
    let clone = world.clone();
    clone.observe_memory(&mut sample);
    assert!(memory_totals(&sample.allocations).unwrap().cpu_allocated > once.cpu_allocated);
    assert_eq!(
        sample.coverage.get("cpu.owner.world"),
        Some(&MemoryCoverage::Partial)
    );
    assert_eq!(
        sample
            .allocations
            .iter()
            .filter(|a| a.allocation_id == pose_id)
            .map(|a| a.bytes)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1
    );
}

#[test]
fn spatial_bulk_pose_bodies_reuse_after_warmup_and_preserve_readers() {
    use project_runtime_sdk::ProjectRuntimeTransform3D as Trs;
    let mut world = spatial_bulk_world();
    let id = EntityId::from("cube");
    let mut addresses = std::collections::BTreeSet::new();
    for tick in 2..10 {
        world.begin_spatial_tick(tick);
        let mut b = ProjectRuntimeMutationBuffer::new();
        b.spatial_local_transform(
            world.spatial_entity_key(&id).unwrap(),
            Trs {
                translation: [tick as f32, 0., 0.],
                ..Trs::identity()
            },
        );
        b.commit_with_runtime(&mut world, None).unwrap();
        world.take_dirty_records();
        if tick >= 4 {
            addresses.insert(
                std::sync::Arc::as_ptr(&world.committed_spatial_pose(&id).unwrap()) as usize,
            );
        }
    }
    assert!(
        addresses.len() <= 2,
        "steady history should reuse two bodies, got {}",
        addresses.len()
    );
    let retained = world.committed_spatial_pose(&id).unwrap();
    let before = retained.as_ref().clone();
    let old_delta = world.spatial_delta();
    let mut cloned = world.clone();
    for target in [&mut world, &mut cloned] {
        for tick in 10..14 {
            target.begin_spatial_tick(tick);
            let mut b = ProjectRuntimeMutationBuffer::new();
            b.spatial_local_transform(
                target.spatial_entity_key(&id).unwrap(),
                Trs {
                    translation: [tick as f32, 0., 0.],
                    ..Trs::identity()
                },
            );
            b.commit_with_runtime(target, None).unwrap();
            target.take_dirty_records();
            assert_eq!(*retained, before);
            assert_eq!(
                old_delta
                    .changes
                    .iter()
                    .find(|c| c.entity_id == id)
                    .unwrap()
                    .pose
                    .as_deref(),
                Some(&before)
            );
        }
    }
}

#[test]
fn spatial_bulk_host_tick_drops_internal_delta_before_settle_at_scale() {
    use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
    use crate::spatial_contract::{
        decode_runtime_scene, RuntimeEntityDocument, RuntimeSceneDocument, RuntimeSpatial,
    };
    use project_runtime_sdk::{spatial_bulk::SpatialEntityKey, ProjectRuntimeTransform3D as Trs};
    struct Session {
        keys: Vec<SpatialEntityKey>,
        tick: u32,
    }
    impl ProjectRuntimeSession for Session {
        fn session_id(&self) -> &str {
            "spatial-tick-history"
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            ProjectRuntimeSessionOutput::no_op()
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            self.tick += 1;
            let mut b = ProjectRuntimeMutationBuffer::new();
            for &key in &self.keys {
                b.spatial_local_transform(
                    key,
                    Trs {
                        translation: [self.tick as f32, 0., 0.],
                        ..Trs::identity()
                    },
                );
            }
            ProjectRuntimeSessionOutput::applied(b)
        }
    }
    for size in [1, 100, 1000, 10000] {
        let RuntimeSceneDocument::Spatial(mut scene) = decode_runtime_scene(include_bytes!(
            "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
        ))
        .unwrap() else {
            panic!("spatial");
        };
        let template = scene.entities[0].clone();
        let camera = scene.entities[1].clone();
        scene.entities = (0..size)
            .map(|i| {
                let mut e = template.clone();
                if let RuntimeEntityDocument::Spatial(ref mut v) = e {
                    v.id = format!("node-{i}");
                    let RuntimeSpatial::World3D { mesh_renderer, .. } = &mut v.spatial;
                    *mesh_renderer = None;
                }
                e
            })
            .collect();
        scene.entities.push(camera);
        let mut world = crate::scene_loader::load_scene_into_world(&scene.into())
            .value
            .unwrap();
        world.take_dirty_records();
        let keys = (0..size)
            .map(|i| {
                world
                    .spatial_entity_key(&format!("node-{i}").into())
                    .unwrap()
            })
            .collect();
        let mut host = EngineHostLoop::with_project_runtime_session(
            "main",
            Default::default(),
            Box::new(Session { keys, tick: 0 }),
        );
        for steps in [1, 4] {
            for frame in 0..6 {
                World::spatial_tick_cow_work(true);
                host.tick(
                    EngineFrameInput::new(EngineHostMode::ExportedGame)
                        .with_fixed_step_count(steps),
                    &mut world,
                );
                let copies = World::spatial_tick_cow_work(false);
                if frame >= 2 {
                    assert_eq!(
                        copies, 0,
                        "internal old delta retained across begin: roots={size}, steps={steps}"
                    );
                }
            }
        }
        // External readers still require COW. Fix only the host's temporary lease.
        let old = world.spatial_delta();
        let pose = old.changes[0].pose.clone().unwrap();
        let original = pose.as_ref().clone();
        World::spatial_tick_cow_work(true);
        host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(1),
            &mut world,
        );
        assert_eq!(World::spatial_tick_cow_work(false), size);
        assert_eq!(*pose, original);
    }
}

#[test]
fn foundation_bm12_hot_bulk_one_one_point_five_two_scale_keep_original_tick_capacity() {
    for count in [10000usize, 15000, 20000] {
        use crate::spatial_contract::{
            decode_runtime_scene, RuntimeEntityDocument, RuntimeSceneDocument, RuntimeSpatial,
        };
        let RuntimeSceneDocument::Spatial(mut scene) = decode_runtime_scene(include_bytes!(
            "../../tests/fixtures/three_d_a0/runtime-scene-v2.json"
        ))
        .unwrap() else {
            panic!("spatial")
        };
        let template = scene.entities[0].clone();
        scene.entities = (0..count)
            .map(|i| {
                let mut entity = template.clone();
                if let RuntimeEntityDocument::Spatial(ref mut v) = entity {
                    v.id = format!("node-{i}");
                    let RuntimeSpatial::World3D {
                        transform,
                        mesh_renderer,
                        ..
                    } = &mut v.spatial;
                    transform.translation = [0.; 3];
                    *mesh_renderer = None;
                }
                entity
            })
            .collect();
        scene.main_camera = None;
        let mut world = crate::scene_loader::load_scene_into_world(&scene.into())
            .value
            .unwrap();
        world.take_dirty_records();
        world.begin_spatial_tick(1);
        let keys = (0..count)
            .map(|i| {
                world
                    .spatial_entity_key(&format!("node-{i}").into())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let make = |keys: &[_], x: f32| {
            let mut batch = ProjectRuntimeMutationBuffer::new();
            for &key in keys {
                batch.spatial_local_transform(
                    key,
                    project_runtime_sdk::ProjectRuntimeTransform3D {
                        translation: [x, 0., 0.],
                        ..project_runtime_sdk::ProjectRuntimeTransform3D::identity()
                    },
                );
            }
            batch
        };
        let result = make(&keys, 2.).commit_with_runtime(&mut world, None);
        if count <= 16384 {
            assert_eq!(result.unwrap().0.committed_count, count);
            let retained = world.committed_spatial_pose(&"node-0".into()).unwrap();
            world.take_dirty_records();
            world.begin_spatial_tick(2);
            assert_eq!(
                make(&keys, 3.)
                    .commit_with_runtime(&mut world, None)
                    .unwrap()
                    .0
                    .committed_count,
                count
            );
            assert_eq!(world.finish_spatial_tick().changes.len(), count);
            assert_eq!(retained.local.translation[0], 2.);
        } else {
            assert!(result.unwrap_err().message.contains("budget"));
            assert!(world.take_dirty_records().is_empty());
            assert!(world.finish_spatial_tick().changes.is_empty());
            for i in [0, count / 2, count - 1] {
                assert_eq!(
                    world
                        .transform3d(&format!("node-{i}").into())
                        .unwrap()
                        .translation[0],
                    0.
                );
            }
            // Splitting cannot bypass the unchanged 16384-per-tick capacity.
            make(&keys[..10000], 4.)
                .commit_with_runtime(&mut world, None)
                .unwrap();
            let before = world.spatial_delta();
            let notifications = world.dirty_records().to_vec();
            assert!(make(&keys[10000..], 5.)
                .commit_with_runtime(&mut world, None)
                .unwrap_err()
                .message
                .contains("budget"));
            assert_eq!(world.spatial_delta(), before);
            assert_eq!(world.dirty_records(), notifications);
            assert_eq!(
                world.transform3d(&"node-15000".into()).unwrap().translation[0],
                0.
            );
            world.take_dirty_records();
            world.begin_spatial_tick(2);
            make(&keys[10000..], 6.)
                .commit_with_runtime(&mut world, None)
                .unwrap();
            assert_eq!(world.finish_spatial_tick().changes.len(), 10000);
            assert_eq!(before.changes.len(), 10000);
            assert_eq!(
                before.changes[0].pose.as_ref().unwrap().local.translation[0],
                4.
            );
        }
    }
}
