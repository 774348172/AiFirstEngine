use engine_runtime::canonical_digest::sha256_prefixed;
use project_authoring_execution::{
    platform_3d_benchmark::*, GameProjectCompiler, ProjectAuthoringSession, TargetProfile,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

// Sessions own the project's existing mutation lock, including read snapshot acquisition.
static PROJECT_SESSION: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[path = "support/bm10_combined_cook.rs"]
mod s5d_bm10;
#[path = "support/bm13_world_cook.rs"]
mod s5d_bm13;

fn project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/platform_3d_benchmark_project")
        .canonicalize()
        .unwrap()
}
fn request() -> BenchmarkRequest {
    BenchmarkRequest {
        revision: "371-request-v1".into(),
        run_id: "371-test-fresh-output".into(),
        output_parent: std::env::temp_dir(),
        project_root: project(),
        source_revision: format!("sha256:{}", "a".repeat(64)),
        target_recipe_sha256: sha256_prefixed(TARGET_RECIPES),
        workload: "BM-02".into(),
        stage_recipe: "static-source-v1".into(),
        mode: "diagnostic".into(),
        operation: "cook".into(),
        target_profile: "windows-dev".into(),
        device_profile: "pc-rtx3070-v1".into(),
        backend: "headless-cpu".into(),
        evidence_level: "summary".into(),
        window: DiagnosticWindow {
            warmup_frames: 0,
            sample_frames: 3,
            repeats: 1,
            minimum_wall_seconds: 0,
        },
        fault_schedule: Vec::new(),
        timeout_ms: 300000,
    }
}

#[test]
fn bm13_world_driver_admits_real_windows_consumers_and_rejects_unimplemented_claims() {
    let mut r = request();
    r.workload = "BM-13".into();
    r.stage_recipe = "world-v1".into();
    r.target_profile = "windows-release".into();
    r.validate()
        .expect("original terrain/vegetation Cook consumer");
    r.operation = "build".into();
    r.validate().unwrap();
    r.operation = "run-windowed".into();
    r.backend = "wgpu-windowed".into();
    r.window.sample_frames = 120;
    r.validate().unwrap();
    r.operation = "run-scenario".into();
    r.validate().unwrap();
    for workload in ["BM-08"] {
        let mut unsupported = r.clone();
        unsupported.workload = workload.into();
        assert!(unsupported.validate().is_err());
    }
    for operation in ["run-headless", "qualify"] {
        let mut unsupported = r.clone();
        unsupported.operation = operation.into();
        assert!(unsupported.validate().is_err());
    }
    r.target_profile = "android-release".into();
    assert!(r.validate().is_err());
}

#[test]
fn bm10_world_driver_requires_its_own_combined_functional_scenario() {
    let mut r = request();
    r.workload = "BM-10".into();
    r.stage_recipe = "world-v1".into();
    r.target_profile = "windows-release".into();
    r.validate().unwrap();
    r.operation = "run-scenario".into();
    r.backend = "wgpu-windowed".into();
    r.window.sample_frames = 360;
    r.validate().unwrap();
    for frames in [120, 359, 361] {
        let mut bad = r.clone();
        bad.window.sample_frames = frames;
        assert!(bad.validate().is_err());
    }
    for (field, value) in [
        ("profile", "android-release"),
        ("mode", "qualify"),
        ("recipe", "sim-v1"),
    ] {
        let mut bad = r.clone();
        match field {
            "profile" => bad.target_profile = value.into(),
            "mode" => bad.mode = value.into(),
            _ => bad.stage_recipe = value.into(),
        };
        assert!(bad.validate().is_err());
    }
}

#[test]
fn foundation_pressure_driver_does_not_claim_full_matrix_or_fault_execution() {
    let mut r = request();
    r.workload = "BM-12".into();
    r.stage_recipe = "foundation-v1".into();
    r.validate().unwrap();
    r.operation = "run-windowed".into();
    r.backend = "wgpu-windowed".into();
    r.validate().unwrap();
    for operation in ["run-scenario", "run-headless"] {
        let mut invalid = r.clone();
        invalid.operation = operation.into();
        assert!(invalid.validate().is_err());
    }
    let mut fault = r.clone();
    fault.fault_schedule.push("F-01".into());
    assert!(fault.validate().is_err());
    let mut full = r.clone();
    full.stage_recipe = "world-v1".into();
    assert!(full.validate().is_err());
    r.window.sample_frames = 121;
    assert!(r.validate().is_err());
}

#[test]
fn foundation_bm01_driver_keeps_complete_scenario_separate_from_short_window() {
    let mut r = request();
    r.workload = "BM-01".into();
    r.stage_recipe = "foundation-v1".into();
    r.validate().expect("actual foundation Cook consumer");
    r.operation = "run-windowed".into();
    r.backend = "wgpu-windowed".into();
    r.validate().unwrap();
    r.window.sample_frames = 7201;
    assert!(
        r.validate().is_err(),
        "ordinary frame diagnostics are still bounded"
    );
    r.operation = "run-scenario".into();
    r.validate()
        .expect("full simulation scenario has a distinct consumer");
    r.window.sample_frames = 7200;
    assert!(
        r.validate().is_err(),
        "initialization cannot replace the final tick"
    );
    r.window.sample_frames = 7201;
    r.target_profile = "android-release".into();
    assert!(r.validate().is_err());
    r.target_profile = "windows-release".into();
    r.workload = "BM-09".into();
    r.window.sample_frames = 63;
    r.validate()
        .expect("three anchors and twenty relocations each");
    r.window.sample_frames = 62;
    assert!(
        r.validate().is_err(),
        "missing final relocation must reject"
    );
    r.window.sample_frames = 63;
    r.workload = "BM-08".into();
    assert!(
        r.validate().is_err(),
        "unimplemented fault/cell scenario stays rejected"
    );
}

#[test]
#[ignore = "explicit generated BM01 source and fresh evidence required"]
fn foundation_bm01_real_source_cook_counts_live_entities_and_hud_before_build() {
    let source = std::env::var_os("S1_FOUNDATION_SOURCE").expect("generated BM01 source");
    let mut r = request();
    r.project_root = PathBuf::from(source);
    r.workload = "BM-01".into();
    r.stage_recipe = "foundation-v1".into();
    r.output_parent =
        PathBuf::from(std::env::var_os("S1_FOUNDATION_EVIDENCE").expect("fresh evidence parent"));
    r.run_id = "bm01-cook".into();
    r.source_revision = project_source_digest(&r.project_root).unwrap();
    let input = r.output_parent.join("cook-request.json");
    fs::write(&input, serde_json::to_vec_pretty(&r).unwrap()).unwrap();
    let output = run_request(&input).unwrap();
    let cooked: Value =
        serde_json::from_slice(&fs::read(output.join("cook.json")).unwrap()).unwrap();
    assert_eq!(cooked["pureSpatial"], 10000);
    assert_eq!(cooked["hud"], 20);
    assert_eq!(cooked["cameras"], 1);
    assert_eq!(cooked["lights"], 0);
    let result: Value =
        serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["qualification"]["performance"], "unqualified");
    assert_eq!(result["player"], "not_executed");
}

#[test]
fn s2d_lit_request_accepts_windows_delivery_without_granting_qualification() {
    let mut value = request();
    value.workload = "BM-04".into();
    value.stage_recipe = "lit-v1".into();
    value.validate().expect("BM04 lit Cook has a real consumer");
    value.target_profile = "windows-release".into();
    value
        .validate()
        .expect("Release-target Cook is not Release performance");
    value.target_profile = "android-release".into();
    assert!(value.validate().is_err(), "Android lit is not implemented");
    value.target_profile = "windows-dev".into();
    value.operation = "build".into();
    value.validate().expect("real Compiler build consumer");
    value.operation = "run-windowed".into();
    assert!(
        value.validate().is_err(),
        "headless cannot prove a GPU window"
    );
    value.backend = "wgpu-windowed".into();
    value.validate().expect("bounded real window consumer");
    for stage in ["lit-v1", "lit-mini-v1"] {
        value.stage_recipe = stage.into();
        for target in ["windows-dev", "windows-release"] {
            value.target_profile = target.into();
            value.validate().unwrap();
        }
    }
    value.operation = "run-headless".into();
    value.backend = "headless-cpu".into();
    assert!(
        value.validate().is_err(),
        "headless does not consume lit drawing"
    );
    value.operation = "cook".into();
    value.mode = "qualification".into();
    assert!(value.validate().is_err());
}

#[test]
fn s2d_window_backend_is_not_a_fallback_for_other_recipes_or_operations() {
    let mut value = request();
    value.operation = "run-windowed".into();
    value.backend = "wgpu-windowed".into();
    assert!(
        value.validate().is_err(),
        "static window consumer not implemented here"
    );
    value.workload = "BM-04".into();
    value.stage_recipe = "lit-mini-v1".into();
    value.validate().unwrap();
    for operation in ["cook", "build", "run-headless"] {
        value.operation = operation.into();
        assert!(
            value.validate().is_err(),
            "GPU execution must not be claimed for {operation}"
        );
    }
    value.operation = "run-windowed".into();
    value.target_profile = "android-release".into();
    assert!(value.validate().is_err());
    value.target_profile = "windows-dev".into();
    value.window.sample_frames = 121;
    assert!(
        value.validate().is_err(),
        "bounded diagnostic is not a qualification window"
    );
}

#[test]
fn s2d_mini_driver_consumes_real_cook_and_preserves_full_scale_rejection() {
    let _exclusive_project = PROJECT_SESSION.lock().unwrap();
    let mut value = request();
    value.project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/platform_3d_lit_project")
        .canonicalize()
        .unwrap();
    value.source_revision = project_source_digest(&value.project_root).unwrap();
    let owned = std::env::temp_dir().join(format!(
        "371-lit-driver-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&owned).unwrap();
    value.output_parent = owned.clone();
    value.run_id = "mini-cook".into();
    value.workload = "BM-04".into();
    value.stage_recipe = "lit-mini-v1".into();
    let input = owned.join("request.json");
    fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    let output = run_request(&input).unwrap();
    let result: Value =
        serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
    let cook: Value = serde_json::from_slice(&fs::read(output.join("cook.json")).unwrap()).unwrap();
    assert_eq!(cook["instances"], 20);
    assert_eq!(cook["scope"], "diagnostic-mini-ineligible");
    assert_eq!(result["player"], "not_executed");
    assert_eq!(result["qualification"]["performance"], "unqualified");
    assert_eq!(result["measurement"]["buildMs"], 0.0);
    assert!(!output.join("player.json").exists());
    assert!(!output.join("delivery.json").exists());
    assert_eq!(
        run_request(&input).unwrap_err().code,
        "benchmark.output_exists"
    );

    // A fresh evidence directory must not force another Cook of unchanged
    // payloads. The Compiler still validates the exact source on every request.
    value.run_id = "mini-cache-hit".into();
    fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    let cached_output = run_request(&input).unwrap();
    let reused: Value =
        serde_json::from_slice(&fs::read(cached_output.join("prepare.json")).unwrap()).unwrap();
    assert_eq!(
        reused["producedCount"], 0,
        "unchanged Cook producers should be reused across fresh runs: {reused}"
    );
    assert_eq!(reused["reusedCount"], reused["producerCount"]);

    value.run_id = "full-scale-rejected".into();
    value.stage_recipe = "lit-v1".into();
    fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        run_request(&input).unwrap_err().code,
        "benchmark.lit_payload_ineligible"
    );
    let failed = owned.join("full-scale-rejected");
    assert!(failed.join("failure.json").exists());
    assert!(!failed.join("result.json").exists());
    assert!(!failed.join("delivery.json").exists());
    // Only this freshly created, exact test-owned child is disposable. Failure
    // above intentionally leaves its first evidence intact for investigation.
    fs::remove_dir_all(owned).unwrap();
}

#[test]
#[ignore = "requires the generated run-owned foundation source and fresh evidence output"]
fn foundation_driver_cooks_the_actual_ordinary_project_without_granting_qualification() {
    let mut value = request();
    value.project_root = std::env::var_os("AIFE_CELL_PLAYER_PROJECT")
        .map(PathBuf::from)
        .unwrap();
    value.output_parent = std::env::var_os("AIFE_CELL_PLAYER_EVIDENCE")
        .map(PathBuf::from)
        .unwrap();
    fs::create_dir_all(&value.output_parent).unwrap();
    value.run_id = "foundation-driver".into();
    value.source_revision = project_source_digest(&value.project_root).unwrap();
    value.workload = "BM-08".into();
    value.stage_recipe = "foundation-v1".into();
    let path = value.output_parent.join("foundation-driver-request.json");
    fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let output = run_request(&path).unwrap();
    let cells: Value =
        serde_json::from_slice(&fs::read(output.join("cook.json")).unwrap()).unwrap();
    assert_eq!(cells.as_array().unwrap().len(), 2);
    assert!(cells
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["uniqueUploadBytes"].as_u64().unwrap() >= 32 * 1024 * 1024));
    assert!(!output.join("delivery.json").exists());
}

#[test]
fn foundation_diagnostic_request_never_grants_formal_or_headless_cell_qualification() {
    let mut value = request();
    value.workload = "BM-08".into();
    value.stage_recipe = "foundation-v1".into();
    value.validate().unwrap();
    value.mode = "qualification".into();
    assert_eq!(
        value.validate().unwrap_err().code,
        "benchmark.recipe_unimplemented"
    );
    value.mode = "diagnostic".into();
    value.operation = "run-headless".into();
    assert_eq!(
        value.validate().unwrap_err().code,
        "benchmark.profile_unimplemented"
    );
    value.operation = "build".into();
    value.target_profile = "android-release".into();
    assert_eq!(
        value.validate().unwrap_err().code,
        "benchmark.profile_unimplemented"
    );
    value.target_profile = "windows-release".into();
    value.validate().unwrap();
}

#[test]
fn full_static_cook_request_does_not_grant_runtime_or_platform_qualification() {
    let mut value = request();
    value.workload = "BM-14".into();
    value.stage_recipe = "foundation-v1".into();
    value.target_profile = "windows-release".into();
    value.validate().unwrap();
    value.mode = "qualification".into();
    assert_eq!(
        value.validate().unwrap_err().code,
        "benchmark.recipe_unimplemented"
    );
    value.mode = "diagnostic".into();
    for operation in ["build", "run-headless"] {
        value.operation = operation.into();
        assert_eq!(
            value.validate().unwrap_err().code,
            "benchmark.profile_unimplemented"
        );
    }
    value.operation = "cook".into();
    value.target_profile = "android-release".into();
    assert_eq!(
        value.validate().unwrap_err().code,
        "benchmark.profile_unimplemented"
    );
}

#[test]
fn recipe_catalog_preserves_all_final_pc_android_targets_and_stage_gaps() {
    let catalog = target_catalog().unwrap();
    let rows = catalog["workloads"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        15
    );
    assert_eq!(rows[1]["pc"]["instances"], 10000);
    assert_eq!(rows[1]["android"]["instances"], 2000);
    assert_eq!(rows[2]["pc"]["meshes"], 10000);
    assert_eq!(rows[4]["pc"]["characters"], 100);
    assert_eq!(rows[5]["pc"]["dynamicBodies"], 1000);
    assert_eq!(rows[6]["android"]["navAgents"], 200);
    assert_eq!(rows[7]["pc"]["gpuDatasetMinBytes"], 8 * GIB);
    assert_eq!(rows[9]["pc"]["liveInstances"], 50000);
    assert_eq!(rows[12]["android"]["samplesPerTileSide"], 129);
    assert_eq!(rows[14]["pc"]["anchorMetres"], json!([0, 1000000]));
    assert_eq!(rows[3]["pc"]["point"], 128);
    assert!(rows.iter().all(|r| r["executionStatus"] == "target_only"));
    assert_eq!(
        catalog["layout"]["finalTextureSemantics"],
        json!(["baseColor", "normal", "ORM"])
    );
    assert_eq!(
        catalog["executable"]["BM-02/static-source-v1"]["notExecuted"][0],
        "PBR"
    );
}

#[test]
fn recipe_rng_matches_independent_published_vector_and_integer_seed_mapping() {
    let mut state = 1;
    assert_eq!(
        (0..5).map(|_| xorshift32(&mut state)).collect::<Vec<_>>(),
        vec![270369, 67634689, 2647435461, 307599695, 2398689233]
    );
    for (index, row) in target_catalog().unwrap()["workloads"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(
            row["seed"].as_u64().unwrap(),
            u64::from(BASE_SEED ^ (index as u32 + 1))
        );
    }
}

#[test]
fn recipe_camera_keys_cover_full_loop_and_canonical_orientation() {
    let catalog = target_catalog().unwrap();
    for track in ["K1", "K3"] {
        let keys = catalog["tracks"][track]["keys"].as_array().unwrap();
        assert_eq!(keys.len(), 5);
        assert_eq!(keys[0]["position"], keys[4]["position"]);
        assert_eq!(keys[4]["tick"], 7200);
        for (i, key) in keys.iter().enumerate() {
            assert_eq!(key["tick"], i * 1800);
            let q = key["rotationXyzw"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>();
            assert!((q.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-12);
            let [x, y, z, w] = [q[0], q[1], q[2], q[3]];
            let forward = [
                -2. * (x * z + y * w),
                -2. * (y * z - x * w),
                -(1. - 2. * (x * x + y * y)),
            ];
            let delta = std::array::from_fn::<_, 3, _>(|j| {
                key["target"][j].as_f64().unwrap() - key["position"][j].as_f64().unwrap()
            });
            let length = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
            for j in 0..3 {
                assert!((forward[j] - delta[j] / length).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn recipe_native_sources_have_real_distinct_topology_and_exact_grid_bindings() {
    let root = project();
    let scene: Value =
        serde_json::from_slice(&fs::read(root.join("Scenes/Main.scene.json")).unwrap()).unwrap();
    assert_eq!(scene["entities"].as_array().unwrap().len(), 10001);
    assert_eq!(
        scene["entities"][0]["components"][0]["data"]["translation"],
        json!([-99, -99, 0])
    );
    assert_eq!(
        scene["entities"][9999]["components"][0]["data"]["translation"],
        json!([99, 99, 0])
    );
    let mut hashes = BTreeSet::new();
    for i in 0..100 {
        let source: Value = serde_json::from_slice(
            &fs::read(root.join(format!("Assets/mesh-{i:03}.json"))).unwrap(),
        )
        .unwrap();
        let g = &source["renderData"]["data"]["payload"];
        assert_eq!(g["vertexCount"], 600);
        assert_eq!(g["indexCount"], 3000);
        let bytes = |field: &str| {
            g[field]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect::<Vec<_>>()
        };
        let vb = bytes("vertexBytes");
        let ib = bytes("indexBytes");
        assert!(hashes.insert(sha256_prefixed(&vb)));
        let mut triangles = BTreeSet::new();
        for face in ib.chunks_exact(12) {
            let indices = std::array::from_fn::<_, 3, _>(|j| {
                u32::from_le_bytes(face[j * 4..j * 4 + 4].try_into().unwrap()) as usize
            });
            assert!(indices.iter().all(|v| *v < 600));
            assert!(triangles.insert(indices));
            let p = indices.map(|v| {
                std::array::from_fn::<_, 3, _>(|j| {
                    f32::from_le_bytes(vb[v * 32 + j * 4..v * 32 + j * 4 + 4].try_into().unwrap())
                })
            });
            let twice_area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
            assert!(twice_area > 0.001); // Real counter-clockwise faces, not repeated padded triangles.
        }
        assert_eq!(triangles.len(), 1000);
    }
}

#[test]
fn recipe_real_compiler_cooks_frozen_static_source_counts_and_payloads_deterministically() {
    let _exclusive_project = PROJECT_SESSION.lock().unwrap();
    let mut session = ProjectAuthoringSession::open(project()).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("371-assets-test", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let first = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    let evidence = inspect_static_cook(&first).unwrap();
    assert_eq!(evidence.unique_geometry_triangles, 100000);
    assert_eq!(evidence.vertex_bytes, 100 * 600 * 32);
    assert_eq!(evidence.index_bytes, 100 * 3000 * 4);
    assert_eq!(evidence.base_color_decoded_bytes, 8 * 2048 * 2048 * 4);
    let second = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    assert_eq!(evidence, inspect_static_cook(&second).unwrap());
    assert_eq!(first.assembly_digest(), second.assembly_digest());
}

#[test]
fn recipe_request_rejects_future_qualification_profile_and_unowned_output() {
    request().validate().unwrap();
    for (field, value, code) in [
        ("mode", "qualification", "benchmark.recipe_unimplemented"),
        (
            "stageRecipe",
            "foundation-v1",
            "benchmark.recipe_unimplemented",
        ),
        ("workload", "BM-05", "benchmark.recipe_unimplemented"),
        (
            "targetProfile",
            "android-release",
            "benchmark.profile_unimplemented",
        ),
        ("backend", "Vulkan", "benchmark.profile_unimplemented"),
        ("runId", "../escape", "benchmark.output_ownership"),
    ] {
        let mut value_request = serde_json::to_value(request()).unwrap();
        value_request[field] = json!(value);
        let decoded: BenchmarkRequest = serde_json::from_value(value_request).unwrap();
        assert_eq!(decoded.validate().unwrap_err().code, code);
    }
    let mut value = serde_json::to_value(request()).unwrap();
    value["extra"] = json!(true);
    assert!(serde_json::from_value::<BenchmarkRequest>(value).is_err());
    let mut bad = request();
    bad.fault_schedule.push("F-01".into());
    assert_eq!(
        bad.validate().unwrap_err().code,
        "benchmark.diagnostic_scope"
    );
}

#[test]
fn recipe_driver_retains_first_failure_and_refuses_overwrite() {
    let _exclusive_project = PROJECT_SESSION.lock().unwrap();
    let dir = std::env::temp_dir().join(format!(
        "371-driver-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    let mut rejected = request();
    rejected.output_parent = dir.clone();
    rejected.mode = "qualification".into();
    let path = dir.join("request.json");
    fs::write(&path, serde_json::to_vec(&rejected).unwrap()).unwrap();
    assert_eq!(
        run_request(&path).unwrap_err().code,
        "benchmark.recipe_unimplemented"
    );
    let failure = dir.join(&rejected.run_id).join("failure.json");
    let original = fs::read(&failure).unwrap();
    assert_eq!(
        run_request(&path).unwrap_err().code,
        "benchmark.output_exists"
    );
    assert_eq!(fs::read(&failure).unwrap(), original);
    rejected.run_id = "371-changed-source".into();
    rejected.mode = "diagnostic".into();
    fs::write(&path, serde_json::to_vec(&rejected).unwrap()).unwrap();
    assert_eq!(
        run_request(&path).unwrap_err().code,
        "benchmark.source_changed"
    );
    // Retain the tiny owned failure fixture as evidence, no broad recursive cleanup.
}

fn reset() -> ResetBoundary {
    ResetBoundary {
        warmup_frames: 600,
        camera_tick: 0,
        input_cursor: 0,
        event_cursor: 0,
        measurement_time_ns: 0,
        initial_simulation_restored: true,
    }
}
fn coverage() -> WindowCoverage {
    WindowCoverage {
        wall_seconds: 120.,
        simulated_seconds: 120.,
        simulation_ticks: 7200,
        presented_frames: 7200,
        complete_trajectory_cycles: 1,
        distinct_cells: 0,
        revisited_cells: 0,
        uploaded_bytes: 0,
    }
}
fn round(pid: u32) -> MeasurementRound {
    MeasurementRound {
        process_id: pid,
        identity: "identical-source-package-backend-quality-instruments".into(),
        reset: reset(),
        coverage: coverage(),
        cpu: distribution(&vec![10.; 7200], 16.67).unwrap(),
        gpu: Some(distribution(&vec![9.; 7200], 16.67).unwrap()),
        present: Some(distribution(&vec![1000. / 60.; 7199], 33.4).unwrap()),
        unmatched_events: 0,
        minute_presented_counts: vec![3600, 3600],
    }
}

#[test]
fn measurement_nearest_rank_preserves_outliers_and_rejects_invalid_values() {
    let seq = (1..=101).map(|v| v as f64).collect::<Vec<_>>();
    let d = distribution(&seq, 99.).unwrap();
    assert_eq!(
        (d.p50, d.p95, d.p99, d.max, d.exceeded),
        (51., 96., 100., 101., 2)
    );
    assert_eq!(d.mean, 51.);
    assert_eq!(distribution(&[1., 2.], 0.).unwrap().p50, 1.);
    for bad in [Vec::new(), vec![f64::NAN], vec![f64::INFINITY], vec![-1.]] {
        assert!(distribution(&bad, 10.).is_err());
    }
}

#[test]
fn measurement_warmup_requires_joint_camera_input_event_clock_reset() {
    reset().validate(600).unwrap();
    for field in [
        "cameraTick",
        "inputCursor",
        "eventCursor",
        "measurementTimeNs",
        "warmupFrames",
    ] {
        let mut value = serde_json::to_value(reset()).unwrap();
        value[field] = json!(1);
        let bad: ResetBoundary = serde_json::from_value(value).unwrap();
        assert_eq!(
            bad.validate(600).unwrap_err().code,
            "measurement.reset_incomplete"
        );
    }
    let mut bad = reset();
    bad.initial_simulation_restored = false;
    assert!(bad.validate(600).is_err());
}

#[test]
fn measurement_window_requires_full_intersection_and_rejects_slow_motion() {
    validate_window(QualificationWindow::Trajectory, false, &coverage()).unwrap();
    for variant in 0..4 {
        let mut c = coverage();
        match variant {
            0 => c.wall_seconds = 119.,
            1 => c.simulation_ticks = 7199,
            2 => c.presented_frames = 7199,
            _ => c.complete_trajectory_cycles = 0,
        };
        assert_eq!(
            validate_window(QualificationWindow::Trajectory, false, &c)
                .unwrap_err()
                .code,
            "measurement.window_incomplete"
        );
    }
    let mut slow = coverage();
    slow.wall_seconds = 180.;
    assert_eq!(
        validate_window(QualificationWindow::Trajectory, false, &slow)
            .unwrap_err()
            .code,
        "measurement.simulation_slowed"
    );
    let mut extended = coverage();
    extended.wall_seconds = 240.;
    extended.simulated_seconds = 240.;
    extended.simulation_ticks = 14400;
    extended.presented_frames = 14400;
    assert!(validate_window(QualificationWindow::Trajectory, false, &extended).is_err());
    extended.complete_trajectory_cycles = 2;
    validate_window(QualificationWindow::Trajectory, false, &extended).unwrap();
}

#[test]
fn measurement_streaming_requires_distinct_cells_revisits_and_more_than_twice_residency() {
    let c = WindowCoverage {
        wall_seconds: 600.,
        simulated_seconds: 600.,
        simulation_ticks: 36000,
        presented_frames: 36000,
        complete_trajectory_cycles: 1,
        distinct_cells: 40,
        revisited_cells: 4,
        uploaded_bytes: 4 * GIB + 1,
    };
    validate_window(QualificationWindow::Streaming, false, &c).unwrap();
    for field in 0..3 {
        let mut bad = c.clone();
        match field {
            0 => bad.distinct_cells = 39,
            1 => bad.revisited_cells = 3,
            _ => bad.uploaded_bytes = 4 * GIB,
        };
        assert_eq!(
            validate_window(QualificationWindow::Streaming, false, &bad)
                .unwrap_err()
                .code,
            "measurement.streaming_incomplete"
        );
    }
}

#[test]
fn recipe_target_layouts_match_static_sources_and_world_coverage_without_future_cook() {
    let layout: Value =
        serde_json::from_slice(&fs::read(project().join("Benchmark/target-layouts.json")).unwrap())
            .unwrap();
    assert_eq!(
        layout["workloads"]["BM-02/pc"]["instances"]
            .as_array()
            .unwrap()
            .len(),
        10000
    );
    for i in 0..10000 {
        assert_eq!(
            layout["workloads"]["BM-03/pc"]["instances"][i]["position"],
            layout["workloads"]["BM-02/pc"]["instances"][i]["position"]
        );
    }
    assert_eq!(
        layout["workloads"]["BM-02/pc"]["instances"][0]["position"],
        json!([-99, -99, 0])
    );
    assert_eq!(
        layout["workloads"]["BM-08/pc"]["cells"]
            .as_array()
            .unwrap()
            .len(),
        256
    );
    let catalog = target_catalog().unwrap();
    for (track, min) in [("K2-16", 40), ("K2-8", 24)] {
        let keys = catalog["tracks"][track]["keys"].as_array().unwrap();
        let distinct = keys
            .iter()
            .filter(|key| key["tick"].as_u64().unwrap() <= 36000)
            .map(|key| key["cell"].to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            distinct.len() as u64,
            catalog["tracks"][track]["distinctCellsIn600Seconds"]
                .as_u64()
                .unwrap()
        );
        assert!(distinct.len() >= min as usize);
        assert!(
            catalog["tracks"][track]["distinctCellsIn600Seconds"]
                .as_u64()
                .unwrap()
                >= min
        );
        assert!(keys
            .iter()
            .any(|key| key["tick"] == 7200 && key["operation"] == "teleport"));
        assert!(keys
            .windows(2)
            .all(|pair| pair[0]["tick"].as_u64().unwrap() <= pair[1]["tick"].as_u64().unwrap()));
    }
    let frozen: Value =
        serde_json::from_slice(&fs::read(project().join("Benchmark/budget-v1.json")).unwrap())
            .unwrap();
    assert_eq!(
        frozen["pcMemory"]["cpuTrackedAggregate"],
        MemoryBudget::pc_main().cpu_total
    );
    assert_eq!(
        frozen["pcMemory"]["gpuTrackedAllocated"],
        MemoryBudget::pc_main().gpu_total
    );
    assert_eq!(frozen["revision"], "371-budget-v3");
    assert_eq!(
        frozen["pcMemory"]["roleLimits"]["gpu.resident"],
        MemoryBudget::pc_main().role_limits["gpu.resident"]
    );
    assert_eq!(
        frozen["pcMemory"]["processPrivatePeak"],
        MemoryBudget::pc_main().process_private_peak
    );
    assert_eq!(
        frozen["pcMemory"]["minimumSystemAvailable"],
        MemoryBudget::pc_main().minimum_system_available
    );
    assert!(frozen["android"]["totalMemoryBudgets"].is_null());
}

#[test]
fn measurement_present_join_deduplicates_repeated_image_but_keeps_gap() {
    let associations = vec![
        PresentAssociation {
            present_id: 11,
            packet_id: 1,
            submission_id: 21,
            snapshot_epoch: 1,
        },
        PresentAssociation {
            present_id: 12,
            packet_id: 2,
            submission_id: 22,
            snapshot_epoch: 1,
        },
    ];
    let events = vec![
        PresentedEvent {
            present_id: 11,
            time_ns: 0,
            compositor_repeat: false,
        },
        PresentedEvent {
            present_id: 11,
            time_ns: 16_000_000,
            compositor_repeat: true,
        },
        PresentedEvent {
            present_id: 12,
            time_ns: 40_000_000,
            compositor_repeat: false,
        },
    ];
    let result = correlate_present(&associations, &events).unwrap();
    assert_eq!(result.unique_application_frames, 2);
    assert_eq!(result.duplicate_events, 1);
    assert_eq!(result.intervals_ms, vec![40.]);
    assert_eq!(
        present_minute_counts(&result, 0, 60_000_000_000).unwrap(),
        vec![2]
    );
    let mut unmatched = events.clone();
    unmatched[2].present_id = 999;
    assert_eq!(
        correlate_present(&associations, &unmatched)
            .unwrap_err()
            .code,
        "measurement.unmatched_present"
    );
    let mut backward = events.clone();
    backward[2].time_ns = 1;
    assert!(correlate_present(&associations, &backward).is_err());
    let mut ambiguous = associations.clone();
    ambiguous.push(associations[0].clone());
    assert!(correlate_present(&ambiguous, &events).is_err());
}

#[test]
fn measurement_every_repeat_and_every_minute_pass_independently() {
    let mut rounds = vec![round(1), round(2), round(3)];
    assess_three_rounds(&rounds, QualificationWindow::Trajectory, false).unwrap();
    rounds[1].cpu = distribution(&vec![17.; 7200], 16.67).unwrap();
    assert_eq!(
        assess_three_rounds(&rounds, QualificationWindow::Trajectory, false)
            .unwrap_err()
            .code,
        "measurement.round_budget"
    );
    rounds[1] = round(2);
    rounds[1].minute_presented_counts = vec![3500, 3700];
    assert_eq!(
        assess_three_rounds(&rounds, QualificationWindow::Trajectory, false)
            .unwrap_err()
            .code,
        "measurement.minute_rate"
    );
    rounds[1] = round(2);
    rounds[1].gpu = None;
    assert_eq!(
        assess_three_rounds(&rounds, QualificationWindow::Trajectory, false)
            .unwrap_err()
            .code,
        "measurement.instrument_unavailable"
    );
    rounds[1] = round(2);
    rounds[1].unmatched_events = 1;
    assert!(assess_three_rounds(&rounds, QualificationWindow::Trajectory, false).is_err());
    rounds[1] = round(1);
    assert_eq!(
        assess_three_rounds(&rounds, QualificationWindow::Trajectory, false)
            .unwrap_err()
            .code,
        "measurement.repeat_identity"
    );
}

#[test]
fn measurement_execution_dimensions_do_not_require_os_display_or_each_other() {
    let mut rounds = vec![round(1), round(2), round(3)];
    for r in &mut rounds {
        r.present = None;
        r.coverage.presented_frames = 0;
        r.unmatched_events = 5;
        r.minute_presented_counts.clear();
    }
    for dimension in [
        ExecutionMeasurementDimension::Cpu,
        ExecutionMeasurementDimension::Gpu,
    ] {
        assess_execution_three_rounds(
            &rounds,
            QualificationWindow::Trajectory,
            false,
            dimension,
            &[7200; 3],
        )
        .unwrap();
    }
    assert!(assess_three_rounds(&rounds, QualificationWindow::Trajectory, false).is_err());
    rounds[1].gpu = None;
    assess_execution_three_rounds(
        &rounds,
        QualificationWindow::Trajectory,
        false,
        ExecutionMeasurementDimension::Cpu,
        &[7200; 3],
    )
    .unwrap();
    assert_eq!(
        assess_execution_three_rounds(
            &rounds,
            QualificationWindow::Trajectory,
            false,
            ExecutionMeasurementDimension::Gpu,
            &[7200; 3]
        )
        .unwrap_err()
        .code,
        "measurement.instrument_unavailable"
    );
}

#[test]
fn measurement_execution_dimensions_preserve_budget_window_and_identity_rejection() {
    let mut rounds = vec![round(1), round(2), round(3)];
    let check = |rs: &[MeasurementRound], frames: &[u64]| {
        assess_execution_three_rounds(
            rs,
            QualificationWindow::Trajectory,
            false,
            ExecutionMeasurementDimension::Cpu,
            frames,
        )
    };
    check(&rounds, &[7200; 3]).unwrap();
    assert!(check(&rounds, &[7200, 7199, 7200]).is_err());
    assert!(check(&rounds, &[7200; 2]).is_err());
    rounds[1].cpu = distribution(&vec![17.; 7200], 16.67).unwrap();
    assert_eq!(
        check(&rounds, &[7200; 3]).unwrap_err().code,
        "measurement.round_budget"
    );
    rounds[1] = round(2);
    rounds[1].cpu.p99 = f64::NAN;
    assert_eq!(
        check(&rounds, &[7200; 3]).unwrap_err().code,
        "measurement.invalid_distribution"
    );
    rounds[1] = round(2);
    rounds[1].coverage.wall_seconds = 119.;
    assert!(check(&rounds, &[7200; 3]).is_err());
    rounds[1] = round(2);
    rounds[1].identity = "other-binary".into();
    assert_eq!(
        check(&rounds, &[7200; 3]).unwrap_err().code,
        "measurement.repeat_identity"
    );
}

fn allocation(
    id: &str,
    domain: MemoryDomain,
    owner: &str,
    bytes: u64,
    role: &str,
) -> AllocationSample {
    AllocationSample {
        allocation_id: id.into(),
        domain,
        owner: owner.into(),
        bytes,
        roles: BTreeSet::from([role.into()]),
    }
}
fn capacities() -> BTreeMap<String, u64> {
    MemoryBudget::pc_main()
        .capacity_limits
        .keys()
        .map(|key| (key.clone(), 0))
        .collect()
}
fn external() -> ExternalMemorySample {
    ExternalMemorySample {
        process_private: Some(5 * GIB),
        system_available: Some(8 * GIB),
        gpu_available: Some(3 * GIB),
    }
}

#[test]
fn measurement_memory_counts_shared_allocation_once_across_overlapping_roles() {
    let resident = allocation("shared", MemoryDomain::Gpu, "assets", 64 * MIB, "resident");
    let mut retiring = resident.clone();
    retiring.roles = BTreeSet::from(["retiring".into()]);
    let totals = memory_totals(&[resident.clone(), resident, retiring]).unwrap();
    assert_eq!(totals.gpu_allocated, 64 * MIB);
    assert_eq!(totals.unique_allocations, 1);
    assert_eq!(totals.by_role["gpu.retiring"], 64 * MIB);
    assert_eq!(totals.by_role["gpu.resident"], 64 * MIB);
}

#[test]
fn measurement_memory_four_gib_resident_keeps_total_and_process_limits() {
    let budget = MemoryBudget::pc_main();
    let totals = memory_totals(&[allocation(
        "resident",
        MemoryDomain::Gpu,
        "renderer",
        4 * GIB,
        "resident",
    )])
    .unwrap();
    let mut measured = external();
    measured.process_private = Some(6 * GIB);
    assert_eq!(
        assess_memory(&budget, &totals, &measured, &capacities()).status,
        "passed"
    );
    measured.process_private = Some(6 * GIB + 1);
    assert_eq!(
        assess_memory(&budget, &totals, &measured, &capacities()).breaches,
        vec!["process.private"]
    );
    let over = memory_totals(&[allocation(
        "resident",
        MemoryDomain::Gpu,
        "renderer",
        4 * GIB + 1,
        "resident",
    )])
    .unwrap();
    let breaches = assess_memory(&budget, &over, &external(), &capacities()).breaches;
    assert!(breaches.contains(&"gpu.total".into()));
    assert!(breaches.contains(&"gpu.resident".into()));
}

#[test]
fn measurement_memory_rejects_total_even_when_each_category_passes() {
    let totals = memory_totals(&[
        allocation("world", MemoryDomain::Cpu, "world", 3 * GIB, "other"),
        allocation(
            "cache",
            MemoryDomain::Cpu,
            "assets",
            GIB + GIB / 2,
            "decodedCache",
        ),
    ])
    .unwrap();
    let verdict = assess_memory(
        &MemoryBudget::pc_main(),
        &totals,
        &external(),
        &capacities(),
    );
    assert_eq!(verdict.status, "failed");
    assert_eq!(verdict.breaches, vec!["cpu.total"]);
}

#[test]
fn measurement_memory_conflicts_overflow_limits_and_missing_instruments_are_explicit() {
    let a = allocation("a", MemoryDomain::Cpu, "world", 1, "other");
    let mut b = a.clone();
    b.bytes = 2;
    assert!(memory_totals(&[a, b]).is_err());
    assert_eq!(
        memory_totals(&[
            allocation("a", MemoryDomain::Cpu, "world", u64::MAX, "other"),
            allocation("b", MemoryDomain::Cpu, "world", 1, "other")
        ])
        .unwrap_err()
        .code,
        "measurement.memory_overflow"
    );
    let totals = memory_totals(&[]).unwrap();
    assert_eq!(
        assess_memory(
            &MemoryBudget::pc_main(),
            &totals,
            &external(),
            &capacities()
        )
        .status,
        "passed"
    );
    let unavailable = ExternalMemorySample {
        process_private: None,
        system_available: None,
        gpu_available: None,
    };
    assert_eq!(
        assess_memory(
            &MemoryBudget::pc_main(),
            &totals,
            &unavailable,
            &BTreeMap::new()
        )
        .status,
        "unqualified"
    );
    let mut over = capacities();
    over.insert("activeRequests".into(), 257);
    assert!(
        assess_memory(&MemoryBudget::pc_main(), &totals, &external(), &over)
            .breaches
            .contains(&"activeRequests".into())
    );
    let mut low = external();
    let minimum = 4 * GIB + GIB / 2;
    assert_eq!(MemoryBudget::pc_main().minimum_system_available, minimum);
    for available in [minimum, minimum + 1, 5 * GIB] {
        low.system_available = Some(available);
        assert_eq!(
            assess_memory(&MemoryBudget::pc_main(), &totals, &low, &capacities()).status,
            "passed"
        );
    }
    low.system_available = Some(minimum - 1);
    assert!(
        assess_memory(&MemoryBudget::pc_main(), &totals, &low, &capacities())
            .breaches
            .contains(&"system.available".into())
    );
}

#[test]
fn measurement_summary_overhead_is_bound_to_same_identity_and_allowance() {
    assert_eq!(
        summary_overhead(10., 10.3, true).unwrap(),
        0.3000000000000007
    );
    assert!(summary_overhead(10., 10.31, true).is_err());
    assert!(summary_overhead(1., 1.21, true).is_err());
    assert!(summary_overhead(10., 10., false).is_err());
    assert!(summary_overhead(f64::NAN, 10., true).is_err());
}

#[test]
fn recipe_release_driver_selects_product_target_and_rejects_android_pc_fallback() {
    let mut pc = request();
    pc.target_profile = "windows-release".into();
    pc.operation = "build".into();
    pc.validate().unwrap();
    let mut android = pc.clone();
    android.target_profile = "android-release".into();
    assert_eq!(
        android.validate().unwrap_err().code,
        "benchmark.profile_unimplemented"
    );
    android.device_profile = "android-toolchain-unqualified".into();
    android.backend = "native-build".into();
    android.validate().unwrap();
    android.operation = "run-headless".into();
    assert!(android.validate().is_err());
}

#[test]
fn recipe_delivery_smoke_does_not_become_bm02_capacity_evidence() {
    let mut smoke = request();
    smoke.workload = "DELIVERY".into();
    smoke.stage_recipe = "delivery-source-v1".into();
    smoke.target_profile = "windows-release".into();
    smoke.validate().unwrap();
    smoke.workload = "BM-02".into();
    assert_eq!(
        smoke.validate().unwrap_err().code,
        "benchmark.recipe_unimplemented"
    );
}

mod s5d_bm07 {
    use super::*;
    #[test]
    fn full_bm07_request_is_its_own_simulation_workload() {
        let mut r = request();
        r.workload = "BM-07".into();
        r.stage_recipe = "sim-v1".into();
        r.validate().unwrap();
        r.operation = "run-scenario".into();
        r.backend = "wgpu-windowed".into();
        r.window.sample_frames = 1200;
        r.validate().unwrap();
        r.window.sample_frames = 720;
        assert!(r.validate().is_err());
    }
    #[test]
    #[ignore = "explicit full BM07 Compiler Cook and seven real-payload rejection cases"]
    fn actual_full_bm07_cook_counts_native_tiles_agents_motors_and_obstacles() {
        let project = PathBuf::from(std::env::var_os("AIFE_BM07_PROJECT").unwrap());
        let evidence = PathBuf::from(std::env::var_os("AIFE_BM07_EVIDENCE").unwrap());
        fs::create_dir(&evidence).unwrap();
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("bm07-full-cook-owner", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let result = compiler.prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &evidence.parent().unwrap().join(".lit-cook-cache"),
        );
        if let Err(error) = &result {
            fs::write(evidence.join("first-failure.json"),serde_json::to_vec_pretty(&json!({"code":error.code(),"message":error.message(),"diagnostics":error.diagnostics()})).unwrap()).unwrap();
        }
        let prepared = result.unwrap();
        let actual = inspect_simulation_cook(&prepared, "BM-07").unwrap();
        let mut rejected = Vec::new();
        for case in [
            "missing-agent",
            "disabled-motor",
            "missing-obstacle",
            "missing-tile",
            "bad-native-tile",
            "wrong-tile-width",
            "missing-prefab",
        ] {
            let mut bad = prepared.runtime_package_build_input().clone();
            match case {
                "missing-prefab" => bad.prefabs.clear(),
                "missing-agent" => bad.scenes[0].entities.retain(|e| e.id != "agent-0999"),
                "missing-obstacle" => bad.scenes[0].entities.retain(|e| e.id != "obstacle-49"),
                "disabled-motor" => {
                    bad.scenes[0]
                        .entities
                        .iter_mut()
                        .find(|e| e.id == "agent-0000")
                        .unwrap()
                        .components
                        .iter_mut()
                        .find(|c| c.component_type == "engine.motor3d")
                        .unwrap()
                        .data["enabled"] = json!(false);
                }
                _ => {
                    let asset = bad
                        .assets
                        .iter_mut()
                        .find(|a| a.asset_id == "navigation")
                        .unwrap();
                    let mut nav: engine_runtime::navigation_3d::CookedNavigation3D =
                        serde_json::from_slice(asset.runtime_payload.as_ref().unwrap()).unwrap();
                    match case {
                        "missing-tile" => {
                            nav.tiles.pop();
                        }
                        "bad-native-tile" => nav.tiles[0].data = vec![0; 128],
                        _ => nav.profile.tile_width = 16.,
                    };
                    asset.runtime_payload = Some(serde_json::to_vec(&nav).unwrap());
                }
            }
            let error = inspect_bm07_input(&bad).expect_err(case);
            rejected.push(json!({"case":case,"code":error.code,"message":error.message}));
        }
        fs::write(evidence.join("terminal.json"),serde_json::to_vec_pretty(&json!({"status":"passed","scope":"actual full BM07 Cook only, no runtime qualification","actual":actual,"rejected":rejected,"sourceDigest":source_digest(lease.snapshot())})).unwrap()).unwrap();
    }
}

mod s5d_bm06 {
    use project_authoring_execution::{
        platform_3d_benchmark::*, GameProjectCompiler, ProjectAuthoringSession, TargetProfile,
    };
    use serde_json::json;
    use std::{fs, path::PathBuf, time::Instant};
    #[test]
    fn full_bm06_request_requires_its_own_1200_tick_consumer() {
        let parent = std::env::temp_dir();
        let mut request = BenchmarkRequest {
            revision: "371-request-v1".into(),
            run_id: "bm06-request-contract".into(),
            output_parent: parent.clone(),
            project_root: parent,
            source_revision: format!("sha256:{}", "0".repeat(64)),
            target_recipe_sha256: engine_runtime::canonical_digest::sha256_prefixed(TARGET_RECIPES),
            workload: "BM-06".into(),
            stage_recipe: "sim-v1".into(),
            mode: "diagnostic".into(),
            operation: "run-scenario".into(),
            target_profile: "windows-release".into(),
            device_profile: "pc-rtx3070-v1".into(),
            backend: "wgpu-windowed".into(),
            evidence_level: "summary".into(),
            window: DiagnosticWindow {
                warmup_frames: 0,
                sample_frames: 1200,
                repeats: 1,
                minimum_wall_seconds: 0,
            },
            fault_schedule: vec![],
            timeout_ms: 600000,
        };
        request.validate().unwrap();
        for frames in [120, 720, 1199] {
            let mut bad = request.clone();
            bad.window.sample_frames = frames;
            assert!(bad.validate().is_err());
        }
        request.workload = "BM-10".into();
        assert!(request.validate().is_err());
        request.workload = "BM-06".into();
        request.operation = "cook".into();
        request.backend = "headless-cpu".into();
        request.window.sample_frames = 120;
        request.validate().unwrap();
    }
    #[test]
    #[ignore = "explicit original BM06 project; actual Compiler Cook and five retained-input rejection cases"]
    fn actual_full_bm06_cook_counts_real_bodies_shapes_ccd_joints_and_motors() {
        let project = PathBuf::from(std::env::var_os("AIFE_BM06_PROJECT").unwrap());
        let evidence = PathBuf::from(std::env::var_os("AIFE_BM06_EVIDENCE").unwrap());
        fs::create_dir(&evidence).unwrap();
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("bm06-full-cook-owner", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let started = Instant::now();
        let result = compiler.prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &evidence.parent().unwrap().join(".lit-cook-cache"),
        );
        if let Err(error) = &result {
            fs::write(evidence.join("first-failure.json"),serde_json::to_vec_pretty(&json!({"code":error.code(),"message":error.message(),"diagnostics":error.diagnostics()})).unwrap()).unwrap();
        }
        let prepared = result.unwrap();
        let actual = inspect_simulation_cook(&prepared, "BM-06").unwrap();
        fs::write(
            evidence.join("actual-cook.json"),
            serde_json::to_vec_pretty(&actual).unwrap(),
        )
        .unwrap();
        let input = prepared.runtime_package_build_input();
        let mut rejected = Vec::new();
        for case in [
            "missing-body",
            "shape-mix",
            "ccd-count",
            "joint-target",
            "disabled-motor",
            "unexpected-kinematic",
        ] {
            let mut bad = input.clone();
            if case == "missing-body" {
                bad.scenes[0].entities.retain(|e| e.id != "dynamic-0999");
            } else {
                let (entity, component) = match case {
                    "shape-mix" | "ccd-count" => ("dynamic-0000", "engine.body3d"),
                    "joint-target" => ("dynamic-0000", "engine.joint3d"),
                    "unexpected-kinematic" => ("static-0001", "engine.body3d"),
                    _ => ("motor-0", "engine.motor3d"),
                };
                let component = bad.scenes[0]
                    .entities
                    .iter_mut()
                    .find(|e| e.id == entity)
                    .unwrap()
                    .components
                    .iter_mut()
                    .find(|c| c.component_type == component)
                    .unwrap();
                match case {
                    "shape-mix" => component.data["shape"] = json!({"kind":"sphere","radius":0.4}),
                    "ccd-count" => component.data["ccd"] = json!(false),
                    "joint-target" => component.data["other"]["entityRef"] = json!("missing"),
                    "unexpected-kinematic" => component.data["motion"] = json!("kinematic"),
                    _ => component.data["enabled"] = json!(false),
                }
            }
            let error = inspect_bm06_input(&bad).expect_err(case);
            rejected.push(json!({"case":case,"code":error.code,"message":error.message}));
        }
        fs::write(evidence.join("terminal.json"),serde_json::to_vec_pretty(&json!({"status":"passed","scope":"original full BM06 actual Cook and six owner rejections; not runtime/performance","actual":actual,"rejected":rejected,"elapsedMs":started.elapsed().as_secs_f64()*1000.,"sourceDigest":source_digest(lease.snapshot())})).unwrap()).unwrap();
    }
}

mod s5d_bm05 {
    use project_authoring_execution::platform_3d_benchmark::*;
    use serde_json::{json, Value};

    #[test]
    fn full_recipe_request_accepts_real_windows_simulation_and_rejects_other_domains() {
        let parent = std::env::temp_dir();
        let mut request = BenchmarkRequest {
            revision: "371-request-v1".into(),
            run_id: "bm05-request-contract".into(),
            output_parent: parent.clone(),
            project_root: parent,
            source_revision: format!("sha256:{}", "0".repeat(64)),
            target_recipe_sha256: engine_runtime::canonical_digest::sha256_prefixed(TARGET_RECIPES),
            workload: "BM-05".into(),
            stage_recipe: "sim-v1".into(),
            mode: "diagnostic".into(),
            operation: "run-scenario".into(),
            target_profile: "windows-release".into(),
            device_profile: "pc-rtx3070-v1".into(),
            backend: "wgpu-windowed".into(),
            evidence_level: "summary".into(),
            window: DiagnosticWindow {
                warmup_frames: 0,
                sample_frames: 720,
                repeats: 1,
                minimum_wall_seconds: 0,
            },
            fault_schedule: vec![],
            timeout_ms: 600000,
        };
        request
            .validate()
            .expect("full BM05 sim-v1 has a real consumer");
        for field in ["frames", "android", "workload", "recipe", "backend"] {
            let mut bad = request.clone();
            match field {
                "frames" => bad.window.sample_frames = 719,
                "android" => bad.target_profile = "android-release".into(),
                "workload" => bad.workload = "BM-06".into(),
                "recipe" => bad.stage_recipe = "foundation-v1".into(),
                "backend" => bad.backend = "headless-cpu".into(),
                _ => unreachable!(),
            }
            assert!(
                bad.validate().is_err(),
                "unsupported {field} must not borrow BM05 evidence"
            );
        }
        request.operation = "cook".into();
        request.backend = "headless-cpu".into();
        request.window.sample_frames = 3;
        request.validate().unwrap();
    }

    #[test]
    #[ignore = "explicit full BM05 ordinary project; actual Compiler Cook once, mutate retained output for owner rejection checks"]
    fn actual_full_cook_requires_all_skins_four_weights_two_clips_and_static_payloads() {
        use project_authoring_execution::{
            GameProjectCompiler, ProjectAuthoringSession, TargetProfile,
        };
        use std::{fs, path::PathBuf, time::Instant};
        let project =
            PathBuf::from(std::env::var_os("AIFE_BM05_PROJECT").expect("explicit full project"));
        let evidence =
            PathBuf::from(std::env::var_os("AIFE_BM05_EVIDENCE").expect("fresh evidence"));
        fs::create_dir(&evidence).unwrap();
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("bm05-full-cook-owner", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let started = Instant::now();
        let prepared = compiler.prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &evidence.parent().unwrap().join(".lit-cook-cache"),
        );
        if let Err(error) = &prepared {
            fs::write(evidence.join("first-failure.json"),serde_json::to_vec_pretty(&json!({"code":error.code(),"message":error.message(),"diagnostics":error.diagnostics(),"elapsedMs":started.elapsed().as_secs_f64()*1000.})).unwrap()).unwrap();
        }
        let prepared = prepared.unwrap();
        let actual = inspect_simulation_cook(&prepared, "BM-05").unwrap();
        fs::write(
            evidence.join("actual-cook.json"),
            serde_json::to_vec_pretty(&actual).unwrap(),
        )
        .unwrap();
        fs::write(
            evidence.join("producers.json"),
            serde_json::to_vec_pretty(prepared.producer_reports()).unwrap(),
        )
        .unwrap();
        let input = prepared.runtime_package_build_input();
        let mut rejected = Vec::new();
        for case in [
            "missing-skin-target",
            "single-clip",
            "zero-weight-influences",
            "missing-static-mesh",
        ] {
            let mut bad = input.clone();
            match case {
                "missing-skin-target" => bad.scenes[0]
                    .entities
                    .retain(|e| e.id != "actor-099-joint-00"),
                "single-clip" => {
                    let actor = bad.scenes[0]
                        .entities
                        .iter_mut()
                        .find(|e| e.id == "actor-001")
                        .unwrap();
                    let character = actor
                        .components
                        .iter_mut()
                        .find(|c| c.component_type == "engine.character3d")
                        .unwrap();
                    character.data["graph"]["layers"] = json!([]);
                }
                "zero-weight-influences" => {
                    let rig = bad
                        .assets
                        .iter_mut()
                        .find(|a| a.asset_type == "character3d")
                        .unwrap();
                    let mut value: Value =
                        serde_json::from_slice(rig.runtime_payload.as_ref().unwrap()).unwrap();
                    value["skins"][0]["influences"][0]["weights"] = json!([1., 0., 0., 0.]);
                    rig.runtime_payload = Some(serde_json::to_vec(&value).unwrap());
                }
                "missing-static-mesh" => bad.assets.retain(|a| a.asset_id != "static.mesh.0"),
                _ => unreachable!(),
            }
            let failure = inspect_bm05_input(&bad).expect_err(case);
            rejected.push(json!({"case":case,"code":failure.code,"message":failure.message}));
        }
        fs::write(evidence.join("terminal.json"),serde_json::to_vec_pretty(&json!({"status":"passed","scope":"full frozen BM05 actual Cook and four owner rejections, not runtime/performance","actual":actual,"rejected":rejected,"elapsedMs":started.elapsed().as_secs_f64()*1000.,"sourceDigest":source_digest(lease.snapshot())})).unwrap()).unwrap();
    }
}
