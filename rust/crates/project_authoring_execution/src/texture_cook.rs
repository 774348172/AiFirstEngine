//! Offline target encoding; runtime never runs an encoder or silently decompresses.
use engine_runtime::runtime_package_builder::RuntimePackageSourceTexture;
use engine_runtime::runtime_texture::{
    generate_rgba8_mip_chain, rgba8_mip_layout, texture_mip_byte_len,
};

pub(crate) fn bc3(
    mut source: RuntimePackageSourceTexture,
    srgb: bool,
) -> Result<RuntimePackageSourceTexture, &'static str> {
    let m = &mut source.metadata;
    if !matches!(m.format.as_str(), "rgba8Unorm" | "rgba8UnormSrgb")
        || !matches!(m.color_space.as_str(), "srgb" | "linear")
        || m.mip_count != 1
    {
        return Err("texture.bc3_source_format_or_color_space");
    }
    if m.width == 0 || m.height == 0 || !m.width.is_multiple_of(4) || !m.height.is_multiple_of(4) {
        return Err("texture.bc3_base_extent_requires_multiple_of_four");
    }
    let chain = generate_rgba8_mip_chain(m.width, m.height, &source.rgba8, srgb)?;
    m.mip_count = 32 - m.width.max(m.height).leading_zeros();
    m.format = if srgb {
        "bc3RgbaUnormSrgb"
    } else {
        "bc3RgbaUnorm"
    }
    .into();
    m.color_space = if srgb { "srgb" } else { "linear" }.into();
    m.byte_length = texture_mip_byte_len(&m.format, m.width, m.height, m.mip_count)
        .ok_or("texture.bc3_layout")?;
    let mut blocks = Vec::with_capacity(m.byte_length);
    for mip in rgba8_mip_layout(m.width, m.height, m.mip_count).ok_or("texture.bc3_layout")? {
        let start = blocks.len();
        blocks.resize(
            start + texpresso::Format::Bc3.compressed_size(mip.width as usize, mip.height as usize),
            0,
        );
        texpresso::Format::Bc3.compress(
            &chain[mip.offset..mip.offset + mip.byte_len],
            mip.width as usize,
            mip.height as usize,
            texpresso::Params {
                algorithm: texpresso::Algorithm::ClusterFit,
                // Data channels have equal value; alpha remains independent.
                weights: texpresso::COLOUR_WEIGHTS_UNIFORM,
                weigh_colour_by_alpha: false,
            },
            &mut blocks[start..],
        );
    }
    source.rgba8 = blocks.into();
    m.pixel_data_path = format!("cooked/textures/{}.bc3", m.asset_id);
    Ok(source)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[128, 64, 220, 128].repeat((width * height) as usize))
                .unwrap();
        }
        bytes
    }
    #[test]
    fn bc3_png_source_cooks_full_chain_and_rejects_unsupported_inputs() {
        let pixels = png(8, 4);
        let settings = json!({"gpuCompression":"bc3","colorSpace":"srgb"});
        let first = super::super::cached_texture(
            "image",
            "image.png",
            &pixels,
            Some(&settings),
            None,
            &mut vec![],
        )
        .unwrap();
        let second = super::super::cached_texture(
            "image",
            "image.png",
            &pixels,
            Some(&settings),
            None,
            &mut vec![],
        )
        .unwrap();
        assert_eq!(first.rgba8, second.rgba8);
        assert_eq!(first.metadata.format, "bc3RgbaUnormSrgb");
        assert_eq!(first.metadata.mip_count, 4);
        assert_eq!(first.rgba8.len(), 80);
        let plain = super::super::cook_texture("image", "image.png", &pixels, None).unwrap();
        assert_eq!(plain.metadata.mip_count, 1);
        assert_eq!(plain.rgba8.len(), 128);
        for invalid in [
            json!({"gpuCompression":"bc7"}),
            json!({"gpuCompression":true}),
        ] {
            let error = super::super::cached_texture(
                "image",
                "image.png",
                &pixels,
                Some(&invalid),
                None,
                &mut vec![],
            )
            .unwrap_err();
            assert!(error.code().ends_with("texture_compression_unsupported"));
        }
        assert!(
            super::super::cook_texture("image", "image.png", &png(3, 4), Some(&settings)).is_none()
        );
        assert!(super::super::cook_texture(
            "image",
            "image.png",
            &pixels,
            Some(&json!({"gpuCompression":"bc3","colorSpace":"hdr"}))
        )
        .is_none());
    }
}
