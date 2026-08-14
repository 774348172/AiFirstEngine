use std::sync::Arc;

use editor_ui_renderer::{UiColor, UiDrawList, UiRect};
use wgpu::util::DeviceExt;

use crate::diagnostics::RealUiPresentReport;
use crate::draw_plan::{
    UiGpuDrawPlan, UiGpuImageTextureQuad, UiGpuTextGlyph, UiGpuViewportTextureQuad, UiUvRect,
};
use crate::image_texture::EditorImageTextureRegistry;
use crate::render_graph::{UiRenderGraph, UiRhiCommandKind, UiRhiCommandPlan};
use crate::shared_gpu::{EditorSharedGpuContext, EditorSharedGpuContextSummary};
use crate::surface::{backend_error_label, backend_present_label};
use crate::viewport_texture::EditorViewportTextureRegistry;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UiVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl UiVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

impl TextVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TexturedVertex {
    position: [f32; 2],
    uv: [f32; 2],
    tint: [f32; 4],
}

impl TexturedVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub struct RealWgpuUiRenderer {
    surface: wgpu::Surface<'static>,
    context: Arc<EditorSharedGpuContext>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    viewport_texture_pipeline: wgpu::RenderPipeline,
    viewport_texture_bind_group_layout: wgpu::BindGroupLayout,
    viewport_textures: EditorViewportTextureRegistry,
    image_textures: EditorImageTextureRegistry,
    theme_texture_diagnostics: Vec<String>,
    size: (u32, u32),
    backend_name: String,
    capture_surface_copy_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiRgbaCapture {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
    pub source_format: String,
    pub backend: String,
}

impl RealWgpuUiRenderer {
    pub fn new(window: Arc<winit::window::Window>) -> Result<Self, String> {
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Err("ui_present.zero_sized_window".to_string());
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
        });
        let surface = instance
            .create_surface(window)
            .map_err(|error| format!("ui_present.create_surface_failed:{error}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|error| format!("ui_present.request_adapter_failed:{error}"))?;
        let backend_name = format!("{:?}", adapter.get_info().backend);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("editor-ui-device"),
            required_features: wgpu::Features::empty(),
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| format!("ui_present.request_device_failed:{error}"))?;
        let capabilities = surface.get_capabilities(&adapter);
        let capture_surface_copy_supported =
            capabilities.usages.contains(wgpu::TextureUsages::COPY_DST);
        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or_else(|| "ui_present.default_surface_config_unavailable".to_string())?;
        if capture_surface_copy_supported {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        let context = Arc::new(EditorSharedGpuContext::from_device_queue(
            device,
            queue,
            backend_name.clone(),
            config.format,
        ));
        surface.configure(context.device(), &config);
        let pipeline = create_pipeline(context.device(), config.format);
        let text_bind_group_layout = create_text_bind_group_layout(context.device());
        let text_pipeline =
            create_text_pipeline(context.device(), config.format, &text_bind_group_layout);
        let viewport_texture_bind_group_layout =
            create_viewport_texture_bind_group_layout(context.device());
        let viewport_texture_pipeline = create_viewport_texture_pipeline(
            context.device(),
            config.format,
            &viewport_texture_bind_group_layout,
        );

        let mut image_textures = EditorImageTextureRegistry::new();
        let theme_texture_diagnostics =
            image_textures.upload_builtin_control_textures_gpu(context.device(), context.queue());

        Ok(Self {
            surface,
            context,
            config,
            pipeline,
            text_pipeline,
            text_bind_group_layout,
            viewport_texture_pipeline,
            viewport_texture_bind_group_layout,
            viewport_textures: EditorViewportTextureRegistry::new(),
            image_textures,
            theme_texture_diagnostics,
            size: (size.width, size.height),
            backend_name,
            capture_surface_copy_supported,
        })
    }

    pub fn shared_context(&self) -> Arc<EditorSharedGpuContext> {
        Arc::clone(&self.context)
    }

    pub fn shared_context_summary(&self) -> EditorSharedGpuContextSummary {
        self.context.summary().clone()
    }

    pub fn viewport_textures(&self) -> &EditorViewportTextureRegistry {
        &self.viewport_textures
    }

    pub fn viewport_textures_mut(&mut self) -> &mut EditorViewportTextureRegistry {
        &mut self.viewport_textures
    }

    pub fn viewport_texture_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn image_textures(&self) -> &EditorImageTextureRegistry {
        &self.image_textures
    }

    pub fn image_textures_mut(&mut self) -> &mut EditorImageTextureRegistry {
        &mut self.image_textures
    }

    pub fn theme_texture_diagnostics(&self) -> &[String] {
        &self.theme_texture_diagnostics
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(self.context.device(), &self.config);
    }

    pub fn present(&mut self, draw_list: &UiDrawList) -> RealUiPresentReport {
        self.present_internal(draw_list, false).0
    }

    pub fn present_with_rgba_capture(
        &mut self,
        draw_list: &UiDrawList,
    ) -> (RealUiPresentReport, Result<UiRgbaCapture, String>) {
        let (report, capture) = self.present_internal(draw_list, true);
        (
            report,
            capture.unwrap_or_else(|| Err("ui_capture.not_requested".to_string())),
        )
    }

    fn present_internal(
        &mut self,
        draw_list: &UiDrawList,
        capture_requested: bool,
    ) -> (RealUiPresentReport, Option<Result<UiRgbaCapture, String>>) {
        if capture_requested && !self.capture_surface_copy_supported {
            return (
                RealUiPresentReport::failed(
                    backend_error_label(&self.backend_name),
                    "ui_capture.surface_copy_unsupported",
                    "The active WGPU surface does not support COPY_DST presentation.",
                    "editor_wgpu_renderer.real_wgpu.capture_preflight",
                ),
                Some(Err("ui_capture.surface_copy_unsupported".to_string())),
            );
        }
        let plan = match UiGpuDrawPlan::from_draw_list(draw_list) {
            Ok(plan) => plan,
            Err(error) => {
                return (
                    RealUiPresentReport::failed(
                        backend_error_label(&self.backend_name),
                        error.clone(),
                        error.clone(),
                        "editor_wgpu_renderer.real_wgpu.plan",
                    ),
                    capture_requested.then_some(Err(error)),
                );
            }
        };
        let graph = UiRenderGraph::from_draw_plan(&plan);
        let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);

        let surface_texture = match self.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(self.context.device(), &self.config);
                return (
                    RealUiPresentReport::failed(
                        backend_error_label(&self.backend_name),
                        "ui_present.surface_reconfigured",
                        "Surface was lost or outdated and has been reconfigured.",
                        "editor_wgpu_renderer.real_wgpu.acquire",
                    ),
                    capture_requested.then(|| Err("ui_present.surface_reconfigured".to_string())),
                );
            }
            Err(error) => {
                return (
                    RealUiPresentReport::failed(
                        backend_error_label(&self.backend_name),
                        "ui_present.acquire_surface_failed",
                        error.to_string(),
                        "editor_wgpu_renderer.real_wgpu.acquire",
                    ),
                    capture_requested.then(|| Err("ui_present.acquire_surface_failed".to_string())),
                );
            }
        };

        let capture_texture = capture_requested.then(|| {
            self.context
                .device()
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("editor-ui-capture-texture"),
                    size: wgpu::Extent3d {
                        width: self.size.0,
                        height: self.size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.config.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
        });
        let view = capture_texture.as_ref().map_or_else(
            || {
                surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default())
            },
            |texture| texture.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        for quad in &plan.image_texture_quads {
            self.image_textures.touch(&quad.texture_id);
        }
        let vertices = rect_vertices_for_wgpu_backend(&plan, &rhi_plan);
        let text_vertices = text_vertices_for_wgpu_backend(&plan, &rhi_plan);
        let viewport_texture_batches = viewport_texture_batches_for_wgpu_backend(
            self.context.device(),
            &self.viewport_texture_bind_group_layout,
            &plan,
            &self.viewport_textures,
        );
        let image_texture_batches = image_texture_batches_for_wgpu_backend(
            self.context.device(),
            &self.viewport_texture_bind_group_layout,
            &plan,
            &self.image_textures,
        );
        let vertex_buffer =
            self.context
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("editor-ui-vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        let text_vertex_buffer =
            self.context
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("editor-ui-text-vertices"),
                    contents: bytemuck::cast_slice(&text_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        let text_bind_group = create_text_atlas_bind_group(
            self.context.device(),
            self.context.queue(),
            &self.text_bind_group_layout,
            &plan,
        );
        let mut encoder =
            self.context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("editor-ui-present-encoder"),
                });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("editor-ui-present-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.035,
                            g: 0.038,
                            b: 0.043,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            for command in &rhi_plan.commands {
                let first_vertex = (command.first_item * 6) as u32;
                let vertex_count = (command.item_count * 6) as u32;
                match command.kind {
                    UiRhiCommandKind::DrawRectBatch if vertex_count > 0 => {
                        pass.set_pipeline(&self.pipeline);
                        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                        pass.draw(first_vertex..first_vertex + vertex_count, 0..1);
                    }
                    UiRhiCommandKind::DrawTextBatch if vertex_count > 0 => {
                        pass.set_pipeline(&self.text_pipeline);
                        pass.set_bind_group(0, &text_bind_group, &[]);
                        pass.set_vertex_buffer(0, text_vertex_buffer.slice(..));
                        pass.draw(first_vertex..first_vertex + vertex_count, 0..1);
                    }
                    UiRhiCommandKind::DrawViewportTextureBatch => {
                        draw_prepared_texture_range(
                            &mut pass,
                            &self.pipeline,
                            &self.viewport_texture_pipeline,
                            &viewport_texture_batches,
                            command.first_item,
                            command.item_count,
                        );
                    }
                    UiRhiCommandKind::DrawImageTextureBatch => {
                        draw_prepared_texture_range(
                            &mut pass,
                            &self.pipeline,
                            &self.viewport_texture_pipeline,
                            &image_texture_batches,
                            command.first_item,
                            command.item_count,
                        );
                    }
                    _ => {}
                }
            }
        }
        let readback = capture_texture.as_ref().map(|texture| {
            let unpadded_bytes_per_row = self.size.0 * 4;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
            let buffer = self
                .context
                .device()
                .create_buffer(&wgpu::BufferDescriptor {
                    label: Some("editor-ui-capture-readback"),
                    size: u64::from(padded_bytes_per_row) * u64::from(self.size.1),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &surface_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: self.size.0,
                    height: self.size.1,
                    depth_or_array_layers: 1,
                },
            );
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
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(self.size.1),
                    },
                },
                wgpu::Extent3d {
                    width: self.size.0,
                    height: self.size.1,
                    depth_or_array_layers: 1,
                },
            );
            (buffer, padded_bytes_per_row)
        });
        self.context.queue().submit(Some(encoder.finish()));
        surface_texture.present();

        let capture = readback.map(|(buffer, padded_bytes_per_row)| {
            read_rgba_capture(
                self.context.device(),
                buffer,
                self.size,
                padded_bytes_per_row,
                self.config.format,
                &self.backend_name,
            )
        });
        (
            RealUiPresentReport::from_compiled_plan(
                backend_present_label(&self.backend_name),
                &plan,
                &rhi_plan,
                true,
            ),
            capture,
        )
    }
}

fn read_rgba_capture(
    device: &wgpu::Device,
    buffer: wgpu::Buffer,
    size: (u32, u32),
    padded_bytes_per_row: u32,
    format: wgpu::TextureFormat,
    backend: &str,
) -> Result<UiRgbaCapture, String> {
    let buffer_slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait())
        .map_err(|error| format!("ui_capture.poll_failed:{error}"))?;
    receiver
        .recv()
        .map_err(|error| format!("ui_capture.channel_failed:{error}"))?
        .map_err(|error| format!("ui_capture.map_failed:{error}"))?;
    let mapped = buffer_slice.get_mapped_range();
    let unpadded_bytes_per_row = size.0 as usize * 4;
    let mut rgba8 = Vec::with_capacity(unpadded_bytes_per_row * size.1 as usize);
    for row in 0..size.1 as usize {
        let start = row * padded_bytes_per_row as usize;
        let end = start + unpadded_bytes_per_row;
        rgba8.extend_from_slice(&mapped[start..end]);
    }
    drop(mapped);
    buffer.unmap();
    normalize_capture_to_rgba(&mut rgba8, format)?;
    Ok(UiRgbaCapture {
        width: size.0,
        height: size.1,
        rgba8,
        source_format: format!("{format:?}"),
        backend: backend.to_string(),
    })
}

fn normalize_capture_to_rgba(bytes: &mut [u8], format: wgpu::TextureFormat) -> Result<(), String> {
    match format {
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            for pixel in bytes.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            Ok(())
        }
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => Ok(()),
        _ => Err(format!("ui_capture.unsupported_surface_format:{format:?}")),
    }
}

fn create_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("editor-ui-shader"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> VertexOut {
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
        label: Some("editor-ui-pipeline-layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("editor-ui-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[UiVertex::layout()],
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

fn create_text_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("editor-ui-text-bind-group-layout"),
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

fn create_text_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("editor-ui-text-shader"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
@group(0) @binding(0) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(1) var glyph_sampler: sampler;

struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
  @location(0) position: vec2<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) color: vec4<f32>
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4<f32>(position, 0.0, 1.0);
  out.uv = uv;
  out.color = color;
  return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
  let alpha = textureSample(glyph_atlas, glyph_sampler, in.uv).r;
  return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#
            .into(),
        ),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("editor-ui-text-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("editor-ui-text-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[TextVertex::layout()],
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

fn create_viewport_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("editor-ui-viewport-texture-bind-group-layout"),
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

fn create_viewport_texture_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("editor-ui-viewport-texture-shader"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
@group(0) @binding(0) var viewport_texture: texture_2d<f32>;
@group(0) @binding(1) var viewport_sampler: sampler;

struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) tint: vec4<f32>,
};

@vertex
fn vs_main(
  @location(0) position: vec2<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) tint: vec4<f32>
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4<f32>(position, 0.0, 1.0);
  out.uv = uv;
  out.tint = tint;
  return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
  return textureSample(viewport_texture, viewport_sampler, in.uv) * in.tint;
}
"#
            .into(),
        ),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("editor-ui-viewport-texture-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("editor-ui-viewport-texture-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[TexturedVertex::layout()],
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

fn create_text_atlas_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    plan: &UiGpuDrawPlan,
) -> wgpu::BindGroup {
    let width = plan.glyph_atlas_width.max(1);
    let height = plan.glyph_atlas_height.max(1);
    let expected_len = (width * height) as usize;
    let atlas_data = if plan.glyph_atlas_alpha.len() == expected_len {
        plan.glyph_atlas_alpha.as_slice()
    } else {
        &[255u8]
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("editor-ui-glyph-atlas"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        atlas_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("editor-ui-glyph-atlas-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("editor-ui-text-bind-group"),
        layout,
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
    })
}

struct TextureDrawBatch {
    vertex_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: u32,
}

struct FallbackDrawBatch {
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

enum PreparedTextureDraw {
    Resolved(TextureDrawBatch),
    Fallback(FallbackDrawBatch),
}

fn draw_prepared_texture_range<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    rect_pipeline: &'pass wgpu::RenderPipeline,
    texture_pipeline: &'pass wgpu::RenderPipeline,
    batches: &'pass [Option<PreparedTextureDraw>],
    first_item: usize,
    item_count: usize,
) {
    for prepared in batches
        .get(first_item..first_item.saturating_add(item_count))
        .into_iter()
        .flatten()
        .flatten()
    {
        match prepared {
            PreparedTextureDraw::Resolved(batch) => {
                pass.set_pipeline(texture_pipeline);
                pass.set_bind_group(0, &batch.bind_group, &[]);
                pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                pass.draw(0..batch.vertex_count, 0..1);
            }
            PreparedTextureDraw::Fallback(batch) => {
                pass.set_pipeline(rect_pipeline);
                pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                pass.draw(0..batch.vertex_count, 0..1);
            }
        }
    }
}

fn rect_vertices_for_wgpu_backend(
    plan: &UiGpuDrawPlan,
    rhi_plan: &UiRhiCommandPlan,
) -> Vec<UiVertex> {
    let mut vertices = Vec::new();
    if !rhi_plan
        .commands
        .iter()
        .any(|command| command.kind == UiRhiCommandKind::DrawRectBatch)
    {
        return vertices;
    }
    for drawable in &plan.drawable_rects {
        push_rect_vertices(
            &mut vertices,
            drawable.rect,
            drawable.color,
            plan.surface_width as f32,
            plan.surface_height as f32,
        );
    }
    vertices
}

fn viewport_texture_batches_for_wgpu_backend(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    plan: &UiGpuDrawPlan,
    registry: &EditorViewportTextureRegistry,
) -> Vec<Option<PreparedTextureDraw>> {
    let mut batches = Vec::new();
    for quad in &plan.viewport_texture_quads {
        let Some(resolved) = registry.resolve_gpu(&quad.texture_id) else {
            batches.push(quad.fallback_if_missing.then(|| {
                PreparedTextureDraw::Fallback(fallback_draw_batch(
                    device,
                    "editor-ui-missing-viewport-texture-vertices",
                    quad.rect,
                    UiColor::rgba(23, 40, 52, 255),
                    plan.surface_width as f32,
                    plan.surface_height as f32,
                ))
            }));
            continue;
        };
        let mut vertices = Vec::new();
        push_viewport_texture_vertices(
            &mut vertices,
            quad,
            plan.surface_width as f32,
            plan.surface_height as f32,
        );
        if vertices.is_empty() {
            batches.push(None);
            continue;
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("editor-ui-viewport-texture-vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("editor-ui-viewport-texture-bind-group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(resolved.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(resolved.sampler),
                },
            ],
        });
        batches.push(Some(PreparedTextureDraw::Resolved(TextureDrawBatch {
            vertex_buffer,
            bind_group,
            vertex_count: vertices.len() as u32,
        })));
    }
    batches
}

fn image_texture_batches_for_wgpu_backend(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    plan: &UiGpuDrawPlan,
    registry: &EditorImageTextureRegistry,
) -> Vec<Option<PreparedTextureDraw>> {
    let mut batches = Vec::new();
    for quad in &plan.image_texture_quads {
        let Some(resolved) = registry.resolve_gpu(&quad.texture_id) else {
            batches.push(Some(PreparedTextureDraw::Fallback(fallback_draw_batch(
                device,
                "editor-ui-missing-image-texture-vertices",
                quad.rect,
                quad.fallback_color,
                plan.surface_width as f32,
                plan.surface_height as f32,
            ))));
            continue;
        };
        let mut vertices = Vec::new();
        push_image_texture_vertices(
            &mut vertices,
            quad,
            plan.surface_width as f32,
            plan.surface_height as f32,
        );
        if vertices.is_empty() {
            batches.push(None);
            continue;
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("editor-ui-image-texture-vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("editor-ui-image-texture-bind-group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(resolved.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(resolved.sampler),
                },
            ],
        });
        batches.push(Some(PreparedTextureDraw::Resolved(TextureDrawBatch {
            vertex_buffer,
            bind_group,
            vertex_count: vertices.len() as u32,
        })));
    }
    batches
}

fn fallback_draw_batch(
    device: &wgpu::Device,
    label: &str,
    rect: UiRect,
    color: UiColor,
    width: f32,
    height: f32,
) -> FallbackDrawBatch {
    let mut vertices = Vec::new();
    push_rect_vertices(&mut vertices, rect, color, width, height);
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    FallbackDrawBatch {
        vertex_buffer,
        vertex_count: vertices.len() as u32,
    }
}

fn text_vertices_for_wgpu_backend(
    plan: &UiGpuDrawPlan,
    rhi_plan: &UiRhiCommandPlan,
) -> Vec<TextVertex> {
    let mut vertices = Vec::new();
    if !rhi_plan
        .commands
        .iter()
        .any(|command| command.kind == UiRhiCommandKind::DrawTextBatch)
    {
        return vertices;
    }
    for glyph in &plan.text_glyphs {
        push_text_vertices(
            &mut vertices,
            glyph,
            plan.surface_width as f32,
            plan.surface_height as f32,
        );
    }
    vertices
}

fn push_viewport_texture_vertices(
    vertices: &mut Vec<TexturedVertex>,
    quad: &UiGpuViewportTextureQuad,
    width: f32,
    height: f32,
) {
    push_textured_quad_vertices(
        vertices,
        quad.rect,
        quad.uv,
        UiColor::rgba(255, 255, 255, 255),
        width,
        height,
    );
}

fn push_image_texture_vertices(
    vertices: &mut Vec<TexturedVertex>,
    quad: &UiGpuImageTextureQuad,
    width: f32,
    height: f32,
) {
    push_textured_quad_vertices(vertices, quad.rect, quad.uv, quad.tint, width, height);
}

fn push_textured_quad_vertices(
    vertices: &mut Vec<TexturedVertex>,
    rect: UiRect,
    uv: UiUvRect,
    tint: UiColor,
    width: f32,
    height: f32,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let x0 = (rect.x / width) * 2.0 - 1.0;
    let y0 = 1.0 - (rect.y / height) * 2.0;
    let x1 = ((rect.x + rect.width) / width) * 2.0 - 1.0;
    let y1 = 1.0 - ((rect.y + rect.height) / height) * 2.0;
    let tint = [
        tint.r as f32 / 255.0,
        tint.g as f32 / 255.0,
        tint.b as f32 / 255.0,
        tint.a as f32 / 255.0,
    ];
    vertices.extend_from_slice(&[
        TexturedVertex {
            position: [x0, y0],
            uv: [uv.u0, uv.v0],
            tint,
        },
        TexturedVertex {
            position: [x1, y0],
            uv: [uv.u1, uv.v0],
            tint,
        },
        TexturedVertex {
            position: [x1, y1],
            uv: [uv.u1, uv.v1],
            tint,
        },
        TexturedVertex {
            position: [x0, y0],
            uv: [uv.u0, uv.v0],
            tint,
        },
        TexturedVertex {
            position: [x1, y1],
            uv: [uv.u1, uv.v1],
            tint,
        },
        TexturedVertex {
            position: [x0, y1],
            uv: [uv.u0, uv.v1],
            tint,
        },
    ]);
}

fn push_rect_vertices(
    vertices: &mut Vec<UiVertex>,
    rect: UiRect,
    color: UiColor,
    width: f32,
    height: f32,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let x0 = (rect.x / width) * 2.0 - 1.0;
    let y0 = 1.0 - (rect.y / height) * 2.0;
    let x1 = ((rect.x + rect.width) / width) * 2.0 - 1.0;
    let y1 = 1.0 - ((rect.y + rect.height) / height) * 2.0;
    let color = [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ];
    vertices.extend_from_slice(&[
        UiVertex {
            position: [x0, y0],
            color,
        },
        UiVertex {
            position: [x1, y0],
            color,
        },
        UiVertex {
            position: [x1, y1],
            color,
        },
        UiVertex {
            position: [x0, y0],
            color,
        },
        UiVertex {
            position: [x1, y1],
            color,
        },
        UiVertex {
            position: [x0, y1],
            color,
        },
    ]);
}

fn push_text_vertices(
    vertices: &mut Vec<TextVertex>,
    glyph: &UiGpuTextGlyph,
    width: f32,
    height: f32,
) {
    let rect = glyph.rect;
    if rect.width <= 0.0 || rect.height <= 0.0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let x0 = (rect.x / width) * 2.0 - 1.0;
    let y0 = 1.0 - (rect.y / height) * 2.0;
    let x1 = ((rect.x + rect.width) / width) * 2.0 - 1.0;
    let y1 = 1.0 - ((rect.y + rect.height) / height) * 2.0;
    let uv = glyph.uv;
    let color = [
        glyph.color.r as f32 / 255.0,
        glyph.color.g as f32 / 255.0,
        glyph.color.b as f32 / 255.0,
        glyph.color.a as f32 / 255.0,
    ];
    vertices.extend_from_slice(&[
        TextVertex {
            position: [x0, y0],
            uv: [uv.u0, uv.v0],
            color,
        },
        TextVertex {
            position: [x1, y0],
            uv: [uv.u1, uv.v0],
            color,
        },
        TextVertex {
            position: [x1, y1],
            uv: [uv.u1, uv.v1],
            color,
        },
        TextVertex {
            position: [x0, y0],
            uv: [uv.u0, uv.v0],
            color,
        },
        TextVertex {
            position: [x1, y1],
            uv: [uv.u1, uv.v1],
            color,
        },
        TextVertex {
            position: [x0, y1],
            uv: [uv.u0, uv.v1],
            color,
        },
    ]);
}

#[cfg(test)]
mod real_wgpu_tests {
    use super::*;
    use editor_ui_renderer::DrawCommand;

    #[test]
    fn wgpu_backend_rect_vertices_require_rhi_draw_rect_command() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 1,
            surface_width: 100.0,
            surface_height: 100.0,
            commands: vec![DrawCommand::Rect {
                rect: UiRect {
                    x: 0.0,
                    y: 0.0,
                    width: 50.0,
                    height: 50.0,
                },
                color: UiColor::PANEL,
                corner_radius: 0.0,
            }],
            hit_regions: Vec::new(),
        };
        let plan = UiGpuDrawPlan::from_draw_list(&draw_list).unwrap();
        let graph = UiRenderGraph::from_draw_plan(&plan);
        let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);

        assert_eq!(rect_vertices_for_wgpu_backend(&plan, &rhi_plan).len(), 6);
    }

    #[test]
    fn wgpu_backend_text_vertices_require_rhi_draw_text_command() {
        let draw_list = UiDrawList {
            revision: 1,
            frame: 1,
            surface_width: 200.0,
            surface_height: 100.0,
            commands: vec![DrawCommand::Text {
                rect: UiRect {
                    x: 0.0,
                    y: 0.0,
                    width: 120.0,
                    height: 20.0,
                },
                text: "Text".to_string(),
                color: UiColor::TEXT,
                size: 12.0,
            }],
            hit_regions: Vec::new(),
        };
        let plan = UiGpuDrawPlan::from_draw_list(&draw_list).unwrap();
        let graph = UiRenderGraph::from_draw_plan(&plan);
        let rhi_plan = UiRhiCommandPlan::from_render_graph(&graph);

        assert!(plan.rendered_glyph_count > 0);
        if plan.font_loaded {
            assert!(text_vertices_for_wgpu_backend(&plan, &rhi_plan).len() >= 6);
            assert!(rect_vertices_for_wgpu_backend(&plan, &rhi_plan).is_empty());
        } else {
            assert!(rect_vertices_for_wgpu_backend(&plan, &rhi_plan).len() >= 6);
        }
    }

    #[test]
    fn capture_normalizes_bgra_and_preserves_rgba() {
        let mut bgra = vec![1, 2, 3, 255, 10, 20, 30, 128];
        normalize_capture_to_rgba(&mut bgra, wgpu::TextureFormat::Bgra8UnormSrgb).unwrap();
        assert_eq!(bgra, vec![3, 2, 1, 255, 30, 20, 10, 128]);

        let mut rgba = vec![3, 2, 1, 255];
        normalize_capture_to_rgba(&mut rgba, wgpu::TextureFormat::Rgba8Unorm).unwrap();
        assert_eq!(rgba, vec![3, 2, 1, 255]);
    }

    #[test]
    fn capture_rejects_non_rgba_surface_format() {
        let error = normalize_capture_to_rgba(&mut [0; 4], wgpu::TextureFormat::Rgba16Float)
            .expect_err("unsupported capture format");
        assert!(error.contains("unsupported_surface_format"));
    }

    #[test]
    fn image_texture_vertices_forward_tint_to_every_vertex() {
        let quad = UiGpuImageTextureQuad {
            rect: UiRect {
                x: 2.0,
                y: 3.0,
                width: 10.0,
                height: 8.0,
            },
            uv: UiUvRect {
                u0: 0.25,
                v0: 0.1,
                u1: 0.75,
                v1: 0.9,
            },
            texture_id: "test".to_string(),
            fallback_color: UiColor::PANEL,
            tint: UiColor::rgba(128, 64, 255, 204),
        };
        let mut vertices = Vec::new();
        push_image_texture_vertices(&mut vertices, &quad, 100.0, 100.0);
        assert_eq!(vertices.len(), 6);
        for vertex in vertices {
            assert_eq!(vertex.tint, [128.0 / 255.0, 64.0 / 255.0, 1.0, 0.8]);
        }
    }
}
