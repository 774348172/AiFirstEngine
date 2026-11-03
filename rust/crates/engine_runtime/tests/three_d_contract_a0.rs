use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::spatial_contract::*;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

#[path = "fixtures/three_d_a0/old_scene_reader.rs"]
mod old_reader;
const V1: &[u8] = include_bytes!("fixtures/three_d_a0/runtime-scene-v1.json");
const V2: &[u8] = include_bytes!("fixtures/three_d_a0/runtime-scene-v2.json");
fn scene() -> Value {
    serde_json::from_slice(V2).unwrap()
}
fn read(value: &Value) -> ContractResult<RuntimeSceneDocument> {
    decode_runtime_scene(&serde_json::to_vec(value).unwrap())
}
fn fails(value: Value, code: &str, field: &str) {
    let error = read(&value).unwrap_err();
    assert!(error.code.ends_with(code), "{error:?}");
    assert!(error.field_path.contains(field), "{error:?}");
}

#[test]
fn contract_runtime_v2_roundtrip_is_canonical_and_legacy_v1_still_decodes() {
    assert!(matches!(
        decode_runtime_scene(V1).unwrap(),
        RuntimeSceneDocument::Legacy(_)
    ));
    let RuntimeSceneDocument::Spatial(first) = decode_runtime_scene(V2).unwrap() else {
        panic!()
    };
    let bytes = serde_json::to_vec(&first).unwrap();
    let RuntimeSceneDocument::Spatial(second) = decode_runtime_scene(&bytes).unwrap() else {
        panic!()
    };
    assert_eq!(bytes, serde_json::to_vec(&second).unwrap());
    assert!(!String::from_utf8(bytes).unwrap().contains("eulerDegrees"));
}
#[test]
fn contract_quaternion_sign_tolerance_and_negative_zero_are_deterministic() {
    for q in [
        [0.0, 0.0, 0.0, 1.00005],
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.5, 0.5, 0.5, 0.5],
    ] {
        let a = canonical_quaternion(q).unwrap();
        let b = canonical_quaternion(q.map(|v| -v)).unwrap();
        assert_eq!(a.map(f32::to_bits), b.map(f32::to_bits));
        assert!((a.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-6);
    }
    assert!(canonical_quaternion([0.0, 0.0, 0.0, 1.00011]).is_err());
    assert!(canonical_quaternion([0.0; 4]).is_err());
    assert!(canonical_quaternion([f32::NAN, 0.0, 0.0, 1.0]).is_err());
}

#[test]
fn contract_canonical_quaternion_remains_stable_on_repeated_decode() {
    for i in 1..1000 {
        let mut transform = Transform3D::identity();
        transform.rotation = Rotation3D::EulerDegrees {
            xyz: [i as f32 * 0.13, i as f32 * -1.77, i as f32 * 2.29],
        };
        let first = transform.canonicalize().unwrap();
        let mut value = scene();
        value["entities"][0]["spatial"]["transform"] = serde_json::to_value(first).unwrap();
        let RuntimeSceneDocument::Spatial(first_read) = read(&value).unwrap() else {
            panic!()
        };
        let first_value = serde_json::to_value(first_read).unwrap();
        assert_eq!(
            value["entities"][0]["spatial"]["transform"],
            first_value["entities"][0]["spatial"]["transform"],
            "{i}"
        );
        let first_bytes = serde_json::to_vec(&first_value).unwrap();
        let RuntimeSceneDocument::Spatial(second_read) =
            decode_runtime_scene(&first_bytes).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            first_bytes,
            serde_json::to_vec(&serde_json::to_value(second_read).unwrap()).unwrap(),
            "{i}"
        );
    }
}

#[test]
fn contract_runtime_quaternion_requires_canonical_package_values() {
    for q in [
        [0.0, 0.0, 0.0, -1.0],
        [0.0, 0.0, 0.0, 1.00005],
        [-0.0, 0.0, 0.0, 1.0],
    ] {
        let mut value = scene();
        value["entities"][0]["spatial"]["transform"]["rotationXyzw"] = json!(q);
        fails(value, "quaternion_not_canonical", "rotationXyzw");
    }
}
#[test]
fn contract_euler_applies_x_then_y_then_z_with_independent_expected_rotation() {
    let mut transform = Transform3D::identity();
    transform.rotation = Rotation3D::EulerDegrees {
        xyz: [90.0, 90.0, 0.0],
    };
    let q = transform.canonicalize().unwrap().rotation_xyzw;
    for (actual, expected) in q.into_iter().zip([0.5, 0.5, -0.5, 0.5]) {
        assert!((actual - expected).abs() < 1e-6, "{q:?}");
    }
    transform.rotation = Rotation3D::EulerDegrees {
        xyz: [450.0, 450.0, 0.0],
    };
    assert_eq!(transform.canonicalize().unwrap().rotation_xyzw, q);
}
#[test]
fn contract_transform_rejects_nonfinite_singular_and_unrepresentable_values() {
    for scale in [
        [0.0, 1.0, 1.0],
        [f32::MIN_POSITIVE; 3],
        [f32::MAX; 3],
        [f32::INFINITY, 1.0, 1.0],
    ] {
        let mut transform = Transform3D::identity();
        transform.scale = scale;
        assert!(transform.canonicalize().is_err(), "{scale:?}");
    }
    let mut transform = Transform3D::identity();
    transform.scale = [-2.0, 3.0, 4.0];
    assert_eq!(transform.canonicalize().unwrap().scale, transform.scale);
    transform.translation[0] = f32::NAN;
    assert!(transform.canonicalize().is_err());
}
#[test]
fn contract_strict_fields_duplicates_and_unknown_spatial_domains_fail() {
    let mut value = scene();
    value["entities"][0]["transform"] = json!({});
    fails(value, "schema_invalid", "");
    let mut value = scene();
    value["entities"][0]["spatial"]["domain"] = json!("screen3d");
    fails(value, "schema_invalid", "");
    let mut value = scene();
    value["entities"][0]["spatial"]["transform"]
        .as_object_mut()
        .unwrap()
        .remove("rotationXyzw");
    fails(value, "schema_invalid", "");
    let text = String::from_utf8(V2.to_vec()).unwrap().replacen(
        "\"mainCamera\":\"camera\"",
        "\"mainCamera\":\"camera\",\"mainCamera\":\"other\"",
        1,
    );
    assert!(decode_runtime_scene(text.as_bytes())
        .unwrap_err()
        .message
        .contains("duplicate field"));
    assert!(serde_json::from_value::<Rotation3D>(
        json!({"kind":"eulerDegrees","xyz":[0,0,0],"xyzw":[0,0,0,1]})
    )
    .is_err());
}
#[test]
fn contract_main_camera_missing_wrong_disabled_and_scaled_fail() {
    for bad in [Value::Null, json!("missing"), json!("cube")] {
        let mut value = scene();
        value["mainCamera"] = bad;
        assert!(read(&value)
            .unwrap_err()
            .code
            .starts_with("three_d.main_camera"));
    }
    let mut value = scene();
    value["entities"][1]["enabled"] = json!(false);
    fails(value, "main_camera_disabled", "mainCamera");
    let mut value = scene();
    value["entities"][1]["spatial"]["camera"]["enabled"] = json!(false);
    fails(value, "main_camera_invalid", "mainCamera");
    let mut value = scene();
    value["entities"][1]["parentId"] = json!("cube");
    value["entities"][0]["spatial"]["transform"]["scale"] = json!([2, 1, 1]);
    fails(value, "camera_scale_unsupported", "scale");
    let mut value = scene();
    value["entities"][1]["parentId"] = json!("cube");
    value["entities"][0]["enabled"] = json!(false);
    fails(value, "main_camera_disabled", "mainCamera");
}
#[test]
fn contract_camera_projection_reuses_u2_and_rejects_legacy_matrix() {
    for projection in [
        json!({"kind":"perspective","fovYDegrees":180,"near":0.1,"far":10}),
        json!({"kind":"perspective","fovYDegrees":60,"near":0,"far":10}),
        json!({"kind":"orthographic","size":1,"near":2,"far":1}),
        json!({"kind":"legacyMatrix","view":vec![0;16],"projection":vec![0;16]}),
    ] {
        let mut camera = serde_json::to_value(Camera3D::perspective_template()).unwrap();
        camera["projection"] = projection;
        assert!(decode::<Camera3D>(camera, "")
            .and_then(|c| c.validate())
            .is_err());
    }
    let mut camera = Camera3D::perspective_template();
    camera.projection = engine_runtime::render_contract::Projection::Orthographic {
        size: 8.0,
        near: -10.0,
        far: 10.0,
    };
    camera.validate().unwrap();
}
#[test]
fn contract_hierarchy_domain_orphans_cycles_and_identity_are_rejected() {
    let mut value = scene();
    let old: Value = serde_json::from_slice(V1).unwrap();
    value["entities"]
        .as_array_mut()
        .unwrap()
        .push(old["entities"][0].clone());
    read(&value).unwrap();
    value["entities"][0]["parentId"] = json!("legacy");
    fails(value, "domain_conflict", "parentId");
    let mut value = scene();
    value["entities"][0]["parentId"] = json!("missing");
    fails(value, "parent_missing", "parentId");
    let mut value = scene();
    value["entities"][0]["parentId"] = json!("camera");
    value["entities"][1]["parentId"] = json!("cube");
    fails(value, "parent_cycle", "parentId");
    let mut value = scene();
    value["entities"][1]["id"] = json!("cube");
    fails(value, "entity_id_invalid", "id");
}
#[test]
fn contract_assetref_preserves_original_keys_and_named_slots_are_exact() {
    let value = json!({"id":"mesh","type":"mesh","guid":"persistent","sub_asset":"part"});
    let reference: SpatialAssetRef = decode(value.clone(), "").unwrap();
    assert_eq!(serde_json::to_value(reference).unwrap(), value);
    let mut renderer: MeshRenderer3D = decode(
        scene()["entities"][0]["spatial"]["meshRenderer"].clone(),
        "",
    )
    .unwrap();
    renderer.validate_slots(&["surface".into()]).unwrap();
    assert!(renderer.validate_slots(&["Surface".into()]).is_err());
    renderer
        .materials
        .insert("extra".into(), renderer.materials["surface"].clone());
    assert!(renderer.validate_slots(&["surface".into()]).is_err());
    let mut value = value;
    value["subAsset"] = json!("wrong-key");
    assert!(decode::<SpatialAssetRef>(value, "").is_err());
}
#[test]
fn contract_old_reader_rejects_real_new_envelope_and_exposes_old_silent_ignore() {
    assert!(old_reader::read_scene(V1).is_ok());
    // Empty v2 scene isolates the real old envelope check from missing old entity fields.
    let mut value = scene();
    value["entities"] = json!([]);
    value["mainCamera"] = Value::Null;
    let result = old_reader::read_scene(&serde_json::to_vec(&value).unwrap());
    assert!(result
        .issues
        .iter()
        .any(|d| d.path == "scene.schemaVersion" && d.message.contains("runtime-scene.v1")));
    assert!(old_reader::read_scene(V2).has_errors());
    let mut disguise: Value = serde_json::from_slice(V1).unwrap();
    disguise["mainCamera"] = json!("ignored-camera");
    assert!(
        old_reader::read_scene(&serde_json::to_vec(&disguise).unwrap()).is_ok(),
        "frozen reader must reproduce the baseline hole"
    );
    fails(disguise, "envelope_upgrade_required", "mainCamera");
    let old: Value = serde_json::from_slice(V1).unwrap();
    let unchecked: old_reader::RuntimePrefabData = serde_json::from_value(json!({"schemaVersion":"runtime-prefab.v2","id":"p","name":"P","rootEntityId":"legacy","entities":old["entities"]})).unwrap();
    assert_eq!(
        unchecked.schema_version, "runtime-prefab.v2",
        "baseline Prefab reader does not check schema; active scene v2 is the old-runtime guard"
    );
}
#[test]
fn contract_new_data_cannot_be_hidden_in_old_scene_or_entity() {
    for field in ["spatial", "camera3D", "meshRenderer3D", "transform3D"] {
        let mut value: Value = serde_json::from_slice(V1).unwrap();
        value["entities"][0][field] = json!({});
        fails(value, "envelope_upgrade_required", field);
    }
    let mut value: Value = serde_json::from_slice(V1).unwrap();
    value["entities"][0]["components"] =
        json!([{"componentType":"Transform3D","data":Transform3D::identity()}]);
    fails(value, "envelope_upgrade_required", "components/0");
}
#[test]
fn contract_prefab_root_and_versions_are_strict_without_stealing_camera() {
    let mut value = json!({"schemaVersion":"runtime-prefab.v2","id":"cameraPrefab","name":"Camera","rootEntityId":"camera","entities":[scene()["entities"][1].clone()]});
    assert!(matches!(
        decode_runtime_prefab(value.clone()).unwrap(),
        RuntimePrefabDocument::Spatial(_)
    ));
    value["mainCamera"] = json!("camera");
    assert!(decode_runtime_prefab(value.clone()).is_err());
    value.as_object_mut().unwrap().remove("mainCamera");
    value["rootEntityId"] = json!("missing");
    assert!(decode_runtime_prefab(value)
        .unwrap_err()
        .code
        .ends_with("prefab_root_invalid"));
    let old: Value = serde_json::from_slice(V1).unwrap();
    let mut legacy = json!({"schemaVersion":"runtime-prefab.v1","id":"p","name":"P","rootEntityId":"legacy","entities":old["entities"]});
    assert!(matches!(
        decode_runtime_prefab(legacy.clone()).unwrap(),
        RuntimePrefabDocument::Legacy(_)
    ));
    legacy["entities"][0]["spatial"] = json!({});
    assert!(decode_runtime_prefab(legacy).is_err());
}
#[test]
fn contract_real_loader_accepts_v1_and_v2_but_rejects_disguised_spatial_data() {
    let fixture = PackageFixture::new();
    fixture.write("scenes/main.json", &serde_json::from_slice(V1).unwrap());
    let legacy = load_runtime_package(&fixture.root);
    assert!(legacy.value.is_some(), "{:?}", legacy.diagnostics);
    let mut spatial_scene = scene();
    spatial_scene["entities"].as_array_mut().unwrap().remove(0);
    fixture.write("scenes/main.json", &spatial_scene);
    let spatial = load_runtime_package(&fixture.root);
    assert!(spatial.value.is_some(), "{:?}", spatial.diagnostics);
    assert!(spatial.value.unwrap().active_scene.entities[0]
        .spatial
        .is_some());
    let mut disguise: Value = serde_json::from_slice(V1).unwrap();
    disguise["mainCamera"] = json!("camera");
    fixture.write("scenes/main.json", &disguise);
    assert!(load_runtime_package(&fixture.root)
        .diagnostics
        .issues
        .iter()
        .any(|d| d.code == "three_d.envelope_upgrade_required"));
}
#[test]
fn contract_real_loader_rejects_v2_prefab_inside_an_old_active_scene() {
    let fixture = PackageFixture::new();
    fixture.write("scenes/main.json", &serde_json::from_slice(V1).unwrap());
    let prefab = json!({"schemaVersion":"runtime-prefab.v2","id":"p","name":"P","rootEntityId":"camera","entities":[scene()["entities"][1].clone()]});
    fixture.write("assets.json", &json!({"schemaVersion":"runtime-asset-manifest.v1","assets":[{"id":"p","name":"P","type":"prefab","source":"prefab","state":"available","bundleId":"startup","data":prefab}]}));
    let result = load_runtime_package(&fixture.root);
    assert!(result.value.is_none());
    assert!(
        result
            .diagnostics
            .issues
            .iter()
            .any(|d| d.code == "three_d.execution_unavailable"),
        "{:?}",
        result.diagnostics
    );
}

struct PackageFixture {
    root: PathBuf,
}
impl PackageFixture {
    fn new() -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aife-359-a0-package-{}-{stamp}-{}",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("scenes")).unwrap();
        let result = Self { root };
        result.write("manifest.json",&json!({"schemaVersion":"runtime-package.v2","packageMode":"debug-readable","project":{"projectId":"contract","name":"Contract","version":"1","runtimeModule":{"moduleId":"engine.empty.runtime","interfaceVersion":"project-runtime-module.v2","aotContentDigest":"sha256:engine-empty-runtime-v2"}},"activeSceneId":"main","scenes":[{"id":"main","name":"Main","path":"scenes/main.json","entityCount":1}],"assets":{"path":"assets.json","assetCount":0},"rules":{"path":"rules.json","mode":"none"},"input":{"path":"input.json","defaultMappingId":"input.default","mappingCount":1}}));
        result.write(
            "assets.json",
            &json!({"schemaVersion":"runtime-asset-manifest.v1","assets":[]}),
        );
        result.write("rules.json",&json!({"schemaVersion":"runtime-rule-manifest.v1","mode":"none","rules":[],"modules":[]}));
        result.write("input.json",&json!({"schemaVersion":"runtime-input-manifest.v1","defaultMappingId":"input.default","mappings":[{"id":"input.default","path":"mapping.json","enabled":true}]}));
        result.write(
            "mapping.json",
            &serde_json::to_value(
                engine_runtime::input_mapping::InputMappingAsset::gameplay_default(),
            )
            .unwrap(),
        );
        result
    }
    fn write(&self, path: &str, value: &Value) {
        fs::write(self.root.join(path), serde_json::to_vec(value).unwrap()).unwrap();
    }
}
impl Drop for PackageFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
