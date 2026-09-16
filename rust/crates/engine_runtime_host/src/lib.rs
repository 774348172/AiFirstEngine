use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use engine_runtime::runtime_package::load_runtime_package;
use runtime_player_winit::engine_dll_execution::{
    EngineRuntimeExecutionRequest, ENGINE_RUNTIME_EXECUTION_CAPABILITY,
    ENGINE_RUNTIME_EXECUTION_MAX_BYTES, ENGINE_RUNTIME_EXECUTION_MIN_API_MINOR,
};
use runtime_player_winit::semantic_outcome::PlaytestTarget;
use runtime_player_winit::{
    NativePlayerWindowRunMode, NativePlayerWindowRunRequest, NativeWindowHostDiagnostic,
    NativeWindowHostDiagnosticSeverity, NativeWindowHostReport,
};
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const ENGINE_RUNTIME_API_MAJOR: u32 = 1;
pub const ENGINE_RUNTIME_API_MINOR: u32 = ENGINE_RUNTIME_EXECUTION_MIN_API_MINOR;
pub const ENGINE_RUNTIME_CAPABILITIES: u64 = 0b1_1111 | ENGINE_RUNTIME_EXECUTION_CAPABILITY;
pub const ENGINE_RUNTIME_OK: i32 = 0;
pub const ENGINE_RUNTIME_INVALID_ARGUMENT: i32 = 1;
pub const ENGINE_RUNTIME_EXECUTION_FAILED: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EngineRuntimeApiInfo {
    pub struct_size: u32,
    pub api_major: u32,
    pub api_minor: u32,
    pub capabilities: u64,
}

#[repr(C)]
pub struct EngineRuntimeRunRequestV1 {
    pub struct_size: u32,
    pub package_path: *const u8,
    pub package_path_len: usize,
    pub report_path: *const u8,
    pub report_path_len: usize,
    pub frame_limit: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EngineRuntimeRunResultV1 {
    pub struct_size: u32,
    pub status: i32,
    pub frames_completed: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn aife_engine_runtime_api_info_v1() -> EngineRuntimeApiInfo {
    EngineRuntimeApiInfo {
        struct_size: std::mem::size_of::<EngineRuntimeApiInfo>() as u32,
        api_major: ENGINE_RUNTIME_API_MAJOR,
        api_minor: ENGINE_RUNTIME_API_MINOR,
        capabilities: ENGINE_RUNTIME_CAPABILITIES,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn aife_engine_runtime_start_v1() -> i32 {
    ENGINE_RUNTIME_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn aife_engine_runtime_stop_v1() -> i32 {
    ENGINE_RUNTIME_OK
}

#[unsafe(no_mangle)]
/// Execute an existing native Player request inside the Engine DLL.
///
/// # Safety
/// `bytes` must reference `length` readable bytes for the duration of this call.
/// No allocation or Rust object ownership crosses the DLL boundary.
pub unsafe extern "C" fn aife_engine_runtime_execute_v1(
    bytes: *const u8,
    length: usize,
) -> EngineRuntimeRunResultV1 {
    let result = std::panic::catch_unwind(|| {
        if bytes.is_null() || length == 0 || length > ENGINE_RUNTIME_EXECUTION_MAX_BYTES {
            return (ENGINE_RUNTIME_INVALID_ARGUMENT, 0);
        }
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        let Ok(execution) = serde_json::from_slice::<EngineRuntimeExecutionRequest>(bytes) else {
            return (ENGINE_RUNTIME_INVALID_ARGUMENT, 0);
        };
        execute_request(execution)
    });
    run_result(result.unwrap_or((ENGINE_RUNTIME_EXECUTION_FAILED, 0)))
}

#[unsafe(no_mangle)]
/// Legacy headless entrypoint, preserving the original C ABI layout.
///
/// # Safety
/// The request and both path byte ranges must remain readable until return.
pub unsafe extern "C" fn aife_engine_runtime_run_headless_v1(
    request: *const EngineRuntimeRunRequestV1,
) -> EngineRuntimeRunResultV1 {
    let result = std::panic::catch_unwind(|| {
        let Some(request) = request.as_ref() else {
            return Err((ENGINE_RUNTIME_INVALID_ARGUMENT, 0));
        };
        if request.struct_size < std::mem::size_of::<EngineRuntimeRunRequestV1>() as u32
            || request.package_path.is_null()
            || request.report_path.is_null()
            || request.frame_limit == 0
        {
            return Err((ENGINE_RUNTIME_INVALID_ARGUMENT, 0));
        }
        let package_path = utf8_path(request.package_path, request.package_path_len)
            .map_err(|_| (ENGINE_RUNTIME_INVALID_ARGUMENT, 0))?;
        let report_path = utf8_path(request.report_path, request.report_path_len)
            .map_err(|_| (ENGINE_RUNTIME_INVALID_ARGUMENT, 0))?;
        let mut native_request = NativePlayerWindowRunRequest::headless_surface_gate(&package_path);
        native_request.frame_limit = request.frame_limit;
        Ok(execute_request(EngineRuntimeExecutionRequest {
            request: native_request,
            scenario: None,
            report_path,
            capture_directory: None,
        }))
    });
    match result {
        Ok(Ok((status, frames_completed))) | Ok(Err((status, frames_completed))) => {
            run_result((status, frames_completed))
        }
        Err(_) => run_result((ENGINE_RUNTIME_EXECUTION_FAILED, 0)),
    }
}

fn run_result((status, frames_completed): (i32, u64)) -> EngineRuntimeRunResultV1 {
    EngineRuntimeRunResultV1 {
        struct_size: std::mem::size_of::<EngineRuntimeRunResultV1>() as u32,
        status,
        frames_completed,
    }
}

fn validate_execution(execution: &EngineRuntimeExecutionRequest) -> Result<(), String> {
    let request = &execution.request;
    if request.runtime_package_path.as_os_str().is_empty()
        || execution.report_path.as_os_str().is_empty()
        || request.frame_limit == 0
    {
        return Err("Package/report paths and a nonzero frame limit are required.".into());
    }
    if let Some(scenario) = execution.scenario.as_ref() {
        // Full scenario validation needs the package's ObservationContract and is
        // performed by the existing semantic execution owner after package load.
        let mode_matches = matches!(
            (request.mode, scenario.target),
            (
                NativePlayerWindowRunMode::Windowed,
                PlaytestTarget::WindowsWindowed
            ) | (
                NativePlayerWindowRunMode::HeadlessSurfaceGate,
                PlaytestTarget::WindowsHeadless
            )
        );
        if !mode_matches || request.input_script.is_some() || request.screenshot.enabled {
            return Err(
                "Scenario target must match run mode; scenario owns input and captures.".into(),
            );
        }
        if request.mode == NativePlayerWindowRunMode::Windowed
            && execution
                .capture_directory
                .as_ref()
                .is_none_or(|path| path.as_os_str().is_empty())
        {
            return Err("Windowed semantic execution requires a capture directory.".into());
        }
    }
    Ok(())
}

fn execute_request(execution: EngineRuntimeExecutionRequest) -> (i32, u64) {
    if let Err(message) = validate_execution(&execution) {
        let report = failure_report(
            &execution.request,
            "engine_runtime.request_invalid",
            message,
        );
        return if write_report(&execution.report_path, &report).is_ok() {
            (ENGINE_RUNTIME_INVALID_ARGUMENT, 0)
        } else {
            (ENGINE_RUNTIME_EXECUTION_FAILED, 0)
        };
    }
    let linked_modules = match load_project_runtime_module(&execution.request.runtime_package_path)
    {
        Ok(modules) => modules,
        Err(message) => {
            let report = failure_report(
                &execution.request,
                "engine_runtime.project_module_load_failed",
                message,
            );
            let _ = write_report(&execution.report_path, &report);
            return (ENGINE_RUNTIME_EXECUTION_FAILED, 0);
        }
    };
    let request_for_failure = execution.request.clone();
    let mut report =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match (execution.request.mode, execution.scenario) {
            (NativePlayerWindowRunMode::HeadlessSurfaceGate, None) => {
                runtime_player_winit::run_headless_native_player_from_package_with_linked_modules(
                    execution.request,
                    linked_modules.as_ref(),
                )
            }
            (NativePlayerWindowRunMode::Windowed, None) => {
                runtime_player_winit::run_windowed_native_player_from_package_with_linked_modules(
                    execution.request,
                    linked_modules,
                )
            }
            (NativePlayerWindowRunMode::HeadlessSurfaceGate, Some(scenario)) => {
                runtime_player_winit::run_headless_semantic_playtest_with_linked_modules(
                    execution.request,
                    linked_modules.as_ref(),
                    scenario,
                )
            }
            (NativePlayerWindowRunMode::Windowed, Some(scenario)) => {
                runtime_player_winit::run_windowed_semantic_playtest_with_linked_modules(
                    execution.request,
                    linked_modules,
                    scenario,
                    execution.capture_directory.expect("validated capture directory"),
                )
            }
        }
        }))
        .unwrap_or_else(|_| {
            failure_report(
                &request_for_failure,
                "engine_runtime.execution_panicked",
                "Engine DLL execution panicked.".into(),
            )
        });
    report.diagnostics.push(NativeWindowHostDiagnostic {
        severity: NativeWindowHostDiagnosticSeverity::Info,
        code: "engine_runtime.execution_owner".into(),
        layer: "engine_runtime.dll".into(),
        message: "Native Player execution was owned by Engine Runtime DLL.".into(),
        path: None,
    });
    finish_report(&execution.report_path, &report)
}

fn finish_report(path: &Path, report: &NativeWindowHostReport) -> (i32, u64) {
    let status = if write_report(path, report).is_ok() && report.exit_code == 0 {
        ENGINE_RUNTIME_OK
    } else {
        ENGINE_RUNTIME_EXECUTION_FAILED
    };
    (status, report.frames_completed)
}

fn utf8_path(pointer: *const u8, length: usize) -> Result<PathBuf, String> {
    if pointer.is_null() || length == 0 || length > ENGINE_RUNTIME_EXECUTION_MAX_BYTES {
        return Err("Invalid path byte range.".into());
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let path = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    if path.contains('\0') {
        return Err("Path contains NUL.".into());
    }
    Ok(PathBuf::from(path))
}

fn write_report(path: &Path, report: &NativeWindowHostReport) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(report).map_err(std::io::Error::other)?;
    fs::write(path, bytes)
}

fn failure_report(
    request: &NativePlayerWindowRunRequest,
    code: &str,
    message: String,
) -> NativeWindowHostReport {
    let mut report = NativeWindowHostReport::base(request);
    report.exit_code = 1;
    report.diagnostics.push(NativeWindowHostDiagnostic::error(
        code,
        "engine_runtime.dll",
        message,
    ));
    report
}

fn load_project_runtime_module(package: &Path) -> Result<Arc<LinkedProjectRuntimeSet>, String> {
    if !package.is_dir() {
        return Err(format!(
            "RuntimePackage directory does not exist: {}",
            package.display()
        ));
    }
    let dll = std::env::var_os("AIFE_PROJECT_RUNTIME_DLL")
        .map(PathBuf::from)
        .or_else(|| staged_project_runtime_dll(package));
    let Some(dll) = dll else {
        let is_empty_runtime = load_runtime_package(package).value.is_some_and(|runtime| {
            runtime.manifest.project.runtime_module.module_id
                == engine_runtime::project_runtime_module::EMPTY_PROJECT_RUNTIME_MODULE_ID
        });
        if is_empty_runtime {
            return Ok(Arc::new(LinkedProjectRuntimeSet::explicit_empty()));
        }
        return Err(format!(
            "no Project RuntimeModule DLL found for staged package {}",
            package.display()
        ));
    };
    #[cfg(windows)]
    {
        let modules =
            engine_runtime::project_runtime_native_adapter::linked_project_runtime_set_from_dll(
                &dll,
            )
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        Ok(Arc::new(modules))
    }
    #[cfg(not(windows))]
    {
        let _ = dll;
        Err("project runtime DLL loading is only supported on Windows".to_string())
    }
}

fn staged_project_runtime_dll(package: &Path) -> Option<PathBuf> {
    let module_id = load_runtime_package(package)
        .value?
        .manifest
        .project
        .runtime_module
        .module_id;
    let file_stem = module_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let parent = package.parent().unwrap_or(package);
    [
        parent.join("project_runtime.dll"),
        parent.join("bin").join(format!("{file_stem}.dll")),
        parent.join("bin").join("aife_generated_runtime_glue.dll"),
        package.join("project_runtime.dll"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

#[allow(dead_code)]
fn _keep_c_void_for_abi_docs(_: *const c_void) {}

#[cfg(test)]
mod tests;
