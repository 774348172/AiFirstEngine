use engine_runtime::aui::{
    AuiC2Runtime, AuiCanvas, AuiControlInteractionSnapshot, AuiDocument, AuiInteractionState,
    AuiLayoutEngine, AuiNode, AuiNodeKind, AuiRect, AuiRendererBridge,
};
use engine_runtime::aui_layout_runtime::{
    AuiIntrinsicMeasure, AuiLayoutSolver, AuiMeasureInput, AuiMeasureOutput,
};
use engine_runtime::aui_layout_schema::TextLayoutSpec;
use engine_runtime::aui_layout_schema::{
    Dimension, LayoutMode, LayoutSpec, SlotSpec, AUI_LAYOUT_SCHEMA_VERSION,
};
use engine_runtime::aui_paragraph::{layout_paragraph, AuiParagraphCache, ParagraphRequest};
use engine_runtime::aui_theme::AuiThemeRegistry;
use engine_runtime::aui_virtual_list::{
    AuiVirtualListEngine, AuiVirtualListItem, AuiVirtualListSnapshot, AuiVirtualListSpec,
    AuiVirtualListState,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::font_bundle::{
    CookedFontBundleAsset, RuntimeFontBundleRegistry, RuntimeFontFaceDescriptor,
    RuntimeLoadedFontBundle, RuntimePackageSourceFontFace, COOKED_FONT_BUNDLE_SCHEMA_VERSION,
};
use serde_json::json;
use std::time::Instant;

#[cfg(windows)]
fn process_memory_bytes() -> (usize, usize) {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    assert_ne!(ok, 0, "query process memory for release matrix");
    (counters.WorkingSetSize, counters.PagefileUsage)
}

#[derive(Default)]
struct CountMeasure {
    calls: usize,
}

impl AuiIntrinsicMeasure for CountMeasure {
    fn measure(&mut self, input: AuiMeasureInput<'_>) -> AuiMeasureOutput {
        self.calls += 1;
        AuiMeasureOutput {
            width: input.known_width.unwrap_or(80.0),
            height: input.known_height.unwrap_or(20.0),
            first_baseline: input.text.map(|_| 15.0),
        }
    }
}

fn document(count: usize) -> AuiDocument {
    let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
    root.layout = Some(LayoutSpec {
        mode: LayoutMode::Column,
        ..Default::default()
    });
    root.children = (0..count).map(|index| format!("row-{index}")).collect();
    let mut nodes = Vec::with_capacity(count + 1);
    nodes.push(root);
    for index in 0..count {
        let mut child = AuiNode::new(
            format!("row-{index}"),
            AuiNodeKind::Text,
            AuiRect::fixed_position(0.0, 0.0, 80.0, 20.0),
        );
        child.parent = Some("root".into());
        child.text = Some(format!("Item {index}"));
        child.slot = Some(SlotSpec {
            width: Dimension::Fixed { value: 80.0 },
            height: Dimension::Content,
            ..Default::default()
        });
        nodes.push(child);
    }
    let mut document = AuiDocument::new(
        "aui-m5-scale",
        vec![AuiCanvas::screen_overlay("canvas", 1280.0, 720.0, "root")],
        nodes,
    );
    document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
    document
}

fn font_registry() -> RuntimeFontBundleRegistry {
    let bytes = include_bytes!("../../../resources/editor/fonts/NotoSansSC-VF.ttf").to_vec();
    let source_digest = sha256_prefixed(&bytes);
    let metadata = CookedFontBundleAsset {
        schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.into(),
        font_bundle_id: "m5-font".into(),
        font_stack_id: "m5-stack".into(),
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
            source_digest: source_digest.clone(),
            payload_path: "noto.ttf".into(),
        }],
        shaping_recipe: Some(engine_runtime::text_shaping::SHAPING_RECIPE.into()),
        bundle_digest: "sha256:m5-font".into(),
    };
    let mut registry = RuntimeFontBundleRegistry {
        default_ui_font_bundle_id: Some("m5-font".into()),
        ..Default::default()
    };
    std::sync::Arc::make_mut(&mut registry.bundles_by_id).insert(
        "m5-font".into(),
        RuntimeLoadedFontBundle {
            metadata,
            page_payloads: Vec::new(),
            font_face_sources: vec![RuntimePackageSourceFontFace {
                font_face_id: "noto".into(),
                face_index: 0,
                source_digest,
                bytes,
            }],
        },
    );
    registry
}

#[test]
#[ignore = "explicit release CPU composition qualification"]
fn release_theme_layout_draw_composition_matrix() {
    #[cfg(debug_assertions)]
    panic!("run this matrix with --release");
    let fonts = font_registry();
    let themes = AuiThemeRegistry::builtin();
    for count in [100, 1_000, 10_000] {
        let mut source = document(count);
        if count == 10_000 {
            for node in &mut source.nodes[1..] {
                node.kind = AuiNodeKind::Panel;
                node.text = None;
            }
        }
        let mut runtime = AuiC2Runtime::default();
        let render = |source: &AuiDocument, runtime: &mut AuiC2Runtime, frame: u64| {
            let start = Instant::now();
            let resolved = themes
                .resolve_document(source, &AuiControlInteractionSnapshot::default())
                .unwrap();
            let theme_us = start.elapsed().as_micros();
            let (layout, _) = runtime.layout(&resolved.document, frame, &fonts);
            assert!(layout.diagnostics.is_empty(), "{:#?}", layout.diagnostics);
            let layout_us = start.elapsed().as_micros() - theme_us;
            let (draw, _) = AuiLayoutEngine::extract_draw_list(&resolved.document, &layout);
            let draw_us = start.elapsed().as_micros() - theme_us - layout_us;
            let composition = AuiRendererBridge::build_composition_frame(
                frame,
                &resolved.document,
                &layout,
                &draw,
            );
            assert_eq!(layout.computed_nodes.len(), count + 1);
            std::hint::black_box(composition);
            let total_us = start.elapsed().as_micros();
            eprintln!(
                "{}",
                json!({
                    "case":"composition-stage", "nodes":count + 1, "frame":frame,
                    "themeUs":theme_us, "layoutUs":layout_us, "drawUs":draw_us,
                    "compositionUs":total_us-theme_us-layout_us-draw_us
                })
            );
            (total_us, runtime.paragraph_cache_stats())
        };
        let (cold_us, cold_stats) = render(&source, &mut runtime, 1);
        let (clean_us, clean_stats) = render(&source, &mut runtime, 2);
        assert_eq!(clean_stats.reshapes, cold_stats.reshapes);
        if count == 10_000 {
            source.nodes[1].slot.as_mut().unwrap().height = Dimension::Fixed { value: 22.0 };
        } else {
            source.nodes[1].text = Some("Updated menu label".into());
        }
        let (local_us, local_stats) = render(&source, &mut runtime, 3);
        if count != 10_000 {
            let changed = local_stats.reshapes - clean_stats.reshapes;
            assert!(
                (1..=2).contains(&changed),
                "local text should reshape only its own width variants: {changed} / {count}"
            );
        }
        if count == 10_000 {
            assert!(
                cold_us < 200_000,
                "10k cold composition exceeds 200ms: {cold_us}us"
            );
            assert!(
                clean_us < 100_000,
                "10k clean composition exceeds 100ms: {clean_us}us"
            );
            assert!(
                local_us < 150_000,
                "10k local composition exceeds 150ms: {local_us}us"
            );
        }
        eprintln!(
            "{}",
            json!({
                "case":"composition", "profile":"release", "nodes":count + 1,
                "font":"NotoSansSC-VF", "coldUs":cold_us, "cleanUs":clean_us,
                "localUs":local_us, "coldReshapes":cold_stats.reshapes,
                "cleanReshapes":clean_stats.reshapes, "localReshapes":local_stats.reshapes,
                "cacheEntries":runtime.paragraph_cache_len()
            })
        );
    }

    #[cfg(windows)]
    {
        let baseline = process_memory_bytes();
        let mut released_private = Vec::new();
        for cycle in 0..8 {
            let source = document(1_000);
            let mut runtime = AuiC2Runtime::default();
            let resolved = themes
                .resolve_document(&source, &AuiControlInteractionSnapshot::default())
                .unwrap();
            let (layout, _) = runtime.layout(&resolved.document, 1, &fonts);
            assert!(layout.diagnostics.is_empty());
            let (draw, _) = AuiLayoutEngine::extract_draw_list(&resolved.document, &layout);
            let composition =
                AuiRendererBridge::build_composition_frame(1, &resolved.document, &layout, &draw);
            std::hint::black_box(&composition);
            let live = process_memory_bytes();
            drop((composition, draw, layout, resolved, runtime, source));
            let released = process_memory_bytes();
            released_private.push(released.1);
            eprintln!(
                "{}",
                json!({
                    "case":"composition-lifecycle", "cycle":cycle,
                    "liveWorkingSetBytes":live.0, "livePrivateBytes":live.1,
                    "afterDropWorkingSetBytes":released.0, "afterDropPrivateBytes":released.1
                })
            );
        }
        assert!(released_private
            .iter()
            .all(|private| *private <= baseline.1 + 32 * 1024 * 1024));
        eprintln!(
            "{}",
            json!({
                "case":"composition-lifecycle-summary", "baselinePrivateBytes":baseline.1,
                "afterDropPrivateBytes":released_private
            })
        );
    }
}

#[test]
#[ignore = "explicit release performance qualification with a real font"]
fn release_paragraph_cache_matrix() {
    #[cfg(debug_assertions)]
    panic!("run this matrix with --release");
    let fonts = font_registry();
    let spec = TextLayoutSpec::default();
    for count in [100, 1_000] {
        let mut cache = AuiParagraphCache::default();
        let texts = (0..count)
            .map(|index| format!("霓虹控制台 {index} العربية"))
            .collect::<Vec<_>>();
        let mut phase = |label: &str, font_size: f32, change_first: bool| {
            let before = cache.stats();
            let start = Instant::now();
            for (index, text) in texts.iter().enumerate() {
                let changed = "更新后的标题 العربية";
                let input = if change_first && index == 0 {
                    changed
                } else {
                    text
                };
                layout_paragraph(
                    &mut cache,
                    ParagraphRequest {
                        text: input,
                        font_size,
                        font: None,
                        spans: &[],
                        text_layout: &spec,
                        constraint_width: 250.0,
                        box_height: None,
                        font_bundles: &fonts,
                    },
                )
                .unwrap();
            }
            let after = cache.stats();
            let elapsed = start.elapsed().as_micros();
            eprintln!(
                "{}",
                json!({
                    "case":"paragraph", "profile":"release", "nodes":count,
                    "phase":label, "cpuUs":elapsed,
                    "hits":after.hits-before.hits, "reshapes":after.reshapes-before.reshapes,
                    "evictions":after.evictions-before.evictions,
                    "cacheEntries":cache.len(), "cacheBytes":cache.resident_bytes()
                })
            );
            (after.hits - before.hits, after.reshapes - before.reshapes)
        };
        let (_, cold_shapes) = phase("cold", 20.0, false);
        assert_eq!(cold_shapes, count);
        let (_, clean_shapes) = phase("clean", 20.0, false);
        assert_eq!(clean_shapes, 0);
        let (_, local_shapes) = phase("local", 20.0, true);
        assert_eq!(local_shapes, 1);
        let (_, metric_shapes) = phase("theme-metrics", 24.0, true);
        assert_eq!(metric_shapes, count);
        assert!(cache.len() <= 4_096);
        assert!(cache.resident_bytes() <= 32 * 1024 * 1024);
    }
}

#[test]
#[ignore = "explicit release performance qualification, not a default test"]
fn release_layout_matrix_and_virtual_window() {
    #[cfg(debug_assertions)]
    panic!("run this matrix with --release");
    for count in [100, 1_000, 10_000] {
        let mut doc = document(count);
        let mut solver = AuiLayoutSolver::new();
        let mut measure = CountMeasure::default();

        let start = Instant::now();
        solver.solve(&doc, "canvas", 1, &mut measure).unwrap();
        let cold_us = start.elapsed().as_micros();
        let cold = solver.last_stats();
        assert_eq!(
            solver
                .solve(&doc, "canvas", 1, &mut measure)
                .unwrap()
                .nodes
                .len(),
            count + 1
        );
        let start = Instant::now();
        solver.solve(&doc, "canvas", 1, &mut measure).unwrap();
        let clean_us = start.elapsed().as_micros();
        let clean = solver.last_stats();
        assert!(clean.clean_snapshot_reused);
        assert_eq!(clean.measure_calls, 0);
        assert_eq!(clean.visited_nodes, 0);

        doc.nodes[1].text = Some("Changed content".into());
        let start = Instant::now();
        solver.solve(&doc, "canvas", 2, &mut measure).unwrap();
        let local_us = start.elapsed().as_micros();
        let local = solver.last_stats();
        assert!(local.measure_calls > 0);
        assert!(
            local.measure_calls < cold.measure_calls,
            "unrelated siblings remeasured: {local:?} / {cold:?}"
        );

        for child in &mut doc.nodes[1..] {
            child.slot.as_mut().unwrap().height = Dimension::Fixed { value: 22.0 };
        }
        let start = Instant::now();
        solver.solve(&doc, "canvas", 3, &mut measure).unwrap();
        let theme_us = start.elapsed().as_micros();
        let theme = solver.last_stats();
        eprintln!(
            "{}",
            json!({
                "case":"layout", "profile":"release", "nodes":count + 1,
                "coldUs":cold_us, "cleanUs":clean_us, "localUs":local_us,
                "themeMetricsUs":theme_us, "cold":cold, "clean":clean,
                "local":local, "themeMetrics":theme, "measureCallbackCalls":measure.calls
            })
        );
    }

    let spec = AuiVirtualListSpec::fixed("items", 240.0, 24.0)
        .with_overscan(96.0)
        .with_max_live_instances(24);
    let snapshot = AuiVirtualListSnapshot::new(
        (0..10_000)
            .map(|index| AuiVirtualListItem::new(format!("item-{index:05}"), None))
            .collect(),
    );
    let mut state = AuiVirtualListState::default();
    let mut interaction = AuiInteractionState::default();
    let start = Instant::now();
    let window =
        AuiVirtualListEngine::update(&spec, &snapshot, &mut state, &mut interaction).unwrap();
    let cold_us = start.elapsed().as_micros();
    assert_eq!(window.total_item_count, 10_000);
    assert!(window.instances.len() <= 24);
    eprintln!(
        "{}",
        json!({
            "case":"virtual-list", "profile":"release", "items":10_000,
            "live":window.instances.len(), "coldUs":cold_us
        })
    );

    #[cfg(windows)]
    {
        let baseline = process_memory_bytes();
        let mut after_drop = Vec::new();
        for cycle in 0..8 {
            let mut solver = AuiLayoutSolver::new();
            let doc = document(10_000);
            solver
                .solve(&doc, "canvas", 1, &mut CountMeasure::default())
                .unwrap();
            let live = process_memory_bytes();
            drop(solver);
            drop(doc);
            let released = process_memory_bytes();
            after_drop.push(released);
            eprintln!(
                "{}",
                json!({
                    "case":"lifecycle", "cycle":cycle, "liveWorkingSetBytes":live.0,
                    "livePrivateBytes":live.1, "afterDropWorkingSetBytes":released.0,
                    "afterDropPrivateBytes":released.1
                })
            );
        }
        assert!(after_drop
            .iter()
            .all(|(_, private)| *private <= baseline.1 + 16 * 1024 * 1024));
        eprintln!(
            "{}",
            json!({
                "case":"lifecycle-summary", "baselineWorkingSetBytes":baseline.0,
                "baselinePrivateBytes":baseline.1, "afterDrop":after_drop
            })
        );
    }
}
