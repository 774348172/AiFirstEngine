use editor_core::{command_for_test, CommandStatus, EditorSession};
use editor_ui_backend_egui::summarize_model_for_egui_backend;
use editor_ui_model::UiCommandPayload;
use editor_ui_renderer::{SelfUiRenderer, UiRendererConfig};
use editor_window_winit::{
    default_editor_linked_project_runtimes, run_real_native_editor_window_with_model_and_options,
    validate_window_skeleton, NativeEditorApplication, NativeEditorWindowConfig,
    RealNativeEditorLaunchOptions,
};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match try_run_editor_host_with_args(&args) {
        Ok(output) => println!("{output}"),
        Err(diagnostic) => {
            eprintln!("editor_host error: {diagnostic}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
fn run_editor_host_with_args(args: &[String]) -> String {
    try_run_editor_host_with_args(args)
        .unwrap_or_else(|diagnostic| format!("editor_host error: {diagnostic}"))
}

fn try_run_editor_host_with_args(args: &[String]) -> Result<String, String> {
    let launch_request = parse_editor_host_args(args)?;
    if let Some(preflight) = launch_request.gateway_process_preflight {
        return run_gateway_process_preflight(preflight);
    }
    let real_window_options = if launch_request.real_window {
        Some(launch_request.real_window_options()?)
    } else {
        None
    };
    let session = EditorSession::new();
    let model = session.build_ui_model();
    if let Some(options) = real_window_options {
        let report = run_real_native_editor_window_with_model_and_options(model, options);
        return Ok(format!(
            "editor_host real-window: present_status={}, window_created={}, diagnostics={}",
            report.present_status,
            report.window_created,
            report
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }

    let summary = summarize_model_for_egui_backend(&model);
    let draw_list = SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
    let readiness = validate_window_skeleton(&NativeEditorWindowConfig::default());
    Ok(format!(
        "editor_host ready: panels={}, commands={}, renderables={}, draw_commands={}, hit_regions={}, native_window_ready={}",
        summary.panel_count,
        summary.command_count,
        summary.renderable_count,
        draw_list.commands.len(),
        draw_list.hit_regions.len(),
        readiness.window_attributes_ready && readiness.wgpu_instance_ready
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorHostLaunchRequest {
    real_window: bool,
    isolated_project_launch_root: Option<PathBuf>,
    gateway_process_preflight: Option<GatewayProcessPreflight>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GatewayProcessPreflight {
    project_root: PathBuf,
    discovery_root: PathBuf,
    timeout_ms: u64,
}

impl EditorHostLaunchRequest {
    fn real_window_options(&self) -> Result<RealNativeEditorLaunchOptions, String> {
        match &self.isolated_project_launch_root {
            Some(root) => RealNativeEditorLaunchOptions::isolated_project_launch_root(root),
            None => Ok(RealNativeEditorLaunchOptions::default()),
        }
    }
}

fn parse_editor_host_args(args: &[String]) -> Result<EditorHostLaunchRequest, String> {
    let mut real_window = false;
    let mut isolated_project_launch_root = None;
    let mut gateway_process_preflight = false;
    let mut project_root = None;
    let mut gateway_discovery_root = None;
    let mut gateway_preflight_timeout_ms = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--real-window" => {
                if real_window {
                    return Err("editor_host.real_window_duplicate".to_string());
                }
                real_window = true;
                index += 1;
            }
            "--isolated-project-launch-root" => {
                if isolated_project_launch_root.is_some() {
                    return Err("editor_host.isolated_project_launch_root_duplicate".to_string());
                }
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.is_empty() && !value.starts_with("--"))
                    .ok_or_else(|| {
                        "editor_host.isolated_project_launch_root_missing".to_string()
                    })?;
                isolated_project_launch_root = Some(PathBuf::from(value));
                index += 2;
            }
            "--gateway-process-preflight" => {
                if gateway_process_preflight {
                    return Err("editor_host.gateway_process_preflight_duplicate".to_string());
                }
                gateway_process_preflight = true;
                index += 1;
            }
            "--project-root" => {
                project_root = Some(parse_path_arg(
                    args,
                    index,
                    project_root.is_some(),
                    "project_root",
                )?);
                index += 2;
            }
            "--gateway-discovery-root" => {
                gateway_discovery_root = Some(parse_path_arg(
                    args,
                    index,
                    gateway_discovery_root.is_some(),
                    "gateway_discovery_root",
                )?);
                index += 2;
            }
            "--gateway-preflight-timeout-ms" => {
                if gateway_preflight_timeout_ms.is_some() {
                    return Err("editor_host.gateway_preflight_timeout_ms_duplicate".to_string());
                }
                let raw = args
                    .get(index + 1)
                    .filter(|value| !value.is_empty() && !value.starts_with("--"))
                    .ok_or_else(|| {
                        "editor_host.gateway_preflight_timeout_ms_missing".to_string()
                    })?;
                let timeout_ms = raw
                    .parse::<u64>()
                    .map_err(|_| "editor_host.gateway_preflight_timeout_ms_invalid".to_string())?;
                if !(1_000..=60_000).contains(&timeout_ms) {
                    return Err("editor_host.gateway_preflight_timeout_ms_out_of_range".to_string());
                }
                gateway_preflight_timeout_ms = Some(timeout_ms);
                index += 2;
            }
            argument => {
                return Err(format!("editor_host.unknown_argument: {argument}"));
            }
        }
    }
    if isolated_project_launch_root.is_some() && !real_window {
        return Err("editor_host.isolated_project_launch_root_requires_real_window".to_string());
    }
    if gateway_process_preflight && real_window {
        return Err("editor_host.gateway_process_preflight_conflicts_real_window".to_string());
    }
    let gateway_process_preflight = if gateway_process_preflight {
        Some(GatewayProcessPreflight {
            project_root: project_root
                .ok_or_else(|| "editor_host.gateway_preflight_project_root_missing".to_string())?,
            discovery_root: gateway_discovery_root.ok_or_else(|| {
                "editor_host.gateway_preflight_discovery_root_missing".to_string()
            })?,
            timeout_ms: gateway_preflight_timeout_ms.unwrap_or(10_000),
        })
    } else {
        if project_root.is_some()
            || gateway_discovery_root.is_some()
            || gateway_preflight_timeout_ms.is_some()
        {
            return Err("editor_host.gateway_preflight_option_requires_mode".to_string());
        }
        None
    };
    Ok(EditorHostLaunchRequest {
        real_window,
        isolated_project_launch_root,
        gateway_process_preflight,
    })
}

fn parse_path_arg(
    args: &[String],
    index: usize,
    duplicate: bool,
    name: &str,
) -> Result<PathBuf, String> {
    if duplicate {
        return Err(format!("editor_host.{name}_duplicate"));
    }
    args.get(index + 1)
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .map(PathBuf::from)
        .ok_or_else(|| format!("editor_host.{name}_missing"))
}

fn production_editor_session() -> EditorSession {
    EditorSession::with_linked_project_runtimes(default_editor_linked_project_runtimes())
}

fn run_gateway_process_preflight(options: GatewayProcessPreflight) -> Result<String, String> {
    let project_root = options
        .project_root
        .canonicalize()
        .map_err(|error| format!("editor_host.gateway_preflight_project_root_invalid: {error}"))?;
    let discovery_root = if options.discovery_root.is_absolute() {
        options.discovery_root
    } else {
        return Err("editor_host.gateway_preflight_discovery_root_not_absolute".to_string());
    };
    std::fs::create_dir_all(&discovery_root).map_err(|error| {
        format!("editor_host.gateway_preflight_discovery_root_create_failed: {error}")
    })?;

    let mut session = production_editor_session();
    let opened = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
        path: project_root.display().to_string(),
    }));
    if opened.status != CommandStatus::Committed {
        return Err(format!(
            "editor_host.gateway_preflight_project_open_failed: {:?}",
            opened.status
        ));
    }
    let mut app = NativeEditorApplication::with_session_and_gateway_discovery_root(
        NativeEditorWindowConfig::default(),
        session,
        discovery_root,
    );
    app.frame(1280.0, 720.0);
    if let Some(error) = app.gateway_host_error() {
        return Err(format!("{}: {}", error.code, error.message));
    }
    let discovery_path = app
        .gateway_discovery_path()
        .ok_or_else(|| "editor_host.gateway_preflight_discovery_missing".to_string())?
        .to_path_buf();
    println!(
        "editor_host gateway-preflight-ready: discovery={}",
        discovery_path.display()
    );
    std::io::stdout()
        .flush()
        .map_err(|error| format!("editor_host.gateway_preflight_stdout_flush_failed: {error}"))?;

    let deadline = Instant::now() + Duration::from_millis(options.timeout_ms);
    let mut saw_connected_client = false;
    loop {
        app.frame(1280.0, 720.0);
        if let Some(error) = app.gateway_host_error() {
            return Err(format!("{}: {}", error.code, error.message));
        }
        let active_clients = app.gateway_active_client_count();
        saw_connected_client |= active_clients > 0;
        if saw_connected_client && active_clients == 0 {
            drop(app);
            if discovery_path.exists() {
                return Err("editor_host.gateway_preflight_discovery_cleanup_failed".to_string());
            }
            return Ok("editor_host gateway-process-preflight: passed".to_string());
        }
        if Instant::now() >= deadline {
            return Err("editor_host.gateway_preflight_timed_out".to_string());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_core::{
        command_for_test, AiCapabilityGrant, AiCapabilityToolKernel, AiToolExecutionStatus,
        AiToolInvocation, AiToolInvocationPayload, AiToolOperationSnapshot, AiToolOperationState,
        AiToolStartOutcome, CommandStatus, ProjectCandidateEntry, ProjectPreviewFrameTicket,
        AI_TOOL_INVOCATION_SCHEMA_VERSION, TOOL_ID_PROJECT_PREVIEW,
    };
    use editor_input::{EditorInputEvent, EditorInputRouter, PointerButton};
    use editor_ui_model::{EditorUiMode, UiCommandPayload, Vec3};
    use editor_ui_renderer::HitTarget;
    use editor_window_winit::{
        build_real_window_gate_report, route_input_for_cmin, HeadlessRuntimeRenderer,
        HeadlessSurfaceBackend, HeadlessWindowBackend, ViewportHost,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn editor_host_can_build_initial_model() {
        let session = EditorSession::new();
        let model = session.build_ui_model();
        let summary = summarize_model_for_egui_backend(&model);
        assert_eq!(model.mode, EditorUiMode::ProjectLauncher);
        assert_eq!(summary.panel_count, 7);
        assert_eq!(summary.renderable_count, 0);
    }

    #[test]
    fn editor_host_builds_self_renderer_draw_list() {
        let session = EditorSession::new();
        let model = session.build_ui_model();
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
        assert!(draw_list
            .hit_regions
            .iter()
            .any(|region| region.id == "hit.project_launcher.open_project"));
        assert!(!draw_list
            .hit_regions
            .iter()
            .any(|region| region.target == HitTarget::Viewport));
    }

    #[test]
    fn editor_host_routes_project_launcher_input_to_open_project_command() {
        let session = EditorSession::new();
        let model = session.build_ui_model();
        let draw_list =
            SelfUiRenderer::build_draw_list(&model, UiRendererConfig::new(1280.0, 720.0));
        let region = draw_list
            .hit_regions
            .iter()
            .find(|region| region.id == "hit.project_launcher.open_project")
            .expect("open project region");
        assert!(region.enabled);

        let mut router = EditorInputRouter::new();
        let result = router.route(
            EditorInputEvent::PointerDown {
                x: region.rect.x + 1.0,
                y: region.rect.y + 1.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );
        assert_eq!(
            result.command.expect("open project command").payload,
            UiCommandPayload::OpenProject {
                path: String::new()
            }
        );
    }

    #[test]
    fn editor_host_has_native_window_skeleton() {
        let readiness = validate_window_skeleton(&NativeEditorWindowConfig::default());
        assert!(readiness.window_attributes_ready);
        assert!(readiness.wgpu_instance_ready);
    }

    #[test]
    fn editor_host_default_still_prints_readiness() {
        let output = run_editor_host_with_args(&[]);

        assert!(output.contains("editor_host ready:"));
        assert!(output.contains("native_window_ready=true"));
    }

    #[test]
    fn production_runtime_composition_previews_complex_shooter_with_linked_runtime() {
        let project_root = copy_complex_shooter_fixture("production-complex-shooter");
        let (operation, ticket, linked_module_id) =
            production_preview(&project_root, "production-complex-shooter-preview");

        assert_eq!(
            operation.state,
            AiToolOperationState::Running,
            "operation={operation:#?}\nticket={ticket:#?}"
        );
        assert_eq!(operation.stage, "awaiting_frame_evidence");
        assert!(operation.result.is_none());
        let ticket = ticket.expect("production Preview must retain a frame ticket");
        assert_eq!(ticket.operation_id, operation.operation_id);
        assert_eq!(ticket.project_identity, "project-complex-shooter-sample");
        assert_eq!(
            linked_module_id.as_deref(),
            Some("sample.complex-shooter.runtime")
        );

        fs::remove_dir_all(project_root).expect("remove complex-shooter fixture");
    }

    #[test]
    fn production_runtime_composition_rejects_unlinked_project_c01_runtime() {
        let project_root = copy_complex_shooter_fixture("production-c01-unlinked");
        let manifest_path = project_root.join("project.aife.json");
        let mut manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(&manifest_path).expect("read copied project manifest"),
        )
        .expect("parse copied project manifest");
        manifest["runtimeModule"]["moduleId"] =
            serde_json::Value::String("project.c01.runtime".to_string());
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("encode C-01 project manifest"),
        )
        .expect("write C-01 project manifest");

        let (operation, ticket, linked_module_id) =
            production_preview(&project_root, "production-c01-preview");

        let result = operation
            .result
            .expect("unlinked production Preview must fail before frame capture");
        assert_eq!(result.status, AiToolExecutionStatus::Failed);
        assert_eq!(
            result
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.code.as_str()),
            Some("ai_tool.preview_project_runtime_not_linked")
        );
        assert!(result
            .diagnostics
            .first()
            .is_some_and(|diagnostic| diagnostic.message.contains("project.c01.runtime")));
        assert!(ticket.is_none());
        assert!(linked_module_id.is_none());

        fs::remove_dir_all(project_root).expect("remove C-01 fixture");
    }

    fn production_preview(
        project_root: &Path,
        invocation_id: &str,
    ) -> (
        AiToolOperationSnapshot,
        Option<ProjectPreviewFrameTicket>,
        Option<String>,
    ) {
        let mut session = production_editor_session();
        let opened = session.execute_command(command_for_test(UiCommandPayload::OpenProject {
            path: project_root.display().to_string(),
        }));
        assert_eq!(opened.status, CommandStatus::Committed);
        let binding = ProjectCandidateEntry::inspect_project_binding(&session)
            .expect("inspect production project binding");
        let grant = AiCapabilityGrant::read(
            format!("grant-{invocation_id}"),
            binding.project_id,
            binding.project_digest.clone(),
            "production-composition-test",
        )
        .expect("create production Preview read grant");
        let mut kernel = AiCapabilityToolKernel::new();
        let started = kernel.start(
            &session,
            AiToolInvocation {
                schema_version: AI_TOOL_INVOCATION_SCHEMA_VERSION.to_string(),
                invocation_id: invocation_id.to_string(),
                tool_id: TOOL_ID_PROJECT_PREVIEW.to_string(),
                expected_project_digest: binding.project_digest,
                payload: AiToolInvocationPayload::Preview,
            },
            &grant,
        );
        let operation_id = match started {
            AiToolStartOutcome::Accepted(accepted) => accepted.operation_id,
            AiToolStartOutcome::Terminal(result) => {
                panic!("production Preview was rejected before operation start: {result:?}")
            }
        };
        kernel.pump_operations(&mut session, 3);
        let operation = kernel
            .observe(&operation_id)
            .expect("production Preview operation must remain observable");
        let ticket = session.pending_project_preview_frame_ticket().cloned();
        let linked_module_id = session
            .last_game_view_present_report()
            .and_then(|report| report.project_runtime_bind_receipt.as_ref())
            .map(|receipt| receipt.module_id.clone());
        (operation, ticket, linked_module_id)
    }

    fn copy_complex_shooter_fixture(label: &str) -> PathBuf {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/complex_shooter_project");
        let destination = unique_editor_host_temp_root(label);
        copy_project_tree(&source, &destination);
        destination
    }

    fn copy_project_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).expect("create copied project directory");
        for entry in fs::read_dir(source)
            .expect("read project fixture directory")
            .flatten()
        {
            let name = entry.file_name();
            if entry.path().is_dir()
                && matches!(
                    name.to_string_lossy().as_ref(),
                    "Build" | "Library" | ".aife" | "target"
                )
            {
                continue;
            }
            let destination_path = destination.join(&name);
            if entry.path().is_dir() {
                copy_project_tree(&entry.path(), &destination_path);
            } else {
                fs::copy(entry.path(), destination_path).expect("copy project fixture file");
            }
        }
    }

    #[cfg(not(feature = "real-window"))]
    #[test]
    fn editor_host_real_window_arg_reports_missing_feature_without_feature() {
        let output = run_editor_host_with_args(&["--real-window".to_string()]);

        assert!(output.contains("real_window_feature_not_enabled"));
    }

    #[test]
    fn isolated_project_launch_root_parses_as_atomic_profile() {
        let root = unique_editor_host_temp_root("isolated-profile");
        let picker_start = root.join("picker-start");
        fs::create_dir_all(&picker_start).expect("create picker start");
        let request = parse_editor_host_args(&[
            "--real-window".to_string(),
            "--isolated-project-launch-root".to_string(),
            root.display().to_string(),
        ])
        .expect("parse isolated profile");

        let options = request
            .real_window_options()
            .expect("build isolated options");

        assert_eq!(request.isolated_project_launch_root, Some(root.clone()));
        assert_eq!(options.project_dialog_initial_directory(), picker_start);
        let expected_recent_store = root.join("state").join("editor_recent_projects.json");
        assert_eq!(
            options.recent_store_path(),
            Some(expected_recent_store.as_path())
        );
        fs::remove_dir_all(root).expect("remove isolated profile fixture");
    }

    #[test]
    fn isolated_project_launch_root_rejects_missing_relative_or_non_directory_root() {
        assert_eq!(
            parse_editor_host_args(&[
                "--real-window".to_string(),
                "--isolated-project-launch-root".to_string(),
            ]),
            Err("editor_host.isolated_project_launch_root_missing".to_string())
        );

        let relative = parse_editor_host_args(&[
            "--real-window".to_string(),
            "--isolated-project-launch-root".to_string(),
            "relative-run-root".to_string(),
        ])
        .expect("relative path parses before filesystem validation");
        assert!(relative
            .real_window_options()
            .expect_err("relative root must fail")
            .starts_with("editor_host.isolated_project_launch_root_invalid"));

        let root_file = unique_editor_host_temp_root("root-file");
        if let Some(parent) = root_file.parent() {
            fs::create_dir_all(parent).expect("create temp parent");
        }
        fs::write(&root_file, "not a directory").expect("create root file");
        assert!(
            RealNativeEditorLaunchOptions::isolated_project_launch_root(&root_file)
                .expect_err("file root must fail")
                .starts_with("editor_host.isolated_project_launch_root_not_directory")
        );
        fs::remove_file(root_file).expect("remove root file fixture");

        let root_without_picker = unique_editor_host_temp_root("missing-picker");
        fs::create_dir_all(&root_without_picker).expect("create isolated root");
        assert!(
            RealNativeEditorLaunchOptions::isolated_project_launch_root(&root_without_picker)
                .expect_err("missing picker start must fail")
                .starts_with("editor_host.isolated_picker_start_invalid")
        );
        fs::remove_dir_all(root_without_picker).expect("remove missing picker fixture");

        let root_with_non_empty_picker = unique_editor_host_temp_root("non-empty-picker");
        let non_empty_picker = root_with_non_empty_picker.join("picker-start");
        fs::create_dir_all(&non_empty_picker).expect("create non-empty picker");
        fs::write(non_empty_picker.join("history.txt"), "history").expect("seed non-empty picker");
        assert!(RealNativeEditorLaunchOptions::isolated_project_launch_root(
            &root_with_non_empty_picker
        )
        .expect_err("non-empty picker must fail")
        .starts_with("editor_host.isolated_picker_start_not_empty"));
        fs::remove_dir_all(root_with_non_empty_picker).expect("remove non-empty picker fixture");

        let root_with_stale_state = unique_editor_host_temp_root("stale-state");
        fs::create_dir_all(root_with_stale_state.join("picker-start"))
            .expect("create picker start");
        let stale_store = root_with_stale_state
            .join("state")
            .join("editor_recent_projects.json");
        fs::create_dir_all(stale_store.parent().expect("state parent"))
            .expect("create stale state");
        fs::write(
            &stale_store,
            r#"{"schemaVersion":"editor-recent-projects.v1","recentProjects":[{"path":"I:/historical-project"}]}"#,
        )
        .expect("seed stale recent store");
        assert!(RealNativeEditorLaunchOptions::isolated_project_launch_root(
            &root_with_stale_state
        )
        .expect_err("preexisting recent state must fail before application startup")
        .starts_with("editor_host.isolated_recent_state_not_fresh"));
        fs::remove_dir_all(root_with_stale_state).expect("remove stale state fixture");
    }

    #[test]
    fn isolated_project_launch_root_rejects_duplicate_flag() {
        let error = parse_editor_host_args(&[
            "--real-window".to_string(),
            "--isolated-project-launch-root".to_string(),
            "C:\\run-a".to_string(),
            "--isolated-project-launch-root".to_string(),
            "C:\\run-b".to_string(),
        ])
        .expect_err("duplicate isolated root must fail");

        assert_eq!(error, "editor_host.isolated_project_launch_root_duplicate");
    }

    #[test]
    fn isolated_project_launch_cli_rejects_ambiguous_or_conflicting_flags() {
        assert_eq!(
            parse_editor_host_args(&[
                "--isolated-project-launch-root".to_string(),
                "C:\\run".to_string(),
            ]),
            Err("editor_host.isolated_project_launch_root_requires_real_window".to_string())
        );
        assert_eq!(
            parse_editor_host_args(&[
                "--isolated-project-launch-root".to_string(),
                "--real-window".to_string(),
            ]),
            Err("editor_host.isolated_project_launch_root_missing".to_string())
        );
        assert_eq!(
            parse_editor_host_args(&["--unknown".to_string()]),
            Err("editor_host.unknown_argument: --unknown".to_string())
        );
        assert_eq!(
            parse_editor_host_args(&["--real-window".to_string(), "--real-window".to_string(),]),
            Err("editor_host.real_window_duplicate".to_string())
        );
    }

    fn unique_editor_host_temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "editor-host-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn editor_host_runs_headless_real_window_viewport_surface_cmin_scenario() {
        let config = NativeEditorWindowConfig::default();
        let mut window = HeadlessWindowBackend::create_window(&config);
        let mut surface = HeadlessSurfaceBackend::create_surface();
        surface.configure(config.width, config.height, "Bgra8UnormSrgb", "Fifo");

        let session = EditorSession::new();
        let model = session.build_ui_model();
        let draw_list = SelfUiRenderer::build_draw_list(
            &model,
            UiRendererConfig::new(config.width as f32, config.height as f32),
        );
        let viewport_region = editor_ui_renderer::UiRect {
            x: 240.0,
            y: 80.0,
            width: 640.0,
            height: 360.0,
        };

        let mut viewport_host = ViewportHost::new();
        viewport_host
            .register_scene_viewport("scene-view", viewport_region)
            .expect("scene viewport should register");
        viewport_host
            .focus_scene(true)
            .expect("scene viewport should focus");

        let mut router = EditorInputRouter::new();
        let input_route = route_input_for_cmin(
            &mut router,
            &mut viewport_host,
            EditorInputEvent::PointerDown {
                x: viewport_region.x + 4.0,
                y: viewport_region.y + 4.0,
                button: PointerButton::Primary,
            },
            &draw_list,
        );

        surface.acquire();
        let runtime_frame = HeadlessRuntimeRenderer::render(
            1,
            viewport_host
                .scene_viewport()
                .expect("viewport should exist"),
        );
        surface.present();
        window.request_redraw();

        let report = build_real_window_gate_report(
            window.snapshot(),
            surface.snapshot(),
            viewport_host.scene_viewport().cloned(),
            viewport_host.latest_runtime_frame().cloned(),
            &draw_list,
            Some(runtime_frame.clone()),
            vec![input_route],
        );

        assert!(report.window.created);
        assert_eq!(report.surface.presented_frame, 1);
        assert!(report.viewport.as_ref().expect("viewport report").focused);
        assert_eq!(
            report
                .runtime_frame
                .as_ref()
                .expect("runtime frame")
                .frame_hash,
            runtime_frame.frame_hash
        );
        assert!(report.diagnostics.is_empty());
        assert!(serde_json::to_string(&report)
            .expect("report should serialize")
            .contains("real-window-gate-report.v1"));
    }

    #[test]
    fn editor_host_routes_open_scene_document_to_editor_core() {
        let scene_path = write_editor_scene_fixture();
        let mut session = EditorSession::new();

        let result =
            session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
                path: scene_path.display().to_string(),
            }));

        assert_eq!(result.status, CommandStatus::Committed);
        assert_eq!(
            session.build_ui_model().hierarchy.scene_id.as_deref(),
            Some("scene-main")
        );
    }

    #[test]
    fn editor_host_routes_hierarchy_select_to_scene_editing() {
        let scene_path = write_editor_scene_fixture();
        let mut session = opened_scene_session(&scene_path);
        let mut router = EditorInputRouter::new();

        let command = router.scene_hierarchy_select_command("entity-player");
        let result = session.execute_command(command);

        assert_eq!(result.status, CommandStatus::Committed);
        assert_eq!(
            session
                .build_ui_model()
                .inspector
                .selected_entity_id
                .as_deref(),
            Some("entity-player")
        );
    }

    #[test]
    fn editor_host_routes_inspector_transform_to_scene_editing() {
        let scene_path = write_editor_scene_fixture();
        let mut session = opened_scene_session(&scene_path);
        let mut router = EditorInputRouter::new();

        let command = router.inspector_set_scene_transform_command(
            "entity-player",
            Some(Vec3 {
                x: 11.0,
                y: 0.0,
                z: 0.0,
            }),
            None,
            None,
        );
        let result = session.execute_command(command);

        assert_eq!(result.status, CommandStatus::Committed);
        let model = session.build_ui_model();
        let player = model
            .viewport
            .renderables
            .iter()
            .find(|renderable| renderable.entity_id == "entity-player")
            .expect("player renderable");
        assert_eq!(player.local_position.x, 11.0);
    }

    #[test]
    fn editor_host_routes_save_scene_to_scene_save_pipeline() {
        let scene_path = write_editor_scene_fixture();
        let mut session = opened_scene_session(&scene_path);
        let mut router = EditorInputRouter::new();
        let edit = router.inspector_set_scene_transform_command(
            "entity-player",
            Some(Vec3 {
                x: 12.0,
                y: 0.0,
                z: 0.0,
            }),
            None,
            None,
        );
        assert_eq!(
            session.execute_command(edit).status,
            CommandStatus::Committed
        );

        let save = router
            .scene_toolbar_command("save_scene")
            .expect("save scene command");
        let result = session.execute_command(save);

        assert_eq!(result.status, CommandStatus::Committed);
        assert!(result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "scene.save.path_required"));
    }

    #[test]
    fn editor_host_editable_project_loop_report_is_headless_readable() {
        let report = editor_core::run_editable_project_loop_headless();

        assert!(report.opened_scene);
        assert!(report.transform_edit_applied);
        assert!(report.play_finished);
        assert!(serde_json::to_string(&report)
            .expect("editable project loop report should serialize")
            .contains("editable-project-loop-report.v1"));
    }

    fn opened_scene_session(scene_path: &PathBuf) -> EditorSession {
        let mut session = EditorSession::new();
        let result =
            session.execute_command(command_for_test(UiCommandPayload::OpenSceneDocument {
                path: scene_path.display().to_string(),
            }));
        assert_eq!(result.status, CommandStatus::Committed);
        session
    }

    fn write_editor_scene_fixture() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("editor-host-scene-{}", stamp));
        fs::create_dir_all(root.join("scenes")).unwrap();
        let scene_path = root.join("scenes").join("main.scene.json");
        fs::write(
            &scene_path,
            r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
    "id": "entity-player",
    "name": "Player",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },
    "mesh": {
      "primitive": "model",
      "assetRef": { "id": "model-player", "type": "model" },
      "visible": true,
      "layer": "default"
    }
  }]
}"##,
        )
        .unwrap();
        scene_path
    }
}
