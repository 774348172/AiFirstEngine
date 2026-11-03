//! Adapter ownership is scoped to native device/context lifetimes, not pointer equality alone.
use super::correlation::{field, nonzero};
use engine_runtime::windowed_player::{WindowsEtwDecodedEvent, WindowsEtwProvider};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct Span {
    start: usize,
    stop: Option<usize>,
    parent: u64,
    pid: u32,
}

#[derive(Clone, Copy)]
struct QueueSpan {
    start: usize,
    stop: Option<usize>,
    context: u64,
    user_handle: u64,
}

#[derive(Default)]
struct DmaSpan {
    submit: Option<usize>,
    complete: Option<usize>,
}

#[derive(Default)]
pub(super) struct Lifetimes {
    invalid: bool,
    devices: BTreeMap<u64, Vec<Span>>,
    contexts: BTreeMap<u64, Vec<Span>>,
    // Lifecycle ParentDxgHwQueue is the kernel identity used by DMA hHwQueue.
    // The lifecycle hHwQueue is a distinct user handle and may legitimately be zero.
    queues: BTreeMap<u64, Vec<QueueSpan>>,
    // Exact kernel queue/generation/fence index, bounded by decoded input.
    dma: BTreeMap<(u64, usize, u64), DmaSpan>,
}

impl Lifetimes {
    // DMA completion is not screen-time evidence. This seam must resolve the
    // kernel queue generation and owning context before consuming such inputs.
    pub(super) fn hardware_dma(
        &self,
        events: &[WindowsEtwDecodedEvent],
        submit: usize,
    ) -> Option<[usize; 5]> {
        let e = events.get(submit)?;
        if self.invalid
            || e.provider != WindowsEtwProvider::DxgKrnl
            || (e.event_id, e.version, e.opcode) != (450, 0, 0)
            || field(e, "ntStatus") != Some(0)
            || nonzero(e, "pDmaBuffer").is_none()
            || field(e, "NumberOfQueuedPendingFlip").is_none_or(|v| v > u64::from(u32::MAX))
        {
            return None;
        }
        let queue = nonzero(e, "hHwQueue")?;
        let fence = nonzero(e, "ProgressFenceValue")?;
        let spans = self.queues.get(&queue)?;
        let at = spans.partition_point(|span| events[span.start].timestamp < e.timestamp);
        let span = spans.get(at.checked_sub(1)?)?;
        let pair = self.dma.get(&(queue, span.start, fence))?;
        if pair.submit != Some(submit) {
            return None;
        }
        let complete = pair.complete?;
        if events[complete].timestamp <= e.timestamp
            || span
                .stop
                .is_some_and(|stop| events[stop].timestamp <= events[complete].timestamp)
        {
            return None;
        }
        let (device_start, context_start) =
            self.hardware_owner(events, span, events[complete].timestamp)?;
        Some([span.start, device_start, context_start, submit, complete])
    }

    fn hardware_owner(
        &self,
        events: &[WindowsEtwDecodedEvent],
        span: &QueueSpan,
        end: i64,
    ) -> Option<(usize, usize)> {
        if self.invalid {
            return None;
        }
        let contexts = self.contexts.get(&span.context)?;
        let mut live = contexts.iter().filter(|context| {
            events[context.start].timestamp < events[span.start].timestamp
                && context.stop.is_none_or(|stop| events[stop].timestamp > end)
        });
        let context = live.next()?;
        if live.next().is_some() {
            return None;
        }
        let devices = self.devices.get(&context.parent)?;
        let mut live = devices.iter().filter(|device| {
            events[device.start].timestamp < events[context.start].timestamp
                && device.stop.is_none_or(|stop| events[stop].timestamp > end)
        });
        let device = live.next()?;
        if live.next().is_some() {
            return None;
        }
        let (device_start, context_start, _) = self.context(
            events,
            span.context,
            device.pid,
            events[span.start].timestamp,
            end,
        )?;
        Some((device_start, context_start))
    }

    pub(super) fn hardware_inputs_complete(&self, events: &[WindowsEtwDecodedEvent]) -> bool {
        if !events.iter().any(|e| {
            e.provider == WindowsEtwProvider::DxgKrnl
                && matches!(e.event_id, 422 | 423 | 424 | 450 | 451)
        }) {
            return true;
        }
        for spans in self.queues.values() {
            for span in spans {
                let Some(end) = span
                    .stop
                    .map(|stop| events[stop].timestamp)
                    .or_else(|| events[span.start].timestamp.checked_add(1))
                else {
                    return false;
                };
                if self.hardware_owner(events, span, end).is_none() {
                    return false;
                }
            }
        }
        let mut completions = std::collections::BTreeSet::new();
        for (i, e) in events.iter().enumerate() {
            if e.provider == WindowsEtwProvider::DxgKrnl && e.event_id == 450 {
                let Some(chain) = self.hardware_dma(events, i) else {
                    return false;
                };
                if !completions.insert(chain[4]) {
                    return false;
                }
            }
        }
        !self.invalid
            && events.iter().enumerate().all(|(i, e)| {
                e.provider != WindowsEtwProvider::DxgKrnl
                    || e.event_id != 451
                    || completions.contains(&i)
            })
    }
    pub(super) fn new(events: &[WindowsEtwDecodedEvent]) -> Self {
        let mut result = Self::default();
        let mut ordered: Vec<_> = (0..events.len()).collect();
        ordered.sort_unstable_by_key(|i| (events[*i].timestamp, *i));
        for i in ordered {
            let e = &events[i];
            if e.provider != WindowsEtwProvider::DxgKrnl {
                continue;
            }
            if matches!((e.event_id, e.version, e.opcode), (450, 0, 0) | (451, 0, 0)) {
                result.record_dma(events, i);
                continue;
            }
            if matches!(
                (e.event_id, e.version, e.opcode),
                (422, 0, 1) | (423, 0, 2) | (424, 0, 3)
            ) {
                let Some(((kernel, context), user_handle)) = nonzero(e, "ParentDxgHwQueue")
                    .zip(nonzero(e, "hContext"))
                    .zip(field(e, "hHwQueue"))
                else {
                    result.invalid = true;
                    continue;
                };
                let spans = result.queues.entry(kernel).or_default();
                if e.event_id == 423 {
                    if let Some(span) = spans.last_mut() {
                        if span.stop.is_some()
                            || span.context != context
                            || span.user_handle != user_handle
                            || events[span.start].timestamp >= e.timestamp
                        {
                            result.invalid = true;
                        } else {
                            span.stop = Some(i);
                        }
                    } else {
                        result.invalid = true;
                    }
                } else if let Some(span) = spans.last().filter(|span| span.stop.is_none()) {
                    if e.event_id != 424
                        || span.context != context
                        || span.user_handle != user_handle
                        || events[span.start].timestamp >= e.timestamp
                    {
                        result.invalid = true;
                    }
                } else if spans.last().is_some_and(|span| {
                    span.stop
                        .is_none_or(|stop| events[stop].timestamp >= e.timestamp)
                }) {
                    result.invalid = true;
                } else {
                    spans.push(QueueSpan {
                        start: i,
                        stop: None,
                        context,
                        user_handle,
                    });
                }
                continue;
            }
            let (map, handle, parent, pid, start) = match (e.event_id, e.version, e.opcode) {
                (27, 2, 1) | (29, 2, 3) | (28, 2, 2) => (
                    &mut result.devices,
                    nonzero(e, "hDevice"),
                    nonzero(e, "pDxgAdapter"),
                    nonzero(e, "hProcessId").and_then(|p| u32::try_from(p).ok()),
                    e.event_id != 28,
                ),
                (30, 0, 1) | (32, 0, 3) | (31, 0, 2) => (
                    &mut result.contexts,
                    nonzero(e, "hContext"),
                    nonzero(e, "hDevice"),
                    Some(0), // Device payload supplies the real PID, including rundown events.
                    e.event_id != 31,
                ),
                _ => continue,
            };
            let Some((handle, parent, pid)) =
                handle.zip(parent).zip(pid).map(|((h, p), id)| (h, p, id))
            else {
                result.invalid = true;
                continue;
            };
            let spans = map.entry(handle).or_default();
            if start {
                if let Some(span) = spans.last().filter(|span| span.stop.is_none()) {
                    // A live create racing CAPTURE_STATE may be followed by the
                    // same object's rundown. Corroborate its original lifetime;
                    // never replace the origin or accept changed ownership/node.
                    if e.opcode == 3
                        && span.parent == parent
                        && span.pid == pid
                        && events[span.start].timestamp <= e.timestamp
                        && field(&events[span.start], "NodeOrdinal") == field(e, "NodeOrdinal")
                    {
                        continue;
                    }
                    result.invalid = true;
                    continue;
                }
                if spans.last().is_some_and(|s| {
                    s.stop
                        .is_none_or(|stop| events[stop].timestamp >= e.timestamp)
                }) {
                    result.invalid = true;
                    continue;
                }
                spans.push(Span {
                    start: i,
                    stop: None,
                    parent,
                    pid,
                });
            } else if let Some(span) = spans.last_mut() {
                if span.stop.is_some()
                    || span.parent != parent
                    || span.pid != pid
                    || events[span.start].timestamp >= e.timestamp
                {
                    result.invalid = true;
                } else {
                    span.stop = Some(i);
                }
            } else {
                // A stop without a start is incomplete lifetime evidence.
                result.invalid = true;
            }
        }
        result
    }

    fn record_dma(&mut self, events: &[WindowsEtwDecodedEvent], index: usize) {
        let event = &events[index];
        let Some((kernel, fence)) =
            nonzero(event, "hHwQueue").zip(nonzero(event, "ProgressFenceValue"))
        else {
            self.invalid = true;
            return;
        };
        let Some(span) = self.queues.get(&kernel).and_then(|spans| spans.last()) else {
            self.invalid = true;
            return;
        };
        if span.stop.is_some() || events[span.start].timestamp >= event.timestamp {
            self.invalid = true;
            return;
        }
        let pair = self.dma.entry((kernel, span.start, fence)).or_default();
        let duplicate = if event.event_id == 450 {
            pair.submit.replace(index).is_some()
        } else {
            pair.complete.replace(index).is_some()
        };
        if duplicate {
            self.invalid = true;
        }
    }

    pub(super) fn context(
        &self,
        events: &[WindowsEtwDecodedEvent],
        handle: u64,
        pid: u32,
        begin: i64,
        end: i64,
    ) -> Option<(usize, usize, u64)> {
        if self.invalid || begin >= end {
            return None;
        }
        let inside = |span: &&Span| {
            events[span.start].timestamp < begin
                && span.stop.is_none_or(|i| events[i].timestamp > end)
        };
        let mut contexts = self.contexts.get(&handle)?.iter().filter(inside);
        let context = contexts.next()?;
        if contexts.next().is_some() {
            return None;
        }
        let mut devices = self.devices.get(&context.parent)?.iter().filter(inside);
        let device = devices.next()?;
        if devices.next().is_some()
            || device.pid != pid
            || events[device.start].timestamp >= events[context.start].timestamp
            || field(&events[context.start], "NodeOrdinal").is_none_or(|v| v > u64::from(u32::MAX))
        {
            return None;
        }
        Some((device.start, context.start, device.parent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: u16, opcode: u8, time: i64, fields: &[(&str, u64)]) -> WindowsEtwDecodedEvent {
        WindowsEtwDecodedEvent {
            provider: WindowsEtwProvider::DxgKrnl,
            event_id: id,
            version: if id == 27 { 2 } else { 0 },
            opcode,
            process_id: 4, // Header PID is not ownership proof for kernel events.
            thread_id: 9,
            timestamp: time,
            fields: fields
                .iter()
                .map(|(name, value)| (name.to_string(), *value))
                .collect(),
        }
    }
    fn hardware() -> Vec<WindowsEtwDecodedEvent> {
        vec![
            event(
                27,
                1,
                10,
                &[("hDevice", 7), ("pDxgAdapter", 1), ("hProcessId", 100)],
            ),
            event(
                30,
                1,
                20,
                &[("hContext", 42), ("hDevice", 7), ("NodeOrdinal", 0)],
            ),
            event(
                422,
                1,
                30,
                &[("hContext", 42), ("hHwQueue", 0), ("ParentDxgHwQueue", 900)],
            ),
            event(
                450,
                0,
                40,
                &[
                    ("hHwQueue", 900),
                    ("ProgressFenceValue", u64::from(u32::MAX) + 8),
                    ("pDmaBuffer", 12),
                    ("ntStatus", 0),
                    ("NumberOfQueuedPendingFlip", 1),
                ],
            ),
            event(
                451,
                0,
                50,
                &[
                    ("hHwQueue", 900),
                    ("ProgressFenceValue", u64::from(u32::MAX) + 8),
                ],
            ),
        ]
    }
    #[test]
    fn hardware_dma_uses_kernel_queue_identity_and_exact_context_lifetime() {
        let events = hardware();
        assert_eq!(
            Lifetimes::new(&events).hardware_dma(&events, 3),
            Some([2, 0, 1, 3, 4])
        );
        assert!(Lifetimes::new(&events).hardware_inputs_complete(&events));
    }

    #[test]
    fn hardware_queue_reuse_is_generation_scoped_and_rundown_keeps_live_origin() {
        let mut events = hardware();
        events.push(event(
            424,
            3,
            35,
            &[("hContext", 42), ("hHwQueue", 0), ("ParentDxgHwQueue", 900)],
        ));
        events.push(event(
            423,
            2,
            60,
            &[("hContext", 42), ("hHwQueue", 0), ("ParentDxgHwQueue", 900)],
        ));
        events.push(event(
            422,
            1,
            70,
            &[
                ("hContext", 42),
                ("hHwQueue", 13),
                ("ParentDxgHwQueue", 900),
            ],
        ));
        let mut start = events[3].clone();
        start.timestamp = 80;
        events.push(start);
        let mut end = events[4].clone();
        end.timestamp = 90;
        events.push(end);
        let life = Lifetimes::new(&events);
        assert_eq!(life.hardware_dma(&events, 3), Some([2, 0, 1, 3, 4]));
        assert_eq!(life.hardware_dma(&events, 8), Some([7, 0, 1, 8, 9]));
        assert!(life.hardware_inputs_complete(&events));
    }

    #[test]
    fn hardware_dma_rejects_missing_duplicate_faulted_or_boundary_inputs() {
        for case in 0..13 {
            let mut events = hardware();
            match case {
                0 => {
                    events.remove(2);
                }
                1 => {
                    events[3].fields[0].1 = 0;
                }
                2 => {
                    events[3].fields[1].1 = 0;
                }
                3 => {
                    events[3].fields[2].1 = 0;
                }
                4 => {
                    events[3].fields[3].1 = 0xc0000001;
                }
                5 => {
                    events[4].fields[1].1 += 1;
                }
                6 => {
                    events.push(events[3].clone());
                }
                7 => {
                    events.push(events[4].clone());
                }
                8 => {
                    events[4].timestamp = 40;
                }
                9 => {
                    events.push(event(
                        423,
                        2,
                        50,
                        &[("hContext", 42), ("hHwQueue", 0), ("ParentDxgHwQueue", 900)],
                    ));
                }
                10 => {
                    events.push(event(
                        31,
                        2,
                        45,
                        &[("hContext", 42), ("hDevice", 7), ("NodeOrdinal", 0)],
                    ));
                }
                11 => {
                    events[2].fields[0].1 = 43;
                }
                12 => {
                    events[3].fields.push(("ProgressFenceValue".into(), 7));
                }
                _ => unreachable!(),
            }
            assert!(
                !Lifetimes::new(&events).hardware_inputs_complete(&events),
                "case={case}"
            );
        }
    }

    #[test]
    fn hardware_rundown_and_stop_cannot_change_context_or_user_handle() {
        for (id, opcode) in [(424, 3), (423, 2)] {
            for field_index in [0, 1] {
                let mut events = hardware();
                let mut e = event(
                    id,
                    opcode,
                    35,
                    &[("hContext", 42), ("hHwQueue", 0), ("ParentDxgHwQueue", 900)],
                );
                e.fields[field_index].1 += 1;
                events.push(e);
                assert!(!Lifetimes::new(&events).hardware_inputs_complete(&events));
            }
        }
    }
}
