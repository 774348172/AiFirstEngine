use super::*;
use project_authoring_execution::platform_3d_benchmark::{
    inspect_lit_cook, inspect_lit_diagnostic,
};
use sha2::{Digest, Sha256};
use std::path::Path;

#[test]
#[ignore = "run-owned package diagnostic; never recooks or republishes"]
fn s2d_lit_loaded_package_environment_admission_diagnostic() {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        runtime_asset_loader::StreamingBudget,
        runtime_package::{load_runtime_package, RuntimeAssetRef},
        runtime_scene_hydration::RuntimeSceneHydrator,
    };
    let root = PathBuf::from(std::env::var_os("S2D_LIT_PACKAGE").expect("existing owned package"));
    let package = load_runtime_package(&root).value.unwrap();
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    hydrator
        .instance_loader_mut()
        .asset_loader_mut()
        .set_streaming_budget(StreamingBudget {
            max_operations: 256,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let mut host = EngineHostLoop::new("main");
    let frame = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    let loader = hydrator.instance_loader_mut().asset_loader_mut();
    let reference = RuntimeAssetRef {
        id: "studio".into(),
        asset_type: "environment".into(),
        guid: Some("371-lit-studio".into()),
        sub_asset: None,
    };
    eprintln!(
        "frame_errors={:?} budget={:?} streaming={:?} cache={}",
        frame.report.diagnostics,
        loader.streaming_budget(),
        loader.streaming_snapshot(),
        loader.decoded_cache_len()
    );
    let result = loader.request_async(&reference);
    eprintln!("environment_request={result:?}");
    let mut live = std::collections::HashSet::new();
    for event in loader.diagnostics().entries() {
        if let Some(id) = event.handle_id {
            match event.stage {
                engine_runtime::runtime_asset_diagnostics::AssetLoadStage::Apply => {
                    live.insert(id);
                }
                engine_runtime::runtime_asset_diagnostics::AssetLoadStage::Release => {
                    live.remove(&id);
                }
                _ => {}
            }
        }
    }
    eprintln!(
        "sync_live_handles={} limit={}",
        live.len(),
        loader.streaming_budget().max_handles
    );
    assert!(live.len() > loader.streaming_budget().max_handles);
    assert_eq!(
        result,
        Err(engine_runtime::runtime_asset_loader::StreamingError::BudgetExceeded)
    );
    let baseline = loader.streaming_budget();
    let same_budget = loader.set_streaming_budget(baseline);
    assert_eq!(
        same_budget,
        Err(engine_runtime::runtime_asset_loader::StreamingError::BudgetExceeded)
    );
    loader
        .set_streaming_budget(StreamingBudget {
            max_handles: live.len() + 1,
            ..baseline
        })
        .unwrap();
    let result = loader.request_async(&reference);
    eprintln!("environment_request_with_exact_handle_capacity={result:?}");
    assert!(
        result.is_ok(),
        "original package must admit IBL after ordinary mesh/material loads: {result:?}"
    );
}

fn prepare(root: &Path) -> project_authoring_execution::PreparedRuntimePackage {
    prepare_cached(root, None)
}
fn prepare_cached(
    root: &Path,
    cache: Option<&Path>,
) -> project_authoring_execution::PreparedRuntimePackage {
    let mut session = ProjectAuthoringSession::open(root).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session.acquire_snapshot_lease("s2d-lit", paths).unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    if let Some(cache) = cache {
        compiler.prepare_with_artifact_cache(&lease, TargetProfile::WindowsDev, cache)
    } else {
        compiler.prepare(&lease, TargetProfile::WindowsDev)
    }
    .unwrap()
}

#[test]
#[ignore = "bounded one target PNG Cook cost before the full 96-texture recipe"]
fn s2d_lit_single_2048_cook_cost() {
    let f = Fixture::new();
    let image = PathBuf::from(std::env::var_os("S2D_LIT_TEXTURE").expect("one original 2048 PNG"));
    fs::copy(image, f.root.join("Assets/target.png")).unwrap();
    f.write("Assets/target.json", &json!({"schemaVersion":"texture-asset.v1","assetId":"target","assetGuid":"371-cost-target",
        "sourceImage":"Assets/target.png","importer":{"colorSpace":"srgb","gpuCompression":"bc3","sampler":"linearMipmapLinear:linear:repeat:repeat"}}));
    let started = std::time::Instant::now();
    let prepared = prepare(&f.root);
    let source = prepared
        .runtime_package_build_input()
        .texture_payloads
        .iter()
        .find(|t| t.metadata.asset_id == "target")
        .unwrap();
    assert_eq!(
        (
            source.metadata.width,
            source.metadata.mip_count,
            source.rgba8.len()
        ),
        (2048, 12, 5592432)
    );
    eprintln!(
        "target_texture_prepare_ms={:.3} producers={}",
        started.elapsed().as_secs_f64() * 1000.,
        serde_json::to_string(prepared.producer_reports()).unwrap()
    );
}

#[test]
fn s2d_lit_sample_cooks_real_slots_and_rejects_target_inflation() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../samples/platform_3d_lit_project");
    let prepared = prepare(&root);
    let evidence = inspect_lit_diagnostic(&prepared).unwrap();
    assert_eq!(evidence.lights, [1, 128, 16]);
    assert_eq!(evidence.shadow_lights, [1, 0, 8]);
    assert_eq!((evidence.mask_instances, evidence.blend_instances), (2, 1));
    assert_eq!(evidence.qualification, "unqualified");
    assert_eq!(
        inspect_lit_cook(&prepared).unwrap_err().code,
        "benchmark.lit_payload_ineligible"
    );
}

#[test]
#[ignore = "explicit bounded Windows GPU diagnostic; S2D_LIT_SOURCE/S2D_LIT_EVIDENCE optional"]
#[cfg(feature = "real-wgpu")]
fn s2d_lit_recipe_ordinary_project_to_combined_gpu_admission() {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        rhi_command_plan::RhiCommand,
        runtime_package::{load_runtime_package, RuntimeProjectInfo},
        runtime_package_builder::{
            RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
        },
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let root = std::env::var_os("S2D_LIT_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../samples/platform_3d_lit_project")
        });
    let output = std::env::var_os("S2D_LIT_EVIDENCE")
        .map(PathBuf::from)
        .expect("fresh owned evidence root required");
    fs::create_dir(&output).unwrap();
    let started = std::time::Instant::now();
    let (package, evidence, cook_ms, package_ms) = if let Some(existing) =
        std::env::var_os("S2D_LIT_PACKAGE")
    {
        let existing = PathBuf::from(existing);
        let cook = fs::read(existing.parent().unwrap().join("cook.json")).unwrap();
        let evidence: Value = serde_json::from_slice(&cook).unwrap();
        assert_eq!(evidence["qualification"], "unqualified");
        fs::write(output.join("cook.json"), cook).unwrap();
        fs::write(output.join("package-reuse.json"), serde_json::to_vec_pretty(&json!({
            "package":existing,"scope":"same immutable run-owned package; no Cook or package rebuild",
            "manifestSha256":format!("{:x}", Sha256::digest(fs::read(existing.join("manifest.json")).unwrap()))
        })).unwrap()).unwrap();
        let load_start = std::time::Instant::now();
        let loaded = load_runtime_package(&existing);
        assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
        (
            loaded.value.unwrap(),
            evidence,
            0.,
            load_start.elapsed().as_secs_f64() * 1000.,
        )
    } else {
        let cache = std::env::var_os("S2D_LIT_COOK_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|| output.join("cook-cache"));
        let prepared = prepare_cached(&root, Some(&cache));
        let mini = serde_json::from_slice::<Value>(
            &fs::read(root.join("Benchmark/lit-manifest.json")).unwrap(),
        )
        .unwrap()["diagnosticMini"]
            == true;
        let evidence = if mini {
            inspect_lit_diagnostic(&prepared)
        } else {
            inspect_lit_cook(&prepared)
        }
        .unwrap();
        fs::write(
            output.join("cook.json"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
        fs::write(
            output.join("producers.json"),
            serde_json::to_vec_pretty(prepared.producer_reports()).unwrap(),
        )
        .unwrap();
        fs::write(
            output.join("prepare.json"),
            serde_json::to_vec_pretty(&prepared.prepare_summary()).unwrap(),
        )
        .unwrap();
        let cook_ms = started.elapsed().as_secs_f64() * 1000.;
        // Package identity uses explicit empty runtime composition; this diagnostic
        // has no gameplay module and never claims an exported Player.
        let mut input = prepared.runtime_package_build_input().clone();
        input.project = RuntimeProjectInfo::explicit_empty("lit-recipe", "Lit Recipe", "1");
        drop(prepared);
        let package_start = std::time::Instant::now();
        let package_root = output.join("package");
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&package_root, "main"),
            &input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:?}"
        );
        drop(input);
        let loaded = load_runtime_package(&package_root);
        assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
        let package = loaded.value.unwrap();
        let package_ms = package_start.elapsed().as_secs_f64() * 1000.;
        (
            package,
            serde_json::to_value(evidence).unwrap(),
            cook_ms,
            package_ms,
        )
    };
    let runtime_start = std::time::Instant::now();
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    hydrator
        .instance_loader_mut()
        .asset_loader_mut()
        .set_streaming_budget(engine_runtime::runtime_asset_loader::StreamingBudget {
            max_operations: 256,
            // The 10k-instance source holds 50,614 synchronous consumer leases.
            // This is a handle-count profile, not a raised byte/request budget.
            max_handles: 65_536,
            max_cpu_resident_bytes: 2 * 1024 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(GameViewTargetSpec::new(
        1920,
        1080,
        GameViewScalePolicy::Stretch,
    ));
    let mut backend = RealWgpuBackend::new_offscreen(1920, 1080).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let frame = loop {
        let frame = host
            .tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                &mut world,
                RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
            )
            .render_thread_frame
            .unwrap();
        assert!(
            std::time::Instant::now() < deadline,
            "async readiness timeout: {:?}",
            frame.report.diagnostics
        );
        if frame.report.rhi_status != "ok" {
            assert!(
                format!("{:?}", frame.report.diagnostics).contains("environment.async"),
                "{:?}",
                frame.report.diagnostics
            );
            std::thread::yield_now();
            continue;
        }
        let draws = frame
            .renderer_output
            .rhi_command_plan
            .commands
            .iter()
            .flat_map(RhiCommand::geometry_instances)
            .filter(|(draw, _, _)| !draw.view.is_shadow_map())
            .count();
        if draws == 0
            || frame.geometry_textures.len() != evidence["textures"].as_u64().unwrap() as usize
        {
            std::thread::yield_now();
            continue;
        }
        break frame;
    };
    let plan = &frame.renderer_output.rhi_command_plan;
    let shadow_views = plan
        .commands
        .iter()
        .filter(|c| matches!(c,RhiCommand::BeginView { view,.. } if view.is_shadow_map()))
        .count();
    assert_eq!(shadow_views, 12, "four cascades plus eight spots");
    let mut lit_draws = 0;
    for command in &plan.commands {
        if let Some(draw) = command.geometry_draw() {
            if !draw.view.is_shadow_map() {
                assert!(
                    draw.standard_lit.is_some() && draw.environment.is_some(),
                    "PBR/IBL consumer lost"
                );
                lit_draws += command.geometry_instance_count();
            }
        }
    }
    use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, GeometryUploadState};
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 2 * 1024 * 1024 * 1024,
            staging_bytes: 128 * 1024 * 1024,
            bytes_per_frame: 16 * 1024 * 1024,
            max_tickets: 256,
        })
        .unwrap();
    let ticket = backend
        .begin_geometry_upload(&[], &frame.geometry_textures)
        .unwrap();
    let mut upload_frames = 0u64;
    let mut peak_staging = 0;
    let mut peak_in_flight = 0;
    let mut upload_ledger = Vec::new();
    while backend.geometry_upload_state(ticket).unwrap() != GeometryUploadState::Ready {
        assert!(upload_frames < 2048, "upload did not terminate");
        while backend.poll_gpu_completion().unwrap().in_flight >= 2 {
            std::thread::yield_now();
        }
        let bytes = backend.pump_geometry_uploads(upload_frames).unwrap();
        let upload = backend.geometry_upload_snapshot();
        let life = backend.gpu_lifecycle_stats();
        peak_staging = peak_staging.max(upload.staging_bytes);
        peak_in_flight = peak_in_flight.max(life.in_flight);
        assert!(
            bytes <= 16 * 1024 * 1024
                && upload.staging_bytes <= 128 * 1024 * 1024
                && life.in_flight <= 2
        );
        upload_ledger.push(json!({"frame":upload_frames,"bytes":bytes,"staging":upload.staging_bytes,"inFlight":life.in_flight}));
        upload_frames += 1;
    }
    fs::write(
        output.join("upload-ledger.json"),
        serde_json::to_vec_pretty(&upload_ledger).unwrap(),
    )
    .unwrap();
    let cold_started = std::time::Instant::now();
    let pixels = backend.render_plan_to_rgba_bytes(plan, 1920, 1080).unwrap();
    let cold_draw_readback_ms = cold_started.elapsed().as_secs_f64() * 1000.;
    let cold_residency = backend.geometry_residency();
    let cold_stats = backend.retained_geometry_stats();
    let warm_started = std::time::Instant::now();
    let warm_pixels = backend.render_plan_to_rgba_bytes(plan, 1920, 1080).unwrap();
    let warm_draw_readback_ms = warm_started.elapsed().as_secs_f64() * 1000.;
    assert_eq!(
        pixels, warm_pixels,
        "same immutable recipe must replay identical pixels"
    );
    assert_eq!(
        backend.geometry_residency(),
        cold_residency,
        "same plan must reuse resident pipelines/targets"
    );
    let warm_stats = backend.retained_geometry_stats();
    assert_eq!(warm_stats.parameter_uploads, cold_stats.parameter_uploads);
    let mut encoder = png::Encoder::new(
        fs::File::create(output.join("combined.png")).unwrap(),
        1920,
        1080,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
    let mut memory = engine_runtime::resource_memory::ResourceMemorySample::new(0, 0, 0);
    backend.observe_memory(&mut memory);
    fs::write(
        output.join("memory.json"),
        serde_json::to_vec_pretty(&memory).unwrap(),
    )
    .unwrap();
    let residency = backend.geometry_residency();
    assert_eq!(
        residency.meshes,
        evidence["meshes"].as_u64().unwrap() as usize
    );
    assert_eq!(
        residency.textures,
        evidence["textures"].as_u64().unwrap() as usize
    );
    let totals = engine_runtime::resource_memory::memory_totals(&memory.allocations).unwrap();
    assert!(totals.gpu_allocated <= 2 * 1024 * 1024 * 1024);
    let gpu_ms = runtime_start.elapsed().as_secs_f64() * 1000.;
    backend.release_geometry_upload(ticket).unwrap();
    assert_eq!(backend.geometry_residency().textures, 0);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(10))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    fs::write(output.join("result.json"),serde_json::to_vec_pretty(&json!({"scope":evidence["scope"],"qualification":"unqualified",
        "cookMs":cook_ms,"packageMs":package_ms,"hydrateAdapterDrawReadbackMs":gpu_ms,"wallMs":started.elapsed().as_secs_f64()*1000.,
        "litDraws":lit_draws,"shadowViews":shadow_views,"residency":residency,"memoryTotals":totals,"retiringAfterShutdown":0,
        "uploadPumpFrames":upload_frames,"peakStaging":peak_staging,"peakInFlight":peak_in_flight,
        "coldDrawReadbackMs":cold_draw_readback_ms,"warmDrawReadbackMs":warm_draw_readback_ms,"coldStats":cold_stats,"warmStats":warm_stats,
        "notExecuted":["full-K1","actual-present","formal-three-rounds","full-process-memory","per-present-upload-window"]})).unwrap()).unwrap();
}
