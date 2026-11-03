//! Later LODs and vegetation dependencies use the ordinary source/Cook index.
use super::*;
use engine_runtime::runtime_package_builder::RuntimePackageBuildInput;

fn sample(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/platform_3d_world_project/Assets/S4Environment")
        .join(name);
    strict_json(&fs::read(path).unwrap()).unwrap()
}
fn environment(f: &Fixture) {
    for name in ["tree-mesh.asset", "tree-material.asset", "vegetation.asset"] {
        f.write(&format!("Assets/{name}"), &sample(name));
    }
}
fn prepare(f: &Fixture) -> RuntimePackageBuildInput {
    let (compiler, lease) = f.compiler();
    compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap()
        .runtime_package_build_input()
        .clone()
}
fn prepare_error(f: &Fixture) -> GameProjectCompilerError {
    let (compiler, lease) = f.compiler();
    compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err()
}
fn tree_renderer() -> Value {
    json!({"schemaVersion":"mesh-renderer3d.v1","mesh":{"id":"s4-tree-mesh","type":"mesh","guid":"guid-s4-tree-mesh"},"materials":{"foliage":{"id":"s4-tree-material","type":"material","guid":"guid-s4-tree-material"}},"visible":true,"layers":["world"]})
}

#[test]
fn s4b_second_lod_missing_reference_or_wrong_slots_fails_ordinary_prepare() {
    for case in ["missing", "slots"] {
        let f = Fixture::new();
        environment(&f);
        let mut doc = scene();
        let base = doc["entities"][0]["components"][2]["data"].clone();
        let mut next = tree_renderer();
        if case == "missing" {
            next["mesh"]["id"] = json!("missing-lod");
            next["mesh"]["guid"] = json!("guid-missing-lod");
        } else {
            next["materials"] = json!({"surface":{"id":"s4-tree-material","type":"material","guid":"guid-s4-tree-material"}});
        }
        doc["entities"][0]["components"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "componentType":"engine.model_lod3d","data":{"schemaVersion":"model-lod3d.v1",
                "levels":[{"distance":0,"renderer":base},{"distance":30,"renderer":next}]}
            }));
        f.write("Scenes/main.json", &doc);
        let error = prepare_error(&f);
        assert_eq!(
            error.source_location().unwrap().source_path,
            "Scenes/main.json"
        );
        assert!(
            error
                .source_location()
                .unwrap()
                .field_path
                .as_deref()
                .unwrap()
                .contains("/levels/1/renderer"),
            "{case}: {error:?}"
        );
    }
}

#[test]
fn s4b_vegetation_dependency_wrong_guid_or_type_is_rejected_by_real_cook() {
    for case in ["guid", "type"] {
        let f = Fixture::new();
        environment(&f);
        if case == "guid" {
            let mut forest = sample("vegetation.asset");
            forest["clusters"][0]["mesh"]["reference"]["guid"] = json!("wrong-guid");
            f.write("Assets/vegetation.asset", &forest);
        } else {
            let mut wrong = sample("tree-material.asset");
            wrong["assetId"] = json!("s4-tree-mesh");
            wrong["assetGuid"] = json!("guid-s4-tree-mesh");
            f.write("Assets/tree-mesh.asset", &wrong);
        }
        let error = prepare_error(&f);
        assert!(error.code().contains("vegetation3d"), "{case}: {error:?}");
    }
}

#[test]
fn s4b_valid_vegetation_cook_preserves_cluster_dependency_guids_and_scene_closure() {
    let f = Fixture::new();
    environment(&f);
    let mut doc = scene();
    doc["entities"][0]["components"].as_array_mut().unwrap().push(json!({
        "componentType":"engine.vegetation3d","data":{"clusterAsset":{"assetId":"s4-forest","assetGuid":"guid-s4-forest"},"promotedInstanceIds":[]}
    }));
    f.write("Scenes/main.json", &doc);
    let input = prepare(&f);
    let forest = input
        .assets
        .iter()
        .find(|a| a.asset_id == "s4-forest")
        .unwrap();
    let mut actual = forest.dependencies.clone();
    actual.sort();
    assert_eq!(actual, ["guid-s4-tree-material", "guid-s4-tree-mesh"]);
    let cooked: engine_runtime::terrain3d::VegetationAsset3D =
        serde_json::from_slice(forest.runtime_payload.as_ref().unwrap()).unwrap();
    cooked.validate().unwrap();
    assert_eq!(cooked.clusters.len(), 2);
    assert_eq!(
        cooked
            .clusters
            .iter()
            .map(|c| c.instances.len())
            .sum::<usize>(),
        128
    );
    let scene = input.scenes.iter().find(|s| s.id == "main").unwrap();
    assert!(scene
        .entities
        .iter()
        .flat_map(|e| &e.components)
        .any(|c| c.component_type == "engine.vegetation3d"));
}

#[test]
fn s5d_vegetation_lower_lod_enters_real_compiler_dependency_closure() {
    let f = Fixture::new();
    environment(&f);
    let mut coarse = sample("tree-mesh.asset");
    coarse["assetId"] = json!("s4-tree-coarse");
    coarse["assetGuid"] = json!("guid-s4-tree-coarse");
    f.write("Assets/coarse.asset", &coarse);
    let mut forest = sample("vegetation.asset");
    forest["clusters"][0]["lods"] = json!([{"distance":20.,"mesh":{"expectedKind":"mesh","reference":{"id":"s4-tree-coarse","type":"mesh","guid":"guid-s4-tree-coarse"}}}]);
    f.write("Assets/vegetation.asset", &forest);
    let input = prepare(&f);
    let cooked = input
        .assets
        .iter()
        .find(|a| a.asset_id == "s4-forest")
        .unwrap();
    assert!(cooked
        .dependencies
        .contains(&"guid-s4-tree-coarse".to_string()));
    let decoded: engine_runtime::terrain3d::VegetationAsset3D =
        serde_json::from_slice(cooked.runtime_payload.as_ref().unwrap()).unwrap();
    assert_eq!(decoded.clusters[0].lods.len(), 1);
    for (field, value) in [
        ("id", "missing"),
        ("guid", "wrong-guid"),
        ("type", "material"),
    ] {
        let mut bad = forest.clone();
        bad["clusters"][0]["lods"][0]["mesh"]["reference"][field] = json!(value);
        f.write("Assets/vegetation.asset", &bad);
        let failure = prepare_error(&f);
        assert!(
            failure.code().contains("vegetation3d")
                || (field == "type" && failure.message().contains("/clusters/lods/mesh")),
            "{field}: {failure:?}"
        );
    }
}
