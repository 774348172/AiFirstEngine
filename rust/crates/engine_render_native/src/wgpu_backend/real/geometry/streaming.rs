//! Segmented uploads in the existing geometry/texture owners and completion ledger.
use super::*;
use crate::render_asset_production::PreparedTextureAsset;
use crate::render_geometry::PreparedGeometry;
use std::collections::VecDeque;

#[cfg(test)]
#[path = "streaming/source_tests.rs"]
mod source_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryUploadBudget {
    pub resident_bytes: u64,
    pub staging_bytes: usize,
    pub bytes_per_frame: usize,
    pub max_tickets: usize,
}
impl Default for GeometryUploadBudget {
    fn default() -> Self {
        Self {
            resident_bytes: 256 * 1024 * 1024,
            staging_bytes: 4 * 1024 * 1024,
            bytes_per_frame: 1024 * 1024,
            max_tickets: 128,
        }
    }
}

pub use crate::geometry_upload::{GeometryUploadState, GeometryUploadTicket};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GeometryUploadSnapshot {
    pub tickets: usize,
    pub pending_resources: usize,
    pub staging_bytes: usize,
    pub uploaded_bytes: u64,
    pub frame_uploaded_bytes: usize,
}
pub(super) struct MeshUploadSource {
    pub(super) owner: Arc<GeometryBuffers>,
    pub(super) payload: Arc<GeometryBuffers>,
}

pub(super) fn prepare_mesh_sources<T: Send>(
    count: usize,
    parallel: bool,
    acquire: &(impl Fn(usize) -> Result<T, String> + Sync),
) -> Result<Vec<Result<T, String>>, String> {
    if !parallel || count < 2 {
        return Ok((0..count).map(acquire).collect());
    }
    let partitions = count.min(4);
    let width = count.div_ceil(partitions);
    // Keep source errors inside their indexed result. The native owner consumes
    // the original prefix in order, even when a later worker fails first.
    crate::render_geometry::read_native_partitions(partitions, &|partition| {
        let start = partition * width;
        let end = (start + width).min(count);
        Ok((start..end).map(acquire).collect::<Vec<_>>())
    })
    .map(|parts| parts.into_iter().flatten().collect())
    .map_err(|error| error.to_string())
}
impl MeshUploadSource {
    pub(super) fn acquire(owner: Arc<GeometryBuffers>) -> Result<Self, String> {
        let payload = owner.acquire_upload_source()?;
        Ok(Self { owner, payload })
    }
    fn complete(&self) {
        self.owner.release_upload_source();
    }
    fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_arc(&self.owner, "assets", &["queuePayload"]);
        self.owner.observe_memory(sample, "queuePayload");
        sample.cpu_arc(&self.payload, "assets", &["staging", "queuePayload"]);
        self.payload.observe_memory(sample, "queuePayload");
    }
}

impl Drop for MeshUploadSource {
    fn drop(&mut self) {
        // Unqueued preparations must relinquish the shared CPU cache too.
        // Queued leases still live in the original real-completion ledger.
        self.complete();
    }
}

enum Source {
    Mesh {
        lease: Option<MeshUploadSource>,
        vertex: usize,
        index: usize,
    },
    Texture {
        payload: Option<Arc<crate::runtime_texture::RuntimeTexturePayload>>,
        owner: Option<PreparedTextureAsset>,
        offset: usize,
    },
}
struct Progress {
    source: Source,
    identity: String,
    scheduled: bool,
    ready: bool,
    token: Option<GpuSubmissionToken>,
}
#[derive(Default)]
pub(super) struct GeometryStreaming {
    budget: GeometryUploadBudget,
    budget_explicit: bool,
    tickets: BTreeMap<u64, Vec<RenderResourceHandle>>,
    pins: BTreeMap<RenderResourceHandle, usize>,
    resources: BTreeMap<RenderResourceHandle, Progress>,
    // Exact owner state: updates accompany every insertion, completion and final unpin.
    pending_resources: usize,
    batches: VecDeque<(GpuSubmissionToken, usize, Vec<MeshUploadSource>)>,
    pub(super) queued_mesh_sources: Vec<MeshUploadSource>,
    frame: Option<(u64, usize)>,
    // Native loading and gameplay have different frame identities but share the
    // allowance of one actual surface presentation. Headless callers keep their
    // existing logical pump contract when this is not bound.
    presentation_frame: Option<u64>,
    uploaded_bytes: u64,
    pump_cpu_ns: u64,
    texture_write_calls: u64,
    peak_staged: usize,
    // Bytes accepted by an owner but not yet attached to a real submission.
    queued_bytes: usize,
}
impl GeometryStreaming {
    pub(super) fn visual_upload_allowance(&self, frame: u64) -> (usize, usize) {
        let frame = self.presentation_frame.unwrap_or(frame);
        let frame = self.frame.map_or(frame, |(old, _)| old.max(frame));
        let used = self
            .frame
            .filter(|(old, _)| *old == frame)
            .map_or(0, |(_, n)| n);
        let staging = self
            .queued_bytes
            .saturating_add(self.batches.iter().map(|(_, n, _)| *n).sum::<usize>());
        (
            self.budget.bytes_per_frame,
            self.budget
                .bytes_per_frame
                .saturating_sub(used)
                .min(self.budget.staging_bytes.saturating_sub(staging)),
        )
    }
    pub(super) fn visual_uploads_profiled(&self) -> bool {
        self.budget_explicit || self.presentation_frame.is_some() || !self.tickets.is_empty()
    }
    pub(super) fn admit_visual_upload(&mut self, frame: u64, bytes: usize) -> Result<(), String> {
        if bytes == 0 {
            return Ok(());
        }
        let frame = self.presentation_frame.unwrap_or(frame);
        let frame = self.frame.map_or(frame, |(old, _)| old.max(frame));
        let used = self
            .frame
            .filter(|(old, _)| *old == frame)
            .map_or(0, |(_, n)| n);
        let staged = self
            .queued_bytes
            .saturating_add(self.batches.iter().map(|(_, n, _)| *n).sum::<usize>());
        if used.saturating_add(bytes) > self.budget.bytes_per_frame
            || staged.saturating_add(bytes) > self.budget.staging_bytes
        {
            return Err("geometry.visual_upload_capacity".into());
        }
        self.frame = Some((frame, used + bytes));
        self.queued_bytes += bytes;
        self.uploaded_bytes += bytes as u64;
        self.peak_staged = self.peak_staged.max(staged + bytes);
        Ok(())
    }
    pub(super) fn admit_environment_upload(
        &mut self,
        frame: u64,
        bytes: usize,
        resident: u64,
    ) -> Result<(), String> {
        if bytes == 0 {
            return Ok(());
        }
        if resident > self.budget.resident_bytes {
            return Err("environment.resident_capacity".into());
        }
        self.admit_visual_upload(frame, bytes)
            .map_err(|_| "environment.upload_capacity".into())
    }
    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.cpu_deque(&self.batches, "renderer", &["queuePayload"]);
        sample.cpu_vec(&self.queued_mesh_sources, "renderer", &["queuePayload"]);
        for lease in &self.queued_mesh_sources {
            lease.observe_memory(sample);
        }
        for (_, _, leases) in &self.batches {
            sample.cpu_vec(leases, "renderer", &["queuePayload"]);
            for lease in leases {
                lease.observe_memory(sample);
            }
        }
        for handles in self.tickets.values() {
            sample.cpu_vec(handles, "renderer", &["queuePayload"]);
        }
        for progress in self.resources.values() {
            sample.cpu_string(&progress.identity, "renderer", &["queuePayload"]);
            if let Source::Mesh {
                lease: Some(lease), ..
            } = &progress.source
            {
                lease.observe_memory(sample);
            }
            if let Source::Texture {
                payload: Some(payload),
                ..
            } = &progress.source
            {
                sample.cpu_vec(
                    &payload.rgba8,
                    "assets",
                    &["decodedCache", "staging", "queuePayload"],
                );
            }
        }
        for (name, value) in [
            ("uploadPumpCpuNs", self.pump_cpu_ns),
            ("textureWriteCalls", self.texture_write_calls),
            ("uploadStagingPeakBytes", self.peak_staged as u64),
        ] {
            sample.capacities.insert(name.into(), value);
        }
        sample.partial("cpu.owner.renderer", "upload.ticketVectors+identityStrings+sharedPayload;stdNodesOpaque;wgpuImplicitStagingOpaque");
        sample.partial(
            "cpu.role.staging",
            "upload.sourcePixels;wgpuImplicitStagingOpaque",
        );
    }
    pub(super) fn plan_resident_budget(&self) -> Option<u64> {
        // The default is the segmented-loading profile, not a new limit on
        // legacy static uploads. An explicitly configured backend budget is
        // shared by both paths; active streaming pins also keep their limit.
        (self.budget_explicit || !self.tickets.is_empty()).then_some(self.budget.resident_bytes)
    }
    pub(super) fn pinned(&self, handle: &RenderResourceHandle) -> bool {
        self.pins.contains_key(handle)
    }
    fn insert_progress(&mut self, handle: RenderResourceHandle, progress: Progress) {
        let pending = !progress.ready;
        let previous = self
            .resources
            .insert(handle, progress)
            .is_some_and(|p| !p.ready);
        match (previous, pending) {
            (false, true) => self.pending_resources += 1,
            (true, false) => self.pending_resources -= 1,
            _ => (),
        }
    }
    fn remove_progress(&mut self, handle: &RenderResourceHandle) {
        if let Some(progress) = self.resources.remove(handle) {
            if !progress.ready {
                self.pending_resources -= 1;
            }
            if let Source::Mesh {
                lease: Some(lease), ..
            } = progress.source
            {
                if let Some(token) = progress.token {
                    // Cancellation drops the ticket, not an upload's real last
                    // use. The existing completion queue now owns its CPU lease.
                    self.batches.push_back((token, 0, vec![lease]));
                } else {
                    lease.complete();
                }
            }
        }
    }

    pub(super) fn release_mesh_sources_after_device_destroy(&self) {
        for lease in &self.queued_mesh_sources {
            lease.complete();
        }
        for (_, _, leases) in &self.batches {
            for lease in leases {
                lease.complete();
            }
        }
        for progress in self.resources.values() {
            if let Source::Mesh {
                lease: Some(lease), ..
            } = &progress.source
            {
                lease.complete();
            }
        }
    }
    fn complete_progress(&mut self, handle: &RenderResourceHandle) {
        let progress = self
            .resources
            .get_mut(handle)
            .expect("current completed upload");
        if !progress.ready {
            self.pending_resources -= 1;
            progress.ready = true;
        }
        if let Source::Mesh { lease, .. } = &mut progress.source {
            if let Some(lease) = lease.take() {
                lease.complete();
            }
        }
        if let Source::Texture { payload, owner, .. } = &mut progress.source {
            if let Some(owner) = owner.take() {
                owner.release_upload_payload();
            }
            *payload = None;
        }
    }
    pub(super) fn pending(&self, handle: &RenderResourceHandle) -> bool {
        if self.pending_resources == 0 {
            return false;
        }
        #[cfg(test)]
        PENDING_RESOURCE_QUERIES.with(|n| n.set(n.get() + 1));
        self.resources.get(handle).is_some_and(|p| !p.ready)
    }
}
impl GeometryGpuState {
    pub(in crate::wgpu_backend) fn visual_upload_remaining(&self, frame: u64) -> Option<usize> {
        self.streaming
            .visual_uploads_profiled()
            .then(|| self.streaming.visual_upload_allowance(frame).1)
    }
    pub(in crate::wgpu_backend) fn admit_visual_upload(
        &mut self,
        frame: u64,
        bytes: usize,
    ) -> Result<(), String> {
        // Legacy unprofiled 2D clients retain their old owner limits. Explicit
        // Windows 3D profiles, tickets and physical Present bindings opt in.
        if self.streaming.visual_uploads_profiled() {
            self.streaming.admit_visual_upload(frame, bytes)
        } else {
            Ok(())
        }
    }
    pub(in crate::wgpu_backend) fn submitted_visual_uploads(&mut self, token: GpuSubmissionToken) {
        let bytes = std::mem::take(&mut self.streaming.queued_bytes);
        let leases = std::mem::take(&mut self.streaming.queued_mesh_sources);
        if bytes != 0 || !leases.is_empty() {
            self.streaming.batches.push_back((token, bytes, leases));
        }
    }
    pub(in crate::wgpu_backend) fn collect_visual_uploads(&mut self, completed: u64) {
        self.streaming.batches.retain(|(token, _, leases)| {
            if token.serial > completed {
                return true;
            }
            for lease in leases {
                lease.complete();
            }
            false
        });
    }
    pub(in crate::wgpu_backend) fn has_queued_visual_uploads(&self) -> bool {
        self.streaming.queued_bytes != 0 || !self.streaming.queued_mesh_sources.is_empty()
    }
}
impl GeometryGpuState {
    pub(in crate::wgpu_backend::real) fn streaming_texture_pinned(
        &self,
        handle: &RenderResourceHandle,
    ) -> bool {
        self.streaming.pinned(handle)
    }
}

impl RealWgpuBackend {
    /// Bind the upload allowance to the next physical presentation. Repeated
    /// redraw/acquire retries must reuse this number until a Present succeeds.
    /// This does not create a GPU submission, game frame, or measurement sample.
    pub fn bind_geometry_upload_presentation(&mut self, presentation: u64) -> Result<(), String> {
        let streaming = &mut self.geometry.streaming;
        if presentation == 0
            || streaming
                .presentation_frame
                .is_some_and(|old| presentation < old)
            || streaming.frame.is_some_and(|(old, _)| presentation < old)
        {
            return Err("geometry.upload_frame_regression".into());
        }
        streaming.presentation_frame = Some(presentation);
        Ok(())
    }

    pub fn set_geometry_upload_budget(
        &mut self,
        budget: GeometryUploadBudget,
    ) -> Result<(), String> {
        if !self.geometry.streaming.tickets.is_empty()
            || !self.geometry.streaming.batches.is_empty()
            || self.geometry.streaming.queued_bytes != 0
            || !self.geometry.streaming.queued_mesh_sources.is_empty()
        {
            return Err("geometry.upload_budget_busy".into());
        }
        if budget.resident_bytes == 0
            || budget.staging_bytes < 4
            || budget.bytes_per_frame < 4
            || budget.bytes_per_frame > budget.staging_bytes
            || budget.max_tickets == 0
            || budget.max_tickets > 4096
        {
            return Err("geometry.invalid_upload_budget".into());
        }
        self.geometry.streaming.budget = budget;
        self.geometry.streaming.budget_explicit = true;
        Ok(())
    }

    pub fn geometry_upload_snapshot(&self) -> GeometryUploadSnapshot {
        let s = &self.geometry.streaming;
        GeometryUploadSnapshot {
            tickets: s.tickets.len(),
            pending_resources: s.pending_resources,
            staging_bytes: s.queued_bytes + s.batches.iter().map(|(_, b, _)| b).sum::<usize>(),
            uploaded_bytes: s.uploaded_bytes,
            frame_uploaded_bytes: s.frame.map_or(0, |(_, b)| b),
        }
    }

    /// Pins resources in their original physical owners. Admission validates the
    /// complete request before allocating; shared tickets do not duplicate uploads.
    pub fn begin_geometry_upload(
        &mut self,
        geometry: &[PreparedGeometry],
        textures: &[PreparedTextureAsset],
    ) -> Result<GeometryUploadTicket, String> {
        self.admit_gpu_frame()?;
        let budget = self.geometry.streaming.budget;
        if self.geometry.streaming.tickets.len() >= budget.max_tickets {
            return Err("geometry.upload_ticket_budget".into());
        }
        let mut meshes = BTreeMap::new();
        let mut images = BTreeMap::new();
        for source in geometry {
            let (handle, buffers) = source.upload_source();
            if meshes
                .insert(handle, buffers.clone())
                .is_some_and(|old| old != buffers)
                || self
                    .geometry
                    .meshes
                    .get(&handle)
                    .is_some_and(|old| old.source != buffers)
            {
                return Err("geometry.resource_identity_conflict".into());
            }
            if self.geometry.meshes.contains_key(&handle)
                && !self.geometry.streaming.resources.contains_key(&handle)
                && self.last_gpu_submission().is_none()
            {
                return Err("geometry.existing_mesh_without_submission".into());
            }
            if buffers.vertex_stride() > self.device.limits().max_vertex_buffer_array_stride
                || padded(buffers.vertex_byte_len()) as u64 > self.device.limits().max_buffer_size
                || padded(buffers.index_byte_len()) as u64 > self.device.limits().max_buffer_size
            {
                return Err("geometry.device_limit".into());
            }
        }
        for texture in textures {
            texture.validate_upload_descriptor()?;
            if texture.payload.width > self.device.limits().max_texture_dimension_2d
                || texture.payload.height > self.device.limits().max_texture_dimension_2d
            {
                return Err("geometry.texture_device_limit".into());
            }
            if texture.payload.format.starts_with("bc3") {
                if !self
                    .device
                    .features()
                    .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
                {
                    return Err("wgpu.texture_compression_unsupported".into());
                }
                if budget.bytes_per_frame < 16 || budget.staging_bytes < 16 {
                    return Err("geometry.texture_upload_block_budget".into());
                }
            }
            let handle = texture.handle.raw();
            let identity = texture.content_identity();
            let payload = if self.textures.contains_key(&handle) {
                None
            } else {
                Some(texture.acquire_upload_payload()?)
            };
            if images
                .insert(handle, (texture, identity.clone(), payload))
                .is_some_and(|(_, old, _)| old != identity)
                || self
                    .geometry
                    .streaming
                    .resources
                    .get(&handle)
                    .is_some_and(|p| p.identity != identity)
                || (self.textures.contains_key(&handle)
                    && !self.geometry.streaming.resources.contains_key(&handle))
            {
                return Err("geometry.texture_identity_conflict".into());
            }
        }
        let existing = self
            .geometry_residency()
            .physical_mesh_bytes
            .saturating_add(self.gpu_lifecycle_stats().retiring_bytes)
            .saturating_add(self.textures.values().map(|t| t.byte_len()).sum::<u64>())
            .saturating_add(
                self.geometry
                    .environment_buffers
                    .values()
                    .map(|e| e.buffer.size())
                    .sum::<u64>(),
            );
        let visual_existing = self.geometry_backing_bytes();
        let extra_texture: u64 = images
            .iter()
            .filter(|(h, _)| !self.textures.contains_key(h))
            .map(|(_, (t, _, _))| t.upload_byte_len() as u64)
            .sum();
        if existing.saturating_add(extra_texture) > budget.resident_bytes {
            return Err("geometry.upload_resident_budget".into());
        }
        let sources: Vec<_> = meshes
            .iter()
            .filter(|(h, _)| !self.geometry.meshes.contains_key(h))
            .map(|(h, b)| (*h, b.clone()))
            .collect();
        // Acquire every missing CPU source before mutating physical owners.
        let mut mesh_leases = sources
            .iter()
            .map(|(handle, owner)| {
                Ok((
                    *handle,
                    MeshUploadSource {
                        owner: owner.clone(),
                        payload: owner.acquire_upload_source()?,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        self.admit_geometry_backing(extra_texture)?;
        let other_bytes = existing
            .saturating_sub(self.geometry.pages.physical_bytes())
            .saturating_add(extra_texture);
        let visual_page_limit = self.geometry.visual_resident_limit.saturating_sub(
            visual_existing
                .saturating_sub(self.geometry.pages.physical_bytes())
                .saturating_add(extra_texture),
        );
        let reserved = self.geometry.pages.reserve(
            &self.device,
            &sources,
            budget
                .resident_bytes
                .saturating_sub(other_bytes)
                .min(visual_page_limit),
        )?;
        for (handle, mesh) in reserved {
            self.geometry.meshes.insert(handle, mesh);
        }
        let new_handles: BTreeSet<_> = sources.iter().map(|(h, _)| *h).collect();
        let mut handles = Vec::new();
        for (handle, source) in meshes {
            if new_handles.contains(&handle) {
                self.geometry.streaming.insert_progress(
                    handle,
                    Progress {
                        source: Source::Mesh {
                            lease: mesh_leases.remove(&handle),
                            vertex: 0,
                            index: 0,
                        },
                        identity: String::new(),
                        scheduled: false,
                        ready: false,
                        token: None,
                    },
                );
            } else if !self.geometry.streaming.resources.contains_key(&handle) {
                let token = self
                    .last_gpu_submission()
                    .ok_or("geometry.existing_mesh_without_submission")?;
                let ready = self.submission_status(token) == Some(GpuSubmissionStatus::Completed);
                self.geometry.streaming.insert_progress(
                    handle,
                    Progress {
                        source: Source::Mesh {
                            lease: None,
                            vertex: padded(source.vertex_byte_len()),
                            index: padded(source.index_byte_len()),
                        },
                        identity: String::new(),
                        scheduled: true,
                        ready,
                        token: Some(token),
                    },
                );
            }
            handles.push(handle);
        }
        for (handle, (texture, identity, payload)) in images {
            if !self.textures.contains_key(&handle) {
                let payload = payload
                    .as_ref()
                    .expect("missing texture acquired before admission");
                let format = match payload.format.as_str() {
                    "rgba8UnormSrgb" => wgpu::TextureFormat::Rgba8UnormSrgb,
                    "rgba8Unorm" => wgpu::TextureFormat::Rgba8Unorm,
                    "bc3RgbaUnorm" => wgpu::TextureFormat::Bc3RgbaUnorm,
                    "bc3RgbaUnormSrgb" => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
                    _ => unreachable!("validated texture payload"),
                };
                let image = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("streamed-geometry-texture"),
                    size: wgpu::Extent3d {
                        width: payload.width,
                        height: payload.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: payload.mip_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let view = image.create_view(&wgpu::TextureViewDescriptor {
                    mip_level_count: (!crate::runtime_texture::TextureSampler::parse(
                        &payload.sampler,
                    )
                    .expect("validated sampler")
                    .min_filter
                    .uses_mips())
                    .then_some(1),
                    ..Default::default()
                });
                let sampler = self
                    .device
                    .create_sampler(&sampler_descriptor(&payload.sampler));
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("streamed-geometry-texture-binding"),
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });
                self.textures.insert(
                    handle,
                    ResidentTexture {
                        _texture: Arc::new(OwnedTexture(image)),
                        _view: view,
                        _sampler: sampler,
                        bind_group,
                        _width: payload.width,
                        _height: payload.height,
                    },
                );
                self.owned_geometry_textures.insert(handle);
                self.geometry.streaming.insert_progress(
                    handle,
                    Progress {
                        source: Source::Texture {
                            payload: Some(payload.clone()),
                            owner: Some(texture.clone()),
                            offset: 0,
                        },
                        identity,
                        scheduled: false,
                        ready: false,
                        token: None,
                    },
                );
            }
            handles.push(handle);
        }
        for handle in &handles {
            *self.geometry.streaming.pins.entry(*handle).or_default() += 1;
        }
        let ticket =
            GeometryUploadTicket::issue_for_backend(self.gpu_lifecycle_stats().device_epoch);
        let id = ticket.backend_parts().0;
        self.geometry.streaming.tickets.insert(id, handles);
        Ok(ticket)
    }

    /// Called by the render owner. All calls in a frame share one byte budget.
    /// Queue writes are tagged with a real submission in the original ledger.
    pub fn pump_geometry_uploads(&mut self, frame_index: u64) -> Result<usize, String> {
        let frame_index = self
            .geometry
            .streaming
            .presentation_frame
            .unwrap_or(frame_index);
        let pump_started = crate::platform_time::Instant::now();
        self.poll_gpu_completion()?;
        self.refresh_geometry_uploads();
        self.admit_gpu_frame()?;
        if self
            .geometry
            .streaming
            .frame
            .is_some_and(|(old, _)| frame_index < old)
        {
            return Err("geometry.upload_frame_regression".into());
        }
        if self
            .geometry
            .streaming
            .frame
            .is_none_or(|(old, _)| frame_index != old)
        {
            self.geometry.streaming.frame = Some((frame_index, 0));
        }
        let budget = self.geometry.streaming.budget;
        let staged: usize = self.geometry.streaming.queued_bytes
            + self
                .geometry
                .streaming
                .batches
                .iter()
                .map(|(_, b, _)| *b)
                .sum::<usize>();
        let remaining = budget
            .bytes_per_frame
            .saturating_sub(self.geometry.streaming.frame.unwrap().1)
            .min(budget.staging_bytes.saturating_sub(staged))
            & !3;
        if remaining == 0 {
            return Ok(0);
        }
        let mut written = 0usize;
        let mut changed = Vec::new();
        for (handle, progress) in &mut self.geometry.streaming.resources {
            if progress.scheduled || written == remaining {
                continue;
            }
            let before = written;
            match &mut progress.source {
                Source::Mesh {
                    lease,
                    vertex,
                    index,
                } => {
                    let mesh = &self.geometry.meshes[handle];
                    let payload = &lease.as_ref().expect("pending mesh upload source").payload;
                    for (buffer, base, source, offset) in [
                        (
                            mesh.vertices.buffer(),
                            mesh.vertices.start,
                            payload.vertex_bytes(),
                            &mut *vertex,
                        ),
                        (
                            mesh.indices.buffer(),
                            mesh.indices.start,
                            payload.index_bytes(),
                            &mut *index,
                        ),
                    ] {
                        let total = padded(source.len());
                        let size = (remaining - written).min(total - *offset) & !3;
                        if size == 0 {
                            continue;
                        }
                        let end = (*offset + size).min(source.len());
                        if *offset + size > source.len() {
                            let mut tail = vec![0; size];
                            tail[..end - *offset].copy_from_slice(&source[*offset..end]);
                            self.queue
                                .write_buffer(buffer, base + *offset as u64, &tail);
                        } else {
                            self.queue.write_buffer(
                                buffer,
                                base + *offset as u64,
                                &source[*offset..end],
                            );
                        }
                        *offset += size;
                        written += size;
                    }
                    progress.scheduled = *vertex == padded(mesh.source.vertex_byte_len())
                        && *index == padded(mesh.source.index_byte_len());
                    if progress.scheduled {
                        self.geometry.uploaded_meshes += 1;
                        self.state.uploaded_resource_count += 1;
                    }
                }
                Source::Texture {
                    payload, offset, ..
                } => {
                    let payload = payload.as_ref().unwrap();
                    let (bw, bh, block_bytes) =
                        crate::runtime_texture::texture_block_layout(&payload.format).unwrap();
                    let levels = crate::runtime_texture::texture_mip_layout(
                        &payload.format,
                        payload.width,
                        payload.height,
                        payload.mip_count,
                    )
                    .expect("validated mip layout");
                    while *offset < payload.rgba8.len() && written < remaining {
                        let mip = levels
                            .iter()
                            .find(|level| *offset < level.offset + level.byte_len)
                            .expect("upload offset inside payload");
                        let row = mip.width.div_ceil(bw) as usize * block_bytes as usize;
                        let local = *offset - mip.offset;
                        let x = local % row;
                        let size = (remaining - written).min(row - x) / block_bytes as usize
                            * block_bytes as usize;
                        if size == 0 {
                            break;
                        }
                        self.geometry.streaming.texture_write_calls += 1;
                        self.queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &self.textures[handle]._texture,
                                mip_level: mip.level,
                                origin: wgpu::Origin3d {
                                    x: (x / block_bytes as usize) as u32 * bw,
                                    y: (local / row) as u32 * bh,
                                    z: 0,
                                },
                                aspect: wgpu::TextureAspect::All,
                            },
                            &payload.rgba8[*offset..*offset + size],
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(size as u32),
                                rows_per_image: Some(1),
                            },
                            wgpu::Extent3d {
                                width: (size / block_bytes as usize) as u32 * bw,
                                height: bh,
                                depth_or_array_layers: 1,
                            },
                        );
                        *offset += size;
                        written += size;
                    }
                    progress.scheduled = *offset == payload.rgba8.len();
                }
            }
            if written != before {
                changed.push(*handle);
            }
        }
        if written != 0 {
            // The loop's remaining allowance was computed from this same owner.
            self.geometry
                .streaming
                .admit_visual_upload(frame_index, written)?;
            let encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("geometry-upload-completion"),
                });
            let plan = RhiCommandPlan {
                frame_identity: None,
                graph_schedule: None,
                packet_content_version: None,
                frame_index,
                graph_id: "geometry-upload".into(),
                target_kind: "upload".into(),
                commands: vec![].into(),
                diagnostics: vec![],
            };
            self.submit_gpu_frame(encoder, &plan, 0);
            let token = self.last_gpu_submission().unwrap();
            for handle in changed {
                self.geometry
                    .streaming
                    .resources
                    .get_mut(&handle)
                    .unwrap()
                    .token = Some(token);
            }
        }
        self.geometry.streaming.pump_cpu_ns = self
            .geometry
            .streaming
            .pump_cpu_ns
            .saturating_add(pump_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        Ok(written)
    }

    fn refresh_geometry_uploads(&mut self) {
        let completed: Vec<_> = if self.geometry.streaming.pending_resources == 0 {
            Vec::new()
        } else {
            self.geometry
                .streaming
                .resources
                .iter()
                .filter_map(|(h, p)| {
                    (!p.ready
                        && p.scheduled
                        && p.token.is_some_and(|t| {
                            self.submission_status(t) == Some(GpuSubmissionStatus::Completed)
                        }))
                    .then_some(*h)
                })
                .collect()
        };
        for handle in completed {
            self.geometry.streaming.complete_progress(&handle);
        }
        while self
            .geometry
            .streaming
            .batches
            .front()
            .is_some_and(|(t, _, _)| {
                self.submission_status(*t) != Some(GpuSubmissionStatus::Submitted)
            })
        {
            let (token, _, leases) = self.geometry.streaming.batches.pop_front().unwrap();
            if self.submission_status(token) == Some(GpuSubmissionStatus::Completed) {
                for lease in leases {
                    lease.complete();
                }
            }
        }
    }

    pub fn geometry_upload_state(
        &mut self,
        ticket: GeometryUploadTicket,
    ) -> Result<GeometryUploadState, String> {
        if ticket.backend_parts().1 != self.gpu_lifecycle_stats().device_epoch {
            return Err("geometry.stale_upload_ticket".into());
        }
        // Device destruction clears the physical owner and its ticket map. The
        // opaque receipt still identifies that abandoned epoch, never readiness.
        if self.gpu_lifecycle_stats().abandoned {
            return Ok(GeometryUploadState::Abandoned);
        }
        if !self
            .geometry
            .streaming
            .tickets
            .contains_key(&ticket.backend_parts().0)
        {
            return Err("geometry.unknown_upload_ticket".into());
        }
        self.poll_gpu_completion()?;
        self.refresh_geometry_uploads();
        if self.gpu_lifecycle_stats().abandoned {
            return Ok(GeometryUploadState::Abandoned);
        }
        let handles = &self.geometry.streaming.tickets[&ticket.backend_parts().0];
        if handles.iter().all(|h| {
            self.geometry
                .streaming
                .resources
                .get(h)
                .is_none_or(|p| p.ready)
        }) {
            return Ok(GeometryUploadState::Ready);
        }
        if handles.iter().all(|h| {
            self.geometry
                .streaming
                .resources
                .get(h)
                .is_none_or(|p| p.scheduled)
        }) {
            return Ok(GeometryUploadState::Submitted);
        }
        Ok(GeometryUploadState::Uploading)
    }

    pub fn release_geometry_upload(&mut self, ticket: GeometryUploadTicket) -> Result<(), String> {
        if ticket.backend_parts().1 != self.gpu_lifecycle_stats().device_epoch {
            return Err("geometry.stale_upload_ticket".into());
        }
        if self.gpu_lifecycle_stats().abandoned {
            return Ok(());
        }
        let handles = self
            .geometry
            .streaming
            .tickets
            .get(&ticket.backend_parts().0)
            .ok_or("geometry.unknown_upload_ticket")?;
        let retiring: Vec<_> = handles
            .iter()
            .filter(|h| self.geometry.streaming.pins.get(h) == Some(&1))
            .copied()
            .collect();
        let bytes = retiring
            .iter()
            .map(|h| match h.kind {
                RenderResourceKind::MeshBuffer => self.geometry.meshes.get(h).map_or(0, |_| 0),
                RenderResourceKind::Texture => self.textures.get(h).map_or(0, |t| t.byte_len()),
                _ => 0,
            })
            .sum();
        // Bundles own native bindings too; release them through the same
        // completion owner before the last streaming pin retires a resource.
        if !retiring.is_empty() {
            self.invalidate_geometry_bundles()?;
        }
        self.admit_gpu_retirement(
            bytes,
            retiring
                .iter()
                .filter(|h| h.kind == RenderResourceKind::Texture)
                .count()
                + usize::from(
                    retiring
                        .iter()
                        .any(|h| h.kind == RenderResourceKind::MeshBuffer),
                ),
        )?;
        if !retiring.is_empty() {
            // The original plan also owns canonical CPU mesh sources. Its native
            // bundles were retired above; dropping the obsolete proof here leaves
            // actual submitted readers protected by the completion ledger.
            self.geometry.validated_plan = None;
        }
        let mut retired_meshes = Vec::new();
        let handles = self
            .geometry
            .streaming
            .tickets
            .remove(&ticket.backend_parts().0)
            .unwrap();
        for handle in handles {
            let count = self.geometry.streaming.pins.get_mut(&handle).unwrap();
            *count -= 1;
            if *count != 0 {
                continue;
            }
            self.geometry.streaming.pins.remove(&handle);
            self.geometry.streaming.remove_progress(&handle);
            if handle.kind == RenderResourceKind::MeshBuffer {
                if let Some(old) = self.geometry.meshes.remove(&handle) {
                    retired_meshes.push(old);
                }
            } else if handle.kind == RenderResourceKind::Texture {
                self.owned_geometry_textures.remove(&handle);
                if let Some(old) = self.textures.remove(&handle) {
                    let bytes = old.byte_len();
                    self.retire_gpu(old, bytes);
                }
            }
        }
        if !retired_meshes.is_empty() {
            self.retire_gpu(retired_meshes, 0);
        }
        self.geometry.pages.collect_empty();
        Ok(())
    }
}
fn padded(size: usize) -> usize {
    size.saturating_add(3) & !3
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn static_plan_does_not_inherit_the_default_streaming_profile() {
        let mut streaming = GeometryStreaming::default();
        assert_eq!(streaming.budget.resident_bytes, 256 * 1024 * 1024);
        assert_eq!(streaming.plan_resident_budget(), None);
        streaming.tickets.insert(1, vec![]);
        assert_eq!(streaming.plan_resident_budget(), Some(256 * 1024 * 1024));
        streaming.tickets.clear();
        streaming.budget.resident_bytes = 128;
        streaming.budget_explicit = true;
        assert_eq!(streaming.plan_resident_budget(), Some(128));
    }
}

#[cfg(test)]
thread_local! {static PENDING_RESOURCE_QUERIES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
mod pending_work_tests {
    use super::*;
    fn handle(index: u64, generation: u64) -> RenderResourceHandle {
        RenderResourceHandle {
            kind: RenderResourceKind::MeshBuffer,
            index,
            generation,
        }
    }
    fn progress(ready: bool) -> Progress {
        Progress {
            source: Source::Mesh {
                lease: None,
                vertex: 0,
                index: 0,
            },
            identity: String::new(),
            scheduled: ready,
            ready,
            token: None,
        }
    }
    #[test]
    fn pending_count_matches_canonical_progress_through_replacement_ack_and_cancellation() {
        let mut streaming = GeometryStreaming::default();
        let check = |s: &GeometryStreaming| {
            assert_eq!(
                s.pending_resources,
                s.resources.values().filter(|p| !p.ready).count()
            );
            for generation in [1, 2] {
                for i in 0..8 {
                    let h = handle(i, generation);
                    assert_eq!(s.pending(&h), s.resources.get(&h).is_some_and(|p| !p.ready));
                }
            }
        };
        check(&streaming);
        for generation in [1, 2] {
            for i in 0..8 {
                let h = handle(i, generation);
                streaming.insert_progress(h, progress(false));
                check(&streaming);
                // Shared requests leave the same owner progress unchanged.
                streaming.insert_progress(h, progress(false));
                check(&streaming);
                if i % 2 == 0 {
                    streaming.complete_progress(&h);
                    check(&streaming);
                    streaming.complete_progress(&h);
                    check(&streaming);
                }
                if i % 3 == 0 {
                    streaming.insert_progress(h, progress(true));
                    check(&streaming);
                }
                streaming.insert_progress(h, progress(false));
                check(&streaming);
                streaming.remove_progress(&h);
                check(&streaming);
                streaming.remove_progress(&h);
                check(&streaming);
            }
        }
        assert_eq!(streaming.pending_resources, 0);
    }
    #[test]
    fn all_ready_streaming_avoids_per_draw_tree_queries_at_multiple_scales() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let mut streaming = GeometryStreaming::default();
            for i in 0..count {
                streaming.insert_progress(handle(i, 1), progress(true));
            }
            PENDING_RESOURCE_QUERIES.with(|n| n.set(0));
            for _ in 0..8 {
                for i in 0..count {
                    assert!(!streaming.pending(&handle(i, 1)));
                }
            }
            actual.push((count, PENDING_RESOURCE_QUERIES.with(|n| n.get())));
            streaming.insert_progress(handle(1, 2), progress(false));
            assert!(
                streaming.pending(&handle(1, 2)),
                "new generation is not ready"
            );
            assert!(
                !streaming.pending(&handle(1, 1)),
                "old completed generation changed"
            );
            streaming.remove_progress(&handle(1, 2));
            assert!(
                !streaming.pending(&handle(1, 2)),
                "cancelled progress remains pending"
            );
        }
        eprintln!("actual all-ready native progress-tree queries {actual:?}");
        assert!(
            actual.iter().all(|(_, n)| *n == 0),
            "completed resources still cost a tree query per draw: {actual:?}"
        );
    }
}
