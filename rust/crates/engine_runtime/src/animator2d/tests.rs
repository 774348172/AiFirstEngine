use super::*;
use crate::archetype::ComponentValue;
use crate::components::{ComponentTypeId, Hierarchy, SpriteRenderer2D};
use crate::ids::EntityId;
use crate::world::World;
use std::collections::BTreeMap;

#[test]
fn animator2d_evaluator_entry_frame_and_duration_boundary_are_deterministic() {
    let (mut world, registry) = world_and_registry(1000, Animator2DPlayback::Loop);
    let mut module = Animator2DModule::load(registry).unwrap();

    let attach = module.tick(&mut world, 1, Animator2DReportLevel::Trace);
    assert_eq!(sprite(&world), "idle-0");
    assert_eq!(attach.changed_entity_count, 1);
    module.tick(&mut world, 2, Animator2DReportLevel::Summary);
    assert_eq!(sprite(&world), "idle-0");
    module.tick(&mut world, 3, Animator2DReportLevel::Summary);
    assert_eq!(sprite(&world), "idle-1");
    module.tick(&mut world, 4, Animator2DReportLevel::Summary);
    module.tick(&mut world, 5, Animator2DReportLevel::Summary);
    assert_eq!(sprite(&world), "idle-0");

    let mut replay_world = world_fixture(&module.registry().registry_digest);
    let mut replay = Animator2DModule::load(module.registry().clone()).unwrap();
    for tick in 1..=5 {
        replay.tick(&mut replay_world, tick, Animator2DReportLevel::Off);
    }
    assert_eq!(sprite(&world), sprite(&replay_world));
    assert_eq!(
        module.entity_state(&EntityId::from("animated")),
        replay.entity_state(&EntityId::from("animated"))
    );
}

#[test]
fn animator2d_evaluator_same_inputs_replay_same_state_frame_and_report() {
    let registry = transition_registry();
    let mut left_world = world_fixture(&registry.registry_digest);
    let mut right_world = world_fixture(&registry.registry_digest);
    let mut left = Animator2DModule::load(registry.clone()).unwrap();
    let mut right = Animator2DModule::load(registry).unwrap();
    let commands = [
        Animator2DCommand::SetTrigger {
            entity_id: EntityId::from("animated"),
            parameter_id: "go".to_string(),
        },
        Animator2DCommand::SetTrigger {
            entity_id: EntityId::from("animated"),
            parameter_id: "spare".to_string(),
        },
    ];
    left.apply(commands.clone());
    right.apply(commands);

    for tick in 1..=4 {
        assert_eq!(
            left.tick(&mut left_world, tick, Animator2DReportLevel::Trace),
            right.tick(&mut right_world, tick, Animator2DReportLevel::Trace)
        );
    }
    assert_eq!(sprite(&left_world), sprite(&right_world));
    assert_eq!(
        left.entity_state(&EntityId::from("animated")),
        right.entity_state(&EntityId::from("animated"))
    );
}

#[test]
fn animator2d_evaluator_once_holds_and_speed_permille_uses_integer_accumulator() {
    let (mut world, registry) = world_and_registry(2500, Animator2DPlayback::Once);
    let mut module = Animator2DModule::load(registry).unwrap();
    module.tick(&mut world, 1, Animator2DReportLevel::Off);
    module.tick(&mut world, 2, Animator2DReportLevel::Off);
    assert_eq!(sprite(&world), "idle-1");
    assert!(
        !module
            .entity_state(&EntityId::from("animated"))
            .unwrap()
            .completed
    );
    module.tick(&mut world, 3, Animator2DReportLevel::Off);
    assert!(
        module
            .entity_state(&EntityId::from("animated"))
            .unwrap()
            .completed
    );
    module.tick(&mut world, 4, Animator2DReportLevel::Off);
    assert_eq!(sprite(&world), "idle-1");

    let (mut slow_world, slow_registry) = world_and_registry(500, Animator2DPlayback::Loop);
    let mut slow = Animator2DModule::load(slow_registry).unwrap();
    slow.tick(&mut slow_world, 1, Animator2DReportLevel::Off);
    for tick in 2..=4 {
        slow.tick(&mut slow_world, tick, Animator2DReportLevel::Off);
    }
    assert_eq!(sprite(&slow_world), "idle-0");
    slow.tick(&mut slow_world, 5, Animator2DReportLevel::Off);
    assert_eq!(sprite(&slow_world), "idle-1");
}

#[test]
fn animator2d_transition_priority_then_id_and_winning_trigger_consumption() {
    let registry = transition_registry();
    let mut world = world_fixture(&registry.registry_digest);
    let mut module = Animator2DModule::load(registry).unwrap();
    module.apply([
        Animator2DCommand::SetTrigger {
            entity_id: EntityId::from("animated"),
            parameter_id: "go".to_string(),
        },
        Animator2DCommand::SetTrigger {
            entity_id: EntityId::from("animated"),
            parameter_id: "spare".to_string(),
        },
    ]);
    let report = module.tick(&mut world, 1, Animator2DReportLevel::Trace);
    let state = module.entity_state(&EntityId::from("animated")).unwrap();
    assert_eq!(state.state_id, "attack-a");
    assert_eq!(sprite(&world), "attack-a-0");
    assert!(!state.triggers.contains("go"));
    assert!(state.triggers.contains("spare"));
    assert_eq!(report.transition_count, 1);
    assert_eq!(report.trace.len(), 1);
}

#[test]
fn animator2d_transition_clip_end_occurs_on_boundary_and_does_not_chain() {
    let registry = clip_end_registry();
    let mut world = world_fixture(&registry.registry_digest);
    let mut module = Animator2DModule::load(registry).unwrap();

    module.tick(&mut world, 1, Animator2DReportLevel::Trace);
    assert_eq!(sprite(&world), "idle-0");
    let before_boundary = module.tick(&mut world, 2, Animator2DReportLevel::Trace);
    assert_eq!(before_boundary.transition_count, 0);
    assert_eq!(sprite(&world), "idle-0");

    let boundary = module.tick(&mut world, 3, Animator2DReportLevel::Trace);
    assert_eq!(boundary.transition_count, 1);
    assert_eq!(
        boundary.trace[0].transition_id.as_deref(),
        Some("idle-finished")
    );
    assert_eq!(sprite(&world), "attack-0");

    let next_tick = module.tick(&mut world, 4, Animator2DReportLevel::Trace);
    assert_eq!(next_tick.transition_count, 1);
    assert_eq!(
        next_tick.trace[0].transition_id.as_deref(),
        Some("attack-finished")
    );
    assert_eq!(sprite(&world), "done-0");
}

#[test]
fn animator2d_transition_bool_true_and_false_are_typed_and_deterministic() {
    let registry = bool_registry();
    let mut world = world_fixture(&registry.registry_digest);
    let mut module = Animator2DModule::load(registry).unwrap();

    module.apply([Animator2DCommand::SetBool {
        entity_id: EntityId::from("animated"),
        parameter_id: "armed".to_string(),
        value: true,
    }]);
    module.tick(&mut world, 1, Animator2DReportLevel::Off);
    assert_eq!(sprite(&world), "attack-0");
    assert_eq!(
        module
            .entity_state(&EntityId::from("animated"))
            .unwrap()
            .bools["armed"],
        true
    );

    module.apply([Animator2DCommand::SetBool {
        entity_id: EntityId::from("animated"),
        parameter_id: "armed".to_string(),
        value: false,
    }]);
    module.tick(&mut world, 2, Animator2DReportLevel::Off);
    assert_eq!(sprite(&world), "idle-0");
    assert_eq!(
        module
            .entity_state(&EntityId::from("animated"))
            .unwrap()
            .bools["armed"],
        false
    );
}

#[test]
fn animator2d_lifecycle_retires_despawn_and_generation_replacement() {
    let (mut world, registry) = world_and_registry(1000, Animator2DPlayback::Loop);
    let mut module = Animator2DModule::load(registry.clone()).unwrap();
    module.tick(&mut world, 1, Animator2DReportLevel::Off);
    assert_eq!(module.instance_count(), 1);
    world
        .try_despawn_entity(&EntityId::from("animated"))
        .unwrap();
    let report = module.tick(&mut world, 2, Animator2DReportLevel::Summary);
    assert_eq!(module.instance_count(), 0);
    assert_eq!(report.retired_entity_count, 1);

    let mut replacement = registry;
    replacement.clips[0].frames[0].sprite_asset_id = "replacement".to_string();
    replacement =
        CookedAnimator2DRegistry::from_parts(replacement.clips, replacement.controllers).unwrap();
    module.replace_registry(replacement.clone()).unwrap();
    assert_eq!(module.instance_count(), 0);
    assert_eq!(
        module.registry().registry_digest,
        replacement.registry_digest
    );
}

#[test]
fn animator2d_lifecycle_disable_and_entity_generation_replacement_reset_memory() {
    let (mut world, registry) = world_and_registry(1000, Animator2DPlayback::Loop);
    let mut module = Animator2DModule::load(registry.clone()).unwrap();
    module.tick(&mut world, 1, Animator2DReportLevel::Off);
    module.tick(&mut world, 2, Animator2DReportLevel::Off);
    module.tick(&mut world, 3, Animator2DReportLevel::Off);
    assert_eq!(sprite(&world), "idle-1");

    set_animator_enabled(&mut world, &registry.registry_digest, false);
    let disabled = module.tick(&mut world, 4, Animator2DReportLevel::Summary);
    assert_eq!(disabled.retired_entity_count, 1);
    assert_eq!(module.instance_count(), 0);

    set_animator_enabled(&mut world, &registry.registry_digest, true);
    module.tick(&mut world, 5, Animator2DReportLevel::Off);
    assert_eq!(sprite(&world), "idle-0");
    let old_runtime_id = world
        .runtime_id_for_source(&EntityId::from("animated"))
        .unwrap();
    world
        .try_despawn_entity(&EntityId::from("animated"))
        .unwrap();
    spawn_animated(&mut world, &registry.registry_digest);
    let new_runtime_id = world
        .runtime_id_for_source(&EntityId::from("animated"))
        .unwrap();
    assert_ne!(old_runtime_id, new_runtime_id);
    module.tick(&mut world, 6, Animator2DReportLevel::Off);
    assert_eq!(sprite(&world), "idle-0");
}

#[test]
fn animator2d_diagnostics_off_avoids_trace_and_invalid_command_fails_closed() {
    let (mut world, registry) = world_and_registry(1000, Animator2DPlayback::Loop);
    let mut module = Animator2DModule::load(registry).unwrap();
    module.apply([Animator2DCommand::SetBool {
        entity_id: EntityId::from("animated"),
        parameter_id: "missing".to_string(),
        value: true,
    }]);
    let report = module.tick(&mut world, 1, Animator2DReportLevel::Off);
    assert_eq!(report.failed_entity_count, 1);
    assert!(report.trace.is_empty());
    assert!(report.diagnostics.is_empty());
}

#[test]
fn animator2d_diagnostics_summary_deduplicates_and_reports_missing_owners() {
    let (mut world, registry) = world_and_registry(1000, Animator2DPlayback::Loop);
    let mut module = Animator2DModule::load(registry.clone()).unwrap();
    world.remove_sprite_renderer2d(&EntityId::from("animated"));

    let missing_renderer = module.tick(&mut world, 1, Animator2DReportLevel::Summary);
    assert_eq!(missing_renderer.diagnostics.len(), 1);
    assert!(missing_renderer.trace.is_empty());
    assert_eq!(
        missing_renderer.diagnostics[0].code,
        "animator2d.renderer_missing"
    );
    let duplicate = module.tick(&mut world, 2, Animator2DReportLevel::Summary);
    assert!(duplicate.diagnostics.is_empty());

    world
        .try_despawn_entity(&EntityId::from("animated"))
        .unwrap();
    module.apply([Animator2DCommand::SetTrigger {
        entity_id: EntityId::from("animated"),
        parameter_id: "missing".to_string(),
    }]);
    let missing_entity = module.tick(&mut world, 3, Animator2DReportLevel::Summary);
    assert_eq!(
        missing_entity.diagnostics[0].code,
        "animator2d.command_entity_missing"
    );

    spawn_animated(&mut world, &registry.registry_digest);
    set_animator_controller(&mut world, &registry.registry_digest, "unknown", 0, true);
    let missing_controller = module.tick(&mut world, 4, Animator2DReportLevel::Summary);
    assert_eq!(
        missing_controller.diagnostics[0].code,
        "animator2d.controller_missing"
    );
}

fn world_and_registry(
    speed_permille: u32,
    playback: Animator2DPlayback,
) -> (World, CookedAnimator2DRegistry) {
    let registry = registry(speed_permille, playback);
    let world = world_fixture(&registry.registry_digest);
    (world, registry)
}

fn world_fixture(registry_digest: &str) -> World {
    let mut world = World::new();
    spawn_animated(&mut world, registry_digest);
    world
}

fn spawn_animated(world: &mut World, registry_digest: &str) {
    let id = EntityId::from("animated");
    world
        .try_spawn_entity(
            id.clone(),
            "Animated",
            "actor",
            true,
            Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    world
        .try_insert_sprite_renderer2d(id.clone(), SpriteRenderer2D::default())
        .unwrap();
    world
        .try_insert_component_value(
            id,
            ComponentTypeId::animator2d(),
            ComponentValue::Animator2D(RuntimeAnimator2D {
                controller_id: "controller".to_string(),
                controller_index: 0,
                registry_digest: registry_digest.to_string(),
                enabled: true,
                initial_bools: BTreeMap::new(),
            }),
        )
        .unwrap();
}

fn set_animator_enabled(world: &mut World, registry_digest: &str, enabled: bool) {
    set_animator_controller(world, registry_digest, "controller", 0, enabled);
}

fn set_animator_controller(
    world: &mut World,
    registry_digest: &str,
    controller_id: &str,
    controller_index: u32,
    enabled: bool,
) {
    world
        .try_insert_component_value(
            EntityId::from("animated"),
            ComponentTypeId::animator2d(),
            ComponentValue::Animator2D(RuntimeAnimator2D {
                controller_id: controller_id.to_string(),
                controller_index,
                registry_digest: registry_digest.to_string(),
                enabled,
                initial_bools: BTreeMap::new(),
            }),
        )
        .unwrap();
}

fn registry(speed_permille: u32, playback: Animator2DPlayback) -> CookedAnimator2DRegistry {
    CookedAnimator2DRegistry::from_parts(
        vec![clip("idle", playback, ["idle-0", "idle-1"])],
        vec![CookedAnimatorController2D {
            id: "controller".to_string(),
            entry_state_index: 0,
            parameters: Vec::new(),
            states: vec![CookedAnimator2DState {
                id: "idle".to_string(),
                clip_index: 0,
                speed_permille,
            }],
            transitions: Vec::new(),
        }],
    )
    .unwrap()
}

fn transition_registry() -> CookedAnimator2DRegistry {
    CookedAnimator2DRegistry::from_parts(
        vec![
            clip(
                "attack-a",
                Animator2DPlayback::Once,
                ["attack-a-0", "attack-a-1"],
            ),
            clip(
                "attack-z",
                Animator2DPlayback::Once,
                ["attack-z-0", "attack-z-1"],
            ),
            clip("idle", Animator2DPlayback::Loop, ["idle-0", "idle-1"]),
        ],
        vec![CookedAnimatorController2D {
            id: "controller".to_string(),
            entry_state_index: 2,
            parameters: vec![
                CookedAnimator2DParameter {
                    id: "go".to_string(),
                    kind: Animator2DParameterKind::Trigger,
                    default_bool: false,
                },
                CookedAnimator2DParameter {
                    id: "spare".to_string(),
                    kind: Animator2DParameterKind::Trigger,
                    default_bool: false,
                },
            ],
            states: vec![
                CookedAnimator2DState {
                    id: "attack-a".to_string(),
                    clip_index: 0,
                    speed_permille: 1000,
                },
                CookedAnimator2DState {
                    id: "attack-z".to_string(),
                    clip_index: 1,
                    speed_permille: 1000,
                },
                CookedAnimator2DState {
                    id: "idle".to_string(),
                    clip_index: 2,
                    speed_permille: 1000,
                },
            ],
            transitions: vec![
                transition("a-wins", 2, 0),
                CookedAnimator2DTransition {
                    id: "z-loses".to_string(),
                    from_state_index: 2,
                    to_state_index: 1,
                    timing: Animator2DTransitionTiming::Immediate,
                    priority: 10,
                    conditions: vec![CookedAnimator2DCondition::Triggered { parameter_index: 1 }],
                },
            ],
        }],
    )
    .unwrap()
}

fn clip_end_registry() -> CookedAnimator2DRegistry {
    CookedAnimator2DRegistry::from_parts(
        vec![
            clip("attack", Animator2DPlayback::Once, ["attack-0"]),
            clip("done", Animator2DPlayback::Once, ["done-0"]),
            clip("idle", Animator2DPlayback::Once, ["idle-0"]),
        ],
        vec![CookedAnimatorController2D {
            id: "controller".to_string(),
            entry_state_index: 2,
            parameters: vec![CookedAnimator2DParameter {
                id: "enabled".to_string(),
                kind: Animator2DParameterKind::Bool,
                default_bool: true,
            }],
            states: vec![
                CookedAnimator2DState {
                    id: "attack".to_string(),
                    clip_index: 0,
                    speed_permille: 1000,
                },
                CookedAnimator2DState {
                    id: "done".to_string(),
                    clip_index: 1,
                    speed_permille: 1000,
                },
                CookedAnimator2DState {
                    id: "idle".to_string(),
                    clip_index: 2,
                    speed_permille: 1000,
                },
            ],
            transitions: vec![
                bool_transition(
                    "attack-finished",
                    0,
                    1,
                    Animator2DTransitionTiming::Immediate,
                    true,
                ),
                bool_transition(
                    "idle-finished",
                    2,
                    0,
                    Animator2DTransitionTiming::ClipEnd,
                    true,
                ),
            ],
        }],
    )
    .unwrap()
}

fn bool_registry() -> CookedAnimator2DRegistry {
    CookedAnimator2DRegistry::from_parts(
        vec![
            clip("attack", Animator2DPlayback::Loop, ["attack-0"]),
            clip("idle", Animator2DPlayback::Loop, ["idle-0"]),
        ],
        vec![CookedAnimatorController2D {
            id: "controller".to_string(),
            entry_state_index: 1,
            parameters: vec![CookedAnimator2DParameter {
                id: "armed".to_string(),
                kind: Animator2DParameterKind::Bool,
                default_bool: false,
            }],
            states: vec![
                CookedAnimator2DState {
                    id: "attack".to_string(),
                    clip_index: 0,
                    speed_permille: 1000,
                },
                CookedAnimator2DState {
                    id: "idle".to_string(),
                    clip_index: 1,
                    speed_permille: 1000,
                },
            ],
            transitions: vec![
                bool_transition(
                    "attack-to-idle",
                    0,
                    1,
                    Animator2DTransitionTiming::Immediate,
                    false,
                ),
                bool_transition(
                    "idle-to-attack",
                    1,
                    0,
                    Animator2DTransitionTiming::Immediate,
                    true,
                ),
            ],
        }],
    )
    .unwrap()
}

fn bool_transition(
    id: &str,
    from_state_index: u32,
    to_state_index: u32,
    timing: Animator2DTransitionTiming,
    value: bool,
) -> CookedAnimator2DTransition {
    CookedAnimator2DTransition {
        id: id.to_string(),
        from_state_index,
        to_state_index,
        timing,
        priority: 10,
        conditions: vec![CookedAnimator2DCondition::BoolEquals {
            parameter_index: 0,
            value,
        }],
    }
}

fn clip<const N: usize>(
    id: &str,
    playback: Animator2DPlayback,
    sprites: [&str; N],
) -> CookedSpriteAnimationClip2D {
    CookedSpriteAnimationClip2D {
        id: id.to_string(),
        playback,
        frames: sprites
            .into_iter()
            .map(|sprite| CookedSpriteAnimationFrame2D {
                sprite_asset_id: sprite.to_string(),
                duration_ticks: 2,
            })
            .collect(),
    }
}

fn transition(id: &str, from_state_index: u32, to_state_index: u32) -> CookedAnimator2DTransition {
    CookedAnimator2DTransition {
        id: id.to_string(),
        from_state_index,
        to_state_index,
        timing: Animator2DTransitionTiming::Immediate,
        priority: 10,
        conditions: vec![CookedAnimator2DCondition::Triggered { parameter_index: 0 }],
    }
}

fn sprite(world: &World) -> &str {
    world
        .sprite_renderer2d(&EntityId::from("animated"))
        .unwrap()
        .sprite_ref
        .as_deref()
        .unwrap()
}
