//! Backend-owned offscreen outputs and copy readbacks use the original census,
//! admission and real GPU completion ledger. Borrowed OS surfaces stay external.
use super::*;

impl RealWgpuBackend {
    pub(super) fn pending_output_retirement(&self) -> (u64, usize) {
        let textures: u64 = self
            .output_textures
            .iter()
            .map(|t| {
                u64::from(t.width())
                    * u64::from(t.height())
                    * u64::from(t.format().block_copy_size(None).unwrap())
            })
            .sum();
        (
            textures
                + self
                    .readback_buffers
                    .iter()
                    .map(wgpu::Buffer::size)
                    .sum::<u64>(),
            self.output_textures.len() + self.readback_buffers.len(),
        )
    }
    pub(super) fn create_output_texture(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<wgpu::Texture, String> {
        self.poll_gpu_completion()?;
        let maximum = self.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > maximum || height > maximum {
            return Err("wgpu.output_extent_invalid".into());
        }
        let block = self
            .format
            .block_copy_size(None)
            .ok_or("wgpu.output_format_unsupported")?;
        let bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|n| n.checked_mul(u64::from(block)))
            .ok_or("wgpu.output_extent_overflow")?;
        self.admit_geometry_backing(bytes)?;
        self.admit_gpu_retirement(bytes, 1)?;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("runtime-owned-output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.output_textures.insert(texture.clone());
        Ok(texture)
    }

    pub(super) fn release_output_texture(&mut self, texture: wgpu::Texture) {
        self.output_textures.remove(&texture);
        let bytes = u64::from(texture.width())
            * u64::from(texture.height())
            * u64::from(texture.format().block_copy_size(None).unwrap());
        if !self.gpu_lifecycle_stats().abandoned {
            self.retire_gpu(texture, bytes);
        }
    }

    pub(super) fn read_texture_rgba_bytes(
        &mut self,
        texture: &wgpu::Texture,
    ) -> Result<Vec<u8>, String> {
        if !matches!(
            texture.format(),
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            return Err("wgpu.readback_format_unsupported".into());
        }
        let width = texture.width();
        let height = texture.height();
        let row = width
            .checked_mul(4)
            .ok_or("wgpu.readback_extent_overflow")?;
        let padded = row
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            .ok_or("wgpu.readback_extent_overflow")?
            / wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes = u64::from(padded) * u64::from(height);
        self.admit_gpu_frame()?;
        self.admit_geometry_backing(bytes)?;
        self.admit_gpu_retirement(bytes, 1)?;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("runtime-owned-readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.readback_buffers.insert(buffer.clone());
        let result = (|| {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            // The copy is a real maintenance submission, not an untracked queue.submit.
            let measurement = self.measurement.take();
            self.submit_resource_maintenance(encoder);
            self.measurement = measurement;
            let slice = buffer.slice(..);
            let (sender, receiver) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
            let deadline = crate::platform_time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                self.poll_gpu_completion()?;
                match receiver.try_recv() {
                    Ok(result) => {
                        result.map_err(|error| format!("wgpu.readback_map_failed:{error}"))?;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty)
                        if crate::platform_time::Instant::now() < deadline =>
                    {
                        std::thread::yield_now()
                    }
                    _ => return Err("wgpu.readback_timeout".into()),
                }
            }
            let mapped = slice.get_mapped_range();
            let capacity = usize::try_from(u64::from(row) * u64::from(height))
                .map_err(|_| "wgpu.readback_extent_overflow")?;
            let mut rgba = Vec::with_capacity(capacity);
            for y in 0..height as usize {
                let start = y * padded as usize;
                rgba.extend_from_slice(&mapped[start..start + row as usize]);
            }
            if matches!(
                texture.format(),
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ) {
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }
            Ok(rgba)
        })();
        // Cancels a pending map on failure; backing remains retained to actual copy completion.
        buffer.unmap();
        self.readback_buffers.remove(&buffer);
        if !self.gpu_lifecycle_stats().abandoned {
            self.retire_gpu(buffer, bytes);
        }
        result
    }
}
