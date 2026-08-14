#[cfg(feature = "real-window")]
use engine_runtime::windowed_continuous_runtime::WindowedContinuousBackendKind;
use engine_runtime::windowed_continuous_runtime::{
    run_headless_windowed_continuous_runtime, WindowedContinuousRuntimeReport,
    WindowedContinuousRuntimeRequest,
};

pub fn headless_windowed_continuous_runtime_gate() -> WindowedContinuousRuntimeReport {
    run_headless_windowed_continuous_runtime(WindowedContinuousRuntimeRequest::default())
}

#[cfg(feature = "real-window")]
pub fn real_windowed_continuous_runtime_smoke_plan() -> WindowedContinuousRuntimeReport {
    let request = WindowedContinuousRuntimeRequest {
        backend_kind: WindowedContinuousBackendKind::RealWindowSmoke,
        ..Default::default()
    };
    run_headless_windowed_continuous_runtime(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_windowed_continuous_runtime_gate_runs_same_runtime_chain() {
        let report = headless_windowed_continuous_runtime_gate();

        assert!(report.ok);
        assert_eq!(
            report.schema_version,
            "windowed-continuous-runtime-report.v1"
        );
        assert_eq!(report.final_ecs_position_x, 3.0);
        assert_eq!(report.final_render_position_x, Some(3.0));
        assert_eq!(report.frames.len(), 5);
        assert!(report
            .frames
            .iter()
            .all(|frame| frame.present.status == "presented"));
    }

    #[cfg(feature = "real-window")]
    #[test]
    #[ignore = "real window / gpu smoke gate is local-only"]
    fn real_windowed_continuous_runtime_smoke() {
        let report = real_windowed_continuous_runtime_smoke_plan();

        assert!(report.ok);
        assert_eq!(report.backend_kind, "real-window-smoke");
    }
}
