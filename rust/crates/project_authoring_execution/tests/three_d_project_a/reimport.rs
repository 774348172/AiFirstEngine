use super::import_scene::{cooked, renderer, ModelFixture};
use super::*;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::render_contract::RenderAssetData;
use engine_runtime::runtime_package_builder::*;
use project_authoring_execution::{
    inspect_model_import_snapshot, ModelImportInspectionRequest, PreparedRuntimePackage,
    ProjectAssemblyArtifactCacheStatus as CacheStatus,
};

fn preview(f: &ModelFixture) -> Value {
    let (_, lease) = f.project.compiler();
    serde_json::to_value(
        inspect_model_import_snapshot(
            lease.snapshot(),
            &ModelImportInspectionRequest {
                descriptor_path: "Assets/import.json".into(),
                max_entries: 256,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
fn prepared(f: &ModelFixture) -> PreparedRuntimePackage {
    let (compiler, lease) = f.project.compiler();
    compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsDev,
            &f.project.root.join(".aife/import-cache"),
        )
        .unwrap()
}
fn status(p: &PreparedRuntimePackage, producer: &str) -> CacheStatus {
    p.producer_reports()
        .iter()
        .find(|r| r.producer_id == producer)
        .unwrap_or_else(|| panic!("missing {producer}: {:?}", p.producer_reports()))
        .cache_status
}
fn save_explicit(f: &ModelFixture) {
    f.project
        .write(f.descriptor["source"].as_str().unwrap(), &f.model);
    f.project.write("Assets/import.json", &f.descriptor);
}
fn approve_fingerprints(f: &mut ModelFixture) {
    for kind in ["outputs", "nodes"] {
        for output in f.descriptor[kind].as_array_mut().unwrap() {
            output["sourceFingerprint"] = json!(sha256_prefixed(
                &serde_json::to_vec(
                    f.model
                        .pointer(output["sourceLocator"].as_str().unwrap())
                        .unwrap()
                )
                .unwrap()
            ));
        }
    }
    save_explicit(f);
}
#[test]
fn reimport_preview_is_read_only_and_exposes_first_mapping_and_impacts() {
    let mut f = ModelFixture::new();
    f.project.write("Prefabs/override.json",&json!({"prefab":"import.prefab.0","entityId":"authored-node-1","overrides":{"surface0":"guid-import-material-0"}}));
    let before = fs::read(f.project.root.join("Assets/import.json")).unwrap();
    let report = preview(&f);
    assert_eq!(report["buildable"], true);
    assert!(report["expectedRevisionId"].is_string());
    assert!(report["referenceImpacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["fieldPath"] == "/overrides/surface0"));
    assert_eq!(
        fs::read(f.project.root.join("Assets/import.json")).unwrap(),
        before
    );
    f.descriptor["outputs"] = json!([]);
    f.descriptor["nodes"] = json!([]);
    save_explicit(&f);
    let report = preview(&f);
    assert_eq!(report["buildable"], false);
    assert_eq!(report["requiredObjects"].as_array().unwrap().len(), 7);
    assert!(report["committedMappings"].as_array().unwrap().is_empty());
    assert!(report["requiredObjects"]
        .as_array()
        .unwrap()
        .iter()
        .all(|o| o.get("assetGuid").is_none()));
}
#[test]
fn reimport_reorder_preserves_explicit_material_slot_and_node_identities() {
    let mut f = ModelFixture::new();
    let mut second = f.model["materials"][0].clone();
    second["pbrMetallicRoughness"]["baseColorFactor"] = json!([0.25, 0.5, 0.75, 1]);
    f.model["materials"].as_array_mut().unwrap().push(second);
    let mut primitive = f.model["meshes"][0]["primitives"][0].clone();
    primitive["material"] = json!(1);
    f.model["meshes"][0]["primitives"]
        .as_array_mut()
        .unwrap()
        .push(primitive);
    f.save();
    add_instances(&f);
    let original = f.input();
    assert_instances(&f, &original);
    f.model["meshes"][0]["primitives"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    f.model["materials"].as_array_mut().unwrap().swap(0, 1);
    for p in f.model["meshes"][0]["primitives"].as_array_mut().unwrap() {
        p["material"] = json!(1 - p["material"].as_u64().unwrap());
    }
    f.model["nodes"].as_array_mut().unwrap().swap(0, 1);
    f.model["scenes"][0]["nodes"] = json!([1]);
    f.model["nodes"][1]["children"] = json!([0]);
    save_explicit(&f);
    let report = preview(&f);
    assert_eq!(report["buildable"], false);
    assert!(report["committedMappings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "slot" && r["ambiguous"] == true));
    let (compiler, lease) = f.project.compiler();
    assert!(compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err()
        .code()
        .ends_with("mapping_invalid"));
    drop(lease);
    for output in f.descriptor["outputs"].as_array_mut().unwrap() {
        if output["kind"] == "material" {
            let old = output["sourceLocator"]
                .as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .parse::<u32>()
                .unwrap();
            output["sourceLocator"] = json!(format!("/materials/{}", 1 - old));
        }
        for slot in output["materialSlots"].as_array_mut().unwrap() {
            let old = slot["sourceLocator"]
                .as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .parse::<u32>()
                .unwrap();
            slot["sourceLocator"] = json!(format!("/meshes/0/primitives/{}", 1 - old));
        }
    }
    for node in f.descriptor["nodes"].as_array_mut().unwrap() {
        let old = node["sourceLocator"]
            .as_str()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap()
            .parse::<u32>()
            .unwrap();
        node["sourceLocator"] = json!(format!("/nodes/{}", 1 - old));
    }
    approve_fingerprints(&mut f);
    let changed = f.input();
    assert_instances(&f, &changed);
    assert_eq!(renderer(&original).materials, renderer(&changed).materials);
    assert_eq!(
        cooked(&original, "import.material.0"),
        cooked(&changed, "import.material.0")
    );
    let entities = &changed
        .prefabs
        .iter()
        .find(|p| p.prefab_id == "import.prefab.0")
        .unwrap()
        .document["entities"];
    assert!(entities
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == "authored-node-1" && e["name"] == "Quad"));
    let mesh = match cooked(&changed, "import.mesh.0").asset {
        RenderAssetData::Mesh(mesh) => mesh,
        _ => panic!(),
    };
    assert_eq!(mesh.submeshes[0].material_slot, "surface1");
    let impact = preview(&f);
    assert_eq!(impact["buildable"], true);
    assert!(impact["referenceImpacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["sourceInstanceEntityId"] == "instance-a" && r["value"] == "authored-node-1"));
}
#[test]
fn reimport_duplicate_names_rename_deletion_and_truncation_never_assign_ids() {
    let mut f = ModelFixture::new();
    f.model["nodes"][0]["name"] = json!("same");
    f.model["nodes"][1]["name"] = json!("same");
    f.save();
    let p = preview(&f);
    assert_eq!(
        p["requiredObjects"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["duplicateName"] == true)
            .count(),
        2
    );
    f.model["nodes"][1]["name"] = json!("renamed");
    save_explicit(&f);
    let p = preview(&f);
    assert_eq!(p["buildable"], false);
    assert!(p["committedMappings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["entityId"] == "authored-node-1" && m["status"] == "reviewRequired"));
    f.model["scenes"][0]["nodes"] = json!([]);
    save_explicit(&f);
    let p = preview(&f);
    assert!(p["committedMappings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["status"] == "removedOrUnmatched"));
    let (_, lease) = f.project.compiler();
    let p = inspect_model_import_snapshot(
        lease.snapshot(),
        &ModelImportInspectionRequest {
            descriptor_path: "Assets/import.json".into(),
            max_entries: 1,
        },
    )
    .unwrap();
    assert!(p.truncated);
}
#[test]
fn cache_model_reuses_real_decoders_and_invalidates_only_consumed_products() {
    cache_material_reimport(false);
}
#[test]
fn s2a_cache_material_and_texture_reimport_invalidates_only_consumed_products() {
    cache_material_reimport(true);
}
fn cache_material_reimport(lit: bool) {
    let mut f = ModelFixture::new();
    if lit {
        f.descriptor["materialStrategy"] = json!("standardLit");
        f.model["materials"][0]
            .as_object_mut()
            .unwrap()
            .remove("extensions");
        for name in ["normalTexture", "occlusionTexture", "emissiveTexture"] {
            f.model["materials"][0][name] = json!({"index":0});
        }
        f.model["materials"][0]["pbrMetallicRoughness"]["metallicRoughnessTexture"] =
            json!({"index":0});
        f.model["samplers"][0]["minFilter"] = json!(9987);
        f.save();
    }
    let first = prepared(&f);
    assert_eq!(status(&first, "model-import"), CacheStatus::Produced);
    let again = prepared(&f);
    assert_eq!(status(&again, "model-import"), CacheStatus::Hit);
    assert!(!again
        .producer_reports()
        .iter()
        .any(|r| r.producer_id == "model-import-geometry"));
    let before = fs::read(f.project.root.join("Assets/import.json")).unwrap();
    let mut scene = scene();
    scene["entities"][1]["components"][0]["data"]["translation"] = json!([1, 2, 4]);
    f.project.write("Scenes/main.json", &scene);
    let camera = prepared(&f);
    assert_eq!(status(&camera, "model-import"), CacheStatus::Hit);
    assert_ne!(
        camera.lineage().revision_id(),
        again.lineage().revision_id()
    );
    assert_eq!(
        fs::read(f.project.root.join("Assets/import.json")).unwrap(),
        before
    );
    f.model["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"] = json!([0.5, 0.75, 1, 1]);
    if lit {
        f.model["materials"][0]["pbrMetallicRoughness"]["roughnessFactor"] = json!(0.65);
    }
    f.save();
    let color = prepared(&f);
    assert_eq!(status(&color, "model-import"), CacheStatus::Produced);
    assert_eq!(status(&color, "model-import-geometry"), CacheStatus::Hit);
    assert_eq!(status(&color, "model-import-image"), CacheStatus::Hit);
    assert_eq!(
        cooked(first.runtime_package_build_input(), "import.mesh.0"),
        cooked(color.runtime_package_build_input(), "import.mesh.0")
    );
    assert_ne!(
        cooked(first.runtime_package_build_input(), "import.material.0"),
        cooked(color.runtime_package_build_input(), "import.material.0")
    );
    let bin_path = f.project.root.join("Models/quad.bin");
    let mut bytes = fs::read(&bin_path).unwrap();
    bytes[..4].copy_from_slice(&(-0.8f32).to_le_bytes());
    fs::write(bin_path, bytes).unwrap();
    let geometry = prepared(&f);
    assert_eq!(
        status(&geometry, "model-import-geometry"),
        CacheStatus::Produced
    );
    assert_eq!(status(&geometry, "model-import-image"), CacheStatus::Hit);
    assert_ne!(
        cooked(color.runtime_package_build_input(), "import.mesh.0"),
        cooked(geometry.runtime_package_build_input(), "import.mesh.0")
    );
    assert_eq!(
        cooked(color.runtime_package_build_input(), "import.material.0"),
        cooked(geometry.runtime_package_build_input(), "import.material.0")
    );
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[127u8; 16])
            .unwrap();
    }
    fs::write(f.project.root.join("Models/direction.png"), png).unwrap();
    let texture = prepared(&f);
    assert_eq!(status(&texture, "model-import-geometry"), CacheStatus::Hit);
    assert_eq!(
        status(&texture, "model-import-image"),
        CacheStatus::Produced
    );
    assert_eq!(
        cooked(geometry.runtime_package_build_input(), "import.mesh.0"),
        cooked(texture.runtime_package_build_input(), "import.mesh.0")
    );
    assert_eq!(
        cooked(geometry.runtime_package_build_input(), "import.material.0"),
        cooked(texture.runtime_package_build_input(), "import.material.0")
    );
    assert_ne!(
        geometry.runtime_package_build_input().texture_payloads[0].rgba8,
        texture.runtime_package_build_input().texture_payloads[0].rgba8
    );
    // The unchanged material must still acquire the new pixel generation through
    // the actual Runtime loader/Prepare consumer, without a material recook.
    let mut manager = engine_runtime::render_resource::RenderResourceManager::new();
    let producer = engine_runtime::render_asset_production::RuntimeRenderAssetProducer::new();
    let reference = engine_runtime::render_contract::RenderAssetReference {
        expected_kind: engine_runtime::render_contract::RenderAssetKind::Material,
        reference: renderer(geometry.runtime_package_build_input()).materials["surface0"]
            .0
            .clone(),
    };
    let (_, mut old_loader) =
        super::import_scene::package(&f, geometry.runtime_package_build_input());
    let old = producer
        .prepare_cooked_from_package(0, &reference, &mut old_loader, &mut manager)
        .unwrap();
    let (_, mut new_loader) =
        super::import_scene::package(&f, texture.runtime_package_build_input());
    let new = producer
        .prepare_cooked_from_package(1, &reference, &mut new_loader, &mut manager)
        .unwrap();
    assert_eq!(old.cooked_digest, new.cooked_digest);
    assert_ne!(old.textures[0].payload.rgba8, new.textures[0].payload.rgba8);
    assert_ne!(old.textures[0].handle, new.textures[0].handle);
    assert!(old.textures[0]
        .binding
        .validate_residency(&manager)
        .is_err());
    new.textures[0]
        .binding
        .validate_residency(&manager)
        .unwrap();
}
#[test]
fn cache_import_missing_dependency_and_budget_fail_keep_complete_old_package() {
    let mut f = ModelFixture::new();
    let prepared = prepared(&f);
    let input = prepared.runtime_package_build_input();
    let root = f.project.root.join("published");
    let request = RuntimePackageBuildRequest::dev_desktop(&root, "main");
    assert_eq!(
        RuntimePackageBuilder::build(&request, input).status,
        RuntimePackageBuildStatus::Success
    );
    let before = tree(&root);
    let source_revision = prepared.lineage().revision_id().to_string();
    let (compiler, lease) = f.project.compiler();
    let count = std::cell::Cell::new(0);
    compiler
        .prepare_cancellable(&lease, TargetProfile::WindowsDev, None, &|| {
            count.set(count.get() + 1);
            false
        })
        .unwrap();
    for stop_at in [1, 3, count.get()] {
        let seen = std::cell::Cell::new(0);
        let error = compiler
            .prepare_cancellable(&lease, TargetProfile::WindowsDev, None, &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop_at
            })
            .unwrap_err();
        assert!(error.code().ends_with("import_cancelled"), "{error:?}");
        assert_eq!(tree(&root), before);
    }
    drop(lease);
    let original_bin = fs::read(f.project.root.join("Models/quad.bin")).unwrap();
    fs::remove_file(f.project.root.join("Models/quad.bin")).unwrap();
    let (compiler, lease) = f.project.compiler();
    assert_ne!(source_revision, lease.snapshot().revision.revision_id);
    assert!(compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsDev,
            &f.project.root.join(".aife/import-cache")
        )
        .is_err());
    assert_eq!(tree(&root), before);
    drop(lease);
    let mut changed = input.clone();
    changed.assets[0].name = "cancelled replacement".into();
    for stop_at in [1, 2, 3] {
        let count = std::cell::Cell::new(0);
        let cancelled = || {
            count.set(count.get() + 1);
            count.get() >= stop_at
        };
        let report = RuntimePackageBuilder::build_cancellable(&request, &changed, &cancelled);
        assert_ne!(report.status, RuntimePackageBuildStatus::Success);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "output_publish_cancelled"),
            "{report:?}"
        );
        assert_eq!(tree(&root), before);
        assert!(!fs::read_dir(&f.project.root)
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().contains("staging")));
    }
    fs::write(f.project.root.join("Models/quad.bin"), original_bin).unwrap();
    f.descriptor["budgets"] = serde_json::to_value(ImportBudgets::default()).unwrap();
    f.descriptor["budgets"]["cookedBytes"] = json!(1);
    f.project.write("Assets/import.json", &f.descriptor);
    let (compiler, lease) = f.project.compiler();
    let error = compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err();
    assert!(
        error.code().ends_with("import_cooked_budget_exceeded"),
        "{error:?}"
    );
    assert_eq!(tree(&root), before);
}
fn tree(root: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(
        base: &std::path::Path,
        path: &std::path::Path,
        result: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(base).unwrap().to_string_lossy().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = Default::default();
    walk(root, root, &mut result);
    result
}

fn add_instances(f: &ModelFixture) {
    let mut source = scene();
    for (id, overrides) in [
        (
            "instance-a",
            json!([
                {"targetSourceEntityId":"authored-node-1","componentType":"MeshRenderer3D","fieldPath":"materials.surface0","value":{"id":"blue","type":"material","guid":"guid-blue"}},
                {"targetSourceEntityId":"authored-node-1","componentType":"Transform3D","fieldPath":"translation","value":[5,6,7]}
            ]),
        ),
        ("instance-b", json!([])),
    ] {
        source["entities"].as_array_mut().unwrap().push(json!({"id":id,"name":id,"enabled":true,"parentId":null,"siblingOrder":3,"components":[
            {"componentType":"Transform3D","data":{"schemaVersion":"transform3d.v1","translation":[0,0,0],"rotation":{"kind":"quaternion","xyzw":[0,0,0,1]},"scale":[1,1,1]}},
            {"componentType":"engine.prefab_instance","data":{"source":{"id":"import.prefab.0","type":"prefab","guid":"guid-import-prefab-0","sub_asset":"stable-prefab-0"},"overrides":overrides}}
        ]}));
    }
    let typed: SpatialSourceScene = serde_json::from_value(source.clone()).unwrap();
    assert!(typed
        .canonicalize()
        .unwrap_err()
        .code
        .ends_with("prefab_expansion_required"));
    f.project.write("Scenes/main.json", &source);
}
fn assert_instances(f: &ModelFixture, input: &RuntimePackageBuildInput) {
    use engine_runtime::runtime_package::load_runtime_package;
    let root = f.project.root.join("instances");
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&root, "main"),
        input,
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{report:?}"
    );
    let loaded = load_runtime_package(&root);
    assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
    let package = loaded.value.unwrap();
    let mut world = engine_runtime::world::World::new();
    let mut hydrator =
        engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator::from_package(&package);
    let report = hydrator.hydrate_active_scene(&package, &mut world);
    assert!(!report.has_errors(), "{report:?}");
    for (id, expected) in [
        ("instance-a__authored-node-1", "blue"),
        ("instance-b__authored-node-1", "import.material.0"),
    ] {
        let e = input.scenes[0]
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        let RuntimeSpatial::World3D {
            mesh_renderer: Some(renderer),
            ..
        } = e.spatial.as_ref().unwrap()
        else {
            panic!()
        };
        assert_eq!(renderer.materials["surface0"].0.id, expected);
        assert!(world
            .transform3d(&engine_runtime::ids::EntityId::from(id))
            .is_some());
    }
    assert_eq!(
        world
            .transform3d(&engine_runtime::ids::EntityId::from(
                "instance-a__authored-node-1"
            ))
            .unwrap()
            .translation,
        [5., 6., 7.]
    );
}
#[test]
fn reimport_instance_override_removed_target_slot_and_guid_are_rejected() {
    let f = ModelFixture::new();
    add_instances(&f);
    f.input();
    let mut source: Value =
        serde_json::from_slice(&fs::read(f.project.root.join("Scenes/main.json")).unwrap())
            .unwrap();
    for (case, pointer, value) in [
        (
            "node",
            "/entities/2/components/1/data/overrides/0/targetSourceEntityId",
            json!("removed-node"),
        ),
        (
            "slot",
            "/entities/2/components/1/data/overrides/0/fieldPath",
            json!("materials.removed-slot"),
        ),
        (
            "prefab identity",
            "/entities/2/components/1/data/source/guid",
            json!("wrong-guid"),
        ),
        (
            "material identity",
            "/entities/2/components/1/data/overrides/0/value/guid",
            json!("wrong-guid"),
        ),
    ] {
        let original = source.pointer(pointer).unwrap().clone();
        *source.pointer_mut(pointer).unwrap() = value;
        f.project.write("Scenes/main.json", &source);
        let (compiler, lease) = f.project.compiler();
        let error = compiler
            .prepare(&lease, TargetProfile::WindowsDev)
            .unwrap_err();
        let location = error.diagnostics()[0].location.as_ref().unwrap();
        assert_eq!(
            location.source_path, "Scenes/main.json",
            "{case}: {error:?}"
        );
        let expected = match case {
            "prefab identity" => "/entities/2/components/1/data/source",
            "material identity" => "/entities/2/components/1/data/overrides/0/value",
            _ => pointer,
        };
        assert!(
            location
                .field_path
                .as_deref()
                .unwrap()
                .starts_with(expected),
            "{case}: {error:?}"
        );
        drop(lease);
        *source.pointer_mut(pointer).unwrap() = original;
    }
}
#[test]
fn cache_target_profile_recipe_and_corruption_do_not_reuse_stale_imports() {
    let f = ModelFixture::new();
    let cold = prepared(&f);
    let initial = cold
        .producer_reports()
        .iter()
        .find(|r| r.producer_id == "model-import")
        .unwrap();
    let cached = std::path::PathBuf::from(initial.artifact_path.as_ref().unwrap());
    let envelope_path = cached.join("artifact-envelope.json");
    let mut envelope: Value = serde_json::from_slice(&fs::read(&envelope_path).unwrap()).unwrap();
    envelope["producerRecipeVersion"] = json!("obsolete-import-recipe");
    fs::write(&envelope_path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let repaired = prepared(&f);
    assert_eq!(status(&repaired, "model-import"), CacheStatus::Produced);
    assert_eq!(status(&repaired, "model-import-geometry"), CacheStatus::Hit);
    fs::write(cached.join("payload.json"), b"corrupt").unwrap();
    let repaired = prepared(&f);
    assert_eq!(status(&repaired, "model-import"), CacheStatus::Produced);
    f.project.write("BuildProfiles/windows.release.json", &serde_json::from_str::<Value>(include_str!("../../../../../samples/platform_3d_benchmark_project/BuildProfiles/windows.release.json")).unwrap());
    let (compiler, lease) = f.project.compiler();
    let release = compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &f.project.root.join(".aife/import-cache"),
        )
        .unwrap();
    assert_eq!(status(&release, "model-import"), CacheStatus::Produced);
    assert_eq!(
        status(&release, "model-import-geometry"),
        CacheStatus::Produced
    );
    let dev = compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsDev,
            &f.project.root.join(".aife/import-cache"),
        )
        .unwrap();
    // WindowsDev was already prepared above. Returning from Release must reuse
    // that exact product, while keeping the Release key distinct.
    assert_eq!(status(&dev, "model-import"), CacheStatus::Hit);
    assert_eq!(
        dev.producer_reports()
            .iter()
            .find(|r| r.producer_id == "model-import")
            .unwrap()
            .recipe_key,
        cold.producer_reports()
            .iter()
            .find(|r| r.producer_id == "model-import")
            .unwrap()
            .recipe_key
    );
    assert_ne!(
        release
            .producer_reports()
            .iter()
            .find(|r| r.producer_id == "model-import")
            .unwrap()
            .recipe_key,
        dev.producer_reports()
            .iter()
            .find(|r| r.producer_id == "model-import")
            .unwrap()
            .recipe_key
    );
    drop(lease);
    let profile = json!({"schemaVersion":"build-profile.v1","profile":"dev","target":"windows","runtimePackageMode":"debug-readable","frameLimit":3,"headlessSurfaceGate":true,"realWindowSmoke":"optional"});
    f.project.write("BuildProfiles/windows.dev.json", &profile);
    let explicit = prepared(&f);
    assert_eq!(status(&explicit, "model-import"), CacheStatus::Produced);
    let mut changed = profile;
    changed["frameLimit"] = json!(7);
    f.project.write("BuildProfiles/windows.dev.json", &changed);
    let changed = prepared(&f);
    assert_eq!(status(&changed, "model-import"), CacheStatus::Produced);
    assert_ne!(
        explicit
            .producer_reports()
            .iter()
            .find(|r| r.producer_id == "model-import")
            .unwrap()
            .recipe_key,
        changed
            .producer_reports()
            .iter()
            .find(|r| r.producer_id == "model-import")
            .unwrap()
            .recipe_key
    );
}

#[test]
fn reimport_deleted_node_does_not_silently_drop_instance_override() {
    let mut f = ModelFixture::new();
    add_instances(&f);
    f.input();
    f.model["nodes"].as_array_mut().unwrap().pop();
    f.model["nodes"][0]["children"] = json!([]);
    f.descriptor["nodes"].as_array_mut().unwrap().pop();
    f.descriptor["outputs"]
        .as_array_mut()
        .unwrap()
        .retain(|o| o["kind"] == "prefab");
    approve_fingerprints(&mut f);
    let (compiler, lease) = f.project.compiler();
    let error = compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err();
    assert!(
        error.code().ends_with("prefab_override_invalid"),
        "{error:?}"
    );
    assert!(error.message().contains("target was removed"), "{error:?}");
    assert_eq!(
        error.source_location().unwrap().field_path.as_deref(),
        Some("/entities/2/components/1/data/overrides/0/targetSourceEntityId")
    );
}

#[test]
fn reimport_instance_contract_applies_typed_overrides_and_rejects_conflicts() {
    let f = ModelFixture::new();
    let mut source = scene();
    let mut child = source["entities"][0].clone();
    child["id"] = json!("native-child");
    child["parentId"] = json!("native-root");
    child["components"]
        .as_array_mut()
        .unwrap()
        .push(json!({"componentType":"project.stats","data":{"hp":5}}));
    let root = json!({"id":"native-root","name":"Root","enabled":true,"parentId":null,"siblingOrder":0,"components":[child["components"][0].clone()]});
    let mut prefab = json!({"schemaVersion":"authoring-prefab-asset.v2","id":"native-prefab","name":"Native","rootEntityId":"native-root","entities":[root,child]});
    f.project.write("Prefabs/native.json", &prefab);
    let mut placement = prefab["entities"][0].clone();
    placement["id"] = json!("native-instance");
    placement["components"].as_array_mut().unwrap().push(json!({"componentType":"engine.prefab_instance","data":{"source":{"id":"native-prefab","type":"prefab"},"overrides":[
        {"targetSourceEntityId":"native-child","componentType":"Transform3D","fieldPath":"scale","value":[2,3,4]},
        {"targetSourceEntityId":"native-child","componentType":"Transform3D","fieldPath":"rotation","value":{"kind":"eulerDegrees","xyz":[0,90,0]}},
        {"targetSourceEntityId":"native-child","componentType":"Visibility3D","fieldPath":"visible","value":false},
        {"targetSourceEntityId":"native-child","componentType":"project.stats","fieldPath":"hp","value":7}
    ]}}));
    source["entities"]
        .as_array_mut()
        .unwrap()
        .push(placement.clone());
    f.project.write("Scenes/main.json", &source);
    let input = f.input();
    let entity = input.scenes[0]
        .entities
        .iter()
        .find(|e| e.id == "native-instance__native-child")
        .unwrap();
    let RuntimeSpatial::World3D {
        transform,
        visibility: Some(visibility),
        ..
    } = entity.spatial.as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(transform.scale, [2., 3., 4.]);
    assert!(!visibility.visible);
    assert!((transform.rotation_xyzw[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    assert_eq!(
        entity
            .components
            .iter()
            .find(|c| c.component_type == "project.stats")
            .unwrap()
            .data["hp"],
        7
    );
    let (package, _) = super::import_scene::package(&f, &input);
    let mut world = engine_runtime::world::World::new();
    let mut hydrator =
        engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    assert_eq!(
        world
            .transform3d(&engine_runtime::ids::EntityId::from(
                "native-instance__native-child"
            ))
            .unwrap()
            .scale,
        [2., 3., 4.]
    );
    let duplicate = source["entities"][2]["components"][1]["data"]["overrides"][0].clone();
    source["entities"][2]["components"][1]["data"]["overrides"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    f.project.write("Scenes/main.json", &source);
    let (compiler, lease) = f.project.compiler();
    assert!(compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err()
        .message()
        .contains("unique field path"));
    drop(lease);
    source["entities"][2] = placement;
    f.project.write("Scenes/main.json", &source);
    let nested = source["entities"][2]["components"][1].clone();
    prefab["entities"][0]["components"]
        .as_array_mut()
        .unwrap()
        .push(nested);
    f.project.write("Prefabs/native.json", &prefab);
    let (compiler, lease) = f.project.compiler();
    assert!(compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err()
        .code()
        .ends_with("prefab_nested_unsupported"));
}
