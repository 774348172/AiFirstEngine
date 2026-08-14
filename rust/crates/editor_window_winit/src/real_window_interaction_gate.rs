use crate::application::NativeEditorApplication;
use crate::config::NativeEditorWindowConfig;
use crate::headless_app::{
    RealNativeEditorWindowDiagnostic, RealNativeEditorWindowDiagnosticSeverity,
    RealNativeEditorWindowReport,
};
use crate::interaction_gate::{
    NativeEditorInteractionReport, NativeEditorInteractionRunner, NativeEditorInteractionScenario,
    NativeEditorInteractionStatus,
};
use crate::surface::{HeadlessSurfaceBackend, SurfaceState};
use editor_ui_model::EditorUiMode;
use serde::{Deserialize, Serialize};

pub const REAL_WINDOW_INTERACTION_SMOKE_REPORT_SCHEMA_VERSION: &str =
    "real-window-interaction-smoke-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealWindowInteractionSmokeStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealWindowInteractionDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealWindowInteractionDiagnostic {
    pub severity: RealWindowInteractionDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source_stage: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealWindowScreenshotEvidence {
    pub kind: String,
    pub width: u32,
    pub height: u32,
    pub frame_index: u64,
    pub artifact_path: Option<String>,
    pub rgba_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealWindowInteractionSmokeScenario {
    pub scenario: NativeEditorInteractionScenario,
    pub max_frames: u32,
    pub width: u32,
    pub height: u32,
    pub require_present: bool,
    pub require_screenshot_evidence: bool,
}

impl RealWindowInteractionSmokeScenario {
    pub fn new(scenario: NativeEditorInteractionScenario) -> Self {
        Self {
            scenario,
            max_frames: 8,
            width: 1280,
            height: 720,
            require_present: true,
            require_screenshot_evidence: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealWindowInteractionSmokeReport {
    pub schema_version: String,
    pub status: RealWindowInteractionSmokeStatus,
    pub backend: String,
    pub window_created: bool,
    pub surface_created: bool,
    pub surface_configured: bool,
    pub present_status: String,
    pub frame_count: u64,
    pub draw_command_count: usize,
    pub hit_region_count: usize,
    pub final_mode: EditorUiMode,
    pub interaction_report: Option<NativeEditorInteractionReport>,
    pub screenshot: Option<RealWindowScreenshotEvidence>,
    pub surface: Option<SurfaceState>,
    pub diagnostics: Vec<RealWindowInteractionDiagnostic>,
}

impl RealWindowInteractionSmokeReport {
    pub fn skipped(
        backend: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: REAL_WINDOW_INTERACTION_SMOKE_REPORT_SCHEMA_VERSION.to_string(),
            status: RealWindowInteractionSmokeStatus::Skipped,
            backend: backend.into(),
            window_created: false,
            surface_created: false,
            surface_configured: false,
            present_status: "skipped".to_string(),
            frame_count: 0,
            draw_command_count: 0,
            hit_region_count: 0,
            final_mode: EditorUiMode::ProjectLauncher,
            interaction_report: None,
            screenshot: None,
            surface: None,
            diagnostics: vec![RealWindowInteractionDiagnostic {
                severity: RealWindowInteractionDiagnosticSeverity::Warning,
                code: code.into(),
                message: message.into(),
                source_stage: "real_window_interaction_gate".to_string(),
            }],
        }
    }

    pub fn failed(
        backend: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: RealWindowInteractionSmokeStatus::Failed,
            diagnostics: vec![RealWindowInteractionDiagnostic {
                severity: RealWindowInteractionDiagnosticSeverity::Error,
                code: code.into(),
                message: message.into(),
                source_stage: "real_window_interaction_gate".to_string(),
            }],
            ..Self::skipped(backend, "real_window_interaction.failed", "failed")
        }
    }
}

pub struct RealWindowInteractionSmokeRunner {
    backend: String,
}

impl Default for RealWindowInteractionSmokeRunner {
    fn default() -> Self {
        Self::headless_compatible()
    }
}

impl RealWindowInteractionSmokeRunner {
    pub fn headless_compatible() -> Self {
        Self {
            backend: "headless-compatible-window-event-bridge".to_string(),
        }
    }

    pub fn run(
        &self,
        app: &mut NativeEditorApplication,
        scenario: RealWindowInteractionSmokeScenario,
    ) -> RealWindowInteractionSmokeReport {
        let mut diagnostics = Vec::new();
        let mut surface = HeadlessSurfaceBackend::create_surface();
        surface.configure(scenario.width, scenario.height, "Bgra8UnormSrgb", "Fifo");

        let interaction_runner =
            NativeEditorInteractionRunner::headless(scenario.width as f32, scenario.height as f32);
        let interaction_report = interaction_runner.run(app, scenario.scenario);

        surface.acquire();
        surface.present();
        let app_report = app.frame(scenario.width as f32, scenario.height as f32);
        let surface_state = surface.snapshot();

        if scenario.require_present && surface_state.presented_frame == 0 {
            diagnostics.push(error(
                "real_window_interaction.present_missing",
                "No surface frame was presented by the smoke gate.",
                "surface",
            ));
        }

        let screenshot = Some(RealWindowScreenshotEvidence {
            kind: "metadata-only".to_string(),
            width: scenario.width,
            height: scenario.height,
            frame_index: app_report.frame_index,
            artifact_path: None,
            rgba_hash: Some(format!(
                "draw:{}:hit:{}:frame:{}",
                app_report.draw_command_count, app_report.hit_region_count, app_report.frame_index
            )),
        });

        if scenario.require_screenshot_evidence && screenshot.is_none() {
            diagnostics.push(error(
                "real_window_interaction.screenshot_missing",
                "Screenshot evidence was required but not produced.",
                "screenshot",
            ));
        }

        let interaction_failed = interaction_report.status == NativeEditorInteractionStatus::Failed;
        let has_errors = diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == RealWindowInteractionDiagnosticSeverity::Error
        });
        let status = if interaction_failed || has_errors {
            RealWindowInteractionSmokeStatus::Failed
        } else {
            RealWindowInteractionSmokeStatus::Passed
        };

        RealWindowInteractionSmokeReport {
            schema_version: REAL_WINDOW_INTERACTION_SMOKE_REPORT_SCHEMA_VERSION.to_string(),
            status,
            backend: self.backend.clone(),
            window_created: true,
            surface_created: true,
            surface_configured: surface_state.configured,
            present_status: if surface_state.presented_frame > 0 {
                "presented".to_string()
            } else {
                "not_presented".to_string()
            },
            frame_count: surface_state.presented_frame.max(app_report.frame_index),
            draw_command_count: app_report.draw_command_count,
            hit_region_count: app_report.hit_region_count,
            final_mode: app_report.mode,
            interaction_report: Some(interaction_report),
            screenshot,
            surface: Some(surface_state),
            diagnostics,
        }
    }
}

pub fn run_headless_real_window_interaction_smoke(
    scenario: RealWindowInteractionSmokeScenario,
) -> RealWindowInteractionSmokeReport {
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    RealWindowInteractionSmokeRunner::default().run(&mut app, scenario)
}

#[cfg(feature = "real-window")]
pub fn run_real_window_interaction_smoke_local_only(
    scenario: RealWindowInteractionSmokeScenario,
) -> RealWindowInteractionSmokeReport {
    let mut app = NativeEditorApplication::new(NativeEditorWindowConfig::default());
    let mut report = RealWindowInteractionSmokeRunner {
        backend: "winit-wgpu-controlled-window-event-bridge".to_string(),
    }
    .run(&mut app, scenario.clone());
    let outcome = crate::run_real_native_editor_capture_once(
        scenario.width,
        scenario.height,
        crate::EditorReachabilityReportLevel::Summary,
    );
    report.backend = outcome.window_report.backend.clone();
    report.window_created = outcome.window_report.window_created;
    report.surface_created = outcome.window_report.surface_created;
    report.surface_configured = outcome.window_report.surface_configured;
    report.present_status = outcome.window_report.present_status.clone();
    report.frame_count = outcome.window_report.frame_index;
    report.draw_command_count = outcome.window_report.draw_command_count;
    report.hit_region_count = outcome.window_report.hit_region_count;
    report
        .diagnostics
        .extend(merge_real_window_report_diagnostics(&outcome.window_report));
    match outcome.capture {
        Some(capture) => {
            report.screenshot = Some(RealWindowScreenshotEvidence {
                kind: "actual_window_rgba".to_string(),
                width: capture.width,
                height: capture.height,
                frame_index: outcome.window_report.frame_index,
                artifact_path: None,
                rgba_hash: Some(engine_runtime::canonical_digest::sha256_prefixed(
                    &capture.rgba8,
                )),
            });
        }
        None => {
            report.status = RealWindowInteractionSmokeStatus::Failed;
            report.diagnostics.push(error(
                "real_window_interaction.actual_capture_missing",
                outcome
                    .capture_error
                    .unwrap_or_else(|| "No actual RGBA capture was produced.".to_string()),
                "editor_wgpu_renderer.capture",
            ));
        }
    }
    if outcome.window_report.present_status != "presented" {
        report.status = RealWindowInteractionSmokeStatus::Failed;
    }
    report
}

#[cfg(not(feature = "real-window"))]
pub fn run_real_window_interaction_smoke_local_only(
    _scenario: RealWindowInteractionSmokeScenario,
) -> RealWindowInteractionSmokeReport {
    RealWindowInteractionSmokeReport::skipped(
        "real-window-feature",
        "real_window_feature_not_enabled",
        "Re-run with the real-window feature for local-only native window smoke.",
    )
}

pub fn merge_real_window_report_diagnostics(
    report: &RealNativeEditorWindowReport,
) -> Vec<RealWindowInteractionDiagnostic> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| RealWindowInteractionDiagnostic {
            severity: match diagnostic.severity {
                RealNativeEditorWindowDiagnosticSeverity::Info => {
                    RealWindowInteractionDiagnosticSeverity::Info
                }
                RealNativeEditorWindowDiagnosticSeverity::Warning => {
                    RealWindowInteractionDiagnosticSeverity::Warning
                }
                RealNativeEditorWindowDiagnosticSeverity::Error => {
                    RealWindowInteractionDiagnosticSeverity::Error
                }
            },
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            source_stage: diagnostic.source_stage.clone(),
        })
        .collect()
}

fn error(
    code: impl Into<String>,
    message: impl Into<String>,
    source_stage: impl Into<String>,
) -> RealWindowInteractionDiagnostic {
    RealWindowInteractionDiagnostic {
        severity: RealWindowInteractionDiagnosticSeverity::Error,
        code: code.into(),
        message: message.into(),
        source_stage: source_stage.into(),
    }
}

impl From<RealWindowInteractionDiagnostic> for RealNativeEditorWindowDiagnostic {
    fn from(value: RealWindowInteractionDiagnostic) -> Self {
        Self {
            severity: match value.severity {
                RealWindowInteractionDiagnosticSeverity::Info => {
                    RealNativeEditorWindowDiagnosticSeverity::Info
                }
                RealWindowInteractionDiagnosticSeverity::Warning => {
                    RealNativeEditorWindowDiagnosticSeverity::Warning
                }
                RealWindowInteractionDiagnosticSeverity::Error => {
                    RealNativeEditorWindowDiagnosticSeverity::Error
                }
            },
            code: value.code,
            message: value.message,
            source_stage: value.source_stage,
        }
    }
}
