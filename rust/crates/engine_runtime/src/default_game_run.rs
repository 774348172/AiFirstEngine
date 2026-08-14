use crate::diagnostics::{DiagnosticSeverity, RuntimeDiagnostic};
use crate::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use crate::runtime_package::load_runtime_package;
use crate::runtime_scene_hydration::hydrate_active_scene_into_world;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const END_TO_END_GAME_RUN_REPORT_SCHEMA_VERSION: &str = "end-to-end-game-run-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultGameRunMode {
    Headless,
    Windowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultGameRunRequest {
    pub project_path: PathBuf,
    pub runtime_package_path: PathBuf,
    pub mode: DefaultGameRunMode,
    pub scenario_id: String,
    pub frame_limit: u64,
    pub report_path: Option<PathBuf>,
    pub launch_runtime_process: bool,
}

impl DefaultGameRunRequest {
    pub fn headless_for_tests(runtime_package_path: impl Into<PathBuf>) -> Self {
        let runtime_package_path = runtime_package_path.into();
        Self {
            project_path: runtime_package_path.clone(),
            runtime_package_path,
            mode: DefaultGameRunMode::Headless,
            scenario_id: "minimal_game_loop".to_string(),
            frame_limit: 3,
            report_path: None,
            launch_runtime_process: false,
        }
    }

    pub fn windowed_for_user_run(runtime_package_path: impl Into<PathBuf>) -> Self {
        let runtime_package_path = runtime_package_path.into();
        Self {
            project_path: runtime_package_path.clone(),
            runtime_package_path,
            mode: DefaultGameRunMode::Windowed,
            scenario_id: "minimal_game_loop".to_string(),
            frame_limit: 3,
            report_path: None,
            launch_runtime_process: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndToEndGameRunDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndToEndGameRunDiagnostic {
    pub severity: EndToEndGameRunDiagnosticSeverity,
    pub code: String,
    pub layer: String,
    pub message: String,
    pub path: Option<String>,
}

impl EndToEndGameRunDiagnostic {
    pub fn error(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: EndToEndGameRunDiagnosticSeverity::Error,
            code: code.into(),
            layer: layer.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndToEndGameRunReport {
    pub schema_version: String,
    pub run_id: String,
    pub mode: DefaultGameRunMode,
    pub project_path: String,
    pub staged_run_folder: String,
    pub runtime_package_path: String,
    pub scenario_id: String,
    pub frame_limit: u64,
    pub build_status: String,
    pub runtime_spawn_status: String,
    pub package_load_status: String,
    pub asset_load_status: String,
    pub scene_load_status: String,
    pub logic_tick_status: String,
    pub render_extract_status: String,
    pub render_thread_status: String,
    pub rdg_status: String,
    pub rhi_status: String,
    pub surface_status: String,
    pub present_status: String,
    pub frames_requested: u64,
    pub frames_completed: u64,
    pub first_presented_frame: Option<u64>,
    pub exit_code: Option<i32>,
    pub diagnostics: Vec<EndToEndGameRunDiagnostic>,
}

impl EndToEndGameRunReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == EndToEndGameRunDiagnosticSeverity::Error)
    }

    fn base(request: &DefaultGameRunRequest) -> Self {
        Self {
            schema_version: END_TO_END_GAME_RUN_REPORT_SCHEMA_VERSION.to_string(),
            run_id: format!("default-game-run-{}", request.scenario_id),
            mode: request.mode,
            project_path: request.project_path.display().to_string(),
            staged_run_folder: request
                .runtime_package_path
                .parent()
                .unwrap_or(&request.runtime_package_path)
                .display()
                .to_string(),
            runtime_package_path: request.runtime_package_path.display().to_string(),
            scenario_id: request.scenario_id.clone(),
            frame_limit: request.frame_limit,
            build_status: "not_requested".to_string(),
            runtime_spawn_status: if request.launch_runtime_process {
                "not_implemented_in_c_min".to_string()
            } else {
                "not_requested".to_string()
            },
            package_load_status: "not_started".to_string(),
            asset_load_status: "not_started".to_string(),
            scene_load_status: "not_started".to_string(),
            logic_tick_status: "not_started".to_string(),
            render_extract_status: "not_started".to_string(),
            render_thread_status: "not_started".to_string(),
            rdg_status: "not_started".to_string(),
            rhi_status: "not_started".to_string(),
            surface_status: "not_started".to_string(),
            present_status: "not_started".to_string(),
            frames_requested: request.frame_limit,
            frames_completed: 0,
            first_presented_frame: None,
            exit_code: None,
            diagnostics: Vec::new(),
        }
    }
}

pub struct DefaultGameRunOrchestrator;

impl Default for DefaultGameRunOrchestrator {
    fn default() -> Self {
        Self
    }
}

impl DefaultGameRunOrchestrator {
    pub fn run(&self, request: DefaultGameRunRequest) -> EndToEndGameRunReport {
        match request.mode {
            DefaultGameRunMode::Headless => run_headless_end_to_end(request),
            DefaultGameRunMode::Windowed => {
                let mut report = EndToEndGameRunReport::base(&request);
                report.exit_code = Some(1);
                report.diagnostics.push(EndToEndGameRunDiagnostic::error(
                    "windowed_backend_required",
                    "window",
                    "Windowed end-to-end run must be executed by the native window host in C-min",
                ));
                report
            }
        }
    }
}

pub fn run_headless_end_to_end(request: DefaultGameRunRequest) -> EndToEndGameRunReport {
    let mut report = EndToEndGameRunReport::base(&request);
    report.surface_status = "headless".to_string();

    if request.frame_limit == 0 {
        report.exit_code = Some(1);
        report.diagnostics.push(EndToEndGameRunDiagnostic::error(
            "invalid_frame_limit",
            "request",
            "frame_limit must be greater than zero",
        ));
        return report;
    }

    let package_load = load_runtime_package(&request.runtime_package_path);
    report.diagnostics.extend(convert_runtime_diagnostics(
        "package",
        &package_load.diagnostics.issues,
    ));
    let Some(package) = package_load.value else {
        report.package_load_status = "error".to_string();
        report.asset_load_status = "not_started".to_string();
        report.scene_load_status = "not_started".to_string();
        report.exit_code = Some(1);
        return report;
    };
    report.package_load_status = "ok".to_string();
    report.asset_load_status = "ok".to_string();

    let world_load = hydrate_active_scene_into_world(&package);
    report.diagnostics.extend(convert_runtime_diagnostics(
        "scene",
        &world_load.diagnostics.issues,
    ));
    let Some((mut world, _hydration_report)) = world_load.value else {
        report.scene_load_status = "error".to_string();
        report.exit_code = Some(1);
        return report;
    };
    report.scene_load_status = "ok".to_string();

    let mut host = EngineHostLoop::new(package.active_scene.id.clone());
    for _ in 0..request.frame_limit {
        let output = host.tick(
            EngineFrameInput::new(EngineHostMode::ExportedGame),
            &mut world,
        );
        report.frames_completed += 1;
        if output.runtime_advanced {
            report.logic_tick_status = "ok".to_string();
        }
        if output.render_frame_report.is_some() {
            report.render_extract_status = "ok".to_string();
        }
        if let Some(render_thread_frame) = output.render_thread_frame {
            report.render_thread_status = "ok".to_string();
            report.rdg_status = render_thread_frame.report.rdg_status;
            report.rhi_status = render_thread_frame.report.rhi_status;
            report.present_status = render_thread_frame.report.present_status;
            if report.present_status == "presented" && report.first_presented_frame.is_none() {
                report.first_presented_frame = Some(output.frame_index);
            }
        }
    }

    if report.logic_tick_status == "not_started" {
        report.logic_tick_status = "error".to_string();
        report.diagnostics.push(EndToEndGameRunDiagnostic::error(
            "logic_tick_missing",
            "logic",
            "EngineHostLoop did not advance any runtime frame",
        ));
    }
    if report.render_extract_status == "not_started" {
        report.render_extract_status = "error".to_string();
        report.diagnostics.push(EndToEndGameRunDiagnostic::error(
            "render_extract_missing",
            "render_extract",
            "No RenderFrameReport was produced",
        ));
    }
    if report.render_thread_status == "not_started" {
        report.render_thread_status = "error".to_string();
        report.diagnostics.push(EndToEndGameRunDiagnostic::error(
            "render_thread_missing",
            "render_thread",
            "No RenderThreadFrameOutput was produced",
        ));
    }
    if report.present_status == "not_started" {
        report.present_status = "error".to_string();
        report.diagnostics.push(EndToEndGameRunDiagnostic::error(
            "present_missing",
            "present",
            "No present status was produced",
        ));
    }

    report.exit_code = Some(if report.has_errors() { 1 } else { 0 });
    report
}

fn convert_runtime_diagnostics(
    layer: &str,
    diagnostics: &[RuntimeDiagnostic],
) -> Vec<EndToEndGameRunDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| EndToEndGameRunDiagnostic {
            severity: match diagnostic.severity {
                DiagnosticSeverity::Error => EndToEndGameRunDiagnosticSeverity::Error,
                DiagnosticSeverity::Warning => EndToEndGameRunDiagnosticSeverity::Warning,
            },
            code: match diagnostic.severity {
                DiagnosticSeverity::Error => format!("{layer}_load_error"),
                DiagnosticSeverity::Warning => format!("{layer}_load_warning"),
            },
            layer: layer.to_string(),
            message: diagnostic.message.clone(),
            path: Some(diagnostic.path.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_run::tests_support::write_minimal_runtime_package;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_game_run_request_defaults_to_headless_for_tests() {
        let request = DefaultGameRunRequest::headless_for_tests("runtime-package");

        assert_eq!(request.mode, DefaultGameRunMode::Headless);
        assert_eq!(request.scenario_id, "minimal_game_loop");
        assert_eq!(request.frame_limit, 3);
        assert!(!request.launch_runtime_process);
    }

    #[test]
    fn default_game_run_request_supports_windowed_mode() {
        let request = DefaultGameRunRequest::windowed_for_user_run("runtime-package");

        assert_eq!(request.mode, DefaultGameRunMode::Windowed);
        assert!(request.launch_runtime_process);
    }

    #[test]
    fn default_game_run_request_is_json_serializable() {
        let request = DefaultGameRunRequest::headless_for_tests("runtime-package");
        let json = serde_json::to_string(&request).expect("request should serialize");

        assert!(json.contains("minimal_game_loop"));
        assert!(json.contains("headless"));
    }

    #[test]
    fn end_to_end_game_run_report_is_json_serializable() {
        let request = DefaultGameRunRequest::headless_for_tests("runtime-package");
        let mut report = EndToEndGameRunReport::base(&request);
        report.diagnostics.push(EndToEndGameRunDiagnostic::error(
            "present_failed",
            "present",
            "present failed",
        ));

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains(END_TO_END_GAME_RUN_REPORT_SCHEMA_VERSION));
        assert!(json.contains("present"));
    }

    #[test]
    fn default_game_run_orchestrator_runs_headless_minimal_scenario() {
        let root = temp_root("headless-minimal");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = DefaultGameRunRequest::headless_for_tests(package_dir);

        let report = DefaultGameRunOrchestrator.run(request);

        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.package_load_status, "ok");
        assert_eq!(report.scene_load_status, "ok");
        assert_eq!(report.logic_tick_status, "ok");
        assert_eq!(report.render_extract_status, "ok");
        assert_eq!(report.render_thread_status, "ok");
        assert_eq!(report.present_status, "presented");
        assert_eq!(report.frames_completed, 3);
        assert_eq!(report.first_presented_frame, Some(1));
    }

    #[test]
    fn default_game_run_orchestrator_uses_staged_runtime_package() {
        let root = temp_root("staged-path");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = DefaultGameRunRequest::headless_for_tests(package_dir.clone());

        let report = DefaultGameRunOrchestrator.run(request);

        assert_eq!(
            report.runtime_package_path,
            package_dir.display().to_string()
        );
        assert_eq!(report.build_status, "not_requested");
    }

    #[test]
    fn end_to_end_game_run_report_locates_package_failure() {
        let root = temp_root("missing-package");
        let request = DefaultGameRunRequest::headless_for_tests(root.join("missing-package"));

        let report = DefaultGameRunOrchestrator.run(request);

        assert_eq!(report.exit_code, Some(1));
        assert_eq!(report.package_load_status, "error");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.layer == "package"));
    }

    #[test]
    fn end_to_end_game_run_report_locates_present_failure() {
        let request = DefaultGameRunRequest::headless_for_tests("runtime-package");
        let mut report = EndToEndGameRunReport::base(&request);
        report.present_status = "error".to_string();
        report.diagnostics.push(EndToEndGameRunDiagnostic::error(
            "present_missing",
            "present",
            "No present status was produced",
        ));

        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.layer == "present"));
    }

    #[test]
    fn end_to_end_game_run_report_never_reports_unknown_without_layer() {
        let root = temp_root("layered-diagnostics");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = DefaultGameRunRequest::headless_for_tests(package_dir);

        let report = DefaultGameRunOrchestrator.run(request);

        assert!(report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.layer.trim().is_empty()));
        assert!(report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "unknown_error"));
    }

    #[test]
    fn headless_and_windowed_share_runtime_package_scenario_and_frame_limit() {
        let package = PathBuf::from("staged/runtime-package");
        let headless = DefaultGameRunRequest {
            frame_limit: 5,
            scenario_id: "minimal_game_loop".to_string(),
            ..DefaultGameRunRequest::headless_for_tests(package.clone())
        };
        let windowed = DefaultGameRunRequest {
            project_path: headless.project_path.clone(),
            runtime_package_path: headless.runtime_package_path.clone(),
            mode: DefaultGameRunMode::Windowed,
            scenario_id: headless.scenario_id.clone(),
            frame_limit: headless.frame_limit,
            report_path: None,
            launch_runtime_process: true,
        };

        assert_eq!(headless.runtime_package_path, windowed.runtime_package_path);
        assert_eq!(headless.scenario_id, windowed.scenario_id);
        assert_eq!(headless.frame_limit, windowed.frame_limit);
        assert_ne!(headless.mode, windowed.mode);
        assert_ne!(
            headless.launch_runtime_process,
            windowed.launch_runtime_process
        );
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("default-game-run-{name}-{stamp}"))
    }
}
