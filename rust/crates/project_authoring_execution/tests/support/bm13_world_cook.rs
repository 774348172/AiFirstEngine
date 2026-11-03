//! Original BM13 payload admission. This is not a visibility or runtime qualification.
use super::*;
use engine_runtime::{
    runtime_package_builder::RuntimePackageBuildInput, terrain3d::HeightfieldAsset3D,
};
fn inspect(input: &RuntimePackageBuildInput) -> Result<Value, String> {
    inspect_bm13_input(input).map_err(|e| e.message)
}
#[test]
#[ignore = "explicit original full BM13 Cook; bounded construction-owned memory/time monitor required"]
fn actual_bm13_original_world_payloads_cook_and_missing_payloads_reject() {
    let root = PathBuf::from(std::env::var_os("AIFE_BM13_PROJECT").unwrap());
    let evidence = PathBuf::from(std::env::var_os("AIFE_BM13_EVIDENCE").unwrap());
    fs::create_dir(&evidence).unwrap();
    let source: HeightfieldAsset3D =
        serde_json::from_slice(&fs::read(root.join("Assets/World/terrain.asset")).unwrap())
            .unwrap();
    source.validate().unwrap();
    assert_eq!(source.quads, 256);
    assert_eq!(source.tiles.len(), 9);
    let mut session = ProjectAuthoringSession::open(&root).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("bm13-full-cook", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    fs::write(evidence.join("started.json"),serde_json::to_vec_pretty(&json!({"sourceDigest":source_digest(lease.snapshot()),"stage":"actual-Compiler-prepare","qualification":"none"})).unwrap()).unwrap();
    let prepared = compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &evidence.parent().unwrap().join(".lit-cook-cache"),
        )
        .unwrap();
    fs::write(
        evidence.join("prepare.json"),
        serde_json::to_vec_pretty(&prepared.prepare_summary()).unwrap(),
    )
    .unwrap();
    let actual = inspect(prepared.runtime_package_build_input()).unwrap();
    let mut bad = prepared.runtime_package_build_input().clone();
    let mut rejected = Vec::new();
    for id in [
        "bm13-ground-0-fine",
        "bm13-ground-0-coarse",
        "bm13-ground-0-collision",
        "bm13-ground-0-nav",
        "bm13-forest-0",
        "bm13-tree-coarse",
        "bm13-particles",
        "bm13-tone",
    ] {
        let at = bad.assets.iter().position(|a| a.asset_id == id).unwrap();
        let removed = bad.assets.remove(at);
        let error = inspect(&bad).expect_err(id);
        rejected.push(json!({"missing":id,"error":error}));
        bad.assets.insert(at, removed);
    }
    fs::write(evidence.join("terminal.json"),serde_json::to_vec_pretty(&json!({"status":"passed","actual":actual,"rejected":rejected,"sourceDigest":source_digest(lease.snapshot())})).unwrap()).unwrap();
}

#[test]
#[ignore = "original nine full257-square tiles and50K vegetation; three bounded Compiler Cook rounds"]
fn original_bm13_collision_and_navigation_edits_change_only_their_derived_payloads() {
    use engine_runtime::{navigation_3d::CookedNavigation3D, physics_3d::CookedCollision3D};
    let work = PathBuf::from(std::env::var_os("AIFE_BM13_DERIVED_WORK").unwrap());
    let root = work.join("project");
    let evidence = PathBuf::from(std::env::var_os("AIFE_BM13_DERIVED_EVIDENCE").unwrap());
    fs::create_dir(&evidence).expect("fresh evidence identity");
    let mut previous = BTreeMap::new();
    for round in ["baseline", "terrain-collision", "navigation"] {
        if round == "terrain-collision" {
            let path = root.join("Assets/World/terrain.asset");
            let mut source: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(source["quads"], 256);
            assert_eq!(source["tiles"].as_array().unwrap().len(), 9);
            let height = source["tiles"][0]["heights"][128 * 257 + 128]
                .as_f64()
                .unwrap();
            source["tiles"][0]["heights"][128 * 257 + 128] = json!(height + 0.25);
            fs::write(path, serde_json::to_vec(&source).unwrap()).unwrap();
        } else if round == "navigation" {
            let path = root.join("Assets/World/navigation-0.asset");
            let mut source: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            for vertex in source["vertices"].as_array_mut().unwrap() {
                vertex[1] = json!(vertex[1].as_f64().unwrap() + 0.25);
            }
            fs::write(path, serde_json::to_vec(&source).unwrap()).unwrap();
        }
        let mut session = ProjectAuthoringSession::open(&root).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease(format!("bm13-derived-{round}"), paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let started = std::time::Instant::now();
        let result = compiler.prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &work.join("cache"),
        );
        let seconds = started.elapsed().as_secs_f64();
        if let Err(error) = &result {
            fs::write(evidence.join(format!("{round}-failure.json")), serde_json::to_vec_pretty(&json!({"code":error.code(),"message":error.message(),"diagnostics":error.diagnostics(),"prepareSeconds":seconds})).unwrap()).unwrap();
        }
        let prepared = result.unwrap();
        let input = prepared.runtime_package_build_input();
        // This fixture is the full K2 project: its eighteen independent forest
        // assets and terrain/nav prefabs replace the old four scene bindings.
        // Check the same source population without imposing obsolete IDs.
        let forests: Vec<_> = input
            .assets
            .iter()
            .filter(|asset| asset.asset_type == "vegetation3d")
            .map(|asset| {
                serde_json::from_slice::<engine_runtime::terrain3d::VegetationAsset3D>(
                    asset.runtime_payload.as_ref().unwrap(),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(forests.len(), 18);
        let vegetation: usize = forests
            .iter()
            .flat_map(|asset| &asset.clusters)
            .map(|cluster| cluster.instances.len())
            .sum();
        assert_eq!(vegetation, 50_000);
        let source: HeightfieldAsset3D =
            serde_json::from_slice(&fs::read(root.join("Assets/World/terrain.asset")).unwrap())
                .unwrap();
        source.validate().unwrap();
        assert_eq!(source.quads, 256);
        assert_eq!(source.tiles.len(), 9);
        let population = json!({"terrainTiles":9,"samplesPerTileSide":257,
            "vegetationAssets":forests.len(),"vegetationInstances":vegetation});
        let current: BTreeMap<_, _> = input
            .assets
            .iter()
            .map(|asset| (asset.asset_id.clone(), asset.hash.clone()))
            .collect();
        let changed: BTreeSet<_> = current
            .iter()
            .filter(|(id, hash)| previous.get(*id) != Some(*hash))
            .map(|(id, _)| id.clone())
            .collect();
        for i in 0..9 {
            let collision = input
                .assets
                .iter()
                .find(|asset| asset.asset_id == format!("bm13-ground-{i}-collision"))
                .unwrap();
            serde_json::from_slice::<CookedCollision3D>(
                collision.runtime_payload.as_ref().unwrap(),
            )
            .unwrap()
            .validate()
            .unwrap();
            let nav = input
                .assets
                .iter()
                .find(|asset| asset.asset_id == format!("bm13-ground-{i}-nav"))
                .unwrap();
            let cooked: CookedNavigation3D =
                serde_json::from_slice(nav.runtime_payload.as_ref().unwrap()).unwrap();
            cooked.validate().unwrap();
            assert_eq!(cooked.tiles.len(), 1);
            assert_eq!(cooked.profile.key, "bm13-human");
            assert_eq!(cooked.profile.radius, 0.3);
            assert!(!cooked.tiles[0].data.is_empty());
        }
        fs::write(evidence.join(format!("{round}.json")), serde_json::to_vec_pretty(&json!({"round":round,"prepareSeconds":seconds,"population":population,"sourceDigest":source_digest(lease.snapshot()),"assemblyDigest":prepared.assembly_digest(),"outputs":current,"changedAssets":changed,"producerReports":prepared.producer_reports(),"scope":"Original full BM13 nine terrain/collision/nav sources, exact incremental Cook only; not BM08/Android/Player or whole BM14"})).unwrap()).unwrap();
        if round == "terrain-collision" {
            let required = BTreeSet::from([
                "bm13-ground-0-fine".to_string(),
                "bm13-ground-0-coarse".to_string(),
                "bm13-ground-0-collision".to_string(),
            ]);
            assert!(required.is_subset(&changed));
            let mut allowed = required;
            allowed.insert("bm13-ground-source".into());
            assert!(
                changed.is_subset(&allowed),
                "unexpected derived changes: {changed:?}"
            );
        } else if round == "navigation" {
            assert_eq!(changed, BTreeSet::from(["bm13-ground-0-nav".to_string()]));
        }
        assert!(
            seconds < 600.,
            "review actual prepare cost before another round"
        );
        eprintln!("BM13 derived {round}: validated in {seconds:.3}s");
        previous = current;
    }
    fs::write(evidence.join("completed.json"), serde_json::to_vec_pretty(&json!({"status":"passed","rounds":3,"scope":"Original full BM13 collision/nav incremental derivation only"})).unwrap()).unwrap();
}
