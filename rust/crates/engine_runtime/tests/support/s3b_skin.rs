use super::*;
use engine_runtime::character_animation::*;
use std::sync::Arc;

fn skin(translation: f32, reset: bool) -> Arc<SkinDeformation3D> {
    let mut matrix = IDENTITY_MATRIX;
    matrix[12] = translation;
    let source = CharacterSkin {
        mesh_joint: 0,
        mesh_asset_id: "mesh".into(),
        joints: vec![0, 1],
        inverse_bind: vec![IDENTITY_MATRIX; 2],
        influences: vec![
            VertexInfluence {
                joints: [0, 1, 0, 0],
                weights: [0.5, 0.5, 0., 0.]
            };
            3
        ],
        morph_targets: vec![MorphTarget3D {
            position_deltas: vec![[0., 0.25, 0.]; 3],
            normal_deltas: vec![[0.; 3]; 3],
        }],
        morph_weights: vec![0.5],
    };
    Arc::new(
        SkinDeformation3D::new(
            Arc::new(SkinVertexData3D::from_skin(&source)),
            &[IDENTITY_MATRIX, matrix],
            &[IDENTITY_MATRIX; 2],
            &[0.5],
            &[0.],
            reset,
        )
        .unwrap(),
    )
}

fn compact(frame: &mut GeometryFrame) {
    let view = &mut frame.views[0];
    let draw = view.draws[0].clone();
    view.batches = Some(vec![Arc::new(GeometryBatch::new(
        draw.clone(),
        Arc::new(vec![draw.native_instance().clone()]),
        vec![GeometryVisibleInstance {
            instance_index: 0,
            camera_depth: draw.camera_depth,
        }],
    ))]);
}
#[test]
#[ignore = "requires a real GPU"]
fn gpu_s3b_skin_morph_current_previous_match_cpu_and_old_packet_remains_immutable() {
    let mut manager = RenderResourceManager::new();
    let source = geometry_fixture::source(&mut manager);
    let mut frame = geometry_fixture::frame(&source, &manager);
    frame.views[0].batches = None;
    Arc::make_mut(&mut frame.views[0].draws[0]).deformation = Some(skin(0.5, false));
    compact(&mut frame);
    let draw = frame.views[0].draws[0].clone();
    let graph = geometry_fixture::graph(&frame);
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    let mut backend =
        engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    let (current, previous) = backend.read_character_skin_vertices(&draw).unwrap();
    let base = geometry_fixture::triangle();
    for (index, p) in base.vertex_bytes.chunks_exact(12).enumerate() {
        let point =
            std::array::from_fn(|i| f32::from_le_bytes(p[i * 4..i * 4 + 4].try_into().unwrap()));
        for (old, bytes) in [(false, &current), (true, &previous)] {
            let expected = draw
                .deformation
                .as_ref()
                .unwrap()
                .reference_vertex(index as u32, point, [0., 0., 1.], old)
                .unwrap()
                .0;
            for i in 0..3 {
                let offset = index * 12 + i * 4;
                let actual = f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                assert!(
                    (actual - expected[i]).abs() < 1e-5,
                    "vertex {index} axis {i}, previous {old}: {actual} != {}",
                    expected[i]
                );
            }
        }
    }
    let mut changed = frame.clone();
    Arc::make_mut(&mut changed.views[0].draws[0]).deformation = Some(skin(-0.5, false));
    compact(&mut changed);
    let changed_plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &geometry_fixture::graph(&changed),
    );
    let changed_pixels = backend
        .render_plan_to_rgba_bytes(&changed_plan, 64, 64)
        .unwrap();
    assert_ne!(pixels, changed_pixels);
    let restored = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert_eq!(restored, pixels);
    assert_eq!(
        backend.read_character_skin_vertices(&draw).unwrap(),
        (current, previous)
    );
    let mut reset = frame.clone();
    Arc::make_mut(&mut reset.views[0].draws[0]).deformation = Some(skin(0.5, true));
    compact(&mut reset);
    let reset_plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &geometry_fixture::graph(&reset),
    );
    backend
        .render_plan_to_rgba_bytes(&reset_plan, 64, 64)
        .unwrap();
    let (a, b) = backend
        .read_character_skin_vertices(&reset.views[0].draws[0])
        .unwrap();
    assert_eq!(
        a, b,
        "teleport/history reset must not retain old skin positions"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_s3b_standard_lit_skin_morph_normals_match_inverse_transpose_reference() {
    let mut manager = RenderResourceManager::new();
    let geometry = standard_lit_fixture::lit_triangle();
    let mesh = geometry_fixture::prepare(
        "skin-lit-mesh",
        RenderAssetData::Mesh(geometry.clone()),
        &mut manager,
    );
    let material = geometry_fixture::prepare(
        "skin-lit-material",
        RenderAssetData::Material(standard_lit_fixture::standard_lit([1.; 4])),
        &mut manager,
    );
    let source = PreparedGeometry::from_assets(&mesh, &material, 0, &manager).unwrap();
    let mut frame = geometry_fixture::frame(&source, &manager);
    let binding = CharacterSkin {
        mesh_joint: 0,
        mesh_asset_id: "skin-lit-mesh".into(),
        joints: vec![0],
        inverse_bind: vec![IDENTITY_MATRIX],
        influences: vec![
            VertexInfluence {
                joints: [0; 4],
                weights: [1., 0., 0., 0.]
            };
            3
        ],
        morph_targets: vec![MorphTarget3D {
            position_deltas: vec![[0., 0.1, 0.]; 3],
            normal_deltas: vec![[0.2, 0.1, 0.]; 3],
        }],
        morph_weights: vec![0.6],
    };
    let matrix = glam::Mat4::from_scale_rotation_translation(
        glam::Vec3::new(1.2, 0.8, 1.),
        glam::Quat::from_rotation_y(0.7),
        glam::Vec3::ZERO,
    )
    .to_cols_array();
    let deformation = Arc::new(
        SkinDeformation3D::new(
            Arc::new(SkinVertexData3D::from_skin(&binding)),
            &[matrix],
            &[IDENTITY_MATRIX],
            &[0.6],
            &[0.],
            false,
        )
        .unwrap(),
    );
    frame.views[0].batches = None;
    Arc::make_mut(&mut frame.views[0].draws[0]).deformation = Some(deformation.clone());
    compact(&mut frame);
    let draw = &frame.views[0].draws[0];
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(
        &geometry_fixture::graph(&frame),
    );
    let mut backend =
        engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert!(
        pixels.chunks_exact(4).filter(|p| p[0] > 128).count() > 50,
        "The actual StandardLit skin draw must be visible"
    );
    let (current, previous) = backend.read_character_skin_vertices(draw).unwrap();
    for (index, vertex) in geometry.vertex_bytes.chunks_exact(24).enumerate() {
        let position = std::array::from_fn(|i| {
            f32::from_le_bytes(vertex[i * 4..i * 4 + 4].try_into().unwrap())
        });
        for (old, bytes) in [(false, &current), (true, &previous)] {
            let (p, n) = deformation
                .reference_vertex(index as u32, position, [0., 0., 1.], old)
                .unwrap();
            for (offset, expected) in [(0, p), (12, n)] {
                for (axis, expected) in expected.into_iter().enumerate() {
                    let start = index * 24 + offset + axis * 4;
                    let actual = f32::from_le_bytes(bytes[start..start + 4].try_into().unwrap());
                    assert!((actual-expected).abs()<1e-5, "vertex {index}, previous {old}, field {offset}, axis {axis}: {actual} != {expected}");
                }
            }
        }
    }
    assert_ne!(current, previous);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
