//! Generated fixtures are byte-matched against real Compiler output in three_d_project_a.
use super::*;
use engine_runtime::render_asset_production::PreparedMeshRenderer;
use engine_runtime::render_graph::*;
use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;
use engine_runtime::runtime_asset_loader::RuntimeAssetLoader;
use engine_runtime::runtime_texture::{TextureFilter, TextureSampler, TextureWrap};
use engine_runtime::wgpu_backend::real::RealWgpuBackend;
use std::collections::BTreeMap;

const CUBE: &str = include_str!("../fixtures/three_d_a12/cube.json");
const PLANE: &str = include_str!("../fixtures/three_d_a12/plane.json");
const WIDE: &str = include_str!("../fixtures/three_d_a12/wide.json");
const BLUE: &str = include_str!("../fixtures/three_d_a12/blue.json");
const TEXTURED: &str = include_str!("../fixtures/three_d_a12/textured.json");
const DIRECTION: [u8; 16] = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
];

fn material(asset: &mut CookedRenderAsset) -> &mut CookedMaterial {
    let RenderAssetData::Material(m) = &mut asset.asset else {
        panic!("material")
    };
    m
}
fn geometry(asset: &mut CookedRenderAsset) -> &mut CookedGeometry {
    let RenderAssetData::Mesh(m) = &mut asset.asset else {
        panic!("mesh")
    };
    m
}
struct PackageFixture {
    root: PathBuf,
    loader: RuntimeAssetLoader,
    manager: RenderResourceManager,
}
impl PackageFixture {
    fn new(edit: impl FnOnce(&mut BTreeMap<String, CookedRenderAsset>, &mut Vec<u8>)) -> Self {
        let mut assets: BTreeMap<String, _> = [
            ("cube", CUBE),
            ("plane", PLANE),
            ("wide", WIDE),
            ("blue", BLUE),
            ("textured", TEXTURED),
        ]
        .into_iter()
        .map(|(id, text)| {
            (
                id.into(),
                RenderContract::<CookedRenderAsset>::decode(text)
                    .unwrap()
                    .data,
            )
        })
        .collect();
        let mut pixels = DIRECTION.to_vec();
        edit(&mut assets, &mut pixels);
        let root = std::env::temp_dir().join(format!(
            "aife-359-a12-gpu-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
            "generated-gpu",
            "Generated",
            "1",
        ));
        input.scenes.push(RuntimeScene {
            schema_version: RUNTIME_SCENE_SCHEMA_VERSION.into(),
            id: "main".into(),
            name: "Main".into(),
            gravity: 0.,
            background: "#000000".into(),
            sky_color: "#000000".into(),
            entities: vec![],
            main_camera: None,
        });
        let mapping = engine_runtime::input_mapping::InputMappingAsset::gameplay_default();
        input.input_mappings.push(RuntimePackageSourceJson {
            id: mapping.asset_id.clone(),
            document: serde_json::to_value(mapping).unwrap(),
        });
        for (id, asset) in assets {
            let kind = asset.asset.asset_kind().as_str();
            let mut source = RuntimePackageSourceAsset::new(
                &id,
                &id,
                kind,
                format!("Assets/{id}.json"),
                format!("cooked/{id}.json"),
            );
            source.asset_guid = Some(format!("guid-{id}"));
            if let RenderAssetData::Material(m) = &asset.asset {
                source.dependencies = m
                    .textures
                    .values()
                    .map(|r| r.reference.guid.clone().unwrap())
                    .collect();
            }
            let bytes = serde_json::to_vec(&RenderContract::new(asset)).unwrap();
            source.hash = Some(engine_runtime::canonical_digest::sha256_prefixed(&bytes));
            input.assets.push(source.with_runtime_payload(bytes));
        }
        let mut texture = RuntimePackageSourceAsset::new(
            "direction",
            "Direction",
            "texture",
            "Assets/direction.json",
            "cooked/direction.asset",
        );
        texture.asset_guid = Some("guid-direction".into());
        input.assets.push(texture);
        input.texture_payloads.push(RuntimePackageSourceTexture {
            metadata: CookedTextureAsset {
                schema_version: COOKED_TEXTURE_SCHEMA_VERSION.into(),
                asset_id: "direction".into(),
                cooked_asset_id: "direction.cooked".into(),
                source_hash: engine_runtime::canonical_digest::sha256_prefixed(&pixels),
                width: 2,
                height: 2,
                format: "rgba8Unorm".into(),
                color_space: "linear".into(),
                mip_count: 1,
                byte_length: pixels.len(),
                pixel_data_path: "cooked/textures/direction.rgba8".into(),
                sampler: "nearestClamp".into(),
            },
            rgba8: pixels.into(),
        });
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&root, "main"),
            &input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{report:?}"
        );
        let loaded = load_runtime_package(&root);
        assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
        let package = loaded.value.unwrap();
        let mut loader = RuntimeAssetLoader::new(
            &root,
            package.runtime_asset_index,
            package.runtime_asset_mount_table,
        );
        assert!(loader.mount_bundle("startup"));
        Self {
            root,
            loader,
            manager: RenderResourceManager::new(),
        }
    }
    fn prepare(&mut self, mesh: &str, material: &str) -> PreparedMeshRenderer {
        let reference = |id: &str, kind: &str| {
            SpatialAssetRef(RuntimeAssetRef {
                id: id.into(),
                asset_type: kind.into(),
                guid: Some(format!("guid-{id}")),
                sub_asset: None,
            })
        };
        RuntimeRenderAssetProducer::new()
            .prepare_mesh_renderer(
                0,
                &MeshRenderer3D {
                    schema_version: "mesh-renderer3d.v1".into(),
                    mesh: reference(mesh, "mesh"),
                    materials: BTreeMap::from([(
                        "surface".into(),
                        reference(material, "material"),
                    )]),
                    visible: true,
                    layers: vec!["world".into()],
                },
                &mut self.loader,
                &mut self.manager,
            )
            .unwrap()
    }
}
impl Drop for PackageFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn declared_unused_material_resources_share_existing_leases_and_retire() {
    use engine_runtime::render_resource::{
        RenderResourceKind as NativeResourceKind, RenderResourceReleaseRequest, RenderResourceState,
    };
    let mut fixture = PackageFixture::new(|assets, _| {
        geometry(assets.get_mut("cube").unwrap())
            .material_slots
            .push("unused".into());
    });
    let reference = |id: &str, kind: &str| {
        SpatialAssetRef(RuntimeAssetRef {
            id: id.into(),
            asset_type: kind.into(),
            guid: Some(format!("guid-{id}")),
            sub_asset: None,
        })
    };
    let renderer = MeshRenderer3D {
        schema_version: "mesh-renderer3d.v1".into(),
        mesh: reference("cube", "mesh"),
        materials: [
            ("surface".into(), reference("blue", "material")),
            ("unused".into(), reference("textured", "material")),
        ]
        .into(),
        visible: true,
        layers: vec!["world".into()],
    };
    let first = RuntimeRenderAssetProducer::new()
        .prepare_mesh_renderer(1, &renderer, &mut fixture.loader, &mut fixture.manager)
        .unwrap();
    assert!(first
        .geometry
        .iter()
        .all(|geometry| geometry.material_slot() == "surface"));
    let retained: std::collections::BTreeSet<_> =
        first.retained_resources.iter().copied().collect();
    let unused = retained
        .iter()
        .copied()
        .find(|handle| fixture.manager.record(*handle).unwrap().key.asset_id == "guid-textured")
        .expect("declared unused material must retain its actual resource");
    assert!(retained
        .iter()
        .any(|handle| handle.kind == NativeResourceKind::Texture
            && fixture.manager.record(*handle).unwrap().bytes == 16));
    assert_eq!(
        retained
            .iter()
            .map(|handle| fixture.manager.record(*handle).unwrap().bytes)
            .sum::<usize>(),
        fixture.manager.report(1).resident_bytes
    );
    let second = fixture.prepare("plane", "textured");
    assert!(
        second.retained_resources.contains(&unused),
        "another renderer must reuse the same resource"
    );
    let lease = fixture
        .manager
        .lease_resources(second.retained_resources.iter().copied());
    for handle in retained {
        fixture
            .manager
            .release_resource(RenderResourceReleaseRequest {
                handle,
                frame_index: 2,
            })
            .unwrap();
    }
    fixture.manager.end_frame(100);
    assert_eq!(
        fixture.manager.record(unused).unwrap().state,
        RenderResourceState::PendingRelease,
        "existing shared lease must prevent retirement"
    );
    drop(lease);
    for handle in second.retained_resources {
        fixture
            .manager
            .release_resource(RenderResourceReleaseRequest {
                handle,
                frame_index: 101,
            })
            .unwrap();
    }
    let terminal = fixture.manager.end_frame(200);
    assert_eq!(terminal.resident_bytes, 0);
    assert_eq!(terminal.pending_release_count, 0);
    assert_eq!(
        fixture.manager.record(unused).unwrap().state,
        RenderResourceState::Released
    );
}

fn gpu_backend() -> RealWgpuBackend {
    let backend = RealWgpuBackend::new_offscreen(64, 64)
        .expect("A1.2 requires a real GPU adapter; absence must fail");
    eprintln!("A1.2 GPU context: {:?}", backend.device_context());
    backend
}
fn instance(source: &PreparedGeometry, id: u64, matrix: [f32; 16]) -> GeometryInstance<'_> {
    GeometryInstance {
        instance_id: id,
        source,
        world_transform: matrix,
        visibility: Visibility {
            authored: true,
            inherited: Evaluation::Known(true),
        },
        layers: LayerSelection::Named {
            names: vec!["main".into()],
        },
        provenance: Provenance::unspecified(ProducerStage::RenderProjection),
    }
}
fn view() -> RenderView {
    let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 20.);
    view.projection = Projection::Orthographic {
        size: 4.,
        near: 0.1,
        far: 20.,
    };
    view
}
fn frame(
    manager: &RenderResourceManager,
    items: &[GeometryInstance<'_>],
    pose: CameraPose,
) -> GeometryFrame {
    GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view(),
            pose,
        }],
        items,
        manager,
        DiagnosticLevel::Summary,
    )
    .unwrap()
}
fn graph(frame: &GeometryFrame) -> RenderGraph {
    let mut graph = RenderGraph::new("generated", 1);
    graph.output_target = Some("main".into());
    graph
        .resources
        .push(RenderResource::surface_backbuffer("main", 64, 64));
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
    graph
}
fn read(backend: &mut RealWgpuBackend, frame: &GeometryFrame) -> Vec<u8> {
    let plan = compile_render_graph_to_rhi_plan(&graph(frame));
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap()
}
fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
fn close(actual: [u8; 4], expected: [u8; 4]) {
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
        "pixel {actual:?}, expected {expected:?}, tolerance 3/255"
    );
}
fn translated(x: f32, y: f32, z: f32) -> [f32; 16] {
    let mut m = IDENTITY_MATRIX;
    m[12..15].copy_from_slice(&[x, y, z]);
    m
}
// Exact +90 degree X rotation and Z scale 0.5: generated 2 x 4 plane faces +Z as a 2 x 2 square.
fn plane_matrix() -> [f32; 16] {
    [
        1., 0., 0., 0., 0., 0., 1., 0., 0., -0.5, 0., 0., 0., 0., -3., 1.,
    ]
}
fn upload(backend: &mut RealWgpuBackend, bound: &PreparedMeshRenderer) {
    for texture in &bound.textures {
        backend
            .register_cooked_texture(texture.handle.raw(), &texture.payload)
            .unwrap();
    }
}

#[test]
#[ignore = "requires a real GPU, run explicitly for A1.2 qualification"]
fn gpu_generated_size_color_silhouette_and_shared_mesh_uploads() {
    let mut fixture = PackageFixture::new(|assets, _| {
        let mut green = assets["blue"].clone();
        material(&mut green).parameters.insert(
            "baseColor".into(),
            MaterialParameter::Vector4([0., 1., 0., 1.]),
        );
        assets.insert("green".into(), green);
    });
    let cube = fixture.prepare("cube", "blue");
    let green = fixture.prepare("cube", "green");
    let wide = fixture.prepare("wide", "blue");
    let plane = fixture.prepare("plane", "blue");
    let mut backend = gpu_backend();
    let items = [
        instance(&cube.geometry[0], 1, translated(-0.8, 0., -3.)),
        instance(&green.geometry[0], 2, translated(0.8, 0., -3.)),
    ];
    let original = frame(&fixture.manager, &items, CameraPose::default());
    let pixels = read(&mut backend, &original);
    close(pixel(&pixels, 19, 32), [0, 0, 255, 255]);
    close(pixel(&pixels, 45, 32), [0, 255, 0, 255]);
    close(pixel(&pixels, 32, 32), [0, 0, 0, 255]);
    assert_eq!(
        backend.geometry_upload_count(),
        1,
        "instances/materials share actual GPU buffers"
    );
    assert_eq!(pixels, read(&mut backend, &original));
    assert_eq!(backend.geometry_upload_count(), 1);
    let changed = frame(
        &fixture.manager,
        &[instance(&green.geometry[0], 1, translated(-0.8, 0., -3.))],
        CameraPose {
            position: [0.25, 0., 0.],
            ..Default::default()
        },
    );
    close(
        pixel(&read(&mut backend, &changed), 15, 32),
        [0, 255, 0, 255],
    );
    assert_eq!(
        backend.geometry_upload_count(),
        1,
        "color and camera changes do not upload geometry"
    );
    let centered = frame(
        &fixture.manager,
        &[instance(&cube.geometry[0], 1, translated(0., 0., -3.))],
        CameraPose::default(),
    );
    let small = read(&mut backend, &centered);
    close(pixel(&small, 20, 32), [0, 0, 0, 255]);
    close(pixel(&small, 27, 32), [0, 0, 255, 255]);
    let mut rotated = translated(0., 0., -3.);
    let c = std::f32::consts::FRAC_1_SQRT_2;
    rotated[0] = c;
    rotated[2] = -c;
    rotated[8] = c;
    rotated[10] = c;
    let oblique = frame(
        &fixture.manager,
        &[instance(&cube.geometry[0], 1, rotated)],
        CameraPose::default(),
    );
    let pixels = read(&mut backend, &oblique);
    close(pixel(&pixels, 22, 32), [0, 0, 255, 255]);
    close(pixel(&pixels, 18, 32), [0, 0, 0, 255]);
    let enlarged = frame(
        &fixture.manager,
        &[instance(&wide.geometry[0], 1, translated(0., 0., -3.))],
        CameraPose::default(),
    );
    let large = read(&mut backend, &enlarged);
    close(pixel(&large, 20, 32), [0, 0, 255, 255]);
    close(pixel(&large, 32, 20), [0, 0, 0, 255]);
    let horizontal = frame(
        &fixture.manager,
        &[instance(&plane.geometry[0], 1, plane_matrix())],
        CameraPose::default(),
    );
    let flat = read(&mut backend, &horizontal);
    close(pixel(&flat, 19, 19), [0, 0, 255, 255]);
    close(pixel(&flat, 14, 32), [0, 0, 0, 255]);
}

#[test]
#[ignore = "requires a real GPU, run explicitly for A1.2 qualification"]
fn gpu_generated_mirror_front_back_none_and_affine_pipeline_identity() {
    let mut fixture = PackageFixture::new(|assets, _| {
        for (id, cull) in [("front", MaterialCull::Front), ("none", MaterialCull::None)] {
            let mut m = assets["blue"].clone();
            material(&mut m).cull = cull;
            assets.insert(id.into(), m);
        }
    });
    let back = fixture.prepare("plane", "blue");
    let front = fixture.prepare("plane", "front");
    let none = fixture.prepare("plane", "none");
    let mut backend = gpu_backend();
    for mirrored in [false, true] {
        for from_back in [false, true] {
            let mut matrix = plane_matrix();
            if mirrored {
                matrix[0] = -1.;
            }
            if from_back {
                matrix[6] = -1.;
                matrix[9] = 0.5;
            }
            // Nonzero shear exercises the full affine matrix without decomposition.
            matrix[8] = 0.125;
            for (bound, visible) in [(&back, !from_back), (&front, from_back), (&none, true)] {
                let frame = frame(
                    &fixture.manager,
                    &[instance(&bound.geometry[0], 1, matrix)],
                    CameraPose::default(),
                );
                assert_eq!(frame.views[0].draws[0].mirrored(), mirrored);
                close(
                    pixel(&read(&mut backend, &frame), 32, 32),
                    if visible {
                        [0, 0, 255, 255]
                    } else {
                        [0, 0, 0, 255]
                    },
                );
            }
        }
    }
    let normal = frame(
        &fixture.manager,
        &[instance(&back.geometry[0], 1, plane_matrix())],
        CameraPose::default(),
    );
    let mut mirror = plane_matrix();
    mirror[0] = -1.;
    let reflected = frame(
        &fixture.manager,
        &[instance(&back.geometry[0], 1, mirror)],
        CameraPose::default(),
    );
    assert_ne!(
        normal.views[0].draws[0].pipeline_identity("rgba8"),
        reflected.views[0].draws[0].pipeline_identity("rgba8")
    );
    // Two reflected axes preserve orientation, despite two negative diagonal scales.
    mirror[9] = 0.5;
    let double = frame(
        &fixture.manager,
        &[instance(&back.geometry[0], 1, mirror)],
        CameraPose::default(),
    );
    assert!(!double.views[0].draws[0].mirrored());
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
// Independent scalar 2x2 reference sampler; colors are 0/1 so sRGB decode is exact.
fn sample(uv: [f32; 2], filter: TextureFilter, wrap: [TextureWrap; 2]) -> [u8; 4] {
    let texel = |x: i32, y: i32, c: usize| {
        f32::from(DIRECTION[(addressed(y, wrap[1]) * 2 + addressed(x, wrap[0])) * 4 + c])
    };
    match filter {
        TextureFilter::Nearest
        | TextureFilter::NearestMipmapNearest
        | TextureFilter::NearestMipmapLinear => std::array::from_fn(|c| {
            texel((uv[0] * 2.).floor() as i32, (uv[1] * 2.).floor() as i32, c) as u8
        }),
        TextureFilter::Linear
        | TextureFilter::LinearMipmapNearest
        | TextureFilter::LinearMipmapLinear => {
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
    }
}
fn remap_uv(mesh: &mut CookedGeometry, scale: [f32; 2], offset: [f32; 2]) {
    for v in mesh.vertex_bytes.chunks_exact_mut(32) {
        for i in 0..2 {
            let at = 24 + i * 4;
            let uv = f32::from_le_bytes(v[at..at + 4].try_into().unwrap());
            v[at..at + 4].copy_from_slice(&(uv * scale[i] + offset[i]).to_le_bytes());
        }
    }
}

#[test]
#[ignore = "requires a real GPU, run explicitly for A1.2 qualification"]
fn gpu_generated_direction_sampler_min_mag_wrap_variants_and_missing_residency() {
    use TextureFilter::*;
    use TextureWrap::*;
    let mut settings = Vec::new();
    for min_filter in [Nearest, Linear] {
        for mag_filter in [Nearest, Linear] {
            for wrap_s in [Repeat, ClampToEdge, MirroredRepeat] {
                for wrap_t in [Repeat, ClampToEdge, MirroredRepeat] {
                    settings.push(TextureSampler {
                        min_filter,
                        mag_filter,
                        wrap_s,
                        wrap_t,
                    });
                }
            }
        }
    }
    let mut fixture = PackageFixture::new(|assets, _| {
        for (index, sampler) in settings.iter().enumerate() {
            let mut m = assets["textured"].clone();
            material(&mut m).base_color_texture_sampling = Some(*sampler);
            assets.insert(format!("sampler{index}"), m);
        }
        for (id, scale) in [("magnify", [3., 3.]), ("minify", [35., 35.])] {
            let mut mesh = assets["plane"].clone();
            remap_uv(geometry(&mut mesh), scale, [-1., -1.]);
            assets.insert(id.into(), mesh);
        }
    });
    let mut backend = gpu_backend();
    let direction = fixture.prepare("plane", "sampler0");
    upload(&mut backend, &direction);
    let pixels = read(
        &mut backend,
        &frame(
            &fixture.manager,
            &[instance(&direction.geometry[0], 1, plane_matrix())],
            CameraPose::default(),
        ),
    );
    for (x, y, expected) in [
        (23, 23, [255, 0, 0, 255]),
        (40, 23, [0, 255, 0, 255]),
        (23, 40, [0, 0, 255, 255]),
        (40, 40, [255, 255, 255, 255]),
    ] {
        close(pixel(&pixels, x, y), expected);
    }
    let cube = fixture.prepare("cube", "sampler0");
    let pixels = read(
        &mut backend,
        &frame(
            &fixture.manager,
            &[instance(&cube.geometry[0], 1, translated(0., 0., -3.))],
            CameraPose::default(),
        ),
    );
    for (x, y, expected) in [
        (27, 27, [255, 0, 0, 255]),
        (36, 27, [0, 255, 0, 255]),
        (27, 36, [0, 0, 255, 255]),
        (36, 36, [255, 255, 255, 255]),
    ] {
        close(pixel(&pixels, x, y), expected);
    }
    let mut prepared = Vec::new();
    for (index, sampler) in settings.iter().enumerate() {
        for (mesh, scale, filter) in [
            ("magnify", 3., sampler.mag_filter),
            ("minify", 35., sampler.min_filter),
        ] {
            let bound = fixture.prepare(mesh, &format!("sampler{index}"));
            upload(&mut backend, &bound);
            prepared.push((bound, scale, filter, *sampler));
        }
    }
    // Render only after every variant has been prepared: catches false hot-reload invalidation.
    for (bound, scale, filter, sampler) in &prepared {
        let frame = frame(
            &fixture.manager,
            &[instance(&bound.geometry[0], 1, plane_matrix())],
            CameraPose::default(),
        );
        let pixels = read(&mut backend, &frame);
        for x in [18, 25, 34, 44] {
            for y in [19, 27, 36, 45] {
                let uv = [
                    ((x as f32 + 0.5) - 16.) / 32. * scale - 1.,
                    ((y as f32 + 0.5) - 16.) / 32. * scale - 1.,
                ];
                let actual = pixel(&pixels, x, y);
                let expected = sample(uv, *filter, [sampler.wrap_s, sampler.wrap_t]);
                assert!(actual.iter().zip(expected).all(|(a,b)|a.abs_diff(b)<=3),"sampler={sampler:?}, scale={scale}, xy={x},{y}, uv={uv:?}: {actual:?} != {expected:?}");
            }
        }
    }
    let frame = frame(
        &fixture.manager,
        &[instance(&direction.geometry[0], 1, plane_matrix())],
        CameraPose::default(),
    );
    let mut missing = gpu_backend();
    let plan = compile_render_graph_to_rhi_plan(&graph(&frame));
    assert!(missing
        .render_plan_to_rgba_bytes(&plan, 64, 64)
        .unwrap_err()
        .contains("texture"));
    let mut invalid = direction.textures[0].payload.clone();
    std::sync::Arc::make_mut(&mut invalid).sampler = "nearest-but-unknown".into();
    let submission = missing.last_gpu_submission();
    let textures = missing.geometry_residency().textures;
    // Cooked payload validation now rejects this before native sampler creation.
    assert_eq!(
        missing
            .register_cooked_texture(direction.textures[0].handle.raw(), &invalid)
            .unwrap_err(),
        "InvalidAssetPayload at texture.payload (AssetDecode)"
    );
    assert_eq!(missing.geometry_residency().textures, textures);
    assert_eq!(missing.last_gpu_submission(), submission);
}

#[test]
#[ignore = "requires a real GPU, run explicitly for A1.2 qualification"]
fn gpu_generated_base_color_is_linear_texture_is_srgb_and_alpha_is_linear() {
    let mut fixture = PackageFixture::new(|assets, pixels| {
        *pixels = [128, 128, 128, 128].repeat(4);
        material(assets.get_mut("textured").unwrap())
            .parameters
            .insert(
                "baseColor".into(),
                MaterialParameter::Vector4([0.5, 0.25, 1., 0.5]),
            );
        material(assets.get_mut("blue").unwrap()).parameters.insert(
            "baseColor".into(),
            MaterialParameter::Vector4([0.5, 0.25, 0.125, 0.5]),
        );
    });
    let plain = fixture.prepare("plane", "blue");
    let textured = fixture.prepare("plane", "textured");
    let mut backend = gpu_backend();
    upload(&mut backend, &textured);
    let pixels = read(
        &mut backend,
        &frame(
            &fixture.manager,
            &[instance(&plain.geometry[0], 1, plane_matrix())],
            CameraPose::default(),
        ),
    );
    close(pixel(&pixels, 32, 32), [128, 64, 32, 128]);
    let pixels = read(
        &mut backend,
        &frame(
            &fixture.manager,
            &[instance(&textured.geometry[0], 1, plane_matrix())],
            CameraPose::default(),
        ),
    );
    // IEC sRGB: 128/255 -> 0.21586 linear; alpha remains 128/255.
    close(pixel(&pixels, 32, 32), [28, 14, 55, 64]);
}

#[test]
fn a41_geometry_texture_residency_is_reused_and_released_eight_times() {
    let mut fixture = PackageFixture::new(|_, _| {});
    let textured = fixture.prepare("cube", "textured");
    let mut backend = gpu_backend();
    for _ in 0..8 {
        let frame = frame(
            &fixture.manager,
            &[instance(&textured.geometry[0], 1, translated(0., 0., -3.))],
            CameraPose::default(),
        );
        backend
            .prepare_geometry_textures(&textured.textures)
            .unwrap();
        let pixels = read(&mut backend, &frame);
        let count = backend.geometry_upload_count();
        backend
            .prepare_geometry_textures(&textured.textures)
            .unwrap();
        assert_eq!(read(&mut backend, &frame), pixels);
        assert_eq!(backend.geometry_upload_count(), count);
        assert_eq!(backend.geometry_residency().textures, 1);
        assert_eq!(backend.geometry_residency().meshes, 1);
        backend.prepare_geometry_textures(&[]).unwrap();
        let empty = GeometryFrame {
            target: "main".into(),
            extent: [64, 64],
            views: vec![],
        };
        read(&mut backend, &empty);
        let after = backend.geometry_residency();
        assert_eq!(
            (
                after.meshes,
                after.textures,
                after.depth_targets,
                after.pipelines
            ),
            (0, 0, 0, 0)
        );
    }
}

#[test]
#[ignore = "requires real GPU; allocation census does not grant full heap qualification"]
fn gpu_memory_census_tracks_shared_backing_and_does_not_keep_resources_alive() {
    use engine_runtime::resource_memory::{MemoryDomain, ResourceMemorySample};
    let mut fixture = PackageFixture::new(|_, _| {});
    let source = fixture.prepare("cube", "textured");
    let mut backend = gpu_backend();
    upload(&mut backend, &source);
    let geometry = frame(
        &fixture.manager,
        &[
            instance(&source.geometry[0], 1, translated(0., 0., -3.)),
            instance(&source.geometry[0], 2, translated(1., 0., -3.)),
        ],
        CameraPose::default(),
    );
    read(&mut backend, &geometry);
    let mut sample = ResourceMemorySample::new(1, backend.gpu_lifecycle_stats().device_epoch, 0);
    fixture.loader.observe_memory(&mut sample);
    backend.observe_memory(&mut sample);
    let mut unique = BTreeMap::new();
    for a in sample
        .allocations
        .iter()
        .filter(|a| a.domain == MemoryDomain::Gpu)
    {
        if let Some(old) = unique.insert(a.allocation_id.clone(), a.bytes) {
            assert_eq!(old, a.bytes);
        }
    }
    let total: u64 = unique.values().sum();
    assert!(
        total
            >= backend.geometry_residency().physical_mesh_bytes
                + backend.retained_geometry_stats().resident_parameter_bytes
                + 64 * 64 * 4
    );
    assert!(sample
        .allocations
        .iter()
        .any(|a| a.domain == MemoryDomain::Cpu && a.owner == "assets"));
    assert_eq!(backend.geometry_residency().meshes, 1);
    let empty = GeometryFrame {
        target: "main".into(),
        extent: [64, 64],
        views: vec![],
    };
    backend.prepare_geometry_textures(&[]).unwrap();
    read(&mut backend, &empty);
    assert_eq!(backend.geometry_residency().meshes, 0);
    // Keep the frozen report alive through actual release; it contains no native handles.
    assert!(!sample.allocations.is_empty());
    assert_eq!(backend.geometry_residency().textures, 0);
    let mut after = ResourceMemorySample::new(2, sample.device_epoch, 1);
    backend.observe_memory(&mut after);
    assert!(!after
        .allocations
        .iter()
        .any(|a| a.domain == MemoryDomain::Cpu && a.owner == "assets"));
}
