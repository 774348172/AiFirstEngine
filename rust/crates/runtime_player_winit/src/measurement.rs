//! Explicit measurement policy, independent from gameplay diagnostics.
//! Summary retains aggregates; Trace exports only the requested frame interval.
use engine_runtime::windowed_player::WindowedPlayerFramePerformanceSummary;
#[cfg(feature = "real-window")]
use engine_runtime::windowed_player::WindowedPlayerGpuPerformance;
use serde::{Deserialize, Serialize};

/// Shared bound for CPU histories and native present observations in one request.
pub const MAX_CAPTURE_FRAMES: u64 = 262_144;
/// Inventory scans are explicit diagnostics, bounded independently of Trace length.
pub const MAX_ALLOCATION_CAPTURE_FRAMES: u64 = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementLevel {
    #[default]
    Off,
    Summary,
    Trace,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasurementPolicy {
    pub level: MeasurementLevel,
    /// Explicit common CPU/window eligibility probe for Off/Summary cost comparison.
    /// Does not enable GPU queries, UI counters, raw Trace or native capture.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cpu_probe: bool,
    /// Explicit opt-in to system ETW capture; CPU/GPU tracing does not require it.
    #[serde(default)]
    pub native_events: bool,
    /// Opt-in allocation census start, inside the filtered Trace interval.
    /// A bounded consecutive interval still does not establish complete heap coverage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocation_frame: Option<u64>,
    /// Optional exclusive end of a consecutive allocation capture interval.
    /// Omission preserves the historical single-frame request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocation_end_frame: Option<u64>,
    /// Explicit per-sampled-frame census reduced to bounded peaks, not raw history.
    /// Includes startup only when warmup is zero. Never implies full heap coverage.
    #[serde(default, skip_serializing_if = "is_false")]
    pub allocation_summary: bool,
    /// Inclusive engine frame index, not a position in a filtered report.
    #[serde(default)]
    pub first_frame: Option<u64>,
    /// Exclusive engine frame index.
    #[serde(default)]
    pub end_frame: Option<u64>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(any(test, feature = "real-window"))]
pub(crate) fn capture_render_identity(
    work: &mut crate::AuiFrameWorkCounters,
    policy: MeasurementPolicy,
    identity: Option<&std::sync::Arc<engine_runtime::render_thread::RenderFrameIdentity>>,
) {
    work.tick_pair = identity.map(|id| id.tick_pair);
    // Off's common CPU probe needs only tick bounds, not a retained per-frame
    // source/view graph. Summary and Trace keep their existing identity data.
    work.frame_identity = policy.enabled().then(|| identity.cloned()).flatten();
}

/// Reduce existing bounded samples after the run. Ordinary Off has no history.
/// Reject gaps instead of fitting a compressed sequence with a false frame axis.
#[cfg(any(test, feature = "real-window"))]
pub(crate) fn memory_window(
    frames: &[crate::AuiFrameWorkCounters],
    requested: u64,
) -> (Option<u64>, Option<f64>) {
    if frames.len() as u64 != requested
        || frames.iter().enumerate().any(|(i, frame)| {
            frame.frame_index != i as u64 + 1
                || frame
                    .process_memory
                    .is_none_or(|m| m.system_available_bytes.is_none())
        })
    {
        return (None, None);
    }
    let available = frames
        .iter()
        .filter_map(|frame| frame.process_memory?.system_available_bytes)
        .min();
    if frames.len() < 1200 {
        return (available, None);
    }
    let tail = &frames[frames.len() - 1200..];
    // Subtract an integer baseline before conversion to preserve a small slope
    // even when the process has a large private-commit offset.
    let baseline = tail[0].process_memory.unwrap().private_commit_bytes as i128;
    let mut numerator = 0.;
    let mut denominator = 0.;
    for (i, frame) in tail.iter().enumerate() {
        let x = i as f64 - 599.5;
        let y = (frame.process_memory.unwrap().private_commit_bytes as i128 - baseline) as f64;
        numerator += x * y;
        denominator += x * x;
    }
    (available, Some(numerator / denominator))
}

#[cfg(any(test, feature = "real-window"))]
pub(crate) fn requested_memory_peak(
    frames: &[crate::AuiFrameWorkCounters],
    requested: u64,
) -> Option<u64> {
    if frames.len() as u64 != requested {
        return None;
    }
    let mut peak = None;
    for (i, frame) in frames.iter().enumerate() {
        if frame.frame_index != i as u64 + 1 {
            return None;
        }
        let private = frame.process_memory?.private_commit_bytes;
        peak = Some(peak.unwrap_or(0).max(private));
    }
    peak
}

#[cfg(any(test, feature = "real-window"))]
pub(crate) fn memory_percentiles(frames: &[crate::AuiFrameWorkCounters]) -> Option<[u64; 2]> {
    let mut values = frames
        .iter()
        .map(|f| f.process_memory.map(|m| m.private_commit_bytes))
        .collect::<Option<Vec<_>>>()?;
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some([
        values[(values.len() * 95).div_ceil(100) - 1],
        values[(values.len() * 99).div_ceil(100) - 1],
    ])
}

impl MeasurementPolicy {
    pub fn validate(self) -> Result<(), &'static str> {
        if self.allocation_summary && !self.enabled() {
            return Err("measurement_allocation_summary_requires_measurement");
        }
        if self.allocation_end_frame.is_some() && self.allocation_frame.is_none() {
            return Err("measurement_allocation_end_requires_start");
        }
        if self.allocation_frame.is_some_and(|frame| {
            let end = self.allocation_end_frame.or_else(|| frame.checked_add(1));
            frame == 0
                || end.is_none_or(|end| {
                    end <= frame
                        || end - frame > MAX_ALLOCATION_CAPTURE_FRAMES
                        || !self.trace_frame(frame)
                        || !self.trace_frame(end - 1)
                })
        }) {
            return Err("measurement_allocation_frame_requires_filtered_trace");
        }
        if self.native_events && self.level != MeasurementLevel::Trace {
            return Err("measurement_native_events_requires_trace");
        }
        if self.level != MeasurementLevel::Trace
            && (self.first_frame.is_some() || self.end_frame.is_some())
        {
            return Err("measurement_filter_requires_trace");
        }
        if self.first_frame.unwrap_or(0) >= self.end_frame.unwrap_or(u64::MAX) {
            return Err("measurement_invalid_frame_interval");
        }
        Ok(())
    }

    pub fn enabled(self) -> bool {
        self.level != MeasurementLevel::Off
    }

    pub fn allocation_frame(self, frame_index: u64) -> bool {
        self.allocation_frames().contains(&frame_index) && self.trace_frame(frame_index)
    }

    pub fn allocation_frames(self) -> std::ops::Range<u64> {
        let start = self.allocation_frame.unwrap_or(0);
        let end = self
            .allocation_end_frame
            .unwrap_or_else(|| start.saturating_add(1));
        if self.allocation_frame.is_none() {
            0..0
        } else {
            start..end
        }
    }

    pub fn trace_frame(self, frame_index: u64) -> bool {
        self.level == MeasurementLevel::Trace
            && frame_index >= self.first_frame.unwrap_or(0)
            && frame_index < self.end_frame.unwrap_or(u64::MAX)
    }

    pub(crate) fn filter_cpu(self, summary: &mut WindowedPlayerFramePerformanceSummary) {
        summary
            .raw_cpu_frames
            .retain(|f| self.trace_frame(f.frame_index));
    }

    #[cfg(feature = "real-window")]
    pub(crate) fn filter_gpu(self, report: &mut WindowedPlayerGpuPerformance) {
        report.samples.retain(|f| self.trace_frame(f.frame_index));
        report
            .submissions
            .retain(|s| self.trace_frame(s.token.frame_index));
    }
}

/// A constant-size wall-clock probe shared by Summary and Trace.
#[cfg(any(test, feature = "real-window"))]
#[derive(Default)]
pub(crate) struct SamplingClock {
    start: Option<std::time::Instant>,
    elapsed_ns: u64,
    first_frame: u64,
    last_frame: u64,
}
#[cfg(any(test, feature = "real-window"))]
impl SamplingClock {
    pub(crate) fn begin(&mut self, frame: u64, warmup: u64, count: u64, now: std::time::Instant) {
        if count > 0 && frame == warmup.saturating_add(1) && self.start.is_none() {
            self.start = Some(now);
            self.first_frame = frame;
        }
    }
    pub(crate) fn end(&mut self, frame: u64, warmup: u64, count: u64, now: std::time::Instant) {
        if let Some(start) = self.start {
            if frame > warmup && frame <= warmup.saturating_add(count) {
                self.last_frame = frame;
                self.elapsed_ns = now
                    .saturating_duration_since(start)
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
            }
        }
    }
    pub(crate) fn offset_ns(&self, now: std::time::Instant) -> Option<u64> {
        self.start.map(|start| {
            now.saturating_duration_since(start)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64
        })
    }
    pub(crate) fn report(
        &self,
        warmup: u64,
        count: u64,
    ) -> Option<engine_runtime::windowed_player::WindowedPlayerSamplingWindow> {
        (self.first_frame > 0 && self.last_frame >= self.first_frame).then_some(
            engine_runtime::windowed_player::WindowedPlayerSamplingWindow {
                wall_ns: self.elapsed_ns,
                first_frame: self.first_frame,
                last_frame: self.last_frame,
                first_tick: None,
                last_tick: None,
                complete: self.first_frame == warmup.saturating_add(1)
                    && self.last_frame == warmup.saturating_add(count),
                streaming_resources_start: None,
                streaming_resources_end: None,
                streaming_resources_unchanged: None,
                peak_private_commit_bytes: None,
                requested_peak_private_commit_bytes: None,
                minimum_system_available_bytes: None,
                tail1200_private_bytes_per_frame: None,
                private_commit_percentile_bytes: None,
                peak_working_set_bytes: None,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_memory_summary_is_opt_in_and_rejects_off() {
        let old: MeasurementPolicy = serde_json::from_str(r#"{"level":"summary"}"#).unwrap();
        assert!(!old.allocation_summary);
        let mut p = MeasurementPolicy {
            allocation_summary: true,
            ..Default::default()
        };
        assert!(p.validate().is_err());
        p.level = MeasurementLevel::Summary;
        assert!(p.validate().is_ok());
        assert!(p.allocation_frames().is_empty());
    }

    #[test]
    fn allocation_census_is_explicit_single_frame_and_trace_only() {
        let mut p = MeasurementPolicy {
            level: MeasurementLevel::Trace,
            first_frame: Some(7),
            end_frame: Some(9),
            allocation_frame: Some(8),
            ..Default::default()
        };
        assert!(p.validate().is_ok());
        assert!(!p.allocation_frame(7));
        assert!(p.allocation_frame(8));
        p.allocation_frame = Some(9);
        assert!(p.validate().is_err());
        p.allocation_frame = Some(0);
        assert!(p.validate().is_err());
        for level in [MeasurementLevel::Off, MeasurementLevel::Summary] {
            let p = MeasurementPolicy {
                level,
                allocation_frame: Some(1),
                ..Default::default()
            };
            assert!(p.validate().is_err());
            assert!(!p.allocation_frame(1));
        }
        assert!(!MeasurementPolicy {
            level: MeasurementLevel::Trace,
            ..Default::default()
        }
        .allocation_frame(1));
    }

    #[test]
    fn allocation_interval_is_bounded_consecutive_and_backward_compatible() {
        let old: MeasurementPolicy =
            serde_json::from_str(r#"{"level":"trace","allocationFrame":7}"#).unwrap();
        assert!(old.validate().is_ok());
        assert_eq!(old.allocation_frames(), 7..8);
        let mut p = MeasurementPolicy {
            allocation_end_frame: Some(71),
            ..old
        };
        assert!(p.validate().is_ok());
        assert_eq!(p.allocation_frames().count(), 64);
        assert!(p.allocation_frame(7) && p.allocation_frame(70));
        assert!(!p.allocation_frame(6) && !p.allocation_frame(71));
        for end in [7, 6, 72] {
            p.allocation_end_frame = Some(end);
            assert!(p.validate().is_err());
        }
        p.allocation_end_frame = Some(71);
        p.end_frame = Some(70);
        assert!(p.validate().is_err());
        p.end_frame = None;
        p.allocation_frame = None;
        assert!(p.validate().is_err());
        p.allocation_frame = Some(u64::MAX);
        p.allocation_end_frame = None;
        assert!(p.validate().is_err());
        for level in [MeasurementLevel::Off, MeasurementLevel::Summary] {
            let p = MeasurementPolicy {
                level,
                allocation_end_frame: Some(9),
                ..old
            };
            assert!(p.validate().is_err());
            assert!(!p.allocation_frame(7));
        }
    }

    #[test]
    fn sampling_clock_excludes_warmup_and_shutdown_and_retains_partial_interval() {
        use std::time::{Duration, Instant};
        let t = Instant::now();
        let mut clock = SamplingClock::default();
        clock.begin(1, 2, 3, t);
        clock.end(1, 2, 3, t + Duration::from_secs(100));
        assert!(clock.report(2, 3).is_none());
        clock.begin(3, 2, 3, t + Duration::from_secs(200));
        clock.end(3, 2, 3, t + Duration::from_secs(201));
        let partial = clock.report(2, 3).unwrap();
        assert_eq!(partial.wall_ns, 1_000_000_000);
        assert!(!partial.complete);
        clock.begin(4, 2, 3, t + Duration::from_secs(210));
        clock.end(5, 2, 3, t + Duration::from_secs(260));
        clock.end(6, 2, 3, t + Duration::from_secs(900));
        let result = clock.report(2, 3).unwrap();
        assert_eq!(result.wall_ns, 60_000_000_000);
        assert_eq!(
            clock.offset_ns(t + Duration::from_secs(260)),
            Some(60_000_000_000)
        );
        assert_eq!((result.first_frame, result.last_frame), (3, 5));
        assert!(result.complete);
    }
    #[test]
    fn native_capture_requires_independent_explicit_trace_opt_in() {
        for level in [MeasurementLevel::Off, MeasurementLevel::Summary] {
            assert_eq!(
                MeasurementPolicy {
                    level,
                    native_events: true,
                    ..Default::default()
                }
                .validate(),
                Err("measurement_native_events_requires_trace")
            );
        }
        let policy: MeasurementPolicy = serde_json::from_str(r#"{"level":"trace"}"#).unwrap();
        assert!(!policy.native_events);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn trace_filter_is_half_open_and_never_relabels_engine_frames() {
        let policy = MeasurementPolicy {
            level: MeasurementLevel::Trace,
            first_frame: Some(7),
            end_frame: Some(9),
            ..Default::default()
        };
        assert!(policy.validate().is_ok());
        assert!(!policy.trace_frame(6));
        assert!(policy.trace_frame(7));
        assert!(policy.trace_frame(8));
        assert!(!policy.trace_frame(9));
    }

    #[test]
    fn disabled_and_summary_policies_cannot_silently_accept_trace_filters() {
        for level in [MeasurementLevel::Off, MeasurementLevel::Summary] {
            let policy = MeasurementPolicy {
                level,
                first_frame: Some(1),
                end_frame: None,
                ..Default::default()
            };
            assert_eq!(policy.validate(), Err("measurement_filter_requires_trace"));
            assert!(!policy.trace_frame(1));
        }
        let invalid = MeasurementPolicy {
            level: MeasurementLevel::Trace,
            first_frame: Some(9),
            end_frame: Some(9),
            ..Default::default()
        };
        assert_eq!(
            invalid.validate(),
            Err("measurement_invalid_frame_interval")
        );
        assert!(!MeasurementPolicy::default().enabled());
        assert_eq!(
            serde_json::to_value(MeasurementPolicy::default()).unwrap()["level"],
            "off"
        );
    }
}

#[cfg(test)]
mod probe_wire_tests {
    use super::*;
    #[test]
    fn foundation_cpu_probe_false_preserves_historical_policy_wire() {
        let off = serde_json::to_value(MeasurementPolicy::default()).unwrap();
        assert!(
            off.get("cpuProbe").is_none(),
            "old sealed Players deny unknown fields"
        );
        let probe = serde_json::to_value(MeasurementPolicy {
            cpu_probe: true,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(probe["cpuProbe"], true);
        assert!(
            !serde_json::from_value::<MeasurementPolicy>(off)
                .unwrap()
                .cpu_probe
        );
    }
}
