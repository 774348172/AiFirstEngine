use super::*;
use engine_runtime::{engine_host_loop::*, frame_loop::RuntimeFrameContext};
use project_authoring_execution::{
    BuildRequest, ExecutionMode, ProjectRelativePath, VerifyRequest,
};
use std::time::Instant;

#[test]
fn spine374_f_equal_geometry_reuses_prepare_with_distinct_entity_transforms() {
    let f = Fixture::new("f-call-local-prepare");
    let mut description = f.authored();
    description["assetId"] = json!("other-controller");
    description["entity"] = json!("other");
    f.write(
        "Animations/other.animator-description-2d.json",
        &description,
    );
    let scene_path = f.source.join("Scenes/main.scene.json");
    let mut scene: Value = serde_json::from_slice(&fs::read(&scene_path).unwrap()).unwrap();
    let mut other = scene["entities"][0].clone();
    other["id"] = json!("other");
    other["name"] = json!("Other");
    other["transform"]["localPosition"]["x"] = json!(20.);
    scene["entities"].as_array_mut().unwrap().push(other);
    f.write("Scenes/main.scene.json", &scene);
    let input = f.prepare().unwrap();
    let root = f.package(&input);
    let package = load_runtime_package(&root).value.unwrap();
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut loader =
        engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
    let mut host = EngineHostLoop::new("main");
    host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_unscaled_delta_time(1. / 60.)
            .with_fixed_step_count(1)
            .with_deferred_render_submission(),
        &mut world,
        RuntimeFrameContext::new(&package, &mut loader),
    );
    // The first host tick creates the SpineModule. Begin measurement only once
    // that owner exists, as in the original dynamic-host consumer below.
    host.begin_spine_measurement_frame(true);
    host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_unscaled_delta_time(1. / 60.)
            .with_fixed_step_count(1)
            .with_deferred_render_submission(),
        &mut world,
        RuntimeFrameContext::new(&package, &mut loader),
    );
    let snapshot = host.retained_geometry_snapshot().unwrap();
    let prepared = snapshot.prepared_geometries().collect::<Vec<_>>();
    assert_eq!(prepared.len(), 2);
    assert!(
        std::ptr::eq(prepared[0], prepared[1]),
        "duplicate immutable Prepare work"
    );
    let sample = host.spine_cpu_measurement().unwrap();
    assert_eq!(
        (sample.evaluated_instances, sample.geometry_instances),
        (2, 2)
    );
    assert_eq!(
        world.transform(&"other".into()).unwrap().local_position.x,
        20.
    );
}

#[test]
fn spine374_e_host_projects_current_geometry_into_retained_frame() {
    let f = Fixture::new("e-dynamic-host");
    f.authored();
    let input = f.prepare().unwrap();
    let root = f.package(&input);
    let package = load_runtime_package(&root).value.unwrap();
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut loader =
        engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
    let mut host = EngineHostLoop::new("main");
    host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_unscaled_delta_time(1. / 60.)
            .with_fixed_step_count(1)
            .with_deferred_render_submission(),
        &mut world,
        RuntimeFrameContext::new(&package, &mut loader),
    );
    let snapshot = host.retained_geometry_snapshot().expect("production frame");
    assert_eq!(
        snapshot.prepared_geometries().count(),
        1,
        "live Spine pose never entered production geometry"
    );
    #[cfg(feature = "real-wgpu")]
    {
        let frozen = snapshot.clone();
        let before = frozen
            .prepared_geometries()
            .next()
            .unwrap()
            .upload_source()
            .1
            .vertex_bytes()
            .to_vec();
        for _ in 0..20 {
            host.begin_spine_measurement_frame(true);
            host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame)
                    .with_unscaled_delta_time(1. / 60.)
                    .with_fixed_step_count(1)
                    .with_deferred_render_submission(),
                &mut world,
                RuntimeFrameContext::new(&package, &mut loader),
            );
            let measured = host.spine_cpu_measurement().unwrap();
            assert!(measured.control_ns > 0 && measured.official_apply_world_ns > 0);
            assert!(measured.vertices_clipping_ns > 0 && measured.projection_encode_ns > 0);
            assert_eq!(
                (measured.evaluated_instances, measured.geometry_instances),
                (1, 1)
            );
            assert!(measured.vertices > 0 && measured.indices > 0);
        }
        host.begin_spine_measurement_frame(false);
        assert!(host.spine_cpu_measurement().is_none());
        let after = host
            .retained_geometry_snapshot()
            .unwrap()
            .prepared_geometries()
            .next()
            .unwrap()
            .upload_source()
            .1;
        assert_ne!(
            before,
            after.vertex_bytes(),
            "Spine was rendered as a static setup pose"
        );
        assert_eq!(
            before,
            frozen
                .prepared_geometries()
                .next()
                .unwrap()
                .upload_source()
                .1
                .vertex_bytes(),
            "old packet was mutated in place"
        );
    }
}

fn sample(name: &str) -> Fixture {
    let f = Fixture::new(name);
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../samples/spine374_project");
    copy_tree(&source, &f.source);
    f
}
fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        assert!(!entry.file_type().unwrap().is_symlink());
        let out = destination.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &out);
        } else {
            fs::copy(path, out).unwrap();
        }
    }
}
#[test]
fn spine374_e_ordinary_sample_prepares_both_routes_and_deform() {
    let f = sample("e-sample-check");
    let mut session = ProjectAuthoringSession::open(&f.source).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("spine374-e-source", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    // RuntimeModule is compiled once by the explicit delivery consumer below;
    // this cheap consumer validates authoring/Cook without a second Rust build.
    let prepared = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
    let registry = &prepared
        .runtime_package_build_input()
        .spine_animator
        .as_ref()
        .unwrap()
        .registry;
    assert_eq!(registry.controllers.len(), 2);
    assert_eq!(registry.assets.len(), 1);
    assert!(registry.assets[0].skins.iter().any(|s| s == "festival"));
    let root = f.package(prepared.runtime_package_build_input());
    let package = load_runtime_package(&root).value.unwrap();
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .expect("ordinary scene must hydrate before Player build");
    let mut loader =
        engine_runtime::runtime_instance_loader::RuntimeInstanceLoader::from_package(&package);
    let mut host = EngineHostLoop::new("main");
    host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_unscaled_delta_time(1. / 60.)
            .with_fixed_step_count(1)
            .with_deferred_render_submission(),
        &mut world,
        RuntimeFrameContext::new(&package, &mut loader),
    );
    let projected = host.retained_geometry_snapshot().unwrap();
    assert_eq!(projected.prepared_geometries().count(), 2);
    assert_eq!(projected.views.len(), 1);
    assert_eq!(
        projected.views[0]
            .camera
            .view
            .source_camera_entity_id
            .as_deref(),
        Some("camera")
    );
    let features = engine_runtime::renderer_feature_builder::RendererFeatureBuilder::new()
        .build(1, host.render_scene());
    let sprites = engine_runtime::sprite2d_render_pipeline::Sprite2DRenderPipeline::new().build(
        1,
        &features,
        host.render_scene().views().next(),
    );
    assert_eq!(
        sprites.counters.draw_plan_count, 2,
        "socket FX must produce real sprite draws"
    );
    let source = registry.validated_asset(&root, "hero").unwrap();
    use engine_runtime::spine_animation::control::Intent;
    let mut lower = Vec::new();
    for definition in &registry.controllers {
        let table = definition
            .compile(
                &source,
                registry.tick_rate,
                registry.assets[0].pixels_per_unit,
            )
            .unwrap();
        let mut controller = table.instance().unwrap();
        let action = |name: &str| {
            definition
                .actions
                .iter()
                .position(|a| a.name == name)
                .unwrap() as u16
        };
        controller
            .advance(1, &[Intent::Play(action("run"))])
            .unwrap();
        let setup = controller.geometry().unwrap();
        let attack_name = if definition.asset_id == "local-controller" {
            "localAttack"
        } else {
            "maskedAttack"
        };
        controller
            .advance(2, &[Intent::Play(action(attack_name))])
            .unwrap();
        controller.advance(17, &[]).unwrap();
        let attack = controller.geometry().unwrap();
        // Rigid rotation preserves edge lengths. A changed cloth edge proves
        // the authored deform reaches actual backend vertices on both routes.
        let edge = |g: &engine_runtime::spine_animation::control::Controller| {
            let v = g.geometry().unwrap().vertices;
            (v[1].x - v[0].x).powi(2) + (v[1].y - v[0].y).powi(2)
        };
        let original_edge = (setup.vertices[1].x - setup.vertices[0].x).powi(2)
            + (setup.vertices[1].y - setup.vertices[0].y).powi(2);
        assert!(
            (edge(&controller) - original_edge).abs() > 0.1,
            "deform was lost"
        );
        assert!(controller.bone("upper").unwrap().rotation.abs() > 15.);
        lower.push(controller.bone("lower").unwrap().x);
        controller
            .advance_with_skin(18, &[], Some("festival"))
            .unwrap();
        let festival = controller.geometry().unwrap();
        assert_ne!(
            [
                attack.vertices[0].r,
                attack.vertices[0].g,
                attack.vertices[0].b
            ],
            [
                festival.vertices[0].r,
                festival.vertices[0].g,
                festival.vertices[0].b
            ],
            "skin attachment did not change the actual tint"
        );
    }
    assert!(
        (lower[0] - lower[1]).abs() < 0.0001 && lower.iter().all(|x| x.abs() < 1.),
        "masked full-body source displaced the lower body"
    );
}

#[test]
#[ignore = "explicit Gate E ordinary-project dev build, real window and source-free copied delivery"]
fn spine374_e_ordinary_project_build_playtest_verify_and_source_free_copy() {
    let f = sample("e-project-delivery");
    let mut session = ProjectAuthoringSession::open(&f.source).unwrap();
    let paths = session
        .source_inventory()
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    let lease = session
        .acquire_snapshot_lease("spine374-e-delivery", paths)
        .unwrap();
    let compiler = GameProjectCompiler::bind(&lease).unwrap();
    let started = Instant::now();
    let prepared = compiler
        .prepare_with_artifact_cache(&lease, TargetProfile::WindowsDev, &f.root.join("cache"))
        .unwrap();
    let prepare_ms = started.elapsed().as_millis();
    let scenario = prepared.load_playtest_scenario().unwrap();
    let started = Instant::now();
    let built = compiler
        .build(
            &prepared,
            BuildRequest::for_project(
                TargetProfile::WindowsDev,
                &f.source,
                ProjectRelativePath::parse("Build/Playable").unwrap(),
                project_authoring_execution::default_project_runtime_player_build_root(),
            )
            .with_player_verification(false)
            .with_timeout_ms(1_200_000),
        )
        .unwrap();
    let build_ms = started.elapsed().as_millis();
    fs::write(
        f.root.join("build.json"),
        serde_json::to_vec_pretty(built.desktop_export().unwrap()).unwrap(),
    )
    .unwrap();
    let started = Instant::now();
    let playtest = compiler
        .playtest_delivery(built.delivery(), &scenario, f.root.join("playtest"))
        .unwrap();
    let player_ms = started.elapsed().as_millis();
    fs::write(
        f.root.join("playtest.json"),
        serde_json::to_vec_pretty(playtest.process()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(playtest.process()).unwrap()["overall"],
        "passed",
        "{playtest:#?}"
    );
    assert_fx_capture(&f.root.join("playtest/capture-45.png"));
    assert_fx_capture(&f.root.join("playtest/capture-145.png"));
    let verified = compiler
        .verify(
            built.delivery(),
            VerifyRequest::new(ExecutionMode::Windowed, 45, 60_000, true),
        )
        .unwrap();
    fs::write(f.root.join("verify.txt"), format!("{verified:#?}")).unwrap();
    assert_eq!(
        serde_json::to_value(verified.process_verification()).unwrap()["status"],
        "passed"
    );
    let standalone = f.root.join("standalone");
    copy_tree(built.delivery().package_dir(), &standalone);
    // Only this test's temporary project is moved. Existing projects are untouched.
    fs::rename(&f.source, f.root.join("source-offline")).unwrap();
    assert!(!standalone.join("RuntimeModule").exists());
    assert!(!standalone.join("Scenes").exists());
    runtime_cli::validate_desktop_dev_package(&standalone).unwrap();
    let started = Instant::now();
    let independent =
        runtime_cli::run_bounded_semantic_playtest(runtime_cli::SemanticPlaytestProcessRequest {
            player_executable: standalone.join("Game.exe"),
            runtime_package: standalone.join("data/runtime_package"),
            scenario: scenario.scenario().clone(),
            output_dir: f.root.join("independent-playtest"),
        })
        .unwrap();
    let independent_ms = started.elapsed().as_millis();
    fs::write(
        f.root.join("independent.json"),
        serde_json::to_vec_pretty(&independent).unwrap(),
    )
    .unwrap();
    fs::write(f.root.join("timings.json"),serde_json::to_vec_pretty(&json!({"prepareMs":prepare_ms,"buildMs":build_ms,
        "playerMs":player_ms,"independentPlayerMs":independent_ms,"profile":"WindowsDev","releaseQualification":false})).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&independent).unwrap()["overall"],
        "passed",
        "{independent:#?}"
    );
    assert_fx_capture(&f.root.join("independent-playtest/capture-45.png"));
    assert_fx_capture(&f.root.join("independent-playtest/capture-145.png"));
}

fn assert_fx_capture(path: &std::path::Path) {
    let mut reader = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).unwrap()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let mut gold = [0usize; 2];
    for (i, pixel) in pixels[..info.buffer_size()].chunks_exact(4).enumerate() {
        if pixel[0] > 200 && pixel[1] > 150 && pixel[2] < 100 && pixel[3] > 0 {
            gold[usize::from((i % info.width as usize) >= info.width as usize / 2)] += 1;
        }
    }
    assert!(
        gold.iter().all(|n| *n >= 64),
        "socket FX absent from actual GPU capture {}: {gold:?}",
        path.display()
    );
}
