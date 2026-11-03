use super::*;
use engine_runtime::render_asset_production::*;
use engine_runtime::render_contract::*;
use engine_runtime::render_resource::*;
use engine_runtime::runtime_asset_loader::RuntimeAssetLoader;
use engine_runtime::runtime_package::*;
use engine_runtime::runtime_package_builder::*;

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.write(
        "Input/default.json",
        &serde_json::to_value(engine_runtime::input_mapping::InputMappingAsset::gameplay_default())
            .unwrap(),
    );
    let mut plane = strict_json(MESH).unwrap();
    plane["assetId"] = json!("plane");
    plane["assetGuid"] = json!("guid-plane");
    plane["source"]["geometry"] = json!({"primitive":"plane","size":[2,4]});
    fixture.write("Assets/plane.asset", &plane);
    let mut wide = strict_json(MESH).unwrap();
    wide["assetId"] = json!("wide");
    wide["assetGuid"] = json!("guid-wide");
    wide["source"]["geometry"]["size"] = json!([2, 1, 1]);
    fixture.write("Assets/wide.json", &wide);
    let mut textured = strict_json(MATERIAL).unwrap();
    textured["assetId"] = json!("textured");
    textured["assetGuid"] = json!("guid-textured");
    textured["baseColor"] = json!([1, 1, 1, 1]);
    textured["baseColorTexture"] = json!({"texture":{"id":"direction","type":"texture","guid":"guid-direction"},"minFilter":"nearest","magFilter":"linear","wrapS":"repeat","wrapT":"mirroredRepeat"});
    fixture.write("Assets/textured.json", &textured);
    fixture.write("Assets/direction.json", &json!({"schemaVersion":"texture-asset.v1","assetId":"direction","assetGuid":"guid-direction","sourceImage":"Assets/direction.png","importer":{"colorSpace":"linear","sampler":"nearestClamp"}}));
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
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
    fs::write(fixture.root.join("Assets/direction.png"), bytes).unwrap();
    fixture
}
pub(super) fn input(fixture: &Fixture) -> RuntimePackageBuildInput {
    let (compiler, lease) = fixture.compiler();
    let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    let mut input = prepared.runtime_package_build_input().clone();
    input.project = RuntimeProjectInfo::explicit_empty("generated", "Generated", "1");
    input
}
fn asset<'a>(input: &'a RuntimePackageBuildInput, id: &str) -> &'a RuntimePackageSourceAsset {
    input.assets.iter().find(|a| a.asset_id == id).unwrap()
}
fn cooked(input: &RuntimePackageBuildInput, id: &str) -> CookedRenderAsset {
    RenderContract::<CookedRenderAsset>::decode(
        std::str::from_utf8(asset(input, id).runtime_payload.as_ref().unwrap()).unwrap(),
    )
    .unwrap()
    .data
}
fn mesh(input: &RuntimePackageBuildInput, id: &str) -> CookedGeometry {
    match cooked(input, id).asset {
        RenderAssetData::Mesh(v) => v,
        _ => panic!("mesh"),
    }
}
pub(super) fn package(
    fixture: &Fixture,
    input: &RuntimePackageBuildInput,
    name: &str,
) -> (RuntimePackage, RuntimeAssetLoader) {
    let root = fixture.root.join(name);
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
fn reference(id: &str, kind: RenderAssetKind) -> RenderAssetReference {
    RenderAssetReference {
        expected_kind: kind,
        reference: RuntimeAssetRef {
            id: id.into(),
            asset_type: kind.as_str().into(),
            guid: Some(format!("guid-{id}")),
            sub_asset: None,
        },
    }
}
fn renderer() -> MeshRenderer3D {
    decode(scene()["entities"][0]["components"][2]["data"].clone(), "").unwrap()
}

#[test]
fn generated_cube_plane_have_independent_dimensions_normals_uv_winding_and_deterministic_cook() {
    let fixture = fixture();
    let first = input(&fixture);
    let second = input(&fixture);
    for id in ["cube", "plane", "wide", "blue", "textured"] {
        assert_eq!(
            asset(&first, id).runtime_payload,
            asset(&second, id).runtime_payload
        );
        assert_eq!(cooked(&first, id).recipe, GENERATED_RENDER_COOK_RECIPE);
        let expected: &[u8] = match id {
            "cube" => {
                include_bytes!("../../../engine_runtime/tests/fixtures/three_d_a12/cube.json")
            }
            "plane" => {
                include_bytes!("../../../engine_runtime/tests/fixtures/three_d_a12/plane.json")
            }
            "wide" => {
                include_bytes!("../../../engine_runtime/tests/fixtures/three_d_a12/wide.json")
            }
            "blue" => {
                include_bytes!("../../../engine_runtime/tests/fixtures/three_d_a12/blue.json")
            }
            "textured" => {
                include_bytes!("../../../engine_runtime/tests/fixtures/three_d_a12/textured.json")
            }
            _ => unreachable!(),
        };
        assert_eq!(
            asset(&first, id).runtime_payload.as_ref().unwrap(),
            expected,
            "GPU fixture must match actual Compiler Cook bytes: {id}"
        );
    }
    for (id, vertices, indices, half) in [
        ("cube", 24, 36, [0.5, 0.5, 0.5]),
        ("plane", 4, 6, [1., 0., 2.]),
    ] {
        let m = mesh(&first, id);
        assert_eq!(
            (m.vertex_count, m.index_count, m.vertex_stride),
            (vertices, indices, 32)
        );
        assert_eq!(m.bounds.min, half.map(|v| -v));
        assert_eq!(m.bounds.max, half);
        assert_eq!(m.bounds.sphere_center, [0.; 3]);
        assert!(
            (m.bounds.sphere_radius - half.iter().map(|v| v * v).sum::<f32>().sqrt()).abs() < 1e-6
        );
        assert_eq!(m.material_slots, ["surface"]);
        assert_eq!(m.submeshes[0].material_slot, "surface");
        assert_eq!(
            m.coordinate_convention,
            CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ
        );
        let rows: Vec<Vec<f32>> = m
            .vertex_bytes
            .chunks_exact(32)
            .map(|v| {
                v.chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect()
            })
            .collect();
        for triangle in m.index_bytes.chunks_exact(6) {
            let ids: Vec<usize> = triangle
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()) as usize)
                .collect();
            let a = &rows[ids[0]];
            let b = &rows[ids[1]];
            let c = &rows[ids[2]];
            let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
            let ac: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            assert!(
                (0..3).map(|i| cross[i] * a[3 + i]).sum::<f32>() > 0.,
                "{id}: triangle must face its normal"
            );
        }
        for row in &rows {
            assert_eq!(row[3..6].iter().map(|v| v * v).sum::<f32>(), 1.);
            assert!(row[6..8].iter().all(|v| *v == 0. || *v == 1.));
            if id == "cube" {
                assert_eq!((0..3).map(|i| row[i] * row[3 + i]).sum::<f32>(), 0.5);
            } else {
                assert_eq!(&row[3..6], &[0., 1., 0.]);
                assert_eq!(row[1], 0.);
                assert_eq!(row[6], (row[0] + 1.) / 2.);
                assert_eq!(row[7], (row[2] + 2.) / 4.);
            }
        }
    }
}

#[test]
fn generated_real_package_prepare_named_binding_and_base_color_sampling() {
    let fixture = fixture();
    let input = input(&fixture);
    let (package, mut loader) = package(&fixture, &input, "package");
    let producer = RuntimeRenderAssetProducer::new();
    let mut manager = RenderResourceManager::new();
    let plain = producer
        .prepare_cooked_from_package(
            0,
            &reference("blue", RenderAssetKind::Material),
            &mut loader,
            &mut manager,
        )
        .unwrap();
    assert!(plain.textures.is_empty());
    let RenderAssetData::Material(material) = plain.asset.asset else {
        panic!("material")
    };
    assert_eq!(
        material.parameters["baseColor"],
        MaterialParameter::Vector4([0., 0., 1., 1.])
    );
    assert_eq!(
        (material.cull, material.depth, material.blend),
        (
            MaterialCull::Back,
            MaterialDepth::TestAndWrite,
            MaterialBlend::Opaque
        )
    );
    let textured = producer
        .prepare_cooked_from_package(
            0,
            &reference("textured", RenderAssetKind::Material),
            &mut loader,
            &mut manager,
        )
        .unwrap();
    let texture = &textured.textures[0];
    assert_eq!(texture.payload.format, "rgba8UnormSrgb");
    assert_eq!(texture.payload.color_space, "srgb");
    assert_eq!(
        texture.payload.sampler,
        "nearest:linear:repeat:mirroredRepeat"
    );
    assert_eq!(
        textured.binding.texture("baseColorTexture"),
        Some(texture.handle)
    );
    assert_eq!(
        package
            .runtime_asset_index
            .record_by_guid("guid-textured")
            .unwrap()
            .dependencies,
        ["guid-direction"]
    );
    let bound = producer
        .prepare_mesh_renderer(0, &renderer(), &mut loader, &mut manager)
        .unwrap();
    assert_eq!(bound.geometry.len(), 1);
    assert_eq!(bound.geometry[0].material_slot(), "surface");
    assert!(bound.textures.is_empty());
    let mut wrong = renderer();
    let reference = wrong.materials.remove("surface").unwrap();
    wrong.materials.insert("wrong".into(), reference);
    assert_eq!(
        producer
            .prepare_mesh_renderer(0, &wrong, &mut loader, &mut manager)
            .unwrap_err()
            .field,
        "meshRenderer.materials"
    );
    assert_eq!(loader.decoded_cache_len(), 0);
}

#[test]
fn generated_color_camera_sampler_and_size_change_only_their_cooked_inputs() {
    let fixture = fixture();
    let first = input(&fixture);
    let mut material = strict_json(MATERIAL).unwrap();
    material["baseColor"] = json!([0.5, 0.25, 0.125, 0.4]);
    fixture.write("Assets/blue.json", &material);
    let second = input(&fixture);
    assert_ne!(asset(&first, "blue").hash, asset(&second, "blue").hash);
    assert_eq!(
        asset(&first, "cube").runtime_payload,
        asset(&second, "cube").runtime_payload
    );
    let mut source = scene();
    source["entities"][1]["components"][0]["data"]["translation"] = json!([1, 3, 6]);
    fixture.write("Scenes/main.json", &source);
    let third = input(&fixture);
    for id in ["cube", "plane", "blue", "textured"] {
        assert_eq!(asset(&second, id).hash, asset(&third, id).hash);
    }
    assert_ne!(
        serde_json::to_value(&second.scenes).unwrap(),
        serde_json::to_value(&third.scenes).unwrap()
    );
    let mut source = strict_json(MESH).unwrap();
    source["source"]["geometry"]["size"] = json!([2, 4, 6]);
    fixture.write("Assets/cube.json", &source);
    let fourth = input(&fixture);
    assert_ne!(asset(&third, "cube").hash, asset(&fourth, "cube").hash);
    assert_eq!(mesh(&fourth, "cube").bounds.max, [1., 2., 3.]);
    assert_eq!(asset(&third, "blue").hash, asset(&fourth, "blue").hash);
    let mut textured: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("Assets/textured.json")).unwrap())
            .unwrap();
    textured["baseColorTexture"]["minFilter"] = json!("linear");
    fixture.write("Assets/textured.json", &textured);
    let fifth = input(&fixture);
    assert_ne!(
        asset(&fourth, "textured").hash,
        asset(&fifth, "textured").hash
    );
    assert_eq!(
        asset(&fourth, "direction").hash,
        asset(&fifth, "direction").hash
    );
    assert_eq!(asset(&fourth, "cube").hash, asset(&fifth, "cube").hash);
}

#[test]
fn generated_invalid_source_missing_texture_and_deferred_binding_fail_honestly() {
    for size in [
        json!([0, 1, 1]),
        json!([-1, 1, 1]),
        json!([1, 1]),
        json!([1, 1, 1, 1]),
    ] {
        let fixture = Fixture::new();
        let mut source = strict_json(MESH).unwrap();
        source["source"]["geometry"]["size"] = size;
        fixture.write("Assets/cube.json", &source);
        let (compiler, lease) = fixture.compiler();
        assert!(compiler.prepare(&lease, TargetProfile::WindowsDev).is_err());
    }
    let fixture = fixture();
    let input = input(&fixture);
    let (_, mut loader) = package(&fixture, &input, "package");
    let mut manager = RenderResourceManager::new();
    manager.set_upload_budget(RenderUploadBudget::new(1));
    manager.request_resource(0, RenderResourceRequest::texture("occupy", "1", 1));
    let error = RuntimeRenderAssetProducer::new()
        .prepare_mesh_renderer(0, &renderer(), &mut loader, &mut manager)
        .unwrap_err();
    assert_eq!(error.code, ContractErrorCode::ResourceNotReady);
    let mut bad = strict_json(MATERIAL).unwrap();
    bad["baseColorTexture"] = json!({"texture":{"id":"missing","type":"texture","guid":"guid-missing"},"minFilter":"nearest","magFilter":"nearest","wrapS":"repeat","wrapT":"repeat"});
    fixture.write("Assets/blue.json", &bad);
    let (compiler, lease) = fixture.compiler();
    let error = compiler
        .prepare(&lease, TargetProfile::WindowsDev)
        .unwrap_err();
    assert!(
        error
            .source_location()
            .unwrap()
            .field_path
            .as_deref()
            .unwrap()
            .contains("baseColorTexture"),
        "{error:?}"
    );
}

#[test]
fn generated_flat_variant_is_authorable_and_cooked_without_changing_default_assets() {
    let fixture = fixture();
    let original = input(&fixture);
    let RenderAssetData::Material(old) = cooked(&original, "blue").asset else {
        panic!()
    };
    assert!(!old.ignore_vertex_color);
    let mut source = strict_json(MATERIAL).unwrap();
    source["ignoreVertexColor"] = json!(true);
    fixture.write("Assets/blue.json", &source);
    let updated = input(&fixture);
    let RenderAssetData::Material(flat) = cooked(&updated, "blue").asset else {
        panic!()
    };
    assert!(flat.ignore_vertex_color);
    assert_ne!(
        old.pipeline_identity().unwrap(),
        flat.pipeline_identity().unwrap()
    );
    assert_eq!(
        asset(&original, "cube").runtime_payload,
        asset(&updated, "cube").runtime_payload
    );
    assert_ne!(
        asset(&original, "blue").runtime_payload,
        asset(&updated, "blue").runtime_payload
    );
    source["ignoreVertexColor"] = json!("true");
    fixture.write("Assets/blue.json", &source);
    let (compiler, lease) = fixture.compiler();
    assert!(compiler.prepare(&lease, TargetProfile::WindowsDev).is_err());
}
