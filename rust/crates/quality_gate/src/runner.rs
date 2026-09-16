use crate::architecture_artifact::{
    verify_artifact, ArchitectureReviewArtifact, ArtifactExpectation,
};
use crate::architecture_coverage::{read_coverage, CoverageStatus};
use crate::architecture_debt::{read_debt_ledger, DebtStatus};
use crate::architecture_inventory::build_inventory;
use crate::architecture_policy::{load_policy, ArchitectureDiagnostic};
use crate::cargo_json::parse_cargo_json;
use crate::command::{QualityCommandExecutor, QualityCommandOutcome, QualityCommandSpec};
use crate::lint_ledger::{read_ledger, utc_today};
use crate::report::{
    CiEvidence, HygieneEvidence, QualityGateReport, QualityProfile, QualityStageReport, ReportMode,
    StageStatus,
};
use crate::suppression::{reconcile_suppressions, scan_workspace_suppressions};
use crate::toolchain::{evaluate_toolchain, ToolchainOutput};
use crate::workspace::{audit_workspace_lints, combined_digest, file_digest, resolve_report_path};
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SHORT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const LONG_TIMEOUT: Duration = Duration::from_secs(45 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityGateRequest {
    pub workspace_root: PathBuf,
    pub report_mode: ReportMode,
    pub fail_fast: bool,
    pub report_output: Option<PathBuf>,
    pub profile: QualityProfile,
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub dirty_patch_digest: Option<String>,
    pub architecture_artifact: Option<PathBuf>,
    pub artifact_expectation: Option<ArtifactExpectation>,
}

impl QualityGateRequest {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            report_mode: ReportMode::Summary,
            fail_fast: false,
            report_output: None,
            profile: QualityProfile::Fast,
            base_commit: None,
            head_commit: None,
            dirty_patch_digest: None,
            architecture_artifact: None,
            artifact_expectation: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QualityGateRunner<E> {
    executor: E,
}

impl<E> QualityGateRunner<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }
}

impl<E: QualityCommandExecutor> QualityGateRunner<E> {
    pub fn verify(&self, request: QualityGateRequest) -> QualityGateReport {
        let started = Instant::now();
        let mut report = QualityGateReport {
            report_mode: request.report_mode,
            started_at_epoch_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ..QualityGateReport::default()
        };
        report.architecture.profile = request.profile;
        report.fail_fast = request.fail_fast;
        let workspace_root = request.workspace_root.clone();
        let report_path =
            match resolve_report_path(&workspace_root, request.report_output.as_deref()) {
                Ok(path) => Some(path),
                Err(error) => {
                    report.push_diagnostic(
                        "quality_gate.report_write_failed",
                        error,
                        "Choose an output below the workspace target directory.",
                    );
                    None
                }
            };

        let lockfile = workspace_root.join("Cargo.lock");
        let toolchain_file = workspace_root.join("rust-toolchain.toml");
        let ledger_path = workspace_root.join("quality/lint-debt-ledger.v1.json");
        report.lockfile_digest = required_digest(
            &lockfile,
            "quality_gate.lockfile_changed",
            "Restore the committed Cargo.lock before verification.",
            &mut report,
        );
        report.toolchain_file_digest = required_digest(
            &toolchain_file,
            "quality_gate.toolchain_mismatch",
            "Restore the committed rust-toolchain.toml.",
            &mut report,
        );
        report.ledger_digest = required_digest(
            &ledger_path,
            "quality_gate.lint_entry_stale",
            "Restore or repair the committed lint debt ledger.",
            &mut report,
        );
        if !report.diagnostics.is_empty() {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        report.workspace_state = self.workspace_state(&workspace_root);
        report.workspace_commit_sha = self.workspace_commit_sha(&workspace_root);
        let toolchain_ok = self.verify_toolchain(&workspace_root, &mut report);
        if !toolchain_ok {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        let ledger = match read_ledger(&ledger_path) {
            Ok(ledger) => ledger,
            Err(error) => {
                report.push_diagnostic(
                    "quality_gate.lint_entry_stale",
                    format!("failed to read lint ledger: {error}"),
                    "Repair the committed ledger or replace it with a reviewed candidate.",
                );
                return finish_report(report, started, &workspace_root, report_path.as_deref());
            }
        };

        let (mut metadata_stage, metadata) = self.execute(
            command(
                "workspace_metadata",
                "cargo",
                ["metadata", "--locked", "--format-version", "1", "--no-deps"],
                &workspace_root,
                SHORT_TIMEOUT,
            ),
            request.report_mode,
        );
        if metadata.passed() {
            match audit_workspace_lints(&workspace_root, &metadata.stdout) {
                Ok(audit) => {
                    report.manifest_set_digest = audit.manifest_set_digest;
                    if !audit.diagnostics.is_empty() {
                        metadata_stage.status = StageStatus::Failed;
                        metadata_stage.next_action = Some(
                            "Make every workspace member inherit [workspace.lints].".to_string(),
                        );
                        report.diagnostics.extend(audit.diagnostics);
                    }
                }
                Err(error) => {
                    metadata_stage.status = StageStatus::Failed;
                    report.push_diagnostic(
                        "quality_gate.workspace_lint_not_inherited",
                        error,
                        "Repair Cargo metadata or workspace lint declarations.",
                    );
                }
            }
        } else {
            push_command_diagnostic(&metadata_stage, &metadata, &mut report);
        }
        report.stages.push(metadata_stage);
        if should_fail_fast(&request, &report) {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        if metadata.passed() {
            self.verify_architecture(&request, &workspace_root, &metadata.stdout, &mut report);
        } else if request.profile != QualityProfile::Fast {
            report
                .architecture_diagnostics
                .push(architecture_diagnostic(
                    "architecture_gate.metadata_unavailable",
                    "Cargo metadata failed before architecture inventory",
                    "Repair workspace metadata and rerun the architecture gate.",
                ));
            report.stages.push(architecture_stage(false));
        }
        if should_fail_fast(&request, &report) {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        let format_passed = self.run_simple_stage(
            command(
                "format",
                "cargo",
                ["fmt", "--all", "--", "--check"],
                &workspace_root,
                SHORT_TIMEOUT,
            ),
            request.report_mode,
            &mut report,
        );
        if request.fail_fast && !format_passed {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        let today = utc_today();
        match scan_workspace_suppressions(&workspace_root) {
            Ok(observed) => {
                let reconciliation =
                    reconcile_suppressions(&workspace_root, &ledger, &observed, &today);
                reconciliation.apply_to_summary(&mut report.lint);
                report.stages.push(QualityStageReport {
                    id: "source_suppression".to_string(),
                    command_id: None,
                    status: if reconciliation.passed() {
                        StageStatus::Passed
                    } else {
                        StageStatus::Failed
                    },
                    exit_code: None,
                    duration_ms: 0,
                    timed_out: false,
                    output_truncated: false,
                    next_action: (!reconciliation.passed()).then(|| {
                        "Remove unmanaged suppressions or submit a reviewed exception.".to_string()
                    }),
                    trace: None,
                });
                report.diagnostics.extend(reconciliation.diagnostics);
            }
            Err(error) => {
                report.stages.push(manual_failed_stage(
                    "source_suppression",
                    "Repair Rust syntax before suppression inventory.",
                ));
                report.push_diagnostic(
                    "quality_gate.suppression_unregistered",
                    error,
                    "Repair Rust syntax before suppression inventory.",
                );
            }
        }
        if should_fail_fast(&request, &report) {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        let (mut clippy_stage, clippy) = self.execute(
            command(
                "clippy",
                "cargo",
                [
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--locked",
                    "--message-format=json",
                ],
                &workspace_root,
                LONG_TIMEOUT,
            ),
            request.report_mode,
        );
        if clippy.passed() {
            match parse_cargo_json(&clippy.stdout) {
                Ok(observed) => {
                    let reconciliation = crate::lint_ledger::reconcile_lints(
                        &workspace_root,
                        &report.toolchain.expected_release,
                        &ledger,
                        &observed,
                        &today,
                    );
                    report.lint.observed_occurrences = reconciliation.summary.observed_occurrences;
                    report.lint.known_entries = reconciliation.summary.known_entries;
                    report.lint.new_entries = reconciliation.summary.new_entries;
                    report.lint.resolved_entries = reconciliation.summary.resolved_entries;
                    report.lint.stale_entries = reconciliation.summary.stale_entries;
                    let reconciliation_passed = reconciliation.passed();
                    report.lint_items =
                        lint_items_for_report(request.report_mode, reconciliation.items);
                    if !reconciliation_passed {
                        clippy_stage.status = StageStatus::Failed;
                        clippy_stage.next_action = Some(
                            "Fix warnings or review a generated ledger candidate.".to_string(),
                        );
                    }
                    report.diagnostics.extend(reconciliation.diagnostics);
                }
                Err(error) => {
                    clippy_stage.status = StageStatus::Failed;
                    report.push_diagnostic(
                        "quality_gate.cargo_json_malformed",
                        error,
                        "Run Clippy with Cargo JSON output and repair malformed diagnostics.",
                    );
                }
            }
        } else {
            push_command_diagnostic(&clippy_stage, &clippy, &mut report);
        }
        report.stages.push(clippy_stage);
        if should_fail_fast(&request, &report) {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        let default_passed = self.run_simple_stage(
            workspace_test_command(
                "default_workspace",
                ["test", "--workspace", "--locked"],
                &workspace_root,
            ),
            request.report_mode,
            &mut report,
        );
        report.test_matrix.default_workspace_passed = default_passed;
        if request.fail_fast && !default_passed {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }
        let all_features_passed = self.run_simple_stage(
            workspace_test_command(
                "all_features_workspace",
                ["test", "--workspace", "--all-features", "--locked"],
                &workspace_root,
            ),
            request.report_mode,
            &mut report,
        );
        report.test_matrix.all_features_workspace_passed = all_features_passed;
        if request.fail_fast && !all_features_passed {
            return finish_report(report, started, &workspace_root, report_path.as_deref());
        }

        let (mut hygiene_stage, hygiene) = self.execute(
            command(
                "code_hygiene",
                "cargo",
                [
                    "run",
                    "-p",
                    "code_hygiene",
                    "--locked",
                    "--",
                    "--root",
                    "crates",
                ],
                &workspace_root,
                SHORT_TIMEOUT,
            ),
            request.report_mode,
        );
        if hygiene.passed() {
            match parse_hygiene_evidence(&hygiene.stdout) {
                Ok(evidence) => {
                    report.hygiene_summary = evidence;
                    report.test_matrix.hygiene_evidence_generated = true;
                }
                Err(error) => {
                    hygiene_stage.status = StageStatus::Failed;
                    report.push_diagnostic(
                        "quality_gate.command_failed",
                        format!("invalid code_hygiene evidence: {error}"),
                        "Repair code_hygiene.report.v2 serialization.",
                    );
                }
            }
        } else {
            push_command_diagnostic(&hygiene_stage, &hygiene, &mut report);
        }
        report.stages.push(hygiene_stage);

        let lockfile_digest_before = report.lockfile_digest.clone();
        let toolchain_digest_before = report.toolchain_file_digest.clone();
        let ledger_digest_before = report.ledger_digest.clone();
        verify_digest_unchanged(
            &lockfile,
            &lockfile_digest_before,
            "quality_gate.lockfile_changed",
            "Cargo.lock changed during verify; restore it and rerun with --locked.",
            &mut report,
        );
        verify_digest_unchanged(
            &toolchain_file,
            &toolchain_digest_before,
            "quality_gate.toolchain_mismatch",
            "rust-toolchain.toml changed during verify; restore it and rerun.",
            &mut report,
        );
        verify_digest_unchanged(
            &ledger_path,
            &ledger_digest_before,
            "quality_gate.ledger_mutated_by_verify",
            "Restore the ledger and use propose-ledger for reviewed changes.",
            &mut report,
        );

        let workspace_commit = report
            .workspace_commit_sha
            .as_deref()
            .unwrap_or("uncommitted");
        report.workspace_digest = combined_digest(&[
            workspace_commit,
            &report.lockfile_digest,
            &report.toolchain_file_digest,
            &report.manifest_set_digest,
            &report.ledger_digest,
        ]);
        finish_report(report, started, &workspace_root, report_path.as_deref())
    }

    fn verify_architecture(
        &self,
        request: &QualityGateRequest,
        workspace_root: &Path,
        metadata: &[u8],
        report: &mut QualityGateReport,
    ) {
        if request.profile == QualityProfile::Fast {
            report.architecture.artifact_status = "not_required".to_string();
            report.architecture.final_decision = "fast_pass".to_string();
            report.stages.push(architecture_stage(true));
            return;
        }
        let profile_id = match request.profile {
            QualityProfile::Fast => unreachable!(),
            QualityProfile::EngineStrict => "engine-strict",
            QualityProfile::ProjectAdvisory => "project-advisory",
            QualityProfile::ProjectStrict => "project-strict",
        };
        report.architecture.base_commit = request.base_commit.clone();
        report.architecture.head_commit = request.head_commit.clone();
        let policy_path = workspace_root.join("quality/architecture-policy.v1.toml");
        let debt_path = workspace_root.join("quality/architecture-debt-ledger.v1.json");
        let coverage_path = workspace_root.join("quality/architecture-review-coverage.v1.json");

        let policy = match load_policy(&policy_path) {
            Ok(policy) => policy,
            Err(error) => {
                report.architecture_diagnostics.extend(error.diagnostics);
                report.architecture.final_decision = "failed".to_string();
                report.stages.push(architecture_stage(false));
                return;
            }
        };
        report.architecture.policy_digest = match file_digest(&policy_path) {
            Ok(digest) => digest,
            Err(error) => {
                report
                    .architecture_diagnostics
                    .push(architecture_diagnostic(
                        "architecture_gate.policy_digest_failed",
                        error,
                        "Restore the committed architecture policy.",
                    ));
                String::new()
            }
        };
        let inventory = match build_inventory(workspace_root, &policy, profile_id, metadata) {
            Ok(inventory) => inventory,
            Err(diagnostics) => {
                report.architecture_diagnostics.extend(diagnostics);
                report.architecture.final_decision = "failed".to_string();
                report.stages.push(architecture_stage(false));
                return;
            }
        };
        report.architecture.inventory_digest = inventory.digest.clone();
        report
            .architecture_diagnostics
            .extend(inventory.diagnostics.clone());

        match read_debt_ledger(&debt_path, &utc_today()) {
            Ok(ledger) => {
                report.architecture.active_debt = ledger
                    .entries
                    .iter()
                    .filter(|entry| entry.status == DebtStatus::Active)
                    .count();
                report.architecture.debt_ledger_digest =
                    file_digest(&debt_path).unwrap_or_default();
            }
            Err(error) => report
                .architecture_diagnostics
                .push(architecture_diagnostic(
                    "architecture_gate.debt_invalid",
                    error,
                    "Repair the committed architecture debt ledger.",
                )),
        }
        match read_coverage(&coverage_path) {
            Ok(coverage) => {
                report.architecture.coverage_pending = coverage
                    .entries
                    .iter()
                    .filter(|entry| entry.status == CoverageStatus::CoveragePending)
                    .count();
                report.architecture.coverage_ledger_digest =
                    file_digest(&coverage_path).unwrap_or_default();
            }
            Err(error) => report
                .architecture_diagnostics
                .push(architecture_diagnostic(
                    "architecture_gate.coverage_invalid",
                    error,
                    "Repair the committed architecture review coverage ledger.",
                )),
        }

        let strict = matches!(
            request.profile,
            QualityProfile::EngineStrict | QualityProfile::ProjectStrict
        );
        if strict
            && (!request.base_commit.as_deref().is_some_and(exact_commit)
                || !request.head_commit.as_deref().is_some_and(exact_commit))
        {
            report
                .architecture_diagnostics
                .push(architecture_diagnostic(
                    "architecture_gate.exact_commit_missing",
                    "strict profile requires exact 40-character base/head commits",
                    "Resolve exact local Git commits before architecture verification.",
                ));
        }
        match request.architecture_artifact.as_deref() {
            None if strict => {
                report.architecture.artifact_status = "missing".to_string();
                report.architecture.next_action =
                    Some("Generate a trusted exact-candidate architecture artifact.".to_string());
                report
                    .architecture_diagnostics
                    .push(architecture_diagnostic(
                        "architecture_gate.artifact_missing",
                        "strict profile requires an architecture review artifact",
                        "Run architecture_review with a trusted Provider.",
                    ));
            }
            None => {
                report.architecture.artifact_status = "off".to_string();
                report.architecture.final_decision = "advisory_without_provider".to_string();
            }
            Some(path) => {
                let absolute = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    workspace_root.join(path)
                };
                if !absolute.starts_with(workspace_root.join("target/quality-gate")) {
                    report
                        .architecture_diagnostics
                        .push(architecture_diagnostic(
                            "architecture_gate.artifact_path_forbidden",
                            "artifact path is outside target/quality-gate",
                            "Use a confined generated artifact path.",
                        ));
                } else {
                    match (fs::read(&absolute), request.artifact_expectation.as_ref()) {
                        (Ok(bytes), Some(expectation)) => {
                            match serde_json::from_slice::<ArchitectureReviewArtifact>(&bytes) {
                                Ok(artifact) => {
                                    let verification = verify_artifact(
                                        &artifact,
                                        expectation,
                                        &inventory,
                                        &policy,
                                    );
                                    report.architecture.finding_count = artifact.findings.len();
                                    report.architecture.disposition_count =
                                        artifact.dispositions.len();
                                    report.architecture.artifact_status = if verification.passed() {
                                        "verified".to_string()
                                    } else {
                                        "invalid".to_string()
                                    };
                                    report
                                        .architecture_diagnostics
                                        .extend(verification.diagnostics);
                                    report
                                        .architecture_diagnostics
                                        .extend(verification.review.diagnostics);
                                }
                                Err(error) => {
                                    report
                                        .architecture_diagnostics
                                        .push(architecture_diagnostic(
                                            "architecture_gate.artifact_invalid",
                                            error.to_string(),
                                            "Repair the strict architecture artifact schema.",
                                        ))
                                }
                            }
                        }
                        (Err(error), _) => {
                            report
                                .architecture_diagnostics
                                .push(architecture_diagnostic(
                                    "architecture_gate.artifact_read_failed",
                                    error.to_string(),
                                    "Generate the expected architecture artifact.",
                                ))
                        }
                        (_, None) => report
                            .architecture_diagnostics
                            .push(architecture_diagnostic(
                                "architecture_gate.expectation_missing",
                                "artifact verification expectation is missing",
                                "Bind the artifact to exact commits and configuration digests.",
                            )),
                    }
                }
            }
        }
        let passed = report.architecture_diagnostics.is_empty()
            && (!strict || report.architecture.artifact_status == "verified");
        if report.architecture.final_decision.is_empty() {
            report.architecture.final_decision = if passed {
                "passed".to_string()
            } else {
                "failed".to_string()
            };
        }
        report.stages.push(architecture_stage(passed));
    }

    fn verify_toolchain(&self, root: &Path, report: &mut QualityGateReport) -> bool {
        let commands = [
            ("toolchain_rustc", "rustc", vec!["-Vv"]),
            ("toolchain_cargo", "cargo", vec!["-V"]),
            ("toolchain_clippy", "cargo", vec!["clippy", "-V"]),
            ("toolchain_rustfmt", "rustfmt", vec!["-V"]),
        ];
        let mut output = ToolchainOutput::default();
        let mut all_commands_passed = true;
        for (id, program, args) in commands {
            let (stage, outcome) = self.execute(
                command(id, program, args, root, SHORT_TIMEOUT),
                report.report_mode,
            );
            if !outcome.passed() {
                all_commands_passed = false;
                push_command_diagnostic(&stage, &outcome, report);
            }
            let text = String::from_utf8_lossy(&outcome.stdout).to_string();
            match id {
                "toolchain_rustc" => output.rustc = text,
                "toolchain_cargo" => output.cargo = text,
                "toolchain_clippy" => output.clippy = text,
                "toolchain_rustfmt" => output.rustfmt = text,
                _ => {}
            }
            report.stages.push(stage);
        }
        let (evidence, diagnostics) = evaluate_toolchain(output);
        report.toolchain = evidence;
        report.diagnostics.extend(diagnostics);
        all_commands_passed && report.toolchain.matched
    }

    fn workspace_state(&self, root: &Path) -> String {
        let (_, outcome) = self.execute(
            command(
                "git_status",
                "git",
                ["status", "--porcelain"],
                root,
                Duration::from_secs(30),
            ),
            ReportMode::Summary,
        );
        if !outcome.passed() {
            "unknown".to_string()
        } else if outcome.stdout.is_empty() {
            "clean".to_string()
        } else {
            "dirty".to_string()
        }
    }

    fn workspace_commit_sha(&self, root: &Path) -> Option<String> {
        let (_, outcome) = self.execute(
            command(
                "git_commit",
                "git",
                ["rev-parse", "HEAD"],
                root,
                Duration::from_secs(30),
            ),
            ReportMode::Summary,
        );
        outcome
            .passed()
            .then(|| String::from_utf8_lossy(&outcome.stdout).trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn run_simple_stage(
        &self,
        spec: QualityCommandSpec,
        mode: ReportMode,
        report: &mut QualityGateReport,
    ) -> bool {
        let (stage, outcome) = self.execute(spec, mode);
        let passed = outcome.passed();
        if !passed {
            push_command_diagnostic(&stage, &outcome, report);
        }
        report.stages.push(stage);
        passed
    }

    fn execute(
        &self,
        spec: QualityCommandSpec,
        mode: ReportMode,
    ) -> (QualityStageReport, QualityCommandOutcome) {
        let outcome = self.executor.execute(&spec);
        let status = if outcome.timed_out {
            StageStatus::TimedOut
        } else if outcome.passed() {
            StageStatus::Passed
        } else {
            StageStatus::Failed
        };
        let trace = (mode == ReportMode::Trace).then(|| {
            if outcome.passed() {
                format!(
                    "stdout_bytes={}; stderr_bytes={}; spawn_error={}",
                    outcome.stdout.len(),
                    outcome.stderr.len(),
                    outcome.spawn_error.is_some()
                )
            } else {
                sanitized_failure_trace(&spec, &outcome)
            }
        });
        (
            QualityStageReport {
                id: spec.id.clone(),
                command_id: Some(spec.id.clone()),
                status,
                exit_code: outcome.exit_code,
                duration_ms: millis(outcome.duration),
                timed_out: outcome.timed_out,
                output_truncated: outcome.output_truncated,
                next_action: (!outcome.passed()).then(|| {
                    let detail = compact_failure_summary(&spec, &outcome);
                    if detail.is_empty() {
                        format!("Run and repair the {} stage before continuing.", spec.id)
                    } else {
                        format!(
                            "Run and repair the {} stage before continuing. {detail}",
                            spec.id
                        )
                    }
                }),
                trace,
            },
            outcome,
        )
    }
}

fn command<I, S>(
    id: impl Into<String>,
    program: impl Into<String>,
    args: I,
    root: &Path,
    timeout: Duration,
) -> QualityCommandSpec
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    QualityCommandSpec::new(id, program, args, root, timeout)
}

fn workspace_test_command<I, S>(id: impl Into<String>, args: I, root: &Path) -> QualityCommandSpec
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut spec = command(id, "cargo", args, root, LONG_TIMEOUT);
    spec.environment.push((
        "CARGO_TARGET_DIR".to_string(),
        workspace_test_target_dir(root, std::env::var_os("CARGO_TARGET_DIR").as_deref())
            .to_string_lossy()
            .to_string(),
    ));
    spec
}

fn workspace_test_target_dir(root: &Path, configured: Option<&OsStr>) -> PathBuf {
    let base = configured
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| root.join("target/quality-gate"));
    base.join("workspace-tests-target")
}

fn required_digest(
    path: &Path,
    code: &str,
    next_action: &str,
    report: &mut QualityGateReport,
) -> String {
    match file_digest(path) {
        Ok(digest) => digest,
        Err(error) => {
            report.push_diagnostic(code, error, next_action);
            String::new()
        }
    }
}

fn verify_digest_unchanged(
    path: &Path,
    before: &str,
    code: &str,
    next_action: &str,
    report: &mut QualityGateReport,
) {
    match file_digest(path) {
        Ok(after) if after == before => {}
        Ok(_) => report.push_diagnostic(
            code,
            format!(
                "{} changed during verify",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("contract file")
            ),
            next_action,
        ),
        Err(error) => report.push_diagnostic(code, error, next_action),
    }
}

fn finish_report(
    mut report: QualityGateReport,
    started: Instant,
    workspace_root: &Path,
    report_path: Option<&Path>,
) -> QualityGateReport {
    report.duration_ms = millis(started.elapsed());
    let ci_context = ci_runtime_context(workspace_root);
    if ci_context.authoritative
        && ci_context.commit_sha.as_deref() != report.workspace_commit_sha.as_deref()
    {
        report.push_diagnostic(
            "quality_gate.ci_commit_mismatch",
            "CI commit does not match the checked-out workspace commit",
            "Run the quality gate against the exact checked-out commit.",
        );
    }
    report.passed = report.diagnostics.is_empty()
        && report.architecture_diagnostics.is_empty()
        && report
            .stages
            .iter()
            .all(|stage| stage.status == StageStatus::Passed);
    report.ci = ci_evidence(ci_context, report.passed);
    if let Some(path) = report_path {
        if let Err(error) = write_report(path, &report) {
            report.passed = false;
            report.push_diagnostic(
                "quality_gate.report_write_failed",
                error,
                "Create the target report directory and retry.",
            );
        }
    }
    report
}

fn architecture_stage(passed: bool) -> QualityStageReport {
    QualityStageReport {
        id: "architecture_gate".to_string(),
        command_id: None,
        status: if passed {
            StageStatus::Passed
        } else {
            StageStatus::Failed
        },
        exit_code: None,
        duration_ms: 0,
        timed_out: false,
        output_truncated: false,
        next_action: (!passed)
            .then(|| "Inspect architecture_diagnostics and repair the first failure.".to_string()),
        trace: None,
    }
}

fn architecture_diagnostic(
    code: impl Into<String>,
    evidence: impl Into<String>,
    next_action: impl Into<String>,
) -> ArchitectureDiagnostic {
    ArchitectureDiagnostic {
        code: code.into(),
        source_path: None,
        domain: None,
        subject: None,
        stage: "architecture_gate".to_string(),
        observed_evidence: evidence.into(),
        rule_id: None,
        classification: "failed".to_string(),
        next_action: next_action.into(),
    }
}

fn exact_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn write_report(path: &Path, report: &QualityGateReport) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "report path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
    json.push('\n');
    fs::write(path, json).map_err(|error| error.to_string())
}

fn push_command_diagnostic(
    stage: &QualityStageReport,
    outcome: &QualityCommandOutcome,
    report: &mut QualityGateReport,
) {
    let code = if outcome.timed_out {
        "quality_gate.command_timed_out"
    } else if stage.id == "default_workspace" {
        "quality_gate.default_workspace_failed"
    } else if stage.id == "all_features_workspace" {
        "quality_gate.all_features_workspace_failed"
    } else {
        "quality_gate.command_failed"
    };
    let message = if let Some(error) = &outcome.spawn_error {
        format!("{} could not start: {error}", stage.id)
    } else {
        format!("{} failed with exit code {:?}", stage.id, outcome.exit_code)
    };
    report.push_diagnostic(
        code,
        message,
        format!("Run and repair the {} stage.", stage.id),
    );
}

fn manual_failed_stage(id: &str, next_action: &str) -> QualityStageReport {
    QualityStageReport {
        id: id.to_string(),
        command_id: None,
        status: StageStatus::Failed,
        exit_code: None,
        duration_ms: 0,
        timed_out: false,
        output_truncated: false,
        next_action: Some(next_action.to_string()),
        trace: None,
    }
}

fn parse_hygiene_evidence(output: &[u8]) -> Result<HygieneEvidence, String> {
    let value: Value = serde_json::from_slice(output).map_err(|error| error.to_string())?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing schema_version".to_string())?;
    if !matches!(
        schema_version,
        "code_hygiene.report.v1" | "code_hygiene.report.v2"
    ) {
        return Err(format!(
            "unsupported code hygiene schema {schema_version:?}"
        ));
    }
    Ok(HygieneEvidence {
        schema_version: schema_version.to_string(),
        files: usize_field(&value, "files")?,
        total_lines: usize_field(&value, "total_lines")?,
        recommendation_count: value
            .get("recommendations")
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| "missing recommendations".to_string())?,
    })
}

fn usize_field(value: &Value, field: &str) -> Result<usize, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("missing or invalid {field}"))
}

struct CiRuntimeContext {
    adapter: String,
    adapter_configured: bool,
    execution_scope: String,
    run_id: Option<String>,
    commit_sha: Option<String>,
    authoritative: bool,
}

fn ci_runtime_context(workspace_root: &Path) -> CiRuntimeContext {
    if let Ok(adapter) = std::env::var("QUALITY_GATE_CI_ADAPTER") {
        return CiRuntimeContext {
            adapter,
            adapter_configured: true,
            execution_scope: std::env::var("QUALITY_GATE_CI_EXECUTION_SCOPE")
                .unwrap_or_else(|_| "local_commit".to_string()),
            run_id: std::env::var("QUALITY_GATE_CI_RUN_ID").ok(),
            commit_sha: std::env::var("QUALITY_GATE_CI_COMMIT_SHA").ok(),
            authoritative: true,
        };
    }
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        return CiRuntimeContext {
            adapter: "github-actions".to_string(),
            adapter_configured: workspace_root
                .parent()
                .is_some_and(|root| root.join(".github/workflows/rust-quality.yml").is_file()),
            execution_scope: "remote_commit".to_string(),
            run_id: std::env::var("GITHUB_RUN_ID").ok(),
            commit_sha: std::env::var("GITHUB_SHA").ok(),
            authoritative: true,
        };
    }
    CiRuntimeContext {
        adapter: "developer-worktree".to_string(),
        adapter_configured: true,
        execution_scope: "workspace".to_string(),
        run_id: None,
        commit_sha: None,
        authoritative: false,
    }
}

fn ci_evidence(context: CiRuntimeContext, passed: bool) -> CiEvidence {
    let execution_status = if !context.authoritative {
        "not_authoritative"
    } else if passed {
        "observed_commit_passed"
    } else {
        "observed_commit_failed"
    };
    CiEvidence {
        adapter: context.adapter,
        adapter_configured: context.adapter_configured,
        execution_scope: context.execution_scope,
        run_id: context.run_id,
        commit_sha: context.commit_sha,
        execution_status: execution_status.to_string(),
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn lint_items_for_report(
    mode: ReportMode,
    items: Vec<crate::report::LintItemEvidence>,
) -> Vec<crate::report::LintItemEvidence> {
    match mode {
        ReportMode::Summary => Vec::new(),
        ReportMode::Trace => items,
    }
}

fn sanitized_failure_trace(spec: &QualityCommandSpec, outcome: &QualityCommandOutcome) -> String {
    sanitize_failure_text(
        spec,
        format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&outcome.stdout),
            String::from_utf8_lossy(&outcome.stderr)
        ),
    )
}

fn should_fail_fast(request: &QualityGateRequest, report: &QualityGateReport) -> bool {
    request.fail_fast
        && report
            .stages
            .iter()
            .any(|stage| matches!(stage.status, StageStatus::Failed | StageStatus::TimedOut))
}

fn sanitize_failure_text(spec: &QualityCommandSpec, mut text: String) -> String {
    let mut replacements = vec![(
        spec.working_directory.to_string_lossy().to_string(),
        "<workspace>".to_string(),
    )];
    for (name, token) in [
        ("USERPROFILE", "<home>"),
        ("HOME", "<home>"),
        ("CARGO_HOME", "<cargo-home>"),
        ("RUSTUP_HOME", "<rustup-home>"),
        ("TEMP", "<temp>"),
        ("TMP", "<temp>"),
    ] {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() {
                replacements.push((value, token.to_string()));
            }
        }
    }
    replacements.sort_by_key(|item| std::cmp::Reverse(item.0.len()));
    for (value, token) in replacements {
        text = text.replace(&value, &token);
        text = text.replace(&value.replace('\\', "/"), &token);
    }
    text
}

fn compact_failure_summary(spec: &QualityCommandSpec, outcome: &QualityCommandOutcome) -> String {
    const MAX_LINES: usize = 6;
    const STDERR_RESERVED_LINES: usize = 2;
    const TAIL_BYTES: usize = 8 * 1024;

    let stdout = sanitize_failure_text(
        spec,
        String::from_utf8_lossy(tail(&outcome.stdout, TAIL_BYTES)).into_owned(),
    );
    let stderr = sanitize_failure_text(
        spec,
        String::from_utf8_lossy(tail(&outcome.stderr, TAIL_BYTES)).into_owned(),
    );
    let stdout_lines = compact_tail_lines(&stdout, MAX_LINES);
    let stderr_lines = compact_tail_lines(&stderr, MAX_LINES);
    let stderr_take = stderr_lines.len().min(STDERR_RESERVED_LINES);
    let stdout_take = stdout_lines.len().min(MAX_LINES - stderr_take);
    let extra_stderr_take = stderr_lines
        .len()
        .saturating_sub(stderr_take)
        .min(MAX_LINES - stdout_take - stderr_take);
    let stderr_take = stderr_take + extra_stderr_take;

    stdout_lines[stdout_lines.len().saturating_sub(stdout_take)..]
        .iter()
        .chain(&stderr_lines[stderr_lines.len().saturating_sub(stderr_take)..])
        .copied()
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(1024)
        .collect()
}

fn compact_tail_lines(text: &str, limit: usize) -> Vec<&str> {
    let mut lines = text
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty()
        })
        .rev()
        .take(limit)
        .map(str::trim)
        .collect::<Vec<_>>();
    lines.reverse();
    lines
}

fn tail(bytes: &[u8], limit: usize) -> &[u8] {
    &bytes[bytes.len().saturating_sub(limit)..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hygiene_schema_is_parsed() {
        let evidence = parse_hygiene_evidence(
            br#"{"schema_version":"code_hygiene.report.v1","files":2,"total_lines":4,"recommendations":[]}"#,
        )
        .unwrap();
        assert_eq!(evidence.files, 2);
        assert_eq!(evidence.total_lines, 4);
    }

    #[test]
    fn hygiene_v2_schema_is_parsed() {
        let evidence = parse_hygiene_evidence(
            br#"{"schema_version":"code_hygiene.report.v2","files":3,"total_lines":7,"recommendations":[]}"#,
        )
        .unwrap();
        assert_eq!(evidence.schema_version, "code_hygiene.report.v2");
        assert_eq!(evidence.files, 3);
    }

    #[test]
    fn failure_trace_redacts_workspace_path() {
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\Users\example\repo\rust")
        } else {
            PathBuf::from("/home/example/repo/rust")
        };
        let spec =
            QualityCommandSpec::new("test", "cargo", ["test"], &root, Duration::from_secs(1));
        let outcome = QualityCommandOutcome {
            exit_code: Some(1),
            stdout: format!("failed in {}", root.display()).into_bytes(),
            stderr: Vec::new(),
            duration: Duration::ZERO,
            timed_out: false,
            output_truncated: false,
            spawn_error: None,
        };
        let trace = sanitized_failure_trace(&spec, &outcome);
        assert!(trace.contains("<workspace>"));
        assert!(!trace.contains(root.to_string_lossy().as_ref()));
    }

    #[test]
    fn failure_trace_preserves_the_full_bounded_capture() {
        let root = PathBuf::from("workspace");
        let spec =
            QualityCommandSpec::new("test", "cargo", ["test"], &root, Duration::from_secs(1));
        let mut stdout = b"first assertion: left Failed right Committed\n".to_vec();
        stdout.extend(vec![b'x'; 16 * 1024]);
        let outcome = QualityCommandOutcome {
            exit_code: Some(101),
            stdout,
            stderr: b"test failed".to_vec(),
            duration: Duration::ZERO,
            timed_out: false,
            output_truncated: false,
            spawn_error: None,
        };

        let trace = sanitized_failure_trace(&spec, &outcome);

        assert!(trace.contains("first assertion: left Failed right Committed"));
        assert!(trace.contains("test failed"));
    }

    #[test]
    fn fail_fast_detects_the_first_failed_stage() {
        let mut request = QualityGateRequest::new("workspace");
        request.fail_fast = true;
        let mut report = QualityGateReport::default();
        report.stages.push(manual_failed_stage(
            "default_workspace",
            "repair the first failure",
        ));

        assert!(should_fail_fast(&request, &report));
        request.fail_fast = false;
        assert!(!should_fail_fast(&request, &report));
    }

    #[test]
    fn workspace_test_target_dir_respects_absolute_and_relative_overrides() {
        let root = Path::new("workspace");
        assert_eq!(
            workspace_test_target_dir(root, None),
            root.join("target/quality-gate/workspace-tests-target")
        );
        assert_eq!(
            workspace_test_target_dir(root, Some(OsStr::new("custom-target"))),
            root.join("custom-target/workspace-tests-target")
        );
        let absolute = std::env::temp_dir().join("aife-quality-gate-target");
        assert_eq!(
            workspace_test_target_dir(root, Some(absolute.as_os_str())),
            absolute.join("workspace-tests-target")
        );
    }

    #[test]
    fn summary_omits_lint_items_and_trace_preserves_them() {
        let item = crate::report::LintItemEvidence {
            fingerprint: "sha256:fingerprint".to_string(),
            lint_code: "clippy::example".to_string(),
            relative_path: "src/lib.rs".to_string(),
            ledger_id: None,
            classification: crate::report::LintClassification::New,
            occurrences: 1,
        };
        assert!(lint_items_for_report(ReportMode::Summary, vec![item.clone()]).is_empty());
        assert_eq!(
            lint_items_for_report(ReportMode::Trace, vec![item.clone()]),
            vec![item]
        );
    }

    #[test]
    fn compact_failure_summary_is_bounded_and_redacted() {
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\Users\example\repo\rust")
        } else {
            PathBuf::from("/home/example/repo/rust")
        };
        let spec =
            QualityCommandSpec::new("test", "cargo", ["test"], &root, Duration::from_secs(1));
        let outcome = QualityCommandOutcome {
            exit_code: Some(1),
            stdout: Vec::new(),
            stderr: format!("failure in {}\nfinal cause", root.display()).into_bytes(),
            duration: Duration::ZERO,
            timed_out: false,
            output_truncated: false,
            spawn_error: None,
        };
        let summary = compact_failure_summary(&spec, &outcome);
        assert!(summary.contains("<workspace>"));
        assert!(summary.contains("final cause"));
        assert!(summary.chars().count() <= 1024);
    }

    #[test]
    fn compact_failure_summary_preserves_stdout_failure_and_stderr_context() {
        let root = Path::new("workspace");
        let spec = QualityCommandSpec::new(
            "default_workspace",
            "cargo",
            ["test"],
            root,
            Duration::from_secs(1),
        );
        let outcome = QualityCommandOutcome {
            exit_code: Some(101),
            stdout: b"failures:\ntest tests::specific_failure ... FAILED\ntest result: FAILED"
                .to_vec(),
            stderr: b"Running unittests src/lib.rs\nerror: test failed".to_vec(),
            duration: Duration::ZERO,
            timed_out: false,
            output_truncated: false,
            spawn_error: None,
        };

        let summary = compact_failure_summary(&spec, &outcome);
        assert!(summary.contains("tests::specific_failure"));
        assert!(summary.contains("error: test failed"));
    }
}
