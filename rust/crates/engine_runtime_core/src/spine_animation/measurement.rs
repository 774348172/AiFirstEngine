//! Constant-size opt-in timings in the existing frame measurement path.
use serde::{Deserialize, Serialize};
use std::{cell::Cell, time::Instant};

/// Exclusive CPU intervals. Projection includes neutral vertex/index encoding;
/// native GPU execution is a separate timestamp interval, never added here.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpineCpuFrameSample {
    #[serde(default)]
    pub first_fixed_tick: Option<u64>,
    #[serde(default)]
    pub last_fixed_tick: Option<u64>,
    pub control_ns: u64,
    pub official_apply_world_ns: u64,
    pub vertices_clipping_ns: u64,
    pub projection_encode_ns: u64,
    pub evaluated_instances: u64,
    pub geometry_instances: u64,
    pub vertices: u64,
    pub indices: u64,
    pub active_layers: u64,
    pub max_mixing_entries_per_layer: usize,
}
impl SpineCpuFrameSample {
    pub fn total_ns(self) -> u64 {
        self.control_ns
            .saturating_add(self.official_apply_world_ns)
            .saturating_add(self.vertices_clipping_ns)
            .saturating_add(self.projection_encode_ns)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    Control,
    #[cfg(any(feature = "spine", test))]
    Official,
    #[cfg(any(feature = "spine", test))]
    Vertices,
    #[cfg(any(feature = "spine", test))]
    Projection,
}
pub(crate) struct Started {
    at: Instant,
    accounted: u64,
}
#[derive(Debug, Default)]
pub(crate) struct Measurement(Cell<Option<SpineCpuFrameSample>>);
impl Measurement {
    pub(crate) fn begin_frame(&self, enabled: bool) {
        self.0.set(enabled.then(SpineCpuFrameSample::default));
    }
    pub(crate) fn sample(&self) -> Option<SpineCpuFrameSample> {
        self.0.get()
    }
    pub(crate) fn start(&self) -> Option<Started> {
        self.0.get().map(|s| Started {
            at: Instant::now(),
            accounted: s.total_ns(),
        })
    }
    pub(crate) fn finish(&self, stage: Stage, started: Option<Started>) {
        if let Some(started) = started {
            self.finish_elapsed(
                stage,
                started.accounted,
                started.at.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            );
        }
    }
    fn finish_elapsed(&self, stage: Stage, accounted: u64, elapsed: u64) {
        self.update(|s| {
            // Inner preflight/official/vertex spans have already been charged.
            let exclusive = elapsed.saturating_sub(s.total_ns().saturating_sub(accounted));
            let out = match stage {
                Stage::Control => &mut s.control_ns,
                #[cfg(any(feature = "spine", test))]
                Stage::Official => &mut s.official_apply_world_ns,
                #[cfg(any(feature = "spine", test))]
                Stage::Vertices => &mut s.vertices_clipping_ns,
                #[cfg(any(feature = "spine", test))]
                Stage::Projection => &mut s.projection_encode_ns,
            };
            *out = out.saturating_add(exclusive);
        });
    }
    pub(crate) fn update(&self, f: impl FnOnce(&mut SpineCpuFrameSample)) {
        if let Some(mut sample) = self.0.get() {
            f(&mut sample);
            self.0.set(Some(sample));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spine374_f_exclusive_cpu_intervals_do_not_double_count_nested_work() {
        let m = Measurement::default();
        m.begin_frame(true);
        m.finish_elapsed(Stage::Control, 0, 20);
        m.finish_elapsed(Stage::Official, 20, 30);
        m.finish_elapsed(Stage::Control, 0, 100);
        m.finish_elapsed(Stage::Vertices, 100, 70);
        m.finish_elapsed(Stage::Projection, 100, 90);
        let sample = m.sample().unwrap();
        assert_eq!(
            (
                sample.control_ns,
                sample.official_apply_world_ns,
                sample.vertices_clipping_ns,
                sample.projection_encode_ns
            ),
            (70, 30, 70, 20)
        );
        assert_eq!(sample.total_ns(), 190);
    }
    #[test]
    fn spine374_f_measurement_off_has_no_clock_or_stale_sample() {
        let m = Measurement::default();
        assert!(m.start().is_none());
        m.begin_frame(true);
        m.finish_elapsed(Stage::Control, 0, 123);
        assert_eq!(m.sample().unwrap().control_ns, 123);
        m.begin_frame(true);
        assert_eq!(m.sample(), Some(SpineCpuFrameSample::default()));
        m.begin_frame(false);
        assert!(m.start().is_none());
        m.update(|_| panic!("Off evaluated measurement work"));
        assert!(m.sample().is_none());
    }
}
