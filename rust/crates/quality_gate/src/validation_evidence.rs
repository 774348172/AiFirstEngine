use crate::validation_catalog::{EnvironmentIdentity, EvidenceIdentityKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRecord {
    pub evidence_ref: String,
    pub verifier_id: String,
    pub consumed_identities: BTreeMap<EvidenceIdentityKind, String>,
    pub environment: EnvironmentIdentity,
    pub report_digest: Option<String>,
    pub passed: bool,
}

pub trait EvidenceStore {
    fn records_for(&self, verifier_id: &str) -> Vec<EvidenceRecord>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyEvidenceStore;

impl EvidenceStore for EmptyEvidenceStore {
    fn records_for(&self, _verifier_id: &str) -> Vec<EvidenceRecord> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryEvidenceStore {
    records: Vec<EvidenceRecord>,
}

impl InMemoryEvidenceStore {
    pub fn new(records: Vec<EvidenceRecord>) -> Self {
        Self { records }
    }
}

impl EvidenceStore for InMemoryEvidenceStore {
    fn records_for(&self, verifier_id: &str) -> Vec<EvidenceRecord> {
        self.records
            .iter()
            .filter(|record| record.verifier_id == verifier_id)
            .cloned()
            .collect()
    }
}

pub fn evidence_is_reusable(
    record: &EvidenceRecord,
    environment: &EnvironmentIdentity,
    expected_identities: &BTreeMap<EvidenceIdentityKind, String>,
    consumed_kinds: &[EvidenceIdentityKind],
) -> bool {
    record.passed
        && record.environment == *environment
        && record
            .report_digest
            .as_deref()
            .is_some_and(valid_sha256_digest)
        && consumed_kinds.iter().all(|kind| {
            expected_identities
                .get(kind)
                .zip(record.consumed_identities.get(kind))
                .is_some_and(|(expected, observed)| expected == observed)
        })
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment() -> EnvironmentIdentity {
        EnvironmentIdentity {
            platform: "windows-x86_64".to_string(),
            profile: "debug".to_string(),
            features: Vec::new(),
            composition: "source".to_string(),
        }
    }

    fn record() -> EvidenceRecord {
        EvidenceRecord {
            evidence_ref: "owner-pass".to_string(),
            verifier_id: "owner".to_string(),
            consumed_identities: BTreeMap::from([(
                EvidenceIdentityKind::ProductSource,
                "sha256:source".to_string(),
            )]),
            environment: environment(),
            report_digest: Some(format!("sha256:{}", "a".repeat(64))),
            passed: true,
        }
    }

    #[test]
    fn evidence_matching_consumed_identity_is_reusable() {
        let expected = BTreeMap::from([(
            EvidenceIdentityKind::ProductSource,
            "sha256:source".to_string(),
        )]);
        assert!(evidence_is_reusable(
            &record(),
            &environment(),
            &expected,
            &[EvidenceIdentityKind::ProductSource]
        ));
    }

    #[test]
    fn evidence_harness_change_only_invalidates_harness_consumers() {
        let expected = BTreeMap::from([
            (
                EvidenceIdentityKind::ProductSource,
                "sha256:source".to_string(),
            ),
            (
                EvidenceIdentityKind::VerificationHarness,
                "sha256:new-harness".to_string(),
            ),
        ]);
        assert!(evidence_is_reusable(
            &record(),
            &environment(),
            &expected,
            &[EvidenceIdentityKind::ProductSource]
        ));
        assert!(!evidence_is_reusable(
            &record(),
            &environment(),
            &expected,
            &[
                EvidenceIdentityKind::ProductSource,
                EvidenceIdentityKind::VerificationHarness,
            ]
        ));
    }

    #[test]
    fn evidence_missing_report_digest_is_not_reusable() {
        let mut record = record();
        record.report_digest = None;
        assert!(!evidence_is_reusable(
            &record,
            &environment(),
            &record.consumed_identities,
            &[EvidenceIdentityKind::ProductSource]
        ));
    }

    #[test]
    fn evidence_binary_or_composition_mismatch_is_not_reusable() {
        let mut record = record();
        record.consumed_identities.insert(
            EvidenceIdentityKind::ProductionBinary,
            "sha256:old-binary".to_string(),
        );
        let expected = BTreeMap::from([
            (
                EvidenceIdentityKind::ProductSource,
                "sha256:source".to_string(),
            ),
            (
                EvidenceIdentityKind::ProductionBinary,
                "sha256:new-binary".to_string(),
            ),
        ]);
        assert!(!evidence_is_reusable(
            &record,
            &environment(),
            &expected,
            &[
                EvidenceIdentityKind::ProductSource,
                EvidenceIdentityKind::ProductionBinary,
            ]
        ));
        let mut other_environment = environment();
        other_environment.composition = "installed".to_string();
        assert!(!evidence_is_reusable(
            &record,
            &other_environment,
            &record.consumed_identities,
            &[EvidenceIdentityKind::ProductSource]
        ));
    }
}
