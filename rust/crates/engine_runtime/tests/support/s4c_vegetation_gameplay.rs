//! Real original-loader requests and spawned Prefab roots; no second owner.
use super::s4a::{runtime_json, Fixture};
use engine_runtime::{
    archetype::ComponentValue,
    component_value::RuntimeValue,
    components::ComponentTypeId,
    ids::EntityId,
    project_runtime_session::ProjectRuntimeMutationBuffer,
    runtime_asset::{
        CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetLoadState, RuntimeAssetRecord,
    },
    runtime_instance_loader::{CellError, VegetationPromotionFields3D},
    terrain3d::{VegetationAsset3D, VegetationBinding3D},
};
use project_runtime_sdk::{
    ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D as Trs,
};
use serde_json::json;

fn fields() -> VegetationPromotionFields3D {
    VegetationPromotionFields3D {
        component_type: "project.tree".into(),
        source_id: "sourceId".into(),
        cluster_key: "clusterKey".into(),
        cluster_asset_id: "clusterAssetId".into(),
    }
}
fn dynamic(f: &mut Fixture, id: &EntityId, name: &str, value: serde_json::Value) {
    let ty = ComponentTypeId::new(name);
    f.world
        .try_insert_component_value(
            id.clone(),
            ty.clone(),
            ComponentValue::Dynamic {
                component_type: ty,
                value: runtime_json(value),
            },
        )
        .unwrap();
}
fn binding(f: &Fixture) -> VegetationBinding3D {
    let ComponentValue::Dynamic {
        value: RuntimeValue::Object(values),
        ..
    } = f
        .world
        .component_value(&f.owner, &ComponentTypeId::new("engine.vegetation3d"))
        .unwrap()
    else {
        panic!()
    };
    let RuntimeValue::Object(asset) = &values["clusterAsset"] else {
        panic!()
    };
    let RuntimeValue::String(id) = &asset["assetId"] else {
        panic!()
    };
    let RuntimeValue::String(guid) = &asset["assetGuid"] else {
        panic!()
    };
    let RuntimeValue::Array(promoted) = &values["promotedInstanceIds"] else {
        panic!()
    };
    VegetationBinding3D {
        cluster_asset: engine_runtime::terrain3d::TerrainAssetIdentity3D {
            asset_id: id.clone(),
            asset_guid: guid.clone(),
        },
        promoted_instance_ids: promoted
            .iter()
            .map(|v| {
                let RuntimeValue::String(v) = v else { panic!() };
                v.clone()
            })
            .collect(),
    }
}
fn target(f: &mut Fixture, name: &str) -> EntityId {
    let ticket = f.load(name);
    let target = f.node(ticket);
    let renderer=serde_json::from_value(json!({"schemaVersion":"mesh-renderer3d.v1","mesh":{"id":"s4-tree-mesh","type":"mesh","guid":"guid-s4-tree-mesh"},"materials":{"foliage":{"id":"s4-tree-material","type":"material","guid":"guid-s4-tree-material"}},"visible":true,"layers":["world"]})).unwrap();
    f.world
        .try_insert_component_value(
            target.clone(),
            ComponentTypeId::mesh_renderer3d(),
            ComponentValue::MeshRenderer3D(renderer),
        )
        .unwrap();
    dynamic(
        f,
        &target,
        "project.tree",
        json!({"sourceId":"","clusterKey":"","clusterAssetId":""}),
    );
    target
}
fn install(f: &mut Fixture) -> VegetationAsset3D {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../samples/platform_3d_world_project/Assets/S4Environment/vegetation.asset"
    ));
    let asset: VegetationAsset3D = serde_json::from_slice(bytes).unwrap();
    asset.validate().unwrap();
    std::fs::write(f._files.root.join("package/s4-forest.json"), bytes).unwrap();
    let digest = engine_runtime::canonical_digest::sha256_prefixed(bytes);
    let record = RuntimeAssetRecord {
        asset_guid: asset.asset_guid.clone(),
        asset_id: asset.asset_id.clone(),
        asset_type: "vegetation3d".into(),
        sub_asset_id: None,
        version: "1".into(),
        cooked_asset_id: "cooked-s4-forest".into(),
        bundle_id: "startup".into(),
        loader_kind: "vegetation3d".into(),
        dependencies: vec![],
        hash: Some(digest.clone()),
        size: Some(bytes.len() as u64),
        flags: vec![],
        source_map_debug: None,
    };
    let cooked = CookedAssetRecord {
        cooked_asset_id: record.cooked_asset_id.clone(),
        bundle_id: "startup".into(),
        path: Some("s4-forest.json".into()),
        offset: None,
        size: Some(bytes.len() as u64),
        compression: None,
        hash: Some(digest),
    };
    f.loader
        .asset_loader_mut()
        .mount_patch_index(RuntimeAssetIndex::new(vec![record], vec![cooked]));
    assert!(f.loader.asset_loader_mut().mount_bundle("startup"));
    let owner = f.owner.clone();
    dynamic(
        f,
        &owner,
        "engine.vegetation3d",
        json!({"clusterAsset":{"assetId":asset.asset_id,"assetGuid":asset.asset_guid},"promotedInstanceIds":[]}),
    );
    asset
}

#[test]
fn s4c_session_promotion_failed_asset_is_terminal_and_releases_request() {
    use project_runtime_sdk::{
        ProjectRuntimeCellEntity, ProjectRuntimeCellIntent, ProjectRuntimeCellStatus,
        ProjectRuntimeVegetationFields3D,
    };
    let mut f = Fixture::new(None);
    let target = target(&mut f, "failed-promotion-target");
    install(&mut f);
    std::fs::remove_file(f._files.root.join("package/s4-forest.json")).unwrap();
    let (mut host, control) = super::project_cell_session::host();
    control
        .lock()
        .unwrap()
        .next
        .push(ProjectRuntimeCellIntent::PromoteVegetation {
            request_id: 1,
            placement_id: "vegetation-promotion/1".into(),
            owner: ProjectRuntimeCellEntity {
                entity_id: f.owner.to_string(),
                generation: f.world.entity_generation(&f.owner).unwrap(),
            },
            source_instance_id: "tree-west-000".into(),
            target: ProjectRuntimeCellEntity {
                entity_id: target.to_string(),
                generation: f.world.entity_generation(&target).unwrap(),
            },
            fields: ProjectRuntimeVegetationFields3D {
                component_type: "project.tree".into(),
                source_id: "sourceId".into(),
                cluster_key: "clusterKey".into(),
                cluster_asset_id: "clusterAssetId".into(),
            },
        });
    let start = std::time::Instant::now();
    loop {
        super::project_cell_session::tick(
            &mut host,
            &mut f.world,
            &mut f.loader,
            &f._files.package,
            1,
            #[cfg(feature = "real-wgpu")]
            None,
        );
        if f.loader
            .project_cell_snapshot(&f.world)
            .receipts
            .get(&1)
            .is_some_and(|r| r.status == ProjectRuntimeCellStatus::Failed)
        {
            break;
        }
        assert!(
            start.elapsed().as_secs() < 5,
            "failed promotion must not retry forever"
        );
        std::thread::yield_now();
    }
    assert!(binding(&f).promoted_instance_ids.is_empty());
    let snapshot = f.loader.asset_loader().streaming_snapshot();
    assert_eq!(
        (
            snapshot.pending_tickets,
            snapshot.pending_operations,
            snapshot.reserved_bytes
        ),
        (0, 0, 0)
    );
}
fn promote(f: &mut Fixture, target: &EntityId, source: &str) -> Result<(), CellError> {
    let owner = f.owner.clone();
    let og = f.world.entity_generation(&owner).unwrap();
    let tg = f.world.entity_generation(target).unwrap();
    f.loader
        .promote_vegetation_to_entity(&owner, og, source, target, tg, &fields(), &mut f.world)
}
fn snapshot(
    f: &Fixture,
    target: &EntityId,
) -> (
    VegetationBinding3D,
    engine_runtime::spatial_contract::CanonicalTransform3D,
    ComponentValue,
) {
    (
        binding(f),
        f.world.transform3d(target).unwrap().clone(),
        f.world
            .component_value(target, &ComponentTypeId::new("project.tree"))
            .unwrap(),
    )
}

#[test]
fn s4c_real_loader_vegetation_promotion_is_atomic_and_keeps_anchor_source_and_reference_balance() {
    let mut f = Fixture::new(None);
    let target = target(&mut f, "promote-one");
    let asset = install(&mut f);
    let owner = f.owner.clone();
    let generation = f.world.entity_generation(&owner).unwrap();
    let mut batch = ProjectRuntimeMutationBuffer::new();
    batch.spatial_command(
        owner.clone(),
        generation,
        Intent::SetWorldAnchor {
            anchor: [1e9, 0., -1e9],
            preserve_world: false,
        },
    );
    batch.spatial_command(
        owner.clone(),
        generation,
        Intent::Teleport {
            transform: Trs {
                translation: [5., 2., -3.],
                rotation_xyzw: [0., 0., 0., 1.],
                scale: [2.; 3],
            },
        },
    );
    batch
        .prepare(&f.world)
        .unwrap()
        .commit(&mut f.world)
        .unwrap();
    f.frame += 1;
    f.world.begin_spatial_tick(f.frame);
    f.world.finish_spatial_tick();
    let reference = engine_runtime::runtime_package::RuntimeAssetRef {
        id: asset.asset_id.clone(),
        asset_type: "vegetation3d".into(),
        guid: Some(asset.asset_guid.clone()),
        sub_asset: None,
    };
    let cache_before = f.loader.asset_loader().decoded_cache_len();
    let pending = f
        .loader
        .asset_loader_mut()
        .request_async(&reference)
        .unwrap();
    let before = snapshot(&f, &target);
    assert_eq!(
        promote(&mut f, &target, "tree-west-000"),
        Err(CellError::NotReady)
    );
    assert_eq!(snapshot(&f, &target), before);
    let started = std::time::Instant::now();
    while f.loader.asset_loader().get_handle_state(&pending) != Some(RuntimeAssetLoadState::Ready) {
        assert!(
            started.elapsed().as_secs() < 10,
            "vegetation state {:?}, error {:?}, snapshot {:?}",
            f.loader.asset_loader().get_handle_state(&pending),
            f.loader.asset_loader().streaming_error(&pending),
            f.loader.asset_loader().streaming_snapshot()
        );
        f.loader.asset_loader_mut().poll_async();
        std::thread::yield_now();
    }
    let resident = f
        .loader
        .asset_loader()
        .streaming_snapshot()
        .cpu_resident_bytes;
    let count_before = asset.project_instances(&binding(&f), 0.).unwrap().len();
    promote(&mut f, &target, "tree-west-000").unwrap();
    assert_eq!(
        f.loader
            .asset_loader()
            .streaming_snapshot()
            .cpu_resident_bytes,
        resident
    );
    f.frame += 1;
    f.world.begin_spatial_tick(f.frame);
    f.world.finish_spatial_tick();
    let pose = f.world.spatial_world(&target).unwrap();
    assert_eq!(pose.anchor, [1e9, 0., -1e9]);
    let mut final_binding = binding(&f);
    assert_eq!(final_binding.promoted_instance_ids, ["tree-west-000"]);
    let promoted = asset.promote(&final_binding, "tree-west-000").unwrap();
    let expected = engine_runtime::render_contract::multiply_matrix(
        f.world.spatial_world(&owner).unwrap().world_matrix,
        promoted.matrix,
    );
    assert!(pose
        .world_matrix
        .iter()
        .zip(expected)
        .all(|(a, b)| (a - b).abs() < 1e-4));
    assert_eq!(
        asset.project_instances(&final_binding, 0.).unwrap().len(),
        count_before - 1
    );
    let ComponentValue::Dynamic {
        value: RuntimeValue::Object(source),
        ..
    } = f
        .world
        .component_value(&target, &ComponentTypeId::new("project.tree"))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        source["sourceId"],
        RuntimeValue::String("tree-west-000".into())
    );
    assert_eq!(source["clusterKey"], RuntimeValue::String("west".into()));
    assert_eq!(
        source["clusterAssetId"],
        RuntimeValue::String("s4-forest".into())
    );
    let committed = snapshot(&f, &target);
    assert_eq!(
        promote(&mut f, &target, "tree-west-000"),
        Err(CellError::AlreadyCommitted)
    );
    assert_eq!(snapshot(&f, &target), committed);
    assert!(promote(&mut f, &target, "tree-west-001").is_err());
    assert_eq!(snapshot(&f, &target), committed);
    final_binding.promoted_instance_ids.clear(); // local oracle only, not World.
    f.loader.asset_loader_mut().release(&pending).unwrap();
    assert_eq!(f.loader.asset_loader().decoded_cache_len(), cache_before);
    assert_eq!(
        f.loader.asset_loader().streaming_snapshot().pending_tickets,
        0
    );
}

#[test]
fn s4c_promotion_stale_unknown_mismatch_shear_children_and_dynamic_control_leave_world_unchanged() {
    let mut f = Fixture::new(None);
    let target = target(&mut f, "promotion-reject");
    let asset = install(&mut f);
    let owner = f.owner.clone();
    let cache_before = f.loader.asset_loader().decoded_cache_len();
    let reference = engine_runtime::runtime_package::RuntimeAssetRef {
        id: asset.asset_id.clone(),
        asset_type: "vegetation3d".into(),
        guid: Some(asset.asset_guid.clone()),
        sub_asset: None,
    };
    let pending = f
        .loader
        .asset_loader_mut()
        .request_async(&reference)
        .unwrap();
    let started = std::time::Instant::now();
    while f.loader.asset_loader().get_handle_state(&pending) != Some(RuntimeAssetLoadState::Ready) {
        assert!(
            started.elapsed().as_secs() < 10,
            "vegetation state {:?}, error {:?}, snapshot {:?}",
            f.loader.asset_loader().get_handle_state(&pending),
            f.loader.asset_loader().streaming_error(&pending),
            f.loader.asset_loader().streaming_snapshot()
        );
        f.loader.asset_loader_mut().poll_async();
        std::thread::yield_now();
    }
    f.frame += 1;
    f.world.begin_spatial_tick(f.frame);
    f.world.finish_spatial_tick();
    let initial = snapshot(&f, &target);
    let og = f.world.entity_generation(&owner).unwrap();
    let tg = f.world.entity_generation(&target).unwrap();
    assert_eq!(
        f.loader.promote_vegetation_to_entity(
            &owner,
            og + 1,
            "tree-west-000",
            &target,
            tg,
            &fields(),
            &mut f.world
        ),
        Err(CellError::StaleTicket)
    );
    assert_eq!(snapshot(&f, &target), initial);
    assert_eq!(
        f.loader.promote_vegetation_to_entity(
            &owner,
            og,
            "tree-west-000",
            &target,
            tg + 1,
            &fields(),
            &mut f.world
        ),
        Err(CellError::StaleTicket)
    );
    assert_eq!(snapshot(&f, &target), initial);
    assert!(promote(&mut f, &target, "absent").is_err());
    assert_eq!(snapshot(&f, &target), initial);
    let original = f.world.mesh_renderer3d(&target).unwrap().clone();
    let mut wrong = original.clone();
    wrong.mesh.0.id = "other".into();
    f.world
        .try_insert_component_value(
            target.clone(),
            ComponentTypeId::mesh_renderer3d(),
            ComponentValue::MeshRenderer3D(wrong),
        )
        .unwrap();
    assert!(promote(&mut f, &target, "tree-west-000").is_err());
    assert_eq!(snapshot(&f, &target), initial);
    f.world
        .try_insert_component_value(
            target.clone(),
            ComponentTypeId::mesh_renderer3d(),
            ComponentValue::MeshRenderer3D(original),
        )
        .unwrap();
    let mut batch = ProjectRuntimeMutationBuffer::new();
    batch.spatial_command(
        owner.clone(),
        og,
        Intent::Teleport {
            transform: Trs {
                scale: [2., 1., 1.],
                ..Trs::identity()
            },
        },
    );
    batch
        .prepare(&f.world)
        .unwrap()
        .commit(&mut f.world)
        .unwrap();
    f.frame += 1;
    f.world.begin_spatial_tick(f.frame);
    f.world.finish_spatial_tick();
    assert!(
        promote(&mut f, &target, "tree-west-001").is_err(),
        "nonuniform owner times yaw creates nonrepresentable shear"
    );
    assert_eq!(snapshot(&f, &target), initial);
    dynamic(
        &mut f,
        &target,
        "engine.body3d",
        json!({"schemaVersion":"body3d.v1","motion":"dynamic","shape":{"kind":"sphere","radius":0.4},"mass":1,"friction":0.5,"restitution":0,"ccd":false,"allowSleeping":true}),
    );
    assert!(promote(&mut f, &target, "tree-west-000").is_err());
    assert_eq!(snapshot(&f, &target), initial);
    f.world
        .try_remove_component_value(&target, &ComponentTypeId::new("engine.body3d"))
        .unwrap();
    let child_ticket = f.load("foreign-child");
    let child = f.node(child_ticket);
    f.world.try_set_parent(child, Some(target.clone())).unwrap();
    assert_eq!(
        promote(&mut f, &target, "tree-west-000"),
        Err(CellError::ForeignChildren)
    );
    assert_eq!(snapshot(&f, &target), initial);
    f.loader.asset_loader_mut().release(&pending).unwrap();
    assert_eq!(
        f.loader.asset_loader().decoded_cache_len(),
        cache_before,
        "failed requests must not pin the vegetation asset"
    );
}
