use crate::command::{QualityCommandExecutor, QualityCommandOutcome, QualityCommandSpec};
use crate::report::{
    read_quality_gate_report, LocalCiRunReport, QualityDiagnostic, QualityGateReport,
    LOCAL_CI_RUN_REPORT_SCHEMA_VERSION,
};
use crate::workspace::file_digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GIT_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const LOCAL_CI_TIMEOUT: Duration = Duration::from_secs(90 * 60);
static LOCAL_RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCiRequest {
    pub workspace_root: PathBuf,
    pub revision: String,
}

impl LocalCiRequest {
    pub fn new(workspace_root: impl Into<PathBuf>, revision: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            revision: revision.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalCiRunner<E> {
    executor: E,
}

impl<E> LocalCiRunner<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }
}

impl<E: QualityCommandExecutor> LocalCiRunner<E> {
    pub fn run(&self, request: LocalCiRequest) -> LocalCiRunReport {
        let started = Instant::now();
        let started_at_epoch_seconds = epoch_seconds();
        let mut report = LocalCiRunReport {
            schema_version: LOCAL_CI_RUN_REPORT_SCHEMA_VERSION.to_string(),
            started_at_epoch_seconds,
            run_id: format!("local-pending-{started_at_epoch_seconds}"),
            requested_revision: request.revision.clone(),
            source_workspace_state: "unknown".to_string(),
            isolated_workspace_state: "not_created".to_string(),
            cleanup_status: "not_required".to_string(),
            ..LocalCiRunReport::default()
        };

        if !valid_revision(&request.revision) {
            push_diagnostic(
                &mut report,
                "quality_gate.local_ci_revision_invalid",
                "local CI revision is empty, too long, or contains unsupported characters",
                "Use a local Git revision such as HEAD or a commit SHA.",
            );
            return finish_report(report, started, None);
        }

        let workspace_root = match fs::canonicalize(&request.workspace_root) {
            Ok(path) => path,
            Err(error) => {
                push_diagnostic(
                    &mut report,
                    "quality_gate.local_ci_workspace_invalid",
                    format!("workspace root cannot be resolved: {error}"),
                    "Run local-ci from the committed Rust workspace.",
                );
                return finish_report(report, started, None);
            }
        };
        let repository_root = match self.repository_root(&workspace_root, &mut report) {
            Some(path) => path,
            None => return finish_report(report, started, None),
        };
        let workspace_relative = match workspace_root.strip_prefix(&repository_root) {
            Ok(path) if !path.as_os_str().is_empty() => path.to_path_buf(),
            _ => {
                push_diagnostic(
                    &mut report,
                    "quality_gate.local_ci_workspace_invalid",
                    "Rust workspace must be a child of the local Git repository root",
                    "Pass --workspace-root for the repository's committed Rust workspace.",
                );
                return finish_report(report, started, None);
            }
        };

        let source_status = self.execute(command(
            "local_ci_source_status",
            "git",
            ["status", "--porcelain"],
            &repository_root,
            GIT_TIMEOUT,
        ));
        if !source_status.passed() {
            push_command_diagnostic(
                &mut report,
                "quality_gate.local_ci_source_status_failed",
                "local_ci_source_status",
                &source_status,
                "Repair the local Git worktree and retry.",
            );
            return finish_report(report, started, None);
        }
        if source_status.stdout.is_empty() {
            report.source_workspace_state = "clean".to_string();
        } else {
            report.source_workspace_state = "dirty".to_string();
            push_diagnostic(
                &mut report,
                "quality_gate.local_ci_source_dirty",
                "local CI requires a clean source worktree",
                "Commit the complete local integration snapshot, then rerun local-ci --commit HEAD.",
            );
            return finish_report(report, started, None);
        }

        let commit_sha = match self.resolve_commit(
            &repository_root,
            &request.revision,
            "local_ci_resolve_commit",
            &mut report,
        ) {
            Some(commit) => commit,
            None => return finish_report(report, started, None),
        };
        let head_sha = match self.resolve_commit(
            &repository_root,
            "HEAD",
            "local_ci_resolve_head",
            &mut report,
        ) {
            Some(commit) => commit,
            None => return finish_report(report, started, None),
        };
        report.commit_sha = Some(commit_sha.clone());
        report.run_id = local_run_id(&commit_sha, started_at_epoch_seconds);
        if commit_sha != head_sha {
            push_diagnostic(
                &mut report,
                "quality_gate.local_ci_commit_mismatch",
                "requested local CI commit does not match the checked-out HEAD",
                "Check out the intended local integration commit and use --commit HEAD.",
            );
            return finish_report(report, started, None);
        }

        let artifact_dir = workspace_root
            .join("target/quality-gate/local-ci")
            .join(&report.run_id);
        if let Err(error) = fs::create_dir_all(&artifact_dir) {
            push_diagnostic(
                &mut report,
                "quality_gate.local_ci_artifact_failed",
                format!("local CI artifact directory cannot be created: {error}"),
                "Make the workspace target directory writable and retry.",
            );
            return finish_report(report, started, None);
        }

        let worktree_path = local_worktree_path(&report.run_id);
        let worktree_text = worktree_path.to_string_lossy().to_string();
        let add = self.execute(command(
            "git_worktree_add",
            "git",
            ["worktree", "add", "--detach", &worktree_text, &commit_sha],
            &repository_root,
            GIT_TIMEOUT,
        ));
        if !add.passed() {
            push_command_diagnostic(
                &mut report,
                "quality_gate.local_ci_worktree_failed",
                "git_worktree_add",
                &add,
                "Repair local Git worktree metadata and retry.",
            );
            return finish_report(report, started, Some(&artifact_dir));
        }
        report.isolated_workspace_state = "created".to_string();
        report.cleanup_status = "pending".to_string();

        let isolated_workspace = worktree_path.join(&workspace_relative);
        let build_target = local_build_target_path(&workspace_root, &report.run_id);
        let quality_report_source =
            isolated_workspace.join("target/quality-gate/quality-gate-report.v2.json");
        let quality_report_target = artifact_dir.join("quality-gate-report.v2.json");
        let verify = self.execute(local_verify_command(
            &isolated_workspace,
            &build_target,
            &report.run_id,
            &commit_sha,
        ));

        let parsed_report = read_quality_report(&quality_report_source, &mut report);
        if let Some(quality_report) = parsed_report.as_ref() {
            report.isolated_workspace_state = quality_report.workspace_state.clone();
            validate_quality_report(
                &commit_sha,
                &report.run_id.clone(),
                quality_report,
                &mut report,
            );
            match fs::copy(&quality_report_source, &quality_report_target) {
                Ok(_) => {
                    report.quality_gate_report_path = Some(format!(
                        "target/quality-gate/local-ci/{}/quality-gate-report.v2.json",
                        report.run_id
                    ));
                    match file_digest(&quality_report_target) {
                        Ok(digest) => report.quality_gate_report_digest = Some(digest),
                        Err(error) => push_diagnostic(
                            &mut report,
                            "quality_gate.local_ci_artifact_failed",
                            error,
                            "Regenerate the local CI artifact.",
                        ),
                    }
                }
                Err(error) => push_diagnostic(
                    &mut report,
                    "quality_gate.local_ci_artifact_failed",
                    format!("quality report cannot be copied: {error}"),
                    "Make the workspace target directory writable and retry.",
                ),
            }
        }
        if !verify.passed() {
            push_command_diagnostic(
                &mut report,
                "quality_gate.local_ci_gate_failed",
                "local_ci_verify",
                &verify,
                "Repair the failing quality gate stage on the local integration commit.",
            );
        }

        let remove = self.execute(command(
            "git_worktree_remove",
            "git",
            ["worktree", "remove", "--force", &worktree_text],
            &repository_root,
            GIT_TIMEOUT,
        ));
        let build_cleanup = remove_local_build_target(&build_target);
        record_cleanup_outcome(&mut report, &remove, &build_cleanup);

        finish_report(report, started, Some(&artifact_dir))
    }

    fn repository_root(
        &self,
        workspace_root: &Path,
        report: &mut LocalCiRunReport,
    ) -> Option<PathBuf> {
        let outcome = self.execute(command(
            "local_ci_repository_root",
            "git",
            ["rev-parse", "--show-toplevel"],
            workspace_root,
            GIT_TIMEOUT,
        ));
        if !outcome.passed() {
            push_command_diagnostic(
                report,
                "quality_gate.local_ci_not_repository",
                "local_ci_repository_root",
                &outcome,
                "Run local-ci from a local Git repository.",
            );
            return None;
        }
        let text = String::from_utf8_lossy(&outcome.stdout).trim().to_string();
        match fs::canonicalize(text) {
            Ok(path) => Some(path),
            Err(error) => {
                push_diagnostic(
                    report,
                    "quality_gate.local_ci_not_repository",
                    format!("repository root cannot be resolved: {error}"),
                    "Repair the local Git repository and retry.",
                );
                None
            }
        }
    }

    fn resolve_commit(
        &self,
        repository_root: &Path,
        revision: &str,
        id: &str,
        report: &mut LocalCiRunReport,
    ) -> Option<String> {
        let commit_expression = format!("{revision}^{{commit}}");
        let outcome = self.execute(command(
            id,
            "git",
            ["rev-parse", "--verify", &commit_expression],
            repository_root,
            GIT_TIMEOUT,
        ));
        if !outcome.passed() {
            push_command_diagnostic(
                report,
                "quality_gate.local_ci_commit_invalid",
                id,
                &outcome,
                "Use a valid local commit revision.",
            );
            return None;
        }
        let commit = String::from_utf8_lossy(&outcome.stdout).trim().to_string();
        if commit.len() < 40
            || !commit
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            push_diagnostic(
                report,
                "quality_gate.local_ci_commit_invalid",
                "git returned an invalid commit identifier",
                "Repair the local Git repository and retry.",
            );
            None
        } else {
            Some(commit)
        }
    }

    fn execute(&self, spec: QualityCommandSpec) -> QualityCommandOutcome {
        self.executor.execute(&spec)
    }
}

fn command<I, S>(
    id: impl Into<String>,
    program: impl Into<String>,
    args: I,
    working_directory: &Path,
    timeout: Duration,
) -> QualityCommandSpec
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    QualityCommandSpec::new(id, program, args, working_directory, timeout)
}

fn local_verify_command(
    isolated_workspace: &Path,
    build_target: &Path,
    run_id: &str,
    commit_sha: &str,
) -> QualityCommandSpec {
    let mut spec = command(
        "local_ci_verify",
        "cargo",
        [
            "run",
            "-p",
            "quality_gate",
            "--locked",
            "--",
            "verify",
            "--report-mode",
            "trace",
            "--fail-fast",
        ],
        isolated_workspace,
        LOCAL_CI_TIMEOUT,
    );
    spec.environment = vec![
        (
            "CARGO_TARGET_DIR".to_string(),
            build_target.to_string_lossy().to_string(),
        ),
        (
            "QUALITY_GATE_CI_ADAPTER".to_string(),
            "local-git-worktree".to_string(),
        ),
        (
            "QUALITY_GATE_CI_EXECUTION_SCOPE".to_string(),
            "local_commit".to_string(),
        ),
        ("QUALITY_GATE_CI_RUN_ID".to_string(), run_id.to_string()),
        (
            "QUALITY_GATE_CI_COMMIT_SHA".to_string(),
            commit_sha.to_string(),
        ),
    ];
    spec
}

fn read_quality_report(path: &Path, report: &mut LocalCiRunReport) -> Option<QualityGateReport> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            push_diagnostic(
                report,
                "quality_gate.local_ci_report_missing",
                format!("isolated quality report is missing: {error}"),
                "Inspect the local_ci_verify command failure and retry.",
            );
            return None;
        }
    };
    match read_quality_gate_report(&bytes) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            push_diagnostic(
                report,
                "quality_gate.local_ci_report_invalid",
                format!("isolated quality report is invalid: {error}"),
                "Repair quality-gate-report.v2 serialization and retry.",
            );
            None
        }
    }
}

fn validate_quality_report(
    expected_commit: &str,
    expected_run_id: &str,
    quality: &QualityGateReport,
    report: &mut LocalCiRunReport,
) {
    let valid = quality.passed
        && quality.report_mode == crate::ReportMode::Trace
        && quality.fail_fast
        && quality.workspace_state == "clean"
        && quality.workspace_commit_sha.as_deref() == Some(expected_commit)
        && quality.ci.adapter == "local-git-worktree"
        && quality.ci.adapter_configured
        && quality.ci.execution_scope == "local_commit"
        && quality.ci.run_id.as_deref() == Some(expected_run_id)
        && quality.ci.commit_sha.as_deref() == Some(expected_commit)
        && quality.ci.execution_status == "observed_commit_passed";
    if !valid {
        push_diagnostic(
            report,
            "quality_gate.local_ci_report_invalid",
            "isolated quality report is not clean, passed, and bound to the requested commit",
            "Repair the local CI environment or quality gate failure and retry.",
        );
    }
}

fn record_cleanup_outcome(
    report: &mut LocalCiRunReport,
    worktree_outcome: &QualityCommandOutcome,
    build_cleanup: &Result<(), String>,
) {
    let worktree_removed = worktree_outcome.passed();
    if !worktree_removed {
        push_command_diagnostic(
            report,
            "quality_gate.local_ci_cleanup_failed",
            "git_worktree_remove",
            worktree_outcome,
            "Remove the registered temporary worktree, then retry local CI.",
        );
    }
    if let Err(error) = build_cleanup {
        push_diagnostic(
            report,
            "quality_gate.local_ci_build_cleanup_failed",
            error,
            "Remove the run-owned Local CI build target, then retry local CI.",
        );
    }
    if worktree_removed && build_cleanup.is_ok() {
        report.cleanup_status = "removed".to_string();
    } else {
        report.cleanup_status = "failed".to_string();
    }
}

fn remove_local_build_target(build_target: &Path) -> Result<(), String> {
    match fs::remove_dir_all(build_target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "run-owned Local CI build target cannot be removed at {}: {error}",
            build_target.display()
        )),
    }
}

fn local_build_target_path(workspace_root: &Path, run_id: &str) -> PathBuf {
    workspace_root
        .join("target/quality-gate/local-ci-build")
        .join(run_id)
}

fn local_run_id(commit_sha: &str, started_at_epoch_seconds: u64) -> String {
    let sequence = LOCAL_RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "local-{}-{started_at_epoch_seconds}-{}-{sequence}",
        &commit_sha[..12.min(commit_sha.len())],
        std::process::id()
    )
}

fn local_worktree_path(run_id: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ai-first-quality-gate-{run_id}-{}",
        std::process::id()
    ))
}

fn valid_revision(revision: &str) -> bool {
    !revision.is_empty()
        && revision.len() <= 256
        && revision
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-._/~^".contains(character))
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn finish_report(
    mut report: LocalCiRunReport,
    started: Instant,
    artifact_dir: Option<&Path>,
) -> LocalCiRunReport {
    report.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    report.passed = report.diagnostics.is_empty()
        && report.source_workspace_state == "clean"
        && report.isolated_workspace_state == "clean"
        && report.cleanup_status == "removed"
        && report.quality_gate_report_digest.is_some();
    if let Some(artifact_dir) = artifact_dir {
        let path = artifact_dir.join("local-ci-run-report.v1.json");
        if let Err(error) = write_json(&path, &report) {
            push_diagnostic(
                &mut report,
                "quality_gate.local_ci_artifact_failed",
                error,
                "Make the workspace target directory writable and retry.",
            );
            report.passed = false;
            let _ = write_json(&path, &report);
        }
    }
    report
}

fn write_json(path: &Path, report: &LocalCiRunReport) -> Result<(), String> {
    let mut json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
    json.push('\n');
    fs::write(path, json).map_err(|error| error.to_string())
}

fn push_command_diagnostic(
    report: &mut LocalCiRunReport,
    code: &str,
    command_id: &str,
    outcome: &QualityCommandOutcome,
    next_action: &str,
) {
    let message = if outcome.timed_out {
        format!("{command_id} timed out")
    } else if let Some(error) = &outcome.spawn_error {
        format!("{command_id} could not start: {error}")
    } else {
        format!("{command_id} failed with exit code {:?}", outcome.exit_code)
    };
    push_diagnostic(report, code, message, next_action);
}

fn push_diagnostic(
    report: &mut LocalCiRunReport,
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) {
    report.diagnostics.push(QualityDiagnostic {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{QualityCommandOutcome, ScriptedQualityCommandExecutor};
    use crate::report::CiEvidence;

    fn workspace_and_repository() -> (PathBuf, PathBuf) {
        let workspace = fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
        let repository = workspace.parent().unwrap().to_path_buf();
        (workspace, repository)
    }

    #[test]
    fn dirty_source_fails_before_revision_or_worktree_commands() {
        let (workspace, repository) = workspace_and_repository();
        let executor = ScriptedQualityCommandExecutor::default();
        executor.push(
            "local_ci_repository_root",
            QualityCommandOutcome::success(repository.to_string_lossy().as_bytes()),
        );
        executor.push(
            "local_ci_source_status",
            QualityCommandOutcome::success(" M changed.rs"),
        );

        let report =
            LocalCiRunner::new(executor.clone()).run(LocalCiRequest::new(workspace, "HEAD"));

        assert!(!report.passed);
        assert_eq!(report.source_workspace_state, "dirty");
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "quality_gate.local_ci_source_dirty"));
        assert_eq!(
            executor
                .observed()
                .iter()
                .map(|spec| spec.id.as_str())
                .collect::<Vec<_>>(),
            vec!["local_ci_repository_root", "local_ci_source_status"]
        );
    }

    #[test]
    fn requested_revision_must_match_checked_out_head() {
        let (workspace, repository) = workspace_and_repository();
        let executor = ScriptedQualityCommandExecutor::default();
        executor.push(
            "local_ci_repository_root",
            QualityCommandOutcome::success(repository.to_string_lossy().as_bytes()),
        );
        executor.push(
            "local_ci_source_status",
            QualityCommandOutcome::success(Vec::new()),
        );
        executor.push(
            "local_ci_resolve_commit",
            QualityCommandOutcome::success("1111111111111111111111111111111111111111\n"),
        );
        executor.push(
            "local_ci_resolve_head",
            QualityCommandOutcome::success("2222222222222222222222222222222222222222\n"),
        );

        let report = LocalCiRunner::new(executor).run(LocalCiRequest::new(workspace, "HEAD~1"));

        assert!(!report.passed);
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "quality_gate.local_ci_commit_mismatch"));
    }

    #[test]
    fn commit_bound_quality_report_validation_is_fail_closed() {
        let commit = "1111111111111111111111111111111111111111";
        let mut valid = QualityGateReport {
            passed: true,
            report_mode: crate::ReportMode::Trace,
            fail_fast: true,
            workspace_state: "clean".to_string(),
            workspace_commit_sha: Some(commit.to_string()),
            ci: CiEvidence {
                adapter: "local-git-worktree".to_string(),
                adapter_configured: true,
                execution_scope: "local_commit".to_string(),
                run_id: Some("local-run".to_string()),
                commit_sha: Some(commit.to_string()),
                execution_status: "observed_commit_passed".to_string(),
            },
            ..QualityGateReport::default()
        };
        let mut run = LocalCiRunReport::default();
        validate_quality_report(commit, "local-run", &valid, &mut run);
        assert!(run.diagnostics.is_empty());

        valid.report_mode = crate::ReportMode::Summary;
        validate_quality_report(commit, "local-run", &valid, &mut run);
        assert_eq!(run.diagnostics.len(), 1);
        valid.report_mode = crate::ReportMode::Trace;
        run.diagnostics.clear();

        valid.fail_fast = false;
        validate_quality_report(commit, "local-run", &valid, &mut run);
        assert_eq!(run.diagnostics.len(), 1);
        valid.fail_fast = true;
        run.diagnostics.clear();

        valid.workspace_state = "dirty".to_string();
        validate_quality_report(commit, "local-run", &valid, &mut run);
        assert_eq!(run.diagnostics.len(), 1);
        assert_eq!(
            run.diagnostics[0].code,
            "quality_gate.local_ci_report_invalid"
        );
    }

    #[test]
    fn cleanup_failure_is_fail_closed_and_actionable() {
        let mut report = LocalCiRunReport {
            source_workspace_state: "clean".to_string(),
            isolated_workspace_state: "clean".to_string(),
            quality_gate_report_digest: Some("sha256:report".to_string()),
            ..LocalCiRunReport::default()
        };
        let outcome = QualityCommandOutcome {
            exit_code: Some(1),
            stdout: Vec::new(),
            stderr: Vec::new(),
            duration: Duration::ZERO,
            timed_out: false,
            output_truncated: false,
            spawn_error: None,
        };

        record_cleanup_outcome(&mut report, &outcome, &Ok(()));
        let report = finish_report(report, Instant::now(), None);

        assert!(!report.passed);
        assert_eq!(report.cleanup_status, "failed");
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "quality_gate.local_ci_cleanup_failed"
                && !item.next_action.is_empty()));
    }

    #[test]
    fn build_target_cleanup_failure_is_fail_closed_and_actionable() {
        let mut report = LocalCiRunReport {
            source_workspace_state: "clean".to_string(),
            isolated_workspace_state: "clean".to_string(),
            quality_gate_report_digest: Some("sha256:report".to_string()),
            ..LocalCiRunReport::default()
        };
        let worktree_outcome = QualityCommandOutcome::success(Vec::new());

        record_cleanup_outcome(
            &mut report,
            &worktree_outcome,
            &Err("run target is locked".to_string()),
        );
        let report = finish_report(report, Instant::now(), None);

        assert!(!report.passed);
        assert_eq!(report.cleanup_status, "failed");
        assert!(report.diagnostics.iter().any(|item| {
            item.code == "quality_gate.local_ci_build_cleanup_failed"
                && item.message.contains("run target is locked")
                && !item.next_action.is_empty()
        }));
    }

    #[test]
    fn local_verify_command_uses_only_canonical_runner_interface() {
        let build_target = local_build_target_path(Path::new("source"), "local-run");
        let spec = local_verify_command(
            Path::new("isolated/rust"),
            &build_target,
            "local-run",
            "1111111111111111111111111111111111111111",
        );
        assert_eq!(spec.program, "cargo");
        assert_eq!(
            spec.args,
            vec![
                "run",
                "-p",
                "quality_gate",
                "--locked",
                "--",
                "verify",
                "--report-mode",
                "trace",
                "--fail-fast"
            ]
        );
        assert!(spec.environment.iter().any(|(key, value)| {
            key == "QUALITY_GATE_CI_ADAPTER" && value == "local-git-worktree"
        }));
        assert!(spec.environment.iter().any(|(key, value)| {
            key == "CARGO_TARGET_DIR"
                && value
                    .replace('\\', "/")
                    .ends_with("local-ci-build/local-run")
        }));
    }

    #[test]
    fn local_ci_run_ids_are_unique_within_the_same_second() {
        let commit = "1111111111111111111111111111111111111111";

        let first = local_run_id(commit, 1234);
        let second = local_run_id(commit, 1234);

        assert_ne!(first, second);
        assert!(first.starts_with("local-111111111111-1234-"));
        assert!(second.starts_with("local-111111111111-1234-"));
    }

    #[test]
    fn local_ci_build_targets_are_isolated_by_run_id() {
        let workspace = Path::new("source");

        let first = local_build_target_path(workspace, "local-first");
        let second = local_build_target_path(workspace, "local-second");

        assert_ne!(first, second);
        assert!(first
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("target/quality-gate/local-ci-build/local-first"));
        assert!(second
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("target/quality-gate/local-ci-build/local-second"));
    }
}
