//! GPU counterfactual: exact zero contribution must not sample a shadow map.
use super::*;

#[test]
#[ignore = "requires a real GPU"]
fn gpu_zero_light_contribution_preserves_values_without_shadow_work() {
    let backend = RealWgpuBackend::new_offscreen(4, 4).unwrap();
    // A conservative cluster containing all four lights: directional, two
    // point ranges, and a spot whose cone misses the samples. The payload uses
    // the production light-word layout; the oracle does not use CPU binning.
    let mut words = vec![0u32; 20 + 4 * 16 + 3456 * 8];
    for i in [0, 5, 10, 15] {
        words[i] = 1f32.to_bits();
    }
    words[16] = 4;
    words[17] = 84;
    for (i, data) in [
        [
            0., 0., 0., 0., 0., 0., -1., 0., 1., 1., 1., 0., 0., 0., 0., 0.,
        ],
        [
            0., 0., 2., 1., 0., 0., -1., 1., 1., 1., 1., 0., 0., 0., 0., 0.,
        ],
        [
            0., 0., 2., 1., 0., 0., -1., 4., 1., 1., 1., 0., 0., 0., 0., 0.,
        ],
        [
            0., 0., 2., 2., 1., 0., 0., 4., 1., 1., 1., 0.96, 0.86, 0., 0., 0.,
        ],
    ]
    .into_iter()
    .enumerate()
    {
        for (dst, value) in words[20 + i * 16..][..16].iter_mut().zip(data) {
            *dst = f32::to_bits(value);
        }
    }
    for cell in 0..3456 {
        words[84 + cell * 8] = 15;
    }
    let input = backend
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("zero-light-counterfactual-input"),
            contents: bytemuck::cast_slice(&words),
            usage: wgpu::BufferUsages::STORAGE,
        });
    let output = backend.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("zero-light-counterfactual-result"),
        size: 144,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = backend.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("zero-light-counterfactual-readback"),
        size: 144,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let guard = "if(attenuation<=0. || dot(n,l)<=0.){continue;}";
    let actual = super::lights::SHADER
        .replace("@group(2)", "@group(0)")
        .replace("scene_direct", "actual_direct")
        .replace("*attenuation;", "*attenuation*test_shadow(0u);");
    // Retain the pre-optimization equation, including all shadow calls. This
    // tests the actual production WGSL against its unshortened counterpart.
    let reference = actual
        .replace(guard, "")
        .replace("actual_direct", "reference_direct")
        .replace("test_shadow(0u)", "test_shadow(1u)");
    let reference = reference[reference.find("fn reference_direct").unwrap()..].to_string();
    let source = format!(
        "{}\n{actual}\n{reference}\n{}",
        super::lit::DIRECT_BRDF,
        r#"
struct Results { values:array<vec4<f32>,8>, calls:array<atomic<u32>,2> };
@group(0) @binding(1) var<storage,read_write> result:Results;
fn test_shadow(which:u32)->f32 { atomicAdd(&result.calls[which],1u);return 1.; }
@compute @workgroup_size(1) fn main() {
    let positions=array<vec3<f32>,4>(vec3<f32>(0.),vec3<f32>(0.),vec3<f32>(0.,0.,6.),vec3<f32>(0.,0.,-2.));
    let normals=array<vec3<f32>,4>(vec3<f32>(0.,0.,1.),vec3<f32>(0.,0.,-1.),vec3<f32>(0.,0.,1.),vec3<f32>(1.,0.,0.));
    for(var i=0u;i<4u;i++) {
        result.values[i*2u]=vec4<f32>(actual_direct(positions[i],normals[i],vec3<f32>(0.,0.,1.),vec3<f32>(0.5),0.3,0.4,normals[i]),1.);
        result.values[i*2u+1u]=vec4<f32>(reference_direct(positions[i],normals[i],vec3<f32>(0.,0.,1.),vec3<f32>(0.5),0.3,0.4,normals[i]),1.);
    }
}"#
    );
    let shader = backend
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("production-light-zero-counterfactual"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let pipeline = backend
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("production-light-zero-counterfactual"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
    let bindings = backend
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
            ],
        });
    let mut encoder = backend.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, 144);
    backend.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |value| {
            tx.send(value).unwrap();
        });
    backend.device.poll(wgpu::PollType::wait()).unwrap();
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let bytes = readback.slice(..).get_mapped_range();
    let values: &[f32] = bytemuck::cast_slice(&bytes[..128]);
    for case in 0..4 {
        for channel in 0..4 {
            let a = values[case * 8 + channel];
            let b = values[case * 8 + 4 + channel];
            assert!(
                a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-6,
                "case {case}: {a} != {b}"
            );
        }
    }
    let counts: &[u32] = bytemuck::cast_slice(&bytes[128..136]);
    eprintln!(
        "zero-light shadow operations actual={} reference={}",
        counts[0], counts[1]
    );
    assert_eq!(counts[1], 16);
    assert_eq!(counts[0], 3, "zero radiance still performed shadow work");
}
