//! Surface-token handoff and explicit pre-composition discard evidence.
//! DWM consuming content is not scan-out; Retired is not a display timestamp.
use super::correlation::{field, nonzero, only};
use engine_runtime::windowed_player::{
    WindowsEtwCompositionEvidence, WindowsEtwDecodedEvent, WindowsEtwProvider,
};
use std::collections::BTreeMap;

type Key = (u64, u64, u64);
type Groups<K> = BTreeMap<K, Vec<usize>>;
// Opaque consumption metadata within a display target, never a PresentId.
type Target = (u64, u64, u64, u64);

fn target(event: &WindowsEtwDecodedEvent) -> Option<Target> {
    let adapter = nonzero(event, "displayAdapterLuid")?;
    nonzero(event, "renderAdapterLuid")?;
    let source = field(event, "vidPnSourceId")?;
    let target = field(event, "vidPnTargetId")?;
    let unique = field(event, "uniqueId")?;
    [source, target, unique]
        .iter()
        .all(|v| *v <= u64::from(u32::MAX))
        .then_some((adapter, source, target, unique))
}

fn key(event: &WindowsEtwDecodedEvent) -> Option<Key> {
    let (surface, count, bind) = match event.provider {
        WindowsEtwProvider::Win32k => (
            nonzero(event, "CompositionSurfaceLuid")?,
            nonzero(event, "PresentCount")?,
            field(event, "BindId")?,
        ),
        WindowsEtwProvider::DwmCore if event.event_id == 196 => {
            let lo = field(event, "luidSurface.lowpart")?;
            let hi = field(event, "luidSurface.highpart")?;
            if lo > u64::from(u32::MAX) || hi > u64::from(u32::MAX) {
                return None;
            }
            (
                (hi << 32) | lo,
                nonzero(event, "PresentCount")?,
                field(event, "bindId")?,
            )
        }
        WindowsEtwProvider::DwmCore => (
            nonzero(event, "surfaceLuid")?,
            nonzero(event, "presentCount")?,
            field(event, "bindId")?,
        ),
        _ => return None,
    };
    // TokenStateChanged's present count is u32; never truncate the other schemas.
    (surface != 0 && count <= u64::from(u32::MAX)).then_some((surface, count, bind))
}

#[derive(Default)]
pub(super) struct CompositionIndex {
    malformed_key: bool,
    calls: Groups<usize>,
    tokens: Groups<Key>,
    states: Groups<Key>,
    updates: Groups<Key>,
    consumed: Groups<Key>,
}

impl CompositionIndex {
    pub(super) fn call_roots(&self) -> &Groups<usize> {
        &self.calls
    }
    pub(super) fn new(
        events: &[WindowsEtwDecodedEvent],
        call_for: impl Fn(&WindowsEtwDecodedEvent) -> Option<usize>,
    ) -> Self {
        let mut index = Self::default();
        for (i, event) in events.iter().enumerate() {
            if !matches!(
                (event.provider, event.event_id, event.version, event.opcode),
                (WindowsEtwProvider::Win32k, 201 | 301, 1, 0)
                    | (WindowsEtwProvider::DwmCore, 196 | 467 | 468 | 476, 0, 0)
            ) {
                continue;
            }
            let Some(key) = key(event) else {
                // A damaged competing event must not disappear from uniqueness checks.
                index.malformed_key = true;
                continue;
            };
            match (event.provider, event.event_id, event.version, event.opcode) {
                (WindowsEtwProvider::Win32k, 201, 1, 0) => {
                    index.tokens.entry(key).or_default().push(i);
                    if let Some(call) = call_for(event) {
                        index.calls.entry(call).or_default().push(i);
                    }
                }
                (WindowsEtwProvider::Win32k, 301, 1, 0) => {
                    index.states.entry(key).or_default().push(i);
                }
                (WindowsEtwProvider::DwmCore, 196, 0, 0) => {
                    index.updates.entry(key).or_default().push(i);
                }
                (WindowsEtwProvider::DwmCore, 467 | 468 | 476, 0, 0) => {
                    index.consumed.entry(key).or_default().push(i);
                    if target(event).is_none() {
                        index.malformed_key = true;
                    }
                }
                _ => (),
            }
        }
        index
    }

    pub(super) fn resolve(
        &self,
        events: &[WindowsEtwDecodedEvent],
        call: usize,
    ) -> Option<(WindowsEtwCompositionEvidence, &'static str)> {
        if self.malformed_key {
            return None;
        }
        let token = only(&self.calls, &call)?;
        let t = &events[token];
        let key = key(t)?;
        if only(&self.tokens, &key) != Some(token) {
            return None;
        }
        let mut evidence = WindowsEtwCompositionEvidence {
            surface_luid: key.0,
            present_count: key.1,
            bind_id: key.2,
            token_event: token as u32,
            in_frame_event: None,
            discard_event: None,
            surface_update_event: None,
            consumed_events: Vec::new(),
        };
        let mut states = self.states.get(&key)?.clone();
        states.sort_unstable_by_key(|i| events[*i].timestamp);
        let mut last = t.timestamp;
        let mut seen = [false; 7];
        let mut previous_state = None;
        for i in states {
            let s = &events[i];
            let state = field(s, "NewState")?;
            if state > 6
                || seen[state as usize]
                || s.timestamp <= last
                || previous_state.is_some_and(|previous| state <= previous)
            {
                return None;
            }
            previous_state = Some(state);
            seen[state as usize] = true;
            last = s.timestamp;
            match state {
                3 => {
                    if !matches!(field(s, "IndependentFlip"), Some(0 | 1)) {
                        return None;
                    }
                    evidence.in_frame_event = Some(i as u32);
                }
                6 => evidence.discard_event = Some(i as u32),
                _ => (), // Created/Pending/Completed/Confirmed/Retired are not display.
            }
        }
        if evidence.discard_event.is_some() && evidence.in_frame_event.is_none() {
            // Discarded after InFrame can be normal token cleanup. Only a fresh
            // token never entering a composition is an explicit discard candidate.
            if self.updates.contains_key(&key) || self.consumed.contains_key(&key) {
                return None;
            }
            return Some((
                evidence,
                "explicit_precomposition_discard_candidate_unqualified",
            ));
        }
        let in_frame = evidence.in_frame_event? as usize;
        let update = if self.updates.contains_key(&key) {
            let update = only(&self.updates, &key)?;
            let u = &events[update];
            if u.process_id == 0
                || u.process_id == t.process_id
                || u.timestamp <= events[in_frame].timestamp
                || !matches!(field(u, "bDirectFlip"), Some(0 | 1))
                || !matches!(field(u, "bIndependentFlip"), Some(0 | 1))
            {
                return None;
            }
            evidence.surface_update_event = Some(update as u32);
            Some(update)
        } else {
            None
        };
        let mut targets = std::collections::BTreeSet::new();
        let mut owner = update.map(|i| events[i].process_id);
        let mut direct = false;
        let mut composed = false;
        for &i in self.consumed.get(&key).into_iter().flatten() {
            let c = &events[i];
            let target = target(c)?;
            if c.process_id == 0
                || c.process_id == t.process_id
                || owner.is_some_and(|pid| pid != c.process_id)
                || c.timestamp <= events[update.unwrap_or(in_frame)].timestamp
                || evidence
                    .discard_event
                    .is_some_and(|d| c.timestamp >= events[d as usize].timestamp)
                || !targets.insert(target)
            {
                return None;
            }
            owner = Some(c.process_id);
            if c.event_id == 476 {
                for name in ["DirectFlipMode", "presentFlags", "syncInterval"] {
                    if field(c, name).is_none_or(|v| v > u64::from(u32::MAX)) {
                        return None;
                    }
                }
                // Values are preserved as native metadata; no mode/interval is
                // interpreted as evidence of display or a scan-out count.
                direct = true;
            } else {
                let u = &events[update?];
                if field(&events[in_frame], "IndependentFlip") != Some(0)
                    || field(u, "bDirectFlip") != Some(0)
                    || field(u, "bIndependentFlip") != Some(0)
                {
                    return None;
                }
                composed = true;
            }
            evidence.consumed_events.push(i as u32);
        }
        if evidence.consumed_events.is_empty() {
            return None;
        }
        evidence
            .consumed_events
            .sort_unstable_by_key(|i| (events[*i as usize].timestamp, *i));
        let state = match (direct, composed) {
            (true, true) => "mixed_consumed_candidate_display_unknown",
            (true, false) => "direct_flip_consumed_candidate_display_unknown",
            _ => "composition_consumed_candidate_display_unknown",
        };
        Some((evidence, state))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    fn event(
        provider: WindowsEtwProvider,
        id: u16,
        time: i64,
        values: &[(&str, u64)],
    ) -> WindowsEtwDecodedEvent {
        WindowsEtwDecodedEvent {
            provider,
            event_id: id,
            version: u8::from(provider == WindowsEtwProvider::Win32k),
            opcode: 0,
            process_id: if provider == WindowsEtwProvider::Win32k {
                100
            } else {
                200
            },
            thread_id: 9,
            timestamp: time,
            fields: values.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
        }
    }
    pub(in super::super) fn fixture() -> Vec<WindowsEtwDecodedEvent> {
        use WindowsEtwProvider::*;
        vec![
            event(
                Win32k,
                201,
                105,
                &[
                    ("CompositionSurfaceLuid", 11),
                    ("PresentCount", 31),
                    ("BindId", 21),
                ],
            ),
            event(
                Win32k,
                301,
                130,
                &[
                    ("CompositionSurfaceLuid", 11),
                    ("PresentCount", 31),
                    ("BindId", 21),
                    ("NewState", 3),
                    ("IndependentFlip", 0),
                ],
            ),
            event(
                DwmCore,
                196,
                140,
                &[
                    ("luidSurface.lowpart", 11),
                    ("luidSurface.highpart", 0),
                    ("PresentCount", 31),
                    ("bindId", 21),
                    ("bDirectFlip", 0),
                    ("bIndependentFlip", 0),
                ],
            ),
            event(
                DwmCore,
                467,
                150,
                &[
                    ("surfaceLuid", 11),
                    ("presentCount", 31),
                    ("bindId", 21),
                    ("displayAdapterLuid", 1),
                    ("vidPnSourceId", 0),
                    ("vidPnTargetId", 1),
                    ("uniqueId", 9),
                    ("renderAdapterLuid", 2),
                ],
            ),
        ]
    }
    fn resolve(
        events: &[WindowsEtwDecodedEvent],
    ) -> Option<(WindowsEtwCompositionEvidence, &'static str)> {
        CompositionIndex::new(events, |e| {
            (e.process_id == 100 && e.thread_id == 9 && e.timestamp > 100 && e.timestamp < 120)
                .then_some(0)
        })
        .resolve(events, 0)
    }
    #[test]
    fn exact_three_part_surface_key_yields_consumption_not_display() {
        let (e, state) = resolve(&fixture()).unwrap();
        assert_eq!(state, "composition_consumed_candidate_display_unknown");
        assert_eq!(e.consumed_events, vec![3]);
        assert_eq!((e.surface_luid, e.present_count, e.bind_id), (11, 31, 21));
    }
    pub(in super::super) fn direct() -> Vec<WindowsEtwDecodedEvent> {
        let mut events = fixture();
        events.remove(2); // DirectFlip has an exact content key; no synthetic update.
        events[2].event_id = 476;
        events[2].fields.extend([
            ("DirectFlipMode".into(), 1),
            ("presentFlags".into(), 0),
            ("syncInterval".into(), 1),
        ]);
        events
    }

    #[test]
    fn direct_consumption_needs_exact_token_and_inframe_but_not_a_synthetic_update() {
        let (e, state) = resolve(&direct()).unwrap();
        assert_eq!(state, "direct_flip_consumed_candidate_display_unknown");
        assert_eq!(e.consumed_events, vec![2]);
        assert!(e.surface_update_event.is_none());
        for cause in 0..7 {
            let mut events = direct();
            match cause {
                0 => events[2].fields[1].1 += 1,
                1 => events[2].fields[2].1 += 1,
                2 => events[2].process_id = 100,
                3 => events[2].timestamp = 129,
                4 => events[1].fields[3].1 = 2,
                5 => events[2].fields.retain(|(n, _)| n != "DirectFlipMode"),
                _ => events[2].fields.push(("syncInterval".into(), 1)),
            }
            assert!(resolve(&events).is_none(), "cause {cause}");
        }
    }

    #[test]
    fn shared_target_statistics_do_not_replace_exact_content_keys() {
        let mut events = fixture();
        let mut competitor = events[3].clone();
        competitor.fields[0].1 += 1;
        competitor.timestamp += 1;
        events.push(competitor);
        // uniqueId is opaque: two surfaces can share output metadata. It must
        // neither join the competitor to this token nor erase the exact match.
        assert_eq!(resolve(&events).unwrap().0.consumed_events, vec![3]);
    }

    #[test]
    fn direct_and_composed_targets_keep_distinct_keys_and_do_not_count_repeats() {
        let mut events = fixture();
        let mut other = direct().pop().unwrap();
        other.timestamp = 155;
        other.fields[5].1 = 2;
        events.push(other);
        let (e, state) = resolve(&events).unwrap();
        assert_eq!(state, "mixed_consumed_candidate_display_unknown");
        assert_eq!(e.consumed_events, vec![3, 4]);
        events[4].fields[5].1 = 1;
        events[4].fields[6].1 = 10;
        assert_eq!(resolve(&events).unwrap().0.consumed_events.len(), 2);
    }

    #[test]
    fn discarded_content_cannot_be_consumed_before_cleanup_or_without_render_adapter() {
        for direct_mode in [false, true] {
            for cause in 0..4 {
                let mut events = if direct_mode { direct() } else { fixture() };
                let consumed = events.len() - 1;
                let mut discard = events[1].clone();
                discard.timestamp = if cause == 0 { 145 } else { 160 };
                discard.fields[3].1 = 6;
                events.push(discard);
                if cause == 1 {
                    let mut other = events[consumed].clone();
                    other.timestamp = 170;
                    other.fields[6].1 = 10;
                    events.push(other);
                } else if cause == 2 {
                    events[consumed]
                        .fields
                        .retain(|(n, _)| n != "renderAdapterLuid");
                } else if cause == 3 {
                    events[consumed]
                        .fields
                        .push(("renderAdapterLuid".into(), 0));
                }
                assert!(
                    resolve(&events).is_none(),
                    "direct {direct_mode} cause {cause}"
                );
            }
        }
    }

    #[test]
    fn independent_direct_consumption_and_target_owner_changes_are_explicit() {
        let mut events = direct();
        events[1].fields[4].1 = 1;
        assert_eq!(resolve(&events).unwrap().0.consumed_events, vec![2]);
        for cause in 0..5 {
            let mut competing = events[2].clone();
            competing.timestamp = 155;
            competing.fields[5].1 = 2;
            match cause {
                0 => competing.process_id = 300,
                1 => competing.process_id = 0,
                2 => competing.fields[3].1 = 0,
                3 => competing.fields[4].1 = u64::MAX,
                _ => competing.fields[8].1 = u64::MAX,
            }
            let mut bad = events.clone();
            bad.push(competing);
            assert!(resolve(&bad).is_none(), "cause {cause}");
        }
    }

    #[test]
    fn newer_content_on_same_surface_is_not_inferred_as_replacing_or_discarding_old_content() {
        let mut events = direct();
        let mut newer = direct();
        for e in &mut newer {
            e.timestamp += 100;
            for (name, value) in &mut e.fields {
                if matches!(name.as_str(), "PresentCount" | "presentCount") {
                    *value += 1;
                }
            }
        }
        events.extend(newer);
        let index = CompositionIndex::new(&events, |e| {
            (e.provider == WindowsEtwProvider::Win32k && e.event_id == 201)
                .then_some(usize::from(e.timestamp > 200))
        });
        let older = index.resolve(&events, 0).unwrap().0;
        let newer = index.resolve(&events, 1).unwrap().0;
        assert_eq!(older.consumed_events, vec![2]);
        assert_eq!(newer.consumed_events, vec![5]);
        assert!(older.discard_event.is_none() && newer.discard_event.is_none());
        // Reorder records as multi-buffer delivery can do; original indices
        // change, exact content keys continue to select the correct payload.
        events.reverse();
        let older = resolve(&events).unwrap().0;
        assert_eq!(older.present_count, 31);
        assert_eq!(older.consumed_events, vec![3]);
    }

    #[test]
    fn reused_tokens_wrong_providers_and_partial_keys_never_choose_latest() {
        for case in 0..8 {
            let mut events = fixture();
            match case {
                0 => events.push(events[0].clone()),
                1 => events[3].fields[2].1 = 22,
                2 => events[3].provider = WindowsEtwProvider::DxgKrnl,
                3 => events[3].process_id = 300,
                4 => events[2].timestamp = 129,
                5 => events[0].fields.push(("BindId".into(), 21)),
                6 => events[0].fields[1].1 = u64::from(u32::MAX) + 1,
                _ => events.push(events[2].clone()),
            }
            assert!(resolve(&events).is_none(), "case {case}");
        }
    }
    #[test]
    fn discard_is_explicit_and_cleanup_after_inframe_is_not_discard() {
        let mut events = fixture();
        events[1].fields[3].1 = 6;
        events.truncate(2);
        assert_eq!(
            resolve(&events).unwrap().1,
            "explicit_precomposition_discard_candidate_unqualified"
        );
        let mut cleanup = fixture();
        let mut discard = cleanup[1].clone();
        discard.timestamp = 160;
        discard.fields[3].1 = 6;
        cleanup.push(discard);
        assert_eq!(
            resolve(&cleanup).unwrap().1,
            "composition_consumed_candidate_display_unknown"
        );
        for state in [2, 4, 5] {
            events[1].fields[3].1 = state;
            assert!(resolve(&events).is_none());
        }
    }
    #[test]
    fn malformed_competing_keys_and_discard_before_inframe_poison_resolution() {
        for case in 0..4 {
            let mut events = fixture();
            match case {
                0 => {
                    let mut bad = events[0].clone();
                    bad.fields.retain(|(n, _)| n != "BindId");
                    events.push(bad);
                }
                1 => {
                    let mut bad = events[2].clone();
                    bad.fields[0].1 = u64::MAX;
                    events.push(bad);
                }
                2 => {
                    let mut discarded = events[1].clone();
                    discarded.timestamp = 125;
                    discarded.fields[3].1 = 6;
                    events.push(discarded);
                }
                _ => events[3].fields[4].1 = u64::MAX,
            }
            assert!(resolve(&events).is_none(), "case {case}");
        }
    }

    #[test]
    fn independent_modes_duplicate_consumption_and_unknown_state_remain_unknown() {
        for case in 0..5 {
            let mut events = fixture();
            match case {
                0 => events[1].fields[4].1 = 1,
                1 => events[2].fields[4].1 = 1,
                2 => events[2].fields[5].1 = 1,
                3 => events.push(events[3].clone()),
                _ => events[1].fields[3].1 = 99,
            }
            assert!(resolve(&events).is_none(), "case {case}");
        }
    }
}
