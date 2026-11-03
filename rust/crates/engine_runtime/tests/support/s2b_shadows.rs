use super::*;
use engine_runtime::render_graph::*;
use std::sync::Arc;

fn depth_graph() -> RenderGraph {
    let mut manager = RenderResourceManager::new();
    let source = geometry_fixture::source(&mut manager);
    let mut frame = geometry_fixture::frame(&source, &manager);
    let mut graph = geometry_fixture::graph(&frame);
    let shadow = &mut frame.views[0];
    shadow.view.view_id = 100;
    shadow.view.view_kind = ViewKind::Shadow;
    shadow.view.target_kind = TargetKind::ShadowMap;
    shadow.view.clear_color = None;
    shadow.view.depth_clear = Some(1f32.into());
    shadow.depth_target = Some("shadow-atlas".into());
    for draw in &mut shadow.draws {
        let draw = Arc::make_mut(draw);
        draw.view = Arc::new(shadow.view.clone());
        draw.target = "shadow-atlas".into();
        draw.depth_target = shadow.depth_target.as_deref().map(Into::into);
    }
    graph.views.push(RenderGraphView {
        view_id: "geometry-view-100".into(),
        view_kind: "shadow".into(),
        width: 64,
        height: 64,
        clear_color: color([0.; 4]),
    });
    graph.resources.push(RenderResource {
        resource_id: "shadow-atlas".into(),
        resource_name: "shadow-atlas".into(),
        resource_kind: engine_runtime::render_graph::RenderResourceKind::Texture,
        format: RenderResourceFormat::Depth32Float,
        size: RenderResourceSize {
            width: 64,
            height: 64,
        },
        usage: vec![
            RenderResourceUsage::DepthTarget,
            RenderResourceUsage::Sampled,
        ],
        lifetime: RenderResourceLifetime::FrameLocal,
    });
    let pass = |id: &str, kind, command| RenderPass {
        pass_id: id.into(),
        pass_name: id.into(),
        pass_kind: kind,
        view_id: "geometry-view-100".into(),
        reads: vec![],
        writes: vec!["shadow-atlas".into()],
        color_targets: vec![],
        depth_target: shadow.depth_target.clone(),
        commands: vec![command].into(),
        debug_source: None,
    };
    graph.passes.insert(
        1,
        pass(
            "shadow-begin",
            RenderPassKind::BeginView,
            RenderPassCommand::BeginView {
                target: "shadow-atlas".into(),
                view: shadow.view.clone().into(),
                depth_target: shadow.depth_target.clone(),
            },
        ),
    );
    graph.passes.insert(
        2,
        pass(
            "shadow-draw",
            RenderPassKind::DrawGeometry,
            RenderPassCommand::DrawGeometry {
                draw: shadow.draws[0].clone(),
            },
        ),
    );
    graph
}

#[test]
fn shadow_depth_writer_is_a_real_depth_only_graph_phase() {
    let graph = depth_graph();
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
}

#[test]
fn shadow_caster_depth_is_independent_of_unlit_receiver_depth_mode() {
    for depth in [MaterialDepth::Disabled, MaterialDepth::TestOnly] {
        let mut manager = RenderResourceManager::new();
        let mesh = geometry_fixture::prepare(
            "mesh",
            RenderAssetData::Mesh(geometry_fixture::triangle()),
            &mut manager,
        );
        let mut material = geometry_fixture::unlit([1.; 4]);
        material.depth = depth;
        let material = geometry_fixture::prepare(
            "material",
            RenderAssetData::Material(material),
            &mut manager,
        );
        let source = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
        let main = geometry_fixture::frame(&source, &manager);
        assert_eq!(main.views[0].draws[0].depth, depth);
        let mut view = RenderView::perspective(100, 64, 64, 60., 0.1, 10.);
        view.view_kind = ViewKind::Shadow;
        view.target = TargetKind::ShadowMap;
        view.clear_policy = ClearPolicy::Depth { depth: 1. };
        let shadow = GeometryFrame::build(
            "atlas",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &[geometry_fixture::instance(&source, 1, [0., 0., -2.])],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        assert_eq!(shadow.views[0].draws[0].depth, MaterialDepth::TestAndWrite);
        let mut graph = geometry_fixture::graph(&main);
        shadow.append_to_graph(&mut graph);
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    }
}

#[cfg(feature = "real-wgpu")]
pub(super) fn sampled_graph(alpha: f32, mask: bool) -> RenderGraph {
    use engine_runtime::render_projection_adapter::LightProjection;
    use engine_runtime::spatial_contract::{Light3D, LightKind};
    let mut manager = RenderResourceManager::new();
    let mesh = geometry_fixture::prepare(
        "shadow-mesh",
        RenderAssetData::Mesh(standard_lit_fixture::lit_triangle()),
        &mut manager,
    );
    let material = geometry_fixture::prepare(
        "receiver",
        RenderAssetData::Material(standard_lit_fixture::standard_lit([0.5, 0.5, 0.5, 1.])),
        &mut manager,
    );
    let receiver = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
    let caster_material = geometry_fixture::prepare(
        "caster",
        RenderAssetData::Material(if mask {
            masked(alpha, 0.5)
        } else {
            geometry_fixture::unlit([1., 0., 0., 1.])
        }),
        &mut manager,
    );
    let caster = PreparedGeometry::from_assets(&mesh, &caster_material, 0, &manager).unwrap();
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let mut receiver_instance = geometry_fixture::instance(&receiver, 1, [0., 0., -3.]);
    receiver_instance.world_transform[0] = 2.;
    receiver_instance.world_transform[5] = 2.;
    let mut main = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &[receiver_instance],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let lights = vec![LightProjection {
        component: Light3D {
            cast_shadows: false,
            schema_version: "light3d.v1".into(),
            enabled: true,
            color: [1.; 3],
            kind: LightKind::Directional {
                illuminance_lux: 1.,
            },
            layers: vec![],
        },
        position: [0.; 3],
        direction: [0., 0., -1.],
        origin: [0.; 3],
    }];
    main.set_view_lights(1, &lights).unwrap();
    let mut shadow_view = view.clone();
    shadow_view.view_id = 100;
    shadow_view.view_kind = ViewKind::Shadow;
    shadow_view.target = TargetKind::ShadowMap;
    shadow_view.clear_policy = ClearPolicy::Depth { depth: 1. };
    let mut instance = geometry_fixture::instance(&caster, 2, [0., 0., -2.]);
    instance.world_transform[0] = 0.5;
    instance.world_transform[5] = 0.5;
    let shadow = GeometryFrame::build(
        "shadow-atlas",
        [64, 64],
        &[GeometryViewInput {
            view: &shadow_view,
            pose: CameraPose::default(),
        }],
        &[instance],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let projection = ShadowProjection {
        atlas: "shadow-atlas".into(),
        slot: 0,
        light_index: 0,
        matrix: shadow.views[0].view.view_projection().map(Into::into),
        viewport: [0, 0, 64, 64],
        extent: [64, 64],
        depth_range: [0.1.into(), 10f32.into()],
    };
    let draw = Arc::make_mut(&mut main.views[0].draws[0]);
    draw.shadows = Some(
        ShadowMaps::build(
            &draw.view,
            draw.lighting.as_ref().unwrap(),
            vec![projection],
        )
        .unwrap(),
    );
    let mut combined = main.clone();
    combined.views = shadow.views;
    combined.views.extend(main.views);
    geometry_fixture::graph(&combined)
}
#[cfg(feature = "real-wgpu")]
fn sampled_plan(alpha: f32, mask: bool) -> engine_runtime::rhi_command_plan::RhiCommandPlan {
    let graph = sampled_graph(alpha, mask);
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    plan
}

#[test]
#[cfg(feature = "real-wgpu")]
fn mixed_surface_readiness_does_not_repeat_identical_shadow_uploads() {
    use engine_runtime::rhi_command_plan::RhiCommand;
    use engine_runtime::wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend};
    let mut plan = sampled_plan(1., false);
    plan.commands
        .push(RhiCommand::ReconcileParticles { sources: vec![] });
    let shadow_bytes = plan
        .commands
        .iter()
        .find_map(|command| {
            command
                .geometry_draw()?
                .shadows
                .as_ref()
                .map(|maps| maps.words().len() * 4)
        })
        .unwrap();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut backend = RealWgpuBackend::from_shared_device_queue(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        64,
        64,
        "mixed shadow admission",
    );
    let expected = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    // Keep the same native atlas/geometry but remove its receiver binding.
    // The next readiness call must upload a new shadow binding exactly once.
    let mut without_shadows = plan.clone();
    for command in &mut without_shadows.commands {
        if let RhiCommand::Draw {
            payload: engine_runtime::rhi_command_plan::RhiDrawPayload::Geometry { draw },
            ..
        } = command
        {
            Arc::make_mut(draw).shadows = None;
        }
    }
    backend
        .render_plan_to_rgba_bytes(&without_shadows, 64, 64)
        .unwrap();
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            resident_bytes: 128 * 1024 * 1024,
            staging_bytes: 4096,
            bytes_per_frame: shadow_bytes,
            max_tickets: 8,
        })
        .unwrap();
    backend.bind_geometry_upload_presentation(1).unwrap();
    assert!(backend
        .prepare_surface_resources(&plan, [64, 64], None)
        .unwrap());
    let admitted = backend.geometry_upload_snapshot().frame_uploaded_bytes;
    assert_eq!(
        admitted, shadow_bytes,
        "new receiver must upload before readiness succeeds"
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mixed-shadow-output"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let report =
        backend.execute_plan_to_surface_view(&plan, &target.create_view(&Default::default()));
    eprintln!(
        "mixed surface shadow budget={shadow_bytes}, readiness writes={admitted}, draw={:?}",
        report.diagnostics
    );
    assert!(
        report
            .diagnostics
            .iter()
            .all(|d| d.severity != engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity::Error),
        "ready frame rejected during draw: {:?}",
        report.diagnostics
    );
    assert_eq!(
        backend.geometry_upload_snapshot().frame_uploaded_bytes,
        shadow_bytes,
        "draw must consume the same successful readiness upload"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    let uploaded_before_replay = backend.geometry_upload_snapshot().uploaded_bytes;
    backend.bind_geometry_upload_presentation(2).unwrap();
    assert_eq!(
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap(),
        expected
    );
    assert_eq!(
        backend.geometry_upload_snapshot().uploaded_bytes,
        uploaded_before_replay,
        "unchanged shadow parameters and actual depth views need no second upload"
    );
    drop(target);
    let terminal = backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(terminal.retiring_bytes, 0);
    assert_eq!(backend.geometry_upload_snapshot().staging_bytes, 0);
}

#[test]
#[cfg(feature = "real-wgpu")]
fn shadow_validation_reuse_checks_replacement_lights_in_the_same_graph() {
    use engine_runtime::render_projection_adapter::LightProjection;
    use engine_runtime::spatial_contract::{Light3D, LightKind};
    let mut graph = sampled_graph(1., false);
    let baseline = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!baseline.has_errors(), "{:?}", baseline.diagnostics);
    let pass = graph
        .passes
        .iter_mut()
        .find(|pass| {
            pass.commands.iter().any(|command| {
            matches!(command, RenderPassCommand::DrawGeometry { draw } if draw.shadows.is_some())
        })
        })
        .unwrap();
    let original = pass
        .commands
        .iter()
        .find_map(|command| match command {
            RenderPassCommand::DrawGeometry { draw } if draw.shadows.is_some() => {
                Some(draw.clone())
            }
            _ => None,
        })
        .unwrap();
    let mut changed = original.as_ref().clone();
    changed.lighting = Some(
        ClusteredLights::build(
            &changed.view,
            &[LightProjection {
                component: Light3D {
                    cast_shadows: false,
                    schema_version: "light3d.v1".into(),
                    enabled: true,
                    color: [1.; 3],
                    kind: LightKind::Point {
                        intensity_candela: 1.,
                        range_m: 10.,
                    },
                    layers: vec![],
                },
                position: [0.; 3],
                direction: [0., 0., -1.],
                origin: [0.; 3],
            }],
        )
        .unwrap(),
    );
    assert!(
        changed.validate().is_err(),
        "standalone draw must reject directional shadow bound to point light"
    );
    assert!(Arc::ptr_eq(
        changed.shadows.as_ref().unwrap(),
        original.shadows.as_ref().unwrap()
    ));
    let mut valid = graph.clone();
    let valid_pass = valid
        .passes
        .iter_mut()
        .find(|pass| {
            pass.commands.iter().any(|command| {
            matches!(command, RenderPassCommand::DrawGeometry { draw } if draw.shadows.is_some())
        })
        })
        .unwrap();
    valid_pass.commands.push(RenderPassCommand::DrawGeometry {
        draw: original.clone(),
    });
    let valid_plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&valid);
    assert!(
        !valid_plan.has_errors(),
        "valid repeated receiver rejected: {:?}",
        valid_plan.diagnostics
    );
    let pass = graph
        .passes
        .iter_mut()
        .find(|pass| {
            pass.commands.iter().any(|command| {
            matches!(command, RenderPassCommand::DrawGeometry { draw } if draw.shadows.is_some())
        })
        })
        .unwrap();
    pass.commands.push(RenderPassCommand::DrawGeometry {
        draw: Arc::new(changed),
    });
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(
        plan.has_errors() && format!("{:?}", plan.diagnostics).contains("shadow.light_kind"),
        "same shadow pointer hid replacement light kind: {:?}",
        plan.diagnostics
    );
}
#[test]
#[cfg(feature = "real-wgpu")]
fn gpu_depth_only_writer_comparison_and_mask_have_independent_pixels() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let plan = sampled_plan(1., false);
    let mut baseline = plan.clone();
    for command in &mut baseline.commands {
        if let engine_runtime::rhi_command_plan::RhiCommand::Draw {
            payload: engine_runtime::rhi_command_plan::RhiDrawPayload::Geometry { draw },
            ..
        } = command
        {
            Arc::make_mut(draw).shadows = None;
        }
    }
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let lit = backend
        .render_plan_to_rgba_bytes(&baseline, 64, 64)
        .unwrap();
    let shaded = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    let i = (32 * 64 + 32) * 4;
    assert!(lit[i + 1] > 30, "baseline {:?}", &lit[i..i + 4]);
    assert!(
        // The fixture deliberately emits red. Compare the green direct-light
        // channel; saturated red cannot measure shadow attenuation, and u8
        // addition used to overflow under debug assertions.
        u16::from(shaded[i + 1]) + 20 < u16::from(lit[i + 1]),
        "no depth comparison: lit {:?} shadow {:?}",
        &lit[i..i + 4],
        &shaded[i..i + 4]
    );
    assert_eq!(
        &shaded[i..i + 4],
        &[255, 0, 0, 255],
        "shadow must suppress direct light while preserving red emission"
    );
    let outside = (40 * 64 + 20) * 4;
    assert_eq!(
        &lit[outside..outside + 4],
        &shaded[outside..outside + 4],
        "shadow leaked outside caster"
    );
    assert_pcf_reference(&lit, &shaded);
    let holes = backend
        .render_plan_to_rgba_bytes(&sampled_plan(0., true), 64, 64)
        .unwrap();
    assert_eq!(
        &lit[i..i + 4],
        &holes[i..i + 4],
        "mask discarded texels still cast shadows"
    );
    let solid = backend
        .render_plan_to_rgba_bytes(&sampled_plan(1., true), 64, 64)
        .unwrap();
    assert_eq!(&shaded[i..i + 4], &solid[i..i + 4]);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[cfg(feature = "real-wgpu")]
fn gpu_opaque_shadow_batch_preserves_each_instance_depth_coverage() {
    use engine_runtime::rhi_command_plan::{compile_render_graph_to_rhi_plan, RhiCommandPlan};
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let make_plan = |offsets: &[f32]| -> RhiCommandPlan {
        let mut graph = sampled_graph(1., false);
        let pass = graph
            .passes
            .iter_mut()
            .find(|p| {
                p.commands.iter().any(|c| {
                    matches!(
                        c, RenderPassCommand::DrawGeometry { draw } if draw.view.is_shadow_map()
                    )
                })
            })
            .unwrap();
        let template = pass
            .commands
            .iter()
            .find_map(|c| match c {
                RenderPassCommand::DrawGeometry { draw } => Some(draw.as_ref().clone()),
                _ => None,
            })
            .unwrap();
        pass.commands = offsets
            .iter()
            .enumerate()
            .map(|(i, x)| {
                let mut draw = template.clone();
                draw.instance_id = 10 + i as u64;
                draw.world_transform[12] = (*x).into();
                Arc::make_mut(&mut draw.material_binding).binding_id = format!("color-{i}");
                draw.base_color = color([i as f32, 0., 1., 1.]);
                RenderPassCommand::DrawGeometry {
                    draw: Arc::new(draw),
                }
            })
            .collect();
        let plan = compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        plan
    };
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let left = backend
        .render_plan_to_rgba_bytes(&make_plan(&[-0.45]), 64, 64)
        .unwrap();
    let right = backend
        .render_plan_to_rgba_bytes(&make_plan(&[0.45]), 64, 64)
        .unwrap();
    let before = backend.retained_geometry_stats();
    let both = backend
        .render_plan_to_rgba_bytes(&make_plan(&[-0.45, 0.45]), 64, 64)
        .unwrap();
    let after = backend.retained_geometry_stats();
    assert_eq!(
        after.draws - before.draws,
        2,
        "one shadow batch plus receiver"
    );
    assert_eq!(after.instances - before.instances, 3);
    let mut different = 0;
    for ((a, b), actual) in left
        .chunks_exact(4)
        .zip(right.chunks_exact(4))
        .zip(both.chunks_exact(4))
    {
        different += usize::from(a != b);
        for channel in 0..4 {
            assert!(
                actual[channel].abs_diff(a[channel].min(b[channel])) <= 1,
                "combined shadow must equal independent nearest-depth coverage"
            );
        }
    }
    assert!(
        different > 20,
        "reference casters must cover distinct pixels"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
fn shadow_atlas_load_needs_initialization_and_invalidates_schedule_cache() {
    let graph = depth_graph();
    let mut cache = GraphScheduleCache::default();
    cache.compile(&graph).unwrap();
    let mut changed = graph.clone();
    for command in &mut changed.passes[1].commands {
        if let RenderPassCommand::BeginView { view, .. } = command {
            Arc::make_mut(view).depth_clear = None;
        }
    }
    let errors = cache.compile(&changed).unwrap_err();
    assert!(
        errors.iter().any(|d| d.code == "graph.uninitialized_read"),
        "{errors:?}"
    );
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&changed);
    assert!(plan.has_errors());
}
#[test]
fn shadow_writer_rejects_color_dummy_wrong_format_and_missing_depth() {
    for fault in 0..4 {
        let mut graph = depth_graph();
        match fault {
            0 => graph.passes[1].color_targets.push("main".into()),
            1 => {
                graph
                    .resources
                    .iter_mut()
                    .find(|r| r.resource_id == "shadow-atlas")
                    .unwrap()
                    .format = RenderResourceFormat::Rgba8Unorm
            }
            2 => {
                graph
                    .resources
                    .iter_mut()
                    .find(|r| r.resource_id == "shadow-atlas")
                    .unwrap()
                    .usage = vec![RenderResourceUsage::DepthTarget]
            }
            _ => graph.passes[1].depth_target = None,
        }
        assert!(
            engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph).has_errors(),
            "fault {fault}"
        );
    }
}
#[cfg(feature = "real-wgpu")]
fn assert_pcf_reference(lit: &[u8], shaded: &[u8]) {
    // Independent f64 triangle coverage at the nine nearest depth texels.
    // The receiver lies at z=-3 and normal bias moves it toward the light by 0.01m.
    let half = 0.4 / (2. * (std::f64::consts::PI / 6.).tan()) * 32.;
    let vertices = [
        [32. - half, 32. + half],
        [32. + half, 32. + half],
        [32., 32. - half],
    ];
    let inside = |p: [f64; 2]| {
        let cross = |a: [f64; 2], b: [f64; 2]| {
            (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        };
        let e = [
            cross(vertices[0], vertices[1]),
            cross(vertices[1], vertices[2]),
            cross(vertices[2], vertices[0]),
        ];
        e.iter().all(|e| *e <= 0.) || e.iter().all(|e| *e >= 0.)
    };
    let mut fractional = 0;
    for y in 0..64 {
        for x in 0..64 {
            let i = (y * 64 + x) * 4;
            if lit[i + 1] == 0 {
                continue;
            }
            let sx = (x as f64 + 0.5 - 32.) * 3. / 2.99 + 32.;
            let sy = (y as f64 + 0.5 - 32.) * 3. / 2.99 + 32.;
            let mut occluded = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if inside([
                        (sx + dx as f64).floor().clamp(0., 63.) + 0.5,
                        (sy + dy as f64).floor().clamp(0., 63.) + 0.5,
                    ]) {
                        occluded += 1;
                    }
                }
            }
            let expected = (lit[i + 1] as f64 * (9 - occluded) as f64 / 9.).round() as u8;
            assert!(
                shaded[i + 1].abs_diff(expected) <= 2,
                "PCF reference at ({x},{y}): actual {}, expected {expected}, occluded {occluded}",
                shaded[i + 1]
            );
            if occluded > 0 && occluded < 9 {
                fractional += 1;
            }
        }
    }
    assert!(fractional > 20, "reference did not exercise PCF edges");
}

#[cfg(feature = "real-wgpu")]
fn tiled_graph() -> RenderGraph {
    let mut graph = sampled_graph(1., false);
    let find = |kind, id| {
        graph
            .passes
            .iter()
            .find(|p| p.pass_kind == kind && p.view_id == format!("geometry-view-{id}"))
            .unwrap()
            .clone()
    };
    let mut first_begin = find(RenderPassKind::BeginView, 100);
    let mut first_draw = find(RenderPassKind::DrawGeometry, 100);
    let main_begin = find(RenderPassKind::BeginView, 1);
    let mut main_draw = find(RenderPassKind::DrawGeometry, 1);
    let RenderPassCommand::BeginView { view, .. } = &mut first_begin.commands[0] else {
        panic!()
    };
    Arc::make_mut(view).viewport = [0, 0, 32, 64];
    let first_view = view.clone();
    let RenderPassCommand::DrawGeometry { draw } = &mut first_draw.commands[0] else {
        panic!()
    };
    Arc::make_mut(draw).view = first_view;
    let mut second_begin = first_begin.clone();
    second_begin.pass_id = "second-shadow-begin".into();
    second_begin.view_id = "geometry-view-101".into();
    let RenderPassCommand::BeginView { view, .. } = &mut second_begin.commands[0] else {
        panic!()
    };
    Arc::make_mut(view).view_id = 101;
    Arc::make_mut(view).viewport = [32, 0, 32, 64];
    Arc::make_mut(view).depth_clear = None;
    let second_view = view.clone();
    let mut second_draw = first_draw.clone();
    second_draw.pass_id = "second-shadow-draw".into();
    second_draw.view_id = "geometry-view-101".into();
    let RenderPassCommand::DrawGeometry { draw } = &mut second_draw.commands[0] else {
        panic!()
    };
    Arc::make_mut(draw).view = second_view;
    for command in &mut main_draw.commands {
        let RenderPassCommand::DrawGeometry { draw } = command else {
            continue;
        };
        let draw = Arc::make_mut(draw);
        let mut projections = draw.shadows.as_ref().unwrap().projections().to_vec();
        projections[0].viewport = [0, 0, 32, 64];
        draw.shadows = Some(
            ShadowMaps::build(&draw.view, draw.lighting.as_ref().unwrap(), projections).unwrap(),
        );
    }
    graph.views.push(RenderGraphView {
        view_id: "geometry-view-101".into(),
        view_kind: "shadow".into(),
        width: 64,
        height: 64,
        clear_color: color([0.; 4]),
    });
    graph.passes = vec![
        graph.passes[0].clone(),
        first_begin,
        first_draw,
        second_begin,
        second_draw,
        main_begin,
        main_draw,
    ];
    graph
}
#[cfg(feature = "real-wgpu")]
#[test]
fn gpu_atlas_load_preserves_written_tiles_and_rejects_overlap_reclear_or_bad_sampling() {
    use engine_runtime::{
        rhi_command_plan::compile_render_graph_to_rhi_plan, wgpu_backend::real::RealWgpuBackend,
    };
    let graph = tiled_graph();
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert!(
        pixels[(32 * 64 + 32) * 4 + 1] < 5,
        "later atlas BeginView erased first tile"
    );
    for fault in 0..5 {
        let mut bad = graph.clone();
        if fault < 2 {
            let RenderPassCommand::BeginView { view, .. } = &mut bad.passes[3].commands[0] else {
                panic!()
            };
            if fault == 0 {
                Arc::make_mut(view).depth_clear = Some(1f32.into());
            } else {
                Arc::make_mut(view).viewport = [0, 0, 32, 64];
            }
        } else if fault == 2 {
            bad.passes.last_mut().unwrap().reads.clear();
        } else if fault == 3 {
            bad.contracts.resources.insert(
                GraphResourceId("shadow-atlas".into()),
                GraphResourceShape {
                    samples: 4,
                    ..Default::default()
                },
            );
        } else {
            bad.passes.swap(4, 6);
        }
        assert!(
            compile_render_graph_to_rhi_plan(&bad).has_errors(),
            "fault {fault}"
        );
    }
    // Even a mutated RHI packet must fail before submit, independently of graph diagnostics.
    let before = backend.last_gpu_submission();
    let mut bad = plan.clone();
    Arc::make_mut(bad.graph_schedule.as_mut().unwrap())
        .resources
        .iter_mut()
        .find(|r| r.resource_id == "shadow-atlas")
        .unwrap()
        .format = RenderResourceFormat::Rgba8Unorm;
    assert!(backend
        .render_plan_to_rgba_bytes(&bad, 64, 64)
        .unwrap_err()
        .contains("shadow.attachment_descriptor"));
    assert_eq!(backend.last_gpu_submission(), before);
    assert_eq!(
        backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap(),
        pixels
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[cfg(feature = "real-wgpu")]
#[test]
fn three_shadow_submissions_keep_atlases_and_parameters_until_actual_completion() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut backend = engine_runtime::wgpu_backend::real::RealWgpuBackend::from_shared_device_queue(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        64,
        64,
        "S2-B in-flight lights",
    );
    let frames = [
        sampled_plan(1., true),
        sampled_plan(0., true),
        sampled_plan(1., true),
    ];
    let textures: Vec<_> = (0..3)
        .map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("S2-B in-flight output"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        })
        .collect();
    for (plan, texture) in frames.iter().zip(&textures) {
        let report =
            backend.execute_plan_to_surface_view(plan, &texture.create_view(&Default::default()));
        assert!(report.diagnostics.is_empty(), "{report:?}");
    }
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("S2-B three outputs"),
        size: 3 * 64 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    for (i, texture) in textures.iter().enumerate() {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: i as u64 * 64 * 256,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
    }
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    device.poll(wgpu::PollType::wait()).unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let size = 64 * 256;
    assert_eq!(&mapped[..size], &mapped[2 * size..3 * size]);
    assert_ne!(&mapped[..size], &mapped[size..2 * size]);
    let offset = (32 * 64 + 32) * 4;
    assert!(mapped[offset + 1] < 5);
    assert!(mapped[size + offset + 1] > 30);
    drop(mapped);
    buffer.unmap();
    let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(0, 0, 0);
    backend.observe_memory(&mut sample);
    assert!(
        sample.allocations.iter().any(|a| a.domain
            == engine_runtime::resource_memory::MemoryDomain::Gpu
            && a.bytes == 176),
        "actual immutable shadow parameter buffer must be observed"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
}
