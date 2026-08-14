pub use editor_ui_renderer::UiUvRect;
use editor_ui_renderer::{DrawCommand, UiColor, UiDrawList, UiRect};
use serde::{Deserialize, Serialize};

use crate::diagnostics::UI_GPU_DRAW_PLAN_SCHEMA_VERSION;
use crate::font_system::{BuiltinDebugFont, EditorFontSystem};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiGpuDrawableRect {
    pub rect: UiRect,
    pub color: UiColor,
    pub source_kind: UiGpuDrawableRectSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiGpuTextGlyph {
    pub rect: UiRect,
    pub uv: UiUvRect,
    pub color: UiColor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiGpuViewportTextureQuad {
    pub rect: UiRect,
    pub uv: UiUvRect,
    pub texture_id: String,
    pub target_id: Option<String>,
    pub frame_index: u64,
    pub fallback_if_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiGpuImageTextureQuad {
    pub rect: UiRect,
    pub uv: UiUvRect,
    pub texture_id: String,
    pub fallback_color: UiColor,
    pub tint: UiColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiGpuDrawableRectSource {
    Rect,
    ViewportPlaceholder,
    ImageTexturePlaceholder,
    TextGlyph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiGpuPaintBatchKind {
    Rects,
    Text,
    ViewportTextures,
    ImageTextures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiGpuPaintBatch {
    pub kind: UiGpuPaintBatchKind,
    pub first_item: usize,
    pub item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiGpuDrawPlan {
    pub schema_version: String,
    pub surface_width: u32,
    pub surface_height: u32,
    pub draw_command_count: usize,
    pub rect_count: usize,
    pub text_command_count: usize,
    pub skipped_text_count: usize,
    pub rendered_glyph_count: usize,
    pub unsupported_glyph_count: usize,
    pub font_backend: String,
    pub font_loaded: bool,
    pub font_source: Option<String>,
    pub glyph_atlas_width: u32,
    pub glyph_atlas_height: u32,
    pub glyph_cache_count: usize,
    pub missing_glyph_count: usize,
    pub viewport_slot_count: usize,
    pub viewport_texture_quad_count: usize,
    pub viewport_texture_fallback_count: usize,
    pub image_texture_slot_count: usize,
    pub image_texture_quad_count: usize,
    pub image_texture_fallback_count: usize,
    pub hit_region_count: usize,
    pub drawable_rects: Vec<UiGpuDrawableRect>,
    pub text_glyphs: Vec<UiGpuTextGlyph>,
    pub viewport_texture_quads: Vec<UiGpuViewportTextureQuad>,
    pub image_texture_quads: Vec<UiGpuImageTextureQuad>,
    pub paint_batches: Vec<UiGpuPaintBatch>,
    #[serde(skip)]
    pub glyph_atlas_alpha: Vec<u8>,
}

impl UiGpuDrawPlan {
    pub fn from_draw_list(draw_list: &UiDrawList) -> Result<Self, String> {
        if draw_list.surface_width <= 0.0 || draw_list.surface_height <= 0.0 {
            return Err("ui_gpu_draw_plan.empty_surface".to_string());
        }

        let mut font_system = EditorFontSystem::new();
        let mut rect_count = 0;
        let mut text_command_count = 0;
        let mut skipped_text_count = 0;
        let mut rendered_glyph_count = 0;
        let mut unsupported_glyph_count = 0;
        let mut viewport_slot_count = 0;
        let mut viewport_texture_fallback_count = 0;
        let mut image_texture_slot_count = 0;
        let mut image_texture_fallback_count = 0;
        let mut drawable_rects = Vec::new();
        let mut text_glyphs = Vec::new();
        let mut viewport_texture_quads = Vec::new();
        let mut image_texture_quads = Vec::new();
        let mut paint_batches = Vec::new();

        for command in &draw_list.commands {
            let rect_start = drawable_rects.len();
            let text_start = text_glyphs.len();
            let viewport_start = viewport_texture_quads.len();
            let image_start = image_texture_quads.len();
            let clip = command.clip();
            match command.unclipped() {
                DrawCommand::Rect { rect, color, .. } => {
                    let Some(rect) = clipped_rect(*rect, clip) else {
                        continue;
                    };
                    rect_count += 1;
                    drawable_rects.push(UiGpuDrawableRect {
                        rect,
                        color: *color,
                        source_kind: UiGpuDrawableRectSource::Rect,
                    });
                }
                DrawCommand::Text {
                    rect,
                    text,
                    color,
                    size,
                } => {
                    let Some(rect) = clipped_rect(*rect, clip) else {
                        continue;
                    };
                    text_command_count += 1;
                    let stats =
                        font_system.layout_text(&mut text_glyphs, rect, text, *color, *size);
                    if stats.used_builtin_fallback {
                        let fallback_stats = BuiltinDebugFont::push_text_rects(
                            &mut drawable_rects,
                            rect,
                            text,
                            *color,
                            *size,
                        );
                        rendered_glyph_count += fallback_stats.rendered_glyph_count;
                        unsupported_glyph_count += fallback_stats.unsupported_glyph_count;
                        if fallback_stats.skipped {
                            skipped_text_count += 1;
                        }
                        continue;
                    }
                    rendered_glyph_count += stats.rendered_glyph_count;
                    unsupported_glyph_count += stats.unsupported_glyph_count;
                    if stats.skipped {
                        skipped_text_count += 1;
                    }
                }
                DrawCommand::ViewportTextureSlot {
                    rect,
                    frame,
                    texture_id,
                    target_id,
                    ..
                } => {
                    let Some((rect, uv)) = clipped_textured_rect(*rect, UiUvRect::FULL, clip)
                    else {
                        continue;
                    };
                    viewport_slot_count += 1;
                    if let Some(texture_id) = texture_id {
                        viewport_texture_quads.push(UiGpuViewportTextureQuad {
                            rect,
                            uv,
                            texture_id: texture_id.clone(),
                            target_id: target_id.clone(),
                            frame_index: *frame,
                            fallback_if_missing: true,
                        });
                    } else {
                        viewport_texture_fallback_count += 1;
                        drawable_rects.push(UiGpuDrawableRect {
                            rect,
                            color: UiColor::rgba(23, 40, 52, 255),
                            source_kind: UiGpuDrawableRectSource::ViewportPlaceholder,
                        });
                    }
                }
                DrawCommand::ImageTextureSlot {
                    rect,
                    source_uv,
                    texture_id,
                    fallback_color,
                    tint,
                } => {
                    let Some((rect, uv)) = clipped_textured_rect(*rect, *source_uv, clip) else {
                        continue;
                    };
                    image_texture_slot_count += 1;
                    if let Some(texture_id) = texture_id {
                        image_texture_quads.push(UiGpuImageTextureQuad {
                            rect,
                            uv,
                            texture_id: texture_id.clone(),
                            fallback_color: *fallback_color,
                            tint: *tint,
                        });
                    } else {
                        image_texture_fallback_count += 1;
                        drawable_rects.push(UiGpuDrawableRect {
                            rect,
                            color: *fallback_color,
                            source_kind: UiGpuDrawableRectSource::ImageTexturePlaceholder,
                        });
                    }
                }
                DrawCommand::Clipped { .. } => unreachable!("unclipped removes wrappers"),
            }
            push_paint_batch(
                &mut paint_batches,
                UiGpuPaintBatchKind::Rects,
                rect_start,
                drawable_rects.len() - rect_start,
            );
            push_paint_batch(
                &mut paint_batches,
                UiGpuPaintBatchKind::Text,
                text_start,
                text_glyphs.len() - text_start,
            );
            push_paint_batch(
                &mut paint_batches,
                UiGpuPaintBatchKind::ViewportTextures,
                viewport_start,
                viewport_texture_quads.len() - viewport_start,
            );
            push_paint_batch(
                &mut paint_batches,
                UiGpuPaintBatchKind::ImageTextures,
                image_start,
                image_texture_quads.len() - image_start,
            );
        }

        Ok(Self {
            schema_version: UI_GPU_DRAW_PLAN_SCHEMA_VERSION.to_string(),
            surface_width: draw_list.surface_width.round() as u32,
            surface_height: draw_list.surface_height.round() as u32,
            draw_command_count: draw_list.commands.len(),
            rect_count,
            text_command_count,
            skipped_text_count,
            rendered_glyph_count,
            unsupported_glyph_count,
            font_backend: font_system.backend().to_string(),
            font_loaded: font_system.font_loaded(),
            font_source: font_system.font_source().map(str::to_string),
            glyph_atlas_width: font_system.atlas.width,
            glyph_atlas_height: font_system.atlas.height,
            glyph_cache_count: font_system.cache.len(),
            missing_glyph_count: font_system.missing_glyph_count,
            viewport_slot_count,
            viewport_texture_quad_count: viewport_texture_quads.len(),
            viewport_texture_fallback_count,
            image_texture_slot_count,
            image_texture_quad_count: image_texture_quads.len(),
            image_texture_fallback_count,
            hit_region_count: draw_list.hit_regions.len(),
            drawable_rects,
            text_glyphs,
            viewport_texture_quads,
            image_texture_quads,
            paint_batches,
            glyph_atlas_alpha: font_system.atlas.pixels,
        })
    }
}

fn push_paint_batch(
    batches: &mut Vec<UiGpuPaintBatch>,
    kind: UiGpuPaintBatchKind,
    first_item: usize,
    item_count: usize,
) {
    if item_count == 0 {
        return;
    }
    if let Some(previous) = batches.last_mut() {
        if previous.kind == kind && previous.first_item + previous.item_count == first_item {
            previous.item_count += item_count;
            return;
        }
    }
    batches.push(UiGpuPaintBatch {
        kind,
        first_item,
        item_count,
    });
}

fn clipped_rect(rect: UiRect, clip: Option<UiRect>) -> Option<UiRect> {
    let Some(clip) = clip else {
        return Some(rect);
    };
    let x = rect.x.max(clip.x);
    let y = rect.y.max(clip.y);
    let right = (rect.x + rect.width).min(clip.x + clip.width);
    let bottom = (rect.y + rect.height).min(clip.y + clip.height);
    (right > x && bottom > y).then_some(UiRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

fn clipped_textured_rect(
    rect: UiRect,
    source_uv: UiUvRect,
    clip: Option<UiRect>,
) -> Option<(UiRect, UiUvRect)> {
    let clipped = clipped_rect(rect, clip)?;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    let u_span = source_uv.u1 - source_uv.u0;
    let v_span = source_uv.v1 - source_uv.v0;
    let uv = UiUvRect {
        u0: source_uv.u0 + ((clipped.x - rect.x) / rect.width) * u_span,
        v0: source_uv.v0 + ((clipped.y - rect.y) / rect.height) * v_span,
        u1: source_uv.u0 + ((clipped.x + clipped.width - rect.x) / rect.width) * u_span,
        v1: source_uv.v0 + ((clipped.y + clipped.height - rect.y) / rect.height) * v_span,
    };
    Some((clipped, uv))
}
