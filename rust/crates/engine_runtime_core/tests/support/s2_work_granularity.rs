//! Independent alpha/depth oracle for PBR mask phase ordering.
use super::{geometry_fixture as fixture, standard_lit_fixture as lit};
use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_resource::*;
use engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan;
use engine_runtime::wgpu_backend::real::RealWgpuBackend;

fn material(rgb: [f32; 3], alpha: f32, blend: MaterialBlend) -> CookedMaterial {
    let mut m = lit::standard_lit([0., 0., 0., alpha]);
    m.parameters.insert(
        "emissive".into(),
        MaterialParameter::Vector4([rgb[0], rgb[1], rgb[2], 0.]),
    );
    m.blend = blend;
    if blend == MaterialBlend::AlphaMask {
        m.parameters
            .insert("alphaCutoff".into(), MaterialParameter::Float(0.5));
    }
    if blend == MaterialBlend::AlphaBlend {
        m.depth = MaterialDepth::TestOnly;
    }
    m
}

#[test]
#[ignore = "requires real GPU"]
fn pbr_mask_phase_matches_independent_alpha_depth_and_coplanar_reference() {
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let mut manager = RenderResourceManager::new();
    let mesh = fixture::prepare(
        "mask-phase-mesh",
        RenderAssetData::Mesh(lit::lit_triangle()),
        &mut manager,
    );
    // Emissive primary colors isolate phase/depth/alpha from illumination and tone mapping.
    // RGB reference: discard -> red; passing nearer/equal mask -> green; closer opaque -> red.
    for (case, opaque_z, mask_z, alpha, rgb) in [
        (0, -3., -2., 0.49, [255, 0, 0]),
        (1, -3., -2., 0.5, [0, 255, 0]),
        (2, -2., -3., 0.5, [255, 0, 0]),
        (3, -2., -2., 0.5, [0, 255, 0]),
    ] {
        let opaque = fixture::prepare(
            &format!("opaque-{case}"),
            RenderAssetData::Material(material([1., 0., 0.], 1., MaterialBlend::Opaque)),
            &mut manager,
        );
        let mask = fixture::prepare(
            &format!("mask-{case}"),
            RenderAssetData::Material(material([0., 1., 0.], alpha, MaterialBlend::AlphaMask)),
            &mut manager,
        );
        let a = PreparedGeometry::from_assets(&mesh, &opaque, 0, &manager).unwrap();
        let b = PreparedGeometry::from_assets(&mesh, &mask, 0, &manager).unwrap();
        for reverse in [false, true] {
            for mirror in [-1., 1.] {
                let mut instances = vec![
                    fixture::instance(&a, 1, [0., 0., opaque_z]),
                    fixture::instance(&b, 2, [0., 0., mask_z]),
                ];
                instances[1].world_transform[0] = mirror;
                if reverse {
                    instances.reverse();
                }
                let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
                let frame = GeometryFrame::build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    &instances,
                    &manager,
                    DiagnosticLevel::Off,
                )
                .unwrap();
                let plan = compile_render_graph_to_rhi_plan(&fixture::graph(&frame));
                assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
                let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
                let center = &pixels[(32 * 64 + 32) * 4..(32 * 64 + 33) * 4];
                for c in 0..3 {
                    assert!(
                        (i32::from(center[c]) - rgb[c]).abs() <= 1,
                        "case {case}, reverse {reverse}, mirror {mirror}: {center:?}"
                    );
                }
                let expected_alpha = if rgb[1] == 255 { 128 } else { 255 };
                assert!(
                    (i32::from(center[3]) - expected_alpha).abs() <= 1,
                    "mask blended alpha: {center:?}"
                );
            }
        }
    }
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
