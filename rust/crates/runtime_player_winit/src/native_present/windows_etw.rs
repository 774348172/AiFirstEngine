//! One opt-in, uniquely owned DxgKrnl/Win32k/DWM trace. No global-session takeover.
//! ETL is evidence input, not a decoded/attributed display completion result.
use engine_runtime::windowed_player::{WindowsEtwCaptureReport, WindowsEtwProvider};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::System::Diagnostics::Etw::*;

mod composition;
mod correlation;
mod decode;
mod independent;
mod lifetime;
pub(super) use correlation::attach_to_report;

const MAX_FILE_MB: u32 = 32;
const BUFFER_KB: u32 = 64;
const MAX_BUFFERS: u32 = 16;
const DXGKRNL: windows_sys::core::GUID =
    windows_sys::core::GUID::from_u128(0x802ec45a_1e99_4b83_9920_87c98277ba9d);
// Manifest keywords Present and Base. Kernel/system events must remain available;
// filtering by the application's PID here would lose compositor/flip events.
const WIN32K: windows_sys::core::GUID =
    windows_sys::core::GUID::from_u128(0x8c416c79_d49b_4f01_a467_e56d3aa8234c);
const DWM: windows_sys::core::GUID =
    windows_sys::core::GUID::from_u128(0x9e9bba3c_2e38_40cb_99f4_9e8281425164);
const PROVIDERS: [WindowsEtwProvider; 3] = [
    WindowsEtwProvider::DxgKrnl,
    WindowsEtwProvider::Win32k,
    WindowsEtwProvider::DwmCore,
];

fn provider_from_guid(guid: &windows_sys::core::GUID) -> Option<WindowsEtwProvider> {
    PROVIDERS.into_iter().find(|p| {
        let expected = provider_guid(*p);
        guid.data1 == expected.data1
            && guid.data2 == expected.data2
            && guid.data3 == expected.data3
            && guid.data4 == expected.data4
    })
}

fn provider_guid(provider: WindowsEtwProvider) -> windows_sys::core::GUID {
    match provider {
        WindowsEtwProvider::DxgKrnl => DXGKRNL,
        WindowsEtwProvider::Win32k => WIN32K,
        WindowsEtwProvider::DwmCore => DWM,
    }
}

fn event_ids(provider: WindowsEtwProvider) -> &'static [u16] {
    match provider {
        // Include competing modes/lifetimes even while their schemas are unsupported:
        // filtering them out would turn a coverage gap into false successful attribution.
        WindowsEtwProvider::DxgKrnl => &[
            17, 27, 28, 29, 30, 31, 32, 116, 166, 168, 171, 172, 175, 176, 177, 178, 180, 184, 215,
            244, 250, 252, 259, 266, 273, 382, 385, 386, 422, 423, 424, 450, 451, 501,
        ],
        WindowsEtwProvider::Win32k => &[201, 301],
        WindowsEtwProvider::DwmCore => &[15, 196, 203, 464, 467, 468, 473, 476],
    }
}

#[repr(C)]
struct EventIdFilter {
    filter_in: bool,
    reserved: u8,
    count: u16,
    ids: [u16; 64],
}
impl EventIdFilter {
    fn new(provider: WindowsEtwProvider) -> Self {
        let ids = event_ids(provider);
        let mut value = Self {
            filter_in: true,
            reserved: 0,
            count: ids.len() as u16,
            ids: [0; 64],
        };
        value.ids[..ids.len()].copy_from_slice(ids);
        value
    }
    fn descriptor(&self) -> EVENT_FILTER_DESCRIPTOR {
        EVENT_FILTER_DESCRIPTOR {
            Ptr: self as *const Self as u64,
            Size: 4 + 2 * u32::from(self.count),
            Type: EVENT_FILTER_TYPE_EVENT_ID,
        }
    }
    fn rundown() -> Self {
        let mut filter = Self::new(WindowsEtwProvider::DxgKrnl);
        filter.count = 3;
        filter.ids[..3].copy_from_slice(&[29, 32, 424]);
        filter
    }
}
static NEXT_CAPTURE: AtomicU64 = AtomicU64::new(1);

#[repr(C)]
struct Properties {
    properties: EVENT_TRACE_PROPERTIES,
    name: [u16; 128],
    path: [u16; 1024],
}

impl Properties {
    fn new(name: &str, path: &std::path::Path) -> Result<Self, &'static str> {
        use std::os::windows::ffi::OsStrExt;
        let name: Vec<_> = name.encode_utf16().collect();
        let path: Vec<_> = path.as_os_str().encode_wide().collect();
        if name.len() >= 128 || path.len() >= 1024 || name.contains(&0) || path.contains(&0) {
            return Err("invalid_trace_path");
        }
        // SAFETY: EVENT_TRACE_PROPERTIES is a C POD structure; its handles and
        // unions permit zero initialization. Arrays include explicit NUL space.
        let mut value: Self = unsafe { std::mem::zeroed() };
        value.name[..name.len()].copy_from_slice(&name);
        value.path[..path.len()].copy_from_slice(&path);
        let p = &mut value.properties;
        p.Wnode.BufferSize = std::mem::size_of::<Self>() as u32;
        p.Wnode.ClientContext = 1; // ETW QPC clock, same basis as present boundaries.
        p.Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        p.BufferSize = BUFFER_KB;
        p.MinimumBuffers = 2;
        p.MaximumBuffers = MAX_BUFFERS;
        p.MaximumFileSize = MAX_FILE_MB;
        p.LogFileMode = EVENT_TRACE_FILE_MODE_SEQUENTIAL | EVENT_TRACE_NO_PER_PROCESSOR_BUFFERING;
        p.FlushTimer = 1;
        p.LoggerNameOffset = std::mem::offset_of!(Self, name) as u32;
        p.LogFileNameOffset = std::mem::offset_of!(Self, path) as u32;
        Ok(value)
    }
}

trait TraceApi {
    fn start(&mut self, properties: &mut Properties) -> Result<CONTROLTRACE_HANDLE, u32>;
    fn enable(&mut self, handle: CONTROLTRACE_HANDLE, provider: WindowsEtwProvider) -> u32;
    fn rundown(&mut self, handle: CONTROLTRACE_HANDLE) -> u32;
    fn stop(&mut self, handle: CONTROLTRACE_HANDLE, properties: &mut Properties) -> u32;
}

struct WindowsApi;
impl WindowsApi {
    fn control(
        handle: CONTROLTRACE_HANDLE,
        provider: WindowsEtwProvider,
        code: u32,
        filter: EventIdFilter,
    ) -> u32 {
        let mut descriptor = filter.descriptor();
        let parameters = ENABLE_TRACE_PARAMETERS {
            Version: ENABLE_TRACE_PARAMETERS_VERSION_2,
            EnableFilterDesc: &mut descriptor,
            FilterDescCount: 1,
            EnableProperty: 0,
            ControlFlags: 0,
            SourceId: windows_sys::core::GUID::from_u128(0),
        };
        // SAFETY: this owner created the handle; filter and descriptors stay live
        // during the synchronous enable/capture-state request. No global trace is touched.
        unsafe {
            EnableTraceEx2(
                handle,
                &provider_guid(provider),
                code,
                5,
                0,
                0,
                1000,
                &parameters,
            )
        }
    }
}
impl TraceApi for WindowsApi {
    fn start(&mut self, p: &mut Properties) -> Result<CONTROLTRACE_HANDLE, u32> {
        let mut handle = CONTROLTRACE_HANDLE { Value: 0 };
        // SAFETY: The repr(C) structure includes aligned properties and bounded
        // NUL-terminated strings whose offsets remain valid throughout the call.
        let result = unsafe { StartTraceW(&mut handle, p.name.as_ptr(), &mut p.properties) };
        if result == 0 {
            Ok(handle)
        } else {
            Err(result)
        }
    }
    fn enable(&mut self, handle: CONTROLTRACE_HANDLE, provider: WindowsEtwProvider) -> u32 {
        Self::control(
            handle,
            provider,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER,
            EventIdFilter::new(provider),
        )
    }
    fn rundown(&mut self, handle: CONTROLTRACE_HANDLE) -> u32 {
        Self::control(
            handle,
            WindowsEtwProvider::DxgKrnl,
            EVENT_CONTROL_CODE_CAPTURE_STATE,
            EventIdFilter::rundown(),
        )
    }
    fn stop(&mut self, handle: CONTROLTRACE_HANDLE, p: &mut Properties) -> u32 {
        // SAFETY: Stop by our owned handle only, never by a colliding logger name.
        unsafe {
            ControlTraceW(
                handle,
                std::ptr::null(),
                &mut p.properties,
                EVENT_TRACE_CONTROL_STOP,
            )
        }
    }
}

struct OwnedTrace<A: TraceApi> {
    api: A,
    properties: Properties,
    handle: Option<CONTROLTRACE_HANDLE>,
    report: WindowsEtwCaptureReport,
}

impl<A: TraceApi> OwnedTrace<A> {
    fn start(api: A, properties: Properties, report: WindowsEtwCaptureReport) -> Self {
        let mut trace = Self {
            api,
            properties,
            handle: None,
            report,
        };
        let started = Instant::now();
        match trace.api.start(&mut trace.properties) {
            Err(error) => {
                trace.report.start_error = Some(error);
                trace.report.status = match error {
                    5 => "access_denied",
                    183 => "session_name_collision",
                    _ => "start_failed",
                }
                .into();
                trace.report.cleanup_status = "not_created".into();
            }
            Ok(handle) => {
                trace.handle = Some(handle);
                trace.report.cleanup_status = "owned_session_active".into();
                for provider in PROVIDERS {
                    let error = trace.api.enable(handle, provider);
                    if error != 0 {
                        trace.report.failed_provider = Some(provider);
                        trace.report.enable_error = Some(error);
                        trace.report.status = "enable_failed".into();
                        trace.stop();
                        break;
                    }
                    trace.report.enabled_providers.push(provider);
                }
                if trace.report.enable_error.is_none() {
                    trace.report.rundown_requested = true;
                    let error = trace.api.rundown(handle);
                    if error == 0 {
                        trace.report.status = "collecting_raw_events".into();
                    } else {
                        trace.report.rundown_error = Some(error);
                        trace.report.status = "rundown_failed".into();
                        trace.stop();
                    }
                }
            }
        }
        trace.report.lifecycle_cpu_ns += elapsed_ns(started);
        trace
    }

    fn stop(&mut self) {
        let Some(handle) = self.handle else {
            return;
        };
        if self.report.stop_attempts >= 2 {
            return;
        }
        self.report.stop_attempts += 1;
        let result = self.api.stop(handle, &mut self.properties);
        if result == 0 {
            self.handle = None;
            self.report.cleanup_status = "stopped".into();
            self.report.events_lost = Some(self.properties.properties.EventsLost);
            self.report.log_buffers_lost = Some(self.properties.properties.LogBuffersLost);
            self.report.buffers_written = Some(self.properties.properties.BuffersWritten);
        } else if result == 4201 {
            // ERROR_WMI_INSTANCE_NOT_FOUND: logger already ended.
            self.handle = None;
            self.report.stop_error = Some(result);
            self.report.cleanup_status = "already_ended_statistics_unavailable".into();
        } else {
            self.report.stop_error = Some(result);
            self.report.cleanup_status = "stop_failed_session_may_remain".into();
        }
    }

    fn finish(&mut self) -> WindowsEtwCaptureReport {
        let started = Instant::now();
        self.stop();
        if self.handle.is_some() && self.report.stop_attempts < 2 {
            self.stop();
        }
        if self.report.status == "collecting_raw_events" {
            self.report.status = if self.report.cleanup_status != "stopped" {
                "capture_incomplete"
            } else if self.report.events_lost != Some(0) || self.report.log_buffers_lost != Some(0)
            {
                "raw_events_lost"
            } else {
                "raw_capture_unresolved"
            }
            .into();
        }
        self.report.lifecycle_cpu_ns += elapsed_ns(started);
        self.report.clone()
    }
}

impl<A: TraceApi> Drop for OwnedTrace<A> {
    fn drop(&mut self) {
        // Panic/early-exit fallback. A previously failed start never owns a handle.
        if self.handle.is_some() && self.report.stop_attempts < 2 {
            self.stop();
        }
    }
}

pub(super) struct WindowsEtwCapture {
    trace: Option<OwnedTrace<WindowsApi>>,
    report: WindowsEtwCaptureReport,
    root: Option<PathBuf>,
}

impl WindowsEtwCapture {
    pub(super) fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!(
            "aipe-present-{}-{stamp}-{}",
            std::process::id(),
            NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed)
        );
        let root = std::env::temp_dir().join(&name);
        let file = root.join("dxgkrnl.etl");
        let mut report = WindowsEtwCaptureReport {
            process_id: std::process::id(),
            session_name: Some(name.clone()),
            etl_path: Some(file.to_string_lossy().into_owned()),
            scope: "DxgKrnl/Win32k/DwmCore; strict event-ID filters including unsupported competing paths; system-wide PIDs".into(),
            decode_status: "not_attempted_no_capture".into(),
            qpc_frequency: qpc_frequency(),
            maximum_file_bytes: u64::from(MAX_FILE_MB) * 1024 * 1024,
            maximum_buffer_bytes: u64::from(BUFFER_KB) * u64::from(MAX_BUFFERS) * 1024,
            cleanup_status: "not_created".into(),
            ..Default::default()
        };
        if std::fs::create_dir(&root).is_err() {
            report.status = "trace_directory_failed".into();
            return Self {
                trace: None,
                report,
                root: None,
            };
        }
        match Properties::new(&name, &file) {
            Ok(properties) => Self {
                trace: Some(OwnedTrace::start(WindowsApi, properties, report.clone())),
                report,
                root: Some(root),
            },
            Err(error) => {
                report.status = error.into();
                Self {
                    trace: None,
                    report,
                    root: Some(root),
                }
            }
        }
    }

    pub(super) fn finish(mut self) -> WindowsEtwCaptureReport {
        if let Some(trace) = &mut self.trace {
            self.report = trace.finish();
        }
        self.report.etl_bytes = self
            .report
            .etl_path
            .as_deref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len());
        if self.report.etl_bytes.is_none() && self.report.status == "raw_capture_unresolved" {
            self.report.status = "capture_file_missing".into();
        }
        if self
            .report
            .etl_bytes
            .is_some_and(|bytes| bytes >= self.report.maximum_file_bytes)
            && self.report.status == "raw_capture_unresolved"
        {
            // Full sequential files may have stopped before the observation window ended.
            self.report.status = "file_budget_reached_window_incomplete".into();
        }
        // Never read a still-active or unbounded file. Partial/lost capture status
        // remains independent of whether its bounded input records can be decoded.
        if self.report.cleanup_status == "stopped"
            && self
                .report
                .etl_bytes
                .is_some_and(|n| n <= self.report.maximum_file_bytes)
        {
            if let Some(path) = self.report.etl_path.as_deref() {
                let decoded = decode::read(std::path::Path::new(path));
                self.report.decode_status = decoded.status.clone();
                self.report.decoded = Some(decoded);
            }
        }
        self.remove_empty_root();
        self.report.clone()
    }

    fn remove_empty_root(&self) {
        if self
            .trace
            .as_ref()
            .is_some_and(|trace| trace.handle.is_some())
        {
            return;
        }
        if let Some(root) = &self.root {
            // Only an empty directory created by us; ETL evidence is never deleted.
            let _ = std::fs::remove_dir(root);
        }
    }
}

impl Drop for WindowsEtwCapture {
    fn drop(&mut self) {
        if let Some(trace) = &mut self.trace {
            if trace.handle.is_some() && trace.report.stop_attempts < 2 {
                trace.finish();
            }
        }
        self.remove_empty_root();
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

pub(super) fn qpc() -> Option<i64> {
    let mut value = 0;
    // SAFETY: Valid writable scalar; QueryPerformanceCounter has no session side effects.
    (unsafe { windows_sys::Win32::System::Performance::QueryPerformanceCounter(&mut value) } != 0)
        .then_some(value)
}

fn qpc_frequency() -> Option<i64> {
    let mut value = 0;
    // SAFETY: Valid writable scalar. QPC frequency remains independent of ETW availability.
    (unsafe { windows_sys::Win32::System::Performance::QueryPerformanceFrequency(&mut value) } != 0
        && value > 0)
        .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Calls {
        start: u32,
        enable: u32,
        rundown: u32,
        stop: u32,
        handles: Vec<u64>,
        providers: Vec<WindowsEtwProvider>,
    }
    struct FakeApi {
        calls: Arc<Mutex<Calls>>,
        start_error: u32,
        enable_error: u32,
        stop_errors: Vec<u32>,
        lost: u32,
        rundown_error: u32,
    }
    impl TraceApi for FakeApi {
        fn start(&mut self, _: &mut Properties) -> Result<CONTROLTRACE_HANDLE, u32> {
            self.calls.lock().unwrap().start += 1;
            if self.start_error == 0 {
                Ok(CONTROLTRACE_HANDLE { Value: 71 })
            } else {
                Err(self.start_error)
            }
        }
        fn enable(&mut self, h: CONTROLTRACE_HANDLE, provider: WindowsEtwProvider) -> u32 {
            let mut calls = self.calls.lock().unwrap();
            calls.enable += 1;
            calls.providers.push(provider);
            calls.handles.push(h.Value);
            if self.enable_error == 87 && provider != WindowsEtwProvider::Win32k {
                0
            } else {
                self.enable_error
            }
        }
        fn rundown(&mut self, h: CONTROLTRACE_HANDLE) -> u32 {
            let mut calls = self.calls.lock().unwrap();
            assert_eq!(calls.providers, PROVIDERS);
            calls.rundown += 1;
            calls.handles.push(h.Value);
            self.rundown_error
        }
        fn stop(&mut self, h: CONTROLTRACE_HANDLE, p: &mut Properties) -> u32 {
            let mut calls = self.calls.lock().unwrap();
            calls.stop += 1;
            calls.handles.push(h.Value);
            p.properties.EventsLost = self.lost;
            self.stop_errors
                .get(calls.stop as usize - 1)
                .copied()
                .unwrap_or(0)
        }
    }
    fn fake(
        start_error: u32,
        enable_error: u32,
        stop_errors: Vec<u32>,
        lost: u32,
    ) -> (OwnedTrace<FakeApi>, Arc<Mutex<Calls>>) {
        fake_with_rundown(start_error, enable_error, stop_errors, lost, 0)
    }
    fn fake_with_rundown(
        start_error: u32,
        enable_error: u32,
        stop_errors: Vec<u32>,
        lost: u32,
        rundown_error: u32,
    ) -> (OwnedTrace<FakeApi>, Arc<Mutex<Calls>>) {
        let calls = Arc::new(Mutex::new(Calls::default()));
        let api = FakeApi {
            calls: calls.clone(),
            start_error,
            enable_error,
            stop_errors,
            lost,
            rundown_error,
        };
        let trace = OwnedTrace::start(
            api,
            Properties::new("owned-test", std::path::Path::new("C:/owned-test.etl")).unwrap(),
            WindowsEtwCaptureReport::default(),
        );
        (trace, calls)
    }
    #[test]
    fn multi_provider_enable_is_ordered_and_partial_failure_stops_owned_session() {
        let (mut trace, calls) = fake(0, 0, vec![], 0);
        let report = trace.finish();
        assert_eq!(report.enabled_providers, PROVIDERS);
        assert_eq!(report.failed_provider, None);
        assert_eq!(calls.lock().unwrap().providers, PROVIDERS);
        let (mut trace, calls) = fake(0, 87, vec![], 0);
        let report = trace.finish();
        assert_eq!(report.enabled_providers, [WindowsEtwProvider::DxgKrnl]);
        assert_eq!(report.failed_provider, Some(WindowsEtwProvider::Win32k));
        assert_eq!(report.enable_error, Some(87));
        assert_eq!(report.cleanup_status, "stopped");
        assert_eq!(calls.lock().unwrap().providers, PROVIDERS[..2]);
        assert_eq!(calls.lock().unwrap().stop, 1);
    }

    #[test]
    fn late_observer_requests_initial_device_context_state_once_on_owned_session() {
        let (mut trace, calls) = fake(0, 0, vec![], 0);
        let report = trace.finish();
        assert!(report.rundown_requested);
        assert_eq!(report.rundown_error, None);
        let filter = EventIdFilter::rundown();
        assert_eq!(&filter.ids[..filter.count as usize], &[29, 32, 424]);
        assert_eq!(calls.lock().unwrap().rundown, 1);
        for (start, enable) in [(5, 0), (0, 87)] {
            let (mut trace, calls) = fake(start, enable, vec![], 0);
            trace.finish();
            assert_eq!(calls.lock().unwrap().rundown, 0);
        }
    }

    #[test]
    fn hardware_queue_stop_is_not_filtered_out_of_lifetime_evidence() {
        assert!(event_ids(WindowsEtwProvider::DxgKrnl).contains(&423));
    }

    #[test]
    fn rundown_failure_stops_only_owned_session_and_never_becomes_raw_success() {
        for error in [5, 87] {
            let (mut trace, calls) = fake_with_rundown(0, 0, vec![5, 0], 0, error);
            let report = trace.finish();
            drop(trace);
            assert_eq!(report.status, "rundown_failed");
            assert!(report.rundown_requested);
            assert_eq!(report.rundown_error, Some(error));
            assert_eq!(report.enable_error, None);
            assert_eq!(report.failed_provider, None);
            assert_eq!(report.cleanup_status, "stopped");
            assert_eq!(report.stop_attempts, 2);
            let calls = calls.lock().unwrap();
            assert_eq!(calls.rundown, 1);
            assert_eq!(calls.stop, 2);
            assert!(calls.handles.iter().all(|h| *h == 71));
        }
    }

    #[test]
    fn legacy_capture_json_keeps_initial_state_request_unknown() {
        let mut value = serde_json::to_value(WindowsEtwCaptureReport::default()).unwrap();
        value.as_object_mut().unwrap().remove("rundownRequested");
        value.as_object_mut().unwrap().remove("rundownError");
        let report: WindowsEtwCaptureReport = serde_json::from_value(value).unwrap();
        assert!(!report.rundown_requested);
        assert_eq!(report.rundown_error, None);
    }

    #[test]
    fn event_filters_have_native_layout_and_keep_unimplemented_competing_modes() {
        assert_eq!(std::mem::offset_of!(EventIdFilter, ids), 4);
        for provider in PROVIDERS {
            let filter = EventIdFilter::new(provider);
            let descriptor = filter.descriptor();
            assert!(filter.filter_in);
            assert_eq!(descriptor.Type, EVENT_FILTER_TYPE_EVENT_ID);
            assert_eq!(descriptor.Size, 4 + 2 * u32::from(filter.count));
            assert_eq!(&filter.ids[..filter.count as usize], event_ids(provider));
            assert_eq!(provider_from_guid(&provider_guid(provider)), Some(provider));
        }
        assert!(event_ids(WindowsEtwProvider::DxgKrnl).contains(&252));
        assert!(event_ids(WindowsEtwProvider::DwmCore).contains(&203));
        assert!(event_ids(WindowsEtwProvider::DwmCore).contains(&476));
    }

    #[test]
    fn mpo_sync_filter_does_not_hide_multiplane_vsync_from_attribution() {
        assert!(event_ids(WindowsEtwProvider::DxgKrnl).contains(&273));
    }

    #[test]
    fn denied_and_collision_never_enable_or_stop_an_unowned_session() {
        for (code, status) in [
            (5, "access_denied"),
            (183, "session_name_collision"),
            (87, "start_failed"),
        ] {
            let (mut trace, calls) = fake(code, 0, vec![], 0);
            let report = trace.finish();
            drop(trace);
            assert_eq!(report.status, status);
            assert_eq!(report.cleanup_status, "not_created");
            assert_eq!(report.start_error, Some(code));
            let calls = calls.lock().unwrap();
            assert_eq!((calls.start, calls.enable, calls.stop), (1, 0, 0));
        }
    }
    #[test]
    fn enable_failure_stops_only_the_created_handle_once() {
        let (mut trace, calls) = fake(0, 5, vec![], 0);
        let report = trace.finish();
        drop(trace);
        assert_eq!(report.status, "enable_failed");
        assert_eq!(report.enable_error, Some(5));
        assert_eq!(report.cleanup_status, "stopped");
        let calls = calls.lock().unwrap();
        assert_eq!(calls.stop, 1);
        assert!(calls.handles.iter().all(|h| *h == 71));
    }
    #[test]
    fn raw_success_lost_statistics_and_stop_failure_never_become_display_success() {
        for (stops, lost, status, cleanup, count) in [
            (vec![0], 0, "raw_capture_unresolved", "stopped", 1),
            (vec![0], 3, "raw_events_lost", "stopped", 1),
            (vec![5, 0], 0, "raw_capture_unresolved", "stopped", 2),
            (
                vec![5, 5],
                0,
                "capture_incomplete",
                "stop_failed_session_may_remain",
                2,
            ),
            (
                vec![4201],
                0,
                "capture_incomplete",
                "already_ended_statistics_unavailable",
                1,
            ),
        ] {
            let (mut trace, calls) = fake(0, 0, stops, lost);
            let report = trace.finish();
            drop(trace);
            assert_eq!(report.status, status);
            assert_eq!(report.cleanup_status, cleanup);
            assert_eq!(calls.lock().unwrap().stop, count);
        }
    }
    #[test]
    fn early_drop_stops_and_invalid_paths_never_reach_native_api() {
        let (trace, calls) = fake(0, 0, vec![], 0);
        drop(trace);
        assert_eq!(calls.lock().unwrap().stop, 1);
        assert!(Properties::new("bad\0name", std::path::Path::new("C:/x")).is_err());
        assert!(Properties::new("test", std::path::Path::new(&"x".repeat(1024))).is_err());
        let properties = Properties::new("test", std::path::Path::new("C:/x")).unwrap();
        assert_eq!(properties.properties.MaximumFileSize, 32);
        assert_eq!(
            properties.properties.LogFileMode,
            EVENT_TRACE_FILE_MODE_SEQUENTIAL | EVENT_TRACE_NO_PER_PROCESSOR_BUFFERING
        );
        let a = qpc().unwrap();
        let b = qpc().unwrap();
        assert!(b >= a && qpc_frequency().unwrap() > 0);
    }
}
