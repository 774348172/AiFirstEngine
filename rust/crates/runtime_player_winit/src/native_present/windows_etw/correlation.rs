//! Explicit-key candidate chains. No inferred screen time or "oldest present" fallback.
use engine_runtime::windowed_player::{
    NativePresentFrameObservation, WindowsEtwCaptureReport, WindowsEtwCorrelationReport,
    WindowsEtwDecodedEvent, WindowsEtwFrameCorrelation,
};
use std::collections::{BTreeMap, BTreeSet};

// Minimum per-record charge also bounds all per-event correlation indices.
const MAX_EVENTS: usize =
    super::decode::MAX_DECODED_BYTES / (2 * std::mem::size_of::<WindowsEtwDecodedEvent>());
const MAX_FRAMES: usize = 262_144;
type Groups<K> = BTreeMap<K, Vec<usize>>;
type Intervals = BTreeMap<u32, Vec<(i64, i64, usize)>>;

pub(super) fn field(e: &WindowsEtwDecodedEvent, name: &str) -> Option<u64> {
    let mut matches = e.fields.iter().filter(|(key, _)| key == name);
    let value = matches.next()?.1;
    matches.next().is_none().then_some(value)
}

pub(super) fn nonzero(e: &WindowsEtwDecodedEvent, name: &str) -> Option<u64> {
    field(e, name).filter(|v| *v != 0)
}

fn kind(e: &WindowsEtwDecodedEvent) -> u16 {
    use engine_runtime::windowed_player::WindowsEtwProvider;
    if e.provider != WindowsEtwProvider::DxgKrnl {
        return match (e.provider, e.event_id, e.version, e.opcode) {
            (WindowsEtwProvider::Win32k, 201 | 301, 1, 0) => e.event_id,
            (WindowsEtwProvider::DwmCore, 15, 0, 1) => 15,
            (WindowsEtwProvider::DwmCore, 196 | 467 | 468 | 476, 0, 0) => e.event_id,
            _ => 0,
        };
    }
    match (e.event_id, e.version, e.opcode) {
        (27, 2, 1) | (28, 2, 2) | (29, 2, 3) => e.event_id,
        (30, 0, 1) | (31, 0, 2) | (32, 0, 3) => e.event_id,
        (422, 0, 1) | (423, 0, 2) | (424, 0, 3) | (450, 0, 0) | (451, 0, 0) => e.event_id,
        (259 | 266, 3, 0) => e.event_id,
        (273, 4, 0) | (382, 2, 0) | (386, 8, 0) => e.event_id,
        (171, 2, 1) | (215, 2, 1) => 171,
        (172, 2, 0) => 172,
        (168, 0, 0) => 168,
        (178, 1, 1) => 178,
        (244, 1 | 2, 1) => 244,
        (180, 1, 2) => 180,
        (116, 0, 0) => 116,
        (17, 0, 0) => 17,
        (184, 1, 0) => 184,
        _ => 0,
    }
}

pub(super) fn only<K: Ord>(groups: &Groups<K>, key: &K) -> Option<usize> {
    let values = groups.get(key)?;
    (values.len() == 1).then(|| values[0])
}

fn intervals(frames: &[NativePresentFrameObservation]) -> (Intervals, Vec<bool>) {
    let mut counts = BTreeMap::new();
    for f in frames {
        *counts
            .entry((f.submission.device_epoch, f.submission.serial))
            .or_insert(0usize) += 1;
    }
    let mut invalid = vec![false; frames.len()];
    let mut by_thread: Intervals = BTreeMap::new();
    for (index, f) in frames.iter().enumerate() {
        let valid_token = f.submission.device_epoch != 0
            && f.submission.serial != 0
            && counts[&(f.submission.device_epoch, f.submission.serial)] == 1
            && f.frame_identity
                .as_ref()
                .is_none_or(|id| id.frame_index == f.submission.frame_index);
        match (f.application_present_qpc, f.application_present_thread_id) {
            (Some([begin, end]), Some(thread)) if begin >= 0 && begin < end && thread != 0 => {
                invalid[index] = !valid_token;
                by_thread
                    .entry(thread)
                    .or_default()
                    .push((begin, end, index));
            }
            _ => invalid[index] = true,
        }
    }
    for values in by_thread.values_mut() {
        values.sort_unstable();
        let mut furthest: Option<(i64, usize)> = None;
        for &(begin, end, index) in values.iter() {
            if let Some((max_end, other)) = furthest {
                if begin < max_end {
                    invalid[index] = true;
                    invalid[other] = true;
                }
                if end <= max_end {
                    continue;
                }
            }
            furthest = Some((end, index));
        }
    }
    (by_thread, invalid)
}

fn call_for(
    e: &WindowsEtwDecodedEvent,
    pid: u32,
    calls: &Intervals,
    invalid: &[bool],
) -> Option<usize> {
    if e.process_id != pid {
        return None;
    }
    let calls = calls.get(&e.thread_id)?;
    let upper = calls.partition_point(|(begin, _, _)| *begin < e.timestamp);
    let &(begin, end, index) = calls.get(upper.checked_sub(1)?)?;
    (!invalid[index] && begin < e.timestamp && e.timestamp < end).then_some(index)
}

#[derive(Default)]
struct Index {
    roots: Groups<usize>,
    failed_calls: BTreeSet<usize>,
    history_starts: Groups<(u64, u64)>,
    history_info: Groups<(u64, u64)>,
    queue_calls: Groups<usize>,
    queue_starts: Groups<(u64, u64)>,
    queue_stops: Groups<(u64, u64)>,
    sequences: Groups<u64>,
    mmio: Groups<u64>,
    sync: Groups<u64>,
}

impl Index {
    fn new(
        events: &[WindowsEtwDecodedEvent],
        pid: u32,
        calls: &Intervals,
        invalid: &[bool],
    ) -> Self {
        let mut index = Self::default();
        for (i, e) in events.iter().enumerate() {
            match kind(e) {
                171 | 168 => {
                    if let Some(call) = call_for(e, pid, calls, invalid) {
                        index.roots.entry(call).or_default().push(i);
                    }
                    if kind(e) == 171 {
                        if let Some(key) = nonzero(e, "hAdapter").zip(nonzero(e, "Token")) {
                            index.history_starts.entry(key).or_default().push(i);
                        }
                    }
                }
                172 => {
                    if let Some(key) = nonzero(e, "hAdapter").zip(nonzero(e, "Token")) {
                        index.history_info.entry(key).or_default().push(i);
                    }
                }
                178 | 244 => {
                    if let Some(sequence) =
                        nonzero(e, "SubmitSequence").filter(|v| *v <= u64::from(u32::MAX))
                    {
                        // Include other processes, contexts and non-present packets:
                        // a sync event lacking context cannot safely pick one of them.
                        index.sequences.entry(sequence).or_default().push(i);
                        if let Some(context) = nonzero(e, "hContext") {
                            index
                                .queue_starts
                                .entry((context, sequence))
                                .or_default()
                                .push(i);
                        }
                    }
                    if kind(e) == 178 && field(e, "bPresent") == Some(1) {
                        if let Some(call) = call_for(e, pid, calls, invalid) {
                            index.queue_calls.entry(call).or_default().push(i);
                        }
                    }
                }
                180 => {
                    if let Some(key) = nonzero(e, "hContext").zip(nonzero(e, "SubmitSequence")) {
                        index.queue_stops.entry(key).or_default().push(i);
                    }
                }
                116 => {
                    if let Some(sequence) = nonzero(e, "FlipSubmitSequence") {
                        index.mmio.entry(sequence).or_default().push(i);
                    }
                }
                17 => {
                    if let Some(sequence) = nonzero(e, "FlipFenceId")
                        .map(|v| v >> 32)
                        .filter(|v| *v != 0)
                    {
                        index.sync.entry(sequence).or_default().push(i);
                    }
                }
                // Present ReturnStatus has 32-bit status semantics. Never
                // keep a positive candidate after a failure-status event.
                184 if field(e, "ReturnStatus")
                    .is_none_or(|v| v > u64::from(u32::MAX) || v & 0x8000_0000 != 0) =>
                {
                    if let Some(call) = call_for(e, pid, calls, invalid) {
                        index.failed_calls.insert(call);
                    }
                }
                _ => (),
            }
        }
        index
    }

    fn history_ready(&self, events: &[WindowsEtwDecodedEvent], root: usize) -> Option<usize> {
        let e = &events[root];
        let key = nonzero(e, "hAdapter").zip(nonzero(e, "Token"))?;
        if only(&self.history_starts, &key) != Some(root) {
            return None;
        }
        let info = only(&self.history_info, &key)?;
        (events[info].timestamp > e.timestamp
            && field(e, "Model").is_some()
            && field(e, "Model") == field(&events[info], "Model"))
        .then_some(info)
    }

    fn flip_chain(
        &self,
        events: &[WindowsEtwDecodedEvent],
        call: usize,
        root: usize,
        lifetimes: &super::lifetime::Lifetimes,
    ) -> Option<[usize; 6]> {
        let e = &events[root];
        if field(e, "MMIOFlip") != Some(1) {
            return None;
        }
        let queue = only(&self.queue_calls, &call)?;
        let q = &events[queue];
        let sequence = nonzero(q, "SubmitSequence")?;
        let context = nonzero(q, "hContext")?;
        if events.iter().any(|e| {
            matches!(kind(e), 259 | 266)
                && (field(e, "SubmitSequence") == Some(sequence)
                    || field(e, "FlipSubmitSequence").is_some_and(|v| v >> 32 == sequence))
                || matches!(kind(e), 273 | 382 | 386)
                    && super::independent::multi_plane_sequences(e).is_some_and(|values| {
                        values.iter().any(|v| {
                            if kind(e) == 386 {
                                *v == sequence
                            } else {
                                *v >> 32 == sequence
                            }
                        })
                    })
        }) {
            return None;
        }
        if only(&self.queue_starts, &(context, sequence)) != Some(queue)
            || only(&self.sequences, &sequence) != Some(queue)
            || field(q, "PacketType") != Some(3)
        {
            return None;
        }
        let stop = only(&self.queue_stops, &(context, sequence))?;
        let complete = &events[stop];
        if field(complete, "bPreempted") != Some(0) || field(complete, "bTimeouted") != Some(0) {
            return None;
        }
        let mmio = only(&self.mmio, &sequence)?;
        let m = &events[mmio];
        // Only the synchronous legacy path is a candidate here. Immediate/MPO/
        // composed paths need their own complete causal chains, not this fallback.
        if field(m, "Flags") != Some(4) {
            return None;
        }
        let sync = only(&self.sync, &sequence)?;
        let s = &events[sync];
        let adapter = nonzero(m, "pDxgAdapter")?;
        let source = field(e, "VidPnSourceId")?;
        if field(m, "VidPnSourceId") != Some(source)
            || field(s, "VidPnSourceId") != Some(source)
            || field(s, "pDxgAdapter") != Some(adapter)
        {
            return None;
        }
        let (device, context, owner_adapter) =
            lifetimes.context(events, context, e.process_id, e.timestamp, s.timestamp)?;
        if owner_adapter != adapter {
            return None;
        }
        (e.timestamp < q.timestamp
            && q.timestamp < complete.timestamp
            && complete.timestamp < m.timestamp
            && m.timestamp < s.timestamp)
            .then_some([queue, stop, mmio, sync, device, context])
    }
}

pub(crate) fn correlate(
    capture: &WindowsEtwCaptureReport,
    frames: &[NativePresentFrameObservation],
) -> WindowsEtwCorrelationReport {
    let started = std::time::Instant::now();
    let mut report = correlate_inputs(capture, frames);
    report.processing_elapsed_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    report
}

pub(crate) fn attach_to_report(
    mut capture: WindowsEtwCaptureReport,
    report: &mut engine_runtime::windowed_player::NativePresentReport,
) {
    capture.correlated = Some(correlate(&capture, &report.frames));
    report.windows_etw = Some(capture);
}

fn correlate_inputs(
    capture: &WindowsEtwCaptureReport,
    frames: &[NativePresentFrameObservation],
) -> WindowsEtwCorrelationReport {
    let mut report = WindowsEtwCorrelationReport {
        frames_examined: frames.len() as u64,
        unmatched_frames: frames.len() as u64,
        ..Default::default()
    };
    let Some(decoded) = capture.decoded.as_ref() else {
        report.status = "not_attempted_no_decoded_input".into();
        return report;
    };
    if frames.len() > MAX_FRAMES
        || decoded.events.len() > MAX_EVENTS
        || super::decode::decoded_input_bytes(&decoded.events) > super::decode::MAX_DECODED_BYTES
    {
        report.status = "input_capacity_rejected".into();
        return report;
    }
    if capture.status != "raw_capture_unresolved"
        || capture.rundown_error.is_some()
        || capture.cleanup_status != "stopped"
        || capture.events_lost != Some(0)
        || capture.log_buffers_lost != Some(0)
        || decoded.open_error.is_some()
        || decoded.process_error.is_some()
        || decoded.close_error.is_some()
        || decoded.buffers_lost != 0
        || decoded.property_errors != 0
        || decoded.omitted_events != 0
        || decoded.unsupported_events != 0
        || decoded.events.iter().any(|e| {
            kind(e) == 244 && nonzero(e, "SubmitSequence").is_none_or(|v| v > u64::from(u32::MAX))
        })
        || decoded.events.iter().any(|e| kind(e) == 0)
        || decoded.events.iter().any(|e| {
            matches!(kind(e), 273 | 382 | 386)
                && super::independent::multi_plane_sequences(e).is_none()
        })
        || decoded.event_budget_reached
        || decoded.status != "decoded_inputs_no_attribution"
    {
        report.status = "incomplete_input_no_attribution".into();
        return report;
    }
    if capture.process_id == 0
        || decoded.clock_type != 1
        || capture.qpc_frequency.is_none_or(|f| f <= 0)
        || capture.qpc_frequency != decoded.trace_qpc_frequency
    {
        report.status = "incompatible_clock_or_process".into();
        return report;
    }
    let (calls, invalid) = intervals(frames);
    report.rejected_frames = invalid.iter().filter(|v| **v).count() as u64;
    let events = &decoded.events;
    let index = Index::new(events, capture.process_id, &calls, &invalid);
    let lifetimes = super::lifetime::Lifetimes::new(events);
    if !lifetimes.hardware_inputs_complete(events) {
        report.status = "incomplete_input_no_attribution".into();
        return report;
    }
    let composition = super::composition::CompositionIndex::new(events, |e| {
        call_for(e, capture.process_id, &calls, &invalid)
    });
    let mut roots_by_call = composition.call_roots().clone();
    // Dxg roots remain authoritative when present. A Win32k token can root a
    // composition-only call without inventing a missing Dxg history event.
    roots_by_call.extend(index.roots.clone());
    for (call, roots) in &roots_by_call {
        if roots.len() != 1 {
            report.ambiguous_frames += 1;
            continue;
        }
        let root = roots[0];
        let mut evidence = WindowsEtwFrameCorrelation {
            legacy_device_event: None,
            legacy_context_event: None,
            independent_flip: None,
            composition: None,
            frame_record_index: *call as u32,
            submission: frames[*call].submission,
            state: "unresolved_no_display_attribution".into(),
            root_event: root as u32,
            history_info_event: None,
            queue_submit_event: None,
            queue_complete_event: None,
            mmio_flip_event: None,
            vsync_event: None,
        };
        if index.failed_calls.contains(call) {
            evidence.state = "application_present_failed".into();
            report.rejected_frames += 1;
        } else if kind(&events[root]) == 171 {
            if let Some(info) = index.history_ready(events, root) {
                evidence.history_info_event = Some(info as u32);
                evidence.state = "history_ready_candidate_display_unknown".into();
                report.history_ready_candidates += 1;
            }
        } else if let Some([queue, stop, mmio, sync, device, context]) =
            index.flip_chain(events, *call, root, &lifetimes)
        {
            evidence.legacy_device_event = Some(device as u32);
            evidence.legacy_context_event = Some(context as u32);
            evidence.queue_submit_event = Some(queue as u32);
            evidence.queue_complete_event = Some(stop as u32);
            evidence.mmio_flip_event = Some(mmio as u32);
            evidence.vsync_event = Some(sync as u32);
            evidence.state = "legacy_flip_chain_candidate_unqualified".into();
            report.legacy_flip_candidates += 1;
        }
        if !index.failed_calls.contains(call) {
            if let Some((provenance, state)) = composition.resolve(events, *call) {
                if provenance.discard_event.is_some() && provenance.in_frame_event.is_none() {
                    report.explicit_discard_candidates += 1;
                } else {
                    report.composition_handoff_candidates += 1;
                }
                evidence.composition = Some(provenance);
                evidence.state = state.into();
            }
            if let Some(token) = only(composition.call_roots(), call) {
                if let Some(provenance) = super::independent::resolve(events, token, &lifetimes) {
                    report.independent_flip_candidates += 1;
                    evidence.independent_flip = Some(provenance);
                    evidence.state = "independent_flip_scanout_candidate_unqualified".into();
                }
            }
        }
        report.frames.push(evidence);
    }
    report.unmatched_frames -= report
        .frames
        .iter()
        .filter(|f| {
            f.state != "unresolved_no_display_attribution"
                && f.state != "application_present_failed"
        })
        .count() as u64;
    report.status = "candidate_chains_unqualified".into();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_runtime::render_thread::GpuSubmissionToken;
    use engine_runtime::windowed_player::{
        NativePresentReport, WindowsEtwDecodeReport, WindowsEtwProvider,
    };

    fn frame(serial: u64, begin: i64) -> NativePresentFrameObservation {
        NativePresentFrameObservation {
            application_present_qpc: Some([begin, begin + 20]),
            application_present_thread_id: Some(9),
            submission: GpuSubmissionToken {
                device_epoch: 1,
                serial,
                frame_index: serial,
                content_version: 1,
            },
            frame_identity: None,
            application_present_index: serial,
            present_id: None,
            swapchain_epoch: 1,
            image_index: 0,
            actual_present_time_ns: None,
            earliest_present_time_ns: None,
            present_margin_ns: None,
        }
    }
    fn event(id: u16, pid: u32, time: i64, keys: &[(&str, u64)]) -> WindowsEtwDecodedEvent {
        let (version, opcode) = match id {
            171 => (2, 1),
            172 => (2, 0),
            178 => (1, 1),
            180 => (1, 2),
            _ => (0, 0),
        };
        WindowsEtwDecodedEvent {
            provider: Default::default(),
            event_id: id,
            version,
            opcode,
            process_id: pid,
            thread_id: 9,
            timestamp: time,
            fields: keys.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }
    fn capture(events: Vec<WindowsEtwDecodedEvent>) -> WindowsEtwCaptureReport {
        WindowsEtwCaptureReport {
            status: "raw_capture_unresolved".into(),
            process_id: 100,
            cleanup_status: "stopped".into(),
            events_lost: Some(0),
            log_buffers_lost: Some(0),
            qpc_frequency: Some(10_000_000),
            decoded: Some(WindowsEtwDecodeReport {
                status: "decoded_inputs_no_attribution".into(),
                clock_type: 1,
                trace_qpc_frequency: Some(10_000_000),
                events,
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    fn history() -> Vec<WindowsEtwDecodedEvent> {
        vec![
            event(
                171,
                100,
                105,
                &[("hAdapter", 1), ("Token", 7), ("Model", 2)],
            ),
            event(172, 4, 130, &[("hAdapter", 1), ("Token", 7), ("Model", 2)]),
        ]
    }

    #[test]
    fn valid_hardware_dma_input_preserves_history_candidate_without_display_promotion() {
        let mut events = history();
        for (id, version, opcode, time, fields) in [
            (
                27,
                2,
                1,
                10,
                vec![("hDevice", 7), ("pDxgAdapter", 1), ("hProcessId", 100)],
            ),
            (
                30,
                0,
                1,
                20,
                vec![("hDevice", 7), ("hContext", 42), ("NodeOrdinal", 0)],
            ),
            (
                424,
                0,
                3,
                30,
                vec![("hContext", 42), ("hHwQueue", 0), ("ParentDxgHwQueue", 900)],
            ),
            (
                450,
                0,
                0,
                40,
                vec![
                    ("hHwQueue", 900),
                    ("ProgressFenceValue", u64::from(u32::MAX) + 8),
                    ("pDmaBuffer", 12),
                    ("ntStatus", 0),
                    ("NumberOfQueuedPendingFlip", 1),
                ],
            ),
            (
                451,
                0,
                0,
                50,
                vec![
                    ("hHwQueue", 900),
                    ("ProgressFenceValue", u64::from(u32::MAX) + 8),
                ],
            ),
        ] {
            let mut e = event(id, 4, time, &fields);
            e.version = version;
            e.opcode = opcode;
            events.push(e);
        }
        let report = correlate(&capture(events.clone()), &[frame(1, 100)]);
        assert_eq!(report.status, "candidate_chains_unqualified");
        assert_eq!(report.history_ready_candidates, 1);
        assert_eq!(report.independent_flip_candidates, 0);
        assert_eq!(
            report.frames[0].state,
            "history_ready_candidate_display_unknown"
        );
        for name in ["hHwQueue", "ProgressFenceValue", "ntStatus"] {
            let mut damaged = events.clone();
            let value = damaged
                .iter_mut()
                .find(|e| e.event_id == 450)
                .unwrap()
                .fields
                .iter_mut()
                .find(|(n, _)| n == name)
                .unwrap();
            value.1 = if name == "ntStatus" { 0xc0000001 } else { 0 };
            let result = correlate(&capture(damaged), &[frame(1, 100)]);
            assert_eq!(result.status, "incomplete_input_no_attribution");
            assert_eq!(result.history_ready_candidates, 0);
        }
    }
    fn flip() -> Vec<WindowsEtwDecodedEvent> {
        let mut events = vec![
            event(168, 100, 104, &[("VidPnSourceId", 3), ("MMIOFlip", 1)]),
            event(
                178,
                100,
                108,
                &[
                    ("hContext", 42),
                    ("PacketType", 3),
                    ("SubmitSequence", 8),
                    ("bPresent", 1),
                ],
            ),
            event(
                180,
                4,
                130,
                &[
                    ("hContext", 42),
                    ("SubmitSequence", 8),
                    ("bPreempted", 0),
                    ("bTimeouted", 0),
                ],
            ),
            event(
                116,
                4,
                140,
                &[
                    ("pDxgAdapter", 1),
                    ("VidPnSourceId", 3),
                    ("FlipSubmitSequence", 8),
                    ("Flags", 4),
                ],
            ),
            event(
                17,
                4,
                150,
                &[
                    ("pDxgAdapter", 1),
                    ("VidPnSourceId", 3),
                    ("FlipFenceId", 8u64 << 32),
                ],
            ),
        ];
        let mut device = event(
            27,
            4,
            80,
            &[("hDevice", 10), ("hProcessId", 100), ("pDxgAdapter", 1)],
        );
        device.version = 2;
        device.opcode = 1;
        let mut context = event(
            30,
            4,
            90,
            &[("hDevice", 10), ("hContext", 42), ("NodeOrdinal", 0)],
        );
        context.opcode = 1;
        events.extend([device, context]);
        events
    }

    #[test]
    fn legacy_flip_rejects_missing_wrong_adapter_or_reused_context_lifetimes() {
        for cause in 0..7 {
            let mut events = flip();
            match cause {
                0 => events.truncate(5),
                1 => events[5].fields[1].1 = 200,
                2 => events[5].fields[2].1 = 2,
                3 => events[6].timestamp = 105,
                4 => {
                    let mut stop = events[6].clone();
                    stop.event_id = 31;
                    stop.opcode = 2;
                    stop.timestamp = 145;
                    events.push(stop);
                }
                5 => events[6].fields[0].1 = 11,
                _ => events.push(events[6].clone()),
            }
            assert_eq!(
                correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
                0,
                "cause {cause}"
            );
        }
    }

    #[test]
    fn direct_consumption_attachment_keeps_parent_index_and_final_display_unknown() {
        let frames = vec![frame(1, 40), frame(2, 100)];
        let mut parent = NativePresentReport {
            frames: frames.clone(),
            ..Default::default()
        };
        attach_to_report(
            capture(super::super::composition::tests::direct()),
            &mut parent,
        );
        assert_eq!(parent.frames, frames);
        assert!(parent.discarded_frames.is_none() && parent.repeated_displays.is_none());
        let result = parent
            .windows_etw
            .as_ref()
            .unwrap()
            .correlated
            .as_ref()
            .unwrap();
        assert_eq!(result.composition_handoff_candidates, 1);
        assert_eq!(result.frames[0].frame_record_index, 1);
        assert_eq!(result.frames[0].submission, frames[1].submission);
        assert_eq!(
            result.frames[0].state,
            "direct_flip_consumed_candidate_display_unknown"
        );
        assert!(result.frames[0].vsync_event.is_none());
        let roundtrip: NativePresentReport =
            serde_json::from_value(serde_json::to_value(&parent).unwrap()).unwrap();
        assert_eq!(roundtrip, parent);
    }

    #[test]
    fn independent_scanout_candidate_keeps_parent_identity_and_never_promotes_native_timing() {
        let events = super::super::independent::tests::fixture();
        let frames = vec![frame(1, 40), frame(2, 100)];
        let mut parent = NativePresentReport {
            frames: frames.clone(),
            ..Default::default()
        };
        attach_to_report(capture(events), &mut parent);
        assert_eq!(parent.frames, frames);
        assert!(parent.discarded_frames.is_none() && parent.repeated_displays.is_none());
        assert!(parent
            .frames
            .iter()
            .all(|f| f.actual_present_time_ns.is_none()));
        let result = parent.windows_etw.unwrap().correlated.unwrap();
        assert_eq!(
            (result.independent_flip_candidates, result.unmatched_frames),
            (1, 1)
        );
        let correlated = &result.frames[0];
        assert_eq!(correlated.frame_record_index, 1);
        assert_eq!(correlated.submission, frames[1].submission);
        assert_eq!(
            correlated.independent_flip.as_ref().unwrap().scanout_event,
            8
        );
        assert_eq!(
            correlated.state,
            "independent_flip_scanout_candidate_unqualified"
        );
    }

    #[test]
    fn independent_chain_requires_unique_successful_application_call_and_complete_trace() {
        for cause in 0..8 {
            let mut events = super::super::independent::tests::fixture();
            let mut frames = vec![frame(1, 100)];
            match cause {
                0 => events.push(event(184, 100, 110, &[("ReturnStatus", 0x8000_0001)])),
                1 => frames.push(frame(2, 101)),
                2 => events[2].process_id = 200,
                3 => events[2].thread_id = 10,
                _ => (),
            }
            let mut cap = capture(events);
            match cause {
                4 => cap.events_lost = Some(1),
                5 => cap.decoded.as_mut().unwrap().unsupported_events = 1,
                6 => cap.decoded.as_mut().unwrap().clock_type = 2,
                7 => cap.rundown_error = Some(5),
                _ => (),
            }
            let result = correlate(&cap, &frames);
            assert_eq!(result.independent_flip_candidates, 0, "cause {cause}");
            assert!(result.frames.iter().all(|f| f.independent_flip.is_none()));
        }
    }

    #[test]
    fn legacy_correlation_json_does_not_invent_independent_provenance() {
        let mut value = serde_json::to_value(correlate(
            &capture(super::super::independent::tests::fixture()),
            &[frame(1, 100)],
        ))
        .unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("independentFlipCandidates");
        value["frames"][0]
            .as_object_mut()
            .unwrap()
            .remove("independentFlip");
        let result: WindowsEtwCorrelationReport = serde_json::from_value(value).unwrap();
        assert_eq!(result.independent_flip_candidates, 0);
        assert!(result.frames[0].independent_flip.is_none());
    }

    #[test]
    fn multiplane_input_cannot_be_ignored_by_legacy_flip_or_incomplete_trace_validation() {
        let mut events = flip();
        events.push(super::super::independent::tests::multi_plane());
        let result = correlate(&capture(events), &[frame(1, 100)]);
        assert_eq!(result.legacy_flip_candidates, 0);
        let mut events = super::super::independent::tests::fixture();
        let mut incomplete = super::super::independent::tests::multi_plane();
        incomplete
            .fields
            .retain(|(n, _)| n != "FlipSubmitSequence[1]");
        events.push(incomplete);
        let result = correlate(&capture(events), &[frame(1, 100)]);
        assert_eq!(result.status, "incomplete_input_no_attribution");
        assert_eq!(result.independent_flip_candidates, 0);
    }

    #[test]
    fn decoded_hardware_queue_dma_and_mpo_cannot_be_assumed_to_be_attributed() {
        for (id, opcode) in [(422, 1), (424, 3), (450, 0), (451, 0), (252, 0)] {
            let mut events = history();
            let mut event = events[0].clone();
            event.event_id = id;
            event.version = 0;
            event.opcode = opcode;
            events.push(event);
            let report = correlate(&capture(events), &[frame(1, 100)]);
            assert_eq!(report.status, "incomplete_input_no_attribution");
            assert_eq!(report.history_ready_candidates, 0);
            assert_eq!(report.independent_flip_candidates, 0);
            assert_eq!(report.composition_handoff_candidates, 0);
            assert_eq!(report.legacy_flip_candidates, 0);
        }
    }

    #[test]
    fn decoded_memory_budget_is_checked_before_candidate_indices() {
        let mut events = history();
        let mut name = String::with_capacity(super::super::decode::MAX_DECODED_BYTES);
        name.push_str("oversized-field");
        events[0].fields[0].0 = name;
        let result = correlate(&capture(events), &[frame(1, 100)]);
        assert_eq!(result.status, "input_capacity_rejected");
        assert!(result.frames.is_empty());
    }
    #[test]
    fn composition_only_and_history_calls_keep_frozen_parent_index_and_unknown_display() {
        for with_history in [false, true] {
            let mut events = super::super::composition::tests::fixture();
            if with_history {
                events.extend(history());
            }
            let frames = vec![frame(1, 10), frame(2, 100)];
            let result = correlate(&capture(events), &frames);
            assert_eq!(result.composition_handoff_candidates, 1);
            assert_eq!(result.unmatched_frames, 1);
            assert_eq!(result.frames.len(), 1);
            assert_eq!(result.frames[0].frame_record_index, 1);
            assert_eq!(result.frames[0].submission, frames[1].submission);
            assert_eq!(
                result.frames[0].state,
                "composition_consumed_candidate_display_unknown"
            );
            assert_eq!(result.frames[0].history_info_event.is_some(), with_history);
            let mut parent = NativePresentReport {
                frames: frames.clone(),
                ..Default::default()
            };
            attach_to_report(
                capture(super::super::composition::tests::fixture()),
                &mut parent,
            );
            assert_eq!(parent.frames, frames);
            assert!(parent.discarded_frames.is_none() && parent.repeated_displays.is_none());
            assert!(parent
                .frames
                .iter()
                .all(|f| f.actual_present_time_ns.is_none()));
        }
    }

    #[test]
    fn composition_failed_present_wrong_provider_and_unsupported_schema_do_not_succeed() {
        for case in 0..4 {
            let mut events = super::super::composition::tests::fixture();
            match case {
                0 => events.push(event(184, 100, 115, &[("ReturnStatus", 0xc0000001)])),
                1 => events[0].provider = WindowsEtwProvider::DwmCore,
                2 => events[3].version = 99,
                _ => events[0].thread_id = 10,
            }
            if case == 0 {
                events.last_mut().unwrap().version = 1;
            }
            let result = correlate(&capture(events), &[frame(1, 100)]);
            assert_eq!(result.composition_handoff_candidates, 0, "case {case}");
            assert_eq!(result.explicit_discard_candidates, 0);
        }
    }

    #[test]
    fn exact_history_handoff_keeps_display_unknown() {
        let f = frame(1, 100);
        let c = capture(history());
        let report = correlate(&c, std::slice::from_ref(&f));
        assert_eq!(report.history_ready_candidates, 1);
        assert_eq!(report.frames[0].history_info_event, Some(1));
        assert_eq!(report.frames[0].submission, f.submission);
        assert!(f.actual_present_time_ns.is_none());
    }
    #[test]
    fn direct_flip_chain_uses_system_events_and_all_explicit_keys() {
        let c = capture(flip());
        let report = correlate(&c, &[frame(1, 100)]);
        assert_eq!(report.legacy_flip_candidates, 1);
        assert_eq!(report.frames[0].vsync_event, Some(4));
        assert_eq!(report.frames[0].legacy_device_event, Some(5));
        assert_eq!(report.frames[0].legacy_context_event, Some(6));
    }

    #[test]
    fn older_frame_json_does_not_invent_legacy_lifetime_origins() {
        let mut value =
            serde_json::to_value(correlate(&capture(flip()), &[frame(1, 100)])).unwrap();
        for key in ["legacyDeviceEvent", "legacyContextEvent"] {
            value["frames"][0].as_object_mut().unwrap().remove(key);
        }
        let old: WindowsEtwCorrelationReport = serde_json::from_value(value).unwrap();
        assert!(old.frames[0].legacy_device_event.is_none());
        assert!(old.frames[0].legacy_context_event.is_none());
    }

    #[test]
    fn wait_packets_preserve_candidates_only_when_their_sequence_is_distinct() {
        for version in [1, 2] {
            for sequence in [8, 9] {
                let mut events = flip();
                let mut wait = event(
                    244,
                    999,
                    90,
                    &[("hContext", 99), ("SubmitSequence", sequence)],
                );
                wait.version = version;
                wait.opcode = 1;
                events.push(wait);
                let report = correlate(&capture(events), &[frame(1, 100)]);
                assert_eq!(report.status, "candidate_chains_unqualified");
                assert_eq!(report.legacy_flip_candidates, u64::from(sequence != 8));
                assert_eq!(report.frames[0].vsync_event, (sequence != 8).then_some(4));
            }
        }
    }

    #[test]
    fn wait_packet_cannot_replace_a_present_queue_submit_even_with_forged_present_flag() {
        let mut events = flip();
        events[1].event_id = 244;
        assert_eq!(
            correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
            0
        );
    }

    #[test]
    fn wait_without_a_valid_sequence_cannot_silently_hide_a_collision() {
        for keys in [
            vec![("hContext", 99)],
            vec![("hContext", 99), ("SubmitSequence", 0)],
            vec![
                ("hContext", 99),
                ("SubmitSequence", u64::from(u32::MAX) + 1),
            ],
        ] {
            let mut events = flip();
            let mut wait = event(244, 999, 90, &keys);
            wait.version = 2;
            wait.opcode = 1;
            events.push(wait);
            let report = correlate(&capture(events), &[frame(1, 100)]);
            assert_eq!(report.legacy_flip_candidates, 0);
            assert_eq!(report.status, "incomplete_input_no_attribution");
        }
    }
    #[test]
    fn wrong_adapter_or_model_never_joins_history_token() {
        for (name, value) in [("hAdapter", 2), ("Model", 9)] {
            let mut events = history();
            events[1].fields.iter_mut().find(|x| x.0 == name).unwrap().1 = value;
            assert_eq!(
                correlate(&capture(events), &[frame(1, 100)]).history_ready_candidates,
                0
            );
        }
    }
    #[test]
    fn token_reuse_and_duplicate_info_do_not_choose_oldest_or_latest() {
        for id in [171, 172] {
            let mut events = history();
            let mut dup = events[usize::from(id == 172)].clone();
            dup.timestamp += 1;
            events.push(dup);
            assert_eq!(
                correlate(&capture(events), &[frame(1, 100)]).history_ready_candidates,
                0
            );
        }
    }
    #[test]
    fn sequence_collision_in_another_context_or_process_blocks_scan_match() {
        let mut events = flip();
        events.push(event(
            178,
            999,
            90,
            &[("hContext", 99), ("SubmitSequence", 8), ("bPresent", 1)],
        ));
        assert_eq!(
            correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
            0
        );
    }
    #[test]
    fn foreign_pid_thread_boundary_and_overlapping_calls_are_not_nearest_matched() {
        for (pid, tid, time) in [
            (999, 9, 105),
            (100, 10, 105),
            (100, 9, 100),
            (100, 9, 120),
            (100, 9, 121),
        ] {
            let mut events = history();
            events[0].process_id = pid;
            events[0].thread_id = tid;
            events[0].timestamp = time;
            assert_eq!(
                correlate(&capture(events), &[frame(1, 100)]).history_ready_candidates,
                0
            );
        }
        assert_eq!(
            correlate(&capture(history()), &[frame(1, 100), frame(2, 101)])
                .history_ready_candidates,
            0
        );
    }
    #[test]
    fn missing_or_reversed_boundaries_and_replayed_submissions_are_rejected() {
        let mut f = frame(1, 100);
        f.application_present_qpc = None;
        assert_eq!(
            correlate(&capture(history()), &[f.clone()]).history_ready_candidates,
            0
        );
        f.application_present_qpc = Some([120, 100]);
        assert_eq!(
            correlate(&capture(history()), &[f]).history_ready_candidates,
            0
        );
        assert_eq!(
            correlate(&capture(history()), &[frame(1, 100), frame(1, 200)])
                .history_ready_candidates,
            0
        );
    }
    #[test]
    fn lost_truncated_or_non_qpc_inputs_cannot_create_candidates() {
        for case in 0..7 {
            let mut c = capture(history());
            match case {
                0 => c.events_lost = Some(1),
                1 => c.decoded.as_mut().unwrap().omitted_events = 1,
                2 => c.decoded.as_mut().unwrap().clock_type = 2,
                3 => c.decoded.as_mut().unwrap().property_errors = 1,
                4 => c.decoded.as_mut().unwrap().process_error = Some(13),
                5 => c.qpc_frequency = Some(1),
                _ => c.status = "access_denied".into(),
            }
            assert!(correlate(&c, &[frame(1, 100)]).frames.is_empty());
        }
    }
    #[test]
    fn wrong_context_source_and_duplicate_vsync_do_not_complete_flip_chain() {
        for case in 0..4 {
            let mut events = flip();
            match case {
                0 => events[2].fields[0].1 = 999,
                1 => events[4].fields[1].1 = 4,
                2 => events.push(events[4].clone()),
                _ => events[2].timestamp = 107,
            }
            assert_eq!(
                correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
                0
            );
        }
    }
    #[test]
    fn multiple_roots_and_zero_keys_are_unresolved() {
        let mut events = flip();
        events.push(history().remove(0));
        assert_eq!(
            correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
            0
        );
        let mut events = history();
        events[0].fields[1].1 = 0;
        assert_eq!(
            correlate(&capture(events), &[frame(1, 100)]).history_ready_candidates,
            0
        );
    }

    #[test]
    fn an_invalid_submission_interval_cannot_hide_an_overlapping_call() {
        let mut bad = frame(2, 101);
        bad.submission.serial = 0;
        let report = correlate(&capture(history()), &[frame(1, 100), bad]);
        assert_eq!(report.history_ready_candidates, 0);
        assert_eq!(report.rejected_frames, 2);
    }

    #[test]
    fn queue_reuse_faults_missing_keys_and_immediate_mode_remain_unresolved() {
        for case in 0..8 {
            let mut events = flip();
            match case {
                0 => events.push(events[1].clone()),
                1 => events.push(events[2].clone()),
                2 => events[2].fields[2].1 = 1,
                3 => events[2].fields[3].1 = 1,
                4 => events[3].fields[3].1 = 2,
                5 => events[1].fields[1].1 = 0,
                6 => events[1].fields.retain(|(name, _)| name != "hContext"),
                _ => events[1].fields[3].1 = 0,
            }
            assert_eq!(
                correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
                0,
                "case {case}"
            );
        }
    }

    #[test]
    fn input_order_is_not_causality_and_unsupported_version_cannot_hide_a_collision() {
        let mut events = flip();
        events.reverse();
        assert_eq!(
            correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
            1
        );
        let mut events = flip();
        let mut unknown = events[1].clone();
        unknown.version = 99;
        events.push(unknown);
        assert!(correlate(&capture(events), &[frame(1, 100)])
            .frames
            .is_empty());
    }

    #[test]
    fn oversized_event_or_frame_input_is_rejected_before_indexing() {
        let mut c = capture(history());
        c.decoded.as_mut().unwrap().events = vec![history().remove(0); MAX_EVENTS + 1];
        assert_eq!(
            correlate(&c, &[frame(1, 100)]).status,
            "input_capacity_rejected"
        );
        assert_eq!(
            correlate(&capture(history()), &vec![frame(1, 100); MAX_FRAMES + 1]).status,
            "input_capacity_rejected"
        );
    }

    #[test]
    fn production_report_attachment_keeps_frozen_frame_and_display_unknown() {
        use engine_runtime::render_thread::{RenderFrameIdentity, RenderFrameSourceIdentity};
        use engine_runtime::windowed_player::NativePresentReport;
        use std::sync::Arc;
        let mut f = frame(1, 100);
        let identity = Arc::new(RenderFrameIdentity {
            source: Arc::new(RenderFrameSourceIdentity {
                run_id: "captured-run".into(),
                run_epoch: 7,
                session_id: "old-session".into(),
                package_content_hash: Some("old-package".into()),
                world_epoch: 9,
            }),
            frame_index: 1,
            tick_pair: [17, 18],
            presentation_alpha_bits: 0.5f32.to_bits(),
            views: Arc::new(Vec::new()),
        });
        f.frame_identity = Some(identity.clone());
        let mut report = NativePresentReport {
            frames: vec![f.clone()],
            source: "unavailable".into(),
            ..Default::default()
        };
        attach_to_report(capture(history()), &mut report);
        let correlated = report
            .windows_etw
            .as_ref()
            .unwrap()
            .correlated
            .as_ref()
            .unwrap();
        let evidence = &correlated.frames[0];
        assert_eq!(evidence.submission, f.submission);
        assert!(Arc::ptr_eq(
            report.frames[evidence.frame_record_index as usize]
                .frame_identity
                .as_ref()
                .unwrap(),
            &identity
        ));
        assert_eq!(report.frames[0], f);
        assert!(report.frames[0].actual_present_time_ns.is_none());
        assert!(report.discarded_frames.is_none() && report.repeated_displays.is_none());
        let roundtrip: NativePresentReport =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        assert_eq!(roundtrip, report);
        assert_eq!(report.source, "unavailable");
    }

    #[test]
    fn wrong_frozen_frame_identity_cannot_be_attributed() {
        use engine_runtime::render_thread::{RenderFrameIdentity, RenderFrameSourceIdentity};
        use std::sync::Arc;
        let mut f = frame(1, 100);
        f.frame_identity = Some(Arc::new(RenderFrameIdentity {
            source: Arc::new(RenderFrameSourceIdentity {
                run_id: "run".into(),
                run_epoch: 1,
                session_id: "session".into(),
                package_content_hash: None,
                world_epoch: 1,
            }),
            frame_index: 99,
            tick_pair: [0, 1],
            presentation_alpha_bits: 0.0f32.to_bits(),
            views: Arc::new(Vec::new()),
        }));
        assert_eq!(
            correlate(&capture(history()), &[f]).history_ready_candidates,
            0
        );
    }

    #[test]
    fn explicit_failed_present_blocks_history_and_flip_candidates() {
        for mut events in [history(), flip()] {
            let mut failed = event(184, 100, 115, &[("ReturnStatus", 0xc0000001)]);
            failed.version = 1;
            events.push(failed);
            let report = correlate(&capture(events), &[frame(1, 100)]);
            assert_eq!(
                report.history_ready_candidates + report.legacy_flip_candidates,
                0
            );
            assert_eq!(report.frames[0].state, "application_present_failed");
        }
        for status in [0, 1] {
            let mut events = flip();
            let mut success = event(184, 100, 115, &[("ReturnStatus", status)]);
            success.version = 1;
            events.push(success);
            assert_eq!(
                correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
                1
            );
        }
        let mut events = flip();
        let mut foreign = event(184, 999, 115, &[("ReturnStatus", 0xc0000001)]);
        foreign.version = 1;
        events.push(foreign);
        assert_eq!(
            correlate(&capture(events), &[frame(1, 100)]).legacy_flip_candidates,
            1
        );
    }
}
