use super::*;
use crate::render_geometry::{batch_test_frame, batch_test_graph};
use crate::rhi_command_plan::compile_render_graph_to_rhi_plan;

#[test]
#[ignore = "requires a real GPU"]
fn gpu_batch_commands_match_legacy_pixels_and_consume_all_instance_indices() {
    for blend in [
        MaterialBlend::Opaque,
        MaterialBlend::AlphaMask,
        MaterialBlend::AlphaBlend,
    ] {
        let legacy = batch_test_frame(false, blend, 128, 2);
        let compact = batch_test_frame(true, blend, 128, 2);
        let old = compile_render_graph_to_rhi_plan(&batch_test_graph(&legacy));
        let plan = compile_render_graph_to_rhi_plan(&batch_test_graph(&compact));
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        let reference = backend.render_plan_to_rgba_bytes(&old, 64, 64).unwrap();
        let before = backend.retained_geometry_stats();
        let actual = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
        assert_eq!(actual, reference, "batch semantics changed {blend:?}");
        let red = &actual[(36 * 64 + 16) * 4..][..4];
        let green = &actual[(36 * 64 + 48) * 4..][..4];
        assert!(
            red[0] > red[1] && green[1] > green[0],
            "independent red/green instance values lost: {red:?}/{green:?}"
        );
        assert_eq!(
            backend.retained_geometry_stats().validation_draw_visits
                - before.validation_draw_visits,
            2
        );
        assert_eq!(
            backend.geometry.parameters.as_ref().unwrap().indices.len(),
            256
        );
        let before = backend.retained_geometry_stats();
        assert_eq!(
            backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap(),
            actual
        );
        assert_eq!(
            backend.retained_geometry_stats().validation_draw_visits,
            before.validation_draw_visits,
            "same immutable packet did not reuse complete proof"
        );
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_batch_tail_and_cross_batch_duplicate_fail_before_upload() {
    let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
    let plan = compile_render_graph_to_rhi_plan(&batch_test_graph(&frame));
    let mut invalid = plan.clone();
    let batch = invalid
        .commands
        .iter_mut()
        .find_map(|command| {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::GeometryBatch { batch },
                ..
            } = command
            {
                Some(batch)
            } else {
                None
            }
        })
        .unwrap();
    Arc::make_mut(batch)
        .visible
        .last_mut()
        .unwrap()
        .instance_index = u32::MAX;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    assert!(backend
        .render_plan_to_rgba_bytes(&invalid, 64, 64)
        .unwrap_err()
        .contains("instance_index"));
    assert_eq!(backend.geometry_upload_count(), 0);
    assert_eq!(
        backend.retained_geometry_stats().parameter_uploaded_bytes,
        0
    );
    let mut duplicate = plan.clone();
    let command = plan
        .commands
        .iter()
        .find(|c| c.geometry_draw().is_some())
        .unwrap()
        .clone();
    let at = duplicate
        .commands
        .iter()
        .position(|c| matches!(c, RhiCommand::Submit))
        .unwrap();
    duplicate.commands.insert(at, command);
    assert!(backend
        .render_plan_to_rgba_bytes(&duplicate, 64, 64)
        .unwrap_err()
        .contains("duplicate_view_instance"));
    assert_eq!(backend.geometry_upload_count(), 0);
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_batch_non_head_lease_retires_only_on_real_completion() {
    use crate::engine_rhi::EngineRhiBackend;
    let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
    let mut plan = compile_render_graph_to_rhi_plan(&batch_test_graph(&frame));
    let lease = Arc::new(());
    let weak = Arc::downgrade(&lease);
    let batch = plan
        .commands
        .iter_mut()
        .find_map(|command| {
            if let RhiCommand::Draw {
                payload: RhiDrawPayload::GeometryBatch { batch },
                ..
            } = command
            {
                Some(batch)
            } else {
                None
            }
        })
        .unwrap();
    let table = Arc::make_mut(&mut Arc::make_mut(batch).instances);
    Arc::make_mut(&mut table[127]).resource_lease = lease;
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let report = backend.execute_plan(&plan);
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.severity == crate::engine_rhi::RhiBackendDiagnosticSeverity::Error),
        "{:?}",
        report.diagnostics
    );
    let token = backend.last_gpu_submission().unwrap();
    drop((plan, frame));
    assert!(weak.upgrade().is_some(), "batch kept only the head lease");
    backend
        .wait_gpu_submission(token, std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        weak.upgrade().is_none(),
        "completed packet still pins non-head lease"
    );
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
#[ignore = "requires a real GPU"]
fn gpu_batch_motion_preserves_selected_instance_bundle_starts() {
    let mut frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 2);
    // Exercise the unchanged Unlit execution state, whose complete native
    // recipe is safe to retain while only camera matrices change.
    for view in &mut frame.views {
        for batch in view.batches.as_mut().unwrap() {
            let batch = Arc::make_mut(batch);
            for instance in Arc::make_mut(&mut batch.instances) {
                Arc::make_mut(instance).standard_lit = None;
            }
            let head = Arc::make_mut(&mut batch.draw);
            head.standard_lit = None;
            head.lighting = None;
            batch.validate().unwrap();
        }
        view.draws = view
            .batches
            .as_ref()
            .unwrap()
            .iter()
            .map(|b| b.draw.clone())
            .collect();
    }
    let plan = compile_render_graph_to_rhi_plan(&batch_test_graph(&frame));
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
    let first = backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap();
    assert!(first.chunks_exact(4).any(|p| p[0] > p[1]));
    assert!(first.chunks_exact(4).any(|p| p[1] > p[0]));
    assert_eq!(
        backend
            .geometry
            .validated_plan
            .as_ref()
            .unwrap()
            .bundles
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 128]
    );
    let mut moved = plan.clone();
    let mut views = std::collections::BTreeMap::new();
    for command in &mut moved.commands {
        match command {
            RhiCommand::BeginView { view, .. } => {
                let mut value = view.as_ref().clone();
                value.view_matrix[12] = 0.1f32.into();
                let value = Arc::new(value);
                views.insert(value.view_id, value.clone());
                *view = value;
            }
            RhiCommand::Draw {
                payload: RhiDrawPayload::GeometryBatch { batch },
                ..
            } => {
                let batch = Arc::make_mut(batch);
                Arc::make_mut(&mut batch.draw).view = views[&batch.draw.view.view_id].clone();
            }
            _ => (),
        }
    }
    backend
        .prepare_geometry_plan_for_extent(&moved, [64, 64], None)
        .unwrap();
    assert_eq!(
        backend
            .geometry
            .validated_plan
            .as_ref()
            .unwrap()
            .bundles
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 128],
        "bundle lifetime ranges used command counts instead of selected instance counts"
    );
    let second = backend.render_plan_to_rgba_bytes(&moved, 64, 64).unwrap();
    assert_ne!(first, second, "actual Unlit camera movement disappeared");
    backend
        .shutdown_gpu(std::time::Duration::from_secs(5))
        .unwrap();
}
