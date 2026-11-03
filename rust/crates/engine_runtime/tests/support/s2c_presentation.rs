use super::*;
use engine_runtime::{particle_effect::*, particle_render_contract::*, render_graph::*};

pub(super) fn effect(space: ParticleSpace, alpha: f32) -> CookedParticleEffect {
    let mut d: ParticleEffectDescription =
        serde_json::from_str(include_str!("../fixtures/particle-effect.json")).unwrap();
    let e = &mut d.emitters[0];
    e.space = space;
    e.capacity = 4;
    e.bursts[0].count = 1;
    e.velocity_min = [0.; 3];
    e.velocity_max = [0.; 3];
    e.lifetime_seconds = [10.; 2];
    e.size_meters = [0.5; 2];
    e.color = [1., 0., 0., alpha];
    e.updates = vec![ParticleUpdate::Integrate {}];
    e.draw.blend = ParticleBlend::Alpha;
    e.draw.sort = ParticleSort::BackToFront;
    d.validate().unwrap();
    let programs = d
        .emitters
        .iter()
        .map(|e| CookedParticleProgram {
            emitter: e.name.clone(),
            wgsl: program::compute_source(
                &d,
                e,
                &(program::prefix(&d, e) + program::DEFAULT_BEHAVIOR + program::CONTRACT_CALLS),
            ),
            author_path: None,
            author_first_line: 0,
            author_line_count: 0,
            particle_stride: program::particle_layout(e).2,
        })
        .collect();
    CookedParticleEffect {
        description: d,
        program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
        programs,
        source_digests: Default::default(),
    }
}

fn scene(
    space: ParticleSpace,
    alpha: f32,
    z: f32,
    x: f32,
    step: u64,
    hdr: bool,
    origin: f64,
) -> RenderGraph {
    let mut manager = RenderResourceManager::new();
    let mesh = geometry_fixture::prepare(
        "presentation-mesh",
        RenderAssetData::Mesh(geometry_fixture::triangle()),
        &mut manager,
    );
    let mat = geometry_fixture::prepare(
        "presentation-blue",
        RenderAssetData::Material(geometry_fixture::unlit([0., 0., 1., 1.])),
        &mut manager,
    );
    let source = PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).unwrap();
    let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    if hdr {
        view.color_pipeline = Some(Default::default());
    }
    let shift = (1e10 - origin) as f32;
    let mut frame = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose {
                position: [shift, 0., 0.],
                ..Default::default()
            },
        }],
        &[geometry_fixture::instance(&source, 1, [shift, 0., -2.])],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    frame.views[0].coordinate_origin = [origin, 0., 0.];
    let source=ParticleSourceFrame {
        simulation_origin_bits:[1e10f64.to_bits(),0,0],instance:7,epoch:0,
        assets:serde_json::json!({"effect":effect(space,alpha),"textures":[null],"meshes":[null],"tints":[[1,1,1,1]]}).to_string().into(),
        parameters:"{}".into(),textures:vec![None],meshes:vec![None],
        step:ParticleRenderStep{emitter_affine: Default::default(),step_id:step,delta_seconds:0.015625.into(),origin:[x.into(),0.0.into(),z.into()],emitting:true,paused:step>1},
    };
    assemble(frame, source)
}

pub(super) fn assemble(frame: GeometryFrame, source: ParticleSourceFrame) -> RenderGraph {
    let empty = GeometryFrame {
        target: "main".into(),
        extent: [64, 64],
        views: vec![],
    };
    let mut graph = geometry_fixture::graph(&empty);
    graph.passes.push(RenderPass {
        pass_id: "particles".into(),
        pass_name: "particles".into(),
        pass_kind: RenderPassKind::DrawParticles,
        view_id: "old-2d".into(),
        reads: vec![],
        writes: vec!["main".into()],
        color_targets: vec!["main".into()],
        depth_target: None,
        debug_source: None,
        commands: vec![
            RenderPassCommand::ReconcileParticles {
                target: "main".into(),
                sources: vec![source.clone()],
            },
            RenderPassCommand::SimulateParticles {
                target: "main".into(),
                instance: source.instance,
                step: source.step.clone(),
            },
            RenderPassCommand::DrawParticles {
                target: "main".into(),
                instance: source.instance,
                emitter: 0,
                view: ParticleRenderView {
                    origin: source.step.origin,
                    ..Default::default()
                },
                texture: None,
                mesh: None,
            },
        ]
        .into(),
    });
    frame.append_to_graph(&mut graph);
    graph
}

fn compile(g: &RenderGraph) -> engine_runtime::rhi_command_plan::RhiCommandPlan {
    let p = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(g);
    assert!(!p.has_errors(), "{:?}", p.diagnostics);
    p
}

#[test]
fn particle_scene_graph_declares_shared_depth_and_pre_tone_write() {
    let g = scene(ParticleSpace::World, 0.5, -1., 0., 1, true, 1e10);
    compile(&g);
    let p = g
        .passes
        .iter()
        .position(|p| p.pass_id == "particles")
        .unwrap();
    assert_eq!(
        g.passes[p].depth_target.as_deref(),
        Some("main:depth:view-1")
    );
    assert_eq!(g.passes[p].color_targets, ["color3d-1-hdr"]);
    assert!(
        g.passes
            .iter()
            .position(|p| p.pass_kind == RenderPassKind::ResolveColor3d)
            .unwrap()
            > p
    );
    let mut bad = g.clone();
    bad.passes[p].depth_target = None;
    assert!(engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&bad).has_errors());
}

#[test]
fn mesh_decal_contract_rejects_bad_composition_and_preserves_legacy_identity() {
    let mut m = geometry_fixture::unlit([1., 0., 0., 0.5]);
    let old = m.pipeline_identity().unwrap();
    assert!(serde_json::to_value(&m).unwrap().get("decalBias").is_none());
    m.decal_bias = Some(-2);
    assert!(m.validate().is_err());
    m.blend = MaterialBlend::AlphaBlend;
    m.depth = MaterialDepth::TestOnly;
    m.validate().unwrap();
    assert_ne!(old, m.pipeline_identity().unwrap());
    for v in [-17, 0, 1] {
        m.decal_bias = Some(v);
        assert!(m.validate().is_err());
    }
    m.decal_bias = Some(-2);
    m.depth = MaterialDepth::TestAndWrite;
    assert!(m.validate().is_err());
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn gpu_scene_particles_share_depth_hdr_and_stable_world_local_origins() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let center = |v: &[u8]| -> [u8; 4] { v[(32 * 64 + 32) * 4..][..4].try_into().unwrap() };
    for (z, expected) in [(-3., [0, 0, 255, 255]), (-1., [255, 0, 0, 255])] {
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let g = scene(ParticleSpace::World, 1., z, 0., 1, false, 1e10);
        let image = backend
            .render_plan_to_rgba_bytes(&compile(&g), 64, 64)
            .unwrap();
        assert_eq!(center(&image), expected, "z={z}");
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
    for space in [ParticleSpace::World, ParticleSpace::Local] {
        let mut b = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let first = compile(&scene(space, 1., -1., 0., 1, false, 1e10));
        let a = b.render_plan_to_rgba_bytes(&first, 64, 64).unwrap();
        assert_eq!(center(&a), [255, 0, 0, 255]);
        let rebased = compile(&scene(space, 1., -1., 0., 1, false, 1e10 + 0.125));
        assert_eq!(
            a,
            b.render_plan_to_rgba_bytes(&rebased, 64, 64).unwrap(),
            "view rebasing moved particles"
        );
        let moved = compile(&scene(space, 1., -1., 0.6, 2, false, 1e10 + 0.125));
        let moved = b.render_plan_to_rgba_bytes(&moved, 64, 64).unwrap();
        assert_eq!(
            center(&moved),
            if space == ParticleSpace::World {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        );
        b.shutdown_gpu(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(b.gpu_lifecycle_stats().retiring_objects, 0);
    }
    let mut b = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let g = scene(ParticleSpace::World, 0.5, -1., 0., 1, true, 1e10);
    let image = b.render_plan_to_rgba_bytes(&compile(&g), 64, 64).unwrap();
    let p = center(&image);
    let tone = |x: f64| {
        let t = (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0., 1.);
        ((1.055 * t.powf(1. / 2.4) - 0.055) * 255.).round() as u8
    };
    assert!(
        p[0].abs_diff(tone(0.5)) <= 2 && p[2].abs_diff(tone(0.5)) <= 2 && p[1] == 0,
        "HDR blend must precede tone: {p:?}"
    );
    if let Some(path) = std::env::var_os("S2C_PRESENTATION_EVIDENCE") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        let mut e = png::Encoder::new(
            std::fs::File::create(path.join("particle-hdr-depth.png")).unwrap(),
            64,
            64,
        );
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&image).unwrap();
    }
    // UI stays in display composition, independent of scene depth and tone mapping.
    let mut ui_graph = g.clone();
    let vertices = [
        [-0.2, -0.2],
        [0.2, -0.2],
        [0.2, 0.2],
        [-0.2, -0.2],
        [0.2, 0.2],
        [-0.2, 0.2],
    ]
    .into_iter()
    .map(|p| RenderDrawVertex::new(p, [0., 1., 0., 1.], [0., 0.]))
    .collect::<Vec<_>>()
    .into();
    ui_graph.passes.push(RenderPass {
        pass_id: "ui".into(),
        pass_name: "ui".into(),
        pass_kind: RenderPassKind::DrawUiComposition,
        view_id: "overlay".into(),
        reads: vec![],
        writes: vec!["main".into()],
        color_targets: vec!["main".into()],
        depth_target: None,
        debug_source: None,
        commands: vec![RenderPassCommand::DrawUiComposition {
            target: "main".into(),
            stage: "ScreenOverlay".into(),
            item_count: 1,
            text_count: 0,
            image_count: 1,
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
    });
    assert_eq!(
        center(
            &b.render_plan_to_rgba_bytes(&compile(&ui_graph), 64, 64)
                .unwrap()
        ),
        [0, 255, 0, 255]
    );
    // Forged raw attachment must fail before advancing the effect clock.
    let mut bad = compile(&g);
    for c in &mut bad.commands {
        if let engine_runtime::rhi_command_plan::RhiCommand::Draw {
            payload: engine_runtime::rhi_command_plan::RhiDrawPayload::Particles { view, .. },
            ..
        } = c
        {
            view.depth_target = Some("wrong-depth".into());
        }
    }
    assert!(b
        .render_plan_to_rgba_bytes(&bad, 64, 64)
        .unwrap_err()
        .contains("particle_render.scene_attachment_mismatch"));
    b.shutdown_gpu(std::time::Duration::from_secs(5)).unwrap();
}
