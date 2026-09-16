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
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
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

/// Execute inside the loaded DLL; keep it loaded until its native objects have
/// been destroyed and its report has been read. No static execution fallback.
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
    let result = unsafe { entry(bytes.as_ptr(), bytes.len()) };
    if result.struct_size < std::mem::size_of::<RunResult>() as u32 {
        return Err("engine runtime returned a truncated execution result".into());
    }
    let mut report_bytes = Vec::new();
    fs::File::open(&request.report_path)
        .and_then(|file| {
            file.take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut report_bytes)
        })
        .map_err(|error| {
            format!(
                "engine runtime report missing (status {}): {error}",
                result.status
            )
        })?;
    if report_bytes.len() > 4 * 1024 * 1024 {
        return Err("engine runtime report exceeds its byte limit".into());
    }
    let report: NativeWindowHostReport = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("engine runtime report invalid: {error}"))?;
    validate_report(&report, result, request)?;
    Ok(report)
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
