//! Migration preserves the original native surface owner and atomic World TRS.
use super::s4a::{runtime_json, Fixture};
use engine_runtime::{
    archetype::ComponentValue,
    components::ComponentTypeId,
    ids::EntityId,
    navigation_3d::*,
    project_runtime_session::ProjectRuntimeMutationBuffer,
    runtime_asset::{CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord},
    runtime_instance_loader::*,
};
use project_runtime_sdk::{
    ProjectRuntimeSpatialIntent as Intent, ProjectRuntimeTransform3D as Trs,
};
use serde_json::json;

fn insert(f: &mut Fixture, id: &EntityId, name: &str, value: serde_json::Value) {
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
fn pose(f: &mut Fixture, id: &EntityId, transform: Trs) {
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_command(
        id.clone(),
        f.world.entity_generation(id).unwrap(),
        Intent::Teleport { transform },
    );
    b.prepare(&f.world).unwrap().commit(&mut f.world).unwrap();
    f.frame += 1;
    f.world.begin_spatial_tick(f.frame);
    f.world.finish_spatial_tick();
}
fn nav_asset() -> CookedNavigation3D {
    // Same native profile and padded plane recipe as the S3-D navigation fixture.
    let profile = AgentProfile3D {
        key: "humanoid".into(),
        radius: 0.3,
        height: 1.8,
        climb: 0.3,
        max_slope_degrees: 45.,
        cell_size: 0.2,
        cell_height: 0.1,
        tile_width: 16.,
    };
    let cfg = engine_navigation_native::BakeConfig {
        cell_size: profile.cell_size,
        cell_height: profile.cell_height,
        radius: profile.radius,
        height: profile.height,
        climb: profile.climb,
        slope: profile.max_slope_degrees,
        width: 16.,
        min: [0., -1., 0.],
        max: [16., 4., 16.],
        x: 0,
        z: 0,
    };
    let data = engine_navigation_native::bake(
        &cfg,
        &[
            [-2., 0., -2.],
            [-2., 0., 18.],
            [18., 0., 18.],
            [18., 0., -2.],
        ],
        &[[0, 1, 2], [0, 2, 3]],
        &[],
    )
    .unwrap();
    CookedNavigation3D {
        schema_version: "navigation3d.v1".into(),
        asset_id: "migration-nav".into(),
        source_digest: format!("sha256:{}", "a".repeat(64)),
        recipe: NAV_RECIPE.into(),
        profile,
        tiles: vec![CookedNavTile3D { x: 0, z: 0, data }],
    }
}
fn install(f: &mut Fixture) {
    let bytes = serde_json::to_vec(&nav_asset()).unwrap();
    let digest = engine_runtime::canonical_digest::sha256_prefixed(&bytes);
    std::fs::write(
        f._files.package.package_dir.join("migration-nav.json"),
        &bytes,
    )
    .unwrap();
    let record = RuntimeAssetRecord {
        asset_guid: "migration-nav".into(),
        asset_id: "migration-nav".into(),
        asset_type: "navigation3d".into(),
        sub_asset_id: None,
        version: "1".into(),
        cooked_asset_id: "cooked-migration-nav".into(),
        bundle_id: "startup".into(),
        loader_kind: "navigation3d".into(),
        dependencies: vec![],
        hash: Some(digest.clone()),
        size: Some(bytes.len() as u64),
        flags: vec![],
        source_map_debug: None,
    };
    let cooked = CookedAssetRecord {
        cooked_asset_id: record.cooked_asset_id.clone(),
        bundle_id: "startup".into(),
        path: Some("migration-nav.json".into()),
        offset: None,
        size: record.size,
        compression: None,
        hash: Some(digest),
    };
    f.loader
        .asset_loader_mut()
        .mount_patch_index(RuntimeAssetIndex::new(vec![record], vec![cooked]));
    assert!(f.loader.asset_loader_mut().mount_bundle("startup"));
}
fn advance(f: &mut Fixture) {
    f.frame += 1;
    f.world.begin_spatial_tick(f.frame);
    f.nav
        .advance(&mut f.world, f.loader.asset_loader_mut(), f.frame, 1. / 60.)
        .unwrap();
    f.world.finish_spatial_tick();
}
fn goal(f: &mut Fixture) {
    let mut b = ProjectRuntimeMutationBuffer::new();
    b.spatial_command(
        f.owner.clone(),
        f.world.entity_generation(&f.owner).unwrap(),
        Intent::NavigationTarget {
            target: [12., 0., 8.],
            deadline_ticks: 120,
        },
    );
    b.prepare(&f.world).unwrap().commit(&mut f.world).unwrap();
}
fn wait_path(f: &mut Fixture) {
    let started = std::time::Instant::now();
    loop {
        advance(f);
        if let Some(s) = f.world.navigation3d_state(&f.owner) {
            if s.status != NavigationStatus3D::Pending {
                assert!(!s.points.is_empty(), "{:?}", s);
                assert!(s.points.last().unwrap()[0] > 11.5);
                return;
            }
        }
        assert!(started.elapsed().as_secs() < 5);
        std::thread::yield_now();
    }
}

#[test]
fn s4a_migrated_real_navigation_surface_survives_source_release_and_persistent_release_retires_it()
{
    let mut f = Fixture::new(None);
    install(&mut f);
    let source = f.load("nav-source");
    let id = f.node(source);
    pose(&mut f, &id, Trs::identity());
    insert(
        &mut f,
        &id,
        "engine.navigation3d",
        json!({"schemaVersion":"navigation3d-binding.v1","navigation":{"id":"migration-nav","type":"navigation3d","guid":"migration-nav"}}),
    );
    let actor = f.owner.clone();
    pose(
        &mut f,
        &actor,
        Trs {
            translation: [2., 0.9, 8.],
            ..Trs::identity()
        },
    );
    insert(
        &mut f,
        &actor,
        "engine.motor3d",
        json!({"schemaVersion":"motor3d.v1","radius":0.2,"halfHeight":0.3,"maxSlopeDegrees":45,"stepHeight":0,"mass":70,"gravity":0}),
    );
    insert(
        &mut f,
        &actor,
        "engine.navigation_agent3d",
        json!({"schemaVersion":"navigation3d-agent.v1","profile":"humanoid","speed":4,"arrivalDistance":0.1,"avoidanceRadius":1,"maxNeighbours":8}),
    );
    advance(&mut f);
    assert_eq!(f.nav.resident_surface_count(), 1);
    goal(&mut f);
    wait_path(&mut f);
    let runtime = f.world.runtime_id_for_source(&id).unwrap();
    let generation = f.world.entity_generation(&id).unwrap();
    let world_pose = f.world.spatial_world(&id).unwrap().clone();
    let request = CellMigrationRequest {
        source_entity_id: "node".into(),
        source_generation: generation,
        persistent_placement_id: "persistent-nav".into(),
        parent: None,
        preserve_world: true,
    };
    let persistent = f
        .loader
        .migrate_cell_subtree_with_simulation(
            source,
            request,
            &mut f.world,
            &mut f.manager,
            &mut f.physics,
            &mut f.nav,
        )
        .unwrap();
    f.release(source);
    assert_eq!(f.loader.cell_status(source), Ok(CellStatus::Released));
    assert_eq!(f.world.runtime_id_for_source(&id), Some(runtime));
    let retained_pose = f.world.spatial_world(&id).unwrap();
    assert_eq!(retained_pose.world_matrix, world_pose.world_matrix);
    assert_eq!(retained_pose.anchor, world_pose.anchor);
    assert_eq!(f.nav.resident_surface_count(), 1);
    goal(&mut f);
    wait_path(&mut f);
    assert!(f.world.navigation3d_state(&actor).unwrap().points.len() >= 2);
    f.release(persistent);
    assert!(f.world.entity(&id).is_none());
    assert_eq!(f.nav.resident_surface_count(), 0);
    assert_eq!(f.loader.cell_lifetime_counts(), (0, 0));
    assert_eq!(f.loader.asset_loader().decoded_cache_len(), 0);
    assert!(
        f.world
            .navigation3d_state(&actor)
            .unwrap()
            .points
            .is_empty(),
        "original group retirement must invalidate path projection immediately"
    );
}

#[test]
fn s4a_migration_to_nonuniform_rotated_parent_rejects_shear_without_half_commit() {
    let mut f = Fixture::new(None);
    let source = f.load("shear-source");
    let destination = f.load("shear-parent");
    let id = f.node(source);
    let parent = f.node(destination);
    pose(&mut f, &id, Trs::identity());
    let a = std::f32::consts::FRAC_PI_8;
    pose(
        &mut f,
        &parent,
        Trs {
            rotation_xyzw: [0., a.sin(), 0., a.cos()],
            scale: [2., 1., 1.],
            ..Trs::identity()
        },
    );
    let before = (
        f.world.spatial_world(&id).unwrap().clone(),
        f.world.transform3d(&id).unwrap().clone(),
        f.world.hierarchy(&id).unwrap().clone(),
        f.world.spatial_world(&parent).unwrap().clone(),
        f.loader.cell_lifetime_counts(),
    );
    let request = CellMigrationRequest {
        source_entity_id: "node".into(),
        source_generation: f.world.entity_generation(&id).unwrap(),
        persistent_placement_id: "must-not-exist".into(),
        parent: Some(parent.to_string()),
        preserve_world: true,
    };
    assert!(f
        .loader
        .migrate_cell_subtree_with_simulation(
            source,
            request,
            &mut f.world,
            &mut f.manager,
            &mut f.physics,
            &mut f.nav
        )
        .is_err());
    assert_eq!(f.loader.cell_status(source), Ok(CellStatus::Active));
    assert_eq!(f.loader.cell_status(destination), Ok(CellStatus::Active));
    assert_eq!(
        (
            f.world.spatial_world(&id).unwrap().clone(),
            f.world.transform3d(&id).unwrap().clone(),
            f.world.hierarchy(&id).unwrap().clone(),
            f.world.spatial_world(&parent).unwrap().clone(),
            f.loader.cell_lifetime_counts()
        ),
        before
    );
    assert_eq!(f.node(source), id);
    assert_eq!(f.node(destination), parent);
    f.release(source);
    f.release(destination);
    assert_eq!(f.loader.cell_lifetime_counts(), (0, 0));
}
