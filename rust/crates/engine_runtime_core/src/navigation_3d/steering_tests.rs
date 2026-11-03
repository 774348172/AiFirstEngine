use super::NavigationModule;
use crate::{physics_3d::PhysicsModule, runtime_instance_loader::RuntimeInstanceLoader};

#[test]
#[ignore = "explicit original BM10 package; actual obstacle-edge steering, not full K2"]
fn bm10_original_obstacle_edge_steering_stays_on_resident_mesh() {
    use crate::navigation_3d::CookedNavigation3D;
    use engine_navigation_native::{translated_tile, NativeNav};
    let package = crate::runtime_package::load_runtime_package(
        std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap(),
    )
    .value
    .unwrap();
    let reference = crate::runtime_package::RuntimeAssetRef {
        id: "bm10-k2-navigation".into(),
        asset_type: "navigation3d".into(),
        guid: None,
        sub_asset: None,
    };
    let record = package.runtime_asset_index.resolve(&reference).unwrap();
    let cooked = package
        .runtime_asset_index
        .cooked_asset(&record.cooked_asset_id)
        .unwrap();
    let asset: CookedNavigation3D = serde_json::from_slice(
        &std::fs::read(package.package_dir.join(cooked.path.as_ref().unwrap())).unwrap(),
    )
    .unwrap();
    let mut nav = NativeNav::new(asset.profile.tile_width).unwrap();
    let mut boxes = Vec::new();
    for z in 1..=3 {
        for x in 1..=3 {
            for tile in &asset.tiles {
                nav.add_tile(
                    &translated_tile(
                        &tile.data,
                        [x as f32 * 128., 0., z as f32 * 128.],
                        [x * 4, z * 4],
                    )
                    .unwrap(),
                )
                .unwrap();
            }
            let v = (z % 3) * 3 + x % 3;
            let prefab: serde_json::Value = serde_json::from_slice(
                &std::fs::read(
                    package
                        .package_dir
                        .join(format!("prefabs/bm10-k2-{v}.json")),
                )
                .unwrap(),
            )
            .unwrap();
            for e in prefab["entities"].as_array().unwrap() {
                let c = e["components"].as_array().unwrap();
                if let Some(obstacle) = c
                    .iter()
                    .find(|c| c["componentType"] == "engine.navigation_obstacle3d")
                {
                    let p: [f32; 3] =
                        serde_json::from_value(e["spatial"]["transform"]["translation"].clone())
                            .unwrap();
                    let h: [f32; 3] =
                        serde_json::from_value(obstacle["data"]["halfExtents"].clone()).unwrap();
                    let p = [p[0] + x as f32 * 128., p[1], p[2] + z as f32 * 128.];
                    boxes.push(std::array::from_fn(|i| {
                        if i < 3 {
                            p[i] - h[i]
                        } else {
                            p[i - 3] + h[i - 3]
                        }
                    }));
                }
            }
        }
    }
    let from = [81.393555 + 112., 0.84999985, 206.77745];
    let to = [370.66666, 0., 256.];
    let extent = [
        asset.profile.radius,
        asset.profile.height,
        asset.profile.radius,
    ];
    let clear = nav.find_path(from, to, extent);
    nav.set_obstacles(&boxes).unwrap();
    let blocked = nav.find_path(from, to, extent);
    eprintln!("K2 captured15426 original16tiles*9/{}obstacles from={from:?} to={to:?}: clear={clear:?}; original obstacles={blocked:?}",boxes.len());
    assert!(clear.is_ok(), "captured geometry alone {clear:?}");
    assert_eq!(
        blocked,
        Err(engine_navigation_native::NavError::NotResident)
    );

    // Exercise the actual steering and Jolt owners at the captured blocked
    // polygon edge. A nearby stationary agent supplies normal avoidance.
    let mut scene: serde_json::Value = serde_json::from_slice(
        &std::fs::read(package.package_dir.join("scenes/main.json")).unwrap(),
    )
    .unwrap();
    let mut actor = scene["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "agent-0000")
        .unwrap()
        .clone();
    actor["spatial"]["transform"]["translation"] = serde_json::json!([191.8, 0.85, 206.8]);
    actor["spatial"]
        .as_object_mut()
        .unwrap()
        .remove("meshRenderer");
    let mut neighbour = actor.clone();
    neighbour["id"] = "edge-neighbour".into();
    neighbour["spatial"]["transform"]["translation"] = serde_json::json!([191., 0.85, 206.8]);
    let mut entities = vec![actor, neighbour];
    for z in 1..=3 {
        for x in 1..=3 {
            let v = (z % 3) * 3 + x % 3;
            let prefab: serde_json::Value = serde_json::from_slice(
                &std::fs::read(
                    package
                        .package_dir
                        .join(format!("prefabs/bm10-k2-{v}.json")),
                )
                .unwrap(),
            )
            .unwrap();
            for entity in prefab["entities"].as_array().unwrap() {
                let components = entity["components"].as_array().unwrap();
                let keep = components.iter().any(|c| {
                    c["componentType"] == "engine.navigation3d"
                        || c["componentType"] == "engine.navigation_obstacle3d"
                        || (c["componentType"] == "engine.body3d"
                            && c["data"]["motion"] == "static"
                            && c["data"]["shape"]["halfExtents"][0].as_f64().unwrap_or(0.) >= 64.)
                });
                if !keep {
                    continue;
                }
                let mut e = entity.clone();
                e["id"] = format!("edge-{x}-{z}-{}", entity["id"].as_str().unwrap()).into();
                e["parentId"] = serde_json::Value::Null;
                e["spatial"].as_object_mut().unwrap().remove("meshRenderer");
                let p = e["spatial"]["transform"]["translation"]
                    .as_array_mut()
                    .unwrap();
                p[0] = (p[0].as_f64().unwrap() + x as f64 * 128.).into();
                p[2] = (p[2].as_f64().unwrap() + z as f64 * 128.).into();
                entities.push(e);
            }
        }
    }
    scene["entities"] = entities.into();
    scene["mainCamera"] = serde_json::Value::Null;
    let crate::spatial_contract::RuntimeSceneDocument::Spatial(scene) =
        crate::spatial_contract::decode_runtime_scene(&serde_json::to_vec(&scene).unwrap())
            .unwrap()
    else {
        unreachable!()
    };
    let mut world = crate::scene_loader::load_scene_into_world(&scene.into())
        .value
        .unwrap();
    let mut loader = RuntimeInstanceLoader::from_package(&package);
    let mut navigation = NavigationModule::default();
    let mut physics = PhysicsModule::default();
    let actor = crate::ids::EntityId::from("agent-0000");
    let mut commands = crate::project_runtime_session::ProjectRuntimeMutationBuffer::default();
    commands.spatial_command(
        actor.clone(),
        world.entity_generation(&actor).unwrap(),
        project_runtime_sdk::ProjectRuntimeSpatialIntent::NavigationTarget {
            target: to.map(f64::from),
            deadline_ticks: 120,
        },
    );
    commands
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();
    let mut following = 0;
    for tick in 1..=60 {
        world.begin_spatial_tick(tick);
        navigation
            .advance(&mut world, loader.asset_loader_mut(), tick, 1. / 60.)
            .unwrap();
        physics.advance(&mut world, tick, 1. / 60.).unwrap();
        world.finish_spatial_tick();
        let p = world.transform3d(&actor).unwrap().translation;
        let state = world.navigation3d_state(&actor).unwrap();
        if state.status == crate::navigation_3d::NavigationStatus3D::Following {
            following += 1;
            assert!(
                nav.find_path(p, to, extent).is_ok(),
                "actual navigation steering left resident mesh tick={tick} p={p:?} state={state:?}"
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    eprintln!(
        "Actual steering samples following={following} finalPose={:?} finalState={:?}",
        world.transform3d(&actor).unwrap().translation,
        world.navigation3d_state(&actor).unwrap()
    );
    assert!(
        following > 0,
        "steering probe must actually follow a native path"
    );
}
