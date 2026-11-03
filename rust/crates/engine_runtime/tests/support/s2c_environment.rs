use super::*;
use engine_runtime::color_pipeline::*;
use engine_runtime::render_graph::*;

fn sky() -> GradientSky3D {
    GradientSky3D {
        zenith: [0.1, 0.3, 1.5].map(Into::into),
        horizon: [1.2, 0.6, 0.2].map(Into::into),
        nadir: [0.02, 0.05, 0.08].map(Into::into),
    }
}
fn camera() -> RenderView {
    let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.);
    view.clear_policy = ClearPolicy::Color {
        color: [0.1, 0.2, 0.3, 0.25],
    };
    view.color_pipeline = Some(ColorPipeline3D {
        anti_aliasing: AntiAliasing3D::None,
        ..Default::default()
    });
    view
}
#[cfg(feature = "real-wgpu")]
fn build(
    view: &RenderView,
    pose: CameraPose,
    lit: bool,
    alpha: f32,
    z: Option<f32>,
) -> RenderGraph {
    let mut resources = RenderResourceManager::new();
    let mut instances = Vec::new();
    let source;
    if let Some(z) = z {
        let mesh = geometry_fixture::prepare(
            "mesh",
            // The reused backend must see one immutable mesh identity across material variants.
            RenderAssetData::Mesh(standard_lit_fixture::lit_triangle()),
            &mut resources,
        );
        let mut material = if lit {
            standard_lit_fixture::standard_lit([0.8, 0.4, 0.1, alpha.abs()])
        } else {
            geometry_fixture::unlit([0.8, 0.4, 0.1, alpha.abs()])
        };
        if lit {
            material.parameters.insert(
                "emissive".into(),
                MaterialParameter::Vector4([0.8, 0.4, 0.1, 0.]),
            );
        }
        if alpha < 0. {
            material.blend = MaterialBlend::AlphaMask;
            material
                .parameters
                .insert("alphaCutoff".into(), MaterialParameter::Float(0.5));
            material.parameters.insert(
                "baseColor".into(),
                MaterialParameter::Vector4([0.8, 0.4, 0.1, -alpha]),
            );
        } else if alpha < 1. {
            material.blend = MaterialBlend::AlphaBlend;
        }
        let material = geometry_fixture::prepare(
            "material",
            RenderAssetData::Material(material),
            &mut resources,
        );
        source = PreparedGeometry::from_assets(&mesh, &material, 0, &resources).unwrap();
        instances.push(geometry_fixture::instance(&source, 1, [0., 0., z]));
    }
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput { view, pose }],
        &instances,
        &resources,
        DiagnosticLevel::Off,
    )
    .unwrap();
    geometry_fixture::graph(&frame)
}
#[test]
fn environment_authoring_is_numeric_strict_optional_and_bounded() {
    let mut pipeline = ColorPipeline3D::default();
    let old = serde_json::to_value(&pipeline).unwrap();
    assert!(old.get("sky").is_none() && old.get("fog").is_none());
    pipeline.sky = Some(sky());
    pipeline.fog = Some(DistanceFog3D {
        color: [0.2, 0.4, 0.8].map(Into::into),
        density_per_m: 0.3.into(),
        start_distance_m: 1.5.into(),
    });
    pipeline.validate().unwrap();
    let value = serde_json::to_value(&pipeline).unwrap();
    assert_eq!(value["fog"]["startDistanceM"], serde_json::json!(1.5));
    assert_eq!(
        serde_json::from_value::<ColorPipeline3D>(value.clone()).unwrap(),
        pipeline
    );
    for (path, value) in [
        ("densityPerM", -0.1),
        ("densityPerM", 101.),
        ("startDistanceM", -1.),
        ("startDistanceM", 1e10),
    ] {
        let mut bad = serde_json::to_value(&pipeline).unwrap();
        bad["fog"][path] = serde_json::json!(value);
        assert!(serde_json::from_value::<ColorPipeline3D>(bad)
            .unwrap()
            .validate()
            .is_err());
    }
    let mut bad = value.clone();
    bad["sky"]["sunMagic"] = true.into();
    assert!(serde_json::from_value::<ColorPipeline3D>(bad).is_err());
    let mut bad = pipeline.clone();
    bad.sky.as_mut().unwrap().zenith[0] = f32::NAN.into();
    assert!(bad.validate().is_err());
    let mut bad = pipeline.clone();
    bad.fog.as_mut().unwrap().color[0] = 70000f32.into();
    assert!(bad.validate().is_err());
    // Frozen previous strict reader cannot silently omit environment semantics.
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    #[allow(dead_code)]
    struct OldColor {
        schema_version: String,
        exposure_ev: f32,
        tone_mapping: String,
        anti_aliasing: String,
        ambient_diffuse: [f32; 3],
    }
    assert!(serde_json::from_value::<OldColor>(value).is_err());
    let mut view = camera();
    view.color_pipeline.as_mut().unwrap().sky = Some(sky());
    let mut executable = view.evaluate(CameraPose::default()).unwrap();
    executable.projection_matrix[8] = 0.2.into();
    assert!(
        executable.validate_extent(64, 64).is_err(),
        "off-center sky lens must not silently use centered rays"
    );
}
#[cfg(feature = "real-wgpu")]
fn reference(x: f64) -> u8 {
    let x = x.clamp(0., 65504.);
    let t = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
    let encoded = if t <= 0.0031308 {
        t * 12.92
    } else {
        1.055 * t.powf(1. / 2.4) - 0.055
    };
    (encoded * 255.).round() as u8
}
#[cfg(feature = "real-wgpu")]
fn render(
    backend: &mut engine_runtime::wgpu_backend::real::RealWgpuBackend,
    graph: &RenderGraph,
) -> Vec<u8> {
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap()
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_sky_matches_world_rays_rotation_projection_and_translation_invariance() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let mut view = camera();
    view.color_pipeline.as_mut().unwrap().sky = Some(sky());
    let mut baseline = None;
    for (angle, fov) in [(0f64, 60f64), (0.6, 40.), (-0.6, 80.)] {
        view.projection = Projection::Perspective {
            fov_y_degrees: fov as f32,
            near: 0.1,
            far: 100.,
        };
        let pose = CameraPose {
            position: [0.; 3],
            rotation: [(angle / 2.).sin() as f32, 0., 0., (angle / 2.).cos() as f32],
        };
        let graph = build(&view, pose, false, 1., None);
        let pixels = render(&mut backend, &graph);
        for y in 0..64 {
            for x in 0..64 {
                let vx = (2. * (x as f64 + 0.5) / 64. - 1.) * ((fov / 2.).to_radians()).tan();
                let vy = (1. - 2. * (y as f64 + 0.5) / 64.) * ((fov / 2.).to_radians()).tan();
                let dy = (vy * angle.cos() + angle.sin()) / (vx * vx + vy * vy + 1.).sqrt();
                for (c, (h, pole)) in [1.2, 0.6, 0.2]
                    .into_iter()
                    .zip(if dy >= 0. {
                        [0.1, 0.3, 1.5]
                    } else {
                        [0.02, 0.05, 0.08]
                    })
                    .enumerate()
                {
                    let expected = reference(h + (pole - h) * dy.abs());
                    let actual = pixels[(y * 64 + x) * 4 + c];
                    assert!(
                        actual.abs_diff(expected) <= 2,
                        "angle={angle} pixel={x},{y} c={c} actual={actual} expected={expected}"
                    );
                }
                assert_eq!(pixels[(y * 64 + x) * 4 + 3], 64);
            }
        }
        let shifted = build(
            &view,
            CameraPose {
                position: [10000., -20000., 30000.],
                ..pose
            },
            false,
            1.,
            None,
        );
        assert_eq!(
            render(&mut backend, &shifted),
            pixels,
            "sky moved with camera translation"
        );
        if angle == 0. {
            baseline = Some((graph, pixels));
        }
    }
    let (graph, pixels) = baseline.unwrap();
    assert_eq!(
        render(&mut backend, &graph),
        pixels,
        "old sky packet changed"
    );
    let mut ortho = RenderView::orthographic(1, 64, 64, 2., 0.1, 100.);
    ortho.domain = RenderDomain::World3d;
    ortho.coordinate_convention=engine_runtime::render_contract::CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ;
    ortho.clear_policy = view.clear_policy.clone();
    ortho.color_pipeline = view.color_pipeline.clone();
    let image = render(
        &mut backend,
        &build(&ortho, CameraPose::default(), false, 1., None),
    );
    assert!(
        image.chunks_exact(4).all(|p| p == &image[..4]),
        "orthographic sky must use parallel rays"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_fog_is_radial_scene_linear_and_precedes_alpha_for_lit_and_unlit() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    for lit in [false, true] {
        for alpha in [1f32, 0.5] {
            for z in [-0.8f32, -2., -4.] {
                for start in [0., 3.] {
                    let mut view = camera();
                    view.color_pipeline.as_mut().unwrap().fog = Some(DistanceFog3D {
                        color: [0.2, 0.4, 0.8].map(Into::into),
                        density_per_m: 0.8.into(),
                        start_distance_m: start.into(),
                    });
                    let image = render(
                        &mut backend,
                        &build(&view, CameraPose::default(), lit, alpha, Some(z)),
                    );
                    let points = if z == -0.8 {
                        vec![(32usize, 32usize), (8, 48), (56, 48)]
                    } else {
                        vec![(32usize, 32usize), (30, 34), (34, 34)]
                    };
                    for (x, y) in points {
                        let vz = f64::from(-z);
                        let tangent = 30f64.to_radians().tan();
                        let vx = (2. * (x as f64 + 0.5) / 64. - 1.) * tangent * vz;
                        let vy = (1. - 2. * (y as f64 + 0.5) / 64.) * tangent * vz;
                        let distance = (vx * vx + vy * vy + vz * vz).sqrt();
                        let t = (-0.8 * (distance - f64::from(start)).max(0.)).exp();
                        for (c, ((base, fog), clear)) in [0.8, 0.4, 0.1]
                            .into_iter()
                            .zip([0.2, 0.4, 0.8])
                            .zip([0.1, 0.2, 0.3])
                            .enumerate()
                        {
                            let scene = (base * t + fog * (1. - t)) * f64::from(alpha)
                                + clear * (1. - f64::from(alpha));
                            let expected = reference(scene);
                            let actual = image[(y * 64 + x) * 4 + c];
                            assert!(actual.abs_diff(expected)<=2,"lit={lit} alpha={alpha} z={z} start={start} at={x},{y} c={c} actual={actual} expected={expected}");
                        }
                        let expected = ((f64::from(alpha) + 0.25 * (1. - f64::from(alpha))) * 255.)
                            .round() as u8;
                        assert!(image[(y * 64 + x) * 4 + 3].abs_diff(expected) <= 1);
                    }
                    // Fog does not fill/fog the background or discard its alpha.
                    for c in 0..3 {
                        assert!(image[c].abs_diff(reference([0.1, 0.2, 0.3][c])) <= 2);
                    }
                }
            }
        }
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_sky_fog_respects_mask_depth_and_screen_ui() {
    let mut backend =
        engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let mut view = camera();
    view.color_pipeline.as_mut().unwrap().sky = Some(sky());
    view.color_pipeline.as_mut().unwrap().fog = Some(DistanceFog3D {
        color: [0.2, 0.4, 0.8].map(Into::into),
        density_per_m: 1.0.into(),
        start_distance_m: 0.0.into(),
    });
    let background = render(
        &mut backend,
        &build(&view, CameraPose::default(), false, 1., None),
    );
    for lit in [false, true] {
        let masked = render(
            &mut backend,
            &build(&view, CameraPose::default(), lit, -0.4, Some(-2.)),
        );
        assert_eq!(
            background, masked,
            "mask discard must leave sky and depth untouched"
        );
        let visible = render(
            &mut backend,
            &build(&view, CameraPose::default(), lit, -0.5, Some(-2.)),
        );
        assert_ne!(background, visible, "mask cutoff equality must receive fog");
    }
    let mut g = build(&view, CameraPose::default(), true, 1., Some(-2.));
    let vertices = [[-0.2, -0.2], [0.2, -0.2], [0., 0.2]]
        .into_iter()
        .map(|p| RenderDrawVertex::new(p, [0., 0., 1., 1.], [0.; 2]))
        .collect::<Vec<_>>()
        .into();
    g.passes.push(RenderPass {
        pass_id: "ui".into(),
        pass_name: "ui".into(),
        pass_kind: RenderPassKind::DrawUiComposition,
        view_id: "ui".into(),
        reads: vec![],
        writes: vec!["main".into()],
        color_targets: vec!["main".into()],
        depth_target: None,
        commands: vec![RenderPassCommand::DrawUiComposition {
            target: "main".into(),
            stage: "ScreenOverlay".into(),
            item_count: 1,
            text_count: 0,
            image_count: 0,
            glyph_count: 0,
            font_atlas_id: None,
            text_pass_inserted: false,
            debug_label: "ui".into(),
            texture: None,
            font_render_mode: None,
            font_page_index: None,
            vertices,
        }]
        .into(),
        debug_source: None,
    });

    let pixels = render(&mut backend, &g);
    assert_eq!(
        &pixels[(32 * 64 + 32) * 4..][..4],
        [0, 0, 255, 255],
        "screen UI was fogged or tinted by sky"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}
