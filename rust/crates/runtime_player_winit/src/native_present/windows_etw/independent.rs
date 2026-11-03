//! Surface-token keyed independent flip. Never uses DWM scheduling proximity.
use super::correlation::{field, nonzero};
use engine_runtime::windowed_player::{
    WindowsEtwDecodedEvent, WindowsEtwIndependentFlipEvidence, WindowsEtwProvider,
};

fn surface(e: &WindowsEtwDecodedEvent) -> Option<(u64, u64, u64)> {
    Some((
        nonzero(e, "CompositionSurfaceLuid")?,
        nonzero(e, "PresentCount").filter(|v| *v <= u64::from(u32::MAX))?,
        field(e, "BindId")?,
    ))
}

fn descriptor(e: &WindowsEtwDecodedEvent) -> (WindowsEtwProvider, u16, u8, u8) {
    (e.provider, e.event_id, e.version, e.opcode)
}

fn unique(
    events: &[WindowsEtwDecodedEvent],
    mut predicate: impl FnMut(&WindowsEtwDecodedEvent) -> bool,
) -> Option<(usize, &WindowsEtwDecodedEvent)> {
    let mut matches = events.iter().enumerate().filter(|(_, e)| predicate(e));
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

/// Validate complete, bounded MPO array input before any candidate can use it.
pub(super) fn multi_plane_sequences(e: &WindowsEtwDecodedEvent) -> Option<Vec<u64>> {
    if !matches!(
        descriptor(e),
        (WindowsEtwProvider::DxgKrnl, 273, 4, 0)
            | (WindowsEtwProvider::DxgKrnl, 382, 2, 0)
            | (WindowsEtwProvider::DxgKrnl, 386, 8, 0)
    ) {
        return None;
    }
    let arrays = super::decode::arrays(e.event_id, e.version, e.opcode);
    let mut sequences = Vec::new();
    for &(count, max, names) in arrays {
        let count = field(e, count).filter(|n| *n <= max)?;
        for name in names {
            for index in 0..count {
                let value = field(e, &format!("{name}[{index}]"))?;
                if *name == "FlipSubmitSequence" {
                    sequences.push(value);
                }
            }
        }
    }
    if e.event_id == 386 {
        sequences.push(nonzero(e, "FlipSubmitSequence").filter(|v| *v <= u64::from(u32::MAX))?);
    }
    Some(sequences)
}

pub(super) fn resolve(
    events: &[WindowsEtwDecodedEvent],
    token: usize,
    lifetimes: &super::lifetime::Lifetimes,
) -> Option<WindowsEtwIndependentFlipEvidence> {
    use WindowsEtwProvider::{DxgKrnl, Win32k};
    let t = events.get(token)?;
    if descriptor(t) != (Win32k, 201, 1, 0) || t.process_id == 0 || t.timestamp < 0 {
        return None;
    }
    // A damaged competing key/sequence cannot silently disappear from uniqueness checks.
    if events.iter().any(|e| {
        matches!(
            descriptor(e),
            (Win32k, 201 | 301, 1, 0) | (DxgKrnl, 266, 3, 0)
        ) && surface(e).is_none()
            || matches!(
                descriptor(e),
                (DxgKrnl, 178, 1, 1) | (DxgKrnl, 244, 1 | 2, 1)
            ) && nonzero(e, "SubmitSequence").is_none_or(|s| s > u64::from(u32::MAX))
    }) {
        return None;
    }
    let key = surface(t)?;
    if unique(events, |e| {
        descriptor(e) == (Win32k, 201, 1, 0) && surface(e) == Some(key)
    })?
    .0 != token
    {
        return None;
    }
    let (independent, i) = unique(events, |e| {
        descriptor(e) == (DxgKrnl, 266, 3, 0) && surface(e) == Some(key)
    })?;
    let sequence = nonzero(i, "SubmitSequence").filter(|v| *v <= u64::from(u32::MAX))?;
    let context = nonzero(i, "hContext")?;
    let source = u32::try_from(field(i, "VidPnSourceId")?).ok()?;
    let plane = u32::try_from(field(i, "PlaneIndex")?).ok()?;
    if field(i, "FlipTrueImmediate") != Some(0)
        || field(i, "SuppressVSync") != Some(0)
        || field(i, "FlipInterval").is_none_or(|v| v == 0 || v > u64::from(u32::MAX))
    {
        return None;
    }
    let (queue, q) = unique(events, |e| {
        matches!(
            descriptor(e),
            (DxgKrnl, 178, 1, 1) | (DxgKrnl, 244, 1 | 2, 1)
        ) && field(e, "SubmitSequence") == Some(sequence)
    })?;
    if descriptor(q) != (DxgKrnl, 178, 1, 1)
        || field(q, "hContext") != Some(context)
        || q.process_id != t.process_id
        || field(q, "PacketType") != Some(3)
        || field(q, "bPresent") != Some(1)
    {
        return None;
    }
    let (stop, c) = unique(events, |e| {
        descriptor(e) == (DxgKrnl, 180, 1, 2)
            && field(e, "hContext") == Some(context)
            && field(e, "SubmitSequence") == Some(sequence)
    })?;
    if field(c, "bPreempted") != Some(0) || field(c, "bTimeouted") != Some(0) {
        return None;
    }
    let (mmio, m) = unique(events, |e| {
        descriptor(e) == (DxgKrnl, 259, 3, 0)
            && field(e, "FlipSubmitSequence").is_some_and(|v| v >> 32 == sequence)
    })?;
    if field(m, "FlipEntryStatusAfterFlip") != Some(5)
        || field(m, "Enabled") != Some(1)
        || field(m, "LayerIndex") != Some(u64::from(plane))
        || field(m, "VidPnSourceId") != Some(u64::from(source))
    {
        return None;
    }
    // Competing legacy MMIO or another independent surface may not borrow this sequence.
    if events.iter().any(|e| {
        descriptor(e) == (DxgKrnl, 116, 0, 0) && field(e, "FlipSubmitSequence") == Some(sequence)
            || descriptor(e) == (DxgKrnl, 266, 3, 0)
                && field(e, "SubmitSequence") == Some(sequence)
                && surface(e) != Some(key)
    }) {
        return None;
    }
    let fence = nonzero(m, "FlipSubmitSequence")?;
    let (sync, s) = unique(events, |e| {
        descriptor(e) == (DxgKrnl, 17, 0, 0) && field(e, "FlipFenceId") == Some(fence)
    })?;
    let target = u32::try_from(field(s, "VidPnTargetId")?).ok()?;
    if field(s, "VidPnSourceId") != Some(u64::from(source))
        || nonzero(s, "hFlipDevice").is_none()
        || field(s, "FrameNumber").is_none_or(|v| v > u64::from(u32::MAX))
        || nonzero(s, "FrameQPCTime").is_none_or(|v| v > i64::MAX as u64 || v > s.timestamp as u64)
        || !(t.timestamp < q.timestamp
            && q.timestamp < c.timestamp
            && c.timestamp < m.timestamp
            && m.timestamp < s.timestamp
            && t.timestamp < i.timestamp
            && i.timestamp < m.timestamp)
    {
        return None;
    }
    let (device, context_event, adapter) =
        lifetimes.context(events, context, t.process_id, t.timestamp, s.timestamp)?;
    if field(m, "pDxgAdapter") != Some(adapter) || field(s, "pDxgAdapter") != Some(adapter) {
        return None;
    }
    let mut multi_plane_events = Vec::new();
    for (index, e) in events.iter().enumerate() {
        if e.provider != DxgKrnl || !matches!(e.event_id, 273 | 382 | 386) {
            continue;
        }
        let sequences = multi_plane_sequences(e)?;
        if e.event_id == 386 {
            if sequences.contains(&sequence) {
                return None;
            } // This alternate MMIO path is not qualified here.
            continue;
        }
        if !sequences.iter().any(|v| *v >> 32 == sequence) {
            continue;
        }
        // HSync can represent a partial update, not this full-VSync chain.
        if e.event_id != 273
            || field(e, "pDxgAdapter") != Some(adapter)
            || field(e, "VidPnSourceId") != Some(u64::from(source))
            || field(e, "FrameNumber") != field(s, "FrameNumber")
            || e.timestamp < s.timestamp
            || sequences.iter().filter(|v| **v == fence).count() != 1
            || sequences.iter().filter(|v| **v >> 32 == sequence).count() != 1
        {
            return None;
        }
        multi_plane_events.push(index as u32);
    }
    if multi_plane_events.len() > 1 {
        return None;
    }
    let mut states: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| descriptor(e) == (Win32k, 301, 1, 0) && surface(e) == Some(key))
        .collect();
    states.sort_unstable_by_key(|(_, e)| e.timestamp);
    let mut previous = None;
    let mut last = t.timestamp;
    let mut in_frame = None;
    for (index, e) in states {
        let state = field(e, "NewState")?;
        if state > 6 || previous.is_some_and(|p| state <= p) || e.timestamp <= last {
            return None;
        }
        last = e.timestamp;
        previous = Some(state);
        if state == 3 {
            if field(e, "IndependentFlip") != Some(1) || e.timestamp >= m.timestamp {
                return None;
            }
            in_frame = Some(index);
        }
        if state == 6 && e.timestamp <= s.timestamp {
            return None;
        }
    }
    Some(WindowsEtwIndependentFlipEvidence {
        token_event: token as u32,
        in_frame_event: in_frame? as u32,
        independent_event: independent as u32,
        device_event: device as u32,
        context_event: context_event as u32,
        queue_submit_event: queue as u32,
        queue_complete_event: stop as u32,
        mmio_flip_event: mmio as u32,
        scanout_event: sync as u32,
        multi_plane_events,
        adapter,
        vid_pn_source_id: source,
        vid_pn_target_id: target,
        plane_index: plane,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use engine_runtime::windowed_player::WindowsEtwProvider;
    pub(in super::super) fn fixture() -> Vec<WindowsEtwDecodedEvent> {
        let event = |id, time, fields: &[(&str, u64)]| WindowsEtwDecodedEvent {
            provider: WindowsEtwProvider::DxgKrnl,
            event_id: id,
            version: match id {
                27 => 2,
                178 | 180 => 1,
                266 | 259 => 3,
                _ => 0,
            },
            opcode: match id {
                27 | 30 | 178 => 1,
                180 => 2,
                _ => 0,
            },
            process_id: if id == 27 || id == 30 || id == 178 {
                100
            } else {
                4
            },
            thread_id: 9,
            timestamp: time,
            fields: fields.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
        };
        let mut events = vec![
            event(
                27,
                10,
                &[("hDevice", 7), ("pDxgAdapter", 1), ("hProcessId", 100)],
            ),
            event(
                30,
                20,
                &[("hContext", 42), ("hDevice", 7), ("NodeOrdinal", 0)],
            ),
            event(
                201,
                105,
                &[
                    ("CompositionSurfaceLuid", 11),
                    ("PresentCount", 31),
                    ("BindId", 21),
                ],
            ),
            event(
                301,
                106,
                &[
                    ("CompositionSurfaceLuid", 11),
                    ("PresentCount", 31),
                    ("BindId", 21),
                    ("NewState", 3),
                    ("IndependentFlip", 1),
                ],
            ),
            event(
                178,
                108,
                &[
                    ("hContext", 42),
                    ("SubmitSequence", 8),
                    ("PacketType", 3),
                    ("bPresent", 1),
                ],
            ),
            event(
                180,
                130,
                &[
                    ("hContext", 42),
                    ("SubmitSequence", 8),
                    ("bPreempted", 0),
                    ("bTimeouted", 0),
                ],
            ),
            event(
                266,
                135,
                &[
                    ("hContext", 42),
                    ("SubmitSequence", 8),
                    ("CompositionSurfaceLuid", 11),
                    ("PresentCount", 31),
                    ("BindId", 21),
                    ("VidPnSourceId", 3),
                    ("PlaneIndex", 0),
                    ("FlipInterval", 1),
                    ("FlipTrueImmediate", 0),
                    ("SuppressVSync", 0),
                ],
            ),
            event(
                259,
                140,
                &[
                    ("pDxgAdapter", 1),
                    ("VidPnSourceId", 3),
                    ("LayerIndex", 0),
                    ("FlipSubmitSequence", 8u64 << 32),
                    ("FlipEntryStatusAfterFlip", 5),
                    ("Enabled", 1),
                ],
            ),
            event(
                17,
                150,
                &[
                    ("pDxgAdapter", 1),
                    ("VidPnSourceId", 3),
                    ("VidPnTargetId", 2),
                    ("FlipFenceId", 8u64 << 32),
                    ("FrameNumber", 1),
                    ("FrameQPCTime", 150),
                    ("hFlipDevice", 7),
                ],
            ),
        ];
        for e in &mut events[2..4] {
            e.provider = WindowsEtwProvider::Win32k;
            e.version = 1;
            e.process_id = 100;
        }
        events
    }
    #[test]
    fn surface_key_reaches_exact_context_adapter_and_scanout_candidate() {
        let events = fixture();
        let evidence = resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events))
            .expect("exact independent chain");
        assert_eq!(
            (
                evidence.device_event,
                evidence.context_event,
                evidence.token_event,
                evidence.scanout_event
            ),
            (0, 1, 2, 8)
        );
        assert_eq!(
            (
                evidence.adapter,
                evidence.vid_pn_source_id,
                evidence.vid_pn_target_id,
                evidence.plane_index
            ),
            (1, 3, 2, 0)
        );
    }

    fn set(e: &mut WindowsEtwDecodedEvent, name: &str, value: u64) {
        e.fields
            .iter_mut()
            .find(|(n, _)| n == name)
            .expect("fixture field")
            .1 = value;
    }

    pub(in super::super) fn multi_plane() -> WindowsEtwDecodedEvent {
        let mut e = fixture()[8].clone();
        e.event_id = 273;
        e.version = 4;
        e.timestamp = 151;
        e.fields = [
            ("pDxgAdapter", 1),
            ("VidPnSourceId", 3),
            ("FrameNumber", 1),
            ("PlaneCount", 2),
            ("FlipEntryCount", 2),
            ("FlipQueues", 0),
            ("PresentIdOrPhysicalAddress[0]", 42),
            ("PresentIdOrPhysicalAddress[1]", 43),
            ("InterruptTargetPresentId[0]", 0),
            ("InterruptTargetPresentId[1]", 0),
            ("FlipSubmitSequence[0]", 8u64 << 32),
            ("FlipSubmitSequence[1]", 9u64 << 32),
        ]
        .iter()
        .map(|(n, v)| (n.to_string(), *v))
        .collect();
        e
    }

    #[test]
    fn multiplane_corroboration_requires_exact_fence_frame_adapter_and_source() {
        let mut events = fixture();
        events.push(multi_plane());
        let result = resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).unwrap();
        assert_eq!(result.multi_plane_events, [9]);
        for (name, value) in [
            ("pDxgAdapter", 2),
            ("VidPnSourceId", 4),
            ("FrameNumber", 2),
            ("FlipSubmitSequence[0]", (8u64 << 32) | 1),
            ("FlipSubmitSequence[1]", 8u64 << 32),
            ("PlaneCount", 17),
            ("FlipEntryCount", 65),
        ] {
            let mut events = fixture();
            let mut mpo = multi_plane();
            set(&mut mpo, name, value);
            events.push(mpo);
            assert!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none(),
                "{name}={value}"
            );
        }
        let mut events = fixture();
        let mut early = multi_plane();
        early.timestamp = 149;
        events.push(early);
        assert!(resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none());
        for field in [
            "PlaneCount",
            "FlipEntryCount",
            "PresentIdOrPhysicalAddress[1]",
            "FlipSubmitSequence[1]",
        ] {
            let mut events = fixture();
            let mut incomplete = multi_plane();
            incomplete.fields.retain(|(n, _)| n != field);
            events.push(incomplete);
            assert!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none()
            );
        }
        let mut events = fixture();
        events.extend([multi_plane(), multi_plane()]);
        assert!(resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none());
    }

    #[test]
    fn hsync_and_alternate_mmio_cannot_borrow_a_full_vsync_candidate() {
        let mut hsync = multi_plane();
        hsync.event_id = 382;
        hsync.version = 2;
        for (name, _) in &mut hsync.fields {
            *name = name.replace("PresentIdOrPhysicalAddress", "ScannedPhysicalAddress");
        }
        let mut events = fixture();
        events.push(hsync);
        assert!(resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none());
        let mut alternative = multi_plane();
        alternative.event_id = 386;
        alternative.version = 8;
        alternative.fields = vec![("PlaneCount".into(), 0), ("FlipSubmitSequence".into(), 8)];
        let mut events = fixture();
        events.push(alternative);
        assert!(resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none());
    }

    #[test]
    fn rejects_wrong_surface_lifetime_adapter_target_plane_and_packet_semantics() {
        for (index, name, value) in [
            (0, "hProcessId", 200),
            (0, "hProcessId", u64::MAX),
            (0, "pDxgAdapter", 2),
            (1, "hDevice", 8),
            (1, "NodeOrdinal", u64::MAX),
            (2, "PresentCount", u64::MAX),
            (3, "IndependentFlip", 0),
            (3, "NewState", 6),
            (4, "hContext", 43),
            (4, "PacketType", 0),
            (4, "bPresent", 0),
            (5, "bTimeouted", 1),
            (5, "bPreempted", 1),
            (6, "CompositionSurfaceLuid", 12),
            (6, "BindId", 22),
            (6, "PresentCount", 32),
            (6, "FlipInterval", 0),
            (6, "SuppressVSync", 1),
            (6, "FlipTrueImmediate", 1),
            (6, "SubmitSequence", u64::MAX),
            (6, "PlaneIndex", u64::MAX),
            (7, "pDxgAdapter", 2),
            (7, "VidPnSourceId", 4),
            (7, "LayerIndex", 1),
            (7, "FlipSubmitSequence", 8),
            (7, "FlipEntryStatusAfterFlip", 15),
            (7, "Enabled", 0),
            (8, "pDxgAdapter", 2),
            (8, "VidPnSourceId", 4),
            (8, "VidPnTargetId", u64::MAX),
            (8, "FlipFenceId", (8u64 << 32) | 1),
            (8, "hFlipDevice", 0),
            (8, "FrameQPCTime", 0),
            (8, "FrameQPCTime", 151),
        ] {
            let mut events = fixture();
            set(&mut events[index], name, value);
            assert!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none(),
                "{index}/{name}={value}"
            );
        }
    }

    #[test]
    fn rejects_missing_duplicate_competing_and_malformed_evidence() {
        for index in 0..9 {
            let mut events = fixture();
            events.remove(index);
            let token = if index < 2 { 1 } else { 2 };
            assert!(
                resolve(
                    &events,
                    token,
                    &super::super::lifetime::Lifetimes::new(&events)
                )
                .is_none(),
                "missing {index}"
            );
            let mut events = fixture();
            events.push(events[index].clone());
            assert!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none(),
                "duplicate {index}"
            );
        }
        for (index, name, value) in [
            (4, "hContext", 43),
            (6, "BindId", 22),
            (8, "VidPnTargetId", 3),
            (8, "pDxgAdapter", 2),
        ] {
            let mut events = fixture();
            let mut competing = events[index].clone();
            set(&mut competing, name, value);
            events.push(competing);
            assert!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none()
            );
        }
        for index in [2, 3, 6] {
            let mut events = fixture();
            let mut malformed = events[index].clone();
            malformed.fields.clear();
            events.push(malformed);
            assert!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none()
            );
        }
    }

    #[test]
    fn rejects_lifetime_boundaries_and_state_order_without_guessing_repeats() {
        for (index, time) in [
            (0, 21),
            (1, 105),
            (3, 105),
            (4, 130),
            (5, 140),
            (6, 140),
            (7, 150),
            (8, 140),
        ] {
            let mut events = fixture();
            events[index].timestamp = time;
            assert!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none(),
                "{index}/{time}"
            );
        }
        for (index, stop_id, opcode) in [(0, 28, 2), (1, 31, 2)] {
            for time in [104, 108, 150] {
                let mut events = fixture();
                let mut stop = events[index].clone();
                stop.event_id = stop_id;
                stop.opcode = opcode;
                stop.timestamp = time;
                events.push(stop);
                assert!(
                    resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none()
                );
            }
        }
        for time in [149, 150, 151] {
            let mut events = fixture();
            let mut discard = events[3].clone();
            set(&mut discard, "NewState", 6);
            discard.timestamp = time;
            events.push(discard);
            assert_eq!(
                resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_some(),
                time > 150
            );
        }
        let mut events = fixture();
        let mut repeated = events[8].clone();
        repeated.timestamp = 160;
        set(&mut repeated, "FrameQPCTime", 160);
        set(&mut repeated, "FrameNumber", 2);
        events.push(repeated);
        assert!(
            resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events)).is_none(),
            "a second scanout is not automatically a repeat qualification"
        );
    }

    #[test]
    fn context_handle_reuse_is_bound_to_separate_device_generations_and_rundown_pid() {
        let mut events = fixture();
        for (index, id, time) in [(1, 31, 155), (0, 28, 160)] {
            let mut stop = events[index].clone();
            stop.event_id = id;
            stop.opcode = 2;
            stop.timestamp = time;
            events.push(stop);
        }
        let mut next = fixture();
        next[0].event_id = 29;
        next[0].opcode = 3;
        next[0].process_id = 4;
        next[1].event_id = 32;
        next[1].opcode = 3;
        next[1].process_id = 4;
        for e in &mut next {
            e.timestamp += 200;
            for (n, v) in &mut e.fields {
                match n.as_str() {
                    "PresentCount" => *v = 32,
                    "pDxgAdapter" => *v = 2,
                    "SubmitSequence" => *v = 9,
                    "FlipSubmitSequence" | "FlipFenceId" => *v = 9u64 << 32,
                    "FrameQPCTime" => *v += 200,
                    _ => (),
                }
            }
        }
        events.extend(next);
        let life = super::super::lifetime::Lifetimes::new(&events);
        let old = resolve(&events, 2, &life).unwrap();
        let new = resolve(&events, 13, &life).unwrap();
        assert_eq!(
            (old.device_event, old.context_event, old.adapter),
            (0, 1, 1)
        );
        assert_eq!(
            (new.device_event, new.context_event, new.adapter),
            (11, 12, 2)
        );
        // Rundown header PID belongs to the system; the device's hProcessId is authoritative.
        assert_eq!(events[new.device_event as usize].process_id, 4);
        events[9].timestamp = 350; // old context now overlaps its reused generation
        assert!(resolve(
            &events,
            13,
            &super::super::lifetime::Lifetimes::new(&events)
        )
        .is_none());
    }

    #[test]
    fn matching_rundown_corroborates_live_start_without_replacing_lifetime_origin() {
        let mut events = fixture();
        for (index, id, time) in [(0, 29, 30), (1, 32, 40)] {
            let mut snapshot = events[index].clone();
            snapshot.event_id = id;
            snapshot.opcode = 3;
            snapshot.timestamp = time;
            snapshot.process_id = 4;
            events.push(snapshot);
        }
        let result = resolve(&events, 2, &super::super::lifetime::Lifetimes::new(&events))
            .expect("start/rundown race with matching state");
        assert_eq!((result.device_event, result.context_event), (0, 1));
        for (index, field) in [(9, "pDxgAdapter"), (10, "hDevice"), (10, "NodeOrdinal")] {
            let mut competing = events.clone();
            set(&mut competing[index], field, 99);
            assert!(resolve(
                &competing,
                2,
                &super::super::lifetime::Lifetimes::new(&competing)
            )
            .is_none());
        }
    }
}
