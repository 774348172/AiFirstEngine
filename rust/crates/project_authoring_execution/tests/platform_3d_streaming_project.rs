//! Local Windows integration proof through the ordinary Compiler and exported Player.
use project_authoring_execution::{
    default_project_runtime_player_build_root, BuildRequest, ExecutionMode, GameProjectCompiler,
    ProjectAuthoringSession, ProjectRelativePath, TargetProfile, VerifyRequest,
};
use std::{fs, path::PathBuf};

fn assert_recorded_foundation_returns_to_baseline(child: &serde_json::Value) {
    let samples = child["framePerformanceSummary"]["rawCpuFrames"]
        .as_array()
        .unwrap();
    let resources: Vec<_> = samples
        .iter()
        .filter_map(|f| f.get("streamingResources"))
        .collect();
    let baseline = resources.first().expect("actual Trace resource samples")["cpuResidentBytes"]
        .as_u64()
        .unwrap();
    // The initially hydrated camera-only scene remains cached. Cells must return
    // to that measured baseline, rather than pretending the whole loader is empty.
    let idle_fields = [
        "activeCells",
        "liveCells",
        "cpuReservedBytes",
        "pendingAssetOperations",
        "pendingAssetTickets",
        "gpuMeshBytes",
        "gpuTextureBytes",
        "gpuStagingBytes",
        "gpuRetiringBytes",
        "pendingGpuTickets",
        "gpuMeshes",
        "gpuTextures",
    ];
    let idle = |r: &serde_json::Value| {
        r["cpuResidentBytes"].as_u64() == Some(baseline)
            && idle_fields.iter().all(|k| r[*k].as_u64() == Some(0))
    };
    assert!(
        idle(resources[0]),
        "first sample must establish the empty cell baseline"
    );
    let mut loaded = false;
    let mut closed = 0;
    for r in resources {
        if r["activeCells"] == 2 && r["gpuTextureBytes"].as_u64().unwrap() >= 64 * 1024 * 1024 {
            loaded = true;
        }
        if loaded && idle(r) {
            closed += 1;
            loaded = false;
        }
    }
    assert!(
        closed >= 21,
        "initial release plus twenty complete load/release intervals required, got {closed}"
    );
}

#[test]
#[ignore = "consumes a retained real Player Trace, without relaunching the window"]
fn foundation_recorded_trace_returns_every_tracked_domain_to_baseline() {
    let evidence = PathBuf::from(std::env::var_os("AIFE_CELL_PLAYER_EVIDENCE").unwrap());
    let child =
        serde_json::from_slice(&fs::read(evidence.join("measured-child.json")).unwrap()).unwrap();
    assert_recorded_foundation_returns_to_baseline(&child);
}

#[test]
#[ignore = "requires the generated run-owned foundation source and an evidence directory"]
fn foundation_incremental_cook_rounds_keep_real_producer_and_output_evidence() {
    use project_authoring_execution::ProjectAssemblyArtifactCacheStatus as Status;
    let project = std::env::var_os("AIFE_CELL_PLAYER_PROJECT")
        .map(PathBuf::from)
        .unwrap();
    let evidence = std::env::var_os("AIFE_CELL_PLAYER_EVIDENCE")
        .map(PathBuf::from)
        .unwrap()
        .join("cook-revisions");
    fs::create_dir_all(&evidence).unwrap();
    let copied = evidence.join("source");
    assert!(
        !copied.exists(),
        "use a fresh run root; preserve first evidence"
    );
    let mut source_session = ProjectAuthoringSession::open(&project).unwrap();
    let paths = source_session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let source_lease = source_session
        .acquire_snapshot_lease("371-cook-input-copy", paths)
        .unwrap();
    for file in &source_lease.snapshot().files {
        let path = copied.join(&file.relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &file.bytes).unwrap();
    }
    drop(source_lease);
    drop(source_session);
    let cache = evidence.join("cache");
    let mut previous = std::collections::BTreeMap::new();
    let mut previous_digest = String::new();
    for (round, change) in [
        ("cold", None),
        ("unchanged", None),
        (
            "material",
            Some(("material-a-0", "Assets/material-a-0.asset")),
        ),
        ("mesh", Some(("cube-a", "Assets/cube-a.asset"))),
        (
            "shared-dependency",
            Some(("cube-shared", "Assets/cube-shared.asset")),
        ),
    ] {
        if let Some((asset_id, relative)) = change {
            let path = copied.join(relative);
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            if asset_id.starts_with("material") {
                value["baseColor"][0] = serde_json::json!(0.85);
            } else {
                value["source"]["geometry"]["size"][0] = serde_json::json!(1.73);
            }
            fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        }
        let mut session = ProjectAuthoringSession::open(&copied).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease(format!("371-cook-{round}"), paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let started = std::time::Instant::now();
        let prepared = compiler
            .prepare_with_artifact_cache(&lease, TargetProfile::WindowsDev, &cache)
            .unwrap();
        let wall_ms = started.elapsed().as_secs_f64() * 1000.;
        let cells =
            project_authoring_execution::platform_3d_benchmark::inspect_foundation_cells(&prepared)
                .unwrap();
        let assets: std::collections::BTreeMap<_, _> = prepared
            .runtime_package_build_input()
            .assets
            .iter()
            .map(|a| (a.asset_id.clone(), a.hash.clone()))
            .collect();
        let changed: Vec<_> = assets
            .iter()
            .filter(|(id, hash)| previous.get(*id) != Some(*hash))
            .map(|(id, _)| id.clone())
            .collect();
        let producers: Vec<_> = prepared
            .producer_reports()
            .iter()
            .filter(|r| matches!(r.producer_id.as_str(), "render-asset-cook" | "texture-cook"))
            .collect();
        let outputs: Vec<_> = prepared.runtime_package_build_input().assets.iter().map(|a| serde_json::json!({
            "assetId":a.asset_id,"source":a.source,"dependencies":a.dependencies,"digest":a.hash,
            "runtimePayloadBytes":a.runtime_payload.as_ref().map_or(0, Vec::len)
        })).collect();
        fs::write(evidence.join(format!("{round}.json")), serde_json::to_vec_pretty(&serde_json::json!({
            "scope":"BM-08 double-cell resources incremental diagnostic, not BM-14 full static qualification",
            "round":round,"sourceDigest":project_authoring_execution::platform_3d_benchmark::source_digest(lease.snapshot()),
            "target":"windows-dev","cache":"fresh only for cold; retained for subsequent rounds; OS IO cache uncontrolled",
            "wallMs":wall_ms,"assemblyDigest":prepared.assembly_digest(),"changedAssets":changed,
            "producerCalls":producers,"outputs":outputs,"cells":cells,
            "textureOutputs":prepared.runtime_package_build_input().texture_payloads.iter().map(|t| serde_json::json!({
                "assetId":t.metadata.asset_id,"decodedBytes":t.rgba8.len(),
                "pixelDigest":engine_runtime::canonical_digest::sha256_prefixed(&t.rgba8)
            })).collect::<Vec<_>>(),
            "decodedTextureBytes":prepared.runtime_package_build_input().texture_payloads.iter().map(|t| t.rgba8.len()).sum::<usize>()
        })).unwrap()).unwrap();
        match round {
            "cold" => assert!(producers.iter().all(|r| r.cache_status == Status::Produced)),
            "unchanged" => {
                assert!(producers.iter().all(|r| r.cache_status == Status::Hit));
                assert!(changed.is_empty());
                assert_eq!(prepared.assembly_digest(), previous_digest);
            }
            _ => {
                assert_eq!(changed, vec![change.unwrap().0.to_string()]);
                assert_eq!(
                    producers
                        .iter()
                        .filter(|r| r.cache_status == Status::Produced)
                        .count(),
                    1
                );
                assert_eq!(
                    producers
                        .iter()
                        .filter(|r| r.cache_status == Status::Hit)
                        .count(),
                    producers.len() - 1
                );
                assert_ne!(prepared.assembly_digest(), previous_digest);
            }
        }
        previous = assets;
        previous_digest = prepared.assembly_digest().to_string();
    }
}

#[test]
#[ignore = "builds an ordinary project DLL and launches the real Windows Player"]
fn sdk_cells_complete_in_exported_windows_player() {
    assert!(cfg!(windows));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = std::env::var_os("AIFE_CELL_PLAYER_PROJECT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("samples/platform_3d_streaming_project"));
    let evidence = std::env::var_os("AIFE_CELL_PLAYER_EVIDENCE")
        .map(PathBuf::from)
        .expect("set a run-owned evidence directory");
    fs::create_dir_all(&evidence).unwrap();
    let mut session = ProjectAuthoringSession::open(&project).unwrap();
    let frame_limit = if project.join("Benchmark/foundation.json").exists() {
        7200
    } else {
        3600
    };
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("cell-player-integration", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    if project.join("Benchmark/foundation.json").exists() {
        let cells =
            project_authoring_execution::platform_3d_benchmark::inspect_foundation_cells(&prepared)
                .expect(
                    "actual enabled prefab closure must meet each cell's unique 32 MiB requirement",
                );
        fs::write(
            evidence.join("foundation-cook.json"),
            serde_json::to_vec_pretty(&cells).unwrap(),
        )
        .unwrap();
        assert_eq!(cells.len(), 2);
        assert!(cells
            .iter()
            .all(|c| c.render_instances == 2 && c.base_color_bytes == 32 * 1024 * 1024));
    }
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsDev,
                &project,
                ProjectRelativePath::parse("Build/CellPlayerIntegration").unwrap(),
                default_project_runtime_player_build_root(),
            )
            .with_frame_limit(frame_limit)
            .with_timeout_ms(600_000)
            .with_player_verification(false),
        )
        .unwrap();
    fs::write(
        evidence.join("build-report.json"),
        serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
    )
    .unwrap();
    let verified = compiler
        .verify(
            built.delivery(),
            VerifyRequest::new(ExecutionMode::Windowed, frame_limit, 600_000, false),
        )
        .unwrap();
    let report = verified.process_verification();
    fs::write(
        evidence.join("verification-report.json"),
        serde_json::to_vec_pretty(report).unwrap(),
    )
    .unwrap();
    assert_eq!(report.process_exit_code, Some(0), "{report:#?}");
    assert_eq!(report.child_player_exit_code, Some(0), "{report:#?}");
    let child: serde_json::Value =
        serde_json::from_slice(&fs::read(&report.child_report_path).unwrap()).unwrap();
    fs::write(
        evidence.join("child-report.json"),
        serde_json::to_vec_pretty(&child).unwrap(),
    )
    .unwrap();
    assert_eq!(child["exitReason"], "session_exit_requested", "{child:#?}");
    // The default screenshot targets the frame limit; orderly session exit precedes
    // that limit. A separate bounded run consumes the same delivery during its
    // final display period. The generated fixture keeps 600 ticks so independent
    // worker timing can vary around this midpoint. It proves pixels separately.
    let capture_frame = report
        .child_frames_completed
        .unwrap()
        .saturating_sub(if project.join("Benchmark/foundation.json").exists() {
            300
        } else {
            10
        })
        .max(1);
    let captured = compiler
        .verify(
            built.delivery(),
            VerifyRequest::new(ExecutionMode::Windowed, capture_frame, 240_000, true),
        )
        .unwrap();
    let capture = captured.process_verification();
    fs::write(
        evidence.join("capture-verification-report.json"),
        serde_json::to_vec_pretty(capture).unwrap(),
    )
    .unwrap();
    assert_eq!(
        capture.screenshot_status.as_deref(),
        Some("captured"),
        "{capture:#?}"
    );
    let screenshot = capture.screenshot_path.as_ref().expect("real capture path");
    let pixels = image::open(screenshot).unwrap().to_rgb8();
    let mut red = 0usize;
    let mut blue = 0usize;
    for (x, _, pixel) in pixels.enumerate_pixels() {
        let [r, g, b] = pixel.0.map(u16::from);
        if x < pixels.width() / 2 && r > 80 && r > g * 2 && r > b * 2 {
            red += 1;
        }
        if x > pixels.width() / 2 && b > 80 && b > r * 2 && b > g * 2 {
            blue += 1;
        }
    }
    fs::copy(screenshot, evidence.join("final-cells.png")).unwrap();
    fs::write(
        evidence.join("pixel-proof.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"leftRed":red,"rightBlue":blue,
            "width":pixels.width(),"height":pixels.height()}))
        .unwrap(),
    )
    .unwrap();
    assert!(
        red > 100 && blue > 100,
        "both dynamically loaded cells must render: red={red}, blue={blue}"
    );
    if project.join("Benchmark/foundation.json").exists() {
        let measured = runtime_cli::verify_exported_player_process_with_options(
            runtime_cli::ExportedPlayerProcessVerificationRequest {
                exported_package_dir: built.delivery().package_dir().to_path_buf(),
                mode: "windowed".into(),
                frame_limit,
                report_path: Some(evidence.join("measured-verification.json")),
                timeout_ms: 240_000,
                screenshot: false,
                screenshot_path: None,
            },
            runtime_cli::ExportedPlayerProcessVerificationOptions {
                measurement: Some(runtime_player_winit::measurement::MeasurementPolicy {
                    level: runtime_player_winit::measurement::MeasurementLevel::Trace,
                    native_events: false,
                    allocation_frame: None,
                    allocation_end_frame: None,
                    ..Default::default()
                }),
                // Measure the production gameplay report policy independently
                // from Trace-only resource/frame diagnostics.
                runtime_report_level: Some("off".into()),
                performance_warmup_frames: 0,
                performance_sample_frames: frame_limit,
                ..Default::default()
            },
        );
        fs::write(
            evidence.join("measured-verification.json"),
            serde_json::to_vec_pretty(&measured).unwrap(),
        )
        .unwrap();
        assert_eq!(measured.process_exit_code, Some(0), "{measured:#?}");
        assert_eq!(measured.child_player_exit_code, Some(0), "{measured:#?}");
        let child = fs::read(&measured.child_report_path).unwrap();
        fs::write(evidence.join("measured-child.json"), &child).unwrap();
        let child: serde_json::Value = serde_json::from_slice(&child).unwrap();
        assert_eq!(child["exitReason"], "session_exit_requested", "{child:#?}");
        let samples = child["framePerformanceSummary"]["rawCpuFrames"]
            .as_array()
            .expect("Trace frame samples");
        let resources: Vec<_> = samples
            .iter()
            .filter_map(|s| s.get("streamingResources"))
            .collect();
        assert!(
            !resources.is_empty(),
            "real streaming domains were not sampled"
        );
        assert!(
            resources.iter().any(|r| r["activeCells"] == 2
                && r["gpuTextureBytes"].as_u64().unwrap_or(0) >= 64 * 1024 * 1024),
            "two active cells must retain their real GPU textures"
        );
        assert_recorded_foundation_returns_to_baseline(&child);
    }
}
