use crate::{
    particle_effect::*, particle_render_contract::*, render_graph::*, rhi_command_plan::*,
    wgpu_backend::real::RealWgpuBackend,
};

#[test]
fn particle_render_measurement_actual_gpu_counts_timestamps_clear_and_retirement() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let info = adapter.get_info();
    eprintln!("measurement adapter: {info:?}");
    let features = crate::gpu_frame_measurement::features(adapter.features(), true);
    assert!(
        !features.is_empty(),
        "timestamp qualification requires real support"
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: features,
        required_limits: crate::particle_gpu::renderer_device_limits(adapter.limits()),
        ..Default::default()
    }))
    .unwrap();
    let mut backend = RealWgpuBackend::from_device_queue(
        device.clone(),
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        128,
        128,
        "test",
    );
    assert!(
        backend.finish_frame_measurement().is_none(),
        "normal path must not allocate measurement"
    );
    let mut description = effect(ParticleGeometry::Billboard2d {}).description;
    description.budget.max_particles = 100_000;
    description.emitters[0].capacity = 100_000;
    description.emitters[0].bursts[0].count = 200_000;
    description.emitters[0].size_meters = [0.001; 2];
    backend
        .install_particle_effect(1, &cook(description, program::DEFAULT_BEHAVIOR))
        .unwrap();
    let bytes = backend.particle_buffer_bytes(1).unwrap();
    assert!(bytes > 10_000_000 && bytes <= 128 << 20);
    backend.enable_frame_measurement(&info, 0, 4).unwrap();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let submit = |backend: &mut RealWgpuBackend, commands| {
        let r = backend.execute_plan_to_surface_view(&plan(commands), &view);
        assert!(
            !r.diagnostics
                .iter()
                .any(|d| d.severity == crate::engine_rhi::RhiBackendDiagnosticSeverity::Error),
            "{r:?}"
        );
    };
    submit(&mut backend, vec![step(1), draw()]);
    submit(&mut backend, vec![step(2), draw()]);
    backend.clear_particle_effect(1).unwrap();
    submit(&mut backend, vec![draw()]);
    // Submit and retire without waiting for any in-flight frame or counter readback.
    assert!(backend.remove_particle_effect(1));
    assert!(backend.particle_buffer_bytes(1).is_none());
    submit(&mut backend, vec![]);
    let report = backend.finish_frame_measurement().unwrap();
    assert_eq!(report.status, "complete", "{report:?}");
    let samples = &report.samples;
    assert_eq!(samples.len(), 4);
    assert_eq!(
        (samples[0].live, samples[0].indirect_instances),
        (100_000, 100_000)
    );
    assert_eq!(samples[0].spawned_last_substep, 100_000);
    assert_eq!(samples[0].dropped_last_substep, 100_000);
    assert_eq!(samples[1].live, 100_000);
    assert_eq!(samples[1].dropped_last_substep, 0);
    assert_eq!(samples[0].dispatch_count, 5);
    assert_eq!(samples[0].particle_draw_count, 1);
    assert_eq!(samples[0].particle_buffer_bytes, bytes);
    assert_eq!((samples[2].live, samples[2].indirect_instances), (0, 0));
    assert_eq!(samples[3].particle_buffer_bytes, 0);
    for s in &samples[..2] {
        assert!(
            s.frame_ms > 0.0
                && s.particle_simulation_ms > 0.0
                && s.particle_prepare_ms > 0.0
                && s.particle_draw_ms > 0.0,
            "{s:?}"
        );
        assert!(
            s.frame_ms >= s.particle_simulation_ms + s.particle_prepare_ms + s.particle_draw_ms
        );
    }
    let json = serde_json::to_string(&report).unwrap();
    assert_eq!(
        serde_json::from_str::<crate::windowed_player::WindowedPlayerGpuPerformance>(&json)
            .unwrap(),
        report
    );
}

#[test]
fn particle_render_measurement_unsupported_is_explicit_and_opt_out_empty() {
    assert!(crate::gpu_frame_measurement::features(wgpu::Features::all(), false).is_empty());
    assert!(
        crate::gpu_frame_measurement::features(wgpu::Features::TIMESTAMP_QUERY, true).is_empty()
    );
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let info = wgpu::AdapterInfo {
        name: "unsupported fixture".into(),
        vendor: 0,
        device: 0,
        device_type: wgpu::DeviceType::Other,
        driver: String::new(),
        driver_info: String::new(),
        backend: wgpu::Backend::Vulkan,
    };
    assert!(backend.enable_frame_measurement(&info, 0, 2049).is_err());
    backend.enable_frame_measurement(&info, 0, 1).unwrap();
    let report = backend.finish_frame_measurement().unwrap();
    assert_eq!(report.status, "unsupported_timestamp");
    assert!(report.samples.is_empty());
    assert_eq!(report.measurement_buffer_bytes, 0);
}

fn effect(geometry: ParticleGeometry) -> CookedParticleEffect {
    let mut desc: ParticleEffectDescription =
        serde_json::from_str(include_str!("../../tests/fixtures/particle-effect.json")).unwrap();
    let e = &mut desc.emitters[0];
    e.capacity = 4;
    e.bursts[0].count = 1;
    e.velocity_min = [0.0; 3];
    e.velocity_max = [0.0; 3];
    e.lifetime_seconds = [1.0; 2];
    e.color = [1.0, 0.0, 0.0, 1.0];
    e.size_meters = [0.5; 2];
    e.updates = vec![ParticleUpdate::Integrate {}];
    e.draw.geometry = geometry;
    cook(desc, program::DEFAULT_BEHAVIOR)
}
fn cook(desc: ParticleEffectDescription, author: &str) -> CookedParticleEffect {
    desc.validate().unwrap();
    let programs = desc
        .emitters
        .iter()
        .map(|e| CookedParticleProgram {
            emitter: e.name.clone(),
            wgsl: program::compute_source(
                &desc,
                e,
                &(program::prefix(&desc, e) + author + program::CONTRACT_CALLS),
            ),
            author_path: None,
            author_first_line: 0,
            author_line_count: 0,
            particle_stride: program::particle_layout(e).2,
        })
        .collect();
    CookedParticleEffect {
        program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
        description: desc,
        programs,
        source_digests: Default::default(),
    }
}
fn step(id: u64) -> RenderPassCommand {
    RenderPassCommand::SimulateParticles {
        target: "out".into(),
        instance: 1,
        step: ParticleRenderStep {
            step_id: id,
            delta_seconds: (1.0 / 64.0).into(),
            origin: [0.0.into(), 0.0.into(), 0.5.into()],
            emitting: true,
            paused: false,
        },
    }
}
fn draw() -> RenderPassCommand {
    RenderPassCommand::DrawParticles {
        target: "out".into(),
        instance: 1,
        emitter: 0,
        view: Default::default(),
        texture: None,
        mesh: None,
    }
}
fn plan(commands: Vec<RenderPassCommand>) -> RhiCommandPlan {
    let mut graph = RenderGraph::new("particle-test", 1);
    graph.output_target = Some("out".into());
    graph
        .resources
        .push(RenderResource::surface_backbuffer("out", 128, 128));
    graph.passes.push(RenderPass {
        pass_id: "particles".into(),
        pass_name: "Particles".into(),
        pass_kind: RenderPassKind::DrawParticles,
        view_id: "camera".into(),
        reads: vec![],
        writes: vec!["out".into()],
        color_targets: vec!["out".into()],
        depth_target: None,
        commands,
        debug_source: None,
    });
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    plan
}
fn pixel(image: &[u8], x: usize, y: usize) -> [u8; 4] {
    image[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
fn save(name: &str, image: &[u8]) {
    if let Some(root) = std::env::var_os("PARTICLE_RENDER_EVIDENCE_DIR") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let mut encoder = png::Encoder::new(
            std::fs::File::create(root.join(format!("{name}.png"))).unwrap(),
            128,
            128,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(image)
            .unwrap();
    }
}
fn close(actual: [u8; 4], expected: [u8; 4]) {
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
        "{actual:?} != {expected:?}"
    );
}
fn asset(kind: &str) -> crate::runtime_package::RuntimeAssetRef {
    serde_json::from_value(serde_json::json!({"id":"test-asset","type":kind,"guid":"test-guid"}))
        .unwrap()
}
fn handle(
    kind: crate::render_resource::RenderResourceKind,
) -> crate::render_resource::RenderResourceHandle {
    crate::render_resource::RenderResourceHandle {
        kind,
        index: 41,
        generation: 1,
    }
}

#[test]
fn particle_render_real_graph_dispatch_and_indirect_pixels() {
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .install_particle_effect(1, &effect(ParticleGeometry::Billboard3d {}))
        .unwrap();
    let bytes = backend.particle_buffer_bytes(1);
    let commands = plan(vec![step(1), draw()]);
    let image = backend
        .render_plan_to_rgba_bytes(&commands, 128, 128)
        .unwrap();
    save("billboard-front", &image);
    assert_eq!(pixel(&image, 64, 64), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 20, 20), [0, 0, 0, 255]);
    let again = backend
        .render_plan_to_rgba_bytes(&commands, 128, 128)
        .unwrap();
    assert_eq!(image, again);
    assert_eq!(bytes, backend.particle_buffer_bytes(1));
    assert_eq!(
        backend.capture_particle_state_for_validation(1).unwrap()[0].particles[0].age,
        0.0
    );
}

#[test]
fn particle_render_consecutive_draws_preserve_order_and_repeated_emitter_views() {
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let mut desc = effect(ParticleGeometry::Billboard2d {}).description;
    desc.emitters[0].space = ParticleSpace::Local;
    desc.emitters[0].draw.blend = ParticleBlend::Alpha;
    let mut second = desc.emitters[0].clone();
    second.name = "green".into();
    second.color = [0.0, 1.0, 0.0, 1.0];
    desc.emitters.push(second);
    backend
        .install_particle_effect(1, &cook(desc, program::DEFAULT_BEHAVIOR))
        .unwrap();
    let shifted = |emitter, x: f32| {
        let mut d = draw();
        if let RenderPassCommand::DrawParticles {
            emitter: e, view, ..
        } = &mut d
        {
            *e = emitter;
            view.origin = [x.into(), 0.0.into(), 0.5.into()];
        }
        d
    };
    let image = backend
        .render_plan_to_rgba_bytes(
            &plan(vec![
                step(1),
                shifted(0, -0.5),
                shifted(1, -0.5),
                shifted(0, 0.5),
            ]),
            128,
            128,
        )
        .unwrap();
    // Green covers the first red draw, while red must also survive at the
    // second origin. Preparing both red draws together would erase the left one.
    assert_eq!(pixel(&image, 32, 64), [0, 255, 0, 255]);
    assert_eq!(pixel(&image, 96, 64), [255, 0, 0, 255]);
    let repeated = backend
        .render_plan_to_rgba_bytes(&plan(vec![shifted(0, -0.5), shifted(0, 0.5)]), 128, 128)
        .unwrap();
    assert_eq!(pixel(&repeated, 32, 64), [255, 0, 0, 255]);
    assert_eq!(pixel(&repeated, 96, 64), [255, 0, 0, 255]);
}

#[test]
fn particle_render_camera_billboards_local_world_and_y_direction() {
    for geometry in [
        ParticleGeometry::Billboard2d {},
        ParticleGeometry::Billboard3d {},
    ] {
        let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
        backend
            .install_particle_effect(1, &effect(geometry.clone()))
            .unwrap();
        let mut command = draw();
        if let RenderPassCommand::DrawParticles { view, .. } = &mut command {
            view.world_to_clip = [
                [0., 0., -1., 0.],
                [0., 1., 0., 0.],
                [1., 0., 0., 0.],
                [-0.5, 0., 0.5, 1.],
            ]
            .map(|v| v.map(Into::into));
            view.right = [0., 0., 1.].map(Into::into);
            view.forward = [-1., 0., 0.].map(Into::into);
        }
        let image = backend
            .render_plan_to_rgba_bytes(&plan(vec![step(1), command]), 128, 128)
            .unwrap();
        let facing = matches!(geometry, ParticleGeometry::Billboard3d {});
        assert_eq!(pixel(&image, 64, 64)[0] > 200, facing);
        save(
            if facing {
                "billboard-side-3d"
            } else {
                "billboard-side-2d"
            },
            &image,
        );
    }
    for space in [ParticleSpace::World, ParticleSpace::Local] {
        let mut base = effect(ParticleGeometry::Billboard2d {}).description;
        base.emitters[0].space = space;
        let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
        backend
            .install_particle_effect(1, &cook(base, program::DEFAULT_BEHAVIOR))
            .unwrap();
        let mut command = draw();
        if let RenderPassCommand::DrawParticles { view, .. } = &mut command {
            view.origin = [0.5, 0.5, 0.0].map(Into::into);
        }
        let image = backend
            .render_plan_to_rgba_bytes(&plan(vec![step(1), command]), 128, 128)
            .unwrap();
        let local = space == ParticleSpace::Local;
        assert_eq!(pixel(&image, 96, 32)[0] > 200, local);
        assert_eq!(pixel(&image, 64, 64)[0] > 200, !local);
        assert_eq!(pixel(&image, 96, 96)[0], 0);
        save(
            if local {
                "local-moved-up-right"
            } else {
                "world-keeps-birth-position"
            },
            &image,
        );
    }
}

#[test]
fn particle_render_gpu_depth_sort_and_alpha_additive_have_red_oracle() {
    let author = r#"fn particle_init(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {var q=p;q.position.z=select(0.2,0.8,c.particle_id==1u);q.color=select(vec4<f32>(1.0,0.0,0.0,0.5),vec4<f32>(0.0,0.0,1.0,0.5),c.particle_id==1u);return q;}
fn particle_update(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {return p;}"#;
    for (name, blend, reverse, expected) in [
        (
            "sorted-alpha",
            ParticleBlend::Alpha,
            false,
            [128, 0, 64, 255],
        ),
        (
            "wrong-sort-direction",
            ParticleBlend::Alpha,
            true,
            [64, 0, 128, 255],
        ),
        (
            "additive",
            ParticleBlend::Additive,
            false,
            [128, 0, 128, 255],
        ),
    ] {
        let mut desc = effect(ParticleGeometry::Billboard3d {}).description;
        desc.emitters[0].bursts[0].count = 2;
        desc.emitters[0].draw.sort = ParticleSort::BackToFront;
        desc.emitters[0].draw.blend = blend;
        let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
        backend
            .install_particle_effect(1, &cook(desc, author))
            .unwrap();
        let mut command = draw();
        if reverse {
            if let RenderPassCommand::DrawParticles { view, .. } = &mut command {
                view.forward = [0., 0., -1.].map(Into::into);
            }
        }
        let image = backend
            .render_plan_to_rgba_bytes(&plan(vec![step(1), command]), 128, 128)
            .unwrap();
        close(pixel(&image, 64, 64), expected);
        save(name, &image);
        if reverse {
            assert!(
                pixel(&image, 64, 64)[0].abs_diff(128) > 20,
                "same alpha oracle must reject wrong GPU sort direction"
            );
        }
    }
}

#[test]
fn particle_render_small_sort_matches_large_sort_with_partial_live_and_reversed_view() {
    let author = r#"fn particle_init(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {
var q=p;let rank=(c.particle_id*17u)%67u;let depth=0.1+f32(rank)*0.01;
q.position=vec3<f32>(0.0,0.0,depth);q.color=vec4<f32>(depth,0.0,1.0-depth,0.2);return q;}
fn particle_update(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {return p;}"#;
    for live in [1, 17, 36, 63, 64] {
        for reverse in [false, true] {
            let mut pixels = Vec::new();
            for capacity in [64, 65] {
                let mut desc = effect(ParticleGeometry::Billboard2d {}).description;
                desc.budget.max_particles = 65;
                desc.emitters[0].capacity = capacity;
                desc.emitters[0].bursts[0].count = live;
                desc.emitters[0].draw.sort = ParticleSort::BackToFront;
                desc.emitters[0].draw.blend = ParticleBlend::Alpha;
                let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
                backend
                    .install_particle_effect(1, &cook(desc, author))
                    .unwrap();
                let mut command = draw();
                if reverse {
                    if let RenderPassCommand::DrawParticles { view, .. } = &mut command {
                        view.forward = [0., 0., -1.].map(Into::into);
                    }
                }
                let image = backend
                    .render_plan_to_rgba_bytes(&plan(vec![step(1), command]), 128, 128)
                    .unwrap();
                pixels.push(pixel(&image, 64, 64));
            }
            assert_eq!(pixels[0], pixels[1], "live={live} reverse={reverse}");
        }
    }
}

#[test]
fn particle_render_flipbook_and_texture_vertical_orientation() {
    let texture = handle(crate::render_resource::RenderResourceKind::Texture);
    let mut desc = effect(ParticleGeometry::Billboard2d {}).description;
    desc.emitters[0].draw.texture = Some(asset("texture"));
    desc.emitters[0].color = [1.; 4];
    desc.emitters[0].draw.flipbook = Some(ParticleFlipbook {
        columns: 2,
        rows: 2,
        fps: 64.,
    });
    let colors = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
    ];
    let rgba: Vec<u8> = (0..4)
        .flat_map(|y| (0..4).flat_map(move |x| colors[(y / 2) * 2 + x / 2]))
        .collect();
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .register_rgba8_texture(texture, 4, 4, &rgba, "nearestClamp")
        .unwrap();
    backend
        .install_particle_effect(1, &cook(desc, program::DEFAULT_BEHAVIOR))
        .unwrap();
    let mut command = draw();
    if let RenderPassCommand::DrawParticles { texture: t, .. } = &mut command {
        *t = Some(texture);
    }
    for id in 1..=4 {
        let image = backend
            .render_plan_to_rgba_bytes(&plan(vec![step(id), command.clone()]), 128, 128)
            .unwrap();
        close(pixel(&image, 64, 64), colors[id as usize - 1]);
        save(&format!("flipbook-{id}"), &image);
    }
    let mut desc = effect(ParticleGeometry::Billboard2d {}).description;
    desc.emitters[0].draw.texture = Some(asset("texture"));
    desc.emitters[0].color = [1.; 4];
    backend.remove_particle_effect(1);
    backend
        .install_particle_effect(1, &cook(desc, program::DEFAULT_BEHAVIOR))
        .unwrap();
    let image = backend
        .render_plan_to_rgba_bytes(&plan(vec![step(1), command]), 128, 128)
        .unwrap();
    assert!(pixel(&image, 54, 54)[0] > 200);
    assert!(pixel(&image, 54, 74)[2] > 200);
    save("texture-top-red-bottom-blue", &image);
    close(pixel(&image, 63, 54), [255, 0, 0, 255]);
}

#[test]
fn particle_render_mesh_is_real_uploaded_geometry_and_rotates() {
    let mesh = handle(crate::render_resource::RenderResourceKind::MeshBuffer);
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .register_particle_mesh(
            mesh,
            &[[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [-0.5, 0.5, 0.0]],
            &[[0., 1.], [1., 1.], [0., 0.]],
            &[0, 1, 2],
        )
        .unwrap();
    for rotated in [false, true] {
        let mut desc = effect(ParticleGeometry::Mesh {
            asset: asset("mesh"),
        })
        .description;
        desc.emitters[0].size_meters = [0.8; 2];
        desc.emitters[0].rotation_radians = if rotated {
            std::f32::consts::FRAC_PI_2
        } else {
            0.
        };
        backend
            .install_particle_effect(1, &cook(desc, program::DEFAULT_BEHAVIOR))
            .unwrap();
        let mut command = draw();
        if let RenderPassCommand::DrawParticles { mesh: m, .. } = &mut command {
            *m = Some(mesh);
        }
        let image = backend
            .render_plan_to_rgba_bytes(&plan(vec![step(1), command]), 128, 128)
            .unwrap();
        assert_eq!(pixel(&image, 82, 45)[0] > 200, rotated);
        if !rotated {
            assert!(pixel(&image, 45, 75)[0] > 200);
        }
        save(
            if rotated {
                "mesh-rotated"
            } else {
                "mesh-asymmetric"
            },
            &image,
        );
        backend.remove_particle_effect(1);
    }
}

#[test]
fn particle_render_scene_depth_occludes_particles_without_covering_background() {
    let mut desc = effect(ParticleGeometry::Billboard3d {}).description;
    desc.emitters[0].size_meters = [1.4; 2];
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .install_particle_effect(1, &cook(desc, program::DEFAULT_BEHAVIOR))
        .unwrap();
    let image = backend
        .render_plan_to_rgba_bytes(
            &plan(vec![
                step(1),
                RenderPassCommand::DrawTestGeometry {
                    target: "out".into(),
                    vertex_count: 3,
                },
                draw(),
            ]),
            128,
            128,
        )
        .unwrap();
    let center = pixel(&image, 64, 64);
    assert!(
        center[1] > 30 && center[0] < 230,
        "depth failed: {center:?}"
    );
    close(pixel(&image, 95, 64), [255, 0, 0, 255]);
    save("scene-depth-occluder", &image);
}

#[test]
fn particle_render_trail_records_curve_once_per_step_and_expires() {
    let author = r#"fn particle_init(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {var q=p;q.position=vec3<f32>(-0.5,-0.3,0.5);return q;}
fn particle_update(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {var q=p;q.position=vec3<f32>(-0.5+q.age*8.0,-0.3+q.age*q.age*60.0,0.5);return q;}"#;
    for short in [false, true] {
        let mut desc = effect(ParticleGeometry::Billboard3d {}).description;
        desc.emitters[0].size_meters = [0.04; 2];
        desc.emitters[0].draw.trail = Some(ParticleTrail {
            segments: 8,
            lifetime_seconds: if short { 0.025 } else { 0.2 },
            width_meters: 0.06,
        });
        let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
        backend
            .install_particle_effect(1, &cook(desc, author))
            .unwrap();
        let bytes = backend.particle_buffer_bytes(1);
        let mut commands: Vec<_> = (1..=8).map(step).collect();
        commands.push(draw());
        let commands = plan(commands);
        let image = backend
            .render_plan_to_rgba_bytes(&commands, 128, 128)
            .unwrap();
        assert_eq!(
            pixel(&image, 48, 79)[0] > 20,
            !short,
            "trail history/lifetime: {:?}",
            pixel(&image, 48, 79)
        );
        save(
            if short {
                "trail-expired-tail"
            } else {
                "trail-curved-history"
            },
            &image,
        );
        let repeat = backend
            .render_plan_to_rgba_bytes(&commands, 128, 128)
            .unwrap();
        assert_eq!(image, repeat);
        assert_eq!(backend.particle_buffer_bytes(1), bytes);
        backend.clear_particle_effect(1).unwrap();
        let cleared = backend
            .render_plan_to_rgba_bytes(&plan(vec![draw()]), 128, 128)
            .unwrap();
        assert!(cleared.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]));
        let restarted = backend
            .render_plan_to_rgba_bytes(&plan(vec![step(9), draw()]), 128, 128)
            .unwrap();
        assert_eq!(
            pixel(&restarted, 48, 79)[0],
            0,
            "restart must not connect old trail"
        );
    }
}

#[test]
fn particle_render_recycled_child_slot_does_not_join_previous_trail() {
    let mut desc = effect(ParticleGeometry::Billboard3d {}).description;
    let mut child = desc.emitters[0].clone();
    child.name = "child".into();
    child.capacity = 1;
    child.bursts.clear();
    child.lifetime_seconds = [1.0 / 64.0; 2];
    child.size_meters = [0.02; 2];
    child.draw.trail = Some(ParticleTrail {
        segments: 4,
        lifetime_seconds: 1.0,
        width_meters: 0.06,
    });
    desc.emitters[0].collisions = vec![ParticleCollision {
        shape: ParticleCollider::Plane {
            normal: [0., 1., 0.],
            distance: 0.1,
        },
        restitution: 0.0,
        kill: false,
    }];
    desc.emitters[0].child_emission = vec![ParticleChildEmission {
        event: ParticleEvent::Collision,
        target_emitter: "child".into(),
        count: 1,
        max_generation: 1,
    }];
    desc.emitters.push(child);
    let author = r#"fn particle_init(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {return p;}
fn particle_update(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {var q=p;q.position.x+=0.1;q.position.y=0.0;return q;}"#;
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .install_particle_effect(1, &cook(desc, author))
        .unwrap();
    let mut command = draw();
    if let RenderPassCommand::DrawParticles { emitter, .. } = &mut command {
        *emitter = 1;
    }
    let mut commands: Vec<_> = (1..=4).map(step).collect();
    commands.push(command);
    let image = backend
        .render_plan_to_rgba_bytes(&plan(commands), 128, 128)
        .unwrap();
    save("recycled-child-no-old-trail", &image);
    assert_eq!(
        pixel(&image, 73, 57)[0],
        0,
        "a new child must not inherit the old child's trail even when its random seed repeats"
    );
}

#[test]
fn particle_render_perspective_projection_preserves_depth_size() {
    let mut desc = effect(ParticleGeometry::Billboard3d {}).description;
    desc.emitters[0].bursts[0].count = 2;
    desc.emitters[0].size_meters = [0.4; 2];
    let author = r#"fn particle_init(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {var q=p;q.position=select(vec3<f32>(-0.5,0.0,-1.0),vec3<f32>(0.5,0.0,-3.0),c.particle_id==1u);q.color=select(vec4<f32>(1.0,0.0,0.0,1.0),vec4<f32>(0.0,0.0,1.0,1.0),c.particle_id==1u);return q;}
fn particle_update(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {return p;}"#;
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .install_particle_effect(1, &cook(desc, author))
        .unwrap();
    let mut command = draw();
    if let RenderPassCommand::DrawParticles { view, .. } = &mut command {
        view.world_to_clip = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., -10.0 / 9.9, -1.],
            [0., 0., -1.0 / 9.9, 0.],
        ]
        .map(|v| v.map(Into::into));
        view.forward = [0., 0., -1.].map(Into::into);
    }
    let image = backend
        .render_plan_to_rgba_bytes(&plan(vec![step(1), command]), 128, 128)
        .unwrap();
    close(pixel(&image, 20, 64), [255, 0, 0, 255]);
    close(pixel(&image, 75, 64), [0, 0, 255, 255]);
    close(pixel(&image, 85, 64), [0, 0, 0, 255]);
    let red = (0..128).filter(|x| pixel(&image, *x, 64)[0] > 200).count();
    let blue = (0..128).filter(|x| pixel(&image, *x, 64)[2] > 200).count();
    assert!(
        red >= 24 && blue <= 10,
        "near width={red}, far width={blue}"
    );
    save("perspective-near-large-far-small", &image);
}

#[test]
fn particle_render_existing_ui_composition_remains_above_world() {
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .install_particle_effect(1, &effect(ParticleGeometry::Billboard3d {}))
        .unwrap();
    let texture = handle(crate::render_resource::RenderResourceKind::Texture);
    backend
        .register_rgba8_texture(texture, 1, 1, &[255; 4], "nearestClamp")
        .unwrap();
    let vertices = [
        [-0.2, -0.2],
        [0.2, -0.2],
        [0.2, 0.2],
        [-0.2, -0.2],
        [0.2, 0.2],
        [-0.2, 0.2],
    ]
    .into_iter()
    .map(|p| RenderDrawVertex::new(p, [0., 1., 0., 0.5], [0.5, 0.5]))
    .collect();
    let ui = RenderPassCommand::DrawUiComposition {
        target: "out".into(),
        stage: "AfterWorld".into(),
        item_count: 1,
        text_count: 0,
        image_count: 1,
        glyph_count: 0,
        font_atlas_id: None,
        text_pass_inserted: false,
        debug_label: "ui-composition".into(),
        texture: Some(texture),
        font_render_mode: None,
        font_page_index: None,
        vertices,
    };
    let image = backend
        .render_plan_to_rgba_bytes(&plan(vec![step(1), draw(), ui]), 128, 128)
        .unwrap();
    close(pixel(&image, 64, 64), [127, 128, 0, 255]);
    save("ui-after-particles", &image);
}

#[test]
fn particle_render_multiple_views_keep_ordered_uniforms_without_resimulation() {
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    backend
        .install_particle_effect(1, &effect(ParticleGeometry::Billboard3d {}))
        .unwrap();
    let mut left = draw();
    let mut right = draw();
    if let RenderPassCommand::DrawParticles { view, .. } = &mut left {
        view.world_to_clip[3][0] = (-0.5).into();
    }
    if let RenderPassCommand::DrawParticles { view, .. } = &mut right {
        view.world_to_clip[3][0] = 0.5.into();
    }
    let image = backend
        .render_plan_to_rgba_bytes(&plan(vec![step(1), left, right]), 128, 128)
        .unwrap();
    close(pixel(&image, 32, 64), [255, 0, 0, 255]);
    close(pixel(&image, 96, 64), [255, 0, 0, 255]);
    close(pixel(&image, 64, 64), [0, 0, 0, 255]);
    assert_eq!(
        backend.capture_particle_state_for_validation(1).unwrap()[0].particles[0].age,
        0.0
    );
    save("two-views-one-simulation", &image);
}

#[test]
fn particle_render_invalid_resources_fail_before_simulation_and_headless_is_unsupported() {
    use crate::engine_rhi::EngineRhiBackend;
    let mut backend = RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let mut desc = effect(ParticleGeometry::Billboard3d {}).description;
    desc.emitters[0].draw.texture = Some(asset("texture"));
    backend
        .install_particle_effect(1, &cook(desc, program::DEFAULT_BEHAVIOR))
        .unwrap();
    let commands = plan(vec![step(1), draw()]);
    assert!(backend
        .render_plan_to_rgba_bytes(&commands, 128, 128)
        .unwrap_err()
        .contains("texture_missing"));
    assert_eq!(
        backend.capture_particle_state_for_validation(1).unwrap()[0].live,
        0
    );
    let mut headless = crate::headless_rhi_backend::HeadlessRhiBackend::new();
    assert!(headless
        .execute_plan(&commands)
        .diagnostics
        .iter()
        .any(|d| d.code == "particle_render.unsupported_backend"));
    let mut view = ParticleRenderView::default();
    view.up = view.right;
    assert!(view.validate().is_err());
}
