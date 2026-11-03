use engine_runtime::engine_rhi::EngineRhiBackend;
use engine_runtime::render_asset_production::*;
use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_graph::{RenderPassCommand, RenderResourceFormat};
use engine_runtime::render_resource::*;
use engine_runtime::rhi_command_plan::{
    compile_render_graph_to_rhi_plan, RhiCommand, RhiDrawPayload,
};

#[path = "support/geometry_fixture.rs"]
mod geometry_fixture;
use geometry_fixture::*;

#[test]
fn immutable_draw_is_shared_from_frame_through_graph_and_rhi() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let graph = graph(&frame);
    let command = graph
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
        std::ptr::eq::<DrawGeometry>(frame.views[0].draws[0].as_ref(), command.as_ref()),
        "the immutable frame draw was allocated again for the graph"
    );
    let plan = compile_render_graph_to_rhi_plan(&graph);
    let compiled = plan
        .commands
        .iter()
        .find_map(|c| {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } = c
            {
                Some(draw)
            } else {
                None
            }
        })
        .unwrap();
    assert!(
        std::ptr::eq::<DrawGeometry>(command.as_ref(), compiled.as_ref()),
        "the checked graph draw was allocated again for the RHI plan"
    );
}

#[test]
fn immutable_draw_clones_share_prepared_binding_descriptors() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let draw = &frame.views[0].draws[0];
    for _ in 0..128 {
        let copy = draw.clone();
        assert_eq!(
            draw.geometry_binding.binding_id.as_ptr(),
            copy.geometry_binding.binding_id.as_ptr(),
            "frame/graph/RHI clone deep-copied the immutable geometry binding"
        );
        assert_eq!(
            draw.material_binding.binding_id.as_ptr(),
            copy.material_binding.binding_id.as_ptr(),
            "frame/graph/RHI clone deep-copied the immutable material binding"
        );
        assert_eq!(copy, *draw);
    }
    let decoded: DrawGeometry =
        serde_json::from_value(serde_json::to_value(draw).unwrap()).unwrap();
    assert_eq!(decoded, **draw);
    decoded.validate().unwrap();
}

#[test]
fn memoized_pipeline_preserves_current_execution_recipe_and_variants() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let template = &frame.views[0].draws[0];
    for flat in [false, true] {
        for opaque in [false, true] {
            for cull in [MaterialCull::None, MaterialCull::Front, MaterialCull::Back] {
                for format in ["Rgba8Unorm", "Bgra8Unorm"] {
                    for mirrored in [false, true] {
                        let mut draw = template.clone();
                        std::sync::Arc::make_mut(&mut draw).ignore_vertex_color = flat;
                        std::sync::Arc::make_mut(&mut draw).opaque_alpha = opaque;
                        std::sync::Arc::make_mut(&mut draw).cull = cull;
                        if mirrored {
                            std::sync::Arc::make_mut(&mut draw).world_transform[0] = (-1f32).into();
                        }
                        let mut expected = engine_runtime::canonical_digest::sha256_prefixed(
                            &serde_json::to_vec(&(
                                "draw-geometry.v10.layered-world-position",
                                (false, false, false, false, 0u8), // Unlit, no scene lights/color processing/shadows.
                                u8::from(
                                    draw.material_binding.texture("baseColorTexture").is_some(),
                                ),
                                UNLIT_SHADER_SOURCE,
                                UNLIT_GEOMETRY_SHADER_SOURCE,
                                draw.geometry.vertex_stride(),
                                draw.geometry.attributes(),
                                draw.geometry.index_format(),
                                format,
                                draw.depth_target.as_ref().map(|_| "depth32Float"),
                                draw.blend,
                                draw.cull,
                                draw.mirrored(),
                                draw.depth,
                                draw.view.domain,
                                draw.material_binding.texture("baseColorTexture").is_some(),
                            ))
                            .unwrap(),
                        );
                        if flat {
                            expected = engine_runtime::canonical_digest::sha256_prefixed(
                                format!("{expected}|unlit-flat").as_bytes(),
                            );
                        }
                        if opaque {
                            expected = engine_runtime::canonical_digest::sha256_prefixed(
                                format!("{expected}|opaque-alpha-one").as_bytes(),
                            );
                        }
                        assert_eq!(draw.pipeline_identity(format), expected);
                        assert_eq!(draw.pipeline_identity(format), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn geometry_draws_share_the_view_pass_without_losing_order_or_commands() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut frame = frame(&source, &manager);
    let template = frame.views[0].draws[0].clone();
    frame.views[0].draws = (1..=128)
        .map(|id| {
            let mut d = template.clone();
            std::sync::Arc::make_mut(&mut d).instance_id = id;
            d
        })
        .collect();
    let graph = graph(&frame);
    let passes: Vec<_> = graph
        .passes
        .iter()
        .filter(|p| p.pass_kind == engine_runtime::render_graph::RenderPassKind::DrawGeometry)
        .collect();
    assert_eq!(
        passes.len(),
        1,
        "draws in one view have identical attachment dependencies"
    );
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let ids: Vec<_> = plan
        .commands
        .iter()
        .filter_map(|c| match c {
            RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } => Some(draw.instance_id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, (1..=128).collect::<Vec<_>>());
}

#[test]
fn geometry_pass_attachment_checks_are_shared_but_each_draw_is_validated() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut frame = frame(&source, &manager);
    let template = frame.views[0].draws[0].clone();
    frame.views[0].draws = (1..=128)
        .map(|id| {
            let mut draw = template.clone();
            std::sync::Arc::make_mut(&mut draw).instance_id = id;
            draw
        })
        .collect();
    let mut graph = graph(&frame);
    let pass = graph
        .passes
        .iter_mut()
        .find(|p| p.pass_kind == engine_runtime::render_graph::RenderPassKind::DrawGeometry)
        .unwrap();
    pass.color_targets.clear();
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert_eq!(
        plan.diagnostics
            .iter()
            .filter(|d| d.code == "geometry.attachment_mismatch")
            .count(),
        1,
        "one invalid pass repeated the same attachment lookup per instance"
    );
    let pass = graph
        .passes
        .iter_mut()
        .find(|p| p.pass_kind == engine_runtime::render_graph::RenderPassKind::DrawGeometry)
        .unwrap();
    pass.color_targets = vec!["main".into()];
    for command in pass.commands.iter_mut().skip(1) {
        if let RenderPassCommand::DrawGeometry { draw } = command {
            std::sync::Arc::make_mut(draw).index_count = 1;
        }
    }
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert_eq!(
        plan.diagnostics
            .iter()
            .filter(|d| d.code == "geometry.invalid_draw")
            .count(),
        127,
        "shared attachment checks cannot bypass individual draw validation"
    );
    assert!(plan.has_errors());
    let pass = graph
        .passes
        .iter_mut()
        .find(|p| p.pass_kind == engine_runtime::render_graph::RenderPassKind::DrawGeometry)
        .unwrap();
    if let RenderPassCommand::DrawGeometry { draw } = pass.commands.last_mut().unwrap() {
        std::sync::Arc::make_mut(draw).target = "different-target".into();
    }
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert_eq!(
        plan.diagnostics
            .iter()
            .filter(|d| d.code == "geometry.attachment_mismatch")
            .count(),
        1,
        "attachment reuse cannot skip a later draw with a different target"
    );
}

#[test]
#[ignore = "run-owned development diagnosis of BM02 render stages; not Release qualification"]
fn bm02_render_stage_cost_diagnostic() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let view = RenderView::perspective(1, 1920, 1080, 60., 0.1, 100.);
    let items: Vec<_> = (1..=10000)
        .map(|id| instance(&source, id, [0., 0., -2.]))
        .collect();
    let start = std::time::Instant::now();
    let frame = GeometryFrame::build(
        "main",
        [1920, 1080],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &items,
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let built = start.elapsed();
    let start = std::time::Instant::now();
    for draw in &frame.views[0].draws {
        std::hint::black_box(draw.pipeline_identity("Rgba8Unorm"));
    }
    let pipeline_keys = start.elapsed();
    let start = std::time::Instant::now();
    for draw in &frame.views[0].draws {
        draw.validate().unwrap();
    }
    eprintln!(
        "BM02 development only: 10000 pipeline keys={pipeline_keys:?}, validations={:?}",
        start.elapsed()
    );
    let start = std::time::Instant::now();
    let graph = graph(&frame);
    let graphed = start.elapsed();
    let start = std::time::Instant::now();
    let plan = compile_render_graph_to_rhi_plan(&graph);
    let compiled = start.elapsed();
    let start = std::time::Instant::now();
    let report =
        engine_runtime::headless_rhi_backend::HeadlessRhiBackend::new().execute_plan(&plan);
    eprintln!("BM02 owner test/unoptimized: build={built:?},graph={graphed:?},compile={compiled:?},headless={:?},passes={},commands={},draws={}",start.elapsed(),graph.passes.len(),plan.commands.len(),report.draw_count);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    assert_eq!(report.draw_count, 10000);
    #[cfg(feature = "real-wgpu")]
    {
        let mut backend =
            engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(1920, 1080).unwrap();
        for iteration in 0..3 {
            let start = std::time::Instant::now();
            backend
                .render_plan_to_rgba_bytes(&plan, 1920, 1080)
                .unwrap();
            eprintln!(
                "BM02 development small triangle GPU consumer {iteration}: {:?}",
                start.elapsed()
            );
        }
    }
}

#[test]
fn immutable_geometry_hash_work_does_not_scale_with_pixel_or_vertex_bytes() {
    use std::hash::{Hash, Hasher};
    #[derive(Default)]
    struct Bytes(usize);
    impl Hasher for Bytes {
        fn finish(&self) -> u64 {
            self.0 as u64
        }
        fn write(&mut self, bytes: &[u8]) {
            self.0 += bytes.len();
        }
    }
    let mut cooked = triangle();
    // Same valid triangle repeated; index content and bounds remain valid.
    let vertices = cooked.vertex_bytes.clone();
    cooked.vertex_bytes = vertices.repeat(1000);
    cooked.vertex_count *= 1000;
    let geometry = GeometryBuffers::from_cooked(&cooked).unwrap();
    let mut bytes = Bytes::default();
    geometry.hash(&mut bytes);
    assert!(
        bytes.0 <= 64,
        "immutable prepared geometry rehashed {} bytes",
        bytes.0
    );
    let copy: GeometryBuffers =
        serde_json::from_value(serde_json::to_value(&geometry).unwrap()).unwrap();
    assert_eq!(geometry, copy);
    let hash = |g: &GeometryBuffers| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        g.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(&geometry), hash(&copy));
    cooked.vertex_bytes[0..4].copy_from_slice(&(-0.4f32).to_le_bytes());
    let changed = GeometryBuffers::from_cooked(&cooked).unwrap();
    assert_ne!(geometry, changed);
    assert_ne!(hash(&geometry), hash(&changed));
}

#[test]
fn camera_perspective_depth_endpoints_and_aspect() {
    let v = RenderView::perspective(1, 200, 100, 90., 1., 11.)
        .evaluate(CameraPose::default())
        .unwrap();
    let m = v.view_projection();
    let near = transform_point(m, [0., 0., -1., 1.]);
    let far = transform_point(m, [0., 0., -11., 1.]);
    assert!(near[2].abs() < 1e-6);
    assert!((far[2] / far[3] - 1.).abs() < 1e-6);
    let right = transform_point(m, [2., 0., -1., 1.]);
    assert!((right[0] / right[3] - 1.).abs() < 1e-6);
    assert!(transform_point(m, [0., 0., 1., 1.])[3] < 0.);
}

#[test]
fn camera_orthographic_pose_and_rotation_are_executable() {
    let mut view = RenderView::orthographic(2, 100, 100, 4., 0., 10.);
    view.source_camera_entity_id = Some("camera".into());
    view.version = 7;
    let v = view
        .evaluate(CameraPose {
            position: [2., 1., 0.],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(v.source_camera_entity_id, view.source_camera_entity_id);
    assert_eq!((v.view_kind, v.version), (view.view_kind, 7));
    let center = transform_point(v.view_projection(), [2., 1., -5., 1.]);
    assert_eq!(center, [0., 0., 0.5, 1.]);
    view.domain = RenderDomain::World3d;
    view.coordinate_convention = CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ;
    let rotated = view
        .evaluate(CameraPose {
            position: [0.; 3],
            rotation: [0., 1., 0., 0.],
        })
        .unwrap();
    assert!((transform_point(rotated.view_projection(), [0., 0., 5., 1.])[2] - 0.5).abs() < 1e-6);
}

#[test]
fn camera_rejects_nonunit_pose_overflow_and_invalid_viewport() {
    let mut view = RenderView::perspective(1, 20, 20, 60., 0.1, 20.);
    view.target = TargetKind::ShadowMap;
    assert_eq!(view.execution_support(), ExecutionSupport::ContractOnly);
    assert!(view.evaluate(CameraPose::default()).is_err());
    view.target = TargetKind::Window;
    assert!(view
        .evaluate(CameraPose {
            rotation: [0.; 4],
            ..Default::default()
        })
        .is_err());
    assert!(view
        .evaluate(CameraPose {
            position: [f32::NAN, 0., 0.],
            ..Default::default()
        })
        .is_err());
    view.projection = Projection::Orthographic {
        size: f32::MIN_POSITIVE,
        near: 1.,
        far: 1.0000001,
    };
    view.viewport = [0, 0, 1, u32::MAX];
    assert!(view.evaluate(CameraPose::default()).is_err());
    view.viewport = [0, 0, 0, 1];
    assert!(view.evaluate(CameraPose::default()).is_err());
}

#[test]
fn culling_is_per_view_and_reports_layers_hidden_and_frustum() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let a = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let mut b = a.clone();
    b.view_id = 2;
    let views = [
        GeometryViewInput {
            view: &a,
            pose: CameraPose::default(),
        },
        GeometryViewInput {
            view: &b,
            pose: CameraPose {
                position: [20., 0., 0.],
                ..Default::default()
            },
        },
    ];
    let mut hidden = instance(&source, 3, [0., 0., -2.]);
    hidden.visibility.authored = false;
    let items = [
        instance(&source, 1, [0., 0., -2.]),
        instance(&source, 2, [20., 0., -2.]),
        hidden,
    ];
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
        &views,
        &items,
        &manager,
        DiagnosticLevel::Trace,
    )
    .unwrap();
    assert_eq!(frame.views[0].draws[0].instance_id, 1);
    assert_eq!(frame.views[1].draws[0].instance_id, 2);
    assert_eq!(frame.views[0].summary.as_ref().unwrap().frustum_excluded, 1);
    assert_eq!(frame.views[0].summary.as_ref().unwrap().hidden, 1);
    assert_eq!(
        frame.views[0].trace[1].culling,
        CullingDecision::FrustumExcluded
    );
    let off = GeometryFrame::build(
        "main",
        [64, 64],
        &views,
        &items,
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    assert!(off.views[0].summary.is_none());
    assert!(off.views[0].trace.is_empty());
    b.layers = LayerSelection::Named {
        names: vec!["other".into()],
    };
    let layers = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &b,
            pose: CameraPose::default(),
        }],
        &items,
        &manager,
        DiagnosticLevel::Summary,
    )
    .unwrap();
    assert_eq!(layers.views[0].summary.as_ref().unwrap().layer_excluded, 2);
}

#[test]
fn culling_keeps_near_plane_intersections_and_rejects_unevaluated_visibility() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let mut item = instance(&source, 1, [0., 0., -0.1]);
    let views = [GeometryViewInput {
        view: &view,
        pose: CameraPose::default(),
    }];
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
        &views,
        &[item],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    assert_eq!(frame.views[0].draws.len(), 1);
    item = instance(&source, 1, [0., 0., -2.]);
    item.visibility.inherited = Evaluation::NotEvaluated;
    let error = GeometryFrame::build(
        "main",
        [64, 64],
        &views,
        &[item],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap_err();
    assert_eq!(
        (error.view_id, error.instance_id, error.error.field),
        (Some(1), Some(1), "visibility.inherited")
    );
}

#[test]
fn graph_neutral_commands_preserve_bytes_bindings_transform_and_depth() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let graph = graph(&frame);
    let plan = compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let copy: engine_runtime::rhi_command_plan::RhiCommandPlan =
        serde_json::from_str(&serde_json::to_string(&plan).unwrap()).unwrap();
    assert_eq!(copy, plan);
    let draw = plan
        .commands
        .iter()
        .find_map(|c| match c {
            RhiCommand::Draw {
                payload: RhiDrawPayload::Geometry { draw },
                ..
            } => Some(draw),
            _ => None,
        })
        .unwrap();
    assert_eq!(draw.geometry.vertex_bytes(), triangle().vertex_bytes);
    assert_eq!(draw.geometry_source.asset_id.as_deref(), Some("mesh"));
    assert_eq!(
        draw.material_source.source_path.as_deref(),
        Some("assets/material.asset")
    );
    assert!(is_sha256_digest(
        draw.geometry_source.content_digest.as_deref().unwrap()
    ));
    assert_ne!(
        draw.geometry_source.content_digest,
        draw.material_source.content_digest
    );
    assert_eq!(draw.geometry.index_bytes(), &[0, 0, 1, 0, 2, 0]);
    assert_eq!(draw.world_transform[14].to_f32(), -2.);
    assert_eq!(draw.depth_target.as_deref(), Some("main:depth:view-1"));
    let report =
        engine_runtime::headless_rhi_backend::HeadlessRhiBackend::new().execute_plan(&plan);
    assert_eq!(report.draw_count, 1);
    assert_eq!(report.binding_count, 2);
    assert_eq!(report.uploaded_resource_count, 0);
    assert_eq!(
        report.target_hash,
        engine_runtime::headless_rhi_backend::HeadlessRhiBackend::new()
            .execute_plan(&copy)
            .target_hash
    );
    let mut moved = frame.clone();
    std::sync::Arc::make_mut(&mut moved.views[0].draws[0]).world_transform[12] = 0.2.into();
    let moved_plan = compile_render_graph_to_rhi_plan(&self::graph(&moved));
    assert_ne!(
        report.target_hash,
        engine_runtime::headless_rhi_backend::HeadlessRhiBackend::new()
            .execute_plan(&moved_plan)
            .target_hash
    );
}

#[test]
fn graph_rejects_missing_wrong_size_format_and_view_depth_before_draws() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    for case in 0..5 {
        let mut g = graph(&frame);
        match case {
            0 => {
                g.resources.pop();
            }
            1 => g.resources[1].format = RenderResourceFormat::Rgba8Unorm,
            2 => g.resources[1].size.width = 1,
            3 => {
                if let RenderPassCommand::DrawGeometry { draw } = &mut g.passes[2].commands[0] {
                    std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(draw).view).view_id = 99;
                }
            }
            _ => {
                if let RenderPassCommand::BeginView { view, .. } = &mut g.passes[1].commands[0] {
                    std::sync::Arc::make_mut(view).viewport = [63, 0, 2, 64];
                }
            }
        }
        let plan = compile_render_graph_to_rhi_plan(&g);
        assert!(plan.has_errors(), "case {case}");
        assert!(!plan
            .commands
            .iter()
            .any(|c| matches!(c, RhiCommand::Draw { .. })));
    }
}

#[test]
fn converter_rejects_stale_deferred_wrong_kind_and_missing_legacy_data() {
    let mut manager = RenderResourceManager::new();
    let mesh = prepare("m", RenderAssetData::Mesh(triangle()), &mut manager);
    let mut mat = prepare("a", RenderAssetData::Material(unlit([1.; 4])), &mut manager);
    assert!(PreparedGeometry::from_assets(&mat, &mesh, 0, &manager).is_err());
    assert!(PreparedGeometry::from_assets(&mesh, &mat, 9, &manager).is_err());
    mat.state = RenderAssetPreparationState::Deferred;
    assert_eq!(
        PreparedGeometry::from_assets(&mesh, &mat, 0, &manager)
            .unwrap_err()
            .code,
        ContractErrorCode::ResourceNotReady
    );
    mat.state = RenderAssetPreparationState::Prepared;
    let command = RenderPassCommand::DrawMeshBasic {
        target: "main".into(),
        mesh_ref: "m".into(),
        material_ref: Some("a".into()),
    };
    assert!(PreparedGeometry::from_legacy_mesh(&command, &mesh, &mat, &manager).is_ok());
    let wrong = RenderPassCommand::DrawMeshBasic {
        target: "main".into(),
        mesh_ref: "unknown".into(),
        material_ref: Some("a".into()),
    };
    assert!(PreparedGeometry::from_legacy_mesh(&wrong, &mesh, &mat, &manager).is_err());
    let src = PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).unwrap();
    let mut replacement = mesh.resource_request.clone();
    replacement.key.asset_version = "2".into();
    manager.request_resource(1, replacement);
    assert!(PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).is_err());
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    assert!(GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default()
        }],
        &[instance(&src, 1, [0., 0., -2.])],
        &manager,
        DiagnosticLevel::Off
    )
    .is_err());
}

#[test]
fn sorting_uses_camera_depth_transparency_and_stable_identity() {
    let mut manager = RenderResourceManager::new();
    let mesh = prepare("m", RenderAssetData::Mesh(triangle()), &mut manager);
    let opaque = prepare("o", RenderAssetData::Material(unlit([1.; 4])), &mut manager);
    let mut alpha = unlit([1., 0., 0., 0.5]);
    alpha.blend = MaterialBlend::AlphaBlend;
    alpha.depth = MaterialDepth::TestOnly;
    let alpha = prepare("a", RenderAssetData::Material(alpha), &mut manager);
    let o = PreparedGeometry::from_assets(&mesh, &opaque, 0, &manager).unwrap();
    let a = PreparedGeometry::from_assets(&mesh, &alpha, 0, &manager).unwrap();
    let items = [
        instance(&a, 4, [0., 0., -2.]),
        instance(&o, 3, [0., 0., -3.]),
        instance(&a, 1, [0., 0., -4.]),
        instance(&o, 2, [0., 0., -3.]),
    ];
    let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &items,
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    assert_eq!(
        frame.views[0]
            .draws
            .iter()
            .map(|d| d.instance_id)
            .collect::<Vec<_>>(),
        [2, 3, 1, 4]
    );
    let draw = &frame.views[0].draws[0];
    let key = draw.pipeline_identity("Rgba8Unorm");
    let mut changed = draw.clone();
    std::sync::Arc::make_mut(&mut changed).base_color = [0.5.into(); 4];
    assert_eq!(key, changed.pipeline_identity("Rgba8Unorm"));
    std::sync::Arc::make_mut(&mut changed).cull = MaterialCull::Back;
    assert_ne!(key, changed.pipeline_identity("Rgba8Unorm"));
    assert_ne!(key, draw.pipeline_identity("Bgra8Unorm"));
}

#[test]
fn typed_runtime_entry_preserves_all_aui_composition_boundaries() {
    use engine_runtime::aui::*;
    use engine_runtime::runtime_renderer::*;
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let mut canvases = Vec::new();
    let mut nodes = Vec::new();
    for (name, stage, rect, color) in [
        (
            "before",
            AuiCompositionStage::BeforeWorld,
            AuiRect::stretch_full(),
            "#00ff00",
        ),
        (
            "overlay",
            AuiCompositionStage::ScreenOverlay,
            AuiRect::fixed_position(30., 30., 12., 12.),
            "#0000ff",
        ),
        (
            "modal",
            AuiCompositionStage::Modal,
            AuiRect::fixed_position(35., 35., 5., 5.),
            "#ffffff",
        ),
    ] {
        let mut canvas = AuiCanvas::screen_overlay(name, 64., 64., name);
        canvas.composition_stage = stage;
        canvases.push(canvas);
        let mut node = AuiNode::new(name, AuiNodeKind::Panel, rect);
        node.style = Some(AuiStyle::color(color));
        nodes.push(node);
    }
    let document = AuiDocument::new("u2-composition", canvases, nodes);
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let composition = AuiRendererBridge::build_composition_frame(1, &document, &layout, &list);
    use engine_runtime::components::{Renderable, Transform};
    use engine_runtime::ids::{RuntimeEntityId, SourceEntityId};
    use engine_runtime::math::Vec3;
    use engine_runtime::render_state::*;
    let mut scene = RenderSceneState::new();
    let mut sprite = RenderProxy::new(
        RenderProxyId(0),
        RuntimeEntityId::new(2, 0),
        SourceEntityId::from("legacy-sprite"),
        Transform {
            local_position: Vec3 {
                x: -5.,
                y: 0.,
                z: 0.,
            },
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        },
        Renderable {
            mesh_ref: None,
            material_ref: None,
            visible: true,
            layer: "default".into(),
        },
    );
    sprite.payload = RenderProxyPayload::Sprite(SpritePayload {
        sprite_ref: Some("a41-sprite".into()),
        material_ref: None,
        color: [1., 1., 0., 1.],
        flip_x: false,
        flip_y: false,
        sorting_layer: 0,
        order_in_layer: 0,
        sort_z: 0.,
    });
    scene.insert_proxy(sprite);
    let output = RuntimeRenderer::new().build_with_geometry(
        RuntimeRendererInput {
            frame_index: 1,
            render_scene_state: &scene,
            render_view_state: None,
            aui_overlay: None,
            aui_composition: Some(&composition),
            sprite_texture_bindings: None,
            runtime_texture_bindings: None,
            game_view_presentation: None,
            quality_profile: QualityProfile::default(),
            render_target: RenderTarget::headless_texture("main", 64, 64),
        },
        Some(&frame),
    );
    assert!(
        !output.rhi_command_plan.has_errors(),
        "{:?}",
        output.rhi_command_plan.diagnostics
    );
    let ids = output
        .render_graph
        .passes
        .iter()
        .map(|p| p.pass_id.as_str())
        .collect::<Vec<_>>();
    let positions = [
        "geometry-view-1-begin",
        "draw-aui-before-world",
        "draw-sprite2d-1",
        "geometry-view-1-draws",
        "draw-aui-screen-overlay",
        "draw-aui-modal",
        "present-main",
    ]
    .map(|id| ids.iter().position(|s| s == &id).unwrap());
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(output.render_frame_report.draw_item_count, 5);
    #[cfg(feature = "real-wgpu")]
    {
        let mut gpu =
            engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let pixels = gpu
            .render_plan_to_rgba_bytes(&output.rhi_command_plan, 64, 64)
            .unwrap();
        let pixel = |x: usize, y: usize| &pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(pixel(4, 4), [0, 255, 0, 255], "BeforeWorld outside mesh");
        assert_eq!(
            pixel(10, 32),
            [255, 255, 0, 255],
            "real Sprite2D pipeline shares the output"
        );
        assert_eq!(
            pixel(32, 24),
            [255, 0, 0, 255],
            "native world over BeforeWorld"
        );
        assert_eq!(
            pixel(32, 32),
            [0, 0, 255, 255],
            "ScreenOverlay over native world"
        );
        assert_eq!(
            pixel(37, 37),
            [255, 255, 255, 255],
            "Modal over ScreenOverlay"
        );
    }

    assert!(output
        .render_graph
        .passes
        .iter()
        .filter(|p| p.pass_id.starts_with("draw-aui"))
        .all(|p| p.depth_target.is_none()));
}

#[test]
fn geometry_wire_and_material_required_slots_fail_closed() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let frame = frame(&source, &manager);
    let draw = &frame.views[0].draws[0];
    let mut wire = serde_json::to_value(&draw.geometry).unwrap();
    wire["indexBytes"][0] = serde_json::json!(99);
    assert!(serde_json::from_value::<GeometryBuffers>(wire).is_err());
    let mut wire = serde_json::to_value(&draw.geometry).unwrap();
    wire["vertexStride"] = serde_json::json!(0);
    assert!(serde_json::from_value::<GeometryBuffers>(wire).is_err());
    let mesh = prepare("other-m", RenderAssetData::Mesh(triangle()), &mut manager);
    let mut mat = prepare(
        "other-a",
        RenderAssetData::Material(unlit([1.; 4])),
        &mut manager,
    );
    let tex = manager.request_resource(0, RenderResourceRequest::texture("tex", "1", 4));
    mat.binding
        .slots
        .push(NamedRenderBinding::new("baseColorTexture", tex));
    assert!(
        PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).is_err(),
        "extra texture must not silently change the material"
    );
}

#[test]
fn culling_2d_and_3d_views_keep_separate_domain_reasons() {
    let mut manager = RenderResourceManager::new();
    let world3 = source(&mut manager);
    let mut m = triangle();
    m.coordinate_convention = CoordinateConvention::Legacy2d;
    let mesh = prepare("mesh2d", RenderAssetData::Mesh(m), &mut manager);
    let mut material = unlit([1.; 4]);
    material.depth = MaterialDepth::Disabled;
    let mat = prepare("mat2d", RenderAssetData::Material(material), &mut manager);
    let world2 = PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).unwrap();
    let a = RenderView::orthographic(1, 64, 64, 4., 0.1, 10.);
    let b = RenderView::perspective(2, 64, 64, 60., 0.1, 10.);
    let frame = GeometryFrame::build(
        "main",
        [64, 64],
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
        &[
            instance(&world2, 1, [0., 0., -2.]),
            instance(&world3, 2, [0., 0., -2.]),
        ],
        &manager,
        DiagnosticLevel::Trace,
    )
    .unwrap();
    assert_eq!(frame.views[0].draws[0].instance_id, 1);
    assert_eq!(frame.views[1].draws[0].instance_id, 2);
    assert_eq!(
        frame.views[0].trace[1].culling,
        CullingDecision::DomainExcluded
    );
    assert_eq!(frame.views[1].summary.as_ref().unwrap().domain_excluded, 1);
}

#[cfg(feature = "real-wgpu")]
mod gpu {
    use super::*;
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;

    fn backend() -> RealWgpuBackend {
        let backend = RealWgpuBackend::new_offscreen(64, 64)
            .expect("U2 GPU qualification requires a real adapter; do not skip");
        eprintln!("U2 GPU context: {:?}", backend.device_context());
        backend
    }
    fn pixel(bytes: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        bytes[(y * width + x) * 4..(y * width + x) * 4 + 4]
            .try_into()
            .unwrap()
    }
    fn read(backend: &mut RealWgpuBackend, frame: &GeometryFrame) -> Vec<u8> {
        let plan = compile_render_graph_to_rhi_plan(&graph(frame));
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        backend
            .render_plan_to_rgba_bytes(&plan, frame.extent[0], frame.extent[1])
            .unwrap()
    }

    #[test]
    fn gpu_indexed_geometry_depth_camera_and_buffer_reuse_have_pixel_oracles() {
        let mut manager = RenderResourceManager::new();
        let mesh = prepare("m", RenderAssetData::Mesh(triangle()), &mut manager);
        let red = prepare(
            "r",
            RenderAssetData::Material(unlit([1., 0., 0., 1.])),
            &mut manager,
        );
        let green = prepare(
            "g",
            RenderAssetData::Material(unlit([0., 1., 0., 1.])),
            &mut manager,
        );
        let red = PreparedGeometry::from_assets(&mesh, &red, 0, &manager).unwrap();
        let green = PreparedGeometry::from_assets(&mesh, &green, 0, &manager).unwrap();
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        let items = [
            instance(&green, 2, [0., 0., -3.]),
            instance(&red, 1, [0., 0., -2.]),
        ];
        let frame = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &items,
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        let mut backend = backend();
        let pixels = read(&mut backend, &frame);
        assert_eq!(pixel(&pixels, 64, 32, 32), [255, 0, 0, 255]);
        assert_eq!(backend.geometry_upload_count(), 1);
        assert_eq!(read(&mut backend, &frame), pixels);
        assert_eq!(
            backend.geometry_upload_count(),
            1,
            "same mesh must not upload per instance/frame"
        );
        // Depth-disabled painter ordering must be selected before phase bucketing.
        // Mutating depth after GeometryFrame::build would retain opaque bucket order.
        let mut red_no_depth = unlit([1., 0., 0., 1.]);
        red_no_depth.depth = MaterialDepth::Disabled;
        let mut green_no_depth = unlit([0., 1., 0., 1.]);
        green_no_depth.depth = MaterialDepth::Disabled;
        let red_no_depth = prepare(
            "r-no-depth",
            RenderAssetData::Material(red_no_depth),
            &mut manager,
        );
        let green_no_depth = prepare(
            "g-no-depth",
            RenderAssetData::Material(green_no_depth),
            &mut manager,
        );
        let red_no_depth =
            PreparedGeometry::from_assets(&mesh, &red_no_depth, 0, &manager).unwrap();
        let green_no_depth =
            PreparedGeometry::from_assets(&mesh, &green_no_depth, 0, &manager).unwrap();
        let no_depth_frame = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &[
                instance(&green_no_depth, 2, [0., 0., -3.]),
                instance(&red_no_depth, 1, [0., 0., -2.]),
            ],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        let no_depth = read(&mut backend, &no_depth_frame);
        assert_eq!(
            pixel(&no_depth, 64, 32, 32),
            [0, 255, 0, 255],
            "far draw must overwrite without depth"
        );
        let moved = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose {
                    position: [2., 0., 0.],
                    ..Default::default()
                },
            }],
            &items,
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        assert_eq!(
            pixel(&read(&mut backend, &moved), 64, 32, 32),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn a41_projection_size_depth_swap_and_clip_have_pixel_oracles() {
        let mut manager = RenderResourceManager::new();
        let mesh = prepare("a41-m", RenderAssetData::Mesh(triangle()), &mut manager);
        let red = prepare(
            "a41-r",
            RenderAssetData::Material(unlit([1., 0., 0., 1.])),
            &mut manager,
        );
        let green = prepare(
            "a41-g",
            RenderAssetData::Material(unlit([0., 1., 0., 1.])),
            &mut manager,
        );
        let red = PreparedGeometry::from_assets(&mesh, &red, 0, &manager).unwrap();
        let green = PreparedGeometry::from_assets(&mesh, &green, 0, &manager).unwrap();
        let mut gpu = backend();
        let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        let render =
            |gpu: &mut RealWgpuBackend, view: &RenderView, items: &[GeometryInstance<'_>]| {
                read(
                    gpu,
                    &GeometryFrame::build(
                        "main",
                        [64, 64],
                        &[GeometryViewInput {
                            view,
                            pose: CameraPose::default(),
                        }],
                        items,
                        &manager,
                        DiagnosticLevel::Summary,
                    )
                    .unwrap(),
                )
            };
        for orthographic in [false, true] {
            if orthographic {
                view.projection = Projection::Orthographic {
                    size: 4.,
                    near: 0.1,
                    far: 10.,
                };
            }
            for z in [2f32, 4.] {
                let pixels = render(&mut gpu, &view, &[instance(&red, 1, [0., 0., -z])]);
                let ys: Vec<_> = (0..64)
                    .filter(|y| pixel(&pixels, 64, 32, *y)[0] > 250)
                    .collect();
                let height = ys.last().unwrap() - ys.first().unwrap() + 1;
                let half_height = if orthographic {
                    0.8 / 4. * 64.
                } else {
                    0.8 / (z * 30f32.to_radians().tan()) * 32.
                };
                // Column 32 samples x=32.5. This triangle's upper edge falls by
                // two pixels per x pixel, so its sampled tip is one pixel below
                // the geometric apex. Compare each raster edge, not summed error.
                let expected_first = (32. - half_height + 1. - 0.5).ceil() as usize;
                let expected_last = (32. + half_height - 0.5).ceil() as usize - 1;
                assert!(
                    ys[0].abs_diff(expected_first) <= 1,
                    "top {:?}, expected {expected_first}, z {z}",
                    ys.first()
                );
                assert!(
                    ys.last().unwrap().abs_diff(expected_last) <= 1,
                    "bottom {:?}, expected {expected_last}, z {z}",
                    ys.last()
                );
                assert!(height > 0);
            }
        }
        view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        for (red_z, green_z, expected) in
            [(-2., -3., [255, 0, 0, 255]), (-3., -2., [0, 255, 0, 255])]
        {
            let pixels = render(
                &mut gpu,
                &view,
                &[
                    instance(&red, 1, [0., 0., red_z]),
                    instance(&green, 2, [0., 0., green_z]),
                ],
            );
            assert_eq!(pixel(&pixels, 64, 32, 32), expected);
        }
        for z in [-0.05, -11.] {
            let pixels = render(&mut gpu, &view, &[instance(&red, 1, [0., 0., z])]);
            assert!(pixels.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
        }
        let mut invisible = instance(&red, 1, [0., 0., -2.]);
        invisible.visibility.authored = false;
        assert_eq!(
            pixel(&render(&mut gpu, &view, &[invisible]), 64, 32, 32),
            [0, 0, 0, 255]
        );
        let mut excluded = instance(&red, 1, [0., 0., -2.]);
        excluded.layers = LayerSelection::Named {
            names: vec!["excluded".into()],
        };
        view.layers = LayerSelection::Named {
            names: vec!["main".into()],
        };
        assert_eq!(
            pixel(&render(&mut gpu, &view, &[excluded]), 64, 32, 32),
            [0, 0, 0, 255]
        );
    }

    #[test]
    fn gpu_views_clear_only_their_viewports_and_resize_depth_attachments() {
        let manager = RenderResourceManager::new();
        let mut a = RenderView::perspective(1, 24, 64, 60., 0.1, 10.);
        a.clear_policy = ClearPolicy::Color {
            color: [1., 0., 0., 1.],
        };
        let mut b = a.clone();
        b.view_id = 2;
        b.viewport = [40, 0, 24, 64];
        b.clear_policy = ClearPolicy::Color {
            color: [0., 0., 1., 0.],
        };
        let frame = GeometryFrame::build(
            "main",
            [64, 64],
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
            &[],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        let mut backend = backend();
        let pixels = read(&mut backend, &frame);
        assert_eq!(pixel(&pixels, 64, 12, 32), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 64, 32, 32), [0, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 64, 52, 32), [0, 0, 255, 0]);
        a.viewport = [0, 0, 16, 32];
        b.viewport = [16, 0, 16, 32];
        let frame = GeometryFrame::build(
            "main",
            [32, 32],
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
            &[],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        let pixels = read(&mut backend, &frame);
        assert_eq!(pixel(&pixels, 32, 8, 16), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 32, 24, 16), [0, 0, 255, 0]);
    }

    #[test]
    fn gpu_named_texture_cull_u32_indices_and_missing_upload_are_honest() {
        let mut manager = RenderResourceManager::new();
        let mut m = triangle();
        m.index_format = GeometryIndexFormat::Uint32;
        m.index_bytes = [0u32, 1, 2]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let mesh = prepare("m32", RenderAssetData::Mesh(m), &mut manager);
        let mut mat = prepare(
            "white",
            RenderAssetData::Material(unlit([1.; 4])),
            &mut manager,
        );
        let texture = manager.request_resource(0, RenderResourceRequest::texture("tex", "1", 4));
        mat.binding
            .slots
            .push(NamedRenderBinding::new("baseColorTexture", texture));
        let RenderAssetData::Material(params) = &mut mat.asset.asset else {
            unreachable!()
        };
        params.textures.insert(
            "baseColorTexture".into(),
            RenderAssetReference {
                expected_kind: RenderAssetKind::Texture,
                reference: engine_runtime::runtime_package::RuntimeAssetRef {
                    id: "tex".into(),
                    asset_type: "texture".into(),
                    guid: Some("tex".into()),
                    sub_asset: None,
                },
            },
        );
        let source = PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).unwrap();
        let mut frame = frame(&source, &manager);
        let mut backend = backend();
        let plan = compile_render_graph_to_rhi_plan(&graph(&frame));
        assert!(backend
            .render_plan_to_rgba_bytes(&plan, 64, 64)
            .unwrap_err()
            .contains("texture"));
        assert_eq!(backend.geometry_upload_count(), 0);
        backend
            .register_rgba8_texture(texture, 1, 1, &[0, 0, 255, 255], "nearestClamp")
            .unwrap();
        assert_eq!(
            pixel(&read(&mut backend, &frame), 64, 32, 32),
            [0, 0, 255, 255]
        );
        // Replacing a native binding under the same logical handle must drop
        // recorded bundles, even though the immutable draw plan still matches.
        assert_eq!(
            pixel(&read(&mut backend, &frame), 64, 32, 32),
            [0, 0, 255, 255]
        );
        backend
            .register_rgba8_texture(texture, 1, 1, &[255, 0, 0, 255], "nearestClamp")
            .unwrap();
        assert_eq!(
            pixel(&read(&mut backend, &frame), 64, 32, 32),
            [255, 0, 0, 255]
        );
        backend
            .register_rgba8_texture(texture, 1, 1, &[0, 0, 255, 255], "nearestClamp")
            .unwrap();
        std::sync::Arc::make_mut(&mut frame.views[0].draws[0]).cull = MaterialCull::Front;
        assert_eq!(
            pixel(&read(&mut backend, &frame), 64, 32, 32),
            [0, 0, 0, 255]
        );
        std::sync::Arc::make_mut(&mut frame.views[0].draws[0]).cull = MaterialCull::Back;
        assert_eq!(
            pixel(&read(&mut backend, &frame), 64, 32, 32),
            [0, 0, 255, 255]
        );
    }

    #[test]
    fn gpu_alpha_test_only_and_legacy_ui_remain_ordered() {
        let mut manager = RenderResourceManager::new();
        let mesh = prepare("m", RenderAssetData::Mesh(triangle()), &mut manager);
        let opaque = prepare(
            "g",
            RenderAssetData::Material(unlit([0., 1., 0., 1.])),
            &mut manager,
        );
        let mut alpha = unlit([1., 0., 0., 0.5]);
        alpha.blend = MaterialBlend::AlphaBlend;
        alpha.depth = MaterialDepth::TestOnly;
        let alpha = prepare("a", RenderAssetData::Material(alpha), &mut manager);
        let o = PreparedGeometry::from_assets(&mesh, &opaque, 0, &manager).unwrap();
        let a = PreparedGeometry::from_assets(&mesh, &alpha, 0, &manager).unwrap();
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        let frame = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &[
                instance(&o, 1, [0., 0., -3.]),
                instance(&a, 2, [0., 0., -2.]),
            ],
            &manager,
            DiagnosticLevel::Off,
        )
        .unwrap();
        let mut backend = backend();
        let center = pixel(&read(&mut backend, &frame), 64, 32, 32);
        assert!(
            (127..=128).contains(&center[0]) && (127..=128).contains(&center[1]) && center[2] == 0,
            "{center:?}"
        );
        let mut g = graph(&frame);
        let vertices = [[-0.2, -0.2], [0.2, -0.2], [0., 0.2]]
            .into_iter()
            .map(|p| {
                engine_runtime::render_graph::RenderDrawVertex::new(p, [0., 0., 1., 1.], [0.; 2])
            })
            .collect::<Vec<_>>()
            .into();
        g.passes.push(engine_runtime::render_graph::RenderPass {
            pass_id: "ui".into(),
            pass_name: "ui".into(),
            pass_kind: engine_runtime::render_graph::RenderPassKind::DrawUiComposition,
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
        let plan = compile_render_graph_to_rhi_plan(&g);
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(pixel(&pixels, 64, 32, 32), [0, 0, 255, 255]);
    }

    #[test]
    fn gpu_load_view_preserves_existing_target_without_an_implicit_clear() {
        let adapter = pollster::block_on(
            wgpu::Instance::new(&Default::default()).request_adapter(&Default::default()),
        )
        .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut backend = RealWgpuBackend::from_shared_device_queue(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            64,
            64,
            "u2-load-test",
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("u2-load-target"),
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
        });
        let target = texture.create_view(&Default::default());
        let manager = RenderResourceManager::new();
        let mut view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
        view.clear_policy = ClearPolicy::Color {
            color: [1., 0., 0., 1.],
        };
        for load in [false, true] {
            if load {
                view.clear_policy = ClearPolicy::Load;
            }
            let frame = GeometryFrame::build(
                "main",
                [64, 64],
                &[GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                }],
                &[],
                &manager,
                DiagnosticLevel::Off,
            )
            .unwrap();
            let mut g = graph(&frame);
            g.passes.remove(0);
            let plan = compile_render_graph_to_rhi_plan(&g);
            let report = backend.execute_plan_to_surface_view(&plan, &target);
            assert!(
                report.diagnostics.iter().all(|d| d.severity
                    != engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity::Error),
                "{:?}",
                report.diagnostics
            );
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("u2-load-readback"),
            size: 64 * 64 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
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
        queue.submit(Some(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            tx.send(r).unwrap();
        });
        device.poll(wgpu::PollType::wait()).unwrap();
        rx.recv().unwrap().unwrap();
        assert_eq!(
            pixel(&buffer.slice(..).get_mapped_range(), 64, 32, 32),
            [255, 0, 0, 255]
        );
        buffer.unmap();
    }

    #[test]
    fn gpu_native_geometry_and_existing_particle_simulation_share_the_ordered_plan() {
        use engine_runtime::particle_effect::*;
        use engine_runtime::particle_render_contract::*;
        use engine_runtime::render_graph::*;
        let mut desc: ParticleEffectDescription =
            serde_json::from_str(include_str!("fixtures/particle-effect.json")).unwrap();
        desc.emitters.truncate(1);
        let e = &mut desc.emitters[0];
        e.capacity = 4;
        e.bursts[0].count = 1;
        e.velocity_min = [0.; 3];
        e.velocity_max = [0.; 3];
        e.lifetime_seconds = [1.; 2];
        e.color = [0., 0., 1., 1.];
        e.size_meters = [0.2; 2];
        e.updates = vec![ParticleUpdate::Integrate {}];
        e.draw.geometry = ParticleGeometry::Billboard3d {};
        desc.validate().unwrap();
        let programs = desc
            .emitters
            .iter()
            .map(|e| CookedParticleProgram {
                emitter: e.name.clone(),
                wgsl: program::compute_source(
                    &desc,
                    e,
                    &(program::prefix(&desc, e)
                        + program::DEFAULT_BEHAVIOR
                        + program::CONTRACT_CALLS),
                ),
                author_path: None,
                author_first_line: 0,
                author_line_count: 0,
                particle_stride: program::particle_layout(e).2,
            })
            .collect();
        let cooked = CookedParticleEffect {
            program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
            description: desc,
            programs,
            source_digests: Default::default(),
        };
        let mut backend = backend();
        let mut manager = RenderResourceManager::new();
        let source = source(&mut manager);
        let frame = frame(&source, &manager);
        // Assemble particles before the production geometry adapter binds scene depth/camera.
        let particle_step = ParticleRenderStep {
            emitter_affine: Default::default(),
            step_id: 1,
            delta_seconds: (1. / 64.).into(),
            // At z=-1, x=0.4 projects to x=54 in this 60-degree scene camera.
            origin: [0.4.into(), 0f32.into(), (-1f32).into()],
            emitting: true,
            paused: false,
        };
        let mut g = graph(&GeometryFrame {
            target: frame.target.clone(),
            extent: frame.extent,
            views: vec![],
        });
        // Existing Sprite command remains a legacy 2D draw in the same native plan.
        g.passes.push(RenderPass {
            pass_id: "legacy-sprite".into(),
            pass_name: "legacy-sprite".into(),
            pass_kind: RenderPassKind::DrawSpriteTextured,
            view_id: "legacy".into(),
            reads: vec![],
            writes: vec!["main".into()],
            color_targets: vec!["main".into()],
            depth_target: None,
            commands: vec![RenderPassCommand::DrawSpriteTextured {
                target: "main".into(),
                sprite_ref: "a41-sprite".into(),
                material_ref: None,
                sort_key: "0".into(),
                texture: None,
                binding: None,
                fallback_used: false,
                vertices: [
                    [-0.9, -0.2],
                    [-0.5, -0.2],
                    [-0.5, 0.2],
                    [-0.9, -0.2],
                    [-0.5, 0.2],
                    [-0.9, 0.2],
                ]
                .into_iter()
                .map(|p| RenderDrawVertex::new(p, [1., 1., 0., 1.], [0.; 2]))
                .collect(),
            }]
            .into(),
            debug_source: None,
        });
        g.passes.push(RenderPass {
            pass_id: "particles".into(),
            pass_name: "particles".into(),
            pass_kind: RenderPassKind::DrawParticles,
            view_id: "legacy".into(),
            reads: vec![],
            writes: vec!["main".into()],
            color_targets: vec!["main".into()],
            depth_target: None,
            commands: vec![
                RenderPassCommand::ReconcileParticles {
                    target: "main".into(),
                    sources: vec![ParticleSourceFrame {
                        simulation_origin_bits: [0; 3],
                        instance: 1,
                        epoch: 0,
                        assets: serde_json::json!({"effect":cooked,"textures":[null],"meshes":[null],"tints":[[1,1,1,1]]}).to_string().into(),
                        parameters: "{}".into(),
                        textures: vec![None],
                        meshes: vec![None],
                        step: particle_step.clone(),
                    }],
                },
                RenderPassCommand::SimulateParticles {
                    target: "main".into(),
                    instance: 1,
                    step: particle_step,
                },
                RenderPassCommand::DrawParticles {
                    target: "main".into(),
                    instance: 1,
                    emitter: 0,
                    view: ParticleRenderView::default(),
                    texture: None,
                    mesh: None,
                },
            ].into(),
            debug_source: None,
        });
        frame.append_to_graph(&mut g);
        let plan = compile_render_graph_to_rhi_plan(&g);
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(pixel(&pixels, 64, 32, 32), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 64, 54, 32), [0, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 64, 10, 32), [255, 255, 0, 255]);
    }
}

// Same resource scale shape as BM-03: every mesh is distinct; execution states repeat.
fn unique_mesh_execution_states() -> (
    RenderResourceManager,
    GeometryFrame,
    Vec<RenderResourceHandle>,
) {
    let mut manager = RenderResourceManager::new();
    let textures: Vec<_> = (0..2)
        .map(|i| {
            manager.request_resource(
                0,
                RenderResourceRequest::texture(format!("state-tex-{i}"), "1", 4),
            )
        })
        .collect();
    let materials: Vec<_> = (0..4)
        .map(|i| {
            let mut params = unlit([1.; 4]);
            params.ignore_vertex_color = i >= 2;
            params.textures.insert(
                "baseColorTexture".into(),
                RenderAssetReference {
                    expected_kind: RenderAssetKind::Texture,
                    reference: engine_runtime::runtime_package::RuntimeAssetRef {
                        id: format!("state-tex-{}", i % 2),
                        asset_type: "texture".into(),
                        guid: Some(format!("state-tex-{}", i % 2)),
                        sub_asset: None,
                    },
                },
            );
            let mut mat = prepare(
                &format!("state-material-{i}"),
                RenderAssetData::Material(params),
                &mut manager,
            );
            mat.binding
                .slots
                .push(NamedRenderBinding::new("baseColorTexture", textures[i % 2]));
            mat
        })
        .collect();
    let sources: Vec<_> = (0..128)
        .map(|i| {
            let mut geometry = triangle();
            geometry.vertex_bytes[..4].copy_from_slice(&(-0.8f32 + i as f32 * 0.001).to_le_bytes());
            let mesh = prepare(
                &format!("unique-state-mesh-{i}"),
                RenderAssetData::Mesh(geometry),
                &mut manager,
            );
            PreparedGeometry::from_assets(&mesh, &materials[i % 4], 0, &manager).unwrap()
        })
        .collect();
    let items: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let mut item = instance(
                source,
                i as u64 + 1,
                [(i % 16) as f32 * 0.4 - 3., (i / 16) as f32 * 0.4 - 1.4, -2.],
            );
            item.world_transform[0] = 0.1;
            item.world_transform[5] = 0.1;
            item.world_transform[10] = 0.1;
            item
        })
        .collect();
    let mut view = RenderView::perspective(1, 256, 128, 60., 0.1, 10.);
    view.projection = Projection::Orthographic {
        size: 4.,
        near: 0.1,
        far: 10.,
    };
    let frame = GeometryFrame::build(
        "main",
        [256, 128],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &items,
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    (manager, frame, textures)
}

#[test]
fn opaque_unique_meshes_group_actual_pipeline_and_texture_states() {
    let (_manager, frame, _) = unique_mesh_execution_states();
    let draws = &frame.views[0].draws;
    assert_eq!(draws.len(), 128);
    let handles: std::collections::BTreeSet<_> =
        draws.iter().map(|d| d.geometry_handle().raw()).collect();
    assert_eq!(
        handles.len(),
        128,
        "unique meshes were replaced by shared handles"
    );
    let bytes: std::collections::HashSet<_> = draws.iter().map(|d| d.geometry.as_ref()).collect();
    assert_eq!(
        bytes.len(),
        128,
        "distinct verified mesh bytes were replaced"
    );
    let states: Vec<_> = draws
        .iter()
        .map(|d| {
            (
                d.pipeline_identity("Rgba8Unorm"),
                d.material_binding.texture("baseColorTexture"),
            )
        })
        .collect();
    let pipelines = 1 + states.windows(2).filter(|w| w[0].0 != w[1].0).count();
    let textures = 1 + states.windows(2).filter(|w| w[0] != w[1]).count();
    assert_eq!(
        pipelines, 2,
        "combined batch digest scattered identical native pipelines"
    );
    assert_eq!(
        textures, 4,
        "unique meshes scattered identical texture state inside each pipeline"
    );
    // Wire/API batch identity remains the full compatibility identity, not the grouping prefix.
    let batches: std::collections::BTreeSet<_> = draws.iter().map(|d| d.batch_identity()).collect();
    assert_eq!(batches.len(), 128);
    let (_manager2, repeated, _) = unique_mesh_execution_states();
    assert_eq!(
        draws.iter().map(|d| d.instance_id).collect::<Vec<_>>(),
        repeated.views[0]
            .draws
            .iter()
            .map(|d| d.instance_id)
            .collect::<Vec<_>>()
    );
}

#[cfg(feature = "real-wgpu")]
#[test]
fn gpu_unique_mesh_state_buckets_preserve_reference_pixels_and_resource_draws() {
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    let (_manager, frame, textures) = unique_mesh_execution_states();
    let mut reference = frame.clone();
    reference.views[0].draws.sort_by_key(|d| d.instance_id);
    let plan = compile_render_graph_to_rhi_plan(&graph(&frame));
    let reference_plan = compile_render_graph_to_rhi_plan(&graph(&reference));
    assert!(!plan.has_errors());
    assert!(!reference_plan.has_errors());
    let mut backend = RealWgpuBackend::new_offscreen(256, 128).unwrap();
    for (i, handle) in textures.iter().enumerate() {
        let bytes = if i == 0 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        };
        backend
            .register_rgba8_texture(*handle, 1, 1, &bytes, "nearestClamp")
            .unwrap();
    }
    let expected = backend
        .render_plan_to_rgba_bytes(&reference_plan, 256, 128)
        .unwrap();
    let actual = backend.render_plan_to_rgba_bytes(&plan, 256, 128).unwrap();
    assert_eq!(
        actual, expected,
        "opaque buckets changed non-overlapping reference output"
    );
    assert_eq!(backend.geometry_residency().meshes, 128);
    assert_eq!(backend.retained_geometry_stats().draws, 256);
    assert_eq!(backend.retained_geometry_stats().instances, 256);
    assert!(actual.chunks_exact(4).any(|p| p == [255, 0, 0, 255]));
    assert!(actual.chunks_exact(4).any(|p| p == [0, 0, 255, 255]));
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
