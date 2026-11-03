//! Local indexed UI geometry and ordered instance parameters. One queue, write/encode/submit.
use super::*;
use crate::aui_retained::{UiLocalGeometry, UiRetainedDraw};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 1024;
const MAX_INSTANCES: usize = 4096;
const PARAM_BYTES: usize = 64;

struct Resident {
    source: Arc<UiLocalGeometry>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    bytes: usize,
    last_used: u64,
}
pub(super) struct Prepared {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    instances: std::ops::Range<u32>,
    scissor: [u32; 4],
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetainedUiStats {
    pub geometry_uploads: u64,
    pub geometry_uploaded_bytes: u64,
    pub parameter_uploaded_bytes: u64,
    pub draws: u64,
    pub instances: u64,
    pub cache_hits: u64,
    pub resident_bytes: usize,
    pub resident_entries: usize,
    pub retiring_bytes: usize,
    pub completed_submission: u64,
}
pub(super) struct RetainedUiGpu {
    pipeline: wgpu::RenderPipeline,
    parameters: wgpu::Buffer,
    residents: BTreeMap<u64, Resident>,
    retiring: Vec<(u64, Resident)>,
    completed: Arc<AtomicU64>,
    submitted: u64,
    epoch: u64,
    unsubmitted_writes: bool,
    pub stats: RetainedUiStats,
}
impl RetainedUiGpu {
    pub(super) fn observe_memory<'a>(&'a self, gpu: &mut super::memory::GpuMemoryCensus<'a>) {
        gpu.buffer(&self.parameters, "renderer", &["resident"]);
        for value in self.residents.values() {
            gpu.buffer(&value.vertices, "renderer", &["resident"]);
            gpu.buffer(&value.indices, "renderer", &["resident"]);
        }
        for (_, value) in &self.retiring {
            gpu.buffer(&value.vertices, "renderer", &["retiring"]);
            gpu.buffer(&value.indices, "renderer", &["retiring"]);
        }
    }
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aui-retained-local"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct Out { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vs_main(@location(0) p: vec2<f32>, @location(1) color: vec4<f32>,
 @location(2) uv: vec2<f32>, @location(3) row0: vec4<f32>, @location(4) row1: vec4<f32>,
 @location(5) tint: vec4<f32>, @location(6) view: vec4<f32>) -> Out {
 let q = vec3<f32>(p, 1.0);
 let canvas = vec2<f32>(dot(row0.xyz,q), dot(row1.xyz,q));
 var out: Out;
 out.position = vec4<f32>(canvas * view.xy + view.zw, 0.0, 1.0);
 out.color = clamp(color * tint, vec4<f32>(0.0), vec4<f32>(1.0));
 return out;
}
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> { return in.color; }
"#
                .into(),
            ),
        });
        const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aui-retained-local"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    RuntimeVertex::layout(),
                    wgpu::VertexBufferLayout {
                        array_stride: PARAM_BYTES as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &ATTRIBUTES,
                    },
                ],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let parameters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aui-retained-instance-page"),
            size: (MAX_INSTANCES * PARAM_BYTES) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            parameters,
            residents: Default::default(),
            retiring: Vec::new(),
            completed: Arc::new(AtomicU64::new(0)),
            submitted: 0,
            epoch: 0,
            unsubmitted_writes: false,
            stats: Default::default(),
        }
    }
    pub(super) fn collect(&mut self, device: &wgpu::Device) -> Result<(), String> {
        device
            .poll(wgpu::PollType::Poll)
            .map_err(|e| format!("aui.retained_poll:{e}"))?;
        self.collect_completed();
        Ok(())
    }
    pub(super) fn collect_completed(&mut self) {
        let completed = self.completed.load(Ordering::Acquire);
        self.retiring.retain(|(serial, _)| *serial > completed);
        self.stats.retiring_bytes = self.retiring.iter().map(|(_, r)| r.bytes).sum();
        self.stats.completed_submission = completed;
    }
    pub fn submitted(&mut self, queue: &wgpu::Queue) {
        self.unsubmitted_writes = false;
        self.submitted += 1;
        let serial = self.submitted;
        let completed = self.completed.clone();
        queue.on_submitted_work_done(move || {
            completed.fetch_max(serial, Ordering::Release);
        });
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        plan: &RhiCommandPlan,
        retiring_limit: usize,
        retiring_objects: usize,
    ) -> Result<Vec<Vec<Prepared>>, String> {
        self.collect(device)?;
        self.epoch += 1;
        let mut active = BTreeMap::new();
        for command in &plan.commands {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::UiRetained { draws },
                ..
            } = command
            {
                for draw in draws {
                    active.insert(
                        Arc::as_ptr(&draw.geometry) as usize,
                        draw.geometry.vertices.len() * 32 + draw.geometry.indices.len() * 4,
                    );
                }
            }
        }
        if active.len() > MAX_ENTRIES || active.values().sum::<usize>() > MAX_BYTES {
            return Err("aui.retained_frame_geometry_budget".into());
        }
        let mut parameters = Vec::<[f32; 16]>::new();
        let mut output = Vec::with_capacity(plan.commands.len());
        for command in &plan.commands {
            let mut prepared: Vec<Prepared> = Vec::new();
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::UiRetained { draws },
                ..
            } = command
            {
                for draw in draws {
                    validate(draw)?;
                    if parameters.len() == MAX_INSTANCES {
                        return Err("aui.retained_instance_budget".into());
                    }
                    let key = draw.geometry.id;
                    let hit = self
                        .residents
                        .get(&key)
                        .is_some_and(|r| Arc::ptr_eq(&r.source, &draw.geometry));
                    if !hit {
                        validate_geometry(&draw.geometry)?;
                        self.retire_key(device, key, retiring_limit, retiring_objects)?;
                        let bytes =
                            draw.geometry.vertices.len() * 32 + draw.geometry.indices.len() * 4;
                        if bytes > MAX_BYTES {
                            return Err("aui.retained_geometry_budget".into());
                        }
                        while self.residents.len() >= MAX_ENTRIES
                            || self.residents.values().map(|r| r.bytes).sum::<usize>() + bytes
                                > MAX_BYTES
                        {
                            let oldest = self
                                .residents
                                .iter()
                                .min_by_key(|(_, r)| r.last_used)
                                .map(|(&k, _)| k)
                                .unwrap();
                            self.retire_key(device, oldest, retiring_limit, retiring_objects)?;
                        }
                        let vertices: Vec<_> = draw
                            .geometry
                            .vertices
                            .iter()
                            .map(RealWgpuBackend::runtime_vertex)
                            .collect();
                        self.unsubmitted_writes = true;
                        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("aui-local-vertices"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("aui-local-indices"),
                            contents: bytemuck::cast_slice(&draw.geometry.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                        self.residents.insert(
                            key,
                            Resident {
                                source: draw.geometry.clone(),
                                vertices: vb,
                                indices: ib,
                                bytes,
                                last_used: self.epoch,
                            },
                        );
                        self.stats.geometry_uploads += 1;
                        self.stats.geometry_uploaded_bytes += bytes as u64;
                    } else {
                        self.stats.cache_hits += 1;
                    }
                    let r = self.residents.get_mut(&key).unwrap();
                    r.last_used = self.epoch;
                    let instance = parameters.len() as u32;
                    parameters.push(draw.parameters.map(|v| v.to_f32()));
                    if let Some(last) = prepared.last_mut().filter(|p| {
                        p.scissor == draw.scissor
                            && p.vertices == r.vertices
                            && p.instances.end == instance
                    }) {
                        last.instances.end += 1;
                    } else {
                        prepared.push(Prepared {
                            vertices: r.vertices.clone(),
                            indices: r.indices.clone(),
                            count: draw.geometry.indices.len() as u32,
                            instances: instance..instance + 1,
                            scissor: draw.scissor,
                        });
                    }
                }
            }
            output.push(prepared);
        }
        // Queue writes are ordered before the following immediate submission. Older immutable
        // CPU packets regenerate their own page; no packet stores offsets into mutable CPU data.
        if !parameters.is_empty() {
            self.unsubmitted_writes = true;
            queue.write_buffer(&self.parameters, 0, bytemuck::cast_slice(&parameters));
            self.stats.parameter_uploaded_bytes += (parameters.len() * PARAM_BYTES) as u64;
        }
        self.stats.resident_entries = self.residents.len();
        self.stats.resident_bytes = self.residents.values().map(|r| r.bytes).sum();
        Ok(output)
    }
    /// Empty geometry frames may omit command-indexed UI preparation only when
    /// there is no resident, queued write or retirement maintenance to perform.
    pub(super) fn empty_for_geometry_frame(&self) -> bool {
        self.residents.is_empty() && self.retiring.is_empty() && !self.unsubmitted_writes
    }

    pub(super) fn retiring_count(&self) -> usize {
        self.retiring.len()
    }
    pub(super) fn has_unsubmitted_work(&self) -> bool {
        self.unsubmitted_writes
            || self
                .retiring
                .iter()
                .any(|(serial, _)| *serial > self.submitted)
    }

    pub(super) fn planned_backing_bytes(&self, plan: &RhiCommandPlan) -> Result<u64, String> {
        let mut changed = BTreeMap::new();
        let mut bytes = 0u64;
        for command in &plan.commands {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::UiRetained { draws },
                ..
            } = command
            {
                for draw in draws {
                    validate(draw)?;
                    let key = draw.geometry.id;
                    let pointer = Arc::as_ptr(&draw.geometry);
                    let previous = changed
                        .get(&key)
                        .copied()
                        .or_else(|| self.residents.get(&key).map(|r| Arc::as_ptr(&r.source)));
                    if previous != Some(pointer) {
                        validate_geometry(&draw.geometry)?;
                        bytes = bytes
                            .checked_add(
                                (draw.geometry.vertices.len() * 32
                                    + draw.geometry.indices.len() * 4)
                                    as u64,
                            )
                            .ok_or("geometry.visual_resident_budget")?;
                        changed.insert(key, pointer);
                    }
                }
            }
        }
        Ok(bytes)
    }

    fn retire_key(
        &mut self,
        device: &wgpu::Device,
        key: u64,
        shared_limit: usize,
        shared_objects: usize,
    ) -> Result<(), String> {
        self.collect(device)?;
        let Some(r) = self.residents.get(&key) else {
            return Ok(());
        };
        if self.stats.retiring_bytes.saturating_add(r.bytes) > MAX_BYTES.min(shared_limit)
            || self.retiring.len() >= shared_objects
        {
            return Err("aui.retained_retirement_backpressure".into());
        }
        let r = self.residents.remove(&key).unwrap();
        // A replaced geometry may already be in this very frame's Prepared
        // draws. Its first/last use then belongs to the next actual submission.
        let serial = self.submitted + u64::from(r.last_used == self.epoch);
        self.retiring.push((serial, r));
        self.stats.retiring_bytes = self.retiring.iter().map(|(_, r)| r.bytes).sum();
        Ok(())
    }
    pub fn draw(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        prepared: &[Prepared],
        viewport: Option<GameViewRect>,
        target: [u32; 2],
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(1, self.parameters.slice(..));
        for p in prepared {
            let [x, y, w, h] = p.scissor;
            let scissor = if let Some(v) = viewport {
                let sx = v.width / target[0] as f32;
                let sy = v.height / target[1] as f32;
                let edge = |v: f32| (v - 0.5).ceil().max(0.) as u32;
                let left = edge(v.x + x as f32 * sx);
                let top = edge(v.y + y as f32 * sy);
                let right = edge(v.x + (x + w) as f32 * sx);
                let bottom = edge(v.y + (y + h) as f32 * sy);
                [
                    left,
                    top,
                    right.saturating_sub(left),
                    bottom.saturating_sub(top),
                ]
            } else {
                p.scissor
            };
            if scissor[2] == 0 || scissor[3] == 0 {
                continue;
            }
            pass.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3]);
            pass.set_vertex_buffer(0, p.vertices.slice(..));
            pass.set_index_buffer(p.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..p.count, 0, p.instances.clone());
            self.stats.draws += 1;
            self.stats.instances += u64::from(p.instances.end - p.instances.start);
        }
    }
}
fn validate(d: &UiRetainedDraw) -> Result<(), String> {
    if d.parameters.iter().any(|v| !v.to_f32().is_finite())
        || d.target_extent.contains(&0)
        || d.scissor[2] == 0
        || d.scissor[3] == 0
        || d.scissor[0]
            .checked_add(d.scissor[2])
            .is_none_or(|x| x > d.target_extent[0])
        || d.scissor[1]
            .checked_add(d.scissor[3])
            .is_none_or(|y| y > d.target_extent[1])
    {
        return Err("aui.retained_packet_invalid".into());
    }
    Ok(())
}
fn validate_geometry(g: &UiLocalGeometry) -> Result<(), String> {
    if g.vertices.is_empty()
        || g.indices.is_empty()
        || !g.indices.len().is_multiple_of(3)
        || g.indices.iter().any(|i| *i as usize >= g.vertices.len())
        || g.vertices.iter().any(|v| {
            v.position
                .iter()
                .chain(&v.color)
                .chain(&v.uv)
                .any(|v| !v.to_f32().is_finite())
        })
    {
        return Err("aui.retained_geometry_invalid".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_replacement_in_current_prepare_waits_for_next_submission_and_rejects_before_removal(
    ) {
        let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
        let ui = &mut backend.retained_ui;
        ui.epoch = 1;
        ui.submitted = 4;
        ui.completed.store(4, Ordering::Release);
        ui.residents.insert(
            42,
            Resident {
                source: Arc::new(UiLocalGeometry {
                    id: 42,
                    vertices: vec![],
                    indices: vec![],
                }),
                vertices: backend.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 96,
                    usage: wgpu::BufferUsages::VERTEX,
                    mapped_at_creation: false,
                }),
                indices: backend.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 12,
                    usage: wgpu::BufferUsages::INDEX,
                    mapped_at_creation: false,
                }),
                bytes: 108,
                last_used: 1,
            },
        );
        assert!(ui.retire_key(&backend.device, 42, 107, 1).is_err());
        assert!(ui.retire_key(&backend.device, 42, 108, 0).is_err());
        assert!(ui.residents.contains_key(&42));
        ui.retire_key(&backend.device, 42, 108, 1).unwrap();
        assert_eq!(ui.retiring[0].0, 5);
        ui.collect(&backend.device).unwrap();
        assert_eq!(
            ui.stats.retiring_bytes, 108,
            "the previous completion cannot retire this frame's draw"
        );
        assert_eq!(
            backend.gpu_lifecycle_stats().retiring_bytes,
            108,
            "the public completion owner must include retained UI retirement"
        );
        assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 1);
        backend.retained_ui.completed.store(5, Ordering::Release);
        backend.poll_gpu_completion().unwrap();
        assert_eq!(
            backend.retained_ui.stats.retiring_bytes, 0,
            "an ordinary completion poll must collect before the next allocation admission"
        );
        assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
    }
    #[test]
    fn retained_parameters_are_ordered_for_three_submissions_without_intermediate_gpu_wait() {
        let mut backend = RealWgpuBackend::new_offscreen(64, 32).unwrap();
        let geometry = Arc::new(UiLocalGeometry {
            id: 42,
            vertices: [[0., 0.], [10., 0.], [10., 10.], [0., 10.]]
                .map(|p| RenderDrawVertex::new(p, [0., 1., 0., 1.], [0.; 2]))
                .to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3],
        });
        let mut targets = Vec::new();
        for (frame, x) in [10., 30., 10.].into_iter().enumerate() {
            let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("retained-inflight-test"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: backend.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let draw = UiRetainedDraw {
                geometry: geometry.clone(),
                parameters: [
                    1.,
                    0.,
                    x,
                    0.,
                    0.,
                    1.,
                    10.,
                    0.,
                    1.,
                    1.,
                    1.,
                    1.,
                    2. / 64.,
                    -2. / 32.,
                    -1.,
                    1.,
                ]
                .map(Into::into),
                scissor: [0, 0, 64, 32],
                target_extent: [64, 32],
            };
            let plan = RhiCommandPlan {
                graph_schedule: None,
                frame_identity: None,
                packet_content_version: None,
                frame_index: frame as u64,
                graph_id: "inflight-ui".into(),
                target_kind: "headlessTexture".into(),
                diagnostics: vec![],
                commands: vec![RhiCommand::Draw {
                    target: "target".into(),
                    draw_kind: RhiDrawKind::UiComposition,
                    vertex_count: 6,
                    payload: RhiDrawPayload::UiRetained { draws: vec![draw] },
                }]
                .into(),
            };
            let report = backend
                .execute_plan_to_surface_view(&plan, &texture.create_view(&Default::default()));
            assert!(report.diagnostics.is_empty(), "{report:?}");
            targets.push(texture);
        }
        // All three submissions are already enqueued. Copy/read only now.
        for (index, texture) in targets.into_iter().enumerate() {
            let readback = backend.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 64 * 32 * 4,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = backend.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: Default::default(),
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(32),
                    },
                },
                wgpu::Extent3d {
                    width: 64,
                    height: 32,
                    depth_or_array_layers: 1,
                },
            );
            backend.queue.submit(Some(encoder.finish()));
            let (send, recv) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    send.send(result).unwrap();
                });
            backend.device.poll(wgpu::PollType::wait()).unwrap();
            recv.recv().unwrap().unwrap();
            let bytes = readback.slice(..).get_mapped_range();
            let green = |x: usize| bytes[(15 * 64 + x) * 4 + 1];
            assert_eq!(
                green(15),
                if index == 1 { 0 } else { 255 },
                "earlier submission used later parameters"
            );
            assert_eq!(
                green(35),
                if index == 1 { 255 } else { 0 },
                "later submission used stale parameters"
            );
        }
    }
}
