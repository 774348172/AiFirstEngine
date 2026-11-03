#![cfg(feature = "real-wgpu")]
//! Real official pose -> existing Projection -> neutral Prepare/graph -> native GPU.
extern crate engine_runtime_core as engine_runtime;
use engine_render_native::wgpu_backend::real::RealWgpuBackend;
use engine_runtime::render_asset_production::*;
use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_graph::{RenderGraph, RenderPassCommand, RenderResource};
use engine_runtime::render_projection_adapter::{spine::*, RenderProjectionAdapter};
use engine_runtime::render_resource::*;
use engine_runtime::runtime_texture::RuntimeTexturePayload;
use engine_runtime::spine_animation::SpinePage;
use engine_spine_runtime::{
    import::{validate, ValidatedSource},
    BlendMode, Geometry,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::PathBuf};
const W: u32 = 1280;
const H: u32 = 720;
const SIZE: f32 = 8.0;
const BG: [f32; 4] = [0.12, 0.19, 0.08, 0.4];
fn geometry_graph(frame: &GeometryFrame) -> RenderGraph {
    let mut graph = RenderGraph::new("u2", 1);
    graph.output_target = Some(frame.target.clone());
    graph.resources.push(RenderResource::surface_backbuffer(
        &frame.target,
        frame.extent[0],
        frame.extent[1],
    ));
    graph.passes.push(engine_runtime::render_graph::RenderPass {
        pass_id: "clear".into(),
        pass_name: "clear".into(),
        pass_kind: engine_runtime::render_graph::RenderPassKind::Clear,
        view_id: "global".into(),
        reads: vec![],
        writes: vec![frame.target.clone()],
        color_targets: vec![frame.target.clone()],
        depth_target: None,
        commands: vec![RenderPassCommand::Clear {
            target: frame.target.clone(),
            color: engine_runtime::render_graph::color([0., 0., 0., 1.]),
        }]
        .into(),
        debug_source: None,
    });
    frame.append_to_graph(&mut graph);
    graph
}
fn prepare_geometry(
    id: &str,
    asset: RenderAssetData,
    manager: &mut RenderResourceManager,
) -> PreparedCookedRenderAsset {
    asset.validate().unwrap();
    let (kind, binding_kind, source, bytes) = match &asset {
        RenderAssetData::Mesh(m) => (
            RenderResourceKind::MeshBuffer,
            RenderBindingKind::Mesh,
            RenderResourceSource::MeshBufferDescriptor {
                vertex_count: m.vertex_count,
                index_count: m.index_count,
            },
            m.vertex_bytes.len() + m.index_bytes.len(),
        ),
        RenderAssetData::Material(m) => (
            RenderResourceKind::MaterialParams,
            RenderBindingKind::Material,
            RenderResourceSource::MaterialParamsDescriptor {
                param_count: m.parameters.len() as u32,
            },
            16,
        ),
    };
    let request = RenderResourceRequest {
        key: RenderAssetKey::new(id, "1", kind),
        source,
        byte_len: bytes,
        reason: "u2-test".into(),
    };
    let handle = manager.request_resource(0, request.clone());
    let cooked = CookedRenderAsset {
        source_digest: format!("sha256:{}", "1".repeat(64)),
        recipe: NATIVE_RENDER_COOK_RECIPE.into(),
        asset,
    };
    let cooked_digest = engine_runtime::canonical_digest::sha256_prefixed(
        &serde_json::to_vec(&RenderContract::new(cooked.clone())).unwrap(),
    );
    PreparedCookedRenderAsset {
        asset: cooked,
        binding: RenderBindingSet::single(
            id.into(),
            binding_kind,
            handle,
            "nearestClamp".into(),
            false,
            id.into(),
        )
        .unwrap(),
        state: RenderAssetPreparationState::Prepared,
        textures: vec![],
        resource_request: request,
        asset_guid: id.into(),
        cooked_digest,
        source_path: Some(format!("assets/{id}.asset")),
        pipeline_identity: None,
    }
}
const ATLAS: &str = include_str!("../../engine_spine_runtime/tests/fixtures/probe.atlas");

fn region(blend: &str) -> Value {
    json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"}],
        "slots":[{"name":"image","bone":"root","attachment":"image","blend":blend,"color":"c080e0a0"}],
        "skins":[{"name":"default","attachments":{"image":{"image":{"type":"region","path":"triangle","width":2,"height":2,"color":"80c0ffb0"}}}}]})
}
fn checked(v: &Value, atlas: &str) -> ValidatedSource {
    let refs = BTreeMap::from([
        ("probe.png".into(), "t0".into()),
        ("second.png".into(), "t1".into()),
    ]);
    // Only atlas-declared pages belong to its closure.
    let refs = refs
        .into_iter()
        .filter(|(name, _)| atlas.lines().any(|l| l == name))
        .collect();
    validate(&v.to_string(), atlas, &refs).unwrap()
}
fn pages(source: &ValidatedSource) -> Vec<SpinePage> {
    source
        .pages
        .iter()
        .map(|p| SpinePage {
            name: p.name.clone(),
            texture_asset_id: p.texture_asset_id.clone(),
            pma: p.pma,
        })
        .collect()
}
fn payloads(source: &ValidatedSource, pixels: &[Vec<u8>]) -> Vec<RuntimeTexturePayload> {
    source
        .pages
        .iter()
        .zip(pixels)
        .map(|(p, rgba)| RuntimeTexturePayload {
            asset_id: p.texture_asset_id.clone(),
            cooked_asset_id: format!("cooked:{}", p.texture_asset_id),
            width: p.width,
            height: p.height,
            format: "rgba8Unorm".into(),
            color_space: "linear".into(),
            mip_count: 1,
            rgba8: p
                .linear_premultiplied_rgba8(p.width, p.height, rgba)
                .unwrap()
                .into(),
            sampler: "nearestClamp".into(),
            source_hash: engine_runtime::canonical_digest::sha256_prefixed(rgba),
        })
        .collect()
}
fn prepare_frame(
    g: &Geometry,
    pages: &[SpinePage],
    textures: &[RuntimeTexturePayload],
    transform: [f32; 16],
    manager: &mut RenderResourceManager,
) -> (GeometryFrame, Vec<PreparedTextureAsset>) {
    let projected = SpineRendererAdapter
        .project(
            &"spine-pixel".into(),
            &SpineGeometrySource { geometry: g, pages },
        )
        .unwrap();
    assert_eq!(
        projected.iter().map(|b| b.draw_order).collect::<Vec<_>>(),
        (0..g.batches.len() as u32).collect::<Vec<_>>()
    );
    let producer = RuntimeRenderAssetProducer::new();
    let textures = textures
        .iter()
        .map(|p| {
            producer
                .prepare_texture_payload(1, p.clone(), RenderBindingKind::Texture, manager)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut prepared = Vec::new();
    for (n, batch) in projected.into_iter().enumerate() {
        let identity = engine_runtime::canonical_digest::sha256_prefixed(
            &serde_json::to_vec(&batch.geometry).unwrap(),
        );
        let mesh = prepare_geometry(
            &format!("mesh-{n}-{identity}"),
            RenderAssetData::Mesh(batch.geometry),
            manager,
        );
        let mut material = prepare_geometry(
            &format!("mat-{n}"),
            RenderAssetData::Material(batch.material),
            manager,
        );
        let texture = textures
            .iter()
            .find(|t| t.payload.asset_id == batch.texture_asset_id)
            .unwrap();
        material.binding.slots.push(NamedRenderBinding {
            name: "baseColorTexture".into(),
            resource: BindingResource::Texture(texture.handle),
        });
        material.binding.sampler = texture.payload.sampler.clone();
        material.textures.push(texture.clone());
        prepared.push(PreparedGeometry::from_assets(&mesh, &material, 0, manager).unwrap());
    }
    let instances = prepared
        .iter()
        .enumerate()
        .map(|(n, source)| GeometryInstance {
            instance_id: n as u64 + 1,
            source,
            world_transform: transform,
            visibility: Visibility {
                authored: true,
                inherited: Evaluation::Known(true),
            },
            layers: LayerSelection::Named {
                names: vec!["main".into()],
            },
            provenance: Provenance::unspecified(ProducerStage::RenderProjection),
        })
        .collect::<Vec<_>>();
    let mut view = RenderView::orthographic(1, W, H, SIZE, -1., 1.);
    view.layers = LayerSelection::Named {
        names: vec!["main".into()],
    };
    view.clear_policy = ClearPolicy::Color { color: BG };
    let frame = GeometryFrame::build(
        "main",
        [W, H],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &instances,
        manager,
        DiagnosticLevel::Summary,
    )
    .unwrap();
    assert_eq!(frame.views[0].draws.len(), g.batches.len());
    assert_eq!(
        frame.views[0]
            .draws
            .iter()
            .map(|d| d.instance_id)
            .collect::<Vec<_>>(),
        (1..=g.batches.len() as u64).collect::<Vec<_>>(),
        "transparent segments must retain order"
    );
    (frame, textures)
}
fn render(
    g: &Geometry,
    pages: &[SpinePage],
    textures: &[RuntimeTexturePayload],
    transform: [f32; 16],
) -> Vec<u8> {
    let mut manager = RenderResourceManager::new();
    let (frame, textures) = prepare_frame(g, pages, textures, transform, &mut manager);
    let plan =
        engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&geometry_graph(&frame));
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let mut backend =
        RealWgpuBackend::new_offscreen(W, H).expect("Gate B requires real GPU; no skipped pass");
    backend.prepare_geometry_textures(&textures).unwrap();
    let rgba = backend.render_plan_to_rgba_bytes(&plan, W, H).unwrap();
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    rgba
}

#[path = "spine374_pixels/gate_f.rs"]
mod gate_f;
fn byte(v: f32) -> u8 {
    (v.clamp(0., 1.) * 255.).round() as u8
}
fn blend(src: [f32; 4], dst: [f32; 4], mode: BlendMode) -> [f32; 4] {
    let mut out = [0.; 4];
    for c in 0..3 {
        out[c] = match mode {
            BlendMode::Normal => src[c] + dst[c] * (1. - src[3]),
            BlendMode::Additive => src[c] + dst[c],
            BlendMode::Multiply => src[c] * dst[c] + dst[c] * (1. - src[3]),
            BlendMode::Screen => src[c] + dst[c] * (1. - src[c]),
        };
    }
    out[3] = src[3] + dst[3] * (1. - src[3]);
    out
}
fn screen(x: f32, y: f32, m: [f32; 16]) -> [f32; 2] {
    let wx = m[0] * x + m[4] * y + m[12];
    let wy = m[1] * x + m[5] * y + m[13];
    [
        W as f32 * 0.5 + wx * H as f32 / SIZE,
        H as f32 * 0.5 - wy * H as f32 / SIZE,
    ]
}
fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}
// Independent nearest-sampling raster reference. Exclude a frozen one-pixel edge
// mask, retaining all exterior and interior pixels for the <=2/255 assertion.
fn reference(
    g: &Geometry,
    textures: &[RuntimeTexturePayload],
    transform: [f32; 16],
) -> (Vec<u8>, Vec<bool>) {
    let mut rgba = BG.map(byte).repeat((W * H) as usize);
    let mut edges = vec![false; (W * H) as usize];
    for batch in &g.batches {
        let tex = &textures[batch.page as usize];
        let mut covered = vec![false; (W * H) as usize];
        for triangle in g.indices
            [batch.first_index as usize..(batch.first_index + batch.index_count) as usize]
            .chunks_exact(3)
        {
            let indices: [u32; 3] = triangle.try_into().unwrap();
            let v = indices.map(|i| g.vertices[i as usize]);
            let q = v.map(|v| screen(v.x, v.y, transform));
            let area = edge(q[0], q[1], q[2]);
            let minx = q
                .iter()
                .map(|p| p[0])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.) as u32;
            let maxx = q
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(W as f32) as u32;
            let miny = q
                .iter()
                .map(|p| p[1])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.) as u32;
            let maxy = q
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(H as f32) as u32;
            for y in miny.saturating_sub(1)..(maxy + 1).min(H) {
                for x in minx.saturating_sub(1)..(maxx + 1).min(W) {
                    let p = [x as f32 + 0.5, y as f32 + 0.5];
                    let at = (y * W + x) as usize;
                    let e = [
                        edge(q[1], q[2], p),
                        edge(q[2], q[0], p),
                        edge(q[0], q[1], p),
                    ];
                    for n in 0..3 {
                        let a = q[(n + 1) % 3];
                        let b = q[(n + 2) % 3];
                        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                        if len > 0. && e[n].abs() / len <= 1.0 {
                            edges[at] = true;
                        }
                    }
                    let weights = e.map(|e| e / area);
                    if covered[at] || weights.iter().any(|w| *w < -1e-6) {
                        continue;
                    }
                    covered[at] = true;
                    let interpolate = |f: fn(engine_spine_runtime::Vertex) -> f32| -> f32 {
                        (0..3).map(|n| f(v[n]) * weights[n]).sum()
                    };
                    let u = interpolate(|v| v.u);
                    let vv = interpolate(|v| v.v);
                    // Nearest-filter texel boundaries are image edges too. Freeze
                    // their one-screen-pixel mask analytically from affine UV slope.
                    for (coord, extent, get) in [
                        (
                            u,
                            tex.width,
                            (|v: engine_spine_runtime::Vertex| v.u) as fn(_) -> _,
                        ),
                        (
                            vv,
                            tex.height,
                            (|v: engine_spine_runtime::Vertex| v.v) as fn(_) -> _,
                        ),
                    ] {
                        let sample = |p| {
                            let w = [
                                edge(q[1], q[2], p) / area,
                                edge(q[2], q[0], p) / area,
                                edge(q[0], q[1], p) / area,
                            ];
                            (0..3).map(|n| get(v[n]) * w[n]).sum::<f32>()
                        };
                        let slope = ((sample([p[0] + 1., p[1]]) - coord).powi(2)
                            + (sample([p[0], p[1] + 1.]) - coord).powi(2))
                        .sqrt();
                        let line = (coord * extent as f32).round();
                        if line > 0.
                            && line < extent as f32
                            && slope > 0.
                            && (coord - line / extent as f32).abs() / slope <= 1.0
                        {
                            edges[at] = true;
                        }
                    }
                    let tx = (u * tex.width as f32)
                        .floor()
                        .clamp(0., tex.width as f32 - 1.) as usize;
                    let ty = (vv * tex.height as f32)
                        .floor()
                        .clamp(0., tex.height as f32 - 1.) as usize;
                    let offset = (ty * tex.width as usize + tx) * 4;
                    let tint = [
                        interpolate(|v| v.r),
                        interpolate(|v| v.g),
                        interpolate(|v| v.b),
                        interpolate(|v| v.a),
                    ];
                    let mut src = [0.; 4];
                    for c in 0..3 {
                        src[c] = tex.rgba8[offset + c] as f32 / 255. * tint[c] * tint[3];
                    }
                    src[3] = tex.rgba8[offset + 3] as f32 / 255. * tint[3];
                    let dst = std::array::from_fn(|c| rgba[at * 4 + c] as f32 / 255.);
                    rgba[at * 4..at * 4 + 4]
                        .copy_from_slice(&blend(src, dst, batch.blend).map(byte));
                }
            }
        }
    }
    (rgba, edges)
}
fn evidence() -> PathBuf {
    std::env::var_os("SPINE374_EVIDENCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/374/pixels")
        })
}

#[test]
fn spine374_b_projection_capacity_and_unqualified_material_combinations_fail_closed() {
    let source = checked(&region("normal"), ATLAS);
    let pages = pages(&source);
    let g = source
        .load()
        .unwrap()
        .instance()
        .unwrap()
        .geometry(1.)
        .unwrap();
    for case in ["index", "page", "range", "bounds"] {
        let mut bad = g.clone();
        match case {
            "index" => bad.indices[0] = u32::MAX,
            "page" => bad.batches[0].page = 9,
            "range" => bad.batches[0].first_index = 1,
            _ => bad.bounds = Some([1., 1., 0., 0.]),
        }
        assert!(
            SpineRendererAdapter
                .project(
                    &"invalid".into(),
                    &SpineGeometrySource {
                        geometry: &bad,
                        pages: &pages
                    }
                )
                .is_err(),
            "admitted {case}"
        );
    }
    let mut projected = SpineRendererAdapter
        .project(
            &"valid".into(),
            &SpineGeometrySource {
                geometry: &g,
                pages: &pages,
            },
        )
        .unwrap();
    let batch = projected.remove(0);
    for depth in [MaterialDepth::TestOnly, MaterialDepth::TestAndWrite] {
        let mut bad = batch.material.clone();
        bad.depth = depth;
        assert!(bad.validate().is_err());
    }
    let mut manager = RenderResourceManager::new();
    let mesh = prepare_geometry(
        "policy-mesh",
        RenderAssetData::Mesh(batch.geometry),
        &mut manager,
    );
    let mut material = batch.material;
    material.textures.clear();
    let material = prepare_geometry(
        "policy-material",
        RenderAssetData::Material(material),
        &mut manager,
    );
    let prepared = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
    let view = RenderView::orthographic(1, W, H, SIZE, -1., 1.);
    let instance = GeometryInstance {
        instance_id: 1,
        source: &prepared,
        world_transform: IDENTITY_MATRIX,
        visibility: Visibility {
            authored: true,
            inherited: Evaluation::Known(true),
        },
        layers: view.layers.clone(),
        provenance: Provenance::unspecified(ProducerStage::RenderProjection),
    };
    let frame = GeometryFrame::build(
        "main",
        [W, H],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &[instance],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let mut draw = frame.views[0].draws[0].as_ref().clone();
    std::sync::Arc::make_mut(&mut draw.view).color_pipeline = Some(Default::default());
    assert_eq!(
        draw.validate().unwrap_err().field,
        "drawGeometry.premultipliedPolicy"
    );
}
fn png(path: PathBuf, rgba: &[u8]) {
    let file = fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(file, W, H);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(rgba).unwrap();
}
fn verify(
    name: &str,
    g: &Geometry,
    textures: &[RuntimeTexturePayload],
    pages: &[SpinePage],
    transform: [f32; 16],
) -> Vec<u8> {
    let actual = render(g, pages, textures, transform);
    let (expected, edges) = reference(g, textures, transform);
    let root = evidence().join(name);
    fs::create_dir_all(&root).unwrap();
    let mut bad = 0;
    let mut edge_bad = 0;
    let mut max_delta = 0u8;
    let mut diff = vec![0; actual.len()];
    for (at, (a, b)) in actual
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .enumerate()
    {
        let delta = (0..4).map(|c| a[c].abs_diff(b[c])).max().unwrap();
        if edges[at] {
            if delta > 2 {
                edge_bad += 1;
            }
        } else {
            max_delta = max_delta.max(delta);
            if delta > 2 {
                bad += 1;
            }
        }
        diff[at * 4..at * 4 + 4].copy_from_slice(&[delta, if edges[at] { 64 } else { 0 }, 0, 255]);
    }
    png(root.join("actual.png"), &actual);
    png(root.join("golden.png"), &expected);
    png(root.join("diff.png"), &diff);
    let mask = edges
        .iter()
        .flat_map(|edge| {
            if *edge {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect::<Vec<_>>();
    png(root.join("edge-mask.png"), &mask);
    let report = json!({"width":W,"height":H,"nonEdgeTolerance":2,"edgeMaskPixels":edges.iter().filter(|e|**e).count(),
        "nonEdgeOutliers":bad,"maxNonEdgeDelta":max_delta,"edgeOutliers":edge_bad,"outlierFraction":(bad+edge_bad) as f64/(W*H) as f64});
    fs::write(
        root.join("pixels.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(bad, 0, "{name}: {report}");
    assert!(
        (bad + edge_bad) as f64 / (W * H) as f64 <= 0.001,
        "{name}: {report}"
    );
    actual
}

#[test]
#[ignore = "requires a real GPU"]
fn spine374_b_gpu_four_blends_tint_straight_and_pma() {
    for mode in ["normal", "additive", "multiply", "screen"] {
        let v = region(mode);
        let straight = checked(&v, ATLAS);
        let pma = checked(&v, &ATLAS.replace("pma: false", "pma: true"));
        // Exactly representable sRGB straight/PMA pairs; includes half alpha.
        let texture = payloads(&straight, &[[200, 100, 50, 128].repeat(4)]);
        let premultiplied = payloads(&pma, &[[100, 50, 25, 128].repeat(4)]);
        let g = straight
            .load()
            .unwrap()
            .instance()
            .unwrap()
            .geometry(1.)
            .unwrap();
        let a = verify(
            &format!("{mode}-straight"),
            &g,
            &texture,
            &pages(&straight),
            IDENTITY_MATRIX,
        );
        let b = verify(
            &format!("{mode}-pma"),
            &g,
            &premultiplied,
            &pages(&pma),
            IDENTITY_MATRIX,
        );
        assert!(
            a.iter().zip(&b).all(|(a, b)| a.abs_diff(*b) <= 2),
            "straight/PMA differs: {mode}"
        );
    }
}

#[test]
#[ignore = "requires a real GPU"]
fn spine374_b_gpu_rotated_atlas_multiple_pages_attachment_skin_and_transform() {
    let mut v = region("normal");
    v["slots"][0]["color"] = json!("ffffffff");
    v["skins"][0]["attachments"]["image"]["image"]["color"] = json!("ffffffff");
    v["skins"][0]["attachments"]["image"]["alternate"] =
        json!({"type":"region","path":"second","width":2,"height":2});
    let mut wide = v["skins"][0].clone();
    wide["name"] = json!("wide");
    wide["attachments"]["image"]["image"]["width"] = json!(4);
    v["skins"].as_array_mut().unwrap().push(wide);
    v["animations"] = json!({"switch":{"slots":{"image":{"attachment":[{"name":"alternate"}]}}}});
    let atlas = format!(
        "{}\n{}",
        ATLAS.replace("rotate: false", "rotate: 90"),
        ATLAS
            .replace("probe.png", "second.png")
            .replace("triangle\n", "second\n")
    );
    let source = checked(&v, &atlas);
    let textures = payloads(
        &source,
        &[
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            ],
            [255, 0, 255, 255].repeat(4),
        ],
    );
    let asset = source.load().unwrap();
    let mut i = asset.instance().unwrap();
    let g = i.geometry(2.).unwrap();
    let mut transform = IDENTITY_MATRIX;
    transform[0] = 1.5;
    transform[5] = 0.75;
    transform[12] = 1.25;
    transform[13] = -0.5;
    let a = verify(
        "atlas90-ppu2-transform",
        &g,
        &textures,
        &pages(&source),
        transform,
    );
    // Independent world point selects an interior quadrant; PPU and TRS each once.
    let center = screen(0., 0., transform);
    assert_ne!(
        &a[((center[1] as u32 * W + center[0] as u32) * 4) as usize..][..3],
        &BG.map(byte)[..3]
    );
    i.set_skin("wide").unwrap();
    let wide = i.geometry(2.).unwrap();
    assert_eq!(wide.bounds.unwrap(), [-1., -0.5, 1., 0.5]);
    verify("wide-skin", &wide, &textures, &pages(&source), transform);
    i.play(0, "switch", None, 0., false, false).unwrap();
    i.step(0.).unwrap();
    let g = i.geometry(2.).unwrap();
    assert_eq!(g.batches[0].page, 1);
    verify(
        "attachment-page1",
        &g,
        &textures,
        &pages(&source),
        transform,
    );
}

#[test]
#[ignore = "requires a real GPU"]
fn spine374_b_gpu_clipping_and_transparent_draw_order() {
    let v = json!({"skeleton":{"spine":"4.2.00"},"bones":[{"name":"root"}],
        "slots":[{"name":"clip","bone":"root","attachment":"clip"},{"name":"image","bone":"root","attachment":"image"}],
        "skins":[{"name":"default","attachments":{"clip":{"clip":{"type":"clipping","end":"image","vertexCount":4,"vertices":[-1,-1,-1,1,0,1,0,-1]}},"image":{"image":{"type":"region","path":"triangle","width":2,"height":2}}}}]});
    let source = checked(&v, ATLAS);
    let textures = payloads(&source, &[[255, 0, 0, 255].repeat(4)]);
    let g = source
        .load()
        .unwrap()
        .instance()
        .unwrap()
        .geometry(1.)
        .unwrap();
    assert!(g
        .bounds
        .unwrap()
        .iter()
        .zip([-1., -1., 0., 1.])
        .all(|(a, b)| (*a - b).abs() <= 1e-4));
    verify(
        "clipped-left-half",
        &g,
        &textures,
        &pages(&source),
        IDENTITY_MATRIX,
    );
    let mut v = region("normal");
    v["slots"] = json!([{"name":"a","bone":"root","attachment":"image","color":"ff000080"},{"name":"b","bone":"root","attachment":"image","color":"00ff0080","blend":"additive"},{"name":"c","bone":"root","attachment":"image","color":"0000ff80","blend":"screen"}]);
    let image = json!({"image":{"type":"region","path":"triangle","width":2,"height":2}});
    v["skins"][0]["attachments"] = json!({"a":image,"b":image,"c":image});
    v["animations"] = json!({"swap":{"drawOrder":[{"offsets":[{"slot":"a","offset":2}]}]}});
    v["skins"][0]["attachments"]["b"]["image"]["path"] = json!("second");
    let atlas = format!(
        "{}\n{}",
        ATLAS,
        ATLAS
            .replace("probe.png", "second.png")
            .replace("triangle\n", "second\n")
    );
    let source = checked(&v, &atlas);
    let textures = payloads(
        &source,
        &[
            [255, 255, 255, 255].repeat(4),
            [255, 255, 255, 255].repeat(4),
        ],
    );
    let asset = source.load().unwrap();
    let mut i = asset.instance().unwrap();
    assert_eq!(
        i.geometry(1.)
            .unwrap()
            .batches
            .iter()
            .map(|b| b.page)
            .collect::<Vec<_>>(),
        [0, 1, 0]
    );
    let before = verify(
        "order-abc",
        &i.geometry(1.).unwrap(),
        &textures,
        &pages(&source),
        IDENTITY_MATRIX,
    );
    i.play(0, "swap", None, 0., false, false).unwrap();
    i.step(0.).unwrap();
    let after = verify(
        "order-bca",
        &i.geometry(1.).unwrap(),
        &textures,
        &pages(&source),
        IDENTITY_MATRIX,
    );
    assert_ne!(before, after, "draw-order change must change real pixels");
}

#[test]
#[ignore = "requires a real GPU"]
fn spine374_b_gpu_weighted_linked_deform_and_dynamic_bounds() {
    let mut v: Value = serde_json::from_str(include_str!(
        "../../engine_spine_runtime/tests/fixtures/probe.json"
    ))
    .unwrap();
    v["skins"][0]["attachments"]["cloth"]["triangle"]["vertices"] = json!([
        2, 0, 0, 0, 0.5, 2, 0, 0, 0.5, 2, 0, 2, 0, 0.5, 2, 2, 0, 0.5, 2, 0, 0, 2, 0.5, 2, 0, 2, 0.5
    ]);
    v["skins"][0]["attachments"]["cloth"]["linked"] =
        json!({"type":"linkedmesh","path":"triangle","parent":"triangle","timelines":true});
    v["slots"][0]["attachment"] = json!("linked");
    v["animations"]["motion"]["attachments"]["default"]["cloth"]["triangle"]["deform"][1]
        ["vertices"] = json!([2, 0, 2, 0]);
    v["animations"]["attack"]["attachments"]["default"]["cloth"]["triangle"]["deform"][1]
        ["vertices"] = json!([4, 0, 4, 0]);
    let source = checked(&v, ATLAS);
    let textures = payloads(&source, &[[255, 128, 64, 200].repeat(4)]);
    let asset = source.load().unwrap();
    let mut i = asset.instance().unwrap();
    let mut transform = IDENTITY_MATRIX;
    transform[12] = -2.;
    transform[13] = -6.;
    let setup = i.geometry(1.).unwrap();
    let before = verify(
        "weighted-linked-setup",
        &setup,
        &textures,
        &pages(&source),
        transform,
    );
    i.play(0, "motion", None, 0., false, false).unwrap();
    i.step(0.5).unwrap();
    let g = i.geometry(1.).unwrap();
    for (actual, expected) in g.vertices.iter().zip([(2., 5.), (3., 5.), (1., 7.)]) {
        assert!((actual.x - expected.0).abs() < 1e-4 && (actual.y - expected.1).abs() < 1e-4);
    }
    assert_ne!(setup.bounds, g.bounds);
    let after = verify(
        "weighted-linked-deformed",
        &g,
        &textures,
        &pages(&source),
        transform,
    );
    assert_ne!(before, after, "actual posed mesh must change GPU pixels");
}
