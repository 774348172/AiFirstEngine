//! Normalization for the existing texture Cook owner. Accept decoded RGBA8 only;
//! decoding, AssetGraph lookup and package publication stay with that owner.
use super::{Page, Result, fail};
impl Page {
    /// Unpremultiply source gamma-PMA BEFORE sRGB decoding, then premultiply in
    /// linear space. Returned bytes require a linear UNORM GPU format.
    pub fn linear_premultiplied_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<Vec<u8>> {
        if width != self.width
            || height != self.height
            || width == 0
            || height == 0
            || width > 4096
            || height > 4096
        {
            return fail(
                "/texture/dimensions",
                "decoded image differs from atlas or exceeds capacity",
            );
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if expected != Some(rgba.len()) {
            return fail("/texture/pixels", "decoded RGBA8 length mismatch");
        }
        let mut output = Vec::with_capacity(rgba.len());
        for pixel in rgba.chunks_exact(4) {
            if self.pma && pixel[3] != 0 && pixel[..3].iter().any(|c| *c > pixel[3]) {
                return fail("/texture/pma", "gamma-premultiplied channel exceeds alpha");
            }
            let a = f64::from(pixel[3]) / 255.0;
            for &channel in &pixel[..3] {
                let encoded = if a == 0.0 {
                    0.0
                } else if self.pma {
                    (f64::from(channel) / 255.0 / a).min(1.0)
                } else {
                    f64::from(channel) / 255.0
                };
                let linear = if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                };
                output.push((linear * a * 255.0).round().clamp(0.0, 255.0) as u8);
            }
            output.push(pixel[3]);
        }
        Ok(output)
    }
}
