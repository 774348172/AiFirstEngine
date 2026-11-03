use super::*;

// Direct renderer tests use an empty project module. The real sample module,
// observation contract and AUI are verified by the exported Player scenario.
fn presentation_fixture() -> Fixture {
    let fixture = shadows::fixture_at("platform_3d_presentation_project");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("project.aife.json")).unwrap()).unwrap();
    manifest["runtimeModule"] = serde_json::to_value(
        project_authoring_execution::ProjectRuntimeModuleBuildSpec::explicit_empty(),
    )
    .unwrap();
    manifest
        .as_object_mut()
        .unwrap()
        .remove("observationContract");
    manifest.as_object_mut().unwrap().remove("playtestScenario");
    fixture.write("project.aife.json", &manifest);
    fixture
}

// Keep the independent particle/decal numerical oracle isolated from the gallery
// witnesses. The complete source is exercised separately by the combination test.
#[cfg(feature = "real-wgpu")]
fn particle_oracle_fixture() -> Fixture {
    let fixture = presentation_fixture();
    let mut scene: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("Scenes/main.json")).unwrap()).unwrap();
    scene["entities"].as_array_mut().unwrap().retain(|entity| {
        !matches!(
            entity["id"].as_str(),
            Some("lit-reference" | "lit-rough-reference" | "shadow-receiver" | "shadow-caster")
        )
    });
    for entity in scene["entities"].as_array_mut().unwrap() {
        for component in entity["components"].as_array_mut().unwrap() {
            if component["componentType"] == "Light3D" {
                component["data"]["castShadows"] = false.into();
            }
            if component["componentType"] == "Camera3D" {
                let color = component["data"]["colorPipeline"].as_object_mut().unwrap();
                color.remove("ibl");
                // Main-view numerical references below explicitly use no fog;
                // the auxiliary-view tests add and independently calculate it.
                color.remove("fog");
            }
        }
    }
    fixture.write("Scenes/main.json", &scene);
    fixture
}

#[test]
fn presentation_source_decal_and_spatial_particle_references_are_strict() {
    let f = presentation_fixture();
    let input = generated::input(&f);
    // The combined gallery must reach Cook with sampled PBR inputs, rather than
    // accepting source fields which disappear before the production consumer.
    use engine_runtime::render_contract::{
        CookedRenderAsset, MaterialPreset, RenderAssetData, RenderContract,
    };
    let gold = input
        .assets
        .iter()
        .find(|asset| asset.asset_id == "lit-gold")
        .unwrap();
    let cooked = RenderContract::<CookedRenderAsset>::decode(
        std::str::from_utf8(gold.runtime_payload.as_ref().unwrap()).unwrap(),
    )
    .unwrap();
    let RenderAssetData::Material(material) = cooked.data.asset else {
        panic!("gallery reference must Cook to a material");
    };
    assert_eq!(material.preset, MaterialPreset::StandardLit);
    assert_eq!(
        material
            .textures
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "baseColorTexture",
            "emissiveTexture",
            "metallicRoughnessTexture",
            "normalTexture",
            "occlusionTexture",
        ]
    );
    assert!(input
        .assets
        .iter()
        .any(|asset| asset.asset_id == "studio" && asset.asset_type == "environment"));
    for effect in input
        .assets
        .iter()
        .filter(|a| a.asset_type == "particle-effect")
    {
        let cooked: engine_runtime::particle_effect::CookedParticleEffect =
            serde_json::from_slice(effect.runtime_payload.as_ref().unwrap()).unwrap();
        assert_eq!(cooked.program_contract, "particle-program.v2");
        assert_eq!(
            input
                .assets
                .iter()
                .filter(|a| a.asset_id == effect.asset_id)
                .count(),
            1,
            "particle cooker must be the sole producer of {}",
            effect.asset_id
        );
    }
    assert_eq!(
        input
            .assets
            .iter()
            .filter(|a| a.asset_type == "particle-effect")
            .count(),
        3
    );
    let mut material: Value =
        serde_json::from_slice(&fs::read(f.root.join("Assets/decal.json")).unwrap()).unwrap();
    for bad in [json!(0), json!(-17), json!(1), json!(-1.5)] {
        material["decalBias"] = bad;
        assert!(decode::<MaterialAssetV2>(material.clone(), "")
            .and_then(|m| m.validate())
            .is_err());
    }
    let mut value: Value =
        serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
    let mut missing_camera = value.clone();
    missing_camera["mainCamera"] = Value::Null;
    assert!(decode::<SpatialSourceScene>(missing_camera, "")
        .and_then(|s| s.canonicalize())
        .is_err());
    let entity = value["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == "world-sparks")
        .unwrap();
    entity["components"][1]["data"]["unexpected"] = true.into();
    assert!(decode::<SpatialSourceScene>(value, "")
        .and_then(|s| s.canonicalize())
        .is_err());
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2d_gpu_combined_gallery_samples_ibl_and_shadows_without_losing_depth_effects() {
    use engine_runtime::{
        archetype::ComponentValue,
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        rhi_command_plan::{RhiCommand, RhiDrawPayload},
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };

    let fixture = presentation_fixture();
    let input = generated::input(&fixture);
    let (package, _) = generated::package(&fixture, &input, "combined-gallery");
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let hydration = hydrator.hydrate_active_scene(&package, &mut world);
    assert!(!hydration.has_errors(), "{hydration:?}");
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(GameViewTargetSpec::new(
        800,
        480,
        GameViewScalePolicy::Stretch,
    ));
    let tick = |host: &mut EngineHostLoop,
                world: &mut engine_runtime::world::World,
                hydrator: &mut RuntimeSceneHydrator| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let frame = host
                .tick_with_runtime_context(
                    EngineFrameInput::new(EngineHostMode::ExportedGame),
                    world,
                    RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
                )
                .render_thread_frame
                .unwrap();
            if frame.report.rdg_status != "ok" {
                assert!(
                    !frame.report.diagnostics.is_empty()
                        && frame.report.diagnostics.iter().all(|d| {
                            d.code == "particle_render.unsupported_backend"
                                || (d.code == "three_d.production_frame_failed"
                                    && d.message
                                        == "ResourceNotReady at environment.async (ContractValidation)")
                        }),
                    "{:?}",
                    frame.report.diagnostics
                );
                assert!(std::time::Instant::now() < deadline, "IBL not ready");
                std::thread::yield_now();
                continue;
            }
            assert_eq!(frame.report.rdg_status, "ok", "{:?}", frame.report);
            assert!(
                frame
                    .report
                    .diagnostics
                    .iter()
                    .all(|d| d.code == "particle_render.unsupported_backend"),
                "{:?}",
                frame.report
            );
            let plan = &frame.renderer_output.rhi_command_plan;
            let lit = plan
                .commands
                .iter()
                .filter(|c| {
                    c.geometry_draw()
                        .is_some_and(|d| !d.view.is_shadow_map() && d.standard_lit.is_some())
                })
                .map(RhiCommand::geometry_instance_count)
                .sum::<usize>();
            let particles = plan
                .commands
                .iter()
                .filter(|c| {
                    matches!(
                        c,
                        RhiCommand::Draw {
                            payload: RhiDrawPayload::Particles { .. },
                            ..
                        }
                    )
                })
                .count();
            if lit >= 4 && particles == 3 {
                break frame;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "combination not ready: lit={lit}, particles={particles}"
            );
            std::thread::yield_now();
        }
    };
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    let render = |backend: &mut RealWgpuBackend,
                  frame: &engine_runtime::render_thread::RenderThreadFrameOutput| {
        backend
            .prepare_geometry_textures(&frame.geometry_textures)
            .unwrap();
        backend
            .render_plan_to_rgba_bytes(&frame.renderer_output.rhi_command_plan, 800, 480)
            .unwrap()
    };
    let capture = |name: &str, pixels: &[u8]| {
        if let Some(root) = std::env::var_os("AIFE_S2D_COMPOSITION_CAPTURE_ROOT") {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).unwrap();
            let mut encoder =
                png::Encoder::new(fs::File::create(root.join(name)).unwrap(), 800, 480);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(pixels)
                .unwrap();
        }
    };
    let first = tick(&mut host, &mut world, &mut hydrator);
    assert!(first
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .any(|c| c.geometry_draw().is_some_and(|d| d.view.is_shadow_map())));
    let combined = render(&mut backend, &first);
    capture("combined.png", &combined);

    let mut sun = world.light3d(&"sun".into()).unwrap().clone();
    sun.cast_shadows = false;
    world
        .try_insert_component_value(
            "sun".into(),
            engine_runtime::components::ComponentTypeId::light3d(),
            ComponentValue::Light3D(sun.clone()),
        )
        .unwrap();
    let no_shadow_frame = tick(&mut host, &mut world, &mut hydrator);
    let no_shadow = render(&mut backend, &no_shadow_frame);
    capture("no-shadow.png", &no_shadow);
    // These lower regions contain static geometry, avoiding animated particle
    // differences as a false explanation of the light/IBL counterfactual.
    let changed = |a: &[u8], b: &[u8], xmin: usize, xmax: usize| {
        (380..470)
            .flat_map(|y| (xmin..xmax).map(move |x| (y * 800 + x) * 4))
            .filter(|&i| (0..3).any(|c| a[i + c].abs_diff(b[i + c]) > 2))
            .count()
    };
    assert!(
        changed(&combined, &no_shadow, 180, 620) > 10,
        "shadow setting was accepted but produced no receiver response"
    );
    let mut camera = world.camera3d(&"camera".into()).unwrap().clone();
    let original_ibl = camera.color_pipeline.as_ref().unwrap().ibl.clone();
    camera.color_pipeline.as_mut().unwrap().ibl = None;
    world
        .try_insert_component_value(
            "camera".into(),
            engine_runtime::components::ComponentTypeId::camera3d(),
            ComponentValue::Camera3D(camera.clone()),
        )
        .unwrap();
    let no_ibl_frame = tick(&mut host, &mut world, &mut hydrator);
    let no_ibl = render(&mut backend, &no_ibl_frame);
    capture("no-ibl.png", &no_ibl);
    assert!(
        changed(&no_shadow, &no_ibl, 20, 130) > 20,
        "five-slot metal did not consume the scene IBL"
    );
    sun.cast_shadows = true;
    world
        .try_insert_component_value(
            "sun".into(),
            engine_runtime::components::ComponentTypeId::light3d(),
            ComponentValue::Light3D(sun),
        )
        .unwrap();
    camera.color_pipeline.as_mut().unwrap().ibl = original_ibl;
    world
        .try_insert_component_value(
            "camera".into(),
            engine_runtime::components::ComponentTypeId::camera3d(),
            ComponentValue::Camera3D(camera),
        )
        .unwrap();
    let restored_frame = tick(&mut host, &mut world, &mut hydrator);
    let restored = render(&mut backend, &restored_frame);
    capture("restored.png", &restored);
    assert_eq!(changed(&combined, &restored, 20, 780), 0);

    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2c_gpu_ordinary_project_parent_rotation_nonuniform_scale_world_local_and_retirement() {
    use engine_runtime::{
        archetype::ComponentValue,
        engine_host_loop::*,
        frame_loop::RuntimeFrameContext,
        game_view_presentation::*,
        rhi_command_plan::{RhiCommand, RhiDrawPayload},
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let started = std::time::Instant::now();
    let f = particle_oracle_fixture();
    let mut scene: Value =
        serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
    let root = scene["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == "emitter-root")
        .unwrap();
    root["components"][0]["data"]["rotation"] = json!({"kind":"eulerDegrees","xyz":[0,0,45]});
    root["components"][0]["data"]["scale"] = json!([1.2, 0.7, 1]);
    f.write("Scenes/main.json", &scene);
    let input = generated::input(&f);
    let cook = started.elapsed();
    let (package, _) = generated::package(&f, &input, "trs-package");
    let pack = started.elapsed() - cook;
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(GameViewTargetSpec::new(
        800,
        480,
        GameViewScalePolicy::Stretch,
    ));
    let tick = |host: &mut EngineHostLoop,
                world: &mut engine_runtime::world::World,
                hydrator: &mut RuntimeSceneHydrator| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let frame = host
                .tick_with_runtime_context(
                    EngineFrameInput::new(EngineHostMode::ExportedGame),
                    world,
                    RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
                )
                .render_thread_frame
                .unwrap();
            assert_eq!(frame.report.rdg_status, "ok", "{:?}", frame.report);
            assert!(
                frame
                    .report
                    .diagnostics
                    .iter()
                    .all(|d| d.code == "particle_render.unsupported_backend"),
                "{:?}",
                frame.report
            );
            let draws = frame
                .renderer_output
                .rhi_command_plan
                .commands
                .iter()
                .filter(|c| {
                    matches!(
                        c,
                        RhiCommand::Draw {
                            payload: RhiDrawPayload::Particles { .. },
                            ..
                        }
                    )
                })
                .count();
            if draws == 3 {
                break frame;
            }
            assert!(std::time::Instant::now() < deadline, "particles not ready");
            std::thread::yield_now();
        }
    };
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    let render = |backend: &mut RealWgpuBackend,
                  frame: &engine_runtime::render_thread::RenderThreadFrameOutput| {
        backend
            .prepare_geometry_textures(&frame.geometry_textures)
            .unwrap();
        backend
            .render_plan_to_rgba_bytes(&frame.renderer_output.rhi_command_plan, 800, 480)
            .unwrap()
    };
    let first = tick(&mut host, &mut world, &mut hydrator);
    let before = render(&mut backend, &first);
    let pixel = |image: &[u8], x: usize, y: usize| -> [u8; 4] {
        image[(y * 800 + x) * 4..][..4].try_into().unwrap()
    };
    // Independent Rz(45)*S: emitter positions (+/-1,0) -> (+/-1.2/sqrt(2), +/-1.2/sqrt(2)).
    let offset = 1.2f64 / 2f64.sqrt();
    let world_pixel = (
        (400. - 120. * offset).floor() as usize,
        (240. - 120. * (0.7 - offset)).floor() as usize,
    );
    let local_pixel = (
        (400. + 120. * offset).floor() as usize,
        (240. - 120. * (0.7 + offset)).floor() as usize,
    );
    assert!(pixel(&before, world_pixel.0, world_pixel.1)[0] > 200);
    assert!(pixel(&before, local_pixel.0, local_pixel.1)[1] > 200);
    let id = "emitter-root".into();
    let mut t = world.transform3d(&id).unwrap().clone();
    t.translation[0] = 0.3;
    t.rotation_xyzw = [
        0.,
        0.,
        -(std::f32::consts::PI / 8.).sin(),
        (std::f32::consts::PI / 8.).cos(),
    ];
    t.scale = [0.6, 1.4, 1.];
    world
        .try_insert_component_value(
            id,
            engine_runtime::components::ComponentTypeId::transform3d(),
            ComponentValue::Transform3D(t),
        )
        .unwrap();
    let second = tick(&mut host, &mut world, &mut hydrator);
    let after = render(&mut backend, &second);
    assert_eq!(
        pixel(&after, world_pixel.0, world_pixel.1),
        pixel(&before, world_pixel.0, world_pixel.1),
        "world birth transform changed"
    );
    let moved = (
        (400. + 120. * (0.3 + 0.6 / 2f64.sqrt())).floor() as usize,
        (240. - 120. * (0.7 - 0.6 / 2f64.sqrt())).floor() as usize,
    );
    assert!(
        pixel(&after, moved.0, moved.1)[1] > 200,
        "local parent TRS ignored"
    );
    assert!(
        pixel(&after, local_pixel.0, local_pixel.1)[1] < 100,
        "local particle remained at old parent pose"
    );
    assert!(backend
        .render_plan_to_rgba_bytes(&first.renderer_output.rhi_command_plan, 800, 480)
        .unwrap_err()
        .contains("stale_projection"));
    if let Some(root) = std::env::var_os("AIFE_S2C_PRESENTATION_CAPTURE_ROOT") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        for (name, image) in [
            ("presentation-trs-before.png", &before),
            ("presentation-trs-after.png", &after),
        ] {
            let mut e = png::Encoder::new(fs::File::create(root.join(name)).unwrap(), 800, 480);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(image).unwrap();
        }
    }
    for id in ["world-sparks", "local-sparks", "hidden-sparks"] {
        world.try_despawn_entity(&id.into()).unwrap();
    }
    let end = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    render(&mut backend, &end);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    eprintln!("ordinary TRS Cook={cook:?} package={pack:?} total={:?}; world={world_pixel:?} local={local_pixel:?} moved={moved:?}",started.elapsed());
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2c_gpu_ordinary_project_decal_particles_parent_motion_and_retirement() {
    presentation_consumer(false, false);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2c_gpu_ordinary_project_particles_independent_fog_output() {
    presentation_consumer(true, false);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2c_gpu_ordinary_project_shared_split_output() {
    presentation_consumer(true, true);
}

#[cfg(feature = "real-wgpu")]
fn presentation_consumer(multiview: bool, split: bool) {
    use engine_runtime::{
        archetype::ComponentValue,
        engine_host_loop::*,
        frame_loop::RuntimeFrameContext,
        game_view_presentation::*,
        rhi_command_plan::{RhiCommand, RhiDrawPayload},
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let started = std::time::Instant::now();
    let f = particle_oracle_fixture();
    if multiview {
        let mut scene: Value =
            serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
        let mut aux = scene["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "camera")
            .unwrap()
            .clone();
        aux["id"] = "fog-camera".into();
        aux["name"] = "fog-camera".into();
        let camera = &mut aux["components"][1]["data"]["colorPipeline"];
        camera["exposureEv"] = (-1).into();
        camera["fog"] = json!({"color":[0,1,0],"densityPerM":0.5,"startDistanceM":0});
        if split {
            let mut right = aux.clone();
            right["id"] = "split-right-camera".into();
            right["name"] = "split-right-camera".into();
            right["components"][1]["data"]["colorPipeline"]["exposureEv"] = 1.into();
            right["components"][1]["data"]["colorPipeline"]["fog"]["color"] = json!([1, 0, 0]);
            scene["entities"].as_array_mut().unwrap().push(right);
        }
        scene["entities"].as_array_mut().unwrap().push(aux);
        f.write("Scenes/main.json", &scene);
    }
    let input = generated::input(&f);
    let cook = started.elapsed();
    let (package, _) = generated::package(&f, &input, "presentation-package");
    let pack = started.elapsed() - cook;
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let h = hydrator.hydrate_active_scene(&package, &mut world);
    assert!(!h.has_errors(), "{h:?}");
    assert!(world.particle_effect(&"world-sparks".into()).is_some());
    let mut host = EngineHostLoop::new("main");
    host.set_game_view_target(GameViewTargetSpec::new(
        800,
        480,
        GameViewScalePolicy::Stretch,
    ));
    if multiview {
        // Evaluate the authored camera through the same World's projection owner first.
        host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        );
        let auxiliary = host
            .project_auxiliary_camera_view(
                &world,
                &"fog-camera".into(),
                engine_runtime::render_state::Viewport {
                    x: 0,
                    y: 0,
                    width: if split { 400 } else { 800 },
                    height: 480,
                },
                engine_runtime::render_state::RenderViewKind::Preview,
                engine_runtime::render_state::RenderTargetKind::ViewportTexture,
            )
            .unwrap();
        let mut target = engine_runtime::runtime_renderer::RenderTarget::viewport_texture(
            "particle-fog-preview",
            800,
            480,
        );
        target.format = "Rgba8Unorm".into();
        host.set_geometry_view_target(auxiliary, Some(target.clone()))
            .unwrap();
        if split {
            let right = host
                .project_auxiliary_camera_view(
                    &world,
                    &"split-right-camera".into(),
                    engine_runtime::render_state::Viewport {
                        x: 400,
                        y: 0,
                        width: 400,
                        height: 480,
                    },
                    engine_runtime::render_state::RenderViewKind::Preview,
                    engine_runtime::render_state::RenderTargetKind::ViewportTexture,
                )
                .unwrap();
            host.set_geometry_view_target(right, Some(target)).unwrap();
        }
    }
    let tick = |host: &mut EngineHostLoop,
                world: &mut engine_runtime::world::World,
                hydrator: &mut RuntimeSceneHydrator| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let frame = host
                .tick_with_runtime_context(
                    EngineFrameInput::new(EngineHostMode::ExportedGame),
                    world,
                    RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
                )
                .render_thread_frame
                .unwrap();
            assert_eq!(frame.report.rdg_status, "ok", "{:?}", frame.report);
            // This Host records intent; the native backend below executes the same plan.
            // Its deliberate refusal to pretend to execute GPU particles must remain.
            assert!(
                frame
                    .report
                    .diagnostics
                    .iter()
                    .all(|d| d.code == "particle_render.unsupported_backend"),
                "{:?}",
                frame.report
            );
            let p = &frame.renderer_output.rhi_command_plan;
            let meshes = p
                .commands
                .iter()
                .map(RhiCommand::geometry_instance_count)
                .sum::<usize>();
            let particles = p
                .commands
                .iter()
                .filter(|c| {
                    matches!(
                        c,
                        RhiCommand::Draw {
                            payload: RhiDrawPayload::Particles { .. },
                            ..
                        }
                    )
                })
                .count();
            if meshes
                == (if split {
                    6
                } else if multiview {
                    4
                } else {
                    2
                })
                && particles
                    == (if split {
                        9
                    } else if multiview {
                        6
                    } else {
                        3
                    })
            {
                break frame;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "not ready: meshes={meshes} particles={particles}"
            );
            std::thread::yield_now();
        }
    };
    let first = tick(&mut host, &mut world, &mut hydrator);
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    let render = |backend: &mut RealWgpuBackend,
                  frame: &engine_runtime::render_thread::RenderThreadFrameOutput| {
        backend
            .prepare_geometry_textures(&frame.geometry_textures)
            .unwrap();
        backend
            .render_plan_to_rgba_bytes(&frame.renderer_output.rhi_command_plan, 800, 480)
            .unwrap()
    };
    let image = render(&mut backend, &first);
    let auxiliary_image = multiview.then(|| {
        backend
            .read_geometry_color_target_rgba("particle-fog-preview")
            .unwrap()
    });
    let plan = &first.renderer_output.rhi_command_plan;
    assert_eq!(
        plan.commands
            .iter()
            .filter(|c| matches!(c, RhiCommand::SimulateParticles { .. }))
            .count(),
        3,
        "each source must simulate once, regardless of output count"
    );
    let pix = |v: &[u8], x: usize, y: usize| -> [u8; 4] {
        v[(y * 800 + x) * 4..][..4].try_into().unwrap()
    };
    let tone = |x: f64| {
        let t = (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
        ((1.055 * t.powf(1. / 2.4) - 0.055) * 255.).round() as u8
    };
    let decal = pix(&image, 400, 300);
    assert!(
        decal[0].abs_diff(tone(0.5)) <= 2 && decal[2].abs_diff(tone(0.5)) <= 2 && decal[1] == 0,
        "decal {decal:?}"
    );
    assert_eq!(
        pix(&image, 400, 240),
        // This location is also inside the decal. Hidden cyan must contribute no green.
        [tone(0.5), 0, tone(0.5), 255],
        "behind-wall particle leaked"
    );
    let a = pix(&image, 280, 156);
    let b = pix(&image, 520, 156);
    assert!(
        a[0] > 200 && b[1] > 200,
        "ordinary particles missing: {a:?} {b:?}"
    );
    if let Some(aux) = &auxiliary_image {
        // Orthographic pixel center -> independent radial fog and linear alpha oracle.
        let x: f64 = if split {
            (80.5 - 200.) / 120.
        } else {
            (280.5 - 400.) / 120.
        };
        let y: f64 = (240. - 156.5) / 120.;
        let particle_t = (-0.5f64 * (x * x + y * y + 4.).sqrt()).exp();
        let wall_t = (-0.5f64 * (x * x + y * y + 2.95f64.powi(2)).sqrt()).exp();
        let reference = [
            0.85 * particle_t,
            0.85 * (1. - 0.7 * particle_t) + 0.15 * (1. - wall_t),
            0.15 * wall_t,
        ]
        .map(|v| tone(v * 0.5));
        let actual = pix(aux, if split { 80 } else { 280 }, 156);
        assert!(
            actual[..3]
                .iter()
                .zip(reference)
                .all(|(a, b)| a.abs_diff(b) <= 2),
            "ordinary fog {actual:?} reference {reference:?}"
        );
        assert_ne!(actual, a, "aux fog/exposure was ignored");
        if split {
            // Same authored world particle in second camera; red fog and EV+1.
            let reference =
                [1. - 0.15 * wall_t, 0.85 * 0.3 * particle_t, 0.15 * wall_t].map(|v| tone(v * 2.));
            let right = pix(aux, 480, 156);
            assert!(
                right[..3]
                    .iter()
                    .zip(reference)
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "split right {right:?} reference {reference:?}"
            );
            assert_ne!(actual, right);
            // Every draw and resolve keeps its original per-view viewport.
            assert_eq!(
                plan.commands
                    .iter()
                    .filter(|c| matches!(c, RhiCommand::ResolveColor3d { .. }))
                    .count(),
                6
            );
        }
    }
    let id = "emitter-root".into();
    let mut t = world.transform3d(&id).unwrap().clone();
    t.translation[0] = 0.75;
    world
        .try_insert_component_value(
            id.clone(),
            engine_runtime::components::ComponentTypeId::transform3d(),
            ComponentValue::Transform3D(t),
        )
        .unwrap();
    let moved = tick(&mut host, &mut world, &mut hydrator);
    let moved_image = render(&mut backend, &moved);
    assert_eq!(
        pix(&moved_image, 280, 156),
        a,
        "world particle moved with parent"
    );
    assert_eq!(
        pix(&moved_image, 520, 156),
        [0, 0, tone(1.), 255],
        "local particle did not follow parent"
    );
    assert_eq!(pix(&moved_image, 610, 156), b);
    if multiview {
        let aux = backend
            .read_geometry_color_target_rgba("particle-fog-preview")
            .unwrap();
        assert_ne!(
            pix(&aux, if split { 690 } else { 610 }, 156),
            pix(&moved_image, 610, 156)
        );
    }
    if split {
        let views: Vec<_> = host
            .render_scene()
            .views()
            .filter(|v| v.view_kind == engine_runtime::render_state::RenderViewKind::Preview)
            .map(|v| v.view_id)
            .collect();
        assert_eq!(views.len(), 2);
        for id in views {
            let mut output = engine_runtime::runtime_renderer::RenderTarget::viewport_texture(
                "particle-fog-preview",
                960,
                480,
            );
            output.format = "Rgba8Unorm".into();
            host.set_geometry_view_target(id, Some(output)).unwrap();
        }
        let resized = tick(&mut host, &mut world, &mut hydrator);
        render(&mut backend, &resized);
        let aux = backend
            .read_geometry_color_target_rgba("particle-fog-preview")
            .unwrap();
        assert_eq!(aux.len(), 960 * 480 * 4);
        let original = auxiliary_image.as_ref().unwrap();
        // Orthographic size is the vertical span (4m). Height stays 480px,
        // so scale remains 120px/m while each resized viewport center moves.
        for (new_x, old_x) in [(240 - 120, 200 - 120), (720 - 120, 600 - 120)] {
            let actual = &aux[(156 * 960 + new_x) * 4..][..4];
            assert!(
                actual
                    .iter()
                    .zip(pix(original, old_x, 156))
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "shared resize at {new_x} vs old {old_x}: actual={actual:?} reference={:?}",
                pix(original, old_x, 156)
            );
        }
    }
    let old_error = backend
        .render_plan_to_rgba_bytes(&first.renderer_output.rhi_command_plan, 800, 480)
        .unwrap_err();
    assert!(
        old_error.contains("particle_render.stale_projection"),
        "old packet must not rewind simulation: {old_error}"
    );
    if let Some(root) = std::env::var_os("AIFE_S2C_PRESENTATION_CAPTURE_ROOT") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        let mut images = vec![
            ("presentation-before.png", &image),
            ("presentation-parent-moved.png", &moved_image),
        ];
        if let Some(aux) = &auxiliary_image {
            images.push((
                if split {
                    "presentation-shared-split.png"
                } else {
                    "presentation-fog-preview.png"
                },
                aux,
            ));
        }
        for (name, data) in images {
            let mut e = png::Encoder::new(fs::File::create(root.join(name)).unwrap(), 800, 480);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(data).unwrap();
        }
    }
    for id in ["world-sparks", "local-sparks", "hidden-sparks"] {
        world.try_despawn_entity(&id.into()).unwrap();
    }
    let end = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    render(&mut backend, &end);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    eprintln!("S2C presentation Cook={cook:?} package={pack:?} total={:?}; decal={decal:?} world={a:?} local={b:?}",started.elapsed());
}
