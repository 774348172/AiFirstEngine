#![cfg(feature = "real-wgpu")]
//! Gate C uses the production UI projection and an actual GPU, never a skipped adapter.
use engine_runtime::aui::*;
use engine_runtime::aui_extension::{AuiCustomNodeSpec, AuiCustomRuntimeOutput};
use engine_runtime::font_bundle::RuntimeFontBundleRegistry;
use engine_runtime::game_view_presentation::*;
use engine_runtime::render_state::RenderSceneState;
use engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry;
use engine_runtime::runtime_renderer::{
    QualityProfile, RenderTarget, RuntimeRenderer, RuntimeRendererInput,
};
use engine_runtime::wgpu_backend::real::RealWgpuBackend;
use project_runtime_sdk::aui_draw2d::*;
use project_runtime_sdk::aui_extension::UiExtensionPaintOutput;
use project_runtime_sdk::aui_presentation::{NodePresentation, Transform2D};

fn node(id: &str, parent: &str, rect: AuiRect) -> AuiNode {
    AuiNode::new(id, AuiNodeKind::Panel, rect).with_parent(parent)
}
fn doc() -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_style(AuiStyle::color("#000000"));
    root.children = vec!["clip".into(), "mirror".into(), "nested0".into()];
    let mut clip = node("clip", "root", AuiRect::fixed_position(50., 50., 80., 80.));
    clip.clip_policy = AuiClipPolicy::Rect;
    clip.style = Some(AuiStyle::color("#00000000"));
    clip.children = vec!["fill".into()];
    clip.presentation = Some(NodePresentation {
        transform: Transform2D {
            rotation: std::f32::consts::FRAC_PI_4,
            ..Default::default()
        },
        ..Default::default()
    });
    let fill = node(
        "fill",
        "clip",
        AuiRect::fixed_position(-60., -60., 200., 200.),
    )
    .with_style(AuiStyle::color("#ffffff"));
    let mut mirror = AuiNode::new(
        "mirror",
        AuiNodeKind::Custom,
        AuiRect::fixed_position(190., 10., 50., 50.),
    )
    .with_parent("root");
    mirror.custom = Some(AuiCustomNodeSpec {
        type_id: "test.draw".into(),
        schema_version: 1,
        properties: Default::default(),
    });
    let mut draw = Draw2D::new();
    draw.fill(
        Path2D::polygon(&[[0., 0.], [50., 0.], [0., 50.]], true).unwrap(),
        [1.0; 4],
        FillRule::NonZero,
    )
    .unwrap();
    mirror.custom_runtime = Some(AuiCustomRuntimeOutput {
        paint: Some(UiExtensionPaintOutput::from_graphics(draw.finish())),
        ..Default::default()
    });
    mirror.presentation = Some(NodePresentation {
        transform: Transform2D {
            scale: [-1., 1.],
            ..Default::default()
        },
        opacity: 0.5,
        tint: [1., 0., 0., 1.],
    });
    let mut nodes = vec![root, clip, fill, mirror];
    for i in 0..8 {
        let parent = if i == 0 {
            "root".into()
        } else {
            format!("nested{}", i - 1)
        };
        let mut clip = node(
            &format!("nested{i}"),
            &parent,
            AuiRect::fixed_position(
                if i == 0 { 190. } else { 0. },
                if i == 0 { 90. } else { 0. },
                60.,
                60.,
            ),
        );
        clip.clip_policy = AuiClipPolicy::Rect;
        clip.style = Some(AuiStyle::color("#00000000"));
        clip.children = vec![if i == 7 {
            "nested-fill".into()
        } else {
            format!("nested{}", i + 1)
        }];
        clip.presentation = Some(NodePresentation {
            transform: Transform2D {
                rotation: 0.08,
                scale: if i == 3 { [-1., 1.] } else { [1., 1.] },
                ..Default::default()
            },
            opacity: if i == 0 { 0.5 } else { 1. },
            ..Default::default()
        });
        nodes.push(clip);
    }
    let mut fill = node(
        "nested-fill",
        "nested7",
        AuiRect::fixed_position(-20., -20., 100., 100.),
    )
    .with_style(AuiStyle::color("#00ff00"));
    fill.presentation = Some(NodePresentation {
        opacity: 0.5,
        ..Default::default()
    });
    nodes.push(fill);
    let mut doc = AuiDocument::new(
        "presentation",
        vec![AuiCanvas::screen_overlay("main", 300., 180., "root")],
        nodes,
    );
    doc.schema_version = engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
    doc
}
fn mapping(scale: f32) -> ResolvedGameViewPresentation {
    let target = GameViewExtent::new((300. * scale) as u32, (180. * scale) as u32);
    GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "gate-c".into(),
        target_id: "target".into(),
        target_extent: target,
        display_rect: GameViewRect::from_extent(target),
        scale_policy: GameViewScalePolicy::Stretch,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 300, 180)],
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
            "gate-c",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(frame),
        ),
        &RuntimeAuiFontAtlasRegistry::empty("gate-c"),
        &RuntimeFontBundleRegistry::default(),
        Some(&mapping(scale)),
        runtime,
    )
}
fn render(
    backend: &mut RealWgpuBackend,
    renderer: &RuntimeRenderer,
    out: &AuiRuntimePresentOutput,
    scale: f32,
) -> Vec<u8> {
    let mapping = mapping(scale);
    assert_eq!(
        out.report.status,
        AuiRuntimePresentStatus::Success,
        "{:?}",
        out.report
    );
    let plan = renderer.build(RuntimeRendererInput {
        frame_index: out.report.frame_index,
        render_scene_state: &RenderSceneState::new(),
        render_view_state: None,
        aui_overlay: Some(&out.overlay),
        aui_composition: Some(&out.composition),
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: Some(&mapping),
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::headless_texture(
            "target",
            mapping.target_extent.width,
            mapping.target_extent.height,
        ),
    });
    assert!(
        plan.render_graph_report.diagnostics.is_empty(),
        "{:?}",
        plan.render_graph_report
    );
    backend
        .render_plan_to_rgba_bytes(
            &plan.rhi_command_plan,
            mapping.target_extent.width,
            mapping.target_extent.height,
        )
        .unwrap()
}
fn pixel(bytes: &[u8], scale: f32, x: usize, y: usize) -> [u8; 4] {
    let i =
        (((y as f32 * scale) as usize) * (300. * scale) as usize + (x as f32 * scale) as usize) * 4;
    bytes[i..i + 4].try_into().unwrap()
}

#[test]
fn aui_color_contract_srgb_gpu_matches_hex_and_preserves_linear_paint_and_tint() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("Color regression requires a real GPU");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("aui-color-regression"),
        ..Default::default()
    }))
    .unwrap();
    let mut backend = RealWgpuBackend::from_device_queue(
        device,
        queue,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        300,
        180,
        format!("{:?}", adapter.get_info().backend),
    );
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_style(AuiStyle::color("#20372f"));
    root.children = vec![
        "hex".into(),
        "primitive".into(),
        "graphics".into(),
        "tinted".into(),
        "hp".into(),
    ];
    let hex = node("hex", "root", AuiRect::fixed_position(10., 10., 40., 40.))
        .with_style(AuiStyle::color("#808080"));
    let mut primitive = AuiNode::new(
        "primitive",
        AuiNodeKind::Custom,
        AuiRect::fixed_position(60., 10., 40., 40.),
    )
    .with_parent("root");
    primitive.custom = Some(AuiCustomNodeSpec {
        type_id: "color-patch".into(),
        schema_version: 1,
        properties: Default::default(),
    });
    primitive.custom_runtime = Some(AuiCustomRuntimeOutput {
        paint: Some(UiExtensionPaintOutput {
            vertices: Vec::new(),
            indices: Vec::new(),
            asset_id: None,
            primitives: vec![
                project_runtime_sdk::aui_extension::UiExtensionPaintPrimitive::Rect {
                    rect: [0., 0., 40., 40.],
                    color: [0.2158605, 0.2158605, 0.2158605, 1.],
                },
            ],
        }),
        ..Default::default()
    });
    let mut draw = Draw2D::new();
    draw.fill(
        Path2D::rectangle([0., 0., 40., 40.]).unwrap(),
        [0.2158605, 0.2158605, 0.2158605, 1.],
        FillRule::NonZero,
    )
    .unwrap();
    let mut graphics = AuiNode::new(
        "graphics",
        AuiNodeKind::Custom,
        AuiRect::fixed_position(110., 10., 40., 40.),
    )
    .with_parent("root");
    graphics.custom = Some(AuiCustomNodeSpec {
        type_id: "color-patch".into(),
        schema_version: 1,
        properties: Default::default(),
    });
    graphics.custom_runtime = Some(AuiCustomRuntimeOutput {
        paint: Some(UiExtensionPaintOutput::from_graphics(draw.finish())),
        ..Default::default()
    });
    let mut tinted = node(
        "tinted",
        "root",
        AuiRect::fixed_position(160., 10., 40., 40.),
    )
    .with_style(AuiStyle::color("#808080"));
    tinted.presentation = Some(NodePresentation {
        tint: [0.5, 1., 1., 1.],
        ..Default::default()
    });
    let mut hp = AuiNode::new(
        "hp",
        AuiNodeKind::ProgressBar,
        AuiRect::fixed_position(210., 10., 80., 20.),
    )
    .with_parent("root")
    .with_progress_value(0.5)
    .with_style(AuiStyle::color("#303030"));
    hp.style.as_mut().unwrap().text_color = Some("#a34837".into());
    let doc = AuiDocument::new(
        "color-regression",
        vec![AuiCanvas::screen_overlay("main", 300., 180., "root")],
        vec![root, hex, primitive, graphics, tinted, hp],
    );
    let out = present(&doc, 1., 1, &mut AuiC2Runtime::default());
    let pixels = render(&mut backend, &RuntimeRenderer::new(), &out, 1.);
    for (x, y, expected) in [
        (5, 100, [32, 55, 47, 255]),
        (30, 30, [128, 128, 128, 255]),
        (80, 30, [128, 128, 128, 255]),
        (130, 30, [128, 128, 128, 255]),
        (180, 30, [92, 128, 128, 255]),
        (225, 20, [163, 72, 55, 255]),
        (275, 20, [48, 48, 48, 255]),
    ] {
        let actual = pixel(&pixels, 1., x, y);
        assert!(
            actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1),
            "sRGB pixel ({x},{y}): {actual:?}, expected {expected:?}"
        );
    }
}

#[test]
fn production_affine_mirror_pivot_eight_clips_modulation_dpi_and_cache() {
    let mut doc = doc();
    let mut runtime = AuiC2Runtime::default();
    let renderer = RuntimeRenderer::new();
    let mut backend = RealWgpuBackend::new_offscreen(600, 360).expect("Gate C requires a real GPU");
    backend.set_ui_measurement_enabled(true);
    eprintln!("Gate C adapter: {:?}", backend.device_context());
    for (frame, scale) in [1., 1.5, 2.].into_iter().enumerate() {
        let out = present(&doc, scale, frame as u64 + 1, &mut runtime);
        let pixels = render(&mut backend, &renderer, &out, scale);
        assert_eq!(
            pixel(&pixels, scale, 90, 90),
            [255, 255, 255, 255],
            "pivot center"
        );
        assert_eq!(
            pixel(&pixels, scale, 40, 90),
            [255, 255, 255, 255],
            "rotated interior outside original rect"
        );
        assert_eq!(
            pixel(&pixels, scale, 50, 50),
            [0, 0, 0, 255],
            "no AABB corner leakage"
        );
        assert_eq!(
            pixel(&pixels, scale, 195, 50),
            [0, 0, 0, 255],
            "mirror exclusion"
        );
        let red = pixel(&pixels, scale, 235, 20);
        assert!(
            (125..=130).contains(&red[0]) && red[1] == 0 && red[2] == 0,
            "mirror/tint/opacity: {red:?}"
        );
        let green = pixel(&pixels, scale, 220, 120);
        assert!(
            (62..=66).contains(&green[1]) && green[0] == 0,
            "inherited opacity: {green:?}"
        );
        assert_eq!(
            pixel(&pixels, scale, 188, 88),
            [0, 0, 0, 255],
            "nested clip leakage"
        );
        let fill = out
            .layout
            .computed_nodes
            .iter()
            .find(|n| n.node_id == "nested-fill")
            .unwrap();
        assert_eq!(fill.effective_clips.len(), 8);
        assert!(fill
            .effective_clips
            .iter()
            .all(|clip| clip.contains(AuiPointer::new(220., 120.))));
        if let Ok(root) = std::env::var("AIFE_365_C_EVIDENCE_ROOT") {
            if scale == 1. {
                std::fs::create_dir_all(&root).unwrap();
                let file =
                    std::fs::File::create(std::path::Path::new(&root).join("presentation-gpu.png"))
                        .unwrap();
                let mut encoder = png::Encoder::new(file, 300, 180);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(&pixels)
                    .unwrap();
            }
        }
    }
    let builds = runtime.graphics_cache_stats().0;
    let before = backend.ui_vertex_upload_stats().uploads;
    for frame in 4..10 {
        let out = present(&doc, 2., frame, &mut runtime);
        render(&mut backend, &renderer, &out, 2.);
    }
    assert_eq!(
        runtime.graphics_cache_stats().0,
        builds,
        "static retessellation"
    );
    assert_eq!(
        backend.ui_vertex_upload_stats().uploads,
        before,
        "static upload"
    );
    let mirror = doc
        .nodes
        .iter_mut()
        .find(|n| n.node_id == "mirror")
        .unwrap();
    mirror.presentation.as_mut().unwrap().opacity = 0.25;
    let out = present(&doc, 2., 10, &mut runtime);
    let pixels = render(&mut backend, &renderer, &out, 2.);
    assert!((62..=66).contains(&pixel(&pixels, 2., 235, 20)[0]));
    assert_eq!(
        runtime.graphics_cache_stats().0,
        builds,
        "opacity retessellation"
    );
}

#[test]
fn noncentral_pivot_nonuniform_rounded_clip_matches_inverse_hit_at_all_dpis() {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_style(AuiStyle::color("#000000"));
    root.children = vec!["clip".into()];
    let mut clip = node("clip", "root", AuiRect::fixed_position(110., 70., 80., 80.))
        .with_style(AuiStyle::color("#00000000"));
    clip.children = vec!["fill".into()];
    clip.clip_policy = AuiClipPolicy::Rounded;
    clip.clip_radius = Some(25.);
    clip.presentation = Some(NodePresentation {
        transform: Transform2D {
            rotation: 0.6,
            scale: [1.4, 0.7],
            pivot: [0.15, 0.9],
            ..Default::default()
        },
        ..Default::default()
    });
    let fill = node(
        "fill",
        "clip",
        AuiRect::fixed_position(-100., -100., 280., 280.),
    )
    .with_style(AuiStyle::color("#ffffff"));
    let mut document = AuiDocument::new(
        "rounded",
        vec![AuiCanvas::screen_overlay("main", 300., 180., "root")],
        vec![root, clip, fill],
    );
    document.schema_version = engine_runtime::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
    let mut runtime = AuiC2Runtime::default();
    let renderer = RuntimeRenderer::new();
    let mut backend = RealWgpuBackend::new_offscreen(600, 360).expect("real GPU");
    for (frame, scale) in [1., 1.5, 2.].into_iter().enumerate() {
        let out = present(&document, scale, frame as u64 + 1, &mut runtime);
        let pixels = render(&mut backend, &renderer, &out, scale);
        let clip = out
            .layout
            .computed_nodes
            .iter()
            .find(|n| n.node_id == "clip")
            .unwrap();
        let matrix = project_runtime_sdk::aui_presentation::Affine2D::from_coefficients(
            clip.presentation.unwrap().local_to_canvas,
        )
        .unwrap();
        let fill = out
            .layout
            .computed_nodes
            .iter()
            .find(|n| n.node_id == "fill")
            .unwrap();
        for (local, expected) in [
            ([40., 40.], [255; 4]),
            ([3., 3.], [0, 0, 0, 255]),
            ([17., 17.], [255; 4]),
        ] {
            let point = matrix.transform_point(local).unwrap();
            let index = (((point[1] * scale).floor() as usize) * (300. * scale) as usize
                + (point[0] * scale).floor() as usize)
                * 4;
            assert_eq!(
                &pixels[index..index + 4],
                &expected,
                "scale {scale}, local {local:?}, canvas {point:?}"
            );
            assert_eq!(
                fill.effective_clips[0].contains(AuiPointer::new(point[0], point[1])),
                expected[0] == 255
            );
        }
    }
}

#[test]
fn singular_and_clip_overflow_are_explicit_and_safe() {
    let mut doc = doc();
    doc.nodes
        .iter_mut()
        .find(|n| n.node_id == "mirror")
        .unwrap()
        .presentation
        .as_mut()
        .unwrap()
        .transform
        .scale = [0., 1.];
    let mut runtime = AuiC2Runtime::default();
    let out = present(&doc, 1., 1, &mut runtime);
    let mirror = out
        .layout
        .computed_nodes
        .iter()
        .find(|n| n.node_id == "mirror")
        .unwrap();
    assert!(!mirror.contains_presented_point(AuiPointer::new(215., 20.)));
    let mut backend = RealWgpuBackend::new_offscreen(300, 180).expect("real GPU");
    let pixels = render(&mut backend, &RuntimeRenderer::new(), &out, 1.);
    assert_eq!(pixel(&pixels, 1., 215, 20), [0, 0, 0, 255]);
    doc.nodes
        .iter_mut()
        .find(|n| n.node_id == "nested-fill")
        .unwrap()
        .clip_policy = AuiClipPolicy::Rect;
    doc.nodes
        .iter_mut()
        .find(|n| n.node_id == "nested-fill")
        .unwrap()
        .children
        .push("overflow".into());
    doc.nodes.push(node(
        "overflow",
        "nested-fill",
        AuiRect::fixed_position(0., 0., 10., 10.),
    ));
    let out = present(&doc, 1., 2, &mut runtime);
    assert!(out
        .layout
        .diagnostics
        .iter()
        .any(|d| d.code == "aui_clip_depth_exceeded"));
    assert!(
        !out.layout
            .computed_nodes
            .iter()
            .find(|n| n.node_id == "overflow")
            .unwrap()
            .effective_visible
    );
}
