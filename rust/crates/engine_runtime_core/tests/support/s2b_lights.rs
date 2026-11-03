use super::*;
use engine_runtime::render_projection_adapter::LightProjection;
use engine_runtime::spatial_contract::{Light3D, LightKind};
use std::sync::Arc;
fn light(kind: LightKind, position: [f32; 3], color: [f32; 3]) -> LightProjection {
    LightProjection {
        component: Light3D {
            cast_shadows: false,
            schema_version: "light3d.v1".into(),
            enabled: true,
            color,
            kind,
            layers: vec!["main".into()],
        },
        position,
        direction: [0., 0., -1.],
        origin: [0.; 3],
    }
}
fn directional() -> LightProjection {
    light(
        LightKind::Directional {
            illuminance_lux: 1.,
        },
        [0.; 3],
        [1.; 3],
    )
}
fn source(manager: &mut RenderResourceManager) -> PreparedGeometry {
    let mesh = geometry_fixture::prepare(
        "light-mesh",
        RenderAssetData::Mesh(standard_lit_fixture::lit_triangle()),
        manager,
    );
    let mut material = standard_lit_fixture::standard_lit([0.5; 4]);
    material
        .parameters
        .insert("emissive".into(), MaterialParameter::Vector4([0.; 4]));
    let material = geometry_fixture::prepare(
        "light-material",
        RenderAssetData::Material(material),
        manager,
    );
    PreparedGeometry::from_assets(&mesh, &material, 0, manager).unwrap()
}
#[test]
fn fields_reject_nonfinite_ranges_negative_units_and_cone_degeneracy() {
    for kind in [
        LightKind::Point {
            intensity_candela: -1.,
            range_m: 1.,
        },
        LightKind::Point {
            intensity_candela: 1.,
            range_m: 0.,
        },
        LightKind::Directional {
            illuminance_lux: f32::INFINITY,
        },
        LightKind::Spot {
            intensity_candela: 1.,
            range_m: 2.,
            inner_half_angle_degrees: 45.,
            outer_half_angle_degrees: 45.,
        },
        LightKind::Spot {
            intensity_candela: 1.,
            range_m: 2.,
            inner_half_angle_degrees: 0.,
            outer_half_angle_degrees: 0.00001,
        },
    ] {
        assert!(light(kind, [0.; 3], [1.; 3]).component.validate().is_err());
    }
    let mut v = directional();
    v.component.color[0] = -1.;
    assert!(v.component.validate().is_err());
    v = directional();
    v.component.layers = vec!["a".into(), "a".into()];
    assert!(v.component.validate().is_err());
}
#[test]
fn worst_overlap_is_complete_bounded_and_overflow_rejected() {
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.)
        .evaluate(CameraPose::default())
        .unwrap();
    let lights = vec![directional(); 256];
    let list = ClusteredLights::build(&view, &lights).unwrap();
    assert_eq!(list.light_count(), 256);
    assert_eq!(list.max_cluster_lights(), 256);
    for cell in 0..3456 {
        assert_eq!(
            list.cluster_lights(cell).unwrap(),
            (0..256).collect::<Vec<_>>()
        );
    }
    assert!(ClusteredLights::build(&view, &vec![directional(); 257]).is_err());
    assert!(
        serde_json::to_vec(&list).unwrap().len() < 400_000,
        "bounded bitset storage, no repeated 256-entry lists"
    );
}
#[test]
fn sphere_binning_never_loses_an_inside_sample_for_perspective_or_ortho() {
    for orthographic in [false, true] {
        let mut view = RenderView::perspective(1, 128, 96, 65., 0.1, 100.);
        if orthographic {
            view.projection = Projection::Orthographic {
                size: 8.,
                near: 0.1,
                far: 100.,
            };
        }
        let view = view
            .evaluate(CameraPose {
                position: [1., 2., 3.],
                ..Default::default()
            })
            .unwrap();
        for (position, radius) in [
            ([0., 0., -2.], 0.5),
            ([2., 2., -4.], 2.),
            ([1., 2., 2.9], 1.),
            ([0., 0., -75.], 10.),
        ] {
            let l = light(
                LightKind::Point {
                    intensity_candela: 1.,
                    range_m: radius,
                },
                position,
                [1.; 3],
            );
            let list = ClusteredLights::build(&view, &[l]).unwrap();
            let m = view.view_projection();
            // Independent sample oracle, not a copy of the plane/sphere binning implementation.
            for ix in -4i32..=4 {
                for iy in -4i32..=4 {
                    for iz in -4i32..=4 {
                        let d = [ix as f32 / 4., iy as f32 / 4., iz as f32 / 4.];
                        if d.iter().map(|x| x * x).sum::<f32>() > 1. {
                            continue;
                        }
                        let p = std::array::from_fn::<_, 3, _>(|j| position[j] + radius * d[j]);
                        let clip = std::array::from_fn::<_, 4, _>(|j| {
                            m[j] * p[0] + m[4 + j] * p[1] + m[8 + j] * p[2] + m[12 + j]
                        });
                        let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];
                        if clip[3] <= 0.
                            || ndc[0].abs() > 1.
                            || ndc[1].abs() > 1.
                            || ndc[2] < 0.
                            || ndc[2] >= 1.
                        {
                            continue;
                        }
                        let x = (((ndc[0] + 1.) * 8.) as usize).min(15);
                        let y = (((1. - ndc[1]) * 4.5) as usize).min(8);
                        let z = ((ndc[2] * 24.) as usize).min(23);
                        assert!(
                            list.cluster_lights(z * 144 + y * 16 + x)
                                .unwrap()
                                .contains(&0),
                            "lost {p:?}, ortho={orthographic}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn frozen_light_versions_do_not_change_material_pipeline_or_old_frame_and_bad_view_is_rejected() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let original = geometry_fixture::frame(&source, &manager);
    let mut first = original.clone();
    first.set_view_lights(1, &[directional()]).unwrap();
    let saved = first.views[0].draws[0].clone();
    let mut second = first.clone();
    let mut changed = directional();
    changed.component.color = [0., 1., 0.];
    second.set_view_lights(1, &[changed]).unwrap();
    assert_ne!(
        first.views[0].draws[0].lighting,
        second.views[0].draws[0].lighting
    );
    assert_eq!(saved, first.views[0].draws[0]);
    assert_eq!(
        saved.pipeline_identity("rgba8"),
        second.views[0].draws[0].pipeline_identity("rgba8")
    );
    assert_ne!(
        saved.pipeline_identity("rgba8"),
        original.views[0].draws[0].pipeline_identity("rgba8")
    );
    let mut wrong = saved.as_ref().clone();
    Arc::make_mut(&mut wrong.view).viewport[2] = 32;
    assert!(wrong.validate().is_err());
    let mut wire = serde_json::to_value(saved.as_ref()).unwrap();
    wire["lighting"]["words"][16] = 257.into();
    let corrupt: DrawGeometry = serde_json::from_value(wire).unwrap();
    assert!(corrupt.validate().is_err());
    for words in [serde_json::json!([]), serde_json::json!([0, 0, 0])] {
        let mut wire = serde_json::to_value(saved.as_ref()).unwrap();
        wire["lighting"]["words"] = words;
        let corrupt: DrawGeometry = serde_json::from_value(wire).unwrap();
        let light = corrupt.lighting.as_ref().unwrap();
        assert_eq!(light.light_count(), 0);
        assert_eq!(light.max_cluster_lights(), 0);
        assert!(light.cluster_lights(0).is_none());
        assert!(corrupt.validate().is_err());
    }
    let mut wire = serde_json::to_value(saved.as_ref()).unwrap();
    wire["lighting"]["words"][18] = 1.into();
    let corrupt: DrawGeometry = serde_json::from_value(wire).unwrap();
    assert!(corrupt.validate().is_err());
}
#[cfg(feature = "real-wgpu")]
mod gpu_s2b {
    use super::*;
    use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    #[test]
    #[ignore = "requires real GPU"]
    fn real_light_kinds_match_independent_reference_and_preview_does_not_leak() {
        let mut manager = RenderResourceManager::new();
        let source = source(&mut manager);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let ticket = backend
            .begin_geometry_upload(std::slice::from_ref(&source), &[])
            .unwrap();
        for index in 1..=16 {
            backend.pump_geometry_uploads(index).unwrap();
            if let Some(token) = backend.last_gpu_submission() {
                backend
                    .wait_gpu_submission(token, std::time::Duration::from_secs(5))
                    .unwrap();
            }
            if backend.geometry_upload_state(ticket).unwrap()
                == engine_runtime::wgpu_backend::real::GeometryUploadState::Ready
            {
                break;
            }
        }
        assert_eq!(
            backend.geometry_upload_state(ticket).unwrap(),
            engine_runtime::wgpu_backend::real::GeometryUploadState::Ready
        );
        backend
            .set_material_preview_illumination([0., 0., 1.], [100.; 3], [100.; 3])
            .unwrap();
        for (label, lights) in [
            ("dark", vec![]),
            ("directional", vec![directional()]),
            (
                "point",
                vec![light(
                    LightKind::Point {
                        intensity_candela: 4.,
                        range_m: 4.,
                    },
                    [0., 0., 0.],
                    [1.; 3],
                )],
            ),
            (
                "spot",
                vec![light(
                    LightKind::Spot {
                        intensity_candela: 4.,
                        range_m: 4.,
                        inner_half_angle_degrees: 15.,
                        outer_half_angle_degrees: 30.,
                    },
                    [0., 0., 0.],
                    [1.; 3],
                )],
            ),
            ("sum", vec![directional(), directional()]),
            (
                "point-outside-range",
                vec![light(
                    LightKind::Point {
                        intensity_candela: 4.,
                        range_m: 1.,
                    },
                    [0., 0., 0.],
                    [1.; 3],
                )],
            ),
            ("spot-outside-cone", {
                let mut l = light(
                    LightKind::Spot {
                        intensity_candela: 4.,
                        range_m: 4.,
                        inner_half_angle_degrees: 15.,
                        outer_half_angle_degrees: 30.,
                    },
                    [0., 0., 0.],
                    [1.; 3],
                );
                l.direction = [1., 0., 0.];
                vec![l]
            }),
            ("last-bit-255", {
                let mut list = vec![directional(); 256];
                for l in &mut list {
                    l.component.color = [0.; 3];
                }
                list[255].component.color = [0., 1., 0.];
                list
            }),
        ] {
            let mut frame = geometry_fixture::frame(&source, &manager);
            frame.set_view_lights(1, &lights).unwrap();
            let plan = compile_render_graph_to_rhi_plan(&geometry_fixture::graph(&frame));
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
            // Pixel center reconstructs perspective ray independently in f64.
            let x = (2. * 32.5 / 64. - 1.) * 2. * (30f64.to_radians().tan());
            let y = (1. - 2. * 32.5 / 64.) * 2. * (30f64.to_radians().tan());
            let p = [x, y, -2.];
            let norm = |v: [f64; 3]| {
                let l = v.iter().map(|n| n * n).sum::<f64>().sqrt();
                v.map(|n| n / l)
            };
            let v = norm([-x, -y, 2.]);
            let mut value = [0.; 3];
            for light in &lights {
                let (l, attenuation) = match light.component.kind {
                    LightKind::Directional { illuminance_lux } => {
                        ([0., 0., 1.], f64::from(illuminance_lux))
                    }
                    LightKind::Point {
                        intensity_candela,
                        range_m,
                    }
                    | LightKind::Spot {
                        intensity_candela,
                        range_m,
                        ..
                    } => {
                        let d2 = p.iter().map(|n| n * n).sum::<f64>();
                        let r = d2.sqrt() / f64::from(range_m);
                        let mut a = (1. - r.powi(4)).max(0.).powi(2) * f64::from(intensity_candela)
                            / d2.max(0.01);
                        if let LightKind::Spot {
                            inner_half_angle_degrees,
                            outer_half_angle_degrees,
                            ..
                        } = light.component.kind
                        {
                            let dir = light.direction.map(f64::from);
                            let toward = [p[0], p[1], p[2]];
                            let cosine =
                                (0..3).map(|j| dir[j] * toward[j]).sum::<f64>() / d2.sqrt();
                            let lo = f64::from(outer_half_angle_degrees).to_radians().cos();
                            let hi = f64::from(inner_half_angle_degrees).to_radians().cos();
                            let t = ((cosine - lo) / (hi - lo)).clamp(0., 1.);
                            a *= t * t * (3. - 2. * t);
                        }
                        (norm([-p[0], -p[1], -p[2]]), a)
                    }
                };
                let h = norm([v[0] + l[0], v[1] + l[1], v[2] + l[2]]);
                let nl = l[2].max(0.);
                let nv = v[2].max(1e-5);
                let nh = h[2].max(0.);
                let vh = (0..3).map(|j| v[j] * h[j]).sum::<f64>().max(0.);
                let a2 = 0.5f64.powi(4);
                let d = a2 / (std::f64::consts::PI * (nh * nh * (a2 - 1.) + 1.).powi(2));
                let g = |cos: f64| 2. * cos / (cos + (a2 + (1. - a2) * cos * cos).sqrt() + 1e-6);
                let f = 0.04 + 0.96 * (1. - vh).powi(5);
                let diffuse = (1. - f) * 0.5 / std::f64::consts::PI;
                let spec = d * g(nl) * g(nv) * f / (4. * nl * nv).max(1e-5);
                for c in 0..3 {
                    value[c] +=
                        (diffuse + spec) * nl * attenuation * f64::from(light.component.color[c]);
                }
            }
            let expected = value.map(|v| (v.clamp(0., 1.) * 255.).round() as i32);
            let pixel = &pixels[(32 * 64 + 32) * 4..][..4];
            for c in 0..3 {
                assert!(
                    (i32::from(pixel[c]) - expected[c]).abs() <= 2,
                    "{label}: {pixel:?}, expected {expected:?}"
                );
            }
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    #[test]
    #[ignore = "requires real GPU"]
    fn independent_views_keep_light_lists_viewports_and_resize_separate() {
        let mut manager = RenderResourceManager::new();
        let source = source(&mut manager);
        let mut backend = RealWgpuBackend::new_offscreen(128, 64).unwrap();
        let make = |extent: [u32; 2]| {
            let mut a = RenderView::perspective(1, extent[0], extent[1], 60., 0.1, 10.);
            a.viewport = [0, 0, extent[0] / 2, extent[1]];
            let mut b = a.clone();
            b.view_id = 2;
            b.viewport[0] = extent[0] / 2;
            let mut frame = GeometryFrame::build(
                "main",
                extent,
                &[
                    GeometryViewInput {
                        view: &a,
                        pose: CameraPose::default(),
                    },
                    GeometryViewInput {
                        view: &b,
                        pose: CameraPose::default(),
                    },
                ],
                &[geometry_fixture::instance(&source, 1, [0., 0., -2.])],
                &manager,
                DiagnosticLevel::Off,
            )
            .unwrap();
            let mut red = directional();
            red.component.color = [1., 0., 0.];
            let mut green = directional();
            green.component.color = [0., 1., 0.];
            frame.set_view_lights(1, &[red]).unwrap();
            frame.set_view_lights(2, &[green]).unwrap();
            compile_render_graph_to_rhi_plan(&geometry_fixture::graph(&frame))
        };
        for extent in [[128, 64], [256, 96], [128, 64]] {
            let plan = make(extent);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let pixels = backend
                .render_plan_to_rgba_bytes(&plan, extent[0], extent[1])
                .unwrap();
            for (x, channel) in [(extent[0] / 4, 0), (extent[0] * 3 / 4, 1)] {
                let offset = ((extent[1] / 2 * extent[0] + x) * 4) as usize;
                let p = &pixels[offset..offset + 4];
                assert!(p[channel] > 30, "{extent:?} {p:?}");
                assert_eq!(p[1 - channel], 0, "cross-view light leak");
                assert_eq!(p[2], 0);
            }
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    #[test]
    #[ignore = "requires real GPU"]
    fn ninth_distinct_light_dataset_is_rejected_before_submission_and_old_plan_recovers() {
        let mut manager = RenderResourceManager::new();
        let source = source(&mut manager);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let make = |count: u64, distinct: bool| {
            let views = (1..=count)
                .map(|id| RenderView::perspective(id, 64, 64, 60., 0.1, 10.))
                .collect::<Vec<_>>();
            let inputs = views
                .iter()
                .map(|v| GeometryViewInput {
                    view: v,
                    pose: CameraPose::default(),
                })
                .collect::<Vec<_>>();
            let mut frame = GeometryFrame::build(
                "main",
                [64, 64],
                &inputs,
                &[geometry_fixture::instance(&source, 1, [0., 0., -2.])],
                &manager,
                DiagnosticLevel::Off,
            )
            .unwrap();
            for id in 1..=count {
                let mut l = directional();
                l.component.color = [if distinct { id as f32 / 16. } else { 1. / 16. }, 1., 0.];
                frame.set_view_lights(id, &[l]).unwrap();
            }
            let plan = compile_render_graph_to_rhi_plan(&geometry_fixture::graph(&frame));
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            plan
        };
        let good = make(1, true);
        let pixels = backend.render_plan_to_rgba_bytes(&good, 64, 64).unwrap();
        let eight = make(8, true);
        backend.render_plan_to_rgba_bytes(&eight, 64, 64).unwrap();
        let sixteen_shared = make(16, false);
        assert_eq!(
            backend
                .render_plan_to_rgba_bytes(&sixteen_shared, 64, 64)
                .unwrap(),
            pixels
        );
        let before = backend.last_gpu_submission();
        let error = backend
            .render_plan_to_rgba_bytes(&make(9, true), 64, 64)
            .unwrap_err();
        assert!(error.contains("light.view_capacity"), "{error}");
        assert_eq!(backend.last_gpu_submission(), before);
        assert_eq!(
            backend.render_plan_to_rgba_bytes(&good, 64, 64).unwrap(),
            pixels
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    #[test]
    #[ignore = "requires real GPU"]
    fn three_in_flight_light_versions_use_immutable_native_storage_until_completion() {
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
            "S2-B in-flight lights",
        );
        let mut manager = RenderResourceManager::new();
        let source = source(&mut manager);
        let mut frames = Vec::new();
        for color in [[1., 0., 0.], [0., 1., 0.], [1., 0., 0.]] {
            let mut frame = geometry_fixture::frame(&source, &manager);
            let mut l = directional();
            l.component.color = color;
            frame.set_view_lights(1, &[l]).unwrap();
            frames.push(compile_render_graph_to_rhi_plan(&geometry_fixture::graph(
                &frame,
            )));
        }
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
            let report = backend
                .execute_plan_to_surface_view(plan, &texture.create_view(&Default::default()));
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
        assert!(mapped[offset] > 30);
        assert_eq!(mapped[offset + 1], 0);
        assert_eq!(mapped[size + offset], 0);
        assert!(mapped[size + offset + 1] > 30);
        drop(mapped);
        buffer.unmap();
        let mut sample = engine_runtime::resource_memory::ResourceMemorySample::new(0, 0, 0);
        backend.observe_memory(&mut sample);
        assert!(
            sample.allocations.iter().any(|a| a.domain
                == engine_runtime::resource_memory::MemoryDomain::Gpu
                && a.bytes == 110736),
            "actual one-light bitset allocation must be observed"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
    }
}

#[test]
fn bm04_light_count_cluster_build_cost_is_recorded_without_qualifying_frame_time() {
    let view = RenderView::perspective(1, 1920, 1080, 60., 0.1, 100.)
        .evaluate(CameraPose::default())
        .unwrap();
    let lights = (0..145)
        .map(|i| {
            light(
                if i < 128 {
                    LightKind::Point {
                        intensity_candela: 1.,
                        range_m: 32768.,
                    }
                } else if i < 144 {
                    LightKind::Spot {
                        intensity_candela: 1.,
                        range_m: 32768.,
                        inner_half_angle_degrees: 15.,
                        outer_half_angle_degrees: 30.,
                    }
                } else {
                    LightKind::Directional {
                        illuminance_lux: 1.,
                    }
                },
                [0., 0., -5.],
                [1.; 3],
            )
        })
        .collect::<Vec<_>>();
    let mut times = Vec::new();
    for _ in 0..11 {
        let start = std::time::Instant::now();
        let list =
            ClusteredLights::build(std::hint::black_box(&view), std::hint::black_box(&lights))
                .unwrap();
        times.push(start.elapsed().as_secs_f64() * 1000.);
        assert_eq!(list.max_cluster_lights(), 145);
        assert_eq!(
            list.cluster_lights(3455).unwrap(),
            (0..145).collect::<Vec<_>>()
        );
    }
    times.sort_by(f64::total_cmp);
    eprintln!("S2-B 145-light CPU cluster build only: median_ms={:.3} max_ms={:.3}; not BM04 frame qualification",times[5],times[10]);
}
