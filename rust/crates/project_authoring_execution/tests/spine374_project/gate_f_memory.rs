//! Explicit bounded diagnosis at existing owners; not Release qualification.
use super::*;
use engine_runtime::{
    engine_host_loop::*,
    frame_loop::RuntimeFrameContext,
    project_runtime_session::*,
    spine_animation::{control, session::AnimatorCommand},
};
use project_runtime_sdk::ProjectRuntimeAnimatorIntent;

#[test]
#[ignore = "build a fixed single WindowsRelease delivery once for memory verification"]
fn spine374_f_build_fixed_memory_delivery() {
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    let f = project(&contract["recipes"][0]);
    let mut session = ProjectAuthoringSession::open(&f.source).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("spine374-f-fixed-memory", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let started = Instant::now();
    let prepared = compiler
        .prepare_with_artifact_cache(&lease, TargetProfile::WindowsRelease, &f.root.join("cache"))
        .unwrap();
    let prepare_ms = started.elapsed().as_millis();
    let started = Instant::now();
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsRelease,
                &f.source,
                ProjectRelativePath::parse("Build/Playable").unwrap(),
                project_authoring_execution::default_project_runtime_player_build_root(),
            )
            .with_frame_limit(4200)
            .with_player_verification(false)
            .with_timeout_ms(1_200_000),
        )
        .unwrap();
    let evidence = PathBuf::from(std::env::var_os("SPINE374_EVIDENCE_ROOT").unwrap());
    fs::write(
        evidence.join("fixed-delivery.json"),
        serde_json::to_vec_pretty(&json!({
            "prepareMs":prepare_ms,"buildMs":started.elapsed().as_millis(),
            "package":built.delivery().package_dir(),"projectSource":f.source,
            "export":built.desktop_export(),"formalQualification":false
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "prepare an empty counterfactual with the same project module; no binary build"]
fn spine374_f_prepare_empty_memory_counterfactual() {
    let original_root = PathBuf::from(std::env::var_os("SPINE374_MEMORY_PACKAGE").unwrap());
    let original = load_runtime_package(&original_root).value.unwrap();
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    let mut recipe = contract["recipes"][0].clone();
    recipe["actors"] = json!(0);
    let f = project(&recipe);
    let mut input = prepare_input(&f, TargetProfile::WindowsRelease);
    // The project source is identical; reuse its compiled module identity while
    // assembling only scene data through the ordinary package builder.
    assert_eq!(
        fs::read(f.source.join("RuntimeModule/src/lib.rs")).unwrap(),
        include_bytes!("gate_f_runtime.rs")
    );
    input.project.runtime_module = original.manifest.project.runtime_module.clone();
    let root = f.package(&input);
    let empty = load_runtime_package(&root).value.unwrap();
    assert_eq!(empty.active_scene.entities.len(), 2);
    assert!(
        empty
            .spine_animator_registry
            .as_ref()
            .is_none_or(|r| r.controllers.is_empty())
    );
    assert_eq!(
        empty.manifest.project.runtime_module,
        original.manifest.project.runtime_module
    );
    let evidence = PathBuf::from(std::env::var_os("SPINE374_EVIDENCE_ROOT").unwrap());
    fs::write(
        evidence.join("empty-package.json"),
        serde_json::to_vec_pretty(&json!({
            "original":original_root,"empty":root,"projectSource":f.source,
            "module":empty.manifest.project.runtime_module,"formalQualification":false
        }))
        .unwrap(),
    )
    .unwrap();
}

fn sample(phase: &str, tick: u64) -> Value {
    json!({"phase":phase,"tick":tick,
        "privateBytes":engine_runtime::windowed_player::sample_player_process_memory().map(|s|s.private_commit_bytes),
        "nativeLiveAllocations":engine_spine_runtime::live_native_allocations()})
}
fn clip(tick: u64) -> Option<usize> {
    [0, 30].iter().position(|&at| at == (tick - 1) % 120)
}
struct Cycle;
impl ProjectRuntimeSession for Cycle {
    fn session_id(&self) -> &str {
        "spine374-f-memory-owner"
    }
    fn handle_aui_actions(
        &mut self,
        _: ProjectRuntimeSessionContext<'_>,
        _: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput {
        ProjectRuntimeSessionOutput::no_op()
    }
    fn fixed_update(&mut self, c: ProjectRuntimeSessionContext<'_>) -> ProjectRuntimeSessionOutput {
        let mut mutations = ProjectRuntimeMutationBuffer::new();
        if let Some(clip) = clip(c.time.fixed_frame_count) {
            for layer in 0..2 {
                mutations.animator_command(AnimatorCommand {
                    entity_id: "actor-0".into(),
                    generation: c.world.entity_generation(&"actor-0".into()).unwrap(),
                    intent: ProjectRuntimeAnimatorIntent::Play {
                        name: format!("layer{layer}-clip{clip}"),
                    },
                });
            }
        }
        ProjectRuntimeSessionOutput::applied(mutations)
    }
}

#[test]
#[ignore = "bounded single-recipe memory diagnosis; explicit run, not qualification"]
fn spine374_f_memory_at_native_and_host_render_owners() {
    let root = std::env::var_os("SPINE374_MEMORY_PACKAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let contract: Value = serde_json::from_str(CONTRACT).unwrap();
            let f = project(&contract["recipes"][0]);
            f.package(&prepare_input(&f, TargetProfile::WindowsRelease))
        });
    let package = load_runtime_package(&root).value.unwrap();
    let registry = package.spine_animator_registry.as_ref().unwrap();
    assert_eq!(registry.controllers.len(), 1);
    let source = registry.validated_asset(&root, "hero").unwrap();
    let definition = &registry.controllers[0];
    let mut samples = vec![sample("before", 0)];
    let baseline = engine_spine_runtime::live_native_allocations();
    {
        let table = definition
            .compile(&source, 60, registry.assets[0].pixels_per_unit)
            .unwrap();
        let mut instance = table.instance().unwrap();
        let mut steady = None;
        let started = Instant::now();
        for tick in 1..=4200 {
            let intents = clip(tick)
                .map(|clip| {
                    (0..2)
                        .map(|layer| {
                            let name = format!("layer{layer}-clip{clip}");
                            control::Intent::Play(
                                definition
                                    .actions
                                    .iter()
                                    .position(|a| a.name == name)
                                    .unwrap() as u16,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            instance.advance(tick, &intents).unwrap();
            let geometry = instance.geometry().unwrap();
            assert_eq!(geometry.vertices.len(), 512);
            drop(geometry);
            if tick >= 600 && tick % 120 == 0 {
                let count = engine_spine_runtime::live_native_allocations();
                assert_eq!(
                    *steady.get_or_insert(count),
                    count,
                    "official allocation count grew at cycle {tick}"
                );
                samples.push(sample("native", tick));
            }
            assert!(
                started.elapsed().as_secs() < 90,
                "native owner diagnosis exceeded bound"
            );
        }
    }
    assert_eq!(engine_spine_runtime::live_native_allocations(), baseline);
    samples.push(sample("native-dropped", 4200));
    host_render_owner(&package, false, &mut samples);
    samples.push(sample("host-dropped", 4200));
    save_samples(&samples);
}

#[test]
#[ignore = "bounded original HostLoop plus real offscreen GPU memory diagnosis"]
fn spine374_f_memory_at_gpu_render_owner() {
    let root = PathBuf::from(std::env::var_os("SPINE374_MEMORY_PACKAGE").unwrap());
    let package = load_runtime_package(&root).value.unwrap();
    let mut samples = vec![sample("before", 0)];
    host_render_owner(&package, true, &mut samples);
    samples.push(sample("gpu-host-dropped", 4200));
    save_samples(&samples);
}

fn host_render_owner(package: &RuntimePackage, gpu: bool, samples: &mut Vec<Value>) {
    use engine_runtime::engine_rhi::{EngineRhiBackend, RhiBackendDiagnosticSeverity};
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    use std::time::Duration;
    {
        let mut backend = gpu.then(|| RealWgpuBackend::new_offscreen(1280, 720).unwrap());
        let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
            .value
            .unwrap();
        let mut loader =
            engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(package);
        let mut host = EngineHostLoop::with_project_runtime_session(
            "main",
            engine_runtime::project_logic::ProjectLogicRunner::empty(),
            Box::new(Cycle),
        );
        host.set_external_rhi_enabled(true);
        let started = Instant::now();
        for tick in 1..=4200 {
            let output = host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_unscaled_delta_time(1. / 60.)
                    .with_fixed_step_count(1)
                    .with_deferred_render_submission(),
                &mut world,
                RuntimeFrameContext::new(package, &mut loader),
            );
            assert!(
                output.runtime_advanced && output.host_control.is_none(),
                "{output:#?}"
            );
            let render = host.render_thread_for_target(
                engine_runtime::runtime_renderer::RenderTarget::surface("memory-owner", 1280, 720),
            );
            assert_eq!(render.report.rhi_status, "ok", "{:?}", render.report);
            assert_eq!(
                host.retained_geometry_snapshot()
                    .unwrap()
                    .prepared_geometries()
                    .count(),
                1
            );
            if let Some(backend) = &mut backend {
                backend
                    .prepare_geometry_textures(&render.geometry_textures)
                    .unwrap();
                let submitted = backend.execute_plan(&render.renderer_output.rhi_command_plan);
                assert!(
                    !submitted
                        .diagnostics
                        .iter()
                        .any(|d| d.severity == RhiBackendDiagnosticSeverity::Error),
                    "{:?}",
                    submitted.diagnostics
                );
                backend
                    .wait_gpu_submission(
                        backend.last_gpu_submission().unwrap(),
                        Duration::from_secs(5),
                    )
                    .unwrap();
            }
            drop((output, render));
            if tick >= 600 && tick % 120 == 0 {
                samples.push(sample(
                    if gpu {
                        "host-gpu-render"
                    } else {
                        "host-cpu-render"
                    },
                    tick,
                ));
                save_samples(samples);
            }
            assert!(
                started.elapsed().as_secs() < 90,
                "host owner diagnosis exceeded bound"
            );
        }
        if let Some(backend) = &mut backend {
            backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
        }
    }
}

fn save_samples(samples: &[Value]) {
    let parent = std::env::var_os("SPINE374_EVIDENCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&parent).unwrap();
    fs::write(
        parent.join("f-owner-memory.json"),
        serde_json::to_vec_pretty(&samples).unwrap(),
    )
    .unwrap();
}
