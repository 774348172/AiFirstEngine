use engine_runtime::aui::{
    AuiCanvas, AuiDocument, AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect, AuiRendererBridge,
};
use engine_runtime::aui_extension::{AuiCustomNodeSpec, AuiCustomRuntimeOutput};
use engine_runtime::aui_instance::AuiInstance;
use engine_runtime::aui_layout_runtime::AuiLayoutSolver;
use engine_runtime::aui_layout_schema::{
    Dimension, LayoutMode, LayoutSpec, SlotSpec, AUI_LAYOUT_SCHEMA_VERSION,
};
use engine_runtime::aui_paragraph::{AuiParagraphCache, AuiParagraphContext};
use engine_runtime::font_bundle::RuntimeFontBundleRegistry;
use engine_runtime::game_view_presentation::{
    CanvasReferenceFact, GameViewExtent, GameViewPresentationModule, GameViewPresentationSpec,
    GameViewRect, GameViewScalePolicy,
};
use engine_runtime::runtime_renderer::ui_projection_geometry_vertices;
use project_runtime_sdk::aui::UiNodeHandle;
use project_runtime_sdk::aui_extension::{
    UiExtensionChildRect, UiExtensionMeasureOutput, UiExtensionMeshVertex, UiExtensionPaintOutput,
    UiExtensionPaintPrimitive,
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn custom_node(id: &str, rect: AuiRect) -> AuiNode {
    let mut node = AuiNode::new(id, AuiNodeKind::Custom, rect);
    node.custom = Some(AuiCustomNodeSpec {
        type_id: "sample.custom".into(),
        schema_version: 1,
        properties: BTreeMap::new(),
    });
    node
}

fn c2_document(mut nodes: Vec<AuiNode>) -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Row,
        align_items: engine_runtime::aui_layout_schema::AlignItems::Start,
        ..Default::default()
    });
    root.children = nodes.iter().map(|node| node.node_id.clone()).collect();
    for node in &mut nodes {
        node.parent = Some("root".into());
        node.layout.get_or_insert(LayoutSpec {
            mode: LayoutMode::Row,
            ..Default::default()
        });
        node.slot.get_or_insert_with(SlotSpec::default);
    }
    let mut document = AuiDocument::new(
        "custom-ui",
        vec![AuiCanvas::screen_overlay("main", 320.0, 200.0, "root")],
        std::iter::once(root).chain(nodes).collect(),
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

#[test]
fn custom_measure_changes_the_existing_taffy_solver_result() {
    let mut measured = custom_node("measured", AuiRect::fixed_position(0.0, 0.0, 0.0, 0.0));
    measured.custom_runtime = Some(AuiCustomRuntimeOutput {
        measure: Some(UiExtensionMeasureOutput {
            size: [83.0, 37.0],
            first_baseline: Some(29.0),
        }),
        ..Default::default()
    });
    let mut document = c2_document(vec![measured]);
    let mut solver = AuiLayoutSolver::default();
    let mut cache = AuiParagraphCache::default();
    let fonts = RuntimeFontBundleRegistry::default();
    let mut measure = AuiParagraphContext::new(&document, &fonts, &mut cache);
    let snapshot = solver.solve(&document, "main", 1, &mut measure).unwrap();
    let rect = snapshot.node("measured").unwrap().rect;
    assert_eq!([rect.width, rect.height], [83.0, 37.0]);
    drop(measure);
    let mut runtime = engine_runtime::aui::AuiC2Runtime::default();
    runtime.layout(&document, 1, &fonts);
    let builds = runtime.solver_build_count();
    document.nodes[1].custom_runtime.as_mut().unwrap().measure = Some(UiExtensionMeasureOutput {
        size: [101., 43.],
        first_baseline: Some(30.),
    });
    let mut measure = AuiParagraphContext::new(&document, &fonts, &mut cache);
    let changed = solver.solve(&document, "main", 2, &mut measure).unwrap();
    assert_eq!(changed.node("measured").unwrap().rect.width, 101.);
    assert_eq!(changed.node("measured").unwrap().rect.height, 43.);
    assert_eq!(solver.last_stats().source_nodes_loaded, 0);
    let (incremental, _) = runtime.layout(&document, 2, &fonts);
    let (fresh, _) = engine_runtime::aui::AuiC2Runtime::default().layout(&document, 2, &fonts);
    assert_eq!(incremental, fresh);
    assert_eq!(runtime.solver_build_count(), builds);
}

#[test]
fn radial_custom_layout_overrides_only_direct_child_rects() {
    let mut radial = custom_node("radial", AuiRect::fixed_position(0.0, 0.0, 120.0, 120.0));
    radial.layout = Some(LayoutSpec {
        mode: LayoutMode::Overlay,
        ..Default::default()
    });
    radial.slot = Some(SlotSpec {
        width: Dimension::Fixed { value: 120.0 },
        height: Dimension::Fixed { value: 120.0 },
        ..Default::default()
    });
    radial.children = vec!["east".into(), "south".into()];
    radial.custom_runtime = Some(AuiCustomRuntimeOutput {
        child_rects: vec![
            UiExtensionChildRect {
                handle: UiNodeHandle {
                    instance: 1,
                    slot: 2,
                    generation: 1,
                },
                rect: [90.0, 50.0, 20.0, 20.0],
            },
            UiExtensionChildRect {
                handle: UiNodeHandle {
                    instance: 1,
                    slot: 3,
                    generation: 1,
                },
                rect: [50.0, 90.0, 20.0, 20.0],
            },
        ],
        ..Default::default()
    });
    let mut east = AuiNode::new(
        "east",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 20.0, 20.0),
    );
    let mut south = AuiNode::new(
        "south",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 20.0, 20.0),
    );
    east.parent = Some("radial".into());
    south.parent = Some("radial".into());
    east.slot = Some(SlotSpec::default());
    south.slot = Some(SlotSpec::default());
    let mut document = c2_document(vec![radial, east, south]);
    document.nodes[0].children = vec!["radial".into()];
    document.nodes[2].parent = Some("radial".into());
    document.nodes[3].parent = Some("radial".into());
    let mut solver = AuiLayoutSolver::default();
    let mut cache = AuiParagraphCache::default();
    let fonts = RuntimeFontBundleRegistry::default();
    let mut measure = AuiParagraphContext::new(&document, &fonts, &mut cache);
    let snapshot = solver.solve(&document, "main", 1, &mut measure).unwrap();
    let radial_rect = snapshot.node("radial").unwrap().rect;
    let east_rect = snapshot.node("east").unwrap().rect;
    let south_rect = snapshot.node("south").unwrap().rect;
    assert_eq!(
        [east_rect.x - radial_rect.x, east_rect.y - radial_rect.y],
        [90.0, 50.0]
    );
    assert_eq!(
        [south_rect.x - radial_rect.x, south_rect.y - radial_rect.y],
        [50.0, 90.0]
    );
}

#[test]
fn circular_progress_mesh_reaches_the_real_ui_projection_vertices() {
    let mut circle = custom_node("circle", AuiRect::fixed_position(20.0, 20.0, 80.0, 80.0));
    circle.custom_runtime = Some(AuiCustomRuntimeOutput {
        paint: Some(UiExtensionPaintOutput {
            vertices: vec![
                UiExtensionMeshVertex {
                    position: [40.0, 40.0],
                    uv: [0.5, 0.5],
                    color: [0.1, 0.8, 0.3, 1.0],
                },
                UiExtensionMeshVertex {
                    position: [40.0, 0.0],
                    uv: [0.5, 0.0],
                    color: [0.1, 0.8, 0.3, 1.0],
                },
                UiExtensionMeshVertex {
                    position: [80.0, 40.0],
                    uv: [1.0, 0.5],
                    color: [0.1, 0.8, 0.3, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
            asset_id: None,
            primitives: vec![],
        }),
        ..Default::default()
    });
    let document = AuiDocument::new(
        "circle-ui",
        vec![AuiCanvas::screen_overlay("main", 200.0, 120.0, "circle")],
        vec![circle],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (draw, _) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    let composition = AuiRendererBridge::build_composition_frame(1, &document, &layout, &draw);
    let stage = composition
        .stages
        .iter()
        .find(|stage| !stage.draw_items.is_empty())
        .unwrap();
    assert_eq!(
        stage.draw_items[0].mesh.as_ref().unwrap().indices,
        vec![0, 1, 2]
    );
    let presentation = GameViewPresentationModule::resolve(GameViewPresentationSpec {
        session_id: "test".into(),
        target_id: "target".into(),
        target_extent: GameViewExtent::new(200, 120),
        display_rect: GameViewRect::new(0.0, 0.0, 200.0, 120.0),
        scale_policy: GameViewScalePolicy::Stretch,
        surface_generation: 1,
        presentation_revision: 1,
        canvas_references: vec![CanvasReferenceFact::new("main", 200, 120)],
    })
    .unwrap();
    let vertices = ui_projection_geometry_vertices(stage, &presentation);
    assert_eq!(
        vertices.len(),
        3,
        "indexed custom triangle must reach renderer vertices"
    );
}

#[test]
fn custom_rect_image_and_text_use_the_existing_draw_paths() {
    let mut custom = custom_node("paint", AuiRect::fixed_position(10.0, 20.0, 100.0, 80.0));
    custom.custom_runtime = Some(AuiCustomRuntimeOutput {
        paint: Some(UiExtensionPaintOutput {
            vertices: vec![],
            indices: vec![],
            asset_id: None,
            primitives: vec![
                UiExtensionPaintPrimitive::Rect {
                    rect: [1.0, 2.0, 30.0, 20.0],
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                UiExtensionPaintPrimitive::Image {
                    rect: [35.0, 2.0, 20.0, 20.0],
                    asset_id: "ui.icon".into(),
                    color: [1.0; 4],
                },
                UiExtensionPaintPrimitive::Text {
                    rect: [1.0, 30.0, 80.0, 20.0],
                    text: "守城".into(),
                    color: [0.0, 1.0, 0.0, 1.0],
                    font_size: 18.0,
                },
            ],
        }),
        ..Default::default()
    });
    let document = AuiDocument::new(
        "custom-paint",
        vec![AuiCanvas::screen_overlay("main", 200.0, 120.0, "paint")],
        vec![custom],
    );
    let layout = AuiLayoutEngine::layout(&document, 1);
    let (draw, report) = AuiLayoutEngine::extract_draw_list(&document, &layout);
    assert_eq!(draw.commands.len(), 3);
    assert!(matches!(
        draw.commands[0].as_ref(),
        engine_runtime::aui::AuiDrawCommand::DrawRect { .. }
    ));
    assert!(
        matches!(draw.commands[1].as_ref(), engine_runtime::aui::AuiDrawCommand::DrawImage { asset_id, .. } if asset_id == "ui.icon")
    );
    assert!(
        matches!(draw.commands[2].as_ref(), engine_runtime::aui::AuiDrawCommand::DrawText { text, font_size: Some(18.0), .. } if text == "守城")
    );
    assert_eq!(report.image_count, 1);
    assert_eq!(report.text_count, 1);
}

#[test]
fn custom_node_without_a_stable_spec_is_rejected_at_mount() {
    let document = AuiDocument::new(
        "invalid-custom",
        vec![AuiCanvas::screen_overlay("main", 100.0, 100.0, "custom")],
        vec![AuiNode::new(
            "custom",
            AuiNodeKind::Custom,
            AuiRect::fixed_position(0.0, 0.0, 40.0, 40.0),
        )],
    );
    let diagnostics = match AuiInstance::mount(Arc::new(document)) {
        Ok(_) => panic!("custom node without a spec must fail mounting"),
        Err(diagnostics) => diagnostics,
    };
    assert!(diagnostics.iter().any(|item| {
        item.code == "aui_custom_node_schema_invalid" && item.node_id.as_deref() == Some("custom")
    }));
}
