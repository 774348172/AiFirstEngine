use std::collections::BTreeSet;

use crate::engine_rhi::{
    EngineRhiBackend, EngineRhiDrawCall, EngineRhiFrame, RhiBackendDiagnostic, RhiBackendReport,
};
use crate::render_graph::OrderedF32;
use crate::rhi_command_plan::{RhiCommandPlan, RhiDrawPayload};

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

    fn record_payload(&mut self, payload: &RhiDrawPayload) {
        match payload {
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
                    self.uploaded_resource_count += binding.resources.len();
                }
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
        self.draw_count += 1;
        self.record_payload(&draw_call.payload);
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
pub mod real {
    use super::*;
    use crate::font_bundle::{FontBundleRenderMode, RuntimeLoadedFontBundle};
    use crate::game_view_presentation::GameViewRect;
    use crate::render_resource::{RenderResourceHandle, RenderResourceKind};
    use crate::rhi_command_plan::{RhiCommand, RhiDrawKind};
    use crate::runtime_renderer::font_bundle_page_generation_render_handle;
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

    struct ResidentTexture {
        _texture: Arc<wgpu::Texture>,
        _view: wgpu::TextureView,
        _sampler: wgpu::Sampler,
        bind_group: wgpu::BindGroup,
        _width: u32,
        _height: u32,
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

    pub struct RealWgpuBackend {
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
        state: WgpuBackend,
    }

    impl RealWgpuBackend {
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

        pub fn new_offscreen(width: u32, height: u32) -> Result<Self, String> {
            if width == 0 || height == 0 {
                return Err("wgpu_backend.zero_sized_target".to_string());
            }
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
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
            let (device, queue) = pollster::block_on(
                adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("runtime-wgpu-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                }),
            )
            .map_err(|error| format!("wgpu_backend.request_device_failed:{error}"))?;
            let format = wgpu::TextureFormat::Rgba8Unorm;
            let mut backend =
                Self::from_device_queue(device, queue, format, width, height, backend_name);
            backend.state.target_context.target_id = "offscreen".to_string();
            backend.state.target_context.target_kind = "offscreenTexture".to_string();
            Ok(backend)
        }

        pub fn register_rgba8_texture(
            &mut self,
            handle: RenderResourceHandle,
            width: u32,
            height: u32,
            rgba8: &[u8],
            sampler: &str,
        ) -> Result<(), String> {
            if handle.kind != RenderResourceKind::Texture {
                return Err("wgpu.texture_handle_kind_must_be_texture".to_string());
            }
            if width == 0 || height == 0 {
                return Err("wgpu.texture_zero_sized".to_string());
            }
            let expected_len = width as usize * height as usize * 4;
            if rgba8.len() != expected_len {
                return Err(format!(
                    "wgpu.texture_byte_len_mismatch: expected {expected_len}, got {}",
                    rgba8.len()
                ));
            }
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("runtime-wgpu-sprite-texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba8,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
            self.textures.insert(
                handle,
                ResidentTexture {
                    _texture: Arc::new(texture),
                    _view: view,
                    _sampler: sampler,
                    bind_group,
                    _width: width,
                    _height: height,
                },
            );
            Ok(())
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
                let texture = Arc::new(self.device.create_texture(&wgpu::TextureDescriptor {
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
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                }));
                for (layer, (global_page_index, page)) in pages.iter().enumerate() {
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
                                z: layer as u32,
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
                        label: Some("runtime-wgpu-font-array-layer-view"),
                        format: Some(format),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: Some(1),
                        base_array_layer: layer as u32,
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
            let payload_vertices = match payload {
                RhiDrawPayload::SpriteTextured { vertices, .. } if !vertices.is_empty() => {
                    Some(vertices)
                }
                RhiDrawPayload::UiComposition { vertices, .. } => Some(vertices),
                _ => None,
            };
            if let Some(vertices) = payload_vertices {
                return vertices
                    .iter()
                    .map(|vertex| RuntimeVertex {
                        position: vertex.position.map(|value| value.to_f32()),
                        color: vertex.color.map(|value| value.to_f32()),
                        uv: vertex.uv.map(|value| value.to_f32()),
                    })
                    .collect();
            }
            match draw_kind {
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

        pub fn execute_plan_to_surface_view(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
        ) -> RhiBackendReport {
            self.execute_plan_to_surface_view_with_rect(plan, view, None)
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
            for command in &plan.commands {
                match command {
                    RhiCommand::BeginFrame { target } => self.begin_frame(EngineRhiFrame {
                        frame_index: plan.frame_index,
                        target_id: target.clone(),
                    }),
                    RhiCommand::Clear { target, color } => self.clear(target, *color),
                    RhiCommand::Draw {
                        target,
                        draw_kind,
                        vertex_count,
                        payload,
                    } => self.draw(EngineRhiDrawCall {
                        target_id: target.clone(),
                        draw_kind: *draw_kind,
                        vertex_count: *vertex_count,
                        payload: payload.clone(),
                    }),
                    RhiCommand::Submit => self.submit(),
                    RhiCommand::Present { target } => self.present(target),
                }
            }
            if let Err(error) = self.render_to_view(plan, view, display_content_rect) {
                self.state.diagnostics.push(RhiBackendDiagnostic::error(
                    "wgpu.render_surface_failed",
                    error,
                ));
            }
            self.finish_report(plan)
        }

        pub fn validate_plan_texture_residency(&self, plan: &RhiCommandPlan) -> Result<(), String> {
            let missing = plan.commands.iter().find_map(|command| {
                let RhiCommand::Draw { payload, .. } = command else {
                    return None;
                };
                let (handle, role) = required_texture_binding(payload)?;
                (!self.textures.contains_key(&handle)).then_some((handle, role))
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

        fn render_plan_to_rgba_bytes_with_rect(
            &mut self,
            plan: &RhiCommandPlan,
            width: u32,
            height: u32,
            display_content_rect: Option<GameViewRect>,
        ) -> Result<Vec<u8>, String> {
            if width == 0 || height == 0 {
                return Err("wgpu.readback_zero_sized_target".to_string());
            }
            let bytes_per_pixel = 4u32;
            let unpadded_bytes_per_row = width * bytes_per_pixel;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
            let output_buffer_size = padded_bytes_per_row as u64 * height as u64;
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("runtime-wgpu-screenshot-texture"),
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
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.render_to_view(plan, &view, display_content_rect)?;
            let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("runtime-wgpu-screenshot-readback"),
                size: output_buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("runtime-wgpu-screenshot-copy-encoder"),
                });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
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
            self.queue.submit(Some(encoder.finish()));
            let buffer_slice = output_buffer.slice(..);
            let (sender, receiver) = std::sync::mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
            self.device
                .poll(wgpu::PollType::wait())
                .map_err(|error| format!("wgpu.readback_poll_failed:{error}"))?;
            receiver
                .recv()
                .map_err(|error| format!("wgpu.readback_channel_failed:{error}"))?
                .map_err(|error| format!("wgpu.readback_map_failed:{error}"))?;
            let mapped = buffer_slice.get_mapped_range();
            let mut rgba = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
            for row in 0..height as usize {
                let start = row * padded_bytes_per_row as usize;
                let end = start + unpadded_bytes_per_row as usize;
                rgba.extend_from_slice(&mapped[start..end]);
            }
            if matches!(
                self.format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ) {
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }
            drop(mapped);
            output_buffer.unmap();
            Ok(rgba)
        }

        fn render_to_view(
            &mut self,
            plan: &RhiCommandPlan,
            view: &wgpu::TextureView,
            display_content_rect: Option<GameViewRect>,
        ) -> Result<(), String> {
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
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("runtime-wgpu-surface-encoder"),
                });
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
                for command in &plan.commands {
                    if let RhiCommand::Draw {
                        draw_kind,
                        vertex_count,
                        payload,
                        ..
                    } = command
                    {
                        let vertices = Self::vertices_for_draw(*draw_kind, *vertex_count, payload);
                        if vertices.is_empty() {
                            continue;
                        }
                        let vertex_buffer =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("runtime-wgpu-draw-vertices"),
                                    contents: bytemuck::cast_slice(&vertices),
                                    usage: wgpu::BufferUsages::VERTEX,
                                });
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
                        pass.draw(0..vertices.len() as u32, 0..1);
                    }
                }
            }
            self.queue.submit(Some(encoder.finish()));
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
    }

    fn required_texture_binding(
        payload: &RhiDrawPayload,
    ) -> Option<(RenderResourceHandle, &'static str)> {
        match payload {
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
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("runtime-wgpu-offscreen-execute-target"),
                size: wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.execute_plan_to_surface_view(plan, &view)
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
        let filter = if label.to_ascii_lowercase().contains("nearest") {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        };
        wgpu::SamplerDescriptor {
            label: Some("runtime-wgpu-sprite-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: filter,
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
            RenderResource,
        };
        use crate::rhi_command_plan::compile_render_graph_to_rhi_plan;

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
                ],
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
                }],
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
                }],
                debug_source: None,
            });
            let plan = compile_render_graph_to_rhi_plan(&graph);

            let rgba = backend.render_plan_to_rgba_bytes(&plan, 8, 8).unwrap();

            assert_eq!(rgba.len(), 8 * 8 * 4);
            assert!(rgba.iter().any(|byte| *byte != 0));
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
                ],
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
                        vertices: quad_vertices_for_test(),
                    },
                ],
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
                        vertices: quad_vertices_for_test(),
                    },
                ],
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
                        vertices: quad_vertices_for_test(),
                    },
                ],
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
                }],
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
                        vertices: full_quad_vertices_for_test(),
                    },
                ],
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
                    bundle_digest: "sha256:test-bundle".to_string(),
                },
                page_payloads: vec![
                    vec![255, 0, 0, 255],
                    vec![
                        255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255,
                    ],
                ],
            }
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
                        vertices: quad_vertices_for_test(),
                    },
                ],
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
            ],
            debug_source: None,
        });
        compile_render_graph_to_rhi_plan(&graph)
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
