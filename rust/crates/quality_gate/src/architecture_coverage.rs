use crate::architecture_debt::candidate_path;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const ARCHITECTURE_COVERAGE_SCHEMA_VERSION: &str = "architecture-review-coverage.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureCoverageLedger {
    pub schema_version: String,
    pub entries: Vec<ArchitectureCoverageEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureCoverageEntry {
    pub subject: String,
    pub domain: String,
    pub owner: String,
    pub status: CoverageStatus,
    pub context_digest: String,
    pub artifact_digest: Option<String>,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    CoveragePending,
    Reviewed,
}

pub fn read_coverage(path: &Path) -> Result<ArchitectureCoverageLedger, String> {
    let source =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let ledger: ArchitectureCoverageLedger =
        serde_json::from_str(&source).map_err(|error| error.to_string())?;
    validate_coverage(&ledger)?;
    Ok(ledger)
}

pub fn validate_coverage(ledger: &ArchitectureCoverageLedger) -> Result<(), String> {
    if ledger.schema_version != ARCHITECTURE_COVERAGE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported architecture coverage schema {:?}",
            ledger.schema_version
        ));
    }
    let mut subjects = BTreeSet::new();
    for entry in &ledger.entries {
        if entry.subject.trim().is_empty()
            || entry.domain.trim().is_empty()
            || entry.owner.trim().is_empty()
            || !subjects.insert(entry.subject.as_str())
            || !valid_digest(&entry.context_digest)
        {
            return Err(format!("invalid coverage entry for {:?}", entry.subject));
        }
        match entry.status {
            CoverageStatus::CoveragePending => {
                if entry.artifact_digest.is_some() || entry.reviewed_at.is_some() {
                    return Err(format!(
                        "coverage_pending subject {:?} cannot claim review evidence",
                        entry.subject
                    ));
                }
            }
            CoverageStatus::Reviewed => {
                if !entry.artifact_digest.as_deref().is_some_and(valid_digest)
                    || entry.reviewed_at.as_deref().is_none_or(str::is_empty)
                {
                    return Err(format!(
                        "reviewed subject {:?} is missing artifact evidence",
                        entry.subject
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn write_coverage_candidate(
    workspace_root: &Path,
    ledger: &ArchitectureCoverageLedger,
) -> Result<PathBuf, String> {
    validate_coverage(ledger)?;
    let path = candidate_path(workspace_root, "architecture-review-coverage.v1.json")?;
    let json = serde_json::to_string_pretty(ledger).map_err(|error| error.to_string())?;
    fs::write(&path, format!("{json}\n")).map_err(|error| error.to_string())?;
    Ok(path)
}

fn valid_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_coverage_pending_cannot_claim_reviewed() {
        let ledger = ArchitectureCoverageLedger {
            schema_version: ARCHITECTURE_COVERAGE_SCHEMA_VERSION.to_string(),
            entries: vec![ArchitectureCoverageEntry {
                subject: "crates/a/src/lib.rs".to_string(),
                domain: "a".to_string(),
                owner: "owner-a".to_string(),
                status: CoverageStatus::CoveragePending,
                context_digest: digest('a'),
                artifact_digest: Some(digest('b')),
                reviewed_at: Some("2026-07-12T00:00:00Z".to_string()),
            }],
        };
        assert!(validate_coverage(&ledger).is_err());
    }

    fn digest(ch: char) -> String {
        format!("sha256:{}", ch.to_string().repeat(64))
    }
}
