use crate::architecture_coverage::{
    write_coverage_candidate, ArchitectureCoverageEntry, ArchitectureCoverageLedger,
    CoverageStatus, ARCHITECTURE_COVERAGE_SCHEMA_VERSION,
};
use crate::architecture_debt::{
    candidate_path, write_debt_candidate, ArchitectureDebtEntry, ArchitectureDebtLedger,
    DebtStatus, ARCHITECTURE_DEBT_SCHEMA_VERSION,
};
use crate::architecture_inventory::{build_inventory, ArchitectureInventory};
use crate::architecture_policy::{load_policy, ArchitecturePolicy};
use crate::cargo_json::sha256_hex;
use crate::command::{QualityCommandExecutor, QualityCommandSpec};
use code_hygiene::v2::{generate_hygiene_report_v2, HygieneFileEvidence, HygieneRiskBand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureBaselineCandidateReport {
    pub schema_version: String,
    pub inventory_files: usize,
    pub debt_entries: usize,
    pub coverage_entries: usize,
    pub inventory_digest: String,
    pub debt_candidate: String,
    pub coverage_candidate: String,
}

pub fn propose_architecture_baseline<E: QualityCommandExecutor>(
    executor: &E,
    workspace_root: &Path,
) -> Result<ArchitectureBaselineCandidateReport, String> {
    let policy = load_policy(&workspace_root.join("quality/architecture-policy.v1.toml"))
        .map_err(|error| error.to_string())?;
    let metadata = executor.execute(&QualityCommandSpec::new(
        "architecture_baseline_metadata",
        "cargo",
        ["metadata", "--locked", "--format-version", "1", "--no-deps"],
        workspace_root,
        Duration::from_secs(5 * 60),
    ));
    if !metadata.passed() {
        return Err("cargo metadata --locked failed during architecture bootstrap".to_string());
    }
    let inventory = build_inventory(workspace_root, &policy, "engine-strict", &metadata.stdout)
        .map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.observed_evidence))
                .collect::<Vec<_>>()
                .join("; ")
        })?;
    if !inventory.diagnostics.is_empty() {
        return Err(inventory
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.observed_evidence))
            .collect::<Vec<_>>()
            .join("; "));
    }
    let hygiene = generate_hygiene_report_v2(workspace_root.join("crates"))
        .map_err(|error| error.to_string())?;
    let debt = debt_candidate(&hygiene.file_evidence, &policy)?;
    let coverage = coverage_candidate(&hygiene.file_evidence, &policy)?;
    let debt_path = write_debt_candidate(workspace_root, &debt)?;
    let coverage_path = write_coverage_candidate(workspace_root, &coverage)?;
    write_inventory_candidate(workspace_root, &inventory)?;
    let report = ArchitectureBaselineCandidateReport {
        schema_version: "architecture-baseline-candidate-report.v1".to_string(),
        inventory_files: inventory.files.len(),
        debt_entries: debt.entries.len(),
        coverage_entries: coverage.entries.len(),
        inventory_digest: inventory.digest,
        debt_candidate: relative(workspace_root, &debt_path),
        coverage_candidate: relative(workspace_root, &coverage_path),
    };
    let report_path = candidate_path(workspace_root, "architecture-baseline-report.v1.json")?;
    write_json(&report_path, &report)?;
    Ok(report)
}

fn debt_candidate(
    files: &[HygieneFileEvidence],
    policy: &ArchitecturePolicy,
) -> Result<ArchitectureDebtLedger, String> {
    let mut entries = files
        .iter()
        .filter(|file| file.risk_band != HygieneRiskBand::Normal)
        .map(|file| {
            let policy_path = format!("crates/{}", file.path);
            let (domain, owner) = domain_owner(policy, &policy_path)?;
            let evidence_digest = digest(
                format!("{}\n{}\n{}", policy_path, file.lines, file.fingerprint).as_bytes(),
            );
            let id_hash = sha256_hex(policy_path.as_bytes());
            Ok(ArchitectureDebtEntry {
                id: format!("historical-{}", &id_hash[..16]),
                subject_path: policy_path,
                fingerprint: file.fingerprint.clone(),
                domain,
                owner,
                status: DebtStatus::Active,
                reason: format!(
                    "deterministic size risk {:?} at {} lines; size alone is not a semantic failure",
                    file.risk_band, file.lines
                ),
                review_by: "2027-07-12".to_string(),
                plan: "Review the subject by risk; use a dedicated extraction plan only when semantic evidence confirms mixed responsibility.".to_string(),
                evidence_digest,
                baseline_risk: risk_value(file.risk_band),
                baseline_dependencies: 0,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left.subject_path.cmp(&right.subject_path));
    Ok(ArchitectureDebtLedger {
        schema_version: ARCHITECTURE_DEBT_SCHEMA_VERSION.to_string(),
        entries,
    })
}

fn coverage_candidate(
    files: &[HygieneFileEvidence],
    policy: &ArchitecturePolicy,
) -> Result<ArchitectureCoverageLedger, String> {
    let mut entries = files
        .iter()
        .map(|file| {
            let policy_path = format!("crates/{}", file.path);
            let (domain, owner) = domain_owner(policy, &policy_path)?;
            Ok(ArchitectureCoverageEntry {
                subject: policy_path,
                domain,
                owner,
                status: CoverageStatus::CoveragePending,
                context_digest: file.content_digest.clone(),
                artifact_digest: None,
                reviewed_at: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left.subject.cmp(&right.subject));
    Ok(ArchitectureCoverageLedger {
        schema_version: ARCHITECTURE_COVERAGE_SCHEMA_VERSION.to_string(),
        entries,
    })
}

fn write_inventory_candidate(
    workspace_root: &Path,
    inventory: &ArchitectureInventory,
) -> Result<(), String> {
    let path = candidate_path(workspace_root, "architecture-inventory.v1.json")?;
    write_json(&path, inventory)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, format!("{json}\n")).map_err(|error| error.to_string())
}

fn domain_owner(policy: &ArchitecturePolicy, path: &str) -> Result<(String, String), String> {
    policy
        .domains
        .iter()
        .find(|domain| {
            domain
                .include
                .iter()
                .any(|pattern| crate::architecture_inventory::matches_pattern(path, pattern))
        })
        .map(|domain| (domain.id.clone(), domain.owner.clone()))
        .ok_or_else(|| format!("no policy domain owns {path}"))
}

fn risk_value(risk: HygieneRiskBand) -> u8 {
    match risk {
        HygieneRiskBand::Normal => 0,
        HygieneRiskBand::Review => 1,
        HygieneRiskBand::High => 2,
        HygieneRiskBand::Critical => 3,
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_bootstrap_risk_is_not_automatic_semantic_failure() {
        assert_eq!(risk_value(HygieneRiskBand::Review), 1);
        assert_eq!(risk_value(HygieneRiskBand::Critical), 3);
    }
}
