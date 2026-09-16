use crate::{
    run_complex_shooter_gameplay_rule_runtime_report,
    run_complex_shooter_project_rule_driven_ui_state_report,
    run_complex_shooter_real_texture_present_report, ComplexShooterGameplayRuleRuntimeRequest,
    ComplexShooterGameplayRuleRuntimeStatus, ComplexShooterProjectRuleDrivenUiStateRequest,
    ComplexShooterProjectRuleDrivenUiStateStatus, ComplexShooterRealTexturePresentRequest,
    ComplexShooterRealTexturePresentStatus,
};
use editor_core::{DesktopExportPipeline, DesktopExportRequest, ExplicitExportOutput};
use engine_runtime::aui::AuiSnapshotSource;
use engine_runtime::windowed_player::WindowedPlayerRunReport;
use runtime_cli::{
    verify_exported_player_process, ExportedPlayerProcessVerificationReport,
    ExportedPlayerProcessVerificationRequest,
};
use runtime_player_winit::NativePlayerWindowRunRequest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_REPORT_SCHEMA_VERSION: &str =
    "complex-shooter-exported-windows-playable-golden-gate-report.v1";
pub const COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_SCENARIO_ID: &str =
    "complex-shooter-exported-windows-playable-golden-gate-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterExportedWindowsPlayableGoldenStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterGoldenEvidenceStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexShooterRealWindowEvidenceStatus {
    Captured,
    FeatureDisabled,
    EnvironmentBlocked,
    LocalOnlySkipped,
}

impl ComplexShooterRealWindowEvidenceStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Captured => "captured",
            Self::FeatureDisabled => "feature_disabled",
            Self::EnvironmentBlocked => "environment_blocked",
            Self::LocalOnlySkipped => "local_only_skipped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenPackageEvidence {
    pub exported_package_dir: String,
    pub game_exe_path: String,
    pub package_manifest_path: String,
    pub runtime_package_path: String,
    pub desktop_export_report_path: String,
    pub target_os: String,
    pub actual_host_os: String,
    pub executable_name: String,
    pub export_status: String,
    pub runtime_package_status: String,
    pub desktop_player_exit_code: Option<i32>,
    pub desktop_player_exit_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenProcessEvidence {
    pub exported_process_contract_used: bool,
    pub verifier_status: String,
    pub mode: String,
    pub requested_frames: u64,
    pub process_exit_code: Option<i32>,
    pub process_exit_reason: String,
    pub child_player_exit_code: Option<i32>,
    pub child_present_status: Option<String>,
    pub child_frames_completed: Option<u64>,
    pub child_report_path: String,
    pub verifier_report_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenEvidenceSummary {
    pub texture_status: ComplexShooterGoldenEvidenceStatus,
    pub gameplay_status: ComplexShooterGoldenEvidenceStatus,
    pub hud_status: ComplexShooterGoldenEvidenceStatus,
    pub aui_status: ComplexShooterGoldenEvidenceStatus,
    pub render_status: ComplexShooterGoldenEvidenceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenTextureEvidence {
    pub source_domain: String,
    pub source_report_path: Option<String>,
    pub loaded_texture_count: usize,
    pub uploaded_texture_count: usize,
    pub sprite_texture_binding_ready: bool,
    pub fallback_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenGameplayEvidence {
    pub source_domain: String,
    pub source_report_path: Option<String>,
    pub input_source: String,
    pub input_script_id: String,
    pub fire_action_observed: bool,
    pub projectile_spawn_observed: bool,
    pub movement_observed: bool,
    pub fire_observed: bool,
    pub collision_pair_count: usize,
    pub score_after: Option<i64>,
    pub score_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenHudEvidence {
    pub source_domain: String,
    pub source_report_path: Option<String>,
    pub snapshot_source: String,
    pub producer_id: Option<String>,
    pub active_binding_paths: Vec<String>,
    pub produced_paths: Vec<String>,
    pub missing_paths: Vec<String>,
    pub score_text: Option<String>,
    pub score_text_matches_score_after: bool,
    pub rendered_glyph_count: usize,
    pub aui_draw_item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenRealWindowEvidence {
    pub status: ComplexShooterRealWindowEvidenceStatus,
    pub blocking: bool,
    pub screenshot_requested: bool,
    pub screenshot_status: Option<String>,
    pub screenshot_path: Option<String>,
    pub environment_diagnostic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenReportMode {
    pub runtime_report_level: String,
    pub editor_report_level: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenArtifact {
    pub artifact_id: String,
    pub path: String,
    pub source_domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterGoldenDiagnostic {
    pub severity: String,
    pub code: String,
    pub domain: String,
    pub stage: String,
    pub source_path: Option<String>,
    pub message: String,
    pub next_action: Option<String>,
}

impl ComplexShooterGoldenDiagnostic {
    fn error(
        code: impl Into<String>,
        domain: impl Into<String>,
        stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            domain: domain.into(),
            stage: stage.into(),
            source_path: None,
            message: message.into(),
            next_action: None,
        }
    }

    fn warning(
        code: impl Into<String>,
        domain: impl Into<String>,
        stage: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: "warning".to_string(),
            code: code.into(),
            domain: domain.into(),
            stage: stage.into(),
            source_path: None,
            message: message.into(),
            next_action: None,
        }
    }

    fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    fn with_next_action(mut self, next_action: impl Into<String>) -> Self {
        self.next_action = Some(next_action.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexShooterExportedWindowsPlayableGoldenReport {
    pub schema_version: String,
    pub scenario_id: String,
    pub status: ComplexShooterExportedWindowsPlayableGoldenStatus,
    pub project_root: String,
    pub output_root: String,
    pub package: ComplexShooterGoldenPackageEvidence,
    pub process: ComplexShooterGoldenProcessEvidence,
    pub golden_evidence: ComplexShooterGoldenEvidenceSummary,
    pub texture_evidence: ComplexShooterGoldenTextureEvidence,
    pub gameplay_evidence: ComplexShooterGoldenGameplayEvidence,
    pub hud_evidence: ComplexShooterGoldenHudEvidence,
    pub real_window_evidence: ComplexShooterGoldenRealWindowEvidence,
    pub report_mode: ComplexShooterGoldenReportMode,
    pub artifacts: Vec<ComplexShooterGoldenArtifact>,
    pub diagnostics: Vec<ComplexShooterGoldenDiagnostic>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexShooterExportedWindowsPlayableGoldenRequest {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub frames: u64,
    pub include_optional_real_window_step: bool,
}

impl ComplexShooterExportedWindowsPlayableGoldenRequest {
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            output_root: output_root.into(),
            frames: 6,
            include_optional_real_window_step: false,
        }
    }
}

pub fn run_complex_shooter_exported_windows_playable_golden_report(
    request: ComplexShooterExportedWindowsPlayableGoldenRequest,
) -> ComplexShooterExportedWindowsPlayableGoldenReport {
    let _ = fs::create_dir_all(request.output_root.join("reports"));

    let mut export_request = DesktopExportRequest::windows_dev(&request.project_root)
        .with_explicit_output(ExplicitExportOutput::from_user_selected(
            &request.output_root,
        ));
    export_request.output_root = request.output_root.join("Build").join("Windows");
    export_request.frame_limit = request.frames.max(1);
    let export_report = DesktopExportPipeline::export(export_request);
    let exported_package_dir = PathBuf::from(&export_report.package_dir);
    let runtime_package_path = PathBuf::from(&export_report.runtime_package_dir);
    let desktop_export_report_path = exported_package_dir
        .join("reports")
        .join("desktop-export-report.json");
    let verifier_report_path = request
        .output_root
        .join("reports")
        .join("exported-player-process-verification-report.json");
    let verification = verify_exported_player_process(ExportedPlayerProcessVerificationRequest {
        exported_package_dir: exported_package_dir.clone(),
        mode: "headless-gate".to_string(),
        frame_limit: request.frames.max(1),
        report_path: Some(verifier_report_path.clone()),
        timeout_ms: 30_000,
        screenshot: false,
        screenshot_path: None,
    });
    let child_report = read_child_player_report(&verification.child_report_path);

    let native_report = if runtime_package_path.join("manifest.json").exists() {
        let mut native_request =
            NativePlayerWindowRunRequest::headless_surface_gate(&runtime_package_path);
        native_request.frame_limit = request.frames.max(1);
        let linked_modules = crate::complex_shooter_linked_project_runtimes();
        Some(
            runtime_player_winit::run_headless_native_player_from_package_with_linked_modules(
                native_request,
                linked_modules.as_ref(),
            ),
        )
    } else {
        None
    };

    let texture_report = run_complex_shooter_real_texture_present_report(
        ComplexShooterRealTexturePresentRequest::new(
            &request.project_root,
            request
                .output_root
                .join("subreports")
                .join("real-texture-present"),
        ),
    );
    let gameplay_report = run_complex_shooter_gameplay_rule_runtime_report(
        ComplexShooterGameplayRuleRuntimeRequest::new(
            &request.project_root,
            request
                .output_root
                .join("subreports")
                .join("gameplay-rule-runtime"),
        ),
    );
    let ui_state_report = run_complex_shooter_project_rule_driven_ui_state_report(
        ComplexShooterProjectRuleDrivenUiStateRequest::new(
            &request.project_root,
            request
                .output_root
                .join("subreports")
                .join("project-rule-driven-ui-state"),
        ),
    );

    let mut report = ComplexShooterExportedWindowsPlayableGoldenReport {
        schema_version: COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_REPORT_SCHEMA_VERSION
            .to_string(),
        scenario_id: COMPLEX_SHOOTER_EXPORTED_WINDOWS_PLAYABLE_GOLDEN_SCENARIO_ID.to_string(),
        status: ComplexShooterExportedWindowsPlayableGoldenStatus::Failed,
        project_root: request.project_root.display().to_string(),
        output_root: request.output_root.display().to_string(),
        package: ComplexShooterGoldenPackageEvidence {
            exported_package_dir: export_report.package_dir.clone(),
            game_exe_path: verification.game_exe_path.clone(),
            package_manifest_path: export_report.package_manifest_path.clone(),
            runtime_package_path: export_report.runtime_package_dir.clone(),
            desktop_export_report_path: desktop_export_report_path.display().to_string(),
            target_os: "windows".to_string(),
            actual_host_os: std::env::consts::OS.to_string(),
            executable_name: Path::new(&verification.game_exe_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(if cfg!(windows) { "Game.exe" } else { "Game" })
                .to_string(),
            export_status: status_name(&export_report.status),
            runtime_package_status: format!("{:?}", export_report.runtime_package_status)
                .to_ascii_lowercase(),
            desktop_player_exit_code: export_report.player_exit_code,
            desktop_player_exit_reason: export_report.player_exit_reason.clone(),
        },
        process: ComplexShooterGoldenProcessEvidence {
            exported_process_contract_used: true,
            verifier_status: status_name(&verification.status),
            mode: verification.mode.clone(),
            requested_frames: verification.frame_limit,
            process_exit_code: verification.process_exit_code,
            process_exit_reason: verification.process_exit_reason.clone(),
            child_player_exit_code: verification.child_player_exit_code,
            child_present_status: verification.child_present_status.clone(),
            child_frames_completed: verification.child_frames_completed,
            child_report_path: verification.child_report_path.clone(),
            verifier_report_path: verifier_report_path.display().to_string(),
        },
        golden_evidence: ComplexShooterGoldenEvidenceSummary {
            texture_status: if texture_report.status
                == ComplexShooterRealTexturePresentStatus::Passed
            {
                ComplexShooterGoldenEvidenceStatus::Passed
            } else {
                ComplexShooterGoldenEvidenceStatus::Failed
            },
            gameplay_status: if gameplay_report.status
                == ComplexShooterGameplayRuleRuntimeStatus::Passed
            {
                ComplexShooterGoldenEvidenceStatus::Passed
            } else {
                ComplexShooterGoldenEvidenceStatus::Failed
            },
            hud_status: if ui_state_report.status
                == ComplexShooterProjectRuleDrivenUiStateStatus::Passed
            {
                ComplexShooterGoldenEvidenceStatus::Passed
            } else {
                ComplexShooterGoldenEvidenceStatus::Failed
            },
            aui_status: ComplexShooterGoldenEvidenceStatus::Failed,
            render_status: ComplexShooterGoldenEvidenceStatus::Failed,
        },
        texture_evidence: ComplexShooterGoldenTextureEvidence {
            source_domain: "project_e2e.real_texture_present".to_string(),
            source_report_path: find_report_artifact(
                &texture_report.artifacts,
                "complex-shooter-real-texture-present-report.json",
            ),
            loaded_texture_count: texture_report.metrics.loaded_texture_payload_count,
            uploaded_texture_count: texture_report
                .metrics
                .rhi_non_fallback_texture_command_count,
            sprite_texture_binding_ready: texture_report.metrics.sprite_texture_binding_ready,
            fallback_count: texture_report
                .metrics
                .sprite_draw_command_count
                .saturating_sub(texture_report.metrics.non_fallback_sprite_draw_count),
        },
        gameplay_evidence: ComplexShooterGoldenGameplayEvidence {
            source_domain: "project_e2e.gameplay_rule_runtime".to_string(),
            source_report_path: find_report_artifact(
                &gameplay_report.artifacts,
                "complex-shooter-gameplay-rule-runtime-execution-report.json",
            ),
            input_source: "deterministic_action_snapshot".to_string(),
            input_script_id: "complex_shooter_p0_229_default_fire_and_move".to_string(),
            fire_action_observed: gameplay_report.metrics.fire_command_enqueue_count > 0,
            projectile_spawn_observed: gameplay_report.metrics.bullet_prefab_apply_count > 0,
            movement_observed: gameplay_report.metrics.player_move_write_count > 0,
            fire_observed: gameplay_report.metrics.fire_command_enqueue_count > 0,
            collision_pair_count: gameplay_report.metrics.collision_pair_count,
            score_after: gameplay_report.metrics.score_after,
            score_changed: gameplay_report.metrics.score_changed,
        },
        hud_evidence: hud_evidence_from_reports(&ui_state_report, native_report.as_ref()),
        real_window_evidence: real_window_evidence_from_request(&request, &verification),
        report_mode: ComplexShooterGoldenReportMode {
            runtime_report_level: "summary".to_string(),
            editor_report_level: "summary".to_string(),
        },
        artifacts: Vec::new(),
        diagnostics: Vec::new(),
        next_actions: Vec::new(),
    };

    if child_report.as_ref().is_some_and(|child| {
        child
            .renderer_summary
            .as_ref()
            .is_some_and(|summary| summary.rhi_command_count > 0)
            || child
                .renderer_summary
                .as_ref()
                .is_some_and(|summary| summary.draw_item_count > 0)
    }) || native_report
        .as_ref()
        .is_some_and(|native| native.rhi_command_count > 0)
    {
        report.golden_evidence.render_status = ComplexShooterGoldenEvidenceStatus::Passed;
    }
    if report.hud_evidence.rendered_glyph_count > 0
        && report.hud_evidence.aui_draw_item_count > 0
        && report.hud_evidence.snapshot_source == "project_producer"
    {
        report.golden_evidence.aui_status = ComplexShooterGoldenEvidenceStatus::Passed;
    }

    push_artifact(
        &mut report,
        "desktop-export-report",
        desktop_export_report_path,
        "editor_core.desktop_export",
    );
    push_artifact(
        &mut report,
        "exported-player-process-verification-report",
        verifier_report_path,
        "runtime_cli.exported_player_verification",
    );
    push_artifact(
        &mut report,
        "child-windowed-player-run-report",
        PathBuf::from(&verification.child_report_path),
        "exported_process.child_report",
    );
    for (artifact_id, source_domain, path) in [
        (
            "real-texture-present-report",
            "project_e2e.real_texture_present",
            report.texture_evidence.source_report_path.clone(),
        ),
        (
            "gameplay-rule-runtime-report",
            "project_e2e.gameplay_rule_runtime",
            report.gameplay_evidence.source_report_path.clone(),
        ),
        (
            "project-rule-driven-ui-state-report",
            "project_e2e.project_rule_driven_ui_state",
            report.hud_evidence.source_report_path.clone(),
        ),
    ] {
        if let Some(path) = path {
            push_artifact(&mut report, artifact_id, PathBuf::from(path), source_domain);
        }
    }

    absorb_diagnostics(
        &mut report,
        &export_report.diagnostics,
        "editor_core.desktop_export",
        "desktop-export",
    );
    absorb_verification_diagnostics(&mut report, &verification);
    absorb_subreport_diagnostics(
        &mut report,
        &texture_report.diagnostics,
        "project_e2e.real_texture_present",
        "texture-evidence",
    );
    absorb_subreport_diagnostics(
        &mut report,
        &gameplay_report.diagnostics,
        "project_e2e.gameplay_rule_runtime",
        "gameplay-evidence",
    );
    absorb_subreport_diagnostics(
        &mut report,
        &ui_state_report.diagnostics,
        "project_e2e.project_rule_driven_ui_state",
        "hud-evidence",
    );

    finalize_report(request.output_root.as_path(), report)
}

fn hud_evidence_from_reports(
    ui_state_report: &crate::ComplexShooterProjectRuleDrivenUiStateReport,
    native_report: Option<&runtime_player_winit::NativeWindowHostReport>,
) -> ComplexShooterGoldenHudEvidence {
    let snapshot_report = ui_state_report.ui_state_snapshot_report.as_ref();
    let native_aui = native_report.map(|report| &report.aui);
    ComplexShooterGoldenHudEvidence {
        source_domain: "project_e2e.project_rule_driven_ui_state+runtime_player_winit.headless_exported_package"
            .to_string(),
        source_report_path: find_report_artifact(
            &ui_state_report.artifacts,
            "complex-shooter-project-rule-driven-ui-state-snapshot-report.json",
        ),
        snapshot_source: snapshot_report
            .map(|report| snapshot_source_id(report.snapshot_source).to_string())
            .or_else(|| native_aui.map(|aui| aui.snapshot_source.clone()))
            .unwrap_or_else(|| "not_reported".to_string()),
        producer_id: snapshot_report
            .map(|report| report.producer_id.clone())
            .or_else(|| native_aui.and_then(|aui| aui.producer_id.clone())),
        active_binding_paths: snapshot_report
            .map(|report| report.active_binding_paths.clone())
            .or_else(|| native_aui.map(|aui| aui.active_binding_paths.clone()))
            .unwrap_or_default(),
        produced_paths: snapshot_report
            .map(|report| report.produced_paths.clone())
            .or_else(|| native_aui.map(|aui| aui.produced_paths.clone()))
            .unwrap_or_default(),
        missing_paths: snapshot_report
            .map(|report| report.missing_paths.clone())
            .or_else(|| native_aui.map(|aui| aui.missing_paths.clone()))
            .unwrap_or_default(),
        score_text: ui_state_report.metrics.score_text.clone(),
        score_text_matches_score_after: ui_state_report
            .metrics
            .score_text_matches_runtime_score,
        rendered_glyph_count: native_aui
            .map(|aui| aui.rendered_glyph_count)
            .unwrap_or_default(),
        aui_draw_item_count: native_aui.map(|aui| aui.draw_item_count).unwrap_or_default(),
    }
}

fn real_window_evidence_from_request(
    request: &ComplexShooterExportedWindowsPlayableGoldenRequest,
    verification: &ExportedPlayerProcessVerificationReport,
) -> ComplexShooterGoldenRealWindowEvidence {
    let status = if verification.screenshot_status.as_deref() == Some("captured") {
        ComplexShooterRealWindowEvidenceStatus::Captured
    } else if request.include_optional_real_window_step {
        ComplexShooterRealWindowEvidenceStatus::FeatureDisabled
    } else {
        ComplexShooterRealWindowEvidenceStatus::LocalOnlySkipped
    };
    ComplexShooterGoldenRealWindowEvidence {
        status,
        blocking: false,
        screenshot_requested: verification.screenshot_requested,
        screenshot_status: verification.screenshot_status.clone(),
        screenshot_path: verification.screenshot_path.clone(),
        environment_diagnostic: format!(
            "{}; optional real-window evidence is non-blocking for P0-4 B-min",
            status.as_str()
        ),
    }
}

fn finalize_report(
    output_root: &Path,
    mut report: ComplexShooterExportedWindowsPlayableGoldenReport,
) -> ComplexShooterExportedWindowsPlayableGoldenReport {
    let blocking_passed = report.package.export_status == "success"
        && report.process.exported_process_contract_used
        && report.process.verifier_status == "passed"
        && report.process.child_player_exit_code == Some(0)
        && report
            .process
            .child_frames_completed
            .is_some_and(|frames| frames >= report.process.requested_frames)
        && report.golden_evidence.texture_status == ComplexShooterGoldenEvidenceStatus::Passed
        && report.golden_evidence.gameplay_status == ComplexShooterGoldenEvidenceStatus::Passed
        && report.golden_evidence.hud_status == ComplexShooterGoldenEvidenceStatus::Passed
        && report.golden_evidence.aui_status == ComplexShooterGoldenEvidenceStatus::Passed
        && report.golden_evidence.render_status == ComplexShooterGoldenEvidenceStatus::Passed
        && report.texture_evidence.loaded_texture_count > 0
        && report.texture_evidence.uploaded_texture_count > 0
        && report.texture_evidence.sprite_texture_binding_ready
        && report.texture_evidence.fallback_count == 0
        && report.gameplay_evidence.fire_action_observed
        && report.gameplay_evidence.projectile_spawn_observed
        && report.gameplay_evidence.movement_observed
        && report.gameplay_evidence.collision_pair_count > 0
        && report.gameplay_evidence.score_changed
        && report.hud_evidence.score_text_matches_score_after
        && report.hud_evidence.rendered_glyph_count > 0
        && report.hud_evidence.missing_paths.is_empty();

    if !blocking_passed {
        report
            .next_actions
            .push("inspect_complex_shooter_exported_windows_playable_golden_report".to_string());
        if report.process.verifier_status != "passed" {
            report
                .next_actions
                .push("fix_exported_game_exe_process_contract".to_string());
        }
        if report.golden_evidence.texture_status != ComplexShooterGoldenEvidenceStatus::Passed {
            report
                .next_actions
                .push("fix_228_texture_evidence".to_string());
        }
        if report.golden_evidence.gameplay_status != ComplexShooterGoldenEvidenceStatus::Passed {
            report
                .next_actions
                .push("fix_229_gameplay_rule_runtime_evidence".to_string());
        }
        if report.golden_evidence.hud_status != ComplexShooterGoldenEvidenceStatus::Passed
            || report.golden_evidence.aui_status != ComplexShooterGoldenEvidenceStatus::Passed
        {
            report
                .next_actions
                .push("fix_230_hud_or_aui_evidence".to_string());
        }
    }

    if report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
        || !blocking_passed
    {
        report.status = ComplexShooterExportedWindowsPlayableGoldenStatus::Failed;
    } else {
        report.status = ComplexShooterExportedWindowsPlayableGoldenStatus::Passed;
    }
    if matches!(
        report.real_window_evidence.status,
        ComplexShooterRealWindowEvidenceStatus::LocalOnlySkipped
            | ComplexShooterRealWindowEvidenceStatus::FeatureDisabled
            | ComplexShooterRealWindowEvidenceStatus::EnvironmentBlocked
    ) && !report.real_window_evidence.blocking
        && report.status == ComplexShooterExportedWindowsPlayableGoldenStatus::Passed
    {
        report
            .next_actions
            .retain(|action| action != "run_real_window_smoke");
    }

    let report_path = output_root
        .join("reports")
        .join("complex-shooter-exported-windows-playable-golden-gate-report.json");
    push_artifact(
        &mut report,
        "complex-shooter-exported-windows-playable-golden-gate-report",
        report_path.clone(),
        "project_e2e.exported_windows_playable_golden",
    );
    if let Err(error) = write_json(&report_path, &report) {
        report.diagnostics.push(
            ComplexShooterGoldenDiagnostic::error(
                "report_write_failed",
                "project_e2e.exported_windows_playable_golden",
                "finalize",
                format!("failed to write P0-4 report: {error}"),
            )
            .with_source_path(report_path.display().to_string()),
        );
        report.status = ComplexShooterExportedWindowsPlayableGoldenStatus::Failed;
    }
    report
}

fn read_child_player_report(path: &str) -> Option<WindowedPlayerRunReport> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<WindowedPlayerRunReport>(&text).ok())
}

fn find_report_artifact(artifacts: &[String], suffix: &str) -> Option<String> {
    artifacts
        .iter()
        .find(|path| path.ends_with(suffix))
        .cloned()
}

fn push_artifact(
    report: &mut ComplexShooterExportedWindowsPlayableGoldenReport,
    artifact_id: impl Into<String>,
    path: PathBuf,
    source_domain: impl Into<String>,
) {
    report.artifacts.push(ComplexShooterGoldenArtifact {
        artifact_id: artifact_id.into(),
        path: path.display().to_string(),
        source_domain: source_domain.into(),
    });
}

fn absorb_diagnostics(
    report: &mut ComplexShooterExportedWindowsPlayableGoldenReport,
    diagnostics: &[editor_core::DesktopExportDiagnostic],
    domain: &str,
    stage: &str,
) {
    for diagnostic in diagnostics {
        let mut item = ComplexShooterGoldenDiagnostic {
            severity: format!("{:?}", diagnostic.severity).to_ascii_lowercase(),
            code: diagnostic.code.clone(),
            domain: domain.to_string(),
            stage: stage.to_string(),
            source_path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
            next_action: diagnostic.suggestion.clone(),
        };
        if item.severity == "warning" && item.code == "PlayerExecutableMissing" {
            item.next_action =
                Some("cargo test -p runtime_cli exported_player_process_verification".to_string());
        }
        report.diagnostics.push(item);
    }
}

fn absorb_verification_diagnostics(
    report: &mut ComplexShooterExportedWindowsPlayableGoldenReport,
    verification: &ExportedPlayerProcessVerificationReport,
) {
    for diagnostic in &verification.diagnostics {
        report.diagnostics.push(ComplexShooterGoldenDiagnostic {
            severity: diagnostic.severity.clone(),
            code: diagnostic.code.clone(),
            domain: "runtime_cli.exported_player_verification".to_string(),
            stage: "exported-process".to_string(),
            source_path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
            next_action: Some("inspect_exported_player_process_verification_report".to_string()),
        });
    }
}

fn absorb_subreport_diagnostics(
    report: &mut ComplexShooterExportedWindowsPlayableGoldenReport,
    diagnostics: &[String],
    domain: &str,
    stage: &str,
) {
    for diagnostic in diagnostics {
        if diagnostic.starts_with("fail:") {
            report.diagnostics.push(
                ComplexShooterGoldenDiagnostic::error(
                    diagnostic
                        .split(':')
                        .nth(1)
                        .unwrap_or("subreport_failed")
                        .to_string(),
                    domain,
                    stage,
                    diagnostic.clone(),
                )
                .with_next_action(format!("inspect_{domain}_report")),
            );
        } else {
            report
                .diagnostics
                .push(ComplexShooterGoldenDiagnostic::warning(
                    "subreport_diagnostic",
                    domain,
                    stage,
                    diagnostic.clone(),
                ));
        }
    }
}

fn status_name(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
        .replace("EnvironmentBlocked", "EnvironmentBlocked")
        .to_ascii_lowercase()
        .replace('_', "-")
}

fn snapshot_source_id(source: AuiSnapshotSource) -> &'static str {
    match source {
        AuiSnapshotSource::EmptyDefaultSnapshot => "empty_default_snapshot",
        AuiSnapshotSource::PackageSmokeSnapshot => "package_smoke_snapshot",
        AuiSnapshotSource::ProjectProducer => "project_producer",
        AuiSnapshotSource::TestSnapshot => "test_snapshot",
        AuiSnapshotSource::ProjectRuleSnapshot => "project_rule_snapshot",
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}
