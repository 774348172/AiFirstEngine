//! Real encoder-call regression, independent of the production PBR pixel oracle.
use super::*;

#[test]
#[ignore = "requires a real GPU"]
fn view_bindings_record_only_actual_state_transitions_at_multiple_scales() {
    let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
    let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    let mut backend = RealWgpuBackend::new_offscreen(640, 480).unwrap();
    backend
        .prepare_geometry_plan_for_extent(&plan, [640, 480], None)
        .unwrap();
    let draw = plan.commands.iter().find_map(geometry_draw).unwrap();
    let mut recipe = Recipe::capture(&backend, &[draw], 0);
    let empty = backend
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("view-state-test-empty"),
            entries: &[],
        });
    let material = backend
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("view-state-test-empty"),
            layout: &empty,
            entries: &[],
        });
    let parameter = recipe.batches[0].pipeline.get_bind_group_layout(0);
    let layout = backend
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("view-state-test"),
            bind_group_layouts: &[
                &parameter,
                &empty,
                &backend.geometry.light_layout,
                &backend.geometry.light_layout,
            ],
            push_constant_ranges: &[],
        });
    let shader = backend.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("view-state-test"), source: wgpu::ShaderSource::Wgsl(r#"
@group(2) @binding(0) var<storage,read> lights:array<vec4<f32>>;
@group(3) @binding(0) var<storage,read> shadows:array<vec4<f32>>;
@vertex fn vs_main(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32>{
 let p=array<vec2<f32>,3>(vec2<f32>(-1.,-1.),vec2<f32>(3.,-1.),vec2<f32>(-1.,3.));return vec4<f32>(p[i],0.,1.);
}
@fragment fn fs_main()->@location(0) vec4<f32>{return lights[0]+shadows[0];}
"#.into()),
    });
    let make_pipeline = || {
        backend
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("view-state-test"),
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
                        format: recipe.color_format.unwrap(),
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
    };
    let first_pipeline = make_pipeline();
    let second_pipeline = make_pipeline();
    let make_binding = |color: [f32; 4]| {
        let mut words = vec![0f32; 20];
        words[..4].copy_from_slice(&color);
        let buffer = backend
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("view-state-test"),
                contents: bytemuck::cast_slice(&words),
                usage: wgpu::BufferUsages::STORAGE,
            });
        backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("view-state-test"),
                layout: &backend.geometry.light_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
    };
    let first_binding = make_binding([0.25, 0., 0., 0.5]);
    let second_binding = make_binding([0., 0.25, 0., 0.5]);
    let native = Arc::make_mut(&mut recipe.batches[0].native);
    native.pipeline = first_pipeline.clone();
    native.material = Some(material);
    recipe.batches[0].lights = Some(first_binding.clone());
    recipe.batches[0].shadows = Some(first_binding.clone());
    let batch = recipe.batches[0].clone();
    let mut actual = Vec::new();
    for count in [4, 32, 256] {
        for change in [0, 1, 2] {
            recipe.batches = vec![batch.clone(); count];
            if change == 1 {
                recipe.batches[count / 2].lights = Some(second_binding.clone());
                recipe.batches[count / 2].shadows = Some(second_binding.clone());
            } else if change == 2 {
                Arc::make_mut(&mut recipe.batches[count / 2].native).pipeline =
                    second_pipeline.clone();
            }
            VIEW_BINDING_CALLS.with(|n| n.set([0; 2]));
            let mut encoder =
                backend
                    .device
                    .create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                        label: Some("view-state-test"),
                        color_formats: &[recipe.color_format],
                        depth_stencil: None,
                        sample_count: 1,
                        multiview: None,
                    });
            assert_eq!(recipe.record(&mut encoder), count as u64);
            let _bundle = encoder.finish(&wgpu::RenderBundleDescriptor {
                label: Some("view-state-test"),
            });
            actual.push((count, change, VIEW_BINDING_CALLS.with(|n| n.get())));
        }
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
    eprintln!("actual shadow/light set_bind_group calls {actual:?}");
    assert!(
        actual
            .iter()
            .all(|(_, change, n)| *n == if *change == 0 { [1, 1] } else { [3, 3] }),
        "redundant encoder calls or hidden state transition: {actual:?}"
    );
}
