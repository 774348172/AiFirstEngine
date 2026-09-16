use quality_gate::architecture_artifact::{
    verify_artifact, ArchitectureReviewArtifact, ArtifactExpectation, ArtifactProvenance,
    ReviewOutcome, ARCHITECTURE_ARTIFACT_SCHEMA_VERSION,
};
use quality_gate::architecture_debt::{
    ArchitectureDebtEntry, ArchitectureDebtLedger, DebtStatus, ObservedDebt,
    ARCHITECTURE_DEBT_SCHEMA_VERSION,
};
use quality_gate::architecture_inventory::{
    ArchitectureInventory, InventoryFile, InventoryFileKind, ARCHITECTURE_INVENTORY_SCHEMA_VERSION,
};
use quality_gate::architecture_policy::{
    ArchitecturePolicy, DependencyRule, DomainPolicy, PolicyMode, PolicyProfile,
    ARCHITECTURE_POLICY_SCHEMA_VERSION,
};
use quality_gate::architecture_review::{
    ArchitectureFinding, ArchitectureReviewRequest, FindingCoverage, FindingSeverity,
    ARCHITECTURE_REVIEW_SCHEMA_VERSION,
};
use quality_gate::change_scope::{
    canonical_rust_subject, compute_change_scope, reconcile_touched_debt, ChangeClassification,
    ChangeScopeRequest, MergeBasePolicy, CHANGE_SCOPE_SCHEMA_VERSION,
};

#[test]
fn rename_move_preserves_canonical_subject_identity() {
    let before = canonical_rust_subject("src/old.rs", "pub struct Stable;", &[]).unwrap();
    let after = canonical_rust_subject("src/new.rs", "pub struct Stable;", &[]).unwrap();
    let scope = compute_change_scope(&request(), std::slice::from_ref(&before), &[after]);
    assert_eq!(
        scope.subjects[0].classification,
        ChangeClassification::RenameMove
    );
    assert_eq!(scope.subjects[0].identity_digest, before.ast_digest);
}

#[test]
fn touched_debt_blocks_growth_but_unrelated_debt_does_not() {
    let before = canonical_rust_subject("src/hot.rs", "pub fn old() {}", &[]).unwrap();
    let after = canonical_rust_subject(
        "src/hot.rs",
        "pub fn old() {} pub fn added_responsibility() {}",
        &["new_dependency".to_string()],
    )
    .unwrap();
    let scope = compute_change_scope(&request(), &[before], &[after]);
    let ledger = ledger();
    let observed = vec![
        ObservedDebt {
            subject_path: "src/hot.rs".to_string(),
            fingerprint: digest('b'),
            risk: 4,
            dependencies: 2,
        },
        ObservedDebt {
            subject_path: "src/unrelated.rs".to_string(),
            fingerprint: digest('c'),
            risk: 4,
            dependencies: 9,
        },
    ];
    let result = reconcile_touched_debt(&scope, &ledger, &observed, "2026-07-12");
    assert!(!result.passed());
    assert_eq!(result.items.len(), 1);
    assert!(result
        .items
        .iter()
        .any(|item| item.subject_path == "src/hot.rs"));
}

#[test]
fn ai_evidence_hallucinated_symbol_cannot_block() {
    let mut artifact = artifact(ArtifactProvenance::ScriptedFixture);
    artifact.findings[0].symbol = Some("fn:hallucinated".to_string());
    let result = verify_artifact(&artifact, &expectation(false), &inventory(), &policy());
    assert!(!result.passed());
    assert!(result
        .review
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "provider_invalid_evidence"));
}

#[test]
fn artifact_provenance_scripted_cannot_satisfy_merge() {
    let result = verify_artifact(
        &artifact(ArtifactProvenance::ScriptedFixture),
        &expectation(true),
        &inventory(),
        &policy(),
    );
    assert!(!result.passed());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "architecture_artifact.untrusted_provenance" }));
}

fn request() -> ChangeScopeRequest {
    ChangeScopeRequest {
        schema_version: CHANGE_SCOPE_SCHEMA_VERSION.to_string(),
        base_commit: "a".repeat(40),
        head_commit: "b".repeat(40),
        dirty_patch_digest: None,
        merge_base_policy: MergeBasePolicy::ExactBase,
        accepted_merge_base: None,
    }
}

fn ledger() -> ArchitectureDebtLedger {
    ArchitectureDebtLedger {
        schema_version: ARCHITECTURE_DEBT_SCHEMA_VERSION.to_string(),
        entries: vec![
            entry("hot", "src/hot.rs"),
            entry("unrelated", "src/unrelated.rs"),
        ],
    }
}

fn entry(id: &str, path: &str) -> ArchitectureDebtEntry {
    ArchitectureDebtEntry {
        id: id.to_string(),
        subject_path: path.to_string(),
        fingerprint: digest('a'),
        domain: "test".to_string(),
        owner: "test-owner".to_string(),
        status: DebtStatus::Active,
        reason: "historical debt".to_string(),
        review_by: "2027-01-01".to_string(),
        plan: "remediate".to_string(),
        evidence_digest: digest('e'),
        baseline_risk: 3,
        baseline_dependencies: 1,
    }
}

fn digest(ch: char) -> String {
    format!("sha256:{}", ch.to_string().repeat(64))
}

fn artifact(provenance: ArtifactProvenance) -> ArchitectureReviewArtifact {
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
        provider_id: "fixture".to_string(),
        model_id: "fixture-model".to_string(),
        generated_at_epoch_seconds: 100,
        expires_at_epoch_seconds: 200,
        findings: vec![ArchitectureFinding {
            id: "finding-1".to_string(),
            path: "crates/a/src/lib.rs".to_string(),
            symbol: Some("struct:Present".to_string()),
            issue_type: "forbidden_dependency".to_string(),
            severity: FindingSeverity::High,
            confidence: 0.95,
            rule_ids: vec!["runtime-to-quality".to_string()],
            coverage: FindingCoverage::Full,
            evidence_digest: digest('e'),
            observed_evidence: "dependency edge".to_string(),
        }],
        dispositions: Vec::new(),
    }
}

fn expectation(require_trusted_provider: bool) -> ArtifactExpectation {
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

fn inventory() -> ArchitectureInventory {
    ArchitectureInventory {
        schema_version: ARCHITECTURE_INVENTORY_SCHEMA_VERSION.to_string(),
        profile_id: "engine".to_string(),
        files: vec![InventoryFile {
            path: "crates/a/src/lib.rs".to_string(),
            domain: "runtime".to_string(),
            owner: "runtime".to_string(),
            kind: InventoryFileKind::Rust,
            content_digest: digest('f'),
            symbols: vec!["struct:Present".to_string()],
            imports: Vec::new(),
            impls: Vec::new(),
        }],
        crate_dependencies: Vec::new(),
        digest: digest('i'),
        diagnostics: Vec::new(),
    }
}

fn policy() -> ArchitecturePolicy {
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
