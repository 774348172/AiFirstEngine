use engine_runtime::aui::{
    AuiCanvas, AuiDocument, AuiDrawCommand, AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect,
    AuiRuntimePresentStatus, AuiRuntimePresenter, AuiScaleMode,
};
use engine_runtime::aui_layout_schema::{
    LayoutMode, LayoutSpec, LayoutVisibility, SlotSpec, AUI_LAYOUT_SCHEMA_VERSION,
};
use engine_runtime::aui_viewport::{
    AuiSafeAreaPolicy, AuiViewportCondition, AuiViewportInput, AuiViewportInsets,
    AuiViewportLayoutSpec, AuiViewportOrientation, AuiViewportResolver,
};

fn viewport_document() -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full())
        .with_children(["wide", "portrait"]);
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Column,
        ..Default::default()
    });
    let mut wide = AuiNode::new(
        "wide",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 10.0, 10.0),
    )
    .with_parent("root")
    .with_viewport_condition("wide");
    wide.slot = Some(SlotSpec {
        min_height: 64.0,
        ..Default::default()
    });
    let mut portrait = AuiNode::new(
        "portrait",
        AuiNodeKind::Button,
        AuiRect::fixed_position(0.0, 0.0, 10.0, 10.0),
    )
    .with_parent("root")
    .with_viewport_condition("portrait");
    portrait.slot = Some(SlotSpec {
        min_height: 80.0,
        ..Default::default()
    });
    let canvas = AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root").with_viewport_layout(
        AuiViewportLayoutSpec {
            safe_area_policy: AuiSafeAreaPolicy::InsetRoot,
            conditions: vec![
                AuiViewportCondition::new("wide")
                    .with_min_width(900.0)
                    .with_orientation(AuiViewportOrientation::Landscape),
                AuiViewportCondition::new("portrait")
                    .with_max_width(700.0)
                    .with_orientation(AuiViewportOrientation::Portrait),
            ],
        },
    );
    let mut document = AuiDocument::new("viewport-c2", vec![canvas], vec![root, wide, portrait]);
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.to_string();
    document
}

fn input(
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
    safe_insets: AuiViewportInsets,
) -> AuiViewportInput {
    AuiViewportInput::new(physical_width, physical_height, scale_factor)
        .with_physical_safe_insets(safe_insets)
}

fn assert_close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() <= 0.01, "{actual} != {expected}");
}

fn node<'a>(document: &'a AuiDocument, node_id: &str) -> &'a AuiNode {
    document
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .unwrap()
}

#[test]
fn dpi_and_safe_area_resolve_into_one_logical_coordinate_space() {
    let resolved = AuiViewportResolver::resolve_document(
        &viewport_document(),
        &input(
            2400,
            1600,
            2.0,
            AuiViewportInsets::new(40.0, 20.0, 80.0, 40.0),
        ),
    )
    .unwrap();
    let canvas = resolved.canvas("main").unwrap();
    assert_close(canvas.logical_viewport.width, 1200.0);
    assert_close(canvas.logical_viewport.height, 800.0);
    assert_close(canvas.safe_rect.x, 20.0);
    assert_close(canvas.safe_rect.y, 40.0);
    assert_close(canvas.safe_rect.width, 1170.0);
    assert_close(canvas.safe_rect.height, 740.0);
    assert_eq!(canvas.orientation, AuiViewportOrientation::Landscape);
    assert_eq!(canvas.matched_condition_ids, vec!["wide"]);

    let root_rect = AuiLayoutEngine::layout(&resolved.document, 1)
        .computed_nodes
        .into_iter()
        .find(|node| node.node_id == "root")
        .unwrap()
        .rect;
    assert_eq!(root_rect, canvas.safe_rect);
}

#[test]
fn rotation_reselects_explicit_root_viewport_conditions() {
    let source = viewport_document();
    let landscape = AuiViewportResolver::resolve_document(
        &source,
        &input(1200, 800, 1.0, AuiViewportInsets::default()),
    )
    .unwrap();
    let portrait = AuiViewportResolver::resolve_document(
        &source,
        &input(600, 1000, 1.0, AuiViewportInsets::default()),
    )
    .unwrap();

    assert_eq!(node(&landscape.document, "wide").layout_visibility, None);
    assert_eq!(
        node(&landscape.document, "portrait").layout_visibility,
        Some(LayoutVisibility::Collapsed)
    );
    assert_eq!(
        node(&portrait.document, "wide").layout_visibility,
        Some(LayoutVisibility::Collapsed)
    );
    assert_eq!(node(&portrait.document, "portrait").layout_visibility, None);
    assert_eq!(
        portrait.canvas("main").unwrap().matched_condition_ids,
        vec!["portrait"]
    );
}

#[test]
fn responsive_conditions_reject_ambiguous_or_missing_references_atomically() {
    let source = viewport_document();
    let original = source.clone();
    let mut ambiguous = source.clone();
    ambiguous.canvases[0]
        .viewport_layout
        .as_mut()
        .unwrap()
        .conditions
        .push(AuiViewportCondition::new("wide").with_min_width(500.0));
    let error = AuiViewportResolver::resolve_document(
        &ambiguous,
        &input(1200, 800, 1.0, AuiViewportInsets::default()),
    )
    .unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.viewport.condition_duplicate"));

    let mut missing = source.clone();
    missing.nodes[1].viewport_condition = Some("missing".to_string());
    let error = AuiViewportResolver::resolve_document(
        &missing,
        &input(1200, 800, 1.0, AuiViewportInsets::default()),
    )
    .unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.viewport.condition_missing"));
    assert_eq!(source, original);
}

#[test]
fn invalid_dpi_or_safe_area_fails_without_a_partial_document() {
    let source = viewport_document();
    let invalid_dpi = input(1200, 800, 0.0, AuiViewportInsets::default());
    let error = AuiViewportResolver::resolve_document(&source, &invalid_dpi).unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.viewport.scale_factor_invalid"));

    let unrepresentable_dpi = input(
        1200,
        800,
        f64::from(f32::MAX) * 2.0,
        AuiViewportInsets::default(),
    );
    let error = AuiViewportResolver::resolve_document(&source, &unrepresentable_dpi).unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.viewport.scale_factor_invalid"));

    let swallowed = input(
        1200,
        800,
        1.0,
        AuiViewportInsets::new(700.0, 600.0, 0.0, 0.0),
    );
    let error = AuiViewportResolver::resolve_document(&source, &swallowed).unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.viewport.safe_area_invalid"));
    assert_eq!(source.canvases[0].reference_resolution.x, 1280.0);
}

#[test]
fn repeated_resolution_restores_original_visibility_and_rejects_shared_responsive_subtrees() {
    let source = viewport_document();
    let landscape = AuiViewportResolver::resolve_document(
        &source,
        &input(1200, 800, 1.0, AuiViewportInsets::default()),
    )
    .unwrap();
    let portrait = AuiViewportResolver::resolve_document(
        &landscape.document,
        &input(600, 1000, 1.0, AuiViewportInsets::default()),
    )
    .unwrap();
    assert_eq!(
        node(&portrait.document, "wide").layout_visibility,
        Some(LayoutVisibility::Collapsed)
    );
    assert_eq!(node(&portrait.document, "portrait").layout_visibility, None);

    let mut ambiguous = source;
    let mut second_canvas = ambiguous.canvases[0].clone();
    second_canvas.canvas_id = "second".to_string();
    ambiguous.canvases.push(second_canvas);
    let error = AuiViewportResolver::resolve_document(
        &ambiguous,
        &input(1200, 800, 1.0, AuiViewportInsets::default()),
    )
    .unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.viewport.condition_multiple_canvas_owners"));
}

#[test]
fn reference_canvas_ignores_viewport_reflow_and_keeps_legacy_extent() {
    let root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    let mut source = AuiDocument::new(
        "reference",
        vec![AuiCanvas::screen_overlay("main", 1280.0, 720.0, "root")],
        vec![root],
    );
    source.schema_version = AUI_LAYOUT_SCHEMA_VERSION.to_string();
    let resolved = AuiViewportResolver::resolve_document(
        &source,
        &input(
            600,
            1000,
            2.0,
            AuiViewportInsets::new(20.0, 20.0, 40.0, 40.0),
        ),
    )
    .unwrap();
    assert_eq!(resolved.document, source);
    assert_eq!(
        resolved.document.canvases[0].scale_mode,
        AuiScaleMode::ConstantPixelSize
    );
    assert_eq!(resolved.document.canvases[0].reference_resolution.x, 1280.0);
}

#[test]
fn viewport_root_geometry_is_shared_by_draw_and_hit_testing() {
    let resolved = AuiViewportResolver::resolve_document(
        &viewport_document(),
        &input(
            1200,
            800,
            1.0,
            AuiViewportInsets::new(20.0, 10.0, 30.0, 40.0),
        ),
    )
    .unwrap();
    let layout = AuiLayoutEngine::layout(&resolved.document, 3);
    let root = layout
        .computed_nodes
        .iter()
        .find(|node| node.node_id == "root")
        .unwrap();
    let (draw, _) = AuiLayoutEngine::extract_draw_list(&resolved.document, &layout);
    let root_draw_rect = draw
        .commands
        .iter()
        .find_map(|command| match command.as_ref() {
            AuiDrawCommand::DrawRect { node_id, rect, .. }
            | AuiDrawCommand::DrawImage { node_id, rect, .. }
            | AuiDrawCommand::DrawText { node_id, rect, .. }
                if node_id == "root" =>
            {
                Some(*rect)
            }
            _ => None,
        });
    assert_eq!(root_draw_rect, Some(root.rect));
    assert!(root.rect.contains(20.0, 30.0));
    assert!(!root.rect.contains(19.0, 29.0));
}

#[test]
fn viewport_contract_roundtrips_and_present_requires_explicit_input() {
    let source = viewport_document();
    let value = serde_json::to_value(&source).unwrap();
    assert!(value["canvases"][0].get("viewportLayout").is_some());
    assert_eq!(value["nodes"][1]["viewportCondition"], "wide");
    assert_eq!(
        serde_json::from_value::<AuiDocument>(value).unwrap(),
        source
    );

    let present = AuiRuntimePresenter::present_package_smoke(&source, 1);
    assert_eq!(present.report.status, AuiRuntimePresentStatus::Failed);
    assert_eq!(present.report.viewport_status, "error");
    assert!(present
        .report
        .diagnostics
        .iter()
        .any(|item| item.code == "aui.viewport.input_required"));
}
