use crate::cargo_json::{occurrence_counts, parse_cargo_json, ObservedLint};
use crate::command::{QualityCommandExecutor, QualityCommandSpec};
use crate::lint_ledger::{
    date_is_before, read_ledger, utc_today, LintDebtEntry, LintDebtLedger, SourceSuppressionEntry,
    LINT_DEBT_LEDGER_SCHEMA_VERSION,
};
use crate::suppression::{scan_workspace_suppressions, ObservedSuppression};
use crate::toolchain::EXPECTED_RUST_RELEASE;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const REVIEW_BY: &str = "2026-10-01";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerProposal {
    pub ledger_path: PathBuf,
    pub diff_report_path: PathBuf,
    pub entry_count: usize,
    pub warning_occurrences: usize,
    pub suppression_count: usize,
}

#[derive(Debug, Serialize)]
struct ProposalDiff {
    schema_version: &'static str,
    previous_ledger_readable: bool,
    previous_entries: usize,
    candidate_entries: usize,
    candidate_warning_occurrences: usize,
    previous_suppressions: usize,
    candidate_suppressions: usize,
    added_diagnostics: Vec<DiagnosticReference>,
    removed_diagnostics: Vec<DiagnosticReference>,
    changed_diagnostics: Vec<ChangedDiagnostic>,
    new_suppressions: Vec<SuppressionReference>,
    expired_entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticReference {
    fingerprint: String,
    lint_code: String,
    relative_path: String,
}

#[derive(Debug, Serialize)]
struct ChangedDiagnostic {
    lint_code: String,
    relative_path: String,
    removed_fingerprints: Vec<String>,
    added_fingerprints: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SuppressionReference {
    lint: String,
    relative_path: String,
    anchor_hash: String,
}

pub fn propose_ledger<E: QualityCommandExecutor>(
    executor: &E,
    workspace_root: &Path,
) -> Result<LedgerProposal, String> {
    verify_rustc(executor, workspace_root)?;
    let spec = QualityCommandSpec::new(
        "propose_clippy",
        "cargo",
        [
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--message-format=json",
        ],
        workspace_root,
        Duration::from_secs(45 * 60),
    );
    let outcome = executor.execute(&spec);
    if !outcome.passed() {
        return Err(format!(
            "Clippy baseline failed: exit={:?}, timed_out={}, spawn_error={:?}",
            outcome.exit_code, outcome.timed_out, outcome.spawn_error
        ));
    }
    let observed = parse_cargo_json(&outcome.stdout)?;
    let suppressions = scan_workspace_suppressions(workspace_root)?;
    let ledger = build_ledger(&observed, &suppressions);

    let candidate_directory = workspace_root.join("target/quality-gate/candidate");
    fs::create_dir_all(&candidate_directory).map_err(|error| error.to_string())?;
    let ledger_path = candidate_directory.join("lint-debt-ledger.v1.json");
    write_json(&ledger_path, &ledger)?;

    let previous = read_ledger(&workspace_root.join("quality/lint-debt-ledger.v1.json"));
    let diff = build_diff(previous.as_ref().ok(), &ledger);
    let diff_report_path = candidate_directory.join("lint-debt-ledger-diff.v1.json");
    write_json(&diff_report_path, &diff)?;

    Ok(LedgerProposal {
        ledger_path,
        diff_report_path,
        entry_count: ledger.entries.len(),
        warning_occurrences: ledger
            .entries
            .iter()
            .map(|entry| entry.allowed_occurrences)
            .sum(),
        suppression_count: ledger.source_suppressions.len(),
    })
}

fn build_diff(previous: Option<&LintDebtLedger>, candidate: &LintDebtLedger) -> ProposalDiff {
    let previous_entries = previous.map_or(&[][..], |ledger| ledger.entries.as_slice());
    let previous_by_fingerprint: BTreeMap<_, _> = previous_entries
        .iter()
        .map(|entry| (entry.fingerprint.as_str(), entry))
        .collect();
    let candidate_by_fingerprint: BTreeMap<_, _> = candidate
        .entries
        .iter()
        .map(|entry| (entry.fingerprint.as_str(), entry))
        .collect();
    let added_diagnostics = candidate_by_fingerprint
        .iter()
        .filter(|(fingerprint, _)| !previous_by_fingerprint.contains_key(**fingerprint))
        .map(|(_, entry)| diagnostic_reference(entry))
        .collect::<Vec<_>>();
    let removed_diagnostics = previous_by_fingerprint
        .iter()
        .filter(|(fingerprint, _)| !candidate_by_fingerprint.contains_key(**fingerprint))
        .map(|(_, entry)| diagnostic_reference(entry))
        .collect::<Vec<_>>();

    let mut changes = BTreeMap::<(String, String), (BTreeSet<String>, BTreeSet<String>)>::new();
    for entry in &removed_diagnostics {
        changes
            .entry((entry.lint_code.clone(), entry.relative_path.clone()))
            .or_default()
            .0
            .insert(entry.fingerprint.clone());
    }
    for entry in &added_diagnostics {
        changes
            .entry((entry.lint_code.clone(), entry.relative_path.clone()))
            .or_default()
            .1
            .insert(entry.fingerprint.clone());
    }
    let changed_diagnostics = changes
        .into_iter()
        .filter(|(_, (removed, added))| !removed.is_empty() && !added.is_empty())
        .map(
            |((lint_code, relative_path), (removed_fingerprints, added_fingerprints))| {
                ChangedDiagnostic {
                    lint_code,
                    relative_path,
                    removed_fingerprints: removed_fingerprints.into_iter().collect(),
                    added_fingerprints: added_fingerprints.into_iter().collect(),
                }
            },
        )
        .collect();

    let previous_suppressions =
        previous.map_or(&[][..], |ledger| ledger.source_suppressions.as_slice());
    let previous_suppression_keys: BTreeSet<_> =
        previous_suppressions.iter().map(suppression_key).collect();
    let new_suppressions = candidate
        .source_suppressions
        .iter()
        .filter(|entry| !previous_suppression_keys.contains(&suppression_key(entry)))
        .map(|entry| SuppressionReference {
            lint: entry.lint.clone(),
            relative_path: entry.relative_path.clone(),
            anchor_hash: entry.anchor_hash.clone(),
        })
        .collect();

    let today = utc_today();
    let mut expired_entries = previous
        .into_iter()
        .flat_map(|ledger| {
            ledger
                .entries
                .iter()
                .map(|entry| (&entry.id, &entry.review_by))
                .chain(
                    ledger
                        .source_suppressions
                        .iter()
                        .map(|entry| (&entry.id, &entry.review_by)),
                )
        })
        .filter(|(_, review_by)| date_is_before(review_by, &today))
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    expired_entries.sort();

    ProposalDiff {
        schema_version: "lint-debt-ledger-diff.v1",
        previous_ledger_readable: previous.is_some(),
        previous_entries: previous_entries.len(),
        candidate_entries: candidate.entries.len(),
        candidate_warning_occurrences: candidate
            .entries
            .iter()
            .map(|entry| entry.allowed_occurrences)
            .sum(),
        previous_suppressions: previous_suppressions.len(),
        candidate_suppressions: candidate.source_suppressions.len(),
        added_diagnostics,
        removed_diagnostics,
        changed_diagnostics,
        new_suppressions,
        expired_entries,
    }
}

fn diagnostic_reference(entry: &LintDebtEntry) -> DiagnosticReference {
    DiagnosticReference {
        fingerprint: entry.fingerprint.clone(),
        lint_code: entry.lint_code.clone(),
        relative_path: entry.relative_path.clone(),
    }
}

fn suppression_key(entry: &SourceSuppressionEntry) -> (&str, &str, &str) {
    (&entry.relative_path, &entry.lint, &entry.anchor_hash)
}

fn verify_rustc<E: QualityCommandExecutor>(
    executor: &E,
    workspace_root: &Path,
) -> Result<(), String> {
    let spec = QualityCommandSpec::new(
        "propose_toolchain",
        "rustc",
        ["-Vv"],
        workspace_root,
        Duration::from_secs(5 * 60),
    );
    let outcome = executor.execute(&spec);
    if !outcome.passed() {
        return Err("failed to resolve the committed Rust toolchain".to_string());
    }
    let output = String::from_utf8(outcome.stdout).map_err(|error| error.to_string())?;
    let release = output
        .lines()
        .find_map(|line| line.strip_prefix("release:").map(str::trim));
    if release != Some(EXPECTED_RUST_RELEASE) {
        return Err(format!(
            "expected Rust {EXPECTED_RUST_RELEASE}, observed {release:?}"
        ));
    }
    Ok(())
}

fn build_ledger(observed: &[ObservedLint], suppressions: &[ObservedSuppression]) -> LintDebtLedger {
    let counts = occurrence_counts(observed);
    let by_fingerprint: BTreeMap<_, _> = observed
        .iter()
        .map(|item| (item.fingerprint.clone(), item))
        .collect();
    let entries = counts
        .iter()
        .enumerate()
        .map(|(index, (fingerprint, count))| {
            let item = by_fingerprint[fingerprint];
            LintDebtEntry {
                id: format!("lint-debt-{:04}", index + 1),
                fingerprint: fingerprint.clone(),
                lint_code: item.lint_code.clone(),
                relative_path: item.relative_path.clone(),
                anchor_hash: item.anchor_hash.clone(),
                allowed_occurrences: *count,
                origin: "post-244-baseline".to_string(),
                reason: "Legacy lint debt captured after CQ-06; cleanup is deferred to CQ-07."
                    .to_string(),
                owner: "engine-maintainers".to_string(),
                review_by: REVIEW_BY.to_string(),
            }
        })
        .collect();

    let mut suppression_counts = BTreeMap::<(String, String, String), usize>::new();
    for item in suppressions {
        *suppression_counts
            .entry((
                item.relative_path.clone(),
                item.lint.clone(),
                item.anchor_hash.clone(),
            ))
            .or_default() += 1;
    }
    let source_suppressions = suppression_counts
        .into_iter()
        .enumerate()
        .map(
            |(index, ((relative_path, lint, anchor_hash), allowed_occurrences))| {
                SourceSuppressionEntry {
                    id: format!("source-suppression-{:04}", index + 1),
                    lint,
                    relative_path,
                    anchor_hash,
                    allowed_occurrences,
                    reason: "Existing source-level suppression captured in the post-244 baseline."
                        .to_string(),
                    owner: "engine-maintainers".to_string(),
                    review_by: REVIEW_BY.to_string(),
                }
            },
        )
        .collect();

    LintDebtLedger {
        schema_version: LINT_DEBT_LEDGER_SCHEMA_VERSION.to_string(),
        toolchain: EXPECTED_RUST_RELEASE.to_string(),
        generated_from: "post-244-baseline".to_string(),
        entries,
        source_suppressions,
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    json.push('\n');
    fs::write(path, json).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{QualityCommandOutcome, ScriptedQualityCommandExecutor};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn candidate_groups_warning_and_suppression_occurrences() {
        let warning = ObservedLint {
            fingerprint: "sha256:fingerprint".to_string(),
            lint_code: "clippy::example".to_string(),
            relative_path: "src/lib.rs".to_string(),
            anchor_hash: "sha256:anchor".to_string(),
            message: "message".to_string(),
        };
        let suppression = ObservedSuppression {
            lint: "dead_code".to_string(),
            relative_path: "src/lib.rs".to_string(),
            anchor_hash: "sha256:suppression".to_string(),
        };
        let ledger = build_ledger(
            &[warning.clone(), warning],
            &[suppression.clone(), suppression],
        );
        assert_eq!(ledger.entries[0].allowed_occurrences, 2);
        assert_eq!(ledger.source_suppressions[0].allowed_occurrences, 2);
    }

    #[test]
    fn diff_lists_added_removed_changed_suppression_and_expired_items() {
        fn debt(fingerprint: &str, review_by: &str) -> LintDebtEntry {
            LintDebtEntry {
                id: format!("debt-{fingerprint}"),
                fingerprint: fingerprint.to_string(),
                lint_code: "clippy::example".to_string(),
                relative_path: "src/lib.rs".to_string(),
                anchor_hash: "sha256:anchor".to_string(),
                allowed_occurrences: 1,
                origin: "test".to_string(),
                reason: "test".to_string(),
                owner: "test".to_string(),
                review_by: review_by.to_string(),
            }
        }
        let previous = LintDebtLedger {
            schema_version: LINT_DEBT_LEDGER_SCHEMA_VERSION.to_string(),
            toolchain: EXPECTED_RUST_RELEASE.to_string(),
            generated_from: "test".to_string(),
            entries: vec![debt("old", "2020-01-01")],
            source_suppressions: Vec::new(),
        };
        let candidate = LintDebtLedger {
            schema_version: LINT_DEBT_LEDGER_SCHEMA_VERSION.to_string(),
            toolchain: EXPECTED_RUST_RELEASE.to_string(),
            generated_from: "test".to_string(),
            entries: vec![debt("new", "2099-01-01")],
            source_suppressions: vec![SourceSuppressionEntry {
                id: "suppression-new".to_string(),
                lint: "dead_code".to_string(),
                relative_path: "src/lib.rs".to_string(),
                anchor_hash: "sha256:suppression".to_string(),
                allowed_occurrences: 1,
                reason: "test".to_string(),
                owner: "test".to_string(),
                review_by: "2099-01-01".to_string(),
            }],
        };

        let diff = build_diff(Some(&previous), &candidate);
        assert_eq!(diff.added_diagnostics.len(), 1);
        assert_eq!(diff.removed_diagnostics.len(), 1);
        assert_eq!(diff.changed_diagnostics.len(), 1);
        assert_eq!(diff.new_suppressions.len(), 1);
        assert_eq!(diff.expired_entries, vec!["debt-old"]);
    }

    #[test]
    fn proposal_writes_only_candidate_directory() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "quality-gate-proposal-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("quality")).unwrap();
        let committed = root.join("quality/lint-debt-ledger.v1.json");
        fs::write(&committed, b"committed-sentinel").unwrap();
        let executor = ScriptedQualityCommandExecutor::default();
        executor.push(
            "propose_toolchain",
            QualityCommandOutcome::success(
                b"host: x86_64-pc-windows-msvc\nrelease: 1.96.0\n".to_vec(),
            ),
        );
        let warning = serde_json::json!({
            "reason": "compiler-message",
            "message": {
                "level": "warning",
                "message": "example",
                "code": {"code": "clippy::example"},
                "spans": [{
                    "is_primary": true,
                    "file_name": "src/lib.rs",
                    "text": [{"text": "fn example() {}"}]
                }]
            }
        })
        .to_string();
        executor.push(
            "propose_clippy",
            QualityCommandOutcome::success(warning.into_bytes()),
        );

        let proposal = propose_ledger(&executor, &root).unwrap();
        assert!(proposal.ledger_path.starts_with(root.join("target")));
        assert_eq!(fs::read(&committed).unwrap(), b"committed-sentinel");
        fs::remove_dir_all(root).unwrap();
    }
}
