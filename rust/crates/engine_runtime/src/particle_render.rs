//! Persistent renderer-side particle buffers. Simulation and rendering share records.
use crate::particle_effect::*;
use crate::particle_render_contract::ParticleRenderView;
use wgpu::util::DeviceExt;

pub struct ParticleMesh {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) vertex_count: u32,
}
impl ParticleMesh {
    /// Upload indexed mesh data once; simulation never duplicates mesh data per particle.
    pub fn new(
        device: &wgpu::Device,
        positions: &[[f32; 3]],
        uvs: &[[f32; 2]],
        indices: &[u32],
    ) -> Result<Self, String> {
        if positions.len() != uvs.len()
            || indices.is_empty()
            || indices.len() % 3 != 0
            || indices.len() > 1_000_000
            || !positions
                .iter()
                .flatten()
                .chain(uvs.iter().flatten())
                .all(|v| v.is_finite())
            || indices.iter().any(|i| *i as usize >= positions.len())
        {
            return Err("particle_render.invalid_mesh".into());
        }
        let data: Vec<[f32; 8]> = indices
            .iter()
            .map(|i| {
                let p = positions[*i as usize];
                let uv = uvs[*i as usize];
                [p[0], p[1], p[2], 0.0, uv[0], uv[1], 0.0, 0.0]
            })
            .collect();
        Ok(Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("particle-mesh"),
                contents: bytemuck::cast_slice(&data),
                usage: wgpu::BufferUsages::STORAGE,
            }),
            vertex_count: indices.len() as u32,
        })
    }
}

pub(crate) struct ParticleEmitterRender {
    pub pipeline: wgpu::RenderPipeline,
    pub indirect: wgpu::Buffer,
    history: wgpu::Buffer,
    uniform: wgpu::Buffer,
    compute: Vec<wgpu::ComputePipeline>,
    compute_binding: wgpu::BindGroup,
    draw_layout: wgpu::BindGroupLayout,
    records: wgpu::Buffer,
    scratch: wgpu::Buffer,
    dummy_mesh: wgpu::Buffer,
    dummy_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    desc: ParticleEmitter,
    sort_size: u32,
    pub bytes: u64,
    pub(crate) material_tint: [f32; 4],
    pub(crate) material_resolved: bool,
}
fn storage(
    device: &wgpu::Device,
    label: &str,
    bytes: u64,
    extra: wgpu::BufferUsages,
) -> Result<wgpu::Buffer, String> {
    if bytes > u64::from(device.limits().max_storage_buffer_binding_size)
        || bytes > device.limits().max_buffer_size
    {
        return Err(format!("particle_render.buffer_limit:{label}:{bytes}"));
    }
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
        mapped_at_creation: false,
    }))
}
impl ParticleEmitterRender {
    pub fn validate_assets(&self, textured: bool, mesh: bool) -> Result<(), String> {
        if self.desc.draw.material.is_some() && !self.material_resolved {
            return Err("particle_render.material_binding_unresolved".into());
        }
        if matches!(self.desc.draw.geometry, ParticleGeometry::Mesh { .. }) && !mesh {
            return Err("particle_render.mesh_missing".into());
        }
        if self.desc.draw.texture.is_some() && !textured {
            return Err("particle_render.texture_missing".into());
        }
        Ok(())
    }
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        desc: &ParticleEmitter,
        records: &wgpu::Buffer,
        scratch: &wgpu::Buffer,
    ) -> Result<Self, String> {
        let sort_size = desc.capacity.next_power_of_two();
        if sort_size.div_ceil(64) > device.limits().max_compute_workgroups_per_dimension {
            return Err("particle_render.dispatch_limit".into());
        }
        let segments = desc.draw.trail.as_ref().map_or(0, |t| t.segments);
        let history_bytes = if segments == 0 {
            16
        } else {
            u64::from(desc.capacity) * (4 + u64::from(segments) * 4) * 4
        };
        let indirect_bytes = 16 + u64::from(sort_size) * 8;
        let indirect = storage(
            device,
            "particle-draw-indirect-sort",
            indirect_bytes,
            wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_SRC,
        )?;
        let history = storage(
            device,
            "particle-trail-history",
            history_bytes,
            wgpu::BufferUsages::empty(),
        )?;
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle-view"),
            size: 192,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (_, particle_bytes, stride) = program::particle_layout(desc);
        let geometry = match desc.draw.geometry {
            ParticleGeometry::Billboard2d {} => 0,
            ParticleGeometry::Billboard3d {} => 1,
            ParticleGeometry::Mesh { .. } => 2,
        };
        let prefix=format!("const CAPACITY:u32={}u;const STRIDE:u32={}u;const ALIVE:u32={}u;const SERIAL:u32={}u;const SORT_SIZE:u32={}u;const TRAIL_SEGMENTS:u32={}u;const LOCAL_SPACE:bool={};const GEOMETRY:u32={}u;const FLIPBOOK:bool={};const FLIP_FPS:f32={:?};\n",desc.capacity,stride/4,particle_bytes/4+6,particle_bytes/4+8,sort_size,segments,desc.space==ParticleSpace::Local,geometry,desc.draw.flipbook.is_some(),desc.draw.flipbook.as_ref().map_or(0.0,|f|f.fps));
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let common = prefix + include_str!("particle_render/common.wgsl");
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle-render-compute"),
            source: wgpu::ShaderSource::Wgsl(
                (common.clone() + include_str!("particle_render/compute.wgsl")).into(),
            ),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle-render-draw"),
            source: wgpu::ShaderSource::Wgsl(
                (common + include_str!("particle_render/draw.wgsl")).into(),
            ),
        });
        let entries = |draw: bool| -> Vec<wgpu::BindGroupLayoutEntry> {
            (0..if draw { 8 } else { 5 })
                .map(|binding| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: if draw {
                        if binding >= 6 {
                            wgpu::ShaderStages::FRAGMENT
                        } else if binding == 4 {
                            wgpu::ShaderStages::VERTEX_FRAGMENT
                        } else {
                            wgpu::ShaderStages::VERTEX
                        }
                    } else {
                        wgpu::ShaderStages::COMPUTE
                    },
                    ty: match binding {
                        6 => wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        7 => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        _ => wgpu::BindingType::Buffer {
                            ty: if binding == 4 {
                                wgpu::BufferBindingType::Uniform
                            } else {
                                wgpu::BufferBindingType::Storage {
                                    read_only: draw || binding < 2,
                                }
                            },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                    },
                    count: None,
                })
                .collect()
        };
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle-render-compute"),
            entries: &entries(false),
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle-render-draw"),
            entries: &entries(true),
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&compute_layout],
                push_constant_ranges: &[],
            });
        let draw_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&draw_layout],
            push_constant_ranges: &[],
        });
        let compute = [
            "capture_history",
            "prepare_draw",
            "sort_draw",
            "sort_small_draw",
        ]
        .iter()
        .map(|entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&compute_pipeline_layout),
                module: &compute_shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        })
        .collect();
        let blend = if desc.draw.blend == ParticleBlend::Alpha {
            wgpu::BlendState::ALPHA_BLENDING
        } else {
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particle-indirect-draw"),
            layout: Some(&draw_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            return Err(format!("particle_render.pipeline_invalid:{error}"));
        }
        let buffers = [records, scratch, &indirect, &history, &uniform];
        let compute_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle-render-compute"),
            layout: &compute_layout,
            entries: &buffers
                .iter()
                .enumerate()
                .map(|(binding, b)| wgpu::BindGroupEntry {
                    binding: binding as u32,
                    resource: b.as_entire_binding(),
                })
                .collect::<Vec<_>>(),
        });
        let dummy_mesh = storage(
            device,
            "particle-empty-mesh",
            32,
            wgpu::BufferUsages::empty(),
        )?;
        let dummy = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("particle-unused-texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Ok(Self {
            pipeline,
            indirect,
            history,
            uniform,
            compute,
            compute_binding,
            draw_layout,
            records: records.clone(),
            scratch: scratch.clone(),
            dummy_mesh,
            dummy_view: dummy.create_view(&Default::default()),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            desc: desc.clone(),
            sort_size,
            bytes: history_bytes + indirect_bytes + 192 + 32,
            material_tint: [1.0; 4],
            material_resolved: false,
        })
    }
    fn upload(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &ParticleRenderView,
        time: f32,
        mesh_count: u32,
        textured: bool,
        j: u32,
        k: u32,
    ) {
        let mut words: Vec<u32> = view
            .world_to_clip
            .iter()
            .flatten()
            .map(|v| v.to_f32().to_bits())
            .collect();
        for vector in [&view.right, &view.up, &view.forward, &view.origin] {
            words.extend(vector.iter().map(|v| v.to_f32().to_bits()));
            words.push(0);
        }
        words.extend([j, k, mesh_count, u32::from(textured)]);
        words.extend(
            [
                time,
                self.desc
                    .draw
                    .trail
                    .as_ref()
                    .map_or(0.0, |t| t.lifetime_seconds),
                self.desc
                    .draw
                    .trail
                    .as_ref()
                    .map_or(0.0, |t| t.width_meters),
                0.0,
            ]
            .map(f32::to_bits),
        );
        words.extend([
            self.desc.draw.flipbook.as_ref().map_or(1, |f| f.columns),
            self.desc.draw.flipbook.as_ref().map_or(1, |f| f.rows),
            0,
            0,
        ]);
        words.extend(self.material_tint.map(f32::to_bits));
        let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particle-render-upload"),
            contents: bytemuck::cast_slice(&words),
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        encoder.copy_buffer_to_buffer(&source, 0, &self.uniform, 0, 192);
    }
    fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, pipeline: usize, count: u32) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("particle-render-prepare"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compute[pipeline]);
        pass.set_bind_group(0, &self.compute_binding, &[]);
        pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
    }
    pub fn capture(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        time: f32,
    ) -> u64 {
        if self.desc.draw.trail.is_none() {
            return 0;
        }
        self.upload(
            device,
            encoder,
            &ParticleRenderView::default(),
            time,
            6,
            false,
            0,
            0,
        );
        self.dispatch(encoder, 0, self.desc.capacity);
        1
    }
    pub fn clear(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.clear_buffer(&self.history, 0, None);
        encoder.clear_buffer(&self.indirect, 0, None);
    }
    pub fn prepare(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &ParticleRenderView,
        time: f32,
        texture: Option<(&wgpu::TextureView, &wgpu::Sampler)>,
        mesh: Option<&ParticleMesh>,
    ) -> Result<(wgpu::BindGroup, u64), String> {
        view.validate()?;
        if matches!(self.desc.draw.geometry, ParticleGeometry::Mesh { .. }) && mesh.is_none() {
            return Err("particle_render.mesh_missing".into());
        }
        if self.desc.draw.texture.is_some() && texture.is_none() {
            return Err("particle_render.texture_missing".into());
        }
        let vertex_count = if matches!(self.desc.draw.geometry, ParticleGeometry::Mesh { .. }) {
            mesh.unwrap().vertex_count
        } else {
            6
        };
        self.upload(
            device,
            encoder,
            view,
            time,
            vertex_count,
            texture.is_some(),
            0,
            0,
        );
        // Unsorted emitters are dispatched together by the caller after all
        // independent uniform uploads. Sorted emitters retain their dependencies.
        if self.desc.draw.sort != ParticleSort::None {
            self.dispatch(encoder, 1, self.sort_size);
        }
        let mut dispatches = 1;
        if self.desc.draw.sort == ParticleSort::BackToFront && self.sort_size <= 64 {
            self.dispatch(encoder, 3, 64);
            dispatches += 1;
        } else if self.desc.draw.sort == ParticleSort::BackToFront {
            let mut k = 2;
            while k <= self.sort_size {
                let mut j = k / 2;
                while j > 0 {
                    self.upload(
                        device,
                        encoder,
                        view,
                        time,
                        vertex_count,
                        texture.is_some(),
                        j,
                        k,
                    );
                    self.dispatch(encoder, 2, self.sort_size);
                    dispatches += 1;
                    j /= 2;
                }
                k *= 2;
            }
        }
        let buffers = [
            &self.records,
            &self.scratch,
            &self.indirect,
            &self.history,
            &self.uniform,
            mesh.map_or(&self.dummy_mesh, |m| &m.buffer),
        ];
        let mut entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(binding, b)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 6,
            resource: wgpu::BindingResource::TextureView(texture.map_or(&self.dummy_view, |t| t.0)),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 7,
            resource: wgpu::BindingResource::Sampler(texture.map_or(&self.sampler, |t| t.1)),
        });
        Ok((
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("particle-indirect-draw"),
                layout: &self.draw_layout,
                entries: &entries,
            }),
            dispatches,
        ))
    }

    pub fn encode_unordered_prepare<'a>(&'a self, pass: &mut wgpu::ComputePass<'a>) {
        if self.desc.draw.sort == ParticleSort::None {
            pass.set_pipeline(&self.compute[1]);
            pass.set_bind_group(0, &self.compute_binding, &[]);
            pass.dispatch_workgroups(self.sort_size.div_ceil(64), 1, 1);
        }
    }
}

#[cfg(test)]
mod tests;
