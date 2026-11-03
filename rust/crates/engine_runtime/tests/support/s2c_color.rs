use super::*;
use engine_runtime::color_pipeline::*;
use engine_runtime::render_graph::*;
fn graph(ev: f32, aa: AntiAliasing3D, emission: [f32; 3]) -> RenderGraph {
    make_graph(ev, aa, emission, false)
}
fn make_graph(ev: f32, aa: AntiAliasing3D, emission: [f32; 3], blend: bool) -> RenderGraph {
    make_material_graph(ev, aa, emission, blend, [0.; 3], 0.)
}
fn make_material_graph(
    ev: f32,
    aa: AntiAliasing3D,
    emission: [f32; 3],
    blend: bool,
    ambient: [f32; 3],
    metallic: f32,
) -> RenderGraph {
    let mut manager = RenderResourceManager::new();
    let mesh = geometry_fixture::prepare(
        "mesh",
        RenderAssetData::Mesh(standard_lit_fixture::lit_triangle()),
        &mut manager,
    );
    let mut material = standard_lit_fixture::standard_lit([0.8, 0.4, 0.1, 1.]);
    material
        .parameters
        .insert("metallic".into(), MaterialParameter::Float(metallic));
    material.parameters.insert(
        "emissive".into(),
        MaterialParameter::Vector4([emission[0], emission[1], emission[2], 0.]),
    );
    if blend {
        material.blend = MaterialBlend::AlphaBlend;
        material.parameters.insert(
            "baseColor".into(),
            MaterialParameter::Vector4([0.8, 0.4, 0.1, 0.5]),
        );
    }
    let mat = geometry_fixture::prepare("mat", RenderAssetData::Material(material), &mut manager);
    let source = PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).unwrap();
    let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    if blend {
        view.clear_policy = ClearPolicy::Color {
            color: [0.25, 0.5, 1., 0.5],
        };
    }
    view.color_pipeline = Some(ColorPipeline3D {
        exposure_ev: ev.into(),
        anti_aliasing: aa,
        ambient_diffuse: ambient.map(Into::into),
        ..Default::default()
    });
    let frame = GeometryFrame::build(
        "main",
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
    geometry_fixture::graph(&frame)
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_packet_ambient_diffuse_matches_energy_and_does_not_fake_metal_ibl() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    for metallic in [0., 0.5, 1.] {
        let graph = make_material_graph(
            0.,
            AntiAliasing3D::None,
            [0.; 3],
            false,
            [0.5, 0.25, 0.125],
            metallic,
        );
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        let center = &pixels[(32 * 64 + 32) * 4..][..4];
        for (c, (base, ambient)) in [0.8f64, 0.4, 0.1]
            .into_iter()
            .zip([0.5, 0.25, 0.125])
            .enumerate()
        {
            let metal = f64::from(metallic);
            let f0 = 0.04 * (1. - metal) + base * metal;
            let x = (1. - f0) * (1. - metal) * base * ambient;
            let tone = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
            let display = if tone <= 0.0031308 {
                tone * 12.92
            } else {
                1.055 * tone.powf(1. / 2.4) - 0.055
            };
            let expected = (display * 255.).round() as u8;
            assert!(
                center[c].abs_diff(expected) <= 2,
                "metallic={metallic} c={c}: {center:?} expected={expected}"
            );
        }
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
}
#[test]
fn scene_color_contract_is_opt_in_strict_and_bounded() {
    assert!(ColorPipeline3D::default().validate_clear(None).is_err());
    assert!(ColorPipeline3D::default()
        .validate_clear(Some([70000., 0., 0., 1.]))
        .is_err());
    let old = engine_runtime::spatial_contract::Camera3D::perspective_template();
    let mut value = serde_json::to_value(&old).unwrap();
    assert!(value.get("colorPipeline").is_none());
    value["colorPipeline"] = serde_json::json!({"schemaVersion":"view-color3d.v1","exposureEv":2.0,"toneMapping":"acesFitted","antiAliasing":"fxaa","ambientDiffuse":[0.06,0.08,0.12]});
    let camera: engine_runtime::spatial_contract::Camera3D =
        serde_json::from_value(value.clone()).unwrap();
    camera.validate().unwrap();
    assert_eq!(
        camera.color_pipeline.as_ref().unwrap().exposure_ev.to_f32(),
        2.
    );
    assert_eq!(
        serde_json::to_value(camera.color_pipeline.as_ref().unwrap()).unwrap()["exposureEv"],
        serde_json::json!(2.)
    );
    for ev in [-17., 17., f32::NAN] {
        let c = ColorPipeline3D {
            exposure_ev: ev.into(),
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }
    value["colorPipeline"]["autoExposure"] = true.into();
    assert!(serde_json::from_value::<engine_runtime::spatial_contract::Camera3D>(value).is_err());
    // Frozen old camera schema cannot silently drop the new field.
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    #[allow(dead_code)]
    struct OldCamera {
        schema_version: String,
        projection: Projection,
        viewport: engine_runtime::spatial_contract::CameraViewport,
        clear_color: [f32; 4],
        layers: Vec<String>,
        enabled: bool,
    }
    assert!(serde_json::from_value::<OldCamera>(serde_json::to_value(camera).unwrap()).is_err());
}
#[test]
fn hdr_graph_has_explicit_ordered_resources_and_rejects_missing_reads() {
    let graph = graph(0., AntiAliasing3D::Fxaa, [4., 0.5, 0.18]);
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    assert_eq!(
        graph
            .resources
            .iter()
            .filter(|r| r.format == RenderResourceFormat::Rgba16Float)
            .count(),
        1
    );
    let mut bad = graph.clone();
    bad.passes
        .iter_mut()
        .find(|p| p.pass_kind == RenderPassKind::ResolveColor3d)
        .unwrap()
        .reads
        .clear();
    assert!(engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&bad).has_errors());
    let mut bad = graph;
    bad.resources
        .iter_mut()
        .find(|r| r.format == RenderResourceFormat::Rgba16Float)
        .unwrap()
        .format = RenderResourceFormat::Rgba8Unorm;
    assert!(engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&bad).has_errors());
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_hdr_preserves_high_radiance_exposure_and_fxaa_edges() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    fn reference(x: f64, ev: f64) -> u8 {
        let x = x * 2f64.powf(ev);
        let x = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
        let x = if x <= 0.0031308 {
            x * 12.92
        } else {
            1.055 * x.powf(1. / 2.4) - 0.055
        };
        (x * 255.).round() as u8
    }
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let mut outputs = Vec::new();
    for ev in [-2., 0., 2.] {
        let graph = graph(ev, AntiAliasing3D::None, [4., 0.5, 0.18]);
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        let center = &pixels[(32 * 64 + 32) * 4..][..4];
        for c in 0..3 {
            let expected = reference([4., 0.5, 0.18][c], ev as f64);
            assert!(
                center[c].abs_diff(expected) <= 2,
                "ev={ev} channel={c}: actual={} expected={expected}",
                center[c]
            );
        }
        outputs.push((plan, pixels));
    }
    assert_ne!(outputs[0].1, outputs[1].1);
    assert_ne!(outputs[1].1, outputs[2].1);
    let aa = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph(
        0.,
        AntiAliasing3D::Fxaa,
        [4., 0.5, 0.18],
    ));
    let smooth = backend.render_plan_to_rgba_bytes(&aa, 64, 64).unwrap();
    assert_eq!(
        &smooth[(32 * 64 + 32) * 4..][..4],
        &outputs[1].1[(32 * 64 + 32) * 4..][..4],
        "flat interior changed"
    );
    let changed = smooth
        .chunks_exact(4)
        .zip(outputs[1].1.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 20 && changed < 500,
        "FXAA must affect silhouette only: {changed}"
    );
    let expected = fxaa_reference(&outputs[1].1, 64, 64);
    for (index, (a, b)) in smooth.iter().zip(&expected).enumerate() {
        assert!(
            a.abs_diff(*b) <= 2,
            "FXAA independent reference byte {index}: actual={a} expected={b}"
        );
    }
    let replay = backend
        .render_plan_to_rgba_bytes(&outputs[0].0, 64, 64)
        .unwrap();
    assert_eq!(replay, outputs[0].1, "old immutable exposure/frame changed");
    let mut bad = aa.clone();
    bad.commands
        .iter_mut()
        .find_map(|c| {
            if let engine_runtime::rhi_command_plan::RhiCommand::ResolveColor3d { resolve } = c {
                Some(resolve)
            } else {
                None
            }
        })
        .unwrap()
        .source = "absent".into();
    let before = backend.gpu_lifecycle_stats().submitted;
    assert!(backend.render_plan_to_rgba_bytes(&bad, 64, 64).is_err());
    assert_eq!(before, backend.gpu_lifecycle_stats().submitted);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    eprintln!("HDR >1/exposure -2,0,2 match independent f64 ACES/sRGB; FXAA {changed} edge pixels; immutable replay identical");
}

#[cfg(feature = "real-wgpu")]
fn fxaa_reference(image: &[u8], width: usize, height: usize) -> Vec<u8> {
    let sample = |x: f64, y: f64| -> [f64; 3] {
        let x = x.clamp(0., (width - 1) as f64);
        let y = y.clamp(0., (height - 1) as f64);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);
        let a = x - x0 as f64;
        let b = y - y0 as f64;
        std::array::from_fn(|c| {
            let read = |x: usize, y: usize| f64::from(image[(y * width + x) * 4 + c]) / 255.;
            (read(x0, y0) * (1. - a) + read(x1, y0) * a) * (1. - b)
                + (read(x0, y1) * (1. - a) + read(x1, y1) * a) * b
        })
    };
    let luma = |v: [f64; 3]| v[0] * 0.299 + v[1] * 0.587 + v[2] * 0.114;
    let mut result = image.to_vec();
    for y in 0..height {
        for x in 0..width {
            let xf = x as f64;
            let yf = y as f64;
            let mid = sample(xf, yf);
            let a = luma(sample(xf - 1., yf - 1.));
            let b = luma(sample(xf + 1., yf - 1.));
            let c = luma(sample(xf - 1., yf + 1.));
            let d = luma(sample(xf + 1., yf + 1.));
            let m = luma(mid);
            let lo = m.min(a.min(b).min(c.min(d)));
            let hi = m.max(a.max(b).max(c.max(d)));
            if hi - lo < (1f64 / 32.).max(hi / 8.) {
                continue;
            }
            let dir = [-(a + b - c - d), a + c - b - d];
            let reduce = ((a + b + c + d) / 32.).max(1. / 128.);
            let denom = dir[0].abs().min(dir[1].abs()) + reduce;
            let dir = dir.map(|v| (v / denom).clamp(-8., 8.));
            let at = |t: f64| sample(xf + dir[0] * t, yf + dir[1] * t);
            let a1 = at(1. / 3. - 0.5);
            let a2 = at(2. / 3. - 0.5);
            let b1 = at(-0.5);
            let b2 = at(0.5);
            let a: [f64; 3] = std::array::from_fn(|c| (a1[c] + a2[c]) * 0.5);
            let b = std::array::from_fn(|c| a[c] * 0.5 + (b1[c] + b2[c]) * 0.25);
            let lb = luma(b);
            let chosen = if lb < lo || lb > hi { a } else { b };
            for c in 0..3 {
                result[(y * width + x) * 4 + c] = (chosen[c] * 255.).round() as u8;
            }
        }
    }
    result
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_color_formats_linear_blend_and_ui_stay_separate() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let instance = wgpu::Instance::new(&Default::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut images = Vec::new();
    for (native, format) in [
        (
            wgpu::TextureFormat::Rgba8Unorm,
            RenderResourceFormat::Rgba8Unorm,
        ),
        (
            wgpu::TextureFormat::Bgra8Unorm,
            RenderResourceFormat::Bgra8Unorm,
        ),
        (
            wgpu::TextureFormat::Rgba8UnormSrgb,
            RenderResourceFormat::Rgba8UnormSrgb,
        ),
        (
            wgpu::TextureFormat::Bgra8UnormSrgb,
            RenderResourceFormat::Bgra8UnormSrgb,
        ),
    ] {
        let mut backend = RealWgpuBackend::from_shared_device_queue(
            &device,
            &queue,
            native,
            64,
            64,
            "color-transfer-test",
        );
        let mut g = make_graph(0., AntiAliasing3D::None, [4., 0.5, 0.18], true);
        g.resources[0].format = format;
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&g);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        let pixel = &pixels[(32 * 64 + 32) * 4..][..4];
        // Independent f64 blend first in scene-linear HDR, then ACES and display transfer.
        for (channel, x) in [2.125f64, 0.5, 0.59].into_iter().enumerate() {
            let tone = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
            let expected = ((1.055 * tone.powf(1. / 2.4) - 0.055) * 255.).round() as u8;
            assert!(
                pixel[channel].abs_diff(expected) <= 2,
                "{native:?}: {pixel:?} channel={channel} expected={expected}"
            );
        }
        assert!(
            pixel[3].abs_diff(191) <= 1,
            "alpha must not be exposed or tone mapped: {pixel:?}"
        );
        images.push(pixels);
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
        let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&g);
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(
            &pixels[(32 * 64 + 32) * 4..][..4],
            [0, 0, 255, 255],
            "UI must follow scene color processing"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    for pixels in &images[1..] {
        for (a, b) in pixels.iter().zip(&images[0]) {
            assert!(a.abs_diff(*b) <= 1, "RGBA/BGRA linear/sRGB transfer differ");
        }
    }
}

#[test]
fn hdr_graph_keeps_immutable_draw_arcs_across_frame_reuse() {
    let mut manager = RenderResourceManager::new();
    let source = geometry_fixture::source(&mut manager);
    let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    view.color_pipeline = Some(ColorPipeline3D::default());
    let frame = GeometryFrame::build(
        "main",
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
    let original = &frame.views[0].draws[0];
    for _ in 0..2 {
        let g = geometry_fixture::graph(&frame);
        let draw = g
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| {
                if let RenderPassCommand::DrawGeometry { draw } = c {
                    Some(draw)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            std::sync::Arc::ptr_eq(original, draw),
            "appending the same immutable frame cloned all HDR draws"
        );
    }
}
