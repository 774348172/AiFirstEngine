use super::*;
use engine_runtime::{color_pipeline::*, environment_lighting::*, render_graph::*};
use std::sync::Arc;

pub(super) fn config() -> ImageBasedLighting3D {
    serde_json::from_value(serde_json::json!({"environment":{"id":"studio","type":"environment","guid":"guid-studio"},"intensity":1.0})).unwrap()
}
#[test]
fn ibl_authoring_reference_numeric_bounds_and_old_reader_rejection() {
    let mut p = ColorPipeline3D {
        ibl: Some(config()),
        ..Default::default()
    };
    p.validate().unwrap();
    let value = serde_json::to_value(&p).unwrap();
    assert_eq!(value["ibl"]["intensity"], 1.0);
    for field in ["id", "guid", "type"] {
        let mut bad = value.clone();
        bad["ibl"]["environment"][field] = "".into();
        assert!(serde_json::from_value::<ColorPipeline3D>(bad)
            .unwrap()
            .validate()
            .is_err());
    }
    for x in [-1., 101., f32::NAN] {
        p.ibl.as_mut().unwrap().intensity = x.into();
        assert!(p.validate().is_err());
    }
    let mut bad = value.clone();
    bad["ibl"]["secretRotation"] = true.into();
    assert!(serde_json::from_value::<ColorPipeline3D>(bad).is_err());
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    #[allow(dead_code)]
    struct PreviousColor {
        sky: Option<GradientSky3D>,
        fog: Option<DistanceFog3D>,
        schema_version: String,
        exposure_ev: f32,
        tone_mapping: String,
        anti_aliasing: String,
        ambient_diffuse: [f32; 3],
    }
    assert!(serde_json::from_value::<PreviousColor>(value).is_err());
}
#[cfg(feature = "real-wgpu")]
pub(super) fn map(scale: f32) -> Arc<EnvironmentMap> {
    let mut w = vec![0u32; 32];
    w[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, 2, 2, 32]);
    for k in 0..3 {
        w[4 + k] = ([0.4, 0.6, 0.8][k] * scale / 0.2820947918).to_bits();
    }
    for level in 0..2 {
        let size = 2 >> level;
        for face in 0..6 {
            for _ in 0..size * size {
                let c = face_color(face, level);
                w.extend(c.map(|v| (v as f32 * scale).to_bits()));
            }
        }
    }
    for y in 0..32 {
        for x in 0..32 {
            let nv = (x as f32 + 0.5) / 32.;
            let rough = (y as f32 + 0.5) / 32.;
            w.extend([0.2 + 0.3 * nv + 0.1 * rough, 0.05 * rough].map(f32::to_bits));
        }
    }
    EnvironmentMap::from_bytes(&w.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>()).unwrap()
}
#[cfg(feature = "real-wgpu")]
pub(super) fn face_color(face: usize, level: usize) -> [f64; 3] {
    let c = [
        [1.2, 0.1, 0.2],
        [0.1, 1.2, 0.2],
        [0.2, 0.1, 1.2],
        [1.1, 0.8, 0.1],
        [0.2, 0.9, 1.1],
        [0.8, 0.2, 0.8],
    ][face];
    c.map(|v| if level == 0 { v } else { 0.3 + v * 0.1 })
}
#[cfg(feature = "real-wgpu")]
fn graph(
    map: Arc<EnvironmentMap>,
    normal: [f32; 3],
    metallic: f32,
    roughness: f32,
    intensity: f32,
    unlit: bool,
) -> RenderGraph {
    let mut manager = RenderResourceManager::new();
    let mut g = standard_lit_fixture::lit_triangle();
    for vertex in g.vertex_bytes.chunks_exact_mut(24) {
        for k in 0..3 {
            vertex[12 + k * 4..16 + k * 4].copy_from_slice(&normal[k].to_le_bytes());
        }
    }
    // Stable identity per immutable normal variant, including replay of previous variants.
    let name = format!("mesh-{normal:?}");
    let mesh = geometry_fixture::prepare(&name, RenderAssetData::Mesh(g), &mut manager);
    let mut m = if unlit {
        geometry_fixture::unlit([0.8, 0.4, 0.2, 1.])
    } else {
        standard_lit_fixture::standard_lit([0.8, 0.4, 0.2, 1.])
    };
    if !unlit {
        for (name, v) in [("metallic", metallic), ("roughness", roughness)] {
            m.parameters
                .insert(name.into(), MaterialParameter::Float(v));
        }
        m.parameters
            .insert("emissive".into(), MaterialParameter::Vector4([0.; 4]));
    }
    let material =
        geometry_fixture::prepare("material", RenderAssetData::Material(m), &mut manager);
    let prepared = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
    let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    view.projection = Projection::Orthographic {
        size: 2.,
        near: 0.1,
        far: 10.,
    };
    view.clear_policy = ClearPolicy::Color {
        color: [0., 0., 0., 1.],
    };
    let mut ibl = config();
    ibl.intensity = intensity.into();
    view.color_pipeline = Some(ColorPipeline3D {
        ibl: Some(ibl),
        anti_aliasing: AntiAliasing3D::None,
        ..Default::default()
    });
    let mut frame = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &[geometry_fixture::instance(&prepared, 1, [0., 0., -2.])],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    if !unlit {
        for d in &mut frame.views[0].draws {
            Arc::make_mut(d).environment = Some(Arc::new(EnvironmentLighting {
                map: map.clone(),
                intensity: intensity.into(),
            }));
        }
    }
    geometry_fixture::graph(&frame)
}
#[cfg(feature = "real-wgpu")]
fn display(x: f64) -> u8 {
    let x = x.max(0.);
    let t = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
    let s = if t <= 0.0031308 {
        12.92 * t
    } else {
        1.055 * t.powf(1. / 2.4) - 0.055
    };
    (255. * s).round() as u8
}
#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_ibl_direction_metallic_roughness_intensity_and_unlit_reference() {
    use engine_runtime::{
        rhi_command_plan::compile_render_graph_to_rhi_plan, wgpu_backend::real::RealWgpuBackend,
    };
    let map = map(1.);
    for (normal, face) in [
        ([0., 0., 1.], 4),
        (
            [
                std::f32::consts::FRAC_1_SQRT_2,
                0.,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            0,
        ),
        (
            [
                -std::f32::consts::FRAC_1_SQRT_2,
                0.,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            1,
        ),
        (
            [
                0.,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            2,
        ),
        (
            [
                0.,
                -std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            3,
        ),
    ] {
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        for metallic in [0., 0.5, 1.] {
            for rough in [0.2, 0.8] {
                for intensity in [0., 1., 2.] {
                    let graph = graph(map.clone(), normal, metallic, rough, intensity, false);
                    let plan = compile_render_graph_to_rhi_plan(&graph);
                    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
                    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
                    let p = &pixels[(32 * 64 + 32) * 4..][..4];
                    let nv = f64::from(normal[2]);
                    let r = f64::from(rough);
                    let m = f64::from(metallic);
                    let base = [0.8, 0.4, 0.2];
                    let a = 0.2 + 0.3 * nv + 0.1 * r;
                    let b = 0.05 * r;
                    for k in 0..3 {
                        let f0 = 0.04 * (1. - m) + base[k] * m;
                        let f = f0 + ((1. - r).max(f0) - f0) * (1. - nv).powi(5);
                        let l = face_color(face, 0)[k] * (1. - r) + face_color(face, 1)[k] * r;
                        let linear = ((1. - f) * (1. - m) * base[k] * [0.4, 0.6, 0.8][k]
                            + l * (f0 * a + b))
                            * f64::from(intensity);
                        let expected = display(linear);
                        assert!((i16::from(p[k])-i16::from(expected)).abs()<=2,"face={face} metal={m} rough={r} intensity={intensity} channel={k} actual={} expected={expected}",p[k]);
                    }
                    assert_eq!(p[3], 255);
                }
            }
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let graph = graph(map, [0., 0., 1.], 0., 0.5, 1., true);
    let plan = compile_render_graph_to_rhi_plan(&graph);
    let p = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    let p = &p[(32 * 64 + 32) * 4..][..4];
    for k in 0..3 {
        assert!((i16::from(p[k]) - i16::from(display([0.8, 0.4, 0.2][k]))).abs() <= 2);
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_ibl_missing_backing_and_upload_limits_reject_before_rendering() {
    use engine_runtime::{
        rhi_command_plan::compile_render_graph_to_rhi_plan,
        wgpu_backend::real::{GeometryUploadBudget, RealWgpuBackend},
    };
    let mut g = graph(map(1.), [0., 0., 1.], 1., 0.5, 1., false);
    for c in g.passes.iter_mut().flat_map(|p| &mut p.commands) {
        if let RenderPassCommand::DrawGeometry { draw } = c {
            Arc::make_mut(draw).environment = None;
        }
    }
    assert!(compile_render_graph_to_rhi_plan(&g).has_errors());
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    // Install unchanged mesh/parameters/clears first, so the rejection below
    // actually reaches the environment owner rather than an earlier upload.
    let previous =
        compile_render_graph_to_rhi_plan(&graph(map(0.1), [0., 0., 1.], 1., 0.5, 1., false));
    backend
        .render_plan_to_rgba_bytes(&previous, 64, 64)
        .unwrap();
    backend
        .set_geometry_upload_budget(GeometryUploadBudget {
            bytes_per_frame: 4,
            staging_bytes: 4,
            ..Default::default()
        })
        .unwrap();
    let plan = compile_render_graph_to_rhi_plan(&graph(map(1.), [0., 0., 1.], 1., 0.5, 1., false));
    let before = backend.gpu_lifecycle_stats().submitted;
    let failure = backend
        .render_plan_to_rgba_bytes(&plan, 64, 64)
        .unwrap_err();
    assert!(failure.contains("environment.upload_capacity"), "{failure}");
    assert_eq!(backend.gpu_lifecycle_stats().submitted, before);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_ibl_three_in_flight_environment_versions_and_replay_preserve_completion_ownership() {
    use engine_runtime::{
        rhi_command_plan::compile_render_graph_to_rhi_plan, wgpu_backend::real::RealWgpuBackend,
    };
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut backend = RealWgpuBackend::from_shared_device_queue(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        64,
        64,
        "S2-C IBL in-flight lights",
    );
    let first = map(1.);
    let second = map(0.1);
    let frames: Vec<_> = [first.clone(), second, first]
        .into_iter()
        .map(|m| {
            let g = graph(m, [0., 0., 1.], 1., 0.5, 1., false);
            let plan = compile_render_graph_to_rhi_plan(&g);
            assert!(!plan.has_errors());
            plan
        })
        .collect();
    let textures: Vec<_> = (0..3)
        .map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("S2-C IBL in-flight output"),
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
        label: Some("S2-C IBL three outputs"),
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
    assert!(mapped[offset + 1] > mapped[size + offset + 1] + 30);
    drop(mapped);
    buffer.unmap();
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
}
