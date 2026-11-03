use super::*;
use engine_runtime::{
    color_pipeline::*, particle_effect::*, particle_render_contract::*, render_graph::*,
};

fn graph(
    secondary: [u32; 2],
    fog: bool,
    hidden: bool,
    hdr: bool,
    format: &str,
    step: u64,
) -> RenderGraph {
    graph_mode(secondary, fog, hidden, hdr, format, step, false)
}
fn graph_mode(
    secondary: [u32; 2],
    fog: bool,
    hidden: bool,
    hdr: bool,
    format: &str,
    step: u64,
    shared: bool,
) -> RenderGraph {
    let mut manager = RenderResourceManager::new();
    let mesh = geometry_fixture::prepare(
        "views-mesh",
        RenderAssetData::Mesh(geometry_fixture::triangle()),
        &mut manager,
    );
    let mat = geometry_fixture::prepare(
        "views-blue",
        RenderAssetData::Material(geometry_fixture::unlit([0., 0., 1., 1.])),
        &mut manager,
    );
    let prepared = PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).unwrap();
    let mut frame = GeometryFrame {
        target: "main".into(),
        extent: [64, 64],
        views: vec![],
    };
    for (id, extent, target, density, ev) in [
        (1, [64, 64], "main", 0.2, 0.),
        (2, secondary, "aux", 0.8, -1.),
    ] {
        let mut view = RenderView::perspective(id, extent[0], extent[1], 60., 0.1, 10.);
        if id == 2 && !shared {
            view.target = engine_runtime::render_contract::TargetKind::ViewportTexture;
        }
        if shared {
            view.viewport = [if id == 1 { 4 } else { 36 }, 8, 24, 48];
        }
        if hdr {
            view.color_pipeline = Some(ColorPipeline3D {
                exposure_ev: ev.into(),
                anti_aliasing: AntiAliasing3D::None,
                fog: fog.then(|| DistanceFog3D {
                    color: [0., 1., 0.].map(Into::into),
                    density_per_m: density.into(),
                    start_distance_m: 0.0.into(),
                }),
                ..Default::default()
            });
        }
        let z = if id == 2 && hidden { -0.5 } else { -2. };
        let mut part = GeometryFrame::build(
            if shared { "main" } else { target },
            extent,
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &[geometry_fixture::instance(&prepared, 1, [0., 0., z])],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        if id == 2 && !shared {
            let mut output = engine_runtime::runtime_renderer::RenderTarget::viewport_texture(
                target, extent[0], extent[1],
            );
            output.format = format.into();
            part.views[0].color_target = Some(output);
        }
        frame.views.extend(part.views);
    }
    let source=ParticleSourceFrame {
        simulation_origin_bits:[0;3],instance:31,epoch:0,
        assets:serde_json::json!({"effect":s2c_presentation::effect(ParticleSpace::World,0.5),"textures":[null],"meshes":[null],"tints":[[1,1,1,1]]}).to_string().into(),
        parameters:"{}".into(), textures:vec![None],meshes:vec![None],
        step:ParticleRenderStep {emitter_affine: Default::default(),step_id:step,delta_seconds:0.015625.into(),origin:[0.,0.,-1.].map(Into::into),emitting:true,paused:step>1},
    };
    s2c_presentation::assemble(frame, source)
}
#[test]
fn shared_partial_views_compile_without_duplicating_simulation() {
    let p = compile(&graph_mode(
        [64, 64],
        true,
        true,
        true,
        "Rgba8Unorm",
        1,
        true,
    ));
    assert_eq!(
        p.commands
            .iter()
            .filter(|c| matches!(
                c,
                engine_runtime::rhi_command_plan::RhiCommand::SimulateParticles { .. }
            ))
            .count(),
        1
    );
}
fn compile(g: &RenderGraph) -> engine_runtime::rhi_command_plan::RhiCommandPlan {
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(g);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    plan
}
#[test]
fn scene_views_duplicate_only_draws_and_keep_per_view_depth_fog_and_resolve_order() {
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    let p = compile(&graph([80, 48], true, false, true, "Rgba8Unorm", 1));
    assert_eq!(
        p.commands
            .iter()
            .filter(|c| matches!(c, RhiCommand::ReconcileParticles { .. }))
            .count(),
        1
    );
    assert_eq!(
        p.commands
            .iter()
            .filter(|c| matches!(c, RhiCommand::SimulateParticles { .. }))
            .count(),
        1
    );
    let draws: Vec<_> = p
        .commands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            RhiCommand::Draw {
                target,
                payload: RhiDrawPayload::Particles { view, .. },
                ..
            } => Some((i, target, view)),
            _ => None,
        })
        .collect();
    assert_eq!(draws.len(), 2);
    assert_ne!(draws[0].2.depth_target, draws[1].2.depth_target);
    assert_ne!(draws[0].2.fog, draws[1].2.fog);
    assert_eq!(draws[1].2.viewport, Some([0, 0, 80, 48]));
    for (index, target, _) in draws {
        assert!(p.commands[index + 1..].iter().any(
            |c| matches!(c,RhiCommand::ResolveColor3d{resolve} if resolve.source==target.as_ref())
        ));
    }
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_scene_views_use_independent_depth_fog_exposure_resize_and_single_simulation() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let center = |v: &[u8], w: usize, h: usize| -> [u8; 4] {
        v[((h / 2) * w + w / 2) * 4..][..4].try_into().unwrap()
    };
    let mut expected_age = None;
    for (extent, hidden) in [([64, 64], false), ([80, 48], false), ([80, 48], true)] {
        let p = compile(&graph(extent, true, hidden, true, "Rgba8Unorm", 1));
        let primary = backend.render_plan_to_rgba_bytes(&p, 64, 64).unwrap();
        let aux = backend.read_geometry_color_target_rgba("aux").unwrap();
        for (image, w, h, density, ev, z, alpha) in [
            (&primary, 64, 64, 0.2, 0., 2., 0.5),
            (
                &aux,
                extent[0] as usize,
                extent[1] as usize,
                0.8,
                -1.,
                if hidden { 0.5 } else { 2. },
                if hidden { 0. } else { 0.5 },
            ),
        ] {
            // Independent perspective ray and radial transmittance at the sampled pixel.
            let ray2 = 1. + (1. / h as f64 * (std::f64::consts::PI / 6.).tan()).powi(2) * 2.;
            let a = (-density * ray2.sqrt()).exp();
            let b = (-density * z * ray2.sqrt()).exp();
            let linear = [
                alpha * a,
                alpha * (1. - a) + (1. - alpha) * (1. - b),
                (1. - alpha) * b,
            ];
            let reference = linear.map(|x| {
                let x = x * 2f64.powf(ev);
                let y = (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
                ((if y <= 0.0031308 {
                    12.92 * y
                } else {
                    1.055 * y.powf(1. / 2.4) - 0.055
                }) * 255.)
                    .round() as u8
            });
            let actual = center(image, w, h);
            assert!(
                actual[..3]
                    .iter()
                    .zip(reference)
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "{w}x{h} hidden={hidden}: {actual:?}, reference={reference:?}"
            );
        }
        let age = backend.capture_particle_state_for_validation(31).unwrap()[0].particles[0].age;
        assert_eq!(
            *expected_age.get_or_insert(age),
            age,
            "second view or resize resimulated"
        );
        // A forged per-view fog/depth must be rejected before either simulation or submit.
        let mut bad = p.clone();
        for c in &mut bad.commands {
            if let engine_runtime::rhi_command_plan::RhiCommand::Draw {
                payload: engine_runtime::rhi_command_plan::RhiDrawPayload::Particles { view, .. },
                ..
            } = c
            {
                view.fog.as_mut().unwrap().density_per_m = 0.123.into();
                break;
            }
        }
        let submitted = backend.last_gpu_submission();
        assert!(backend
            .render_plan_to_rgba_bytes(&bad, 64, 64)
            .unwrap_err()
            .contains("scene_attachment_mismatch"));
        assert_eq!(submitted, backend.last_gpu_submission());
        let mut wrong_affine = p.clone();
        for c in &mut wrong_affine.commands {
            if let engine_runtime::rhi_command_plan::RhiCommand::Draw {
                payload: engine_runtime::rhi_command_plan::RhiDrawPayload::Particles { view, .. },
                ..
            } = c
            {
                view.emitter_affine.columns[0][0] = 2.0.into();
                break;
            }
        }
        assert!(backend
            .render_plan_to_rgba_bytes(&wrong_affine, 64, 64)
            .unwrap_err()
            .contains("source_affine_mismatch"));
        assert_eq!(submitted, backend.last_gpu_submission());
        let mut singular = p.clone();
        for c in &mut singular.commands {
            if let engine_runtime::rhi_command_plan::RhiCommand::SimulateParticles {
                step, ..
            } = c
            {
                step.emitter_affine.columns[0][0] = 0.0.into();
                break;
            }
        }
        assert!(backend
            .render_plan_to_rgba_bytes(&singular, 64, 64)
            .unwrap_err()
            .contains("singular_affine"));
        assert_eq!(submitted, backend.last_gpu_submission());
        assert_eq!(
            age,
            backend.capture_particle_state_for_validation(31).unwrap()[0].particles[0].age
        );
        let mut partial = p.clone();
        for c in &mut partial.commands {
            if let engine_runtime::rhi_command_plan::RhiCommand::BeginView { view, .. } = c {
                std::sync::Arc::make_mut(view).viewport[2] /= 2;
                break;
            }
        }
        assert!(backend
            .render_plan_to_rgba_bytes(&partial, 64, 64)
            .unwrap_err()
            .contains("scene_attachment_mismatch"));
        assert_eq!(submitted, backend.last_gpu_submission());
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_particle_secondary_wrong_native_format_rejects_before_simulation() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let p = compile(&graph([64, 64], false, false, false, "Bgra8Unorm", 1));
    let submitted = backend.last_gpu_submission();
    assert!(backend
        .render_plan_to_rgba_bytes(&p, 64, 64)
        .unwrap_err()
        .contains("output_format_mismatch"));
    assert_eq!(submitted, backend.last_gpu_submission());
    assert!(backend.capture_particle_state_for_validation(31).is_err());
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_shared_partial_outputs_keep_depth_exposure_edges_and_simulation_independent() {
    use engine_runtime::{
        rhi_command_plan::{RhiCommand, RhiDrawPayload},
        wgpu_backend::real::RealWgpuBackend,
    };
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let pix = |v: &[u8], x: usize, y: usize| -> [u8; 4] {
        v[(y * 64 + x) * 4..][..4].try_into().unwrap()
    };
    let tone = |x: f64| {
        let t = (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
        ((if t <= 0.0031308 {
            12.92 * t
        } else {
            1.055 * t.powf(1. / 2.4) - 0.055
        }) * 255.)
            .round() as u8
    };
    for hdr in [true, false] {
        let g = graph_mode([64, 64], true, true, hdr, "Rgba8Unorm", 1, true);
        let p = compile(&g);
        let image = backend.render_plan_to_rgba_bytes(&p, 64, 64).unwrap();
        for (x, density, ev, z, alpha) in [(16, 0.2, 0., 2., 0.5), (48, 0.8, -1., 0.5, 0.)] {
            let ray = (1. + 2. * ((std::f64::consts::PI / 6.).tan() / 48.).powi(2)).sqrt();
            let a = (-density * ray).exp();
            let b = (-density * z * ray).exp();
            let linear = if hdr {
                [
                    alpha * a,
                    alpha * (1. - a) + (1. - alpha) * (1. - b),
                    (1. - alpha) * b,
                ]
            } else {
                [alpha, 0., 1. - alpha]
            };
            let expected = linear.map(|v| {
                if hdr {
                    tone(v * 2f64.powf(ev))
                } else {
                    (v * 255.).round() as u8
                }
            });
            let actual = pix(&image, x, 32);
            assert!(
                actual[..3]
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "hdr={hdr} x={x} {actual:?} vs {expected:?}"
            );
        }
        for (x, y) in [
            (0, 0),
            (32, 32),
            (63, 63),
            (3, 32),
            (28, 32),
            (35, 32),
            (60, 32),
        ] {
            assert_eq!(
                pix(&image, x, y),
                [0, 0, 0, 255],
                "region leaked at {x},{y}"
            );
        }
        let age = backend.capture_particle_state_for_validation(31).unwrap()[0].particles[0].age;
        // Existing packets remain immutable and reusing a view cannot simulate twice.
        assert_eq!(
            image,
            backend.render_plan_to_rgba_bytes(&p, 64, 64).unwrap()
        );
        assert_eq!(
            age,
            backend.capture_particle_state_for_validation(31).unwrap()[0].particles[0].age
        );
        let contained = backend
            .render_plan_to_rgba_bytes_with_rect(
                &p,
                64,
                64,
                Some(engine_runtime::game_view_presentation::GameViewRect::new(
                    8., 8., 48., 48.,
                )),
            )
            .unwrap();
        for (a, b) in [(16, 20), (48, 44)] {
            assert!(
                pix(&image, a, 32)
                    .iter()
                    .zip(pix(&contained, b, 32))
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "partial viewport ignored content rectangle: hdr={hdr}"
            );
        }
        assert_eq!(pix(&contained, 32, 32), [0, 0, 0, 255]);
        let submitted = backend.last_gpu_submission();
        if hdr {
            assert!(backend
                .render_plan_to_rgba_bytes(&p, 32, 32)
                .unwrap_err()
                .contains("target_extent_mismatch"));
            assert_eq!(submitted, backend.last_gpu_submission());
            let mut bad = p.clone();
            for c in &mut bad.commands {
                if let RhiCommand::ResolveColor3d { resolve } = c {
                    resolve.viewport = Some([63, 0, 2, 64]);
                    break;
                }
            }
            assert!(backend.render_plan_to_rgba_bytes(&bad, 64, 64).is_err());
            assert_eq!(submitted, backend.last_gpu_submission());
            // A second resolve writing the same viewport is not a legal shared output.
            let mut duplicate = p.clone();
            let r = duplicate
                .commands
                .iter()
                .find(|c| matches!(c, RhiCommand::ResolveColor3d { .. }))
                .unwrap()
                .clone();
            duplicate.commands.push(r);
            assert!(backend
                .render_plan_to_rgba_bytes(&duplicate, 64, 64)
                .is_err());
            assert_eq!(submitted, backend.last_gpu_submission());
        }
        // Raw view overlap must fail before any simulation or submission.
        let mut overlap = p.clone();
        for c in &mut overlap.commands {
            if let RhiCommand::BeginView { view, .. } = c {
                if view.view_id == 2 {
                    std::sync::Arc::make_mut(view).viewport = [4, 8, 24, 48];
                }
            }
        }
        assert!(backend
            .render_plan_to_rgba_bytes(&overlap, 64, 64)
            .unwrap_err()
            .contains("overlapping_output_unsupported"));
        assert_eq!(submitted, backend.last_gpu_submission());
        // Opaque secondary depth prevents the red particle there, independent of main depth.
        assert!(p.commands.iter().any(|c|matches!(c,RhiCommand::Draw{payload:RhiDrawPayload::Particles{view,..},..} if view.scene_view==Some(2))));
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_partial_color_fxaa_clamps_edges_and_preserves_uncovered_pixels() {
    use engine_runtime::{rhi_command_plan::RhiCommand, wgpu_backend::real::RealWgpuBackend};
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let mut g = graph_mode([64, 64], false, false, true, "Rgba8Unorm", 1, true);
    // Flat, distinct HDR clears with no draws isolate boundary filtering, including FXAA.
    for pass in &mut g.passes {
        pass.commands.retain(|c| {
            matches!(
                c,
                RenderPassCommand::BeginView { .. }
                    | RenderPassCommand::ResolveColor3d { .. }
                    | RenderPassCommand::Clear { .. }
                    | RenderPassCommand::Present { .. }
            )
        });
        for c in &mut pass.commands {
            match c {
                RenderPassCommand::BeginView { view, .. } => {
                    let view = std::sync::Arc::make_mut(view);
                    view.clear_color = Some(
                        if view.view_id == 1 {
                            [4., 0., 0., 1.]
                        } else {
                            [0., 4., 0., 1.]
                        }
                        .map(Into::into),
                    );
                    view.color_pipeline.as_mut().unwrap().anti_aliasing = AntiAliasing3D::Fxaa;
                }
                RenderPassCommand::ResolveColor3d { resolve } => {
                    resolve.pipeline.anti_aliasing = AntiAliasing3D::Fxaa
                }
                _ => (),
            }
        }
    }
    // Remove empty passes rather than leave undeclared execution requirements.
    g.passes.retain(|p| !p.commands.is_empty());
    let p = compile(&g);
    let image = backend.render_plan_to_rgba_bytes(&p, 64, 64).unwrap();
    let pix =
        |x: usize, y: usize| -> [u8; 4] { image[(y * 64 + x) * 4..][..4].try_into().unwrap() };
    for (x0, color) in [(4, 0), (36, 1)] {
        let center = pix(x0 + 12, 32);
        assert!(center[color] > 220);
        for y in 8..56 {
            for x in x0..x0 + 24 {
                assert_eq!(pix(x, y), center, "FXAA crossed region at {x},{y}");
            }
        }
    }
    assert_eq!(pix(32, 32), [0, 0, 0, 255]);
    // Contract roundtrip: old full resolve omits viewport, partial requires explicit field.
    let r = p
        .commands
        .iter()
        .find_map(|c| {
            if let RhiCommand::ResolveColor3d { resolve } = c {
                Some(resolve)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        r,
        &serde_json::from_value::<ColorResolve3D>(serde_json::to_value(r).unwrap()).unwrap()
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
