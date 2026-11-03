//! Offline reader for our stopped ETL. Named TDH properties avoid payload-offset guesses.
use engine_runtime::windowed_player::{
    WindowsEtwDecodeReport, WindowsEtwDecodedEvent, WindowsEtwProvider,
    WindowsEtwUnsupportedEventKind,
};
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::System::Diagnostics::Etw::*;

// Trace-only decoded storage, including field/name allocations and Vec growth.
// Small events are not truncated at an unrelated fixed record count.
pub(super) const MAX_DECODED_BYTES: usize = 8 * 1024 * 1024;
const MAX_EVENTS: u64 = 1_000_000;
const MAX_UNSUPPORTED_KINDS: usize = 16;
// Concurrent mixed-width TDH reads of one descriptor returned inconsistent
// offsets in owner tests. Serialize size/value pairs within this owner;
// the internal Windows cache mechanism is not part of our contract.
static TDH_PROPERTIES: std::sync::Mutex<()> = std::sync::Mutex::new(());

// These exact local-manifest tuples describe DMA execution or node metadata.
// They do not participate in history handoff or the legacy flip candidate chain.
// In particular, queue waits, device/context lifetimes, MPO and other versions
// are NOT covered by this exclusion. This is not a full display/GPU-time policy.
fn irrelevant(id: u16, version: u8, opcode: u8) -> bool {
    matches!(
        (id, version, opcode),
        (175, 1, 1) | (176, 0, 2) | (177, 1, 0) | (250, 0, 0)
    )
}

// Exact event/version/opcode tuples from the retained local DxgKrnl manifest.
// No event here, including VSyncDPC, proves which application frame was displayed.
fn fields(id: u16, version: u8, opcode: u8) -> Option<&'static [&'static str]> {
    match (id, version, opcode) {
        (422, 0, 1) | (423, 0, 2) | (424, 0, 3) => {
            Some(&["hContext", "hHwQueue", "ParentDxgHwQueue"])
        }
        (450, 0, 0) => Some(&[
            "hHwQueue",
            "ProgressFenceValue",
            "pDmaBuffer",
            "ntStatus",
            "NumberOfQueuedPendingFlip",
        ]),
        (451, 0, 0) => Some(&["hHwQueue", "ProgressFenceValue"]),
        (252, 0, 0) => Some(&[
            "VidPnSourceId",
            "LayerIndex",
            "Enabled",
            "hAllocation",
            "Flags",
        ]),
        (27, 2, 1) | (28, 2, 2) | (29, 2, 3) => Some(&["hProcessId", "pDxgAdapter", "hDevice"]),
        (30, 0, 1) | (31, 0, 2) | (32, 0, 3) => Some(&["hDevice", "hContext", "NodeOrdinal"]),
        (266, 3, 0) => Some(&[
            "hContext",
            "SubmitSequence",
            "VidPnSourceId",
            "FlipInterval",
            "PlaneIndex",
            "CompositionSurfaceLuid",
            "BindId",
            "PresentCount",
            "FlipTrueImmediate",
            "SuppressVSync",
        ]),
        (259, 3, 0) => Some(&[
            "pDxgAdapter",
            "VidPnSourceId",
            "LayerIndex",
            "FlipSubmitSequence",
            "FlipEntryStatusAfterFlip",
            "Enabled",
        ]),
        (273, 4, 0) => Some(&[
            "pDxgAdapter",
            "VidPnSourceId",
            "FrameNumber",
            "PlaneCount",
            "FlipEntryCount",
            "FlipQueues",
        ]),
        (382, 2, 0) => Some(&[
            "pDxgAdapter",
            "VidPnSourceId",
            "FrameNumber",
            "PlaneCount",
            "FlipEntryCount",
        ]),
        (386, 8, 0) => Some(&[
            "pDxgAdapter",
            "VidPnSourceId",
            "PlaneCount",
            "InputFlags",
            "OutputFlags",
            "FlipDoNotFlip",
            "FlipIntervalTarget",
            "FlipSubmitSequence",
            "FlipPresentId",
        ]),
        (171, 2, 1) | (172, 2, 0) | (215, 2, 1) => Some(&["hAdapter", "Token", "Model"]),
        (116, 0, 0) => Some(&[
            "pDxgAdapter",
            "VidPnSourceId",
            "FlipSubmitSequence",
            "Flags",
        ]),
        (168, 0, 0) => Some(&["VidPnSourceId", "FlipInterval", "MMIOFlip"]),
        (178, 1, 1) => Some(&["hContext", "PacketType", "SubmitSequence", "bPresent"]),
        (244, 1 | 2, 1) => Some(&["hContext", "SubmitSequence"]),
        (180, 1, 2) => Some(&["hContext", "SubmitSequence", "bPreempted", "bTimeouted"]),
        (184, 1, 0) => Some(&["hContext", "hWindow", "ReturnStatus"]),
        (17, 0, 0) => Some(&[
            "pDxgAdapter",
            "VidPnSourceId",
            "VidPnTargetId",
            "FrameNumber",
            "FrameQPCTime",
            "hFlipDevice",
            "FlipFenceId",
        ]),
        _ => None,
    }
}

fn provider_fields(
    provider: WindowsEtwProvider,
    id: u16,
    version: u8,
    opcode: u8,
) -> Option<&'static [&'static str]> {
    match provider {
        WindowsEtwProvider::DxgKrnl => fields(id, version, opcode),
        WindowsEtwProvider::Win32k => match (id, version, opcode) {
            (201, 1, 0) => Some(&["CompositionSurfaceLuid", "PresentCount", "BindId"]),
            (301, 1, 0) => Some(&[
                "CompositionSurfaceLuid",
                "PresentCount",
                "BindId",
                "NewState",
                "IndependentFlip",
            ]),
            _ => None,
        },
        WindowsEtwProvider::DwmCore => match (id, version, opcode) {
            (15, 0, 1) => Some(&["tCurrent", "tPresent", "cRefreshCurrent", "cRefreshPresent"]),
            (196, 0, 0) => Some(&[
                "luidSurface.lowpart",
                "luidSurface.highpart",
                "bindId",
                "PresentCount",
                "bDirectFlip",
                "bIndependentFlip",
            ]),
            (467 | 468, 0, 0) => Some(&[
                "surfaceLuid",
                "bindId",
                "presentCount",
                "displayAdapterLuid",
                "renderAdapterLuid",
                "vidPnSourceId",
                "vidPnTargetId",
                "uniqueId",
            ]),
            (476, 0, 0) => Some(&[
                "surfaceLuid",
                "bindId",
                "presentCount",
                "displayAdapterLuid",
                "renderAdapterLuid",
                "vidPnSourceId",
                "vidPnTargetId",
                "uniqueId",
                "DirectFlipMode",
                "presentFlags",
                "syncInterval",
            ]),
            _ => None,
        },
    }
}

fn integer(bytes: &[u8]) -> Option<u64> {
    match bytes.len() {
        4 => Some(u64::from(u32::from_le_bytes(bytes.try_into().ok()?))),
        8 => Some(u64::from_le_bytes(bytes.try_into().ok()?)),
        _ => None,
    }
}

// Only exact manifest arrays are consumed. Limits bound per-event property work,
// including unrelated system planes; zero count is valid and never means display.
type ArrayLayout = (&'static str, u64, &'static [&'static str]);

pub(super) fn arrays(id: u16, version: u8, opcode: u8) -> &'static [ArrayLayout] {
    match (id, version, opcode) {
        (273, 4, 0) => &[
            (
                "PlaneCount",
                16,
                &["PresentIdOrPhysicalAddress", "InterruptTargetPresentId"],
            ),
            ("FlipEntryCount", 64, &["FlipSubmitSequence"]),
            ("FlipQueues", 16, &["FlipQueueIntervalTarget"]),
        ],
        (382, 2, 0) => &[
            (
                "PlaneCount",
                16,
                &["ScannedPhysicalAddress", "InterruptTargetPresentId"],
            ),
            ("FlipEntryCount", 64, &["FlipSubmitSequence"]),
        ],
        (386, 8, 0) => &[("PlaneCount", 16, &["PresentId", "LayerIndex"])],
        _ => &[],
    }
}

#[derive(Default)]
struct Consumer {
    report: WindowsEtwDecodeReport,
    decoded_bytes: usize,
}

fn event_bytes(event: &WindowsEtwDecodedEvent) -> usize {
    // Twice the record size covers unused geometric-growth capacity of events.
    event.fields.iter().fold(
        (2 * std::mem::size_of::<WindowsEtwDecodedEvent>()).saturating_add(
            event
                .fields
                .capacity()
                .saturating_mul(std::mem::size_of::<(String, u64)>()),
        ),
        |bytes, (name, _)| bytes.saturating_add(name.capacity()),
    )
}

pub(super) fn decoded_input_bytes(events: &[WindowsEtwDecodedEvent]) -> usize {
    events.iter().fold(0usize, |bytes, event| {
        bytes.saturating_add(event_bytes(event))
    })
}

impl Consumer {
    fn finish(mut self) -> WindowsEtwDecodeReport {
        self.report.status =
            if self.report.process_error.is_some() || self.report.close_error.is_some() {
                "read_failed_no_attribution"
            } else if self.incomplete() || self.report.buffers_lost != 0 {
                "partial_inputs_no_attribution"
            } else if self.report.events.is_empty() {
                "no_supported_events_no_attribution"
            } else {
                "decoded_inputs_no_attribution"
            }
            .into();
        self.report
    }

    fn incomplete(&self) -> bool {
        self.report.omitted_events != 0
            || self.report.property_errors != 0
            || self.report.unsupported_events != 0
            || self.report.event_budget_reached
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn accept(
        &mut self,
        id: u16,
        version: u8,
        opcode: u8,
        pid: u32,
        tid: u32,
        timestamp: i64,
        property: impl FnMut(&str) -> Result<u64, u32>,
    ) {
        self.accept_provider(
            WindowsEtwProvider::DxgKrnl,
            (id, version, opcode),
            pid,
            tid,
            timestamp,
            property,
        );
    }

    fn accept_provider(
        &mut self,
        provider: WindowsEtwProvider,
        descriptor: (u16, u8, u8),
        pid: u32,
        tid: u32,
        timestamp: i64,
        mut property: impl FnMut(&str) -> Result<u64, u32>,
    ) {
        let (id, version, opcode) = descriptor;
        if provider == WindowsEtwProvider::DxgKrnl && irrelevant(id, version, opcode) {
            self.report.ignored_irrelevant_events += 1;
            return;
        }
        let Some(names) = provider_fields(provider, id, version, opcode) else {
            self.report.unsupported_events += 1;
            if let Some(kind) = self.report.unsupported_event_kinds.iter_mut().find(|kind| {
                (kind.provider, kind.event_id, kind.version, kind.opcode)
                    == (provider, id, version, opcode)
            }) {
                kind.count += 1;
            } else if self.report.unsupported_event_kinds.len() < MAX_UNSUPPORTED_KINDS {
                self.report
                    .unsupported_event_kinds
                    .push(WindowsEtwUnsupportedEventKind {
                        provider,
                        event_id: id,
                        version,
                        opcode,
                        count: 1,
                    });
            } else {
                self.report.unlisted_unsupported_events += 1;
            }
            return;
        };
        if self.decoded_bytes >= MAX_DECODED_BYTES {
            self.report.omitted_events += 1;
            return;
        }
        let mut values = Vec::with_capacity(names.len());
        for name in names {
            match property(name) {
                Ok(value) => values.push(((*name).into(), value)),
                Err(error) => {
                    self.report.property_errors += 1;
                    self.report.first_property_error.get_or_insert(error);
                    return;
                }
            }
        }
        if provider == WindowsEtwProvider::DxgKrnl {
            for &(count_name, max, names) in arrays(id, version, opcode) {
                let count = values
                    .iter()
                    .find(|(n, _)| n == count_name)
                    .map(|(_, v)| *v);
                let Some(count) = count.filter(|n| *n <= max) else {
                    self.report.property_errors += 1;
                    self.report.first_property_error.get_or_insert(13);
                    return;
                };
                for name in names {
                    for index in 0..count {
                        let name = format!("{name}[{index}]");
                        match property(&name) {
                            Ok(value) => values.push((name, value)),
                            Err(error) => {
                                self.report.property_errors += 1;
                                self.report.first_property_error.get_or_insert(error);
                                return;
                            }
                        }
                    }
                }
            }
        }
        let event = WindowsEtwDecodedEvent {
            provider,
            event_id: id,
            version,
            opcode,
            process_id: pid,
            thread_id: tid,
            timestamp,
            fields: values,
        };
        let bytes = self.decoded_bytes.saturating_add(event_bytes(&event));
        if bytes > MAX_DECODED_BYTES {
            self.report.omitted_events += 1;
            return;
        }
        self.decoded_bytes = bytes;
        self.report.events.push(event);
    }
}

fn property(event: &EVENT_RECORD, name: &str) -> Result<u64, u32> {
    let _guard = TDH_PROPERTIES.lock().map_err(|_| 13u32)?;
    let (name, array_index) = if let Some(indexed) = name.strip_suffix(']') {
        let (name, index) = indexed.rsplit_once('[').ok_or(13u32)?;
        let index = index.parse::<u32>().map_err(|_| 13u32)?;
        if index >= 64 {
            return Err(13);
        }
        (name, index)
    } else {
        (name, u32::MAX)
    };
    // TDH requires parent+member descriptors for a struct. A dotted name passed
    // as one scalar descriptor cannot resolve DWM's luidSurface members.
    let names: Vec<Vec<u16>> = name
        .split('.')
        .map(|part| part.encode_utf16().chain(Some(0)).collect())
        .collect();
    let descriptors: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, part)| PROPERTY_DATA_DESCRIPTOR {
            PropertyName: part.as_ptr() as u64,
            ArrayIndex: if i + 1 < names.len() { 0 } else { array_index },
            Reserved: 0,
        })
        .collect();
    let mut size = 0;
    // SAFETY: callback-owned EVENT_RECORD remains valid through both synchronous TDH
    // calls. Descriptor points to a NUL-terminated name alive for their duration.
    let result = unsafe {
        TdhGetPropertySize(
            event,
            0,
            std::ptr::null(),
            descriptors.len() as u32,
            descriptors.as_ptr(),
            &mut size,
        )
    };
    if result != 0 {
        return Err(result);
    }
    if size != 4 && size != 8 {
        return Err(13);
    } // ERROR_INVALID_DATA
    let mut bytes = [0; 8];
    // SAFETY: size is checked against this fixed output buffer; no payload is cast.
    let result = unsafe {
        TdhGetProperty(
            event,
            0,
            std::ptr::null(),
            descriptors.len() as u32,
            descriptors.as_ptr(),
            size,
            bytes.as_mut_ptr(),
        )
    };
    if result != 0 {
        return Err(result);
    }
    integer(&bytes[..size as usize]).ok_or(13)
}

unsafe extern "system" fn on_event(event: *mut EVENT_RECORD) {
    if event.is_null() {
        return;
    }
    // SAFETY: ProcessTrace supplies the event and the stack-owned Context assigned
    // in read(); callbacks run synchronously and neither pointer escapes this call.
    let event = unsafe { &*event };
    if event.UserContext.is_null() {
        return;
    }
    let c = unsafe { &mut *event.UserContext.cast::<Consumer>() };
    if c.report.events_seen >= MAX_EVENTS {
        c.report.event_budget_reached = true;
        return;
    }
    c.report.events_seen += 1;
    let h = &event.EventHeader;
    let Some(provider) = super::provider_from_guid(&h.ProviderId) else {
        return;
    };
    let d = h.EventDescriptor;
    c.accept_provider(
        provider,
        (d.Id, d.Version, d.Opcode),
        h.ProcessId,
        h.ThreadId,
        h.TimeStamp,
        |name| property(event, name),
    );
}

unsafe extern "system" fn on_buffer(log: *mut EVENT_TRACE_LOGFILEW) -> u32 {
    if log.is_null() {
        return 0;
    }
    // SAFETY: same synchronous ProcessTrace lifetime as on_event.
    let log = unsafe { &*log };
    if log.Context.is_null() {
        return 0;
    }
    let c = unsafe { &mut *log.Context.cast::<Consumer>() };
    u32::from(!c.report.event_budget_reached)
}

struct TraceHandle(PROCESSTRACE_HANDLE);
impl Drop for TraceHandle {
    fn drop(&mut self) {
        // SAFETY: only a successful OpenTrace handle is owned, and it is closed once.
        unsafe {
            CloseTrace(self.0);
        }
    }
}

pub(super) fn read(path: &std::path::Path) -> WindowsEtwDecodeReport {
    let mut c = Consumer::default();
    let mut name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if name[..name.len() - 1].contains(&0) {
        c.report.status = "invalid_path".into();
        return c.report;
    }
    // SAFETY: zero is valid for this C POD; all used callbacks/mode/context are set.
    let mut log: EVENT_TRACE_LOGFILEW = unsafe { std::mem::zeroed() };
    log.LogFileName = name.as_mut_ptr();
    log.Anonymous1.ProcessTraceMode =
        PROCESS_TRACE_MODE_EVENT_RECORD | PROCESS_TRACE_MODE_RAW_TIMESTAMP;
    log.Anonymous2.EventRecordCallback = Some(on_event);
    log.BufferCallback = Some(on_buffer);
    log.Context = (&mut c as *mut Consumer).cast();
    // SAFETY: filename, logfile and context remain alive until ProcessTrace returns.
    let handle = unsafe { OpenTraceW(&mut log) };
    if handle.Value == u64::MAX {
        c.report.open_error = Some(unsafe { windows_sys::Win32::Foundation::GetLastError() });
        c.report.status = "open_failed".into();
        return c.report;
    }
    let handle = TraceHandle(handle);
    c.report.trace_qpc_frequency =
        (log.LogfileHeader.PerfFreq > 0).then_some(log.LogfileHeader.PerfFreq);
    c.report.clock_type = log.LogfileHeader.ReservedFlags;
    c.report.buffers_lost = log.LogfileHeader.BuffersLost;
    // SAFETY: single owned offline handle. No real-time/session mutation; all callback
    // state remains alive. Native file I/O has no strict wall-clock timeout guarantee.
    let error = unsafe { ProcessTrace(&handle.0, 1, std::ptr::null(), std::ptr::null()) };
    if error != 0 {
        c.report.process_error = Some(error);
    }
    // SAFETY: processing has ended, this closes the single owned reader handle.
    let error = unsafe { CloseTrace(handle.0) };
    std::mem::forget(handle); // Explicit close was attempted; never close another owner's handle.
    if error != 0 {
        c.report.close_error = Some(error);
    }
    c.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    // Above the retired count limit; irrelevant inputs must never consume bytes.
    const IRRELEVANT_REPEATS: usize = 5000;

    #[test]
    fn captured_hardware_queue_dma_and_mpo_schemas_preserve_exact_keys() {
        let mut c = Consumer::default();
        for (id, opcode, names) in [
            (422, 1, &["hContext", "hHwQueue", "ParentDxgHwQueue"][..]),
            (423, 2, &["hContext", "hHwQueue", "ParentDxgHwQueue"][..]),
            (424, 3, &["hContext", "hHwQueue", "ParentDxgHwQueue"][..]),
            (
                450,
                0,
                &[
                    "hHwQueue",
                    "ProgressFenceValue",
                    "pDmaBuffer",
                    "ntStatus",
                    "NumberOfQueuedPendingFlip",
                ][..],
            ),
            (451, 0, &["hHwQueue", "ProgressFenceValue"][..]),
            (
                252,
                0,
                &[
                    "VidPnSourceId",
                    "LayerIndex",
                    "Enabled",
                    "hAllocation",
                    "Flags",
                ][..],
            ),
        ] {
            c.accept(id, 0, opcode, 4, 9, 123, |name| {
                assert!(names.contains(&name));
                Ok(7)
            });
            assert_eq!(c.report.events.last().map(|e| e.event_id), Some(id));
            assert_eq!(c.report.events.last().unwrap().fields.len(), names.len());
        }
        assert_eq!(c.report.unsupported_events, 0);
        assert_eq!(c.report.ignored_irrelevant_events, 0);
        assert_eq!(c.report.property_errors, 0);
        // Decoder coverage is not a hardware-queue or screen-time attribution.
        assert_eq!(c.finish().status, "decoded_inputs_no_attribution");
    }

    #[test]
    fn native_hardware_queue_dma_and_mpo_tdh_preserves_both_pointer_widths() {
        for (flags, width) in [
            (EVENT_HEADER_FLAG_32_BIT_HEADER, 4),
            (EVENT_HEADER_FLAG_64_BIT_HEADER, 8),
        ] {
            for (id, opcode) in [(422, 1), (423, 2), (424, 3), (450, 0), (451, 0), (252, 0)] {
                let mut bytes = vec![0u8; 128];
                let write = |bytes: &mut [u8], at: usize, size: usize, value: u64| {
                    bytes[at..at + size].copy_from_slice(&value.to_le_bytes()[..size]);
                };
                match id {
                    422..=424 => {
                        write(&mut bytes, 0, width, 11);
                        write(&mut bytes, width, width, 12);
                        write(&mut bytes, width * 2, width, 13);
                    }
                    450 | 451 => {
                        write(&mut bytes, 0, width, 12);
                        write(&mut bytes, width, 8, u64::from(u32::MAX) + 1);
                        if id == 450 {
                            write(&mut bytes, width + 8, width, 14);
                            write(&mut bytes, width * 2 + 8, 4, 0xc0000001);
                            write(&mut bytes, width * 2 + 12, 4, 2);
                        }
                    }
                    252 => {
                        write(&mut bytes, 0, 4, 1);
                        write(&mut bytes, 4, 4, 2);
                        write(&mut bytes, 8, 4, 1);
                        write(&mut bytes, 12, width, 15);
                        write(&mut bytes, 12 + width, 4, 16);
                    }
                    _ => unreachable!(),
                }
                // SAFETY: manifest-layout test payload and callback context remain
                // alive during synchronous TDH reads; production never casts bytes.
                let mut event: EVENT_RECORD = unsafe { std::mem::zeroed() };
                event.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
                event.EventHeader.Flags = flags as u16;
                event.EventHeader.ProviderId = super::super::DXGKRNL;
                event.EventHeader.EventDescriptor.Id = id;
                event.EventHeader.EventDescriptor.Opcode = opcode;
                event.UserData = bytes.as_mut_ptr().cast();
                event.UserDataLength = bytes.len() as u16;
                let mut c = Consumer::default();
                event.UserContext = (&mut c as *mut Consumer).cast();
                unsafe {
                    on_event(&mut event);
                }
                assert_eq!(c.report.property_errors, 0, "id={id} width={width}");
                assert_eq!(c.report.events.len(), 1);
                let fields = &c.report.events[0].fields;
                if matches!(id, 450 | 451) {
                    assert!(
                        fields.contains(&("ProgressFenceValue".into(), u64::from(u32::MAX) + 1))
                    );
                }
                if id == 450 {
                    assert!(fields.contains(&("ntStatus".into(), 0xc0000001)));
                }
                if id == 252 {
                    assert!(fields.contains(&("hAllocation".into(), 15)));
                }
                event.UserDataLength = 1;
                assert!(property(&event, fields[0].0.as_str()).is_err());
            }
        }
    }

    #[test]
    fn decoded_small_events_do_not_truncate_at_the_old_record_count() {
        let mut c = Consumer::default();
        for timestamp in 0..5000 {
            c.accept(116, 0, 0, 4, 9, timestamp, |_| Ok(7));
        }
        assert_eq!(
            c.report.events.len(),
            5000,
            "complete small ETL inputs were truncated solely by record count"
        );
        assert_eq!(c.report.omitted_events, 0);
        assert!(!c.incomplete());
    }

    #[test]
    #[ignore = "reads an existing stopped role-owned ETL; no trace or elevated state is created"]
    fn retained_stopped_etl_decodes_without_truncation_or_unknown_schema() {
        let path = std::path::PathBuf::from(std::env::var_os("AIFE_STOPPED_ETL").unwrap());
        let evidence =
            std::path::PathBuf::from(std::env::var_os("AIFE_ETL_DECODE_REPORT").unwrap());
        assert!(!evidence.exists());
        let r = read(&path);
        std::fs::write(&evidence, serde_json::to_vec_pretty(&r).unwrap()).unwrap();
        assert_eq!(r.open_error, None);
        assert_eq!(r.process_error, None);
        assert_eq!(r.close_error, None);
        assert_eq!(r.property_errors, 0);
        assert_eq!(r.unsupported_events, 0);
        assert_eq!(r.omitted_events, 0);
        assert!(!r.event_budget_reached);
        assert_eq!(r.buffers_lost, 0);
        assert_eq!(r.status, "decoded_inputs_no_attribution");
    }

    #[test]
    fn native_win32k_token_is_preserved_with_provider_identity() {
        let mut c = Consumer::default();
        let mut data = vec![0u8; 64];
        // x64 manifest: two pointers, swapchain u32, then three unaligned u64.
        data[20..28].copy_from_slice(&31u64.to_le_bytes());
        data[28..36].copy_from_slice(&11u64.to_le_bytes());
        data[36..44].copy_from_slice(&21u64.to_le_bytes());
        let mut e: EVENT_RECORD = unsafe { std::mem::zeroed() };
        e.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
        e.EventHeader.ProviderId =
            windows_sys::core::GUID::from_u128(0x8c416c79_d49b_4f01_a467_e56d3aa8234c);
        e.EventHeader.Flags = EVENT_HEADER_FLAG_64_BIT_HEADER as u16;
        e.EventHeader.EventDescriptor.Id = 201;
        e.EventHeader.EventDescriptor.Version = 1;
        e.UserData = data.as_mut_ptr().cast();
        e.UserDataLength = data.len() as u16;
        e.UserContext = (&mut c as *mut Consumer).cast();
        unsafe {
            on_event(&mut e);
        }
        assert_eq!(c.report.events.len(), 1);
        assert_eq!(
            c.report.events[0].provider,
            engine_runtime::windowed_player::WindowsEtwProvider::Win32k
        );
        assert!(c.report.events[0]
            .fields
            .contains(&("CompositionSurfaceLuid".into(), 11)));
    }

    #[test]
    fn known_dma_and_node_metadata_do_not_invalidate_candidate_inputs() {
        let mut c = Consumer::default();
        for (id, version, opcode) in [(175, 1, 1), (176, 0, 2), (177, 1, 0), (250, 0, 0)] {
            c.accept(id, version, opcode, 4, 9, 123, |_| {
                panic!("irrelevant payload")
            });
        }
        assert_eq!(c.report.unsupported_events, 0);
        assert_eq!(c.report.ignored_irrelevant_events, 4);
        assert!(!c.incomplete());
        assert!(c.report.events.is_empty());
    }

    #[test]
    fn queue_wait_versions_preserve_keys_instead_of_being_ignored() {
        let mut c = Consumer::default();
        for version in [1, 2] {
            c.accept(244, version, 1, 4, 9, 123, |name| match name {
                "hContext" => Ok(42),
                "SubmitSequence" => Ok(8),
                _ => panic!("not a present packet"),
            });
        }
        assert_eq!(c.report.events.len(), 2);
        assert_eq!(c.report.unsupported_events, 0);
        assert!(c.report.events.iter().all(|e| e.fields.len() == 2));
    }

    #[test]
    fn irrelevant_exclusion_is_exact_and_never_hides_display_or_lifecycle_events() {
        let mut c = Consumer::default();
        for (id, version, opcode) in [
            (175, 0, 1),
            (176, 1, 2),
            (177, 1, 1),
            (250, 1, 0),
            (244, 3, 1),
            (244, 1, 0),
            (450, 1, 0),
            (451, 0, 1),
            (30, 1, 1),
            (31, 1, 2),
            (252, 1, 0),
            (266, 4, 0),
        ] {
            c.accept(id, version, opcode, 4, 9, 123, |_| {
                panic!("unsupported payload")
            });
        }
        assert_eq!(c.report.unsupported_events, 12);
        assert_eq!(c.report.ignored_irrelevant_events, 0);
        assert!(c.incomplete());
    }

    #[test]
    fn unknown_kind_evidence_is_bounded_and_counts_retained_repeats_after_overflow() {
        let mut c = Consumer::default();
        for id in 1000..1018 {
            c.accept(id, 1, 0, 4, 9, 123, |_| panic!("unknown payload"));
        }
        for (id, version, opcode) in [(1000, 1, 0), (1017, 1, 0), (1000, 2, 0)] {
            c.accept(id, version, opcode, 4, 9, 123, |_| {
                panic!("unknown payload")
            });
        }
        assert_eq!(c.report.unsupported_events, 21);
        assert_eq!(c.report.unsupported_event_kinds.len(), 16);
        assert_eq!(c.report.unsupported_event_kinds[0].count, 2);
        assert_eq!(c.report.unlisted_unsupported_events, 4);
        assert_eq!(
            c.report
                .unsupported_event_kinds
                .iter()
                .map(|k| k.count)
                .sum::<u64>()
                + c.report.unlisted_unsupported_events,
            c.report.unsupported_events
        );
        assert!(c.incomplete());
    }

    #[test]
    fn legacy_decode_json_defaults_classification_evidence() {
        let mut old = serde_json::to_value(WindowsEtwDecodeReport::default()).unwrap();
        for key in [
            "ignoredIrrelevantEvents",
            "unsupportedEventKinds",
            "unlistedUnsupportedEvents",
        ] {
            old.as_object_mut().unwrap().remove(key);
        }
        let decoded: WindowsEtwDecodeReport = serde_json::from_value(old).unwrap();
        assert_eq!(decoded.ignored_irrelevant_events, 0);
        assert!(decoded.unsupported_event_kinds.is_empty());
        assert_eq!(decoded.unlisted_unsupported_events, 0);
    }

    #[test]
    fn native_wait_tdh_resolves_both_versions_and_pointer_widths() {
        for pointer_size in [4usize, 8] {
            for version in [1, 2] {
                let mut data = Vec::new();
                data.extend_from_slice(&42u64.to_le_bytes()[..pointer_size]);
                data.extend_from_slice(&8u32.to_le_bytes());
                data.extend_from_slice(&0u32.to_le_bytes());
                data.extend(vec![0; pointer_size + 8 + pointer_size]);
                if version == 2 {
                    data.extend(vec![0; pointer_size]);
                }
                // SAFETY: zero-valid POD and live payload/context for synchronous TDH.
                let mut e: EVENT_RECORD = unsafe { std::mem::zeroed() };
                let mut c = Consumer::default();
                e.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
                e.EventHeader.ProviderId = super::super::DXGKRNL;
                e.EventHeader.Flags = if pointer_size == 4 {
                    EVENT_HEADER_FLAG_32_BIT_HEADER as u16
                } else {
                    EVENT_HEADER_FLAG_64_BIT_HEADER as u16
                };
                e.EventHeader.EventDescriptor.Id = 244;
                e.EventHeader.EventDescriptor.Version = version;
                e.EventHeader.EventDescriptor.Opcode = 1;
                e.UserData = data.as_mut_ptr().cast();
                e.UserDataLength = data.len() as u16;
                e.UserContext = (&mut c as *mut Consumer).cast();
                unsafe {
                    on_event(&mut e);
                }
                assert_eq!(c.report.property_errors, 0);
                assert_eq!(c.report.events.len(), 1);
                assert_eq!(
                    c.report.events[0].fields,
                    [("hContext".into(), 42), ("SubmitSequence".into(), 8)]
                );
                e.UserDataLength = 1;
                unsafe {
                    on_event(&mut e);
                }
                assert_eq!(c.report.events.len(), 1);
                assert_eq!(c.report.property_errors, 1);
                assert!(c.incomplete());
            }
        }
    }

    #[test]
    fn native_irrelevant_callback_respects_provider_and_global_event_budget() {
        let mut c = Consumer::default();
        // SAFETY: live callback context; irrelevant/wrong-provider paths read no payload.
        let mut e: EVENT_RECORD = unsafe { std::mem::zeroed() };
        e.UserContext = (&mut c as *mut Consumer).cast();
        e.EventHeader.EventDescriptor.Id = 175;
        e.EventHeader.EventDescriptor.Version = 1;
        e.EventHeader.EventDescriptor.Opcode = 1;
        unsafe {
            on_event(&mut e);
        }
        assert_eq!(c.report.ignored_irrelevant_events, 0);
        e.EventHeader.ProviderId = super::super::DXGKRNL;
        for _ in 0..IRRELEVANT_REPEATS {
            unsafe {
                on_event(&mut e);
            }
        }
        assert_eq!(
            c.report.ignored_irrelevant_events,
            IRRELEVANT_REPEATS as u64
        );
        assert_eq!(c.report.omitted_events, 0);
        assert!(c.report.events.is_empty());
        assert!(!c.incomplete());
        c.report.events_seen = MAX_EVENTS;
        unsafe {
            on_event(&mut e);
        }
        assert!(c.incomplete());
        assert_eq!(
            c.report.ignored_irrelevant_events,
            IRRELEVANT_REPEATS as u64
        );
    }

    #[test]
    fn classified_decode_flows_into_frozen_parent_report_without_promoting_display() {
        use engine_runtime::windowed_player::{
            NativePresentFrameObservation, NativePresentReport, WindowsEtwCaptureReport,
        };
        for unknown in [false, true] {
            let mut c = Consumer::default();
            c.report.clock_type = 1;
            c.report.trace_qpc_frequency = Some(10_000_000);
            for (id, opcode, pid, time) in [(171, 1, 100, 105), (172, 0, 4, 130)] {
                c.accept(id, 2, opcode, pid, 9, time, |name| {
                    Ok(match name {
                        "hAdapter" => 1,
                        "Token" => 7,
                        "Model" => 2,
                        _ => unreachable!(),
                    })
                });
            }
            for _ in 0..IRRELEVANT_REPEATS {
                c.accept(175, 1, 1, 4, 9, 110, |_| panic!("irrelevant payload"));
            }
            if unknown {
                c.accept(252, 1, 0, 100, 9, 110, |_| {
                    panic!("unsupported MPO version")
                });
            }
            let capture = WindowsEtwCaptureReport {
                status: "raw_capture_unresolved".into(),
                cleanup_status: "stopped".into(),
                process_id: 100,
                events_lost: Some(0),
                log_buffers_lost: Some(0),
                qpc_frequency: Some(10_000_000),
                decoded: Some(c.finish()),
                ..Default::default()
            };
            let mut parent = NativePresentReport {
                frames: vec![NativePresentFrameObservation {
                    submission: engine_runtime::render_thread::GpuSubmissionToken {
                        device_epoch: 1,
                        serial: 1,
                        frame_index: 1,
                        content_version: 1,
                    },
                    frame_identity: None,
                    application_present_index: 1,
                    application_present_qpc: Some([100, 120]),
                    application_present_thread_id: Some(9),
                    present_id: None,
                    swapchain_epoch: 1,
                    image_index: 0,
                    actual_present_time_ns: None,
                    earliest_present_time_ns: None,
                    present_margin_ns: None,
                }],
                ..Default::default()
            };
            let original = parent.frames.clone();
            super::super::correlation::attach_to_report(capture, &mut parent);
            assert_eq!(parent.frames, original);
            let capture = parent.windows_etw.unwrap();
            let decoded = capture.decoded.unwrap();
            assert_eq!(decoded.ignored_irrelevant_events, IRRELEVANT_REPEATS as u64);
            assert_eq!(decoded.omitted_events, 0);
            assert_eq!(decoded.events.len(), 2);
            let correlated = capture.correlated.unwrap();
            assert_eq!(correlated.history_ready_candidates, u64::from(!unknown));
            if unknown {
                assert_eq!(correlated.status, "incomplete_input_no_attribution");
                assert_eq!(decoded.unsupported_event_kinds[0].event_id, 252);
            } else {
                assert_eq!(correlated.frames[0].frame_record_index, 0);
                assert_eq!(correlated.frames[0].submission, original[0].submission);
                assert_eq!(correlated.frames[0].history_info_event, Some(1));
            }
            assert!(parent.discarded_frames.is_none());
            assert!(parent.repeated_displays.is_none());
        }
    }

    #[test]
    fn native_tdh_decodes_queue_and_flip_keys_with_both_pointer_widths() {
        for pointer_size in [4usize, 8] {
            for id in [168u16, 171, 172, 178, 180, 215] {
                let mut data = Vec::new();
                // Scalar layout is used only to construct manifest test payloads.
                // Production TDH still resolves names and pointer widths itself.
                let pointer = |v: u64| v.to_le_bytes()[..pointer_size].to_vec();
                let u32bytes = |v: u32| v.to_le_bytes();
                match id {
                    168 => {
                        data.extend(pointer(42));
                        data.extend(u32bytes(3));
                        data.extend(pointer(1));
                        data.extend(u32bytes(1));
                        data.extend(u32bytes(0));
                        data.extend(u32bytes(1));
                    }
                    178 => {
                        data.extend(pointer(42));
                        data.extend(u32bytes(3));
                        data.extend(u32bytes(8));
                        data.extend(0u64.to_le_bytes());
                        data.extend([0u8; 8]);
                        data.extend(u32bytes(1));
                        data.extend(pointer(0));
                        data.extend(pointer(0));
                        data.extend(0u64.to_le_bytes());
                    }
                    180 => {
                        data.extend(pointer(42));
                        data.extend(u32bytes(3));
                        data.extend(u32bytes(8));
                        data.extend([0u8; 8]);
                        data.extend(pointer(0));
                    }
                    _ => {
                        data.extend(pointer(1));
                        data.extend(pointer(7));
                        data.extend(u32bytes(2));
                        data.extend([0u8; 20]);
                        if id == 215 {
                            data.extend([0u8; 68]);
                        }
                        data.extend(pointer(0));
                    }
                }
                let mut event: EVENT_RECORD = unsafe { std::mem::zeroed() };
                event.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
                event.EventHeader.ProviderId = super::super::DXGKRNL;
                event.EventHeader.Flags = if pointer_size == 4 {
                    EVENT_HEADER_FLAG_32_BIT_HEADER
                } else {
                    EVENT_HEADER_FLAG_64_BIT_HEADER
                } as u16;
                event.EventHeader.EventDescriptor.Id = id;
                event.EventHeader.EventDescriptor.Version = match id {
                    171 | 172 | 215 => 2,
                    178 | 180 => 1,
                    _ => 0,
                };
                event.EventHeader.EventDescriptor.Opcode = match id {
                    171 | 178 | 215 => 1,
                    180 => 2,
                    _ => 0,
                };
                event.UserData = data.as_mut_ptr().cast();
                event.UserDataLength = data.len() as u16;
                let mut c = Consumer::default();
                event.UserContext = (&mut c as *mut Consumer).cast();
                unsafe {
                    on_event(&mut event);
                }
                assert_eq!(c.report.property_errors, 0, "id {id}, width {pointer_size}");
                assert_eq!(c.report.events.len(), 1);
                let decoded = &c.report.events[0];
                let expected = match id {
                    168 => ("MMIOFlip", 1),
                    178 => ("bPresent", 1),
                    180 => ("bTimeouted", 0),
                    _ => ("Token", 7),
                };
                assert!(decoded
                    .fields
                    .iter()
                    .any(|(name, v)| name == expected.0 && *v == expected.1));
            }
        }
    }

    #[test]
    fn native_composition_tdh_resolves_structs_and_both_pointer_widths() {
        use WindowsEtwProvider::*;
        for width in [4usize, 8] {
            for (provider, id, version, opcode) in [
                (Win32k, 201, 1, 0),
                (Win32k, 301, 1, 0),
                (DwmCore, 15, 0, 1),
                (DwmCore, 196, 0, 0),
                (DwmCore, 467, 0, 0),
                (DwmCore, 468, 0, 0),
                (DwmCore, 476, 0, 0),
            ] {
                let mut data = Vec::new();
                let pointer = |v: u64| v.to_le_bytes()[..width].to_vec();
                match id {
                    201 => {
                        data.extend(pointer(1));
                        data.extend(pointer(2));
                        data.extend(0u32.to_le_bytes());
                        for v in [31u64, 11, 21] {
                            data.extend(v.to_le_bytes());
                        }
                        data.extend([0u8; 12]);
                    }
                    301 => {
                        data.extend(pointer(2));
                        data.extend(0u32.to_le_bytes());
                        data.extend(31u32.to_le_bytes());
                        data.extend(0u64.to_le_bytes());
                        data.extend(3u32.to_le_bytes());
                        data.extend([0u8; 8]);
                        data.extend(11u64.to_le_bytes());
                        data.extend(21u64.to_le_bytes());
                        data.extend(0u32.to_le_bytes());
                    }
                    196 => {
                        data.extend(11u32.to_le_bytes());
                        data.extend(0u32.to_le_bytes());
                        data.extend(21u64.to_le_bytes());
                        data.extend(31u64.to_le_bytes());
                        data.extend([0u8; 80]);
                    }
                    15 => {
                        data.extend([0u8; 36]);
                    }
                    467 | 468 | 476 => {
                        if id == 467 {
                            data.extend(99u64.to_le_bytes());
                        }
                        data.extend(1u64.to_le_bytes());
                        data.extend(2u64.to_le_bytes());
                        for v in [0u32, 1, 9] {
                            data.extend(v.to_le_bytes());
                        }
                        data.extend(11u64.to_le_bytes());
                        data.extend(21u64.to_le_bytes());
                        data.extend(31u32.to_le_bytes());
                        if id == 476 {
                            for v in [1u32, 0, 1] {
                                data.extend(v.to_le_bytes());
                            }
                        } else {
                            data.extend([0u8; 16]);
                        }
                    }
                    _ => unreachable!(),
                }
                // SAFETY: zero-valid EVENT_RECORD with live payload and context.
                let mut event: EVENT_RECORD = unsafe { std::mem::zeroed() };
                event.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
                event.EventHeader.ProviderId = super::super::provider_guid(provider);
                event.EventHeader.Flags = if width == 4 {
                    EVENT_HEADER_FLAG_32_BIT_HEADER
                } else {
                    EVENT_HEADER_FLAG_64_BIT_HEADER
                } as u16;
                event.EventHeader.EventDescriptor.Id = id;
                event.EventHeader.EventDescriptor.Version = version;
                event.EventHeader.EventDescriptor.Opcode = opcode;
                event.UserData = data.as_mut_ptr().cast();
                event.UserDataLength = data.len() as u16;
                let mut consumer = Consumer::default();
                event.UserContext = (&mut consumer as *mut Consumer).cast();
                unsafe {
                    on_event(&mut event);
                }
                assert_eq!(
                    consumer.report.property_errors, 0,
                    "{provider:?} {id} width {width}: {:?}",
                    consumer.report
                );
                assert_eq!(consumer.report.events.len(), 1);
                let decoded = &consumer.report.events[0];
                assert_eq!(decoded.provider, provider);
                let expected = match id {
                    201 | 301 => ("CompositionSurfaceLuid", 11),
                    196 => ("luidSurface.lowpart", 11),
                    467 | 468 | 476 => ("presentCount", 31),
                    _ => ("tCurrent", 0),
                };
                assert!(
                    decoded
                        .fields
                        .iter()
                        .any(|(n, v)| n == expected.0 && *v == expected.1),
                    "width {width}: {decoded:?}"
                );
                if id == 476 {
                    for expected in [
                        ("renderAdapterLuid", 2),
                        ("DirectFlipMode", 1),
                        ("presentFlags", 0),
                        ("syncInterval", 1),
                    ] {
                        assert!(decoded
                            .fields
                            .iter()
                            .any(|(n, v)| n == expected.0 && *v == expected.1));
                    }
                    // Truncation at the final required field cannot publish a partial record.
                    let mut truncated = Consumer::default();
                    event.UserContext = (&mut truncated as *mut Consumer).cast();
                    event.UserDataLength -= 1;
                    unsafe {
                        on_event(&mut event);
                    }
                    assert_eq!(truncated.report.property_errors, 1);
                    assert!(truncated.report.events.is_empty());
                }
                event.UserDataLength = 1;
                let name = if id == 196 {
                    "luidSurface.lowpart"
                } else {
                    expected.0
                };
                assert!(property(&event, name).is_err());
            }
        }
    }

    #[test]
    fn provider_schema_and_unknown_evidence_are_not_interchangeable() {
        let mut c = Consumer::default();
        for provider in [
            WindowsEtwProvider::DxgKrnl,
            WindowsEtwProvider::Win32k,
            WindowsEtwProvider::DwmCore,
        ] {
            c.accept_provider(provider, (201, 99, 0), 1, 2, 3, |_| {
                panic!("unknown schema")
            });
        }
        assert_eq!(c.report.unsupported_event_kinds.len(), 3);
        let old = serde_json::json!({"eventId":116,"version":0,"opcode":0,
            "processId":1,"threadId":2,"timestamp":3,"fields":[]});
        let decoded: WindowsEtwDecodedEvent = serde_json::from_value(old).unwrap();
        assert_eq!(decoded.provider, WindowsEtwProvider::DxgKrnl);
    }

    #[test]
    fn native_callbacks_filter_provider_and_cancel_at_event_budget() {
        let mut c = Consumer::default();
        // SAFETY: zero-valid C POD; callbacks receive live local context. Wrong
        // provider/budget branches must not inspect the null payload.
        let mut e: EVENT_RECORD = unsafe { std::mem::zeroed() };
        e.UserContext = (&mut c as *mut Consumer).cast();
        e.EventHeader.EventDescriptor.Id = 116;
        unsafe {
            on_event(&mut e);
        }
        assert_eq!(c.report.events_seen, 1);
        assert_eq!(c.report.unsupported_events, 0);
        assert_eq!(c.report.property_errors, 0);
        assert!(c.report.events.is_empty());
        c.report.events_seen = MAX_EVENTS;
        e.EventHeader.ProviderId = super::super::DXGKRNL;
        unsafe {
            on_event(&mut e);
        }
        assert!(c.report.event_budget_reached);
        assert_eq!(c.report.events_seen, MAX_EVENTS);
        let mut log: EVENT_TRACE_LOGFILEW = unsafe { std::mem::zeroed() };
        log.Context = e.UserContext;
        assert_eq!(unsafe { on_buffer(&mut log) }, 0);
        c.report.event_budget_reached = false;
        assert_eq!(unsafe { on_buffer(&mut log) }, 1);
    }

    #[test]
    fn native_tdh_reads_manifest_properties_for_both_pointer_widths() {
        for (flags, pointer_size) in [
            (EVENT_HEADER_FLAG_32_BIT_HEADER, 4),
            (EVENT_HEADER_FLAG_64_BIT_HEADER, 8),
        ] {
            // DxgKrnl event 116 v0 manifest payload, not an ETL/display fixture.
            let mut data = vec![0u8; if pointer_size == 4 { 40 } else { 48 }];
            data[..pointer_size].copy_from_slice(&0x1234u64.to_le_bytes()[..pointer_size]);
            data[pointer_size..pointer_size + 4].copy_from_slice(&7u32.to_le_bytes());
            data[pointer_size + 4..pointer_size + 8].copy_from_slice(&91u32.to_le_bytes());
            // SAFETY: zero-valid C event structure; data pointer is alive for TDH calls.
            let mut e: EVENT_RECORD = unsafe { std::mem::zeroed() };
            e.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
            e.EventHeader.ProviderId = super::super::DXGKRNL;
            e.EventHeader.Flags = flags as u16;
            e.EventHeader.EventDescriptor.Id = 116;
            e.UserData = data.as_mut_ptr().cast();
            e.UserDataLength = data.len() as u16;
            assert_eq!(property(&e, "pDxgAdapter"), Ok(0x1234));
            assert_eq!(property(&e, "VidPnSourceId"), Ok(7));
            assert_eq!(property(&e, "FlipSubmitSequence"), Ok(91));
            let mut c = Consumer::default();
            e.UserContext = (&mut c as *mut Consumer).cast();
            unsafe {
                on_event(&mut e);
            }
            assert_eq!(c.report.property_errors, 0);
            assert_eq!(c.report.events[0].fields.len(), 4);
            assert!(c.report.events[0]
                .fields
                .iter()
                .any(|(name, value)| name == "Flags" && *value == 0));
            e.UserDataLength = 1;
            assert!(property(&e, "FlipSubmitSequence").is_err());
        }
    }

    #[test]
    fn versions_and_opcodes_are_not_guessed() {
        assert!(fields(171, 2, 1).is_some());
        assert!(fields(171, 3, 1).is_none());
        assert!(fields(171, 2, 0).is_none());
        assert!(fields(116, 0, 0).is_some());
        assert!(fields(999, 0, 0).is_none());
    }

    #[test]
    fn property_width_is_checked_without_truncation() {
        assert_eq!(integer(&[255, 255, 255, 255]), Some(u32::MAX as u64));
        assert_eq!(integer(&u64::MAX.to_le_bytes()), Some(u64::MAX));
        assert_eq!(integer(&[1, 2, 3]), None);
        assert_eq!(integer(&[1; 16]), None);
    }

    #[test]
    fn system_events_are_preserved_and_failure_does_not_publish_partial_keys() {
        let mut c = Consumer::default();
        c.accept(116, 0, 0, 4, 9, 123, |_| Ok(7));
        assert_eq!(c.report.events[0].process_id, 4);
        assert_eq!(c.report.events[0].fields.len(), 4);
        c.accept(171, 2, 1, 100, 9, 124, |name| {
            if name == "Token" {
                Err(13)
            } else {
                Ok(1)
            }
        });
        assert_eq!(c.report.events.len(), 1);
        assert_eq!(c.report.property_errors, 1);
        assert_eq!(c.report.first_property_error, Some(13));
    }

    #[test]
    fn record_budget_reports_omissions_and_never_counts_them_as_discarded() {
        let mut c = Consumer::default();
        while c.report.omitted_events == 0 {
            c.accept(116, 0, 0, 4, 9, 123, |_| Ok(7));
        }
        assert!(c.report.events.len() > 5000);
        assert!(c.decoded_bytes <= MAX_DECODED_BYTES);
        assert_eq!(decoded_input_bytes(&c.report.events), c.decoded_bytes);
        assert_eq!(c.report.omitted_events, 1);
        assert!(c.incomplete());
    }

    #[test]
    fn decoded_budget_charges_actual_field_and_string_capacity() {
        let mut event = WindowsEtwDecodedEvent {
            provider: WindowsEtwProvider::DxgKrnl,
            event_id: 116,
            version: 0,
            opcode: 0,
            process_id: 1,
            thread_id: 2,
            timestamp: 3,
            fields: Vec::with_capacity(64),
        };
        let empty = event_bytes(&event);
        let mut name = String::with_capacity(MAX_DECODED_BYTES);
        name.push_str("field");
        event.fields.push((name, 1));
        assert!(event_bytes(&event) > MAX_DECODED_BYTES);
        assert!(event_bytes(&event) >= empty + MAX_DECODED_BYTES);
    }

    #[test]
    fn unknown_event_is_explicit_and_not_decoded() {
        let mut c = Consumer::default();
        c.accept(171, 99, 1, 4, 9, 123, |_| panic!("must not read payload"));
        assert!(c.report.events.is_empty());
        assert_eq!(c.report.unsupported_events, 1);
        assert!(c.incomplete());
    }

    #[test]
    fn missing_file_uses_real_open_failure_without_creating_trace_state() {
        let path = std::env::temp_dir()
            .join(format!("aipe-no-etl-{}", std::process::id()))
            .join("missing.etl");
        let r = read(&path);
        assert_eq!(r.status, "open_failed");
        assert!(r.open_error.is_some());
        assert!(!path.exists());
    }
    // Payload layout frozen from this host's manifest; production reads TDH names.
    #[test]
    fn native_lifetime_independent_and_target_tdh_resolves_both_producer_widths() {
        type NativeCase = (u16, u8, u8, &'static [(usize, u64)]);
        let cases: &[NativeCase] = &[
            (
                27,
                2,
                1,
                &[
                    (0, 100),
                    (0, 1),
                    (4, 0),
                    (0, 7),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (0, 0),
                    (4, 0),
                    (0, 0),
                ],
            ),
            (
                28,
                2,
                2,
                &[
                    (0, 100),
                    (0, 1),
                    (4, 0),
                    (0, 7),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (0, 0),
                    (4, 0),
                    (0, 0),
                ],
            ),
            (
                29,
                2,
                3,
                &[
                    (0, 100),
                    (0, 1),
                    (4, 0),
                    (0, 7),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (0, 0),
                    (4, 0),
                    (0, 0),
                ],
            ),
            (
                30,
                0,
                1,
                &[
                    (0, 7),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (0, 42),
                    (0, 0),
                    (0, 0),
                ],
            ),
            (
                31,
                0,
                2,
                &[
                    (0, 7),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (0, 42),
                    (0, 0),
                    (0, 0),
                ],
            ),
            (
                32,
                0,
                3,
                &[
                    (0, 7),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (0, 42),
                    (0, 0),
                    (0, 0),
                ],
            ),
            (
                259,
                3,
                0,
                &[
                    (0, 1),
                    (4, 3),
                    (4, 0),
                    (8, 34359738368),
                    (0, 0),
                    (8, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 5),
                    (4, 1),
                    (4, 0),
                    (4, 0),
                ],
            ),
            (
                266,
                3,
                0,
                &[
                    (0, 42),
                    (4, 8),
                    (4, 3),
                    (0, 0),
                    (4, 1),
                    (4, 0),
                    (4, 0),
                    (8, 11),
                    (8, 21),
                    (4, 31),
                    (4, 0),
                    (0, 0),
                    (4, 0),
                    (8, 0),
                    (4, 0),
                ],
            ),
            (
                17,
                0,
                0,
                &[
                    (0, 1),
                    (4, 2),
                    (8, 0),
                    (4, 3),
                    (4, 0),
                    (8, 0),
                    (0, 0),
                    (4, 0),
                    (8, 34359738368),
                ],
            ),
        ];
        for width in [4usize, 8] {
            for &(id, version, opcode, layout) in cases {
                let mut payload = Vec::new();
                for &(size, value) in layout {
                    let size = if size == 0 { width } else { size };
                    payload.extend_from_slice(&value.to_le_bytes()[..size]);
                }
                // SAFETY: POD event owns a live bounded payload and consumer for synchronous callback.
                let mut event: EVENT_RECORD = unsafe { std::mem::zeroed() };
                event.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
                event.EventHeader.ProviderId = super::super::DXGKRNL;
                event.EventHeader.Flags = if width == 4 {
                    EVENT_HEADER_FLAG_32_BIT_HEADER
                } else {
                    EVENT_HEADER_FLAG_64_BIT_HEADER
                } as u16;
                event.EventHeader.EventDescriptor.Id = id;
                event.EventHeader.EventDescriptor.Version = version;
                event.EventHeader.EventDescriptor.Opcode = opcode;
                event.UserData = payload.as_mut_ptr().cast();
                event.UserDataLength = payload.len() as u16;
                let mut consumer = Consumer::default();
                event.UserContext = (&mut consumer as *mut Consumer).cast();
                unsafe { on_event(&mut event) };
                assert_eq!(consumer.report.unsupported_events, 0, "{id} width {width}");
                assert_eq!(
                    consumer.report.property_errors, 0,
                    "{id} width {width}: {:?}",
                    consumer.report
                );
                assert_eq!(consumer.report.events.len(), 1);
                let expected = match id {
                    27..=29 => ("hProcessId", 100),
                    30..=32 => ("hContext", 42),
                    259 => ("FlipSubmitSequence", 8u64 << 32),
                    266 => ("CompositionSurfaceLuid", 11),
                    17 => ("VidPnTargetId", 2),
                    _ => unreachable!(),
                };
                assert_eq!(
                    super::super::correlation::field(&consumer.report.events[0], expected.0),
                    Some(expected.1)
                );
                event.UserDataLength = 1;
                unsafe { on_event(&mut event) };
                assert_eq!(consumer.report.events.len(), 1);
                assert_eq!(consumer.report.property_errors, 1);
            }
        }
    }

    #[test]
    fn native_mpo_tdh_reads_indexed_arrays_at_both_pointer_widths_and_rejects_truncation() {
        type NativeCase = (u16, u8, &'static [(usize, u64)]);
        let cases: &[NativeCase] = &[
            (
                273,
                4,
                &[
                    (0, 1),
                    (4, 2),
                    (8, 0),
                    (8, 0),
                    (4, 3),
                    (4, 1),
                    (4, 2),
                    (8, 34359738368),
                    (8, 38654705664),
                    (4, 2),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (8, 0),
                    (8, 0),
                ],
            ),
            (
                382,
                2,
                &[
                    (0, 1),
                    (4, 2),
                    (8, 0),
                    (8, 0),
                    (4, 3),
                    (4, 1),
                    (4, 2),
                    (8, 34359738368),
                    (8, 38654705664),
                    (8, 0),
                    (8, 0),
                ],
            ),
            (
                386,
                8,
                &[
                    (0, 1),
                    (4, 3),
                    (4, 2),
                    (8, 0),
                    (8, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 0),
                    (4, 1),
                    (4, 0),
                    (8, 0),
                    (8, 0),
                    (4, 0),
                    (4, 0),
                    (4, 1),
                    (4, 8),
                    (4, 0),
                ],
            ),
        ];
        for width in [4usize, 8] {
            for &(id, version, layout) in cases {
                let mut payload = Vec::new();
                for &(size, value) in layout {
                    payload.extend_from_slice(
                        &value.to_le_bytes()[..if size == 0 { width } else { size }],
                    );
                }
                // SAFETY: POD with live payload and consumer during synchronous callback.
                let mut event: EVENT_RECORD = unsafe { std::mem::zeroed() };
                event.EventHeader.Size = std::mem::size_of::<EVENT_HEADER>() as u16;
                event.EventHeader.ProviderId = super::super::DXGKRNL;
                event.EventHeader.Flags = if width == 4 {
                    EVENT_HEADER_FLAG_32_BIT_HEADER
                } else {
                    EVENT_HEADER_FLAG_64_BIT_HEADER
                } as u16;
                event.EventHeader.EventDescriptor.Id = id;
                event.EventHeader.EventDescriptor.Version = version;
                event.UserData = payload.as_mut_ptr().cast();
                event.UserDataLength = payload.len() as u16;
                let mut consumer = Consumer::default();
                event.UserContext = (&mut consumer as *mut Consumer).cast();
                unsafe { on_event(&mut event) };
                assert_eq!(
                    consumer.report.property_errors, 0,
                    "id {id} width {width}: {:?}",
                    consumer.report
                );
                assert_eq!(consumer.report.events.len(), 1);
                let decoded = &consumer.report.events[0];
                let expected = if id == 386 {
                    vec![8]
                } else {
                    vec![8u64 << 32, 9u64 << 32]
                };
                assert_eq!(
                    super::super::independent::multi_plane_sequences(decoded),
                    Some(expected)
                );
                event.UserDataLength -= 1;
                unsafe { on_event(&mut event) };
                assert_eq!(
                    consumer.report.events.len(),
                    1,
                    "partial array must not escape"
                );
                assert_eq!(consumer.report.property_errors, 1);
            }
        }
    }

    #[test]
    fn mpo_counts_are_bounded_before_array_property_calls_and_unknown_versions_stay_unknown() {
        for (id, version, count, max) in [
            (273, 4, "PlaneCount", 16),
            (273, 4, "FlipEntryCount", 64),
            (273, 4, "FlipQueues", 16),
            (382, 2, "PlaneCount", 16),
            (386, 8, "PlaneCount", 16),
        ] {
            let mut consumer = Consumer::default();
            consumer.accept(id, version, 0, 4, 9, 150, |name| {
                assert!(
                    !name.contains('['),
                    "oversize counts must reject before arrays"
                );
                Ok(if name == count { max + 1 } else { 0 })
            });
            assert_eq!(consumer.report.property_errors, 1);
            assert!(consumer.report.events.is_empty());
            let mut consumer = Consumer::default();
            consumer.accept(id, version + 1, 0, 4, 9, 150, |_| {
                panic!("unknown schema payload")
            });
            assert_eq!(consumer.report.unsupported_events, 1);
            assert!(consumer.report.events.is_empty());
        }
        for (id, version) in [(273, 4), (382, 2), (386, 8)] {
            let mut consumer = Consumer::default();
            consumer.accept(id, version, 0, 4, 9, 150, |name| {
                assert!(!name.contains('['));
                Ok(0)
            });
            assert_eq!(consumer.report.property_errors, 0);
            assert_eq!(consumer.report.events.len(), 1);
        }
    }
}
