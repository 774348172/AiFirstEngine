use engine_runtime::aui::{
    AuiCanvas, AuiControlInteractionSnapshot, AuiDocument, AuiFontRasterPolicy, AuiFontStyle,
    AuiFontStyleKind, AuiNode, AuiNodeKind, AuiRect,
};
use engine_runtime::aui_layout_schema::{SlotSpec, AUI_LAYOUT_SCHEMA_VERSION};
use engine_runtime::aui_theme::{
    AuiResolvedBrush, AuiThemeAsset, AuiThemeRegistry, BUILTIN_AUI_THEME_ID,
};
use engine_runtime::runtime_package::RuntimeAssetRef;

fn asset_ref(id: &str) -> RuntimeAssetRef {
    RuntimeAssetRef {
        id: id.into(),
        asset_type: "aui-theme".into(),
        guid: Some(format!("guid-{id}")),
        sub_asset: None,
    }
}

fn document(kind: AuiNodeKind) -> AuiDocument {
    let mut node = AuiNode::new("control", kind, AuiRect::stretch_full());
    node.interactable = matches!(
        kind,
        AuiNodeKind::Button
            | AuiNodeKind::InputField
            | AuiNodeKind::Toggle
            | AuiNodeKind::Slider
            | AuiNodeKind::List
    );
    node.slot = Some(SlotSpec::default());
    let mut document = AuiDocument::new(
        "theme-test",
        vec![AuiCanvas::screen_overlay("canvas", 320.0, 180.0, "control")],
        vec![node],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

fn theme(json: serde_json::Value) -> AuiThemeAsset {
    serde_json::from_value(json).expect("theme fixture must deserialize")
}

#[test]
fn typed_tokens_reject_mismatch_missing_and_theme_cycles() {
    let mismatched = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1",
        "assetId": "bad",
        "tokens": { "radius": { "kind": "color", "value": "#ff0000ff" } },
        "styles": {
            "button": {
                "role": "button",
                "paint": { "radius": { "source": "token", "token": "radius" } }
            }
        }
    }));
    let errors = AuiThemeRegistry::from_assets(vec![mismatched]).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.code == "aui_theme.token_type_mismatch"));

    let missing = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1",
        "assetId": "missing",
        "tokens": {},
        "styles": {
            "button": {
                "role": "button",
                "paint": { "background": { "kind": "solid", "color": { "source": "token", "token": "unknown" } } }
            }
        }
    }));
    let errors = AuiThemeRegistry::from_assets(vec![missing]).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.code == "aui_theme.token_missing"));

    let a = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1", "assetId": "a",
        "extends": { "id": "b", "type": "aui-theme", "guid": "guid-b" },
        "tokens": {}, "styles": {}
    }));
    let b = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1", "assetId": "b",
        "extends": { "id": "a", "type": "aui-theme", "guid": "guid-a" },
        "tokens": {}, "styles": {}
    }));
    let errors = AuiThemeRegistry::from_assets(vec![a, b]).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.code == "aui_theme.inheritance_cycle"));
}

#[test]
fn builtin_theme_is_versioned_and_covers_default_controls() {
    let registry = AuiThemeRegistry::builtin();
    assert_eq!(registry.default_theme_id(), BUILTIN_AUI_THEME_ID);
    for kind in [
        AuiNodeKind::Panel,
        AuiNodeKind::Text,
        AuiNodeKind::Button,
        AuiNodeKind::InputField,
        AuiNodeKind::Toggle,
        AuiNodeKind::Slider,
        AuiNodeKind::ProgressBar,
        AuiNodeKind::List,
    ] {
        let resolved = registry
            .resolve_document(&document(kind), &AuiControlInteractionSnapshot::default())
            .unwrap();
        assert!(resolved.styles.contains_key("control"), "missing {kind:?}");
    }
}

#[test]
fn state_order_is_base_hover_pressed_focus_and_disabled_replaces_pointer_states() {
    let asset = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1",
        "assetId": "states",
        "tokens": {},
        "styles": {
            "button": {
                "role": "button",
                "metrics": { "minWidth": { "source": "value", "value": 80.0 } },
                "paint": { "background": { "kind": "solid", "color": { "source": "value", "value": "#101010ff" } } },
                "states": {
                    "hover": { "background": { "kind": "solid", "color": { "source": "value", "value": "#202020ff" } } },
                    "pressed": { "background": { "kind": "solid", "color": { "source": "value", "value": "#303030ff" } } },
                    "focus": { "border": { "color": { "source": "value", "value": "#ffffffff" }, "width": { "source": "value", "value": 2.0 } } },
                    "disabled": { "background": { "kind": "solid", "color": { "source": "value", "value": "#404040ff" } } }
                }
            }
        }
    }));
    let registry = AuiThemeRegistry::from_assets(vec![asset]).unwrap();
    let mut doc = document(AuiNodeKind::Button);
    doc.theme = Some(asset_ref("states"));

    let active = AuiControlInteractionSnapshot {
        hovered_node: Some("control".into()),
        pressed_node: Some("control".into()),
        pressed_inside: true,
        focused_node: Some("control".into()),
        focus_visible: true,
        ..Default::default()
    };
    let resolved = registry.resolve_document(&doc, &active).unwrap();
    let style = &resolved.styles["control"];
    assert_eq!(style.metrics.min_width, Some(80.0));
    assert_eq!(style.paint.background, AuiResolvedBrush::solid("#303030ff"));
    assert_eq!(style.paint.border.width, 2.0);

    doc.nodes[0].interactable = false;
    let disabled = registry.resolve_document(&doc, &active).unwrap();
    let style = &disabled.styles["control"];
    assert_eq!(style.metrics.min_width, Some(80.0));
    assert_eq!(style.paint.background, AuiResolvedBrush::solid("#404040ff"));
}

#[test]
fn explicit_style_variant_scope_and_inline_style_have_deterministic_precedence() {
    let base = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1", "assetId": "base",
        "tokens": { "accent": { "kind": "color", "value": "#0066ccff" } },
        "styles": {
            "button": { "role": "button", "paint": { "background": { "kind": "solid", "color": { "source": "value", "value": "#111111ff" } } } },
            "button.primary": { "baseStyle": "button", "role": "button", "variant": "primary", "paint": { "background": { "kind": "solid", "color": { "source": "token", "token": "accent" } } } }
        }
    }));
    let scoped = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1", "assetId": "scoped",
        "tokens": {},
        "styles": { "button": { "role": "button", "paint": { "background": { "kind": "solid", "color": { "source": "value", "value": "#990000ff" } } } } }
    }));
    let registry = AuiThemeRegistry::from_assets(vec![base, scoped]).unwrap();
    let mut doc = document(AuiNodeKind::Button);
    doc.theme = Some(asset_ref("base"));
    doc.nodes[0].variant = Some("primary".into());
    let resolved = registry
        .resolve_document(&doc, &AuiControlInteractionSnapshot::default())
        .unwrap();
    assert_eq!(
        resolved.styles["control"].paint.background,
        AuiResolvedBrush::solid("#0066ccff")
    );

    doc.nodes[0].theme = Some(asset_ref("scoped"));
    doc.nodes[0].variant = None;
    doc.nodes[0].style = Some(engine_runtime::aui::AuiStyle::color("#00ff00"));
    let resolved = registry
        .resolve_document(&doc, &AuiControlInteractionSnapshot::default())
        .unwrap();
    assert_eq!(
        resolved.styles["control"].paint.background,
        AuiResolvedBrush::solid("#00ff00ff")
    );
}

#[test]
fn state_schema_cannot_smuggle_layout_metrics_into_paint_patch() {
    let source = serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1", "assetId": "invalid-state",
        "tokens": {},
        "styles": {
            "button": {
                "role": "button",
                "states": { "hover": { "minWidth": { "source": "value", "value": 999.0 } } }
            }
        }
    });
    assert!(serde_json::from_value::<AuiThemeAsset>(source).is_err());
}

#[test]
fn state_re_resolve_changes_foreground_without_turning_theme_output_into_inline_override() {
    let asset = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1",
        "assetId": "foreground-state",
        "tokens": {},
        "styles": {
            "button": {
                "role": "button",
                "paint": { "foreground": { "source": "value", "value": "#ffffffff" } },
                "states": {
                    "hover": { "foreground": { "source": "value", "value": "#ff0000ff" } }
                }
            }
        }
    }));
    let registry = AuiThemeRegistry::from_assets(vec![asset]).unwrap();
    let mut doc = document(AuiNodeKind::Button);
    doc.theme = Some(asset_ref("foreground-state"));

    let base = registry
        .resolve_document(&doc, &AuiControlInteractionSnapshot::default())
        .unwrap();
    assert_eq!(
        base.document.nodes[0].style.as_ref().unwrap().text_color,
        None
    );
    let hovered = registry
        .resolve_document(
            &base.document,
            &AuiControlInteractionSnapshot {
                hovered_node: Some("control".into()),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(
        hovered.styles["control"].paint.foreground.as_deref(),
        Some("#ff0000ff")
    );
    assert_eq!(
        base.styles["control"].metrics,
        hovered.styles["control"].metrics
    );
}

#[test]
fn font_token_selects_the_cooked_bundle_without_overriding_inline_font() {
    let asset = theme(serde_json::json!({
        "schemaVersion": "aui-theme-asset.v1",
        "assetId": "font-theme",
        "tokens": {
            "bodyFont": {
                "kind": "font",
                "value": {
                    "id": "font-atlas-profile-body",
                    "type": "font",
                    "guid": "guid-font-atlas-profile-body"
                }
            }
        },
        "styles": {
            "text": {
                "role": "text",
                "metrics": { "font": { "source": "token", "token": "bodyFont" } }
            }
        }
    }));
    let registry = AuiThemeRegistry::from_assets(vec![asset]).unwrap();
    let mut doc = document(AuiNodeKind::Text);
    doc.theme = Some(asset_ref("font-theme"));

    let resolved = registry
        .resolve_document(&doc, &AuiControlInteractionSnapshot::default())
        .unwrap();
    assert_eq!(
        resolved.document.nodes[0]
            .style
            .as_ref()
            .and_then(|style| style.font.as_ref())
            .and_then(|font| font.font_bundle_id.as_deref()),
        Some("font-atlas-profile-body")
    );

    doc.nodes[0].style = Some(engine_runtime::aui::AuiStyle {
        color: None,
        text_color: None,
        font_size: None,
        font: Some(AuiFontStyle {
            font_bundle_id: Some("font-atlas-profile-inline".into()),
            font_family_id: Some("family-inline".into()),
            style: AuiFontStyleKind::Italic,
            weight: 700,
            raster_policy: AuiFontRasterPolicy::Msdf,
        }),
    });
    let resolved = registry
        .resolve_document(&doc, &AuiControlInteractionSnapshot::default())
        .unwrap();
    let font = resolved.document.nodes[0]
        .style
        .as_ref()
        .and_then(|style| style.font.as_ref())
        .unwrap();
    assert_eq!(
        font.font_bundle_id.as_deref(),
        Some("font-atlas-profile-inline")
    );
    assert_eq!(font.font_family_id.as_deref(), Some("family-inline"));
    assert_eq!(font.style, AuiFontStyleKind::Italic);
    assert_eq!(font.weight, 700);
    assert_eq!(font.raster_policy, AuiFontRasterPolicy::Msdf);
}
