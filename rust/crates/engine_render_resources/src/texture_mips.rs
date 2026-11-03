//! Tight texture mip layout and semantic downsampling. No GPU or asset owner.
#[cfg(test)]
use generate_rgba8_mip_chain as generate_rgba8_chain;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureMipLevel {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub offset: usize,
    pub byte_len: usize,
}

/// Tight block geometry; unknown formats must never fall back to RGBA8.
pub fn texture_block_layout(format: &str) -> Option<(u32, u32, u32)> {
    match format {
        "rgba8Unorm" | "rgba8UnormSrgb" => Some((1, 1, 4)),
        "bc3RgbaUnorm" | "bc3RgbaUnormSrgb" => Some((4, 4, 16)),
        _ => None,
    }
}

pub fn rgba8_mip_layout(width: u32, height: u32, count: u32) -> Option<Vec<TextureMipLevel>> {
    texture_mip_layout("rgba8Unorm", width, height, count)
}

pub fn texture_mip_layout(
    format: &str,
    width: u32,
    height: u32,
    count: u32,
) -> Option<Vec<TextureMipLevel>> {
    let (bw, bh, block_bytes) = texture_block_layout(format)?;
    if width == 0 || height == 0 || count == 0 || count > 32 - width.max(height).leading_zeros() {
        return None;
    }
    let mut result = Vec::with_capacity(count as usize);
    let mut offset = 0usize;
    for level in 0..count {
        let width = (width >> level).max(1);
        let height = (height >> level).max(1);
        let byte_len = (width.div_ceil(bw) as usize)
            .checked_mul(height.div_ceil(bh) as usize)?
            .checked_mul(block_bytes as usize)?;
        result.push(TextureMipLevel {
            level,
            width,
            height,
            offset,
            byte_len,
        });
        offset = offset.checked_add(byte_len)?;
    }
    Some(result)
}

pub fn rgba8_mip_byte_len(width: u32, height: u32, count: u32) -> Option<usize> {
    let levels = rgba8_mip_layout(width, height, count)?;
    let last = levels.last()?;
    last.offset.checked_add(last.byte_len)
}

pub fn texture_mip_byte_len(format: &str, width: u32, height: u32, count: u32) -> Option<usize> {
    let levels = texture_mip_layout(format, width, height, count)?;
    let last = levels.last()?;
    last.offset.checked_add(last.byte_len)
}

fn decode(value: u8, srgb: bool) -> f64 {
    let x = f64::from(value) / 255.;
    if !srgb {
        x
    } else if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}
fn encode(value: f64, srgb: bool) -> u8 {
    let x = if !srgb {
        value
    } else if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1. / 2.4) - 0.055
    };
    (x.clamp(0., 1.) * 255.).round() as u8
}

pub fn generate_rgba8_mip_chain(
    width: u32,
    height: u32,
    base: &[u8],
    srgb: bool,
) -> Result<Vec<u8>, &'static str> {
    let count = 32 - width.max(height).leading_zeros();
    let levels = rgba8_mip_layout(width, height, count).ok_or("texture.mip_extent_invalid")?;
    if base.len() != levels[0].byte_len {
        return Err("texture.mip_base_length_invalid");
    }
    let mut result = Vec::with_capacity(rgba8_mip_byte_len(width, height, count).unwrap());
    result.extend_from_slice(base);
    for pair in levels.windows(2) {
        let (source, target) = (pair[0], pair[1]);
        // Area coverage includes odd edge texels instead of dropping the final row/column.
        for y in 0..target.height {
            let y0 = f64::from(y) * f64::from(source.height) / f64::from(target.height);
            let y1 = f64::from(y + 1) * f64::from(source.height) / f64::from(target.height);
            for x in 0..target.width {
                let x0 = f64::from(x) * f64::from(source.width) / f64::from(target.width);
                let x1 = f64::from(x + 1) * f64::from(source.width) / f64::from(target.width);
                let mut sum = [0.; 4];
                for sy in y0.floor() as u32..y1.ceil() as u32 {
                    for sx in x0.floor() as u32..x1.ceil() as u32 {
                        let weight = (x1.min(f64::from(sx + 1)) - x0.max(f64::from(sx)))
                            * (y1.min(f64::from(sy + 1)) - y0.max(f64::from(sy)));
                        let i =
                            source.offset + (sy as usize * source.width as usize + sx as usize) * 4;
                        for channel in 0..4 {
                            sum[channel] +=
                                decode(result[i + channel], srgb && channel < 3) * weight;
                        }
                    }
                }
                let area = (x1 - x0) * (y1 - y0);
                for (channel, value) in sum.into_iter().enumerate() {
                    result.push(encode(value / area, srgb && channel < 3));
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bc3_block_layout_charges_tail_mips_and_checks_overflow() {
        assert_eq!(texture_mip_byte_len("bc3RgbaUnorm", 4, 4, 3), Some(48));
        assert_eq!(texture_mip_byte_len("bc3RgbaUnormSrgb", 8, 4, 4), Some(80));
        assert_eq!(
            texture_mip_byte_len("bc3RgbaUnorm", 2048, 2048, 12),
            Some(5_592_432)
        );
        assert!(texture_mip_byte_len("bc7RgbaUnorm", 4, 4, 3).is_none());
        assert!(texture_mip_byte_len("bc3RgbaUnorm", 4, 4, 4).is_none());
        assert!(texture_mip_byte_len("bc3RgbaUnorm", 0, 4, 1).is_none());
    }
    #[test]
    fn mip_layout_rejects_invalid_and_charges_every_level() {
        assert_eq!(rgba8_mip_byte_len(4, 4, 3), Some(84));
        assert_eq!(rgba8_mip_byte_len(3, 5, 3), Some(72));
        assert!(rgba8_mip_layout(0, 1, 1).is_none());
        assert!(rgba8_mip_layout(4, 4, 4).is_none());
        assert!(rgba8_mip_layout(1, 1, 0).is_none());
    }
    #[test]
    fn srgb_mips_filter_in_linear_light_and_alpha_stays_linear() {
        let base = [0, 0, 0, 0, 255, 255, 255, 255];
        let color = generate_rgba8_chain(2, 1, &base, true).unwrap();
        assert_eq!(&color[8..], [188, 188, 188, 128]);
        let data = generate_rgba8_chain(2, 1, &base, false).unwrap();
        assert_eq!(&data[8..], [128, 128, 128, 128]);
    }
    #[test]
    fn odd_extent_last_texel_contributes_and_thin_chain_reaches_one() {
        let base = [0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255];
        let chain = generate_rgba8_chain(3, 1, &base, false).unwrap();
        assert_eq!(&chain[12..], [85, 85, 85, 255]);
        assert!(generate_rgba8_chain(3, 1, &base[..8], false).is_err());
    }
}
