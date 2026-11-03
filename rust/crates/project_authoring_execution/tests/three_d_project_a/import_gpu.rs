//! Real Compiler -> RuntimePackage -> Prepare -> GeometryFrame -> WGPU consumption.
use super::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_graph::*;
use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;
use engine_runtime::runtime_texture::TextureWrap;
use engine_runtime::wgpu_backend::real::RealWgpuBackend;

const DIRECTION: [u8; 16] = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
];
fn capture(f: &ModelFixture, matrix: [f32; 16]) -> Vec<u8> {
    capture_preview(f, matrix, None)
}
fn capture_preview(
    f: &ModelFixture,
    matrix: [f32; 16],
    preview: Option<([f32; 3], [f32; 3], [f32; 3])>,
) -> Vec<u8> {
    capture_extent(f, matrix, preview, [64, 64], 4.)
}
fn capture_extent(
    f: &ModelFixture,
    matrix: [f32; 16],
    preview: Option<([f32; 3], [f32; 3], [f32; 3])>,
    extent: [u32; 2],
    size: f32,
) -> Vec<u8> {
    let cost_started = std::time::Instant::now();
    let input = f.input();
    let prepare_done = std::time::Instant::now();
    let (_, mut loader) = package(f, &input);
    let package_done = std::time::Instant::now();
    let component = renderer(&input);
    let mut async_handles = Vec::new();
    for reference in
        std::iter::once(&component.mesh.0).chain(component.materials.values().map(|m| &m.0))
    {
        async_handles.push(loader.request_async(reference).unwrap());
    }
    let started = std::time::Instant::now();
    while async_handles.iter().any(|h| {
        loader.get_handle_state(h)
            == Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Loading)
    }) {
        loader.poll_async();
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        std::thread::yield_now();
    }
    for h in &async_handles {
        assert_eq!(
            loader.get_handle_state(h),
            Some(engine_runtime::runtime_asset::RuntimeAssetLoadState::Ready),
            "{:?}",
            loader.streaming_error(h)
        );
    }
    let async_done = std::time::Instant::now();
    let mut manager = RenderResourceManager::new();
    let bound = RuntimeRenderAssetProducer::new()
        .prepare_mesh_renderer(0, &renderer(&input), &mut loader, &mut manager)
        .unwrap();
    let binding_done = std::time::Instant::now();
    let mut backend = RealWgpuBackend::new_offscreen(extent[0], extent[1])
        .expect("A2.2 real GPU adapter required");
    if let Some((direction, radiance, ambient)) = preview {
        backend
            .set_material_preview_illumination(direction, radiance, ambient)
            .unwrap();
    }
    eprintln!("A2.2 GPU context: {:?}", backend.device_context());
    let adapter_done = std::time::Instant::now();
    let ticket = backend
        .begin_geometry_upload(&bound.geometry, &bound.textures)
        .unwrap();
    for frame in 1..=128 {
        backend.pump_geometry_uploads(frame).unwrap();
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
    let upload_done = std::time::Instant::now();
    let mut view = RenderView::perspective(1, extent[0], extent[1], 60., 0.1, 20.);
    view.projection = Projection::Orthographic {
        size,
        near: 0.1,
        far: 20.,
    };
    let instances: Vec<_> = bound
        .geometry
        .iter()
        .enumerate()
        .map(|(i, geometry)| GeometryInstance {
            instance_id: i as u64,
            source: geometry,
            world_transform: matrix,
            visibility: Visibility {
                authored: true,
                inherited: Evaluation::Known(true),
            },
            layers: LayerSelection::Named {
                names: vec!["main".into()],
            },
            provenance: Provenance::unspecified(ProducerStage::RenderProjection),
        })
        .collect();
    let frame = GeometryFrame::build(
        "main",
        extent,
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &instances,
        &manager,
        DiagnosticLevel::Summary,
    )
    .unwrap();
    let mut graph = RenderGraph::new("imported", 1);
    graph.output_target = Some("main".into());
    graph.resources.push(RenderResource::surface_backbuffer(
        "main", extent[0], extent[1],
    ));
    graph.passes.push(RenderPass {
        pass_id: "clear".into(),
        pass_name: "clear".into(),
        pass_kind: RenderPassKind::Clear,
        view_id: "global".into(),
        reads: vec![],
        writes: vec!["main".into()],
        color_targets: vec!["main".into()],
        depth_target: None,
        commands: vec![RenderPassCommand::Clear {
            target: "main".into(),
            color: color([0., 0., 0., 1.]),
        }]
        .into(),
        debug_source: None,
    });
    frame.append_to_graph(&mut graph);
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let pixels = backend
        .render_plan_to_rgba_bytes(&plan, extent[0], extent[1])
        .unwrap();
    let draw_done = std::time::Instant::now();
    eprintln!("S2-A test-stage-cost extent={extent:?} prepare_cook_ms={:.3} package_ms={:.3} async_ms={:.3} binding_ms={:.3} adapter_ms={:.3} upload_wait_ms={:.3} frame_graph_draw_readback_ms={:.3}",
        prepare_done.duration_since(cost_started).as_secs_f64()*1000.,
        package_done.duration_since(prepare_done).as_secs_f64()*1000.,
        async_done.duration_since(package_done).as_secs_f64()*1000.,
        binding_done.duration_since(async_done).as_secs_f64()*1000.,
        adapter_done.duration_since(binding_done).as_secs_f64()*1000.,
        upload_done.duration_since(adapter_done).as_secs_f64()*1000.,
        draw_done.duration_since(upload_done).as_secs_f64()*1000.);
    assert_eq!(backend.geometry_upload_count(), 1);
    backend.release_geometry_upload(ticket).unwrap();
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    for h in &async_handles {
        loader.release(h).unwrap();
    }
    assert!(loader.shutdown_streaming(std::time::Duration::from_secs(2)));
    pixels
}
fn matrix() -> [f32; 16] {
    let mut m = IDENTITY_MATRIX;
    m[14] = -3.;
    m
}
fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
fn close(actual: [u8; 4], expected: [u8; 4]) {
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
        "{actual:?} != {expected:?}, tolerance 3/255"
    );
}
fn evidence(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("AIFE_A22_EVIDENCE_ROOT") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        let file = fs::File::create(root.join(format!("{name}.png"))).unwrap();
        let mut encoder = png::Encoder::new(file, 64, 64);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(bytes)
            .unwrap();
    }
}

#[test]
#[ignore = "A2.2 real GPU acceptance"]
fn gpu_import_glb_direction_alpha_and_double_sided_are_actually_consumed() {
    let mut f = ModelFixture::new();
    embedded_glb(&mut f);
    let pixels = capture(&f, matrix());
    for (x, y, color) in [
        (23, 23, [255, 0, 0, 255]),
        (40, 23, [0, 255, 0, 255]),
        (23, 40, [0, 0, 255, 255]),
        (40, 40, [255, 255, 255, 255]),
    ] {
        close(pixel(&pixels, x, y), color);
    }
    evidence("import-glb-direction", &pixels);
    let mut f = ModelFixture::new();
    f.model["materials"][0]["pbrMetallicRoughness"]
        .as_object_mut()
        .unwrap()
        .remove("baseColorTexture");
    f.model.as_object_mut().unwrap().remove("images");
    f.model["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"] =
        json!([0.2, 0.4, 0.6, 0.5]);
    f.model["materials"][0]["alphaMode"] = json!("BLEND");
    f.model["materials"][0]["doubleSided"] = json!(true);
    // Remove now-unreferenced textures too: the mature parser checks structural references.
    f.model.as_object_mut().unwrap().remove("textures");
    f.save();
    let mut back = matrix();
    back[0] = -1.;
    back[10] = -1.;
    let blended = capture(&f, back);
    close(pixel(&blended, 32, 32), [26, 51, 76, 255]);
    evidence("import-alpha-double-sided", &blended);
    f.model["materials"][0]["doubleSided"] = json!(false);
    f.save();
    close(pixel(&capture(&f, back), 32, 32), [0, 0, 0, 255]);
}

fn addressed(i: i32, wrap: TextureWrap) -> usize {
    match wrap {
        TextureWrap::ClampToEdge => i.clamp(0, 1) as usize,
        TextureWrap::Repeat => i.rem_euclid(2) as usize,
        TextureWrap::MirroredRepeat => {
            let i = i.rem_euclid(4);
            if i < 2 {
                i as usize
            } else {
                (3 - i) as usize
            }
        }
    }
}
fn sample(uv: [f32; 2], linear: bool, wrap: TextureWrap) -> [u8; 4] {
    let texel = |x: i32, y: i32, c: usize| {
        f32::from(DIRECTION[(addressed(y, wrap) * 2 + addressed(x, wrap)) * 4 + c])
    };
    if !linear {
        return std::array::from_fn(|c| {
            texel((uv[0] * 2.).floor() as i32, (uv[1] * 2.).floor() as i32, c) as u8
        });
    }
    let x = uv[0] * 2. - 0.5;
    let y = uv[1] * 2. - 0.5;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    std::array::from_fn(|c| {
        ((texel(ix, iy, c) * (1. - fx) + texel(ix + 1, iy, c) * fx) * (1. - fy)
            + (texel(ix, iy + 1, c) * (1. - fx) + texel(ix + 1, iy + 1, c) * fx) * fy)
            .round() as u8
    })
}

#[test]
#[ignore = "A2.2 real GPU acceptance"]
fn gpu_import_sampler_min_mag_and_three_wrap_modes_match_independent_reference() {
    for (wrap_id, wrap) in [
        (10497, TextureWrap::Repeat),
        (33071, TextureWrap::ClampToEdge),
        (33648, TextureWrap::MirroredRepeat),
    ] {
        for linear_min in [false, true] {
            for scale in [3.0f32, 35.] {
                let mut f = ModelFixture::new();
                f.model["samplers"][0] = json!({"wrapS":wrap_id,"wrapT":wrap_id,"minFilter":if linear_min{9729}else{9728},"magFilter":if linear_min{9728}else{9729}});
                f.save();
                let path = f.project.root.join("Models/quad.bin");
                let mut bytes = fs::read(&path).unwrap();
                for row in bytes[96..128].chunks_exact_mut(4) {
                    let value = f32::from_le_bytes(row.try_into().unwrap());
                    row.copy_from_slice(&(value * scale - 1.).to_le_bytes());
                }
                fs::write(path, bytes).unwrap();
                let pixels = capture(&f, matrix());
                for x in [18, 25, 34, 44] {
                    for y in [19, 27, 36, 45] {
                        let uv = [
                            ((x as f32 + 0.5) - 16.) / 32. * scale - 1.,
                            ((y as f32 + 0.5) - 16.) / 32. * scale - 1.,
                        ];
                        let expected =
                            sample(uv, if scale > 10. { linear_min } else { !linear_min }, wrap);
                        let actual = pixel(&pixels, x, y);
                        assert!(actual.iter().zip(expected).all(|(a,b)| a.abs_diff(b)<=3),
                            "wrap={wrap_id} linear_min={linear_min} scale={scale} pixel=({x},{y}) uv={uv:?}: {actual:?} != {expected:?}");
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "A2.2 real GPU acceptance"]
fn gpu_import_coordinate_reflection_and_independent_box_have_correct_pixels() {
    let mut f = ModelFixture::new();
    f.descriptor["coordinates"]["handedness"] = json!("left");
    f.save();
    let mut turned = matrix();
    turned[0] = -1.;
    turned[10] = -1.;
    let pixels = capture(&f, turned);
    close(pixel(&pixels, 23, 23), [0, 255, 0, 255]);
    close(pixel(&pixels, 40, 23), [255, 0, 0, 255]);
    close(pixel(&pixels, 23, 40), [255, 255, 255, 255]);
    evidence("import-reflection", &pixels);
    let f = independent_box();
    let pixels = capture(&f, matrix());
    close(pixel(&pixels, 32, 32), [204, 0, 0, 255]);
    close(pixel(&pixels, 20, 32), [0, 0, 0, 255]);
    evidence("import-independent-box", &pixels);
}

#[test]
#[ignore = "A2.2 real GPU acceptance"]
fn gpu_import_opaque_ignores_factor_and_texture_alpha() {
    let mut f = ModelFixture::new();
    let mut image = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut image, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0, 255, 255, 255, 0])
            .unwrap();
    }
    fs::write(f.project.root.join("Models/direction.png"), image).unwrap();
    f.model["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"] = json!([1, 1, 1, 0.25]);
    f.save();
    let pixels = capture(&f, matrix());
    close(pixel(&pixels, 23, 23), [255, 0, 0, 255]);
    close(pixel(&pixels, 40, 40), [255, 255, 255, 255]);
}

fn s2a_fixture() -> ModelFixture {
    let mut f = ModelFixture::new();
    f.descriptor["materialStrategy"] = json!("standardLit");
    f.model["materials"][0]
        .as_object_mut()
        .unwrap()
        .remove("extensions");
    f.model["materials"][0]["pbrMetallicRoughness"] = json!({"baseColorFactor":[0.8,0.7,0.6,1.],"metallicFactor":0.7,"roughnessFactor":0.8,"baseColorTexture":{"index":0},"metallicRoughnessTexture":{"index":2}});
    f.model["materials"][0]["normalTexture"] = json!({"index":1,"scale":0.8});
    f.model["materials"][0]["occlusionTexture"] = json!({"index":3,"strength":0.75});
    f.model["materials"][0]["emissiveTexture"] = json!({"index":4});
    f.model["materials"][0]["emissiveFactor"] = json!([0.1, 0.15, 0.2]);
    f.model["images"] = json!([]);
    f.model["textures"] = json!([]);
    f.model["samplers"][0]["minFilter"] = json!(9987);
    for (i, c) in [
        [128, 180, 220, 255],
        [128, 230, 204, 255],
        [211, 153, 77, 255],
        [64, 0, 0, 255],
        [80, 110, 150, 255],
    ]
    .into_iter()
    .enumerate()
    {
        let file = fs::File::create(f.project.root.join(format!("Models/pbr-{i}.png"))).unwrap();
        let mut encoder = png::Encoder::new(file, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&c.repeat(4))
            .unwrap();
        f.model["images"]
            .as_array_mut()
            .unwrap()
            .push(json!({"uri":format!("pbr-{i}.png")}));
        f.model["textures"]
            .as_array_mut()
            .unwrap()
            .push(json!({"source":i,"sampler":0}));
    }
    f.save();
    f
}
fn normalize(v: [f64; 3]) -> [f64; 3] {
    let length = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    v.map(|x| x / length)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn linear(c: u8) -> f64 {
    let v = f64::from(c) / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
// Independent f64 oracle: fixed authored quad has T=+X, B=-Y, N=+Z.
fn pbr_reference(
    normal: [f64; 3],
    metal: f64,
    rough: f64,
    alpha: f64,
    direct: bool,
    ao: bool,
) -> [u8; 4] {
    let n = normalize(normal);
    let v = [0., 0., 1.];
    let l = [0., -0.6, 0.8];
    let h = normalize(std::array::from_fn(|i| v[i] + l[i]));
    let (nl, nv, nh, vh) = (
        dot(n, l).max(0.),
        dot(n, v).max(1e-5),
        dot(n, h).max(0.),
        dot(v, h).max(0.),
    );
    let a2 = rough.clamp(0.045, 1.).powi(4);
    let d = a2 / (std::f64::consts::PI * (nh * nh * (a2 - 1.) + 1.).powi(2));
    let smith = |x: f64| 2. * x / (x + (a2 + (1. - a2) * x * x).sqrt() + 1e-6);
    let base = [linear(128) * 0.8, linear(180) * 0.7, linear(220) * 0.6];
    let emission = [linear(80) * 0.1, linear(110) * 0.15, linear(150) * 0.2];
    let occ = if ao {
        1. - 0.75 + 0.75 * 64. / 255.
    } else {
        1.
    };
    let mut result = [0u8; 4];
    for c in 0..3 {
        let f0 = 0.04 * (1. - metal) + base[c] * metal;
        let f = f0 + (1. - f0) * (1. - vh).powi(5);
        let spec = d * smith(nl) * smith(nv) * f / (4. * nl * nv).max(1e-5);
        let diff = (1. - f) * (1. - metal) * base[c] / std::f64::consts::PI;
        let radiance = if direct { (diff + spec) * nl } else { 0. };
        let value = radiance + (1. - f0) * (1. - metal) * base[c] * 0.1 * occ + emission[c];
        result[c] = (value * alpha * 255.).clamp(0., 255.).round() as u8;
    }
    result[3] = 255;
    result
}
#[test]
#[ignore = "S2-A real GPU material pixels"]
fn s2a_gpu_standard_lit_five_maps_normal_roughness_alpha_and_default_dark_match_reference() {
    five_maps_reference(false);
}

#[test]
#[ignore = "S2-D BC3 real GPU material reference"]
fn s2d_bc3_gpu_five_maps_normal_roughness_alpha_match_independent_reference() {
    five_maps_reference(true);
}

fn five_maps_reference(compressed: bool) {
    let mut f = s2a_fixture();
    if compressed {
        f.descriptor["textureCompression"] = json!("bc3");
        for (i, c) in [
            [128, 180, 220, 255],
            [128, 230, 204, 255],
            [211, 153, 77, 255],
            [64, 0, 0, 255],
            [80, 110, 150, 255],
        ]
        .into_iter()
        .enumerate()
        {
            let file =
                fs::File::create(f.project.root.join(format!("Models/pbr-{i}.png"))).unwrap();
            let mut encoder = png::Encoder::new(file, 4, 4);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&c.repeat(16))
                .unwrap();
        }
        f.save();
        let input = f.input();
        assert_eq!(input.texture_payloads.len(), 5);
        for texture in &input.texture_payloads {
            assert!(texture.metadata.format.starts_with("bc3"));
            assert_eq!(texture.metadata.mip_count, 3);
            assert_eq!(texture.rgba8.len(), 48);
        }
    }
    let light = Some(([0., -0.6, 0.8], [1.; 3], [0.1; 3]));
    let n = [
        (128. / 255. * 2. - 1.) * 0.8,
        -(230. / 255. * 2. - 1.) * 0.8,
        204. / 255. * 2. - 1.,
    ];
    let metal = 0.7 * 77. / 255.;
    let rough = 0.8 * 153. / 255.;
    let result = capture_preview(&f, matrix(), light);
    close(
        pixel(&result, 32, 32),
        pbr_reference(n, metal, rough, 1., true, true),
    );
    for (x, y) in [(20, 20), (44, 44)] {
        close(
            pixel(&result, x, y),
            pbr_reference(n, metal, rough, 1., true, true),
        );
    }
    evidence("s2a-five-maps-preview", &result);
    // With no preview illumination the actual production default is emission only.
    let dark = capture(&f, matrix());
    close(
        pixel(&dark, 32, 32),
        [
            (linear(80) * 0.1 * 255.).round() as u8,
            (linear(110) * 0.15 * 255.).round() as u8,
            (linear(150) * 0.2 * 255.).round() as u8,
            255,
        ],
    );
    evidence("s2a-production-emission-only", &dark);
    f.model["materials"][0]["normalTexture"]["scale"] = json!(0.);
    f.save();
    let flat = capture_preview(&f, matrix(), light);
    close(
        pixel(&flat, 32, 32),
        pbr_reference([0., 0., 1.], metal, rough, 1., true, true),
    );
    evidence("s2a-flat-normal-preview", &flat);
    assert_ne!(pixel(&flat, 32, 32), pixel(&result, 32, 32));
    f.model["materials"][0]["pbrMetallicRoughness"]["roughnessFactor"] = json!(0.25);
    f.save();
    let smooth = capture_preview(&f, matrix(), light);
    close(
        pixel(&smooth, 32, 32),
        pbr_reference([0., 0., 1.], metal, 0.25 * 153. / 255., 1., true, true),
    );
    evidence("s2a-smooth-preview", &smooth);
    assert_ne!(pixel(&flat, 32, 32), pixel(&smooth, 32, 32));
    f.model["materials"][0]["alphaMode"] = json!("BLEND");
    f.model["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"][3] = json!(0.5);
    f.save();
    let blend = capture_preview(&f, matrix(), light);
    close(
        pixel(&blend, 32, 32),
        pbr_reference([0., 0., 1.], metal, 0.25 * 153. / 255., 0.5, true, true),
    );
    evidence("s2a-blend-preview", &blend);
    f.model["materials"][0]["alphaMode"] = json!("MASK");
    f.model["materials"][0]["alphaCutoff"] = json!(0.51);
    f.save();
    close(
        pixel(&capture_preview(&f, matrix(), light), 32, 32),
        [0, 0, 0, 255],
    );
    f.model["materials"][0]["alphaCutoff"] = json!(0.5);
    f.save();
    let keep = capture_preview(&f, matrix(), light);
    let mut expected = pbr_reference([0., 0., 1.], metal, 0.25 * 153. / 255., 1., true, true);
    expected[3] = 128;
    close(pixel(&keep, 32, 32), expected);
}

#[test]
#[ignore = "S2-D BC3 mip and encoded alpha GPU reference"]
fn s2d_bc3_gpu_minification_filters_color_in_linear_light_and_keeps_alpha() {
    let mut f = ModelFixture::new();
    f.descriptor["textureCompression"] = json!("bc3");
    f.model["samplers"][0]["minFilter"] = json!(9984);
    let mut bytes = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            bytes.extend_from_slice(&[if (x + y) % 2 == 0 { 255 } else { 0 }, 0, 0, 128]);
        }
    }
    let file = fs::File::create(f.project.root.join("Models/direction.png")).unwrap();
    let mut encoder = png::Encoder::new(file, 8, 8);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&bytes)
        .unwrap();
    f.save();
    let small = capture_extent(&f, matrix(), None, [64, 64], 64.);
    // Independent linear-light average of black/red, not 0.5 sRGB decoded as 0.214.
    close(pixel(&small, 32, 32), [128, 0, 0, 255]);
    f.model["materials"][0]["alphaMode"] = json!("BLEND");
    f.save();
    let blend = capture(&f, matrix());
    // Top-left texel has encoded alpha 128, color is red at this sample.
    close(pixel(&blend, 22, 22), [128, 0, 0, 255]);
}

#[test]
#[ignore = "S2-A ordinary project visible material comparison"]
fn s2a_gpu_ordinary_project_material_sphere_comparison() {
    let mut f = s2a_fixture();
    // Six material assets on one ordinary imported mesh; no backend-authored geometry.
    f.model["materials"] = json!([]);
    f.model["bufferViews"] = json!([]);
    f.model["accessors"] = json!([]);
    f.model["meshes"][0]["primitives"] = json!([]);
    let mut bytes = Vec::new();
    for index in 0..6 {
        let cx = (index % 3) as f32 * 2.3 - 2.3;
        let cy = if index < 3 { 1.05 } else { -1.05 };
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        for y in 0..=12 {
            for x in 0..=24 {
                let theta = y as f32 / 12. * std::f32::consts::PI;
                let phi = x as f32 / 24. * 2. * std::f32::consts::PI;
                let n = [
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                ];
                positions.extend([cx + n[0] * 0.85, cy + n[1] * 0.85, n[2] * 0.85]);
                normals.extend(n);
                uvs.extend([x as f32 / 24., y as f32 / 12.]);
            }
        }
        for y in 0..12u16 {
            for x in 0..24u16 {
                let a = y * 25 + x;
                let b = a + 25;
                indices.extend([a, a + 1, b, a + 1, b + 1, b]);
            }
        }
        let mut ids = Vec::new();
        for (values, components, kind) in [
            (&positions, 3, "VEC3"),
            (&normals, 3, "VEC3"),
            (&uvs, 2, "VEC2"),
        ] {
            let view = f.model["bufferViews"].as_array().unwrap().len();
            let id = f.model["accessors"].as_array().unwrap().len();
            let offset = bytes.len();
            bytes.extend(values.iter().flat_map(|v| v.to_le_bytes()));
            f.model["bufferViews"]
                .as_array_mut()
                .unwrap()
                .push(json!({"buffer":0,"byteOffset":offset,"byteLength":bytes.len()-offset}));
            let mut accessor = json!({"bufferView":view,"componentType":5126,"count":values.len()/components,"type":kind});
            if ids.is_empty() {
                accessor["min"] = json!([cx - 0.85, cy - 0.85, -0.85]);
                accessor["max"] = json!([cx + 0.85, cy + 0.85, 0.85]);
            }
            f.model["accessors"].as_array_mut().unwrap().push(accessor);
            ids.push(id);
        }
        let view = f.model["bufferViews"].as_array().unwrap().len();
        let id = f.model["accessors"].as_array().unwrap().len();
        let offset = bytes.len();
        bytes.extend(indices.iter().flat_map(|v| v.to_le_bytes()));
        f.model["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(json!({"buffer":0,"byteOffset":offset,"byteLength":bytes.len()-offset}));
        f.model["accessors"].as_array_mut().unwrap().push(
            json!({"bufferView":view,"componentType":5123,"count":indices.len(),"type":"SCALAR"}),
        );
        f.model["meshes"][0]["primitives"].as_array_mut().unwrap().push(json!({"attributes":{"POSITION":ids[0],"NORMAL":ids[1],"TEXCOORD_0":ids[2]},"indices":id,"material":index}));
        let (metal, rough) = [
            (0., 0.85),
            (0., 0.2),
            (1., 0.8),
            (1., 0.25),
            (0.4, 0.5),
            (0., 0.5),
        ][index];
        let mut mat = json!({"name":format!("S2-A-{index}"),"doubleSided":true,"pbrMetallicRoughness":{"baseColorFactor":[0.8,0.35,0.07,1.],"metallicFactor":metal,"roughnessFactor":rough}});
        if index == 4 {
            mat["normalTexture"] = json!({"index":1,"scale":0.35});
            mat["pbrMetallicRoughness"]["metallicRoughnessTexture"] = json!({"index":2});
            mat["pbrMetallicRoughness"]["baseColorTexture"] = json!({"index":0});
            mat["occlusionTexture"] = json!({"index":3});
            mat["emissiveFactor"] = json!([0.03, 0.08, 0.12]);
            mat["emissiveTexture"] = json!({"index":4});
        }
        if index == 5 {
            mat["emissiveFactor"] = json!([0.02, 0.15, 0.45]);
        }
        f.model["materials"].as_array_mut().unwrap().push(mat);
    }
    f.model["buffers"][0]["byteLength"] = json!(bytes.len());
    fs::write(f.project.root.join("Models/quad.bin"), bytes).unwrap();
    f.save();
    let mut source = scene();
    source["name"] = json!("S2-A material comparison - preview illumination required");
    source["entities"][0]["components"][0]["data"]["translation"] = json!([0, 0, -6]);
    source["entities"][0]["components"][2]["data"]["mesh"] = json!({"id":"import.mesh.0","type":"mesh","guid":"guid-import-mesh-0","sub_asset":"stable-mesh-0"});
    source["entities"][0]["components"][2]["data"]["materials"] = json!({});
    for i in 0..6 {
        source["entities"][0]["components"][2]["data"]["materials"][format!("surface{i}")] = json!({"id":format!("import.material.{i}"),"type":"material","guid":format!("guid-import-material-{i}"),"sub_asset":format!("stable-material-{i}")});
    }
    source["entities"][1]["components"][0]["data"]["translation"] = json!([0, 0, 0]);
    source["entities"][1]["components"][1]["data"]["projection"] =
        json!({"kind":"orthographic","size":5.,"near":0.1,"far":20.});
    f.project.write("Scenes/main.json", &source);
    let mut m = matrix();
    m[14] = -6.;
    let pixels = capture_extent(
        &f,
        m,
        Some(([-0.4, 0.6, 0.8], [3.; 3], [0.12; 3])),
        [800, 480],
        5.,
    );
    assert!(
        pixels
            .chunks_exact(4)
            .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
            .count()
            > 40000
    );
    if let Some(root) = std::env::var_os("AIFE_A22_EVIDENCE_ROOT") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        let file = fs::File::create(root.join("s2a-sphere-comparison.png")).unwrap();
        let mut encoder = png::Encoder::new(file, 800, 480);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        let target = root.join("ordinary-project");
        for folder in ["Models", "Scenes", "Assets", "Input"] {
            fs::create_dir_all(target.join(folder)).unwrap();
            for entry in fs::read_dir(f.project.root.join(folder)).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_file() {
                    fs::copy(entry.path(), target.join(folder).join(entry.file_name())).unwrap();
                }
            }
        }
        fs::copy(
            f.project.root.join("project.aife.json"),
            target.join("project.aife.json"),
        )
        .unwrap();
    }
}
