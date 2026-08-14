use engine_runtime::runtime_package::{
    CookedFontAtlasAsset, CookedFontAtlasGlyph, COOKED_FONT_ATLAS_SCHEMA_VERSION,
};
use engine_runtime::runtime_package_builder::{
    RuntimePackageSourceFontAtlas, RuntimePackageSourceJson,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const FONT_ATLAS_ID: &str = "ui-default-cmin";
const FONT_ASSET_ID: &str = "font-main";
const CELL_WIDTH: u32 = 8;
const CELL_HEIGHT: u32 = 8;
const GLYPH_WIDTH: u32 = 5;
const GLYPH_HEIGHT: u32 = 7;
const COLUMNS: u32 = 16;

pub struct AuiFontAtlasCookerCmin;

impl AuiFontAtlasCookerCmin {
    pub fn cook_for_documents(
        project_root: &Path,
        documents: &[RuntimePackageSourceJson],
    ) -> RuntimePackageSourceFontAtlas {
        let chars = collect_required_chars(documents);
        let rows = ((chars.len() as u32 + COLUMNS - 1) / COLUMNS).max(1);
        let atlas_width = COLUMNS * CELL_WIDTH;
        let atlas_height = rows * CELL_HEIGHT;
        let mut atlas_alpha = vec![0u8; (atlas_width * atlas_height) as usize];
        let mut glyphs = Vec::new();

        for (index, ch) in chars.into_iter().enumerate() {
            let col = index as u32 % COLUMNS;
            let row = index as u32 / COLUMNS;
            let x = col * CELL_WIDTH + 1;
            let y = row * CELL_HEIGHT;
            let (glyph_rows, unsupported) = glyph_rows(ch);
            for (row_index, bits) in glyph_rows.iter().enumerate() {
                for col_index in 0..GLYPH_WIDTH {
                    let mask = 1 << (GLYPH_WIDTH - 1 - col_index);
                    if bits & mask == 0 {
                        continue;
                    }
                    let px = x + col_index;
                    let py = y + row_index as u32;
                    let offset = (py * atlas_width + px) as usize;
                    atlas_alpha[offset] = 255;
                }
            }
            let pixel_rect = [x, y, GLYPH_WIDTH, GLYPH_HEIGHT];
            let uv_rect = [
                x as f32 / atlas_width as f32,
                y as f32 / atlas_height as f32,
                (x + GLYPH_WIDTH) as f32 / atlas_width as f32,
                (y + GLYPH_HEIGHT) as f32 / atlas_height as f32,
            ];
            glyphs.push(CookedFontAtlasGlyph {
                codepoint: ch as u32,
                glyph_id: if unsupported {
                    "fallback-question".to_string()
                } else {
                    format!("builtin-{:04X}", ch as u32)
                },
                uv_rect,
                pixel_rect,
                bearing_x: 0.0,
                bearing_y: GLYPH_HEIGHT as f32,
                advance: (GLYPH_WIDTH + 1) as f32,
                page_index: 0,
            });
        }

        let font_asset_status = detect_font_asset_status(project_root);
        let metadata = CookedFontAtlasAsset {
            schema_version: COOKED_FONT_ATLAS_SCHEMA_VERSION.to_string(),
            font_atlas_id: FONT_ATLAS_ID.to_string(),
            font_asset_id: FONT_ASSET_ID.to_string(),
            font_source_kind: "engine_builtin_cooked_fallback".to_string(),
            font_asset_status,
            atlas_image_path: format!("fonts/{FONT_ATLAS_ID}.fontatlas.r8"),
            atlas_format: "r8Alpha".to_string(),
            atlas_width,
            atlas_height,
            atlas_generation: 1,
            atlas_alpha_byte_len: atlas_alpha.len(),
            glyphs,
            fallback_used: true,
            diagnostics: vec![
                "AUI FontAtlas C-min used engine builtin cooked fallback glyphs.".to_string(),
                "FontBundleV2MigrationRequired: Import a Project Font Asset and configure a defaultUi FontAtlasProfile to become eligible for the 261 Font Quality Gate.".to_string(),
            ],
        };

        RuntimePackageSourceFontAtlas {
            metadata,
            atlas_alpha,
        }
    }
}

fn collect_required_chars(documents: &[RuntimePackageSourceJson]) -> BTreeSet<char> {
    let mut chars = (' '..='~').collect::<BTreeSet<_>>();
    for document in documents {
        collect_text_chars_from_value(&document.document, &mut chars);
    }
    chars
}

fn collect_text_chars_from_value(value: &serde_json::Value, chars: &mut BTreeSet<char>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if key == "text" {
                    if let Some(text) = value.as_str() {
                        chars.extend(text.chars().map(normalize_char));
                    }
                } else {
                    collect_text_chars_from_value(value, chars);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_text_chars_from_value(value, chars);
            }
        }
        _ => {}
    }
}

fn normalize_char(ch: char) -> char {
    if (' '..='~').contains(&ch) {
        ch
    } else {
        '?'
    }
}

fn detect_font_asset_status(project_root: &Path) -> String {
    let path = project_root.join("Assets").join("font-main.asset");
    if !path.exists() {
        return "missing".to_string();
    }
    match fs::read_to_string(&path) {
        Ok(text) if text.to_ascii_lowercase().contains("placeholder") => "placeholder".to_string(),
        Ok(_) => "unparsed_project_font".to_string(),
        Err(_) => "unreadable".to_string(),
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
        ' ' => [0, 0, 0, 0, 0, 0, 0],
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
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        ',' => [0, 0, 0, 0, 0b01100, 0b00100, 0b01000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        '%' => [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
        _ => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
    };
    (rows, unsupported)
}
