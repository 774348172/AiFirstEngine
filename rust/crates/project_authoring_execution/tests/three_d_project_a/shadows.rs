//! Ordinary project shadow opt-in reaches the existing package/host/production geometry path.
use super::*;
fn fixture() -> Fixture {
    fixture_at("platform_3d_shadows_project")
}
pub(super) fn fixture_at(source: &str) -> Fixture {
    let f = Fixture::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples")
        .join(source);
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
fn shadow_opt_in_is_strict_and_point_or_too_short_spot_is_rejected() {
    let directional = json!({"schemaVersion":"light3d.v1","enabled":true,"color":[1,1,1],"kind":{"type":"directional","illuminanceLux":1},"layers":["world"],"castShadows":true});
    let l: Light3D = serde_json::from_value(directional.clone()).unwrap();
    l.validate().unwrap();
    assert!(l.cast_shadows);
    let mut bad = directional.clone();
    bad["kind"] = json!({"type":"point","intensityCandela":1,"rangeM":1});
    assert!(serde_json::from_value::<Light3D>(bad)
        .unwrap()
        .validate()
        .is_err());
    let mut bad = directional.clone();
    bad["kind"] = json!({"type":"spot","intensityCandela":1,"rangeM":0.05,"innerHalfAngleDegrees":10,"outerHalfAngleDegrees":20});
    assert!(serde_json::from_value::<Light3D>(bad)
        .unwrap()
        .validate()
        .is_err());
    let mut bad = directional;
    bad["shadowMagic"] = true.into();
    assert!(serde_json::from_value::<Light3D>(bad).is_err());
    let mut old = l;
    old.cast_shadows = false;
    assert!(serde_json::to_value(&old)
        .unwrap()
        .get("castShadows")
        .is_none());
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2b_gpu_ordinary_project_cascades_spot_and_outside_camera_caster() {
    use engine_runtime::{
        archetype::ComponentValue,
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        render_contract::{TargetKind, ViewKind},
        rhi_command_plan::RhiCommand,
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let started = std::time::Instant::now();
    let f = fixture();
    let input = generated::input(&f);
    let cook = started.elapsed();
    let (package, _) = generated::package(&f, &input, "production-shadow-package");
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
        let frame = host
            .tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                world,
                RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
            )
            .render_thread_frame
            .unwrap();
        assert_eq!(frame.report.rhi_status, "ok", "{:?}", frame.report);
        frame
    };
    let first = tick(&mut host, &mut world, &mut hydrator);
    let plan = &first.renderer_output.rhi_command_plan;
    let shadow_views: Vec<_> = plan
        .commands
        .iter()
        .filter_map(|c| match c {
            RhiCommand::BeginView { target, view, .. } if view.view_kind == ViewKind::Shadow => {
                Some((target, view))
            }
            _ => None,
        })
        .collect();
    assert_eq!(shadow_views.len(), 5);
    assert_eq!(
        shadow_views
            .iter()
            .map(|(target, _)| *target)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
    assert_eq!(
        shadow_views
            .iter()
            .filter(|(_, v)| v.viewport[2..] == [2048, 2048])
            .count(),
        4
    );
    assert_eq!(
        shadow_views
            .iter()
            .filter(|(_, v)| v.viewport[2..] == [1024, 1024])
            .count(),
        1
    );
    for (_, v) in &shadow_views {
        assert_eq!(v.target_extent, [4096, 4096]);
        assert!(v.clear_color.is_none());
    }
    let draws: Vec<_> = plan
        .commands
        .iter()
        .flat_map(RhiCommand::geometry_instances)
        .collect();
    let outside =
        |d: &(
            &engine_runtime::render_geometry::DrawGeometry,
            &std::sync::Arc<engine_runtime::render_geometry::DrawInstance>,
            engine_runtime::render_graph::OrderedF32,
        )| { d.1.provenance.source_entity_id.as_deref() == Some("off-camera-caster") };
    assert!(
        !draws
            .iter()
            .filter(|d| d.0.view.target_kind != TargetKind::ShadowMap)
            .any(outside),
        "caster must be outside main view"
    );
    assert!(
        draws
            .iter()
            .filter(|d| d.0.view.target_kind == TargetKind::ShadowMap)
            .any(outside),
        "main-view culling erased shadow caster"
    );
    let ready = started.elapsed();
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    backend
        .prepare_geometry_textures(&first.geometry_textures)
        .unwrap();
    let both = backend.render_plan_to_rgba_bytes(plan, 800, 480).unwrap();
    let set = |world: &mut engine_runtime::world::World, sun: bool, spot: bool| {
        for (id, enabled) in [("sun", sun), ("spot", spot)] {
            let mut light = world.light3d(&id.into()).unwrap().clone();
            light.cast_shadows = enabled;
            world
                .try_insert_component_value(
                    id.into(),
                    engine_runtime::components::ComponentTypeId::light3d(),
                    ComponentValue::Light3D(light),
                )
                .unwrap();
        }
    };
    set(&mut world, false, false);
    let unshadowed = tick(&mut host, &mut world, &mut hydrator);
    let baseline = backend
        .render_plan_to_rgba_bytes(&unshadowed.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    set(&mut world, true, false);
    let dir = tick(&mut host, &mut world, &mut hydrator);
    let directional = backend
        .render_plan_to_rgba_bytes(&dir.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    set(&mut world, false, true);
    let spot = tick(&mut host, &mut world, &mut hydrator);
    let spot_pixels = backend
        .render_plan_to_rgba_bytes(&spot.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    let difference = |a: &[u8], b: &[u8]| {
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(a, b)| a[..3].iter().zip(&b[..3]).any(|(a, b)| a.abs_diff(*b) > 2))
            .count()
    };
    let dir_changed = difference(&baseline, &directional);
    let spot_changed = difference(&baseline, &spot_pixels);
    assert!(
        dir_changed > 100,
        "directional shadow missing {dir_changed}"
    );
    assert!(spot_changed > 10, "spot shadow missing {spot_changed}");
    let replay = backend.render_plan_to_rgba_bytes(plan, 800, 480).unwrap();
    assert_eq!(replay, both, "old immutable packet shadow data changed");
    if let Some(root) = std::env::var_os("AIFE_S2B_SHADOW_EVIDENCE_ROOT") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        for (name, pixels) in [
            ("shadows-both", &both),
            ("shadows-off", &baseline),
            ("shadows-directional", &directional),
            ("shadows-spot", &spot_pixels),
        ] {
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
                .write_image_data(pixels)
                .unwrap();
        }
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    eprintln!("S2-B shadow cook_ms={:.3} package_ms={:.3} hydrate_host_ms={:.3} gpu_host_updates_readback_ms={:.3} directional_changed={dir_changed} spot_changed={spot_changed}",cook.as_secs_f64()*1000.,pack.as_secs_f64()*1000.,(ready-cook-pack).as_secs_f64()*1000.,(started.elapsed()-ready).as_secs_f64()*1000.);
}

#[test]
fn s2b_production_shadow_capacity_is_four_cascades_eight_spots_and_ninth_is_an_error() {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        rhi_command_plan::RhiCommand,
        runtime_scene_hydration::RuntimeSceneHydrator,
    };
    for count in [8, 9] {
        let f = fixture();
        let mut source: Value =
            serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
        let template = source["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "spot")
            .unwrap()
            .clone();
        for i in 1..count {
            let mut e = template.clone();
            e["id"] = format!("spot-{i}").into();
            e["name"] = format!("spot-{i}").into();
            e["siblingOrder"] = (10 + i).into();
            source["entities"].as_array_mut().unwrap().push(e);
        }
        f.write("Scenes/main.json", &source);
        let input = generated::input(&f);
        let (package, _) = generated::package(&f, &input, "shadow-capacity");
        let mut world = engine_runtime::world::World::new();
        let mut hydrator = RuntimeSceneHydrator::from_package(&package);
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
        let plan = &frame.renderer_output.rhi_command_plan;
        if count == 9 {
            assert_eq!(
                frame.report.rhi_status, "error",
                "ninth shadow was silently truncated"
            );
            assert_eq!(frame.report.present_status, "not_submitted");
            assert!(
                plan.commands.is_empty(),
                "failed production frame must not be submitted"
            );
            assert!(
                frame
                    .report
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "three_d.production_frame_failed"
                        && d.message.contains("shadow.projection_capacity")),
                "{:?}",
                frame.report.diagnostics
            );
            continue;
        }
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let views: Vec<_> = plan
            .commands
            .iter()
            .filter_map(|c| match c {
                RhiCommand::BeginView { view, .. } if view.is_shadow_map() => Some(view),
                _ => None,
            })
            .collect();
        assert_eq!(views.len(), 12);
        let spots: std::collections::BTreeSet<_> = views
            .iter()
            .filter(|v| v.viewport[2] == 1024)
            .map(|v| v.viewport)
            .collect();
        assert_eq!(spots.len(), 8);
        assert!(spots.contains(&[3072, 1024, 1024, 1024]));
        #[cfg(feature = "real-wgpu")]
        {
            let mut backend =
                engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(1280, 720)
                    .unwrap();
            backend
                .prepare_geometry_textures(&frame.geometry_textures)
                .unwrap();
            let pixels = backend.render_plan_to_rgba_bytes(plan, 1280, 720).unwrap();
            assert!(
                pixels
                    .chunks_exact(4)
                    .filter(|p| p[..3].iter().any(|v| *v > 10))
                    .count()
                    > 1000
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
            eprintln!("S2-B capacity: four cascades and eight spot tiles executed on real GPU; not BM04 scale qualification");
        }
    }
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2b_gpu_shadow_two_cameras_rebased_origins_and_resize_keep_independent_atlases() {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        project_runtime_session::ProjectRuntimeMutationBuffer,
        render_state::{RenderTargetKind, RenderViewKind, Viewport},
        rhi_command_plan::RhiCommand,
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let f = fixture();
    let mut source: Value =
        serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
    let mut aux = source["entities"][1].clone();
    aux["id"] = "aux".into();
    aux["name"] = "aux".into();
    aux["components"][0]["data"]["translation"] = json!([-100, 0, 0]);
    source["entities"].as_array_mut().unwrap().push(aux);
    f.write("Scenes/main.json", &source);
    let input = generated::input(&f);
    let (package, _) = generated::package(&f, &input, "shadow-dual-origin");
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    for entity in source["entities"].as_array().unwrap() {
        let id: engine_runtime::ids::EntityId = entity["id"].as_str().unwrap().into();
        mutations.spatial_command(
            id.clone(),
            world.entity_generation(&id).unwrap(),
            project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
                anchor: [
                    if id.as_str() == "aux" {
                        1_000_100.
                    } else {
                        1_000_000.
                    },
                    0.,
                    0.,
                ],
                preserve_world: false,
            },
        );
    }
    mutations
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();
    let mut host = EngineHostLoop::new("main");
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    for [width, height] in [[800, 480], [960, 576], [800, 480]] {
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
        let plan = &frame.renderer_output.rhi_command_plan;
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let snapshot = host.retained_geometry_snapshot().unwrap();
        assert_eq!(snapshot.views.len(), 2);
        assert_ne!(
            snapshot.views[0].camera.origin,
            snapshot.views[1].camera.origin
        );
        let atlases: std::collections::BTreeSet<_> = plan
            .commands
            .iter()
            .filter_map(|c| match c {
                RhiCommand::BeginView { target, view, .. } if view.is_shadow_map() => {
                    Some(target.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            atlases.len(),
            4,
            "per-camera directional/spot atlases must not alias"
        );
        backend
            .prepare_geometry_textures(&frame.geometry_textures)
            .unwrap();
        let pixels = backend
            .render_plan_to_rgba_bytes(plan, width, height)
            .unwrap();
        let mut changed = 0usize;
        let mut total_error = 0u64;
        let mut visible = 0usize;
        for y in 0..height {
            for x in 0..width / 2 {
                let a = ((y * width + x) * 4) as usize;
                let b = ((y * width + x + width / 2) * 4) as usize;
                if pixels[a..a + 3].iter().any(|v| *v > 10) {
                    visible += 1;
                }
                let delta = (0..3)
                    .map(|c| pixels[a + c].abs_diff(pixels[b + c]))
                    .max()
                    .unwrap();
                changed += usize::from(delta > 2);
                total_error += u64::from(delta);
            }
        }
        assert!(visible > 1000, "missing scene");
        assert!(changed<=64 && total_error < u64::from(width*height)/20,
            "same absolute camera pose diverged under rebasing {width}x{height}: pixels>2={changed}, total_error={total_error}");
        eprintln!("S2-B shadow dual origin {width}x{height}: four atlases, changed={changed}, total_error={total_error}");
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2b_gpu_independent_color_targets_keep_full_light_shadow_chain_and_resize() {
    use engine_runtime::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        project_runtime_session::ProjectRuntimeMutationBuffer,
        render_state::{RenderTargetKind, RenderViewKind, Viewport},
        rhi_command_plan::RhiCommand,
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let f = fixture();
    let mut source: Value =
        serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
    let mut aux = source["entities"][1].clone();
    aux["id"] = "aux".into();
    aux["name"] = "aux".into();
    aux["components"][0]["data"]["translation"] = json!([-100, 0, 0]);
    source["entities"].as_array_mut().unwrap().push(aux);
    f.write("Scenes/main.json", &source);
    let input = generated::input(&f);
    let (package, _) = generated::package(&f, &input, "shadow-independent-targets");
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let mut mutations = ProjectRuntimeMutationBuffer::new();
    for entity in source["entities"].as_array().unwrap() {
        let id: engine_runtime::ids::EntityId = entity["id"].as_str().unwrap().into();
        mutations.spatial_command(
            id.clone(),
            world.entity_generation(&id).unwrap(),
            project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
                anchor: [
                    if id.as_str() == "aux" {
                        1_000_100.
                    } else {
                        1_000_000.
                    },
                    0.,
                    0.,
                ],
                preserve_world: false,
            },
        );
    }
    mutations
        .prepare(&world)
        .unwrap()
        .commit(&mut world)
        .unwrap();
    // Each host consumes one World's projection stream. A second host attached
    // after the first consumed structural events is not a valid reference.
    let mut reference_world = engine_runtime::world::World::new();
    let mut reference_hydrator = RuntimeSceneHydrator::from_package(&package);
    assert!(!reference_hydrator
        .hydrate_active_scene(&package, &mut reference_world)
        .has_errors());
    let mut reference_mutations = ProjectRuntimeMutationBuffer::new();
    for entity in source["entities"].as_array().unwrap() {
        let id: engine_runtime::ids::EntityId = entity["id"].as_str().unwrap().into();
        reference_mutations.spatial_command(
            id.clone(),
            reference_world.entity_generation(&id).unwrap(),
            project_runtime_sdk::ProjectRuntimeSpatialIntent::SetWorldAnchor {
                anchor: [
                    if id.as_str() == "aux" {
                        1_000_100.
                    } else {
                        1_000_000.
                    },
                    0.,
                    0.,
                ],
                preserve_world: false,
            },
        );
    }
    reference_mutations
        .prepare(&reference_world)
        .unwrap()
        .commit(&mut reference_world)
        .unwrap();
    let mut reference_host = EngineHostLoop::new("reference");
    let mut primary_baseline = None;
    let mut host = EngineHostLoop::new("main");
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    for [width, height] in [[800, 480], [960, 576], [800, 480]] {
        host.set_game_view_target(GameViewTargetSpec::new(
            800,
            480,
            GameViewScalePolicy::Stretch,
        ));
        host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        );
        let aux_view = host
            .project_auxiliary_camera_view(
                &world,
                &"aux".into(),
                Viewport {
                    x: 0,
                    y: 0,
                    width,
                    height,
                },
                RenderViewKind::Preview,
                RenderTargetKind::ViewportTexture,
            )
            .unwrap();
        host.set_geometry_view_target(
            aux_view,
            Some(
                engine_runtime::runtime_renderer::RenderTarget::viewport_texture(
                    "independent-preview",
                    width,
                    height,
                ),
            ),
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
        let plan = &frame.renderer_output.rhi_command_plan;
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let snapshot = host.retained_geometry_snapshot().unwrap();
        assert_eq!(snapshot.views.len(), 2);
        assert_ne!(
            snapshot.views[0].camera.origin,
            snapshot.views[1].camera.origin
        );
        let atlases: std::collections::BTreeSet<_> = plan
            .commands
            .iter()
            .filter_map(|c| match c {
                RhiCommand::BeginView { target, view, .. } if view.is_shadow_map() => {
                    Some(target.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            atlases.len(),
            4,
            "per-camera directional/spot atlases must not alias"
        );
        backend
            .prepare_geometry_textures(&frame.geometry_textures)
            .unwrap();
        let pixels = backend.render_plan_to_rgba_bytes(plan, 800, 480).unwrap();
        let secondary = backend
            .read_geometry_color_target_rgba("independent-preview")
            .unwrap();
        assert_eq!(secondary.len(), (width * height * 4) as usize);
        // Independent reference: ordinary main camera only, at the secondary's resolution.
        // Same package and absolute pose, no remapping of the tested plan or shader oracle.
        reference_host.set_game_view_target(GameViewTargetSpec::new(
            width,
            height,
            GameViewScalePolicy::Stretch,
        ));
        let expected = plan
            .commands
            .iter()
            .flat_map(RhiCommand::geometry_instances)
            .filter(|(draw, _, _)| {
                !draw.view.is_shadow_map() && draw.target.as_ref() != "independent-preview"
            })
            .count();
        assert!(expected > 0);
        let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let reference = loop {
            let frame = reference_host
                .tick_with_runtime_context(
                    EngineFrameInput::new(EngineHostMode::ExportedGame),
                    &mut reference_world,
                    RuntimeFrameContext::new(&package, reference_hydrator.instance_loader_mut()),
                )
                .render_thread_frame
                .unwrap();
            assert_eq!(frame.report.rhi_status, "ok", "{:?}", frame.report);
            let count = frame
                .renderer_output
                .rhi_command_plan
                .commands
                .iter()
                .flat_map(RhiCommand::geometry_instances)
                .filter(|(draw, _, _)| !draw.view.is_shadow_map())
                .count();
            if count == expected {
                break frame;
            }
            assert!(
                std::time::Instant::now() < ready_deadline,
                "single-camera reference never ready: expected={expected} actual={count}"
            );
            std::thread::yield_now();
        };
        assert_eq!(reference.report.rhi_status, "ok", "{:?}", reference.report);
        backend
            .prepare_geometry_textures(&reference.geometry_textures)
            .unwrap();
        let reference_pixels = backend
            .render_plan_to_rgba_bytes(&reference.renderer_output.rhi_command_plan, width, height)
            .unwrap();
        if let Some(path) = std::env::var_os("AIFE_371_MULTITARGET_CAPTURE_ROOT") {
            let root = PathBuf::from(path);
            fs::create_dir_all(&root).unwrap();
            for (name, w, h, bytes) in [
                ("diagnostic-primary", 800, 480, &pixels),
                ("diagnostic-secondary", width, height, &secondary),
                ("diagnostic-reference", width, height, &reference_pixels),
            ] {
                let mut encoder = png::Encoder::new(
                    fs::File::create(root.join(format!("{name}-{width}x{height}.png"))).unwrap(),
                    w,
                    h,
                );
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(bytes)
                    .unwrap();
            }
        }

        let mut changed = 0usize;
        let mut total_error = 0u64;
        for (a, b) in secondary
            .chunks_exact(4)
            .zip(reference_pixels.chunks_exact(4))
        {
            let delta = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap();
            changed += usize::from(delta > 2);
            total_error += u64::from(delta);
        }
        if let Some(previous) = &primary_baseline {
            assert_eq!(
                &pixels, previous,
                "resizing the independent target changed primary output"
            );
        } else {
            primary_baseline = Some(pixels.clone());
        }
        assert!(changed <= 128 && total_error < u64::from(width*height)/10,
            "independent output differs from ordinary single-camera reference: {width}x{height}, changed={changed},total_error={total_error}");
        assert!(
            pixels
                .chunks_exact(4)
                .filter(|p| p[..3].iter().any(|v| *v > 10))
                .count()
                > 1000
        );
        assert!(
            backend
                .read_geometry_color_target_rgba("independent-preview")
                .is_err(),
            "removed output retained native storage after reference-only plan"
        );
        // Replaying the old immutable frame must recreate its output with its own extent.
        let replay = backend.render_plan_to_rgba_bytes(plan, 800, 480).unwrap();
        assert_eq!(pixels, replay);
        let replay_secondary = backend
            .read_geometry_color_target_rgba("independent-preview")
            .unwrap();
        assert_eq!(secondary, replay_secondary);
        if let Some(path) = std::env::var_os("AIFE_371_MULTITARGET_CAPTURE_ROOT") {
            let root = PathBuf::from(path);
            fs::create_dir_all(&root).unwrap();
            for (name, w, h, bytes) in [
                ("primary", 800, 480, &pixels),
                ("secondary", width, height, &secondary),
            ] {
                let mut encoder = png::Encoder::new(
                    fs::File::create(root.join(format!("{name}-{width}x{height}.png"))).unwrap(),
                    w,
                    h,
                );
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(bytes)
                    .unwrap();
            }
        }
        eprintln!("S2-B independent color targets: primary=800x480 secondary={width}x{height}, four atlases, reference_changed={changed}, total_error={total_error}; old frame replay identical");
    }
    // Last mesh unload must still clear both live camera outputs.
    let meshes: Vec<_> = world
        .entity_ids()
        .into_iter()
        .filter(|id| world.mesh_renderer3d(id).is_some())
        .cloned()
        .collect();
    for id in meshes {
        world.try_despawn_entity(&id).unwrap();
    }
    let empty = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    assert_eq!(empty.report.rhi_status, "ok", "{:?}", empty.report);
    backend
        .prepare_geometry_textures(&empty.geometry_textures)
        .unwrap();
    let cleared = backend
        .render_plan_to_rgba_bytes(&empty.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    assert!(cleared.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
    let preview = backend
        .read_geometry_color_target_rgba("independent-preview")
        .unwrap();
    assert!(preview.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
    world.try_despawn_entity(&"aux".into()).unwrap();
    let removed = host
        .tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        )
        .render_thread_frame
        .unwrap();
    assert_eq!(removed.report.rhi_status, "ok", "{:?}", removed.report);
    backend
        .render_plan_to_rgba_bytes(&removed.renderer_output.rhi_command_plan, 800, 480)
        .unwrap();
    assert!(backend
        .read_geometry_color_target_rgba("independent-preview")
        .is_err());
    eprintln!(
        "S2-B unload: both camera outputs cleared; removed auxiliary camera released its output"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2c_gpu_ordinary_project_hdr_aa_ambient_resize_and_immutable_replay() {
    color_project_consumer(
        "platform_3d_color_project",
        "AIFE_S2C_COLOR_CAPTURE_ROOT",
        false,
        false,
    );
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2c_gpu_ordinary_project_sky_fog_keeps_independent_outputs_and_old_frames() {
    color_project_consumer(
        "platform_3d_environment_project",
        "AIFE_S2C_ENV_CAPTURE_ROOT",
        true,
        false,
    );
}
#[cfg(feature = "real-wgpu")]
fn color_project_consumer(source: &str, capture_environment: &str, environment: bool, ibl: bool) {
    use engine_runtime::rhi_command_plan::RhiCommand;
    use engine_runtime::{
        archetype::ComponentValue,
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        game_view_presentation::{GameViewScalePolicy, GameViewTargetSpec},
        runtime_scene_hydration::RuntimeSceneHydrator,
        wgpu_backend::real::RealWgpuBackend,
    };
    let f = fixture_at(source);
    let mut authored: Value =
        serde_json::from_slice(&fs::read(f.root.join("Scenes/main.json")).unwrap()).unwrap();
    let mut aux = authored["entities"][1].clone();
    aux["id"] = "aux".into();
    aux["name"] = "aux".into();
    authored["entities"].as_array_mut().unwrap().push(aux);
    f.write("Scenes/main.json", &authored);
    let started = std::time::Instant::now();
    let input = generated::input(&f);
    let cook = started.elapsed();
    let (package, _) = generated::package(&f, &input, "color3d-production");
    let packed = started.elapsed();
    let mut world = engine_runtime::world::World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    assert!(!hydrator
        .hydrate_active_scene(&package, &mut world)
        .has_errors());
    let id: engine_runtime::ids::EntityId = "camera".into();
    assert!(
        world.camera3d(&id).unwrap().color_pipeline.is_some(),
        "ordinary project camera color settings lost in Cook/package/hydration"
    );
    if environment {
        let config = world
            .camera3d(&id)
            .unwrap()
            .color_pipeline
            .as_ref()
            .unwrap();
        assert!(
            config.sky.is_some() && config.fog.is_some(),
            "ordinary asset environment lost in package"
        );
    }
    let mut host = EngineHostLoop::new("main");
    let mut backend = RealWgpuBackend::new_offscreen(800, 480).unwrap();
    let mut old = None;
    let mut baseline = None;
    let mut auxiliary_baseline = None;
    for (width, height, ev) in [(800, 480, 0.), (960, 576, -1.), (800, 480, 1.)] {
        host.set_game_view_target(GameViewTargetSpec::new(
            width,
            height,
            GameViewScalePolicy::Stretch,
        ));
        let mut camera = world.camera3d(&id).unwrap().clone();
        camera.color_pipeline.as_mut().unwrap().exposure_ev = ev.into();
        if ibl {
            let setting = camera
                .color_pipeline
                .as_mut()
                .unwrap()
                .ibl
                .as_mut()
                .unwrap();
            setting.intensity = (if ev > 0. { 2. } else { 1. }).into();
            if ev < 0. {
                setting.environment.0.id = "warm".into();
                setting.environment.0.guid = Some("guid-warm".into());
            } else {
                setting.environment.0.id = "studio".into();
                setting.environment.0.guid = Some("guid-studio".into());
            }
        }
        if environment {
            camera
                .color_pipeline
                .as_mut()
                .unwrap()
                .fog
                .as_mut()
                .unwrap()
                .density_per_m = (if ev == 0. {
                0.06
            } else if ev < 0. {
                0.1
            } else {
                0.02
            })
            .into();
        }
        world
            .try_insert_component_value(
                id.clone(),
                engine_runtime::components::ComponentTypeId::camera3d(),
                ComponentValue::Camera3D(camera),
            )
            .unwrap();
        host.tick_with_runtime_context(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
            RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
        );
        let auxiliary = host
            .project_auxiliary_camera_view(
                &world,
                &"aux".into(),
                engine_runtime::render_state::Viewport {
                    x: 0,
                    y: 0,
                    width: 800,
                    height: 480,
                },
                engine_runtime::render_state::RenderViewKind::Preview,
                engine_runtime::render_state::RenderTargetKind::ViewportTexture,
            )
            .unwrap();
        host.set_geometry_view_target(
            auxiliary,
            Some(
                engine_runtime::runtime_renderer::RenderTarget::viewport_texture(
                    "color-preview",
                    800,
                    480,
                ),
            ),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let frame = loop {
            let frame = host
                .tick_with_runtime_context(
                    EngineFrameInput::new(EngineHostMode::ExportedGame),
                    &mut world,
                    RuntimeFrameContext::new(&package, hydrator.instance_loader_mut()),
                )
                .render_thread_frame
                .unwrap();
            if ibl && frame.report.rhi_status != "ok" {
                assert!(
                    format!("{:?}", frame.report).contains("environment.async"),
                    "{:?}",
                    frame.report
                );
                assert!(
                    std::time::Instant::now() < deadline,
                    "environment async never completed"
                );
                std::thread::yield_now();
                continue;
            }
            assert_eq!(frame.report.rhi_status, "ok", "{:?}", frame.report);
            let draw_count = frame
                .renderer_output
                .rhi_command_plan
                .commands
                .iter()
                .flat_map(RhiCommand::geometry_instances)
                .filter(|(draw, _, _)| !draw.view.is_shadow_map())
                .count();
            if draw_count == 14 {
                break frame;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "ordinary color scene never ready: {draw_count}"
            );
            std::thread::yield_now();
        };
        let plan = &frame.renderer_output.rhi_command_plan;
        if environment {
            let mut color_views = 0;
            for command in &plan.commands {
                if let RhiCommand::BeginView { view, .. } = command {
                    if view.is_shadow_map() {
                        assert!(view.color_pipeline.is_none());
                    } else {
                        let config = view.color_pipeline.as_ref().unwrap();
                        assert!(config.sky.is_some() && config.fog.is_some());
                        color_views += 1;
                    }
                }
            }
            assert_eq!(
                color_views, 2,
                "asset environment must survive both immutable view packets"
            );
        }

        if ibl {
            let mut count = 0;
            for c in &plan.commands {
                if let Some(draw) = c.geometry_draw() {
                    if draw.view.is_shadow_map() {
                        assert!(draw.environment.is_none());
                    } else if draw.standard_lit.is_some() {
                        assert!(draw.environment.is_some());
                        count += c.geometry_instance_count();
                    }
                }
            }
            assert!(
                count >= 12,
                "ordinary material environment binding lost: {count}"
            );
        }
        assert_eq!(
            plan.commands
                .iter()
                .filter(|c| matches!(c, RhiCommand::ResolveColor3d { .. }))
                .count(),
            4
        );
        assert_eq!(
            plan.commands
                .iter()
                .filter(|c| matches!(c,RhiCommand::BeginView{view,..} if view.is_shadow_map()))
                .count(),
            10
        );
        backend
            .prepare_geometry_textures(&frame.geometry_textures)
            .unwrap();
        let pixels = backend
            .render_plan_to_rgba_bytes(plan, width, height)
            .unwrap();
        assert!(
            pixels
                .chunks_exact(4)
                .filter(|p| p[..3].iter().any(|v| *v > 10))
                .count()
                > 1000
        );
        let preview = backend
            .read_geometry_color_target_rgba("color-preview")
            .unwrap();
        if let Some(expected) = &auxiliary_baseline {
            assert_eq!(
                &preview, expected,
                "main exposure/resize contaminated independent HDR output"
            );
        } else {
            assert_eq!(
                &preview, &pixels,
                "equal cameras on independent HDR targets differ"
            );
            auxiliary_baseline = Some(preview);
        }
        if old.is_none() {
            old = Some(frame.clone());
            baseline = Some(pixels.clone());
        }
        if let Some(root) = std::env::var_os(capture_environment) {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).unwrap();
            let mut encoder = png::Encoder::new(
                fs::File::create(root.join(format!("color-{width}x{height}-ev{ev}.png"))).unwrap(),
                width,
                height,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
        eprintln!("ordinary color pipeline: {width}x{height}, EV={ev}, ten shadow writers and four actual color resolves; fixed auxiliary output unchanged");
    }
    let old = old.unwrap();
    backend
        .prepare_geometry_textures(&old.geometry_textures)
        .unwrap();
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&old.renderer_output.rhi_command_plan, 800, 480)
            .unwrap(),
        baseline.unwrap()
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    eprintln!(
        "color project cost: Cook {:.3}s, package {:.3}s, host/GPU/readback {:.3}s",
        cook.as_secs_f64(),
        (packed - cook).as_secs_f64(),
        (started.elapsed() - packed).as_secs_f64()
    );
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn s2c_gpu_ordinary_project_ibl_asset_replacement_and_independent_outputs() {
    color_project_consumer(
        "platform_3d_ibl_project",
        "AIFE_S2C_IBL_CAPTURE_ROOT",
        true,
        true,
    );
}
