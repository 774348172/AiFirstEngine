#![cfg(feature = "real-wgpu")]
use engine_runtime::engine_rhi::EngineRhiBackend;
use engine_runtime::render_contract::*;
use engine_runtime::render_geometry::*;
use engine_runtime::render_graph::*;
use engine_runtime::render_resource::*;
use engine_runtime::rhi_command_plan::*;
use engine_runtime::wgpu_backend::real::*;
use std::time::Duration;
#[path = "support/geometry_fixture.rs"]
mod fixture;
use fixture::*;
fn pixel(bytes: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    bytes[(y * width + x) * 4..(y * width + x) * 4 + 4]
        .try_into()
        .unwrap()
}

#[test]
#[ignore = "real GPU; reverse consumption of immutable content generations"]
fn gpu_f04_reverse_content_generations_keep_transform_material_and_presentation_alpha() {
    use engine_runtime::render_thread::{RenderFrameIdentity, RenderFrameSourceIdentity};
    use std::sync::Arc;
    // Retain three/four immutable plans and consume them in reverse directly at
    // native. This does not inject delay into the actual render worker.
    for count in [3usize, 4] {
        let mut manager = RenderResourceManager::with_safe_frame_delay(0);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        backend.set_submission_capture_enabled(true);
        let colors = [
            [1., 0., 0., 1.],
            [0., 1., 0., 0.5],
            [0., 0., 1., 0.25],
            [1., 1., 0., 0.75],
        ];
        let mut plans = Vec::new();
        let mut handles = Vec::new();
        for (i, color) in colors.iter().copied().enumerate().take(count) {
            let mesh = prepare(
                &format!("f04-mesh-{i}"),
                RenderAssetData::Mesh(triangle()),
                &mut manager,
            );
            let mut material = unlit(color);
            if color[3] < 1. {
                material.blend = MaterialBlend::AlphaBlend;
                material.depth = MaterialDepth::TestOnly;
            }
            let mat = prepare(
                &format!("f04-material-{i}"),
                RenderAssetData::Material(material),
                &mut manager,
            );
            let source = PreparedGeometry::from_assets(&mesh, &mat, 0, &manager).unwrap();
            let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
            let position = [(i as f32 - 1.5) * 0.2, 0., -2.];
            let f = GeometryFrame::build(
                "main",
                [64, 64],
                &[GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                }],
                &[instance(&source, 1, position)],
                &manager,
                DiagnosticLevel::Summary,
            )
            .unwrap();
            let mut p = compile_render_graph_to_rhi_plan(&graph(&f));
            p.frame_index = i as u64 + 1;
            p.frame_identity = Some(Arc::new(RenderFrameIdentity {
                source: Arc::new(RenderFrameSourceIdentity {
                    run_id: "f04-owner".into(),
                    run_epoch: 1,
                    session_id: "delayed-content".into(),
                    package_content_hash: None,
                    world_epoch: 1,
                }),
                frame_index: p.frame_index,
                tick_pair: [i as u64, i as u64 + 1],
                presentation_alpha_bits: (i as f32 / 4.).to_bits(),
                views: Arc::new(Vec::new()),
            }));
            assert!(!p.has_errors());
            for slot in f.views[0].draws[0]
                .geometry_binding
                .slots
                .iter()
                .chain(&f.views[0].draws[0].material_binding.slots)
            {
                handles.push(slot.resource.raw());
            }
            plans.push((p, position, color));
        }
        let sealed: Vec<_> = plans
            .iter()
            .map(|(p, _, _)| serde_json::to_vec(p).unwrap())
            .collect();
        for handle in handles {
            manager
                .release_resource(RenderResourceReleaseRequest {
                    handle,
                    frame_index: 10,
                })
                .unwrap();
        }
        assert!(
            manager.end_frame(1000).pending_release_count > 0,
            "CPU frames cannot retire delayed immutable packets"
        );
        for i in (0..count).rev() {
            let (plan, position, color) = &plans[i];
            assert_eq!(serde_json::to_vec(plan).unwrap(), sealed[i]);
            let (pixels, token) = backend
                .render_plan_to_rgba_bytes_with_receipt(plan, 64, 64, None)
                .unwrap();
            let x = (32. + position[0] * 32. / (2. * (30f32.to_radians()).tan())).round() as usize;
            let actual = pixel(&pixels, 64, x, 32);
            let expected = [
                color[0] * color[3] * 255.,
                color[1] * color[3] * 255.,
                color[2] * color[3] * 255.,
                255.,
            ];
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(a, e)| (*a as f32 - e).abs() <= 1.),
                "generation {i} consumed later material/alpha: {actual:?} vs {expected:?}"
            );
            let observations = backend.submission_observations();
            let observation = observations.iter().find(|o| o.token == token).unwrap();
            assert_eq!(observation.frame_identity, plan.frame_identity);
            assert_eq!(
                backend
                    .wait_gpu_submission(token, Duration::from_secs(3))
                    .unwrap(),
                GpuSubmissionStatus::Completed
            );
        }
        drop(plans);
        assert_eq!(
            manager.end_frame(1001).pending_release_count,
            0,
            "all leases retire only after real GPU completion and packet release"
        );
    }
}

#[test]
#[ignore = "real GPU required"]
fn gpu_frame_identity_survives_delayed_old_plan_capture_and_rejects_mismatch() {
    use engine_runtime::render_thread::{RenderFrameIdentity, RenderFrameSourceIdentity};
    use std::sync::Arc;
    let mut manager = RenderResourceManager::with_safe_frame_delay(0);
    let source = source(&mut manager);
    let f = frame(&source, &manager);
    let mut old = compile_render_graph_to_rhi_plan(&graph(&f));
    old.frame_index = 41;
    let identity = Arc::new(RenderFrameIdentity {
        source: Arc::new(RenderFrameSourceIdentity {
            run_id: "synthetic-owner-fixture".into(),
            run_epoch: 7,
            session_id: "delayed-fixture".into(),
            package_content_hash: None,
            world_epoch: 9,
        }),
        frame_index: 41,
        tick_pair: [10, 11],
        presentation_alpha_bits: 0.25f32.to_bits(),
        views: Arc::new(Vec::new()),
    });
    old.frame_identity = Some(identity.clone());
    let mut backend = RealWgpuBackend::new_offscreen_measured(64, 64, 2).unwrap();
    backend.execute_plan(&old);
    let first = backend.last_gpu_submission().unwrap();
    let frozen = backend.submission_observations()[0].clone();
    let mut newer = old.clone();
    newer.frame_index = 42;
    let mut next_identity = (*identity).clone();
    next_identity.frame_index = 42;
    next_identity.tick_pair = [11, 12];
    next_identity.presentation_alpha_bits = 0.75f32.to_bits();
    Arc::make_mut(&mut next_identity.source).world_epoch = 10;
    newer.frame_identity = Some(Arc::new(next_identity));
    backend.execute_plan(&newer);
    let second = backend.last_gpu_submission().unwrap();
    assert_ne!(first.serial, second.serial);
    let (pixels, captured) = backend
        .render_plan_to_rgba_bytes_with_receipt(&old, 64, 64, None)
        .unwrap();
    assert_eq!(pixel(&pixels, 64, 32, 32), [255, 0, 0, 255]);
    assert_ne!(backend.last_gpu_submission().unwrap(), captured);
    assert!(backend.record_submission_capture(captured));
    let observations = backend.submission_observations();
    assert_eq!(frozen.frame_identity.as_ref(), Some(&identity));
    let capture = observations.iter().find(|o| o.token == captured).unwrap();
    assert!(capture.offscreen_capture && capture.capture_count == 1);
    assert_eq!(capture.frame_identity.as_ref(), Some(&identity));
    assert_ne!(
        observations
            .iter()
            .find(|o| o.token == second)
            .unwrap()
            .frame_identity,
        capture.frame_identity
    );
    let submitted = backend.gpu_lifecycle_stats().submitted;
    let mut invalid = old.clone();
    invalid.frame_index = 999;
    assert!(invalid.has_errors());
    backend.execute_plan(&invalid);
    assert_eq!(
        backend.gpu_lifecycle_stats().submitted,
        submitted,
        "mismatched source cannot submit"
    );
    let measurement = backend.finish_frame_measurement().unwrap();
    assert_eq!(measurement.status, "complete");
    assert_eq!(measurement.samples.len(), 2);
    assert_eq!(
        measurement.samples[0].frame_identity.as_ref(),
        Some(&identity)
    );
    assert_eq!(measurement.samples[0].frame_index, 41);
    assert_eq!(measurement.samples[1].frame_index, 42);
    assert_eq!(measurement.submission_observations_omitted, 0);
}

#[test]
#[ignore = "real GPU required"]
fn gpu_lifetime_real_completion_keeps_native_leases_and_rejects_old_epochs() {
    let mut manager = RenderResourceManager::with_safe_frame_delay(0);
    let source = source(&mut manager);
    let f = frame(&source, &manager);
    let mut plan = compile_render_graph_to_rhi_plan(&graph(&f));
    plan.frame_index = 41;
    let handles = f.views[0].draws[0]
        .geometry_binding
        .slots
        .iter()
        .chain(&f.views[0].draws[0].material_binding.slots)
        .map(|s| s.resource.raw())
        .collect::<Vec<_>>();
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    backend.set_submission_capture_enabled(true);
    let report = backend.execute_plan(&plan);
    assert!(!report
        .diagnostics
        .iter()
        .any(|d| d.severity == engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity::Error));
    let token = backend.last_gpu_submission().unwrap();
    assert_eq!(token.frame_index, 41);
    assert_eq!(
        backend.submission_observations()[0].surface_present_calls,
        0
    );
    for handle in &handles {
        manager
            .release_resource(RenderResourceReleaseRequest {
                handle: *handle,
                frame_index: 42,
            })
            .unwrap();
    }
    assert!(
        manager.end_frame(1000).pending_release_count > 0,
        "CPU frame advancement cannot retire a retained frame"
    );
    drop((plan, f));
    assert!(
        manager.end_frame(1001).pending_release_count > 0,
        "in-flight GPU callback owns native leases"
    );
    assert_eq!(
        backend
            .wait_gpu_submission(token, Duration::from_secs(3))
            .unwrap(),
        GpuSubmissionStatus::Completed
    );
    assert_eq!(manager.end_frame(1002).pending_release_count, 0);
    let source2 = fixture::source(&mut manager);
    let f2 = frame(&source2, &manager);
    let p2 = compile_render_graph_to_rhi_plan(&graph(&f2));
    backend.execute_plan(&p2);
    let pending = backend.last_gpu_submission().unwrap();
    let previous = backend.gpu_lifecycle_stats().completed;
    backend.abandon_device_epoch();
    assert_eq!(backend.gpu_lifecycle_stats().completed, previous);
    assert_eq!(
        backend.submission_status(pending),
        Some(GpuSubmissionStatus::Abandoned)
    );
    assert_eq!(
        backend.submission_status(token),
        Some(GpuSubmissionStatus::Completed),
        "real completion evidence remains true after later device loss"
    );
    let mut rebuilt = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    assert_ne!(
        rebuilt.gpu_lifecycle_stats().device_epoch,
        token.device_epoch
    );
    assert!(rebuilt
        .wait_gpu_submission(token, Duration::ZERO)
        .unwrap_err()
        .contains("stale"));
    assert!(!rebuilt.record_surface_present(token));
    assert_eq!(rebuilt.gpu_lifecycle_stats().unmatched_observations, 1);
}

#[test]
#[ignore = "real GPU required"]
fn gpu_lifetime_additional_dependency_pass_and_transient_depth_alias_readback() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let mut left = RenderView::perspective(1, 64, 64, 60., 0.1, 10.);
    left.viewport = [0, 0, 32, 64];
    let mut right = left.clone();
    right.view_id = 2;
    right.viewport = [32, 0, 32, 64];
    let mut f = GeometryFrame::build(
        "main",
        [64, 64],
        &[
            GeometryViewInput {
                view: &left,
                pose: CameraPose::default(),
            },
            GeometryViewInput {
                view: &right,
                pose: CameraPose::default(),
            },
        ],
        &[instance(&source, 7, [0., 0., -2.])],
        &manager,
        DiagnosticLevel::Off,
    )
    .unwrap();
    std::sync::Arc::make_mut(&mut f.views[1].draws[0]).base_color =
        [0., 1., 0., 1.].map(Into::into);
    let mut g = graph(&f);
    let mut passes = std::mem::take(&mut g.passes);
    let clear = passes.remove(0);
    let begin_a = passes.remove(0);
    let begin_b = passes.remove(0);
    let draw_a = passes.remove(0);
    let draw_b = passes.remove(0);
    let mut extra = draw_b.clone();
    extra.pass_id = "additional-depth-consumer".into();
    extra.pass_name = "Additional depth consumer".into();
    if let RenderPassCommand::DrawGeometry { draw } = &mut extra.commands[0] {
        std::sync::Arc::make_mut(draw).instance_id = 78;
        std::sync::Arc::make_mut(draw).base_color = [0., 0., 1., 1.].map(Into::into);
    }
    let extra_id = extra.pass_id.clone();
    let draw_b_id = draw_b.pass_id.clone();
    g.passes = vec![clear, begin_a, draw_a, begin_b, draw_b, extra.clone()];
    g.passes.extend(passes);
    g.contracts
        .before
        .push((GraphPassId(draw_b_id), GraphPassId(extra_id)));
    let producer = g.passes[2].pass_id.clone();
    let consumer = g.passes[3].pass_id.clone();
    g.contracts
        .before
        .push((GraphPassId(producer), GraphPassId(consumer)));
    let s = g.compile_schedule().unwrap();
    let depths = s
        .lifetimes
        .iter()
        .filter(|l| l.resource.0.contains("depth"))
        .collect::<Vec<_>>();
    assert_eq!(depths.len(), 2);
    assert_eq!(depths[0].transient_slot, depths[1].transient_slot);
    let plan = compile_render_graph_to_rhi_plan(&g);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let pixels = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert_eq!(pixel(&pixels, 64, 16, 32), [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 64, 48, 32), [0, 0, 255, 255]);
    assert_eq!(
        backend.retained_geometry_stats().depth_allocations,
        1,
        "two nonoverlap logical depth resources share one real allocation"
    );
    // The original all-begins-before-draws schedule overlaps, so must allocate independent depths.
    let mut overlap_graph = graph(&f);
    let at = overlap_graph
        .passes
        .iter()
        .position(|p| p.pass_kind == RenderPassKind::Present)
        .unwrap_or(overlap_graph.passes.len());
    overlap_graph.passes.insert(at, extra);
    let overlap = compile_render_graph_to_rhi_plan(&overlap_graph);
    let mut separate = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    assert_eq!(
        separate
            .render_plan_to_rgba_bytes(&overlap, 64, 64)
            .unwrap(),
        pixels
    );
    assert_eq!(separate.retained_geometry_stats().depth_allocations, 2);
    assert_eq!(
        backend.render_plan_to_rgba_bytes(&overlap, 64, 64).unwrap(),
        pixels,
        "cached aliases must be rebuilt when the next graph overlaps"
    );
    assert_eq!(backend.retained_geometry_stats().depth_allocations, 3);
}

#[test]
#[ignore = "real GPU required"]
fn gpu_lifetime_real_timestamps_are_bound_to_submission_and_pixel_capture() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let f = frame(&source, &manager);
    let mut plan = compile_render_graph_to_rhi_plan(&graph(&f));
    plan.frame_index = 73;
    let mut backend = RealWgpuBackend::new_offscreen_measured(64, 64, 2).unwrap();
    backend.execute_plan(&plan);
    let old = backend.last_gpu_submission().unwrap();
    let (before, old_capture) = backend
        .render_plan_to_rgba_bytes_with_receipt(&plan, 64, 64, None)
        .unwrap();
    assert!(backend.record_submission_capture(old_capture));
    assert_ne!(old, old_capture);
    assert_eq!(old.content_version, old_capture.content_version);
    let mut changed = f.clone();
    std::sync::Arc::make_mut(&mut changed.views[0].draws[0]).base_color =
        [0., 1., 0., 1.].map(Into::into);
    let mut plan = compile_render_graph_to_rhi_plan(&graph(&changed));
    plan.frame_index = 74;
    backend.execute_plan(&plan);
    let new = backend.last_gpu_submission().unwrap();
    let (after, new_capture) = backend
        .render_plan_to_rgba_bytes_with_receipt(&plan, 64, 64, None)
        .unwrap();
    assert!(backend.record_submission_capture(new_capture));
    assert_ne!(before, after);
    assert_ne!(old.content_version, new.content_version);
    let report = backend.finish_frame_measurement().unwrap();
    assert_eq!(report.status, "complete");
    assert_eq!(report.samples.len(), 2);
    assert_eq!(report.samples[0].submission, Some(old));
    assert_eq!(report.samples[1].submission, Some(new));
    assert!(report
        .samples
        .iter()
        .all(|s| s.frame_ms > 0. && s.frame_ms.is_finite()));
    assert_eq!(report.submissions.len(), 6);
    assert_eq!(
        report
            .submissions
            .iter()
            .filter(|s| s.token.frame_index == 0 && !s.offscreen_capture)
            .count(),
        2,
        "each capture retains its independent maintenance copy submission"
    );
    assert!(report
        .submissions
        .iter()
        .all(|s| s.capture_count == u32::from(s.offscreen_capture)
            && s.surface_present_calls == 0
            && s.status == GpuSubmissionStatus::Completed));
    if let Some(path) = std::env::var_os("S1D_GPU_EVIDENCE_DIR") {
        std::fs::write(
            std::path::Path::new(&path).join("timestamp-submission-report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}

#[test]
#[ignore = "real GPU required"]
fn gpu_lifetime_invalid_graph_is_rejected_before_any_gpu_submission() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let f = frame(&source, &manager);
    let mut g = graph(&f);
    g.contracts.before.push((
        GraphPassId(g.passes[1].pass_id.clone()),
        GraphPassId(g.passes[1].pass_id.clone()),
    ));
    let plan = compile_render_graph_to_rhi_plan(&g);
    assert!(plan.has_errors());
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    assert!(backend.render_plan_to_rgba_bytes(&plan, 64, 64).is_err());
    assert_eq!(backend.gpu_lifecycle_stats().submitted, 0);
    assert_eq!(backend.geometry_upload_count(), 0);
    let mut g = graph(&f);
    let depth = g
        .resources
        .iter()
        .find(|r| r.format == RenderResourceFormat::Depth32Float)
        .unwrap()
        .resource_id
        .clone();
    g.contracts.resources.insert(
        GraphResourceId(depth),
        GraphResourceShape {
            samples: 4,
            ..Default::default()
        },
    );
    // Make all attachments agree; the neutral graph is legal, this backend profile is not.
    for r in &g.resources {
        g.contracts.resources.insert(
            GraphResourceId(r.resource_id.clone()),
            GraphResourceShape {
                samples: 4,
                ..Default::default()
            },
        );
    }
    let p = compile_render_graph_to_rhi_plan(&g);
    assert!(!p.has_errors(), "{:?}", p.diagnostics);
    assert!(backend
        .render_plan_to_rgba_bytes(&p, 64, 64)
        .unwrap_err()
        .contains("unsupported_backend"));
    assert_eq!(backend.gpu_lifecycle_stats().submitted, 0);
    assert_eq!(backend.geometry_upload_count(), 0);
}

#[test]
#[ignore = "real GPU required; F08 graph fault consumers, not full BM11 scale qualification"]
fn gpu_f08_all_declared_graph_faults_reject_before_submit_and_keep_old_success_valid() {
    let mut manager = RenderResourceManager::new();
    let source = source(&mut manager);
    let f = frame(&source, &manager);
    let valid_graph = graph(&f);
    let valid_plan = compile_render_graph_to_rhi_plan(&valid_graph);
    assert!(!valid_plan.has_errors());
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let before = backend
        .render_plan_to_rgba_bytes(&valid_plan, 64, 64)
        .unwrap();
    assert_eq!(pixel(&before, 64, 32, 32), [255, 0, 0, 255]);

    for (case, expected) in [
        ("uninitialized", "graph.uninitialized_read"),
        ("feedback", "graph.feedback"),
        ("subresource", "graph.invalid_subresource"),
        ("format", "graph.depth_format"),
        ("samples", "graph.attachment_mismatch"),
        ("history", "graph.invalid_history"),
    ] {
        // Keep the graph/plan identity, resource handles and valid predecessor.
        // A cached success must not suppress new declaration failures.
        let mut invalid = valid_graph.clone();
        let depth = invalid
            .resources
            .iter()
            .position(|r| r.format == RenderResourceFormat::Depth32Float)
            .unwrap();
        let target = invalid.output_target.clone().unwrap();
        match case {
            "uninitialized" => {
                let mut scratch = invalid.resources[0].clone();
                scratch.resource_id = "never-initialized".into();
                scratch.lifetime = RenderResourceLifetime::FrameLocal;
                invalid.passes[0].reads.push(scratch.resource_id.clone());
                invalid.resources.push(scratch);
            }
            "feedback" => invalid.passes[0].reads.push(target.clone()),
            "subresource" => {
                invalid.contracts.accesses.insert(
                    GraphPassId(invalid.passes[0].pass_id.clone()),
                    vec![GraphAccess {
                        resource: GraphResourceId(target.clone()),
                        aspect: GraphAspect::Color,
                        mip: 1,
                        layer: 0,
                        kind: GraphAccessKind::Write,
                    }],
                );
            }
            "format" => invalid.resources[depth].format = RenderResourceFormat::Rgba8Unorm,
            "samples" => {
                invalid.contracts.resources.insert(
                    GraphResourceId(invalid.resources[depth].resource_id.clone()),
                    GraphResourceShape {
                        samples: 4,
                        ..Default::default()
                    },
                );
            }
            "history" => {
                invalid.contracts.resources.insert(
                    GraphResourceId(invalid.resources[depth].resource_id.clone()),
                    GraphResourceShape {
                        history: Some(GraphHistoryIdentity {
                            device_epoch: 1,
                            origin_epoch: 1,
                            camera_id: 1,
                            resource_version: 1,
                            extent: [1, 1],
                        }),
                        ..Default::default()
                    },
                );
            }
            _ => unreachable!(),
        }
        let plan = compile_render_graph_to_rhi_plan(&invalid);
        assert!(plan.has_errors(), "{case} incorrectly reused old success");
        assert!(
            plan.diagnostics.iter().any(|d| d.code == expected),
            "{case}: expected {expected}, actual {:?}",
            plan.diagnostics
        );
        let submitted = backend.gpu_lifecycle_stats().submitted;
        let uploads = backend.geometry_upload_count();
        assert!(backend.render_plan_to_rgba_bytes(&plan, 64, 64).is_err());
        assert_eq!(backend.gpu_lifecycle_stats().submitted, submitted, "{case}");
        assert_eq!(backend.geometry_upload_count(), uploads, "{case}");
        eprintln!("F08 {case}: {expected}; illegal submissions=0, new uploads=0");
    }
    let after = backend
        .render_plan_to_rgba_bytes(&valid_plan, 64, 64)
        .unwrap();
    assert_eq!(before, after, "faults must not corrupt the valid old plan");
    backend.shutdown_gpu(Duration::from_secs(5)).unwrap();
    assert_eq!(backend.gpu_lifecycle_stats().retiring_bytes, 0);
}
