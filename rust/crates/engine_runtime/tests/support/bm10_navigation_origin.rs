//! Captured original request at tick1529, without the unrelated 56K entities.
use engine_runtime::{
    ids::EntityId,
    navigation_3d::{NavigationModule, NavigationStatus3D},
    project_runtime_session::ProjectRuntimeMutationBuffer,
    runtime_instance_loader::RuntimeInstanceLoader,
    runtime_package::load_runtime_package,
    scene_loader::load_scene_into_world,
    spatial_contract::RuntimeSpatial,
};
use project_runtime_sdk::ProjectRuntimeSpatialIntent as Intent;
#[allow(dead_code)]
#[path = "../../../../../samples/platform_3d_bm10_project/RuntimeModule/src/bm07.rs"]
mod project_bm07;

#[test]
#[ignore = "explicit BM10 package; captured original origin, not full K3 qualification"]
fn bm10_captured_origin_keeps_conservative_obstacle_result() {
    let package = load_runtime_package(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let id = EntityId::from("agent-0094");
    for obstacles in [false, true] {
        let mut scene = package.active_scene.clone();
        scene.entities.retain(|entity| {
            entity.id == id.as_str()
                || entity.id == "camera"
                || entity.components.iter().any(|component| {
                    component.component_type == "engine.navigation3d"
                        || (obstacles && component.component_type == "engine.navigation_obstacle3d")
                })
        });
        let agent = scene
            .entities
            .iter_mut()
            .find(|entity| entity.id == id.as_str())
            .unwrap();
        let Some(RuntimeSpatial::World3D { transform, .. }) = &mut agent.spatial else {
            panic!("missing agent transform")
        };
        transform.translation = [96.49002, 0.8500001, 128.76482];
        let loaded = load_scene_into_world(&scene);
        let mut world = loaded
            .value
            .unwrap_or_else(|| panic!("captured scene hydration: {:?}", loaded.diagnostics));
        let mut loader = RuntimeInstanceLoader::from_package(&package);
        assert!(loader.asset_loader_mut().mount_bundle("startup"));
        let mut navigation = NavigationModule::default();
        let started = std::time::Instant::now();
        let mut terminal = false;
        for tick in 1..=120 {
            world.begin_spatial_tick(tick);
            if tick == 1 {
                let mut batch = ProjectRuntimeMutationBuffer::default();
                batch.spatial_command(
                    id.clone(),
                    world.entity_generation(&id).unwrap(),
                    Intent::NavigationTarget {
                        target: [94., 0., 128.4],
                        deadline_ticks: 120,
                    },
                );
                batch.prepare(&world).unwrap().commit(&mut world).unwrap();
            }
            navigation
                .advance(&mut world, loader.asset_loader_mut(), tick, 1. / 60.)
                .unwrap();
            world.finish_spatial_tick();
            let state = world.navigation3d_state(&id).unwrap();
            if state.status != NavigationStatus3D::Pending {
                eprintln!("captured BM10 obstacles={obstacles} entities={} surfaces={} elapsed={:?} state={state:?}", scene.entities.len(), navigation.resident_surface_count(), started.elapsed());
                assert_eq!(
                    state.status,
                    if obstacles {
                        NavigationStatus3D::NotResident
                    } else {
                        NavigationStatus3D::Following
                    }
                );
                terminal = true;
                break;
            }
            assert!(started.elapsed() < std::time::Duration::from_secs(15));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(terminal, "captured request did not complete");
    }
}

#[test]
#[ignore = "explicit original BM10 package; real Detour target-layout oracle, not full K3"]
fn bm10_distributed_targets_preserve_opposite_flow_and_real_path_mix() {
    use engine_navigation_native::{NativeNav, NavError};
    use engine_runtime::navigation_3d::CookedNavigation3D;
    use std::collections::{BTreeMap, BTreeSet};
    let package = load_runtime_package(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap())
        .value
        .unwrap();
    let reference = engine_runtime::runtime_package::RuntimeAssetRef {
        id: "navigation".into(),
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
    for tile in &asset.tiles {
        nav.add_tile(&tile.data).unwrap();
    }
    let mut starts = BTreeMap::new();
    let mut boxes = Vec::new();
    for entity in &package.active_scene.entities {
        let Some(RuntimeSpatial::World3D { transform, .. }) = &entity.spatial else {
            continue;
        };
        if entity.id.starts_with("agent-") {
            starts.insert(entity.id.clone(), transform.translation);
        }
        for component in &entity.components {
            if component.component_type == "engine.navigation_obstacle3d" {
                assert_eq!(transform.scale, [1.; 3]);
                assert_eq!(transform.rotation_xyzw, [0., 0., 0., 1.]);
                let extents: [f32; 3] =
                    serde_json::from_value(component.data["halfExtents"].clone()).unwrap();
                boxes.push(std::array::from_fn(|i| {
                    if i < 3 {
                        transform.translation[i] - extents[i]
                    } else {
                        transform.translation[i - 3] + extents[i - 3]
                    }
                }));
            }
        }
    }
    assert_eq!(starts.len(), 1000);
    assert_eq!(boxes.len(), 50);
    nav.set_obstacles(&boxes).unwrap();
    let extent = [
        asset.profile.radius.max(0.25),
        asset.profile.height,
        asset.profile.radius.max(0.25),
    ];
    assert_eq!(
        nav.find_path([96.49002, 0.8500001, 128.76482], [94., 0., 128.4], extent),
        Err(NavError::NotResident)
    );
    let mut unique = BTreeSet::new();
    let mut completed = [0usize; 3];
    for agent in 0..1000 {
        let from = starts[&format!("agent-{agent:04}")];
        let class = if agent % 10 < 7 {
            0
        } else if agent % 10 < 9 {
            1
        } else {
            2
        };
        let target = match class {
            0 => project_bm07::reachable_target(agent),
            1 => [200., 0., 220.],
            _ => [280., 0., 128.],
        };
        let path = nav.find_path(from, target.map(|p| p as f32), extent);
        match class {
            0 => {
                assert!(unique.insert(target.map(f64::to_bits)));
                assert!(if agent < 500 {
                    target[0] > 68.
                } else {
                    target[0] < 60.
                });
                let path = path.unwrap_or_else(|e| {
                    panic!("agent={agent} from={from:?} target={target:?}: {e:?}")
                });
                assert!(path.points.iter().any(|p| p[0] < 60.));
                assert!(path.points.iter().any(|p| p[0] > 68.));
            }
            1 => assert_eq!(path, Err(NavError::Unreachable)),
            _ => assert_eq!(path, Err(NavError::NotResident)),
        }
        completed[class] += 1;
    }
    assert_eq!(completed, [700, 200, 100]);
    assert_eq!(unique.len(), 700);
    eprintln!("original63 resident tiles/50 obstacles/1000 authored origins: real Detour mix={completed:?}, 700 distinct opposite-side targets; removed[0,0] tile remains absent; no full K3 claim");
}
