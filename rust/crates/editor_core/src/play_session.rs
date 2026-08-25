use engine_runtime::default_game_run::{
    DefaultGameRunMode, DefaultGameRunOrchestrator, DefaultGameRunRequest,
    EndToEndGameRunDiagnostic, EndToEndGameRunDiagnosticSeverity, EndToEndGameRunReport,
};
use engine_runtime::game_view_presentation::GameViewTargetSpec;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const PLAY_SESSION_REPORT_SCHEMA_VERSION: &str = "play-session-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlaySessionMode {
    HeadlessGate,
    WindowedUserRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaySessionState {
    Idle,
    Preparing,
    Building,
    StagingPackage,
    Launching,
    Running,
    Stopping,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaySessionRequestedBy {
    Toolbar,
    Automation,
    EditorCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySessionRequest {
    pub session_id: String,
    pub mode: PlaySessionMode,
    pub project_root: PathBuf,
    pub runtime_package_path: PathBuf,
    pub scene_ref: Option<String>,
    pub build_profile: Option<String>,
    pub run_profile: Option<String>,
    pub frame_limit: u64,
    pub report_path: Option<PathBuf>,
    pub requested_by: PlaySessionRequestedBy,
    pub preview_package_report_path: Option<String>,
    pub preview_cache_status: Option<String>,
    pub preview_dirty_domains: Vec<String>,
    pub preview_prepare_duration_ms: Option<u64>,
    #[serde(default)]
    pub game_view_target: GameViewTargetSpec,
}

impl PlaySessionRequest {
    pub fn headless_gate(runtime_package_path: impl Into<PathBuf>) -> Self {
        let runtime_package_path = runtime_package_path.into();
        Self {
            session_id: "play-session-headless-gate".to_string(),
            mode: PlaySessionMode::HeadlessGate,
            project_root: runtime_package_path.clone(),
            runtime_package_path,
            scene_ref: None,
            build_profile: None,
            run_profile: Some("headless-gate".to_string()),
            frame_limit: 3,
            report_path: None,
            requested_by: PlaySessionRequestedBy::Toolbar,
            preview_package_report_path: None,
            preview_cache_status: None,
            preview_dirty_domains: Vec::new(),
            preview_prepare_duration_ms: None,
            game_view_target: GameViewTargetSpec::default(),
        }
    }

    pub fn windowed_user_run(runtime_package_path: impl Into<PathBuf>) -> Self {
        let runtime_package_path = runtime_package_path.into();
        Self {
            session_id: "play-session-windowed-user-run".to_string(),
            mode: PlaySessionMode::WindowedUserRun,
            project_root: runtime_package_path.clone(),
            runtime_package_path,
            scene_ref: None,
            build_profile: None,
            run_profile: Some("windowed-user-run".to_string()),
            frame_limit: 3,
            report_path: None,
            requested_by: PlaySessionRequestedBy::Toolbar,
            preview_package_report_path: None,
            preview_cache_status: None,
            preview_dirty_domains: Vec::new(),
            preview_prepare_duration_ms: None,
            game_view_target: GameViewTargetSpec::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySessionRequestSummary {
    pub project_root: String,
    pub runtime_package_path: String,
    pub scene_ref: Option<String>,
    pub build_profile: Option<String>,
    pub run_profile: Option<String>,
    pub frame_limit: u64,
    pub requested_by: PlaySessionRequestedBy,
    pub game_view_target: GameViewTargetSpec,
}

impl From<&PlaySessionRequest> for PlaySessionRequestSummary {
    fn from(request: &PlaySessionRequest) -> Self {
        Self {
            project_root: request.project_root.display().to_string(),
            runtime_package_path: request.runtime_package_path.display().to_string(),
            scene_ref: request.scene_ref.clone(),
            build_profile: request.build_profile.clone(),
            run_profile: request.run_profile.clone(),
            frame_limit: request.frame_limit,
            requested_by: request.requested_by,
            game_view_target: request.game_view_target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySessionBuildSummary {
    pub status: String,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySessionProcessSummary {
    pub launch_requested: bool,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySessionDiagnostic {
    pub severity: PlaySessionDiagnosticSeverity,
    pub code: String,
    pub layer: String,
    pub message: String,
    pub path: Option<String>,
}

impl PlaySessionDiagnostic {
    pub fn error(
        code: impl Into<String>,
        layer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: PlaySessionDiagnosticSeverity::Error,
            code: code.into(),
            layer: layer.into(),
            message: message.into(),
            path: None,
        }
    }
}

impl From<&EndToEndGameRunDiagnostic> for PlaySessionDiagnostic {
    fn from(diagnostic: &EndToEndGameRunDiagnostic) -> Self {
        Self {
            severity: match diagnostic.severity {
                EndToEndGameRunDiagnosticSeverity::Info => PlaySessionDiagnosticSeverity::Info,
                EndToEndGameRunDiagnosticSeverity::Warning => {
                    PlaySessionDiagnosticSeverity::Warning
                }
                EndToEndGameRunDiagnosticSeverity::Error => PlaySessionDiagnosticSeverity::Error,
            },
            code: diagnostic.code.clone(),
            layer: diagnostic.layer.clone(),
            message: diagnostic.message.clone(),
            path: diagnostic.path.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaySessionDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySessionReport {
    pub schema_version: String,
    pub session_id: String,
    pub mode: PlaySessionMode,
    pub state: PlaySessionState,
    pub request_summary: PlaySessionRequestSummary,
    pub build_summary: PlaySessionBuildSummary,
    pub runtime_report: Option<EndToEndGameRunReport>,
    pub process_summary: PlaySessionProcessSummary,
    pub diagnostics: Vec<PlaySessionDiagnostic>,
    pub started_at_frame: Option<u64>,
    pub ended_at_frame: Option<u64>,
    pub preview_package_report_path: Option<String>,
    pub preview_cache_status: Option<String>,
    pub preview_dirty_domains: Vec<String>,
    pub preview_prepare_duration_ms: Option<u64>,
    pub runner_kind: Option<String>,
    pub game_view_present_report_path: Option<String>,
    pub game_view_frame_count: Option<u64>,
    pub game_view_last_frame_hash: Option<String>,
}

impl PlaySessionReport {
    fn completed(request: &PlaySessionRequest, runtime_report: EndToEndGameRunReport) -> Self {
        let has_errors = runtime_report.has_errors() || runtime_report.exit_code != Some(0);
        let diagnostics = runtime_report
            .diagnostics
            .iter()
            .map(PlaySessionDiagnostic::from)
            .collect::<Vec<_>>();
        Self {
            schema_version: PLAY_SESSION_REPORT_SCHEMA_VERSION.to_string(),
            session_id: request.session_id.clone(),
            mode: request.mode,
            state: if has_errors {
                PlaySessionState::Failed
            } else {
                PlaySessionState::Completed
            },
            request_summary: request.into(),
            build_summary: PlaySessionBuildSummary {
                status: runtime_report.build_status.clone(),
                profile: request.build_profile.clone(),
            },
            process_summary: PlaySessionProcessSummary {
                launch_requested: false,
                pid: None,
                exit_code: runtime_report.exit_code,
                status: runtime_report.runtime_spawn_status.clone(),
            },
            runtime_report: Some(runtime_report),
            diagnostics,
            started_at_frame: Some(0),
            ended_at_frame: Some(request.frame_limit),
            preview_package_report_path: request.preview_package_report_path.clone(),
            preview_cache_status: request.preview_cache_status.clone(),
            preview_dirty_domains: request.preview_dirty_domains.clone(),
            preview_prepare_duration_ms: request.preview_prepare_duration_ms,
            runner_kind: Some("default_headless_gate".to_string()),
            game_view_present_report_path: None,
            game_view_frame_count: None,
            game_view_last_frame_hash: None,
        }
    }

    pub(crate) fn failed_before_runtime(
        request: &PlaySessionRequest,
        code: &str,
        layer: &str,
        message: &str,
    ) -> Self {
        Self {
            schema_version: PLAY_SESSION_REPORT_SCHEMA_VERSION.to_string(),
            session_id: request.session_id.clone(),
            mode: request.mode,
            state: PlaySessionState::Failed,
            request_summary: request.into(),
            build_summary: PlaySessionBuildSummary {
                status: "not_requested".to_string(),
                profile: request.build_profile.clone(),
            },
            runtime_report: None,
            process_summary: PlaySessionProcessSummary {
                launch_requested: request.mode == PlaySessionMode::WindowedUserRun,
                pid: None,
                exit_code: Some(1),
                status: "not_started".to_string(),
            },
            diagnostics: vec![PlaySessionDiagnostic::error(code, layer, message)],
            started_at_frame: Some(0),
            ended_at_frame: Some(0),
            preview_package_report_path: request.preview_package_report_path.clone(),
            preview_cache_status: request.preview_cache_status.clone(),
            preview_dirty_domains: request.preview_dirty_domains.clone(),
            preview_prepare_duration_ms: request.preview_prepare_duration_ms,
            runner_kind: None,
            game_view_present_report_path: None,
            game_view_frame_count: None,
            game_view_last_frame_hash: None,
        }
    }

    pub(crate) fn from_game_view_present_report(
        request: &PlaySessionRequest,
        success: bool,
        game_view_present_report_path: Option<String>,
        game_view_frame_count: u64,
        game_view_last_frame_hash: Option<String>,
        diagnostics: Vec<PlaySessionDiagnostic>,
    ) -> Self {
        Self {
            schema_version: PLAY_SESSION_REPORT_SCHEMA_VERSION.to_string(),
            session_id: request.session_id.clone(),
            mode: request.mode,
            state: if success {
                PlaySessionState::Running
            } else {
                PlaySessionState::Failed
            },
            request_summary: request.into(),
            build_summary: PlaySessionBuildSummary {
                status: "not_requested".to_string(),
                profile: request.build_profile.clone(),
            },
            runtime_report: None,
            process_summary: PlaySessionProcessSummary {
                launch_requested: false,
                pid: None,
                exit_code: if success { None } else { Some(1) },
                status: if success {
                    "editor_in_process_gameview_running".to_string()
                } else {
                    "editor_in_process_gameview_failed".to_string()
                },
            },
            diagnostics,
            started_at_frame: Some(0),
            ended_at_frame: if success {
                None
            } else {
                Some(game_view_frame_count)
            },
            preview_package_report_path: request.preview_package_report_path.clone(),
            preview_cache_status: request.preview_cache_status.clone(),
            preview_dirty_domains: request.preview_dirty_domains.clone(),
            preview_prepare_duration_ms: request.preview_prepare_duration_ms,
            runner_kind: Some("editor_in_process_gameview".to_string()),
            game_view_present_report_path,
            game_view_frame_count: Some(game_view_frame_count),
            game_view_last_frame_hash,
        }
    }
}

pub trait PlayRunner {
    fn run_play_session(&self, request: PlaySessionRequest) -> PlaySessionReport;
}

impl PlayRunner for DefaultGameRunOrchestrator {
    fn run_play_session(&self, request: PlaySessionRequest) -> PlaySessionReport {
        match request.mode {
            PlaySessionMode::HeadlessGate => run_headless_play_session(request, self),
            PlaySessionMode::WindowedUserRun => PlaySessionReport::failed_before_runtime(
                &request,
                "windowed_session_runner_not_configured",
                "runner",
                "WindowedUserRun requires an EditorGameViewPlayRunner or external window runner.",
            ),
        }
    }
}

#[derive(Debug, Default)]
pub struct PlaySessionController {
    current_state: PlaySessionState,
    current_session_id: Option<String>,
    last_report: Option<PlaySessionReport>,
    queued_request: Option<PlaySessionRequest>,
    queued_stop: Option<Option<String>>,
}

impl PlaySessionController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> PlaySessionState {
        self.current_state
    }

    pub fn last_report(&self) -> Option<&PlaySessionReport> {
        self.last_report.as_ref()
    }

    pub fn queue_start(&mut self, request: PlaySessionRequest) {
        self.queued_request = Some(request);
    }

    pub fn queue_stop(&mut self, session_id: Option<String>) {
        self.queued_stop = Some(session_id);
    }

    pub fn drain_queued_with_runner(
        &mut self,
        runner: &dyn PlayRunner,
    ) -> Option<PlaySessionReport> {
        if let Some(stop_session_id) = self.queued_stop.take() {
            return Some(self.stop_current(stop_session_id));
        }
        let Some(request) = self.queued_request.take() else {
            return None;
        };
        self.current_state = PlaySessionState::Launching;
        self.current_session_id = Some(request.session_id.clone());
        let report = runner.run_play_session(request);
        self.current_state = report.state;
        self.current_session_id = Some(report.session_id.clone());
        self.last_report = Some(report.clone());
        Some(report)
    }

    pub fn drain_queued_with_runtime(
        &mut self,
        runner: &DefaultGameRunOrchestrator,
    ) -> Option<PlaySessionReport> {
        self.drain_queued_with_runner(runner)
    }

    fn stop_current(&mut self, session_id: Option<String>) -> PlaySessionReport {
        let request = PlaySessionRequest {
            session_id: session_id
                .clone()
                .or_else(|| self.current_session_id.clone())
                .unwrap_or_else(|| "play-session-stop".to_string()),
            mode: PlaySessionMode::HeadlessGate,
            project_root: PathBuf::new(),
            runtime_package_path: PathBuf::new(),
            scene_ref: None,
            build_profile: None,
            run_profile: Some("stop".to_string()),
            frame_limit: 0,
            report_path: None,
            requested_by: PlaySessionRequestedBy::Toolbar,
            preview_package_report_path: None,
            preview_cache_status: None,
            preview_dirty_domains: Vec::new(),
            preview_prepare_duration_ms: None,
            game_view_target: GameViewTargetSpec::default(),
        };
        let active = matches!(
            self.current_state,
            PlaySessionState::Preparing
                | PlaySessionState::Building
                | PlaySessionState::StagingPackage
                | PlaySessionState::Launching
                | PlaySessionState::Running
        );
        let report = if active {
            self.current_state = PlaySessionState::Completed;
            PlaySessionReport {
                schema_version: PLAY_SESSION_REPORT_SCHEMA_VERSION.to_string(),
                session_id: request.session_id.clone(),
                mode: request.mode,
                state: PlaySessionState::Completed,
                request_summary: (&request).into(),
                build_summary: PlaySessionBuildSummary {
                    status: "not_requested".to_string(),
                    profile: None,
                },
                runtime_report: None,
                process_summary: PlaySessionProcessSummary {
                    launch_requested: false,
                    pid: None,
                    exit_code: Some(0),
                    status: "stopped".to_string(),
                },
                diagnostics: Vec::new(),
                started_at_frame: None,
                ended_at_frame: None,
                preview_package_report_path: request.preview_package_report_path.clone(),
                preview_cache_status: request.preview_cache_status.clone(),
                preview_dirty_domains: request.preview_dirty_domains.clone(),
                preview_prepare_duration_ms: request.preview_prepare_duration_ms,
                runner_kind: None,
                game_view_present_report_path: None,
                game_view_frame_count: None,
                game_view_last_frame_hash: None,
            }
        } else {
            PlaySessionReport::failed_before_runtime(
                &request,
                "no_active_play_session",
                "request",
                "No active play session can be stopped in C-min.",
            )
        };
        self.last_report = Some(report.clone());
        report
    }
}

fn run_headless_play_session(
    request: PlaySessionRequest,
    runner: &DefaultGameRunOrchestrator,
) -> PlaySessionReport {
    let runtime_request = DefaultGameRunRequest {
        project_path: request.project_root.clone(),
        runtime_package_path: request.runtime_package_path.clone(),
        mode: DefaultGameRunMode::Headless,
        scenario_id: request
            .run_profile
            .clone()
            .unwrap_or_else(|| "minimal_game_loop".to_string()),
        frame_limit: request.frame_limit,
        report_path: request.report_path.clone(),
        launch_runtime_process: false,
    };
    PlaySessionReport::completed(&request, runner.run(runtime_request))
}

impl Default for PlaySessionState {
    fn default() -> Self {
        Self::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn play_session_request_defaults_to_headless_gate() {
        let request = PlaySessionRequest::headless_gate("runtime-package");

        assert_eq!(request.mode, PlaySessionMode::HeadlessGate);
        assert_eq!(request.frame_limit, 3);
        assert_eq!(request.requested_by, PlaySessionRequestedBy::Toolbar);
    }

    #[test]
    fn play_session_report_is_json_serializable() {
        let request = PlaySessionRequest::windowed_user_run("runtime-package");
        let report = PlaySessionReport::failed_before_runtime(
            &request,
            "windowed_session_runner_not_configured",
            "runner",
            "Windowed user run requires a configured runner.",
        );

        let json = serde_json::to_string(&report).expect("report should serialize");

        assert!(json.contains(PLAY_SESSION_REPORT_SCHEMA_VERSION));
        assert!(json.contains("windowed_session_runner_not_configured"));
    }

    #[test]
    fn play_session_report_wraps_end_to_end_report() {
        let root = temp_root("wraps-report");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let request = PlaySessionRequest::headless_gate(package_dir);
        let runtime_report = DefaultGameRunOrchestrator.run(DefaultGameRunRequest {
            project_path: request.project_root.clone(),
            runtime_package_path: request.runtime_package_path.clone(),
            mode: DefaultGameRunMode::Headless,
            scenario_id: "minimal_game_loop".to_string(),
            frame_limit: request.frame_limit,
            report_path: None,
            launch_runtime_process: false,
        });

        let report = PlaySessionReport::completed(&request, runtime_report);

        assert_eq!(report.state, PlaySessionState::Completed);
        assert_eq!(
            report
                .runtime_report
                .as_ref()
                .map(|runtime| runtime.frames_completed),
            Some(3)
        );
    }

    #[test]
    fn play_session_controller_queues_start_without_running_immediately() {
        let mut controller = PlaySessionController::new();

        controller.queue_start(PlaySessionRequest::headless_gate("runtime-package"));

        assert_eq!(controller.state(), PlaySessionState::Idle);
        assert!(controller.last_report().is_none());
    }

    #[test]
    fn play_session_controller_runs_headless_at_stable_point() {
        let root = temp_root("headless-controller");
        let package_dir = write_minimal_runtime_package(&root, "runtime-package");
        let mut controller = PlaySessionController::new();
        controller.queue_start(PlaySessionRequest::headless_gate(package_dir));

        let report = controller
            .drain_queued_with_runtime(&DefaultGameRunOrchestrator)
            .expect("queued play session should run");

        assert_eq!(report.state, PlaySessionState::Completed);
        assert_eq!(controller.state(), PlaySessionState::Completed);
        assert_eq!(
            report
                .runtime_report
                .as_ref()
                .map(|runtime| runtime.frames_completed),
            Some(3)
        );
    }

    #[test]
    fn play_session_controller_windowed_uses_configured_runner() {
        struct StubGameViewRunner;

        impl PlayRunner for StubGameViewRunner {
            fn run_play_session(&self, request: PlaySessionRequest) -> PlaySessionReport {
                PlaySessionReport::from_game_view_present_report(
                    &request,
                    true,
                    Some("gameview-report.json".to_string()),
                    2,
                    Some("frame-hash".to_string()),
                    Vec::new(),
                )
            }
        }

        let mut controller = PlaySessionController::new();
        controller.queue_start(PlaySessionRequest::windowed_user_run("runtime-package"));

        let report = controller
            .drain_queued_with_runner(&StubGameViewRunner)
            .expect("queued play session should run");

        assert_eq!(report.state, PlaySessionState::Running);
        assert_eq!(
            report.runner_kind.as_deref(),
            Some("editor_in_process_gameview")
        );
        assert_eq!(report.game_view_frame_count, Some(2));
    }

    #[test]
    fn play_session_controller_stop_only_targets_current_session() {
        let mut controller = PlaySessionController::new();

        controller.queue_stop(Some("missing".to_string()));
        let report = controller
            .drain_queued_with_runtime(&DefaultGameRunOrchestrator)
            .expect("queued stop should produce report");

        assert_eq!(report.state, PlaySessionState::Failed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "no_active_play_session"));
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("editor-play-session-{name}-{stamp}"))
    }

    fn write_minimal_runtime_package(root: &std::path::Path, name: &str) -> PathBuf {
        let package_dir = root.join(name);
        fs::create_dir_all(package_dir.join("scenes")).unwrap();
        fs::create_dir_all(package_dir.join("assets")).unwrap();
        fs::create_dir_all(package_dir.join("rules")).unwrap();
        fs::create_dir_all(package_dir.join("input")).unwrap();
        fs::write(
            package_dir.join("manifest.json"),
            r#"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "editor-play-session-test",
    "name": "Editor Play Session Test",
    "version": "0.0.2",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 1 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 1 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "rust-aot" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.none", "mappingCount": 1 },
  "contentHash": null
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("scenes").join("scene-main.json"),
            r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000000",
  "skyColor": "#101010",
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
    }
  }]
}"##,
        )
        .unwrap();
        fs::write(
            package_dir.join("assets").join("asset-manifest.json"),
            r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [{
    "id": "scene-main",
    "name": "Main",
    "type": "scene",
    "source": "scenes/scene-main.json",
    "state": "available",
    "bundleId": "startup"
  }],
  "runtimeAssetIndex": [{
    "assetGuid": "scene-main",
    "assetId": "scene-main",
    "assetType": "scene",
    "subAssetId": null,
    "version": "1",
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "loaderKind": "scene",
    "dependencies": [],
    "hash": null,
    "size": null,
    "flags": ["test"]
  }],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [{
    "cookedAssetId": "cooked-scene-main",
    "bundleId": "startup",
    "path": "scenes/scene-main.json",
    "offset": null,
    "size": null,
    "compression": "none",
    "hash": null
  }],
  "dependencyTable": []
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("rules").join("rule-manifest.json"),
            r#"{
  "schemaVersion": "runtime-rule-manifest.v1",
  "mode": "rust-aot",
  "rules": [],
  "modules": []
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input-manifest.json"),
            r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.none",
  "mappings": [{ "id": "input.none", "path": "input/input.none.json", "enabled": true }]
}"#,
        )
        .unwrap();
        fs::write(
            package_dir.join("input").join("input.none.json"),
            r#"{
  "schema_version": "input-mapping.v2",
  "asset_id": "input.none",
  "actions": [],
  "contexts": [],
  "bindings": [],
  "platform_overrides": []
}"#,
        )
        .unwrap();
        package_dir
    }
}
