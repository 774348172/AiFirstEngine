//! A4.1 uses the real production Host/Prepare/Projection plan and real GPU owner.
use super::*;
use engine_runtime::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use engine_runtime::frame_loop::RuntimeFrameContext;
use engine_runtime::game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec};
use engine_runtime::render_thread::RenderThreadFrameOutput;
use engine_runtime::runtime_renderer::RenderTarget;
use engine_runtime::wgpu_backend::real::RealWgpuBackend;
use std::time::Instant;

fn tick(
    host: &mut EngineHostLoop,
    world: &mut World,
    hydrator: &mut RuntimeSceneHydrator,
    fixture: &Fixture,
) -> RenderThreadFrameOutput {
    let output = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            world,
            RuntimeFrameContext::new(&fixture.package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    assert_eq!(output.report.rhi_status, "ok", "{:?}", output.report);
    output
}
fn present(backend: &mut RealWgpuBackend, output: &RenderThreadFrameOutput, extent: [u32; 2]) {
    backend
        .prepare_geometry_textures(&output.geometry_textures)
        .unwrap();
    let pixels = backend
        .render_plan_to_rgba_bytes(
            &output.renderer_output.rhi_command_plan,
            extent[0],
            extent[1],
        )
        .unwrap();
    assert_eq!(pixels.len(), extent[0] as usize * extent[1] as usize * 4);
}
fn source(count: usize) -> Value {
    let mut source = base();
    source["entities"][1]["spatial"]["transform"]["translation"] = json!([0, 0, 5]);
    let mesh = source["entities"][0].clone();
    for i in 1..count {
        let mut copy = mesh.clone();
        copy["id"] = json!(format!("cube-{i}"));
        source["entities"].as_array_mut().unwrap().push(copy);
    }
    source
}
fn save(name: &str, value: Value) {
    eprintln!("A41 {name}: {value}");
    if let Some(root) = std::env::var_os("AIFE_A41_EVIDENCE_ROOT") {
        fs::create_dir_all(&root).unwrap();
        fs::write(
            PathBuf::from(root).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn a41_shared_mesh_scale_has_no_clean_frame_uploads() {
    let mut rows = Vec::new();
    let mut backend = RealWgpuBackend::new_offscreen(1280, 720).expect("real GPU required");
    eprintln!("A41 adapter: {:?}", backend.device_context());
    for count in [1, 100, 1000] {
        let fixture = Fixture::new(source(count), None);
        let (mut world, mut hydrator) = fixture.hydrate();
        let mut host = EngineHostLoop::new("main");
        host.set_game_view_target(GameViewTargetSpec::new(
            1280,
            720,
            GameViewScalePolicy::Stretch,
        ));
        let mut cpu = Vec::new();
        let mut gpu_readback = Vec::new();
        let mut initial = None;
        let mut uploads = 0;
        let mut resident_bytes = 0;
        for frame in 0..150 {
            let start = Instant::now();
            let output = tick(&mut host, &mut world, &mut hydrator, &fixture);
            let cpu_ms = start.elapsed().as_secs_f64() * 1000.;
            let draws = production_draws(&output);
            assert_eq!(draws.len(), count);
            let geometry = &draws[0].geometry;
            for draw in &draws {
                assert!(std::sync::Arc::ptr_eq(geometry, &draw.geometry));
            }
            if let Some(first) = &initial {
                assert!(std::sync::Arc::ptr_eq(first, geometry));
                assert_eq!(output.report.resource_lifetime_report.created_count, 0);
                assert_eq!(output.report.resource_lifetime_report.uploaded_bytes, 0);
            } else {
                initial = Some(geometry.clone());
            }
            let start = Instant::now();
            present(&mut backend, &output, [1280, 720]);
            let readback_ms = start.elapsed().as_secs_f64() * 1000.;
            if frame == 0 {
                uploads = backend.geometry_upload_count();
            }
            assert_eq!(backend.geometry_upload_count(), uploads);
            assert_eq!(backend.geometry_residency().meshes, 1);
            assert_eq!(backend.geometry_residency().textures, 0);
            resident_bytes = output.report.resource_lifetime_report.resident_bytes;
            if frame >= 30 {
                cpu.push(cpu_ms);
                gpu_readback.push(readback_ms);
            }
        }
        cpu.sort_by(f64::total_cmp);
        rows.push(json!({"instances":count,"warmup":30,"samples":120,"extent":[1280,720],"profile":"dev","cpuScope":"Host tick through production RHI plan; excludes GPU/readback","cpuMeanMs":cpu.iter().sum::<f64>() / 120.,"cpuP95Ms":cpu[113],"gpuSubmitAndReadbackMeanMs":gpu_readback.iter().sum::<f64>() / 120.,"draws":count,"cleanPrepareCreates":0,"cleanGeometryUploads":0,"textureUploads":0,"textureFixture":"untextured","logicalResidentBytes":resident_bytes,"gpuResidency":backend.geometry_residency()}));
    }
    save("scale", json!(rows));
}

#[test]
fn a41_eight_scene_cycles_release_handles_and_restore_after_resize() {
    let fixture = Fixture::new(source(1), None);
    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&fixture.package);
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(GameViewTargetSpec::new(
        1280,
        720,
        GameViewScalePolicy::Stretch,
    ));
    let mut backend = RealWgpuBackend::new_offscreen(1280, 720).expect("real GPU required");
    let mut old_geometry = Vec::new();
    let mut rows = Vec::new();
    for cycle in 0..8 {
        let report = hydrator.hydrate_active_scene(&fixture.package, &mut world);
        assert!(!report.has_errors(), "{report:?}");
        let instance = report.instance.unwrap();
        let output = tick(&mut host, &mut world, &mut hydrator, &fixture);
        let geometry = production_draws(&output)[0].geometry.clone();
        let handle = production_draws(&output)[0].geometry_handle();
        assert!(
            !old_geometry.contains(&handle),
            "released generation reused"
        );
        old_geometry.push(handle);
        present(&mut backend, &output, [1280, 720]);
        let uploads = backend.geometry_upload_count();
        let resized = host.render_thread_for_target(RenderTarget::surface("main", 960, 540));
        assert_eq!(
            production_draws(&resized)[0].view.viewport,
            [0, 0, 960, 540]
        );
        present(&mut backend, &resized, [960, 540]);
        let suspended = host.render_thread_for_target(RenderTarget::surface("main", 0, 0));
        assert!(production_draws(&suspended).is_empty());
        let restored = host.render_thread_for_target(RenderTarget::surface("main", 1280, 720));
        present(&mut backend, &restored, [1280, 720]);
        assert_eq!(backend.geometry_upload_count(), uploads);
        assert_eq!(backend.geometry_residency().depth_targets, 1);
        drop((output, resized, suspended, restored, geometry));
        hydrator
            .instance_loader_mut()
            .unload_scene_instance(instance.instance_id, &mut world);
        assert_eq!(world.entity_count(), 0);
        // Resource owner has a safe-frame delay; drain it using real Host frames.
        let mut after = tick(&mut host, &mut world, &mut hydrator, &fixture);
        for _ in 0..4 {
            after = tick(&mut host, &mut world, &mut hydrator, &fixture);
        }
        assert!(production_draws(&after).is_empty());
        assert_eq!(host.render_scene().proxies_len(), 0);
        assert_eq!(
            host.render_scene()
                .views()
                .filter(|v| v.spatial.is_some())
                .count(),
            0
        );
        assert_eq!(
            after.report.resource_lifetime_report.pending_release_count,
            0
        );
        assert_eq!(after.report.resource_lifetime_report.resident_bytes, 0);
        assert_eq!(
            hydrator
                .instance_loader_mut()
                .asset_loader_mut()
                .decoded_cache_len(),
            0
        );
        present(&mut backend, &after, [1280, 720]);
        let residency = backend.geometry_residency();
        assert_eq!(
            (
                residency.meshes,
                residency.textures,
                residency.depth_targets,
                residency.pipelines
            ),
            (0, 0, 0, 0)
        );
        rows.push(json!({"cycle":cycle+1,"logicalResidentBytes":0,"gpuResidency":residency,"resize":[960,540],"suspendRestore":"owner target events, not OS minimize"}));
    }
    save("lifecycle", json!(rows));
}
