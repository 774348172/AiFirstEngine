//! Deterministic Unicode shaping owner for AUI text.
//!
//! This module deliberately stops at GlyphRun. AUI layout and atlas quad
//! generation consume the run but do not perform shaping a second time.

use rustybuzz::{Face, UnicodeBuffer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphRunItem {
    pub glyph_id: u32,
    pub cluster_start: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub advance_x: i32,
    pub advance_y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphRun {
    pub face_id: String,
    pub units_per_em: u16,
    pub text_digest: String,
    pub direction: String,
    pub items: Vec<GlyphRunItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphPlacement {
    pub x: i32,
    pub y: i32,
    pub advance_x: i32,
}

pub fn layout_glyph_run(run: &GlyphRun) -> Vec<GlyphPlacement> {
    let mut cursor = 0;
    run.items
        .iter()
        .map(|item| {
            let placement = GlyphPlacement {
                x: cursor + item.offset_x,
                y: item.offset_y,
                advance_x: item.advance_x,
            };
            cursor += item.advance_x;
            placement
        })
        .collect()
}

pub fn shape_text(face_id: impl Into<String>, font_bytes: &[u8], text: &str) -> Option<GlyphRun> {
    let face = Face::from_slice(font_bytes, 0)?;
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    let buffer = rustybuzz::shape(&face, &[], buffer);
    let items = buffer
        .glyph_infos()
        .iter()
        .zip(buffer.glyph_positions())
        .map(|(info, position)| GlyphRunItem {
            glyph_id: info.glyph_id,
            cluster_start: info.cluster,
            offset_x: position.x_offset,
            offset_y: position.y_offset,
            advance_x: position.x_advance,
            advance_y: position.y_advance,
        })
        .collect();
    Some(GlyphRun {
        face_id: face_id.into(),
        units_per_em: face.units_per_em().clamp(1, u16::MAX as i32) as u16,
        text_digest: md5_digest(text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        direction: "ltr".to_string(),
        items,
    })
}

fn md5_digest(bytes: &[u8]) -> [u8; 16] {
    // Keep Gate A independent of a second hash dependency; this compact,
    // deterministic digest is an identity hint, not a security hash.
    let mut out = [0u8; 16];
    for (index, byte) in bytes.iter().enumerate() {
        out[index % 16] = out[index % 16]
            .wrapping_add(*byte)
            .rotate_left((index % 8) as u32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_cjk_latin_and_punctuation_deterministically() {
        let bytes = include_bytes!("../../../resources/editor/fonts/NotoSansSC-VF.ttf");
        let text = "中文，句号。引号“测试” 01:23 Agpy";
        let first = shape_text("noto", bytes, text).expect("font face should load");
        let second = shape_text("noto", bytes, text).expect("font face should load");
        assert_eq!(first, second);
        assert!(!first.items.is_empty());
        assert!(first
            .items
            .windows(2)
            .all(|pair| { pair[0].cluster_start <= pair[1].cluster_start }));
        assert!(first.items.iter().all(|item| item.advance_x >= 0));
        let layout = layout_glyph_run(&first);
        assert_eq!(layout.len(), first.items.len());
        assert!(layout.windows(2).all(|pair| pair[1].x >= pair[0].x));
    }
}
