use engine_runtime::aui::*;
use engine_runtime::aui_layout_schema::*;

fn document() -> AuiDocument {
    let mut root = AuiNode::new(
        "root",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 320.0, 240.0),
    );
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Column,
        ..Default::default()
    });
    let mut doc = AuiDocument::new(
        "c2",
        vec![AuiCanvas::screen_overlay("main", 320.0, 240.0, "root")],
        vec![root],
    );
    doc.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    doc
}
fn has(doc: &AuiDocument, code: &str) -> bool {
    validate(doc).iter().any(|v| v.code == code)
}

#[test]
fn c2_roundtrip_preserves_typed_fields_and_legacy_omits_them() {
    let doc = document();
    let bytes = serde_json::to_vec(&doc).unwrap();
    assert_eq!(serde_json::from_slice::<AuiDocument>(&bytes).unwrap(), doc);
    let node = AuiNode::new(
        "old",
        AuiNodeKind::Panel,
        AuiRect::fixed_position(0.0, 0.0, 1.0, 1.0),
    );
    let value = serde_json::to_value(node).unwrap();
    for name in [
        "layout",
        "slot",
        "textLayout",
        "layoutVisibility",
        "overflow",
    ] {
        assert!(value.get(name).is_none());
    }
    assert!(validate(&doc).is_empty());
}
#[test]
fn strict_new_fields_reject_misspelled_or_invalid_tagged_values() {
    let mut value = serde_json::to_value(document()).unwrap();
    value["nodes"][0]["layout"]["gapp"] = serde_json::json!(2);
    assert!(serde_json::from_value::<AuiDocument>(value).is_err());
    assert!(
        serde_json::from_value::<Dimension>(serde_json::json!({"kind":"content","value":7}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<GridTrack>(serde_json::json!({"kind":"content","value":7}))
            .is_err()
    );
    let slot: SlotSpec =
        serde_json::from_value(serde_json::json!({"alignSelf":"inherit"})).unwrap();
    assert_eq!(slot.align_self, None);
}
#[test]
fn source_normalization_preserves_future_version_rejection_and_c2_identity() {
    let mut doc = document();
    doc.schema_version = "aui-document.v2".into();
    assert!(has(&doc, "aui_layout_requires_v3"));
    normalize_source_version(&mut doc).unwrap();
    assert_eq!(doc.schema_version, AUI_LAYOUT_SCHEMA_VERSION);
    doc.schema_version = "aui-document.v999".into();
    assert!(normalize_source_version(&mut doc).is_err());
    assert_eq!(doc.schema_version, "aui-document.v999");
}
#[test]
fn numeric_and_grid_constraints_reject_nan_negative_and_recursive_minmax() {
    let mut doc = document();
    doc.nodes[0].layout.as_mut().unwrap().padding.left = f32::NAN;
    assert!(has(&doc, "aui_layout_metrics_invalid"));
    doc.nodes[0].layout = Some(LayoutSpec::default());
    doc.nodes[0].slot = Some(SlotSpec {
        min_width: 10.0,
        max_width: Some(5.0),
        ..Default::default()
    });
    assert!(has(&doc, "aui_slot_invalid"));
    doc.nodes[0].slot = Some(SlotSpec {
        grid_column: GridPlacement {
            start: Some(0),
            span: 1,
        },
        ..Default::default()
    });
    assert!(has(&doc, "aui_slot_invalid"));
    let nested = GridTrack::Minmax {
        min: Box::new(GridTrack::Content),
        max: Box::new(GridTrack::Minmax {
            min: Box::new(GridTrack::Content),
            max: Box::new(GridTrack::Content),
        }),
    };
    doc.nodes[0].layout.as_mut().unwrap().grid_columns = vec![nested];
    assert!(has(&doc, "aui_layout_metrics_invalid"));
}
#[test]
fn cyclic_inconsistent_or_missing_children_are_diagnosed_without_recursion() {
    let mut doc = document();
    doc.nodes[0].children = vec!["missing".into()];
    assert!(has(&doc, "aui_layout_missing_child"));
    doc.nodes[0].children = vec!["root".into()];
    doc.nodes[0].parent = Some("root".into());
    assert!(has(&doc, "aui_layout_cycle"));
    doc.nodes[0].children.clear();
    assert!(has(&doc, "aui_layout_parent_mismatch"));
}
#[test]
fn text_limits_and_negative_line_height_are_rejected() {
    let mut doc = document();
    doc.nodes[0].text_layout = Some(TextLayoutSpec {
        line_height: LineHeight::Fixed { value: -1.0 },
        ..Default::default()
    });
    assert!(has(&doc, "aui_text_layout_invalid"));
    doc.nodes[0].text_layout = Some(TextLayoutSpec {
        max_lines: Some(0),
        ..Default::default()
    });
    assert!(has(&doc, "aui_text_layout_invalid"));
    doc.nodes[0].text_layout = None;
    doc.nodes[0].text = Some("a".repeat(65_537));
    assert!(has(&doc, "aui_text_layout_invalid"));
}
#[test]
fn legacy_shape_cannot_silently_drop_nested_c2_fields() {
    assert!(validate_legacy_source(
        &serde_json::json!({"root":{"children":[{"layout":{"mode":"row"}}]}})
    )
    .is_err());
    assert!(validate_legacy_source(
        &serde_json::json!({"schemaVersion":"aui-document.v999","root":{}})
    )
    .is_err());
    assert!(validate_legacy_source(&serde_json::json!({"root":{"text":"old"}})).is_ok());
}
#[test]
fn existing_validation_entry_exposes_c2_errors() {
    let mut doc = document();
    doc.nodes[0].slot = Some(SlotSpec {
        flex_grow: -1.0,
        ..Default::default()
    });
    assert!(!AuiLayoutEngine::validate(&doc, None).ok);
}

#[test]
fn activated_c2_uses_solver_geometry_and_future_schema_stays_rejected() {
    let doc = document();
    let layout = AuiLayoutEngine::layout(&doc, 7);
    assert!(layout.diagnostics.is_empty());
    assert_eq!(layout.computed_nodes.len(), 1);
    assert_eq!(layout.computed_nodes[0].rect.width, 320.0);
    let present = AuiRuntimePresenter::present_package_smoke(&doc, 7);
    assert_eq!(present.report.status, AuiRuntimePresentStatus::Success);
    assert!(present.report.diagnostics.is_empty());
    let mut future = doc;
    future.nodes[0].layout = None;
    future.schema_version = "aui-document.v999".into();
    let layout = AuiLayoutEngine::layout(&future, 8);
    assert!(layout.computed_nodes.is_empty());
    assert_eq!(layout.diagnostics[0].code, "aui_schema_unsupported");
}
