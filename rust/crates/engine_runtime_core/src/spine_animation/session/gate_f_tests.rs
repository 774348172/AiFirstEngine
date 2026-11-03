use super::*;
use crate::components::Hierarchy;

#[test]
fn spine374_f_first_geometry_and_hash_fallback_preserve_interleaved_poses() {
    use std::sync::Arc;
    let (mut module, mut world) = tests::fixture();
    for (index, id) in ["a-first", "z-same", "zz-same"].into_iter().enumerate() {
        world
            .try_spawn_entity(
                id.into(),
                id,
                "entity",
                true,
                Hierarchy {
                    parent_id: None,
                    sibling_order: index as i32 + 1,
                },
            )
            .unwrap();
        module
            .bindings
            .push((id.into(), module.bindings[0].1.clone()));
    }
    module.refresh(&world).unwrap();
    module.advance(&world, 1).unwrap();
    let pause = |id: &str| AnimatorCommand {
        entity_id: id.into(),
        generation: world.entity_generation(&id.into()).unwrap(),
        intent: Wire::SetPaused { paused: true },
    };
    module.queue(vec![pause("robot")]);
    module.advance(&world, 2).unwrap();
    let sources = module.render_sources().unwrap();
    let get = |id: &str| &sources.iter().find(|s| s.0.as_str() == id).unwrap().1;
    assert!(Arc::ptr_eq(get("a-first"), get("z-same")));
    assert!(Arc::ptr_eq(get("a-first"), get("zz-same")));
    assert!(!Arc::ptr_eq(get("a-first"), get("robot")));
    for (id, geometry, _) in &sources {
        assert_eq!(**geometry, module.geometry(id.as_str()).unwrap());
    }
    module.queue(vec![pause("a-first")]);
    module.advance(&world, 3).unwrap();
    let next = module.render_sources().unwrap();
    let get_next = |id: &str| &next.iter().find(|s| s.0.as_str() == id).unwrap().1;
    assert!(Arc::ptr_eq(get_next("z-same"), get_next("zz-same")));
    assert!(!Arc::ptr_eq(get_next("a-first"), get_next("z-same")));
    assert!(!Arc::ptr_eq(get_next("a-first"), get_next("robot")));
    for (id, geometry, _) in &next {
        assert_eq!(**geometry, module.geometry(id.as_str()).unwrap());
        assert!(!sources.iter().any(|s| Arc::ptr_eq(&s.1, geometry)));
    }
}

#[test]
fn spine374_f_call_local_geometry_reuse_preserves_independent_instances_and_counts() {
    use std::sync::Arc;
    let (mut module, mut world) = tests::fixture();
    world
        .try_spawn_entity(
            "other".into(),
            "Other",
            "entity",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 1,
            },
        )
        .unwrap();
    module
        .bindings
        .push(("other".into(), module.bindings[0].1.clone()));
    module.refresh(&world).unwrap();
    module.begin_measurement_frame(true);
    module.advance(&world, 1).unwrap();
    let sources = module.render_sources().unwrap();
    assert_eq!(sources.len(), 2);
    assert!(Arc::ptr_eq(&sources[0].1, &sources[1].1));
    let sample = module.cpu_measurement().unwrap();
    assert_eq!(
        (sample.evaluated_instances, sample.geometry_instances),
        (2, 2)
    );
    assert_eq!(sample.vertices, 2 * sources[0].1.vertices.len() as u64);
    assert_eq!(
        module.events.len(),
        2,
        "each independent event cursor publishes"
    );
    let repeat = module.render_sources().unwrap();
    assert!(Arc::ptr_eq(&repeat[0].1, &repeat[1].1));
    assert!(
        !Arc::ptr_eq(&sources[0].1, &repeat[0].1),
        "no cross-call retention"
    );
    module.queue(vec![AnimatorCommand {
        entity_id: "robot".into(),
        generation: world.entity_generation(&"robot".into()).unwrap(),
        intent: Wire::SetPaused { paused: true },
    }]);
    module.advance(&world, 2).unwrap();
    let changed = module.render_sources().unwrap();
    assert!(!Arc::ptr_eq(&changed[0].1, &changed[1].1));
    assert_ne!(changed[0].1, changed[1].1);
    assert_eq!(
        *sources[0].1,
        module.geometry("robot").unwrap(),
        "old immutable output retained"
    );
}

#[test]
fn spine374_f_cpu_measurement_tracks_real_work_without_changing_pose_or_failure_state() {
    let (mut measured, world) = tests::fixture();
    let (mut reference, other_world) = tests::fixture();
    measured.begin_measurement_frame(true);
    measured.callback_snapshots(&world).unwrap();
    measured.advance(&world, 1).unwrap();
    reference.advance(&other_world, 1).unwrap();
    let geometry = measured.render_sources().unwrap();
    assert_eq!(geometry[0].1, reference.render_sources().unwrap()[0].1);
    let sample = measured.cpu_measurement().unwrap();
    assert_eq!(
        (sample.first_fixed_tick, sample.last_fixed_tick),
        (Some(1), Some(1))
    );
    assert!(
        sample.control_ns > 0
            && sample.official_apply_world_ns > 0
            && sample.vertices_clipping_ns > 0
    );
    assert_eq!(
        (sample.evaluated_instances, sample.geometry_instances),
        (1, 1)
    );
    assert_eq!(sample.vertices, geometry[0].1.vertices.len() as u64);
    assert_eq!(
        sample.projection_encode_ns, 0,
        "unexecuted stage invented time"
    );
    assert!(reference.cpu_measurement().is_none());
    measured.begin_measurement_frame(true);
    assert!(measured.advance(&world, 0).is_err());
    let failed = measured.cpu_measurement().unwrap();
    assert_eq!(
        (failed.evaluated_instances, failed.official_apply_world_ns),
        (0, 0)
    );
    assert!(failed.control_ns > 0);
    assert!(failed.first_fixed_tick.is_none() && failed.last_fixed_tick.is_none());
    measured.begin_measurement_frame(false);
    measured.advance(&world, 2).unwrap();
    measured.render_sources().unwrap();
    assert!(measured.cpu_measurement().is_none());
}

#[test]
fn spine374_f_world_replacement_cannot_inherit_same_name_and_generation_state() {
    let (mut module, world) = tests::fixture();
    module.advance(&world, 20).unwrap();
    module.queue(vec![AnimatorCommand {
        entity_id: "robot".into(),
        generation: world.entity_generation(&"robot".into()).unwrap(),
        intent: Wire::SetPaused { paused: true },
    }]);
    assert!(!module.events.is_empty());
    let mut replacement = World::new();
    replacement
        .try_spawn_entity(
            "robot".into(),
            "Robot",
            "entity",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    assert_ne!(world.epoch(), replacement.epoch());
    assert_eq!(
        world.entity_generation(&"robot".into()),
        replacement.entity_generation(&"robot".into())
    );
    assert_eq!(
        module.preflight(&replacement, 1, &[]).unwrap_err(),
        "animator.runtime.stale_handle"
    );
    module.refresh(&replacement).unwrap();
    assert_eq!(
        module.tick_index(),
        None,
        "new World inherited the previous simulation clock"
    );
    assert!(
        module.events.is_empty() && module.pending.is_empty(),
        "old World work crossed the replacement boundary"
    );
    assert_eq!(module.snapshot("robot").unwrap().pose_revision, 0);
    module.advance(&replacement, 1).unwrap();
    assert_eq!(module.snapshot("robot").unwrap().tick, 1);
}

#[test]
fn spine374_f_advance_detects_world_replacement_before_clock_validation() {
    let (mut module, world) = tests::fixture();
    module.advance(&world, 20).unwrap();
    let (_, replacement) = tests::fixture();
    module.advance(&replacement, 1).unwrap();
    assert_eq!(module.tick_index(), Some(1));
    assert_eq!(module.snapshot("robot").unwrap().pose_revision, 1);
}

#[test]
fn spine374_f_shared_data_keeps_independent_pose_and_rejects_invalid_skin() {
    let (mut module, mut world) = tests::fixture();
    world
        .try_spawn_entity(
            "other".into(),
            "Other",
            "entity",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 1,
            },
        )
        .unwrap();
    module
        .bindings
        .push(("other".into(), module.bindings[0].1.clone()));
    module.refresh(&world).unwrap();
    // One compiled table, two native instances. The table must outlive either instance.
    assert_eq!(std::rc::Rc::strong_count(&module.tables[0]), 3);
    let generation = world.entity_generation(&"robot".into()).unwrap();
    module.queue(vec![AnimatorCommand {
        entity_id: "robot".into(),
        generation,
        intent: Wire::SetPaused { paused: true },
    }]);
    for tick in 1..=60 {
        module.callback_snapshots(&world).unwrap();
        module.advance(&world, tick).unwrap();
    }
    assert_eq!(module.live[&"robot".into()].controller.logical_tick(), 0);
    assert_eq!(module.live[&"other".into()].controller.logical_tick(), 59);
    assert_ne!(
        module.geometry("robot").unwrap(),
        module.geometry("other").unwrap()
    );
    let before = module.snapshot("other").unwrap().clone();
    let rejected = AnimatorCommand {
        entity_id: "robot".into(),
        generation,
        intent: Wire::SetSkin {
            name: "missing-skin".into(),
        },
    };
    assert!(module.preflight(&world, 61, &[rejected]).is_err());
    assert_eq!(module.snapshot("other"), Some(&before));
    module.queue(vec![AnimatorCommand {
        entity_id: "robot".into(),
        generation,
        intent: Wire::SetPaused { paused: false },
    }]);
    module.callback_snapshots(&world).unwrap();
    module.advance(&world, 61).unwrap();
    module.advance(&world, 62).unwrap();
    assert_eq!(module.live[&"robot".into()].controller.logical_tick(), 1);
}

#[test]
fn spine374_f_world_replacement_reclaims_native_instances_without_recompiling_data() {
    let baseline = engine_spine_runtime::live_native_allocations();
    {
        let (mut module, world) = tests::fixture();
        module.advance(&world, 1).unwrap();
        let steady = engine_spine_runtime::live_native_allocations();
        for _ in 0..64 {
            let mut replacement = World::new();
            replacement
                .try_spawn_entity(
                    "robot".into(),
                    "Robot",
                    "entity",
                    true,
                    Hierarchy {
                        parent_id: None,
                        sibling_order: 0,
                    },
                )
                .unwrap();
            module.advance(&replacement, 1).unwrap();
            assert_eq!(std::rc::Rc::strong_count(&module.tables[0]), 2);
            assert_eq!(engine_spine_runtime::live_native_allocations(), steady);
        }
    }
    assert_eq!(engine_spine_runtime::live_native_allocations(), baseline);
}

#[test]
fn spine374_f_disable_and_recreate_retire_events_commands_and_native_instances() {
    use crate::archetype::ComponentValue;
    let (mut module, mut world) = tests::fixture();
    let id: EntityId = "robot".into();
    let old_generation = world.entity_generation(&id).unwrap();
    module.advance(&world, 20).unwrap();
    module.queue(vec![AnimatorCommand {
        entity_id: id.clone(),
        generation: old_generation,
        intent: Wire::SetPaused { paused: true },
    }]);
    let mut meta = world.entity(&id).unwrap().clone();
    meta.enabled = false;
    let value = ComponentValue::EntityMeta(meta.clone());
    world.insert_component_value(id.clone(), value);
    module.refresh(&world).unwrap();
    assert!(module.live.is_empty() && module.events.is_empty() && module.pending.is_empty());
    assert_eq!(std::rc::Rc::strong_count(&module.tables[0]), 1);
    meta.enabled = true;
    let value = ComponentValue::EntityMeta(meta);
    world.insert_component_value(id.clone(), value);
    module.refresh(&world).unwrap();
    assert_eq!(module.snapshot("robot").unwrap().pose_revision, 0);
    for tick in 21..=2400 {
        module.callback_snapshots(&world).unwrap();
        module.advance(&world, tick).unwrap();
        assert!(module.events.len() <= 64);
        assert!(module.live[&id].controller.live_entry_count(0) <= 1);
        assert_eq!(std::rc::Rc::strong_count(&module.tables[0]), 2);
    }
    world.try_despawn_entity(&id).unwrap();
    world
        .try_spawn_entity(
            id.clone(),
            "Robot",
            "entity",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    module.refresh(&world).unwrap();
    assert_ne!(world.entity_generation(&id).unwrap(), old_generation);
    assert!(module.events.is_empty());
    assert_eq!(module.snapshot("robot").unwrap().pose_revision, 0);
    assert_eq!(
        module
            .preflight(
                &world,
                2401,
                &[AnimatorCommand {
                    entity_id: id,
                    generation: old_generation,
                    intent: Wire::Resume {}
                }]
            )
            .unwrap_err(),
        "animator.runtime.stale_handle"
    );
}
