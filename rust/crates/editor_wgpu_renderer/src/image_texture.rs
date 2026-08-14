use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[cfg(feature = "real-wgpu")]
use sha2::{Digest, Sha256};

pub const EDITOR_IMAGE_TEXTURE_MAX_ITEMS: usize = 128;
pub const EDITOR_IMAGE_TEXTURE_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorImageTextureUploadStatus {
    Uploaded,
    Replaced,
    Reused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorImageTextureSummary {
    pub texture_id: String,
    pub width: u32,
    pub height: u32,
    pub byte_len: usize,
    pub content_hash: String,
    pub generation: u64,
    pub last_used_tick: u64,
    pub upload_status: EditorImageTextureUploadStatus,
}

struct EditorImageTextureEntry {
    summary: EditorImageTextureSummary,
    #[cfg(feature = "real-wgpu")]
    texture: Option<wgpu::Texture>,
    #[cfg(feature = "real-wgpu")]
    view: Option<wgpu::TextureView>,
    #[cfg(feature = "real-wgpu")]
    sampler: Option<wgpu::Sampler>,
}

impl EditorImageTextureEntry {
    fn mock(summary: EditorImageTextureSummary) -> Self {
        Self {
            summary,
            #[cfg(feature = "real-wgpu")]
            texture: None,
            #[cfg(feature = "real-wgpu")]
            view: None,
            #[cfg(feature = "real-wgpu")]
            sampler: None,
        }
    }
}

#[derive(Default)]
pub struct EditorImageTextureRegistry {
    entries: HashMap<String, EditorImageTextureEntry>,
    byte_len: usize,
    tick: u64,
    upload_count: u64,
    eviction_count: u64,
}

impl EditorImageTextureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn texture_count(&self) -> usize {
        self.entries.len()
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn upload_count(&self) -> u64 {
        self.upload_count
    }

    pub fn eviction_count(&self) -> u64 {
        self.eviction_count
    }

    pub fn contains(&self, texture_id: &str) -> bool {
        self.entries.contains_key(texture_id)
    }

    pub fn resolve_summary(&self, texture_id: &str) -> Option<&EditorImageTextureSummary> {
        self.entries.get(texture_id).map(|entry| &entry.summary)
    }

    pub fn touch(&mut self, texture_id: &str) -> bool {
        let Some(entry) = self.entries.get_mut(texture_id) else {
            return false;
        };
        self.tick = self.tick.saturating_add(1);
        entry.summary.last_used_tick = self.tick;
        true
    }

    pub fn upload_mock(
        &mut self,
        texture_id: impl Into<String>,
        width: u32,
        height: u32,
        content_hash: impl Into<String>,
        rgba8: &[u8],
    ) -> Result<EditorImageTextureSummary, String> {
        let texture_id = texture_id.into();
        let content_hash = content_hash.into();
        let (summary, should_upload) =
            self.prepare_upload(texture_id.clone(), width, height, content_hash, rgba8)?;
        if should_upload {
            self.entries
                .insert(texture_id, EditorImageTextureEntry::mock(summary.clone()));
        }
        Ok(summary)
    }

    #[cfg(feature = "real-wgpu")]
    pub fn upload_gpu(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_id: impl Into<String>,
        width: u32,
        height: u32,
        content_hash: impl Into<String>,
        rgba8: &[u8],
    ) -> Result<EditorImageTextureSummary, String> {
        let texture_id = texture_id.into();
        let content_hash = content_hash.into();
        let (summary, should_upload) =
            self.prepare_upload(texture_id.clone(), width, height, content_hash, rgba8)?;
        if !should_upload {
            return Ok(summary);
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("editor-image-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba8,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("editor-image-texture-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let mut entry = EditorImageTextureEntry::mock(summary.clone());
        entry.texture = Some(texture);
        entry.view = Some(view);
        entry.sampler = Some(sampler);
        self.entries.insert(texture_id, entry);
        Ok(summary)
    }

    #[cfg(feature = "real-wgpu")]
    pub fn upload_builtin_control_textures_gpu(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<String> {
        let textures = match editor_ui_renderer::dark_neutral_control_textures() {
            Ok(textures) => textures,
            Err(diagnostics) => {
                return diagnostics
                    .into_iter()
                    .map(|item| format!("{}:{}", item.code, item.message))
                    .collect();
            }
        };
        let mut diagnostics = Vec::new();
        for texture in textures {
            let digest = format!("{:x}", Sha256::digest(texture.png_bytes));
            if digest != texture.sha256 {
                diagnostics.push(format!(
                    "editor_theme_texture.digest_mismatch:{}",
                    texture.texture_id
                ));
                continue;
            }
            let decoded = decode_png_rgba8(texture.png_bytes).and_then(|(width, height, rgba8)| {
                if width != texture.width || height != texture.height {
                    return Err(format!(
                        "editor_theme_texture.dimension_mismatch:{}",
                        texture.texture_id
                    ));
                }
                self.upload_gpu(
                    device,
                    queue,
                    texture.texture_id.clone(),
                    width,
                    height,
                    texture.sha256.clone(),
                    &rgba8,
                )
                .map(|_| ())
            });
            if let Err(error) = decoded {
                diagnostics.push(error);
            }
        }
        diagnostics
    }

    #[cfg(feature = "real-wgpu")]
    pub fn resolve_gpu(&self, texture_id: &str) -> Option<ResolvedImageTexture<'_>> {
        let entry = self.entries.get(texture_id)?;
        Some(ResolvedImageTexture {
            summary: &entry.summary,
            view: entry.view.as_ref()?,
            sampler: entry.sampler.as_ref()?,
        })
    }

    fn prepare_upload(
        &mut self,
        texture_id: String,
        width: u32,
        height: u32,
        content_hash: String,
        rgba8: &[u8],
    ) -> Result<(EditorImageTextureSummary, bool), String> {
        if width == 0 || height == 0 {
            return Err("image_texture.zero_sized_payload".to_string());
        }
        let expected = width as usize * height as usize * 4;
        if rgba8.len() != expected {
            return Err(format!(
                "image_texture.rgba_size_mismatch:expected={expected}:actual={}",
                rgba8.len()
            ));
        }
        if expected > EDITOR_IMAGE_TEXTURE_MAX_BYTES {
            return Err("image_texture.payload_exceeds_budget".to_string());
        }
        self.tick = self.tick.saturating_add(1);
        if let Some(existing) = self.entries.get_mut(&texture_id) {
            if existing.summary.width == width
                && existing.summary.height == height
                && existing.summary.content_hash == content_hash
            {
                existing.summary.last_used_tick = self.tick;
                existing.summary.upload_status = EditorImageTextureUploadStatus::Reused;
                return Ok((existing.summary.clone(), false));
            }
        }
        let generation = self
            .entries
            .get(&texture_id)
            .map_or(1, |entry| entry.summary.generation.saturating_add(1));
        let status = if self.remove(&texture_id).is_some() {
            EditorImageTextureUploadStatus::Replaced
        } else {
            EditorImageTextureUploadStatus::Uploaded
        };
        self.evict_to_fit(expected);
        if self.entries.len() >= EDITOR_IMAGE_TEXTURE_MAX_ITEMS
            || self.byte_len.saturating_add(expected) > EDITOR_IMAGE_TEXTURE_MAX_BYTES
        {
            return Err("image_texture.registry_budget_exhausted".to_string());
        }
        self.byte_len = self.byte_len.saturating_add(expected);
        self.upload_count = self.upload_count.saturating_add(1);
        Ok((
            EditorImageTextureSummary {
                texture_id,
                width,
                height,
                byte_len: expected,
                content_hash,
                generation,
                last_used_tick: self.tick,
                upload_status: status,
            },
            true,
        ))
    }

    fn evict_to_fit(&mut self, incoming_bytes: usize) {
        while self.entries.len() >= EDITOR_IMAGE_TEXTURE_MAX_ITEMS
            || self.byte_len.saturating_add(incoming_bytes) > EDITOR_IMAGE_TEXTURE_MAX_BYTES
        {
            let candidate = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.summary.last_used_tick)
                .map(|(id, _)| id.clone());
            let Some(candidate) = candidate else {
                break;
            };
            self.remove(&candidate);
            self.eviction_count = self.eviction_count.saturating_add(1);
        }
    }

    fn remove(&mut self, texture_id: &str) -> Option<EditorImageTextureEntry> {
        let entry = self.entries.remove(texture_id)?;
        self.byte_len = self.byte_len.saturating_sub(entry.summary.byte_len);
        Some(entry)
    }
}

#[cfg(feature = "real-wgpu")]
fn decode_png_rgba8(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("editor_theme_texture.decode_failed:{error}"))?;
    let mut payload = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut payload)
        .map_err(|error| format!("editor_theme_texture.decode_failed:{error}"))?;
    payload.truncate(info.buffer_size());
    let rgba8 = match info.color_type {
        png::ColorType::Rgba => payload,
        png::ColorType::Rgb => payload
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        _ => return Err("editor_theme_texture.unsupported_color_type".to_string()),
    };
    Ok((info.width, info.height, rgba8))
}

#[cfg(feature = "real-wgpu")]
pub struct ResolvedImageTexture<'a> {
    pub summary: &'a EditorImageTextureSummary,
    pub view: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
}

#[cfg(all(test, feature = "real-wgpu"))]
mod tests {
    use super::*;

    #[test]
    fn builtin_control_image_texture_payloads_decode_and_match_manifest() {
        let textures = editor_ui_renderer::dark_neutral_control_textures()
            .expect("valid built-in texture manifest");
        for texture in textures {
            let digest = format!("{:x}", Sha256::digest(texture.png_bytes));
            assert_eq!(digest, texture.sha256);
            let (width, height, rgba8) = decode_png_rgba8(texture.png_bytes).expect("decode png");
            assert_eq!((width, height), (texture.width, texture.height));
            assert_eq!(rgba8.len(), width as usize * height as usize * 4);
        }
    }
}
