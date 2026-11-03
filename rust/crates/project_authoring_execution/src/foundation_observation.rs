//! Read-only projection of retained Player evidence; never reads a live World.
use engine_runtime::render_thread::RenderFrameIdentity;
use runtime_player_winit::{NativeWindowHostReport, NativeWindowScreenshotStatus};
use serde_json::{json, Value};

fn validate_identity(
    id: &RenderFrameIdentity,
    player: &NativeWindowHostReport,
    frame: u64,
) -> Result<(), String> {
    let alpha = f32::from_bits(id.presentation_alpha_bits);
    if id.source.run_id != player.run_id
        || id.source.run_epoch == 0
        || id.source.world_epoch == 0
        || id.frame_index != frame
        || frame == 0
        || frame > player.frames_completed
        || id.tick_pair[0] > id.tick_pair[1]
        || !alpha.is_finite()
        || !(0.0..=1.0).contains(&alpha)
        || id.views.iter().any(|v| {
            v.origin_bits
                .iter()
                .any(|b| !f64::from_bits(*b).is_finite())
        })
    {
        return Err("foundation.observe_frame_identity_mismatch".into());
    }
    let mut views = std::collections::BTreeSet::new();
    if id.views.iter().any(|v| !views.insert(v.view_id)) {
        return Err("foundation.observe_view_identity_duplicate".into());
    }
    Ok(())
}

/// Projects available dimensions without upgrading their qualification status.
/// Trace and capture each retain their own frame; an earlier capture is never
/// combined with the final World or labelled as an OS display observation.
pub fn foundation_player_observation(player: &NativeWindowHostReport) -> Result<Value, String> {
    let summary = player.frame_performance_summary.as_ref();
    let mut trace_identity = None;
    if let Some(summary) = summary {
        let mut previous = 0;
        for sample in &summary.raw_cpu_frames {
            if sample.frame_index <= previous {
                return Err("foundation.observe_frame_order".into());
            }
            previous = sample.frame_index;
            if let Some(id) = &sample.frame_identity {
                validate_identity(id, player, sample.frame_index)?;
                trace_identity = Some(id);
            }
        }
    }
    let capture = &player.screenshot;
    if let Some(id) = &capture.frame_identity {
        validate_identity(id, player, id.frame_index)?;
        let token = capture
            .submission
            .ok_or("foundation.observe_capture_submission_missing")?;
        if token.device_epoch == 0 || token.serial == 0 || token.frame_index != id.frame_index {
            return Err("foundation.observe_capture_frame_mismatch".into());
        }
        if summary.is_some_and(|s| {
            s.raw_cpu_frames.iter().any(|sample| {
                sample.frame_index == id.frame_index
                    && sample
                        .frame_identity
                        .as_ref()
                        .is_some_and(|trace| trace != id)
            })
        }) {
            return Err("foundation.observe_capture_source_mismatch".into());
        }
    }
    let native = summary
        .and_then(|s| s.gpu.as_ref())
        .and_then(|g| g.native_present.as_ref());
    if let Some(native) = native {
        for frame in &native.frames {
            if let Some(id) = &frame.frame_identity {
                validate_identity(id, player, frame.submission.frame_index)?;
            }
        }
    }
    Ok(json!({
        "source":"retained_player_report", "runtimeRunId":player.run_id,
        "framesCompleted":player.frames_completed,
        "traceIdentity":trace_identity,
        "traceIdentityStatus":if trace_identity.is_some() {"observed"} else {"unavailable"},
        "capture":capture,
        "captureIdentityStatus":if capture.status == NativeWindowScreenshotStatus::Captured && capture.frame_identity.is_some() {"observed"} else {"unavailable"},
        "measurementPolicy":player.measurement_policy,
        "samplingWindow":summary.and_then(|s| s.sampling_window.as_ref()),
        "cpu":summary.and_then(|s| s.cpu_excluding_present_wait.as_ref()),
        "gpu":summary.and_then(|s| s.gpu.as_ref()).and_then(|g| g.frame.as_ref()),
        "resources":summary.and_then(|s| s.sampling_window.as_ref()).and_then(|w| w.streaming_resources_end),
        "resourceScope":"sampled loader/backend fields; not all engine allocations or driver memory",
        "actualDisplay":{
            "status":if native.is_some_and(|n| n.frames.iter().any(|f| f.actual_present_time_ns.is_some())) {"observed_unqualified"} else {"unavailable"},
            "qualified":false,
            "source":native.map(|n| &n.source), "instrumentStatus":native.map(|n| &n.status),
            "unobservedFrames":native.map(|n| n.unobserved_frames),
            "unmatchedEvents":native.map(|n| n.unmatched_events),
            "duplicateEvents":native.map(|n| n.duplicate_events),
            "discardedFrames":native.and_then(|n| n.discarded_frames),
            "repeatedDisplays":native.and_then(|n| n.repeated_displays),
            "correlation":native.and_then(|n| n.windows_etw.as_ref()).and_then(|e| e.correlated.as_ref()),
            "errors":native.map(|n| &n.errors)
        },
        "qualification":"not_assessed_by_observe"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::render_thread::{GpuSubmissionToken, RenderFrameSourceIdentity};
    use std::sync::Arc;

    #[test]
    fn retained_capture_uses_original_runtime_identity_and_rejects_mixed_runs() {
        let mut player = NativeWindowHostReport::base(
            &runtime_player_winit::NativePlayerWindowRunRequest::headless_surface_gate(
                "not-loaded",
            ),
        );
        player.run_id = "original-runtime".into();
        player.frames_completed = 16;
        player.screenshot.frame_identity = Some(Arc::new(RenderFrameIdentity {
            source: Arc::new(RenderFrameSourceIdentity {
                run_id: player.run_id.clone(),
                run_epoch: 1,
                session_id: "session".into(),
                package_content_hash: None,
                world_epoch: 1,
            }),
            frame_index: 8,
            tick_pair: [7, 8],
            presentation_alpha_bits: 0.5f32.to_bits(),
            views: Arc::new(Vec::new()),
        }));
        player.screenshot.submission = Some(GpuSubmissionToken {
            device_epoch: 1,
            serial: 1,
            frame_index: 8,
            content_version: 1,
        });
        let frozen = player.screenshot.frame_identity.clone();
        let observed = foundation_player_observation(&player).unwrap();
        assert_eq!(observed["runtimeRunId"], "original-runtime");
        assert_eq!(player.screenshot.frame_identity, frozen);
        player.run_id = "different-job-or-runtime".into();
        assert_eq!(
            foundation_player_observation(&player).unwrap_err(),
            "foundation.observe_frame_identity_mismatch"
        );
        assert_eq!(player.screenshot.frame_identity, frozen);
    }
}
