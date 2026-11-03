//! Unicode shaping and logical line layout. Renderer consumes final quads only.
use crate::font_bundle::RuntimePackageSourceFontFace;
use engine_content::canonical_digest::sha256_prefixed;
use rustybuzz::{Face, UnicodeBuffer};
use serde::{Deserialize, Serialize};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

pub const SHAPING_RECIPE: &str = "rustybuzz-0.20.1-unicode-bidi-arabic-indic-v2";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphRunItem {
    pub glyph_id: u32,
    pub has_outline: bool,
    pub ink_bounds: Option<[i16; 4]>,
    pub cluster_start: u32,
    pub cluster_end: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub advance_x: i32,
    pub advance_y: i32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphRun {
    pub face_id: String,
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub text_digest: String,
    pub direction: String,
    pub fallback_used: bool,
    pub items: Vec<GlyphRunItem>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct LineGlyph {
    pub run_index: usize,
    pub item_index: usize,
    pub x: f32,
    pub y: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct LineLayout {
    pub ascender: f32,
    pub descender: f32,
    pub line_gap: f32,
    pub baseline: f32,
    pub width: f32,
    pub height: f32,
    pub glyphs: Vec<LineGlyph>,
}

pub fn layout_line(runs: &[GlyphRun], size: f32, box_height: f32, center: bool) -> LineLayout {
    let mut line = LineLayout {
        ascender: 0.0,
        descender: 0.0,
        line_gap: 0.0,
        baseline: 0.0,
        width: 0.0,
        height: 0.0,
        glyphs: Vec::new(),
    };
    for (run_index, run) in runs.iter().enumerate() {
        let scale = size / f32::from(run.units_per_em);
        line.ascender = line.ascender.max(f32::from(run.ascender) * scale);
        line.descender = line.descender.min(f32::from(run.descender) * scale);
        line.line_gap = line.line_gap.max(f32::from(run.line_gap) * scale);
        for (item_index, item) in run.items.iter().enumerate() {
            line.glyphs.push(LineGlyph {
                run_index,
                item_index,
                x: line.width + item.offset_x as f32 * scale,
                y: -item.offset_y as f32 * scale,
            });
            line.width += item.advance_x as f32 * scale;
        }
    }
    line.height = line.ascender - line.descender + line.line_gap;
    line.baseline = line.ascender
        + line.line_gap * 0.5
        + if center {
            (box_height - line.height) * 0.5
        } else {
            0.0
        };
    line
}

pub fn shape_text(face_id: impl Into<String>, font_bytes: &[u8], text: &str) -> Option<GlyphRun> {
    shape_face(face_id, font_bytes, 0, text)
}

pub fn shape_face(
    face_id: impl Into<String>,
    font_bytes: &[u8],
    face_index: u32,
    text: &str,
) -> Option<GlyphRun> {
    let face = Face::from_slice(font_bytes, face_index)?;
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let direction = format!("{:?}", buffer.direction());
    let buffer = rustybuzz::shape(&face, &[], buffer);
    let mut boundaries: Vec<u32> = buffer
        .glyph_infos()
        .iter()
        .map(|info| info.cluster)
        .collect();
    boundaries.push(text.len() as u32);
    boundaries.sort_unstable();
    boundaries.dedup();
    let items = buffer
        .glyph_infos()
        .iter()
        .zip(buffer.glyph_positions())
        .map(|(info, pos)| GlyphRunItem {
            ink_bounds: face
                .glyph_bounding_box(rustybuzz::ttf_parser::GlyphId(info.glyph_id as u16))
                .map(|b| [b.x_min, b.y_min, b.x_max, b.y_max]),
            has_outline: face
                .glyph_bounding_box(rustybuzz::ttf_parser::GlyphId(info.glyph_id as u16))
                .is_some(),
            glyph_id: info.glyph_id,
            cluster_start: info.cluster,
            cluster_end: boundaries
                .iter()
                .copied()
                .find(|end| *end > info.cluster)
                .unwrap_or(text.len() as u32),
            offset_x: pos.x_offset,
            offset_y: pos.y_offset,
            advance_x: pos.x_advance,
            advance_y: pos.y_advance,
        })
        .collect();
    Some(GlyphRun {
        face_id: face_id.into(),
        units_per_em: face.units_per_em() as u16,
        ascender: face.ascender(),
        descender: face.descender(),
        line_gap: face.line_gap(),
        text_digest: sha256_prefixed(text.as_bytes()),
        direction,
        fallback_used: false,
        items,
    })
}

/// Select a face for a complete grapheme, then shape adjacent graphemes together.
/// This preserves ligatures/kerning while preventing split-face combining sequences.
pub fn shape_stack(sources: &[&RuntimePackageSourceFontFace], text: &str) -> Option<Vec<GlyphRun>> {
    if text.is_empty() {
        return Some(Vec::new());
    }
    let parsed: Vec<_> = sources
        .iter()
        .map(|source| Face::from_slice(&source.bytes, source.face_index))
        .collect::<Option<_>>()?;
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    let mut previous_script = Script::Common;
    for (start, grapheme) in text.grapheme_indices(true) {
        let script = grapheme
            .chars()
            .map(|ch| ch.script())
            .find(|s| !matches!(s, Script::Common | Script::Inherited))
            .unwrap_or(previous_script);
        let face_index = parsed
            .iter()
            .position(|face| {
                if grapheme.chars().all(|ch| face.glyph_index(ch).is_some()) {
                    return true;
                }
                // HarfBuzz normalization can compose a grapheme absent from raw cmap coverage.
                let mut buffer = UnicodeBuffer::new();
                buffer.push_str(grapheme);
                buffer.guess_segment_properties();
                rustybuzz::shape(face, &[], buffer)
                    .glyph_infos()
                    .iter()
                    .all(|g| g.glyph_id != 0)
            })
            .unwrap_or(0);
        match spans.last_mut() {
            Some((selected, _, end)) if *selected == face_index && script == previous_script => {
                *end = start + grapheme.len()
            }
            _ => spans.push((face_index, start, start + grapheme.len())),
        }
        previous_script = script;
    }
    let mut runs = Vec::new();
    for (selected, start, end) in spans {
        let source = sources.get(selected)?;
        let mut run = shape_face(
            &source.font_face_id,
            &source.bytes,
            source.face_index,
            &text[start..end],
        )?;
        run.fallback_used = selected != 0;
        for glyph in &mut run.items {
            glyph.cluster_start += start as u32;
            glyph.cluster_end += start as u32;
        }
        runs.push(run);
    }
    Some(runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    const FONT: &[u8] = include_bytes!("../../../resources/editor/fonts/NotoSansSC-VF.ttf");
    #[test]
    fn shapes_cjk_latin_and_punctuation_deterministically() {
        let text = "中文，句号。引号“测试” 01:23 Agpy";
        let a = shape_text("noto", FONT, text).unwrap();
        assert_eq!(a, shape_text("noto", FONT, text).unwrap());
        assert_eq!(a.text_digest, sha256_prefixed(text.as_bytes()));
        assert!(a.ascender > 0 && a.descender < 0);
        assert!(a.items.iter().all(|g| g.cluster_end > g.cluster_start));
        assert!(shape_face("noto", FONT, u32::MAX, text).is_none());
    }
    #[test]
    fn ligatures_and_combining_marks_are_glyphs_not_characters() {
        assert!(
            shape_text("noto", FONT, "ffi").unwrap().items.len() < 3,
            "ligature must reduce glyph count"
        );
        let run = shape_text("noto", FONT, "ffi a\u{301}").unwrap();
        assert!(
            run.items.len() < "ffi a\u{301}".chars().count(),
            "font must exercise actual shaping"
        );
        assert!(run
            .items
            .iter()
            .any(|g| g.cluster_end - g.cluster_start > 1));
    }
    #[test]
    fn fallback_keeps_combining_cluster_together_and_uses_collection_index() {
        let primary = RuntimePackageSourceFontFace {
            font_face_id: "primary".into(),
            face_index: 1,
            source_digest: String::new(),
            bytes: include_bytes!(
                "../../../fixtures/fonts/qualification/AifeNotoSCQualification.ttc"
            )
            .to_vec(),
        };
        let fallback = RuntimePackageSourceFontFace {
            font_face_id: "fallback".into(),
            face_index: 0,
            source_digest: String::new(),
            bytes: FONT.to_vec(),
        };
        assert!(shape_face("primary", &primary.bytes, 1, "A").is_some());
        let text = "A a\u{301}";
        let runs = shape_stack(&[&primary, &fallback], text).unwrap();
        assert_eq!(runs[0].face_id, "primary");
        let last = runs.last().unwrap();
        assert_eq!(last.face_id, "fallback");
        assert!(last.fallback_used);
        assert!(last
            .items
            .iter()
            .all(|g| g.cluster_start >= 2 && g.cluster_end == text.len() as u32));
        let line = layout_line(&runs, 32., 100., false);
        for placed in &line.glyphs {
            let run = &runs[placed.run_index];
            assert!(
                (placed.y
                    + run.items[placed.item_index].offset_y as f32 * 32.
                        / f32::from(run.units_per_em))
                .abs()
                    < 0.001
            );
        }
    }

    #[test]
    fn real_line_metrics_center_all_sizes() {
        let runs = vec![shape_text("noto", FONT, "Ag中文，").unwrap()];
        for size in [14., 18., 24., 32., 48., 64.] {
            let layout = layout_line(&runs, size, 100., true);
            assert!(
                (layout.baseline
                    - layout.ascender
                    - layout.line_gap * 0.5
                    - (100. - layout.height) * 0.5)
                    .abs()
                    < 0.001
            );
            assert!(layout.width > 0.);
        }
    }
}
