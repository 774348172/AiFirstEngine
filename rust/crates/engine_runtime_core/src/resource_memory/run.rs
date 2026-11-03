//! Constant-history, explicit per-frame census reduction. Peaks are independent,
//! never a simultaneous sum. Missing frames/coverage remain ineligible.
use super::*;
use crate::render_thread::RenderFrameIdentity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPeak {
    pub bytes: u64,
    pub frame_index: u64,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMemoryRunSummary {
    pub first_identity: Option<std::sync::Arc<RenderFrameIdentity>>,
    pub device_epoch: u64,
    pub first_frame: u64,
    pub last_frame: u64,
    pub frame_count: u64,
    pub first_capture_ns: u64,
    pub last_capture_ns: u64,
    pub maximum_capture_duration_ns: u64,
    pub cpu_peak: Option<MemoryPeak>,
    pub gpu_peak: Option<MemoryPeak>,
    pub owner_peaks: BTreeMap<String, MemoryPeak>,
    pub role_peaks: BTreeMap<String, MemoryPeak>,
    pub capacity_peaks: BTreeMap<String, u64>,
    pub process_private_peak: Option<MemoryPeak>,
    pub system_available_minimum: Option<MemoryPeak>,
    pub gpu_available_minimum: Option<MemoryPeak>,
    /// Intersection: Complete only when every observed frame declared it Complete.
    pub coverage: BTreeMap<String, MemoryCoverage>,
    pub missing_inputs: BTreeSet<String>,
    pub observed_scopes: BTreeSet<String>,
    pub errors: BTreeSet<String>,
}
impl ResourceMemoryRunSummary {
    pub fn observe(&mut self, s: &ResourceMemorySample) {
        // Sticky errors: later recovery must not erase the first broken boundary.
        if let Err(e) = self.observe_checked(s) {
            self.errors.insert(e.code.into());
        }
    }
    fn observe_checked(&mut self, s: &ResourceMemorySample) -> Result<(), MemoryObservationError> {
        let id = s.frame_identity.as_ref().ok_or_else(|| {
            MemoryObservationError::new("measurement.memory_capture_identity", "missing Extract")
        })?;
        if s.frame_index == 0
            || s.device_epoch == 0
            || id.frame_index != s.frame_index
            || id.source.run_id.trim().is_empty()
            || id.source.run_epoch == 0
            || id.source.world_epoch == 0
            || s.capture_finished_ns < s.capture_started_ns
        {
            return Err(MemoryObservationError::new(
                "measurement.memory_capture_identity",
                "invalid capture",
            ));
        }
        if let Some(first) = &self.first_identity {
            let a = &first.source;
            let b = &id.source;
            if self.last_frame.checked_add(1) != Some(s.frame_index)
                || s.capture_started_ns < self.last_capture_ns
                || self.device_epoch != s.device_epoch
                || a.run_id != b.run_id
                || a.run_epoch != b.run_epoch
                || a.session_id != b.session_id
                || a.package_content_hash != b.package_content_hash
            {
                return Err(MemoryObservationError::new(
                    "measurement.memory_window_identity",
                    "missing/duplicate/cross-run capture",
                ));
            }
        }
        if s.gpu_available_bytes
            != s.gpu_memory_budget
                .as_ref()
                .and_then(GpuMemoryBudgetSample::available_bytes)
            || s.gpu_memory_budget
                .as_ref()
                .is_some_and(|b| b.available_bytes().is_none())
        {
            return Err(MemoryObservationError::new(
                "measurement.gpu_budget_identity",
                "driver provenance mismatch",
            ));
        }
        let totals = memory_totals(&s.allocations)?;
        if self.first_identity.is_none() {
            self.first_identity = Some(id.clone());
            self.device_epoch = s.device_epoch;
            self.first_frame = s.frame_index;
            self.first_capture_ns = s.capture_started_ns;
        }
        self.last_frame = s.frame_index;
        self.frame_count += 1;
        self.last_capture_ns = s.capture_finished_ns;
        self.maximum_capture_duration_ns = self
            .maximum_capture_duration_ns
            .max(s.capture_finished_ns - s.capture_started_ns);
        fn peak(p: &mut Option<MemoryPeak>, bytes: u64, frame: u64, minimum: bool) {
            if p.as_ref().is_none_or(|p| {
                if minimum {
                    bytes < p.bytes
                } else {
                    bytes > p.bytes
                }
            }) {
                *p = Some(MemoryPeak {
                    bytes,
                    frame_index: frame,
                });
            }
        }
        peak(
            &mut self.cpu_peak,
            totals.cpu_allocated,
            s.frame_index,
            false,
        );
        peak(
            &mut self.gpu_peak,
            totals.gpu_allocated,
            s.frame_index,
            false,
        );
        for (out, values) in [
            (&mut self.owner_peaks, totals.by_owner),
            (&mut self.role_peaks, totals.by_role),
        ] {
            for (k, bytes) in values {
                let p = out.entry(k).or_insert(MemoryPeak {
                    bytes,
                    frame_index: s.frame_index,
                });
                if bytes > p.bytes {
                    *p = MemoryPeak {
                        bytes,
                        frame_index: s.frame_index,
                    };
                }
            }
        }
        for (k, v) in &s.capacities {
            let p = self.capacity_peaks.entry(k.clone()).or_default();
            *p = (*p).max(*v);
        }
        for key in [
            "activeRequests",
            "readyQueue",
            "failureCache",
            "framesInFlight",
            "concurrentDecodes",
            "uploadBytesPerFrame",
        ] {
            if !s.capacities.contains_key(key) {
                self.missing_inputs.insert(key.into());
            }
        }
        for (key, value, out, min) in [
            (
                "process.private",
                s.process_private_bytes,
                &mut self.process_private_peak,
                false,
            ),
            (
                "system.available",
                s.system_available_bytes,
                &mut self.system_available_minimum,
                true,
            ),
            (
                "gpu.available",
                s.gpu_available_bytes,
                &mut self.gpu_available_minimum,
                true,
            ),
        ] {
            if let Some(bytes) = value {
                peak(out, bytes, s.frame_index, min);
            } else {
                self.missing_inputs.insert(key.into());
            }
        }
        for domain in ["cpu", "gpu"] {
            for key in MEMORY_OWNERS
                .iter()
                .map(|n| format!("{domain}.owner.{n}"))
                .chain(MEMORY_ROLES.iter().map(|n| format!("{domain}.role.{n}")))
            {
                let now = s
                    .coverage
                    .get(&key)
                    .copied()
                    .unwrap_or(MemoryCoverage::Unavailable);
                self.coverage
                    .entry(key)
                    .and_modify(|old| {
                        *old = match (*old, now) {
                            (MemoryCoverage::Unavailable, _) | (_, MemoryCoverage::Unavailable) => {
                                MemoryCoverage::Unavailable
                            }
                            (MemoryCoverage::Partial, _) | (_, MemoryCoverage::Partial) => {
                                MemoryCoverage::Partial
                            }
                            _ => MemoryCoverage::Complete,
                        };
                    })
                    .or_insert(now);
            }
        }
        self.observed_scopes
            .extend(s.observed_scopes.iter().cloned());
        Ok(())
    }
    pub fn covers(&self, first: u64, end: u64) -> bool {
        first > 0
            && end > first
            && self.errors.is_empty()
            && self.first_frame == first
            && self.last_frame.checked_add(1) == Some(end)
            && self.frame_count == end - first
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_thread::RenderFrameSourceIdentity;
    use std::sync::Arc;
    fn sample(frame: u64, bytes: u64) -> ResourceMemorySample {
        let mut s = ResourceMemorySample::new(frame, 1, frame * 10);
        s.capture_finished_ns = frame * 10 + 1;
        s.frame_identity = Some(Arc::new(RenderFrameIdentity {
            source: Arc::new(RenderFrameSourceIdentity {
                run_id: "run".into(),
                run_epoch: 1,
                session_id: "session".into(),
                package_content_hash: Some("hash".into()),
                world_epoch: 1,
            }),
            frame_index: frame,
            tick_pair: [frame, frame],
            presentation_alpha_bits: 0,
            views: Arc::new(vec![]),
        }));
        s.allocations.push(AllocationSample {
            allocation_id: "shared".into(),
            domain: MemoryDomain::Cpu,
            owner: "assets".into(),
            bytes,
            roles: BTreeSet::from(["decodedCache".into()]),
        });
        s
    }
    #[test]
    fn run_summary_keeps_aliases_early_peak_and_missing_inputs_without_history() {
        let mut r = ResourceMemoryRunSummary::default();
        let mut s = sample(1, 100);
        let mut alias = s.allocations[0].clone();
        alias.roles = BTreeSet::from(["retiring".into()]);
        s.allocations.push(alias);
        s.system_available_bytes = Some(5);
        r.observe(&s);
        s = sample(2, 40);
        s.system_available_bytes = Some(9);
        r.observe(&s);
        assert!(r.covers(1, 3));
        assert!(!r.covers(1, 4));
        assert_eq!(r.cpu_peak.as_ref().unwrap().bytes, 100);
        assert_eq!(r.role_peaks["cpu.retiring"].bytes, 100);
        assert_eq!(r.system_available_minimum.as_ref().unwrap().frame_index, 1);
        assert!(r.missing_inputs.contains("gpu.available"));
        assert_eq!(r.coverage["cpu.owner.world"], MemoryCoverage::Unavailable);
        assert_eq!(r.owner_peaks.len(), 1);
    }
    #[test]
    fn run_summary_rejects_gap_duplicate_cross_run_device_and_overlap_stickily() {
        for variant in 0..5 {
            let mut r = ResourceMemoryRunSummary::default();
            r.observe(&sample(1, 1));
            let mut next = sample(2, 1);
            match variant {
                0 => next = sample(3, 1),
                1 => next = sample(1, 1),
                2 => {
                    Arc::make_mut(
                        &mut Arc::make_mut(next.frame_identity.as_mut().unwrap()).source,
                    )
                    .run_id = "other".into()
                }
                3 => next.device_epoch = 2,
                _ => next.capture_started_ns = 0,
            }
            r.observe(&next);
            r.observe(&sample(2, 1));
            assert!(!r.covers(1, 3));
            assert!(!r.errors.is_empty());
        }
    }
    #[test]
    fn run_summary_intersects_coverage_and_does_not_sum_independent_role_peaks() {
        let mut r = ResourceMemoryRunSummary::default();
        let mut a = sample(1, 100);
        a.coverage
            .insert("cpu.role.decodedCache".into(), MemoryCoverage::Complete);
        r.observe(&a);
        let mut b = sample(2, 100);
        b.allocations[0].roles = BTreeSet::from(["retiring".into()]);
        b.coverage
            .insert("cpu.role.decodedCache".into(), MemoryCoverage::Partial);
        r.observe(&b);
        assert_eq!(r.cpu_peak.unwrap().bytes, 100);
        assert_eq!(r.role_peaks.values().map(|p| p.bytes).sum::<u64>(), 200);
        assert_eq!(r.coverage["cpu.role.decodedCache"], MemoryCoverage::Partial);
    }
}
