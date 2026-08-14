use super::*;
use editor_ui_renderer::{DrawCommand, UiColor, UiDrawList, UiRect};

#[test]
fn ordered_painter_batches_preserve_cross_kind_occlusion() {
    let mut draw_list = fixture_draw_list();
    draw_list.commands = vec![
        DrawCommand::Text {
            rect: UiRect {
                x: 8.0,
                y: 8.0,
                width: 120.0,
                height: 20.0,
            },
            text: "launcher".to_string(),
            color: UiColor::TEXT,
            size: 12.0,
        },
        DrawCommand::Rect {
            rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 180.0,
            },
            color: UiColor::rgba(6, 8, 12, 190),
            corner_radius: 0.0,
        },
        DrawCommand::Text {
            rect: UiRect {
                x: 24.0,
                y: 24.0,
                width: 180.0,
                height: 24.0,
            },
            text: "modal".to_string(),
            color: UiColor::TEXT,
            size: 14.0,
        },
    ];

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");
    let graph = UiRenderGraph::from_draw_plan(&plan);
    let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);
    let draw_kinds = rhi_plan
        .commands
        .iter()
        .filter_map(|command| match command.kind {
            UiRhiCommandKind::DrawRectBatch | UiRhiCommandKind::DrawTextBatch => {
                Some(command.kind.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if plan.font_loaded {
        assert_eq!(
            draw_kinds,
            vec![
                UiRhiCommandKind::DrawTextBatch,
                UiRhiCommandKind::DrawRectBatch,
                UiRhiCommandKind::DrawTextBatch,
            ],
            "the modal rect must remain between launcher text and modal text"
        );
    } else {
        assert_eq!(draw_kinds, vec![UiRhiCommandKind::DrawRectBatch]);
        let scrim_index = plan
            .drawable_rects
            .iter()
            .position(|drawable| drawable.source_kind == UiGpuDrawableRectSource::Rect)
            .expect("modal scrim drawable");
        assert!(plan.drawable_rects[..scrim_index]
            .iter()
            .any(|drawable| drawable.source_kind == UiGpuDrawableRectSource::TextGlyph));
        assert!(plan.drawable_rects[scrim_index + 1..]
            .iter()
            .any(|drawable| drawable.source_kind == UiGpuDrawableRectSource::TextGlyph));
    }
}

#[test]
fn ordered_painter_batches_preserve_texture_positions_and_ranges() {
    let mut draw_list = fixture_draw_list();
    let rect = UiRect {
        x: 0.0,
        y: 0.0,
        width: 64.0,
        height: 64.0,
    };
    draw_list.commands = vec![
        DrawCommand::Rect {
            rect,
            color: UiColor::PANEL,
            corner_radius: 0.0,
        },
        DrawCommand::ViewportTextureSlot {
            rect,
            scene_id: Some("scene".to_string()),
            frame: 1,
            texture_id: Some("viewport".to_string()),
            target_id: Some("game".to_string()),
        },
        DrawCommand::ImageTextureSlot {
            rect,
            source_uv: UiUvRect::FULL,
            texture_id: Some("image".to_string()),
            fallback_color: UiColor::PANEL_DARK,
            tint: UiColor::rgba(255, 255, 255, 255),
        },
        DrawCommand::Rect {
            rect,
            color: UiColor::rgba(6, 8, 12, 190),
            corner_radius: 0.0,
        },
    ];

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");
    let graph = UiRenderGraph::from_draw_plan(&plan);
    let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);
    let draws = rhi_plan
        .commands
        .iter()
        .filter(|command| {
            matches!(
                command.kind,
                UiRhiCommandKind::DrawRectBatch
                    | UiRhiCommandKind::DrawViewportTextureBatch
                    | UiRhiCommandKind::DrawImageTextureBatch
            )
        })
        .map(|command| (command.kind.clone(), command.first_item, command.item_count))
        .collect::<Vec<_>>();

    assert_eq!(
        draws,
        vec![
            (UiRhiCommandKind::DrawRectBatch, 0, 1),
            (UiRhiCommandKind::DrawViewportTextureBatch, 0, 1),
            (UiRhiCommandKind::DrawImageTextureBatch, 0, 1),
            (UiRhiCommandKind::DrawRectBatch, 1, 1),
        ]
    );
}

#[test]
fn draw_plan_counts_rect_text_viewport() {
    let draw_list = fixture_draw_list();

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");

    assert_eq!(plan.draw_command_count, 3);
    assert_eq!(plan.rect_count, 1);
    assert_eq!(plan.text_command_count, 1);
    assert_eq!(plan.skipped_text_count, 0);
    assert!(plan.rendered_glyph_count > 0);
    assert_eq!(plan.viewport_slot_count, 1);
    assert_eq!(plan.viewport_texture_quad_count, 0);
    assert_eq!(plan.viewport_texture_fallback_count, 1);
    assert!(plan
        .drawable_rects
        .iter()
        .any(|rect| rect.source_kind == UiGpuDrawableRectSource::Rect));
    assert!(plan
        .drawable_rects
        .iter()
        .any(|rect| rect.source_kind == UiGpuDrawableRectSource::ViewportPlaceholder));
    if plan.font_loaded {
        assert_eq!(plan.font_backend, "ab_glyph_atlas");
        assert!(!plan.text_glyphs.is_empty());
        assert!(plan.glyph_cache_count > 0);
        assert!(plan.glyph_atlas_alpha.iter().any(|alpha| *alpha > 0));
    } else {
        assert!(plan
            .drawable_rects
            .iter()
            .any(|rect| rect.source_kind == UiGpuDrawableRectSource::TextGlyph));
    }
}

#[test]
fn shared_gpu_context_headless_summary_is_reportable() {
    let summary = EditorSharedGpuContextSummary::headless_mock();

    assert_eq!(summary.status, EditorSharedGpuContextStatus::HeadlessMock);
    assert_eq!(summary.backend_name, "headless");
    assert!(!summary.real_wgpu_available);
    assert!(!summary.diagnostics.is_empty());
}

#[test]
fn viewport_texture_registry_tracks_lifecycle() {
    let mut registry = EditorViewportTextureRegistry::new();

    let first = registry.allocate_or_resize_mock(
        "session-a",
        "viewport-main",
        "viewport-main::frame-1",
        1280,
        720,
        "Rgba8Unorm",
        "editor-gameview",
    );
    assert_eq!(first.generation, 1);
    assert_eq!(
        first.present_status,
        EditorViewportTexturePresentStatus::Allocated
    );
    assert!(registry.contains("viewport-main::frame-1"));

    let resized = registry.allocate_or_resize_mock(
        "session-a",
        "viewport-main",
        "viewport-main::frame-1",
        1920,
        1080,
        "Rgba8Unorm",
        "editor-gameview",
    );
    assert_eq!(resized.generation, 2);
    assert_eq!(
        resized.present_status,
        EditorViewportTexturePresentStatus::Resized
    );

    let rendered = registry
        .mark_rendered("viewport-main::frame-1", 7)
        .expect("texture should exist");
    assert_eq!(rendered.last_frame_index, Some(7));
    assert_eq!(
        rendered.present_status,
        EditorViewportTexturePresentStatus::Rendered
    );
    assert_eq!(registry.unregister_session("session-a"), 1);
    assert!(!registry.contains("viewport-main::frame-1"));
}

#[test]
fn viewport_texture_publication_reuses_surface_and_separates_generation_from_content() {
    let mut registry = EditorViewportTextureRegistry::new();
    let surface_id = "gameview-surface::session-a::viewport-main";
    let allocated = registry.allocate_or_resize_mock(
        "session-a",
        "viewport-main",
        surface_id,
        1280,
        720,
        "Rgba8Unorm",
        "editor-gameview",
    );
    let lifecycle_after_allocate = registry.lifecycle_event_count();
    let reused = registry.allocate_or_resize_mock(
        "session-a",
        "viewport-main",
        surface_id,
        1280,
        720,
        "Rgba8Unorm",
        "editor-gameview",
    );
    assert_eq!(allocated.generation, reused.generation);
    assert_eq!(registry.lifecycle_event_count(), lifecycle_after_allocate);

    let first = registry
        .mark_published(surface_id, 10, "hash-10")
        .expect("first publication");
    let second = registry
        .mark_published(surface_id, 11, "hash-11")
        .expect("second publication");
    assert_eq!(first.publication.surface_id, surface_id);
    assert_eq!(first.publication.surface_generation, 1);
    assert_eq!(first.publication.publication_index, 1);
    assert_eq!(second.publication.surface_generation, 1);
    assert_eq!(second.publication.publication_index, 2);
    assert!(second.submit_serial > first.submit_serial);
    assert_eq!(second.content.frame_index, 11);
    assert_eq!(second.content.frame_hash, "hash-11");

    let last_good = registry.last_receipt(surface_id).expect("last receipt");
    assert_eq!(last_good.status, GameViewPublicationStatus::Reused);
    assert_eq!(last_good.publication, second.publication);
    assert_eq!(last_good.content, second.content);

    let resized = registry.allocate_or_resize_mock(
        "session-a",
        "viewport-main",
        surface_id,
        1600,
        900,
        "Rgba8Unorm",
        "editor-gameview",
    );
    assert_eq!(resized.generation, 2);
    assert_eq!(resized.publication_index, 0);
    assert!(registry.last_receipt(surface_id).is_none());
}

#[test]
fn viewport_texture_slot_with_texture_id_builds_texture_quad() {
    let mut draw_list = fixture_draw_list();
    draw_list.commands = vec![DrawCommand::ViewportTextureSlot {
        rect: UiRect {
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 200.0,
        },
        scene_id: Some("scene-main".to_string()),
        frame: 42,
        texture_id: Some("viewport-main::frame-42".to_string()),
        target_id: Some("viewport-main".to_string()),
    }];

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");
    let graph = UiRenderGraph::from_draw_plan(&plan);
    let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);

    assert_eq!(plan.viewport_slot_count, 1);
    assert_eq!(plan.viewport_texture_quad_count, 1);
    assert_eq!(plan.viewport_texture_fallback_count, 0);
    assert!(plan.drawable_rects.is_empty());
    assert_eq!(
        plan.viewport_texture_quads[0].texture_id,
        "viewport-main::frame-42"
    );
    assert!(graph
        .passes
        .iter()
        .any(|pass| pass.kind == UiRenderPassKind::DrawViewportTextures));
    assert!(rhi_plan
        .commands
        .iter()
        .any(|command| command.kind == UiRhiCommandKind::DrawViewportTextureBatch));
}

#[test]
fn image_texture_slot_builds_independent_draw_plan_and_rhi_pass() {
    let mut draw_list = fixture_draw_list();
    draw_list.commands = vec![DrawCommand::ImageTextureSlot {
        rect: UiRect {
            x: 8.0,
            y: 12.0,
            width: 96.0,
            height: 48.0,
        },
        source_uv: UiUvRect::FULL,
        texture_id: Some("asset-thumbnail::player".to_string()),
        fallback_color: UiColor::PANEL_DARK,
        tint: UiColor::rgba(255, 255, 255, 255),
    }];

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");
    let graph = UiRenderGraph::from_draw_plan(&plan);
    let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);

    assert_eq!(plan.image_texture_slot_count, 1);
    assert_eq!(plan.image_texture_quad_count, 1);
    assert_eq!(plan.viewport_slot_count, 0);
    assert_eq!(
        plan.image_texture_quads[0].texture_id,
        "asset-thumbnail::player"
    );
    assert!(graph
        .passes
        .iter()
        .any(|pass| pass.kind == UiRenderPassKind::DrawImageTextures));
    assert!(graph
        .resources
        .iter()
        .any(|resource| resource.kind == UiRenderResourceKind::ImageTexture));
    assert!(rhi_plan
        .commands
        .iter()
        .any(|command| command.kind == UiRhiCommandKind::DrawImageTextureBatch));
}

#[test]
fn image_texture_registry_reuses_upload_and_enforces_lru_item_budget() {
    let mut registry = EditorImageTextureRegistry::new();
    let pixels = [255, 32, 16, 255];

    let first = registry
        .upload_mock("thumbnail-0", 1, 1, "hash-0", &pixels)
        .expect("first upload");
    let reused = registry
        .upload_mock("thumbnail-0", 1, 1, "hash-0", &pixels)
        .expect("reused upload");
    assert_eq!(first.generation, 1);
    assert_eq!(reused.upload_status, EditorImageTextureUploadStatus::Reused);
    assert_eq!(registry.upload_count(), 1);

    for index in 1..=EDITOR_IMAGE_TEXTURE_MAX_ITEMS {
        registry
            .upload_mock(
                format!("thumbnail-{index}"),
                1,
                1,
                format!("hash-{index}"),
                &pixels,
            )
            .expect("bounded upload");
    }
    assert_eq!(registry.texture_count(), EDITOR_IMAGE_TEXTURE_MAX_ITEMS);
    assert!(registry.eviction_count() >= 1);
    assert!(registry.byte_len() <= EDITOR_IMAGE_TEXTURE_MAX_BYTES);
}

#[test]
fn draw_plan_rejects_empty_surface() {
    let mut draw_list = fixture_draw_list();
    draw_list.surface_width = 0.0;

    assert_eq!(
        UiGpuDrawPlan::from_draw_list(&draw_list),
        Err("ui_gpu_draw_plan.empty_surface".to_string())
    );
}

#[test]
fn clip_contract_crops_gpu_geometry_and_texture_uv() {
    let mut draw_list = fixture_draw_list();
    let clip = UiRect {
        x: 25.0,
        y: 10.0,
        width: 50.0,
        height: 20.0,
    };
    draw_list.commands = vec![
        DrawCommand::Rect {
            rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            color: UiColor::PANEL,
            corner_radius: 0.0,
        }
        .with_clip(Some(clip)),
        DrawCommand::ImageTextureSlot {
            rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            source_uv: UiUvRect {
                u0: 0.2,
                v0: 0.1,
                u1: 0.8,
                v1: 0.9,
            },
            texture_id: Some("image".into()),
            fallback_color: UiColor::PANEL,
            tint: UiColor::rgba(128, 160, 192, 204),
        }
        .with_clip(Some(clip)),
    ];

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("clipped draw plan");

    assert_eq!(plan.drawable_rects[0].rect, clip);
    assert_eq!(plan.image_texture_quads[0].rect, clip);
    let actual_uv = plan.image_texture_quads[0].uv;
    let expected_uv = UiUvRect {
        u0: 0.35,
        v0: 0.26,
        u1: 0.65,
        v1: 0.58,
    };
    for (actual, expected) in [
        (actual_uv.u0, expected_uv.u0),
        (actual_uv.v0, expected_uv.v0),
        (actual_uv.u1, expected_uv.u1),
        (actual_uv.v1, expected_uv.v1),
    ] {
        assert!((actual - expected).abs() <= f32::EPSILON);
    }
    assert_eq!(
        plan.image_texture_quads[0].tint,
        UiColor::rgba(128, 160, 192, 204)
    );
}

#[test]
fn nine_slice_commands_preserve_source_uv_and_ordered_image_batch() {
    let brush = editor_ui_renderer::ControlBrush::NineSlice {
        texture_id: "editor-control-tab-hover".to_string(),
        fallback_color: UiColor::PANEL,
        tint: UiColor::rgba(220, 230, 240, 192),
        slice: editor_ui_renderer::ControlSliceInsets {
            left: 4.0,
            top: 4.0,
            right: 4.0,
            bottom: 4.0,
        },
    };
    let mut draw_list = fixture_draw_list();
    draw_list.commands = editor_ui_renderer::paint_control_brush(
        UiRect {
            x: 10.0,
            y: 20.0,
            width: 80.0,
            height: 24.0,
        },
        &brush,
        1.0,
    )
    .commands;
    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("nine-slice draw plan");
    assert_eq!(plan.image_texture_quads.len(), 9);
    assert_eq!(
        plan.paint_batches,
        vec![UiGpuPaintBatch {
            kind: UiGpuPaintBatchKind::ImageTextures,
            first_item: 0,
            item_count: 9,
        }]
    );
    assert_eq!(plan.image_texture_quads[0].uv.u0, 0.0);
    assert_eq!(plan.image_texture_quads[8].uv.u1, 1.0);
    assert!(plan
        .image_texture_quads
        .windows(2)
        .all(|pair| pair[0].rect.y <= pair[1].rect.y));
}

#[test]
fn headless_renderer_reports_rendered_glyphs() {
    let renderer = HeadlessUiGpuRenderer::new();

    let report = renderer.present(&fixture_draw_list());

    assert!(report.presented);
    assert_eq!(report.skipped_text_count, 0);
    assert!(report.rendered_glyph_count > 0);
    assert!(report.submitted_batch_count >= 1);
    assert_eq!(report.present_status, "presented");
}

#[test]
fn ui_render_graph_compiles_draw_plan_into_passes_and_resources() {
    let plan = UiGpuDrawPlan::from_draw_list(&fixture_draw_list()).expect("draw plan");

    let graph = UiRenderGraph::from_draw_plan(&plan);

    assert_eq!(graph.schema_version, UI_RENDER_GRAPH_SCHEMA_VERSION);
    assert!(graph
        .passes
        .iter()
        .any(|pass| pass.kind == UiRenderPassKind::Clear));
    assert!(graph
        .passes
        .iter()
        .any(|pass| pass.kind == UiRenderPassKind::DrawRects));
    if plan.font_loaded {
        assert!(graph
            .passes
            .iter()
            .any(|pass| pass.kind == UiRenderPassKind::DrawText));
        assert!(graph
            .resources
            .iter()
            .any(|resource| resource.kind == UiRenderResourceKind::GlyphAtlasTexture));
    }
    assert!(graph
        .passes
        .iter()
        .any(|pass| pass.kind == UiRenderPassKind::Present));
}

#[test]
fn ui_rhi_command_plan_compiles_render_graph_for_backend_execution() {
    let plan = UiGpuDrawPlan::from_draw_list(&fixture_draw_list()).expect("draw plan");
    let graph = UiRenderGraph::from_draw_plan(&plan);

    let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);

    assert_eq!(rhi_plan.schema_version, UI_RHI_COMMAND_PLAN_SCHEMA_VERSION);
    assert!(rhi_plan
        .commands
        .iter()
        .any(|command| command.kind == UiRhiCommandKind::ClearSurface));
    assert!(rhi_plan
        .commands
        .iter()
        .any(|command| command.kind == UiRhiCommandKind::DrawRectBatch));
    if plan.font_loaded {
        assert!(rhi_plan
            .commands
            .iter()
            .any(|command| command.kind == UiRhiCommandKind::DrawTextBatch));
    }
    assert!(rhi_plan
        .commands
        .iter()
        .any(|command| command.kind == UiRhiCommandKind::PresentSurface));
}

#[test]
fn draw_plan_uses_embedded_cjk_font_for_chinese_text() {
    let mut draw_list = fixture_draw_list();
    draw_list.commands = vec![DrawCommand::Text {
        rect: UiRect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 20.0,
        },
        text: "中文".to_string(),
        color: UiColor::TEXT,
        size: 12.0,
    }];

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");

    assert_eq!(plan.text_command_count, 1);
    assert_eq!(plan.skipped_text_count, 0);
    assert_eq!(plan.rendered_glyph_count, 2);
    assert!(plan.font_loaded);
    assert!(!plan.text_glyphs.is_empty());
    assert_eq!(plan.unsupported_glyph_count, 0);
    assert!(plan.glyph_cache_count >= 2);
    assert!(plan.glyph_atlas_alpha.iter().any(|alpha| *alpha > 0));
}

#[test]
fn draw_plan_skips_text_when_rect_too_small() {
    let mut draw_list = fixture_draw_list();
    draw_list.commands = vec![DrawCommand::Text {
        rect: UiRect {
            x: 0.0,
            y: 0.0,
            width: 2.0,
            height: 2.0,
        },
        text: "Tiny".to_string(),
        color: UiColor::TEXT,
        size: 4.0,
    }];

    let plan = UiGpuDrawPlan::from_draw_list(&draw_list).expect("draw plan");

    assert_eq!(plan.text_command_count, 1);
    assert_eq!(plan.skipped_text_count, 1);
    assert_eq!(plan.rendered_glyph_count, 0);
}

#[test]
fn headless_renderer_treats_viewport_slot_as_placeholder_rect() {
    let plan = UiGpuDrawPlan::from_draw_list(&fixture_draw_list()).expect("draw plan");

    assert!(plan
        .drawable_rects
        .iter()
        .any(|rect| rect.source_kind == UiGpuDrawableRectSource::ViewportPlaceholder));
}

#[test]
fn report_is_serializable() {
    let report = HeadlessUiGpuRenderer::new().present(&fixture_draw_list());

    let json = serde_json::to_string(&report).expect("report should serialize");

    assert!(json.contains("real-ui-present-report.v1"));
    assert!(json.contains("rendered_glyph_count"));
}

fn fixture_draw_list() -> UiDrawList {
    UiDrawList {
        revision: 1,
        frame: 1,
        surface_width: 1280.0,
        surface_height: 720.0,
        commands: vec![
            DrawCommand::Rect {
                rect: UiRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 50.0,
                },
                color: UiColor::PANEL,
                corner_radius: 0.0,
            },
            DrawCommand::Text {
                rect: UiRect {
                    x: 8.0,
                    y: 8.0,
                    width: 80.0,
                    height: 16.0,
                },
                text: "Skipped".to_string(),
                color: UiColor::TEXT,
                size: 12.0,
            },
            DrawCommand::ViewportTextureSlot {
                rect: UiRect {
                    x: 100.0,
                    y: 100.0,
                    width: 300.0,
                    height: 200.0,
                },
                scene_id: Some("scene-main".to_string()),
                frame: 1,
                texture_id: None,
                target_id: None,
            },
        ],
        hit_regions: Vec::new(),
    }
}
