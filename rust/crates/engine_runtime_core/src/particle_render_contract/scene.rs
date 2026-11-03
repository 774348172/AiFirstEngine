//! Bind existing particle commands to the same rebased scene view and depth owner.
use super::*;
use crate::{render_contract::RenderDomain, render_graph::*, rhi_command_plan::*};

pub(crate) fn offset(from: [f64; 3], to: [f64; 3]) -> Result<[OrderedF32; 3], String> {
    use crate::spatial_contract::{CoordinateDomain, WorldPosition};
    (|| {
        WorldPosition::new(from, [0.; 3])?.relative_to(
            WorldPosition::new(to, [0.; 3])?,
            CoordinateDomain::new(1e12, 32768., 32768., 0.005)?,
        )
    })()
    .map(|v| v.map(Into::into))
    .map_err(|e: crate::spatial_contract::SpatialContractError| {
        format!("particle_render.coordinate_origin:{e}")
    })
}

fn camera(view: &crate::render_contract::ExecutableRenderView, depth: &str) -> ParticleRenderView {
    let v = view.view_matrix;
    let p = view.view_projection();
    ParticleRenderView {
        fog: view.color_pipeline.as_ref().and_then(|p| p.fog.clone()),
        camera_position: std::array::from_fn(|i| {
            (-(v[i * 4].to_f32() * v[12].to_f32()
                + v[i * 4 + 1].to_f32() * v[13].to_f32()
                + v[i * 4 + 2].to_f32() * v[14].to_f32()))
            .into()
        }),
        scene_view: Some(view.view_id),
        depth_target: Some(depth.into()),
        viewport: Some(view.viewport),
        world_to_clip: std::array::from_fn(|c| std::array::from_fn(|r| p[c * 4 + r].into())),
        right: [v[0], v[4], v[8]],
        up: [v[1], v[5], v[9]],
        forward: [
            (-v[2].to_f32()).into(),
            (-v[6].to_f32()).into(),
            (-v[10].to_f32()).into(),
        ],
        ..Default::default()
    }
}

pub(crate) fn bind_scene(
    graph: &mut RenderGraph,
    geometry: &crate::render_geometry::GeometryFrame,
) {
    if !graph
        .passes
        .iter()
        .flat_map(|p| &p.commands)
        .any(|c| matches!(c, RenderPassCommand::DrawParticles { .. }))
    {
        return;
    }
    let views: Vec<_> = geometry
        .views
        .iter()
        .filter(|v| v.view.domain == RenderDomain::World3d && !v.view.is_shadow_map())
        .collect();
    if views.is_empty() {
        return;
    }
    let result = (|| -> Result<(), String> {
        if views.len() > 16 {
            return Err("particle_render.view_capacity".into());
        }
        crate::color_pipeline::validate_output_regions(views.iter().map(|scene| {
            (
                scene
                    .color_target
                    .as_ref()
                    .map_or(geometry.target.as_str(), |t| t.target_id.as_str()),
                &scene.view,
            )
        }))?;
        let origins: std::collections::BTreeMap<_, _> = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .filter_map(|c| {
                if let RenderPassCommand::ReconcileParticles { sources, .. } = c {
                    Some(sources)
                } else {
                    None
                }
            })
            .flatten()
            .map(|s| {
                (
                    s.instance,
                    (
                        s.simulation_origin_bits.map(f64::from_bits),
                        s.step.emitter_affine,
                    ),
                )
            })
            .collect();
        let mut bound_passes = Vec::with_capacity(graph.passes.len());
        for mut pass in graph.passes.iter().cloned() {
            let draws: Vec<_> = pass
                .commands
                .iter()
                .filter(|c| matches!(c, RenderPassCommand::DrawParticles { .. }))
                .cloned()
                .collect();
            if draws.is_empty() {
                bound_passes.push(pass);
                continue;
            }
            // Reconcile and simulation remain single ordered commands, never per-view copies.
            let original = pass.clone();
            pass.commands
                .retain(|c| !matches!(c, RenderPassCommand::DrawParticles { .. }));
            if !pass.commands.is_empty() {
                pass.pass_id = format!("{}-controls", pass.pass_id);
                pass.depth_target = None;
                bound_passes.push(pass);
            }
            for (index, scene) in views.iter().enumerate() {
                let depth = scene
                    .depth_target
                    .as_deref()
                    .ok_or("particle_render.scene_depth_required")?;
                let target = scene
                    .color_target
                    .as_ref()
                    .map_or(&geometry.target, |t| &t.target_id);
                let mut next_pass = original.clone();
                if index != 0 {
                    next_pass.pass_id = format!("{}-view-{}", original.pass_id, scene.view.view_id);
                }
                next_pass.pass_kind = RenderPassKind::DrawParticles;
                next_pass.view_id = format!("geometry-view-{}", scene.view.view_id);
                next_pass.depth_target = Some(depth.into());
                next_pass.writes = vec![target.clone()];
                next_pass.color_targets = vec![target.clone()];
                next_pass.commands = draws.clone().into();
                for command in &mut next_pass.commands {
                    if let RenderPassCommand::DrawParticles {
                        target: output,
                        instance,
                        view,
                        ..
                    } = command
                    {
                        if *output != geometry.target {
                            return Err("particle_render.target_mismatch".into());
                        }
                        let mut next = camera(&scene.view, depth);
                        next.origin = view.origin;
                        next.emitter_affine = origins
                            .get(instance)
                            .ok_or("particle_render.source_missing")?
                            .1;
                        next.simulation_to_view = offset(
                            origins
                                .get(instance)
                                .copied()
                                .ok_or("particle_render.source_missing")?
                                .0,
                            scene.coordinate_origin,
                        )?;
                        *view = next;
                        *output = target.clone();
                    }
                }
                bound_passes.push(next_pass);
            }
        }
        graph.passes = bound_passes;
        Ok(())
    })();
    if let Err(e) = result {
        graph.diagnostics.push(RenderGraphDiagnostic::error(
            "particle_render.scene_binding",
            e,
        ));
    }
}

/// Validate raw plans too, before any simulation or mutable particle upload.
#[doc(hidden)]
pub fn validate_plan(plan: &RhiCommandPlan) -> Result<(), String> {
    let mut source_affines = std::collections::BTreeMap::new();
    for c in &plan.commands {
        match c {
            RhiCommand::ReconcileParticles { sources } => {
                for s in sources {
                    s.step.emitter_affine.inverse()?;
                    source_affines.insert(s.instance, s.step.emitter_affine);
                }
            }
            RhiCommand::SimulateParticles { step, .. } => {
                step.emitter_affine.inverse()?;
            }
            _ => (),
        }
    }
    let scene_views: Vec<_> = plan
        .commands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            RhiCommand::BeginView {
                target,
                view,
                depth_target,
            } if view.domain == RenderDomain::World3d && !view.is_shadow_map() => {
                Some((i, target, view, depth_target))
            }
            _ => None,
        })
        .collect();
    if plan.commands.iter().any(|c| {
        matches!(
            c,
            RhiCommand::Draw {
                payload: RhiDrawPayload::Particles { .. },
                ..
            }
        )
    }) {
        if scene_views.len() > 16 {
            return Err("particle_render.view_capacity".into());
        }
        crate::color_pipeline::validate_output_regions(scene_views.iter().map(
            |(_, target, scene, _)| {
                let tone = plan.commands.iter().find_map(|c| match c {
                    RhiCommand::ResolveColor3d { resolve } if &resolve.source == *target => {
                        Some(resolve)
                    }
                    _ => None,
                });
                let output = tone
                    .and_then(|t| {
                        plan.commands.iter().find_map(|c| match c {
                            RhiCommand::ResolveColor3d { resolve }
                                if resolve.source == t.target =>
                            {
                                Some(resolve.target.as_str())
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or(target.as_str());
                (output, scene.as_ref())
            },
        ))?;
    }
    for (index, c) in plan.commands.iter().enumerate() {
        let RhiCommand::Draw {
            target,
            payload: RhiDrawPayload::Particles { instance, view, .. },
            ..
        } = c
        else {
            continue;
        };
        view.validate()?;
        if source_affines
            .get(instance)
            .is_some_and(|a| *a != view.emitter_affine)
        {
            return Err("particle_render.source_affine_mismatch".into());
        }
        let Some(id) = view.scene_view else {
            if !scene_views.is_empty() {
                return Err("particle_render.unbound_scene_view".into());
            }
            continue;
        };
        let mut matches = scene_views.iter().filter(|(_, _, v, _)| v.view_id == id);
        let &(begin, scene_target, scene, depth) =
            matches.next().ok_or("particle_render.scene_view_missing")?;
        if matches.next().is_some() {
            return Err("particle_render.scene_view_duplicate".into());
        }
        let expected = camera(
            scene,
            depth
                .as_deref()
                .ok_or("particle_render.scene_depth_required")?,
        );
        if begin >= index
            || scene.view_id != id
            || scene_target != target.as_ref()
            || depth != &view.depth_target
            || expected.world_to_clip != view.world_to_clip
            || expected.right != view.right
            || expected.up != view.up
            || expected.forward != view.forward
            || expected.viewport != view.viewport
            || expected.camera_position != view.camera_position
            || expected.fog != view.fog
        {
            return Err("particle_render.scene_attachment_mismatch".into());
        }
        if plan.commands[..index].iter().any(
            |c| matches!(c,RhiCommand::ResolveColor3d{resolve} if &resolve.source==scene_target),
        ) {
            return Err("particle_render.writer_after_resolve".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fog_camera_uses_inverse_rotated_view_translation() {
        use crate::render_contract::{CameraPose, RenderView};
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.)
            .evaluate(CameraPose {
                position: [17., -3., 5.],
                rotation: [0., (0.4f32).sin(), 0., (0.4f32).cos()],
            })
            .unwrap();
        let bound = camera(&view, "depth");
        for (actual, expected) in bound.camera_position.iter().zip([17., -3., 5.]) {
            assert!((actual.to_f32() - expected).abs() < 0.00001);
        }
    }

    #[test]
    fn overlapping_and_invalid_outputs_reject_without_mutating_particle_passes() {
        use crate::render_contract::{CameraPose, RenderView};
        use crate::render_geometry::{GeometryFrame, GeometryViewInput};
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.);
        let frame = GeometryFrame::build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            }],
            &[],
            &crate::render_resource::RenderResourceManager::new(),
            crate::render_contract::DiagnosticLevel::Off,
        )
        .unwrap();
        for partial in [false, true] {
            let mut geometry = frame.clone();
            if partial {
                geometry.views[0].view.viewport = [40, 0, 32, 64];
            } else {
                let mut second = geometry.views[0].clone();
                second.view.view_id = 2;
                geometry.views.push(second);
            }
            let mut graph = RenderGraph::new("negative", 0);
            graph.passes.push(RenderPass {
                pass_id: "particles".into(),
                pass_name: "particles".into(),
                pass_kind: RenderPassKind::DrawParticles,
                view_id: "old".into(),
                reads: vec![],
                writes: vec!["main".into()],
                color_targets: vec!["main".into()],
                depth_target: None,
                debug_source: None,
                commands: vec![RenderPassCommand::DrawParticles {
                    target: "main".into(),
                    instance: 1,
                    emitter: 0,
                    view: Default::default(),
                    texture: None,
                    mesh: None,
                }]
                .into(),
            });
            let original = graph.passes.clone();
            bind_scene(&mut graph, &geometry);
            assert_eq!(graph.passes, original);
            assert!(graph
                .diagnostics
                .iter()
                .any(|d| d.message.contains(if partial {
                    "invalid_output_region"
                } else {
                    "overlapping_output_unsupported"
                })));
        }
    }

    #[test]
    fn particle_scene_origin_subtracts_before_narrowing_and_rejects_excess_distance() {
        assert_eq!(
            offset([1e10 + 0.125, 0., 0.], [1e10, 0., 0.]).unwrap()[0].to_f32(),
            0.125
        );
        assert!(offset([1e10, 0., 0.], [0.; 3]).is_err());
        assert!(offset([f64::NAN, 0., 0.], [0.; 3]).is_err());
    }
}
