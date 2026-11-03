//! Formal Off CPU/memory budgets and separately measured feature/GPU limits.
use super::*;

pub(super) fn evaluate(
    recipe: &Value,
    trace: &budget::Verdict,
    off: &Value,
    empty: &Value,
) -> budget::Verdict {
    let mut missing = trace
        .missing
        .iter()
        .filter(|m| !m.contains("empty-scene"))
        .cloned()
        .collect::<Vec<_>>();
    // Trace's per-frame histories are measurement cost. Its feature intervals,
    // actual GPU work, upload and resident backing retain their frozen limits.
    let mut failures = trace
        .failures
        .iter()
        .filter(|f| {
            f.starts_with("spineCpuMs")
                || f.starts_with("spineGpuMs")
                || f.starts_with("gpuFrameMs")
                || f.starts_with("uploadBytes")
                || f.starts_with("requested GPU backing")
        })
        .cloned()
        .collect::<Vec<_>>();
    for (label, summary) in [("off", off), ("empty-off", empty)] {
        let w = &summary["samplingWindow"];
        if summary["warmupFrames"] != 600
            || summary["requestedSampleFrames"] != 3600
            || summary["observedSampleFrames"] != 3600
            || w["complete"] != true
            || w["firstFrame"] != 601
            || w["lastFrame"] != 4200
            || w["wallNs"].as_u64().is_none_or(|n| n < 60_000_000_000)
            || summary["gpu"].is_object()
            || summary["rawCpuFrames"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
            || w["minimumSystemAvailableBytes"].as_u64().is_none()
            || w["tail1200PrivateBytesPerFrame"]
                .as_f64()
                .is_none_or(|v| !v.is_finite())
            || w["peakPrivateCommitBytes"].as_u64().is_none()
            || w["requestedPeakPrivateCommitBytes"].as_u64().is_none()
            || w["privateCommitPercentileBytes"]
                .as_array()
                .is_none_or(|a| a.len() != 2 || a.iter().any(|n| n.as_u64().is_none()))
        {
            missing.push(format!("{label}: complete Off CPU/memory sampling window"));
        }
    }
    for (stat, limit) in [("p95CpuFrameMs", 13.3), ("p99CpuFrameMs", 16.67)] {
        match off[stat].as_f64() {
            Some(v) if v.is_finite() && v <= limit => {}
            Some(_) => failures.push(format!("Off CPU {stat} exceeds {limit}")),
            None => missing.push(format!("Off CPU {stat}")),
        }
    }
    let w = &off["samplingWindow"];
    if w["minimumSystemAvailableBytes"]
        .as_u64()
        .is_some_and(|n| n < 4_831_838_208)
    {
        failures.push("Off system available physical memory below 4.5 GiB".into());
    }
    if w["tail1200PrivateBytesPerFrame"]
        .as_f64()
        .is_some_and(|n| n > 1024.)
    {
        failures.push("Off tail1200 private growth exceeds 1 KiB/frame".into());
    }
    let increment = w["requestedPeakPrivateCommitBytes"]
        .as_u64()
        .zip(empty["samplingWindow"]["requestedPeakPrivateCommitBytes"].as_u64())
        .map(|(feature, base)| feature as i128 - base as i128);
    if increment.is_some_and(|n| {
        n > recipe["thresholds"]["incrementalProcessPeakBytes"]
            .as_u64()
            .unwrap() as i128
    }) {
        failures.push("Off incremental private peak exceeds recipe limit".into());
    }
    let percentile_increments = [0, 1].map(|i| {
        w["privateCommitPercentileBytes"][i]
            .as_u64()
            .zip(empty["samplingWindow"]["privateCommitPercentileBytes"][i].as_u64())
            .map(|(a, b)| a as i128 - b as i128)
    });
    if percentile_increments.iter().flatten().any(|n| {
        *n > recipe["thresholds"]["incrementalProcessPeakBytes"]
            .as_u64()
            .unwrap() as i128
    }) {
        failures.push("Off incremental private P95/P99 exceeds recipe limit".into());
    }
    let metrics = json!({"offCpuP95Ms":off["p95CpuFrameMs"],"offCpuP99Ms":off["p99CpuFrameMs"],
        "offMemoryWindow":w,"emptyOffMemoryWindow":empty["samplingWindow"],
        "offIncrementalPrivatePeakBytes":increment,"offIncrementalPrivateP95P99Bytes":percentile_increments,
        "traceFeatureMetrics":trace.metrics,
        "scope":"Off CPU/common bounded probe and memory; feature CPU/GPU/upload/backing measured separately with Trace; no diagnostic cost subtraction"});
    budget::Verdict {
        eligible: missing.is_empty(),
        passed: missing.is_empty() && failures.is_empty(),
        missing,
        failures,
        metrics,
    }
}

#[test]
fn spine374_f_release_budget_rejects_missing_requested_peak() {
    let recipes: Value = serde_json::from_str(CONTRACT).unwrap();
    let trace = budget::Verdict {
        eligible: true,
        passed: true,
        missing: vec![],
        failures: vec![],
        metrics: json!({}),
    };
    let off = json!({"warmupFrames":600,"requestedSampleFrames":3600,"observedSampleFrames":3600,"p95CpuFrameMs":1.,"p99CpuFrameMs":2.,
        "samplingWindow":{"complete":true,"firstFrame":601,"lastFrame":4200,"wallNs":60_000_000_000u64,
        "minimumSystemAvailableBytes":4_831_838_208u64,"tail1200PrivateBytesPerFrame":0.,"peakPrivateCommitBytes":100_000_000,"privateCommitPercentileBytes":[100_000_000,100_000_000]}});
    assert!(
        !evaluate(&recipes["recipes"][0], &trace, &off, &off).eligible,
        "sampled peak cannot establish a peak that includes warmup"
    );
}

#[test]
fn spine374_f_release_budget_separates_trace_cost_and_keeps_off_limits() {
    let recipes: Value = serde_json::from_str(CONTRACT).unwrap();
    let recipe = &recipes["recipes"][0];
    let trace = budget::Verdict {
        eligible: false,
        passed: false,
        missing: vec!["complete same-binary empty-scene private-memory baseline".into()],
        failures: vec![
            "tail1200 private growth exceeds 1 KiB/frame".into(),
            "system available physical memory below 4.5 GiB".into(),
        ],
        metrics: json!({}),
    };
    let mut off = json!({"warmupFrames":600,"requestedSampleFrames":3600,"observedSampleFrames":3600,"p95CpuFrameMs":13.3,"p99CpuFrameMs":16.67,
        "samplingWindow":{"complete":true,"firstFrame":601,"lastFrame":4200,"wallNs":60_000_000_000u64,
        "minimumSystemAvailableBytes":4_831_838_208u64,"tail1200PrivateBytesPerFrame":1024.,"peakPrivateCommitBytes":100_000_000,"requestedPeakPrivateCommitBytes":100_000_000,"privateCommitPercentileBytes":[100_000_000,100_000_000]}});
    let empty = off.clone();
    assert!(evaluate(recipe, &trace, &off, &empty).passed);
    for (field, value) in [
        ("minimumSystemAvailableBytes", json!(4_831_838_207u64)),
        ("tail1200PrivateBytesPerFrame", json!(1025.)),
        ("requestedPeakPrivateCommitBytes", json!(200_000_000)),
    ] {
        let mut bad = off.clone();
        bad["samplingWindow"][field] = value;
        assert!(!evaluate(recipe, &trace, &bad, &empty).passed, "{field}");
    }
    off["p95CpuFrameMs"] = json!(13.3001);
    assert!(!evaluate(recipe, &trace, &off, &empty).passed);
    off["p95CpuFrameMs"] = json!(13.3);
    off["samplingWindow"]["tail1200PrivateBytesPerFrame"] = Value::Null;
    assert!(!evaluate(recipe, &trace, &off, &empty).eligible);
    let mut bad_trace = trace;
    bad_trace
        .missing
        .push("matching CPU/GPU frame identities".into());
    assert!(!evaluate(recipe, &bad_trace, &empty, &empty).eligible);
    bad_trace.missing.pop();
    bad_trace.failures.push("spineCpuMs.p95 exceeds 0.5".into());
    assert!(!evaluate(recipe, &bad_trace, &empty, &empty).passed);
}
