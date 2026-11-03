use super::{BenchmarkError, BenchmarkResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    pub count: usize,
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    pub exceeded: usize,
}
/// Nearest rank ceil(p*N), one-based. No interpolation or outlier deletion.
pub fn distribution(samples: &[f64], threshold: f64) -> BenchmarkResult<Distribution> {
    if samples.is_empty()
        || !threshold.is_finite()
        || threshold < 0.
        || samples.iter().any(|v| !v.is_finite() || *v < 0.)
    {
        return Err(BenchmarkError::new(
            "measurement.invalid_sample",
            "finite nonnegative samples and threshold required",
        ));
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = |p: usize| {
        sorted[(sorted.len() / 100 * p + (sorted.len() % 100 * p).div_ceil(100)).max(1) - 1]
    };
    let mut mean = 0.;
    for (i, v) in samples.iter().enumerate() {
        mean += (v - mean) / (i + 1) as f64;
    }
    Ok(Distribution {
        count: samples.len(),
        mean,
        p50: rank(50),
        p95: rank(95),
        p99: rank(99),
        max: *sorted.last().unwrap(),
        exceeded: samples.iter().filter(|v| **v > threshold).count(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetBoundary {
    pub warmup_frames: u64,
    pub camera_tick: u64,
    pub input_cursor: u64,
    pub event_cursor: u64,
    pub measurement_time_ns: u64,
    pub initial_simulation_restored: bool,
}
impl ResetBoundary {
    pub fn validate(&self, expected_warmup: u64) -> BenchmarkResult<()> {
        if self.warmup_frames != expected_warmup
            || self.camera_tick != 0
            || self.input_cursor != 0
            || self.event_cursor != 0
            || self.measurement_time_ns != 0
            || !self.initial_simulation_restored
        {
            return Err(BenchmarkError::new(
                "measurement.reset_incomplete",
                "camera/input/events/time/simulation must reset together",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualificationWindow {
    FixedCamera,
    Trajectory,
    Streaming,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WindowCoverage {
    pub wall_seconds: f64,
    pub simulated_seconds: f64,
    pub simulation_ticks: u64,
    pub presented_frames: u64,
    pub complete_trajectory_cycles: u64,
    pub distinct_cells: u64,
    pub revisited_cells: u64,
    pub uploaded_bytes: u64,
}
pub fn validate_window(
    kind: QualificationWindow,
    android: bool,
    coverage: &WindowCoverage,
) -> BenchmarkResult<()> {
    if !coverage.wall_seconds.is_finite()
        || !coverage.simulated_seconds.is_finite()
        || coverage.wall_seconds < 0.
        || coverage.simulated_seconds < 0.
    {
        return Err(BenchmarkError::new(
            "measurement.invalid_window",
            "invalid clock",
        ));
    }
    let (seconds, ticks, frames) = match kind {
        QualificationWindow::FixedCamera => (60., 3600, 3600),
        QualificationWindow::Trajectory => (120., 7200, if android { 3600 } else { 7200 }),
        QualificationWindow::Streaming => (600., 36000, if android { 18000 } else { 36000 }),
    };
    if coverage.wall_seconds < seconds
        || coverage.simulation_ticks < ticks
        || coverage.presented_frames < frames
        || (kind != QualificationWindow::FixedCamera && coverage.complete_trajectory_cycles == 0)
        || (kind == QualificationWindow::Trajectory
            && coverage.complete_trajectory_cycles
                < (coverage.simulated_seconds / 120.).floor() as u64)
    {
        return Err(BenchmarkError::new(
            "measurement.window_incomplete",
            "wall/tick/present/trajectory intersection not covered",
        ));
    }
    // An extended window must continue simulation in real time, not collect frames by slow motion.
    if (coverage.simulated_seconds - coverage.wall_seconds).abs() > 2. / 60.
        || (coverage.simulated_seconds * 60. - coverage.simulation_ticks as f64).abs() > 2.
    {
        return Err(BenchmarkError::new(
            "measurement.simulation_slowed",
            "simulation and wall clock disagree",
        ));
    }
    if kind == QualificationWindow::Streaming {
        let (cells, residency) = if android {
            (24, 512u64 * 1024 * 1024)
        } else {
            (40, 2u64 * 1024 * 1024 * 1024)
        };
        if coverage.distinct_cells < cells
            || coverage.revisited_cells < 4
            || coverage.uploaded_bytes <= residency * 2
        {
            return Err(BenchmarkError::new(
                "measurement.streaming_incomplete",
                "unique cells/revisits/upload coverage missing",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentAssociation {
    pub present_id: u64,
    pub packet_id: u64,
    pub submission_id: u64,
    pub snapshot_epoch: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentedEvent {
    pub present_id: u64,
    pub time_ns: u64,
    pub compositor_repeat: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentSamples {
    pub unique_application_frames: usize,
    pub duplicate_events: usize,
    pub intervals_ms: Vec<f64>,
    pub associations: Vec<PresentAssociation>,
    pub application_frame_times_ns: Vec<u64>,
}
/// Offline OS event join. API present requests/returns must never be supplied as presented events.
pub fn correlate_present(
    associations: &[PresentAssociation],
    events: &[PresentedEvent],
) -> BenchmarkResult<PresentSamples> {
    if associations.len() > 2_000_000 || events.len() > 4_000_000 {
        return Err(BenchmarkError::new(
            "measurement.sample_capacity",
            "offline association/event capacity exceeded",
        ));
    }
    let mut map = BTreeMap::new();
    for a in associations {
        if map.insert(a.present_id, a).is_some() {
            return Err(BenchmarkError::new(
                "measurement.ambiguous_present",
                "duplicate present token",
            ));
        }
    }
    let mut packets = BTreeSet::new();
    let mut last_event = None;
    let mut last_unique = None;
    let mut result = PresentSamples {
        unique_application_frames: 0,
        duplicate_events: 0,
        intervals_ms: Vec::new(),
        associations: Vec::new(),
        application_frame_times_ns: Vec::new(),
    };
    for event in events {
        if last_event.is_some_and(|time| time > event.time_ns) {
            return Err(BenchmarkError::new(
                "measurement.non_monotonic_present",
                "OS clock moved backwards",
            ));
        }
        last_event = Some(event.time_ns);
        let a = map.get(&event.present_id).ok_or_else(|| {
            BenchmarkError::new(
                "measurement.unmatched_present",
                format!("present {}", event.present_id),
            )
        })?;
        let packet_key = (a.snapshot_epoch, a.packet_id);
        if event.compositor_repeat && !packets.contains(&packet_key) {
            return Err(BenchmarkError::new(
                "measurement.unmatched_repeat",
                "repeat has no matched original frame",
            ));
        }
        if !packets.insert(packet_key) {
            result.duplicate_events += 1;
            continue;
        }
        if let Some(time) = last_unique {
            result
                .intervals_ms
                .push((event.time_ns - time) as f64 / 1e6);
        }
        last_unique = Some(event.time_ns);
        result.associations.push((*a).clone());
        result.application_frame_times_ns.push(event.time_ns);
        result.unique_application_frames += 1;
    }
    Ok(result)
}

pub fn present_minute_counts(
    samples: &PresentSamples,
    start_ns: u64,
    end_ns: u64,
) -> BenchmarkResult<Vec<u64>> {
    let span = end_ns.checked_sub(start_ns).ok_or_else(|| {
        BenchmarkError::new("measurement.invalid_window", "reversed event window")
    })?;
    if span > 86_400_000_000_000 {
        return Err(BenchmarkError::new(
            "measurement.window_capacity",
            "offline window exceeds 24 hours",
        ));
    }
    let mut counts = vec![0; (span / 60_000_000_000) as usize];
    for time in &samples.application_frame_times_ns {
        if *time < start_ns || *time > end_ns {
            return Err(BenchmarkError::new(
                "measurement.present_outside_window",
                "OS event is outside measurement window",
            ));
        }
        let index = ((*time - start_ns) / 60_000_000_000) as usize;
        if let Some(count) = counts.get_mut(index) {
            *count += 1;
        }
    }
    Ok(counts)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementRound {
    pub process_id: u32,
    pub identity: String,
    pub reset: ResetBoundary,
    pub coverage: WindowCoverage,
    pub cpu: Distribution,
    pub gpu: Option<Distribution>,
    pub present: Option<Distribution>,
    pub unmatched_events: usize,
    /// Counts for each complete wall-clock minute, from unique associated OS events.
    pub minute_presented_counts: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMeasurementDimension {
    Cpu,
    Gpu,
}

/// Application-frame counts are distinct from associated OS displayed-frame counts.
pub fn assess_execution_three_rounds(
    rounds: &[MeasurementRound],
    kind: QualificationWindow,
    android: bool,
    dimension: ExecutionMeasurementDimension,
    application_frames: &[u64],
) -> BenchmarkResult<()> {
    validate_repeat_identity(rounds)?;
    if application_frames.len() != rounds.len() {
        return Err(BenchmarkError::new(
            "measurement.window_incomplete",
            "one observed application-frame count per process required",
        ));
    }
    let (p95, p99) = if android {
        (26.7, 33.33)
    } else {
        (13.3, 16.67)
    };
    for (index, (round, frames)) in rounds.iter().zip(application_frames).enumerate() {
        round.reset.validate(600)?;
        // Reuse the common time/tick/trajectory rules with this dimension's
        // frame source. Never mutate the caller's OS coverage or infer scan-out.
        let mut application_coverage = round.coverage.clone();
        application_coverage.presented_frames = *frames;
        validate_window(kind, android, &application_coverage)?;
        let samples = match dimension {
            ExecutionMeasurementDimension::Cpu => &round.cpu,
            ExecutionMeasurementDimension::Gpu => round.gpu.as_ref().ok_or_else(|| {
                BenchmarkError::new(
                    "measurement.instrument_unavailable",
                    format!("round {index}: GPU missing"),
                )
            })?,
        };
        validate_distribution(samples)?;
        if (samples.count as u64) < *frames {
            return Err(BenchmarkError::new(
                "measurement.association_incomplete",
                format!("round {index}: execution samples missing"),
            ));
        }
        if samples.p95 > p95 || samples.p99 > p99 {
            return Err(BenchmarkError::new(
                "measurement.round_budget",
                format!("round {index} failed {dimension:?}; pooling forbidden"),
            ));
        }
    }
    Ok(())
}

fn validate_repeat_identity(rounds: &[MeasurementRound]) -> BenchmarkResult<()> {
    if rounds.len() != 3
        || rounds
            .iter()
            .map(|r| r.process_id)
            .collect::<BTreeSet<_>>()
            .len()
            != 3
        || rounds
            .iter()
            .any(|r| r.identity.is_empty() || r.identity != rounds[0].identity)
    {
        return Err(BenchmarkError::new(
            "measurement.repeat_identity",
            "three independent processes with identical input/config required",
        ));
    }
    Ok(())
}

fn validate_distribution(samples: &Distribution) -> BenchmarkResult<()> {
    if samples.count == 0
        || samples.exceeded > samples.count
        || [
            samples.mean,
            samples.p50,
            samples.p95,
            samples.p99,
            samples.max,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.)
        || samples.p50 > samples.p95
        || samples.p95 > samples.p99
        || samples.p99 > samples.max
    {
        return Err(BenchmarkError::new(
            "measurement.invalid_distribution",
            "invalid raw-sample summary",
        ));
    }
    Ok(())
}
/// Each of three independent processes must pass. Missing instruments are not success.
pub fn assess_three_rounds(
    rounds: &[MeasurementRound],
    kind: QualificationWindow,
    android: bool,
) -> BenchmarkResult<()> {
    validate_repeat_identity(rounds)?;
    let (p95, p99, present95, present99) = if android {
        (26.7, 33.33, 34.5, 66.7)
    } else {
        (13.3, 16.67, 17.5, 33.4)
    };
    for (index, round) in rounds.iter().enumerate() {
        round.reset.validate(600)?;
        validate_window(kind, android, &round.coverage)?;
        let (Some(gpu), Some(present)) = (&round.gpu, &round.present) else {
            return Err(BenchmarkError::new(
                "measurement.instrument_unavailable",
                format!("round {index}: GPU/actual present missing"),
            ));
        };
        let minimum_samples = round.coverage.presented_frames as usize;
        for samples in [&round.cpu, gpu, present] {
            validate_distribution(samples)?;
        }
        if round.cpu.count < minimum_samples
            || gpu.count < minimum_samples
            || present.count + 1 < minimum_samples
            || round.unmatched_events != 0
        {
            return Err(BenchmarkError::new(
                "measurement.association_incomplete",
                format!("round {index}: samples/association missing"),
            ));
        }
        let effective_seconds = present.mean * present.count as f64 / 1000.;
        let required_rate = if android { 29.5 } else { 59. };
        let complete_minutes = (round.coverage.wall_seconds / 60.).floor() as usize;
        if round.minute_presented_counts.len() != complete_minutes
            || round
                .minute_presented_counts
                .iter()
                .any(|count| *count as f64 / 60. < required_rate)
            || round
                .minute_presented_counts
                .iter()
                .try_fold(0u64, |sum, v| sum.checked_add(*v))
                .is_none_or(|total| total > round.coverage.presented_frames)
        {
            return Err(BenchmarkError::new(
                "measurement.minute_rate",
                "each complete wall minute must meet the presented-frame rate",
            ));
        }
        if round.cpu.p95 > p95
            || round.cpu.p99 > p99
            || gpu.p95 > p95
            || gpu.p99 > p99
            || present.p95 > present95
            || present.p99 > present99
            || present.exceeded as f64 / present.count as f64 > 0.001
            || effective_seconds <= 0.
            || present.count as f64 / effective_seconds < required_rate
        {
            return Err(BenchmarkError::new(
                "measurement.round_budget",
                format!("round {index} failed; pooling forbidden"),
            ));
        }
    }
    Ok(())
}

pub fn summary_overhead(
    off_p95: f64,
    summary_p95: f64,
    same_identity: bool,
) -> BenchmarkResult<f64> {
    if !same_identity
        || !off_p95.is_finite()
        || !summary_p95.is_finite()
        || off_p95 < 0.
        || summary_p95 < 0.
    {
        return Err(BenchmarkError::new(
            "measurement.overhead_identity",
            "same identity and finite timings required",
        ));
    }
    let delta = summary_p95 - off_p95;
    if summary_p95 > off_p95 + (off_p95 * 0.03).max(0.2) {
        return Err(BenchmarkError::new(
            "measurement.summary_overhead",
            "Summary exceeds 3% or 0.2ms allowance",
        ));
    }
    Ok(delta)
}
