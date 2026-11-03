//! Optional GPU preprocessing inside the original geometry owner. A query is
//! reusable only for the exact immutable complete plan, native preparation and
//! output rectangle that produced it. New content always starts visible.
use super::*;
use std::sync::Weak;
use wgpu::util::DeviceExt;

const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MIN_COMMANDS: usize = 32;
const MAX_COMMANDS: usize = 4096;
type Key = (u64, u32, usize);

#[derive(Default)]
pub(super) struct GpuScaleState {
    source: Weak<Vec<RhiCommand>>,
    extent: [u32; 2],
    rect: Option<GameViewRect>,
    parameters: Option<(wgpu::Buffer, wgpu::Buffer, wgpu::Buffer)>,
    ranges: BTreeMap<Key, ScaleRange>,
    pipeline: Option<wgpu::ComputePipeline>,
    disabled: bool,
    pub(super) direct_queries: u64,
    pub(super) indirect_frames: u64,
    pub(super) fallbacks: u64,
}
struct ScaleRange {
    recipe: encoding::Recipe,
    queries: wgpu::QuerySet,
    results: wgpu::Buffer,
    seeds: wgpu::Buffer,
    arguments: wgpu::Buffer,
    binding: wgpu::BindGroup,
    warmed: bool,
    count: u32,
    viewport: GameViewRect,
    depth: wgpu::TextureView,
}
impl ScaleRange {
    fn bytes(&self) -> u64 {
        self.results.size() + self.seeds.size() + self.arguments.size()
    }
    fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        for buffer in [&self.results, &self.seeds, &self.arguments] {
            gpu.buffer(buffer, "renderer", &[role, "gpuScale"]);
        }
        self.recipe.observe(gpu, sample, role);
        if !gpu.is_backing_only() {
            sample.partial(
                "gpu.owner.renderer",
                "occlusionQuerySetDriverStorageUnavailable",
            );
        }
    }
}
impl super::super::memory::RetiredGpuResource for Vec<ScaleRange> {
    fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        for range in self {
            range.observe(gpu, sample, "retiring");
        }
    }
}
impl GpuScaleState {
    pub(super) fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        for range in self.ranges.values() {
            range.observe(gpu, sample, "resident");
        }
    }
    fn supported(device: &wgpu::Device) -> bool {
        let limits = device.limits();
        device
            .features()
            .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE)
            && limits.max_compute_invocations_per_workgroup >= 64
            && limits.max_compute_workgroup_size_x >= 64
            && limits.max_storage_buffers_per_shader_stage >= 3
    }
    fn pipeline(device: &wgpu::Device) -> wgpu::ComputePipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world-gpu-preprocessing"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_scale.wgsl").into()),
        });
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("world-gpu-preprocessing"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }
}
impl RealWgpuBackend {
    pub(super) fn invalidate_gpu_scale(&mut self) -> Result<(), String> {
        if !self.geometry.gpu_scale.ranges.is_empty() {
            let bytes = self
                .geometry
                .gpu_scale
                .ranges
                .values()
                .map(ScaleRange::bytes)
                .sum();
            self.admit_gpu_retirement(bytes, 1)?;
            let old = std::mem::take(&mut self.geometry.gpu_scale.ranges)
                .into_values()
                .collect::<Vec<_>>();
            self.retire_gpu(old, bytes);
        }
        self.geometry.gpu_scale.source = Weak::new();
        Ok(())
    }
    /// Explicit CPU reference/fallback switch, also usable on capable devices.
    pub fn set_geometry_gpu_scale_enabled(&mut self, enabled: bool) {
        self.geometry.gpu_scale.disabled = !enabled;
    }
    /// Compact actual-path counters. No GPU synchronization or JSON in the frame.
    pub fn geometry_gpu_scale_counts(&self) -> [u64; 3] {
        let state = &self.geometry.gpu_scale;
        [state.direct_queries, state.indirect_frames, state.fallbacks]
    }

    pub(super) fn try_encode_gpu_scale(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        commands: &[RhiCommand],
        first_instance: u32,
        viewport: GameViewRect,
    ) -> Result<bool, String> {
        let Some(plan) = self.geometry.validated_plan.as_ref() else {
            return Ok(false);
        };
        let Some(parameters) = self.geometry.parameters.as_ref() else {
            return Ok(false);
        };
        let source = plan.command_backing.clone();
        let extent = plan.extent;
        let rect = plan.rect;
        let buffers = (
            parameters.buffer.clone(),
            parameters.view_buffer.clone(),
            parameters.index_buffer.clone(),
        );
        let state = &self.geometry.gpu_scale;
        let current = Weak::ptr_eq(&state.source, &source)
            && state.extent == extent
            && state.rect == rect
            && state.parameters.as_ref() == Some(&buffers);
        if !current && !state.ranges.is_empty() {
            let bytes = state.ranges.values().map(ScaleRange::bytes).sum();
            self.admit_gpu_retirement(bytes, 1)?;
            let ranges = std::mem::take(&mut self.geometry.gpu_scale.ranges)
                .into_values()
                .collect::<Vec<_>>();
            self.retire_gpu(ranges, bytes);
        }
        let state = &mut self.geometry.gpu_scale;
        if !current {
            state.source = source.clone();
            state.extent = extent;
            state.rect = rect;
            state.parameters = Some(buffers.clone());
        }
        if state.disabled
            || !GpuScaleState::supported(&self.device)
            || !(MIN_COMMANDS..=MAX_COMMANDS).contains(&commands.len())
        {
            state.fallbacks += 1;
            return Ok(false);
        }
        // Deforming, blended, mask and shadow draws retain the existing CPU
        // path. A query may only suppress a wholly opaque depth-writing batch.
        if commands.iter().any(|command| {
            geometry_draw(command).is_none_or(|draw| {
                draw.deformation.is_some()
                    || draw.blend != MaterialBlend::Opaque
                    || draw.alpha_cutoff.is_some()
                    || draw.view.is_shadow_map()
                    || draw.depth != MaterialDepth::TestAndWrite
                    || draw.depth_target.is_none()
            })
        }) {
            state.fallbacks += 1;
            return Ok(false);
        }
        let draw = geometry_draw(&commands[0]).unwrap();
        let key = (draw.view.view_id, first_instance, commands.len());
        let depth = draw
            .depth_target
            .as_ref()
            .and_then(|id| self.geometry.depths.get::<str>(id.as_ref()))
            .map(|(_, _, v)| v)
            .unwrap()
            .clone();
        if self
            .geometry
            .gpu_scale
            .ranges
            .get(&key)
            .is_some_and(|range| range.viewport != viewport || range.depth != depth)
        {
            self.invalidate_gpu_scale()?;
            let state = &mut self.geometry.gpu_scale;
            state.source = source;
            state.extent = extent;
            state.rect = rect;
            state.parameters = Some(buffers);
        }
        if !self.geometry.gpu_scale.ranges.contains_key(&key) {
            let recipe = encoding::Recipe::capture_commands(self, commands, first_instance)?;
            let seeds = recipe.indirect_seeds();
            let bytes = seeds.len() as u64 * 48;
            let used: u64 = self
                .geometry
                .gpu_scale
                .ranges
                .values()
                .map(ScaleRange::bytes)
                .sum();
            if seeds.is_empty()
                || used + bytes > MAX_BYTES
                || self.geometry.gpu_scale.ranges.len() >= 64
                || self.admit_geometry_backing(bytes).is_err()
            {
                self.geometry.gpu_scale.fallbacks += 1;
                return Ok(false);
            }
            let state = &mut self.geometry.gpu_scale;
            let pipeline = state
                .pipeline
                .get_or_insert_with(|| GpuScaleState::pipeline(&self.device));
            let seeds_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("world-indirect-seeds"),
                    contents: bytemuck::cast_slice(&seeds),
                    usage: wgpu::BufferUsages::STORAGE,
                });
            // A render that aborts before query submission must remain visible:
            // initial results are nonzero, never an uninitialized zero verdict.
            let visible = vec![[1u32, 0]; seeds.len()];
            let results = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("world-occlusion-results"),
                    contents: bytemuck::cast_slice(&visible),
                    usage: wgpu::BufferUsages::QUERY_RESOLVE
                        | wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_SRC,
                });
            let arguments = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("world-indexed-indirect"),
                size: seeds.len() as u64 * 20,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("world-gpu-preprocessing"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: seeds_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: results.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: arguments.as_entire_binding(),
                    },
                ],
            });
            let count = seeds.len() as u32;
            let queries = self.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("world-opaque-occlusion"),
                ty: wgpu::QueryType::Occlusion,
                count,
            });
            state.ranges.insert(
                key,
                ScaleRange {
                    recipe,
                    queries,
                    results,
                    seeds: seeds_buffer,
                    arguments,
                    binding,
                    warmed: false,
                    count,
                    viewport,
                    depth: depth.clone(),
                },
            );
        }
        let state = &mut self.geometry.gpu_scale;
        let range = state.ranges.get_mut(&key).unwrap();
        if range.warmed {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("world-gpu-preprocessing"),
                timestamp_writes: None,
            });
            pass.set_pipeline(state.pipeline.as_ref().unwrap());
            pass.set_bind_group(0, &range.binding, &[]);
            pass.dispatch_workgroups(range.count.div_ceil(64), 1, 1);
        }
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("world-gpu-scale"),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: (!range.warmed).then_some(&range.queries),
        });
        set_viewport(&mut pass, viewport);
        let recorded = if range.warmed {
            state.indirect_frames += 1;
            range.recipe.record_indirect(&mut pass, &range.arguments)
        } else {
            state.direct_queries += 1;
            range.recipe.record_queries(&mut pass, &range.queries)
        };
        drop(pass);
        if !range.warmed {
            encoder.resolve_query_set(&range.queries, 0..range.count, &range.results, 0);
            range.warmed = true;
        }
        self.geometry.stats.passes += 1;
        self.geometry.stats.draws += recorded;
        self.geometry.stats.instances += commands
            .iter()
            .map(RhiCommand::geometry_instance_count)
            .sum::<usize>() as u64;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "gpu_scale_tests.rs"]
mod tests;
