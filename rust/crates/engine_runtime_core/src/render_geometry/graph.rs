use super::*;
use crate::render_graph::*;

impl GeometryFrame {
    /// Add phases to the same graph used by Sprite/Particle/UiProjection.
    pub fn append_to_graph(&self, graph: &mut RenderGraph) {
        let only_shadow =
            !self.views.is_empty() && self.views.iter().all(|v| v.view.is_shadow_map());
        if !only_shadow {
            let Some(target) = graph
                .resources
                .iter()
                .find(|r| r.resource_id == self.target)
            else {
                graph.diagnostics.push(RenderGraphDiagnostic::error(
                    "geometry.target_missing",
                    &self.target,
                ));
                return;
            };
            if graph.output_target.as_ref() != Some(&self.target)
                || [target.size.width, target.size.height] != self.extent
            {
                graph.diagnostics.push(RenderGraphDiagnostic::error(
                    "geometry.target_mismatch",
                    &self.target,
                ));
                return;
            }
        }
        let mut begins = Vec::new();
        let mut draws = Vec::new();
        for frame in &self.views {
            if let Some(batches) = &frame.batches {
                if batches.len() != frame.draws.len()
                    || !batches.iter().zip(&frame.draws).all(|(b, d)| b.draw == *d)
                {
                    graph.diagnostics.push(RenderGraphDiagnostic::error(
                        "geometry.batch.head_desynchronized",
                        "compact heads must match their immutable batch",
                    ));
                    return;
                }
            }
            let shadow = frame.view.is_shadow_map();
            let extent = frame.view.target_extent;
            if let Some(output) = &frame.color_target {
                let resource = RenderResource {
                    resource_id: output.target_id.clone(),
                    resource_name: output.target_id.clone(),
                    resource_kind: RenderResourceKind::Texture,
                    format: match output.format.as_str() {
                        "Rgba8Unorm" => RenderResourceFormat::Rgba8Unorm,
                        "Bgra8Unorm" => RenderResourceFormat::Bgra8Unorm,
                        "Rgba8UnormSrgb" => RenderResourceFormat::Rgba8UnormSrgb,
                        "Bgra8UnormSrgb" => RenderResourceFormat::Bgra8UnormSrgb,
                        _ => {
                            graph.diagnostics.push(RenderGraphDiagnostic::error(
                                "geometry.output_format",
                                &output.format,
                            ));
                            return;
                        }
                    },
                    size: RenderResourceSize {
                        width: output.width,
                        height: output.height,
                    },
                    usage: vec![RenderResourceUsage::ColorTarget],
                    lifetime: RenderResourceLifetime::Persistent,
                };
                if let Some(old) = graph
                    .resources
                    .iter()
                    .find(|r| r.resource_id == output.target_id)
                {
                    if old != &resource {
                        graph.diagnostics.push(RenderGraphDiagnostic::error(
                            "geometry.output_conflict",
                            &output.target_id,
                        ));
                        return;
                    }
                } else {
                    graph.resources.push(resource);
                }
            }
            let target = if shadow {
                let Some(depth) = frame.depth_target.as_ref() else {
                    graph.diagnostics.push(RenderGraphDiagnostic::error(
                        "shadow.depth_missing",
                        "ShadowMap requires a depth attachment",
                    ));
                    return;
                };
                depth
            } else {
                frame
                    .color_target
                    .as_ref()
                    .map_or(&self.target, |target| &target.target_id)
            };
            let view_id = format!("geometry-view-{}", frame.view.view_id);
            graph.views.push(RenderGraphView {
                view_id: view_id.clone(),
                view_kind: "geometry".into(),
                width: extent[0],
                height: extent[1],
                clear_color: frame.view.clear_color.unwrap_or(color([0.; 4])),
            });
            if let Some(depth) = &frame.depth_target {
                if !graph.resources.iter().any(|r| &r.resource_id == depth) {
                    graph.resources.push(RenderResource {
                        resource_id: depth.clone(),
                        resource_name: depth.clone(),
                        resource_kind: RenderResourceKind::Texture,
                        format: RenderResourceFormat::Depth32Float,
                        size: RenderResourceSize {
                            width: extent[0],
                            height: extent[1],
                        },
                        usage: if shadow {
                            vec![
                                RenderResourceUsage::DepthTarget,
                                RenderResourceUsage::Sampled,
                            ]
                        } else {
                            vec![RenderResourceUsage::DepthTarget]
                        },
                        lifetime: RenderResourceLifetime::FrameLocal,
                    });
                }
            }
            let make_pass = |suffix: String, kind, command| RenderPass {
                pass_id: format!("{view_id}-{suffix}"),
                pass_name: format!("{:?} {suffix}", frame.view.domain),
                pass_kind: kind,
                view_id: view_id.clone(),
                reads: vec![],
                writes: vec![target.clone()],
                color_targets: if shadow { vec![] } else { vec![target.clone()] },
                depth_target: frame.depth_target.clone(),
                commands: vec![command].into(),
                debug_source: Some("RenderProjection.DrawGeometry".into()),
            };
            begins.push(make_pass(
                "begin".into(),
                RenderPassKind::BeginView,
                RenderPassCommand::BeginView {
                    target: target.clone(),
                    view: Arc::new(frame.view.clone()),
                    depth_target: frame.depth_target.clone(),
                },
            ));
            if !frame.draws.is_empty() {
                // Attachment hazards belong to this ordered view phase, not to
                // each instance. Commands retain the already sorted draw order.
                let mut pass = make_pass(
                    "draws".into(),
                    RenderPassKind::DrawGeometry,
                    RenderPassCommand::DrawGeometry {
                        draw: frame.draws[0].clone(),
                    },
                );
                // Borrow the current maps for this immutable phase. Repeated
                // heads share their map; only final unique resource names own strings.
                // Detached maps still contribute every distinct atlas by value.
                let mut sampled = std::collections::BTreeSet::new();
                let mut previous_maps = None;
                for draw in &frame.draws {
                    if let Some(shadows) = &draw.shadows {
                        let identity = Arc::as_ptr(shadows);
                        if previous_maps == Some(identity) {
                            continue;
                        }
                        previous_maps = Some(identity);
                        sampled.extend(shadows.atlases());
                    }
                }
                pass.reads = sampled
                    .into_iter()
                    .map(|atlas| {
                        #[cfg(test)]
                        SAMPLED_ATLAS_COPIES.with(|n| n.set(n.get() + 1));
                        atlas.to_owned()
                    })
                    .collect();
                pass.commands = if let Some(batches) = &frame.batches {
                    batches
                        .iter()
                        .map(|batch| RenderPassCommand::DrawGeometryBatch {
                            batch: batch.clone(),
                        })
                        .collect()
                } else {
                    frame
                        .draws
                        .iter()
                        .map(|draw| RenderPassCommand::DrawGeometry { draw: draw.clone() })
                        .collect()
                };
                draws.push(pass);
            }
        }
        // Attachment initialization precedes UI BeforeWorld; world draws preserve UI stages.
        let begin_at = graph
            .passes
            .iter()
            .position(|p| p.pass_kind != RenderPassKind::Clear)
            .unwrap_or(graph.passes.len());
        graph.passes.splice(begin_at..begin_at, begins);
        let draw_at = graph.passes.iter().position(|p| matches!(p.pass_kind,RenderPassKind::DrawParticles|RenderPassKind::ParticleSimulation|RenderPassKind::Present)
            || p.commands.iter().any(|c| matches!(c,RenderPassCommand::DrawUiComposition { stage,.. } if stage != "BeforeWorld"))).unwrap_or(graph.passes.len());
        graph.passes.splice(draw_at..draw_at, draws);
        crate::particle_render_contract::bind_scene(graph, self);
        crate::color_pipeline::append_to_graph(graph);
    }
}

pub(crate) fn validate_geometry_graph(graph: &RenderGraph) -> Vec<RenderGraphDiagnostic> {
    validate_geometry_graph_inner(graph, None).0
}

#[cfg(test)]
thread_local! {
    static VIEW_VALUE_COMPARISONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn matching_view(a: &ExecutableRenderView, b: &ExecutableRenderView) -> bool {
    if std::ptr::eq(a, b) {
        return true;
    }
    #[cfg(test)]
    VIEW_VALUE_COMPARISONS.with(|count| count.set(count.get() + 1));
    a == b
}

fn matching_attachments(
    a: (&str, &ExecutableRenderView, Option<&str>),
    b: (&str, &ExecutableRenderView, Option<&str>),
) -> bool {
    a.0 == b.0 && matching_view(a.1, b.1) && a.2 == b.2
}

pub(crate) fn validate_geometry_graph_with_cache(
    graph: &RenderGraph,
    proofs: &mut Vec<std::sync::Weak<DrawGeometry>>,
) -> (Vec<RenderGraphDiagnostic>, u64) {
    // A proof budget, never a draw admission limit. Oversized graphs still get
    // full validation and cannot leave an unbounded retained allocation behind.
    const MAX_DRAW_PROOFS: usize = 65_536;
    let count = graph
        .passes
        .iter()
        .flat_map(|p| &p.commands)
        .filter(|c| c.geometry_draw().is_some())
        .take(MAX_DRAW_PROOFS + 1)
        .count();
    if count > MAX_DRAW_PROOFS {
        *proofs = Vec::new();
        return validate_geometry_graph_inner(graph, None);
    }
    validate_geometry_graph_inner(graph, Some(proofs))
}

fn validate_geometry_graph_inner(
    graph: &RenderGraph,
    mut proofs: Option<&mut Vec<std::sync::Weak<DrawGeometry>>>,
) -> (Vec<RenderGraphDiagnostic>, u64) {
    let mut validations = 0;
    let mut draw_index = 0;
    let mut errors = Vec::new();
    let mut views = std::collections::BTreeMap::new();
    let mut depths = DepthAttachmentValidation::default();
    for (pass_index, pass) in graph.passes.iter().enumerate() {
        let mut checked_shadows = None;
        let mut checked_attachments = None;
        let mut checked_begun = None;
        let mut view_validation = DrawViewValidation::default();
        for command in &pass.commands {
            if let RenderPassCommand::DrawGeometryBatch { batch } = command {
                if let Err(e) = proof::selection_with_view(batch, &mut view_validation) {
                    errors.push(RenderGraphDiagnostic::error(
                        "geometry.invalid_batch",
                        e.to_string(),
                    ));
                }
            }
            let (target, view, depth) = match command {
                RenderPassCommand::BeginView {
                    target,
                    view,
                    depth_target,
                } => {
                    // BeginView may replace a declaration within this same pass.
                    checked_begun = None;
                    if let Some(id) = depth_target {
                        if let Err(e) = depths.insert(id, view) {
                            errors.push(RenderGraphDiagnostic::error(
                                "geometry.shared_view_depth",
                                e.to_string(),
                            ));
                        }
                    }
                    if views
                        .insert(
                            view.view_id,
                            (target.as_str(), view.as_ref(), depth_target.as_deref()),
                        )
                        .is_some()
                    {
                        errors.push(RenderGraphDiagnostic::error(
                            "geometry.duplicate_view",
                            "view id was already begun",
                        ));
                    }
                    (target.as_str(), view.as_ref(), depth_target.as_deref())
                }
                command if command.geometry_draw().is_some() => {
                    let draw = command.geometry_draw().unwrap();
                    if draw.standard_lit.is_some()
                        && draw
                            .view
                            .color_pipeline
                            .as_ref()
                            .is_some_and(|p| p.ibl.is_some())
                        && draw.environment.is_none()
                    {
                        errors.push(RenderGraphDiagnostic::error(
                            "environment.binding_missing",
                            "IBL camera requires a prepared environment for StandardLit",
                        ));
                    }
                    if let Some(shadows) = &draw.shadows {
                        if checked_shadows != Some(Arc::as_ptr(shadows)) {
                            if let Err(e) = shadows.validate_graph(graph, pass, pass_index) {
                                errors.push(RenderGraphDiagnostic::error(
                                    "shadow.invalid_sampling",
                                    e.to_string(),
                                ));
                            }
                            checked_shadows = Some(Arc::as_ptr(shadows));
                        }
                    }
                    // The incoming Arc keeps this value alive; its retained Weak
                    // prevents address recycling and cannot retain a native lease.
                    // Safe mutation through Arc::make_mut dissociates this proof.
                    let reused = proofs
                        .as_ref()
                        .and_then(|p| p.get(draw_index))
                        .is_some_and(|old| old.as_ptr() == Arc::as_ptr(draw));
                    let validation = if reused {
                        Ok(())
                    } else {
                        validations += 1;
                        draw.validate_shared_with_view(&mut view_validation)
                    };
                    if let Some(proofs) = &mut proofs {
                        if !reused {
                            let proof = if validation.is_ok() {
                                Arc::downgrade(draw)
                            } else {
                                std::sync::Weak::new()
                            };
                            if draw_index < proofs.len() {
                                proofs[draw_index] = proof;
                            } else {
                                proofs.push(proof);
                            }
                        }
                    }
                    draw_index += 1;
                    if let Err(e) = validation {
                        errors.push(RenderGraphDiagnostic::error(
                            "geometry.invalid_draw",
                            e.to_string(),
                        ));
                    }
                    let attachments = (
                        draw.target.as_ref(),
                        draw.view.as_ref(),
                        draw.depth_target.as_deref(),
                    );
                    // Reuse only a successful comparison during this immutable pass.
                    // The borrowed view identity is shared by all draws of a view;
                    // changed values/targets/depths and later BeginView still recheck.
                    let begun = checked_begun
                        .is_some_and(|checked| matching_attachments(checked, attachments))
                        || views
                            .get(&draw.view.view_id)
                            .is_some_and(|begun| matching_attachments(*begun, attachments));
                    if begun {
                        checked_begun = Some(attachments);
                    } else {
                        errors.push(RenderGraphDiagnostic::error(
                            "geometry.view_not_begun",
                            "draw must use its declared view and attachments",
                        ));
                    }
                    (
                        draw.target.as_ref(),
                        draw.view.as_ref(),
                        draw.depth_target.as_deref(),
                    )
                }
                _ => continue,
            };
            let attachments = (target, view, depth);
            if checked_attachments.is_some_and(|checked| matching_attachments(checked, attachments))
            {
                continue;
            }
            checked_attachments = Some(attachments);
            let targets: Vec<_> = graph
                .resources
                .iter()
                .filter(|r| r.resource_id == target)
                .collect();
            let shadow = view.is_shadow_map();
            if targets.len() != 1
                || (shadow && depth != Some(target))
                || if shadow {
                    !pass.color_targets.is_empty()
                } else {
                    pass.color_targets.len() != 1 || pass.color_targets[0] != target
                }
                || pass.depth_target.as_deref() != depth
                || pass.view_id != format!("geometry-view-{}", view.view_id)
            {
                errors.push(RenderGraphDiagnostic::error(
                    "geometry.attachment_mismatch",
                    "view, target or depth attachment differs from pass",
                ));
                continue;
            }
            let color = targets[0];
            let declared: Vec<_> = graph
                .views
                .iter()
                .filter(|v| v.view_id == pass.view_id)
                .collect();
            if declared.len() != 1
                || declared[0].width != color.size.width
                || declared[0].height != color.size.height
            {
                errors.push(RenderGraphDiagnostic::error(
                    "geometry.view_missing",
                    "view must be declared once with the target extent",
                ));
            }
            if view.target_extent != [color.size.width, color.size.height]
                || if shadow {
                    color.format != RenderResourceFormat::Depth32Float
                        || !color.usage.contains(&RenderResourceUsage::DepthTarget)
                        || !color.usage.contains(&RenderResourceUsage::Sampled)
                } else {
                    !color.usage.contains(&RenderResourceUsage::ColorTarget)
                        || !matches!(
                            color.format,
                            RenderResourceFormat::Rgba16Float
                                | RenderResourceFormat::Rgba8Unorm
                                | RenderResourceFormat::Bgra8Unorm
                                | RenderResourceFormat::Rgba8UnormSrgb
                                | RenderResourceFormat::Bgra8UnormSrgb
                        )
                }
                || view
                    .validate_extent(color.size.width, color.size.height)
                    .is_err()
            {
                errors.push(RenderGraphDiagnostic::error(
                    "geometry.invalid_target",
                    "invalid color format/usage/viewport",
                ));
            }
            if depth.is_some() != (view.depth_policy != DepthPolicy::Disabled) {
                errors.push(RenderGraphDiagnostic::error(
                    "geometry.depth_missing",
                    "view depth policy needs its attachment",
                ));
            }
            if let Some(id) = depth {
                let resources: Vec<_> = graph
                    .resources
                    .iter()
                    .filter(|r| r.resource_id == id)
                    .collect();
                if resources.len() != 1
                    || resources[0].format != RenderResourceFormat::Depth32Float
                    || resources[0].size != color.size
                    || resources[0].resource_kind != RenderResourceKind::Texture
                    || !resources[0]
                        .usage
                        .contains(&RenderResourceUsage::DepthTarget)
                {
                    errors.push(RenderGraphDiagnostic::error(
                        "geometry.invalid_depth",
                        "depth must be one matching Depth32Float attachment",
                    ));
                }
            }
        }
    }
    if let Some(proofs) = proofs {
        proofs.truncate(draw_index);
    }
    (errors, validations)
}

#[cfg(test)]
mod attachment_identity_tests {
    use super::*;

    #[test]
    fn shared_view_begin_proof_is_pass_local_and_changed_draws_still_fail() {
        use crate::rhi_command_plan::tests::retained_geometry_graph;
        let graph = retained_geometry_graph(128);
        let mut proofs = Vec::new();
        for _ in 0..2 {
            VIEW_VALUE_COMPARISONS.with(|n| n.set(0));
            let (errors, _) = validate_geometry_graph_with_cache(&graph, &mut proofs);
            assert!(errors.is_empty(), "{errors:?}");
            assert!(
                VIEW_VALUE_COMPARISONS.with(|n| n.get()) <= 4,
                "same begun view compared by value for every draw: {}",
                VIEW_VALUE_COMPARISONS.with(|n| n.get())
            );
        }
        let pass_index = graph
            .passes
            .iter()
            .position(|p| p.pass_kind == RenderPassKind::DrawGeometry)
            .unwrap();
        for kind in 0..3 {
            let mut changed = graph.clone();
            let RenderPassCommand::DrawGeometry { draw } =
                &mut changed.passes[pass_index].commands[64]
            else {
                panic!()
            };
            let draw = Arc::make_mut(draw);
            match kind {
                0 => Arc::make_mut(&mut draw.view).clear_color = Some(color([0.2, 0., 0., 1.])),
                1 => draw.target = "other".into(),
                _ => draw.depth_target = Some("other-depth".into()),
            }
            for _ in 0..2 {
                let (errors, _) = validate_geometry_graph_with_cache(&changed, &mut proofs);
                assert_eq!(
                    errors
                        .iter()
                        .filter(|e| e.code == "geometry.view_not_begun")
                        .count(),
                    1,
                    "changed draw was hidden by a successful neighbor: {errors:?}"
                );
            }
        }
        // A BeginView inside the pass invalidates any earlier begun-view proof.
        let mut changed = graph.clone();
        let mut begin = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find(|c| matches!(c, RenderPassCommand::BeginView { .. }))
            .unwrap()
            .clone();
        let RenderPassCommand::BeginView { view, .. } = &mut begin else {
            panic!()
        };
        Arc::make_mut(view).clear_color = Some(color([0.3, 0., 0., 1.]));
        changed.passes[pass_index].commands.insert(64, begin);
        let (errors, _) = validate_geometry_graph_with_cache(&changed, &mut proofs);
        assert_eq!(
            errors
                .iter()
                .filter(|e| e.code == "geometry.view_not_begun")
                .count(),
            64,
            "a later BeginView reused the earlier declaration proof: {errors:?}"
        );
    }

    #[test]
    fn borrowed_view_identity_avoids_repeated_value_comparisons_and_keeps_value_fallback() {
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 100.)
            .evaluate(CameraPose::default())
            .unwrap();
        VIEW_VALUE_COMPARISONS.with(|count| count.set(0));
        for _ in 0..128 {
            assert!(matching_view(&view, &view));
        }
        assert_eq!(VIEW_VALUE_COMPARISONS.with(|count| count.get()), 0);
        let mut distinct = view.clone();
        assert!(matching_view(&view, &distinct));
        assert_eq!(VIEW_VALUE_COMPARISONS.with(|count| count.get()), 1);
        distinct.viewport[2] -= 1;
        assert!(!matching_view(&view, &distinct));
        assert_eq!(VIEW_VALUE_COMPARISONS.with(|count| count.get()), 2);
        let target = "main".to_owned();
        let changed = "other".to_owned();
        assert!(!matching_attachments(
            (&target, &view, None),
            (&changed, &view, None)
        ));
        assert!(!matching_attachments(
            (&target, &view, None),
            (&target, &view, Some("depth"))
        ));
    }
}

#[cfg(test)]
thread_local! { static SAMPLED_ATLAS_COPIES: std::cell::Cell<usize> = const {std::cell::Cell::new(0)}; }
#[cfg(test)]
mod dynamic_consumption_tests {
    use super::*;
    #[test]
    fn dynamic_consumption_shadow_declarations_copy_unique_atlases_not_per_draw() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let (manager, source, view, light) =
                shadow::receiver_scene(MaterialBlend::Opaque, -100.);
            let instances: Vec<_> = (0..count)
                .map(|id| GeometryInstance {
                    instance_id: id as u64,
                    source: &source,
                    world_transform: {
                        let mut m = IDENTITY_MATRIX;
                        m[14] = -100.;
                        m
                    },
                    visibility: Visibility {
                        authored: true,
                        inherited: Evaluation::Known(true),
                    },
                    layers: LayerSelection::Named { names: vec![] },
                    provenance: source.geometry_source.as_ref().clone(),
                })
                .collect();
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            let mut frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            shadow::append_scene_lighting(
                &mut frame,
                GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                },
                [0.; 3],
                &mut inputs,
                &[light],
            )
            .unwrap();
            let expected: std::collections::BTreeSet<_> = frame
                .views
                .iter()
                .flat_map(|v| &v.draws)
                .filter_map(|d| d.shadows.as_ref())
                .flat_map(|m| m.atlases())
                .map(str::to_owned)
                .collect();
            assert!(!expected.is_empty());
            let mut graph = RenderGraph::new("atlas-consumption", 1);
            graph
                .resources
                .push(RenderResource::surface_backbuffer("main", 64, 64));
            graph.output_target = Some("main".into());
            SAMPLED_ATLAS_COPIES.with(|n| n.set(0));
            frame.append_to_graph(&mut graph);
            let copies = SAMPLED_ATLAS_COPIES.with(|n| n.get());
            let reads: std::collections::BTreeSet<_> = graph
                .passes
                .iter()
                .flat_map(|p| p.reads.iter().cloned())
                .collect();
            assert_eq!(reads, expected);
            assert!(!graph
                .validate()
                .iter()
                .any(|d| d.severity == RenderGraphDiagnosticSeverity::Error));
            observed.push((count, copies, expected.len()));
        }
        eprintln!("actual atlas string copies {observed:?}");
        assert!(
            observed.iter().all(|(_, copies, unique)| copies == unique),
            "per-draw atlas allocations {observed:?}"
        );
    }
}
