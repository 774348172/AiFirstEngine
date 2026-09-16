use crate::{
    run_c01_golden_gate, C01GoldenGateEntryMode, C01GoldenGateReport, C01GoldenGateRequest,
    C01GoldenGateStatus,
};
use editor_core::{
    CommandStatus, EditorSession, ProjectCandidateEntry, PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION,
};
use editor_ui_model::UiCommandPayload;
use engine_runtime::canonical_digest::{canonical_json_bytes, sha256_prefixed};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const C01_FROM_BLANK_CREATION_REPORT_SCHEMA_VERSION: &str = "c01-from-blank-creation-report.v1";
pub const C01_FROM_BLANK_CREATION_ENTRY_MODE: &str = "creation_mode";
pub const C01_FROM_BLANK_PROVIDER_MODE: &str = "provider_independent_imported_codex";

const C01_FEATURE_SPEC: &str = r#"C-01 2D Combat Arena Vertical Slice v1
1280x720; player movement and clamp; 300ms 3x dash with 2s cooldown;
6 shots/s; reusable enemy and bullet prefabs; collision, HP, score, waves;
game over and restart; runtime AUI HUD; save/reopen; preview; Windows external delivery;
four frozen textures; 8/8 transitions; 120 warmup and 600 performance samples."#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01FromBlankCreationRequest {
    pub project_root: PathBuf,
    pub project_name: String,
    pub engine_sdk_root: PathBuf,
    pub candidate_store_root: PathBuf,
    pub evidence_root: PathBuf,
    pub frozen_asset_root: PathBuf,
    pub external_export_root: PathBuf,
    pub approved_by: String,
    #[serde(default)]
    pub prior_attempt_report: Option<PathBuf>,
}

impl C01FromBlankCreationRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_root: impl Into<PathBuf>,
        project_name: impl Into<String>,
        engine_sdk_root: impl Into<PathBuf>,
        candidate_store_root: impl Into<PathBuf>,
        evidence_root: impl Into<PathBuf>,
        frozen_asset_root: impl Into<PathBuf>,
        external_export_root: impl Into<PathBuf>,
        approved_by: impl Into<String>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            project_name: project_name.into(),
            engine_sdk_root: engine_sdk_root.into(),
            candidate_store_root: candidate_store_root.into(),
            evidence_root: evidence_root.into(),
            frozen_asset_root: frozen_asset_root.into(),
            external_export_root: external_export_root.into(),
            approved_by: approved_by.into(),
            prior_attempt_report: None,
        }
    }

    pub fn with_prior_attempt_report(mut self, path: impl Into<PathBuf>) -> Self {
        self.prior_attempt_report = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum C01FromBlankCreationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01FromBlankPreflightEvidence {
    pub project_root_existed_before: bool,
    pub candidate_store_existed_before: bool,
    pub evidence_root_existed_before: bool,
    pub external_export_existed_before: bool,
    pub roots_disjoint: bool,
    pub formal_project_create: String,
    pub initial_empty_project_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01GoalApprovalEvidence {
    pub feature_spec_digest: String,
    pub approval_digest: String,
    pub approved_by: String,
    pub manual_confirmation_count: u32,
    pub internal_candidate_approval_actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01EngineSourceSnapshot {
    pub digest: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01PriorAttemptEvidence {
    pub report_path: String,
    pub status: C01FromBlankCreationStatus,
    pub first_blocker: String,
    pub fresh_candidate_count: usize,
    pub total_wall_clock_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01FromBlankTimingEvidence {
    pub started_unix_ms: u64,
    pub first_playable_ms: u64,
    pub total_wall_clock_ms: u64,
    pub automation_active_ms: u64,
    pub external_wait_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct C01FromBlankCreationReport {
    pub schema_version: String,
    pub status: C01FromBlankCreationStatus,
    pub entry_mode: String,
    pub provider_mode: String,
    pub project_root: String,
    pub preflight: C01FromBlankPreflightEvidence,
    pub goal_approval: C01GoalApprovalEvidence,
    pub fresh_candidate_count: usize,
    pub reused_candidate_count: usize,
    pub repair_count: usize,
    #[serde(default)]
    pub attempt_history: Vec<C01PriorAttemptEvidence>,
    pub engine_source_before: Option<C01EngineSourceSnapshot>,
    pub engine_source_after: Option<C01EngineSourceSnapshot>,
    pub engine_source_unchanged: bool,
    pub timing: C01FromBlankTimingEvidence,
    pub c01: Option<C01GoldenGateReport>,
    pub first_blocker: Option<String>,
    pub diagnostics: Vec<String>,
    pub next_actions: Vec<String>,
}

impl C01FromBlankCreationReport {
    fn new(request: &C01FromBlankCreationRequest) -> Result<Self, String> {
        let feature_spec_digest = sha256_prefixed(C01_FEATURE_SPEC.as_bytes());
        let approval_value = serde_json::json!({
            "approvedBy": request.approved_by,
            "featureSpecDigest": feature_spec_digest,
            "projectRoot": absolute_path(&request.project_root)?.display().to_string(),
        });
        let approval_digest = digest_json(&approval_value)?;
        let actor_suffix = approval_digest
            .strip_prefix("sha256:")
            .unwrap_or(&approval_digest)
            .chars()
            .take(16)
            .collect::<String>();
        Ok(Self {
            schema_version: C01_FROM_BLANK_CREATION_REPORT_SCHEMA_VERSION.to_string(),
            status: C01FromBlankCreationStatus::Failed,
            entry_mode: C01_FROM_BLANK_CREATION_ENTRY_MODE.to_string(),
            provider_mode: C01_FROM_BLANK_PROVIDER_MODE.to_string(),
            project_root: request.project_root.display().to_string(),
            preflight: C01FromBlankPreflightEvidence {
                project_root_existed_before: request.project_root.exists(),
                candidate_store_existed_before: request.candidate_store_root.exists(),
                evidence_root_existed_before: request.evidence_root.exists(),
                external_export_existed_before: request.external_export_root.exists(),
                roots_disjoint: false,
                formal_project_create: "not_started".to_string(),
                initial_empty_project_digest: None,
            },
            goal_approval: C01GoalApprovalEvidence {
                feature_spec_digest,
                approval_digest,
                approved_by: request.approved_by.clone(),
                manual_confirmation_count: 1,
                internal_candidate_approval_actor: format!("goal-approval-{actor_suffix}"),
            },
            fresh_candidate_count: 0,
            reused_candidate_count: 0,
            repair_count: 0,
            attempt_history: Vec::new(),
            engine_source_before: None,
            engine_source_after: None,
            engine_source_unchanged: false,
            timing: C01FromBlankTimingEvidence {
                started_unix_ms: unix_ms(),
                ..C01FromBlankTimingEvidence::default()
            },
            c01: None,
            first_blocker: None,
            diagnostics: Vec::new(),
            next_actions: Vec::new(),
        })
    }
}

pub fn run_c01_from_blank_creation_gate(
    request: C01FromBlankCreationRequest,
) -> C01FromBlankCreationReport {
    let started = Instant::now();
    let mut report = match C01FromBlankCreationReport::new(&request) {
        Ok(report) => report,
        Err(error) => return minimal_failed_report(&request, started, error),
    };
    if let Err(error) = validate_preflight(&request, &mut report) {
        return finish_failed(&request, report, started, error);
    }
    if let Some(path) = &request.prior_attempt_report {
        match load_prior_attempt(path) {
            Ok(prior) => {
                report.repair_count = 1;
                report.diagnostics.push(format!(
                    "Retry is bound to preserved failed attempt evidence at {}.",
                    prior.report_path
                ));
                report.attempt_history.push(prior);
            }
            Err(error) => return finish_failed(&request, report, started, error),
        }
    }
    let source_before = match engine_source_snapshot(&request.engine_sdk_root) {
        Ok(snapshot) => snapshot,
        Err(error) => return finish_failed(&request, report, started, error),
    };
    report.engine_source_before = Some(source_before);

    let initial_digest = match create_formal_empty_project(&request) {
        Ok(digest) => digest,
        Err(error) => return finish_failed(&request, report, started, error),
    };
    report.preflight.formal_project_create = "passed".to_string();
    report.preflight.initial_empty_project_digest = Some(initial_digest);

    let c01_request = C01GoldenGateRequest::new(
        &request.project_root,
        &request.engine_sdk_root,
        &request.candidate_store_root,
        &request.evidence_root,
        &request.frozen_asset_root,
        &request.external_export_root,
    )
    .with_approval_actor(
        report
            .goal_approval
            .internal_candidate_approval_actor
            .clone(),
    );
    let c01_started_ms = elapsed_ms(started);
    let c01 = run_c01_golden_gate(c01_request);
    report.fresh_candidate_count = c01.candidates.len();
    report.repair_count = report.repair_count.saturating_add(c01.repairs);
    report.timing.first_playable_ms = c01_started_ms.saturating_add(c01.timing.first_playable_ms);
    report.timing.external_wait_ms = c01.timing.external_wait_ms;

    let nested_error = validate_nested_report(&c01).err();
    report.c01 = Some(c01);
    let source_after = match engine_source_snapshot(&request.engine_sdk_root) {
        Ok(snapshot) => snapshot,
        Err(error) => return finish_failed(&request, report, started, error),
    };
    report.engine_source_unchanged = report.engine_source_before.as_ref() == Some(&source_after);
    report.engine_source_after = Some(source_after);
    if !report.engine_source_unchanged {
        return finish_failed(
            &request,
            report,
            started,
            "Engine SDK source changed during the creation Gate.".to_string(),
        );
    }
    if let Some(error) = nested_error {
        return finish_failed(&request, report, started, error);
    }

    report.status = C01FromBlankCreationStatus::Passed;
    finish_timing(&mut report, started);
    report.next_actions.push(
        "Freeze the creation-mode project, candidate store, evidence, export and engine source snapshot."
            .to_string(),
    );
    persist_report(&request, &mut report);
    report
}

fn validate_preflight(
    request: &C01FromBlankCreationRequest,
    report: &mut C01FromBlankCreationReport,
) -> Result<(), String> {
    if request.approved_by.trim().is_empty() {
        return Err("A non-empty approved_by identity is required.".to_string());
    }
    if request.project_name.trim().is_empty() {
        return Err("A non-empty project name is required.".to_string());
    }
    for (label, path) in [
        ("project root", &request.project_root),
        ("candidate store", &request.candidate_store_root),
        ("evidence root", &request.evidence_root),
        ("external export", &request.external_export_root),
    ] {
        if path.exists() {
            return Err(format!(
                "{label} must not exist at creation Gate start: {}",
                path.display()
            ));
        }
    }
    if !request.engine_sdk_root.is_dir() {
        return Err(format!(
            "Engine SDK root is not a directory: {}",
            request.engine_sdk_root.display()
        ));
    }
    if !request.frozen_asset_root.is_dir() {
        return Err(format!(
            "Frozen asset root is not a directory: {}",
            request.frozen_asset_root.display()
        ));
    }
    let engine_sdk = absolute_path(&request.engine_sdk_root)?;
    let mutable_roots = [
        absolute_path(&request.project_root)?,
        absolute_path(&request.candidate_store_root)?,
        absolute_path(&request.evidence_root)?,
        absolute_path(&request.external_export_root)?,
    ];
    for root in &mutable_roots {
        if paths_overlap(root, &engine_sdk) {
            return Err(format!(
                "Mutable Gate root overlaps Engine SDK root: {}",
                root.display()
            ));
        }
    }
    for left in 0..mutable_roots.len() {
        for right in (left + 1)..mutable_roots.len() {
            if paths_overlap(&mutable_roots[left], &mutable_roots[right]) {
                return Err(format!(
                    "Mutable Gate roots overlap: {} and {}",
                    mutable_roots[left].display(),
                    mutable_roots[right].display()
                ));
            }
        }
    }
    report.preflight.roots_disjoint = true;
    Ok(())
}

fn create_formal_empty_project(request: &C01FromBlankCreationRequest) -> Result<String, String> {
    let mut session = EditorSession::new();
    let create = session.execute_command(editor_core::command_for_test(
        UiCommandPayload::CreateProject {
            path: request.project_root.display().to_string(),
            name: request.project_name.clone(),
        },
    ));
    if create.status != CommandStatus::Committed {
        return Err(format!(
            "Formal CreateProject failed: {:?}",
            create.diagnostics
        ));
    }
    let binding = ProjectCandidateEntry::inspect_project_binding(&session)
        .map_err(|error| format!("Created project binding inspect failed: {error}"))?;
    Ok(binding.project_digest)
}

fn validate_nested_report(report: &C01GoldenGateReport) -> Result<(), String> {
    if report.status != C01GoldenGateStatus::Passed {
        return Err(format!(
            "Nested C-01 construction failed: {}",
            report.first_blocker.as_deref().unwrap_or("unknown blocker")
        ));
    }
    if report.entry_mode != C01GoldenGateEntryMode::CandidateConstruction {
        return Err("Nested C-01 report is not candidate_construction mode.".to_string());
    }
    if report.candidates.len() != 10 {
        return Err(format!(
            "Nested C-01 report produced {} fresh candidates instead of 10.",
            report.candidates.len()
        ));
    }
    for candidate in &report.candidates {
        let envelope_path =
            Path::new(&candidate.evidence_path).join("00-imported-codex-envelope.json");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&envelope_path).map_err(|error| {
                format!(
                    "Imported candidate evidence {} read failed: {error}",
                    envelope_path.display()
                )
            })?)
            .map_err(|error| format!("Imported candidate evidence parse failed: {error}"))?;
        if value["schemaVersion"] != PROJECT_CANDIDATE_ENVELOPE_SCHEMA_VERSION
            || value["sourceKind"] != "imported_codex"
        {
            return Err(format!(
                "Candidate {} did not use the strict imported Codex envelope entry.",
                candidate.candidate_id
            ));
        }
    }
    Ok(())
}

fn load_prior_attempt(path: &Path) -> Result<C01PriorAttemptEvidence, String> {
    if !path.is_file() {
        return Err(format!(
            "Prior attempt report is not a regular file: {}",
            path.display()
        ));
    }
    let prior: C01FromBlankCreationReport = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("Prior attempt report read failed: {error}"))?,
    )
    .map_err(|error| format!("Prior attempt report parse failed: {error}"))?;
    if prior.schema_version != C01_FROM_BLANK_CREATION_REPORT_SCHEMA_VERSION
        || prior.entry_mode != C01_FROM_BLANK_CREATION_ENTRY_MODE
        || prior.status != C01FromBlankCreationStatus::Failed
    {
        return Err("Prior attempt report is not a failed creation-mode v1 report.".to_string());
    }
    let first_blocker = prior
        .first_blocker
        .ok_or_else(|| "Prior failed attempt has no first blocker.".to_string())?;
    Ok(C01PriorAttemptEvidence {
        report_path: path.display().to_string(),
        status: prior.status,
        first_blocker,
        fresh_candidate_count: prior.fresh_candidate_count,
        total_wall_clock_ms: prior.timing.total_wall_clock_ms,
    })
}

fn engine_source_snapshot(root: &Path) -> Result<C01EngineSourceSnapshot, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Engine SDK root canonicalize failed: {error}"))?;
    let mut files = Vec::new();
    collect_engine_source_files(&root, &root, &mut files)?;
    files.sort();
    let value = serde_json::to_value(&files)
        .map_err(|error| format!("Engine source inventory serialize failed: {error}"))?;
    Ok(C01EngineSourceSnapshot {
        digest: digest_json(&value)?,
        file_count: files.len(),
    })
}

fn collect_engine_source_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("Engine source directory read failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Engine source entry read failed: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("Engine source relative path failed: {error}"))?;
        if path.is_dir() {
            let first = relative.components().next().map(|value| value.as_os_str());
            if first.is_some_and(|value| value == "target") {
                continue;
            }
            collect_engine_source_files(root, &path, files)?;
            continue;
        }
        if !is_engine_source_file(relative) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| {
            format!("Engine source file {} read failed: {error}", path.display())
        })?;
        files.push((
            relative.to_string_lossy().replace('\\', "/"),
            sha256_prefixed(&bytes),
        ));
    }
    Ok(())
}

fn is_engine_source_file(relative: &Path) -> bool {
    let file_name = relative.file_name().and_then(|value| value.to_str());
    if relative.components().count() == 1 {
        return matches!(
            file_name,
            Some("Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml")
        );
    }
    let first = relative.components().next().map(|value| value.as_os_str());
    matches!(first, Some(value) if value == "crates" || value == "project_modules")
        && (relative.extension().and_then(|value| value.to_str()) == Some("rs")
            || file_name == Some("Cargo.toml"))
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|error| {
        format!(
            "Absolute path resolve failed for {}: {error}",
            path.display()
        )
    })
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

fn digest_json(value: &serde_json::Value) -> Result<String, String> {
    let bytes = canonical_json_bytes(value)
        .map_err(|error| format!("Canonical JSON digest failed: {error}"))?;
    Ok(sha256_prefixed(&bytes))
}

fn finish_failed(
    request: &C01FromBlankCreationRequest,
    mut report: C01FromBlankCreationReport,
    started: Instant,
    error: String,
) -> C01FromBlankCreationReport {
    report.first_blocker = Some(error.clone());
    report.diagnostics.push(error);
    report.next_actions.push(
        "Preserve the failed creation roots and resolve the first blocker before a fresh-root rerun."
            .to_string(),
    );
    finish_timing(&mut report, started);
    persist_report(request, &mut report);
    report
}

fn minimal_failed_report(
    request: &C01FromBlankCreationRequest,
    started: Instant,
    error: String,
) -> C01FromBlankCreationReport {
    let feature_spec_digest = sha256_prefixed(C01_FEATURE_SPEC.as_bytes());
    C01FromBlankCreationReport {
        schema_version: C01_FROM_BLANK_CREATION_REPORT_SCHEMA_VERSION.to_string(),
        status: C01FromBlankCreationStatus::Failed,
        entry_mode: C01_FROM_BLANK_CREATION_ENTRY_MODE.to_string(),
        provider_mode: C01_FROM_BLANK_PROVIDER_MODE.to_string(),
        project_root: request.project_root.display().to_string(),
        preflight: C01FromBlankPreflightEvidence {
            project_root_existed_before: request.project_root.exists(),
            candidate_store_existed_before: request.candidate_store_root.exists(),
            evidence_root_existed_before: request.evidence_root.exists(),
            external_export_existed_before: request.external_export_root.exists(),
            roots_disjoint: false,
            formal_project_create: "not_started".to_string(),
            initial_empty_project_digest: None,
        },
        goal_approval: C01GoalApprovalEvidence {
            feature_spec_digest,
            approval_digest: String::new(),
            approved_by: request.approved_by.clone(),
            manual_confirmation_count: 0,
            internal_candidate_approval_actor: String::new(),
        },
        fresh_candidate_count: 0,
        reused_candidate_count: 0,
        repair_count: 0,
        attempt_history: Vec::new(),
        engine_source_before: None,
        engine_source_after: None,
        engine_source_unchanged: false,
        timing: C01FromBlankTimingEvidence {
            started_unix_ms: unix_ms(),
            total_wall_clock_ms: elapsed_ms(started),
            ..C01FromBlankTimingEvidence::default()
        },
        c01: None,
        first_blocker: Some(error.clone()),
        diagnostics: vec![error],
        next_actions: vec!["Fix request construction before retrying.".to_string()],
    }
}

fn finish_timing(report: &mut C01FromBlankCreationReport, started: Instant) {
    report.timing.total_wall_clock_ms = elapsed_ms(started);
    report.timing.automation_active_ms = report
        .timing
        .total_wall_clock_ms
        .saturating_sub(report.timing.external_wait_ms);
}

fn persist_report(request: &C01FromBlankCreationRequest, report: &mut C01FromBlankCreationReport) {
    if report.preflight.evidence_root_existed_before {
        report.diagnostics.push(
            "Creation report was not persisted because the evidence root pre-existed the Gate."
                .to_string(),
        );
        return;
    }
    if request.evidence_root.exists() || fs::create_dir_all(&request.evidence_root).is_ok() {
        let path = request
            .evidence_root
            .join("c01-from-blank-creation-report.json");
        match serde_json::to_vec_pretty(report)
            .map_err(|error| error.to_string())
            .and_then(|bytes| fs::write(&path, bytes).map_err(|error| error.to_string()))
        {
            Ok(()) => {}
            Err(error) => report
                .diagnostics
                .push(format!("Creation report persist failed: {error}")),
        }
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_request(root: &Path) -> C01FromBlankCreationRequest {
        let sdk = root.join("sdk");
        fs::create_dir_all(sdk.join("crates/fixture/src")).unwrap();
        fs::write(sdk.join("Cargo.toml"), b"[workspace]\n").unwrap();
        fs::write(sdk.join("Cargo.lock"), b"version = 4\n").unwrap();
        fs::write(
            sdk.join("crates/fixture/Cargo.toml"),
            b"[package]\nname='fixture'\nversion='0.1.0'\n",
        )
        .unwrap();
        fs::write(
            sdk.join("crates/fixture/src/lib.rs"),
            b"pub fn fixture() {}\n",
        )
        .unwrap();
        let frozen = root.join("frozen");
        fs::create_dir_all(&frozen).unwrap();
        C01FromBlankCreationRequest::new(
            root.join("project"),
            "From Blank Fixture",
            sdk,
            root.join("candidates"),
            root.join("evidence"),
            frozen,
            root.join("export"),
            "test-user",
        )
    }

    fn temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{label}-{stamp}"))
    }

    #[test]
    fn c01_from_blank_preflight_requires_absent_disjoint_roots() {
        let root = temp_root("c01-from-blank-preflight");
        let request = fixture_request(&root);
        let mut report = C01FromBlankCreationReport::new(&request).unwrap();
        validate_preflight(&request, &mut report).unwrap();
        assert!(report.preflight.roots_disjoint);

        fs::create_dir_all(&request.project_root).unwrap();
        let mut report = C01FromBlankCreationReport::new(&request).unwrap();
        let error = validate_preflight(&request, &mut report).unwrap_err();
        assert!(error.contains("project root must not exist"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn c01_from_blank_formal_create_produces_ready_empty_project() {
        let root = temp_root("c01-from-blank-create");
        let request = fixture_request(&root);
        let mut report = C01FromBlankCreationReport::new(&request).unwrap();
        validate_preflight(&request, &mut report).unwrap();
        let before = engine_source_snapshot(&request.engine_sdk_root).unwrap();
        let digest = create_formal_empty_project(&request).unwrap();
        let after = engine_source_snapshot(&request.engine_sdk_root).unwrap();
        assert!(request.project_root.join("project.aife.json").is_file());
        assert!(digest.starts_with("sha256:"));
        assert_eq!(before, after);
        let _ = fs::remove_dir_all(root);
    }
}
