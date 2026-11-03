//! GPU skin objects share the existing geometry preparation, budget and retirement owner.
use super::*;
use crate::character_animation::SkinDeformation3D;
use std::sync::{Arc, Weak};
use wgpu::util::DeviceExt;

type Key = (usize, RenderResourceHandle);
#[derive(Default)]
pub(super) struct SkinGpuState {
    entries: BTreeMap<Key, Arc<NativeSkin>>,
    pipeline: Option<wgpu::ComputePipeline>,
}
pub(super) struct NativeSkin {
    source: Weak<SkinDeformation3D>,
    pub(super) current: wgpu::Buffer,
    pub(super) previous: wgpu::Buffer,
    inputs: Vec<wgpu::Buffer>,
    original: wgpu::Buffer,
    original_offset: u64,
}
impl NativeSkin {
    pub(super) fn matches(&self, draw: &DrawGeometry) -> bool {
        draw.deformation
            .as_ref()
            .is_some_and(|d| self.source.as_ptr() == Arc::as_ptr(d))
    }
    pub(super) fn original_matches(&self, mesh: &ResidentGeometry) -> bool {
        self.original == *mesh.vertices.buffer() && self.original_offset == mesh.vertices.start
    }
    fn bytes(&self) -> u64 {
        self.current.size()
            + self.previous.size()
            + self.inputs.iter().map(|b| b.size()).sum::<u64>()
    }
    pub(super) fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        role: &str,
    ) {
        gpu.buffer(&self.current, "renderer", &[role, "skinCurrent"]);
        gpu.buffer(&self.previous, "renderer", &[role, "skinPrevious"]);
        for b in &self.inputs {
            gpu.buffer(b, "renderer", &[role, "skinInput"]);
        }
    }
}
impl super::super::memory::RetiredGpuResource for Arc<NativeSkin> {
    fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        _: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        self.as_ref().observe(gpu, "retiring");
    }
}
impl SkinGpuState {
    fn key(draw: &DrawGeometry) -> Option<Key> {
        Some((
            Arc::as_ptr(draw.deformation.as_ref()?) as usize,
            draw.geometry_handle().raw(),
        ))
    }
    pub(super) fn resolve(&self, draw: &DrawGeometry) -> Option<Arc<NativeSkin>> {
        self.entries
            .get(&Self::key(draw)?)
            .filter(|s| s.matches(draw))
            .cloned()
    }
    pub(super) fn observe<'a>(&'a self, gpu: &mut super::super::memory::GpuMemoryCensus<'a>) {
        for skin in self.entries.values() {
            skin.observe(gpu, "resident");
        }
    }
}
impl RealWgpuBackend {
    pub(super) fn geometry_skins_current(&self, plan: &RhiCommandPlan) -> bool {
        plan.commands
            .iter()
            .filter_map(RhiCommand::geometry_draw)
            .all(|draw| {
                draw.deformation.is_none()
                    || self.geometry.skins.resolve(draw).is_some_and(|skin| {
                        self.geometry
                            .meshes
                            .get(&draw.geometry_handle().raw())
                            .is_some_and(|mesh| skin.original_matches(mesh))
                    })
            })
    }
    pub(super) fn prepare_geometry_skins(&mut self, plan: &RhiCommandPlan) -> Result<(), String> {
        let mut selected = BTreeMap::new();
        for command in &plan.commands {
            if let Some(draw) = command.geometry_draw() {
                if let Some(key) = SkinGpuState::key(draw) {
                    selected.entry(key).or_insert(draw);
                }
            }
        }
        let retired = self
            .geometry
            .skins
            .entries
            .keys()
            .filter(|k| !selected.contains_key(k))
            .copied()
            .collect::<Vec<_>>();
        let bytes = retired
            .iter()
            .map(|k| self.geometry.skins.entries[k].bytes())
            .sum();
        self.admit_gpu_retirement(bytes, retired.len())?;
        for key in retired {
            let old = self.geometry.skins.entries.remove(&key).unwrap();
            let bytes = old.bytes();
            self.retire_gpu(old, bytes);
        }
        if selected.is_empty() {
            return Ok(());
        }
        if self.device.limits().max_storage_buffers_per_shader_stage < 5 {
            return Err("geometry.skin.compute_unsupported".into());
        }
        if self.geometry.skins.pipeline.is_none() {
            let module = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("character-skin"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("skin.wgsl").into()),
                });
            self.geometry.skins.pipeline = Some(self.device.create_compute_pipeline(
                &wgpu::ComputePipelineDescriptor {
                    label: Some("character-skin"),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                },
            ));
        }
        for (key, draw) in selected {
            if self.geometry.skins.resolve(draw).is_some() {
                continue;
            }
            // Each skin submits real compute; retain completed preparation on retry.
            self.admit_gpu_frame()?;
            let d = draw.deformation.as_ref().unwrap();
            d.validate().map_err(|e| format!("geometry.skin: {e:?}"))?;
            let source = draw.geometry.acquire_upload_source()?;
            if source.vertex_count() != d.source.vertex_count
                || !source.vertex_stride().is_multiple_of(4)
            {
                return Err("geometry.skin.vertex_layout".into());
            }
            let offset = |semantic| {
                source
                    .attributes()
                    .iter()
                    .find(|a| a.semantic == semantic)
                    .map_or(u32::MAX, |a| a.offset / 4)
            };
            let header = [
                d.source.vertex_count,
                source.vertex_stride() / 4,
                offset(VertexSemantic::Position),
                offset(VertexSemantic::Normal),
                u32::MAX,
                d.source.joint_count,
                d.source.morph_count,
                0,
            ];
            let previous = if d.history_reset {
                &d.current_matrices
            } else {
                &d.previous_matrices
            };
            let previous_weights = if d.history_reset {
                &d.current_weights
            } else {
                &d.previous_weights
            };
            let mut data = header
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            for bytes in [
                &d.current_matrices,
                previous,
                &d.current_weights,
                previous_weights,
                &d.source.morphs,
            ] {
                data.extend_from_slice(bytes);
            }
            let upload = source.vertex_bytes().len() + d.source.influences.len() + data.len();
            let allocated = upload as u64 + source.vertex_byte_len() as u64 * 2;
            self.admit_geometry_backing(allocated)?;
            self.geometry
                .streaming
                .admit_visual_upload(plan.frame_index, upload)?;
            for size in [
                source.vertex_byte_len(),
                d.source.influences.len(),
                data.len(),
            ] {
                if size as u64 > u64::from(self.device.limits().max_storage_buffer_binding_size) {
                    return Err("geometry.skin.storage_budget".into());
                }
            }
            let inputs = [
                source.vertex_bytes(),
                d.source.influences.as_slice(),
                data.as_slice(),
            ]
            .into_iter()
            .map(|bytes| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("skin-immutable-input"),
                        contents: bytes,
                        usage: wgpu::BufferUsages::STORAGE,
                    })
            })
            .collect::<Vec<_>>();
            let output = || {
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("skin-immutable-output"),
                    size: source.vertex_byte_len() as u64,
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::VERTEX
                        | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                })
            };
            let current = output();
            let previous = output();
            let pipeline = self.geometry.skins.pipeline.as_ref().unwrap();
            let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("skin-snapshot"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[&inputs[0], &inputs[1], &inputs[2], &current, &previous]
                    .into_iter()
                    .enumerate()
                    .map(|(i, b)| wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: b.as_entire_binding(),
                    })
                    .collect::<Vec<_>>(),
            });
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("skin-current-previous"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &binding, &[]);
                pass.dispatch_workgroups(d.source.vertex_count.div_ceil(64), 1, 1);
            }
            let mesh = &self.geometry.meshes[&draw.geometry_handle().raw()];
            let skin = Arc::new(NativeSkin {
                source: Arc::downgrade(d),
                current,
                previous,
                inputs,
                original: mesh.vertices.buffer().clone(),
                original_offset: mesh.vertices.start,
            });
            // The normal completion owner tracks compute just like other preparation uploads.
            self.geometry.skins.entries.insert(key, skin);
            self.submit_resource_maintenance(encoder);
        }
        Ok(())
    }
}

impl RealWgpuBackend {
    /// Bounded inspection of the exact immutable buffers used by the native draw.
    #[doc(hidden)]
    pub fn read_character_skin_vertices(
        &mut self,
        draw: &DrawGeometry,
    ) -> Result<(Vec<u8>, Vec<u8>), String> {
        let skin = self
            .geometry
            .skins
            .resolve(draw)
            .ok_or("geometry.skin.not_prepared")?;
        let size = skin.current.size();
        self.admit_geometry_backing(size * 2)?;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skin-inspection"),
            size: size * 2,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&skin.current, 0, &readback, 0, size);
        encoder.copy_buffer_to_buffer(&skin.previous, 0, &readback, size, size);
        self.submit_resource_maintenance(encoder);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait())
            .map_err(|e| e.to_string())?;
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = readback.slice(..).get_mapped_range();
        let current = mapped[..size as usize].to_vec();
        let previous = mapped[size as usize..].to_vec();
        drop(mapped);
        readback.unmap();
        Ok((current, previous))
    }
}

#[cfg(test)]
mod backpressure_tests {
    use super::*;
    use crate::character_animation::*;
    use crate::render_graph::RenderPassCommand;
    use std::time::Duration;

    #[test]
    #[ignore = "real GPU; delayed actual completion proves multi-skin admission and retry"]
    fn multi_skin_preparation_preserves_sixteen_submission_cap_and_retries_same_plan() {
        for count in [16, 24] {
            let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
            let warm = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            assert!(backend
                .prepare_surface_resources(&warm, [640, 480], None)
                .unwrap());
            backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
            for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
                if let RenderPassCommand::DrawGeometry { draw } = command {
                    let skin = CharacterSkin {
                        mesh_joint: 0,
                        mesh_asset_id: "mesh".into(),
                        joints: vec![0],
                        inverse_bind: vec![IDENTITY_MATRIX],
                        influences: vec![
                            VertexInfluence {
                                joints: [0; 4],
                                weights: [1., 0., 0., 0.],
                            };
                            3
                        ],
                        morph_targets: vec![],
                        morph_weights: vec![],
                    };
                    Arc::make_mut(draw).deformation = Some(Arc::new(
                        SkinDeformation3D::new(
                            Arc::new(SkinVertexData3D::from_skin(&skin)),
                            &[IDENTITY_MATRIX],
                            &[IDENTITY_MATRIX],
                            &[],
                            &[],
                            false,
                        )
                        .unwrap(),
                    ));
                }
            }
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let delayed = backend.defer_gpu_completion_callbacks_for_test();
            let first = backend
                .prepare_surface_resources(&plan, [640, 480], None)
                .unwrap();
            let stats = backend.gpu_lifecycle_stats();
            assert!(
                stats.in_flight <= 16,
                "multi-skin preparation exceeded original cap: {stats:?}"
            );
            assert!(!first, "partial preparation must retain the same plan");
            assert_eq!(stats.in_flight, 16);
            assert_eq!(backend.geometry.skins.entries.len(), 16);
            let serial = backend.last_gpu_submission().unwrap();
            assert!(!backend
                .prepare_surface_resources(&plan, [640, 480], None)
                .unwrap());
            assert_eq!(backend.last_gpu_submission(), Some(serial));
            backend.device.poll(wgpu::PollType::wait()).unwrap();
            for complete in std::mem::take(&mut *delayed.lock().unwrap()) {
                complete();
            }
            assert!(backend
                .prepare_surface_resources(&plan, [640, 480], None)
                .unwrap());
            assert_eq!(backend.geometry.skins.entries.len(), count);
            assert!(backend.gpu_lifecycle_stats().in_flight <= 16);
            assert!(backend.geometry_skins_current(&plan));
            backend.device.poll(wgpu::PollType::wait()).unwrap();
            for complete in std::mem::take(&mut *delayed.lock().unwrap()) {
                complete();
            }
            backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
            assert_eq!(backend.gpu_lifecycle_stats().in_flight, 0);
        }
    }
}
