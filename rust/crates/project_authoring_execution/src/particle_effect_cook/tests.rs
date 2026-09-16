use super::*;
use engine_runtime::runtime_package::{
    load_runtime_package, RuntimeProjectInfo, RuntimeScene, RUNTIME_SCENE_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildInput, RuntimePackageBuildRequest, RuntimePackageBuildStatus,
    RuntimePackageBuilder,
};

const PATH: &str = "Assets/test.particle-effect.json";
const AUTHOR: &str = "Assets/test.particle.wgsl";
const FIXTURE: &str = include_str!("../../../engine_runtime/tests/fixtures/particle-effect.json");
const BEHAVIOR: &str = r#"fn particle_init(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle { return p; }
fn particle_update(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle {
    var result = p;
    result.velocity.y += sin(ctx.time) * params.speed * ctx.dt;
    return result;
}
"#;

fn source(custom: bool) -> CompilerSourceView {
    let mut files = BTreeMap::from([(PATH.into(), FIXTURE.as_bytes().to_vec())]);
    if custom {
        let mut desc: Value = serde_json::from_str(FIXTURE).unwrap();
        desc["emitters"][0]["behaviorSource"] = AUTHOR.into();
        files.insert(PATH.into(), serde_json::to_vec(&desc).unwrap());
        files.insert(AUTHOR.into(), BEHAVIOR.as_bytes().to_vec());
    }
    CompilerSourceView { files }
}
fn edit(source: &mut CompilerSourceView, change: impl FnOnce(&mut Value)) {
    let mut desc = serde_json::from_slice(source.bytes(PATH).unwrap()).unwrap();
    change(&mut desc);
    source
        .files
        .insert(PATH.into(), serde_json::to_vec(&desc).unwrap());
}
fn cooked(source: &CompilerSourceView) -> CookedParticleEffect {
    serde_json::from_slice(cook(source).unwrap()[0].runtime_payload.as_ref().unwrap()).unwrap()
}

#[test]
fn particle_effect_cook_preserves_data_and_generates_real_wgsl_parameter_contract() {
    let snapshot = source(true);
    let before = snapshot.clone();
    let effect = cooked(&snapshot);
    assert_eq!(effect.description.seed, 42);
    assert_eq!(effect.programs[0].author_path.as_deref(), Some(AUTHOR));
    let module = naga::front::wgsl::parse_str(&effect.programs[0].wgsl).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    assert!(module
        .functions
        .iter()
        .any(|(_, f)| f.name.as_deref() == Some("particle_update")));
    assert!(effect.programs[0].wgsl.contains("speed: f32"));
    assert_eq!(
        effect.source_digests[AUTHOR],
        sha256_prefixed(BEHAVIOR.as_bytes())
    );
    assert_eq!(
        snapshot, before,
        "cook must not rewrite immutable source or open live files"
    );
}

#[test]
fn particle_effect_cook_defaults_are_valid_without_author_code() {
    let effect = cooked(&source(false));
    assert_eq!(effect.programs[0].author_path, None);
    assert!(effect.programs[0].wgsl.contains("engine_particle_update"));
}

#[test]
fn particle_effect_cook_accepts_negative_plane_distance() {
    let mut snapshot = source(false);
    edit(&mut snapshot, |v| {
        v["emitters"][0]["collisions"] = serde_json::json!([{
            "shape": {"kind":"plane", "normal":[0,1,0], "distance":-2.6},
            "restitution":0.2, "kill":true
        }]);
    });
    // Cook invokes the real WGSL parser/validator on the complete compute program.
    let effect = cooked(&snapshot);
    assert_eq!(effect.description.emitters[0].collisions.len(), 1);
}

#[cfg(feature = "real-wgpu")]
#[test]
fn particle_effect_cook_gpu_consumer_executes_packaged_author_program() {
    use engine_runtime::{particle_gpu::ParticleGpuStep, wgpu_backend::real::RealWgpuBackend};
    let mut snapshot = source(true);
    edit(&mut snapshot, |v| {
        v["emitters"][0]["bursts"][0]["count"] = 1.into();
        v["emitters"][0]["velocityMin"] = serde_json::json!([0, 0, 0]);
        v["emitters"][0]["velocityMax"] = serde_json::json!([0, 0, 0]);
        v["emitters"][0]["updates"] = serde_json::json!([{"kind":"integrate"}]);
    });
    snapshot.files.insert(
        AUTHOR.into(),
        BEHAVIOR
            .replace(
                "result.velocity.y += sin(ctx.time) * params.speed * ctx.dt;",
                "result.position.y += params.speed * ctx.dt;",
            )
            .into_bytes(),
    );
    // Consume the serialized ordinary asset payload, not a test-generated shader.
    let program = cooked(&snapshot);
    drop(snapshot);
    let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
    backend.install_particle_effect(1, &program).unwrap();
    for step_id in 1..=2 {
        backend
            .simulate_particle_effect(
                1,
                ParticleGpuStep {
                    step_id,
                    delta_seconds: 1.0 / 64.0,
                    origin: [0.0; 3],
                    emitting: true,
                    paused: false,
                },
            )
            .unwrap();
    }
    let state = backend.capture_particle_state_for_validation(1).unwrap();
    assert_eq!(state[0].live, 1);
    assert!((state[0].particles[0].position[1] - 0.03125).abs() < 1e-4);
    assert!((state[0].particles[0].age - 0.015625).abs() < 1e-5);
}

#[test]
fn particle_effect_cook_maps_generated_field_errors_to_description() {
    let mut snapshot = source(false);
    edit(&mut snapshot, |v| v["parameters"][0]["name"] = "if".into());
    let failure = cook(&snapshot).unwrap_err();
    let location = failure.source_location().unwrap();
    assert_eq!(location.source_path, PATH);
    assert_eq!(location.field_path.as_deref(), Some("parameters.if.name"));
    assert!(!location.generated);
}

#[test]
fn particle_effect_cook_generates_parameter_input_and_state_types_without_duplicate_author_types() {
    let mut snapshot = source(true);
    edit(&mut snapshot, |v| {
        v["parameters"][0]["default"] = serde_json::json!({"type":"vec3","value":[1,2,3]});
        v["emitters"][0]["customState"] =
            serde_json::json!([{"name":"phase","default":{"type":"uint","value":0}}]);
        v["emitters"][0]["inputs"] =
            serde_json::json!([{"name":"wind","value":{"type":"vec3","value":[1,0,0]}}]);
    });
    snapshot.files.insert(
        AUTHOR.into(),
        BEHAVIOR
            .replace(
                "sin(ctx.time) * params.speed * ctx.dt",
                "params.speed.y * ctx.dt + inputs.wind.y",
            )
            .replace(
                "return result;",
                "result.custom.phase += 1u; return result;",
            )
            .into_bytes(),
    );
    let effect = cooked(&snapshot);
    assert!(effect.programs[0].wgsl.contains("speed: vec3<f32>"));
    assert!(effect.programs[0].wgsl.contains("phase: u32"));
}

#[test]
fn particle_effect_cook_reports_author_syntax_line_not_generated_line() {
    let mut snapshot = source(true);
    snapshot.files.insert(
        AUTHOR.into(),
        BEHAVIOR
            .replace("var result = p;", "var result = unknown_particle;")
            .into_bytes(),
    );
    let fail = cook(&snapshot).unwrap_err();
    let loc = fail.source_location().unwrap();
    assert_eq!(loc.source_path, AUTHOR);
    assert_eq!(loc.line, Some(3));
    assert!(!loc.generated);
    assert!(fail.message().contains("unknown_particle"));
    assert_eq!(fail.diagnostics()[0].location.as_ref(), Some(loc));
}

#[test]
fn particle_effect_cook_reports_author_type_failure_and_wrong_entry_signature() {
    let mut snapshot = source(true);
    snapshot.files.insert(
        AUTHOR.into(),
        BEHAVIOR
            .replace("var result = p;", "var result: f32 = p;")
            .into_bytes(),
    );
    let fail = cook(&snapshot).unwrap_err();
    assert_eq!(fail.source_location().unwrap().source_path, AUTHOR);
    assert_eq!(fail.source_location().unwrap().line, Some(3));
    snapshot.files.insert(AUTHOR.into(),BEHAVIOR.replace("fn particle_update(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs)","fn particle_update(p: Particle)").replace("    result.velocity.y += sin(ctx.time) * params.speed * ctx.dt;","").into_bytes());
    let fail = cook(&snapshot).unwrap_err();
    assert!(fail.message().contains("particle_update"));
    assert_eq!(fail.source_location().unwrap().source_path, PATH);
    snapshot.files.insert(
        AUTHOR.into(),
        BEHAVIOR
            .replace("fn particle_init(", "fn alternate_init(")
            .into_bytes(),
    );
    assert!(cook(&snapshot)
        .unwrap_err()
        .message()
        .contains("particle_init"));
}

#[test]
fn particle_effect_cook_rejects_project_entry_binding_and_mutable_global() {
    for forbidden in [
        "@compute @workgroup_size(1) fn custom_entry() {}\n",
        "@group(0) @binding(0) var<storage, read_write> external: array<f32>;\n",
        "var<private> hidden: f32;\n",
        "fn engine_takeover() {}\n",
    ] {
        let mut snapshot = source(true);
        snapshot
            .files
            .insert(AUTHOR.into(), format!("{forbidden}{BEHAVIOR}").into_bytes());
        assert!(cook(&snapshot).is_err(), "accepted {forbidden}");
    }
    let mut snapshot = source(true);
    snapshot.files.insert(AUTHOR.into(),format!("// @compute and @binding are harmless comments\nconst TAU: f32 = 6.283185;\nfn helper(x: f32) -> f32 {{ return cos(x * TAU); }}\n{BEHAVIOR}").into_bytes());
    cook(&snapshot).unwrap();
}

#[test]
fn particle_effect_cook_rejects_path_escape_missing_source_and_invalid_utf8() {
    for path in [
        "../escape.particle.wgsl",
        "C:/outside.particle.wgsl",
        "Assets/missing.particle.wgsl",
        "Assets/wrong.txt",
    ] {
        let mut snapshot = source(true);
        edit(&mut snapshot, |v| {
            v["emitters"][0]["behaviorSource"] = path.into()
        });
        assert!(cook(&snapshot).is_err(), "accepted {path}");
    }
    let mut snapshot = source(true);
    snapshot.files.insert(AUTHOR.into(), vec![0xff]);
    assert!(cook(&snapshot).is_err());
}

fn texture(snapshot: &mut CompilerSourceView) {
    edit(
        snapshot,
        |v| {
            v["emitters"][0]["draw"]["texture"] =
                serde_json::json!({"id":"smoke","type":"texture","guid":"guid-smoke"})
        },
    );
    snapshot.files.insert("Assets/smoke.asset".into(),br#"{"schemaVersion":"texture-asset.v1","assetId":"smoke","assetGuid":"guid-smoke","sourceImage":"Assets/smoke.png"}"#.to_vec());
    snapshot
        .files
        .insert("Assets/smoke.png".into(), vec![1, 2, 3]);
}

#[test]
fn particle_effect_cook_resolves_guid_dependencies_and_rejects_bad_references() {
    let mut snapshot = source(false);
    texture(&mut snapshot);
    let asset = cook(&snapshot).unwrap().remove(0);
    assert_eq!(asset.dependencies, vec!["guid-smoke"]);
    for (field, value) in [
        ("id", "absent"),
        ("guid", "wrong"),
        ("type", "audio"),
        ("typo", "value"),
    ] {
        let mut bad = snapshot.clone();
        edit(&mut bad, |v| {
            v["emitters"][0]["draw"]["texture"][field] = value.into()
        });
        assert!(cook(&bad).is_err(), "accepted bad {field}");
    }
    snapshot.files.remove("Assets/smoke.png");
    assert!(cook(&snapshot).unwrap_err().message().contains("missing"));
}

#[test]
fn particle_effect_cook_detects_duplicate_ids_and_guids() {
    for key in ["assetId", "assetGuid"] {
        let mut snapshot = source(false);
        let mut second: Value = serde_json::from_str(FIXTURE).unwrap();
        second["assetId"] = "second".into();
        second["assetGuid"] = "guid-second".into();
        second[key] = if key == "assetId" {
            "test-effect".into()
        } else {
            "guid-test-effect".into()
        };
        snapshot.files.insert(
            "Assets/second.particle-effect.json".into(),
            serde_json::to_vec(&second).unwrap(),
        );
        assert!(cook(&snapshot).unwrap_err().message().contains("duplicate"));
    }
}

#[test]
fn particle_effect_cook_tracks_local_invalidation_for_code_defaults_and_resource_bytes() {
    let mut initial = source(true);
    texture(&mut initial);
    let mut second: Value = serde_json::from_str(FIXTURE).unwrap();
    second["assetId"] = "unrelated".into();
    second["assetGuid"] = "guid-unrelated".into();
    initial.files.insert(
        "Assets/unrelated.particle-effect.json".into(),
        serde_json::to_vec(&second).unwrap(),
    );
    let before = cook(&initial).unwrap();
    for mutation in 0..3 {
        let mut changed = initial.clone();
        match mutation {
            0 => {
                changed.files.insert(
                    AUTHOR.into(),
                    BEHAVIOR
                        .replace("sin(ctx.time)", "cos(ctx.time)")
                        .into_bytes(),
                );
            }
            1 => edit(&mut changed, |v| {
                v["parameters"][0]["default"]["value"] = 3.into()
            }),
            _ => {
                changed
                    .files
                    .insert("Assets/smoke.png".into(), vec![4, 5, 6]);
            }
        }
        let after = cook(&changed).unwrap();
        assert_ne!(
            before[0].hash, after[0].hash,
            "mutation {mutation} did not invalidate effect"
        );
        assert_eq!(
            before[1].runtime_payload, after[1].runtime_payload,
            "unrelated effect changed"
        );
    }
}

#[test]
fn particle_effect_cook_rejects_schema_errors_with_source_location() {
    let mut snapshot = source(false);
    edit(&mut snapshot, |v| v["emitters"][0]["capacity"] = 0.into());
    let failure = cook(&snapshot).unwrap_err();
    assert_eq!(failure.source_location().unwrap().source_path, PATH);
    assert!(failure
        .source_location()
        .unwrap()
        .field_path
        .as_ref()
        .unwrap()
        .contains("capacity"));
}

#[test]
fn particle_effect_cook_is_consumed_by_the_existing_assembler() {
    let mut snapshot = source(true);
    snapshot.files.insert(
        "project.aife.json".into(),
        br#"{"projectId":"particle-test","projectName":"Particle Test"}"#.to_vec(),
    );
    let (input, digest, _) = crate::neutral_assembler::assemble(&snapshot, None).unwrap();
    assert_eq!(
        input
            .assets
            .iter()
            .filter(|a| a.asset_type == "particle-effect")
            .count(),
        1
    );
    let before = digest;
    snapshot.files.insert(
        AUTHOR.into(),
        BEHAVIOR
            .replace("sin(ctx.time)", "cos(ctx.time)")
            .into_bytes(),
    );
    let (_, after, _) = crate::neutral_assembler::assemble(&snapshot, None).unwrap();
    assert_ne!(before, after);
}

#[test]
fn particle_effect_cook_package_owns_exact_program_bytes_without_author_tree() {
    let asset = cook(&source(true)).unwrap().remove(0);
    let bytes = asset.runtime_payload.clone().unwrap();
    let uri = asset.runtime_uri.clone();
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "particle-test",
        "Particle",
        "1",
    ));
    let mapping =
        engine_runtime::input_mapping::InputMappingAsset::new("input.none", vec![], vec![], vec![]);
    input.input_mappings.push(
        engine_runtime::runtime_package_builder::RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        },
    );
    input.scenes.push(RuntimeScene {
        schema_version: RUNTIME_SCENE_SCHEMA_VERSION.into(),
        id: "main".into(),
        name: "Main".into(),
        gravity: 0.0,
        background: "color".into(),
        sky_color: "#000000".into(),
        entities: vec![],
    });
    input.assets.push(asset);
    let root = std::env::temp_dir().join(format!(
        "aife-particle-package-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(root.join("package"), "main"),
        &input,
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{report:?}"
    );
    assert!(load_runtime_package(root.join("package"))
        .diagnostics
        .is_ok());
    let stored = std::fs::read(root.join("package").join(uri)).unwrap();
    assert_eq!(stored, bytes);
    let decoded: CookedParticleEffect = serde_json::from_slice(&stored).unwrap();
    assert!(decoded.programs[0].wgsl.contains("sin(ctx.time)"));
    let manifest =
        std::fs::read_to_string(root.join("package/assets/asset-manifest.json")).unwrap();
    assert!(manifest.contains("guid-test-effect") && manifest.contains("particle-effect"));
    std::fs::remove_dir_all(root).unwrap();
}
