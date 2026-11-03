//! Actual retained Prepare rows and per-camera production selection.
use super::*;
use crate::model_lod3d::{Hlod3D, HlodRole3D, MeshPresentation3D, ModelLod3D, ModelLodLevel3D};
use crate::render_contract::MaterialBlend;
use crate::render_state::{RenderProxy, RenderProxyDescriptor, RenderProxyId};

#[test]
#[ignore = "explicit published BM10 package; main-view Prepare count at frozen K3 keys and weakest sample"]
fn s5d_bm10_published_k3_main_view_selects_original_static_population() {
    use crate::{
        render_command::apply_batch, render_extract::RenderExtractContext,
        runtime_instance_loader::RuntimeInstanceLoader, runtime_package::load_runtime_package,
        world::World,
    };
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM10_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(root).value.unwrap();
    let mut world = World::new();
    let mut instances = RuntimeInstanceLoader::from_package(&package);
    assert!(instances.asset_loader_mut().mount_bundle("startup"));
    let (loaded, report) = instances.load_active_scene_instance(&package, &mut world);
    assert!(loaded.is_some(), "{report:?}");
    world.finish_spatial_tick();
    let mut extract = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let batch = extract
        .extract_world_dirty(1, &mut world, &scene)
        .normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &batch).is_empty());
    extract
        .project_main_camera(&world, &mut scene, [1765, 993])
        .unwrap()
        .unwrap();
    let mut cache = ProductionGeometryCache::default();
    let mut manager = RenderResourceManager::new();
    let start = std::time::Instant::now();
    let mut frame = 1;
    let retained = loop {
        instances.asset_loader_mut().poll_async();
        let snapshot = cache.snapshot(
            frame,
            &scene,
            "bm10-original-k3-selection",
            Some(instances.asset_loader_mut()),
            &mut manager,
            None,
        );
        if let Some(error) = snapshot.preparation_error() {
            assert_eq!(error.code, ContractErrorCode::ResourceNotReady, "{error:?}");
            assert!(start.elapsed().as_secs() < 45, "{error:?}");
        } else {
            break snapshot;
        }
        frame += 1;
        std::thread::yield_now();
    };
    let mut snapshot = retained.as_ref().clone();
    let fillers: HashSet<_> = snapshot
        .instances
        .iter()
        .filter_map(|row| match &row.projection {
            InstanceProjection::Mesh(p)
                if p.provenance
                    .source_entity_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("combined-static-")) =>
            {
                Some(row.instance_id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(fillers.len(), 44_843);
    // Frozen K3 camera keys plus phase4620, the weakest of the source-only
    // 241 samples. Count actual production batch members in the game view;
    // shadow references and simulation populations cannot inflate this proof.
    const POSITIONS: [[f32; 3]; 5] = [
        [0., 4., 10.],
        [40., 6., 20.],
        [60., 8., -40.],
        [-40., 5., -20.],
        [0., 4., 10.],
    ];
    const ROTATIONS: [[f32; 4]; 5] = [
        [-0.14521314, 0., 0., 0.9894004],
        [-0.22975291, 0., 0., 0.973249],
        [-0.21258415, 0.67439455, 0.21258415, 0.67439455],
        [-0.1337192, -0.69434804, -0.1337192, 0.69434804],
        [-0.14521314, 0., 0., 0.9894004],
    ];
    for tick in [0, 1800, 3600, 4620, 5400, 7200] {
        let index = ((tick / 1800) as usize).min(3);
        let t = (tick - index as u64 * 1800) as f32 / 1800.;
        let a = ROTATIONS[index];
        let mut b = ROTATIONS[index + 1];
        let mut dot = a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
        if dot < 0. {
            b = b.map(|x| -x);
            dot = -dot;
        }
        let theta = dot.clamp(-1., 1.).acos();
        let mut rotation = if dot > 0.9995 {
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
        } else {
            std::array::from_fn(|i| {
                (a[i] * ((1. - t) * theta).sin() + b[i] * (t * theta).sin()) / theta.sin()
            })
        };
        let length = rotation.iter().map(|x| x * x).sum::<f32>().sqrt();
        rotation = rotation.map(|x| x / length);
        for view in Arc::make_mut(&mut snapshot.views) {
            if view.camera.view.view_kind == crate::render_contract::ViewKind::Game {
                view.camera.pose.position = std::array::from_fn(|i| {
                    POSITIONS[index][i] + (POSITIONS[index + 1][i] - POSITIONS[index][i]) * t
                });
                view.camera.pose.rotation = rotation;
            }
        }
        let selected = snapshot
            .build_uncached(&RenderTarget::headless_texture("main", 1765, 993), &manager)
            .unwrap();
        let views: Vec<_> = selected
            .views
            .iter()
            .filter(|view| !view.view.is_shadow_map())
            .collect();
        assert_eq!(views.len(), 1);
        let mut visible = HashSet::new();
        for batch in views[0].batches.as_ref().unwrap() {
            for member in batch.visible.iter() {
                let id = batch.instances[member.instance_index as usize].instance_id;
                if fillers.contains(&id) {
                    assert!(visible.insert(id));
                }
            }
        }
        for draw in &views[0].draws {
            if views[0].batches.is_none() && fillers.contains(&draw.instance_id) {
                assert!(visible.insert(draw.instance_id));
            }
        }
        eprintln!(
            "BM10 actual Prepare K3phase={tick} mainStaticFillers={}",
            visible.len()
        );
        assert!(
            visible.len() >= 10_000,
            "K3phase={tick}: only {} static fillers selected",
            visible.len()
        );
    }
}

#[test]
#[ignore = "explicit original BM13 package; actual terrain LOD Prepare and native simulation independence"]
fn s5d_bm13_original_lod_prepare_retirement_keeps_collision_and_navigation() {
    use crate::{
        ids::EntityId,
        navigation_3d::NavigationModule,
        physics_3d::{PhysicsModule, PhysicsRay3D},
        render_command::apply_batch,
        render_extract::RenderExtractContext,
        runtime_instance_loader::RuntimeInstanceLoader,
        runtime_package::load_runtime_package,
        world::World,
    };
    let root = std::path::PathBuf::from(std::env::var_os("AIFE_BM13_LOAD_PACKAGE").unwrap());
    let package = load_runtime_package(root).value.unwrap();
    let mut world = World::new();
    let mut instances = RuntimeInstanceLoader::from_package(&package);
    instances
        .asset_loader_mut()
        .set_streaming_budget(crate::runtime_asset_loader::StreamingBudget {
            max_handles: 65_536,
            max_operations: 256,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    assert!(instances.asset_loader_mut().mount_bundle("startup"));
    let (loaded, report) = instances.load_active_scene_instance(&package, &mut world);
    assert!(loaded.is_some(), "{report:?}");
    let terrain: Vec<_> = (0..9)
        .map(|i| EntityId::from(format!("terrain-{i}")))
        .collect();
    let generations: Vec<_> = terrain
        .iter()
        .map(|id| world.entity_generation(id).unwrap())
        .collect();
    let mut physics = PhysicsModule::default();
    let mut nav = NavigationModule::default();
    world.begin_spatial_tick(1);
    nav.advance(&mut world, instances.asset_loader_mut(), 1, 1. / 60.)
        .unwrap();
    physics
        .advance_with_assets(&mut world, instances.asset_loader_mut(), 1, 1. / 60.)
        .unwrap();
    world.finish_spatial_tick();
    assert_eq!(physics.body_count(), 9);
    assert_eq!(nav.resident_surface_count(), 9);
    let ray = PhysicsRay3D {
        origin: [1., 1000., 1.],
        displacement: [0., -2000., 0.],
    };
    let hit = physics
        .raycast(&ray)
        .unwrap()
        .expect("original terrain must participate in actual Jolt queries");
    let mut extract = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    let batch = extract
        .extract_world_dirty(1, &mut world, &scene)
        .normalize_merge(&scene);
    assert!(apply_batch(&mut scene, &batch).is_empty());
    extract
        .project_main_camera(&world, &mut scene, [1765, 993])
        .unwrap()
        .unwrap();
    let mut cache = ProductionGeometryCache::default();
    let mut manager = RenderResourceManager::new();
    let start = std::time::Instant::now();
    let mut frame = 1;
    let retained = loop {
        instances.asset_loader_mut().poll_async();
        let snapshot = cache.snapshot(
            frame,
            &scene,
            "bm13-original-independent-domains",
            Some(instances.asset_loader_mut()),
            &mut manager,
            None,
        );
        if let Some(error) = snapshot.preparation_error() {
            assert_eq!(error.code, ContractErrorCode::ResourceNotReady, "{error:?}");
            assert!(
                start.elapsed().as_secs() < 45,
                "actual original Prepare did not terminate: {error:?}"
            );
        } else {
            break snapshot;
        }
        frame += 1;
        std::thread::yield_now();
    };
    let mut snapshot = retained.as_ref().clone();
    drop(retained);
    assert_eq!(
        cache.vegetation.instances.len(),
        100_000,
        "all original50K forest sources retain both prepared LOD candidates"
    );
    let base = snapshot
        .views
        .iter()
        .find(|view| view.camera.view.view_kind == crate::render_contract::ViewKind::Game)
        .unwrap()
        .camera
        .clone();
    for (index, id) in terrain.iter().enumerate() {
        let runtime = world.runtime_id_for_source(id).unwrap();
        let row = snapshot.instances.iter().find(|item| matches!(&item.projection, InstanceProjection::Mesh(p) if p.runtime_entity_id == runtime)).unwrap();
        let InstanceProjection::Mesh(projection) = &row.projection else {
            unreachable!()
        };
        let matrix = projection.world.world_matrix;
        let absolute: [f64; 3] = std::array::from_fn(|axis| {
            f64::from(matrix[12 + axis]) + projection.coordinate_origin[axis]
        });
        for (distance, suffix) in [(0., "fine"), (71., "coarse")] {
            let mut camera = base.clone();
            camera.pose.position = std::array::from_fn(|axis| {
                (absolute[axis] - camera.origin[axis] + if axis == 0 { distance } else { 0. })
                    as f32
            });
            let selected = snapshot.build_instances(&camera).unwrap();
            let ids: std::collections::BTreeSet<_> =
                selected.iter().map(|item| item.instance_id).collect();
            let actual: std::collections::BTreeSet<_> = snapshot
                .instances
                .iter()
                .filter(|item| ids.contains(&item.instance_id))
                .filter_map(|item| match &item.projection {
                    InstanceProjection::Mesh(p) if p.runtime_entity_id == runtime => {
                        Some(p.component.mesh.0.id.clone())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                actual,
                [format!("bm13-ground-{index}-{suffix}")].into(),
                "each actual tile must select exactly one original prepared LOD"
            );
            if index == 0 {
                Arc::make_mut(&mut snapshot.views)[0].camera = camera;
                snapshot
                    .build_uncached(&RenderTarget::headless_texture("main", 1765, 993), &manager)
                    .unwrap();
            }
            assert_eq!(physics.body_count(), 9);
            assert_eq!(nav.resident_surface_count(), 9);
            assert_eq!(physics.raycast(&ray).unwrap(), Some(hit.clone()));
        }
    }
    assert!(
        !cache.owned_handles.is_empty(),
        "actual visual Prepare must own resources before retirement"
    );
    drop(snapshot);
    cache.reset_world(frame + 1, &mut manager);
    manager.end_frame(frame + 4);
    assert!(cache.owned_handles.is_empty());
    assert_eq!(
        manager.report(frame + 4).resident_bytes,
        0,
        "released visual Prepare resources must not be kept by this cache"
    );
    world.begin_spatial_tick(2);
    nav.advance(&mut world, instances.asset_loader_mut(), 2, 1. / 60.)
        .unwrap();
    physics
        .advance_with_assets(&mut world, instances.asset_loader_mut(), 2, 1. / 60.)
        .unwrap();
    world.finish_spatial_tick();
    assert_eq!(physics.body_count(), 9);
    assert_eq!(nav.resident_surface_count(), 9);
    assert_eq!(physics.raycast(&ray).unwrap(), Some(hit));
    for (id, generation) in terrain.iter().zip(generations) {
        assert_eq!(world.entity_generation(id), Some(generation));
    }
    eprintln!("BM13 original329 package: all9 dense terrain collision bodies and9 nav surfaces survive actual near/far LOD Prepare and visual owner retirement; original50K forest retained; no GPU retirement/fullK2/performance claim");
}

fn projection(snapshot: &ProductionGeometrySnapshot) -> MeshProjection {
    match &snapshot.instances[0].projection {
        InstanceProjection::Mesh(p) => p.as_ref().clone(),
        _ => panic!("ordinary mesh fixture required"),
    }
}

fn scene_with(projections: &[MeshProjection]) -> RenderSceneState {
    let mut scene = RenderSceneState::new();
    for (index, p) in projections.iter().enumerate() {
        scene.insert_proxy(RenderProxy::from_descriptor(
            RenderProxyId(0),
            p.runtime_entity_id,
            format!("representation-{index}").as_str().into(),
            crate::components::Transform::identity(),
            RenderProxyDescriptor {
                visible: true,
                layer: "main".into(),
                payload: RenderProxyPayload::Mesh3D(Box::new(p.clone())),
            },
        ));
    }
    scene
}

fn retain(snapshot: &mut ProductionGeometrySnapshot, projections: Vec<Arc<MeshProjection>>) {
    let base = snapshot.instances[0].clone();
    snapshot.instances = Arc::new(
        projections
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                let mut item = base.as_ref().clone();
                item.instance_id = i as u64 + 1;
                item.projection = InstanceProjection::Mesh(p);
                Arc::new(item)
            })
            .collect(),
    );
}

#[test]
fn s4_model_lod_production_two_cameras_share_origin_select_one_without_identity_mutation() {
    let (manager, mut snapshot) = output_binding_tests::mesh_snapshot(MaterialBlend::Opaque);
    let mut p = projection(&snapshot);
    let gameplay = p.runtime_entity_id;
    p.presentation = Some(Arc::new(MeshPresentation3D {
        lod: Some(ModelLod3D {
            schema_version: "model-lod3d.v1".into(),
            levels: vec![
                ModelLodLevel3D {
                    distance: 0.,
                    renderer: p.component.clone(),
                },
                ModelLodLevel3D {
                    distance: 10.,
                    renderer: p.component.clone(),
                },
            ],
        }),
        ..Default::default()
    }));
    let scene = scene_with(&[p]);
    let representations = representation_projections(&scene).unwrap();
    assert_eq!(representations.len(), 2);
    assert!(representations
        .iter()
        .all(|(_, p)| p.runtime_entity_id == gameplay));
    retain(
        &mut snapshot,
        representations.into_iter().map(|(_, p)| p).collect(),
    );
    let mut near = snapshot.views[0].camera.clone();
    near.pose.position = [0., 0., -100.];
    let mut far = near.clone();
    far.pose.position[0] = 20.;
    assert_eq!(near.origin, far.origin);
    assert_eq!(
        snapshot
            .build_instances(&near)
            .unwrap()
            .iter()
            .map(|i| i.instance_id)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        snapshot
            .build_instances(&far)
            .unwrap()
            .iter()
            .map(|i| i.instance_id)
            .collect::<Vec<_>>(),
        vec![2]
    );
    let near_work = snapshot.prepared_workset(&near, &manager).unwrap();
    let far_work = snapshot.prepared_workset(&far, &manager).unwrap();
    assert!(
        !Arc::ptr_eq(&near_work, &far_work),
        "same-origin camera positions must not reuse another LOD's workset"
    );
    assert!(Arc::ptr_eq(
        &near_work,
        &snapshot.prepared_workset(&near, &manager).unwrap()
    ));
    // Exercise the retained production consumer, not only LOD selection.
    // The workset row table must have exactly the filtered Prepare row order.
    for camera in [&near, &far] {
        Arc::make_mut(&mut snapshot.views)[0].camera = camera.clone();
        snapshot
            .build_uncached(&RenderTarget::headless_texture("main", 64, 64), &manager)
            .unwrap();
    }
    for (distance, expected) in [(9.999, 1), (10., 2), (10.001, 2)] {
        near.pose.position[0] = distance;
        let rows = snapshot.build_instances(&near).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "LOD threshold must have neither hole nor duplicate"
        );
        assert_eq!(rows[0].instance_id, expected);
    }
    assert!(snapshot.instances.iter().all(
        |r| matches!(&r.projection,InstanceProjection::Mesh(p) if p.runtime_entity_id==gameplay)
    ));
}

fn hlod(p: &MeshProjection, role: HlodRole3D) -> MeshProjection {
    let mut p = p.clone();
    p.presentation = Some(Arc::new(MeshPresentation3D {
        hlod: Some(Hlod3D {
            schema_version: "hlod3d.v1".into(),
            group: "forest".into(),
            center: [0., 0., -100.],
            distance: 10.,
            role,
        }),
        ..Default::default()
    }));
    p
}

#[test]
fn s4_hlod_production_threshold_selects_one_set_for_each_camera() {
    let (manager, mut snapshot) = output_binding_tests::mesh_snapshot(MaterialBlend::Opaque);
    let base = projection(&snapshot);
    let detail = hlod(&base, HlodRole3D::Detail);
    let mut proxy = hlod(&base, HlodRole3D::Proxy);
    proxy.runtime_entity_id = crate::ids::RuntimeEntityId::new(2, 1);
    let scene = scene_with(&[detail.clone(), proxy.clone()]);
    retain(
        &mut snapshot,
        representation_projections(&scene)
            .unwrap()
            .into_iter()
            .map(|(_, p)| p)
            .collect(),
    );
    let mut camera = snapshot.views[0].camera.clone();
    camera.pose.position = [0., 0., -100.];
    let near = snapshot.prepared_workset(&camera, &manager).unwrap();
    camera.pose.position[0] = 20.;
    let far = snapshot.prepared_workset(&camera, &manager).unwrap();
    assert!(!Arc::ptr_eq(&near, &far));
    for (distance, expected) in [(0., 1), (9.999, 1), (10., 2), (10.001, 2), (20., 2)] {
        camera.pose.position[0] = distance;
        let rows = snapshot.build_instances(&camera).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "HLOD must have exactly one representation set"
        );
        assert_eq!(rows[0].instance_id, expected);
    }
    assert_eq!(detail.runtime_entity_id, base.runtime_entity_id);
    assert_eq!(
        proxy.runtime_entity_id,
        crate::ids::RuntimeEntityId::new(2, 1)
    );
}

#[test]
fn s4_hlod_incomplete_duplicate_proxy_and_inconsistent_group_reject_before_prepare() {
    let (_, snapshot) = output_binding_tests::mesh_snapshot(MaterialBlend::Opaque);
    let base = projection(&snapshot);
    let detail = hlod(&base, HlodRole3D::Detail);
    let proxy = hlod(&base, HlodRole3D::Proxy);
    assert!(representation_projections(&scene_with(&[detail.clone()])).is_err());
    assert!(representation_projections(&scene_with(&[proxy.clone()])).is_err());
    assert!(representation_projections(&scene_with(&[
        detail.clone(),
        proxy.clone(),
        proxy.clone()
    ]))
    .is_err());
    let mut mismatch = proxy.clone();
    Arc::make_mut(mismatch.presentation.as_mut().unwrap())
        .hlod
        .as_mut()
        .unwrap()
        .distance = 11.;
    assert!(representation_projections(&scene_with(&[detail.clone(), mismatch])).is_err());
    let mut mismatch = proxy;
    Arc::make_mut(mismatch.presentation.as_mut().unwrap())
        .hlod
        .as_mut()
        .unwrap()
        .center[0] = 1.;
    assert!(representation_projections(&scene_with(&[detail, mismatch])).is_err());
}
