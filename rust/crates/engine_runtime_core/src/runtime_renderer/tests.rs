use super::*;

#[test]
fn joined_graph_cpu_disposal_overlaps_current_build_and_finishes_before_return() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct OldRow {
        first: bool,
        started: Arc<(Mutex<bool>, std::sync::Condvar)>,
        dropped: Arc<AtomicUsize>,
        overlap: Arc<AtomicUsize>,
    }
    impl Drop for OldRow {
        fn drop(&mut self) {
            if self.first {
                let (lock, wake) = self.started.as_ref();
                let (started, _) = wake
                    .wait_timeout_while(
                        lock.lock().unwrap(),
                        std::time::Duration::from_millis(100),
                        |started| !*started,
                    )
                    .unwrap();
                self.overlap.store(usize::from(*started), Ordering::SeqCst);
            }
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
    let mut observed = Vec::new();
    for rows in [4, 32, 256] {
        let started = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let dropped = Arc::new(AtomicUsize::new(0));
        let overlap = Arc::new(AtomicUsize::new(0));
        let old: Vec<_> = (0..rows)
            .map(|i| OldRow {
                first: i == 0,
                started: started.clone(),
                dropped: dropped.clone(),
                overlap: overlap.clone(),
            })
            .collect();
        let output = dispose_graph_while_building(old, true, || {
            *started.0.lock().unwrap() = true;
            started.1.notify_all();
            371
        });
        assert_eq!(output, 371);
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            rows,
            "destruction escaped this call"
        );
        observed.push((rows, overlap.load(Ordering::SeqCst)));
    }
    eprintln!("actual retired CPU row destruction/build overlap {observed:?}");
    assert!(
        observed.iter().all(|(_, overlap)| *overlap == 1),
        "{observed:?}"
    );
}
#[test]
fn joined_graph_cpu_disposal_preserves_serial_order_and_joins_on_unwind() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Old(Arc<AtomicUsize>, bool);
    impl Drop for Old {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            assert!(!self.1, "owned-old-graph-panic");
        }
    }
    let done = Arc::new(AtomicUsize::new(0));
    dispose_graph_while_building(Old(done.clone(), false), false, || {
        assert_eq!(done.load(Ordering::SeqCst), 1, "serial order changed");
    });
    let result = std::panic::catch_unwind(|| {
        dispose_graph_while_building(Old(done.clone(), false), true, || {
            panic!("current-build-panic")
        })
    });
    assert!(result.is_err());
    assert_eq!(
        done.load(Ordering::SeqCst),
        2,
        "unwind left disposal running"
    );
    let result = std::panic::catch_unwind(|| {
        dispose_graph_while_building(Old(done.clone(), true), true, || 371)
    });
    assert!(result.is_err(), "worker panic was hidden");
    assert_eq!(done.load(Ordering::SeqCst), 3);
}
use crate::aui::{
    AuiCanvas, AuiCompositionStage, AuiCompositionStageFrame, AuiComputedRect, AuiDocument,
    AuiDrawCommand, AuiDrawList, AuiLayoutEngine, AuiNode, AuiNodeKind, AuiOverlayDrawItem,
    AuiOverlayFrame, AuiOverlayItemKind, AuiOverlaySortKey, AuiRect, AuiRenderReport,
    AuiRendererBridge, AuiStyle, AuiTextGlyphPlan, AuiTextGlyphQuad,
};
use crate::components::{Renderable, Transform};
use crate::font_bundle::FontBundleRenderMode;
use crate::headless_rhi_backend::HeadlessRhiBackend;
use crate::ids::{RuntimeEntityId, SourceEntityId};
use crate::math::Vec3;
use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
use crate::render_state::{
    RenderProxy, RenderProxyId, RenderProxyPayload, RenderTargetKind, RenderViewId, RenderViewKind,
    SpritePayload,
};
use crate::rhi_command_plan::{RhiCommand, RhiDrawKind};

fn target() -> RenderTarget {
    RenderTarget::headless_texture("surface-main", 640, 480)
}

#[test]
fn retained_geometry_plan_reuses_only_immutable_frame_and_equal_base_graph() {
    let renderer = RuntimeRenderer::new();
    let scene = RenderSceneState::default();
    let geometry = Arc::new(crate::render_geometry::GeometryFrame {
        target: "surface-main".into(),
        extent: [640, 480],
        views: Vec::new(),
    });
    let first = renderer.build_with_retained_geometry(input(1, &scene), Some(&geometry));
    assert!(!first.rhi_command_plan.has_errors());
    let second = renderer.build_with_retained_geometry(input(2, &scene), Some(&geometry));
    let reference = RuntimeRenderer::new().build_with_geometry(input(2, &scene), Some(&geometry));
    assert_eq!(
        second, reference,
        "reuse must retag all current reports and plans"
    );
    let schedule = renderer.graph_schedule.lock().unwrap();
    assert_eq!(
        schedule.compilations + schedule.reuses,
        1,
        "same immutable geometry/base must not append or compile again"
    );
}

#[test]
fn retained_geometry_plan_invalidates_base_and_complete_target_inputs() {
    let scene = scene_with_view();
    let overlay = aui_overlay(2);
    let geometry = Arc::new(crate::render_geometry::GeometryFrame {
        target: "surface-main".into(),
        extent: [640, 480],
        views: Vec::new(),
    });
    for case in 0..7 {
        let renderer = RuntimeRenderer::new();
        renderer.build_with_retained_geometry(input(1, &scene), Some(&geometry));
        let make_input = || {
            let mut changed = input(2, &scene);
            match case {
                0 => changed.aui_overlay = Some(&overlay),
                1 => changed.render_target.width = 320,
                2 => changed.render_target.format = "Bgra8UnormSrgb".into(),
                3 => changed.render_target.color_space = "linear".into(),
                4 => changed.render_target.target_kind = RuntimeRenderTargetKind::Surface,
                5 => changed.quality_profile.profile_id = "changed".into(),
                6 => changed.render_target.target_id = "other-target".into(),
                _ => unreachable!(),
            }
            changed
        };
        let actual = renderer.build_with_retained_geometry(make_input(), Some(&geometry));
        let reference = RuntimeRenderer::new().build_with_geometry(make_input(), Some(&geometry));
        assert_eq!(actual, reference, "changed input case {case}");
        let schedule = renderer.graph_schedule.lock().unwrap();
        assert_eq!(
            schedule.compilations + schedule.reuses,
            2,
            "must compile case {case}"
        );
    }
    let renderer = RuntimeRenderer::new();
    renderer.build_with_retained_geometry(input(1, &scene), Some(&geometry));
    let other_scene = RenderSceneState::default();
    let actual = renderer.build_with_retained_geometry(input(2, &other_scene), Some(&geometry));
    let reference =
        RuntimeRenderer::new().build_with_geometry(input(2, &other_scene), Some(&geometry));
    assert_eq!(
        actual, reference,
        "changed view must not retain the old graph"
    );
    let schedule = renderer.graph_schedule.lock().unwrap();
    assert_eq!(schedule.compilations + schedule.reuses, 2);
}

#[test]
fn retained_geometry_plan_rejects_mutation_and_does_not_cache_errors() {
    let renderer = RuntimeRenderer::new();
    let scene = RenderSceneState::default();
    let mut geometry = Arc::new(crate::render_geometry::GeometryFrame {
        target: "surface-main".into(),
        extent: [640, 480],
        views: Vec::new(),
    });
    renderer.build_with_retained_geometry(input(1, &scene), Some(&geometry));
    assert!(
        Arc::get_mut(&mut geometry).is_none(),
        "retained input cannot mutate in place"
    );
    Arc::make_mut(&mut geometry).extent = [1, 1];
    let invalid = renderer.build_with_retained_geometry(input(2, &scene), Some(&geometry));
    assert!(invalid.rhi_command_plan.has_errors());
    assert!(renderer.retained_geometry_plan.lock().unwrap().is_none());
    Arc::make_mut(&mut geometry).extent = [640, 480];
    let valid = renderer.build_with_retained_geometry(input(3, &scene), Some(&geometry));
    let reference = RuntimeRenderer::new().build_with_geometry(input(3, &scene), Some(&geometry));
    assert_eq!(valid, reference);
    // Public borrowed callers can mutate their geometry between calls and
    // must still take normal graph validation, even with the same address.
    let mut borrowed = (*geometry).clone();
    renderer.build_with_geometry(input(4, &scene), Some(&borrowed));
    borrowed.extent = [1, 1];
    assert!(renderer
        .build_with_geometry(input(5, &scene), Some(&borrowed))
        .rhi_command_plan
        .has_errors());
    assert!(renderer.retained_geometry_plan.lock().unwrap().is_none());
    renderer.build_with_retained_geometry(input(6, &scene), Some(&geometry));
    renderer.build_with_retained_geometry(input(7, &scene), None);
    assert!(
        renderer.retained_geometry_plan.lock().unwrap().is_none(),
        "removed geometry releases the single entry"
    );
}

#[test]
fn retained_geometry_plan_preserves_real_draw_values_and_old_frame() {
    use crate::rhi_command_plan::{RhiCommand, RhiDrawPayload};
    let source = crate::rhi_command_plan::tests::retained_geometry_graph(1536);
    let draws: Vec<_> = source
        .passes
        .iter()
        .flat_map(|p| &p.commands)
        .filter_map(|c| match c {
            RenderPassCommand::DrawGeometry { draw } => Some(draw.clone()),
            _ => None,
        })
        .collect();
    let mut geometry = Arc::new(crate::render_geometry::GeometryFrame {
        target: "surface-main".into(),
        extent: [640, 480],
        views: vec![crate::render_geometry::GeometryViewFrame {
            coordinate_origin: [0.; 3],
            color_target: None,
            view: draws[0].view.as_ref().clone(),
            depth_target: None,
            draws,
            batches: None,
            summary: None,
            trace: Vec::new(),
        }],
    });
    let scene = RenderSceneState::default();
    let renderer = RuntimeRenderer::new();
    let original = renderer.build_with_retained_geometry(input(1, &scene), Some(&geometry));
    assert!(!original.rhi_command_plan.has_errors());
    let warm = renderer.build_with_retained_geometry(input(2, &scene), Some(&geometry));
    let reference = RuntimeRenderer::new().build_with_geometry(input(2, &scene), Some(&geometry));
    assert_eq!(warm, reference);
    Arc::make_mut(&mut Arc::make_mut(&mut geometry).views[0].draws[0]).world_transform[12] =
        0.25.into();
    let changed = renderer.build_with_retained_geometry(input(3, &scene), Some(&geometry));
    let reference = RuntimeRenderer::new().build_with_geometry(input(3, &scene), Some(&geometry));
    assert_eq!(changed, reference);
    let first_x = |plan: &RhiCommandPlan| {
        plan.commands
            .iter()
            .find_map(|c| match c {
                RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } => Some(draw.world_transform[12]),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(first_x(&changed.rhi_command_plan), 0.25.into());
    assert_eq!(first_x(&original.rhi_command_plan), 0.0.into());
    let schedule = renderer.graph_schedule.lock().unwrap();
    assert_eq!(schedule.compilations + schedule.reuses, 2);
}

#[test]
#[ignore = "bounded CPU owner cost diagnostic; not Release or scale qualification"]
fn diagnostic_retained_plan_clone_cost() {
    let graph = crate::rhi_command_plan::tests::retained_geometry_graph(48_688);
    let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors());
    let mut graph_ns = 0;
    let mut plan_ns = 0;
    for _ in 0..20 {
        let start = crate::platform_time::Instant::now();
        drop(std::hint::black_box(graph.clone()));
        graph_ns += start.elapsed().as_nanos();
        let start = crate::platform_time::Instant::now();
        drop(std::hint::black_box(plan.clone()));
        plan_ns += start.elapsed().as_nanos();
    }
    eprintln!("owner-clone Debug commands={} graphEnumBytes={} rhiEnumBytes={} graphCloneDropMeanNs={} planCloneDropMeanNs={}",
            plan.commands.len(), std::mem::size_of::<RenderPassCommand>(),
            std::mem::size_of::<crate::rhi_command_plan::RhiCommand>(), graph_ns/20, plan_ns/20);
}

fn viewport_target() -> RenderTarget {
    RenderTarget::viewport_texture("viewport-scene", 800, 450)
}

fn surface_target() -> RenderTarget {
    RenderTarget::surface("surface-main", 1024, 576)
}

fn input<'a>(frame_index: u64, scene: &'a RenderSceneState) -> RuntimeRendererInput<'a> {
    RuntimeRendererInput {
        frame_index,
        render_scene_state: scene,
        render_view_state: None,
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: target(),
    }
}

fn viewport_input<'a>(frame_index: u64, scene: &'a RenderSceneState) -> RuntimeRendererInput<'a> {
    RuntimeRendererInput {
        frame_index,
        render_scene_state: scene,
        render_view_state: None,
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: viewport_target(),
    }
}

fn surface_input<'a>(frame_index: u64, scene: &'a RenderSceneState) -> RuntimeRendererInput<'a> {
    RuntimeRendererInput {
        frame_index,
        render_scene_state: scene,
        render_view_state: None,
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: surface_target(),
    }
}

fn aui_overlay(frame_index: u64) -> crate::aui::AuiOverlayFrame {
    let draw_list = AuiDrawList {
        commands: vec![
            Arc::new(AuiDrawCommand::DrawRect {
                node_id: "panel".to_string(),
                rect: AuiComputedRect {
                    x: 10.0,
                    y: 20.0,
                    width: 200.0,
                    height: 100.0,
                },
                effective_clip_rect: None,
                color: AuiStyle::color("#223344").color,
            }),
            Arc::new(AuiDrawCommand::DrawText {
                node_id: "label".to_string(),
                rect: AuiComputedRect {
                    x: 20.0,
                    y: 30.0,
                    width: 160.0,
                    height: 32.0,
                },
                effective_clip_rect: None,
                text: "Hello".to_string(),
                color: Some("#ffffff".to_string()),
                font_size: Some(18.0),
                font: None,
            }),
        ],
    };
    AuiRendererBridge::build_overlay_frame(frame_index, &draw_list)
}

fn aui_composition(frame_index: u64) -> crate::aui::AuiCompositionFrame {
    let mut before = AuiCanvas::screen_overlay("before", 800.0, 600.0, "before-root");
    before.composition_stage = AuiCompositionStage::BeforeWorld;
    let overlay = AuiCanvas::screen_overlay("overlay", 800.0, 600.0, "overlay-root");
    let mut modal = AuiCanvas::screen_overlay("modal", 800.0, 600.0, "modal-root");
    modal.composition_stage = AuiCompositionStage::Modal;
    let document = AuiDocument::new(
        "runtime-renderer-composition",
        vec![before, overlay, modal],
        vec![
            AuiNode::new("before-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            AuiNode::new("overlay-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
            AuiNode::new("modal-root", AuiNodeKind::Panel, AuiRect::stretch_full()),
        ],
    );
    let layout = AuiLayoutEngine::layout(&document, frame_index);
    let (draw_list, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    AuiRendererBridge::build_composition_frame(frame_index, &document, &layout, &draw_list)
}

fn image_overlay(frame_index: u64, asset_ids: &[&str]) -> AuiOverlayFrame {
    let draw_items = asset_ids
        .iter()
        .enumerate()
        .map(|(index, asset_id)| {
            Arc::new(AuiOverlayDrawItem {
                presentation: None,
                item_id: format!("image:{index}"),
                canvas_id: "legacy-overlay".to_string(),
                composition_stage: AuiCompositionStage::ScreenOverlay,
                node_id: format!("image-{index}"),
                item_kind: AuiOverlayItemKind::Image,
                rect: AuiComputedRect {
                    x: index as f32 * 40.0,
                    y: 0.0,
                    width: 40.0,
                    height: 40.0,
                },
                effective_clip_rect: None,
                effective_clips: Vec::new(),
                paint: None,
                color: None,
                asset_id: Some((*asset_id).to_string()),
                text: None,
                font_size: None,
                font: None,
                mesh: None,
                sort_key: AuiOverlaySortKey {
                    canvas_layer: 0,
                    canvas_sorting_order: 0,
                    tree_order: index,
                },
            })
        })
        .collect::<Vec<_>>();
    AuiOverlayFrame {
        frame_index,
        report: AuiRenderReport {
            draw_command_count: draw_items.len(),
            text_count: 0,
            image_count: draw_items.len(),
            graphics_diagnostics: Vec::new(),
            effective_clip_item_count: 0,
            culled_draw_item_count: 0,
            scrollbar_visible_count: 0,
            batch_hint_count: 0,
        },
        draw_items: draw_items.into(),
        glyph_plan: None,
    }
}

fn ui_test_presentation(width: u32, height: u32) -> ResolvedGameViewPresentation {
    GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "m3-renderer-test".to_string(),
        target_id: "target".to_string(),
        target_extent: GameViewExtent::new(width, height),
        display_rect: GameViewRect::new(0.0, 0.0, width as f32, height as f32),
        scale_policy: GameViewScalePolicy::Stretch,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact {
            canvas_id: "canvas".to_string(),
            reference_extent: GameViewExtent::new(width, height),
        }],
    })
    .unwrap()
}

fn ui_test_item(rect: AuiComputedRect) -> AuiOverlayDrawItem {
    AuiOverlayDrawItem {
        presentation: None,
        item_id: "item".to_string(),
        canvas_id: "canvas".to_string(),
        composition_stage: AuiCompositionStage::ScreenOverlay,
        node_id: "node".to_string(),
        item_kind: AuiOverlayItemKind::Rect,
        rect,
        effective_clip_rect: None,
        effective_clips: Vec::new(),
        paint: None,
        color: Some("#ffffffff".to_string()),
        asset_id: None,
        text: None,
        font_size: None,
        font: None,
        mesh: None,
        sort_key: AuiOverlaySortKey {
            canvas_layer: 0,
            canvas_sorting_order: 0,
            tree_order: 0,
        },
    }
}

#[test]
fn aui_color_contract_decodes_hex_rgb_once_and_keeps_alpha_linear() {
    let color = parse_aui_color(Some("#20372f80")).unwrap();
    for (actual, expected) in color
        .iter()
        .zip([0.014443844, 0.038204372, 0.02842604, 128. / 255.])
    {
        assert!((actual - expected).abs() < 1e-6, "{color:?}");
    }
    assert_eq!(parse_aui_color(Some("#00000000")), Some([0.; 4]));
    assert_eq!(parse_aui_color(Some("#ffffff")), Some([1.; 4]));
    assert!(parse_aui_color(Some("#notrgb")).is_none());
}

#[test]
fn aui_color_contract_applies_linear_tint_after_hex_decode() {
    let rect = AuiComputedRect {
        x: 10.,
        y: 10.,
        width: 20.,
        height: 20.,
    };
    let mut item = ui_test_item(rect);
    item.color = Some("#80808080".into());
    item.presentation = Some(crate::aui::AuiPresentationGeometry {
        local_to_canvas: [1., 0., 0., 1., 10., 10.],
        vertex_transform: [1., 0., 0., 1., 0., 0.],
        bounds: rect,
        modulation: [0.5, 1., 1., 0.5],
    });
    let output = ui_projection_items(
        &AuiCompositionFrame::default(),
        &[Arc::new(item)],
        &ui_test_presentation(100, 100),
        None,
        &BTreeMap::new(),
        false,
    );
    assert_eq!(output.batches.len(), 1);
    for vertex in output.batches[0].vertices.iter() {
        for (actual, expected) in
            vertex
                .color
                .iter()
                .zip([0.10793025, 0.2158605, 0.2158605, 64. / 255.])
        {
            assert!((actual.to_f32() - expected).abs() < 1e-6);
        }
    }
}

#[test]
fn feedback_brightness_is_applied_before_final_color_saturation() {
    let rect = AuiComputedRect {
        x: 10.,
        y: 10.,
        width: 20.,
        height: 20.,
    };
    let mut item = ui_test_item(rect);
    item.presentation = Some(crate::aui::AuiPresentationGeometry {
        local_to_canvas: [1., 0., 0., 1., 10., 10.],
        vertex_transform: [1., 0., 0., 1., 0., 0.],
        bounds: rect,
        modulation: [1.5, 1.5, 1.5, 0.5],
    });
    let vertices = projected_clipped_quad_vertices(
        &ui_test_presentation(100, 100),
        "canvas",
        rect,
        &item,
        [0.4, 0.8, 0.2, 0.8],
        [0., 0., 1., 1.],
    );
    assert_eq!(vertices.len(), 6);
    for vertex in vertices {
        for (actual, expected) in vertex.color.iter().zip([0.6, 1., 0.3, 0.4]) {
            assert!(
                (actual.to_f32() - expected).abs() < 1e-5,
                "Feedback must brighten the actual primitive color."
            );
        }
    }
}

#[test]
fn cross_canvas_polygon_keeps_visible_area_and_interpolates_uv_rgba() {
    let presentation = ui_test_presentation(100, 100);
    let polygon = [[-50., -50.], [150., -50.], [150., 150.], [-50., 150.]]
        .into_iter()
        .map(|position| {
            let uv = [(position[0] + 50.) / 200., (position[1] + 50.) / 200.];
            UiLogicalVertex {
                position,
                uv,
                color: [uv[0], 0., 0., uv[1]],
            }
        })
        .collect::<Vec<_>>();
    let vertices = project_polygon(&presentation, "canvas", &polygon, [1.; 4]);
    assert_eq!(
        vertices.len(),
        6,
        "Crossing the canvas must clip, not discard the polygon."
    );
    for vertex in vertices {
        let x = (vertex.position[0].to_f32() + 1.) * 50.;
        let y = (1. - vertex.position[1].to_f32()) * 50.;
        assert!((0. ..=100.).contains(&x) && (0. ..=100.).contains(&y));
        let expected = [(x + 50.) / 200., (y + 50.) / 200.];
        assert!((vertex.uv[0].to_f32() - expected[0]).abs() < 1e-5);
        assert!((vertex.uv[1].to_f32() - expected[1]).abs() < 1e-5);
        assert!((vertex.color[0].to_f32() - expected[0]).abs() < 1e-5);
        assert!((vertex.color[3].to_f32() - expected[1]).abs() < 1e-5);
    }
}

#[test]
fn rounded_precision_budget_is_rejected_by_production_projection() {
    let rect = AuiComputedRect {
        x: 0.,
        y: 0.,
        width: 100.,
        height: 60.,
    };
    let mut item = ui_test_item(rect);
    item.paint = Some(crate::aui_theme::AuiResolvedPaint {
        radius: 12.,
        ..Default::default()
    });
    item.presentation = Some(crate::aui::AuiPresentationGeometry {
        local_to_canvas: [1e9, 0., 0., 1e9, 0., 0.],
        vertex_transform: [1e9, 0., 0., 1e9, 0., 0.],
        bounds: rect,
        modulation: [1.; 4],
    });
    let output = ui_projection_items(
        &AuiCompositionFrame::default(),
        &[Arc::new(item)],
        &ui_test_presentation(100, 60),
        None,
        &BTreeMap::new(),
        false,
    );
    assert!(output.batches.is_empty());
    assert_eq!(
        output.diagnostics[0].code,
        "aui_presentation.geometry_budget"
    );
    let segments = rounded_rect_segments(rect, 12., 2.).unwrap();
    let sagitta = 24.0 * (1.0 - (std::f64::consts::FRAC_PI_2 / (segments as f64) / 2.0).cos());
    assert!(sagitta <= 0.25);
}

#[test]
fn m3_rounded_rect_and_border_use_real_segmented_geometry() {
    let rect = AuiComputedRect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 60.0,
    };
    let polygon = rounded_rect_polygon(rect, 12.0);
    assert_eq!(polygon.len(), 28);
    assert!(polygon
        .iter()
        .all(|vertex| vertex.position[0].is_finite() && vertex.position[1].is_finite()));
    assert!(!polygon.iter().any(|vertex| vertex.position == [0.0, 0.0]));

    let presentation = ui_test_presentation(100, 60);
    let item = ui_test_item(rect);
    let ring = projected_rounded_ring_vertices(
        &presentation,
        "canvas",
        rect,
        12.0,
        5.0,
        &item,
        [1.0, 0.0, 0.0, 1.0],
    );
    // Adaptive outer/inner contours share angular samples; triangle count follows precision.
    let segments =
        rounded_rect_segments(rect, 12., item_pixel_scale(&presentation, &item)).unwrap();
    assert_eq!(ring.len(), 4 * (segments + 1) * 6);
    let inner_contains = |point: [f32; 2]| {
        let nearest_x = point[0].clamp(12.0, 88.0);
        let nearest_y = point[1].clamp(12.0, 48.0);
        let dx = point[0] - nearest_x;
        let dy = point[1] - nearest_y;
        dx * dx + dy * dy <= 7.0 * 7.0
    };
    for triangle in ring.chunks_exact(3) {
        let centroid = triangle.iter().fold([0.0, 0.0], |mut total, vertex| {
            let ndc = vertex.position.map(|value| value.to_f32());
            total[0] += (ndc[0] + 1.0) * 50.0 / 3.0;
            total[1] += (1.0 - ndc[1]) * 30.0 / 3.0;
            total
        });
        assert!(
            !inner_contains(centroid),
            "border triangle filled the inner content at {centroid:?}"
        );
    }
}

#[test]
fn m3_nine_slice_clamps_small_targets_and_keeps_uv_finite() {
    let presentation = ui_test_presentation(100, 100);
    let slice = crate::aui_theme::AuiSliceInsets {
        left: 12.0,
        top: 10.0,
        right: 12.0,
        bottom: 10.0,
    };
    let normal = projected_nine_slice_vertices(
        &presentation,
        "canvas",
        AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 60.0,
        },
        slice,
        32,
        32,
        [1.0; 4],
        &[],
    );
    assert_eq!(normal.len(), 54);

    let small = projected_nine_slice_vertices(
        &presentation,
        "canvas",
        AuiComputedRect {
            x: 5.0,
            y: 7.0,
            width: 8.0,
            height: 6.0,
        },
        slice,
        16,
        16,
        [1.0; 4],
        &[],
    );
    assert!(!small.is_empty());
    assert!(small.iter().all(|vertex| {
        vertex
            .position
            .iter()
            .chain(vertex.uv.iter())
            .all(|value| value.to_f32().is_finite())
            && vertex
                .uv
                .iter()
                .all(|value| (0.0..=1.0).contains(&value.to_f32()))
    }));
}

#[test]
fn m3_nested_rounded_clip_preserves_interpolated_uvs() {
    let subject = vec![
        UiLogicalVertex {
            position: [0.0, 0.0],
            uv: [0.0, 0.0],
            color: [1.0; 4],
        },
        UiLogicalVertex {
            position: [100.0, 0.0],
            uv: [1.0, 0.0],
            color: [1.0; 4],
        },
        UiLogicalVertex {
            position: [100.0, 100.0],
            uv: [1.0, 1.0],
            color: [1.0; 4],
        },
        UiLogicalVertex {
            position: [0.0, 100.0],
            uv: [0.0, 1.0],
            color: [1.0; 4],
        },
    ];
    let clips = vec![
        crate::aui::AuiResolvedClip {
            transform: None,
            node_id: "outer".to_string(),
            rect: AuiComputedRect {
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 80.0,
            },
            radius: 0.0,
        },
        crate::aui::AuiResolvedClip {
            transform: None,
            node_id: "inner".to_string(),
            rect: AuiComputedRect {
                x: 20.0,
                y: 20.0,
                width: 60.0,
                height: 60.0,
            },
            radius: 10.0,
        },
    ];
    let clipped = clip_polygon_chain(subject, &clips);
    assert!(clipped.len() >= 20);
    assert!(!clipped.iter().any(|vertex| vertex.position == [20.0, 20.0]));
    for vertex in clipped {
        assert!((20.0..=80.0).contains(&vertex.position[0]));
        assert!((20.0..=80.0).contains(&vertex.position[1]));
        assert!((vertex.uv[0] - vertex.position[0] / 100.0).abs() < 0.0001);
        assert!((vertex.uv[1] - vertex.position[1] / 100.0).abs() < 0.0001);
    }
}

#[test]
fn m3_theme_texture_miss_is_an_explicit_renderer_diagnostic() {
    let presentation = ui_test_presentation(100, 100);
    let item = ui_test_item(AuiComputedRect {
        x: 10.0,
        y: 10.0,
        width: 40.0,
        height: 20.0,
    });
    let paint = crate::aui_theme::AuiResolvedPaint {
        background: crate::aui_theme::AuiResolvedBrush::Image {
            asset_id: "missing-theme-texture".to_string(),
            tint: "#ffffffff".to_string(),
        },
        ..Default::default()
    };
    let mut output = UiProjectedOrderedBatches::default();
    append_resolved_paint_batches(&mut output, &item, &paint, &presentation, None);

    assert!(output.batches.is_empty());
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].code, "aui_theme.texture_not_resolved");
    assert!(output.diagnostics[0]
        .message
        .contains("missing-theme-texture"));
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real local WGPU adapter"]
fn m3_real_wgpu_theme_readback_covers_shape_clip_shadow_and_nine_slice() {
    use crate::render_graph::{color, RenderGraph, RenderPass, RenderResource};
    use crate::rhi_command_plan::compile_render_graph_to_rhi_plan;
    use crate::wgpu_backend::real::RealWgpuBackend;

    let presentation = ui_test_presentation(64, 64);
    let mut projected = UiProjectedOrderedBatches::default();

    let rounded = ui_test_item(AuiComputedRect {
        x: 4.0,
        y: 4.0,
        width: 24.0,
        height: 20.0,
    });
    append_resolved_paint_batches(
        &mut projected,
        &rounded,
        &crate::aui_theme::AuiResolvedPaint {
            background: crate::aui_theme::AuiResolvedBrush::solid("#00ff00ff"),
            border: crate::aui_theme::AuiResolvedBorder {
                color: Some("#ff0000ff".into()),
                width: 2.0,
            },
            radius: 6.0,
            ..Default::default()
        },
        &presentation,
        None,
    );

    let shadowed = ui_test_item(AuiComputedRect {
        x: 38.0,
        y: 5.0,
        width: 16.0,
        height: 16.0,
    });
    append_resolved_paint_batches(
        &mut projected,
        &shadowed,
        &crate::aui_theme::AuiResolvedPaint {
            background: crate::aui_theme::AuiResolvedBrush::solid("#00ff00ff"),
            radius: 4.0,
            shadow: crate::aui_theme::AuiResolvedShadow {
                color: Some("#0000ffff".into()),
                offset_x: 3.0,
                offset_y: 3.0,
                blur: 2.0,
                spread: 1.0,
            },
            ..Default::default()
        },
        &presentation,
        None,
    );

    let clipped = projected_quad_with_presentation(
        &presentation,
        "canvas",
        AuiComputedRect {
            x: 0.0,
            y: 32.0,
            width: 30.0,
            height: 30.0,
        },
        &[crate::aui::AuiResolvedClip {
            transform: None,
            node_id: "clip".into(),
            rect: AuiComputedRect {
                x: 4.0,
                y: 36.0,
                width: 22.0,
                height: 22.0,
            },
            radius: 7.0,
        }],
        [1.0, 0.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        None,
    );
    append_ui_projected_batch(
        &mut projected.batches,
        UiProjectedBatchKey::Solid,
        None,
        None,
        None,
        0,
        "rounded-child-clip".into(),
        clipped,
    );

    let texture_handle = crate::runtime_texture::runtime_texture_render_handle("m3-nine");
    let nine_slice = projected_nine_slice_vertices(
        &presentation,
        "canvas",
        AuiComputedRect {
            x: 36.0,
            y: 36.0,
            width: 24.0,
            height: 24.0,
        },
        crate::aui_theme::AuiSliceInsets {
            left: 1.0,
            top: 1.0,
            right: 1.0,
            bottom: 1.0,
        },
        3,
        3,
        [1.0; 4],
        &[],
    );

    let target = "m3-offscreen".to_string();
    let mut graph = RenderGraph::new("m3-theme-readback", 1);
    graph.output_target = Some(target.clone());
    graph
        .resources
        .push(RenderResource::surface_backbuffer(target.clone(), 64, 64));
    let mut commands = vec![RenderPassCommand::Clear {
        target: target.clone(),
        color: color([0.0, 0.0, 0.0, 1.0]),
    }];
    for batch in projected.batches {
        commands.push(RenderPassCommand::DrawUiComposition {
            target: target.clone(),
            stage: "screen_overlay".into(),
            item_count: 1,
            text_count: 0,
            image_count: 0,
            glyph_count: 0,
            font_atlas_id: None,
            text_pass_inserted: false,
            debug_label: batch.debug_label,
            texture: None,
            font_render_mode: None,
            font_page_index: None,
            vertices: batch.vertices,
        });
    }
    commands.push(RenderPassCommand::DrawUiComposition {
        target: target.clone(),
        stage: "screen_overlay".into(),
        item_count: 1,
        text_count: 0,
        image_count: 1,
        glyph_count: 0,
        font_atlas_id: None,
        text_pass_inserted: false,
        debug_label: "nine-slice".into(),
        texture: Some(texture_handle),
        font_render_mode: None,
        font_page_index: None,
        vertices: nine_slice.into(),
    });
    graph.passes.push(RenderPass {
        pass_id: "m3-ui".into(),
        pass_name: "M3 UI Readback".into(),
        pass_kind: RenderPassKind::DrawUiComposition,
        view_id: "view".into(),
        reads: Vec::new(),
        writes: vec![target.clone()],
        color_targets: vec![target],
        depth_target: None,
        commands: commands.into(),
        debug_source: Some("m3-real-wgpu-readback".into()),
    });

    let mut backend = RealWgpuBackend::new_offscreen(64, 64)
        .expect("M3 qualification requires a real WGPU adapter");
    eprintln!("M3 WGPU backend: {}", backend.device_context().backend_name);
    let texels = [
        [255, 0, 0, 255],
        [32, 32, 32, 255],
        [0, 255, 0, 255],
        [32, 32, 32, 255],
        [255, 255, 255, 255],
        [32, 32, 32, 255],
        [0, 0, 255, 255],
        [32, 32, 32, 255],
        [255, 255, 0, 255],
    ];
    let texture_bytes = texels.into_iter().flatten().collect::<Vec<_>>();
    backend
        .register_rgba8_texture(texture_handle, 3, 3, &texture_bytes, "nearestClamp")
        .unwrap();
    let rgba = backend
        .render_plan_to_rgba_bytes(&compile_render_graph_to_rhi_plan(&graph), 64, 64)
        .unwrap();
    let pixel = |x: usize, y: usize| -> &[u8] { &rgba[(y * 64 + x) * 4..(y * 64 + x + 1) * 4] };

    assert!(pixel(16, 14)[1] > 220, "center: {:?}", pixel(16, 14));
    assert!(pixel(5, 14)[0] > 220, "border: {:?}", pixel(5, 14));
    assert!(
        pixel(4, 4)[0] < 16 && pixel(4, 4)[1] < 16,
        "rounded corner leaked: {:?}",
        pixel(4, 4)
    );
    assert!(pixel(59, 14)[2] > 16, "shadow: {:?}", pixel(59, 14));
    assert!(
        pixel(4, 36)[0] < 16 && pixel(4, 36)[2] < 16,
        "rounded clip leaked: {:?}",
        pixel(4, 36)
    );
    assert!(
        pixel(15, 47)[0] > 220 && pixel(15, 47)[2] > 220,
        "clipped center: {:?}",
        pixel(15, 47)
    );
    assert!(
        pixel(36, 36)[0] > 200,
        "nine-slice corner: {:?}",
        pixel(36, 36)
    );
    assert!(
        pixel(48, 48)[0] > 200 && pixel(48, 48)[1] > 200 && pixel(48, 48)[2] > 200,
        "nine-slice center: {:?}",
        pixel(48, 48)
    );
}

#[test]
fn runtime_renderer_batches_only_adjacent_images_with_same_texture() {
    let scene = scene_with_view();
    let overlay = image_overlay(30, &["texture-a", "texture-a", "texture-b", "texture-a"]);
    let mut bindings = RuntimeTextureBindingContext::default();
    for asset_id in ["texture-a", "texture-b"] {
        bindings.insert(
            asset_id,
            crate::runtime_texture::runtime_texture_render_handle(asset_id),
            "linearClamp",
        );
    }
    let mut renderer_input = input(30, &scene);
    renderer_input.aui_overlay = Some(&overlay);
    renderer_input.runtime_texture_bindings = Some(&bindings);

    let output = RuntimeRenderer::new().build(renderer_input);
    let commands = output
        .render_graph
        .passes
        .iter()
        .find(|pass| pass.pass_kind == RenderPassKind::DrawUiComposition)
        .expect("UI pass")
        .commands
        .iter()
        .filter(|command| matches!(command, RenderPassCommand::DrawUiComposition { .. }))
        .collect::<Vec<_>>();

    assert_eq!(commands.len(), 3);
    assert_eq!(output.render_frame_report.ui_image_request_count, 4);
    assert_eq!(output.render_frame_report.ui_image_resolved_count, 4);
    assert_eq!(output.render_frame_report.ui_image_missing_count, 0);
    assert_eq!(output.render_frame_report.ui_textured_batch_count, 3);
    assert!(matches!(
        commands[0],
        RenderPassCommand::DrawUiComposition { vertices, .. } if vertices.len() == 12
    ));
}

#[test]
fn runtime_renderer_reports_missing_aui_image_texture_without_solid_fallback() {
    let scene = scene_with_view();
    let overlay = image_overlay(31, &["texture-missing"]);
    let mut renderer_input = input(31, &scene);
    renderer_input.aui_overlay = Some(&overlay);

    let output = RuntimeRenderer::new().build(renderer_input);

    assert_eq!(output.render_frame_report.ui_image_request_count, 1);
    assert_eq!(output.render_frame_report.ui_image_resolved_count, 0);
    assert_eq!(output.render_frame_report.ui_image_missing_count, 1);
    assert!(output
        .render_frame_report
        .diagnostics
        .iter()
        .any(|diagnostic| {
            diagnostic.code == "aui_image.texture_not_resolved"
                && diagnostic.message.contains("texture-missing")
        }));
    assert!(output.render_graph.passes.iter().all(|pass| {
        pass.commands.iter().all(|command| {
            !matches!(
                command,
                RenderPassCommand::DrawUiComposition { texture: None, .. }
            )
        })
    }));
}

#[test]
fn ui_projection_image_requires_textured_quad_with_clip_adjusted_uv() {
    let mut stage = AuiCompositionStageFrame::empty(AuiCompositionStage::ScreenOverlay);
    Arc::make_mut(&mut stage.draw_items).push(Arc::new(AuiOverlayDrawItem {
        presentation: None,
        item_id: "image:item".to_string(),
        canvas_id: "canvas".to_string(),
        composition_stage: AuiCompositionStage::ScreenOverlay,
        node_id: "image".to_string(),
        item_kind: AuiOverlayItemKind::Image,
        rect: AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        effective_clip_rect: Some(AuiComputedRect {
            x: 25.0,
            y: 20.0,
            width: 50.0,
            height: 60.0,
        }),
        effective_clips: Vec::new(),
        paint: None,
        color: None,
        asset_id: Some("texture-ui".to_string()),
        text: None,
        font_size: None,
        font: None,
        mesh: None,
        sort_key: AuiOverlaySortKey {
            canvas_layer: 0,
            canvas_sorting_order: 0,
            tree_order: 0,
        },
    }));
    let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "image-test".to_string(),
        target_id: "target".to_string(),
        target_extent: GameViewExtent::new(100, 100),
        display_rect: GameViewRect::new(0.0, 0.0, 100.0, 100.0),
        scale_policy: GameViewScalePolicy::Stretch,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact {
            canvas_id: "canvas".to_string(),
            reference_extent: GameViewExtent::new(100, 100),
        }],
    })
    .unwrap();

    let vertices = ui_projection_image_vertices(&stage.draw_items[0], &presentation)
        .expect("projected image vertices");

    assert_eq!(vertices.len(), 6, "AUI Image must not be silently skipped");
    assert_eq!(vertices[0].uv.map(|value| value.to_f32()), [0.25, 0.8]);
    assert_eq!(vertices[2].uv.map(|value| value.to_f32()), [0.75, 0.2]);
}

#[test]
fn ui_projection_font_batches_v2_glyphs_by_stable_mode_and_page() {
    let mut overlay = aui_overlay(21);
    let text_item = overlay
        .draw_items
        .iter()
        .find(|item| item.text.is_some())
        .expect("text draw item")
        .clone();
    let glyph = |render_mode, page_index, x| AuiTextGlyphQuad {
        font_bundle: None,
        item_id: text_item.item_id.clone(),
        node_id: text_item.node_id.clone(),
        codepoint: 0x4e2d + page_index,
        glyph_id: format!("glyph-{page_index}"),
        rect: AuiComputedRect {
            x,
            y: 30.0,
            width: 12.0,
            height: 18.0,
        },
        uv_rect: [0.125, 0.25, 0.375, 0.75],
        page_index,
        render_mode,
        clipped: false,
        color: None,
    };
    overlay.glyph_plan = Some(Arc::new(AuiTextGlyphPlan {
        font_atlas_id: "tower-defense-ui".to_string(),
        font_source_kind: "project_font_bundle_v2".to_string(),
        font_asset_id: "tower-defense-ui".to_string(),
        font_asset_status: "ready".to_string(),
        fallback_used: false,
        requested_glyph_count: 3,
        rendered_glyph_count: 3,
        unsupported_glyph_count: 0,
        clipped_glyph_count: 0,
        atlas_width: 256,
        atlas_height: 256,
        atlas_generation: 7,
        glyph_plan_hash: "sha256:test".to_string(),
        quads: vec![
            Arc::new(glyph(FontBundleRenderMode::MsdfRgba8, 2, 50.0)),
            Arc::new(glyph(FontBundleRenderMode::BitmapR8, 1, 35.0)),
            Arc::new(glyph(FontBundleRenderMode::BitmapR8, 0, 20.0)),
        ],
    }));
    let composition = crate::aui::AuiCompositionFrame::from_overlay_frame(&overlay);
    let stage = composition
        .stage(AuiCompositionStage::ScreenOverlay)
        .expect("screen overlay");

    let presentation = resolve_ui_presentation(
        &composition,
        stage,
        [800.0, 600.0],
        "font-test",
        GameViewScalePolicy::Stretch,
    )
    .unwrap();
    let batches = ui_projection_font_batches(&composition, stage, &presentation);

    assert_eq!(batches.len(), 3);
    assert_eq!(
        batches
            .iter()
            .map(|batch| (batch.render_mode, batch.page_index, batch.glyph_count))
            .collect::<Vec<_>>(),
        vec![
            (FontBundleRenderMode::BitmapR8, 0, 1),
            (FontBundleRenderMode::BitmapR8, 1, 1),
            (FontBundleRenderMode::MsdfRgba8, 2, 1),
        ]
    );
    assert!(batches.iter().all(|batch| batch.font_render_mode.is_some()));
    assert!(batches.iter().all(|batch| batch.vertices.len() == 6));
    // Clip-space Y is up; atlas rows are down. Top vertices must sample v0,
    // bottom vertices v1, for BOTH bitmap and MSDF. The ordering is not a flip.
    for batch in &batches {
        let bottom = &batch.vertices[0];
        let top = &batch.vertices[2];
        assert!(top.position[1].to_f32() > bottom.position[1].to_f32());
        assert_eq!(top.uv[1].to_f32(), 0.25);
        assert_eq!(bottom.uv[1].to_f32(), 0.75);
    }
    assert_eq!(
        batches
            .iter()
            .map(|batch| batch.texture)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
}

#[test]
fn affine_glyph_projection_applies_matrix_tint_to_ordered_and_public_consumers() {
    let rect = AuiComputedRect {
        x: 10.,
        y: 20.,
        width: 20.,
        height: 10.,
    };
    let mut item = ui_test_item(rect);
    item.item_kind = AuiOverlayItemKind::Text;
    item.text = Some("人".into());
    item.presentation = Some(crate::aui::AuiPresentationGeometry {
        local_to_canvas: [0., 1., -1., 0., 80., 10.],
        vertex_transform: [0., 1., -1., 0., 100., 0.],
        bounds: AuiComputedRect {
            x: 70.,
            y: 10.,
            width: 10.,
            height: 20.,
        },
        modulation: [0.5, 1., 1., 0.25],
    });
    let mut overlay = AuiOverlayFrame::default();
    overlay.draw_items = Arc::new(vec![Arc::new(item)]);
    overlay.glyph_plan = Some(Arc::new(AuiTextGlyphPlan {
        font_atlas_id: "font".into(),
        font_source_kind: "project_font_bundle_v2_paragraph".into(),
        font_asset_id: "font".into(),
        font_asset_status: "ready".into(),
        fallback_used: false,
        requested_glyph_count: 1,
        rendered_glyph_count: 1,
        unsupported_glyph_count: 0,
        clipped_glyph_count: 0,
        atlas_width: 1,
        atlas_height: 1,
        atlas_generation: 1,
        glyph_plan_hash: "glyph".into(),
        quads: vec![Arc::new(AuiTextGlyphQuad {
            item_id: "item".into(),
            node_id: "node".into(),
            codepoint: '人' as u32,
            glyph_id: "glyph".into(),
            font_bundle: Some(("font".into(), 1)),
            rect,
            uv_rect: [0., 0., 1., 1.],
            page_index: 0,
            render_mode: FontBundleRenderMode::BitmapR8,
            clipped: false,
            color: Some("#ffffff".into()),
        })],
    }));
    let composition = AuiCompositionFrame::from_overlay_frame(&overlay);
    let stage = composition
        .stage(AuiCompositionStage::ScreenOverlay)
        .unwrap();
    let presentation = ui_test_presentation(100, 100);
    let public = ui_projection_font_batches(&composition, stage, &presentation);
    let ordered = ui_projection_ordered_batches(&composition, stage, &presentation, None);
    assert_eq!(public.len(), 1);
    assert_eq!(ordered.batches.len(), 1);
    let ordered_vertices = ordered.batches[0]
        .vertices
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    for vertices in [public[0].vertices.as_slice(), ordered_vertices.as_slice()] {
        assert_eq!(vertices.len(), 6);
        for vertex in vertices {
            let p = vertex.position.map(|v| v.to_f32());
            assert!((p[0] - 0.4).abs() < 1e-5 || (p[0] - 0.6).abs() < 1e-5);
            assert!((p[1] - 0.4).abs() < 1e-5 || (p[1] - 0.8).abs() < 1e-5);
            assert_eq!(vertex.color[0].to_f32(), 0.5);
            assert_eq!(vertex.color[3].to_f32(), 0.25);
        }
    }
}

#[test]
fn gate_b_projection_keeps_selected_bundle_and_generation() {
    let mut overlay = aui_overlay(21);
    let text_item = overlay
        .draw_items
        .iter()
        .find(|item| item.text.is_some())
        .unwrap()
        .clone();
    overlay.glyph_plan = Some(Arc::new(AuiTextGlyphPlan {
        font_atlas_id: "default".into(),
        font_source_kind: "project_font_bundle_v2_paragraph".into(),
        font_asset_id: "default".into(),
        font_asset_status: "ready".into(),
        fallback_used: false,
        requested_glyph_count: 2,
        rendered_glyph_count: 2,
        unsupported_glyph_count: 0,
        clipped_glyph_count: 0,
        atlas_width: 256,
        atlas_height: 256,
        atlas_generation: 1,
        glyph_plan_hash: "test".into(),
        quads: ["primary", "selected"]
            .iter()
            .enumerate()
            .map(|(index, id)| {
                Arc::new(AuiTextGlyphQuad {
                    item_id: text_item.item_id.clone(),
                    node_id: text_item.node_id.clone(),
                    codepoint: 65,
                    glyph_id: "face:1".into(),
                    font_bundle: Some((id.to_string(), 8 + index as u64)),
                    rect: text_item.rect,
                    uv_rect: [0., 0., 1., 1.],
                    page_index: 0,
                    render_mode: FontBundleRenderMode::BitmapR8,
                    clipped: false,
                    color: None,
                })
            })
            .collect(),
    }));
    let composition = crate::aui::AuiCompositionFrame::from_overlay_frame(&overlay);
    let stage = composition
        .stage(AuiCompositionStage::ScreenOverlay)
        .unwrap();
    let presentation = resolve_ui_presentation(
        &composition,
        stage,
        [800., 600.],
        "bundles",
        GameViewScalePolicy::Stretch,
    )
    .unwrap();
    let batches = ui_projection_font_batches(&composition, stage, &presentation);
    assert_eq!(batches.len(), 2);
    let expected =
        font_bundle_page_generation_render_handle("selected", FontBundleRenderMode::BitmapR8, 0, 9);
    assert!(batches.iter().any(|batch| batch.texture == expected));
    let ordered = ui_projection_ordered_batches(&composition, stage, &presentation, None);
    assert!(ordered
        .batches
        .iter()
        .any(|batch| batch.texture == Some(expected)));
    let mut cache = UiProjectionCache::default();
    cache.project(&composition, stage, &presentation, None);
    let initial = cache.stats;
    assert!(initial.glyphs_indexed > 0);
    for _ in 0..8 {
        cache.project(&composition, stage, &presentation, None);
    }
    assert_eq!(cache.stats.glyphs_indexed, initial.glyphs_indexed);
    assert_eq!(cache.stats.vertices_generated, initial.vertices_generated);
    let mut changed = composition.clone();
    Arc::make_mut(&mut Arc::make_mut(changed.glyph_plan.as_mut().unwrap()).quads[1])
        .font_bundle
        .as_mut()
        .unwrap()
        .1 = 10;
    let refreshed = cache.project(
        &changed,
        changed.stage(AuiCompositionStage::ScreenOverlay).unwrap(),
        &presentation,
        None,
    );
    assert!(refreshed.batches.iter().any(|b| b.texture
        == Some(RenderResourceHandle {
            generation: 10,
            ..expected
        })));
    assert_eq!(
        composition.glyph_plan.as_ref().unwrap().quads[1]
            .font_bundle
            .as_ref()
            .unwrap()
            .1,
        9
    );
    assert_eq!(cache.stats.stage_rebuilds, 2);
}

fn scene_with_view() -> RenderSceneState {
    let mut scene = RenderSceneState::new();
    scene.register_view(RenderViewState::new(
        RenderViewId(1),
        RenderViewKind::Game,
        RenderTargetKind::ViewportTexture,
    ));
    scene
}

fn b3_ui_buffers(output: &RuntimeRendererOutput) -> Vec<UiVertexStream> {
    output
        .rhi_command_plan
        .commands
        .iter()
        .filter_map(|command| match command {
            RhiCommand::Draw {
                payload: crate::rhi_command_plan::RhiDrawPayload::UiComposition { vertices, .. },
                ..
            } => Some(vertices.clone()),
            _ => None,
        })
        .collect()
}

fn b7_compare_projection(
    cache: &mut UiProjectionCache,
    composition: &AuiCompositionFrame,
    presentation: &ResolvedGameViewPresentation,
) -> UiProjectedOrderedBatches {
    let stage = composition
        .stage(AuiCompositionStage::ScreenOverlay)
        .unwrap();
    let actual = cache.project(composition, stage, presentation, None);
    let expected = ui_projection_ordered_batches(composition, stage, presentation, None);
    assert_eq!(actual.batches, expected.batches);
    assert_eq!(actual.diagnostics, expected.diagnostics);
    actual
}

#[test]
fn b7_single_item_changes_reuse_siblings_and_preserve_clips_order_and_batches() {
    let mut overlay = image_overlay(1, &[]);
    overlay.draw_items = (0..1001)
        .map(|i| {
            let mut item = ui_test_item(AuiComputedRect {
                x: i as f32 % 100.,
                y: 0.,
                width: 20.,
                height: 20.,
            });
            item.item_id = format!("rect-{i}");
            item.node_id = format!("node-{i}");
            item.sort_key.tree_order = i;
            Arc::new(item)
        })
        .collect::<Vec<_>>()
        .into();
    let mut composition = AuiCompositionFrame::from_overlay_frame(&overlay);
    let presentation = ui_test_presentation(800, 600);
    let mut cache = UiProjectionCache::default();
    b7_compare_projection(&mut cache, &composition, &presentation);
    for phase in 0..4 {
        let old = cache.stats;
        let item = Arc::make_mut(&mut Arc::make_mut(&mut composition.stages[0].draw_items)[500]);
        match phase {
            0 => item.rect.x += 10.,
            1 => item.color = Some("#ff000080".into()),
            2 => item.effective_clips.push(crate::aui::AuiResolvedClip {
                transform: None,
                node_id: "clip".into(),
                rect: AuiComputedRect {
                    x: 10.,
                    y: 0.,
                    width: 15.,
                    height: 20.,
                },
                radius: 4.,
            }),
            _ => item.effective_clips.clear(),
        }
        b7_compare_projection(&mut cache, &composition, &presentation);
        assert_eq!(cache.stats.items_projected - old.items_projected, 1);
        assert_eq!(cache.stats.fragments_reused - old.fragments_reused, 1000);
        assert_eq!(cache.stats.glyphs_indexed, 0);
        assert!(cache.stats.vertices_generated - old.vertices_generated < 1000);
    }
    // Reordering, deletion and insertion preserve the same transparent sequence and merging.
    Arc::make_mut(&mut composition.stages[0].draw_items).swap(0, 500);
    b7_compare_projection(&mut cache, &composition, &presentation);
    Arc::make_mut(&mut composition.stages[0].draw_items).remove(1);
    b7_compare_projection(&mut cache, &composition, &presentation);
    let mut image = composition.stages[0].draw_items[0].as_ref().clone();
    image.item_kind = AuiOverlayItemKind::Image;
    image.asset_id = Some("missing".into());
    Arc::make_mut(&mut composition.stages[0].draw_items).insert(1, Arc::new(image));
    let output = b7_compare_projection(&mut cache, &composition, &presentation);
    assert_eq!(output.diagnostics.len(), 1);
    let old = cache.stats;
    b7_compare_projection(&mut cache, &composition, &ui_test_presentation(400, 300));
    assert_eq!(cache.stats.items_projected - old.items_projected, 1001);
}

#[test]
fn b17_trusted_single_slot_hint_skips_stage_scan_but_external_cow_falls_back() {
    let mut overlay = image_overlay(1, &[]);
    overlay.draw_items = (0..1001)
        .map(|i| {
            let mut item = ui_test_item(AuiComputedRect {
                x: i as f32 % 100.,
                y: 0.,
                width: 20.,
                height: 20.,
            });
            item.item_id = format!("rect-{i}");
            item.node_id = format!("node-{i}");
            item.sort_key.tree_order = i;
            Arc::new(item)
        })
        .collect::<Vec<_>>()
        .into();
    let original = AuiCompositionFrame::from_overlay_frame(&overlay);
    let presentation = ui_test_presentation(800, 600);

    let mut hinted = original.clone();
    Arc::make_mut(&mut Arc::make_mut(&mut hinted.stages[0].draw_items)[500])
        .rect
        .x += 4.;
    hinted.install_projection_hint_from_previous_for_test(&original);
    let mut hinted_cache = UiProjectionCache::default();
    let original_projected = b7_compare_projection(&mut hinted_cache, &original, &presentation);
    let before = hinted_cache.stats;
    let changed_projected = b7_compare_projection(&mut hinted_cache, &hinted, &presentation);
    assert_eq!(hinted_cache.stats.items_compared - before.items_compared, 1);
    assert_eq!(
        hinted_cache.stats.items_projected - before.items_projected,
        1
    );
    assert_eq!(
        hinted_cache.stats.hint_slots_consumed - before.hint_slots_consumed,
        1
    );
    assert_eq!(
        hinted_cache.stats.fallback_stage_scans,
        before.fallback_stage_scans
    );
    assert_eq!(
        hinted_cache.stats.fragments_reused - before.fragments_reused,
        1000
    );
    assert_eq!(
        hinted_cache.stats.fragments_merged - before.fragments_merged,
        1001
    );
    assert_eq!(hinted_cache.stats.vertices_copied, before.vertices_copied);
    assert_eq!(original_projected.batches.len(), 1);
    assert_eq!(changed_projected.batches.len(), 1);
    assert_eq!(
        hinted_cache.stats.stream_chunks_reused - before.stream_chunks_reused,
        1000
    );
    assert_eq!(
        hinted_cache.stats.stream_chunks_changed - before.stream_chunks_changed,
        1
    );
    let previous_stream = &original_projected.batches[0].vertices;
    let changed_stream = &changed_projected.batches[0].vertices;
    assert_eq!(changed_stream.stream_id(), previous_stream.stream_id());
    let ranges = changed_stream
        .patch_ranges_from(previous_stream)
        .expect("single changed item must preserve trusted stream lineage");
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].vertex_count, 6);

    let mut external = original.clone();
    Arc::make_mut(&mut Arc::make_mut(&mut external.stages[0].draw_items)[500])
        .rect
        .x += 4.;
    let mut fallback_cache = UiProjectionCache::default();
    b7_compare_projection(&mut fallback_cache, &original, &presentation);
    let before = fallback_cache.stats;
    b7_compare_projection(&mut fallback_cache, &external, &presentation);
    assert_eq!(
        fallback_cache.stats.items_compared - before.items_compared,
        1001
    );
    assert_eq!(
        fallback_cache.stats.fallback_stage_scans - before.fallback_stage_scans,
        1
    );
    assert_eq!(
        fallback_cache.stats.hint_slots_consumed,
        before.hint_slots_consumed
    );
}

#[test]
fn b7_glyph_changes_only_project_their_item_and_clean_plan_keeps_its_index() {
    let mut overlay = image_overlay(1, &[]);
    let items: Vec<_> = (0..2)
        .map(|i| {
            let mut item = ui_test_item(AuiComputedRect {
                x: i as f32 * 30.,
                y: 0.,
                width: 20.,
                height: 20.,
            });
            item.item_kind = AuiOverlayItemKind::Text;
            item.item_id = format!("text-{i}");
            Arc::new(item)
        })
        .collect();
    let quads = items
        .iter()
        .map(|item| {
            Arc::new(crate::aui::AuiTextGlyphQuad {
                font_bundle: None,
                item_id: item.item_id.clone(),
                node_id: item.node_id.clone(),
                codepoint: 65,
                glyph_id: "A".into(),
                rect: item.rect,
                uv_rect: [0., 0., 1., 1.],
                page_index: 0,
                render_mode: FontBundleRenderMode::BitmapR8,
                clipped: false,
                color: None,
            })
        })
        .collect();
    overlay.draw_items = items.into();
    overlay.glyph_plan = Some(Arc::new(crate::aui::AuiTextGlyphPlan {
        font_atlas_id: "font".into(),
        font_source_kind: "project_font_bundle_v2".into(),
        font_asset_id: "font".into(),
        font_asset_status: "ready".into(),
        fallback_used: false,
        requested_glyph_count: 2,
        rendered_glyph_count: 2,
        unsupported_glyph_count: 0,
        clipped_glyph_count: 0,
        atlas_width: 16,
        atlas_height: 16,
        atlas_generation: 1,
        glyph_plan_hash: "unused".into(),
        quads,
    }));
    let mut composition = AuiCompositionFrame::from_overlay_frame(&overlay);
    let presentation = ui_test_presentation(800, 600);
    let mut cache = UiProjectionCache::default();
    b7_compare_projection(&mut cache, &composition, &presentation);
    for frame in 0..3 {
        let old = cache.stats;
        let quad =
            Arc::make_mut(&mut Arc::make_mut(composition.glyph_plan.as_mut().unwrap()).quads[0]);
        match frame {
            0 => quad.rect.x += 3.,
            1 => quad.color = Some("#f00".into()),
            _ => quad.uv_rect[0] = 0.25,
        }
        b7_compare_projection(&mut cache, &composition, &presentation);
        assert_eq!(cache.stats.items_projected - old.items_projected, 1);
        assert_eq!(cache.stats.fragments_reused - old.fragments_reused, 1);
    }
    let indexed = cache.stats.glyphs_indexed;
    Arc::make_mut(&mut Arc::make_mut(&mut composition.stages[0].draw_items)[0]).color =
        Some("#0f0".into());
    b7_compare_projection(&mut cache, &composition, &presentation);
    assert_eq!(cache.stats.glyphs_indexed, indexed);
    let old = cache.stats;
    Arc::make_mut(composition.glyph_plan.as_mut().unwrap()).atlas_generation = 2;
    b7_compare_projection(&mut cache, &composition, &presentation);
    assert_eq!(cache.stats.items_projected - old.items_projected, 2);
    Arc::make_mut(composition.glyph_plan.as_mut().unwrap())
        .quads
        .clear();
    assert!(
        b7_compare_projection(&mut cache, &composition, &presentation)
            .batches
            .is_empty()
    );
}

#[test]
fn b17_trusted_glyph_hint_updates_one_group_without_indexing_siblings() {
    let mut overlay = image_overlay(1, &[]);
    let items: Vec<_> = (0..1001)
        .map(|i| {
            let mut item = ui_test_item(AuiComputedRect {
                x: i as f32,
                y: 0.,
                width: 20.,
                height: 20.,
            });
            item.item_kind = AuiOverlayItemKind::Text;
            item.item_id = format!("text-{i}");
            Arc::new(item)
        })
        .collect();
    let quads = items
        .iter()
        .map(|item| {
            Arc::new(crate::aui::AuiTextGlyphQuad {
                font_bundle: None,
                item_id: item.item_id.clone(),
                node_id: item.node_id.clone(),
                codepoint: 65,
                glyph_id: "A".into(),
                rect: item.rect,
                uv_rect: [0., 0., 1., 1.],
                page_index: 0,
                render_mode: FontBundleRenderMode::BitmapR8,
                clipped: false,
                color: None,
            })
        })
        .collect();
    overlay.draw_items = items.into();
    overlay.glyph_plan = Some(Arc::new(crate::aui::AuiTextGlyphPlan {
        font_atlas_id: "font".into(),
        font_source_kind: "project_font_bundle_v2".into(),
        font_asset_id: "font".into(),
        font_asset_status: "ready".into(),
        fallback_used: false,
        requested_glyph_count: 1001,
        rendered_glyph_count: 1001,
        unsupported_glyph_count: 0,
        clipped_glyph_count: 0,
        atlas_width: 16,
        atlas_height: 16,
        atlas_generation: 1,
        glyph_plan_hash: "unused".into(),
        quads,
    }));
    let original = AuiCompositionFrame::from_overlay_frame(&overlay);
    let mut changed = original.clone();
    Arc::make_mut(&mut Arc::make_mut(changed.glyph_plan.as_mut().unwrap()).quads[500])
        .rect
        .x += 2.;
    changed.install_projection_hint_from_previous_for_test(&original);

    let presentation = ui_test_presentation(800, 600);
    let mut cache = UiProjectionCache::default();
    b7_compare_projection(&mut cache, &original, &presentation);
    let before = cache.stats;
    b7_compare_projection(&mut cache, &changed, &presentation);
    assert_eq!(cache.stats.items_compared - before.items_compared, 1);
    assert_eq!(cache.stats.items_projected - before.items_projected, 1);
    assert_eq!(cache.stats.glyphs_indexed, before.glyphs_indexed);
    assert_eq!(
        cache.stats.glyph_groups_updated - before.glyph_groups_updated,
        1
    );
    assert_eq!(
        cache.stats.hint_slots_consumed - before.hint_slots_consumed,
        1
    );
}

#[test]
fn b7_fragment_cache_does_not_retain_over_budget_item_sets() {
    let mut overlay = image_overlay(1, &[]);
    let mut item = ui_test_item(AuiComputedRect {
        x: 0.,
        y: 0.,
        width: 1.,
        height: 1.,
    });
    item.color = None;
    overlay.draw_items = vec![Arc::new(item); 16_385].into();
    let mut composition = AuiCompositionFrame::from_overlay_frame(&overlay);
    let presentation = ui_test_presentation(800, 600);
    let mut cache = UiProjectionCache::default();
    b7_compare_projection(&mut cache, &composition, &presentation);
    assert!(cache.stages.is_empty());
    Arc::make_mut(&mut composition.stages[0].draw_items).truncate(1);
    b7_compare_projection(&mut cache, &composition, &presentation);
    assert_eq!(cache.stages.len(), 1);
    let before = cache.stats;
    for _ in 0..8 {
        b7_compare_projection(&mut cache, &composition, &presentation);
    }
    assert_eq!(cache.stats.items_compared, before.items_compared);
    assert_eq!(cache.stats.items_projected, before.items_projected);
    assert_eq!(cache.stats.stage_hits - before.stage_hits, 8);
    Arc::make_mut(&mut composition.stages[0].draw_items).clear();
    b7_compare_projection(&mut cache, &composition, &presentation);
    assert!(cache
        .stages
        .values()
        .all(|s| s.fragments.is_empty() && s.projected.batches.is_empty()));
}

#[test]
fn ui_projection_reuses_bounded_alternating_presentations() {
    let composition = AuiCompositionFrame::from_overlay_frame(&aui_overlay(1));
    let mut alternate_composition = composition.clone();
    Arc::make_mut(&mut Arc::make_mut(&mut alternate_composition.stages[0].draw_items)[0]).color =
        Some("#4080bfff".to_string());
    let stage = &composition.stages[0];
    let alternate_stage = &alternate_composition.stages[0];
    let first = ui_test_presentation(800, 600);
    let alternate = ui_test_presentation(400, 300);
    let mut cache = UiProjectionCache::default();

    cache.project(&composition, stage, &first, None);
    cache.project(&alternate_composition, alternate_stage, &alternate, None);
    let before = cache.stats;
    cache.project(&composition, stage, &first, None);

    assert_eq!(cache.stats.stage_hits - before.stage_hits, 1);
    assert_eq!(cache.stats.stage_rebuilds, 2);

    let mut large_overlay = aui_overlay(2);
    large_overlay.draw_items = vec![large_overlay.draw_items[0].clone(); 4_097].into();
    let large = AuiCompositionFrame::from_overlay_frame(&large_overlay);
    let large_stage = &large.stages[0];
    let mut large_cache = UiProjectionCache::default();
    large_cache.project(&large, large_stage, &first, None);
    large_cache.project(&large, large_stage, &alternate, None);
    assert!(
        large_cache.stage_variants.is_empty(),
        "large stages must not retain a second projection"
    );
}

#[test]
fn b3_clean_projection_shares_rhi_vertices_and_invalidates_cow_clip_and_viewport() {
    let renderer = RuntimeRenderer::new();
    let scene = scene_with_view();
    let original = AuiCompositionFrame::from_overlay_frame(&aui_overlay(1));
    let render = |composition: &AuiCompositionFrame, width| {
        let mut request = input(1, &scene);
        request.aui_composition = Some(composition);
        request.render_target.width = width;
        renderer.build(request)
    };
    let first = render(&original, 800);
    let initial = renderer.ui_projection_stats();
    assert!(initial.items_projected > 0 && initial.vertices_generated > 0);
    let buffers = b3_ui_buffers(&first);
    for frame in 2..10 {
        let mut cloned = original.clone();
        cloned.frame_index = frame;
        let next = render(&cloned, 800);
        assert!(buffers[0].shares_data_with(&b3_ui_buffers(&next)[0]));
    }
    let clean = renderer.ui_projection_stats();
    assert_eq!(clean.stage_hits, 8);
    assert_eq!(clean.items_projected, initial.items_projected);
    assert_eq!(clean.mapping_items_scanned, initial.mapping_items_scanned);
    assert_eq!(clean.glyphs_indexed, initial.glyphs_indexed);
    assert_eq!(clean.vertices_generated, initial.vertices_generated);
    let graph_buffer = first
        .render_graph
        .passes
        .iter()
        .flat_map(|p| &p.commands)
        .find_map(|c| match c {
            RenderPassCommand::DrawUiComposition { vertices, .. } => Some(vertices),
            _ => None,
        })
        .unwrap();
    assert!(graph_buffer.shares_data_with(&buffers[0]));

    let mut changed = original.clone();
    Arc::make_mut(&mut Arc::make_mut(&mut changed.stages[0].draw_items)[0]).effective_clip_rect =
        Some(AuiComputedRect {
            x: 20.0,
            y: 30.0,
            width: 20.0,
            height: 20.0,
        });
    let clipped = render(&changed, 800);
    let localized = renderer.ui_projection_stats();
    assert_eq!(localized.items_projected - clean.items_projected, 1);
    assert_eq!(
        localized.fragments_reused - clean.fragments_reused,
        original.stages[0].draw_items.len() as u64 - 1
    );
    assert_ne!(buffers[0], b3_ui_buffers(&clipped)[0]);
    assert!(original.stages[0].draw_items[0]
        .effective_clip_rect
        .is_none());
    let resized = render(&changed, 400);
    // Stretch can leave NDC vertices equal; rebuilding is nevertheless required
    // because other scale policies and pixel-based shaping depend on the mapping.
    assert_eq!(renderer.ui_projection_stats().stage_rebuilds, 3);
    assert!(!resized.rhi_command_plan.has_errors());
    changed.canvas_references[0].reference_extent.width *= 2;
    let remapped = render(&changed, 400);
    assert_ne!(b3_ui_buffers(&resized)[0], b3_ui_buffers(&remapped)[0]);
    assert_eq!(renderer.ui_projection_stats().stage_rebuilds, 4);
    let json = serde_json::to_value(&changed).unwrap();
    assert!(json["stages"][0]["drawItems"].is_array());
    let decoded: AuiCompositionFrame = serde_json::from_value(json).unwrap();
    assert_eq!(changed, decoded);
    renderer.build(input(10, &scene));
    assert!(renderer.ui_projection.lock().unwrap().stages.is_empty());
}

fn gate_d_panel_frames() -> (
    crate::aui::AuiRuntimePresentOutput,
    crate::aui::AuiRuntimePresentOutput,
) {
    use crate::aui::*;
    use crate::aui_instance::AuiInstance;
    use project_runtime_sdk::aui::{UiProperty, UiValue, UiWrite};
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_style(AuiStyle::color("#000000"));
    root.layout = Some(crate::aui_layout_schema::LayoutSpec {
        mode: crate::aui_layout_schema::LayoutMode::Overlay,
        ..Default::default()
    });
    let mut nodes = Vec::new();
    for i in 0..400 {
        let id = format!("p-{i}");
        root.children.push(id.clone());
        let mut node = AuiNode::new(
            id,
            AuiNodeKind::Panel,
            if i == 0 {
                AuiRect::fixed_position(4., 4., 8., 8.)
            } else {
                AuiRect::fixed_position(40., 40., 2., 2.)
            },
        )
        .with_parent("root")
        .with_style(AuiStyle::color("#ff0000"));
        node.slot = Some(crate::aui_layout_schema::SlotSpec {
            participation: crate::aui_layout_schema::Participation::OutOfFlow,
            ..Default::default()
        });
        nodes.push(node);
    }
    nodes.insert(0, root);
    let mut document = AuiDocument::new(
        "gate-d-panels",
        vec![AuiCanvas::screen_overlay("main", 64., 64., "root")],
        nodes,
    );
    document.schema_version = crate::aui_layout_schema::AUI_LAYOUT_SCHEMA_VERSION.into();
    let mut instance = AuiInstance::mount(Arc::new(document)).unwrap();
    let mut runtime = AuiC2Runtime::default();
    let atlases = crate::runtime_package::RuntimeAuiFontAtlasRegistry::empty("gate-d");
    let bundles = crate::font_bundle::RuntimeFontBundleRegistry::default();
    let snapshot = |frame| {
        ProjectUiStateSnapshotOutput::new(
            "gate-d",
            AuiSnapshotSource::TestSnapshot,
            ProjectUiStateSnapshot::new(frame),
        )
    };
    runtime.observe_retained(Some(&instance));
    let first = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
        instance.source(),
        snapshot(1),
        &atlases,
        &bundles,
        None,
        &mut runtime,
    );
    assert_eq!(first.report.status, AuiRuntimePresentStatus::Success);
    let node = instance.node("p-0").unwrap();
    let mut edit = instance.edit();
    edit.write(
        node,
        UiProperty::Translation,
        UiValue::Translation([8., 0.]),
    )
    .unwrap();
    edit.commit().unwrap();
    runtime.observe_retained(Some(&instance));
    runtime.reuse_prepared_input_once = true;
    let before = runtime.drawing_stats();
    let changed = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
        instance.source(),
        snapshot(2),
        &atlases,
        &bundles,
        None,
        &mut runtime,
    );
    let delta = runtime.drawing_stats().since(before);
    assert_eq!(delta.nodes_visited, 1);
    assert_eq!(delta.items_visited, 1);
    assert_eq!(
        first
            .layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "p-0")
            .unwrap()
            .rect
            .x,
        4.
    );
    assert_eq!(
        changed
            .layout
            .computed_nodes
            .iter()
            .find(|node| node.node_id == "p-0")
            .unwrap()
            .rect
            .x,
        12.
    );
    (first, changed)
}

#[test]
fn gate_d_presenter_changes_reach_projection_chunks_without_full_discovery() {
    let (first, changed) = gate_d_panel_frames();
    let renderer = RuntimeRenderer::new();
    renderer.set_ui_measurement_enabled(true);
    let scene = RenderSceneState::new();
    let render = |composition: &AuiCompositionFrame, renderer: &RuntimeRenderer| {
        let mut request = input(1, &scene);
        request.aui_composition = Some(composition);
        request.render_target.width = 64;
        request.render_target.height = 64;
        renderer.build(request)
    };
    let old = render(&first.composition, &renderer);
    let before = renderer.ui_projection_stats();
    let next = render(&changed.composition, &renderer);
    let delta = renderer.ui_projection_stats().since(before);
    assert_eq!(delta.items_compared, 1);
    assert_eq!(delta.items_projected, 1);
    assert_eq!(delta.hint_slots_consumed, 1);
    assert_eq!(delta.fallback_stage_scans, 0);
    assert_eq!(delta.vertices_copied, 0);
    assert_eq!(
        b3_ui_buffers(&next),
        b3_ui_buffers(&render(&changed.composition, &RuntimeRenderer::new()))
    );
    assert!(b3_ui_buffers(&next)[0]
        .patch_ranges_from(&b3_ui_buffers(&old)[0])
        .is_some());
    assert!(delta.cpu_ns > 0);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real local WGPU adapter; absence fails explicit qualification"]
fn gate_d_real_gpu_presenter_partial_upload_and_backend_recovery_match_fresh_pixels() {
    use crate::wgpu_backend::real::RealWgpuBackend;
    let (first, changed) = gate_d_panel_frames();
    let renderer = RuntimeRenderer::new();
    let scene = RenderSceneState::new();
    let render = |composition: &AuiCompositionFrame, renderer: &RuntimeRenderer| {
        let mut request = input(1, &scene);
        request.aui_composition = Some(composition);
        request.render_target.width = 64;
        request.render_target.height = 64;
        renderer.build(request)
    };
    let old = render(&first.composition, &renderer);
    let next = render(&changed.composition, &renderer);
    let fresh = render(&changed.composition, &RuntimeRenderer::new());
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).expect("Gate D requires real GPU");
    backend.set_ui_measurement_enabled(true);
    let original_pixels = backend
        .render_plan_to_rgba_bytes(&old.rhi_command_plan, 64, 64)
        .unwrap();
    let before = backend.ui_vertex_upload_stats();
    let incremental_pixels = backend
        .render_plan_to_rgba_bytes(&next.rhi_command_plan, 64, 64)
        .unwrap();
    let after = backend.ui_vertex_upload_stats();
    assert_eq!(after.full_uploads, before.full_uploads);
    assert_eq!(after.partial_uploads - before.partial_uploads, 1);
    assert!(after.uploaded_bytes - before.uploaded_bytes < before.uploaded_bytes);
    let pixel =
        |pixels: &[u8], x: usize, y: usize| pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].to_vec();
    assert_eq!(pixel(&original_pixels, 6, 6), vec![255, 0, 0, 255]);
    assert_eq!(pixel(&incremental_pixels, 6, 6), vec![0, 0, 0, 255]);
    assert_eq!(pixel(&incremental_pixels, 14, 6), vec![255, 0, 0, 255]);
    let mut recovered = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let fresh_pixels = recovered
        .render_plan_to_rgba_bytes(&fresh.rhi_command_plan, 64, 64)
        .unwrap();
    assert_eq!(incremental_pixels, fresh_pixels);
    let recovered_pixels = recovered
        .render_plan_to_rgba_bytes(&next.rhi_command_plan, 64, 64)
        .unwrap();
    assert_eq!(recovered_pixels, fresh_pixels);
    assert!(
        recovered
            .ui_vertex_upload_stats()
            .full_upload_cold_or_evicted
            > 0
    );
    assert!(after.cpu_draw_buffer_prepare_ns > before.cpu_draw_buffer_prepare_ns);
    assert_eq!(
        recovered
            .ui_vertex_upload_stats()
            .cpu_draw_buffer_prepare_ns,
        0,
        "measurement Off does not collect upload clock intervals"
    );
    eprintln!(
        "gate_d_gpu: adapter={:?}, full_bytes={}, changed_bytes={}, partial_uploads={}",
        backend.device_context(),
        before.uploaded_bytes,
        after.uploaded_bytes - before.uploaded_bytes,
        after.partial_uploads - before.partial_uploads
    );
}

#[test]
fn b3_texture_generation_and_batch_order_preserve_unaffected_uploads() {
    let renderer = RuntimeRenderer::new();
    let scene = scene_with_view();
    let composition = AuiCompositionFrame::from_overlay_frame(&image_overlay(1, &["a", "b", "a"]));
    let a = font_atlas_render_handle("a");
    let b = font_atlas_render_handle("b");
    let mut textures = RuntimeTextureBindingContext::default();
    textures.insert("a", a, "nearestClamp");
    textures.insert("b", b, "nearestClamp");
    let render = |textures: &RuntimeTextureBindingContext| {
        let mut request = input(1, &scene);
        request.aui_composition = Some(&composition);
        request.runtime_texture_bindings = Some(textures);
        renderer.build(request)
    };
    let first = render(&textures);
    let first_buffers = b3_ui_buffers(&first);
    assert_eq!(
        first_buffers.len(),
        3,
        "transparent order must not merge separated a batches"
    );
    render(&textures.clone());
    assert_eq!(renderer.ui_projection_stats().stage_hits, 1);
    let old_textures = textures.clone();
    textures.insert(
        "a",
        RenderResourceHandle { generation: 2, ..a },
        "nearestClamp",
    );
    assert_eq!(old_textures.get("a").unwrap().handle.generation, 1);
    let changed = render(&textures);
    let changed_buffers = b3_ui_buffers(&changed);
    assert!(first_buffers[1].shares_data_with(&changed_buffers[1]));
    let handles: Vec<_> = changed
        .rhi_command_plan
        .commands
        .iter()
        .filter_map(|c| match c {
            RhiCommand::Draw {
                payload: crate::rhi_command_plan::RhiDrawPayload::UiComposition { texture, .. },
                ..
            } => *texture,
            _ => None,
        })
        .collect();
    assert_eq!(
        handles,
        vec![
            RenderResourceHandle { generation: 2, ..a },
            b,
            RenderResourceHandle { generation: 2, ..a }
        ]
    );
    assert_eq!(renderer.ui_projection_stats().stage_rebuilds, 2);
    assert_eq!(renderer.ui_projection_stats().batches_reused, 1);
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires a real local WGPU adapter; absence is a failure when explicitly run"]
fn b3_real_gpu_projection_upload_reuse_and_cow_pixel_refresh() {
    use crate::wgpu_backend::real::RealWgpuBackend;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).expect("B3 GPU required");
    let renderer = RuntimeRenderer::new();
    let scene = RenderSceneState::new();
    let mut overlay = aui_overlay(1);
    std::sync::Arc::make_mut(&mut overlay.draw_items).truncate(1);
    std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(&mut overlay.draw_items)[0]).rect =
        AuiComputedRect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
    std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(&mut overlay.draw_items)[0]).color =
        Some("#ff0000".into());
    let mut composition = AuiCompositionFrame::from_overlay_frame(&overlay);
    let render = |composition: &AuiCompositionFrame| {
        let mut request = input(1, &scene);
        request.aui_composition = Some(composition);
        request.render_target.width = 64;
        request.render_target.height = 64;
        renderer.build(request)
    };
    let first = render(&composition);
    let pixels = backend
        .render_plan_to_rgba_bytes(&first.rhi_command_plan, 64, 64)
        .unwrap();
    let center = (32 * 64 + 32) * 4;
    assert_eq!(&pixels[center..center + 4], &[255, 0, 0, 255]);
    let upload = backend.ui_vertex_upload_stats();
    assert_eq!(upload.uploads, 1);
    for _ in 0..8 {
        let next = render(&composition.clone());
        let pixels_next = backend
            .render_plan_to_rgba_bytes(&next.rhi_command_plan, 64, 64)
            .unwrap();
        assert_eq!(pixels_next, pixels);
    }
    let clean = backend.ui_vertex_upload_stats();
    assert_eq!(clean.uploads, upload.uploads);
    assert_eq!(clean.uploaded_bytes, upload.uploaded_bytes);
    assert_eq!(clean.hits, 8);
    Arc::make_mut(&mut Arc::make_mut(&mut composition.stages[0].draw_items)[0]).color =
        Some("#00ff00".into());
    let changed = render(&composition);
    let pixels = backend
        .render_plan_to_rgba_bytes(&changed.rhi_command_plan, 64, 64)
        .unwrap();
    assert_eq!(&pixels[center..center + 4], &[0, 255, 0, 255]);
    assert_eq!(backend.ui_vertex_upload_stats().uploads, 2);
    assert_eq!(backend.ui_vertex_upload_stats().resident_buffers, 1);
    let empty = renderer.build(input(10, &scene));
    backend
        .render_plan_to_rgba_bytes(&empty.rhi_command_plan, 64, 64)
        .unwrap();
    assert_eq!(backend.ui_vertex_upload_stats().resident_buffers, 0);
}

fn add_proxy(scene: &mut RenderSceneState, mesh_ref: Option<&str>) {
    let proxy = RenderProxy::new(
        RenderProxyId(0),
        RuntimeEntityId::new(1, 0),
        SourceEntityId::from("entity-a"),
        Transform {
            local_position: Vec3::ZERO,
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        },
        Renderable {
            mesh_ref: mesh_ref.map(str::to_string),
            material_ref: Some("material-a".to_string()),
            visible: true,
            layer: "default".to_string(),
        },
    );
    scene.insert_proxy(proxy);
}

fn add_sprite_proxy(scene: &mut RenderSceneState, sprite_ref: &str) {
    let mut proxy = RenderProxy::new(
        RenderProxyId(0),
        RuntimeEntityId::new(2, 0),
        SourceEntityId::from("entity-sprite"),
        Transform {
            local_position: Vec3::ZERO,
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        },
        Renderable {
            mesh_ref: None,
            material_ref: Some("material-sprite".to_string()),
            visible: true,
            layer: "default".to_string(),
        },
    );
    proxy.payload = RenderProxyPayload::Sprite(SpritePayload {
        sprite_ref: Some(sprite_ref.to_string()),
        material_ref: Some("material-sprite".to_string()),
        color: [1.0, 1.0, 1.0, 1.0],
        flip_x: false,
        flip_y: false,
        sorting_layer: 0,
        order_in_layer: 0,
        sort_z: 0.0,
    });
    scene.insert_proxy(proxy);
}

#[test]
fn empty_scene_generates_clear_and_present_graph() {
    let scene = scene_with_view();

    let output = RuntimeRenderer::new().build(input(1, &scene));

    assert_eq!(output.render_graph.passes.len(), 2);
    assert_eq!(output.render_frame_report.draw_item_count, 0);
    assert!(!output.rhi_command_plan.has_errors());
}

#[test]
fn scene_with_mesh_generates_draw_pass() {
    let mut scene = scene_with_view();
    add_proxy(&mut scene, Some("mesh-a"));

    let output = RuntimeRenderer::new().build(input(1, &scene));

    assert_eq!(output.render_frame_report.draw_item_count, 1);
    assert!(output
        .render_graph
        .passes
        .iter()
        .any(|pass| pass.pass_kind == RenderPassKind::DrawMeshBasic));
    assert!(output.rhi_command_plan.commands.iter().any(|command| {
        matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::MeshBasic,
                payload: crate::rhi_command_plan::RhiDrawPayload::MeshBasic { mesh_ref, .. },
                ..
            } if mesh_ref == "mesh-a"
        )
    }));
}

#[test]
fn missing_mesh_falls_back_to_test_geometry() {
    let mut scene = scene_with_view();
    add_proxy(&mut scene, None);

    let output = RuntimeRenderer::new().build(input(1, &scene));

    assert_eq!(output.render_frame_report.fallback_count, 1);
    assert!(output
        .render_graph
        .passes
        .iter()
        .any(|pass| pass.pass_kind == RenderPassKind::DrawTestGeometry));
}

#[test]
fn scene_with_sprite_generates_sprite_draw_pass() {
    let mut scene = scene_with_view();
    add_sprite_proxy(&mut scene, "sprite-a");

    let output = RuntimeRenderer::new().build(input(1, &scene));

    assert_eq!(output.render_frame_report.draw_item_count, 1);
    assert!(output
        .render_graph
        .passes
        .iter()
        .any(|pass| pass.pass_kind == RenderPassKind::DrawSpriteTextured));
    assert!(output.render_graph.passes.iter().any(|pass| {
        pass.commands.iter().any(|command| {
            matches!(
                command,
                RenderPassCommand::DrawSpriteTextured { sprite_ref, .. }
                    if sprite_ref == "sprite-a"
            )
        })
    }));
    assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::SpriteTextured,
                    payload: crate::rhi_command_plan::RhiDrawPayload::SpriteTextured { sprite_ref, binding: Some(_), .. },
                    ..
                } if sprite_ref == "sprite-a"
            )
        }));
    assert!(output
        .render_frame_report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "sprite_ready"));
    assert!(output
        .render_frame_report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "sprite_binding_fallback"));
}

#[test]
fn sprite_vertices_use_scene_view_orthographic_projection() {
    let mut scene = RenderSceneState::new();
    let mut view = RenderViewState::new(
        RenderViewId(1),
        RenderViewKind::Game,
        RenderTargetKind::ViewportTexture,
    );
    view.projection_matrix = orthographic_2d_projection(5.4, 9.6);
    scene.register_view(view);
    add_sprite_proxy(&mut scene, "sprite-a");

    let output = RuntimeRenderer::new().build(input(1, &scene));
    let vertices = output
        .render_graph
        .passes
        .iter()
        .flat_map(|pass| pass.commands.iter())
        .find_map(|command| match command {
            RenderPassCommand::DrawSpriteTextured { vertices, .. } => Some(vertices),
            _ => None,
        })
        .expect("Sprite2D vertices");

    assert!((vertices[0].position[0].to_f32() + 1.0 / 5.4).abs() < 0.0001);
    assert!((vertices[0].position[1].to_f32() + 1.0 / 9.6).abs() < 0.0001);
}

#[test]
fn sprite_vertices_keep_valid_identity_projection_from_scene_camera() {
    let mut scene = RenderSceneState::new();
    let mut view = RenderViewState::new(
        RenderViewId(1),
        RenderViewKind::Game,
        RenderTargetKind::ViewportTexture,
    );
    view.source_entity_id = Some(SourceEntityId::from("camera-unit-ortho"));
    scene.register_view(view);
    add_sprite_proxy(&mut scene, "sprite-a");

    let output = RuntimeRenderer::new().build(input(1, &scene));
    let first = output
        .render_graph
        .passes
        .iter()
        .flat_map(|pass| pass.commands.iter())
        .find_map(|command| match command {
            RenderPassCommand::DrawSpriteTextured { vertices, .. } => vertices.first(),
            _ => None,
        })
        .expect("Sprite2D first vertex");

    assert_eq!(first.position.map(|value| value.to_f32()), [-1.0, -1.0]);
}

#[test]
fn scene_with_sprite_uses_prepared_texture_binding_when_supplied() {
    let mut scene = scene_with_view();
    add_sprite_proxy(&mut scene, "sprite-a");
    let mut bindings = Sprite2DTextureBindingContext::new();
    bindings.insert_texture_handle(
        "sprite-a",
        RenderResourceHandle {
            kind: RenderResourceKind::Texture,
            index: 9,
            generation: 2,
        },
        "linearClamp",
    );
    // The first slot is deliberately unrelated. Both Graph and RHI must
    // select baseColorTexture by name, preserving index/generation below.
    let mut named = bindings.binding_for("sprite-a").unwrap();
    named.slots.insert(
        0,
        crate::render_contract::NamedRenderBinding::new(
            "auxiliaryTexture",
            RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 999,
                generation: 8,
            },
        ),
    );
    bindings.insert_binding("sprite-a", named);
    let mut input = input(1, &scene);
    input.sprite_texture_bindings = Some(&bindings);

    let output = RuntimeRenderer::new().build(input);

    assert!(output.render_graph.passes.iter().any(|pass| {
        pass.commands.iter().any(|command| {
            matches!(
                command,
                RenderPassCommand::DrawSpriteTextured {
                    texture: Some(RenderResourceHandle {
                        index: 9,
                        generation: 2,
                        ..
                    }),
                    fallback_used: false,
                    ..
                }
            )
        })
    }));
    assert!(output.rhi_command_plan.commands.iter().any(|command| {
        matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::SpriteTextured,
                payload: crate::rhi_command_plan::RhiDrawPayload::SpriteTextured {
                    texture: Some(RenderResourceHandle {
                        index: 9,
                        generation: 2,
                        ..
                    }),
                    binding: Some(_),
                    ..
                },
                ..
            }
        )
    }));
    assert!(output
        .render_frame_report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "sprite_texture_binding_ready"));
    assert!(!output
        .render_frame_report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "sprite_binding_fallback"));
}

#[test]
fn runtime_renderer_executes_mesh_through_engine_rhi_backend() {
    let mut scene = scene_with_view();
    add_proxy(&mut scene, Some("mesh-a"));
    let mut backend = HeadlessRhiBackend::new();

    let output = RuntimeRenderer::new().render_with_rhi_backend(input(1, &scene), &mut backend);
    let backend_report = output.rhi_backend_report.expect("backend report");

    assert_eq!(backend_report.backend_kind, "headless-rhi");
    assert_eq!(backend_report.draw_count, 1);
    assert_eq!(backend_report.present_count, 1);
    assert!(output.rhi_command_plan.commands.iter().any(|command| {
        matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::MeshBasic,
                payload: crate::rhi_command_plan::RhiDrawPayload::MeshBasic { mesh_ref, .. },
                ..
            } if mesh_ref == "mesh-a"
        )
    }));
}

#[test]
fn runtime_renderer_executes_sprite_through_engine_rhi_backend() {
    let mut scene = scene_with_view();
    add_sprite_proxy(&mut scene, "sprite-a");
    let mut backend = HeadlessRhiBackend::new();

    let output = RuntimeRenderer::new().render_with_rhi_backend(input(1, &scene), &mut backend);
    let backend_report = output.rhi_backend_report.expect("backend report");

    assert_eq!(backend_report.backend_kind, "headless-rhi");
    assert_eq!(backend_report.draw_count, 1);
    assert_eq!(backend_report.present_count, 1);
    assert!(output.rhi_command_plan.commands.iter().any(|command| {
            matches!(
                command,
                RhiCommand::Draw {
                    draw_kind: RhiDrawKind::SpriteTextured,
                    vertex_count: 6,
                    payload: crate::rhi_command_plan::RhiDrawPayload::SpriteTextured { sprite_ref, binding: Some(_), .. },
                    ..
                } if sprite_ref == "sprite-a"
            )
        }));
}

#[test]
fn runtime_renderer_does_not_panic_with_sprite_draw_item() {
    let mut scene = scene_with_view();
    add_sprite_proxy(&mut scene, "sprite-a");

    let output = RuntimeRenderer::new().render_headless(input(1, &scene));

    assert_eq!(output.render_frame_report.fallback_count, 0);
    assert_eq!(
        output
            .rhi_backend_report
            .as_ref()
            .expect("backend report")
            .draw_count,
        1
    );
}

#[test]
fn headless_end_to_end_outputs_backend_report() {
    let mut scene = scene_with_view();
    add_proxy(&mut scene, Some("mesh-a"));

    let output = RuntimeRenderer::new().render_headless(input(1, &scene));
    let backend_report = output.rhi_backend_report.expect("backend report");

    assert_eq!(backend_report.draw_count, 1);
    assert_eq!(backend_report.present_count, 1);
    assert_eq!(output.render_graph_report.error_count, 0);
}

#[test]
fn viewport_texture_target_outputs_descriptor() {
    let scene = scene_with_view();

    let output = RuntimeRenderer::new().build(viewport_input(7, &scene));
    let descriptor = output
        .texture_descriptor
        .as_ref()
        .expect("viewport texture descriptor");

    assert_eq!(descriptor.target_id, "viewport-scene");
    assert_eq!(descriptor.texture_id, "viewport-scene");
    assert_eq!(descriptor.frame_index, 7);
    assert_eq!(descriptor.width, 800);
    assert_eq!(descriptor.height, 450);
    assert_eq!(
        output.target_summary.texture_descriptor.as_ref(),
        Some(descriptor)
    );
}

#[test]
fn surface_target_does_not_emit_viewport_texture_descriptor() {
    let scene = scene_with_view();

    let output = RuntimeRenderer::new().render_headless(surface_input(9, &scene));

    assert_eq!(
        output.target_summary.target_kind,
        RuntimeRenderTargetKind::Surface
    );
    assert_eq!(output.target_summary.target_id, "surface-main");
    assert!(output.texture_descriptor.is_none());
    assert_eq!(
        output
            .rhi_backend_report
            .as_ref()
            .expect("backend report")
            .present_count,
        1
    );
}

#[test]
fn runtime_renderer_inserts_aui_overlay_pass_before_present() {
    let scene = scene_with_view();
    let overlay = aui_overlay(12);
    let input = RuntimeRendererInput {
        frame_index: 12,
        render_scene_state: &scene,
        render_view_state: None,
        aui_overlay: Some(&overlay),
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: target(),
    };

    let output = RuntimeRenderer::new().build(input);

    let ui_pass_index = output
        .render_graph
        .passes
        .iter()
        .position(|pass| pass.pass_kind == RenderPassKind::DrawUiComposition)
        .expect("ui composition pass");
    let present_index = output
        .render_graph
        .passes
        .iter()
        .position(|pass| pass.pass_kind == RenderPassKind::Present)
        .expect("present pass");
    assert!(ui_pass_index < present_index);
    assert_eq!(output.render_frame_report.draw_item_count, 2);
    assert_eq!(output.render_frame_report.ui_composition_stage_count, 1);
    assert_eq!(output.render_frame_report.ui_screen_overlay_item_count, 2);
    assert!(output.render_frame_report.ui_screen_overlay_pass_present);
    assert!(output.render_frame_report.ui_before_world_skipped);
    assert!(output.render_frame_report.ui_modal_skipped);
    assert!(matches!(
        output.render_graph.passes[ui_pass_index].commands[0],
        RenderPassCommand::DrawUiComposition {
            ref stage,
            item_count: 2,
            text_count: 1,
            image_count: 0,
            ..
        } if stage == "ScreenOverlay"
    ));
    let RenderPassCommand::DrawUiComposition { vertices, .. } =
        &output.render_graph.passes[ui_pass_index].commands[0]
    else {
        panic!("expected UI composition geometry command");
    };
    assert_eq!(vertices.len(), 6);
    assert_eq!(
        vertices
            .iter()
            .next()
            .unwrap()
            .color
            .map(|channel| channel.to_f32()),
        [0.015996292, 0.033104762, 0.057805434, 1.0]
    );
    assert!(vertices
        .iter()
        .any(|vertex| vertex.position[0].to_f32() < 0.0));
    assert!(output.rhi_command_plan.commands.iter().any(|command| {
        matches!(
            command,
            RhiCommand::Draw {
                draw_kind: RhiDrawKind::UiComposition,
                payload: crate::rhi_command_plan::RhiDrawPayload::UiComposition {
                    stage,
                    item_count: 2,
                    text_count: 1,
                    image_count: 0,
                    ..
                },
                ..
            } if stage == "ScreenOverlay"
        )
    }));
}

#[test]
fn runtime_renderer_orders_multi_stage_aui_composition_around_world() {
    let mut scene = scene_with_view();
    add_sprite_proxy(&mut scene, "sprite-a");
    let composition = aui_composition(13);
    let input = RuntimeRendererInput {
        frame_index: 13,
        render_scene_state: &scene,
        render_view_state: None,
        aui_overlay: None,
        aui_composition: Some(&composition),
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: target(),
    };

    let output = RuntimeRenderer::new().build(input);
    let pass_ids = output
        .render_graph
        .passes
        .iter()
        .map(|pass| pass.pass_id.as_str())
        .collect::<Vec<_>>();

    let before_index = pass_ids
        .iter()
        .position(|pass_id| *pass_id == "draw-aui-before-world")
        .expect("before world pass");
    let sprite_index = pass_ids
        .iter()
        .position(|pass_id| pass_id.starts_with("draw-sprite2d-"))
        .expect("sprite pass");
    let overlay_index = pass_ids
        .iter()
        .position(|pass_id| *pass_id == "draw-aui-screen-overlay")
        .expect("screen overlay pass");
    let modal_index = pass_ids
        .iter()
        .position(|pass_id| *pass_id == "draw-aui-modal")
        .expect("modal pass");
    let present_index = pass_ids
        .iter()
        .position(|pass_id| *pass_id == "present-main")
        .expect("present pass");

    assert!(before_index < sprite_index);
    assert!(sprite_index < overlay_index);
    assert!(overlay_index < modal_index);
    assert!(modal_index < present_index);
    assert_eq!(output.render_frame_report.ui_composition_stage_count, 3);
    assert_eq!(output.render_frame_report.ui_before_world_item_count, 1);
    assert_eq!(output.render_frame_report.ui_screen_overlay_item_count, 1);
    assert_eq!(output.render_frame_report.ui_modal_item_count, 1);
    assert!(output.render_frame_report.ui_before_world_pass_present);
    assert!(output.render_frame_report.ui_screen_overlay_pass_present);
    assert!(output.render_frame_report.ui_modal_pass_present);
}

#[test]
fn runtime_renderer_without_aui_overlay_keeps_existing_passes() {
    let scene = scene_with_view();

    let output = RuntimeRenderer::new().build(input(1, &scene));

    assert_eq!(output.render_graph.passes.len(), 2);
    assert!(!output
        .render_graph
        .passes
        .iter()
        .any(|pass| pass.pass_kind == RenderPassKind::DrawUiComposition));
    assert_eq!(output.render_frame_report.ui_composition_stage_count, 0);
    assert!(output.render_frame_report.ui_before_world_skipped);
    assert!(output.render_frame_report.ui_screen_overlay_skipped);
    assert!(output.render_frame_report.ui_modal_skipped);
}

#[test]
fn sprite_missing_ref_is_reported_without_test_geometry_fallback() {
    let mut scene = scene_with_view();
    let mut proxy = RenderProxy::new(
        RenderProxyId(0),
        RuntimeEntityId::new(3, 0),
        SourceEntityId::from("entity-sprite-missing"),
        Transform {
            local_position: Vec3::ZERO,
            local_rotation: Vec3::ZERO,
            local_scale: Vec3::ONE,
        },
        Renderable {
            mesh_ref: None,
            material_ref: Some("material-sprite".to_string()),
            visible: true,
            layer: "sprite2d".to_string(),
        },
    );
    proxy.payload = RenderProxyPayload::Sprite(SpritePayload::default());
    scene.insert_proxy(proxy);

    let output = RuntimeRenderer::new().build(input(1, &scene));

    assert_eq!(output.render_frame_report.draw_item_count, 0);
    assert_eq!(output.render_frame_report.fallback_count, 0);
    assert!(!output
        .render_graph
        .passes
        .iter()
        .any(|pass| pass.pass_kind == RenderPassKind::DrawTestGeometry));
    assert!(output
        .render_frame_report
        .diagnostics
        .iter()
        .any(|diagnostic| {
            diagnostic.code == "sprite_missing_ref"
                && diagnostic.severity == RuntimeRendererDiagnosticSeverity::Warning
        }));
}

#[test]
fn sprite_layer_mismatch_is_reported() {
    let mut scene = scene_with_view();
    let mut view = RenderViewState::new(
        RenderViewId(2),
        RenderViewKind::Game,
        RenderTargetKind::Window,
    );
    view.layer_mask = Some("gameplay".to_string());
    scene.register_view(view.clone());
    add_sprite_proxy(&mut scene, "sprite-a");

    let mut input = input(1, &scene);
    input.render_view_state = Some(&view);
    let output = RuntimeRenderer::new().build(input);

    assert_eq!(output.render_frame_report.draw_item_count, 0);
    assert!(output
        .render_frame_report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "sprite_layer_mismatch"));
}
