use crate::headless_app::RealNativeEditorWindowReport;
use crate::{default_native_editor_recent_store_path, default_project_dialog_initial_directory};
#[cfg(feature = "real-window")]
use crate::{EditorReachabilityReportLevel, EditorWidgetTreeSnapshot};
use editor_ui_model::EditorUiModel;
#[cfg(feature = "real-window")]
use editor_wgpu_renderer::RealUiPresentReport;
#[cfg(feature = "real-window")]
use editor_wgpu_renderer::UiRgbaCapture;
use engine_runtime::project_runtime_module::LinkedProjectRuntimeSet;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub const PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT: &str = "--project-editor-handoff-ticket";
pub const PROJECT_EDITOR_HANDOFF_ISOLATED_LAUNCH_ROOT_ENV: &str =
    "AIFE_PROJECT_EDITOR_HANDOFF_ISOLATED_LAUNCH_ROOT";

pub fn project_editor_handoff_ticket_from_args(
    args: impl IntoIterator<Item = OsString>,
) -> Result<Option<PathBuf>, String> {
    let mut values = args.into_iter();
    let mut ticket = None;
    while let Some(argument) = values.next() {
        if argument != PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT {
            continue;
        }
        if ticket.is_some() {
            return Err("project_editor_composition.handoff_ticket_argument_duplicate".to_string());
        }
        let value = values.next().ok_or_else(|| {
            "project_editor_composition.handoff_ticket_argument_missing".to_string()
        })?;
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err("project_editor_composition.handoff_ticket_argument_invalid".to_string());
        }
        ticket = Some(path);
    }
    Ok(ticket)
}

#[cfg(any(feature = "real-window", test))]
pub(crate) fn acknowledge_project_editor_candidate_after_present(
    presented: bool,
    readiness: &mut Option<editor_core::EditorCompositionCandidateReadiness>,
) -> Result<Option<editor_core::ProjectEditorCompositionLaunchReceipt>, String> {
    if !presented {
        return Ok(None);
    }
    readiness
        .take()
        .map(|value| editor_core::acknowledge_editor_composition_candidate(&value))
        .transpose()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealNativeEditorLaunchOptions {
    project_dialog_initial_directory: PathBuf,
    recent_store_path: Option<PathBuf>,
    isolated_project_launch_root: Option<PathBuf>,
}

impl Default for RealNativeEditorLaunchOptions {
    fn default() -> Self {
        Self {
            project_dialog_initial_directory: default_project_dialog_initial_directory(),
            recent_store_path: Some(default_native_editor_recent_store_path()),
            isolated_project_launch_root: None,
        }
    }
}

impl RealNativeEditorLaunchOptions {
    pub fn project_dialog_initial_directory(&self) -> &Path {
        &self.project_dialog_initial_directory
    }

    pub fn recent_store_path(&self) -> Option<&Path> {
        self.recent_store_path.as_deref()
    }

    pub fn isolated_project_launch_root(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        if root.as_os_str().is_empty() {
            return Err("editor_host.isolated_project_launch_root_missing".to_string());
        }
        if !root.is_absolute()
            || root
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(format!(
                "editor_host.isolated_project_launch_root_invalid: {}",
                root.display()
            ));
        }
        let root_metadata = fs::symlink_metadata(root).map_err(|error| {
            format!(
                "editor_host.isolated_project_launch_root_not_directory: {}: {error}",
                root.display()
            )
        })?;
        if !root_metadata.is_dir() || metadata_is_link_or_reparse_point(&root_metadata) {
            return Err(format!(
                "editor_host.isolated_project_launch_root_not_directory: {}",
                root.display()
            ));
        }
        validate_run_root_ancestors(root)?;

        let picker_start = root.join("picker-start");
        let picker_metadata = fs::symlink_metadata(&picker_start).map_err(|error| {
            format!(
                "editor_host.isolated_picker_start_invalid: {}: {error}",
                picker_start.display()
            )
        })?;
        if !picker_metadata.is_dir() || metadata_is_link_or_reparse_point(&picker_metadata) {
            return Err(format!(
                "editor_host.isolated_picker_start_invalid: {}",
                picker_start.display()
            ));
        }
        let canonical_root = fs::canonicalize(root).map_err(|error| {
            format!(
                "editor_host.isolated_project_launch_root_unreadable: {}: {error}",
                root.display()
            )
        })?;
        let canonical_picker = fs::canonicalize(&picker_start).map_err(|error| {
            format!(
                "editor_host.isolated_picker_start_unreadable: {}: {error}",
                picker_start.display()
            )
        })?;
        if canonical_picker.parent() != Some(canonical_root.as_path()) {
            return Err(format!(
                "editor_host.isolated_picker_start_outside_root: {}",
                picker_start.display()
            ));
        }
        let picker_has_entries = picker_start
            .read_dir()
            .map_err(|error| {
                format!(
                    "editor_host.isolated_picker_start_unreadable: {}: {error}",
                    picker_start.display()
                )
            })?
            .next()
            .is_some();
        if picker_has_entries {
            return Err(format!(
                "editor_host.isolated_picker_start_not_empty: {}",
                picker_start.display()
            ));
        }

        let state_root = root.join("state");
        match fs::symlink_metadata(&state_root) {
            Ok(_) => {
                return Err(format!(
                    "editor_host.isolated_recent_state_not_fresh: {}",
                    state_root.display()
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "editor_host.isolated_recent_state_unreadable: {}: {error}",
                    state_root.display()
                ));
            }
        }

        Ok(Self {
            project_dialog_initial_directory: picker_start,
            recent_store_path: Some(state_root.join("editor_recent_projects.json")),
            isolated_project_launch_root: Some(root.to_path_buf()),
        })
    }

    #[cfg(any(feature = "real-window", test))]
    #[cfg(feature = "real-window")]
    fn validate_for_launch(&self) -> Result<(), String> {
        let Some(root) = &self.isolated_project_launch_root else {
            return Ok(());
        };
        let expected = Self::isolated_project_launch_root(root)?;
        if self.project_dialog_initial_directory != expected.project_dialog_initial_directory
            || self.recent_store_path != expected.recent_store_path
        {
            return Err("editor_host.isolated_project_launch_profile_path_mismatch".to_string());
        }
        Ok(())
    }
}

fn validate_run_root_ancestors(root: &Path) -> Result<(), String> {
    for ancestor in root
        .ancestors()
        .skip(1)
        .filter(|path| !path.as_os_str().is_empty())
    {
        let metadata = fs::symlink_metadata(ancestor).map_err(|error| {
            format!(
                "editor_host.isolated_project_launch_root_ancestor_unreadable: {}: {error}",
                ancestor.display()
            )
        })?;
        if metadata_is_link_or_reparse_point(&metadata) {
            return Err(format!(
                "editor_host.isolated_project_launch_root_reparse_component: {}",
                ancestor.display()
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn metadata_is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(target_os = "windows"))]
fn metadata_is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(feature = "real-window")]
#[derive(Debug, Clone)]
pub struct RealNativeEditorAuthorityOptions {
    pub physical_width: u32,
    pub physical_height: u32,
    pub report_level: EditorReachabilityReportLevel,
    pub project_root: Option<PathBuf>,
    pub workspace_layout_store_root: Option<PathBuf>,
    pub click_widget_id: Option<String>,
    pub wheel_delta: Option<i32>,
    pub drag_target_widget_id: Option<String>,
    pub drag_delta: Option<(i32, i32)>,
    pub scenario_path: Option<PathBuf>,
}

#[cfg(feature = "real-window")]
pub struct RealProjectEditorCompositionAuthorityOptions {
    pub authority: RealNativeEditorAuthorityOptions,
    pub linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    pub identity: editor_core::ProjectEditorCompositionIdentity,
}

#[cfg(feature = "real-window")]
pub struct RealNativeEditorCaptureOutcome {
    pub window_report: RealNativeEditorWindowReport,
    pub native_window_id: Option<String>,
    pub screen_rect: Option<(i32, i32, u32, u32)>,
    pub scale_factor: f64,
    pub physical_width: u32,
    pub physical_height: u32,
    pub snapshot: Option<EditorWidgetTreeSnapshot>,
    pub capture: Option<UiRgbaCapture>,
    pub capture_error: Option<String>,
    pub input_replay: Option<crate::EditorInputReplayEvidence>,
    pub present_report: Option<RealUiPresentReport>,
    pub workspace_layout_revision_before: Option<u64>,
    pub workspace_layout_revision_after: Option<u64>,
    pub workspace_drag_preview_observed: bool,
    pub workspace_diagnostics: Vec<String>,
    pub game_view_present_report: Option<editor_core::GameViewPresentReport>,
    pub game_view_capture: Option<editor_wgpu_renderer::EditorViewportTextureReadback>,
    pub active_runtime_after_play: bool,
    pub active_runtime_package_visible: bool,
    pub runtime_inspector_temporary: bool,
    pub project_lifecycle: Option<crate::ProjectEditorCompositionRealLifecycleEvidence>,
    pub production_authority_report: Option<crate::ProductionAuthorityReport>,
}

#[cfg(feature = "real-window")]
#[derive(Debug, Clone)]
pub struct RealWorkspaceAuthorityOptions {
    pub physical_width: u32,
    pub physical_height: u32,
    pub project_root: PathBuf,
    pub workspace_layout_store_root: PathBuf,
    pub scenario_id: String,
}

#[cfg(feature = "real-window")]
pub struct RealWorkspaceAuthorityWindowEvidence {
    pub workspace_window_id: String,
    pub native_window_id: String,
    pub scale_factor: f64,
    pub screen_rect: (i32, i32, u32, u32),
    pub surface_created: bool,
    pub capture: Option<UiRgbaCapture>,
}

#[cfg(feature = "real-window")]
pub struct RealWorkspaceAuthorityOutcome {
    pub window_report: RealNativeEditorWindowReport,
    pub windows: Vec<RealWorkspaceAuthorityWindowEvidence>,
    pub main_window_count: usize,
    pub floating_window_count: usize,
    pub proxy_created: bool,
    pub proxy_destroyed: bool,
    pub resolved_target_token: Option<String>,
    pub layout_revision_before: Option<u64>,
    pub layout_revision_after: Option<u64>,
    pub panel_unique: bool,
    pub workspace_diagnostics: Vec<String>,
    pub input_observed: bool,
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_window() -> RealNativeEditorWindowReport {
    real_native_editor_window::run_real_native_editor_window(
        None,
        RealNativeEditorLaunchOptions::default(),
        crate::default_editor_linked_project_runtimes(),
        None,
        None,
    )
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_window_with_composition(
    linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
) -> RealNativeEditorWindowReport {
    real_native_editor_window::run_real_native_editor_window(
        None,
        RealNativeEditorLaunchOptions::default(),
        linked_project_runtimes,
        None,
        None,
    )
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_window_with_project_composition(
    linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    identity: editor_core::ProjectEditorCompositionIdentity,
) -> RealNativeEditorWindowReport {
    real_native_editor_window::run_real_native_editor_window(
        None,
        RealNativeEditorLaunchOptions::default(),
        linked_project_runtimes,
        Some(identity),
        None,
    )
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_window_with_project_composition_and_handoff(
    linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    identity: editor_core::ProjectEditorCompositionIdentity,
    ticket_path: PathBuf,
) -> RealNativeEditorWindowReport {
    let now_epoch_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let candidate_process_id = std::process::id();
    let current_executable_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            return RealNativeEditorWindowReport::environment_blocked(
                "project-editor-composition-handoff",
                error.to_string(),
            );
        }
    };
    let readiness = match editor_core::prepare_editor_composition_candidate_readiness(
        ticket_path,
        current_executable_path,
        identity.clone(),
        format!("project-editor-{candidate_process_id}-{now_epoch_ms}"),
        candidate_process_id,
        now_epoch_ms,
    ) {
        Ok(readiness) => readiness,
        Err(error) => {
            return RealNativeEditorWindowReport::environment_blocked(
                "project-editor-composition-handoff",
                error,
            );
        }
    };
    let launch_options = match std::env::var_os(PROJECT_EDITOR_HANDOFF_ISOLATED_LAUNCH_ROOT_ENV) {
        Some(root) => match RealNativeEditorLaunchOptions::isolated_project_launch_root(root) {
            Ok(options) => options,
            Err(error) => {
                return RealNativeEditorWindowReport::environment_blocked(
                    "project-editor-composition-handoff-isolation",
                    error,
                );
            }
        },
        None => RealNativeEditorLaunchOptions::default(),
    };
    real_native_editor_window::run_real_native_editor_window(
        None,
        launch_options,
        linked_project_runtimes,
        Some(identity),
        Some(readiness),
    )
}

#[cfg(not(feature = "real-window"))]
pub fn run_real_native_editor_window() -> RealNativeEditorWindowReport {
    RealNativeEditorWindowReport::feature_not_enabled()
}

#[cfg(not(feature = "real-window"))]
pub fn run_real_native_editor_window_with_composition(
    _linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
) -> RealNativeEditorWindowReport {
    RealNativeEditorWindowReport::feature_not_enabled()
}

#[cfg(not(feature = "real-window"))]
pub fn run_real_native_editor_window_with_project_composition(
    _linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    _identity: editor_core::ProjectEditorCompositionIdentity,
) -> RealNativeEditorWindowReport {
    RealNativeEditorWindowReport::feature_not_enabled()
}

#[cfg(not(feature = "real-window"))]
pub fn run_real_native_editor_window_with_project_composition_and_handoff(
    _linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
    _identity: editor_core::ProjectEditorCompositionIdentity,
    _ticket_path: PathBuf,
) -> RealNativeEditorWindowReport {
    RealNativeEditorWindowReport::feature_not_enabled()
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_window_with_model(
    model: EditorUiModel,
) -> RealNativeEditorWindowReport {
    run_real_native_editor_window_with_model_and_options(
        model,
        RealNativeEditorLaunchOptions::default(),
    )
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_window_with_model_and_options(
    model: EditorUiModel,
    options: RealNativeEditorLaunchOptions,
) -> RealNativeEditorWindowReport {
    real_native_editor_window::run_real_native_editor_window(
        Some(model),
        options,
        crate::default_editor_linked_project_runtimes(),
        None,
        None,
    )
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_capture_once(
    physical_width: u32,
    physical_height: u32,
    report_level: EditorReachabilityReportLevel,
) -> RealNativeEditorCaptureOutcome {
    real_native_editor_window::run_real_native_editor_capture_once(
        physical_width,
        physical_height,
        report_level,
    )
}

#[cfg(feature = "real-window")]
pub fn run_real_native_editor_authority(
    options: RealNativeEditorAuthorityOptions,
) -> RealNativeEditorCaptureOutcome {
    real_native_editor_window::run_real_native_editor_authority(options)
}

#[cfg(feature = "real-window")]
pub fn run_real_project_editor_composition_authority(
    options: RealProjectEditorCompositionAuthorityOptions,
) -> RealNativeEditorCaptureOutcome {
    real_native_editor_window::run_real_project_editor_composition_authority(options)
}

#[cfg(feature = "real-window")]
pub fn run_real_workspace_authority(
    options: RealWorkspaceAuthorityOptions,
) -> RealWorkspaceAuthorityOutcome {
    real_native_editor_window::run_real_workspace_authority(options)
}

#[cfg(not(feature = "real-window"))]
pub fn run_real_native_editor_window_with_model(
    _model: EditorUiModel,
) -> RealNativeEditorWindowReport {
    RealNativeEditorWindowReport::feature_not_enabled()
}

#[cfg(not(feature = "real-window"))]
pub fn run_real_native_editor_window_with_model_and_options(
    _model: EditorUiModel,
    _options: RealNativeEditorLaunchOptions,
) -> RealNativeEditorWindowReport {
    RealNativeEditorWindowReport::feature_not_enabled()
}

#[cfg(test)]
mod launch_options_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "native-launch-options-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn project_editor_handoff_launch_input_requires_one_absolute_ticket_path() {
        let ticket = unique_root("handoff-ticket").join("handoff.json");
        assert_eq!(
            project_editor_handoff_ticket_from_args([
                OsString::from("editor.exe"),
                OsString::from(PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT),
                ticket.clone().into_os_string(),
            ])
            .unwrap(),
            Some(ticket.clone())
        );
        assert_eq!(
            project_editor_handoff_ticket_from_args([OsString::from("editor.exe")]).unwrap(),
            None
        );
        assert_eq!(
            project_editor_handoff_ticket_from_args([
                OsString::from("editor.exe"),
                OsString::from(PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT),
            ])
            .unwrap_err(),
            "project_editor_composition.handoff_ticket_argument_missing"
        );
        assert_eq!(
            project_editor_handoff_ticket_from_args([
                OsString::from("editor.exe"),
                OsString::from(PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT),
                ticket.clone().into_os_string(),
                OsString::from(PROJECT_EDITOR_HANDOFF_TICKET_ARGUMENT),
                ticket.into_os_string(),
            ])
            .unwrap_err(),
            "project_editor_composition.handoff_ticket_argument_duplicate"
        );
    }

    #[cfg(target_os = "windows")]
    fn create_directory_link(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .expect("create Windows junction");
        assert!(status.success(), "mklink /J failed with {status}");
    }

    #[cfg(unix)]
    fn create_directory_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(target_os = "windows")]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[test]
    fn isolated_launch_rejects_picker_start_directory_link_escape() {
        let root = unique_root("picker-link-root");
        let external = unique_root("picker-link-external");
        fs::create_dir_all(&root).expect("create isolated root");
        fs::create_dir_all(&external).expect("create external picker target");
        let picker_start = root.join("picker-start");
        create_directory_link(&picker_start, &external);

        let error = RealNativeEditorLaunchOptions::isolated_project_launch_root(&root)
            .expect_err("linked picker start must fail closed");

        assert!(
            error.starts_with("editor_host.isolated_picker_start_invalid")
                || error.starts_with("editor_host.isolated_picker_start_outside_root"),
            "unexpected diagnostic: {error}"
        );
        remove_directory_link(&picker_start);
        fs::remove_dir_all(root).expect("remove isolated root");
        fs::remove_dir_all(external).expect("remove external target");
    }

    #[test]
    fn isolated_launch_rejects_run_root_directory_link_escape() {
        let linked_root = unique_root("linked-run-root");
        let external = unique_root("linked-run-root-external");
        fs::create_dir_all(external.join("picker-start")).expect("create external run root");
        create_directory_link(&linked_root, &external);

        let error = RealNativeEditorLaunchOptions::isolated_project_launch_root(&linked_root)
            .expect_err("linked run root must fail closed");

        assert!(error.starts_with("editor_host.isolated_project_launch_root_not_directory"));
        remove_directory_link(&linked_root);
        fs::remove_dir_all(external).expect("remove external target");
    }

    #[test]
    fn isolated_launch_rejects_directory_link_in_run_root_ancestor() {
        let linked_ancestor = unique_root("linked-ancestor");
        let external = unique_root("linked-ancestor-external");
        let external_run = external.join("run");
        fs::create_dir_all(external_run.join("picker-start"))
            .expect("create external nested run root");
        create_directory_link(&linked_ancestor, &external);
        let root = linked_ancestor.join("run");

        let result = RealNativeEditorLaunchOptions::isolated_project_launch_root(&root);
        remove_directory_link(&linked_ancestor);
        fs::remove_dir_all(external).expect("remove external target");

        let error = result.expect_err("linked ancestor must fail closed");
        assert!(error.starts_with("editor_host.isolated_project_launch_root_reparse_component"));
    }

    #[test]
    fn isolated_launch_rejects_state_directory_link_escape() {
        let root = unique_root("state-link-root");
        let external = unique_root("state-link-external");
        fs::create_dir_all(root.join("picker-start")).expect("create picker start");
        fs::create_dir_all(&external).expect("create external state target");
        let state = root.join("state");
        create_directory_link(&state, &external);

        let error = RealNativeEditorLaunchOptions::isolated_project_launch_root(&root)
            .expect_err("linked state must fail closed");

        assert!(error.starts_with("editor_host.isolated_recent_state_not_fresh"));
        remove_directory_link(&state);
        fs::remove_dir_all(root).expect("remove isolated root");
        fs::remove_dir_all(external).expect("remove external target");
    }
}

#[cfg(feature = "real-window")]
pub(crate) mod real_native_editor_window {
    use super::*;
    use crate::application::NativeEditorApplication;
    #[cfg(test)]
    use crate::application::NativeEditorApplicationReport;
    use crate::config::NativeEditorWindowConfig;
    use crate::config::{physical_to_logical, PhysicalPoint};
    use crate::dialog::NativeFolderDialogBackend;
    use crate::headless_app::{
        RealNativeEditorWindowDiagnostic, RealNativeEditorWindowDiagnosticSeverity,
    };
    use crate::project_manager::ProjectManagerController;
    use crate::reachability_gate::{
        snapshot_widget_tree, EditorReachabilityViewport, EditorWidgetSnapshotContext,
    };
    use crate::window_plan::winit_window_attributes;
    use crate::WorkspacePointerCursor;
    use crate::{EditorFramePublicationError, EditorFramePublicationModule};
    use editor_core::EditorSession;
    #[cfg(feature = "real-wgpu-surface")]
    use editor_core::{
        GameViewPresentDiagnostic, GameViewRuntimeFrame, ProjectPreviewCaptureKind,
        ProjectPreviewFrameReadback, ProjectPreviewFrameTicket, ProjectPreviewPixelFormat,
    };
    use editor_input::{EditorInputEvent, PointerButton};
    use editor_ui_renderer::{
        editor_workspace_rect, UiPoint, UiRect, WorkspaceDisplay, WorkspaceDragWindowFacts,
        WorkspaceWindowId, WorkspaceWindowPlacement,
    };
    use editor_wgpu_renderer::RealWgpuUiRenderer;
    #[cfg(feature = "real-wgpu-surface")]
    use editor_wgpu_renderer::{
        EditorViewportTextureReadback, EditorViewportTextureRegistry, GameViewPublicationReceipt,
    };
    #[cfg(feature = "real-wgpu-surface")]
    use engine_runtime::engine_rhi::RhiBackendDiagnosticSeverity;
    #[cfg(feature = "real-wgpu-surface")]
    use engine_runtime::rhi_command_plan::RhiCommandPlan;
    #[cfg(feature = "real-wgpu-surface")]
    use engine_runtime::wgpu_backend::real::RealWgpuBackend;
    #[cfg(test)]
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::{collections::BTreeMap, collections::BTreeSet};
    use winit::application::ApplicationHandler;
    use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::keyboard::{Key, NamedKey};
    #[cfg(target_os = "windows")]
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::{CursorIcon, Window};

    struct RealWorkspaceWindowState {
        window: Arc<Window>,
        ui_renderer: RealWgpuUiRenderer,
        last_cursor_position: Option<(f32, f32)>,
        scale_factor: f64,
        focused: bool,
    }

    const GAME_VIEW_TICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);
    const GAME_VIEW_MAX_CATCH_UP_TICKS: usize = 8;

    fn advance_game_view_tick_deadline(
        now: std::time::Instant,
        next_tick: &mut std::time::Instant,
    ) -> usize {
        if now < *next_tick {
            return 0;
        }
        let overdue = now.duration_since(*next_tick);
        let due = 1usize.saturating_add(
            (overdue.as_nanos() / GAME_VIEW_TICK_INTERVAL.as_nanos())
                .try_into()
                .unwrap_or(usize::MAX),
        );
        let tick_count = due.min(GAME_VIEW_MAX_CATCH_UP_TICKS);
        *next_tick += GAME_VIEW_TICK_INTERVAL * tick_count as u32;
        if due > GAME_VIEW_MAX_CATCH_UP_TICKS {
            *next_tick = now + GAME_VIEW_TICK_INTERVAL;
        }
        tick_count
    }

    fn reset_game_view_tick_deadline(now: std::time::Instant, next_tick: &mut std::time::Instant) {
        *next_tick = now + GAME_VIEW_TICK_INTERVAL;
    }

    fn earliest_editor_deadline(
        game_view: Option<std::time::Instant>,
        project_composition: Option<std::time::Instant>,
    ) -> Option<std::time::Instant> {
        match (game_view, project_composition) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
            (None, None) => None,
        }
    }

    #[cfg(feature = "real-wgpu-surface")]
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct GameViewGpuResourceIdentity {
        session_id: String,
        target_id: String,
        width: u32,
        height: u32,
        texture_format: String,
        textures: Vec<(String, String, u32, u32, String)>,
        font_bundles: Vec<(String, u64)>,
    }

    #[cfg(feature = "real-wgpu-surface")]
    #[derive(Default)]
    struct GameViewGpuResidencyLedger {
        identity: Option<GameViewGpuResourceIdentity>,
        upload_generation: u64,
        total_texture_upload_count: usize,
    }

    #[cfg(feature = "real-wgpu-surface")]
    impl GameViewGpuResidencyLedger {
        fn requires_upload(&self, identity: &GameViewGpuResourceIdentity) -> bool {
            self.identity.as_ref() != Some(identity)
        }

        fn commit_upload(&mut self, identity: GameViewGpuResourceIdentity, uploaded: usize) {
            self.identity = Some(identity);
            self.upload_generation += 1;
            self.total_texture_upload_count += uploaded;
        }

        fn retire(&mut self) -> Option<String> {
            self.identity.take().map(|identity| identity.session_id)
        }
    }

    #[cfg(feature = "real-wgpu-surface")]
    #[derive(Default)]
    struct GameViewGpuResidentState {
        ledger: GameViewGpuResidencyLedger,
        backend: Option<RealWgpuBackend>,
    }

    #[cfg(feature = "real-wgpu-surface")]
    impl GameViewGpuResidentState {
        fn resource_identity(
            frame: &GameViewRuntimeFrame,
            texture_format: wgpu::TextureFormat,
            runtime_textures: Option<
                &engine_runtime::runtime_texture::RuntimeTextureUploadRegistry,
            >,
            font_bundles: Option<&engine_runtime::font_bundle::RuntimeFontBundleRegistry>,
        ) -> GameViewGpuResourceIdentity {
            let textures = runtime_textures
                .into_iter()
                .flat_map(|registry| registry.uploads())
                .map(|upload| {
                    (
                        upload.asset_id.clone(),
                        upload.payload.source_hash.clone(),
                        upload.payload.width,
                        upload.payload.height,
                        upload.payload.sampler.clone(),
                    )
                })
                .collect();
            let font_bundles = font_bundles
                .into_iter()
                .flat_map(|registry| registry.bundles_by_id.iter())
                .map(|(bundle_id, bundle)| (bundle_id.clone(), bundle.metadata.generation))
                .collect();
            GameViewGpuResourceIdentity {
                session_id: frame.session_id.clone(),
                target_id: frame.target_id.clone(),
                width: frame.width.max(1),
                height: frame.height.max(1),
                texture_format: format!("{texture_format:?}"),
                textures,
                font_bundles,
            }
        }

        fn prepare<'a>(
            &'a mut self,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            texture_format: wgpu::TextureFormat,
            frame: &GameViewRuntimeFrame,
            runtime_textures: Option<
                &engine_runtime::runtime_texture::RuntimeTextureUploadRegistry,
            >,
            font_bundles: Option<&engine_runtime::font_bundle::RuntimeFontBundleRegistry>,
        ) -> Result<&'a mut RealWgpuBackend, String> {
            let identity =
                Self::resource_identity(frame, texture_format, runtime_textures, font_bundles);
            if self.ledger.requires_upload(&identity) || self.backend.is_none() {
                let mut candidate = RealWgpuBackend::from_shared_device_queue(
                    device,
                    queue,
                    texture_format,
                    frame.width.max(1),
                    frame.height.max(1),
                    "editor-gameview-shared-wgpu",
                );
                let mut uploaded = 0;
                if let Some(runtime_textures) = runtime_textures {
                    for upload in runtime_textures.uploads() {
                        candidate.register_rgba8_texture(
                            upload.handle,
                            upload.payload.width,
                            upload.payload.height,
                            &upload.payload.rgba8,
                            &upload.payload.sampler,
                        )?;
                        uploaded += 1;
                    }
                }
                if let Some(font_bundles) = font_bundles {
                    for bundle in font_bundles.bundles_by_id.values() {
                        let report = candidate.register_font_texture_arrays(bundle)?;
                        uploaded += report.registered_page_handles.len();
                    }
                }
                self.backend = Some(candidate);
                self.ledger.commit_upload(identity, uploaded);
            }
            self.backend
                .as_mut()
                .ok_or_else(|| "editor_gameview.gpu_resident_backend_missing".to_string())
        }

        fn retire(&mut self) -> Option<String> {
            let session_id = self.ledger.retire();
            self.backend = None;
            session_id
        }
    }

    pub struct RealNativeEditorApp {
        app: NativeEditorApplication,
        frame_publication: EditorFramePublicationModule,
        #[cfg(feature = "real-wgpu-surface")]
        game_view_gpu_residency: GameViewGpuResidentState,
        windows: BTreeMap<WorkspaceWindowId, RealWorkspaceWindowState>,
        native_to_workspace: BTreeMap<winit::window::WindowId, WorkspaceWindowId>,
        drag_proxy: Option<Arc<Window>>,
        last_drag_screen_pointer: Option<UiPoint>,
        report: RealNativeEditorWindowReport,
        candidate_readiness: Option<editor_core::EditorCompositionCandidateReadiness>,
        pending_handoff_project_open: Option<PathBuf>,
        graceful_exit_requested: Arc<AtomicBool>,
        next_game_view_tick: std::time::Instant,
    }

    impl RealNativeEditorApp {
        #[cfg(test)]
        pub fn new(config: NativeEditorWindowConfig, _model: Option<EditorUiModel>) -> Self {
            Self::new_with_launch_options(config, _model, RealNativeEditorLaunchOptions::default())
        }

        #[cfg(test)]
        pub fn new_with_launch_options(
            config: NativeEditorWindowConfig,
            _model: Option<EditorUiModel>,
            options: RealNativeEditorLaunchOptions,
        ) -> Self {
            Self::try_new_with_launch_options(config, _model, options)
                .expect("real native editor launch options should remain valid before app creation")
        }

        #[cfg(test)]
        pub fn try_new_with_launch_options(
            config: NativeEditorWindowConfig,
            _model: Option<EditorUiModel>,
            options: RealNativeEditorLaunchOptions,
        ) -> Result<Self, String> {
            Self::try_new_with_launch_options_and_gateway_wake(config, _model, options, None)
        }

        #[cfg(test)]
        fn try_new_with_launch_options_and_gateway_wake(
            config: NativeEditorWindowConfig,
            _model: Option<EditorUiModel>,
            options: RealNativeEditorLaunchOptions,
            gateway_wake: Option<ai_tool_gateway::GatewayOwnerThreadWake>,
        ) -> Result<Self, String> {
            Self::try_new_with_launch_options_and_gateway_wake_and_composition(
                config,
                _model,
                options,
                gateway_wake,
                crate::default_editor_linked_project_runtimes(),
                None,
            )
        }

        fn try_new_with_launch_options_and_gateway_wake_and_composition(
            config: NativeEditorWindowConfig,
            _model: Option<EditorUiModel>,
            options: RealNativeEditorLaunchOptions,
            gateway_wake: Option<ai_tool_gateway::GatewayOwnerThreadWake>,
            linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
            project_composition_identity: Option<editor_core::ProjectEditorCompositionIdentity>,
        ) -> Result<Self, String> {
            options.validate_for_launch()?;
            let project_manager = match options.recent_store_path {
                Some(path) => ProjectManagerController::with_recent_store_path(path),
                None => ProjectManagerController::default(),
            };
            let session = match project_composition_identity {
                Some(identity) => EditorSession::with_project_editor_composition(
                    linked_project_runtimes,
                    identity,
                )
                .map_err(|error| error.to_string())?,
                None => EditorSession::with_linked_project_runtimes(linked_project_runtimes),
            };
            let gateway_discovery_root_override = options
                .isolated_project_launch_root
                .as_ref()
                .map(|root| root.join("gateway-discovery"));
            #[allow(unused_mut)]
            let mut app = NativeEditorApplication::with_project_manager_and_dialog_initial_directory_and_gateway(
                    config,
                    session,
                    project_manager,
                    Box::new(NativeFolderDialogBackend),
                    options.project_dialog_initial_directory,
                    gateway_wake,
                    gateway_discovery_root_override,
                );
            let graceful_exit_requested = Arc::new(AtomicBool::new(false));
            #[cfg(not(test))]
            {
                let editor_build_identity = crate::current_editor_build_identity()?;
                let state_root = crate::project_editor_composition_state_root()?;
                crate::install_project_editor_composition_production_services(
                    &mut app,
                    &state_root,
                    editor_core::default_engine_sdk_root(),
                    editor_build_identity,
                    graceful_exit_requested.clone(),
                )?;
            }
            Ok(Self {
                app,
                frame_publication: EditorFramePublicationModule::new(),
                #[cfg(feature = "real-wgpu-surface")]
                game_view_gpu_residency: GameViewGpuResidentState::default(),
                windows: BTreeMap::new(),
                native_to_workspace: BTreeMap::new(),
                drag_proxy: None,
                last_drag_screen_pointer: None,
                report: RealNativeEditorWindowReport::new("winit-wgpu"),
                candidate_readiness: None,
                pending_handoff_project_open: None,
                graceful_exit_requested,
                next_game_view_tick: std::time::Instant::now(),
            })
        }

        fn request_redraw(&self) {
            for state in self.windows.values() {
                state.window.request_redraw();
            }
        }

        fn route_editor_input(
            &mut self,
            workspace_window_id: &WorkspaceWindowId,
            event: EditorInputEvent,
        ) {
            let drag_windows = self.workspace_drag_window_facts();
            let screen_pointer = self.windows.get(workspace_window_id).and_then(|state| {
                let local = state.last_cursor_position?;
                let outer = state.window.outer_position().ok()?;
                Some(UiPoint {
                    x: outer.x as f32 + local.0 * state.scale_factor as f32,
                    y: outer.y as f32 + local.1 * state.scale_factor as f32,
                })
            });
            self.last_drag_screen_pointer = screen_pointer;
            if let Some(state) = self.windows.get(workspace_window_id) {
                let size = state.window.inner_size();
                self.app.prepare_workspace_window_input(
                    workspace_window_id,
                    size.width as f32 / state.scale_factor as f32,
                    size.height as f32 / state.scale_factor as f32,
                    screen_pointer,
                    drag_windows,
                );
            }
            self.report.input_event_count += 1;
            let before_command = self.app.report().last_command_id;
            let app_report = self.app.handle_input_event(event);
            if app_report.last_command_id != before_command {
                self.report.ui_command_count += 1;
            }
            if let Some(state) = self.windows.get(workspace_window_id) {
                let cursor = match self.app.workspace_pointer_cursor() {
                    WorkspacePointerCursor::Default => CursorIcon::Default,
                    WorkspacePointerCursor::ColumnResize => CursorIcon::ColResize,
                    WorkspacePointerCursor::RowResize => CursorIcon::RowResize,
                };
                state.window.set_cursor(cursor);
            }
            self.request_redraw();
        }

        fn workspace_drag_window_facts(&self) -> Vec<WorkspaceDragWindowFacts> {
            self.windows
                .iter()
                .filter_map(|(window_id, state)| {
                    let outer = state.window.outer_position().ok()?;
                    let size = state.window.inner_size();
                    let logical_width = size.width as f32 / state.scale_factor as f32;
                    let logical_height = size.height as f32 / state.scale_factor as f32;
                    Some(WorkspaceDragWindowFacts {
                        window_id: window_id.clone(),
                        screen_rect: UiRect {
                            x: outer.x as f32,
                            y: outer.y as f32,
                            width: size.width as f32,
                            height: size.height as f32,
                        },
                        workspace_rect: editor_workspace_rect(logical_width, logical_height),
                        scale_factor: state.scale_factor as f32,
                    })
                })
                .collect()
        }

        fn reconcile_drag_proxy(&mut self, event_loop: &ActiveEventLoop) {
            if !self.app.workspace_docking().drag_requires_native_proxy() {
                self.drag_proxy = None;
                return;
            }
            let Some(pointer) = self.last_drag_screen_pointer else {
                self.drag_proxy = None;
                return;
            };
            if self.drag_proxy.is_none() {
                match event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Panel")
                        .with_inner_size(winit::dpi::LogicalSize::new(120.0, 28.0))
                        .with_decorations(false)
                        .with_resizable(false)
                        .with_transparent(true)
                        .with_active(false)
                        .with_window_level(winit::window::WindowLevel::AlwaysOnTop),
                ) {
                    Ok(window) => {
                        if let Err(error) = window.set_cursor_hittest(false) {
                            self.report
                                .diagnostics
                                .push(RealNativeEditorWindowDiagnostic {
                                    severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                                    code: "native_drag_proxy_hittest_failed".to_string(),
                                    message: error.to_string(),
                                    source_stage: "winit.drag_proxy.hittest".to_string(),
                                });
                            self.app.cancel_workspace_panel_drag();
                            return;
                        }
                        self.drag_proxy = Some(Arc::new(window));
                    }
                    Err(error) => {
                        self.report
                            .diagnostics
                            .push(RealNativeEditorWindowDiagnostic {
                                severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                                code: "native_drag_proxy_create_failed".to_string(),
                                message: error.to_string(),
                                source_stage: "winit.drag_proxy.create".to_string(),
                            });
                        self.app.cancel_workspace_panel_drag();
                        return;
                    }
                }
            }
            if let Some(proxy) = &self.drag_proxy {
                proxy.set_outer_position(winit::dpi::PhysicalPosition::new(
                    pointer.x.round() as i32 + 16,
                    pointer.y.round() as i32 + 16,
                ));
            }
        }

        fn reconcile_windows(&mut self, event_loop: &ActiveEventLoop) {
            let displays = event_loop
                .available_monitors()
                .enumerate()
                .map(|(index, monitor)| {
                    let position = monitor.position();
                    let size = monitor.size();
                    WorkspaceDisplay {
                        display_id: monitor.name().unwrap_or_else(|| format!("monitor-{index}")),
                        work_area: UiRect {
                            x: position.x as f32,
                            y: position.y as f32,
                            width: size.width as f32,
                            height: size.height as f32,
                        },
                    }
                })
                .collect::<Vec<_>>();
            let plan = self.app.workspace_docking().window_plan(&displays);
            let desired = plan
                .windows
                .iter()
                .map(|entry| entry.window_id.clone())
                .collect::<BTreeSet<_>>();
            let removed = self
                .windows
                .keys()
                .filter(|window_id| !window_id.is_main() && !desired.contains(*window_id))
                .cloned()
                .collect::<Vec<_>>();
            for window_id in removed {
                if let Some(state) = self.windows.remove(&window_id) {
                    self.native_to_workspace.remove(&state.window.id());
                }
            }

            for entry in plan.windows {
                if self.windows.contains_key(&entry.window_id) {
                    continue;
                }
                let attributes = if entry.window_id.is_main() {
                    winit_window_attributes(self.app.config())
                } else {
                    floating_window_attributes(
                        self.app.config(),
                        &entry.window_id,
                        &entry.placement,
                    )
                };
                let window = match event_loop.create_window(attributes) {
                    Ok(window) => Arc::new(window),
                    Err(error) => {
                        self.report
                            .diagnostics
                            .push(RealNativeEditorWindowDiagnostic {
                                severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                                code: "workspace_window_create_failed".to_string(),
                                message: error.to_string(),
                                source_stage: "winit.workspace_window.create".to_string(),
                            });
                        if entry.window_id.is_main() {
                            event_loop.exit();
                            return;
                        }
                        continue;
                    }
                };
                let renderer = match RealWgpuUiRenderer::new(window.clone()) {
                    Ok(renderer) => renderer,
                    Err(error) => {
                        self.report
                            .diagnostics
                            .push(RealNativeEditorWindowDiagnostic {
                                severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                                code: "workspace_surface_create_failed".to_string(),
                                message: error,
                                source_stage: "editor_wgpu_renderer.workspace_window.new"
                                    .to_string(),
                            });
                        if entry.window_id.is_main() {
                            event_loop.exit();
                            return;
                        }
                        continue;
                    }
                };
                self.report.window_created = true;
                self.report.surface_created = true;
                self.report.surface_configured = true;
                self.report.device_created = true;
                self.report
                    .apply_shared_gpu_context_summary(&renderer.shared_context_summary());
                self.report.apply_viewport_texture_registry_state(
                    renderer.viewport_textures().texture_count(),
                    renderer.viewport_textures().lifecycle_event_count(),
                );
                self.report.present_status = "workspace_window_created".to_string();
                let native_id = window.id();
                let scale_factor = window.scale_factor();
                window.request_redraw();
                self.native_to_workspace
                    .insert(native_id, entry.window_id.clone());
                self.windows.insert(
                    entry.window_id,
                    RealWorkspaceWindowState {
                        window,
                        ui_renderer: renderer,
                        last_cursor_position: None,
                        scale_factor,
                        focused: false,
                    },
                );
            }
        }

        #[cfg(test)]
        pub fn shell_report(&self) -> NativeEditorApplicationReport {
            self.app.report()
        }

        #[cfg(test)]
        pub fn has_panel(&self, panel_id: &str) -> bool {
            editor_ui_renderer::native_editor_panel_manifest()
                .iter()
                .any(|entry| entry.panel_id == panel_id)
        }

        #[cfg(test)]
        pub fn recent_store_path(&self) -> Option<&Path> {
            self.app.project_manager().recent_store_path.as_deref()
        }

        #[cfg(test)]
        pub fn project_dialog_initial_directory(&self) -> &Path {
            self.app.project_dialog_initial_directory()
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum WorkspaceAuthorityStage {
        AwaitingFrames,
        AwaitingPointerDown,
        AwaitingPointerMove,
        AwaitingCommit,
        Capture,
        Finished,
    }

    struct RealWorkspaceAuthorityApp {
        inner: RealNativeEditorApp,
        scenario_id: String,
        stage: WorkspaceAuthorityStage,
        requested_physical_size: (u32, u32),
        source_window_id: Option<WorkspaceWindowId>,
        pending_target_client: Option<(i32, i32)>,
        deadline: std::time::Instant,
        outcome: RealWorkspaceAuthorityOutcome,
    }

    impl RealWorkspaceAuthorityApp {
        fn new(options: RealWorkspaceAuthorityOptions) -> Self {
            let config = NativeEditorWindowConfig {
                title: format!("AI First Engine 258 Authority - {}", options.scenario_id),
                width: options.physical_width,
                height: options.physical_height,
                resizable: true,
                scale_factor: 1.0,
            };
            let mut session = EditorSession::with_linked_project_runtimes(
                crate::default_editor_linked_project_runtimes(),
            );
            let payload = editor_ui_model::UiCommandPayload::OpenProject {
                path: options.project_root.display().to_string(),
            };
            let open = session.execute_command(editor_ui_model::UiCommand {
                command_id: editor_ui_model::ui_command_id_for_payload(&payload).to_string(),
                source: editor_ui_model::UiCommandSource::Test,
                request_id: format!("258-authority-open-{}", options.scenario_id),
                payload,
            });
            let app = NativeEditorApplication::with_session(config.clone(), session)
                .with_workspace_layout_store(crate::workspace_layout_store_at_root(
                    options.workspace_layout_store_root,
                ));
            let mut report = RealNativeEditorWindowReport::new("winit-wgpu-258-authority");
            if open.status != editor_core::CommandStatus::Committed {
                report.diagnostics.push(RealNativeEditorWindowDiagnostic {
                    severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                    code: "authority.open_project_failed".to_string(),
                    message: open.diagnostics.first().map_or_else(
                        || "Open project failed.".to_string(),
                        |value| value.message.clone(),
                    ),
                    source_stage: "authority.open_project".to_string(),
                });
            }
            let inner = RealNativeEditorApp {
                app,
                frame_publication: EditorFramePublicationModule::new(),
                #[cfg(feature = "real-wgpu-surface")]
                game_view_gpu_residency: GameViewGpuResidentState::default(),
                windows: BTreeMap::new(),
                native_to_workspace: BTreeMap::new(),
                drag_proxy: None,
                last_drag_screen_pointer: None,
                report,
                candidate_readiness: None,
                pending_handoff_project_open: None,
                graceful_exit_requested: Arc::new(AtomicBool::new(false)),
                next_game_view_tick: std::time::Instant::now(),
            };
            Self {
                inner,
                scenario_id: options.scenario_id,
                stage: WorkspaceAuthorityStage::AwaitingFrames,
                requested_physical_size: (options.physical_width, options.physical_height),
                source_window_id: None,
                pending_target_client: None,
                deadline: std::time::Instant::now() + std::time::Duration::from_secs(20),
                outcome: RealWorkspaceAuthorityOutcome {
                    window_report: RealNativeEditorWindowReport::new("winit-wgpu-258-authority"),
                    windows: Vec::new(),
                    main_window_count: 0,
                    floating_window_count: 0,
                    proxy_created: false,
                    proxy_destroyed: false,
                    resolved_target_token: None,
                    layout_revision_before: None,
                    layout_revision_after: None,
                    panel_unique: false,
                    workspace_diagnostics: Vec::new(),
                    input_observed: false,
                },
            }
        }

        fn fail(&mut self, code: &str, message: impl Into<String>, stage: &str) {
            self.inner
                .report
                .diagnostics
                .push(RealNativeEditorWindowDiagnostic {
                    severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                    code: code.to_string(),
                    message: message.into(),
                    source_stage: stage.to_string(),
                });
            self.stage = WorkspaceAuthorityStage::Finished;
        }

        fn revision(&self) -> u64 {
            self.inner
                .app
                .workspace_docking()
                .snapshot(editor_workspace_rect(
                    self.requested_physical_size.0 as f32,
                    self.requested_physical_size.1 as f32,
                ))
                .layout_revision
        }

        fn panel_is_unique(&self, panel_id: &str) -> bool {
            let topology = self.inner.app.workspace_docking().topology();
            let mut count = 0usize;
            fn count_panel(node: &editor_ui_renderer::DockNode, panel_id: &str, count: &mut usize) {
                match node {
                    editor_ui_renderer::DockNode::Split { first, second, .. } => {
                        count_panel(first, panel_id, count);
                        count_panel(second, panel_id, count);
                    }
                    editor_ui_renderer::DockNode::Stack { tabs, .. } => {
                        *count += tabs
                            .iter()
                            .filter(|candidate| candidate.as_str() == panel_id)
                            .count();
                    }
                }
            }
            count_panel(&topology.main_root.root, panel_id, &mut count);
            for floating in &topology.floating_roots {
                count_panel(&floating.root, panel_id, &mut count);
            }
            count == 1
        }

        fn start_drag(
            &mut self,
            source_window_id: &WorkspaceWindowId,
            target_screen: (i32, i32),
        ) -> Result<(), String> {
            let source = self
                .inner
                .windows
                .get(source_window_id)
                .ok_or_else(|| "authority.source_window_missing".to_string())?;
            let tab = self
                .inner
                .app
                .latest_draw_list()
                .hit_regions
                .iter()
                .find(|region| {
                    matches!(
                        &region.target,
                        editor_ui_renderer::HitTarget::DockTab { panel_id }
                            if panel_id == "ai_panel"
                    )
                })
                .ok_or_else(|| "authority.ai_panel_tab_missing".to_string())?;
            let start_x =
                ((tab.rect.x + tab.rect.width * 0.5) * source.scale_factor as f32).round() as i32;
            let start_y =
                ((tab.rect.y + tab.rect.height * 0.5) * source.scale_factor as f32).round() as i32;
            let source_outer = source
                .window
                .outer_position()
                .map_err(|error| format!("authority.source_position_failed:{error}"))?;
            let target_client = (
                target_screen.0 - source_outer.x,
                target_screen.1 - source_outer.y,
            );
            crate::begin_authority_primary_drag(&source.window, start_x, start_y)?;
            self.source_window_id = Some(source_window_id.clone());
            self.pending_target_client = Some(target_client);
            self.stage = WorkspaceAuthorityStage::AwaitingPointerDown;
            eprintln!(
                "258-authority stage=awaiting_pointer_down source={} target_client={},{}",
                source_window_id.as_str(),
                target_client.0,
                target_client.1
            );
            Ok(())
        }

        fn maybe_start_scenario(&mut self) -> Result<(), String> {
            let main = WorkspaceWindowId::main();
            if self.scenario_id == "258-main-to-floating" {
                if self.inner.windows.len() != 1 || !self.inner.windows.contains_key(&main) {
                    return Err("authority.main_to_floating_requires_one_main_window".to_string());
                }
                let state = &self.inner.windows[&main];
                let outer = state
                    .window
                    .outer_position()
                    .map_err(|error| format!("authority.main_position_failed:{error}"))?;
                let size = state.window.inner_size();
                self.outcome.layout_revision_before = Some(self.revision());
                return self.start_drag(
                    &main,
                    (
                        outer.x + (size.width / 2) as i32,
                        outer.y + size.height as i32 + 80,
                    ),
                );
            }
            if self.scenario_id == "258-floating-redock-close" {
                let floating = self
                    .inner
                    .windows
                    .keys()
                    .find(|window_id| !window_id.is_main())
                    .cloned()
                    .ok_or_else(|| "authority.floating_window_missing".to_string())?;
                let main_state = &self.inner.windows[&main];
                let outer = main_state
                    .window
                    .outer_position()
                    .map_err(|error| format!("authority.main_position_failed:{error}"))?;
                let size = main_state.window.inner_size();
                self.outcome.layout_revision_before = Some(self.revision());
                return self.start_drag(
                    &floating,
                    (
                        outer.x + (size.width / 2) as i32,
                        outer.y + (size.height / 2) as i32,
                    ),
                );
            }
            Err(format!(
                "authority.unsupported_workspace_scenario:{}",
                self.scenario_id
            ))
        }

        fn observe_drag(&mut self) {
            if self.inner.drag_proxy.is_some() {
                self.outcome.proxy_created = true;
            }
            if let Some(token) = self
                .inner
                .app
                .workspace_docking()
                .resolved_drag_target_token()
            {
                self.outcome.resolved_target_token = Some(format!(
                    "{}:{}:{:?}:{}",
                    token.window_id.as_str(),
                    token.node_id.as_str(),
                    token.zone,
                    token.layout_revision
                ));
            }
        }

        fn capture_and_finish(&mut self, event_loop: &ActiveEventLoop) {
            let ids = self.inner.windows.keys().cloned().collect::<Vec<_>>();
            let mut windows = Vec::new();
            for workspace_window_id in ids {
                let Some(state) = self.inner.windows.get_mut(&workspace_window_id) else {
                    continue;
                };
                let size = state.window.inner_size();
                let outer = state.window.outer_position().unwrap_or_default();
                self.inner.app.frame_workspace_window(
                    &workspace_window_id,
                    size.width as f32 / state.scale_factor as f32,
                    size.height as f32 / state.scale_factor as f32,
                );
                let (_, capture) = state
                    .ui_renderer
                    .present_with_rgba_capture(self.inner.app.latest_draw_list());
                windows.push(RealWorkspaceAuthorityWindowEvidence {
                    workspace_window_id: workspace_window_id.as_str().to_string(),
                    native_window_id: format!("{:?}", state.window.id()),
                    scale_factor: state.scale_factor,
                    screen_rect: (outer.x, outer.y, size.width, size.height),
                    surface_created: true,
                    capture: capture.ok(),
                });
            }
            self.outcome.main_window_count = windows
                .iter()
                .filter(|window| window.workspace_window_id == "main")
                .count();
            self.outcome.floating_window_count = windows.len() - self.outcome.main_window_count;
            self.outcome.windows = windows;
            self.outcome.layout_revision_after = Some(self.revision());
            self.outcome.panel_unique = self.panel_is_unique("ai_panel");
            self.outcome.workspace_diagnostics = self
                .inner
                .app
                .workspace_docking()
                .snapshot(editor_workspace_rect(
                    self.requested_physical_size.0 as f32,
                    self.requested_physical_size.1 as f32,
                ))
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect();
            self.outcome.proxy_destroyed =
                self.outcome.proxy_created && self.inner.drag_proxy.is_none();
            self.outcome.window_report = self.inner.report.clone();
            self.stage = WorkspaceAuthorityStage::Finished;
            event_loop.exit();
        }
    }

    impl ApplicationHandler for RealWorkspaceAuthorityApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.inner.resumed(event_loop);
            if let Some(main) = self.inner.windows.get(&WorkspaceWindowId::main()) {
                let _ = main
                    .window
                    .request_inner_size(winit::dpi::PhysicalSize::new(
                        self.requested_physical_size.0,
                        self.requested_physical_size.1,
                    ));
                main.window.focus_window();
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            native_window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            if std::time::Instant::now() >= self.deadline
                && self.stage != WorkspaceAuthorityStage::Finished
            {
                let stage = self.stage;
                let _ = crate::finish_authority_primary_drag();
                self.fail(
                    "authority.timeout",
                    format!("The real workspace authority scenario timed out in {stage:?}."),
                    "authority.window_event",
                );
                event_loop.exit();
                return;
            }
            let workspace_window_id = self
                .inner
                .native_to_workspace
                .get(&native_window_id)
                .cloned();
            let is_redraw = matches!(event, WindowEvent::RedrawRequested);
            let is_pointer_down = matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    ..
                }
            );
            let is_pointer_move = matches!(event, WindowEvent::CursorMoved { .. });
            let is_pointer_up = matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                }
            );
            self.inner.window_event(event_loop, native_window_id, event);

            if self.inner.drag_proxy.is_some() {
                self.outcome.proxy_created = true;
            }
            if is_redraw
                && self.stage == WorkspaceAuthorityStage::AwaitingFrames
                && workspace_window_id.as_ref().is_some_and(|id| {
                    if self.scenario_id == "258-main-to-floating" {
                        id.is_main()
                    } else {
                        !id.is_main()
                    }
                })
            {
                if let Err(error) = self.maybe_start_scenario() {
                    self.fail("authority.scenario_start_failed", error, "authority.input");
                    event_loop.exit();
                }
                return;
            }
            if is_pointer_down && self.stage == WorkspaceAuthorityStage::AwaitingPointerDown {
                self.outcome.input_observed = true;
                let Some(source) = self.source_window_id.as_ref() else {
                    self.fail(
                        "authority.source_window_missing",
                        "Source window identity disappeared.",
                        "authority.input",
                    );
                    event_loop.exit();
                    return;
                };
                let Some(target) = self.pending_target_client else {
                    self.fail(
                        "authority.target_missing",
                        "Pointer target disappeared.",
                        "authority.input",
                    );
                    event_loop.exit();
                    return;
                };
                let Some(window) = self
                    .inner
                    .windows
                    .get(source)
                    .map(|state| state.window.clone())
                else {
                    self.fail(
                        "authority.source_window_missing",
                        "Source native window disappeared.",
                        "authority.input",
                    );
                    event_loop.exit();
                    return;
                };
                self.stage = WorkspaceAuthorityStage::AwaitingPointerMove;
                eprintln!("258-authority stage=awaiting_pointer_move");
                if let Err(error) = crate::move_authority_primary_drag(&window, target.0, target.1)
                {
                    self.fail("authority.pointer_move_failed", error, "windows.send_input");
                    event_loop.exit();
                }
                return;
            }
            if is_pointer_move && self.stage == WorkspaceAuthorityStage::AwaitingPointerMove {
                self.observe_drag();
                self.stage = WorkspaceAuthorityStage::AwaitingCommit;
                eprintln!(
                    "258-authority stage=awaiting_commit token={:?} proxy={}",
                    self.outcome.resolved_target_token, self.outcome.proxy_created
                );
                if let Err(error) = crate::finish_authority_primary_drag() {
                    self.fail("authority.pointer_up_failed", error, "windows.send_input");
                    event_loop.exit();
                }
                return;
            }
            if is_pointer_up && self.stage == WorkspaceAuthorityStage::AwaitingCommit {
                self.outcome.input_observed = true;
                self.observe_drag();
                self.inner.reconcile_drag_proxy(event_loop);
                self.inner.reconcile_windows(event_loop);
                self.stage = WorkspaceAuthorityStage::Capture;
                eprintln!(
                    "258-authority stage=capture windows={}",
                    self.inner.windows.len()
                );
                self.inner.request_redraw();
                return;
            }
            if is_redraw && self.stage == WorkspaceAuthorityStage::Capture {
                self.capture_and_finish(event_loop);
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if std::time::Instant::now() >= self.deadline
                && self.stage != WorkspaceAuthorityStage::Finished
            {
                let _ = crate::finish_authority_primary_drag();
                self.fail(
                    "authority.timeout",
                    "The real workspace authority scenario exceeded 20 seconds.",
                    "authority.event_loop",
                );
                event_loop.exit();
            }
        }
    }

    pub fn run_real_workspace_authority(
        options: RealWorkspaceAuthorityOptions,
    ) -> RealWorkspaceAuthorityOutcome {
        let mut event_loop_builder = EventLoop::builder();
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            event_loop_builder.with_any_thread(true);
        }
        let event_loop = match event_loop_builder.build() {
            Ok(event_loop) => event_loop,
            Err(error) => {
                return RealWorkspaceAuthorityOutcome {
                    window_report: RealNativeEditorWindowReport::environment_blocked(
                        "winit-wgpu-258-authority",
                        error.to_string(),
                    ),
                    windows: Vec::new(),
                    main_window_count: 0,
                    floating_window_count: 0,
                    proxy_created: false,
                    proxy_destroyed: false,
                    resolved_target_token: None,
                    layout_revision_before: None,
                    layout_revision_after: None,
                    panel_unique: false,
                    workspace_diagnostics: Vec::new(),
                    input_observed: false,
                };
            }
        };
        let mut app = RealWorkspaceAuthorityApp::new(options);
        if let Err(error) = event_loop.run_app(&mut app) {
            app.fail(
                "authority.event_loop_failed",
                error.to_string(),
                "winit.event_loop",
            );
        }
        app.outcome
    }

    impl ApplicationHandler for RealNativeEditorApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.reconcile_windows(event_loop);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            let Some(workspace_window_id) = self.native_to_workspace.get(&window_id).cloned()
            else {
                return;
            };
            match event {
                WindowEvent::CloseRequested => {
                    if workspace_window_id.is_main() {
                        self.report.close_requested = true;
                        let receipt = self.app.shutdown_llm();
                        if let Some(diagnostic) = receipt.diagnostic {
                            self.report
                                .diagnostics
                                .push(RealNativeEditorWindowDiagnostic {
                                    severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                                    code: diagnostic.code,
                                    message: diagnostic.message,
                                    source_stage: "editor_session.shutdown_llm".to_string(),
                                });
                        }
                        event_loop.exit();
                    } else {
                        self.app
                            .close_floating_workspace_window(workspace_window_id);
                        self.reconcile_windows(event_loop);
                    }
                }
                WindowEvent::Resized(size) => {
                    self.report.resize_count += 1;
                    self.report.surface_configured = size.width > 0 && size.height > 0;
                    let Some(state) = self.windows.get_mut(&workspace_window_id) else {
                        return;
                    };
                    let logical_width = (f64::from(size.width) / state.scale_factor).round() as u32;
                    let logical_height =
                        (f64::from(size.height) / state.scale_factor).round() as u32;
                    if workspace_window_id.is_main() {
                        self.app.resize(logical_width, logical_height);
                    }
                    state.ui_renderer.resize(size.width, size.height);
                    self.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let Some(state) = self.windows.get_mut(&workspace_window_id) else {
                        return;
                    };
                    let logical = physical_to_logical(
                        PhysicalPoint {
                            x: position.x,
                            y: position.y,
                        },
                        state.scale_factor,
                    );
                    let x = logical.x as f32;
                    let y = logical.y as f32;
                    state.last_cursor_position = Some((x, y));
                    self.route_editor_input(
                        &workspace_window_id,
                        EditorInputEvent::PointerMove { x, y },
                    );
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if let Some(button) = pointer_button_from_winit(button) {
                        let (x, y) = self
                            .windows
                            .get(&workspace_window_id)
                            .and_then(|state| state.last_cursor_position)
                            .unwrap_or((0.0, 0.0));
                        let event = match state {
                            ElementState::Pressed => EditorInputEvent::PointerDown { x, y, button },
                            ElementState::Released => EditorInputEvent::PointerUp { x, y, button },
                        };
                        self.route_editor_input(&workspace_window_id, event);
                        match state {
                            ElementState::Pressed
                                if self
                                    .app
                                    .workspace_docking()
                                    .active_panel_drag_id()
                                    .is_some() =>
                            {
                                if let Some(window) = self
                                    .windows
                                    .get(&workspace_window_id)
                                    .map(|state| state.window.as_ref())
                                {
                                    set_native_pointer_capture(window);
                                }
                            }
                            ElementState::Released => release_native_pointer_capture(),
                            _ => {}
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(position) => position.y as f32,
                    };
                    self.route_editor_input(
                        &workspace_window_id,
                        EditorInputEvent::MouseWheel { delta },
                    );
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    let key = key_name(&event.logical_key);
                    let input = match event.state {
                        ElementState::Pressed => EditorInputEvent::KeyDown { key },
                        ElementState::Released => EditorInputEvent::KeyUp { key },
                    };
                    self.route_editor_input(&workspace_window_id, input);
                }
                WindowEvent::Focused(focused) => {
                    if let Some(state) = self.windows.get_mut(&workspace_window_id) {
                        state.focused = focused;
                    }
                    if !focused {
                        release_native_pointer_capture();
                        self.route_editor_input(&workspace_window_id, EditorInputEvent::FocusLost);
                    }
                }
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    if let Some(state) = self.windows.get_mut(&workspace_window_id) {
                        state.scale_factor = scale_factor;
                        let size = state.window.inner_size();
                        if workspace_window_id.is_main() {
                            self.app.resize(
                                (f64::from(size.width) / scale_factor).round() as u32,
                                (f64::from(size.height) / scale_factor).round() as u32,
                            );
                        }
                    }
                    self.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    let Some(state) = self.windows.get_mut(&workspace_window_id) else {
                        return;
                    };
                    let size = state.window.inner_size();
                    let scale_factor = state.scale_factor;
                    #[cfg(feature = "real-wgpu-surface")]
                    if workspace_window_id.is_main() {
                        present_active_game_view_to_shared_texture(
                            &mut self.app,
                            &mut state.ui_renderer,
                            &mut self.frame_publication,
                            &mut self.game_view_gpu_residency,
                            &mut self.report,
                        );
                    }
                    let app_report = self.app.frame_workspace_window(
                        &workspace_window_id,
                        size.width as f32 / scale_factor as f32,
                        size.height as f32 / scale_factor as f32,
                    );
                    if self.graceful_exit_requested.load(Ordering::Acquire) {
                        event_loop.exit();
                        return;
                    }
                    self.report.frame_index += 1;
                    self.report.hit_region_count = app_report.hit_region_count;
                    upload_visible_asset_thumbnails(
                        &mut self.app,
                        &mut state.ui_renderer,
                        &mut self.report,
                    );
                    let ui_report = state.ui_renderer.present(self.app.latest_draw_list());
                    self.report.apply_ui_present_report(&ui_report);
                    if workspace_window_id.is_main() {
                        let had_candidate_readiness = self.candidate_readiness.is_some();
                        if let Err(error) = acknowledge_project_editor_candidate_after_present(
                            ui_report.presented,
                            &mut self.candidate_readiness,
                        ) {
                            self.report
                                .diagnostics
                                .push(RealNativeEditorWindowDiagnostic {
                                    severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                                    code: "project_editor_composition.readiness_ack_failed"
                                        .to_string(),
                                    message: error,
                                    source_stage: "editor_window.first_present".to_string(),
                                });
                            event_loop.exit();
                        } else if had_candidate_readiness && self.candidate_readiness.is_none() {
                            if let Some(project_root) = self.pending_handoff_project_open.take() {
                                let _ =
                                    self.app.dispatch_verified_composition_handoff_project_open(
                                        &project_root,
                                        format!(
                                            "project-composition-handoff-open-{}",
                                            self.report.frame_index
                                        ),
                                    );
                            }
                        }
                    }
                    self.report.apply_shared_gpu_context_summary(
                        &state.ui_renderer.shared_context_summary(),
                    );
                    self.report.apply_viewport_texture_registry_state(
                        state.ui_renderer.viewport_textures().texture_count(),
                        state
                            .ui_renderer
                            .viewport_textures()
                            .lifecycle_event_count(),
                    );
                    if app_report.redraw_requested || self.app.report().redraw_requested {
                        self.request_redraw();
                    }
                }
                _ => {}
            }
            self.reconcile_drag_proxy(event_loop);
            self.reconcile_windows(event_loop);
        }

        fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
            self.request_redraw();
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            let now = std::time::Instant::now();
            let game_view_deadline = if self.app.session().has_active_editor_runtime_play_instance()
            {
                let tick_count =
                    advance_game_view_tick_deadline(now, &mut self.next_game_view_tick);
                let mut advanced = false;
                for _ in 0..tick_count {
                    advanced |= self
                        .app
                        .tick_active_game_view_runtime_descriptor_frame()
                        .is_some();
                }
                if advanced {
                    self.request_redraw();
                }
                Some(self.next_game_view_tick)
            } else {
                reset_game_view_tick_deadline(now, &mut self.next_game_view_tick);
                None
            };
            if self.app.take_project_composition_progress_redraw_due(now) {
                self.request_redraw();
            }
            match earliest_editor_deadline(
                game_view_deadline,
                self.app.project_composition_progress_deadline(),
            ) {
                Some(deadline) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
                None => event_loop.set_control_flow(ControlFlow::Wait),
            }
        }
    }

    fn floating_window_attributes(
        config: &NativeEditorWindowConfig,
        window_id: &WorkspaceWindowId,
        placement: &WorkspaceWindowPlacement,
    ) -> winit::window::WindowAttributes {
        Window::default_attributes()
            .with_title(format!("{} - {}", config.title, window_id.as_str()))
            .with_inner_size(winit::dpi::PhysicalSize::new(
                placement.width.max(1.0).round() as u32,
                placement.height.max(1.0).round() as u32,
            ))
            .with_position(winit::dpi::PhysicalPosition::new(
                placement.x.round() as i32,
                placement.y.round() as i32,
            ))
            .with_resizable(true)
    }

    #[cfg(test)]
    #[test]
    fn floating_window_attributes_preserve_physical_workspace_placement() {
        let placement = WorkspaceWindowPlacement {
            x: 1440.0,
            y: 320.0,
            width: 640.0,
            height: 480.0,
            display_id: Some("display-2".to_string()),
        };
        let attributes = floating_window_attributes(
            &NativeEditorWindowConfig::default(),
            &WorkspaceWindowId::new("floating-test").unwrap(),
            &placement,
        );
        assert_eq!(
            attributes.position,
            Some(winit::dpi::Position::Physical(
                winit::dpi::PhysicalPosition::new(1440, 320)
            ))
        );
        assert_eq!(
            attributes.inner_size,
            Some(winit::dpi::Size::Physical(winit::dpi::PhysicalSize::new(
                640, 480
            )))
        );
    }

    #[cfg(target_os = "windows")]
    fn set_native_pointer_capture(window: &Window) {
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::SetCapture(
                handle.hwnd.get() as windows_sys::Win32::Foundation::HWND
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn set_native_pointer_capture(_window: &Window) {}

    #[cfg(target_os = "windows")]
    fn release_native_pointer_capture() {
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn release_native_pointer_capture() {}

    fn upload_visible_asset_thumbnails(
        app: &mut NativeEditorApplication,
        renderer: &mut RealWgpuUiRenderer,
        window_report: &mut RealNativeEditorWindowReport,
    ) {
        let missing_ids = app
            .visible_asset_thumbnail_ids()
            .into_iter()
            .filter(|thumbnail_id| !renderer.image_textures().contains(thumbnail_id))
            .collect::<std::collections::BTreeSet<_>>();
        if missing_ids.is_empty() {
            return;
        }
        let payloads = app.asset_thumbnail_payloads_for_ids(&missing_ids);
        if payloads.is_empty() {
            return;
        }
        let shared_context = renderer.shared_context();
        let mut uploaded = false;
        for payload in payloads {
            match renderer.image_textures_mut().upload_gpu(
                shared_context.device(),
                shared_context.queue(),
                payload.thumbnail_id,
                payload.width,
                payload.height,
                payload.content_hash,
                &payload.rgba8,
            ) {
                Ok(_) => uploaded = true,
                Err(error) => window_report
                    .diagnostics
                    .push(RealNativeEditorWindowDiagnostic {
                        severity: RealNativeEditorWindowDiagnosticSeverity::Warning,
                        code: "asset_thumbnail_gpu_upload_failed".to_string(),
                        message: error,
                        source_stage: "editor_image_texture_upload".to_string(),
                    }),
            }
        }
        if uploaded {
            app.request_redraw();
        }
    }

    #[cfg(feature = "real-wgpu-surface")]
    pub(crate) struct ExactSharedTexturePresent {
        pub readback: Option<EditorViewportTextureReadback>,
        pub receipt: GameViewPublicationReceipt,
        diagnostics: Vec<GameViewPresentDiagnostic>,
    }

    #[cfg(feature = "real-wgpu-surface")]
    struct ExactSharedTexturePresentError {
        code: &'static str,
        message: String,
        diagnostics: Vec<GameViewPresentDiagnostic>,
    }

    #[cfg(all(feature = "real-wgpu-surface", test))]
    pub(crate) fn render_game_view_plan_to_exact_shared_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_format: wgpu::TextureFormat,
        viewport_textures: &mut EditorViewportTextureRegistry,
        frame: &GameViewRuntimeFrame,
        plan: &RhiCommandPlan,
        capture_readback: bool,
    ) -> Result<ExactSharedTexturePresent, String> {
        let mut frame_publication = EditorFramePublicationModule::new();
        let mut gpu_residency = GameViewGpuResidentState::default();
        render_game_view_plan_to_exact_shared_texture_internal(
            device,
            queue,
            texture_format,
            viewport_textures,
            &mut frame_publication,
            &mut gpu_residency,
            frame,
            plan,
            None,
            None,
            capture_readback,
        )
        .map_err(|error| format!("{}:{}", error.code, error.message))
    }

    #[cfg(feature = "real-wgpu-surface")]
    fn render_game_view_plan_to_exact_shared_texture_internal(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_format: wgpu::TextureFormat,
        viewport_textures: &mut EditorViewportTextureRegistry,
        frame_publication: &mut EditorFramePublicationModule,
        gpu_residency: &mut GameViewGpuResidentState,
        frame: &GameViewRuntimeFrame,
        plan: &RhiCommandPlan,
        font_bundles: Option<&engine_runtime::font_bundle::RuntimeFontBundleRegistry>,
        runtime_textures: Option<&engine_runtime::runtime_texture::RuntimeTextureUploadRegistry>,
        capture_readback: bool,
    ) -> Result<ExactSharedTexturePresent, ExactSharedTexturePresentError> {
        if let Some(last_good) = frame_publication.reusable_last_good(frame).cloned() {
            let reused = frame_publication
                .publish(frame, || {
                    Err(EditorFramePublicationError::new(
                        "publication.unexpected_runtime_submit",
                        "A redraw of the last-good GameView frame must not submit Runtime work.",
                    ))
                })
                .map_err(|error| ExactSharedTexturePresentError {
                    code: error.code,
                    message: error.message,
                    diagnostics: Vec::new(),
                })?;
            debug_assert_eq!(reused.receipt.publication, last_good.publication);
            let readback = if capture_readback {
                Some(
                    viewport_textures
                        .readback_gpu_exact(device, queue, &reused.receipt)
                        .map_err(|message| ExactSharedTexturePresentError {
                            code: "project_preview_evidence.readback_failed",
                            message,
                            diagnostics: Vec::new(),
                        })?,
                )
            } else {
                None
            };
            return Ok(ExactSharedTexturePresent {
                readback,
                receipt: reused.receipt,
                diagnostics: Vec::new(),
            });
        }
        let backend = gpu_residency
            .prepare(
                device,
                queue,
                texture_format,
                frame,
                runtime_textures,
                font_bundles,
            )
            .map_err(|message| ExactSharedTexturePresentError {
                code: "aui_image.texture_upload_failed",
                message,
                diagnostics: Vec::new(),
            })?;
        backend
            .validate_plan_texture_residency(plan)
            .map_err(|message| ExactSharedTexturePresentError {
                code: "wgpu.texture_binding_missing",
                diagnostics: vec![GameViewPresentDiagnostic::error(
                    "wgpu.texture_binding_missing",
                    "gpu_present",
                    message.clone(),
                )],
                message,
            })?;

        viewport_textures
            .allocate_or_resize_gpu(
                device,
                frame.session_id.clone(),
                frame.target_id.clone(),
                frame.texture_id.clone(),
                frame.width.max(1),
                frame.height.max(1),
                texture_format,
                "editor-gameview-runtime",
            )
            .map_err(|message| ExactSharedTexturePresentError {
                code: "project_preview_evidence.shared_texture_allocate_failed",
                message,
                diagnostics: Vec::new(),
            })?;

        let backend_report = {
            let resolved = viewport_textures
                .resolve_gpu(&frame.texture_id)
                .ok_or_else(|| ExactSharedTexturePresentError {
                    code: "project_preview_evidence.shared_texture_resolve_failed",
                    message: "The rendered GameView texture could not be resolved for exact-frame capture."
                        .to_string(),
                    diagnostics: Vec::new(),
                })?;
            backend.execute_plan_to_surface_view(plan, resolved.view)
        };
        let diagnostics = backend_report
            .diagnostics
            .iter()
            .map(|diagnostic| match diagnostic.severity {
                RhiBackendDiagnosticSeverity::Info => GameViewPresentDiagnostic::info(
                    diagnostic.code.clone(),
                    "gpu_present",
                    diagnostic.message.clone(),
                ),
                RhiBackendDiagnosticSeverity::Warning => GameViewPresentDiagnostic::warning(
                    diagnostic.code.clone(),
                    "gpu_present",
                    diagnostic.message.clone(),
                ),
                RhiBackendDiagnosticSeverity::Error => GameViewPresentDiagnostic::error(
                    diagnostic.code.clone(),
                    "gpu_present",
                    diagnostic.message.clone(),
                ),
            })
            .collect::<Vec<_>>();
        if backend_report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == RhiBackendDiagnosticSeverity::Error)
        {
            return Err(ExactSharedTexturePresentError {
                code: "project_preview_evidence.shared_texture_render_failed",
                message: "The runtime RHI plan failed while rendering the pending Preview frame."
                    .to_string(),
                diagnostics,
            });
        }

        let receipt = viewport_textures
            .mark_published(
                &frame.texture_id,
                frame.frame_index,
                frame.frame_hash.clone(),
            )
            .ok_or_else(|| ExactSharedTexturePresentError {
                code: "project_preview_evidence.shared_texture_mark_rendered_failed",
                message:
                    "The exact GameView texture disappeared before it could be marked rendered."
                        .to_string(),
                diagnostics: diagnostics.clone(),
            })?;
        let publication = frame_publication
            .publish(frame, || Ok(receipt))
            .map_err(|error| ExactSharedTexturePresentError {
                code: error.code,
                message: error.message,
                diagnostics: diagnostics.clone(),
            })?;
        let readback = if capture_readback {
            let readback = viewport_textures
                .readback_gpu_exact(device, queue, &publication.receipt)
                .map_err(|message| ExactSharedTexturePresentError {
                    code: "project_preview_evidence.readback_failed",
                    message,
                    diagnostics: diagnostics.clone(),
                })?;
            validate_preview_readback_frame(&readback, frame).map_err(|code| {
                ExactSharedTexturePresentError {
                    code,
                    message: format!(
                        "Exact texture readback did not match the pending frame: owner={} target={} texture={} frame={} size={}x{}.",
                        readback.owner_session_id,
                        readback.target_id,
                        readback.texture_id,
                        readback.frame_index,
                        readback.width,
                        readback.height,
                    ),
                    diagnostics: diagnostics.clone(),
                }
            })?;
            Some(readback)
        } else {
            None
        };
        Ok(ExactSharedTexturePresent {
            readback,
            receipt: publication.receipt,
            diagnostics,
        })
    }

    #[cfg(feature = "real-wgpu-surface")]
    fn present_active_game_view_to_shared_texture(
        app: &mut NativeEditorApplication,
        renderer: &mut RealWgpuUiRenderer,
        frame_publication: &mut EditorFramePublicationModule,
        gpu_residency: &mut GameViewGpuResidentState,
        window_report: &mut RealNativeEditorWindowReport,
    ) {
        if !app.session().has_active_editor_runtime_play_instance() {
            if let Some(session_id) = gpu_residency.retire() {
                frame_publication.retire_session(&session_id);
            }
            return;
        }
        let pending_ticket = app.pending_project_preview_frame_ticket().cloned();
        let Some(frame) = app.active_game_view_frame_for_window_present() else {
            fail_pending_preview_frame(
                app,
                pending_ticket.as_ref(),
                "project_preview_evidence.runtime_frame_missing",
                "The pending Preview ticket has no retained GameView runtime frame.",
            );
            return;
        };
        if let Some(ticket) = &pending_ticket {
            if validate_preview_ticket_frame(ticket, &frame).is_err() {
                let message = format!(
                    "Pending Preview ticket does not match retained GameView frame: ticket=({}, {}, {}, {}) frame=({}, {}, {}, {}).",
                    ticket.game_view_session_id,
                    ticket.expected_texture_id,
                    ticket.expected_frame_index,
                    ticket.expected_runtime_frame_hash,
                    frame.session_id,
                    frame.texture_id,
                    frame.frame_index,
                    frame.frame_hash,
                );
                fail_pending_preview_frame(
                    app,
                    Some(ticket),
                    "project_preview_evidence.window_frame_ticket_mismatch",
                    &message,
                );
                window_report
                    .diagnostics
                    .push(RealNativeEditorWindowDiagnostic {
                        severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                        code: "project_preview_evidence.window_frame_ticket_mismatch".to_string(),
                        message,
                        source_stage: "editor_gameview_gpu_present".to_string(),
                    });
                return;
            }
        }
        let Some(plan) = app.active_game_view_rhi_command_plan().cloned() else {
            fail_pending_preview_frame(
                app,
                pending_ticket.as_ref(),
                "project_preview_evidence.rhi_command_plan_missing",
                "The pending Preview frame has no RHI command plan to render.",
            );
            let diagnostic = GameViewPresentDiagnostic::error(
                "gameview_rhi_command_plan_missing",
                "gpu_present",
                "EditorRuntimePlayInstance produced a frame without an RhiCommandPlan.",
            );
            app.mark_active_game_view_gpu_present_result(
                "failed",
                "Available",
                vec![diagnostic.clone()],
            );
            window_report
                .diagnostics
                .push(RealNativeEditorWindowDiagnostic {
                    severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                    code: diagnostic.code,
                    message: diagnostic.message,
                    source_stage: "editor_gameview_gpu_present".to_string(),
                });
            return;
        };

        let shared_context = renderer.shared_context();
        let shared_context_summary = renderer.shared_context_summary();
        let shared_context_status = format!("{:?}", shared_context_summary.status);
        let texture_format = renderer.viewport_texture_format();
        let font_bundles = app.active_game_view_font_bundles();
        let runtime_textures = app.active_game_view_runtime_texture_uploads();
        let presented = render_game_view_plan_to_exact_shared_texture_internal(
            shared_context.device(),
            shared_context.queue(),
            texture_format,
            renderer.viewport_textures_mut(),
            frame_publication,
            gpu_residency,
            &frame,
            &plan,
            font_bundles,
            runtime_textures,
            pending_ticket.is_some(),
        );
        let presented = match presented {
            Ok(presented) => presented,
            Err(error) => {
                fail_pending_preview_frame(
                    app,
                    pending_ticket.as_ref(),
                    error.code,
                    &error.message,
                );
                let diagnostics = if error.diagnostics.is_empty() {
                    vec![GameViewPresentDiagnostic::error(
                        error.code,
                        "gpu_present",
                        error.message.clone(),
                    )]
                } else {
                    error.diagnostics
                };
                app.mark_active_game_view_gpu_present_result(
                    "failed",
                    shared_context_status,
                    diagnostics,
                );
                window_report
                    .diagnostics
                    .push(RealNativeEditorWindowDiagnostic {
                        severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                        code: error.code.to_string(),
                        message: error.message,
                        source_stage: "editor_gameview_gpu_present".to_string(),
                    });
                return;
            }
        };
        window_report.apply_game_view_publication_receipt(presented.receipt.clone());
        app.mark_active_game_view_gpu_present_result(
            "presented",
            shared_context_status,
            presented.diagnostics,
        );
        if let Some(ticket) = &pending_ticket {
            let Some(readback) = presented.readback else {
                fail_pending_preview_frame(
                    app,
                    Some(ticket),
                    "project_preview_evidence.readback_missing",
                    "The pending Preview frame render completed without exact texture readback.",
                );
                return;
            };
            if let Err(error) =
                app.record_project_preview_presented_frame(ProjectPreviewFrameReadback {
                    game_view_session_id: readback.owner_session_id,
                    texture_id: readback.texture_id,
                    frame_index: readback.frame_index,
                    width: readback.width,
                    height: readback.height,
                    pixel_format: ProjectPreviewPixelFormat::Rgba8Unorm,
                    capture_kind: ProjectPreviewCaptureKind::RealWgpuExactSharedTexture,
                    rgba8: readback.rgba8,
                })
            {
                window_report
                    .diagnostics
                    .push(RealNativeEditorWindowDiagnostic {
                        severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                        code: error.code.to_string(),
                        message: error.message,
                        source_stage: "editor_gameview_frame_readback".to_string(),
                    });
            }
        }
    }

    #[cfg(feature = "real-wgpu-surface")]
    fn fail_pending_preview_frame(
        app: &mut NativeEditorApplication,
        pending_ticket: Option<&ProjectPreviewFrameTicket>,
        diagnostic_code: &str,
        diagnostic_message: &str,
    ) {
        if let Some(ticket) = pending_ticket {
            app.fail_project_preview_frame_capture(
                &ticket.operation_id,
                diagnostic_code,
                diagnostic_message,
            );
        }
    }

    #[cfg(feature = "real-wgpu-surface")]
    pub(crate) fn validate_preview_ticket_frame(
        ticket: &ProjectPreviewFrameTicket,
        frame: &GameViewRuntimeFrame,
    ) -> Result<(), &'static str> {
        if ticket.game_view_session_id != frame.session_id {
            return Err("project_preview_evidence.game_view_session_mismatch");
        }
        if ticket.expected_texture_id != frame.texture_id {
            return Err("project_preview_evidence.texture_mismatch");
        }
        if ticket.expected_frame_index != frame.frame_index {
            return Err("project_preview_evidence.frame_index_mismatch");
        }
        if ticket.expected_runtime_frame_hash != frame.frame_hash {
            return Err("project_preview_evidence.runtime_frame_hash_mismatch");
        }
        Ok(())
    }

    #[cfg(feature = "real-wgpu-surface")]
    pub(crate) fn validate_preview_readback_frame(
        readback: &EditorViewportTextureReadback,
        frame: &GameViewRuntimeFrame,
    ) -> Result<(), &'static str> {
        if readback.owner_session_id != frame.session_id {
            return Err("project_preview_evidence.readback_owner_mismatch");
        }
        if readback.target_id != frame.target_id {
            return Err("project_preview_evidence.readback_target_mismatch");
        }
        if readback.texture_id != frame.texture_id {
            return Err("project_preview_evidence.readback_texture_mismatch");
        }
        if readback.frame_index != frame.frame_index {
            return Err("project_preview_evidence.readback_frame_index_mismatch");
        }
        if readback.width != frame.width.max(1) || readback.height != frame.height.max(1) {
            return Err("project_preview_evidence.readback_size_mismatch");
        }
        Ok(())
    }

    fn pointer_button_from_winit(button: MouseButton) -> Option<PointerButton> {
        match button {
            MouseButton::Left => Some(PointerButton::Primary),
            MouseButton::Right => Some(PointerButton::Secondary),
            MouseButton::Middle => Some(PointerButton::Middle),
            _ => None,
        }
    }

    fn key_name(key: &Key) -> String {
        match key {
            Key::Named(NamedKey::Space) => "Space".to_string(),
            Key::Named(named) => format!("{named:?}"),
            Key::Character(text) => text.to_string(),
            Key::Unidentified(_) => "Unidentified".to_string(),
            Key::Dead(dead) => format!("Dead({dead:?})"),
        }
    }

    pub fn run_real_native_editor_window(
        model: Option<EditorUiModel>,
        options: RealNativeEditorLaunchOptions,
        linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
        project_composition_identity: Option<editor_core::ProjectEditorCompositionIdentity>,
        candidate_readiness: Option<editor_core::EditorCompositionCandidateReadiness>,
    ) -> RealNativeEditorWindowReport {
        let event_loop = match EventLoop::new() {
            Ok(event_loop) => event_loop,
            Err(error) => {
                return RealNativeEditorWindowReport::environment_blocked(
                    "winit-wgpu",
                    error.to_string(),
                );
            }
        };
        let gateway_event_proxy = event_loop.create_proxy();
        let gateway_wake: ai_tool_gateway::GatewayOwnerThreadWake = Arc::new(move || {
            let _ = gateway_event_proxy.send_event(());
        });
        let mut app =
            match RealNativeEditorApp::try_new_with_launch_options_and_gateway_wake_and_composition(
                NativeEditorWindowConfig::default(),
                model,
                options,
                Some(gateway_wake),
                linked_project_runtimes,
                project_composition_identity,
            ) {
                Ok(app) => app,
                Err(error) => {
                    return RealNativeEditorWindowReport::environment_blocked(
                        "project-launch-isolation",
                        error,
                    );
                }
            };
        app.pending_handoff_project_open = candidate_readiness
            .as_ref()
            .map(|readiness| readiness.project_root.clone());
        app.candidate_readiness = candidate_readiness;
        match event_loop.run_app(&mut app) {
            Ok(()) => app.report,
            Err(error) => {
                RealNativeEditorWindowReport::environment_blocked("winit-wgpu", error.to_string())
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AuthorityCaptureStage {
        InitialFrame,
        AwaitingOsInput,
        CaptureAfterInput,
    }

    struct RealNativeEditorCaptureApp {
        app: NativeEditorApplication,
        frame_publication: EditorFramePublicationModule,
        #[cfg(feature = "real-wgpu-surface")]
        game_view_gpu_residency: GameViewGpuResidentState,
        window: Option<Arc<Window>>,
        renderer: Option<RealWgpuUiRenderer>,
        outcome: RealNativeEditorCaptureOutcome,
        requested_physical_size: (u32, u32),
        report_level: EditorReachabilityReportLevel,
        click_widget_id: Option<String>,
        stage: AuthorityCaptureStage,
        last_cursor_position: Option<(f32, f32)>,
        input_deadline: Option<std::time::Instant>,
        wheel_delta: Option<i32>,
        drag_target_widget_id: Option<String>,
        drag_delta: Option<(i32, i32)>,
        pending_drag_target: Option<(i32, i32)>,
        pending_primary_release: Option<(i32, i32)>,
        qualify_project_lifecycle: bool,
        production_scenario: Option<crate::ProductionAuthorityScenario>,
        scenario_step_index: usize,
        scenario_started_at: Option<std::time::Instant>,
        scenario_step_started_at: Option<std::time::Instant>,
        scenario_reports: Vec<crate::ProductionAuthorityStepReport>,
        pending_game_view_coordinates: Option<crate::GameViewAuiCoordinateEvidence>,
        next_game_view_tick: std::time::Instant,
    }

    impl RealNativeEditorCaptureApp {
        fn new(options: RealNativeEditorAuthorityOptions) -> Self {
            Self::new_with_session(
                options,
                EditorSession::with_linked_project_runtimes(
                    crate::default_editor_linked_project_runtimes(),
                ),
                false,
            )
        }

        fn new_with_project_composition(
            options: RealNativeEditorAuthorityOptions,
            linked_project_runtimes: Arc<LinkedProjectRuntimeSet>,
            identity: editor_core::ProjectEditorCompositionIdentity,
        ) -> Self {
            match EditorSession::with_project_editor_composition(linked_project_runtimes, identity)
            {
                Ok(session) => Self::new_with_session(options, session, true),
                Err(error) => {
                    let config = NativeEditorWindowConfig {
                        title: "AI First Engine Editor Authority Capture".to_string(),
                        width: options.physical_width,
                        height: options.physical_height,
                        resizable: false,
                        scale_factor: 1.0,
                    };
                    Self::failed_initialization(options, config, error.to_string())
                }
            }
        }

        fn new_with_session(
            options: RealNativeEditorAuthorityOptions,
            mut session: EditorSession,
            qualify_project_lifecycle: bool,
        ) -> Self {
            let config = NativeEditorWindowConfig {
                title: "AI First Engine Editor Authority Capture".to_string(),
                width: options.physical_width,
                height: options.physical_height,
                resizable: false,
                scale_factor: 1.0,
            };
            let production_scenario = match options.scenario_path.as_ref() {
                Some(path) => match crate::ProductionAuthorityScenario::load(path) {
                    Ok(scenario) => Some(scenario),
                    Err(error) => return Self::failed_initialization(options, config, error),
                },
                None => None,
            };
            if let Some(target) = production_scenario
                .as_ref()
                .and_then(|scenario| scenario.game_view_target)
            {
                session.set_game_view_target(target);
            }
            if production_scenario.is_none() {
                if let Some(project_root) = &options.project_root {
                    let payload = editor_ui_model::UiCommandPayload::OpenProject {
                        path: project_root.display().to_string(),
                    };
                    let result = session.execute_command(editor_ui_model::UiCommand {
                        command_id: editor_ui_model::ui_command_id_for_payload(&payload)
                            .to_string(),
                        source: editor_ui_model::UiCommandSource::Test,
                        request_id: "authority-open-project".to_string(),
                        payload,
                    });
                    if result.status != editor_core::CommandStatus::Committed {
                        let message = result
                            .diagnostics
                            .first()
                            .map(|diagnostic| diagnostic.message.clone())
                            .unwrap_or_else(|| {
                                "Open project failed without diagnostics.".to_string()
                            });
                        return Self::failed_initialization(options, config, message);
                    }
                    let payload = editor_ui_model::UiCommandPayload::SelectSceneEntity {
                        entity_id: "entity-camera-main".to_string(),
                    };
                    let _selection = session.execute_command(editor_ui_model::UiCommand {
                        command_id: editor_ui_model::ui_command_id_for_payload(&payload)
                            .to_string(),
                        source: editor_ui_model::UiCommandSource::Test,
                        request_id: "authority-select-initial-entity".to_string(),
                        payload,
                    });
                }
            }
            let mut app = match production_scenario.as_ref() {
                Some(scenario) => NativeEditorApplication::with_project_manager(
                    config,
                    session,
                    ProjectManagerController::with_recent_store_path(
                        &scenario.recent_project_store_path,
                    ),
                    Box::new(NativeFolderDialogBackend),
                ),
                None => NativeEditorApplication::with_session(config, session),
            };
            if let Some(store_root) = options.workspace_layout_store_root {
                app = app
                    .with_workspace_layout_store(crate::workspace_layout_store_at_root(store_root));
            }
            Self {
                app,
                frame_publication: EditorFramePublicationModule::new(),
                #[cfg(feature = "real-wgpu-surface")]
                game_view_gpu_residency: GameViewGpuResidentState::default(),
                window: None,
                renderer: None,
                outcome: RealNativeEditorCaptureOutcome {
                    window_report: RealNativeEditorWindowReport::new("winit-wgpu-authority"),
                    native_window_id: None,
                    screen_rect: None,
                    scale_factor: 0.0,
                    physical_width: 0,
                    physical_height: 0,
                    snapshot: None,
                    capture: None,
                    capture_error: None,
                    input_replay: None,
                    present_report: None,
                    workspace_layout_revision_before: None,
                    workspace_layout_revision_after: None,
                    workspace_drag_preview_observed: false,
                    workspace_diagnostics: Vec::new(),
                    game_view_present_report: None,
                    game_view_capture: None,
                    active_runtime_after_play: false,
                    active_runtime_package_visible: false,
                    runtime_inspector_temporary: false,
                    project_lifecycle: None,
                    production_authority_report: None,
                },
                requested_physical_size: (options.physical_width, options.physical_height),
                report_level: options.report_level,
                click_widget_id: options.click_widget_id,
                stage: AuthorityCaptureStage::InitialFrame,
                last_cursor_position: None,
                input_deadline: None,
                wheel_delta: options.wheel_delta,
                drag_target_widget_id: options.drag_target_widget_id,
                drag_delta: options.drag_delta,
                pending_drag_target: None,
                pending_primary_release: None,
                qualify_project_lifecycle: qualify_project_lifecycle
                    && production_scenario.is_none(),
                production_scenario,
                scenario_step_index: 0,
                scenario_started_at: None,
                scenario_step_started_at: None,
                scenario_reports: Vec::new(),
                pending_game_view_coordinates: None,
                next_game_view_tick: std::time::Instant::now(),
            }
        }

        fn failed_initialization(
            options: RealNativeEditorAuthorityOptions,
            config: NativeEditorWindowConfig,
            message: String,
        ) -> Self {
            let mut outcome = RealNativeEditorCaptureOutcome {
                window_report: RealNativeEditorWindowReport::environment_blocked(
                    "winit-wgpu-authority",
                    message.clone(),
                ),
                native_window_id: None,
                screen_rect: None,
                scale_factor: 0.0,
                physical_width: 0,
                physical_height: 0,
                snapshot: None,
                capture: None,
                capture_error: Some(message),
                input_replay: None,
                present_report: None,
                workspace_layout_revision_before: None,
                workspace_layout_revision_after: None,
                workspace_drag_preview_observed: false,
                workspace_diagnostics: Vec::new(),
                game_view_present_report: None,
                game_view_capture: None,
                active_runtime_after_play: false,
                active_runtime_package_visible: false,
                runtime_inspector_temporary: false,
                project_lifecycle: None,
                production_authority_report: None,
            };
            outcome.window_report.diagnostics[0].source_stage =
                "authority.open_project".to_string();
            Self {
                app: NativeEditorApplication::new(config),
                frame_publication: EditorFramePublicationModule::new(),
                #[cfg(feature = "real-wgpu-surface")]
                game_view_gpu_residency: GameViewGpuResidentState::default(),
                window: None,
                renderer: None,
                outcome,
                requested_physical_size: (options.physical_width, options.physical_height),
                report_level: options.report_level,
                click_widget_id: options.click_widget_id,
                stage: AuthorityCaptureStage::InitialFrame,
                last_cursor_position: None,
                input_deadline: None,
                wheel_delta: options.wheel_delta,
                drag_target_widget_id: options.drag_target_widget_id,
                drag_delta: options.drag_delta,
                pending_drag_target: None,
                pending_primary_release: None,
                qualify_project_lifecycle: false,
                production_scenario: None,
                scenario_step_index: 0,
                scenario_started_at: None,
                scenario_step_started_at: None,
                scenario_reports: Vec::new(),
                pending_game_view_coordinates: None,
                next_game_view_tick: std::time::Instant::now(),
            }
        }

        fn fail(&mut self, code: &str, message: impl Into<String>, stage: &str) {
            self.outcome.window_report.present_status = "environment_blocked".to_string();
            self.outcome
                .window_report
                .diagnostics
                .push(RealNativeEditorWindowDiagnostic {
                    severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                    code: code.to_string(),
                    message: message.into(),
                    source_stage: stage.to_string(),
                });
        }

        fn update_snapshot(
            &mut self,
            viewport: EditorReachabilityViewport,
            app_report: &crate::NativeEditorApplicationReport,
        ) {
            let Some(tree) = self.app.retained_ui_renderer().tree() else {
                self.fail(
                    "authority_capture.widget_tree_missing",
                    "The retained editor frame did not produce a WidgetTree.",
                    "editor_ui_renderer.snapshot",
                );
                return;
            };
            let (snapshot, diagnostics) = snapshot_widget_tree(
                tree,
                EditorWidgetSnapshotContext {
                    frame_index: app_report.frame_index,
                    model_revision: app_report.model_revision,
                    viewport,
                    keyboard_focus: self.app.focus_input().keyboard_focus.as_ref(),
                    pointer_capture: self.app.focus_input().pointer_capture.as_ref(),
                    level: self.report_level,
                },
            );
            self.outcome.snapshot = Some(snapshot);
            for diagnostic in diagnostics {
                self.outcome
                    .window_report
                    .diagnostics
                    .push(RealNativeEditorWindowDiagnostic {
                        severity: match diagnostic.severity {
                            crate::EditorReachabilityDiagnosticSeverity::Info => {
                                RealNativeEditorWindowDiagnosticSeverity::Info
                            }
                            crate::EditorReachabilityDiagnosticSeverity::Warning => {
                                RealNativeEditorWindowDiagnosticSeverity::Warning
                            }
                            crate::EditorReachabilityDiagnosticSeverity::Error => {
                                RealNativeEditorWindowDiagnosticSeverity::Error
                            }
                        },
                        code: diagnostic.code,
                        message: diagnostic.message,
                        source_stage: diagnostic.source_stage,
                    });
            }
        }

        fn prepare_authority_click(&mut self) -> Result<(), String> {
            let target_widget_id = self
                .click_widget_id
                .clone()
                .ok_or_else(|| "authority_input.target_missing".to_string())?;
            let widget_id = editor_ui_renderer::WidgetId::semantic(target_widget_id.clone())
                .map_err(|error| format!("authority_input.invalid_widget_id:{error}"))?;
            let tree = self
                .app
                .retained_ui_renderer()
                .tree()
                .ok_or_else(|| "authority_input.widget_tree_missing".to_string())?;
            let node = tree
                .node(&widget_id)
                .ok_or_else(|| format!("authority_input.widget_missing:{target_widget_id}"))?;
            if node.visibility != editor_ui_renderer::WidgetVisibility::Visible {
                return Err(format!(
                    "authority_input.widget_not_visible:{target_widget_id}"
                ));
            }
            if !node.enabled {
                return Err(format!(
                    "authority_input.widget_disabled:{target_widget_id}:{}",
                    node.binding
                        .as_ref()
                        .and_then(|binding| binding.reason_disabled.as_deref())
                        .unwrap_or("reason_missing")
                ));
            }
            let hit_rect = node.effective_clip.map_or(Some(node.logical_rect), |clip| {
                node.logical_rect.intersection(clip)
            });
            let hit_rect = hit_rect
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                .ok_or_else(|| format!("authority_input.widget_clipped:{target_widget_id}"))?;
            let logical_x = f64::from(hit_rect.x + hit_rect.width * 0.5);
            let logical_y = f64::from(hit_rect.y + hit_rect.height * 0.5);
            let client_x = (logical_x * self.outcome.scale_factor).round() as i32;
            let client_y = (logical_y * self.outcome.scale_factor).round() as i32;
            if client_x < 0
                || client_y < 0
                || client_x >= self.outcome.physical_width as i32
                || client_y >= self.outcome.physical_height as i32
            {
                return Err(format!(
                    "authority_input.coordinate_out_of_bounds:{client_x},{client_y}"
                ));
            }
            let before = self.app.report();
            self.outcome.workspace_layout_revision_before = Some(
                self.app
                    .workspace_docking()
                    .snapshot(editor_ui_renderer::editor_workspace_rect(
                        self.outcome.physical_width as f32
                            / self.outcome.scale_factor.max(1.0) as f32,
                        self.outcome.physical_height as f32
                            / self.outcome.scale_factor.max(1.0) as f32,
                    ))
                    .layout_revision,
            );
            let command_id = node
                .binding
                .as_ref()
                .map(|binding| binding.command_id.clone());
            let mut evidence = crate::EditorInputReplayEvidence {
                input_kind: if self.wheel_delta.is_some() {
                    "wheel".to_string()
                } else if self.drag_target_widget_id.is_some() || self.drag_delta.is_some() {
                    "primary_drag".to_string()
                } else {
                    "primary_click".to_string()
                },
                target_widget_id,
                command_id,
                client_logical_x: logical_x,
                client_logical_y: logical_y,
                client_physical_x: client_x,
                client_physical_y: client_y,
                screen_physical_x: None,
                screen_physical_y: None,
                target_pid: None,
                foreground_verified: false,
                pointer_down_observed: false,
                pointer_up_observed: false,
                wheel_observed: false,
                before_command_id: before.last_command_id,
                after_command_id: None,
                before_model_revision: before.model_revision,
                after_model_revision: before.model_revision,
                focused_widget_id: None,
                route_status: crate::EditorReachabilityStatus::NotEvaluated,
                diagnostics: Vec::new(),
            };
            #[cfg(target_os = "windows")]
            {
                let window = self
                    .window
                    .as_ref()
                    .ok_or_else(|| "authority_input.window_missing".to_string())?;
                let receipt = if let Some(delta) = self.wheel_delta {
                    crate::send_authority_mouse_wheel(window, client_x, client_y, delta)?
                } else if self.drag_target_widget_id.is_some() || self.drag_delta.is_some() {
                    let (end_x, end_y) = self.authority_drag_target(client_x, client_y)?;
                    self.pending_drag_target = Some((end_x, end_y));
                    crate::begin_authority_primary_drag(window, client_x, client_y)?
                } else {
                    self.pending_primary_release = Some((client_x, client_y));
                    crate::begin_authority_primary_click(window, client_x, client_y)?
                };
                evidence.screen_physical_x = Some(receipt.screen_x);
                evidence.screen_physical_y = Some(receipt.screen_y);
                evidence.target_pid = Some(receipt.target_pid);
                evidence.foreground_verified = receipt.foreground_verified;
            }
            #[cfg(not(target_os = "windows"))]
            {
                return Err("authority_input.windows_only".to_string());
            }
            self.outcome.input_replay = Some(evidence);
            self.stage = AuthorityCaptureStage::AwaitingOsInput;
            self.input_deadline =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            Ok(())
        }

        fn authority_drag_target(
            &self,
            start_client_x: i32,
            start_client_y: i32,
        ) -> Result<(i32, i32), String> {
            if let Some(target_widget_id) = &self.drag_target_widget_id {
                let widget_id = editor_ui_renderer::WidgetId::semantic(target_widget_id.clone())
                    .map_err(|error| format!("authority_input.invalid_drag_target:{error}"))?;
                let tree = self
                    .app
                    .retained_ui_renderer()
                    .tree()
                    .ok_or_else(|| "authority_input.widget_tree_missing".to_string())?;
                let node = tree.node(&widget_id).ok_or_else(|| {
                    format!("authority_input.drag_target_missing:{target_widget_id}")
                })?;
                let rect = node.effective_clip.map_or(Some(node.logical_rect), |clip| {
                    node.logical_rect.intersection(clip)
                });
                let rect = rect
                    .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                    .ok_or_else(|| {
                        format!("authority_input.drag_target_clipped:{target_widget_id}")
                    })?;
                return Ok((
                    ((rect.x + rect.width * 0.5) * self.outcome.scale_factor as f32).round() as i32,
                    ((rect.y + rect.height * 0.5) * self.outcome.scale_factor as f32).round()
                        as i32,
                ));
            }
            let (delta_x, delta_y) = self
                .drag_delta
                .ok_or_else(|| "authority_input.drag_target_missing".to_string())?;
            Ok((start_client_x + delta_x, start_client_y + delta_y))
        }

        fn begin_scenario_primary_click(
            &mut self,
            target: String,
            command_id: Option<String>,
            logical_point: UiPoint,
        ) -> Result<(), String> {
            let logical_x = f64::from(logical_point.x);
            let logical_y = f64::from(logical_point.y);
            let client_x = (logical_x * self.outcome.scale_factor).round() as i32;
            let client_y = (logical_y * self.outcome.scale_factor).round() as i32;
            if client_x < 0
                || client_y < 0
                || client_x >= self.outcome.physical_width as i32
                || client_y >= self.outcome.physical_height as i32
            {
                return Err(format!(
                    "authority_input.coordinate_out_of_bounds:{client_x},{client_y}"
                ));
            }
            let before = self.app.report();
            let window = self
                .window
                .as_ref()
                .ok_or_else(|| "authority_input.window_missing".to_string())?;
            #[cfg(target_os = "windows")]
            let receipt = crate::begin_authority_primary_click(window, client_x, client_y)?;
            #[cfg(not(target_os = "windows"))]
            return Err("authority_input.windows_only".to_string());
            self.pending_primary_release = Some((client_x, client_y));
            self.outcome.input_replay = Some(crate::EditorInputReplayEvidence {
                input_kind: "primary_click".to_string(),
                target_widget_id: target,
                command_id,
                client_logical_x: logical_x,
                client_logical_y: logical_y,
                client_physical_x: client_x,
                client_physical_y: client_y,
                screen_physical_x: Some(receipt.screen_x),
                screen_physical_y: Some(receipt.screen_y),
                target_pid: Some(receipt.target_pid),
                foreground_verified: receipt.foreground_verified,
                pointer_down_observed: false,
                pointer_up_observed: false,
                wheel_observed: false,
                before_command_id: before.last_command_id,
                after_command_id: None,
                before_model_revision: before.model_revision,
                after_model_revision: before.model_revision,
                focused_widget_id: None,
                route_status: crate::EditorReachabilityStatus::NotEvaluated,
                diagnostics: Vec::new(),
            });
            self.stage = AuthorityCaptureStage::AwaitingOsInput;
            self.input_deadline = Some(
                std::time::Instant::now()
                    + std::time::Duration::from_millis(self.production_scenario.as_ref().map_or(
                        5_000,
                        |scenario| {
                            scenario
                                .steps
                                .get(self.scenario_step_index)
                                .and_then(crate::ProductionAuthorityStep::timeout_ms)
                                .unwrap_or(scenario.per_step_timeout_ms)
                        },
                    )),
            );
            Ok(())
        }

        fn process_production_scenario_step(&mut self, event_loop: &ActiveEventLoop) -> bool {
            let Some(scenario) = self.production_scenario.clone() else {
                return false;
            };
            let now = std::time::Instant::now();
            let started = *self.scenario_started_at.get_or_insert(now);
            if now.duration_since(started).as_millis() as u64 > scenario.overall_timeout_ms {
                self.fail_production_scenario(event_loop, "authority.scenario_overall_timeout");
                return true;
            }
            if self.scenario_step_index >= scenario.steps.len() {
                self.finish_production_scenario(crate::ProductionAuthorityTerminal::Passed);
                event_loop.exit();
                return true;
            }
            let step = scenario.steps[self.scenario_step_index].clone();
            let step_started = *self.scenario_step_started_at.get_or_insert(now);
            let step_timeout_ms = step.timeout_ms().unwrap_or(scenario.per_step_timeout_ms);
            if now.duration_since(step_started).as_millis() as u64 > step_timeout_ms {
                if let crate::ProductionAuthorityStep::WaitFor {
                    step_id,
                    condition:
                        crate::ProductionAuthorityCondition::ProjectValueEquals { path, equals },
                    ..
                } = &step
                {
                    let evaluation = self.production_project_value_evaluation(path, equals);
                    self.push_scenario_report(self.project_value_step_report(
                        step_id.clone(),
                        "failed",
                        now.duration_since(step_started).as_millis() as u64,
                        step_timeout_ms,
                        evaluation,
                        vec!["authority.scenario_step_timeout".to_string()],
                    ));
                }
                self.fail_production_scenario(event_loop, "authority.scenario_step_timeout");
                return true;
            }
            if self.app.session().has_active_editor_runtime_play_instance() {
                self.app
                    .session_mut()
                    .set_active_game_view_project_runtime_report_level(
                        engine_runtime::project_runtime_session::ProjectRuntimeSessionReportLevel::Trace,
                    );
            }
            match step {
                crate::ProductionAuthorityStep::ClickEditorWidget {
                    step_id: _,
                    widget_id,
                } => {
                    let result = (|| {
                        let widget_id_value = editor_ui_renderer::WidgetId::semantic(
                            widget_id.clone(),
                        )
                        .map_err(|error| format!("authority_input.invalid_widget_id:{error}"))?;
                        let tree = self
                            .app
                            .retained_ui_renderer()
                            .tree()
                            .ok_or_else(|| "authority_input.widget_tree_missing".to_string())?;
                        let node = tree
                            .node(&widget_id_value)
                            .ok_or_else(|| format!("authority_input.widget_missing:{widget_id}"))?;
                        if node.visibility != editor_ui_renderer::WidgetVisibility::Visible
                            || !node.enabled
                        {
                            return Err(format!(
                                "authority_input.widget_not_actionable:{widget_id}"
                            ));
                        }
                        let rect = node.effective_clip.map_or(Some(node.logical_rect), |clip| {
                            node.logical_rect.intersection(clip)
                        });
                        let rect = rect
                            .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                            .ok_or_else(|| format!("authority_input.widget_clipped:{widget_id}"))?;
                        let command_id = node
                            .binding
                            .as_ref()
                            .map(|binding| binding.command_id.clone());
                        Ok((
                            UiPoint {
                                x: rect.x + rect.width * 0.5,
                                y: rect.y + rect.height * 0.5,
                            },
                            command_id,
                        ))
                    })();
                    match result.and_then(|(point, command_id)| {
                        self.begin_scenario_primary_click(widget_id, command_id, point)
                    }) {
                        Ok(()) => {
                            event_loop.set_control_flow(ControlFlow::WaitUntil(
                                self.input_deadline.expect("scenario input deadline"),
                            ));
                        }
                        Err(error) if scenario_actionability_pending(&error) => {
                            self.defer_production_scenario_step(event_loop)
                        }
                        Err(error) => self.fail_production_scenario(event_loop, &error),
                    }
                    true
                }
                crate::ProductionAuthorityStep::ClickGameViewAuiNode {
                    step_id: _,
                    node_id,
                    expected_action_id,
                } => {
                    match self.app.game_view_aui_action_logical_point(&node_id) {
                        Ok((point, action_id, coordinates)) => {
                            if expected_action_id
                                .as_ref()
                                .is_some_and(|expected| expected != &action_id)
                            {
                                self.fail_production_scenario(
                                    event_loop,
                                    "authority.aui_target_action_mismatch",
                                );
                            } else if let Err(error) =
                                self.begin_scenario_primary_click(node_id, Some(action_id), point)
                            {
                                self.fail_production_scenario(event_loop, &error);
                            } else {
                                self.pending_game_view_coordinates = Some(coordinates);
                                event_loop.set_control_flow(ControlFlow::WaitUntil(
                                    self.input_deadline.expect("scenario input deadline"),
                                ));
                            }
                        }
                        Err(error) if scenario_actionability_pending(&error) => {
                            self.defer_production_scenario_step(event_loop)
                        }
                        Err(error) => self.fail_production_scenario(event_loop, &error),
                    }
                    true
                }
                crate::ProductionAuthorityStep::WaitFor {
                    step_id,
                    timeout_ms: _,
                    condition,
                } => {
                    if let crate::ProductionAuthorityCondition::ProjectValueEquals {
                        path,
                        equals,
                    } = &condition
                    {
                        let evaluation = self.production_project_value_evaluation(path, equals);
                        match evaluation.status {
                            crate::ProductionAuthorityConditionStatus::Passed => {
                                self.push_scenario_report(self.project_value_step_report(
                                    step_id,
                                    "passed",
                                    now.duration_since(step_started).as_millis() as u64,
                                    step_timeout_ms,
                                    evaluation,
                                    Vec::new(),
                                ));
                                self.advance_scenario_step();
                            }
                            crate::ProductionAuthorityConditionStatus::Pending => {}
                            crate::ProductionAuthorityConditionStatus::Failed => {
                                let diagnostic =
                                    evaluation.diagnostic_code.clone().unwrap_or_else(|| {
                                        "authority.project_observation_failed".to_string()
                                    });
                                self.push_scenario_report(self.project_value_step_report(
                                    step_id,
                                    "failed",
                                    now.duration_since(step_started).as_millis() as u64,
                                    step_timeout_ms,
                                    evaluation,
                                    vec![diagnostic.clone()],
                                ));
                                self.fail_production_scenario(event_loop, &diagnostic);
                            }
                        }
                    } else if self.production_condition_matches(&condition) {
                        self.push_scenario_report(crate::ProductionAuthorityStepReport {
                            step_id,
                            kind: "waitFor".to_string(),
                            target: Some(format!("{condition:?}")),
                            status: "passed".to_string(),
                            actionable: None,
                            pointer_down_observed: false,
                            pointer_up_observed: false,
                            before_command_id: self.app.report().last_command_id.clone(),
                            after_command_id: self.app.report().last_command_id,
                            runtime_action_id: self.latest_runtime_action_id(),
                            runtime_frame_index: self
                                .app
                                .session()
                                .last_game_view_runtime_frame()
                                .map(|frame| frame.frame_index),
                            runtime_session_id: self
                                .app
                                .session()
                                .last_game_view_runtime_frame()
                                .map(|frame| frame.session_id.clone()),
                            game_view_coordinates: None,
                            viewport_input_route: None,
                            observation_path: None,
                            observation_declared_type: None,
                            observation_expected: None,
                            observation_last_actual: None,
                            observation_contract_id: None,
                            timeout_ms: Some(step_timeout_ms),
                            screenshot_path: None,
                            screenshot_sha256: None,
                            elapsed_ms: now.duration_since(step_started).as_millis() as u64,
                            diagnostics: Vec::new(),
                        });
                        self.advance_scenario_step();
                    }
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        now + std::time::Duration::from_millis(16),
                    ));
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    true
                }
                crate::ProductionAuthorityStep::Capture {
                    step_id,
                    checkpoint_id,
                } => {
                    match self.capture_scenario_checkpoint(&scenario, &checkpoint_id) {
                        Ok((path, hash)) => {
                            self.push_scenario_report(crate::ProductionAuthorityStepReport {
                                step_id,
                                kind: "capture".to_string(),
                                target: Some(checkpoint_id),
                                status: "passed".to_string(),
                                actionable: None,
                                pointer_down_observed: false,
                                pointer_up_observed: false,
                                before_command_id: self.app.report().last_command_id.clone(),
                                after_command_id: self.app.report().last_command_id,
                                runtime_action_id: self.latest_runtime_action_id(),
                                runtime_frame_index: self
                                    .app
                                    .session()
                                    .last_game_view_runtime_frame()
                                    .map(|frame| frame.frame_index),
                                runtime_session_id: self
                                    .app
                                    .session()
                                    .last_game_view_runtime_frame()
                                    .map(|frame| frame.session_id.clone()),
                                game_view_coordinates: None,
                                viewport_input_route: None,
                                observation_path: None,
                                observation_declared_type: None,
                                observation_expected: None,
                                observation_last_actual: None,
                                observation_contract_id: None,
                                timeout_ms: None,
                                screenshot_path: Some(path),
                                screenshot_sha256: Some(hash),
                                elapsed_ms: now.duration_since(step_started).as_millis() as u64,
                                diagnostics: Vec::new(),
                            });
                            self.advance_scenario_step();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        Err(error) => self.fail_production_scenario(event_loop, &error),
                    }
                    true
                }
            }
        }

        fn production_condition_matches(
            &self,
            condition: &crate::ProductionAuthorityCondition,
        ) -> bool {
            match condition {
                crate::ProductionAuthorityCondition::EditorMode { mode } => {
                    format!("{:?}", self.app.report().mode).eq_ignore_ascii_case(mode)
                }
                crate::ProductionAuthorityCondition::LastCommandId { command_id } => self
                    .app
                    .report()
                    .last_command_id
                    .as_deref()
                    .is_some_and(|candidate| candidate == command_id),
                crate::ProductionAuthorityCondition::ActiveRuntime { active } => {
                    self.app.session().has_active_editor_runtime_play_instance() == *active
                }
                crate::ProductionAuthorityCondition::RuntimeFrameAtLeast { frame_index } => self
                    .app
                    .session()
                    .last_game_view_runtime_frame()
                    .is_some_and(|frame| frame.frame_index >= *frame_index),
                crate::ProductionAuthorityCondition::RuntimeFrameAdvancedSinceStep {
                    step_id,
                    minimum_delta,
                } => {
                    let previous = self
                        .scenario_reports
                        .iter()
                        .find(|report| report.step_id == *step_id)
                        .and_then(|report| report.runtime_frame_index);
                    previous.is_some_and(|previous| {
                        self.app
                            .session()
                            .last_game_view_runtime_frame()
                            .is_some_and(|frame| {
                                frame.frame_index >= previous.saturating_add(*minimum_delta)
                            })
                    })
                }
                crate::ProductionAuthorityCondition::RuntimeActionId { action_id } => {
                    self.latest_runtime_action_id().as_deref() == Some(action_id)
                }
                crate::ProductionAuthorityCondition::GameViewAuiNodeActionable { node_id } => {
                    self.app.game_view_aui_action_logical_point(node_id).is_ok()
                }
                crate::ProductionAuthorityCondition::RuntimeSessionChanged {
                    previous_session_id,
                } => self
                    .app
                    .session()
                    .last_game_view_runtime_frame()
                    .is_some_and(|frame| frame.session_id != *previous_session_id),
                crate::ProductionAuthorityCondition::RuntimeSessionChangedSinceStep { step_id } => {
                    let previous = self
                        .scenario_reports
                        .iter()
                        .find(|report| report.step_id == *step_id)
                        .and_then(|report| report.runtime_session_id.as_deref());
                    previous.is_some_and(|previous| {
                        self.app
                            .session()
                            .last_game_view_runtime_frame()
                            .is_some_and(|frame| frame.session_id != previous)
                    })
                }
                crate::ProductionAuthorityCondition::ProjectValueEquals { .. } => false,
            }
        }

        fn production_project_value_evaluation(
            &self,
            path: &str,
            expected: &serde_json::Value,
        ) -> crate::ProductionAuthorityProjectValueEvaluation {
            let report = self.app.session().last_game_view_present_report();
            let active_session_id = report
                .and_then(|report| report.project_runtime_bind_receipt.as_ref())
                .map(|receipt| receipt.session_id.as_str());
            let latest_mutating_action_frame = self
                .scenario_reports
                .iter()
                .rev()
                .find(|report| report.runtime_action_id.is_some())
                .and_then(|report| report.runtime_frame_index);
            crate::evaluate_project_value_condition(
                path,
                expected,
                self.app
                    .session()
                    .last_game_view_project_observation_state(),
                active_session_id,
                latest_mutating_action_frame,
            )
        }

        fn project_value_step_report(
            &self,
            step_id: String,
            status: &str,
            elapsed_ms: u64,
            timeout_ms: u64,
            evaluation: crate::ProductionAuthorityProjectValueEvaluation,
            diagnostics: Vec<String>,
        ) -> crate::ProductionAuthorityStepReport {
            crate::ProductionAuthorityStepReport {
                step_id,
                kind: "waitFor".to_string(),
                target: Some(format!("projectValueEquals:{}", evaluation.path)),
                status: status.to_string(),
                actionable: None,
                pointer_down_observed: false,
                pointer_up_observed: false,
                before_command_id: self.app.report().last_command_id.clone(),
                after_command_id: self.app.report().last_command_id,
                runtime_action_id: self.latest_runtime_action_id(),
                runtime_frame_index: evaluation.runtime_frame,
                runtime_session_id: evaluation.session_id,
                game_view_coordinates: None,
                viewport_input_route: None,
                observation_path: Some(evaluation.path),
                observation_declared_type: evaluation
                    .declared_type
                    .map(|value| value.as_str().to_string()),
                observation_expected: Some(evaluation.expected),
                observation_last_actual: evaluation.last_actual,
                observation_contract_id: evaluation.contract_id,
                timeout_ms: Some(timeout_ms),
                screenshot_path: None,
                screenshot_sha256: None,
                elapsed_ms,
                diagnostics,
            }
        }

        fn defer_production_scenario_step(&self, event_loop: &ActiveEventLoop) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            ));
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        fn latest_runtime_action_id(&self) -> Option<String> {
            self.app
                .session()
                .last_game_view_present_report()
                .and_then(|report| report.project_runtime_session_report.as_ref())
                .and_then(|report| {
                    report
                        .stages
                        .iter()
                        .flat_map(|stage| stage.action_trace.iter())
                        .last()
                })
                .map(|action| action.action_id.clone())
        }

        fn capture_scenario_checkpoint(
            &mut self,
            scenario: &crate::ProductionAuthorityScenario,
            checkpoint_id: &str,
        ) -> Result<(String, String), String> {
            if checkpoint_id.is_empty()
                || !checkpoint_id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character))
            {
                return Err("authority.checkpoint_id_invalid".to_string());
            }
            let renderer = self
                .renderer
                .as_mut()
                .ok_or_else(|| "authority_capture.renderer_missing".to_string())?;
            let (present, capture) =
                renderer.present_with_rgba_capture(self.app.latest_draw_list());
            self.outcome.window_report.apply_ui_present_report(&present);
            let capture =
                capture.map_err(|error| format!("authority.checkpoint_capture_failed:{error}"))?;
            let scenario_root = scenario.evidence_root.join(&scenario.scenario_id);
            std::fs::create_dir_all(&scenario_root).map_err(|error| {
                format!(
                    "authority.checkpoint_root_create_failed:{}:{error}",
                    scenario_root.display()
                )
            })?;
            let path = scenario_root.join(format!("{checkpoint_id}.png"));
            let file = std::fs::File::create(&path)
                .map_err(|error| format!("authority.checkpoint_create_failed:{error}"))?;
            let mut encoder =
                png::Encoder::new(std::io::BufWriter::new(file), capture.width, capture.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|error| format!("authority.checkpoint_header_failed:{error}"))?;
            writer
                .write_image_data(&capture.rgba8)
                .map_err(|error| format!("authority.checkpoint_write_failed:{error}"))?;
            Ok((
                path.display().to_string(),
                engine_runtime::canonical_digest::sha256_prefixed(&capture.rgba8),
            ))
        }

        fn push_scenario_report(&mut self, report: crate::ProductionAuthorityStepReport) {
            self.scenario_reports.push(report);
        }

        fn advance_scenario_step(&mut self) {
            self.scenario_step_index += 1;
            self.scenario_step_started_at = None;
            self.outcome.input_replay = None;
            self.stage = AuthorityCaptureStage::InitialFrame;
        }

        fn complete_scenario_click_step(&mut self) {
            let Some(scenario) = self.production_scenario.as_ref() else {
                return;
            };
            let Some(step) = scenario.steps.get(self.scenario_step_index) else {
                return;
            };
            let now = std::time::Instant::now();
            let started = self.scenario_step_started_at.unwrap_or(now);
            let evidence = self.outcome.input_replay.as_ref();
            let (kind, target, expected_action) = match step {
                crate::ProductionAuthorityStep::ClickEditorWidget { widget_id, .. } => {
                    ("clickEditorWidget", Some(widget_id.clone()), None)
                }
                crate::ProductionAuthorityStep::ClickGameViewAuiNode {
                    node_id,
                    expected_action_id,
                    ..
                } => (
                    "clickGameViewAuiNode",
                    Some(node_id.clone()),
                    expected_action_id.clone(),
                ),
                _ => return,
            };
            let runtime_action_id = self.latest_runtime_action_id();
            let mut diagnostics = Vec::new();
            if expected_action
                .as_ref()
                .is_some_and(|expected| runtime_action_id.as_ref() != Some(expected))
            {
                diagnostics.push("authority.runtime_action_not_observed".to_string());
            }
            self.scenario_reports
                .push(crate::ProductionAuthorityStepReport {
                    step_id: step.step_id().to_string(),
                    kind: kind.to_string(),
                    target,
                    status: if diagnostics.is_empty() {
                        "passed".to_string()
                    } else {
                        "failed".to_string()
                    },
                    actionable: Some(true),
                    pointer_down_observed: evidence
                        .is_some_and(|value| value.pointer_down_observed),
                    pointer_up_observed: evidence.is_some_and(|value| value.pointer_up_observed),
                    before_command_id: evidence.and_then(|value| value.before_command_id.clone()),
                    after_command_id: evidence.and_then(|value| value.after_command_id.clone()),
                    runtime_action_id,
                    runtime_frame_index: self
                        .app
                        .session()
                        .last_game_view_runtime_frame()
                        .map(|frame| frame.frame_index),
                    runtime_session_id: self
                        .app
                        .session()
                        .last_game_view_runtime_frame()
                        .map(|frame| frame.session_id.clone()),
                    game_view_coordinates: if kind == "clickGameViewAuiNode" {
                        self.pending_game_view_coordinates.take()
                    } else {
                        None
                    },
                    viewport_input_route: if kind == "clickGameViewAuiNode" {
                        self.app.last_viewport_input_route().cloned()
                    } else {
                        None
                    },
                    observation_path: None,
                    observation_declared_type: None,
                    observation_expected: None,
                    observation_last_actual: None,
                    observation_contract_id: None,
                    timeout_ms: None,
                    screenshot_path: None,
                    screenshot_sha256: None,
                    elapsed_ms: now.duration_since(started).as_millis() as u64,
                    diagnostics: diagnostics.clone(),
                });
            if diagnostics.is_empty() {
                self.advance_scenario_step();
            }
        }

        fn fail_production_scenario(&mut self, event_loop: &ActiveEventLoop, diagnostic: &str) {
            self.finish_production_scenario(crate::ProductionAuthorityTerminal::Failed {
                diagnostic: diagnostic.to_string(),
            });
            self.fail(
                "authority.scenario_failed",
                diagnostic,
                "production_authority_scenario",
            );
            event_loop.exit();
        }

        fn finish_production_scenario(&mut self, terminal: crate::ProductionAuthorityTerminal) {
            crate::finalize_production_authority_report_once(
                self.production_scenario.as_ref(),
                self.scenario_started_at,
                &self.scenario_reports,
                &mut self.outcome.production_authority_report,
                terminal,
            );
        }

        fn finish_production_scenario_for_close_requested(&mut self) {
            if self.production_scenario.is_some() {
                self.finish_production_scenario(crate::ProductionAuthorityTerminal::Failed {
                    diagnostic: "authority.window_close_requested".to_string(),
                });
            }
        }

        fn ensure_production_scenario_terminal_report(&mut self, diagnostic: &str) {
            crate::ensure_production_authority_terminal_report(
                self.production_scenario.as_ref(),
                self.scenario_started_at,
                &self.scenario_reports,
                &mut self.outcome.production_authority_report,
                diagnostic,
            );
        }
    }

    impl ApplicationHandler for RealNativeEditorCaptureApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let attributes = winit_window_attributes(self.app.config());
            match event_loop.create_window(attributes) {
                Ok(window) => {
                    let window = Arc::new(window);
                    let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                        self.requested_physical_size.0,
                        self.requested_physical_size.1,
                    ));
                    window.focus_window();
                    self.outcome.scale_factor = window.scale_factor();
                    self.outcome.native_window_id = Some(format!("{:?}", window.id()));
                    self.outcome.window_report.window_created = true;
                    match RealWgpuUiRenderer::new(window.clone()) {
                        Ok(renderer) => {
                            self.outcome.window_report.surface_created = true;
                            self.outcome.window_report.surface_configured = true;
                            self.outcome.window_report.device_created = true;
                            self.outcome.window_report.apply_shared_gpu_context_summary(
                                &renderer.shared_context_summary(),
                            );
                            self.renderer = Some(renderer);
                            self.window = Some(window.clone());
                            window.request_redraw();
                        }
                        Err(error) => {
                            self.fail(
                                "authority_capture.gpu_init_failed",
                                error,
                                "editor_wgpu_renderer.new",
                            );
                            event_loop.exit();
                        }
                    }
                }
                Err(error) => {
                    self.fail(
                        "authority_capture.window_create_failed",
                        error.to_string(),
                        "winit.create_window",
                    );
                    event_loop.exit();
                }
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::Resized(size) => {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.resize(size.width, size.height);
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let logical = physical_to_logical(
                        PhysicalPoint {
                            x: position.x,
                            y: position.y,
                        },
                        self.outcome.scale_factor,
                    );
                    let x = logical.x as f32;
                    let y = logical.y as f32;
                    self.last_cursor_position = Some((x, y));
                    self.app
                        .handle_input_event(EditorInputEvent::PointerMove { x, y });
                    if self
                        .app
                        .workspace_docking()
                        .snapshot(editor_ui_renderer::editor_workspace_rect(
                            self.outcome.physical_width as f32
                                / self.outcome.scale_factor.max(1.0) as f32,
                            self.outcome.physical_height as f32
                                / self.outcome.scale_factor.max(1.0) as f32,
                        ))
                        .drag_preview
                        .is_some()
                    {
                        self.outcome.workspace_drag_preview_observed = true;
                    }
                    if self
                        .pending_drag_target
                        .is_some_and(|(target_x, target_y)| {
                            (position.x - f64::from(target_x)).abs() <= 2.0
                                && (position.y - f64::from(target_y)).abs() <= 2.0
                        })
                    {
                        match crate::finish_authority_primary_drag() {
                            Ok(()) => self.pending_drag_target = None,
                            Err(error) => {
                                self.fail(
                                    "authority_input.drag_finish_failed",
                                    error,
                                    "windows.send_input",
                                );
                                event_loop.exit();
                                return;
                            }
                        }
                    }
                    self.outcome.window_report.input_event_count += 1;
                }
                WindowEvent::MouseInput { state, button, .. }
                    if self.stage == AuthorityCaptureStage::AwaitingOsInput =>
                {
                    let Some(button) = pointer_button_from_winit(button) else {
                        return;
                    };
                    let (x, y) = self.last_cursor_position.unwrap_or((0.0, 0.0));
                    let input = match state {
                        ElementState::Pressed => EditorInputEvent::PointerDown { x, y, button },
                        ElementState::Released => EditorInputEvent::PointerUp { x, y, button },
                    };
                    let before_command = self.app.report().last_command_id;
                    let app_report = self.app.handle_input_event(input);
                    let mut scenario_click_released = false;
                    self.outcome.window_report.input_event_count += 1;
                    if app_report.last_command_id != before_command {
                        self.outcome.window_report.ui_command_count += 1;
                    }
                    if state == ElementState::Pressed {
                        let pending_release = self.pending_primary_release.take();
                        if let Some((client_x, client_y)) = pending_release {
                            let Some(window) = &self.window else {
                                self.fail(
                                    "authority_input.window_missing",
                                    "The authority window disappeared during click.",
                                    "windows.authority_input",
                                );
                                event_loop.exit();
                                return;
                            };
                            if let Err(error) =
                                crate::finish_authority_primary_click(window, client_x, client_y)
                            {
                                self.fail(
                                    "authority_input.click_finish_failed",
                                    error,
                                    "windows.authority_input",
                                );
                                event_loop.exit();
                                return;
                            }
                        }
                    }
                    if let Some(evidence) = &mut self.outcome.input_replay {
                        match state {
                            ElementState::Pressed => {
                                evidence.pointer_down_observed = true;
                                if let Some((end_x, end_y)) = self.pending_drag_target {
                                    let Some(window) = &self.window else {
                                        self.fail(
                                            "authority_input.window_missing",
                                            "The authority window disappeared during drag.",
                                            "windows.send_input",
                                        );
                                        event_loop.exit();
                                        return;
                                    };
                                    if let Err(error) =
                                        crate::move_authority_primary_drag(window, end_x, end_y)
                                    {
                                        self.fail(
                                            "authority_input.drag_finish_failed",
                                            error,
                                            "windows.send_input",
                                        );
                                        event_loop.exit();
                                    }
                                }
                            }
                            ElementState::Released => {
                                evidence.pointer_up_observed = true;
                                evidence.after_command_id = app_report.last_command_id;
                                evidence.after_model_revision = app_report.model_revision;
                                evidence.focused_widget_id = self
                                    .app
                                    .focus_input()
                                    .keyboard_focus
                                    .as_ref()
                                    .map(|id| id.as_str().to_string());
                                evidence.route_status = if evidence.pointer_down_observed
                                    && evidence.foreground_verified
                                {
                                    crate::EditorReachabilityStatus::Passed
                                } else {
                                    crate::EditorReachabilityStatus::Failed
                                };
                                let workspace = self.app.workspace_docking().snapshot(
                                    editor_ui_renderer::editor_workspace_rect(
                                        self.outcome.physical_width as f32
                                            / self.outcome.scale_factor.max(1.0) as f32,
                                        self.outcome.physical_height as f32
                                            / self.outcome.scale_factor.max(1.0) as f32,
                                    ),
                                );
                                self.outcome.workspace_layout_revision_after =
                                    Some(workspace.layout_revision);
                                self.outcome.workspace_diagnostics = workspace
                                    .diagnostics
                                    .iter()
                                    .map(|diagnostic| diagnostic.code.clone())
                                    .collect();
                                if self.production_scenario.is_some() {
                                    scenario_click_released = true;
                                } else {
                                    self.stage = AuthorityCaptureStage::CaptureAfterInput;
                                    if let Some(window) = &self.window {
                                        window.request_redraw();
                                    }
                                }
                            }
                        }
                    }
                    if scenario_click_released {
                        self.complete_scenario_click_step();
                        let failed = self
                            .scenario_reports
                            .last()
                            .is_some_and(|report| report.status == "failed");
                        if failed {
                            self.fail_production_scenario(
                                event_loop,
                                "authority.runtime_action_not_observed",
                            );
                        } else if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. }
                    if self.stage == AuthorityCaptureStage::AwaitingOsInput =>
                {
                    let delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(position) => position.y as f32,
                    };
                    let app_report = self
                        .app
                        .handle_input_event(EditorInputEvent::MouseWheel { delta });
                    self.outcome.window_report.input_event_count += 1;
                    if let Some(evidence) = &mut self.outcome.input_replay {
                        evidence.wheel_observed = true;
                        evidence.after_command_id = app_report.last_command_id;
                        evidence.after_model_revision = app_report.model_revision;
                        evidence.focused_widget_id = self
                            .app
                            .focus_input()
                            .keyboard_focus
                            .as_ref()
                            .map(|id| id.as_str().to_string());
                        evidence.route_status = if evidence.foreground_verified {
                            crate::EditorReachabilityStatus::Passed
                        } else {
                            crate::EditorReachabilityStatus::Failed
                        };
                    }
                    self.stage = AuthorityCaptureStage::CaptureAfterInput;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                WindowEvent::RedrawRequested => {
                    let Some(window) = &self.window else {
                        self.fail(
                            "authority_capture.window_missing",
                            "The authority window disappeared before capture.",
                            "winit.redraw",
                        );
                        event_loop.exit();
                        return;
                    };
                    let size = window.inner_size();
                    self.outcome.physical_width = size.width;
                    self.outcome.physical_height = size.height;
                    self.outcome.scale_factor = window.scale_factor();
                    if let Ok(outer) = window.outer_position() {
                        self.outcome.screen_rect =
                            Some((outer.x, outer.y, size.width, size.height));
                    }
                    let viewport = EditorReachabilityViewport::from_physical(
                        size.width,
                        size.height,
                        self.outcome.scale_factor,
                    );
                    let app_report = self.app.frame(
                        viewport.logical_width as f32,
                        viewport.logical_height as f32,
                    );
                    self.outcome.window_report.frame_index = app_report.frame_index;
                    self.outcome.window_report.hit_region_count = app_report.hit_region_count;
                    self.update_snapshot(viewport, &app_report);
                    self.outcome.active_runtime_after_play =
                        self.app.session().has_active_editor_runtime_play_instance();
                    self.outcome.active_runtime_package_visible =
                        self.app.latest_model().active_runtime_package.is_some();
                    if self.qualify_project_lifecycle
                        && self.outcome.active_runtime_after_play
                        && crate::qualify_active_project_editor_composition_runtime_inspector(
                            self.app.session_mut(),
                        )
                    {
                        let app_report = self.app.frame(
                            viewport.logical_width as f32,
                            viewport.logical_height as f32,
                        );
                        self.outcome.window_report.frame_index = app_report.frame_index;
                        self.outcome.window_report.hit_region_count = app_report.hit_region_count;
                        self.update_snapshot(viewport, &app_report);
                    }
                    self.outcome.runtime_inspector_temporary = matches!(
                        self.app.latest_model().inspector.persistence,
                        editor_ui_model::InspectorPersistence::TemporaryPlaySession
                    );
                    let Some(renderer) = &mut self.renderer else {
                        self.fail(
                            "authority_capture.renderer_missing",
                            "The real WGPU renderer is unavailable.",
                            "editor_wgpu_renderer.capture",
                        );
                        event_loop.exit();
                        return;
                    };
                    #[cfg(feature = "real-wgpu-surface")]
                    if self.outcome.active_runtime_after_play {
                        present_active_game_view_to_shared_texture(
                            &mut self.app,
                            renderer,
                            &mut self.frame_publication,
                            &mut self.game_view_gpu_residency,
                            &mut self.outcome.window_report,
                        );
                        let app_report = self.app.frame(
                            viewport.logical_width as f32,
                            viewport.logical_height as f32,
                        );
                        self.outcome.window_report.frame_index = app_report.frame_index;
                        self.outcome.window_report.hit_region_count = app_report.hit_region_count;
                        self.outcome
                            .window_report
                            .apply_viewport_texture_registry_state(
                                renderer.viewport_textures().texture_count(),
                                renderer.viewport_textures().lifecycle_event_count(),
                            );
                        self.outcome.game_view_present_report =
                            self.app.session().last_game_view_present_report().cloned();
                        if self.qualify_project_lifecycle {
                            if let Some(frame) = self.app.session().last_game_view_runtime_frame() {
                                let shared_context = renderer.shared_context();
                                let receipt = self
                                    .frame_publication
                                    .last_good(&frame.session_id, &frame.target_id);
                                let readback = receipt.ok_or_else(|| {
                                    "The authority capture has no last-good publication receipt."
                                        .to_string()
                                }).and_then(|receipt| {
                                    renderer.viewport_textures().readback_gpu_exact(
                                        shared_context.device(),
                                        shared_context.queue(),
                                        receipt,
                                    )
                                });
                                match readback {
                                    Ok(capture) => self.outcome.game_view_capture = Some(capture),
                                    Err(error) => self
                                        .outcome
                                        .window_report
                                        .diagnostics
                                        .push(RealNativeEditorWindowDiagnostic {
                                        severity: RealNativeEditorWindowDiagnosticSeverity::Error,
                                        code:
                                            "project_editor_composition.game_view_readback_failed"
                                                .to_string(),
                                        message: error,
                                        source_stage: "editor_gameview_exact_texture_readback"
                                            .to_string(),
                                    }),
                                }
                            }
                        }
                    }
                    if self.production_scenario.is_some()
                        && self.stage == AuthorityCaptureStage::InitialFrame
                    {
                        let present_report = renderer.present(self.app.latest_draw_list());
                        self.outcome
                            .window_report
                            .apply_ui_present_report(&present_report);
                        self.outcome.present_report = Some(present_report);
                        let _ = renderer;
                        self.process_production_scenario_step(event_loop);
                        return;
                    }
                    if self.stage == AuthorityCaptureStage::InitialFrame
                        && self.click_widget_id.is_some()
                    {
                        let present_report = renderer.present(self.app.latest_draw_list());
                        self.outcome
                            .window_report
                            .apply_ui_present_report(&present_report);
                        self.outcome.present_report = Some(present_report);
                        match self.prepare_authority_click() {
                            Ok(()) => {
                                event_loop.set_control_flow(ControlFlow::WaitUntil(
                                    self.input_deadline.expect("authority input deadline"),
                                ));
                            }
                            Err(error) => {
                                self.fail(
                                    "authority_input.injection_failed",
                                    error,
                                    "windows.send_input",
                                );
                                event_loop.exit();
                            }
                        }
                        return;
                    }
                    let (present_report, capture) =
                        renderer.present_with_rgba_capture(self.app.latest_draw_list());
                    self.outcome
                        .window_report
                        .apply_ui_present_report(&present_report);
                    self.outcome.present_report = Some(present_report);
                    match capture {
                        Ok(capture) => self.outcome.capture = Some(capture),
                        Err(error) => {
                            self.outcome.capture_error = Some(error.clone());
                            self.fail(
                                "authority_capture.readback_failed",
                                error,
                                "editor_wgpu_renderer.capture",
                            );
                        }
                    }
                    if self.qualify_project_lifecycle && self.outcome.active_runtime_after_play {
                        self.outcome.project_lifecycle =
                            Some(crate::qualify_active_project_editor_composition_lifecycle(
                                self.app.session_mut(),
                            ));
                    }
                    event_loop.exit();
                }
                WindowEvent::CloseRequested => {
                    self.finish_production_scenario_for_close_requested();
                    event_loop.exit();
                }
                _ => {}
            }
        }

        fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
            self.ensure_production_scenario_terminal_report(
                "authority.event_loop_exited_without_terminal_report",
            );
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            let now = std::time::Instant::now();
            let game_view_deadline = if self.app.session().has_active_editor_runtime_play_instance()
            {
                let tick_count =
                    advance_game_view_tick_deadline(now, &mut self.next_game_view_tick);
                let mut advanced = false;
                for _ in 0..tick_count {
                    advanced |= self
                        .app
                        .tick_active_game_view_runtime_descriptor_frame()
                        .is_some();
                }
                if advanced {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                Some(self.next_game_view_tick)
            } else {
                reset_game_view_tick_deadline(now, &mut self.next_game_view_tick);
                None
            };
            if self.app.take_project_composition_progress_redraw_due(now) {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            match earliest_editor_deadline(
                game_view_deadline,
                self.app.project_composition_progress_deadline(),
            ) {
                Some(deadline) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
                None => event_loop.set_control_flow(ControlFlow::Wait),
            }
            if self.stage == AuthorityCaptureStage::AwaitingOsInput
                && self
                    .input_deadline
                    .is_some_and(|deadline| std::time::Instant::now() >= deadline)
            {
                if self.pending_drag_target.take().is_some() {
                    let _ = crate::finish_authority_primary_drag();
                }
                if let Some((client_x, client_y)) = self.pending_primary_release.take() {
                    if let Some(window) = &self.window {
                        let _ = crate::finish_authority_primary_click(window, client_x, client_y);
                    }
                }
                self.fail(
                    "authority_input.timeout",
                    "The target window did not observe the injected pointer down/up events.",
                    "windows.send_input",
                );
                if self.production_scenario.is_some() {
                    self.fail_production_scenario(event_loop, "authority_input.timeout");
                } else {
                    event_loop.exit();
                }
            }
        }
    }

    pub fn run_real_native_editor_capture_once(
        physical_width: u32,
        physical_height: u32,
        report_level: EditorReachabilityReportLevel,
    ) -> RealNativeEditorCaptureOutcome {
        run_real_native_editor_authority(RealNativeEditorAuthorityOptions {
            physical_width,
            physical_height,
            report_level,
            project_root: None,
            workspace_layout_store_root: None,
            click_widget_id: None,
            wheel_delta: None,
            drag_target_widget_id: None,
            drag_delta: None,
            scenario_path: None,
        })
    }

    fn scenario_actionability_pending(error: &str) -> bool {
        error.starts_with("authority_input.widget_missing:")
            || error.starts_with("authority_input.widget_not_actionable:")
            || error.starts_with("authority_input.widget_clipped:")
            || error == "authority.game_viewport_missing"
            || error.starts_with("authority.aui_target_not_actionable:")
            || (error.starts_with("authority.aui_target_not_unique:") && error.ends_with(":0"))
    }

    pub fn run_real_native_editor_authority(
        options: RealNativeEditorAuthorityOptions,
    ) -> RealNativeEditorCaptureOutcome {
        run_real_native_editor_authority_app(RealNativeEditorCaptureApp::new(options))
    }

    pub fn run_real_project_editor_composition_authority(
        options: RealProjectEditorCompositionAuthorityOptions,
    ) -> RealNativeEditorCaptureOutcome {
        run_real_native_editor_authority_app(
            RealNativeEditorCaptureApp::new_with_project_composition(
                options.authority,
                options.linked_project_runtimes,
                options.identity,
            ),
        )
    }

    fn run_real_native_editor_authority_app(
        mut app: RealNativeEditorCaptureApp,
    ) -> RealNativeEditorCaptureOutcome {
        let mut event_loop_builder = EventLoop::builder();
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            event_loop_builder.with_any_thread(true);
        }
        let event_loop = match event_loop_builder.build() {
            Ok(event_loop) => event_loop,
            Err(error) => {
                app.fail(
                    "authority_capture.event_loop_build_failed",
                    error.to_string(),
                    "winit.event_loop",
                );
                app.ensure_production_scenario_terminal_report(
                    "authority.runner_missing_terminal_report",
                );
                return app.outcome;
            }
        };
        if let Err(error) = event_loop.run_app(&mut app) {
            app.fail(
                "authority_capture.event_loop_failed",
                error.to_string(),
                "winit.event_loop",
            );
        }
        app.ensure_production_scenario_terminal_report("authority.runner_missing_terminal_report");
        app.outcome
    }

    #[cfg(test)]
    mod production_authority_wiring_tests {
        use super::*;

        #[test]
        fn game_view_tick_deadline_preserves_cadence_and_bounds_catch_up() {
            let base = std::time::Instant::now();
            let mut next_tick = base + GAME_VIEW_TICK_INTERVAL;

            assert_eq!(
                advance_game_view_tick_deadline(base + GAME_VIEW_TICK_INTERVAL, &mut next_tick),
                1
            );
            assert_eq!(next_tick, base + GAME_VIEW_TICK_INTERVAL * 2);

            assert_eq!(
                advance_game_view_tick_deadline(
                    base + std::time::Duration::from_millis(52),
                    &mut next_tick,
                ),
                2
            );
            assert_eq!(next_tick, base + GAME_VIEW_TICK_INTERVAL * 4);

            let stalled = base + std::time::Duration::from_millis(500);
            assert_eq!(
                advance_game_view_tick_deadline(stalled, &mut next_tick),
                GAME_VIEW_MAX_CATCH_UP_TICKS
            );
            assert_eq!(next_tick, stalled + GAME_VIEW_TICK_INTERVAL);
        }

        #[test]
        fn project_editor_composition_progress_deadline_uses_earliest_wait_until() {
            let base = std::time::Instant::now();
            let game = base + std::time::Duration::from_millis(16);
            let progress = base + std::time::Duration::from_millis(100);
            assert_eq!(
                earliest_editor_deadline(Some(game), Some(progress)),
                Some(game)
            );
            assert_eq!(
                earliest_editor_deadline(Some(progress), Some(game)),
                Some(game)
            );
            assert_eq!(
                earliest_editor_deadline(None, Some(progress)),
                Some(progress)
            );
            assert_eq!(earliest_editor_deadline(None, None), None);
        }

        #[test]
        fn inactive_game_view_resets_tick_debt() {
            let base = std::time::Instant::now();
            let mut next_tick = base;
            let resumed_at = base + std::time::Duration::from_secs(30);
            reset_game_view_tick_deadline(resumed_at, &mut next_tick);

            assert_eq!(next_tick, resumed_at + GAME_VIEW_TICK_INTERVAL);
            assert_eq!(
                advance_game_view_tick_deadline(resumed_at, &mut next_tick),
                0
            );
        }

        #[cfg(feature = "real-wgpu-surface")]
        fn gpu_identity(session_id: &str, source_hash: &str) -> GameViewGpuResourceIdentity {
            GameViewGpuResourceIdentity {
                session_id: session_id.to_string(),
                target_id: "portrait-main".to_string(),
                width: 1080,
                height: 1920,
                texture_format: "Bgra8UnormSrgb".to_string(),
                textures: vec![(
                    "texture-background".to_string(),
                    source_hash.to_string(),
                    1080,
                    1920,
                    "linearClamp".to_string(),
                )],
                font_bundles: vec![("default-ui".to_string(), 1)],
            }
        }

        #[cfg(feature = "real-wgpu-surface")]
        #[test]
        fn game_view_gpu_residency_uploads_once_until_resource_identity_changes() {
            let mut ledger = GameViewGpuResidencyLedger::default();
            let first = gpu_identity("session-a", "sha256:first");
            assert!(ledger.requires_upload(&first));
            ledger.commit_upload(first.clone(), 18);

            assert!(!ledger.requires_upload(&first));
            assert_eq!(ledger.upload_generation, 1);
            assert_eq!(ledger.total_texture_upload_count, 18);

            let changed_package = gpu_identity("session-a", "sha256:changed");
            assert!(ledger.requires_upload(&changed_package));
            ledger.commit_upload(changed_package, 18);
            assert_eq!(ledger.upload_generation, 2);
            assert_eq!(ledger.total_texture_upload_count, 36);
        }

        #[cfg(feature = "real-wgpu-surface")]
        #[test]
        fn game_view_gpu_residency_retires_on_stop_and_rebuilds_for_new_session() {
            let mut ledger = GameViewGpuResidencyLedger::default();
            let first = gpu_identity("session-a", "sha256:first");
            ledger.commit_upload(first, 18);

            assert_eq!(ledger.retire().as_deref(), Some("session-a"));
            let replacement = gpu_identity("session-b", "sha256:first");
            assert!(ledger.requires_upload(&replacement));
            ledger.commit_upload(replacement, 18);
            assert_eq!(ledger.upload_generation, 2);
        }

        fn production_authority_test_app(
            scenario_id: &str,
        ) -> (std::path::PathBuf, RealNativeEditorCaptureApp) {
            let root = std::env::temp_dir().join(format!(
                "aife-production-authority-terminal-wiring-{}-{scenario_id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let scenario_path = root.join("scenario.json");
            let scenario = crate::ProductionAuthorityScenario {
                schema_version: crate::PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION.to_string(),
                scenario_id: scenario_id.to_string(),
                evidence_root: root.join("evidence"),
                project_root: root.join("project"),
                recent_project_store_path: root.join("state/editor_recent_projects.json"),
                workspace_layout_store_root: root.join("workspace"),
                physical_width: 1280,
                physical_height: 720,
                game_view_target: None,
                per_step_timeout_ms: 1_000,
                overall_timeout_ms: 5_000,
                steps: vec![crate::ProductionAuthorityStep::WaitFor {
                    step_id: "launcher".to_string(),
                    timeout_ms: None,
                    condition: crate::ProductionAuthorityCondition::ActiveRuntime { active: false },
                }],
            };
            std::fs::write(&scenario_path, serde_json::to_vec(&scenario).unwrap()).unwrap();
            let app = RealNativeEditorCaptureApp::new_with_session(
                RealNativeEditorAuthorityOptions {
                    physical_width: 1280,
                    physical_height: 720,
                    report_level: EditorReachabilityReportLevel::Summary,
                    project_root: None,
                    workspace_layout_store_root: None,
                    click_widget_id: None,
                    wheel_delta: None,
                    drag_target_widget_id: None,
                    drag_delta: None,
                    scenario_path: Some(scenario_path),
                },
                EditorSession::with_linked_project_runtimes(
                    crate::default_editor_linked_project_runtimes(),
                ),
                false,
            );
            (root, app)
        }

        #[test]
        fn production_authority_scenario_close_requested_routes_terminal_report() {
            let (root, mut app) = production_authority_test_app("close-requested-wiring");
            app.finish_production_scenario_for_close_requested();
            let report = app
                .outcome
                .production_authority_report
                .as_ref()
                .expect("CloseRequested adapter must finalize the active scenario");
            assert_eq!(report.status, "failed");
            assert_eq!(
                report.diagnostics,
                vec!["authority.window_close_requested".to_string()]
            );
            app.ensure_production_scenario_terminal_report(
                "authority.event_loop_exited_without_terminal_report",
            );
            assert_eq!(
                app.outcome
                    .production_authority_report
                    .as_ref()
                    .expect("terminal report")
                    .diagnostics,
                vec!["authority.window_close_requested".to_string()]
            );
            std::fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn production_authority_scenario_close_is_noop_when_scenario_disabled() {
            let mut app = RealNativeEditorCaptureApp::new_with_session(
                RealNativeEditorAuthorityOptions {
                    physical_width: 1280,
                    physical_height: 720,
                    report_level: EditorReachabilityReportLevel::Summary,
                    project_root: None,
                    workspace_layout_store_root: None,
                    click_widget_id: None,
                    wheel_delta: None,
                    drag_target_widget_id: None,
                    drag_delta: None,
                    scenario_path: None,
                },
                EditorSession::with_linked_project_runtimes(
                    crate::default_editor_linked_project_runtimes(),
                ),
                false,
            );
            app.finish_production_scenario_for_close_requested();
            app.ensure_production_scenario_terminal_report(
                "authority.event_loop_exited_without_terminal_report",
            );
            assert!(app.outcome.production_authority_report.is_none());
        }

        #[test]
        fn production_authority_scenario_loads_declared_recent_project_store() {
            let root = std::env::temp_dir().join(format!(
                "aife-production-authority-recent-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let recent_store = root.join("state/editor_recent_projects.json");
            let scenario_path = root.join("scenario.json");
            let scenario = crate::ProductionAuthorityScenario {
                schema_version: crate::PRODUCTION_AUTHORITY_SCENARIO_SCHEMA_VERSION.to_string(),
                scenario_id: "recent-store-wiring".to_string(),
                evidence_root: root.join("evidence"),
                project_root: root.join("project"),
                recent_project_store_path: recent_store.clone(),
                workspace_layout_store_root: root.join("workspace"),
                physical_width: 1280,
                physical_height: 720,
                game_view_target: Some(
                    engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_1080x1920(
                    ),
                ),
                per_step_timeout_ms: 1_000,
                overall_timeout_ms: 5_000,
                steps: vec![crate::ProductionAuthorityStep::WaitFor {
                    step_id: "launcher".to_string(),
                    timeout_ms: None,
                    condition: crate::ProductionAuthorityCondition::ActiveRuntime { active: false },
                }],
            };
            std::fs::write(&scenario_path, serde_json::to_vec(&scenario).unwrap()).unwrap();

            let app = RealNativeEditorCaptureApp::new_with_session(
                RealNativeEditorAuthorityOptions {
                    physical_width: 1280,
                    physical_height: 720,
                    report_level: EditorReachabilityReportLevel::Summary,
                    project_root: None,
                    workspace_layout_store_root: None,
                    click_widget_id: None,
                    wheel_delta: None,
                    drag_target_widget_id: None,
                    drag_delta: None,
                    scenario_path: Some(scenario_path),
                },
                EditorSession::with_linked_project_runtimes(
                    crate::default_editor_linked_project_runtimes(),
                ),
                false,
            );

            assert_eq!(
                app.app.project_manager().recent_store_path.as_deref(),
                Some(recent_store.as_path())
            );
            assert_eq!(
                app.app.session().game_view_target(),
                engine_runtime::game_view_presentation::GameViewTargetSpec::portrait_1080x1920()
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
