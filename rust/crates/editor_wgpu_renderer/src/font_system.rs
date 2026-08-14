use ab_glyph::{point, Font, FontArc, Glyph, PxScale, ScaleFont};
use editor_ui_renderer::{UiColor, UiRect};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::draw_plan::{UiGpuDrawableRect, UiGpuDrawableRectSource, UiGpuTextGlyph, UiUvRect};
use crate::texture_atlas::{CpuGlyphAtlas, GlyphAtlasEntry, GlyphKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TextRenderStats {
    pub(crate) rendered_glyph_count: usize,
    pub(crate) unsupported_glyph_count: usize,
    pub(crate) skipped: bool,
    pub(crate) used_builtin_fallback: bool,
}

pub(crate) struct EditorFontSystem {
    fonts: Vec<EditorFontFace>,
    source_summary: Option<String>,
    pub(crate) atlas: CpuGlyphAtlas,
    pub(crate) cache: HashMap<GlyphKey, GlyphAtlasEntry>,
    pub(crate) missing_glyph_count: usize,
}

struct EditorFontFace {
    id: u16,
    font: FontArc,
    source: String,
}

impl EditorFontSystem {
    pub(crate) fn new() -> Self {
        let fonts = load_editor_font_stack();
        let source_summary = (!fonts.is_empty()).then(|| {
            fonts
                .iter()
                .map(|face| face.source.as_str())
                .collect::<Vec<_>>()
                .join(";")
        });
        Self {
            fonts,
            source_summary,
            atlas: CpuGlyphAtlas::new(1024, 1024),
            cache: HashMap::new(),
            missing_glyph_count: 0,
        }
    }

    pub(crate) fn backend(&self) -> &'static str {
        if !self.fonts.is_empty() {
            "ab_glyph_atlas"
        } else {
            "builtin_debug_font"
        }
    }

    pub(crate) fn font_loaded(&self) -> bool {
        !self.fonts.is_empty()
    }

    pub(crate) fn font_source(&self) -> Option<&str> {
        self.source_summary.as_deref()
    }

    pub(crate) fn layout_text(
        &mut self,
        output: &mut Vec<UiGpuTextGlyph>,
        rect: UiRect,
        text: &str,
        color: UiColor,
        size: f32,
    ) -> TextRenderStats {
        if rect.width < 4.0 || rect.height < 6.0 || size < 6.0 {
            return TextRenderStats {
                skipped: true,
                ..Default::default()
            };
        }
        if self.fonts.is_empty() {
            return TextRenderStats {
                used_builtin_fallback: true,
                ..Default::default()
            };
        }

        let mut x = rect.x;
        let baseline = rect.y + (rect.height * 0.5) + (size * 0.36);
        let max_x = rect.x + rect.width;
        let mut stats = TextRenderStats::default();
        for ch in text.chars() {
            let normalized = if ch.is_control() { '?' } else { ch };
            let Some((font_face_id, rendered_ch)) = self.select_face(normalized) else {
                stats.unsupported_glyph_count += 1;
                self.missing_glyph_count += 1;
                continue;
            };
            let key = GlyphKey {
                font_face_id,
                ch: rendered_ch,
                px_size: size.round().clamp(6.0, 96.0) as u32,
            };
            let Some(entry) = self.glyph_entry(key) else {
                self.missing_glyph_count += 1;
                continue;
            };
            if x + entry.advance > max_x {
                break;
            }
            if entry.width > 0 && entry.height > 0 {
                output.push(UiGpuTextGlyph {
                    rect: UiRect {
                        x: x + entry.bearing_x,
                        y: baseline - entry.bearing_y,
                        width: entry.width as f32,
                        height: entry.height as f32,
                    },
                    uv: entry.uv,
                    color,
                });
            }
            stats.rendered_glyph_count += 1;
            x += entry.advance;
        }
        if stats.rendered_glyph_count == 0 && !text.is_empty() {
            stats.skipped = true;
        }
        stats
    }

    fn select_face(&self, ch: char) -> Option<(u16, char)> {
        if let Some(face) = self.fonts.iter().find(|face| face.font.glyph_id(ch).0 != 0) {
            return Some((face.id, ch));
        }

        self.fonts
            .iter()
            .find(|face| face.font.glyph_id('?').0 != 0)
            .map(|face| (face.id, '?'))
    }

    fn glyph_entry(&mut self, key: GlyphKey) -> Option<GlyphAtlasEntry> {
        if let Some(entry) = self.cache.get(&key) {
            return Some(*entry);
        }
        let font = self
            .fonts
            .iter()
            .find(|face| face.id == key.font_face_id)?
            .font
            .clone();
        let scale = PxScale::from(key.px_size as f32);
        let scaled = font.as_scaled(scale);
        let glyph = Glyph {
            id: font.glyph_id(key.ch),
            scale,
            position: point(0.0, 0.0),
        };
        if glyph.id.0 == 0 {
            return None;
        }
        let advance = scaled.h_advance(glyph.id).max(key.px_size as f32 * 0.35);
        let Some(outlined) = font.outline_glyph(glyph) else {
            let entry = GlyphAtlasEntry {
                uv: UiUvRect {
                    u0: 0.0,
                    v0: 0.0,
                    u1: 0.0,
                    v1: 0.0,
                },
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance,
            };
            self.cache.insert(key, entry);
            return Some(entry);
        };
        let bounds = outlined.px_bounds();
        let width = bounds.width().ceil().max(1.0) as u32;
        let height = bounds.height().ceil().max(1.0) as u32;
        let (atlas_x, atlas_y) = self.atlas.allocate(width, height)?;
        let mut bitmap = vec![0u8; (width * height) as usize];
        outlined.draw(|x, y, coverage| {
            let idx = (y * width + x) as usize;
            bitmap[idx] = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
        });
        self.atlas
            .write_alpha(atlas_x, atlas_y, width, height, &bitmap);
        let entry = GlyphAtlasEntry {
            uv: UiUvRect {
                u0: atlas_x as f32 / self.atlas.width as f32,
                v0: atlas_y as f32 / self.atlas.height as f32,
                u1: (atlas_x + width) as f32 / self.atlas.width as f32,
                v1: (atlas_y + height) as f32 / self.atlas.height as f32,
            },
            width,
            height,
            bearing_x: bounds.min.x,
            bearing_y: -bounds.min.y,
            advance,
        };
        self.cache.insert(key, entry);
        Some(entry)
    }
}

fn load_editor_font_stack() -> Vec<EditorFontFace> {
    let mut fonts = Vec::new();
    for path in system_font_candidates() {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        if let Ok(font) = FontArc::try_from_vec(bytes) {
            push_font_face(&mut fonts, font, path.display().to_string());
            break;
        }
    }

    let cjk_bytes = include_bytes!("../../../resources/editor/fonts/NotoSansSC-VF.ttf");
    if let Ok(font) = FontArc::try_from_slice(cjk_bytes) {
        push_font_face(&mut fonts, font, "embedded:NotoSansSC-VF.ttf".to_string());
    }
    fonts
}

fn push_font_face(fonts: &mut Vec<EditorFontFace>, font: FontArc, source: String) {
    let id = u16::try_from(fonts.len()).expect("editor font stack exceeds u16 face identity");
    fonts.push(EditorFontFace { id, font, source });
}

fn system_font_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if cfg!(target_os = "windows") {
        candidates.push(PathBuf::from(r"C:\Windows\Fonts\segoeui.ttf"));
        candidates.push(PathBuf::from(r"C:\Windows\Fonts\arial.ttf"));
        candidates.push(PathBuf::from(r"C:\Windows\Fonts\consola.ttf"));
    }
    candidates
}

pub(crate) struct BuiltinDebugFont;

impl BuiltinDebugFont {
    const GLYPH_WIDTH: usize = 5;
    const GLYPH_HEIGHT: usize = 7;

    pub(crate) fn push_text_rects(
        output: &mut Vec<UiGpuDrawableRect>,
        rect: UiRect,
        text: &str,
        color: UiColor,
        size: f32,
    ) -> TextRenderStats {
        if rect.width < 4.0 || rect.height < 6.0 || size < 6.0 {
            return TextRenderStats {
                skipped: true,
                ..Default::default()
            };
        }

        let glyph_h = size.min(rect.height).max(6.0);
        let cell = (glyph_h / Self::GLYPH_HEIGHT as f32).max(1.0);
        let advance = (cell * (Self::GLYPH_WIDTH as f32 + 1.0)).max(4.0);
        let max_x = rect.x + rect.width;
        let max_y = rect.y + rect.height;
        let mut x = rect.x;
        let y = rect.y + ((rect.height - cell * Self::GLYPH_HEIGHT as f32) * 0.5).max(0.0);
        let mut stats = TextRenderStats::default();

        for ch in text.chars() {
            if x + advance > max_x {
                break;
            }
            if ch == ' ' {
                x += advance;
                continue;
            }

            let (glyph, unsupported) = glyph_rows(ch);
            if unsupported {
                stats.unsupported_glyph_count += 1;
            }

            for (row_index, row) in glyph.iter().enumerate() {
                for col in 0..Self::GLYPH_WIDTH {
                    let mask = 1 << (Self::GLYPH_WIDTH - 1 - col);
                    if row & mask == 0 {
                        continue;
                    }
                    let glyph_rect = UiRect {
                        x: x + col as f32 * cell,
                        y: y + row_index as f32 * cell,
                        width: (cell * 0.82).max(1.0),
                        height: (cell * 0.82).max(1.0),
                    };
                    if glyph_rect.y + glyph_rect.height <= max_y {
                        output.push(UiGpuDrawableRect {
                            rect: glyph_rect,
                            color,
                            source_kind: UiGpuDrawableRectSource::TextGlyph,
                        });
                    }
                }
            }
            stats.rendered_glyph_count += 1;
            x += advance;
        }

        if stats.rendered_glyph_count == 0 && !text.is_empty() {
            stats.skipped = true;
        }
        stats
    }
}

fn glyph_rows(ch: char) -> ([u8; 7], bool) {
    let normalized = if ch.is_ascii_lowercase() {
        ch.to_ascii_uppercase()
    } else {
        ch
    };
    let unsupported = !normalized.is_ascii() || !(' '..='~').contains(&normalized);
    let rows = match normalized {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b01010, 0b01010, 0b00100, 0b01010, 0b01010, 0b10001,
        ],
        'Y' => [
            0b10001, 0b01010, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '\\' => [
            0b10000, 0b01000, 0b01000, 0b00100, 0b00010, 0b00010, 0b00001,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '=' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000,
        ],
        '\'' => [
            0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '"' => [
            0b01010, 0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
        '#' => [
            0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010,
        ],
        '%' => [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
        '&' => [
            0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
        ],
        '*' => [
            0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000,
        ],
        '<' => [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        '>' => [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        '|' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        _ => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
    };
    (rows, unsupported)
}
