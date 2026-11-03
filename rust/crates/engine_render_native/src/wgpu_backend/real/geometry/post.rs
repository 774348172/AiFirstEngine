//! HDR tone mapping and spatial AA in the existing geometry resource lifetime owner.
use super::super::memory::{GpuMemoryCensus, RetiredGpuResource};
use super::*;
use crate::color_pipeline::{AntiAliasing3D, ColorResolve3D, ColorResolveStage};
use crate::resource_memory::ResourceMemorySample;
pub(super) struct ColorPass {
    resolve: ColorResolve3D,
    source: wgpu::TextureView,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
    pipeline_key: String,
}
impl RetiredGpuResource for ColorPass {
    fn observe<'a>(&'a self, gpu: &mut GpuMemoryCensus<'a>, sample: &mut ResourceMemorySample) {
        self.observe(gpu, sample, "retiring");
    }
}
impl ColorPass {
    fn observe<'a>(
        &'a self,
        gpu: &mut GpuMemoryCensus<'a>,
        sample: &mut ResourceMemorySample,
        role: &str,
    ) {
        gpu.buffer(&self.uniform, "renderer", &[role]);
        gpu.texture(self.source.texture(), &[role]);
        if gpu.is_backing_only() {
            return;
        }
        for s in [
            &self.resolve.source,
            &self.resolve.target,
            &self.pipeline_key,
        ] {
            sample.cpu_string(s, "renderer", &[role]);
        }
        self.resolve
            .pipeline
            .observe_memory(sample, "renderer", role);
    }
}
pub(super) struct PostColorState {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    passes: BTreeMap<String, ColorPass>,
    pipelines: BTreeMap<String, wgpu::RenderPipeline>,
}
impl PostColorState {
    pub(super) fn observe_memory<'a>(
        &'a self,
        gpu: &mut GpuMemoryCensus<'a>,
        sample: &mut ResourceMemorySample,
    ) {
        for (key, pass) in &self.passes {
            if !gpu.is_backing_only() {
                sample.cpu_string(key, "renderer", &["resident"]);
            }
            pass.observe(gpu, sample, "resident");
        }
        if gpu.is_backing_only() {
            return;
        }
        for key in self.pipelines.keys() {
            sample.cpu_string(key, "renderer", &["resident"]);
        }
        sample
            .capacities
            .insert("colorResolvePasses".into(), self.passes.len() as u64);
        sample
            .capacities
            .insert("colorResolvePipelines".into(), self.pipelines.len() as u64);
        sample.partial(
            "cpu.owner.renderer",
            "color3d.pass+pipelineKeys+parameters+assetRefs;stdNodesAndNativePipelineBackingOpaque",
        );
    }
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("color3d-layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(32),
                    },
                    count: None,
                },
            ],
        });
        Self {
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("color3d-clamp-linear"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            passes: BTreeMap::new(),
            pipelines: BTreeMap::new(),
        }
    }
}
impl RealWgpuBackend {
    pub(in crate::wgpu_backend) fn prepare_color_resolves(
        &mut self,
        plan: &RhiCommandPlan,
    ) -> Result<(), String> {
        if let Some(cached) = self.geometry.validated_plan.as_mut() {
            cached.outputs_prepared = false;
        }
        crate::color_pipeline::validate_plan(plan)?;
        let resolves: Vec<_> = plan
            .commands
            .iter()
            .filter_map(|c| match c {
                RhiCommand::ResolveColor3d { resolve } => Some(resolve),
                _ => None,
            })
            .collect();
        if resolves.is_empty() && self.geometry.post.is_none() {
            return Ok(());
        }
        if self.geometry.post.is_none() {
            self.geometry.post = Some(PostColorState::new(&self.device));
        }
        let post = self.geometry.post.as_ref().unwrap();
        let removed: Vec<_> = post
            .passes
            .iter()
            .filter(|(target, p)| {
                !resolves.iter().any(|r| {
                    r.source == **target
                        && **r == p.resolve
                        && self
                            .geometry
                            .colors
                            .get(&r.source)
                            .is_some_and(|(_, _, v)| v == &p.source)
                })
            })
            .map(|(k, _)| k.clone())
            .collect();
        self.admit_gpu_retirement(removed.len() as u64 * 32, removed.len())?;
        for id in removed {
            let old = self
                .geometry
                .post
                .as_mut()
                .unwrap()
                .passes
                .remove(&id)
                .unwrap();
            self.retire_gpu(old, 32);
        }
        let extra = resolves
            .iter()
            .filter(|r| {
                !self
                    .geometry
                    .post
                    .as_ref()
                    .unwrap()
                    .passes
                    .contains_key(&r.source)
            })
            .count() as u64
            * 32;
        self.admit_geometry_backing(extra)?;
        self.geometry
            .admit_visual_upload(plan.frame_index, extra as usize)?;
        let post = self.geometry.post.as_mut().unwrap();
        for resolve in resolves {
            if post.passes.contains_key(&resolve.source) {
                continue;
            }
            let source = self
                .geometry
                .colors
                .get(&resolve.source)
                .ok_or("color.source_not_resident")?
                .2
                .clone();
            let format = self
                .geometry
                .colors
                .get(&resolve.target)
                .map(|(_, _, v)| v.texture().format())
                .unwrap_or(self.format);
            let key = format!("{:?}-{:?}", resolve.stage, format);
            post.pipelines
                .entry(key.clone())
                .or_insert_with(|| pipeline(&self.device, &post.layout, resolve.stage, format));
            let [x, y, w, h] = resolve.region();
            let [width, height] = resolve.extent;
            let data = [
                resolve.pipeline.exposure_ev.to_f32(),
                f32::from(resolve.pipeline.anti_aliasing == AntiAliasing3D::Fxaa),
                0.,
                0.,
                x as f32 / width as f32,
                y as f32 / height as f32,
                w as f32 / width as f32,
                h as f32 / height as f32,
            ];
            let uniform = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("color3d-immutable-parameters"),
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("color3d-bindings"),
                layout: &post.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&post.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            post.passes.insert(
                resolve.source.clone(),
                ColorPass {
                    resolve: resolve.clone(),
                    source,
                    uniform,
                    binding,
                    pipeline_key: key,
                },
            );
        }
        Ok(())
    }
    pub(in crate::wgpu_backend) fn encode_color_resolve(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        primary: &wgpu::TextureView,
        resolve: &ColorResolve3D,
        rect: Option<GameViewRect>,
    ) -> Result<(), String> {
        let post = self.geometry.post.as_ref().ok_or("color.not_prepared")?;
        let prepared = post
            .passes
            .get(&resolve.source)
            .ok_or("color.pass_not_prepared")?;
        let secondary = self.geometry.colors.get(&resolve.target);
        let target = secondary.map(|(_, _, v)| v).unwrap_or(primary);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("color3d-resolve"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let viewport = output_region(
            resolve.region(),
            resolve.extent,
            if secondary.is_none() { rect } else { None },
        );
        set_viewport(&mut pass, viewport);
        pass.set_pipeline(&post.pipelines[&prepared.pipeline_key]);
        pass.set_bind_group(0, &prepared.binding, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    stage: ColorResolveStage,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let mut source = SHADER.to_owned();
    source.push_str(match stage {
        ColorResolveStage::ToneMap => TONE,
        ColorResolveStage::Output => OUTPUT,
    });
    if stage == ColorResolveStage::Output && format.is_srgb() {
        source = source.replace(
            "return result;",
            "return vec4<f32>(decode(result.rgb),result.a);",
        );
    }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("color3d-shader"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("color3d-pipeline-layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("color3d-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
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
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}
const SHADER: &str = r#"
@group(0) @binding(0) var image:texture_2d<f32>;
@group(0) @binding(1) var linear:sampler;
@group(0) @binding(2) var<uniform> parameters:Parameters;
struct Parameters { settings:vec4<f32>, region:vec4<f32> };
struct Vertex { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) i:u32)->Vertex {
    let p=array<vec2<f32>,3>(vec2<f32>(-1.,-1.),vec2<f32>(3.,-1.),vec2<f32>(-1.,3.));
    var o:Vertex;o.position=vec4<f32>(p[i],0.,1.);o.uv=p[i]*vec2<f32>(0.5,-0.5)+vec2<f32>(0.5);return o;
}
fn sample_at(uv:vec2<f32>)->vec4<f32>{
    let r=parameters.region; let half_texel=vec2<f32>(0.5)/vec2<f32>(textureDimensions(image));
    return textureSampleLevel(image,linear,clamp(r.xy+uv*r.zw,r.xy+half_texel,r.xy+r.zw-half_texel),0.);
}
fn encode(x:vec3<f32>)->vec3<f32>{return select(x*12.92,1.055*pow(max(x,vec3<f32>(0.)),vec3<f32>(1./2.4))-vec3<f32>(0.055),x>vec3<f32>(0.0031308));}
fn decode(x:vec3<f32>)->vec3<f32>{return select(x/12.92,pow((x+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),x>vec3<f32>(0.04045));}
"#;
const TONE: &str = r#"
@fragment fn fs_main(v:Vertex)->@location(0) vec4<f32>{
    let scene=sample_at(v.uv);let x=max(scene.rgb,vec3<f32>(0.))*exp2(parameters.settings.x);
    // Beyond this bound the frozen curve already clips to display white.
    let safe=min(x,vec3<f32>(65504.));
    let mapped=clamp((safe*(2.51*safe+vec3<f32>(0.03)))/(safe*(2.43*safe+vec3<f32>(0.59))+vec3<f32>(0.14)),vec3<f32>(0.),vec3<f32>(1.));
    return vec4<f32>(encode(mapped),scene.a);
}
"#;
const OUTPUT: &str = r#"
fn luma(c:vec3<f32>)->f32{return dot(c,vec3<f32>(0.299,0.587,0.114));}
@fragment fn fs_main(v:Vertex)->@location(0) vec4<f32>{
    let center=sample_at(v.uv);var result=center;
    if parameters.settings.y>0.5 {
        let px=1./(vec2<f32>(textureDimensions(image))*parameters.region.zw);
        let nw=sample_at(v.uv+vec2<f32>(-1.,-1.)*px).rgb;let ne=sample_at(v.uv+vec2<f32>(1.,-1.)*px).rgb;
        let sw=sample_at(v.uv+vec2<f32>(-1.,1.)*px).rgb;let se=sample_at(v.uv+vec2<f32>(1.,1.)*px).rgb;
        let a=luma(nw);let b=luma(ne);let c=luma(sw);let d=luma(se);let m=luma(center.rgb);
        let lo=min(m,min(min(a,b),min(c,d)));let hi=max(m,max(max(a,b),max(c,d)));
        if hi-lo>=max(1./32.,hi/8.) {
            var direction=vec2<f32>(-(a+b-c-d),a+c-b-d);
            let reduce=max((a+b+c+d)*(0.25/8.),1./128.);
            direction=clamp(direction/(min(abs(direction.x),abs(direction.y))+reduce),vec2<f32>(-8.),vec2<f32>(8.))*px;
            let candidateA=0.5*(sample_at(v.uv+direction*(1./3.-0.5)).rgb+sample_at(v.uv+direction*(2./3.-0.5)).rgb);
            let candidateB=candidateA*0.5+0.25*(sample_at(v.uv+direction*(-0.5)).rgb+sample_at(v.uv+direction*0.5).rgb);
            let lb=luma(candidateB);result=vec4<f32>(select(candidateB,candidateA,lb<lo || lb>hi),center.a);
        }
    }
    return result;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_color_parameters_survive_three_in_flight_frames_and_shutdown() {
        use crate::{
            render_contract::*,
            render_geometry::{GeometryFrame, GeometryViewInput},
            render_graph::*,
        };
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let mut outputs = Vec::new();
        for ev in [-2., 2., -2.] {
            let mut camera = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
            camera.clear_policy = ClearPolicy::Color {
                color: [4., 0.5, 0.18, 1.],
            };
            camera.color_pipeline = Some(crate::color_pipeline::ColorPipeline3D {
                exposure_ev: ev.into(),
                ..Default::default()
            });
            let frame = GeometryFrame::build(
                "main",
                [64, 64],
                &[GeometryViewInput {
                    view: &camera,
                    pose: CameraPose::default(),
                }],
                &[],
                &crate::render_resource::RenderResourceManager::new(),
                DiagnosticLevel::Off,
            )
            .unwrap();
            let mut graph = RenderGraph::new("in-flight-color", 1);
            graph.output_target = Some("main".into());
            graph
                .resources
                .push(RenderResource::surface_backbuffer("main", 64, 64));
            frame.append_to_graph(&mut graph);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("in-flight-color-capture"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            backend.render_to_view(&plan, &view, None).unwrap();
            outputs.push(texture);
        }
        assert_eq!(backend.gpu_lifecycle_stats().submitted, 3);
        let mut captures = Vec::new();
        for texture in &outputs {
            captures.push(backend.read_texture_rgba_bytes(texture).unwrap());
        }
        assert_eq!(captures[0], captures[2]);
        assert_ne!(captures[0], captures[1]);
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(backend.gpu_lifecycle_stats().retiring_objects, 0);
    }
}
