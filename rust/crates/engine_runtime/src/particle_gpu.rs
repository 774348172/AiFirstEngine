//! Engine-owned persistent GPU particle simulation on the renderer's device/queue.
use crate::particle_effect::{program, *};
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy)]
pub struct ParticleGpuStep {
    pub step_id: u64,
    pub delta_seconds: f32,
    pub origin: [f32; 3],
    pub emitting: bool,
    pub paused: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParticleGpuStepReport {
    pub dispatch_count: u64,
    pub duplicate: bool,
    pub substeps: u32,
    pub simulated_seconds: f32,
    pub discarded_seconds: f32,
}
#[derive(Debug, Clone)]
pub struct ParticleGpuSample {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub age: f32,
    pub lifetime: f32,
    pub color: [f32; 4],
    pub size: [f32; 2],
    pub rotation: f32,
    pub generation: u32,
    pub serial: u32,
    pub custom_bytes: Vec<u8>,
}
#[derive(Debug, Clone)]
pub struct ParticleGpuSnapshot {
    pub emitter: String,
    pub live: u32,
    pub spawned: u32,
    pub dropped: u32,
    pub collisions: u32,
    pub died: u32,
    pub invalid: u32,
    pub indirect_instances: u32,
    pub particles: Vec<ParticleGpuSample>,
}

pub struct ParticleGpuEffect {
    description: ParticleEffectDescription,
    pub(crate) emitters: Vec<GpuEmitter>,
    events: [wgpu::Buffer; 2],
    event_read: usize,
    last_step: Option<u64>,
    time: f64,
    buffer_bytes: u64,
}
pub(crate) struct GpuEmitter {
    pub(crate) render: Option<crate::particle_render::ParticleEmitterRender>,
    records: wgpu::Buffer,
    pub(crate) scratch: wgpu::Buffer,
    _parameters: wgpu::Buffer,
    step: wgpu::Buffer,
    bindings: [wgpu::BindGroup; 2],
    pipelines: Vec<wgpu::ComputePipeline>,
    serial: u32,
    stride: u32,
}

pub fn supports_particle_compute(limits: &wgpu::Limits) -> bool {
    limits.max_storage_buffers_per_shader_stage >= 5
        && limits.max_compute_invocations_per_workgroup >= 64
        && limits.max_compute_workgroup_size_x >= 64
        && limits.max_compute_workgroups_per_dimension >= 15625
}

/// The Player and offscreen renderer request the same bounded device capability.
/// Unrelated graphics limits retain their existing downlevel baseline.
pub fn renderer_device_limits(available: wgpu::Limits) -> wgpu::Limits {
    let mut required =
        wgpu::Limits::downlevel_webgl2_defaults().using_resolution(available.clone());
    if supports_particle_compute(&available) {
        required.max_storage_buffers_per_shader_stage = 5;
        required.max_storage_buffer_binding_size =
            available.max_storage_buffer_binding_size.min(128 << 20);
        required.max_compute_invocations_per_workgroup = 64;
        required.max_compute_workgroup_size_x = 64;
        required.max_compute_workgroup_size_y = 1;
        required.max_compute_workgroup_size_z = 1;
        required.max_compute_workgroups_per_dimension =
            available.max_compute_workgroups_per_dimension.min(16384);
    }
    required
}
fn buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    extra: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST
            | extra,
        mapped_at_creation: false,
    })
}
fn scratch_size(capacity: u32) -> u64 {
    u64::from(program::align(48 + capacity * 8, 16)) + u64::from(capacity) * 32
}

impl ParticleGpuEffect {
    pub fn new(device: &wgpu::Device, cooked: &CookedParticleEffect) -> Result<Self, String> {
        if !supports_particle_compute(&device.limits()) {
            return Err("particle_gpu.unsupported_compute_limits".into());
        }
        cooked
            .description
            .validate()
            .map_err(|e| format!("particle_gpu.invalid_effect:{e}"))?;
        if cooked.program_contract != PARTICLE_PROGRAM_CONTRACT
            || cooked.programs.len() != cooked.description.emitters.len()
        {
            return Err("particle_gpu.program_contract_mismatch".into());
        }
        let event_bytes = 16 + u64::from(cooked.description.budget.max_events_per_step.max(1)) * 32;
        let mut total = 2 * event_bytes;
        for (desc, code) in cooked.description.emitters.iter().zip(&cooked.programs) {
            if code.emitter != desc.name || code.particle_stride != program::particle_layout(desc).2
            {
                return Err("particle_gpu.layout_mismatch".into());
            }
            for size in [
                u64::from(desc.capacity) * u64::from(code.particle_stride),
                scratch_size(desc.capacity),
                event_bytes,
                program::parameter_bytes(&cooked.description, desc).len() as u64,
            ] {
                if size > device.limits().max_buffer_size
                    || size > u64::from(device.limits().max_storage_buffer_binding_size)
                {
                    return Err(format!("particle_gpu.buffer_limit:{size}"));
                }
            }
            total += u64::from(desc.capacity) * u64::from(code.particle_stride)
                + scratch_size(desc.capacity)
                + program::parameter_bytes(&cooked.description, desc).len() as u64
                + 48;
        }
        let events = [
            buffer(
                device,
                "particle-events-a",
                event_bytes,
                wgpu::BufferUsages::empty(),
            ),
            buffer(
                device,
                "particle-events-b",
                event_bytes,
                wgpu::BufferUsages::empty(),
            ),
        ];
        let mut emitters = Vec::new();
        for (desc, code) in cooked.description.emitters.iter().zip(&cooked.programs) {
            // Capacity-dependent WGSL arrays require explicit per-emitter sizes.
            // A shared unsized layout can retain a larger late minimum when switching
            // differently sized pipelines within one compute pass.
            let sizes = [
                u64::from(desc.capacity) * u64::from(code.particle_stride),
                scratch_size(desc.capacity),
                event_bytes,
                event_bytes,
                program::parameter_bytes(&cooked.description, desc).len() as u64,
                48,
            ];
            let entries: Vec<_> = (0..6)
                .map(|binding| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: if binding == 5 {
                            wgpu::BufferBindingType::Uniform
                        } else {
                            wgpu::BufferBindingType::Storage {
                                read_only: binding == 2 || binding == 4,
                            }
                        },
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(sizes[binding as usize]),
                    },
                    count: None,
                })
                .collect();
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("particle-compute-bindings"),
                entries: &entries,
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("particle-compute-layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&desc.name),
                source: wgpu::ShaderSource::Wgsl(code.wgsl.clone().into()),
            });
            let pipelines = [
                "engine_update",
                "engine_route",
                "engine_spawn",
                "engine_finish",
            ]
            .iter()
            .map(|entry| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
            })
            .collect();
            if let Some(error) = pollster::block_on(device.pop_error_scope()) {
                return Err(format!(
                    "particle_gpu.pipeline_invalid:{}:{error}",
                    desc.name
                ));
            }
            let records = buffer(
                device,
                "particle-records",
                u64::from(desc.capacity) * u64::from(code.particle_stride),
                wgpu::BufferUsages::empty(),
            );
            let scratch = buffer(
                device,
                "particle-indices",
                scratch_size(desc.capacity),
                wgpu::BufferUsages::INDIRECT,
            );
            let parameters = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("particle-parameters"),
                contents: &program::parameter_bytes(&cooked.description, desc),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
            let step = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("particle-step"),
                size: 48,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bindings = std::array::from_fn(|read| {
                let buffers = [
                    &records,
                    &scratch,
                    &events[read],
                    &events[1 - read],
                    &parameters,
                    &step,
                ];
                let entries: Vec<_> = buffers
                    .iter()
                    .enumerate()
                    .map(|(binding, buffer)| wgpu::BindGroupEntry {
                        binding: binding as u32,
                        resource: buffer.as_entire_binding(),
                    })
                    .collect();
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("particle-compute"),
                    layout: &layout,
                    entries: &entries,
                })
            });
            emitters.push(GpuEmitter {
                render: None,
                records,
                scratch,
                _parameters: parameters,
                step,
                bindings,
                pipelines,
                serial: 0,
                stride: code.particle_stride,
            });
        }
        Ok(Self {
            description: cooked.description.clone(),
            emitters,
            events,
            event_read: 0,
            last_step: None,
            time: 0.0,
            buffer_bytes: total,
        })
    }
    pub fn buffer_bytes(&self) -> u64 {
        self.buffer_bytes
            + self
                .emitters
                .iter()
                .filter_map(|e| e.render.as_ref())
                .map(|r| r.bytes)
                .sum::<u64>()
    }
    pub(crate) fn enable_render(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> Result<(), String> {
        for (emitter, desc) in self.emitters.iter_mut().zip(&self.description.emitters) {
            emitter.render = Some(crate::particle_render::ParticleEmitterRender::new(
                device,
                format,
                desc,
                &emitter.records,
                &emitter.scratch,
            )?);
        }
        Ok(())
    }
    pub fn time_seconds(&self) -> f64 {
        self.time
    }

    pub fn set_parameters(
        &self,
        queue: &wgpu::Queue,
        values: &std::collections::BTreeMap<String, ParticleValue>,
    ) -> Result<(), String> {
        let mut description = self.description.clone();
        for (name, value) in values {
            crate::runtime_particles::validate_parameter(&description, name, value)?;
            description
                .parameters
                .iter_mut()
                .find(|p| p.name == *name)
                .unwrap()
                .default = value.clone();
        }
        for (gpu, desc) in self.emitters.iter().zip(&description.emitters) {
            queue.write_buffer(
                &gpu._parameters,
                0,
                &program::parameter_bytes(&description, desc),
            );
        }
        Ok(())
    }

    pub fn validate_parameters(
        &self,
        values: &std::collections::BTreeMap<String, ParticleValue>,
    ) -> Result<(), String> {
        for (name, value) in values {
            crate::runtime_particles::validate_parameter(&self.description, name, value)?;
        }
        Ok(())
    }

    /// Enqueue simulation into the renderer's encoder. No GPU wait or readback.
    pub fn encode_step(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        request: ParticleGpuStep,
    ) -> Result<ParticleGpuStepReport, String> {
        if !request.delta_seconds.is_finite()
            || request.delta_seconds < 0.0
            || !request.origin.iter().all(|v| v.is_finite())
        {
            return Err("particle_gpu.invalid_step".into());
        }
        if self.last_step.is_some_and(|last| request.step_id <= last) {
            return Ok(ParticleGpuStepReport {
                duplicate: true,
                ..Default::default()
            });
        }
        self.last_step = Some(request.step_id);
        if request.paused {
            return Ok(ParticleGpuStepReport::default());
        }
        let simulated = (request.delta_seconds as f64).min(8.0 / 60.0);
        let mut remaining = simulated;
        let mut report = ParticleGpuStepReport {
            simulated_seconds: simulated as f32,
            discarded_seconds: (request.delta_seconds as f64 - simulated).max(0.0) as f32,
            ..Default::default()
        };
        while remaining > 1e-9 {
            let dt = remaining.min(1.0 / 60.0);
            let next = self.time + dt;
            encoder.clear_buffer(&self.events[1 - self.event_read], 0, Some(16));
            let mut step_words = Vec::with_capacity(self.emitters.len() * 12);
            for (index, (gpu, desc)) in self
                .emitters
                .iter_mut()
                .zip(&self.description.emitters)
                .enumerate()
            {
                encoder.clear_buffer(&gpu.scratch, 0, Some(48));
                let scale = f64::from(self.description.budget.quality_scale);
                let requested = if request.emitting {
                    ((emissions_until(desc, next) * scale).floor()
                        - (emissions_until(desc, self.time) * scale).floor())
                    .max(0.0)
                    .min(u32::MAX as f64) as u32
                } else {
                    0
                };
                let words = [
                    (next as f32).to_bits(),
                    (dt as f32).to_bits(),
                    requested,
                    gpu.serial,
                    self.description.seed,
                    index as u32,
                    self.description.budget.max_events_per_step,
                    0,
                    request.origin[0].to_bits(),
                    request.origin[1].to_bits(),
                    request.origin[2].to_bits(),
                    0,
                ];
                step_words.extend(words);
                gpu.serial = gpu.serial.wrapping_add(requested);
            }
            // One upload allocation per substep, retaining command-ordered copies
            // so multiple substeps in the same submission see their own values.
            let staging = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("particle-step-upload"),
                contents: bytemuck::cast_slice(&step_words),
                usage: wgpu::BufferUsages::COPY_SRC,
            });
            for (index, gpu) in self.emitters.iter().enumerate() {
                encoder.copy_buffer_to_buffer(&staging, index as u64 * 48, &gpu.step, 0, 48);
            }
            {
                // Dispatches retain their ordered storage dependencies. A separate
                // pass per emitter/stage only repeats encoder validation and barriers.
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("particle-simulation"),
                    timestamp_writes: None,
                });
                for stage in 0..4 {
                    for (gpu, desc) in self.emitters.iter().zip(&self.description.emitters) {
                        let groups = match stage {
                            1 => self
                                .description
                                .budget
                                .max_events_per_step
                                .max(1)
                                .div_ceil(64),
                            3 => 1,
                            _ => desc.capacity.div_ceil(64),
                        };
                        pass.set_pipeline(&gpu.pipelines[stage]);
                        pass.set_bind_group(0, &gpu.bindings[self.event_read], &[]);
                        pass.dispatch_workgroups(groups, 1, 1);
                        report.dispatch_count += 1;
                    }
                }
            }
            for gpu in &self.emitters {
                if let Some(render) = &gpu.render {
                    report.dispatch_count += render.capture(device, encoder, next as f32);
                }
            }
            self.event_read = 1 - self.event_read;
            self.time = next;
            remaining -= dt;
            report.substeps += 1;
        }
        Ok(report)
    }
    /// Explicit reset, including queued child events. Buffer allocations stay resident.
    pub fn encode_clear(&mut self, encoder: &mut wgpu::CommandEncoder) {
        for gpu in &mut self.emitters {
            if let Some(render) = &gpu.render {
                render.clear(encoder);
            }
            encoder.clear_buffer(&gpu.records, 0, None);
            encoder.clear_buffer(&gpu.scratch, 0, None);
            gpu.serial = 0;
        }
        for events in &self.events {
            encoder.clear_buffer(events, 0, None);
        }
        self.time = 0.0;
        self.event_read = 0;
    }

    /// Bounded, opt-in validation capture. Never called from normal simulation.
    pub fn readback_for_validation(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Vec<ParticleGpuSnapshot>, String> {
        if self
            .description
            .emitters
            .iter()
            .map(|e| u64::from(e.capacity))
            .sum::<u64>()
            > 64
        {
            return Err("particle_gpu.validation_capture_limit_64".into());
        }
        let mut result = Vec::new();
        for (gpu, desc) in self.emitters.iter().zip(&self.description.emitters) {
            let records = read_buffer(
                device,
                queue,
                &gpu.records,
                u64::from(desc.capacity) * u64::from(gpu.stride),
            )?;
            let header = read_buffer(device, queue, &gpu.scratch, 48)?;
            let (custom, particle_bytes, _) = program::particle_layout(desc);
            let mut particles = Vec::new();
            for data in records.chunks_exact(gpu.stride as usize) {
                if word(data, particle_bytes + 24) == 0 {
                    continue;
                }
                particles.push(ParticleGpuSample {
                    position: floats(data, 0),
                    velocity: floats(data, 16),
                    age: real(data, 12),
                    lifetime: real(data, 28),
                    color: floats(data, 32),
                    size: floats(data, 48),
                    rotation: real(data, 56),
                    generation: word(data, particle_bytes + 28),
                    serial: word(data, particle_bytes + 32),
                    custom_bytes: data[custom as usize..particle_bytes as usize].to_vec(),
                });
            }
            particles.sort_by_key(|p| p.serial);
            result.push(ParticleGpuSnapshot {
                emitter: desc.name.clone(),
                live: word(&header, 0),
                spawned: word(&header, 12),
                dropped: word(&header, 16),
                collisions: word(&header, 20),
                died: word(&header, 24),
                invalid: word(&header, 28),
                indirect_instances: word(&header, 36),
                particles,
            });
        }
        Ok(result)
    }
    pub fn read_event_counts_for_validation(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(u32, u32), String> {
        let header = read_buffer(device, queue, &self.events[self.event_read], 16)?;
        Ok((word(&header, 0), word(&header, 4)))
    }
}

fn emissions_until(emitter: &ParticleEmitter, time: f64) -> f64 {
    let elapsed = (time - f64::from(emitter.delay_seconds)).max(0.0);
    if elapsed == 0.0 {
        return 0.0;
    }
    let duration = f64::from(emitter.duration_seconds);
    let rate = f64::from(emitter.rate_per_second);
    let (cycles, within) = if emitter.looping {
        let cycles = (elapsed / duration).floor();
        (cycles, elapsed - cycles * duration)
    } else {
        (0.0, elapsed.min(duration))
    };
    // Keep the fractional rate across loop boundaries; only bursts restart per cycle.
    let burst_count = emitter
        .bursts
        .iter()
        .map(|b| f64::from(b.count))
        .sum::<f64>();
    cycles * burst_count
        + (if emitter.looping { elapsed } else { within } * rate).floor()
        + emitter
            .bursts
            .iter()
            .filter(|b| f64::from(b.time_seconds) < within)
            .map(|b| f64::from(b.count))
            .sum::<f64>()
}
fn read_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &wgpu::Buffer,
    size: u64,
) -> Result<Vec<u8>, String> {
    let target = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particle-validation-readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("particle-validation-copy"),
    });
    encoder.copy_buffer_to_buffer(source, 0, &target, 0, size);
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    target
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let bytes = target.slice(..).get_mapped_range().to_vec();
    target.unmap();
    Ok(bytes)
}
fn word(data: &[u8], offset: u32) -> u32 {
    u32::from_le_bytes(
        data[offset as usize..offset as usize + 4]
            .try_into()
            .unwrap(),
    )
}
fn real(data: &[u8], offset: u32) -> f32 {
    f32::from_bits(word(data, offset))
}
fn floats<const N: usize>(data: &[u8], offset: u32) -> [f32; N] {
    std::array::from_fn(|i| real(data, offset + i as u32 * 4))
}

#[cfg(test)]
mod tests;
