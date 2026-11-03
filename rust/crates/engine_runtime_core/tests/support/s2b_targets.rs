use super::*;
use engine_runtime::render_graph::*;

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn independent_color_targets_are_not_rejected_or_redirected_to_primary() {
    let graph = two_target_graph();
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let mut backend =
        engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let primary = backend
        .render_plan_to_rgba_bytes(&plan, 64, 64)
        .expect("legal independent color targets must execute");
    let secondary = backend.read_geometry_color_target_rgba("second").unwrap();
    assert!(secondary.chunks_exact(4).all(|p| p == [0, 255, 0, 255]));
    assert_ne!(
        &primary[(32 * 64 + 32) * 4..][..3],
        &[0, 255, 0],
        "secondary clear leaked into primary"
    );
}

#[cfg(feature = "real-wgpu")]
fn two_target_graph() -> RenderGraph {
    let mut manager = RenderResourceManager::new();
    let source = geometry_fixture::source(&mut manager);
    let first = geometry_fixture::frame(&source, &manager);
    let mut view = RenderView::perspective(2, 64, 64, 60., 0.1, 10.);
    view.target = TargetKind::ViewportTexture;
    view.clear_policy = ClearPolicy::Color {
        color: [0., 1., 0., 1.],
    };
    let second = GeometryFrame::build(
        "second",
        [64, 64],
        &[GeometryViewInput {
            view: &view,
            pose: CameraPose::default(),
        }],
        &[],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    let mut graph = geometry_fixture::graph(&first);
    let mut other = geometry_fixture::graph(&second);
    other.resources[0].resource_kind = engine_runtime::render_graph::RenderResourceKind::Texture;
    other.resources[0].usage = vec![RenderResourceUsage::ColorTarget];
    other.resources[0].lifetime = RenderResourceLifetime::Persistent;
    graph.resources.extend(other.resources);
    graph.views.extend(other.views);
    for mut pass in other.passes {
        pass.pass_id = format!("second-{}", pass.pass_id);
        graph.passes.push(pass);
    }
    graph
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn independent_output_invalid_descriptor_is_rejected_before_submission() {
    use engine_runtime::rhi_command_plan::{compile_render_graph_to_rhi_plan, RhiCommand};
    let mut backend =
        engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let good = compile_render_graph_to_rhi_plan(&two_target_graph());
    for (name, mut plan) in [
        ("format", good.clone()),
        ("extent", good.clone()),
        ("usage", good.clone()),
        ("subresource", good.clone()),
        ("missing", good.clone()),
    ] {
        let schedule = std::sync::Arc::make_mut(plan.graph_schedule.as_mut().unwrap());
        if name == "missing" {
            schedule.resources.retain(|r| r.resource_id != "second");
        } else {
            let resource = schedule
                .resources
                .iter_mut()
                .find(|r| r.resource_id == "second")
                .unwrap();
            match name {
                "format" => resource.format = RenderResourceFormat::Bgra8Unorm,
                "extent" => resource.size.width = 63,
                "usage" => resource.usage.push(RenderResourceUsage::Sampled),
                "subresource" => {
                    schedule
                        .resource_shapes
                        .entry(GraphResourceId("second".into()))
                        .or_default()
                        .samples = 4;
                }
                _ => unreachable!(),
            }
        }
        let before = backend.last_gpu_submission();
        assert!(
            backend.render_plan_to_rgba_bytes(&plan, 64, 64).is_err(),
            "accepted {name}"
        );
        assert_eq!(
            before,
            backend.last_gpu_submission(),
            "submitted invalid {name}"
        );
        assert!(backend.read_geometry_color_target_rgba("second").is_err());
    }
    // No graph may present an internal target as a surface.
    let mut bad = good.clone();
    bad.commands.push(RhiCommand::Present {
        target: "second".into(),
    });
    assert!(backend
        .render_plan_to_rgba_bytes(&bad, 64, 64)
        .unwrap_err()
        .contains("secondary_present"));
    backend.render_plan_to_rgba_bytes(&good, 64, 64).unwrap();
}

#[cfg(feature = "real-wgpu")]
fn shared_atlas_graph() -> RenderGraph {
    use std::sync::Arc;
    let mut graph = super::s2b_shadows::sampled_graph(1., false);
    let original = graph
        .passes
        .iter()
        .flat_map(|p| &p.commands)
        .find_map(|c| match c {
            RenderPassCommand::DrawGeometry { draw } if !draw.view.is_shadow_map() => {
                Some(draw.clone())
            }
            _ => None,
        })
        .unwrap();
    let mut view = original.view.as_ref().clone();
    view.view_id = 2;
    view.target_kind = TargetKind::ViewportTexture;
    let lighting = ClusteredLights::build(&view,&[engine_runtime::render_projection_adapter::LightProjection {
        component: serde_json::from_value(serde_json::json!({"schemaVersion":"light3d.v1","enabled":true,"color":[1,1,1],"kind":{"type":"directional","illuminanceLux":1},"layers":[]})).unwrap(),
        position:[0.;3],direction:[0.,0.,-1.],origin:[0.;3],
    }]).unwrap();
    let mut draw = original.as_ref().clone();
    draw.shadows = Some(
        ShadowMaps::build(
            &view,
            &lighting,
            draw.shadows.as_ref().unwrap().projections().to_vec(),
        )
        .unwrap(),
    );
    draw.lighting = Some(lighting);
    draw.view = Arc::new(view.clone());
    draw.target = "second".into();
    draw.depth_target = Some("second-depth".into());
    GeometryFrame {
        target: "main".into(),
        extent: [64, 64],
        views: vec![GeometryViewFrame {
            coordinate_origin: [0.; 3],
            color_target: Some(
                engine_runtime::runtime_renderer::RenderTarget::viewport_texture("second", 64, 64),
            ),
            view,
            depth_target: draw.depth_target.clone(),
            draws: vec![Arc::new(draw)],
            batches: None,
            summary: None,
            trace: vec![],
        }],
    }
    .append_to_graph(&mut graph);
    graph
}

#[cfg(feature = "real-wgpu")]
#[test]
#[ignore = "requires real GPU"]
fn shared_shadow_atlas_survives_the_last_receiver_on_another_color_target() {
    let graph = shared_atlas_graph();
    let plan = engine_runtime::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let schedule = plan.graph_schedule.as_ref().unwrap();
    let receiver = graph
        .passes
        .iter()
        .position(|p| p.pass_id == "geometry-view-2-draws")
        .unwrap();
    let position = schedule.order.iter().position(|p| *p == receiver).unwrap();
    let lifetime = schedule
        .lifetimes
        .iter()
        .find(|l| l.resource.0 == "shadow-atlas")
        .unwrap();
    assert_eq!(
        lifetime.last, position,
        "shared shadow atlas ended before last target receiver"
    );
    let mut backend =
        engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let a = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    let b = backend.read_geometry_color_target_rgba("second").unwrap();
    assert_eq!(
        a, b,
        "same receiver on two independent targets must sample the shared atlas identically"
    );
    let center = (32 * 64 + 32) * 4;
    assert!(
        a[center + 1] < 10,
        "shared atlas did not shadow the last receiver: {:?}",
        &a[center..center + 4]
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
