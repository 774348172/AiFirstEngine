//! Analytic view environment, owned by the existing geometry parameters and clear resources.
use super::*;
pub(super) const CLEAR_BYTES: u64 = 144;
pub(super) fn clear_data(view: &ExecutableRenderView) -> [f32; 36] {
    let mut data = [0.; 36];
    data[..16].copy_from_slice(&IDENTITY_MATRIX);
    if let Some(color) = view.clear_color {
        data[16..20].copy_from_slice(&color.map(OrderedF32::to_f32));
    }
    if let Some(sky) = view.color_pipeline.as_ref().and_then(|p| p.sky.as_ref()) {
        // Inverse rigid view rotation. Translation never enters a direction at infinity.
        let v = view.view_matrix.map(OrderedF32::to_f32);
        data[..16].copy_from_slice(&[
            v[0], v[4], v[8], 0., v[1], v[5], v[9], 0., v[2], v[6], v[10], 0., 0., 0., 0., 1.,
        ]);
        data[20..23].copy_from_slice(&sky.zenith.map(OrderedF32::to_f32));
        data[24..27].copy_from_slice(&sky.horizon.map(OrderedF32::to_f32));
        data[28..31].copy_from_slice(&sky.nadir.map(OrderedF32::to_f32));
        let p = view.projection_matrix.map(OrderedF32::to_f32);
        data[32..36].copy_from_slice(&[p[0], p[5], f32::from(p[15] == 1.), 1.]);
    }
    data
}
pub(super) const CLEAR_SHADER: &str = r#"
struct Uniforms { rotation:mat4x4<f32>, color:vec4<f32>, zenith:vec4<f32>, horizon:vec4<f32>, nadir:vec4<f32>, lens:vec4<f32> };
@group(0) @binding(0) var<uniform> u:Uniforms;
struct Vertex { @builtin(position) position:vec4<f32>, @location(0) clip:vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) i:u32)->Vertex {
    let p=array<vec2<f32>,3>(vec2<f32>(-1.,-1.),vec2<f32>(3.,-1.),vec2<f32>(-1.,3.));
    var o:Vertex;o.position=vec4<f32>(p[i],0.,1.);o.clip=p[i];return o;
}
@fragment fn fs_main(v:Vertex)->@location(0) vec4<f32> {
    if u.lens.w<0.5 { return u.color; }
    let perspective=vec3<f32>(v.clip.x/u.lens.x,v.clip.y/u.lens.y,-1.);
    let local=select(perspective,vec3<f32>(0.,0.,-1.),u.lens.z>0.5);
    let direction=normalize((u.rotation*vec4<f32>(local,0.)).xyz);
    let pole=select(u.nadir.rgb,u.zenith.rgb,direction.y>=0.);
    return vec4<f32>(mix(u.horizon.rgb,pole,abs(direction.y)),u.color.a);
}
"#;
pub(super) const FOG_SHADER: &str = r#"
fn distance_fog(color:vec3<f32>, position:vec3<f32>, camera:vec3<f32>, fog:vec4<f32>, settings:vec4<f32>)->vec3<f32> {
    if fog.w==0. { return color; }
    let distance=max(length(position-camera)-settings.x,0.);
    let transmittance=exp(-fog.w*distance);
    return mix(fog.rgb,color,transmittance);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires real GPU"]
    fn gpu_sky_clear_versions_survive_three_in_flight_frames_and_shutdown() {
        use crate::{
            render_contract::*,
            render_geometry::{GeometryFrame, GeometryViewInput},
            render_graph::*,
        };
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let mut outputs = Vec::new();
        for red in [0.1, 2., 0.1] {
            let mut camera = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
            camera.clear_policy = ClearPolicy::Color {
                color: [4., 0.5, 0.18, 1.],
            };
            camera.color_pipeline = Some(crate::color_pipeline::ColorPipeline3D {
                sky: Some(crate::color_pipeline::GradientSky3D {
                    zenith: [red, 0.2, 0.3].map(Into::into),
                    horizon: [red, 0.2, 0.3].map(Into::into),
                    nadir: [red, 0.2, 0.3].map(Into::into),
                }),
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
