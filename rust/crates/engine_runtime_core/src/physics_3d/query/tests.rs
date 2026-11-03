use super::*;
use crate::{scene_loader::load_scene_into_world, spatial_contract::*, world::World};
use serde_json::json;

fn floor_world() -> World {
    let scene = json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"queries","gravity":0,
    "background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[{
        "schemaVersion":"runtime-entity.v2","id":"floor","name":"floor","enabled":true,"parentId":null,"siblingOrder":0,
        "spatial":{"domain":"world3d","transform":{"translation":[0,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},
        "components":[{"componentType":"engine.body3d","data":{"schemaVersion":"body3d.v1","motion":"static",
            "shape":{"kind":"box","halfExtents":[2,0.5,2]},"mass":1,"friction":0.5,"restitution":0,"ccd":false,"sensor":false,"allowSleeping":true}}]
    }]});
    let RuntimeSceneDocument::Spatial(scene) =
        decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()
    else {
        unreachable!()
    };
    load_scene_into_world(&scene.into()).value.unwrap()
}
fn batch() -> PhysicsQueryBatch3D {
    let mut queries = vec![
        PhysicsQuery3D::Raycast {
            origin: [0., 4., 0.],
            displacement: [0., -8., 0.]
        };
        200
    ];
    queries[1] = PhysicsQuery3D::Raycast {
        origin: [9., 4., 0.],
        displacement: [0., -8., 0.],
    };
    queries.extend((0..50).map(|i| PhysicsQuery3D::Sweep {
        origin: [0., 4., 0.],
        displacement: [0., -8., 0.],
        shape: match i % 3 {
            0 => PhysicsQueryShape3D::Sphere { radius: 0.25 },
            1 => PhysicsQueryShape3D::Box {
                half_extents: [0.25; 3],
            },
            _ => PhysicsQueryShape3D::Capsule {
                half_height: 0.25,
                radius: 0.25,
            },
        },
        rotation_xyzw: [0., 0., 0., 1.],
    }));
    PhysicsQueryBatch3D { queries }
}

#[test]
fn real_jolt_batch_preserves_order_tick_hits_and_current_generations() {
    let mut world = floor_world();
    let mut module = PhysicsModule::default();
    let batch = batch();
    let pending = module.query_batch3d(&world, &batch).unwrap();
    assert_eq!(
        pending,
        PhysicsQueryResult3D {
            tick: None,
            hits: Vec::new()
        }
    );
    world.begin_spatial_tick(7);
    module.advance(&mut world, 7, 1. / 60.).unwrap();
    world.finish_spatial_tick();
    let before = world.physics3d_frame();
    let result = module.query_batch3d(&world, &batch).unwrap();
    assert_eq!(result.tick, Some(7));
    assert_eq!(result.hits.len(), 250);
    assert!(result.hits[1].is_none());
    for (i, hit) in result.hits.iter().enumerate().filter(|(i, _)| *i != 1) {
        let hit = hit
            .as_ref()
            .unwrap_or_else(|| panic!("missing real query {i}"));
        assert_eq!(hit.entity, "floor");
        assert_eq!(
            Some(hit.generation),
            world.entity_generation(&"floor".into())
        );
        assert!((0. ..=1.).contains(&hit.fraction));
        assert!(hit.position.iter().all(|v| v.is_finite()));
        if i < 200 {
            assert!((hit.position[1] - 0.5).abs() < 1e-4);
        }
    }
    assert_eq!(world.physics3d_frame(), before, "query advanced simulation");
    assert_eq!(module.solver_creation_count(), 1);
    let old = world.entity_generation(&"floor".into()).unwrap();
    world.try_despawn_entity(&"floor".into()).unwrap();
    world
        .try_spawn_entity(
            "floor".into(),
            "replacement",
            "actor",
            true,
            crate::components::Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
        )
        .unwrap();
    assert_ne!(world.entity_generation(&"floor".into()), Some(old));
    assert!(
        module
            .query_batch3d(&world, &batch)
            .unwrap()
            .hits
            .iter()
            .all(Option::is_none),
        "retired native body leaked into a replacement incarnation"
    );
    assert_eq!(
        module.query_batch3d(&floor_world(), &batch).unwrap_err(),
        Status::INVALID_HANDLE
    );
}

#[test]
fn physics_batch_rejects_invalid_tail_before_not_ready_and_world_api_borrows_real_owner() {
    let world = floor_world();
    let module = PhysicsModule::default();
    let batch = batch();
    let missing = crate::world_api::WorldReadApi::new(&world)
        .physics_query3d(&batch)
        .unwrap_err();
    assert_eq!(missing, Status::UNSUPPORTED);
    let view = crate::world_api::WorldReadApi::new(&world).with_physics(Some(&module));
    assert_eq!(view.physics_query3d(&batch).unwrap().tick, None);
    let mut bad = batch;
    bad.queries.push(PhysicsQuery3D::Raycast {
        origin: [0.; 3],
        displacement: [0.; 3],
    });
    assert_eq!(
        view.physics_query3d(&bad).unwrap_err(),
        Status::INVALID_ARGUMENT
    );
}
