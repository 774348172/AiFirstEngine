use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectFolderDialogPurpose {
    OpenProject,
    CreateProject,
    OpenRuntimePackage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFolderDialogRequest {
    pub purpose: ProjectFolderDialogPurpose,
    pub title: String,
    pub initial_directory: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectFolderDialogResponse {
    Selected { path: String },
    Cancelled,
    Unavailable { diagnostic: String },
}

pub trait ProjectLocationDialogService {
    fn pick_folder(&mut self, request: ProjectFolderDialogRequest) -> ProjectFolderDialogResponse;
}

#[derive(Default)]
pub struct HeadlessFolderDialogBackend {
    open_project_paths: Vec<String>,
    create_project_paths: Vec<String>,
    open_runtime_package_paths: Vec<String>,
    pub requests: Vec<ProjectFolderDialogRequest>,
}

impl HeadlessFolderDialogBackend {
    pub fn with_open_project_path(path: impl Into<String>) -> Self {
        Self {
            open_project_paths: vec![path.into()],
            create_project_paths: Vec::new(),
            open_runtime_package_paths: Vec::new(),
            requests: Vec::new(),
        }
    }

    pub fn with_create_project_path(path: impl Into<String>) -> Self {
        Self {
            open_project_paths: Vec::new(),
            create_project_paths: vec![path.into()],
            open_runtime_package_paths: Vec::new(),
            requests: Vec::new(),
        }
    }

    pub fn with_open_runtime_package_path(path: impl Into<String>) -> Self {
        Self {
            open_project_paths: Vec::new(),
            create_project_paths: Vec::new(),
            open_runtime_package_paths: vec![path.into()],
            requests: Vec::new(),
        }
    }

    pub fn push_open_project_path(&mut self, path: impl Into<String>) {
        self.open_project_paths.push(path.into());
    }

    pub fn push_create_project_path(&mut self, path: impl Into<String>) {
        self.create_project_paths.push(path.into());
    }

    pub fn push_open_runtime_package_path(&mut self, path: impl Into<String>) {
        self.open_runtime_package_paths.push(path.into());
    }
}

impl ProjectLocationDialogService for HeadlessFolderDialogBackend {
    fn pick_folder(&mut self, request: ProjectFolderDialogRequest) -> ProjectFolderDialogResponse {
        self.requests.push(request.clone());
        let queue = match request.purpose {
            ProjectFolderDialogPurpose::OpenProject => &mut self.open_project_paths,
            ProjectFolderDialogPurpose::CreateProject => &mut self.create_project_paths,
            ProjectFolderDialogPurpose::OpenRuntimePackage => &mut self.open_runtime_package_paths,
        };
        if queue.is_empty() {
            ProjectFolderDialogResponse::Cancelled
        } else {
            ProjectFolderDialogResponse::Selected {
                path: queue.remove(0),
            }
        }
    }
}

pub struct NativeFolderDialogBackend;

impl ProjectLocationDialogService for NativeFolderDialogBackend {
    fn pick_folder(&mut self, request: ProjectFolderDialogRequest) -> ProjectFolderDialogResponse {
        pick_native_folder(request)
    }
}

pub fn default_project_dialog_initial_directory() -> PathBuf {
    let env_directory = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let user_profile = env_directory("USERPROFILE");
    let candidates = [
        user_profile.as_ref().map(|path| path.join("Documents")),
        user_profile,
        env_directory("HOME"),
        std::env::current_dir().ok(),
        Some(std::env::temp_dir()),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| path.is_absolute() && path.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(any(feature = "real-window", test))]
fn validate_project_dialog_initial_directory(path: &std::path::Path) -> Result<(), String> {
    let diagnostic = if path.as_os_str().is_empty() {
        Some("project.dialog.initial_directory_empty")
    } else if !path.is_absolute() {
        Some("project.dialog.initial_directory_not_absolute")
    } else if !path.exists() {
        Some("project.dialog.initial_directory_missing")
    } else if !path.is_dir() {
        Some("project.dialog.initial_directory_not_directory")
    } else if path.to_str().is_none() {
        Some("project.dialog.initial_directory_not_representable")
    } else {
        None
    };
    match diagnostic {
        Some(code) => Err(format!("{code}: {}", path.display())),
        None => Ok(()),
    }
}

#[cfg(any(feature = "real-window", test))]
trait NativeFolderDialogDriver {
    fn pick_folder(
        &mut self,
        title: &str,
        initial_directory: &std::path::Path,
    ) -> Result<Option<PathBuf>, String>;
}

#[cfg(all(feature = "real-window", not(target_os = "windows")))]
struct RfdNativeFolderDialogDriver;

#[cfg(all(feature = "real-window", not(target_os = "windows")))]
impl NativeFolderDialogDriver for RfdNativeFolderDialogDriver {
    fn pick_folder(
        &mut self,
        title: &str,
        initial_directory: &std::path::Path,
    ) -> Result<Option<PathBuf>, String> {
        Ok(rfd::FileDialog::new()
            .set_title(title)
            .set_directory(initial_directory)
            .pick_folder())
    }
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
struct WindowsNativeFolderDialogDriver;

#[cfg(all(feature = "real-window", target_os = "windows"))]
impl NativeFolderDialogDriver for WindowsNativeFolderDialogDriver {
    fn pick_folder(
        &mut self,
        title: &str,
        initial_directory: &std::path::Path,
    ) -> Result<Option<PathBuf>, String> {
        pick_windows_folder(title, initial_directory)
    }
}

#[cfg(any(feature = "real-window", test))]
fn pick_native_folder_with_driver(
    request: ProjectFolderDialogRequest,
    driver: &mut dyn NativeFolderDialogDriver,
) -> ProjectFolderDialogResponse {
    if let Err(diagnostic) = validate_project_dialog_initial_directory(&request.initial_directory) {
        return ProjectFolderDialogResponse::Unavailable { diagnostic };
    }
    match driver.pick_folder(&request.title, &request.initial_directory) {
        Ok(Some(path)) => match path.into_os_string().into_string() {
            Ok(path) => ProjectFolderDialogResponse::Selected { path },
            Err(_) => ProjectFolderDialogResponse::Unavailable {
                diagnostic: "project.dialog.selected_path_not_representable".to_string(),
            },
        },
        Ok(None) => ProjectFolderDialogResponse::Cancelled,
        Err(diagnostic) => ProjectFolderDialogResponse::Unavailable { diagnostic },
    }
}

#[cfg(feature = "real-window")]
fn pick_native_folder(request: ProjectFolderDialogRequest) -> ProjectFolderDialogResponse {
    #[cfg(target_os = "windows")]
    let mut driver = WindowsNativeFolderDialogDriver;
    #[cfg(not(target_os = "windows"))]
    let mut driver = RfdNativeFolderDialogDriver;
    pick_native_folder_with_driver(request, &mut driver)
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
struct WindowsComApartment;

#[cfg(all(feature = "real-window", target_os = "windows"))]
impl WindowsComApartment {
    fn initialize() -> Result<Self, String> {
        use windows::Win32::System::Com::{
            CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
        };

        let result =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        result
            .ok()
            .map_err(|error| windows_dialog_error("com_init", error))?;
        Ok(Self)
    }
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
impl Drop for WindowsComApartment {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
fn windows_shell_parsing_path(initial_directory: &std::path::Path) -> Result<PathBuf, String> {
    use std::path::{Component, Prefix};

    let mut components = initial_directory.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Ok(initial_directory.to_path_buf());
    };

    let mut shell_path = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:\\", char::from(drive))),
        Prefix::VerbatimUNC(server, share) => {
            let mut path = PathBuf::from(r"\\");
            path.push(server);
            path.push(share);
            path
        }
        Prefix::Verbatim(_) | Prefix::DeviceNS(_) => {
            return Err("project.dialog.windows_shell_path_unsupported_prefix".to_string());
        }
        _ => return Ok(initial_directory.to_path_buf()),
    };

    for component in components {
        if component != Component::RootDir {
            shell_path.push(component.as_os_str());
        }
    }
    Ok(shell_path)
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
fn configured_windows_folder_dialog(
    title: &str,
    initial_directory: &std::path::Path,
) -> Result<windows::Win32::UI::Shell::IFileOpenDialog, String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName,
        FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
    };

    let dialog: IFileOpenDialog = unsafe {
        CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| windows_dialog_error("create", error))?
    };
    let options = unsafe {
        dialog
            .GetOptions()
            .map_err(|error| windows_dialog_error("get_options", error))?
    };
    unsafe {
        dialog
            .SetOptions(
                options
                    | FOS_PICKFOLDERS
                    | FOS_FORCEFILESYSTEM
                    | FOS_PATHMUSTEXIST
                    | FOS_NOCHANGEDIR,
            )
            .map_err(|error| windows_dialog_error("set_options", error))?;
        dialog
            .SetTitle(&HSTRING::from(title))
            .map_err(|error| windows_dialog_error("set_title", error))?;
    }

    let initial_shell_path = windows_shell_parsing_path(initial_directory)?;
    let initial_text = initial_shell_path
        .to_str()
        .ok_or_else(|| "project.dialog.initial_directory_not_representable".to_string())?;
    let initial_item: IShellItem = unsafe {
        SHCreateItemFromParsingName(&HSTRING::from(initial_text), None)
            .map_err(|error| windows_dialog_error("create_initial_shell_item", error))?
    };
    unsafe {
        dialog
            .SetFolder(&initial_item)
            .map_err(|error| windows_dialog_error("set_initial_folder", error))?;
    }
    Ok(dialog)
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
fn windows_shell_item_path(
    item: &windows::Win32::UI::Shell::IShellItem,
    stage: &str,
) -> Result<PathBuf, String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::SIGDN_FILESYSPATH;

    let display_name = unsafe {
        item.GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| windows_dialog_error(stage, error))?
    };
    let path_result = unsafe { display_name.to_string() }
        .map(PathBuf::from)
        .map_err(|error| windows_dialog_error("decode_result_path", error.into()));
    unsafe { CoTaskMemFree(Some(display_name.0.cast())) };
    path_result
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
fn is_windows_dialog_cancelled_hresult(code: i32) -> bool {
    const ERROR_CANCELLED_HRESULT: i32 = 0x8007_04C7u32 as i32;
    code == ERROR_CANCELLED_HRESULT
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
fn pick_windows_folder(
    title: &str,
    initial_directory: &std::path::Path,
) -> Result<Option<PathBuf>, String> {
    use windows::Win32::Foundation::HWND;

    let _apartment = WindowsComApartment::initialize()?;
    let dialog = configured_windows_folder_dialog(title, initial_directory)?;

    match unsafe { dialog.Show(HWND::default()) } {
        Ok(()) => {}
        Err(error) if is_windows_dialog_cancelled_hresult(error.code().0) => return Ok(None),
        Err(error) => return Err(windows_dialog_error("show", error)),
    }

    let result = unsafe {
        dialog
            .GetResult()
            .map_err(|error| windows_dialog_error("get_result", error))?
    };
    windows_shell_item_path(&result, "get_result_path").map(Some)
}

#[cfg(all(feature = "real-window", target_os = "windows"))]
fn windows_dialog_error(stage: &str, error: windows::core::Error) -> String {
    format!(
        "project.dialog.windows_{stage}_failed: 0x{:08X}: {error}",
        error.code().0 as u32
    )
}

#[cfg(not(feature = "real-window"))]
fn pick_native_folder(_request: ProjectFolderDialogRequest) -> ProjectFolderDialogResponse {
    ProjectFolderDialogResponse::Unavailable {
        diagnostic: "project.dialog.native_backend_not_available".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        unique_temp_root_for_stamp(stamp)
    }

    fn unique_temp_root_for_stamp(stamp: u128) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static TEMP_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let sequence = TEMP_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "native-dialog-initial-directory-{}-{stamp}-{sequence}",
            std::process::id(),
        ))
    }

    #[test]
    fn native_dialog_temp_roots_are_unique_within_same_clock_tick() {
        assert_ne!(unique_temp_root_for_stamp(1), unique_temp_root_for_stamp(1));
    }

    #[test]
    fn native_dialog_rejects_invalid_initial_directory_without_fallback() {
        let root = unique_temp_root();
        fs::create_dir_all(&root).expect("create valid directory");
        let missing = root.join("missing");
        let file = root.join("file.txt");
        fs::write(&file, "not a directory").expect("create file fixture");

        assert!(validate_project_dialog_initial_directory(&root).is_ok());
        for (path, code) in [
            (PathBuf::new(), "project.dialog.initial_directory_empty"),
            (
                PathBuf::from("relative/project-root"),
                "project.dialog.initial_directory_not_absolute",
            ),
            (missing, "project.dialog.initial_directory_missing"),
            (file, "project.dialog.initial_directory_not_directory"),
        ] {
            let error = validate_project_dialog_initial_directory(&path)
                .expect_err("invalid initial directory must fail closed");
            assert!(error.starts_with(code), "unexpected diagnostic: {error}");
        }

        fs::remove_dir_all(root).expect("remove dialog fixture");
    }

    #[test]
    fn native_dialog_driver_receives_exact_valid_initial_directory() {
        #[derive(Default)]
        struct RecordingDriver {
            calls: Vec<(String, PathBuf)>,
        }

        impl NativeFolderDialogDriver for RecordingDriver {
            fn pick_folder(
                &mut self,
                title: &str,
                initial_directory: &std::path::Path,
            ) -> Result<Option<PathBuf>, String> {
                self.calls
                    .push((title.to_string(), initial_directory.to_path_buf()));
                Ok(None)
            }
        }

        let root = unique_temp_root();
        fs::create_dir_all(&root).expect("create valid directory");
        let mut driver = RecordingDriver::default();
        let response = pick_native_folder_with_driver(
            ProjectFolderDialogRequest {
                purpose: ProjectFolderDialogPurpose::CreateProject,
                title: "Create Project".to_string(),
                initial_directory: root.clone(),
            },
            &mut driver,
        );

        assert_eq!(response, ProjectFolderDialogResponse::Cancelled);
        assert_eq!(
            driver.calls,
            vec![("Create Project".to_string(), root.clone())]
        );

        let invalid = pick_native_folder_with_driver(
            ProjectFolderDialogRequest {
                purpose: ProjectFolderDialogPurpose::CreateProject,
                title: "Create Project".to_string(),
                initial_directory: PathBuf::from("relative"),
            },
            &mut driver,
        );
        assert!(matches!(
            invalid,
            ProjectFolderDialogResponse::Unavailable { .. }
        ));
        assert_eq!(driver.calls.len(), 1);
        fs::remove_dir_all(root).expect("remove dialog fixture");
    }

    #[cfg(all(feature = "real-window", target_os = "windows"))]
    #[test]
    fn windows_shell_parsing_path_translates_only_filesystem_verbatim_prefixes() {
        assert_eq!(
            windows_shell_parsing_path(std::path::Path::new(r"\\?\C:\projects\game")).unwrap(),
            PathBuf::from(r"C:\projects\game")
        );
        assert_eq!(
            windows_shell_parsing_path(std::path::Path::new(r"\\?\UNC\server\share\projects\game"))
                .unwrap(),
            PathBuf::from(r"\\server\share\projects\game")
        );
        assert_eq!(
            windows_shell_parsing_path(std::path::Path::new(r"C:\projects\game")).unwrap(),
            PathBuf::from(r"C:\projects\game")
        );
        assert_eq!(
            windows_shell_parsing_path(std::path::Path::new(
                r"\\?\GLOBALROOT\Device\HarddiskVolume1"
            ))
            .unwrap_err(),
            "project.dialog.windows_shell_path_unsupported_prefix"
        );
    }

    #[cfg(all(feature = "real-window", target_os = "windows"))]
    #[test]
    fn windows_dialog_only_maps_explicit_error_cancelled_hresult_to_cancelled() {
        assert!(is_windows_dialog_cancelled_hresult(0x8007_04C7u32 as i32));
        assert!(!is_windows_dialog_cancelled_hresult(0x8007_0005u32 as i32));
        assert!(!is_windows_dialog_cancelled_hresult(0));
    }

    #[cfg(all(feature = "real-window", target_os = "windows"))]
    #[test]
    fn windows_dialog_configures_exact_initial_directory_before_show() {
        let root = unique_temp_root();
        fs::create_dir_all(&root).expect("create Windows dialog smoke directory");
        let expected = fs::canonicalize(&root).expect("canonicalize expected dialog directory");
        let smoke_root = root.clone();

        let observed = std::thread::spawn(move || -> Result<PathBuf, String> {
            let _apartment = WindowsComApartment::initialize()?;
            let dialog = configured_windows_folder_dialog("Folder Dialog Smoke", &smoke_root)?;
            let folder = unsafe {
                dialog
                    .GetFolder()
                    .map_err(|error| windows_dialog_error("smoke_get_folder", error))?
            };
            windows_shell_item_path(&folder, "smoke_get_folder_path")
        })
        .join()
        .expect("Windows dialog smoke thread should not panic")
        .expect("Windows dialog should accept exact initial directory");
        let observed = fs::canonicalize(observed).expect("canonicalize observed dialog directory");

        assert_eq!(observed, expected);
        fs::remove_dir_all(root).expect("remove Windows dialog smoke directory");
    }

    #[cfg(all(feature = "real-window", target_os = "windows"))]
    #[test]
    #[ignore = "local-only actual Windows folder dialog smoke"]
    fn windows_dialog_show_starts_in_exact_initial_directory() {
        use std::time::{Duration, Instant};
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_CLOSE};

        let root = unique_temp_root();
        fs::create_dir_all(&root).expect("create actual dialog smoke directory");
        let expected = fs::canonicalize(&root).expect("canonicalize actual smoke directory");
        let smoke_root = root.clone();
        let title = format!(
            "AI First Engine Folder Dialog Smoke {}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be monotonic")
                .as_nanos()
        );
        let closer_title = title.clone();

        let observed = std::thread::spawn(move || -> Result<PathBuf, String> {
            let _apartment = WindowsComApartment::initialize()?;
            let dialog = configured_windows_folder_dialog(&title, &smoke_root)?;
            let closer = std::thread::spawn(move || -> Result<(), String> {
                let mut title_wide = closer_title.encode_utf16().collect::<Vec<_>>();
                title_wide.push(0);
                let deadline = Instant::now() + Duration::from_secs(10);
                while Instant::now() < deadline {
                    let window = unsafe { FindWindowW(None, PCWSTR(title_wide.as_ptr())) };
                    if let Ok(window) = window {
                        unsafe {
                            PostMessageW(window, WM_CLOSE, WPARAM(0), LPARAM(0))
                                .map_err(|error| windows_dialog_error("smoke_close", error))?;
                        }
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err("project.dialog.windows_smoke_window_not_found".to_string())
            });

            match unsafe { dialog.Show(HWND::default()) } {
                Err(error) if is_windows_dialog_cancelled_hresult(error.code().0) => {}
                Ok(()) => {
                    return Err("project.dialog.windows_smoke_unexpected_selection".to_string());
                }
                Err(error) => return Err(windows_dialog_error("smoke_show", error)),
            }
            closer
                .join()
                .map_err(|_| "project.dialog.windows_smoke_closer_panicked".to_string())??;
            let folder = unsafe {
                dialog
                    .GetFolder()
                    .map_err(|error| windows_dialog_error("smoke_get_shown_folder", error))?
            };
            windows_shell_item_path(&folder, "smoke_get_shown_folder_path")
        })
        .join()
        .expect("actual dialog smoke thread should not panic")
        .expect("actual Windows dialog smoke should complete");
        let observed = fs::canonicalize(observed).expect("canonicalize shown dialog directory");

        assert_eq!(observed, expected);
        fs::remove_dir_all(root).expect("remove actual dialog smoke directory");
    }
}
