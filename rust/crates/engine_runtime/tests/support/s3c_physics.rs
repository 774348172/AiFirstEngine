use engine_runtime::{
    ids::EntityId, physics_3d::*, project_runtime_session::ProjectRuntimeMutationBuffer,
    scene_loader::load_scene_into_world, spatial_contract::*, world::World,
};
use project_runtime_sdk::ProjectRuntimeSpatialIntent as I;
use serde_json::{json, Value};

#[test]
#[ignore = "explicit original BM06 package; scaled body/CCD/joint capacity and controlled exhaustion"]
fn s5d_original_bm06_scaled_physics_capacity_refuses_exhaustion_and_recovers() {
    use engine_runtime::{
        runtime_instance_loader::RuntimeInstanceLoader, runtime_package::load_runtime_package,
    };
    use std::collections::BTreeMap;
    let package = load_runtime_package(std::path::PathBuf::from(
        std::env::var_os("AIFE_BM06_LOAD_PACKAGE").unwrap(),
    ))
    .value
    .unwrap();
    let mut classes = BTreeMap::<String, Vec<_>>::new();
    for entity in &package.active_scene.entities {
        if let Some(body) = entity
            .components
            .iter()
            .find(|c| c.component_type == "engine.body3d")
        {
            if !matches!(body.data["motion"].as_str(), Some("dynamic" | "static")) {
                continue;
            }
            let joint = entity
                .components
                .iter()
                .any(|c| c.component_type == "engine.joint3d");
            classes
                .entry(format!(
                    "{}/{}/{}/{joint}",
                    body.data["motion"], body.data["shape"]["kind"], body.data["ccd"]
                ))
                .or_default()
                .push(entity.clone());
        }
    }
    assert_eq!(classes.values().map(Vec::len).sum::<usize>(), 3000);
    assert!(classes.values().all(|group| group.len() % 2 == 0));
    for numerator in [2usize, 3, 4, 8] {
        let mut scene = package.active_scene.clone();
        let mut extras = Vec::new();
        for layer in 0..(numerator - 2).div_ceil(2) {
            let full = (numerator - 2) - layer * 2 >= 2;
            let selected: Vec<_> = classes
                .values()
                .flat_map(|group| {
                    group
                        .iter()
                        .take(if full { group.len() } else { group.len() / 2 })
                })
                .collect();
            let names: BTreeMap<_, _> = selected
                .iter()
                .map(|e| (e.id.clone(), format!("pressure-{layer}-{}", e.id)))
                .collect();
            for original in selected {
                let mut entity = original.clone();
                entity.id = names[&original.id].clone();
                entity.name = entity.id.clone();
                assert!(entity.parent_id.is_none());
                let Some(RuntimeSpatial::World3D { transform, .. }) = &mut entity.spatial else {
                    panic!("original body must be World3D")
                };
                transform.translation[0] += 256. * (layer + 1) as f32;
                for component in &mut entity.components {
                    if component.component_type == "engine.joint3d" {
                        let other = component.data["other"]["entityRef"].as_str().unwrap();
                        if let Some(new) = names.get(other) {
                            component.data["other"]["entityRef"] = json!(new);
                        }
                    }
                }
                extras.push(EntityId::from(entity.id.clone()));
                scene.entities.push(entity);
            }
        }
        let bodies: Vec<_> = scene
            .entities
            .iter()
            .flat_map(|e| &e.components)
            .filter(|c| c.component_type == "engine.body3d")
            .map(|c| &c.data)
            .collect();
        assert_eq!(
            bodies.iter().filter(|d| d["motion"] == "dynamic").count(),
            1000 * numerator / 2
        );
        assert_eq!(
            bodies.iter().filter(|d| d["motion"] == "static").count(),
            2000 * numerator / 2
        );
        assert_eq!(
            bodies.iter().filter(|d| d["ccd"] == true).count(),
            100 * numerator / 2
        );
        assert_eq!(
            scene
                .entities
                .iter()
                .flat_map(|e| &e.components)
                .filter(|c| c.component_type == "engine.joint3d")
                .count(),
            100 * numerator / 2
        );
        let count = bodies.len();
        assert_eq!(count, 3000 * numerator / 2 + 1);
        let mut world = load_scene_into_world(&scene).value.unwrap();
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        let mut physics = PhysicsModule::default();
        world.begin_spatial_tick(1);
        let started = std::time::Instant::now();
        let result =
            physics.advance_with_assets(&mut world, loader.asset_loader_mut(), 1, 1. / 60.);
        if count <= 12000 {
            result.unwrap();
            assert_eq!(world.physics3d_frame().bodies.len(), count);
            assert_eq!(world.physics3d_frame().motors.len(), 2);
            assert!(world
                .physics3d_frame()
                .bodies
                .values()
                .all(|b| b.position.iter().all(|v| v.is_finite())));
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.field_path, "/capacity");
            assert!(error.message.contains("budget exceeded"));
            assert!(started.elapsed().as_secs() < 10);
            assert!(world.physics3d_frame().bodies.is_empty());
            for id in &extras {
                world.try_despawn_entity(id).unwrap();
            }
            world.finish_spatial_tick();
            world.begin_spatial_tick(2);
            physics
                .advance_with_assets(&mut world, loader.asset_loader_mut(), 2, 1. / 60.)
                .unwrap();
            assert_eq!(world.physics3d_frame().bodies.len(), 3001);
            assert_eq!(world.physics3d_frame().motors.len(), 2);
        }
        world.finish_spatial_tick();
        eprintln!("BM06 published pressure: factor={:.1}, requestedBodies={count}, actualCCD/joints={}, result={} in{:.3}s; separated capacity islands only, no K3/query/performance/fullBM12 claim",numerator as f64/2.,100*numerator/2,if count<=12000 {"accepted native Jolt"} else {"controlled refusal and original3001body recovery"},started.elapsed().as_secs_f64());
    }
}

pub(super) fn entity(id: &str, p: [f32; 3], components: Vec<Value>) -> Value {
    json!({"schemaVersion":"runtime-entity.v2","id":id,"name":id,"enabled":true,"parentId":null,"siblingOrder":0,"spatial":{"domain":"world3d","transform":{"translation":p,"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},"components":components})
}
pub(super) fn body(motion: &str, shape: Value) -> Value {
    json!({"componentType":"engine.body3d","data":{"schemaVersion":"body3d.v1","motion":motion,"shape":shape,"mass":1,"friction":0.5,"restitution":0,"ccd":true,"sensor":false,"allowSleeping":true}})
}
pub(super) fn motor() -> Value {
    json!({"componentType":"engine.motor3d","data":{"schemaVersion":"motor3d.v1","radius":0.35,"halfHeight":0.5,"maxSlopeDegrees":45,"stepHeight":0.25,"mass":70,"gravity":9.81}})
}
pub(super) fn world(entities: Vec<Value>) -> World {
    let scene = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Physics","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":entities});
    let RuntimeSceneDocument::Spatial(scene) =
        decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()
    else {
        unreachable!()
    };
    load_scene_into_world(&scene.into()).value.unwrap()
}
pub(super) fn command(world: &mut World, id: &str, intent: I) {
    let id = EntityId::from(id);
    let mut buffer = ProjectRuntimeMutationBuffer::default();
    buffer.spatial_command(id.clone(), world.entity_generation(&id).unwrap(), intent);
    buffer.prepare(world).unwrap().commit(world).unwrap();
}
pub(super) fn tick(module: &mut PhysicsModule, world: &mut World, tick: u64) {
    world.begin_spatial_tick(tick);
    module.advance(world, tick, 1. / 60.).unwrap();
    world.finish_spatial_tick();
}
pub(super) fn floor() -> Value {
    entity(
        "floor",
        [0., -0.5, 0.],
        vec![body(
            "static",
            json!({"kind":"box","halfExtents":[20,0.5,20]}),
        )],
    )
}

#[test]
fn root_body_scale_is_not_inferred_again_from_rotating_solver_pose() {
    let mut entities = vec![floor()];
    let q = [0.2f32, 0.3, 0.4, 0.8];
    let length = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    let q = q.map(|v| v / length);
    let authored = [
        (
            "capsule",
            [1.; 3],
            json!({"kind":"capsule","halfHeight":0.4,"radius":0.25}),
        ),
        ("sphere", [2.; 3], json!({"kind":"sphere","radius":0.25})),
        (
            "box",
            [1., 2., 0.5],
            json!({"kind":"box","halfExtents":[0.4,0.3,0.2]}),
        ),
    ];
    for (index, (id, scale, shape)) in authored.iter().enumerate() {
        let mut entity = entity(
            id,
            [index as f32 * 4. - 4., 3., 0.],
            vec![body("dynamic", shape.clone())],
        );
        entity["spatial"]["transform"]["rotationXyzw"] = json!(q);
        entity["spatial"]["transform"]["scale"] = json!(scale);
        entities.push(entity);
    }
    let mut world = world(entities);
    let mut module = PhysicsModule::default();
    for frame in 1..=240 {
        tick(&mut module, &mut world, frame);
        for (id, scale, _) in &authored {
            assert_eq!(
                world.transform3d(&EntityId::from(*id)).unwrap().scale,
                *scale,
                "physics changes position and rotation, never root scale: {id}, tick{frame}"
            );
        }
    }
    assert_eq!(module.body_count(), 4);
    assert!(world.physics3d_frame().bodies["capsule"].position[1] < 2.);
}

#[test]
fn high_level_body_commands_read_actual_state_and_dynamic_write_paths_reject_atomically() {
    let mut world = world(vec![
        floor(),
        entity(
            "ball",
            [0., 3., 0.],
            vec![body("dynamic", json!({"kind":"sphere","radius":0.5}))],
        ),
    ]);
    let mut module = PhysicsModule::default();
    tick(&mut module, &mut world, 1);
    let id = EntityId::from("ball");
    let generation = world.entity_generation(&id).unwrap();
    let handle = project_game_sdk::EntityHandle::new("ball", generation).unwrap();
    let mut sdk = project_game_sdk::DeferredMutationWriter::default();
    sdk.body_velocity3d(&handle, [1., 0., 0.]).unwrap();
    sdk.body_impulse3d(&handle, [0., 2., 0.]).unwrap();
    let mut batch = ProjectRuntimeMutationBuffer::default();
    for mutation in sdk.into_inner() {
        let project_game_sdk::DeferredMutation::Spatial {
            entity_id,
            generation,
            intent,
        } = mutation
        else {
            unreachable!()
        };
        batch.spatial_command(entity_id.into(), generation, intent);
    }
    batch.prepare(&world).unwrap().commit(&mut world).unwrap();
    tick(&mut module, &mut world, 2);
    let state = engine_runtime::world_api::WorldReadApi::new(&world)
        .read_component(
            &id,
            &engine_runtime::components::ComponentTypeId::new("engine.physics3d_state"),
        )
        .unwrap();
    let engine_runtime::archetype::ComponentValue::Dynamic { value, .. } = state else {
        unreachable!()
    };
    let engine_runtime::component_value::RuntimeValue::Object(fields) = value else {
        unreachable!()
    };
    assert_eq!(
        fields["tick"],
        engine_runtime::component_value::RuntimeValue::I64(2)
    );
    assert!(fields.contains_key("body"));
    let actual = world.physics3d_frame();
    assert!(actual.bodies["ball"].velocity[0] > 0.9 && actual.bodies["ball"].velocity[1] > 1.8);
    let transform = project_runtime_sdk::ProjectRuntimeTransform3D {
        translation: [10., 10., 0.],
        rotation_xyzw: [0., 0., 0., 1.],
        scale: [1.; 3],
    };
    let before = world.transform3d(&id).unwrap().clone();
    let mut direct = ProjectRuntimeMutationBuffer::default();
    direct.spatial_command(
        id.clone(),
        generation,
        I::SetLocalTransform {
            transform: transform.clone(),
        },
    );
    assert!(direct.prepare(&world).is_err());
    let runtime = world.runtime_id_for_source(&id).unwrap();
    let mut bulk = ProjectRuntimeMutationBuffer::default();
    bulk.spatial_local_transform(
        project_runtime_sdk::spatial_bulk::SpatialEntityKey {
            world_epoch: world.epoch(),
            index: runtime.index,
            generation: runtime.generation,
        },
        transform,
    );
    assert!(bulk.prepare(&world).is_err());
    assert_eq!(&before, world.transform3d(&id).unwrap());
    world.try_despawn_entity(&id).unwrap();
    assert!(engine_runtime::world_api::WorldReadApi::new(&world)
        .read_component(
            &id,
            &engine_runtime::components::ComponentTypeId::new("engine.physics3d_state")
        )
        .is_err());
}

#[test]
fn motor_read_projection_rejects_a_retired_entity_generation_before_the_next_step() {
    let mut world = world(vec![
        floor(),
        entity("actor", [0., 0.85, 0.], vec![motor()]),
    ]);
    let mut module = PhysicsModule::default();
    tick(&mut module, &mut world, 1);
    let id = EntityId::from("actor");
    let generation = world.entity_generation(&id).unwrap();
    let ty = engine_runtime::components::ComponentTypeId::new("engine.physics3d_state");
    assert!(engine_runtime::world_api::WorldReadApi::new(&world)
        .read_component(&id, &ty)
        .is_ok());
    world.try_despawn_entity(&id).unwrap();
    world
        .try_spawn_entity(
            id.clone(),
            "new actor",
            "actor",
            true,
            engine_runtime::components::Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    assert_ne!(world.entity_generation(&id).unwrap(), generation);
    // Native synchronization occurs at the next fixed step. The last frame
    // still owns the old motor, but the replacement must not observe its data.
    assert!(engine_runtime::world_api::WorldReadApi::new(&world)
        .read_component(&id, &ty)
        .is_err());
    assert!(world.physics3d_frame().motors.contains_key("actor"));
}

#[test]
fn real_jolt_persists_contact_sleep_query_ccd_and_generation_checked_deferred_commands() {
    let mut world = world(vec![
        floor(),
        entity(
            "ball",
            [0., 3., 0.],
            vec![body("dynamic", json!({"kind":"sphere","radius":0.5}))],
        ),
    ]);
    let mut module = PhysicsModule::default();
    let mut added = 0;
    for frame in 1..=300 {
        tick(&mut module, &mut world, frame);
        added += world
            .physics3d_frame()
            .contacts
            .iter()
            .filter(|c| c.phase == "added")
            .count();
    }
    // Jolt 5.6.0's frozen PhysicsSettings use 20 mm penetration slop. This is
    // solver contact policy, separate from the 2 mm relative query precision.
    assert_eq!(module.solver_creation_count(), 1);
    assert_eq!(added, 1);
    let frame = world.physics3d_frame();
    assert!(
        (frame.bodies["ball"].position[1] - 0.48).abs() < 0.001,
        "{:?}",
        frame.bodies["ball"]
    );
    assert!(frame.bodies["ball"].sleeping);
    let ray = PhysicsRay3D {
        origin: [0., 5., 0.],
        displacement: [0., -10., 0.],
    };
    let hit = module.raycast(&ray).unwrap().unwrap();
    assert_eq!(hit.entity, "ball");
    assert!((f64::from(hit.distance) - (4.5 - frame.bodies["ball"].position[1])).abs() < 0.002);
    let sweep = module
        .sweep(
            &CollisionShape3D::Sphere { radius: 0.25 },
            &PhysicsRay3D {
                origin: [3., 5., 0.],
                displacement: [0., -10., 0.],
            },
            [0., 0., 0., 1.],
        )
        .unwrap()
        .unwrap();
    assert_eq!(sweep.entity, "floor");
    assert!((sweep.distance - 4.75).abs() < 0.01);
    command(
        &mut world,
        "ball",
        I::BodyImpulse {
            impulse: [0., 5., 0.],
        },
    );
    tick(&mut module, &mut world, 301);
    assert!(world.physics3d_frame().bodies["ball"].velocity[1] > 4.);
    let id = EntityId::from("ball");
    let mut stale = ProjectRuntimeMutationBuffer::default();
    stale.spatial_command(
        id.clone(),
        world.entity_generation(&id).unwrap() + 1,
        I::BodyVelocity { velocity: [0.; 3] },
    );
    assert!(stale.prepare(&world).is_err());
    // No new control revision may replay the old impulse on the next tick.
    tick(&mut module, &mut world, 302);
    assert!(world.physics3d_frame().bodies["ball"].velocity[1] < 5.);
    world.try_despawn_entity(&id).unwrap();
    tick(&mut module, &mut world, 303);
    assert_eq!(module.body_count(), 1);
    assert!(module
        .raycast(&PhysicsRay3D {
            origin: [0., 5., 0.],
            displacement: [0., -10., 0.]
        })
        .unwrap()
        .is_some_and(|h| h.entity == "floor"));
    assert!(frame.bodies.contains_key("ball"));
}

#[test]
fn real_motor_gravity_wall_block_accepted_motion_and_jump() {
    let mut world = world(vec![
        floor(),
        entity(
            "wall",
            [2., 1., 0.],
            vec![body(
                "static",
                json!({"kind":"box","halfExtents":[0.2,1,5]}),
            )],
        ),
        entity("actor", [0., 0.9, 0.], vec![motor()]),
    ]);
    let mut module = PhysicsModule::default();
    for frame in 1..=20 {
        tick(&mut module, &mut world, frame);
    }
    assert!(world.physics3d_frame().motors["actor"].grounded);
    for frame in 21..=80 {
        command(
            &mut world,
            "actor",
            I::MotorMove {
                displacement: [0.05, 0., 0.],
            },
        );
        tick(&mut module, &mut world, frame);
    }
    let p = world
        .transform3d(&EntityId::from("actor"))
        .unwrap()
        .translation;
    assert!(p[0] > 1. && p[0] < 1.48, "{p:?}");
    let result = &world.physics3d_frame().motors["actor"];
    assert!(result.accepted[0] < result.requested[0] * 0.1, "{result:?}");
    command(&mut world, "actor", I::MotorJump { speed: 4. });
    tick(&mut module, &mut world, 81);
    assert!(!world.physics3d_frame().motors["actor"].grounded);
    assert!(world.physics3d_frame().motors["actor"].accepted[1] > 0.03);
}

#[test]
fn backend_rebase_moves_motors_exactly_once_and_preserves_ground_contacts() {
    let mut world = world(vec![floor(), entity("actor", [2., 0.9, 0.], vec![motor()])]);
    let mut module = PhysicsModule::default();
    for frame in 1..=20 {
        tick(&mut module, &mut world, frame);
    }
    let actor = EntityId::from("actor");
    let before = world.transform3d(&actor).unwrap().translation;
    assert!(world.physics3d_frame().motors["actor"].grounded);
    for step in 1..=20 {
        module.rebase([f64::from(step) * 16., 0., 0.]).unwrap();
        tick(&mut module, &mut world, 20 + step as u64);
        let after = world.transform3d(&actor).unwrap().translation;
        for axis in 0..3 {
            assert!(
                (after[axis] - before[axis]).abs() < 0.002,
                "step {step}, axis {axis}: {before:?} -> {after:?}"
            );
        }
        assert!(world.physics3d_frame().motors["actor"].grounded);
    }
    assert_eq!(module.solver_creation_count(), 1);
}

#[test]
fn backend_rebase_preserves_absolute_pose_queries_and_solver_state() {
    let mut world = world(vec![
        floor(),
        entity(
            "ball",
            [0., 3., 0.],
            vec![body("dynamic", json!({"kind":"sphere","radius":0.5}))],
        ),
    ]);
    let mut module = PhysicsModule::default();
    tick(&mut module, &mut world, 1);
    let before = world.physics3d_frame().bodies["ball"].position;
    let ray = PhysicsRay3D {
        origin: [0., 5., 0.],
        displacement: [0., -10., 0.],
    };
    let hit = module.raycast(&ray).unwrap().unwrap();
    module.rebase([2048., 0., 0.]).unwrap();
    let after = module.raycast(&ray).unwrap().unwrap();
    assert!((hit.distance - after.distance).abs() < 0.002);
    tick(&mut module, &mut world, 2);
    assert!((world.physics3d_frame().bodies["ball"].position[0] - before[0]).abs() < 0.002);
    assert_eq!(module.solver_creation_count(), 1);
    assert!(module.rebase([1e12, 0., 0.]).is_err());
}

#[test]
fn physics_contract_rejects_bad_shapes_dynamic_mesh_and_nonfinite_intents() {
    assert!(CollisionShape3D::Convex {
        vertices: vec![[0.; 3]; 3]
    }
    .validate()
    .is_err());
    assert!(CollisionShape3D::Sphere { radius: f32::NAN }
        .validate()
        .is_err());
    let bad:Body3D=serde_json::from_value(json!({"schemaVersion":"body3d.v1","motion":"dynamic","shape":{"kind":"staticMesh","vertices":[[0,0,0],[1,0,0],[0,0,1]],"triangles":[[0,1,2]]}})).unwrap();
    assert!(bad.validate().is_err());
    let world = world(vec![floor()]);
    let id = EntityId::from("floor");
    let mut buffer = ProjectRuntimeMutationBuffer::default();
    buffer.spatial_command(
        id.clone(),
        world.entity_generation(&id).unwrap(),
        I::BodyImpulse {
            impulse: [1., 0., 0.],
        },
    );
    assert!(buffer.prepare(&world).is_err());
    assert!(PhysicsRay3D {
        origin: [0.; 3],
        displacement: [0.; 3]
    }
    .validate()
    .is_err());
}

#[test]
fn joint_lifecycle_overlap_sorted_and_disabled_body_retirement() {
    let mut anchor = entity(
        "anchor",
        [0., 4., 0.],
        vec![body(
            "static",
            json!({"kind":"box","halfExtents":[0.1,0.1,0.1]}),
        )],
    );
    let mut cube = entity(
        "cube",
        [0., 3., 0.],
        vec![
            body("dynamic", json!({"kind":"box","halfExtents":[0.2,0.2,0.2]})),
            json!({"componentType":"engine.joint3d","data":{"schemaVersion":"joint3d.v1","kind":"fixed","other":{"entityRef":"anchor"},"pivot":[0,1,0]}}),
        ],
    );
    let mut w = world(vec![floor(), anchor.clone(), cube.clone()]);
    let mut p = PhysicsModule::default();
    for t in 1..=120 {
        tick(&mut p, &mut w, t);
    }
    assert_eq!(p.joint_count(), 1);
    assert!((w.physics3d_frame().bodies["cube"].position[1] - 3.).abs() < 0.002);
    let overlaps = p
        .overlap(
            &CollisionShape3D::Box {
                half_extents: [1., 2., 1.],
            },
            [0., 3., 0.],
            [0., 0., 0., 1.],
        )
        .unwrap();
    assert_eq!(
        overlaps
            .iter()
            .map(|h| h.entity.as_str())
            .collect::<Vec<_>>(),
        vec!["anchor", "cube"]
    );
    w.try_despawn_entity(&EntityId::from("cube")).unwrap();
    tick(&mut p, &mut w, 121);
    assert_eq!(p.joint_count(), 0);
    assert_eq!(p.body_count(), 2);
    anchor["enabled"] = json!(false);
    cube["enabled"] = json!(false);
    let mut disabled = world(vec![floor(), anchor, cube]);
    let mut dp = PhysicsModule::default();
    tick(&mut dp, &mut disabled, 1);
    assert_eq!(dp.body_count(), 1);
    assert_eq!(dp.joint_count(), 0);
}

#[test]
fn actual_low_ceiling_blocks_jump_and_ccd_blocks_thin_wall() {
    let mut w = world(vec![
        floor(),
        entity(
            "ceiling",
            [0., 2.0, 0.],
            vec![body(
                "static",
                json!({"kind":"box","halfExtents":[2,0.1,2]}),
            )],
        ),
        entity("actor", [0., 0.9, 0.], vec![motor()]),
    ]);
    let mut p = PhysicsModule::default();
    for t in 1..=20 {
        tick(&mut p, &mut w, t);
    }
    command(&mut w, "actor", I::MotorJump { speed: 8. });
    let mut high = 0.;
    for t in 21..=60 {
        tick(&mut p, &mut w, t);
        high = f32::max(
            high,
            w.transform3d(&EntityId::from("actor")).unwrap().translation[1],
        );
    }
    assert!(high > 0.9 && high < 1.07, "{high}");
    let mut w = world(vec![
        entity(
            "wall",
            [0., 0., 0.],
            vec![body(
                "static",
                json!({"kind":"box","halfExtents":[0.01,10,10]}),
            )],
        ),
        entity(
            "bullet",
            [-2., 0., 0.],
            vec![body("dynamic", json!({"kind":"sphere","radius":0.1}))],
        ),
    ]);
    let mut p = PhysicsModule::default();
    command(
        &mut w,
        "bullet",
        I::BodyVelocity {
            velocity: [300., 0., 0.],
        },
    );
    tick(&mut p, &mut w, 1);
    assert!(w.physics3d_frame().bodies["bullet"].position[0] < 0.11);
}

#[test]
fn motor_climbs_authored_step_and_follows_moving_platform() {
    let mut w = world(vec![
        floor(),
        entity(
            "step",
            [1.5, 0.09, 0.],
            vec![body(
                "static",
                json!({"kind":"box","halfExtents":[0.7,0.09,2]}),
            )],
        ),
        entity("actor", [0., 0.9, 0.], vec![motor()]),
    ]);
    let mut p = PhysicsModule::default();
    for t in 1..=20 {
        tick(&mut p, &mut w, t);
    }
    for t in 21..=60 {
        command(
            &mut w,
            "actor",
            I::MotorMove {
                displacement: [0.04, 0., 0.],
            },
        );
        tick(&mut p, &mut w, t);
    }
    let pos = w.transform3d(&EntityId::from("actor")).unwrap().translation;
    assert!(pos[0] > 1.4 && pos[1] > 0.96, "{pos:?}");
    let mut w = world(vec![
        entity(
            "platform",
            [0., -0.25, 0.],
            vec![body(
                "kinematic",
                json!({"kind":"box","halfExtents":[3,0.25,3]}),
            )],
        ),
        entity("actor", [0., 0.9, 0.], vec![motor()]),
    ]);
    let mut p = PhysicsModule::default();
    for t in 1..=30 {
        tick(&mut p, &mut w, t);
    }
    for t in 31..=90 {
        let x = (t - 30) as f32 * 0.01;
        command(
            &mut w,
            "platform",
            I::SetLocalTransform {
                transform: project_runtime_sdk::ProjectRuntimeTransform3D {
                    translation: [x, -0.25, 0.],
                    rotation_xyzw: [0., 0., 0., 1.],
                    scale: [1.; 3],
                },
            },
        );
        tick(&mut p, &mut w, t);
    }
    let pos = w.transform3d(&EntityId::from("actor")).unwrap().translation;
    assert!(pos[0] > 0.45 && pos[0] < 0.7, "{pos:?}");
}

#[test]
fn sensor_contact_and_far_anchor_queries_match_zero_anchor_reference() {
    let mut sensor = body("static", json!({"kind":"box","halfExtents":[1,1,1]}));
    sensor["data"]["sensor"] = json!(true);
    sensor["data"]["ccd"] = json!(false);
    let mut reference = None;
    for anchor in [0., 1_000_000., 1_000_000_000.] {
        let mut w = world(vec![
            entity("trigger", [0., 0., 0.], vec![sensor.clone()]),
            entity(
                "ball",
                [0., 0., 0.],
                vec![body("dynamic", json!({"kind":"sphere","radius":0.2}))],
            ),
        ]);
        for id in ["trigger", "ball"] {
            command(
                &mut w,
                id,
                I::SetWorldAnchor {
                    anchor: [anchor, 0., 0.],
                    preserve_world: false,
                },
            );
        }
        let mut p = PhysicsModule::default();
        tick(&mut p, &mut w, 1);
        assert!(w
            .physics3d_frame()
            .contacts
            .iter()
            .any(|c| c.sensor && c.phase == "added"));
        let hit = p
            .raycast(&PhysicsRay3D {
                origin: [anchor, 5., 0.],
                displacement: [0., -10., 0.],
            })
            .unwrap()
            .unwrap();
        let d = hit.distance;
        if let Some(r) = reference {
            assert!((d - r as f32).abs() < 0.002);
        } else {
            reference = Some(d);
        }
        assert_eq!(p.origin(), [anchor, 0., 0.]);
    }
}

#[test]
fn motor_accepts_walkable_ramp_and_rejects_steep_ramp() {
    let run = |height: f32| {
        let ramp = entity(
            "ramp",
            [0.; 3],
            vec![body(
                "static",
                json!({"kind":"staticMesh","vertices":[[0.,0.,-2.],[0.,0.,2.],[4.,height,2.],[4.,height,-2.]],"triangles":[[0,1,2],[0,2,3]]}),
            )],
        );
        let mut w = world(vec![
            floor(),
            ramp,
            entity("actor", [-1., 0.9, 0.], vec![motor()]),
        ]);
        let mut p = PhysicsModule::default();
        for t in 1..=120 {
            command(
                &mut w,
                "actor",
                I::MotorMove {
                    displacement: [0.04, 0., 0.],
                },
            );
            tick(&mut p, &mut w, t);
        }
        w.transform3d(&EntityId::from("actor")).unwrap().translation
    };
    let shallow = run(2.);
    let steep = run(6.);
    // Horizontal requested motion is projected onto the 26.6-degree surface.
    assert!(shallow[0] > 2.5 && shallow[1] > 2., "{shallow:?}");
    assert!(steep[0] < 1. && steep[1] < 2., "{steep:?}");
}

#[test]
fn bm10_k2_joint_under_translated_parent_keeps_authored_shape_and_constraint() {
    let prefab: Value = serde_json::from_slice(
        &std::fs::read("samples/platform_3d_bm10_k2_project/Prefabs/K2/bm10-k2-0.prefab.json")
            .unwrap(),
    )
    .unwrap();
    for parented in [true, false] {
        let mut entities = vec![floor(), entity("placement", [12., 0., 0.], vec![])];
        let mut authored = Vec::new();
        for id in ["dynamic-0000", "dynamic-0100"] {
            let source = prefab["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"] == id)
                .unwrap();
            let transform = source["components"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["componentType"] == "Transform3D")
                .unwrap()["data"]
                .clone();
            let components = source["components"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| {
                    matches!(
                        c["componentType"].as_str(),
                        Some("engine.body3d" | "engine.joint3d")
                    )
                })
                .cloned()
                .collect();
            let p: [f32; 3] = serde_json::from_value(transform["translation"].clone()).unwrap();
            let mut e = entity(id, [p[0], p[1] + 5., p[2] + 28.], components);
            if parented {
                e["parentId"] = json!("placement");
            }
            entities.push(e);
            authored.push(id);
        }
        let mut w = world(entities);
        let mut physics = PhysicsModule::default();
        let mut first_scale = None;
        for frame in 1..=2000u64 {
            if frame > 600 {
                for (id, index) in [("dynamic-0000", 0.), ("dynamic-0100", 100.)] {
                    let phase = (frame as f32 + index * 17.) * 0.025;
                    command(
                        &mut w,
                        id,
                        I::BodyVelocity {
                            velocity: [phase.sin() * 1.2, phase.cos() * 0.35, 0.4],
                        },
                    );
                }
            }
            w.begin_spatial_tick(frame);
            let result = physics.advance(&mut w, frame, 1. / 60.);
            if let Err(ref error) = result {
                eprintln!("parented={parented} frame={frame} first_scale={first_scale:?} actual={:?} error={error:?}",w.physics3d_frame().bodies);
            }
            result.unwrap();
            w.finish_spatial_tick();
            for id in &authored {
                let scale = w.transform3d(&EntityId::from(*id)).unwrap().scale;
                if scale != [1.; 3] && first_scale.is_none() {
                    first_scale = Some((frame, scale));
                }
            }
        }
        eprintln!(
            "parented={parented} first_scale={first_scale:?} joints={} actual={:?}",
            physics.joint_count(),
            w.physics3d_frame().bodies
        );
        assert_eq!(
            first_scale, None,
            "native pose output must preserve authored shape scale under translation-only parent"
        );
        assert_eq!(physics.joint_count(), 1);
    }
}

#[test]
fn joint_backend_rejection_keeps_body_identity_and_actual_positions() {
    let mut w = world(vec![
        entity(
            "owner",
            [0., 1., 0.],
            vec![
                body("dynamic", json!({"kind":"box","halfExtents":[0.4,0.4,0.4]})),
                json!({"componentType":"engine.joint3d","data":{"schemaVersion":"joint3d.v1","kind":"point","other":{"entityRef":"other"},"pivot":[0,0,0]}}),
            ],
        ),
        entity(
            "other",
            [12., 1., 0.],
            vec![body(
                "dynamic",
                json!({"kind":"box","halfExtents":[0.4,0.4,0.4]}),
            )],
        ),
    ]);
    let mut physics = PhysicsModule::default();
    w.begin_spatial_tick(1);
    let error = physics.advance(&mut w, 1, 1. / 60.).unwrap_err();
    assert_eq!(error.field_path, "/backend");
    for expected in [
        "lever-arm ratio",
        "owner=owner",
        "other=other",
        "owner_position=[0.0, 1.0, 0.0]",
        "other_position=[12.0, 1.0, 0.0]",
    ] {
        assert!(error.message.contains(expected), "{error:?}");
    }
}
