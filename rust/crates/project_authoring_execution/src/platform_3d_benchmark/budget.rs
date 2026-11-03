use super::{BenchmarkError, BenchmarkResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MIB: u64 = 1024 * 1024;
pub const GIB: u64 = 1024 * MIB;

pub use engine_runtime::resource_memory::MemoryTotals;
pub use engine_runtime::resource_memory::{AllocationSample, MemoryDomain};
pub fn memory_totals(samples: &[AllocationSample]) -> BenchmarkResult<MemoryTotals> {
    engine_runtime::resource_memory::memory_totals(samples)
        .map_err(|e| BenchmarkError::new(e.code, e.message))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryBudget {
    pub cpu_total: u64,
    pub process_private_peak: u64,
    pub gpu_total: u64,
    pub minimum_system_available: u64,
    pub minimum_gpu_available: u64,
    pub role_limits: BTreeMap<String, u64>,
    pub capacity_limits: BTreeMap<String, u64>,
}
impl MemoryBudget {
    /// Frozen candidate for the inventoried 16GiB/RTX3070-8GiB PC, not measured capacity.
    pub fn pc_main() -> Self {
        Self {
            cpu_total: 4 * GIB,
            process_private_peak: 6 * GIB,
            gpu_total: 4 * GIB,
            minimum_system_available: 4 * GIB + GIB / 2,
            minimum_gpu_available: 2 * GIB,
            role_limits: BTreeMap::from([
                ("cpu.decodedCache".into(), 2 * GIB),
                ("gpu.resident".into(), 4 * GIB),
                ("cpu.staging".into(), 128 * MIB),
                ("gpu.staging".into(), 128 * MIB),
                ("cpu.retiring".into(), 256 * MIB),
                ("gpu.retiring".into(), 256 * MIB),
                ("cpu.decodeWork".into(), 256 * MIB),
            ]),
            capacity_limits: BTreeMap::from([
                ("activeRequests".into(), 256),
                ("readyQueue".into(), 128),
                ("failureCache".into(), 128),
                ("framesInFlight".into(), 2),
                ("concurrentDecodes".into(), 4),
                ("uploadBytesPerFrame".into(), 16 * MIB),
            ]),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMemorySample {
    pub process_private: Option<u64>,
    pub system_available: Option<u64>,
    pub gpu_available: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetVerdict {
    pub status: String,
    pub breaches: Vec<String>,
    pub unavailable: Vec<String>,
}
pub fn assess_memory(
    budget: &MemoryBudget,
    totals: &MemoryTotals,
    external: &ExternalMemorySample,
    capacities: &BTreeMap<String, u64>,
) -> BudgetVerdict {
    let mut breaches = Vec::new();
    let mut unavailable = Vec::new();
    for (key, value, limit) in [
        ("cpu.total", totals.cpu_allocated, budget.cpu_total),
        ("gpu.total", totals.gpu_allocated, budget.gpu_total),
    ] {
        if value > limit {
            breaches.push(key.into());
        }
    }
    for (key, limit) in &budget.role_limits {
        if totals.by_role.get(key).copied().unwrap_or(0) > *limit {
            breaches.push(key.clone());
        }
    }
    for (key, limit) in &budget.capacity_limits {
        match capacities.get(key) {
            Some(value) if value > limit => breaches.push(key.clone()),
            None => unavailable.push(key.clone()),
            _ => {}
        }
    }
    for (key, value, limit, minimum) in [
        (
            "process.private",
            external.process_private,
            budget.process_private_peak,
            false,
        ),
        (
            "system.available",
            external.system_available,
            budget.minimum_system_available,
            true,
        ),
        (
            "gpu.available",
            external.gpu_available,
            budget.minimum_gpu_available,
            true,
        ),
    ] {
        match value {
            Some(v) if (!minimum && v > limit) || (minimum && v < limit) => {
                breaches.push(key.into())
            }
            None => unavailable.push(key.into()),
            _ => {}
        }
    }
    BudgetVerdict {
        status: if !breaches.is_empty() {
            "failed"
        } else if !unavailable.is_empty() {
            "unqualified"
        } else {
            "passed"
        }
        .into(),
        breaches,
        unavailable,
    }
}

/// Production observations feed the same allocation sums and budget evaluator.
/// Capture identity and completeness are additional eligibility checks, not a
/// second arithmetic budget implementation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedMemoryAssessment {
    pub frame_index: u64,
    pub frame_identity: std::sync::Arc<engine_runtime::render_thread::RenderFrameIdentity>,
    pub device_epoch: u64,
    pub capture_duration_ns: u64,
    pub tracked_totals: MemoryTotals,
    pub verdict: BudgetVerdict,
}
pub fn assess_observed_memory(
    budget: &MemoryBudget,
    sample: &engine_runtime::resource_memory::ResourceMemorySample,
    expected_frame: &engine_runtime::render_thread::RenderFrameIdentity,
    expected_device: u64,
) -> BenchmarkResult<ObservedMemoryAssessment> {
    use engine_runtime::resource_memory::{MemoryCoverage, MEMORY_OWNERS};
    if sample.frame_index == 0
        || sample.device_epoch == 0
        || sample.frame_index != expected_frame.frame_index
        || sample.frame_identity.as_deref() != Some(expected_frame)
        || expected_frame.source.run_id.trim().is_empty()
        || expected_frame.source.run_epoch == 0
        || expected_frame.source.world_epoch == 0
        || sample.device_epoch != expected_device
        || sample.capture_finished_ns < sample.capture_started_ns
    {
        return Err(BenchmarkError::new(
            "measurement.memory_capture_identity",
            "frame/device/ordered capture boundary required",
        ));
    }
    if sample.gpu_available_bytes
        != sample
            .gpu_memory_budget
            .as_ref()
            .and_then(|v| v.available_bytes())
        || sample
            .gpu_memory_budget
            .as_ref()
            .is_some_and(|v| v.available_bytes().is_none())
    {
        return Err(BenchmarkError::new(
            "measurement.gpu_budget_identity",
            "driver budget provenance/headroom mismatch",
        ));
    }
    let totals = memory_totals(&sample.allocations)?;
    let mut verdict = assess_memory(
        budget,
        &totals,
        &ExternalMemorySample {
            process_private: sample.process_private_bytes,
            system_available: sample.system_available_bytes,
            gpu_available: sample.gpu_available_bytes,
        },
        &sample.capacities,
    );
    for domain in ["cpu", "gpu"] {
        let mut complete = true;
        for owner in MEMORY_OWNERS {
            let key = format!("{domain}.owner.{owner}");
            if sample.coverage.get(&key) != Some(&MemoryCoverage::Complete) {
                verdict.unavailable.push(key);
                complete = false;
            }
        }
        if !complete {
            verdict.unavailable.push(format!("{domain}.total"));
        }
    }
    for key in budget.role_limits.keys() {
        let (domain, role) = key
            .split_once('.')
            .ok_or_else(|| BenchmarkError::new("measurement.memory_role", key))?;
        if sample.coverage.get(&format!("{domain}.role.{role}")) != Some(&MemoryCoverage::Complete)
        {
            verdict.unavailable.push(key.clone());
        }
    }
    verdict.unavailable.sort();
    verdict.unavailable.dedup();
    if verdict.breaches.is_empty() && !verdict.unavailable.is_empty() {
        verdict.status = "unqualified".into();
    }
    Ok(ObservedMemoryAssessment {
        frame_index: sample.frame_index,
        frame_identity: std::sync::Arc::new(expected_frame.clone()),
        device_epoch: sample.device_epoch,
        capture_duration_ns: sample
            .capture_finished_ns
            .saturating_sub(sample.capture_started_ns),
        tracked_totals: totals,
        verdict,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPeak {
    pub bytes: u64,
    pub frame_index: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedMemoryWindowAssessment {
    pub first_frame: u64,
    pub end_frame: u64,
    pub frame_count: usize,
    /// Only the explicitly declared consecutive capture interval is covered.
    pub cpu_total_peak: MemoryPeak,
    pub gpu_total_peak: MemoryPeak,
    pub by_owner_peaks: BTreeMap<String, MemoryPeak>,
    pub by_role_peaks: BTreeMap<String, MemoryPeak>,
    pub frames: Vec<ObservedMemoryAssessment>,
    pub verdict: BudgetVerdict,
}

/// Every expected frame must be present exactly once; independent category peaks
/// are never added together to manufacture an allocation total.
pub fn assess_observed_memory_window(
    budget: &MemoryBudget,
    samples: &[engine_runtime::resource_memory::ResourceMemorySample],
    expected_frames: &[engine_runtime::render_thread::RenderFrameIdentity],
    expected_device: u64,
    first_frame: u64,
    end_frame: u64,
) -> BenchmarkResult<ObservedMemoryWindowAssessment> {
    if first_frame == 0
        || end_frame <= first_frame
        || end_frame - first_frame != samples.len() as u64
        || samples.len() != expected_frames.len()
    {
        return Err(BenchmarkError::new(
            "measurement.memory_window_incomplete",
            "one ordered census per expected frame required",
        ));
    }
    let mut frames = Vec::with_capacity(samples.len());
    let mut cpu = MemoryPeak {
        bytes: 0,
        frame_index: first_frame,
    };
    let mut gpu = cpu.clone();
    let mut owners = BTreeMap::new();
    let mut roles = BTreeMap::new();
    let mut breaches = std::collections::BTreeSet::new();
    let mut unavailable = std::collections::BTreeSet::new();
    let mut prior_end = 0;
    let source = &expected_frames[0].source;
    let update = |peak: &mut MemoryPeak, bytes, frame_index| {
        if bytes > peak.bytes {
            *peak = MemoryPeak { bytes, frame_index };
        }
    };
    for (i, (sample, expected)) in samples.iter().zip(expected_frames).enumerate() {
        if sample.frame_index != first_frame + i as u64
            || sample.capture_started_ns < prior_end
            || expected.source.run_id != source.run_id
            || expected.source.run_epoch != source.run_epoch
            || expected.source.session_id != source.session_id
            || expected.source.package_content_hash != source.package_content_hash
        {
            return Err(BenchmarkError::new(
                "measurement.memory_window_identity",
                "ordered same-run/device capture interval required",
            ));
        }
        let assessment = assess_observed_memory(budget, sample, expected, expected_device)?;
        prior_end = sample.capture_finished_ns;
        let totals = &assessment.tracked_totals;
        update(&mut cpu, totals.cpu_allocated, sample.frame_index);
        update(&mut gpu, totals.gpu_allocated, sample.frame_index);
        for (target, values) in [
            (&mut owners, &totals.by_owner),
            (&mut roles, &totals.by_role),
        ] {
            for (key, bytes) in values {
                let peak = target.entry(key.clone()).or_insert(MemoryPeak {
                    bytes: 0,
                    frame_index: sample.frame_index,
                });
                update(peak, *bytes, sample.frame_index);
            }
        }
        breaches.extend(assessment.verdict.breaches.iter().cloned());
        unavailable.extend(assessment.verdict.unavailable.iter().cloned());
        frames.push(assessment);
    }
    let status = if !breaches.is_empty() {
        "failed"
    } else if !unavailable.is_empty() {
        "unqualified"
    } else {
        "passed"
    };
    Ok(ObservedMemoryWindowAssessment {
        first_frame,
        end_frame,
        frame_count: frames.len(),
        cpu_total_peak: cpu,
        gpu_total_peak: gpu,
        by_owner_peaks: owners,
        by_role_peaks: roles,
        frames,
        verdict: BudgetVerdict {
            status: status.into(),
            breaches: breaches.into_iter().collect(),
            unavailable: unavailable.into_iter().collect(),
        },
    })
}

#[cfg(test)]
mod observed_tests {
    use super::*;
    use engine_runtime::resource_memory::{MemoryCoverage, ResourceMemorySample};
    use std::collections::BTreeSet;
    fn frame(index: u64) -> engine_runtime::render_thread::RenderFrameIdentity {
        use engine_runtime::render_thread::{RenderFrameIdentity, RenderFrameSourceIdentity};
        use std::sync::Arc;
        RenderFrameIdentity {
            source: Arc::new(RenderFrameSourceIdentity {
                run_id: "memory-test".into(),
                run_epoch: 1,
                session_id: "memory-test".into(),
                package_content_hash: Some(format!("sha256:{}", "a".repeat(64))),
                world_epoch: 1,
            }),
            frame_index: index,
            tick_pair: [6, 7],
            presentation_alpha_bits: 0,
            views: Arc::new(Vec::new()),
        }
    }
    fn sample() -> ResourceMemorySample {
        let mut sample = ResourceMemorySample::new(7, 3, 100);
        sample.frame_identity = Some(std::sync::Arc::new(frame(7)));
        sample.capture_finished_ns = 110;
        sample.process_private_bytes = Some(GIB);
        sample.system_available_bytes = Some(8 * GIB);
        sample.gpu_available_bytes = Some(3 * GIB);
        sample.gpu_memory_budget = Some(engine_runtime::resource_memory::GpuMemoryBudgetSample {
            source: "VK_EXT_memory_budget".into(),
            adapter_uuid: "ab".repeat(16),
            local_heaps: vec![engine_runtime::resource_memory::GpuLocalHeapBudget {
                heap_index: 0,
                heap_size: 4 * GIB,
                budget: 4 * GIB,
                usage: GIB,
            }],
        });
        sample.capacities = MemoryBudget::pc_main()
            .capacity_limits
            .keys()
            .map(|k| (k.clone(), 0))
            .collect();
        sample
    }
    #[test]
    fn observed_run_keeps_early_budget_failure_and_rejects_incomplete_window() {
        use engine_runtime::resource_memory::ResourceMemoryRunSummary;
        let mut run = ResourceMemoryRunSummary::default();
        let mut first = sample();
        first.system_available_bytes = Some(5 * GIB);
        run.observe(&first);
        let mut next = first.clone();
        next.frame_index = 8;
        next.frame_identity = Some(std::sync::Arc::new(frame(8)));
        next.capture_started_ns = 200;
        next.capture_finished_ns = 201;
        next.system_available_bytes = Some(9 * GIB);
        run.observe(&next);
        let result = assess_observed_memory_run(&MemoryBudget::pc_main(), &run, 7, 9).unwrap();
        assert_eq!(result.status, "failed");
        assert!(result.breaches.contains(&"system.available".into()));
        assert!(result.unavailable.contains(&"cpu.total".into()));
        assert!(assess_observed_memory_run(&MemoryBudget::pc_main(), &run, 7, 10).is_err());
    }
    #[test]
    fn observed_memory_complete_inventory_passes_but_one_unknown_role_does_not() {
        let mut s = sample();
        for domain in ["cpu", "gpu"] {
            for owner in engine_runtime::resource_memory::MEMORY_OWNERS {
                s.coverage
                    .insert(format!("{domain}.owner.{owner}"), MemoryCoverage::Complete);
            }
        }
        for key in MemoryBudget::pc_main().role_limits.keys() {
            let (domain, role) = key.split_once('.').unwrap();
            s.coverage
                .insert(format!("{domain}.role.{role}"), MemoryCoverage::Complete);
        }
        assert_eq!(
            assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(7), 3)
                .unwrap()
                .verdict
                .status,
            "passed"
        );
        s.coverage.remove("gpu.role.retiring");
        assert!(
            assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(7), 3)
                .unwrap()
                .verdict
                .unavailable
                .contains(&"gpu.retiring".into())
        );
    }
    #[test]
    fn observed_memory_missing_or_partial_coverage_never_passes() {
        let mut s = sample();
        let result = assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(7), 3).unwrap();
        assert_eq!(result.verdict.status, "unqualified");
        assert!(result.verdict.unavailable.contains(&"cpu.total".into()));
        s.coverage
            .insert("cpu.owner.world".into(), MemoryCoverage::Partial);
        let result = assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(7), 3).unwrap();
        assert!(result
            .verdict
            .unavailable
            .contains(&"cpu.owner.world".into()));
    }
    #[test]
    fn observed_memory_rejects_cross_frame_device_and_reversed_capture() {
        let mut s = sample();
        assert!(assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(8), 3).is_err());
        assert!(assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(7), 4).is_err());
        let mut cross_run = s.clone();
        std::sync::Arc::make_mut(
            &mut std::sync::Arc::make_mut(cross_run.frame_identity.as_mut().unwrap()).source,
        )
        .run_id = "different-process".into();
        assert!(
            assess_observed_memory(&MemoryBudget::pc_main(), &cross_run, &frame(7), 3).is_err()
        );
        s.capture_finished_ns = 99;
        assert!(assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(7), 3).is_err());
    }
    #[test]
    fn observed_memory_deduplicates_shared_roles_and_keeps_breach_despite_gaps() {
        let mut s = sample();
        let a = AllocationSample {
            allocation_id: "shared".into(),
            domain: MemoryDomain::Gpu,
            owner: "renderer".into(),
            bytes: 5 * GIB,
            roles: BTreeSet::from(["resident".into()]),
        };
        let mut b = a.clone();
        b.roles = BTreeSet::from(["retiring".into()]);
        s.allocations = vec![a, b];
        let r = assess_observed_memory(&MemoryBudget::pc_main(), &s, &frame(7), 3).unwrap();
        assert_eq!(r.tracked_totals.gpu_allocated, 5 * GIB);
        assert_eq!(r.tracked_totals.by_role["gpu.retiring"], 5 * GIB);
        assert_eq!(r.verdict.status, "failed");
        assert!(r.verdict.breaches.contains(&"gpu.total".into()));
        assert!(r.verdict.unavailable.contains(&"cpu.total".into()));
    }
    fn window_samples() -> Vec<ResourceMemorySample> {
        (7..9)
            .map(|i| {
                let mut s = sample();
                s.frame_index = i;
                s.frame_identity = Some(std::sync::Arc::new(frame(i)));
                s.capture_started_ns = i * 100;
                s.capture_finished_ns = i * 100 + 10;
                s
            })
            .collect()
    }
    #[test]
    fn observed_window_keeps_simultaneous_totals_separate_from_category_peaks() {
        let mut samples = window_samples();
        for (s, sizes) in samples.iter_mut().zip([[90, 10], [10, 90]]) {
            s.allocations = sizes
                .into_iter()
                .zip(["world", "renderer"])
                .map(|(bytes, owner)| AllocationSample {
                    allocation_id: owner.into(),
                    domain: MemoryDomain::Cpu,
                    owner: owner.into(),
                    bytes,
                    roles: BTreeSet::from(["resident".into()]),
                })
                .collect();
        }
        let r = assess_observed_memory_window(
            &MemoryBudget::pc_main(),
            &samples,
            &[frame(7), frame(8)],
            3,
            7,
            9,
        )
        .unwrap();
        assert_eq!(r.cpu_total_peak.bytes, 100);
        assert_eq!(r.by_owner_peaks["cpu.world"].bytes, 90);
        assert_eq!(r.by_owner_peaks["cpu.renderer"].bytes, 90);
        assert_eq!(r.by_owner_peaks["cpu.world"].frame_index, 7);
        assert_eq!(r.by_owner_peaks["cpu.renderer"].frame_index, 8);
        assert_eq!(r.verdict.status, "unqualified");
        samples[0].process_private_bytes = Some(7 * GIB);
        let r = assess_observed_memory_window(
            &MemoryBudget::pc_main(),
            &samples,
            &[frame(7), frame(8)],
            3,
            7,
            9,
        )
        .unwrap();
        assert_eq!(r.verdict.status, "failed");
        assert!(r.verdict.breaches.contains(&"process.private".into()));
    }
    #[test]
    fn observed_window_rejects_missing_duplicate_cross_run_device_and_overlapping_capture() {
        let good = window_samples();
        let expected = [frame(7), frame(8)];
        let b = MemoryBudget::pc_main();
        assert!(assess_observed_memory_window(&b, &good, &expected, 3, 7, 9).is_ok());
        assert!(assess_observed_memory_window(&b, &good[..1], &expected, 3, 7, 9).is_err());
        let mut bad = good.clone();
        bad[1] = bad[0].clone();
        assert!(assess_observed_memory_window(&b, &bad, &expected, 3, 7, 9).is_err());
        let mut bad = good.clone();
        bad.reverse();
        assert!(assess_observed_memory_window(&b, &bad, &expected, 3, 7, 9).is_err());
        let mut bad = good.clone();
        bad[1].capture_started_ns = 700;
        assert!(assess_observed_memory_window(&b, &bad, &expected, 3, 7, 9).is_err());
        let mut bad = good.clone();
        bad[1].device_epoch = 4;
        assert!(assess_observed_memory_window(&b, &bad, &expected, 3, 7, 9).is_err());
        let mut cross = expected.clone();
        std::sync::Arc::make_mut(&mut cross[1].source).run_epoch = 2;
        let mut bad = good.clone();
        bad[1].frame_identity = Some(std::sync::Arc::new(cross[1].clone()));
        assert!(assess_observed_memory_window(&b, &bad, &cross, 3, 7, 9).is_err());
    }
    #[test]
    fn observed_memory_rejects_gpu_headroom_without_matching_driver_provenance() {
        let b = MemoryBudget::pc_main();
        let mut s = sample();
        s.gpu_available_bytes = Some(GIB);
        assert!(assess_observed_memory(&b, &s, &frame(7), 3).is_err());
        s.gpu_memory_budget = None;
        assert!(assess_observed_memory(&b, &s, &frame(7), 3).is_err());
        s.gpu_available_bytes = None;
        assert!(assess_observed_memory(&b, &s, &frame(7), 3)
            .unwrap()
            .verdict
            .unavailable
            .contains(&"gpu.available".into()));
    }
}

/// Limits apply independently to each metric's full-window extremum. This does
/// not construct a fictitious simultaneous total from owner/role peaks.
pub fn assess_observed_memory_run(
    budget: &MemoryBudget,
    run: &engine_runtime::resource_memory::ResourceMemoryRunSummary,
    first_frame: u64,
    end_frame: u64,
) -> BenchmarkResult<BudgetVerdict> {
    use engine_runtime::resource_memory::{MemoryCoverage, MEMORY_OWNERS};
    if !run.covers(first_frame, end_frame) {
        return Err(BenchmarkError::new(
            "measurement.memory_window_incomplete",
            "complete ordered full-run capture required",
        ));
    }
    let totals = MemoryTotals {
        cpu_allocated: run.cpu_peak.as_ref().map_or(0, |p| p.bytes),
        gpu_allocated: run.gpu_peak.as_ref().map_or(0, |p| p.bytes),
        by_owner: run
            .owner_peaks
            .iter()
            .map(|(k, v)| (k.clone(), v.bytes))
            .collect(),
        by_role: run
            .role_peaks
            .iter()
            .map(|(k, v)| (k.clone(), v.bytes))
            .collect(),
        unique_allocations: 0,
    };
    let mut verdict = assess_memory(
        budget,
        &totals,
        &ExternalMemorySample {
            process_private: run.process_private_peak.as_ref().map(|p| p.bytes),
            system_available: run.system_available_minimum.as_ref().map(|p| p.bytes),
            gpu_available: run.gpu_available_minimum.as_ref().map(|p| p.bytes),
        },
        &run.capacity_peaks,
    );
    verdict
        .unavailable
        .extend(run.missing_inputs.iter().cloned());
    for domain in ["cpu", "gpu"] {
        let mut complete = true;
        for owner in MEMORY_OWNERS {
            let key = format!("{domain}.owner.{owner}");
            if run.coverage.get(&key) != Some(&MemoryCoverage::Complete) {
                verdict.unavailable.push(key);
                complete = false;
            }
        }
        if !complete {
            verdict.unavailable.push(format!("{domain}.total"));
        }
    }
    for key in budget.role_limits.keys() {
        let (domain, role) = key
            .split_once('.')
            .ok_or_else(|| BenchmarkError::new("measurement.memory_role", key))?;
        if run.coverage.get(&format!("{domain}.role.{role}")) != Some(&MemoryCoverage::Complete) {
            verdict.unavailable.push(key.clone());
        }
    }
    verdict.unavailable.sort();
    verdict.unavailable.dedup();
    if verdict.breaches.is_empty() && !verdict.unavailable.is_empty() {
        verdict.status = "unqualified".into();
    }
    Ok(verdict)
}
