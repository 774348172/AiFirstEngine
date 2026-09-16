use crate::{digest, response_schema, DEFAULT_PROMPT};
use quality_gate::architecture_artifact::ArtifactExpectation;
use quality_gate::architecture_debt::{ArchitectureDebtLedger, DebtStatus};
use quality_gate::architecture_inventory::{ArchitectureInventory, InventoryFile};
use quality_gate::architecture_policy::ArchitecturePolicy;
use quality_gate::architecture_review::{
    ArchitectureReviewRequest, ARCHITECTURE_REVIEW_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const CANONICAL_CONTEXT_SCHEMA_VERSION: &str = "canonical-architecture-context.v1";
pub const DEFAULT_CONTEXT_LIMIT_BYTES: usize = 512 * 1024;
pub const DEFAULT_SUBJECT_LIMIT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalArchitectureContext {
    pub schema_version: String,
    pub profile_id: String,
    pub policy: ArchitecturePolicy,
    pub inventory_digest: String,
    pub subjects: Vec<CanonicalReviewSubject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalReviewSubject {
    pub path: String,
    pub domain: String,
    pub owner: String,
    pub content_digest: String,
    pub symbols: Vec<String>,
    pub imports: Vec<String>,
    pub impls: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedReviewBundle {
    pub request: ArchitectureReviewRequest,
    pub context_json: String,
}

pub struct ReviewBundleInput<'a> {
    pub workspace_root: &'a Path,
    pub policy: ArchitecturePolicy,
    pub inventory: &'a ArchitectureInventory,
    pub debt: &'a ArchitectureDebtLedger,
    pub base_commit: String,
    pub head_commit: String,
    pub policy_digest: String,
    pub coverage_digest: String,
    pub subject_limit: usize,
    pub context_limit_bytes: usize,
}

pub fn prepare_review_bundle(input: ReviewBundleInput<'_>) -> Result<PreparedReviewBundle, String> {
    if input.subject_limit == 0 || input.context_limit_bytes == 0 {
        return Err("subject and context limits must be non-zero".to_string());
    }
    let mut candidates = input
        .debt
        .entries
        .iter()
        .filter(|entry| entry.status == DebtStatus::Active)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .baseline_risk
            .cmp(&left.baseline_risk)
            .then_with(|| left.subject_path.cmp(&right.subject_path))
    });

    let mut context = CanonicalArchitectureContext {
        schema_version: CANONICAL_CONTEXT_SCHEMA_VERSION.to_string(),
        profile_id: input.inventory.profile_id.clone(),
        policy: input.policy,
        inventory_digest: input.inventory.digest.clone(),
        subjects: Vec::new(),
    };
    for debt_entry in candidates.into_iter().take(input.subject_limit) {
        let Some(file) = input
            .inventory
            .files
            .iter()
            .find(|file| file.path == debt_entry.subject_path)
        else {
            return Err(format!(
                "active debt subject {:?} is absent from inventory",
                debt_entry.subject_path
            ));
        };
        let subject = canonical_subject(input.workspace_root, file)?;
        context.subjects.push(subject);
        let candidate = canonical_context_json(&context)?;
        if candidate.len() > input.context_limit_bytes {
            context.subjects.pop();
            break;
        }
    }
    if context.subjects.is_empty() {
        return Err("no active debt subject fits the canonical context budget".to_string());
    }

    let context_json = canonical_context_json(&context)?;
    let request = ArchitectureReviewRequest {
        schema_version: ARCHITECTURE_REVIEW_SCHEMA_VERSION.to_string(),
        profile_id: input.inventory.profile_id.clone(),
        base_commit: input.base_commit,
        head_commit: input.head_commit,
        dirty_patch_digest: None,
        workspace_digest: input.inventory.digest.clone(),
        policy_digest: input.policy_digest,
        prompt_digest: digest(DEFAULT_PROMPT.as_bytes()),
        response_schema_digest: digest(
            &serde_json::to_vec(&response_schema()).map_err(|error| error.to_string())?,
        ),
        context_digest: digest(context_json.as_bytes()),
        coverage_digest: input.coverage_digest,
        subjects: context
            .subjects
            .iter()
            .map(|subject| subject.path.clone())
            .collect(),
    };
    Ok(PreparedReviewBundle {
        request,
        context_json,
    })
}

pub fn artifact_expectation(
    request: &ArchitectureReviewRequest,
    now_epoch_seconds: u64,
) -> ArtifactExpectation {
    ArtifactExpectation {
        base_commit: request.base_commit.clone(),
        head_commit: request.head_commit.clone(),
        dirty_patch_digest: request.dirty_patch_digest.clone(),
        workspace_digest: request.workspace_digest.clone(),
        policy_digest: request.policy_digest.clone(),
        prompt_digest: request.prompt_digest.clone(),
        response_schema_digest: request.response_schema_digest.clone(),
        context_digest: request.context_digest.clone(),
        coverage_digest: request.coverage_digest.clone(),
        require_trusted_provider: true,
        now_epoch_seconds,
    }
}

pub fn file_digest(path: &Path) -> Result<String, String> {
    fs::read(path)
        .map(|bytes| digest(&bytes))
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn canonical_subject(
    workspace_root: &Path,
    file: &InventoryFile,
) -> Result<CanonicalReviewSubject, String> {
    let source = fs::read_to_string(workspace_root.join(&file.path))
        .map_err(|error| format!("{}: {error}", file.path))?;
    Ok(CanonicalReviewSubject {
        path: file.path.clone(),
        domain: file.domain.clone(),
        owner: file.owner.clone(),
        content_digest: file.content_digest.clone(),
        symbols: file.symbols.clone(),
        imports: file.imports.clone(),
        impls: file.impls.clone(),
        source,
    })
}

fn canonical_context_json(context: &CanonicalArchitectureContext) -> Result<String, String> {
    serde_json::to_string(context)
        .map(|json| format!("{json}\n"))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quality_gate::architecture_debt::{
        ArchitectureDebtEntry, ARCHITECTURE_DEBT_SCHEMA_VERSION,
    };
    use quality_gate::architecture_inventory::{
        ArchitectureInventory, InventoryFileKind, ARCHITECTURE_INVENTORY_SCHEMA_VERSION,
    };
    use quality_gate::architecture_policy::{
        DependencyRule, DomainPolicy, PolicyMode, PolicyProfile, ARCHITECTURE_POLICY_SCHEMA_VERSION,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn bundle_selects_highest_risk_and_binds_canonical_context() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("architecture-review-bundle-{unique}"));
        fs::create_dir_all(root.join("crates/example/src")).unwrap();
        fs::write(root.join("crates/example/src/a.rs"), "pub fn a() {}\n").unwrap();
        fs::write(root.join("crates/example/src/b.rs"), "pub fn b() {}\n").unwrap();
        let files = vec![
            inventory_file("crates/example/src/a.rs", "fn:a"),
            inventory_file("crates/example/src/b.rs", "fn:b"),
        ];
        let inventory = ArchitectureInventory {
            schema_version: ARCHITECTURE_INVENTORY_SCHEMA_VERSION.to_string(),
            profile_id: "engine-strict".to_string(),
            files,
            crate_dependencies: Vec::new(),
            digest: digest(b"inventory"),
            diagnostics: Vec::new(),
        };
        let debt = ArchitectureDebtLedger {
            schema_version: ARCHITECTURE_DEBT_SCHEMA_VERSION.to_string(),
            entries: vec![
                debt_entry("crates/example/src/a.rs", 1),
                debt_entry("crates/example/src/b.rs", 3),
            ],
        };
        let bundle = prepare_review_bundle(ReviewBundleInput {
            workspace_root: &root,
            policy: policy(),
            inventory: &inventory,
            debt: &debt,
            base_commit: "a".repeat(40),
            head_commit: "b".repeat(40),
            policy_digest: digest(b"policy"),
            coverage_digest: digest(b"coverage"),
            subject_limit: 1,
            context_limit_bytes: 64 * 1024,
        })
        .unwrap();

        assert_eq!(
            bundle.request.subjects,
            vec!["crates/example/src/b.rs".to_string()]
        );
        assert_eq!(
            bundle.request.context_digest,
            digest(bundle.context_json.as_bytes())
        );
        let expectation = artifact_expectation(&bundle.request, 42);
        assert_eq!(expectation.head_commit, "b".repeat(40));
        assert_eq!(expectation.now_epoch_seconds, 42);
        assert!(expectation.require_trusted_provider);
        fs::remove_dir_all(root).unwrap();
    }

    fn inventory_file(path: &str, symbol: &str) -> InventoryFile {
        InventoryFile {
            path: path.to_string(),
            domain: "quality-tooling".to_string(),
            owner: "engine-quality".to_string(),
            kind: InventoryFileKind::Rust,
            content_digest: digest(path.as_bytes()),
            symbols: vec![symbol.to_string()],
            imports: Vec::new(),
            impls: Vec::new(),
        }
    }

    fn debt_entry(path: &str, risk: u8) -> ArchitectureDebtEntry {
        ArchitectureDebtEntry {
            id: format!("debt-{risk}"),
            subject_path: path.to_string(),
            fingerprint: digest(path.as_bytes()),
            domain: "quality-tooling".to_string(),
            owner: "engine-quality".to_string(),
            status: DebtStatus::Active,
            reason: "test".to_string(),
            review_by: "2027-01-01".to_string(),
            plan: "test".to_string(),
            evidence_digest: digest(format!("evidence-{risk}").as_bytes()),
            baseline_risk: risk,
            baseline_dependencies: 0,
        }
    }

    fn policy() -> ArchitecturePolicy {
        ArchitecturePolicy {
            schema_version: ARCHITECTURE_POLICY_SCHEMA_VERSION.to_string(),
            profiles: vec![PolicyProfile {
                id: "engine-strict".to_string(),
                mode: PolicyMode::EngineStrict,
                include: vec!["crates/**".to_string()],
                exclude: Vec::new(),
            }],
            domains: vec![DomainPolicy {
                id: "quality-tooling".to_string(),
                owner: "engine-quality".to_string(),
                include: vec!["crates/**".to_string()],
                facade_only: Vec::new(),
            }],
            dependency_rules: vec![DependencyRule {
                id: "quality-rule".to_string(),
                from: "quality-tooling".to_string(),
                to: "quality-tooling".to_string(),
                allowed: true,
            }],
            review_authorities: vec!["local-maintainer".to_string()],
        }
    }
}
