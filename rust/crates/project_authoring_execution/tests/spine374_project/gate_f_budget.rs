//! Offline evaluation of original typed samples; never a runtime report path.
use super::*;
use engine_runtime::windowed_player::{
    WindowedPlayerCpuFrameSample as Cpu, WindowedPlayerGpuFrameSample as Gpu,
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Verdict {
    pub eligible: bool,
    pub passed: bool,
    pub missing: Vec<String>,
    pub failures: Vec<String>,
    pub metrics: Value,
}

fn distribution(mut values: Vec<f64>) -> Value {
    if values.is_empty() || values.iter().any(|n| !n.is_finite()) {
        return Value::Null;
    }
    values.sort_by(f64::total_cmp);
    let at = |p: f64| values[((values.len() as f64 * p).ceil() as usize).saturating_sub(1)];
    json!({"p95":at(0.95),"p99":at(0.99),"max":values.last().unwrap()})
}

fn slope(values: &[f64]) -> f64 {
    let x_mean = (values.len() - 1) as f64 / 2.;
    let y_mean = values.iter().sum::<f64>() / values.len() as f64;
    values
        .iter()
        .enumerate()
        .map(|(i, y)| (i as f64 - x_mean) * (y - y_mean))
        .sum::<f64>()
        / values
            .iter()
            .enumerate()
            .map(|(i, _)| (i as f64 - x_mean).powi(2))
            .sum::<f64>()
}

pub(super) fn evaluate(
    recipe: &Value,
    cpu: &[Cpu],
    gpu: &[Gpu],
    baseline: Option<&[Cpu]>,
) -> Verdict {
    let mut out = Verdict {
        eligible: false,
        passed: false,
        missing: Vec::new(),
        failures: Vec::new(),
        metrics: json!({}),
    };
    let sequence = |frames: &[Cpu]| {
        frames.len() == 4200
            && frames
                .iter()
                .enumerate()
                .all(|(i, f)| f.frame_index == i as u64 + 1 && f.sampled == (i >= 600))
    };
    if !sequence(cpu) {
        out.missing
            .push("ordered 1..4200 CPU capture with 600 warmup / 3600 sampled frames".into());
    }
    if gpu.len() != 3600
        || gpu.iter().enumerate().any(|(i, g)| {
            g.frame_index != i as u64 + 601
                || !g.frame_ms.is_finite()
                || !g.spine_draw_ms.is_finite()
                // Successful native readback accepts equal (quantized) stamps.
                // Draw counts are recorded only after the exclusive range ends.
                || g.spine_draw_ms < 0.
                || g.spine_draw_ms > g.frame_ms
                || g.spine_draw_count == 0
                || g.spine_mixed_source_batches != 0
        })
    {
        out.missing
            .push("3600 ordered, complete, exclusive Spine GPU intervals".into());
    }
    let measured = cpu.iter().filter(|f| f.sampled).collect::<Vec<_>>();
    let actors = recipe["actors"].as_u64().unwrap();
    let layers = recipe["activeLayersPerActor"].as_u64().unwrap();
    let chain = recipe["maxMixingEntriesPerLayer"].as_u64().unwrap() as usize;
    if gpu.iter().any(|g| g.spine_instances < actors) {
        out.missing
            .push("Spine GPU instances for every visible actor".into());
    }
    if measured.len() != 3600
        || measured.iter().any(|f| {
            f.spine.is_none_or(|s| {
                s.geometry_instances != actors
                    || !s.evaluated_instances.is_multiple_of(actors)
                    || s.vertices == 0
                    || s.indices == 0
                    || s.control_ns == 0
                    || s.vertices_clipping_ns == 0
                    || s.projection_encode_ns == 0
                    || (s.evaluated_instances > 0
                        && (s.official_apply_world_ns == 0
                            || s.active_layers != s.evaluated_instances * layers))
                    || s.max_mixing_entries_per_layer > chain
            })
        })
        || measured
            .iter()
            .filter_map(|f| f.spine)
            .map(|s| s.max_mixing_entries_per_layer)
            .max()
            != Some(chain)
    {
        out.missing
            .push("real actor/layer/mix-chain workload and four CPU intervals".into());
    }
    if cpu.iter().any(|f| {
        f.process_memory
            .is_none_or(|m| m.system_available_bytes.is_none())
    }) {
        out.missing
            .push("per-frame process private and system available memory, including warmup".into());
    }
    if cpu.iter().any(|f| {
        f.spatial_gpu
            .is_none_or(|s| s.requested_gpu_backing_bytes.is_none())
    }) {
        out.missing
            .push("per-frame native upload and requested GPU backing bound".into());
    }
    if cpu.windows(2).any(|p| {
        p[1].spatial_gpu
            .zip(p[0].spatial_gpu)
            .is_some_and(|(b, a)| {
                b.cumulative_parameter_uploaded_bytes < a.cumulative_parameter_uploaded_bytes
            })
    }) {
        out.missing
            .push("monotonic native parameter upload counter".into());
    }
    if cpu.iter().any(|f| f.frame_identity.is_none()) {
        // GPU identity is checked against the corresponding sampled CPU frame below.
        out.missing
            .push("CPU frame/tick identity for cycle-aligned memory fit".into());
    }
    if gpu
        .iter()
        .zip(&measured)
        .any(|(g, c)| g.frame_identity.is_none() || g.frame_identity != c.frame_identity)
    {
        out.missing.push("matching CPU/GPU frame identities".into());
    }
    let spine_ticks = cpu
        .iter()
        .skip(2999)
        .map(|f| f.spine.and_then(|s| s.last_fixed_tick))
        .collect::<Option<Vec<_>>>();
    if spine_ticks.as_ref().is_none_or(|ticks| {
        ticks.len() != 1201 || ticks.windows(2).any(|t| t[1].checked_sub(t[0]) != Some(1))
    }) {
        out.missing
            .push("last 1200 frames cover ten complete 120-tick Spine cycles".into());
    }
    let memory = distribution(
        measured
            .iter()
            .filter_map(|f| f.process_memory)
            .map(|m| m.private_commit_bytes as f64)
            .collect(),
    );
    let mut incremental = Value::Null;
    if let Some(empty) =
        baseline.filter(|b| sequence(b) && b.iter().all(|f| f.process_memory.is_some()))
    {
        let base = distribution(
            empty
                .iter()
                .filter(|f| f.sampled)
                .map(|f| f.process_memory.unwrap().private_commit_bytes as f64)
                .collect(),
        );
        incremental = json!({"p95":memory["p95"].as_f64().unwrap_or(f64::NAN)-base["p95"].as_f64().unwrap(),
            "p99":memory["p99"].as_f64().unwrap_or(f64::NAN)-base["p99"].as_f64().unwrap(),
            "max":cpu.iter().filter_map(|f|f.process_memory).map(|m|m.private_commit_bytes).max().unwrap_or(0) as f64 - empty.iter().filter_map(|f|f.process_memory).map(|m|m.private_commit_bytes).max().unwrap() as f64});
    } else {
        out.missing
            .push("complete same-binary empty-scene private-memory baseline".into());
    }
    let uploads = cpu
        .windows(2)
        .filter(|p| p[1].sampled)
        .filter_map(|p| {
            let (a, b) = (p[0].spatial_gpu?, p[1].spatial_gpu?);
            Some(
                (b.frame_uploaded_bytes
                    + b.cumulative_parameter_uploaded_bytes
                        .saturating_sub(a.cumulative_parameter_uploaded_bytes)
                    + p[1].ui_uploaded_bytes) as f64,
            )
        })
        .collect();
    let tail = cpu
        .iter()
        .rev()
        .take(1200)
        .rev()
        .filter_map(|f| f.process_memory)
        .map(|m| m.private_commit_bytes as f64)
        .collect::<Vec<_>>();
    let fit = (tail.len() == 1200).then(|| slope(&tail));
    out.metrics = json!({
        "spineCpuMs":distribution(measured.iter().filter_map(|f|f.spine).map(|s|s.total_ns() as f64/1e6).collect()),
        "controlMs":distribution(measured.iter().filter_map(|f|f.spine).map(|s|s.control_ns as f64/1e6).collect()),
        "officialApplyWorldMs":distribution(measured.iter().filter_map(|f|f.spine).map(|s|s.official_apply_world_ns as f64/1e6).collect()),
        "verticesClippingMs":distribution(measured.iter().filter_map(|f|f.spine).map(|s|s.vertices_clipping_ns as f64/1e6).collect()),
        "projectionEncodeMs":distribution(measured.iter().filter_map(|f|f.spine).map(|s|s.projection_encode_ns as f64/1e6).collect()),
        "spineGpuMs":distribution(gpu.iter().map(|f|f.spine_draw_ms).collect()),
        "cpuFrameMs":distribution(measured.iter().map(|f|f.cpu_frame_ns as f64/1e6).collect()),
        "cpuExcludingPresentWaitMs":distribution(measured.iter().map(|f|f.cpu_frame_ns.saturating_sub(f.present_wait_ns) as f64/1e6).collect()),
        "gpuFrameMs":distribution(gpu.iter().map(|f|f.frame_ms).collect()),
        "uploadBytes":distribution(uploads),"privateBytes":memory,"incrementalPrivateBytes":incremental,
        "minimumSystemAvailableBytes":cpu.iter().filter_map(|f|f.process_memory?.system_available_bytes).min(),
        "requestedGpuBackingBytesMax":cpu.iter().filter_map(|f|f.spatial_gpu?.requested_gpu_backing_bytes).max(),
        "tail1200PrivateBytesPerFrame":fit,
        "gpuBackingScope":"conservative entire backend requested buffers/textures, resident+in-flight+retiring; excludes measurement buffers, external surfaces and driver-private heaps",
        "projectionScope":"Spine neutral vertex/index encoding and Prepare; native generic renderer encoding is outside this stage"
    });
    let limits = &recipe["thresholds"];
    let checks = [
        (
            "spineCpuMs",
            "p95",
            limits["spineCpuP95Ms"].as_f64().unwrap(),
        ),
        (
            "spineCpuMs",
            "p99",
            limits["spineCpuP99Ms"].as_f64().unwrap(),
        ),
        (
            "spineGpuMs",
            "p95",
            limits["spineGpuP95Ms"].as_f64().unwrap(),
        ),
        (
            "spineGpuMs",
            "p99",
            limits["spineGpuP99Ms"].as_f64().unwrap(),
        ),
        ("cpuFrameMs", "p95", 13.3),
        ("cpuFrameMs", "p99", 16.67),
        ("gpuFrameMs", "p95", 13.3),
        ("gpuFrameMs", "p99", 16.67),
        (
            "uploadBytes",
            "max",
            limits["uploadBytesPerFrameMax"].as_f64().unwrap(),
        ),
        (
            "incrementalPrivateBytes",
            "p95",
            limits["incrementalProcessPeakBytes"].as_f64().unwrap(),
        ),
        (
            "incrementalPrivateBytes",
            "p99",
            limits["incrementalProcessPeakBytes"].as_f64().unwrap(),
        ),
        (
            "incrementalPrivateBytes",
            "max",
            limits["incrementalProcessPeakBytes"].as_f64().unwrap(),
        ),
    ];
    for (name, stat, limit) in checks {
        if out.metrics[name][stat].as_f64().is_some_and(|n| n > limit) {
            out.failures.push(format!("{name}.{stat} exceeds {limit}"));
        }
    }
    if out.metrics["minimumSystemAvailableBytes"]
        .as_u64()
        .is_some_and(|n| n < 4_831_838_208)
    {
        out.failures
            .push("system available physical memory below 4.5 GiB".into());
    }
    if out.metrics["requestedGpuBackingBytesMax"]
        .as_u64()
        .is_some_and(|n| n > 256 * 1024 * 1024)
    {
        out.failures
            .push("requested GPU backing exceeds 256 MiB".into());
    }
    if fit.is_some_and(|n| n > 1024.) {
        out.failures
            .push("tail1200 private growth exceeds 1 KiB/frame".into());
    }
    out.eligible = out.missing.is_empty();
    out.passed = out.eligible && out.failures.is_empty();
    out
}

#[test]
fn spine374_f_budget_statistics_keep_nearest_rank_and_large_memory_offsets() {
    assert_eq!(
        distribution((1..=100).map(f64::from).collect()),
        json!({"p95":95.,"p99":99.,"max":100.})
    );
    let values = (0..1200)
        .map(|i| 8_000_000_000. + i as f64 * 1024.)
        .collect::<Vec<_>>();
    assert!((slope(&values) - 1024.).abs() < 1e-8);
    assert!(distribution(vec![f64::NAN]).is_null());
}

#[test]
fn spine374_f_budget_missing_samples_cannot_become_a_zero_budget_pass() {
    let recipe: Value = serde_json::from_str(CONTRACT).unwrap();
    let verdict = evaluate(&recipe["recipes"][0], &[], &[], None);
    assert!(!verdict.passed && !verdict.eligible);
    assert!(verdict.missing.len() >= 4);
    assert!(verdict.metrics["spineCpuMs"].is_null());
}

#[test]
fn spine374_f_budget_rejects_one_frame_upload_reserve_and_tail_growth_failures() {
    use engine_runtime::spine_animation::measurement::SpineCpuFrameSample;
    use engine_runtime::windowed_player::{
        WindowedPlayerProcessMemorySample, WindowedPlayerSpatialGpuSample,
    };
    let recipes: Value = serde_json::from_str(CONTRACT).unwrap();
    let recipe = &recipes["recipes"][0];
    let source = std::sync::Arc::new(engine_runtime::render_thread::RenderFrameSourceIdentity {
        run_id: "budget-owner".into(),
        run_epoch: 1,
        session_id: "recipe".into(),
        package_content_hash: Some("test-package".into()),
        world_epoch: 1,
    });
    let mut frames = (1..=4200)
        .map(|frame_index| Cpu {
            frame_index,
            frame_identity: Some(std::sync::Arc::new(
                engine_runtime::render_thread::RenderFrameIdentity {
                    source: source.clone(),
                    frame_index,
                    tick_pair: [0, 0],
                    presentation_alpha_bits: 0,
                    views: Default::default(),
                },
            )),
            sampled: frame_index > 600,
            warmup: frame_index <= 600,
            cpu_frame_ns: 1_000_000,
            spine: Some(SpineCpuFrameSample {
                first_fixed_tick: Some(frame_index),
                last_fixed_tick: Some(frame_index),
                control_ns: 100,
                official_apply_world_ns: 100,
                vertices_clipping_ns: 100,
                projection_encode_ns: 100,
                evaluated_instances: 1,
                geometry_instances: 1,
                vertices: 512,
                indices: 768,
                active_layers: 2,
                max_mixing_entries_per_layer: 2,
            }),
            process_memory: Some(WindowedPlayerProcessMemorySample {
                working_set_bytes: 1,
                private_commit_bytes: 100_000_000,
                system_available_bytes: Some(8 * 1024 * 1024 * 1024),
                sampling_cpu_ns: 1,
            }),
            spatial_gpu: Some(WindowedPlayerSpatialGpuSample {
                requested_gpu_backing_bytes: Some(16 * 1024 * 1024),
                ..Default::default()
            }),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let empty = frames.clone();
    let gpu = (601..=4200)
        .map(|frame_index| Gpu {
            frame_index,
            frame_identity: frames[frame_index as usize - 1].frame_identity.clone(),
            frame_ms: 1.,
            spine_draw_ms: 0.1,
            spine_draw_count: 8,
            spine_instances: 8,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    assert!(
        evaluate(recipe, &frames, &gpu, Some(&empty)).passed,
        "2D render tickPair is zero; actual Spine tick sequence must establish eligibility"
    );
    let mut quantized = gpu.clone();
    quantized[2091].spine_draw_ms = 0.;
    assert!(
        evaluate(recipe, &frames, &quantized, Some(&empty)).passed,
        "a completed timestamp range can quantize to zero; actual draws and frame identity establish presence"
    );
    quantized[2091].spine_draw_ms = -0.001;
    assert!(!evaluate(recipe, &frames, &quantized, Some(&empty)).eligible);
    quantized[2091].spine_draw_ms = 0.;
    quantized[2091].spine_draw_count = 0;
    assert!(!evaluate(recipe, &frames, &quantized, Some(&empty)).eligible);
    quantized[2091].spine_draw_count = 8;
    quantized[2091].frame_identity = None;
    assert!(!evaluate(recipe, &frames, &quantized, Some(&empty)).eligible);
    frames[4000].spine.as_mut().unwrap().last_fixed_tick = None;
    assert!(!evaluate(recipe, &frames, &gpu, Some(&empty)).eligible);
    frames[4000].spine.as_mut().unwrap().last_fixed_tick = Some(4001);
    frames[800]
        .spatial_gpu
        .as_mut()
        .unwrap()
        .frame_uploaded_bytes = 1_048_577;
    let verdict = evaluate(recipe, &frames, &gpu, Some(&empty));
    assert_eq!(verdict.failures, ["uploadBytes.max exceeds 1048576"]);
    frames[800]
        .spatial_gpu
        .as_mut()
        .unwrap()
        .frame_uploaded_bytes = 0;
    frames[0]
        .process_memory
        .as_mut()
        .unwrap()
        .system_available_bytes = Some(4_831_838_207);
    assert!(
        evaluate(recipe, &frames, &gpu, Some(&empty))
            .failures
            .iter()
            .any(|f| f.contains("4.5 GiB")),
        "warmup reserve violation disappeared"
    );
    frames[0]
        .process_memory
        .as_mut()
        .unwrap()
        .system_available_bytes = Some(8 * 1024 * 1024 * 1024);
    for (i, f) in frames.iter_mut().skip(3000).enumerate() {
        f.process_memory.as_mut().unwrap().private_commit_bytes += i as u64 * 1025;
    }
    assert!(
        evaluate(recipe, &frames, &gpu, Some(&empty))
            .failures
            .iter()
            .any(|f| f.contains("tail1200"))
    );
    frames[2000].spine.as_mut().unwrap().geometry_instances = 0;
    frames[1000]
        .spatial_gpu
        .as_mut()
        .unwrap()
        .requested_gpu_backing_bytes = None;
    let verdict = evaluate(recipe, &frames, &gpu, Some(&empty));
    assert!(
        !verdict.eligible
            && verdict.missing.iter().any(|m| m.contains("actor/layer"))
            && verdict.missing.iter().any(|m| m.contains("backing"))
    );
}
