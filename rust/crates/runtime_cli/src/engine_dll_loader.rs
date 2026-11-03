use runtime_player_winit::engine_dll_execution::{
    EngineRuntimeExecutionRequest, ENGINE_RUNTIME_EXECUTION_CAPABILITY,
    ENGINE_RUNTIME_EXECUTION_MAX_BYTES, ENGINE_RUNTIME_EXECUTION_MIN_API_MINOR,
};
use runtime_player_winit::NativeWindowHostReport;
use std::fs;
use std::io::Read;
use std::path::Path;
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GetProcAddress, LoadLibraryExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_PIN, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ApiInfo {
    pub struct_size: u32,
    pub api_major: u32,
    pub api_minor: u32,
    pub capabilities: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RunResult {
    pub struct_size: u32,
    pub status: i32,
    pub frames_completed: u64,
}

struct Library(HMODULE);

impl Library {
    fn open(path: &Path) -> Result<Self, String> {
        use std::os::windows::ffi::OsStrExt;
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let handle = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        if handle.is_null() {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(Self(handle))
        }
    }

    fn info(&self) -> Result<ApiInfo, String> {
        let symbol =
            unsafe { GetProcAddress(self.0, c"aife_engine_runtime_api_info_v1".as_ptr().cast()) }
                .ok_or("engine API symbol missing")?;
        let entry: extern "C" fn() -> ApiInfo = unsafe { std::mem::transmute(symbol) };
        let info = entry();
        if info.struct_size < std::mem::size_of::<ApiInfo>() as u32 || info.api_major != 1 {
            return Err(format!(
                "unsupported engine ABI major {} or truncated API info",
                info.api_major
            ));
        }
        Ok(info)
    }

    fn pin_for_execution(&self) -> Result<(), String> {
        let mut pinned = std::ptr::null_mut();
        // The module address identifies the exact loaded image, independent of
        // path aliases. Native workers may outlive bounded object shutdown.
        let ok = unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                self.0.cast(),
                &mut pinned,
            )
        };
        if ok == 0 {
            Err(format!(
                "cannot retain executing engine module: {}",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.0);
        }
    }
}

pub fn load_and_probe(path: &Path) -> Result<ApiInfo, String> {
    Library::open(path)?.info()
}

pub fn require_execution_api(info: ApiInfo) -> Result<(), String> {
    if info.api_minor < ENGINE_RUNTIME_EXECUTION_MIN_API_MINOR
        || info.capabilities & ENGINE_RUNTIME_EXECUTION_CAPABILITY == 0
    {
        return Err("engine runtime lacks windowed/semantic execution API; rebuild or update engine_runtime.dll".into());
    }
    Ok(())
}

/// Execute inside the loaded DLL. The executed image remains mapped until process
/// exit because bounded shutdown can detach native workers. Probes still unload.
pub fn execute(
    path: &Path,
    request: &EngineRuntimeExecutionRequest,
) -> Result<NativeWindowHostReport, String> {
    let bytes = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    if bytes.len() > ENGINE_RUNTIME_EXECUTION_MAX_BYTES {
        return Err("engine execution request exceeds its byte limit".into());
    }
    let library = Library::open(path)?;
    require_execution_api(library.info()?)?;
    let symbol =
        unsafe { GetProcAddress(library.0, c"aife_engine_runtime_execute_v1".as_ptr().cast()) }
            .ok_or("engine runtime execution symbol missing")?;
    let entry: unsafe extern "C" fn(*const u8, usize) -> RunResult =
        unsafe { std::mem::transmute(symbol) };
    // Prevent an old report from impersonating this execution, including when
    // the DLL rejects the request before it can write a diagnostic report.
    match fs::remove_file(&request.report_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot clear prior engine report: {error}")),
    }
    library.pin_for_execution()?;
    let result = unsafe { entry(bytes.as_ptr(), bytes.len()) };
    if result.struct_size < std::mem::size_of::<RunResult>() as u32 {
        return Err("engine runtime returned a truncated execution result".into());
    }
    let report_bytes = read_report_bytes(&request.report_path, report_byte_limit(request))
        .map_err(|error| format!("engine runtime report (status {}): {error}", result.status))?;
    let report: NativeWindowHostReport = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("engine runtime report invalid: {error}"))?;
    validate_report(&report, result, request)?;
    Ok(report)
}

fn report_byte_limit(request: &EngineRuntimeExecutionRequest) -> usize {
    if request.request.performance_sample_frames == 0 {
        return 4 * 1024 * 1024;
    }
    // A ten-minute Trace includes ~39k CPU/GPU/owner records (~10 KiB each
    // in current pretty JSON). Scale only the explicit capture, with a hard cap.
    // Summary and a 300-frame filtered Trace retain the existing 128 MiB bound.
    let policy = request.request.measurement_policy();
    let frames = if policy.level == runtime_player_winit::measurement::MeasurementLevel::Trace {
        let end = policy
            .end_frame
            .unwrap_or(request.request.frame_limit)
            .min(request.request.frame_limit);
        end.saturating_sub(policy.first_frame.unwrap_or(0))
    } else {
        0
    };
    frames
        .saturating_mul(16 * 1024)
        .clamp(128 * 1024 * 1024, 512 * 1024 * 1024) as usize
}

fn read_report_bytes(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut report_bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(limit as u64 + 1).read_to_end(&mut report_bytes))
        .map_err(|error| format!("missing: {error}"))?;
    if report_bytes.len() > limit {
        return Err("engine runtime report exceeds its byte limit".into());
    }
    Ok(report_bytes)
}

fn validate_report(
    report: &NativeWindowHostReport,
    result: RunResult,
    request: &EngineRuntimeExecutionRequest,
) -> Result<(), String> {
    if report.schema_version != runtime_player_winit::NATIVE_WINDOW_HOST_REPORT_SCHEMA_VERSION
        || report.runtime_package_path != request.request.runtime_package_path.display().to_string()
        || report.mode != request.request.mode
        || (result.status == 0) != (report.exit_code == 0)
        || report.frames_completed != result.frames_completed
        || result.status == 0
            && !report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "engine_runtime.execution_owner")
    {
        return Err("engine runtime execution result does not match its report".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_performance_report_can_exceed_normal_report_limit() {
        let root = std::env::temp_dir().join(format!("aui-report-limit-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("large.json");
        let bytes = vec![b' '; 4 * 1024 * 1024 + 1];
        fs::write(&path, &bytes).unwrap();
        let mut request = EngineRuntimeExecutionRequest {
            request: runtime_player_winit::NativePlayerWindowRunRequest::windowed("package"),
            scenario: None,
            report_path: path.clone(),
            capture_directory: None,
        };
        assert!(read_report_bytes(&path, report_byte_limit(&request)).is_err());
        request.request.performance_sample_frames = 2048;
        let actual = read_report_bytes(&path, report_byte_limit(&request));
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&root).unwrap();
        assert_eq!(actual.unwrap(), bytes);
    }

    #[test]
    fn long_trace_budget_covers_declared_frames_without_unbounding_normal_or_filtered_reports() {
        let mut request = EngineRuntimeExecutionRequest {
            request: runtime_player_winit::NativePlayerWindowRunRequest::windowed("package"),
            scenario: None,
            report_path: "report.json".into(),
            capture_directory: None,
        };
        request.request.performance_sample_frames = 100000;
        request.request.frame_limit = 100000;
        request.request.measurement = Some(runtime_player_winit::measurement::MeasurementPolicy {
            level: runtime_player_winit::measurement::MeasurementLevel::Trace,
            native_events: false,
            allocation_frame: None,
            allocation_end_frame: None,
            allocation_summary: false,
            first_frame: None,
            end_frame: None,

            cpu_probe: false,
        });
        assert!(
            report_byte_limit(&request) >= 384 * 1024 * 1024,
            "ten-minute raw CPU/GPU/owner trace exceeds the old 128 MiB cap"
        );
        assert!(report_byte_limit(&request) <= 512 * 1024 * 1024);
        request.request.measurement.as_mut().unwrap().first_frame = Some(1801);
        request.request.measurement.as_mut().unwrap().end_frame = Some(2101);
        assert_eq!(report_byte_limit(&request), 128 * 1024 * 1024);
        request.request.performance_sample_frames = 0;
        assert_eq!(report_byte_limit(&request), 4 * 1024 * 1024);
    }
    #[test]
    fn report_reader_still_rejects_limit_plus_one() {
        let root = std::env::temp_dir().join(format!("aui-report-bound-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("bounded.json");
        fs::write(&path, b"12345").unwrap();
        assert_eq!(read_report_bytes(&path, 5).unwrap(), b"12345");
        assert!(read_report_bytes(&path, 4)
            .unwrap_err()
            .contains("byte limit"));
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&root).unwrap();
    }

    #[test]
    fn old_headless_api_cannot_claim_windowed_or_semantic_execution() {
        let mut info = ApiInfo {
            struct_size: std::mem::size_of::<ApiInfo>() as u32,
            api_major: 1,
            api_minor: 1,
            capabilities: 0b1_1111,
        };
        assert!(require_execution_api(info).unwrap_err().contains("update"));
        info.api_minor = ENGINE_RUNTIME_EXECUTION_MIN_API_MINOR;
        assert!(require_execution_api(info).is_err());
        info.capabilities |= ENGINE_RUNTIME_EXECUTION_CAPABILITY;
        require_execution_api(info).unwrap();
    }

    #[test]
    fn dll_result_requires_matching_report_identity_and_actual_owner() {
        let request = EngineRuntimeExecutionRequest {
            request: runtime_player_winit::NativePlayerWindowRunRequest::windowed("package"),
            scenario: None,
            report_path: "report.json".into(),
            capture_directory: None,
        };
        let mut report = NativeWindowHostReport::base(&request.request);
        report.exit_code = 0;
        report.frames_completed = 1;
        let result = RunResult {
            struct_size: std::mem::size_of::<RunResult>() as u32,
            status: 0,
            frames_completed: 1,
        };
        assert!(validate_report(&report, result, &request).is_err());
        report
            .diagnostics
            .push(runtime_player_winit::NativeWindowHostDiagnostic {
                severity: runtime_player_winit::NativeWindowHostDiagnosticSeverity::Info,
                code: "engine_runtime.execution_owner".into(),
                layer: "engine_runtime.dll".into(),
                message: "Executed in DLL".into(),
                path: None,
            });
        validate_report(&report, result, &request).unwrap();
        report.runtime_package_path = "different-package".into();
        assert!(validate_report(&report, result, &request).is_err());
        report.runtime_package_path = "package".into();
        report.schema_version = "wrong-report".into();
        assert!(validate_report(&report, result, &request).is_err());
    }
}
