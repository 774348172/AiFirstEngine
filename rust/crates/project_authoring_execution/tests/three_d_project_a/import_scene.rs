use super::*;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::render_asset_production::*;
use engine_runtime::render_contract::*;
use engine_runtime::render_resource::RenderResourceManager;
use engine_runtime::runtime_asset_loader::RuntimeAssetLoader;
use engine_runtime::runtime_package::*;
use engine_runtime::runtime_package_builder::*;

pub(super) struct ModelFixture {
    pub project: Fixture,
    pub model: Value,
    pub descriptor: Value,
}
impl ModelFixture {
    pub fn new() -> Self {
        let project = Fixture::new();
        project.write(
            "Input/default.json",
            &serde_json::to_value(
                engine_runtime::input_mapping::InputMappingAsset::gameplay_default(),
            )
            .unwrap(),
        );
        // Hand-authored asymmetric quad: four XY positions, +Z normals, top-left UV origin.
        let mut bytes = Vec::new();
        for value in [-1.0f32, -1., 0., 1., -1., 0., 1., 1., 0., -1., 1., 0.] {
            bytes.extend(value.to_le_bytes());
        }
        for _ in 0..4 {
            for value in [0.0f32, 0., 1.] {
                bytes.extend(value.to_le_bytes());
            }
        }
        for value in [0.0f32, 1., 1., 1., 1., 0., 0., 0.] {
            bytes.extend(value.to_le_bytes());
        }
        for value in [0u16, 1, 2, 0, 2, 3] {
            bytes.extend(value.to_le_bytes());
        }
        fs::create_dir_all(project.root.join("Models")).unwrap();
        fs::write(project.root.join("Models/quad.bin"), bytes).unwrap();
        let mut pixels = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut pixels, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                ])
                .unwrap();
        }
        fs::write(project.root.join("Models/direction.png"), pixels).unwrap();
        let model = json!({"asset":{"version":"2.0","generator":"359 hand-authored numerical fixture"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"name":"AuthoredRoot","translation":[2,3,4],"children":[1]},{"name":"Quad","mesh":0}],
            "meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},"indices":3,"material":0}]}],
            "materials":[{"extensions":{"KHR_materials_unlit":{}},"pbrMetallicRoughness":{"baseColorFactor":[1,1,1,1],"baseColorTexture":{"index":0}}}],
            "textures":[{"source":0,"sampler":0}],"samplers":[{"minFilter":9728,"magFilter":9728,"wrapS":33071,"wrapT":33071}],"images":[{"uri":"direction.png"}],
            "buffers":[{"uri":"quad.bin","byteLength":140}],"bufferViews":[{"buffer":0,"byteLength":48},{"buffer":0,"byteOffset":48,"byteLength":48},{"buffer":0,"byteOffset":96,"byteLength":32},{"buffer":0,"byteOffset":128,"byteLength":12}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":4,"type":"VEC3","min":[-1,-1,0],"max":[1,1,0]},{"bufferView":1,"componentType":5126,"count":4,"type":"VEC3"},{"bufferView":2,"componentType":5126,"count":4,"type":"VEC2"},{"bufferView":3,"componentType":5123,"count":6,"type":"SCALAR"}],"extensionsUsed":["KHR_materials_unlit"]});
        let mut descriptor = strict_json(IMPORT).unwrap();
        descriptor["nodes"] = json!([]);
        descriptor["outputs"] = json!([]);
        let mut f = Self {
            project,
            model,
            descriptor,
        };
        f.save();
        f
    }
    pub fn save(&mut self) {
        self.descriptor = descriptor_for(&self.model, self.descriptor.clone());
        let source = self.descriptor["source"].as_str().unwrap();
        self.project.write(source, &self.model);
        self.project.write("Assets/import.json", &self.descriptor);
    }
    pub fn input(&self) -> RuntimePackageBuildInput {
        let (compiler, lease) = self.project.compiler();
        let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
        prepared.runtime_package_build_input().clone()
    }
    pub fn report(&self) -> Value {
        let (compiler, lease) = self.project.compiler();
        serde_json::to_value(
            compiler
                .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
                .unwrap()
                .model_imports
                .remove(0),
        )
        .unwrap()
    }
    pub fn error(&self, code: &str, field: &str) {
        let error = self.project.check_error();
        assert!(error.code().ends_with(code), "{error:?}");
        assert!(
            error.diagnostics()[0]
                .location
                .as_ref()
                .unwrap()
                .field_path
                .as_deref()
                .unwrap()
                .contains(field),
            "{error:?}"
        );
    }
}

// Test authoring helper, independent of importer internals. Identity values are explicit
// fixture constants; recomputing matching evidence never allocates production GUIDs.
pub(super) fn descriptor_for(model: &Value, mut descriptor: Value) -> Value {
    let fingerprint = |value: &Value| sha256_prefixed(&serde_json::to_vec(value).unwrap());
    let mut outputs = Vec::new();
    for (kind, collection) in [
        ("mesh", "meshes"),
        ("material", "materials"),
        ("texture", "images"),
        ("prefab", "scenes"),
    ] {
        for (index, value) in model[collection]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let mut slots = Vec::new();
            if kind == "mesh" {
                for n in 0..value["primitives"].as_array().unwrap().len() {
                    slots.push(json!({"key":format!("slot-key-{n}"),"name":format!("surface{n}"),"sourceLocator":format!("/meshes/{index}/primitives/{n}")}));
                }
            }
            outputs.push(json!({"key":format!("stable-{kind}-{index}"),"assetId":format!("import.{kind}.{index}"),"assetGuid":format!("guid-import-{kind}-{index}"),"kind":kind,"sourceLocator":format!("/{collection}/{index}"),"sourceFingerprint":fingerprint(value),"materialSlots":slots}));
        }
    }
    descriptor["outputs"] = json!(outputs);
    descriptor["nodes"]=json!(model["nodes"].as_array().into_iter().flatten().enumerate().map(|(i,n)|json!({"key":format!("stable-node-{i}"),"entityId":format!("authored-node-{i}"),"sourceLocator":format!("/nodes/{i}"),"sourceFingerprint":fingerprint(n)})).collect::<Vec<_>>());
    descriptor
}
pub(super) fn cooked(input: &RuntimePackageBuildInput, id: &str) -> CookedRenderAsset {
    let asset = input.assets.iter().find(|a| a.asset_id == id).unwrap();
    RenderContract::<CookedRenderAsset>::decode(
        std::str::from_utf8(asset.runtime_payload.as_ref().unwrap()).unwrap(),
    )
    .unwrap()
    .data
}
fn mesh(input: &RuntimePackageBuildInput) -> CookedGeometry {
    match cooked(input, "import.mesh.0").asset {
        RenderAssetData::Mesh(m) => m,
        _ => panic!("mesh"),
    }
}
fn material(input: &RuntimePackageBuildInput) -> CookedMaterial {
    match cooked(input, "import.material.0").asset {
        RenderAssetData::Material(m) => m,
        _ => panic!("material"),
    }
}
pub(super) fn package(
    f: &ModelFixture,
    input: &RuntimePackageBuildInput,
) -> (RuntimePackage, RuntimeAssetLoader) {
    let root = f.project.root.join("package");
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
    let mut loader = RuntimeAssetLoader::new(
        &root,
        package.runtime_asset_index.clone(),
        package.runtime_asset_mount_table.clone(),
    );
    assert!(loader.mount_bundle("startup"));
    (package, loader)
}
pub(super) fn renderer(input: &RuntimePackageBuildInput) -> MeshRenderer3D {
    let p: &RuntimePackageSourcePrefab = input
        .prefabs
        .iter()
        .find(|p| p.prefab_id == "import.prefab.0")
        .unwrap();
    let prefab: RuntimePrefabV2 = serde_json::from_value(p.document.clone()).unwrap();
    prefab
        .entities
        .iter()
        .find_map(|e| match e.spatial()? {
            RuntimeSpatial::World3D { mesh_renderer, .. } => mesh_renderer.clone(),
        })
        .unwrap()
}
fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
fn import_material_strict_default_rejects_pbr_and_explicit_degradation_lists_semantics() {
    let mut f = ModelFixture::new();
    f.model["materials"][0]
        .as_object_mut()
        .unwrap()
        .remove("extensions");
    f.model["materials"][0]["normalTexture"] = json!({"index":0,"scale":0.5});
    f.model["materials"][0]["occlusionTexture"] = json!({"index":0});
    f.model["materials"][0]["emissiveTexture"] = json!({"index":0});
    f.model["materials"][0]["pbrMetallicRoughness"]["metallicRoughnessTexture"] =
        json!({"index":0});
    f.save();
    f.error("material.model_unsupported", "/materials/0");
    f.descriptor["materialStrategy"] = json!("baseColorOnly");
    f.save();
    let report = f.report();
    assert_eq!(report["status"], "degraded");
    assert_eq!(report["stage"], "assembly");
    assert_eq!(report["assemblyAvailable"], true);
    let paths: Vec<_> = report["notices"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["outcome"] == "discarded")
        .map(|n| n["fieldPath"].as_str().unwrap())
        .collect();
    for field in [
        "/materials/0",
        "/materials/0/pbrMetallicRoughness/metallicFactor",
        "/materials/0/pbrMetallicRoughness/roughnessFactor",
        "/materials/0/normalTexture",
        "/materials/0/occlusionTexture",
        "/materials/0/emissiveTexture",
        "/materials/0/emissiveFactor",
        "/materials/0/pbrMetallicRoughness/metallicRoughnessTexture",
    ] {
        assert!(paths.contains(&field), "missing {field}: {report}");
    }
    assert_eq!(material(&f.input()).preset, MaterialPreset::Unlit);
    fs::write(f.project.root.join("Models/direction.png"), b"broken image").unwrap();
    f.error("import_image_invalid", "/images/0");
}

#[test]
fn import_material_alpha_cull_color_and_all_sampler_combinations_reach_cooked_bindings() {
    let mut f = ModelFixture::new();
    f.model["materials"][0]["alphaMode"] = json!("BLEND");
    f.model["materials"][0]["doubleSided"] = json!(true);
    f.model["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"] =
        json!([0.25, 0.5, 0.75, 0.4]);
    for min in [9728, 9729] {
        for mag in [9728, 9729] {
            for s in [10497, 33071, 33648] {
                for t in [10497, 33071, 33648] {
                    f.model["samplers"][0] =
                        json!({"minFilter":min,"magFilter":mag,"wrapS":s,"wrapT":t});
                    f.save();
                    let m = material(&f.input());
                    let sampler = m.base_color_texture_sampling.unwrap();
                    assert_eq!(
                        sampler.min_filter,
                        if min == 9728 {
                            TextureFilter::Nearest
                        } else {
                            TextureFilter::Linear
                        }
                    );
                    assert_eq!(
                        sampler.mag_filter,
                        if mag == 9728 {
                            TextureFilter::Nearest
                        } else {
                            TextureFilter::Linear
                        }
                    );
                    let wrap = |v| match v {
                        10497 => TextureWrap::Repeat,
                        33071 => TextureWrap::ClampToEdge,
                        _ => TextureWrap::MirroredRepeat,
                    };
                    assert_eq!((sampler.wrap_s, sampler.wrap_t), (wrap(s), wrap(t)));
                    assert_eq!(
                        (m.blend, m.cull, m.depth),
                        (
                            MaterialBlend::AlphaBlend,
                            MaterialCull::None,
                            MaterialDepth::TestOnly
                        )
                    );
                    assert_eq!(
                        m.parameters["baseColor"],
                        MaterialParameter::Vector4([0.25, 0.5, 0.75, 0.4])
                    );
                }
            }
        }
    }
    f.model["textures"][0]
        .as_object_mut()
        .unwrap()
        .remove("sampler");
    f.save();
    assert_eq!(
        material(&f.input())
            .base_color_texture_sampling
            .unwrap()
            .key(),
        "linear:linear:repeat:repeat"
    );
    assert!(f.report()["notices"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |n| n["outcome"] == "resolvedSampler" && n["message"] == "linear:linear:repeat:repeat"
        ));
}

#[test]
fn import_material_unsupported_uv_and_texture_transform_are_located() {
    for (case, code, field) in [
        ("uv", "import_uv_missing", "/attributes/TEXCOORD_0"),
        (
            "transform",
            "import_extension_unsupported",
            "/extensions/KHR_texture_transform",
        ),
    ] {
        let mut f = ModelFixture::new();
        match case {
            "mask" => f.model["materials"][0]["alphaMode"] = json!("MASK"),
            "mips" => f.model["samplers"][0]["minFilter"] = json!(9987),
            "uv" => {
                f.model["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["texCoord"] =
                    json!(1)
            }
            _ => {
                f.model["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["extensions"] =
                    json!({"KHR_texture_transform":{"offset":[0.1,0.2]}})
            }
        }
        f.save();
        f.error(code, field);
    }
}

#[test]
fn import_material_mask_default_and_explicit_cutoff_survive_cook_package_and_prepare() {
    let mut f = ModelFixture::new();
    f.model["materials"][0]["alphaMode"] = json!("MASK");
    for cutoff in [None, Some(0.0), Some(0.75), Some(1.0)] {
        f.model["materials"][0]
            .as_object_mut()
            .unwrap()
            .remove("alphaCutoff");
        if let Some(cutoff) = cutoff {
            f.model["materials"][0]["alphaCutoff"] = json!(cutoff);
        }
        f.save();
        let input = f.input();
        let cooked = material(&input);
        assert_eq!(cooked.blend, MaterialBlend::AlphaMask);
        assert_eq!(cooked.depth, MaterialDepth::TestAndWrite);
        assert!(!cooked.opaque_alpha);
        assert_eq!(
            cooked.parameters["alphaCutoff"],
            MaterialParameter::Float(cutoff.unwrap_or(0.5))
        );
        let (_package, mut loader) = package(&f, &input);
        let mut manager = RenderResourceManager::new();
        let prepared = RuntimeRenderAssetProducer::new()
            .prepare_mesh_renderer(0, &renderer(&input), &mut loader, &mut manager)
            .unwrap();
        assert_eq!(prepared.geometry.len(), 1);
        assert_eq!(prepared.textures.len(), 1);
    }
    for cutoff in [-0.1, 1.1] {
        f.model["materials"][0]["alphaCutoff"] = json!(cutoff);
        f.save();
        f.error("material.alpha_cutoff_invalid", "/alphaCutoff");
    }
}

#[test]
fn import_scene_mesh_prefab_identity_package_dependencies_and_real_prepare_roundtrip() {
    let f = ModelFixture::new();
    let first = f.input();
    let second = f.input();
    assert_eq!(
        first.assembly_input_digest().unwrap(),
        second.assembly_input_digest().unwrap()
    );
    let m = mesh(&first);
    assert_eq!((m.vertex_count, m.index_count, m.vertex_stride), (4, 6, 48));
    assert_eq!(
        &floats(&m.vertex_bytes)[..8],
        &[-1., -1., 0., 0., 0., 1., 0., 1.]
    );
    assert_eq!((m.bounds.min, m.bounds.max), ([-1., -1., 0.], [1., 1., 0.]));
    assert_eq!(m.material_slots, ["surface0"]);
    let (package, mut loader) = package(&f, &first);
    let mut manager = RenderResourceManager::new();
    let bound = RuntimeRenderAssetProducer::new()
        .prepare_mesh_renderer(0, &renderer(&first), &mut loader, &mut manager)
        .unwrap();
    assert_eq!(bound.geometry.len(), 1);
    assert_eq!(
        bound.textures[0].payload.rgba8.as_slice(),
        [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]
    );
    for kind in ["mesh", "material", "texture", "prefab"] {
        let r = package
            .runtime_asset_index
            .record_by_guid(&format!("guid-import-{kind}-0"))
            .unwrap();
        assert_eq!(
            r.sub_asset_id.as_deref(),
            Some(format!("stable-{kind}-0").as_str())
        );
        assert_eq!(r.source_map_debug.as_deref(), Some("Assets/import.json"));
    }
    assert_eq!(
        package
            .runtime_asset_index
            .record_by_guid("guid-import-material-0")
            .unwrap()
            .dependencies,
        ["guid-import-texture-0"]
    );
    let mut world = engine_runtime::world::World::new();
    let mut hydrator =
        engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let before = world.entity_count();
    let (instance, report) = hydrator
        .instance_loader_mut()
        .instantiate_prefab_from_package(
            &package,
            RuntimeAssetRef {
                id: "import.prefab.0".into(),
                asset_type: "prefab".into(),
                guid: Some("guid-import-prefab-0".into()),
                sub_asset: Some("stable-prefab-0".into()),
            },
            None,
            None,
            &mut world,
        );
    assert!(instance.is_some(), "{report:?}");
    assert_eq!(world.entity_count(), before + 3);
}

#[test]
fn import_scene_conversion_changes_geometry_normals_winding_bounds_and_nodes_once() {
    let mut f = ModelFixture::new();
    f.descriptor["coordinates"] = json!({"upAxis":"z","handedness":"left","metersPerUnit":0.01});
    f.model["nodes"][0]["scale"] = json!([-2, 3, 4]);
    f.model["nodes"][1]["translation"] = json!([5, 6, 7]);
    f.save();
    let input = f.input();
    let m = mesh(&input);
    let v = floats(&m.vertex_bytes);
    assert_eq!(&v[..6], &[-0.01, 0., -0.01, 0., 1., 0.]);
    assert_eq!(&m.index_bytes[..6], &[0, 0, 2, 0, 1, 0]);
    assert_eq!(
        (m.bounds.min, m.bounds.max),
        ([-0.01, 0., -0.01], [0.01, 0., 0.01])
    );
    let p = &input.prefabs[0].document;
    let entities = p["entities"].as_array().unwrap();
    let root = entities
        .iter()
        .find(|e| e["id"] == "authored-node-0")
        .unwrap();
    let child = entities
        .iter()
        .find(|e| e["id"] == "authored-node-1")
        .unwrap();
    let rt: CanonicalTransform3D =
        serde_json::from_value(root["spatial"]["transform"].clone()).unwrap();
    let ct: CanonicalTransform3D =
        serde_json::from_value(child["spatial"]["transform"].clone()).unwrap();
    assert_eq!(rt.translation, [0.02, 0.04, 0.03]);
    assert_eq!(rt.scale, [-2., 4., 3.]);
    // Source metres-per-unit is f32; calculate its exact f64 product before rounding.
    assert_eq!(
        ct.translation,
        [5., 7., 6.].map(|v| (v * f64::from(0.01f32)) as f32)
    );
    assert_eq!(child["parentId"], "authored-node-0");
    // C*(parent*(child*p)) = (-.06,.32,.18); scale is applied once per node.
    let world: Vec<_> = (0..3)
        .map(|i| rt.translation[i] + rt.scale[i] * (ct.translation[i] + v[i]))
        .collect();
    for (a, b) in world.iter().zip([-0.06, 0.32, 0.18]) {
        assert!((a - b).abs() < 1e-6, "{world:?}");
    }
}

#[test]
fn import_scene_shear_singular_and_nonunit_rotation_fail_at_source_node() {
    for case in ["shear", "singular", "rotation"] {
        let mut f = ModelFixture::new();
        f.model["nodes"][1] = match case {
            "shear" => json!({"mesh":0,"matrix":[1,0,0,0,0.2,1,0,0,0,0,1,0,0,0,0,1]}),
            "singular" => json!({"mesh":0,"scale":[1,0,1]}),
            _ => json!({"mesh":0,"rotation":[0,0,0,2]}),
        };
        f.save();
        f.error("import_transform_unsupported", "/nodes/1");
    }
}

#[test]
fn import_scene_mapping_mismatch_and_unconsumed_outputs_fail_without_source_writes() {
    let mut f = ModelFixture::new();
    let before = fs::read(f.project.root.join("Assets/import.json")).unwrap();
    f.model["nodes"][1]["name"] = json!("Changed");
    f.project.write("Models/scene.gltf", &f.model);
    f.error("mapping_invalid", "/nodes/");
    assert_eq!(
        before,
        fs::read(f.project.root.join("Assets/import.json")).unwrap()
    );
    f.save();
    f.descriptor["outputs"][0]["kind"] = json!("material");
    f.descriptor["outputs"][0]["materialSlots"] = json!([]);
    f.project.write("Assets/import.json", &f.descriptor);
    f.error("mapping_invalid", "/outputs");
}

#[test]
fn import_scene_multisubmesh_keeps_committed_slot_names_and_material_binding() {
    let mut f = ModelFixture::new();
    let mut p = f.model["meshes"][0]["primitives"][0].clone();
    p["material"] = json!(1);
    f.model["meshes"][0]["primitives"]
        .as_array_mut()
        .unwrap()
        .push(p);
    let mut m = f.model["materials"][0].clone();
    m["pbrMetallicRoughness"]["baseColorFactor"] = json!([0, 1, 0, 1]);
    f.model["materials"].as_array_mut().unwrap().push(m);
    f.save();
    f.descriptor["outputs"][0]["materialSlots"][0]["name"] = json!("original-face");
    f.descriptor["outputs"][0]["materialSlots"][1]["name"] = json!("second-face");
    f.project.write("Assets/import.json", &f.descriptor);
    let input = f.input();
    let m = mesh(&input);
    assert_eq!((m.vertex_count, m.index_count), (8, 12));
    assert_eq!(m.material_slots, ["original-face", "second-face"]);
    assert_eq!(m.submeshes[1].first_index, 6);
    assert_eq!(&m.index_bytes[12..18], &[4, 0, 5, 0, 6, 0]);
    let renderer = renderer(&input);
    assert_eq!(
        renderer.materials["second-face"].0.guid.as_deref(),
        Some("guid-import-material-1")
    );
    let (_, mut loader) = package(&f, &input);
    let mut manager = RenderResourceManager::new();
    assert_eq!(
        RuntimeRenderAssetProducer::new()
            .prepare_mesh_renderer(0, &renderer, &mut loader, &mut manager)
            .unwrap()
            .geometry
            .len(),
        2
    );
}

#[test]
fn import_scene_serialized_payload_budget_and_normalized_layout_are_bounded() {
    let mut f = ModelFixture::new();
    let size = f.report()["reservedCookedBytes"].as_u64().unwrap();
    let input = f.input();
    package(&f, &input);
    let package_root = f.project.root.join("package");
    let mut payload_paths: Vec<String> = input
        .assets
        .iter()
        .filter(|asset| asset.source == "Assets/import.json")
        .map(|asset| asset.runtime_uri.clone())
        .collect();
    payload_paths.extend(
        input
            .texture_payloads
            .iter()
            .filter(|t| t.metadata.asset_id.starts_with("import."))
            .map(|t| t.metadata.pixel_data_path.clone()),
    );
    payload_paths.extend(
        input
            .prefabs
            .iter()
            .filter(|p| {
                p.identity
                    .as_ref()
                    .is_some_and(|i| i.source == "Assets/import.json")
            })
            .map(|p| format!("prefabs/{}.json", p.prefab_id)),
    );
    let actual: u64 = payload_paths
        .iter()
        .map(|p| fs::metadata(package_root.join(p)).unwrap().len())
        .sum();
    assert_eq!(size, actual, "budget must cover the actual model payload files, including texture metadata and formatted prefab JSON");
    f.descriptor["budgets"] = serde_json::to_value(ImportBudgets::default()).unwrap();
    f.descriptor["budgets"]["cookedBytes"] = json!(size - 1);
    f.save();
    f.error("import_cooked_budget_exceeded", "");
    f.descriptor["budgets"]["cookedBytes"] = json!(size);
    f.save();
    assert_eq!(f.report()["reservedCookedBytes"], size);
}

pub(super) fn independent_box() -> ModelFixture {
    let mut f = ModelFixture::new();
    f.model = serde_json::from_slice(include_bytes!(
        "../fixtures/three_d_import/khronos_box/glTF/Box.gltf"
    ))
    .unwrap();
    f.descriptor["materialStrategy"] = json!("baseColorOnly");
    fs::write(
        f.project.root.join("Models/Box0.bin"),
        include_bytes!("../fixtures/three_d_import/khronos_box/glTF/Box0.bin"),
    )
    .unwrap();
    f.save();
    f
}

#[test]
fn import_scene_independent_collada_box_gltf_and_glb_reach_package_and_prefab() {
    let mut f = independent_box();
    let input = f.input();
    let m = mesh(&input);
    assert_eq!((m.vertex_count, m.index_count), (24, 36));
    assert_eq!((m.bounds.min, m.bounds.max), ([-0.5; 3], [0.5; 3]));
    assert_eq!(
        material(&input).parameters["baseColor"],
        MaterialParameter::Vector4([0.8, 0., 0., 1.])
    );
    assert_eq!(f.report()["status"], "degraded");
    let (_, mut loader) = package(&f, &input);
    let mut manager = RenderResourceManager::new();
    assert_eq!(
        RuntimeRenderAssetProducer::new()
            .prepare_mesh_renderer(0, &renderer(&input), &mut loader, &mut manager)
            .unwrap()
            .geometry
            .len(),
        1
    );
    let glb = include_bytes!("../fixtures/three_d_import/khronos_box/glTF-Binary/Box.glb");
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    f.model = serde_json::from_slice(&glb[20..20 + json_length]).unwrap();
    f.descriptor["source"] = json!("Models/scene.glb");
    f.save();
    fs::write(f.project.root.join("Models/scene.glb"), glb).unwrap();
    let binary = f.input();
    assert_eq!(mesh(&binary), m);
    assert_eq!(material(&binary), material(&input));
    assert_eq!(binary.prefabs[0].document, input.prefabs[0].document);
}

pub(super) fn embedded_glb(f: &mut ModelFixture) {
    let mut bin = fs::read(f.project.root.join("Models/quad.bin")).unwrap();
    let image = fs::read(f.project.root.join("Models/direction.png")).unwrap();
    let offset = bin.len();
    bin.extend(&image);
    f.model["buffers"][0] = json!({"byteLength":bin.len()});
    f.model["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(json!({"buffer":0,"byteOffset":offset,"byteLength":image.len()}));
    f.model["images"][0] = json!({"bufferView":4,"mimeType":"image/png"});
    f.descriptor["source"] = json!("Models/scene.glb");
    f.save();
    let mut source = serde_json::to_vec(&f.model).unwrap();
    while source.len() % 4 != 0 {
        source.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let mut bytes = b"glTF".to_vec();
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(((28 + source.len() + bin.len()) as u32).to_le_bytes());
    bytes.extend((source.len() as u32).to_le_bytes());
    bytes.extend(b"JSON");
    bytes.extend(source);
    bytes.extend((bin.len() as u32).to_le_bytes());
    bytes.extend(b"BIN\0");
    bytes.extend(bin);
    fs::write(f.project.root.join("Models/scene.glb"), bytes).unwrap();
}

#[test]
fn import_scene_embedded_glb_image_has_manifest_dependency_and_native_material_can_bind_it() {
    let mut f = ModelFixture::new();
    embedded_glb(&mut f);
    let mut authored = strict_json(MATERIAL).unwrap();
    authored["assetId"] = json!("uses-imported-image");
    authored["assetGuid"] = json!("guid-uses-imported-image");
    authored["baseColorTexture"] = json!({"texture":{"id":"import.texture.0","type":"texture","guid":"guid-import-texture-0","sub_asset":"stable-texture-0"},"minFilter":"nearest","magFilter":"nearest","wrapS":"repeat","wrapT":"repeat"});
    f.project
        .write("Assets/uses-imported-image.json", &authored);
    let input = f.input();
    let report = f.report();
    assert!(report["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["locator"] == "/images/0" && d["sourcePath"] == "Models/scene.glb"));
    let (package, mut loader) = package(&f, &input);
    let record = package
        .runtime_asset_index
        .record_by_guid("guid-uses-imported-image")
        .unwrap();
    assert_eq!(record.dependencies, ["guid-import-texture-0"]);
    let mut manager = RenderResourceManager::new();
    let reference = RenderAssetReference {
        expected_kind: RenderAssetKind::Material,
        reference: RuntimeAssetRef {
            id: "uses-imported-image".into(),
            asset_type: "material".into(),
            guid: Some("guid-uses-imported-image".into()),
            sub_asset: None,
        },
    };
    let prepared = RuntimeRenderAssetProducer::new()
        .prepare_cooked_from_package(0, &reference, &mut loader, &mut manager)
        .unwrap();
    assert_eq!(
        prepared.textures[0].payload.rgba8.as_slice(),
        input.texture_payloads[0].rgba8.as_slice()
    );
}

#[test]
fn import_scene_builder_rejects_conflicting_or_incomplete_import_identity() {
    let f = ModelFixture::new();
    let input = f.input();
    for case in ["guid", "key", "source", "dependency"] {
        let mut changed = input.clone();
        let identity = changed.prefabs[0].identity.as_mut().unwrap();
        match case {
            "guid" => identity.asset_guid = "guid-import-mesh-0".into(),
            "key" => identity.sub_asset_id.clear(),
            "source" => identity.source.clear(),
            _ => identity.dependencies.push("missing-guid".into()),
        };
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(
                f.project.root.join(format!("bad-{case}")),
                "main",
            ),
            &changed,
        );
        assert_ne!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{case}: {report:?}"
        );
    }
}

#[test]
fn import_scene_rotated_nonuniform_reflected_hierarchy_hydrates_expected_world_vertex() {
    let mut f = ModelFixture::new();
    let q = std::f32::consts::FRAC_1_SQRT_2;
    f.descriptor["coordinates"] = json!({"upAxis":"z","handedness":"left","metersPerUnit":0.01});
    f.model["nodes"][0]["scale"] = json!([-2, 3, 4]);
    f.model["nodes"][0]["rotation"] = json!([0, 0, q, q]);
    f.model["nodes"][1]["translation"] = json!([5, 6, 7]);
    f.model["nodes"][1]["scale"] = json!([1, 2, 0.5]);
    f.model["nodes"][1]["rotation"] = json!([q, 0, 0, q]);
    f.save();
    let input = f.input();
    let (package, _) = package(&f, &input);
    let mut world = engine_runtime::world::World::new();
    let mut hydrator =
        engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let (instance, report) = hydrator
        .instance_loader_mut()
        .instantiate_prefab_from_package(
            &package,
            RuntimeAssetRef {
                id: "import.prefab.0".into(),
                asset_type: "prefab".into(),
                guid: Some("guid-import-prefab-0".into()),
                sub_asset: Some("stable-prefab-0".into()),
            },
            None,
            None,
            &mut world,
        );
    let instance = instance.unwrap_or_else(|| panic!("{report:?}"));
    let source_id = engine_runtime::ids::EntityId::from("authored-node-1");
    let id = &instance.source_to_world_entity[&source_id];
    let mut context = engine_runtime::render_extract::RenderExtractContext::new();
    let mut scene = engine_runtime::render_state::RenderSceneState::new();
    let batch = context
        .extract_world_dirty(1, &mut world, &scene)
        .normalize_merge(&scene);
    assert!(engine_runtime::render_command::apply_batch(&mut scene, &batch).is_empty());
    let matrix = world.spatial_world(id).unwrap().world_matrix;
    let v = floats(&mesh(&input).vertex_bytes);
    let actual: [f32; 3] = std::array::from_fn(|r| {
        matrix[r] * v[0] + matrix[4 + r] * v[1] + matrix[8 + r] * v[2] + matrix[12 + r]
    });
    // Independently: child Rx90 -> (4,6,5); parent Rz90 -> (-16,-5,24);
    // Z-up/left to Y-up/right in centimetres yields (-.16,.24,-.05).
    for (a, b) in actual.into_iter().zip([-0.16, 0.24, -0.05]) {
        assert!((a - b).abs() < 2e-6, "world vertex {actual:?}");
    }
    assert!(world.transform3d(id).is_some());
}

#[test]
fn import_material_opaque_alpha_has_distinct_pipeline_identity_and_rejects_blend_conflict() {
    let f = ModelFixture::new();
    let mut imported = material(&f.input());
    assert!(imported.opaque_alpha);
    let imported_identity = imported.pipeline_identity().unwrap();
    imported.opaque_alpha = false;
    assert_ne!(imported_identity, imported.pipeline_identity().unwrap());
    assert!(serde_json::to_value(&imported)
        .unwrap()
        .get("opaqueAlpha")
        .is_none());
    imported.opaque_alpha = true;
    imported.blend = MaterialBlend::AlphaBlend;
    assert!(imported.validate().is_err());
}

#[cfg(feature = "real-wgpu")]
#[path = "import_gpu.rs"]
mod gpu;

#[test]
fn import_mip_filters_generate_complete_srgb_chains_without_mutating_shared_base_pixels() {
    let mut f = ModelFixture::new();
    for (filter, key) in [
        (9984, "nearestMipmapNearest"),
        (9985, "linearMipmapNearest"),
        (9986, "nearestMipmapLinear"),
        (9987, "linearMipmapLinear"),
    ] {
        f.model["samplers"][0]["minFilter"] = json!(filter);
        f.save();
        let input = f.input();
        assert!(material(&input)
            .base_color_texture_sampling
            .unwrap()
            .key()
            .starts_with(key));
        let (_package, mut loader) = package(&f, &input);
        let mut manager = RenderResourceManager::new();
        let prepared = RuntimeRenderAssetProducer::new()
            .prepare_mesh_renderer(0, &renderer(&input), &mut loader, &mut manager)
            .unwrap();
        let texture = &prepared.textures[0].payload;
        assert_eq!(texture.mip_count, 2);
        assert_eq!(texture.rgba8.len(), 20);
        assert_eq!(&texture.rgba8[16..], [188, 188, 188, 255]);
        assert_eq!(
            &texture.rgba8[..16],
            [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]
        );
    }
}

#[test]
fn s2a_standard_lit_preserves_five_slots_and_shares_only_matching_color_spaces() {
    let mut f = ModelFixture::new();
    f.descriptor["materialStrategy"] = json!("standardLit");
    f.model["materials"][0]
        .as_object_mut()
        .unwrap()
        .remove("extensions");
    f.model["materials"][0]["normalTexture"] = json!({"index":0,"scale":0.7});
    f.model["materials"][0]["occlusionTexture"] = json!({"index":0,"strength":0.6});
    f.model["materials"][0]["emissiveTexture"] = json!({"index":0});
    f.model["materials"][0]["emissiveFactor"] = json!([0.1, 0.2, 0.3]);
    f.model["materials"][0]["pbrMetallicRoughness"]["metallicRoughnessTexture"] =
        json!({"index":0});
    f.model["samplers"][0]["minFilter"] = json!(9987);
    f.save();
    let input = f.input();
    let mat = material(&input);
    assert_eq!(mat.preset, MaterialPreset::StandardLit);
    assert_eq!(mat.textures.len(), 5);
    assert_eq!(mat.texture_sampling.len(), 5);
    assert!(!f.report()["notices"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["outcome"] == "discarded"));
    let (_, mut loader) = package(&f, &input);
    let mut manager = RenderResourceManager::new();
    let bound = RuntimeRenderAssetProducer::new()
        .prepare_mesh_renderer(0, &renderer(&input), &mut loader, &mut manager)
        .unwrap();
    assert_eq!(bound.textures.len(), 5);
    let mut handles = std::collections::BTreeSet::new();
    for t in &bound.textures {
        handles.insert(t.handle.raw());
        assert_eq!(t.payload.mip_count, 2);
        assert_eq!(t.payload.rgba8.len(), 20);
    }
    assert_eq!(
        handles.len(),
        2,
        "one image needs exactly sRGB and linear variants"
    );
    let draw = engine_runtime::render_geometry::GeometryFrame::build(
        "main",
        [64, 64],
        &[engine_runtime::render_geometry::GeometryViewInput {
            view: &RenderView::perspective(1, 64, 64, 60., 0.1, 20.),
            pose: CameraPose::default(),
        }],
        &[engine_runtime::render_geometry::GeometryInstance {
            instance_id: 1,
            source: &bound.geometry[0],
            world_transform: {
                let mut m = engine_runtime::render_contract::IDENTITY_MATRIX;
                m[14] = -3.;
                m
            },
            visibility: Visibility {
                authored: true,
                inherited: Evaluation::Known(true),
            },
            layers: LayerSelection::Named {
                names: vec!["main".into()],
            },
            provenance: Provenance::unspecified(ProducerStage::RenderProjection),
        }],
        &manager,
        DiagnosticLevel::Summary,
    )
    .unwrap();
    assert!(draw.views[0].draws[0].standard_lit.is_some());
    assert_eq!(draw.views[0].draws[0].texture_mask(), 31);
}

#[test]
fn s2d_bc3_import_rejects_dual_color_space_and_unsupported_target() {
    let mut f = ModelFixture::new();
    f.descriptor["textureCompression"] = json!("astc");
    f.save();
    f.error("texture_compression_unsupported", "/textureCompression");
    f.descriptor["textureCompression"] = json!("bc3");
    f.descriptor["materialStrategy"] = json!("standardLit");
    f.model["materials"][0]
        .as_object_mut()
        .unwrap()
        .remove("extensions");
    f.model["materials"][0]["normalTexture"] = json!({"index":0});
    f.save();
    let (compiler, lease) = f.project.compiler();
    let error = compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err();
    assert!(error.message().contains("both color and data"), "{error:?}");
}

#[test]
fn s2a_standard_lit_rejects_missing_normals_and_nonzero_uv() {
    let mut f = ModelFixture::new();
    f.descriptor["materialStrategy"] = json!("standardLit");
    f.model["materials"][0]
        .as_object_mut()
        .unwrap()
        .remove("extensions");
    f.model["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .unwrap()
        .remove("NORMAL");
    f.save();
    f.error("material.normal_required", "/primitives/0");
    f.model["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(1);
    f.model["materials"][0]["normalTexture"] = json!({"index":0,"texCoord":1});
    f.save();
    f.error("material.uv_unsupported", "normalTexture");
    f.model["materials"][0]["normalTexture"]["texCoord"] = json!(0);
    f.model["materials"][0]["pbrMetallicRoughness"]
        .as_object_mut()
        .unwrap()
        .remove("baseColorTexture");
    f.model["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .unwrap()
        .remove("TEXCOORD_0");
    f.save();
    f.error("material.uv_required", "/primitives/0");
    f.model["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"] = json!(2);
    let path = f.project.root.join("Models/quad.bin");
    let mut bytes = fs::read(&path).unwrap();
    bytes[96..128].fill(0);
    fs::write(path, bytes).unwrap();
    f.save();
    f.error("material.geometry_invalid", "/materials/0");
}

mod s3a {
    use super::*;
    use engine_runtime::{character_animation::session::CharacterModule,components::ComponentTypeId,query::QuerySpec,project_runtime_session::ProjectRuntimeMutationBuffer};
    fn append(f:&mut ModelFixture,values:&[f32],count:usize,kind:&str)->usize {
        let path=f.project.root.join("Models/quad.bin");let mut bytes=fs::read(&path).unwrap();while bytes.len()%4!=0 {bytes.push(0);}
        let offset=bytes.len();bytes.extend(values.iter().flat_map(|v|v.to_le_bytes()));fs::write(path,&bytes).unwrap();f.model["buffers"][0]["byteLength"]=json!(bytes.len());
        let view=f.model["bufferViews"].as_array().unwrap().len();f.model["bufferViews"].as_array_mut().unwrap().push(json!({"buffer":0,"byteOffset":offset,"byteLength":values.len()*4}));
        let accessor=f.model["accessors"].as_array().unwrap().len();f.model["accessors"].as_array_mut().unwrap().push(json!({"bufferView":view,"componentType":5126,"count":count,"type":kind}));accessor
    }
    #[test]
    fn real_project_compiler_cook_prefab_sdk_world_pose_skin_and_despawn() {
        let mut f=ModelFixture::new();f.model["nodes"][0]["children"]=json!([1,2]);f.model["nodes"].as_array_mut().unwrap().push(json!({"name":"Hand"}));f.model["nodes"][1]["skin"]=json!(0);f.model["skins"]=json!([{"joints":[2]}]);
        let path=f.project.root.join("Models/quad.bin");let mut bytes=fs::read(&path).unwrap();let offset=bytes.len();bytes.extend([0u8;16]);fs::write(path,&bytes).unwrap();
        f.model["bufferViews"].as_array_mut().unwrap().push(json!({"buffer":0,"byteOffset":offset,"byteLength":16}));f.model["accessors"].as_array_mut().unwrap().push(json!({"bufferView":4,"componentType":5121,"count":4,"type":"VEC4"}));
        f.model["meshes"][0]["primitives"][0]["attributes"]["JOINTS_0"]=json!(4);
        let weights=append(&mut f,&[1.,0.,0.,0.,1.,0.,0.,0.,1.,0.,0.,0.,1.,0.,0.,0.],4,"VEC4");f.model["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_0"]=json!(weights);
        let times=append(&mut f,&[0.,1.],2,"SCALAR");let values=append(&mut f,&[0.,0.,0.,0.,1.,0.],2,"VEC3");
        f.model["animations"]=json!([{"name":"wave","samplers":[{"input":times,"output":values}],"channels":[{"sampler":0,"target":{"node":2,"path":"translation"}}],"extras":{"aifeEvents":[{"time":0.25,"name":"gesture"}]}}]);f.save();
        f.descriptor["outputs"].as_array_mut().unwrap().push(json!({"key":"stable-character-0","assetId":"import.character.0","assetGuid":"guid-import-character-0","kind":"character","sourceLocator":"/scenes/0/character","sourceFingerprint":sha256_prefixed(&serde_json::to_vec(&f.model["scenes"][0]).unwrap()),"materialSlots":[]}));f.project.write("Assets/import.json",&f.descriptor);
        let input=f.input();let (package,_)=package(&f,&input);let mut world=engine_runtime::world::World::new();let mut hydrator=engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator::from_package(&package);assert!(!hydrator.hydrate_active_scene(&package,&mut world).has_errors());
        let (instance,report)=hydrator.instance_loader_mut().instantiate_prefab_from_package(&package,RuntimeAssetRef{id:"import.prefab.0".into(),asset_type:"prefab".into(),guid:Some("guid-import-prefab-0".into()),sub_asset:None},None,None,&mut world);assert!(instance.is_some(),"{report:?}");let instance=instance.unwrap();
        let ids=world.query_entities(&QuerySpec::all([ComponentTypeId::new("engine.character3d")]));assert_eq!(ids.len(),1);let id=ids[0].clone();
        let handle=project_game_sdk::EntityHandle::new(id.as_str(),world.entity_generation(&id).unwrap()).unwrap();let mut writer=project_game_sdk::DeferredMutationWriter::default();writer.character_play3d(&handle,"wave",true).unwrap();
        let mut mutations=ProjectRuntimeMutationBuffer::new();for mutation in writer.into_inner(){let project_game_sdk::DeferredMutation::Spatial{entity_id,generation,intent}=mutation else{unreachable!()};mutations.spatial_command(entity_id.into(),generation,intent);}mutations.prepare(&world).unwrap().commit(&mut world).unwrap();
        let mut module=CharacterModule::default();world.begin_spatial_tick(1);module.advance(&mut world,hydrator.instance_loader_mut().asset_loader_mut(),1,0.5).unwrap();world.finish_spatial_tick();let frame=world.character_pose(&id).unwrap();assert_eq!(frame.events.len(),1);assert_eq!(frame.events[0].name,"gesture");assert_eq!(frame.current.joints[2].translation,[0.,0.5,0.]);
        let skin=world.character_skin(&frame.joints[1]).unwrap();let (point,_)=skin.reference_vertex(0,[0.;3],[0.,0.,1.],false).unwrap();assert!((point[1]-0.5).abs()<1e-5);
        let report=hydrator.instance_loader_mut().despawn_prefab_instance(instance.instance_id,&mut world);assert!(!report.has_errors(),"{report:?}");module.advance(&mut world,hydrator.instance_loader_mut().asset_loader_mut(),2,0.5).unwrap();assert!(world.character_pose(&id).is_none());assert!(world.character_skin(&frame.joints[1]).is_none());
    }
}

#[cfg(not(target_arch="wasm32"))]
mod s3c {
    use super::*;
    use engine_runtime::{physics_3d::*,ids::EntityId};
    #[test]
    fn collision_asset_real_cook_load_shared_shape_and_input_invalidation(){
        let f=ModelFixture::new();let descriptor=json!({"schemaVersion":"collision3d-asset.v1","assetId":"collision.shared","assetGuid":"guid-collision-shared","shape":{"kind":"sphere","radius":0.5}});f.project.write("Assets/collision.json",&descriptor);
        let input=f.input();let a=input.assets.iter().find(|a|a.asset_id=="collision.shared").unwrap();let first_hash=a.hash.clone();let cooked:CookedCollision3D=serde_json::from_slice(a.runtime_payload.as_ref().unwrap()).unwrap();cooked.validate().unwrap();
        let (package,_)=package(&f,&input);let mut loader=engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
        let b=|id:&str,x:f32|json!({"schemaVersion":"runtime-entity.v2","id":id,"name":id,"enabled":true,"parentId":null,"siblingOrder":0,"spatial":{"domain":"world3d","transform":{"translation":[x,0,0],"rotationXyzw":[0,0,0,1],"scale":[1,1,1]}},"components":[{"componentType":"engine.body3d","data":{"schemaVersion":"body3d.v1","motion":"static","shape":{"kind":"asset","source":{"id":"collision.shared","type":"collision3d","guid":"guid-collision-shared"}}}}]});
        let scene=json!({"schemaVersion":"runtime-scene.v2","id":"main","name":"Main","gravity":0,"background":"#000000","skyColor":"#000000","mainCamera":null,"entities":[b("a",0.),b("b",3.)]});
        let RuntimeSceneDocument::Spatial(scene)=decode_runtime_scene(&serde_json::to_vec(&scene).unwrap()).unwrap()else{unreachable!()};let mut world=engine_runtime::scene_loader::load_scene_into_world(&scene.into()).value.unwrap();let mut physics=PhysicsModule::default();world.begin_spatial_tick(1);physics.advance_with_assets(&mut world,loader.asset_loader_mut(),1,1./60.).unwrap();world.finish_spatial_tick();assert_eq!(physics.body_count(),2);
        for x in [0.,3.] {let hit=physics.raycast(&PhysicsRay3D{origin:[x,2.,0.],displacement:[0.,-4.,0.]}).unwrap().unwrap();assert!((hit.distance-1.5).abs()<0.002);}
        world.try_despawn_entity(&EntityId::from("a")).unwrap();world.begin_spatial_tick(2);physics.advance_with_assets(&mut world,loader.asset_loader_mut(),2,1./60.).unwrap();world.finish_spatial_tick();assert_eq!(physics.body_count(),1);assert!(physics.raycast(&PhysicsRay3D{origin:[3.,2.,0.],displacement:[0.,-4.,0.]}).unwrap().is_some());
        let mut changed=descriptor;changed["shape"]["radius"]=json!(0.75);f.project.write("Assets/collision.json",&changed);let second=f.input();assert_ne!(first_hash,second.assets.iter().find(|a|a.asset_id=="collision.shared").unwrap().hash);
    }
}

#[cfg(not(target_arch="wasm32"))]
mod s3d {
 use super::*;
 #[test]
 fn real_recast_cook_profile_offline_tiles_and_incremental_invalidation(){
  let f=ModelFixture::new();let mut descriptor=json!({"schemaVersion":"navigation3d-asset.v1","assetId":"nav.cooked","assetGuid":"guid-nav-cooked","profile":{"key":"human","radius":0.3,"height":1.8,"climb":0.3,"maxSlopeDegrees":45,"cellSize":0.2,"cellHeight":0.1,"tileWidth":16},"vertices":[[-2,0,-2],[-2,0,18],[34,0,18],[34,0,-2]],"triangles":[[0,1,2],[0,2,3]],"tiles":[[0,0],[1,0]],"verticalBounds":[-1,4],"offMeshLinks":[]});f.project.write("Assets/nav.json",&descriptor);let input=f.input();let a=input.assets.iter().find(|a|a.asset_id=="nav.cooked").unwrap();let hash=a.hash.clone();let nav:engine_runtime::navigation_3d::CookedNavigation3D=serde_json::from_slice(a.runtime_payload.as_ref().unwrap()).unwrap();nav.validate().unwrap();assert_eq!(nav.tiles.len(),2);assert_eq!(nav.profile.radius,0.3);let second=f.input();assert_eq!(hash,second.assets.iter().find(|a|a.asset_id=="nav.cooked").unwrap().hash);descriptor["profile"]["radius"]=json!(0.6);f.project.write("Assets/nav.json",&descriptor);let changed=f.input();assert_ne!(hash,changed.assets.iter().find(|a|a.asset_id=="nav.cooked").unwrap().hash);
 }
}
