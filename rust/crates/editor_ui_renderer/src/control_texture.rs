use serde::Deserialize;

use crate::{ControlBrush, ControlSliceInsets, DrawCommand, UiColor, UiRect, UiUvRect};

pub const EDITOR_THEME_TEXTURE_MANIFEST_SCHEMA_VERSION: &str = "editor-theme-texture-manifest.v1";

const DARK_NEUTRAL_TEXTURE_MANIFEST: &str =
    include_str!("../../../resources/editor/themes/dark-neutral/control-textures.v1.json");
const TAB_HOVER_PNG: &[u8] =
    include_bytes!("../../../resources/editor/themes/dark-neutral/tab-hover.png");
const TAB_ACTIVE_PNG: &[u8] =
    include_bytes!("../../../resources/editor/themes/dark-neutral/tab-active.png");
const TAB_SELECTED_PNG: &[u8] =
    include_bytes!("../../../resources/editor/themes/dark-neutral/tab-selected.png");
const TAB_SELECTED_HOVER_PNG: &[u8] =
    include_bytes!("../../../resources/editor/themes/dark-neutral/tab-selected-hover.png");

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltInControlTexture {
    pub texture_id: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
    pub slice: ControlSliceInsets,
    pub fallback_color: UiColor,
    pub png_bytes: &'static [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlTextureDiagnostic {
    pub code: String,
    pub texture_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlBrushPaintOutput {
    pub commands: Vec<DrawCommand>,
    pub diagnostics: Vec<ControlTextureDiagnostic>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TextureManifestSpec {
    schema_version: String,
    theme_id: String,
    textures: Vec<TextureSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TextureSpec {
    texture_id: String,
    path: String,
    width: u32,
    height: u32,
    sha256: String,
    slice: ControlSliceInsets,
    fallback_color: String,
}

pub fn dark_neutral_control_textures(
) -> Result<Vec<BuiltInControlTexture>, Vec<ControlTextureDiagnostic>> {
    let manifest: TextureManifestSpec = serde_json::from_str(DARK_NEUTRAL_TEXTURE_MANIFEST)
        .map_err(|error| {
            vec![ControlTextureDiagnostic {
                code: "editor_theme_texture.manifest_invalid".to_string(),
                texture_id: None,
                message: error.to_string(),
            }]
        })?;
    let mut diagnostics = Vec::new();
    if manifest.schema_version != EDITOR_THEME_TEXTURE_MANIFEST_SCHEMA_VERSION {
        diagnostics.push(ControlTextureDiagnostic {
            code: "editor_theme_texture.schema_unsupported".to_string(),
            texture_id: None,
            message: manifest.schema_version.clone(),
        });
    }
    if manifest.theme_id != "dark-neutral" {
        diagnostics.push(ControlTextureDiagnostic {
            code: "editor_theme_texture.theme_mismatch".to_string(),
            texture_id: None,
            message: manifest.theme_id.clone(),
        });
    }
    let mut textures = Vec::with_capacity(manifest.textures.len());
    for spec in manifest.textures {
        let png_bytes = embedded_png(&spec.path);
        let fallback_color = parse_hex_color(&spec.fallback_color);
        if spec.width == 0
            || spec.height == 0
            || !valid_sha256(&spec.sha256)
            || png_bytes.is_none()
            || fallback_color.is_none()
        {
            diagnostics.push(ControlTextureDiagnostic {
                code: "editor_theme_texture.entry_invalid".to_string(),
                texture_id: Some(spec.texture_id),
                message: spec.path,
            });
            continue;
        }
        textures.push(BuiltInControlTexture {
            texture_id: spec.texture_id,
            path: spec.path,
            width: spec.width,
            height: spec.height,
            sha256: spec.sha256,
            slice: spec.slice,
            fallback_color: fallback_color.expect("checked above"),
            png_bytes: png_bytes.expect("checked above"),
        });
    }
    if diagnostics.is_empty() {
        Ok(textures)
    } else {
        Err(diagnostics)
    }
}

pub fn paint_control_brush(
    rect: UiRect,
    brush: &ControlBrush,
    opacity: f32,
) -> ControlBrushPaintOutput {
    let opacity = opacity.clamp(0.0, 1.0);
    match brush {
        ControlBrush::None => ControlBrushPaintOutput {
            commands: Vec::new(),
            diagnostics: Vec::new(),
        },
        ControlBrush::Solid { color } => ControlBrushPaintOutput {
            commands: vec![DrawCommand::Rect {
                rect,
                color: with_opacity(*color, opacity),
                corner_radius: 0.0,
            }],
            diagnostics: Vec::new(),
        },
        ControlBrush::Texture {
            texture_id,
            fallback_color,
            tint,
        } => ControlBrushPaintOutput {
            commands: vec![image_command(
                rect,
                UiUvRect::FULL,
                texture_id,
                with_opacity(*fallback_color, opacity),
                with_opacity(*tint, opacity),
            )],
            diagnostics: Vec::new(),
        },
        ControlBrush::NineSlice {
            texture_id,
            fallback_color,
            tint,
            slice,
        } => {
            let textures = dark_neutral_control_textures().unwrap_or_default();
            let Some(texture) = textures.iter().find(|item| item.texture_id == *texture_id) else {
                return ControlBrushPaintOutput {
                    commands: vec![DrawCommand::Rect {
                        rect,
                        color: with_opacity(*fallback_color, opacity),
                        corner_radius: 0.0,
                    }],
                    diagnostics: vec![ControlTextureDiagnostic {
                        code: "editor_theme_texture.texture_missing".to_string(),
                        texture_id: Some(texture_id.clone()),
                        message: "Nine-Slice texture is not in the built-in manifest.".to_string(),
                    }],
                };
            };
            ControlBrushPaintOutput {
                commands: expand_nine_slice(
                    rect,
                    texture.width as f32,
                    texture.height as f32,
                    *slice,
                    texture_id,
                    with_opacity(*fallback_color, opacity),
                    with_opacity(*tint, opacity),
                ),
                diagnostics: Vec::new(),
            }
        }
    }
}

fn expand_nine_slice(
    rect: UiRect,
    source_width: f32,
    source_height: f32,
    slice: ControlSliceInsets,
    texture_id: &str,
    fallback_color: UiColor,
    tint: UiColor,
) -> Vec<DrawCommand> {
    if rect.width <= 0.0 || rect.height <= 0.0 || source_width <= 0.0 || source_height <= 0.0 {
        return Vec::new();
    }
    let (source_left, source_right) = clamp_pair(slice.left, slice.right, source_width);
    let (source_top, source_bottom) = clamp_pair(slice.top, slice.bottom, source_height);
    let (target_left, target_right) = clamp_pair(source_left, source_right, rect.width);
    let (target_top, target_bottom) = clamp_pair(source_top, source_bottom, rect.height);
    let xs = [
        rect.x,
        rect.x + target_left,
        rect.x + rect.width - target_right,
        rect.x + rect.width,
    ];
    let ys = [
        rect.y,
        rect.y + target_top,
        rect.y + rect.height - target_bottom,
        rect.y + rect.height,
    ];
    let us = [
        0.0,
        source_left / source_width,
        1.0 - source_right / source_width,
        1.0,
    ];
    let vs = [
        0.0,
        source_top / source_height,
        1.0 - source_bottom / source_height,
        1.0,
    ];
    let mut commands = Vec::with_capacity(9);
    for row in 0..3 {
        for column in 0..3 {
            let width = xs[column + 1] - xs[column];
            let height = ys[row + 1] - ys[row];
            if width <= 0.0 || height <= 0.0 {
                continue;
            }
            commands.push(image_command(
                UiRect {
                    x: xs[column],
                    y: ys[row],
                    width,
                    height,
                },
                UiUvRect {
                    u0: us[column],
                    v0: vs[row],
                    u1: us[column + 1],
                    v1: vs[row + 1],
                },
                texture_id,
                fallback_color,
                tint,
            ));
        }
    }
    commands
}

fn image_command(
    rect: UiRect,
    source_uv: UiUvRect,
    texture_id: &str,
    fallback_color: UiColor,
    tint: UiColor,
) -> DrawCommand {
    DrawCommand::ImageTextureSlot {
        rect,
        source_uv,
        texture_id: Some(texture_id.to_string()),
        fallback_color,
        tint,
    }
}

fn clamp_pair(first: f32, second: f32, available: f32) -> (f32, f32) {
    let first = first.max(0.0);
    let second = second.max(0.0);
    let total = first + second;
    if total <= available || total == 0.0 {
        (first, second)
    } else {
        let scale = available / total;
        (first * scale, second * scale)
    }
}

fn with_opacity(mut color: UiColor, opacity: f32) -> UiColor {
    color.a = ((color.a as f32 * opacity).round()).clamp(0.0, 255.0) as u8;
    color
}

fn embedded_png(path: &str) -> Option<&'static [u8]> {
    match path {
        "tab-hover.png" => Some(TAB_HOVER_PNG),
        "tab-active.png" => Some(TAB_ACTIVE_PNG),
        "tab-selected.png" => Some(TAB_SELECTED_PNG),
        "tab-selected-hover.png" => Some(TAB_SELECTED_HOVER_PNG),
        _ => None,
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_hex_color(value: &str) -> Option<UiColor> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 8 {
        return None;
    }
    Some(UiColor::rgba(
        u8::from_str_radix(&digits[0..2], 16).ok()?,
        u8::from_str_radix(&digits[2..4], 16).ok()?,
        u8::from_str_radix(&digits[4..6], 16).ok()?,
        u8::from_str_radix(&digits[6..8], 16).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nine_slice() -> ControlBrush {
        ControlBrush::NineSlice {
            texture_id: "editor-control-tab-hover".to_string(),
            fallback_color: UiColor::PANEL,
            tint: UiColor::rgba(200, 210, 220, 255),
            slice: ControlSliceInsets {
                left: 4.0,
                top: 4.0,
                right: 4.0,
                bottom: 4.0,
            },
        }
    }

    #[test]
    fn control_brush_nine_slice_expands_in_row_major_painter_order() {
        let output = paint_control_brush(
            UiRect {
                x: 10.0,
                y: 20.0,
                width: 30.0,
                height: 18.0,
            },
            &nine_slice(),
            0.5,
        );
        assert!(output.diagnostics.is_empty());
        assert_eq!(output.commands.len(), 9);
        let DrawCommand::ImageTextureSlot {
            rect,
            source_uv,
            tint,
            ..
        } = &output.commands[0]
        else {
            panic!("expected image quad");
        };
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (10.0, 20.0, 4.0, 4.0)
        );
        assert_eq!((source_uv.u1, source_uv.v1), (1.0 / 3.0, 1.0 / 3.0));
        assert_eq!(tint.a, 128);
        let DrawCommand::ImageTextureSlot { rect, .. } = &output.commands[8] else {
            panic!("expected image quad");
        };
        assert_eq!((rect.x, rect.y), (36.0, 34.0));
    }

    #[test]
    fn control_brush_nine_slice_clamps_small_target_without_negative_rects() {
        let output = paint_control_brush(
            UiRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 3.0,
            },
            &nine_slice(),
            1.0,
        );
        assert!(output.commands.len() <= 9);
        for command in output.commands {
            let DrawCommand::ImageTextureSlot { rect, .. } = command else {
                panic!("expected image quad");
            };
            assert!(rect.width > 0.0 && rect.height > 0.0);
            assert!(rect.x >= 0.0 && rect.y >= 0.0);
            assert!(rect.x + rect.width <= 5.0);
            assert!(rect.y + rect.height <= 3.0);
        }
    }

    #[test]
    fn control_brush_manifest_has_real_embedded_png_payloads() {
        let textures = dark_neutral_control_textures().expect("valid built-in manifest");
        assert_eq!(textures.len(), 4);
        assert!(textures
            .iter()
            .all(|texture| texture.png_bytes.starts_with(b"\x89PNG\r\n\x1a\n")));
    }
}
