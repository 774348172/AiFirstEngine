use super::*;
use crate::spatial_contract::Transform3D;

fn body(shape: CollisionShape3D) -> Body3D {
    Body3D {
        enabled: true,
        schema_version: "body3d.v1".into(),
        motion: BodyMotion3D::Static,
        shape,
        mass: 1.,
        friction: 0.7,
        restitution: 0.1,
        ccd: false,
        sensor: false,
        allow_sleeping: true,
    }
}
fn mesh(y: f32) -> Body3D {
    body(CollisionShape3D::StaticMesh {
        vertices: vec![[-3., y, -3.], [3., y, -3.], [0., y, 3.]],
        triangles: vec![[0, 2, 1]],
    })
}
fn spawn(world: &mut World, id: &str, y: f32) {
    let mut transform = Transform3D::identity();
    transform.translation[1] = y;
    world.commit_prepared_entity(
        id.into(),
        id.into(),
        "spatial".into(),
        true,
        crate::components::Hierarchy {
            parent_id: None,
            sibling_order: 0,
        },
        vec![ComponentValue::Transform3D(
            transform.canonicalize().unwrap(),
        )],
    );
}
fn fixture() -> (World, PhysicsModule, RuntimeAssetLoader) {
    let mut world = World::new();
    spawn(&mut world, "floor", 0.);
    world.insert_component_value(
        "floor".into(),
        body_value(&body(CollisionShape3D::Box {
            half_extents: [2., 0.5, 2.],
        })),
    );
    let mut module = PhysicsModule::default();
    module.advance(&mut world, 1, 1. / 60.).unwrap();
    let loader = RuntimeAssetLoader::new(
        "unused",
        crate::runtime_asset::RuntimeAssetIndex::new(vec![], vec![]),
        crate::runtime_asset::RuntimePackageMountTable::new(vec![]),
    );
    (world, module, loader)
}
fn ray(module: &PhysicsModule) -> PhysicsHit3D {
    module
        .raycast(&PhysicsRay3D {
            origin: [0., 4., 0.],
            displacement: [0., -8., 0.],
        })
        .unwrap()
        .unwrap()
}

#[test]
fn generated_content_376_collision_native_prepared_is_invisible_and_existing_static_replacement_changes_real_queries(
) {
    let (mut world, mut module, mut loader) = fixture();
    let id: EntityId = "floor".into();
    let old_handle = module.bodies[&id].handle;
    let before = world.physics3d_frame();
    let p = module
        .prepare_static_collision(&world, &id, Some(mesh(1.)), &mut loader)
        .unwrap();
    assert_eq!(module.prepared_capacity.load(Ordering::Relaxed), 1);
    assert_eq!(module.solver.as_ref().unwrap().body_count(), 1);
    assert_eq!(world.physics3d_frame(), before);
    assert!(
        (ray(&module).position[1] - 0.5).abs() < 1e-4,
        "Prepared shape entered live query"
    );
    let capsule = CollisionShape3D::Capsule {
        half_height: 0.3,
        radius: 0.2,
    };
    let query = PhysicsRay3D {
        origin: [0., 4., 0.],
        displacement: [0., -8., 0.],
    };
    let sweep_before = module
        .sweep(&capsule, &query, [0., 0., 0., 1.])
        .unwrap()
        .unwrap();
    let binding = module
        .commit_static_collision(&mut world, p)
        .unwrap()
        .unwrap();
    assert_ne!(binding.handle, Some(old_handle));
    assert!(!module.solver.as_ref().unwrap().contains_body(old_handle));
    assert_eq!(module.solver.as_ref().unwrap().body_count(), 1);
    assert_eq!(
        module.solver_creation_count(),
        1,
        "replacement created another solver"
    );
    assert_eq!(module.prepared_capacity.load(Ordering::Relaxed), 0);
    assert!((ray(&module).position[1] - 1.).abs() < 1e-4);
    let sweep_after = module
        .sweep(&capsule, &query, [0., 0., 0., 1.])
        .unwrap()
        .unwrap();
    assert!(sweep_after.fraction < sweep_before.fraction);
    assert_eq!(
        ray(&module).generation,
        world.entity_generation(&id).unwrap()
    );
    module.advance(&mut world, 2, 1. / 60.).unwrap();
    assert_eq!(
        module.bodies[&id].handle,
        binding.handle.unwrap(),
        "normal simulation recreated committed native shape"
    );
}

#[test]
fn generated_content_376_collision_native_post_activation_conflict_rolls_back_real_candidate() {
    let (mut world, mut module, mut loader) = fixture();
    let id: EntityId = "floor".into();
    let old = module.bodies[&id].handle;
    let p = module
        .prepare_static_collision(&world, &id, Some(mesh(1.)), &mut loader)
        .unwrap();
    let active = module.activate_static_collision(&world, &p).unwrap();
    let candidate = active.handle.unwrap();
    assert_eq!(module.solver.as_ref().unwrap().body_count(), 2);
    let external = body(CollisionShape3D::Sphere { radius: 0.8 });
    world.insert_component_value(id.clone(), body_value(&external));
    let error = module
        .finish_static_collision(&mut world, p, active)
        .unwrap_err();
    assert_eq!(error.field_path, "/generated/identity");
    assert!(module.solver.as_ref().unwrap().contains_body(old));
    assert!(!module.solver.as_ref().unwrap().contains_body(candidate));
    assert_eq!(module.solver.as_ref().unwrap().body_count(), 1);
    assert_eq!(module.prepared_capacity.load(Ordering::Relaxed), 0);
    assert_eq!(
        world.component_value(&id, &ComponentTypeId::new("engine.body3d")),
        Some(body_value(&external))
    );
    assert!((ray(&module).position[1] - 0.5).abs() < 1e-4);
}

#[test]
fn generated_content_376_collision_native_failure_pose_owner_capacity_and_unsupported_endpoints_preserve_old_body(
) {
    let (mut world, mut module, mut loader) = fixture();
    let id: EntityId = "floor".into();
    let old = module.bodies[&id].handle;
    // Structurally valid point data cannot form an actual Jolt convex hull.
    assert!(module
        .prepare_static_collision(
            &world,
            &id,
            Some(body(CollisionShape3D::Convex {
                vertices: vec![[0.; 3]; 4]
            })),
            &mut loader
        )
        .is_err());
    assert_eq!(module.bodies[&id].handle, old);
    module.prepared_capacity.store(11999, Ordering::Relaxed);
    assert_eq!(
        module
            .prepare_static_collision(&world, &id, Some(mesh(1.)), &mut loader)
            .unwrap_err()
            .field_path,
        "/generated/capacity"
    );
    assert_eq!(module.prepared_capacity.swap(0, Ordering::Relaxed), 11999);
    let p = module
        .prepare_static_collision(&world, &id, Some(mesh(1.)), &mut loader)
        .unwrap();
    let mut foreign = PhysicsModule::default();
    assert!(foreign.commit_static_collision(&mut world, p).is_err());
    assert_eq!(module.prepared_capacity.load(Ordering::Relaxed), 0);
    let p = module
        .prepare_static_collision(&world, &id, Some(mesh(1.)), &mut loader)
        .unwrap();
    let mut moved = Transform3D::identity();
    moved.translation[0] = 1.;
    world.insert_component_value(
        id.clone(),
        ComponentValue::Transform3D(moved.canonicalize().unwrap()),
    );
    world.propagate_spatial();
    assert_eq!(
        module
            .commit_static_collision(&mut world, p)
            .unwrap_err()
            .field_path,
        "/generated/pose"
    );
    world.insert_component_value(
        id.clone(),
        ComponentValue::Transform3D(Transform3D::identity().canonicalize().unwrap()),
    );
    world.propagate_spatial();
    for motion in [BodyMotion3D::Dynamic, BodyMotion3D::Kinematic] {
        let mut d = body(CollisionShape3D::Sphere { radius: 1. });
        d.motion = motion;
        world.insert_component_value(id.clone(), body_value(&d));
        assert_eq!(
            module
                .prepare_static_collision(&world, &id, None, &mut loader)
                .unwrap_err()
                .field_path,
            "/generated/motion"
        );
    }
    world.insert_component_value(
        id.clone(),
        body_value(&body(CollisionShape3D::Box {
            half_extents: [2., 0.5, 2.],
        })),
    );
    world.insert_dynamic_component(
        id.clone(),
        ComponentTypeId::new("engine.motor3d"),
        crate::character_animation::runtime_json(serde_json::json!({})),
    );
    assert_eq!(
        module
            .prepare_static_collision(&world, &id, None, &mut loader)
            .unwrap_err()
            .field_path,
        "/generated/motion"
    );
    world.remove_component_value(&id, &ComponentTypeId::new("engine.motor3d"));
    spawn(&mut world, "joint-owner", 0.);
    world.insert_dynamic_component(
        "joint-owner".into(),
        ComponentTypeId::new("engine.joint3d"),
        crate::character_animation::runtime_json(
            serde_json::json!({"other":{"entityRef":"floor"}}),
        ),
    );
    assert_eq!(
        module
            .prepare_static_collision(&world, &id, None, &mut loader)
            .unwrap_err()
            .field_path,
        "/generated/motion"
    );
    assert!(module.solver.as_ref().unwrap().contains_body(old));
    assert_eq!(module.prepared_capacity.load(Ordering::Relaxed), 0);
}

#[test]
fn generated_content_376_collision_native_motor_consumes_new_static_mesh_after_prepared_isolation()
{
    let (mut world, mut module, mut loader) = fixture();
    spawn(&mut world, "walker", 3.);
    world.insert_dynamic_component(
        "walker".into(),
        ComponentTypeId::new("engine.motor3d"),
        crate::character_animation::runtime_json(serde_json::json!({
            "schemaVersion":"motor3d.v1", "radius":0.3,"halfHeight":0.4,
            "maxSlopeDegrees":45,"stepHeight":0.3,"mass":80,"gravity":9.81
        })),
    );
    for tick in 2..=120 {
        module.advance(&mut world, tick, 1. / 60.).unwrap();
    }
    let old_y = world.transform3d(&"walker".into()).unwrap().translation[1];
    let p = module
        .prepare_static_collision(&world, &"floor".into(), Some(mesh(-1.)), &mut loader)
        .unwrap();
    for tick in 121..=130 {
        module.advance(&mut world, tick, 1. / 60.).unwrap();
    }
    assert!((world.transform3d(&"walker".into()).unwrap().translation[1] - old_y).abs() < 0.02);
    module.commit_static_collision(&mut world, p).unwrap();
    for tick in 131..=260 {
        module.advance(&mut world, tick, 1. / 60.).unwrap();
    }
    let frame = world.physics3d_frame();
    assert!(
        frame.motors["walker"].grounded,
        "{:?}",
        frame.motors["walker"]
    );
    let new_y = world.transform3d(&"walker".into()).unwrap().translation[1];
    assert!(
        (new_y - old_y + 1.5).abs() < 0.03,
        "old={old_y}, new={new_y}, motor={:?}",
        frame.motors["walker"]
    );
    assert_eq!(module.solver_creation_count(), 1);
}

#[test]
fn generated_content_376_collision_native_retirement_protects_external_owner_and_new_incarnation() {
    let (mut world, mut module, mut loader) = fixture();
    let id: EntityId = "floor".into();
    let p = module
        .prepare_static_collision(&world, &id, Some(mesh(1.)), &mut loader)
        .unwrap();
    let binding = module
        .commit_static_collision(&mut world, p)
        .unwrap()
        .unwrap();
    let external = body(CollisionShape3D::Sphere { radius: 0.8 });
    world.insert_component_value(id.clone(), body_value(&external));
    module
        .retire_generated_static(&mut world, &binding)
        .unwrap();
    assert_eq!(
        world.component_value(&id, &ComponentTypeId::new("engine.body3d")),
        Some(body_value(&external))
    );
    module.advance(&mut world, 2, 1. / 60.).unwrap();
    assert_eq!(module.body_count(), 1);
    module
        .retire_generated_static(&mut world, &binding)
        .unwrap();
    assert_eq!(module.body_count(), 1);
    let p = module
        .prepare_static_collision(&world, &id, Some(mesh(1.)), &mut loader)
        .unwrap();
    let binding = module
        .commit_static_collision(&mut world, p)
        .unwrap()
        .unwrap();
    world.despawn_entity(&id).unwrap();
    spawn(&mut world, "floor", 0.);
    world.insert_component_value(id.clone(), body_value(&external));
    module
        .retire_generated_static(&mut world, &binding)
        .unwrap();
    assert_eq!(
        world.component_value(&id, &ComponentTypeId::new("engine.body3d")),
        Some(body_value(&external))
    );
    module.advance(&mut world, 3, 1. / 60.).unwrap();
    assert_eq!(module.body_count(), 1);
}

#[test]
fn generated_content_376_collision_native_old_contacts_cannot_map_to_replacement_and_disabled_body_stays_inactive(
) {
    let (mut world, mut module, mut loader) = fixture();
    spawn(&mut world, "ball", 1.);
    let mut ball = body(CollisionShape3D::Sphere { radius: 0.5 });
    ball.motion = BodyMotion3D::Dynamic;
    world.insert_component_value("ball".into(), body_value(&ball));
    let mut touched = false;
    for tick in 2..=45 {
        module.advance(&mut world, tick, 1. / 60.).unwrap();
        touched |= world
            .physics3d_frame()
            .contacts
            .iter()
            .any(|c| c.entity1 == "floor" || c.entity2 == "floor");
    }
    assert!(touched, "fixture never produced a real contact");
    let old = module.bodies[&EntityId::from("floor")].handle;
    let p = module
        .prepare_static_collision(&world, &"floor".into(), Some(mesh(-1.)), &mut loader)
        .unwrap();
    module.commit_static_collision(&mut world, p).unwrap();
    assert!(world.physics3d_frame().contacts.is_empty());
    module.advance(&mut world, 46, 1. / 60.).unwrap();
    assert!(!module.solver.as_ref().unwrap().contains_body(old));
    assert!(
        !world
            .physics3d_frame()
            .contacts
            .iter()
            .any(|c| c.entity1 == "floor" || c.entity2 == "floor"),
        "old-handle removal event leaked onto replacement"
    );
    let mut grounded_on_new = false;
    for tick in 47..=180 {
        module.advance(&mut world, tick, 1. / 60.).unwrap();
        grounded_on_new |= world
            .physics3d_frame()
            .contacts
            .iter()
            .any(|c| c.entity1 == "floor" || c.entity2 == "floor");
    }
    assert!(grounded_on_new);
    let mut disabled = mesh(-1.);
    disabled.enabled = false;
    let p = module
        .prepare_static_collision(&world, &"floor".into(), Some(disabled), &mut loader)
        .unwrap();
    let binding = module
        .commit_static_collision(&mut world, p)
        .unwrap()
        .unwrap();
    assert!(binding.handle.is_none());
    assert!(!module.bodies.contains_key(&EntityId::from("floor")));
    module.advance(&mut world, 181, 1. / 60.).unwrap();
    assert!(!world.physics3d_frame().bodies.contains_key("floor"));
    module
        .retire_generated_static(&mut world, &binding)
        .unwrap();
    assert!(world
        .component_value(&"floor".into(), &ComponentTypeId::new("engine.body3d"))
        .is_none());
    assert_eq!(module.prepared_capacity.load(Ordering::Relaxed), 0);
}
