use std::collections::BTreeSet;

use crate::engine_rhi::{
    EngineRhiBackend, EngineRhiDrawCall, EngineRhiFrame, RhiBackendDiagnostic, RhiBackendReport,
};
use crate::render_graph::OrderedF32;
use crate::rhi_command_plan::{RhiCommandPlan, RhiDrawPayload};

#[cfg(test)]
thread_local! {
    static GEOMETRY_INTENT_PIPELINE_BUILDS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static GEOMETRY_INTENT_VALUE_VISITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgpuDeviceContext {
    pub backend_name: String,
    pub device_label: String,
    pub queue_label: String,
    pub real_wgpu_enabled: bool,
}

impl WgpuDeviceContext {
    pub fn unavailable() -> Self {
        Self {
            backend_name: "wgpu".to_string(),
            device_label: "unavailable".to_string(),
            queue_label: "unavailable".to_string(),
            real_wgpu_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgpuTargetContext {
    pub target_id: String,
    pub target_kind: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub presented: bool,
}

impl Default for WgpuTargetContext {
    fn default() -> Self {
        Self {
            target_id: "unbound".to_string(),
            target_kind: "unavailable".to_string(),
            width: 0,
            height: 0,
            format: "unknown".to_string(),
            presented: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WgpuPipelineCacheReport {
    pub requested_keys: Vec<String>,
    pub miss_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WgpuResourceRegistryReport {
    pub mesh_refs: Vec<String>,
    pub sprite_refs: Vec<String>,
    pub material_refs: Vec<String>,
    pub ui_overlay_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgpuBackend {
    device_context: WgpuDeviceContext,
    target_context: WgpuTargetContext,
    clear_count: usize,
    draw_count: usize,
    submit_count: usize,
    present_count: usize,
    pipeline_keys: BTreeSet<String>,
    mesh_refs: BTreeSet<String>,
    sprite_refs: BTreeSet<String>,
    material_refs: BTreeSet<String>,
    ui_overlay_count: usize,
    binding_count: usize,
    uploaded_resource_count: usize,
    diagnostics: Vec<RhiBackendDiagnostic>,
}

impl Default for WgpuBackend {
    fn default() -> Self {
        Self::new_unavailable()
    }
}

impl WgpuBackend {
    pub fn new_unavailable() -> Self {
        Self {
            device_context: WgpuDeviceContext::unavailable(),
            target_context: WgpuTargetContext::default(),
            clear_count: 0,
            draw_count: 0,
            submit_count: 0,
            present_count: 0,
            pipeline_keys: BTreeSet::new(),
            mesh_refs: BTreeSet::new(),
            sprite_refs: BTreeSet::new(),
            material_refs: BTreeSet::new(),
            ui_overlay_count: 0,
            binding_count: 0,
            uploaded_resource_count: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn device_context(&self) -> &WgpuDeviceContext {
        &self.device_context
    }

    pub fn pipeline_cache_report(&self) -> WgpuPipelineCacheReport {
        WgpuPipelineCacheReport {
            requested_keys: self.pipeline_keys.iter().cloned().collect(),
            miss_count: if self.device_context.real_wgpu_enabled {
                0
            } else {
                self.pipeline_keys.len()
            },
        }
    }

    pub fn resource_registry_report(&self) -> WgpuResourceRegistryReport {
        WgpuResourceRegistryReport {
            mesh_refs: self.mesh_refs.iter().cloned().collect(),
            sprite_refs: self.sprite_refs.iter().cloned().collect(),
            material_refs: self.material_refs.iter().cloned().collect(),
            ui_overlay_count: self.ui_overlay_count,
        }
    }

    fn record_draw(&mut self, payload: &RhiDrawPayload) {
        self.draw_count += 1;
        self.record_payload(payload);
    }

    #[cfg(feature = "real-wgpu")]
    fn record_reused_geometry_intent(&mut self, payload: &RhiDrawPayload) {
        if payload.geometry_draw().is_some() {
            self.draw_count += 1;
            self.binding_count += 2;
        } else {
            self.record_draw(payload);
        }
    }

    // Record today's CPU intent completely. A prepared pipeline string is only
    // pure interpretation; this does not assert successful native encoding.
    fn record_geometry_metadata(&mut self, draw: &crate::render_geometry::DrawGeometry, key: &str) {
        #[cfg(test)]
        GEOMETRY_INTENT_VALUE_VISITS.with(|n| n.set(n.get() + 1));
        if !self.pipeline_keys.contains(key) {
            self.pipeline_keys.insert(key.to_owned());
        }
        self.binding_count += 2;
        if !self.mesh_refs.contains(&draw.geometry_binding.binding_id) {
            self.mesh_refs
                .insert(draw.geometry_binding.binding_id.clone());
        }
        if !self
            .material_refs
            .contains(&draw.material_binding.binding_id)
        {
            self.material_refs
                .insert(draw.material_binding.binding_id.clone());
        }
    }
    #[cfg(feature = "real-wgpu")]
    fn record_prepared_geometry_intent(
        &mut self,
        draw: &crate::render_geometry::DrawGeometry,
        key: &str,
    ) {
        self.draw_count += 1;
        self.record_geometry_metadata(draw, key);
    }
    fn record_payload(&mut self, payload: &RhiDrawPayload) {
        match payload {
            payload @ (RhiDrawPayload::Geometry { .. } | RhiDrawPayload::GeometryBatch { .. }) => {
                let draw = payload.geometry_draw().unwrap();
                #[cfg(test)]
                GEOMETRY_INTENT_PIPELINE_BUILDS.with(|n| n.set(n.get() + 1));
                let key = draw.pipeline_key(&self.target_context.format);
                self.record_geometry_metadata(draw, key.as_ref());
            }
            RhiDrawPayload::Particles { .. } => {
                self.pipeline_keys.insert("particle-indirect".into());
            }
            RhiDrawPayload::TestGeometry { .. } => {
                self.pipeline_keys
                    .insert("test-geometry.default".to_string());
            }
            RhiDrawPayload::MeshBasic {
                mesh_ref,
                material_ref,
                pipeline_key,
            } => {
                self.pipeline_keys.insert(pipeline_key.clone());
                self.mesh_refs.insert(mesh_ref.clone());
                if let Some(material_ref) = material_ref {
                    self.material_refs.insert(material_ref.clone());
                }
            }
            RhiDrawPayload::SpriteBasic {
                sprite_ref,
                material_ref,
                pipeline_key,
                ..
            } => {
                self.pipeline_keys.insert(pipeline_key.clone());
                self.sprite_refs.insert(sprite_ref.clone());
                if let Some(material_ref) = material_ref {
                    self.material_refs.insert(material_ref.clone());
                }
            }
            RhiDrawPayload::SpriteTextured {
                sprite_ref,
                material_ref,
                pipeline_key,
                binding,
                ..
            } => {
                self.pipeline_keys.insert(pipeline_key.clone());
                self.sprite_refs.insert(sprite_ref.clone());
                if let Some(material_ref) = material_ref {
                    self.material_refs.insert(material_ref.clone());
                }
                if let Some(binding) = binding {
                    self.binding_count += 1;
                    self.uploaded_resource_count += binding.slots.len();
                }
            }
            RhiDrawPayload::UiRetained { draws } => {
                self.pipeline_keys.insert("ui-retained.local".into());
                self.ui_overlay_count += draws.len();
            }
            RhiDrawPayload::UiOverlay {
                item_count,
                pipeline_key,
                ..
            }
            | RhiDrawPayload::UiComposition {
                item_count,
                pipeline_key,
                ..
            } => {
                self.pipeline_keys.insert(pipeline_key.clone());
                self.ui_overlay_count += *item_count;
            }
        }
    }
}

impl EngineRhiBackend for WgpuBackend {
    fn supports_geometry_batches(&self) -> bool {
        true
    }
    fn resolve_color_3d(&mut self, _resolve: &crate::color_pipeline::ColorResolve3D) {
        self.draw_count += 1;
    }
    fn begin_view(
        &mut self,
        _target: &str,
        view: &crate::render_contract::ExecutableRenderView,
        _depth_target: Option<&str>,
    ) {
        self.clear_count += usize::from(view.clear_color.is_some());
    }
    fn reconcile_particles(
        &mut self,
        sources: &[crate::particle_render_contract::ParticleSourceFrame],
    ) {
        if !sources.is_empty() && !self.device_context.real_wgpu_enabled {
            self.diagnostics.push(RhiBackendDiagnostic::error(
                "particle_render.unsupported_backend",
                "GPU particles require real WGPU",
            ));
        }
    }
    fn particle_step(
        &mut self,
        _instance: u64,
        _step: &crate::particle_render_contract::ParticleRenderStep,
    ) {
        if !self.device_context.real_wgpu_enabled {
            self.diagnostics.push(RhiBackendDiagnostic::error(
                "particle_render.unsupported_backend",
                "GPU particle simulation requires real WGPU",
            ));
        }
    }
    fn backend_kind(&self) -> &'static str {
        "wgpu"
    }

    fn begin_frame(&mut self, frame: EngineRhiFrame) {
        self.target_context.target_id = frame.target_id;
        self.target_context.target_kind = "surfaceOrTexture".to_string();
        if !self.device_context.real_wgpu_enabled {
            self.diagnostics.push(RhiBackendDiagnostic::warning(
                "wgpu.real_backend_not_enabled",
                "real-wgpu feature is not enabled; WgpuBackend only records command intent",
            ));
        }
    }

    fn clear(&mut self, _target_id: &str, _color: [OrderedF32; 4]) {
        self.clear_count += 1;
    }

    fn draw(&mut self, draw_call: EngineRhiDrawCall) {
        self.record_draw(&draw_call.payload);
    }

    fn submit(&mut self) {
        self.submit_count += 1;
    }

    fn present(&mut self, _target_id: &str) {
        self.present_count += 1;
        self.target_context.presented = true;
    }

    fn finish_report(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport {
        if !self.device_context.real_wgpu_enabled {
            self.diagnostics.push(RhiBackendDiagnostic::error(
                "backend_unavailable",
                "real WGPU backend is not available in this build",
            ));
        }
        RhiBackendReport {
            backend_kind: self.backend_kind().to_string(),
            frame_index: plan.frame_index,
            target_kind: self.target_context.target_kind.clone(),
            clear_count: self.clear_count,
            draw_count: self.draw_count,
            submit_count: self.submit_count,
            present_count: self.present_count,
            binding_count: self.binding_count,
            uploaded_resource_count: self.uploaded_resource_count,
            reused_resource_count: 0,
            failed_resource_count: if plan.has_errors() { 1 } else { 0 },
            target_hash: if self.target_context.presented {
                format!(
                    "wgpu-intent:{}:{}:{}:{}",
                    self.clear_count, self.draw_count, self.submit_count, self.present_count
                )
            } else {
                "not-presented".to_string()
            },
            diagnostics: self.diagnostics.clone(),
        }
    }
}

#[cfg(feature = "real-wgpu")]
#[cfg_attr(test, path = "wgpu_backend/real")]
pub mod real {
    mod geometry;
    mod lifecycle;
    pub(crate) mod memory;
    mod output;
    mod retained_ui;
    use super::*;
    use crate::font_bundle::{FontBundleRenderMode, RuntimeLoadedFontBundle};
    use crate::game_view_presentation::GameViewRect;
    use crate::render_graph::{RenderDrawVertex, UiVertexStream};
    use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
    use crate::rhi_command_plan::{RhiCommand, RhiDrawKind};
    use crate::runtime_renderer::font_bundle_page_generation_render_handle;
    pub use geometry::{
        GeometryResidency, GeometryUploadBudget, GeometryUploadSnapshot, GeometryUploadState,
        GeometryUploadTicket, RetainedGeometryStats,
    };
    pub use lifecycle::{
        GpuLifecycleStats, GpuSubmissionObservation, GpuSubmissionStatus, GpuSubmissionToken,
    };
    pub use retained_ui::RetainedUiStats;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use wgpu::util::DeviceExt;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct RuntimeVertex {
        position: [f32; 2],
        color: [f32; 4],
        uv: [f32; 2],
    }

    impl RuntimeVertex {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32x2];

        fn layout() -> wgpu::VertexBufferLayout<'static> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &Self::ATTRIBUTES,
            }
        }
    }

    // Engine physical ownership is separate from wgpu's internal reference graph.
    // Explicit destruction at the last engine owner releases native storage even
    // when a backend tracker still has an indirect texture/view reference.
    struct OwnedTexture(wgpu::Texture);
    impl std::ops::Deref for OwnedTexture {
        type Target = wgpu::Texture;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl Drop for OwnedTexture {
        fn drop(&mut self) {
            self.0.destroy();
        }
    }
    #[cfg(test)]
    mod texture_owner_tests {
        use super::*;
        #[test]
        #[ignore = "requires real Vulkan GPU"]
        fn gpu_texture_last_physical_owner_destroys_storage_but_shared_pin_does_not() {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                ..Default::default()
            });
            let adapter =
                pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
            let (device, _queue) =
                pollster::block_on(adapter.request_device(&Default::default())).unwrap();
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("physical-owner-regression"),
                size: wgpu::Extent3d {
                    width: 4,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let native = texture.clone();
            let first = Arc::new(OwnedTexture(texture));
            let neighbor = first.clone();
            drop(first);
            // SAFETY: read-only HAL liveness probe; the guard never escapes.
            assert!(unsafe { native.as_hal::<wgpu::hal::api::Vulkan>() }.is_some());
            drop(neighbor);
            assert!(
                unsafe { native.as_hal::<wgpu::hal::api::Vulkan>() }.is_none(),
                "indirect native references must not keep the final owner's allocation live"
            );
        }
    }
    struct ResidentTexture {
        _texture: Arc<OwnedTexture>,
        _view: wgpu::TextureView,
        _sampler: wgpu::Sampler,
        bind_group: wgpu::BindGroup,
        _width: u32,
        _height: u32,
    }

    impl ResidentTexture {
        fn byte_len(&self) -> u64 {
            let (bw, bh) = self._texture.format().block_dimensions();
            let block_bytes = self
                ._texture
                .format()
                .block_copy_size(None)
                .expect("sampled texture format");
            (0..self._texture.mip_level_count())
                .map(|level| {
                    u64::from((self._width >> level).max(1).div_ceil(bw))
                        * u64::from((self._height >> level).max(1).div_ceil(bh))
                        * u64::from(block_bytes)
                })
                .sum()
        }
    }

    fn uses_standalone_font_page_textures(backend_name: &str) -> bool {
        backend_name.eq_ignore_ascii_case("gl")
    }

    fn surface_content_scissor(rect: GameViewRect) -> Result<[u32; 4], String> {
        let right = rect.x + rect.width;
        let bottom = rect.y + rect.height;
        if !rect.x.is_finite()
            || !rect.y.is_finite()
            || !rect.width.is_finite()
            || !rect.height.is_finite()
            || rect.x < 0.0
            || rect.y < 0.0
            || rect.width <= 0.0
            || rect.height <= 0.0
            || !right.is_finite()
            || !bottom.is_finite()
        {
            return Err("wgpu.surface_content_rect_invalid".to_string());
        }
        let x = rect.x.floor() as u32;
        let y = rect.y.floor() as u32;
        let scissor_right = right.ceil() as u32;
        let scissor_bottom = bottom.ceil() as u32;
        let width = scissor_right.saturating_sub(x);
        let height = scissor_bottom.saturating_sub(y);
        if width == 0 || height == 0 {
            return Err("wgpu.surface_content_rect_empty".to_string());
        }
        Ok([x, y, width, height])
    }

    struct ResidentUiVertices {
        source: UiVertexStream,
        buffer: wgpu::Buffer,
        last_used_epoch: u64,
        variant_eligible: bool,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct UiVertexUploadStats {
        /// All draw-buffer preparation and queue writes (UI and world); excludes GPU/wait.
        pub cpu_draw_buffer_prepare_ns: u64,
        pub full_upload_cold_or_evicted: u64,
        pub full_upload_untrusted_lineage: u64,
        pub uploads: u64,
        pub full_uploads: u64,
        pub partial_uploads: u64,
        pub uploaded_bytes: u64,
        pub partial_uploaded_bytes: u64,
        pub hits: u64,
        pub resident_buffers: usize,
        pub resident_bytes: usize,
    }

    const MAX_UI_VERTEX_BUFFERS: usize = 1024;
    const MAX_UI_VERTEX_BYTES: usize = 32 * 1024 * 1024;
    const MAX_UI_VERTEX_VARIANT_AGE: u64 = 3;
    const MAX_UI_VERTEX_VARIANT_ITEMS: usize = 4_096;
    const MAX_UI_VERTEX_VARIANT_BYTES: usize = 8 * 1024 * 1024;

    fn ui_vertex_variant_is_bounded(item_count: usize, vertex_bytes: usize) -> bool {
        item_count <= MAX_UI_VERTEX_VARIANT_ITEMS && vertex_bytes <= MAX_UI_VERTEX_VARIANT_BYTES
    }

    pub struct RealWgpuBackend {
        ui_vertices: BTreeMap<u64, ResidentUiVertices>,
        pending_draw_buffers: memory::PendingGpuBuffers,
        output_textures: std::collections::BTreeSet<wgpu::Texture>,
        readback_buffers: std::collections::BTreeSet<wgpu::Buffer>,
        retained_ui: retained_ui::RetainedUiGpu,
        ui_residency_epoch: u64,
        ui_upload_stats: UiVertexUploadStats,
        ui_measurement_enabled: bool,
        geometry: geometry::GeometryGpuState,
        lifecycle: lifecycle::GpuLifecycle,
        measurement: Option<crate::gpu_frame_measurement::GpuFrameMeasurement>,
        capturing: bool,
        loading_clear: bool,
        font_bundle_identities: BTreeMap<(String, u64), String>,
        particle_meshes: BTreeMap<RenderResourceHandle, crate::particle_render::ParticleMesh>,
        particle_depth: Option<(u32, u32, wgpu::TextureView)>,
        particle_occluder_pipeline: Option<wgpu::RenderPipeline>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        pipeline: wgpu::RenderPipeline,
        textured_pipeline: wgpu::RenderPipeline,
        bitmap_font_pipeline: wgpu::RenderPipeline,
        msdf_font_pipeline: wgpu::RenderPipeline,
        texture_bind_group_layout: wgpu::BindGroupLayout,
        textures: BTreeMap<RenderResourceHandle, ResidentTexture>,
        particle_effects: BTreeMap<u64, crate::particle_gpu::ParticleGpuEffect>,
        project_particle_sources:
            BTreeMap<u64, crate::particle_render_contract::ParticleSourceFrame>,
        owned_particle_textures: std::collections::BTreeSet<RenderResourceHandle>,
        owned_geometry_textures: std::collections::BTreeSet<RenderResourceHandle>,
        owned_particle_meshes: std::collections::BTreeSet<RenderResourceHandle>,
        state: WgpuBackend,
    }

    impl RealWgpuBackend {
        pub fn set_ui_measurement_enabled(&mut self, enabled: bool) {
            self.ui_measurement_enabled = enabled;
        }
        pub fn retained_ui_stats(&self) -> RetainedUiStats {
            self.retained_ui.stats
        }

        pub fn ui_vertex_upload_stats(&self) -> UiVertexUploadStats {
            self.ui_upload_stats
        }

        // Simulate the existing bounded UI vertex cache in command order. This
        // borrows CPU streams only; it neither uploads nor advances a UI clock.
        fn ui_frame_upload_requirements(&self, plan: &RhiCommandPlan) -> Result<usize, String> {
            let epoch = self.ui_residency_epoch.saturating_add(1);
            let active: std::collections::BTreeSet<_> = plan
                .commands
                .iter()
                .filter_map(|c| {
                    if let RhiCommand::Draw {
                        payload: RhiDrawPayload::UiComposition { vertices, .. },
                        ..
                    } = c
                    {
                        Some(vertices.stream_id())
                    } else {
                        None
                    }
                })
                .collect();
            let mut residents: BTreeMap<_, _> = self
                .ui_vertices
                .iter()
                .filter_map(|(key, r)| {
                    let keep = !active.is_empty()
                        && (active.contains(key)
                            || (r.variant_eligible
                                && epoch.saturating_sub(r.last_used_epoch)
                                    <= MAX_UI_VERTEX_VARIANT_AGE));
                    keep.then_some((
                        *key,
                        (
                            &r.source,
                            r.buffer.size() as usize,
                            r.last_used_epoch,
                            r.variant_eligible,
                        ),
                    ))
                })
                .collect();
            let mut resident_bytes = residents.values().map(|r| r.1).sum::<usize>();
            let mut bytes = self.retained_ui.planned_backing_bytes(plan)? as usize;
            for command in &plan.commands {
                let RhiCommand::Draw {
                    draw_kind,
                    vertex_count,
                    payload,
                    ..
                } = command
                else {
                    continue;
                };
                if let RhiDrawPayload::UiRetained { draws } = payload {
                    bytes = bytes.saturating_add(draws.len().saturating_mul(64));
                    continue;
                }
                let RhiDrawPayload::UiComposition {
                    vertices: source,
                    item_count,
                    ..
                } = payload
                else {
                    bytes = bytes.saturating_add(
                        Self::vertices_for_draw(*draw_kind, *vertex_count, payload)
                            .len()
                            .saturating_mul(std::mem::size_of::<RuntimeVertex>()),
                    );
                    continue;
                };
                let source_bytes = source.len() * std::mem::size_of::<RuntimeVertex>();
                let eligible = ui_vertex_variant_is_bounded(*item_count, source_bytes);
                if let Some(resident) = residents.get_mut(&source.stream_id()) {
                    resident.2 = epoch;
                    resident.3 = eligible;
                    if source.shares_data_with(resident.0) {
                        continue;
                    }
                    if let Some(ranges) = source.patch_ranges_from(resident.0) {
                        bytes = bytes.saturating_add(
                            ranges
                                .iter()
                                .map(|range| {
                                    range.vertex_count() * std::mem::size_of::<RuntimeVertex>()
                                })
                                .sum::<usize>(),
                        );
                        resident.0 = source;
                        continue;
                    }
                }
                if source.is_empty() {
                    continue;
                }
                bytes = bytes.saturating_add(source_bytes);
                if let Some(old) = residents.remove(&source.stream_id()) {
                    resident_bytes = resident_bytes.saturating_sub(old.1);
                }
                while residents.len() >= MAX_UI_VERTEX_BUFFERS
                    || resident_bytes + source_bytes > MAX_UI_VERTEX_BYTES
                {
                    let victim = residents
                        .iter()
                        .filter(|(key, _)| !active.contains(key))
                        .min_by_key(|(_, r)| r.2)
                        .map(|(key, _)| *key);
                    let Some(victim) = victim else { break };
                    resident_bytes =
                        resident_bytes.saturating_sub(residents.remove(&victim).unwrap().1);
                }
                if residents.len() < MAX_UI_VERTEX_BUFFERS
                    && resident_bytes + source_bytes <= MAX_UI_VERTEX_BYTES
                {
                    resident_bytes += source_bytes;
                    residents.insert(source.stream_id(), (source, source_bytes, epoch, eligible));
                }
            }
            Ok(bytes)
        }

        fn prepare_draw_buffers(
            &mut self,
            plan: &RhiCommandPlan,
        ) -> Result<Vec<Option<(wgpu::Buffer, u32)>>, String> {
            #[cfg(test)]
            UI_PREPARATION_CALLS.with(|n| n.set(n.get() + 1));
            let started = self
                .ui_measurement_enabled
                .then(crate::platform_time::Instant::now);
            self.ui_residency_epoch = self.ui_residency_epoch.saturating_add(1);
            let residency_epoch = self.ui_residency_epoch;
            let active = plan
                .commands
                .iter()
                .filter_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::UiComposition { vertices, .. },
                        ..
                    } => Some(vertices.stream_id()),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>();
            let unused = self
                .ui_vertices
                .iter()
                .filter(|(key, resident)| {
                    active.is_empty()
                        || (!active.contains(key)
                            && !(resident.variant_eligible
                                && residency_epoch.saturating_sub(resident.last_used_epoch)
                                    <= MAX_UI_VERTEX_VARIANT_AGE))
                })
                .map(|(key, _)| *key)
                .collect::<Vec<_>>();
            for key in unused {
                self.remove_ui_vertices(key)?;
            }
            let mut resident_bytes = self
                .ui_vertices
                .values()
                .map(|v| v.source.len() * std::mem::size_of::<RuntimeVertex>())
                .sum::<usize>();
            let result = plan
                .commands
                .iter()
                .map(|command| {
                    let RhiCommand::Draw {
                        draw_kind,
                        vertex_count,
                        payload,
                        ..
                    } = command
                    else {
                        return Ok(None);
                    };
                    let ui_source = match payload {
                        RhiDrawPayload::UiComposition {
                            vertices,
                            item_count,
                            ..
                        } => Some((vertices, *item_count)),
                        _ => None,
                    };
                    if let Some((source, item_count)) = ui_source {
                        let source_bytes = source.len() * std::mem::size_of::<RuntimeVertex>();
                        let variant_eligible =
                            ui_vertex_variant_is_bounded(item_count, source_bytes);
                        if let Some(resident) = self.ui_vertices.get_mut(&source.stream_id()) {
                            resident.last_used_epoch = residency_epoch;
                            resident.variant_eligible = variant_eligible;
                            if source.shares_data_with(&resident.source) {
                                self.ui_upload_stats.hits += 1;
                                self.pending_draw_buffers
                                    .insert(resident.buffer.clone(), "renderer");
                                return Ok(Some((resident.buffer.clone(), source.len() as u32)));
                            }
                            if let Some(ranges) = source.patch_ranges_from(&resident.source) {
                                let bytes = ranges
                                    .iter()
                                    .map(|range| {
                                        range.vertex_count() * std::mem::size_of::<RuntimeVertex>()
                                    })
                                    .sum();
                                self.geometry.admit_visual_upload(plan.frame_index, bytes)?;
                                for range in ranges {
                                    let vertices = source
                                        .vertices_in_range(*range)
                                        .map(Self::runtime_vertex)
                                        .collect::<Vec<_>>();
                                    let bytes = bytemuck::cast_slice(&vertices);
                                    let byte_offset =
                                        range.first_vertex() * std::mem::size_of::<RuntimeVertex>();
                                    self.queue.write_buffer(
                                        &resident.buffer,
                                        byte_offset as wgpu::BufferAddress,
                                        bytes,
                                    );
                                    self.ui_upload_stats.uploads += 1;
                                    self.ui_upload_stats.partial_uploads += 1;
                                    self.ui_upload_stats.uploaded_bytes += bytes.len() as u64;
                                    self.ui_upload_stats.partial_uploaded_bytes +=
                                        bytes.len() as u64;
                                }
                                resident.source = source.clone();
                                self.ui_upload_stats.hits += 1;
                                self.pending_draw_buffers
                                    .insert(resident.buffer.clone(), "renderer");
                                return Ok(Some((resident.buffer.clone(), source.len() as u32)));
                            }
                        }
                    }
                    let vertices = Self::vertices_for_draw(*draw_kind, *vertex_count, payload);
                    if vertices.is_empty() {
                        return Ok(None);
                    }
                    let bytes = bytemuck::cast_slice(&vertices);
                    self.admit_geometry_backing(bytes.len() as u64)?;
                    self.admit_gpu_retirement(bytes.len() as u64, 1)?;
                    self.geometry
                        .admit_visual_upload(plan.frame_index, bytes.len())?;
                    let buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("runtime-draw-vertices"),
                                contents: bytes,
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            });
                    self.pending_draw_buffers.insert(buffer.clone(), "renderer");
                    if let Some((source, item_count)) = ui_source {
                        if self.ui_vertices.contains_key(&source.stream_id()) {
                            self.ui_upload_stats.full_upload_untrusted_lineage += 1;
                        } else {
                            self.ui_upload_stats.full_upload_cold_or_evicted += 1;
                        }
                        self.ui_upload_stats.uploads += 1;
                        self.ui_upload_stats.full_uploads += 1;
                        self.ui_upload_stats.uploaded_bytes += bytes.len() as u64;
                        if let Some(replaced) = self.ui_vertices.get(&source.stream_id()) {
                            resident_bytes =
                                resident_bytes.saturating_sub(replaced.buffer.size() as usize);
                            self.remove_ui_vertices(source.stream_id())?;
                        }
                        while (self.ui_vertices.len() >= MAX_UI_VERTEX_BUFFERS
                            || resident_bytes + bytes.len() > MAX_UI_VERTEX_BYTES)
                            && self
                                .ui_vertices
                                .iter()
                                .any(|(key, _)| !active.contains(key))
                        {
                            let victim = self
                                .ui_vertices
                                .iter()
                                .filter(|(key, _)| !active.contains(key))
                                .min_by_key(|(_, resident)| resident.last_used_epoch)
                                .map(|(key, _)| *key)
                                .expect("inactive resident UI buffer");
                            resident_bytes = resident_bytes
                                .saturating_sub(self.ui_vertices[&victim].buffer.size() as usize);
                            self.remove_ui_vertices(victim)?;
                        }
                        if self.ui_vertices.len() < MAX_UI_VERTEX_BUFFERS
                            && resident_bytes + bytes.len() <= MAX_UI_VERTEX_BYTES
                        {
                            resident_bytes += bytes.len();
                            self.ui_vertices.insert(
                                source.stream_id(),
                                ResidentUiVertices {
                                    source: source.clone(),
                                    buffer: buffer.clone(),
                                    last_used_epoch: residency_epoch,
                                    variant_eligible: ui_vertex_variant_is_bounded(
                                        item_count,
                                        bytes.len(),
                                    ),
                                },
                            );
                        }
                    }
                    Ok(Some((buffer, vertices.len() as u32)))
                })
                .collect::<Result<Vec<_>, String>>()?;
            self.ui_upload_stats.resident_buffers = self.ui_vertices.len();
            self.ui_upload_stats.resident_bytes = resident_bytes;
            self.ui_upload_stats.cpu_draw_buffer_prepare_ns += crate::aui::aui_elapsed_ns(started);
            self.admit_gpu_retirement(0, 0)?;
            Ok(result)
        }

        fn remove_ui_vertices(&mut self, key: u64) -> Result<(), String> {
            let Some(old) = self.ui_vertices.get(&key) else {
                return Ok(());
            };
            let pending = self.pending_draw_buffers.contains_key(&old.buffer);
            if !pending {
                self.admit_gpu_retirement(old.buffer.size(), 1)?;
            }
            let old = self.ui_vertices.remove(&key).unwrap();
            if !pending {
                let bytes = old.buffer.size();
                self.retire_gpu(old.buffer, bytes);
            }
            Ok(())
        }

        fn pending_draw_retirement(&self) -> (u64, usize) {
            self.pending_draw_buffers
                .keys()
                .filter(|buffer| !self.ui_vertices.values().any(|v| &v.buffer == *buffer))
                .fold((0, 0), |(bytes, count), b| (bytes + b.size(), count + 1))
        }

        fn retire_pending_draw_buffers(&mut self) {
            for (buffer, owner) in std::mem::take(&mut self.pending_draw_buffers) {
                if !self.ui_vertices.values().any(|v| v.buffer == buffer) {
                    let bytes = buffer.size();
                    self.retire_gpu((buffer, owner), bytes);
                }
            }
        }

        fn prepare_retained_ui(
            &mut self,
            plan: &RhiCommandPlan,
        ) -> Result<Vec<Vec<retained_ui::Prepared>>, String> {
            #[cfg(test)]
            UI_PREPARATION_CALLS.with(|n| n.set(n.get() + 1));
            let backing = self.retained_ui.planned_backing_bytes(plan)?;
            self.admit_geometry_backing(backing)?;
            let parameters: usize = plan
                .commands
                .iter()
                .map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::UiRetained { draws },
                        ..
                    } => draws.len() * 64,
                    _ => 0,
                })
                .sum();
            self.geometry
                .admit_visual_upload(plan.frame_index, backing as usize + parameters)?;
            let remaining = lifecycle::MAX_RETIRING_BYTES.saturating_sub(
                self.gpu_lifecycle_stats()
                    .retiring_bytes
                    .saturating_sub(self.retained_ui.stats.retiring_bytes as u64)
                    + self.pending_draw_retirement().0,
            );
            let remaining_objects = lifecycle::MAX_RETIRING_OBJECTS.saturating_sub(
                self.gpu_lifecycle_stats()
                    .retiring_objects
                    .saturating_sub(self.retained_ui.retiring_count())
                    + self.pending_draw_retirement().1,
            );
            self.retained_ui.prepare(
                &self.device,
                &self.queue,
                plan,
                remaining as usize,
                remaining_objects,
            )
        }

        pub fn device_context(&self) -> &WgpuDeviceContext {
            self.state.device_context()
        }

        pub fn from_device_queue(
            device: wgpu::Device,
            queue: wgpu::Queue,
            format: wgpu::TextureFormat,
            width: u32,
            height: u32,
            backend_name: impl Into<String>,
        ) -> Self {
            let pipeline = create_pipeline(&device, format);
            let texture_bind_group_layout = create_texture_bind_group_layout(&device);
            let geometry = geometry::GeometryGpuState::new(&device, format);
            let textured_pipeline =
                create_textured_pipeline(&device, format, &texture_bind_group_layout);
            let bitmap_font_pipeline =
                create_font_pipeline(&device, format, &texture_bind_group_layout, false);
            let msdf_font_pipeline =
                create_font_pipeline(&device, format, &texture_bind_group_layout, true);
            let mut state = WgpuBackend::new_unavailable();
            state.device_context = WgpuDeviceContext {
                backend_name: backend_name.into(),
                device_label: "runtime-wgpu-device".to_string(),
                queue_label: "runtime-wgpu-queue".to_string(),
                real_wgpu_enabled: true,
            };
            state.target_context = WgpuTargetContext {
                target_id: "surface".to_string(),
                target_kind: "surface".to_string(),
                width,
                height,
                format: format!("{format:?}"),
                presented: false,
            };

            Self {
                ui_vertices: BTreeMap::new(),
                pending_draw_buffers: Default::default(),
                output_textures: Default::default(),
                readback_buffers: Default::default(),
                retained_ui: retained_ui::RetainedUiGpu::new(&device, format),
                ui_residency_epoch: 0,
                ui_upload_stats: Default::default(),
                ui_measurement_enabled: false,
                geometry,
                lifecycle: lifecycle::GpuLifecycle::new(&device),
                measurement: None,
                capturing: false,
                loading_clear: false,
                font_bundle_identities: BTreeMap::new(),
                device,
                queue,
                format,
                width,
                height,
                pipeline,
                textured_pipeline,
                bitmap_font_pipeline,
                msdf_font_pipeline,
                texture_bind_group_layout,
                textures: BTreeMap::new(),
                particle_effects: BTreeMap::new(),
                project_particle_sources: BTreeMap::new(),
                owned_particle_textures: Default::default(),
                owned_geometry_textures: Default::default(),
                owned_particle_meshes: Default::default(),
                particle_meshes: BTreeMap::new(),
                particle_depth: None,
                particle_occluder_pipeline: None,
                state,
            }
        }

        pub fn from_shared_device_queue(
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
            width: u32,
            height: u32,
            backend_name: impl Into<String>,
        ) -> Self {
            Self::from_device_queue(
                device.clone(),
                queue.clone(),
                format,
                width,
                height,
                backend_name,
            )
        }

        pub fn enable_frame_measurement(
            &mut self,
            info: &wgpu::AdapterInfo,
            warmup: u64,
            samples: u64,
        ) -> Result<(), String> {
            let capacity = warmup
                .checked_add(samples)
                .and_then(|n| n.checked_add(16))
                .filter(|n| *n <= 262_144)
                .ok_or("gpu.observation_capacity_limit_1_262144")?;
            self.set_submission_capture_capacity(capacity as usize)?;
            self.measurement = Some(crate::gpu_frame_measurement::GpuFrameMeasurement::new(
                &self.device,
                &self.queue,
                info,
                warmup,
                samples,
            )?);
            self.set_submission_capture_enabled(true);
            Ok(())
        }

        pub fn finish_frame_measurement(
            &mut self,
        ) -> Option<crate::windowed_player::WindowedPlayerGpuPerformance> {
            let mut report = self.measurement.take()?.finish(&self.device, &self.queue);
            let _ = self.poll_gpu_completion();
            report.submissions = self.submission_observations();
            report.submission_observations_omitted =
                self.gpu_lifecycle_stats().omitted_observations;
            Some(report)
        }

        pub fn new_offscreen(width: u32, height: u32) -> Result<Self, String> {
            Self::new_offscreen_with_backends(width, height, wgpu::Backends::PRIMARY)
        }
        pub fn new_offscreen_measured(
            width: u32,
            height: u32,
            samples: u64,
        ) -> Result<Self, String> {
            Self::new_offscreen_options(width, height, wgpu::Backends::PRIMARY, Some(samples))
        }

        fn new_offscreen_with_backends(
            width: u32,
            height: u32,
            backends: wgpu::Backends,
        ) -> Result<Self, String> {
            Self::new_offscreen_options(width, height, backends, None)
        }
        fn new_offscreen_options(
            width: u32,
            height: u32,
            backends: wgpu::Backends,
            measurement_samples: Option<u64>,
        ) -> Result<Self, String> {
            if width == 0 || height == 0 {
                return Err("wgpu_backend.zero_sized_target".to_string());
            }
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends,
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                backend_options: wgpu::BackendOptions::default(),
            });
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                }))
                .map_err(|error| format!("wgpu_backend.request_adapter_failed:{error}"))?;
            let backend_name = format!("{:?}", adapter.get_info().backend);
            #[cfg(test)]
            if std::env::var_os("PARTICLE_RENDER_EVIDENCE_DIR").is_some() {
                eprintln!("343 GPU: {:?}", adapter.get_info());
            }
            let required_limits = crate::particle_gpu::renderer_device_limits(adapter.limits());
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("runtime-wgpu-device"),
                    required_features: crate::gpu_frame_measurement::features(
                        adapter.features(),
                        measurement_samples.is_some(),
                    ) | (adapter.features()
                        & (wgpu::Features::TEXTURE_COMPRESSION_BC
                            | wgpu::Features::INDIRECT_FIRST_INSTANCE)),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                }))
                .map_err(|error| format!("wgpu_backend.request_device_failed:{error}"))?;
            let format = wgpu::TextureFormat::Rgba8Unorm;
            let mut backend =
                Self::from_device_queue(device, queue, format, width, height, backend_name);
            if let Some(samples) = measurement_samples {
                backend.enable_frame_measurement(&adapter.get_info(), 0, samples)?;
            }
            backend.state.target_context.target_id = "offscreen".to_string();
            backend.state.target_context.target_kind = "offscreenTexture".to_string();
            Ok(backend)
        }

        pub fn reconcile_project_particles(
            &mut self,
            sources: &[crate::particle_render_contract::ParticleSourceFrame],
        ) -> Result<(), String> {
            use crate::{
                particle_effect::ParticleValue, runtime_particles::ParticlePreparedAssets,
            };
            let ids = sources
                .iter()
                .map(|s| s.instance)
                .collect::<std::collections::BTreeSet<_>>();
            if ids.len() != sources.len() {
                return Err("particle_render.duplicate_instance".into());
            }
            self.admit_gpu_frame()?;
            let mut prepared = Vec::new();
            let mut pending_bytes = 0u64;
            let mut pending_meshes = std::collections::BTreeSet::new();
            let mut pending_textures = std::collections::BTreeSet::new();
            let mut allocated_meshes = std::collections::BTreeSet::new();
            for source in sources {
                if !source
                    .simulation_origin_bits
                    .iter()
                    .all(|v| f64::from_bits(*v).is_finite())
                {
                    return Err("particle_render.nonfinite_simulation_origin".into());
                }
                if !source.step.delta_seconds.to_f32().is_finite()
                    || source.step.delta_seconds.to_f32() < 0.0
                    || !source.step.origin.iter().all(|v| v.to_f32().is_finite())
                {
                    return Err("particle_gpu.invalid_step".into());
                }
                source.step.emitter_affine.inverse()?;
                if let Some(old) = self.project_particle_sources.get(&source.instance) {
                    if source.simulation_origin_bits != old.simulation_origin_bits {
                        return Err("particle_render.simulation_origin_changed".into());
                    }
                    if source.epoch < old.epoch || source.step.step_id < old.step.step_id {
                        return Err("particle_render.stale_projection".into());
                    }
                    if !std::sync::Arc::ptr_eq(&old.assets, &source.assets)
                        && old.assets != source.assets
                    {
                        return Err("particle_render.instance_asset_changed".into());
                    }
                    let parameters: std::collections::BTreeMap<String, ParticleValue> =
                        serde_json::from_str(&source.parameters).map_err(|e| e.to_string())?;
                    // Validate all sources before any clear/step or parameter upload.
                    let effect = &self.particle_effects[&source.instance];
                    effect.validate_parameters(&parameters)?;
                    effect.validate_affine(&source.step.emitter_affine)?;
                    continue;
                }
                let assets: ParticlePreparedAssets = serde_json::from_str(&source.assets)
                    .map_err(|e| format!("particle_render.assets_invalid:{e}"))?;
                let count = assets.effect.description.emitters.len();
                if [
                    assets.textures.len(),
                    assets.meshes.len(),
                    assets.tints.len(),
                    source.textures.len(),
                    source.meshes.len(),
                ]
                .iter()
                .any(|n| *n != count)
                {
                    return Err("particle_render.binding_count".into());
                }
                // Charge the entire prepared transaction before any GPU allocation.
                // Staged effects are not in the installed owner maps yet.
                pending_bytes = pending_bytes
                    .checked_add(
                        crate::particle_gpu::ParticleGpuEffect::planned_backing_bytes(
                            &assets.effect,
                        )?,
                    )
                    .ok_or("geometry.visual_resident_budget")?;
                for i in 0..count {
                    if let Some(handle) = source.meshes[i] {
                        if !self.particle_meshes.contains_key(&handle)
                            && pending_meshes.insert(handle)
                        {
                            let mesh = assets.meshes[i]
                                .as_ref()
                                .ok_or("particle_render.binding_mismatch")?;
                            pending_bytes = pending_bytes
                                .checked_add(mesh.indices.len() as u64 * 32)
                                .ok_or("geometry.visual_resident_budget")?;
                        }
                    }
                    if let Some(handle) = source.textures[i] {
                        if !self.textures.contains_key(&handle) && pending_textures.insert(handle) {
                            let texture = assets.textures[i]
                                .as_ref()
                                .ok_or("particle_render.binding_mismatch")?;
                            pending_bytes = pending_bytes
                                .checked_add(texture.rgba8.len() as u64)
                                .ok_or("geometry.visual_resident_budget")?;
                        }
                    }
                }
                self.admit_geometry_backing(pending_bytes)?;
                let mesh_bytes: usize = assets
                    .meshes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        source.meshes[*i].is_some_and(|h| {
                            !self.particle_meshes.contains_key(&h) && !allocated_meshes.contains(&h)
                        })
                    })
                    .filter_map(|(_, m)| m.as_ref())
                    .map(|m| m.indices.len() * 32)
                    .sum();
                self.geometry.admit_visual_upload(
                    0,
                    crate::particle_gpu::ParticleGpuEffect::initial_upload_bytes(&assets.effect)
                        + mesh_bytes,
                )?;
                let mut effect =
                    crate::particle_gpu::ParticleGpuEffect::new(&self.device, &assets.effect)?;
                effect.validate_affine(&source.step.emitter_affine)?;
                effect.enable_render(&self.device, self.format)?;
                let parameters: std::collections::BTreeMap<String, ParticleValue> =
                    serde_json::from_str(&source.parameters).map_err(|e| e.to_string())?;
                effect.validate_parameters(&parameters)?;
                let mut meshes = Vec::new();
                for i in 0..count {
                    let render = effect.emitters[i].render.as_mut().unwrap();
                    if !assets.tints[i]
                        .iter()
                        .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0)
                    {
                        return Err("particle_render.material_tint".into());
                    }
                    render.material_tint = assets.tints[i];
                    render.material_resolved = true;
                    render.validate_assets(
                        assets.textures[i].is_some(),
                        assets.meshes[i].is_some(),
                    )?;
                    if source.textures[i].is_some() != assets.textures[i].is_some()
                        || source.meshes[i].is_some() != assets.meshes[i].is_some()
                    {
                        return Err("particle_render.binding_mismatch".into());
                    }
                    if let Some(t) = &assets.textures[i] {
                        if t.width == 0
                            || t.height == 0
                            || u64::from(t.width) * u64::from(t.height) * 4 != t.rgba8.len() as u64
                            || !matches!(t.format.as_str(), "rgba8Unorm" | "rgba8UnormSrgb")
                        {
                            return Err("particle_render.texture_invalid".into());
                        }
                    }
                    meshes.push(
                        assets.meshes[i]
                            .as_ref()
                            .filter(|_| {
                                source.meshes[i].is_some_and(|h| {
                                    !self.particle_meshes.contains_key(&h)
                                        && allocated_meshes.insert(h)
                                })
                            })
                            .map(|m| {
                                crate::particle_render::ParticleMesh::new(
                                    &self.device,
                                    &m.positions,
                                    &m.uvs,
                                    &m.indices,
                                )
                            })
                            .transpose()?,
                    );
                }
                prepared.push((source, assets, effect, meshes));
            }
            let textures = sources
                .iter()
                .flat_map(|s| s.textures.iter().flatten().copied())
                .collect::<std::collections::BTreeSet<_>>();
            let meshes = sources
                .iter()
                .flat_map(|s| s.meshes.iter().flatten().copied())
                .collect::<std::collections::BTreeSet<_>>();
            let retiring_effects = self
                .project_particle_sources
                .keys()
                .filter(|id| !ids.contains(id))
                .filter_map(|id| self.particle_effects.get(id));
            let retiring_textures = self
                .owned_particle_textures
                .difference(&textures)
                .filter(|h| !self.geometry.streaming_texture_pinned(h))
                .filter_map(|h| self.textures.get(h));
            let retiring_meshes = self
                .owned_particle_meshes
                .difference(&meshes)
                .filter_map(|h| self.particle_meshes.get(h));
            let retire_sizes = retiring_effects
                .map(|e| e.buffer_bytes() + e.emitters.len() as u64 * 4)
                .chain(retiring_textures.map(|t| t.byte_len()))
                .chain(retiring_meshes.map(|m| m.buffer.size()))
                .collect::<Vec<_>>();
            self.admit_gpu_retirement(retire_sizes.iter().sum(), retire_sizes.len())?;
            for (source, assets, effect, meshes) in prepared {
                for (i, mesh) in meshes.into_iter().enumerate() {
                    if let Some(texture) = &assets.textures[i] {
                        let handle = source.textures[i].unwrap();
                        if !self.textures.contains_key(&handle) {
                            self.register_cooked_texture(handle, texture)?;
                            self.owned_particle_textures.insert(handle);
                        }
                    }
                    if let Some(mesh) = mesh {
                        let handle = source.meshes[i].unwrap();
                        if !self.particle_meshes.contains_key(&handle) {
                            self.particle_meshes.insert(handle, mesh);
                            self.owned_particle_meshes.insert(handle);
                        }
                    }
                }
                self.particle_effects.insert(source.instance, effect);
            }
            for source in sources {
                let old = self.project_particle_sources.get(&source.instance);
                let restart = old.is_some_and(|old| old.epoch != source.epoch);
                let parameters_changed = old.is_none_or(|old| {
                    !std::sync::Arc::ptr_eq(&old.parameters, &source.parameters)
                        && old.parameters != source.parameters
                });
                if restart {
                    self.clear_particle_effect(source.instance)?;
                }
                if parameters_changed {
                    let values =
                        serde_json::from_str(&source.parameters).map_err(|e| e.to_string())?;
                    self.particle_effects[&source.instance].validate_parameters(&values)?;
                    let bytes = self.particle_effects[&source.instance].parameter_upload_bytes();
                    self.geometry.admit_visual_upload(0, bytes)?;
                    self.particle_effects[&source.instance].set_parameters(&self.queue, &values)?;
                }
            }
            let retired = self
                .project_particle_sources
                .keys()
                .filter(|id| !ids.contains(id))
                .copied()
                .collect::<Vec<_>>();
            for id in retired {
                self.remove_particle_effect(id)?;
            }
            self.project_particle_sources =
                sources.iter().map(|s| (s.instance, s.clone())).collect();
            let textures = sources
                .iter()
                .flat_map(|s| s.textures.iter().flatten().copied())
                .collect::<std::collections::BTreeSet<_>>();
            let meshes = sources
                .iter()
                .flat_map(|s| s.meshes.iter().flatten().copied())
                .collect::<std::collections::BTreeSet<_>>();
            for handle in self
                .owned_particle_textures
                .difference(&textures)
                .copied()
                .collect::<Vec<_>>()
            {
                // A geometry ticket may share a particle-owned texture. Keep its
                // original owner until both consumers release it.
                if self.geometry.streaming_texture_pinned(&handle) {
                    continue;
                }
                self.invalidate_geometry_texture_bundles(&handle)?;
                if let Some(texture) = self.textures.remove(&handle) {
                    let bytes = texture.byte_len();
                    self.retire_gpu(texture, bytes);
                }
                self.owned_particle_textures.remove(&handle);
            }
            for handle in self
                .owned_particle_meshes
                .difference(&meshes)
                .copied()
                .collect::<Vec<_>>()
            {
                if let Some(mesh) = self.particle_meshes.remove(&handle) {
                    let bytes = mesh.buffer.size();
                    self.retire_gpu(mesh, bytes);
                }
                self.owned_particle_meshes.remove(&handle);
            }
            Ok(())
        }

        pub fn install_particle_effect(
            &mut self,
            instance: u64,
            cooked: &crate::particle_effect::CookedParticleEffect,
        ) -> Result<(), String> {
            if self.particle_effects.contains_key(&instance) {
                return Err("particle_gpu.instance_already_exists".into());
            }
            self.admit_gpu_frame()?;
            self.admit_geometry_backing(
                crate::particle_gpu::ParticleGpuEffect::planned_backing_bytes(cooked)?,
            )?;
            self.geometry.admit_visual_upload(
                0,
                crate::particle_gpu::ParticleGpuEffect::initial_upload_bytes(cooked),
            )?;
            let mut effect = crate::particle_gpu::ParticleGpuEffect::new(&self.device, cooked)?;
            effect.enable_render(&self.device, self.format)?;
            self.particle_effects.insert(instance, effect);
            Ok(())
        }

        pub fn simulate_particle_effect(
            &mut self,
            instance: u64,
            step: crate::particle_gpu::ParticleGpuStep,
        ) -> Result<crate::particle_gpu::ParticleGpuStepReport, String> {
            self.admit_gpu_frame()?;
            let bytes = self
                .particle_effects
                .get(&instance)
                .ok_or("particle_gpu.instance_missing")?
                .step_upload_bytes(step);
            let objects = self.particle_effects[&instance].step_upload_objects(step);
            self.admit_geometry_backing(bytes as u64)?;
            self.admit_gpu_retirement(bytes as u64, objects)?;
            self.geometry.admit_visual_upload(step.step_id, bytes)?;
            let effect = self
                .particle_effects
                .get_mut(&instance)
                .ok_or("particle_gpu.instance_missing")?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("runtime-particle-simulation"),
                });
            let report = effect.encode_step_retained(
                &self.device,
                &mut encoder,
                step,
                &mut self.pending_draw_buffers,
            )?;
            if report.substeps > 0 {
                self.submit_resource_maintenance(encoder);
            }
            Ok(report)
        }

        pub fn clear_particle_effect(&mut self, instance: u64) -> Result<(), String> {
            self.admit_gpu_frame()?;
            let effect = self
                .particle_effects
                .get_mut(&instance)
                .ok_or("particle_gpu.instance_missing")?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("runtime-particle-clear"),
                });
            effect.encode_clear(&mut encoder);
            self.submit_resource_maintenance(encoder);
            Ok(())
        }

        fn submit_resource_maintenance(&mut self, encoder: wgpu::CommandEncoder) {
            let plan = RhiCommandPlan {
                frame_identity: None,
                graph_schedule: None,
                packet_content_version: None,
                frame_index: 0,
                graph_id: "resource-maintenance".into(),
                target_kind: "offscreenTexture".into(),
                commands: vec![].into(),
                diagnostics: vec![],
            };
            self.submit_gpu_frame(encoder, &plan, 0);
            self.retire_pending_draw_buffers();
        }

        pub fn remove_particle_effect(&mut self, instance: u64) -> Result<bool, String> {
            self.poll_gpu_completion()?;
            let Some(effect) = self.particle_effects.get(&instance) else {
                return Ok(false);
            };
            let bytes = effect.buffer_bytes() + effect.emitters.len() as u64 * 4;
            self.admit_gpu_retirement(bytes, 1)?;
            let effect = self.particle_effects.remove(&instance).unwrap();
            self.retire_gpu(effect, bytes);
            Ok(true)
        }

        pub fn particle_buffer_bytes(&self, instance: u64) -> Option<u64> {
            self.particle_effects
                .get(&instance)
                .map(|effect| effect.buffer_bytes())
        }

        pub fn capture_particle_state_for_validation(
            &self,
            instance: u64,
        ) -> Result<Vec<crate::particle_gpu::ParticleGpuSnapshot>, String> {
            self.particle_effects
                .get(&instance)
                .ok_or("particle_gpu.instance_missing")?
                .readback_for_validation(&self.device, &self.queue)
        }

        pub fn register_rgba8_texture(
            &mut self,
            handle: RenderResourceHandle,
            width: u32,
            height: u32,
            rgba8: &[u8],
            sampler: &str,
        ) -> Result<(), String> {
            self.register_texture_bytes(
                handle,
                width,
                height,
                rgba8,
                sampler,
                wgpu::TextureFormat::Rgba8Unorm,
                1,
            )
        }

        pub fn register_particle_mesh(
            &mut self,
            handle: RenderResourceHandle,
            positions: &[[f32; 3]],
            uvs: &[[f32; 2]],
            indices: &[u32],
        ) -> Result<(), String> {
            if handle.kind != RenderResourceKind::MeshBuffer {
                return Err("particle_render.mesh_handle_kind".into());
            }
            self.admit_gpu_frame()?;
            self.admit_geometry_backing(indices.len() as u64 * 32)?;
            if let Some(old) = self.particle_meshes.get(&handle) {
                self.admit_gpu_retirement(old.buffer.size(), 1)?;
            }
            self.geometry.admit_visual_upload(0, indices.len() * 32)?;
            let mesh =
                crate::particle_render::ParticleMesh::new(&self.device, positions, uvs, indices)?;
            if let Some(old) = self.particle_meshes.insert(handle, mesh) {
                let bytes = old.buffer.size();
                self.retire_gpu(old, bytes);
            }
            Ok(())
        }

        fn register_texture_bytes(
            &mut self,
            handle: RenderResourceHandle,
            width: u32,
            height: u32,
            rgba8: &[u8],
            sampler: &str,
            format: wgpu::TextureFormat,
            mip_count: u32,
        ) -> Result<(), String> {
            if crate::runtime_texture::TextureSampler::parse(sampler).is_none() {
                return Err("wgpu.texture_sampler_unsupported".into());
            }
            if handle.kind != RenderResourceKind::Texture {
                return Err("wgpu.texture_handle_kind_must_be_texture".to_string());
            }
            if width == 0 || height == 0 {
                return Err("wgpu.texture_zero_sized".to_string());
            }
            let wire_format = match format {
                wgpu::TextureFormat::Rgba8Unorm => "rgba8Unorm",
                wgpu::TextureFormat::Rgba8UnormSrgb => "rgba8UnormSrgb",
                wgpu::TextureFormat::Bc3RgbaUnorm => "bc3RgbaUnorm",
                wgpu::TextureFormat::Bc3RgbaUnormSrgb => "bc3RgbaUnormSrgb",
                _ => return Err("wgpu.texture_format_unsupported".into()),
            };
            if format.is_compressed()
                && !self
                    .device
                    .features()
                    .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
            {
                return Err("wgpu.texture_compression_unsupported".into());
            }
            let (bw, bh) = format.block_dimensions();
            let block_bytes = format.block_copy_size(None).unwrap();
            if width % bw != 0 || height % bh != 0 {
                return Err("wgpu.texture_block_extent_invalid".into());
            }
            let levels =
                crate::runtime_texture::texture_mip_layout(wire_format, width, height, mip_count)
                    .ok_or("wgpu.texture_mip_layout_invalid")?;
            let expected_len = levels.last().map(|l| l.offset + l.byte_len).unwrap();
            if rgba8.len() != expected_len {
                return Err(format!(
                    "wgpu.texture_byte_len_mismatch: expected {expected_len}, got {}",
                    rgba8.len()
                ));
            }
            self.admit_geometry_backing(expected_len as u64)?;
            self.invalidate_geometry_texture_bundles(&handle)?;
            self.admit_gpu_retirement(
                self.textures
                    .get(&handle)
                    .map_or(0, ResidentTexture::byte_len),
                usize::from(self.textures.contains_key(&handle)),
            )?;
            self.geometry.admit_visual_upload(0, expected_len)?;
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("runtime-wgpu-sprite-texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: mip_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            for level in levels {
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: level.level,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &rgba8[level.offset..level.offset + level.byte_len],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(level.width.div_ceil(bw) * block_bytes),
                        rows_per_image: Some(level.height.div_ceil(bh)),
                    },
                    wgpu::Extent3d {
                        width: level.width.div_ceil(bw) * bw,
                        height: level.height.div_ceil(bh) * bh,
                        depth_or_array_layers: 1,
                    },
                );
            }
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                mip_level_count: (!crate::runtime_texture::TextureSampler::parse(sampler)
                    .expect("validated sampler")
                    .min_filter
                    .uses_mips())
                .then_some(1),
                ..Default::default()
            });
            let sampler = self.device.create_sampler(&sampler_descriptor(sampler));
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("runtime-wgpu-sprite-texture-bind-group"),
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
            let old = self.textures.insert(
                handle,
                ResidentTexture {
                    _texture: Arc::new(OwnedTexture(texture)),
                    _view: view,
                    _sampler: sampler,
                    bind_group,
                    _width: width,
                    _height: height,
                },
            );
            if let Some(old) = old {
                let bytes = old.byte_len();
                self.retire_gpu(old, bytes);
            }
            // Public registration makes this a shared renderer resource. Reconcilers
            // mark only their own new uploads as exclusively owned afterward.
            self.owned_particle_textures.remove(&handle);
            self.owned_geometry_textures.remove(&handle);
            Ok(())
        }

        /// Reconcile Prepared geometry textures in the existing backend texture owner.
        /// Logical handles include generation; unchanged frames do not upload again.
        pub fn prepare_geometry_textures(
            &mut self,
            textures: &[crate::render_asset_production::PreparedTextureAsset],
        ) -> Result<(), String> {
            self.admit_gpu_frame()?;
            let active: std::collections::BTreeSet<_> =
                textures.iter().map(|t| t.handle.raw()).collect();
            let retiring = self
                .owned_geometry_textures
                .difference(&active)
                .filter(|h| !self.geometry.streaming_texture_pinned(h))
                .filter_map(|h| self.textures.get(h))
                .collect::<Vec<_>>();
            self.admit_gpu_retirement(retiring.iter().map(|t| t.byte_len()).sum(), retiring.len())?;
            for texture in textures {
                let handle = texture.handle.raw();
                if !self.textures.contains_key(&handle) {
                    let payload = texture.acquire_upload_payload()?;
                    self.register_cooked_texture(handle, &payload)?;
                    self.owned_geometry_textures.insert(handle);
                }
            }
            for handle in self
                .owned_geometry_textures
                .difference(&active)
                .filter(|h| !self.geometry.streaming_texture_pinned(h))
                .copied()
                .collect::<Vec<_>>()
            {
                self.invalidate_geometry_texture_bundles(&handle)?;
                self.owned_geometry_textures.remove(&handle);
                if let Some(old) = self.textures.remove(&handle) {
                    let bytes = old.byte_len();
                    self.retire_gpu(old, bytes);
                }
            }
            Ok(())
        }

        pub fn register_cooked_texture(
            &mut self,
            handle: RenderResourceHandle,
            payload: &crate::runtime_texture::RuntimeTexturePayload,
        ) -> Result<(), String> {
            crate::render_asset_production::validate_texture_payload(payload)
                .map_err(|e| e.to_string())?;
            let format = match payload.format.as_str() {
                "rgba8UnormSrgb" => wgpu::TextureFormat::Rgba8UnormSrgb,
                "rgba8Unorm" => wgpu::TextureFormat::Rgba8Unorm,
                "bc3RgbaUnorm" => wgpu::TextureFormat::Bc3RgbaUnorm,
                "bc3RgbaUnormSrgb" => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
                other => return Err(format!("unsupported cooked texture format: {other}")),
            };
            self.register_texture_bytes(
                handle,
                payload.width,
                payload.height,
                &payload.rgba8,
                &payload.sampler,
                format,
                payload.mip_count,
            )
        }

        pub fn register_alpha8_texture(
            &mut self,
            handle: RenderResourceHandle,
            width: u32,
            height: u32,
            alpha8: &[u8],
            sampler: &str,
        ) -> Result<(), String> {
            let expected_len = width as usize * height as usize;
            if alpha8.len() != expected_len {
                return Err(format!(
                    "wgpu.alpha_texture_byte_len_mismatch: expected {expected_len}, got {}",
                    alpha8.len()
                ));
            }
            let mut rgba8 = Vec::with_capacity(expected_len * 4);
            for alpha in alpha8 {
                rgba8.extend_from_slice(&[255, 255, 255, *alpha]);
            }
            self.register_rgba8_texture(handle, width, height, &rgba8, sampler)
        }

        pub fn register_font_texture_arrays(
            &mut self,
            bundle: &RuntimeLoadedFontBundle,
        ) -> Result<FontTextureArrayUploadReport, String> {
            let identity = (
                bundle.metadata.font_bundle_id.clone(),
                bundle.metadata.generation,
            );
            if self
                .font_bundle_identities
                .get(&identity)
                .is_some_and(|old| old != &bundle.metadata.bundle_digest)
            {
                return Err("wgpu.font_generation_identity_conflict".into());
            }
            self.font_bundle_identities
                .insert(identity, bundle.metadata.bundle_digest.clone());
            let mut report = FontTextureArrayUploadReport {
                font_bundle_id: bundle.metadata.font_bundle_id.clone(),
                generation: bundle.metadata.generation,
                bitmap_layer_count: 0,
                msdf_layer_count: 0,
                registered_page_handles: Vec::new(),
            };
            for render_mode in [
                FontBundleRenderMode::BitmapR8,
                FontBundleRenderMode::MsdfRgba8,
            ] {
                let pages = bundle
                    .metadata
                    .pages
                    .iter()
                    .enumerate()
                    .filter(|(_, page)| page.render_mode == render_mode)
                    .collect::<Vec<_>>();
                if pages.is_empty() {
                    continue;
                }
                let width = pages[0].1.width;
                let height = pages[0].1.height;
                if pages
                    .iter()
                    .any(|(_, page)| page.width != width || page.height != height)
                {
                    return Err("wgpu.font_array_page_dimensions_mismatch".to_string());
                }
                let format = match render_mode {
                    FontBundleRenderMode::BitmapR8 => wgpu::TextureFormat::R8Unorm,
                    FontBundleRenderMode::MsdfRgba8 => wgpu::TextureFormat::Rgba8Unorm,
                };
                let bytes_per_pixel = match render_mode {
                    FontBundleRenderMode::BitmapR8 => 1,
                    FontBundleRenderMode::MsdfRgba8 => 4,
                };
                let standalone_pages =
                    uses_standalone_font_page_textures(&self.state.device_context.backend_name);
                let handle_for = |page: &crate::font_bundle::CookedFontBundlePage| {
                    font_bundle_page_generation_render_handle(
                        &bundle.metadata.font_bundle_id,
                        render_mode,
                        page.page_index,
                        bundle.metadata.generation,
                    )
                };
                let missing = pages
                    .iter()
                    .filter(|(_, p)| !self.textures.contains_key(&handle_for(p)))
                    .count();
                let page_bytes = u64::from(width) * u64::from(height) * u64::from(bytes_per_pixel);
                let existing = (!standalone_pages)
                    .then(|| {
                        pages.iter().find_map(|(_, p)| {
                            self.textures
                                .get(&handle_for(p))
                                .map(|t| t._texture.clone())
                        })
                    })
                    .flatten();
                let extra = if standalone_pages {
                    page_bytes * missing as u64
                } else if existing.is_none() {
                    page_bytes * pages.len() as u64
                } else {
                    0
                };
                self.admit_geometry_backing(extra)?;
                // Never allocate an array merely to discover that this Present
                // cannot write even its first missing page.
                if missing != 0
                    && self
                        .geometry
                        .visual_upload_remaining(0)
                        .is_some_and(|remaining| remaining < page_bytes as usize)
                {
                    return Err("geometry.visual_upload_capacity".into());
                }
                let shared_texture = if standalone_pages || missing == 0 {
                    existing
                } else {
                    Some(existing.unwrap_or_else(|| {
                        Arc::new(OwnedTexture(self.device.create_texture(
                            &wgpu::TextureDescriptor {
                                label: Some("runtime-wgpu-font-texture-array"),
                                size: wgpu::Extent3d {
                                    width,
                                    height,
                                    depth_or_array_layers: pages.len() as u32,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format,
                                usage: wgpu::TextureUsages::TEXTURE_BINDING
                                    | wgpu::TextureUsages::COPY_DST,
                                view_formats: &[],
                            },
                        )))
                    }))
                };
                for (layer, (global_page_index, page)) in pages.iter().enumerate() {
                    let handle = handle_for(page);
                    if self.textures.contains_key(&handle) {
                        report.registered_page_handles.push(handle);
                        continue;
                    }
                    let payload = bundle
                        .page_payloads
                        .get(*global_page_index)
                        .ok_or("wgpu.font_array_page_payload_missing")?;
                    if payload.len() != page_bytes as usize {
                        return Err("wgpu.font_array_page_payload_invalid".into());
                    }
                    self.geometry.admit_visual_upload(0, page_bytes as usize)?;
                    let texture = shared_texture.clone().unwrap_or_else(|| {
                        Arc::new(OwnedTexture(self.device.create_texture(
                            &wgpu::TextureDescriptor {
                                label: Some("runtime-wgpu-font-page-texture"),
                                size: wgpu::Extent3d {
                                    width,
                                    height,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format,
                                usage: wgpu::TextureUsages::TEXTURE_BINDING
                                    | wgpu::TextureUsages::COPY_DST,
                                view_formats: &[],
                            },
                        )))
                    });
                    let texture_layer = if standalone_pages { 0 } else { layer as u32 };
                    let payload = bundle
                        .page_payloads
                        .get(*global_page_index)
                        .ok_or_else(|| "wgpu.font_array_page_payload_missing".to_string())?;
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: texture.as_ref(),
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: texture_layer,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        payload,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(width * bytes_per_pixel),
                            rows_per_image: Some(height),
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                    let view = texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some(if standalone_pages {
                            "runtime-wgpu-font-page-view"
                        } else {
                            "runtime-wgpu-font-array-layer-view"
                        }),
                        format: Some(format),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: Some(1),
                        base_array_layer: texture_layer,
                        array_layer_count: Some(1),
                    });
                    let sampler =
                        self.device
                            .create_sampler(&sampler_descriptor(match render_mode {
                                FontBundleRenderMode::BitmapR8 => "nearestClamp",
                                FontBundleRenderMode::MsdfRgba8 => "linearClamp",
                            }));
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("runtime-wgpu-font-array-layer-bind-group"),
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
                    let handle = font_bundle_page_generation_render_handle(
                        &bundle.metadata.font_bundle_id,
                        render_mode,
                        page.page_index,
                        bundle.metadata.generation,
                    );
                    self.textures.insert(
                        handle,
                        ResidentTexture {
                            _texture: texture.clone(),
                            _view: view,
                            _sampler: sampler,
                            bind_group,
                            _width: width,
                            _height: height,
                        },
                    );
                    report.registered_page_handles.push(handle);
                }
                match render_mode {
                    FontBundleRenderMode::BitmapR8 => report.bitmap_layer_count = pages.len(),
                    FontBundleRenderMode::MsdfRgba8 => report.msdf_layer_count = pages.len(),
                }
            }
            Ok(report)
        }

        pub fn retire_font_texture_arrays(
            &mut self,
            report: &FontTextureArrayUploadReport,
        ) -> usize {
            self.font_bundle_identities
                .remove(&(report.font_bundle_id.clone(), report.generation));
            report
                .registered_page_handles
                .iter()
                .filter(|handle| self.textures.remove(handle).is_some())
                .count()
        }

        fn vertices_for_draw(
            draw_kind: RhiDrawKind,
            vertex_count: u32,
            payload: &RhiDrawPayload,
        ) -> Vec<RuntimeVertex> {
            if matches!(payload, RhiDrawPayload::UiRetained { .. }) {
                return Vec::new();
            }
            if let RhiDrawPayload::SpriteTextured { vertices, .. } = payload {
                if !vertices.is_empty() {
                    return vertices.iter().map(Self::runtime_vertex).collect();
                }
            }
            if let RhiDrawPayload::UiComposition { vertices, .. } = payload {
                return vertices.iter().map(Self::runtime_vertex).collect();
            }
            match draw_kind {
                RhiDrawKind::Geometry => Vec::new(),
                RhiDrawKind::Particles => Vec::new(),
                RhiDrawKind::SpriteBasic
                | RhiDrawKind::SpriteTextured
                | RhiDrawKind::UiOverlay
                | RhiDrawKind::UiComposition => vec![
                    RuntimeVertex {
                        position: [-0.5, -0.5],
                        color: [0.2, 0.7, 1.0, 1.0],
                        uv: [0.0, 1.0],
                    },
                    RuntimeVertex {
                        position: [0.5, -0.5],
                        color: [0.2, 0.7, 1.0, 1.0],
                        uv: [1.0, 1.0],
                    },
                    RuntimeVertex {
                        position: [0.5, 0.5],
                        color: [0.2, 0.7, 1.0, 1.0],
                        uv: [1.0, 0.0],
                    },
                    RuntimeVertex {
                        position: [-0.5, -0.5],
                        color: [0.2, 0.7, 1.0, 1.0],
                        uv: [0.0, 1.0],
                    },
                    RuntimeVertex {
                        position: [0.5, 0.5],
                        color: [0.2, 0.7, 1.0, 1.0],
                        uv: [1.0, 0.0],
                    },
                    RuntimeVertex {
                        position: [-0.5, 0.5],
                        color: [0.2, 0.7, 1.0, 1.0],
                        uv: [0.0, 0.0],
                    },
                ],
                RhiDrawKind::MeshBasic | RhiDrawKind::TestGeometry => {
                    let mut vertices = vec![
                        RuntimeVertex {
                            position: [0.0, 0.6],
                            color: [1.0, 0.3, 0.2, 1.0],
                            uv: [0.5, 0.0],
                        },
                        RuntimeVertex {
                            position: [-0.6, -0.5],
                            color: [0.2, 1.0, 0.4, 1.0],
                            uv: [0.0, 1.0],
                        },
                        RuntimeVertex {
                            position: [0.6, -0.5],
                            color: [0.3, 0.5, 1.0, 1.0],
                            uv: [1.0, 1.0],
                        },
                    ];
                    vertices.truncate(vertex_count as usize);
                    vertices
                }
            }
        }

        fn runtime_vertex(vertex: &RenderDrawVertex) -> RuntimeVertex {
            RuntimeVertex {
                position: vertex.position.map(|value| value.to_f32()),
                color: vertex.color.map(|value| value.to_f32()),
                uv: vertex.uv.map(|value| value.to_f32()),
            }
        }

        pub fn execute_plan_to_surface_view(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
        ) -> RhiBackendReport {
            self.execute_plan_to_surface_view_with_rect(plan, view, None)
        }

        /// Prepare only existing production owners. Capacity waits retain the same
        /// immutable game frame; the caller must actually Present before retrying.
        pub fn prepare_surface_resources(
            &mut self,
            plan: &RhiCommandPlan,
            extent: [u32; 2],
            rect: Option<GameViewRect>,
        ) -> Result<bool, String> {
            self.validate_graph_capabilities(plan)?;
            let geometry_only = Self::geometry_command_shape(plan);
            if !geometry_only {
                self.validate_plan_texture_residency(plan)?;
            }
            let result: Result<(), String> = (|| {
                self.admit_gpu_frame()?;
                self.prepare_geometry_frame_resources(plan, extent, rect)?;
                if !geometry_only {
                    for command in &plan.commands {
                        if let RhiCommand::ReconcileParticles { sources } = command {
                            self.reconcile_project_particles(sources)?;
                        }
                    }
                    let (particle_bytes, _) = self.particle_frame_upload_requirements(plan)?;
                    let pending_bytes =
                        particle_bytes.saturating_add(self.ui_frame_upload_requirements(plan)?);
                    if self
                        .geometry
                        .visual_upload_remaining(plan.frame_index)
                        .is_some_and(|remaining| pending_bytes > remaining)
                    {
                        return Err("geometry.visual_upload_capacity".into());
                    }
                }
                // Skin preparation can submit compute; leave admission for the draw.
                self.admit_gpu_frame()
            })();
            // The geometry owner checks the complete current closure before any
            // upload. On every failed admission, including backpressure, retain
            // the original texture-first diagnostic before interpreting its error.
            if geometry_only && result.is_err() {
                self.validate_plan_texture_residency(plan)?;
            }
            match result {
                Ok(()) => Ok(true),
                Err(error)
                    if matches!(
                        error.as_str(),
                        "geometry.visual_upload_capacity"
                            | "environment.upload_capacity"
                            | "gpu.in_flight_backpressure"
                    ) =>
                {
                    Ok(false)
                }
                Err(error) => Err(error),
            }
        }

        /// A real loading clear has a submission and Present, but no game-frame
        /// sample. Keep the ordinary validated rendering path and completion owner.
        pub fn execute_loading_clear_to_surface_view(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
        ) -> Result<RhiBackendReport, String> {
            if plan.frame_index != 0
                || plan.frame_identity.is_some()
                || plan.commands.iter().any(|c| {
                    !matches!(
                        c,
                        RhiCommand::BeginFrame { .. }
                            | RhiCommand::Clear { .. }
                            | RhiCommand::Submit
                            | RhiCommand::Present { .. }
                    )
                })
            {
                return Err("geometry.loading_clear_invalid".into());
            }
            let measurement = self.measurement.take();
            self.loading_clear = true;
            let report = self.execute_plan_to_surface_view(plan, view);
            self.loading_clear = false;
            self.measurement = measurement;
            Ok(report)
        }

        pub fn execute_plan_to_surface_view_in_rect(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
            display_content_rect: GameViewRect,
        ) -> RhiBackendReport {
            self.execute_plan_to_surface_view_with_rect(plan, view, Some(display_content_rect))
        }

        fn execute_plan_to_surface_view_with_rect(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
            display_content_rect: Option<GameViewRect>,
        ) -> RhiBackendReport {
            if let Err(error) = self.validate_plan_texture_residency(plan) {
                self.state.diagnostics.push(RhiBackendDiagnostic::error(
                    "wgpu.texture_binding_missing",
                    error,
                ));
                return self.finish_report(plan);
            }
            let reuse_intent = self.reuse_geometry_intent(plan);
            // BeginFrame normalizes the reporting format. That normalization
            // cannot retroactively authorize a template recorded under a
            // different incoming consumption context.
            let prepared_intent = !reuse_intent
                && self.state.target_context.format == "Rgba8Unorm"
                && self.validated_geometry_only_commands(plan);
            for (command_index, command) in plan.commands.iter().enumerate() {
                match command {
                    RhiCommand::ResolveColor3d { resolve } => self.resolve_color_3d(resolve),
                    RhiCommand::BeginView {
                        target,
                        view,
                        depth_target,
                    } => self.begin_view(target, view, depth_target.as_deref()),
                    RhiCommand::ReconcileParticles { sources } => {
                        self.state.reconcile_particles(sources)
                    }
                    RhiCommand::SimulateParticles { instance, step } => {
                        self.state.particle_step(*instance, step)
                    }
                    RhiCommand::BeginFrame { target } => self.begin_frame(EngineRhiFrame {
                        frame_index: plan.frame_index,
                        target_id: target.clone(),
                    }),
                    RhiCommand::Clear { target, color } => self.clear(target, *color),
                    RhiCommand::Draw { payload, .. } => {
                        if reuse_intent {
                            self.state.record_reused_geometry_intent(payload);
                        } else if prepared_intent
                            && self.state.target_context.format == "Rgba8Unorm"
                        {
                            self.record_current_geometry_intent(
                                &plan.commands,
                                command_index,
                                payload,
                            );
                        } else {
                            self.state.record_draw(payload);
                        }
                    }
                    RhiCommand::Submit => self.submit(),
                    RhiCommand::Present { target } => self.present(target),
                }
            }
            if let Err(error) = self.render_to_view(plan, view, display_content_rect) {
                self.state.diagnostics.push(RhiBackendDiagnostic::error(
                    "wgpu.render_surface_failed",
                    error,
                ));
            } else {
                self.mark_geometry_intent(plan);
            }
            self.finish_report(plan)
        }

        pub fn validate_plan_texture_residency(&self, plan: &RhiCommandPlan) -> Result<(), String> {
            // Reuse the existing complete command proof, then prove residency
            // against today's registry for every required texture. A changed
            // command or a missing texture falls back to the original ordered
            // scan, preserving the first failure and its diagnostic role.
            if self
                .validated_geometry_texture_handles(plan)
                .is_some_and(|handles| {
                    handles.iter().all(|handle| {
                        #[cfg(test)]
                        TEXTURE_RESIDENCY_QUERIES.with(|n| n.set(n.get() + 1));
                        self.textures.contains_key(handle)
                    })
                })
            {
                return Ok(());
            }
            // The whole plan and live registry stay immutably borrowed here.
            // Interpret each lit binding allocation once and retain only positive
            // handle membership in this call. No result survives replacement or
            // retirement between calls; the ordered first-error scan remains.
            let mut checked_lit_bindings = std::collections::HashSet::new();
            let mut resident = std::collections::HashSet::new();
            for draw in plan
                .commands
                .iter()
                .filter_map(|command| {
                    let draw = command.geometry_draw();
                    #[cfg(test)]
                    if draw.is_some() {
                        CURRENT_TEXTURE_DESCRIPTOR_VISITS.with(|n| n.set(n.get() + 1));
                    }
                    draw
                })
                .filter(|draw| draw.standard_lit.is_some())
                .filter(|draw| {
                    checked_lit_bindings.insert(std::sync::Arc::as_ptr(&draw.material_binding))
                })
            {
                #[cfg(test)]
                LIT_RESIDENCY_BINDING_VISITS.with(|n| n.set(n.get() + 1));
                let slots = crate::render_contract::STANDARD_LIT_TEXTURE_SLOTS;
                for (slot, texture) in slots.into_iter().zip(draw.material_textures()) {
                    if let Some(h) = texture {
                        if resident.contains(&h.raw()) {
                            continue;
                        }
                        #[cfg(test)]
                        TEXTURE_RESIDENCY_QUERIES.with(|n| n.set(n.get() + 1));
                        if !self.textures.contains_key(&h.raw()) {
                            return Err(format!(
                                "DrawGeometry missing resident {slot}: {:?}",
                                h.raw()
                            ));
                        }
                        resident.insert(h.raw());
                    }
                }
            }
            let mut geometry_binding = None;
            let mut geometry_requirements = std::collections::HashMap::new();
            let mut resident_handle = None;
            let missing = plan.commands.iter().find_map(|command| {
                let RhiCommand::Draw { payload, .. } = command else {
                    return None;
                };
                let required = if let Some(draw) = payload.geometry_draw() {
                    // This whole call immutably borrows every binding. Cache
                    // empty results too, and reuse interleaved allocations.
                    // Detached equal bindings still get the complete comparison
                    // used by the old adjacent-binding path, never ID/hash equality.
                    let identity = std::sync::Arc::as_ptr(&draw.material_binding);
                    let required = *geometry_requirements.entry(identity).or_insert_with(|| {
                        if let Some((binding, required)) = geometry_binding {
                            if binding == &draw.material_binding {
                                return required;
                            }
                        }
                        required_texture_binding(payload)
                    });
                    geometry_binding = Some((&draw.material_binding, required));
                    required
                } else {
                    required_texture_binding(payload)
                };
                let (handle, role) = required?;
                if resident_handle == Some(handle) || resident.contains(&handle) {
                    return None;
                }
                #[cfg(test)]
                TEXTURE_RESIDENCY_QUERIES.with(|n| n.set(n.get() + 1));
                if self.textures.contains_key(&handle) {
                    resident_handle = Some(handle);
                    resident.insert(handle);
                    None
                } else {
                    Some((handle, role))
                }
            });
            let Some((handle, role)) = missing else {
                return Ok(());
            };
            Err(format!(
                "{role} command references texture handle {handle:?}, but RealWgpuBackend has no registered GPU texture."
            ))
        }

        pub fn render_plan_to_rgba_bytes(
            &mut self,
            plan: &RhiCommandPlan,
            width: u32,
            height: u32,
        ) -> Result<Vec<u8>, String> {
            self.render_plan_to_rgba_bytes_with_rect(plan, width, height, None)
        }

        pub fn render_plan_to_rgba_bytes_with_rect(
            &mut self,
            plan: &RhiCommandPlan,
            width: u32,
            height: u32,
            display_content_rect: Option<GameViewRect>,
        ) -> Result<Vec<u8>, String> {
            self.render_plan_to_rgba_bytes_with_receipt(plan, width, height, display_content_rect)
                .map(|(rgba, _)| rgba)
        }

        /// Return the actual offscreen draw receipt. The later copy is a separate
        /// maintenance submission and must never replace the capture's identity.
        pub fn render_plan_to_rgba_bytes_with_receipt(
            &mut self,
            plan: &RhiCommandPlan,
            width: u32,
            height: u32,
            display_content_rect: Option<GameViewRect>,
        ) -> Result<(Vec<u8>, GpuSubmissionToken), String> {
            if width == 0 || height == 0 {
                return Err("wgpu.readback_zero_sized_target".to_string());
            }
            let texture = self.create_output_texture(width, height)?;
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let measurement = self.measurement.take();
            self.capturing = true;
            let result = self.render_to_view(plan, &view, display_content_rect);
            self.capturing = false;
            self.measurement = measurement;
            let result = result.and_then(|_| {
                let draw = self
                    .last_gpu_submission()
                    .ok_or("wgpu.capture_submission_missing")?;
                self.read_texture_rgba_bytes(&texture)
                    .map(|rgba| (rgba, draw))
            });
            self.release_output_texture(texture);
            result
        }

        fn render_to_view(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
            display_content_rect: Option<GameViewRect>,
        ) -> Result<(), String> {
            if plan.has_errors() {
                return Err("graph.invalid_plan".into());
            }
            crate::particle_render_contract::validate_plan(plan)?;
            self.validate_graph_capabilities(plan)?;
            self.admit_gpu_frame()?;
            let encoded_started = crate::platform_time::Instant::now();
            if plan.has_errors() {
                return Err("wgpu.invalid_rhi_plan".into());
            }
            if !self.loading_clear {
                self.prepare_geometry_plan(plan, view, display_content_rect)?;
                for command in &plan.commands {
                    if let RhiCommand::ReconcileParticles { sources } = command {
                        self.reconcile_project_particles(sources)?;
                    }
                }
            }
            let has_independent_outputs = if self.loading_clear {
                !self.geometry_color_layout(plan)?.is_empty()
            } else {
                // The output owner just prepared exactly this plan. Consume its
                // actual allocation table instead of rebuilding the same layout.
                !self.geometry.colors.is_empty()
            };
            if has_independent_outputs
                || plan.commands.iter().any(|c| {
                    matches!(
                        c,
                        RhiCommand::BeginView { .. }
                            | RhiCommand::Draw {
                                payload: RhiDrawPayload::Geometry { .. }
                                    | RhiDrawPayload::GeometryBatch { .. },
                                ..
                            }
                            | RhiCommand::SimulateParticles { .. }
                            | RhiCommand::Draw {
                                payload: RhiDrawPayload::Particles { .. },
                                ..
                            }
                    )
                })
            {
                return self.render_particle_plan(plan, view, display_content_rect);
            }
            if let Err(error) = self.validate_plan_texture_residency(plan) {
                self.state.diagnostics.push(RhiBackendDiagnostic::error(
                    "wgpu.texture_binding_missing",
                    error.clone(),
                ));
                return Err(error);
            }
            let content_viewport = display_content_rect
                .map(|rect| surface_content_scissor(rect).map(|scissor| (rect, scissor)))
                .transpose()?;
            let draw_buffers = self.prepare_draw_buffers(plan)?;
            let retained_started = self
                .ui_measurement_enabled
                .then(crate::platform_time::Instant::now);
            let retained_draws = if self.loading_clear {
                Vec::new()
            } else {
                self.prepare_retained_ui(plan)?
            };
            self.ui_upload_stats.cpu_draw_buffer_prepare_ns +=
                crate::aui::aui_elapsed_ns(retained_started);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("runtime-wgpu-surface-encoder"),
                });
            if let Some(m) = &mut self.measurement {
                m.begin(&mut encoder);
            }
            let clear_color = plan.commands.iter().find_map(|command| match command {
                RhiCommand::Clear { color, .. } => Some(wgpu::Color {
                    r: color[0].to_f32() as f64,
                    g: color[1].to_f32() as f64,
                    b: color[2].to_f32() as f64,
                    a: color[3].to_f32() as f64,
                }),
                _ => None,
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("runtime-wgpu-surface-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color.unwrap_or(wgpu::Color::BLACK)),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                if let Some((rect, [x, y, width, height])) = content_viewport {
                    pass.set_viewport(rect.x, rect.y, rect.width, rect.height, 0.0, 1.0);
                    pass.set_scissor_rect(x, y, width, height);
                }
                for (command_index, command) in plan.commands.iter().enumerate() {
                    if let RhiCommand::Draw { payload, .. } = command {
                        if let RhiDrawPayload::UiRetained { draws } = payload {
                            if draws.is_empty() {
                                continue;
                            }
                            self.retained_ui.draw(
                                &mut pass,
                                &retained_draws[command_index],
                                content_viewport.map(|(r, _)| r),
                                draws[0].target_extent,
                            );
                            if let Some((_, [x, y, w, h])) = content_viewport {
                                pass.set_scissor_rect(x, y, w, h);
                            } else {
                                pass.set_scissor_rect(
                                    0,
                                    0,
                                    view.texture().width(),
                                    view.texture().height(),
                                );
                            }
                            continue;
                        }
                        let Some((vertex_buffer, vertex_count)) = &draw_buffers[command_index]
                        else {
                            continue;
                        };
                        if let Some(texture) = self.texture_for_payload(payload) {
                            let pipeline = match payload {
                                RhiDrawPayload::UiComposition {
                                    font_render_mode: Some(FontBundleRenderMode::BitmapR8),
                                    ..
                                } => &self.bitmap_font_pipeline,
                                RhiDrawPayload::UiComposition {
                                    font_render_mode: Some(FontBundleRenderMode::MsdfRgba8),
                                    ..
                                } => &self.msdf_font_pipeline,
                                _ => &self.textured_pipeline,
                            };
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, &texture.bind_group, &[]);
                        } else {
                            pass.set_pipeline(&self.pipeline);
                        }
                        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                        pass.draw(0..*vertex_count, 0..1);
                    }
                }
            }
            if let Some(m) = &mut self.measurement {
                m.end(&mut encoder, &self.particle_effects)?;
            }
            self.submit_gpu_frame(
                encoder,
                plan,
                encoded_started
                    .elapsed()
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64,
            );
            self.retire_pending_draw_buffers();
            Ok(())
        }

        fn texture_for_payload(&self, payload: &RhiDrawPayload) -> Option<&ResidentTexture> {
            let handle = match payload {
                RhiDrawPayload::SpriteTextured {
                    texture: Some(handle),
                    fallback_used: false,
                    ..
                }
                | RhiDrawPayload::UiComposition {
                    texture: Some(handle),
                    ..
                } => handle,
                _ => return None,
            };
            self.textures.get(handle)
        }

        // Surface admission and actual execution consume the same particle
        // requirements. Checking readiness never advances an effect step.
        fn particle_frame_upload_requirements(
            &self,
            plan: &RhiCommandPlan,
        ) -> Result<(usize, usize), String> {
            // Validate every resource before advancing any effect clock.
            for command in &plan.commands {
                match command {
                    RhiCommand::SimulateParticles { instance, step } => {
                        if !self.particle_effects.contains_key(instance) {
                            return Err("particle_gpu.instance_missing".into());
                        }
                        self.particle_effects[instance].validate_affine(&step.emitter_affine)?;
                        if !step.delta_seconds.to_f32().is_finite()
                            || step.delta_seconds.to_f32() < 0.0
                            || !step.origin.iter().all(|v| v.to_f32().is_finite())
                        {
                            return Err("particle_gpu.invalid_step".into());
                        }
                    }
                    RhiCommand::Draw {
                        payload:
                            RhiDrawPayload::Particles {
                                instance,
                                emitter,
                                view,
                                texture,
                                mesh,
                            },
                        ..
                    } => {
                        view.validate()?;
                        let effect = self
                            .particle_effects
                            .get(instance)
                            .ok_or("particle_gpu.instance_missing")?;
                        let state = effect
                            .emitters
                            .get(*emitter as usize)
                            .and_then(|e| e.render.as_ref())
                            .ok_or("particle_render.emitter_missing")?;
                        effect.validate_affine(&view.emitter_affine)?;
                        state.validate_assets(texture.is_some(), mesh.is_some())?;
                        if texture.is_some_and(|h| !self.textures.contains_key(&h)) {
                            return Err("particle_render.texture_missing".into());
                        }
                        if mesh.is_some_and(|h| !self.particle_meshes.contains_key(&h)) {
                            return Err("particle_render.mesh_missing".into());
                        }
                    }
                    _ => (),
                }
            }
            let particle_bytes: usize = plan
                .commands
                .iter()
                .map(|command| match command {
                    RhiCommand::SimulateParticles { instance, step } => {
                        self.particle_effects[instance].step_upload_bytes(step.into())
                    }
                    RhiCommand::Draw {
                        payload:
                            RhiDrawPayload::Particles {
                                instance, emitter, ..
                            },
                        ..
                    } => self.particle_effects[instance].emitters[*emitter as usize]
                        .render
                        .as_ref()
                        .unwrap()
                        .prepare_upload_bytes(),
                    _ => 0,
                })
                .sum();
            let particle_objects: usize = plan
                .commands
                .iter()
                .map(|command| match command {
                    RhiCommand::SimulateParticles { instance, step } => {
                        self.particle_effects[instance].step_upload_objects(step.into())
                    }
                    RhiCommand::Draw {
                        payload:
                            RhiDrawPayload::Particles {
                                instance, emitter, ..
                            },
                        ..
                    } => {
                        self.particle_effects[instance].emitters[*emitter as usize]
                            .render
                            .as_ref()
                            .unwrap()
                            .prepare_upload_bytes()
                            / 304
                    }
                    _ => 0,
                })
                .sum();
            Ok((particle_bytes, particle_objects))
        }

        fn render_particle_plan(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
            rect: Option<GameViewRect>,
        ) -> Result<(), String> {
            let encoded_started = crate::platform_time::Instant::now();
            self.validate_plan_texture_residency(plan)?;
            let (particle_bytes, particle_objects) =
                self.particle_frame_upload_requirements(plan)?;
            self.admit_geometry_backing(particle_bytes as u64)?;
            self.admit_gpu_retirement(particle_bytes as u64, particle_objects)?;
            self.geometry
                .admit_visual_upload(plan.frame_index, particle_bytes)?;
            let viewport = rect
                .map(|r| surface_content_scissor(r).map(|s| (r, s)))
                .transpose()?;
            // The existing successful plan admits only geometry/view/resolve
            // commands. Empty UI owners need no per-command placeholder arrays;
            // real residents and queued work still take the original cleanup path.
            let skip_empty_ui = self.validated_geometry_only_commands(plan)
                && self.ui_vertices.is_empty()
                && self.pending_draw_buffers.is_empty()
                && self.retained_ui.empty_for_geometry_frame();
            let draw_buffers = if skip_empty_ui {
                Vec::new()
            } else {
                self.prepare_draw_buffers(plan)?
            };
            let retained_started = self
                .ui_measurement_enabled
                .then(crate::platform_time::Instant::now);
            let retained_draws = if skip_empty_ui {
                Vec::new()
            } else {
                self.prepare_retained_ui(plan)?
            };
            self.ui_upload_stats.cpu_draw_buffer_prepare_ns +=
                crate::aui::aui_elapsed_ns(retained_started);
            let width = view.texture().width();
            let height = view.texture().height();
            if self
                .particle_depth
                .as_ref()
                .is_none_or(|(w, h, _)| *w != width || *h != height)
            {
                self.admit_geometry_backing(u64::from(width) * u64::from(height) * 4)?;
                if let Some((w, h, _)) = &self.particle_depth {
                    self.admit_gpu_retirement(u64::from(*w) * u64::from(*h) * 4, 1)?;
                }
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("runtime-particle-scene-depth"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                if let Some(old) = self.particle_depth.replace((
                    width,
                    height,
                    texture.create_view(&Default::default()),
                )) {
                    let bytes = u64::from(old.0) * u64::from(old.1) * 4;
                    self.retire_gpu(old, bytes);
                }
            }
            if self.particle_occluder_pipeline.is_none() {
                self.particle_occluder_pipeline =
                    Some(create_basic_pipeline(&self.device, self.format, true));
            }
            let depth_view = self.particle_depth.as_ref().unwrap().2.clone();
            let depth = &depth_view;
            self.admit_geometry_backing(particle_bytes as u64)?;
            self.admit_gpu_retirement(particle_bytes as u64, particle_objects)?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("runtime-particle-graph"),
                });
            if let Some(m) = &mut self.measurement {
                m.begin(&mut encoder);
            }
            let clear = plan
                .commands
                .iter()
                .find_map(|c| {
                    if let RhiCommand::Clear { target, color } = c {
                        if geometry::targets::primary(plan).is_some_and(|p| p != target) {
                            return None;
                        }
                        Some(wgpu::Color {
                            r: color[0].to_f32() as f64,
                            g: color[1].to_f32() as f64,
                            b: color[2].to_f32() as f64,
                            a: color[3].to_f32() as f64,
                        })
                    } else {
                        None
                    }
                })
                .unwrap_or(wgpu::Color::BLACK);
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("particle-scene-clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: if plan
                                .commands
                                .iter()
                                .any(|c| matches!(c, RhiCommand::BeginView { .. }))
                                && !plan
                                    .commands
                                    .iter()
                                    .any(|c| matches!(c, RhiCommand::Clear { .. }))
                            {
                                wgpu::LoadOp::Load
                            } else {
                                wgpu::LoadOp::Clear(clear)
                            },
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }
            let primary = geometry::targets::primary(plan);
            let mut consumed_through = 0;
            let mut geometry_instance_offset = 0;
            for (command_index, command) in plan.commands.iter().enumerate() {
                if command_index < consumed_through {
                    continue;
                }
                if let RhiCommand::ResolveColor3d { resolve } = command {
                    self.encode_color_resolve(&mut encoder, view, resolve, rect)?;
                    if let Some(m) = &mut self.measurement {
                        m.draw();
                    }
                    continue;
                }
                if let RhiCommand::Clear { target, color } = command {
                    if primary.is_some_and(|p| p != target) {
                        let output = &self.geometry.colors[target].2;
                        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("independent-output-clear"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: output,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: color[0].to_f32() as f64,
                                        g: color[1].to_f32() as f64,
                                        b: color[2].to_f32() as f64,
                                        a: color[3].to_f32() as f64,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                    }
                    continue;
                }
                if let RhiCommand::BeginView {
                    target,
                    view: camera,
                    depth_target,
                } = command
                {
                    let output = self.geometry.colors.get(target).map(|(_, _, v)| v.clone());
                    self.encode_geometry_begin(
                        &mut encoder,
                        output.as_ref().unwrap_or(view),
                        camera,
                        depth_target.as_deref(),
                        if output.is_some() { None } else { rect },
                    )?;
                    continue;
                }
                if let Some(draw) = command.geometry_draw() {
                    let mut end = self.geometry_draw_end(plan, command_index);
                    let measuring = self.measurement.as_ref().is_some_and(|m| m.active());
                    let source = measuring
                        .then(|| geometry::measurement_draw_source(command))
                        .flatten();
                    let spine = source == Some(true);
                    if measuring && source.is_none() {
                        self.measurement
                            .as_mut()
                            .unwrap()
                            .spine_mixed_source_batch();
                    }
                    if measuring {
                        end = geometry::measurement_draw_end(&plan.commands, command_index, end);
                    }
                    let start = if spine {
                        self.measurement.as_mut().unwrap().mark(&mut encoder)?
                    } else {
                        None
                    };
                    let draws_before = self.retained_geometry_stats().draws;
                    let output = self
                        .geometry
                        .colors
                        .get(draw.target.as_ref())
                        .map(|(_, _, v)| v.clone());
                    self.encode_geometry_draws(
                        &mut encoder,
                        output.as_ref().unwrap_or(view),
                        &plan.commands[command_index..end],
                        geometry_instance_offset,
                        if output.is_some() { None } else { rect },
                    )?;
                    if spine {
                        let draws = self.retained_geometry_stats().draws - draws_before;
                        let m = self.measurement.as_mut().unwrap();
                        m.end_range(&mut encoder, start, 4)?;
                        m.spine_draws(
                            draws,
                            plan.commands[command_index..end]
                                .iter()
                                .map(|c| c.geometry_instance_count() as u64)
                                .sum(),
                        );
                    }
                    geometry_instance_offset += plan.commands[command_index..end]
                        .iter()
                        .map(|c| c.geometry_instance_count() as u32)
                        .sum::<u32>();
                    consumed_through = end;
                    continue;
                }
                if matches!(
                    command,
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Particles { .. },
                        ..
                    }
                ) {
                    // Prepare consecutive independent emitter draws, then keep them
                    // in one render pass. Preserve ordering around non-particle work
                    // and split repeated emitters because their uniforms are mutable.
                    let RhiCommand::Draw {
                        target: particle_target,
                        payload:
                            RhiDrawPayload::Particles {
                                view: particle_view,
                                ..
                            },
                        ..
                    } = command
                    else {
                        unreachable!()
                    };
                    let scene_depth = particle_view
                        .depth_target
                        .as_ref()
                        .map(|id| {
                            self.geometry
                                .depths
                                .get(id)
                                .map(|(_, _, v)| v.clone())
                                .ok_or("particle_render.scene_depth_missing")
                        })
                        .transpose()?;
                    let particle_output = self
                        .geometry
                        .colors
                        .get(particle_target.as_ref())
                        .map(|(_, _, v)| v.clone());
                    let output = particle_output.as_ref().unwrap_or(view);
                    let particle_format = output.texture().format();
                    let mut keys = std::collections::BTreeSet::new();
                    let mut batch = Vec::new();
                    let prepare_start = self
                        .measurement
                        .as_mut()
                        .map(|m| m.mark(&mut encoder))
                        .transpose()?
                        .flatten();
                    for next in &plan.commands[command_index..] {
                        let RhiCommand::Draw {
                            payload:
                                RhiDrawPayload::Particles {
                                    instance,
                                    emitter,
                                    view: camera,
                                    texture,
                                    mesh,
                                },
                            target: next_target,
                            ..
                        } = next
                        else {
                            break;
                        };
                        if next_target != particle_target
                            || camera.depth_target != particle_view.depth_target
                            || camera.viewport != particle_view.viewport
                        {
                            break;
                        }
                        if !keys.insert((*instance, *emitter)) {
                            break;
                        }
                        let effect = &self.particle_effects[instance];
                        let render = effect.emitters[*emitter as usize].render.as_ref().unwrap();
                        let (bindings, dispatches) = render.prepare(
                            &self.device,
                            &mut encoder,
                            camera,
                            effect.time_seconds() as f32,
                            texture
                                .and_then(|h| self.textures.get(&h))
                                .map(|t| (&t._view, &t._sampler)),
                            mesh.and_then(|h| self.particle_meshes.get(&h)),
                            &mut self.pending_draw_buffers,
                        )?;
                        if let Some(m) = &mut self.measurement {
                            m.dispatches(dispatches);
                        }
                        batch.push((render, bindings));
                    }
                    {
                        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("particle-independent-prepare-batch"),
                            timestamp_writes: None,
                        });
                        for (render, _) in &batch {
                            render.encode_unordered_prepare(&mut pass);
                        }
                    }
                    if let Some(m) = &mut self.measurement {
                        m.end_range(&mut encoder, prepare_start, 2)?;
                    }
                    consumed_through = command_index + batch.len();
                    let start = self
                        .measurement
                        .as_mut()
                        .map(|m| m.mark(&mut encoder))
                        .transpose()?
                        .flatten();
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("runtime-particle-draw-batch"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: output,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: scene_depth.as_ref().unwrap_or(depth),
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    if let Some(region) = particle_view.viewport {
                        // Scene depth is allocated at the validated logical view
                        // extent, even when primary color maps into a content rect.
                        // Reuse that owner instead of scanning the plan per batch.
                        let logical = scene_depth.as_ref().unwrap_or(output);
                        let extent = [logical.texture().width(), logical.texture().height()];
                        let r = geometry::output_region(
                            region,
                            extent,
                            if particle_output.is_some() {
                                None
                            } else {
                                rect
                            },
                        );
                        let [x, y, w, h] = surface_content_scissor(r)?;
                        pass.set_viewport(r.x, r.y, r.width, r.height, 0., 1.);
                        pass.set_scissor_rect(x, y, w, h);
                    } else if let Some((r, [x, y, w, h])) = viewport {
                        pass.set_viewport(r.x, r.y, r.width, r.height, 0.0, 1.0);
                        pass.set_scissor_rect(x, y, w, h);
                    }
                    for (render, bindings) in &batch {
                        pass.set_pipeline(if particle_format == wgpu::TextureFormat::Rgba16Float {
                            &render.hdr_pipeline
                        } else {
                            &render.pipeline
                        });
                        pass.set_bind_group(0, bindings, &[]);
                        pass.draw_indirect(&render.indirect, 0);
                    }
                    drop(pass);
                    if let Some(m) = &mut self.measurement {
                        m.end_range(&mut encoder, start, 3)?;
                        for _ in &batch {
                            m.draw();
                        }
                    }
                    continue;
                }
                if let RhiCommand::SimulateParticles { instance, step } = command {
                    let start = self
                        .measurement
                        .as_mut()
                        .map(|m| m.mark(&mut encoder))
                        .transpose()?
                        .flatten();
                    let step_report = self
                        .particle_effects
                        .get_mut(instance)
                        .unwrap()
                        .encode_step_retained(
                            &self.device,
                            &mut encoder,
                            step.into(),
                            &mut self.pending_draw_buffers,
                        )?;
                    if let Some(m) = &mut self.measurement {
                        m.end_range(&mut encoder, start, 1)?;
                        m.dispatches(step_report.dispatch_count);
                    }
                    continue;
                }
                let RhiCommand::Draw {
                    draw_kind, payload, ..
                } = command
                else {
                    continue;
                };
                if let RhiDrawPayload::UiRetained { draws } = payload {
                    if draws.is_empty() {
                        continue;
                    }
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("aui-retained-mixed-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    if let Some((r, _)) = viewport {
                        pass.set_viewport(r.x, r.y, r.width, r.height, 0., 1.);
                    }
                    self.retained_ui.draw(
                        &mut pass,
                        &retained_draws[command_index],
                        viewport.map(|(r, _)| r),
                        draws[0].target_extent,
                    );
                    continue;
                }
                let Some((vertex_buffer, vertex_count)) = &draw_buffers[command_index] else {
                    continue;
                };
                let occluder = matches!(
                    draw_kind,
                    RhiDrawKind::MeshBasic | RhiDrawKind::TestGeometry
                );
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("runtime-ordered-draw"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: occluder.then_some(
                        wgpu::RenderPassDepthStencilAttachment {
                            view: depth,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        },
                    ),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                if let Some((r, [x, y, w, h])) = viewport {
                    pass.set_viewport(r.x, r.y, r.width, r.height, 0.0, 1.0);
                    pass.set_scissor_rect(x, y, w, h);
                }
                {
                    if occluder {
                        pass.set_pipeline(self.particle_occluder_pipeline.as_ref().unwrap());
                    } else if let Some(texture) = self.texture_for_payload(payload) {
                        let pipeline = match payload {
                            RhiDrawPayload::UiComposition {
                                font_render_mode: Some(FontBundleRenderMode::BitmapR8),
                                ..
                            } => &self.bitmap_font_pipeline,
                            RhiDrawPayload::UiComposition {
                                font_render_mode: Some(FontBundleRenderMode::MsdfRgba8),
                                ..
                            } => &self.msdf_font_pipeline,
                            _ => &self.textured_pipeline,
                        };
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(0, &texture.bind_group, &[]);
                    } else {
                        pass.set_pipeline(&self.pipeline);
                    }
                    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    pass.draw(0..*vertex_count, 0..1);
                }
                drop(pass);
            }
            if let Some(m) = &mut self.measurement {
                m.end(&mut encoder, &self.particle_effects)?;
            }
            self.submit_gpu_frame(
                encoder,
                plan,
                encoded_started
                    .elapsed()
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64,
            );
            self.retire_pending_draw_buffers();
            Ok(())
        }
    }

    #[cfg(test)]
    thread_local! {
        static UI_PREPARATION_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static GEOMETRY_TEXTURE_LOOKUPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
        pub(super) static CURRENT_TEXTURE_DESCRIPTOR_VISITS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};
        pub(super) static CURRENT_TEXTURE_CLOSURE_QUERIES: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};
        static LIT_RESIDENCY_BINDING_VISITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
        static TEXTURE_RESIDENCY_QUERIES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }

    fn required_texture_binding(
        payload: &RhiDrawPayload,
    ) -> Option<(RenderResourceHandle, &'static str)> {
        match payload {
            payload if payload.geometry_draw().is_some() => {
                let draw = payload.geometry_draw().unwrap();
                #[cfg(test)]
                GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.set(n.get() + 1));
                draw.material_textures()[0].map(|h| (h.raw(), "DrawGeometry"))
            }
            RhiDrawPayload::SpriteTextured {
                texture: Some(handle),
                fallback_used: false,
                ..
            } => Some((*handle, "SpriteTextured")),
            RhiDrawPayload::UiComposition {
                texture: Some(handle),
                ..
            } => Some((*handle, "UiComposition")),
            _ => None,
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FontTextureArrayUploadReport {
        pub font_bundle_id: String,
        pub generation: u64,
        pub bitmap_layer_count: usize,
        pub msdf_layer_count: usize,
        pub registered_page_handles: Vec<RenderResourceHandle>,
    }

    impl EngineRhiBackend for RealWgpuBackend {
        fn begin_geometry_upload(
            &mut self,
            geometry: &[crate::render_geometry::PreparedGeometry],
            textures: &[crate::render_asset_production::PreparedTextureAsset],
        ) -> Result<crate::geometry_upload::GeometryUploadTicket, String> {
            RealWgpuBackend::begin_geometry_upload(self, geometry, textures)
        }
        fn geometry_upload_state(
            &mut self,
            ticket: crate::geometry_upload::GeometryUploadTicket,
        ) -> Result<crate::geometry_upload::GeometryUploadState, String> {
            RealWgpuBackend::geometry_upload_state(self, ticket)
        }
        fn release_geometry_upload(
            &mut self,
            ticket: crate::geometry_upload::GeometryUploadTicket,
        ) -> Result<(), String> {
            RealWgpuBackend::release_geometry_upload(self, ticket)
        }

        fn supports_geometry_batches(&self) -> bool {
            true
        }
        fn resolve_color_3d(&mut self, resolve: &crate::color_pipeline::ColorResolve3D) {
            self.state.resolve_color_3d(resolve);
        }
        fn begin_view(
            &mut self,
            target: &str,
            view: &crate::render_contract::ExecutableRenderView,
            depth_target: Option<&str>,
        ) {
            self.state.begin_view(target, view, depth_target);
        }
        fn reconcile_particles(
            &mut self,
            sources: &[crate::particle_render_contract::ParticleSourceFrame],
        ) {
            self.state.reconcile_particles(sources);
        }
        fn particle_step(
            &mut self,
            instance: u64,
            step: &crate::particle_render_contract::ParticleRenderStep,
        ) {
            self.state.particle_step(instance, step);
        }
        fn backend_kind(&self) -> &'static str {
            "wgpu"
        }

        fn begin_frame(&mut self, frame: EngineRhiFrame) {
            self.state.begin_frame(frame);
            self.state.target_context.target_kind = "offscreenTexture".to_string();
            self.state.target_context.width = self.width;
            self.state.target_context.height = self.height;
            self.state.target_context.format = "Rgba8Unorm".to_string();
        }

        fn clear(&mut self, target_id: &str, color: [OrderedF32; 4]) {
            self.state.clear(target_id, color);
        }

        fn draw(&mut self, draw_call: EngineRhiDrawCall) {
            self.state.draw(draw_call);
        }

        fn submit(&mut self) {
            self.state.submit();
        }

        fn present(&mut self, target_id: &str) {
            self.state.present(target_id);
        }

        fn execute_plan(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport {
            let texture = match self.create_output_texture(self.width, self.height) {
                Ok(texture) => texture,
                Err(error) => {
                    self.state.diagnostics.push(RhiBackendDiagnostic::error(
                        "wgpu.output_admission_failed",
                        error,
                    ));
                    return self.finish_report(plan);
                }
            };
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let report = self.execute_plan_to_surface_view(plan, &view);
            self.release_output_texture(texture);
            report
        }

        fn finish_report(&mut self, plan: &RhiCommandPlan) -> RhiBackendReport {
            let mut report = self.state.finish_report(plan);
            report.target_hash = format!(
                "wgpu-real:{}:{}:{}:{}",
                report.clear_count, report.draw_count, report.submit_count, report.present_count
            );
            report
        }
    }

    fn create_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        create_basic_pipeline(device, format, false)
    }
    fn create_basic_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth: bool,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("runtime-wgpu-basic-shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
  @location(0) position: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) _uv: vec2<f32>
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4<f32>(position, 0.0, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
  return in.color;
}
"#
                .into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("runtime-wgpu-basic-pipeline-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("runtime-wgpu-basic-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[RuntimeVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: depth.then_some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        })
    }

    fn create_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("runtime-wgpu-texture-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_textured_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("runtime-wgpu-textured-shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) uv: vec2<f32>,
};

@group(0) @binding(0) var sprite_texture: texture_2d<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;

@vertex
fn vs_main(
  @location(0) position: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) uv: vec2<f32>
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4<f32>(position, 0.0, 1.0);
  out.color = color;
  out.uv = uv;
  return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
  return textureSample(sprite_texture, sprite_sampler, in.uv) * in.color;
}
"#
                .into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("runtime-wgpu-textured-pipeline-layout"),
            bind_group_layouts: &[texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("runtime-wgpu-textured-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[RuntimeVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        })
    }

    fn create_font_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        msdf: bool,
    ) -> wgpu::RenderPipeline {
        let fragment = if msdf {
            r#"
fn median3(value: vec3<f32>) -> f32 {
  return max(min(value.r, value.g), min(max(value.r, value.g), value.b));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
  let distance = median3(textureSample(font_texture, font_sampler, in.uv).rgb);
  let width = max(fwidth(distance), 0.0001);
  let coverage = smoothstep(0.5 - width, 0.5 + width, distance);
  return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
"#
        } else {
            r#"
@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
  let coverage = textureSample(font_texture, font_sampler, in.uv).r;
  return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
"#
        };
        let shader_source = format!(
            r#"
struct VertexOut {{
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) uv: vec2<f32>,
}};

@group(0) @binding(0) var font_texture: texture_2d<f32>;
@group(0) @binding(1) var font_sampler: sampler;

@vertex
fn vs_main(
  @location(0) position: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) uv: vec2<f32>
) -> VertexOut {{
  var out: VertexOut;
  out.position = vec4<f32>(position, 0.0, 1.0);
  out.color = color;
  out.uv = uv;
  return out;
}}

{fragment}
"#
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(if msdf {
                "runtime-wgpu-msdf-font-shader"
            } else {
                "runtime-wgpu-bitmap-font-shader"
            }),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("runtime-wgpu-font-pipeline-layout"),
            bind_group_layouts: &[texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(if msdf {
                "runtime-wgpu-msdf-font-pipeline"
            } else {
                "runtime-wgpu-bitmap-font-pipeline"
            }),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[RuntimeVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        })
    }

    fn sampler_descriptor(label: &str) -> wgpu::SamplerDescriptor<'static> {
        use crate::runtime_texture::{TextureFilter, TextureSampler, TextureWrap};
        let settings =
            TextureSampler::parse(label).expect("validated texture or built-in font sampler");
        let filter = |value: TextureFilter| match value.base_filter() {
            TextureFilter::Nearest => wgpu::FilterMode::Nearest,
            TextureFilter::Linear => wgpu::FilterMode::Linear,
            _ => unreachable!("base filter"),
        };
        let wrap = |value| match value {
            TextureWrap::Repeat => wgpu::AddressMode::Repeat,
            TextureWrap::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            TextureWrap::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        };
        wgpu::SamplerDescriptor {
            label: Some("runtime-wgpu-sampler"),
            address_mode_u: wrap(settings.wrap_s),
            address_mode_v: wrap(settings.wrap_t),
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            min_filter: filter(settings.min_filter),
            mag_filter: filter(settings.mag_filter),
            mipmap_filter: filter(settings.min_filter.mip_filter()),
            // Restrict the texture view, not LOD: clamping to zero changes min/mag selection.
            lod_max_clamp: 32.,
            ..Default::default()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::font_bundle::{
            CookedFontBundleAsset, CookedFontBundlePage, RuntimeLoadedFontBundle,
            COOKED_FONT_BUNDLE_SCHEMA_VERSION,
        };
        use crate::render_graph::{
            color, RenderDrawVertex, RenderGraph, RenderPass, RenderPassCommand, RenderPassKind,
            RenderResource, UiVertexStream,
        };
        use crate::rhi_command_plan::compile_render_graph_to_rhi_plan;

        fn texture_residency_plan(
            handle: RenderResourceHandle,
            independent: bool,
        ) -> RhiCommandPlan {
            let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(128);
            let mut shared = None;
            for command in graph.passes.iter_mut().flat_map(|p| &mut p.commands) {
                if let RenderPassCommand::DrawGeometry { draw } = command {
                    let draw = Arc::make_mut(draw);
                    let binding = shared.get_or_insert_with(|| {
                        let mut binding = (*draw.material_binding).clone();
                        binding
                            .slots
                            .push(crate::render_contract::NamedRenderBinding::new(
                                "baseColorTexture",
                                handle,
                            ));
                        Arc::new(binding)
                    });
                    draw.material_binding = if independent {
                        Arc::new((**binding).clone())
                    } else {
                        binding.clone()
                    };
                }
            }
            let plan = compile_render_graph_to_rhi_plan(&graph);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            plan
        }

        #[test]
        #[ignore = "requires a real local WGPU adapter"]
        fn texture_residency_reuses_equal_immutable_bindings_within_one_call() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).expect("GPU required");
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 404,
                generation: 7,
            };
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            for independent in [false, true] {
                let plan = texture_residency_plan(handle, independent);
                GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.set(0));
                TEXTURE_RESIDENCY_QUERIES.with(|n| n.set(0));
                backend.validate_plan_texture_residency(&plan).unwrap();
                assert_eq!(
                    GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.get()),
                    1,
                    "independent={independent}"
                );
                assert_eq!(
                    TEXTURE_RESIDENCY_QUERIES.with(|n| n.get()),
                    1,
                    "independent={independent}"
                );
            }
        }

        #[test]
        #[ignore = "requires a real local WGPU adapter"]
        fn execution_chain_texture_requirements_parse_interleaved_and_empty_bindings_once() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 404,
                generation: 7,
            };
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            let mut actual = Vec::new();
            for count in [4, 32, 256] {
                let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(count);
                let mut bindings = None;
                for (i, command) in graph
                    .passes
                    .iter_mut()
                    .flat_map(|p| &mut p.commands)
                    .enumerate()
                {
                    let RenderPassCommand::DrawGeometry { draw } = command else {
                        continue;
                    };
                    let pair = bindings.get_or_insert_with(|| {
                        let empty = draw.material_binding.clone();
                        let mut textured = (*empty).clone();
                        textured
                            .slots
                            .push(crate::render_contract::NamedRenderBinding::new(
                                "baseColorTexture",
                                handle,
                            ));
                        [empty, Arc::new(textured)]
                    });
                    Arc::make_mut(draw).material_binding = pair[i % 2].clone();
                }
                let plan = compile_render_graph_to_rhi_plan(&graph);
                assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
                GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.set(0));
                backend.validate_plan_texture_residency(&plan).unwrap();
                actual.push((count, GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.get())));
            }
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            assert!(
                actual.iter().all(|&(_, n)| n == 2),
                "binding parses {actual:?}"
            );
        }

        #[test]
        #[ignore = "requires a real local WGPU adapter"]
        fn residual_lit_residency_interprets_shared_binding_once_and_queries_current_handles() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 404,
                generation: 7,
            };
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            let mut actual = Vec::new();
            for count in [4, 32, 128] {
                let mut plan = texture_residency_plan(handle, false);
                let mut draws = 0;
                plan.commands.retain(|c| {
                    if c.geometry_draw().is_some() {
                        draws += 1;
                        draws <= count
                    } else {
                        true
                    }
                });
                for c in &mut plan.commands {
                    if let RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } = c
                    {
                        Arc::make_mut(draw).standard_lit =
                            Some(crate::render_contract::StandardLitParameters {
                                surface: [0., 0.5, 1., 1.].map(Into::into),
                                emissive: [0.; 4].map(Into::into),
                            });
                    }
                }
                LIT_RESIDENCY_BINDING_VISITS.with(|n| n.set(0));
                TEXTURE_RESIDENCY_QUERIES.with(|n| n.set(0));
                backend.validate_plan_texture_residency(&plan).unwrap();
                actual.push((
                    count,
                    LIT_RESIDENCY_BINDING_VISITS.with(|n| n.get()),
                    TEXTURE_RESIDENCY_QUERIES.with(|n| n.get()),
                ));
            }
            eprintln!("lit residency binding visits/registry work {actual:?}");
            // A detached/COW binding with the same ID must not inherit the
            // successful five-slot fact of another allocation or another call.
            let mut plan = texture_residency_plan(handle, false);
            let missing = RenderResourceHandle {
                generation: 8,
                ..handle
            };
            for c in &mut plan.commands {
                if let RhiCommand::Draw {
                    payload: RhiDrawPayload::Geometry { draw },
                    ..
                } = c
                {
                    Arc::make_mut(draw).standard_lit =
                        Some(crate::render_contract::StandardLitParameters {
                            surface: [0., 0.5, 1., 1.].map(Into::into),
                            emissive: [0.; 4].map(Into::into),
                        });
                }
            }
            let last = plan
                .commands
                .iter_mut()
                .rev()
                .find_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .unwrap();
            Arc::make_mut(&mut Arc::make_mut(last).material_binding)
                .slots
                .push(crate::render_contract::NamedRenderBinding::new(
                    "normalTexture",
                    missing,
                ));
            for _ in 0..2 {
                let error = backend.validate_plan_texture_residency(&plan).unwrap_err();
                assert!(
                    error.starts_with("DrawGeometry missing resident normalTexture"),
                    "{error}"
                );
                assert!(error.contains("generation: 8"), "{error}");
            }
            backend
                .register_rgba8_texture(missing, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            backend.validate_plan_texture_residency(&plan).unwrap();
            crate::render_contract::RenderBindingSet::take_validation_count_for_test();
            for _ in 0..32 {
                backend.validate_plan_texture_residency(&plan).unwrap();
            }
            let raw_slot_validations =
                crate::render_contract::RenderBindingSet::take_validation_count_for_test();
            eprintln!("warm residency actual raw binding validations {raw_slot_validations}");
            assert_eq!(
                raw_slot_validations, 0,
                "warm pure slot facts were fully interpreted again"
            );
            backend.textures.remove(&handle);
            let error = backend.validate_plan_texture_residency(&plan).unwrap_err();
            assert!(
                error.starts_with("DrawGeometry missing resident baseColorTexture"),
                "{error}"
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            assert!(
                actual
                    .iter()
                    .all(|(_, interpret, queries)| *interpret == 1 && *queries == 1),
                "actual binding visits/registry work {actual:?}"
            );
        }

        #[test]
        #[ignore = "requires a real local WGPU adapter"]
        fn texture_residency_uses_existing_validated_plan_and_rechecks_registry() {
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).expect("GPU required");
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 404,
                generation: 7,
            };
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            let plan = texture_residency_plan(handle, true);
            backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            assert!(
                backend.retained_geometry_stats().validation_draw_visits > 0,
                "no successful geometry preparation"
            );
            for _ in 0..2 {
                GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.set(0));
                TEXTURE_RESIDENCY_QUERIES.with(|n| n.set(0));
                backend.validate_plan_texture_residency(&plan).unwrap();
                assert_eq!(
                    GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.get()),
                    0,
                    "matching successful plan reinterpreted bindings per draw"
                );
                assert_eq!(
                    TEXTURE_RESIDENCY_QUERIES.with(|n| n.get()),
                    1,
                    "each real required texture must be checked on every call"
                );
            }
            let mut changed = plan.clone();
            let missing = RenderResourceHandle {
                generation: 8,
                ..handle
            };
            let draw = changed
                .commands
                .iter_mut()
                .filter_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .nth(1)
                .unwrap();
            Arc::make_mut(&mut Arc::make_mut(draw).material_binding)
                .slots
                .last_mut()
                .unwrap()
                .resource =
                crate::render_contract::NamedRenderBinding::new("baseColorTexture", missing)
                    .resource;
            assert!(
                backend
                    .validate_plan_texture_residency(&changed)
                    .unwrap_err()
                    .contains("generation: 8"),
                "COW reused an old requirement"
            );
            backend.textures.remove(&handle);
            for _ in 0..2 {
                assert!(
                    backend
                        .validate_plan_texture_residency(&plan)
                        .unwrap_err()
                        .contains("generation: 7"),
                    "plan proof substituted for live residency"
                );
            }
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            backend.validate_plan_texture_residency(&plan).unwrap();
        }

        #[test]
        #[ignore = "requires a real local WGPU adapter"]
        fn texture_residency_rechecks_cow_binding_and_registry_between_calls() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).expect("GPU required");
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 404,
                generation: 7,
            };
            backend
                .register_rgba8_texture(handle, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            let mut plan = texture_residency_plan(handle, true);
            backend.validate_plan_texture_residency(&plan).unwrap();
            let missing = RenderResourceHandle {
                generation: 8,
                ..handle
            };
            let second = plan
                .commands
                .iter_mut()
                .filter_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .nth(1)
                .unwrap();
            let binding = Arc::make_mut(&mut Arc::make_mut(second).material_binding);
            binding.slots.last_mut().unwrap().resource =
                crate::render_contract::NamedRenderBinding::new("baseColorTexture", missing)
                    .resource;
            let error = backend.validate_plan_texture_residency(&plan).unwrap_err();
            assert!(error.starts_with("DrawGeometry command"));
            assert!(error.contains("generation: 8"));
            backend
                .register_rgba8_texture(missing, 1, 1, &[255; 4], "nearestClamp")
                .unwrap();
            backend.validate_plan_texture_residency(&plan).unwrap();
            // Equal IDs and texture slots are insufficient: all binding fields
            // must participate, including fields that make texture() invalid.
            let mut invalid = plan.clone();
            let second = invalid
                .commands
                .iter_mut()
                .filter_map(|c| match c {
                    RhiCommand::Draw {
                        payload: RhiDrawPayload::Geometry { draw },
                        ..
                    } => Some(draw),
                    _ => None,
                })
                .nth(1)
                .unwrap();
            Arc::make_mut(&mut Arc::make_mut(second).material_binding)
                .binding_id
                .clear();
            GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.set(0));
            backend.validate_plan_texture_residency(&invalid).unwrap();
            assert_eq!(GEOMETRY_TEXTURE_LOOKUPS.with(|n| n.get()), 3);
            backend.textures.remove(&handle);
            let error = backend.validate_plan_texture_residency(&plan).unwrap_err();
            assert!(error.contains("generation: 7"));
        }

        #[test]
        fn e7_ui_vertex_variants_are_bounded() {
            assert!(ui_vertex_variant_is_bounded(4_096, 8 * 1024 * 1024));
            assert!(!ui_vertex_variant_is_bounded(4_097, 1));
            assert!(!ui_vertex_variant_is_bounded(1, 8 * 1024 * 1024 + 1));
        }

        #[test]
        #[ignore = "requires a real local WGPU adapter"]
        fn b3_real_gpu_vertex_cache_is_bounded_and_retires_unused_sources() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).expect("B3 GPU required");
            let mut plan = RhiCommandPlan {
                graph_schedule: None,
                frame_identity: None,
                packet_content_version: None,
                frame_index: 1,
                graph_id: "b3-capacity".into(),
                target_kind: "offscreen".into(),
                commands: Vec::new().into(),
                diagnostics: Vec::new(),
            };
            for _ in 0..MAX_UI_VERTEX_BUFFERS + 1 {
                plan.commands.push(RhiCommand::Draw {
                    target: "offscreen".into(),
                    draw_kind: RhiDrawKind::UiComposition,
                    vertex_count: 6,
                    payload: RhiDrawPayload::UiComposition {
                        stage: "screen_overlay".into(),
                        item_count: 1,
                        text_count: 0,
                        image_count: 0,
                        glyph_count: 0,
                        font_atlas_id: None,
                        text_pass_inserted: false,
                        debug_label: "b3".into(),
                        pipeline_key: "ui".into(),
                        texture: None,
                        font_render_mode: None,
                        font_page_index: None,
                        vertices: quad_vertices_for_test().into(),
                    },
                });
            }
            let prepared = backend.prepare_draw_buffers(&plan).unwrap();
            assert_eq!(
                prepared.iter().flatten().count(),
                MAX_UI_VERTEX_BUFFERS + 1,
                "capacity must not drop visible draws"
            );
            let first = backend.ui_vertex_upload_stats();
            assert_eq!(first.resident_buffers, MAX_UI_VERTEX_BUFFERS);
            assert!(first.resident_bytes <= MAX_UI_VERTEX_BYTES);
            backend.prepare_draw_buffers(&plan).unwrap();
            let second = backend.ui_vertex_upload_stats();
            assert_eq!(
                second.uploads - first.uploads,
                1,
                "overflow is drawn transiently"
            );
            assert_eq!(second.hits - first.hits, MAX_UI_VERTEX_BUFFERS as u64);
            plan.commands.clear();
            backend.prepare_draw_buffers(&plan).unwrap();
            assert_eq!(backend.ui_vertex_upload_stats().resident_buffers, 0);
            assert_eq!(backend.ui_vertex_upload_stats().resident_bytes, 0);
        }

        fn b18_ui_plan(vertices: UiVertexStream) -> RhiCommandPlan {
            RhiCommandPlan {
                graph_schedule: None,
                frame_identity: None,
                packet_content_version: None,
                frame_index: 1,
                graph_id: "b18-partial-ui-upload".into(),
                target_kind: "offscreen".into(),
                commands: vec![RhiCommand::Draw {
                    target: "offscreen".into(),
                    draw_kind: RhiDrawKind::UiComposition,
                    vertex_count: vertices.len() as u32,
                    payload: RhiDrawPayload::UiComposition {
                        stage: "screen_overlay".into(),
                        item_count: 1001,
                        text_count: 0,
                        image_count: 0,
                        glyph_count: 0,
                        font_atlas_id: None,
                        text_pass_inserted: false,
                        debug_label: "b18".into(),
                        pipeline_key: "ui".into(),
                        texture: None,
                        font_render_mode: None,
                        font_page_index: None,
                        vertices,
                    },
                }]
                .into(),
                diagnostics: Vec::new(),
            }
        }

        #[test]
        #[ignore = "real GPU required"]
        fn gpu_geometry_only_frame_skips_empty_ui_work_but_cleans_old_ui() {
            let graph = crate::rhi_command_plan::tests::retained_geometry_graph(2);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
            let first = backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap();
            UI_PREPARATION_CALLS.with(|n| n.set(0));
            assert_eq!(
                backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap(),
                first
            );
            assert_eq!(
                UI_PREPARATION_CALLS.with(|n| n.get()),
                0,
                "geometry-only replay prepared per-command empty UI buffers"
            );
            let ui = b18_ui_plan(UiVertexStream::from_chunks(vec![Arc::new(
                quad_vertices_for_test(),
            )]));
            backend.prepare_draw_buffers(&ui).unwrap();
            assert!(!backend.ui_vertices.is_empty());
            UI_PREPARATION_CALLS.with(|n| n.set(0));
            assert_eq!(
                backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap(),
                first
            );
            assert_eq!(
                UI_PREPARATION_CALLS.with(|n| n.get()),
                2,
                "old UI residents require the original cleanup path"
            );
            assert!(backend.ui_vertices.is_empty());
            UI_PREPARATION_CALLS.with(|n| n.set(0));
            assert_eq!(
                backend.render_plan_to_rgba_bytes(&plan, 640, 480).unwrap(),
                first
            );
            assert_eq!(UI_PREPARATION_CALLS.with(|n| n.get()), 0);
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
        }

        #[test]
        fn gpu_joint_ui_parameter_baseline_is_part_of_resident_admission() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            assert_eq!(
                backend.set_geometry_visual_resident_limit(4096 * 64 - 1),
                Err("geometry.visual_resident_budget".into())
            );
        }

        #[test]
        fn gpu_joint_ui_new_vertices_cannot_bypass_the_shared_pool() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            backend
                .set_geometry_visual_resident_limit(4096 * 64 + 6 * 32 - 1)
                .unwrap();
            let plan = b18_ui_plan(UiVertexStream::from_chunks(vec![Arc::new(
                quad_vertices_for_test(),
            )]));
            assert_eq!(
                backend.prepare_draw_buffers(&plan).unwrap_err(),
                "geometry.visual_resident_budget"
            );
            assert!(
                backend.ui_vertices.is_empty(),
                "UI allocated outside the joint resident limit"
            );
        }

        #[test]
        fn gpu_joint_ui_prepared_but_unsubmitted_buffers_survive_eviction_until_shutdown_completion(
        ) {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            let baseline = backend.geometry_backing_bytes();
            let mut plan = b18_ui_plan(UiVertexStream::from_chunks(vec![Arc::new(
                quad_vertices_for_test(),
            )]));
            backend.prepare_draw_buffers(&plan).unwrap();
            assert_eq!(
                backend.geometry_backing_bytes(),
                baseline + 6 * 32,
                "resident and queued aliases must count one native buffer"
            );
            plan.commands.clear();
            backend.prepare_draw_buffers(&plan).unwrap();
            assert!(backend.ui_vertices.is_empty());
            assert_eq!(backend.geometry_backing_bytes(), baseline + 6 * 32);
            assert_eq!(backend.gpu_lifecycle_stats().submitted, 0);
            let finished = backend
                .shutdown_gpu(std::time::Duration::from_secs(2))
                .unwrap();
            assert_eq!(
                finished.submitted, 1,
                "queued writes require a real tracked flush"
            );
            assert_eq!(finished.completed, finished.submitted);
            assert!(backend.pending_draw_buffers.is_empty());
            assert_eq!(backend.geometry_backing_bytes(), baseline);
        }

        #[test]
        fn e7_real_gpu_reuses_alternating_bounded_ui_streams_when_adapter_exists() {
            let mut backend = match RealWgpuBackend::new_offscreen(8, 8) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let a = UiVertexStream::from_chunks(vec![Arc::new(quad_vertices_for_test())]);
            let mut b_vertices = quad_vertices_for_test();
            b_vertices[0].position[0] = crate::render_graph::OrderedF32::from(0.25);
            let b = UiVertexStream::from_chunks(vec![Arc::new(b_vertices)]);
            let plan_a = b18_ui_plan(a);
            let plan_b = b18_ui_plan(b);

            backend.prepare_draw_buffers(&plan_a).unwrap();
            let first_a = backend.ui_vertex_upload_stats();
            assert_eq!(first_a.full_uploads, 1);
            backend.prepare_draw_buffers(&plan_b).unwrap();
            let first_b = backend.ui_vertex_upload_stats();
            assert_eq!(first_b.full_uploads - first_a.full_uploads, 1);
            backend.prepare_draw_buffers(&plan_a).unwrap();
            let second_a = backend.ui_vertex_upload_stats();

            assert_eq!(second_a.full_uploads, first_b.full_uploads);
            assert_eq!(second_a.hits - first_b.hits, 1);
            assert_eq!(second_a.resident_buffers, 2);
        }

        #[test]
        fn e7_real_gpu_does_not_retain_large_alternating_streams_when_adapter_exists() {
            let mut backend = match RealWgpuBackend::new_offscreen(8, 8) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let large_plan = |vertices: UiVertexStream| {
                let mut plan = b18_ui_plan(vertices);
                let RhiCommand::Draw { payload, .. } = &mut plan.commands[0] else {
                    panic!("expected draw command");
                };
                let RhiDrawPayload::UiComposition { item_count, .. } = payload else {
                    panic!("expected UI composition");
                };
                *item_count = 4_097;
                plan
            };
            let a = UiVertexStream::from_chunks(vec![Arc::new(quad_vertices_for_test())]);
            let mut b_vertices = quad_vertices_for_test();
            b_vertices[0].position[0] = crate::render_graph::OrderedF32::from(0.25);
            let b = UiVertexStream::from_chunks(vec![Arc::new(b_vertices)]);
            let plan_a = large_plan(a);
            let plan_b = large_plan(b);

            backend.prepare_draw_buffers(&plan_a).unwrap();
            backend.prepare_draw_buffers(&plan_b).unwrap();
            let first_b = backend.ui_vertex_upload_stats();
            assert_eq!(first_b.resident_buffers, 1);
            backend.prepare_draw_buffers(&plan_a).unwrap();
            let second_a = backend.ui_vertex_upload_stats();

            assert_eq!(second_a.full_uploads - first_b.full_uploads, 1);
            assert_eq!(second_a.resident_buffers, 1);
        }

        #[test]
        fn b18_real_gpu_updates_one_chunk_and_rejects_stale_or_external_lineage_when_adapter_exists(
        ) {
            let mut backend = match RealWgpuBackend::new_offscreen(8, 8) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let chunks = (0..1001)
                .map(|_| Arc::new(quad_vertices_for_test()))
                .collect::<Vec<_>>();
            let initial = UiVertexStream::from_chunks(chunks.clone());
            let initial_plan = b18_ui_plan(initial.clone());
            backend.prepare_draw_buffers(&initial_plan).unwrap();
            let first = backend.ui_vertex_upload_stats();
            let vertex_bytes = std::mem::size_of::<RuntimeVertex>();
            assert_eq!(first.full_uploads, 1);
            assert_eq!(first.partial_uploads, 0);
            assert_eq!(first.full_upload_cold_or_evicted, 1);
            assert_eq!(first.full_upload_untrusted_lineage, 0);
            assert_eq!(first.uploaded_bytes, (1001 * 6 * vertex_bytes) as u64);

            let mut changed_chunks = chunks.clone();
            let mut changed_quad = quad_vertices_for_test();
            changed_quad[0].position[0] = crate::render_graph::OrderedF32::from(0.25);
            changed_chunks[500] = Arc::new(changed_quad);
            let mut changed = UiVertexStream::from_chunks(changed_chunks);
            assert_eq!(changed.continue_from(&initial), (1000, 1));
            let changed_plan = b18_ui_plan(changed.clone());
            backend.prepare_draw_buffers(&changed_plan).unwrap();
            let partial = backend.ui_vertex_upload_stats();
            assert_eq!(partial.full_uploads, first.full_uploads);
            assert_eq!(partial.partial_uploads - first.partial_uploads, 1);
            assert_eq!(
                partial.uploaded_bytes - first.uploaded_bytes,
                (6 * vertex_bytes) as u64
            );
            assert_eq!(
                partial.partial_uploaded_bytes - first.partial_uploaded_bytes,
                (6 * vertex_bytes) as u64
            );

            backend.prepare_draw_buffers(&changed_plan).unwrap();
            let clean = backend.ui_vertex_upload_stats();
            assert_eq!(clean.uploads, partial.uploads);
            assert_eq!(clean.uploaded_bytes, partial.uploaded_bytes);
            assert_eq!(clean.hits - partial.hits, 1);

            let mut skipped = UiVertexStream::from_chunks(chunks.clone());
            skipped.continue_from(&changed);
            let mut stale_chunks = chunks.clone();
            let mut stale_quad = quad_vertices_for_test();
            stale_quad[1].position[1] = crate::render_graph::OrderedF32::from(0.5);
            stale_chunks[1] = Arc::new(stale_quad);
            let mut stale = UiVertexStream::from_chunks(stale_chunks);
            stale.continue_from(&skipped);
            backend.prepare_draw_buffers(&b18_ui_plan(stale)).unwrap();
            let stale_stats = backend.ui_vertex_upload_stats();
            assert_eq!(stale_stats.full_uploads - clean.full_uploads, 1);
            assert_eq!(
                stale_stats.full_upload_untrusted_lineage - clean.full_upload_untrusted_lineage,
                1
            );

            let decoded: UiVertexStream =
                serde_json::from_str(&serde_json::to_string(&changed).unwrap()).unwrap();
            backend.prepare_draw_buffers(&b18_ui_plan(decoded)).unwrap();
            let external = backend.ui_vertex_upload_stats();
            assert_eq!(external.full_uploads - stale_stats.full_uploads, 1);
            assert_eq!(
                external.full_upload_cold_or_evicted - stale_stats.full_upload_cold_or_evicted,
                1
            );
        }

        #[test]
        fn real_wgpu_offscreen_backend_can_execute_basic_plan_when_adapter_exists() {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-1", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: "draw-main".to_string(),
                pass_name: "Draw Main".to_string(),
                pass_kind: RenderPassKind::DrawSpriteBasic,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([0.0, 0.0, 0.0, 1.0]),
                    },
                    RenderPassCommand::DrawSpriteBasic {
                        target,
                        sprite_ref: "sprite-a".to_string(),
                        material_ref: None,
                        sort_key: "sort".to_string(),
                    },
                ]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let report = backend.execute_plan(&plan);

            assert_eq!(report.backend_kind, "wgpu");
            assert_eq!(report.draw_count, 1);
            assert!(report
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "backend_unavailable"));
        }

        #[test]
        fn real_wgpu_backend_can_reuse_shared_device_queue_for_viewport_texture() {
            let source_backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let mut backend = RealWgpuBackend::from_shared_device_queue(
                &source_backend.device,
                &source_backend.queue,
                wgpu::TextureFormat::Rgba8Unorm,
                16,
                16,
                "shared-test",
            );
            let target = "viewport-main".to_string();
            let texture = source_backend
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("runtime-shared-viewport-test-texture"),
                    size: wgpu::Extent3d {
                        width: 16,
                        height: 16,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut graph = RenderGraph::new("graph-shared-texture", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: "present-shared".to_string(),
                pass_name: "Present Shared".to_string(),
                pass_kind: RenderPassKind::Present,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![RenderPassCommand::Clear {
                    target,
                    color: color([0.1, 0.2, 0.3, 1.0]),
                }]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let report = backend.execute_plan_to_surface_view(&plan, &view);

            assert_eq!(report.backend_kind, "wgpu");
            assert_eq!(report.present_count, 1);
            assert!(report
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "backend_unavailable"));
        }

        #[test]
        fn real_wgpu_screenshot_readback_returns_rgba_bytes_when_adapter_exists() {
            let mut backend = match RealWgpuBackend::new_offscreen(8, 8) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-screenshot", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 8, 8));
            graph.passes.push(RenderPass {
                pass_id: "present-main".to_string(),
                pass_name: "Present Main".to_string(),
                pass_kind: RenderPassKind::Present,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![RenderPassCommand::Clear {
                    target,
                    color: color([0.2, 0.4, 0.6, 1.0]),
                }]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let rgba = backend.render_plan_to_rgba_bytes(&plan, 8, 8).unwrap();

            assert_eq!(rgba.len(), 8 * 8 * 4);
            assert!(rgba.iter().any(|byte| *byte != 0));
        }

        fn s2d_transfer_clear_plan() -> RhiCommandPlan {
            RhiCommandPlan {
                frame_identity: None,
                graph_schedule: None,
                packet_content_version: None,
                frame_index: 1,
                graph_id: "transfer-test".into(),
                target_kind: "offscreenTexture".into(),
                commands: vec![RhiCommand::Clear {
                    target: "offscreen".into(),
                    color: color([0.2, 0.4, 0.6, 1.0]),
                }]
                .into(),
                diagnostics: vec![],
            }
        }

        #[test]
        fn s2d_transfer_output_and_readback_obey_joint_backing_before_allocation() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            let baseline = backend.geometry_backing_bytes();
            backend
                .set_geometry_visual_resident_limit(baseline + 255)
                .unwrap();
            assert_eq!(
                backend
                    .render_plan_to_rgba_bytes(&s2d_transfer_clear_plan(), 8, 8)
                    .unwrap_err(),
                "geometry.visual_resident_budget"
            );
            assert_eq!(backend.gpu_lifecycle_stats().submitted, 0);
            assert_eq!(backend.geometry_backing_bytes(), baseline);
            backend
                .set_geometry_visual_resident_limit(baseline + 256 + 2047)
                .unwrap();
            assert_eq!(
                backend
                    .render_plan_to_rgba_bytes(&s2d_transfer_clear_plan(), 8, 8)
                    .unwrap_err(),
                "geometry.visual_resident_budget"
            );
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(backend.geometry_backing_bytes(), baseline);
        }

        #[test]
        fn s2d_transfer_readback_copy_uses_original_submission_completion() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            let baseline = backend.geometry_backing_bytes();
            let plan = s2d_transfer_clear_plan();
            let (rgba, draw) = backend
                .render_plan_to_rgba_bytes_with_receipt(&plan, 8, 8, None)
                .unwrap();
            assert_eq!(draw.frame_index, plan.frame_index);
            assert_eq!(draw.serial, 1);
            let copy = backend.last_gpu_submission().unwrap();
            assert_eq!(copy.frame_index, 0);
            assert_eq!(copy.serial, draw.serial + 1);
            assert_eq!(rgba.len(), 256);
            assert_eq!(&rgba[..4], &[51, 102, 153, 255]);
            let stats = backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(
                stats.submitted, 2,
                "draw and readback copy must both use the original ledger"
            );
            assert_eq!(stats.submitted, stats.completed);
            assert_eq!(stats.retiring_bytes, 0);
            assert_eq!(backend.geometry_backing_bytes(), baseline);
        }

        #[test]
        fn surface_readiness_accounts_for_ui_uploads_before_rendering() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            backend
                .set_geometry_upload_budget(GeometryUploadBudget {
                    resident_bytes: 1024 * 1024,
                    staging_bytes: 1024,
                    bytes_per_frame: 192,
                    max_tickets: 8,
                })
                .unwrap();
            backend.bind_geometry_upload_presentation(1).unwrap();
            backend.geometry.admit_visual_upload(1, 4).unwrap();
            let source = UiVertexStream::from_chunks(vec![Arc::new(quad_vertices_for_test())]);
            let plan = b18_ui_plan(source);
            assert!(
                !backend
                    .prepare_surface_resources(&plan, [8, 8], None)
                    .unwrap(),
                "ready must reserve the actual UI write in the shared allowance"
            );
            assert_eq!(backend.geometry_upload_snapshot().frame_uploaded_bytes, 4);
            let target = backend.create_output_texture(8, 8).unwrap();
            let mut clear = plan.clone();
            clear.frame_index = 0;
            clear.commands = vec![RhiCommand::Clear {
                target: "offscreen".into(),
                color: [OrderedF32::from(0.); 4],
            }]
            .into();
            backend
                .execute_loading_clear_to_surface_view(
                    &clear,
                    &target.create_view(&Default::default()),
                )
                .unwrap();
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            backend.bind_geometry_upload_presentation(2).unwrap();
            let mut repeated = plan.clone();
            repeated.commands.push(plan.commands[0].clone());
            assert!(
                backend
                    .prepare_surface_resources(&repeated, [8, 8], None)
                    .unwrap(),
                "the same immutable UI stream uploads once across two actual draws"
            );
            let pixels = backend.render_plan_to_rgba_bytes(&repeated, 8, 8).unwrap();
            assert_eq!(pixels.len(), 8 * 8 * 4);
            assert_eq!(backend.geometry_upload_snapshot().frame_uploaded_bytes, 192);
            assert_eq!(backend.ui_vertex_upload_stats().uploaded_bytes, 192);
            backend.release_output_texture(target);
            let final_state = backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(final_state.in_flight, 0);
        }

        #[test]
        fn s2d_transfer_ui_writes_share_texture_upload_allowance_and_rejection_is_retryable() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).unwrap();
            backend
                .set_geometry_upload_budget(GeometryUploadBudget {
                    resident_bytes: 1024 * 1024,
                    staging_bytes: 1024,
                    bytes_per_frame: 192,
                    max_tickets: 8,
                })
                .unwrap();
            backend.bind_geometry_upload_presentation(1).unwrap();
            backend.geometry.admit_visual_upload(1, 4).unwrap();
            let source = UiVertexStream::from_chunks(vec![Arc::new(quad_vertices_for_test())]);
            let plan = b18_ui_plan(source);
            let before = backend.geometry_backing_bytes();
            assert!(backend.prepare_draw_buffers(&plan).is_err());
            assert_eq!(backend.geometry_backing_bytes(), before);
            backend.bind_geometry_upload_presentation(2).unwrap();
            backend.prepare_draw_buffers(&plan).unwrap();
            assert_eq!(backend.geometry_upload_snapshot().frame_uploaded_bytes, 192);
        }

        #[test]
        fn real_wgpu_cooked_texture_preserves_srgb_format() {
            let mut backend = RealWgpuBackend::new_offscreen(8, 8)
                .expect("GPU required for cooked texture format evidence");
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 77,
                generation: 1,
            };
            let mut payload = crate::runtime_texture::RuntimeTexturePayload {
                asset_id: "mid-gray".into(),
                cooked_asset_id: "mid-gray.cooked".into(),
                width: 1,
                height: 1,
                format: "rgba8UnormSrgb".into(),
                color_space: "srgb".into(),
                mip_count: 1,
                rgba8: vec![40, 44, 52, 255].into(),
                sampler: "linearClamp".into(),
                source_hash: "fixture".into(),
            };
            backend.register_cooked_texture(handle, &payload).unwrap();
            assert_eq!(
                backend.textures[&handle]._texture.format(),
                wgpu::TextureFormat::Rgba8UnormSrgb
            );
            payload.format = "rgba8Unorm".into();
            payload.color_space = "linear".into();
            backend.register_cooked_texture(handle, &payload).unwrap();
            assert_eq!(
                backend.textures[&handle]._texture.format(),
                wgpu::TextureFormat::Rgba8Unorm
            );
        }

        #[test]
        fn s2d_bc3_missing_device_feature_rejects_without_allocating() {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter =
                pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                    .unwrap();
            assert!(!device
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC));
            let mut backend = RealWgpuBackend::from_shared_device_queue(
                &device,
                &queue,
                wgpu::TextureFormat::Rgba8Unorm,
                8,
                8,
                "BC disabled",
            );
            let payload = crate::runtime_texture::RuntimeTexturePayload {
                asset_id: "bc3".into(),
                cooked_asset_id: "cooked-bc3".into(),
                width: 4,
                height: 4,
                format: "bc3RgbaUnorm".into(),
                color_space: "linear".into(),
                mip_count: 3,
                rgba8: vec![0; 48].into(),
                sampler: "nearestClamp".into(),
                source_hash: "source".into(),
            };
            let handle = crate::runtime_texture::runtime_texture_render_handle("bc3");
            assert_eq!(
                backend
                    .register_cooked_texture(handle, &payload)
                    .unwrap_err(),
                "wgpu.texture_compression_unsupported"
            );
            assert!(!backend.textures.contains_key(&handle));
        }

        #[test]
        fn real_wgpu_geometry_textures_reuse_and_release_only_owned_uploads() {
            use crate::render_asset_production::RuntimeRenderAssetProducer;
            use crate::render_contract::RenderBindingKind;
            use crate::render_resource::RenderResourceManager;
            let mut backend = RealWgpuBackend::new_offscreen(8, 8).expect("GPU required");
            let mut manager = RenderResourceManager::new();
            let payload = crate::runtime_texture::RuntimeTexturePayload {
                asset_id: "geometry-direction".into(),
                cooked_asset_id: "direction.cooked".into(),
                width: 1,
                height: 1,
                format: "rgba8UnormSrgb".into(),
                color_space: "srgb".into(),
                mip_count: 1,
                rgba8: vec![255, 0, 0, 255].into(),
                sampler: "nearestClamp".into(),
                source_hash: "sha256:direction".into(),
            };
            let prepared = RuntimeRenderAssetProducer::new()
                .prepare_texture_payload(
                    1,
                    payload.clone(),
                    RenderBindingKind::Texture,
                    &mut manager,
                )
                .unwrap();
            let shared = crate::runtime_texture::runtime_texture_render_handle("shared-ui");
            backend.register_cooked_texture(shared, &payload).unwrap();
            backend
                .prepare_geometry_textures(std::slice::from_ref(&prepared))
                .unwrap();
            let texture = Arc::clone(&backend.textures[&prepared.handle.raw()]._texture);
            backend
                .prepare_geometry_textures(std::slice::from_ref(&prepared))
                .unwrap();
            assert!(Arc::ptr_eq(
                &texture,
                &backend.textures[&prepared.handle.raw()]._texture
            ));
            backend.prepare_geometry_textures(&[]).unwrap();
            assert!(!backend.textures.contains_key(&prepared.handle.raw()));
            assert!(backend.textures.contains_key(&shared));
        }

        #[test]
        fn real_wgpu_textured_sprite_samples_registered_rgba8_texture() {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 7,
                generation: 1,
            };
            backend
                .register_rgba8_texture(handle, 1, 1, &[255, 0, 0, 255], "nearestClamp")
                .unwrap();
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-textured-sprite", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: "draw-textured-sprite".to_string(),
                pass_name: "Draw Textured Sprite".to_string(),
                pass_kind: RenderPassKind::DrawSpriteTextured,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([0.0, 0.0, 0.0, 1.0]),
                    },
                    RenderPassCommand::DrawSpriteTextured {
                        target,
                        sprite_ref: "sprite-red".to_string(),
                        material_ref: None,
                        sort_key: "sort".to_string(),
                        texture: Some(handle),
                        binding: None,
                        fallback_used: false,
                        vertices: Vec::new(),
                    },
                ]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let rgba = backend.render_plan_to_rgba_bytes(&plan, 16, 16).unwrap();

            assert_eq!(rgba.len(), 16 * 16 * 4);
            assert!(rgba
                .chunks_exact(4)
                .any(|pixel| { pixel[0] > 30 && pixel[1] < 8 && pixel[2] < 8 && pixel[3] == 255 }));
        }

        #[test]
        fn real_wgpu_rejects_missing_texture_before_rendering_surface() {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let missing_handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 404,
                generation: 7,
            };
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-missing-ui-texture", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: "draw-missing-ui-texture".to_string(),
                pass_name: "Draw Missing UI Texture".to_string(),
                pass_kind: RenderPassKind::DrawUiComposition,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([1.0, 1.0, 1.0, 1.0]),
                    },
                    RenderPassCommand::DrawUiComposition {
                        target,
                        stage: "screen_overlay".to_string(),
                        item_count: 1,
                        text_count: 0,
                        image_count: 1,
                        glyph_count: 0,
                        font_atlas_id: None,
                        text_pass_inserted: false,
                        debug_label: "missing UI image".to_string(),
                        texture: Some(missing_handle),
                        font_render_mode: None,
                        font_page_index: None,
                        vertices: quad_vertices_for_test().into(),
                    },
                ]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let preflight_error = backend
                .validate_plan_texture_residency(&plan)
                .expect_err("missing handle must fail before encoder creation");
            assert!(preflight_error.contains("index: 404"));
            assert!(preflight_error.contains("generation: 7"));
            let report = backend.execute_plan(&plan);
            assert_eq!(report.submit_count, 0);
            assert_eq!(report.present_count, 0);
            assert!(report.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "wgpu.texture_binding_missing"
                    && diagnostic.severity == crate::engine_rhi::RhiBackendDiagnosticSeverity::Error
                    && diagnostic.message.contains("index: 404")
            }));
            let render_error = backend
                .render_plan_to_rgba_bytes(&plan, 16, 16)
                .expect_err("missing texture must not render a white fallback");
            assert_eq!(render_error, preflight_error);
        }

        #[test]
        fn real_wgpu_textured_ui_blends_rgba_texture_alpha() {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 8,
                generation: 1,
            };
            backend
                .register_rgba8_texture(handle, 1, 1, &[255, 0, 0, 128], "nearestClamp")
                .unwrap();
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-textured-ui-alpha", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: "draw-textured-ui-alpha".to_string(),
                pass_name: "Draw Textured UI Alpha".to_string(),
                pass_kind: RenderPassKind::DrawUiComposition,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([0.0, 0.0, 0.0, 1.0]),
                    },
                    RenderPassCommand::DrawUiComposition {
                        target,
                        stage: "screen_overlay".to_string(),
                        item_count: 1,
                        text_count: 0,
                        image_count: 1,
                        glyph_count: 0,
                        font_atlas_id: None,
                        text_pass_inserted: false,
                        debug_label: "alpha image".to_string(),
                        texture: Some(handle),
                        font_render_mode: None,
                        font_page_index: None,
                        vertices: quad_vertices_for_test().into(),
                    },
                ]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let rgba = backend.render_plan_to_rgba_bytes(&plan, 16, 16).unwrap();

            assert!(rgba
                .chunks_exact(4)
                .any(|pixel| { (110..=145).contains(&pixel[0]) && pixel[1] < 8 && pixel[2] < 8 }));
        }

        #[test]
        fn real_wgpu_backend_uses_payload_vertices_instead_of_placeholder_quad() {
            let expected = vec![
                RenderDrawVertex::new([-0.9, -0.8], [1.0, 0.0, 0.0, 1.0], [0.0, 1.0]),
                RenderDrawVertex::new([-0.4, -0.8], [0.0, 1.0, 0.0, 1.0], [1.0, 1.0]),
                RenderDrawVertex::new([-0.4, -0.2], [0.0, 0.0, 1.0, 1.0], [1.0, 0.0]),
            ];
            let payload = RhiDrawPayload::SpriteTextured {
                sprite_ref: "positioned-sprite".to_string(),
                material_ref: None,
                pipeline_key: "sprite2d.textured".to_string(),
                sort_key: "positioned".to_string(),
                texture: None,
                binding: None,
                fallback_used: true,
                vertices: expected.clone(),
            };

            let actual = RealWgpuBackend::vertices_for_draw(
                RhiDrawKind::SpriteTextured,
                expected.len() as u32,
                &payload,
            );

            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                assert_eq!(
                    actual.position,
                    expected.position.map(|value| value.to_f32())
                );
                assert_eq!(actual.color, expected.color.map(|value| value.to_f32()));
                assert_eq!(actual.uv, expected.uv.map(|value| value.to_f32()));
            }
        }

        #[test]
        fn real_wgpu_ui_composition_samples_registered_alpha_atlas() {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let handle = RenderResourceHandle {
                kind: RenderResourceKind::Texture,
                index: 9,
                generation: 1,
            };
            backend
                .register_alpha8_texture(handle, 2, 2, &[255, 0, 0, 255], "nearestClamp")
                .unwrap();
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-ui-alpha-atlas", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: "draw-ui-glyph".to_string(),
                pass_name: "Draw UI Glyph".to_string(),
                pass_kind: RenderPassKind::DrawUiComposition,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([0.0, 0.0, 0.0, 1.0]),
                    },
                    RenderPassCommand::DrawUiComposition {
                        target,
                        stage: "screen_overlay".to_string(),
                        item_count: 1,
                        text_count: 1,
                        image_count: 0,
                        glyph_count: 1,
                        font_atlas_id: Some("test-atlas".to_string()),
                        text_pass_inserted: true,
                        debug_label: "test glyph".to_string(),
                        texture: Some(handle),
                        font_render_mode: None,
                        font_page_index: None,
                        vertices: quad_vertices_for_test().into(),
                    },
                ]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let rgba = backend.render_plan_to_rgba_bytes(&plan, 16, 16).unwrap();

            let white_pixels = rgba
                .chunks_exact(4)
                .filter(|pixel| pixel[0] > 200 && pixel[1] > 200 && pixel[2] > 200)
                .count();
            assert!(white_pixels > 0 && white_pixels < 16 * 16, "{white_pixels}");
        }

        #[test]
        fn s2d_transfer_font_pages_progress_across_present_without_reallocating_array() {
            let mut backend = RealWgpuBackend::new_offscreen(16, 16).unwrap();
            let bundle = test_multi_page_font_bundle();
            backend
                .set_geometry_upload_budget(geometry::GeometryUploadBudget {
                    resident_bytes: 1024 * 1024,
                    staging_bytes: 4096,
                    bytes_per_frame: 16,
                    max_tickets: 4,
                })
                .unwrap();
            backend.bind_geometry_upload_presentation(1).unwrap();
            assert!(backend.register_font_texture_arrays(&bundle).is_err());
            assert_eq!(backend.geometry_upload_snapshot().uploaded_bytes, 8);
            backend.bind_geometry_upload_presentation(2).unwrap();
            assert!(backend.register_font_texture_arrays(&bundle).is_err());
            assert_eq!(
                backend.geometry_upload_snapshot().uploaded_bytes,
                24,
                "completed bitmap pages must not reupload; one MSDF page must advance"
            );
            let first = font_bundle_page_generation_render_handle(
                &bundle.metadata.font_bundle_id,
                FontBundleRenderMode::MsdfRgba8,
                2,
                bundle.metadata.generation,
            );
            let array = backend.textures[&first]._texture.clone();
            backend.bind_geometry_upload_presentation(3).unwrap();
            let report = backend.register_font_texture_arrays(&bundle).unwrap();
            assert_eq!(report.registered_page_handles.len(), 4);
            assert_eq!(backend.geometry_upload_snapshot().uploaded_bytes, 40);
            let second = font_bundle_page_generation_render_handle(
                &bundle.metadata.font_bundle_id,
                FontBundleRenderMode::MsdfRgba8,
                3,
                bundle.metadata.generation,
            );
            assert!(Arc::ptr_eq(&array, &backend.textures[&second]._texture));
            backend
                .shutdown_gpu(std::time::Duration::from_secs(5))
                .unwrap();
        }

        #[test]
        fn font_texture_array_registers_bitmap_and_msdf_layers_when_adapter_exists() {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };

            let report = backend
                .register_font_texture_arrays(&test_font_bundle())
                .unwrap();

            assert_eq!(report.bitmap_layer_count, 1);
            assert_eq!(report.msdf_layer_count, 1);
            assert_eq!(report.registered_page_handles.len(), 2);
            assert_ne!(
                report.registered_page_handles[0],
                report.registered_page_handles[1]
            );
        }

        #[test]
        fn standalone_font_page_texture_policy_is_gl_only() {
            assert!(uses_standalone_font_page_textures("Gl"));
            assert!(uses_standalone_font_page_textures("GL"));
            assert!(!uses_standalone_font_page_textures("Vulkan"));
            assert!(!uses_standalone_font_page_textures("Dx12"));
            assert!(!uses_standalone_font_page_textures("Metal"));
        }

        #[test]
        fn font_texture_gl_multi_page_samples_nonzero_bitmap_and_msdf_pages() {
            let mut backend =
                match RealWgpuBackend::new_offscreen_with_backends(16, 16, wgpu::Backends::GL) {
                    Ok(backend) => backend,
                    Err(error) => {
                        eprintln!("GL adapter unavailable; skipped: {error}");
                        return;
                    }
                };
            assert_eq!(backend.state.device_context.backend_name, "Gl");

            let bundle = test_multi_page_font_bundle();
            let report = backend.register_font_texture_arrays(&bundle).unwrap();
            assert_eq!(report.bitmap_layer_count, 2);
            assert_eq!(report.msdf_layer_count, 2);

            for (render_mode, zero_page, opaque_page) in [
                (FontBundleRenderMode::BitmapR8, 0, 1),
                (FontBundleRenderMode::MsdfRgba8, 2, 3),
            ] {
                let zero =
                    render_font_page_center_pixel(&mut backend, &bundle, render_mode, zero_page);
                let opaque =
                    render_font_page_center_pixel(&mut backend, &bundle, render_mode, opaque_page);
                assert!(
                    zero[1] < 8,
                    "{render_mode:?} page {zero_page} sampled nonzero coverage: {zero:?}"
                );
                assert!(
                    opaque[1] > 200,
                    "{render_mode:?} page {opaque_page} lost nonzero coverage: {opaque:?}"
                );
            }
        }

        #[test]
        fn font_generation_gpu_retirement_keeps_generations_until_explicit_retire() {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let first_bundle = test_font_bundle();
            let first = backend.register_font_texture_arrays(&first_bundle).unwrap();
            let mut second_bundle = test_font_bundle();
            second_bundle.metadata.generation = 2;
            let second = backend
                .register_font_texture_arrays(&second_bundle)
                .unwrap();

            assert_eq!(first.generation, 1);
            assert_eq!(second.generation, 2);
            assert!(first
                .registered_page_handles
                .iter()
                .all(|handle| backend.textures.contains_key(handle)));
            assert!(second
                .registered_page_handles
                .iter()
                .all(|handle| backend.textures.contains_key(handle)));

            assert_eq!(
                backend.retire_font_texture_arrays(&first),
                first.registered_page_handles.len()
            );
            assert!(first
                .registered_page_handles
                .iter()
                .all(|handle| !backend.textures.contains_key(handle)));
            assert!(second
                .registered_page_handles
                .iter()
                .all(|handle| backend.textures.contains_key(handle)));
        }

        #[test]
        fn aui_text_bitmap_pipeline_outputs_local_coverage_when_adapter_exists() {
            assert_font_pipeline_outputs_local_coverage(FontBundleRenderMode::BitmapR8);
        }

        #[test]
        fn aui_text_msdf_pipeline_outputs_local_coverage_when_adapter_exists() {
            assert_font_pipeline_outputs_local_coverage(FontBundleRenderMode::MsdfRgba8);
        }

        #[test]
        fn real_wgpu_bgra_readback_is_converted_to_rgba() {
            let source = match RealWgpuBackend::new_offscreen(8, 8) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let mut backend = RealWgpuBackend::from_shared_device_queue(
                &source.device,
                &source.queue,
                wgpu::TextureFormat::Bgra8Unorm,
                8,
                8,
                "bgra-readback-test",
            );
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-bgra-readback", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 8, 8));
            graph.passes.push(RenderPass {
                pass_id: "clear-red".to_string(),
                pass_name: "Clear Red".to_string(),
                pass_kind: RenderPassKind::Present,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![RenderPassCommand::Clear {
                    target,
                    color: color([0.8, 0.1, 0.05, 1.0]),
                }]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let rgba = backend.render_plan_to_rgba_bytes(&plan, 8, 8).unwrap();

            assert!(rgba[0] > rgba[2], "first pixel was {:?}", &rgba[..4]);
            assert!(rgba[0] > 150, "first pixel was {:?}", &rgba[..4]);
        }

        #[test]
        fn direct_surface_content_rect_keeps_contain_gutters_clear() {
            let mut backend = match RealWgpuBackend::new_offscreen(12, 20) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let target = "portrait-surface".to_string();
            let mut graph = RenderGraph::new("graph-content-rect", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 12, 20));
            graph.passes.push(RenderPass {
                pass_id: "draw-content".to_string(),
                pass_name: "Draw Content".to_string(),
                pass_kind: RenderPassKind::DrawUiComposition,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([0.0, 0.0, 0.0, 1.0]),
                    },
                    RenderPassCommand::DrawUiComposition {
                        target,
                        stage: "screen_overlay".to_string(),
                        item_count: 1,
                        text_count: 0,
                        image_count: 0,
                        glyph_count: 0,
                        font_atlas_id: None,
                        text_pass_inserted: false,
                        debug_label: "target-space-full-quad".to_string(),
                        texture: None,
                        font_render_mode: None,
                        font_page_index: None,
                        vertices: full_quad_vertices_for_test().into(),
                    },
                ]
                .into(),
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let rgba = backend
                .render_plan_to_rgba_bytes_with_rect(
                    &plan,
                    12,
                    20,
                    Some(GameViewRect::new(0.0, 2.0, 12.0, 16.0)),
                )
                .unwrap();
            let pixel = |x: usize, y: usize| &rgba[(y * 12 + x) * 4..(y * 12 + x + 1) * 4];

            assert_eq!(pixel(6, 0), &[0, 0, 0, 255]);
            assert!(
                pixel(6, 10)[1] > 200,
                "content pixel was {:?}",
                pixel(6, 10)
            );
            assert_eq!(pixel(6, 19), &[0, 0, 0, 255]);
        }

        fn full_quad_vertices_for_test() -> Vec<RenderDrawVertex> {
            let corners = [
                ([-1.0, -1.0], [0.0, 1.0]),
                ([1.0, -1.0], [1.0, 1.0]),
                ([1.0, 1.0], [1.0, 0.0]),
                ([-1.0, 1.0], [0.0, 0.0]),
            ];
            [0usize, 1, 2, 0, 2, 3]
                .into_iter()
                .map(|index| {
                    RenderDrawVertex::new(corners[index].0, [0.0, 1.0, 0.0, 1.0], corners[index].1)
                })
                .collect()
        }

        fn quad_vertices_for_test() -> Vec<RenderDrawVertex> {
            let corners = [
                ([-0.75, -0.75], [0.0, 1.0]),
                ([0.75, -0.75], [1.0, 1.0]),
                ([0.75, 0.75], [1.0, 0.0]),
                ([-0.75, 0.75], [0.0, 0.0]),
            ];
            [0usize, 1, 2, 0, 2, 3]
                .into_iter()
                .map(|index| {
                    RenderDrawVertex::new(corners[index].0, [1.0, 1.0, 1.0, 1.0], corners[index].1)
                })
                .collect()
        }

        fn test_font_bundle() -> RuntimeLoadedFontBundle {
            let page = |page_index: u32,
                        render_mode: FontBundleRenderMode,
                        format: &str,
                        byte_len: usize|
             -> CookedFontBundlePage {
                CookedFontBundlePage {
                    page_index,
                    render_mode,
                    format: format.to_string(),
                    width: 2,
                    height: 2,
                    byte_len,
                    sha256: format!("sha256:test-{page_index}"),
                    payload_path: format!("page-{page_index}.bin"),
                }
            };
            RuntimeLoadedFontBundle {
                metadata: CookedFontBundleAsset {
                    schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
                    font_bundle_id: "test-font-bundle".to_string(),
                    font_stack_id: "test-stack".to_string(),
                    generation: 1,
                    max_bitmap_pages: 1,
                    max_msdf_pages: 1,
                    legacy_mode: false,
                    fallback_used: false,
                    quality_gate_eligible: true,
                    pages: vec![
                        page(0, FontBundleRenderMode::BitmapR8, "R8Unorm", 4),
                        page(1, FontBundleRenderMode::MsdfRgba8, "Rgba8Unorm", 16),
                    ],
                    glyphs: Vec::new(),
                    kerning_adjustments: Vec::new(),
                    font_faces: Vec::new(),
                    shaping_recipe: None,
                    bundle_digest: "sha256:test-bundle".to_string(),
                },
                page_payloads: vec![
                    vec![255, 0, 0, 255],
                    vec![
                        255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255,
                    ],
                ],
                font_face_sources: Vec::new(),
            }
        }

        fn test_multi_page_font_bundle() -> RuntimeLoadedFontBundle {
            let page = |page_index: u32,
                        render_mode: FontBundleRenderMode,
                        format: &str,
                        byte_len: usize|
             -> CookedFontBundlePage {
                CookedFontBundlePage {
                    page_index,
                    render_mode,
                    format: format.to_string(),
                    width: 2,
                    height: 2,
                    byte_len,
                    sha256: format!("sha256:multi-page-{page_index}"),
                    payload_path: format!("multi-page-{page_index}.bin"),
                }
            };
            RuntimeLoadedFontBundle {
                metadata: CookedFontBundleAsset {
                    schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.to_string(),
                    font_bundle_id: "test-multi-page-font-bundle".to_string(),
                    font_stack_id: "test-stack".to_string(),
                    generation: 1,
                    max_bitmap_pages: 2,
                    max_msdf_pages: 2,
                    legacy_mode: false,
                    fallback_used: false,
                    quality_gate_eligible: true,
                    pages: vec![
                        page(0, FontBundleRenderMode::BitmapR8, "R8Unorm", 4),
                        page(1, FontBundleRenderMode::BitmapR8, "R8Unorm", 4),
                        page(2, FontBundleRenderMode::MsdfRgba8, "Rgba8Unorm", 16),
                        page(3, FontBundleRenderMode::MsdfRgba8, "Rgba8Unorm", 16),
                    ],
                    glyphs: Vec::new(),
                    kerning_adjustments: Vec::new(),
                    font_faces: Vec::new(),
                    shaping_recipe: None,
                    bundle_digest: "sha256:test-multi-page-bundle".to_string(),
                },
                page_payloads: vec![vec![0; 4], vec![255; 4], vec![0; 16], vec![255; 16]],
                font_face_sources: Vec::new(),
            }
        }

        fn render_font_page_center_pixel(
            backend: &mut RealWgpuBackend,
            bundle: &RuntimeLoadedFontBundle,
            render_mode: FontBundleRenderMode,
            page_index: u32,
        ) -> [u8; 4] {
            let handle = font_bundle_page_generation_render_handle(
                &bundle.metadata.font_bundle_id,
                render_mode,
                page_index,
                bundle.metadata.generation,
            );
            let target = format!("font-page-{page_index}");
            let mut graph = RenderGraph::new(format!("graph-{target}"), page_index as u64 + 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: format!("draw-font-page-{page_index}"),
                pass_name: "Draw Font Page".to_string(),
                pass_kind: RenderPassKind::DrawUiComposition,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([0.0, 0.0, 0.0, 1.0]),
                    },
                    RenderPassCommand::DrawUiComposition {
                        target,
                        stage: "screen_overlay".to_string(),
                        item_count: 1,
                        text_count: 1,
                        image_count: 0,
                        glyph_count: 1,
                        font_atlas_id: Some(bundle.metadata.font_bundle_id.clone()),
                        text_pass_inserted: true,
                        debug_label: format!("font page {page_index}"),
                        texture: Some(handle),
                        font_render_mode: Some(render_mode),
                        font_page_index: Some(page_index),
                        vertices: quad_vertices_for_test().into(),
                    },
                ]
                .into(),
                debug_source: None,
            });
            let rgba = backend
                .render_plan_to_rgba_bytes(&compile_render_graph_to_rhi_plan(&graph), 16, 16)
                .unwrap();
            rgba[(8 * 16 + 8) * 4..(8 * 16 + 9) * 4].try_into().unwrap()
        }

        fn assert_font_pipeline_outputs_local_coverage(render_mode: FontBundleRenderMode) {
            let mut backend = match RealWgpuBackend::new_offscreen(16, 16) {
                Ok(backend) => backend,
                Err(_) => return,
            };
            let bundle = test_font_bundle();
            backend.register_font_texture_arrays(&bundle).unwrap();
            let page_index = match render_mode {
                FontBundleRenderMode::BitmapR8 => 0,
                FontBundleRenderMode::MsdfRgba8 => 1,
            };
            let handle = font_bundle_page_generation_render_handle(
                &bundle.metadata.font_bundle_id,
                render_mode,
                page_index,
                bundle.metadata.generation,
            );
            let target = "offscreen".to_string();
            let mut graph = RenderGraph::new("graph-font-pipeline", 1);
            graph.output_target = Some(target.clone());
            graph
                .resources
                .push(RenderResource::surface_backbuffer(target.clone(), 16, 16));
            graph.passes.push(RenderPass {
                pass_id: "draw-font".to_string(),
                pass_name: "Draw Font".to_string(),
                pass_kind: RenderPassKind::DrawUiComposition,
                view_id: "view-1".to_string(),
                reads: Vec::new(),
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![
                    RenderPassCommand::Clear {
                        target: target.clone(),
                        color: color([0.0, 0.0, 0.0, 1.0]),
                    },
                    RenderPassCommand::DrawUiComposition {
                        target,
                        stage: "screen_overlay".to_string(),
                        item_count: 1,
                        text_count: 1,
                        image_count: 0,
                        glyph_count: 1,
                        font_atlas_id: Some(bundle.metadata.font_bundle_id.clone()),
                        text_pass_inserted: true,
                        debug_label: "font pipeline glyph".to_string(),
                        texture: Some(handle),
                        font_render_mode: Some(render_mode),
                        font_page_index: Some(page_index),
                        vertices: quad_vertices_for_test().into(),
                    },
                ]
                .into(),
                debug_source: None,
            });

            let rgba = backend
                .render_plan_to_rgba_bytes(&compile_render_graph_to_rhi_plan(&graph), 16, 16)
                .unwrap();
            let visible = rgba
                .chunks_exact(4)
                .filter(|pixel| pixel[0] > 100 && pixel[1] > 100 && pixel[2] > 100)
                .count();
            assert!(
                visible > 0 && visible < 16 * 16,
                "{render_mode:?}: {visible}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_graph::{
        color, RenderGraph, RenderPass, RenderPassCommand, RenderPassKind, RenderResource,
    };
    use crate::rhi_command_plan::compile_render_graph_to_rhi_plan;

    fn plan() -> RhiCommandPlan {
        let target = "surface-main".to_string();
        let mut graph = RenderGraph::new("graph-1", 1);
        graph.output_target = Some(target.clone());
        graph
            .resources
            .push(RenderResource::surface_backbuffer(target.clone(), 640, 480));
        graph.passes.push(RenderPass {
            pass_id: "draw-main".to_string(),
            pass_name: "Draw Main".to_string(),
            pass_kind: RenderPassKind::DrawSpriteTextured,
            view_id: "view-1".to_string(),
            reads: Vec::new(),
            writes: vec![target.clone()],
            color_targets: vec![target.clone()],
            depth_target: None,
            commands: vec![
                RenderPassCommand::Clear {
                    target: target.clone(),
                    color: color([0.0, 0.0, 0.0, 1.0]),
                },
                RenderPassCommand::DrawSpriteTextured {
                    target,
                    sprite_ref: "sprite-a".to_string(),
                    material_ref: Some("material-a".to_string()),
                    sort_key: "sort".to_string(),
                    texture: None,
                    binding: None,
                    fallback_used: true,
                    vertices: Vec::new(),
                },
            ]
            .into(),
            debug_source: None,
        });
        compile_render_graph_to_rhi_plan(&graph)
    }

    #[test]
    #[cfg(feature = "real-wgpu")]
    fn repeated_geometry_intent_keeps_full_state_without_revisiting_values() {
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
        let payload = plan
            .commands
            .iter()
            .find_map(|command| {
                let crate::rhi_command_plan::RhiCommand::Draw {
                    payload: payload @ RhiDrawPayload::Geometry { .. },
                    ..
                } = command
                else {
                    return None;
                };
                Some(payload)
            })
            .unwrap();
        let mut full = WgpuBackend::new_unavailable();
        for _ in 0..128 {
            full.record_draw(payload);
        }
        let mut replay = WgpuBackend::new_unavailable();
        GEOMETRY_INTENT_VALUE_VISITS.with(|count| count.set(0));
        replay.record_draw(payload);
        for _ in 1..128 {
            replay.record_reused_geometry_intent(payload);
        }
        assert_eq!(GEOMETRY_INTENT_VALUE_VISITS.with(|count| count.get()), 1);
        assert_eq!(
            replay, full,
            "reused intent lost counters or unique references/pipelines"
        );
        assert_eq!(replay.finish_report(&plan), full.finish_report(&plan));
        let other = RhiDrawPayload::TestGeometry {
            debug_label: "fallback".into(),
        };
        full.record_draw(&other);
        replay.record_reused_geometry_intent(&other);
        assert_eq!(replay, full, "non-geometry must retain full recording");
    }

    #[test]
    fn default_wgpu_backend_reports_unavailable_but_records_intent() {
        let mut backend = WgpuBackend::new_unavailable();

        let report = backend.execute_plan(&plan());

        assert_eq!(report.backend_kind, "wgpu");
        assert_eq!(report.draw_count, 1);
        assert_eq!(report.present_count, 1);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "backend_unavailable"));
        assert_eq!(
            backend.resource_registry_report().sprite_refs,
            vec!["sprite-a".to_string()]
        );
        assert_eq!(
            backend.pipeline_cache_report().requested_keys,
            vec!["sprite-basic.default".to_string()]
        );
    }

    #[test]
    fn wgpu_backend_device_context_defaults_to_feature_disabled() {
        let backend = WgpuBackend::new_unavailable();

        assert_eq!(backend.device_context().backend_name, "wgpu");
        assert!(!backend.device_context().real_wgpu_enabled);
    }
}
