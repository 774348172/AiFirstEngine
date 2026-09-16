use crate::architecture_inventory::ArchitectureInventory;
use crate::architecture_policy::{ArchitectureDiagnostic, ArchitecturePolicy};
use crate::architecture_review::{
    valid_digest, verify_review_evidence, ArchitectureFinding, ArchitectureReviewRequest,
    FindingDisposition, ReviewVerification, ARCHITECTURE_REVIEW_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

pub const ARCHITECTURE_ARTIFACT_SCHEMA_VERSION: &str = "architecture-review-artifact.v1";
pub const MAX_ARCHITECTURE_ARTIFACT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureReviewArtifact {
    pub schema_version: String,
    pub request: ArchitectureReviewRequest,
    pub outcome: ReviewOutcome,
    pub provenance: ArtifactProvenance,
    pub provider_id: String,
    pub model_id: String,
    pub generated_at_epoch_seconds: u64,
    pub expires_at_epoch_seconds: u64,
    pub findings: Vec<ArchitectureFinding>,
    pub dispositions: Vec<FindingDisposition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewOutcome {
    Complete,
    Partial,
    Refused,
    TimedOut,
    BudgetExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactProvenance {
    TrustedProvider,
    ScriptedFixture,
    RecordedFixture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactExpectation {
    pub base_commit: String,
    pub head_commit: String,
    pub dirty_patch_digest: Option<String>,
    pub workspace_digest: String,
    pub policy_digest: String,
    pub prompt_digest: String,
    pub response_schema_digest: String,
    pub context_digest: String,
    pub coverage_digest: String,
    pub require_trusted_provider: bool,
    pub now_epoch_seconds: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtifactVerification {
    pub review: ReviewVerification,
    pub diagnostics: Vec<ArchitectureDiagnostic>,
}

impl ArtifactVerification {
    pub fn passed(&self) -> bool {
        self.diagnostics.is_empty() && self.review.passed()
    }
}

pub fn verify_artifact(
    artifact: &ArchitectureReviewArtifact,
    expectation: &ArtifactExpectation,
    inventory: &ArchitectureInventory,
    policy: &ArchitecturePolicy,
) -> ArtifactVerification {
    let mut verification = ArtifactVerification::default();
    let serialized = match serde_json::to_vec(artifact) {
        Ok(serialized) => serialized,
        Err(error) => {
            verification.diagnostics.push(artifact_diagnostic(
                "architecture_artifact.serialize_failed",
                error.to_string(),
                "Repair the strict artifact schema.",
            ));
            return verification;
        }
    };
    if serialized.len() > MAX_ARCHITECTURE_ARTIFACT_BYTES {
        verification.diagnostics.push(artifact_diagnostic(
            "architecture_artifact.oversize",
            format!("artifact is {} bytes", serialized.len()),
            "Reduce findings/context to the committed artifact budget.",
        ));
    }
    if artifact.schema_version != ARCHITECTURE_ARTIFACT_SCHEMA_VERSION
        || artifact.request.schema_version != ARCHITECTURE_REVIEW_SCHEMA_VERSION
    {
        verification.diagnostics.push(artifact_diagnostic(
            "architecture_artifact.schema_mismatch",
            "artifact or request schema version is unsupported",
            "Use the committed v1 architecture review schemas.",
        ));
    }
    if artifact.outcome != ReviewOutcome::Complete {
        verification.diagnostics.push(artifact_diagnostic(
            "architecture_artifact.incomplete",
            format!("provider outcome is {:?}", artifact.outcome),
            "Rerun the missing review coverage within budget.",
        ));
    }
    if expectation.require_trusted_provider
        && artifact.provenance != ArtifactProvenance::TrustedProvider
    {
        verification.diagnostics.push(artifact_diagnostic(
            "architecture_artifact.untrusted_provenance",
            format!("artifact provenance is {:?}", artifact.provenance),
            "Run the review with a configured trusted Provider.",
        ));
    }
    if artifact.generated_at_epoch_seconds > expectation.now_epoch_seconds
        || artifact.expires_at_epoch_seconds < expectation.now_epoch_seconds
        || artifact.expires_at_epoch_seconds <= artifact.generated_at_epoch_seconds
    {
        verification.diagnostics.push(artifact_diagnostic(
            "architecture_artifact.stale",
            "artifact is not fresh at the verification time",
            "Generate a fresh artifact for the exact candidate.",
        ));
    }
    verify_binding(artifact, expectation, &mut verification.diagnostics);
    if let Some(redaction) = redaction_violation(&serialized) {
        verification.diagnostics.push(artifact_diagnostic(
            "architecture_artifact.redaction_failed",
            redaction,
            "Remove credentials, absolute paths, and raw Provider responses.",
        ));
    }
    verification.review = verify_review_evidence(
        &artifact.findings,
        &artifact.dispositions,
        inventory,
        policy,
    );
    verification
}

fn verify_binding(
    artifact: &ArchitectureReviewArtifact,
    expected: &ArtifactExpectation,
    diagnostics: &mut Vec<ArchitectureDiagnostic>,
) {
    let actual = &artifact.request;
    let bindings = [
        (
            "base_commit",
            actual.base_commit.as_str(),
            expected.base_commit.as_str(),
        ),
        (
            "head_commit",
            actual.head_commit.as_str(),
            expected.head_commit.as_str(),
        ),
        (
            "workspace_digest",
            actual.workspace_digest.as_str(),
            expected.workspace_digest.as_str(),
        ),
        (
            "policy_digest",
            actual.policy_digest.as_str(),
            expected.policy_digest.as_str(),
        ),
        (
            "prompt_digest",
            actual.prompt_digest.as_str(),
            expected.prompt_digest.as_str(),
        ),
        (
            "response_schema_digest",
            actual.response_schema_digest.as_str(),
            expected.response_schema_digest.as_str(),
        ),
        (
            "context_digest",
            actual.context_digest.as_str(),
            expected.context_digest.as_str(),
        ),
        (
            "coverage_digest",
            actual.coverage_digest.as_str(),
            expected.coverage_digest.as_str(),
        ),
    ];
    for (name, actual, expected) in bindings {
        if actual != expected {
            diagnostics.push(artifact_diagnostic(
                "architecture_artifact.binding_mismatch",
                format!("{name} does not match the expected candidate"),
                "Regenerate the artifact for the exact candidate and configuration.",
            ));
        }
    }
    if actual.dirty_patch_digest != expected.dirty_patch_digest {
        diagnostics.push(artifact_diagnostic(
            "architecture_artifact.binding_mismatch",
            "dirty_patch_digest does not match the expected candidate",
            "Regenerate the artifact for the canonical patch.",
        ));
    }
    for digest in [
        &actual.workspace_digest,
        &actual.policy_digest,
        &actual.prompt_digest,
        &actual.response_schema_digest,
        &actual.context_digest,
        &actual.coverage_digest,
    ] {
        if !valid_digest(digest) {
            diagnostics.push(artifact_diagnostic(
                "architecture_artifact.digest_invalid",
                "artifact contains a malformed SHA-256 binding",
                "Recompute all artifact bindings from canonical bytes.",
            ));
        }
    }
}

fn redaction_violation(serialized: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(serialized);
    [
        "Bearer ",
        "sk-",
        "C:\\\\Users\\\\",
        "/home/",
        "raw_response",
    ]
    .into_iter()
    .find(|pattern| text.contains(pattern))
    .map(|pattern| format!("artifact contains prohibited pattern {pattern:?}"))
}

fn artifact_diagnostic(
    code: impl Into<String>,
    evidence: impl Into<String>,
    next_action: impl Into<String>,
) -> ArchitectureDiagnostic {
    ArchitectureDiagnostic {
        code: code.into(),
        source_path: None,
        domain: None,
        subject: None,
        stage: "architecture_artifact".to_string(),
        observed_evidence: evidence.into(),
        rule_id: None,
        classification: "artifact_invalid".to_string(),
        next_action: next_action.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture_review::tests::{digest, finding, inventory, policy};

    #[test]
    fn architecture_artifact_scripted_fixture_cannot_satisfy_trusted_expectation() {
        let artifact = artifact(ArtifactProvenance::ScriptedFixture);
        let result = verify_artifact(&artifact, &expectation(true), &inventory(), &policy());
        assert!(!result.passed());
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "architecture_artifact.untrusted_provenance" }));
    }

    pub(crate) fn artifact(provenance: ArtifactProvenance) -> ArchitectureReviewArtifact {
        ArchitectureReviewArtifact {
            schema_version: ARCHITECTURE_ARTIFACT_SCHEMA_VERSION.to_string(),
            request: ArchitectureReviewRequest {
                schema_version: ARCHITECTURE_REVIEW_SCHEMA_VERSION.to_string(),
                profile_id: "engine".to_string(),
                base_commit: "a".repeat(40),
                head_commit: "b".repeat(40),
                dirty_patch_digest: None,
                workspace_digest: digest('w'),
                policy_digest: digest('p'),
                prompt_digest: digest('r'),
                response_schema_digest: digest('s'),
                context_digest: digest('c'),
                coverage_digest: digest('v'),
                subjects: vec!["crates/a/src/lib.rs".to_string()],
            },
            outcome: ReviewOutcome::Complete,
            provenance,
            provider_id: "test-provider".to_string(),
            model_id: "test-model".to_string(),
            generated_at_epoch_seconds: 100,
            expires_at_epoch_seconds: 200,
            findings: vec![finding()],
            dispositions: Vec::new(),
        }
    }

    pub(crate) fn expectation(require_trusted_provider: bool) -> ArtifactExpectation {
        ArtifactExpectation {
            base_commit: "a".repeat(40),
            head_commit: "b".repeat(40),
            dirty_patch_digest: None,
            workspace_digest: digest('w'),
            policy_digest: digest('p'),
            prompt_digest: digest('r'),
            response_schema_digest: digest('s'),
            context_digest: digest('c'),
            coverage_digest: digest('v'),
            require_trusted_provider,
            now_epoch_seconds: 150,
        }
    }
}
