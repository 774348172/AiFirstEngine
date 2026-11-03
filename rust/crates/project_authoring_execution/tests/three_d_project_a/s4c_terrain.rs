//! Real authoring snapshot -> ordinary Cook -> package evidence for S4-C.
use super::*;
use engine_runtime::{
    navigation_3d::CookedNavigation3D,
    physics_3d::CookedCollision3D,
    render_contract::{CookedRenderAsset, RenderAssetData, RenderContract},
    runtime_package_builder::RuntimePackageBuildInput,
};

fn source() -> Value {
    let identity = |id: &str| json!({"assetId":id,"assetGuid":format!("guid-{id}")});
    let tiles: Vec<_> = (0..2)
        .map(|i| {
            json!({
                "key":format!("tile-{i}"),"coordinate":[i,0],"heights":vec![0.;81],
                "lods":[{"step":1,"mesh":identity(&format!("ground-{i}-fine"))},
                    {"step":4,"mesh":identity(&format!("ground-{i}-coarse"))}],
                "collision":identity(&format!("ground-{i}-collision")),
                "navigation":{"asset":identity(&format!("ground-{i}-nav")),
                    "profile":{"key":"human","radius":0.3,"height":1.8,"climb":0.3,
                    "maxSlopeDegrees":45,"cellSize":0.2,"cellHeight":0.1,"tileWidth":16}}
            })
        })
        .collect();
    json!({"schemaVersion":"terrain3d-asset.v1","assetId":"ground-source","assetGuid":"guid-ground-source",
        "quads":8,"sampleSpacing":2,"materialSlots":["surface"],"tiles":tiles})
}

fn cook(fixture: &Fixture) -> RuntimePackageBuildInput {
    let (compiler, lease) = fixture.compiler();
    let prepared = compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsDev,
            &fixture.root.join(".cache/terrain-test"),
        )
        .unwrap();
    prepared.runtime_package_build_input().clone()
}

#[test]
fn s4c_real_authoring_terrain_expands_to_valid_mesh_collision_recast_and_incremental_tiles() {
    let fixture = Fixture::new();
    let mut description = source();
    fixture.write("Assets/terrain.json", &description);
    let mut scene = scene();
    scene["entities"][0]["components"][2]["data"]["mesh"] =
        json!({"id":"ground-0-fine","type":"mesh","guid":"guid-ground-0-fine"});
    fixture.write("Scenes/main.json", &scene);
    let first = cook(&fixture);
    let asset = |input: &RuntimePackageBuildInput, id: &str| {
        input
            .assets
            .iter()
            .find(|a| a.asset_id == id)
            .unwrap()
            .clone()
    };
    for i in 0..2 {
        let mesh_asset = asset(&first, &format!("ground-{i}-fine"));
        let mesh: RenderContract<CookedRenderAsset> =
            serde_json::from_slice(mesh_asset.runtime_payload.as_ref().unwrap()).unwrap();
        let RenderAssetData::Mesh(mesh) = mesh.data.asset else {
            panic!("real mesh cook required")
        };
        assert!(mesh.index_count > 0);
        let collision_asset = asset(&first, &format!("ground-{i}-collision"));
        let collision: CookedCollision3D =
            serde_json::from_slice(collision_asset.runtime_payload.as_ref().unwrap()).unwrap();
        collision.validate().unwrap();
        let navigation_asset = asset(&first, &format!("ground-{i}-nav"));
        let navigation: CookedNavigation3D =
            serde_json::from_slice(navigation_asset.runtime_payload.as_ref().unwrap()).unwrap();
        navigation.validate().unwrap();
        assert_eq!(navigation.tiles.len(), 1);
        assert!(!navigation.tiles[0].data.is_empty());
    }
    let manifest = asset(&first, "ground-source");
    let compact: Value =
        serde_json::from_slice(manifest.runtime_payload.as_ref().unwrap()).unwrap();
    assert_eq!(compact["schemaVersion"], "terrain3d-manifest-asset.v1");
    assert!(
        compact["tiles"][0].get("heights").is_none(),
        "do not retain authored heights alongside cooked buffers"
    );
    let unchanged = cook(&fixture);
    for suffix in ["fine", "coarse", "collision", "nav"] {
        for i in 0..2 {
            let id = format!("ground-{i}-{suffix}");
            assert_eq!(asset(&first, &id).hash, asset(&unchanged, &id).hash);
        }
    }
    description["tiles"][0]["heights"][40] = json!(0.2);
    fixture.write("Assets/terrain.json", &description);
    let changed = cook(&fixture);
    for suffix in ["fine", "coarse", "collision", "nav"] {
        let changed_id = format!("ground-0-{suffix}");
        let unchanged_id = format!("ground-1-{suffix}");
        assert_ne!(
            asset(&first, &changed_id).hash,
            asset(&changed, &changed_id).hash,
            "edited tile must invalidate {suffix}"
        );
        assert_eq!(
            asset(&first, &unchanged_id).hash,
            asset(&changed, &unchanged_id).hash,
            "unaffected tile keeps {suffix} bytes/key"
        );
    }
}

#[test]
fn s4c_duplicate_derived_identity_and_broken_neighbour_edge_reject_before_publication() {
    let fixture = Fixture::new();
    let mut description = source();
    description["tiles"][0]["lods"][0]["mesh"] = json!({"assetId":"cube","assetGuid":"guid-cube"});
    fixture.write("Assets/terrain.json", &description);
    let (compiler, lease) = fixture.compiler();
    let error = compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err();
    assert!(error.code().contains("terrain3d"), "{error:?}");
    drop(lease);
    let mut description = source();
    description["tiles"][1]["heights"][0] = json!(1.);
    fixture.write("Assets/terrain.json", &description);
    let (compiler, lease) = fixture.compiler();
    let error = compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err();
    assert!(error.code().contains("terrain3d"), "{error:?}");
}

#[test]
fn s4c_independent_terrain_navigation_payloads_have_real_detour_path_across_shared_edge() {
    let fixture = Fixture::new();
    let mut terrain = source();
    fixture.write("Assets/terrain.json", &terrain);
    let input = cook(&fixture);
    let asset = |input: &RuntimePackageBuildInput, id: &str| {
        input
            .assets
            .iter()
            .find(|a| a.asset_id == id)
            .unwrap()
            .clone()
    };
    // Each independently cooked local asset contains only tile (0,0).
    // Ordinary placement translates the second payload into the same Detour
    // group, exactly as NavigationModule's existing surface consumer does.
    let mut nav = engine_navigation_native::NativeNav::new(16.).unwrap();
    let mut tiles = Vec::new();
    for i in 0..2 {
        let cooked: CookedNavigation3D = serde_json::from_slice(
            asset(&input, &format!("ground-{i}-nav"))
                .runtime_payload
                .as_ref()
                .unwrap(),
        )
        .unwrap();
        cooked.validate().unwrap();
        assert_eq!(cooked.tiles.len(), 1);
        assert_eq!((cooked.tiles[0].x, cooked.tiles[0].z), (0, 0));
        let translated = engine_navigation_native::translated_tile(
            &cooked.tiles[0].data,
            [i as f32 * 16., 0., 0.],
            [i, 0],
        )
        .unwrap();
        tiles.push(nav.add_tile(&translated).unwrap());
    }
    let path = nav
        .find_path([4., 0., 8.], [28., 0., 8.], [1., 2., 1.])
        .unwrap();
    assert!(path.points.first().unwrap()[0] < 16.);
    assert!(
        path.points.last().unwrap()[0] > 27.5,
        "must reach the other tile, not a partial path"
    );
    // Detour's straight-path final vertex uses ref=0 with END flag; it is
    // not a polygon reference. The separate corridor destination is real.
    assert!(path
        .references
        .iter()
        .zip(&path.flags)
        .all(|(r, flag)| if *r == 0 {
            *flag & 2 != 0
        } else {
            nav.valid_reference(*r)
        }));
    assert!(nav.valid_reference(path.end_reference));
    nav.remove_tile(tiles[1]).unwrap();
    assert!(!nav.valid_reference(path.end_reference));
    assert!(
        nav.find_path([4., 0., 8.], [28., 0., 8.], [1., 2., 1.])
            .is_err(),
        "detached second payload cannot satisfy path"
    );
    // An edit inside the neighbour halo changes the adjacent Nav Cook key,
    // while its independent collision surface retains its own bytes.
    // Shared edge shading normals may legitimately change in render assets.
    terrain["tiles"][0]["heights"][4 * 9 + 7] = json!(0.2);
    fixture.write("Assets/terrain.json", &terrain);
    let changed = cook(&fixture);
    assert_ne!(
        asset(&input, "ground-1-nav").hash,
        asset(&changed, "ground-1-nav").hash
    );
    assert_eq!(
        asset(&input, "ground-1-collision").hash,
        asset(&changed, "ground-1-collision").hash
    );
}
