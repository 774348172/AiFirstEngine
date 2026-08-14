use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorViewportTextureSummary {
    pub texture_id: String,
    pub target_id: String,
    pub owner_session_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub generation: u64,
    pub last_frame_index: Option<u64>,
    pub last_frame_hash: Option<String>,
    pub publication_index: u64,
    pub last_submit_serial: Option<u64>,
    pub producer: String,
    pub present_status: EditorViewportTexturePresentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeContentIdentity {
    pub session_id: String,
    pub frame_index: u64,
    pub frame_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewPublicationIdentity {
    pub surface_id: String,
    pub surface_generation: u64,
    pub publication_index: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameViewPublicationStatus {
    Published,
    Reused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameViewPublicationReceipt {
    pub content: RuntimeContentIdentity,
    pub publication: GameViewPublicationIdentity,
    pub submit_serial: u64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub status: GameViewPublicationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorViewportTexturePresentStatus {
    Allocated,
    Resized,
    Rendered,
    Released,
}

pub struct EditorViewportTextureEntry {
    pub summary: EditorViewportTextureSummary,
    #[cfg(feature = "real-wgpu")]
    texture: Option<wgpu::Texture>,
    #[cfg(feature = "real-wgpu")]
    gpu_format: Option<wgpu::TextureFormat>,
    #[cfg(feature = "real-wgpu")]
    view: Option<wgpu::TextureView>,
    #[cfg(feature = "real-wgpu")]
    sampler: Option<wgpu::Sampler>,
}

#[cfg(feature = "real-wgpu")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorViewportTextureReadback {
    pub texture_id: String,
    pub target_id: String,
    pub owner_session_id: String,
    pub frame_index: u64,
    pub generation: u64,
    pub publication_index: u64,
    pub frame_hash: String,
    pub submit_serial: u64,
    pub width: u32,
    pub height: u32,
    pub source_format: String,
    pub rgba8: Vec<u8>,
}

impl EditorViewportTextureEntry {
    fn mock(summary: EditorViewportTextureSummary) -> Self {
        Self {
            summary,
            #[cfg(feature = "real-wgpu")]
            texture: None,
            #[cfg(feature = "real-wgpu")]
            gpu_format: None,
            #[cfg(feature = "real-wgpu")]
            view: None,
            #[cfg(feature = "real-wgpu")]
            sampler: None,
        }
    }
}

#[derive(Default)]
pub struct EditorViewportTextureRegistry {
    entries: HashMap<String, EditorViewportTextureEntry>,
    lifecycle_event_count: u64,
    submit_serial: u64,
}

impl EditorViewportTextureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lifecycle_event_count(&self) -> u64 {
        self.lifecycle_event_count
    }

    pub fn texture_count(&self) -> usize {
        self.entries.len()
    }

    pub fn allocate_or_resize_mock(
        &mut self,
        owner_session_id: impl Into<String>,
        target_id: impl Into<String>,
        texture_id: impl Into<String>,
        width: u32,
        height: u32,
        format: impl Into<String>,
        producer: impl Into<String>,
    ) -> EditorViewportTextureSummary {
        let owner_session_id = owner_session_id.into();
        let target_id = target_id.into();
        let texture_id = texture_id.into();
        let format = format.into();
        let producer = producer.into();
        let mut generation = 1;
        let mut status = EditorViewportTexturePresentStatus::Allocated;
        if let Some(existing) = self.entries.get(&texture_id) {
            generation = existing.summary.generation;
            if existing.summary.owner_session_id == owner_session_id
                && existing.summary.target_id == target_id
                && existing.summary.width == width
                && existing.summary.height == height
                && existing.summary.format == format
            {
                return existing.summary.clone();
            }
            if existing.summary.width != width
                || existing.summary.height != height
                || existing.summary.format != format
                || existing.summary.owner_session_id != owner_session_id
                || existing.summary.target_id != target_id
            {
                generation += 1;
                status = EditorViewportTexturePresentStatus::Resized;
            }
        }

        let summary = EditorViewportTextureSummary {
            texture_id: texture_id.clone(),
            target_id,
            owner_session_id,
            width,
            height,
            format,
            color_space: "srgb".to_string(),
            generation,
            last_frame_index: None,
            last_frame_hash: None,
            publication_index: 0,
            last_submit_serial: None,
            producer,
            present_status: status,
        };
        self.entries.insert(
            texture_id,
            EditorViewportTextureEntry::mock(summary.clone()),
        );
        self.lifecycle_event_count += 1;
        summary
    }

    #[cfg(feature = "real-wgpu")]
    pub fn allocate_or_resize_gpu(
        &mut self,
        device: &wgpu::Device,
        owner_session_id: impl Into<String>,
        target_id: impl Into<String>,
        texture_id: impl Into<String>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        producer: impl Into<String>,
    ) -> Result<EditorViewportTextureSummary, String> {
        if width == 0 || height == 0 {
            return Err("viewport_texture.zero_sized_target".to_string());
        }
        let owner_session_id = owner_session_id.into();
        let target_id = target_id.into();
        let texture_id = texture_id.into();
        let format_name = format!("{format:?}");
        if let Some(existing) = self.entries.get(&texture_id) {
            if existing.summary.owner_session_id == owner_session_id
                && existing.summary.target_id == target_id
                && existing.summary.width == width
                && existing.summary.height == height
                && existing.summary.format == format_name
                && existing.texture.is_some()
                && existing.view.is_some()
                && existing.sampler.is_some()
            {
                return Ok(existing.summary.clone());
            }
        }
        let previous = self.entries.get(&texture_id).map(|entry| &entry.summary);
        let generation = previous.map_or(1, |summary| summary.generation.saturating_add(1));
        let status = if previous.is_some() {
            EditorViewportTexturePresentStatus::Resized
        } else {
            EditorViewportTexturePresentStatus::Allocated
        };
        let summary = EditorViewportTextureSummary {
            texture_id: texture_id.clone(),
            target_id,
            owner_session_id,
            width,
            height,
            format: format_name,
            color_space: "srgb".to_string(),
            generation,
            last_frame_index: None,
            last_frame_hash: None,
            publication_index: 0,
            last_submit_serial: None,
            producer: producer.into(),
            present_status: status,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("editor-gameview-shared-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("editor-gameview-shared-texture-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        self.entries.insert(
            texture_id,
            EditorViewportTextureEntry {
                summary: summary.clone(),
                texture: Some(texture),
                gpu_format: Some(format),
                view: Some(view),
                sampler: Some(sampler),
            },
        );
        self.lifecycle_event_count += 1;
        Ok(summary)
    }

    pub fn mark_published(
        &mut self,
        texture_id: &str,
        frame_index: u64,
        frame_hash: impl Into<String>,
    ) -> Option<GameViewPublicationReceipt> {
        let entry = self.entries.get_mut(texture_id)?;
        if entry
            .summary
            .last_frame_index
            .is_some_and(|last| frame_index < last)
        {
            return None;
        }
        self.submit_serial = self.submit_serial.saturating_add(1);
        entry.summary.publication_index = entry.summary.publication_index.saturating_add(1);
        entry.summary.last_frame_index = Some(frame_index);
        let frame_hash = frame_hash.into();
        entry.summary.last_frame_hash = Some(frame_hash.clone());
        entry.summary.last_submit_serial = Some(self.submit_serial);
        entry.summary.present_status = EditorViewportTexturePresentStatus::Rendered;
        Some(GameViewPublicationReceipt {
            content: RuntimeContentIdentity {
                session_id: entry.summary.owner_session_id.clone(),
                frame_index,
                frame_hash,
            },
            publication: GameViewPublicationIdentity {
                surface_id: entry.summary.texture_id.clone(),
                surface_generation: entry.summary.generation,
                publication_index: entry.summary.publication_index,
            },
            submit_serial: self.submit_serial,
            width: entry.summary.width,
            height: entry.summary.height,
            format: entry.summary.format.clone(),
            status: GameViewPublicationStatus::Published,
        })
    }

    pub fn last_receipt(&self, texture_id: &str) -> Option<GameViewPublicationReceipt> {
        let entry = self.entries.get(texture_id)?;
        Some(GameViewPublicationReceipt {
            content: RuntimeContentIdentity {
                session_id: entry.summary.owner_session_id.clone(),
                frame_index: entry.summary.last_frame_index?,
                frame_hash: entry.summary.last_frame_hash.clone()?,
            },
            publication: GameViewPublicationIdentity {
                surface_id: entry.summary.texture_id.clone(),
                surface_generation: entry.summary.generation,
                publication_index: entry.summary.publication_index,
            },
            submit_serial: entry.summary.last_submit_serial?,
            width: entry.summary.width,
            height: entry.summary.height,
            format: entry.summary.format.clone(),
            status: GameViewPublicationStatus::Reused,
        })
    }

    pub fn mark_rendered(
        &mut self,
        texture_id: &str,
        frame_index: u64,
    ) -> Option<EditorViewportTextureSummary> {
        self.mark_published(
            texture_id,
            frame_index,
            format!("legacy-frame-{frame_index}"),
        )?;
        self.resolve_summary(texture_id).cloned()
    }

    pub fn resolve_summary(&self, texture_id: &str) -> Option<&EditorViewportTextureSummary> {
        self.entries.get(texture_id).map(|entry| &entry.summary)
    }

    pub fn contains(&self, texture_id: &str) -> bool {
        self.entries.contains_key(texture_id)
    }

    pub fn fallback_reason(&self, texture_id: Option<&str>) -> Option<String> {
        let texture_id = texture_id?;
        if self.contains(texture_id) {
            None
        } else {
            Some(format!("viewport_texture_missing:{texture_id}"))
        }
    }

    pub fn unregister_session(&mut self, owner_session_id: &str) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| entry.summary.owner_session_id != owner_session_id);
        let removed = before - self.entries.len();
        if removed > 0 {
            self.lifecycle_event_count += removed as u64;
        }
        removed
    }

    #[cfg(feature = "real-wgpu")]
    pub fn resolve_gpu(&self, texture_id: &str) -> Option<ResolvedViewportTexture<'_>> {
        let entry = self.entries.get(texture_id)?;
        Some(ResolvedViewportTexture {
            summary: &entry.summary,
            view: entry.view.as_ref()?,
            sampler: entry.sampler.as_ref()?,
        })
    }

    #[cfg(feature = "real-wgpu")]
    pub fn readback_gpu(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_id: &str,
    ) -> Result<EditorViewportTextureReadback, String> {
        let entry = self
            .entries
            .get(texture_id)
            .ok_or_else(|| format!("viewport_texture_readback.texture_missing:{texture_id}"))?;
        if entry.summary.present_status != EditorViewportTexturePresentStatus::Rendered {
            return Err(format!(
                "viewport_texture_readback.texture_not_rendered:{texture_id}"
            ));
        }
        let frame_index = entry
            .summary
            .last_frame_index
            .ok_or_else(|| format!("viewport_texture_readback.frame_index_missing:{texture_id}"))?;
        let frame_hash =
            entry.summary.last_frame_hash.clone().ok_or_else(|| {
                format!("viewport_texture_readback.frame_hash_missing:{texture_id}")
            })?;
        let submit_serial = entry.summary.last_submit_serial.ok_or_else(|| {
            format!("viewport_texture_readback.submit_serial_missing:{texture_id}")
        })?;
        let texture = entry
            .texture
            .as_ref()
            .ok_or_else(|| format!("viewport_texture_readback.gpu_texture_missing:{texture_id}"))?;
        let format = entry
            .gpu_format
            .ok_or_else(|| format!("viewport_texture_readback.gpu_format_missing:{texture_id}"))?;
        if !matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            return Err(format!(
                "viewport_texture_readback.unsupported_format:{format:?}"
            ));
        }
        let width = entry.summary.width;
        let height = entry.summary.height;
        if width == 0 || height == 0 {
            return Err("viewport_texture_readback.zero_sized_target".to_string());
        }
        let unpadded_bytes_per_row = width
            .checked_mul(4)
            .ok_or_else(|| "viewport_texture_readback.size_overflow".to_string())?;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row
            .checked_add(align - 1)
            .map(|bytes| bytes / align * align)
            .ok_or_else(|| "viewport_texture_readback.size_overflow".to_string())?;
        let output_buffer_size = u64::from(padded_bytes_per_row)
            .checked_mul(u64::from(height))
            .ok_or_else(|| "viewport_texture_readback.size_overflow".to_string())?;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("editor-gameview-shared-texture-readback"),
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("editor-gameview-shared-texture-readback-copy"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));

        let buffer_slice = output_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let map_result = loop {
            device
                .poll(wgpu::PollType::Poll)
                .map_err(|error| format!("viewport_texture_readback.poll_failed:{error}"))?;
            match receiver.try_recv() {
                Ok(result) => break result,
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if std::time::Instant::now() >= deadline {
                        return Err("viewport_texture_readback.map_timeout".to_string());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err("viewport_texture_readback.map_channel_closed".to_string());
                }
            }
        };
        map_result.map_err(|error| format!("viewport_texture_readback.map_failed:{error}"))?;

        let mapped = buffer_slice.get_mapped_range();
        let row_len = usize::try_from(unpadded_bytes_per_row)
            .map_err(|_| "viewport_texture_readback.size_overflow".to_string())?;
        let padded_row_len = usize::try_from(padded_bytes_per_row)
            .map_err(|_| "viewport_texture_readback.size_overflow".to_string())?;
        let rgba_len = row_len
            .checked_mul(
                usize::try_from(height)
                    .map_err(|_| "viewport_texture_readback.size_overflow".to_string())?,
            )
            .ok_or_else(|| "viewport_texture_readback.size_overflow".to_string())?;
        let mut rgba8 = Vec::with_capacity(rgba_len);
        for row in 0..height as usize {
            let start = row
                .checked_mul(padded_row_len)
                .ok_or_else(|| "viewport_texture_readback.size_overflow".to_string())?;
            let end = start
                .checked_add(row_len)
                .ok_or_else(|| "viewport_texture_readback.size_overflow".to_string())?;
            let bytes = mapped
                .get(start..end)
                .ok_or_else(|| "viewport_texture_readback.buffer_length_invalid".to_string())?;
            rgba8.extend_from_slice(bytes);
        }
        drop(mapped);
        output_buffer.unmap();
        if matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in rgba8.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }

        Ok(EditorViewportTextureReadback {
            texture_id: entry.summary.texture_id.clone(),
            target_id: entry.summary.target_id.clone(),
            owner_session_id: entry.summary.owner_session_id.clone(),
            frame_index,
            generation: entry.summary.generation,
            publication_index: entry.summary.publication_index,
            frame_hash,
            submit_serial,
            width,
            height,
            source_format: format!("{format:?}"),
            rgba8,
        })
    }

    #[cfg(feature = "real-wgpu")]
    pub fn readback_gpu_exact(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        receipt: &GameViewPublicationReceipt,
    ) -> Result<EditorViewportTextureReadback, String> {
        let current = self
            .last_receipt(&receipt.publication.surface_id)
            .ok_or_else(|| "publication.capture_receipt_unavailable".to_string())?;
        if current.content != receipt.content
            || current.publication != receipt.publication
            || current.submit_serial != receipt.submit_serial
        {
            return Err("publication.capture_receipt_stale".to_string());
        }
        self.readback_gpu(device, queue, &receipt.publication.surface_id)
    }
}

#[cfg(feature = "real-wgpu")]
pub struct ResolvedViewportTexture<'a> {
    pub summary: &'a EditorViewportTextureSummary,
    pub view: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
}

#[cfg(all(test, feature = "real-wgpu"))]
mod real_wgpu_tests {
    use super::*;

    #[test]
    fn viewport_texture_readback_reads_exact_registered_texture() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
        });
        let adapter = match pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            },
        )) {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!(
                    "viewport_texture_readback_local_environment_unavailable:request_adapter:{error}"
                );
                return;
            }
        };
        let (device, queue) =
            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("viewport-texture-readback-test-device"),
                required_features: wgpu::Features::empty(),
                required_limits:
                    wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })) {
                Ok(pair) => pair,
                Err(error) => {
                    eprintln!(
                    "viewport_texture_readback_local_environment_unavailable:request_device:{error}"
                );
                    return;
                }
            };

        let mut registry = EditorViewportTextureRegistry::new();
        registry
            .allocate_or_resize_gpu(
                &device,
                "game-view-session-1",
                "viewport-main",
                "viewport-main::frame-7",
                3,
                2,
                wgpu::TextureFormat::Bgra8UnormSrgb,
                "editor-gameview",
            )
            .expect("allocate shared texture");
        assert_eq!(
            registry
                .readback_gpu(&device, &queue, "viewport-main::frame-7")
                .expect_err("unrendered texture must not be evidence"),
            "viewport_texture_readback.texture_not_rendered:viewport-main::frame-7"
        );
        let texture = registry
            .entries
            .get("viewport-main::frame-7")
            .and_then(|entry| entry.texture.as_ref())
            .expect("registered GPU texture");
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viewport-texture-readback-test-render"),
        });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport-texture-readback-test-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        queue.submit(Some(encoder.finish()));
        let receipt = registry
            .mark_published("viewport-main::frame-7", 7, "frame-hash-7")
            .expect("publication receipt");

        let readback = registry
            .readback_gpu_exact(&device, &queue, &receipt)
            .expect("read exact shared texture");
        assert_eq!(readback.frame_index, 7);
        assert_eq!(readback.frame_hash, "frame-hash-7");
        assert_eq!(readback.publication_index, 1);
        assert_eq!(readback.submit_serial, receipt.submit_serial);
        assert_eq!(readback.width, 3);
        assert_eq!(readback.height, 2);
        assert_eq!(readback.source_format, "Bgra8UnormSrgb");
        assert_eq!(readback.rgba8, [255, 0, 0, 255].repeat(6));

        registry
            .mark_published("viewport-main::frame-7", 8, "frame-hash-8")
            .expect("newer publication");
        assert_eq!(
            registry
                .readback_gpu_exact(&device, &queue, &receipt)
                .expect_err("stale publication receipt must fail"),
            "publication.capture_receipt_stale"
        );
    }
}
