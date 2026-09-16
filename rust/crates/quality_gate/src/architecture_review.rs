use crate::architecture_inventory::ArchitectureInventory;
use crate::architecture_policy::{ArchitectureDiagnostic, ArchitecturePolicy};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const ARCHITECTURE_REVIEW_SCHEMA_VERSION: &str = "architecture-review.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureReviewRequest {
    pub schema_version: String,
    pub profile_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub dirty_patch_digest: Option<String>,
    pub workspace_digest: String,
    pub policy_digest: String,
    pub prompt_digest: String,
    pub response_schema_digest: String,
    pub context_digest: String,
    pub coverage_digest: String,
    pub subjects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureFinding {
    pub id: String,
    pub path: String,
    pub symbol: Option<String>,
    pub issue_type: String,
    pub severity: FindingSeverity,
    pub confidence: f32,
    pub rule_ids: Vec<String>,
    pub coverage: FindingCoverage,
    pub evidence_digest: String,
    pub observed_evidence: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCoverage {
    Full,
    Partial,
    CoveragePending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingState {
    Candidate,
    EvidenceVerified,
    ReviewRequired,
    ConfirmedBlocking,
    Rejected,
    Remediated,
    ApprovedException,
    InvalidEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindingDisposition {
    pub finding_id: String,
    pub decision: DispositionDecision,
    pub reviewer: String,
    pub reason: String,
    pub evidence_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispositionDecision {
    ConfirmedBlocking,
    Rejected,
    Remediated,
    ApprovedException,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifiedFinding {
    pub finding: ArchitectureFinding,
    pub state: FindingState,
    pub reviewer: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReviewVerification {
    pub findings: Vec<VerifiedFinding>,
    pub diagnostics: Vec<ArchitectureDiagnostic>,
}

impl ReviewVerification {
    pub fn passed(&self) -> bool {
        self.diagnostics.is_empty()
            && self.findings.iter().all(|finding| {
                !matches!(
                    finding.state,
                    FindingState::ReviewRequired
                        | FindingState::ConfirmedBlocking
                        | FindingState::InvalidEvidence
                )
            })
    }
}

pub fn verify_review_evidence(
    findings: &[ArchitectureFinding],
    dispositions: &[FindingDisposition],
    inventory: &ArchitectureInventory,
    policy: &ArchitecturePolicy,
) -> ReviewVerification {
    let files = inventory
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let rule_ids = policy
        .dependency_rules
        .iter()
        .map(|rule| rule.id.as_str())
        .collect::<BTreeSet<_>>();
    let disposition_by_id = dispositions
        .iter()
        .map(|disposition| (disposition.finding_id.as_str(), disposition))
        .collect::<BTreeMap<_, _>>();
    let authority_ids = policy
        .review_authorities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut result = ReviewVerification::default();
    let mut finding_ids = BTreeSet::new();

    for finding in findings {
        let mut invalid_reasons = Vec::new();
        if finding.id.trim().is_empty() || !finding_ids.insert(finding.id.as_str()) {
            invalid_reasons.push("finding ID is empty or duplicated".to_string());
        }
        if !valid_relative_path(&finding.path) {
            invalid_reasons.push("path is not a repository-relative normalized path".to_string());
        }
        let file = files.get(finding.path.as_str()).copied();
        if file.is_none() {
            invalid_reasons.push("path is absent from deterministic inventory".to_string());
        }
        if let (Some(symbol), Some(file)) = (finding.symbol.as_ref(), file) {
            if !file.symbols.contains(symbol) && !file.impls.contains(symbol) {
                invalid_reasons.push(format!("symbol {symbol:?} is absent from local inventory"));
            }
        }
        if finding.rule_ids.is_empty()
            || finding
                .rule_ids
                .iter()
                .any(|rule| !rule_ids.contains(rule.as_str()))
        {
            invalid_reasons.push("finding contains a missing or unknown rule ID".to_string());
        }
        if !(0.0..=1.0).contains(&finding.confidence)
            || !valid_digest(&finding.evidence_digest)
            || finding.observed_evidence.trim().is_empty()
            || finding.observed_evidence.len() > 2048
        {
            invalid_reasons.push("confidence or evidence contract is invalid".to_string());
        }

        if !invalid_reasons.is_empty() {
            result.diagnostics.push(finding_diagnostic(
                "provider_invalid_evidence",
                finding,
                invalid_reasons.join("; "),
                "Discard the provider finding and rerun with canonical local context.",
            ));
            result.findings.push(VerifiedFinding {
                finding: finding.clone(),
                state: FindingState::InvalidEvidence,
                reviewer: None,
            });
            continue;
        }

        let requires_disposition = finding.severity >= FindingSeverity::High;
        let disposition = disposition_by_id.get(finding.id.as_str()).copied();
        let (state, reviewer) = if requires_disposition {
            match disposition {
                None => {
                    result.diagnostics.push(finding_diagnostic(
                        "architecture_review.disposition_required",
                        finding,
                        "verified high/critical finding has no disposition",
                        "Record an authorized reviewer disposition.",
                    ));
                    (FindingState::ReviewRequired, None)
                }
                Some(disposition) if !valid_disposition(disposition, &authority_ids, finding) => {
                    result.diagnostics.push(finding_diagnostic(
                        "architecture_review.disposition_invalid",
                        finding,
                        "disposition reviewer, reason, or evidence digest is invalid",
                        "Use a committed review authority and a reason bound to this finding.",
                    ));
                    (FindingState::ReviewRequired, None)
                }
                Some(disposition) => (
                    match disposition.decision {
                        DispositionDecision::ConfirmedBlocking => FindingState::ConfirmedBlocking,
                        DispositionDecision::Rejected => FindingState::Rejected,
                        DispositionDecision::Remediated => FindingState::Remediated,
                        DispositionDecision::ApprovedException => FindingState::ApprovedException,
                    },
                    Some(disposition.reviewer.clone()),
                ),
            }
        } else {
            (FindingState::EvidenceVerified, None)
        };
        if state == FindingState::ConfirmedBlocking {
            result.diagnostics.push(finding_diagnostic(
                "architecture_review.confirmed_blocker",
                finding,
                "authorized reviewer confirmed a blocking finding",
                "Remediate the finding or obtain a separately reviewed exception.",
            ));
        }
        result.findings.push(VerifiedFinding {
            finding: finding.clone(),
            state,
            reviewer,
        });
    }
    result
}

fn valid_disposition(
    disposition: &FindingDisposition,
    authorities: &BTreeSet<&str>,
    finding: &ArchitectureFinding,
) -> bool {
    authorities.contains(disposition.reviewer.as_str())
        && !disposition.reason.trim().is_empty()
        && disposition.reason.len() <= 1024
        && disposition.evidence_digest == finding.evidence_digest
}

pub fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && !path.split('/').any(|part| matches!(part, "" | "." | ".."))
        && !path.contains('\\')
}

pub fn valid_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn finding_diagnostic(
    code: &str,
    finding: &ArchitectureFinding,
    evidence: impl Into<String>,
    next_action: &str,
) -> ArchitectureDiagnostic {
    ArchitectureDiagnostic {
        code: code.to_string(),
        source_path: Some(finding.path.clone()),
        domain: None,
        subject: finding.symbol.clone(),
        stage: "architecture_review".to_string(),
        observed_evidence: evidence.into(),
        rule_id: finding.rule_ids.first().cloned(),
        classification: "review_required".to_string(),
        next_action: next_action.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::architecture_inventory::{
        ArchitectureInventory, InventoryFile, InventoryFileKind,
        ARCHITECTURE_INVENTORY_SCHEMA_VERSION,
    };
    use crate::architecture_policy::{
        ArchitecturePolicy, DependencyRule, DomainPolicy, PolicyMode, PolicyProfile,
        ARCHITECTURE_POLICY_SCHEMA_VERSION,
    };

    #[test]
    fn architecture_review_rejects_hallucinated_symbol() {
        let mut finding = finding();
        finding.symbol = Some("fn:missing".to_string());
        let result = verify_review_evidence(&[finding], &[], &inventory(), &policy());
        assert!(!result.passed());
        assert_eq!(result.findings[0].state, FindingState::InvalidEvidence);
    }

    pub(crate) fn finding() -> ArchitectureFinding {
        ArchitectureFinding {
            id: "finding-1".to_string(),
            path: "crates/a/src/lib.rs".to_string(),
            symbol: Some("struct:Present".to_string()),
            issue_type: "forbidden_dependency".to_string(),
            severity: FindingSeverity::High,
            confidence: 0.9,
            rule_ids: vec!["runtime-to-quality".to_string()],
            coverage: FindingCoverage::Full,
            evidence_digest: digest('a'),
            observed_evidence: "local dependency edge".to_string(),
        }
    }

    pub(crate) fn inventory() -> ArchitectureInventory {
        ArchitectureInventory {
            schema_version: ARCHITECTURE_INVENTORY_SCHEMA_VERSION.to_string(),
            profile_id: "engine".to_string(),
            files: vec![InventoryFile {
                path: "crates/a/src/lib.rs".to_string(),
                domain: "runtime".to_string(),
                owner: "runtime".to_string(),
                kind: InventoryFileKind::Rust,
                content_digest: digest('c'),
                symbols: vec!["struct:Present".to_string()],
                imports: Vec::new(),
                impls: Vec::new(),
            }],
            crate_dependencies: Vec::new(),
            digest: digest('i'),
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn policy() -> ArchitecturePolicy {
        ArchitecturePolicy {
            schema_version: ARCHITECTURE_POLICY_SCHEMA_VERSION.to_string(),
            profiles: vec![
                profile("engine", PolicyMode::EngineStrict),
                profile("advisory", PolicyMode::ProjectAdvisory),
                profile("strict", PolicyMode::ProjectStrict),
            ],
            domains: vec![DomainPolicy {
                id: "runtime".to_string(),
                owner: "runtime".to_string(),
                include: vec!["crates/a/**".to_string()],
                facade_only: Vec::new(),
            }],
            dependency_rules: vec![DependencyRule {
                id: "runtime-to-quality".to_string(),
                from: "runtime".to_string(),
                to: "quality".to_string(),
                allowed: false,
            }],
            review_authorities: vec!["reviewer".to_string()],
        }
    }

    fn profile(id: &str, mode: PolicyMode) -> PolicyProfile {
        PolicyProfile {
            id: id.to_string(),
            mode,
            include: vec!["crates/**".to_string()],
            exclude: Vec::new(),
        }
    }

    pub(crate) fn digest(ch: char) -> String {
        format!("sha256:{}", ch.to_string().repeat(64))
    }
}
