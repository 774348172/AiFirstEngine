use crate::architecture_policy::ArchitectureDiagnostic;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const ARCHITECTURE_DEBT_SCHEMA_VERSION: &str = "architecture-debt-ledger.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureDebtLedger {
    pub schema_version: String,
    pub entries: Vec<ArchitectureDebtEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureDebtEntry {
    pub id: String,
    pub subject_path: String,
    pub fingerprint: String,
    pub domain: String,
    pub owner: String,
    pub status: DebtStatus,
    pub reason: String,
    pub review_by: String,
    pub plan: String,
    pub evidence_digest: String,
    pub baseline_risk: u8,
    pub baseline_dependencies: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebtStatus {
    Active,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedDebt {
    pub subject_path: String,
    pub fingerprint: String,
    pub risk: u8,
    pub dependencies: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebtClassification {
    Known,
    New,
    Resolved,
    Improved,
    Regressed,
    Reintroduced,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebtReconciliationItem {
    pub subject_path: String,
    pub ledger_id: Option<String>,
    pub classification: DebtClassification,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebtReconciliation {
    pub items: Vec<DebtReconciliationItem>,
    pub diagnostics: Vec<ArchitectureDiagnostic>,
}

impl DebtReconciliation {
    pub fn passed(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn read_debt_ledger(path: &Path, today: &str) -> Result<ArchitectureDebtLedger, String> {
    let source =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let ledger: ArchitectureDebtLedger =
        serde_json::from_str(&source).map_err(|error| error.to_string())?;
    validate_debt_ledger(&ledger, today)?;
    Ok(ledger)
}

pub fn validate_debt_ledger(ledger: &ArchitectureDebtLedger, _today: &str) -> Result<(), String> {
    if ledger.schema_version != ARCHITECTURE_DEBT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported architecture debt schema {:?}",
            ledger.schema_version
        ));
    }
    let mut ids = BTreeSet::new();
    let mut subjects = BTreeSet::new();
    for entry in &ledger.entries {
        if entry.id.trim().is_empty() || !ids.insert(entry.id.as_str()) {
            return Err(format!("empty or duplicate debt ID {:?}", entry.id));
        }
        if !subjects.insert((entry.subject_path.as_str(), entry.fingerprint.as_str())) {
            return Err(format!(
                "duplicate debt subject/fingerprint {} {}",
                entry.subject_path, entry.fingerprint
            ));
        }
        if entry.owner.trim().is_empty()
            || entry.reason.trim().is_empty()
            || entry.plan.trim().is_empty()
            || !valid_date(&entry.review_by)
            || !valid_digest(&entry.evidence_digest)
            || !valid_digest(&entry.fingerprint)
        {
            return Err(format!("debt entry {:?} has incomplete evidence", entry.id));
        }
    }
    Ok(())
}

pub fn reconcile_debt(
    ledger: &ArchitectureDebtLedger,
    observed: &[ObservedDebt],
    today: &str,
) -> DebtReconciliation {
    let by_subject = ledger
        .entries
        .iter()
        .map(|entry| (entry.subject_path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut result = DebtReconciliation::default();
    for observation in observed {
        seen.insert(observation.subject_path.as_str());
        let Some(entry) = by_subject.get(observation.subject_path.as_str()).copied() else {
            result
                .items
                .push(item(observation, None, DebtClassification::New));
            result.diagnostics.push(debt_diagnostic(
                "architecture_debt.new",
                observation,
                None,
                "new",
                "Review the finding; do not silently add it to historical debt.",
            ));
            continue;
        };
        if entry.status == DebtStatus::Resolved {
            result.items.push(item(
                observation,
                Some(entry.id.clone()),
                DebtClassification::Reintroduced,
            ));
            result.diagnostics.push(debt_diagnostic(
                "architecture_debt.resolved_reintroduced",
                observation,
                Some(entry),
                "reintroduced",
                "Fix the reintroduced debt; a resolved baseline cannot be reopened automatically.",
            ));
        } else if entry.review_by.as_str() < today {
            result.items.push(item(
                observation,
                Some(entry.id.clone()),
                DebtClassification::Expired,
            ));
            result.diagnostics.push(debt_diagnostic(
                "architecture_debt.expired",
                observation,
                Some(entry),
                "expired",
                "Complete remediation or obtain an explicit reviewed extension.",
            ));
        } else if observation.risk > entry.baseline_risk
            || observation.dependencies > entry.baseline_dependencies
        {
            result.items.push(item(
                observation,
                Some(entry.id.clone()),
                DebtClassification::Regressed,
            ));
            result.diagnostics.push(debt_diagnostic(
                "architecture_debt.regressed",
                observation,
                Some(entry),
                "regressed",
                "Reduce risk/dependency growth below the committed baseline.",
            ));
        } else {
            let classification = if observation.risk < entry.baseline_risk
                || observation.dependencies < entry.baseline_dependencies
            {
                DebtClassification::Improved
            } else {
                DebtClassification::Known
            };
            result
                .items
                .push(item(observation, Some(entry.id.clone()), classification));
        }
    }
    for entry in &ledger.entries {
        if entry.status == DebtStatus::Active && !seen.contains(entry.subject_path.as_str()) {
            result.items.push(DebtReconciliationItem {
                subject_path: entry.subject_path.clone(),
                ledger_id: Some(entry.id.clone()),
                classification: DebtClassification::Resolved,
            });
        }
    }
    result
}

pub fn write_debt_candidate(
    workspace_root: &Path,
    ledger: &ArchitectureDebtLedger,
) -> Result<PathBuf, String> {
    validate_debt_ledger(ledger, "0000-00-00")?;
    let path = candidate_path(workspace_root, "architecture-debt-ledger.v1.json")?;
    let json = serde_json::to_string_pretty(ledger).map_err(|error| error.to_string())?;
    fs::write(&path, format!("{json}\n")).map_err(|error| error.to_string())?;
    Ok(path)
}

pub(crate) fn candidate_path(workspace_root: &Path, name: &str) -> Result<PathBuf, String> {
    let name_path = Path::new(name);
    if name_path.is_absolute()
        || name_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || name_path.components().count() != 1
    {
        return Err("candidate artifact name must be a single relative file name".to_string());
    }
    let root = workspace_root.join("target/quality-gate/candidate");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(root.join(name_path))
}

fn item(
    observation: &ObservedDebt,
    ledger_id: Option<String>,
    classification: DebtClassification,
) -> DebtReconciliationItem {
    DebtReconciliationItem {
        subject_path: observation.subject_path.clone(),
        ledger_id,
        classification,
    }
}

fn debt_diagnostic(
    code: &str,
    observed: &ObservedDebt,
    entry: Option<&ArchitectureDebtEntry>,
    classification: &str,
    next_action: &str,
) -> ArchitectureDiagnostic {
    ArchitectureDiagnostic {
        code: code.to_string(),
        source_path: Some(observed.subject_path.clone()),
        domain: entry.map(|entry| entry.domain.clone()),
        subject: Some(observed.fingerprint.clone()),
        stage: "architecture_debt".to_string(),
        observed_evidence: format!(
            "risk={}, dependencies={}",
            observed.risk, observed.dependencies
        ),
        rule_id: entry.map(|entry| entry.id.clone()),
        classification: classification.to_string(),
        next_action: next_action.to_string(),
    }
}

fn valid_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn valid_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn architecture_debt_rejects_regression_and_reintroduction() {
        let ledger = ledger_with(DebtStatus::Active);
        let regressed = reconcile_debt(
            &ledger,
            &[ObservedDebt {
                subject_path: "crates/a/src/lib.rs".to_string(),
                fingerprint: digest('b'),
                risk: 4,
                dependencies: 3,
            }],
            "2026-07-12",
        );
        assert!(!regressed.passed());
        assert_eq!(
            regressed.items[0].classification,
            DebtClassification::Regressed
        );

        let resolved = ledger_with(DebtStatus::Resolved);
        let reintroduced = reconcile_debt(
            &resolved,
            &[ObservedDebt {
                subject_path: "crates/a/src/lib.rs".to_string(),
                fingerprint: digest('b'),
                risk: 3,
                dependencies: 2,
            }],
            "2026-07-12",
        );
        assert_eq!(
            reintroduced.items[0].classification,
            DebtClassification::Reintroduced
        );
    }

    #[test]
    fn architecture_debt_candidate_does_not_mutate_committed_ledger() {
        let root = temp_root();
        fs::create_dir_all(root.join("quality")).unwrap();
        let committed = root.join("quality/architecture-debt-ledger.v1.json");
        fs::write(&committed, "committed").unwrap();
        let path = write_debt_candidate(&root, &ledger_with(DebtStatus::Active)).unwrap();
        assert!(path.starts_with(root.join("target/quality-gate/candidate")));
        assert_eq!(fs::read_to_string(&committed).unwrap(), "committed");
        fs::remove_dir_all(root).unwrap();
    }

    fn ledger_with(status: DebtStatus) -> ArchitectureDebtLedger {
        ArchitectureDebtLedger {
            schema_version: ARCHITECTURE_DEBT_SCHEMA_VERSION.to_string(),
            entries: vec![ArchitectureDebtEntry {
                id: "debt-a".to_string(),
                subject_path: "crates/a/src/lib.rs".to_string(),
                fingerprint: digest('a'),
                domain: "a".to_string(),
                owner: "owner-a".to_string(),
                status,
                reason: "historical mixed responsibility".to_string(),
                review_by: "2027-01-01".to_string(),
                plan: "extract responsibilities".to_string(),
                evidence_digest: digest('e'),
                baseline_risk: 3,
                baseline_dependencies: 2,
            }],
        }
    }

    fn digest(ch: char) -> String {
        format!("sha256:{}", ch.to_string().repeat(64))
    }

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("architecture_debt_{nanos}"))
    }
}
