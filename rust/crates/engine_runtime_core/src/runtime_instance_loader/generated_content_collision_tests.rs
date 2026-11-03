//! Original Host + persistent Jolt owner, with controlled render readiness for failure policy.
use super::*;
use crate::physics_3d::{Body3D, BodyMotion3D, CollisionShape3D};

pub(super) fn authored_body() -> Body3D {
    Body3D {
        enabled: true,
        schema_version: "body3d.v1".into(),
        motion: BodyMotion3D::Static,
        shape: CollisionShape3D::Box {
            half_extents: [0.5; 3],
        },
        mass: 1.,
        friction: 0.5,
        restitution: 0.,
        ccd: false,
        sensor: false,
        allow_sleeping: true,
    }
}
pub(super) fn set_body(world: &mut World, body: &Body3D) {
    world.insert_dynamic_component(
        "surface".into(),
        crate::components::ComponentTypeId::new("engine.body3d"),
        crate::character_animation::runtime_json(serde_json::to_value(body).unwrap()),
    );
}
fn body(world: &World) -> Option<Body3D> {
    match world.component_value(
        &"surface".into(),
        &crate::components::ComponentTypeId::new("engine.body3d"),
    ) {
        Some(ComponentValue::Dynamic { value, .. }) => {
            Some(serde_json::from_value(crate::character_animation::dynamic_json(&value)).unwrap())
        }
        _ => None,
    }
}
pub(super) fn mesh_collision() -> CollisionUpdate3D {
    let shape =
        StaticShape3D::triangles(&[[-2., 1., -2.], [2., 1., -2.], [0., 1., 2.]], &[[0, 2, 1]])
            .unwrap();
    CollisionUpdate3D::Replace {
        body: StaticBody3D::new(shape),
    }
}
pub(super) fn hit(
    host: &mut EngineHostLoop,
    world: &World,
) -> Option<project_runtime_sdk::physics_query3d::PhysicsQueryHit3D> {
    use project_runtime_sdk::physics_query3d::*;
    let result = host
        .generated_collision_query_for_test(
            world,
            &PhysicsQueryBatch3D {
                queries: vec![PhysicsQuery3D::Raycast {
                    origin: [0., 4., 0.],
                    displacement: [0., -8., 0.],
                }],
            },
        )
        .unwrap();
    assert_eq!(result.tick, Some(world.physics3d_frame().tick));
    result.hits.into_iter().next().unwrap()
}
fn collision_request(
    world: &World,
    id: u64,
    expected: u64,
    collision: CollisionUpdate3D,
) -> GeneratedContentRequest {
    GeneratedContentRequest {
        operation: GeneratedOperation3D::ReplaceCollision { collision },
        ..request(world, id, expected)
    }
}

#[test]
fn generated_content_376_collision_host_cancel_supersede_and_unsupported_motion_do_not_publish_candidates(
) {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let old = authored_body();
    set_body(&mut world, &old);
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let renderer = world.mesh_renderer3d(&"surface".into()).cloned();
    let make = |world: &World, id| {
        let mut r = request(world, id, 1);
        let GeneratedOperation3D::Replace { collision, .. } = &mut r.operation else {
            unreachable!()
        };
        *collision = mesh_collision();
        r
    };
    backend.ready = false;
    let cancelled = candidate(&mut loader, &world, make(&world, 2));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert!(loader.generated_content.records[&2]
        .prepared_collision
        .is_some());
    loader.cancel_generated_content(cancelled).unwrap();
    backend.ready = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, cancelled).status, ContentStatus::Cancelled);
    assert!(loader.generated_content.records[&2]
        .prepared_collision
        .is_none());
    assert_eq!(body(&world), Some(old.clone()));
    assert_eq!(world.mesh_renderer3d(&"surface".into()).cloned(), renderer);
    backend.ready = false;
    let stale = candidate(&mut loader, &world, make(&world, 3));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let current = candidate(&mut loader, &world, request(&world, 4, 1));
    backend.ready = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, stale).status, ContentStatus::Superseded);
    assert_eq!(receipt(&loader, current).status, ContentStatus::Applied);
    assert_eq!(receipt(&loader, current).collision_revision, 0);
    assert!((hit(&mut host, &world).unwrap().position[1] - 0.5).abs() < 1e-4);
    let mut dynamic = old;
    dynamic.motion = BodyMotion3D::Dynamic;
    set_body(&mut world, &dynamic);
    let unsupported = candidate(
        &mut loader,
        &world,
        collision_request(&world, 5, 4, CollisionUpdate3D::Clear),
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(
        receipt(&loader, unsupported).error.unwrap().code,
        ContentErrorCode::UnsupportedMotionType
    );
    assert_eq!(receipt(&loader, unsupported).applied_revision, 4);
    assert_eq!(body(&world).unwrap().motion, BodyMotion3D::Dynamic);
}

#[test]
fn generated_content_376_collision_host_combined_waits_then_replaces_existing_static_target() {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let old = authored_body();
    set_body(&mut world, &old);
    let mut backend = ControlledBackend::default();
    // Submit before the original first simulation has materialized the authored body.
    let mut replace = request(&world, 1, 0);
    let GeneratedOperation3D::Replace { collision, .. } = &mut replace.operation else {
        unreachable!()
    };
    *collision = mesh_collision();
    let ticket = candidate(&mut loader, &world, replace);
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(
        receipt(&loader, ticket).status,
        ContentStatus::Preparing,
        "old body must remain while native candidate waits for GPU"
    );
    assert_eq!(body(&world), Some(old));
    assert!((hit(&mut host, &world).unwrap().position[1] - 0.5).abs() < 1e-4);
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    backend.ready = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let applied = receipt(&loader, ticket);
    assert_eq!(
        applied.status,
        ContentStatus::Applied,
        "{:?}",
        applied.error
    );
    assert_eq!(applied.collision_revision, applied.candidate_revision);
    assert_eq!(applied.commit_epoch, Some(1));
    assert!(matches!(
        body(&world).unwrap().shape,
        CollisionShape3D::StaticMesh { .. }
    ));
    assert!(world.mesh_renderer3d(&"surface".into()).is_some());
    assert_eq!(world.physics3d_frame().bodies.len(), 1);
    assert!((hit(&mut host, &world).unwrap().position[1] - 1.).abs() < 1e-4);
}

#[test]
fn generated_content_376_collision_host_existing_shape_variants_body_fields_and_disabled_state_round_trip(
) {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    let shapes = [
        StaticShape3D::Box {
            half_extents: [0.6; 3],
        },
        StaticShape3D::Sphere { radius: 0.6 },
        StaticShape3D::Capsule {
            half_height: 0.5,
            radius: 0.2,
        },
        StaticShape3D::Convex {
            vertex_count: 4,
            vertex_bytes: [
                [-1f32, -1., -1.],
                [1., -1., -1.],
                [0., 1., -1.],
                [0., 0., 1.],
            ]
            .into_iter()
            .flatten()
            .flat_map(f32::to_le_bytes)
            .collect(),
        },
    ];
    for (index, shape) in shapes.into_iter().enumerate() {
        let id = index as u64 + 1;
        let mut descriptor = StaticBody3D::new(shape);
        descriptor.mass = 2.;
        descriptor.friction = 0.8;
        descriptor.restitution = 0.2;
        descriptor.allow_sleeping = false;
        let ticket = candidate(
            &mut loader,
            &world,
            collision_request(
                &world,
                id,
                id - 1,
                CollisionUpdate3D::Replace { body: descriptor },
            ),
        );
        tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
        assert_eq!(
            receipt(&loader, ticket).status,
            ContentStatus::Applied,
            "{:?}",
            receipt(&loader, ticket).error
        );
        let native = body(&world).unwrap();
        assert_eq!(native.schema_version, "body3d.v1");
        assert_eq!(native.motion, BodyMotion3D::Static);
        assert_eq!(
            (
                native.mass,
                native.friction,
                native.restitution,
                native.allow_sleeping
            ),
            (2., 0.8, 0.2, false)
        );
        assert!(hit(&mut host, &world).is_some());
        assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    }
    let mut disabled = StaticBody3D::new(StaticShape3D::Sphere { radius: 1. });
    disabled.enabled = false;
    let ticket = candidate(
        &mut loader,
        &world,
        collision_request(&world, 5, 4, CollisionUpdate3D::Replace { body: disabled }),
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Applied);
    assert!(!body(&world).unwrap().enabled);
    assert!(hit(&mut host, &world).is_none());
    loader.close_generated_content();
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert!(body(&world).is_none());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_collision_host_keep_replace_clear_and_release_preserve_independent_bindings_and_charges(
) {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    set_body(&mut world, &authored_body());
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    let first = candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, first).collision_revision, 0);
    let renderer = world.mesh_renderer3d(&"surface".into()).cloned();
    let geometries: Vec<_> = loader.generated_content.bindings[&(
        "surface".into(),
        world.entity_generation(&"surface".into()).unwrap(),
    )]
        .canonical
        .as_ref()
        .unwrap()
        .cpu_backings()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    let second = candidate(
        &mut loader,
        &world,
        collision_request(&world, 2, 1, mesh_collision()),
    );
    assert_eq!(
        loader.generated_content.records[&2].cpu_charge, 96,
        "byte + decoded collision peak must both be reserved"
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, second).collision_revision, 2);
    assert_eq!(world.mesh_renderer3d(&"surface".into()).cloned(), renderer);
    let binding = &loader.generated_content.bindings[&(
        "surface".into(),
        world.entity_generation(&"surface".into()).unwrap(),
    )];
    assert_eq!(binding.collision_cpu_charge, 48);
    assert_eq!(
        binding
            .canonical
            .as_ref()
            .unwrap()
            .cpu_backings()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect::<Vec<_>>(),
        geometries
    );
    assert!((hit(&mut host, &world).unwrap().position[1] - 1.).abs() < 1e-4);
    let third = candidate(&mut loader, &world, request(&world, 3, 2));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(
        receipt(&loader, third).collision_revision,
        2,
        "geometry Keep rewrote collision revision"
    );
    let renderer = world.mesh_renderer3d(&"surface".into()).cloned();
    let clear = candidate(
        &mut loader,
        &world,
        collision_request(&world, 4, 3, CollisionUpdate3D::Clear),
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, clear).status, ContentStatus::Applied);
    assert_eq!(receipt(&loader, clear).collision_revision, 4);
    assert_eq!(world.mesh_renderer3d(&"surface".into()).cloned(), renderer);
    assert!(body(&world).is_none());
    assert!(hit(&mut host, &world).is_none());
    let release = candidate(
        &mut loader,
        &world,
        GeneratedContentRequest {
            operation: GeneratedOperation3D::Release,
            ..request(&world, 5, 4)
        },
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, release).status, ContentStatus::Applied);
    assert_eq!(receipt(&loader, release).collision_revision, 4);
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert!(backend.uploads.is_empty());
    drop(host);
    loader.close_generated_content();
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}

#[test]
fn generated_content_376_collision_host_gpu_physics_and_external_binding_failures_never_half_commit(
) {
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let old = authored_body();
    set_body(&mut world, &old);
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    candidate(&mut loader, &world, request(&world, 1, 0));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let old_renderer = world.mesh_renderer3d(&"surface".into()).cloned();
    let make = |world: &World, id, collision| {
        let mut r = request(world, id, 1);
        let GeneratedOperation3D::Replace { collision: c, .. } = &mut r.operation else {
            unreachable!()
        };
        *c = collision;
        r
    };
    backend.fail_poll = true;
    let gpu = candidate(&mut loader, &world, make(&world, 2, mesh_collision()));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, gpu).status, ContentStatus::Failed);
    assert!(loader.generated_content.records[&2]
        .prepared_collision
        .is_none());
    assert_eq!(body(&world), Some(old.clone()));
    assert_eq!(
        world.mesh_renderer3d(&"surface".into()).cloned(),
        old_renderer
    );
    backend.fail_poll = false;
    let invalid = CollisionUpdate3D::Replace {
        body: StaticBody3D::new(StaticShape3D::Convex {
            vertex_count: 4,
            vertex_bytes: vec![0; 48],
        }),
    };
    let physics = candidate(&mut loader, &world, make(&world, 3, invalid));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, physics).status, ContentStatus::Failed);
    assert_eq!(
        world.mesh_renderer3d(&"surface".into()).cloned(),
        old_renderer
    );
    assert_eq!(body(&world), Some(old));
    backend.ready = false;
    let conflict = candidate(&mut loader, &world, make(&world, 4, mesh_collision()));
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    let mut external = authored_body();
    external.shape = CollisionShape3D::Sphere { radius: 0.8 };
    set_body(&mut world, &external);
    backend.ready = true;
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(
        receipt(&loader, conflict).error.unwrap().field_path,
        "target.collisionBinding"
    );
    assert_eq!(body(&world), Some(external));
    assert_eq!(
        world.mesh_renderer3d(&"surface".into()).cloned(),
        old_renderer
    );
    assert_eq!(receipt(&loader, conflict).applied_revision, 1);
}

#[test]
fn generated_content_376_collision_host_release_and_close_remove_only_generated_collision_ownership(
) {
    for generated in [false, true] {
        let (fixture, mut loader, mut world, mut host) = native_fixture(false);
        let old = authored_body();
        set_body(&mut world, &old);
        let mut backend = ControlledBackend {
            ready: true,
            ..Default::default()
        };
        let mut first = request(&world, 1, 0);
        if generated {
            let GeneratedOperation3D::Replace { collision, .. } = &mut first.operation else {
                unreachable!()
            };
            *collision = mesh_collision();
        }
        candidate(&mut loader, &world, first);
        tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
        let release = candidate(
            &mut loader,
            &world,
            GeneratedContentRequest {
                operation: GeneratedOperation3D::Release,
                ..request(&world, 2, 1)
            },
        );
        tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
        assert_eq!(receipt(&loader, release).status, ContentStatus::Applied);
        assert_eq!(body(&world), if generated { None } else { Some(old) });
        assert_eq!(hit(&mut host, &world).is_some(), !generated);
        assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    }
    let (fixture, mut loader, mut world, mut host) = native_fixture(false);
    let mut backend = ControlledBackend {
        ready: true,
        ..Default::default()
    };
    let ticket = candidate(
        &mut loader,
        &world,
        collision_request(&world, 1, 0, mesh_collision()),
    );
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert_eq!(receipt(&loader, ticket).status, ContentStatus::Applied);
    assert!(world.mesh_renderer3d(&"surface".into()).is_none());
    assert!(hit(&mut host, &world).is_some());
    loader.close_generated_content();
    tick(&mut host, &fixture, &mut loader, &mut world, &mut backend);
    assert!(body(&world).is_none());
    assert!(hit(&mut host, &world).is_none());
    assert_eq!(
        loader.asset_loader.streaming_snapshot().cpu_resident_bytes,
        0
    );
}
