use crate::cargo_json::{occurrence_counts, ObservedLint};
use crate::report::{LintClassification, LintGateSummary, LintItemEvidence, QualityDiagnostic};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const LINT_DEBT_LEDGER_SCHEMA_VERSION: &str = "lint-debt-ledger.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintDebtLedger {
    pub schema_version: String,
    pub toolchain: String,
    pub generated_from: String,
    pub entries: Vec<LintDebtEntry>,
    pub source_suppressions: Vec<SourceSuppressionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintDebtEntry {
    pub id: String,
    pub fingerprint: String,
    pub lint_code: String,
    pub relative_path: String,
    pub anchor_hash: String,
    pub allowed_occurrences: usize,
    pub origin: String,
    pub reason: String,
    pub owner: String,
    pub review_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSuppressionEntry {
    pub id: String,
    pub lint: String,
    pub relative_path: String,
    pub anchor_hash: String,
    pub allowed_occurrences: usize,
    pub reason: String,
    pub owner: String,
    pub review_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerReconciliation {
    pub summary: LintGateSummary,
    pub items: Vec<LintItemEvidence>,
    pub diagnostics: Vec<QualityDiagnostic>,
}

impl LedgerReconciliation {
    pub fn passed(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn read_ledger(path: &Path) -> Result<LintDebtLedger, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let ledger: LintDebtLedger =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    validate_ledger_shape(&ledger)?;
    Ok(ledger)
}

pub fn validate_ledger_shape(ledger: &LintDebtLedger) -> Result<(), String> {
    if ledger.schema_version != LINT_DEBT_LEDGER_SCHEMA_VERSION {
        return Err(format!(
            "unsupported ledger schema {}",
            ledger.schema_version
        ));
    }
    if ledger.toolchain.is_empty() || ledger.generated_from.is_empty() {
        return Err("ledger toolchain/generated_from must not be empty".to_string());
    }
    let mut ids = BTreeSet::new();
    let mut fingerprints = BTreeSet::new();
    let mut suppression_keys = BTreeSet::new();
    for entry in &ledger.entries {
        validate_common(
            &entry.id,
            &entry.relative_path,
            entry.allowed_occurrences,
            &entry.reason,
            &entry.owner,
            &entry.review_by,
        )?;
        validate_hash(&entry.id, "fingerprint", &entry.fingerprint)?;
        validate_hash(&entry.id, "anchor_hash", &entry.anchor_hash)?;
        if entry.lint_code.is_empty() || entry.origin.is_empty() {
            return Err(format!(
                "ledger entry {} has empty lint_code/origin",
                entry.id
            ));
        }
        if !ids.insert(entry.id.clone()) {
            return Err(format!("duplicate ledger id {}", entry.id));
        }
        if !fingerprints.insert(entry.fingerprint.clone()) {
            return Err(format!("duplicate fingerprint {}", entry.fingerprint));
        }
    }
    for suppression in &ledger.source_suppressions {
        validate_common(
            &suppression.id,
            &suppression.relative_path,
            suppression.allowed_occurrences,
            &suppression.reason,
            &suppression.owner,
            &suppression.review_by,
        )?;
        validate_hash(&suppression.id, "anchor_hash", &suppression.anchor_hash)?;
        if suppression.lint.is_empty() {
            return Err(format!("suppression {} has empty lint", suppression.id));
        }
        if !ids.insert(suppression.id.clone()) {
            return Err(format!("duplicate ledger id {}", suppression.id));
        }
        let key = (
            suppression.relative_path.as_str(),
            suppression.lint.as_str(),
            suppression.anchor_hash.as_str(),
        );
        if !suppression_keys.insert(key) {
            return Err(format!(
                "duplicate source suppression identity {}",
                suppression.id
            ));
        }
    }
    Ok(())
}

fn validate_common(
    id: &str,
    path: &str,
    allowed_occurrences: usize,
    reason: &str,
    owner: &str,
    review_by: &str,
) -> Result<(), String> {
    if id.is_empty()
        || path.is_empty()
        || reason.is_empty()
        || owner.is_empty()
        || review_by.is_empty()
    {
        return Err("ledger entry has an empty required field".to_string());
    }
    if allowed_occurrences == 0 {
        return Err(format!("ledger entry {id} allows zero occurrences"));
    }
    if crate::cargo_json::is_absolute_path(path) || path.contains('\\') {
        return Err(format!("ledger entry {id} contains absolute path"));
    }
    parse_date(review_by).ok_or_else(|| format!("ledger entry {id} has invalid review_by"))?;
    Ok(())
}

fn validate_hash(id: &str, field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("ledger entry {id} has invalid {field}"));
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("ledger entry {id} has invalid {field}"));
    }
    Ok(())
}

pub fn reconcile_lints(
    workspace_root: &Path,
    expected_toolchain: &str,
    ledger: &LintDebtLedger,
    observed: &[ObservedLint],
    today: &str,
) -> LedgerReconciliation {
    let actual_counts = occurrence_counts(observed);
    let actual_by_fingerprint: BTreeMap<_, _> = observed
        .iter()
        .map(|item| (item.fingerprint.clone(), item))
        .collect();
    let mut diagnostics = Vec::new();
    let mut items = Vec::new();
    let mut summary = LintGateSummary {
        observed_occurrences: observed.len(),
        ..LintGateSummary::default()
    };

    if ledger.toolchain != expected_toolchain {
        diagnostics.push(diagnostic(
            "quality_gate.toolchain_mismatch",
            format!(
                "ledger toolchain {} does not match expected {expected_toolchain}",
                ledger.toolchain
            ),
            "Regenerate a reviewed ledger with the selected toolchain.",
        ));
    }

    for entry in &ledger.entries {
        let path = workspace_root.join(
            entry
                .relative_path
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        let expired = date_is_before(&entry.review_by, today);
        let stale = !path.is_file() || expired;
        if stale {
            summary.stale_entries += 1;
            diagnostics.push(diagnostic(
                if expired {
                    "quality_gate.lint_entry_expired"
                } else {
                    "quality_gate.lint_entry_stale"
                },
                format!("ledger entry {} is stale or expired", entry.id),
                "Fix the debt, update owner/reason/review_by, or remove the stale entry.",
            ));
        }
        match actual_counts.get(&entry.fingerprint).copied() {
            Some(actual) if actual <= entry.allowed_occurrences => {
                summary.known_entries += 1;
                items.push(lint_item(
                    entry,
                    if stale {
                        LintClassification::Stale
                    } else {
                        LintClassification::Known
                    },
                    actual,
                ));
            }
            Some(actual) => {
                summary.new_entries += 1;
                items.push(lint_item(entry, LintClassification::New, actual));
                diagnostics.push(diagnostic(
                    "quality_gate.lint_occurrence_exceeded",
                    format!(
                        "{} occurs {actual} times; ledger allows {}",
                        entry.id, entry.allowed_occurrences
                    ),
                    "Remove the added occurrence or submit a reviewed ledger change.",
                ));
            }
            None => {
                summary.resolved_entries += 1;
                items.push(lint_item(entry, LintClassification::Resolved, 0));
                diagnostics.push(diagnostic(
                    "quality_gate.lint_resolved_prune_required",
                    format!("ledger entry {} no longer occurs", entry.id),
                    "Remove the resolved entry from the committed ledger.",
                ));
            }
        }
    }

    let known: BTreeSet<_> = ledger
        .entries
        .iter()
        .map(|entry| entry.fingerprint.as_str())
        .collect();
    for (fingerprint, actual) in &actual_counts {
        if !known.contains(fingerprint.as_str()) {
            summary.new_entries += 1;
            let item = actual_by_fingerprint[fingerprint];
            diagnostics.push(diagnostic(
                "quality_gate.lint_new",
                format!(
                    "new {} diagnostic in {} ({actual} occurrence(s))",
                    item.lint_code, item.relative_path
                ),
                "Fix the warning or submit a reviewed ledger candidate.",
            ));
            items.push(LintItemEvidence {
                fingerprint: item.fingerprint.clone(),
                lint_code: item.lint_code.clone(),
                relative_path: item.relative_path.clone(),
                ledger_id: None,
                classification: LintClassification::New,
                occurrences: *actual,
            });
        }
    }

    LedgerReconciliation {
        summary,
        items,
        diagnostics,
    }
}

fn lint_item(
    entry: &LintDebtEntry,
    classification: LintClassification,
    occurrences: usize,
) -> LintItemEvidence {
    LintItemEvidence {
        fingerprint: entry.fingerprint.clone(),
        lint_code: entry.lint_code.clone(),
        relative_path: entry.relative_path.clone(),
        ledger_id: Some(entry.id.clone()),
        classification,
        occurrences,
    }
}

pub fn utc_today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

pub(crate) fn parse_date(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=days_in_month).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

pub(crate) fn date_is_before(value: &str, today: &str) -> bool {
    parse_date(value)
        .zip(parse_date(today))
        .is_some_and(|(left, right)| left < right)
}

fn diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    next_action: impl Into<String>,
) -> QualityDiagnostic {
    QualityDiagnostic {
        code: code.into(),
        message: message.into(),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo_json::ObservedLint;

    fn entry(fingerprint: &str, allowed: usize) -> LintDebtEntry {
        LintDebtEntry {
            id: format!("entry-{fingerprint}"),
            fingerprint: fingerprint.to_string(),
            lint_code: "clippy::example".to_string(),
            relative_path: "Cargo.toml".to_string(),
            anchor_hash: "sha256:anchor".to_string(),
            allowed_occurrences: allowed,
            origin: "test".to_string(),
            reason: "test debt".to_string(),
            owner: "test-owner".to_string(),
            review_by: "2099-01-01".to_string(),
        }
    }

    fn observed(fingerprint: &str) -> ObservedLint {
        ObservedLint {
            fingerprint: fingerprint.to_string(),
            lint_code: "clippy::example".to_string(),
            relative_path: "Cargo.toml".to_string(),
            anchor_hash: "sha256:anchor".to_string(),
            message: "message".to_string(),
        }
    }

    fn ledger(entries: Vec<LintDebtEntry>) -> LintDebtLedger {
        LintDebtLedger {
            schema_version: LINT_DEBT_LEDGER_SCHEMA_VERSION.to_string(),
            toolchain: "1.96.0".to_string(),
            generated_from: "test".to_string(),
            entries,
            source_suppressions: Vec::new(),
        }
    }

    #[test]
    fn known_warning_passes() {
        let result = reconcile_lints(
            Path::new("."),
            "1.96.0",
            &ledger(vec![entry("fp", 1)]),
            &[observed("fp")],
            "2026-07-12",
        );
        assert!(result.passed());
    }

    #[test]
    fn same_total_replacement_fails_as_resolved_and_new() {
        let result = reconcile_lints(
            Path::new("."),
            "1.96.0",
            &ledger(vec![entry("old", 1)]),
            &[observed("new")],
            "2026-07-12",
        );
        assert_eq!(result.summary.resolved_entries, 1);
        assert_eq!(result.summary.new_entries, 1);
    }

    #[test]
    fn occurrence_growth_fails() {
        let result = reconcile_lints(
            Path::new("."),
            "1.96.0",
            &ledger(vec![entry("fp", 1)]),
            &[observed("fp"), observed("fp")],
            "2026-07-12",
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "quality_gate.lint_occurrence_exceeded"));
    }

    #[test]
    fn expired_entry_fails() {
        let mut expired = entry("fp", 1);
        expired.review_by = "2020-01-01".to_string();
        let result = reconcile_lints(
            Path::new("."),
            "1.96.0",
            &ledger(vec![expired]),
            &[observed("fp")],
            "2026-07-12",
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "quality_gate.lint_entry_expired"));
    }
}
