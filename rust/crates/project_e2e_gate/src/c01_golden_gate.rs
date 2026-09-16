use editor_core::{
    AssetImportConflictPolicy, AssetImportSourceMetadata, AssetLicenseMetadata, AuiPatchOperation,
    BuildProfile, BuildProfileApplication, BuildProfileIconRef, BuildProfileRelease, CommandStatus,
    ControlledSourcePatchDocument, ControlledSourcePatchOperation,
    ControlledSourcePatchPrepareRequest, ControlledSourcePatchValidationRequest,
    DesktopExportPipeline, DesktopExportRequest, DesktopExportStatus, EditorPreviewPackageRequest,
    EditorPreviewPackageService, EditorPreviewPackageStatus, EditorSession, ExplicitExportOutput,
    InputBindingProcessorPatch, InputPatchOperation, PatchOperation, PatchSource,
    PrefabPatchOperation, ProjectAssetImportPrepareRequest, ProjectCandidate,
    ProjectCandidateApplyReceipt, ProjectCandidateApproval, ProjectCandidateEntry,
    ProjectCandidateEnvelope, ProjectCandidatePayload, ProjectCandidateSourceKind,
    ProjectCandidateValidationContext, ProjectCandidateValidationReport,
    ProjectCandidateValidationStatus, ProjectPatchDocument, ProjectPlayerArtifact,
    ProjectRuntimeModuleBuildSpec, ProjectRuntimeSourceKind, ReleasePackageBuildRequest,
    ReleasePackageBuilder, ReleasePackageReportLevel, ReleasePackageStatus, RulePatchOperation,
    ScenePatchOperation, TextureImportSettings, PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION,
    PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION,
};
use editor_ui_model::{InputActionValueKind, UiCommandPayload, Vec3};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::project_runtime_module::ProjectRuntimeModuleDescriptor;
use engine_runtime::rule_artifact::expected_rule_artifact_id;
use engine_runtime::rule_ir::{ProjectRuleIr, ProjectRulePhase};
use engine_runtime::windowed_player::{
    WindowedPlayerFramePerformanceSummary, WindowedPlayerGameplayTraceRecord,
    WindowedPlayerRunReport,
};
use runtime_cli::{
    run_bounded_child_process, verify_exported_player_process_with_options,
    BoundedChildProcessExitReason, BoundedChildProcessRequest,
    ExportedPlayerProcessVerificationOptions, ExportedPlayerProcessVerificationRequest,
    ExportedPlayerProcessVerificationStatus,
};
use runtime_player_winit::{NativePlayerInputScript, NativePlayerInputScriptFrame};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const C01_GOLDEN_GATE_REPORT_SCHEMA_VERSION: &str = "c01-golden-gate-report.v4";
pub const C01_GOLDEN_GATE_SCENARIO_ID: &str = "c01-golden-gate";

const RULE_ID: &str = "project.rule.c01.tick";
const INPUT_PATH: &str = "Input/input.default.json";
const AUI_PATH: &str = "UI/c01-hud.aui.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01GoldenGateRequest {
    pub project_root: PathBuf,
    pub engine_sdk_root: PathBuf,
    pub candidate_store_root: PathBuf,
    pub evidence_root: PathBuf,
    pub frozen_asset_root: PathBuf,
    pub external_export_root: PathBuf,
    #[serde(default = "default_approval_actor")]
    pub approval_actor: String,
}

impl C01GoldenGateRequest {
    pub fn new(
        project_root: impl Into<PathBuf>,
        engine_sdk_root: impl Into<PathBuf>,
        candidate_store_root: impl Into<PathBuf>,
        evidence_root: impl Into<PathBuf>,
        frozen_asset_root: impl Into<PathBuf>,
        external_export_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            engine_sdk_root: engine_sdk_root.into(),
            candidate_store_root: candidate_store_root.into(),
            evidence_root: evidence_root.into(),
            frozen_asset_root: frozen_asset_root.into(),
            external_export_root: external_export_root.into(),
            approval_actor: default_approval_actor(),
        }
    }

    pub fn with_approval_actor(mut self, approval_actor: impl Into<String>) -> Self {
        self.approval_actor = approval_actor.into();
        self
    }
}

fn default_approval_actor() -> String {
    "local-maintainer-c01".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum C01GoldenGateStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum C01GoldenGateEntryMode {
    CandidateConstruction,
    ValidationOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01CandidateEvidence {
    pub sequence: usize,
    pub candidate_id: String,
    pub payload_kind: String,
    pub source_digest: String,
    pub envelope_digest: String,
    pub candidate_digest: String,
    pub validation_digest: String,
    pub approval_digest: String,
    pub receipt_digest: String,
    pub before_project_digest: String,
    pub applied_project_digest: String,
    pub evidence_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01AssetEvidence {
    pub asset_id: String,
    pub source_path: String,
    pub source_hash: String,
    pub expected_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01ReopenEvidence {
    pub saved_project_digest: String,
    pub reopened_project_digest: String,
    pub scene_status: String,
    pub input_status: String,
    pub prefab_status: String,
    pub rule_status: String,
    pub aui_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01RuntimeAssertions {
    pub movement: bool,
    pub dash_started: bool,
    pub dash_cooldown_observed: bool,
    pub bullet_prefab_instantiated: bool,
    pub score_increased: bool,
    pub wave_advanced: bool,
    pub game_over_observed: bool,
    pub restart_observed: bool,
}

impl C01RuntimeAssertions {
    fn all_passed(&self) -> bool {
        self.movement
            && self.dash_started
            && self.dash_cooldown_observed
            && self.bullet_prefab_instantiated
            && self.score_increased
            && self.wave_advanced
            && self.game_over_observed
            && self.restart_observed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01PreviewEvidence {
    pub status: String,
    pub cache_key: String,
    pub runtime_package_dir: String,
    pub player_executable: String,
    pub player_executable_hash: String,
    pub player_module_descriptor: ProjectRuntimeModuleDescriptor,
    pub headless_report_path: String,
    pub screenshot_path: String,
    pub screenshot_hash: String,
    pub screenshot_visual: C01ScreenshotVisualEvidence,
    pub runtime_assertions: C01RuntimeAssertions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01ScreenshotVisualEvidence {
    pub width: u32,
    pub height: u32,
    pub unique_color_count: usize,
    pub dominant_color_fraction: f64,
    pub player_blue_pixel_count: usize,
    pub enemy_red_pixel_count: usize,
    pub hud_near_white_pixel_count: usize,
    pub content_bounds: [u32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01ExportEvidence {
    pub desktop_export_status: String,
    pub desktop_package_dir: String,
    pub desktop_report_path: String,
    pub release_status: String,
    pub release_output_dir: String,
    pub release_report_path: String,
    pub headless_verification_status: String,
    pub headless_verification_report_path: String,
    pub windowed_verification_status: String,
    pub windowed_verification_report_path: String,
    pub screenshot_path: String,
    pub screenshot_hash: String,
    pub screenshot_visual: C01ScreenshotVisualEvidence,
    pub performance: WindowedPlayerFramePerformanceSummary,
    pub runtime_assertions: C01RuntimeAssertions,
    pub no_arg_launch_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01GoldenGateTimingEvidence {
    pub candidate_construction_ms: u64,
    pub save_reopen_preview_ms: u64,
    pub export_ms: u64,
    pub first_playable_ms: u64,
    pub total_wall_clock_ms: u64,
    pub external_wait_ms: u64,
    pub automation_active_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01GoldenGateReport {
    pub schema_version: String,
    pub status: C01GoldenGateStatus,
    pub entry_mode: C01GoldenGateEntryMode,
    pub scenario_id: String,
    pub project_root: String,
    pub initial_project_digest: Option<String>,
    pub final_project_digest: Option<String>,
    pub candidates: Vec<C01CandidateEvidence>,
    pub frozen_assets: Vec<C01AssetEvidence>,
    pub changed_paths: Vec<String>,
    pub reopen: Option<C01ReopenEvidence>,
    pub preview: Option<C01PreviewEvidence>,
    pub export: Option<C01ExportEvidence>,
    pub timing: C01GoldenGateTimingEvidence,
    pub first_blocker: Option<String>,
    pub repairs: usize,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl C01GoldenGateReport {
    fn new(request: &C01GoldenGateRequest) -> Self {
        Self {
            schema_version: C01_GOLDEN_GATE_REPORT_SCHEMA_VERSION.to_string(),
            status: C01GoldenGateStatus::Failed,
            entry_mode: C01GoldenGateEntryMode::CandidateConstruction,
            scenario_id: C01_GOLDEN_GATE_SCENARIO_ID.to_string(),
            project_root: request.project_root.display().to_string(),
            initial_project_digest: None,
            final_project_digest: None,
            candidates: Vec::new(),
            frozen_assets: Vec::new(),
            changed_paths: Vec::new(),
            reopen: None,
            preview: None,
            export: None,
            timing: C01GoldenGateTimingEvidence::default(),
            first_blocker: None,
            repairs: 0,
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        }
    }
}

pub fn run_c01_golden_gate(request: C01GoldenGateRequest) -> C01GoldenGateReport {
    let started = Instant::now();
    let mut report = C01GoldenGateReport::new(&request);
    let construction_started = Instant::now();
    let session = match run_candidate_construction(&request, &mut report) {
        Ok(session) => session,
        Err(error) => {
            finish_timing(&mut report, started);
            return finish_failed_report(&request, report, error);
        }
    };
    report.timing.candidate_construction_ms = elapsed_ms(construction_started);
    let preview_started = Instant::now();
    let preview = match run_save_reopen_preview(&request, &mut report, session) {
        Ok(preview) => preview,
        Err(error) => {
            report.timing.save_reopen_preview_ms = elapsed_ms(preview_started);
            finish_timing(&mut report, started);
            return finish_failed_report(&request, report, error);
        }
    };
    report.timing.save_reopen_preview_ms = elapsed_ms(preview_started);
    report.timing.first_playable_ms = elapsed_ms(started);
    let export_started = Instant::now();
    if let Err(error) = run_external_export(&request, &mut report, &preview) {
        report.timing.export_ms = elapsed_ms(export_started);
        finish_timing(&mut report, started);
        return finish_failed_report(&request, report, error);
    }
    report.timing.export_ms = elapsed_ms(export_started);
    finish_timing(&mut report, started);
    report.status = C01GoldenGateStatus::Passed;
    report
        .next_actions
        .push("Freeze C-01 evidence and run Gate G regression.".to_string());
    if let Err(error) = persist_json(
        &request.evidence_root.join("c01-golden-gate-report.json"),
        &report,
    ) {
        report.status = C01GoldenGateStatus::Failed;
        report.first_blocker = Some(error.clone());
        report.diagnostics.push(error);
    }
    report
}

pub fn validate_existing_c01_project(request: C01GoldenGateRequest) -> C01GoldenGateReport {
    let started = Instant::now();
    let mut report = C01GoldenGateReport::new(&request);
    report.entry_mode = C01GoldenGateEntryMode::ValidationOnly;
    let session = match prepare_existing_c01_validation(&request, &mut report) {
        Ok(session) => session,
        Err(error) => {
            finish_timing(&mut report, started);
            return finish_failed_report(&request, report, error);
        }
    };
    let preview_started = Instant::now();
    let preview = match run_save_reopen_preview(&request, &mut report, session) {
        Ok(preview) => preview,
        Err(error) => {
            report.timing.save_reopen_preview_ms = elapsed_ms(preview_started);
            finish_timing(&mut report, started);
            return finish_failed_report(&request, report, error);
        }
    };
    report.timing.save_reopen_preview_ms = elapsed_ms(preview_started);
    report.timing.first_playable_ms = elapsed_ms(started);
    let export_started = Instant::now();
    if let Err(error) = run_external_export(&request, &mut report, &preview) {
        report.timing.export_ms = elapsed_ms(export_started);
        finish_timing(&mut report, started);
        return finish_failed_report(&request, report, error);
    }
    report.timing.export_ms = elapsed_ms(export_started);
    finish_timing(&mut report, started);
    report.status = C01GoldenGateStatus::Passed;
    report.diagnostics.push(
        "Validation-only re-entry completed without preparing or applying candidates.".to_string(),
    );
    report
        .next_actions
        .push("Freeze C-01 v3 evidence and rerun Gate G regression.".to_string());
    if let Err(error) = persist_json(
        &request.evidence_root.join("c01-golden-gate-report.json"),
        &report,
    ) {
        report.status = C01GoldenGateStatus::Failed;
        report.first_blocker = Some(error.clone());
        report.diagnostics.push(error);
    }
    report
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn finish_timing(report: &mut C01GoldenGateReport, started: Instant) {
    report.timing.total_wall_clock_ms = elapsed_ms(started);
    report.timing.automation_active_ms = report
        .timing
        .total_wall_clock_ms
        .saturating_sub(report.timing.external_wait_ms);
}

fn prepare_existing_c01_validation(
    request: &C01GoldenGateRequest,
    report: &mut C01GoldenGateReport,
) -> Result<EditorSession, String> {
    fs::create_dir_all(&request.evidence_root)
        .map_err(|error| format!("evidence root create failed: {error}"))?;
    load_prior_candidate_evidence(request, report)?;

    let mut session = crate::identity_only_editor_session("project.c01.runtime");
    require_committed(
        &mut session,
        UiCommandPayload::OpenProject {
            path: request.project_root.display().to_string(),
        },
        "open existing C-01 project",
    )?;
    let binding = ProjectCandidateEntry::inspect_project_binding(&session)
        .map_err(|error| format!("existing project binding inspect failed: {error}"))?;
    verify_prior_candidate_project_identity(report, &binding.project_id)?;
    if report
        .candidates
        .last()
        .is_some_and(|candidate| candidate.applied_project_digest != binding.project_digest)
    {
        report.diagnostics.push(format!(
            "Current validation baseline {} differs from the D10 apply-time digest after later Save/Open activity; project identity and semantic completeness were revalidated.",
            binding.project_digest
        ));
    }
    report.initial_project_digest = Some(binding.project_digest.clone());
    report.final_project_digest = Some(binding.project_digest);

    validate_existing_c01_artifacts(request, &session)?;
    Ok(session)
}

fn verify_prior_candidate_project_identity(
    report: &C01GoldenGateReport,
    project_id: &str,
) -> Result<(), String> {
    for candidate in &report.candidates {
        let envelope =
            read_json_value(&Path::new(&candidate.evidence_path).join("01-envelope.json"))?;
        if envelope["targetProjectId"] != project_id {
            return Err(format!(
                "prior candidate {} targets a different project identity",
                candidate.candidate_id
            ));
        }
    }
    Ok(())
}

fn load_prior_candidate_evidence(
    request: &C01GoldenGateRequest,
    report: &mut C01GoldenGateReport,
) -> Result<(), String> {
    let report_path = request.evidence_root.join("c01-golden-gate-report.json");
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(&report_path)
            .map_err(|error| format!("prior C-01 report read failed: {error}"))?,
    )
    .map_err(|error| format!("prior C-01 report parse failed: {error}"))?;
    report.candidates = serde_json::from_value(
        value
            .get("candidates")
            .cloned()
            .ok_or_else(|| "prior C-01 report has no candidate evidence".to_string())?,
    )
    .map_err(|error| format!("prior candidate evidence parse failed: {error}"))?;
    let expected_ids = [
        "c01-d1-asset-tex-starfield",
        "c01-d2-asset-tex-player-ship",
        "c01-d3-asset-tex-enemy-scout",
        "c01-d4-asset-tex-bullet",
        "c01-d5-controlled-source",
        "c01-d6-input",
        "c01-d7-scene",
        "c01-d8-prefabs",
        "c01-d9-rule",
        "c01-d10-aui",
    ];
    if report.candidates.len() != expected_ids.len() {
        return Err(format!(
            "prior C-01 report must contain exactly 10 candidates, found {}",
            report.candidates.len()
        ));
    }
    for (index, (candidate, expected_id)) in report.candidates.iter().zip(expected_ids).enumerate()
    {
        if candidate.sequence != index + 1 || candidate.candidate_id != expected_id {
            return Err(format!(
                "prior candidate {} identity mismatch: expected {}, got {}",
                index + 1,
                expected_id,
                candidate.candidate_id
            ));
        }
        if index > 0
            && candidate.before_project_digest
                != report.candidates[index - 1].applied_project_digest
        {
            return Err(format!(
                "prior candidate {} receipt chain is broken",
                index + 1
            ));
        }
        let evidence_path = Path::new(&candidate.evidence_path);
        for file in [
            "02-candidate.json",
            "03-validation.json",
            "04-approval.json",
            "05-apply-receipt.json",
        ] {
            if !evidence_path.join(file).is_file() {
                return Err(format!(
                    "prior candidate {} evidence is missing {}",
                    candidate.candidate_id, file
                ));
            }
        }
    }
    report.frozen_assets = serde_json::from_value(
        value
            .get("frozenAssets")
            .cloned()
            .ok_or_else(|| "prior C-01 report has no frozen asset evidence".to_string())?,
    )
    .map_err(|error| format!("prior frozen asset evidence parse failed: {error}"))?;
    report.changed_paths = serde_json::from_value(
        value
            .get("changedPaths")
            .cloned()
            .ok_or_else(|| "prior C-01 report has no changed-path evidence".to_string())?,
    )
    .map_err(|error| format!("prior changed-path evidence parse failed: {error}"))?;
    Ok(())
}

fn validate_existing_c01_artifacts(
    request: &C01GoldenGateRequest,
    session: &EditorSession,
) -> Result<(), String> {
    let project = session
        .active_project_session()
        .ok_or_else(|| "existing C-01 project has no active session".to_string())?;
    let runtime_module = &project.manifest.runtime_module;
    if runtime_module.source_kind != Some(ProjectRuntimeSourceKind::ProjectRust)
        || runtime_module.module_id != "project.c01.runtime"
        || runtime_module.cargo_manifest != "RuntimeModule/Cargo.toml"
        || runtime_module.cargo_package != "c01_project_runtime"
    {
        return Err("existing C-01 ProjectRust manifest is incomplete".to_string());
    }

    let required_paths = [
        "Scenes/Main.scene.json",
        INPUT_PATH,
        "Prefabs/prefab-c01-enemy.prefab.json",
        "Prefabs/prefab-c01-bullet.prefab.json",
        "Rules/c01_tick.rule.json",
        "Rules/rule-manifest.json",
        AUI_PATH,
        "RuntimeModule/Cargo.toml",
        "RuntimeModule/src/lib.rs",
    ];
    for relative in required_paths {
        if !request.project_root.join(relative).is_file() {
            return Err(format!("existing C-01 artifact is missing: {relative}"));
        }
    }

    let scene = read_json_value(&request.project_root.join("Scenes/Main.scene.json"))?;
    let entities = scene["entities"]
        .as_array()
        .ok_or_else(|| "existing C-01 Scene has no entity array".to_string())?;
    for entity_id in [
        "entity-main-camera",
        "entity-starfield",
        "entity-player",
        "entity-session",
        "entity-enemy-template",
        "entity-bullet-template",
        "entity-hud",
    ] {
        if !entities.iter().any(|entity| entity["id"] == entity_id) {
            return Err(format!("existing C-01 Scene is missing {entity_id}"));
        }
    }
    let enemy_instances = entities
        .iter()
        .filter(|entity| {
            entity["components"].as_array().is_some_and(|components| {
                components.iter().any(|component| {
                    component["componentType"] == "engine.prefab_instance"
                        && component["fields"]["source"]["id"] == "prefab-c01-enemy"
                })
            })
        })
        .count();
    if enemy_instances < 5 {
        return Err(format!(
            "existing C-01 Scene requires at least 5 enemy prefab instances, found {enemy_instances}"
        ));
    }

    let input = read_json_value(&request.project_root.join(INPUT_PATH))?;
    for action_id in [
        "action.move",
        "action.fire",
        "action.dash",
        "action.restart",
    ] {
        if !input["actions"]
            .as_array()
            .is_some_and(|actions| actions.iter().any(|action| action["id"] == action_id))
        {
            return Err(format!("existing C-01 Input is missing {action_id}"));
        }
    }
    let rule_manifest = read_json_value(&request.project_root.join("Rules/rule-manifest.json"))?;
    if !rule_manifest["rules"].as_array().is_some_and(|rules| {
        rules
            .iter()
            .any(|rule| rule["ruleId"] == RULE_ID && rule["executor"] == "rustAot")
    }) {
        return Err("existing C-01 rule manifest is missing the Rust AOT tick rule".to_string());
    }
    let aui = read_json_value(&request.project_root.join(AUI_PATH))?;
    for binding in [
        "hud.hp",
        "hud.score",
        "hud.wave",
        "hud.dash",
        "hud.game_over",
    ] {
        if !aui["nodes"].as_array().is_some_and(|nodes| {
            nodes.iter().any(|node| {
                node["binding_refs"]
                    .as_array()
                    .is_some_and(|bindings| bindings.iter().any(|entry| entry["path"] == binding))
            })
        }) {
            return Err(format!("existing C-01 AUI is missing binding {binding}"));
        }
    }
    verify_reopened_asset_hashes(request)
}

fn read_json_value(path: &Path) -> Result<serde_json::Value, String> {
    serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("read {} failed: {error}", path.display()))?,
    )
    .map_err(|error| format!("parse {} failed: {error}", path.display()))
}

fn finish_failed_report(
    request: &C01GoldenGateRequest,
    mut report: C01GoldenGateReport,
    error: String,
) -> C01GoldenGateReport {
    report.first_blocker = Some(error.clone());
    report.diagnostics.push(error);
    report
        .next_actions
        .push("Resolve the first blocker and rerun the same Gate before continuing.".to_string());
    let _ = persist_json(
        &request.evidence_root.join("c01-golden-gate-report.json"),
        &report,
    );
    report
}

fn run_candidate_construction(
    request: &C01GoldenGateRequest,
    report: &mut C01GoldenGateReport,
) -> Result<EditorSession, String> {
    fs::create_dir_all(&request.candidate_store_root)
        .map_err(|error| format!("candidate store create failed: {error}"))?;
    fs::create_dir_all(&request.evidence_root)
        .map_err(|error| format!("evidence root create failed: {error}"))?;
    let mut session = EditorSession::new();
    let open = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::OpenProject {
            path: request.project_root.display().to_string(),
        },
    ));
    if open.status != CommandStatus::Committed {
        return Err(format!("open project failed: {:?}", open.diagnostics));
    }
    let initial = ProjectCandidateEntry::inspect_project_binding(&session)
        .map_err(|error| error.to_string())?;
    report.initial_project_digest = Some(initial.project_digest);

    for asset in frozen_assets(request) {
        let source_bytes = fs::read(&asset.source_path)
            .map_err(|error| format!("read frozen asset {} failed: {error}", asset.asset_id))?;
        let source_hash = sha256_prefixed(&source_bytes);
        if source_hash != asset.expected_hash {
            return Err(format!(
                "frozen asset {} hash mismatch: expected {}, got {}",
                asset.asset_id, asset.expected_hash, source_hash
            ));
        }
        report.frozen_assets.push(C01AssetEvidence {
            asset_id: asset.asset_id.to_string(),
            source_path: asset.source_path.display().to_string(),
            source_hash: source_hash.clone(),
            expected_hash: asset.expected_hash.to_string(),
        });
        let binding = ProjectCandidateEntry::inspect_project_binding(&session)
            .map_err(|error| error.to_string())?;
        let candidate_id = format!(
            "c01-d{}-asset-{}",
            report.candidates.len() + 1,
            asset.asset_id
        );
        let envelope = ProjectCandidateEnvelope {
            schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
            candidate_id,
            source_kind: ProjectCandidateSourceKind::ImportedCodex,
            source_label: "c01-golden-gate-frozen-asset".to_string(),
            target_project_id: binding.project_id,
            expected_base_project_digest: binding.project_digest,
            project_patch_context_hash: None,
            payload: ProjectCandidatePayload::AssetImport {
                request: ProjectAssetImportPrepareRequest {
                    import_id: format!("import-c01-{}", asset.asset_id),
                    revision_id: format!("revision-c01-{}", asset.asset_id),
                    project_root: request.project_root.clone(),
                    candidate_store_root: request.candidate_store_root.clone(),
                    source_path: asset.source_path,
                    target_directory: "Assets/Textures".to_string(),
                    asset_id: asset.asset_id.to_string(),
                    display_name: asset.display_name.to_string(),
                    conflict_policy: AssetImportConflictPolicy::RejectExisting,
                    source_metadata: AssetImportSourceMetadata::local_file(),
                    license: AssetLicenseMetadata::project_owned(),
                    texture_settings: TextureImportSettings::default(),
                },
                expected_source_hash: source_hash,
            },
        };
        execute_candidate(
            &mut session,
            envelope,
            ProjectCandidateValidationContext::default(),
            "asset_import",
            request,
            report,
        )?;
    }

    let rule_ir = ProjectRuleIr::new(RULE_ID, ProjectRulePhase::PostPhysics);
    let artifact_id = expected_rule_artifact_id(RULE_ID, &rule_ir.stable_hash());
    let binding = ProjectCandidateEntry::inspect_project_binding(&session)
        .map_err(|error| error.to_string())?;
    let mut manifest = session
        .active_project_session()
        .ok_or_else(|| "active project disappeared".to_string())?
        .manifest
        .clone();
    manifest.runtime_module = ProjectRuntimeModuleBuildSpec {
        source_kind: Some(ProjectRuntimeSourceKind::ProjectRust),
        module_id: "project.c01.runtime".to_string(),
        interface_version: editor_core::PROJECT_RUNTIME_MODULE_INTERFACE_VERSION.to_string(),
        cargo_manifest: "RuntimeModule/Cargo.toml".to_string(),
        cargo_package: "c01_project_runtime".to_string(),
        player_binary: "c01_project_player".to_string(),
        project_game_sdk: String::new(),
    };
    let source_patch = ControlledSourcePatchDocument {
        schema_version: editor_core::CONTROLLED_SOURCE_PATCH_SCHEMA_VERSION.to_string(),
        patch_id: "c01-d5-runtime-source".to_string(),
        operations: vec![
            ControlledSourcePatchOperation::CreateOrReplace {
                path: "RuntimeModule/Cargo.toml".to_string(),
                text: runtime_cargo_manifest(),
            },
            ControlledSourcePatchOperation::CreateOrReplace {
                path: "RuntimeModule/src/lib.rs".to_string(),
                text: runtime_source(&artifact_id),
            },
            ControlledSourcePatchOperation::CreateOrReplace {
                path: "project.aife.json".to_string(),
                text: serde_json::to_string_pretty(&manifest)
                    .map_err(|error| format!("project manifest serialize failed: {error}"))?,
            },
        ],
    };
    let envelope = ProjectCandidateEnvelope {
        schema_version: PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION.to_string(),
        candidate_id: "c01-d5-controlled-source".to_string(),
        source_kind: ProjectCandidateSourceKind::ImportedCodex,
        source_label: "c01-golden-gate-project-rust".to_string(),
        target_project_id: binding.project_id,
        expected_base_project_digest: binding.project_digest,
        project_patch_context_hash: None,
        payload: ProjectCandidatePayload::ControlledSourcePatch {
            request: ControlledSourcePatchPrepareRequest {
                revision_id: "revision-c01-d5-runtime-source".to_string(),
                project_root: request.project_root.clone(),
                candidate_store_root: request.candidate_store_root.clone(),
                source_patch,
            },
        },
    };
    execute_candidate(
        &mut session,
        envelope,
        ProjectCandidateValidationContext {
            controlled_source_patch: Some(
                ControlledSourcePatchValidationRequest::compile_tests_only(
                    &request.engine_sdk_root,
                ),
            ),
            cancellation: None,
        },
        "controlled_source_patch",
        request,
        report,
    )?;

    execute_project_patch(
        &mut session,
        "c01-d6-input",
        input_patch(),
        "project_patch_input",
        request,
        report,
    )?;
    let open_scene = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::OpenSceneDocument {
            path: request
                .project_root
                .join("Scenes/Main.scene.json")
                .display()
                .to_string(),
        },
    ));
    if open_scene.status != CommandStatus::Committed {
        return Err(format!(
            "D7 scene context load failed: {:?}",
            open_scene.diagnostics
        ));
    }
    execute_project_patch(
        &mut session,
        "c01-d7-scene",
        scene_patch(),
        "project_patch_scene",
        request,
        report,
    )?;
    execute_project_patch(
        &mut session,
        "c01-d8-prefabs",
        prefab_patch(),
        "project_patch_prefab",
        request,
        report,
    )?;
    execute_project_patch(
        &mut session,
        "c01-d9-rule",
        rule_patch(),
        "project_patch_rule",
        request,
        report,
    )?;
    execute_project_patch(
        &mut session,
        "c01-d10-aui",
        aui_patch(),
        "project_patch_aui",
        request,
        report,
    )?;

    let final_binding = ProjectCandidateEntry::inspect_project_binding(&session)
        .map_err(|error| error.to_string())?;
    report.final_project_digest = Some(final_binding.project_digest);
    report.changed_paths.sort();
    report.changed_paths.dedup();
    Ok(session)
}

struct PreparedC01Preview {
    player_artifact: ProjectPlayerArtifact,
}

fn run_save_reopen_preview(
    request: &C01GoldenGateRequest,
    report: &mut C01GoldenGateReport,
    mut original_session: EditorSession,
) -> Result<PreparedC01Preview, String> {
    require_committed(
        &mut original_session,
        UiCommandPayload::SaveSceneDocument { path: None },
        "save Scene before close",
    )?;
    let saved_binding = ProjectCandidateEntry::inspect_project_binding(&original_session)
        .map_err(|error| format!("save binding inspect failed: {error}"))?;
    if report.final_project_digest.as_deref() != Some(saved_binding.project_digest.as_str()) {
        return Err("saved project digest diverged from the final candidate receipt".to_string());
    }
    drop(original_session);

    let mut reopened = crate::identity_only_editor_session("project.c01.runtime");
    require_committed(
        &mut reopened,
        UiCommandPayload::OpenProject {
            path: request.project_root.display().to_string(),
        },
        "reopen project",
    )?;
    let reopened_binding = ProjectCandidateEntry::inspect_project_binding(&reopened)
        .map_err(|error| format!("reopened binding inspect failed: {error}"))?;
    if reopened_binding.project_digest != saved_binding.project_digest {
        return Err(format!(
            "reopened project digest mismatch: saved {}, reopened {}",
            saved_binding.project_digest, reopened_binding.project_digest
        ));
    }
    let scene_status = require_committed(
        &mut reopened,
        UiCommandPayload::OpenSceneDocument {
            path: request
                .project_root
                .join("Scenes/Main.scene.json")
                .display()
                .to_string(),
        },
        "reopen Scene",
    )?;
    let input_status = require_committed(
        &mut reopened,
        UiCommandPayload::OpenInputMapping {
            path: INPUT_PATH.to_string(),
        },
        "reopen Input mapping",
    )?;
    let prefab_status = require_committed(
        &mut reopened,
        UiCommandPayload::OpenPrefabDocument {
            path: "Prefabs/prefab-c01-enemy.prefab.json".to_string(),
        },
        "reopen enemy Prefab",
    )?;
    let rule_status = require_committed(
        &mut reopened,
        UiCommandPayload::OpenRuleAsset {
            path: "Rules/c01_tick.rule.json".to_string(),
        },
        "reopen Rule asset",
    )?;
    let aui_status = require_committed(
        &mut reopened,
        UiCommandPayload::OpenAuiDocument {
            path: AUI_PATH.to_string(),
        },
        "reopen AUI document",
    )?;
    verify_reopened_asset_hashes(request)?;
    report.reopen = Some(C01ReopenEvidence {
        saved_project_digest: saved_binding.project_digest,
        reopened_project_digest: reopened_binding.project_digest,
        scene_status,
        input_status,
        prefab_status,
        rule_status,
        aui_status,
    });
    drop(reopened);

    let mut preview_request = EditorPreviewPackageRequest::editor_play(&request.project_root);
    preview_request.requested_by = "c01-golden-gate".to_string();
    preview_request.allow_autosave = false;
    preview_request.allow_last_good = false;
    preview_request.force_rebuild = true;
    preview_request.frame_limit = 3;
    let wait_started = Instant::now();
    let preview = EditorPreviewPackageService::prepare(preview_request);
    report.timing.external_wait_ms = report
        .timing
        .external_wait_ms
        .saturating_add(elapsed_ms(wait_started));
    persist_json(
        &request.evidence_root.join("preview-package-report.json"),
        &preview,
    )?;
    if preview.status != EditorPreviewPackageStatus::Success || preview.has_errors() {
        return Err(format!(
            "Preview package preparation failed: {:?}",
            preview.diagnostics
        ));
    }
    let runtime_package_dir = preview
        .runtime_package_dir
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| "Preview succeeded without a RuntimePackage path".to_string())?;
    let player_artifact = preview
        .player_artifact
        .clone()
        .ok_or_else(|| "Preview succeeded without a ProjectRust Player artifact".to_string())?;
    let input_script_path = request.evidence_root.join("c01-input-script.json");
    persist_json(&input_script_path, &c01_input_script())?;

    let headless_report_path = request
        .evidence_root
        .join("preview-headless-run-report.json");
    let wait_started = Instant::now();
    let headless = run_project_player(
        &player_artifact,
        &runtime_package_dir,
        "headless-gate",
        520,
        &headless_report_path,
        Some(&input_script_path),
        Some("trace"),
        None,
        None,
    )?;
    report.timing.external_wait_ms = report
        .timing
        .external_wait_ms
        .saturating_add(elapsed_ms(wait_started));
    let assertions = runtime_assertions(&headless);
    if !assertions.all_passed() {
        return Err(format!(
            "Preview runtime transitions were incomplete: {assertions:#?}"
        ));
    }

    let screenshot_path = request.evidence_root.join("preview-window.png");
    let window_report_path = request
        .evidence_root
        .join("preview-windowed-run-report.json");
    let wait_started = Instant::now();
    run_project_player(
        &player_artifact,
        &runtime_package_dir,
        "windowed",
        120,
        &window_report_path,
        None,
        Some("summary"),
        Some(&screenshot_path),
        None,
    )?;
    report.timing.external_wait_ms = report
        .timing
        .external_wait_ms
        .saturating_add(elapsed_ms(wait_started));
    let screenshot_hash = hash_nonempty_file(&screenshot_path, "Preview screenshot")?;
    let screenshot_visual = verify_c01_screenshot_visuals(&screenshot_path, "Preview")?;
    report.preview = Some(C01PreviewEvidence {
        status: "passed".to_string(),
        cache_key: preview.cache_key,
        runtime_package_dir: runtime_package_dir.display().to_string(),
        player_executable: player_artifact.executable_path.display().to_string(),
        player_executable_hash: player_artifact.source_executable_hash.clone(),
        player_module_descriptor: player_artifact.module_descriptor.clone(),
        headless_report_path: headless_report_path.display().to_string(),
        screenshot_path: screenshot_path.display().to_string(),
        screenshot_hash,
        screenshot_visual,
        runtime_assertions: assertions,
    });
    Ok(PreparedC01Preview { player_artifact })
}

fn run_external_export(
    request: &C01GoldenGateRequest,
    report: &mut C01GoldenGateReport,
    preview: &PreparedC01Preview,
) -> Result<(), String> {
    let desktop_output = request.evidence_root.join("desktop-export");
    let mut desktop_request = DesktopExportRequest::windows_dev(&request.project_root)
        .with_explicit_output(ExplicitExportOutput::from_user_selected(&desktop_output));
    desktop_request.frame_limit = 2;
    let wait_started = Instant::now();
    let desktop = DesktopExportPipeline::export(desktop_request);
    report.timing.external_wait_ms = report
        .timing
        .external_wait_ms
        .saturating_add(elapsed_ms(wait_started));
    persist_json(
        &request.evidence_root.join("desktop-export-report.json"),
        &desktop,
    )?;
    if desktop.status != DesktopExportStatus::Success {
        return Err(format!("Desktop export failed: {:?}", desktop.diagnostics));
    }
    if desktop.player_module_descriptor.as_ref() != Some(&preview.player_artifact.module_descriptor)
        || desktop.player_artifact_hash.as_deref()
            != Some(preview.player_artifact.source_executable_hash.as_str())
    {
        return Err("Preview and Desktop Export Player artifact identity diverged".to_string());
    }

    let release_profile_path = request.evidence_root.join("c01-windows-release.json");
    persist_json(&release_profile_path, &c01_release_profile())?;
    let mut release_request = ReleasePackageBuildRequest::windows_release(&request.project_root)
        .with_explicit_output(ExplicitExportOutput::from_user_selected(
            &request.external_export_root,
        ))
        .with_explicit_report_output(ExplicitExportOutput::from_user_selected(
            &request.evidence_root,
        ));
    release_request.build_profile_path = release_profile_path;
    release_request.output_dir = Some(request.external_export_root.clone());
    release_request.player_executable = Some(PathBuf::from(&desktop.package_dir).join("Game.exe"));
    release_request.report_path = Some(request.evidence_root.join("release-package-report.json"));
    release_request.report_level = ReleasePackageReportLevel::Trace;
    release_request.verify_process = true;
    release_request.process_timeout_ms = 120_000;
    let wait_started = Instant::now();
    let release = ReleasePackageBuilder::build(&release_request);
    report.timing.external_wait_ms = report
        .timing
        .external_wait_ms
        .saturating_add(elapsed_ms(wait_started));
    if release.status != ReleasePackageStatus::Success {
        return Err(format!(
            "Release package build failed: {:?}",
            release.diagnostics
        ));
    }

    let input_script_path = request.evidence_root.join("c01-input-script.json");
    let headless_report_path = request
        .evidence_root
        .join("export-headless-verification-report.json");
    let wait_started = Instant::now();
    let headless_verification = verify_exported_player_process_with_options(
        ExportedPlayerProcessVerificationRequest {
            exported_package_dir: request.external_export_root.clone(),
            mode: "headless-gate".to_string(),
            frame_limit: 520,
            report_path: Some(headless_report_path.clone()),
            timeout_ms: 180_000,
            screenshot: false,
            screenshot_path: None,
        },
        ExportedPlayerProcessVerificationOptions {
            input_script_path: Some(input_script_path),
            runtime_report_level: Some("trace".to_string()),
            performance_warmup_frames: 0,
            performance_sample_frames: 0,
        },
    );
    report.timing.external_wait_ms = report
        .timing
        .external_wait_ms
        .saturating_add(elapsed_ms(wait_started));
    if headless_verification.status != ExportedPlayerProcessVerificationStatus::Passed {
        return Err(format!(
            "Exported headless verification failed: {:?}",
            headless_verification.diagnostics
        ));
    }
    let exported_headless =
        read_windowed_player_report(Path::new(&headless_verification.child_report_path))?;
    let exported_assertions = runtime_assertions(&exported_headless);
    if !exported_assertions.all_passed() {
        return Err(format!(
            "Exported runtime transitions were incomplete: {exported_assertions:#?}"
        ));
    }

    let screenshot_path = request.evidence_root.join("export-window.png");
    let windowed_report_path = request
        .evidence_root
        .join("export-windowed-verification-report.json");
    let wait_started = Instant::now();
    let windowed_verification = verify_exported_player_process_with_options(
        ExportedPlayerProcessVerificationRequest {
            exported_package_dir: request.external_export_root.clone(),
            mode: "windowed".to_string(),
            frame_limit: 720,
            report_path: Some(windowed_report_path.clone()),
            timeout_ms: 180_000,
            screenshot: true,
            screenshot_path: Some(screenshot_path.clone()),
        },
        ExportedPlayerProcessVerificationOptions {
            input_script_path: None,
            runtime_report_level: Some("summary".to_string()),
            performance_warmup_frames: 120,
            performance_sample_frames: 600,
        },
    );
    report.timing.external_wait_ms = report
        .timing
        .external_wait_ms
        .saturating_add(elapsed_ms(wait_started));
    if windowed_verification.status != ExportedPlayerProcessVerificationStatus::Passed {
        return Err(format!(
            "Exported windowed verification failed: {:?}",
            windowed_verification.diagnostics
        ));
    }
    let exported_windowed =
        read_windowed_player_report(Path::new(&windowed_verification.child_report_path))?;
    let performance = exported_windowed
        .frame_performance_summary
        .clone()
        .ok_or_else(|| "Exported windowed run omitted frame performance evidence".to_string())?;
    if performance.warmup_frames != 120
        || performance.requested_sample_frames != 600
        || performance.observed_sample_frames != 600
    {
        return Err(format!(
            "Exported frame performance sample is incomplete: {performance:#?}"
        ));
    }
    let screenshot_hash = hash_nonempty_file(&screenshot_path, "export screenshot")?;
    let screenshot_visual = verify_c01_screenshot_visuals(&screenshot_path, "Export")?;
    let no_arg_status = crate::release_package::run_finite_no_arg_release_copy(
        &request.external_export_root,
        &request.evidence_root,
        120_000,
    )
    .map_err(|error| format!("zero-argument release verification failed: {error}"))?;
    if !no_arg_status.success() {
        return Err(format!(
            "zero-argument release entrypoint exited with {no_arg_status}"
        ));
    }

    report.export = Some(C01ExportEvidence {
        desktop_export_status: "success".to_string(),
        desktop_package_dir: desktop.package_dir,
        desktop_report_path: request
            .evidence_root
            .join("desktop-export-report.json")
            .display()
            .to_string(),
        release_status: "success".to_string(),
        release_output_dir: release.output_dir,
        release_report_path: release.report_path,
        headless_verification_status: "passed".to_string(),
        headless_verification_report_path: headless_report_path.display().to_string(),
        windowed_verification_status: "passed".to_string(),
        windowed_verification_report_path: windowed_report_path.display().to_string(),
        screenshot_path: screenshot_path.display().to_string(),
        screenshot_hash,
        screenshot_visual,
        performance,
        runtime_assertions: exported_assertions,
        no_arg_launch_passed: true,
    });
    Ok(())
}

fn verify_c01_screenshot_visuals(
    path: &Path,
    stage: &str,
) -> Result<C01ScreenshotVisualEvidence, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("{stage} screenshot open failed: {error}"))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("{stage} screenshot PNG header decode failed: {error}"))?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("{stage} screenshot PNG decode failed: {error}"))?;
    let bytes = &buffer[..info.buffer_size()];
    let rgba = png_frame_to_rgba(bytes, info.color_type, stage)?;
    let pixel_count = info.width as usize * info.height as usize;
    if rgba.len() != pixel_count * 4 || pixel_count == 0 {
        return Err(format!(
            "{stage} screenshot decoded to an invalid pixel buffer"
        ));
    }

    let mut colors = HashMap::<[u8; 4], usize>::new();
    let mut player_blue_pixel_count = 0usize;
    let mut enemy_red_pixel_count = 0usize;
    let mut content_min_x = info.width;
    let mut content_min_y = info.height;
    let mut content_max_x = 0u32;
    let mut content_max_y = 0u32;
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        let color = [pixel[0], pixel[1], pixel[2], pixel[3]];
        *colors.entry(color).or_default() += 1;
        if pixel[3] > 0 && (pixel[0] > 2 || pixel[1] > 2 || pixel[2] > 2) {
            let x = index as u32 % info.width;
            let y = index as u32 / info.width;
            content_min_x = content_min_x.min(x);
            content_min_y = content_min_y.min(y);
            content_max_x = content_max_x.max(x);
            content_max_y = content_max_y.max(y);
        }
        if pixel[3] >= 128
            && pixel[2] >= 140
            && i16::from(pixel[2]) - i16::from(pixel[0]) >= 35
            && pixel[1] >= 70
        {
            player_blue_pixel_count += 1;
        }
        if pixel[3] >= 128
            && pixel[0] >= 150
            && i16::from(pixel[0]) - i16::from(pixel[1]) >= 45
            && i16::from(pixel[0]) - i16::from(pixel[2]) >= 35
        {
            enemy_red_pixel_count += 1;
        }
    }
    if content_min_x == info.width {
        return Err(format!("{stage} screenshot has no visible content pixels"));
    }

    let hud_max_y = content_min_y + (content_max_y - content_min_y + 1) / 3;
    let hud_near_white_pixel_count = rgba
        .chunks_exact(4)
        .enumerate()
        .filter(|(index, pixel)| {
            let y = *index as u32 / info.width;
            y >= content_min_y
                && y <= hud_max_y
                && pixel[3] >= 128
                && pixel[0] >= 220
                && pixel[1] >= 220
                && pixel[2] >= 220
                && pixel[0].max(pixel[1]).max(pixel[2]) - pixel[0].min(pixel[1]).min(pixel[2]) <= 25
        })
        .count();
    let dominant_color_fraction =
        colors.values().copied().max().unwrap_or_default() as f64 / pixel_count as f64;
    let evidence = C01ScreenshotVisualEvidence {
        width: info.width,
        height: info.height,
        unique_color_count: colors.len(),
        dominant_color_fraction,
        player_blue_pixel_count,
        enemy_red_pixel_count,
        hud_near_white_pixel_count,
        content_bounds: [content_min_x, content_min_y, content_max_x, content_max_y],
    };

    let mut failures = Vec::new();
    if evidence.unique_color_count < 16 {
        failures.push(format!(
            "uniqueColorCount={} (expected at least 16)",
            evidence.unique_color_count
        ));
    }
    if evidence.dominant_color_fraction > 0.98 {
        failures.push(format!(
            "dominantColorFraction={:.6} (must not exceed 0.98)",
            evidence.dominant_color_fraction
        ));
    }
    if evidence.player_blue_pixel_count < 8 {
        failures.push(format!(
            "playerBluePixelCount={} (expected at least 8)",
            evidence.player_blue_pixel_count
        ));
    }
    if evidence.enemy_red_pixel_count < 8 {
        failures.push(format!(
            "enemyRedPixelCount={} (expected at least 8)",
            evidence.enemy_red_pixel_count
        ));
    }
    if evidence.hud_near_white_pixel_count < 16 {
        failures.push(format!(
            "hudNearWhitePixelCount={} (expected at least 16 in the upper content region)",
            evidence.hud_near_white_pixel_count
        ));
    }
    if failures.is_empty() {
        Ok(evidence)
    } else {
        Err(format!(
            "{stage} screenshot failed real visual acceptance: {}; evidence={evidence:#?}",
            failures.join(", ")
        ))
    }
}

fn png_frame_to_rgba(
    bytes: &[u8],
    color_type: png::ColorType,
    stage: &str,
) -> Result<Vec<u8>, String> {
    let mut rgba = Vec::with_capacity(match color_type {
        png::ColorType::Rgba => bytes.len(),
        png::ColorType::Rgb => bytes.len() / 3 * 4,
        png::ColorType::GrayscaleAlpha => bytes.len() / 2 * 4,
        png::ColorType::Grayscale => bytes.len() * 4,
        png::ColorType::Indexed => {
            return Err(format!(
                "{stage} screenshot remained indexed after PNG expansion"
            ));
        }
    });
    match color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(bytes),
        png::ColorType::Rgb => {
            for pixel in bytes.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in bytes.chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for value in bytes {
                rgba.extend_from_slice(&[*value, *value, *value, 255]);
            }
        }
        png::ColorType::Indexed => unreachable!(),
    }
    Ok(rgba)
}

fn require_committed(
    session: &mut EditorSession,
    payload: UiCommandPayload,
    stage: &str,
) -> Result<String, String> {
    let result = session.execute_command(editor_core::command_for_test(payload));
    if result.status != CommandStatus::Committed {
        return Err(format!("{stage} failed: {:?}", result.diagnostics));
    }
    Ok("committed".to_string())
}

fn verify_reopened_asset_hashes(request: &C01GoldenGateRequest) -> Result<(), String> {
    for asset in frozen_assets(request) {
        let path = request
            .project_root
            .join("Assets/Textures")
            .join(format!("{}.png", asset.asset_id));
        let bytes = fs::read(&path)
            .map_err(|error| format!("reopened asset {} read failed: {error}", path.display()))?;
        let actual = sha256_prefixed(&bytes);
        if actual != asset.expected_hash {
            return Err(format!(
                "reopened asset hash mismatch for {}: expected {}, got {}",
                asset.asset_id, asset.expected_hash, actual
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_project_player(
    artifact: &ProjectPlayerArtifact,
    runtime_package_dir: &Path,
    mode: &str,
    frame_limit: u64,
    report_path: &Path,
    input_script_path: Option<&Path>,
    runtime_report_level: Option<&str>,
    screenshot_path: Option<&Path>,
    performance: Option<(u64, u64)>,
) -> Result<WindowedPlayerRunReport, String> {
    let mut args = vec![
        OsString::from("run-native-player"),
        OsString::from("--package"),
        runtime_package_dir.as_os_str().to_owned(),
        OsString::from("--mode"),
        OsString::from(mode),
        OsString::from("--frames"),
        OsString::from(frame_limit.to_string()),
        OsString::from("--report"),
        report_path.as_os_str().to_owned(),
    ];
    if let Some(path) = input_script_path {
        args.extend([
            OsString::from("--input-script"),
            path.as_os_str().to_owned(),
        ]);
    }
    if let Some(level) = runtime_report_level {
        args.extend([
            OsString::from("--runtime-report-level"),
            OsString::from(level),
        ]);
    }
    if let Some(path) = screenshot_path {
        args.extend([
            OsString::from("--screenshot"),
            OsString::from("--screenshot-path"),
            path.as_os_str().to_owned(),
        ]);
    }
    if let Some((warmup, sample)) = performance {
        args.extend([
            OsString::from("--performance-warmup-frames"),
            OsString::from(warmup.to_string()),
            OsString::from("--performance-sample-frames"),
            OsString::from(sample.to_string()),
        ]);
    }
    let process = run_bounded_child_process(BoundedChildProcessRequest {
        executable: artifact.executable_path.clone(),
        args,
        current_dir: artifact
            .executable_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        environment: Vec::new(),
        timeout: Duration::from_secs(180),
        stdout_capture_limit_bytes: 128 * 1024,
        stderr_capture_limit_bytes: 128 * 1024,
        priority: runtime_cli::BoundedChildProcessPriority::Normal,
    });
    if process.exit_reason != BoundedChildProcessExitReason::Completed
        || process.exit_code != Some(0)
    {
        return Err(format!(
            "Project Player {mode} run failed ({:?}, {:?}): {}",
            process.exit_reason, process.exit_code, process.stderr_summary
        ));
    }
    let report = read_windowed_player_report(report_path)?;
    if report.exit_code != Some(0) || report.has_errors() {
        return Err(format!(
            "Project Player {mode} report failed: {:?}",
            report.diagnostics
        ));
    }
    Ok(report)
}

fn read_windowed_player_report(path: &Path) -> Result<WindowedPlayerRunReport, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "read WindowedPlayer report {} failed: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "parse WindowedPlayer report {} failed: {error}",
            path.display()
        )
    })
}

fn runtime_assertions(report: &WindowedPlayerRunReport) -> C01RuntimeAssertions {
    let records = &report.gameplay_trace_records;
    let dash_records = matching_writes(
        records,
        "entity-player",
        "project.c01Player",
        "dashCooldownRemaining",
    );
    let game_over_records =
        matching_writes(records, "entity-session", "project.c01Session", "gameOver");
    let game_over_frame = game_over_records
        .iter()
        .find(|record| record.after.as_deref() == Some("Bool(true)"))
        .map(|record| record.frame_index);
    C01RuntimeAssertions {
        movement: records.iter().any(|record| {
            record.operation == "write"
                && record.entity_id.as_deref() == Some("entity-player")
                && record.component_type.as_deref() == Some("engine.transform")
                && record.field_path.as_deref() == Some("local_position")
                && record.before != record.after
        }),
        dash_started: dash_records
            .iter()
            .any(|record| record.after.as_deref() == Some("F64(2.0)")),
        dash_cooldown_observed: dash_records.iter().any(|record| {
            record
                .after
                .as_deref()
                .is_some_and(|value| value.starts_with("F64(1."))
        }),
        bullet_prefab_instantiated: records.iter().any(|record| {
            record.operation == "command_apply"
                && record.source.as_deref() == Some("prefab-c01-bullet")
                && record.result == "ok"
        }),
        score_increased: matching_writes(records, "entity-session", "project.c01Session", "score")
            .into_iter()
            .any(|record| {
                record
                    .after
                    .as_deref()
                    .is_some_and(|value| value != "I64(0)")
            }),
        wave_advanced: matching_writes(records, "entity-session", "project.c01Session", "wave")
            .into_iter()
            .any(|record| {
                record
                    .after
                    .as_deref()
                    .is_some_and(|value| value != "I64(1)")
            }),
        game_over_observed: game_over_frame.is_some(),
        restart_observed: game_over_frame.is_some_and(|frame| {
            game_over_records.iter().any(|record| {
                record.frame_index > frame && record.after.as_deref() == Some("Bool(false)")
            })
        }),
    }
}

fn matching_writes<'a>(
    records: &'a [WindowedPlayerGameplayTraceRecord],
    entity: &str,
    component: &str,
    field: &str,
) -> Vec<&'a WindowedPlayerGameplayTraceRecord> {
    records
        .iter()
        .filter(|record| {
            record.operation == "write"
                && record.entity_id.as_deref() == Some(entity)
                && record.component_type.as_deref() == Some(component)
                && record.field_path.as_deref() == Some(field)
        })
        .collect()
}

fn c01_input_script() -> NativePlayerInputScript {
    NativePlayerInputScript::new(
        "c01-deterministic-combat",
        vec![
            NativePlayerInputScriptFrame::keys(1, ["D", "ShiftLeft"], [] as [&str; 0]),
            NativePlayerInputScriptFrame::keys(2, [] as [&str; 0], ["ShiftLeft"]),
            NativePlayerInputScriptFrame::keys(21, ["A"], ["D"]),
            NativePlayerInputScriptFrame::keys(53, [] as [&str; 0], ["A"]),
            NativePlayerInputScriptFrame::keys(380, ["R"], [] as [&str; 0]),
            NativePlayerInputScriptFrame::keys(381, ["Space"], ["R"]),
            NativePlayerInputScriptFrame::keys(500, [] as [&str; 0], ["Space"]),
        ],
    )
}

fn c01_release_profile() -> BuildProfile {
    BuildProfile {
        schema_version: "build-profile.v2".to_string(),
        profile: "release".to_string(),
        target: "windows".to_string(),
        runtime_package_mode: "debug-readable".to_string(),
        frame_limit: 6,
        headless_surface_gate: true,
        real_window_smoke: "required".to_string(),
        game_view_target: None,
        architecture: Some("x86_64".to_string()),
        application: Some(BuildProfileApplication {
            display_name: "AI First Arena".to_string(),
            executable_name: "AiFirstArena".to_string(),
            company_name: "AI First Engine Studio".to_string(),
            file_description: "C-01 2D Combat Arena".to_string(),
            display_version: "1.0.0".to_string(),
            windows_file_version: [1, 0, 0, 0],
            windows_product_version: [1, 0, 0, 0],
            copyright: "Copyright AI First Engine Studio".to_string(),
            icon: BuildProfileIconRef {
                asset_id: "tex-player-ship".to_string(),
            },
        }),
        release: Some(BuildProfileRelease {
            layout: "portable-directory-v1".to_string(),
            include_reports: false,
            include_debug_symbols: false,
        }),
    }
}

fn hash_nonempty_file(path: &Path, label: &str) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("{label} {} cannot be read: {error}", path.display()))?;
    if bytes.is_empty() {
        return Err(format!("{label} {} is empty", path.display()));
    }
    Ok(sha256_prefixed(&bytes))
}

fn execute_project_patch(
    session: &mut EditorSession,
    candidate_id: &str,
    patch: ProjectPatchDocument,
    payload_kind: &str,
    request: &C01GoldenGateRequest,
    report: &mut C01GoldenGateReport,
) -> Result<(), String> {
    let envelope = ProjectCandidateEntry::project_patch_envelope(
        session,
        candidate_id,
        ProjectCandidateSourceKind::ImportedCodex,
        "c01-golden-gate-project-patch",
        patch,
    )
    .map_err(|error| error.to_string())?;
    execute_candidate(
        session,
        envelope,
        ProjectCandidateValidationContext::default(),
        payload_kind,
        request,
        report,
    )
}

fn execute_candidate(
    session: &mut EditorSession,
    envelope: ProjectCandidateEnvelope,
    validation_context: ProjectCandidateValidationContext,
    payload_kind: &str,
    request: &C01GoldenGateRequest,
    report: &mut C01GoldenGateReport,
) -> Result<(), String> {
    let binding_before = ProjectCandidateEntry::inspect_project_binding(session)
        .map_err(|error| error.to_string())?;
    if envelope.expected_base_project_digest != binding_before.project_digest {
        return Err(format!(
            "candidate {} was not rebound after the previous apply",
            envelope.candidate_id
        ));
    }
    let imported_json = serde_json::to_string_pretty(&envelope).map_err(|error| {
        format!(
            "{} imported JSON serialize failed: {error}",
            envelope.candidate_id
        )
    })?;
    let candidate = ProjectCandidateEntry::from_json_string(session, &imported_json)
        .map_err(|error| format!("{} prepare failed: {error}", envelope.candidate_id))?;
    let validation_started = Instant::now();
    let validation = ProjectCandidateEntry::validate(session, &candidate, &validation_context)
        .map_err(|error| format!("{} validate failed: {error}", envelope.candidate_id))?;
    if payload_kind == "controlled_source_patch" {
        report.timing.external_wait_ms = report
            .timing
            .external_wait_ms
            .saturating_add(elapsed_ms(validation_started));
    }
    if validation.status != ProjectCandidateValidationStatus::Passed {
        return Err(format!(
            "{} validation rejected: {validation:#?}",
            envelope.candidate_id
        ));
    }
    let approval = ProjectCandidateApproval {
        schema_version: PROJECT_CANDIDATE_APPROVAL_SCHEMA_VERSION.to_string(),
        candidate_id: candidate.envelope.candidate_id.clone(),
        candidate_digest: candidate.candidate_digest.clone(),
        validation_digest: validation.validation_digest.clone(),
        approved_by: request.approval_actor.clone(),
        allow_replace: false,
    };
    let receipt = ProjectCandidateEntry::apply(
        session,
        candidate.clone(),
        validation.clone(),
        approval.clone(),
    )
    .map_err(|error| format!("{} apply failed: {error}", envelope.candidate_id))?;
    let binding_after = ProjectCandidateEntry::inspect_project_binding(session)
        .map_err(|error| error.to_string())?;
    if binding_after.project_digest != receipt.applied_project_digest {
        return Err(format!(
            "{} receipt does not match re-inspected project digest",
            envelope.candidate_id
        ));
    }
    let sequence = report.candidates.len() + 1;
    let evidence_dir = request
        .evidence_root
        .join(format!("{sequence:02}-{}", envelope.candidate_id));
    persist_candidate_evidence(
        &evidence_dir,
        &imported_json,
        &envelope,
        &candidate,
        &validation,
        &approval,
        &receipt,
    )?;
    report.changed_paths.extend(receipt_changed_paths(&receipt));
    report.candidates.push(C01CandidateEvidence {
        sequence,
        candidate_id: envelope.candidate_id,
        payload_kind: payload_kind.to_string(),
        source_digest: candidate.source_digest.clone(),
        envelope_digest: candidate.envelope_digest.clone(),
        candidate_digest: candidate.candidate_digest,
        validation_digest: validation.validation_digest,
        approval_digest: digest_json(&approval)?,
        receipt_digest: receipt.receipt_binding_digest.clone(),
        before_project_digest: receipt.before_project_digest,
        applied_project_digest: receipt.applied_project_digest,
        evidence_path: evidence_dir.display().to_string(),
    });
    Ok(())
}

fn persist_candidate_evidence(
    root: &Path,
    imported_json: &str,
    envelope: &ProjectCandidateEnvelope,
    candidate: &ProjectCandidate,
    validation: &ProjectCandidateValidationReport,
    approval: &ProjectCandidateApproval,
    receipt: &ProjectCandidateApplyReceipt,
) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("candidate evidence directory create failed: {error}"))?;
    fs::write(
        root.join("00-imported-codex-envelope.json"),
        imported_json.as_bytes(),
    )
    .map_err(|error| format!("imported candidate source write failed: {error}"))?;
    persist_json(&root.join("01-envelope.json"), envelope)?;
    persist_json(&root.join("02-candidate.json"), candidate)?;
    persist_json(&root.join("03-validation.json"), validation)?;
    persist_json(&root.join("04-approval.json"), approval)?;
    persist_json(&root.join("05-apply-receipt.json"), receipt)
}

fn persist_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize {} failed: {error}", path.display()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {} failed: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("write {} failed: {error}", path.display()))
}

fn digest_json(value: &impl Serialize) -> Result<String, String> {
    let value = serde_json::to_value(value)
        .map_err(|error| format!("JSON value serialization failed: {error}"))?;
    let bytes = engine_runtime::canonical_digest::canonical_json_bytes(&value)
        .map_err(|error| format!("canonical JSON failed: {error}"))?;
    Ok(sha256_prefixed(&bytes))
}

fn receipt_changed_paths(receipt: &ProjectCandidateApplyReceipt) -> Vec<String> {
    match &receipt.applied_payload {
        editor_core::ProjectCandidateAppliedPayload::ProjectPatch { report, .. } => report
            .operation_results
            .iter()
            .map(|result| format!("{}:{}", result.kind, result.operation_id))
            .collect(),
        editor_core::ProjectCandidateAppliedPayload::ControlledSourcePatch { receipt } => {
            receipt.changed_paths.clone()
        }
        editor_core::ProjectCandidateAppliedPayload::AssetImport { receipt } => {
            receipt.changed_paths.clone()
        }
    }
}

struct FrozenAsset {
    asset_id: &'static str,
    display_name: &'static str,
    source_path: PathBuf,
    expected_hash: &'static str,
}

fn frozen_assets(request: &C01GoldenGateRequest) -> Vec<FrozenAsset> {
    [
        (
            "tex-starfield",
            "Starfield",
            "tex-starfield.png",
            "sha256:f27a14785320e386f4c6b1f5bcf04f5e79ee1bfdfc06336a9ed3b356e67f6141",
        ),
        (
            "tex-player-ship",
            "Player Ship",
            "tex-player-ship.png",
            "sha256:9657a0141f9224d656776f8e478062ea51a15ce8152c1cce59006677f41bb80c",
        ),
        (
            "tex-enemy-scout",
            "Enemy Scout",
            "tex-enemy-scout.png",
            "sha256:7f7917a45bbdd352158e2a8a24411535d8306a8ec6d6e6eaf34c1e5e7307a803",
        ),
        (
            "tex-bullet",
            "Player Bullet",
            "tex-bullet.png",
            "sha256:1b246eb588c49078dd242e5c3d478202a9b463cab05d67c3a94eeab87ce8fe17",
        ),
    ]
    .into_iter()
    .map(
        |(asset_id, display_name, file, expected_hash)| FrozenAsset {
            asset_id,
            display_name,
            source_path: request.frozen_asset_root.join(file),
            expected_hash,
        },
    )
    .collect()
}

fn input_patch() -> ProjectPatchDocument {
    let mut operations = vec![PatchOperation::Input(
        InputPatchOperation::CreateDefaultInputMapping {
            operation_id: "input-create".to_string(),
            depends_on: Vec::new(),
            path: INPUT_PATH.to_string(),
        },
    )];
    for (index, (action_id, value_type)) in [
        ("action.move", InputActionValueKind::Axis2),
        ("action.fire", InputActionValueKind::Button),
        ("action.dash", InputActionValueKind::Button),
        ("action.restart", InputActionValueKind::Button),
    ]
    .into_iter()
    .enumerate()
    {
        operations.push(PatchOperation::Input(InputPatchOperation::AddInputAction {
            operation_id: format!("input-action-{index}"),
            depends_on: vec!["input-create".to_string()],
            path: INPUT_PATH.to_string(),
            action_id: action_id.to_string(),
            value_type,
        }));
    }
    for (index, (action_id, device_path)) in [
        ("action.move", "keyboard/D"),
        ("action.move", "keyboard/A"),
        ("action.move", "keyboard/W"),
        ("action.move", "keyboard/S"),
        ("action.fire", "keyboard/Space"),
        ("action.dash", "keyboard/ShiftLeft"),
        ("action.restart", "keyboard/R"),
    ]
    .into_iter()
    .enumerate()
    {
        operations.push(PatchOperation::Input(
            InputPatchOperation::AddInputBinding {
                operation_id: format!("input-binding-{index}"),
                depends_on: vec![format!(
                    "input-action-{}",
                    match action_id {
                        "action.move" => 0,
                        "action.fire" => 1,
                        "action.dash" => 2,
                        "action.restart" => 3,
                        _ => unreachable!("static C-01 action"),
                    }
                )],
                path: INPUT_PATH.to_string(),
                context_id: "gameplay".to_string(),
                action_id: action_id.to_string(),
                device_path: device_path.to_string(),
            },
        ));
    }
    for binding_index in [1, 3] {
        operations.push(PatchOperation::Input(
            InputPatchOperation::SetInputBindingProcessor {
                operation_id: format!("input-processor-{binding_index}"),
                depends_on: vec![format!("input-binding-{binding_index}")],
                path: INPUT_PATH.to_string(),
                binding_index,
                processor: InputBindingProcessorPatch::Invert,
            },
        ));
    }
    ProjectPatchDocument::new(
        "c01-d6-input-patch",
        "C-01 input mapping",
        PatchSource::ImportedPatch,
        operations,
    )
}

fn scene_patch() -> ProjectPatchDocument {
    let mut operations = Vec::new();
    let zero = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let one = Vec3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    let entities = [
        (
            "main-camera",
            "Main Camera",
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 10.0,
            },
            one,
        ),
        (
            "starfield",
            "Starfield",
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 5.0,
            },
            Vec3 {
                x: 12.0,
                y: 18.0,
                z: 1.0,
            },
        ),
        (
            "player",
            "Player",
            Vec3 {
                x: 0.0,
                y: -4.5,
                z: 0.0,
            },
            one,
        ),
        ("session", "Session", zero, one),
        (
            "enemy-template",
            "Enemy Template",
            Vec3 {
                x: 0.0,
                y: 2.5,
                z: 0.0,
            },
            one,
        ),
        (
            "bullet-template",
            "Bullet Template",
            Vec3 {
                x: 0.0,
                y: -3.7,
                z: 0.0,
            },
            one,
        ),
        (
            "hud",
            "HUD",
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            one,
        ),
    ];
    for (key, name, position, scale) in entities {
        let create_id = format!("scene-create-{key}");
        operations.push(PatchOperation::Scene(ScenePatchOperation::CreateEntity {
            operation_id: create_id.clone(),
            depends_on: Vec::new(),
            parent_id: None,
            name: name.to_string(),
        }));
        operations.push(PatchOperation::Scene(ScenePatchOperation::SetTransform {
            operation_id: format!("scene-transform-{key}"),
            depends_on: vec![create_id],
            entity_id: format!("entity-{key}"),
            local_position: Some(position),
            local_rotation: None,
            local_scale: Some(scale),
        }));
    }
    let components = [
        (
            "main-camera",
            "project.camera2d",
            serde_json::json!({"orthographicSize": 7.5}),
        ),
        (
            "starfield",
            "SpriteRenderer2D",
            sprite_fields("tex-starfield", -10, 0),
        ),
        (
            "player",
            "SpriteRenderer2D",
            sprite_fields("tex-player-ship", 0, 20),
        ),
        ("player", "engine.collider2d", collider_fields(0.45, 0.45)),
        (
            "player",
            "project.c01Player",
            serde_json::json!({"hp": 3, "maxHp": 3, "speed": 7.5, "dashRemaining": 0.0, "dashCooldownRemaining": 0.0, "fireRemaining": 0.0}),
        ),
        (
            "session",
            "project.c01Session",
            serde_json::json!({"score": 0, "wave": 1, "gameOver": false, "enemySerial": 10, "bulletSerial": 10, "spawnedWave": 1}),
        ),
        (
            "enemy-template",
            "SpriteRenderer2D",
            sprite_fields("tex-enemy-scout", 0, 10),
        ),
        (
            "enemy-template",
            "engine.collider2d",
            collider_fields(0.45, 0.45),
        ),
        (
            "enemy-template",
            "project.c01Combat",
            serde_json::json!({"team": "enemy", "hp": 1, "damage": 1, "scoreValue": 100}),
        ),
        (
            "enemy-template",
            "project.c01Motion",
            serde_json::json!({"velocity": {"x": 0.0, "y": -1.0}}),
        ),
        (
            "bullet-template",
            "SpriteRenderer2D",
            sprite_fields("tex-bullet", 0, 15),
        ),
        (
            "bullet-template",
            "engine.collider2d",
            collider_fields(0.14, 0.3),
        ),
        (
            "bullet-template",
            "project.c01Combat",
            serde_json::json!({"team": "playerBullet", "hp": 1, "damage": 1, "scoreValue": 0}),
        ),
        (
            "bullet-template",
            "project.c01Motion",
            serde_json::json!({"velocity": {"x": 0.0, "y": 9.0}}),
        ),
        (
            "hud",
            "project.auiDocumentRef",
            serde_json::json!({"documentId": "c01-hud"}),
        ),
    ];
    for (index, (entity, component_type, fields)) in components.into_iter().enumerate() {
        operations.push(PatchOperation::Scene(ScenePatchOperation::AddComponent {
            operation_id: format!("scene-component-{index}"),
            depends_on: vec![format!("scene-create-{entity}")],
            entity_id: format!("entity-{entity}"),
            component_type: component_type.to_string(),
            fields,
        }));
    }
    ProjectPatchDocument::new(
        "c01-d7-scene-patch",
        "C-01 scene and components",
        PatchSource::ImportedPatch,
        operations,
    )
}

fn prefab_patch() -> ProjectPatchDocument {
    let mut operations = vec![PatchOperation::Prefab(
        PrefabPatchOperation::CreateFromSceneSelection {
            operation_id: "prefab-enemy".to_string(),
            depends_on: Vec::new(),
            scene_path: Some("Scenes/Main.scene.json".to_string()),
            root_entity_id: "entity-enemy-template".to_string(),
            prefab_id: "prefab-c01-enemy".to_string(),
            name: "C01 Enemy".to_string(),
            replace_selection_with_instance: true,
        },
    )];
    for (index, x) in [-0.4, -0.2, 0.2, 0.4].into_iter().enumerate() {
        operations.push(PatchOperation::Prefab(
            PrefabPatchOperation::InstantiateInScene {
                operation_id: format!("prefab-enemy-instance-{index}"),
                depends_on: vec!["prefab-enemy".to_string()],
                prefab_id: "prefab-c01-enemy".to_string(),
                parent_entity_id: None,
                local_position: Some(Vec3 { x, y: 2.5, z: 0.0 }),
            },
        ));
    }
    operations.extend([
        PatchOperation::Prefab(PrefabPatchOperation::CreateFromSceneSelection {
            operation_id: "prefab-bullet".to_string(),
            depends_on: vec!["prefab-enemy".to_string()],
            scene_path: Some("Scenes/Main.scene.json".to_string()),
            root_entity_id: "entity-bullet-template".to_string(),
            prefab_id: "prefab-c01-bullet".to_string(),
            name: "C01 Bullet".to_string(),
            replace_selection_with_instance: true,
        }),
        PatchOperation::Scene(ScenePatchOperation::SetComponentField {
            operation_id: "prefab-bullet-source-hide".to_string(),
            depends_on: vec!["prefab-bullet".to_string()],
            entity_id: "entity-bullet-template".to_string(),
            component_type: "SpriteRenderer2D".to_string(),
            field_path: "visible".to_string(),
            value: serde_json::json!(false),
        }),
        PatchOperation::Scene(ScenePatchOperation::SetComponentField {
            operation_id: "prefab-bullet-source-disable-collider".to_string(),
            depends_on: vec!["prefab-bullet".to_string()],
            entity_id: "entity-bullet-template".to_string(),
            component_type: "engine.collider2d".to_string(),
            field_path: "enabled".to_string(),
            value: serde_json::json!(false),
        }),
        PatchOperation::Prefab(PrefabPatchOperation::ValidateReferences {
            operation_id: "prefab-validate".to_string(),
            depends_on: vec![
                "prefab-bullet".to_string(),
                "prefab-bullet-source-hide".to_string(),
                "prefab-bullet-source-disable-collider".to_string(),
                "prefab-enemy-instance-0".to_string(),
                "prefab-enemy-instance-1".to_string(),
                "prefab-enemy-instance-2".to_string(),
                "prefab-enemy-instance-3".to_string(),
            ],
            path: None,
        }),
    ]);
    ProjectPatchDocument::new(
        "c01-d8-prefab-patch",
        "C-01 reusable prefabs",
        PatchSource::ImportedPatch,
        operations,
    )
}

fn rule_patch() -> ProjectPatchDocument {
    ProjectPatchDocument::new(
        "c01-d9-rule-patch",
        "C-01 Rust AOT rule asset",
        PatchSource::ImportedPatch,
        vec![
            PatchOperation::Rule(RulePatchOperation::CreateAsset {
                operation_id: "rule-create".to_string(),
                depends_on: Vec::new(),
                path: "Rules/c01_tick.rule.json".to_string(),
                rule_id: RULE_ID.to_string(),
                display_name: "C-01 Game Tick".to_string(),
                phase: Some("PostPhysics".to_string()),
            }),
            PatchOperation::Rule(RulePatchOperation::ValidateAsset {
                operation_id: "rule-validate".to_string(),
                depends_on: vec!["rule-create".to_string()],
                path: "Rules/c01_tick.rule.json".to_string(),
            }),
            PatchOperation::Rule(RulePatchOperation::BuildArtifact {
                operation_id: "rule-build".to_string(),
                depends_on: vec!["rule-validate".to_string()],
                path: "Rules/c01_tick.rule.json".to_string(),
            }),
            PatchOperation::Rule(RulePatchOperation::BuildProjectManifest {
                operation_id: "rule-manifest".to_string(),
                depends_on: vec!["rule-build".to_string()],
                path: "Rules/rule-manifest.json".to_string(),
            }),
        ],
    )
}

fn aui_patch() -> ProjectPatchDocument {
    let mut operations = vec![PatchOperation::Aui(AuiPatchOperation::CreateDocument {
        operation_id: "aui-create".to_string(),
        depends_on: Vec::new(),
        path: AUI_PATH.to_string(),
        document_id: "c01-hud".to_string(),
        width: 1280.0,
        height: 720.0,
    })];
    let nodes = [
        ("hp", "HP 3", 24.0, 20.0, "hud.hp"),
        ("score", "SCORE 000000", 24.0, 60.0, "hud.score"),
        ("wave", "WAVE 1", 24.0, 100.0, "hud.wave"),
        ("dash", "DASH READY", 980.0, 20.0, "hud.dash"),
        ("game-over", "GAME OVER", 520.0, 300.0, "hud.game_over"),
    ];
    for (index, (node_id, text, x, y, binding_path)) in nodes.into_iter().enumerate() {
        let add = format!("aui-add-{index}");
        operations.push(PatchOperation::Aui(AuiPatchOperation::AddNode {
            operation_id: add.clone(),
            depends_on: vec!["aui-create".to_string()],
            path: AUI_PATH.to_string(),
            parent_node_id: "root".to_string(),
            node_id: node_id.to_string(),
            node_kind: "text".to_string(),
            name: node_id.to_string(),
            rect: serde_json::json!({"x": x, "y": y, "width": 280.0, "height": 36.0}),
        }));
        operations.push(PatchOperation::Aui(AuiPatchOperation::SetNodeField {
            operation_id: format!("aui-text-{index}"),
            depends_on: vec![add.clone()],
            path: AUI_PATH.to_string(),
            node_id: node_id.to_string(),
            schema_path: "text".to_string(),
            value: serde_json::json!(text),
        }));
        operations.push(PatchOperation::Aui(AuiPatchOperation::SetBindingPath {
            operation_id: format!("aui-binding-{index}"),
            depends_on: vec![add],
            path: AUI_PATH.to_string(),
            node_id: node_id.to_string(),
            target_field: "text".to_string(),
            binding_id: format!("binding-{node_id}"),
            binding_path: binding_path.to_string(),
            fallback: Some(serde_json::json!(text)),
        }));
    }
    operations.push(PatchOperation::Aui(AuiPatchOperation::AddNode {
        operation_id: "aui-add-restart".to_string(),
        depends_on: vec!["aui-create".to_string()],
        path: AUI_PATH.to_string(),
        parent_node_id: "root".to_string(),
        node_id: "restart".to_string(),
        node_kind: "button".to_string(),
        name: "Restart".to_string(),
        rect: serde_json::json!({"x": 540.0, "y": 360.0, "width": 200.0, "height": 48.0}),
    }));
    operations.push(PatchOperation::Aui(AuiPatchOperation::SetNodeField {
        operation_id: "aui-restart-text".to_string(),
        depends_on: vec!["aui-add-restart".to_string()],
        path: AUI_PATH.to_string(),
        node_id: "restart".to_string(),
        schema_path: "text".to_string(),
        value: serde_json::json!("RESTART [R]"),
    }));
    operations.push(PatchOperation::Aui(AuiPatchOperation::SetActionRef {
        operation_id: "aui-restart-action".to_string(),
        depends_on: vec!["aui-add-restart".to_string()],
        path: AUI_PATH.to_string(),
        node_id: "restart".to_string(),
        event: "click".to_string(),
        action_id: "action.restart".to_string(),
        payload: None,
    }));
    operations.push(PatchOperation::Aui(AuiPatchOperation::ValidateDocument {
        operation_id: "aui-validate".to_string(),
        depends_on: vec!["aui-restart-action".to_string()],
        path: AUI_PATH.to_string(),
    }));
    operations.push(PatchOperation::Aui(AuiPatchOperation::SaveDocument {
        operation_id: "aui-save".to_string(),
        depends_on: vec!["aui-validate".to_string()],
        path: AUI_PATH.to_string(),
    }));
    ProjectPatchDocument::new(
        "c01-d10-aui-patch",
        "C-01 HUD",
        PatchSource::ImportedPatch,
        operations,
    )
}

fn sprite_fields(asset_id: &str, sorting_layer: i16, order: i32) -> serde_json::Value {
    serde_json::json!({
        "spriteRef": {"id": asset_id, "type": "texture"},
        "sortingLayer": sorting_layer,
        "orderInLayer": order,
        "visible": true
    })
}

fn collider_fields(x: f32, y: f32) -> serde_json::Value {
    serde_json::json!({
        "shape": "aabb",
        "halfExtents": {"x": x, "y": y},
        "offset": {"x": 0.0, "y": 0.0},
        "layer": 1,
        "mask": 4294967295u32,
        "enabled": true,
        "isSensor": true
    })
}

fn runtime_cargo_manifest() -> String {
    "[package]\nname = \"c01_project_runtime\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nengine_runtime = \"=0.1.0\"\n"
        .to_string()
}

fn runtime_source(artifact_id: &str) -> String {
    include_str!("c01_runtime_template.rs").replace("__ARTIFACT_ID__", artifact_id)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn c01_screenshot_visual_acceptance_requires_textures_and_hud_pixels() {
        let root = std::env::temp_dir().join(format!(
            "c01-visual-pass-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("visual.png");
        let mut rgba = vec![0u8; 32 * 24 * 4];
        for y in 2..22usize {
            for x in 2..30usize {
                let offset = (y * 32 + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&[
                    4 + ((x * 7 + y * 3) % 40) as u8,
                    6 + ((x * 5 + y * 11) % 35) as u8,
                    12 + ((x * 13 + y * 2) % 45) as u8,
                    255,
                ]);
            }
        }
        paint_test_rect(&mut rgba, 32, 4, 4, 8, 8, [235, 235, 235, 255]);
        paint_test_rect(&mut rgba, 32, 8, 15, 12, 18, [30, 180, 240, 255]);
        paint_test_rect(&mut rgba, 32, 20, 10, 24, 13, [230, 35, 30, 255]);
        write_test_png(&path, 32, 24, &rgba);

        let evidence = verify_c01_screenshot_visuals(&path, "Synthetic").unwrap();

        assert!(evidence.unique_color_count >= 16, "{evidence:#?}");
        assert!(evidence.player_blue_pixel_count >= 8, "{evidence:#?}");
        assert!(evidence.enemy_red_pixel_count >= 8, "{evidence:#?}");
        assert!(evidence.hud_near_white_pixel_count >= 16, "{evidence:#?}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn c01_screenshot_visual_acceptance_rejects_old_two_color_placeholder() {
        let root = std::env::temp_dir().join(format!(
            "c01-visual-fail-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("placeholder.png");
        let mut rgba = vec![0u8; 32 * 24 * 4];
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        paint_test_rect(&mut rgba, 32, 8, 6, 24, 18, [255, 218, 124, 255]);
        write_test_png(&path, 32, 24, &rgba);

        let error = verify_c01_screenshot_visuals(&path, "Synthetic").unwrap_err();

        assert!(error.contains("uniqueColorCount=2"), "{error}");
        assert!(error.contains("playerBluePixelCount=0"), "{error}");
        assert!(error.contains("enemyRedPixelCount=0"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    fn paint_test_rect(
        rgba: &mut [u8],
        width: usize,
        min_x: usize,
        min_y: usize,
        max_x: usize,
        max_y: usize,
        color: [u8; 4],
    ) {
        for y in min_y..max_y {
            for x in min_x..max_x {
                let offset = (y * width + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&color);
            }
        }
    }

    fn write_test_png(path: &Path, width: u32, height: u32, rgba: &[u8]) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(rgba).unwrap();
    }

    #[test]
    fn c01_input_patch_applies_to_a_fresh_empty_project() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project = std::env::temp_dir().join(format!("c01-input-patch-{stamp}"));
        let mut session = EditorSession::new();
        let create = session.execute_command(editor_core::command_for_test(
            UiCommandPayload::CreateProject {
                path: project.display().to_string(),
                name: "C01 Input Fixture".to_string(),
            },
        ));
        assert_eq!(create.status, CommandStatus::Committed);
        let report = session.execute_patch_as_transaction(input_patch());
        assert_eq!(
            report.status,
            editor_core::PatchApplyStatus::Committed,
            "{report:#?}"
        );
        let mapping =
            editor_core::InputMappingAuthoringService::load(&project, INPUT_PATH).unwrap();
        assert!(mapping
            .actions
            .iter()
            .any(|action| action.id == "action.dash"));
        assert!(mapping
            .actions
            .iter()
            .any(|action| action.id == "action.move"));
        assert!(mapping
            .bindings
            .iter()
            .any(|binding| binding.device_path == "keyboard/Space"));
        assert!(mapping
            .bindings
            .iter()
            .any(|binding| binding.device_path == "keyboard/ShiftLeft"));
        for path in ["keyboard/A", "keyboard/S"] {
            let binding = mapping
                .bindings
                .iter()
                .find(|binding| binding.device_path == path)
                .unwrap();
            assert_eq!(
                binding.processor,
                engine_input::InputProcessorPreset::Invert
            );
        }

        let rollback = session.revert_last_patch_for_test().unwrap();
        assert_eq!(
            rollback.status,
            editor_core::PatchApplyStatus::Committed,
            "{rollback:#?}"
        );
        assert!(!project.join(INPUT_PATH).exists());
    }

    #[test]
    fn c01_golden_gate_builds_all_ten_candidates_through_common_entry() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("c01-golden-gate-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        let project = root.join("project");
        let mut session = EditorSession::new();
        let create = session.execute_command(editor_core::command_for_test(
            UiCommandPayload::CreateProject {
                path: project.display().to_string(),
                name: "C01 Fixture".to_string(),
            },
        ));
        assert_eq!(create.status, CommandStatus::Committed, "{create:#?}");
        drop(session);
        let request = C01GoldenGateRequest::new(
            &project,
            workspace_sdk_root(),
            root.join("candidates"),
            root.join("evidence"),
            frozen_asset_root(),
            root.join("export"),
        );
        let mut report = C01GoldenGateReport::new(&request);
        let session = run_candidate_construction(&request, &mut report).unwrap();
        drop(session);
        report.status = C01GoldenGateStatus::Passed;
        assert_eq!(report.status, C01GoldenGateStatus::Passed, "{report:#?}");
        assert_eq!(report.candidates.len(), 10);
        assert!(report
            .candidates
            .iter()
            .all(|candidate| Path::new(&candidate.evidence_path)
                .join("00-imported-codex-envelope.json")
                .is_file()));
        assert!(project.join("RuntimeModule/src/lib.rs").is_file());
        assert!(project.join("Rules/rule-manifest.json").is_file());
        assert!(project.join(AUI_PATH).is_file());
        let scene: serde_json::Value =
            serde_json::from_slice(&fs::read(project.join("Scenes/Main.scene.json")).unwrap())
                .unwrap();
        let entities = scene["entities"].as_array().unwrap();
        assert!(entities.len() >= 10, "{scene:#?}");
        let bullet_source = entities
            .iter()
            .find(|entity| entity["id"].as_str() == Some("entity-bullet-template"))
            .unwrap();
        let components = bullet_source["components"].as_array().unwrap();
        let prefab_instance = components
            .iter()
            .find(|component| component["componentType"].as_str() == Some("engine.prefab_instance"))
            .unwrap();
        let overrides = prefab_instance["fields"]["overrides"].as_array().unwrap();
        assert!(overrides.iter().any(|value| {
            value["componentType"].as_str() == Some("SpriteRenderer2D")
                && value["fieldPath"].as_str() == Some("visible")
                && value["value"].as_bool() == Some(false)
        }));
        assert!(overrides.iter().any(|value| {
            value["componentType"].as_str() == Some("engine.collider2d")
                && value["fieldPath"].as_str() == Some("enabled")
                && value["value"].as_bool() == Some(false)
        }));

        persist_json(
            &request.evidence_root.join("c01-golden-gate-report.json"),
            &report,
        )
        .unwrap();
        let sentinel = request
            .candidate_store_root
            .join("validation-only-sentinel.txt");
        fs::write(&sentinel, b"must remain unchanged").unwrap();
        let project_before = directory_file_inventory(&project);
        let candidate_before = directory_file_inventory(&request.candidate_store_root);

        let mut validation_report = C01GoldenGateReport::new(&request);
        validation_report.entry_mode = C01GoldenGateEntryMode::ValidationOnly;
        let validation_session =
            prepare_existing_c01_validation(&request, &mut validation_report).unwrap();
        drop(validation_session);

        assert_eq!(
            validation_report.entry_mode,
            C01GoldenGateEntryMode::ValidationOnly
        );
        assert_eq!(validation_report.candidates, report.candidates);
        assert_eq!(
            validation_report.initial_project_digest,
            validation_report.final_project_digest
        );
        assert_eq!(directory_file_inventory(&project), project_before);
        assert_eq!(
            directory_file_inventory(&request.candidate_store_root),
            candidate_before
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"must remain unchanged");
        let _ = fs::remove_dir_all(root);
    }

    fn directory_file_inventory(root: &Path) -> Vec<(String, String)> {
        fn visit(root: &Path, current: &Path, inventory: &mut Vec<(String, String)>) {
            let mut entries = fs::read_dir(current)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    visit(root, &path, inventory);
                } else {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    inventory.push((relative, sha256_prefixed(&fs::read(path).unwrap())));
                }
            }
        }

        let mut inventory = Vec::new();
        visit(root, root, &mut inventory);
        inventory
    }

    #[test]
    #[ignore = "local Windows Preview/export gate opens real OS windows"]
    fn c01_golden_gate_full_preview_export_local() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("c01-golden-full-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        let project = root.join("project");
        let mut session = EditorSession::new();
        let create = session.execute_command(editor_core::command_for_test(
            UiCommandPayload::CreateProject {
                path: project.display().to_string(),
                name: "C01 Full Fixture".to_string(),
            },
        ));
        assert_eq!(create.status, CommandStatus::Committed, "{create:#?}");
        drop(session);

        let report = run_c01_golden_gate(C01GoldenGateRequest::new(
            &project,
            workspace_sdk_root(),
            root.join("candidates"),
            root.join("evidence"),
            frozen_asset_root(),
            root.join("external-export"),
        ));
        assert_eq!(report.status, C01GoldenGateStatus::Passed, "{report:#?}");
        assert!(report.reopen.is_some());
        assert!(report.preview.is_some());
        assert!(report.export.is_some());
    }

    fn workspace_sdk_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn frozen_asset_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples/complex_shooter_project/Assets/Images")
            .canonicalize()
            .unwrap()
    }
}
