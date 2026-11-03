//! Local Release lifecycle consumer, separate from full static Cook and scan-out.
use project_authoring_execution::{
    default_project_runtime_player_build_root, BuildRequest, GameProjectCompiler,
    ProjectAuthoringSession, ProjectRelativePath, TargetProfile,
};
use std::{fs, path::PathBuf};

fn static_window_labels(diagnostic: bool) -> &'static [&'static str] {
    if diagnostic {
        &["probe"]
    } else {
        &["probe", "steady-0", "steady-1", "steady-2"]
    }
}

fn static_cost_window_labels() -> &'static [&'static str] {
    &[
        "probe",
        "off-0",
        "summary-0",
        "off-1",
        "summary-1",
        "off-2",
        "summary-2",
        "filtered-trace",
    ]
}
fn static_measurement_policy(
    label: &str,
    cost: bool,
) -> runtime_player_winit::measurement::MeasurementPolicy {
    use runtime_player_winit::measurement::{MeasurementLevel, MeasurementPolicy};
    MeasurementPolicy {
        level: if label.starts_with("off-") {
            MeasurementLevel::Off
        } else if label == "probe" || label == "filtered-trace" {
            MeasurementLevel::Trace
        } else {
            MeasurementLevel::Summary
        },
        cpu_probe: cost && label != "probe" && label != "filtered-trace",
        native_events: false,
        allocation_frame: None,
        allocation_end_frame: None,
        allocation_summary: false,
        first_frame: (label == "filtered-trace").then_some(601),
        end_frame: (label == "filtered-trace").then_some(901),
    }
}
#[test]
fn static_cost_compares_measurement_policies_with_same_probe_and_filtered_trace() {
    let labels = static_cost_window_labels();
    assert_eq!(labels.iter().filter(|l| l.starts_with("off-")).count(), 3);
    assert_eq!(
        labels.iter().filter(|l| l.starts_with("summary-")).count(),
        3
    );
    let off = static_measurement_policy("off-0", true);
    let summary = static_measurement_policy("summary-0", true);
    assert!(!off.enabled());
    assert!(summary.enabled());
    assert!(off.cpu_probe && summary.cpu_probe);
    assert!(!off.native_events && !summary.native_events);
    let trace = static_measurement_policy("filtered-trace", true);
    assert!(trace.trace_frame(601));
    assert!(trace.trace_frame(900));
    assert!(!trace.trace_frame(901));
    assert!(!static_measurement_policy("steady-0", false).cpu_probe);
}

fn validate_static_reuse_package(
    package: &std::path::Path,
    expected_manifest_digest: &str,
) -> Result<serde_json::Value, String> {
    let manifest_path = package.join("package-manifest.json");
    if runtime_cli::semantic_file_digest(&manifest_path)? != expected_manifest_digest {
        return Err("static.reuse_manifest_changed".into());
    }
    let bytes = fs::read(&manifest_path).map_err(|e| e.to_string())?;
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if manifest["schemaVersion"] != "desktop-package-manifest.v1"
        || manifest["target"] != "windows"
        || manifest["profile"] != "release"
    {
        return Err("static.reuse_requires_windows_release".into());
    }
    let module = manifest["playerModuleDescriptor"]["moduleId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("static.reuse_missing_module")?;
    let stem: String = module
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    for (file, key) in [
        ("Game.exe".to_string(), "playerArtifactHash"),
        ("engine_runtime.dll".into(), "engineRuntimeHash"),
        (format!("data/bin/{stem}.dll"), "projectRuntimeModuleHash"),
    ] {
        if manifest[key].as_str()
            != Some(runtime_cli::semantic_file_digest(&package.join(file))?.as_str())
        {
            return Err(format!("static.reuse_changed_{key}"));
        }
    }
    if manifest["runtimePackageDigest"].as_str()
        != Some(
            runtime_cli::runtime_package_digest(&package.join("data/runtime_package"))?.as_str(),
        )
    {
        return Err("static.reuse_changed_runtime_package".into());
    }
    Ok(manifest)
}

#[test]
fn static_reuse_rejects_changed_identity_and_each_consumed_payload() {
    let root = std::env::temp_dir().join(format!(
        "371-reuse-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("data/bin")).unwrap();
    fs::create_dir_all(root.join("data/runtime_package")).unwrap();
    for file in [
        "Game.exe",
        "engine_runtime.dll",
        "data/bin/test_module.dll",
        "data/runtime_package/input.json",
    ] {
        fs::write(root.join(file), b"original").unwrap();
    }
    let digest = |p: &str| runtime_cli::semantic_file_digest(&root.join(p)).unwrap();
    let manifest = serde_json::json!({
        "schemaVersion":"desktop-package-manifest.v1", "target":"windows", "profile":"release",
        "playerModuleDescriptor":{"moduleId":"test.module"},
        "playerArtifactHash":digest("Game.exe"), "engineRuntimeHash":digest("engine_runtime.dll"),
        "projectRuntimeModuleHash":digest("data/bin/test_module.dll"),
        "runtimePackageDigest":runtime_cli::runtime_package_digest(&root.join("data/runtime_package")).unwrap()
    });
    fs::write(
        root.join("package-manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let expected = digest("package-manifest.json");
    validate_static_reuse_package(&root, &expected).unwrap();
    assert!(validate_static_reuse_package(&root, "sha256:wrong").is_err());
    for file in [
        "Game.exe",
        "engine_runtime.dll",
        "data/bin/test_module.dll",
        "data/runtime_package/input.json",
    ] {
        fs::write(root.join(file), b"changed").unwrap();
        assert!(
            validate_static_reuse_package(&root, &expected).is_err(),
            "reject changed {file}"
        );
        fs::write(root.join(file), b"original").unwrap();
    }
    let mut dev = manifest;
    dev["profile"] = "dev".into();
    fs::write(
        root.join("package-manifest.json"),
        serde_json::to_vec(&dev).unwrap(),
    )
    .unwrap();
    assert!(validate_static_reuse_package(&root, &digest("package-manifest.json")).is_err());
    for file in [
        "package-manifest.json",
        "Game.exe",
        "engine_runtime.dll",
        "data/bin/test_module.dll",
        "data/runtime_package/input.json",
    ] {
        fs::remove_file(root.join(file)).unwrap();
    }
    for dir in ["data/bin", "data/runtime_package", "data", ""] {
        fs::remove_dir(root.join(dir)).unwrap();
    }
}

#[test]
fn static_diagnostic_runs_only_one_probe_and_never_formal_windows() {
    assert_eq!(static_window_labels(true), &["probe"]);
    assert_eq!(
        static_window_labels(false),
        &["probe", "steady-0", "steady-1", "steady-2"]
    );
}

// Capture replays the same plan offscreen before its final surface submission.
// Count only explicitly identified submissions; never accept an arbitrary doubled draw.
fn static_instance_counts(summary: &serde_json::Value, warmup: usize) -> Result<(), String> {
    let frames = summary["rawCpuFrames"]
        .as_array()
        .ok_or("CPU frames missing")?;
    let submissions = summary["gpu"]["submissions"]
        .as_array()
        .ok_or("submissions missing")?;
    if summary["gpu"]["submissionObservationsOmitted"] != 0 {
        return Err("incomplete submission observations".into());
    }
    let terminal = frames.last().ok_or("empty CPU frames")?["frameIndex"].clone();
    for pair in frames.windows(2).skip(warmup) {
        let current = &pair[1];
        let matching = submissions
            .iter()
            .filter(|s| s["token"]["frameIndex"] == current["frameIndex"])
            .collect::<Vec<_>>();
        let presented = matching
            .iter()
            .filter(|s| {
                s["surfacePresentCalls"] == 1
                    && s["offscreenCapture"] == false
                    && s["captureCount"] == 0
            })
            .count();
        let captures = matching
            .iter()
            .filter(|s| {
                s["surfacePresentCalls"] == 0
                    && s["offscreenCapture"] == true
                    && s["captureCount"] == 1
            })
            .count();
        if presented != 1
            || matching.len() != presented + captures
            || captures != usize::from(current["frameIndex"] == terminal)
            || matching.iter().any(|s| {
                s["frameIdentity"] != current["frameIdentity"] || s["status"] != "completed"
            })
        {
            return Err(format!(
                "invalid present/capture identity at {}",
                current["frameIndex"]
            ));
        }
        let previous = pair[0]["spatialGpu"]["cumulativeInstances"]
            .as_u64()
            .ok_or("previous instances missing")?;
        let now = current["spatialGpu"]["cumulativeInstances"]
            .as_u64()
            .ok_or("instances missing")?;
        if now.checked_sub(previous) != Some(10000 * matching.len() as u64) {
            return Err(format!(
                "K0 real backend instance count at {}",
                current["frameIndex"]
            ));
        }
    }
    Ok(())
}

#[test]
fn static_instance_counts_require_identified_capture_and_reject_duplicate_surface() {
    use serde_json::json;
    let mut summary = json!({"rawCpuFrames":[
        {"frameIndex":1,"spatialGpu":{"cumulativeInstances":10000}},
        {"frameIndex":2,"frameIdentity":{"frameIndex":2},"spatialGpu":{"cumulativeInstances":30000}}
    ],"gpu":{"submissionObservationsOmitted":0,"submissions":[
        {"token":{"frameIndex":2},"frameIdentity":{"frameIndex":2},"status":"completed","surfacePresentCalls":1,"offscreenCapture":false,"captureCount":0},
        {"token":{"frameIndex":2},"frameIdentity":{"frameIndex":2},"status":"completed","surfacePresentCalls":0,"offscreenCapture":true,"captureCount":1}
    ]}});
    assert!(static_instance_counts(&summary, 0).is_ok());
    summary["gpu"]["submissions"][1]["surfacePresentCalls"] = json!(1);
    assert!(static_instance_counts(&summary, 0).is_err());
    summary["gpu"]["submissions"][1]["surfacePresentCalls"] = json!(0);
    summary["gpu"]["submissions"][1]["frameIdentity"]["frameIndex"] = json!(3);
    assert!(static_instance_counts(&summary, 0).is_err());
    summary["gpu"]["submissions"][1]["frameIdentity"]["frameIndex"] = json!(2);
    summary["rawCpuFrames"][1]["spatialGpu"]["cumulativeInstances"] = json!(20000);
    assert!(static_instance_counts(&summary, 0).is_err());
    summary["rawCpuFrames"][1]["spatialGpu"]["cumulativeInstances"] = json!(30000);
    summary["gpu"]["submissionObservationsOmitted"] = json!(1);
    assert!(static_instance_counts(&summary, 0).is_err());
}

#[test]
#[ignore = "replays a retained Trace probe without building or launching a player"]
fn static_instance_counts_replay_retained_probe() {
    let path = std::env::var_os("AIFE_STATIC_RETAINED_SUMMARY").unwrap();
    let summary = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    static_instance_counts(&summary, 120).unwrap();
}

#[test]
#[ignore = "three isolated ordinary Release double-cell event windows with real payloads"]
fn windows_release_cell_event_budgets() {
    use runtime_player_winit::measurement::{MeasurementLevel, MeasurementPolicy};
    let evidence = PathBuf::from(std::env::var_os("AIFE_CELL_BUDGET_EVIDENCE").unwrap());
    assert!(!evidence.exists());
    fs::create_dir_all(&evidence).unwrap();
    let reuse = std::env::var_os("AIFE_CELL_BUDGET_REUSE_PACKAGE").map(PathBuf::from);
    let expected = std::env::var("AIFE_CELL_BUDGET_REUSE_MANIFEST_DIGEST").ok();
    assert_eq!(
        reuse.is_some(),
        expected.is_some(),
        "reuse requires explicit package and expected manifest identity"
    );
    let package = if let Some(package) = &reuse {
        let identity =
            validate_static_reuse_package(package, expected.as_deref().unwrap()).unwrap();
        fs::write(
            evidence.join("reuse.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "manifest":identity,"manifestDigest":expected,"compilerInvoked":false,
                "scope":"this immutable Release package; no implicit current-source qualification"
            }))
            .unwrap(),
        )
        .unwrap();
        package.clone()
    } else {
        let project = PathBuf::from(std::env::var_os("AIFE_CELL_BUDGET_PROJECT").unwrap());
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("371-cell-budget", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let prepared = compiler
            .prepare(&lease, TargetProfile::WindowsRelease)
            .unwrap();
        let cells =
            project_authoring_execution::platform_3d_benchmark::inspect_foundation_cells(&prepared)
                .unwrap();
        fs::write(
            evidence.join("cook.json"),
            serde_json::to_vec_pretty(&cells).unwrap(),
        )
        .unwrap();
        let built = compiler
            .build(
                &prepared,
                BuildRequest::for_project(
                    TargetProfile::WindowsRelease,
                    &project,
                    ProjectRelativePath::parse(format!("Build/EventBudget-{}", std::process::id()))
                        .unwrap(),
                    default_project_runtime_player_build_root(),
                )
                .with_timeout_ms(1200000)
                .with_frame_limit(3000)
                .with_player_verification(false),
            )
            .unwrap();
        fs::write(
            evidence.join("build.json"),
            serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
        )
        .unwrap();
        let package = built.delivery().package_dir().to_path_buf();
        drop(built);
        drop(prepared);
        drop(compiler);
        drop(lease);
        drop(session);
        package
    };
    for repeat in 0..3 {
        let directory = evidence.join(format!("events-{repeat}"));
        fs::create_dir_all(&directory).unwrap();
        let report = runtime_cli::verify_exported_player_process_with_options(
            runtime_cli::ExportedPlayerProcessVerificationRequest {
                exported_package_dir: package.clone(),
                mode: "windowed".into(),
                frame_limit: 3000,
                report_path: Some(directory.join("verification.json")),
                timeout_ms: 180000,
                screenshot: false,
                screenshot_path: None,
            },
            runtime_cli::ExportedPlayerProcessVerificationOptions {
                runtime_report_level: Some("off".into()),
                measurement: Some(MeasurementPolicy {
                    level: MeasurementLevel::Trace,
                    native_events: false,
                    allocation_frame: None,
                    allocation_end_frame: None,
                    ..Default::default()
                }),
                performance_warmup_frames: 0,
                performance_sample_frames: 3000,
                ..Default::default()
            },
        );
        fs::write(
            directory.join("verification.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(report.process_exit_code, Some(0), "{report:#?}");
        let child: serde_json::Value =
            serde_json::from_slice(&fs::read(&report.child_report_path).unwrap()).unwrap();
        let frames = child["framePerformanceSummary"]["rawCpuFrames"]
            .as_array()
            .unwrap();
        let mut requests = std::collections::BTreeMap::<u64, (u64, u64, Option<u64>, bool)>::new();
        let mut previous_wall = 0;
        let mut peaks = std::collections::BTreeMap::<String, u64>::new();
        for frame in frames {
            let wall = frame["samplingWallNs"].as_u64().unwrap();
            if let Some(receipts) = frame["cellReceipts"].as_array() {
                for receipt in receipts {
                    let key = receipt["requestId"].as_u64().unwrap();
                    let request = requests
                        .entry(key)
                        .or_insert((previous_wall, wall, None, false));
                    if receipt["status"] == "active" && request.2.is_none() {
                        request.2 = Some(wall);
                    }
                    if receipt["status"] == "released" {
                        request.3 = true;
                    }
                    assert_ne!(receipt["status"], "failed", "{receipt}");
                }
            }
            for (field, limit) in [
                ("cpuResidentBytes", 2 * 1024 * 1024 * 1024),
                ("cpuReservedBytes", 256 * 1024 * 1024),
                ("gpuMeshBytes", 2 * 1024 * 1024 * 1024),
                ("gpuTextureBytes", 2 * 1024 * 1024 * 1024),
                ("gpuStagingBytes", 128 * 1024 * 1024),
                ("gpuRetiringBytes", 256 * 1024 * 1024),
                ("pendingAssetOperations", 256),
                ("pendingAssetTickets", 256),
            ] {
                let value = frame["streamingResources"][field].as_u64().unwrap();
                peaks
                    .entry(field.into())
                    .and_modify(|p| *p = (*p).max(value))
                    .or_insert(value);
                assert!(value <= limit, "{field}: {value}>{limit}");
            }
            let upload = frame["spatialGpu"]["frameUploadedBytes"].as_u64().unwrap();
            peaks
                .entry("frameUploadedBytes".into())
                .and_modify(|p| *p = (*p).max(upload))
                .or_insert(upload);
            assert!(upload <= 16 * 1024 * 1024);
            previous_wall = wall;
        }
        let latencies:Vec<_>=requests.iter().filter_map(|(key,(lower,upper,active,released))|active.map(|active| {
            serde_json::json!({"requestId":key,"acceptedWallLowerNs":lower,"acceptedWallUpperNs":upper,
                "activeObservedWallNs":active,"latencyLowerNs":active-upper,"latencyUpperNs":active-lower,"released":released})
        })).collect();
        // Sampling starts after session initialization. Include all unobserved
        // spawn/init/exit/report time in a conservative startup upper bound.
        let unobserved_ns =
            (report.process_elapsed_ms as u64 * 1000000).saturating_sub(previous_wall);
        let initial_active = latencies
            .iter()
            .filter(|v| v["requestId"].as_u64().is_some_and(|k| k == 1 || k == 2))
            .map(|v| v["activeObservedWallNs"].as_u64().unwrap())
            .max()
            .unwrap();
        let startup_upper_ns = unobserved_ns + initial_active;
        fs::write(directory.join("events.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "scope":"fresh process CPU/GPU caches; OS file cache unknown; frame-resolution acceptance-to-Active bounds",
            "latencies":latencies,"peaks":peaks,"startupUpperNs":startup_upper_ns,
            "summary":child["framePerformanceSummary"]["cpuExcludingPresentWait"],
            "samplingWindow":child["framePerformanceSummary"]["samplingWindow"],
            "display":"API/owner evidence; no OS scan-out qualification; Trace overhead included"
        })).unwrap()).unwrap();
        assert!(
            latencies.len() >= 44,
            "full initial/reload/migration/20 paired rounds/final activation: {}",
            latencies.len()
        );
        let upper: Vec<f64> = latencies
            .iter()
            .map(|v| v["latencyUpperNs"].as_u64().unwrap() as f64 / 1000000.)
            .collect();
        let distribution =
            project_authoring_execution::platform_3d_benchmark::distribution(&upper, 2000.)
                .unwrap();
        assert!(
            distribution.p95 <= 2000.,
            "acceptance-to-Active upper bound {distribution:?}"
        );
        assert!(
            startup_upper_ns <= 10000000000,
            "spawn-to-initial-both-Active conservative upper bound {startup_upper_ns}"
        );
        assert!(
            child["framePerformanceSummary"]["samplingWindow"]["peakPrivateCommitBytes"]
                .as_u64()
                .unwrap()
                <= 6 * 1024 * 1024 * 1024
        );
        if let Some(expected) = &expected {
            validate_static_reuse_package(&package, expected).unwrap();
        }
        eprintln!("cell events {repeat}: {distribution:?}");
    }
}

fn assert_static_frame_time_budget(cpu: &serde_json::Value, gpu: &serde_json::Value, probe: bool) {
    // A §6.1/6.3: the short Trace probe diagnoses cost and verifies the
    // composition. Formal budgets apply to each independent Summary window.
    if probe {
        return;
    }
    assert!(
        cpu["p95Ms"].as_f64().unwrap() <= 13.3 && cpu["p99Ms"].as_f64().unwrap() <= 16.67,
        "CPU {cpu}"
    );
    assert!(
        gpu["p95Ms"].as_f64().unwrap() <= 13.3 && gpu["p99Ms"].as_f64().unwrap() <= 16.67,
        "GPU {gpu}"
    );
}

#[test]
fn trace_probe_cost_does_not_gate_formal_summary_but_summary_budgets_stay_strict() {
    let valid = serde_json::json!({"p95Ms":13.3,"p99Ms":16.67});
    let over = serde_json::json!({"p95Ms":30.0,"p99Ms":40.0});
    assert_static_frame_time_budget(&over, &over, true);
    assert_static_frame_time_budget(&valid, &valid, false);
    assert!(
        std::panic::catch_unwind(|| assert_static_frame_time_budget(&over, &valid, false)).is_err()
    );
    assert!(
        std::panic::catch_unwind(|| assert_static_frame_time_budget(&valid, &over, false)).is_err()
    );
}

#[test]
#[ignore = "full BM02/BM03 ordinary Release build, fail-fast probe then three independent K0 windows"]
fn windows_release_full_static_runtime_windows() {
    let evidence = PathBuf::from(std::env::var_os("AIFE_STATIC_RUNTIME_EVIDENCE").unwrap());
    let recipe = std::env::var("AIFE_STATIC_RUNTIME_RECIPE").unwrap();
    assert!(matches!(recipe.as_str(), "BM-02" | "BM-03"));
    let diagnostic = match std::env::var("AIFE_STATIC_RUNTIME_DIAGNOSTIC_ONLY")
        .ok()
        .as_deref()
    {
        None | Some("0") => false,
        Some("1") => true,
        _ => panic!("diagnostic mode must be 0 or 1"),
    };
    assert!(!evidence.exists(), "fresh attempt required");
    fs::create_dir_all(&evidence).unwrap();
    let preparation_started = std::time::Instant::now();
    let prepare_only = std::env::var("AIFE_STATIC_RUNTIME_PREPARE_ONLY")
        .ok()
        .as_deref()
        == Some("1");
    let mut preparation_stages = serde_json::Map::new();
    let cost_only = match std::env::var("AIFE_STATIC_RUNTIME_COST_ONLY")
        .unwrap_or_else(|_| "0".into())
        .as_str()
    {
        "0" => false,
        "1" => true,
        _ => panic!("cost-only must be 0 or 1"),
    };
    assert!(
        !(cost_only && diagnostic),
        "cost qualification cannot be diagnostic-only"
    );
    let reuse_package = std::env::var_os("AIFE_STATIC_RUNTIME_REUSE_PACKAGE").map(PathBuf::from);
    let reuse_digest = std::env::var("AIFE_STATIC_RUNTIME_REUSE_MANIFEST_DIGEST").ok();
    assert_eq!(
        reuse_package.is_some(),
        reuse_digest.is_some(),
        "reuse requires both explicit package and expected identity"
    );
    let mut costs = Vec::new();
    let (meshes, textures, texture_bytes) = if recipe == "BM-02" {
        (100, 8, 134217728u64)
    } else {
        (10000, 64, 1073741824u64)
    };
    let package = if let Some(package) = &reuse_package {
        let identity =
            validate_static_reuse_package(package, reuse_digest.as_deref().unwrap()).unwrap();
        fs::write(evidence.join("reuse.json"), serde_json::to_vec_pretty(&serde_json::json!({"manifest":identity,"manifestDigest":reuse_digest,"compilerInvoked":false,"scope":"existing package identity; does not qualify current source","diagnosticOnly":diagnostic})).unwrap()).unwrap();
        package.clone()
    } else {
        let project = PathBuf::from(std::env::var_os("AIFE_STATIC_RUNTIME_PROJECT").unwrap());
        let mut stage_started = std::time::Instant::now();
        let mut session = ProjectAuthoringSession::open(&project).unwrap();
        preparation_stages.insert(
            "openMs".into(),
            (stage_started.elapsed().as_millis() as u64).into(),
        );
        stage_started = std::time::Instant::now();
        let paths = session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.relative_path)
            .collect();
        let lease = session
            .acquire_snapshot_lease("371-static-runtime", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        preparation_stages.insert(
            "snapshotMs".into(),
            (stage_started.elapsed().as_millis() as u64).into(),
        );
        stage_started = std::time::Instant::now();
        let prepared = compiler
            .prepare_with_artifact_cache(
                &lease,
                TargetProfile::WindowsRelease,
                &project.join("Library/CompilerCache"),
            )
            .unwrap();
        preparation_stages.insert(
            "prepareMs".into(),
            (stage_started.elapsed().as_millis() as u64).into(),
        );
        fs::write(
            evidence.join("producer-reports.json"),
            serde_json::to_vec_pretty(prepared.producer_reports()).unwrap(),
        )
        .unwrap();
        fs::write(
            evidence.join("prepare-summary.json"),
            serde_json::to_vec_pretty(&prepared.prepare_summary()).unwrap(),
        )
        .unwrap();
        stage_started = std::time::Instant::now();
        let cooked = if recipe == "BM-02" {
            project_authoring_execution::platform_3d_benchmark::inspect_static_cook(&prepared)
                .unwrap()
        } else {
            project_authoring_execution::platform_3d_benchmark::inspect_foundation_static_cook(
                &prepared,
            )
            .unwrap()
            .static_scene
        };
        fs::write(
            evidence.join("cook.json"),
            serde_json::to_vec_pretty(&cooked).unwrap(),
        )
        .unwrap();
        assert_eq!(
            (
                cooked.unique_meshes,
                cooked.unique_textures,
                cooked.base_color_decoded_bytes
            ),
            (meshes, textures, texture_bytes),
            "Cook must preserve the frozen static recipe before publication"
        );
        preparation_stages.insert(
            "inspectionMs".into(),
            (stage_started.elapsed().as_millis() as u64).into(),
        );
        stage_started = std::time::Instant::now();
        let built = compiler
            .build(
                &prepared,
                BuildRequest::for_project(
                    TargetProfile::WindowsRelease,
                    &project,
                    ProjectRelativePath::parse(if cost_only {
                        format!("Build/MeasurementCost-{}", std::process::id())
                    } else {
                        "Build/RuntimeBudget".into()
                    })
                    .unwrap(),
                    default_project_runtime_player_build_root(),
                )
                .with_timeout_ms(1200000)
                .with_frame_limit(4261)
                .with_player_verification(false),
            )
            .unwrap();
        preparation_stages.insert(
            "buildMs".into(),
            (stage_started.elapsed().as_millis() as u64).into(),
        );
        fs::write(
            evidence.join("build.json"),
            serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
        )
        .unwrap();
        // Release process must have memory available to load/render, without the
        // test's full prepared/package/source snapshot copies remaining resident.
        let package = built.delivery().package_dir().to_path_buf();
        drop(built);
        drop(prepared);
        drop(compiler);
        drop(lease);
        drop(session);
        package
    };
    fs::write(evidence.join("preparation.json"), serde_json::to_vec_pretty(&serde_json::json!({"preparationElapsedMs":preparation_started.elapsed().as_millis(),"stages":preparation_stages,"compilerInvoked":reuse_package.is_none(),"prepareOnly":prepare_only,"scope":"prepare/Cook/build/publication combined, not compiler-only time"})).unwrap()).unwrap();
    let expected_package =
        runtime_cli::semantic_file_digest(&package.join("package-manifest.json")).unwrap();
    validate_static_reuse_package(&package, &expected_package).unwrap();
    if prepare_only {
        fs::write(evidence.join("prepare-only.json"), serde_json::to_vec_pretty(&serde_json::json!({"manifestDigest":expected_package,"package":package,"playerProcesses":0,"qualification":"build preparation only"})).unwrap()).unwrap();
        return;
    }
    let mut off_p95 = None;
    let mut process_ids = std::collections::BTreeSet::new();
    for &label in if cost_only {
        static_cost_window_labels()
    } else {
        static_window_labels(diagnostic)
    } {
        let probe = label == "probe";
        let filtered_trace = label == "filtered-trace";
        let off = label.starts_with("off-");
        let (warmup, samples) = if probe {
            (120, 600)
        } else if filtered_trace {
            (600, 300)
        } else {
            (600, 3661)
        };
        let directory = evidence.join(label);
        fs::create_dir_all(&directory).unwrap();
        let report = runtime_cli::verify_exported_player_process_with_options(
            runtime_cli::ExportedPlayerProcessVerificationRequest {
                exported_package_dir: package.clone(),
                mode: "windowed".into(),
                frame_limit: warmup + samples,
                report_path: Some(directory.join("verification.json")),
                timeout_ms: if probe { 180000 } else { 300000 },
                screenshot: probe,
                screenshot_path: probe.then(|| directory.join("backbuffer.png")),
            },
            runtime_cli::ExportedPlayerProcessVerificationOptions {
                runtime_report_level: Some("off".into()),
                measurement: Some(static_measurement_policy(label, cost_only)),
                performance_warmup_frames: warmup,
                performance_sample_frames: samples,
                ..Default::default()
            },
        );
        fs::write(
            directory.join("verification.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(report.process_exit_code, Some(0), "{report:#?}");
        let child: serde_json::Value =
            serde_json::from_slice(&fs::read(&report.child_report_path).unwrap()).unwrap();
        let summary = &child["framePerformanceSummary"];
        fs::write(
            directory.join("summary.json"),
            serde_json::to_vec_pretty(summary).unwrap(),
        )
        .unwrap();
        let window = &summary["samplingWindow"];
        let resources = &window["streamingResourcesStart"];
        assert_eq!(summary["observedSampleFrames"], samples);
        assert_eq!(resources["gpuMeshes"], meshes);
        assert_eq!(resources["gpuTextures"], textures);
        assert_eq!(resources["gpuTextureBytes"], texture_bytes);
        assert_eq!(window["streamingResourcesUnchanged"], true);
        if probe {
            let frames = summary["rawCpuFrames"].as_array().unwrap();
            assert_eq!(frames.len(), (warmup + samples) as usize);
            static_instance_counts(summary, warmup as usize).unwrap();
            assert_eq!(report.screenshot_status.as_deref(), Some("captured"));
        } else if filtered_trace {
            let frames = summary["rawCpuFrames"].as_array().unwrap();
            assert_eq!(frames.len(), 300);
            assert!(frames.iter().all(|f| f["frameIndex"]
                .as_u64()
                .is_some_and(|i| (601..901).contains(&i))));
        } else {
            assert!(window["wallNs"].as_u64().unwrap() >= 60000000000);
            assert!(
                window["lastTick"].as_u64().unwrap() - window["firstTick"].as_u64().unwrap()
                    >= 3600
            );
        }
        let cpu = &summary["cpuExcludingPresentWait"];
        let gpu = &summary["gpu"]["frame"];
        if off {
            assert!(summary.get("gpu").is_none());
            assert!(summary.get("rawCpuFrames").is_none());
            assert!(
                cpu["p95Ms"].as_f64().unwrap() <= 13.3 && cpu["p99Ms"].as_f64().unwrap() <= 16.67
            );
        } else {
            assert_eq!(gpu["observedSampleFrames"], samples);
            assert_static_frame_time_budget(cpu, gpu, probe || filtered_trace);
        }
        validate_static_reuse_package(&package, &expected_package).unwrap();
        if cost_only && !probe && !filtered_trace {
            assert!(process_ids.insert(report.process_id.expect("independent process required")));
            let p95 = cpu["p95Ms"].as_f64().unwrap();
            if off {
                off_p95 = Some(p95);
            } else {
                project_authoring_execution::platform_3d_benchmark::summary_overhead(
                    off_p95.take().expect("paired Off baseline required"),
                    p95,
                    true,
                )
                .unwrap();
            }
        }
        assert!(window["peakPrivateCommitBytes"].as_u64().unwrap() <= 6 * 1024 * 1024 * 1024);
        costs.push(serde_json::json!({"window":label,"processElapsedMs":report.process_elapsed_ms,"sampleWallNs":window["wallNs"],"warmupFrames":warmup,"sampleFrames":samples,"processId":report.process_id,"cpuP95Ms":cpu["p95Ms"],"cpuP99Ms":cpu["p99Ms"],"requestedMeasurementPolicy":static_measurement_policy(label, cost_only),"scope":"process includes startup/IO/warmup/measurement; startup not independently instrumented"}));
        fs::write(
            evidence.join("execution-cost.json"),
            serde_json::to_vec_pretty(&costs).unwrap(),
        )
        .unwrap();
        eprintln!("{recipe} {label}: {cpu} GPU {gpu}");
    }
    if let Some(package) = &reuse_package {
        validate_static_reuse_package(package, reuse_digest.as_deref().unwrap()).unwrap();
    }
}

#[test]
#[ignore = "builds ordinary Windows Release and runs twenty rounds plus ten real minutes"]
fn windows_release_cells_complete_twenty_rounds_and_retirement_soak() {
    let project = PathBuf::from(std::env::var_os("AIFE_RELEASE_CELL_PROJECT").unwrap());
    let evidence = PathBuf::from(std::env::var_os("AIFE_RELEASE_CELL_EVIDENCE").unwrap());
    assert!(!evidence.exists(), "preserve first attempt");
    fs::create_dir_all(&evidence).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(project.join("Benchmark/foundation.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["retirementSoakSeconds"], 600);
    let mut session = ProjectAuthoringSession::open(&project).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("371-release-lifecycle", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let prepared = compiler
        .prepare(&lease, TargetProfile::WindowsRelease)
        .unwrap();
    let cells =
        project_authoring_execution::platform_3d_benchmark::inspect_foundation_cells(&prepared)
            .unwrap();
    fs::write(
        evidence.join("cook.json"),
        serde_json::to_vec_pretty(&cells).unwrap(),
    )
    .unwrap();
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsRelease,
                &project,
                ProjectRelativePath::parse("Build/ReleaseLifecycle").unwrap(),
                default_project_runtime_player_build_root(),
            )
            .with_timeout_ms(1_200_000)
            .with_frame_limit(100_000)
            .with_player_verification(false),
        )
        .unwrap();
    fs::write(
        evidence.join("build.json"),
        serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
    )
    .unwrap();
    let report = runtime_cli::verify_exported_player_process_with_options(
        runtime_cli::ExportedPlayerProcessVerificationRequest {
            exported_package_dir: built.delivery().package_dir().to_path_buf(),
            mode: "windowed".into(),
            frame_limit: 100_000,
            report_path: Some(evidence.join("verification.json")),
            timeout_ms: 900_000,
            screenshot: false,
            screenshot_path: None,
        },
        runtime_cli::ExportedPlayerProcessVerificationOptions {
            runtime_report_level: Some("off".into()),
            measurement: Some(runtime_player_winit::measurement::MeasurementPolicy {
                level: runtime_player_winit::measurement::MeasurementLevel::Trace,
                native_events: false,
                allocation_frame: None,
                allocation_end_frame: None,
                ..Default::default()
            }),
            performance_warmup_frames: 0,
            performance_sample_frames: 100_000,
            ..Default::default()
        },
    );
    fs::write(
        evidence.join("verification.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(report.process_exit_code, Some(0), "{report:#?}");
    assert_eq!(report.child_player_exit_code, Some(0), "{report:#?}");
    assert!(
        report.process_elapsed_ms >= 600_000,
        "ten minutes must be real wall time"
    );
    let bytes = fs::read(&report.child_report_path).unwrap();
    fs::write(evidence.join("measured-child.json"), &bytes).unwrap();
    let child: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(child["exitReason"], "session_exit_requested");
    let samples = child["framePerformanceSummary"]["rawCpuFrames"]
        .as_array()
        .unwrap();
    let zero = [
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
    let baseline = samples[0]["streamingResources"]["cpuResidentBytes"]
        .as_u64()
        .unwrap();
    let idle = |r: &serde_json::Value| {
        r["cpuResidentBytes"] == baseline && zero.iter().all(|k| r[*k] == 0)
    };
    let mut loaded = false;
    let mut closed = 0;
    let mut last_return = None;
    for sample in samples {
        let r = &sample["streamingResources"];
        if r["activeCells"] == 2 && r["gpuTextureBytes"].as_u64().unwrap() >= 64 * 1024 * 1024 {
            loaded = true;
        }
        if loaded && idle(r) {
            closed += 1;
            loaded = false;
            last_return = Some(sample);
        }
    }
    assert!(
        closed >= 22,
        "initial +20 + final display release: {closed}"
    );
    let last_return = last_return.unwrap();
    let last = samples.last().unwrap();
    assert!(idle(&last["streamingResources"]));
    let retired_wall_ns =
        last["samplingWallNs"].as_u64().unwrap() - last_return["samplingWallNs"].as_u64().unwrap();
    assert!(
        retired_wall_ns >= 600_000_000_000,
        "ten real minutes after actual resource retirement: {retired_wall_ns}ns"
    );
    let start_tick = last_return["frameIdentity"]["tickPair"][1]
        .as_u64()
        .unwrap();
    let end_tick = last["frameIdentity"]["tickPair"][1].as_u64().unwrap();
    assert!(
        end_tick - start_tick >= 35_900,
        "post-retirement simulation must keep advancing for ten minutes"
    );
    fs::write(evidence.join("lifecycle.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "closedIntervals":closed,"baselineCpuBytes":baseline,"firstRetirement":last_return,
        "lastSample":last,"retiredSoakTicks":end_tick-start_tick,"retiredSoakWallNs":retired_wall_ns,
        "limits":"Release double-cell CPU/GPU tracked lifecycle only; no OS-present or full static runtime scale qualification"
    })).unwrap()).unwrap();
}

/// Report-policy cost only for the real double-cell steady fixture. This does
/// not confer BM02/BM03 runtime capacity, OS display, or measurement-Off cost.
#[test]
#[ignore = "six isolated Release windows and one fixed filtered Trace; no competing Cook/build"]
fn windows_release_steady_report_cost_windows() {
    use runtime_player_winit::measurement::{MeasurementLevel, MeasurementPolicy};
    let project = PathBuf::from(std::env::var_os("AIFE_RELEASE_STEADY_PROJECT").unwrap());
    let evidence = PathBuf::from(std::env::var_os("AIFE_RELEASE_STEADY_EVIDENCE").unwrap());
    assert!(!evidence.exists(), "fresh evidence required");
    fs::create_dir_all(&evidence).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_slice(&fs::read(project.join("Benchmark/foundation.json")).unwrap())
            .unwrap();
    assert_eq!(fixture["finalDisplayTicks"], 30000);
    let mut session = ProjectAuthoringSession::open(&project).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("371-release-steady", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let prepared = compiler
        .prepare(&lease, TargetProfile::WindowsRelease)
        .unwrap();
    project_authoring_execution::platform_3d_benchmark::inspect_foundation_cells(&prepared)
        .unwrap();
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsRelease,
                &project,
                ProjectRelativePath::parse(format!("Build/ReleaseSteady-{}", std::process::id()))
                    .unwrap(),
                default_project_runtime_player_build_root(),
            )
            .with_timeout_ms(1200000)
            .with_frame_limit(8400)
            .with_player_verification(false),
        )
        .unwrap();
    fs::write(
        evidence.join("build.json"),
        serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
    )
    .unwrap();
    let mut results = Vec::new();
    for repeat in 0..3 {
        let mut off = 0.0_f64;
        for report_level in ["off", "summary"] {
            let label = format!("{report_level}-{repeat}");
            // The verifier's child report filename is fixed within its output
            // directory. Each window needs its own parent to preserve originals.
            let window_evidence = evidence.join(&label);
            fs::create_dir_all(&window_evidence).unwrap();
            let report = runtime_cli::verify_exported_player_process_with_options(
                runtime_cli::ExportedPlayerProcessVerificationRequest {
                    exported_package_dir: built.delivery().package_dir().to_path_buf(),
                    mode: "windowed".into(),
                    frame_limit: 8400,
                    report_path: Some(window_evidence.join("verification.json")),
                    timeout_ms: 240000,
                    screenshot: false,
                    screenshot_path: None,
                },
                runtime_cli::ExportedPlayerProcessVerificationOptions {
                    runtime_report_level: Some("off".into()),
                    measurement: Some(MeasurementPolicy {
                        level: if report_level == "off" {
                            MeasurementLevel::Off
                        } else {
                            MeasurementLevel::Summary
                        },
                        cpu_probe: true,
                        native_events: false,
                        allocation_frame: None,
                        allocation_end_frame: None,
                        ..Default::default()
                    }),
                    performance_warmup_frames: 3600,
                    performance_sample_frames: 4800,
                    ..Default::default()
                },
            );
            fs::write(
                window_evidence.join("verification.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            assert_eq!(report.process_exit_code, Some(0), "{report:#?}");
            let child: serde_json::Value =
                serde_json::from_slice(&fs::read(&report.child_report_path).unwrap()).unwrap();
            let summary = &child["framePerformanceSummary"];
            let window = &summary["samplingWindow"];
            results.push(serde_json::json!({"repeat":repeat,"gameplayReportLevel":"off","measurement":report_level,"commonCpuProbe":true,
                "processId":report.process_id,"summary":summary}));
            fs::write(
                evidence.join("cost-windows.json"),
                serde_json::to_vec_pretty(&results).unwrap(),
            )
            .unwrap();
            assert_eq!(summary["observedSampleFrames"], 4800);
            assert_eq!(window["complete"], true);
            assert!(
                window["wallNs"].as_u64().unwrap() >= 60000000000,
                "real sampling wall {window}"
            );
            assert!(
                window["lastTick"].as_u64().unwrap() - window["firstTick"].as_u64().unwrap()
                    >= 3600
            );
            assert_eq!(
                window["streamingResourcesUnchanged"], true,
                "sample must contain only steady resident cells"
            );
            assert_eq!(window["streamingResourcesStart"]["activeCells"], 2);
            assert_eq!(
                window["streamingResourcesStart"]["gpuTextureBytes"],
                64 * 1024 * 1024
            );
            let cpu = &summary["cpuExcludingPresentWait"];
            let p95 = cpu["p95Ms"].as_f64().unwrap();
            assert!(
                p95 <= 13.3 && cpu["p99Ms"].as_f64().unwrap() <= 16.67,
                "CPU {cpu}"
            );
            if report_level == "summary" {
                let gpu = &summary["gpu"];
                assert_eq!(gpu["backend"], "Vulkan");
                assert!(gpu["adapter"].as_str().unwrap().contains("RTX 3070"));
                assert_eq!(gpu["frame"]["observedSampleFrames"], 4800, "GPU {gpu}");
                assert!(
                    gpu["frame"]["p95Ms"].as_f64().unwrap() <= 13.3
                        && gpu["frame"]["p99Ms"].as_f64().unwrap() <= 16.67
                );
            } else {
                assert!(
                    summary.get("gpu").is_none(),
                    "Off must not collect GPU queries"
                );
                assert!(
                    summary.get("rawCpuFrames").is_none(),
                    "Off must not export Trace"
                );
            }
            assert!(window["peakPrivateCommitBytes"].as_u64().unwrap() <= 6 * 1024 * 1024 * 1024);
            if report_level == "off" {
                off = p95;
            } else {
                assert!(
                    p95 - off <= (off * 0.03).max(0.2),
                    "measurement Summary overhead: off={off} summary={p95}"
                );
            }
            eprintln!(
                "Release {label}: sample wall={}ms, CPU P95={p95}ms",
                window["wallNs"].as_u64().unwrap() / 1000000
            );
        }
    }
    let trace_evidence = evidence.join("filtered-trace");
    fs::create_dir_all(&trace_evidence).unwrap();
    let trace = runtime_cli::verify_exported_player_process_with_options(
        runtime_cli::ExportedPlayerProcessVerificationRequest {
            exported_package_dir: built.delivery().package_dir().to_path_buf(),
            mode: "windowed".into(),
            frame_limit: 3900,
            report_path: Some(trace_evidence.join("verification.json")),
            timeout_ms: 120000,
            screenshot: false,
            screenshot_path: None,
        },
        runtime_cli::ExportedPlayerProcessVerificationOptions {
            runtime_report_level: Some("off".into()),
            measurement: Some(MeasurementPolicy {
                level: MeasurementLevel::Trace,
                native_events: false,
                allocation_frame: None,
                allocation_end_frame: None,
                allocation_summary: false,
                first_frame: Some(3601),
                end_frame: Some(3901),

                cpu_probe: false,
            }),
            performance_warmup_frames: 3600,
            performance_sample_frames: 300,
            ..Default::default()
        },
    );
    assert_eq!(trace.process_exit_code, Some(0), "{trace:#?}");
    let child: serde_json::Value =
        serde_json::from_slice(&fs::read(&trace.child_report_path).unwrap()).unwrap();
    let frames = child["framePerformanceSummary"]["rawCpuFrames"]
        .as_array()
        .unwrap();
    assert_eq!(frames.len(), 300);
    assert!(frames.iter().all(|f| f["frameIndex"]
        .as_u64()
        .is_some_and(|i| (3601..3901).contains(&i))
        && f["streamingResources"]["activeCells"] == 2));
    fs::write(evidence.join("limits.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "scope":"double-cell Release steady report cost, not full static runtime capacity",
        "gameplayPolicy":"off","measurementPolicies":["off","summary"],"commonProbe":"explicit CPU timing/window identity/resource eligibility probe on both sides; GPU/UI counters disabled in Off",
        "display":"OS-present remains unqualified; API frame counts are not actual display events"
    })).unwrap()).unwrap();
}

/// Small real composition for allocation/stall producers. This is deliberately
/// not a replacement for original-scale or full BM08 performance qualification.
#[test]
#[ignore = "one actual Compiler/Release/Player composition; fresh project/evidence required"]
fn windows_release_memory_and_required_region_trajectory() {
    use project_authoring_execution::platform_3d_benchmark::{
        assess_observed_memory_window, MemoryBudget,
    };
    use runtime_player_winit::measurement::{MeasurementLevel, MeasurementPolicy};
    let full_run = std::env::var_os("AIFE_RESOURCE_FULL_RUN").is_some();
    let frame_limit = if full_run { 45000 } else { 1200 };
    let project = PathBuf::from(std::env::var_os("AIFE_RESOURCE_PROJECT").unwrap());
    let evidence = PathBuf::from(std::env::var_os("AIFE_RESOURCE_EVIDENCE").unwrap());
    assert!(!evidence.exists());
    fs::create_dir_all(&evidence).unwrap();
    let started = std::time::Instant::now();
    let mut session = ProjectAuthoringSession::open(&project).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("371-resource-observation", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let prepared = compiler
        .prepare_with_artifact_cache(
            &lease,
            TargetProfile::WindowsRelease,
            &project.join("Library/CompilerCache"),
        )
        .unwrap();
    let cells =
        project_authoring_execution::platform_3d_benchmark::inspect_foundation_cells(&prepared)
            .unwrap();
    assert!(cells
        .iter()
        .all(|c| c.render_instances == 2 && c.base_color_bytes == 32 * 1024 * 1024));
    fs::write(
        evidence.join("cook.json"),
        serde_json::to_vec_pretty(&cells).unwrap(),
    )
    .unwrap();
    let prepare_ms = started.elapsed().as_millis();
    let build_start = std::time::Instant::now();
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsRelease,
                &project,
                ProjectRelativePath::parse(format!(
                    "Build/ResourceObservation-{}",
                    std::process::id()
                ))
                .unwrap(),
                default_project_runtime_player_build_root(),
            )
            .with_timeout_ms(1200000)
            .with_frame_limit(frame_limit)
            .with_player_verification(false),
        )
        .unwrap();
    let build_ms = build_start.elapsed().as_millis();
    fs::write(
        evidence.join("build.json"),
        serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
    )
    .unwrap();
    let player_start = std::time::Instant::now();
    let report = runtime_cli::verify_exported_player_process_with_options(
        runtime_cli::ExportedPlayerProcessVerificationRequest {
            exported_package_dir: built.delivery().package_dir().to_path_buf(),
            mode: "windowed".into(),
            frame_limit,
            report_path: Some(evidence.join("verification.json")),
            timeout_ms: if full_run { 800000 } else { 90000 },
            screenshot: false,
            screenshot_path: None,
        },
        runtime_cli::ExportedPlayerProcessVerificationOptions {
            runtime_report_level: Some("summary".into()),
            measurement: Some(MeasurementPolicy {
                level: MeasurementLevel::Trace,
                allocation_frame: (!full_run).then_some(120),
                allocation_end_frame: (!full_run).then_some(184),
                allocation_summary: full_run,
                first_frame: Some(1),
                end_frame: Some(if full_run { 65 } else { 1201 }),
                ..Default::default()
            }),
            performance_warmup_frames: 0,
            performance_sample_frames: frame_limit,
            ..Default::default()
        },
    );
    let player_ms = player_start.elapsed().as_millis();
    // Preserve stage cost even when a later report assertion rejects the run.
    fs::write(evidence.join("timings.json"),serde_json::to_vec_pretty(&serde_json::json!({"prepareMs":prepare_ms,
        "buildMs":build_ms,"playerMs":player_ms,"totalMs":started.elapsed().as_millis(),
        "claim":"production allocation/stall integration only; full memory coverage, original-scale performance and presentation stall unqualified"})).unwrap()).unwrap();
    fs::write(
        evidence.join("verification.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(report.process_exit_code, Some(0), "{report:#?}");
    assert_eq!(report.child_player_exit_code, Some(0), "{report:#?}");
    let native_path =
        PathBuf::from(&report.child_report_path).with_extension("engine-runtime.json");
    let bytes = fs::read(native_path).unwrap();
    fs::write(evidence.join("native-player.json"), &bytes).unwrap();
    let native: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(native["sessionExitRequested"], true);
    let summary = &native["framePerformanceSummary"];
    let frames = summary["rawCpuFrames"].as_array().unwrap();
    if full_run {
        use project_authoring_execution::platform_3d_benchmark::assess_observed_memory_run;
        let run: engine_runtime::resource_memory::ResourceMemoryRunSummary =
            serde_json::from_value(native["resourceMemorySummary"].clone()).unwrap();
        let count = native["framesCompleted"].as_u64().unwrap();
        assert!(
            run.covers(1, count + 1),
            "full per-frame census interval required: {run:#?}"
        );
        let verdict =
            assess_observed_memory_run(&MemoryBudget::pc_main(), &run, 1, count + 1).unwrap();
        fs::write(
            evidence.join("full-memory-assessment.json"),
            serde_json::to_vec_pretty(&verdict).unwrap(),
        )
        .unwrap();
        fs::write(
            evidence.join("full-memory-summary.json"),
            serde_json::to_vec_pretty(&run).unwrap(),
        )
        .unwrap();
        assert!(
            frames.iter().all(|f| f.get("resourceMemory").is_none()),
            "summary must not retain raw censuses"
        );
        let obs = &native["finalProjectObservation"]["snapshot"]["values"];
        fs::write(
            evidence.join("trajectory.json"),
            serde_json::to_vec_pretty(obs).unwrap(),
        )
        .unwrap();
        let waits = assess_full_required_region_waits(obs).expect("complete wait ledger");
        fs::write(
            evidence.join("full-wait-assessment.json"),
            serde_json::to_vec_pretty(&waits).unwrap(),
        )
        .unwrap();
        assert_eq!(obs["trajectory.completed"], true);
        assert_eq!(obs["trajectory.waiting"], false);
        assert!(obs["trajectory.streamingWallNs"].as_f64().unwrap() >= 600_000_000_000.0);
        assert!(obs["trajectory.roundTrips"].as_u64().unwrap() >= 1);
        assert!(
            (obs["trajectory.actualPosition"].as_f64().unwrap()
                - obs["trajectory.expectedActualPosition"].as_f64().unwrap())
            .abs()
                < 0.001
        );
        let last = &summary["samplingWindow"]["streamingResourcesEnd"];
        for key in [
            "activeCells",
            "liveCells",
            "pendingAssetOperations",
            "pendingAssetTickets",
            "gpuRetiringBytes",
            "pendingGpuTickets",
            "gpuMeshes",
            "gpuTextures",
        ] {
            assert_eq!(
                last[key].as_u64(),
                Some(0),
                "final retirement {key}: {last:#?}"
            );
        }
        assert!(run.system_available_minimum.is_some());
        return;
    }
    let inventories: Vec<_> = frames
        .iter()
        .filter(|f| f.get("resourceMemory").is_some())
        .collect();
    assert_eq!(
        inventories.len(),
        64,
        "every requested census frame must exist"
    );
    let mut samples = Vec::new();
    let mut expected = Vec::new();
    let mut device = None;
    for inventory in inventories {
        let sample: engine_runtime::resource_memory::ResourceMemorySample =
            serde_json::from_value(inventory["resourceMemory"].clone()).unwrap();
        let gpu_sample = summary["gpu"]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["frameIndex"].as_u64() == Some(sample.frame_index))
            .unwrap();
        let matching_device = gpu_sample["submission"]["deviceEpoch"].as_u64().unwrap();
        assert_eq!(*device.get_or_insert(matching_device), matching_device);
        let frame: engine_runtime::render_thread::RenderFrameIdentity =
            serde_json::from_value(inventory["frameIdentity"].clone()).unwrap();
        assert_eq!(frame.source.run_id, native["runId"].as_str().unwrap());
        let gpu_frame: engine_runtime::render_thread::RenderFrameIdentity =
            serde_json::from_value(gpu_sample["frameIdentity"].clone()).unwrap();
        assert_eq!(
            gpu_frame, frame,
            "GPU sample must consume the same frozen Extract"
        );
        for key in [
            "activeRequests",
            "readyQueue",
            "failureCache",
            "concurrentDecodes",
            "uploadBytesPerFrame",
            "framesInFlight",
        ] {
            assert!(
                sample.capacities.contains_key(key),
                "missing actual capacity {key}"
            );
        }
        // This Windows composition currently selects Vulkan on the frozen adapter.
        // A different backend must remain explicitly unsupported, not reuse these results.
        let driver = sample
            .gpu_memory_budget
            .as_ref()
            .expect("same-device Vulkan driver budget required for this integration");
        assert_eq!(driver.source, "VK_EXT_memory_budget");
        assert_eq!(sample.gpu_available_bytes, driver.available_bytes());
        expected.push(frame);
        samples.push(sample);
    }
    let assessment = assess_observed_memory_window(
        &MemoryBudget::pc_main(),
        &samples,
        &expected,
        device.unwrap(),
        120,
        184,
    )
    .unwrap();
    assert!(assessment.cpu_total_peak.bytes > 0 && assessment.gpu_total_peak.bytes > 0);
    assert!(assessment.verdict.unavailable.contains(&"cpu.total".into()));
    assert!(!assessment
        .verdict
        .unavailable
        .contains(&"gpu.available".into()));
    assert_ne!(
        assessment.verdict.status, "passed",
        "partial backing inventory must not grant full memory qualification"
    );
    for frame in &assessment.frames {
        assert_eq!(
            frame.tracked_totals.by_owner["gpu.instrumentation"],
            summary["gpu"]["measurementBufferBytes"].as_u64().unwrap()
        );
    }
    fs::write(
        evidence.join("allocation-samples.json"),
        serde_json::to_vec_pretty(&samples).unwrap(),
    )
    .unwrap();
    fs::write(
        evidence.join("allocation-assessment.json"),
        serde_json::to_vec_pretty(&assessment).unwrap(),
    )
    .unwrap();
    let observation = &native["finalProjectObservation"];
    assert_eq!(observation["status"], "published");
    let trajectory =
        validate_required_region_observation(&observation["snapshot"]["values"]).unwrap();
    fs::write(
        evidence.join("trajectory.json"),
        serde_json::to_vec_pretty(&trajectory).unwrap(),
    )
    .unwrap();
}

fn validate_required_region_observation(
    values: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let number = |key: &str| {
        values[key]
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("missing/nonfinite {key}"))
    };
    if values["trajectory.completed"] != true || values["trajectory.waiting"] != false {
        return Err("trajectory incomplete/open wait".into());
    }
    let position = number("trajectory.position")?;
    let actual = number("trajectory.actualPosition")?;
    if !(128.0..128.54).contains(&position)
        || (position - actual).abs() > 0.001
        || number("trajectory.speed")? != 32.0
    {
        return Err("actual motion/speed mismatch".into());
    }
    let count = values["trajectory.waitCount"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 8)
        .ok_or("bounded startup/boundary waits required")?;
    let mut waits = Vec::new();
    let mut prior_end = 0.0;
    for i in 0..count {
        let key = |field: &str| format!("trajectory.wait{i}.{field}");
        let start = number(&key("startNs"))?;
        let end = number(&key("endNs"))?;
        let start_tick = number(&key("startTick"))?;
        let end_tick = number(&key("endTick"))?;
        let cell = number(&key("cell"))?;
        let at = number(&key("position"))?;
        let reason = values[key("reason")]
            .as_str()
            .ok_or("wait reason required")?;
        if start < prior_end || end < start || start_tick <= 0.0 || end_tick <= start_tick {
            return Err("wait chronology/duration missing".into());
        }
        match reason {
            "required_current_region_not_ready" | "required_next_region_not_ready"
                if (at - cell * 32.0).abs() <= 0.001 => {}
            "cell_command_safe_point" if at >= cell * 32.0 && at < cell * 32.0 + 0.54 => {}
            _ => return Err("wait position/cause does not match required region".into()),
        }
        prior_end = end;
        waits.push(serde_json::json!({"position":at,"cell":cell,"reason":reason,
            "startTick":start_tick,"endTick":end_tick,"startNs":start,"endNs":end,"durationNs":end-start}));
    }
    Ok(
        serde_json::json!({"position":position,"actualPosition":actual,"blockedSimulationSeconds":number("trajectory.blockedSimulationSeconds")?,
        "waits":waits,"presentationStall":"unqualified","loadingServiceLatency":"separate metric"}),
    )
}

// Complete simulation-wait ledger. Presentation gaps still require the OS chain.
fn assess_full_required_region_waits(
    values: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let number = |key: &str| {
        values[key]
            .as_f64()
            .filter(|v| {
                v.is_finite() && *v >= 0. && *v <= 9_007_199_254_740_991. && v.fract() == 0.
            })
            .map(|v| v as u64)
            .ok_or_else(|| format!("missing integer {key}"))
    };
    let records: Vec<serde_json::Value> = serde_json::from_str(
        values["trajectory.allWaits"]
            .as_str()
            .ok_or("all waits required")?,
    )
    .map_err(|e| e.to_string())?;
    if records.is_empty()
        || records.len() > 1024
        || records.len() as u64 != number("trajectory.waitEvents")?
    {
        return Err("incomplete wait ledger".into());
    }
    let mut total = 0u64;
    let mut maximum = 0u64;
    let mut previous_end = 0;
    let mut previous_tick = 0;
    for w in &records {
        let n = |key: &str| {
            w[key]
                .as_u64()
                .ok_or_else(|| format!("wait {key} required"))
        };
        let (start, end, first, last, cell) = (
            n("startNs")?,
            n("endNs")?,
            n("startTick")?,
            n("endTick")?,
            n("cell")?,
        );
        let at = w["position"]
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.)
            .ok_or("wait position required")?;
        let correct_position = match w["reason"].as_str() {
            Some("required_current_region_not_ready") => cell == 0 && at == 0.,
            Some("required_next_region_not_ready") => (at - cell as f64 * 32.).abs() < 0.001,
            Some("cell_command_safe_point") => {
                cell > 0 && at >= cell as f64 * 32. && at <= cell as f64 * 32. + 1.
            }
            _ => false,
        };
        if !correct_position
            || start < previous_end
            || first < previous_tick
            || end <= start
            || last <= first
        {
            return Err("invalid wait cause/position/chronology".into());
        }
        total = total
            .checked_add(end - start)
            .ok_or("wait total overflow")?;
        maximum = maximum.max(end - start);
        previous_end = end;
        previous_tick = last;
    }
    if total != number("trajectory.waitWallNs")? || maximum != number("trajectory.maximumWaitNs")? {
        return Err("wait aggregates differ from complete ledger".into());
    }
    Ok(
        json!({"waitCount":records.len(),"wallNs":total,"maximumWallNs":maximum,"waits":records,
        "scope":"complete required simulation-region waits; not actual display stalls","presentationStall":"unqualified"}),
    )
}
#[test]
fn full_wait_consumer_rejects_truncated_open_wrong_position_and_aggregate_mismatch() {
    use serde_json::json;
    let valid = json!({"trajectory.allWaits":r#"[{"position":0.0,"cell":0,"reason":"required_current_region_not_ready","startTick":1,"endTick":2,"startNs":10,"endNs":20}]"#,
        "trajectory.waitEvents":1,"trajectory.waitWallNs":10,"trajectory.maximumWaitNs":10});
    assert!(assess_full_required_region_waits(&valid).is_ok());
    for (key, value) in [
        ("trajectory.waitEvents", json!(2)),
        ("trajectory.waitWallNs", json!(11)),
        ("trajectory.maximumWaitNs", json!(11)),
        ("trajectory.allWaits", json!("[]")),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert!(assess_full_required_region_waits(&bad).is_err(), "{key}");
    }
    for key in ["endTick", "endNs", "position"] {
        let mut records: Vec<serde_json::Value> =
            serde_json::from_str(valid["trajectory.allWaits"].as_str().unwrap()).unwrap();
        records[0][key] = json!(if key == "position" { 1 } else { 0 });
        let mut bad = valid.clone();
        bad["trajectory.allWaits"] = json!(serde_json::to_string(&records).unwrap());
        assert!(assess_full_required_region_waits(&bad).is_err(), "{key}");
    }
}

#[test]
fn required_region_consumer_rejects_missing_actual_position_open_wait_and_wrong_boundary() {
    use serde_json::json;
    let valid = json!({"trajectory.completed":true,"trajectory.waiting":false,"trajectory.position":128.0,
        "trajectory.actualPosition":128.0,"trajectory.speed":32.0,"trajectory.blockedSimulationSeconds":0.1,
        "trajectory.waitCount":1,"trajectory.wait0.startNs":10,"trajectory.wait0.endNs":20,
        "trajectory.wait0.startTick":1,"trajectory.wait0.endTick":2,"trajectory.wait0.cell":1,
        "trajectory.wait0.position":32.0,"trajectory.wait0.reason":"required_next_region_not_ready"});
    assert!(validate_required_region_observation(&valid).is_ok());
    for (key, value) in [
        ("trajectory.actualPosition", json!(0)),
        ("trajectory.waiting", json!(true)),
        ("trajectory.wait0.endNs", json!(0)),
        ("trajectory.wait0.position", json!(33)),
        ("trajectory.wait0.reason", json!("request_to_active")),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert!(validate_required_region_observation(&bad).is_err(), "{key}");
    }
}
