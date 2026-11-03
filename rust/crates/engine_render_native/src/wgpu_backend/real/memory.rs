//! Borrowed native identities, scoped to one capture. No hash-only IDs, cloned
//! native handles, global resource registry, or lifetime changes for reporting.
use super::*;
use crate::resource_memory::{AllocationSample, MemoryDomain, ResourceMemorySample};
use std::collections::{BTreeMap, BTreeSet};
// These keys are borrowed native object identities, never project strings.
// Keep full Eq checks and first-visit allocation IDs with the existing wgpu
// dependency's native identity tables; no hash-only resource proof is stored.
#[cfg(not(target_arch = "wasm32"))]
pub(super) use wgpu::naga::{FastHashMap, FastHashSet};
#[cfg(target_arch = "wasm32")]
pub(super) type FastHashSet<T> = std::collections::HashSet<T>;
#[cfg(target_arch = "wasm32")]
pub(super) type FastHashMap<K, V> = std::collections::HashMap<K, V>;

pub(crate) type PendingGpuBuffers = BTreeMap<wgpu::Buffer, &'static str>;

#[cfg(test)]
mod particle_copy_tests {
    use super::*;

    #[test]
    fn s2d_particle_copy_pending_native_aliases_are_observed_and_retire_on_completion() {
        let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        let baseline = backend.geometry_backing_bytes();
        backend
            .set_geometry_visual_resident_limit(baseline + 448)
            .unwrap();
        let mut encoder = backend.device.create_command_encoder(&Default::default());
        let destination = backend.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test-external-copy-destination"),
            size: 448,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        for (bytes, offset) in [(144, 0), (304, 144)] {
            let source = backend.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("test-particle-copy"),
                size: bytes,
                usage: wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            encoder.copy_buffer_to_buffer(&source, 0, &destination, offset, bytes);
            backend
                .pending_draw_buffers
                .insert(source.clone(), "particles");
            backend.pending_draw_buffers.insert(source, "particles");
        }
        assert_eq!(backend.geometry_backing_bytes(), baseline + 448);
        assert_eq!(
            backend.admit_geometry_backing(1).unwrap_err(),
            "geometry.visual_resident_budget"
        );
        let mut sample = ResourceMemorySample::new(1, 1, 0);
        backend.observe_memory(&mut sample);
        let copies: Vec<_> = sample
            .allocations
            .iter()
            .filter(|a| {
                a.domain == MemoryDomain::Gpu
                    && a.owner == "particles"
                    && a.roles.iter().any(|r| r == "inFlight")
            })
            .collect();
        assert_eq!(copies.len(), 2);
        assert_eq!(copies.iter().map(|a| a.bytes).sum::<u64>(), 448);
        backend.submit_resource_maintenance(encoder);
        assert!(backend.pending_draw_buffers.is_empty());
        let token = backend.last_gpu_submission().unwrap();
        assert_eq!(token.frame_index, 0);
        assert_eq!(
            backend
                .wait_gpu_submission(token, std::time::Duration::from_secs(2))
                .unwrap(),
            GpuSubmissionStatus::Completed
        );
        assert_eq!(backend.geometry_backing_bytes(), baseline);
    }
}

#[derive(Default)]
pub(crate) struct GpuMemoryCensus<'a> {
    buffers: FastHashMap<&'a wgpu::Buffer, usize>,
    textures: FastHashMap<&'a wgpu::Texture, (usize, u64)>,
    backing_templates: FastHashSet<usize>,
    allocations: Vec<AllocationSample>,
    backing_only: bool,
    requested_bytes: u64,
}
impl<'a> GpuMemoryCensus<'a> {
    pub(super) fn backing_only() -> Self {
        Self {
            backing_only: true,
            ..Self::default()
        }
    }
    pub(super) fn is_backing_only(&self) -> bool {
        self.backing_only
    }
    pub(super) fn requested_bytes(&self) -> u64 {
        self.requested_bytes
    }

    // Borrowed for this one census: no resource can be mutated or its allocation
    // recycled while this identity is live. Trace still emits every role alias.
    pub(super) fn visit_template<T>(&mut self, template: &'a T) -> bool {
        !self.backing_only
            || self
                .backing_templates
                .insert(std::ptr::from_ref(template).addr())
    }

    pub(crate) fn buffer(&mut self, buffer: &'a wgpu::Buffer, owner: &str, roles: &[&str]) {
        let next = self.buffers.len();
        let id = *self.buffers.entry(buffer).or_insert_with(|| {
            self.requested_bytes = self.requested_bytes.saturating_add(buffer.size());
            next
        });
        if self.backing_only {
            return;
        }
        self.allocations.push(AllocationSample {
            allocation_id: format!("buffer:{id}"),
            domain: MemoryDomain::Gpu,
            owner: owner.into(),
            bytes: buffer.size(),
            roles: roles.iter().map(|r| (*r).into()).collect(),
        });
    }
    pub(crate) fn texture(&mut self, texture: &'a wgpu::Texture, roles: &[&str]) {
        // Exact engine-requested texel storage for the supported static formats;
        // backend tiling, alignment and driver-private heaps remain unavailable.
        if texture.sample_count() != 1 || texture.dimension() == wgpu::TextureDimension::D3 {
            return;
        }
        let Some(block_bytes) = texture.format().block_copy_size(None) else {
            return;
        };
        let next = self.textures.len();
        let (id, bytes) = *self.textures.entry(texture).or_insert_with(|| {
            #[cfg(test)]
            TEXTURE_STORAGE_CALCULATIONS.with(|n| n.set(n.get() + 1));
            let (bw, bh) = texture.format().block_dimensions();
            let size = texture.size();
            let bytes: u64 = (0..texture.mip_level_count())
                .map(|level| {
                    u64::from((size.width >> level).max(1).div_ceil(bw))
                        * u64::from((size.height >> level).max(1).div_ceil(bh))
                        * u64::from(size.depth_or_array_layers)
                        * u64::from(block_bytes)
                })
                .sum();
            self.requested_bytes = self.requested_bytes.saturating_add(bytes);
            (next, bytes)
        });
        if self.backing_only {
            return;
        }
        self.allocations.push(AllocationSample {
            allocation_id: format!("texture:{id}"),
            domain: MemoryDomain::Gpu,
            owner: "renderer".into(),
            bytes,
            roles: roles.iter().map(|r| (*r).into()).collect(),
        });
    }
}
/// Type erasure preserves the original resource and its completion-driven Drop.
/// Observation borrows it; no separate handles keep retired storage alive.
pub(crate) trait RetiredGpuResource: wgpu::WasmNotSend {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, sample: &mut ResourceMemorySample);
}
impl RetiredGpuResource for wgpu::Buffer {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {
        gpu.buffer(self, "renderer", &["retiring"]);
    }
}
impl RetiredGpuResource for (wgpu::Buffer, &'static str) {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {
        gpu.buffer(&self.0, self.1, &["retiring"]);
    }
}
impl RetiredGpuResource for wgpu::Texture {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {
        gpu.texture(self, &["retiring"]);
    }
}
impl RetiredGpuResource for wgpu::BindGroup {
    fn observe<'a>(&'a self, _: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {}
}
impl RetiredGpuResource for crate::particle_gpu::ParticleGpuEffect {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {
        self.observe_memory(gpu, &["retiring"]);
    }
}
impl RetiredGpuResource for crate::particle_render::ParticleMesh {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {
        gpu.buffer(&self.buffer, "particles", &["retiring"]);
    }
}
impl RetiredGpuResource for wgpu::RenderBundle {
    fn observe<'a>(&'a self, _: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {}
}
impl RetiredGpuResource for wgpu::RenderPipeline {
    fn observe<'a>(&'a self, _: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {}
}
impl RetiredGpuResource for ResidentTexture {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, sample: &mut ResourceMemorySample) {
        if !gpu.is_backing_only() {
            sample.cpu_storage(
                self as *const _ as usize,
                std::mem::size_of_val(self),
                "renderer",
                &["retiring"],
            );
            sample.cpu_arc(&self._texture, "renderer", &["retiring"]);
        }
        gpu.texture(&self._texture, &["retiring"]);
    }
}
impl RetiredGpuResource for (u32, u32, wgpu::TextureView) {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {
        gpu.texture(self.2.texture(), &["retiring"]);
    }
}
impl RetiredGpuResource for ([f32; 36], wgpu::Buffer, wgpu::BindGroup) {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, _: &mut ResourceMemorySample) {
        gpu.buffer(&self.1, "renderer", &["retiring"]);
    }
}
impl RealWgpuBackend {
    /// Borrow the existing physical owners only when admission needs new backing.
    /// Includes geometry, textures, particles and original retirement owners.
    /// Includes UI backing; external output/readback and driver heaps remain separate.
    /// No CPU payload walk, report strings, native handle clones or retained census.
    pub(in crate::wgpu_backend) fn geometry_backing_bytes(&self) -> u64 {
        let mut gpu = GpuMemoryCensus::backing_only();
        let mut sample = ResourceMemorySample::new(0, 0, 0);
        self.geometry.observe_memory(&mut gpu, &mut sample);
        for texture in self.textures.values() {
            gpu.texture(&texture._texture, &[]);
        }
        self.observe_particle_memory(&mut gpu, &["resident"]);
        self.retained_ui.observe_memory(&mut gpu);
        for texture in &self.output_textures {
            gpu.texture(texture, &["output", "inFlight"]);
        }
        for buffer in &self.readback_buffers {
            gpu.buffer(buffer, "renderer", &["readback", "inFlight"]);
        }
        for v in self.ui_vertices.values() {
            gpu.buffer(&v.buffer, "renderer", &["resident"]);
        }
        for (buffer, owner) in &self.pending_draw_buffers {
            gpu.buffer(buffer, owner, &["inFlight"]);
        }
        self.lifecycle.observe_memory(&mut gpu, &mut sample);
        debug_assert!(sample.allocations.is_empty());
        gpu.requested_bytes()
    }

    pub(in crate::wgpu_backend) fn admit_geometry_backing(&self, extra: u64) -> Result<(), String> {
        // Fixed-profile reuse must not scan the resource pool every frame.
        if extra == 0 {
            return Ok(());
        }
        let limit = self.geometry.visual_resident_limit();
        if self
            .geometry_backing_bytes()
            .checked_add(extra)
            .is_none_or(|n| n > limit)
        {
            return Err("geometry.visual_resident_budget".into());
        }
        Ok(())
    }

    /// Called only at an explicit filtered Trace boundary on the rendering owner.
    pub fn observe_memory(&self, sample: &mut ResourceMemorySample) {
        crate::render_geometry::observe_success_proof_memory(sample);
        for ((name, _), digest) in &self.font_bundle_identities {
            sample.cpu_string(name, "renderer", &["decodedCache"]);
            sample.cpu_string(digest, "renderer", &["decodedCache"]);
        }
        let mut gpu = GpuMemoryCensus::default();
        self.geometry.observe_memory(&mut gpu, sample);
        for value in self.textures.values() {
            sample.cpu_arc(&value._texture, "renderer", &["resident"]);
            gpu.texture(&value._texture, &["resident"]);
        }
        for value in self.ui_vertices.values() {
            gpu.buffer(&value.buffer, "renderer", &["resident"]);
        }
        self.retained_ui.observe_memory(&mut gpu);
        for texture in &self.output_textures {
            gpu.texture(texture, &["output", "inFlight"]);
        }
        for buffer in &self.readback_buffers {
            gpu.buffer(buffer, "renderer", &["readback", "inFlight"]);
        }
        for (buffer, owner) in &self.pending_draw_buffers {
            gpu.buffer(buffer, owner, &["inFlight"]);
        }
        self.observe_particle_memory(&mut gpu, &["resident"]);
        if let Some(value) = &self.measurement {
            value.observe_memory(&mut gpu, sample);
        }
        self.lifecycle.observe_memory(&mut gpu, sample);
        sample.allocations.extend(gpu.allocations);
        sample.partial(
            "gpu.owner.renderer",
            "gpu.geometryPages+parameters+depth+texture+ui+retiredBacking",
        );
        sample.partial(
            "gpu.owner.particles",
            "gpu.nativeSimulation+render+meshBuffers;fallbackTexelsInRenderer",
        );
        sample.partial(
            "gpu.role.inFlight",
            "gpu.pendingDrawBacking;residentAliasesDeduplicated;notEveryLiveBinding",
        );
        sample.partial("gpu.role.resident", "gpu.engineRequestedResidentBacking");
        sample.partial("gpu.role.retiring", "gpu.engineRequestedRetiredBacking");
        // A queue write's upload charge is not a physical staging buffer census.
        // Query sets, pipelines, bundles and driver heaps remain opaque.
        sample.observed_scopes.extend(BTreeSet::from([
            "gpu.bufferIdentity=wgpuEquality".into(),
            "gpu.textureBytes=engineRequestedTexels".into(),
        ]));
        sample.capacities.insert(
            "framesInFlight".into(),
            self.gpu_lifecycle_stats().in_flight as u64,
        );
        sample.capacities.insert(
            "uploadBytesPerFrame".into(),
            self.geometry_upload_snapshot().frame_uploaded_bytes as u64,
        );
    }

    fn observe_particle_memory<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, roles: &[&str]) {
        for effect in self.particle_effects.values() {
            effect.observe_memory(gpu, roles);
        }
        for mesh in self.particle_meshes.values() {
            gpu.buffer(&mesh.buffer, "particles", roles);
        }
        if let Some((_, _, view)) = &self.particle_depth {
            gpu.texture(view.texture(), roles);
        }
    }
}

#[cfg(test)]
thread_local! {static TEXTURE_STORAGE_CALCULATIONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
mod backing_work_tests {
    use super::*;
    #[test]
    fn gpu_backing_census_resolves_storage_once_per_native_object_keeps_all_roles() {
        let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
        let descriptor = wgpu::TextureDescriptor {
            label: Some("census-alias-work"),
            size: wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            mip_level_count: 4,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = backend.device.create_texture(&descriptor);
        let other = backend.device.create_texture(&descriptor);
        let buffer = backend.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("census-buffer-alias"),
            size: 96,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let aliases: Vec<_> = (0..count)
                .map(|_| (texture.clone(), buffer.clone()))
                .collect();
            for backing_only in [false, true] {
                TEXTURE_STORAGE_CALCULATIONS.with(|n| n.set(0));
                let mut census = if backing_only {
                    GpuMemoryCensus::backing_only()
                } else {
                    GpuMemoryCensus::default()
                };
                for (t, b) in &aliases {
                    census.texture(t, &["resident"]);
                    census.texture(t, &["retiring"]);
                    census.buffer(b, "renderer", &["resident"]);
                    census.buffer(b, "renderer", &["retiring"]);
                }
                census.texture(&other, &["resident"]);
                assert_eq!(census.requested_bytes(), 340 * 2 + 96);
                assert_eq!(census.buffers.len(), 1);
                assert_eq!(census.textures.len(), 2);
                if backing_only {
                    assert!(census.allocations.is_empty());
                } else {
                    assert_eq!(census.allocations.len(), count * 4 + 1);
                    let texture_id = &census.allocations[0].allocation_id;
                    assert!(census
                        .allocations
                        .iter()
                        .filter(|a| a.allocation_id == *texture_id)
                        .any(|a| a.roles.iter().any(|r| r == "retiring")));
                    assert_ne!(
                        census.allocations.last().unwrap().allocation_id,
                        *texture_id
                    );
                }
                observed.push((
                    count,
                    backing_only,
                    TEXTURE_STORAGE_CALCULATIONS.with(|n| n.get()),
                ));
            }
        }
        eprintln!("actual native texture storage calculations {observed:?}");
        assert!(
            observed.iter().all(|(_, _, n)| *n == 2),
            "alias count multiplied storage work: {observed:?}"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
}
