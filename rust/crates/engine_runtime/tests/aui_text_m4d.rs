use engine_runtime::aui::*;
use engine_runtime::aui_layout_schema::*;
use engine_runtime::aui_paragraph::{
    layout_paragraph, AuiParagraphCache, ParagraphHitAffinity, ParagraphRequest,
};
use engine_runtime::font_bundle::{
    CookedFontBundleAsset, CookedFontBundleGlyph, CookedFontBundlePage, FontBundleRenderMode,
    FontBundleStyle, RuntimeFontBundleRegistry, RuntimeFontFaceDescriptor, RuntimeLoadedFontBundle,
    RuntimePackageSourceFontFace, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
use unicode_segmentation::UnicodeSegmentation;

const FONT: &[u8] = include_bytes!("../../../resources/editor/fonts/NotoSansSC-VF.ttf");

fn fonts() -> RuntimeFontBundleRegistry {
    let source_digest = engine_runtime::canonical_digest::sha256_prefixed(FONT);
    let source = RuntimePackageSourceFontFace {
        font_face_id: "noto".into(),
        face_index: 0,
        source_digest: source_digest.clone(),
        bytes: FONT.to_vec(),
    };
    let metadata = CookedFontBundleAsset {
        schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.into(),
        font_bundle_id: "default".into(),
        font_stack_id: "default-stack".into(),
        generation: 1,
        max_bitmap_pages: 1,
        max_msdf_pages: 1,
        legacy_mode: false,
        fallback_used: false,
        quality_gate_eligible: true,
        pages: Vec::new(),
        glyphs: Vec::new(),
        kerning_adjustments: Vec::new(),
        font_faces: vec![RuntimeFontFaceDescriptor {
            font_face_id: "noto".into(),
            face_index: 0,
            source_digest,
            payload_path: "noto.ttf".into(),
        }],
        shaping_recipe: Some(engine_runtime::text_shaping::SHAPING_RECIPE.into()),
        bundle_digest: "sha256:test".into(),
    };
    let mut registry = RuntimeFontBundleRegistry {
        default_ui_font_bundle_id: Some("default".into()),
        ..Default::default()
    };
    std::sync::Arc::make_mut(&mut registry.bundles_by_id).insert(
        "default".into(),
        RuntimeLoadedFontBundle {
            metadata,
            page_payloads: Vec::new(),
            font_face_sources: vec![source],
        },
    );
    registry
}

fn renderable_fonts(text: &str) -> RuntimeFontBundleRegistry {
    let mut registry = fonts();
    let bundle = std::sync::Arc::make_mut(&mut registry.bundles_by_id)
        .get_mut("default")
        .unwrap();
    bundle.metadata.pages.push(CookedFontBundlePage {
        page_index: 0,
        render_mode: FontBundleRenderMode::BitmapR8,
        format: "r8Unorm".into(),
        width: 64,
        height: 64,
        byte_len: 4096,
        sha256: "sha256:page".into(),
        payload_path: "page.r8".into(),
    });
    bundle.page_payloads.push(vec![0; 4096]);
    let mut glyph_ids =
        engine_runtime::text_shaping::shape_text("noto", &bundle.font_face_sources[0].bytes, text)
            .unwrap()
            .items
            .into_iter()
            .filter(|glyph| glyph.has_outline)
            .map(|glyph| glyph.glyph_id as u16)
            .collect::<Vec<_>>();
    glyph_ids.sort_unstable();
    glyph_ids.dedup();
    bundle.metadata.glyphs = glyph_ids
        .into_iter()
        .enumerate()
        .flat_map(|(index, glyph_id)| {
            [20, 24].map(|pixel_size| CookedFontBundleGlyph {
                font_family_id: "family-ui".into(),
                font_face_id: "noto".into(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id,
                codepoint: u32::MAX,
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size,
                page_index: 0,
                pixel_rect: [(index as u32 % 7) * 8, (index as u32 / 7) * 12, 8, 12],
                bearing_x: 0,
                bearing_y: 12,
                advance_per_em_millionths: 600_000,
                plane_bounds_per_em_millionths: None,
            })
        })
        .collect();
    registry
}

fn document_with_text(text: &str) -> AuiDocument {
    let mut node = AuiNode::new(
        "label",
        AuiNodeKind::Text,
        AuiRect::fixed_position(0.0, 0.0, 240.0, 80.0),
    );
    node.text = Some(text.into());
    node.style = Some(AuiStyle::text("#ffffff", 20.0));
    node.text_layout = Some(TextLayoutSpec::default());
    let mut document = AuiDocument::new(
        "m4d",
        vec![AuiCanvas::screen_overlay("main", 240.0, 80.0, "label")],
        vec![node],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

#[test]
fn typed_spans_roundtrip_and_reject_non_grapheme_boundaries_atomically() {
    let text = "a\u{301} red";
    let mut document = document_with_text(text);
    document.nodes[0].text_spans = vec![AuiRichTextSpan {
        start: 0,
        end: 3,
        color: Some("#ff0000".into()),
        font_size: Some(24.0),
        font: None,
    }];
    let bytes = serde_json::to_vec(&document).unwrap();
    assert_eq!(
        serde_json::from_slice::<AuiDocument>(&bytes).unwrap(),
        document
    );
    assert!(validate(&document).is_empty());

    let before = serde_json::to_vec(&document).unwrap();
    document.nodes[0].text_spans[0].end = 1;
    let errors = validate(&document);
    assert!(errors
        .iter()
        .any(|item| item.code == "aui_rich_text_span_invalid"));
    assert_eq!(
        before,
        serde_json::to_vec(&serde_json::from_slice::<AuiDocument>(&before).unwrap()).unwrap()
    );
    assert!(validate_legacy_source(&serde_json::json!({
        "root": { "textSpans": [{"start":0,"end":1}] }
    }))
    .is_err());
}

#[test]
fn rich_text_span_resolves_to_paragraph_metrics_and_final_glyph_paint() {
    let text = "red blue";
    let fonts = renderable_fonts(text);
    let mut document = document_with_text(text);
    document.nodes[0].text_spans = vec![AuiRichTextSpan {
        start: 0,
        end: 3,
        color: Some("#ff0000".into()),
        font_size: Some(24.0),
        font: None,
    }];
    let output = ProjectUiStateSnapshotOutput::new(
        "m4d",
        AuiSnapshotSource::TestSnapshot,
        ProjectUiStateSnapshot::new(1),
    );
    let mut runtime = AuiC2Runtime::default();
    let present = AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached(
        &document,
        output,
        &engine_runtime::runtime_package::RuntimeAuiFontAtlasRegistry::empty("m4d"),
        &fonts,
        None,
        &mut runtime,
    );
    let paragraph = &present.paragraph_layouts["label"];
    assert!(paragraph
        .glyphs
        .iter()
        .filter(|glyph| glyph.cluster_start < 3)
        .all(|glyph| glyph.font_size == 24.0 && glyph.color.as_deref() == Some("#ff0000")));
    let glyphs = &present.overlay.glyph_plan.as_ref().unwrap().quads;
    assert!(glyphs
        .iter()
        .any(|glyph| glyph.color.as_deref() == Some("#ff0000")));
    assert!(glyphs.iter().any(|glyph| glyph.color.is_none()));
}

#[test]
fn mixed_bidi_uses_visual_runs_and_cluster_safe_geometry() {
    let fonts = fonts();
    let text = "abc אבג 123 a\u{301}";
    let spec = TextLayoutSpec {
        direction: TextDirection::Auto,
        wrap: TextWrap::GraphemeFallback,
        ..Default::default()
    };
    let mut cache = AuiParagraphCache::default();
    let layout = layout_paragraph(
        &mut cache,
        ParagraphRequest {
            text,
            font_size: 20.0,
            font: None,
            spans: &[],
            text_layout: &spec,
            constraint_width: 150.0,
            box_height: None,
            font_bundles: &fonts,
        },
    )
    .unwrap();

    assert!(layout
        .lines
        .iter()
        .any(|line| line.visual_run_order.len() > 1));
    assert!(layout
        .lines
        .iter()
        .flat_map(|line| &line.visual_run_order)
        .any(|run| run.rtl));
    let boundaries = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    for glyph in &layout.glyphs {
        assert!(boundaries.contains(&(glyph.cluster_start as usize)));
        assert!(boundaries.contains(&(glyph.cluster_end as usize)));
    }
    let hit = layout.hit_test(layout.glyphs[0].x, layout.lines[0].baseline);
    assert!(boundaries.contains(&(hit.byte_offset as usize)));
    assert!(matches!(
        hit.affinity,
        ParagraphHitAffinity::Upstream | ParagraphHitAffinity::Downstream
    ));
    let caret = layout.caret_rect(hit.byte_offset).unwrap();
    assert!(caret.height > 0.0);
    assert!(!layout.selection_rects(0, text.len() as u32).is_empty());
}

#[test]
fn input_editing_uses_utf8_grapheme_offsets_and_composition_is_atomic() {
    let mut node = AuiNode::new(
        "input",
        AuiNodeKind::InputField,
        AuiRect::fixed_position(0.0, 0.0, 200.0, 32.0),
    );
    node.text = Some("a\u{301}中".into());
    let mut state = AuiInputFieldState::from_node(&node);
    assert_eq!(state.caret_index, node.text.as_ref().unwrap().len());
    assert!(state.move_grapheme_left(false));
    assert_eq!(state.caret_index, "a\u{301}".len());
    assert!(state.backspace_grapheme());
    assert_eq!(state.draft_text, "中");
    state.set_composition("かな", 0, 2);
    assert_eq!(state.draft_text, "中");
    state.cancel_composition();
    assert_eq!(state.draft_text, "中");
    assert!(state.commit_composition("文", None));
    assert_eq!(state.draft_text, "文中");
}

#[test]
fn ime_request_uses_focused_input_paragraph_caret_and_disables_when_hidden() {
    let fonts = fonts();
    let mut input = AuiNode::new(
        "input",
        AuiNodeKind::InputField,
        AuiRect::fixed_position(10.0, 12.0, 180.0, 36.0),
    );
    input.text = Some("输入".into());
    input.style = Some(AuiStyle::text("#ffffff", 20.0));
    input.interactable = true;
    input.focusable = Some(true);
    input.text_layout = Some(TextLayoutSpec::default());
    let mut document = AuiDocument::new(
        "ime",
        vec![AuiCanvas::screen_overlay("main", 240.0, 80.0, "input")],
        vec![input],
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    let mut runtime = AuiC2Runtime::default();
    let (layout, paragraphs) = runtime.layout(&document, 1, &fonts);
    assert!(layout.diagnostics.is_empty(), "{:#?}", layout.diagnostics);
    let mut interaction = AuiInteractionState::default();
    interaction.focus.focused_node = Some("input".into());
    interaction.input_mode = AuiInputMode::TextEditing {
        node_id: "input".into(),
    };
    interaction.input_field = Some(AuiInputFieldState::from_node(&document.nodes[0]));

    let request =
        AuiInteractionSystem::ime_platform_request(&document, &layout, &paragraphs, &interaction);
    assert!(request.enabled);
    assert_eq!(request.node_id.as_deref(), Some("input"));
    assert!(request.cursor_rect.unwrap().height > 0.0);

    interaction
        .input_field
        .as_mut()
        .unwrap()
        .commit_composition("新增", None);
    assert!(
        AuiInteractionSystem::ime_platform_request(&document, &layout, &paragraphs, &interaction,)
            .enabled
    );

    let mut hidden_layout = layout;
    hidden_layout.computed_nodes[0].effective_visible = false;
    assert!(
        !AuiInteractionSystem::ime_platform_request(
            &document,
            &hidden_layout,
            &paragraphs,
            &interaction,
        )
        .enabled
    );
}

#[cfg(windows)]
#[test]
fn windows_real_fonts_shape_arabic_and_indic_without_missing_glyphs() {
    for (path, text, expected_direction) in [
        (r"C:\Windows\Fonts\arial.ttf", "العربية", "RightToLeft"),
        (r"C:\Windows\Fonts\Nirmala.ttc", "हिन्दी", "LeftToRight"),
    ] {
        let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let run = engine_runtime::text_shaping::shape_face(path, &bytes, 0, text)
            .unwrap_or_else(|| panic!("could not shape {path}"));
        assert!(run.items.iter().all(|glyph| glyph.glyph_id != 0), "{path}");
        assert_eq!(run.direction, expected_direction, "{path}");
        assert!(
            run.items
                .iter()
                .any(|glyph| glyph.cluster_end - glyph.cluster_start > 1),
            "{path}"
        );
    }
}
