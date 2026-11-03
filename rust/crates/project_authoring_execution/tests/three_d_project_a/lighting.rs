//! Ordinary source project -> Compiler Prepare -> package -> World -> production frame.
use super::*;
use engine_runtime::{
    archetype::ComponentValue, render_extract::RenderExtractContext, render_state::RenderSceneState,
};
fn fixture() -> Fixture {
    let f = Fixture::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/platform_3d_lighting_project");
    for folder in ["Assets", "Models", "Scenes", "Input"] {
        fs::create_dir_all(f.root.join(folder)).unwrap();
        for entry in fs::read_dir(root.join(folder)).unwrap() {
            let e = entry.unwrap();
            assert!(e.file_type().unwrap().is_file());
            fs::copy(e.path(), f.root.join(folder).join(e.file_name())).unwrap();
        }
    }
    fs::copy(
        root.join("project.aife.json"),
        f.root.join("project.aife.json"),
    )
    .unwrap();
    f
}
#[test]
fn s2b_source_package_world_projection_updates_and_rejects_invalid_mutation() {
    let f = fixture();
    let (_, lease) = f.compiler();
    let report = inspect_spatial_project_snapshot(lease.snapshot());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(
        report.scenes[0]
            .entities
            .iter()
            .filter(|e| e.light.is_some())
            .count(),
        3
    );
    assert!(report.supported_schemas.contains(&"light3d.v1"));
    let _ = lease.release();
    let input = generated::input(&f);
    let (package, _) = generated::package(&f, &input, "light-package");
    let mut world = engine_runtime::scene_loader::load_scene_into_world(&package.active_scene)
        .value
        .unwrap();
    let mut extract = RenderExtractContext::new();
    let mut scene = RenderSceneState::new();
    world.prepare_spatial_presentation(1, 1.).unwrap();
    let id = extract
        .project_main_camera(&world, &mut scene, [800, 480])
        .unwrap()
        .unwrap();
    let old = scene.view(id).unwrap().spatial.clone().unwrap();
    assert_eq!(old.lights.len(), 3);
    assert!(old
        .lights
        .iter()
        .any(|l| matches!(l.component.kind, LightKind::Spot { .. })));
    let mut changed = world.light3d(&"sun".into()).unwrap().clone();
    changed.color = [0., 1., 0.];
    world
        .try_insert_component_value(
            "sun".into(),
            engine_runtime::components::ComponentTypeId::light3d(),
            ComponentValue::Light3D(changed.clone()),
        )
        .unwrap();
    extract
        .project_main_camera(&world, &mut scene, [800, 480])
        .unwrap();
    let next = scene.view(id).unwrap().spatial.clone().unwrap();
    assert_ne!(next.lights, old.lights);
    assert!(next.view.version > old.view.version);
    changed.color[0] = -1.;
    assert!(world
        .try_insert_component_value(
            "sun".into(),
            engine_runtime::components::ComponentTypeId::light3d(),
            ComponentValue::Light3D(changed)
        )
        .is_err());
    assert_eq!(world.light3d(&"sun".into()).unwrap().color, [0., 1., 0.]);
    let mut disabled = world.light3d(&"point".into()).unwrap().clone();
    disabled.enabled = false;
    world
        .try_insert_component_value(
            "point".into(),
            engine_runtime::components::ComponentTypeId::light3d(),
            ComponentValue::Light3D(disabled),
        )
        .unwrap();
    extract
        .project_main_camera(&world, &mut scene, [800, 480])
        .unwrap();
    assert_eq!(
        scene
            .view(id)
            .unwrap()
            .spatial
            .as_ref()
            .unwrap()
            .lights
            .len(),
        2
    );
    let mut source: Value =
        serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
    source["entities"][2]["components"][1]["data"]["kind"]["illuminanceLux"] = json!(-1);
    f.write("Scenes/main.json", &source);
    let e = f.check_error();
    assert!(e.code().contains("light_value_invalid"), "{e:?}");
    assert!(e
        .source_location()
        .unwrap()
        .field_path
        .as_ref()
        .unwrap()
        .contains("/components/1/data"));
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2b_gpu_ordinary_project_lights_reach_production_packets_and_pixels() {
    production_lights(false);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "S2-D BC3 ordinary project and real scene lights"]
fn s2d_bc3_gpu_ordinary_project_reaches_production_lights_and_old_packet_replay() {
    production_lights(true);
}

#[cfg(feature = "real-wgpu")]
fn production_lights(compressed: bool) {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        rhi_command_plan::RhiCommand,
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let started = std::time::Instant::now();
    let f = fixture();
    if compressed {
        let path = f.root.join("Assets/import.json");
        let mut descriptor: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        descriptor["textureCompression"] = json!("bc3");
        f.write("Assets/import.json", &descriptor);
        // Source fixture uses 2x2 maps. Replicate texels into a supported 4x4
        // source without changing the UV image; this is not a scale benchmark.
        for i in 0..5 {
            let path = f.root.join(format!("Models/pbr-{i}.png"));
            let image = image::load_from_memory(&fs::read(&path).unwrap())
                .unwrap()
                .to_rgba8();
            let mut rgba = Vec::new();
            for y in 0..4 {
                for x in 0..4 {
                    rgba.extend_from_slice(&image.get_pixel(x / 2, y / 2).0);
                }
            }
            let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), 4, 4);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&rgba)
                .unwrap();
        }
    }
    let input = generated::input(&f);
    if compressed {
        assert_eq!(input.texture_payloads.len(), 5);
        assert!(input
            .texture_payloads
            .iter()
            .all(|t| t.metadata.format.starts_with("bc3") && t.metadata.mip_count == 3));
    }
    let cook = started.elapsed();
    let (package, _) = generated::package(&f, &input, "production-light-package");
    let pack = started.elapsed() - cook;
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let report = hydrator.hydrate_active_scene(&package, &mut world);
    assert!(!report.has_errors(), "{report:?}");
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(GameViewTargetSpec::new(
        800,
        480,
        GameViewScalePolicy::Stretch,
    ));
    let tick = |host: &mut EngineHostLoop,
                world: &mut engine_runtime::world::World,
                hydrator: &mut RuntimeSceneHydrator| {
        host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap()
    };
    let first = tick(&mut host, &mut world, &mut hydrator);
    assert_eq!(first.report.rhi_status, "ok", "{:?}", first.report);
    let draws: Vec<_> = first
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .filter_map(RhiCommand::geometry_draw)
        .collect();
    assert_eq!(
        first
            .renderer_output
            .rhi_command_plan
            .commands
            .iter()
            .map(RhiCommand::geometry_instance_count)
            .sum::<usize>(),
        6
    );
    for d in draws {
        assert_eq!(d.lighting.as_ref().unwrap().light_count(), 3);
    }
    let ready = started.elapsed();
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    eprintln!("S2-B adapter {:?}", backend.device_context());
    backend
        .prepare_geometry_textures(&first.geometry_textures)
        .unwrap();
    let pixels = backend
        .render_plan_to_rgba_bytes(&first.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    assert!(
        pixels
            .chunks_exact(4)
            .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
            .count()
            > 20000
    );
    // All scene lights disabled: direct light disappears, authored emissive remains.
    let mut writer = project_game_sdk::DeferredMutationWriter::default();
    for name in ["sun", "point", "spot"] {
        let handle = project_game_sdk::EntityHandle::new(
            name,
            world.entity_generation(&name.into()).unwrap(),
        )
        .unwrap();
        writer.set_light_color(&handle, [0., 1., 0.]).unwrap();
        writer.set_light_intensity(&handle, 2.).unwrap();
        writer.set_light_enabled(&handle, false).unwrap();
    }
    let mut mutations =
        engine_runtime::project_runtime_session::ProjectRuntimeMutationBuffer::new();
    for mutation in writer.into_inner() {
        let wire = serde_json::to_vec(&mutation).unwrap();
        let decoded: project_game_sdk::DeferredMutation = serde_json::from_slice(&wire).unwrap();
        let project_game_sdk::DeferredMutation::Spatial {
            entity_id,
            generation,
            intent,
        } = decoded
        else {
            panic!("expected spatial light command")
        };
        mutations.spatial_command(entity_id.into(), generation, intent);
    }
    mutations
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();
    let dark = tick(&mut host, &mut world, &mut hydrator);
    let dark_pixels = backend
        .render_plan_to_rgba_bytes(&dark.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    assert_ne!(pixels, dark_pixels);
    assert!(
        pixels
            .chunks_exact(4)
            .zip(dark_pixels.chunks_exact(4))
            .filter(|(a, b)| a[..3] != b[..3])
            .count()
            > 20000
    );
    let replay = backend
        .render_plan_to_rgba_bytes(&first.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    assert_eq!(
        replay, pixels,
        "old immutable production packet keeps its authored lights"
    );
    if let Some(root) = std::env::var_os("AIFE_S2B_EVIDENCE_ROOT") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        for (name, data) in [("scene-lights", &pixels), ("scene-dark", &dark_pixels)] {
            let mut encoder = png::Encoder::new(
                fs::File::create(root.join(format!("{name}.png"))).unwrap(),
                800,
                480,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(data)
                .unwrap();
        }
        if compressed {
            let target = root.join("ordinary-project");
            for folder in ["Assets", "Models", "Scenes", "Input"] {
                fs::create_dir_all(target.join(folder)).unwrap();
                for entry in fs::read_dir(f.root.join(folder)).unwrap() {
                    let entry = entry.unwrap();
                    assert!(entry.file_type().unwrap().is_file());
                    fs::copy(entry.path(), target.join(folder).join(entry.file_name())).unwrap();
                }
            }
            fs::copy(
                f.root.join("project.aife.json"),
                target.join("project.aife.json"),
            )
            .unwrap();
        }
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    eprintln!("S2-B costs cook_ms={:.3} package_ms={:.3} hydrate_host_ms={:.3} gpu_adapter_draws_readback_ms={:.3}",cook.as_secs_f64()*1000.,pack.as_secs_f64()*1000.,(ready-cook-pack).as_secs_f64()*1000.,(started.elapsed()-ready).as_secs_f64()*1000.);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2b_gpu_ordinary_production_two_camera_layers_and_resize_have_separate_light_pixels() {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        render_state::{RenderTargetKind, RenderViewKind, Viewport},
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let f = fixture();
    let mut scene: Value =
        serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
    scene["entities"][0]["components"][2]["data"]["layers"] = json!(["world", "aux"]);
    scene["entities"][1]["components"][1]["data"]["projection"]["size"] = 6.into();
    let mut auxiliary = scene["entities"][1].clone();
    auxiliary["id"] = "aux".into();
    auxiliary["name"] = "Auxiliary".into();
    auxiliary["components"][1]["data"]["layers"] = json!(["aux"]);
    for entity in scene["entities"].as_array_mut().unwrap().iter_mut().skip(2) {
        let id = entity["id"].as_str().unwrap().to_owned();
        let data = &mut entity["components"][1]["data"];
        match id.as_str() {
            "sun" => {
                data["color"] = json!([1, 0, 0]);
            }
            "point" => {
                data["color"] = json!([0, 0, 1]);
                data["kind"]["intensityCandela"] = 200.into();
                data["layers"] = json!(["aux"]);
            }
            "spot" => data["enabled"] = false.into(),
            _ => unreachable!(),
        }
    }
    scene["entities"].as_array_mut().unwrap().push(auxiliary);
    f.write("Scenes/main.json", &scene);
    let input = generated::input(&f);
    let (package, _) = generated::package(&f, &input, "dual-light-package");
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let mut host = EngineHostLoop::new("main");
    let mut backend = RealWgpuBackend::new_offscreen(512, 256).unwrap();
    for [width, height] in [[512, 256], [768, 384], [512, 256]] {
        host.set_game_view_target(GameViewTargetSpec::new(
            width,
            height,
            GameViewScalePolicy::Stretch,
        ));
        host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        );
        host.project_auxiliary_camera_view(
            &world,
            &"aux".into(),
            Viewport {
                x: width,
                y: 0,
                width,
                height,
            },
            RenderViewKind::Preview,
            RenderTargetKind::ViewportTexture,
        )
        .unwrap();
        let frame = host
            .tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                &mut world,
                RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
            )
            .render_thread_frame
            .unwrap();
        assert_eq!(frame.report.rhi_status, "ok", "{:?}", frame.report);
        let snapshot = host.retained_geometry_snapshot().unwrap();
        assert_eq!(snapshot.views.len(), 2);
        assert!(snapshot.views.iter().all(|v| v.camera.lights.len() == 1));
        let mut viewports = frame
            .renderer_output
            .rhi_command_plan
            .commands
            .iter()
            .filter_map(|c| match c {
                engine_runtime::rhi_command_plan::RhiCommand::BeginView { view, .. } => {
                    Some(view.viewport)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        viewports.sort();
        assert_eq!(
            viewports,
            vec![[0, 0, width / 2, height], [width / 2, 0, width / 2, height]],
            "production must normalize the adjacent camera canvas without overlap"
        );
        backend
            .prepare_geometry_textures(&frame.geometry_textures)
            .unwrap();
        let pixels = backend
            .render_plan_to_rgba_bytes(&frame.renderer_output.rhi_command_plan, width, height)
            .unwrap();
        let y = (height as f32 / 2. - 1.05 * height as f32 / 6.) as u32;
        let red_x = (width as f32 / 4. - 2.3 * height as f32 / 6.) as u32;
        for (x, channel, threshold) in [(red_x, 0, 20), (width * 3 / 4, 2, 5)] {
            let i = ((y * width + x) * 4) as usize;
            let pixel = &pixels[i..i + 4];
            assert!(
                pixel[channel] > threshold,
                "{width}x{height} pixel({x},{y})={pixel:?}"
            );
            for other in 0..3 {
                if other != channel {
                    assert_eq!(pixel[other], 0, "camera light layer leaked: {pixel:?}");
                }
            }
        }
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
