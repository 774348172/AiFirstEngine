use engine_runtime::spatial_contract::*;
use project_authoring_execution::{
    three_d_contract::*, CheckProfile, GameProjectCompiler, GameProjectCompilerError,
    ProjectAuthoringSession, TargetProfile,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
#[path = "fixtures/three_d_a0/future_api.rs"]
mod future_api;
#[path = "fixtures/three_d_a0/old_project_guard.rs"]
mod old_project;
const SCENE: &[u8] = include_bytes!("fixtures/three_d_a0/scene.json");
const MESH: &[u8] = include_bytes!("fixtures/three_d_a0/mesh.json");
const MATERIAL: &[u8] = include_bytes!("fixtures/three_d_a0/material.json");
const IMPORT: &[u8] = include_bytes!("fixtures/three_d_a0/import.json");
#[path = "three_d_project_a/generated.rs"]
mod generated;
#[path = "three_d_project_a/import_decode.rs"]
mod import_decode;
#[path = "three_d_project_a/import_scene.rs"]
mod import_scene;
#[path = "three_d_project_a/lit_recipe.rs"]
mod lit_recipe;
#[path = "three_d_project_a/presentation.rs"]
mod presentation;
#[path = "three_d_project_a/reimport.rs"]
mod reimport;
fn scene() -> Value {
    serde_json::from_slice(SCENE).unwrap()
}
fn canonical(value: Value) -> ContractResult<RuntimeSceneV2> {
    decode::<SpatialSourceScene>(value, "").and_then(|v| v.canonicalize())
}

#[test]
fn contract_source_roundtrip_to_typed_runtime_has_one_rotation_authority() {
    let source: SpatialSourceScene = decode(strict_json(SCENE).unwrap(), "").unwrap();
    let runtime = source.canonicalize().unwrap();
    assert_eq!(runtime.schema_version, "runtime-scene.v2");
    let bytes = serde_json::to_vec(&runtime).unwrap();
    assert!(matches!(
        decode_runtime_scene(&bytes).unwrap(),
        RuntimeSceneDocument::Spatial(_)
    ));
    let value = serde_json::to_value(runtime).unwrap();
    assert!(value["entities"][0].get("transform").is_none());
    assert_eq!(
        value["entities"][0]["spatial"]["transform"]["rotationXyzw"],
        json!([0.0, 0.0, 0.0, 1.0])
    );
    assert_eq!(
        Transform3D::identity(),
        decode::<Transform3D>(source.entities[0].components[0].data.clone(), "").unwrap()
    );
    assert_eq!(
        Camera3D::perspective_template(),
        decode::<Camera3D>(source.entities[1].components[1].data.clone(), "").unwrap()
    );
}
#[test]
fn contract_source_invalid_components_domains_and_transforms_have_paths() {
    for case in [
        "dual",
        "missing",
        "two_d",
        "duplicate",
        "unknown",
        "unknown_field",
        "bad_rotation",
        "zero_scale",
        "nonunit_camera",
        "orphan",
        "cycle",
    ] {
        let mut value = scene();
        match case {
            "dual" => value["entities"][0]["transform"] = legacy_transform(),
            "missing" => {
                value["entities"][0]["components"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }
            "two_d" => value["entities"][0]["components"]
                .as_array_mut()
                .unwrap()
                .push(json!({"componentType":"SpriteRenderer2D","data":{}})),
            "duplicate" => {
                let c = value["entities"][0]["components"][0].clone();
                value["entities"][0]["components"]
                    .as_array_mut()
                    .unwrap()
                    .push(c);
            }
            "unknown" => {
                value["entities"][0]["components"][0]["componentType"] = json!("Transform3d")
            }
            "unknown_field" => {
                value["entities"][0]["components"][0]["data"]["position"] = json!([1, 2, 3])
            }
            "bad_rotation" => {
                value["entities"][0]["components"][0]["data"]["rotation"] =
                    json!({"kind":"quaternion","xyzw":[0,0,0,2]})
            }
            "zero_scale" => {
                value["entities"][0]["components"][0]["data"]["scale"] = json!([0, 1, 1])
            }
            "nonunit_camera" => {
                value["entities"][1]["components"][0]["data"]["scale"] = json!([-1, 1, 1])
            }
            "orphan" => value["entities"][0]["parentId"] = json!("gone"),
            "cycle" => {
                value["entities"][0]["parentId"] = json!("camera");
                value["entities"][1]["parentId"] = json!("cube");
            }
            _ => unreachable!(),
        }
        let error = canonical(value.clone()).unwrap_err();
        assert!(
            error.field_path.starts_with("/entities/"),
            "{case}: {error:?}"
        );
        if case == "nonunit_camera" {
            assert!(
                value.pointer(&error.field_path).is_some(),
                "source location must name a real source field: {error:?}"
            );
        }
    }
}
#[test]
fn contract_source_mixed_independent_subtrees_and_project_data_are_allowed() {
    let mut value = scene();
    value["entities"].as_array_mut().unwrap().push(json!({"id":"legacy","name":"Legacy","enabled":true,"parentId":null,"siblingOrder":2,"transform":legacy_transform(),"components":[{"componentType":"project.score","data":{"value":2}}]}));
    canonical(value.clone()).unwrap();
    value["entities"][0]["parentId"] = json!("legacy");
    assert!(canonical(value)
        .unwrap_err()
        .code
        .ends_with("domain_conflict"));
}
#[test]
fn contract_real_compiler_check_and_prepare_allow_generated_presentation() {
    let fixture = Fixture::new();
    let (compiler, lease) = fixture.compiler();
    let report = compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code.ends_with("three_d.generated_static") && d.severity == "info"));
    assert!(report
        .checked_scope
        .iter()
        .any(|s| s == "spatial_contracts_and_version_boundary"));
    let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    let error = compiler
        .build(
            &prepared,
            project_authoring_execution::BuildRequest::new(TargetProfile::WindowsDev),
        )
        .unwrap_err();
    assert!(
        error.code().ends_with("execution_binding_missing"),
        "{error:?}"
    );
    assert!(!fixture.root.join("Build").exists());
}
#[test]
fn contract_compiler_rejects_downgraded_project_and_reports_the_source_field() {
    let fixture = Fixture::new();
    fixture.write("project.aife.json",&json!({"schemaVersion":"aife-project.v2","projectId":"three-d-contract","defaultScene":"Scenes/main.json"}));
    let error = fixture.check_error();
    assert!(
        error.code().ends_with("three_d.project_upgrade_required"),
        "{error:?}"
    );
    assert_eq!(
        error.source_location().unwrap().field_path.as_deref(),
        Some("/schemaVersion")
    );
}
#[test]
fn contract_baseline_old_project_guard_rejects_v3_and_new_context_refresh_accepts_it() {
    let fixture = Fixture::new();
    let bytes = fs::read(fixture.root.join("project.aife.json")).unwrap();
    let old = old_project::open_guard(&bytes).unwrap_err();
    assert_eq!(
        old.diagnostic.code,
        "authoring_context.manifest_schema_unsupported"
    );
    old_project::open_guard(br#"{"schemaVersion":"aife-project.v2","projectId":"old"}"#).unwrap();
    let mut session = ProjectAuthoringSession::open(&fixture.root).unwrap();
    session.refresh().unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("refresh-spatial", paths)
        .unwrap();
    assert_eq!(
        lease.snapshot().qualification,
        authoring_project_context::ProjectQualification::Ready
    );
}
#[test]
fn contract_legacy_project_prepare_stays_compatible() {
    let fixture = Fixture::empty("aife-project.v2");
    let (compiler, lease) = fixture.compiler();
    let report = compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
    assert!(!report
        .diagnostics
        .iter()
        .any(|d| d.code.contains("three_d.generated_static")));
    compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
}
#[test]
fn contract_compiler_rejects_disguised_scene_and_asset_envelopes() {
    for kind in ["scene", "mesh", "material"] {
        let fixture = Fixture::new();
        let (path, mut value, new_version) = match kind {
            "scene" => ("Scenes/main.json", scene(), "authoring-scene.v1"),
            "mesh" => (
                "Assets/cube.json",
                serde_json::from_slice(MESH).unwrap(),
                "mesh-asset.v1",
            ),
            _ => (
                "Assets/blue.json",
                serde_json::from_slice(MATERIAL).unwrap(),
                "material-asset.v1",
            ),
        };
        value["schemaVersion"] = json!(new_version);
        fixture.write(path, &value);
        assert!(
            fixture
                .check_error()
                .code()
                .ends_with("three_d.envelope_upgrade_required"),
            "{kind}"
        );
    }
}
#[test]
fn contract_compiler_asset_slots_and_reference_identities_must_resolve() {
    for case in [
        "slot_missing",
        "slot_extra",
        "slot_typo",
        "missing_mesh",
        "missing_material",
        "type_mismatch",
        "guid_conflict",
        "bad_sub_asset",
    ] {
        let fixture = Fixture::new();
        let mut value = scene();
        let renderer = &mut value["entities"][0]["components"][2]["data"];
        match case {
            "slot_missing" => renderer["materials"] = json!({}),
            "slot_extra" => {
                renderer["materials"]["extra"] = renderer["materials"]["surface"].clone()
            }
            "slot_typo" => {
                renderer["materials"] = json!({"Surface":{"id":"blue","type":"material"}})
            }
            "missing_mesh" => renderer["mesh"] = json!({"id":"missing","type":"mesh"}),
            "missing_material" => {
                renderer["materials"]["surface"] = json!({"id":"missing","type":"material"})
            }
            "type_mismatch" => renderer["mesh"]["type"] = json!("material"),
            "guid_conflict" => renderer["mesh"]["id"] = json!("different"),
            "bad_sub_asset" => renderer["mesh"]["sub_asset"] = json!("other"),
            _ => unreachable!(),
        }
        fixture.write("Scenes/main.json", &value);
        let error = fixture.check_error();
        assert_eq!(
            error.source_location().unwrap().source_path,
            "Scenes/main.json"
        );
        assert!(
            error
                .source_location()
                .unwrap()
                .field_path
                .as_ref()
                .unwrap()
                .starts_with("/entities/0/components/2/data"),
            "{case}: {error:?}"
        );
    }
}
#[test]
fn contract_high_level_assets_have_strict_inputs_and_explicit_template_values() {
    let mesh: MeshAssetV2 = decode(strict_json(MESH).unwrap(), "").unwrap();
    mesh.validate().unwrap();
    let material: MaterialAssetV2 = decode(strict_json(MATERIAL).unwrap(), "").unwrap();
    material.validate().unwrap();
    for size in [json!([0, 1, 1]), json!([-1, 1, 1]), json!([1, 1])] {
        let mut value = strict_json(MESH).unwrap();
        value["source"]["geometry"]["size"] = size;
        assert!(decode::<MeshAssetV2>(value, "")
            .and_then(|v| v.validate())
            .is_err());
    }
    let mut value = strict_json(MESH).unwrap();
    value["renderData"] = json!({});
    assert!(decode::<MeshAssetV2>(value, "").is_err());
    let mut plane = strict_json(MESH).unwrap();
    plane["source"]["geometry"] = json!({"primitive":"plane","size":[2,4]});
    decode::<MeshAssetV2>(plane, "")
        .unwrap()
        .validate()
        .unwrap();
    for (key, bad) in [
        ("preset", json!("pbr")),
        ("baseColor", json!([1, 0, 0])),
        ("depth", json!("disabled")),
        ("shaderSource", json!("invented")),
    ] {
        let mut value = strict_json(MATERIAL).unwrap();
        value[key] = bad;
        assert!(
            decode::<MaterialAssetV2>(value, "")
                .and_then(|v| v.validate())
                .is_err(),
            "{key}"
        );
    }
    let mut hdr = material.clone();
    hdr.base_color = [2.0, 1.0, 0.0, 1.0];
    hdr.validate().unwrap();
    hdr.base_color[0] = f32::NAN;
    assert!(hdr.validate().is_err());
}
#[test]
fn contract_declared_missing_texture_is_failure_while_no_slot_is_plain_color() {
    let fixture = Fixture::new();
    let mut value = strict_json(MATERIAL).unwrap();
    value["baseColorTexture"] = json!({"texture":{"id":"missing","type":"texture"},"minFilter":"linear","magFilter":"nearest","wrapS":"mirroredRepeat","wrapT":"clampToEdge"});
    fixture.write("Assets/blue.json", &value);
    let error = fixture.check_error();
    assert_eq!(
        error.source_location().unwrap().field_path.as_deref(),
        Some("/baseColorTexture/texture")
    );
    fixture.write(
        "Assets/texture.json",
        &json!({"schemaVersion":"texture-asset.v1","assetId":"missing","assetGuid":"texture-guid"}),
    );
    let (compiler, lease) = fixture.compiler();
    compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
}
#[test]
fn contract_import_settings_budgets_and_persistent_mapping_are_validated() {
    let import: ModelImportV1 = decode(strict_json(IMPORT).unwrap(), "").unwrap();
    import.validate().unwrap();
    assert_eq!(
        import.material_strategy,
        ImportMaterialStrategy::StrictUnlit
    );
    assert_eq!(import.budgets, ImportBudgets::default());
    let mut tight = import.clone();
    tight.budgets.source_bytes = 1;
    tight.validate().unwrap();
    for case in [
        "budget",
        "zero_budget",
        "outside",
        "url",
        "unit",
        "duplicate_guid",
        "duplicate_slot",
        "duplicate_node",
        "unknown",
    ] {
        let mut value = strict_json(IMPORT).unwrap();
        match case {
            "budget" => {
                value["budgets"] = serde_json::to_value(ImportBudgets::default()).unwrap();
                value["budgets"]["vertices"] = json!(2_000_001);
            }
            "zero_budget" => {
                value["budgets"] = serde_json::to_value(ImportBudgets::default()).unwrap();
                value["budgets"]["depth"] = json!(0);
            }
            "outside" => value["source"] = json!("../outside.gltf"),
            "url" => value["source"] = json!("https://example.org/model.glb"),
            "unit" => value["coordinates"]["metersPerUnit"] = json!(0),
            "duplicate_guid" => {
                value["outputs"][1]["assetGuid"] = value["outputs"][0]["assetGuid"].clone()
            }
            "duplicate_slot" => {
                let slot = value["outputs"][0]["materialSlots"][0].clone();
                value["outputs"][0]["materialSlots"]
                    .as_array_mut()
                    .unwrap()
                    .push(slot);
            }
            "duplicate_node" => {
                let node = value["nodes"][0].clone();
                value["nodes"].as_array_mut().unwrap().push(node);
            }
            "unknown" => value["ignoreUnsupported"] = json!(true),
            _ => unreachable!(),
        }
        assert!(
            decode::<ModelImportV1>(value, "")
                .and_then(|v| v.validate())
                .is_err(),
            "{case}"
        );
    }
}
#[test]
fn contract_import_descriptor_and_binary_sources_use_existing_snapshot_inventory() {
    let fixture = Fixture::new();
    fixture.write("Assets/import.json", &strict_json(IMPORT).unwrap());
    fixture.write(
        "Models/scene.gltf",
        &json!({"asset":{"version":"2.0"},"scenes":[{"nodes":[]}]}),
    );
    let model: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("Models/scene.gltf")).unwrap()).unwrap();
    fixture.write(
        "Assets/import.json",
        &import_scene::descriptor_for(&model, strict_json(IMPORT).unwrap()),
    );
    fs::write(fixture.root.join("Models/data.bin"), [1, 2, 3]).unwrap();
    let session = ProjectAuthoringSession::open(&fixture.root).unwrap();
    let inventory = session.source_inventory().unwrap();
    for path in ["Assets/import.json", "Models/scene.gltf", "Models/data.bin"] {
        assert!(inventory.entries.iter().any(|e| e.relative_path == path));
    }
    let (compiler, lease) = fixture.compiler();
    compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
    // New source metadata is read-only during check; source bytes remain byte-identical.
    let before = fs::read(fixture.root.join("Assets/import.json")).unwrap();
    compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    assert_eq!(
        before,
        fs::read(fixture.root.join("Assets/import.json")).unwrap()
    );
}
#[test]
fn contract_import_missing_source_and_primary_budget_are_located_before_import() {
    let fixture = Fixture::new();
    let mut import = strict_json(IMPORT).unwrap();
    fixture.write("Assets/import.json", &import);
    assert!(fixture
        .check_error()
        .code()
        .ends_with("three_d.model_source_missing"));
    fixture.write("Models/scene.gltf", &json!({"asset":{"version":"2.0"}}));
    import["budgets"] = serde_json::to_value(ImportBudgets::default()).unwrap();
    import["budgets"]["sourceBytes"] = json!(1);
    fixture.write("Assets/import.json", &import);
    assert!(fixture
        .check_error()
        .code()
        .ends_with("three_d.source_budget_exceeded"));
}
#[test]
fn contract_prefab_accepts_camera_without_selecting_scene_main_camera() {
    let fixture = Fixture::new();
    let prefab = json!({"schemaVersion":"authoring-prefab-asset.v2","id":"p","name":"P","rootEntityId":"camera","entities":[scene()["entities"][1].clone()]});
    fixture.write("Prefabs/camera.json", &prefab);
    let (compiler, lease) = fixture.compiler();
    compiler
        .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
        .unwrap();
    let runtime = decode::<SpatialSourcePrefab>(prefab, "")
        .unwrap()
        .canonicalize()
        .unwrap();
    assert_eq!(runtime.schema_version, RUNTIME_PREFAB_V2);
}
#[test]
fn contract_duplicate_json_fields_are_rejected_by_the_real_compiler() {
    let fixture = Fixture::new();
    let text = String::from_utf8(MESH.to_vec()).unwrap().replace(
        "\"assetId\":\"cube\"",
        "\"assetId\":\"other\",\"assetId\":\"cube\"",
    );
    fs::write(fixture.root.join("Assets/cube.json"), text).unwrap();
    assert!(fixture.check_error().message().contains("duplicate field"));
}

#[test]
fn contract_frozen_sdk_fixture_is_consumed_by_spatial_sdk_without_reusing_legacy_transform() {
    use future_api::*;
    let handle = project_game_sdk::EntityHandle::new("cube", 7).unwrap();
    let write = write_local_transform3d(&handle, &Transform3D::identity()).unwrap();
    let hide = set_mesh_visible(&handle, false);
    let material = set_material(
        &handle,
        "surface".into(),
        decode(
            json!({"id":"blue","type":"material","guid":"guid-blue"}),
            "",
        )
        .unwrap(),
    );
    let camera = select_main_camera(&handle);
    for command in [write, hide, material, camera] {
        let value = serde_json::to_value(command).unwrap();
        assert_eq!(value["generation"], json!(7));
        assert_eq!(value["entityId"], json!("cube"));
        let _: SpatialMutationFixture = serde_json::from_value(value.clone()).unwrap();
        assert!(
            serde_json::from_value::<project_runtime_sdk::ProjectRuntimeDeferredMutation>(value)
                .is_ok(),
            "A3 consumes the frozen spatial wire contract"
        );
    }
    assert!(project_runtime_sdk::PROJECT_RUNTIME_SDK_SCHEMA.contains("spatial-session-intents"));
}

#[test]
fn contract_future_observation_keeps_source_world_view_resource_and_frame_evidence_distinct() {
    use future_api::*;
    let fixture = SpatialObservationFixture {
        schema_version: "spatial-observation.v1".into(),
        revision: "fixture-revision".into(),
        delivery_digest: "fixture-delivery".into(),
        run_ref: "fixture-only-not-a-run".into(),
        operation_id: "fixture-operation".into(),
        tick: 4,
        frame_sequence: 3,
        main_camera: "camera".into(),
        viewport: [640, 480],
        view_version: 2,
        suspended: false,
        entity: SpatialReadFixture {
            source_entity_id: "source.cube".into(),
            entity_id: "cube".into(),
            generation: 7,
            local: Transform3D::identity().canonicalize().unwrap(),
            world_matrix: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0,
            ],
            authored_visible: true,
            inherited_visible: true,
        },
        frustum_included: true,
        resource_state: ResourceStateFixture::Deferred,
        frame_stage: FrameStageFixture::Submitted,
        truncated: false,
    };
    let value = serde_json::to_value(&fixture).unwrap();
    assert_eq!(value["resourceState"], json!("deferred"));
    assert_eq!(value["frameStage"], json!("submitted"));
    assert_eq!(
        value["entity"]["local"]["translation"],
        json!([0.0, 0.0, 0.0])
    );
    assert_eq!(value["entity"]["worldMatrix"][12], json!(3.0));
    let _: SpatialObservationFixture = serde_json::from_value(value.clone()).unwrap();
    let mut wrong = value;
    wrong["occlusionVisible"] = json!(true);
    assert!(serde_json::from_value::<SpatialObservationFixture>(wrong).is_err());
}

fn legacy_transform() -> Value {
    json!({"localPosition":{"x":0,"y":0,"z":0},"localRotation":{"x":0,"y":0,"z":0},"localScale":{"x":1,"y":1,"z":1}})
}

#[test]
fn hierarchy_compiler_prepare_preserves_typed_scene_prefab_and_legacy_lowering() {
    use engine_runtime::runtime_package::*;
    use engine_runtime::runtime_package_builder::*;
    use engine_runtime::runtime_scene_hydration::RuntimeSceneHydrator;
    use engine_runtime::world::World;
    let fixture = Fixture::empty("aife-project.v3");
    fixture.write(
        "Input/default.json",
        &serde_json::to_value(engine_runtime::input_mapping::InputMappingAsset::gameplay_default())
            .unwrap(),
    );
    fixture.write("project.aife.json",&json!({"schemaVersion":"aife-project.v3","projectId":"spatial-hydration","defaultScene":"Scenes/main.json"}));
    let mut source = scene();
    source["entities"][0]["components"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c["componentType"] != "MeshRenderer3D");
    source["entities"].as_array_mut().unwrap().push(json!({"id":"sprite","name":"Sprite","enabled":true,"parentId":null,"siblingOrder":2,"transform":legacy_transform(),"components":[{"componentType":"SpriteRenderer2D","data":{"visible":false,"color":"#336699FF"}}]}));
    fixture.write("Scenes/main.json", &source);
    fixture.write("Prefabs/camera.json",&json!({"schemaVersion":"authoring-prefab-asset.v2","id":"p","name":"Camera","rootEntityId":"camera","entities":[source["entities"][1].clone()]}));
    let (compiler, lease) = fixture.compiler();
    let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    assert!(compiler
        .build(
            &prepared,
            project_authoring_execution::BuildRequest::new(TargetProfile::WindowsDev)
        )
        .unwrap_err()
        .code()
        .ends_with("execution_binding_missing"));
    let mut input = prepared.runtime_package_build_input().clone();
    assert_eq!(input.scenes.len(), 1);
    assert_eq!(input.prefabs.len(), 1);
    assert_eq!(input.scenes[0].schema_version, RUNTIME_SCENE_V2);
    assert_eq!(input.scenes[0].main_camera.as_deref(), Some("camera"));
    assert!(input.scenes[0].entities[0].spatial.is_some());
    let legacy = input.scenes[0]
        .entities
        .iter()
        .find(|e| e.id == "sprite")
        .unwrap();
    assert!(legacy.spatial.is_none());
    assert_eq!(
        legacy.sprite_renderer2d.as_ref().unwrap().visible,
        Some(false)
    );
    assert!(legacy.components.is_empty());
    assert_eq!(
        input.prefabs[0].document["schemaVersion"],
        "runtime-prefab.v2"
    );
    // Hydration qualification uses an empty project module; no Host/DLL claim here.
    input.project = RuntimeProjectInfo::explicit_empty("spatial-hydration", "Spatial", "1");
    let package_dir = fixture.root.join("runtime-package");
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&package_dir, &input.scenes[0].id),
        &input,
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{report:?}"
    );
    let package = load_runtime_package(&package_dir);
    assert!(package.diagnostics.is_ok(), "{:?}", package.diagnostics);
    let package = package.value.unwrap();
    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let report = hydrator.hydrate_active_scene(&package, &mut world);
    assert!(!report.has_errors(), "{report:?}");
    assert_eq!(world.entity_count(), 3);
    assert!(world
        .transform3d(&engine_runtime::ids::EntityId::from("cube"))
        .is_some());
    assert!(world
        .sprite_renderer2d(&engine_runtime::ids::EntityId::from("sprite"))
        .is_some());
    let (instance, report) = hydrator
        .instance_loader_mut()
        .instantiate_prefab_from_package(
            &package,
            RuntimeAssetRef {
                id: "p".into(),
                asset_type: "prefab".into(),
                guid: None,
                sub_asset: None,
            },
            None,
            None,
            &mut world,
        );
    assert!(instance.is_some(), "{report:?}");
    assert_eq!(world.main_camera().unwrap().as_str(), "camera");
}
struct Fixture {
    root: PathBuf,
}

#[test]
fn contract_ordinary_3d_sample_with_business_aui_actions_passes_compiler_check() {
    fn copy(source: &std::path::Path, target: &std::path::Path) {
        fs::create_dir_all(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target.join(entry.file_name()));
            } else if entry.file_name() != "Cargo.lock" {
                fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
            }
        }
    }
    let fixture = Fixture::empty("aife-project.v3");
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../samples/three_d_lab_project");
    copy(&source, &fixture.root);
    let (compiler, lease) = fixture.compiler();
    compiler
        .check(
            &lease,
            CheckProfile::new(TargetProfile::WindowsDev).with_process_approval(true),
        )
        .unwrap();
}
impl Fixture {
    fn empty(schema: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aife-359-a0-source-{}-{stamp}-{}",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self { root };
        fixture.write(
            "project.aife.json",
            &json!({"schemaVersion":schema,"projectId":"three-d-contract"}),
        );
        fixture
    }
    fn new() -> Self {
        let fixture = Self::empty("aife-project.v3");
        fixture.write("project.aife.json",&json!({"schemaVersion":"aife-project.v3","projectId":"three-d-contract","defaultScene":"Scenes/main.json"}));
        fixture.write("Scenes/main.json", &scene());
        fixture.write("Assets/cube.json", &strict_json(MESH).unwrap());
        fixture.write("Assets/blue.json", &strict_json(MATERIAL).unwrap());
        fixture
    }
    fn write(&self, path: &str, value: &Value) {
        let target = self.root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, serde_json::to_vec(value).unwrap()).unwrap();
    }
    fn compiler(
        &self,
    ) -> (
        GameProjectCompiler,
        authoring_project_context::ProjectSnapshotLease,
    ) {
        let mut session = ProjectAuthoringSession::open(&self.root).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session.acquire_snapshot_lease("three-d-a0", paths).unwrap();
        (GameProjectCompiler::bind(&lease).unwrap(), lease)
    }
    fn check_error(&self) -> GameProjectCompilerError {
        let (compiler, lease) = self.compiler();
        compiler
            .check(&lease, CheckProfile::new(TargetProfile::WindowsDev))
            .unwrap_err()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn spatial_inspection_reports_owner_first_cause_stable_refs_and_bounded_entities() {
    let fixture = Fixture::new();
    let (_, lease) = fixture.compiler();
    let report = inspect_spatial_project_snapshot(lease.snapshot());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.schema_version, "spatial-project-inspection.v1");
    assert_eq!(report.scenes[0].main_camera.as_deref(), Some("camera"));
    assert_eq!(
        report.scenes[0].entities[0]
            .mesh_renderer
            .as_ref()
            .unwrap()
            .mesh
            .0
            .id,
        "cube"
    );
    let _ = lease.release();
    let mut value = scene();
    value["entities"][1]["components"][1]["data"]["projection"]["far"] = json!(0.01);
    fixture.write("Scenes/main.json", &value);
    let (_, lease) = fixture.compiler();
    let report = inspect_spatial_project_snapshot(lease.snapshot());
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code, "three_d.projection_invalid");
    assert!(report.diagnostics[0].field_path.contains("projection"));
    let _ = lease.release();
    let mut value = scene();
    for i in 0..70 {
        let mut entity = value["entities"][0].clone();
        entity["id"] = json!(format!("extra-{i}"));
        value["entities"].as_array_mut().unwrap().push(entity);
    }
    fixture.write("Scenes/main.json", &value);
    let (_, lease) = fixture.compiler();
    let report = inspect_spatial_project_snapshot(lease.snapshot());
    assert!(report.truncated);
    assert_eq!(report.scenes[0].entities.len(), 64);
}

#[path = "three_d_project_a/lighting.rs"]
mod lighting;

#[path = "three_d_project_a/shadows.rs"]
mod shadows;

#[path = "three_d_project_a/s4b_world_refs.rs"]
mod s4b_world_refs;
#[path = "three_d_project_a/s4c_terrain.rs"]
mod s4c_terrain;
