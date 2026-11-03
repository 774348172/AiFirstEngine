#![cfg(feature = "real-wgpu")]
//! Gate B: production Presenter -> UiProjection -> RuntimeRenderer -> RHI -> GPU.
use engine_runtime::aui::*;
use engine_runtime::aui_extension::{AuiCustomNodeSpec, AuiCustomRuntimeOutput};
use engine_runtime::aui_graphics_asset::GraphicsAssetRegistry;
use engine_runtime::font_bundle::RuntimeFontBundleRegistry;
use engine_runtime::game_view_presentation::*;
use engine_runtime::render_state::RenderSceneState;
use engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry;
use engine_runtime::runtime_renderer::{
    QualityProfile, RenderTarget, RuntimeRenderer, RuntimeRendererInput,
};
use engine_runtime::wgpu_backend::real::RealWgpuBackend;
use project_runtime_sdk::aui_draw2d::*;
use project_runtime_sdk::aui_extension::{UiExtensionPaintOutput, UiExtensionPaintPrimitive};
use std::collections::BTreeMap;

fn graphics(path: Path2D, color: [f32; 4], rule: FillRule) -> Graphics2D {
    let mut draw = Draw2D::new();
    draw.fill(path, color, rule).unwrap();
    draw.finish()
}
fn node(id: &str, x: f32, y: f32, primitive: UiExtensionPaintPrimitive) -> AuiNode {
    let mut node = AuiNode::new(
        id,
        AuiNodeKind::Custom,
        AuiRect::fixed_position(x, y, 100., 90.),
    );
    node.parent = Some("root".into());
    node.custom = Some(AuiCustomNodeSpec {
        type_id: "test.graphics".into(),
        schema_version: 1,
        properties: BTreeMap::new(),
    });
    node.custom_runtime = Some(AuiCustomRuntimeOutput {
        paint: Some(UiExtensionPaintOutput {
            vertices: vec![],
            indices: vec![],
            asset_id: None,
            primitives: vec![primitive],
        }),
        ..Default::default()
    });
    node
}
fn document() -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.style = Some(AuiStyle {
        color: Some("#000000".into()),
        text_color: None,
        font_size: None,
        font: None,
    });
    // Same winding: only EvenOdd cuts the inner hole.
    let mut path = PathBuilder::new();
    for points in [
        [[5., 5.], [75., 5.], [75., 75.], [5., 75.]],
        [[25., 25.], [55., 25.], [55., 55.], [25., 55.]],
    ] {
        path.move_to(points[0]).unwrap();
        for p in &points[1..] {
            path.line_to(*p).unwrap();
        }
        path.close().unwrap();
    }
    let path = path.finish();
    let asset = Graphics2DAsset {
        schema_version: GRAPHICS_ASSET_SCHEMA.into(),
        asset_id: "ring".into(),
        asset_guid: "guid-ring".into(),
        graphics: graphics(path.clone(), [0., 1., 0., 1.], FillRule::EvenOdd),
    };
    let mut curve = PathBuilder::new();
    curve
        .move_to([5., 45.])
        .unwrap()
        .quadratic_to([25., 5.], [45., 45.])
        .unwrap()
        .cubic_to([55., 75.], [65., 5.], [85., 45.])
        .unwrap();
    let mut strokes = Draw2D::new();
    strokes
        .stroke(
            curve.finish(),
            StrokeStyle {
                color: [1., 1., 1., 1.],
                width: 5.,
                cap: LineCap::Round,
                join: LineJoin::Round,
                ..Default::default()
            },
        )
        .unwrap();
    let mut nodes = vec![
        node(
            "evenodd",
            0.,
            0.,
            UiExtensionPaintPrimitive::GraphicsAsset {
                asset_id: "ring".into(),
                guid: "guid-ring".into(),
            },
        ),
        node(
            "nonzero",
            85.,
            0.,
            UiExtensionPaintPrimitive::Graphics {
                graphics: graphics(path, [1., 0., 0., 1.], FillRule::NonZero),
            },
        ),
        node(
            "circle",
            170.,
            0.,
            UiExtensionPaintPrimitive::Graphics {
                graphics: graphics(
                    Path2D::circle([40., 40.], 30.).unwrap(),
                    [1., 1., 1., 0.5],
                    FillRule::NonZero,
                ),
            },
        ),
        node(
            "curve",
            0.,
            85.,
            UiExtensionPaintPrimitive::Graphics {
                graphics: strokes.finish(),
            },
        ),
        node(
            "concave",
            85.,
            85.,
            UiExtensionPaintPrimitive::Graphics {
                graphics: graphics(
                    Path2D::polygon(
                        &[
                            [5., 5.],
                            [75., 5.],
                            [75., 25.],
                            [25., 25.],
                            [25., 75.],
                            [5., 75.],
                        ],
                        true,
                    )
                    .unwrap(),
                    [0., 0., 1., 1.],
                    FillRule::NonZero,
                ),
            },
        ),
        node(
            "crossing",
            170.,
            85.,
            UiExtensionPaintPrimitive::Graphics {
                graphics: graphics(
                    Path2D::polygon(&[[5., 5.], [75., 75.], [5., 75.], [75., 5.]], true).unwrap(),
                    [1., 1., 0., 1.],
                    FillRule::EvenOdd,
                ),
            },
        ),
    ];
    let mut small = Draw2D::new();
    small
        .fill(
            Path2D::rounded_rectangle([0.5, 0.5, 8., 8.], 1.5).unwrap(),
            [0.25, 0.5, 1., 0.6],
            FillRule::NonZero,
        )
        .unwrap();
    small
        .fill(
            Path2D::circle([20.5, 5.5], 2.5).unwrap(),
            [1.; 4],
            FillRule::NonZero,
        )
        .unwrap();
    small
        .fill(
            Path2D::circle([35.5, 5.5], 0.4).unwrap(),
            [1.; 4],
            FillRule::NonZero,
        )
        .unwrap();
    small
        .stroke(
            Path2D::polygon(&[[50.5, 8.5], [52.5, 0.5], [54.5, 8.5]], false).unwrap(),
            StrokeStyle {
                width: 1.5,
                miter_limit: 2.,
                ..Default::default()
            },
        )
        .unwrap();
    nodes.push(node(
        "small",
        5.,
        165.,
        UiExtensionPaintPrimitive::Graphics {
            graphics: small.finish(),
        },
    ));
    root.children = nodes.iter().map(|n| n.node_id.clone()).collect();
    let mut doc = AuiDocument::new(
        "draw2d",
        vec![AuiCanvas::screen_overlay("main", 270., 180., "root")],
        std::iter::once(root).chain(nodes).collect(),
    );
    doc.graphics_assets = GraphicsAssetRegistry::from_assets(vec![asset]).unwrap();
    doc
}
fn presentation(scale: f32) -> ResolvedGameViewPresentation {
    let width = (270. * scale) as u32;
    let height = (180. * scale) as u32;
    GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "gate-b".into(),
        target_id: "target".into(),
        target_extent: GameViewExtent::new(width, height),
        display_rect: GameViewRect::new(0., 0., width as f32, height as f32),
        scale_policy: GameViewScalePolicy::Stretch,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 270, 180)],
    })
    .unwrap()
}
fn present(
    doc: &AuiDocument,
    scale: f32,
    frame: u64,
    runtime: &mut AuiC2Runtime,
) -> AuiRuntimePresentOutput {
    AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
        doc,
        ProjectUiStateSnapshotOutput::new(
            "gate-b",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(frame),
        ),
        &RuntimeAuiFontAtlasRegistry::empty("gate-b"),
        &RuntimeFontBundleRegistry::default(),
        Some(&presentation(scale)),
        runtime,
    )
}
fn render(
    backend: &mut RealWgpuBackend,
    renderer: &RuntimeRenderer,
    output: &AuiRuntimePresentOutput,
    scale: f32,
) -> Vec<u8> {
    let plan = build_plan(renderer, output, scale);
    let target = presentation(scale).target_extent;
    backend
        .render_plan_to_rgba_bytes(&plan.rhi_command_plan, target.width, target.height)
        .unwrap()
}
fn build_plan(
    renderer: &RuntimeRenderer,
    output: &AuiRuntimePresentOutput,
    scale: f32,
) -> engine_runtime::runtime_renderer::RuntimeRendererOutput {
    let mapping = presentation(scale);
    let target = mapping.target_extent;
    let plan = renderer.build(RuntimeRendererInput {
        frame_index: output.overlay.frame_index,
        render_scene_state: &RenderSceneState::new(),
        render_view_state: None,
        aui_overlay: Some(&output.overlay),
        aui_composition: Some(&output.composition),
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: Some(&mapping),
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::headless_texture("target", target.width, target.height),
    });
    assert_eq!(
        plan.render_frame_report.ui_screen_overlay_item_count,
        output.overlay.draw_items.len()
    );
    assert!(
        plan.render_graph_report.diagnostics.is_empty(),
        "{:?}",
        plan.render_graph_report
    );
    plan
}
fn pixel(bytes: &[u8], scale: f32, x: usize, y: usize) -> [u8; 4] {
    let width = (270. * scale) as usize;
    let i = ((y as f32 * scale) as usize * width + (x as f32 * scale) as usize) * 4;
    bytes[i..i + 4].try_into().unwrap()
}
#[test]
fn production_paths_holes_curves_alpha_cache_dpi_and_gpu_recreation() {
    let doc = document();
    let mut runtime = AuiC2Runtime::default();
    let renderer = RuntimeRenderer::new();
    let mut backend = RealWgpuBackend::new_offscreen(540, 360).expect("Gate B requires a real GPU");
    backend.set_ui_measurement_enabled(true);
    eprintln!("Gate B adapter: {:?}", backend.device_context());
    let first = present(&doc, 1., 1, &mut runtime);
    assert_eq!(
        first.report.status,
        AuiRuntimePresentStatus::Success,
        "{:?}",
        first.report
    );
    let pixels = render(&mut backend, &renderer, &first, 1.);
    assert_eq!(pixel(&pixels, 1., 40, 40), [0, 0, 0, 255], "hole");
    assert_eq!(pixel(&pixels, 1., 10, 10), [0, 255, 0, 255], "ring");
    assert_eq!(
        pixel(&pixels, 1., 125, 40),
        [255, 0, 0, 255],
        "nonzero center"
    );
    let alpha = pixel(&pixels, 1., 210, 40)[0];
    assert!((125..=130).contains(&alpha), "straight alpha: {alpha}");
    let rgba = pixel(&pixels, 1., 9, 169);
    assert!(
        rgba[0].abs_diff(38) <= 1 && rgba[1].abs_diff(77) <= 1 && rgba[2].abs_diff(153) <= 1,
        "linear RGBA/straight alpha: {rgba:?}"
    );
    assert_eq!(
        pixel(&pixels, 1., 25, 170),
        [255, 255, 255, 255],
        "small circle center"
    );
    assert!(
        pixel(&pixels, 1., 40, 170)[0] > 0,
        "subpixel geometry disappeared"
    );
    assert_eq!(
        pixel(&pixels, 1., 135, 135),
        [0, 0, 0, 255],
        "concave exclusion"
    );
    assert_eq!(
        pixel(&pixels, 1., 95, 95),
        [0, 0, 255, 255],
        "concave interior"
    );
    assert_eq!(
        pixel(&pixels, 1., 25, 110),
        [255, 255, 255, 255],
        "quadratic midpoint"
    );
    assert_eq!(
        pixel(&pixels, 1., 62, 126),
        [255, 255, 255, 255],
        "cubic midpoint"
    );
    assert_eq!(
        pixel(&pixels, 1., 210, 95),
        [255, 255, 0, 255],
        "crossing top lobe"
    );
    assert_eq!(
        pixel(&pixels, 1., 178, 125),
        [0, 0, 0, 255],
        "crossing exclusion"
    );
    assert!(
        pixels
            .chunks_exact(4)
            .any(|p| p[0] > 0 && p[0] < 120 && p[0] == p[1] && p[1] == p[2]),
        "curved AA coverage"
    );
    let builds = runtime.graphics_cache_stats().0;
    let uploads = backend.ui_vertex_upload_stats().uploads;
    for frame in 2..8 {
        let output = present(&doc, 1., frame, &mut runtime);
        render(&mut backend, &renderer, &output, 1.);
    }
    assert_eq!(
        runtime.graphics_cache_stats().0,
        builds,
        "clean frames retessellated"
    );
    assert_eq!(
        backend.ui_vertex_upload_stats().uploads,
        uploads,
        "clean frames reuploaded"
    );
    for scale in [1.5, 2., 1.] {
        let output = present(&doc, scale, 10, &mut runtime);
        assert_eq!(
            output.report.status,
            AuiRuntimePresentStatus::Success,
            "{:?}",
            output.report
        );
        let bytes = render(&mut backend, &renderer, &output, scale);
        assert_eq!(pixel(&bytes, scale, 40, 40), [0, 0, 0, 255]);
        assert_eq!(pixel(&bytes, scale, 10, 10), [0, 255, 0, 255]);
    }
    assert_eq!(
        runtime.graphics_cache_stats().0,
        builds * 3,
        "return to DPI1 must reuse geometry"
    );
    let mut recreated = RealWgpuBackend::new_offscreen(270, 180).unwrap();
    assert_eq!(
        render(&mut recreated, &renderer, &first, 1.),
        pixels,
        "GPU recreation must restore pixels"
    );
    if let Some(root) = std::env::var_os("AIFE_365_B_EVIDENCE_ROOT") {
        std::fs::create_dir_all(&root).unwrap();
        let file =
            std::fs::File::create(std::path::PathBuf::from(root).join("draw2d-gpu.png")).unwrap();
        let mut encoder = png::Encoder::new(file, 270, 180);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
    }
    eprintln!(
        "Gate B geometry cache: {:?}; GPU uploads: {:?}",
        runtime.graphics_cache_stats(),
        backend.ui_vertex_upload_stats()
    );
    let mut moved = doc.clone();
    moved.nodes[1].rect = AuiRect::fixed_position(3., 0., 100., 90.);
    let before = runtime.graphics_cache_stats().0;
    let output = present(&moved, 1., 11, &mut runtime);
    let bytes = render(&mut backend, &renderer, &output, 1.);
    assert_eq!(
        runtime.graphics_cache_stats().0,
        before,
        "translation retessellated"
    );
    assert_eq!(pixel(&bytes, 1., 6, 10), [0, 0, 0, 255]);
    assert_eq!(pixel(&bytes, 1., 13, 10), [0, 255, 0, 255]);
    let original = doc.graphics_assets.resolve("ring", "guid-ring").unwrap();
    moved.graphics_assets = GraphicsAssetRegistry::from_assets(vec![Graphics2DAsset {
        graphics: graphics(
            match &original.graphics.items()[0] {
                Draw2DItem::Fill { path, .. } | Draw2DItem::Stroke { path, .. } => path.clone(),
            },
            [0., 1., 1., 1.],
            FillRule::EvenOdd,
        ),
        ..original.clone()
    }])
    .unwrap();
    let output = present(&moved, 1., 12, &mut runtime);
    let bytes = render(&mut backend, &renderer, &output, 1.);
    assert_eq!(
        pixel(&bytes, 1., 13, 10),
        [0, 255, 255, 255],
        "asset replacement remained stale"
    );
    assert_eq!(
        runtime.graphics_cache_stats().0,
        before + 1,
        "unaffected paths rebuilt"
    );
}

#[test]
fn retained_graphics_translation_avoids_cpu_mesh_and_geometry_upload() {
    let mut doc = document();
    doc.nodes
        .retain(|node| node.node_id == "root" || node.node_id == "evenodd");
    doc.nodes[0].children = vec!["evenodd".into()];
    doc.nodes[0].clip_policy = AuiClipPolicy::Rect;
    let mut runtime = AuiC2Runtime::default();
    let renderer = RuntimeRenderer::new();
    renderer.set_ui_measurement_enabled(true);
    let mut backend = RealWgpuBackend::new_offscreen(270, 180).unwrap();
    backend.set_ui_measurement_enabled(true);
    let initial = present(&doc, 1., 1, &mut runtime);
    render(&mut backend, &renderer, &initial, 1.);
    let projection_before = renderer.ui_projection_stats();
    let uploads_before = backend.ui_vertex_upload_stats();
    let retained_before = backend.retained_ui_stats();
    let builds_before = runtime.graphics_cache_stats().0;
    doc.nodes[1].rect = AuiRect::fixed_position(3., 0., 100., 90.);
    let moved = present(&doc, 1., 2, &mut runtime);
    let bytes = render(&mut backend, &renderer, &moved, 1.);
    let projection = renderer.ui_projection_stats().since(projection_before);
    assert_eq!(runtime.graphics_cache_stats().0, builds_before);
    assert_eq!(
        projection.vertices_generated, 0,
        "warm translated Graphics2D still generated CPU draw vertices: {projection:?}"
    );
    assert_eq!(
        backend.ui_vertex_upload_stats().uploaded_bytes,
        uploads_before.uploaded_bytes
    );
    let retained = backend.retained_ui_stats();
    assert_eq!(
        retained.geometry_uploaded_bytes,
        retained_before.geometry_uploaded_bytes
    );
    assert_eq!(
        retained.parameter_uploaded_bytes - retained_before.parameter_uploaded_bytes,
        64
    );
    assert_eq!(pixel(&bytes, 1., 6, 10), [0, 0, 0, 255]);
    assert_eq!(pixel(&bytes, 1., 13, 10), [0, 255, 0, 255]);
}

#[test]
fn retained_cpu_reference_mixed_fallback_and_old_packets_match_real_gpu() {
    use project_runtime_sdk::aui_presentation::{NodePresentation, Transform2D};
    let mut doc = document();
    // Rotate/tint one draw, retain others, and force a non-integral clipping boundary
    // in another stage of this test. Both paths must preserve holes and alpha fringe.
    doc.nodes[2].presentation = Some(NodePresentation {
        transform: Transform2D {
            rotation: 0.13,
            scale: [-1., 1.],
            ..Default::default()
        },
        opacity: 0.6,
        tint: [0.8, 1., 0.7, 1.],
    });
    let mut raw = node(
        "raw",
        12.,
        4.,
        UiExtensionPaintPrimitive::Graphics {
            graphics: Draw2D::new().finish(),
        },
    );
    raw.custom_runtime.as_mut().unwrap().paint = Some(UiExtensionPaintOutput {
        vertices: [[0., 0.], [80., 0.], [80., 70.], [0., 70.]]
            .map(
                |position| project_runtime_sdk::aui_extension::UiExtensionMeshVertex {
                    position,
                    color: [0.8, 0.2, 0.3, 0.35],
                    uv: [0.; 2],
                },
            )
            .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
        asset_id: None,
        primitives: vec![],
    });
    doc.nodes[0].children.insert(1, "raw".into());
    doc.nodes.push(raw);
    let gpu = RuntimeRenderer::new();
    let cpu = RuntimeRenderer::new();
    cpu.set_retained_graphics_enabled(false);
    let mut runtime = AuiC2Runtime::default();
    let mut backend = RealWgpuBackend::new_offscreen(270, 180).unwrap();
    let old = present(&doc, 1., 1, &mut runtime);
    let old_packet = build_plan(&gpu, &old, 1.);
    let expected_old = render(&mut backend, &cpu, &old, 1.);
    doc.nodes[1].rect = AuiRect::fixed_position(12., 3., 100., 90.);
    let asset = doc
        .graphics_assets
        .resolve("ring", "guid-ring")
        .unwrap()
        .clone();
    doc.graphics_assets = GraphicsAssetRegistry::from_assets(vec![Graphics2DAsset {
        graphics: graphics(
            match &asset.graphics.items()[0] {
                Draw2DItem::Fill { path, .. } | Draw2DItem::Stroke { path, .. } => path.clone(),
            },
            [0., 1., 1., 1.],
            FillRule::EvenOdd,
        ),
        ..asset
    }])
    .unwrap();
    let next = present(&doc, 1., 2, &mut runtime);
    let next_packet = build_plan(&gpu, &next, 1.);
    let expected_next = render(&mut backend, &cpu, &next, 1.);
    let compare = |actual: &[u8], expected: &[u8]| {
        let max = actual
            .iter()
            .zip(expected)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(max <= 2, "CPU/GPU coverage or ordering differs by {max}");
    };
    // CPU packet A -> B -> A tests that no immutable packet points to overwritten
    // instance data. Real readback waits on submitted GPU work, not a CPU frame delay.
    for (packet, expected) in [
        (&old_packet, &expected_old),
        (&next_packet, &expected_next),
        (&old_packet, &expected_old),
    ] {
        compare(
            &backend
                .render_plan_to_rgba_bytes(&packet.rhi_command_plan, 270, 180)
                .unwrap(),
            expected,
        );
    }
    assert!(backend.retained_ui_stats().geometry_uploads > 0);
    doc.nodes[0].clip_policy = AuiClipPolicy::Rect;
    doc.nodes[0].rect = AuiRect::fixed_position(0., 0., 204.25, 180.);
    let fractional = present(&doc, 1., 3, &mut runtime);
    compare(
        &render(&mut backend, &gpu, &fractional, 1.),
        &render(&mut backend, &cpu, &fractional, 1.),
    );
}

#[test]
fn retained_eviction_replays_old_geometry_and_rejects_invalid_or_overbudget_packets() {
    use engine_runtime::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    let mut doc = document();
    doc.nodes
        .retain(|n| n.node_id == "root" || n.node_id == "evenodd");
    doc.nodes[0].children = vec!["evenodd".into()];
    let presented = present(&doc, 1., 1, &mut AuiC2Runtime::default());
    let packet = build_plan(&RuntimeRenderer::new(), &presented, 1.).rhi_command_plan;
    let slot = packet
        .commands
        .iter()
        .position(|c| {
            matches!(
                c,
                RhiCommand::Draw {
                    payload: RhiDrawPayload::UiRetained { .. },
                    ..
                }
            )
        })
        .unwrap();
    let mut backend = RealWgpuBackend::new_offscreen(270, 180).unwrap();
    let original = backend
        .render_plan_to_rgba_bytes(&packet, 270, 180)
        .unwrap();
    for generation in 0..1025 {
        let mut next = packet.clone();
        let RhiCommand::Draw {
            payload: RhiDrawPayload::UiRetained { draws },
            ..
        } = &mut next.commands[slot]
        else {
            unreachable!()
        };
        let geometry = std::sync::Arc::make_mut(&mut draws[0].geometry);
        geometry.id = 1_000_000 + generation;
        // Replace geometry incarnation while preserving the retained old CPU packet.
        backend.render_plan_to_rgba_bytes(&next, 270, 180).unwrap();
    }
    let stats = backend.retained_ui_stats();
    assert_eq!(stats.resident_entries, 1024);
    assert!(stats.resident_bytes <= 32 * 1024 * 1024 && stats.retiring_bytes <= 32 * 1024 * 1024);
    assert!(
        stats.completed_submission > 0,
        "no real GPU completion observed"
    );
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&packet, 270, 180)
            .unwrap(),
        original
    );
    assert_eq!(
        backend.retained_ui_stats().geometry_uploads,
        stats.geometry_uploads + 1,
        "evicted old packet did not reupload its own geometry"
    );
    let mut invalid = packet.clone();
    let RhiCommand::Draw {
        payload: RhiDrawPayload::UiRetained { draws },
        ..
    } = &mut invalid.commands[slot]
    else {
        unreachable!()
    };
    std::sync::Arc::make_mut(&mut draws[0].geometry).indices[0] = u32::MAX;
    assert!(backend
        .render_plan_to_rgba_bytes(&invalid, 270, 180)
        .unwrap_err()
        .contains("geometry_invalid"));
    let mut overflow = packet.clone();
    let RhiCommand::Draw {
        payload: RhiDrawPayload::UiRetained { draws },
        ..
    } = &mut overflow.commands[slot]
    else {
        unreachable!()
    };
    draws.resize(4097, draws[0].clone());
    assert!(backend
        .render_plan_to_rgba_bytes(&overflow, 270, 180)
        .unwrap_err()
        .contains("instance_budget"));
    assert_eq!(
        backend
            .render_plan_to_rgba_bytes(&packet, 270, 180)
            .unwrap(),
        original,
        "budget failure poisoned good frame"
    );
}

#[test]
fn clipping_preserves_vertex_coverage_and_invalid_assets_remain_errors() {
    let mut doc = document();
    doc.nodes[0].clip_policy = AuiClipPolicy::Rect;
    doc.nodes[0].rect = AuiRect::fixed_position(0., 0., 205., 180.);
    let mut runtime = AuiC2Runtime::default();
    let renderer = RuntimeRenderer::new();
    let mut backend = RealWgpuBackend::new_offscreen(270, 180).unwrap();
    let reference = present(&document(), 1., 0, &mut AuiC2Runtime::default());
    let reference_bytes = render(&mut backend, &renderer, &reference, 1.);
    let output = present(&doc, 1., 1, &mut runtime);
    let bytes = render(&mut backend, &renderer, &output, 1.);
    assert_eq!(pixel(&bytes, 1., 210, 40)[0], 0, "clip outside");
    let partial = (10..70)
        .flat_map(|y| (170..205).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let v = pixel(&bytes, 1., x, y)[0];
            v > 0 && v < 120
        })
        .count();
    assert!(
        partial > 10,
        "clipped fringe lost interpolated alpha: {partial}"
    );
    for y in 10..70 {
        for x in 170..205 {
            let a = pixel(&reference_bytes, 1., x, y);
            let b = pixel(&bytes, 1., x, y);
            assert!(
                a.iter().zip(b).all(|(&a, b)| a.abs_diff(b) <= 1),
                "clip changed coverage at {x},{y}: {a:?}/{b:?}"
            );
        }
    }
    doc.graphics_assets = GraphicsAssetRegistry::default();
    for frame in [2, 3] {
        let invalid = present(&doc, 1., frame, &mut runtime);
        assert!(
            invalid
                .report
                .diagnostics
                .iter()
                .any(|d| d.message.contains("guid") || d.message.contains("GUID")),
            "{:?}",
            invalid.report
        );
    }
}
